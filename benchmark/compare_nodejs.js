#!/usr/bin/env node
/**
 * JavaScript/Node.js对比版本 - 无Slime优化技术
 * 用于性能对比测试
 */

// 测试1: 斐波那契（无CTFE优化）
function fibRuntime(n) {
    if (n <= 1) return n;
    return fibRuntime(n - 1) + fibRuntime(n - 2);
}

// 测试2: 收敛计算（无值收敛检测）
function convergenceDemo() {
    let sum = 0.0;
    let prev = 0.0;
    
    for (let i = 0; i < 1000000; i++) {
        prev = sum;
        sum = sum + 1.0 / ((i + 1) * (i + 1));
    }
    
    return sum;
}

// 测试3: 双重循环（无时间折叠）
function temporalCollapse(iterations) {
    let result = 0;
    
    for (let t1 = 0; t1 < iterations; t1++) {
        for (let t2 = 0; t2 < iterations; t2++) {
            if ((t1 * t2) % 7 === 0) {
                result++;
            }
        }
    }
    
    return result;
}

// 测试4: 多任务（单线程执行）
function computeHeavy(n) {
    let sum = 0;
    for (let i = 0; i < n; i++) {
        sum += i * i;
    }
    return sum;
}

function parallelTasks() {
    const task1 = computeHeavy(100);
    const task2 = computeHeavy(200);
    const task3 = computeHeavy(300);
    return task1 + task2 + task3;
}

// 测试5: 并发求和（无预折叠）
function sumRange(start, end) {
    let sum = 0;
    for (let i = start; i < end; i++) {
        sum += i;
    }
    return sum;
}

function concurrentSum(n) {
    const part1 = sumRange(0, Math.floor(n / 4));
    const part2 = sumRange(Math.floor(n / 4), Math.floor(n / 2));
    const part3 = sumRange(Math.floor(n / 2), Math.floor(n * 3 / 4));
    const part4 = sumRange(Math.floor(n * 3 / 4), n);
    return part1 + part2 + part3 + part4;
}

// 测试6: 矩阵计算（无指令记忆化）
function matrixMultiplySnippet() {
    let result = 0;
    
    for (let i = 0; i < 1000; i++) {
        const temp = i * 3 + 5;
        result += temp * temp;
    }
    
    return result;
}

// 测试7: 多项式求值（无部分求值）
function polynomialEval(x, coeffs) {
    let result = coeffs[0];
    let power = 1;
    
    for (let i = 1; i < 5; i++) {
        power = power * x;
        result = result + coeffs[i] * power;
    }
    
    return result;
}

// ============================================================================
// 主测试函数
// ============================================================================

function main() {
    console.log("=== JavaScript/Node.js版本 (无Slime优化) ===\n");
    
    let totalTime = 0;
    
    // 测试1: 斐波那契
    console.log("【1】斐波那契计算 (运行时计算):");
    let start = process.hrtime.bigint();
    const fibResult = fibRuntime(30);
    let elapsed = Number(process.hrtime.bigint() - start) / 1000000;
    totalTime += elapsed;
    console.log(`  fib(30) = ${fibResult}`);
    console.log(`  耗时: ${elapsed.toFixed(3)} ms\n`);
    
    // 测试2: 收敛计算
    console.log("【2】收敛计算 (完整100万次迭代):");
    start = process.hrtime.bigint();
    const convResult = convergenceDemo();
    elapsed = Number(process.hrtime.bigint() - start) / 1000000;
    totalTime += elapsed;
    console.log(`  结果 = ${convResult.toFixed(6)}`);
    console.log(`  耗时: ${elapsed.toFixed(3)} ms\n`);
    
    // 测试3: 双重循环
    console.log("【3】双重循环 (标准实现):");
    start = process.hrtime.bigint();
    const tceResult = temporalCollapse(100);
    elapsed = Number(process.hrtime.bigint() - start) / 1000000;
    totalTime += elapsed;
    console.log(`  结果 = ${tceResult}`);
    console.log(`  耗时: ${elapsed.toFixed(3)} ms\n`);
    
    // 测试4: 多任务
    console.log("【4】多任务计算 (串行执行):");
    start = process.hrtime.bigint();
    const schedResult = parallelTasks();
    elapsed = Number(process.hrtime.bigint() - start) / 1000000;
    totalTime += elapsed;
    console.log(`  结果 = ${schedResult}`);
    console.log(`  耗时: ${elapsed.toFixed(3)} ms\n`);
    
    // 测试5: 并发求和
    console.log("【5】并发求和 (无预折叠):");
    start = process.hrtime.bigint();
    const prefoldResult = concurrentSum(10000);
    elapsed = Number(process.hrtime.bigint() - start) / 1000000;
    totalTime += elapsed;
    console.log(`  结果 = ${prefoldResult}`);
    console.log(`  耗时: ${elapsed.toFixed(3)} ms\n`);
    
    // 测试6: 矩阵计算
    console.log("【6】矩阵计算 (无记忆化):");
    start = process.hrtime.bigint();
    const ifmResult = matrixMultiplySnippet();
    elapsed = Number(process.hrtime.bigint() - start) / 1000000;
    totalTime += elapsed;
    console.log(`  结果 = ${ifmResult}`);
    console.log(`  耗时: ${elapsed.toFixed(3)} ms\n`);
    
    // 测试7: 多项式求值
    console.log("【7】多项式求值 (运行时计算):");
    start = process.hrtime.bigint();
    const coeffs = [1, 2, 3, 4, 5];
    const dopeResult = polynomialEval(10, coeffs);
    elapsed = Number(process.hrtime.bigint() - start) / 1000000;
    totalTime += elapsed;
    console.log(`  结果 = ${dopeResult}`);
    console.log(`  耗时: ${elapsed.toFixed(3)} ms\n`);
    
    console.log(`=== 总耗时: ${totalTime.toFixed(3)} ms ===`);
    
    return 0;
}

// 运行主函数
main();
