//! Monomorphization + method lowering pass (runs after parse, before codegen/interp).

use std::collections::{HashMap, VecDeque};

use crate::ast::*;
use crate::diag::{Diagnostic, Span};

pub fn monomorphize(program: &mut Program) -> Result<(), Diagnostic> {
    expand_struct_methods(program);
    let mut m = Monomorphizer::new(std::mem::take(&mut program.structs), std::mem::take(&mut program.functions));
    m.run()?;
    program.structs = m.concrete_structs.into_values().collect();
    program.functions = m.concrete_fns.into_values().collect();
    // Stable order for codegen/debug
    program.structs.sort_by(|a, b| a.name.cmp(&b.name));
    program.functions.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(())
}

fn expand_struct_methods(program: &mut Program) {
    let mut extra = Vec::new();
    for sd in &program.structs {
        for m in &sd.methods {
            let mut f = m.func.clone();
            if f.generic_params.is_empty() && !sd.generic_params.is_empty() {
                f.generic_params = sd.generic_params.clone();
            }
            f.name = if m.is_virtual {
                format!("__virt_{}_{}", sd.name, f.name)
            } else {
                format!("{}_{}", sd.name, f.name)
            };
            extra.push(f);
        }
    }
    program.functions.extend(extra);
    for sd in &mut program.structs {
        sd.methods.clear();
    }
}

fn mangle_inst(base: &str, args: &[Type]) -> String {
    if args.is_empty() {
        base.to_string()
    } else {
        format!(
            "{}__{}",
            base,
            args.iter().map(Type::mono_name).collect::<Vec<_>>().join("_")
        )
    }
}

/// Rebuild `a.b.c` from a field/ident chain + trailing method name (module-style calls).
fn dotted_callee(recv: &Expr, method: &str) -> String {
    fn push_path(expr: &Expr, out: &mut Vec<String>) {
        match &expr.kind {
            ExprKind::Ident(n) => out.push(n.clone()),
            ExprKind::Field { base, field, .. } => {
                push_path(base, out);
                out.push(field.clone());
            }
            _ => out.push("_".into()),
        }
    }
    let mut parts = Vec::new();
    push_path(recv, &mut parts);
    parts.push(method.to_string());
    parts.join(".")
}

fn subst_type(ty: &Type, map: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Named(name) if map.contains_key(name) => map[name].clone(),
        Type::Generic(name) => map
            .get(name)
            .cloned()
            .unwrap_or_else(|| Type::Generic(name.clone())),
        Type::Apply { name, args } => {
            let nargs: Vec<_> = args.iter().map(|a| subst_type(a, map)).collect();
            if nargs.iter().any(|t| matches!(t, Type::Generic(_))) {
                Type::Apply {
                    name: name.clone(),
                    args: nargs,
                }
            } else {
                Type::Named(mangle_inst(name, &nargs))
            }
        }
        Type::Ptr(inner) => Type::Ptr(Box::new(subst_type(inner, map))),
        Type::Array { elem, len } => Type::Array {
            elem: Box::new(subst_type(elem, map)),
            len: *len,
        },
        Type::Owned { spec, inner } => Type::Owned {
            spec: *spec,
            inner: Box::new(subst_type(inner, map)),
        },
        Type::Borrowed { mutable, inner } => Type::Borrowed {
            mutable: *mutable,
            inner: Box::new(subst_type(inner, map)),
        },
        other => other.clone(),
    }
}

fn subst_param(p: &Param, map: &HashMap<String, Type>) -> Param {
    Param {
        mode: p.mode,
        name: p.name.clone(),
        name_span: p.name_span,
        ty: subst_type(&p.ty, map),
    }
}

