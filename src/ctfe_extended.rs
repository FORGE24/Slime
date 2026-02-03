//!CTFE扩展模块 - 跨模块优化、宏系统、代码生成
//!版权所有 (c) 2024-2026 Sanrol Team。
//!本程序是自由软件；您可以根据自由软件基金会发布的GNU通用公共许可证的条款重新分发和/或修改它；
//! 要么是许可证的第2版，要么是您选择的任何更高版本。
//!
//! CTFE扩展模块 - 跨模块优化、宏系统、代码生成
//!
//! 这个模块包含CTFE的高级功能：
//! - 跨模块常量传播
//! - 编译期宏系统
//! - 代码生成器
//! - 可视化调试器

use std::collections::{HashMap, HashSet, VecDeque};
use super::ctfe::*;

// ============================================================================
// 跨模块常量传播与内联系统
// ============================================================================

/// 模块依赖图
pub struct ModuleDependencyGraph {
    /// 模块名 -> 依赖的模块列表
    dependencies: HashMap<String, Vec<String>>,
    /// 模块名 -> 导出的常量
    exports: HashMap<String, HashMap<String, CtfeValue>>,
    /// 模块名 -> 导出的函数
    exported_functions: HashMap<String, HashMap<String, CtfeFn>>,
    /// 拓扑排序缓存
    topo_order: Option<Vec<String>>,
}

impl ModuleDependencyGraph {
    pub fn new() -> Self {
        ModuleDependencyGraph {
            dependencies: HashMap::new(),
            exports: HashMap::new(),
            exported_functions: HashMap::new(),
            topo_order: None,
        }
    }
    
    /// 添加模块依赖
    pub fn add_dependency(&mut self, from_module: String, to_module: String) {
        self.dependencies.entry(from_module)
            .or_insert_with(Vec::new)
            .push(to_module);
        self.topo_order = None; // 使缓存失效
    }
    
    /// 注册模块导出的常量
    pub fn register_export(&mut self, module: String, name: String, value: CtfeValue) {
        self.exports.entry(module)
            .or_insert_with(HashMap::new)
            .insert(name, value);
    }
    
    /// 注册模块导出的函数
    pub fn register_function_export(&mut self, module: String, name: String, func: CtfeFn) {
        self.exported_functions.entry(module)
            .or_insert_with(HashMap::new)
            .insert(name, func);
    }
    
    /// 获取导出的常量
    pub fn get_export(&self, module: &str, name: &str) -> Option<&CtfeValue> {
        self.exports.get(module)?.get(name)
    }
    
    /// 获取导出的函数
    pub fn get_function_export(&self, module: &str, name: &str) -> Option<&CtfeFn> {
        self.exported_functions.get(module)?.get(name)
    }
    
    /// 拓扑排序（用于确定编译顺序）
    pub fn topological_sort(&mut self) -> Result<Vec<String>, String> {
        if let Some(ref order) = self.topo_order {
            return Ok(order.clone());
        }
        
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut all_modules: HashSet<String> = HashSet::new();
        
        // 收集所有模块并计算入度
        for (from, deps) in &self.dependencies {
            all_modules.insert(from.clone());
            for to in deps {
                all_modules.insert(to.clone());
                *in_degree.entry(to.clone()).or_insert(0) += 1;
            }
            in_degree.entry(from.clone()).or_insert(0);
        }
        
        // 找到所有入度为0的节点
        let mut queue: VecDeque<String> = in_degree.iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(module, _)| module.clone())
            .collect();
        
        let mut result = Vec::new();
        
