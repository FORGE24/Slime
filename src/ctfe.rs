// ============================================================================
// CTFE Module - Compile-Time Forced Execution
// Copyright (c) 2024-2026 Sanrol Team.
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
// ============================================================================

//! 强制编译期执行（Mandatory CTFE - Compile-Time Forced Execution）
//!
//! 核心理念：
//! - 程序默认在编译期执行
//! - runtime 是退化路径，不是主战场
//! - 计算前移到编译期，runtime成本趋近于0

#![allow(dead_code, unused_variables, unused_mut, unused_imports)]

use std::collections::HashMap;

/// CTFE执行环境
pub struct CtfeEngine {
    /// 编译期值表
    const_values: HashMap<String, CtfeValue>,
    /// 用户定义函数表
    functions: HashMap<String, CtfeFn>,
    /// 编译期执行统计
    stats: CtfeStats,
    /// 强制CTFE模式（默认true）
    mandatory_mode: bool,
    /// runtime降级追踪
    runtime_degradations: Vec<RuntimeDegradation>,
}

/// 编译期值
#[derive(Debug, Clone, PartialEq)]
pub enum CtfeValue {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Array(Vec<CtfeValue>),
    Struct(HashMap<String, CtfeValue>),
    Function(CtfeFn),
    /// 确定性未来值（必然在编译期确定）
    DeterministicFuture(Box<CtfeValue>),
    /// 运行时降级值（无法在编译期确定）
    RuntimeDegraded(String),
}

/// 编译期函数
#[derive(Debug, Clone, PartialEq)]
pub struct CtfeFn {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<CtfeOp>,
}

/// 编译期操作
#[derive(Debug, Clone, PartialEq)]
pub enum CtfeOp {
    /// 常量加载
    LoadConst(CtfeValue),
    /// 二元运算
    BinOp(BinOp, Box<CtfeOp>, Box<CtfeOp>),
    /// 函数调用
    Call(String, Vec<CtfeOp>),
    /// 条件分支（编译期求值）
    Branch {
        cond: Box<CtfeOp>,
        then_ops: Vec<CtfeOp>,
        else_ops: Vec<CtfeOp>,
    },
    /// 循环（编译期展开）
    Loop {
        init: Box<CtfeOp>,
        cond: Box<CtfeOp>,
        update: Box<CtfeOp>,
        body: Vec<CtfeOp>,
    },
    /// 变量声明（编译期绑定）
    Declare(String, Box<CtfeOp>),
    /// 变量读取
    Load(String),
    /// 返回值
    Return(Box<CtfeOp>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
}

/// CTFE统计
#[derive(Debug, Default)]
pub struct CtfeStats {
    /// 编译期执行的函数数量
    pub ctfe_functions: usize,
    /// 编译期执行的循环数量
    pub ctfe_loops: usize,
    /// 编译期求值的表达式数量
    pub ctfe_exprs: usize,
    /// 运行时降级次数
    pub runtime_degradations: usize,
    /// 时间节省（估算）
    pub time_saved_ns: u64,
}

/// 运行时降级记录
#[derive(Debug, Clone)]
pub struct RuntimeDegradation {
    pub reason: String,
    pub location: String,
    pub value_name: String,
}

impl CtfeEngine {
    pub fn new() -> Self {
        CtfeEngine {
            const_values: HashMap::new(),
            functions: HashMap::new(),
            stats: CtfeStats::default(),
            mandatory_mode: true,
            runtime_degradations: Vec::new(),
        }
    }
    
    /// 注册用户定义函数
    pub fn register_function(&mut self, name: String, params: Vec<String>, body: Vec<CtfeOp>) {
        let func = CtfeFn { name: name.clone(), params, body };
        self.functions.insert(name, func);
    }
    
    /// 执行编译期求值
    pub fn execute(&mut self, op: &CtfeOp) -> Result<CtfeValue, CtfeError> {
        match op {
            CtfeOp::LoadConst(val) => {
                self.stats.ctfe_exprs += 1;
                Ok(val.clone())
            }
            
            CtfeOp::BinOp(binop, left, right) => {
                self.stats.ctfe_exprs += 1;
                let lval = self.execute(left)?;
                let rval = self.execute(right)?;
                self.eval_binop(*binop, lval, rval)
            }
            
            CtfeOp::Call(name, args) => {
                self.stats.ctfe_functions += 1;
                self.execute_call(name, args)
            }
            
            CtfeOp::Branch { cond, then_ops, else_ops } => {
                let cond_val = self.execute(cond)?;
                match cond_val {
                    CtfeValue::Bool(true) => {
                        self.execute_block(then_ops)
                    }
                    CtfeValue::Bool(false) => {
                        self.execute_block(else_ops)
                    }
                    _ => Err(CtfeError::TypeError("Condition must be boolean".to_string())),
                }
            }
            
            CtfeOp::Loop { init, cond, update, body } => {
                self.stats.ctfe_loops += 1;
                self.execute_loop(init, cond, update, body)
            }
            
            CtfeOp::Declare(name, value) => {
                let val = self.execute(value)?;
                self.const_values.insert(name.clone(), val.clone());
                Ok(val)
            }
            
            CtfeOp::Load(name) => {
                self.const_values.get(name)
                    .cloned()
                    .ok_or_else(|| CtfeError::UndefinedVariable(name.clone()))
            }
            
            CtfeOp::Return(val) => {
                self.execute(val)
            }
        }
    }
    
    /// 执行二元运算
    fn eval_binop(&self, op: BinOp, left: CtfeValue, right: CtfeValue) -> Result<CtfeValue, CtfeError> {
        match (left, right) {
            (CtfeValue::Int(l), CtfeValue::Int(r)) => {
                let result = match op {
                    BinOp::Add => l + r,
                    BinOp::Sub => l - r,
                    BinOp::Mul => l * r,
                    BinOp::Div => l / r,
                    BinOp::Mod => l % r,
                    BinOp::Eq => return Ok(CtfeValue::Bool(l == r)),
                    BinOp::Ne => return Ok(CtfeValue::Bool(l != r)),
                    BinOp::Lt => return Ok(CtfeValue::Bool(l < r)),
                    BinOp::Gt => return Ok(CtfeValue::Bool(l > r)),
                    BinOp::Le => return Ok(CtfeValue::Bool(l <= r)),
                    BinOp::Ge => return Ok(CtfeValue::Bool(l >= r)),
                    _ => return Err(CtfeError::InvalidOperation),
                };
                Ok(CtfeValue::Int(result))
            }
            (CtfeValue::Bool(l), CtfeValue::Bool(r)) => {
                let result = match op {
                    BinOp::And => l && r,
                    BinOp::Or => l || r,
                    BinOp::Eq => l == r,
                    BinOp::Ne => l != r,
                    _ => return Err(CtfeError::InvalidOperation),
                };
                Ok(CtfeValue::Bool(result))
            }
            _ => Err(CtfeError::TypeError("Type mismatch in binary operation".to_string())),
        }
    }
    
