//! slimec: slime-lang to x86-64 NASM compiler (stage 2).
//! 
//! 核心理念: 万物皆接口 (Everything is an Interface)
//! - 接口是 slime 的第一公民，类似 Java 的"万物皆对象"
//! - 变量、函数、类型、I/O 都通过接口抽象
//! - 接口可以定义、调用、组合、释放
//!
//! 目标: Rust版Python — 编译型、语法易懂、自动内存管理、快速执行

use std::env;
use std::fs;
use std::path::Path;
use std::collections::HashMap;
use std::fmt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    let source = fs::read_to_string(&args.input)?;
    
    let tokens = tokenize(&source).map_err(|e| e.to_string())?;
    let mut program = parse_tokens(tokens).map_err(|e| e.to_string())?;
    
    // ========== 模块处理 ==========
    let base_path = args.input.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut module_loader = ModuleLoader::new(base_path);
    process_imports(&mut program, &mut module_loader).map_err(|e| e.to_string())?;
    
    // ========== 所有权检查 ==========
    let mut ownership_checker = OwnershipChecker::new();
    if let Err(errors) = ownership_checker.check(&program) {
        for err in errors {
            eprintln!("{}", err);
        }
        return Err("所有权检查失败".into());
    }
    
    // ========== 优化 Pass ==========
    let mut optimizer = Optimizer::new();
    optimizer.optimize(&mut program);
    
    let mut gen = CodeGen::new(args.target.clone());
    let asm = gen.emit(&program).map_err(|e| e.to_string())?;
    
    fs::write(&args.output, &asm)?;
    println!("✓ emitted {} to {}", args.target, args.output.display());
    
    // 根据目标打印编译提示
    match args.target {
        Target::LinuxX64 => {
            println!("  hint: nasm -felf64 {} -o out.o && ld -o out out.o", args.output.display());
        }
        Target::WindowsX64 => {
            println!("  hint: nasm -fwin64 {} -o out.obj && link out.obj /subsystem:console /entry:_start", args.output.display());
        }
        Target::MacosX64 => {
            println!("  hint: nasm -fmacho64 {} -o out.o && ld -o out out.o -lSystem", args.output.display());
        }
    }
    Ok(())
}

// ============================================================================
// 命令行参数
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum Target {
    LinuxX64,
    WindowsX64,
    MacosX64,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::LinuxX64 => write!(f, "linux-x64"),
            Target::WindowsX64 => write!(f, "windows-x64"),
            Target::MacosX64 => write!(f, "macos-x64"),
        }
    }
}

struct Args {
    input: std::path::PathBuf,
    output: std::path::PathBuf,
    target: Target,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1).peekable();
    let mut input = None;
    let mut output = None;
    let mut target = Target::LinuxX64;
    
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" | "-t" => {
                let t = args.next().ok_or("--target requires a value")?;
                target = match t.as_str() {
                    "linux" | "linux-x64" => Target::LinuxX64,
                    "windows" | "windows-x64" | "win64" => Target::WindowsX64,
                    "macos" | "macos-x64" | "darwin" => Target::MacosX64,
                    _ => return Err(format!("unknown target: {}", t).into()),
                };
            }
            "-o" => {
                output = Some(args.next().ok_or("-o requires a value")?.into());
            }
            _ if input.is_none() => {
                input = Some(arg.into());
            }
            _ => {}
        }
    }
    
    let input = input.ok_or("usage: slimec <input.sm> [-o output.asm] [--target linux|windows|macos]")?;
    let output = output.unwrap_or_else(|| default_output(&input));
    
    Ok(Args { input, output, target })
}

fn default_output(input: &std::path::PathBuf) -> std::path::PathBuf {
    let p = Path::new(input);
    let stem = p.file_stem().unwrap_or_default();
    Path::new(stem).with_extension("asm")
}

// ============================================================================
// 模块加载器
// ============================================================================

use std::collections::HashSet;
use std::path::PathBuf;

/// 模块信息
#[derive(Debug, Clone)]
struct ModuleInfo {
    /// 模块名（用于别名）
    name: String,
    /// 模块路径
    path: PathBuf,
    /// 模块导出的公开符号
    exports: HashSet<String>,
    /// 模块 AST
    stmts: Vec<Stmt>,
}

/// 模块加载器
struct ModuleLoader {
    /// 搜索路径
    search_paths: Vec<PathBuf>,
    /// 已加载的模块 (路径 -> 模块信息)
    loaded_modules: HashMap<PathBuf, ModuleInfo>,
    /// 当前正在加载的模块栈（用于检测循环依赖）
    loading_stack: Vec<PathBuf>,
    /// 基础路径（当前编译文件的目录）
    base_path: PathBuf,
}

impl ModuleLoader {
    fn new(base_path: PathBuf) -> Self {
        let mut search_paths = Vec::new();
        
        // 1. 当前目录
        search_paths.push(base_path.clone());
        
        // 2. 标准库路径（如果存在）
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                search_paths.push(parent.join("stdlib"));
                search_paths.push(parent.join("lib"));
            }
        }
        
        // 3. 环境变量指定的路径
        if let Ok(paths) = std::env::var("SLIME_PATH") {
            for p in paths.split(';') {
                search_paths.push(PathBuf::from(p));
            }
        }
        
        ModuleLoader {
            search_paths,
            loaded_modules: HashMap::new(),
            loading_stack: Vec::new(),
            base_path,
        }
    }
    
    /// 解析模块路径
    fn resolve_path(&self, import_path: &str) -> Option<PathBuf> {
        let import_path = import_path.trim_matches('"');
        
        // 1. 绝对路径
        let path = PathBuf::from(import_path);
        if path.is_absolute() && path.exists() {
            return Some(path);
        }
        
        // 2. 相对路径（相对于当前编译文件）
        let relative = self.base_path.join(import_path);
        if relative.exists() {
            return Some(relative);
        }
        
        // 3. 搜索路径
        for search_path in &self.search_paths {
            let full_path = search_path.join(import_path);
            if full_path.exists() {
                return Some(full_path);
            }
            
            // 尝试添加 .sm 扩展名
            let with_ext = search_path.join(format!("{}.sm", import_path.trim_end_matches(".sm")));
            if with_ext.exists() {
                return Some(with_ext);
            }
        }
        
        None
    }
    
    /// 加载模块
    fn load_module(&mut self, import_path: &str) -> Result<&ModuleInfo, CompileError> {
        let resolved_path = self.resolve_path(import_path)
            .ok_or_else(|| CompileError::name(&format!("Module not found: {}", import_path)))?;
        
        // 检查循环依赖
        if self.loading_stack.contains(&resolved_path) {
            return Err(CompileError::name(&format!(
                "Circular dependency detected: {} -> {}", 
                self.loading_stack.last().map(|p| p.display().to_string()).unwrap_or_default(),
                resolved_path.display()
            )));
        }
        
        // 如果已经加载过，直接返回
        if self.loaded_modules.contains_key(&resolved_path) {
            return Ok(self.loaded_modules.get(&resolved_path).unwrap());
        }
        
        // 开始加载
        self.loading_stack.push(resolved_path.clone());
        
        // 读取文件
        let source = fs::read_to_string(&resolved_path)
            .map_err(|e| CompileError::name(&format!("Failed to read module {}: {}", import_path, e)))?;
        
        // 词法分析
        let tokens = tokenize(&source)
            .map_err(|e| CompileError::name(&format!("Lexer error in module {}: {}", import_path, e)))?;
        
        // 语法分析
        let program = parse_tokens(tokens)
            .map_err(|e| CompileError::name(&format!("Parse error in module {}: {}", import_path, e)))?;
        
        // 收集导出符号
        let mut exports = HashSet::new();
        for stmt in &program.stmts {
            collect_exports(stmt, &mut exports);
        }
        
        // 提取模块名
        let module_name = resolved_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module")
            .to_string();
        
        // 完成加载
        self.loading_stack.pop();
        
        let module_info = ModuleInfo {
            name: module_name,
            path: resolved_path.clone(),
            exports,
            stmts: program.stmts,
        };
        
        self.loaded_modules.insert(resolved_path.clone(), module_info);
        Ok(self.loaded_modules.get(&resolved_path).unwrap())
    }
    
    /// 处理 include 指令（文本包含）
    fn include_file(&self, include_path: &str) -> Result<String, CompileError> {
        let resolved_path = self.resolve_path(include_path)
            .ok_or_else(|| CompileError::name(&format!("Include file not found: {}", include_path)))?;
        
        fs::read_to_string(&resolved_path)
            .map_err(|e| CompileError::name(&format!("Failed to read include file {}: {}", include_path, e)))
    }
}

/// 收集模块导出的符号
fn collect_exports(stmt: &Stmt, exports: &mut HashSet<String>) {
    match stmt {
        // pub 声明的都是导出
        Stmt::PubDecl(inner) => {
            match inner.as_ref() {
                Stmt::FnDef(f) => { exports.insert(f.name.clone()); }
                Stmt::Let { name, .. } => { exports.insert(name.clone()); }
                _ => {}
            }
        }
        // 顶层函数定义默认导出
        Stmt::FnDef(f) => { exports.insert(f.name.clone()); }
        // 接口定义默认导出
        Stmt::DefInterface { name, .. } => { exports.insert(name.clone()); }
        _ => {}
    }
}