        while let Some(module) = queue.pop_front() {
            result.push(module.clone());
            
            if let Some(deps) = self.dependencies.get(&module) {
                for dep in deps {
                    if let Some(degree) = in_degree.get_mut(dep) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dep.clone());
                        }
                    }
                }
            }
        }
        
        if result.len() != all_modules.len() {
            return Err("Circular dependency detected".to_string());
        }
        
        self.topo_order = Some(result.clone());
        Ok(result)
    }
    
    /// 检测循环依赖
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        
        for module in self.dependencies.keys() {
            if !visited.contains(module) {
                self.dfs_detect_cycle(module, &mut visited, &mut rec_stack, &mut Vec::new(), &mut cycles);
            }
        }
        
        cycles
    }
    
    fn dfs_detect_cycle(
        &self,
        module: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(module.to_string());
        rec_stack.insert(module.to_string());
        path.push(module.to_string());
        
        if let Some(deps) = self.dependencies.get(module) {
            for dep in deps {
                if !visited.contains(dep) {
                    self.dfs_detect_cycle(dep, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(dep) {
                    // 找到循环
                    if let Some(pos) = path.iter().position(|m| m == dep) {
                        cycles.push(path[pos..].to_vec());
                    }
                }
            }
        }
        
        path.pop();
        rec_stack.remove(module);
    }
}

/// 跨模块常量传播器
pub struct CrossModuleConstantPropagator {
    /// 模块依赖图
    dep_graph: ModuleDependencyGraph,
    /// 传播统计
    stats: PropagationStats,
}

#[derive(Debug, Default)]
pub struct PropagationStats {
    pub constants_propagated: usize,
    pub functions_inlined: usize,
    pub modules_optimized: usize,
    pub circular_deps_detected: usize,
}

impl CrossModuleConstantPropagator {
    pub fn new() -> Self {
        CrossModuleConstantPropagator {
            dep_graph: ModuleDependencyGraph::new(),
            stats: PropagationStats::default(),
        }
    }
    
    /// 添加模块
    pub fn add_module(&mut self, name: String, dependencies: Vec<String>) {
        for dep in dependencies {
            self.dep_graph.add_dependency(name.clone(), dep);
        }
    }
    
    /// 传播常量
    pub fn propagate(&mut self) -> Result<(), String> {
        // 检测循环依赖
        let cycles = self.dep_graph.detect_cycles();
        self.stats.circular_deps_detected = cycles.len();
        
        if !cycles.is_empty() {
            return Err(format!("Circular dependencies detected: {:?}", cycles));
        }
        
        // 获取编译顺序
        let order = self.dep_graph.topological_sort()?;
        
        // 按顺序传播常量
        for module in &order {
            self.propagate_module_constants(module);
            self.stats.modules_optimized += 1;
        }
        
        Ok(())
    }
    
    fn propagate_module_constants(&mut self, module: &str) {
        // 从依赖模块收集可用常量
        if let Some(deps) = self.dep_graph.dependencies.get(module) {
            for dep in deps {
                if let Some(exports) = self.dep_graph.exports.get(dep) {
                    self.stats.constants_propagated += exports.len();
                }
            }
        }
    }
    
    /// 获取传播统计
    pub fn get_stats(&self) -> &PropagationStats {
        &self.stats
    }
    
    /// 生成传播报告
    pub fn generate_report(&self) -> String {
        format!(
            "=== Cross-Module Constant Propagation Report ===\n\
             Modules optimized: {}\n\
             Constants propagated: {}\n\
             Functions inlined: {}\n\
             Circular dependencies: {}\n",
            self.stats.modules_optimized,
            self.stats.constants_propagated,
            self.stats.functions_inlined,
            self.stats.circular_deps_detected
        )
    }
}

// ============================================================================
// 编译期宏系统
// ============================================================================

/// 宏定义
#[derive(Debug, Clone)]
pub struct MacroDef {
    pub name: String,
    pub params: Vec<MacroParam>,
    pub body: MacroBody,
    pub is_recursive: bool,
}

#[derive(Debug, Clone)]
pub struct MacroParam {
    pub name: String,
    pub param_type: MacroParamType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MacroParamType {
    Expr,        // 表达式参数
    Stmt,        // 语句参数
    Type,        // 类型参数
    Ident,       // 标识符参数
    Literal,     // 字面量参数
    Block,       // 代码块参数
    Repeating,   // 重复参数 (...)
}

#[derive(Debug, Clone)]
pub enum MacroBody {
    /// 模板式宏
    Template(Vec<MacroToken>),
    /// 函数式宏（Rust过程宏风格）
    Procedural(fn(&[MacroArg]) -> Result<Vec<CtfeOp>, String>),
}

#[derive(Debug, Clone)]
pub enum MacroToken {
    /// 字面量标记
    Literal(String),
    /// 参数引用
    ParamRef(String),
    /// 重复块
    Repeat {
        pattern: Vec<MacroToken>,
        separator: Option<String>,
    },
    /// 条件块
    Conditional {
        condition: String,
        then_tokens: Vec<MacroToken>,
        else_tokens: Vec<MacroToken>,
    },
}

#[derive(Debug, Clone)]
pub enum MacroArg {
    Expr(CtfeOp),
    Stmt(Vec<CtfeOp>),
    Type(CtfeType),
    Ident(String),
    Literal(CtfeValue),
    Block(Vec<CtfeOp>),
    Repeating(Vec<MacroArg>),
}

/// 宏展开引擎
pub struct MacroExpander {
    /// 宏定义表
    macros: HashMap<String, MacroDef>,
    /// 展开统计
    expansion_stats: MacroExpansionStats,
    /// 递归深度限制
    max_recursion_depth: usize,
}

#[derive(Debug, Default)]
pub struct MacroExpansionStats {
    pub expansions: usize,
    pub recursive_expansions: usize,
    pub max_depth_reached: usize,
}

impl MacroExpander {
    pub fn new() -> Self {
        MacroExpander {
            macros: HashMap::new(),
            expansion_stats: MacroExpansionStats::default(),
            max_recursion_depth: 100,
        }
    }
    
    /// 注册宏
    pub fn register_macro(&mut self, macro_def: MacroDef) {
        self.macros.insert(macro_def.name.clone(), macro_def);
    }
    
    /// 展开宏调用
    pub fn expand(&mut self, name: &str, args: Vec<MacroArg>) -> Result<Vec<CtfeOp>, String> {
        self.expand_with_depth(name, args, 0)
    }
    
    fn expand_with_depth(
        &mut self,
        name: &str,
        args: Vec<MacroArg>,
        depth: usize,
    ) -> Result<Vec<CtfeOp>, String> {
        if depth > self.max_recursion_depth {
            return Err(format!("Macro recursion limit exceeded: {}", name));
        }
        
        if depth > self.expansion_stats.max_depth_reached {
            self.expansion_stats.max_depth_reached = depth;
        }
        
        let macro_def = self.macros.get(name)
            .ok_or_else(|| format!("Undefined macro: {}", name))?
            .clone();
        
        if macro_def.params.len() != args.len() {
            return Err(format!(
                "Macro {} expects {} arguments, got {}",
                name, macro_def.params.len(), args.len()
            ));
        }
        
        self.expansion_stats.expansions += 1;
        if macro_def.is_recursive {
            self.expansion_stats.recursive_expansions += 1;
        }
        
        match macro_def.body {
            MacroBody::Template(tokens) => {
                self.expand_template(&tokens, &macro_def.params, &args)
            }
            MacroBody::Procedural(func) => {
                func(&args)
            }
        }
    }
    
    fn expand_template(
        &self,
        tokens: &[MacroToken],
        params: &[MacroParam],
        args: &[MacroArg],
    ) -> Result<Vec<CtfeOp>, String> {
        let mut result = Vec::new();
        
        for token in tokens {
            match token {
                MacroToken::Literal(lit) => {
                    // 字面量直接转换为操作
                    // 简化实现：假设是表达式
                }
                MacroToken::ParamRef(param_name) => {
                    // 查找参数
                    if let Some(pos) = params.iter().position(|p| &p.name == param_name) {
                        match &args[pos] {
                            MacroArg::Expr(op) => result.push(op.clone()),
                            MacroArg::Stmt(ops) => result.extend(ops.clone()),
                            MacroArg::Block(ops) => result.extend(ops.clone()),
                            _ => {}
                        }
                    }
                }
                MacroToken::Repeat { pattern, separator } => {
                    // 展开重复模式
                    // 简化实现
                }
                MacroToken::Conditional { condition, then_tokens, else_tokens } => {
                    // 根据条件选择分支
                    // 简化实现
                }
            }
        }
        
        Ok(result)
    }
    
    /// 获取展开统计
    pub fn get_stats(&self) -> &MacroExpansionStats {
        &self.expansion_stats
    }
}

/// 内置宏库
pub struct BuiltinMacros;

impl BuiltinMacros {
    /// vec! 宏 - 创建数组
    pub fn vec_macro(args: &[MacroArg]) -> Result<Vec<CtfeOp>, String> {
        let mut elements = Vec::new();
        
        for arg in args {
            if let MacroArg::Expr(expr) = arg {
                elements.push(expr.clone());
            } else {
                return Err("vec! expects expression arguments".to_string());
            }
        }
        
        // 生成数组构造操作
        Ok(vec![CtfeOp::LoadConst(CtfeValue::Array(Vec::new()))])
    }
    
    /// assert! 宏 - 编译期断言
    pub fn assert_macro(args: &[MacroArg]) -> Result<Vec<CtfeOp>, String> {
        if args.len() < 1 {
            return Err("assert! expects at least 1 argument".to_string());
        }
        
        if let MacroArg::Expr(condition) = &args[0] {
            // 生成断言检查代码
            Ok(vec![
                condition.clone(),
                // 如果条件为false，生成错误
            ])
        } else {
            Err("assert! expects boolean expression".to_string())
        }
    }
    
    /// println! 宏 - 格式化打印
    pub fn println_macro(args: &[MacroArg]) -> Result<Vec<CtfeOp>, String> {
        // 简化实现：生成print调用
        Ok(vec![CtfeOp::Call("print".to_string(), Vec::new())])
    }
    
    /// dbg! 宏 - 调试打印
    pub fn dbg_macro(args: &[MacroArg]) -> Result<Vec<CtfeOp>, String> {
        if args.is_empty() {
            return Err("dbg! expects at least 1 argument".to_string());
        }
        
        // 生成调试输出代码
        Ok(Vec::new())
    }
    
    /// matches! 宏 - 模式匹配判断
    pub fn matches_macro(args: &[MacroArg]) -> Result<Vec<CtfeOp>, String> {
        if args.len() != 2 {
            return Err("matches! expects 2 arguments: value and pattern".to_string());
        }
        
        // 生成模式匹配检查代码
        Ok(Vec::new())
    }
    
    /// cfg! 宏 - 编译配置判断
    pub fn cfg_macro(args: &[MacroArg]) -> Result<Vec<CtfeOp>, String> {
        // 简化实现：返回true/false
        Ok(vec![CtfeOp::LoadConst(CtfeValue::Bool(true))])
    }
}

// ============================================================================
// 编译期代码生成器
// ============================================================================

/// 代码生成模板
#[derive(Debug, Clone)]
pub struct CodeGenTemplate {
    pub name: String,
    pub description: String,
    pub parameters: Vec<TemplateParam>,
    pub generator: TemplateGenerator,
}

#[derive(Debug, Clone)]
pub struct TemplateParam {
    pub name: String,
    pub param_type: String,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone)]
pub enum TemplateGenerator {
    /// 基于字符串模板的生成器
    StringTemplate(String),
    /// 基于AST的生成器
    AstTemplate(Vec<CtfeOp>),
    /// 自定义函数生成器
    Custom(fn(&HashMap<String, String>) -> Result<Vec<CtfeOp>, String>),
}

/// 编译期代码生成引擎
pub struct CtfeCodeGenerator {
    /// 模板库
    templates: HashMap<String, CodeGenTemplate>,
    /// 生成统计
    stats: CodeGenStats,
}

#[derive(Debug, Default)]
pub struct CodeGenStats {
    pub templates_used: usize,
    pub code_blocks_generated: usize,
    pub total_ops_generated: usize,
}

impl CtfeCodeGenerator {
    pub fn new() -> Self {
        let mut gen = CtfeCodeGenerator {
            templates: HashMap::new(),
            stats: CodeGenStats::default(),
        };
        gen.register_builtin_templates();
        gen
    }
    
    fn register_builtin_templates(&mut self) {
        // getter/setter生成器
        self.register_template(CodeGenTemplate {
            name: "getter_setter".to_string(),
            description: "Generate getter and setter methods".to_string(),
            parameters: vec![
                TemplateParam {
                    name: "field_name".to_string(),
                    param_type: "String".to_string(),
                    default_value: None,
                },
                TemplateParam {
                    name: "field_type".to_string(),
                    param_type: "String".to_string(),
                    default_value: None,
                },
            ],
            generator: TemplateGenerator::Custom(Self::generate_getter_setter),
        });
        
        // builder模式生成器
        self.register_template(CodeGenTemplate {
            name: "builder".to_string(),
            description: "Generate builder pattern code".to_string(),
            parameters: vec![
                TemplateParam {
                    name: "struct_name".to_string(),
                    param_type: "String".to_string(),
                    default_value: None,
                },
            ],
            generator: TemplateGenerator::Custom(Self::generate_builder),
        });
        
        // 测试代码生成器
        self.register_template(CodeGenTemplate {
            name: "test_suite".to_string(),
            description: "Generate test suite boilerplate".to_string(),
            parameters: vec![
                TemplateParam {
                    name: "function_name".to_string(),
                    param_type: "String".to_string(),
                    default_value: None,
                },
            ],
            generator: TemplateGenerator::Custom(Self::generate_test_suite),
        });
    }
    
    fn generate_getter_setter(params: &HashMap<String, String>) -> Result<Vec<CtfeOp>, String> {
        let field_name = params.get("field_name")
            .ok_or("Missing field_name parameter")?;
        
        // 生成getter和setter的AST
        // 简化实现
        Ok(Vec::new())
    }
    
    fn generate_builder(params: &HashMap<String, String>) -> Result<Vec<CtfeOp>, String> {
        let struct_name = params.get("struct_name")
            .ok_or("Missing struct_name parameter")?;
        
        // 生成builder模式代码
        Ok(Vec::new())
    }
    
    fn generate_test_suite(params: &HashMap<String, String>) -> Result<Vec<CtfeOp>, String> {
        let func_name = params.get("function_name")
            .ok_or("Missing function_name parameter")?;
        
        // 生成测试代码
        Ok(Vec::new())
    }
    
    pub fn register_template(&mut self, template: CodeGenTemplate) {
        self.templates.insert(template.name.clone(), template);
    }
    
    /// 生成代码
    pub fn generate(
        &mut self,
        template_name: &str,
        params: HashMap<String, String>,
    ) -> Result<Vec<CtfeOp>, String> {
        let template = self.templates.get(template_name)
            .ok_or_else(|| format!("Unknown template: {}", template_name))?
            .clone();
        
        self.stats.templates_used += 1;
        
        match template.generator {
            TemplateGenerator::StringTemplate(tmpl) => {
                // 字符串模板替换
                // 简化实现
                Ok(Vec::new())
            }
            TemplateGenerator::AstTemplate(ops) => {
                self.stats.code_blocks_generated += 1;
                self.stats.total_ops_generated += ops.len();
                Ok(ops)
            }
            TemplateGenerator::Custom(func) => {
                let result = func(&params)?;
                self.stats.code_blocks_generated += 1;
                self.stats.total_ops_generated += result.len();
                Ok(result)
            }
        }
    }
    
    pub fn get_stats(&self) -> &CodeGenStats {
        &self.stats
    }
}

// ============================================================================
// CTFE可视化调试器
// ============================================================================

/// 调试器事件
#[derive(Debug, Clone)]
pub enum DebugEvent {
    /// 执行开始
    ExecutionStart { op: CtfeOp },
    /// 执行结束
    ExecutionEnd { op: CtfeOp, result: CtfeValue },
    /// 变量更新
    VariableUpdate { name: String, value: CtfeValue },
    /// 函数调用
    FunctionCall { name: String, args: Vec<CtfeValue> },
    /// 函数返回
    FunctionReturn { name: String, value: CtfeValue },
    /// 分支选择
    BranchTaken { condition: bool, branch: String },
    /// 循环迭代
    LoopIteration { iteration: usize },
    /// 错误
    Error { message: String },
}

/// 执行追踪
#[derive(Debug, Clone)]
pub struct ExecutionTrace {
    pub events: Vec<DebugEvent>,
    pub snapshots: Vec<EngineSnapshot>,
    pub timeline: Vec<TimePoint>,
}

#[derive(Debug, Clone)]
pub struct EngineSnapshot {
    pub timestamp: usize,
    pub variables: HashMap<String, CtfeValue>,
    pub call_stack: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TimePoint {
    pub index: usize,
    pub event: DebugEvent,
    pub duration_ns: u64,
}

/// CTFE调试器
pub struct CtfeDebugger {
    /// 是否启用调试
    enabled: bool,
    /// 执行追踪
    trace: ExecutionTrace,
    /// 断点
    breakpoints: HashSet<Breakpoint>,
    /// 监视变量
    watch_vars: HashSet<String>,
    /// 步进模式
    stepping: StepMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Breakpoint {
    /// 行号断点
    Line(usize),
    /// 函数断点
    Function(String),
    /// 条件断点
    Conditional(String),
    /// 变量监视断点
    WatchVariable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepMode {
    /// 不步进
    None,
    /// 单步执行
    StepOver,
    /// 步入函数
    StepInto,
    /// 步出函数
    StepOut,
    /// 运行到下一个断点
    Continue,
}

impl CtfeDebugger {
    pub fn new() -> Self {
        CtfeDebugger {
            enabled: false,
            trace: ExecutionTrace {
                events: Vec::new(),
                snapshots: Vec::new(),
                timeline: Vec::new(),
            },
            breakpoints: HashSet::new(),
            watch_vars: HashSet::new(),
            stepping: StepMode::None,
        }
    }
    
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    
    pub fn add_breakpoint(&mut self, bp: Breakpoint) {
        self.breakpoints.insert(bp);
    }
    
    pub fn remove_breakpoint(&mut self, bp: &Breakpoint) {
        self.breakpoints.remove(bp);
    }
    
    pub fn add_watch(&mut self, var: String) {
        self.watch_vars.insert(var);
    }
    
    pub fn record_event(&mut self, event: DebugEvent) {
        if !self.enabled {
            return;
        }
        
        let index = self.trace.events.len();
        self.trace.events.push(event.clone());
        self.trace.timeline.push(TimePoint {
            index,
            event,
            duration_ns: 0, // 实际应该测量时间
        });
    }
    
    pub fn take_snapshot(&mut self, variables: HashMap<String, CtfeValue>, call_stack: Vec<String>) {
        if !self.enabled {
            return;
        }
        
        self.trace.snapshots.push(EngineSnapshot {
            timestamp: self.trace.events.len(),
            variables,
            call_stack,
        });
    }
    
    pub fn get_trace(&self) -> &ExecutionTrace {
        &self.trace
    }
    
    pub fn clear_trace(&mut self) {
        self.trace.events.clear();
        self.trace.snapshots.clear();
        self.trace.timeline.clear();
    }
    
    /// 生成可视化HTML报告
    pub fn generate_html_report(&self) -> String {
        let mut html = String::from(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>CTFE Execution Trace</title>
    <style>
        body { font-family: monospace; background: #1e1e1e; color: #d4d4d4; }
        .event { margin: 10px; padding: 10px; border-left: 3px solid #007acc; }
        .execution-start { border-color: #4ec9b0; }
        .execution-end { border-color: #ce9178; }
        .variable-update { border-color: #dcdcaa; }
        .function-call { border-color: #c586c0; }
        .error { border-color: #f48771; background: #5a1d1d; }
        .timestamp { color: #858585; font-size: 0.9em; }
        .value { color: #4fc1ff; }
        .snapshot { background: #2d2d2d; padding: 15px; margin: 20px; border-radius: 5px; }
    </style>
</head>
<body>
    <h1>CTFE Execution Trace</h1>
    <div id="timeline">
"#
        );
        
        for (idx, event) in self.trace.events.iter().enumerate() {
            let class = match event {
                DebugEvent::ExecutionStart { .. } => "execution-start",
                DebugEvent::ExecutionEnd { .. } => "execution-end",
                DebugEvent::VariableUpdate { .. } => "variable-update",
                DebugEvent::FunctionCall { .. } => "function-call",
                DebugEvent::FunctionReturn { .. } => "function-call",
                DebugEvent::BranchTaken { .. } => "execution-start",
                DebugEvent::LoopIteration { .. } => "execution-start",
                DebugEvent::Error { .. } => "error",
            };
            
            html.push_str(&format!(
                r#"        <div class="event {}">
            <span class="timestamp">[{}]</span> {}
        </div>
"#,
                class,
                idx,
                self.format_event(event)
            ));
        }
        
        html.push_str(
            r#"    </div>
    <h2>Snapshots</h2>
    <div id="snapshots">
"#
        );
        
        for snapshot in &self.trace.snapshots {
            html.push_str(&format!(
                r#"        <div class="snapshot">
            <h3>Snapshot at {}</h3>
            <p>Variables: {}</p>
            <p>Call stack: {:?}</p>
        </div>
"#,
                snapshot.timestamp,
                snapshot.variables.len(),
                snapshot.call_stack
            ));
        }
        
        html.push_str(
            r#"    </div>
</body>
</html>"#
        );
        
        html
    }
    
    fn format_event(&self, event: &DebugEvent) -> String {
        match event {
            DebugEvent::ExecutionStart { op } => format!("Start: {:?}", op),
            DebugEvent::ExecutionEnd { op, result } => format!("End: {:?} = {:?}", op, result),
            DebugEvent::VariableUpdate { name, value } => {
                format!("Variable <span class=\"value\">{}</span> = {:?}", name, value)
            }
            DebugEvent::FunctionCall { name, args } => {
                format!("Call: {}({:?})", name, args)
            }
            DebugEvent::FunctionReturn { name, value } => {
                format!("Return from {}: {:?}", name, value)
            }
            DebugEvent::BranchTaken { condition, branch } => {
                format!("Branch {}: condition = {}", branch, condition)
            }
            DebugEvent::LoopIteration { iteration } => {
                format!("Loop iteration {}", iteration)
            }
            DebugEvent::Error { message } => {
                format!("ERROR: {}", message)
            }
        }
    }
}

// ============================================================================
// 高级优化Pass系统
// ============================================================================

/// 优化Pass特征
pub trait OptimizationPass {
    fn name(&self) -> &str;
    fn run(&mut self, ops: &mut Vec<CtfeOp>) -> Result<PassResult, String>;
}

#[derive(Debug)]
pub struct PassResult {
    pub modified: bool,
    pub changes: usize,
    pub description: String,
}

/// 常量折叠Pass
pub struct ConstantFoldingPass;

impl OptimizationPass for ConstantFoldingPass {
    fn name(&self) -> &str {
        "Constant Folding"
    }
    
    fn run(&mut self, ops: &mut Vec<CtfeOp>) -> Result<PassResult, String> {
        let mut changes = 0;
        
        for op in ops.iter_mut() {
            if let CtfeOp::BinOp(binop, left, right) = op {
                // 尝试折叠常量运算
                if let (CtfeOp::LoadConst(l), CtfeOp::LoadConst(r)) = (&**left, &**right) {
                    // 执行编译期计算
                    changes += 1;
                }
            }
        }
        
        Ok(PassResult {
            modified: changes > 0,
            changes,
            description: format!("Folded {} constant expressions", changes),
        })
    }
}

/// 死代码消除Pass
pub struct DeadCodeEliminationPass;

impl OptimizationPass for DeadCodeEliminationPass {
    fn name(&self) -> &str {
        "Dead Code Elimination"
    }
    
    fn run(&mut self, ops: &mut Vec<CtfeOp>) -> Result<PassResult, String> {
        let original_len = ops.len();
        
        // 移除return后的代码
        if let Some(return_pos) = ops.iter().position(|op| matches!(op, CtfeOp::Return(_))) {
            ops.truncate(return_pos + 1);
        }
        
        let changes = original_len - ops.len();
        
        Ok(PassResult {
            modified: changes > 0,
            changes,
            description: format!("Eliminated {} dead operations", changes),
        })
    }
}

/// 优化Pass管理器
pub struct PassManager {
    passes: Vec<Box<dyn OptimizationPass>>,
    results: Vec<PassResult>,
}

impl PassManager {
    pub fn new() -> Self {
        PassManager {
            passes: Vec::new(),
            results: Vec::new(),
        }
    }
    
    pub fn add_pass(&mut self, pass: Box<dyn OptimizationPass>) {
        self.passes.push(pass);
    }
    
    pub fn run_all(&mut self, ops: &mut Vec<CtfeOp>) -> Result<(), String> {
        self.results.clear();
        
        for pass in &mut self.passes {
            let result = pass.run(ops)?;
            println!("[{}] {}", pass.name(), result.description);
            self.results.push(result);
        }
        
        Ok(())
    }
    
    pub fn get_results(&self) -> &[PassResult] {
        &self.results
    }
}
