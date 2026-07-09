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
# kaikai: Vec[Point] — the value vector (needs kaikai main, post-0.99.1)
kai build --release kaikai/bench_vec.kai -o /tmp/kv && /tmp/kv 1000000
```

## Performance at scale

The op-count above predicts a cost; here is the measured one. Same machine
(macOS arm64), Rust `-O` with `black_box`, kaikai `--release` (`-O2`);
wall-clock is the min of 6 warm runs, RSS is peak resident set from
`/usr/bin/time -l`. N is read from `argv` so `rustc` cannot constant-fold the
answer. Verified on **kaikai 0.99.1** / **rustc 1.92.0**.

Two things shape these numbers:

1. **RC leaks around record arrays have moved across versions — read the trace,
   not the version tag.** The `Array[Point]` benchmark *fills* the array
   (`a[i] := ...`) and *reads it back* (`a[i].x`). On 0.98 the fill leaked
   ([#1048](https://github.com/lnds/kaikai/issues/1048)/[#1112](https://github.com/lnds/kaikai/issues/1112));
   0.99.1 fixed the fill path, but the same release regressed the **read** path —
   `a[i].field` in a loop now leaks the record on both backends
   ([#1126](https://github.com/lnds/kaikai/issues/1126)). For *this* workload the
   leak is **benign for the ratio**: the array lives to program end, so the
   records were going to occupy memory anyway — `leaked≈N` inflates the RC counts
   but native peak RSS still lands within a few percent of C. The numbers below
   use the **default native backend**; the C column shows the codegen delta. (#1126 has since been
   fixed — [lnds/kaikai#1128](https://github.com/lnds/kaikai/pull/1128) — the
   discard shape reclaims again.)
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

**The `Vec[Point]` row (measured 2026-07-09 on kaikai `main`, post-0.99.1:
`Vec[T]` shipped across lnds/kaikai#1142/#1147/#1152/#1154) changes the case's
verdict.** The idiomatic surface (`import collections.vec`, `vec.push` /
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

## Where Rust wins / the trade-off

- **Zero-cost ownership.** The borrow checker proved, at compile time, that
  no record is aliased and no count is needed. Rust spends nothing at runtime
  to know that. kaikai's Perceus must maintain counts because it does *not*
  have that static aliasing proof; the small per-touch cost is the price of
  not having a borrow checker.
- **Layout control.** `#[repr(C)]`, inline storage, contiguous cache-friendly
  buffers, and FFI-compatible structs are first-class in Rust. kaikai's
  default heap value cannot give you a guaranteed contiguous `repr` the same
  way (you reach for `Array`/`region`, which are opt-in and more limited).
- **This is the dual of [case 4](../04-tree-rewrite-reuse/).** There, the
  cost of the borrow checker was expressivity (cyclic/shared/mutable shapes
  get pushed to `Rc`/`Arc`). Here, the payoff of the borrow checker is a
  runtime that pays nothing for ownership. Same trade, opposite sign. A fair
  comparison reports both.
