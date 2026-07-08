use std::hint::black_box;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point { x: i64, y: i64 }

fn sum_x(xs: &[Point]) -> i64 { xs.iter().map(|p| p.x).sum() }
fn build(n: i64) -> Vec<Point> {
    (1..=n).rev().map(|k| Point { x: k, y: k * 2 }).collect()
}

fn main() {
    let n: i64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let pts = build(black_box(n));
    let total = sum_x(black_box(&pts));
    println!("total={}", black_box(total));
}
