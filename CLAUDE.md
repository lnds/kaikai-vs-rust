# CLAUDE.md — kaikai-vs-rust

Adversarial, honest comparison repo between **kaikai** and **Rust**. Every
claim is proven by compiling/running code, never asserted. Local-only git (no
remote, no push — the owner reviews and publishes).

## What this repo is

- `cases/NN-*/` — one folder per blinded case. Each has `README.md`, `rust/`
  (the safe version that fails **and** the version Rust forces), `kaikai/`
  (the `.kai` that compiles and runs). The case README pastes the real `rustc`
  error and the real kaikai output.
- `discarded/` — candidate claims that turned out false under compilation,
  documented not deleted.
- Verified against **rustc 1.92.0 (edition 2024)** and **kaikai 0.99.0**.

### Case status (who wins)

| # | Case | Verdict |
|---|------|---------|
| 1 | cyclic-graph | kaikai (ergonomics) — `&` refs rejected E0506; kaikai uses `Mutable` effect |
| 3 | shared-mutable-state | even → Rust (parallelism) — **both** reject shared mutation at compile time (Rust E0499; kaikai "mutable cell escapes its block"); Rust wins only on real multi-core |
| 4 | tree-rewrite-reuse | kaikai (ergonomics) — unique reuse `reuse_in_place=5`, shared case copies correctly, both backends (was split on 0.94). On 0.99 native codegen closed the gap: native≈C wall-clock (was ~2.5× slower) |
| 5 | effects-vs-boilerplate | kaikai (ergonomics) — viral `async` vs discharged effect row |
| 6 | flat-layout-no-rc | **Rust** — `Vec<Point>` zero RC vs ~10k RC ops. On 0.99 native RSS ≈ C (record leak no longer inflates it here — see #1048 note) |
| — | self-referential | discarded — kaikai has no interior references; problem can't be expressed |
| — | shared-mutable-cache | discarded — `Rc<RefCell<HashMap>>` is safe, claim false |

Case numbering skips 2 (self-referential was discarded).

## Upstream bugs found while building this

Filed at `github.com/lnds/kaikai`. The first three were found on 0.94 and are
**fixed as of 0.98**; the fourth was found on 0.98 while benchmarking. Kept here
as the record of why the repo runs every claim.

- **#995** (fixed) — native reused recursive-ADT nodes in place when shared,
  corrupting the original. Case 4 flipped split → kaikai win on 0.98.
- **#996** (fixed) — C backend `redefinition of kai_<var>` for a `var` named
  `x`/`state`/`log`. Never surfaced in a case.
- **#997** (fixed) — SIGSEGV when a spawned fiber captured a `var`. On 0.98 it's
  a compile-time error ("mutable cell escapes its block"); case 3's kaikai side
  is that static rejection, mirroring Rust's E0499.
- **#1048** — native record leak. **The GitHub API reports it CLOSED, but it is
  actually reopened**: the fix (#1049) was reverted (#1056) because it uncovered
  a pre-existing UAF (#747) that SIGSEGVs the self-compile; re-fix is deferred to
  the #1054/#747 lane. Do not trust `--json state` alone — read the comments.
  **In practice on 0.99 the scope shrank** (verified by measurement, not the
  issue text):
  - Short-lived records (the issue's own MCVE) and records in a `[T]` linked
    list now **free correctly** on native (`leaked=3–4`, RSS flat — 10M
    short-lived records = 1 MB, down from 768 MB on 0.98).
  - **What still leaks on native: boxed values stored in an `Array[T]`** —
    `Array[Record]` AND `Array[Wrap(Int)]` (an ADT variant) both show
    `leaked≈N`, `free_total≈1`; `Array[Int]` (unboxed) is fine. So it is not
    record-specific anymore; it is "`array_make` of a boxed element type never
    decrefs its slots on native." Different scope from the original #1048 (which
    was record-specific and spared variants) — worth a fresh comment/issue.
  - This leak is **benign for RSS when the array is long-lived** (case 6's
    workload): the records live to program end anyway, so native RSS ≈ C RSS.
    That is why the **case 6 table no longer needs `--backend=c`** — native is
    now honest for that shape. It would still inflate RSS in a workload that
    builds and discards many arrays-of-boxed in a loop.

If kaikai upgrades again, re-run the suite (below) — verdicts and perf are a
snapshot of the compiler and can move. Check state with
`gh issue view <N> --repo lnds/kaikai --json state`.

## Benchmarking notes

- Perf numbers live in the `## Performance` section of cases 4 and 6. Method:
  same machine, Rust `-O`, kaikai `--release`; wall-clock = min of ~6 warm runs
  via `/usr/bin/time -l`; peak RSS from the same. No `hyperfine` installed.
- Force Rust to not constant-fold the whole program: read N from `argv` and wrap
  the result in `std::hint::black_box`, else `rustc -O` computes it at compile
  time and reports ~0 for everything.
- On 0.99 native RSS is honest for the case-6 shape (long-lived array), so
  perf numbers use the **default native backend**; keep a C column only to show
  the codegen delta, not for memory correctness. (The `Array[T]` leak is benign
  here — see the #1048 note above.)
- **Stack, not the compiler:** the `[Point]` linked-list version of case 6 uses
  non-tail recursion (`build`/`sum_x` with no accumulator) and **SIGSEGVs above
  ~100k** even with `ulimit -s` maxed (64 MB on macOS). So the list variant is
  benchmarked at N=100k; only the tail-recursive `Array` variant (and Rust)
  reach 1M–10M. Do not claim a 1M list number — it does not run.
- `KAI_TRACE_RC=1 ./bin` (stderr) gives `alloc_total`, `free_total`, `leaked`,
  `live_peak`, `reuse_in_place`, `incref_total`, `decref_total` — the honest way
  to see RC cost and catch leaks (leaked scaling with N = real leak).

## How to re-verify everything

Run the full suite (re-create it if gone — it just builds + runs every example
and checks the expected error/output/exit):
the canonical commands are in each case's `## Reproduce` section. Quick loop:

```sh
# Rust side — edition 2024 (latest), the fairness baseline
rustc --edition 2024 <case>/rust/<file>.rs -o /tmp/x   # or -O for case 6
# kaikai side — test BOTH backends where a bug is backend-specific
kai build --backend=native <case>/kaikai/<file>.kai -o /tmp/k && /tmp/k
kai build --backend=c      <case>/kaikai/<file>.kai -o /tmp/k && /tmp/k
```

True exit codes: run the built binary directly (`./bin; echo $?`), not through
a pipe — `139` = SIGSEGV. RC behavior: `KAI_TRACE_RC=1 ./bin` prints
`reuse_in_place`, `incref_total`, `decref_total`, `alloc_total` to **stderr**.

## Environment gotchas

- The user's shell prints a noisy `zoxide` doctor warning on every `kai`/`git`
  invocation (from `~/.zshrc` line ~322). It is harmless. Filter it:
  `2>&1 | grep -vE "zoxide|ajeetdsouza|persists|initialized|Disable|Please ensure"`.
  A clean wrapper script (`#!/bin/zsh -f` + `export _ZO_DOCTOR=0` + `exec kai`)
  also works.
- **`kai info <topic>` is the source of truth for kaikai syntax** — do not
  extrapolate from Haskell/ML/Rust. `kai info syntax` has a NOT-IN-KAIKAI
  section of false friends. `kai doc <module>` for stdlib.
- kaikai facts that bit during this work: `[T]` is a linked list, `Array[T]` is
  the indexable array (`array_make`, `from_list`); tuples are `Pair`/`Triple`
  records (`.fst`/`.snd`); mutating an `Array` cell or `Ref` needs the
  `Mutable` effect in the row; `var` desugars to a `State` handler; the
  idiomatic concurrent-state form is an **actor** (`with_mailbox`), not a
  shared `var`.

## Conventions

- Commits: Conventional, English, no Claude attribution.
- **Local only** — never `git push` or `gh repo create` for this repo. The
  owner publishes. (The three bug issues in `lnds/kaikai` were filed on
  explicit request — that is a different repo.)
- READMEs are in English (the comparison is for a skeptical Rust audience).
- Honesty rule: if a safe Rust version compiles, the claim is false → move it
  to `discarded/`. Every case keeps a "Where Rust wins / trade-off" section.
