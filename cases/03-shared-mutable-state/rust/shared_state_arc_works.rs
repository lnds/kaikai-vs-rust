// The safe way: Arc<Mutex<T>> shared across threads.
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let counter = Arc::new(Mutex::new(0));
    let mut handles = vec![];
    for x in 1..=5 {
        let c = Arc::clone(&counter);
        let h = thread::spawn(move || {
            let mut guard = c.lock().unwrap();
            *guard += x;
        });
        handles.push(h);
    }
    for h in handles {
        h.join().unwrap();
    }
    println!("counter={}", *counter.lock().unwrap());
}
