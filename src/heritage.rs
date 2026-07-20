//! Slime2 ↔ Slime1 (FORGE24/Slime) inheritance map
//!
//! Upstream: https://github.com/FORGE24/Slime
//! Architecture: ETCA (Execution-Time Collapse Architecture)
//!
//! | Slime1 module              | Status in Slime2      | Role                          |
//! |----------------------------|-----------------------|-------------------------------|
//! | `ctfe.rs`                  | **ported**            | Compile-Time Forced Execution |
//! | `dynamic_precomp.rs`       | **ported**            | Dynamic precomputation        |
//! | `dope.rs`                  | **ported**            | Deterministic Online PE       |
//! | `tce.rs`                   | **ported (LLVM slim)**| Time-Collapse → IR consts     |
//! | `ifm.rs`                   | **ported**            | Instruction Frequency Memo    |
//! | `scheduler_elimination.rs` | **ported**            | Scheduler-level task elim     |
//! | `pre_concurrency.rs`       | **ported**            | Pre-concurrency folding       |
//! | `ctfe_extended.rs`         | optional later        | Cross-module / macros         |
//!
//! Slime2: ETCA engines emit **LLVM IR constants** during codegen.
//! The AST is a read-only parse tree — no CTFE AST rewriting.
//!
//! CTFE2 (Slime1 path): `register_functions_from_program` lowers pure `fn` bodies
//! to `CtfeOp`, skips recursive / non-lowerable functions; call sites fold to
//! LLVM constants via `try_etca_fold` (`Call` → compile-time VM → const).
//!
//! Closed `while` / `for` are also CTFE'd whole: the VM runs the loop, then
//! codegen emits residual `store` of final locals (no loop IR). Open loops
//! (arrays, I/O, break, …) stay runtime; loop-carried idents are not frozen.
//!
//! Full ETCA stack is now wired in `etca.rs`.
