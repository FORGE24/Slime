# 语法树 (AST) 详解
## Abstract Syntax Tree in Slime Compiler

---

## 🌳 什么是语法树？

**语法树（AST - Abstract Syntax Tree）** 是编译器将源代码转换成的**树形数据结构**，代表了程序的语法结构。

### 为什么叫"抽象"语法树？
- **抽象**：去除了源代码中的语法细节（如括号、分号）
- **语法**：保留了程序的语法结构和语义
- **树**：用树形结构表示代码的层次关系

---

## 🔄 编译流程中的位置

```
源代码 (Source Code)
    ↓
词法分析 (Lexical Analysis) → Token流
    ↓
语法分析 (Syntax Analysis) → 🌳 AST
    ↓
语义分析 (Semantic Analysis) → 类型检查的AST
    ↓
优化 (ETCA Optimization) → 优化后的AST
    ↓
代码生成 (Code Generation) → 汇编代码
```

**AST是编译器的核心中间表示！**

---

## 🏗️ Slime 语法树结构

### 三大核心组件

#### 1. **Program** - 程序根节点
```rust
struct Program {
    stmts: Vec<Stmt>,    // 顶层语句列表
}
```

#### 2. **Stmt** - 语句节点
```rust
enum Stmt {
    // 变量声明
    Let { name: String, ty: Option<Type>, value: Expr, mutable: bool },
    
    // 赋值语句
    Assign { name: String, value: Expr },
    
    // 控制流
    If(Box<IfStmt>),
    While(Box<WhileStmt>),
    For(Box<ForStmt>),
    
    // 函数定义
    FnDef(Box<FnDef>),
    
    // 返回语句
    Return(Option<Expr>),
    
    // 接口定义
    DefInterface { kind: String, name: String, ... },
    
    // 其他...
}
```

#### 3. **Expr** - 表达式节点
```rust
enum Expr {
    // 字面量
    Int(i64),
    Str(String),
    Bool(bool),
    
    // 变量引用
    Var(String),
    
    // 二元运算
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    
    // 函数调用
    Call(String, Vec<Expr>),
    
    // 其他...
}
```

---

## 📝 实例：从代码到AST

### 源代码
```slime
fn add(x: int, y: int) -> int {
    var result = x + y
    return result
}

fn main() {
    var a = 10
    var b = 20
    var c = add(a, b)
    printint(c)
}
```

### 对应的AST（简化表示）
```
Program
├── Stmt::FnDef
│   ├── name: "add"
│   ├── params: [("x", int), ("y", int)]
│   ├── return_type: int
│   └── body: [
│       ├── Stmt::Let
│       │   ├── name: "result"
│       │   └── value: Expr::BinOp
│       │       ├── left: Expr::Var("x")
│       │       ├── op: BinOp::Add
│       │       └── right: Expr::Var("y")
│       └── Stmt::Return
│           └── Some(Expr::Var("result"))
│   ]
│
└── Stmt::FnDef
    ├── name: "main"
    ├── params: []
    └── body: [
        ├── Stmt::Let { name: "a", value: Expr::Int(10) }
        ├── Stmt::Let { name: "b", value: Expr::Int(20) }
        ├── Stmt::Let
        │   ├── name: "c"
        │   └── value: Expr::Call
        │       ├── function: "add"
        │       └── args: [Expr::Var("a"), Expr::Var("b")]
        └── Stmt::Expr
            └── Expr::Call
                ├── function: "printint"
                └── args: [Expr::Var("c")]
    ]
```

---

## 🎯 AST 的作用

### 1. **语法验证**
- 检查代码结构是否合法
- 确保括号匹配、语句完整

### 2. **语义分析**
- 类型检查：`var x: int = "hello"` ❌
- 作用域分析：变量是否已声明
- 所有权检查：资源管理

### 3. **优化基础**
ETCA的所有优化都在AST上进行：

```rust
// 常量折叠优化
Expr::BinOp(
    Box::new(Expr::Int(10)),
    BinOp::Add,
    Box::new(Expr::Int(20))
)
    ↓ 优化后
Expr::Int(30)
```

### 4. **代码生成**
- 遍历AST生成目标代码
- 每个节点对应特定的汇编指令

---

## 🔍 Slime AST 特色

### 1. **接口优先设计**
```rust
Stmt::DefInterface {
    kind: "static.interface",
    name: "Example.Print",
    data_type: "str",
    direction: "Out",
    target: "System.Output.Print",
}
```

