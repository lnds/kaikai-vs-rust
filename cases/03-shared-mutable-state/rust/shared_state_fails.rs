// Attempt: mutate ONE shared counter from several scoped threads,
// safe, without Arc/Mutex. Use a scope so threads can borrow locals,
// and &mut so they share the same cell rather than copying it.
use std::thread;

fn main() {
    let mut counter = 0;
    thread::scope(|s| {
        for x in 1..=5 {
            s.spawn(|| {
                counter += x; // every thread mutates the SAME counter
            });
        }
    });
    println!("counter={}", counter);
}
