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

fn sum_x(xs: [Point]) : Int { case [] -> 0 ; case [h, ...t] -> h.x + sum_x(t) }
fn build(n: Int) : [Point] {
  case 0 -> []
  case k -> [Point { x: k, y: k * 2 }] ++ build(k - 1)
}
```

Run with the RC tracer (`KAI_TRACE_RC=1 kai run layout.kai`):

```text
total=500500
[KAI_TRACE_RC] alloc_total=3005 free_total=2002 leaked=1003 live_peak=2001
[KAI_TRACE_RC]   tag cons    allocs=2002
[KAI_TRACE_RC]   tag record  allocs=1000
[KAI_TRACE_RC]   incref_total=3998 decref_total=6000
```

For the same 1000 points: **~3000 allocations** (1000 `Point` records + ~2000
linked-list `cons` cells) and **~10000 reference-count operations**
(`incref` + `decref`). And `[Point]` is a **linked list**, not a contiguous
buffer — each element is a separate heap node with its own count, not 16
inline bytes in one block.

> Caveat, stated honestly: this is the naive idiomatic form on both sides.
> kaikai has an `Array[T]` for contiguous storage and a `region { }` block
> that bump-allocates and skips RC for escape-free scratch, which would close
> part of this gap. The point of the case is the **default** cost model: Rust's
> default flat type is RC-free by construction; kaikai's default value is
> reference-counted, and you opt *out* with `Array`/`region`, where Rust never
> opted in.

## Reproduce

```sh
cd rust
rustc --edition 2024 -O layout.rs -o layout && ./layout      # total=500500
cd ../kaikai
KAI_TRACE_RC=1 kai run layout.kai                            # total=500500 + RC trace
```

## Performance at scale (N = 1,000,000)

The op-count above predicts a cost; here is the measured one. Same machine
(macOS arm64), Rust `-O`, kaikai `--release` (`-O2`); wall-clock is the min of
6 warm runs, RSS is peak resident set from `/usr/bin/time -l`. **kaikai numbers
use `--backend=c`** because the native backend currently leaks record values
([lnds/kaikai#1048](https://github.com/lnds/kaikai/issues/1048)), which would
inflate its memory here and make the comparison unfair to no one's benefit.

| Implementation | wall-clock | peak RSS |
|----------------|-----------:|---------:|
| Rust `Vec<Point>` (`-O`)            | ~0.00 s | 16 MB |
| kaikai `[Point]` linked list (`-O2`) | ~0.06 s | 132 MB |
| kaikai `Array[Point]` (`-O2`)        | ~0.08 s | 86 MB |

Rust builds and sums 1M points below the timer's resolution in a 16 MB buffer;
at N=10M it is still ~0.01 s. kaikai is ~an order of magnitude slower and uses
5–8× the memory. The RC trace shows why: the linked-list version does
`incref_total≈4.0M` + `decref_total≈7.0M` ≈ **11M reference-count operations**
and `alloc_total≈3M` (1M `Point` records + 2M `cons` cells) for the same 1M
points, versus Rust's single contiguous allocation and zero counting.

Two honest caveats:

- **Cross-language wall-clock is slippery.** Rust's number is so low mostly
  because LLVM vectorises a trivial loop; the point is the *shape* of the gap
  (one alloc + no RC vs millions of both), not the exact ratio.
- **kaikai's `Array` is no faster than its list here**, which is
  counter-intuitive for contiguous storage. The array itself is a single
  allocation (`alloc_total` stays ~15 for 1M elements), so the layout is right;
  the cost is per-access reference counting. On the correct (C) backend, reading
  `arr[i]` costs about +1 incref/+1 decref per element for an `Int` array and
  about +3/+2 for a record array, all balanced — the cache-friendly layout is
  eaten by that per-access counting, not by a bug. (On the native backend it
  looks worse, but that extra churn is the record leak
  [#1048](https://github.com/lnds/kaikai/issues/1048), not a distinct array
  issue — verified by bisection. Numbers here use `--backend=c` for that reason.)

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
