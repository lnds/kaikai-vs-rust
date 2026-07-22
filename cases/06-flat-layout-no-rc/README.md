# Case 6 — flat layout with zero reference counting (Rust wins)

**This case is where Rust wins, cleanly.** A repo that only praised kaikai
would not be honest. The borrow checker buys something Perceus cannot: a
contiguous, owned, reference-count-free data layout, proven safe at compile
time with **zero** runtime bookkeeping. This is the exact dual of the
Perceus-reuse argument — the same property (compile-time ownership) that costs
Rust expressivity elsewhere pays off here.

## The claim

> Processing a bulk of flat records (`Point { x, y }`) in Rust is a
> `Vec<Point>`: one heap allocation, values inline, the whole buffer freed
> once, and **no reference counting at all** — the borrow checker proved none
> is needed. The same workload in kaikai routes every record and every list
> cell through Perceus RC, paying thousands of `incref`/`decref` operations
> and one allocation per node, on a linked list rather than a contiguous
> buffer.

Both programs compute the same answer (`total=500500`, the sum of `x` over
1000 points). The difference is entirely in what the runtime pays.

## Rust — one allocation, zero RC

[`rust/layout.rs`](rust/layout.rs) — `rustc -O`, prints `total=500500`:

```rust
#[repr(C)]
#[derive(Clone, Copy)]
struct Point { x: i64, y: i64 }

fn sum_x(xs: &[Point]) -> i64 { xs.iter().map(|p| p.x).sum() }
fn build(n: i64) -> Vec<Point> {
    (1..=n).rev().map(|k| Point { x: k, y: k * 2 }).collect()
}

fn main() {
    let pts = build(1000);          // ONE heap allocation (the Vec buffer)
    println!("total={}", sum_x(&pts));
}
```

Output:

```text
total=500500
size_of::<Point>() = 16 bytes, contiguous
```

`Point` is `Copy` and `#[repr(C)]`: 16 bytes, living **inline** in the Vec's
buffer. `sum_x` borrows the slice — no ownership transfer, no clone, no
counting. The Vec is one allocation, freed once when `pts` leaves scope. There
is no `Rc`, no `Arc`, no `RefCell`, and nothing increments or decrements a
count anywhere in this program.

## kaikai — every value under Perceus RC

[`kaikai/layout.kai`](kaikai/layout.kai) — `kai run`, prints `total=500500`:

```kaikai
type Point = { x: Int, y: Int }

fn main() : Int / Stdout = {
  let pts = [1..1000] | (k => Point { x: k, y: k * 2 })
  let total = pts | (.x) |> list.sum
  Stdout.print("total=#{int_to_string(total)}")
  0
}
```

This is the idiomatic form: a range mapped to records, then a field projection
summed — no hand-written recursion. Run with the RC tracer
(`KAI_TRACE_RC=1 kai run layout.kai`):

```text
total=500500
[KAI_TRACE_RC] alloc_total=6007 free_total=6004 leaked=3 live_peak=2001
[KAI_TRACE_RC]   tag cons    allocs=5002
[KAI_TRACE_RC]   tag record  allocs=1000
[KAI_TRACE_RC]   incref_total=15995 decref_total=21999
```

For the same 1000 points: 1000 `Point` records + several thousand linked-list
`cons` cells, and **~38000 reference-count operations** (`incref_total=15995` +
`decref_total=21999`). `[Point]` is a **linked list**, not a contiguous buffer —
each element is a separate heap node with its own count, not 16 inline bytes in
one block. That per-node RC is the cost the case is about, and Rust's `Vec` pays
none of it.

