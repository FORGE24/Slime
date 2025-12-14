# Slime Lang 解释器

一个基于 C++ 实现的简单编程语言解释器，支持基本的编程功能和数学运算。

## 功能特点

- 支持基本的编程语法（函数创建、调用、删除）
- 支持数学运算（加、减、乘、除、取余）
- 支持接口创建和映射
- 支持预执行库引入
- 支持常量定义和配置设置
- 支持运算符优先级和括号
- 支持条件语句（if-else）
- 支持循环语句（while、for）
- 支持循环控制语句（break、continue）
- 支持变量赋值和引用
- 支持字节码编译和执行
- 支持编译为可执行文件（exe）
- 统一的值类型系统（数值、字符串、布尔值）
- 智能指针自动内存管理
- 性能基准测试工具
- CMake构建系统支持

## 安装和编译

### 环境要求

- C++ 编译器（支持 C++11 或更高版本）
- Windows 或 Linux 操作系统

### 编译方法

#### 方法1：使用g++直接编译

在项目根目录下执行以下命令：

```bash
g++ -std=c++11 simple_interpreter.cpp bytecode.cpp -o simple_interpreter
g++ -std=c++11 slime_benchmark.cpp -o slime_benchmark
```

#### 方法2：使用CMake构建（推荐）

在项目根目录下执行以下命令：

```bash
cmake .
make
```

或者在Windows上使用：

```bash
cmake .
cmake --build .
```

这将生成两个可执行文件：
- `simple_interpreter`：Slime语言解释器
- `slime_benchmark`：基准测试工具

## 使用方法

### 运行程序

```bash
./simple_interpreter.exe <filename>
```

例如：

```bash
./simple_interpreter.exe main.sme
```

### 编译为字节码

```bash
./simple_interpreter.exe --compile-to-btc <input_file> <output_file.btc>
```

### 编译为可执行文件

```bash
./simple_interpreter.exe --compile-to-exe <input_file> <output_file.exe>
```

### 执行字节码文件

```bash
./simple_interpreter.exe --run-bytecode <bytecode_file.btc>
```

### 基本语法

#### 1. 预执行库引入

```sme
// 预执行库引入
#v introduce "baseline.smeh"
```

用于引入基础库文件，包含常用函数映射和常量定义。`#v` 是特殊的预执行指令标记，不是注释。

#### 2. 函数创建

```sme
cra main{
    // 函数体
}
```

用于创建一个函数，函数名可以自定义，函数体包含在大括号内。

#### 3. 接口创建

```sme
cre sta.interface "Example.Print" int Out "System.Output.Print"
```

- 创建一个接口，命名为"Example.Print"
- 定义接口规范（sta.interface）
- 指定I/O行为为输出（int Out）
- 将接口映射到系统函数"System.Output.Print"

#### 4. 函数调用

```sme
use Example.Print "Hello World"
```

用于调用已创建的接口或函数，参数可以是字符串字面量或表达式。

#### 5. 资源删除

```sme
del Example.Print
```

用于删除接口或函数，释放资源。

#### 6. 变量赋值

```sme
let x = 10
let y = x + 5
```

使用 `let` 关键字进行变量赋值。

#### 7. 条件语句

```sme
if x > 5 {
    use print "x is greater than 5"
} else {
    use print "x is less than or equal to 5"
}
```

#### 8. 循环语句

##### While 循环

```sme
let i = 0
while i < 10 {
    use print i
    let i = i + 1
}
```

##### For 循环

```sme
for let i = 0; i < 10; let i = i + 1 {
    use print i
}
```

#### 9. 循环控制语句

##### Break 语句

```sme
let i = 0
while true {
    if i > 5 {
        break
    }
    use print i
    let i = i + 1
}
```

##### Continue 语句

```sme
let i = 0
while i < 10 {
    let i = i + 1
    if i == 5 {
        continue
    }
    use print i
}
```

## 示例代码

### 基本打印示例

```sme
// 预执行库引入
#v introduce "baseline.smeh"

// 创建主函数
cra main{
    // 打印输出
    use print "Hello World"
}

// 删除主函数
del main
```

### 接口创建和调用示例

