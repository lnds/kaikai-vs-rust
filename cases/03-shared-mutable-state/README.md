# Case 3 — one counter mutated by N concurrent tasks

**Category B.** Five tasks each add their number to a single shared counter.
Expected total: `1+2+3+4+5 = 15`.

## The claim

> Sharing one `&mut counter` across several scoped threads is rejected by
> the borrow checker (E0499): two mutable borrows of the same cell may not
> be live at once. The safe way is `Arc<Mutex<i32>>` — atomic reference
> counting plus a lock, taken and released on every increment. kaikai writes
> `var counter := 0` and mutates it from fibers spawned in a nursery; the
> `State` handler **serialises the writes by construction**, so there is no
> data race to guard against — not because the programmer was disciplined,
> but because cooperative fibers never interleave a read-modify-write.

## Rust — the idiomatic safe attempt fails

`thread::scope` is exactly the tool meant to let threads borrow locals, so a
Rust programmer reaches for it with `&mut`. [`rust/shared_state_fails.rs`](rust/shared_state_fails.rs):

```rust
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
```

`rustc --edition 2021 rust/shared_state_fails.rs`:

```text
error[E0499]: cannot borrow `counter` as mutable more than once at a time
   --> rust/shared_state_fails.rs:10:21
    |
  8 |       thread::scope(|s| {
    |                      - has type `&'1 Scope<'1, '_>`
  9 |           for x in 1..=5 {
 10 |               s.spawn(|| {
    |               -       ^^ `counter` was mutably borrowed here in the previous iteration of the loop
 11 | |                 counter += x; // every thread mutates the SAME counter
    | |                 ------- borrows occur due to use of `counter` in closure
```

This is the borrow checker doing its job: concurrent `&mut` to one cell is a
data race, and it refuses to compile it. Correct and valuable.

## Rust — what you are forced to write

[`rust/shared_state_arc_works.rs`](rust/shared_state_arc_works.rs) — compiles
and runs, prints `counter=15`:

```rust
use std::sync::{Arc, Mutex};
use std::thread;

let counter = Arc::new(Mutex::new(0));
for x in 1..=5 {
    let c = Arc::clone(&counter);
    thread::spawn(move || { *c.lock().unwrap() += x; });
}
// join all, then read *counter.lock().unwrap()
```

`Arc` for shared ownership across threads, `Mutex` for exclusive access,
`lock().unwrap()` at every touch, `Arc::clone` per task.

## kaikai — a shared `var` across fibers

[`kaikai/shared_state.kai`](kaikai/shared_state.kai) — compiles and runs,
prints `counter=15`:

```kaikai
import spawn

fn main() : Int / Spawn + Stdout = {
  var counter := 0
  nursery { n ->
    let fibers = [1, 2, 3, 4, 5] | (x => n.spawn(() => {
      counter := counter + x
      ()
    }))
    fibers | (f => n.await(f))
  }
  Stdout.print("counter=#{int_to_string(counter)}")
  0
}
```

No `Arc`, no `Mutex`, no `clone`. The `var` desugars to a `State[Int]`
handler; reads and writes route through it.

## Reproduce

```sh
cd rust
rustc --edition 2021 shared_state_fails.rs                 # E0499
rustc --edition 2021 shared_state_arc_works.rs -o ss && ./ss   # counter=15
cd ../kaikai
kai run shared_state.kai                                   # counter=15
```

## Where Rust wins / the trade-off

This is the case where Rust wins on something kaikai cannot match, and it
must be stated plainly:

- **Rust's `Arc<Mutex>` buys real multi-core parallelism.** Those five
  threads run on different cores; the `Mutex` makes the shared write safe
  *under true preemptive parallelism*. The result is correct because the
  lock enforces it.
- **kaikai's fibers are cooperative and single-threaded here.** There is no
  data race because the writes never actually run in parallel — the `State`
  handler serialises them. That is why `var counter` is safe without a lock,
  but it is also why this kaikai program does **not** give you CPU-parallel
  speedup the way the Rust version can. Same answer (`15`), different
  execution model.
- So the honest summary: kaikai is **simpler to write** for shared mutable
  state, and **safe by construction** for that simplicity — but the safety
  comes from *not* running the increments in parallel. When you genuinely
  need N cores hammering shared memory, Rust's `Arc<Mutex>` (or atomics) is
  doing strictly more, and the ceremony is the price of that power.
