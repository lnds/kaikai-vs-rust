// Attempt: two nodes that point at each other with plain &mut references.
// This is what you'd reach for before resorting to Rc<RefCell>.
struct Node<'a> {
    id: i32,
    next: Option<&'a Node<'a>>,
}

fn main() {
    let mut a = Node { id: 1, next: None };
    let mut b = Node { id: 2, next: None };
    a.next = Some(&b);
    b.next = Some(&a); // cycle: a -> b -> a
    println!("{}", a.next.unwrap().id);
}
