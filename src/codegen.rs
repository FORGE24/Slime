use std::collections::HashMap;
use std::fmt::Write as _;

use crate::ast::*;
use crate::ctfe::CtfeValue;
use crate::diag::{Diagnostic, Span};

fn argc_mismatch(callee_span: Span, call_span: Span, expected: usize, got: usize) -> Diagnostic {
    Diagnostic::error(format!(
        "this function takes {expected} argument(s) but {got} were supplied"
    ))
    .code("E0061")
    .label(call_span, "argument count mismatch")
    .label(callee_span, "builtin")
}


use crate::etca::{EtcaBundle, EtcaOpts};

/// Emits textual LLVM IR. ETCA (CTFE/DOPE/Precomp/TCE) folds into **IR constants**
/// — the AST is read-only.
pub struct Codegen {
    strings: Vec<String>,
    late_globals: String,
    next_tmp: usize,
    next_label: usize,
    locals: HashMap<String, (LlvmTy, String, Span)>,
    /// Locals that hold heap C strings (not opaque ptrs like dict).
    str_locals: std::collections::HashSet<String>,
    /// Locals that are ref/out parameters or ref bindings — auto-deref on load.
    ref_locals: HashMap<String, Type>,
    struct_defs: HashMap<String, StructDef>,
    func_sigs: HashMap<String, FuncSig>,
    out: String,
    terminated: bool,
    etca: Option<EtcaBundle>,
    current_ret: Option<Type>,
    is_main: bool,
    /// `(continue_label, break_label)` for enclosing loops
    loop_stack: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct FuncSig {
    params: Vec<(ParamMode, Type)>,
    ret_ty: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LlvmTy {
    I64,
    F64,
    I1,
    I8Ptr,
    Struct(String),
    Array {
        elem: Box<LlvmTy>,
        len: i64,
    },
    Void,
}

impl LlvmTy {
    fn ir_type(&self) -> String {
        match self {
            LlvmTy::I64 => "i64".into(),
            LlvmTy::F64 => "double".into(),
            LlvmTy::I1 => "i1".into(),
            LlvmTy::I8Ptr => "ptr".into(),
            LlvmTy::Struct(name) => format!("%{name}"),
            LlvmTy::Array { elem, len } => format!("[{len} x {}]", elem.ir_type()),
            LlvmTy::Void => "void".into(),
        }
    }

    fn is_struct(&self) -> bool {
        matches!(self, LlvmTy::Struct(_))
    }

    fn is_array(&self) -> bool {
        matches!(self, LlvmTy::Array { .. })
    }

    fn array_elem_len(&self) -> Option<(LlvmTy, i64)> {
        match self {
            LlvmTy::Array { elem, len } => Some((*elem.clone(), *len)),
            _ => None,
        }
    }
}

struct Value {
    ty: LlvmTy,
    repr: String,
    /// `true` when `repr` is an alloca pointer to a struct (not a loaded value).
    is_struct_ptr: bool,
    /// Heap string the caller must free or transfer (itoa/md5/strcat/strdup).
    owned: bool,
}

impl Value {
    fn scalar(ty: LlvmTy, repr: String) -> Self {
        Self {
            ty,
            repr,
            is_struct_ptr: false,
            owned: false,
        }
    }

    fn owned_str(repr: String) -> Self {
        Self {
            ty: LlvmTy::I8Ptr,
            repr,
            is_struct_ptr: false,
            owned: true,
        }
    }

    fn struct_ptr(name: String, ptr: String) -> Self {
        Self {
            ty: LlvmTy::Struct(name),
            repr: ptr,
            is_struct_ptr: true,
            owned: false,
        }
    }
}

impl Codegen {
    pub fn compile(program: &Program, opts: EtcaOpts) -> Result<(String, Option<EtcaBundle>), Diagnostic> {
        let etca = if opts.any_enabled() {
            let mut e = EtcaBundle::new(opts);
            e.register_functions_from_program(program);
            Some(e)
        } else {
            None
        };

        let struct_defs: HashMap<String, StructDef> = program
            .structs
            .iter()
            .map(|s| (s.name.clone(), s.clone()))
            .collect();

        let mut func_sigs = HashMap::new();
        for func in &program.functions {
            func_sigs.insert(
                func.name.clone(),
                FuncSig {
                    params: func
                        .params
                        .iter()
                        .map(|p| (p.mode, p.ty.clone()))
                        .collect(),
                    ret_ty: func.ret_ty.clone(),
                },
            );
        }

        let mut cg = Self {
            strings: Vec::new(),
            late_globals: String::new(),
            next_tmp: 0,
            next_label: 0,
            locals: HashMap::new(),
            str_locals: std::collections::HashSet::new(),
            ref_locals: HashMap::new(),
            struct_defs,
            func_sigs,
            out: String::new(),
            terminated: false,
            etca,
            current_ret: None,
            is_main: false,
            loop_stack: Vec::new(),
        };
        cg.emit_program(program)?;
        if !cg.late_globals.is_empty() {
            cg.out.push_str(&cg.late_globals);
        }
        Ok((cg.out, cg.etca))
    }

    fn resolve_type(&self, ty: &Type, span: Span) -> Result<LlvmTy, Diagnostic> {
        match ty {
            Type::Int => Ok(LlvmTy::I64),
            Type::Float => Ok(LlvmTy::F64),
            Type::Bool => Ok(LlvmTy::I1),
            Type::Str => Ok(LlvmTy::I8Ptr),
            Type::Void => Ok(LlvmTy::Void),
            Type::Infer => Err(Diagnostic::error("type annotation required")
                .code("E0282")
                .label(span, "cannot infer type here")),
            Type::Named(name) => {
                if self.struct_defs.contains_key(name) {
                    Ok(LlvmTy::Struct(name.clone()))
                } else {
                    Err(Diagnostic::error(format!("unknown type `{name}`"))
                        .code("E0412")
                        .label(span, format!("cannot find type `{name}` in this scope")))
                }
            }
            Type::Apply { name, args } => {
                let mono = format!(
                    "{}__{}",
                    name,
                    args.iter().map(Type::mono_name).collect::<Vec<_>>().join("_")
                );
                if self.struct_defs.contains_key(&mono) {
                    Ok(LlvmTy::Struct(mono))
                } else {
                    Err(Diagnostic::error(format!("unknown monomorphized type `{mono}`"))
                        .code("E0412")
                        .label(span, "type should be monomorphized before codegen"))
                }
            }
            Type::Owned { inner, .. } | Type::Borrowed { inner, .. } => {
                // Ownership is erased for LLVM; share/joint become i8* cells.
                match ty {
                    Type::Owned {
                        spec:
                            OwnSpec {
                                kind: OwnKind::Share(_) | OwnKind::Joint,
                                ..
                            },
                        ..
                    } => Ok(LlvmTy::I8Ptr),
                    _ => self.resolve_type(inner, span),
                }
            }
            Type::Ptr(inner) => match inner.as_ref() {
                Type::Int | Type::Float | Type::Bool | Type::Str => Ok(LlvmTy::I8Ptr),
                Type::Named(name) if self.struct_defs.contains_key(name) => Ok(LlvmTy::I8Ptr),
                other => self.resolve_type(other, span).map(|_| LlvmTy::I8Ptr),
            },
            Type::Generic(name) => Err(Diagnostic::error(format!(
                "unresolved generic type parameter `{name}`"
            ))
            .code("E0412")
            .label(span, "generic type should be monomorphized")),
            Type::Array { elem, len } => {
                let elem_lty = self.resolve_type(elem, span)?;
                Ok(LlvmTy::Array {
                    elem: Box::new(elem_lty),
                    len: *len,
                })
            }
        }
    }

    fn struct_has_virtual(&self, struct_name: &str) -> bool {
        let prefix = format!("__virt_{struct_name}_");
        self.func_sigs.keys().any(|k| k.starts_with(&prefix))
    }

    fn virtual_methods(&self, struct_name: &str) -> Vec<String> {
        let prefix = format!("__virt_{struct_name}_");
        let mut names: Vec<_> = self
            .func_sigs
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        names.sort();
        names
    }

    fn struct_field_llvm_index(&self, struct_name: &str, field_index: usize) -> usize {
        if self.struct_has_virtual(struct_name) {
            field_index + 1
        } else {
            field_index
        }
    }

    fn struct_field_index(&self, struct_name: &str, field: &str, field_span: Span) -> Result<(usize, Type), Diagnostic> {
        let def = self.struct_defs.get(struct_name).ok_or_else(|| {
            Diagnostic::error(format!("unknown struct `{struct_name}`"))
                .code("E0412")
                .label(field_span, "struct not defined")
        })?;
        def.fields
            .iter()
            .position(|f| f.name == field)
            .map(|i| (self.struct_field_llvm_index(struct_name, i), def.fields[i].ty.clone()))
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "struct `{struct_name}` has no field named `{field}`"
                ))
                .code("E0609")
                .label(field_span, format!("no field `{field}` on `{struct_name}`"))
            })
    }

    /// Emit a CTFE value as an LLVM SSA/constant (no arithmetic instructions).
    fn emit_ctfe_value(&mut self, val: &CtfeValue) -> Option<Value> {
        match val {
            CtfeValue::Int(n) => Some(Value::scalar(LlvmTy::I64, n.to_string())),
            CtfeValue::Bool(b) => Some(Value::scalar(
                LlvmTy::I1,
                if *b { "1".into() } else { "0".into() },
            )),
            CtfeValue::Str(s) => {
                let idx = if let Some(i) = self.strings.iter().position(|x| x == s) {
                    i
                } else {
                    let i = self.strings.len();
                    self.strings.push(s.clone());
                    let escaped = escape_llvm_string(s);
                    let len = s.len() + 1;
                    writeln!(
                        self.late_globals,
                        "@.str.{i} = private unnamed_addr constant [{len} x i8] c\"{escaped}\\00\", align 1"
                    )
                    .unwrap();
                    i
                };
                let len = s.len() + 1;
                let t = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = getelementptr inbounds [{len} x i8], ptr @.str.{idx}, i64 0, i64 0"
                )
                .unwrap();
                Some(Value::scalar(LlvmTy::I8Ptr, t))
            }
            CtfeValue::Float(n) => Some(Value::scalar(LlvmTy::F64, format_float(*n))),
            _ => None,
        }
    }

    fn try_etca_fold(&mut self, expr: &Expr) -> Option<Value> {
        // Inside a residual *runtime* loop, never fold exprs that read locals —
        // that freezes loop-carried state into IR. Closed exprs (literals / const
        // calls) remain foldable — CTFE scope stays wide.
        if !self.loop_stack.is_empty() && expr_reads_ident(expr) {
            return None;
        }
        let val = {
            let etca = self.etca.as_mut()?;
            etca.try_eval(expr)?
        };
        let out = self.emit_ctfe_value(&val)?;
        if let Some(etca) = self.etca.as_mut() {
            etca.llvm_folds += 1;
        }
        Some(out)
    }

    /// Apply CTFE residual bindings: update engines + store constants into locals.
    fn apply_ctfe_residuals(&mut self, residuals: Vec<(String, CtfeValue)>) -> Result<(), Diagnostic> {
        for (name, val) in residuals {
            if let Some(etca) = self.etca.as_mut() {
                etca.bind_const(&name, &val);
            }
            let Some((lty, ptr, _)) = self.locals.get(&name).cloned() else {
                continue;
            };
            if lty.is_struct() || lty.is_array() {
                continue;
            }
            let Some(v) = self.emit_ctfe_value(&val) else {
                continue;
            };
            let casted = self.coerce(v, lty.clone(), Span { start: 0, end: 0 })?;
            writeln!(
                self.out,
                "  store {} {}, ptr {ptr}, align 8",
                lty.ir_type(),
                casted.repr
            )
            .unwrap();
        }
        Ok(())
    }

    fn emit_program(&mut self, program: &Program) -> Result<(), Diagnostic> {
        writeln!(self.out, "; ModuleID = 'slime'").unwrap();
        writeln!(self.out, "source_filename = \"slime\"").unwrap();
        writeln!(self.out, "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"").unwrap();
        writeln!(self.out, "target triple = \"x86_64-pc-windows-msvc\"").unwrap();
        writeln!(self.out).unwrap();

        for link in &program.links {
            writeln!(self.out, "; !link {link}").unwrap();
        }

        for func in &program.functions {
            self.collect_strings_fn(func);
        }

        for sd in &program.structs {
            let has_v = self.struct_has_virtual(&sd.name);
            let mut field_tys = String::new();
            if has_v {
                field_tys.push_str("ptr");
            }
            for (i, field) in sd.fields.iter().enumerate() {
                if has_v || i > 0 {
                    field_tys.push_str(", ");
                }
                let lty = self.resolve_type(&field.ty, field.name_span)?;
                field_tys.push_str(&lty.ir_type());
            }
            writeln!(self.out, "%{} = type {{ {field_tys} }}", sd.name).unwrap();

            if has_v {
                let vmethods = self.virtual_methods(&sd.name);
                let mut vtable_tys = vmethods
                    .iter()
                    .map(|_| "ptr".to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut vtable_vals = vmethods
                    .iter()
                    .map(|m| format!("ptr @{m}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    self.out,
                    "@{0}_vtable = global {{ {1} }} {{ {2} }}",
                    sd.name, vtable_tys, vtable_vals
                )
                .unwrap();
            }
        }
        if !program.structs.is_empty() {
            writeln!(self.out).unwrap();
        }

        for (i, s) in self.strings.iter().enumerate() {
            let escaped = escape_llvm_string(s);
            let len = s.len() + 1;
            writeln!(
                self.out,
                "@.str.{i} = private unnamed_addr constant [{len} x i8] c\"{escaped}\\00\", align 1"
            )
            .unwrap();
        }
        writeln!(self.out).unwrap();

        writeln!(self.out, "declare i32 @puts(ptr noundef)").unwrap();
        writeln!(self.out, "declare i32 @printf(ptr noundef, ...)").unwrap();
        writeln!(self.out, "declare double @sin(double)").unwrap();
        writeln!(self.out, "declare double @cos(double)").unwrap();
        writeln!(self.out, "declare double @slime_mono_now()").unwrap();
        writeln!(self.out, "declare ptr @slime_md5_hex(ptr noundef)").unwrap();
        writeln!(self.out, "declare ptr @slime_dict_new()").unwrap();
        writeln!(
            self.out,
            "declare void @slime_dict_put(ptr noundef, ptr noundef, double)"
        )
        .unwrap();
        writeln!(self.out, "declare void @slime_dict_del_prefix1(ptr noundef)").unwrap();
        writeln!(self.out, "declare double @slime_dict_sum(ptr noundef)").unwrap();
        writeln!(self.out, "declare void @slime_dict_free(ptr noundef)").unwrap();
        writeln!(self.out, "declare ptr @slime_itoa(i64)").unwrap();
        writeln!(self.out, "declare void @slime_print_f6(double)").unwrap();
        writeln!(self.out, "declare ptr @slime_strcat(ptr noundef, ptr noundef)").unwrap();
        writeln!(self.out, "declare ptr @slime_strdup(ptr noundef)").unwrap();
        writeln!(self.out, "declare void @slime_str_free(ptr noundef)").unwrap();
        writeln!(self.out, "declare ptr @slime_alloc_f64(i64)").unwrap();
        writeln!(self.out, "declare ptr @slime_alloc_i64(i64)").unwrap();
        writeln!(self.out, "declare double @sqrt(double)").unwrap();
        writeln!(self.out, "declare double @pow(double, double)").unwrap();
        writeln!(self.out, "declare double @floor(double)").unwrap();
        writeln!(self.out, "declare double @ceil(double)").unwrap();
        writeln!(self.out, "declare double @round(double)").unwrap();
        writeln!(self.out, "declare double @llvm.fabs.f64(double)").unwrap();
        writeln!(self.out, "declare double @llvm.minnum.f64(double, double)").unwrap();
        writeln!(self.out, "declare double @llvm.maxnum.f64(double, double)").unwrap();
        writeln!(self.out, "declare i64 @labs(i64)").unwrap();
        writeln!(self.out, "declare i64 @strlen(ptr noundef)").unwrap();
        writeln!(self.out, "declare i32 @strcmp(ptr noundef, ptr noundef)").unwrap();
        writeln!(self.out, "declare i64 @atoll(ptr noundef)").unwrap();
        writeln!(self.out, "declare double @atof(ptr noundef)").unwrap();
        writeln!(
            self.out,
            "declare ptr @slime_substr(ptr noundef, i64, i64)"
        )
        .unwrap();
        writeln!(self.out).unwrap();
        writeln!(
            self.out,
            "@.fmt.int = private unnamed_addr constant [6 x i8] c\"%lld\\0A\\00\", align 1"
        )
        .unwrap();
        writeln!(self.out).unwrap();

        for func in &program.functions {
            self.emit_function(func)?;
        }
        Ok(())
    }

    fn collect_strings_fn(&mut self, func: &Function) {
        for stmt in &func.body {
            self.collect_strings_stmt(stmt);
        }
    }

    fn collect_strings_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::VarDecl { init, .. } => self.collect_strings_expr(init),
            StmtKind::ArrayDecl { .. } => {}
            StmtKind::IndexAssign { index, value, .. } => {
                self.collect_strings_expr(index);
                self.collect_strings_expr(value);
            }
            StmtKind::Assign { value, .. } => self.collect_strings_expr(value),
            StmtKind::DerefAssign { ptr, value } => {
                self.collect_strings_expr(ptr);
                self.collect_strings_expr(value);
            }
            StmtKind::FieldAssign { value, .. } => self.collect_strings_expr(value),
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                self.collect_strings_expr(cond);
                for s in then_body {
                    self.collect_strings_stmt(s);
                }
                for s in else_body {
                    self.collect_strings_stmt(s);
                }
            }
            StmtKind::While { cond, body } => {
                self.collect_strings_expr(cond);
                for s in body {
                    self.collect_strings_stmt(s);
                }
            }
            StmtKind::For {
                start, end, body, ..
            } => {
                self.collect_strings_expr(start);
                self.collect_strings_expr(end);
                for s in body {
                    self.collect_strings_stmt(s);
                }
            }
            StmtKind::Print(args) => {
                for a in args {
                    self.collect_strings_expr(a);
                }
            }
            StmtKind::Break | StmtKind::Continue | StmtKind::Return(None) => {}
            StmtKind::Return(Some(e)) => self.collect_strings_expr(e),
            StmtKind::Expr(e) => self.collect_strings_expr(e),
        }
    }

    fn collect_strings_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::StrLit(s) => {
                if !self.strings.contains(s) {
                    self.strings.push(s.clone());
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.collect_strings_expr(left);
                self.collect_strings_expr(right);
            }
            ExprKind::Call { args, .. } => {
                for a in args {
                    self.collect_strings_expr(a);
                }
            }
            ExprKind::Unary { expr, .. } => self.collect_strings_expr(expr),
            ExprKind::MethodCall { receiver, args, .. } => {
                self.collect_strings_expr(receiver);
                for a in args {
                    self.collect_strings_expr(a);
                }
            }
            ExprKind::StructLit { fields, .. } => {
                for (_, _, e) in fields {
                    self.collect_strings_expr(e);
                }
            }
            ExprKind::Field { base, .. } => self.collect_strings_expr(base),
            ExprKind::Index { base, index } => {
                self.collect_strings_expr(base);
                self.collect_strings_expr(index);
            }
            _ => {}
        }
    }

    fn str_index(&self, s: &str) -> usize {
        self.strings
            .iter()
            .position(|x| x == s)
            .expect("string collected")
    }

    fn tmp(&mut self) -> String {
        let n = self.next_tmp;
        self.next_tmp += 1;
        format!("%t{n}")
    }

    fn label(&mut self, prefix: &str) -> String {
        let n = self.next_label;
        self.next_label += 1;
        format!("{prefix}{n}")
    }

    fn emit_function(&mut self, func: &Function) -> Result<(), Diagnostic> {
        self.locals.clear();
        self.str_locals.clear();
        self.ref_locals.clear();
        self.next_tmp = 0;
        self.next_label = 0;
        self.terminated = false;

        self.is_main = func.name == "main";
        self.current_ret = if self.is_main {
            None
        } else {
            func.ret_ty.clone()
        };

        if self.is_main {
            writeln!(self.out, "define dso_local i32 @main() {{").unwrap();
        } else {
            let ret_ir = match &func.ret_ty {
                None | Some(Type::Void) => "void".to_string(),
                Some(ty) => self.resolve_type(ty, func.span)?.ir_type(),
            };
            let mut params_ir = String::new();
            for (i, param) in func.params.iter().enumerate() {
                if i > 0 {
                    params_ir.push_str(", ");
                }
                let lty = match param.mode {
                    ParamMode::Value
                    | ParamMode::Own
                    | ParamMode::Share
                    | ParamMode::Joint => self.resolve_type(&param.ty, param.name_span)?,
                    ParamMode::Ref
                    | ParamMode::Out
                    | ParamMode::Borrow
                    | ParamMode::BorrowMut => LlvmTy::I8Ptr,
                };
                write!(params_ir, "{} %{}", lty.ir_type(), param.name).unwrap();
            }
            writeln!(
                self.out,
                "define dso_local {} @{}({}) {{",
                ret_ir,
                mangle(&func.name),
                params_ir
            )
            .unwrap();
        }
        writeln!(self.out, "entry:").unwrap();

        for param in &func.params {
            match param.mode {
                ParamMode::Value
                | ParamMode::Own
                | ParamMode::Share
                | ParamMode::Joint => {
                    let lty = self.resolve_type(&param.ty, param.name_span)?;
                    let ptr = format!("%{}.addr", param.name);
                    writeln!(
                        self.out,
                        "  {ptr} = alloca {}, align 8",
                        lty.ir_type()
                    )
                    .unwrap();
                    if !self.is_main {
                        writeln!(
                            self.out,
                            "  store {} %{}, ptr {ptr}, align 8",
                            lty.ir_type(),
                            param.name
                        )
                        .unwrap();
                    }
                    self.locals
                        .insert(param.name.clone(), (lty, ptr, param.name_span));
                }
                ParamMode::Ref
                | ParamMode::Out
                | ParamMode::Borrow
                | ParamMode::BorrowMut => {
                    let ptr = format!("%{}.addr", param.name);
                    writeln!(self.out, "  {ptr} = alloca ptr, align 8").unwrap();
                    if !self.is_main {
                        writeln!(
                            self.out,
                            "  store ptr %{}, ptr {ptr}, align 8",
                            param.name
                        )
                        .unwrap();
                    }
                    self.ref_locals
                        .insert(param.name.clone(), param.ty.clone());
                    self.locals.insert(
                        param.name.clone(),
                        (LlvmTy::I8Ptr, ptr, param.name_span),
                    );
                }
            }
        }

        // Hoist all local allocas into the entry block (LLVM alloca-in-loop = stack blowup).
        let mut decls = Vec::new();
        Self::collect_var_decls(&func.body, &mut decls);
        for (name, ty, init, span) in &decls {
            if self.locals.contains_key(name) {
                continue;
            }
            let lty = self.guess_local_ty(ty, init, *span)?;
            let ptr = format!("%{name}.addr");
            writeln!(self.out, "  {ptr} = alloca {}, align 8", lty.ir_type()).unwrap();
            if matches!(lty, LlvmTy::I8Ptr) {
                writeln!(self.out, "  store ptr null, ptr {ptr}, align 8").unwrap();
            }
            self.locals
                .insert(name.clone(), (lty, ptr, *span));
        }

        for stmt in &func.body {
            if self.terminated {
                break;
            }
            self.emit_stmt(stmt)?;
        }

        if !self.terminated {
            if self.is_main {
                writeln!(self.out, "  ret i32 0").unwrap();
            } else if func.ret_ty.as_ref().is_none_or(|t| t.is_void()) {
                writeln!(self.out, "  ret void").unwrap();
            } else {
                return Err(Diagnostic::error("missing return statement")
                    .code("E0308")
                    .label(func.span, "non-void function must return a value"));
            }
        }
        writeln!(self.out, "}}\n").unwrap();
        Ok(())
    }

    fn array_local(&self, name: &str, span: Span) -> Result<(String, LlvmTy, i64), Diagnostic> {
        let (lty, ptr, _) = self.locals.get(name).cloned().ok_or_else(|| {
            Diagnostic::error(format!("cannot find value `{name}` in this scope"))
                .code("E0425")
                .label(span, "not found in this scope")
        })?;
        let Some((elem_lty, len)) = lty.array_elem_len() else {
            return Err(Diagnostic::error(format!(
                "indexing requires an array, found `{}`",
                lty.ir_type()
            ))
            .code("E0608")
            .label(span, "cannot index non-array value"));
        };
        Ok((ptr, elem_lty, len))
    }

    fn ensure_local_alloca(&mut self, name: &str, lty: &LlvmTy, span: Span) -> String {
        if let Some((_, ptr, _)) = self.locals.get(name) {
            return ptr.clone();
        }
        // Fallback for locals not seen by the entry prepass.
        let ptr = format!("%{name}.addr");
        writeln!(
            self.out,
            "  {ptr} = alloca {}, align 8",
            lty.ir_type()
        )
        .unwrap();
        if matches!(lty, LlvmTy::I8Ptr) {
            writeln!(self.out, "  store ptr null, ptr {ptr}, align 8").unwrap();
        }
        self.locals
            .insert(name.to_string(), (lty.clone(), ptr.clone(), span));
        ptr
    }

    fn free_str_if_owned(&mut self, v: &Value) {
        if v.owned && v.ty == LlvmTy::I8Ptr {
            writeln!(
                self.out,
                "  call void @slime_str_free(ptr noundef {})",
                v.repr
            )
            .unwrap();
        }
    }

    /// Ensure a string value is heap-owned (strdup borrows).
    fn take_str_owned(&mut self, v: Value) -> Value {
        if v.ty != LlvmTy::I8Ptr {
            return v;
        }
        if v.owned {
            return v;
        }
        let t = self.tmp();
        writeln!(
            self.out,
            "  {t} = call ptr @slime_strdup(ptr noundef {})",
            v.repr
        )
        .unwrap();
        Value::owned_str(t)
    }

    fn store_local_scalar(
        &mut self,
        name: &str,
        ptr: &str,
        lty: &LlvmTy,
        v: Value,
        free_old_str: bool,
    ) -> Result<(), Diagnostic> {
        let manage_str = self.str_locals.contains(name) || v.owned;
        if *lty == LlvmTy::I8Ptr && manage_str {
            self.str_locals.insert(name.to_string());
            if free_old_str {
                let old = self.tmp();
                writeln!(self.out, "  {old} = load ptr, ptr {ptr}, align 8").unwrap();
                writeln!(
                    self.out,
                    "  call void @slime_str_free(ptr noundef {old})"
                )
                .unwrap();
            }
            let owned = self.take_str_owned(v);
            writeln!(
                self.out,
                "  store ptr {}, ptr {ptr}, align 8",
                owned.repr
            )
            .unwrap();
            Ok(())
        } else {
            writeln!(
                self.out,
                "  store {} {}, ptr {ptr}, align 8",
                lty.ir_type(),
                v.repr
            )
            .unwrap();
            Ok(())
        }
    }

    fn emit_str_concat(&mut self, lv: Value, rv: Value) -> Value {
        let t = self.tmp();
        writeln!(
            self.out,
            "  {t} = call ptr @slime_strcat(ptr noundef {}, ptr noundef {})",
            lv.repr, rv.repr
        )
        .unwrap();
        self.free_str_if_owned(&lv);
        self.free_str_if_owned(&rv);
        Value::owned_str(t)
    }

    fn guess_local_ty(&self, ty: &Type, init: &Expr, span: Span) -> Result<LlvmTy, Diagnostic> {
        if !matches!(ty, Type::Infer) {
            return self.resolve_type(ty, span);
        }
        Ok(match &init.kind {
            ExprKind::IntLit(_) | ExprKind::BoolLit(_) => LlvmTy::I64,
            ExprKind::FloatLit(_) => LlvmTy::F64,
            ExprKind::StrLit(_) => LlvmTy::I8Ptr,
            ExprKind::Call { callee, .. } => match callee.as_str() {
                "dict_new" | "itoa" | "md5" | "slime_strcat" => LlvmTy::I8Ptr,
                "sin" | "cos" | "mono_now" | "float" | "dict_sum" => LlvmTy::F64,
                _ => LlvmTy::I64,
            },
            ExprKind::Binary { op, left, right } => {
                if matches!(
                    op,
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
                        | BinOp::And | BinOp::Or
                ) {
                    LlvmTy::I1
                } else if matches!(&left.kind, ExprKind::FloatLit(_))
                    || matches!(&right.kind, ExprKind::FloatLit(_))
                {
                    LlvmTy::F64
                } else {
                    // float(x) * lit etc. — prefer F64 if either side looks float
                    let l = self.guess_local_ty(&Type::Infer, left, span)?;
                    let r = self.guess_local_ty(&Type::Infer, right, span)?;
                    if l == LlvmTy::F64 || r == LlvmTy::F64 {
                        LlvmTy::F64
                    } else if l == LlvmTy::I8Ptr || r == LlvmTy::I8Ptr {
                        LlvmTy::I8Ptr
                    } else {
                        LlvmTy::I64
                    }
                }
            }
            ExprKind::Ident(n) => self
                .locals
                .get(n)
                .map(|(t, _, _)| t.clone())
                .unwrap_or(LlvmTy::I64),
            _ => LlvmTy::I64,
        })
    }

    fn collect_var_decls(stmts: &[Stmt], out: &mut Vec<(String, Type, Expr, Span)>) {
        for stmt in stmts {
            match &stmt.kind {
                StmtKind::VarDecl {
                    name,
                    name_span,
                    ty,
                    init,
                    ..
                } => out.push((name.clone(), ty.clone(), init.clone(), *name_span)),
                StmtKind::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    Self::collect_var_decls(then_body, out);
                    Self::collect_var_decls(else_body, out);
                }
                StmtKind::While { body, .. } | StmtKind::For { body, .. } => {
                    Self::collect_var_decls(body, out);
                }
                _ => {}
            }
        }
    }

    fn emit_stmt(&mut self, stmt: &Stmt) -> Result<(), Diagnostic> {
        if self.terminated {
            return Ok(());
        }
        match &stmt.kind {
            StmtKind::VarDecl {
                name,
                name_span,
                ty,
                init,
                ..
            } => {
                if matches!(ty, Type::Infer) {
                    let v = self.emit_expr(init)?;
                    let lty = v.ty.clone();
                    if matches!(lty, LlvmTy::Void) {
                        return Err(Diagnostic::error("cannot infer type of void value")
                            .code("E0282")
                            .label(*name_span, "type annotations needed"));
                    }
                    let ptr = self.ensure_local_alloca(name, &lty, *name_span);
                    if lty.is_struct() {
                        self.store_struct_value(&ptr, &lty, v, init.span)?;
                    } else {
                        if let Some(etca) = self.etca.as_mut() {
                            if let Some(cv) = etca.try_eval(init) {
                                etca.bind_const(name, &cv);
                            }
                            etca.tick();
                        }
                        if matches!(lty, LlvmTy::I8Ptr)
                            && (v.owned || matches!(init.kind, ExprKind::StrLit(_)))
                        {
                            self.str_locals.insert(name.clone());
                        }
                        self.store_local_scalar(name, &ptr, &lty, v, false)?;
                    }
                } else {
                    let lty = self.resolve_type(ty, *name_span)?;
                    let ptr = self.ensure_local_alloca(name, &lty, *name_span);
                    if matches!(ty, Type::Str) {
                        self.str_locals.insert(name.clone());
                    }

                    if lty.is_struct() {
                        if let ExprKind::StructLit {
                            name: lit_name,
                            fields,
                            ..
                        } = &init.kind
                        {
                            self.emit_struct_lit_into(lit_name, fields, &ptr, init.span)?;
                        } else {
                            let v = self.emit_expr(init)?;
                            self.store_struct_value(&ptr, &lty, v, init.span)?;
                        }
                    } else {
                        let v = self.emit_expr(init)?;
                        if let Some(etca) = self.etca.as_mut() {
                            if let Some(cv) = etca.try_eval(init) {
                                etca.bind_const(name, &cv);
                            }
                            etca.tick();
                        }
                        let casted = self.coerce(v, lty.clone(), init.span)?;
                        self.store_local_scalar(name, &ptr, &lty, casted, false)?;
                    }
                }
            }
            StmtKind::ArrayDecl {
                name,
                name_span,
                elem,
                len,
            } => {
                let elem_lty = self.resolve_type(elem, *name_span)?;
                let lty = LlvmTy::Array {
                    elem: Box::new(elem_lty.clone()),
                    len: *len,
                };
                let ptr = format!("%{name}.buf");
                match elem_lty {
                    LlvmTy::F64 => {
                        writeln!(
                            self.out,
                            "  {ptr} = call ptr @slime_alloc_f64(i64 {len})"
                        )
                        .unwrap();
                    }
                    LlvmTy::I64 => {
                        writeln!(
                            self.out,
                            "  {ptr} = call ptr @slime_alloc_i64(i64 {len})"
                        )
                        .unwrap();
                    }
                    _ => {
                        return Err(Diagnostic::error(format!(
                            "array element type `{}` is not supported",
                            elem_lty.ir_type()
                        ))
                        .code("E0308")
                        .label(*name_span, "only int and float arrays are supported"));
                    }
                }
                self.locals.insert(name.clone(), (lty, ptr, *name_span));
            }
            StmtKind::IndexAssign {
                base,
                base_span,
                index,
                value,
            } => {
                let (array_ptr, elem_lty, _) = self.array_local(base, *base_span)?;
                let idx_v = self.emit_expr(index)?;
                let idx_i64 = self.coerce(idx_v, LlvmTy::I64, index.span)?;
                let gep = self.tmp();
                writeln!(
                    self.out,
                    "  {gep} = getelementptr {}, ptr {array_ptr}, i64 {}",
                    elem_lty.ir_type(),
                    idx_i64.repr
                )
                .unwrap();
                let v = self.emit_expr(value)?;
                let casted = self.coerce(v, elem_lty.clone(), value.span)?;
                writeln!(
                    self.out,
                    "  store {} {}, ptr {gep}, align 8",
                    elem_lty.ir_type(),
                    casted.repr
                )
                .unwrap();
            }
            StmtKind::Assign {
                name,
                name_span,
                value,
            } => {
                if self.ref_locals.contains_key(name) {
                    let pointee = self.ref_locals.get(name).cloned().unwrap();
                    let pointee_lty = self.resolve_type(&pointee, *name_span)?;
                    let (lty, ptr, _) = self.locals.get(name).cloned().ok_or_else(|| {
                        Diagnostic::error(format!("cannot find value `{name}` in this scope"))
                            .code("E0425")
                            .label(*name_span, "not found in this scope")
                    })?;
                    let _ = lty;
                    let slot = self.tmp();
                    writeln!(self.out, "  {slot} = load ptr, ptr {ptr}, align 8").unwrap();
                    let v = self.emit_expr(value)?;
                    let casted = self.coerce(v, pointee_lty.clone(), value.span)?;
                    writeln!(
                        self.out,
                        "  store {} {}, ptr {slot}, align 8",
                        pointee_lty.ir_type(),
                        casted.repr
                    )
                    .unwrap();
                } else {
                let (lty, ptr, _) = self.locals.get(name).cloned().ok_or_else(|| {
                    Diagnostic::error(format!("cannot find value `{name}` in this scope"))
                        .code("E0425")
                        .label(*name_span, "not found in this scope")
                        .help(format!("declare it first, e.g. `int {name} = ...`"))
                })?;
                if lty.is_struct() {
                    let v = self.emit_expr(value)?;
                    self.store_struct_value(&ptr, &lty, v, value.span)?;
                } else {
                    let v = self.emit_expr(value)?;
                    if let Some(etca) = self.etca.as_mut() {
                        if self.loop_stack.is_empty() {
                            if let Some(cv) = etca.try_eval(value) {
                                etca.bind_const(name, &cv);
                            } else {
                                etca.invalidate_const(name);
                            }
                        } else {
                            etca.invalidate_const(name);
                        }
                        etca.tick();
                    }
                    let casted = self.coerce(v, lty.clone(), value.span)?;
                    self.store_local_scalar(name, &ptr, &lty, casted, true)?;
                }
                }
            }
            StmtKind::DerefAssign { ptr, value } => {
                let pv = self.emit_expr(ptr)?;
                let pointee_lty = if pv.ty == LlvmTy::I8Ptr {
                    LlvmTy::I64
                } else {
                    pv.ty.clone()
                };
                let slot = if pv.ty == LlvmTy::I8Ptr {
                    pv.repr
                } else {
                    let t = self.tmp();
                    writeln!(self.out, "  {t} = load ptr, ptr {}, align 8", pv.repr).unwrap();
                    t
                };
                let v = self.emit_expr(value)?;
                let casted = self.coerce(v, pointee_lty.clone(), value.span)?;
                writeln!(
                    self.out,
                    "  store {} {}, ptr {slot}, align 8",
                    pointee_lty.ir_type(),
                    casted.repr
                )
                .unwrap();
            }
            StmtKind::FieldAssign {
                base,
                base_span,
                field,
                field_span,
                value,
            } => {
                let (struct_name, struct_ptr) = if let Some(pointee) = self.ref_locals.get(base) {
                    let pointee_lty = self.resolve_type(pointee, *base_span)?;
                    let (_, ptr, _) = self.locals.get(base).cloned().ok_or_else(|| {
                        Diagnostic::error(format!("cannot find value `{base}` in this scope"))
                            .code("E0425")
                            .label(*base_span, "not found in this scope")
                    })?;
                    let LlvmTy::Struct(struct_name) = pointee_lty else {
                        return Err(Diagnostic::error(format!(
                            "field assignment requires a struct, found `{}`",
                            pointee_lty.ir_type()
                        ))
                        .code("E0609")
                        .label(*field_span, "field assignment on non-struct"));
                    };
                    let slot = self.tmp();
                    writeln!(self.out, "  {slot} = load ptr, ptr {ptr}, align 8").unwrap();
                    (struct_name, slot)
                } else {
                let (lty, ptr, _) = self.locals.get(base).cloned().ok_or_else(|| {
                    Diagnostic::error(format!("cannot find value `{base}` in this scope"))
                        .code("E0425")
                        .label(*base_span, "not found in this scope")
                })?;
                let LlvmTy::Struct(struct_name) = &lty else {
                    return Err(Diagnostic::error(format!(
                        "field assignment requires a struct, found `{}`",
                        lty.ir_type()
                    ))
                    .code("E0609")
                    .label(*field_span, "field assignment on non-struct"));
                };
                (struct_name.clone(), ptr)
                };
                let (idx, field_ty) =
                    self.struct_field_index(&struct_name, field, *field_span)?;
                let field_lty = self.resolve_type(&field_ty, *field_span)?;
                let gep = self.tmp();
                writeln!(
                    self.out,
                    "  {gep} = getelementptr inbounds %{struct_name}, ptr {struct_ptr}, i32 0, i32 {idx}"
                )
                .unwrap();
                let v = self.emit_expr(value)?;
                let casted = self.coerce(v, field_lty.clone(), value.span)?;
                writeln!(
                    self.out,
                    "  store {} {}, ptr {gep}, align 8",
                    field_lty.ir_type(),
                    casted.repr
                )
                .unwrap();
            }
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                // Const if-fold only outside loops (same freeze hazard as while).
                if self.loop_stack.is_empty() {
                    if let Some(CtfeValue::Bool(b)) =
                        self.etca.as_mut().and_then(|e| e.try_eval(cond))
                    {
                        if let Some(etca) = self.etca.as_mut() {
                            etca.llvm_folds += 1;
                            etca.note_const_control_fold(b, &[i64::from(b)]);
                            etca.tick();
                        }
                        let body = if b { then_body } else { else_body };
                        for s in body {
                            self.emit_stmt(s)?;
                            if self.terminated {
                                break;
                            }
                        }
                        return Ok(());
                    }
                }

                let c = self.emit_expr(cond)?;
                let c1 = self.to_i1(c)?;
                let then_l = self.label("if.then");
                let else_l = self.label("if.else");
                let end_l = self.label("if.end");
                writeln!(
                    self.out,
                    "  br i1 {}, label %{then_l}, label %{else_l}",
                    c1.repr
                )
                .unwrap();

                writeln!(self.out, "{then_l}:").unwrap();
                self.terminated = false;
                for s in then_body {
                    self.emit_stmt(s)?;
                    if self.terminated {
                        break;
                    }
                }
                let then_term = self.terminated;
                if !then_term {
                    writeln!(self.out, "  br label %{end_l}").unwrap();
                }

                writeln!(self.out, "{else_l}:").unwrap();
                self.terminated = false;
                for s in else_body {
                    self.emit_stmt(s)?;
                    if self.terminated {
                        break;
                    }
                }
                let else_term = self.terminated;
                if !else_term {
                    writeln!(self.out, "  br label %{end_l}").unwrap();
                }

                if then_term && else_term {
                    self.terminated = true;
                } else {
                    writeln!(self.out, "{end_l}:").unwrap();
                    self.terminated = false;
                }
            }
            StmtKind::While { cond, body } => {
                // Broad CTFE: run the whole loop in the compile-time VM when closed.
                if self.loop_stack.is_empty() {
                    if let Some(residuals) = self
                        .etca
                        .as_mut()
                        .and_then(|e| e.try_ctfe_while(cond, body))
                    {
                        return self.apply_ctfe_residuals(residuals);
                    }
                    // Dead loop (cond statically false) — still fold away.
                    if let Some(CtfeValue::Bool(false)) =
                        self.etca.as_mut().and_then(|e| e.try_eval(cond))
                    {
                        if let Some(etca) = self.etca.as_mut() {
                            etca.llvm_folds += 1;
                        }
                        return Ok(());
                    }
                }

                let cond_l = self.label("while.cond");
                let body_l = self.label("while.body");
                let end_l = self.label("while.end");
                self.loop_stack.push((cond_l.clone(), end_l.clone()));
                writeln!(self.out, "  br label %{cond_l}").unwrap();

                writeln!(self.out, "{cond_l}:").unwrap();
                self.terminated = false;
                let c = self.emit_expr(cond)?;
                let c1 = self.to_i1(c)?;
                writeln!(
                    self.out,
                    "  br i1 {}, label %{body_l}, label %{end_l}",
                    c1.repr
                )
                .unwrap();

                writeln!(self.out, "{body_l}:").unwrap();
                self.terminated = false;
                for s in body {
                    self.emit_stmt(s)?;
                    if self.terminated {
                        break;
                    }
                }
                if !self.terminated {
                    writeln!(self.out, "  br label %{cond_l}").unwrap();
                }

                self.loop_stack.pop();
                writeln!(self.out, "{end_l}:").unwrap();
                self.terminated = false;
            }
            StmtKind::For {
                name,
                name_span,
                start,
                end,
                body,
            } => {
                if self.loop_stack.is_empty() {
                    if let Some(residuals) = self.etca.as_mut().and_then(|e| {
                        e.try_ctfe_for(name, start, end, body)
                    }) {
                        // Ensure loop index local exists for residual store.
                        if !self.locals.contains_key(name) {
                            let ptr = self.ensure_local_alloca(name, &LlvmTy::I64, *name_span);
                            let _ = ptr;
                        }
                        return self.apply_ctfe_residuals(residuals);
                    }
                }

                let start_v = self.emit_expr(start)?;
                let start_i = self.coerce(start_v, LlvmTy::I64, start.span)?;
                let end_v = self.emit_expr(end)?;
                let end_i = self.coerce(end_v, LlvmTy::I64, end.span)?;
                let end_tmp = self.tmp();
                writeln!(self.out, "  {end_tmp} = add i64 {}, 0", end_i.repr).unwrap();

                let ptr = format!("%{name}.addr");
                if !self.locals.contains_key(name) {
                    writeln!(self.out, "  {ptr} = alloca i64, align 8").unwrap();
                    self.locals
                        .insert(name.clone(), (LlvmTy::I64, ptr.clone(), *name_span));
                }
                let (_, ptr, _) = self.locals.get(name).cloned().unwrap();
                writeln!(
                    self.out,
                    "  store i64 {}, ptr {ptr}, align 8",
                    start_i.repr
                )
                .unwrap();

                let cond_l = self.label("for.cond");
                let body_l = self.label("for.body");
                let step_l = self.label("for.step");
                let end_l = self.label("for.end");
                self.loop_stack.push((step_l.clone(), end_l.clone()));
                writeln!(self.out, "  br label %{cond_l}").unwrap();

                writeln!(self.out, "{cond_l}:").unwrap();
                self.terminated = false;
                let cur = self.tmp();
                writeln!(self.out, "  {cur} = load i64, ptr {ptr}, align 8").unwrap();
                let cmp = self.tmp();
                writeln!(
                    self.out,
                    "  {cmp} = icmp slt i64 {cur}, {end_tmp}"
                )
                .unwrap();
                writeln!(
                    self.out,
                    "  br i1 {cmp}, label %{body_l}, label %{end_l}"
                )
                .unwrap();

                writeln!(self.out, "{body_l}:").unwrap();
                self.terminated = false;
                for s in body {
                    self.emit_stmt(s)?;
                    if self.terminated {
                        break;
                    }
                }
                if !self.terminated {
                    writeln!(self.out, "  br label %{step_l}").unwrap();
                }

                writeln!(self.out, "{step_l}:").unwrap();
                self.terminated = false;
                let cur2 = self.tmp();
                writeln!(self.out, "  {cur2} = load i64, ptr {ptr}, align 8").unwrap();
                let next = self.tmp();
                writeln!(self.out, "  {next} = add i64 {cur2}, 1").unwrap();
                writeln!(self.out, "  store i64 {next}, ptr {ptr}, align 8").unwrap();
                writeln!(self.out, "  br label %{cond_l}").unwrap();

                self.loop_stack.pop();
                writeln!(self.out, "{end_l}:").unwrap();
                self.terminated = false;
            }
            StmtKind::Break => {
                let Some((_, break_l)) = self.loop_stack.last() else {
                    return Err(Diagnostic::error("`break` outside of loop")
                        .code("E0267")
                        .label(stmt.span, "cannot break here"));
                };
                let break_l = break_l.clone();
                writeln!(self.out, "  br label %{break_l}").unwrap();
                self.terminated = true;
            }
            StmtKind::Continue => {
                let Some((cont_l, _)) = self.loop_stack.last() else {
                    return Err(Diagnostic::error("`continue` outside of loop")
                        .code("E0267")
                        .label(stmt.span, "cannot continue here"));
                };
                let cont_l = cont_l.clone();
                writeln!(self.out, "  br label %{cont_l}").unwrap();
                self.terminated = true;
            }
            StmtKind::Print(args) => {
                if args.is_empty() {
                    return Err(Diagnostic::error("`print` requires at least one argument")
                        .code("E0061")
                        .label(stmt.span, "missing arguments"));
                }
                for arg in args {
                    let _ = self.emit_call("print", stmt.span, std::slice::from_ref(arg), arg.span)?;
                }
            }
            StmtKind::Return(None) => {
                if self.is_main {
                    writeln!(self.out, "  ret i32 0").unwrap();
                } else if self
                    .current_ret
                    .as_ref()
                    .is_none_or(|t| t.is_void())
                {
                    writeln!(self.out, "  ret void").unwrap();
                } else {
                    return Err(Diagnostic::error("return value required")
                        .code("E0308")
                        .label(stmt.span, "expected a return value"));
                }
                self.terminated = true;
            }
            StmtKind::Return(Some(e)) => {
                if self.is_main {
                    let v = self.emit_expr(e)?;
                    let casted = self.coerce(v, LlvmTy::I64, e.span)?;
                    let t = self.tmp();
                    writeln!(self.out, "  {t} = trunc i64 {} to i32", casted.repr).unwrap();
                    writeln!(self.out, "  ret i32 {t}").unwrap();
                } else {
                    let ret_ty = self.current_ret.clone().ok_or_else(|| {
                        Diagnostic::error("return value in void function")
                            .code("E0308")
                            .label(e.span, "void function cannot return a value")
                    })?;
                    if ret_ty.is_void() {
                        return Err(Diagnostic::error("return value in void function")
                            .code("E0308")
                            .label(e.span, "void function cannot return a value"));
                    }
                    let target = self.resolve_type(&ret_ty, stmt.span)?;
                    let v = self.emit_expr(e)?;
                    let casted = self.coerce(v, target.clone(), e.span)?;
                    writeln!(
                        self.out,
                        "  ret {} {}",
                        target.ir_type(),
                        casted.repr
                    )
                    .unwrap();
                }
                self.terminated = true;
            }
            StmtKind::Expr(e) => {
                let _ = self.emit_expr(e)?;
            }
        }
        Ok(())
    }

    fn store_struct_value(
        &mut self,
        dst_ptr: &str,
        dst_ty: &LlvmTy,
        src: Value,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let LlvmTy::Struct(dst_name) = dst_ty else {
            unreachable!();
        };
        if src.is_struct_ptr {
            if src.ty != *dst_ty {
                return Err(Diagnostic::error("mismatched struct types")
                    .code("E0308")
                    .label(span, "struct type mismatch"));
            }
            let agg = self.tmp();
            writeln!(
                self.out,
                "  {agg} = load %{dst_name}, ptr {}, align 8",
                src.repr
            )
            .unwrap();
            writeln!(
                self.out,
                "  store %{dst_name} {agg}, ptr {dst_ptr}, align 8"
            )
            .unwrap();
        } else if matches!(&src.ty, LlvmTy::Struct(n) if n == dst_name) {
            // Loaded struct SSA (e.g. function return value)
            writeln!(
                self.out,
                "  store %{dst_name} {}, ptr {dst_ptr}, align 8",
                src.repr
            )
            .unwrap();
        } else {
            return Err(Diagnostic::error("expected struct value")
                .code("E0308")
                .label(span, "cannot assign non-struct to struct variable"));
        }
        Ok(())
    }

    fn emit_struct_lit_into(
        &mut self,
        struct_name: &str,
        fields: &[(String, Span, Expr)],
        dest_ptr: &str,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let struct_fields: Vec<StructField> = self
            .struct_defs
            .get(struct_name)
            .ok_or_else(|| {
                Diagnostic::error(format!("unknown struct `{struct_name}`"))
                    .code("E0412")
                    .label(span, "struct not defined")
            })?
            .fields
            .clone();
        let has_v = self.struct_has_virtual(struct_name);
        if has_v {
            let gep = self.tmp();
            writeln!(
                self.out,
                "  {gep} = getelementptr inbounds %{struct_name}, ptr {dest_ptr}, i32 0, i32 0"
            )
            .unwrap();
            let vt = self.tmp();
            writeln!(
                self.out,
                "  {vt} = getelementptr inbounds @{struct_name}_vtable, i32 0, i32 0"
            )
            .unwrap();
            writeln!(self.out, "  store ptr {vt}, ptr {gep}, align 8").unwrap();
        }
        for (i, sf) in struct_fields.iter().enumerate() {
            let init = fields.iter().find(|(n, _, _)| n == &sf.name).ok_or_else(|| {
                Diagnostic::error(format!(
                    "missing field `{}` in struct literal",
                    sf.name
                ))
                .code("E0063")
                .label(span, format!("missing `{struct_name}.{}`", sf.name))
            })?;
            let gep = self.tmp();
            let llvm_i = self.struct_field_llvm_index(struct_name, i);
            writeln!(
                self.out,
                "  {gep} = getelementptr inbounds %{struct_name}, ptr {dest_ptr}, i32 0, i32 {llvm_i}"
            )
            .unwrap();
            let field_lty = self.resolve_type(&sf.ty, sf.name_span)?;
            let v = self.emit_expr(&init.2)?;
            let casted = self.coerce(v, field_lty.clone(), init.2.span)?;
            writeln!(
                self.out,
                "  store {} {}, ptr {gep}, align 8",
                field_lty.ir_type(),
                casted.repr
            )
            .unwrap();
        }
        Ok(())
    }

    fn emit_expr(&mut self, expr: &Expr) -> Result<Value, Diagnostic> {
        if let Some(v) = self.try_etca_fold(expr) {
            return Ok(v);
        }
        if matches!(
            expr.kind,
            ExprKind::Binary { .. }
                | ExprKind::Call { .. }
                | ExprKind::Ident(_)
                | ExprKind::StructLit { .. }
                | ExprKind::Field { .. }
                | ExprKind::Index { .. }
        ) {
            if let Some(etca) = self.etca.as_mut() {
                etca.note_runtime("runtime expr");
            }
        }

        match &expr.kind {
            ExprKind::IntLit(n) => Ok(Value::scalar(LlvmTy::I64, n.to_string())),
            ExprKind::FloatLit(n) => {
                Ok(Value::scalar(LlvmTy::F64, format_float(*n)))
            }
            ExprKind::BoolLit(b) => Ok(Value::scalar(
                LlvmTy::I1,
                if *b { "1".into() } else { "0".into() },
            )),
            ExprKind::StrLit(s) => {
                let idx = self.str_index(s);
                let len = s.len() + 1;
                let t = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = getelementptr inbounds [{len} x i8], ptr @.str.{idx}, i64 0, i64 0"
                )
                .unwrap();
                Ok(Value::scalar(LlvmTy::I8Ptr, t))
            }
            ExprKind::Ident(name) => {
                if self.loop_stack.is_empty() {
                    if let Some(lit) = self
                        .etca
                        .as_ref()
                        .and_then(|e| e.tce.get_compile_const(name))
                        .map(|s| s.to_string())
                    {
                        if let Some((lty, _, _)) = self.locals.get(name) {
                            if !lty.is_struct() && !lty.is_array() && !self.ref_locals.contains_key(name)
                            {
                                return Ok(Value::scalar(lty.clone(), lit));
                            }
                        }
                    }
                }
                let (lty, ptr, _) = self.locals.get(name).cloned().ok_or_else(|| {
                    Diagnostic::error(format!("cannot find value `{name}` in this scope"))
                        .code("E0425")
                        .label(expr.span, "not found in this scope")
                })?;
                if let Some(pointee) = self.ref_locals.get(name) {
                    let pointee_lty = self.resolve_type(pointee, expr.span)?;
                    let slot = self.tmp();
                    writeln!(self.out, "  {slot} = load ptr, ptr {ptr}, align 8").unwrap();
                    if let LlvmTy::Struct(struct_name) = pointee_lty {
                        return Ok(Value::struct_ptr(struct_name, slot));
                    }
                    let t = self.tmp();
                    writeln!(
                        self.out,
                        "  {t} = load {}, ptr {slot}, align 8",
                        pointee_lty.ir_type()
                    )
                    .unwrap();
                    return Ok(Value::scalar(pointee_lty, t));
                }
                if lty.is_struct() {
                    return Ok(Value::struct_ptr(
                        match &lty {
                            LlvmTy::Struct(n) => n.clone(),
                            _ => unreachable!(),
                        },
                        ptr,
                    ));
                }
                if lty.is_array() {
                    return Err(Diagnostic::error("array value used without index")
                        .code("E0425")
                        .label(expr.span, "use `name[index]` to access array elements"));
                }
                let t = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = load {}, ptr {ptr}, align 8",
                    lty.ir_type()
                )
                .unwrap();
                Ok(Value::scalar(lty, t))
            }
            ExprKind::Unary { op, expr: inner } => match op {
                UnaryOp::AddrOf | UnaryOp::AddrOfMut => {
                    if let ExprKind::Ident(name) = &inner.kind {
                        let (_, ptr, _) = self.locals.get(name).cloned().ok_or_else(|| {
                            Diagnostic::error(format!("cannot find value `{name}` in this scope"))
                                .code("E0425")
                                .label(expr.span, "not found in this scope")
                        })?;
                        Ok(Value::scalar(LlvmTy::I8Ptr, ptr))
                    } else {
                        let v = self.emit_expr(inner)?;
                        if v.is_struct_ptr {
                            Ok(Value::scalar(LlvmTy::I8Ptr, v.repr))
                        } else {
                            Err(Diagnostic::error("address-of requires an lvalue")
                                .code("E0599")
                                .label(expr.span, "expected variable name"))
                        }
                    }
                }
                UnaryOp::Move => self.emit_expr(inner),
                UnaryOp::Deref => {
                    let v = self.emit_expr(inner)?;
                    let pointee_lty = if v.ty == LlvmTy::I8Ptr {
                        LlvmTy::I64
                    } else {
                        v.ty.clone()
                    };
                    let slot = if v.ty == LlvmTy::I8Ptr {
                        v.repr
                    } else {
                        let t = self.tmp();
                        writeln!(self.out, "  {t} = load ptr, ptr {}, align 8", v.repr).unwrap();
                        t
                    };
                    let t = self.tmp();
                    writeln!(
                        self.out,
                        "  {t} = load {}, ptr {slot}, align 8",
                        pointee_lty.ir_type()
                    )
                    .unwrap();
                    Ok(Value::scalar(pointee_lty, t))
                }
            },
            ExprKind::Binary { op, left, right } => self.emit_binary(*op, left, right, expr.span),
            ExprKind::Call {
                callee,
                callee_span,
                type_args: _,
                args,
            } => self.emit_call(callee, *callee_span, args, expr.span),
            ExprKind::MethodCall { method_span, .. } => Err(Diagnostic::error(
                "method call should be monomorphized before codegen",
            )
            .code("E0599")
            .label(*method_span, "MethodCall not lowered")),
            ExprKind::StructLit {
                name,
                name_span,
                type_args: _,
                fields,
            } => {
                let tmp = self.tmp();
                writeln!(
                    self.out,
                    "  {tmp} = alloca %{name}, align 8"
                )
                .unwrap();
                self.emit_struct_lit_into(name, fields, &tmp, *name_span)?;
                Ok(Value::struct_ptr(name.clone(), tmp))
            }
            ExprKind::Field {
                base,
                field,
                field_span,
            } => {
                let struct_ptr = if let ExprKind::Ident(base_name) = &base.kind {
                    if let Some(pointee) = self.ref_locals.get(base_name) {
                        let pointee_lty = self.resolve_type(pointee, base.span)?;
                        let (_, ptr, _) = self.locals.get(base_name).cloned().ok_or_else(|| {
                            Diagnostic::error(format!("cannot find value `{base_name}` in this scope"))
                                .code("E0425")
                                .label(base.span, "not found in this scope")
                        })?;
                        let LlvmTy::Struct(struct_name) = pointee_lty else {
                            return Err(Diagnostic::error(format!(
                                "field access requires a struct, found `{}`",
                                pointee_lty.ir_type()
                            ))
                            .code("E0609")
                            .label(*field_span, "field access on non-struct"));
                        };
                        let slot = self.tmp();
                        writeln!(self.out, "  {slot} = load ptr, ptr {ptr}, align 8").unwrap();
                        (struct_name, slot)
                    } else {
                    let (lty, ptr, _) = self.locals.get(base_name).cloned().ok_or_else(|| {
                        Diagnostic::error(format!("cannot find value `{base_name}` in this scope"))
                            .code("E0425")
                            .label(base.span, "not found in this scope")
                    })?;
                    let LlvmTy::Struct(struct_name) = lty else {
                        return Err(Diagnostic::error(format!(
                            "field access requires a struct, found `{}`",
                            lty.ir_type()
                        ))
                        .code("E0609")
                        .label(*field_span, "field access on non-struct"));
                    };
                    (struct_name, ptr)
                    }
                } else {
                    let base_v = self.emit_expr(base)?;
                    if !base_v.is_struct_ptr {
                        return Err(Diagnostic::error("field access requires struct pointer")
                            .code("E0609")
                            .label(*field_span, "invalid field access base"));
                    }
                    let LlvmTy::Struct(struct_name) = base_v.ty else {
                        unreachable!();
                    };
                    (struct_name, base_v.repr)
                };
                let (idx, field_ty) =
                    self.struct_field_index(&struct_ptr.0, field, *field_span)?;
                let field_lty = self.resolve_type(&field_ty, *field_span)?;
                let gep = self.tmp();
                writeln!(
                    self.out,
                    "  {gep} = getelementptr inbounds %{}, ptr {}, i32 0, i32 {idx}",
                    struct_ptr.0, struct_ptr.1
                )
                .unwrap();
                let t = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = load {}, ptr {gep}, align 8",
                    field_lty.ir_type()
                )
                .unwrap();
                Ok(Value::scalar(field_lty, t))
            }
            ExprKind::Index { base, index } => {
                let (array_ptr, elem_lty, _) = match &base.kind {
                    ExprKind::Ident(name) => self.array_local(name, base.span)?,
                    _ => {
                        return Err(Diagnostic::error("array index base must be a local variable")
                            .code("E0608")
                            .label(base.span, "expected array variable name"));
                    }
                };
                let idx_v = self.emit_expr(index)?;
                let idx_i64 = self.coerce(idx_v, LlvmTy::I64, index.span)?;
                let gep = self.tmp();
                writeln!(
                    self.out,
                    "  {gep} = getelementptr {}, ptr {array_ptr}, i64 {}",
                    elem_lty.ir_type(),
                    idx_i64.repr
                )
                .unwrap();
                let t = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = load {}, ptr {gep}, align 8",
                    elem_lty.ir_type()
                )
                .unwrap();
                Ok(Value::scalar(elem_lty, t))
            }
        }
    }

    fn emit_binary(
        &mut self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        match op {
            BinOp::And => {
                let rhs_l = self.label("and.rhs");
                let end_l = self.label("and.end");
                let lv = self.emit_expr(left)?;
                let l1 = self.to_i1(lv)?;
                let result = format!("%and.{}", self.next_tmp);
                self.next_tmp += 1;
                let slot = self.tmp();
                writeln!(self.out, "  {slot} = alloca i1, align 1").unwrap();
                writeln!(self.out, "  store i1 0, ptr {slot}").unwrap();
                writeln!(
                    self.out,
                    "  br i1 {}, label %{rhs_l}, label %{end_l}",
                    l1.repr
                )
                .unwrap();
                writeln!(self.out, "{rhs_l}:").unwrap();
                let rv = self.emit_expr(right)?;
                let r1 = self.to_i1(rv)?;
                writeln!(self.out, "  store i1 {}, ptr {slot}", r1.repr).unwrap();
                writeln!(self.out, "  br label %{end_l}").unwrap();
                writeln!(self.out, "{end_l}:").unwrap();
                writeln!(self.out, "  {result} = load i1, ptr {slot}").unwrap();
                Ok(Value::scalar(LlvmTy::I1, result))
            }
            BinOp::Or => {
                let rhs_l = self.label("or.rhs");
                let end_l = self.label("or.end");
                let lv = self.emit_expr(left)?;
                let l1 = self.to_i1(lv)?;
                let result = format!("%or.{}", self.next_tmp);
                self.next_tmp += 1;
                let slot = self.tmp();
                writeln!(self.out, "  {slot} = alloca i1, align 1").unwrap();
                writeln!(self.out, "  store i1 1, ptr {slot}").unwrap();
                writeln!(
                    self.out,
                    "  br i1 {}, label %{end_l}, label %{rhs_l}",
                    l1.repr
                )
                .unwrap();
                writeln!(self.out, "{rhs_l}:").unwrap();
                let rv = self.emit_expr(right)?;
                let r1 = self.to_i1(rv)?;
                writeln!(self.out, "  store i1 {}, ptr {slot}", r1.repr).unwrap();
                writeln!(self.out, "  br label %{end_l}").unwrap();
                writeln!(self.out, "{end_l}:").unwrap();
                writeln!(self.out, "  {result} = load i1, ptr {slot}").unwrap();
                Ok(Value::scalar(LlvmTy::I1, result))
            }
            _ => {
                let lv = self.emit_expr(left)?;
                let rv = self.emit_expr(right)?;
                if lv.ty.is_struct() || rv.ty.is_struct() {
                    return Err(Diagnostic::error(
                        "binary operation cannot be applied to struct types",
                    )
                    .code("E0369")
                    .label(span, "invalid operands for binary operator"));
                }
                match op {
                    BinOp::Add if lv.ty == LlvmTy::I8Ptr && rv.ty == LlvmTy::I8Ptr => {
                        Ok(self.emit_str_concat(lv, rv))
                    }
                    BinOp::Add if lv.ty == LlvmTy::I8Ptr && matches!(rv.ty, LlvmTy::I64 | LlvmTy::I1) => {
                        let ri = self.coerce(rv, LlvmTy::I64, right.span)?;
                        let itoa = self.tmp();
                        writeln!(self.out, "  {itoa} = call ptr @slime_itoa(i64 {})", ri.repr).unwrap();
                        Ok(self.emit_str_concat(lv, Value::owned_str(itoa)))
                    }
                    BinOp::Add if matches!(lv.ty, LlvmTy::I64 | LlvmTy::I1) && rv.ty == LlvmTy::I8Ptr => {
                        let li = self.coerce(lv, LlvmTy::I64, left.span)?;
                        let itoa = self.tmp();
                        writeln!(self.out, "  {itoa} = call ptr @slime_itoa(i64 {})", li.repr).unwrap();
                        Ok(self.emit_str_concat(Value::owned_str(itoa), rv))
                    }
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                        let use_float = lv.ty == LlvmTy::F64 || rv.ty == LlvmTy::F64;
                        if use_float {
                            let l = self.coerce(lv, LlvmTy::F64, left.span)?;
                            let r = self.coerce(rv, LlvmTy::F64, right.span)?;
                            let t = self.tmp();
                            let inst = match op {
                                BinOp::Add => "fadd",
                                BinOp::Sub => "fsub",
                                BinOp::Mul => "fmul",
                                BinOp::Div => "fdiv",
                                BinOp::Mod => "frem",
                                _ => unreachable!(),
                            };
                            writeln!(self.out, "  {t} = {inst} double {}, {}", l.repr, r.repr).unwrap();
                            Ok(Value::scalar(LlvmTy::F64, t))
                        } else {
                            let l = self.coerce(lv, LlvmTy::I64, left.span)?;
                            let r = self.coerce(rv, LlvmTy::I64, right.span)?;
                            let t = self.tmp();
                            let inst = match op {
                                BinOp::Add => "add",
                                BinOp::Sub => "sub",
                                BinOp::Mul => "mul",
                                BinOp::Div => "sdiv",
                                BinOp::Mod => "srem",
                                _ => unreachable!(),
                            };
                            writeln!(self.out, "  {t} = {inst} i64 {}, {}", l.repr, r.repr).unwrap();
                            Ok(Value::scalar(LlvmTy::I64, t))
                        }
                    }
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                        let l = self.coerce(lv, LlvmTy::I64, left.span)?;
                        let r = self.coerce(rv, LlvmTy::I64, right.span)?;
                        let t = self.tmp();
                        let inst = match op {
                            BinOp::BitAnd => "and",
                            BinOp::BitOr => "or",
                            BinOp::BitXor => "xor",
                            BinOp::Shl => "shl",
                            BinOp::Shr => "lshr",
                            _ => unreachable!(),
                        };
                        writeln!(self.out, "  {t} = {inst} i64 {}, {}", l.repr, r.repr).unwrap();
                        Ok(Value::scalar(LlvmTy::I64, t))
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        if lv.ty == LlvmTy::I8Ptr || rv.ty == LlvmTy::I8Ptr {
                            return Err(Diagnostic::error(
                                "binary operation cannot be applied to type `str`",
                            )
                            .code("E0369")
                            .label(span, "cannot compare strings with relational operators"));
                        }
                        let use_float = lv.ty == LlvmTy::F64 || rv.ty == LlvmTy::F64;
                        if use_float {
                            let l = self.coerce(lv, LlvmTy::F64, left.span)?;
                            let r = self.coerce(rv, LlvmTy::F64, right.span)?;
                            let pred = match op {
                                BinOp::Eq => "oeq",
                                BinOp::Ne => "one",
                                BinOp::Lt => "olt",
                                BinOp::Le => "ole",
                                BinOp::Gt => "ogt",
                                BinOp::Ge => "oge",
                                _ => unreachable!(),
                            };
                            let t = self.tmp();
                            writeln!(
                                self.out,
                                "  {t} = fcmp {pred} double {}, {}",
                                l.repr, r.repr
                            )
                            .unwrap();
                            Ok(Value::scalar(LlvmTy::I1, t))
                        } else {
                            let l = self.coerce(lv, LlvmTy::I64, left.span)?;
                            let r = self.coerce(rv, LlvmTy::I64, right.span)?;
                            let pred = match op {
                                BinOp::Eq => "eq",
                                BinOp::Ne => "ne",
                                BinOp::Lt => "slt",
                                BinOp::Le => "sle",
                                BinOp::Gt => "sgt",
                                BinOp::Ge => "sge",
                                _ => unreachable!(),
                            };
                            let t = self.tmp();
                            writeln!(
                                self.out,
                                "  {t} = icmp {pred} i64 {}, {}",
                                l.repr, r.repr
                            )
                            .unwrap();
                            Ok(Value::scalar(LlvmTy::I1, t))
                        }
                    }
                    BinOp::And | BinOp::Or => unreachable!(),
                }
            }
        }
    }

    fn emit_call(
        &mut self,
        callee: &str,
        callee_span: Span,
        args: &[Expr],
        call_span: Span,
    ) -> Result<Value, Diagnostic> {
        match callee {
            "console.writeline" | "print" | "puts" => {
                if args.is_empty() {
                    return Err(Diagnostic::error(format!(
                        "this function takes at least 1 argument but {} arguments were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "expected at least 1 argument")
                    .label(callee_span, "function defined here (builtin)"));
                }
                for arg in args {
                    let v = self.emit_expr(arg)?;
                    match v.ty {
                        LlvmTy::I8Ptr => {
                            let t = self.tmp();
                            writeln!(self.out, "  {t} = call i32 @puts(ptr noundef {})", v.repr)
                                .unwrap();
                        }
                        LlvmTy::I64 | LlvmTy::I1 => {
                            let as_i64 = self.coerce(v, LlvmTy::I64, arg.span)?;
                            let t = self.tmp();
                            writeln!(
                                self.out,
                                "  {t} = call i32 (ptr, ...) @printf(ptr noundef @.fmt.int, i64 {})",
                                as_i64.repr
                            )
                            .unwrap();
                        }
                        LlvmTy::F64 => {
                            let as_f = self.coerce(v, LlvmTy::F64, arg.span)?;
                            writeln!(
                                self.out,
                                "  call void @slime_print_f6(double {})",
                                as_f.repr
                            )
                            .unwrap();
                        }
                        LlvmTy::Struct(_) | LlvmTy::Void | LlvmTy::Array { .. } => {
                            return Err(Diagnostic::error(format!(
                                "cannot print value of type `{}`",
                                v.ty.ir_type()
                            ))
                            .code("E0308")
                            .label(arg.span, "unsupported print argument type"));
                        }
                    }
                }
                Ok(Value::scalar(LlvmTy::I64, "0".into()))
            }
            "sin" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error(format!(
                        "this function takes 1 argument but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `sin`"));
                }
                let v = self.emit_expr(&args[0])?;
                let f = self.coerce(v, LlvmTy::F64, args[0].span)?;
                let t = self.tmp();
                writeln!(self.out, "  {t} = call double @sin(double {})", f.repr).unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            "cos" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error(format!(
                        "this function takes 1 argument but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `cos`"));
                }
                let v = self.emit_expr(&args[0])?;
                let f = self.coerce(v, LlvmTy::F64, args[0].span)?;
                let t = self.tmp();
                writeln!(self.out, "  {t} = call double @cos(double {})", f.repr).unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            "float" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error(format!(
                        "this function takes 1 argument but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `float`"));
                }
                let v = self.emit_expr(&args[0])?;
                let i = self.coerce(v, LlvmTy::I64, args[0].span)?;
                let t = self.tmp();
                writeln!(self.out, "  {t} = sitofp i64 {} to double", i.repr).unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            "mono_now" => {
                if !args.is_empty() {
                    return Err(Diagnostic::error(format!(
                        "this function takes 0 arguments but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `mono_now`"));
                }
                let t = self.tmp();
                writeln!(self.out, "  {t} = call double @slime_mono_now()").unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            "md5" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error(format!(
                        "this function takes 1 argument but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `md5`"));
                }
                let v = self.emit_expr(&args[0])?;
                let s = self.coerce(v, LlvmTy::I8Ptr, args[0].span)?;
                let t = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = call ptr @slime_md5_hex(ptr noundef {})",
                    s.repr
                )
                .unwrap();
                self.free_str_if_owned(&s);
                Ok(Value::owned_str(t))
            }
            "dict_new" => {
                if !args.is_empty() {
                    return Err(Diagnostic::error(format!(
                        "this function takes 0 arguments but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `dict_new`"));
                }
                let t = self.tmp();
                writeln!(self.out, "  {t} = call ptr @slime_dict_new()").unwrap();
                Ok(Value::scalar(LlvmTy::I8Ptr, t))
            }
            "dict_put" => {
                if args.len() != 3 {
                    return Err(Diagnostic::error(format!(
                        "this function takes 3 arguments but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `dict_put`"));
                }
                let d = self.emit_expr(&args[0])?;
                let dp = self.coerce(d, LlvmTy::I8Ptr, args[0].span)?;
                let k = self.emit_expr(&args[1])?;
                let kp = self.coerce(k, LlvmTy::I8Ptr, args[1].span)?;
                let v = self.emit_expr(&args[2])?;
                let vf = self.coerce(v, LlvmTy::F64, args[2].span)?;
                writeln!(
                    self.out,
                    "  call void @slime_dict_put(ptr noundef {}, ptr noundef {}, double {})",
                    dp.repr, kp.repr, vf.repr
                )
                .unwrap();
                self.free_str_if_owned(&kp);
                Ok(Value::scalar(LlvmTy::I64, "0".into()))
            }
            "dict_del_prefix1" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error(format!(
                        "this function takes 1 argument but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `dict_del_prefix1`"));
                }
                let d = self.emit_expr(&args[0])?;
                let dp = self.coerce(d, LlvmTy::I8Ptr, args[0].span)?;
                writeln!(
                    self.out,
                    "  call void @slime_dict_del_prefix1(ptr noundef {})",
                    dp.repr
                )
                .unwrap();
                Ok(Value::scalar(LlvmTy::I64, "0".into()))
            }
            "dict_sum" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error(format!(
                        "this function takes 1 argument but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `dict_sum`"));
                }
                let d = self.emit_expr(&args[0])?;
                let dp = self.coerce(d, LlvmTy::I8Ptr, args[0].span)?;
                let t = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = call double @slime_dict_sum(ptr noundef {})",
                    dp.repr
                )
                .unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            "itoa" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error(format!(
                        "this function takes 1 argument but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `itoa`"));
                }
                let v = self.emit_expr(&args[0])?;
                let i = self.coerce(v, LlvmTy::I64, args[0].span)?;
                let t = self.tmp();
                writeln!(self.out, "  {t} = call ptr @slime_itoa(i64 {})", i.repr).unwrap();
                Ok(Value::owned_str(t))
            }
            "print_f6" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error(format!(
                        "this function takes 1 argument but {} were supplied",
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "builtin `print_f6`"));
                }
                let v = self.emit_expr(&args[0])?;
                let f = self.coerce(v, LlvmTy::F64, args[0].span)?;
                writeln!(self.out, "  call void @slime_print_f6(double {})", f.repr).unwrap();
                Ok(Value::scalar(LlvmTy::I64, "0".into()))
            }
            "abs" => {
                if args.len() != 1 {
                    return Err(argc_mismatch(callee_span, call_span, 1, args.len()));
                }
                let v = self.emit_expr(&args[0])?;
                if matches!(v.ty, LlvmTy::F64) {
                    let t = self.tmp();
                    writeln!(self.out, "  {t} = call double @llvm.fabs.f64(double {})", v.repr)
                        .unwrap();
                    Ok(Value::scalar(LlvmTy::F64, t))
                } else {
                    let i = self.coerce(v, LlvmTy::I64, args[0].span)?;
                    let t = self.tmp();
                    writeln!(self.out, "  {t} = call i64 @labs(i64 {})", i.repr).unwrap();
                    Ok(Value::scalar(LlvmTy::I64, t))
                }
            }
            "min" | "max" => {
                if args.len() != 2 {
                    return Err(argc_mismatch(callee_span, call_span, 2, args.len()));
                }
                let a = self.emit_expr(&args[0])?;
                let b = self.emit_expr(&args[1])?;
                if matches!(a.ty, LlvmTy::F64) || matches!(b.ty, LlvmTy::F64) {
                    let af = self.coerce(a, LlvmTy::F64, args[0].span)?;
                    let bf = self.coerce(b, LlvmTy::F64, args[1].span)?;
                    let intr = if callee == "min" {
                        "llvm.minnum.f64"
                    } else {
                        "llvm.maxnum.f64"
                    };
                    let t = self.tmp();
                    writeln!(
                        self.out,
                        "  {t} = call double @{intr}(double {}, double {})",
                        af.repr, bf.repr
                    )
                    .unwrap();
                    Ok(Value::scalar(LlvmTy::F64, t))
                } else {
                    let ai = self.coerce(a, LlvmTy::I64, args[0].span)?;
                    let bi = self.coerce(b, LlvmTy::I64, args[1].span)?;
                    let cmp = self.tmp();
                    let t = self.tmp();
                    let pred = if callee == "min" { "slt" } else { "sgt" };
                    writeln!(
                        self.out,
                        "  {cmp} = icmp {pred} i64 {}, {}\n  {t} = select i1 {cmp}, i64 {}, i64 {}",
                        ai.repr, bi.repr, ai.repr, bi.repr
                    )
                    .unwrap();
                    Ok(Value::scalar(LlvmTy::I64, t))
                }
            }
            "clamp" => {
                if args.len() != 3 {
                    return Err(argc_mismatch(callee_span, call_span, 3, args.len()));
                }
                // clamp(x, lo, hi) = min(max(x, lo), hi)
                let x = self.emit_expr(&args[0])?;
                let lo = self.emit_expr(&args[1])?;
                let hi = self.emit_expr(&args[2])?;
                if matches!(x.ty, LlvmTy::F64)
                    || matches!(lo.ty, LlvmTy::F64)
                    || matches!(hi.ty, LlvmTy::F64)
                {
                    let xf = self.coerce(x, LlvmTy::F64, args[0].span)?;
                    let lof = self.coerce(lo, LlvmTy::F64, args[1].span)?;
                    let hif = self.coerce(hi, LlvmTy::F64, args[2].span)?;
                    let t1 = self.tmp();
                    let t2 = self.tmp();
                    writeln!(
                        self.out,
                        "  {t1} = call double @llvm.maxnum.f64(double {}, double {})\n  {t2} = call double @llvm.minnum.f64(double {t1}, double {})",
                        xf.repr, lof.repr, hif.repr
                    )
                    .unwrap();
                    Ok(Value::scalar(LlvmTy::F64, t2))
                } else {
                    let xi = self.coerce(x, LlvmTy::I64, args[0].span)?;
                    let loi = self.coerce(lo, LlvmTy::I64, args[1].span)?;
                    let hii = self.coerce(hi, LlvmTy::I64, args[2].span)?;
                    let c1 = self.tmp();
                    let t1 = self.tmp();
                    let c2 = self.tmp();
                    let t2 = self.tmp();
                    writeln!(
                        self.out,
                        "  {c1} = icmp sgt i64 {}, {}\n  {t1} = select i1 {c1}, i64 {}, i64 {}\n  {c2} = icmp slt i64 {t1}, {}\n  {t2} = select i1 {c2}, i64 {t1}, i64 {}",
                        xi.repr, loi.repr, xi.repr, loi.repr, hii.repr, hii.repr
                    )
                    .unwrap();
                    Ok(Value::scalar(LlvmTy::I64, t2))
                }
            }
            "sqrt" => {
                if args.len() != 1 {
                    return Err(argc_mismatch(callee_span, call_span, 1, args.len()));
                }
                let v = self.emit_expr(&args[0])?;
                let f = self.coerce(v, LlvmTy::F64, args[0].span)?;
                let t = self.tmp();
                writeln!(self.out, "  {t} = call double @sqrt(double {})", f.repr).unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            "pow" => {
                if args.len() != 2 {
                    return Err(argc_mismatch(callee_span, call_span, 2, args.len()));
                }
                let a0 = self.emit_expr(&args[0])?;
                let a = self.coerce(a0, LlvmTy::F64, args[0].span)?;
                let b0 = self.emit_expr(&args[1])?;
                let b = self.coerce(b0, LlvmTy::F64, args[1].span)?;
                let t = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = call double @pow(double {}, double {})",
                    a.repr, b.repr
                )
                .unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            "floor" | "ceil" | "round" => {
                if args.len() != 1 {
                    return Err(argc_mismatch(callee_span, call_span, 1, args.len()));
                }
                let v = self.emit_expr(&args[0])?;
                let f = self.coerce(v, LlvmTy::F64, args[0].span)?;
                let t = self.tmp();
                writeln!(self.out, "  {t} = call double @{callee}(double {})", f.repr).unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            "len" | "strlen" => {
                if args.len() != 1 {
                    return Err(argc_mismatch(callee_span, call_span, 1, args.len()));
                }
                let v = self.emit_expr(&args[0])?;
                let s = self.coerce(v, LlvmTy::I8Ptr, args[0].span)?;
                let t = self.tmp();
                writeln!(self.out, "  {t} = call i64 @strlen(ptr noundef {})", s.repr).unwrap();
                Ok(Value::scalar(LlvmTy::I64, t))
            }
            "strcmp" => {
                if args.len() != 2 {
                    return Err(argc_mismatch(callee_span, call_span, 2, args.len()));
                }
                let a0 = self.emit_expr(&args[0])?;
                let a = self.coerce(a0, LlvmTy::I8Ptr, args[0].span)?;
                let b0 = self.emit_expr(&args[1])?;
                let b = self.coerce(b0, LlvmTy::I8Ptr, args[1].span)?;
                let t = self.tmp();
                let e = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = call i32 @strcmp(ptr noundef {}, ptr noundef {})\n  {e} = sext i32 {t} to i64",
                    a.repr, b.repr
                )
                .unwrap();
                Ok(Value::scalar(LlvmTy::I64, e))
            }
            "substr" => {
                if args.len() != 3 {
                    return Err(argc_mismatch(callee_span, call_span, 3, args.len()));
                }
                let s0 = self.emit_expr(&args[0])?;
                let s = self.coerce(s0, LlvmTy::I8Ptr, args[0].span)?;
                let a0 = self.emit_expr(&args[1])?;
                let a = self.coerce(a0, LlvmTy::I64, args[1].span)?;
                let b0 = self.emit_expr(&args[2])?;
                let b = self.coerce(b0, LlvmTy::I64, args[2].span)?;
                let t = self.tmp();
                writeln!(
                    self.out,
                    "  {t} = call ptr @slime_substr(ptr noundef {}, i64 {}, i64 {})",
                    s.repr, a.repr, b.repr
                )
                .unwrap();
                Ok(Value::owned_str(t))
            }
            "atoi" => {
                if args.len() != 1 {
                    return Err(argc_mismatch(callee_span, call_span, 1, args.len()));
                }
                let v = self.emit_expr(&args[0])?;
                let s = self.coerce(v, LlvmTy::I8Ptr, args[0].span)?;
                let t = self.tmp();
                writeln!(self.out, "  {t} = call i64 @atoll(ptr noundef {})", s.repr).unwrap();
                Ok(Value::scalar(LlvmTy::I64, t))
            }
            "atof" => {
                if args.len() != 1 {
                    return Err(argc_mismatch(callee_span, call_span, 1, args.len()));
                }
                let v = self.emit_expr(&args[0])?;
                let s = self.coerce(v, LlvmTy::I8Ptr, args[0].span)?;
                let t = self.tmp();
                writeln!(self.out, "  {t} = call double @atof(ptr noundef {})", s.repr).unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            other => {
                let sig = self.func_sigs.get(other).cloned().ok_or_else(|| {
                    Diagnostic::error(format!("cannot find function `{other}` in this scope"))
                        .code("E0425")
                        .label(callee_span, "not found in this scope")
                        .note("available builtins: `console.writeline`, `print`")
                })?;
                if args.len() != sig.params.len() {
                    return Err(Diagnostic::error(format!(
                        "this function takes {} argument(s) but {} were supplied",
                        sig.params.len(),
                        args.len()
                    ))
                    .code("E0061")
                    .label(call_span, "argument count mismatch")
                    .label(callee_span, "function defined here"));
                }

                let mut arg_vals = Vec::new();
                let mut arg_ir = Vec::new();
                for (arg_expr, (pmode, param_ty)) in args.iter().zip(sig.params.iter()) {
                    match pmode {
                        ParamMode::Ref
                        | ParamMode::Out
                        | ParamMode::Borrow
                        | ParamMode::BorrowMut => {
                            if let ExprKind::Ident(name) = &arg_expr.kind {
                                let (_, ptr, _) = self.locals.get(name).cloned().ok_or_else(|| {
                                    Diagnostic::error(format!(
                                        "cannot find value `{name}` in this scope"
                                    ))
                                    .code("E0425")
                                    .label(arg_expr.span, "not found in this scope")
                                })?;
                                arg_ir.push(format!("ptr {ptr}"));
                            } else if let ExprKind::Unary {
                                op: UnaryOp::AddrOf | UnaryOp::AddrOfMut,
                                expr: inner,
                            } = &arg_expr.kind
                            {
                                let v = self.emit_expr(inner)?;
                                arg_ir.push(format!("ptr {}", v.repr));
                            } else {
                                return Err(Diagnostic::error(
                                    "ref/out/borrow argument requires address of lvalue",
                                )
                                .code("E0308")
                                .label(arg_expr.span, "pass `&var` or a variable name"));
                            }
                        }
                        ParamMode::Value
                        | ParamMode::Own
                        | ParamMode::Share
                        | ParamMode::Joint => {
                            let v = self.emit_expr(arg_expr)?;
                            let param_lty = self.resolve_type(param_ty, arg_expr.span)?;
                            let casted = self.coerce(v, param_lty.clone(), arg_expr.span)?;
                            arg_ir.push(format!("{} {}", param_lty.ir_type(), casted.repr));
                            arg_vals.push(casted);
                        }
                    }
                }
                let _ = arg_vals;

                let ret_is_void = sig.ret_ty.as_ref().is_none_or(|t| t.is_void());
                let ret_lty = if ret_is_void {
                    LlvmTy::Void
                } else {
                    self.resolve_type(sig.ret_ty.as_ref().unwrap(), callee_span)?
                };

                if ret_is_void {
                    writeln!(
                        self.out,
                        "  call void @{}({})",
                        mangle(other),
                        arg_ir.join(", ")
                    )
                    .unwrap();
                    Ok(Value::scalar(LlvmTy::I64, "0".into()))
                } else {
                    let t = self.tmp();
                    writeln!(
                        self.out,
                        "  {t} = call {} @{}({})",
                        ret_lty.ir_type(),
                        mangle(other),
                        arg_ir.join(", ")
                    )
                    .unwrap();
                    Ok(Value::scalar(ret_lty, t))
                }
            }
        }
    }

    fn to_i1(&mut self, v: Value) -> Result<Value, Diagnostic> {
        match v.ty {
            LlvmTy::I1 => Ok(v),
            LlvmTy::I64 => {
                let t = self.tmp();
                writeln!(self.out, "  {t} = icmp ne i64 {}, 0", v.repr).unwrap();
                Ok(Value::scalar(LlvmTy::I1, t))
            }
            LlvmTy::F64 => {
                let t = self.tmp();
                writeln!(self.out, "  {t} = fcmp one double {}, 0.0", v.repr).unwrap();
                Ok(Value::scalar(LlvmTy::I1, t))
            }
            LlvmTy::I8Ptr => {
                let t = self.tmp();
                writeln!(self.out, "  {t} = icmp ne ptr {}, null", v.repr).unwrap();
                Ok(Value::scalar(LlvmTy::I1, t))
            }
            LlvmTy::Struct(_) | LlvmTy::Void | LlvmTy::Array { .. } => Err(Diagnostic::error(format!(
                "cannot use `{}` as a boolean condition",
                v.ty.ir_type()
            ))
            .code("E0308")
            .label(Span { start: 0, end: 0 }, "expected scalar boolean")),
        }
    }

    fn coerce(&mut self, v: Value, target: LlvmTy, span: Span) -> Result<Value, Diagnostic> {
        if v.is_struct_ptr {
            if v.ty == target {
                let agg = match &target {
                    LlvmTy::Struct(name) => {
                        let t = self.tmp();
                        writeln!(
                            self.out,
                            "  {t} = load %{name}, ptr {}, align 8",
                            v.repr
                        )
                        .unwrap();
                        t
                    }
                    _ => unreachable!(),
                };
                return Ok(Value::scalar(target, agg));
            }
            return Err(Diagnostic::error(format!(
                "mismatched types: expected `{}`, found `{}`",
                target.ir_type(),
                v.ty.ir_type()
            ))
            .code("E0308")
            .label(span, format!("expected `{}`", target.ir_type())));
        }

        if v.ty == target {
            return Ok(v);
        }

        match (&v.ty, &target) {
            (LlvmTy::I1, LlvmTy::I64) => {
                let t = self.tmp();
                writeln!(self.out, "  {t} = zext i1 {} to i64", v.repr).unwrap();
                Ok(Value::scalar(LlvmTy::I64, t))
            }
            (LlvmTy::I64, LlvmTy::F64) => {
                let t = self.tmp();
                writeln!(self.out, "  {t} = sitofp i64 {} to double", v.repr).unwrap();
                Ok(Value::scalar(LlvmTy::F64, t))
            }
            (LlvmTy::F64, LlvmTy::I64) => {
                let t = self.tmp();
                writeln!(self.out, "  {t} = fptosi double {} to i64", v.repr).unwrap();
                Ok(Value::scalar(LlvmTy::I64, t))
            }
            (LlvmTy::I64, LlvmTy::I1) => self.to_i1(v),
            (LlvmTy::F64, LlvmTy::I1) => self.to_i1(v),
            (LlvmTy::Struct(a), LlvmTy::Struct(b)) if a == b => Ok(v),
            (LlvmTy::Struct(_), LlvmTy::I64)
            | (LlvmTy::Struct(_), LlvmTy::F64)
            | (LlvmTy::Struct(_), LlvmTy::I1)
            | (LlvmTy::Struct(_), LlvmTy::I8Ptr) => {
                Err(Diagnostic::error("cannot coerce struct to scalar type")
                    .code("E0308")
                    .label(span, "struct cannot be used as a scalar"))
            }
            _ => Err(Diagnostic::error(format!(
                "mismatched types: expected `{}`, found `{}`",
                target.ir_type(),
                v.ty.ir_type()
            ))
            .code("E0308")
            .label(span, format!("expected `{}`", target.ir_type()))),
        }
    }
}

