# Slime 模块系统

Slime 支持完整的模块系统，包括包导入、库包含和符号导出。

## 语法概览

```
┌─────────────────────────────────────────────────────────────┐
│                    Slime 模块系统                            │
├─────────────────────────────────────────────────────────────┤
│  1. import "path/module.sm"       → 导入整个模块             │
│  2. import "module" as alias      → 导入并取别名             │
│  3. use module::func              → 导入特定符号 (Rust风格)  │
│  4. use module::{a, b, c}         → 批量导入符号             │
│  5. use module::*                 → 导入所有公开符号          │
│  6. from "module" import func     → Python风格导入           │
│  7. pub fn / pub let              → 公开导出                 │
│  8. include "header.smh"          → C风格文本包含            │
└─────────────────────────────────────────────────────────────┘
```

## 1. import - 模块导入

### 基本导入
```slime
// 导入整个模块
import "mathlib.sm"

// 使用模块中的函数
let result = add(1, 2)
```

### 带别名导入
```slime
// 导入并指定别名
import "utils/string-helper.sm" as strhelper

// 现在可以通过别名引用（未来支持）
```

### 模块路径解析
1. 首先检查绝对路径
2. 相对于当前文件目录
3. 搜索 `SLIME_PATH` 环境变量指定的路径
4. 搜索编译器目录下的 `stdlib` 和 `lib` 文件夹

## 2. use - Rust 风格导入

### 导入单个符号
```slime
// 从模块导入特定函数
use math::add
use math::factorial

let x = add(1, 2)
let y = factorial(5)
```

### 批量导入
```slime
// 导入多个符号
use math::{add, sub, mul, div}

// 带别名的批量导入
use math::{add as plus, sub as minus}
```

### 通配符导入
```slime
// 导入模块所有公开符号
use math::*

// 可以直接使用所有导出的函数
let a = add(1, 2)
let b = factorial(5)
let c = power(2, 10)
```

## 3. from ... import - Python 风格导入

```slime
// Python 风格的导入语法
from "mathlib.sm" import add, sub

// 带别名
from "mathlib.sm" import add as plus, sub as minus
```

## 4. pub - 公开导出

使用 `pub` 关键字标记需要导出的符号：

```slime
// mathlib.sm - 数学库模块

// 公开函数
pub fn add(a: int, b: int) -> int {
    return a + b
}

// 公开常量
pub let PI_INT = 3

// 私有函数（不加 pub，仅模块内部使用）
fn internal_helper(x: int) -> int {
    return x * 2
}
```

## 5. include - C 风格文本包含

用于简单的文本级别包含，类似 C 的 `#include`：

```slime
// 包含头文件
include "common.smh"

// common.smh 的内容会被直接插入到此位置
```

### 头文件示例 (common.smh)
```slime
// 常用常量
let MAX_SIZE = 100
let MIN_SIZE = 1

// 接口定义
def static.interface "debug_print" any Out "host.stdout"
```

## 6. 模块搜索路径

模块加载器按以下顺序搜索模块：

1. **当前目录** - 相对于正在编译的文件
2. **标准库路径** - 编译器所在目录的 `stdlib/` 和 `lib/`
3. **环境变量** - `SLIME_PATH` 环境变量指定的路径（用 `;` 分隔）

### 设置环境变量
```powershell
# Windows PowerShell
$env:SLIME_PATH = "C:\slime\libs;D:\my-slime-modules"

# Linux/macOS
export SLIME_PATH="/usr/local/slime/libs:/home/user/slime-modules"
```

## 7. 循环依赖检测

模块加载器会自动检测循环依赖并报错：

```
错误: Circular dependency detected: a.sm -> b.sm -> a.sm
```

## 8. 完整示例

### mathlib.sm (库模块)
```slime
// 公开函数
pub fn add(a: int, b: int) -> int {
    return a + b
}

pub fn factorial(n: int) -> int {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}
```

### main.sm (主程序)
```slime
// 导入数学库
import "mathlib.sm"

fn main {
    let sum = add(10, 20)
    print "10 + 20 =", sum
    
    let fact = factorial(5)
    print "5! =", fact
}
```

### 编译运行
```bash
slimec main.sm -o main.asm --target windows
nasm -fwin64 main.asm -o main.obj
link main.obj /subsystem:console /entry:Start kernel32.lib
main.exe
```

## 9. 最佳实践

1. **使用明确导入** - 优先使用 `use module::func` 而非 `use module::*`
2. **合理组织模块** - 相关功能放在同一模块
3. **避免循环依赖** - 使用层次化的模块结构
4. **使用 pub 控制可见性** - 只导出需要公开的符号
5. **头文件用于常量** - `include` 适合包含常量定义和接口声明
