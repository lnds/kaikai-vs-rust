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

| # | Case | Who wins | Claim (one line) |
|---|------|----------|------------------|
| 1 | [cyclic-graph](cases/01-cyclic-graph/) | kaikai (ergonomics) | A two-node pointer cycle: safe `&` refs are rejected (E0506); Rust forces `Rc<RefCell>`+`Weak`, kaikai uses the `Mutable` effect. |
| 3 | [shared-mutable-state](cases/03-shared-mutable-state/) | kaikai (ergonomics) / Rust (parallelism) | One counter mutated by N tasks: safe `&mut` is rejected (E0499); Rust forces `Arc<Mutex>` but gets real multi-core; kaikai uses a shared `var`. |
| 4 | [tree-rewrite-reuse](cases/04-tree-rewrite-reuse/) | **split** | Unique-owner rewrite: Perceus reuses in place for free (kaikai win). Shared rewrite: kaikai 0.93 **corrupts** the original (Rust win on correctness). |
| 5 | [effects-vs-boilerplate](cases/05-effects-vs-boilerplate/) | kaikai (ergonomics) | `async` is a viral colour in Rust; a kaikai effect row is discharged by a handler. Capability injection without rewriting signatures. |
| 6 | [flat-layout-no-rc](cases/06-flat-layout-no-rc/) | **Rust** | Bulk flat records: `Vec<Point>` is one allocation, zero RC; kaikai pays ~10k RC ops + ~3k allocations on a linked list. |

> Case numbering skips 2: the self-referential candidate was discarded (see
> below). Each case folder has its own `README.md`, a `rust/` directory (the
> safe version that fails **and** the version Rust forces you to write), and a
> `kaikai/` directory (the `.kai` that compiles and runs).

Note that two of the six cases are **Rust wins** (6 outright, 4 on the shared
path) and a third (3) concedes Rust's parallelism. That balance is the point.

## Discarded cases

Candidate claims that turned out **false** under compilation (safe Rust does
the thing), documented not deleted, in [`discarded/`](discarded/):

- **self-referential struct** — Rust fails (E0597/E0505), but kaikai has no
  interior references, so the problem cannot even be expressed; not a win.
- **shared mutable cache** — the "needs `unsafe`" claim is false;
  `Rc<RefCell<HashMap>>` is safe and compiles.

That honesty is part of the point.

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

The single most important finding came from trying to build the strongest
*pro-kaikai* case (Perceus in-place reuse, case 4) and discovering by running
it that kaikai 0.93 **silently corrupts shared data** during reuse for
user-defined recursive types — the exact bug the borrow checker exists to
prevent. It is reported in full rather than buried. That is the whole reason
every claim here is compiled and run: an asserted comparison would have shipped
that as a kaikai win; a verified one caught it as a Rust win.
