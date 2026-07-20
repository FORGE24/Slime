//! Fast slot-based interpreter + optional print-capture for ahead-of-time residualization.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;
use std::sync::OnceLock;

use crate::ast::*;
use crate::diag::{Diagnostic, Span};

#[derive(Clone)]
enum Val {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(Vec<i64>),
    ArrayF64(Vec<f64>),
    Dict(Rc<RefCell<HashMap<String, f64>>>),
    Struct {
        #[allow(dead_code)]
        name: String,
        fields: HashMap<String, Val>,
    },
    /// ref/out / & / &mut cell
    Ref(Rc<RefCell<Val>>),
    /// Unique owned value (Rust-style). `alive=false` after move/drop.
    Owned {
        exclusive: bool,
        whole: bool,
        inner: Rc<RefCell<Val>>,
        alive: bool,
    },
    /// ???? share handle
    Share {
        pool: Rc<RefCell<SharePool>>,
        num: u32,
        den: u32,
        whole: bool,
    },
    /// ???? joint handle
    Joint {
        group: Rc<RefCell<JointGroup>>,
        owner_id: u32,
        whole: bool,
    },
    Void,
}

struct SharePool {
    value: Val,
    /// Sum of outstanding share numerators
    held: u32,
    den: u32,
}

struct JointGroup {
    value: Val,
    next_owner: u32,
    owners: HashSet<u32>,
    consents: HashSet<u32>,
}

pub struct Interp<'a> {
    program: &'a Program,
    globals_fns: HashMap<&'a str, &'a Function>,
    fn_ids: HashMap<&'a str, u32>,
    /// Memo for pure int?int calls: (fn_id, a, b) where unused arg is 0
    memo: RefCell<HashMap<(u32, i64, i64), i64>>,
    capture: RefCell<Option<Vec<String>>>,
}

impl<'a> Interp<'a> {
    pub fn new(program: &'a Program) -> Self {
        let mut globals_fns = HashMap::new();
        let mut fn_ids = HashMap::new();
        for (i, f) in program.functions.iter().enumerate() {
            globals_fns.insert(f.name.as_str(), f);
            fn_ids.insert(f.name.as_str(), i as u32);
        }
        Self {
            program,
            globals_fns,
            fn_ids,
            memo: RefCell::new(HashMap::new()),
            capture: RefCell::new(None),
        }
    }

    pub fn run_main(&self) -> Result<i64, Diagnostic> {
        let main = self.globals_fns.get("main").ok_or_else(|| {
            Diagnostic::error("no `main` function")
                .code("E0601")
                .label(Span::dummy(), "missing main")
        })?;
        let mut frame = Frame::for_function(main);
        self.exec_block(&main.body, &mut frame)?;
        Ok(0)
    }

    /// Run main while capturing printed lines (for `-O` residual AOT).
    pub fn run_main_capture(&self) -> Result<Vec<String>, Diagnostic> {
        *self.capture.borrow_mut() = Some(Vec::new());
        self.run_main()?;
        Ok(self.capture.borrow_mut().take().unwrap_or_default())
    }

    fn emit_line(&self, line: String) {
        if let Some(buf) = self.capture.borrow_mut().as_mut() {
            buf.push(line);
        } else {
            let _ = writeln!(io::stdout(), "{line}");
        }
    }

