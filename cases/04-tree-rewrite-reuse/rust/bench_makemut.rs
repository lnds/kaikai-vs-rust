use std::rc::Rc;
use std::hint::black_box;

#[derive(Clone)]
enum Tree { Leaf(i64), Node(Rc<Tree>, Rc<Tree>) }

fn build(d: i64) -> Rc<Tree> {
    if d <= 0 { Rc::new(Tree::Leaf(1)) }
    else { Rc::new(Tree::Node(build(d - 1), build(d - 1))) }
}

fn inc_leaves(t: &mut Rc<Tree>) {
    match Rc::make_mut(t) {
        Tree::Leaf(n) => *n += 1,
        Tree::Node(l, r) => { inc_leaves(l); inc_leaves(r); }
    }
}

fn sum(t: &Rc<Tree>) -> i64 {
    match &**t { Tree::Leaf(n) => *n, Tree::Node(l, r) => sum(l) + sum(r) }
}

fn main() {
    let d: i64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let mut t = build(black_box(d));   // unique owner: strong_count == 1 everywhere
    inc_leaves(&mut t);                // make_mut reuses in place
    println!("sum={}", black_box(sum(&t)));
}
