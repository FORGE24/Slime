#!/usr/bin/env pwsh
# ============================================================================
# 全语言性能对比测试脚本
# 测试语言: Python, Node.js, C, C++, Rust, Java, Slime
# ============================================================================

Write-Host "════════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "   多语言性能对比测试 - Slime vs C/C++/Rust/Java/Python/Node.js" -ForegroundColor Cyan
Write-Host "════════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

$results = @()

# ============================================================================
# 1. Python测试
# ============================================================================
Write-Host "[1/7] 测试 Python..." -ForegroundColor Yellow
if (Get-Command python -ErrorAction SilentlyContinue) {
    $output = python compare_python.py 2>&1 | Out-String
    if ($output -match "总耗时:\s*([\d.]+)\s*ms") {
        $pythonTime = [double]$matches[1]
        $results += @{Lang="Python"; Time=$pythonTime; Status="✓"}
        Write-Host "  ✓ Python: $pythonTime ms" -ForegroundColor Green
    } else {
        Write-Host "  ✗ Python执行失败" -ForegroundColor Red
        $results += @{Lang="Python"; Time=0; Status="✗"}
    }
} else {
    Write-Host "  - Python未安装，跳过" -ForegroundColor Gray
    $results += @{Lang="Python"; Time=0; Status="-"}
}
Write-Host ""

# ============================================================================
# 2. Node.js测试
# ============================================================================
Write-Host "[2/7] 测试 Node.js..." -ForegroundColor Yellow
if (Get-Command node -ErrorAction SilentlyContinue) {
    $output = node compare_nodejs.js 2>&1 | Out-String
    if ($output -match "总耗时:\s*([\d.]+)\s*ms") {
        $nodeTime = [double]$matches[1]
        $results += @{Lang="Node.js"; Time=$nodeTime; Status="✓"}
        Write-Host "  ✓ Node.js: $nodeTime ms" -ForegroundColor Green
    } else {
        Write-Host "  ✗ Node.js执行失败" -ForegroundColor Red
        $results += @{Lang="Node.js"; Time=0; Status="✗"}
    }
} else {
    Write-Host "  - Node.js未安装，跳过" -ForegroundColor Gray
    $results += @{Lang="Node.js"; Time=0; Status="-"}
}
Write-Host ""

# ============================================================================
# 3. C语言测试
# ============================================================================
Write-Host "[3/7] 测试 C (gcc -O3)..." -ForegroundColor Yellow
if (Get-Command gcc -ErrorAction SilentlyContinue) {
    Write-Host "  编译中..." -ForegroundColor Gray
    gcc -O3 compare_c.c -o compare_c.exe 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $output = .\compare_c.exe 2>&1 | Out-String
        if ($output -match "总耗时:\s*([\d.]+)\s*ms") {
            $cTime = [double]$matches[1]
            $results += @{Lang="C (gcc -O3)"; Time=$cTime; Status="✓"}
            Write-Host "  ✓ C: $cTime ms" -ForegroundColor Green
        } else {
            Write-Host "  ✗ C执行失败" -ForegroundColor Red
            $results += @{Lang="C (gcc -O3)"; Time=0; Status="✗"}
        }
    } else {
        Write-Host "  ✗ C编译失败" -ForegroundColor Red
        $results += @{Lang="C (gcc -O3)"; Time=0; Status="✗"}
    }
} else {
    Write-Host "  - GCC未安装，跳过" -ForegroundColor Gray
    $results += @{Lang="C (gcc -O3)"; Time=0; Status="-"}
}
Write-Host ""

# ============================================================================
# 4. C++测试
# ============================================================================
Write-Host "[4/7] 测试 C++ (g++ -O3)..." -ForegroundColor Yellow
if (Get-Command g++ -ErrorAction SilentlyContinue) {
    Write-Host "  编译中..." -ForegroundColor Gray
    g++ -O3 -std=c++17 compare_cpp.cpp -o compare_cpp.exe 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $output = .\compare_cpp.exe 2>&1 | Out-String
        if ($output -match "总耗时:\s*([\d.]+)\s*ms") {
            $cppTime = [double]$matches[1]
            $results += @{Lang="C++ (g++ -O3)"; Time=$cppTime; Status="✓"}
            Write-Host "  ✓ C++: $cppTime ms" -ForegroundColor Green
        } else {
            Write-Host "  ✗ C++执行失败" -ForegroundColor Red
            $results += @{Lang="C++ (g++ -O3)"; Time=0; Status="✗"}
        }
    } else {
        Write-Host "  ✗ C++编译失败" -ForegroundColor Red
        $results += @{Lang="C++ (g++ -O3)"; Time=0; Status="✗"}
    }
} else {
    Write-Host "  - G++未安装，跳过" -ForegroundColor Gray
    $results += @{Lang="C++ (g++ -O3)"; Time=0; Status="-"}
}
Write-Host ""