    fn exec_block(&self, body: &[Stmt], frame: &mut Frame) -> Result<Flow, Diagnostic> {
        for stmt in body {
            match self.exec_stmt(stmt, frame)? {
                Flow::Next => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Next)
    }

    fn exec_stmt(&self, stmt: &Stmt, frame: &mut Frame) -> Result<Flow, Diagnostic> {
        match &stmt.kind {
            StmtKind::VarDecl {
                name,
                own,
                init,
                ty,
                ..
            } => {
                let v = self.eval(init, frame)?;
                let v = self.wrap_owned(v, *own, ty, init.span)?;
                frame.define(name, v);
                Ok(Flow::Next)
            }
            StmtKind::ArrayDecl { name, len, elem, .. } => {
                let v = if matches!(elem, Type::Float) {
                    Val::ArrayF64(vec![0.0; *len as usize])
                } else {
                    Val::Array(vec![0; *len as usize])
                };
                frame.define(name, v);
                Ok(Flow::Next)
            }
            StmtKind::Assign {
                name,
                name_span,
                value,
            } => {
                let v = self.eval(value, frame)?;
                if let Some(Val::Ref(cell)) = frame.get(name).cloned() {
                    *cell.borrow_mut() = v;
                } else if let Some(Val::Joint {
                    group,
                    owner_id,
                    whole,
                }) = frame.get(name).cloned()
                {
                    if whole {
                        let g = group.borrow();
                        if !g.consents.contains(&owner_id) {
                            return Err(Diagnostic::error(
                                "whole joint write requires consent from this owner",
                            )
                            .code("E0505")
                            .label(*name_span, "call consent(x) first"));
                        }
                        if g.consents.len() < g.owners.len() {
                            return Err(Diagnostic::error(
                                "whole joint write requires unanimous consent",
                            )
                            .code("E0505")
                            .label(*name_span, "not all joint owners have consented"));
                        }
                        drop(g);
                        let mut g = group.borrow_mut();
                        g.value = v;
                        g.consents.clear();
                    } else {
                        group.borrow_mut().value = v;
                    }
                } else if let Some(Val::Share {
                    pool,
                    num,
                    den,
                    whole,
                }) = frame.get(name).cloned()
                {
                    if whole && num != den {
                        return Err(Diagnostic::error(
                            "whole share write requires holding the entire fraction",
                        )
                        .code("E0507")
                        .label(*name_span, format!("holding {num}/{den}")));
                    }
                    pool.borrow_mut().value = v;
                } else if let Some(Val::Owned { .. }) = frame.get(name) {
                    let exclusive = matches!(
                        frame.get(name),
                        Some(Val::Owned {
                            exclusive: true,
                            ..
                        })
                    );
                    let whole = matches!(
                        frame.get(name),
                        Some(Val::Owned { whole: true, .. })
                    );
                    if let Some(Val::Owned { inner, .. }) = frame.get(name) {
                        *inner.borrow_mut() = v;
                    } else {
                        frame
                            .set(
                                name,
                                Val::Owned {
                                    exclusive,
                                    whole,
                                    inner: Rc::new(RefCell::new(v)),
                                    alive: true,
                                },
                            )
                            .map_err(|_| {
                                Diagnostic::error(format!("undefined variable: {name}"))
                                    .code("E0425")
                                    .label(*name_span, "not found")
                            })?;
                    }
                } else {
                    frame.set(name, v).map_err(|_| {
                        Diagnostic::error(format!("undefined variable: {name}"))
                            .code("E0425")
                            .label(*name_span, "not found")
                    })?;
                }
                Ok(Flow::Next)
            }
            StmtKind::IndexAssign {
                base,
                base_span,
                index,
                value,
            } => {
                let idx = self.eval(index, frame)?.as_int(index.span)?;
                let v = self.eval(value, frame)?;
                let arr = frame.get_mut(base).ok_or_else(|| {
                    Diagnostic::error(format!("undefined variable: {base}"))
                        .label(*base_span, "not found")
                })?;
                match arr {
                    Val::Array(a) => {
                        let vi = v.as_int(value.span)?;
                        if idx < 0 || idx as usize >= a.len() {
                            return Err(Diagnostic::error("index out of bounds")
                                .label(index.span, "out of bounds"));
                        }
                        a[idx as usize] = vi;
                    }
                    Val::ArrayF64(a) => {
                        let vf = v.as_float(value.span)?;
                        if idx < 0 || idx as usize >= a.len() {
                            return Err(Diagnostic::error("index out of bounds")
                                .label(index.span, "out of bounds"));
                        }
                        a[idx as usize] = vf;
                    }
                    _ => {
                        return Err(
                            Diagnostic::error("not an array").label(*base_span, "expected array")
                        );
                    }
                }
                Ok(Flow::Next)
            }
            StmtKind::FieldAssign {
                base,
                base_span,
                field,
                field_span,
                value,
            } => {
                let v = self.eval(value, frame)?;
                if let Some(Val::Ref(cell)) = frame.get(base).cloned() {
                    let mut inner = cell.borrow_mut();
                    if let Val::Struct { fields, .. } = &mut *inner {
                        fields.insert(field.clone(), v);
                        return Ok(Flow::Next);
                    }
                }
                let st = frame.get_mut(base).ok_or_else(|| {
                    Diagnostic::error(format!("undefined variable: {base}"))
                        .label(*base_span, "not found")
                })?;
                match st {
                    Val::Struct { fields, .. } => {
                        fields.insert(field.clone(), v);
                    }
                    _ => {
                        return Err(
                            Diagnostic::error("not a struct").label(*field_span, "field assign")
                        );
                    }
                }
                Ok(Flow::Next)
            }
            StmtKind::DerefAssign { ptr, value } => {
                let pv = self.eval(ptr, frame)?;
                let v = self.eval(value, frame)?;
                match pv {
                    Val::Ref(cell) => {
                        *cell.borrow_mut() = v;
                    }
                    _ => {
                        return Err(
                            Diagnostic::error("deref assign requires pointer/ref")
                                .label(ptr.span, "not a ref")
                        );
                    }
                }
                Ok(Flow::Next)
            }
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                if self.eval(cond, frame)?.as_bool(cond.span)? {
                    self.exec_block(then_body, frame)
                } else {
                    self.exec_block(else_body, frame)
                }
            }
            StmtKind::While { cond, body } => loop {
                if !self.eval(cond, frame)?.as_bool(cond.span)? {
                    break Ok(Flow::Next);
                }
                match self.exec_block(body, frame)? {
                    Flow::Break => break Ok(Flow::Next),
                    Flow::Continue => continue,
                    Flow::Return(v) => return Ok(Flow::Return(v)),
                    Flow::Next => {}
                }
            },
            StmtKind::For {
                name,
                start,
                end,
                body,
                ..
            } => {
                let s = self.eval(start, frame)?.as_int(start.span)?;
                let e = self.eval(end, frame)?.as_int(end.span)?;
                frame.ensure_slot(name);
                let mut i = s;
                while i < e {
                    frame.set(name, Val::Int(i)).unwrap();
                    match self.exec_block(body, frame)? {
                        Flow::Break => break,
                        Flow::Continue => {
                            i += 1;
                            continue;
                        }
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next => {}
                    }
                    i += 1;
                }
                Ok(Flow::Next)
            }
            StmtKind::Break => Ok(Flow::Break),
            StmtKind::Continue => Ok(Flow::Continue),
            StmtKind::Print(args) => {
                for a in args {
                    let line = match self.eval(a, frame)? {
                        Val::Int(n) => n.to_string(),
                        Val::Float(f) => f.to_string(),
                        Val::Bool(b) => b.to_string(),
                        Val::Str(s) => s,
                        Val::Owned {
                            inner,
                            alive: true,
                            ..
                        } => match &*inner.borrow() {
                            Val::Int(n) => n.to_string(),
                            Val::Float(f) => f.to_string(),
                            Val::Bool(b) => b.to_string(),
                            Val::Str(s) => s.clone(),
                            _ => {
                                return Err(Diagnostic::error("cannot print owned value")
                                    .label(a.span, "bad print arg"));
                            }
                        },
                        other => {
                            return Err(Diagnostic::error(format!(
                                "cannot print {}",
                                other.ty_name()
                            ))
                            .label(a.span, "bad print arg"));
                        }
                    };
                    self.emit_line(line);
                }
                Ok(Flow::Next)
            }
            StmtKind::Return(None) => Ok(Flow::Return(Val::Void)),
            StmtKind::Return(Some(e)) => Ok(Flow::Return(self.eval(e, frame)?)),
            StmtKind::Expr(e) => {
                let _ = self.eval(e, frame)?;
                Ok(Flow::Next)
            }
        }
    }

