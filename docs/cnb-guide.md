# Slime CNB (C/C++ Native Bridge)

CNB 是 Slime 语言的 C/C++ 原生桥接系统，允许直接调用 C 库函数、Windows API 和其他原生代码。

## 功能特性

- **extern 函数声明** - 声明外部 C 函数
- **调用约定支持** - cdecl, stdcall, fastcall, win64, sysv
- **C 类型映射** - 完整的 C 类型到 Slime 类型映射
- **结构体互操作** - 定义和使用 C 结构体
- **动态库支持** - 指定函数所在的 DLL/SO

## 基本语法

### 声明外部函数

```slime
// 基本语法
extern "calling_conv" fn name(params) -> ret_type

// Windows API (stdcall)
extern "stdcall" fn MessageBoxA(hwnd: uintptr, text: cstr, caption: cstr, flags: uint) -> int

// C 运行时 (cdecl)
extern "C" fn printf(fmt: cstr) -> int
extern "C" fn malloc(size: size_t) -> *void

// 指定库名
extern "C" fn my_func(x: int) -> int from "mylib.dll"
```

### 调用约定

| 约定 | 别名 | 说明 |
|------|------|------|
| `"C"` | `"cdecl"` | C 默认调用约定 |
| `"stdcall"` | `"STDCALL"` | Windows API 标准约定 |
| `"fastcall"` | `"FASTCALL"` | 快速调用（使用寄存器） |
| `"win64"` | `"ms"` | Windows x64 约定 |
| `"sysv"` | `"linux"` | Linux/macOS x64 约定 |

### C 类型映射

#### 基本类型

| Slime 类型 | C 类型 | 大小 |
|------------|--------|------|
| `void` | `void` | 0 |
| `char` | `char` | 1 |
| `short` | `short` | 2 |
| `int` | `int` | 4 |
| `long` | `long` | 4/8 |
| `longlong` | `long long` | 8 |
| `float` / `f32` | `float` | 4 |
| `double` / `f64` | `double` | 8 |
| `bool` | `_Bool` | 1 |

#### 无符号类型

| Slime 类型 | C 类型 | 大小 |
|------------|--------|------|
| `uchar` | `unsigned char` | 1 |
| `ushort` | `unsigned short` | 2 |
| `uint` / `unsigned` | `unsigned int` | 4 |
| `ulong` | `unsigned long` | 4/8 |
| `ulonglong` | `unsigned long long` | 8 |

#### 固定大小类型

| Slime 类型 | C 类型 | 大小 |
|------------|--------|------|
| `i8` / `int8` | `int8_t` | 1 |
| `i16` / `int16` | `int16_t` | 2 |
| `i32` / `int32` | `int32_t` | 4 |
| `i64` / `int64` | `int64_t` | 8 |
| `u8` / `uint8` / `byte` | `uint8_t` | 1 |
| `u16` / `uint16` | `uint16_t` | 2 |
| `u32` / `uint32` | `uint32_t` | 4 |
| `u64` / `uint64` | `uint64_t` | 8 |

#### 平台相关类型

| Slime 类型 | C 类型 | 说明 |
|------------|--------|------|
| `size_t` / `usize` | `size_t` | 无符号大小 |
| `isize` / `ptrdiff_t` | `ptrdiff_t` | 有符号差值 |
| `intptr` | `intptr_t` | 有符号指针 |
| `uintptr` / `HANDLE` / `HWND` | `uintptr_t` | 无符号指针 |

#### 指针类型

| Slime 类型 | C 类型 |
|------------|--------|
| `*T` | `T*` |
| `*const T` / `const *T` | `const T*` |
| `cstr` / `LPCSTR` | `const char*` |
| `wstr` / `LPCWSTR` | `const wchar_t*` |

### 声明外部结构体

```slime
extern struct POINT {
    x: int,
    y: int
}

extern struct RECT {
    left: int,
    top: int,
    right: int,
    bottom: int
}

// Windows 消息结构
extern struct MSG {
    hwnd: HWND,
    message: uint,
    wParam: uintptr,
    lParam: intptr,
    time: u32,
    pt: POINT
}
```

### unsafe 块

对于需要进行原始指针操作的代码，使用 unsafe 块：

```slime
unsafe {
    let ptr = malloc(100)
    // 原始指针操作
    free(ptr)
}
```

## 完整示例

### Windows MessageBox

```slime
extern "stdcall" fn MessageBoxA(hwnd: uintptr, text: cstr, caption: cstr, flags: uint) -> int

MessageBoxA(0, "Hello from Slime!", "CNB Demo", 0)
```