fn subst_expr(expr: &Expr, map: &HashMap<String, Type>) -> Expr {
    let kind = match &expr.kind {
        ExprKind::IntLit(n) => ExprKind::IntLit(*n),
        ExprKind::FloatLit(n) => ExprKind::FloatLit(*n),
        ExprKind::StrLit(s) => ExprKind::StrLit(s.clone()),
        ExprKind::BoolLit(b) => ExprKind::BoolLit(*b),
        ExprKind::Ident(s) => ExprKind::Ident(s.clone()),
        ExprKind::Unary { op, expr } => ExprKind::Unary {
            op: *op,
            expr: Box::new(subst_expr(expr, map)),
        },
        ExprKind::Binary { op, left, right } => ExprKind::Binary {
            op: *op,
            left: Box::new(subst_expr(left, map)),
            right: Box::new(subst_expr(right, map)),
        },
        ExprKind::Call {
            callee,
            callee_span,
            type_args,
            args,
        } => {
            let nargs: Vec<_> = type_args.iter().map(|t| subst_type(t, map)).collect();
            let mono_callee = if nargs.is_empty() {
                callee.clone()
            } else if type_args.is_empty() {
                callee.clone()
            } else {
                mangle_inst(callee, &nargs)
            };
            ExprKind::Call {
                callee: mono_callee,
                callee_span: *callee_span,
                type_args: Vec::new(),
                args: args.iter().map(|a| subst_expr(a, map)).collect(),
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            method_span,
            args,
        } => ExprKind::MethodCall {
            receiver: Box::new(subst_expr(receiver, map)),
            method: method.clone(),
            method_span: *method_span,
            args: args.iter().map(|a| subst_expr(a, map)).collect(),
        },
        ExprKind::StructLit {
            name,
            name_span,
            type_args,
            fields,
        } => {
            let nargs: Vec<_> = type_args.iter().map(|t| subst_type(t, map)).collect();
            let mono_name = if nargs.is_empty() {
                name.clone()
            } else {
                mangle_inst(name, &nargs)
            };
            ExprKind::StructLit {
                name: mono_name,
                name_span: *name_span,
                type_args: Vec::new(),
                fields: fields
                    .iter()
                    .map(|(n, sp, e)| (n.clone(), *sp, subst_expr(e, map)))
                    .collect(),
            }
        }
        ExprKind::Field {
            base,
            field,
            field_span,
        } => ExprKind::Field {
            base: Box::new(subst_expr(base, map)),
            field: field.clone(),
            field_span: *field_span,
        },
        ExprKind::Index { base, index } => ExprKind::Index {
            base: Box::new(subst_expr(base, map)),
            index: Box::new(subst_expr(index, map)),
        },
    };
    Expr {
        span: expr.span,
        kind,
    }
}

fn subst_stmt(stmt: &Stmt, map: &HashMap<String, Type>) -> Stmt {
    let kind = match &stmt.kind {
        StmtKind::VarDecl {
            name,
            name_span,
            ty,
            own,
            init,
        } => StmtKind::VarDecl {
            name: name.clone(),
            name_span: *name_span,
            ty: subst_type(ty, map),
            own: *own,
            init: subst_expr(init, map),
        },
        StmtKind::ArrayDecl {
            name,
            name_span,
            elem,
            len,
        } => StmtKind::ArrayDecl {
            name: name.clone(),
            name_span: *name_span,
            elem: subst_type(elem, map),
            len: *len,
        },
        StmtKind::IndexAssign {
            base,
            base_span,
            index,
            value,
        } => StmtKind::IndexAssign {
            base: base.clone(),
            base_span: *base_span,
            index: subst_expr(index, map),
            value: subst_expr(value, map),
        },
        StmtKind::Assign {
            name,
            name_span,
            value,
        } => StmtKind::Assign {
            name: name.clone(),
            name_span: *name_span,
            value: subst_expr(value, map),
        },
        StmtKind::DerefAssign { ptr, value } => StmtKind::DerefAssign {
            ptr: subst_expr(ptr, map),
            value: subst_expr(value, map),
        },
        StmtKind::FieldAssign {
            base,
            base_span,
            field,
            field_span,
            value,
        } => StmtKind::FieldAssign {
            base: base.clone(),
            base_span: *base_span,
            field: field.clone(),
            field_span: *field_span,
            value: subst_expr(value, map),
        },
        StmtKind::If {
            cond,
            then_body,
            else_body,
        } => StmtKind::If {
            cond: subst_expr(cond, map),
            then_body: then_body.iter().map(|s| subst_stmt(s, map)).collect(),
            else_body: else_body.iter().map(|s| subst_stmt(s, map)).collect(),
        },
        StmtKind::While { cond, body } => StmtKind::While {
            cond: subst_expr(cond, map),
            body: body.iter().map(|s| subst_stmt(s, map)).collect(),
        },
        StmtKind::For {
            name,
            name_span,
            start,
            end,
            body,
        } => StmtKind::For {
            name: name.clone(),
            name_span: *name_span,
            start: subst_expr(start, map),
            end: subst_expr(end, map),
            body: body.iter().map(|s| subst_stmt(s, map)).collect(),
        },
        StmtKind::Break => StmtKind::Break,
        StmtKind::Continue => StmtKind::Continue,
        StmtKind::Print(args) => StmtKind::Print(args.iter().map(|a| subst_expr(a, map)).collect()),
        StmtKind::Return(e) => StmtKind::Return(e.as_ref().map(|x| subst_expr(x, map))),
        StmtKind::Expr(e) => StmtKind::Expr(subst_expr(e, map)),
    };
    Stmt {
        span: stmt.span,
        kind,
    }
}

fn mono_function_named(base: &Function, args: &[Type], name: String) -> Function {
    let map: HashMap<_, _> = base
        .generic_params
        .iter()
        .cloned()
        .zip(args.iter().cloned())
        .collect();
    Function {
        name,
        name_span: base.name_span,
        generic_params: Vec::new(),
        params: base.params.iter().map(|p| subst_param(p, &map)).collect(),
        ret_ty: base.ret_ty.as_ref().map(|t| subst_type(t, &map)),
        body: base.body.iter().map(|s| subst_stmt(s, &map)).collect(),
        span: base.span,
    }
}

fn mono_function(base: &Function, args: &[Type]) -> Function {
    mono_function_named(base, args, mangle_inst(&base.name, args))
}

fn mono_struct_def(base: &StructDef, args: &[Type]) -> StructDef {
    let map: HashMap<_, _> = base
        .generic_params
        .iter()
        .cloned()
        .zip(args.iter().cloned())
        .collect();
    StructDef {
        name: mangle_inst(&base.name, args),
        name_span: base.name_span,
        generic_params: Vec::new(),
        fields: base
            .fields
            .iter()
            .map(|f| StructField {
                name: f.name.clone(),
                name_span: f.name_span,
                ty: subst_type(&f.ty, &map),
            })
            .collect(),
        methods: Vec::new(),
        span: base.span,
    }
}

fn is_monomorphizable(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Int | Type::Float | Type::Bool | Type::Str | Type::Named(_)
    )
}