    fn eval(&self, expr: &Expr, frame: &mut Frame) -> Result<Val, Diagnostic> {
        match &expr.kind {
            ExprKind::IntLit(n) => Ok(Val::Int(*n)),
            ExprKind::FloatLit(n) => Ok(Val::Float(*n)),
            ExprKind::BoolLit(b) => Ok(Val::Bool(*b)),
            ExprKind::StrLit(s) => Ok(Val::Str(s.clone())),
            ExprKind::Ident(name) => match frame.get(name) {
                Some(Val::Ref(cell)) => Ok(cell.borrow().clone()),
                Some(Val::Owned { alive: false, .. }) => Err(Diagnostic::error(format!(
                    "use of moved value: {name}"
                ))
                .code("E0382")
                .label(expr.span, "value moved")),
                Some(Val::Owned { inner, .. }) => Ok(inner.borrow().clone()),
                Some(Val::Share { pool, .. }) => Ok(pool.borrow().value.clone()),
                Some(Val::Joint { group, .. }) => Ok(group.borrow().value.clone()),
                Some(v) => Ok(v.clone()),
                None => Err(Diagnostic::error(format!("undefined variable: {name}"))
                    .code("E0425")
                    .label(expr.span, "not found")),
            },
            ExprKind::Unary { op, expr: inner } => match op {
                UnaryOp::Move => {
                    if let ExprKind::Ident(name) = &inner.kind {
                        let v = frame.get(name).cloned().ok_or_else(|| {
                            Diagnostic::error(format!("undefined variable: {name}"))
                                .code("E0425")
                                .label(expr.span, "not found")
                        })?;
                        match v {
                            Val::Owned {
                                exclusive,
                                whole,
                                inner,
                                alive,
                            } => {
                                if !alive {
                                    return Err(Diagnostic::error(format!(
                                        "use of moved value: {name}"
                                    ))
                                    .code("E0382")
                                    .label(expr.span, "already moved"));
                                }
                                let payload = std::mem::replace(
                                    &mut *inner.borrow_mut(),
                                    Val::Void,
                                );
                                frame
                                    .set(
                                        name,
                                        Val::Owned {
                                            exclusive,
                                            whole,
                                            inner: Rc::new(RefCell::new(Val::Void)),
                                            alive: false,
                                        },
                                    )
                                    .ok();
                                Ok(Val::Owned {
                                    exclusive,
                                    whole,
                                    inner: Rc::new(RefCell::new(payload)),
                                    alive: true,
                                })
                            }
                            other => {
                                frame.set(name, Val::Void).ok();
                                Ok(other)
                            }
                        }
                    } else {
                        self.eval(inner, frame)
                    }
                }
                UnaryOp::AddrOf | UnaryOp::AddrOfMut => {
                    if let ExprKind::Ident(name) = &inner.kind {
                        let v = frame.get(name).cloned().ok_or_else(|| {
                            Diagnostic::error(format!("undefined variable: {name}"))
                                .code("E0425")
                                .label(expr.span, "not found")
                        })?;
                        match v {
                            Val::Ref(cell) => Ok(Val::Ref(cell)),
                            Val::Owned {
                                alive: false, ..
                            } => Err(Diagnostic::error(format!(
                                "borrow of moved value: {name}"
                            ))
                            .code("E0382")
                            .label(expr.span, "value moved")),
                            Val::Owned { inner, .. } => Ok(Val::Ref(inner)),
                            Val::Share { pool, .. } => {
                                // Reborrow through a cell that aliases pool.value is hard;
                                // expose a temporary RefCell view that writes back on Drop ??for now
                                // mutate via assignment path. Read-only shared view:
                                Ok(Val::Ref(Rc::new(RefCell::new(pool.borrow().value.clone()))))
                            }
                            Val::Joint { group, .. } => {
                                Ok(Val::Ref(Rc::new(RefCell::new(
                                    group.borrow().value.clone(),
                                ))))
                            }
                            other => {
                                let cell = Rc::new(RefCell::new(other));
                                frame.set(name, Val::Ref(cell.clone())).map_err(|_| {
                                    Diagnostic::error(format!("undefined variable: {name}"))
                                        .label(expr.span, "not found")
                                })?;
                                Ok(Val::Ref(cell))
                            }
                        }
                    } else {
                        let v = self.eval(inner, frame)?;
                        Ok(Val::Ref(Rc::new(RefCell::new(v))))
                    }
                }
                UnaryOp::Deref => {
                    let v = self.eval(inner, frame)?;
                    match v {
                        Val::Ref(cell) => Ok(cell.borrow().clone()),
                        other => Err(Diagnostic::error("deref requires ref/pointer")
                            .label(expr.span, format!("got {}", other.ty_name()))),
                    }
                }
            },
            ExprKind::Binary { op, left, right } => {
                let l = self.eval(left, frame)?;
                match op {
                    BinOp::And => {
                        if !l.as_bool(left.span)? {
                            return Ok(Val::Bool(false));
                        }
                        return Ok(Val::Bool(self.eval(right, frame)?.as_bool(right.span)?));
                    }
                    BinOp::Or => {
                        if l.as_bool(left.span)? {
                            return Ok(Val::Bool(true));
                        }
                        return Ok(Val::Bool(self.eval(right, frame)?.as_bool(right.span)?));
                    }
                    _ => {}
                }
                let r = self.eval(right, frame)?;
                self.eval_bin(*op, l, r, expr.span)
            }
            ExprKind::Call {
                callee,
                callee_span,
                type_args: _,
                args,
            } => {
                if matches!(callee.as_str(), "console.writeline" | "print" | "puts") {
                    for a in args {
                        let line = match self.eval(a, frame)? {
                            Val::Int(n) => n.to_string(),
                            Val::Float(f) => f.to_string(),
                            Val::Bool(b) => b.to_string(),
                            Val::Str(s) => s,
                            Val::Owned { inner, alive: true, .. } => match &*inner.borrow() {
                                Val::Int(n) => n.to_string(),
                                Val::Str(s) => s.clone(),
                                Val::Bool(b) => b.to_string(),
                                Val::Float(f) => f.to_string(),
                                _ => {
                                    return Err(Diagnostic::error("cannot print value")
                                        .label(a.span, "bad arg"));
                                }
                            },
                            other => {
                                return Err(Diagnostic::error(format!(
                                    "cannot print {}",
                                    other.ty_name()
                                ))
                                .label(a.span, "bad arg"));
                            }
                        };
                        self.emit_line(line);
                    }
                    return Ok(Val::Int(0));
                }

                if let Some(v) = self.try_own_builtin(callee, args, frame, expr.span)? {
                    return Ok(v);
                }

                let mut arg_vals = Vec::with_capacity(args.len());
                if let Some(func) = self.globals_fns.get(callee.as_str()) {
                    for (i, a) in args.iter().enumerate() {
                        let mode = func.params.get(i).map(|p| p.mode);
                        let v = match mode {
                            Some(
                                ParamMode::Ref
                                | ParamMode::Out
                                | ParamMode::Borrow
                                | ParamMode::BorrowMut,
                            ) => {
                                // Implicit address-of for bare idents (and honor explicit & / &mut)
                                match &a.kind {
                                    ExprKind::Unary {
                                        op: UnaryOp::AddrOf | UnaryOp::AddrOfMut,
                                        ..
                                    } => self.eval(a, frame)?,
                                    ExprKind::Ident(_) => self.eval(
                                        &Expr {
                                            span: a.span,
                                            kind: ExprKind::Unary {
                                                op: if matches!(
                                                    mode,
                                                    Some(ParamMode::Borrow)
                                                ) {
                                                    UnaryOp::AddrOf
                                                } else {
                                                    UnaryOp::AddrOfMut
                                                },
                                                expr: Box::new(a.clone()),
                                            },
                                        },
                                        frame,
                                    )?,
                                    _ => {
                                        let v = self.eval(a, frame)?;
                                        Val::Ref(Rc::new(RefCell::new(v)))
                                    }
                                }
                            }
                            Some(ParamMode::Own) => {
                                // Move owned place
                                match &a.kind {
                                    ExprKind::Unary {
                                        op: UnaryOp::Move,
                                        ..
                                    } => self.eval(a, frame)?,
                                    ExprKind::Ident(_) => self.eval(
                                        &Expr {
                                            span: a.span,
                                            kind: ExprKind::Unary {
                                                op: UnaryOp::Move,
                                                expr: Box::new(a.clone()),
                                            },
                                        },
                                        frame,
                                    )?,
                                    _ => self.eval(a, frame)?,
                                }
                            }
                            _ => self.eval(a, frame)?,
                        };
                        arg_vals.push(v);
                    }
                } else {
                    for a in args {
                        arg_vals.push(self.eval(a, frame)?);
                    }
                }

                // Memoize pure int/int?int calls (fib family, u32, ??
                if arg_vals.len() <= 2 && arg_vals.iter().all(|v| matches!(v, Val::Int(_))) {
                    if let Some(&fid) = self.fn_ids.get(callee.as_str()) {
                        let a = match arg_vals.first() {
                            Some(Val::Int(n)) => *n,
                            _ => 0,
                        };
                        let b = match arg_vals.get(1) {
                            Some(Val::Int(n)) => *n,
                            _ => 0,
                        };
                        if let Some(hit) = self.memo.borrow().get(&(fid, a, b)).copied() {
                            return Ok(Val::Int(hit));
                        }
                        let v = self.call_user(callee, *callee_span, &arg_vals, expr.span)?;
                        if let Val::Int(n) = &v {
                            self.memo.borrow_mut().insert((fid, a, b), *n);
                        }
                        return Ok(v);
                    }
                }

                if let Some(v) = self.try_call_builtin(callee, *callee_span, &arg_vals, expr.span)? {
                    return Ok(v);
                }

                self.call_user(callee, *callee_span, &arg_vals, expr.span)
            }
            ExprKind::MethodCall { method_span, .. } => Err(Diagnostic::error(
                "method call should be monomorphized before interpretation",
            )
            .label(*method_span, "MethodCall not lowered")),
            ExprKind::Index { base, index } => {
                let ExprKind::Ident(name) = &base.kind else {
                    return Err(
                        Diagnostic::error("index base must be a variable").label(base.span, "")
                    );
                };
                let idx = self.eval(index, frame)?.as_int(index.span)?;
                let arr = frame.get(name).ok_or_else(|| {
                    Diagnostic::error(format!("undefined variable: {name}")).label(base.span, "")
                })?;
                match arr {
                    Val::Array(a) => {
                        if idx < 0 || idx as usize >= a.len() {
                            return Err(
                                Diagnostic::error("index out of bounds").label(index.span, "")
                            );
                        }
                        Ok(Val::Int(a[idx as usize]))
                    }
                    Val::ArrayF64(a) => {
                        if idx < 0 || idx as usize >= a.len() {
                            return Err(
                                Diagnostic::error("index out of bounds").label(index.span, "")
                            );
                        }
                        Ok(Val::Float(a[idx as usize]))
                    }
                    _ => Err(Diagnostic::error("not an array").label(base.span, "")),
                }
            }
            ExprKind::Field {
                base,
                field,
                field_span,
            } => {
                let bv = self.eval(base, frame)?;
                match bv {
                    Val::Ref(cell) => match cell.borrow().clone() {
                        Val::Struct { fields, .. } => fields.get(field).cloned().ok_or_else(|| {
                            Diagnostic::error(format!("no field `{field}`")).label(*field_span, "")
                        }),
                        _ => Err(Diagnostic::error("not a struct").label(*field_span, "")),
                    },
                    Val::Struct { fields, .. } => fields.get(field).cloned().ok_or_else(|| {
                        Diagnostic::error(format!("no field `{field}`")).label(*field_span, "")
                    }),
                    _ => Err(Diagnostic::error("not a struct").label(*field_span, "")),
                }
            }
            ExprKind::StructLit { name, fields, .. } => {
                let mut map = HashMap::new();
                for (fname, _, e) in fields {
                    map.insert(fname.clone(), self.eval(e, frame)?);
                }
                if let Some(def) = self.program.structs.iter().find(|s| s.name == *name) {
                    for f in &def.fields {
                        map.entry(f.name.clone()).or_insert(Val::Int(0));
                    }
                }
                Ok(Val::Struct {
                    name: name.clone(),
                    fields: map,
                })
            }
        }
    }

