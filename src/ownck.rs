//! Ownership / co-ownership checker.
//!
//! Covers Rust-style unique ownership (`own` / `move` / `&` / `&mut`) plus
//! Chinese civil-law inspired co-ownership:
//! - `share[n/d]` — 按份共有 (dispose of own share independently)
//! - `joint` — 共同共有 (undivided; whole disposition needs unanimous consent)
//! - `exclusive` — 排他
//! - `whole` — 整体

use std::collections::HashMap;

use crate::ast::*;
use crate::diag::{Diagnostic, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BorrowState {
    None,
    Shared(u32),
    Mut,
}

#[derive(Debug, Clone)]
struct Binding {
    own: OwnSpec,
    #[allow(dead_code)]
    span: Span,
    moved: bool,
    borrow: BorrowState,
    /// Static pool id for share/joint groups (approximate).
    pool: Option<u32>,
    /// Whether this joint owner has called `consent` since last clear.
    consented: bool,
}

struct Checker<'a> {
    #[allow(dead_code)]
    program: &'a Program,
    fns: HashMap<&'a str, &'a Function>,
    next_pool: u32,
    errors: Vec<Diagnostic>,
}

pub fn check(program: &Program) -> Result<(), Diagnostic> {
    let mut ck = Checker {
        program,
        fns: HashMap::new(),
        next_pool: 1,
        errors: Vec::new(),
    };
    for f in &program.functions {
        ck.fns.insert(f.name.as_str(), f);
    }
    for f in &program.functions {
        ck.check_function(f);
    }
    if let Some(err) = ck.errors.into_iter().next() {
        Err(err)
    } else {
        Ok(())
    }
}

impl<'a> Checker<'a> {
    fn err(&mut self, d: Diagnostic) {
        self.errors.push(d);
    }

    fn check_function(&mut self, func: &Function) {
        let mut env: HashMap<String, Binding> = HashMap::new();
        for p in &func.params {
            let own = match p.mode {
                ParamMode::Own => OwnSpec::own_exclusive(),
                ParamMode::Share => p.ty.own_spec(),
                ParamMode::Joint => OwnSpec {
                    kind: OwnKind::Joint,
                    exclusive: false,
                    whole: p.ty.own_spec().whole,
                },
                ParamMode::Borrow => OwnSpec::default(),
                ParamMode::BorrowMut => OwnSpec {
                    kind: OwnKind::None,
                    exclusive: true,
                    whole: false,
                },
                _ => p.ty.own_spec(),
            };
            let pool = if own.is_share() || own.is_joint() {
                let id = self.next_pool;
                self.next_pool += 1;
                Some(id)
            } else {
                None
            };
            env.insert(
                p.name.clone(),
                Binding {
                    own,
                    span: p.name_span,
                    moved: false,
                    borrow: BorrowState::None,
                    pool,
                    consented: false,
                },
            );
        }
        self.check_block(&func.body, &mut env);
    }

