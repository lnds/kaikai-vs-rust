// A flat, contiguous Vec<Point>. Point is Copy and #[repr(C)]: the values
// live inline in the Vec's buffer, there is no Rc/Arc anywhere, and the
// whole buffer is freed in one shot when `pts` drops. Zero reference
// counting, zero per-element bookkeeping — the borrow checker proved at
// compile time that none is needed.
#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: i64,
    y: i64,
}

fn sum_x(xs: &[Point]) -> i64 {
    xs.iter().map(|p| p.x).sum()
}

fn build(n: i64) -> Vec<Point> {
    (1..=n).rev().map(|k| Point { x: k, y: k * 2 }).collect()
}

fn main() {
    let pts = build(1000);
    let total = sum_x(&pts);
    println!("total={}", total);
    // exactly one heap allocation (the Vec buffer); freed once at scope end.
    println!("size_of::<Point>() = {} bytes, contiguous", std::mem::size_of::<Point>());
}
