//! ETCA orchestration for Slime2 — engines talk **LLVM IR**, not AST rewrites.
//!
//! Inherited from [FORGE24/Slime](https://github.com/FORGE24/Slime):
//! CTFE · Dynamic Precomp · DOPE · TCE · IFM · Scheduler Elimination · Pre-Concurrency
//!
//! Pipeline:
//!   parse(AST read-only) → codegen(+ETCA) → LLVM IR

use std::collections::{HashMap, HashSet};

use crate::ast::{
    BinOp as AstBinOp, Expr, ExprKind, Function, Program, Stmt, StmtKind,
};
use crate::ctfe::{BinOp as CtfeBinOp, CtfeEngine, CtfeOp, CtfeValue};
use crate::dope::{self, DopeEngine, Expression as DopeExpr, Value as DopeValue};
use crate::dynamic_precomp::{DynamicPrecomputer, RuntimeValue};
use crate::ifm::{IfmEngine, InstructionSignature, Operand};
use crate::pre_concurrency::{
    Branch, BranchState, Computation, ConcurrentKind, ConcurrentPoint, PreConcurrencyEngine,
};
use crate::scheduler_elimination::{
    EliminationStrategy, SchedulerEliminationEngine, Task, TaskInput, TaskKind, TaskOutput,
    TaskState,
};
use crate::tce::{CollapseStrategy, TceEngine};

#[derive(Debug, Clone, Copy)]
pub struct EtcaOpts {
    pub ctfe: bool,
    pub dope: bool,
    pub precomp: bool,
    pub tce: bool,
    pub ifm: bool,
    pub scheduler: bool,
    pub pre_concurrency: bool,
    pub report: bool,
}

impl Default for EtcaOpts {
    fn default() -> Self {
        Self {
            ctfe: true,
            dope: true,
            precomp: true,
            tce: true,
            ifm: true,
            scheduler: true,
            pre_concurrency: true,
            report: false,
        }
    }
}

impl EtcaOpts {
    pub fn any_enabled(self) -> bool {
        self.ctfe
            || self.dope
            || self.precomp
            || self.tce
            || self.ifm
            || self.scheduler
            || self.pre_concurrency
    }

    pub fn disable_all(&mut self) {
        self.ctfe = false;
        self.dope = false;
        self.precomp = false;
        self.tce = false;
        self.ifm = false;
        self.scheduler = false;
        self.pre_concurrency = false;
    }
}

pub struct EtcaBundle {
    pub opts: EtcaOpts,
    pub ctfe: CtfeEngine,
    pub dope: DopeEngine,
    pub precomp: DynamicPrecomputer,
    pub tce: TceEngine,
    pub ifm: IfmEngine,
    pub scheduler: SchedulerEliminationEngine,
    pub pre_concurrency: PreConcurrencyEngine,
    pub llvm_folds: usize,
    pub pre_concurrency_llvm_folds: usize,
    exec_point: usize,
    task_seq: usize,
    pre_conc_seq: usize,
}

impl EtcaBundle {
    pub fn new(opts: EtcaOpts) -> Self {
        Self {
            opts,
            ctfe: CtfeEngine::new(),
            dope: DopeEngine::new(),
            precomp: DynamicPrecomputer::new(),
            tce: TceEngine::new(CollapseStrategy::Balanced),
            ifm: IfmEngine::new(),
            scheduler: SchedulerEliminationEngine::new(EliminationStrategy::Balanced),
            pre_concurrency: PreConcurrencyEngine::new(),
            llvm_folds: 0,
            pre_concurrency_llvm_folds: 0,
            exec_point: 0,
            task_seq: 0,
            pre_conc_seq: 0,
        }
    }

    pub fn tick(&mut self) {
        self.exec_point = self.exec_point.wrapping_add(1);
    }

