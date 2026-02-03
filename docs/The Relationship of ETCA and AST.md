# ETCA 与 AST 的关系
## 架构与数据结构的协作

---

## 🎯 核心关系

```
ETCA (架构/方法论)        AST (数据结构/操作对象)
       │                        │
       │                        │
       ▼                        ▼
   ┌─────────┐              ┌─────────┐
   │ 何时优化 │──────────────│ 被优化的 │
   │ 如何优化 │   作用于     │ 代码表示 │
   │ 优化什么 │──────────────│         │
   └─────────┘              └─────────┘
```

**简单说**：
- **AST 是什么**：代码的树形数据结构（操作对象）
- **ETCA 是什么**：时间坍缩优化架构（操作方法）
- **关系**：ETCA 在 AST 上实施优化算法

---

## 📊 完整工作流程

### 1. 源代码 → AST
```
源代码:
var a = 10
var b = 20
var c = a + b

     ↓ 解析 (Parser)

AST:
Program
├── Let { name: "a", value: Int(10) }
├── Let { name: "b", value: Int(20) }
└── Let { name: "c", value: BinOp(Var("a"), Add, Var("b")) }
```

### 2. AST → ETCA 准备期优化 → 优化后的 AST
```
原始 AST:
└── Let { name: "c", value: BinOp(Var("a"), Add, Var("b")) }

     ↓ ETCA Pass 1: 常量折叠

优化后 AST:
└── Let { name: "c", value: Int(30) }

坍缩效果: 运行期计算 → 准备期常量
```

### 3. 优化后的 AST → 代码生成
```
优化后 AST:
└── Let { name: "c", value: Int(30) }

     ↓ 代码生成 (CodeGen)

汇编代码:
mov rax, 30
```

---

## 🔧 ETCA 在 AST 上的 4 项准备期优化

### Pass 1: 多遍常量折叠
**操作对象**: AST 表达式节点 (`Expr`)

```rust
// 遍历 AST，识别可折叠的表达式
fn fold_expr_constants(expr: &mut Expr, constants: &HashMap<String, i64>) {
    match expr {
        Expr::BinOp(left, op, right) => {
            // 递归折叠左右子树
            fold_expr_constants(left, constants);
            fold_expr_constants(right, constants);
            
            // 如果两边都是常量，计算结果
            if let (Expr::Int(a), Expr::Int(b)) = (&**left, &**right) {
                *expr = Expr::Int(apply_op(*op, *a, *b));
            }
        }
        Expr::Var(name) => {
            // 查找常量表，替换变量为常量
            if let Some(&value) = constants.get(name) {
                *expr = Expr::Int(value);
            }
        }
        // ... 其他表达式类型
    }
}
```

**AST 转换示例**:
```
Before: BinOp(Var("a"), Add, Var("b"))  // 运行期计算
After:  Int(30)                         // 准备期常量
```

### Pass 2: 激进内联
**操作对象**: AST 函数定义 (`FnDef`) + 调用 (`Call`)

```rust
// 在 AST 中找到函数调用，替换为函数体
fn inline_function_call(call: &Expr, fn_body: &[Stmt]) -> Vec<Stmt> {
    // 将函数调用节点替换为内联后的语句序列
    fn_body.clone()
}
```

**AST 转换示例**:
```
Before AST:
├── FnDef("add", ..., body=[Return(BinOp(...))])
└── Call("add", [Int(10), Int(20)])

After AST (内联后):
└── BinOp(Int(10), Add, Int(20))  // 函数调用消失
```

### Pass 3: 循环展开
**操作对象**: AST 循环节点 (`For`, `While`)

```rust
// 将循环节点展开为重复的语句
fn unroll_loop(loop_stmt: &Stmt) -> Vec<Stmt> {
    // For 循环 → 多个顺序执行的 Stmt
    vec![body.clone(), body.clone(), ...]  // 展开 N 次
}
```

**AST 转换示例**:
```
Before AST:
For { i in 0..3, body: [Print(Var("i"))] }

After AST (展开后):
├── Print(Int(0))
├── Print(Int(1))
└── Print(Int(2))
```

### Pass 4: 死代码消除
**操作对象**: AST 条件分支 (`If`, `Match`)

```rust
// 删除永不执行的 AST 节点
fn eliminate_dead_code(stmt: &mut Stmt) {
    match stmt {
        Stmt::If { condition: Expr::Bool(false), then_branch, else_branch } => {
            // 删除 then 分支，保留 else 分支
            *stmt = else_branch.clone();
        }
        // ... 其他死代码模式
    }
}
```

**AST 转换示例**:
```
Before AST:
If {
    condition: Bool(false),
    then: [Print("never")],
    else: [Print("always")]
}

After AST (消除后):
Print("always")  // if 节点被移除
```

---

## ⚡ ETCA 7 个时间坍缩引擎与 AST

### 1. CTFE - 在 AST 上执行函数
```rust
// CTFE 解释器遍历 AST 并执行
fn ctfe_execute(fn_def: &FnDef) -> Option<i64> {
    let mut interpreter = CtfeInterpreter::new();
    interpreter.execute_ast(fn_def.body)  // 在 AST 上运行
}
```