fn mangle(name: &str) -> String {
    name.replace('.', "_")
}

fn format_float(n: f64) -> String {
    let s = n.to_string();
    if s.contains(['.', 'e', 'E']) {
        s
    } else {
        format!("{s}.0")
    }
}

/// True if `expr` loads any identifier (loop-carried / local state).
fn expr_reads_ident(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Ident(_) => true,
        ExprKind::Binary { left, right, .. } => {
            expr_reads_ident(left) || expr_reads_ident(right)
        }
        ExprKind::Call { args, .. } => args.iter().any(expr_reads_ident),
        ExprKind::Unary { expr, .. } => expr_reads_ident(expr),
        ExprKind::MethodCall { receiver, args, .. } => {
            expr_reads_ident(receiver) || args.iter().any(expr_reads_ident)
        }
        ExprKind::Field { base, .. } => expr_reads_ident(base),
        ExprKind::Index { base, index } => {
            expr_reads_ident(base) || expr_reads_ident(index)
        }
        ExprKind::StructLit { fields, .. } => fields.iter().any(|(_, _, e)| expr_reads_ident(e)),
        ExprKind::IntLit(_)
        | ExprKind::FloatLit(_)
        | ExprKind::StrLit(_)
        | ExprKind::BoolLit(_) => false,
    }
}

fn escape_llvm_string(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\22"),
            0x0A => out.push_str("\\0A"),
            0x09 => out.push_str("\\09"),
            0x0D => out.push_str("\\0D"),
            0x20..=0x7E => out.push(b as char),
            _ => {
                out.push('\\');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}