    /// Fold independent constant branches before any thread would spawn.
    /// Slime1 emits NASM in FoldedConcurrency.code; Slime2 uses results as LLVM consts.
    pub fn fold_pre_concurrency_constants(
        &mut self,
        tag: &str,
        values: &[i64],
    ) -> Option<Vec<i64>> {
        if !self.opts.pre_concurrency || values.is_empty() {
            return None;
        }
        self.pre_conc_seq += 1;
        let id = format!("{tag}_{}", self.pre_conc_seq);
        let branches: Vec<Branch> = values
            .iter()
            .enumerate()
            .map(|(i, v)| Branch {
                id: format!("{id}_b{i}"),
                computation: Computation::Constant { value: *v },
                dependencies: vec![],
                state: BranchState::Pending,
            })
            .collect();
        let n = branches.len();
        self.pre_concurrency.register_concurrent_point(ConcurrentPoint {
            id: id.clone(),
            kind: ConcurrentKind::DataParallel { chunks: n },
            branches,
            can_fold: false,
        });
        if !self.pre_concurrency.analyze_concurrent_point(&id) {
            return None;
        }
        let folded = self.pre_concurrency.fold_concurrent_point(&id)?;
        self.pre_concurrency_llvm_folds += folded.results.len();
        Some(folded.results)
    }

    pub fn note_const_control_fold(&mut self, taken: bool, live_consts: &[i64]) {
        if !self.opts.pre_concurrency {
            return;
        }
        let tag = if taken {
            "const_if_then"
        } else {
            "const_if_else"
        };
        let _ = self.fold_pre_concurrency_constants(tag, live_consts);
    }

    /// Slime1 `register_functions_to_ctfe`: lower AST fn bodies to `CtfeOp`,
    /// skip recursive / non-lowerable functions, then CTFE can fold calls.
    pub fn register_functions_from_program(&mut self, program: &Program) {
        if !self.opts.ctfe {
            return;
        }

        let by_name: HashMap<&str, &Function> = program
            .functions
            .iter()
            .map(|f| (f.name.as_str(), f))
            .collect();

        let mut recursive = HashSet::new();
        for name in by_name.keys() {
            let mut visited = HashSet::new();
            if is_recursive_function(*name, &by_name, &mut visited) {
                recursive.insert((*name).to_string());
            }
        }

        for func in &program.functions {
            if func.name == "main" || recursive.contains(&func.name) {
                continue;
            }
            let params: Vec<String> = func.params.iter().map(|p| p.name.clone()).collect();
            let body: Option<Vec<CtfeOp>> = func
                .body
                .iter()
                .map(stmt_to_ctfe_op)
                .collect();
            let Some(body) = body else {
                continue;
            };
            if body.len() != func.body.len() {
                continue;
            }
            self.ctfe
                .register_function(func.name.clone(), params, body);
        }
    }

    pub fn try_eval(&mut self, expr: &Expr) -> Option<CtfeValue> {
        if self.opts.ifm {
            if let Some(sig) = expr_to_ifm_sig(expr) {
                for _ in 0..4 {
                    if let Some(entry) =
                        self.ifm.execute_instruction(sig.clone(), self.exec_point)
                    {
                        self.tick();
                        return Some(CtfeValue::Int(entry.output));
                    }
                    self.tick();
                }
            }
        }

        if !self.opts.ctfe && !self.opts.dope {
            return None;
        }

        if self.opts.dope {
            let id = format!("e@{}", expr.span.start);
            let dop = ast_to_dope_expr(expr)?;
            if !self.dope.analyze_determinism(id, &dop) {
                return None;
            }
        }

        // Function folding requires CTFE registration (CTFE2).
        if matches!(expr.kind, ExprKind::Call { .. }) && !self.opts.ctfe {
            return None;
        }

        let op = expr_to_ctfe_op(expr)?;
        match self.ctfe.execute(&op) {
            Ok(CtfeValue::RuntimeDegraded(_)) => None,
            Ok(v) => {
                if self.opts.ifm {
                    if let (Some(sig), CtfeValue::Int(_)) = (expr_to_ifm_sig(expr), &v) {
                        for _ in 0..4 {
                            let _ = self.ifm.execute_instruction(sig.clone(), self.exec_point);
                            self.tick();
                        }
                    }
                }
                if self.opts.scheduler {
                    if let CtfeValue::Int(n) = &v {
                        self.record_scheduler_fold(expr, *n);
                    }
                }
                if self.opts.pre_concurrency {
                    if let CtfeValue::Int(n) = &v {
                        let _ = self.fold_pre_concurrency_constants("expr", &[*n]);
                    }
                }
                Some(v)
            }
            Err(_) => None,
        }
    }

