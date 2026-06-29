# kaikai vs Rust — an adversarial, honest comparison

This repo confronts Rust. It is written for a skeptical Rust expert who will
try to refute every claim. So every claim here is **proven by compiling
code**, not asserted. For each "Rust needs `unsafe` / `Rc` / `Arc` here"
claim there is a real `rustc` error pasted in; for each "kaikai is more
direct here" claim there is a `.kai` program that compiles and runs, with
its real output pasted in.

If the safe Rust version actually compiles, the claim is **false** and it
does not get published as a win — it goes to [`discarded/`](discarded/)
instead, because conceding a refuted claim is more valuable than a fragile
one.

## Disclaimer / methodology

- **Verified with `rustc 1.92.0` and `kaikai 0.93.0` (hanga-roa, stage 2).**
  The task brief referenced kaikai 0.94; the toolchain actually installed
  on this machine is 0.93.0, and that is what every example was run against.
- Every Rust "this requires `unsafe`/`Rc`/`Arc`" claim is backed by the
  **actual compiler error** from the safe version, reproduced in the case
  README. Reproduce it yourself: `cd <case>/rust && rustc --edition 2021
  <file>.rs`.
- Every kaikai example **compiles and runs**: `cd <case>/kaikai && kai run
  <file>.kai`. The pasted output is the real program output.
- The Rust shown is the version an **expert would write** — the idiomatic
  safe attempt first. Only when that genuinely fails to compile is the
  `Rc`/`Arc`/`unsafe` workaround presented. No strawmen.
- This is **not propaganda**. Every case has a *"Where Rust wins / the
  trade-off"* section. The borrow checker is a real, zero-runtime-cost
  safety system; Perceus reference counting has a small but non-zero
  runtime cost the borrow checker does not. Both are stated plainly.

## What the two categories mean

- **Category A — Rust needs `unsafe` or `Rc`/`RefCell`/`Arc`/`Pin`
  gymnastics**, where kaikai is more direct. The honest claim is almost
  never "Rust *cannot*". It is "the safe, idiomatic reach (plain `&`
  references) is rejected by the borrow checker, and the working Rust
  forces runtime machinery (`Rc<RefCell>`, `Weak`, `Pin`+`unsafe`)".
- **Category B — kaikai is simpler because of Perceus (compile-time RC, no
  borrow checker) and algebraic effects**, where Rust reaches for
  `Arc<Mutex>` / trait objects / `?`-threading.

## Index

| # | Case | Category | Claim (one line) |
|---|------|----------|------------------|
| 1 | [cyclic-graph](cases/01-cyclic-graph/) | A | A two-node pointer cycle: safe `&` refs are rejected (E0506); Rust forces `Rc<RefCell>`+`Weak`. |
| 2 | [self-referential](cases/02-self-referential/) | A | A struct holding a reference into its own field: rejected (E0597/E0505); Rust forces `Pin`+`unsafe` or indices. |
| 3 | [shared-mutable-state](cases/03-shared-mutable-state/) | B | One counter mutated by N concurrent tasks: safe `&mut` is rejected (E0499); Rust forces `Arc<Mutex>`. |
| 4 | [tree-rewrite-reuse](cases/04-tree-rewrite-reuse/) | B | Updating a shared immutable tree: Rust forces `Rc::make_mut`/clone; Perceus reuses transparently. |
| 5 | [effects-vs-boilerplate](cases/05-effects-vs-boilerplate/) | B | Injectable IO + audit + failure: kaikai's effect row vs Rust's generics/`dyn` + `Result`/`?`. |

> Each case folder has its own `README.md`, a `rust/` directory (the safe
> version that fails **and** the version Rust forces you to write), and a
> `kaikai/` directory (the `.kai` that compiles and runs).

## Discarded cases

Candidate claims that turned out **false** under compilation (safe Rust does
the thing) are documented, not deleted, in [`discarded/`](discarded/). That
honesty is part of the point.

## Bottom line up front

The borrow checker buys Rust a real thing kaikai does not have: aliasing
discipline checked at compile time with **zero** runtime cost, and real
multi-core shared-memory parallelism. What it costs is expressivity: data
shapes with cycles, shared mutation, or self-reference do not fit the
"single owner + borrows" model, so they get pushed onto `Rc`/`RefCell`/
`Arc`/`Pin` — which is runtime reference counting and locking, plus
ceremony. kaikai pays a small, uniform Perceus RC cost everywhere and in
exchange lets you write those shapes directly. Different bet, honestly
stated, case by case below.
