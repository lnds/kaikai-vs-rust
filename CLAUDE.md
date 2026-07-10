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
- Verified against **rustc 1.92.0 (edition 2024)** and **kaikai 0.99.3**.

### Case status (who wins)

| # | Case | Verdict |
|---|------|---------|
| 1 | cyclic-graph | kaikai (ergonomics) — `&` refs rejected E0506; kaikai uses `Mutable` effect |
| 3 | shared-mutable-state | even → Rust (parallelism) — **both** reject shared mutation at compile time (Rust E0499; kaikai "mutable cell escapes its block"); Rust wins only on real multi-core |
| 4 | tree-rewrite-reuse | kaikai (ergonomics) — unique reuse `reuse_in_place=5`, shared case copies correctly, both backends (was split on 0.94). On 0.99 native codegen closed the gap: native≈C wall-clock (was ~2.5× slower) |
| 5 | effects-vs-boilerplate | kaikai (ergonomics) — viral `async` vs discharged effect row |
| 6 | flat-layout-no-rc | **Rust** (absolute wall only) — on 0.99.3 kaikai's `Vec[Point]` (value vector, unboxed inline, in-place when unique) matches Rust's memory model: 0.06 s / 155 MB vs Rust 0.01 s / 154 MB at N=10M. Rust still wins the wall (~6×, mostly LLVM vectorizing); it no longer wins the layout/RC story. The boxed `Array[Point]` row (853 MB) is now the *old* opt-out, not the point |
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
- **#1048 / #1112 / #1126** — the native boxed-value leak family. History: on
  0.98 native records were never freed when they left scope (#1048); the fix
  (#1049) was reverted (#1056) over a pre-existing UAF, so on 0.99.0 the
  residue was "`Array[boxed]` slots never decref'd" (#1112) and the read path
  regressed (#1126). **On 0.99.3 the whole family is closed and verified by
  measurement:** the `Array[Point]` fill+read workload at N=100k now traces
  `free_total≈N`, `leaked=4` (just format strings) on native — down from
  `leaked≈N`. What *remains* on `Array[boxed]` is not a leak but per-access RC
  (see #1120 below): reading `a[i].field` in a fold still pays a balanced
  incref/decref per touch (~1.3M ops at 100k), which is a cost, not a
  correctness bug. The zero-RC path is now `Vec[T]`, not `Array`.

### Status on 0.99.3 (re-verified by measurement, 2026-07-10)

Everything this repo filed as a bug is now **CLOSED and confirmed fixed by
running it**, plus the value-record proposal shipped as `Vec[T]`:

- **#1112** (`Array[boxed]` fill leak) — **fixed** (0.99.1). `Array[Record]`
  fill is `leaked≈1`, `free_total≈N`.
- **#1113** (`:=` on a `Ref` field) — **fixed** (PR #1116). Full `Ref` sugar:
  `&x` makes a Ref, `@r` derefs, `r := v` / `record.field := v` sets. Case 1
  uses `next: &None` / `@n.next` / `a.next := Some(b)` and still runs
  (`trace=1 2 1 2 1 2`).
- **#1117** (map pipe over records leaks input) — **fixed** (PR #1122). Case 6
  `layout.kai` trace `leaked=1003` → `leaked=3`.
- **#1120** (borrow annotations on `array_get`) — **closed**. The *array*
  incref/decref per access is elided. The *element* incref on the boxed-record
  read path is still there: `Array[Point]` at N=100k shows
  `incref_total≈600k + decref_total≈700k` ≈ 1.3M RC ops for a pure fold — that
  residual per-touch counting is why boxed `Array` stays ~10× off Rust. `Vec`
  (below) sidesteps it entirely, so this stopped mattering for case 6.
- **#1126** (the 0.99.1 read-path leak) — **fixed** (PR #1128), verified: the
  same `Array[Point]` fill+`a[i].x`-fold loop that leaked `≈N` on 0.99.1 now
  frees correctly — N=100k → `leaked=4` (just the format strings),
  `free_total≈N`. Also re-attributed upstream: it was **not** a #1120
  regression (0.98.0 leaked byte-identically) but the tail-recursive read loop
  never dropping the re-threaded array at the base case.
- **#1121** (unboxed value records + monomorphized `Array`) — **superseded by
  #1135 and delivered as `Vec[T]`** (PRs #1142/#1147/#1152/#1154). `Vec[T]` is
  a pure *value* vector: flat unboxed monomorphized storage (`Vec[Point]` =
  16.2 B/elem, records inline, one RC header for the whole buffer), in-place
  mutation when rc==1 / copy-on-write when shared, purely functional API (no
  `Mutable` in the row). Case 6's `bench_vec.kai` at N=100k traces
  `alloc_total=9`, `vec_inplace=100000 vec_cow=0`, `incref_total=0` — the
  Rust `Vec` cost model proven by the refcount. Coexists with `Array[T]` (the
  mutable *reference*, `Mutable` in the row); intents don't overlap.

Still open (design proposal, not a bug): **#1123** (`kind Region` — escaping
arenas as a kind), narrowed by #1135 to trees / lifetime-batching / the
escaping-generational case (`Vec` took the confined bulk case).

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
- On 0.99.3 native RSS is honest for the case-6 shape, so perf numbers use the
  **default native backend**; the C column only shows the codegen delta. The
  case-6 headline is now `Vec[Point]` (native = C: 0.06 s / 155 MB at N=10M),
  not the boxed `Array[Point]` row (853 MB) which is kept as the *old* opt-out.
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