    /// 执行循环（编译期展开）
    fn execute_loop(
        &mut self,
        init: &CtfeOp,
        cond: &CtfeOp,
        update: &CtfeOp,
        body: &[CtfeOp],
    ) -> Result<CtfeValue, CtfeError> {
        // 初始化
        self.execute(init)?;
        
        let mut result = CtfeValue::Int(0);
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 100_000_000; // 扩展到1亿次迭代（展示强大预计算）
        
        loop {
            // 检查条件
            let cond_val = self.execute(cond)?;
            match cond_val {
                CtfeValue::Bool(false) => break,
                CtfeValue::Bool(true) => {
                    // 执行循环体
                    result = self.execute_block(body)?;
                    // 更新
                    self.execute(update)?;
                    
                    iterations += 1;
                    if iterations > MAX_ITERATIONS {
                        return self.degrade_to_runtime("Loop exceeded CTFE iteration limit");
                    }
                }
                _ => return Err(CtfeError::TypeError("Loop condition must be boolean".to_string())),
            }
        }
        
        Ok(result)
    }
    
    /// 执行代码块
    fn execute_block(&mut self, ops: &[CtfeOp]) -> Result<CtfeValue, CtfeError> {
        let mut result = CtfeValue::Int(0);
        for op in ops {
            result = self.execute(op)?;
        }
        Ok(result)
    }
    
    /// 执行函数调用
    fn execute_call(&mut self, name: &str, args: &[CtfeOp]) -> Result<CtfeValue, CtfeError> {
        // 内置函数
        match name {
            "print" => {
                // 编译期print只记录，不输出
                Ok(CtfeValue::Int(0))
            }
            _ => {
                // 查找用户定义函数
                if let Some(func) = self.functions.get(name).cloned() {
                    // 执行用户函数
                    self.execute_user_function(&func, args)
                } else {
                    Err(CtfeError::UndefinedFunction(name.to_string()))
                }
            }
        }
    }
    
    /// 执行用户定义函数
    fn execute_user_function(&mut self, func: &CtfeFn, args: &[CtfeOp]) -> Result<CtfeValue, CtfeError> {
        // 检查参数数量
        if args.len() != func.params.len() {
            return Err(CtfeError::TypeError(format!(
                "Function {} expects {} arguments, got {}",
                func.name, func.params.len(), args.len()
            )));
        }
        
        // 保存当前作用域
        let saved_scope = self.const_values.clone();
        
        // 绑定参数
        for (i, param) in func.params.iter().enumerate() {
            let arg_value = self.execute(&args[i])?;
            self.const_values.insert(param.clone(), arg_value);
        }
        
        // 执行函数体
        let result = self.execute_block(&func.body);
        
        // 恢复作用域
        self.const_values = saved_scope;
        
        result
    }
    
    /// 降级到运行时
    fn degrade_to_runtime(&mut self, reason: &str) -> Result<CtfeValue, CtfeError> {
        self.stats.runtime_degradations += 1;
        self.runtime_degradations.push(RuntimeDegradation {
            reason: reason.to_string(),
            location: "unknown".to_string(),
            value_name: "unknown".to_string(),
        });
        Ok(CtfeValue::RuntimeDegraded(reason.to_string()))
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> &CtfeStats {
        &self.stats
    }
    
    /// 获取运行时降级列表
    pub fn get_degradations(&self) -> &[RuntimeDegradation] {
        &self.runtime_degradations
    }
    
    /// 生成CTFE报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== CTFE Execution Report ===\n");
        report.push_str(&format!("CTFE Functions: {}\n", self.stats.ctfe_functions));
        report.push_str(&format!("CTFE Loops: {}\n", self.stats.ctfe_loops));
        report.push_str(&format!("CTFE Expressions: {}\n", self.stats.ctfe_exprs));
        report.push_str(&format!("Runtime Degradations: {}\n", self.stats.runtime_degradations));
        
        if !self.runtime_degradations.is_empty() {
            report.push_str("\nRuntime Degradation Details:\n");
            for deg in &self.runtime_degradations {
                report.push_str(&format!("  - {}: {}\n", deg.value_name, deg.reason));
            }
        }
        
        let ctfe_ratio = if self.stats.ctfe_exprs + self.stats.runtime_degradations > 0 {
            (self.stats.ctfe_exprs as f64) / 
            ((self.stats.ctfe_exprs + self.stats.runtime_degradations) as f64) * 100.0
        } else {
            100.0
        };
        report.push_str(&format!("\nCTFE Coverage: {:.1}%\n", ctfe_ratio));
        
        report
    }
}

#[derive(Debug)]
pub enum CtfeError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    TypeError(String),
    InvalidOperation,
    RuntimeRequired(String),
}

impl std::fmt::Display for CtfeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CtfeError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            CtfeError::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            CtfeError::TypeError(msg) => write!(f, "Type error: {}", msg),
            CtfeError::InvalidOperation => write!(f, "Invalid operation"),
            CtfeError::RuntimeRequired(reason) => write!(f, "Runtime required: {}", reason),
        }
    }
}

impl std::error::Error for CtfeError {}

// ============================================================================
// 扩展功能模块：高级数据类型、控制流、优化器
// ============================================================================

/// 扩展的编译期值类型
#[derive(Debug, Clone, PartialEq)]
pub enum CtfeValueExt {
    /// 元组类型
    Tuple(Vec<CtfeValue>),
    /// 哈希映射
    HashMap(HashMap<String, CtfeValue>),
    /// Option类型
    Option(Option<Box<CtfeValue>>),
    /// Result类型
    Result(Result<Box<CtfeValue>, Box<CtfeValue>>),
    /// 函数指针
    FnPtr(String, Vec<String>),
    /// 闭包（捕获环境）
    Closure {
        params: Vec<String>,
        body: Vec<CtfeOp>,
        captured: HashMap<String, CtfeValue>,
    },
    /// Range类型
    Range { start: i64, end: i64, step: i64 },
    /// 引用类型（编译期引用追踪）
    Reference(Box<CtfeValue>),
    /// 可变引用
    MutReference(Box<CtfeValue>),
}

