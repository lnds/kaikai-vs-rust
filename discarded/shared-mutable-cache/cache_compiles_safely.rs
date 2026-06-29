// Candidate claim: "a mutable cache shared between several callbacks needs
// unsafe in Rust." This is FALSE. Rc<RefCell<HashMap>> is safe and idiomatic,
// and several closures can share and mutate it. Compiles and runs.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn main() {
    let cache: Rc<RefCell<HashMap<String, i32>>> = Rc::new(RefCell::new(HashMap::new()));

    // two callbacks, each holding a clone of the shared handle
    let writer = {
        let c = Rc::clone(&cache);
        move |k: &str, v: i32| {
            c.borrow_mut().insert(k.to_string(), v);
        }
    };
    let reader = {
        let c = Rc::clone(&cache);
        move |k: &str| -> Option<i32> { c.borrow().get(k).copied() }
    };

    writer("a", 1);
    writer("b", 2);
    println!("a={:?} b={:?} miss={:?}", reader("a"), reader("b"), reader("z"));
}