fn infer_expr_type(
    expr: &Expr,
    env: &HashMap<String, Type>,
    structs: &HashMap<String, StructDef>,
) -> Option<Type> {
    match &expr.kind {
        ExprKind::IntLit(_) => Some(Type::Int),
        ExprKind::FloatLit(_) => Some(Type::Float),
        ExprKind::BoolLit(_) => Some(Type::Bool),
        ExprKind::StrLit(_) => Some(Type::Str),
        ExprKind::Ident(n) => env.get(n).cloned(),
        ExprKind::Unary { op: UnaryOp::Deref, expr } => match infer_expr_type(expr, env, structs) {
            Some(Type::Ptr(inner) | Type::Borrowed { inner, .. }) => Some(*inner),
            _ => None,
        },
        ExprKind::Unary {
            op: UnaryOp::AddrOf,
            expr,
        } => {
            let inner = infer_expr_type(expr, env, structs)?;
            Some(Type::Borrowed {
                mutable: false,
                inner: Box::new(inner.peel().clone()),
            })
        }
        ExprKind::Unary {
            op: UnaryOp::AddrOfMut,
            expr,
        } => {
            let inner = infer_expr_type(expr, env, structs)?;
            Some(Type::Borrowed {
                mutable: true,
                inner: Box::new(inner.peel().clone()),
            })
        }
        ExprKind::Unary {
            op: UnaryOp::Move,
            expr,
        } => infer_expr_type(expr, env, structs),
        ExprKind::Binary { left, .. } => infer_expr_type(left, env, structs),
        ExprKind::Call { callee, type_args, .. } => {
            if type_args.len() == 1 && is_monomorphizable(&type_args[0]) {
                Some(type_args[0].clone())
            } else if !type_args.is_empty() {
                None
            } else {
                env.get(callee).cloned()
            }
        }
        ExprKind::StructLit { name, type_args, .. } => {
            if type_args.is_empty() {
                Some(Type::Named(name.clone()))
            } else if type_args.iter().all(is_monomorphizable) {
                Some(Type::Named(mangle_inst(name, type_args)))
            } else {
                None
            }
        }
        ExprKind::Field { base, field, .. } => {
            let base_ty = infer_expr_type(base, env, structs)?;
            if let Type::Named(ref sname) = base_ty {
                if let Some(sd) = structs.get(sname) {
                    if let Some(f) = sd.fields.iter().find(|f| f.name == *field) {
                        return Some(f.ty.clone());
                    }
                }
            }
            Some(base_ty)
        }
        ExprKind::MethodCall { receiver, .. } => infer_expr_type(receiver, env, structs),
        ExprKind::Index { base, .. } => infer_expr_type(base, env, structs),
    }
}