    fn record_scheduler_fold(&mut self, expr: &Expr, value: i64) {
        self.task_seq += 1;
        let id = format!("t{}", self.task_seq);
        let kind = match &expr.kind {
            ExprKind::IntLit(_) | ExprKind::BoolLit(_) => TaskKind::Constant { value },
            ExprKind::Binary { .. } => TaskKind::Computation {
                expr: format!("fold@{value}"),
            },
            _ => TaskKind::Constant { value },
        };
        self.scheduler.add_task(Task {
            id,
            kind,
            inputs: Vec::<TaskInput>::new(),
            output: Some(TaskOutput {
                value: Some(value),
            }),
            state: TaskState::Pending,
        });
    }

    pub fn finalize_scheduler(&mut self) {
        if self.opts.scheduler {
            self.scheduler.eliminate_tasks();
        }
    }

    pub fn bind_const(&mut self, name: &str, val: &CtfeValue) {
        let op = CtfeOp::Declare(
            name.to_string(),
            Box::new(CtfeOp::LoadConst(val.clone())),
        );
        let _ = self.ctfe.execute(&op);

        if self.opts.precomp {
            if let Some(rv) = ctfe_to_runtime(val) {
                self.precomp
                    .track_convergence(name, rv, self.exec_point);
            }
        }

        if self.opts.tce {
            if let Some(lit) = ctfe_to_llvm_literal(val) {
                self.tce.collapse_to_llvm_const(
                    name,
                    lit,
                    format!("CTFE bind `{name}`"),
                );
            }
        }

        if self.opts.scheduler {
            if let CtfeValue::Int(n) = val {
                self.task_seq += 1;
                let id = format!("bind_{name}_{}", self.task_seq);
                self.scheduler.add_task(Task {
                    id,
                    kind: TaskKind::Constant { value: *n },
                    inputs: vec![],
                    output: Some(TaskOutput {
                        value: Some(*n),
                    }),
                    state: TaskState::Pending,
                });
            }
        }

        if self.opts.pre_concurrency {
            if let CtfeValue::Int(n) = val {
                let _ = self.fold_pre_concurrency_constants(&format!("bind_{name}"), &[*n]);
            }
        }
    }

    /// Drop compile-time binding when a local is mutated with a non-foldable value.
    pub fn invalidate_const(&mut self, name: &str) {
        self.ctfe.clear_const(name);
        self.tce.clear_compile_const(name);
    }

    /// Execute a `while` fully in the CTFE VM; return residual bindings on success.
    /// Falls back to `None` when the loop (or any stmt) is not lowerable / not closed.
    pub fn try_ctfe_while(
        &mut self,
        cond: &Expr,
        body: &[Stmt],
    ) -> Option<Vec<(String, CtfeValue)>> {
        if !self.opts.ctfe {
            return None;
        }
        let cond_op = expr_to_ctfe_op(cond)?;
        let body_ops: Vec<CtfeOp> = body.iter().map(stmt_to_ctfe_op).collect::<Option<_>>()?;
        let loop_op = CtfeOp::Loop {
            init: Box::new(CtfeOp::LoadConst(CtfeValue::Int(0))),
            cond: Box::new(cond_op),
            update: Box::new(CtfeOp::LoadConst(CtfeValue::Int(0))),
            body: body_ops,
        };
        match self.ctfe.execute(&loop_op) {
            Ok(CtfeValue::RuntimeDegraded(_)) | Err(_) => None,
            Ok(_) => {
                let mut names = HashSet::new();
                collect_assigned_names(body, &mut names);
                let mut out = Vec::new();
                for name in names {
                    if let Some(v) = self.ctfe.get_const(&name).cloned() {
                        out.push((name, v));
                    }
                }
                self.llvm_folds += 1;
                Some(out)
            }
        }
    }

