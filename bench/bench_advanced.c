// C 高级性能测试：与 bench_advanced.sm 对齐的算法
#include <stdio.h>

static long long heavy_sum_loop(int n) {
    long long s = 0;
    int i = 0;
    while (i < n) {
        if (i % 2 == 0)
            s += i;
        else
            s += i - 1;
        i++;
    }
    return s;
}

static long long heavy_fact_loop(int n) {
    long long acc = 1;
    int i = 1;
    while (i <= n) {
        if (i % 2 == 0)
            acc *= (i + 1);
        else
            acc *= i;
        i++;
    }
    return acc;
}

static long long fib_iter(int n) {
    if (n <= 1) return n;
    long long a = 0, b = 1;
    for (int i = 2; i <= n; ++i) {
        long long next = a + b;
        a = b;
        b = next;
    }
    return b;
}

int main(void) {
    const int n_large = 1000000;
    const int n_fact  = 12;
    const int n_fib   = 40;

    long long r1 = heavy_sum_loop(n_large);
    long long r2 = heavy_fact_loop(n_fact);
    long long r3 = fib_iter(n_fib);

    printf("[runtime] heavy_sum_loop  = %lld\n", r1);
    printf("[runtime] heavy_fact_loop = %lld\n", r2);
    printf("[runtime] fib_iter        = %lld\n", r3);
    return 0;
}