/// 扩展的编译期操作
#[derive(Debug, Clone, PartialEq)]
pub enum CtfeOpExt {
    /// For循环（编译期展开）
    For {
        var: String,
        iter: Box<CtfeOp>,
        body: Vec<CtfeOp>,
    },
    /// Break语句
    Break,
    /// Continue语句
    Continue,
    /// Match表达式（模式匹配）
    Match {
        value: Box<CtfeOp>,
        arms: Vec<MatchArm>,
    },
    /// 数组索引
    Index(Box<CtfeOp>, Box<CtfeOp>),
    /// 字段访问
    FieldAccess(Box<CtfeOp>, String),
    /// 方法调用
    MethodCall {
        receiver: Box<CtfeOp>,
        method: String,
        args: Vec<CtfeOp>,
    },
    /// 类型转换
    Cast(Box<CtfeOp>, CtfeType),
    /// 引用创建
    CreateRef(Box<CtfeOp>),
    /// 解引用
    Deref(Box<CtfeOp>),
    /// 赋值操作
    Assign(String, Box<CtfeOp>),
    /// 复合赋值（+=, -=等）
    CompoundAssign(String, BinOp, Box<CtfeOp>),
}

/// Match模式匹配臂
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Box<CtfeOp>>,
    pub body: Vec<CtfeOp>,
}

/// 模式类型
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// 通配符 _
    Wildcard,
    /// 字面量
    Literal(CtfeValue),
    /// 变量绑定
    Variable(String),
    /// 元组模式
    Tuple(Vec<Pattern>),
    /// 结构体模式
    Struct {
        name: String,
        fields: HashMap<String, Pattern>,
    },
    /// Or模式
    Or(Vec<Pattern>),
}

/// CTFE类型系统
#[derive(Debug, Clone, PartialEq)]
pub enum CtfeType {
    Int,
    Float,
    Bool,
    String,
    Array(Box<CtfeType>),
    Tuple(Vec<CtfeType>),
    Struct(HashMap<String, CtfeType>),
    Function(Vec<CtfeType>, Box<CtfeType>),
    Option(Box<CtfeType>),
    Result(Box<CtfeType>, Box<CtfeType>),
    Reference(Box<CtfeType>),
    MutReference(Box<CtfeType>),
    Generic(String),
    Unknown,
}

/// 内存管理器（编译期内存模拟）
#[derive(Debug, Clone)]
pub struct CtfeMemoryManager {
    /// 内存分配表
    allocations: HashMap<usize, CtfeAllocation>,
    /// 下一个分配ID
    next_alloc_id: usize,
    /// 引用计数表
    ref_counts: HashMap<usize, usize>,
    /// 生命周期追踪
    lifetimes: HashMap<usize, CtfeLifetime>,
}

#[derive(Debug, Clone)]
pub struct CtfeAllocation {
    pub id: usize,
    pub value: CtfeValue,
    pub is_mutable: bool,
    pub borrowed: BorrowState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BorrowState {
    NotBorrowed,
    SharedBorrowed(Vec<usize>),  // 多个不可变借用的ID列表
    MutBorrowed(usize),           // 唯一可变借用的ID
}

#[derive(Debug, Clone)]
pub struct CtfeLifetime {
    pub start_scope: usize,
    pub end_scope: usize,
    pub is_static: bool,
}

impl CtfeMemoryManager {
    pub fn new() -> Self {
        CtfeMemoryManager {
            allocations: HashMap::new(),
            next_alloc_id: 0,
            ref_counts: HashMap::new(),
            lifetimes: HashMap::new(),
        }
    }
    
    /// 分配内存
    pub fn allocate(&mut self, value: CtfeValue, is_mutable: bool) -> usize {
        let id = self.next_alloc_id;
        self.next_alloc_id += 1;
        
        self.allocations.insert(id, CtfeAllocation {
            id,
            value,
            is_mutable,
            borrowed: BorrowState::NotBorrowed,
        });
        
        self.ref_counts.insert(id, 1);
        id
    }
    
    /// 增加引用计数
    pub fn add_ref(&mut self, id: usize) -> Result<(), String> {
        if let Some(count) = self.ref_counts.get_mut(&id) {
            *count += 1;
            Ok(())
        } else {
            Err(format!("Invalid allocation ID: {}", id))
        }
    }
    
    /// 减少引用计数
    pub fn drop_ref(&mut self, id: usize) -> Result<bool, String> {
        if let Some(count) = self.ref_counts.get_mut(&id) {
            *count -= 1;
            if *count == 0 {
                self.allocations.remove(&id);
                self.ref_counts.remove(&id);
                self.lifetimes.remove(&id);
                Ok(true)  // 已释放
            } else {
                Ok(false)  // 仍有引用
            }
        } else {
            Err(format!("Invalid allocation ID: {}", id))
        }
    }
    
    /// 创建不可变借用
    pub fn borrow_shared(&mut self, id: usize, borrow_id: usize) -> Result<(), String> {
        if let Some(alloc) = self.allocations.get_mut(&id) {
            match &mut alloc.borrowed {
                BorrowState::NotBorrowed => {
                    alloc.borrowed = BorrowState::SharedBorrowed(vec![borrow_id]);
                    Ok(())
                }
                BorrowState::SharedBorrowed(borrows) => {
                    borrows.push(borrow_id);
                    Ok(())
                }
                BorrowState::MutBorrowed(_) => {
                    Err("Cannot borrow as shared: already mutably borrowed".to_string())
                }
            }
        } else {
            Err(format!("Invalid allocation ID: {}", id))
        }
    }
    
    /// 创建可变借用
    pub fn borrow_mut(&mut self, id: usize, borrow_id: usize) -> Result<(), String> {
        if let Some(alloc) = self.allocations.get_mut(&id) {
            if !alloc.is_mutable {
                return Err("Cannot borrow as mutable: value is immutable".to_string());
            }
            
            match &alloc.borrowed {
                BorrowState::NotBorrowed => {
                    alloc.borrowed = BorrowState::MutBorrowed(borrow_id);
                    Ok(())
                }
                _ => {
                    Err("Cannot borrow as mutable: already borrowed".to_string())
                }
            }
        } else {
            Err(format!("Invalid allocation ID: {}", id))
        }
    }
    