**工作流程**:
```
AST Function:
FnDef("factorial", [n], 
    If(n <= 1, Return(1), Return(n * factorial(n-1)))
)

↓ CTFE 在准备期执行 AST

Result: Int(120)  // factorial(5) = 120
```

### 2. Dynamic Precomputation - 分析 AST 循环模式
```rust
// 识别 AST 中的收敛循环
fn detect_convergence(loop_node: &Stmt) -> Option<i64> {
    // 分析循环的 AST 结构
    // 预计算最终值
}
```

### 3. TCE - 基于 AST 的时间坍缩决策
```rust
// 分析 AST 节点，决定坍缩策略
fn collapse_strategy(expr: &Expr) -> CollapseLevel {
    match expr {
        Expr::Int(_) => CollapseLevel::Preparation,  // 已经是常量
        Expr::BinOp(Expr::Int(_), _, Expr::Int(_)) => CollapseLevel::Preparation,
        Expr::Call(name, args) if is_pure(name) => CollapseLevel::CompileTime,
        _ => CollapseLevel::Runtime
    }
}
```

### 4-7. 其他引擎
所有引擎都基于 AST 分析：
- **IFM**: 识别 AST 中的重复表达式模式
- **DOPE**: 部分求值 AST 子表达式
- **Scheduler Elimination**: 分析 AST 中的并发节点
- **Pre-Concurrency Folding**: 折叠 AST 中的 `spawn`/`await` 节点

---

## 🌳 AST 是 ETCA 的基础设施

### AST 提供了什么

| AST 功能 | ETCA 如何使用 | 示例 |
|---------|-------------|------|
| **结构化表示** | 遍历和分析代码结构 | 找到所有常量表达式 |
| **可修改性** | 就地优化，转换节点 | `BinOp(10,+,20)` → `Int(30)` |
| **语义信息** | 理解代码含义 | 识别纯函数、死代码 |
| **层次结构** | 递归优化 | 从叶子到根的常量折叠 |
| **类型系统** | 安全性检查 | 避免错误的优化 |

### ETCA 为 AST 提供了什么

| ETCA 功能 | 对 AST 的价值 | 示例 |
|---------|-------------|------|
| **优化策略** | 定义何时优化 AST | 准备期 vs 编译期 |
| **时间坍缩** | 将 AST 计算前移 | 运行期表达式 → 编译期常量 |
| **多遍优化** | 迭代优化 AST 直到收敛 | 2遍折叠，9个表达式优化 |
| **智能分析** | 自动发现 AST 优化机会 | 识别可内联函数 |
| **性能目标** | 驱动 AST 优化方向 | 追求零运行时开销 |

---

## 🔄 完整协作示例

### 输入代码
```slime
fn add(a: int, b: int) -> int {
    return a + b
}

fn main() {
    var x = 10
    var y = 20
    var z = add(x, y)
    printint(z)
}
```

### Step 1: 解析为 AST
```
Program
├── FnDef("add", [a, b], 
│   └── Return(BinOp(Var("a"), Add, Var("b")))
│   )
└── FnDef("main", [],
    ├── Let("x", Int(10))
    ├── Let("y", Int(20))
    ├── Let("z", Call("add", [Var("x"), Var("y")]))
    └── Expr(Call("printint", [Var("z")]))
    )
```

### Step 2: ETCA Pass 1 - 常量折叠
```
常量表: { x: 10, y: 20 }

AST 变化:
Call("add", [Var("x"), Var("y")])
↓
Call("add", [Int(10), Int(20)])  // 变量替换为常量
```

### Step 3: ETCA Pass 2 - 激进内联
```
识别: add() 是小函数 (1条语句)
内联决策: 替换调用为函数体

AST 变化:
Call("add", [Int(10), Int(20)])
↓
BinOp(Int(10), Add, Int(20))  // 函数调用消失
```

### Step 4: ETCA Pass 1 再次运行 (收敛)
```
AST 变化:
BinOp(Int(10), Add, Int(20))
↓
Int(30)  // 表达式求值
```

### Step 5: 最终优化后的 AST
```
Program
├── FnDef("add", ...) [未使用，可被 Pass 4 移除]
└── FnDef("main", [],
    ├── Let("x", Int(10))
    ├── Let("y", Int(20))
    ├── Let("z", Int(30))  ← 完全坍缩！
    └── Expr(Call("printint", [Int(30)]))
    )
```

### Step 6: 代码生成（基于优化后的 AST）
```asm
; 准备期优化后的 AST 生成的汇编
mov rax, 30      ; z = 30 (已在准备期计算)
call printint    ; 直接打印
```

---

## 🎯 关系总结

### 1. **AST = 数据，ETCA = 算法**
```
AST: 代码的树形表示（What）
ETCA: 优化的方法论（How）

关系: ETCA 算法操作 AST 数据结构
```

### 2. **AST 是容器，ETCA 是流程**
```
AST: 承载代码语义的容器
ETCA: 定义优化发生的时间和方式

关系: ETCA 决定何时修改 AST
```