    /// Execute `for name in start..end` fully in the CTFE VM.
    pub fn try_ctfe_for(
        &mut self,
        name: &str,
        start: &Expr,
        end: &Expr,
        body: &[Stmt],
    ) -> Option<Vec<(String, CtfeValue)>> {
        if !self.opts.ctfe {
            return None;
        }
        let start_op = expr_to_ctfe_op(start)?;
        let end_op = expr_to_ctfe_op(end)?;
        let body_ops: Vec<CtfeOp> = body.iter().map(stmt_to_ctfe_op).collect::<Option<_>>()?;
        // name = start
        let init = CtfeOp::Declare(name.to_string(), Box::new(start_op));
        let cond = CtfeOp::BinOp(
            CtfeBinOp::Lt,
            Box::new(CtfeOp::Load(name.to_string())),
            Box::new(end_op),
        );
        let update = CtfeOp::Declare(
            name.to_string(),
            Box::new(CtfeOp::BinOp(
                CtfeBinOp::Add,
                Box::new(CtfeOp::Load(name.to_string())),
                Box::new(CtfeOp::LoadConst(CtfeValue::Int(1))),
            )),
        );
        let loop_op = CtfeOp::Loop {
            init: Box::new(init),
            cond: Box::new(cond),
            update: Box::new(update),
            body: body_ops,
        };
        match self.ctfe.execute(&loop_op) {
            Ok(CtfeValue::RuntimeDegraded(_)) | Err(_) => None,
            Ok(_) => {
                let mut names = HashSet::new();
                names.insert(name.to_string());
                collect_assigned_names(body, &mut names);
                let mut out = Vec::new();
                for n in names {
                    if let Some(v) = self.ctfe.get_const(&n).cloned() {
                        out.push((n, v));
                    }
                }
                self.llvm_folds += 1;
                Some(out)
            }
        }
    }

    pub fn note_runtime(&mut self, reason: &str) {
        if self.opts.tce {
            self.tce.mark_runtime(reason);
        }
    }

    pub fn emit_summary(&self) {
        let pc = self.pre_concurrency.get_stats();
        eprintln!(
            "ETCA/LLVM: folds={} | CTFE={} | DOPE={} | Precomp={} | TCE={} | IFM={:.0}% | Sched={} | PreConc fold={}/avoid_spawn={}",
            self.llvm_folds,
            self.ctfe.get_stats().ctfe_exprs,
            self.dope.get_stats().deterministic_exprs,
            self.precomp.get_stats().converged_values,
            self.tce.get_stats().collapsed_to_compile,
            self.ifm.get_hit_rate() * 100.0,
            self.scheduler.get_stats().eliminated_tasks,
            pc.folded_points,
            pc.avoided_thread_spawns,
        );
    }

    pub fn emit_reports(&self) {
        eprint!("{}", self.ctfe.generate_report());
        eprint!("{}", self.dope.generate_report());
        eprint!("{}", self.precomp.generate_report());
        eprint!("{}", self.tce.generate_report());
        eprint!("{}", self.ifm.generate_report());
        eprint!("{}", self.scheduler.generate_report());
        eprint!("{}", self.pre_concurrency.generate_report());
    }
}

pub fn ctfe_to_llvm_literal(val: &CtfeValue) -> Option<String> {
    match val {
        CtfeValue::Int(n) => Some(n.to_string()),
        CtfeValue::Bool(b) => Some(if *b { "1".into() } else { "0".into() }),
        CtfeValue::Float(n) => {
            let s = n.to_string();
            if s.contains(['.', 'e', 'E']) {
                Some(s)
            } else {
                Some(format!("{s}.0"))
            }
        }
        _ => None,
    }
}

fn ctfe_to_runtime(val: &CtfeValue) -> Option<RuntimeValue> {
    match val {
        CtfeValue::Int(n) => Some(RuntimeValue::Int(*n)),
        CtfeValue::Bool(b) => Some(RuntimeValue::Bool(*b)),
        CtfeValue::Str(s) => Some(RuntimeValue::Str(s.clone())),
        _ => None,
    }
}

