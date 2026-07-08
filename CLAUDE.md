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
- Verified against **rustc 1.92.0 (edition 2024)** and **kaikai 0.99.1**.

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
    was record-specific and spared variants) — filed separately as
    **[#1112](https://github.com/lnds/kaikai/issues/1112)** (open). A churn
    workload (build+discard 10k arrays of 10k records) leaks 100M records =
    7 GB native vs 2 MB C, so it is unbounded, not cleanup-at-exit.
  - This leak is **benign for RSS when the array is long-lived** (case 6's
    workload): the records live to program end anyway, so native RSS ≈ C RSS.

### Status on 0.99.1 (re-verified by measurement)

Four of the issues this repo filed were fixed in 0.99.1, all `COMPLETED` with
PRs (not auto-close-by-revert like #1048 was — verify anyway):

- **#1112** (`Array[boxed]` fill leak) — **fixed**. `Array[Record]` fill is now
  `leaked=1`, `free_total≈N`.
- **#1113** (`:=` on a `Ref` field) — **fixed** (PR #1116). The full `Ref` sugar
  shipped: `&x` makes a Ref, `@r` derefs, `r := v` / `record.field := v` sets.
  Case 1 now uses `next: &None` / `@n.next` / `a.next := Some(b)`.
- **#1117** (map pipe over records leaks input) — **fixed** (PR #1122). Case 6
  `layout.kai` trace went `leaked=1003` → `leaked=3`.
- **#1120** (borrow annotations on `array_get`) — **partially shipped** (PR
  #1125). The *array* incref/decref around each access is now elided (`decref`
  constant instead of per-access). But `array_get` still increfs the *element*,
  and step 3 (cancel the element dup/drop when consumed) did NOT land — so wall
  time did not move (still ~0.17s vs Koka 0.02s on `Array[Int]` at 10M), and it
  introduced a regression (next item).
- **#1126** (NEW regression, open) — reading `a[i].field` from an `Array[Record]`
  in a loop now **leaks the record** on both backends, `leaked≈N`. Caused by
  #1120: the array-count elision left the element incref unbalanced on the read
  path for boxed elements. `Array[Int]` fine. Distinct from #1112 (fill path,
  fixed) and #1117 (map pipe, fixed). This is the current live leak on the case-6
  Array workload.

Still open (design proposals, not bugs): **#1121** (value records +
monomorphization), **#1123** (`kind Region` — escaping arenas as a kind).

If kaikai upgrades again, re-run the suite (below) — verdicts and perf are a
snapshot of the compiler and can move. Check state with
`gh issue view <N> --repo lnds/kaikai --json state` **and read the comments**
(#1048 was CLOSED-but-reopened).

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
- `Ref[T]` is a first-class mutable heap cell (`Mutable.ref_make/ref_get/
  ref_set`, or `.set(v)`/`.get()` via UFCS) — the right primitive for "a mutable
  slot inside a record" (case 1 uses `next: Ref[Option[Node]]`). This is *not* a
  Rust-style interior `&'a self` reference; the discarded self-referential case
  is about that borrow-shaped aliasing, not about `Ref`.
- **`:=` gotcha:** the sugar accepts a bare capability/`Ref` binding (`r := v`),
  an indexed slot (`a[i] := v`), and a field ending in an index
  (`n.next[0] := v`) — but **NOT a `Ref`-typed field** (`n.next := v` where
  `next: Ref[T]`). For a `Ref` field write `n.next.set(v)` or
  `Mutable.ref_set(n.next, v)`. Filed as
  [lnds/kaikai#1113](https://github.com/lnds/kaikai/issues/1113). Do not model a
  record slot as a 1-cell `Array` just to get `:=`; that was the pre-#1113
  workaround in case 1.

## Conventions

- Commits: Conventional, English, no Claude attribution.
- **Local only** — never `git push` or `gh repo create` for this repo. The
  owner publishes. (The three bug issues in `lnds/kaikai` were filed on
  explicit request — that is a different repo.)
- READMEs are in English (the comparison is for a skeptical Rust audience).
- Honesty rule: if a safe Rust version compiles, the claim is false → move it
  to `discarded/`. Every case keeps a "Where Rust wins / trade-off" section.