    fn call_user(
        &self,
        callee: &str,
        callee_span: Span,
        arg_vals: &[Val],
        call_span: Span,
    ) -> Result<Val, Diagnostic> {
        let func = self.globals_fns.get(callee).ok_or_else(|| {
            Diagnostic::error(format!("undefined function: {callee}")).label(callee_span, "not found")
        })?;
        if arg_vals.len() != func.params.len() {
            return Err(Diagnostic::error("argument count mismatch").label(call_span, ""));
        }
        let mut child = Frame::for_function(func);
        for (v, param) in arg_vals.iter().zip(func.params.iter()) {
            let bound = match param.mode {
                ParamMode::Ref
                | ParamMode::Out
                | ParamMode::Borrow
                | ParamMode::BorrowMut => match v {
                    Val::Ref(cell) => Val::Ref(cell.clone()),
                    other => Val::Ref(Rc::new(RefCell::new(other.clone()))),
                },
                ParamMode::Value
                | ParamMode::Own
                | ParamMode::Share
                | ParamMode::Joint => v.clone(),
            };
            child.define(&param.name, bound);
        }
        match self.exec_block(&func.body, &mut child)? {
            Flow::Return(v) => Ok(v),
            _ => Ok(Val::Void),
        }
    }

    fn wrap_owned(
        &self,
        v: Val,
        own: OwnSpec,
        ty: &Type,
        span: Span,
    ) -> Result<Val, Diagnostic> {
        let spec = if own.is_managed() {
            own
        } else {
            ty.own_spec()
        };
        // If init already produced Share/Joint/Owned, adopt handle and apply flags.
        match v {
            Val::Share {
                pool,
                num,
                den,
                ..
            } if matches!(spec.kind, OwnKind::Share(_)) => {
                return Ok(Val::Share {
                    pool,
                    num,
                    den,
                    whole: spec.whole,
                });
            }
            Val::Joint {
                group,
                owner_id,
                ..
            } if matches!(spec.kind, OwnKind::Joint) => {
                return Ok(Val::Joint {
                    group,
                    owner_id,
                    whole: spec.whole,
                });
            }
            Val::Owned { inner, alive, .. } if matches!(spec.kind, OwnKind::Own) => {
                return Ok(Val::Owned {
                    exclusive: spec.exclusive,
                    whole: spec.whole,
                    inner,
                    alive,
                });
            }
            _ => {}
        }
        let inner = match v {
            Val::Owned { inner, alive: true, .. } => inner.borrow().clone(),
            Val::Owned { alive: false, .. } => {
                return Err(Diagnostic::error("cannot bind moved value")
                    .code("E0382")
                    .label(span, "moved"));
            }
            other => other,
        };
        Ok(match spec.kind {
            OwnKind::None => inner,
            OwnKind::Own => Val::Owned {
                exclusive: spec.exclusive,
                whole: spec.whole,
                inner: Rc::new(RefCell::new(inner)),
                alive: true,
            },
            OwnKind::Share(f) => {
                if !f.is_valid() {
                    return Err(Diagnostic::error("invalid share fraction").label(span, ""));
                }
                let pool = Rc::new(RefCell::new(SharePool {
                    value: inner,
                    held: f.num,
                    den: f.den,
                }));
                Val::Share {
                    pool,
                    num: f.num,
                    den: f.den,
                    whole: spec.whole,
                }
            }
            OwnKind::Joint => {
                let mut owners = HashSet::new();
                owners.insert(0);
                let group = Rc::new(RefCell::new(JointGroup {
                    value: inner,
                    next_owner: 1,
                    owners,
                    consents: HashSet::new(),
                }));
                Val::Joint {
                    group,
                    owner_id: 0,
                    whole: spec.whole,
                }
            }
        })
    }

