# Slime 项目概览 / Project Overview

## 📋 项目简介 / Project Summary

**Slime** 是一个快速、轻量级的编译型编程语言，采用 Rust 实现。它的目标是成为"Rust 版的 Python"——编译型、语法易懂、自动内存管理、快速执行。

**Slime** is a fast, lightweight compiled programming language implemented in Rust. It aims to be "Python for Rust" - compiled, easy syntax, automatic memory management, and fast execution.

## 🎯 核心特性 / Core Features

### 1. **编译目标 / Compilation Target**
- 编译到 x86-64 NASM 汇编代码
- 支持多平台：Linux x64, Windows x64, macOS x64
- 生成高度优化的机器代码

### 2. **核心理念：万物皆接口 / Everything is an Interface**
- 接口是 Slime 的第一公民，类似 Java 的"万物皆对象"
- 变量、函数、类型、I/O 都通过接口抽象
- 接口可以定义、调用、组合、释放

### 3. **ETCA 架构 / Execution-Time Collapse Architecture**
Slime 采用独特的"执行时间坍缩架构"（ETCA），包含三个层次：

**第一层：准备期 (Preparation Time)**
- 多遍常量折叠
- 激进内联
- 循环展开
- 死代码消除

**第二层：编译期 (Compilation Time)**
- 零分析、零优化
- 直接翻译为汇编

**第三层：运行期 (Runtime)**
- 零计算开销
- 仅执行必要的I/O
- 接近理论性能上限

### 4. **8大优化引擎 / 8 Optimization Engines**

项目包含以下高级优化模块：

1. **CTFE** (Compile-Time Function Execution) - 编译期函数执行
2. **Dynamic Precomputer** - 动态预计算器
3. **TCE** (Temporal Collapse Engine) - 时间坍缩引擎
4. **IFM** (Interface Manager) - 接口管理器
5. **DOPE** (Dynamic Optimization & Pre-Execution) - 动态优化与预执行
6. **Scheduler Elimination** - 调度器消除
7. **Pre-Concurrency** - 预并发引擎
8. **Extended CTFE** - 扩展编译期执行

## 📁 项目结构 / Project Structure

```
Slime/
├── src/                    # 源代码 (59,059 行)
│   ├── main.rs            # 主编译器 (8,257 行)
│   ├── ctfe.rs            # 编译期函数执行 (1,583 行)
│   ├── ctfe_extended.rs   # 扩展CTFE (1,052 行)
│   ├── dope.rs            # 动态优化 (3,413 行)
│   ├── dynamic_precomp.rs # 动态预计算 (4,072 行)
│   ├── ifm.rs             # 接口管理 (6,273 行)
│   ├── pre_concurrency.rs # 预并发 (6,058 行)
│   ├── scheduler_elimination.rs # 调度器消除 (4,760 行)
│   └── tce.rs             # 时间坍缩引擎 (23,591 行)
├── docs/                   # 文档
│   ├── Abstract Syntax Tree.md
│   ├── C, CPP Native Bridge.md
│   ├── Execution-Time Collapse Architecture.md
│   ├── GetStarted.md
│   ├── Module System.md
│   └── The Relationship of ETCA and AST.md
├── examples/              # 示例代码
│   └── test.sm           # 演示各种优化的测试程序
├── Cargo.toml            # Rust 项目配置
├── Makefile              # 构建脚本
└── README.md             # 项目说明

总代码量：约 59,000 行 Rust 代码
```

## 🔧 技术栈 / Technology Stack

- **语言**: Rust (Edition 2021)
- **编译器版本**: 0.2.0
- **目标**: x86-64 NASM Assembly
- **许可证**: GPL v2 (项目) / MIT (Cargo包)

## 🚀 构建与运行 / Build & Run

### 构建编译器
```bash
# 使用 Cargo
cargo build -r

# 或使用 Make
make
```

### 编译 Slime 程序
```bash
# Linux
slimec input.sm -o output.asm
nasm -felf64 output.asm -o output.o
ld -o output output.o

# Windows
slimec input.sm -o output.asm
nasm -fwin64 output.asm -o output.obj
golink /console /entry main kernel32.dll output.obj

# macOS
slimec input.sm -o output.asm
nasm -fmacho64 output.asm -o output.o
ld -o output output.o -lSystem
```

## 💡 设计亮点 / Design Highlights

### 1. 所有权检查系统
- 内置所有权检查器，确保内存安全
- 自动内存管理

### 2. 模块系统
- 支持模块导入和加载
- 模块化的代码组织

### 3. 超级优化
示例程序 `test.sm` 展示了以下优化：
- 编译期常量计算（斐波那契、求和）
- 循环优化和展开
- 算术序列公式化
- 矩阵计算优化
- 并发任务优化

### 4. 性能目标
根据文档声称：比 C 语言快 35.9%

## 📊 代码统计 / Code Statistics

| 模块 | 行数 | 功能 |
|------|------|------|
| tce.rs | 23,591 | 时间坍缩引擎 |
| main.rs | 8,257 | 主编译器逻辑 |
| ifm.rs | 6,273 | 接口管理 |
| pre_concurrency.rs | 6,058 | 预并发引擎 |
| scheduler_elimination.rs | 4,760 | 调度器消除 |
| dynamic_precomp.rs | 4,072 | 动态预计算 |
| dope.rs | 3,413 | 动态优化 |
| ctfe.rs | 1,583 | 编译期执行 |
| ctfe_extended.rs | 1,052 | 扩展CTFE |

**总计**: 59,059 行代码

## 🎓 学习资源 / Learning Resources

项目提供了详细的文档：
- **AST 详解**: 语法树结构和编译流程
- **ETCA 架构**: 执行时间坍缩架构的详细说明
- **模块系统**: 模块导入和组织
- **C/C++ 桥接**: 与原生代码的接口
- **ETCA 与 AST 的关系**: 优化架构如何作用于语法树

## 🌟 项目特色 / Project Highlights

1. **创新的优化架构**: ETCA 架构通过"时间坍缩"实现极致性能
2. **大规模代码库**: 近 6 万行精心设计的 Rust 代码
3. **完整的编译器实现**: 从词法分析到代码生成的完整流程
4. **多平台支持**: 支持主流的 x64 平台
5. **学术与工程结合**: 既有理论创新又有工程实现

## 📝 示例代码 / Sample Code

```slime
// 编译期常量
const FIB_30 = 832040
const SUM_1M = 499999500000

// 函数定义
fn ultra_compute_heavy(n: int) -> int {
    // sum(i^2) = n*(n+1)*(2n+1)/6
    return n * (n + 1) * (2 * n + 1) / 6
}

// 主函数
fn main {
    print "=== Slime Ultra ==="
    print "fib(30) =", FIB_30
    var result = ultra_compute_heavy(100)
    print "result =", result
}
```

## 🔮 项目状态 / Project Status

- **版本**: 0.2.0
- **开发状态**: 活跃开发中
- **最新提交**: Fix the wrong license in README
- **团队**: Slime Lang Team / Sanrol Team

## 📄 许可证 / License

- 项目代码: GPL v2
- Cargo 包: MIT

---

**总结**: Slime 是一个雄心勃勃的编程语言项目，旨在通过创新的编译器优化技术（ETCA）实现超越 C 语言的性能，同时保持易用的语法和自动内存管理。项目代码量大、架构复杂、优化深入，是一个值得学习的高性能编译器实现。
