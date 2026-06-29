# Case 4 — tree rewrite and in-place reuse (and a sharp Perceus footgun)

**Category B — with a major, honest caveat that partly flips it.** Rewriting
every leaf of a tree (`+1`) is the textbook Perceus showcase: pure functional
code that the compiler turns into in-place mutation when the input is uniquely
owned, with no `clone` and no `Rc` written by hand. That half is real and
measured below. But the **same mechanism corrupts shared data** in kaikai
0.93 for user-defined recursive sum types — exactly the bug class Rust's
borrow checker exists to prevent. Both halves are shown, because publishing
only the first would be dishonest.

## Part 1 — the win: in-place reuse for free (unique owner)

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

## Part 2 — the footgun: shared data gets corrupted (kaikai 0.93)

Now keep the original alive after the rewrite — the data is *shared*.
[`kaikai/aliased_corruption.kai`](kaikai/aliased_corruption.kai):

```kaikai
fn main() : Int / Stdout = {
  let t = Node(Node(Leaf(1), Leaf(2)), Leaf(3))   # sum 6
  let s_before    = sumt(t)         # 6
  let t2          = inc_leaves(t)   # derive a new tree (+1 per leaf)
  let s_orig_after = sumt(t)        # original read AGAIN — expect still 6
  let s_new       = sumt(t2)        # expect 9
  Stdout.print("before=#{int_to_string(s_before)} orig_after=#{int_to_string(s_orig_after)} new=#{int_to_string(s_new)}")
}
```

`kai run aliased_corruption.kai`:

```text
before=6 orig_after=9 new=9
```

**`orig_after=9`. The original tree changed from 6 to 9.** `inc_leaves(t)`
reused `t`'s nodes in place even though `t` is read afterwards, mutating data
that should have been immutable. This reproduces at both `--debug` (`-O0`)
and `--release` (`-O2`), so it is not an optimisation artifact. It happens for
user-defined recursive sum types (`Tree`, a custom `ICons` list both corrupt);
the **builtin** list with the map pipe (`xs | (x => x + 1)`) preserves the
original correctly (`ys=9 xs=6`), which points at the unique-owner analysis
for user ADTs as the culprit rather than the Perceus model itself.

### Rust does not have this footgun

[`rust/tree_makemut.rs`](rust/tree_makemut.rs), shared case:

```text
shared original=30 derived=32
```

When the `Rc` is shared (`strong_count > 1`), `make_mut` **clones** the shared
spine, so the original (`30`) is untouched and `derived` is `32`. And
[`rust/tree_rebuild_rc.rs`](rust/tree_rebuild_rc.rs), the plain persistent
rebuild, prints `orig=6 inc=9` — original preserved. In Rust, mutating shared
data in place without anyone noticing is precisely what the type system
forbids; you either get a fresh clone (`make_mut`) or a compile error.

## Reproduce

```sh
cd rust
rustc --edition 2021 tree_rebuild_rc.rs -o trc && ./trc   # orig=6 inc=9
rustc --edition 2021 tree_makemut.rs   -o tmm && ./tmm    # unique new=9 / shared original=30 derived=32
cd ../kaikai
KAI_TRACE_RC=1 kai run reuse_unique.kai                   # new=9 / reuse_in_place=5
kai run aliased_corruption.kai                            # before=6 orig_after=9 new=9  <- bug
```

## Where Rust wins / the trade-off

- **On the unique-owner path, kaikai wins on ergonomics:** automatic in-place
  reuse, no `Rc`/`make_mut` ceremony, measured `reuse_in_place=5`. If your
  code is linear (you transform a value and discard the old binding), Perceus
  is a real, free win here.
- **On the shared path, Rust wins on correctness, decisively.** kaikai 0.93's
  reuse analysis for user-defined recursive ADTs mutates aliased data in
  place, silently corrupting a value the program still holds. The borrow
  checker makes that class of bug *unrepresentable*; `Rc::make_mut` makes the
  safe choice (clone-on-shared) automatic. This is the strongest pro-Rust
  result in the whole repo, and it came out of trying to build a pro-kaikai
  case — which is exactly why every claim here is run, not asserted.
- The Perceus *model* (as in Koka) is sound; this looks like an
  implementation gap in kaikai 0.93's uniqueness analysis for user ADTs, not
  a flaw in reference-counted reuse as an idea. But a repo aimed at a skeptic
  reports the behavior of the compiler that actually shipped, and this is what
  it does today.