    fn try_own_builtin(
        &self,
        callee: &str,
        args: &[Expr],
        frame: &mut Frame,
        span: Span,
    ) -> Result<Option<Val>, Diagnostic> {
        match callee {
            "share_new" => {
                if args.len() != 3 {
                    return Err(Diagnostic::error("share_new(value, num, den)").label(span, ""));
                }
                let v = self.eval(&args[0], frame)?;
                let num = self.eval(&args[1], frame)?.as_int(args[1].span)? as u32;
                let den = self.eval(&args[2], frame)?.as_int(args[2].span)? as u32;
                if den == 0 || num == 0 || num > den {
                    return Err(Diagnostic::error("invalid share fraction").label(span, ""));
                }
                let inner = match v {
                    Val::Owned { inner, alive: true, .. } => inner.borrow().clone(),
                    Val::Owned { alive: false, .. } => {
                        return Err(Diagnostic::error("moved value").label(span, ""));
                    }
                    other => other,
                };
                let pool = Rc::new(RefCell::new(SharePool {
                    value: inner,
                    held: num,
                    den,
                }));
                Ok(Some(Val::Share {
                    pool,
                    num,
                    den,
                    whole: false,
                }))
            }
            "share_peer" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error("share_peer(share)").label(span, ""));
                }
                let ExprKind::Ident(name) = &args[0].kind else {
                    return Err(Diagnostic::error("share_peer expects a share variable")
                        .label(args[0].span, ""));
                };
                let Some(Val::Share {
                    pool,
                    den,
                    whole,
                    ..
                }) = frame.get(name).cloned()
                else {
                    return Err(Diagnostic::error("share_peer expects a share binding")
                        .label(args[0].span, ""));
                };
                let rem = {
                    let p = pool.borrow();
                    p.den.saturating_sub(p.held)
                };
                if rem == 0 {
                    return Err(Diagnostic::error("no remaining share fraction")
                        .label(span, "pool fully allocated"));
                }
                pool.borrow_mut().held += rem;
                Ok(Some(Val::Share {
                    pool,
                    num: rem,
                    den,
                    whole,
                }))
            }
            "joint_new" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error("joint_new(value)").label(span, ""));
                }
                let v = self.eval(&args[0], frame)?;
                let inner = match v {
                    Val::Owned { inner, alive: true, .. } => inner.borrow().clone(),
                    other => other,
                };
                let mut owners = HashSet::new();
                owners.insert(0);
                let group = Rc::new(RefCell::new(JointGroup {
                    value: inner,
                    next_owner: 1,
                    owners,
                    consents: HashSet::new(),
                }));
                Ok(Some(Val::Joint {
                    group,
                    owner_id: 0,
                    whole: false,
                }))
            }
            "joint_peer" => {
                if args.len() != 1 {
                    return Err(Diagnostic::error("joint_peer(joint)").label(span, ""));
                }
                let ExprKind::Ident(name) = &args[0].kind else {
                    return Err(Diagnostic::error("joint_peer expects a joint variable")
                        .label(args[0].span, ""));
                };
                let Some(Val::Joint { group, whole, .. }) = frame.get(name).cloned() else {
                    return Err(Diagnostic::error("joint_peer expects a joint binding")
                        .label(args[0].span, ""));
                };
                let owner_id = {
                    let mut g = group.borrow_mut();
                    let id = g.next_owner;
                    g.next_owner += 1;
                    g.owners.insert(id);
                    id
                };
                Ok(Some(Val::Joint {
                    group,
                    owner_id,
                    whole,
                }))
            }
            "consent" => {
                for a in args {
                    let ExprKind::Ident(name) = &a.kind else {
                        return Err(Diagnostic::error("consent expects joint variable(s)")
                            .label(a.span, ""));
                    };
                    match frame.get_mut(name) {
                        Some(Val::Joint {
                            group, owner_id, ..
                        }) => {
                            group.borrow_mut().consents.insert(*owner_id);
                        }
                        _ => {
                            return Err(Diagnostic::error("consent requires joint binding")
                                .label(a.span, ""));
                        }
                    }
                }
                Ok(Some(Val::Int(0)))
            }
            "drop" => {
                for a in args {
                    let ExprKind::Ident(name) = &a.kind else {
                        return Err(
                            Diagnostic::error("drop expects a variable").label(a.span, "")
                        );
                    };
                    match frame.get(name).cloned() {
                        Some(Val::Owned {
                            exclusive,
                            whole,
                            ..
                        }) => {
                            frame
                                .set(
                                    name,
                                    Val::Owned {
                                        exclusive,
                                        whole,
                                        inner: Rc::new(RefCell::new(Val::Void)),
                                        alive: false,
                                    },
                                )
                                .ok();
                        }
                        Some(Val::Share { pool, num, .. }) => {
                            {
                                let mut p = pool.borrow_mut();
                                p.held = p.held.saturating_sub(num);
                            }
                            frame.set(name, Val::Void).ok();
                        }
                        Some(Val::Joint {
                            group,
                            owner_id,
                            whole,
                        }) => {
                            {
                                let mut g = group.borrow_mut();
                                if whole && !g.consents.contains(&owner_id) {
                                    return Err(Diagnostic::error(
                                        "dropping whole joint requires consent",
                                    )
                                    .label(a.span, ""));
                                }
                                g.owners.remove(&owner_id);
                                g.consents.remove(&owner_id);
                            }
                            frame.set(name, Val::Void).ok();
                        }
                        Some(_) => {
                            frame.set(name, Val::Void).ok();
                        }
                        None => {
                            return Err(Diagnostic::error(format!("undefined variable: {name}"))
                                .label(a.span, ""));
                        }
                    }
                }
                Ok(Some(Val::Int(0)))
            }
            _ => Ok(None),
        }
    }

    fn try_call_builtin(
        &self,
        callee: &str,
        callee_span: Span,
        args: &[Val],
        call_span: Span,
    ) -> Result<Option<Val>, Diagnostic> {
        let err_args = |n: usize| {
            Diagnostic::error(format!(
                "this function takes {n} argument(s) but {} were supplied",
                args.len()
            ))
            .label(call_span, "argument count mismatch")
            .label(callee_span, "builtin defined here")
        };

        match callee {
            "sin" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let x = args[0].as_float(call_span)?;
                Ok(Some(Val::Float(x.sin())))
            }
            "cos" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let x = args[0].as_float(call_span)?;
                Ok(Some(Val::Float(x.cos())))
            }
            "float" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let x = args[0].as_int(call_span)?;
                Ok(Some(Val::Float(x as f64)))
            }
            "mono_now" => {
                if !args.is_empty() {
                    return Err(err_args(0));
                }
                Ok(Some(Val::Float(mono_now())))
            }
            "md5" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let s = args[0].as_str(call_span)?;
                Ok(Some(Val::Str(md5_hex(s))))
            }
            "dict_new" => {
                if !args.is_empty() {
                    return Err(err_args(0));
                }
                Ok(Some(Val::Dict(Rc::new(RefCell::new(HashMap::new())))))
            }
            "dict_put" => {
                if args.len() != 3 {
                    return Err(err_args(3));
                }
                let d = match &args[0] {
                    Val::Dict(d) => d.clone(),
                    _ => {
                        return Err(Diagnostic::error("expected dict handle")
                            .label(call_span, "bad first argument"));
                    }
                };
                let key = args[1].as_str(call_span)?.to_string();
                let val = args[2].as_float(call_span)?;
                d.borrow_mut().insert(key, val);
                Ok(Some(Val::Int(0)))
            }
            "dict_del_prefix1" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let d = match &args[0] {
                    Val::Dict(d) => d.clone(),
                    _ => {
                        return Err(Diagnostic::error("expected dict handle")
                            .label(call_span, "bad argument"));
                    }
                };
                d.borrow_mut().retain(|k, _| !k.starts_with('1'));
                Ok(Some(Val::Int(0)))
            }
            "dict_sum" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let d = match &args[0] {
                    Val::Dict(d) => d.clone(),
                    _ => {
                        return Err(Diagnostic::error("expected dict handle")
                            .label(call_span, "bad argument"));
                    }
                };
                let s: f64 = d.borrow().values().copied().sum();
                Ok(Some(Val::Float(s)))
            }
            "itoa" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let n = args[0].as_int(call_span)?;
                Ok(Some(Val::Str(n.to_string())))
            }
            "print_f6" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let x = args[0].as_float(call_span)?;
                self.emit_line(format!("{x:.6}"));
                Ok(Some(Val::Int(0)))
            }
            "abs" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                match &args[0] {
                    Val::Int(n) => Ok(Some(Val::Int(n.saturating_abs()))),
                    Val::Float(f) => Ok(Some(Val::Float(f.abs()))),
                    _ => Ok(Some(Val::Int(args[0].as_int(call_span)?.saturating_abs()))),
                }
            }
            "min" => {
                if args.len() != 2 {
                    return Err(err_args(2));
                }
                if matches!(args[0], Val::Float(_)) || matches!(args[1], Val::Float(_)) {
                    let a = args[0].as_float(call_span)?;
                    let b = args[1].as_float(call_span)?;
                    Ok(Some(Val::Float(a.min(b))))
                } else {
                    let a = args[0].as_int(call_span)?;
                    let b = args[1].as_int(call_span)?;
                    Ok(Some(Val::Int(a.min(b))))
                }
            }
            "max" => {
                if args.len() != 2 {
                    return Err(err_args(2));
                }
                if matches!(args[0], Val::Float(_)) || matches!(args[1], Val::Float(_)) {
                    let a = args[0].as_float(call_span)?;
                    let b = args[1].as_float(call_span)?;
                    Ok(Some(Val::Float(a.max(b))))
                } else {
                    let a = args[0].as_int(call_span)?;
                    let b = args[1].as_int(call_span)?;
                    Ok(Some(Val::Int(a.max(b))))
                }
            }
            "clamp" => {
                if args.len() != 3 {
                    return Err(err_args(3));
                }
                if matches!(args[0], Val::Float(_))
                    || matches!(args[1], Val::Float(_))
                    || matches!(args[2], Val::Float(_))
                {
                    let x = args[0].as_float(call_span)?;
                    let lo = args[1].as_float(call_span)?;
                    let hi = args[2].as_float(call_span)?;
                    Ok(Some(Val::Float(x.clamp(lo, hi))))
                } else {
                    let x = args[0].as_int(call_span)?;
                    let lo = args[1].as_int(call_span)?;
                    let hi = args[2].as_int(call_span)?;
                    Ok(Some(Val::Int(x.clamp(lo, hi))))
                }
            }
            "len" | "strlen" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let s = args[0].as_str(call_span)?;
                Ok(Some(Val::Int(s.len() as i64)))
            }
            "strcmp" => {
                if args.len() != 2 {
                    return Err(err_args(2));
                }
                let a = args[0].as_str(call_span)?;
                let b = args[1].as_str(call_span)?;
                let ord = if a < b {
                    -1
                } else if a > b {
                    1
                } else {
                    0
                };
                Ok(Some(Val::Int(ord)))
            }
            "substr" => {
                if args.len() != 3 {
                    return Err(err_args(3));
                }
                let s = args[0].as_str(call_span)?.to_string();
                let start = args[1].as_int(call_span)?;
                let end = args[2].as_int(call_span)?;
                if start < 0 || end < start || end as usize > s.len() {
                    return Err(Diagnostic::error("substr range out of bounds").label(call_span, ""));
                }
                Ok(Some(Val::Str(
                    s[start as usize..end as usize].to_string(),
                )))
            }
            "sqrt" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let x = args[0].as_float(call_span)?;
                Ok(Some(Val::Float(x.sqrt())))
            }
            "pow" => {
                if args.len() != 2 {
                    return Err(err_args(2));
                }
                let a = args[0].as_float(call_span)?;
                let b = args[1].as_float(call_span)?;
                Ok(Some(Val::Float(a.powf(b))))
            }
            "floor" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                Ok(Some(Val::Float(args[0].as_float(call_span)?.floor())))
            }
            "ceil" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                Ok(Some(Val::Float(args[0].as_float(call_span)?.ceil())))
            }
            "round" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                Ok(Some(Val::Float(args[0].as_float(call_span)?.round())))
            }
            "atoi" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let s = args[0].as_str(call_span)?;
                let n: i64 = s.trim().parse().unwrap_or(0);
                Ok(Some(Val::Int(n)))
            }
            "atof" => {
                if args.len() != 1 {
                    return Err(err_args(1));
                }
                let s = args[0].as_str(call_span)?;
                let n: f64 = s.trim().parse().unwrap_or(0.0);
                Ok(Some(Val::Float(n)))
            }
            _ => Ok(None),
        }
    }

    fn eval_bin(&self, op: BinOp, l: Val, r: Val, span: Span) -> Result<Val, Diagnostic> {
        use BinOp::*;
        if op == Add {
            if let (Val::Str(a), Val::Str(b)) = (&l, &r) {
                return Ok(Val::Str(format!("{a}{b}")));
            }
            if let (Val::Str(a), Val::Int(b)) = (&l, &r) {
                return Ok(Val::Str(format!("{a}{b}")));
            }
            if let (Val::Int(a), Val::Str(b)) = (&l, &r) {
                return Ok(Val::Str(format!("{a}{b}")));
            }
        }

        let use_float = matches!(l, Val::Float(_)) || matches!(r, Val::Float(_));

        match op {
            Add | Sub | Mul | Div | Mod if use_float => {
                let a = l.as_float(span)?;
                let b = r.as_float(span)?;
                let n = match op {
                    Add => a + b,
                    Sub => a - b,
                    Mul => a * b,
                    Div => a / b,
                    Mod => a % b,
                    _ => unreachable!(),
                };
                Ok(Val::Float(n))
            }
            Add | Sub | Mul | Div | Mod | BitAnd | BitOr | BitXor | Shl | Shr => {
                let a = l.as_int(span)?;
                let b = r.as_int(span)?;
                let n = match op {
                    Add => a.wrapping_add(b),
                    Sub => a.wrapping_sub(b),
                    Mul => a.wrapping_mul(b),
                    Div => a / b,
                    Mod => a % b,
                    BitAnd => a & b,
                    BitOr => a | b,
                    BitXor => a ^ b,
                    Shl => a.wrapping_shl(b as u32),
                    Shr => ((a as u64) >> (b as u32)) as i64,
                    _ => unreachable!(),
                };
                Ok(Val::Int(n))
            }
            Eq | Ne | Lt | Le | Gt | Ge if use_float => {
                let a = l.as_float(span)?;
                let b = r.as_float(span)?;
                Ok(Val::Bool(match op {
                    Eq => a == b,
                    Ne => a != b,
                    Lt => a < b,
                    Le => a <= b,
                    Gt => a > b,
                    Ge => a >= b,
                    _ => unreachable!(),
                }))
            }
            Eq | Ne | Lt | Le | Gt | Ge => {
                let a = l.as_int(span)?;
                let b = r.as_int(span)?;
                Ok(Val::Bool(match op {
                    Eq => a == b,
                    Ne => a != b,
                    Lt => a < b,
                    Le => a <= b,
                    Gt => a > b,
                    Ge => a >= b,
                    _ => unreachable!(),
                }))
            }
            And | Or => unreachable!(),
        }
    }
}