> History note on the trace: on kaikai 0.99.0 this same program showed
> `leaked=1003` — a real compiler bug where the record-projecting map `| (.x)`
> failed to free its input records
> ([lnds/kaikai#1117](https://github.com/lnds/kaikai/issues/1117)), found *by*
> writing this example idiomatically. **It is fixed as of 0.99.1** (`leaked=3` —
> just the format strings). The point of the case — per-node reference counting
> on a linked list vs Rust's zero — was always the real content; the leak was a
> transient bug on top of it.

> Caveat, stated honestly: this is the naive idiomatic form on both sides,
> and the **default** cost model is still the point: Rust's default flat type
> is RC-free by construction; kaikai's default `[Point]` is a reference-counted
> linked list, and you opt *out*. What changed (2026-07-09) is what the opt-out
> buys: `Vec[T]` — the pure value vector (unboxed inline elements, one RC
> header, in-place mutation when unique, copy-on-write when shared) — now
> matches Rust's cost model while keeping a purely functional API, measured
> below. Whether `[1,2,3]` should *default* to `Vec` is an open decision for
> the kaikai 1.0 surface pin.

## Reproduce

```sh
cd rust
rustc --edition 2024 -O layout.rs -o layout && ./layout      # total=500500
cd ../kaikai
KAI_TRACE_RC=1 kai run layout.kai                            # total=500500 + RC trace
```

The `## Performance` table below uses parametrized versions that read N from
`argv` (so `rustc -O` cannot constant-fold the answer):

```sh
# Rust: Vec<Point>, black_box'd
rustc --edition 2024 -O rust/bench_layout.rs -o /tmp/bl && /tmp/bl 1000000
# kaikai: linked list (SIGSEGVs above ~100k — non-tail recursion)
kai build --release kaikai/bench_list.kai  -o /tmp/kl && /tmp/kl 100000
# kaikai: Array[Point], tail-recursive — reaches 1M/10M
kai build --release kaikai/bench_array.kai -o /tmp/ka && /tmp/ka 1000000
# kaikai: Vec[Point] — the value vector (0.99.3)
kai build --release kaikai/bench_vec.kai -o /tmp/kv && /tmp/kv 1000000
# kaikai: Tree<r> in an arena — kind Region (0.99.5). Since 0.99.7 (#1197
# fixed) native and C agree on the trace; either backend works.
kai build --release kaikai/bench_region.kai -o /tmp/kr
KAI_TRACE_RC=1 /tmp/kr 100    # total=5050; arena_alloc=5050, incref_total=0
```

## Performance at scale

The op-count above predicts a cost; here is the measured one. Same machine
(macOS arm64), Rust `-O` with `black_box`, kaikai `--release` (`-O2`);
wall-clock is the min of 6 warm runs, RSS is peak resident set from
`/usr/bin/time -l`. N is read from `argv` so `rustc` cannot constant-fold the
answer. Wall-clock/RSS table measured on **kaikai 0.99.3** / **rustc 1.92.0**;
the RC behavior (`vec_inplace=N`, `incref_total=0` on `Vec`) re-confirmed by
re-running on **0.99.6**, **0.99.11** and **0.99.12** (the `Vec[Point]` row's
10M cell re-measured on 0.99.12: 0.07 s / 155.4 MiB, within noise of the
table). The `kind Region` arena sub-case (shipped 0.99.5) is measured fresh
below.

**A note on the 0.99.11 RSS blip:** on 0.99.11 (only) the new M:N scheduler
runtime materialised ~150 MiB of thread-local RC pool tables even at the
default `KAI_THREADS=1`, so every measured RSS carried that fixed base —
`bench_vec` at N=10M read 315 MiB (152.6 MiB of data + the base). Filed as
[#1212](https://github.com/lnds/kaikai/issues/1212), **fixed on 0.99.12**
([PR #1216](https://github.com/lnds/kaikai/pull/1216)): hello-world is back to
~1.7 MiB, and `bench_vec` at N=10M re-measures at **0.07 s / 155.4 MiB** —
the table's numbers are the measured numbers again, no base to subtract. The
RC traces were unchanged throughout (`vec_inplace=N`, `incref=0`, data cost
exactly 1×).

Two things shape these numbers:

1. **RC leaks around record arrays have moved across versions — read the trace,
   not the version tag.** The `Array[Point]` benchmark *fills* the array
   (`a[i] := ...`) and *reads it back* (`a[i].x`). On 0.98 the fill leaked
   ([#1048](https://github.com/lnds/kaikai/issues/1048)/[#1112](https://github.com/lnds/kaikai/issues/1112));
   0.99.1 fixed the fill path but transiently regressed the **read** path
   ([#1126](https://github.com/lnds/kaikai/issues/1126)); **both are fixed on
   0.99.3** ([#1128](https://github.com/lnds/kaikai/pull/1128)). At N=100k the
   `Array[Point]` fill+`a[i].x`-fold now traces `free_total≈N`, `leaked=4` (just
   the format strings). What remains on the boxed `Array` is per-access RC, not
   a leak: the fold pays a balanced incref/decref per touch
   (`incref_total≈600k + decref_total≈700k` at 100k). The numbers below use the
   **default native backend**; the C column shows the codegen delta. (Native
   numbers require `runtime_inline.bc` in the install — the 0.104.0 tarball
   shipped without it, silently disabling runtime inlining and making native
   ~10× slower; upstream
   [#1413](https://github.com/lnds/kaikai/issues/1413). With it restored,
   native is 0.10 s vs C 0.07 s at 10M — a ~1.4× residual on the Vec
   workload, wall-clock only.)
2. **The `[Point]` linked-list version cannot reach 1M.** Its `build`/`sum_x`
   are non-tail recursive and **SIGSEGV above ~100k** even with the stack maxed
   (64 MB on macOS). That is a property of the naive recursive shape, not the
   compiler — but it means the honest list number is at N=100k, and only the
   tail-recursive `Array` variant (and Rust) reach 1M+.

**N = 100,000** (the ceiling the list survives), all three shapes:

| Implementation | wall-clock | peak RSS |
|----------------|-----------:|---------:|
| Rust `Vec<Point>` (`-O`)                    | ~0.00 s | 3 MB |
| kaikai `[Point]` linked list, native        | ~0.00 s | 23 MB |
| kaikai `Array[Point]`, native               | ~0.00 s | 10 MB |

**N = 1,000,000 and 10,000,000** (`Array` + Rust; the list SIGSEGVs here):

| Implementation | N=1M wall | N=1M RSS | N=10M wall | N=10M RSS |
|----------------|----------:|---------:|-----------:|----------:|
| Rust `Vec<Point>` (`-O`)          | ~0.00 s | 16 MB | ~0.01 s | 154 MB |
| kaikai `Array[Point]`, C          | ~0.05 s | 93 MB | ~0.58 s | 852 MB |
| kaikai `Array[Point]`, native     | ~0.04 s | 85 MB | ~0.39 s | 844 MB |
| kaikai `Vec[Point]`, C = native   | ~0.00 s | 18 MB | **0.06 s** | **155 MB** |

Rust builds and sums the points below (1M) or near (10M) the timer's resolution
in a contiguous buffer; kaikai is ~an order of magnitude slower and uses ~5×
the memory. The RC trace shows why: at 100k the linked list does
`incref_total≈0.4M` + `decref_total≈0.7M` ≈ **1.1M reference-count operations**
and `alloc_total≈0.3M` (100k `Point` records + 200k `cons` cells), versus Rust's
single contiguous allocation and zero counting. (Note the native backend is now
*faster* than C on the `Array` workload — 0.39 s vs 0.58 s at 10M — the 0.98
codegen gap has largely closed.)

**The `Vec[Point]` row (re-measured on kaikai 0.99.3: `Vec[T]` shipped across
lnds/kaikai#1142/#1147/#1152/#1154) changes the case's verdict.** The idiomatic surface (`import collections.vec`, `vec.push` /
`vec.get` in plain tail-recursive user code — no internal primitives) lands at
**0.06 s / 155 MB** at N=10M against Rust's 0.01 s / 154 MB: memory parity to
within one percent, wall within ~6× of a `black_box`'d `rustc -O` loop — and
**13× faster with 5.5× less memory than the boxed `Array` row above**, on a
purely functional API with no borrow checker. The title of this case was
"Rust wins, cleanly". As of `Vec`: Rust still wins the absolute wall; it no
longer wins the memory model, and the gap is a small constant, not a
complexity class. RC-op count for the whole 10M run: container and element
traffic on the unique path is zero (in-place uniqueness), which is the point —
the counting the borrow checker proves away, Perceus proves away at runtime
with rc == 1.

**Re-confirmed on 0.104.0** (`KAI_THREADS=1`): C 0.07 s / native 0.10 s, both
155.6 MiB at N=10M, `vec_inplace=100000 vec_cow=0 incref_total=0` at 100k on
both backends — the cost model is unchanged; native carries a ~1.4×
wall-clock residual vs C on this workload (they were identical on 0.99.12). Two 0.104 measurement notes: RC traces must run at
`KAI_THREADS=1` (the default M:N scheduler reports only the main thread's
ledger — upstream [#1338](https://github.com/lnds/kaikai/issues/1338) — and
prints a misleading `alloc_total=0`), and the default scheduler adds a
constant ~10 MiB RSS base to any measurement (measure at `KAI_THREADS=1` for
data cost).

Two honest caveats:

- **Cross-language wall-clock is slippery.** Rust's number is so low mostly
  because LLVM vectorises a trivial loop; the point is the *shape* of the gap
  (one alloc + no RC vs millions of both), not the exact ratio.
- **kaikai's `Array` barely beats its list on memory, not on the counting.**
  The array itself is one allocation (`alloc_total` stays ~15 for 1M elements),
  so the layout is right; the residual cost is per-access reference counting on
  the boxed `Point` elements. Reading `arr[i]` costs a balanced incref/decref
  per touch on a record array — the cache-friendly layout is eaten by that
  per-access counting, not by a bug.

## The sub-case Vec can't do: `kind Region` (linked structures in an arena)

`Vec[T]` closed the memory model for *flat* bulk data, but it is contiguous —
it cannot hold a *linked* structure. A tree of `Node(Tree, Int, Tree)` under
plain Perceus pays one heap allocation and reference counting per node. On
kaikai **0.99.5** the follow-up proposal
([#1123](https://github.com/lnds/kaikai/issues/1123)) shipped as `kind Region`
([#1192](https://github.com/lnds/kaikai/pull/1192)), and it covers exactly this
shape: a `region { r -> ... }` block plus region-polymorphic functions over
`Tree<r>` bump-allocate the whole tree in an arena — **zero per-node reference
counting**, freed in one shot at the closing brace.

The surface is opt-in and the region name rides a binder (`kaikai/bench_region.kai`):

```kaikai
type Tree = Leaf | Node(Tree, Int, Tree)

# `<r>` appears only in the signature; the Node ctors stay pure and
# bump-allocate in r's arena. The result is an Int, so nothing escapes.
fn tree_insert[r: Region](t: Tree<r>, k: Int) : Tree<r> = ...
fn tree_sum[r: Region](t: Tree<r>) : Int = ...

let total = region { r ->
  let t = build_tree(Leaf, n)   # n Node's bump-allocated, zero RC
  tree_sum(t)                    # scalar result -> r does not escape
}                                # <- the WHOLE tree frees here: one shot
```

Measured on the installed 0.99.12 (native and C trace identically since
0.99.7), at N=100 (the same size as kaikai's own shipped fixture,
`total=5050`) — `KAI_TRACE_RC=1`:

```
arena_alloc=5050  arena_free=5050  arena_live=0  incref_total=0  alloc_total=9
```

`incref_total=0`: not one per-node reference count for the entire build+fold,
and `arena_free` matches `arena_alloc` (nothing leaks). The heap RC sees only 9
transient allocations (format strings and the like); every tree node lives and
dies in the arena. The count is
`N(N+1)/2` (5050 for N=100) because inserting `n..1` in order builds a
degenerate spine — the same non-tail shape as `bench_list`. That is the arena
cost model, proven by the refcount, on a linked structure `Vec` cannot express.

**Two honest limits, one since resolved, one now sharper:**

- **The backend split is fixed.** On 0.99.6 native and C disagreed on the
  region RC trace (native `arena_alloc=100 incref=98` vs C
  `arena_alloc=5050 incref=0` on the same fixture — filed as
  [#1197](https://github.com/lnds/kaikai/issues/1197)). **Fixed on 0.99.7**
  and re-verified here on 0.99.11: both backends now trace identically —
  `arena_alloc=5050 arena_free=5050 incref_total=0` on this bench, and the
  escape fixture agrees too (`arena_alloc=55`, `incref=9` for the copy-out on
  both). The convergence landed on the pure-bump model: zero per-node RC,
  every ctor bump-allocates. Either backend is now the reference.
- **A bump arena does not recycle abandoned nodes — on both backends, now.**
  A functional `tree_insert` abandons the old spine on every insert; in a pure
  bump arena those dead nodes accumulate until the region frees. Before the
  convergence, native happened to reuse the spine in place (tiny memory,
  `incref≈N`); the converged model bump-allocates on both: at N=10k both
  backends trace `arena_alloc=50M`, `incref=0`, at **~1.7 GB** peak RSS. That
  is the trade stated plainly: zero counting, no recycling. Regions are
  unambiguously the right tool when the lifetime is short and *nothing escapes*
  (build → query → drop); long-lived churn stays where Perceus shines. The
  type (`Tree<r>` vs `Tree`) documents which model you asked for.

## Where Rust wins / the trade-off

- **Zero-cost ownership.** The borrow checker proved, at compile time, that
  no record is aliased and no count is needed. Rust spends nothing at runtime
  to know that. kaikai's Perceus must maintain counts because it does *not*
  have that static aliasing proof; the small per-touch cost is the price of
  not having a borrow checker.
- **Layout control.** `#[repr(C)]`, inline storage, contiguous cache-friendly
  buffers, and FFI-compatible structs are first-class in Rust. kaikai's
  default heap value cannot give you a guaranteed contiguous `repr` the same
  way; you reach for `Vec[T]` (contiguous, unboxed, the flat case) or a
  `region { }` arena (`Tree<r>`, the linked case) — both opt-in, and the arena
  is C-backend-only today and does not recycle (see the sub-case above).
- **This is the dual of [case 4](../04-tree-rewrite-reuse/).** There, the
  cost of the borrow checker was expressivity (cyclic/shared/mutable shapes
  get pushed to `Rc`/`Arc`). Here, the payoff of the borrow checker is a
  runtime that pays nothing for ownership. Same trade, opposite sign. A fair
  comparison reports both.