### 调用 C 标准库

```slime
extern "C" fn printf(fmt: cstr) -> int
extern "C" fn strlen(s: cstr) -> size_t

let msg = "Hello, World!"
let len = strlen(msg)
print "String length:", len
```

### 内存分配

```slime
extern "C" fn malloc(size: size_t) -> *void
extern "C" fn free(ptr: *void)

unsafe {
    let buffer = malloc(1024)
    // 使用 buffer...
    free(buffer)
}
```

### Windows API 综合示例

```slime
// 声明 Windows API
extern "stdcall" fn GetTickCount() -> u32
extern "stdcall" fn Sleep(ms: u32)
extern "stdcall" fn Beep(freq: u32, duration: u32) -> int

// 使用
let start = GetTickCount()
Beep(440, 500)  // A4 音符，500ms
Sleep(100)
Beep(880, 500)  // A5 音符，500ms
let elapsed = GetTickCount() - start
print "Total time:", elapsed, "ms"
```

## 编译和链接

### Windows

```powershell
# 编译为汇编
slimec program.sm -o program.asm --target windows

# 汇编
nasm -fwin64 program.asm -o program.obj

# 链接 (使用需要的库)
golink /entry Start program.obj kernel32.dll user32.dll msvcrt.dll
```

### Linux

```bash
# 编译为汇编
slimec program.sm -o program.asm --target linux

# 汇编
nasm -felf64 program.asm -o program.o

# 链接
ld -o program program.o -lc -dynamic-linker /lib64/ld-linux-x86-64.so.2
```

## 注意事项

1. **调用约定** - Windows API 通常使用 stdcall，C 运行时使用 cdecl
2. **字符串** - C 字符串以 null 结尾，使用 `cstr` 类型
3. **指针安全** - 指针操作应在 unsafe 块中进行
4. **内存管理** - 使用 malloc/free 时需要手动管理内存
5. **平台差异** - `long` 在 Windows 是 4 字节，Linux 是 8 字节

## 常用 Windows API 声明

```slime
// 消息框
extern "stdcall" fn MessageBoxA(hwnd: uintptr, text: cstr, caption: cstr, flags: uint) -> int
extern "stdcall" fn MessageBoxW(hwnd: uintptr, text: wstr, caption: wstr, flags: uint) -> int

// 控制台
extern "stdcall" fn GetStdHandle(handle: int) -> HANDLE
extern "stdcall" fn WriteConsoleA(handle: HANDLE, buffer: cstr, len: u32, written: *u32, reserved: *void) -> int

// 系统
extern "stdcall" fn GetTickCount() -> u32
extern "stdcall" fn Sleep(ms: u32)
extern "stdcall" fn ExitProcess(code: uint)

// 内存
extern "stdcall" fn VirtualAlloc(addr: *void, size: size_t, type: u32, protect: u32) -> *void
extern "stdcall" fn VirtualFree(addr: *void, size: size_t, type: u32) -> int

// 文件
extern "stdcall" fn CreateFileA(name: cstr, access: u32, share: u32, security: *void, disposition: u32, flags: u32, template: HANDLE) -> HANDLE
extern "stdcall" fn ReadFile(handle: HANDLE, buffer: *void, bytes: u32, read: *u32, overlapped: *void) -> int
extern "stdcall" fn WriteFile(handle: HANDLE, buffer: *void, bytes: u32, written: *u32, overlapped: *void) -> int
extern "stdcall" fn CloseHandle(handle: HANDLE) -> int
```

## 常用 C 标准库声明

```slime
// 字符串
extern "C" fn strlen(s: cstr) -> size_t
extern "C" fn strcpy(dest: *char, src: cstr) -> *char
extern "C" fn strcat(dest: *char, src: cstr) -> *char
extern "C" fn strcmp(s1: cstr, s2: cstr) -> int

// 内存
extern "C" fn malloc(size: size_t) -> *void
extern "C" fn calloc(num: size_t, size: size_t) -> *void
extern "C" fn realloc(ptr: *void, size: size_t) -> *void
extern "C" fn free(ptr: *void)
extern "C" fn memcpy(dest: *void, src: *void, n: size_t) -> *void
extern "C" fn memset(s: *void, c: int, n: size_t) -> *void

// I/O
extern "C" fn printf(fmt: cstr) -> int
extern "C" fn puts(s: cstr) -> int
extern "C" fn getchar() -> int

// 数学
extern "C" fn abs(x: int) -> int
extern "C" fn sqrt(x: double) -> double
extern "C" fn sin(x: double) -> double
extern "C" fn cos(x: double) -> double
```