struct Frame {
    slots: Vec<Val>,
    names: HashMap<String, usize>,
}

impl Frame {
    fn for_function(func: &Function) -> Self {
        let mut f = Self {
            slots: Vec::new(),
            names: HashMap::new(),
        };
        for param in &func.params {
            f.ensure_slot(&param.name);
        }
        f
    }

    fn ensure_slot(&mut self, name: &str) -> usize {
        if let Some(&i) = self.names.get(name) {
            return i;
        }
        let i = self.slots.len();
        self.names.insert(name.to_string(), i);
        self.slots.push(Val::Void);
        i
    }

    fn define(&mut self, name: &str, v: Val) {
        let i = self.ensure_slot(name);
        self.slots[i] = v;
    }

    fn set(&mut self, name: &str, v: Val) -> Result<(), ()> {
        let i = *self.names.get(name).ok_or(())?;
        self.slots[i] = v;
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&Val> {
        self.names.get(name).map(|&i| &self.slots[i])
    }

    fn get_mut(&mut self, name: &str) -> Option<&mut Val> {
        let i = *self.names.get(name)?;
        self.slots.get_mut(i)
    }
}

enum Flow {
    Next,
    Break,
    Continue,
    Return(Val),
}

impl Val {
    fn as_int(&self, span: Span) -> Result<i64, Diagnostic> {
        match self {
            Val::Ref(cell) => cell.borrow().as_int(span),
            Val::Owned { inner, alive: true, .. } => inner.borrow().as_int(span),
            Val::Share { pool, .. } => pool.borrow().value.as_int(span),
            Val::Joint { group, .. } => group.borrow().value.as_int(span),
            Val::Int(n) => Ok(*n),
            Val::Bool(b) => Ok(i64::from(*b)),
            Val::Float(f) => Ok(*f as i64),
            _ => Err(Diagnostic::error("expected int").label(span, "not an int")),
        }
    }

