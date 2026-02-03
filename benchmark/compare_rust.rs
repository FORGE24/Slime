/**
 * Rust对比版本 - 标准实现
 * 编译: rustc -O compare_rust.rs
 */

use std::time::Instant;

// 测试1: 斐波那契（递归实现）
fn fib_runtime(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    fib_runtime(n - 1) + fib_runtime(n - 2)
}

// 测试2: 收敛计算
fn convergence_demo() -> f64 {
    let mut sum = 0.0;
    let mut _prev = 0.0;
    
    for i in 0..1000000 {
        _prev = sum;
        sum = sum + 1.0 / ((i + 1) * (i + 1)) as f64;
    }
    
    sum
}

// 测试3: 双重循环
fn temporal_collapse(iterations: i32) -> i32 {
    let mut result = 0;
    
    for t1 in 0..iterations {
        for t2 in 0..iterations {
            if (t1 * t2) % 7 == 0 {
                result += 1;
            }
        }
    }
    
    result
}

// 测试4: 多任务
fn compute_heavy(n: i32) -> i64 {
    let mut sum = 0i64;
    for i in 0..n {
        sum += (i * i) as i64;
    }
    sum
}

fn parallel_tasks() -> i64 {
    let task1 = compute_heavy(100);
    let task2 = compute_heavy(200);
    let task3 = compute_heavy(300);
    task1 + task2 + task3
}

// 测试5: 并发求和
fn sum_range(start: i32, end: i32) -> i64 {
    let mut sum = 0i64;
    for i in start..end {
        sum += i as i64;
    }
    sum
}

fn concurrent_sum(n: i32) -> i64 {
    let part1 = sum_range(0, n / 4);
    let part2 = sum_range(n / 4, n / 2);
    let part3 = sum_range(n / 2, n * 3 / 4);
    let part4 = sum_range(n * 3 / 4, n);
    part1 + part2 + part3 + part4
}

// 测试6: 矩阵计算
fn matrix_multiply_snippet() -> i64 {
    let mut result = 0i64;
    
    for i in 0..1000 {
        let temp = i * 3 + 5;
        result += temp * temp;
    }
    
    result
}

// 测试7: 多项式求值
fn polynomial_eval(x: f64, coeffs: &[f64]) -> f64 {
    let mut result = coeffs[0];
    let mut power = 1.0;
    
    for i in 1..5 {
        power = power * x;
        result = result + coeffs[i] * power;
    }
    
    result
}

fn main() {
    println!("=== Rust版本 (rustc -O优化) ===\n");
    
    let mut total_time = 0.0;
    
    // 测试1: 斐波那契
    println!("【1】斐波那契计算 (递归实现):");
    let start = Instant::now();
    let fib_result = fib_runtime(30);
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    total_time += elapsed;
    println!("  fib(30) = {}", fib_result);
    println!("  耗时: {:.3} ms\n", elapsed);
    
    // 测试2: 收敛计算
    println!("【2】收敛计算 (100万次迭代):");
    let start = Instant::now();
    let conv_result = convergence_demo();
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    total_time += elapsed;
    println!("  结果 = {:.6}", conv_result);
    println!("  耗时: {:.3} ms\n", elapsed);
    
    // 测试3: 双重循环
    println!("【3】双重循环 (100x100):");
    let start = Instant::now();
    let tce_result = temporal_collapse(100);
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    total_time += elapsed;
    println!("  结果 = {}", tce_result);
    println!("  耗时: {:.3} ms\n", elapsed);
    
    // 测试4: 多任务
    println!("【4】多任务计算 (顺序执行):");
    let start = Instant::now();
    let sched_result = parallel_tasks();
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    total_time += elapsed;
    println!("  结果 = {}", sched_result);
    println!("  耗时: {:.3} ms\n", elapsed);
    
    // 测试5: 并发求和
    println!("【5】并发求和 (10000):");
    let start = Instant::now();
    let prefold_result = concurrent_sum(10000);
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    total_time += elapsed;
    println!("  结果 = {}", prefold_result);
    println!("  耗时: {:.3} ms\n", elapsed);
    
    // 测试6: 矩阵计算
    println!("【6】矩阵计算 (1000次):");
    let start = Instant::now();
    let ifm_result = matrix_multiply_snippet();
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    total_time += elapsed;
    println!("  结果 = {}", ifm_result);
    println!("  耗时: {:.3} ms\n", elapsed);
    
    // 测试7: 多项式求值
    println!("【7】多项式求值:");
    let start = Instant::now();
    let coeffs = [1.0, 2.0, 3.0, 4.0, 5.0];
    let dope_result = polynomial_eval(10.0, &coeffs);
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    total_time += elapsed;
    println!("  结果 = {}", dope_result);
    println!("  耗时: {:.3} ms\n", elapsed);
    
    println!("=== 总耗时: {:.3} ms ===", total_time);
}
