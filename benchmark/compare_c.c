/**
 * C语言对比版本 - 标准实现
 * 编译: gcc -O3 compare_c.c -o compare_c.exe
 */

#include <stdio.h>
#include <time.h>
#include <stdint.h>

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
double polynomial_eval(double x, double* coeffs) {
    double result = coeffs[0];
    double power = 1.0;
    
    for (int i = 1; i < 5; i++) {
        power = power * x;
        result = result + coeffs[i] * power;
    }
    
    return result;
}

// 计时辅助函数
#ifdef _WIN32
#include <windows.h>
double get_time_ms() {
    LARGE_INTEGER frequency, counter;
    QueryPerformanceFrequency(&frequency);
    QueryPerformanceCounter(&counter);
    return (counter.QuadPart * 1000.0) / frequency.QuadPart;
}
#else
double get_time_ms() {
    struct timespec ts;
    timespec_get(&ts, TIME_UTC);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}
#endif

int main() {
    printf("=== C语言版本 (gcc -O3优化) ===\n\n");
    
    double total_time = 0;
    double start, elapsed;
    
    // 测试1: 斐波那契
    printf("【1】斐波那契计算 (递归实现):\n");
    start = get_time_ms();
    int64_t fib_result = fib_runtime(30);
    elapsed = get_time_ms() - start;
    total_time += elapsed;
    printf("  fib(30) = %lld\n", fib_result);
    printf("  耗时: %.3f ms\n\n", elapsed);
    
    // 测试2: 收敛计算
    printf("【2】收敛计算 (100万次迭代):\n");
    start = get_time_ms();
    double conv_result = convergence_demo();
    elapsed = get_time_ms() - start;
    total_time += elapsed;
    printf("  结果 = %.6f\n", conv_result);
    printf("  耗时: %.3f ms\n\n", elapsed);
    
    // 测试3: 双重循环
    printf("【3】双重循环 (100x100):\n");
    start = get_time_ms();
    int tce_result = temporal_collapse(100);
    elapsed = get_time_ms() - start;
    total_time += elapsed;
    printf("  结果 = %d\n", tce_result);
    printf("  耗时: %.3f ms\n\n", elapsed);
    
    // 测试4: 多任务
    printf("【4】多任务计算 (顺序执行):\n");
    start = get_time_ms();
    int64_t sched_result = parallel_tasks();
    elapsed = get_time_ms() - start;
    total_time += elapsed;
    printf("  结果 = %lld\n", sched_result);
    printf("  耗时: %.3f ms\n\n", elapsed);
    
    // 测试5: 并发求和
    printf("【5】并发求和 (10000):\n");
    start = get_time_ms();
    int64_t prefold_result = concurrent_sum(10000);
    elapsed = get_time_ms() - start;
    total_time += elapsed;
    printf("  结果 = %lld\n", prefold_result);
    printf("  耗时: %.3f ms\n\n", elapsed);
    
    // 测试6: 矩阵计算
    printf("【6】矩阵计算 (1000次):\n");
    start = get_time_ms();
    int64_t ifm_result = matrix_multiply_snippet();
    elapsed = get_time_ms() - start;
    total_time += elapsed;
    printf("  结果 = %lld\n", ifm_result);
    printf("  耗时: %.3f ms\n\n", elapsed);
    
    // 测试7: 多项式求值
    printf("【7】多项式求值:\n");
    start = get_time_ms();
    double coeffs[] = {1, 2, 3, 4, 5};
    double dope_result = polynomial_eval(10.0, coeffs);
    elapsed = get_time_ms() - start;
    total_time += elapsed;
    printf("  结果 = %.0f\n", dope_result);
    printf("  耗时: %.3f ms\n\n", elapsed);
    
    printf("=== 总耗时: %.3f ms ===\n", total_time);
    
    return 0;
}