    fn check_block(&mut self, body: &[Stmt], env: &mut HashMap<String, Binding>) {
        for stmt in body {
            self.check_stmt(stmt, env);
            // Statement-scoped borrows (NLL-lite): expire after each statement.
            for b in env.values_mut() {
                b.borrow = BorrowState::None;
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, env: &mut HashMap<String, Binding>) {
        match &stmt.kind {
            StmtKind::VarDecl {
                name,
                name_span,
                ty,
                own,
                init,
            } => {
                let mut spec = *own;
                if matches!(spec.kind, OwnKind::None) {
                    spec = ty.own_spec();
                }
                self.check_expr_use(init, env, /*consume_own*/ spec.is_own());

                let pool = match spec.kind {
                    OwnKind::Share(_) | OwnKind::Joint => {
                        // share_peer / joint_peer inherit pool from argument
                        let inherited = self.expr_pool(init, env);
                        Some(inherited.unwrap_or_else(|| {
                            let id = self.next_pool;
                            self.next_pool += 1;
                            id
                        }))
                    }
                    _ => None,
                };

                if let Some(prev) = env.get(name) {
                    if prev.own.is_managed() && !prev.moved {
                        // redefinition drops previous — ok
                    }
                }
                env.insert(
                    name.clone(),
                    Binding {
                        own: spec,
                        span: *name_span,
                        moved: false,
                        borrow: BorrowState::None,
                        pool,
                        consented: false,
                    },
                );
            }
            StmtKind::Assign {
                name,
                name_span,
                value,
            } => {
                self.check_assign_target(name, *name_span, env);
                let consume = env
                    .get(name)
                    .map(|b| b.own.is_own())
                    .unwrap_or(false);
                self.check_expr_use(value, env, consume);
                if let Some(b) = env.get_mut(name) {
                    b.moved = false;
                    if b.own.is_joint() && b.own.whole {
                        // whole joint write requires all peers consented — approximate:
                        // require this binding consented, and clear consents after write
                        if !b.consented {
                            self.err(
                                Diagnostic::error(
                                    "whole joint write requires prior `consent` from this owner",
                                )
                                .code("E0505")
                                .label(stmt.span, "missing consent")
                                .help("call `consent(x)` before mutating a `whole joint` binding"),
                            );
                        }
                        b.consented = false;
                    }
                }
            }
            StmtKind::FieldAssign {
                base,
                base_span,
                value,
                ..
            }
            | StmtKind::IndexAssign {
                base,
                base_span,
                value,
                ..
            } => {
                self.check_assign_target(base, *base_span, env);
                self.check_expr_use(value, env, false);
            }
            StmtKind::DerefAssign { ptr, value } => {
                self.check_expr_use(ptr, env, false);
                self.check_expr_use(value, env, false);
            }
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                self.check_expr_use(cond, env, false);
                let mut then_env = env.clone();
                self.check_block(then_body, &mut then_env);
                let mut else_env = env.clone();
                self.check_block(else_body, &mut else_env);
                // Merge moved flags conservatively
                for (k, tb) in &then_env {
                    if let Some(eb) = else_env.get(k) {
                        if tb.moved || eb.moved {
                            if let Some(b) = env.get_mut(k) {
                                b.moved = true;
                            }
                        }
                    }
                }
            }
            StmtKind::While { cond, body } => {
                self.check_expr_use(cond, env, false);
                let mut loop_env = env.clone();
                self.check_block(body, &mut loop_env);
            }
            StmtKind::For {
                start,
                end,
                body,
                name,
                name_span,
                ..
            } => {
                self.check_expr_use(start, env, false);
                self.check_expr_use(end, env, false);
                let mut loop_env = env.clone();
                loop_env.insert(
                    name.clone(),
                    Binding {
                        own: OwnSpec::default(),
                        span: *name_span,
                        moved: false,
                        borrow: BorrowState::None,
                        pool: None,
                        consented: false,
                    },
                );
                self.check_block(body, &mut loop_env);
            }
            StmtKind::Print(args) => {
                for a in args {
                    self.check_expr_use(a, env, false);
                }
            }
            StmtKind::Return(Some(e)) | StmtKind::Expr(e) => {
                // Returning own moves; expr stmt `move x` consumes
                let consume = matches!(
                    &e.kind,
                    ExprKind::Unary {
                        op: UnaryOp::Move,
                        ..
                    }
                ) || matches!(&stmt.kind, StmtKind::Return(_));
                self.check_expr_use(e, env, consume && self.expr_is_own_place(e, env));
            }
            StmtKind::Return(None)
            | StmtKind::Break
            | StmtKind::Continue
            | StmtKind::ArrayDecl { .. } => {}
        }
    }

    fn check_assign_target(&mut self, name: &str, span: Span, env: &mut HashMap<String, Binding>) {
        let Some(b) = env.get_mut(name) else {
            return; // undefined handled elsewhere
        };
        if b.moved {
            self.err(
                Diagnostic::error(format!("use of moved value `{name}`"))
                    .code("E0382")
                    .label(span, "value moved here"),
            );
            return;
        }
        if matches!(b.borrow, BorrowState::Shared(_) | BorrowState::Mut) {
            self.err(
                Diagnostic::error(format!(
                    "cannot assign to `{name}` while it is borrowed"
                ))
                .code("E0506")
                .label(span, "assignment of borrowed value"),
            );
        }
        if b.own.is_share() && b.own.whole {
            // whole share mutation of the object requires full ownership of den
            if let Some(f) = b.own.fraction() {
                if f.num != f.den {
                    self.err(
                        Diagnostic::error(
                            "whole share write requires holding the entire fraction (num == den)",
                        )
                        .code("E0507")
                        .label(span, format!("holding {}/{}", f.num, f.den))
                        .help("transfer remaining shares first, or drop `whole`"),
                    );
                }
            }
        }
    }

    fn expr_is_own_place(&self, expr: &Expr, env: &HashMap<String, Binding>) -> bool {
        match &expr.kind {
            ExprKind::Ident(n) => env.get(n).is_some_and(|b| b.own.is_own()),
            ExprKind::Unary {
                op: UnaryOp::Move,
                expr,
            } => self.expr_is_own_place(expr, env),
            _ => false,
        }
    }

    fn expr_pool(&self, expr: &Expr, env: &HashMap<String, Binding>) -> Option<u32> {
        match &expr.kind {
            ExprKind::Ident(n) => env.get(n).and_then(|b| b.pool),
            ExprKind::Call { callee, args, .. }
                if matches!(callee.as_str(), "share_peer" | "joint_peer") =>
            {
                args.first().and_then(|a| self.expr_pool(a, env))
            }
            ExprKind::Call { callee, .. }
                if matches!(callee.as_str(), "share_new" | "joint_new") =>
            {
                None // new pool assigned by caller
            }
            _ => None,
        }
    }

    fn check_expr_use(
        &mut self,
        expr: &Expr,
        env: &mut HashMap<String, Binding>,
        consume_own: bool,
    ) {
        match &expr.kind {
            ExprKind::Ident(name) => {
                if let Some(b) = env.get_mut(name) {
                    if b.moved {
                        self.err(
                            Diagnostic::error(format!("use of moved value `{name}`"))
                                .code("E0382")
                                .label(expr.span, "value moved here"),
                        );
                        return;
                    }
                    if consume_own && b.own.is_own() {
                        if matches!(b.borrow, BorrowState::Shared(_) | BorrowState::Mut) {
                            self.err(
                                Diagnostic::error(format!(
                                    "cannot move `{name}` while borrowed"
                                ))
                                .code("E0505")
                                .label(expr.span, "move of borrowed value"),
                            );
                        }
                        b.moved = true;
                        b.borrow = BorrowState::None;
                    }
                }
            }
            ExprKind::Unary {
                op: UnaryOp::Move,
                expr: inner,
            } => {
                if let ExprKind::Ident(name) = &inner.kind {
                    if let Some(b) = env.get_mut(name) {
                        if !b.own.is_own() && b.own.is_managed() {
                            self.err(
                                Diagnostic::error("`move` only applies to `own` bindings")
                                    .code("E0508")
                                    .label(expr.span, "not uniquely owned")
                                    .help(
                                        "use share transfer for `share`, or `consent`+assign for `joint`",
                                    ),
                            );
                        }
                        if b.moved {
                            self.err(
                                Diagnostic::error(format!("use of moved value `{name}`"))
                                    .code("E0382")
                                    .label(expr.span, "already moved"),
                            );
                        } else if matches!(b.borrow, BorrowState::Shared(_) | BorrowState::Mut) {
                            self.err(
                                Diagnostic::error(format!(
                                    "cannot move `{name}` while borrowed"
                                ))
                                .code("E0505")
                                .label(expr.span, "move of borrowed value"),
                            );
                        } else {
                            b.moved = true;
                        }
                    }
                } else {
                    self.check_expr_use(inner, env, true);
                }
            }
            ExprKind::Unary {
                op: UnaryOp::AddrOf,
                expr: inner,
            } => {
                if let ExprKind::Ident(name) = &inner.kind {
                    if let Some(b) = env.get_mut(name) {
                        if b.moved {
                            self.err(
                                Diagnostic::error(format!("borrow of moved value `{name}`"))
                                    .code("E0382")
                                    .label(expr.span, "value moved"),
                            );
                        } else if matches!(b.borrow, BorrowState::Mut) {
                            self.err(
                                Diagnostic::error(format!(
                                    "cannot borrow `{name}` as shared while mutably borrowed"
                                ))
                                .code("E0502")
                                .label(expr.span, "shared borrow"),
                            );
                        } else if b.own.exclusive && matches!(b.borrow, BorrowState::None) {
                            // exclusive own allows shared borrows (Rust rules)
                            b.borrow = BorrowState::Shared(1);
                        } else {
                            match b.borrow {
                                BorrowState::None => b.borrow = BorrowState::Shared(1),
                                BorrowState::Shared(n) => b.borrow = BorrowState::Shared(n + 1),
                                BorrowState::Mut => {}
                            }
                        }
                    }
                } else {
                    self.check_expr_use(inner, env, false);
                }
            }
            ExprKind::Unary {
                op: UnaryOp::AddrOfMut,
                expr: inner,
            } => {
                if let ExprKind::Ident(name) = &inner.kind {
                    if let Some(b) = env.get_mut(name) {
                        if b.moved {
                            self.err(
                                Diagnostic::error(format!("borrow of moved value `{name}`"))
                                    .code("E0382")
                                    .label(expr.span, "value moved"),
                            );
                        } else if !matches!(b.borrow, BorrowState::None) {
                            self.err(
                                Diagnostic::error(format!(
                                    "cannot borrow `{name}` as mutable because it is already borrowed"
                                ))
                                .code("E0499")
                                .label(expr.span, "mutable borrow"),
                            );
                        } else {
                            b.borrow = BorrowState::Mut;
                        }
                    }
                } else {
                    self.check_expr_use(inner, env, false);
                }
            }
            ExprKind::Unary {
                op: UnaryOp::Deref,
                expr: inner,
            } => self.check_expr_use(inner, env, false),
            ExprKind::Binary { left, right, .. } => {
                self.check_expr_use(left, env, false);
                self.check_expr_use(right, env, false);
            }
            ExprKind::Call {
                callee,
                args,
                callee_span,
                ..
            } => {
                match callee.as_str() {
                    "consent" => {
                        for a in args {
                            if let ExprKind::Ident(name) = &a.kind {
                                if let Some(b) = env.get_mut(name) {
                                    if !b.own.is_joint() {
                                        self.err(
                                            Diagnostic::error(
                                                "`consent` requires a `joint` binding",
                                            )
                                            .code("E0509")
                                            .label(a.span, "not joint"),
                                        );
                                    } else {
                                        b.consented = true;
                                    }
                                }
                            } else {
                                self.err(
                                    Diagnostic::error("`consent` expects a joint variable")
                                        .label(a.span, "expected ident"),
                                );
                            }
                        }
                        return;
                    }
                    "drop" => {
                        for a in args {
                            if let ExprKind::Ident(name) = &a.kind {
                                if let Some(b) = env.get_mut(name) {
                                    if b.own.is_own() {
                                        b.moved = true;
                                        b.borrow = BorrowState::None;
                                    } else if b.own.is_share() {
                                        // dropping a share releases that fraction
                                        b.moved = true;
                                    } else if b.own.is_joint() {
                                        if b.own.whole && !b.consented {
                                            self.err(
                                                Diagnostic::error(
                                                    "dropping `whole joint` requires `consent`",
                                                )
                                                .code("E0505")
                                                .label(a.span, "missing consent"),
                                            );
                                        }
                                        b.moved = true;
                                    }
                                }
                            } else {
                                self.check_expr_use(a, env, true);
                            }
                        }
                        return;
                    }
                    "share_new" | "joint_new" | "share_peer" | "joint_peer" => {
                        for a in args {
                            self.check_expr_use(a, env, callee == "share_new" || callee == "joint_new");
                        }
                        return;
                    }
                    _ => {}
                }

                if let Some(func) = self.fns.get(callee.as_str()).copied() {
                    for (i, arg) in args.iter().enumerate() {
                        let consume = func
                            .params
                            .get(i)
                            .is_some_and(|p| matches!(p.mode, ParamMode::Own | ParamMode::Value)
                                && p.ty.own_spec().is_own());
                        // Also consume when passing own place to own param
                        let consume = consume
                            || func.params.get(i).is_some_and(|p| p.mode == ParamMode::Own);
                        match func.params.get(i).map(|p| p.mode) {
                            Some(ParamMode::Borrow) => {
                                // treat as &arg
                                if let ExprKind::Ident(name) = &arg.kind {
                                    self.check_expr_use(
                                        &Expr {
                                            span: arg.span,
                                            kind: ExprKind::Unary {
                                                op: UnaryOp::AddrOf,
                                                expr: Box::new(arg.clone()),
                                            },
                                        },
                                        env,
                                        false,
                                    );
                                    let _ = name;
                                } else {
                                    self.check_expr_use(arg, env, false);
                                }
                            }
                            Some(ParamMode::BorrowMut) | Some(ParamMode::Ref) | Some(ParamMode::Out) => {
                                if let ExprKind::Ident(_) = &arg.kind {
                                    self.check_expr_use(
                                        &Expr {
                                            span: arg.span,
                                            kind: ExprKind::Unary {
                                                op: UnaryOp::AddrOfMut,
                                                expr: Box::new(arg.clone()),
                                            },
                                        },
                                        env,
                                        false,
                                    );
                                } else {
                                    self.check_expr_use(arg, env, false);
                                }
                            }
                            Some(ParamMode::Share) => {
                                // Passing share does not consume peer shares; moves this share binding
                                if let ExprKind::Ident(name) = &arg.kind {
                                    if let Some(b) = env.get_mut(name) {
                                        if b.own.is_share() {
                                            b.moved = true;
                                        }
                                    }
                                }
                                self.check_expr_use(arg, env, false);
                            }
                            Some(ParamMode::Joint) => {
                                // Joint pass shares the group — no move of peers
                                self.check_expr_use(arg, env, false);
                            }
                            _ => self.check_expr_use(arg, env, consume),
                        }
                    }
                } else {
                    // unknown / builtin
                    for a in args {
                        self.check_expr_use(a, env, false);
                    }
                    let _ = callee_span;
                }
            }
            ExprKind::MethodCall {
                receiver, args, ..
            } => {
                self.check_expr_use(receiver, env, false);
                for a in args {
                    self.check_expr_use(a, env, false);
                }
            }
            ExprKind::StructLit { fields, .. } => {
                for (_, _, e) in fields {
                    self.check_expr_use(e, env, false);
                }
            }
            ExprKind::Field { base, .. } => self.check_expr_use(base, env, false),
            ExprKind::Index { base, index } => {
                self.check_expr_use(base, env, false);
                self.check_expr_use(index, env, false);
            }
            ExprKind::IntLit(_)
            | ExprKind::FloatLit(_)
            | ExprKind::StrLit(_)
            | ExprKind::BoolLit(_) => {}
        }
    }
}