/// AST expr → CTFE op (Slime1 `expr_to_ctfe_op`, including Call for function fold).
fn expr_to_ctfe_op(expr: &Expr) -> Option<CtfeOp> {
    match &expr.kind {
        ExprKind::IntLit(n) => Some(CtfeOp::LoadConst(CtfeValue::Int(*n))),
        ExprKind::FloatLit(n) => Some(CtfeOp::LoadConst(CtfeValue::Float(*n))),
        ExprKind::BoolLit(b) => Some(CtfeOp::LoadConst(CtfeValue::Bool(*b))),
        ExprKind::StrLit(s) => Some(CtfeOp::LoadConst(CtfeValue::Str(s.clone()))),
        ExprKind::Ident(name) => Some(CtfeOp::Load(name.clone())),
        ExprKind::Binary { op, left, right } => {
            if matches!(
                op,
                AstBinOp::BitAnd | AstBinOp::BitOr | AstBinOp::BitXor | AstBinOp::Shl | AstBinOp::Shr
            ) {
                return None;
            }
            let l = expr_to_ctfe_op(left)?;
            let r = expr_to_ctfe_op(right)?;
            Some(CtfeOp::BinOp(
                ast_binop_to_ctfe(*op),
                Box::new(l),
                Box::new(r),
            ))
        }
        ExprKind::Call { callee, args, .. } => {
            let mut ctfe_args = Vec::with_capacity(args.len());
            for arg in args {
                ctfe_args.push(expr_to_ctfe_op(arg)?);
            }
            Some(CtfeOp::Call(callee.clone(), ctfe_args))
        }
        ExprKind::StructLit { .. } | ExprKind::Field { .. } | ExprKind::Index { .. } => None,
        ExprKind::Unary { .. } | ExprKind::MethodCall { .. } => None,
    }
}

/// AST stmt → CTFE op (Slime1 `stmt_to_ctfe_op`).
fn stmt_to_ctfe_op(stmt: &Stmt) -> Option<CtfeOp> {
    match &stmt.kind {
        StmtKind::VarDecl { name, init, .. } | StmtKind::Assign { name, value: init, .. } => {
            let val_op = expr_to_ctfe_op(init)?;
            Some(CtfeOp::Declare(name.clone(), Box::new(val_op)))
        }
        StmtKind::Return(Some(expr)) => {
            let ret_op = expr_to_ctfe_op(expr)?;
            Some(CtfeOp::Return(Box::new(ret_op)))
        }
        StmtKind::Return(None) => Some(CtfeOp::Return(Box::new(CtfeOp::LoadConst(
            CtfeValue::Int(0),
        )))),
        StmtKind::If {
            cond,
            then_body,
            else_body,
        } => {
            let cond_op = expr_to_ctfe_op(cond)?;
            let then_ops: Option<Vec<_>> = then_body.iter().map(stmt_to_ctfe_op).collect();
            let else_ops: Option<Vec<_>> = else_body.iter().map(stmt_to_ctfe_op).collect();
            Some(CtfeOp::Branch {
                cond: Box::new(cond_op),
                then_ops: then_ops?,
                else_ops: else_ops?,
            })
        }
        StmtKind::While { cond, body } => {
            let init = Box::new(CtfeOp::LoadConst(CtfeValue::Int(0)));
            let cond_op = Box::new(expr_to_ctfe_op(cond)?);
            let update = Box::new(CtfeOp::LoadConst(CtfeValue::Int(0)));
            let body_ops: Option<Vec<_>> = body.iter().map(stmt_to_ctfe_op).collect();
            Some(CtfeOp::Loop {
                init,
                cond: cond_op,
                update,
                body: body_ops?,
            })
        }
        StmtKind::For {
            name,
            start,
            end,
            body,
            ..
        } => {
            let start_op = expr_to_ctfe_op(start)?;
            let end_op = expr_to_ctfe_op(end)?;
            let body_ops: Option<Vec<_>> = body.iter().map(stmt_to_ctfe_op).collect();
            let init = Box::new(CtfeOp::Declare(
                name.clone(),
                Box::new(start_op),
            ));
            let cond = Box::new(CtfeOp::BinOp(
                CtfeBinOp::Lt,
                Box::new(CtfeOp::Load(name.clone())),
                Box::new(end_op),
            ));
            let update = Box::new(CtfeOp::Declare(
                name.clone(),
                Box::new(CtfeOp::BinOp(
                    CtfeBinOp::Add,
                    Box::new(CtfeOp::Load(name.clone())),
                    Box::new(CtfeOp::LoadConst(CtfeValue::Int(1))),
                )),
            ));
            Some(CtfeOp::Loop {
                init,
                cond,
                update,
                body: body_ops?,
            })
        }
        StmtKind::Expr(expr) => expr_to_ctfe_op(expr),
        StmtKind::ArrayDecl { .. }
        | StmtKind::IndexAssign { .. }
        | StmtKind::FieldAssign { .. }
        | StmtKind::DerefAssign { .. }
        | StmtKind::Break
        | StmtKind::Continue
        | StmtKind::Print(_) => None,
    }
}

