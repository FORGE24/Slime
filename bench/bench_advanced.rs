// Rust 高级性能测试：与 bench_advanced.sm 对齐的算法

fn heavy_sum_loop(n: i64) -> i64 {
    let mut s = 0i64;
    let mut i = 0i64;
    while i < n {
        if i % 2 == 0 {
            s += i;
        } else {
            s += i - 1;
        }
        i += 1;
    }
    s
}

fn heavy_fact_loop(n: i64) -> i64 {
    let mut acc = 1i64;
    let mut i = 1i64;
    while i <= n {
        if i % 2 == 0 {
            acc *= i + 1;
        } else {
            acc *= i;
        }
        i += 1;
    }
    acc
}

fn fib_iter(n: i64) -> i64 {
    if n <= 1 { return n; }
    let mut a = 0i64;
    let mut b = 1i64;
    let mut i = 2i64;
    while i <= n {
        let next = a + b;
        a = b;
        b = next;
        i += 1;
    }
    b
}

fn main() {
    let n_large = 1_000_000i64;
    let n_fact  = 12i64;
    let n_fib   = 40i64;

    let r1 = heavy_sum_loop(n_large);
    let r2 = heavy_fact_loop(n_fact);
    let r3 = fib_iter(n_fib);

    println!("[runtime] heavy_sum_loop  = {}", r1);
    println!("[runtime] heavy_fact_loop = {}", r2);
    println!("[runtime] fib_iter        = {}", r3);
}
