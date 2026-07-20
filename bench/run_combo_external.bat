@echo off
REM Full combo bench — run in an EXTERNAL terminal (not inside Cursor Agent).
REM Cursor crashes when the agent waits on 20s+ CPU+alloc storms.
setlocal
cd /d %~dp0\..

set SLIME=target\release\slime.exe
if not exist %SLIME% (
  echo Build release first: cargo build --release
  exit /b 1
)

echo [1/4] emit LLVM...
%SLIME% bench\combo.sm -o bench\combo.ll
if errorlevel 1 exit /b 1

echo [2/4] NO OPT clang -O0 ...
clang -O0 -fno-vectorize -fno-unroll-loops -fno-slp-vectorize -Wno-override-module -D_CRT_SECURE_NO_WARNINGS bench\combo.ll bench\slime_rt.c -o bench\combo_noopt.exe
if errorlevel 1 exit /b 1
echo NOOPT=
bench\combo_noopt.exe

echo [3/4] FULL OPT clang -O3 ...
clang -O3 -Wno-override-module -D_CRT_SECURE_NO_WARNINGS bench\combo.ll bench\slime_rt.c -o bench\combo_opt.exe
if errorlevel 1 exit /b 1
echo OPT=
bench\combo_opt.exe

echo [4/4] C reference -O0 ...
if not exist bench\combo_ref.exe (
  clang -O0 -fno-vectorize -fno-unroll-loops -fno-slp-vectorize -D_CRT_SECURE_NO_WARNINGS bench\combo_ref.c -o bench\combo_ref.exe
)
echo CREF=
bench\combo_ref.exe

echo Done. Compare the three numbers (seconds, 6 decimals).