fn collect_assigned_names(stmts: &[Stmt], out: &mut HashSet<String>) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::VarDecl { name, .. } | StmtKind::Assign { name, .. } => {
                out.insert(name.clone());
            }
            StmtKind::If {
                then_body,
                else_body,
                ..
            } => {
                collect_assigned_names(then_body, out);
                collect_assigned_names(else_body, out);
            }
            StmtKind::While { body, .. } => collect_assigned_names(body, out),
            StmtKind::For { name, body, .. } => {
                out.insert(name.clone());
                collect_assigned_names(body, out);
            }
            _ => {}
        }
    }
}

fn is_recursive_function(
    func_name: &str,
    functions: &HashMap<&str, &Function>,
    visited: &mut HashSet<String>,
) -> bool {
    if visited.contains(func_name) {
        return true;
    }
    let Some(func) = functions.get(func_name) else {
        return false;
    };
    visited.insert(func_name.to_string());
    for stmt in &func.body {
        if stmt_calls_function(stmt, func_name, functions, visited) {
            visited.remove(func_name);
            return true;
        }
    }
    visited.remove(func_name);
    false
}

fn stmt_calls_function(
    stmt: &Stmt,
    target: &str,
    functions: &HashMap<&str, &Function>,
    visited: &mut HashSet<String>,
) -> bool {
    match &stmt.kind {
        StmtKind::VarDecl { init, .. }
        | StmtKind::Assign { value: init, .. }
        | StmtKind::Return(Some(init))
        | StmtKind::Expr(init)
        | StmtKind::IndexAssign { value: init, .. }
        | StmtKind::FieldAssign { value: init, .. }
        | StmtKind::DerefAssign { value: init, .. } => {
            expr_calls_function(init, target, functions, visited)
        }
        StmtKind::If {
            cond,
            then_body,
            else_body,
        } => {
            expr_calls_function(cond, target, functions, visited)
                || then_body
                    .iter()
                    .any(|s| stmt_calls_function(s, target, functions, visited))
                || else_body
                    .iter()
                    .any(|s| stmt_calls_function(s, target, functions, visited))
        }
        StmtKind::While { cond, body } => {
            expr_calls_function(cond, target, functions, visited)
                || body
                    .iter()
                    .any(|s| stmt_calls_function(s, target, functions, visited))
        }
        StmtKind::For {
            start,
            end,
            body,
            ..
        } => {
            expr_calls_function(start, target, functions, visited)
                || expr_calls_function(end, target, functions, visited)
                || body
                    .iter()
                    .any(|s| stmt_calls_function(s, target, functions, visited))
        }
        StmtKind::Print(args) => args
            .iter()
            .any(|e| expr_calls_function(e, target, functions, visited)),
        StmtKind::ArrayDecl { .. }
        | StmtKind::Break
        | StmtKind::Continue
        | StmtKind::Return(None)
        | StmtKind::DerefAssign { .. } => false,
    }
}

fn expr_calls_function(
    expr: &Expr,
    target: &str,
    functions: &HashMap<&str, &Function>,
    visited: &mut HashSet<String>,
) -> bool {
    match &expr.kind {
        ExprKind::Call { callee, args, .. } => {
            if callee == target {
                return true;
            }
            // Indirect: callee's body eventually calls `target`.
            if let Some(f) = functions.get(callee.as_str()) {
                if !visited.contains(callee) {
                    visited.insert(callee.clone());
                    let hit = f
                        .body
                        .iter()
                        .any(|s| stmt_calls_function(s, target, functions, visited));
                    visited.remove(callee);
                    if hit {
                        return true;
                    }
                }
            }
            args.iter()
                .any(|a| expr_calls_function(a, target, functions, visited))
        }
        ExprKind::Binary { left, right, .. } => {
            expr_calls_function(left, target, functions, visited)
                || expr_calls_function(right, target, functions, visited)
        }
        ExprKind::Field { base, .. } => expr_calls_function(base, target, functions, visited),
        ExprKind::Index { base, index } => {
            expr_calls_function(base, target, functions, visited)
                || expr_calls_function(index, target, functions, visited)
        }
        ExprKind::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, _, e)| expr_calls_function(e, target, functions, visited)),
        ExprKind::IntLit(_)
        | ExprKind::FloatLit(_)
        | ExprKind::StrLit(_)
        | ExprKind::BoolLit(_)
        | ExprKind::Ident(_)
        | ExprKind::Unary { .. }
        | ExprKind::MethodCall { .. } => false,
    }
}

