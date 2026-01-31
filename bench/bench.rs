// Rust 性能测试
fn main() {
    let mut sum: i64 = 0;
    for i in 0..1000000 {
        sum += i;
    }
    println!("{}", sum);
}