fn resolve_apply(ty: &Type, struct_templates: &HashMap<String, StructDef>) -> Type {
    match ty {
        Type::Apply { name, args } if args.iter().all(is_monomorphizable) => {
            if struct_templates.contains_key(name) {
                Type::Named(mangle_inst(name, args))
            } else {
                Type::Named(mangle_inst(name, args))
            }
        }
        Type::Ptr(inner) => Type::Ptr(Box::new(resolve_apply(inner, struct_templates))),
        Type::Array { elem, len } => Type::Array {
            elem: Box::new(resolve_apply(elem, struct_templates)),
            len: *len,
        },
        Type::Owned { spec, inner } => Type::Owned {
            spec: *spec,
            inner: Box::new(resolve_apply(inner, struct_templates)),
        },
        Type::Borrowed { mutable, inner } => Type::Borrowed {
            mutable: *mutable,
            inner: Box::new(resolve_apply(inner, struct_templates)),
        },
        other => other.clone(),
    }
}

struct Monomorphizer {
    fn_templates: HashMap<String, Function>,
    struct_templates: HashMap<String, StructDef>,
    concrete_fns: HashMap<String, Function>,
    concrete_structs: HashMap<String, StructDef>,
    pending_fns: VecDeque<(String, Vec<Type>)>,
    pending_structs: VecDeque<(String, Vec<Type>)>,
}

impl Monomorphizer {
    fn new(structs: Vec<StructDef>, functions: Vec<Function>) -> Self {
        let mut fn_templates = HashMap::new();
        let mut concrete_fns = HashMap::new();
        for f in functions {
            if f.generic_params.is_empty() {
                concrete_fns.insert(f.name.clone(), f);
            } else {
                fn_templates.insert(f.name.clone(), f);
            }
        }

        let mut struct_templates = HashMap::new();
        let mut concrete_structs = HashMap::new();
        for sd in structs {
            if sd.generic_params.is_empty() {
                concrete_structs.insert(sd.name.clone(), sd);
            } else {
                struct_templates.insert(sd.name.clone(), sd);
            }
        }

        Self {
            fn_templates,
            struct_templates,
            concrete_fns,
            concrete_structs,
            pending_fns: VecDeque::new(),
            pending_structs: VecDeque::new(),
        }
    }

    fn run(&mut self) -> Result<(), Diagnostic> {
        // Seed worklist from explicit type args in concrete functions
        for f in self.concrete_fns.values().cloned().collect::<Vec<_>>() {
            self.scan_for_instances(&f.body, &HashMap::new())?;
        }
        for f in self.fn_templates.values().cloned().collect::<Vec<_>>() {
            self.scan_for_instances(&f.body, &HashMap::new())?;
        }

        while let Some((name, args)) = self.pending_fns.pop_front() {
            self.instantiate_fn(&name, &args)?;
        }
        while let Some((name, args)) = self.pending_structs.pop_front() {
            self.instantiate_struct(&name, &args)?;
        }

        // Rewrite method calls and normalize types in all concrete functions
        let names: Vec<_> = self.concrete_fns.keys().cloned().collect();
        for name in names {
            let f = self.concrete_fns.remove(&name).unwrap();
            let mut env = HashMap::new();
            for p in &f.params {
                env.insert(p.name.clone(), p.ty.clone());
            }
            let body = self.rewrite_stmts(f.body, &env)?;
            self.concrete_fns.insert(name, Function { body, ..f });
        }

        Ok(())
    }

    fn rewrite_stmts(
        &self,
        stmts: Vec<Stmt>,
        env: &HashMap<String, Type>,
    ) -> Result<Vec<Stmt>, Diagnostic> {
        let mut out = Vec::with_capacity(stmts.len());
        let mut env = env.clone();
        for stmt in stmts {
            let (stmt, ty) = self.rewrite_stmt(stmt, &env)?;
            if let Some((name, t)) = ty {
                env.insert(name, t);
            }
            out.push(stmt);
        }
        Ok(out)
    }

    fn queue_fn(&mut self, name: &str, args: &[Type]) {
        if args.is_empty() || !args.iter().all(is_monomorphizable) {
            return;
        }
        let key = mangle_inst(name, args);
        if self.concrete_fns.contains_key(&key) || !self.fn_templates.contains_key(name) {
            return;
        }
        self.pending_fns.push_back((name.to_string(), args.to_vec()));
    }

