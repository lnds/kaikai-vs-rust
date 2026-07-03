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

- **Verified with `rustc 1.92.0` and `kaikai 0.98.0` (hanga-roa, stage 2).**
  kaikai has two backends, a default `native` (LLVM) and `--backend=c`; both are
  exercised where a result could differ. Building this repo on 0.94 surfaced
  three compiler bugs (filed as lnds/kaikai#995–997); **all three are fixed as
  of 0.98**, and the affected cases (3 and 4) have been re-verified and
  rewritten. See the history notes in those cases.
- **Rust uses edition 2024** (the latest, stabilised in Rust 1.85) — not an
  older edition, so the comparison is against current Rust. Every result was
  re-checked under `--edition 2024` and is unchanged from 2021.
- Every Rust "this requires `unsafe`/`Rc`/`Arc`" claim is backed by the
  **actual compiler error** from the safe version, reproduced in the case
  README. Reproduce it yourself: `cd <case>/rust && rustc --edition 2024
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
| 3 | [shared-mutable-state](cases/03-shared-mutable-state/) | even → Rust (parallelism) | One counter mutated by N tasks: **both** reject shared mutation at compile time (Rust E0499; kaikai "mutable cell escapes its block"), pushing you to `Arc<Mutex>` / an actor. Rust still wins on real multi-core; kaikai's actor is serial. |
| 4 | [tree-rewrite-reuse](cases/04-tree-rewrite-reuse/) | kaikai (ergonomics) | Perceus reuses nodes in place when unique (`reuse_in_place=5`, no `Rc`/`make_mut`) and copies correctly when shared — both backends, on 0.98. (On 0.94 the native backend corrupted the shared case; that bug is fixed.) |
| 5 | [effects-vs-boilerplate](cases/05-effects-vs-boilerplate/) | kaikai (ergonomics) | `async` is a viral colour in Rust; a kaikai effect row is discharged by a handler. Capability injection without rewriting signatures. |
| 6 | [flat-layout-no-rc](cases/06-flat-layout-no-rc/) | **Rust** | Bulk flat records: `Vec<Point>` is one allocation, zero RC; kaikai pays ~10k RC ops + ~3k allocations on a linked list. |

> Case numbering skips 2: the self-referential candidate was discarded (see
> below). Each case folder has its own `README.md`, a `rust/` directory (the
> safe version that fails **and** the version Rust forces you to write), and a
> `kaikai/` directory (the `.kai` that compiles and runs).

Note the balance: case 6 is an outright **Rust win**, and case 3 tilts to
**Rust** on parallelism (though safety there is now a wash — both reject the
naive attempt at compile time). Cases 1, 4, and 5 are kaikai ergonomics wins.
That is the point — this is not a sales sheet, and the verdicts move as the
compiler improves (case 4 was a Rust win on 0.94, a kaikai win on 0.98).

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

The clearest demonstration of why every claim is run, not asserted, is the
history of case 4. On kaikai 0.94, trying to build the strongest *pro-kaikai*
case (Perceus in-place reuse) surfaced the opposite: the native backend
**silently corrupted shared data** during reuse — the exact bug the borrow
checker exists to prevent. That made case 4 a partial *Rust* win, and it was
reported as such. Building the comparison this way surfaced three reproducible
kaikai 0.94 bugs, all filed upstream:
[#995](https://github.com/lnds/kaikai/issues/995) (native aliasing corruption),
[#996](https://github.com/lnds/kaikai/issues/996) (C-backend `redefinition`
for a `var` named `x`/`state`/`log`), and
[#997](https://github.com/lnds/kaikai/issues/997) (SIGSEGV when a spawned fiber
captures a `var`).

**All three are fixed as of kaikai 0.98**, and re-running the suite flipped the
verdicts accordingly: case 4 became a clean kaikai win (native now copies
correctly on the shared path), and case 3's kaikai side went from a runtime
segfault to a clean compile-time rejection that mirrors Rust's E0499. A repo of
asserted claims would have been wrong twice — once by shipping the corruption as
a win, once by never noticing it was fixed. Running everything is the
methodology; the verdicts are a snapshot of the compiler as it stands, and they
move.
