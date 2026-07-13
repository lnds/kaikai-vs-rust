// The counter under real CPU load: 8 threads each grind a pure sum, then
// take the Mutex once to add their partial. Load comes from argv so -O
// cannot constant-fold the grind.

use std::env;
use std::hint::black_box;
use std::sync::{Arc, Mutex};
use std::thread;

fn grind(load: i64) -> i64 {
    let mut acc = 0;
    let mut i = load;
    while i > 0 {
        acc += i % 7;
        i -= 1;
    }
    acc
}

fn main() {
    let load: i64 = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300_000_000);
    let counter = Arc::new(Mutex::new(0i64));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            let partial = grind(black_box(load));
            *c.lock().unwrap() += partial;
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    println!("counter={}", black_box(*counter.lock().unwrap()));
}