### 2. **支持异步**
```rust
Expr::Await(Box<Expr>)
Expr::Spawn(Box<Expr>)
Expr::Join(Vec<Expr>)
```

### 3. **宏系统**
```rust
Expr::MacroInvoke(String, Vec<Expr>)
Expr::ComptimeExpr(Box<Expr>)
Expr::Quote(Box<Expr>)
```

---

## 🌲 AST 遍历示例

### 前序遍历（Pre-order）
```rust
fn visit_stmt(stmt: &Stmt) {
    match stmt {
        Stmt::Let { name, value, .. } => {
            println!("声明变量: {}", name);
            visit_expr(value);
        }
        Stmt::If(if_stmt) => {
            visit_expr(&if_stmt.cond);
            for s in &if_stmt.then_block {
                visit_stmt(s);
            }
        }
        // ... 其他模式
    }
}
```

### 后序遍历（Post-order）
用于**自底向上**的优化：
```rust
fn optimize_expr(expr: &mut Expr) {
    match expr {
        Expr::BinOp(left, op, right) => {
            // 先优化子表达式
            optimize_expr(left);
            optimize_expr(right);
            
            // 再优化当前节点（常量折叠）
            if let (Expr::Int(l), Expr::Int(r)) = (&**left, &**right) {
                if let BinOp::Add = op {
                    *expr = Expr::Int(l + r);
                }
            }
        }
        // ...
    }
}
```

---

## 🎨 可视化工具

### AST 打印示例
```rust
fn print_ast(program: &Program, indent: usize) {
    for stmt in &program.stmts {
        print_stmt(stmt, indent);
    }
}

fn print_stmt(stmt: &Stmt, indent: usize) {
    let spaces = "  ".repeat(indent);
    match stmt {
        Stmt::Let { name, value, .. } => {
            println!("{}Let: {}", spaces, name);
            print_expr(value, indent + 1);
        }
        Stmt::FnDef(fn_def) => {
            println!("{}Function: {}", spaces, fn_def.name);
            for s in &fn_def.body {
                print_stmt(s, indent + 1);
            }
        }
        // ...
    }
}
```

### 输出示例
```
Program
  Function: add
    Let: result
      BinOp: Add
        Var: x
        Var: y
    Return
      Var: result
  Function: main
    Let: a
      Int: 10
    Let: b
      Int: 20
    Let: c
      Call: add
        Var: a
        Var: b
```

---

## 🔬 AST 在 ETCA 中的角色

### 准备期优化
```rust
// 在AST上执行常量折叠
fn apply_constant_folding(program: &mut Program) {
    for stmt in &mut program.stmts {
        fold_stmt_constants(stmt);
    }
}
```

### 编译期代码生成
```rust
// 从AST生成汇编
fn emit_stmt(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Let { name, value, .. } => {
            let val_asm = emit_expr(value);
            format!("{}\n    mov [_global_{}], rax", val_asm, name)
        }
        // ...
    }
}
```

---

## 📊 AST vs 其他中间表示

| 特性 | AST | IR (LLVM) | 字节码 |
|------|-----|-----------|--------|
| **抽象级别** | 高 | 中 | 低 |
| **保留语法** | ✅ | ❌ | ❌ |
| **优化便利** | 中 | ✅ | ❌ |
| **代码生成** | 中 | ✅ | ✅ |
| **人类可读** | ✅ | 中 | ❌ |

**Slime选择AST**：
- ✅ 直接优化高层语义
- ✅ 保留完整程序结构
- ✅ 便于实现ETCA优化

---

## 🎓 总结

### AST 的本质
```
源代码 → 解析器 → AST → 优化器 → AST' → 代码生成器 → 汇编
         ^^^^^^       ^^^   ^^^^^^^^      ^^^^
         语法分析     树结构  ETCA优化    遍历树
```

### 关键概念
1. **树形结构**：表示代码的层次关系
2. **抽象表示**：去除语法噪音，保留语义
3. **优化基础**：所有优化都在AST上进行
4. **编译桥梁**：连接源码和目标代码

### Slime AST 特点
- 🌟 **接口优先**：万物皆接口的设计
- ⚡ **ETCA友好**：专为时间坍缩优化
- 🚀 **高效遍历**：支持多遍扫描优化
- 🎯 **类型丰富**：支持现代语言特性

**AST = 编译器的灵魂数据结构！**

---

*文档：Slime Compiler AST Guide*
*最后更新: 2026年2月2日*
*版本: Slime v0.2.0 with ETCA*