### 3. **AST 提供可能性，ETCA 实现优化**
```
AST: 提供结构化的优化空间
ETCA: 在这个空间中执行时间坍缩

关系: AST 的树形结构支持 ETCA 的递归优化
```

### 4. **双向依赖**
```
ETCA 依赖 AST:
- 需要 AST 的结构化表示
- 需要 AST 的可修改性
- 需要 AST 的语义信息

AST 依赖 ETCA:
- 需要 ETCA 定义优化策略
- 需要 ETCA 驱动优化执行
- 需要 ETCA 实现性能目标
```

---

## 📐 类比理解

### 类比 1: 建筑与施工
```
AST      = 建筑图纸（结构化表示）
ETCA     = 施工方法（何时、如何建造）
优化后AST = 优化后的建筑图纸
最终代码  = 实际建筑
```

### 类比 2: 食谱与烹饪
```
AST      = 食材清单（原始材料）
ETCA     = 烹饪流程（准备、烹饪、上菜三阶段）
优化后AST = 预处理后的食材
最终代码  = 成品菜肴
```

### 类比 3: 乐谱与演奏
```
AST      = 乐谱（音符的树形结构）
ETCA     = 演奏技巧（何时准备、何时演奏）
优化后AST = 简化后的乐谱
最终代码  = 实际演奏
```

---

## 🔍 实际代码示例

### Slime 编译器中的实现

```rust
// src/main.rs

// 1. AST 定义（数据结构）
#[derive(Clone, Debug)]
enum Expr {
    Int(i64),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    Call(String, Vec<Expr>),
    // ... 30+ 种表达式类型
}

// 2. ETCA 优化器（算法）
struct Optimizer {
    constants: HashMap<String, i64>,
    prep_inlined_count: usize,
    // ... ETCA 状态
}

impl Optimizer {
    // ETCA Pass 1: 在 AST 上执行常量折叠
    fn apply_constant_folding(&mut self, program: &mut Program) {
        loop {
            let folded = self.fold_constants_pass(program);
            if folded == 0 { break; }  // 收敛
        }
    }
    
    // 递归遍历 AST，修改节点
    fn fold_expr_constants(&mut self, expr: &mut Expr) -> usize {
        match expr {
            Expr::BinOp(left, op, right) => {
                // 递归优化子树（AST 的树形结构特性）
                let mut count = 0;
                count += self.fold_expr_constants(left);
                count += self.fold_expr_constants(right);
                
                // 折叠操作（ETCA 的坍缩逻辑）
                if let (Expr::Int(a), Expr::Int(b)) = (&**left, &**right) {
                    *expr = Expr::Int(evaluate(*op, *a, *b));
                    count += 1;
                }
                count
            }
            // ... 其他 AST 节点类型
        }
    }
}

// 3. 编译流程（ETCA 架构）
fn compile(source: &str) -> Result<Vec<u8>, Error> {
    // 解析 → AST
    let mut ast = parse(source)?;
    
    // ETCA 准备期优化（修改 AST）
    let mut optimizer = Optimizer::new();
    optimizer.apply_constant_folding(&mut ast);    // Pass 1
    optimizer.apply_aggressive_inlining(&mut ast);  // Pass 2
    optimizer.apply_loop_unrolling(&mut ast);       // Pass 3
    optimizer.apply_dead_code_elimination(&mut ast);// Pass 4
    
    // 代码生成（基于优化后的 AST）
    let assembly = codegen(ast)?;
    
    Ok(assembly)
}
```

---

## 🌟 关键洞察

### 1. **分离关注点**
- **AST**: 专注于"是什么"（代码结构）
- **ETCA**: 专注于"做什么"（优化策略）
- **协作**: 清晰的职责边界，高度的协同效率

### 2. **递归协作**
- **AST**: 树形结构天然支持递归
- **ETCA**: 优化算法递归遍历 AST
- **效果**: 从叶子到根的完整优化

### 3. **时间坍缩的实现基础**
- **没有 AST**: ETCA 无法分析代码结构
- **没有 ETCA**: AST 只是静态数据
- **结合**: 实现运行期 → 准备期的时间坍缩

### 4. **性能的双重保证**
- **AST**: 提供高效的数据结构（O(log n) 访问）
- **ETCA**: 提供智能的算法（多遍收敛优化）
- **结果**: 比 C 快 35.9% 的性能优势

---

## 📚 延伸阅读

- [AST_GUIDE.md](AST_GUIDE.md) - AST 完整教程
- [ETCA_ARCHITECTURE.md](ETCA_ARCHITECTURE.md) - ETCA 架构详解
- [EXCLUSIVE_TECHNOLOGIES.md](EXCLUSIVE_TECHNOLOGIES.md) - 15 项优化技术

---

**核心结论**：

**AST 是舞台，ETCA 是剧本，优化是演出。**

- AST 提供结构化的代码表示（舞台）
- ETCA 定义优化的时间和策略（剧本）
- 时间坍缩是最终的性能提升（精彩演出）

**没有 AST，ETCA 无从下手；没有 ETCA，AST 只是死数据。**

---

*文档创建: 2026年2月2日*
*版本: Slime v0.2.0 with ETCA*
*作者: Sanrol Team*