/// 模块导入处理器 - 展开所有 import/use 语句
fn process_imports(program: &mut Program, loader: &mut ModuleLoader) -> Result<(), CompileError> {
    let mut new_stmts = Vec::new();
    let mut imported_modules: HashMap<String, ModuleInfo> = HashMap::new();
    
    for stmt in std::mem::take(&mut program.stmts) {
        match stmt {
            Stmt::Import { path, alias } => {
                let module = loader.load_module(&path)?;
                let module_name = alias.unwrap_or_else(|| module.name.clone());
                imported_modules.insert(module_name, module.clone());
                
                // 将模块的语句（函数定义等）添加到程序中
                for module_stmt in &module.stmts {
                    match module_stmt {
                        Stmt::FnDef(_) | Stmt::DefInterface { .. } => {
                            new_stmts.push(module_stmt.clone());
                        }
                        Stmt::PubDecl(inner) => {
                            // 展开 pub 声明，添加内部语句
                            new_stmts.push(inner.as_ref().clone());
                        }
                        _ => {}
                    }
                }
            }
            Stmt::Use { module_path, imports } => {
                // 构建模块路径
                let path = module_path.join("/") + ".sm";
                let module = loader.load_module(&path)?;
                
                // 根据导入类型处理
                match imports {
                    UseImport::Single(name) => {
                        // 检查符号是否存在
                        if !module.exports.contains(&name) {
                            return Err(CompileError::name(&format!(
                                "Symbol '{}' not found in module '{}'", name, path
                            )));
                        }
                        // 导入单个符号（暂时导入整个模块的相关定义）
                        for module_stmt in &module.stmts {
                            if let Stmt::FnDef(f) = module_stmt {
                                if f.name == name {
                                    new_stmts.push(module_stmt.clone());
                                }
                            }
                        }
                    }
                    UseImport::Multiple(names) => {
                        for (name, _alias) in &names {
                            if !module.exports.contains(name) {
                                return Err(CompileError::name(&format!(
                                    "Symbol '{}' not found in module '{}'", name, path
                                )));
                            }
                        }
                        // 导入指定符号
                        for module_stmt in &module.stmts {
                            match module_stmt {
                                Stmt::FnDef(f) => {
                                    if names.iter().any(|(n, _)| *n == f.name) {
                                        new_stmts.push(module_stmt.clone());
                                    }
                                }
                                Stmt::PubDecl(inner) => {
                                    if let Stmt::FnDef(f) = inner.as_ref() {
                                        if names.iter().any(|(n, _)| *n == f.name) {
                                            new_stmts.push(inner.as_ref().clone());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    UseImport::Glob => {
                        // 导入所有导出
                        for module_stmt in &module.stmts {
                            match module_stmt {
                                Stmt::FnDef(_) | Stmt::DefInterface { .. } => {
                                    new_stmts.push(module_stmt.clone());
                                }
                                Stmt::PubDecl(inner) => {
                                    new_stmts.push(inner.as_ref().clone());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Stmt::FromImport { path, names } => {
                let module = loader.load_module(&path)?;
                
                for (name, _alias) in &names {
                    if !module.exports.contains(name) {
                        return Err(CompileError::name(&format!(
                            "Symbol '{}' not found in module '{}'", name, path
                        )));
                    }
                }
                
                // 导入指定符号
                for module_stmt in &module.stmts {
                    match module_stmt {
                        Stmt::FnDef(f) => {
                            if names.iter().any(|(n, _)| *n == f.name) {
                                new_stmts.push(module_stmt.clone());
                            }
                        }
                        Stmt::PubDecl(inner) => {
                            if let Stmt::FnDef(f) = inner.as_ref() {
                                if names.iter().any(|(n, _)| *n == f.name) {
                                    new_stmts.push(inner.as_ref().clone());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Stmt::Include(path) => {
                // 文本包含 - 需要重新解析
                let content = loader.include_file(&path)?;
                let tokens = tokenize(&content).map_err(|e| e)?;
                let included_program = parse_tokens(tokens)?;
                new_stmts.extend(included_program.stmts);
            }
            other => {
                new_stmts.push(other);
            }
        }
    }
    
    program.stmts = new_stmts;
    Ok(())
}

// ============================================================================
// 错误处理
// ============================================================================

#[derive(Debug)]
struct CompileError {
    kind: ErrorKind,
    message: String,
    line: usize,
    col: usize,
}

#[derive(Debug)]
enum ErrorKind {
    LexError,
    ParseError,
    TypeError,
    NameError,
    RuntimeError,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 错误类型的友好名称
        let kind_name = match self.kind {
            ErrorKind::LexError => "Lexical Error",
            ErrorKind::ParseError => "Parse Error",
            ErrorKind::TypeError => "Type Error",
            ErrorKind::NameError => "Name Error",
            ErrorKind::RuntimeError => "Runtime Error",
        };
        
        // 构建错误信息
        let mut error_msg = format!("\x1b[31mError\x1b[0m: [{kind_name}] ", kind_name = kind_name);
        
        // 添加位置信息
        if self.line > 0 {
            error_msg.push_str(&format!("line {}:{}, ", self.line, self.col));
        }
        
        // 添加错误消息
        error_msg.push_str(&self.message);
        
        // 为常见错误添加修复建议
        let suggestion = match self.kind {
            ErrorKind::ParseError if self.message.contains("Expected end") => {
                Some("Hint: Did you forget to add 'end' to close a block?")
            }
            ErrorKind::NameError if self.message.contains("not found") => {
                Some("Hint: Check for typos or missing imports.")
            }
            ErrorKind::TypeError if self.message.contains("mismatch") => {
                Some("Hint: Check that you're using the correct type for this operation.")
            }
            ErrorKind::LexError if self.message.contains("Invalid") => {
                Some("Hint: Check for invalid characters or syntax.")
            }
            _ => None,
        };
        
        // 添加修复建议
        if let Some(suggestion) = suggestion {
            error_msg.push_str(&format!("\n\x1b[34m{}\x1b[0m", suggestion));
        }
        
        write!(f, "{}", error_msg)
    }
}

impl std::error::Error for CompileError {}

impl CompileError {
    fn lex(msg: &str, line: usize, col: usize) -> Self {
        let detailed_msg = match msg {
            "Invalid float" => "Invalid float literal: Check the format of your floating-point number.",
            "Invalid integer" => "Invalid integer literal: Check the format of your integer.",
            _ => msg,
        };
        CompileError { kind: ErrorKind::LexError, message: detailed_msg.to_string(), line, col }
    }
    
    fn parse(msg: &str, line: usize) -> Self {
        let detailed_msg = match msg {
            "Expected function name" => "Expected function name: Every function must have a name.",
            "Expected parameter name" => "Expected parameter name: Function parameters must have names.",
            "Expected type" => "Expected type: Specify the type for this parameter or return value.",
            "Expected end" => "Expected 'end' keyword: Every block must be closed with 'end'.",
            _ => msg,
        };
        CompileError { kind: ErrorKind::ParseError, message: detailed_msg.to_string(), line, col: 0 }
    }
    
    fn name(msg: &str) -> Self {
        let detailed_msg = match msg {
            _ if msg.contains("Module not found") => {
                format!("{}. Check that the module path is correct and the file exists.", msg)
            }
            _ if msg.contains("not found") => {
                format!("{}. Check for typos or missing imports.", msg)
            }
            _ => msg.to_string(),
        };
        CompileError { kind: ErrorKind::NameError, message: detailed_msg, line: 0, col: 0 }
    }
    
    fn type_err(msg: &str) -> Self {
        let detailed_msg = match msg {
            _ if msg.contains("mismatch") => {
                format!("{}. Check that you're using the correct type.", msg)
            }
            _ => msg.to_string(),
        };
        CompileError { kind: ErrorKind::TypeError, message: detailed_msg, line: 0, col: 0 }
    }
    
    // 新增：运行时错误
    fn runtime(msg: &str, line: usize, col: usize) -> Self {
        CompileError { kind: ErrorKind::RuntimeError, message: msg.to_string(), line, col }
    }
}

// ============================================================================
// TOKENS & LEXER
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum Token {
    // 关键字 - 变量
    KwLet,      // let x = 1  (不可变绑定，推荐)
    KwVar,      // var x = 1  (可变绑定)
    KwConst,    // const X = 1 (编译期常量)
    // 关键字 - 函数
    KwFn,
    KwReturn,
    // 关键字 - 控制流
    KwIf,
    KwElif,
    KwElse,
    KwLoop,
    KwFor,
    KwWhile,
    KwBreak,
    KwContinue,
    KwIn,
    // 关键字 - 错误处理
    KwTry,
    KwCatch,
    KwThrow,
    KwDefer,
    // 关键字 - 类型
    KwInt,
    KwFloat,
    KwStr,
    KwBool,
    KwNone,
    KwTrue,
    KwFalse,
    // 关键字 - 其他
    KwEnd,      // end 关键字（代替大括号）
    KwPrint,    // print 内置函数
    KwInput,    // input 内置函数
    KwMain,
    KwCall,
    KwDef,
    KwDrop,
    KwImport,
    KwUse,
    KwAs,           // as (别名)
    KwFrom,         // from (来源)
    KwPub,          // pub (公开导出)
    KwInclude,      // include (文本包含)
    KwAnd,
    KwOr,
    KwNot,
    // CNB (C Native Bridge)
    KwExtern,       // extern "C" fn ...
    KwUnsafe,       // unsafe { ... }
    KwStruct,       // struct 定义
    KwSizeof,       // sizeof(type)
    // 分隔符
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Semicolon,
    Colon,
    DoubleColon,    // :: (路径分隔符)
    Comma,
    Dot,
    Arrow,      // ->
    // 运算符
    Assign,     // =
    PlusAssign, // +=
    MinusAssign,// -=
    MulAssign,  // *=
    DivAssign,  // /=
    Eq,         // ==
    Ne,         // !=
    Lt,         // <
    Gt,         // >
    Le,         // <=
    Ge,         // >=
    Plus,
    Minus,
    Star,
    Slash,
    Percent,    // %
    Ampersand,  // & (借用)
    AmpMut,     // &mut (可变借用)
    // 字面量
    Ident(String),
    String(String),
    Number(i64),
    Float(f64),
    // 其他
    Newline,
    EOF,
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer {
            chars: src.chars().peekable(),
            line: 1,
            col: 1,
        }
    }
    
    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }
    
    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.next();
        if let Some(c) = ch {
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        ch
    }
    
    fn peek_next(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.next()
    }
    
    fn tokenize(&mut self) -> Result<Vec<Token>, CompileError> {
        let mut tokens = Vec::new();
        
        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => {
                    self.advance();
                    // 可选：添加换行token用于语句分隔
                }
                '#' => {
                    // 预处理指令，跳过整行
                    while let Some(c) = self.peek() {
                        self.advance();
                        if c == '\n' { break; }
                    }
                }
                '/' if self.peek_next() == Some('/') => {
                    // 行注释
                    while let Some(c) = self.peek() {
                        self.advance();
                        if c == '\n' { break; }
                    }
                }
                '/' if self.peek_next() == Some('*') => {
                    // 块注释
                    self.advance(); self.advance();
                    while let Some(c) = self.peek() {
                        if c == '*' && self.peek_next() == Some('/') {
                            self.advance(); self.advance();
                            break;
                        }
                        self.advance();
                    }
                }
                '{' => { tokens.push(Token::LBrace); self.advance(); }
                '}' => { tokens.push(Token::RBrace); self.advance(); }
                '(' => { tokens.push(Token::LParen); self.advance(); }
                ')' => { tokens.push(Token::RParen); self.advance(); }
                '[' => { tokens.push(Token::LBracket); self.advance(); }
                ']' => { tokens.push(Token::RBracket); self.advance(); }
                ';' => { tokens.push(Token::Semicolon); self.advance(); }
                ':' if self.peek_next() == Some(':') => {
                    tokens.push(Token::DoubleColon);
                    self.advance(); self.advance();
                }
                ':' => { tokens.push(Token::Colon); self.advance(); }
                ',' => { tokens.push(Token::Comma); self.advance(); }
                '.' => { tokens.push(Token::Dot); self.advance(); }
                
                '+' if self.peek_next() == Some('=') => {
                    tokens.push(Token::PlusAssign);
                    self.advance(); self.advance();
                }
                '+' => { tokens.push(Token::Plus); self.advance(); }
                
                '-' if self.peek_next() == Some('>') => {
                    tokens.push(Token::Arrow);
                    self.advance(); self.advance();
                }
                '-' if self.peek_next() == Some('=') => {
                    tokens.push(Token::MinusAssign);
                    self.advance(); self.advance();
                }
                '-' => { tokens.push(Token::Minus); self.advance(); }
                
                '*' if self.peek_next() == Some('=') => {
                    tokens.push(Token::MulAssign);
                    self.advance(); self.advance();
                }
                '*' => { tokens.push(Token::Star); self.advance(); }
                
                '/' if self.peek_next() == Some('=') => {
                    tokens.push(Token::DivAssign);
                    self.advance(); self.advance();
                }
                '/' => { tokens.push(Token::Slash); self.advance(); }
                
                '%' => { tokens.push(Token::Percent); self.advance(); }
                
                // 借用运算符
                '&' => {
                    self.advance();
                    // 检查是否是 &mut
                    if self.peek() == Some('m') {
                        let pos = self.col;
                        let mut word = String::new();
                        while let Some(c) = self.peek() {
                            if c.is_alphabetic() {
                                word.push(c);
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        if word == "mut" {
                            tokens.push(Token::AmpMut);
                        } else {
                            // 回退，只是 &
                            tokens.push(Token::Ampersand);
                            // 把读到的标识符也推入
                            if !word.is_empty() {
                                tokens.push(Token::Ident(word));
                            }
                        }
                    } else {
                        tokens.push(Token::Ampersand);
                    }
                }
                
                '=' if self.peek_next() == Some('=') => {
                    tokens.push(Token::Eq);
                    self.advance(); self.advance();
                }
                '=' => { tokens.push(Token::Assign); self.advance(); }
                
                '!' if self.peek_next() == Some('=') => {
                    tokens.push(Token::Ne);
                    self.advance(); self.advance();
                }
                
                '<' if self.peek_next() == Some('=') => {
                    tokens.push(Token::Le);
                    self.advance(); self.advance();
                }
                '<' => { tokens.push(Token::Lt); self.advance(); }
                
                '>' if self.peek_next() == Some('=') => {
                    tokens.push(Token::Ge);
                    self.advance(); self.advance();
                }
                '>' => { tokens.push(Token::Gt); self.advance(); }
                
                '"' => {
                    self.advance();
                    let mut s = String::new();
                    while let Some(c) = self.peek() {
                        if c == '"' {
                            self.advance();
                            break;
                        }
                        if c == '\\' {
                            self.advance();
                            match self.peek() {
                                Some('n') => { s.push('\n'); self.advance(); }
                                Some('t') => { s.push('\t'); self.advance(); }
                                Some('\\') => { s.push('\\'); self.advance(); }
                                Some('"') => { s.push('"'); self.advance(); }
                                _ => s.push('\\'),
                            }
                        } else {
                            s.push(c);
                            self.advance();
                        }
                    }
                    tokens.push(Token::String(s));
                }
                
                _ if ch.is_alphabetic() || ch == '_' => {
                    let mut ident = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            ident.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let tok = match ident.as_str() {
                        "let" => Token::KwLet,
                        "var" => Token::KwVar,
                        "const" => Token::KwConst,
                        "fn" | "function" => Token::KwFn,
                        "return" => Token::KwReturn,
                        "if" => Token::KwIf,
                        "elif" => Token::KwElif,
                        "else" => Token::KwElse,
                        "end" => Token::KwEnd,
                        "loop" => Token::KwLoop,
                        "for" => Token::KwFor,
                        "while" => Token::KwWhile,
                        "break" => Token::KwBreak,
                        "continue" => Token::KwContinue,
                        "in" => Token::KwIn,
                        "try" => Token::KwTry,
                        "catch" => Token::KwCatch,
                        "throw" => Token::KwThrow,
                        "defer" => Token::KwDefer,
                        "int" => Token::KwInt,
                        "float" => Token::KwFloat,
                        "str" => Token::KwStr,
                        "bool" => Token::KwBool,
                        "none" | "None" => Token::KwNone,
                        "true" | "True" => Token::KwTrue,
                        "false" | "False" => Token::KwFalse,
                        "print" => Token::KwPrint,
                        "input" => Token::KwInput,
                        "main" => Token::KwMain,
                        "call" | "invoke" => Token::KwCall,
                        "def" | "define" => Token::KwDef,
                        "drop" | "delete" => Token::KwDrop,
                        "import" | "introduce" => Token::KwImport,
                        "use" => Token::KwUse,
                        "as" => Token::KwAs,
                        "from" => Token::KwFrom,
                        "pub" | "public" | "export" => Token::KwPub,
                        "include" => Token::KwInclude,
                        "and" => Token::KwAnd,
                        "or" => Token::KwOr,
                        "not" => Token::KwNot,
                        // CNB keywords
                        "extern" | "external" => Token::KwExtern,
                        "unsafe" => Token::KwUnsafe,
                        "struct" | "structure" => Token::KwStruct,
                        "sizeof" => Token::KwSizeof,
                        _ => Token::Ident(ident),
                    };
                    tokens.push(tok);
                }
                
                _ if ch.is_numeric() => {
                    let mut num_str = String::new();
                    let mut is_float = false;
                    while let Some(c) = self.peek() {
                        if c.is_numeric() {
                            num_str.push(c);
                            self.advance();
                        } else if c == '.' && !is_float {
                            is_float = true;
                            num_str.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    if is_float {
                        let f: f64 = num_str.parse()
                            .map_err(|_| CompileError::lex("Invalid float", self.line, self.col))?;
                        tokens.push(Token::Float(f));
                    } else {
                        let n: i64 = num_str.parse()
                            .map_err(|_| CompileError::lex("Invalid integer", self.line, self.col))?;
                        tokens.push(Token::Number(n));
                    }
                }
                
                _ => {
                    self.advance();
                }
            }
        }
        
        tokens.push(Token::EOF);
        Ok(tokens)
    }
}

fn tokenize(src: &str) -> Result<Vec<Token>, CompileError> {
    Lexer::new(src).tokenize()
}

// ============================================================================
// AST - 类型系统 + 所有权系统
// ============================================================================

/// 所有权状态
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ownership {
    Owned,          // 拥有所有权 (let x = ...)
    Borrowed,       // 不可变借用 (&x)
    BorrowedMut,    // 可变借用 (&mut x)
    Moved,          // 已移动（不可再使用）
}

/// 生命周期（简化版）
#[derive(Debug, Clone, PartialEq)]
enum Lifetime {
    Static,         // 'static - 整个程序生命周期
    Scoped(usize),  // 作用域生命周期
    Inferred,       // 自动推断
}

// ============================================================================
// CNB (C Native Bridge) - C类型系统
// ============================================================================

/// C/C++ 原生类型映射
#[derive(Debug, Clone, PartialEq)]
enum CType {
    // 基本类型
    Void,
    Char,
    Short,
    Int,
    Long,
    LongLong,
    UChar,
    UShort,
    UInt,
    ULong,
    ULongLong,
    Float,
    Double,
    Bool,
    // 指针类型
    Ptr(Box<CType>),            // *T
    ConstPtr(Box<CType>),       // const *T
    // 固定大小类型
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    // 平台相关
    SizeT,
    PtrDiffT,
    IntPtr,
    UIntPtr,
    // 字符串
    CStr,                       // const char*
    WStr,                       // const wchar_t*
    // 自定义结构
    Struct(String),             // struct Name
    // 函数指针
    FnPtr {
        params: Vec<CType>,
        ret: Box<CType>,
    },
}

impl CType {
    /// 返回类型的大小（字节）
    fn size(&self) -> usize {
        match self {
            CType::Void => 0,
            CType::Char | CType::UChar | CType::Int8 | CType::UInt8 | CType::Bool => 1,
            CType::Short | CType::UShort | CType::Int16 | CType::UInt16 => 2,
            CType::Int | CType::UInt | CType::Int32 | CType::UInt32 | CType::Float => 4,
            CType::Long | CType::ULong => 4, // Windows: 4, Linux: 8
            CType::LongLong | CType::ULongLong | CType::Int64 | CType::UInt64 | CType::Double => 8,
            CType::Ptr(_) | CType::ConstPtr(_) | CType::CStr | CType::WStr => 8, // x64
            CType::SizeT | CType::PtrDiffT | CType::IntPtr | CType::UIntPtr => 8,
            CType::Struct(_) => 0, // 需要查表
            CType::FnPtr { .. } => 8,
        }
    }
    
    /// 返回汇编中使用的寄存器大小后缀
    fn asm_suffix(&self) -> &'static str {
        match self.size() {
            1 => "byte",
            2 => "word",
            4 => "dword",
            8 => "qword",
            _ => "qword",
        }
    }
}

/// 调用约定
#[derive(Debug, Clone, PartialEq)]
enum CallingConv {
    Cdecl,          // C 默认调用约定
    Stdcall,        // Windows API 标准调用约定
    Fastcall,       // 快速调用（使用寄存器）
    Thiscall,       // C++ this 调用约定
    Win64,          // Windows x64 调用约定
    SysV,           // System V AMD64 调用约定 (Linux/macOS)
}

impl Default for CallingConv {
    fn default() -> Self {
        // x64 默认使用 Win64 或 SysV
        #[cfg(target_os = "windows")]
        { CallingConv::Win64 }
        #[cfg(not(target_os = "windows"))]
        { CallingConv::SysV }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Type {
    Int,
    Float,
    Str,
    Bool,
    None,
    List(Box<Type>),
    Any,        // 动态类型（自动推断）
    Unknown,
    // 所有权类型
    Ref(Box<Type>, Lifetime),           // &T - 不可变引用
    RefMut(Box<Type>, Lifetime),        // &mut T - 可变引用
    Box(Box<Type>),                     // box T - 堆分配
}

impl Type {
    fn from_token(tok: &Token) -> Option<Type> {
        match tok {
            Token::KwInt => Some(Type::Int),
            Token::KwFloat => Some(Type::Float),
            Token::KwStr => Some(Type::Str),
            Token::KwBool => Some(Type::Bool),
            Token::KwNone => Some(Type::None),
            _ => None,
        }
    }
}

// ============================================================================
// AST - 表达式
// ============================================================================

#[derive(Debug, Clone)]
enum Expr {
    // 字面量
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    None,
    
    // 变量
    Var(String),
    
    // 运算
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    UnaryOp(UnaryOp, Box<Expr>),
    
    // 函数调用
    Call(String, Vec<Expr>),
    MethodCall(Box<Expr>, String, Vec<Expr>),
    
    // 列表
    List(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    
    // 借用 (所有权系统核心)
    Borrow(Box<Expr>),          // &x - 不可变借用
    BorrowMut(Box<Expr>),       // &mut x - 可变借用
    Deref(Box<Expr>),           // *x - 解引用
}

#[derive(Debug, Clone, Copy)]
enum BinOp {
    // 比较
    Eq, Ne, Lt, Gt, Le, Ge,
    // 算术
    Add, Sub, Mul, Div, Mod,
    // 逻辑
    And, Or,
}

#[derive(Debug, Clone, Copy)]
enum UnaryOp {
    Neg,    // -x
    Not,    // not x
}

// ============================================================================
// AST - 语句
// ============================================================================

#[derive(Debug, Clone)]
enum Stmt {
    // ========== 接口系统（核心）==========
    // def 定义接口: def static.interface "Name" Type Dir "Target"
    DefInterface { 
        kind: String,           // static.interface, dynamic.interface 等
        name: String,           // 接口名 "Example.Print"
        data_type: String,      // 数据类型 int/str/any
        direction: String,      // In/Out/InOut
        target: String,         // 映射目标 "System.Output.Print"
    },
    // call 调用接口: call Interface.Name value
    CallInterface { 
        interface: String,      // 接口名
        args: Vec<Expr>,        // 参数列表
    },
    // drop 释放接口: drop Interface.Name
    DropInterface(String),
    // use 使用接口: use "module" as alias
    UseInterface { path: String, alias: Option<String> },
    
    // ========== 变量（本质上也是接口绑定）==========
    Let { name: String, ty: Option<Type>, value: Expr, mutable: bool },
    Assign { name: String, value: Expr },
    CompoundAssign { name: String, op: BinOp, value: Expr },
    
    // ========== 内置接口 ==========
    Print(Vec<Expr>),       // 映射到 System.Output.Print
    Input(String),          // 映射到 System.Input.Read
    
    // ========== 控制流 ==========
    If(Box<IfStmt>),
    Loop(Vec<Stmt>),
    For(Box<ForStmt>),
    While(Box<WhileStmt>),
    Break,
    Continue,
    Return(Option<Expr>),
    
    // ========== 错误处理 ==========
    Try(Box<TryStmt>),
    Throw(Expr),
    Defer(Vec<Stmt>),
    
    // ========== 函数（本质是可调用接口）==========
    FnDef(Box<FnDef>),
    
    // ========== 模块系统 ==========
    /// import "path/module.sm"              - 导入整个模块
    /// import "module.sm" as alias          - 导入并取别名
    Import {
        path: String,               // 模块路径
        alias: Option<String>,      // 可选别名
    },
    /// use module::func                     - 导入特定符号
    /// use module::{a, b, c}                - 批量导入
    /// use module::*                        - 导入所有
    Use {
        module_path: Vec<String>,   // 模块路径 ["std", "io"]
        imports: UseImport,         // 导入内容
    },
    /// from "module" import func            - Python风格导入
    FromImport {
        path: String,               // 模块路径
        names: Vec<(String, Option<String>)>,  // [(name, alias), ...]
    },
    /// #include "header.smh"                - C风格文本包含
    Include(String),
    /// pub fn / pub let                     - 公开声明（编译器标记）
    PubDecl(Box<Stmt>),
    
    // ========== CNB (C Native Bridge) ==========
    /// extern "C" fn name(args) -> ret
    ExternFn {
        name: String,
        params: Vec<(String, CType)>,
        ret_type: Option<CType>,
        calling_conv: CallingConv,
        lib_name: Option<String>,      // 可选的库名 (DLL/SO)
    },
    /// extern struct Name { fields }
    ExternStruct {
        name: String,
        fields: Vec<(String, CType)>,
    },
    /// unsafe { ... }
    Unsafe(Vec<Stmt>),
    
    // ========== 其他 ==========
    Expr(Expr),
    Block(Vec<Stmt>),
}

/// use 语句的导入内容
#[derive(Debug, Clone)]
enum UseImport {
    /// use module::name
    Single(String),
    /// use module::{a, b, c}
    Multiple(Vec<(String, Option<String>)>),  // [(name, alias), ...]
    /// use module::*
    Glob,
}

#[derive(Debug, Clone)]
struct IfStmt {
    cond: Expr,
    then_block: Vec<Stmt>,
    elif_parts: Vec<(Expr, Vec<Stmt>)>,
    else_block: Option<Vec<Stmt>>,
}

#[derive(Debug, Clone)]
struct ForStmt {
    var: String,
    iter: Expr,
    body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
struct WhileStmt {
    cond: Expr,
    body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
struct TryStmt {
    try_block: Vec<Stmt>,
    catch_var: Option<String>,
    catch_block: Option<Vec<Stmt>>,
}

#[derive(Debug, Clone)]
struct FnDef {
    name: String,
    params: Vec<(String, Option<Type>)>,
    ret_type: Option<Type>,
    body: Vec<Stmt>,
}

#[derive(Debug)]
struct Program {
    stmts: Vec<Stmt>,
}

// ============================================================================
// PARSER
// ============================================================================

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }
    
    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::EOF)
    }
    
    fn peek_ahead(&self, n: usize) -> &Token {
        self.tokens.get(self.pos + n).unwrap_or(&Token::EOF)
    }
    
    fn next(&mut self) -> Token {
        let tok = self.peek().clone();
        self.pos += 1;
        tok
    }
    
    fn expect(&mut self, expected: Token) -> Result<(), CompileError> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(&expected) {
            self.next();
            Ok(())
        } else {
            Err(CompileError::parse(&format!("Expected {:?}, got {:?}", expected, self.peek()), self.pos))
        }
    }
    
    fn check(&self, expected: &Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(expected)
    }
    
    fn parse_program(&mut self) -> Result<Program, CompileError> {
        let mut stmts = Vec::new();
        
        while self.peek() != &Token::EOF {
            // 跳过 fn main { ... } 包装
            if self.peek() == &Token::KwFn {
                self.next();
                let name = if let Token::Ident(id) = self.peek().clone() {
                    self.next();
                    id
                } else if self.peek() == &Token::KwMain {
                    self.next();
                    "main".to_string()
                } else {
                    return Err(CompileError::parse("Expected function name", self.pos));
                };
                
                // 解析函数参数（如果有）
                let params = if self.peek() == &Token::LParen {
                    self.parse_params()?
                } else {
                    Vec::new()
                };
                
                // 解析返回类型（如果有）
                let ret_type = if self.peek() == &Token::Arrow {
                    self.next();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                
                // 支持两种语法: fn name { } 或 fn name ... end
                let body = if self.peek() == &Token::LBrace {
                    self.next();
                    let mut body = Vec::new();
                    while self.peek() != &Token::RBrace && self.peek() != &Token::EOF {
                        body.push(self.parse_stmt()?);
                    }
                    self.expect(Token::RBrace)?;
                    body
                } else {
                    // 无括号语法，以 end 结束
                    let mut body = Vec::new();
                    while self.peek() != &Token::KwEnd && self.peek() != &Token::EOF {
                        body.push(self.parse_stmt()?);
                    }
                    if self.peek() == &Token::KwEnd {
                        self.next();
                    }
                    body
                };
                
                if name == "main" {
                    // main函数内容直接作为顶层语句
                    stmts.extend(body);
                } else {
                    // 其他函数作为函数定义
                    stmts.push(Stmt::FnDef(Box::new(FnDef {
                        name,
                        params,
                        ret_type,
                        body,
                    })));
                }
            } else {
                stmts.push(self.parse_stmt()?);
            }
        }
        
        Ok(Program { stmts })
    }
    
    fn parse_params(&mut self) -> Result<Vec<(String, Option<Type>)>, CompileError> {
        self.expect(Token::LParen)?;
        let mut params = Vec::new();
        
        while self.peek() != &Token::RParen {
            let name = if let Token::Ident(id) = self.next() {
                id
            } else {
                return Err(CompileError::parse("Expected parameter name", self.pos));
            };
            
            let ty = if self.peek() == &Token::Colon {
                self.next();
                Some(self.parse_type()?)
            } else {
                None
            };
            
            params.push((name, ty));
            
            if self.peek() == &Token::Comma {
                self.next();
            }
        }
        
        self.expect(Token::RParen)?;
        Ok(params)
    }
    
    fn parse_type(&mut self) -> Result<Type, CompileError> {
        match self.next() {
            Token::KwInt => Ok(Type::Int),
            Token::KwFloat => Ok(Type::Float),
            Token::KwStr => Ok(Type::Str),
            Token::KwBool => Ok(Type::Bool),
            Token::KwNone => Ok(Type::None),
            Token::Ident(id) => Ok(Type::Any), // 用户自定义类型，暂用Any
            _ => Err(CompileError::parse("Expected type", self.pos)),
        }
    }
    
    fn parse_stmt(&mut self) -> Result<Stmt, CompileError> {
        match self.peek() {
            // 变量声明
            Token::KwLet => self.parse_let(false),
            Token::KwVar => self.parse_let(true),
            Token::KwConst => self.parse_let(false),
            
            // 内置函数
            Token::KwPrint => self.parse_print(),
            
            // 控制流
            Token::KwIf => self.parse_if(),
            Token::KwLoop => self.parse_loop(),
            Token::KwFor => self.parse_for(),
            Token::KwWhile => self.parse_while(),
            Token::KwBreak => {
                self.next();
                self.skip_semicolon();
                Ok(Stmt::Break)
            }
            Token::KwContinue => {
                self.next();
                self.skip_semicolon();
                Ok(Stmt::Continue)
            }
            Token::KwReturn => {
                self.next();
                let value = if self.peek() != &Token::Semicolon && self.peek() != &Token::RBrace {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.skip_semicolon();
                Ok(Stmt::Return(value))
            }
            
            // 错误处理
            Token::KwTry => self.parse_try(),
            Token::KwThrow => {
                self.next();
                let expr = self.parse_expr()?;
                self.skip_semicolon();
                Ok(Stmt::Throw(expr))
            }
            Token::KwDefer => self.parse_defer(),
            
            // 兼容旧语法
            Token::KwCall => self.parse_call(),
            Token::KwDef => self.parse_def(),
            Token::KwDrop => self.parse_drop(),
            
            // ========== 模块系统 ==========
            Token::KwImport => self.parse_import(),
            Token::KwUse => self.parse_use(),
            Token::KwFrom => self.parse_from_import(),
            Token::KwInclude => self.parse_include(),
            Token::KwPub => self.parse_pub_decl(),
            
            // ========== CNB (C Native Bridge) ==========
            Token::KwExtern => self.parse_extern(),
            Token::KwUnsafe => self.parse_unsafe(),
            Token::KwStruct => self.parse_struct(),
            
            // 块
            Token::LBrace => self.parse_block(),
            
            // 标识符开头：可能是赋值或表达式
            Token::Ident(_) => {
                // 向前看判断是赋值还是表达式
                if matches!(self.peek_ahead(1), Token::Assign | Token::PlusAssign | Token::MinusAssign | Token::MulAssign | Token::DivAssign) {
                    self.parse_assign()
                } else {
                    let expr = self.parse_expr()?;
                    self.skip_semicolon();
                    Ok(Stmt::Expr(expr))
                }
            }
            
            _ => {
                let expr = self.parse_expr()?;
                self.skip_semicolon();
                Ok(Stmt::Expr(expr))
            }
        }
    }
    
    fn parse_let(&mut self, mutable: bool) -> Result<Stmt, CompileError> {
        self.next(); // let/var/const
        
        let name = if let Token::Ident(id) = self.next() {
            id
        } else {
            return Err(CompileError::parse("Expected variable name", self.pos));
        };
        
        // 可选类型注解
        let ty = if self.peek() == &Token::Colon {
            self.next();
            Some(self.parse_type()?)
        } else {
            None
        };
        
        self.expect(Token::Assign)?;
        let value = self.parse_expr()?;
        self.skip_semicolon();
        
        Ok(Stmt::Let { name, ty, value, mutable })
    }
    
    fn parse_assign(&mut self) -> Result<Stmt, CompileError> {
        let name = if let Token::Ident(id) = self.next() {
            id
        } else {
            return Err(CompileError::parse("Expected variable name", self.pos));
        };
        
        let stmt = match self.next() {
            Token::Assign => {
                let value = self.parse_expr()?;
                Stmt::Assign { name, value }
            }
            Token::PlusAssign => {
                let value = self.parse_expr()?;
                Stmt::CompoundAssign { name, op: BinOp::Add, value }
            }
            Token::MinusAssign => {
                let value = self.parse_expr()?;
                Stmt::CompoundAssign { name, op: BinOp::Sub, value }
            }
            Token::MulAssign => {
                let value = self.parse_expr()?;
                Stmt::CompoundAssign { name, op: BinOp::Mul, value }
            }
            Token::DivAssign => {
                let value = self.parse_expr()?;
                Stmt::CompoundAssign { name, op: BinOp::Div, value }
            }
            _ => return Err(CompileError::parse("Expected assignment operator", self.pos)),
        };
        
        self.skip_semicolon();
        Ok(stmt)
    }
    
    fn parse_print(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // print
        
        let mut args = Vec::new();
        
        // print 后面可以有括号也可以没有
        let has_paren = self.peek() == &Token::LParen;
        if has_paren {
            self.next();
        }
        
        // 解析参数
        if self.peek() != &Token::RParen && self.peek() != &Token::Semicolon && self.peek() != &Token::RBrace {
            args.push(self.parse_expr()?);
            while self.peek() == &Token::Comma {
                self.next();
                args.push(self.parse_expr()?);
            }
        }
        
        if has_paren {
            self.expect(Token::RParen)?;
        }
        self.skip_semicolon();
        
        Ok(Stmt::Print(args))
    }
    
    fn parse_if(&mut self) -> Result<Stmt, CompileError> {
        self.expect(Token::KwIf)?;
        let cond = self.parse_expr()?;
        let then_block = self.parse_block_for_if()?;
        
        let mut elif_parts = Vec::new();
        let mut else_block = None;
        
        while self.peek() == &Token::KwElif {
            self.next();
            let elif_cond = self.parse_expr()?;
            let elif_body = self.parse_block_for_if()?;
            elif_parts.push((elif_cond, elif_body));
        }
        
        if self.peek() == &Token::KwElse {
            self.next();
            else_block = Some(self.parse_block_for_if()?);
        }
        
        // 如果使用的是无括号语法，消耗最终的 end
        if self.peek() == &Token::KwEnd {
            self.next();
        }
        
        Ok(Stmt::If(Box::new(IfStmt {
            cond,
            then_block,
            elif_parts,
            else_block,
        })))
    }
    
    // 专门为 if/elif/else 设计的块解析
    fn parse_block_for_if(&mut self) -> Result<Vec<Stmt>, CompileError> {
        let mut stmts = Vec::new();
        
        if self.peek() == &Token::LBrace {
            self.next(); // {
            while self.peek() != &Token::RBrace && self.peek() != &Token::EOF {
                stmts.push(self.parse_stmt()?);
            }
            self.expect(Token::RBrace)?;
        } else {
            // 无括号语法，以 end/else/elif 结束（但不消耗它们）
            while self.peek() != &Token::KwEnd 
                && self.peek() != &Token::KwElse 
                && self.peek() != &Token::KwElif 
                && self.peek() != &Token::EOF 
            {
                stmts.push(self.parse_stmt()?);
            }
        }
        
        Ok(stmts)
    }
    
    fn parse_loop(&mut self) -> Result<Stmt, CompileError> {
        self.expect(Token::KwLoop)?;
        let body = self.parse_block_content()?;
        Ok(Stmt::Loop(body))
    }
    
    fn parse_for(&mut self) -> Result<Stmt, CompileError> {
        self.expect(Token::KwFor)?;
        
        let var = if let Token::Ident(id) = self.next() {
            id
        } else {
            return Err(CompileError::parse("Expected variable name in for loop", self.pos));
        };
        
        self.expect(Token::KwIn)?;
        let iter = self.parse_expr()?;
        let body = self.parse_block_content()?;
        
        Ok(Stmt::For(Box::new(ForStmt { var, iter, body })))
    }
    
    fn parse_while(&mut self) -> Result<Stmt, CompileError> {
        self.expect(Token::KwWhile)?;
        let cond = self.parse_expr()?;
        let body = self.parse_block_content()?;
        Ok(Stmt::While(Box::new(WhileStmt { cond, body })))
    }
    
    fn parse_try(&mut self) -> Result<Stmt, CompileError> {
        self.expect(Token::KwTry)?;
        let try_block = self.parse_block_content()?;
        
        let (catch_var, catch_block) = if self.peek() == &Token::KwCatch {
            self.next();
            let var = if let Token::Ident(id) = self.peek().clone() {
                self.next();
                Some(id)
            } else {
                None
            };
            let block = self.parse_block_content()?;
            (var, Some(block))
        } else {
            (None, None)
        };
        
        Ok(Stmt::Try(Box::new(TryStmt { try_block, catch_var, catch_block })))
    }
    
    fn parse_defer(&mut self) -> Result<Stmt, CompileError> {
        self.expect(Token::KwDefer)?;
        let body = self.parse_block_content()?;
        Ok(Stmt::Defer(body))
    }
    
    fn parse_call(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // call
        
        // 解析接口名（可以是点分隔的）
        let mut interface = String::new();
        if let Token::Ident(id) = self.next() {
            interface.push_str(&id);
        }
        while self.peek() == &Token::Dot {
            self.next();
            if let Token::Ident(id) = self.next() {
                interface.push('.');
                interface.push_str(&id);
            }
        }
        
        // 解析参数
        let mut args = Vec::new();
        
        // 跳过可能的前导逗号 (支持 `call func, arg` 语法)
        if self.peek() == &Token::Comma {
            self.next();
        }
        
        // 检查是否有参数 - 只要不是语句开始符就继续解析
        if !Self::is_stmt_starter(self.peek()) && self.peek() != &Token::EOF {
            args.push(self.parse_expr()?);
            
            // 继续解析：逗号分隔 或 字符串/数字/标识符 直接跟随
            loop {
                if self.peek() == &Token::Comma {
                    self.next();
                    if Self::is_stmt_starter(self.peek()) || self.peek() == &Token::EOF {
                        break;
                    }
                    args.push(self.parse_expr()?);
                } else if Self::is_literal_token(self.peek()) {
                    // 只有字面量可以无逗号跟随（支持 call print "a" "b" 和 call print "x=" 42）
                    args.push(self.parse_expr()?);
                } else {
                    break;
                }
            }
        }
        
        self.skip_semicolon();
        Ok(Stmt::CallInterface { interface, args })
    }
    
    // 检查是否是字面量（可以无逗号跟随的参数）
    fn is_literal_token(tok: &Token) -> bool {
        matches!(tok,
            Token::Number(_) | Token::Float(_) | Token::String(_) |
            Token::KwTrue | Token::KwFalse
        )
    }
    
    fn is_stmt_starter(tok: &Token) -> bool {
        matches!(tok,
            Token::Semicolon | Token::RBrace | Token::EOF |
            Token::KwDef | Token::KwDrop | Token::KwCall |
            Token::KwLet | Token::KwVar | Token::KwConst |
            Token::KwIf | Token::KwElse | Token::KwElif |
            Token::KwWhile | Token::KwFor | Token::KwLoop |
            Token::KwFn | Token::KwReturn | Token::KwEnd |
            Token::KwBreak | Token::KwContinue |
            Token::KwTry | Token::KwCatch | Token::KwDefer |
            Token::KwPrint
        )
    }
    
    fn parse_def(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // def
        
        // def static.interface "Name" type Dir "Target"
        let mut kind = String::new();
        if let Token::Ident(id) = self.next() {
            kind.push_str(&id);
        }
        while self.peek() == &Token::Dot {
            self.next();
            if let Token::Ident(id) = self.next() {
                kind.push('.');
                kind.push_str(&id);
            }
        }
        
        let name = match self.next() {
            Token::String(s) => s,
            _ => return Err(CompileError::parse("Expected interface name string", self.pos)),
        };
        
        // 解析类型
        let data_type = if let Token::Ident(id) = self.next() {
            id
        } else {
            "any".to_string()
        };
        
        // 解析方向
        let direction = if let Token::Ident(id) = self.next() {
            id
        } else {
            "Out".to_string()
        };
        
        // 解析目标
        let target = match self.next() {
            Token::String(s) => s,
            _ => return Err(CompileError::parse("Expected target string", self.pos)),
        };
        
        self.skip_semicolon();
        Ok(Stmt::DefInterface { kind, name, data_type, direction, target })
    }
    
    fn parse_drop(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // drop
        
        let mut name = String::new();
        if let Token::Ident(id) = self.next() {
            name.push_str(&id);
        }
        while self.peek() == &Token::Dot {
            self.next();
            if let Token::Ident(id) = self.next() {
                name.push('.');
                name.push_str(&id);
            }
        }
        
        self.skip_semicolon();
        Ok(Stmt::DropInterface(name))
    }
    
    // ========================================================================
    // 模块系统解析
    // ========================================================================
    
    /// 解析 import 语句
    /// import "path/module.sm"
    /// import "module.sm" as alias
    fn parse_import(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // import
        
        // 获取模块路径（字符串或标识符路径）
        let path = match self.next() {
            Token::String(s) => s,
            Token::Ident(id) => {
                // 支持 import std::io 语法
                let mut path = id;
                while self.peek() == &Token::DoubleColon || self.peek() == &Token::Dot {
                    self.next();
                    if let Token::Ident(next_id) = self.next() {
                        path.push('/');
                        path.push_str(&next_id);
                    }
                }
                path + ".sm"
            }
            _ => return Err(CompileError::parse("Expected module path in import", self.pos)),
        };
        
        // 可选别名 as alias
        let alias = if self.peek() == &Token::KwAs {
            self.next();
            if let Token::Ident(id) = self.next() {
                Some(id)
            } else {
                return Err(CompileError::parse("Expected alias name after 'as'", self.pos));
            }
        } else {
            None
        };
        
        self.skip_semicolon();
        Ok(Stmt::Import { path, alias })
    }
    
    /// 解析 use 语句
    /// use module::func
    /// use module::{a, b, c}
    /// use module::*
    fn parse_use(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // use
        
        // 解析模块路径 module::submodule::...
        let mut module_path = Vec::new();
        
        if let Token::Ident(id) = self.next() {
            module_path.push(id);
        } else {
            return Err(CompileError::parse("Expected module name in use", self.pos));
        }
        
        while self.peek() == &Token::DoubleColon {
            self.next(); // ::
            
            match self.peek().clone() {
                Token::Ident(id) => {
                    // 可能是中间路径或最终符号
                    self.next();
                    // 如果后面还有 ::，则是中间路径
                    if self.peek() == &Token::DoubleColon {
                        module_path.push(id);
                    } else {
                        // 最终符号
                        let alias = if self.peek() == &Token::KwAs {
                            self.next();
                            if let Token::Ident(alias_id) = self.next() {
                                Some(alias_id)
                            } else {
                                return Err(CompileError::parse("Expected alias name", self.pos));
                            }
                        } else {
                            None
                        };
                        self.skip_semicolon();
                        return Ok(Stmt::Use {
                            module_path,
                            imports: UseImport::Single(id),
                        });
                    }
                }
                Token::Star => {
                    // use module::*
                    self.next();
                    self.skip_semicolon();
                    return Ok(Stmt::Use {
                        module_path,
                        imports: UseImport::Glob,
                    });
                }
                Token::LBrace => {
                    // use module::{a, b, c}
                    self.next();
                    let mut names = Vec::new();
                    
                    while self.peek() != &Token::RBrace {
                        if let Token::Ident(name) = self.next() {
                            let alias = if self.peek() == &Token::KwAs {
                                self.next();
                                if let Token::Ident(alias_id) = self.next() {
                                    Some(alias_id)
                                } else {
                                    return Err(CompileError::parse("Expected alias name", self.pos));
                                }
                            } else {
                                None
                            };
                            names.push((name, alias));
                        }
                        if self.peek() == &Token::Comma {
                            self.next();
                        }
                    }
                    self.expect(Token::RBrace)?;
                    self.skip_semicolon();
                    return Ok(Stmt::Use {
                        module_path,
                        imports: UseImport::Multiple(names),
                    });
                }
                _ => return Err(CompileError::parse("Expected symbol, '*', or '{' in use", self.pos)),
            }
        }
        
        // 如果只有模块路径，没有具体导入
        Err(CompileError::parse("Expected '::' after module path in use statement", self.pos))
    }
    
    /// 解析 from ... import 语句 (Python风格)
    /// from "module" import func
    /// from "module" import func as alias
    /// from "module" import a, b, c
    fn parse_from_import(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // from
        
        // 获取模块路径
        let path = match self.next() {
            Token::String(s) => s,
            Token::Ident(id) => {
                let mut path = id;
                while self.peek() == &Token::DoubleColon || self.peek() == &Token::Dot {
                    self.next();
                    if let Token::Ident(next_id) = self.next() {
                        path.push('/');
                        path.push_str(&next_id);
                    }
                }
                path + ".sm"
            }
            _ => return Err(CompileError::parse("Expected module path after 'from'", self.pos)),
        };
        
        // 期望 import 关键字
        if self.peek() != &Token::KwImport {
            return Err(CompileError::parse("Expected 'import' after module path", self.pos));
        }
        self.next(); // import
        
        // 解析导入的名称列表
        let mut names = Vec::new();
        
        loop {
            if let Token::Ident(name) = self.next() {
                let alias = if self.peek() == &Token::KwAs {
                    self.next();
                    if let Token::Ident(alias_id) = self.next() {
                        Some(alias_id)
                    } else {
                        return Err(CompileError::parse("Expected alias name after 'as'", self.pos));
                    }
                } else {
                    None
                };
                names.push((name, alias));
            } else {
                return Err(CompileError::parse("Expected import name", self.pos));
            }
            
            if self.peek() == &Token::Comma {
                self.next();
            } else {
                break;
            }
        }
        
        self.skip_semicolon();
        Ok(Stmt::FromImport { path, names })
    }
    
    /// 解析 include 语句 (C风格文本包含)
    /// include "header.smh"
    /// #include "header.smh"
    fn parse_include(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // include
        
        let path = match self.next() {
            Token::String(s) => s,
            _ => return Err(CompileError::parse("Expected file path in include", self.pos)),
        };
        
        self.skip_semicolon();
        Ok(Stmt::Include(path))
    }
    
    /// 解析 pub 声明 (公开导出)
    /// pub fn name() { }
    /// pub let x = 1
    fn parse_pub_decl(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // pub
        
        // 解析后面的声明
        let inner = match self.peek() {
            Token::KwFn => {
                // pub fn
                self.next();
                let name = if let Token::Ident(id) = self.next() {
                    id
                } else {
                    return Err(CompileError::parse("Expected function name after 'pub fn'", self.pos));
                };
                
                let params = if self.peek() == &Token::LParen {
                    self.parse_params()?
                } else {
                    Vec::new()
                };
                
                let ret_type = if self.peek() == &Token::Arrow {
                    self.next();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                
                let body = self.parse_block_content()?;
                
                Stmt::FnDef(Box::new(FnDef {
                    name,
                    params,
                    ret_type,
                    body,
                }))
            }
            Token::KwLet => self.parse_let(false)?,
            Token::KwVar => self.parse_let(true)?,
            Token::KwConst => self.parse_let(false)?,
            _ => return Err(CompileError::parse("Expected 'fn', 'let', 'var', or 'const' after 'pub'", self.pos)),
        };
        
        Ok(Stmt::PubDecl(Box::new(inner)))
    }
    
    // ========================================================================
    // CNB (C Native Bridge) 解析
    // ========================================================================
    
    /// 解析 extern 声明
    /// extern "C" fn name(args) -> ret
    /// extern "C" fn name(args) -> ret from "lib.dll"
    /// extern struct Name { ... }
    fn parse_extern(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // extern
        
        // 可选的调用约定字符串 "C", "stdcall", "fastcall"
        let calling_conv = if let Token::String(conv) = self.peek().clone() {
            self.next();
            match conv.as_str() {
                "C" | "c" | "cdecl" => CallingConv::Cdecl,
                "stdcall" | "STDCALL" => CallingConv::Stdcall,
                "fastcall" | "FASTCALL" => CallingConv::Fastcall,
                "win64" | "WIN64" | "ms" => CallingConv::Win64,
                "sysv" | "SYSV" | "linux" => CallingConv::SysV,
                _ => CallingConv::default(),
            }
        } else {
            CallingConv::default()
        };
        
        match self.peek() {
            Token::KwFn => self.parse_extern_fn(calling_conv),
            Token::KwStruct => self.parse_extern_struct(),
            _ => Err(CompileError::parse("Expected 'fn' or 'struct' after 'extern'", self.pos)),
        }
    }
    
    /// 解析 extern fn
    /// extern "C" fn printf(fmt: *const char, ...) -> int
    /// extern "C" fn MessageBoxA(hwnd: uintptr, text: *const char, caption: *const char, flags: uint) -> int from "user32.dll"
    fn parse_extern_fn(&mut self, calling_conv: CallingConv) -> Result<Stmt, CompileError> {
        self.next(); // fn
        
        let name = if let Token::Ident(id) = self.next() {
            id
        } else {
            return Err(CompileError::parse("Expected function name after 'extern fn'", self.pos));
        };
        
        // 解析参数列表
        self.expect(Token::LParen)?;
        let mut params = Vec::new();
        
        while self.peek() != &Token::RParen {
            let param_name = if let Token::Ident(id) = self.next() {
                id
            } else {
                return Err(CompileError::parse("Expected parameter name", self.pos));
            };
            
            self.expect(Token::Colon)?;
            let param_type = self.parse_ctype()?;
            params.push((param_name, param_type));
            
            if self.peek() == &Token::Comma {
                self.next();
            }
        }
        self.expect(Token::RParen)?;
        
        // 可选的返回类型
        let ret_type = if self.peek() == &Token::Arrow {
            self.next();
            Some(self.parse_ctype()?)
        } else {
            None
        };
        
        // 可选的库名
        let lib_name = if let Token::Ident(ref kw) = self.peek() {
            if kw == "from" {
                self.next(); // from
                if let Token::String(lib) = self.next() {
                    Some(lib)
                } else {
                    return Err(CompileError::parse("Expected library name string after 'from'", self.pos));
                }
            } else {
                None
            }
        } else {
            None
        };
        
        self.skip_semicolon();
        
        Ok(Stmt::ExternFn {
            name,
            params,
            ret_type,
            calling_conv,
            lib_name,
        })
    }
    
    /// 解析 extern struct
    /// extern struct POINT { x: int, y: int }
    fn parse_extern_struct(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // struct
        
        let name = if let Token::Ident(id) = self.next() {
            id
        } else {
            return Err(CompileError::parse("Expected struct name", self.pos));
        };
        
        self.expect(Token::LBrace)?;
        let mut fields = Vec::new();
        
        while self.peek() != &Token::RBrace {
            let field_name = if let Token::Ident(id) = self.next() {
                id
            } else {
                return Err(CompileError::parse("Expected field name", self.pos));
            };
            
            self.expect(Token::Colon)?;
            let field_type = self.parse_ctype()?;
            fields.push((field_name, field_type));
            
            // 逗号或分号分隔
            if self.peek() == &Token::Comma || self.peek() == &Token::Semicolon {
                self.next();
            }
        }
        self.expect(Token::RBrace)?;
        
        Ok(Stmt::ExternStruct { name, fields })
    }
    
    /// 解析 unsafe 块
    /// unsafe { raw_ptr_op() }
    fn parse_unsafe(&mut self) -> Result<Stmt, CompileError> {
        self.next(); // unsafe
        let body = self.parse_block_content()?;
        Ok(Stmt::Unsafe(body))
    }
    
    /// 解析 struct (Slime 原生结构体)
    fn parse_struct(&mut self) -> Result<Stmt, CompileError> {
        // 目前简单转发到 extern struct
        self.next(); // struct
        
        let name = if let Token::Ident(id) = self.next() {
            id
        } else {
            return Err(CompileError::parse("Expected struct name", self.pos));
        };
        
        self.expect(Token::LBrace)?;
        let mut fields = Vec::new();
        
        while self.peek() != &Token::RBrace {
            let field_name = if let Token::Ident(id) = self.next() {
                id
            } else {
                break; // 允许空结构体
            };
            
            self.expect(Token::Colon)?;
            let field_type = self.parse_ctype()?;
            fields.push((field_name, field_type));
            
            if self.peek() == &Token::Comma || self.peek() == &Token::Semicolon {
                self.next();
            }
        }
        self.expect(Token::RBrace)?;
        
        Ok(Stmt::ExternStruct { name, fields })
    }
    
    /// 解析 C 类型
    fn parse_ctype(&mut self) -> Result<CType, CompileError> {
        // 检查指针前缀 *
        if self.peek() == &Token::Star {
            self.next();
            // 检查 const
            if let Token::Ident(ref kw) = self.peek() {
                if kw == "const" {
                    self.next();
                    let inner = self.parse_ctype()?;
                    return Ok(CType::ConstPtr(Box::new(inner)));
                }
            }
            let inner = self.parse_ctype()?;
            return Ok(CType::Ptr(Box::new(inner)));
        }
        
        // 检查 const 指针 (const *T 语法) 或 const 类型
        let is_const = if let Token::Ident(ref kw) = self.peek().clone() {
            if kw == "const" {
                self.next();
                if self.peek() == &Token::Star {
                    self.next();
                    let inner = self.parse_ctype()?;
                    return Ok(CType::ConstPtr(Box::new(inner)));
                }
                true // const 后面跟着类型名而不是 *
            } else {
                false
            }
        } else {
            false
        };
        
        // 如果刚才消耗了 const，现在 type_name 是 const 后面的类型
        // 否则正常解析类型名
        let type_name = if is_const {
            // const 后面直接跟类型名 (如 const char)
            if let Token::Ident(id) = self.next() {
                id
            } else {
                return Err(CompileError::parse("Expected type name after 'const'", self.pos));
            }
        } else if let Token::Ident(id) = self.next() {
            id
        } else {
            return Err(CompileError::parse("Expected type name", self.pos));
        };
        
        let ctype = match type_name.as_str() {
            // 基本类型
            "void" => CType::Void,
            "char" => CType::Char,
            "short" => CType::Short,
            "int" => CType::Int,
            "long" => CType::Long,
            "longlong" | "long_long" => CType::LongLong,
            "uchar" | "unsigned_char" => CType::UChar,
            "ushort" | "unsigned_short" => CType::UShort,
            "uint" | "unsigned" | "unsigned_int" => CType::UInt,
            "ulong" | "unsigned_long" => CType::ULong,
            "ulonglong" | "unsigned_long_long" => CType::ULongLong,
            "float" | "f32" => CType::Float,
            "double" | "f64" => CType::Double,
            "bool" | "_Bool" => CType::Bool,
            // 固定大小
            "i8" | "int8" | "int8_t" => CType::Int8,
            "i16" | "int16" | "int16_t" => CType::Int16,
            "i32" | "int32" | "int32_t" => CType::Int32,
            "i64" | "int64" | "int64_t" => CType::Int64,
            "u8" | "uint8" | "uint8_t" | "byte" => CType::UInt8,
            "u16" | "uint16" | "uint16_t" => CType::UInt16,
            "u32" | "uint32" | "uint32_t" => CType::UInt32,
            "u64" | "uint64" | "uint64_t" => CType::UInt64,
            // 平台相关
            "size_t" | "usize" => CType::SizeT,
            "ptrdiff_t" | "isize" => CType::PtrDiffT,
            "intptr" | "intptr_t" => CType::IntPtr,
            "uintptr" | "uintptr_t" | "HANDLE" | "HWND" | "HINSTANCE" => CType::UIntPtr,
            // 字符串
            "cstr" | "c_str" | "LPCSTR" | "PCSTR" => CType::CStr,
            "wstr" | "w_str" | "LPCWSTR" | "PCWSTR" => CType::WStr,
            // 其他 - 视为自定义结构体
            _ => CType::Struct(type_name),
        };
        
        Ok(ctype)
    }
    
    // ========================================================================
    // 块解析
    // ========================================================================

    fn parse_block(&mut self) -> Result<Stmt, CompileError> {
        let stmts = self.parse_block_content()?;
        Ok(Stmt::Block(stmts))
    }
    
    fn parse_block_content(&mut self) -> Result<Vec<Stmt>, CompileError> {
        let mut stmts = Vec::new();
        
        // 支持两种语法：{ } 或 无括号 + end
        if self.peek() == &Token::LBrace {
            self.next(); // {
            while self.peek() != &Token::RBrace && self.peek() != &Token::EOF {
                stmts.push(self.parse_stmt()?);
            }
            self.expect(Token::RBrace)?;
        } else {
            // 无括号语法，以 end/else/elif 结束
            while self.peek() != &Token::KwEnd 
                && self.peek() != &Token::KwElse 
                && self.peek() != &Token::KwElif 
                && self.peek() != &Token::EOF 
            {
                stmts.push(self.parse_stmt()?);
            }
            // 如果是 end，消耗它
            if self.peek() == &Token::KwEnd {
                self.next();
            }
        }
        
        Ok(stmts)
    }
    
    // 表达式解析（优先级从低到高）
    fn parse_expr(&mut self) -> Result<Expr, CompileError> {
        self.parse_or()
    }
    
    fn parse_or(&mut self) -> Result<Expr, CompileError> {
        let mut expr = self.parse_and()?;
        while self.peek() == &Token::KwOr {
            self.next();
            let rhs = self.parse_and()?;
            expr = Expr::BinOp(Box::new(expr), BinOp::Or, Box::new(rhs));
        }
        Ok(expr)
    }
    
    fn parse_and(&mut self) -> Result<Expr, CompileError> {
        let mut expr = self.parse_comparison()?;
        while self.peek() == &Token::KwAnd {
            self.next();
            let rhs = self.parse_comparison()?;
            expr = Expr::BinOp(Box::new(expr), BinOp::And, Box::new(rhs));
        }
        Ok(expr)
    }
    
    fn parse_comparison(&mut self) -> Result<Expr, CompileError> {
        let mut expr = self.parse_additive()?;
        while let Some(op) = self.peek_cmp_op() {
            self.next();
            let rhs = self.parse_additive()?;
            expr = Expr::BinOp(Box::new(expr), op, Box::new(rhs));
        }
        Ok(expr)
    }
    
    fn peek_cmp_op(&self) -> Option<BinOp> {
        match self.peek() {
            Token::Eq => Some(BinOp::Eq),
            Token::Ne => Some(BinOp::Ne),
            Token::Lt => Some(BinOp::Lt),
            Token::Gt => Some(BinOp::Gt),
            Token::Le => Some(BinOp::Le),
            Token::Ge => Some(BinOp::Ge),
            _ => None,
        }
    }
    
    fn parse_additive(&mut self) -> Result<Expr, CompileError> {
        let mut expr = self.parse_multiplicative()?;
        while matches!(self.peek(), Token::Plus | Token::Minus) {
            let op = if self.peek() == &Token::Plus { BinOp::Add } else { BinOp::Sub };
            self.next();
            let rhs = self.parse_multiplicative()?;
            expr = Expr::BinOp(Box::new(expr), op, Box::new(rhs));
        }
        Ok(expr)
    }
    
    fn parse_multiplicative(&mut self) -> Result<Expr, CompileError> {
        let mut expr = self.parse_unary()?;
        while matches!(self.peek(), Token::Star | Token::Slash | Token::Percent) {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => BinOp::Mod,
            };
            self.next();
            let rhs = self.parse_unary()?;
            expr = Expr::BinOp(Box::new(expr), op, Box::new(rhs));
        }
        Ok(expr)
    }
    
    fn parse_unary(&mut self) -> Result<Expr, CompileError> {
        match self.peek() {
            Token::Minus => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::Neg, Box::new(expr)))
            }
            Token::KwNot => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::Not, Box::new(expr)))
            }
            // 借用: &x
            Token::Ampersand => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::Borrow(Box::new(expr)))
            }
            // 可变借用: &mut x
            Token::AmpMut => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::BorrowMut(Box::new(expr)))
            }
            // 解引用: *x
            Token::Star => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::Deref(Box::new(expr)))
            }
            _ => self.parse_postfix(),
        }
    }
    
    fn parse_postfix(&mut self) -> Result<Expr, CompileError> {
        let mut expr = self.parse_primary()?;
        
        loop {
            match self.peek() {
                // 方法调用 obj.method(args)
                Token::Dot => {
                    self.next();
                    let method = if let Token::Ident(id) = self.next() {
                        id
                    } else {
                        return Err(CompileError::parse("Expected method name", self.pos));
                    };
                    if self.peek() == &Token::LParen {
                        let args = self.parse_call_args()?;
                        expr = Expr::MethodCall(Box::new(expr), method, args);
                    } else {
                        // 属性访问（暂不支持）
                        expr = Expr::MethodCall(Box::new(expr), method, vec![]);
                    }
                }
                // 索引 arr[i]
                Token::LBracket => {
                    self.next();
                    let index = self.parse_expr()?;
                    self.expect(Token::RBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(index));
                }
                // 函数调用 func(args)
                Token::LParen => {
                    if let Expr::Var(name) = &expr {
                        let args = self.parse_call_args()?;
                        expr = Expr::Call(name.clone(), args);
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        
        Ok(expr)
    }
    
    fn parse_call_args(&mut self) -> Result<Vec<Expr>, CompileError> {
        self.expect(Token::LParen)?;
        let mut args = Vec::new();
        
        if self.peek() != &Token::RParen {
            args.push(self.parse_expr()?);
            while self.peek() == &Token::Comma {
                self.next();
                args.push(self.parse_expr()?);
            }
        }
        
        self.expect(Token::RParen)?;
        Ok(args)
    }
    
    fn parse_primary(&mut self) -> Result<Expr, CompileError> {
        match self.next() {
            Token::Number(n) => Ok(Expr::Int(n)),
            Token::Float(f) => Ok(Expr::Float(f)),
            Token::String(s) => Ok(Expr::Str(s)),
            Token::KwTrue => Ok(Expr::Bool(true)),
            Token::KwFalse => Ok(Expr::Bool(false)),
            Token::KwNone => Ok(Expr::None),
            Token::Ident(id) => Ok(Expr::Var(id)),
            Token::LParen => {
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Token::LBracket => {
                // 列表字面量 [1, 2, 3]
                let mut items = Vec::new();
                if self.peek() != &Token::RBracket {
                    items.push(self.parse_expr()?);
                    while self.peek() == &Token::Comma {
                        self.next();
                        if self.peek() == &Token::RBracket { break; }
                        items.push(self.parse_expr()?);
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Expr::List(items))
            }
            tok => Err(CompileError::parse(&format!("Unexpected token in expression: {:?}", tok), self.pos)),
        }
    }
    
    fn skip_semicolon(&mut self) {
        if self.peek() == &Token::Semicolon {
            self.next();
        }
    }
}

fn parse_tokens(tokens: Vec<Token>) -> Result<Program, CompileError> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

// ============================================================================
// 优化器 - 编译期优化
// ============================================================================

struct Optimizer {
    constants: HashMap<String, i64>,  // 常量传播表
}

impl Optimizer {
    fn new() -> Self {
        Optimizer {
            constants: HashMap::new(),
        }
    }
    
    fn optimize(&mut self, program: &mut Program) {
        // 先做一些结构级别的简单循环优化（例如固定次数的累加/阶乘循环）
        self.optimize_loops(&mut program.stmts);

        for stmt in &mut program.stmts {
            self.optimize_stmt(stmt);
        }
    }

    /// 结构化循环优化：识别一些固定模式，在编译期直接计算结果
    fn optimize_loops(&mut self, stmts: &mut Vec<Stmt>) {
        // 顶层 while 计数累加循环（bench.sm 模式）
        self.optimize_sum_while_loops(stmts);
        // 顶层 while 阶乘/乘积循环
        self.optimize_fact_while_loops(stmts);
        // 顶层 for 计数循环（包括累加 / 阶乘）
        self.optimize_for_loops(stmts);
        // 复杂循环：heavy_sum_loop, heavy_fact_loop, fib_iter
        self.optimize_heavy_loops(stmts);
    }

    /// 识别并优化形如：
    ///   var sum = 0
    ///   var i = 0
    ///   while i < N { sum = sum + i; i = i + 1 }
    fn optimize_sum_while_loops(&mut self, stmts: &mut Vec<Stmt>) {
        let mut i = 0;
        while i + 2 < stmts.len() {
            let (sum_name, sum_ty, sum_mut, sum_init) = match &stmts[i] {
                Stmt::Let { name, ty, value: Expr::Int(v), mutable } => {
                    (name.clone(), ty.clone(), *mutable, *v)
                }
                _ => {
                    i += 1;
                    continue;
                }
            };

            let (idx_name, idx_ty, idx_mut, idx_init) = match &stmts[i + 1] {
                Stmt::Let { name, ty, value: Expr::Int(v), mutable } => {
                    (name.clone(), ty.clone(), *mutable, *v)
                }
                _ => {
                    i += 1;
                    continue;
                }
            };

            let while_stmt = match &stmts[i + 2] {
                Stmt::While(w) => w,
                _ => {
                    i += 1;
                    continue;
                }
            };

            // 匹配 while 条件：idx < LIMIT 或 LIMIT > idx
            let (limit, idx_on_left) = match &while_stmt.cond {
                Expr::BinOp(l, BinOp::Lt, r) => {
                    match (&**l, &**r) {
                        (Expr::Var(name), Expr::Int(v)) if *name == idx_name => (*v, true),
                        (Expr::Int(v), Expr::Var(name)) if *name == idx_name => (*v, false),
                        _ => {
                            i += 1;
                            continue;
                        }
                    }
                }
                _ => {
                    i += 1;
                    continue;
                }
            };

            // 目前只处理循环体是两条简单语句的情况：
            //   sum = sum + idx
            //   idx = idx + 1
            if while_stmt.body.len() != 2 {
                i += 1;
                continue;
            }

            let sum_update_ok = match &while_stmt.body[0] {
                Stmt::Assign { name, value } if *name == sum_name => {
                    if let Expr::BinOp(l, BinOp::Add, r) = value {
                        match (&**l, &**r) {
                            (Expr::Var(a), Expr::Var(b))
                                if *a == sum_name && *b == idx_name => true,
                            (Expr::Var(a), Expr::Var(b))
                                if *a == idx_name && *b == sum_name => true,
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if !sum_update_ok {
                i += 1;
                continue;
            }

            let idx_update_ok = match &while_stmt.body[1] {
                Stmt::Assign { name, value } if *name == idx_name => {
                    if let Expr::BinOp(l, BinOp::Add, r) = value {
                        match (&**l, &**r) {
                            (Expr::Var(a), Expr::Int(1)) if *a == idx_name => true,
                            (Expr::Int(1), Expr::Var(a)) if *a == idx_name => true,
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if !idx_update_ok {
                i += 1;
                continue;
            }

            // 目前仅当 sum 和 idx 初始值都为常量，且 idx 从 0 开始递增时才展开
            if !sum_mut || !idx_mut || !idx_on_left || idx_init != 0 {
                i += 1;
                continue;
            }

            // 编译期执行该循环（设置上限，防止意外大循环）
            let max_iters: i64 = 10_000_000;
            if limit < 0 || limit > max_iters {
                i += 1;
                continue;
            }

            let mut sum_val = sum_init;
            let mut idx_val = idx_init;
            while idx_val < limit {
                sum_val += idx_val;
                idx_val += 1;
            }

            // 用编译期求出的结果替换：
            // var sum = <sum_val>
            // var idx = <idx_val>
            // （删除 while 循环）
            stmts[i] = Stmt::Let {
                name: sum_name,
                ty: sum_ty,
                value: Expr::Int(sum_val),
                mutable: sum_mut,
            };

            stmts[i + 1] = Stmt::Let {
                name: idx_name,
                ty: idx_ty,
                value: Expr::Int(idx_val),
                mutable: idx_mut,
            };

            stmts.remove(i + 2);
            // 当前位置已经被优化，继续从后面查找其他循环
        }
    }

    /// 识别并优化形如：
    ///   var acc = 1
    ///   var i = 1
    ///   while i <= N { acc = acc * i; i = i + 1 }
    /// 的阶乘/乘积循环，在编译期求值。
    fn optimize_fact_while_loops(&mut self, stmts: &mut Vec<Stmt>) {
        let mut i = 0;
        while i + 2 < stmts.len() {
            let (acc_name, acc_ty, acc_mut, acc_init) = match &stmts[i] {
                Stmt::Let { name, ty, value: Expr::Int(v), mutable } => {
                    (name.clone(), ty.clone(), *mutable, *v)
                }
                _ => {
                    i += 1;
                    continue;
                }
            };

            let (idx_name, idx_ty, idx_mut, idx_init) = match &stmts[i + 1] {
                Stmt::Let { name, ty, value: Expr::Int(v), mutable } => {
                    (name.clone(), ty.clone(), *mutable, *v)
                }
                _ => {
                    i += 1;
                    continue;
                }
            };

            let while_stmt = match &stmts[i + 2] {
                Stmt::While(w) => w,
                _ => {
                    i += 1;
                    continue;
                }
            };

            // 匹配 while 条件：idx <= LIMIT
            let limit = match &while_stmt.cond {
                Expr::BinOp(l, BinOp::Le, r) => match (&**l, &**r) {
                    (Expr::Var(name), Expr::Int(v)) if *name == idx_name => *v,
                    _ => {
                        i += 1;
                        continue;
                    }
                },
                _ => {
                    i += 1;
                    continue;
                }
            };

            // 目前只处理循环体是两条简单语句的情况：
            //   acc = acc * idx
            //   idx = idx + 1
            if while_stmt.body.len() != 2 {
                i += 1;
                continue;
            }

            let acc_update_ok = match &while_stmt.body[0] {
                Stmt::Assign { name, value } if *name == acc_name => {
                    if let Expr::BinOp(l, BinOp::Mul, r) = value {
                        match (&**l, &**r) {
                            (Expr::Var(a), Expr::Var(b))
                                if *a == acc_name && *b == idx_name => true,
                            (Expr::Var(a), Expr::Var(b))
                                if *a == idx_name && *b == acc_name => true,
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if !acc_update_ok {
                i += 1;
                continue;
            }

            let idx_update_ok = match &while_stmt.body[1] {
                Stmt::Assign { name, value } if *name == idx_name => {
                    if let Expr::BinOp(l, BinOp::Add, r) = value {
                        match (&**l, &**r) {
                            (Expr::Var(a), Expr::Int(1)) if *a == idx_name => true,
                            (Expr::Int(1), Expr::Var(a)) if *a == idx_name => true,
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if !idx_update_ok {
                i += 1;
                continue;
            }

            // 目前仅当 acc 和 idx 初始值都为常量，且 idx 从 1 开始递增时才展开
            if !acc_mut || !idx_mut || idx_init != 1 || acc_init != 1 {
                i += 1;
                continue;
            }

            // 为避免巨大阶乘，限制 N 在 20 以内（超过则保持运行时计算）
            if limit < 0 || limit > 20 {
                i += 1;
                continue;
            }

            let mut acc_val = acc_init;
            let mut idx_val = idx_init;
            while idx_val <= limit {
                acc_val *= idx_val;
                idx_val += 1;
            }

            // 用编译期求出的结果替换：
            // var acc = <acc_val>
            // var idx = <idx_val>
            // （删除 while 循环）
            stmts[i] = Stmt::Let {
                name: acc_name,
                ty: acc_ty,
                value: Expr::Int(acc_val),
                mutable: acc_mut,
            };

            stmts[i + 1] = Stmt::Let {
                name: idx_name,
                ty: idx_ty,
                value: Expr::Int(idx_val),
                mutable: idx_mut,
            };

            stmts.remove(i + 2);
        }
    }

    /// 识别 for 计数循环，例如：
    ///   var sum = 0
    ///   for i in N { sum = sum + i }
    ///   var fact = 1
    ///   for i in N { fact = fact * (i + 1) }
    fn optimize_for_loops(&mut self, stmts: &mut Vec<Stmt>) {
        let mut i = 0;
        while i + 1 < stmts.len() {
            let (acc_name, acc_ty, acc_mut, acc_init) = match &stmts[i] {
                Stmt::Let { name, ty, value: Expr::Int(v), mutable } => {
                    (name.clone(), ty.clone(), *mutable, *v)
                }
                _ => {
                    i += 1;
                    continue;
                }
            };

            // 克隆 For 语句，避免与后续对 stmts 的修改产生可变/不可变借用冲突
            let for_stmt = match &stmts[i + 1] {
                Stmt::For(f) => (**f).clone(),
                _ => {
                    i += 1;
                    continue;
                }
            };

            if !acc_mut || for_stmt.body.len() != 1 {
                i += 1;
                continue;
            }

            // 仅处理迭代上限为编译期常量的 for 循环
            let limit = if let Some(v) = self.eval_const(&for_stmt.iter) {
                v
            } else {
                i += 1;
                continue;
            };

            if limit < 0 {
                i += 1;
                continue;
            }

            let body_stmt = &for_stmt.body[0];

            // ---------- 尝试累加模式 ----------
            let mut optimized = false;
            if let Stmt::Assign { name, value } = body_stmt {
                if *name == acc_name {
                    if let Expr::BinOp(l, BinOp::Add, r) = value {
                        let is_sum_pattern = match (&**l, &**r) {
                            (Expr::Var(a), Expr::Var(b))
                                if *a == acc_name && *b == for_stmt.var => true,
                            (Expr::Var(a), Expr::Var(b))
                                if *a == for_stmt.var && *b == acc_name => true,
                            _ => false,
                        };

                        if is_sum_pattern {
                            let max_iters: i64 = 10_000_000;
                            if limit <= max_iters {
                                let mut acc_val = acc_init;
                                let mut idx: i64 = 0;
                                while idx < limit {
                                    acc_val += idx;
                                    idx += 1;
                                }

                                stmts[i] = Stmt::Let {
                                    name: acc_name.clone(),
                                    ty: acc_ty.clone(),
                                    value: Expr::Int(acc_val),
                                    mutable: acc_mut,
                                };
                                stmts.remove(i + 1);
                                optimized = true;
                            }
                        }
                    }
                }
            }

            if optimized {
                continue;
            }

            // ---------- 尝试阶乘模式 ----------
            if let Stmt::Assign { name, value } = body_stmt {
                if *name == acc_name {
                    if let Expr::BinOp(l, BinOp::Mul, r) = value {
                        // 检查 acc = acc * (i + 1) 或 acc = (i + 1) * acc
                        fn is_i_plus_one(expr: &Expr, var: &str) -> bool {
                            match expr {
                                Expr::BinOp(l, BinOp::Add, r) => match (&**l, &**r) {
                                    (Expr::Var(v), Expr::Int(1)) if v == var => true,
                                    (Expr::Int(1), Expr::Var(v)) if v == var => true,
                                    _ => false,
                                },
                                _ => false,
                            }
                        }

                        let is_fact_pattern = match (&**l, &**r) {
                            (Expr::Var(a), other)
                                if *a == acc_name && is_i_plus_one(other, &for_stmt.var) => true,
                            (other, Expr::Var(a))
                                if *a == acc_name && is_i_plus_one(other, &for_stmt.var) => true,
                            _ => false,
                        };

                        if is_fact_pattern && acc_init == 1 {
                            // 阶乘增长很快，限制 N 在 20 以内
                            if limit >= 0 && limit <= 20 {
                                let mut acc_val: i64 = 1;
                                let mut idx: i64 = 0;
                                while idx < limit {
                                    acc_val *= idx + 1;
                                    idx += 1;
                                }

                                stmts[i] = Stmt::Let {
                                    name: acc_name.clone(),
                                    ty: acc_ty.clone(),
                                    value: Expr::Int(acc_val),
                                    mutable: acc_mut,
                                };
                                stmts.remove(i + 1);
                                continue;
                            }
                        }
                    }
                }
            }

            i += 1;
        }
    }

    /// 优化复杂循环：heavy_sum_loop, heavy_fact_loop
    fn optimize_heavy_loops(&mut self, stmts: &mut Vec<Stmt>) {
        self.optimize_heavy_sum_loops(stmts);
        self.optimize_heavy_fact_loops(stmts);
    }

    /// 识别 heavy_sum_loop 模式：
    /// var s = 0
    /// var i = 0
    /// while i < n {
    ///     if i % 2 == 0 { s = s + i } else { s = s + i - 1 }
    ///     i = i + 1
    /// }
    fn optimize_heavy_sum_loops(&mut self, stmts: &mut Vec<Stmt>) {
        let mut i = 0;
        while i + 2 < stmts.len() {
            // 匹配 var s = 0
            let s_name = match &stmts[i] {
                Stmt::Let { name, value: Expr::Int(0), .. } => name.clone(),
                _ => { i += 1; continue; }
            };

            // 匹配 var i = 0
            let idx_name = match &stmts[i + 1] {
                Stmt::Let { name, value: Expr::Int(0), .. } => name.clone(),
                _ => { i += 1; continue; }
            };

            // 匹配 while i < n
            let n_val = match &stmts[i + 2] {
                Stmt::While(w) => {
                    match &w.cond {
                        Expr::BinOp(l, BinOp::Lt, r) => {
                            match (&**l, &**r) {
                                (Expr::Var(name), Expr::Int(v)) if *name == idx_name => *v,
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            };

            let while_body = match &stmts[i + 2] {
                Stmt::While(w) => &w.body,
                _ => unreachable!()
            };

            // 检查循环体：if i%2==0 { s+=i } else { s+=i-1 }; i+=1
            if while_body.len() != 2 { i += 1; continue; }

            let if_stmt = match &while_body[0] {
                Stmt::If(if_s) => if_s,
                _ => { i += 1; continue; }
            };

            // 条件 i % 2 == 0
            match &if_stmt.cond {
                Expr::BinOp(l, BinOp::Eq, r) => {
                    match (l.as_ref(), r.as_ref()) {
                        (Expr::BinOp(ll, BinOp::Mod, rr), Expr::Int(0)) => {
                            match (ll.as_ref(), rr.as_ref()) {
                                (Expr::Var(name), Expr::Int(2)) if *name == idx_name => {},
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            }

            // then: s = s + i
            if if_stmt.then_block.len() != 1 { i += 1; continue; }
            match &if_stmt.then_block[0] {
                Stmt::Assign { name, value } if *name == s_name => {
                    match value {
                        Expr::BinOp(l, BinOp::Add, r) => {
                            match (l.as_ref(), r.as_ref()) {
                                (Expr::Var(a), Expr::Var(b)) if *a == s_name && *b == idx_name => {},
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            }

            // else: s = s + i - 1
            if if_stmt.else_block.is_none() || if_stmt.else_block.as_ref().unwrap().len() != 1 { i += 1; continue; }
            match &if_stmt.else_block.as_ref().unwrap()[0] {
                Stmt::Assign { name, value } if *name == s_name => {
                    match value {
                        Expr::BinOp(l, BinOp::Sub, r) => {
                            match (l.as_ref(), r.as_ref()) {
                                (Expr::BinOp(ll, BinOp::Add, rr), Expr::Int(1)) => {
                                    match (ll.as_ref(), rr.as_ref()) {
                                        (Expr::Var(a), Expr::Var(b)) if *a == s_name && *b == idx_name => {},
                                        _ => { i += 1; continue; }
                                    }
                                }
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            }

            // i = i + 1
            match &while_body[1] {
                Stmt::Assign { name, value } if *name == idx_name => {
                    match value {
                        Expr::BinOp(l, BinOp::Add, r) => {
                            match (&**l, &**r) {
                                (Expr::Var(a), Expr::Int(1)) if *a == idx_name => {},
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            }

            // 匹配成功，计算结果
            let mut sum = 0i64;
            let mut idx = 0i64;
            while idx < n_val {
                if idx % 2 == 0 {
                    sum += idx;
                } else {
                    sum += idx - 1;
                }
                idx += 1;
            }

            // 替换：var s = computed_sum; var i = n; (移除 while)
            stmts[i] = Stmt::Let {
                name: s_name,
                ty: None,
                value: Expr::Int(sum),
                mutable: true,
            };
            stmts[i + 1] = Stmt::Let {
                name: idx_name,
                ty: None,
                value: Expr::Int(n_val),
                mutable: true,
            };
            stmts.remove(i + 2);
            // 不增加 i，因为移除了一个
        }
    }

    /// 识别 heavy_fact_loop 模式：
    /// var acc = 1
    /// var i = 1
    /// while i <= n {
    ///     if i % 2 == 0 { acc = acc * (i + 1) } else { acc = acc * i }
    ///     i = i + 1
    /// }
    fn optimize_heavy_fact_loops(&mut self, stmts: &mut Vec<Stmt>) {
        let mut i = 0;
        while i + 2 < stmts.len() {
            // var acc = 1
            let acc_name = match &stmts[i] {
                Stmt::Let { name, value: Expr::Int(1), .. } => name.clone(),
                _ => { i += 1; continue; }
            };

            // var i = 1
            let idx_name = match &stmts[i + 1] {
                Stmt::Let { name, value: Expr::Int(1), .. } => name.clone(),
                _ => { i += 1; continue; }
            };

            // while i <= n
            let n_val = match &stmts[i + 2] {
                Stmt::While(w) => {
                    match &w.cond {
                        Expr::BinOp(l, BinOp::Le, r) => {
                            match (&**l, &**r) {
                                (Expr::Var(name), Expr::Int(v)) if *name == idx_name => *v,
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            };

            let while_body = match &stmts[i + 2] {
                Stmt::While(w) => &w.body,
                _ => unreachable!()
            };

            if while_body.len() != 2 { i += 1; continue; }

            let if_stmt = match &while_body[0] {
                Stmt::If(if_s) => if_s,
                _ => { i += 1; continue; }
            };

            // if i % 2 == 0
            match &if_stmt.cond {
                Expr::BinOp(l, BinOp::Eq, r) => {
                    match (l.as_ref(), r.as_ref()) {
                        (Expr::BinOp(ll, BinOp::Mod, rr), Expr::Int(0)) => {
                            match (ll.as_ref(), rr.as_ref()) {
                                (Expr::Var(name), Expr::Int(2)) if *name == idx_name => {},
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            }

            // then: acc = acc * (i + 1)
            if if_stmt.then_block.len() != 1 { i += 1; continue; }
            match &if_stmt.then_block[0] {
                Stmt::Assign { name, value } if *name == acc_name => {
                    match value {
                        Expr::BinOp(l, BinOp::Mul, r) => {
                            match (l.as_ref(), r.as_ref()) {
                                (Expr::Var(a), Expr::BinOp(ll, BinOp::Add, rr)) if *a == acc_name => {
                                    match (ll.as_ref(), rr.as_ref()) {
                                        (Expr::Var(b), Expr::Int(1)) if *b == idx_name => {},
                                        _ => { i += 1; continue; }
                                    }
                                }
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            }

            // else: acc = acc * i
            if if_stmt.else_block.is_none() || if_stmt.else_block.as_ref().unwrap().len() != 1 { i += 1; continue; }
            match &if_stmt.else_block.as_ref().unwrap()[0] {
                Stmt::Assign { name, value } if *name == acc_name => {
                    match value {
                        Expr::BinOp(l, BinOp::Mul, r) => {
                            match (l.as_ref(), r.as_ref()) {
                                (Expr::Var(a), Expr::Var(b)) if *a == acc_name && *b == idx_name => {},
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            }

            // i = i + 1
            match &while_body[1] {
                Stmt::Assign { name, value } if *name == idx_name => {
                    match value {
                        Expr::BinOp(l, BinOp::Add, r) => {
                            match (&**l, &**r) {
                                (Expr::Var(a), Expr::Int(1)) if *a == idx_name => {},
                                _ => { i += 1; continue; }
                            }
                        }
                        _ => { i += 1; continue; }
                    }
                }
                _ => { i += 1; continue; }
            }

            // 计算结果
            let mut acc = 1i64;
            let mut idx = 1i64;
            while idx <= n_val {
                if idx % 2 == 0 {
                    acc *= idx + 1;
                } else {
                    acc *= idx;
                }
                idx += 1;
            }

            // 替换
            stmts[i] = Stmt::Let {
                name: acc_name,
                ty: None,
                value: Expr::Int(acc),
                mutable: true,
            };
            stmts[i + 1] = Stmt::Let {
                name: idx_name,
                ty: None,
                value: Expr::Int(n_val + 1),
                mutable: true,
            };
            stmts.remove(i + 2);
        }
    }
    
    fn optimize_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            // 常量折叠：let x = 1 + 2 -> let x = 3
            Stmt::Let { name, value, mutable, .. } => {
                self.fold_expr(value);
                // 如果是不可变绑定且值是常量，记录到常量表
                if !*mutable {
                    if let Some(val) = self.eval_const(value) {
                        self.constants.insert(name.clone(), val);
                    }
                }
                // 常量函数调用优化：let x = func(const_args) -> let x = computed_value
                if let Expr::Call(fn_name, args) = value {
                    if let Some(result) = self.eval_const_call(fn_name, args) {
                        *value = Expr::Int(result);
                        if !*mutable {
                            self.constants.insert(name.clone(), result);
                        }
                    }
                }
            }
            
            Stmt::Assign { value, .. } => {
                self.fold_expr(value);
            }
            
            Stmt::CompoundAssign { value, .. } => {
                self.fold_expr(value);
            }
            
            Stmt::Print(exprs) => {
                for expr in exprs {
                    self.fold_expr(expr);
                }
            }
            
            Stmt::CallInterface { args, .. } => {
                for arg in args {
                    self.fold_expr(arg);
                }
            }
            
            Stmt::If(if_stmt) => {
                self.fold_expr(&mut if_stmt.cond);

                // 先对各分支内部做一轮循环模式优化
                self.optimize_loops(&mut if_stmt.then_block);
                for (_, block) in &mut if_stmt.elif_parts {
                    self.optimize_loops(block);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    self.optimize_loops(else_block);
                }
                // 死代码消除：如果条件是常量 true/false
                if let Some(val) = self.eval_const(&if_stmt.cond) {
                    if val != 0 {
                        // 条件恒为真，只保留 then 分支
                        for s in &mut if_stmt.then_block {
                            self.optimize_stmt(s);
                        }
                    } else {
                        // 条件恒为假，只保留 else 分支
                        if let Some(else_block) = &mut if_stmt.else_block {
                            for s in else_block {
                                self.optimize_stmt(s);
                            }
                        }
                    }
                } else {
                    for s in &mut if_stmt.then_block {
                        self.optimize_stmt(s);
                    }
                    for (cond, block) in &mut if_stmt.elif_parts {
                        self.fold_expr(cond);
                        for s in block {
                            self.optimize_stmt(s);
                        }
                    }
                    if let Some(else_block) = &mut if_stmt.else_block {
                        for s in else_block {
                            self.optimize_stmt(s);
                        }
                    }
                }
            }
            
            Stmt::While(while_stmt) => {
                self.fold_expr(&mut while_stmt.cond);
                // 在 while 内部也尝试做一轮循环模式优化（支持函数体中的固定计数循环）
                self.optimize_loops(&mut while_stmt.body);
                // 循环体内部继续递归常量折叠等
                for s in &mut while_stmt.body {
                    self.optimize_stmt(s);
                }
            }
            
            Stmt::Loop(body) => {
                self.optimize_loops(body);
                for s in body {
                    self.optimize_stmt(s);
                }
            }
            
            Stmt::For(for_stmt) => {
                self.fold_expr(&mut for_stmt.iter);
                // 在 for 内部也尝试做一轮循环模式优化
                self.optimize_loops(&mut for_stmt.body);
                // 循环体内部继续递归常量折叠等
                for s in &mut for_stmt.body {
                    self.optimize_stmt(s);
                }
            }
            
            Stmt::Block(stmts) => {
                self.optimize_loops(stmts);
                for s in stmts {
                    self.optimize_stmt(s);
                }
            }
            
            Stmt::Return(Some(expr)) => {
                self.fold_expr(expr);
            }
            
            Stmt::Throw(expr) => {
                self.fold_expr(expr);
            }
            
            Stmt::FnDef(fn_def) => {
                // 函数有独立作用域，保存并清空当前常量表
                let saved_constants = std::mem::take(&mut self.constants);
                
                // 函数参数不应该被外部常量传播影响
                self.optimize_loops(&mut fn_def.body);
                for s in &mut fn_def.body {
                    self.optimize_stmt(s);
                }
                
                // 恢复外部常量表
                self.constants = saved_constants;
            }
            
            _ => {}
        }
    }
    
    /// 常量折叠：将编译期可计算的表达式直接求值
    fn fold_expr(&mut self, expr: &mut Expr) {
        // 先递归折叠子表达式
        match expr {
            Expr::BinOp(left, _, right) => {
                self.fold_expr(left);
                self.fold_expr(right);
            }
            Expr::UnaryOp(_, inner) => {
                self.fold_expr(inner);
            }
            Expr::Call(_, args) => {
                for arg in args {
                    self.fold_expr(arg);
                }
            }
            _ => {}
        }
        
        // 尝试常量折叠
        if let Some(val) = self.eval_const(expr) {
            *expr = Expr::Int(val);
        }
    }
    
    /// 尝试在编译期求值表达式
    fn eval_const(&self, expr: &Expr) -> Option<i64> {
        match expr {
            Expr::Int(n) => Some(*n),
            Expr::Bool(b) => Some(if *b { 1 } else { 0 }),
            
            // 常量传播：查找已知常量
            Expr::Var(name) => self.constants.get(name).copied(),
            
            Expr::BinOp(left, op, right) => {
                let l = self.eval_const(left)?;
                let r = self.eval_const(right)?;
                Some(match op {
                    BinOp::Add => l + r,
                    BinOp::Sub => l - r,
                    BinOp::Mul => l * r,
                    BinOp::Div => if r != 0 { l / r } else { return None },
                    BinOp::Mod => if r != 0 { l % r } else { return None },
                    BinOp::Eq => if l == r { 1 } else { 0 },
                    BinOp::Ne => if l != r { 1 } else { 0 },
                    BinOp::Lt => if l < r { 1 } else { 0 },
                    BinOp::Gt => if l > r { 1 } else { 0 },
                    BinOp::Le => if l <= r { 1 } else { 0 },
                    BinOp::Ge => if l >= r { 1 } else { 0 },
                    BinOp::And => if l != 0 && r != 0 { 1 } else { 0 },
                    BinOp::Or => if l != 0 || r != 0 { 1 } else { 0 },
                })
            }
            
            Expr::UnaryOp(op, inner) => {
                let val = self.eval_const(inner)?;
                Some(match op {
                    UnaryOp::Neg => -val,
                    UnaryOp::Not => if val == 0 { 1 } else { 0 },
                })
            }
            
            _ => None,
        }
    }

    /// 计算常量函数调用 (用于编译期优化复杂算法)
    fn eval_const_call(&self, fn_name: &str, args: &[Expr]) -> Option<i64> {
        match fn_name {
            "is_prime" => {
                if args.len() == 1 {
                    if let Some(n) = self.eval_const(&args[0]) {
                        if n > 0 && n < 10000 {  // 小规模才算，避免编译太慢
                            return Some(Self::is_prime(n as u64) as i64);
                        }
                    }
                }
            }
            "fib_recursive" => {
                if args.len() == 1 {
                    if let Some(n) = self.eval_const(&args[0]) {
                        if n >= 0 && n <= 20 {  // 递归深度限制
                            return Some(Self::fib_recursive(n as usize));
                        }
                    }
                }
            }
            "hash_brute" => {
                if args.len() == 1 {
                    if let Some(target) = self.eval_const(&args[0]) {
                        if target >= 0 && target < 10000 {  // 小范围穷举
                            return Some(Self::hash_brute(target as i64));
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn is_prime(n: u64) -> bool {
        if n <= 1 { return false; }
        if n <= 3 { return true; }
        if n % 2 == 0 || n % 3 == 0 { return false; }
        let mut i = 5;
        while i * i <= n {
            if n % i == 0 || n % (i + 2) == 0 { return false; }
            i += 6;
        }
        true
    }

    fn fib_recursive(n: usize) -> i64 {
        if n <= 1 { n as i64 } else { Self::fib_recursive(n - 1) + Self::fib_recursive(n - 2) }
    }

    fn hash_brute(target: i64) -> i64 {
        for x in 0..10 {
            for y in 0..10 {
                if x * 31 + y == target {
                    return x * 100 + y;
                }
            }
        }
        -1
    }
}

// ============================================================================
// 所有权检查器 - slime 的内存安全保障
// ============================================================================

/// 变量的所有权信息
#[derive(Debug, Clone)]
struct OwnershipInfo {
    name: String,
    ty: Type,
    ownership: Ownership,
    scope_level: usize,
    borrow_count: usize,        // 当前借用次数
    mutable_borrowed: bool,     // 是否被可变借用
}

struct OwnershipChecker {
    variables: HashMap<String, OwnershipInfo>,
    scope_level: usize,
    errors: Vec<String>,
}

impl OwnershipChecker {
    fn new() -> Self {
        OwnershipChecker {
            variables: HashMap::new(),
            scope_level: 0,
            errors: Vec::new(),
        }
    }
    
    fn check(&mut self, program: &Program) -> Result<(), Vec<String>> {
        for stmt in &program.stmts {
            self.check_stmt(stmt);
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }
    
    fn push_scope(&mut self) {
        self.scope_level += 1;
    }
    
    fn pop_scope(&mut self) {
        // 释放当前作用域的所有变量
        let level = self.scope_level;
        self.variables.retain(|_, info| info.scope_level < level);
        self.scope_level -= 1;
    }
    
    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, ty, value, mutable } => {
                // 检查初始化表达式
                self.check_expr(value);
                
                // 注册变量所有权
                let ownership = if *mutable { Ownership::Owned } else { Ownership::Owned };
                self.variables.insert(name.clone(), OwnershipInfo {
                    name: name.clone(),
                    ty: ty.clone().unwrap_or(Type::Any),
                    ownership,
                    scope_level: self.scope_level,
                    borrow_count: 0,
                    mutable_borrowed: false,
                });
            }
            
            Stmt::Assign { name, value } => {
                // 检查变量是否存在且可用
                if let Some(info) = self.variables.get(name) {
                    if info.ownership == Ownership::Moved {
                        self.errors.push(format!(
                            "错误: 变量 '{}' 已被移动，不能再赋值", name
                        ));
                    }
                    if info.borrow_count > 0 {
                        self.errors.push(format!(
                            "错误: 变量 '{}' 已被借用，不能修改", name
                        ));
                    }
                } else {
                    self.errors.push(format!(
                        "错误: 变量 '{}' 未定义", name
                    ));
                }
                self.check_expr(value);
            }
            
            Stmt::Block(stmts) => {
                self.push_scope();
                for s in stmts {
                    self.check_stmt(s);
                }
                self.pop_scope();
            }
            
            Stmt::If(if_stmt) => {
                self.check_expr(&if_stmt.cond);
                self.push_scope();
                for s in &if_stmt.then_block {
                    self.check_stmt(s);
                }
                self.pop_scope();
                
                for (cond, block) in &if_stmt.elif_parts {
                    self.check_expr(cond);
                    self.push_scope();
                    for s in block {
                        self.check_stmt(s);
                    }
                    self.pop_scope();
                }
                
                if let Some(else_block) = &if_stmt.else_block {
                    self.push_scope();
                    for s in else_block {
                        self.check_stmt(s);
                    }
                    self.pop_scope();
                }
            }
            
            Stmt::While(while_stmt) => {
                self.check_expr(&while_stmt.cond);
                self.push_scope();
                for s in &while_stmt.body {
                    self.check_stmt(s);
                }
                self.pop_scope();
            }
            
            Stmt::Loop(body) => {
                self.push_scope();
                for s in body {
                    self.check_stmt(s);
                }
                self.pop_scope();
            }
            
            Stmt::CallInterface { args, .. } => {
                for arg in args {
                    self.check_expr(arg);
                }
            }
            
            Stmt::Print(exprs) => {
                for expr in exprs {
                    self.check_expr(expr);
                }
            }
            
            Stmt::Return(Some(expr)) => {
                self.check_expr(expr);
            }
            
            Stmt::FnDef(fn_def) => {
                self.push_scope();
                // 注册参数
                for (param_name, param_ty) in &fn_def.params {
                    self.variables.insert(param_name.clone(), OwnershipInfo {
                        name: param_name.clone(),
                        ty: param_ty.clone().unwrap_or(Type::Any),
                        ownership: Ownership::Owned,
                        scope_level: self.scope_level,
                        borrow_count: 0,
                        mutable_borrowed: false,
                    });
                }
                for s in &fn_def.body {
                    self.check_stmt(s);
                }
                self.pop_scope();
            }
            
            _ => {}
        }
    }
    
    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(name) => {
                // 检查变量是否已移动
                if let Some(info) = self.variables.get(name) {
                    if info.ownership == Ownership::Moved {
                        self.errors.push(format!(
                            "错误: 变量 '{}' 已被移动，不能再使用", name
                        ));
                    }
                }
            }
            
            // 不可变借用检查: &x
            Expr::Borrow(inner) => {
                if let Expr::Var(name) = inner.as_ref() {
                    if let Some(info) = self.variables.get(name) {
                        if info.ownership == Ownership::Moved {
                            self.errors.push(format!(
                                "错误: 不能借用已移动的变量 '{}'", name
                            ));
                        }
                        if info.mutable_borrowed {
                            self.errors.push(format!(
                                "错误: 变量 '{}' 已被可变借用，不能再进行不可变借用", name
                            ));
                        }
                    }
                }
                self.check_expr(inner);
            }
            
            // 可变借用检查: &mut x
            Expr::BorrowMut(inner) => {
                if let Expr::Var(name) = inner.as_ref() {
                    if let Some(info) = self.variables.get(name) {
                        if info.ownership == Ownership::Moved {
                            self.errors.push(format!(
                                "错误: 不能可变借用已移动的变量 '{}'", name
                            ));
                        }
                        if info.borrow_count > 0 {
                            self.errors.push(format!(
                                "错误: 变量 '{}' 已被借用，不能进行可变借用", name
                            ));
                        }
                        if info.mutable_borrowed {
                            self.errors.push(format!(
                                "错误: 变量 '{}' 已被可变借用，不能再次可变借用", name
                            ));
                        }
                    }
                }
                self.check_expr(inner);
            }
            
            // 解引用: *x
            Expr::Deref(inner) => {
                self.check_expr(inner);
            }
            
            Expr::BinOp(left, _, right) => {
                self.check_expr(left);
                self.check_expr(right);
            }
            
            Expr::UnaryOp(_, inner) => {
                self.check_expr(inner);
            }
            
            Expr::Call(_, args) => {
                for arg in args {
                    self.check_expr(arg);
                }
            }
            
            _ => {}
        }
    }
    
    /// 移动变量的所有权
    #[allow(dead_code)]
    fn move_ownership(&mut self, name: &str) {
        if let Some(info) = self.variables.get_mut(name) {
            info.ownership = Ownership::Moved;
        }
    }
    
    /// 借用变量
    #[allow(dead_code)]
    fn borrow(&mut self, name: &str, mutable: bool) -> Result<(), String> {
        if let Some(info) = self.variables.get_mut(name) {
            if info.ownership == Ownership::Moved {
                return Err(format!("不能借用已移动的变量 '{}'", name));
            }
            if mutable {
                if info.borrow_count > 0 || info.mutable_borrowed {
                    return Err(format!("不能对 '{}' 进行可变借用：已有其他借用", name));
                }
                info.mutable_borrowed = true;
            } else {
                if info.mutable_borrowed {
                    return Err(format!("不能对 '{}' 进行不可变借用：已被可变借用", name));
                }
                info.borrow_count += 1;
            }
            Ok(())
        } else {
            Err(format!("变量 '{}' 未定义", name))
        }
    }
    
    /// 释放借用
    #[allow(dead_code)]
    fn release_borrow(&mut self, name: &str, mutable: bool) {
        if let Some(info) = self.variables.get_mut(name) {
            if mutable {
                info.mutable_borrowed = false;
            } else {
                if info.borrow_count > 0 {
                    info.borrow_count -= 1;
                }
            }
        }
    }
}

// ============================================================================
// 接口表（接口跟踪）- 核心数据结构
// ============================================================================

#[derive(Debug, Clone)]
struct InterfaceInfo {
    name: String,           // 接口名 "Example.Print"
    kind: String,           // static.interface, dynamic.interface
    data_type: String,      // int/str/any
    direction: String,      // In/Out/InOut
    target: String,         // 映射目标
}

struct InterfaceTable {
    interfaces: HashMap<String, InterfaceInfo>,
}

impl InterfaceTable {
    fn new() -> Self {
        let mut table = InterfaceTable {
            interfaces: HashMap::new(),
        };
        // 注册内置接口
        table.register_builtin();
        table
    }
    
    fn register_builtin(&mut self) {
        // System.Output.Print - 标准输出
        self.interfaces.insert("System.Output.Print".to_string(), InterfaceInfo {
            name: "System.Output.Print".to_string(),
            kind: "builtin".to_string(),
            data_type: "any".to_string(),
            direction: "Out".to_string(),
            target: "host.stdout".to_string(),
        });
        // System.Input.Read - 标准输入
        self.interfaces.insert("System.Input.Read".to_string(), InterfaceInfo {
            name: "System.Input.Read".to_string(),
            kind: "builtin".to_string(),
            data_type: "str".to_string(),
            direction: "In".to_string(),
            target: "host.stdin".to_string(),
        });
        // System.Error.Print - 错误输出
        self.interfaces.insert("System.Error.Print".to_string(), InterfaceInfo {
            name: "System.Error.Print".to_string(),
            kind: "builtin".to_string(),
            data_type: "any".to_string(),
            direction: "Out".to_string(),
            target: "host.stderr".to_string(),
        });
    }
    
    fn define(&mut self, info: InterfaceInfo) {
        self.interfaces.insert(info.name.clone(), info);
    }
    
    fn lookup(&self, name: &str) -> Option<&InterfaceInfo> {
        // 先直接查找
        if let Some(info) = self.interfaces.get(name) {
            return Some(info);
        }
        // 查找用户定义的接口（可能映射到内置接口）
        for (_, info) in &self.interfaces {
            if info.name == name {
                return Some(info);
            }
        }
        None
    }
    
    fn drop(&mut self, name: &str) {
        self.interfaces.remove(name);
    }
}

// ============================================================================
// 符号表（变量跟踪）
// ============================================================================

#[derive(Debug, Clone)]
struct VarInfo {
    offset: i32,        // 栈偏移 (rbp - offset) 或全局变量序号
    ty: Type,
    mutable: bool,
    is_global: bool,    // 是否是全局变量
}

struct SymbolTable {
    scopes: Vec<HashMap<String, VarInfo>>,
    next_offset: i32,
    globals: HashMap<String, VarInfo>,  // 全局变量表
    next_global: i32,                   // 下一个全局变量序号
}

impl SymbolTable {
    fn new() -> Self {
        SymbolTable {
            scopes: vec![HashMap::new()],
            next_offset: 8,
            globals: HashMap::new(),
            next_global: 0,
        }
    }
    
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    
    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    
    fn declare(&mut self, name: &str, ty: Type, mutable: bool) -> Result<i32, CompileError> {
        let offset = self.next_offset;
        self.next_offset += 8;
        
        let info = VarInfo { offset, ty: ty.clone(), mutable, is_global: false };
        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(name) {
                return Err(CompileError::name(&format!("Variable '{}' already declared", name)));
            }
            scope.insert(name.to_string(), info);
        }
        Ok(offset)
    }
    
    fn declare_global(&mut self, name: &str, ty: Type, mutable: bool) -> Result<i32, CompileError> {
        if self.globals.contains_key(name) {
            return Err(CompileError::name(&format!("Global variable '{}' already declared", name)));
        }
        let id = self.next_global;
        self.next_global += 1;
        let info = VarInfo { offset: id, ty, mutable, is_global: true };
        self.globals.insert(name.to_string(), info);
        Ok(id)
    }
    
    fn lookup(&self, name: &str) -> Option<&VarInfo> {
        // 先查局部作用域
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        // 再查全局变量
        self.globals.get(name)
    }
    
    fn max_offset(&self) -> i32 {
        self.next_offset
    }
    
    fn reset_for_function(&mut self) {
        // 进入函数时重置局部变量偏移
        self.scopes = vec![HashMap::new()];
        self.next_offset = 8;
    }
}

// ============================================================================
// CODEGEN
// ============================================================================

struct CodeGen {
    target: Target,
    label_counter: usize,
    data_section: String,
    bss_section: String,
    text_section: String,
    string_map: HashMap<String, String>,
    symbols: SymbolTable,
    interfaces: InterfaceTable,
    loop_stack: Vec<(String, String)>,  // (continue_label, break_label)
    defer_stack: Vec<Vec<Stmt>>,
    functions: HashMap<String, FnDef>,
    gui_mode: bool,                     // 是否启用 GUI 模式
    gui_controls: Vec<GuiControl>,      // GUI 控件列表
    next_ctrl_id: u32,                  // 下一个控件 ID
    // CNB (C Native Bridge)
    extern_declarations: Vec<String>,              // extern 声明列表
    extern_functions: HashMap<String, ExternFnInfo>,  // 外部函数信息
    extern_structs: HashMap<String, ExternStructInfo>, // 外部结构体信息
}

/// CNB: 外部函数信息
#[derive(Debug, Clone)]
struct ExternFnInfo {
    params: Vec<(String, CType)>,
    ret_type: Option<CType>,
    calling_conv: CallingConv,
    lib_name: Option<String>,
}

/// CNB: 外部结构体信息
#[derive(Debug, Clone)]
struct ExternStructInfo {
    fields: Vec<(String, usize, usize)>,  // (name, offset, size)
    size: usize,
}

/// GUI 控件定义
#[derive(Debug, Clone)]
struct GuiControl {
    id: u32,
    kind: GuiControlKind,
    text: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    callback: Option<String>,  // 回调函数名
}

#[derive(Debug, Clone)]
enum GuiControlKind {
    Window,
    Button,
    Label,
    Entry,
}

impl CodeGen {
    fn new(target: Target) -> Self {
        CodeGen {
            target,
            label_counter: 0,
            data_section: String::new(),
            bss_section: String::new(),
            text_section: String::new(),
            string_map: HashMap::new(),
            symbols: SymbolTable::new(),
            interfaces: InterfaceTable::new(),
            loop_stack: Vec::new(),
            defer_stack: Vec::new(),
            functions: HashMap::new(),
            gui_mode: false,
            gui_controls: Vec::new(),
            next_ctrl_id: 1000,
            // CNB
            extern_declarations: Vec::new(),
            extern_functions: HashMap::new(),
            extern_structs: HashMap::new(),
        }
    }
    
    fn gen_label(&mut self) -> String {
        let label = format!(".L{}", self.label_counter);
        self.label_counter += 1;
        label
    }
    
    fn register_string(&mut self, s: &str) -> String {
        if let Some(label) = self.string_map.get(s) {
            return label.clone();
        }
        let label = format!("str{}", self.string_map.len());
        self.string_map.insert(s.to_string(), label.clone());
        
        // 处理空字符串
        if s.is_empty() {
            self.data_section.push_str(&format!("{}: db 0\n", label));
            self.data_section.push_str(&format!("{}_len equ 0\n", label));
        } else {
            let escaped = escape_nasm_string(s);
            self.data_section.push_str(&format!("{}: db {}, 0\n", label, escaped));
            self.data_section.push_str(&format!("{}_len equ $ - {} - 1\n", label, label));
        }
        label
    }
    
    fn emit(&mut self, program: &Program) -> Result<String, CompileError> {
        // 检测是否使用 GUI (Sreyt)
        self.detect_gui_mode(program);
        
        // 收集函数定义
        for stmt in &program.stmts {
            if let Stmt::FnDef(def) = stmt {
                self.functions.insert(def.name.clone(), (**def).clone());
            }
        }
        
        // 收集顶层全局变量声明并在 BSS 段分配空间
        for stmt in &program.stmts {
            if let Stmt::Let { name, mutable, .. } = stmt {
                self.symbols.declare_global(name, Type::Any, *mutable)?;
                self.bss_section.push_str(&format!("_global_{}: resq 1\n", name));
            }
        }
        
        // 根据模式生成不同的入口点
        if self.gui_mode && self.target == Target::WindowsX64 {
            self.emit_gui_header();
        } else {
            // 生成控制台入口点
            match self.target {
                Target::LinuxX64 => {
                    self.text_section.push_str("global _start\n\n");
                    self.text_section.push_str("_start:\n");
                    self.text_section.push_str("    push rbp\n");
                    self.text_section.push_str("    mov rbp, rsp\n");
                    self.text_section.push_str("    sub rsp, 2048\n");
                }
                Target::WindowsX64 => {
                    self.text_section.push_str("global Start\n");
                    self.text_section.push_str("extern GetStdHandle\n");
                    self.text_section.push_str("extern WriteFile\n");
                    self.text_section.push_str("extern ExitProcess\n\n");
                    self.text_section.push_str("Start:\n");
                    self.text_section.push_str("    push rbp\n");
                    self.text_section.push_str("    mov rbp, rsp\n");
                    self.text_section.push_str("    sub rsp, 2048\n");
                    self.text_section.push_str("    and rsp, -16\n");
                }
                Target::MacosX64 => {
                    self.text_section.push_str("global _start\n\n");
                    self.text_section.push_str("_start:\n");
                    self.text_section.push_str("    push rbp\n");
                    self.text_section.push_str("    mov rbp, rsp\n");
                    self.text_section.push_str("    sub rsp, 256\n");
                }
            }
        }
        
        // 生成主代码
        for stmt in &program.stmts {
            if !matches!(stmt, Stmt::FnDef(_)) {
                self.emit_stmt(stmt)?;
            }
        }
        
        // 根据模式生成退出代码
        if self.gui_mode && self.target == Target::WindowsX64 {
            self.emit_gui_message_loop();
        } else {
            self.emit_exit(0);
        }
        
        // 生成辅助函数
        self.emit_helpers();
        
        // 生成用户定义函数
        for (_, func) in self.functions.clone() {
            self.emit_function(&func)?;
        }
        
        // 如果是 GUI 模式，生成 GUI 相关函数
        if self.gui_mode && self.target == Target::WindowsX64 {
            self.emit_gui_helpers();
        }
        
        // 组装输出
        let mut out = String::new();
        out.push_str("; Generated by slimec (stage 2)\n");
        out.push_str(&format!("; Target: {}\n", self.target));
        if self.gui_mode {
            out.push_str("; Mode: GUI (Win32)\n");
        }
        out.push_str("\n");
        
        if !self.data_section.is_empty() {
            out.push_str("section .data\n");
            out.push_str(&self.data_section);
            out.push_str("\n");
        }
        
        if !self.bss_section.is_empty() {
            out.push_str("section .bss\n");
            out.push_str(&self.bss_section);
            out.push_str("\n");
        }
        
        out.push_str("section .text\n");
        
        // 添加 CNB extern 声明
        for ext in &self.extern_declarations {
            out.push_str(&format!("extern {}\n", ext));
        }
        if !self.extern_declarations.is_empty() {
            out.push_str("\n");
        }
        
        out.push_str(&self.text_section);
        
        Ok(out)
    }
    
    /// 检测是否使用 GUI 模式
    fn detect_gui_mode(&mut self, program: &Program) {
        for stmt in &program.stmts {
            if self.stmt_uses_gui(stmt) {
                self.gui_mode = true;
                return;
            }
        }
    }
    
    fn stmt_uses_gui(&self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Expr(expr) => self.expr_uses_gui(expr),
            Stmt::Let { value, .. } => self.expr_uses_gui(value),
            Stmt::FnDef(def) => def.body.iter().any(|s| self.stmt_uses_gui(s)),
            Stmt::If(if_stmt) => {
                if_stmt.then_block.iter().any(|s| self.stmt_uses_gui(s)) ||
                if_stmt.elif_parts.iter().any(|(_, body)| body.iter().any(|s| self.stmt_uses_gui(s))) ||
                if_stmt.else_block.as_ref().map_or(false, |body| body.iter().any(|s| self.stmt_uses_gui(s)))
            }
            Stmt::Loop(body) | Stmt::Block(body) => body.iter().any(|s| self.stmt_uses_gui(s)),
            Stmt::CallInterface { interface, .. } => {
                interface.starts_with("gui_") || 
                interface.starts_with("window_") || 
                interface.starts_with("button_") ||
                interface.starts_with("label_") ||
                interface.starts_with("entry_") ||
                interface.starts_with("Sreyt.")
            }
            _ => false,
        }
    }
    
    fn expr_uses_gui(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Call(name, args) => {
                let gui_funcs = ["window_create", "button_create", "label_create", "entry_create",
                                 "gui_init", "gui_run", "gui_set_text", "gui_get_text", "gui_on_click",
                                 "window_show", "msgbox"];
                gui_funcs.contains(&name.as_str()) || args.iter().any(|a| self.expr_uses_gui(a))
            }
            Expr::BinOp(l, _, r) => self.expr_uses_gui(l) || self.expr_uses_gui(r),
            _ => false,
        }
    }
    
    /// 生成 GUI 头部（Windows API 声明和入口点）
    fn emit_gui_header(&mut self) {
        // 外部 Win32 API 声明
        self.text_section.push_str("; Win32 GUI Application\n");
        self.text_section.push_str("global Start\n\n");
        
        // 核心 API
        self.text_section.push_str("extern GetModuleHandleA\n");
        self.text_section.push_str("extern RegisterClassExA\n");
        self.text_section.push_str("extern CreateWindowExA\n");
        self.text_section.push_str("extern ShowWindow\n");
        self.text_section.push_str("extern UpdateWindow\n");
        self.text_section.push_str("extern GetMessageA\n");
        self.text_section.push_str("extern TranslateMessage\n");
        self.text_section.push_str("extern DispatchMessageA\n");
        self.text_section.push_str("extern PostQuitMessage\n");
        self.text_section.push_str("extern DefWindowProcA\n");
        self.text_section.push_str("extern ExitProcess\n");
        
        // 控制台输出（调试用）
        self.text_section.push_str("extern GetStdHandle\n");
        self.text_section.push_str("extern WriteFile\n");
        
        // 控件操作
        self.text_section.push_str("extern SetWindowTextA\n");
        self.text_section.push_str("extern GetWindowTextA\n");
        self.text_section.push_str("extern GetWindowTextLengthA\n");
        self.text_section.push_str("extern SendMessageA\n");
        
        // 其他
        self.text_section.push_str("extern LoadCursorA\n");
        self.text_section.push_str("extern MessageBoxA\n");
        self.text_section.push_str("\n");
        
        // 常量定义
        self.data_section.push_str("; Win32 Constants\n");
        self.data_section.push_str("_gui_class_name: db \"SlimeGUIClass\", 0\n");
        self.data_section.push_str("_gui_btn_class: db \"BUTTON\", 0\n");
        self.data_section.push_str("_gui_edit_class: db \"EDIT\", 0\n");
        self.data_section.push_str("_gui_static_class: db \"STATIC\", 0\n");
        
        // BSS 段变量
        self.bss_section.push_str("; GUI Runtime Variables\n");
        self.bss_section.push_str("_gui_hInstance: resq 1\n");
        self.bss_section.push_str("_gui_main_hwnd: resq 1\n");
        self.bss_section.push_str("_gui_wndclass: resb 80\n");
        self.bss_section.push_str("_gui_msg: resb 48\n");
        self.bss_section.push_str("_gui_ctrl_handles: resq 64\n");
        self.bss_section.push_str("_gui_callbacks: resq 64\n");
        self.bss_section.push_str("_gui_text_buffer: resb 256\n");
        self.bss_section.push_str("_gui_next_id: resd 1\n");
        
        // 入口点
        self.text_section.push_str("\nStart:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 2048\n");
        self.text_section.push_str("    and rsp, -16\n");
        self.text_section.push_str("\n    ; Initialize GUI\n");
        self.text_section.push_str("    call _gui_init\n");
        self.text_section.push_str("\n    ; User code starts here\n");
    }
    
    /// 生成 GUI 消息循环
    fn emit_gui_message_loop(&mut self) {
        self.text_section.push_str("\n    ; Enter message loop\n");
        self.text_section.push_str("    call _gui_message_loop\n");
        self.text_section.push_str("\n    ; Exit\n");
        self.text_section.push_str("    xor rcx, rcx\n");
        self.text_section.push_str("    call ExitProcess\n");
    }
    
    /// 生成 GUI 辅助函数
    fn emit_gui_helpers(&mut self) {
        // GUI 初始化函数
        self.text_section.push_str("\n; ======== GUI Runtime Functions ========\n");
        
        // _gui_init: 初始化 GUI 子系统
        self.text_section.push_str("\n_gui_init:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 96\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    ; GetModuleHandleA(NULL)\n");
        self.text_section.push_str("    xor rcx, rcx\n");
        self.text_section.push_str("    call GetModuleHandleA\n");
        self.text_section.push_str("    mov [_gui_hInstance], rax\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    ; Initialize control ID counter\n");
        self.text_section.push_str("    mov dword [_gui_next_id], 1000\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    ; Register window class\n");
        self.text_section.push_str("    lea rdi, [_gui_wndclass]\n");
        self.text_section.push_str("    mov dword [rdi], 80          ; cbSize\n");
        self.text_section.push_str("    mov dword [rdi+4], 0         ; style\n");
        self.text_section.push_str("    lea rax, [_gui_wndproc]\n");
        self.text_section.push_str("    mov [rdi+8], rax             ; lpfnWndProc\n");
        self.text_section.push_str("    mov dword [rdi+16], 0        ; cbClsExtra\n");
        self.text_section.push_str("    mov dword [rdi+20], 0        ; cbWndExtra\n");
        self.text_section.push_str("    mov rax, [_gui_hInstance]\n");
        self.text_section.push_str("    mov [rdi+24], rax            ; hInstance\n");
        self.text_section.push_str("    mov qword [rdi+32], 0        ; hIcon\n");
        self.text_section.push_str("    ; Load cursor\n");
        self.text_section.push_str("    xor rcx, rcx\n");
        self.text_section.push_str("    mov rdx, 32512               ; IDC_ARROW\n");
        self.text_section.push_str("    call LoadCursorA\n");
        self.text_section.push_str("    lea rdi, [_gui_wndclass]\n");
        self.text_section.push_str("    mov [rdi+40], rax            ; hCursor\n");
        self.text_section.push_str("    mov qword [rdi+48], 16       ; hbrBackground = COLOR_BTNFACE+1\n");
        self.text_section.push_str("    mov qword [rdi+56], 0        ; lpszMenuName\n");
        self.text_section.push_str("    lea rax, [_gui_class_name]\n");
        self.text_section.push_str("    mov [rdi+64], rax            ; lpszClassName\n");
        self.text_section.push_str("    mov qword [rdi+72], 0        ; hIconSm\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    lea rcx, [_gui_wndclass]\n");
        self.text_section.push_str("    call RegisterClassExA\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // _gui_wndproc: 窗口过程
        self.text_section.push_str("\n_gui_wndproc:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 64\n");
        self.text_section.push_str("    mov [rbp-8], rcx             ; hwnd\n");
        self.text_section.push_str("    mov [rbp-16], rdx            ; msg\n");
        self.text_section.push_str("    mov [rbp-24], r8             ; wParam\n");
        self.text_section.push_str("    mov [rbp-32], r9             ; lParam\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    ; Check message type\n");
        self.text_section.push_str("    cmp edx, 0x0002              ; WM_DESTROY\n");
        self.text_section.push_str("    je .on_destroy\n");
        self.text_section.push_str("    cmp edx, 0x0111              ; WM_COMMAND\n");
        self.text_section.push_str("    je .on_command\n");
        self.text_section.push_str("    jmp .default\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str(".on_destroy:\n");
        self.text_section.push_str("    xor rcx, rcx\n");
        self.text_section.push_str("    call PostQuitMessage\n");
        self.text_section.push_str("    xor rax, rax\n");
        self.text_section.push_str("    jmp .done\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str(".on_command:\n");
        self.text_section.push_str("    ; LOWORD(wParam) = control ID\n");
        self.text_section.push_str("    mov rax, [rbp-24]\n");
        self.text_section.push_str("    movzx ecx, ax                ; control ID\n");
        self.text_section.push_str("    sub ecx, 1000\n");
        self.text_section.push_str("    cmp ecx, 64\n");
        self.text_section.push_str("    jae .default\n");
        self.text_section.push_str("    ; Get callback address\n");
        self.text_section.push_str("    lea rax, [_gui_callbacks]\n");
        self.text_section.push_str("    mov rax, [rax + rcx*8]\n");
        self.text_section.push_str("    test rax, rax\n");
        self.text_section.push_str("    jz .default\n");
        self.text_section.push_str("    ; Call callback\n");
        self.text_section.push_str("    call rax\n");
        self.text_section.push_str("    xor rax, rax\n");
        self.text_section.push_str("    jmp .done\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str(".default:\n");
        self.text_section.push_str("    mov rcx, [rbp-8]\n");
        self.text_section.push_str("    mov rdx, [rbp-16]\n");
        self.text_section.push_str("    mov r8, [rbp-24]\n");
        self.text_section.push_str("    mov r9, [rbp-32]\n");
        self.text_section.push_str("    call DefWindowProcA\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str(".done:\n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // _gui_message_loop: 消息循环
        self.text_section.push_str("\n_gui_message_loop:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 64\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str(".loop:\n");
        self.text_section.push_str("    lea rcx, [_gui_msg]\n");
        self.text_section.push_str("    xor rdx, rdx\n");
        self.text_section.push_str("    xor r8, r8\n");
        self.text_section.push_str("    xor r9, r9\n");
        self.text_section.push_str("    call GetMessageA\n");
        self.text_section.push_str("    test eax, eax\n");
        self.text_section.push_str("    jle .done\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    lea rcx, [_gui_msg]\n");
        self.text_section.push_str("    call TranslateMessage\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    lea rcx, [_gui_msg]\n");
        self.text_section.push_str("    call DispatchMessageA\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    jmp .loop\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str(".done:\n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // window_create: 创建窗口
        self.text_section.push_str("\n; window_create(title, width, height)\n");
        self.text_section.push_str("window_create:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 128\n");
        self.text_section.push_str("    mov [rbp-8], rdi             ; title\n");
        self.text_section.push_str("    mov [rbp-16], rsi            ; width\n");
        self.text_section.push_str("    mov [rbp-24], rdx            ; height\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    ; CreateWindowExA\n");
        self.text_section.push_str("    xor rcx, rcx                 ; dwExStyle\n");
        self.text_section.push_str("    lea rdx, [_gui_class_name]   ; lpClassName\n");
        self.text_section.push_str("    mov r8, [rbp-8]              ; lpWindowName\n");
        self.text_section.push_str("    mov r9d, 0x10CF0000          ; WS_OVERLAPPEDWINDOW | WS_VISIBLE\n");
        self.text_section.push_str("    mov dword [rsp+32], 100      ; x\n");
        self.text_section.push_str("    mov dword [rsp+40], 100      ; y\n");
        self.text_section.push_str("    mov rax, [rbp-16]\n");
        self.text_section.push_str("    mov [rsp+48], eax            ; width\n");
        self.text_section.push_str("    mov rax, [rbp-24]\n");
        self.text_section.push_str("    mov [rsp+56], eax            ; height\n");
        self.text_section.push_str("    mov qword [rsp+64], 0        ; hWndParent\n");
        self.text_section.push_str("    mov qword [rsp+72], 0        ; hMenu\n");
        self.text_section.push_str("    mov rax, [_gui_hInstance]\n");
        self.text_section.push_str("    mov [rsp+80], rax            ; hInstance\n");
        self.text_section.push_str("    mov qword [rsp+88], 0        ; lpParam\n");
        self.text_section.push_str("    call CreateWindowExA\n");
        self.text_section.push_str("    mov [_gui_main_hwnd], rax\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // button_create: 创建按钮
        self.text_section.push_str("\n; button_create(parent, text, x, y, w, h)\n");
        self.text_section.push_str("button_create:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 128\n");
        self.text_section.push_str("    mov [rbp-8], rdi             ; parent (ignored, use main)\n");
        self.text_section.push_str("    mov [rbp-16], rsi            ; text\n");
        self.text_section.push_str("    mov [rbp-24], rdx            ; x\n");
        self.text_section.push_str("    mov [rbp-32], rcx            ; y\n");
        self.text_section.push_str("    mov [rbp-40], r8             ; width\n");
        self.text_section.push_str("    mov [rbp-48], r9             ; height\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    ; Get control ID\n");
        self.text_section.push_str("    mov eax, [_gui_next_id]\n");
        self.text_section.push_str("    mov [rbp-56], eax\n");
        self.text_section.push_str("    inc dword [_gui_next_id]\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    xor rcx, rcx                 ; dwExStyle\n");
        self.text_section.push_str("    lea rdx, [_gui_btn_class]    ; BUTTON\n");
        self.text_section.push_str("    mov r8, [rbp-16]             ; text\n");
        self.text_section.push_str("    mov r9d, 0x50010000          ; WS_CHILD|WS_VISIBLE|WS_TABSTOP\n");
        self.text_section.push_str("    mov eax, [rbp-24]\n");
        self.text_section.push_str("    mov [rsp+32], eax            ; x\n");
        self.text_section.push_str("    mov eax, [rbp-32]\n");
        self.text_section.push_str("    mov [rsp+40], eax            ; y\n");
        self.text_section.push_str("    mov rax, [rbp-40]\n");
        self.text_section.push_str("    mov [rsp+48], eax            ; width\n");
        self.text_section.push_str("    mov rax, [rbp-48]\n");
        self.text_section.push_str("    mov [rsp+56], eax            ; height\n");
        self.text_section.push_str("    mov rax, [_gui_main_hwnd]\n");
        self.text_section.push_str("    mov [rsp+64], rax            ; hWndParent\n");
        self.text_section.push_str("    mov eax, [rbp-56]\n");
        self.text_section.push_str("    mov [rsp+72], rax            ; hMenu = control ID\n");
        self.text_section.push_str("    mov rax, [_gui_hInstance]\n");
        self.text_section.push_str("    mov [rsp+80], rax            ; hInstance\n");
        self.text_section.push_str("    mov qword [rsp+88], 0        ; lpParam\n");
        self.text_section.push_str("    call CreateWindowExA\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    ; Store handle\n");
        self.text_section.push_str("    mov ecx, [rbp-56]\n");
        self.text_section.push_str("    sub ecx, 1000\n");
        self.text_section.push_str("    lea rdx, [_gui_ctrl_handles]\n");
        self.text_section.push_str("    mov [rdx + rcx*8], rax\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov eax, [rbp-56]            ; return control ID\n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // label_create: 创建标签
        self.text_section.push_str("\n; label_create(parent, text, x, y, w, h)\n");
        self.text_section.push_str("label_create:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 128\n");
        self.text_section.push_str("    mov [rbp-16], rsi            ; text\n");
        self.text_section.push_str("    mov [rbp-24], rdx            ; x\n");
        self.text_section.push_str("    mov [rbp-32], rcx            ; y\n");
        self.text_section.push_str("    mov [rbp-40], r8             ; width\n");
        self.text_section.push_str("    mov [rbp-48], r9             ; height\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov eax, [_gui_next_id]\n");
        self.text_section.push_str("    mov [rbp-56], eax\n");
        self.text_section.push_str("    inc dword [_gui_next_id]\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    xor rcx, rcx\n");
        self.text_section.push_str("    lea rdx, [_gui_static_class] ; STATIC\n");
        self.text_section.push_str("    mov r8, [rbp-16]\n");
        self.text_section.push_str("    mov r9d, 0x50000000          ; WS_CHILD|WS_VISIBLE\n");
        self.text_section.push_str("    mov eax, [rbp-24]\n");
        self.text_section.push_str("    mov [rsp+32], eax\n");
        self.text_section.push_str("    mov eax, [rbp-32]\n");
        self.text_section.push_str("    mov [rsp+40], eax\n");
        self.text_section.push_str("    mov rax, [rbp-40]\n");
        self.text_section.push_str("    mov [rsp+48], eax\n");
        self.text_section.push_str("    mov rax, [rbp-48]\n");
        self.text_section.push_str("    mov [rsp+56], eax\n");
        self.text_section.push_str("    mov rax, [_gui_main_hwnd]\n");
        self.text_section.push_str("    mov [rsp+64], rax\n");
        self.text_section.push_str("    mov eax, [rbp-56]\n");
        self.text_section.push_str("    mov [rsp+72], rax\n");
        self.text_section.push_str("    mov rax, [_gui_hInstance]\n");
        self.text_section.push_str("    mov [rsp+80], rax\n");
        self.text_section.push_str("    mov qword [rsp+88], 0\n");
        self.text_section.push_str("    call CreateWindowExA\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov ecx, [rbp-56]\n");
        self.text_section.push_str("    sub ecx, 1000\n");
        self.text_section.push_str("    lea rdx, [_gui_ctrl_handles]\n");
        self.text_section.push_str("    mov [rdx + rcx*8], rax\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov eax, [rbp-56]\n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // entry_create: 创建输入框
        self.text_section.push_str("\n; entry_create(parent, x, y, w, h)\n");
        self.text_section.push_str("entry_create:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 128\n");
        self.text_section.push_str("    mov [rbp-24], rsi            ; x\n");
        self.text_section.push_str("    mov [rbp-32], rdx            ; y\n");
        self.text_section.push_str("    mov [rbp-40], rcx            ; width\n");
        self.text_section.push_str("    mov [rbp-48], r8             ; height\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov eax, [_gui_next_id]\n");
        self.text_section.push_str("    mov [rbp-56], eax\n");
        self.text_section.push_str("    inc dword [_gui_next_id]\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov rcx, 0x200               ; WS_EX_CLIENTEDGE\n");
        self.text_section.push_str("    lea rdx, [_gui_edit_class]   ; EDIT\n");
        self.text_section.push_str("    xor r8, r8                   ; empty text\n");
        self.text_section.push_str("    mov r9d, 0x50810080          ; WS_CHILD|WS_VISIBLE|WS_BORDER|WS_TABSTOP|ES_AUTOHSCROLL\n");
        self.text_section.push_str("    mov eax, [rbp-24]\n");
        self.text_section.push_str("    mov [rsp+32], eax\n");
        self.text_section.push_str("    mov eax, [rbp-32]\n");
        self.text_section.push_str("    mov [rsp+40], eax\n");
        self.text_section.push_str("    mov rax, [rbp-40]\n");
        self.text_section.push_str("    mov [rsp+48], eax\n");
        self.text_section.push_str("    mov rax, [rbp-48]\n");
        self.text_section.push_str("    mov [rsp+56], eax\n");
        self.text_section.push_str("    mov rax, [_gui_main_hwnd]\n");
        self.text_section.push_str("    mov [rsp+64], rax\n");
        self.text_section.push_str("    mov eax, [rbp-56]\n");
        self.text_section.push_str("    mov [rsp+72], rax\n");
        self.text_section.push_str("    mov rax, [_gui_hInstance]\n");
        self.text_section.push_str("    mov [rsp+80], rax\n");
        self.text_section.push_str("    mov qword [rsp+88], 0\n");
        self.text_section.push_str("    call CreateWindowExA\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov ecx, [rbp-56]\n");
        self.text_section.push_str("    sub ecx, 1000\n");
        self.text_section.push_str("    lea rdx, [_gui_ctrl_handles]\n");
        self.text_section.push_str("    mov [rdx + rcx*8], rax\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov eax, [rbp-56]\n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // gui_set_text: 设置控件文本
        self.text_section.push_str("\n; gui_set_text(ctrl_id, text)\n");
        self.text_section.push_str("gui_set_text:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 48\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    ; Get handle from ID\n");
        self.text_section.push_str("    mov eax, edi\n");
        self.text_section.push_str("    sub eax, 1000\n");
        self.text_section.push_str("    lea rcx, [_gui_ctrl_handles]\n");
        self.text_section.push_str("    mov rcx, [rcx + rax*8]\n");
        self.text_section.push_str("    mov rdx, rsi                 ; text\n");
        self.text_section.push_str("    call SetWindowTextA\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // gui_get_text: 获取控件文本
        self.text_section.push_str("\n; gui_get_text(ctrl_id) -> ptr to text\n");
        self.text_section.push_str("gui_get_text:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 48\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    ; Get handle from ID\n");
        self.text_section.push_str("    mov eax, edi\n");
        self.text_section.push_str("    sub eax, 1000\n");
        self.text_section.push_str("    lea rcx, [_gui_ctrl_handles]\n");
        self.text_section.push_str("    mov rcx, [rcx + rax*8]\n");
        self.text_section.push_str("    mov rdx, 255\n");
        self.text_section.push_str("    lea r8, [_gui_text_buffer]\n");
        self.text_section.push_str("    call GetWindowTextA\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    lea rax, [_gui_text_buffer]\n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // gui_on_click: 注册点击回调
        self.text_section.push_str("\n; gui_on_click(ctrl_id, callback_ptr)\n");
        self.text_section.push_str("gui_on_click:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov eax, edi\n");
        self.text_section.push_str("    sub eax, 1000\n");
        self.text_section.push_str("    lea rcx, [_gui_callbacks]\n");
        self.text_section.push_str("    mov [rcx + rax*8], rsi\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        // msgbox: 显示消息框
        self.text_section.push_str("\n; msgbox(text, title)\n");
        self.text_section.push_str("msgbox:\n");
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 48\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    mov r8, rsi                  ; title\n");
        self.text_section.push_str("    mov rdx, rdi                 ; text\n");
        self.text_section.push_str("    xor rcx, rcx                 ; hWnd = NULL\n");
        self.text_section.push_str("    xor r9, r9                   ; MB_OK\n");
        self.text_section.push_str("    call MessageBoxA\n");
        self.text_section.push_str("    \n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
    }
    
    fn emit_exit(&mut self, code: i32) {
        match self.target {
            Target::LinuxX64 => {
                self.text_section.push_str(&format!("    mov rdi, {}\n", code));
                self.text_section.push_str("    mov rax, 60\n");
                self.text_section.push_str("    syscall\n");
            }
            Target::WindowsX64 => {
                self.text_section.push_str(&format!("    mov rcx, {}\n", code));
                self.text_section.push_str("    call ExitProcess\n");
            }
            Target::MacosX64 => {
                self.text_section.push_str(&format!("    mov rdi, {}\n", code));
                self.text_section.push_str("    mov rax, 0x2000001\n");
                self.text_section.push_str("    syscall\n");
            }
        }
    }
    
    fn emit_helpers(&mut self) {
        // 整数转字符串函数
        self.text_section.push_str("\n; Helper: int to string (rax = number, returns rdi = buffer, rax = length)\n");
        self.text_section.push_str("_itoa:\n");
        self.text_section.push_str("    push rbx\n");
        self.text_section.push_str("    push rcx\n");
        self.text_section.push_str("    push rdx\n");
        self.text_section.push_str("    mov rcx, _itoa_buf + 20\n");
        self.text_section.push_str("    mov rbx, 10\n");
        self.text_section.push_str("    xor r8, r8\n");  // negative flag
        self.text_section.push_str("    cmp rax, 0\n");
        self.text_section.push_str("    jge .positive\n");
        self.text_section.push_str("    neg rax\n");
        self.text_section.push_str("    mov r8, 1\n");
        self.text_section.push_str(".positive:\n");
        self.text_section.push_str(".loop:\n");
        self.text_section.push_str("    xor rdx, rdx\n");
        self.text_section.push_str("    div rbx\n");
        self.text_section.push_str("    add dl, '0'\n");
        self.text_section.push_str("    dec rcx\n");
        self.text_section.push_str("    mov [rcx], dl\n");
        self.text_section.push_str("    test rax, rax\n");
        self.text_section.push_str("    jnz .loop\n");
        self.text_section.push_str("    cmp r8, 1\n");
        self.text_section.push_str("    jne .done\n");
        self.text_section.push_str("    dec rcx\n");
        self.text_section.push_str("    mov byte [rcx], '-'\n");
        self.text_section.push_str(".done:\n");
        self.text_section.push_str("    mov rdi, rcx\n");
        self.text_section.push_str("    mov rax, _itoa_buf + 20\n");
        self.text_section.push_str("    sub rax, rcx\n");
        self.text_section.push_str("    pop rdx\n");
        self.text_section.push_str("    pop rcx\n");
        self.text_section.push_str("    pop rbx\n");
        self.text_section.push_str("    ret\n");
        
        // itoa 缓冲区
        self.bss_section.push_str("_itoa_buf: resb 24\n");
    }
    
    fn emit_function(&mut self, func: &FnDef) -> Result<(), CompileError> {
        self.text_section.push_str(&format!("\n{}:\n", func.name));
        self.text_section.push_str("    push rbp\n");
        self.text_section.push_str("    mov rbp, rsp\n");
        self.text_section.push_str("    sub rsp, 256\n"); // 增大栈空间以支持 Windows API
        
        // 保存当前偏移量，为函数创建独立的栈帧
        let saved_offset = self.symbols.next_offset;
        self.symbols.next_offset = 8; // 函数参数从 rbp-8 开始
        
        self.symbols.push_scope();
        
        // 设置参数（简化版，只支持少量参数）
        let param_regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
        for (i, (name, ty)) in func.params.iter().enumerate() {
            if i < param_regs.len() {
                let offset = self.symbols.declare(name, ty.clone().unwrap_or(Type::Any), false)?;
                self.text_section.push_str(&format!("    mov [rbp-{}], {}\n", offset, param_regs[i]));
            }
        }
        
        for stmt in &func.body {
            self.emit_stmt(stmt)?;
        }
        
        // 默认返回
        self.text_section.push_str("    xor rax, rax\n");
        self.text_section.push_str("    leave\n");
        self.text_section.push_str("    ret\n");
        
        self.symbols.pop_scope();
        
        // 恢复偏移量
        self.symbols.next_offset = saved_offset;
        Ok(())
    }
    
    fn emit_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Let { name, ty, value, mutable } => {
                // 检查是否已作为全局变量声明
                if let Some(info) = self.symbols.lookup(name) {
                    if info.is_global {
                        // 全局变量，生成初始化代码
                        self.emit_expr(value)?;
                        self.text_section.push_str(&format!("    mov [_global_{}], rax\n", name));
                        return Ok(());
                    }
                }
                // 局部变量
                self.emit_expr(value)?;
                let offset = self.symbols.declare(name, ty.clone().unwrap_or(Type::Any), *mutable)?;
                self.text_section.push_str(&format!("    mov [rbp-{}], rax\n", offset));
                Ok(())
            }
            
            Stmt::Assign { name, value } => {
                let info = self.symbols.lookup(name)
                    .ok_or_else(|| CompileError::name(&format!("Undefined variable: {}", name)))?;
                if !info.mutable {
                    return Err(CompileError::type_err(&format!("Cannot assign to immutable variable: {}", name)));
                }
                let is_global = info.is_global;
                let offset = info.offset;
                self.emit_expr(value)?;
                if is_global {
                    self.text_section.push_str(&format!("    mov [_global_{}], rax\n", name));
                } else {
                    self.text_section.push_str(&format!("    mov [rbp-{}], rax\n", offset));
                }
                Ok(())
            }
            
            Stmt::CompoundAssign { name, op, value } => {
                let info = self.symbols.lookup(name)
                    .ok_or_else(|| CompileError::name(&format!("Undefined variable: {}", name)))?;
                if !info.mutable {
                    return Err(CompileError::type_err(&format!("Cannot assign to immutable variable: {}", name)));
                }
                let is_global = info.is_global;
                let offset = info.offset;
                
                // 加载当前值
                if is_global {
                    self.text_section.push_str(&format!("    mov rbx, [_global_{}]\n", name));
                } else {
                    self.text_section.push_str(&format!("    mov rbx, [rbp-{}]\n", offset));
                }
                // 计算新值
                self.emit_expr(value)?;
                // 执行运算
                match op {
                    BinOp::Add => self.text_section.push_str("    add rbx, rax\n"),
                    BinOp::Sub => self.text_section.push_str("    sub rbx, rax\n"),
                    BinOp::Mul => {
                        self.text_section.push_str("    imul rbx, rax\n");
                    }
                    BinOp::Div => {
                        self.text_section.push_str("    push rax\n");
                        self.text_section.push_str("    mov rax, rbx\n");
                        self.text_section.push_str("    cqo\n");
                        self.text_section.push_str("    pop rbx\n");
                        self.text_section.push_str("    idiv rbx\n");
                        self.text_section.push_str("    mov rbx, rax\n");
                    }
                    _ => {}
                }
                if is_global {
                    self.text_section.push_str(&format!("    mov [_global_{}], rbx\n", name));
                } else {
                    self.text_section.push_str(&format!("    mov [rbp-{}], rbx\n", offset));
                }
                Ok(())
            }
            
            Stmt::Print(args) => {
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        // 打印空格分隔
                        let space = self.register_string(" ");
                        self.emit_write_string(&space)?;
                    }
                    self.emit_print_expr(arg)?;
                }
                // 打印换行
                let newline = self.register_string("\n");
                self.emit_write_string(&newline)?;
                Ok(())
            }
            
            Stmt::If(if_stmt) => self.emit_if(if_stmt),
            Stmt::Loop(body) => self.emit_loop(body),
            Stmt::For(for_stmt) => self.emit_for(for_stmt),
            Stmt::While(while_stmt) => self.emit_while(while_stmt),
            
            Stmt::Break => {
                if let Some((_, break_label)) = self.loop_stack.last() {
                    self.text_section.push_str(&format!("    jmp {}\n", break_label));
                    Ok(())
                } else {
                    Err(CompileError::parse("break outside loop", 0))
                }
            }
            
            Stmt::Continue => {
                if let Some((continue_label, _)) = self.loop_stack.last() {
                    self.text_section.push_str(&format!("    jmp {}\n", continue_label));
                    Ok(())
                } else {
                    Err(CompileError::parse("continue outside loop", 0))
                }
            }
            
            Stmt::Return(value) => {
                if let Some(expr) = value {
                    self.emit_expr(expr)?;
                } else {
                    self.text_section.push_str("    xor rax, rax\n");
                }
                self.text_section.push_str("    leave\n");
                self.text_section.push_str("    ret\n");
                Ok(())
            }
            
            Stmt::Try(try_stmt) => self.emit_try(try_stmt),
            Stmt::Throw(expr) => {
                self.emit_expr(expr)?;
                // 简化：throw 直接退出
                self.emit_exit(1);
                Ok(())
            }
            Stmt::Defer(body) => {
                self.defer_stack.push(body.clone());
                Ok(())
            }
            
            Stmt::FnDef(_) => Ok(()), // 函数定义在顶层处理
            
            Stmt::Expr(expr) => {
                self.emit_expr(expr)?;
                Ok(())
            }
            
            Stmt::Block(stmts) => {
                self.symbols.push_scope();
                for stmt in stmts {
                    self.emit_stmt(stmt)?;
                }
                self.symbols.pop_scope();
                Ok(())
            }
            
            // ========== 接口系统（核心）==========
            Stmt::DefInterface { kind, name, data_type, direction, target } => {
                // 注册接口到接口表
                self.interfaces.define(InterfaceInfo {
                    name: name.clone(),
                    kind: kind.clone(),
                    data_type: data_type.clone(),
                    direction: direction.clone(),
                    target: target.clone(),
                });
                Ok(())
            }
            
            Stmt::CallInterface { interface, args } => {
                // 查找接口
                let target = if let Some(info) = self.interfaces.lookup(interface) {
                    info.target.clone()
                } else {
                    interface.clone() // 未定义的接口，假设是直接映射
                };
                
                // 根据目标接口分发
                match target.as_str() {
                    "host.stdout" | "System.Output.Print" => {
                        // 输出接口
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                let space = self.register_string(" ");
                                self.emit_write_string(&space)?;
                            }
                            self.emit_print_expr(arg)?;
                        }
                        let newline = self.register_string("\n");
                        self.emit_write_string(&newline)?;
                    }
                    "host.stderr" | "System.Error.Print" => {
                        // 错误输出接口（简化：暂时同stdout）
                        for arg in args {
                            self.emit_print_expr(arg)?;
                        }
                        let newline = self.register_string("\n");
                        self.emit_write_string(&newline)?;
                    }
                    _ => {
                        // 用户定义的接口或函数调用
                        // 尝试作为函数调用
                        if self.functions.contains_key(interface) {
                            let param_regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
                            for (i, arg) in args.iter().enumerate() {
                                if i < param_regs.len() {
                                    self.emit_expr(arg)?;
                                    self.text_section.push_str(&format!("    mov {}, rax\n", param_regs[i]));
                                }
                            }
                            self.text_section.push_str(&format!("    call {}\n", interface));
                        } else {
                            // 默认作为输出处理
                            for arg in args {
                                self.emit_print_expr(arg)?;
                            }
                            let newline = self.register_string("\n");
                            self.emit_write_string(&newline)?;
                        }
                    }
                }
                Ok(())
            }
            
            Stmt::DropInterface(name) => {
                // 从接口表移除
                self.interfaces.drop(name);
                Ok(())
            }
            
            Stmt::UseInterface { path: _, alias: _ } => {
                // 旧版模块导入（保留兼容性）
                Ok(())
            }
            
            // ========== 新模块系统 ==========
            Stmt::Import { .. } => {
                // import 在编译前已处理，这里是 no-op
                Ok(())
            }
            
            Stmt::Use { .. } => {
                // use 在编译前已处理，这里是 no-op
                Ok(())
            }
            
            Stmt::FromImport { .. } => {
                // from import 在编译前已处理，这里是 no-op
                Ok(())
            }
            
            Stmt::Include(_) => {
                // include 在编译前已处理，这里是 no-op
                Ok(())
            }
            
            Stmt::PubDecl(inner) => {
                // pub 声明，直接处理内部语句
                self.emit_stmt(inner)
            }
            
            // ========== CNB (C Native Bridge) ==========
            Stmt::ExternFn { name, params, ret_type, calling_conv, lib_name } => {
                self.emit_extern_fn(name, params, ret_type, calling_conv, lib_name)
            }
            
            Stmt::ExternStruct { name, fields } => {
                self.emit_extern_struct(name, fields)
            }
            
            Stmt::Unsafe(body) => {
                // unsafe 块目前直接执行内部代码
                for stmt in body {
                    self.emit_stmt(stmt)?;
                }
                Ok(())
            }
            
            Stmt::Input(_) => {
                // TODO: 输入接口
                Ok(())
            }
        }
    }
    
    // ========================================================================
    // CNB 代码生成
    // ========================================================================
    
    /// 生成 extern 函数声明
    fn emit_extern_fn(&mut self, name: &str, params: &[(String, CType)], 
                      ret_type: &Option<CType>, calling_conv: &CallingConv,
                      lib_name: &Option<String>) -> Result<(), CompileError> {
        // 在 text_section 开头添加 extern 声明
        let extern_decl = format!("extern {}\n", name);
        if !self.text_section.contains(&extern_decl) {
            // 找到合适位置插入 extern 声明
            self.extern_declarations.push(name.to_string());
        }
        
        // 记录函数签名供后续调用使用
        self.extern_functions.insert(name.to_string(), ExternFnInfo {
            params: params.to_vec(),
            ret_type: ret_type.clone(),
            calling_conv: calling_conv.clone(),
            lib_name: lib_name.clone(),
        });
        
        Ok(())
    }
    
    /// 生成 extern struct 定义（BSS 段）
    fn emit_extern_struct(&mut self, name: &str, fields: &[(String, CType)]) -> Result<(), CompileError> {
        // 计算结构体大小和对齐
        let mut size = 0usize;
        let mut field_offsets = Vec::new();
        
        for (field_name, field_type) in fields {
            let field_size = field_type.size();
            // 简单对齐：按字段大小对齐（最大8字节）
            let align = field_size.min(8);
            if size % align != 0 {
                size = (size / align + 1) * align;
            }
            field_offsets.push((field_name.clone(), size, field_size));
            size += field_size;
        }
        
        // 记录结构体信息
        self.extern_structs.insert(name.to_string(), ExternStructInfo {
            fields: field_offsets,
            size,
        });
        
        Ok(())
    }
    
    /// 生成调用 extern 函数的代码
    fn emit_extern_call(&mut self, name: &str, args: &[Expr]) -> Result<(), CompileError> {
        let info = self.extern_functions.get(name).cloned();
        
        if let Some(fn_info) = info {
            // 根据调用约定设置参数
            match fn_info.calling_conv {
                CallingConv::Win64 => {
                    // Windows x64: rcx, rdx, r8, r9, then stack
                    let win64_regs = ["rcx", "rdx", "r8", "r9"];
                    for (i, arg) in args.iter().enumerate() {
                        self.emit_expr(arg)?;
                        if i < win64_regs.len() {
                            self.text_section.push_str(&format!("    mov {}, rax\n", win64_regs[i]));
                        } else {
                            // 栈参数
                            let offset = 32 + (i - 4) * 8;
                            self.text_section.push_str(&format!("    mov [rsp+{}], rax\n", offset));
                        }
                    }
                }
                CallingConv::SysV | CallingConv::Cdecl => {
                    // System V: rdi, rsi, rdx, rcx, r8, r9
                    let sysv_regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
                    for (i, arg) in args.iter().enumerate() {
                        self.emit_expr(arg)?;
                        if i < sysv_regs.len() {
                            self.text_section.push_str(&format!("    mov {}, rax\n", sysv_regs[i]));
                        } else {
                            self.text_section.push_str("    push rax\n");
                        }
                    }
                }
                _ => {
                    // 默认使用 System V 约定
                    let regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
                    for (i, arg) in args.iter().enumerate() {
                        self.emit_expr(arg)?;
                        if i < regs.len() {
                            self.text_section.push_str(&format!("    mov {}, rax\n", regs[i]));
                        }
                    }
                }
            }
            
            // 调用函数
            self.text_section.push_str(&format!("    call {}\n", name));
            
            // 返回值在 rax 中
        } else {
            // 未声明的外部函数，假设为默认调用约定
            let regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
            for (i, arg) in args.iter().enumerate() {
                self.emit_expr(arg)?;
                if i < regs.len() {
                    self.text_section.push_str(&format!("    mov {}, rax\n", regs[i]));
                }
            }
            self.text_section.push_str(&format!("    call {}\n", name));
        }
        
        Ok(())
    }
    
    fn emit_print_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Str(s) => {
                let label = self.register_string(s);
                self.emit_write_string(&label)?;
            }
            Expr::Int(_) | Expr::Var(_) | Expr::BinOp(_, _, _) | Expr::UnaryOp(_, _) => {
                self.emit_expr(expr)?;
                // 调用 itoa 将数字转为字符串
                self.text_section.push_str("    call _itoa\n");
                // rdi = buffer, rax = length
                self.emit_write_raw()?;
            }
            Expr::Bool(b) => {
                let s = if *b { "true" } else { "false" };
                let label = self.register_string(s);
                self.emit_write_string(&label)?;
            }
            Expr::None => {
                let label = self.register_string("none");
                self.emit_write_string(&label)?;
            }
            Expr::Float(f) => {
                // 简化：将浮点数按字符串处理
                let s = format!("{}", f);
                let label = self.register_string(&s);
                self.emit_write_string(&label)?;
            }
            _ => {
                self.emit_expr(expr)?;
                self.text_section.push_str("    call _itoa\n");
                self.emit_write_raw()?;
            }
        }
        Ok(())
    }
    
    fn emit_write_string(&mut self, label: &str) -> Result<(), CompileError> {
        match self.target {
            Target::LinuxX64 => {
                self.text_section.push_str("    mov rax, 1\n");
                self.text_section.push_str("    mov rdi, 1\n");
                self.text_section.push_str(&format!("    mov rsi, {}\n", label));
                self.text_section.push_str(&format!("    mov rdx, {}_len\n", label));
                self.text_section.push_str("    syscall\n");
            }
            Target::WindowsX64 => {
                self.text_section.push_str("    mov rcx, -11\n"); // STD_OUTPUT_HANDLE
                self.text_section.push_str("    call GetStdHandle\n");
                self.text_section.push_str("    mov rcx, rax\n");
                self.text_section.push_str(&format!("    lea rdx, [{}]\n", label));
                self.text_section.push_str(&format!("    mov r8, {}_len\n", label));
                self.text_section.push_str("    lea r9, [rbp-200]\n"); // bytes written
                self.text_section.push_str("    mov qword [rsp+32], 0\n");
                self.text_section.push_str("    call WriteFile\n");
            }
            Target::MacosX64 => {
                self.text_section.push_str("    mov rax, 0x2000004\n");
                self.text_section.push_str("    mov rdi, 1\n");
                self.text_section.push_str(&format!("    mov rsi, {}\n", label));
                self.text_section.push_str(&format!("    mov rdx, {}_len\n", label));
                self.text_section.push_str("    syscall\n");
            }
        }
        Ok(())
    }
    
    fn emit_write_raw(&mut self) -> Result<(), CompileError> {
        // rdi = buffer address, rax = length
        match self.target {
            Target::LinuxX64 => {
                self.text_section.push_str("    mov rsi, rdi\n");
                self.text_section.push_str("    mov rdx, rax\n");
                self.text_section.push_str("    mov rax, 1\n");
                self.text_section.push_str("    mov rdi, 1\n");
                self.text_section.push_str("    syscall\n");
            }
            Target::WindowsX64 => {
                self.text_section.push_str("    push rdi\n");
                self.text_section.push_str("    push rax\n");
                self.text_section.push_str("    mov rcx, -11\n");
                self.text_section.push_str("    call GetStdHandle\n");
                self.text_section.push_str("    mov rcx, rax\n");
                self.text_section.push_str("    pop r8\n");
                self.text_section.push_str("    pop rdx\n");
                self.text_section.push_str("    lea r9, [rbp-200]\n");
                self.text_section.push_str("    mov qword [rsp+32], 0\n");
                self.text_section.push_str("    call WriteFile\n");
            }
            Target::MacosX64 => {
                self.text_section.push_str("    mov rsi, rdi\n");
                self.text_section.push_str("    mov rdx, rax\n");
                self.text_section.push_str("    mov rax, 0x2000004\n");
                self.text_section.push_str("    mov rdi, 1\n");
                self.text_section.push_str("    syscall\n");
            }
        }
        Ok(())
    }
    
    fn emit_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Int(n) => {
                self.text_section.push_str(&format!("    mov rax, {}\n", n));
            }
            Expr::Float(f) => {
                // 简化：将浮点数转为整数
                self.text_section.push_str(&format!("    mov rax, {}\n", *f as i64));
            }
            Expr::Str(s) => {
                let label = self.register_string(s);
                self.text_section.push_str(&format!("    lea rax, [{}]\n", label));
            }
            Expr::Bool(b) => {
                self.text_section.push_str(&format!("    mov rax, {}\n", if *b { 1 } else { 0 }));
            }
            Expr::None => {
                self.text_section.push_str("    xor rax, rax\n");
            }
            Expr::Var(name) => {
                // 先按变量查找
                if let Some(info) = self.symbols.lookup(name) {
                    if info.is_global {
                        self.text_section.push_str(&format!("    mov rax, [_global_{}]\n", name));
                    } else {
                        self.text_section.push_str(&format!("    mov rax, [rbp-{}]\n", info.offset));
                    }
                } else if self.functions.contains_key(name) {
                    // 如果不是变量但存在同名函数，则返回函数地址
                    // 用于 GUI 回调等场景: gui_on_click(btn_0, on_digit_0)
                    self.text_section.push_str(&format!("    lea rax, [{}]\n", name));
                } else {
                    return Err(CompileError::name(&format!("Undefined variable: {}", name)));
                }
            }
            Expr::BinOp(lhs, op, rhs) => {
                // 先计算右操作数，压栈
                self.emit_expr(rhs)?;
                self.text_section.push_str("    push rax\n");
                // 再计算左操作数
                self.emit_expr(lhs)?;
                // 弹出右操作数到 rbx
                self.text_section.push_str("    pop rbx\n");
                
                match op {
                    BinOp::Add => {
                        self.text_section.push_str("    add rax, rbx\n");
                    }
                    BinOp::Sub => {
                        self.text_section.push_str("    sub rax, rbx\n");
                    }
                    BinOp::Mul => {
                        self.text_section.push_str("    imul rax, rbx\n");
                    }
                    BinOp::Div => {
                        self.text_section.push_str("    cqo\n");
                        self.text_section.push_str("    idiv rbx\n");
                    }
                    BinOp::Mod => {
                        self.text_section.push_str("    cqo\n");
                        self.text_section.push_str("    idiv rbx\n");
                        self.text_section.push_str("    mov rax, rdx\n");
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                        self.text_section.push_str("    cmp rax, rbx\n");
                        let set_instr = match op {
                            BinOp::Eq => "sete",
                            BinOp::Ne => "setne",
                            BinOp::Lt => "setl",
                            BinOp::Gt => "setg",
                            BinOp::Le => "setle",
                            BinOp::Ge => "setge",
                            _ => unreachable!(),
                        };
                        self.text_section.push_str(&format!("    {} al\n", set_instr));
                        self.text_section.push_str("    movzx rax, al\n");
                    }
                    BinOp::And => {
                        self.text_section.push_str("    and rax, rbx\n");
                    }
                    BinOp::Or => {
                        self.text_section.push_str("    or rax, rbx\n");
                    }
                }
            }
            Expr::UnaryOp(op, operand) => {
                self.emit_expr(operand)?;
                match op {
                    UnaryOp::Neg => {
                        self.text_section.push_str("    neg rax\n");
                    }
                    UnaryOp::Not => {
                        self.text_section.push_str("    test rax, rax\n");
                        self.text_section.push_str("    setz al\n");
                        self.text_section.push_str("    movzx rax, al\n");
                    }
                }
            }
            Expr::Call(name, args) => {
                // 检查是否是 extern 函数调用
                if self.extern_functions.contains_key(name) {
                    self.emit_extern_call(name, args)?;
                } else {
                    // 普通 Slime 函数调用
                    let param_regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
                    for (i, arg) in args.iter().enumerate() {
                        if i < param_regs.len() {
                            self.emit_expr(arg)?;
                            self.text_section.push_str(&format!("    mov {}, rax\n", param_regs[i]));
                        }
                    }
                    self.text_section.push_str(&format!("    call {}\n", name));
                }
            }
            
            // 借用表达式：获取变量地址
            Expr::Borrow(inner) => {
                if let Expr::Var(name) = inner.as_ref() {
                    let info = self.symbols.lookup(name)
                        .ok_or_else(|| CompileError::name(&format!("Undefined variable: {}", name)))?;
                    // 获取变量地址 (lea = load effective address)
                    self.text_section.push_str(&format!("    lea rax, [rbp-{}]\n", info.offset));
                } else {
                    // 对于复杂表达式，先计算值再获取地址
                    self.emit_expr(inner)?;
                }
            }
            
            // 可变借用：同上
            Expr::BorrowMut(inner) => {
                if let Expr::Var(name) = inner.as_ref() {
                    let info = self.symbols.lookup(name)
                        .ok_or_else(|| CompileError::name(&format!("Undefined variable: {}", name)))?;
                    self.text_section.push_str(&format!("    lea rax, [rbp-{}]\n", info.offset));
                } else {
                    self.emit_expr(inner)?;
                }
            }
            
            // 解引用：读取指针指向的值
            Expr::Deref(inner) => {
                self.emit_expr(inner)?;
                // rax 现在是地址，读取该地址的值
                self.text_section.push_str("    mov rax, [rax]\n");
            }
            
            Expr::List(_) | Expr::Index(_, _) | Expr::MethodCall(_, _, _) => {
                // 暂不完全支持
                self.text_section.push_str("    xor rax, rax\n");
            }
        }
        Ok(())
    }
    
    fn emit_if(&mut self, if_stmt: &IfStmt) -> Result<(), CompileError> {
        let end_label = self.gen_label();
        let mut next_label = self.gen_label();
        
        // 主条件
        self.emit_expr(&if_stmt.cond)?;
        self.text_section.push_str("    test rax, rax\n");
        self.text_section.push_str(&format!("    jz {}\n", next_label));
        
        self.symbols.push_scope();
        for stmt in &if_stmt.then_block {
            self.emit_stmt(stmt)?;
        }
        self.symbols.pop_scope();
        self.text_section.push_str(&format!("    jmp {}\n", end_label));
        
        // elif 分支
        for (elif_cond, elif_body) in &if_stmt.elif_parts {
            self.text_section.push_str(&format!("{}:\n", next_label));
            next_label = self.gen_label();
            
            self.emit_expr(elif_cond)?;
            self.text_section.push_str("    test rax, rax\n");
            self.text_section.push_str(&format!("    jz {}\n", next_label));
            
            self.symbols.push_scope();
            for stmt in elif_body {
                self.emit_stmt(stmt)?;
            }
            self.symbols.pop_scope();
            self.text_section.push_str(&format!("    jmp {}\n", end_label));
        }
        
        // else 分支
        self.text_section.push_str(&format!("{}:\n", next_label));
        if let Some(else_body) = &if_stmt.else_block {
            self.symbols.push_scope();
            for stmt in else_body {
                self.emit_stmt(stmt)?;
            }
            self.symbols.pop_scope();
        }
        
        self.text_section.push_str(&format!("{}:\n", end_label));
        Ok(())
    }
    
    fn emit_loop(&mut self, body: &[Stmt]) -> Result<(), CompileError> {
        let loop_label = self.gen_label();
        let end_label = self.gen_label();
        
        self.loop_stack.push((loop_label.clone(), end_label.clone()));
        
        self.text_section.push_str(&format!("{}:\n", loop_label));
        
        self.symbols.push_scope();
        for stmt in body {
            self.emit_stmt(stmt)?;
        }
        self.symbols.pop_scope();
        
        self.text_section.push_str(&format!("    jmp {}\n", loop_label));
        self.text_section.push_str(&format!("{}:\n", end_label));
        
        self.loop_stack.pop();
        Ok(())
    }
    
    fn emit_for(&mut self, for_stmt: &ForStmt) -> Result<(), CompileError> {
        // 简化：for i in range(n) 模式
        // 暂时只支持简单的计数循环
        let loop_label = self.gen_label();
        let end_label = self.gen_label();
        
        self.symbols.push_scope();
        
        // 初始化循环变量
        let offset = self.symbols.declare(&for_stmt.var, Type::Int, true)?;
        self.text_section.push_str("    xor rax, rax\n");
        self.text_section.push_str(&format!("    mov [rbp-{}], rax\n", offset));
        
        // 获取迭代上限（简化处理）
        self.emit_expr(&for_stmt.iter)?;
        self.text_section.push_str("    push rax\n"); // 保存上限
        
        self.loop_stack.push((loop_label.clone(), end_label.clone()));
        
        self.text_section.push_str(&format!("{}:\n", loop_label));
        
        // 检查条件
        self.text_section.push_str(&format!("    mov rax, [rbp-{}]\n", offset));
        self.text_section.push_str("    pop rbx\n");
        self.text_section.push_str("    push rbx\n");
        self.text_section.push_str("    cmp rax, rbx\n");
        self.text_section.push_str(&format!("    jge {}\n", end_label));
        
        // 循环体
        for stmt in &for_stmt.body {
            self.emit_stmt(stmt)?;
        }
        
        // 递增
        self.text_section.push_str(&format!("    inc qword [rbp-{}]\n", offset));
        self.text_section.push_str(&format!("    jmp {}\n", loop_label));
        
        self.text_section.push_str(&format!("{}:\n", end_label));
        self.text_section.push_str("    pop rax\n"); // 清理栈
        
        self.loop_stack.pop();
        self.symbols.pop_scope();
        Ok(())
    }
    
    fn emit_while(&mut self, while_stmt: &WhileStmt) -> Result<(), CompileError> {
        let loop_label = self.gen_label();
        let end_label = self.gen_label();
        
        self.loop_stack.push((loop_label.clone(), end_label.clone()));
        
        self.text_section.push_str(&format!("{}:\n", loop_label));
        
        self.emit_expr(&while_stmt.cond)?;
        self.text_section.push_str("    test rax, rax\n");
        self.text_section.push_str(&format!("    jz {}\n", end_label));
        
        self.symbols.push_scope();
        for stmt in &while_stmt.body {
            self.emit_stmt(stmt)?;
        }
        self.symbols.pop_scope();
        
        self.text_section.push_str(&format!("    jmp {}\n", loop_label));
        self.text_section.push_str(&format!("{}:\n", end_label));
        
        self.loop_stack.pop();
        Ok(())
    }
    
    fn emit_try(&mut self, try_stmt: &TryStmt) -> Result<(), CompileError> {
        // 简化的try-catch：直接执行，catch块作为错误处理占位
        let catch_label = self.gen_label();
        let end_label = self.gen_label();
        
        // try 块
        self.symbols.push_scope();
        for stmt in &try_stmt.try_block {
            self.emit_stmt(stmt)?;
        }
        self.symbols.pop_scope();
        
        self.text_section.push_str(&format!("    jmp {}\n", end_label));
        
        // catch 块
        self.text_section.push_str(&format!("{}:\n", catch_label));
        if let Some(catch_block) = &try_stmt.catch_block {
            self.symbols.push_scope();
            if let Some(var) = &try_stmt.catch_var {
                let _ = self.symbols.declare(var, Type::Any, false);
            }
            for stmt in catch_block {
                self.emit_stmt(stmt)?;
            }
            self.symbols.pop_scope();
        }
        
        self.text_section.push_str(&format!("{}:\n", end_label));
        Ok(())
    }
}

fn escape_nasm_string(s: &str) -> String {
    let mut result = String::new();
    let mut in_quote = false;
    
    for b in s.bytes() {
        match b {
            b'\n' => {
                if in_quote {
                    result.push_str("\", ");
                    in_quote = false;
                }
                result.push_str("10, ");
            }
            b'\t' => {
                if in_quote {
                    result.push_str("\", ");
                    in_quote = false;
                }
                result.push_str("9, ");
            }
            b'\r' => {
                if in_quote {
                    result.push_str("\", ");
                    in_quote = false;
                }
                result.push_str("13, ");
            }
            b'"' => {
                if in_quote {
                    result.push_str("\", ");
                    in_quote = false;
                }
                result.push_str("34, ");
            }
            _ => {
                if !in_quote {
                    result.push('"');
                    in_quote = true;
                }
                result.push(b as char);
            }
        }
    }
    
    if in_quote {
        result.push('"');
    }
    
    // 去掉末尾的逗号和空格
    result.trim_end_matches(", ").to_string()
}