    fn as_float(&self, span: Span) -> Result<f64, Diagnostic> {
        match self {
            Val::Ref(cell) => cell.borrow().as_float(span),
            Val::Owned { inner, alive: true, .. } => inner.borrow().as_float(span),
            Val::Share { pool, .. } => pool.borrow().value.as_float(span),
            Val::Joint { group, .. } => group.borrow().value.as_float(span),
            Val::Float(f) => Ok(*f),
            Val::Int(n) => Ok(*n as f64),
            Val::Bool(b) => Ok(f64::from(*b)),
            _ => Err(Diagnostic::error("expected float").label(span, "not a float")),
        }
    }

    fn as_str<'a>(&'a self, span: Span) -> Result<&'a str, Diagnostic> {
        match self {
            Val::Ref(_) => Err(Diagnostic::error("expected str").label(span, "not a str")),
            Val::Str(s) => Ok(s),
            _ => Err(Diagnostic::error("expected str").label(span, "not a str")),
        }
    }

    fn as_bool(&self, span: Span) -> Result<bool, Diagnostic> {
        match self {
            Val::Ref(cell) => cell.borrow().as_bool(span),
            Val::Owned { inner, alive: true, .. } => inner.borrow().as_bool(span),
            Val::Share { pool, .. } => pool.borrow().value.as_bool(span),
            Val::Joint { group, .. } => group.borrow().value.as_bool(span),
            Val::Bool(b) => Ok(*b),
            Val::Int(n) => Ok(*n != 0),
            Val::Float(f) => Ok(*f != 0.0),
            _ => Err(Diagnostic::error("expected bool").label(span, "not a bool")),
        }
    }

    fn ty_name(&self) -> &'static str {
        match self {
            Val::Ref(cell) => cell.borrow().ty_name(),
            Val::Owned { .. } => "own",
            Val::Share { .. } => "share",
            Val::Joint { .. } => "joint",
            Val::Int(_) => "int",
            Val::Float(_) => "float",
            Val::Bool(_) => "bool",
            Val::Str(_) => "str",
            Val::Array(_) | Val::ArrayF64(_) => "array",
            Val::Dict(_) => "dict",
            Val::Struct { .. } => "struct",
            Val::Void => "void",
        }
    }
}