fn ast_binop_to_ctfe(op: AstBinOp) -> CtfeBinOp {
    match op {
        AstBinOp::Add => CtfeBinOp::Add,
        AstBinOp::Sub => CtfeBinOp::Sub,
        AstBinOp::Mul => CtfeBinOp::Mul,
        AstBinOp::Div => CtfeBinOp::Div,
        AstBinOp::Mod => CtfeBinOp::Mod,
        AstBinOp::Eq => CtfeBinOp::Eq,
        AstBinOp::Ne => CtfeBinOp::Ne,
        AstBinOp::Lt => CtfeBinOp::Lt,
        AstBinOp::Le => CtfeBinOp::Le,
        AstBinOp::Gt => CtfeBinOp::Gt,
        AstBinOp::Ge => CtfeBinOp::Ge,
        AstBinOp::And => CtfeBinOp::And,
        AstBinOp::Or => CtfeBinOp::Or,
        AstBinOp::BitAnd | AstBinOp::BitOr | AstBinOp::BitXor | AstBinOp::Shl | AstBinOp::Shr => {
            unreachable!("bitwise ops filtered before ast_binop_to_ctfe")
        }
    }
}

fn expr_to_ifm_sig(expr: &Expr) -> Option<InstructionSignature> {
    let ExprKind::Binary { op, left, right } = &expr.kind else {
        return None;
    };
    let Opcode(opname) = match op {
        AstBinOp::Add => Opcode("add"),
        AstBinOp::Sub => Opcode("sub"),
        AstBinOp::Mul => Opcode("mul"),
        _ => return None,
    };
    let l = match &left.kind {
        ExprKind::IntLit(n) => Operand::Imm(*n),
        _ => return None,
    };
    let r = match &right.kind {
        ExprKind::IntLit(n) => Operand::Imm(*n),
        _ => return None,
    };
    Some(InstructionSignature {
        opcode: opname.into(),
        inputs: vec![l, r],
        flags: None,
    })
}

struct Opcode(&'static str);

fn ast_to_dope_expr(expr: &Expr) -> Option<DopeExpr> {
    match &expr.kind {
        ExprKind::IntLit(n) => Some(DopeExpr::Const(DopeValue::Int(*n))),
        ExprKind::FloatLit(n) => Some(DopeExpr::Const(DopeValue::Float(*n))),
        ExprKind::BoolLit(b) => Some(DopeExpr::Const(DopeValue::Bool(*b))),
        ExprKind::StrLit(s) => Some(DopeExpr::Const(DopeValue::Str(s.clone()))),
        ExprKind::Ident(name) => Some(DopeExpr::Var(name.clone())),
        ExprKind::Binary { op, .. } => {
            let _ = match op {
                AstBinOp::Add => dope::BinOp::Add,
                AstBinOp::Sub => dope::BinOp::Sub,
                AstBinOp::Mul => dope::BinOp::Mul,
                AstBinOp::Div => dope::BinOp::Div,
                _ => return Some(DopeExpr::Var("__cmp__".into())),
            };
            Some(DopeExpr::Var("__arith__".into()))
        }
        // CTFE2: treat registered pure calls as deterministic (DOPE assumes pure).
        ExprKind::Call { callee, args, .. } => Some(DopeExpr::Call {
            func: callee.clone(),
            args: (0..args.len()).map(|i| format!("a{i}")).collect(),
        }),
        ExprKind::StructLit { .. } | ExprKind::Field { .. } | ExprKind::Index { .. } => {
            Some(DopeExpr::Input)
        }
        ExprKind::Unary { .. } | ExprKind::MethodCall { .. } => Some(DopeExpr::Input),
    }
}
