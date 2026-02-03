/**
 * C++对比版本 - 标准实现
 * 编译: g++ -O3 -std=c++17 compare_cpp.cpp -o compare_cpp.exe
 */

#include <iostream>
#include <chrono>
#include <vector>
#include <cstdint>

using namespace std;
using namespace std::chrono;

// 测试1: 斐波那契（递归实现）
int64_t fib_runtime(int n) {
    if (n <= 1) return n;
    return fib_runtime(n - 1) + fib_runtime(n - 2);
}

// 测试2: 收敛计算
double convergence_demo() {
    double sum = 0.0;
    double prev = 0.0;
    
    for (int i = 0; i < 1000000; i++) {
        prev = sum;
        sum = sum + 1.0 / ((i + 1) * (i + 1));
    }
    
    return sum;
}

// 测试3: 双重循环
int temporal_collapse(int iterations) {
    int result = 0;
    
    for (int t1 = 0; t1 < iterations; t1++) {
        for (int t2 = 0; t2 < iterations; t2++) {
            if ((t1 * t2) % 7 == 0) {
                result++;
            }
        }
    }
    
    return result;
}

// 测试4: 多任务
int64_t compute_heavy(int n) {
    int64_t sum = 0;
    for (int i = 0; i < n; i++) {
        sum += i * i;
    }
    return sum;
}

int64_t parallel_tasks() {
    int64_t task1 = compute_heavy(100);
    int64_t task2 = compute_heavy(200);
    int64_t task3 = compute_heavy(300);
    return task1 + task2 + task3;
}

// 测试5: 并发求和
int64_t sum_range(int start, int end) {
    int64_t sum = 0;
    for (int i = start; i < end; i++) {
        sum += i;
    }
    return sum;
}

int64_t concurrent_sum(int n) {
    int64_t part1 = sum_range(0, n / 4);
    int64_t part2 = sum_range(n / 4, n / 2);
    int64_t part3 = sum_range(n / 2, n * 3 / 4);
    int64_t part4 = sum_range(n * 3 / 4, n);
    return part1 + part2 + part3 + part4;
}

// 测试6: 矩阵计算
int64_t matrix_multiply_snippet() {
    int64_t result = 0;
    
    for (int i = 0; i < 1000; i++) {
        int temp = i * 3 + 5;
        result += temp * temp;
    }
    
    return result;
}

// 测试7: 多项式求值
double polynomial_eval(double x, const vector<double>& coeffs) {
    double result = coeffs[0];
    double power = 1.0;
    
    for (size_t i = 1; i < 5; i++) {
        power = power * x;
        result = result + coeffs[i] * power;
    }
    
    return result;
}

int main() {
    cout << "=== C++版本 (g++ -O3优化) ===\n\n";
    
    double total_time = 0;
    
    // 测试1: 斐波那契
    cout << "【1】斐波那契计算 (递归实现):\n";
    auto start = high_resolution_clock::now();
    int64_t fib_result = fib_runtime(30);
    auto end = high_resolution_clock::now();
    double elapsed = duration<double, milli>(end - start).count();
    total_time += elapsed;
    cout << "  fib(30) = " << fib_result << "\n";
    cout << "  耗时: " << elapsed << " ms\n\n";
    
    // 测试2: 收敛计算
    cout << "【2】收敛计算 (100万次迭代):\n";
    start = high_resolution_clock::now();
    double conv_result = convergence_demo();
    end = high_resolution_clock::now();
    elapsed = duration<double, milli>(end - start).count();
    total_time += elapsed;
    cout << "  结果 = " << conv_result << "\n";
    cout << "  耗时: " << elapsed << " ms\n\n";
    
    // 测试3: 双重循环
    cout << "【3】双重循环 (100x100):\n";
    start = high_resolution_clock::now();
    int tce_result = temporal_collapse(100);
    end = high_resolution_clock::now();
    elapsed = duration<double, milli>(end - start).count();
    total_time += elapsed;
    cout << "  结果 = " << tce_result << "\n";
    cout << "  耗时: " << elapsed << " ms\n\n";
    
    // 测试4: 多任务
    cout << "【4】多任务计算 (顺序执行):\n";
    start = high_resolution_clock::now();
    int64_t sched_result = parallel_tasks();
    end = high_resolution_clock::now();
    elapsed = duration<double, milli>(end - start).count();
    total_time += elapsed;
    cout << "  结果 = " << sched_result << "\n";
    cout << "  耗时: " << elapsed << " ms\n\n";
    
    // 测试5: 并发求和
    cout << "【5】并发求和 (10000):\n";
    start = high_resolution_clock::now();
    int64_t prefold_result = concurrent_sum(10000);
    end = high_resolution_clock::now();
    elapsed = duration<double, milli>(end - start).count();
    total_time += elapsed;
    cout << "  结果 = " << prefold_result << "\n";
    cout << "  耗时: " << elapsed << " ms\n\n";
    
    // 测试6: 矩阵计算
    cout << "【6】矩阵计算 (1000次):\n";
    start = high_resolution_clock::now();
    int64_t ifm_result = matrix_multiply_snippet();
    end = high_resolution_clock::now();
    elapsed = duration<double, milli>(end - start).count();
    total_time += elapsed;
    cout << "  结果 = " << ifm_result << "\n";
    cout << "  耗时: " << elapsed << " ms\n\n";
    
    // 测试7: 多项式求值
    cout << "【7】多项式求值:\n";
    start = high_resolution_clock::now();
    vector<double> coeffs = {1, 2, 3, 4, 5};
    double dope_result = polynomial_eval(10.0, coeffs);
    end = high_resolution_clock::now();
    elapsed = duration<double, milli>(end - start).count();
    total_time += elapsed;
    cout << "  结果 = " << dope_result << "\n";
    cout << "  耗时: " << elapsed << " ms\n\n";
    
    cout << "=== 总耗时: " << total_time << " ms ===\n";
    
    return 0;
}
