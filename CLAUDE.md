# CLAUDE.md — kaikai-vs-rust

Adversarial, honest comparison repo between **kaikai** and **Rust**. Every
claim is proven by compiling/running code, never asserted. Remote:
`github.com/lnds/kaikai-vs-rust` (**private**); push on request — making it
public is the owner's call.

## What this repo is

- `cases/NN-*/` — one folder per blinded case. Each has `README.md`, `rust/`
  (the safe version that fails **and** the version Rust forces), `kaikai/`
  (the `.kai` that compiles and runs). The case README pastes the real `rustc`
  error and the real kaikai output.
- `discarded/` — candidate claims that turned out false under compilation,
  documented not deleted.
- Verified against **rustc 1.92.0 (edition 2024)** and **kaikai 0.104.0**.

### Case status (who wins)

| # | Case | Verdict |
|---|------|---------|
| 1 | cyclic-graph | kaikai (ergonomics) — `&` refs rejected E0506; kaikai uses `Mutable` effect. (A 0.99.5 typer regression #1194 briefly blocked `cycle.kai`; fixed on 0.99.6, runs again — `trace=1 2 1 2 1 2`, both backends) |
| 3 | shared-mutable-state | even (wall, out of the box) → Rust (sharing + small RSS) — **both** reject shared mutation at compile time (Rust E0499; kaikai "mutable cell escapes its block"). On 0.104 parallelism is **default-on** (`KAI_THREADS` defaults to ncpu, cap 32 — #1207 F3 done): `parallel_counter` with no env var = 0.14 s vs Rust 8-thread 0.14 s, same total. Rust keeps: shared memory (kaikai deep-copies cross-thread sends) and a smaller footprint — the default multi-thread scheduler carries a constant ~10 MiB RSS base (12.8 MiB vs Rust 1.7 MiB on this bench; 1.8 MiB at `KAI_THREADS=1`) |
| 4 | tree-rewrite-reuse | kaikai (ergonomics) — unique reuse `reuse_in_place=5`, shared case copies correctly, both backends (was split on 0.94). On 0.104 **both kaikai backends beat Rust** on the depth-22 bench: C 0.15 s / native 0.17 s vs Rust 0.21 s (after restoring `runtime_inline.bc`, the #1413 packaging defect) |
| 5 | effects-vs-boilerplate | kaikai (ergonomics) — viral `async` vs discharged effect row |
| 6 | flat-layout-no-rc | **Rust** (absolute wall only) — kaikai's `Vec[Point]` (value vector, unboxed inline, in-place when unique) matches Rust's memory model: 0.07 s / 155.6 MiB (C backend, re-measured on 0.104 at `KAI_THREADS=1`) vs Rust 0.01 s / 154 MB at N=10M. Rust still wins the wall (~6×, mostly LLVM vectorizing); it no longer wins the layout/RC story. The boxed `Array[Point]` row (853 MB) is the *old* opt-out. `kind Region` adds the **linked**-in-arena story Vec can't: a `Tree<r>` folded to a scalar traces `incref=0` / one wholesale free — **both backends trace identically** (pure-bump model: `arena_alloc=5050 arena_free=5050 incref=0` at N=100, re-confirmed on 0.104) |
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

### Status on 0.104.0 (re-verified by measurement, 2026-07-22)

The 0.104.0 re-run: **all cases green, zero open bugs of ours.** Both
leftovers from 0.99.12 are closed and confirmed by measurement:

- **#1219 fixed** — the C backend's constant +6 `leaked` offset is gone, and
  better: hello-world now traces `leaked=0` on **both** backends (was native 1
  / C 7), `layout.kai` traces `leaked=2` byte-identical on both (was 4 / 10).
- **#1207 F3 done — `KAI_THREADS` now defaults to ncpu (cap 32).** Case 3's
  "opt-in" caveat is obsolete: `parallel_counter` with no env var = 0.14 s,
  wall parity with Rust's 8-thread 0.14 s, same total. `KAI_THREADS=1`
  remains the escape hatch (byte-identical cooperative scheduler).

Three new operational findings, none a correctness bug of ours:

- **Native perf was broken by packaging, not codegen (#1413) — RESOLVED
  locally by installing `runtime_inline.bc`.** The 0.104.0 tarball shipped
  `runtime_llvm.bc` but not `runtime_inline.bc`, so the native modular path
  silently skipped ALL runtime inlining: bench_tree 0.65 s native vs 0.15 s C
  (~4.3×), bench_vec 0.66 s vs 0.07 s (~10×), while `kai --version` still
  said `native p2: active`. With the bitcode restored (2026-07-22, present in
  `~/.kaikai/share/kaikai/include/`) native recovered: bench_tree **0.17 s**
  (≈C 0.15 s, parity back) and bench_vec **0.10 s** — which leaves a real
  ~1.4× native-vs-C codegen residual on the Vec workload (0.99.12 had them
  identical at 0.07 s; confirmed not the core cache — wiping it changes
  nothing — and not the scheduler — same at `KAI_THREADS=1`). Traces and RSS
  are identical across backends; the residual is wall-clock only.
- **RC traces now require `KAI_THREADS=1`** — with the default multi-thread
  scheduler, `KAI_TRACE_RC` reports only the reporting thread's ledger
  (#1338, open upstream): a depth-20 bench_tree prints `alloc_total=0` while
  actually allocating 2.1M nodes. With `KAI_THREADS=1` the traces are
  complete and match the historical model exactly (`reuse_in_place=2097151`
  at depth 20, `reuse_in_place=5` on reuse_unique, `vec_inplace=100000
  vec_cow=0 incref=0` on bench_vec, `arena_alloc=5050 arena_free=5050
  incref=0` on bench_region — both backends identical on all of them).
- **The default scheduler adds a constant ~10 MiB RSS base** (hello-world
  10.5 MiB default vs 1.8 MiB at `KAI_THREADS=1`; flat from N=2 to N=8, and
  unrelated to the old #1212 per-thread 152 MiB). It shows up in case 3
  (12.8 vs Rust 1.7 MiB) and adds ~10 MiB to case 6 numbers measured at the
  default. Data-cost scaling is unchanged (bench_vec 10M = 155.6 MiB at
  `KAI_THREADS=1`). One 6-run sweep of bench_vec at the default showed a
  single 308 MB outlier (likely a COW on an unlucky fiber migration) — not
  reproduced since; worth re-checking next upgrade.

Case 4's perf table gained a headline: **kaikai C now beats Rust** on the
depth-22 unique-rewrite bench (0.15 s vs 0.21 s, and 189 vs 259 MiB RSS).

### Status on 0.99.12 (re-verified by measurement, 2026-07-13)

The 0.99.12 re-run: **all cases green, zero open bugs of ours.** #1212 (the
TLS-pool RSS finding from 0.99.11, below) is **fixed** via PR #1216
(lazy-materialise the RC free-list pools), verified by measurement:
hello-world RSS 161.6 MiB → **1.7 MiB** (`KAI_THREADS=1`) and 722 MiB →
**1.9 MiB** (`KAI_THREADS=8`); `parallel_counter` at `KAI_THREADS=8` = 0.14 s
/ 2.4 MiB (wall unchanged-or-better vs 0.99.11, RSS now at parity with Rust's
1.7 MiB); `bench_vec` N=10M back to 0.07 s / 155.4 MiB measured directly (was
315 MiB with the base). `KAI_THREADS` default is still 1 (#1207 F3 pending).
All RC traces byte-compatible with 0.99.11 except one **minor constant
delta, filed as #1219**: the C backend now leaks 6 extra bootstrap `variant`
allocations (hello-world C `leaked=7` vs native `leaked=1`; `layout.kai` C
`leaked=10` vs native 4) — constant across N (bench_vec C `leaked=10` at both
100k and 200k), not a real leak, but it skews the C-backend `leaked` baseline.
Region traces identical on both backends (`arena_alloc=5050 incref=0`).

### Status on 0.99.11 (historical)

The 0.99.11 re-run: **all cases green, zero open bugs of ours** (#1197 fixed
on 0.99.7, confirmed), **M:N parallelism shipped** (#1207 F1 via PR #1211,
`KAI_THREADS=N`, default 1 — case 3 measured at wall parity with Rust
threads), and **one new finding, filed as #1212** (fixed on 0.99.12, above):
the M:N TLS partitioning (PR #1210) made the RC pools (`kai_cell_pool` 1M×8 B
+ `kai_slot_pool` and `kai_var_block_pool` 9×1M×8 B each, `stage2/runtime.h`)
thread-local, so every scheduler thread materialised ~152 MiB of pool tables —
hello-world RSS went ~2 MiB (0.99.6) → 161.6 MiB (0.99.11, `KAI_THREADS=1`) →
722 MiB at `KAI_THREADS=8`. As `static` these were lazily-paged .bss; as TLS
the image was committed per thread. RC traces and data-cost scaling were
unchanged (exactly 1× per byte of data on `Vec`).

### Status as of 0.99.6 (historical)

Everything this repo filed as a bug is now **CLOSED and confirmed fixed by
running it**, plus the value-record proposal shipped as `Vec[T]` and the
region proposal shipped as `kind Region`:

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

- **#1123** (`kind Region` — arenas as a kind) — **shipped and closed on
  0.99.5** (PR #1192), verified by running. Surface: `region { r -> ... }`
  names a fresh region; a fn `[r: Region]` over `Tree<r>` bump-allocates its
  ctors in that region's arena — the whole linked structure with **zero
  per-node RC**, freed wholesale at the brace. The shipped fixture
  (`examples/perceus/region_tree_1123.kai`, a 100-key BST folded to an Int)
  traces `arena_alloc=5050 arena_free=5050 incref_total=0` on `--backend=c` —
  cross-fn arena routing, reproduced with the installed 0.99.6. Escape is
  handled by **deep-copy-out**: a `Tree<r>` that crosses the brace is cloned
  onto the RC heap before the arena frees (`region_tree_escape_1123.kai` →
  `arena_alloc=55`, `incref=9` for the clone). This is the case `Vec` cannot
  do: `Vec[T]` is contiguous/flat; regions do bulk **linked** structures and
  lifetime-batching. Two things stay follow-up: (a) the **generational**
  escaping path (a `Tree<r>` used *by reference* after escape, and
  `region { r1 -> region { r2 -> ... } }` nesting) — the CORE is sound because
  no-escape is a copy, never a UAF; (b) **native vs C RC parity** — on 0.99.5
  only the C backend routed to the arena; **0.99.6** (#1193) made native route
  too, but native and C now **diverge** on the RC trace: on the shipped fixture
  native traces `arena_alloc=100 incref_total=98` where C traces
  `arena_alloc=5050 incref_total=0`, and a one-line change to the insert flips
  which backend pays the per-node incref. Output is correct on both; filed as
  **#1197**. Treat `--backend=c` as the reference cost model (`incref=0`) until
  it settles. Bare `region { }` without the `<r>` binder is the old #120 form
  (only lexically-inside ctors are arena-routed); the `<r>` binder is what broke
  that "helpers don't arena-route" ceiling.

**Regression cycle across 0.99.5 → 0.99.6 (both from re-running the suite):**

- **#1194** (`CLOSED`, fixed on 0.99.6 via PR #1196) — a 0.99.5 typer regression:
  a **reassigned record-typed `var`** whose field is read **inside a `#{}`
  interpolation** errored `type annotation needed: the receiver of '.id' is not
  resolved`. 7-line repro, no `Ref`/`import loop`. It **blocked case 1's
  `cycle.kai`** on 0.99.5; on 0.99.6 the file compiles and runs unchanged
  (`trace=1 2 1 2 1 2`, both backends). Filed → fixed on the next point release.

- **#1197** (`CLOSED`, fixed on 0.99.7 via PR #1198, confirmed on 0.99.11) —
  after #1193 taught the native backend to route region ctors to the arena,
  native and C disagreed on the region RC trace. The fix converged both on the
  **pure-bump model**: on `region_tree_1123.kai` both now trace
  `arena_alloc=5050 arena_free=5050 incref_total=0`, and the escape fixture
  agrees too (`arena_alloc=55`, `incref=9`). Consequence made explicit by the
  convergence: a churning insert at N=10k bump-allocates every spine node on
  *both* backends (`arena_alloc=50M`, ~1.7 GB) — the old native in-place-reuse
  path is gone. Zero counting, no recycling; either backend is the reference
  now.

If kaikai upgrades again, re-run the suite (below) — verdicts and perf are a
snapshot of the compiler and can move. Check state with
`gh issue view <N> --repo lnds/kaikai --json state` **and read the comments**
(#1048 was CLOSED-but-reopened). To watch on the next upgrade:

- **The ~1.4× native-vs-C residual on bench_vec** (native 0.10 s vs C
  0.07 s with runtime inlining active; they were identical on 0.99.12) —
  whether a later release closes it. bench_tree is already back at parity
  (0.17 vs 0.15 s), so it's Vec-shaped, not general.
- **#1338** (trace ledger is per-thread) — when fixed, `KAI_TRACE_RC` at the
  default `KAI_THREADS` should stop printing `alloc_total=0`; until then all
  traces run at `KAI_THREADS=1`.
- Whether the constant ~10 MiB default-scheduler RSS base shrinks, and
  whether the one-off 308 MB bench_vec outlier at the default scheduler
  reproduces (a fiber-migration COW would be a real finding).

## Benchmarking notes

- Perf numbers live in the `## Performance` section of cases 4 and 6. Method:
  same machine, Rust `-O`, kaikai `--release`; wall-clock = min of ~6 warm runs
  via `/usr/bin/time -l`; peak RSS from the same. No `hyperfine` installed.
- Force Rust to not constant-fold the whole program: read N from `argv` and wrap
  the result in `std::hint::black_box`, else `rustc -O` computes it at compile
  time and reports ~0 for everything.
- **Native wall-clock is only honest if `runtime_inline.bc` is installed**
  (`~/.kaikai/share/kaikai/include/`) — without it the native modular path
  silently skips runtime inlining and runs 4–10× slow (#1413), with
  `kai --version` still reporting `native p2: active`. Check the file before
  trusting a native number. With it present (0.104.0): bench_tree native
  0.17 s ≈ C 0.15 s; bench_vec native 0.10 s vs C 0.07 s (~1.4× residual,
  Vec-shaped). The case-6 headline stays `Vec[Point]` (C: 0.07 s /
  155.6 MiB at N=10M), not the boxed `Array[Point]` row (853 MB), the *old*
  opt-out.
- Regions: native and C trace identically (pure-bump, `incref=0`) — either
  backend works for the *trace*; for wall-clock check the inline-bitcode
  rule above.
- **Run RC traces at `KAI_THREADS=1`** — with the default multi-thread
  scheduler the ledger only covers the reporting thread (#1338): you'll see
  `alloc_total=0` on a program that allocates millions of cells. This is a
  trace artifact, not a leak or a fix.
- **RSS on 0.104.0**: the default ncpu scheduler carries a constant ~10 MiB
  base (hello-world 10.5 MiB; 1.8 MiB at `KAI_THREADS=1`) — far below the old
  0.99.11 per-thread 152 MiB (#1212, fixed), but no longer negligible for
  small-footprint comparisons. For data-cost numbers, measure at
  `KAI_THREADS=1` or subtract the base; don't compare default-scheduler RSS
  raw against 0.99.12 numbers.
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
- **`:=` on a `Ref`-typed field now works** (shipped in #1113, verified on
  0.99.5): `a.next := Some(b)` where `next: Ref[Option[Node]]` compiles and runs
  — that is case 1's form. The full sugar: `&x` makes a Ref, `@r` derefs,
  `r := v` sets a bare `Ref` binding, `a[i] := v` an indexed slot, and
  `record.field := v` a field (Ref-typed or not). The pre-#1113 workarounds
  (`n.next.set(v)` / `Mutable.ref_set` / a 1-cell `Array`) are no longer needed;
  don't reintroduce them.

## Conventions

- Commits: Conventional, English, no Claude attribution.
- Remote is `origin` = `github.com/lnds/kaikai-vs-rust`, **private**. Pushing
  `main` is fine when asked; never flip visibility to public — the owner
  publishes. (Bug issues in `lnds/kaikai` are filed on explicit request —
  that is a different repo.)
- READMEs are in English (the comparison is for a skeptical Rust audience).
- Honesty rule: if a safe Rust version compiles, the claim is false → move it
  to `discarded/`. Every case keeps a "Where Rust wins / trade-off" section.