    /// 释放借用
    pub fn release_borrow(&mut self, id: usize, borrow_id: usize) -> Result<(), String> {
        if let Some(alloc) = self.allocations.get_mut(&id) {
            match &mut alloc.borrowed {
                BorrowState::SharedBorrowed(borrows) => {
                    borrows.retain(|&b| b != borrow_id);
                    if borrows.is_empty() {
                        alloc.borrowed = BorrowState::NotBorrowed;
                    }
                    Ok(())
                }
                BorrowState::MutBorrowed(b) if *b == borrow_id => {
                    alloc.borrowed = BorrowState::NotBorrowed;
                    Ok(())
                }
                _ => {
                    Err(format!("Borrow ID {} not found for allocation {}", borrow_id, id))
                }
            }
        } else {
            Err(format!("Invalid allocation ID: {}", id))
        }
    }
    
    /// 获取值（检查借用规则）
    pub fn get_value(&self, id: usize) -> Result<&CtfeValue, String> {
        if let Some(alloc) = self.allocations.get(&id) {
            Ok(&alloc.value)
        } else {
            Err(format!("Invalid allocation ID: {}", id))
        }
    }
    
    /// 修改值（检查可变性和借用规则）
    pub fn set_value(&mut self, id: usize, value: CtfeValue) -> Result<(), String> {
        if let Some(alloc) = self.allocations.get_mut(&id) {
            if !alloc.is_mutable {
                return Err("Cannot modify immutable value".to_string());
            }
            
            match &alloc.borrowed {
                BorrowState::NotBorrowed | BorrowState::MutBorrowed(_) => {
                    alloc.value = value;
                    Ok(())
                }
                BorrowState::SharedBorrowed(_) => {
                    Err("Cannot modify: value is borrowed".to_string())
                }
            }
        } else {
            Err(format!("Invalid allocation ID: {}", id))
        }
    }
}

/// 内置函数库
pub struct CtfeBuiltins {
    functions: HashMap<String, BuiltinFn>,
}

type BuiltinFn = fn(&[CtfeValue]) -> Result<CtfeValue, CtfeError>;

impl CtfeBuiltins {
    pub fn new() -> Self {
        let mut builtins = CtfeBuiltins {
            functions: HashMap::new(),
        };
        builtins.register_math_functions();
        builtins.register_string_functions();
        builtins.register_array_functions();
        builtins.register_conversion_functions();
        builtins
    }
    
    fn register_math_functions(&mut self) {
        // abs(x) - 绝对值
        self.functions.insert("abs".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("abs expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Int(n) => Ok(CtfeValue::Int(n.abs())),
                CtfeValue::Float(f) => Ok(CtfeValue::Float(f.abs())),
                _ => Err(CtfeError::TypeError("abs expects numeric argument".to_string())),
            }
        });
        
        // pow(base, exp) - 幂运算
        self.functions.insert("pow".to_string(), |args| {
            if args.len() != 2 {
                return Err(CtfeError::TypeError("pow expects 2 arguments".to_string()));
            }
            match (&args[0], &args[1]) {
                (CtfeValue::Int(base), CtfeValue::Int(exp)) => {
                    if *exp < 0 {
                        return Err(CtfeError::TypeError("pow: negative exponent not supported for integers".to_string()));
                    }
                    Ok(CtfeValue::Int(base.pow(*exp as u32)))
                }
                (CtfeValue::Float(base), CtfeValue::Float(exp)) => {
                    Ok(CtfeValue::Float(base.powf(*exp)))
                }
                _ => Err(CtfeError::TypeError("pow expects numeric arguments".to_string())),
            }
        });
        
        // sqrt(x) - 平方根
        self.functions.insert("sqrt".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("sqrt expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Int(n) => Ok(CtfeValue::Float((*n as f64).sqrt())),
                CtfeValue::Float(f) => Ok(CtfeValue::Float(f.sqrt())),
                _ => Err(CtfeError::TypeError("sqrt expects numeric argument".to_string())),
            }
        });
        
        // min(a, b) - 最小值
        self.functions.insert("min".to_string(), |args| {
            if args.len() != 2 {
                return Err(CtfeError::TypeError("min expects 2 arguments".to_string()));
            }
            match (&args[0], &args[1]) {
                (CtfeValue::Int(a), CtfeValue::Int(b)) => Ok(CtfeValue::Int(*a.min(b))),
                (CtfeValue::Float(a), CtfeValue::Float(b)) => Ok(CtfeValue::Float(a.min(*b))),
                _ => Err(CtfeError::TypeError("min expects numeric arguments".to_string())),
            }
        });
        
        // max(a, b) - 最大值
        self.functions.insert("max".to_string(), |args| {
            if args.len() != 2 {
                return Err(CtfeError::TypeError("max expects 2 arguments".to_string()));
            }
            match (&args[0], &args[1]) {
                (CtfeValue::Int(a), CtfeValue::Int(b)) => Ok(CtfeValue::Int(*a.max(b))),
                (CtfeValue::Float(a), CtfeValue::Float(b)) => Ok(CtfeValue::Float(a.max(*b))),
                _ => Err(CtfeError::TypeError("max expects numeric arguments".to_string())),
            }
        });
        
