// C++ 高级性能测试：与 bench_advanced.sm 对齐的算法
#include <iostream>

long long heavy_sum_loop(int n) {
    long long s = 0;
    int i = 0;
    while (i < n) {
        if (i % 2 == 0)
            s += i;
        else
            s += i - 1;
        ++i;
    }
    return s;
}

long long heavy_fact_loop(int n) {
    long long acc = 1;
    int i = 1;
    while (i <= n) {
        if (i % 2 == 0)
            acc *= (i + 1);
        else
            acc *= i;
        ++i;
    }
    return acc;
}

long long fib_iter(int n) {
    if (n <= 1) return n;
    long long a = 0, b = 1;
    for (int i = 2; i <= n; ++i) {
        long long next = a + b;
        a = b;
        b = next;
    }
    return b;
}

int main() {
    const int n_large = 1'000'000;
    const int n_fact  = 12;
    const int n_fib   = 40;

    auto r1 = heavy_sum_loop(n_large);
    auto r2 = heavy_fact_loop(n_fact);
    auto r3 = fib_iter(n_fib);

    std::cout << "[runtime] heavy_sum_loop  = " << r1 << '\n';
    std::cout << "[runtime] heavy_fact_loop = " << r2 << '\n';
    std::cout << "[runtime] fib_iter        = " << r3 << '\n';
}
