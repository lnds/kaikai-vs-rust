# Case 4 — tree rewrite and in-place reuse

**Category B — a clean kaikai win.** Rewriting every leaf of a tree (`+1`) is
the textbook Perceus showcase: pure functional code that the compiler turns
into in-place mutation when the input is uniquely owned, with no `clone` and no
`Rc` written by hand — and it stays correct (allocates a fresh copy) when the
value is shared. Rust can match the performance but only by asking for it
explicitly with `Rc::make_mut`.

> History note: on kaikai 0.94 the native backend had a soundness bug here — it
> reused nodes in place even when the value was shared, corrupting the original
> ([lnds/kaikai#995](https://github.com/lnds/kaikai/issues/995)). That bug is
> **fixed as of 0.98** and still fixed on 0.99; native and C now agree and both
> are correct. The case is re-verified below against 0.99.

## Part 1 — in-place reuse for free (unique owner)

When the original tree is consumed and not read again, Perceus reuses its
nodes in place. [`kaikai/reuse_unique.kai`](kaikai/reuse_unique.kai):

```kaikai
fn inc_leaves(t: Tree) : Tree {
  case Leaf(n)    -> Leaf(n + 1)
  case Node(l, r) -> Node(inc_leaves(l), inc_leaves(r))
}

fn main() : Int / Stdout = {
  let t  = Node(Node(Leaf(1), Leaf(2)), Leaf(3))
  let t2 = inc_leaves(t)        # t never used again -> unique owner
  Stdout.print("new=#{int_to_string(sumt(t2))}")   # 9
}
```

`KAI_TRACE_RC=1 kai run reuse_unique.kai`:

```text
new=9
[KAI_TRACE_RC]   reuse_in_place=5
```

`reuse_in_place=5` — all five nodes reused, zero fresh allocations for the
rewrite. The source is plain immutable functional code; no `Rc`, no
`make_mut`, no `clone`.

### Rust's explicit equivalent

Rust can get the same reuse, but you ask for it by name with `Rc::make_mut`.
[`rust/tree_makemut.rs`](rust/tree_makemut.rs):

```rust
fn inc_leaves(t: &mut Rc<Tree>) {
    match Rc::make_mut(t) {           // mutate in place IF strong_count == 1
        Tree::Leaf(n) => *n += 1,
        Tree::Node(l, r) => { inc_leaves(l); inc_leaves(r); }
    }
}
```

Output: `unique new=9`. So on the unique-owner path it is a genuine
ergonomics win for kaikai: **Perceus gives you `make_mut`'s reuse without
writing `make_mut` or `Rc`.**

## Part 2 — shared data is preserved (both languages, both backends)

Now keep the original alive after the rewrite — the data is *shared*, so a
correct implementation must **not** mutate it in place.
[`kaikai/aliased_corruption.kai`](kaikai/aliased_corruption.kai) (the name is
historical — it no longer corrupts):

```kaikai
fn main() : Int / Stdout = {
  let t = Node(Node(Leaf(1), Leaf(2)), Leaf(3))   # sum 6
  let s_before     = sumt(t)         # 6
  let t2           = inc_leaves(t)   # derive a new tree (+1 per leaf)
  let s_orig_after = sumt(t)         # original read AGAIN — must still be 6
  let s_new        = sumt(t2)        # 9
  Stdout.print("before=#{int_to_string(s_before)} orig_after=#{int_to_string(s_orig_after)} new=#{int_to_string(s_new)}")
}
```

Both backends now agree and are correct:

```text
$ kai build --backend=native aliased_corruption.kai -o t && ./t
before=6 orig_after=6 new=9

$ kai build --backend=c aliased_corruption.kai -o t && ./t
before=6 orig_after=6 new=9
```

`orig_after=6` — the original is untouched, `t2` is the new tree. Perceus sees
the shared node (refcount > 1) and allocates a fresh one instead of reusing in
place, exactly like Rust's `Rc::make_mut` clones a shared spine.

### Rust's shared case

[`rust/tree_makemut.rs`](rust/tree_makemut.rs), shared case, prints `shared
original=30 derived=32`, and [`rust/tree_rebuild_rc.rs`](rust/tree_rebuild_rc.rs)
prints `orig=6 inc=9` — original preserved in both. So the two languages land
in the same place: reuse when unique, copy when shared. The difference is that
kaikai decides it automatically from the code, whereas in Rust you pick the
persistent rebuild or `make_mut` explicitly.

## Reproduce

```sh
cd rust
rustc --edition 2024 tree_rebuild_rc.rs -o trc && ./trc   # orig=6 inc=9
rustc --edition 2024 tree_makemut.rs   -o tmm && ./tmm    # unique new=9 / shared original=30 derived=32
cd ../kaikai
KAI_TRACE_RC=1 kai run reuse_unique.kai                          # new=9 / reuse_in_place=5
kai build --backend=native aliased_corruption.kai -o t && ./t   # before=6 orig_after=6 new=9
kai build --backend=c      aliased_corruption.kai -o t && ./t   # before=6 orig_after=6 new=9
```

The `## Performance` table uses depth-parametrized versions (N from `argv`):

```sh
rustc --edition 2024 -O rust/bench_makemut.rs -o /tmp/bm && /tmp/bm 22   # sum=8388608
kai build --release kaikai/bench_tree.kai     -o /tmp/bt && /tmp/bt 22   # sum=8388608
```

## Performance at scale (depth-20 tree ≈ 2.1M nodes)

Rewriting every leaf of a `build(d)` tree and summing the result, unique-owner
path (`rustc -O` with `black_box`, kaikai `--release`); min of several warm
runs, `KAI_TRACE_RC` for allocation behavior. N is read from `argv` so `rustc`
cannot constant-fold. Wall-clock table measured on **kaikai 0.99.3 / rustc
1.92.0**; the RC behavior (`reuse_in_place` = all nodes, shared path copies
correctly) re-confirmed by re-running on **0.99.6**:

| | depth-20 (~2.1M nodes) | depth-22 (~8.4M nodes) | reuse / allocation |
|---|---:|---:|---|
| Rust `Rc::make_mut` (`-O`) | ~0.04 s | ~0.17 s | in-place when `strong_count==1`, ~0 alloc |
| kaikai native              | ~0.03 s | ~0.13 s | `reuse_in_place` = all nodes, ~0 alloc |
| kaikai C                   | ~0.03 s | ~0.11 s | `reuse_in_place` = all nodes, ~0 alloc |

**The three land within a whisker of each other** — this is the point, not a
kaikai throughput win. On the unique-owner path both languages converge on the
*same algorithm*: mutate the existing nodes in place, allocate nothing. Rust
does it because you wrote `Rc::make_mut`; kaikai does it because Perceus saw the
value was uniquely owned. Same work, same speed; the difference is who asked for
it. (kaikai edging ahead here is within cross-language timing noise; the honest
claim is parity, not a lead.)

Every one of kaikai's nodes is reused in place (`reuse_in_place=2097151` at
depth 20, `=8388607` at depth 22; `leaked=4`) — essentially **no new
allocation**, the Perceus win the source never had to ask for. `leaked` stays at
4 (just the format strings): the tree is an ADT `Leaf|Node` reused in place, not
retained data, so nothing accumulates.

**Native codegen caught up on 0.99.** On 0.98 the C backend was ~2.5× faster
than native on this workload; on 0.99 they are within ~1.2× (native 0.13 s vs C
0.11 s at 8.4M nodes), the gap having largely closed. The reuse story is
unchanged — both do full in-place reuse — but the earlier throughput asterisk
against native is now small.

## Where Rust wins / the trade-off

- **kaikai wins on ergonomics, on both paths.** Unique-owner reuse is
  automatic (`reuse_in_place=5`, no `Rc`/`make_mut`), and the shared case is
  transparently correct (copy-on-shared, no annotation). Rust matches the
  behavior but you choose it by hand: `Rc::make_mut` for reuse, or a plain
  rebuild for persistence.
- **Rust's cost is explicit; kaikai's is implicit.** With `Rc`, you can see in
  the type exactly where reference counting happens and reason about
  `strong_count`. kaikai's Perceus counting is uniform and invisible — a small
  runtime cost you do not control per-call. When you need to *guarantee* no
  copy or reason about exactly when reuse fires, Rust's explicitness is the
  advantage; see [case 6](../06-flat-layout-no-rc/) for where that RC cost is
  the thing Rust avoids entirely.
- **The earlier soundness bug is the reason this repo runs everything.** On
  0.94 this case was a partial *Rust* win because kaikai's native backend
  corrupted shared data. Only running it caught that; only re-running it under
  0.98 caught the fix. An asserted comparison would have been wrong in both
  directions at different times.
