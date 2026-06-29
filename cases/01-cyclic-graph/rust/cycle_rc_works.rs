// The idiomatic safe way: Rc<RefCell<...>> with Weak to break the cycle.
use std::rc::{Rc, Weak};
use std::cell::RefCell;

struct Node {
    id: i32,
    next: RefCell<Option<Rc<Node>>>,
    prev: RefCell<Weak<Node>>,
}

fn main() {
    let a = Rc::new(Node { id: 1, next: RefCell::new(None), prev: RefCell::new(Weak::new()) });
    let b = Rc::new(Node { id: 2, next: RefCell::new(None), prev: RefCell::new(Weak::new()) });
    *a.next.borrow_mut() = Some(Rc::clone(&b));
    *b.prev.borrow_mut() = Rc::downgrade(&a);
    *b.next.borrow_mut() = Some(Rc::clone(&a)); // a <-> b cycle

    // walk 6 steps
    let mut cur = Rc::clone(&a);
    let mut trace = String::new();
    for _ in 0..6 {
        trace.push_str(&cur.id.to_string());
        trace.push(' ');
        let nxt = cur.next.borrow().as_ref().map(Rc::clone);
        cur = nxt.unwrap();
    }
    println!("trace={}", trace.trim_end());
}