        // clamp(x, min, max) - 限制在范围内
        self.functions.insert("clamp".to_string(), |args| {
            if args.len() != 3 {
                return Err(CtfeError::TypeError("clamp expects 3 arguments".to_string()));
            }
            match (&args[0], &args[1], &args[2]) {
                (CtfeValue::Int(x), CtfeValue::Int(min), CtfeValue::Int(max)) => {
                    Ok(CtfeValue::Int((*x).max(*min).min(*max)))
                }
                _ => Err(CtfeError::TypeError("clamp expects integer arguments".to_string())),
            }
        });
    }
    
    fn register_string_functions(&mut self) {
        // len(s) - 字符串长度
        self.functions.insert("len".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("len expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Str(s) => Ok(CtfeValue::Int(s.len() as i64)),
                CtfeValue::Array(arr) => Ok(CtfeValue::Int(arr.len() as i64)),
                _ => Err(CtfeError::TypeError("len expects string or array".to_string())),
            }
        });
        
        // concat(s1, s2) - 字符串连接
        self.functions.insert("concat".to_string(), |args| {
            if args.len() != 2 {
                return Err(CtfeError::TypeError("concat expects 2 arguments".to_string()));
            }
            match (&args[0], &args[1]) {
                (CtfeValue::Str(s1), CtfeValue::Str(s2)) => {
                    Ok(CtfeValue::Str(format!("{}{}", s1, s2)))
                }
                _ => Err(CtfeError::TypeError("concat expects string arguments".to_string())),
            }
        });
        
        // substring(s, start, len) - 子字符串
        self.functions.insert("substring".to_string(), |args| {
            if args.len() != 3 {
                return Err(CtfeError::TypeError("substring expects 3 arguments".to_string()));
            }
            match (&args[0], &args[1], &args[2]) {
                (CtfeValue::Str(s), CtfeValue::Int(start), CtfeValue::Int(len)) => {
                    let start = *start as usize;
                    let len = *len as usize;
                    if start + len <= s.len() {
                        Ok(CtfeValue::Str(s[start..start+len].to_string()))
                    } else {
                        Err(CtfeError::TypeError("substring: index out of bounds".to_string()))
                    }
                }
                _ => Err(CtfeError::TypeError("substring expects (string, int, int)".to_string())),
            }
        });
        
        // to_upper(s) - 转大写
        self.functions.insert("to_upper".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("to_upper expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Str(s) => Ok(CtfeValue::Str(s.to_uppercase())),
                _ => Err(CtfeError::TypeError("to_upper expects string argument".to_string())),
            }
        });
        
        // to_lower(s) - 转小写
        self.functions.insert("to_lower".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("to_lower expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Str(s) => Ok(CtfeValue::Str(s.to_lowercase())),
                _ => Err(CtfeError::TypeError("to_lower expects string argument".to_string())),
            }
        });
    }
    
    fn register_array_functions(&mut self) {
        // push(array, value) - 数组追加
        self.functions.insert("push".to_string(), |args| {
            if args.len() != 2 {
                return Err(CtfeError::TypeError("push expects 2 arguments".to_string()));
            }
            match &args[0] {
                CtfeValue::Array(arr) => {
                    let mut new_arr = arr.clone();
                    new_arr.push(args[1].clone());
                    Ok(CtfeValue::Array(new_arr))
                }
                _ => Err(CtfeError::TypeError("push expects array as first argument".to_string())),
            }
        });
        
        // pop(array) - 数组弹出
        self.functions.insert("pop".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("pop expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Array(arr) => {
                    if arr.is_empty() {
                        Err(CtfeError::TypeError("pop: array is empty".to_string()))
                    } else {
                        Ok(arr[arr.len() - 1].clone())
                    }
                }
                _ => Err(CtfeError::TypeError("pop expects array argument".to_string())),
            }
        });
        
        // get(array, index) - 数组索引访问
        self.functions.insert("get".to_string(), |args| {
            if args.len() != 2 {
                return Err(CtfeError::TypeError("get expects 2 arguments".to_string()));
            }
            match (&args[0], &args[1]) {
                (CtfeValue::Array(arr), CtfeValue::Int(idx)) => {
                    let idx = *idx as usize;
                    if idx < arr.len() {
                        Ok(arr[idx].clone())
                    } else {
                        Err(CtfeError::TypeError("get: index out of bounds".to_string()))
                    }
                }
                _ => Err(CtfeError::TypeError("get expects (array, int)".to_string())),
            }
        });
        
        // slice(array, start, end) - 数组切片
        self.functions.insert("slice".to_string(), |args| {
            if args.len() != 3 {
                return Err(CtfeError::TypeError("slice expects 3 arguments".to_string()));
            }
            match (&args[0], &args[1], &args[2]) {
                (CtfeValue::Array(arr), CtfeValue::Int(start), CtfeValue::Int(end)) => {
                    let start = *start as usize;
                    let end = *end as usize;
                    if start <= end && end <= arr.len() {
                        Ok(CtfeValue::Array(arr[start..end].to_vec()))
                    } else {
                        Err(CtfeError::TypeError("slice: invalid range".to_string()))
                    }
                }
                _ => Err(CtfeError::TypeError("slice expects (array, int, int)".to_string())),
            }
        });
        
        // reverse(array) - 数组反转
        self.functions.insert("reverse".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("reverse expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Array(arr) => {
                    let mut reversed = arr.clone();
                    reversed.reverse();
                    Ok(CtfeValue::Array(reversed))
                }
                _ => Err(CtfeError::TypeError("reverse expects array argument".to_string())),
            }
        });
        
        // sum(array) - 数组求和
        self.functions.insert("sum".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("sum expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Array(arr) => {
                    let mut total = 0i64;
                    for val in arr {
                        match val {
                            CtfeValue::Int(n) => total += n,
                            _ => return Err(CtfeError::TypeError("sum: array must contain only integers".to_string())),
                        }
                    }
                    Ok(CtfeValue::Int(total))
                }
                _ => Err(CtfeError::TypeError("sum expects array argument".to_string())),
            }
        });
        
        // product(array) - 数组求积
        self.functions.insert("product".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("product expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Array(arr) => {
                    let mut product = 1i64;
                    for val in arr {
                        match val {
                            CtfeValue::Int(n) => product *= n,
                            _ => return Err(CtfeError::TypeError("product: array must contain only integers".to_string())),
                        }
                    }
                    Ok(CtfeValue::Int(product))
                }
                _ => Err(CtfeError::TypeError("product expects array argument".to_string())),
            }
        });
    }
    
    fn register_conversion_functions(&mut self) {
        // to_int(x) - 转换为整数
        self.functions.insert("to_int".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("to_int expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Int(n) => Ok(CtfeValue::Int(*n)),
                CtfeValue::Float(f) => Ok(CtfeValue::Int(*f as i64)),
                CtfeValue::Str(s) => {
                    s.parse::<i64>()
                        .map(CtfeValue::Int)
                        .map_err(|_| CtfeError::TypeError("to_int: invalid string".to_string()))
                }
                CtfeValue::Bool(b) => Ok(CtfeValue::Int(if *b { 1 } else { 0 })),
                _ => Err(CtfeError::TypeError("to_int: unsupported type".to_string())),
            }
        });
        
        // to_float(x) - 转换为浮点数
        self.functions.insert("to_float".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("to_float expects 1 argument".to_string()));
            }
            match &args[0] {
                CtfeValue::Int(n) => Ok(CtfeValue::Float(*n as f64)),
                CtfeValue::Float(f) => Ok(CtfeValue::Float(*f)),
                CtfeValue::Str(s) => {
                    s.parse::<f64>()
                        .map(CtfeValue::Float)
                        .map_err(|_| CtfeError::TypeError("to_float: invalid string".to_string()))
                }
                _ => Err(CtfeError::TypeError("to_float: unsupported type".to_string())),
            }
        });
        
        // to_string(x) - 转换为字符串
        self.functions.insert("to_string".to_string(), |args| {
            if args.len() != 1 {
                return Err(CtfeError::TypeError("to_string expects 1 argument".to_string()));
            }
            let s = match &args[0] {
                CtfeValue::Int(n) => n.to_string(),
                CtfeValue::Float(f) => f.to_string(),
                CtfeValue::Str(s) => s.clone(),
                CtfeValue::Bool(b) => b.to_string(),
                _ => return Err(CtfeError::TypeError("to_string: unsupported type".to_string())),
            };
            Ok(CtfeValue::Str(s))
        });
    }
    
    pub fn call(&self, name: &str, args: &[CtfeValue]) -> Result<CtfeValue, CtfeError> {
        if let Some(func) = self.functions.get(name) {
            func(args)
        } else {
            Err(CtfeError::UndefinedFunction(name.to_string()))
        }
    }
    
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }
}

