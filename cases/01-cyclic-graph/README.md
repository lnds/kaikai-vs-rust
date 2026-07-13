# Case 1 — a two-node pointer cycle

**Category A.** Two nodes that point at each other: `a → b → a`.

## The claim

> With plain `&` references the borrow checker rejects a mutable cyclic
> graph (E0506). The safe way out is `Rc<RefCell<Option<Rc<Node>>>>` plus
> `Weak`: reference counting at **runtime**, plus reasoning by hand about
> where to put the `Weak` so the cycle does not leak. kaikai expresses the
> same graph with a mutable `Ref[Option[Node]]` cell tracked by the `Mutable`
> effect — it never builds a strong RC cycle, so there is no `Weak` to place.

Note what the claim is **not**: it is *not* "Rust can't do this". Rust does
it (see [`rust/cycle_rc_works.rs`](rust/cycle_rc_works.rs), which compiles
and runs). The point is **where the cognitive load lives** — inside the type
(`Rc<RefCell<…>>` + manual `Weak` placement) versus an effect on the
function row.

## Rust — the idiomatic safe attempt fails

A Rust programmer's first reach is plain references. [`rust/cycle_ref_fails.rs`](rust/cycle_ref_fails.rs):

```rust
struct Node<'a> {
    id: i32,
    next: Option<&'a Node<'a>>,
}

fn main() {
    let mut a = Node { id: 1, next: None };
    let mut b = Node { id: 2, next: None };
    a.next = Some(&b);
    b.next = Some(&a); // cycle: a -> b -> a
    println!("{}", a.next.unwrap().id);
}
```

`rustc --edition 2024 rust/cycle_ref_fails.rs`:

```text
error[E0506]: cannot assign to `b.next` because it is borrowed
  --> rust/cycle_ref_fails.rs:12:5
   |
11 |     a.next = Some(&b);
   |                   -- `b.next` is borrowed here
12 |     b.next = Some(&a); // cycle: a -> b -> a
   |     ^^^^^^^^^^^^^^^^^ `b.next` is assigned to here but it was already borrowed
13 |     println!("{}", a.next.unwrap().id);
   |                    ------ borrow later used here
```

The aliasing the cycle needs (`a` borrows `b` *and* `b` borrows `a`) is
exactly what `&`-references forbid.

## Rust — what you are forced to write

[`rust/cycle_rc_works.rs`](rust/cycle_rc_works.rs) — compiles and runs:

```rust
use std::rc::{Rc, Weak};
use std::cell::RefCell;

struct Node {
    id: i32,
    next: RefCell<Option<Rc<Node>>>,
    prev: RefCell<Weak<Node>>,   // Weak, or the cycle leaks
}
// ... build a, b ; *a.next.borrow_mut() = Some(Rc::clone(&b)); etc.
```

Output: `trace=1 2 1 2 1 2`.

The shape of `Node` carries `Rc`, `RefCell`, `Option`, and `Weak`, and the
programmer must decide which edge is `Weak` to avoid a leak. That decision is
real work the compiler does not make for you, and getting it wrong is a
runtime memory leak, not a compile error.

## kaikai — the cycle directly

[`kaikai/cycle.kai`](kaikai/cycle.kai) — compiles and runs:

```kaikai
type Node = { id: Int, next: Ref[Option[Node]] }   # first-class mutable reference cell

fn make_node(id: Int) : Node / Mutable = Node { id: id, next: &None }   # `&` builds a Ref

fn step(n: Node) : Node / Mutable = match @n.next { ... }               # `@` derefs a Ref

fn main() : Int / Stdout + Mutable = {
  let a = make_node(1)
  let b = make_node(2)
  a.next := Some(b)
  b.next := Some(a)    # cycle a <-> b
  // walk 6 steps ...
}
```

Output: `trace=1 2 1 2 1 2` — identical.

> **A regression came and went on 0.99.5 → 0.99.6.** This exact file ran on
> 0.99.3, then briefly **stopped compiling on 0.99.5**: a typer regression
> (unrelated to the `Ref` cycle, which kept type-checking) lost the type of the
> reassigned record `var` `cur` when its field was read inside the loop's
> `"#{...int_to_string(cur.id)...}"` interpolation, erroring `type annotation
> needed: the receiver of '.id' is not resolved`. It was filed as
> [lnds/kaikai#1194](https://github.com/lnds/kaikai/issues/1194) with a 7-line
> repro (no `Ref`, no `import loop`) and **fixed on 0.99.6** (PR #1196) — the
> file now compiles and runs unchanged on both backends. Kept as a note because
> catching a regression by re-running is exactly why this repo re-runs every
> claim on every upgrade.

The mutation is not invisible: rewriting a node's `next` puts `Mutable` in the
function's effect row, and the compiler enforces that. The difference from Rust
is that the cyclic shape is two direct writes with no `Weak`/strong distinction
to get right, because kaikai is not building a strong-RC cycle to begin with.

> `Ref[T]` has first-class surface sugar: `&x` builds a reference, `@r` derefs
> it, and `r := v` (or `record.field := v`) writes it. On kaikai 0.99.0 the
> `:=` sugar did not yet accept a `Ref`-typed record field, so an earlier draft
> of this case used the explicit `Mutable.ref_set` / one-cell `Array` workaround;
> [lnds/kaikai#1113](https://github.com/lnds/kaikai/issues/1113) shipped that
> sugar in 0.99.1, so the case now reads as the two direct `:=` writes above.

## Reproduce

```sh
cd rust
rustc --edition 2024 cycle_ref_fails.rs        # E0506, as above
rustc --edition 2024 cycle_rc_works.rs -o crc && ./crc   # trace=1 2 1 2 1 2
cd ../kaikai
kai run cycle.kai                              # trace=1 2 1 2 1 2
```

## Where Rust wins / the trade-off

- **Rust's `Weak` is a real leak-prevention tool.** It makes the
  strong/weak edge distinction *explicit in the type*, and the programmer
  must place it correctly. kaikai sidesteps that decision here, but only
  because mutating a `Ref` cell is what lets the cycle form at all — a
  cycle of purely immutable values cannot be constructed in either language
  without some mutable cell. The trade is real either way: reference-counted
  cycles are a problem neither language makes free.
- **The borrow checker's rejection is information.** E0506 is Rust telling
  you, at compile time and at zero runtime cost, that this data shape aliases
  mutably. kaikai accepts the shape and pays a small uniform RC cost at
  runtime instead. That is the core trade of the whole repo, in one case.
- **`Rc<RefCell>` is genuinely safe.** No `unsafe`, no UB. The cost is
  ceremony and runtime RC, not soundness.
