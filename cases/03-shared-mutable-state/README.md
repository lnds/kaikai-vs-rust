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
> shared memory). The one asymmetry that remained through 0.99.6: Rust's
> `Arc<Mutex>` runs on real OS threads across cores; kaikai's actor serialised
> on one scheduler thread. **On 0.99.11 that asymmetry closed on wall-clock**:
> the M:N work-stealing scheduler runs actors across real cores
> (`KAI_THREADS=N`), at wall parity with Rust threads on a CPU-bound bench.
> 0.99.11 paid ~150 MiB of runtime tables per scheduler thread for it; **0.99.12
> fixed that** ([#1212](https://github.com/lnds/kaikai/issues/1212) /
> [PR #1216](https://github.com/lnds/kaikai/pull/1216)) — the same bench is now
> at RSS parity too (measured below).

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

use Actor    # open the effect's ops so send/receive/self resolve by bare name

type Msg = Add(Int) | Stop

fn counter(total: Int) : Int / Actor[Msg] + Stdout = {
  match receive() {
    Add(n) -> counter(total + n)
    Stop   -> total
  }
}

fn main() : Int / Spawn + Stdout = with_mailbox {
  send(self(), Add(1))
  send(self(), Add(2))
  send(self(), Add(3))
  send(self(), Add(4))
  send(self(), Add(5))
  send(self(), Stop)
  Stdout.print("counter=#{int_to_string(counter(0))}")
  0
}
```

No `Arc`, no `Mutex`, no `clone`. Isolation is by message-passing (BEAM-style);
the actor's private heap is never aliased. (`use Actor` opens the `Actor`
effect's ops; without it they read as `Actor.send` / `Actor.receive` /
`Actor.self` — still one effect, spelled out.)

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

## kaikai 0.99.11+ — real multi-core via `KAI_THREADS` (measured)

kaikai **0.99.11** shipped F1 of the M:N work-stealing scheduler
([#1207](https://github.com/lnds/kaikai/issues/1207) /
[PR #1211](https://github.com/lnds/kaikai/pull/1211)): fibers and actors run
on `KAI_THREADS=N` OS threads (default 1), messages are **physically copied**
on a cross-thread send so the non-atomic Perceus RC survives, and the language
surface does not change — no `Send`/`Sync`, no new annotation; the same actor
code from above just runs on more cores.

[`kaikai/parallel_counter.kai`](kaikai/parallel_counter.kai) puts the counter
under real CPU load: 8 worker actors each grind a pure 300M-iteration sum and
send their partial to the counter.
[`rust/parallel_counter.rs`](rust/parallel_counter.rs) is the same shape with
`thread::spawn` + `Arc<Mutex>`. Both print `counter=7200000024` — kaikai's
total is identical at every `KAI_THREADS`, which is the determinism the copy
buys.

Measured on an Apple M4 Pro (14 cores), min of 6 warm runs, `--release` / `-O`,
on **kaikai 0.99.12**:

| Config | wall | speedup | peak RSS |
|---|---|---|---|
| kaikai, `KAI_THREADS=1` (default) | 1.11 s | 1× | 2.2 MiB |
| kaikai, `KAI_THREADS=4` | 0.27 s | 4.1× | 2.4 MiB |
| kaikai, `KAI_THREADS=8` | 0.14 s | 7.9× | 2.4 MiB |
| Rust, 8 × `thread::spawn` | 0.14 s | — | 1.7 MiB |

The C backend matches native (0.14 s at `KAI_THREADS=8`).

On memory: 0.99.11 materialised ~150 MiB of thread-local RC pool tables **per
scheduler thread** — this same bench read 722 MiB at `KAI_THREADS=8`, ~425×
Rust's RSS, and a hello-world was 161.6 MiB resident. Filed as
[#1212](https://github.com/lnds/kaikai/issues/1212); **fixed on 0.99.12**
([PR #1216](https://github.com/lnds/kaikai/pull/1216) lazy-materialises the
pools) and re-measured: hello-world is back to ~1.7 MiB, and this bench is
2.4 MiB at 8 threads — RSS parity with Rust, at every thread count.

One honest caveat remains:

- **Parallelism is opt-in.** The default is `KAI_THREADS=1` (byte-identical
  with the old scheduler); the flip to N=cores is a planned upstream phase.

There is also a structural asymmetry the numbers don't show: cross-thread
sends **copy**. Partial sums are cheap to copy; a large shared structure is
not. Rust's `Arc<Mutex>` shares one allocation across cores; kaikai's model
deliberately never shares, so "N cores hammering one big piece of memory"
remains a shape only Rust expresses.

## Reproduce

```sh
cd rust
rustc --edition 2024 shared_state_fails.rs                      # E0499
rustc --edition 2024 shared_state_arc_works.rs -o ss && ./ss    # counter=15
cd ../kaikai
kai run shared_state_actor.kai                                  # counter=15
kai build shared_state_fibers_REJECTED.kai                      # compile error (escapes its block)

# parallelism (kaikai 0.99.11+)
kai build --release kaikai/parallel_counter.kai -o /tmp/pc
time /tmp/pc                                                    # KAI_THREADS=1
KAI_THREADS=8 sh -c 'time /tmp/pc'                              # ~8× faster, same total
rustc --edition 2024 -O rust/parallel_counter.rs -o /tmp/pr
time /tmp/pr 300000000
```

## Where Rust wins / the trade-off

Safety is symmetric — both languages reject shared mutable state across
concurrency at compile time. As of 0.99.12, wall-clock **and** resident memory
are symmetric too (measured above). What remains:

- **Rust's parallelism is on by default.** kaikai's is opt-in
  (`KAI_THREADS=N`, default 1); until the default flips, the out-of-the-box
  program runs on one core.
- **Rust can share; kaikai copies.** `Arc<Mutex>` gives N cores one allocation;
  kaikai's cross-thread sends deep-copy the message. For partial results that
  is free; for a large shared structure, Rust expresses a shape kaikai
  deliberately does not.
- **On safety and ergonomics they are even.** Both reject the naive shared
  mutation statically; kaikai's actor is lighter to write than `Arc<Mutex>` +
  `lock().unwrap()` + `Arc::clone`, keeps one execution model from 1 to N
  threads, and its total is deterministic at every N. Pick by whether your
  problem is message-shaped (kaikai reads cleaner, now scales too) or genuinely
  shared-memory (Rust is still the only one that expresses it).