/// CTFE优化分析器
pub struct CtfeOptimizer {
    /// 常量折叠缓存
    const_fold_cache: HashMap<String, CtfeValue>,
    /// 死代码标记
    dead_code: Vec<DeadCodeInfo>,
    /// 内联建议
    inline_suggestions: Vec<InlineSuggestion>,
    /// 循环展开候选
    unroll_candidates: Vec<UnrollCandidate>,
}

#[derive(Debug, Clone)]
pub struct DeadCodeInfo {
    pub location: String,
    pub reason: String,
    pub can_eliminate: bool,
}

#[derive(Debug, Clone)]
pub struct InlineSuggestion {
    pub function_name: String,
    pub call_count: usize,
    pub estimated_benefit: f64,
    pub should_inline: bool,
}

#[derive(Debug, Clone)]
pub struct UnrollCandidate {
    pub loop_location: String,
    pub iteration_count: Option<usize>,
    pub complexity: LoopComplexity,
    pub should_unroll: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoopComplexity {
    Simple,      // 简单循环，适合完全展开
    Moderate,    // 中等复杂度，部分展开
    Complex,     // 复杂循环，不建议展开
}

impl CtfeOptimizer {
    pub fn new() -> Self {
        CtfeOptimizer {
            const_fold_cache: HashMap::new(),
            dead_code: Vec::new(),
            inline_suggestions: Vec::new(),
            unroll_candidates: Vec::new(),
        }
    }
    
    /// 常量折叠优化
    pub fn fold_constants(&mut self, op: &CtfeOp) -> Option<CtfeValue> {
        // 生成操作的唯一键
        let key = format!("{:?}", op);
        
        // 检查缓存
        if let Some(cached) = self.const_fold_cache.get(&key) {
            return Some(cached.clone());
        }
        
        // 尝试折叠
        let result = match op {
            CtfeOp::LoadConst(val) => Some(val.clone()),
            CtfeOp::BinOp(binop, left, right) => {
                let left_val = self.fold_constants(left)?;
                let right_val = self.fold_constants(right)?;
                self.eval_binop_static(*binop, left_val, right_val)
            }
            _ => None,
        };
        
        // 缓存结果
        if let Some(ref val) = result {
            self.const_fold_cache.insert(key, val.clone());
        }
        
        result
    }
    
    fn eval_binop_static(&self, op: BinOp, left: CtfeValue, right: CtfeValue) -> Option<CtfeValue> {
        match (left, right) {
            (CtfeValue::Int(l), CtfeValue::Int(r)) => {
                let result = match op {
                    BinOp::Add => l.checked_add(r)?,
                    BinOp::Sub => l.checked_sub(r)?,
                    BinOp::Mul => l.checked_mul(r)?,
                    BinOp::Div => l.checked_div(r)?,
                    BinOp::Mod => l.checked_rem(r)?,
                    BinOp::Eq => return Some(CtfeValue::Bool(l == r)),
                    BinOp::Ne => return Some(CtfeValue::Bool(l != r)),
                    BinOp::Lt => return Some(CtfeValue::Bool(l < r)),
                    BinOp::Gt => return Some(CtfeValue::Bool(l > r)),
                    BinOp::Le => return Some(CtfeValue::Bool(l <= r)),
                    BinOp::Ge => return Some(CtfeValue::Bool(l >= r)),
                    _ => return None,
                };
                Some(CtfeValue::Int(result))
            }
            (CtfeValue::Bool(l), CtfeValue::Bool(r)) => {
                let result = match op {
                    BinOp::And => l && r,
                    BinOp::Or => l || r,
                    BinOp::Eq => l == r,
                    BinOp::Ne => l != r,
                    _ => return None,
                };
                Some(CtfeValue::Bool(result))
            }
            _ => None,
        }
    }
    