    fn queue_struct(&mut self, name: &str, args: &[Type]) {
        if args.is_empty() {
            return;
        }
        if !args.iter().all(is_monomorphizable) {
            return;
        }
        let key = mangle_inst(name, args);
        if self.concrete_structs.contains_key(&key) {
            return;
        }
        if self.struct_templates.contains_key(name) {
            self.pending_structs
                .push_back((name.to_string(), args.to_vec()));
        }
    }

    fn method_mono_name(struct_base: &str, method: &str, args: &[Type], virt: bool) -> String {
        let sm = mangle_inst(struct_base, args);
        if virt {
            format!("__virt_{sm}_{method}")
        } else {
            format!("{sm}_{method}")
        }
    }

    fn instantiate_method(
        &mut self,
        struct_base: &str,
        method: &str,
        args: &[Type],
        virt: bool,
    ) -> Result<(), Diagnostic> {
        let template_key = if virt {
            format!("__virt_{struct_base}_{method}")
        } else {
            format!("{struct_base}_{method}")
        };
        let mono_name = Self::method_mono_name(struct_base, method, args, virt);
        if self.concrete_fns.contains_key(&mono_name) {
            return Ok(());
        }
        let Some(template) = self.fn_templates.get(&template_key).cloned() else {
            return Ok(());
        };
        if args.len() != template.generic_params.len() {
            return Ok(());
        }
        let mono = mono_function_named(&template, args, mono_name);
        self.concrete_fns.insert(mono.name.clone(), mono.clone());
        self.scan_for_instances(&mono.body, &HashMap::new())?;
        Ok(())
    }

    fn queue_struct_methods(&mut self, struct_base: &str, args: &[Type]) -> Result<(), Diagnostic> {
        if args.is_empty() || !args.iter().all(is_monomorphizable) {
            return Ok(());
        }
        for key in self.fn_templates.keys().cloned().collect::<Vec<_>>() {
            if let Some(method) = key.strip_prefix(&format!("{struct_base}_")) {
                if !key.starts_with("__virt_") {
                    self.instantiate_method(struct_base, method, args, false)?;
                }
            } else if let Some(method) = key.strip_prefix(&format!("__virt_{struct_base}_")) {
                self.instantiate_method(struct_base, method, args, true)?;
            }
        }
        Ok(())
    }

    fn instantiate_fn(&mut self, name: &str, args: &[Type]) -> Result<(), Diagnostic> {
        let key = mangle_inst(name, args);
        if self.concrete_fns.contains_key(&key) {
            return Ok(());
        }
        let Some(template) = self.fn_templates.get(name).cloned() else {
            return Ok(());
        };
        if args.len() != template.generic_params.len() {
            return Err(Diagnostic::error(format!(
                "generic argument count mismatch for `{name}`"
            ))
            .code("E0308")
            .label(Span::dummy(), format!("expected {} type args", template.generic_params.len())));
        }
        let mono = mono_function(&template, args);
        self.concrete_fns.insert(key, mono.clone());
        self.scan_for_instances(&mono.body, &HashMap::new())?;
        Ok(())
    }

    fn instantiate_struct(&mut self, name: &str, args: &[Type]) -> Result<(), Diagnostic> {
        let key = mangle_inst(name, args);
        if self.concrete_structs.contains_key(&key) {
            return Ok(());
        }
        let Some(template) = self.struct_templates.get(name).cloned() else {
            return Ok(());
        };
        if args.len() != template.generic_params.len() {
            return Err(Diagnostic::error(format!(
                "generic argument count mismatch for struct `{name}`"
            ))
            .code("E0308")
            .label(Span::dummy(), format!("expected {} type args", template.generic_params.len())));
        }
        let mono = mono_struct_def(&template, args);
        self.concrete_structs.insert(key.clone(), mono);
        self.queue_struct_methods(name, args)?;
        Ok(())
    }