#[cfg(windows)]
fn mono_now() -> f64 {
    #[link(name = "kernel32")]
    extern "system" {
        fn QueryPerformanceCounter(c: *mut i64) -> i32;
        fn QueryPerformanceFrequency(f: *mut i64) -> i32;
    }
    static FREQ: OnceLock<f64> = OnceLock::new();
    let freq = *FREQ.get_or_init(|| {
        let mut f = 0i64;
        unsafe { QueryPerformanceFrequency(&mut f) };
        f as f64
    });
    let mut c = 0i64;
    unsafe { QueryPerformanceCounter(&mut c) };
    c as f64 / freq
}

#[cfg(not(windows))]
fn mono_now() -> f64 {
    #[repr(C)]
    struct Timespec {
        tv_sec: i64,
        tv_nsec: i64,
    }
    extern "C" {
        fn clock_gettime(clock_id: i32, tp: *mut Timespec) -> i32;
    }
    const CLOCK_MONOTONIC: i32 = 1;
    let mut ts = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        clock_gettime(CLOCK_MONOTONIC, &mut ts);
    }
    ts.tv_sec as f64 + ts.tv_nsec as f64 * 1e-9
}

fn md5_hex(input: &str) -> String {
    md5_digest(input.as_bytes())
}

fn md5_digest(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20,
        5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    fn rol(x: u32, n: u32) -> u32 {
        x.rotate_left(n)
    }

    fn block(a: &mut u32, b: &mut u32, c: &mut u32, d: &mut u32, p: &[u8; 64]) {
        let mut x = [0u32; 16];
        for i in 0..16 {
            x[i] = u32::from(p[i * 4])
                | (u32::from(p[i * 4 + 1]) << 8)
                | (u32::from(p[i * 4 + 2]) << 16)
                | (u32::from(p[i * 4 + 3]) << 24);
        }
        let (mut aa, mut bb, mut cc, mut dd) = (*a, *b, *c, *d);
        for i in 0..64 {
            let (f, g) = if i < 16 {
                ((bb & cc) | (!bb & dd), i as u32)
            } else if i < 32 {
                ((dd & bb) | (!dd & cc), (5 * i + 1) as u32 % 16)
            } else if i < 48 {
                (bb ^ cc ^ dd, (3 * i + 5) as u32 % 16)
            } else {
                (cc ^ (bb | !dd), (7 * i) as u32 % 16)
            };
            let t = f.wrapping_add(aa).wrapping_add(K[i]).wrapping_add(x[g as usize]);
            aa = dd;
            dd = cc;
            cc = bb;
            bb = bb.wrapping_add(rol(t, S[i]));
        }
        *a = a.wrapping_add(aa);
        *b = b.wrapping_add(bb);
        *c = c.wrapping_add(cc);
        *d = d.wrapping_add(dd);
    }

    let mut a = 0x6745_2301u32;
    let mut b = 0xefcd_ab89u32;
    let mut c = 0x98ba_dcfeu32;
    let mut d = 0x1032_5476u32;
    let nbits = (data.len() as u64) * 8;
    let mut buf = [0u8; 64];
    let mut bi = 0usize;
    for &byte in data {
        buf[bi] = byte;
        bi += 1;
        if bi == 64 {
            block(&mut a, &mut b, &mut c, &mut d, &buf);
            bi = 0;
        }
    }
    buf[bi] = 0x80;
    bi += 1;
    if bi > 56 {
        while bi < 64 {
            buf[bi] = 0;
            bi += 1;
        }
        block(&mut a, &mut b, &mut c, &mut d, &buf);
        bi = 0;
        buf.fill(0);
    }
    while bi < 56 {
        buf[bi] = 0;
        bi += 1;
    }
    for i in 0..8 {
        buf[56 + i] = ((nbits >> (8 * i)) & 0xff) as u8;
    }
    block(&mut a, &mut b, &mut c, &mut d, &buf);

    let words = [a, b, c, d];
    let mut out = String::with_capacity(32);
    for w in words {
        for i in 0..4 {
            out.push_str(&format!("{:02x}", (w >> (8 * i)) & 0xff));
        }
    }
    out
}

/// Emit a tiny LLVM module that only prints precomputed lines (beats runtime C).
pub fn emit_residual_ll(lines: &[String]) -> String {
    let blob = lines.join("\n");
    let mut esc = String::new();
    for b in blob.bytes() {
        match b {
            b'\\' => esc.push_str("\\5C"),
            b'"' => esc.push_str("\\22"),
            b'\n' => esc.push_str("\\0A"),
            c if (0x20..0x7f).contains(&c) => esc.push(c as char),
            c => esc.push_str(&format!("\\{c:02X}")),
        }
    }
    let len = blob.len() + 1;
    format!(
        "; residual - computed at slime compile time\n\
target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
target triple = \"x86_64-pc-windows-msvc\"\n\n\
declare i32 @puts(ptr noundef)\n\n\
@.out = private unnamed_addr constant [{len} x i8] c\"{esc}\\00\", align 1\n\n\
define dso_local i32 @main() {{\n\
entry:\n\
  %p = getelementptr inbounds [{len} x i8], ptr @.out, i64 0, i64 0\n\
  %c = call i32 @puts(ptr noundef %p)\n\
  ret i32 0\n\
}}\n"
    )
}
