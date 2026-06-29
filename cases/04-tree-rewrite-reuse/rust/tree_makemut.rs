// Rust's in-place reuse for uniquely-owned Rc data: Rc::make_mut clones
// ONLY if the Rc is shared (strong_count > 1), otherwise mutates in place.
// This is the safe, explicit equivalent of what Perceus does automatically.
// Crucially: when the data IS shared, make_mut CLONES, so the original is
// never corrupted. Safety is preserved by construction.
use std::rc::Rc;

#[derive(Clone)]
enum Tree {
    Leaf(i32),
    Node(Rc<Tree>, Rc<Tree>),
}

fn inc_leaves(t: &mut Rc<Tree>) {
    match Rc::make_mut(t) {
        Tree::Leaf(n) => *n += 1,
        Tree::Node(l, r) => {
            inc_leaves(l);
            inc_leaves(r);
        }
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

    // Case A: unique owner. Clone the Rc handle to a fresh variable but
    // drop the alias before mutating -> make_mut reuses in place.
    let mut t_unique = Rc::clone(&t);
    drop(t); // now t_unique is the sole owner
    inc_leaves(&mut t_unique);
    println!("unique new={}", sum(&t_unique)); // 9, reused in place

    // Case B: shared. Keep the original alive while deriving a new version.
    let original = Rc::new(Tree::Node(
        Rc::new(Tree::Leaf(10)),
        Rc::new(Tree::Leaf(20)),
    ));
    let mut derived = Rc::clone(&original); // shared: strong_count == 2
    inc_leaves(&mut derived);               // make_mut CLONES the shared spine
    println!("shared original={} derived={}", sum(&original), sum(&derived));
    // original stays 30, derived is 32 — NO corruption.
}