```sme
// 预执行：引入基础库文件
#v introduce "baseline.smeh"

// 创建(cra)主函数(main)，开始函数体
cra main{
    // 创建接口
    cre sta.interface "Example.Print" int Out "System.Output.Print"
    // 1. 首先创建(cre)一个接口，命名为"Example.Print"
    // 2. 为该接口定义一个通用标准接口规范(sta.interface)
    // 3. 在此标准接口上指定I/O行为为输出(int Out)
    // 4. 将此接口映射到系统底层打印功能"System.Output.Print"
    // 注：这是一个复合操作，一行代码完成了接口创建、标准定义、功能绑定

    // 使用(use)已创建的接口，传入参数"Hello World"进行打印
    use Example.Print "Hello World"
 
    // 删除(del)接口，释放资源
    del Example.Print
}  // 主函数体结束

// 删除主函数定义
del main
```

### 数学运算示例

```sme
// 预执行库引入
#v introduce "baseline.smeh"

// 创建主函数
cra main{
    // 数学运算示例
    use print 2 + 3 * 4  // 输出 14
    use print (2 + 3) * 4  // 输出 20
    use print 10 / 2  // 输出 5
    use print 10 % 3  // 输出 1
}

// 删除主函数
del main
```

### 循环和条件语句示例

```sme
// 预执行库引入
#v introduce "baseline.smeh"

// 创建主函数
cra main{
    // For 循环示例
    for let i = 1; i <= 5; let i = i + 1 {
        use print "Iteration:" i
    }
    
    // While 循环示例
    let j = 10
    while j > 0 {
        if j == 5 {
            use print "Skipping 5"
            let j = j - 1
            continue
        }
        if j == 3 {
            use print "Breaking at 3"
            break
        }
        use print "Countdown:" j
        let j = j - 1
    }
}

// 删除主函数
del main
```

## 项目结构

```
Slime lang/
├── simple_interpreter.cpp  # 解释器主代码
├── bytecode.h              # 字节码定义头文件
├── main.sme               # 示例代码
├── baseline.smeh          # 基础库文件
└── README.md              # 项目说明文档
```

## 支持的函数

### 输出函数
- `print` / `System.Output.Print` - 打印内容
- `println` / `System.Output.Println` - 打印内容并换行

### 输入函数
- `input` / `System.Input.Read` - 读取输入（空格分隔）
- `inputln` / `System.Input.ReadLine` - 读取整行输入

### 数学函数
- `add` / `System.Math.Add` - 加法
- `sub` / `System.Math.Subtract` - 减法
- `mul` / `System.Math.Multiply` - 乘法
- `div` / `System.Math.Divide` - 除法
- `mod` / `System.Math.Modulo` - 取余

## 支持的指令

Slime语言中，以`#`开头的标记是特殊指令，不是注释。常用指令包括：

### 注册指令

```sme
// 注册指令：将"print"映射到"System.Output.Print"
#v register "print" to "System.Output.Print"
```

用于注册函数映射。

### 常量定义指令

```sme
// 常量定义指令：定义TRUE为1
#n define "TRUE" 1
```

用于定义常量。

### 配置设置指令

```sme
// 配置设置指令：开启严格模式
#a set "strict_mode" ON
```

用于设置配置项。

## 字节码格式

Slime 语言支持将源代码编译为字节码格式（.btc 文件），以便更快速地执行。字节码包含了操作码和常量池，可以直接由虚拟机执行。

## 基准测试工具

项目包含了一个基准测试工具 `slime_benchmark`，用于比较直接解释执行和字节码执行的性能差异。

### 使用方法

```bash
./slime_benchmark.exe <filename> <mode> [iterations]
```

参数说明：
- `filename`: 要测试的 Slime 代码文件
- `mode`: 执行模式，可选 `interpret`（直接解释）或 `bytecode`（字节码执行）
- `iterations`: 迭代次数，默认为 10

示例：
```bash
./slime_benchmark.exe benchmark_comprehensive.sme interpret 20
./slime_benchmark.exe benchmark_comprehensive.sme bytecode 20
```

### 输出结果

基准测试工具会输出详细的性能统计数据，包括：
- 最小执行时间
- 最大执行时间
- 平均执行时间
- 标准差
- 总执行时间

## 性能优化

- 使用字节码虚拟机提高执行效率
- 支持将代码编译为原生可执行文件
- 内存管理优化，防止内存泄漏
- 字符串转义防止代码注入攻击

## 注意事项

1. 解释器目前只支持基本功能，可能存在一些限制和bug
2. 建议使用简单的代码结构，避免复杂的嵌套和递归
3. 如果遇到内存问题，可以尝试减少代码规模或简化逻辑
4. 编译为可执行文件时，请确保系统安装了相应的编译器（如 MinGW 或 MSVC）