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

- **Verified with `rustc 1.92.0` and `kaikai 0.99.11` (hanga-roa, stage 2).**
  kaikai has two backends, a default `native` (LLVM) and `--backend=c`; both are
  exercised where a result could differ. Building this repo on 0.94 surfaced
  three compiler bugs (filed as lnds/kaikai#995–997); **all three are fixed as
  of 0.98** and stay fixed on 0.99, and the affected cases (3 and 4) have been
  re-verified. See the history notes in those cases. Re-verifying on 0.99.5 also
  surfaced a typer regression
  ([#1194](https://github.com/lnds/kaikai/issues/1194)) that briefly blocked
  case 1 from compiling; it was filed with a minimal repro and **fixed on
  0.99.6**, and case 1 runs again. Finding a regression by re-running — and
  seeing it fixed on the next upgrade — is exactly why every claim is re-run.
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
  0.99.3**, re-verified by re-running. A sibling region proposal then shipped as
  `kind Region` on **0.99.5** — see case 6's arena sub-case. The numbers use the
  **default native backend**; see the per-case notes.
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
| 1 | [cyclic-graph](cases/01-cyclic-graph/) | kaikai (ergonomics) | A two-node pointer cycle: safe `&` refs are rejected (E0506); Rust forces `Rc<RefCell>`+`Weak`, kaikai uses the `Mutable` effect. *(A 0.99.5 typer regression, [#1194](https://github.com/lnds/kaikai/issues/1194), briefly blocked it; fixed on 0.99.6 — runs again.)* |
| 3 | [shared-mutable-state](cases/03-shared-mutable-state/) | even (wall) → Rust (memory, default) | One counter mutated by N tasks: **both** reject shared mutation at compile time (Rust E0499; kaikai "mutable cell escapes its block"), pushing you to `Arc<Mutex>` / an actor. On 0.99.11 the M:N scheduler (`KAI_THREADS=8`) reaches **wall parity with Rust threads** on a CPU-bound bench (0.16 s vs 0.14 s, 6.7× self-speedup) — but opt-in, and at ~150 MiB of runtime tables per thread (722 MiB vs Rust's 1.7 MiB). |
| 4 | [tree-rewrite-reuse](cases/04-tree-rewrite-reuse/) | kaikai (ergonomics) | Perceus reuses nodes in place when unique (`reuse_in_place=5`, no `Rc`/`make_mut`) and copies correctly when shared — both backends, on 0.99. (On 0.94 the native backend corrupted the shared case; that bug is fixed. On 0.99 native codegen also caught up to C in throughput.) |
| 5 | [effects-vs-boilerplate](cases/05-effects-vs-boilerplate/) | kaikai (ergonomics) | `async` is a viral colour in Rust; a kaikai effect row is discharged by a handler. Capability injection without rewriting signatures. |
| 6 | [flat-layout-no-rc](cases/06-flat-layout-no-rc/) | **Rust** (wall only) | Bulk flat records. The default `[Point]` is a linked list under Perceus RC; the opt-out `Vec[Point]` (value vector, unboxed inline, in-place when unique) now matches Rust's memory model — **0.06 s / 155 MB vs Rust's 0.01 s / 154 MB at N=10M**. Rust still wins the absolute wall (~6×, mostly LLVM vectorizing); it no longer wins the layout/RC story. |

> Case numbering skips 2: the self-referential candidate was discarded (see
> below). Each case folder has its own `README.md`, a `rust/` directory (the
> safe version that fails **and** the version Rust forces you to write), and a
> `kaikai/` directory (the `.kai` that compiles and runs).

Note the balance: case 6 still goes to **Rust**, but now only on the absolute
wall-clock — its `Vec[T]` value vector closed the memory gap to parity, so the
"Rust wins the layout" headline is gone. Case 3's parallelism asymmetry, a
clean Rust win through 0.99.6, **closed on wall-clock on 0.99.11** when the
M:N scheduler shipped — what Rust keeps there is default-on parallelism at
near-zero resident cost, versus kaikai's opt-in `KAI_THREADS` with a ~150 MiB
per-thread runtime base. Cases 1, 4, and 5 are kaikai ergonomics wins. That is
the point — this is not a sales sheet, and the verdicts move as the compiler
improves (case 4 was a Rust win on 0.94, a kaikai win from 0.98; case 6 went
from a clean Rust layout win to a Rust wall-only win once `Vec[T]` shipped;
case 3 went from "kaikai's actor is serial" to wall parity in one release).

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

That thread has a sequel. `Vec[T]` is contiguous — it covers *flat* bulk data,
not *linked* structures. The follow-up proposal
([#1123](https://github.com/lnds/kaikai/issues/1123), `kind Region`) shipped on
**0.99.5** ([#1192](https://github.com/lnds/kaikai/pull/1192)): a
`region { r -> ... }` block plus region-polymorphic functions over `Tree<r>`
bump-allocate a whole linked tree in an arena with **zero per-node reference
counting**, freed in one shot at the closing brace. Running the shipped fixture
on 0.99.5, a 100-key tree folded to a scalar traces `arena_alloc=5050`,
`incref_total=0` on `--backend=c` — the arena cost model, measured, not
asserted. It is opt-in (you write `region`), and the escape case is handled by
copying out of the arena. On 0.99.5 the arena routing emitted on the C backend
only; **0.99.6** ([#1193](https://github.com/lnds/kaikai/issues/1193)) taught the
native default to route too — at first with an RC-trace divergence between the
backends (filed as [#1197](https://github.com/lnds/kaikai/issues/1197), **fixed
on 0.99.7**: both backends now trace identically, `incref=0`), which is
exactly the kind of thing re-running catches. See
[case 6](cases/06-flat-layout-no-rc/) for the measured sub-case. Two sibling
proposals (`Vec[T]`, `kind Region`) landing across several point releases is the
compiler moving under the comparison in real time.

> On the compiler moving under you: the GitHub API once reported #1048 `CLOSED`
> while its comments showed it reopened (fix landed, reverted, deferred). The
> lesson held — trust the measured behavior and the issue discussion, not the
> bare `state` field. The pattern repeated across 0.99.5 → 0.99.6: re-running on
> 0.99.5 caught a fresh typer regression
> ([#1194](https://github.com/lnds/kaikai/issues/1194)) that blocked case 1;
> **0.99.6 fixed it** (case 1 runs again) — and the same re-run then surfaced an
> RC-trace divergence between the native and C backends on the new region path
> ([#1197](https://github.com/lnds/kaikai/issues/1197), filed, output still
> correct, **fixed on 0.99.7**). The 0.99.11 re-run repeated the pattern once
> more: it confirmed #1197's fix and the new M:N parallelism (case 3), and
> surfaced a fresh finding — the M:N runtime's thread-local pools cost ~150 MiB
> resident per scheduler thread (a hello-world went from ~2 MiB to ~162 MiB
> between 0.99.6 and 0.99.11; filed as
> [#1212](https://github.com/lnds/kaikai/issues/1212)). The suite is never
> "done": every upgrade is re-run, and each one both closes old bugs and opens
> new ones.
