# Case 4 — tree rewrite and in-place reuse (and a sharp Perceus footgun)

**Category B — with a major, honest caveat that partly flips it.** Rewriting
every leaf of a tree (`+1`) is the textbook Perceus showcase: pure functional
code that the compiler turns into in-place mutation when the input is uniquely
owned, with no `clone` and no `Rc` written by hand. That half is real and
measured below. But under kaikai 0.94's **default native (LLVM) backend** the
same mechanism **corrupts shared data** for user-defined recursive sum types —
exactly the bug class Rust's borrow checker exists to prevent. Crucially, the
`--backend=c` build of the identical program is correct, which pins the fault
on the native code generator, not the Perceus model. All of this is shown,
because publishing only the happy half would be dishonest.

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

Build it with the **default native (LLVM) backend** and run:

```text
$ kai build --backend=native aliased_corruption.kai -o t && ./t
before=6 orig_after=9 new=9
```

**`orig_after=9`. The original tree changed from 6 to 9.** `inc_leaves(t)`
reused `t`'s nodes in place even though `t` is read afterwards, mutating data
that should have been immutable.

### …but the C backend gets it right — so the model is sound

The same source, built with `--backend=c`, preserves the original:

```text
$ kai build --backend=c aliased_corruption.kai -o t && ./t
before=6 orig_after=6 new=9
```

`orig_after=6` — correct. The original is untouched, `t2` is the new tree.
This is the decisive datum: **the bug is in kaikai 0.94's native (LLVM)
codegen, not in the Perceus model or its uniqueness analysis.** Three controls
confirm it:

1. **C backend, same program:** correct (`orig_after=6`).
2. **Native backend, builtin list** (`xs | (x => x + 1)`): correct
   (`ys=9 xs=6`) on both backends — only user-defined recursive ADTs corrupt,
   and only under native.
3. **Unique-owner reuse works on both backends** (`reuse_in_place=5` above):
   the analysis that decides "this is safe to reuse" is right; the native
   code it emits for the *shared* case is what's wrong.

So the uniqueness analysis correctly *would* clone here (the C backend proves
it), but the native backend emits in-place mutation anyway. That is a
soundness bug in one codegen path, not a flaw in reference-counted reuse as an
idea.

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
rustc --edition 2024 tree_rebuild_rc.rs -o trc && ./trc   # orig=6 inc=9
rustc --edition 2024 tree_makemut.rs   -o tmm && ./tmm    # unique new=9 / shared original=30 derived=32
cd ../kaikai
KAI_TRACE_RC=1 kai run reuse_unique.kai                          # new=9 / reuse_in_place=5
kai build --backend=native aliased_corruption.kai -o t && ./t   # orig_after=9  <- native bug
kai build --backend=c      aliased_corruption.kai -o t && ./t   # orig_after=6  <- C is correct
```

## Where Rust wins / the trade-off

- **On the unique-owner path, kaikai wins on ergonomics:** automatic in-place
  reuse, no `Rc`/`make_mut` ceremony, measured `reuse_in_place=5`. If your
  code is linear (you transform a value and discard the old binding), Perceus
  is a real, free win here.
- **On the shared path, kaikai 0.94's native backend has a soundness bug:**
  it mutates aliased data in place, silently corrupting a value the program
  still holds. Rust's borrow checker makes that class of bug *unrepresentable*,
  and `Rc::make_mut` makes the safe choice (clone-on-shared) automatic — so on
  a correctness comparison against the **native** backend, Rust wins outright.
  This came out of trying to build a pro-kaikai case, which is exactly why
  every claim here is run, not asserted.
- **But the bug is narrow and not a model flaw.** The C backend handles the
  identical program correctly, and the uniqueness analysis is right (it reuses
  only when unique). So the honest verdict is: the *idea* (Perceus
  reference-counted reuse) is sound and even ergonomically ahead of
  `Rc::make_mut` on the unique path; the *native code generator that shipped in
  0.94* has a soundness regression on shared user-ADT reuse that a skeptic can
  and should hold against it until it's fixed. A user today should prefer
  `--backend=c` for this pattern, or not rely on aliased reuse.
