// A persistent tree shared via Rc. We want a NEW version with every leaf
// incremented, while old versions that still alias subtrees keep working.
// Safe Rust: you cannot mutate through Rc (no DerefMut). You must rebuild,
// and to even *read*-share you carried Rc. Rebuild clones the spine.
use std::rc::Rc;

#[derive(Debug)]
enum Tree {
    Leaf(i32),
    Node(Rc<Tree>, Rc<Tree>),
}

fn inc_leaves(t: &Rc<Tree>) -> Rc<Tree> {
    match &**t {
        Tree::Leaf(n) => Rc::new(Tree::Leaf(n + 1)),
        Tree::Node(l, r) => Rc::new(Tree::Node(inc_leaves(l), inc_leaves(r))),
    }
}

fn sum(t: &Rc<Tree>) -> i32 {
    match &**t {
        Tree::Leaf(n) => *n,
        Tree::Node(l, r) => sum(l) + sum(r),
    }
}

fn main() {
    let t = Rc::new(Tree::Node(
        Rc::new(Tree::Node(Rc::new(Tree::Leaf(1)), Rc::new(Tree::Leaf(2)))),
        Rc::new(Tree::Leaf(3)),
    ));
    let t2 = inc_leaves(&t);
    println!("orig={} inc={}", sum(&t), sum(&t2));
}
