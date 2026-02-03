#!/usr/bin/env python3
"""
Python对比版本 - 无Slime优化技术
用于性能对比测试
"""

import time
import sys

# 测试1: 斐波那契（无CTFE优化）
def fib_runtime(n):
    """运行时计算斐波那契 - 无编译时优化"""
    if n <= 1:
        return n
    return fib_runtime(n - 1) + fib_runtime(n - 2)

# 测试2: 收敛计算（无值收敛检测）
def convergence_demo():
    """完整执行100万次迭代 - 无收敛检测"""
    sum_val = 0.0
    prev = 0.0
    
    for i in range(1000000):
        prev = sum_val
        sum_val = sum_val + 1.0 / ((i + 1) * (i + 1))
    
    return sum_val

# 测试3: 双重循环（无时间折叠）
def temporal_collapse(iterations):
    """标准双重循环 - 无时间折叠优化"""
    result = 0
    
    for t1 in range(iterations):
        for t2 in range(iterations):
            if (t1 * t2) % 7 == 0:
                result += 1
    
    return result

# 测试4: 多任务（有Python GIL开销）
def compute_heavy(n):
    sum_val = 0
    for i in range(n):
        sum_val += i * i
    return sum_val

def parallel_tasks():
    """串行执行 - 无调度器消除"""
    task1 = compute_heavy(100)
    task2 = compute_heavy(200)
    task3 = compute_heavy(300)
    return task1 + task2 + task3

# 测试5: 并发求和（无预折叠）
def sum_range(start, end):
    sum_val = 0
    for i in range(start, end):
        sum_val += i
    return sum_val

def concurrent_sum(n):
    """串行计算 - 无预并发折叠"""
    part1 = sum_range(0, n // 4)
    part2 = sum_range(n // 4, n // 2)
    part3 = sum_range(n // 2, n * 3 // 4)
    part4 = sum_range(n * 3 // 4, n)
    return part1 + part2 + part3 + part4

# 测试6: 矩阵计算（无指令记忆化）
def matrix_multiply_snippet():
    """每次都重新计算 - 无IFM优化"""
    result = 0
    
    for i in range(1000):
        temp = i * 3 + 5
        result += temp * temp
    
    return result

# 测试7: 多项式求值（无部分求值）
def polynomial_eval(x, coeffs):
    """运行时完整计算 - 无DOPE优化"""
    result = coeffs[0]
    power = 1
    
    for i in range(1, 5):
        power = power * x
        result = result + coeffs[i] * power
    
    return result

# ============================================================================
# 主测试函数
# ============================================================================

def main():
    print("=== Python版本 (无Slime优化) ===\n")
    
    total_time = 0
    
    # 测试1: 斐波那契
    print("【1】斐波那契计算 (运行时计算):")
    start = time.perf_counter()
    fib_result = fib_runtime(30)
    elapsed = time.perf_counter() - start
    total_time += elapsed
    print(f"  fib(30) = {fib_result}")
    print(f"  耗时: {elapsed*1000:.3f} ms\n")
    
    # 测试2: 收敛计算
    print("【2】收敛计算 (完整100万次迭代):")
    start = time.perf_counter()
    conv_result = convergence_demo()
    elapsed = time.perf_counter() - start
    total_time += elapsed
    print(f"  结果 = {conv_result:.6f}")
    print(f"  耗时: {elapsed*1000:.3f} ms\n")
    
    # 测试3: 双重循环
    print("【3】双重循环 (标准实现):")
    start = time.perf_counter()
    tce_result = temporal_collapse(100)
    elapsed = time.perf_counter() - start
    total_time += elapsed
    print(f"  结果 = {tce_result}")
    print(f"  耗时: {elapsed*1000:.3f} ms\n")
    
    # 测试4: 多任务
    print("【4】多任务计算 (串行执行):")
    start = time.perf_counter()
    sched_result = parallel_tasks()
    elapsed = time.perf_counter() - start
    total_time += elapsed
    print(f"  结果 = {sched_result}")
    print(f"  耗时: {elapsed*1000:.3f} ms\n")
    
    # 测试5: 并发求和
    print("【5】并发求和 (无预折叠):")
    start = time.perf_counter()
    prefold_result = concurrent_sum(10000)
    elapsed = time.perf_counter() - start
    total_time += elapsed
    print(f"  结果 = {prefold_result}")
    print(f"  耗时: {elapsed*1000:.3f} ms\n")
    
    # 测试6: 矩阵计算
    print("【6】矩阵计算 (无记忆化):")
    start = time.perf_counter()
    ifm_result = matrix_multiply_snippet()
    elapsed = time.perf_counter() - start
    total_time += elapsed
    print(f"  结果 = {ifm_result}")
    print(f"  耗时: {elapsed*1000:.3f} ms\n")
    
    # 测试7: 多项式求值
    print("【7】多项式求值 (运行时计算):")
    start = time.perf_counter()
    coeffs = [1, 2, 3, 4, 5]
    dope_result = polynomial_eval(10, coeffs)
    elapsed = time.perf_counter() - start
    total_time += elapsed
    print(f"  结果 = {dope_result}")
    print(f"  耗时: {elapsed*1000:.3f} ms\n")
    
    print(f"=== 总耗时: {total_time*1000:.3f} ms ===")
    
    return 0

if __name__ == "__main__":
    sys.exit(main())