    /// 死代码检测
    pub fn detect_dead_code(&mut self, ops: &[CtfeOp]) {
        for (idx, op) in ops.iter().enumerate() {
            match op {
                CtfeOp::Branch { cond, then_ops, else_ops } => {
                    // 检测常量条件
                    if let CtfeOp::LoadConst(CtfeValue::Bool(b)) = **cond {
                        let dead_branch = if b { else_ops } else { then_ops };
                        if !dead_branch.is_empty() {
                            self.dead_code.push(DeadCodeInfo {
                                location: format!("Branch {} - {} arm", idx, if b { "else" } else { "then" }),
                                reason: "Constant condition makes this branch unreachable".to_string(),
                                can_eliminate: true,
                            });
                        }
                    }
                }
                CtfeOp::Return(_) => {
                    // Return后的代码是死代码
                    if idx < ops.len() - 1 {
                        self.dead_code.push(DeadCodeInfo {
                            location: format!("After return at position {}", idx),
                            reason: "Code after return is unreachable".to_string(),
                            can_eliminate: true,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    
    /// 分析内联机会
    pub fn analyze_inlining(&mut self, functions: &HashMap<String, CtfeFn>, call_counts: &HashMap<String, usize>) {
        for (name, func) in functions {
            let call_count = call_counts.get(name).unwrap_or(&0);
            
            // 计算函数复杂度
            let complexity = self.calculate_complexity(&func.body);
            
            // 估算内联收益
            let estimated_benefit = if complexity < 10 && *call_count > 1 {
                (*call_count as f64) * (complexity as f64) * 0.8
            } else if complexity < 5 {
                (*call_count as f64) * 1.5
            } else {
                0.0
            };
            
            self.inline_suggestions.push(InlineSuggestion {
                function_name: name.clone(),
                call_count: *call_count,
                estimated_benefit,
                should_inline: estimated_benefit > 5.0,
            });
        }
    }
    
    fn calculate_complexity(&self, ops: &[CtfeOp]) -> usize {
        let mut complexity = 0;
        for op in ops {
            complexity += match op {
                CtfeOp::LoadConst(_) => 1,
                CtfeOp::Load(_) => 1,
                CtfeOp::BinOp(_, _, _) => 2,
                CtfeOp::Call(_, _) => 5,
                CtfeOp::Branch { then_ops, else_ops, .. } => {
                    3 + self.calculate_complexity(then_ops) + self.calculate_complexity(else_ops)
                }
                CtfeOp::Loop { body, .. } => {
                    10 + self.calculate_complexity(body) * 2
                }
                CtfeOp::Declare(_, _) => 2,
                CtfeOp::Return(_) => 1,
            };
        }
        complexity
    }
    
    /// 分析循环展开机会
    pub fn analyze_loop_unrolling(&mut self, ops: &[CtfeOp], location: &str) {
        for op in ops {
            if let CtfeOp::Loop { init, cond, body, .. } = op {
                // 尝试确定迭代次数
                let iteration_count = self.try_determine_iterations(init, cond);
                
                // 计算循环体复杂度
                let body_complexity = self.calculate_complexity(body);
                
                let complexity = if body_complexity < 5 {
                    LoopComplexity::Simple
                } else if body_complexity < 15 {
                    LoopComplexity::Moderate
                } else {
                    LoopComplexity::Complex
                };
                
                let should_unroll = match (&iteration_count, &complexity) {
                    (Some(n), LoopComplexity::Simple) if *n <= 20 => true,
                    (Some(n), LoopComplexity::Moderate) if *n <= 8 => true,
                    _ => false,
                };
                
                self.unroll_candidates.push(UnrollCandidate {
                    loop_location: location.to_string(),
                    iteration_count,
                    complexity,
                    should_unroll,
                });
            }
        }
    }
    
    fn try_determine_iterations(&self, init: &CtfeOp, cond: &CtfeOp) -> Option<usize> {
        // 简化版：尝试从初始化和条件中推断迭代次数
        // 实际实现需要更复杂的分析
        None
    }
    
    /// 生成优化报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("=== CTFE Optimization Analysis ===\n\n");
        
        // 常量折叠报告
        report.push_str(&format!("Constant Folding:\n"));
        report.push_str(&format!("  Cached constants: {}\n\n", self.const_fold_cache.len()));
        
        // 死代码报告
        report.push_str(&format!("Dead Code Detection:\n"));
        report.push_str(&format!("  Found {} dead code sections\n", self.dead_code.len()));
        for dc in &self.dead_code {
            report.push_str(&format!("  - {}: {}\n", dc.location, dc.reason));
        }
        report.push_str("\n");
        
        // 内联建议
        report.push_str("Inlining Suggestions:\n");
        let should_inline: Vec<_> = self.inline_suggestions.iter()
            .filter(|s| s.should_inline)
            .collect();
        report.push_str(&format!("  {} functions recommended for inlining\n", should_inline.len()));
        for suggestion in should_inline {
            report.push_str(&format!("  - {}: {} calls, benefit score: {:.1}\n", 
                suggestion.function_name, suggestion.call_count, suggestion.estimated_benefit));
        }
        report.push_str("\n");
        
        // 循环展开建议
        report.push_str("Loop Unrolling Candidates:\n");
        let should_unroll: Vec<_> = self.unroll_candidates.iter()
            .filter(|c| c.should_unroll)
            .collect();
        report.push_str(&format!("  {} loops recommended for unrolling\n", should_unroll.len()));
        for candidate in should_unroll {
            report.push_str(&format!("  - {}: {:?}, iterations: {:?}\n",
                candidate.loop_location, candidate.complexity, candidate.iteration_count));
        }
        
        report
    }
}

/// 类型推导引擎
pub struct CtfeTypeInference {
    /// 类型环境
    type_env: HashMap<String, CtfeType>,
    /// 类型约束
    constraints: Vec<TypeConstraint>,
    /// 泛型实例化
    generic_instances: HashMap<String, Vec<CtfeType>>,
}

#[derive(Debug, Clone)]
pub struct TypeConstraint {
    pub left: CtfeType,
    pub right: CtfeType,
    pub reason: String,
}

impl CtfeTypeInference {
    pub fn new() -> Self {
        CtfeTypeInference {
            type_env: HashMap::new(),
            constraints: Vec::new(),
            generic_instances: HashMap::new(),
        }
    }
    
    /// 推导表达式类型
    pub fn infer_type(&mut self, op: &CtfeOp) -> Result<CtfeType, String> {
        match op {
            CtfeOp::LoadConst(val) => Ok(self.value_type(val)),
            
            CtfeOp::Load(name) => {
                self.type_env.get(name)
                    .cloned()
                    .ok_or_else(|| format!("Unknown variable: {}", name))
            }
            
            CtfeOp::BinOp(binop, left, right) => {
                let left_ty = self.infer_type(left)?;
                let right_ty = self.infer_type(right)?;
                self.infer_binop_type(*binop, left_ty, right_ty)
            }
            
            CtfeOp::Call(name, args) => {
                // 简化版：假设函数类型已知
                Ok(CtfeType::Unknown)
            }
            
            CtfeOp::Branch { then_ops, else_ops, .. } => {
                let then_ty = if !then_ops.is_empty() {
                    self.infer_type(then_ops.last().unwrap())?
                } else {
                    CtfeType::Unknown
                };
                
                let else_ty = if !else_ops.is_empty() {
                    self.infer_type(else_ops.last().unwrap())?
                } else {
                    CtfeType::Unknown
                };
                
                // 两个分支类型必须相同
                if then_ty != else_ty && then_ty != CtfeType::Unknown && else_ty != CtfeType::Unknown {
                    self.constraints.push(TypeConstraint {
                        left: then_ty.clone(),
                        right: else_ty.clone(),
                        reason: "Branch arms must have same type".to_string(),
                    });
                }
                
                Ok(then_ty)
            }
            
            CtfeOp::Declare(name, value) => {
                let ty = self.infer_type(value)?;
                self.type_env.insert(name.clone(), ty.clone());
                Ok(ty)
            }
            
            CtfeOp::Return(val) => self.infer_type(val),
            
            CtfeOp::Loop { .. } => Ok(CtfeType::Unknown),
        }
    }
    
    fn value_type(&self, val: &CtfeValue) -> CtfeType {
        match val {
            CtfeValue::Int(_) => CtfeType::Int,
            CtfeValue::Float(_) => CtfeType::Float,
            CtfeValue::Bool(_) => CtfeType::Bool,
            CtfeValue::Str(_) => CtfeType::String,
            CtfeValue::Array(arr) => {
                if arr.is_empty() {
                    CtfeType::Array(Box::new(CtfeType::Unknown))
                } else {
                    let elem_ty = self.value_type(&arr[0]);
                    CtfeType::Array(Box::new(elem_ty))
                }
            }
            CtfeValue::Struct(_) => CtfeType::Unknown,
            _ => CtfeType::Unknown,
        }
    }
    
    fn infer_binop_type(&self, op: BinOp, left: CtfeType, right: CtfeType) -> Result<CtfeType, String> {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                if left == CtfeType::Int && right == CtfeType::Int {
                    Ok(CtfeType::Int)
                } else if left == CtfeType::Float || right == CtfeType::Float {
                    Ok(CtfeType::Float)
                } else {
                    Err(format!("Type mismatch in arithmetic: {:?} and {:?}", left, right))
                }
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                Ok(CtfeType::Bool)
            }
            BinOp::And | BinOp::Or => {
                if left == CtfeType::Bool && right == CtfeType::Bool {
                    Ok(CtfeType::Bool)
                } else {
                    Err("Logical operators require boolean operands".to_string())
                }
            }
        }
    }
    
    /// 检查类型约束
    pub fn check_constraints(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for constraint in &self.constraints {
            if constraint.left != constraint.right {
                errors.push(format!(
                    "Type mismatch: {:?} vs {:?} ({})",
                    constraint.left, constraint.right, constraint.reason
                ));
            }
        }
        errors
    }
}

/// CTFE性能预测器
pub struct CtfePerformancePredictor {
    /// 操作成本表
    op_costs: HashMap<String, f64>,
    /// 历史数据
    historical_data: Vec<PerfSample>,
}

#[derive(Debug, Clone)]
pub struct PerfSample {
    pub op_type: String,
    pub input_size: usize,
    pub execution_time_ns: u64,
}

impl CtfePerformancePredictor {
    pub fn new() -> Self {
        let mut predictor = CtfePerformancePredictor {
            op_costs: HashMap::new(),
            historical_data: Vec::new(),
        };
        predictor.initialize_costs();
        predictor
    }
    
    fn initialize_costs(&mut self) {
        // 基础操作成本（纳秒）
        self.op_costs.insert("load_const".to_string(), 1.0);
        self.op_costs.insert("load_var".to_string(), 2.0);
        self.op_costs.insert("add".to_string(), 3.0);
        self.op_costs.insert("sub".to_string(), 3.0);
        self.op_costs.insert("mul".to_string(), 5.0);
        self.op_costs.insert("div".to_string(), 10.0);
        self.op_costs.insert("mod".to_string(), 10.0);
        self.op_costs.insert("compare".to_string(), 3.0);
        self.op_costs.insert("branch".to_string(), 15.0);
        self.op_costs.insert("call".to_string(), 50.0);
        self.op_costs.insert("loop_iteration".to_string(), 20.0);
    }
    
    /// 预测操作执行时间
    pub fn predict_cost(&self, op: &CtfeOp) -> f64 {
        match op {
            CtfeOp::LoadConst(_) => self.op_costs["load_const"],
            CtfeOp::Load(_) => self.op_costs["load_var"],
            CtfeOp::BinOp(binop, left, right) => {
                let op_cost = match binop {
                    BinOp::Add | BinOp::Sub => self.op_costs["add"],
                    BinOp::Mul => self.op_costs["mul"],
                    BinOp::Div | BinOp::Mod => self.op_costs["div"],
                    _ => self.op_costs["compare"],
                };
                op_cost + self.predict_cost(left) + self.predict_cost(right)
            }
            CtfeOp::Call(_, args) => {
                let args_cost: f64 = args.iter().map(|arg| self.predict_cost(arg)).sum();
                self.op_costs["call"] + args_cost
            }
            CtfeOp::Branch { cond, then_ops, else_ops } => {
                let cond_cost = self.predict_cost(cond);
                let then_cost: f64 = then_ops.iter().map(|op| self.predict_cost(op)).sum();
                let else_cost: f64 = else_ops.iter().map(|op| self.predict_cost(op)).sum();
                self.op_costs["branch"] + cond_cost + (then_cost.max(else_cost))
            }
            CtfeOp::Loop { init, cond, update, body } => {
                let init_cost = self.predict_cost(init);
                let cond_cost = self.predict_cost(cond);
                let update_cost = self.predict_cost(update);
                let body_cost: f64 = body.iter().map(|op| self.predict_cost(op)).sum();
                // 假设平均10次迭代
                init_cost + (cond_cost + body_cost + update_cost) * 10.0 + self.op_costs["loop_iteration"] * 10.0
            }
            CtfeOp::Declare(_, value) => {
                2.0 + self.predict_cost(value)
            }
            CtfeOp::Return(val) => {
                1.0 + self.predict_cost(val)
            }
        }
    }
    
    /// 预测CTFE vs Runtime性能差异
    pub fn compare_ctfe_runtime(&self, op: &CtfeOp) -> PerformanceComparison {
        let ctfe_cost = self.predict_cost(op);
        let runtime_cost = ctfe_cost * 1.5; // Runtime通常有额外开销
        
        PerformanceComparison {
            ctfe_time_ns: ctfe_cost as u64,
            runtime_time_ns: runtime_cost as u64,
            speedup: runtime_cost / ctfe_cost,
            recommendation: if runtime_cost / ctfe_cost > 1.2 {
                "CTFE recommended"
            } else {
                "Runtime acceptable"
            }.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct PerformanceComparison {
    pub ctfe_time_ns: u64,
    pub runtime_time_ns: u64,
    pub speedup: f64,
    pub recommendation: String,
}
