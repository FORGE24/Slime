# Python 高级性能测试：与 bench_advanced.sm 对齐的算法


def heavy_sum_loop(n: int) -> int:
    s = 0
    i = 0
    while i < n:
        if i % 2 == 0:
            s += i
        else:
            s += i - 1
        i += 1
    return s


def heavy_fact_loop(n: int) -> int:
    acc = 1
    i = 1
    while i <= n:
        if i % 2 == 0:
            acc *= i + 1
        else:
            acc *= i
        i += 1
    return acc


def fib_iter(n: int) -> int:
    if n <= 1:
        return n
    a, b = 0, 1
    for _ in range(2, n + 1):
        a, b = b, a + b
    return b


if __name__ == "__main__":
    n_large = 1_000_000
    n_fact = 12
    n_fib = 40

    r1 = heavy_sum_loop(n_large)
    r2 = heavy_fact_loop(n_fact)
    r3 = fib_iter(n_fib)

    print(f"[runtime] heavy_sum_loop  = {r1}")
    print(f"[runtime] heavy_fact_loop = {r2}")
    print(f"[runtime] fib_iter        = {r3}")
