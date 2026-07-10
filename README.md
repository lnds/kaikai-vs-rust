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

- **Verified with `rustc 1.92.0` and `kaikai 0.99.3` (hanga-roa, stage 2).**
  kaikai has two backends, a default `native` (LLVM) and `--backend=c`; both are
  exercised where a result could differ. Building this repo on 0.94 surfaced
  three compiler bugs (filed as lnds/kaikai#995–997); **all three are fixed as
  of 0.98** and stay fixed on 0.99, and the affected cases (3 and 4) have been
  re-verified. See the history notes in those cases.
- **Rust uses edition 2024** (the latest, stabilised in Rust 1.85) — not an
  older edition, so the comparison is against current Rust. Every result was
  re-checked under `--edition 2024` and is unchanged from 2021.
- Every Rust "this requires `unsafe`/`Rc`/`Arc`" claim is backed by the
  **actual compiler error** from the safe version, reproduced in the case
  README. Reproduce it yourself: `cd <case>/rust && rustc --edition 2024
  <file>.rs`.
- Every kaikai example **compiles and runs**: `cd <case>/kaikai && kai run
  <file>.kai`. The pasted output is the real program output.
- Cases 4 and 6 carry a **`## Performance`** section with measured wall-clock
  and peak RSS at N=1M+ (Rust `-O` vs kaikai `--release`), not just op counts.
  Benchmarking these surfaced a family of native-backend leaks
  ([#1048](https://github.com/lnds/kaikai/issues/1048)/[#1112](https://github.com/lnds/kaikai/issues/1112)/[#1126](https://github.com/lnds/kaikai/issues/1126))
  and a value-record proposal that shipped as `Vec[T]`; **all fixed/landed as of
  0.99.3**, re-verified by re-running. The numbers use the **default native
  backend**; see the per-case notes.
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
| 4 | [tree-rewrite-reuse](cases/04-tree-rewrite-reuse/) | kaikai (ergonomics) | Perceus reuses nodes in place when unique (`reuse_in_place=5`, no `Rc`/`make_mut`) and copies correctly when shared — both backends, on 0.99. (On 0.94 the native backend corrupted the shared case; that bug is fixed. On 0.99 native codegen also caught up to C in throughput.) |
| 5 | [effects-vs-boilerplate](cases/05-effects-vs-boilerplate/) | kaikai (ergonomics) | `async` is a viral colour in Rust; a kaikai effect row is discharged by a handler. Capability injection without rewriting signatures. |
| 6 | [flat-layout-no-rc](cases/06-flat-layout-no-rc/) | **Rust** (wall only) | Bulk flat records. The default `[Point]` is a linked list under Perceus RC; the opt-out `Vec[Point]` (value vector, unboxed inline, in-place when unique) now matches Rust's memory model — **0.06 s / 155 MB vs Rust's 0.01 s / 154 MB at N=10M**. Rust still wins the absolute wall (~6×, mostly LLVM vectorizing); it no longer wins the layout/RC story. |

> Case numbering skips 2: the self-referential candidate was discarded (see
> below). Each case folder has its own `README.md`, a `rust/` directory (the
> safe version that fails **and** the version Rust forces you to write), and a
> `kaikai/` directory (the `.kai` that compiles and runs).

Note the balance: case 6 still goes to **Rust**, but now only on the absolute
wall-clock — its `Vec[T]` value vector closed the memory gap to parity, so the
"Rust wins the layout" headline is gone. Case 3 tilts to **Rust** on
parallelism (though safety there is now a wash — both reject the naive attempt
at compile time). Cases 1, 4, and 5 are kaikai ergonomics wins. That is the
point — this is not a sales sheet, and the verdicts move as the compiler
improves (case 4 was a Rust win on 0.94, a kaikai win from 0.98; case 6 went
from a clean Rust layout win to a Rust wall-only win once `Vec[T]` shipped).

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

**All three are fixed as of kaikai 0.98** and stay fixed on 0.99, and re-running
the suite flipped the verdicts accordingly: case 4 became a clean kaikai win
(native now copies correctly on the shared path), and case 3's kaikai side went
from a runtime segfault to a clean compile-time rejection that mirrors Rust's
E0499. A repo of asserted claims would have been wrong twice — once by shipping
the corruption as a win, once by never noticing it was fixed. Running everything
is the methodology; the verdicts are a snapshot of the compiler as it stands,
and they move.

Then benchmarking case 6 surfaced more bugs, and following them across releases
is the clearest example of the methodology. On 0.98 the native backend leaked
record values ([#1048](https://github.com/lnds/kaikai/issues/1048)): 10M
short-lived records took 768 MB on native versus 1 MB on `--backend=c`. Chasing
it across versions: on 0.99 the leak narrowed to boxed values *retained in an
`Array`* ([#1112](https://github.com/lnds/kaikai/issues/1112)); 0.99.1 fixed the
fill path but transiently regressed the *read* path
([#1126](https://github.com/lnds/kaikai/issues/1126)). **On 0.99.3 the whole
family is fixed** — measured, not asserted: the `Array[Point]` fill+fold at
N=100k now frees correctly (`leaked=4`, was `leaked≈N`). The candidate "arrays
of records are slow" finding resolved into exactly this — a retained-`Array`
leak plus ordinary per-access reference counting, both since addressed.

More: the same benchmarking motivated a value-record proposal
([#1121](https://github.com/lnds/kaikai/issues/1121)) that shipped as
[`Vec[T]`](https://github.com/lnds/kaikai/issues/1135) — a flat unboxed value
vector that flips case 6 from a Rust *layout* win to a Rust *wall-only* win. So
case 6's verdict itself moved under the compiler, which is the whole point of
running every claim.

> On the compiler moving under you: the GitHub API once reported #1048 `CLOSED`
> while its comments showed it reopened (fix landed, reverted, deferred). The
> lesson held — trust the measured behavior and the issue discussion, not the
> bare `state` field. Re-checked on 0.99.3, every bug this repo filed is now
> genuinely closed, confirmed by re-running.