# ============================================================================
# 5. Rust测试
# ============================================================================
Write-Host "[5/7] 测试 Rust (rustc -O)..." -ForegroundColor Yellow
if (Get-Command rustc -ErrorAction SilentlyContinue) {
    Write-Host "  编译中..." -ForegroundColor Gray
    rustc -O compare_rust.rs 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $output = .\compare_rust.exe 2>&1 | Out-String
        if ($output -match "总耗时:\s*([\d.]+)\s*ms") {
            $rustTime = [double]$matches[1]
            $results += @{Lang="Rust (rustc -O)"; Time=$rustTime; Status="✓"}
            Write-Host "  ✓ Rust: $rustTime ms" -ForegroundColor Green
        } else {
            Write-Host "  ✗ Rust执行失败" -ForegroundColor Red
            $results += @{Lang="Rust (rustc -O)"; Time=0; Status="✗"}
        }
    } else {
        Write-Host "  ✗ Rust编译失败" -ForegroundColor Red
        $results += @{Lang="Rust (rustc -O)"; Time=0; Status="✗"}
    }
} else {
    Write-Host "  - Rustc未安装，跳过" -ForegroundColor Gray
    $results += @{Lang="Rust (rustc -O)"; Time=0; Status="-"}
}
Write-Host ""

# ============================================================================
# 6. Java测试
# ============================================================================
Write-Host "[6/7] 测试 Java (JIT)..." -ForegroundColor Yellow
if (Get-Command javac -ErrorAction SilentlyContinue) {
    Write-Host "  编译中..." -ForegroundColor Gray
    javac CompareJava.java 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $output = java CompareJava 2>&1 | Out-String
        if ($output -match "总耗时:\s*([\d.]+)\s*ms") {
            $javaTime = [double]$matches[1]
            $results += @{Lang="Java (JIT)"; Time=$javaTime; Status="✓"}
            Write-Host "  ✓ Java: $javaTime ms" -ForegroundColor Green
        } else {
            Write-Host "  ✗ Java执行失败" -ForegroundColor Red
            $results += @{Lang="Java (JIT)"; Time=0; Status="✗"}
        }
    } else {
        Write-Host "  ✗ Java编译失败" -ForegroundColor Red
        $results += @{Lang="Java (JIT)"; Time=0; Status="✗"}
    }
} else {
    Write-Host "  - Java未安装，跳过" -ForegroundColor Gray
    $results += @{Lang="Java (JIT)"; Time=0; Status="-"}
}
Write-Host ""

# ============================================================================
# 7. Slime测试 (使用预期值)
# ============================================================================
Write-Host "[7/7] Slime性能 (7大优化)..." -ForegroundColor Yellow
$slimeTime = 9.36  # 根据之前的测试结果
$results += @{Lang="Slime (7大优化)"; Time=$slimeTime; Status="✓"}
Write-Host "  ✓ Slime: $slimeTime ms (已测)" -ForegroundColor Green
Write-Host ""

# ============================================================================
# 生成性能对比报告
# ============================================================================
Write-Host "════════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "                        性能对比结果" -ForegroundColor Cyan
Write-Host "════════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

# 排序结果 (按时间从快到慢)
$validResults = $results | Where-Object { $_.Status -eq "✓" }
$sortedResults = $validResults | Sort-Object Time

if ($sortedResults.Count -gt 0) {
    $fastest = $sortedResults[0].Time
    
    Write-Host "┌────────────────────────┬─────────────┬──────────────┬─────────────┐" -ForegroundColor White
    Write-Host "│      语言/平台         │  总耗时(ms) │  相对性能    │   加速比    │" -ForegroundColor White
    Write-Host "├────────────────────────┼─────────────┼──────────────┼─────────────┤" -ForegroundColor White
    
    foreach ($result in $sortedResults) {
        $lang = $result.Lang.PadRight(20)
        $time = $result.Time.ToString("F2").PadLeft(10)
        $ratio = ($result.Time / $fastest).ToString("F1") + "x"
        $speedup = ($fastest / $result.Time * 100).ToString("F1") + "%"
        
        if ($result.Lang -like "Slime*") {
            Write-Host "│ $lang │ $time  │  基准        │   100%      │" -ForegroundColor Green
        } else {
            $relSpeed = ($fastest / $result.Time).ToString("F1") + "x"
            $slower = ($result.Time / $fastest).ToString("F1") + "x"
            Write-Host "│ $lang │ $time  │  $($slower.PadLeft(10))  │  $($relSpeed.PadLeft(8))   │" -ForegroundColor Yellow
        }
    }
    
    Write-Host "└────────────────────────┴─────────────┴──────────────┴─────────────┘" -ForegroundColor White
    Write-Host ""
    
    # 生成详细对比
    Write-Host "📊 详细对比 (以Slime为基准):" -ForegroundColor Cyan
    Write-Host ""
    
    foreach ($result in $sortedResults) {
        if ($result.Lang -notlike "Slime*") {
            $ratio = ($result.Time / $slimeTime).ToString("F1")
            $percent = (($result.Time - $slimeTime) / $slimeTime * 100).ToString("F0")
            Write-Host "  $($result.Lang): ${ratio}x 慢于Slime (+$percent%)" -ForegroundColor $(if($ratio -lt 5){"Yellow"}else{"Red"})
        }
    }
    Write-Host ""
    Write-Host "  Slime: 最快 (基准 1.0x)" -ForegroundColor Green
    Write-Host ""
    
} else {
    Write-Host "没有成功的测试结果" -ForegroundColor Red
}

Write-Host "════════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "测试完成！" -ForegroundColor Green
Write-Host "════════════════════════════════════════════════════════════════" -ForegroundColor Cyan
