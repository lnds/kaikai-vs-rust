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
- Verified against **rustc 1.99.0 (edition 2024)** and **kaikai 0.139.2**.
  Perf numbers since the 0.139.2 run come from an **Apple M3 Max (16 cores)**;
  earlier runs were on an M4 Pro (14 cores), roughly 1.15× faster per core —
  compare ratios across releases, not absolute seconds.

### Case status (who wins)

| # | Case | Verdict |
|---|------|---------|
| 1 | cyclic-graph | kaikai (ergonomics) — `&` refs rejected E0506; kaikai uses `Mutable` effect. (A 0.99.5 typer regression #1194 briefly blocked `cycle.kai`; fixed on 0.99.6, runs again — `trace=1 2 1 2 1 2`, both backends) |
| 3 | shared-mutable-state | even (wall, out of the box) → Rust (sharing + small RSS) — **both** reject shared mutation at compile time (Rust E0499; kaikai "mutable cell escapes its block"). Parallelism is **default-on** since 0.104 (`KAI_THREADS` defaults to ncpu, cap 32 — #1207 F3 done): on 0.139 `parallel_counter` with no env var = 0.16 s vs Rust 8-thread 0.16 s, same total. Rust keeps: shared memory (kaikai deep-copies cross-thread sends) and a smaller footprint — but that gap collapsed on 0.139: the default scheduler's RSS base fell from ~10 MiB to ~2 MiB (5.4 MiB vs Rust 1.8 MiB on this bench; 3.5 MiB at `KAI_THREADS=1`) |
| 4 | tree-rewrite-reuse | kaikai (ergonomics) — unique reuse `reuse_in_place=5`, shared case copies correctly, both backends (was split on 0.94). On 0.139 the depth-22 bench is **wall parity at the default scheduler** (C 0.20 s / native 0.22 s vs Rust 0.22 s) and a kaikai lead at `KAI_THREADS=1` (C 0.16 s / native 0.19 s), with kaikai ~36% below on RSS (165 vs 259 MiB). The wall result has moved every release (lead on 0.104, parity on 0.114); the RSS lead is the stable part |
| 5 | effects-vs-boilerplate | kaikai (ergonomics) — viral `async` vs discharged effect row |
| 6 | flat-layout-no-rc | **Rust** (absolute wall only) — kaikai's `Vec[Point]` (value vector, unboxed inline, in-place when unique) matches Rust's memory model: native 53 ms / 155.5 MiB, C 79 ms (0.139.6 at `KAI_THREADS=1`) vs Rust 15 ms / 154 MiB at N=10M. Rust still wins the wall (~3.5×, was ~8× on 0.139.2 before raw `Vec` element access; not vectorization: releases link LLVM 18); it no longer wins the layout/RC story. The boxed `Array[Point]` row (853 MiB, ~0.8 s on 0.139) is the *old* opt-out. `kind Region` adds the **linked**-in-arena story Vec can't: a `Tree<r>` folded to a scalar traces `incref=0` / one wholesale free — **both backends trace identically** (pure-bump model: `arena_alloc=5050 arena_free=5050 incref=0` at N=100, re-confirmed on 0.104) |
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

### Status on 0.139.2 (re-verified by measurement, 2026-10-09)

The 0.139.2 re-run (25 minor releases after 0.114, and rustc 1.92 → 1.99):
**all cases green on both backends, zero open bugs of ours, every verdict
unchanged.** `v0.139.3` is tagged upstream (three small resolve/emit/typer
fixes) but was not the installed toolchain; this run is 0.139.2.

One source fix, cosmetic:

- **`import spawn` was unused in both case-3 fixtures** and 0.139 warns about
  it (`import 'spawn' is not used in this file, but 'time.kai' needs it
  loaded`). Removed from `shared_state_actor.kai` and `parallel_counter.kai`;
  both build warning-free and print the same output. The warning's wording is
  misleading — deleting the import breaks nothing, so nothing "needs it
  loaded" from the fixture — but that is a diagnostics nit, not filed.

What moved, all measured:

- **#1338 is fixed in practice — RC traces are complete at the default
  scheduler.** `KAI_TRACE_RC=1 bench_tree 20` with no `KAI_THREADS` prints
  `alloc_total=2097159 reuse_in_place=2097151`, byte-identical to the
  `KAI_THREADS=1` trace (it printed `alloc_total=0` on 0.104/0.114). The
  `KAI_THREADS=1` rule for traces is no longer needed.
- **The default-scheduler RSS base shrank ~10 MiB → ~2 MiB.** Hello-world is
  3.7 MiB at the default vs 1.7 MiB at `KAI_THREADS=1` (was 11.3 vs 1.8).
  Case 3's footprint gap went from 12.9 vs 1.7 MiB to **5.4 vs 1.8 MiB**.
- **Case 3 still wall parity:** `parallel_counter` default = 0.16 s vs Rust
  8-thread 0.16 s; `KAI_THREADS=1` = 1.25 s (7.8× self-speedup at 8).
- **Case 4 is parity at the default, a lead at one thread.** Depth-22, min of
  15 warm runs: Rust 0.22 s / 259 MiB; kaikai default C 0.20 s / native
  0.22 s / 165 MiB; `KAI_THREADS=1` C **0.16 s** / native 0.19 s / 163 MiB.
  New: the default scheduler costs ~0.03–0.04 s on this RC-heavy
  single-fiber workload (reproduced in two sweeps; not visible on bench_vec).
  RSS improved 179 → 165 MiB.
- **Case 6 `Vec` unchanged in model, native residual wider.** bench_vec
  N=10M: C 0.08–0.09 s / native 0.14 s / 155.6 MiB vs Rust 0.01 s /
  154.2 MiB. The native-vs-C residual is now ~1.6× (was ~1.4×).
- **Boxed `Array[Point]` got slower in wall-clock.** N=10M: native 0.82 s /
  C 0.79 s / 853 MiB, against the 0.99.3 table's 0.39 s / 0.58 s. The machine
  change explains ~1.15× of that, not 2×, and the row was never re-measured
  between 0.99.3 and now, so *when* it regressed is unknown. RC ops are
  unchanged (`incref=600002 decref=700011` at 100k), so it is not extra
  counting. Not the headline row (that is `Vec`), not filed.
- **The idiomatic list pipeline got cheaper.** `layout.kai` now traces
  `alloc_total=4006` (cons 2002 + record 1000) and ~20k RC ops, down from
  6007 allocs (cons 5002) and ~38k. `bench_list` at 100k: 200k allocs and
  ~0.6M RC ops, down from ~0.3M and ~1.1M. Still per-node RC on a linked
  list — the case's point stands, with a smaller constant.
- **The linked-list ceiling moved and is now backend- and
  scheduler-dependent** (see Benchmarking notes). `bench_list` at N=1M
  **runs** with `ulimit -s 65520` on both backends (0.09 s / 178–195 MiB),
  so "the list cannot reach 1M" is retired. At the default scheduler the
  overflow is now diagnosed (`kai: fiber stack overflow`, exit 138) instead
  of a bare SIGSEGV.
- **`leaked` baselines dropped again:** hello, `reuse_unique`, `layout` trace
  `leaked=0`; bench_tree / bench_vec / bench_array / bench_list `leaked=1`
  (was 4); bench_region `leaked=2`. All constant in N.
- All RC models intact and equal across backends: `reuse_in_place=5` /
  `=2097151` at depth 20, `vec_inplace=100000 vec_cow=0 incref_total=0`,
  `arena_alloc=5050 arena_free=5050 incref=0`, escape fixture `arena_alloc=55
  incref=9`, region churn at N=10k `arena_alloc=50005000` / ~1.5 GiB. The only
  cross-backend difference is `live_peak` off by one on tree shapes (C one
  higher) — "byte-identical" was too strong.
- rustc 1.99: E0506 / E0499 / E0597+E0505 reproduce with the same primary
  text; 1.99 adds an `unused_assignments` warning to case 1 and an extra
  `note:` to case 3's E0499. All working Rust versions print the same output.

### Status on 0.114.0 (re-verified by measurement, 2026-09-04)

The 0.114.0 re-run: **all cases green after one source fix; zero open bugs of
ours.** One real regression surfaced, in the repo's own source, not the
compiler:

- **`Fail` was dropped from the builtin effect catalog.** `case 5`'s
  `swap_handlers.kai` declared `pipeline : Int / Fetch + Audit + Fail` and
  called `Fail.fail(...)` without ever declaring the effect — it compiled on
  older releases because `Fail` was in the builtin catalog. On 0.114 the
  catalog is `Console, Stdin, Env, File, State, Reader, Writer, Mutable, Ffi`
  and the file errors `unknown effect: Fail`. **Fixed in-repo** by declaring
  `effect Fail { fail(msg: String) : Nothing }` alongside `Fetch`/`Audit` —
  which is the more honest fixture anyway (the case is about user-declared
  effect rows). Output matches the README byte for byte again: `[audit]…
  result=18`, both backends. `kai info effects` shows the same shape as
  `effect Failure { fail(msg) : Nothing }`, a user effect.

Everything else is unchanged or better:

- **`runtime_inline.bc` now ships in the tarball** (#1413's packaging defect is
  gone — present in `~/.kaikai/share/kaikai/include/` on a clean install).
  Still worth checking before trusting a native wall number.
- **Trace hygiene improved:** `layout.kai` now traces `leaked=0` on both
  backends (was `leaked=2` on 0.104); hello-world stays `leaked=0`.
- **Case 4 lost its wall lead, kept its memory lead.** Depth-22, min of 10 warm
  runs: kaikai C **0.19 s** / native **0.20 s** / Rust **0.19 s**, kaikai
  **179 MiB** vs Rust **259 MiB**. On 0.104 kaikai C led at 0.15 s vs 0.21 s;
  that did not hold. The native-vs-C gap on this workload is gone.
- **Case 6 unchanged:** bench_vec N=10M at `KAI_THREADS=1` — C 0.07 s / native
  0.10 s / Rust 0.01 s, all ~155 MiB. The ~1.4× native-vs-C Vec-shaped
  residual persists.
- **Case 3 unchanged:** `parallel_counter` default = 0.14 s / 12.9 MiB vs Rust
  8-thread 0.14 s / 1.7 MiB; `KAI_THREADS=1` = 1.07 s / 3.5 MiB.
- **The ~10 MiB default-scheduler RSS base persists** (hello-world 11.3 MiB
  default vs 1.8 MiB at `KAI_THREADS=1`, both backends).
- **#1338 still open** — RC traces still need `KAI_THREADS=1`.
- All RC traces byte-identical across backends and consistent with 0.104:
  `reuse_in_place=5` / `=2097151` at depth 20, `arena_alloc=5050 arena_free=5050
  incref=0`, `vec_inplace` model intact.
- The 0.104 one-off 308 MB bench_vec outlier at the default scheduler **did not
  reproduce**.

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

- **The native-vs-C residual on bench_vec** — ~1.4× on 0.104/0.114 (0.10 vs
  0.07 s), ~1.6× on 0.139 (0.14 vs 0.08–0.09 s); they were identical on
  0.99.12. It is widening, not closing. bench_tree also shows native a touch
  behind C now (0.19 vs 0.16 s at `KAI_THREADS=1`).
- **Case 4's wall result moves between releases** — 0.104 had kaikai C
  leading Rust (0.15 vs 0.21 s), 0.114 was parity (0.19 s both), 0.139 is
  parity at the default and a lead at `KAI_THREADS=1` (0.16 vs 0.22 s).
  Re-measure rather than carrying the previous headline forward, and measure
  **both** scheduler modes. The RSS lead (165 vs 259 MiB) has been stable.
- **The default scheduler's wall cost on single-fiber RC-heavy code**
  (bench_tree: ~0.03–0.04 s at depth 22, default vs `KAI_THREADS=1`) — new on
  0.139; whether it grows or goes away.
- **Boxed `Array[Point]` wall-clock** (~0.8 s at 10M on 0.139 vs 0.39 s
  native on 0.99.3, across a machine change). Re-measure the row every
  upgrade from now on so a regression can be pinned to a release.
- **The list recursion ceiling** — it differs by backend and scheduler on
  0.139 (see Benchmarking notes); re-probe instead of quoting a number.
- **The builtin effect catalog can shrink.** `Fail` was in it through 0.104
  and is gone on 0.114 (catalog: `Console, Stdin, Env, File, State, Reader,
  Writer, Mutable, Ffi`). Fixtures that lean on a builtin effect break
  silently across upgrades — declare effects in the fixture.
- **#1338** (trace ledger is per-thread) — fixed as observed on 0.139: the
  default-scheduler trace is complete. If `alloc_total=0` ever reappears on
  an allocating program, that is the regression.
- The default-scheduler RSS base: ~10 MiB on 0.104/0.114, **~2 MiB on
  0.139**. The one-off 308 MB bench_vec outlier has not reproduced since
  0.104 (158 MiB at the default on 0.139).

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
  `kai --version` still reporting `native p2: active`. 0.114.0 ships it in
  the tarball; 0.104.0 did not. Check the file before trusting a native
  number. On 0.139.2 (file present): bench_tree native 0.19 s vs C 0.16 s at
  `KAI_THREADS=1`; bench_vec native 0.14 s vs C 0.08–0.09 s (~1.6×
  residual). The case-6 headline stays `Vec[Point]` (C: 0.08 s / 155.6 MiB
  at N=10M), not the boxed `Array[Point]` row (853 MiB), the *old* opt-out.
- **Measure wall-clock in both scheduler modes** (default and
  `KAI_THREADS=1`). On 0.139 the default costs ~0.03–0.04 s on bench_tree at
  depth 22 and nothing measurable on bench_vec; quoting only one mode hides
  that.
- Regions: native and C trace identically (pure-bump, `incref=0`) — either
  backend works for the *trace*; for wall-clock check the inline-bitcode
  rule above.
- **RC traces no longer need `KAI_THREADS=1`** (0.139): the default-scheduler
  ledger is complete and matches the single-thread trace. On 0.104/0.114 it
  covered only the reporting thread (#1338) and printed `alloc_total=0` on a
  program allocating millions of cells — if you see that again, pin
  `KAI_THREADS=1` and treat it as a regression.
- **RSS and the default scheduler**: the constant base was ~10 MiB on
  0.104/0.114 and is ~2 MiB on 0.139 (hello-world 3.7 MiB default vs 1.7 MiB
  at `KAI_THREADS=1`). Small, but for data-cost numbers still measure at
  `KAI_THREADS=1` or subtract it.
- **Stack, not the compiler:** the `[Point]` linked-list version of case 6 uses
  non-tail recursion (`build`/`sum_x` with no accumulator), so its ceiling is
  the main stack — and on 0.139 that ceiling depends on backend and
  scheduler. At the stock 8 MiB `ulimit -s`: native survives 150k and dies
  at 200k in both modes; C dies at 150k at the default scheduler but runs
  1M at `KAI_THREADS=1`. At the default scheduler the failure is diagnosed
  (`kai: fiber stack overflow at 0x…`, exit 138); native at `KAI_THREADS=1`
  is still a bare SIGSEGV (139). With `ulimit -s 65520` **1M runs on both backends in both
  modes** (at the default scheduler `KAI_FIBER_STACK_SIZE=67108864` does the
  same job); 3M does not. So N=100k is the size that runs everywhere with no
  setup, and a 1M list number is legitimate only with the stack raised —
  say so next to it.
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
a pipe — `139` = SIGSEGV, `138` = a diagnosed fiber stack overflow. RC behavior: `KAI_TRACE_RC=1 ./bin` prints
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
- **The builtin effect catalog is `Console, Stdin, Env, File, State, Reader,
  Writer, Mutable, Ffi`** (plus the runtime set `Stdin/Stdout/Stderr/Env/File`
  installed at `main`). `Fail` is *not* builtin on 0.114 — declare it
  (`effect Fail { fail(msg: String) : Nothing }`). Check with
  `kai info effects` rather than assuming a name is in the catalog.
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
