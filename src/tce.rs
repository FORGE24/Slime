// ============================================================================
// TCE Module - Temporal Collapse Execution (Slime2 / LLVM IR)
// Inherited conceptually from Slime1: https://github.com/FORGE24/Slime
// Copyright (c) 2024-2026 Sanrol Team / FORGE24.
//
// Slime1's full `tce.rs` (~600KB) depends on `scheduler_elimination`.
// This is the LLVM-facing core: collapse known work onto compile-time IR.
// ============================================================================

//! 时间坍缩执行（TCE）— Slime2 LLVM IR 精简版
//!
//! 把可确定的计算从 Runtime 时间点坍缩到 Compile 时间点，
//! 结果以 LLVM 常量直接写入 IR，不再经过 AST 改写。

#![allow(dead_code)]

use std::collections::HashMap;

/// 时间点（一等可优化维度）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TimePoint {
    Compile,
    Startup,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapseStrategy {
    Aggressive,
    Balanced,
    Conservative,
}

#[derive(Debug, Default)]
pub struct TceStats {
    pub collapsed_to_compile: usize,
    pub left_at_runtime: usize,
    pub llvm_consts_emitted: usize,
}

/// 已坍缩的计算记录（供报告 / 调试）
#[derive(Debug, Clone)]
pub struct CollapsedComputation {
    pub from: TimePoint,
    pub to: TimePoint,
    pub llvm_const: String,
    pub note: String,
}

pub struct TceEngine {
    strategy: CollapseStrategy,
    stats: TceStats,
    collapsed: Vec<CollapsedComputation>,
    /// name → compile-time LLVM constant text (e.g. "42", "1")
    compile_vals: HashMap<String, String>,
}

impl TceEngine {
    pub fn new(strategy: CollapseStrategy) -> Self {
        Self {
            strategy,
            stats: TceStats::default(),
            collapsed: Vec::new(),
            compile_vals: HashMap::new(),
        }
    }

    pub fn strategy(&self) -> CollapseStrategy {
        self.strategy
    }

    /// Record a successful collapse: Runtime/Startup → Compile LLVM constant.
    pub fn collapse_to_llvm_const(
        &mut self,
        name: &str,
        llvm_const: impl Into<String>,
        note: impl Into<String>,
    ) {
        let llvm_const = llvm_const.into();
        self.compile_vals
            .insert(name.to_string(), llvm_const.clone());
        self.stats.collapsed_to_compile += 1;
        self.stats.llvm_consts_emitted += 1;
        self.collapsed.push(CollapsedComputation {
            from: TimePoint::Runtime,
            to: TimePoint::Compile,
            llvm_const,
            note: note.into(),
        });
    }

    pub fn mark_runtime(&mut self, _reason: &str) {
        self.stats.left_at_runtime += 1;
    }

    pub fn get_compile_const(&self, name: &str) -> Option<&str> {
        self.compile_vals.get(name).map(|s| s.as_str())
    }

    pub fn clear_compile_const(&mut self, name: &str) {
        self.compile_vals.remove(name);
    }

    pub fn get_stats(&self) -> &TceStats {
        &self.stats
    }

    pub fn generate_report(&self) -> String {
        let mut r = String::from("=== TCE (LLVM) Report ===\n");
        r.push_str(&format!(
            "Collapsed → compile: {}\n",
            self.stats.collapsed_to_compile
        ));
        r.push_str(&format!(
            "Left at runtime: {}\n",
            self.stats.left_at_runtime
        ));
        r.push_str(&format!(
            "LLVM consts emitted: {}\n",
            self.stats.llvm_consts_emitted
        ));
        if !self.collapsed.is_empty() {
            r.push_str("\nRecent collapses:\n");
            for c in self.collapsed.iter().rev().take(8) {
                r.push_str(&format!(
                    "  - {:?} → {:?} = {} ({})\n",
                    c.from, c.to, c.llvm_const, c.note
                ));
            }
        }
        r
    }
}
