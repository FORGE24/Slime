# 连招压测实测（公平对比）

机器：本机 Windows，clang。  
规范：`bench/COMBO_SPEC.md`。种子 `666`，`SCALE=90`。  
日期：2026-07-20。

## A. 公平对比（规范：`-O0` + 关向量化）

| 实现 | 编译 | 单次总耗时（秒） |
|------|------|------------------|
| `bench/combo_ref.c` | clang `-O0 -fno-vectorize -fno-unroll-loops -fno-slp-vectorize` | **40.434270** |
| slime2 `bench/combo.sm` | slime→LLVM + clang **`-O0`**（同上） | **77.886711** |
| FoxLang | — | 无法合规跑满 |

## B. 优化全开（非规范对照）

| 实现 | 编译 | 单次总耗时（秒） |
|------|------|------------------|
| C `combo_ref.c` | clang **`-O3`** | **13.632982** |
| slime2（无 ETCA） | slime→LLVM + clang **`-O3`** | **39.737517** |
| slime2（ETCA 全开） | slime **`--etca`** →LLVM + clang **`-O3`** | **37.661321** |

说明：连招主体是运行期循环（矩阵/sin/排序/哈希/字符串），CTFE 只能折叠少量循环外常量；`--etca` 相对纯 `-O3` 略快约 5%。C `-O3` 仍明显更快（手写 C + 优化器更吃得开）。

### 复现（优化全开）

```text
target\release\slime.exe bench\combo.sm --etca -o bench\combo_etca.ll
clang -O3 -Wno-override-module -D_CRT_SECURE_NO_WARNINGS ^
  bench\combo_etca.ll bench\slime_rt.c -o bench\combo_fullopt.exe
.\bench\combo_fullopt.exe

clang -O3 -D_CRT_SECURE_NO_WARNINGS bench\combo_ref.c -o bench\combo_ref_o3.exe
.\bench\combo_ref_o3.exe
```

### 复现（公平 `-O0`）

```text
target\release\slime.exe bench\combo.sm -o bench\combo.ll

clang -O0 -fno-vectorize -fno-unroll-loops -fno-slp-vectorize -Wno-override-module -D_CRT_SECURE_NO_WARNINGS ^
  bench\combo.ll bench\slime_rt.c -o bench\combo_noopt.exe
.\bench\combo_noopt.exe

clang -O0 -fno-vectorize -fno-unroll-loops -fno-slp-vectorize -D_CRT_SECURE_NO_WARNINGS ^
  bench\combo_ref.c -o bench\combo_ref.exe
.\bench\combo_ref.exe
```

或：`bench\run_combo_external.bat`（勿在 Cursor Agent 里挂起全量压测）。

## FoxLang 缺口

- 无 `*` `/` `%`、数组、哈希、MD5；RNG 违反种子 `666`

## C. 对 CTFE 有利的基准（`bench/ctfe_fav.sm` / `examples/ctfe_loop.sm`）

连招（A/B）几乎全是运行期数据依赖循环（数组/浮点/哈希/字符串），CTFE 吃不到。  
CTFE 折叠范围现已覆盖：

- 纯函数调用（常量实参）→ `store` 常量  
- **封闭的 `while` / `for`（整段进编译期 VM）** → 无循环 IR，只留残留 `store`  
- 运行期残留循环内：仍可折**不读局部变量**的闭包表达式（如 `fib(45)`）；禁止冻循环携带状态

`examples/ctfe_loop.sm`（`N=1e5` 内联 while + for）在 `--etca` 下 `@main` **零** `while.`/`for.` 标签，结果与 `-fc` 一致。

工作量（`ctfe_fav`）：`work(2_500_000)×4` + `fib(45..47)`。

| 路径 | 含义 | 实测 |
|------|------|------|
| C `-O3` | **运行期**做完全部计算 | 计算 **~14 ms**（程序自报）；wall ~22 ms |
| slime `-fc` | 解释执行 | interp **~3166 ms** |
| slime `-O` | 编译期 PE → residual 只 `puts` | PE **~2741 ms**；之后再跑 exe **~13 ms** |
| slime `--etca` + clang `-O3` | CTFE2 函数+循环折叠进 IR | emit **~26 s**；exe 稳态 **~8 ms** |

**读法：** 对 CTFE 有利时，贵的是**编译/折叠**，便宜的是**每次运行**；连招则相反。

### 复现

```text
target\release\slime.exe examples\ctfe_loop.sm --etca -o examples\ctfe_loop.ll
# @main 应无 while./for.，只有 store 常量 + printf

clang -O3 -D_CRT_SECURE_NO_WARNINGS bench\ctfe_fav_ref.c -o bench\ctfe_fav_ref.exe
.\bench\ctfe_fav_ref.exe

target\release\slime.exe bench\ctfe_fav.sm -fc --time
target\release\slime.exe bench\ctfe_fav.sm -O --time -o bench\ctfe_fav_residual.ll
.\bench\ctfe_fav.exe

target\release\slime.exe bench\ctfe_fav.sm --etca -o bench\ctfe_fav_etca.ll
clang -O3 -Wno-override-module bench\ctfe_fav_etca.ll -o bench\ctfe_fav_etca.exe
.\bench\ctfe_fav_etca.exe
```

## 备注

- 开放循环（数组下标、I/O、`break` 等）仍降级运行期；循环条件不会再被冻成 `br i1 1`。
- 字符串 `+` 已按所有权释放中间结果。
