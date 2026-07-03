# Case 3 — one counter mutated by N concurrent tasks

**Category B.** Five tasks each add their number to a single shared counter.
Expected total: `1+2+3+4+5 = 15`.

## The claim

> Sharing one `&mut counter` across several scoped threads is rejected by
> the borrow checker (E0499): two mutable borrows of the same cell may not
> be live at once. The safe way is `Arc<Mutex<i32>>` — atomic reference
> counting plus a lock, taken and released on every increment.
>
> **kaikai rejects the same mistake, statically, too.** Capturing a shared
> `var` (State cell) in a spawned fiber is a compile error: *"a closure that
> accesses the mutable cell `counter` escapes its block."* Both languages
> refuse concurrent shared mutation at compile time and push you to a safe
> model — Rust to `Arc<Mutex>`, kaikai to an **actor** (message-passing, no
> shared memory). The one asymmetry that remains: Rust's `Arc<Mutex>` runs on
> real OS threads across cores; kaikai's actor serialises on one fiber. So Rust
> wins on *parallelism*, but no longer on *safety* — that part is now a wash.

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

`rustc --edition 2024 rust/shared_state_fails.rs`:

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

## kaikai — state as an actor (message-passing, not shared memory)

kaikai's idiomatic answer is **not** shared memory at all: the counter lives
in one actor's private heap, and "tasks" send it `Add` messages. The actor
processes its mailbox one message at a time, so the writes serialise without
any lock. [`kaikai/shared_state_actor.kai`](kaikai/shared_state_actor.kai) —
compiles and runs (native backend), prints `counter=15`:

```kaikai
import actor
import spawn

type Msg = Add(Int) | Stop

fn counter(total: Int) : Int / Actor[Msg] + Stdout = {
  match Actor.receive() {
    Add(n) -> counter(total + n)
    Stop   -> total
  }
}

fn main() : Int / Spawn + Stdout = with_mailbox {
  Actor.send(Actor.self(), Add(1))
  Actor.send(Actor.self(), Add(2))
  Actor.send(Actor.self(), Add(3))
  Actor.send(Actor.self(), Add(4))
  Actor.send(Actor.self(), Add(5))
  Actor.send(Actor.self(), Stop)
  Stdout.print("counter=#{int_to_string(counter(0))}")
  0
}
```

No `Arc`, no `Mutex`, no `clone`. Isolation is by message-passing (BEAM-style);
the actor's private heap is never aliased.

### kaikai — the idiomatic safe attempt also fails (statically)

The most direct translation of the Rust version is a shared `var counter`
captured by fibers spawned in a `nursery` — the exact analogue of the Rust
`thread::scope` + `&mut` attempt above.
[`kaikai/shared_state_fibers_REJECTED.kai`](kaikai/shared_state_fibers_REJECTED.kai):

```kaikai
fn main() : Int / Spawn + Stdout = {
  var counter := 0
  nursery { n ->
    let fibers = [1, 2, 3, 4, 5] | (x => n.spawn(() => {
      counter := counter + x       # capture a shared var in a spawned fiber
      ()
    }))
    fibers | (f => n.await(f))
  }
  Stdout.print("counter=#{int_to_string(counter)}")
  0
}
```

`kai build shared_state_fibers_REJECTED.kai` (both backends):

```text
error: a closure that accesses the mutable cell `counter` escapes its block
  --> shared_state_fibers_REJECTED.kai:6:50
   |
 6 |     let fibers = [1, 2, 3, 4, 5] | (x => n.spawn(() => {
   |                                                  ^
  = help: the handler for `counter` is fiber-local: it is torn down when its
    block exits and does not cross a `spawn`; a `var`/State/Reader cell or a
    capability bound by `as` cannot outlive its `handle` — pass its value down
    as a call argument, or model shared mutable state across fibers as an actor
```

This is the direct counterpart of Rust's E0499: kaikai statically refuses to
let a mutable cell be shared across the fiber boundary, and the diagnostic even
names the fix (an actor). On kaikai 0.94 this same program instead **compiled
and segfaulted** ([lnds/kaikai#997](https://github.com/lnds/kaikai/issues/997));
0.98 turned the runtime crash into this compile-time rejection — the safety
story is now static on both sides.

## Reproduce

```sh
cd rust
rustc --edition 2024 shared_state_fails.rs                      # E0499
rustc --edition 2024 shared_state_arc_works.rs -o ss && ./ss    # counter=15
cd ../kaikai
kai run shared_state_actor.kai                                  # counter=15
kai build shared_state_fibers_REJECTED.kai                      # compile error (escapes its block)
```

## Where Rust wins / the trade-off

Safety is now symmetric — both languages reject shared mutable state across
concurrency at compile time. The remaining asymmetry is parallelism:

- **Rust's `Arc<Mutex>` buys real multi-core parallelism.** Those five threads
  run on different cores; the `Mutex` makes the shared write safe *under true
  preemptive parallelism*. When you genuinely need N cores hammering one piece
  of shared memory, this is doing strictly more than kaikai's actor.
- **kaikai's actor serialises by construction.** No data race because the actor
  processes one message at a time on its private heap — but that is
  single-threaded for this counter, so it does **not** give the CPU-parallel
  speedup the Rust version can. Same answer (`15`), different execution model.
- **On safety and ergonomics they are now even.** Both reject the naive shared
  mutation statically; kaikai's actor is arguably lighter to write than
  `Arc<Mutex>` + `lock().unwrap()` + `Arc::clone`, while Rust's version, once it
  compiles, gives real parallelism. Pick by whether you need message-passing
  concurrency (kaikai reads cleaner) or shared-memory parallelism across cores
  (Rust is the only one that delivers it here).