    fn infer_fn_type_args(
        &self,
        template: &Function,
        type_args: &[Type],
        args: &[Expr],
        env: &HashMap<String, Type>,
    ) -> Option<Vec<Type>> {
        if !type_args.is_empty() {
            if type_args.len() == template.generic_params.len()
                && type_args.iter().all(is_monomorphizable)
            {
                return Some(type_args.to_vec());
            }
            return None;
        }
        if template.generic_params.is_empty() {
            return Some(Vec::new());
        }
        if template.generic_params.len() == 1 {
            for arg in args {
                if let Some(t) = infer_expr_type(arg, env, &self.concrete_structs) {
                    if is_monomorphizable(&t) {
                        return Some(vec![t]);
                    }
                }
            }
            for p in &template.params {
                if is_monomorphizable(&p.ty) {
                    return Some(vec![p.ty.clone()]);
                }
            }
        }
        None
    }

    fn scan_expr(&mut self, expr: &Expr, env: &HashMap<String, Type>) -> Result<(), Diagnostic> {
        match &expr.kind {
            ExprKind::Call {
                callee,
                type_args,
                args,
                ..
            } => {
                if let Some(template) = self.fn_templates.get(callee).cloned() {
                    if let Some(margs) = self.infer_fn_type_args(&template, type_args, args, env) {
                        self.queue_fn(callee, &margs);
                    }
                }
                for a in args {
                    self.scan_expr(a, env)?;
                }
            }
            ExprKind::StructLit { name, type_args, fields, .. } => {
                if !type_args.is_empty() {
                    self.queue_struct(name, type_args);
                }
                for (_, _, e) in fields {
                    self.scan_expr(e, env)?;
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.scan_expr(receiver, env)?;
                for a in args {
                    self.scan_expr(a, env)?;
                }
            }
            ExprKind::Unary { expr, .. } => self.scan_expr(expr, env)?,
            ExprKind::Binary { left, right, .. } => {
                self.scan_expr(left, env)?;
                self.scan_expr(right, env)?;
            }
            ExprKind::Field { base, .. } => self.scan_expr(base, env)?,
            ExprKind::Index { base, index } => {
                self.scan_expr(base, env)?;
                self.scan_expr(index, env)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn scan_stmt(&mut self, stmt: &Stmt, env: &HashMap<String, Type>) -> Result<(), Diagnostic> {
        match &stmt.kind {
            StmtKind::VarDecl { ty, init, .. } => {
                self.scan_expr(init, env)?;
                if let Type::Apply { name: sname, args } = ty {
                    if args.iter().all(is_monomorphizable) {
                        self.queue_struct(sname, args);
                    }
                }
            }
            StmtKind::Assign { value, .. }
            | StmtKind::Return(Some(value))
            | StmtKind::Expr(value) => {
                self.scan_expr(value, env)?;
            }
            StmtKind::DerefAssign { ptr, value } => {
                self.scan_expr(ptr, env)?;
                self.scan_expr(value, env)?;
            }
            StmtKind::If { cond, then_body, else_body } => {
                self.scan_expr(cond, env)?;
                self.scan_stmts(then_body, env)?;
                self.scan_stmts(else_body, env)?;
            }
            StmtKind::While { cond, body } => {
                self.scan_expr(cond, env)?;
                self.scan_stmts(body, env)?;
            }
            StmtKind::For { start, end, body, .. } => {
                self.scan_expr(start, env)?;
                self.scan_expr(end, env)?;
                self.scan_stmts(body, env)?;
            }
            StmtKind::Print(args) => {
                for a in args {
                    self.scan_expr(a, env)?;
                }
            }
            StmtKind::IndexAssign { index, value, .. } => {
                self.scan_expr(index, env)?;
                self.scan_expr(value, env)?;
            }
            StmtKind::FieldAssign { value, .. } => self.scan_expr(value, env)?,
            StmtKind::ArrayDecl { .. }
            | StmtKind::Break
            | StmtKind::Continue
            | StmtKind::Return(None) => {}
        }
        Ok(())
    }

    fn scan_stmts(&mut self, stmts: &[Stmt], env: &HashMap<String, Type>) -> Result<(), Diagnostic> {
        for s in stmts {
            self.scan_stmt(s, env)?;
        }
        Ok(())
    }

    fn scan_for_instances(&mut self, body: &[Stmt], env: &HashMap<String, Type>) -> Result<(), Diagnostic> {
        self.scan_stmts(body, env)?;
        // Drain pending queues iteratively
        while let Some((n, a)) = self.pending_fns.pop_front() {
            self.instantiate_fn(&n, &a)?;
        }
        while let Some((n, a)) = self.pending_structs.pop_front() {
            self.instantiate_struct(&n, &a)?;
        }
        Ok(())
    }

    fn lookup_method(&self, struct_mono: &str, method: &str) -> Option<String> {
        let direct = format!("{struct_mono}_{method}");
        if self.concrete_fns.contains_key(&direct) {
            return Some(direct);
        }
        let virt = format!("__virt_{struct_mono}_{method}");
        if self.concrete_fns.contains_key(&virt) {
            return Some(virt);
        }
        None
    }

    fn method_needs_receiver(&self, callee: &str) -> bool {
        self.concrete_fns
            .get(callee)
            .and_then(|f| f.params.first())
            .is_some_and(|p| {
                p.mode != ParamMode::Value
                    || p.name == "self"
                    || matches!(
                        p.mode,
                        ParamMode::Own
                            | ParamMode::Share
                            | ParamMode::Joint
                            | ParamMode::Borrow
                            | ParamMode::BorrowMut
                    )
            })
    }

    fn rewrite_expr(&self, expr: Expr, env: &HashMap<String, Type>) -> Result<Expr, Diagnostic> {
        let span = expr.span;
        let kind = match expr.kind {
            ExprKind::MethodCall {
                receiver,
                method,
                method_span,
                args,
            } => {
                let recv = self.rewrite_expr(*receiver, env)?;
                // `console.writeline(...)` is parsed like a method call; fall back to a
                // dotted free-function call when the receiver is not a known local/struct.
                let Some(recv_ty) = infer_expr_type(&recv, env, &self.concrete_structs) else {
                    let callee = dotted_callee(&recv, &method);
                    return Ok(Expr {
                        span,
                        kind: ExprKind::Call {
                            callee,
                            callee_span: method_span,
                            type_args: Vec::new(),
                            args: args
                                .into_iter()
                                .map(|a| self.rewrite_expr(a, env))
                                .collect::<Result<_, _>>()?,
                        },
                    });
                };
                let struct_mono = match recv_ty {
                    Type::Named(n) => n,
                    other => {
                        return Err(Diagnostic::error(format!(
                            "method call requires struct receiver, found `{other:?}`"
                        ))
                        .code("E0599")
                        .label(recv.span, "invalid receiver"));
                    }
                };
                let callee = self.lookup_method(&struct_mono, &method).ok_or_else(|| {
                    Diagnostic::error(format!("no method `{method}` on `{struct_mono}`"))
                        .code("E0599")
                        .label(method_span, "method not found")
                })?;
                let mut call_args = Vec::new();
                if self.method_needs_receiver(&callee) {
                    call_args.push(Expr {
                        span: recv.span,
                        kind: ExprKind::Unary {
                            op: UnaryOp::AddrOf,
                            expr: Box::new(recv),
                        },
                    });
                }
                for a in args {
                    call_args.push(self.rewrite_expr(a, env)?);
                }
                return Ok(Expr {
                    span,
                    kind: ExprKind::Call {
                        callee,
                        callee_span: method_span,
                        type_args: Vec::new(),
                        args: call_args,
                    },
                });
            }
            ExprKind::Call {
                callee,
                callee_span,
                type_args,
                args,
            } => {
                let margs = if type_args.is_empty() {
                    None
                } else {
                    Some(type_args.clone())
                };
                if let Some(margs) = margs {
                    if self.fn_templates.contains_key(&callee) {
                        let mono_name = mangle_inst(&callee, &margs);
                        ExprKind::Call {
                            callee: mono_name,
                            callee_span,
                            type_args: Vec::new(),
                            args: args
                                .into_iter()
                                .map(|a| self.rewrite_expr(a, env))
                                .collect::<Result<_, _>>()?,
                        }
                    } else {
                        ExprKind::Call {
                            callee,
                            callee_span,
                            type_args: Vec::new(),
                            args: args
                                .into_iter()
                                .map(|a| self.rewrite_expr(a, env))
                                .collect::<Result<_, _>>()?,
                        }
                    }
                } else {
                    ExprKind::Call {
                        callee,
                        callee_span,
                        type_args: Vec::new(),
                        args: args
                            .into_iter()
                            .map(|a| self.rewrite_expr(a, env))
                            .collect::<Result<_, _>>()?,
                    }
                }
            }
            ExprKind::StructLit {
                name,
                name_span,
                type_args,
                fields,
            } => {
                let mono_name = if type_args.is_empty() {
                    name
                } else {
                    mangle_inst(&name, &type_args)
                };
                ExprKind::StructLit {
                    name: mono_name,
                    name_span,
                    type_args: Vec::new(),
                    fields: fields
                        .into_iter()
                        .map(|(n, sp, e)| Ok((n, sp, self.rewrite_expr(e, env)?)))
                        .collect::<Result<_, _>>()?,
                }
            }
            ExprKind::Unary { op, expr } => ExprKind::Unary {
                op,
                expr: Box::new(self.rewrite_expr(*expr, env)?),
            },
            ExprKind::Binary { op, left, right } => ExprKind::Binary {
                op,
                left: Box::new(self.rewrite_expr(*left, env)?),
                right: Box::new(self.rewrite_expr(*right, env)?),
            },
            ExprKind::Field {
                base,
                field,
                field_span,
            } => ExprKind::Field {
                base: Box::new(self.rewrite_expr(*base, env)?),
                field,
                field_span,
            },
            ExprKind::Index { base, index } => ExprKind::Index {
                base: Box::new(self.rewrite_expr(*base, env)?),
                index: Box::new(self.rewrite_expr(*index, env)?),
            },
            other => other,
        };
        Ok(Expr { span, kind })
    }

    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        env: &HashMap<String, Type>,
    ) -> Result<(Stmt, Option<(String, Type)>), Diagnostic> {
        let mut binding = None;
        let kind = match stmt.kind {
            StmtKind::VarDecl {
                name,
                name_span,
                ty,
                own,
                init,
            } => {
                let init_e = self.rewrite_expr(init, env)?;
                let rty = resolve_apply(&ty, &self.struct_templates);
                binding = Some((
                    name.clone(),
                    infer_expr_type(&init_e, env, &self.concrete_structs).unwrap_or(rty.clone()),
                ));
                StmtKind::VarDecl {
                    name,
                    name_span,
                    ty: rty,
                    own,
                    init: init_e,
                }
            }
            StmtKind::ArrayDecl {
                name,
                name_span,
                elem,
                len,
            } => {
                binding = Some((
                    name.clone(),
                    Type::Array {
                        elem: Box::new(elem.clone()),
                        len,
                    },
                ));
                StmtKind::ArrayDecl {
                    name,
                    name_span,
                    elem: resolve_apply(&elem, &self.struct_templates),
                    len,
                }
            }
            StmtKind::Assign {
                name,
                name_span,
                value,
            } => StmtKind::Assign {
                name,
                name_span,
                value: self.rewrite_expr(value, env)?,
            },
            StmtKind::DerefAssign { ptr, value } => StmtKind::DerefAssign {
                ptr: self.rewrite_expr(ptr, env)?,
                value: self.rewrite_expr(value, env)?,
            },
            StmtKind::IndexAssign {
                base,
                base_span,
                index,
                value,
            } => StmtKind::IndexAssign {
                base,
                base_span,
                index: self.rewrite_expr(index, env)?,
                value: self.rewrite_expr(value, env)?,
            },
            StmtKind::FieldAssign {
                base,
                base_span,
                field,
                field_span,
                value,
            } => StmtKind::FieldAssign {
                base,
                base_span,
                field,
                field_span,
                value: self.rewrite_expr(value, env)?,
            },
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => StmtKind::If {
                cond: self.rewrite_expr(cond, env)?,
                then_body: self.rewrite_stmts(then_body, env)?,
                else_body: self.rewrite_stmts(else_body, env)?,
            },
            StmtKind::While { cond, body } => StmtKind::While {
                cond: self.rewrite_expr(cond, env)?,
                body: self.rewrite_stmts(body, env)?,
            },
            StmtKind::For {
                name,
                name_span,
                start,
                end,
                body,
            } => {
                let mut loop_env = env.clone();
                loop_env.insert(name.clone(), Type::Int);
                StmtKind::For {
                    name,
                    name_span,
                    start: self.rewrite_expr(start, env)?,
                    end: self.rewrite_expr(end, env)?,
                    body: self.rewrite_stmts(body, &loop_env)?,
                }
            }
            StmtKind::Print(args) => StmtKind::Print(
                args.into_iter()
                    .map(|a| self.rewrite_expr(a, env))
                    .collect::<Result<_, _>>()?,
            ),
            StmtKind::Return(e) => StmtKind::Return(
                e.map(|x| self.rewrite_expr(x, env))
                    .transpose()?,
            ),
            StmtKind::Expr(e) => StmtKind::Expr(self.rewrite_expr(e, env)?),
            other => other,
        };
        Ok((Stmt { span: stmt.span, kind }, binding))
    }
}
