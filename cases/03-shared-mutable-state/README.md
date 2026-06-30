# Case 3 — one counter mutated by N concurrent tasks

**Category B.** Five tasks each add their number to a single shared counter.
Expected total: `1+2+3+4+5 = 15`.

## The claim

> Sharing one `&mut counter` across several scoped threads is rejected by
> the borrow checker (E0499): two mutable borrows of the same cell may not
> be live at once. The safe way is `Arc<Mutex<i32>>` — atomic reference
> counting plus a lock, taken and released on every increment. kaikai's
> idiomatic answer avoids shared memory entirely: the counter lives in an
> **actor**, and tasks send it messages; the actor serialises them on its
> private mailbox, no lock needed.
>
> Two honest caveats, both shown below: kaikai's actor is single-threaded
> for this counter (no real parallelism, where Rust gets it), and the *most
> direct* translation — a shared `var` mutated from fibers — **segfaults on
> kaikai 0.94**. So this case is more mixed than a clean kaikai win.

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

### A simpler `var` form crashes on 0.94 — reported honestly

The most direct translation of the Rust version is a shared `var counter`
captured by fibers spawned in a `nursery`. It is kept as
[`kaikai/shared_state_fibers_CRASHES.kai`](kaikai/shared_state_fibers_CRASHES.kai)
because on **kaikai 0.94 it compiles but segfaults at runtime** (deterministic,
both `--backend=native` and `--backend=c`):

```text
$ kai run shared_state_fibers_CRASHES.kai
.../kai: line ...: Segmentation fault: 11
```

Capturing a `State`-effect `var` inside a spawned fiber is the trigger
(reading it is enough; `await` is not required); a `nursery` with no shared
`var` runs fine, and the same logic as an actor (above) runs fine. This is a
0.94 runtime bug, reported upstream as
[lnds/kaikai#997](https://github.com/lnds/kaikai/issues/997) — not the intended
way to write it (the actor is the idiom), but a repo built for skeptics reports
what the shipped compiler does, crashes included.

## Reproduce

```sh
cd rust
rustc --edition 2024 shared_state_fails.rs                      # E0499
rustc --edition 2024 shared_state_arc_works.rs -o ss && ./ss    # counter=15
cd ../kaikai
kai run shared_state_actor.kai                                  # counter=15
kai run shared_state_fibers_CRASHES.kai                         # SIGSEGV on 0.94
```

## Where Rust wins / the trade-off

This is the case where Rust wins on something kaikai cannot match, and it
must be stated plainly:

- **Rust's `Arc<Mutex>` buys real multi-core parallelism.** Those five
  threads run on different cores; the `Mutex` makes the shared write safe
  *under true preemptive parallelism*. The result is correct because the
  lock enforces it.
- **kaikai's actor serialises by construction.** There is no data race
  because the actor processes one message at a time on its private heap — but
  that is also single-threaded for this counter, so it does **not** give you
  the CPU-parallel speedup the Rust version can. Same answer (`15`), different
  execution model.
- **And the most direct kaikai form does not even run on 0.94** (the fiber +
  shared `var` segfault above). Rust's version compiles once it satisfies the
  borrow checker and then runs, every time. On reliability of the obvious
  translation, Rust clearly wins here today.
- Honest summary: the *actor* model is genuinely simpler to write than
  `Arc<Mutex>` and safe by message-passing, but it trades away real
  parallelism, and kaikai 0.94's shared-`var`-over-fibers path is currently
  broken. The borrow checker, by contrast, is doing strictly more work and
  the ceremony is the price of safe, real parallelism that actually runs.
