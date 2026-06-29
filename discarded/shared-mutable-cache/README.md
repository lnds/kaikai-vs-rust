# Discarded — shared mutable cache between callbacks

**Candidate claim (Category A):** "A mutable cache shared between several
callbacks needs `unsafe` in Rust."

**Verdict: false. Discarded.**

The safe, idiomatic Rust version compiles and runs.
[`cache_compiles_safely.rs`](cache_compiles_safely.rs):

```rust
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

let cache: Rc<RefCell<HashMap<String, i32>>> = Rc::new(RefCell::new(HashMap::new()));

let writer = { let c = Rc::clone(&cache); move |k: &str, v: i32| { c.borrow_mut().insert(k.into(), v); } };
let reader = { let c = Rc::clone(&cache); move |k: &str| c.borrow().get(k).copied() };

writer("a", 1); writer("b", 2);
println!("a={:?} b={:?} miss={:?}", reader("a"), reader("b"), reader("z"));
```

`rustc --edition 2021 cache_compiles_safely.rs && ./a.out`:

```text
a=Some(1) b=Some(2) miss=None
```

No `unsafe` anywhere. `Rc<RefCell<T>>` is the standard, safe way to share
mutable state between closures in single-threaded Rust, and several callbacks
each holding an `Rc::clone` of the same cell is exactly what it is for.

Two reasons this is discarded rather than published:

1. **The claim is simply wrong** — there is no compiler error to show, so
   there is no Category-A "Rust is forced into `unsafe`" result here.
2. **It would be redundant.** The only honest framing left ("`Rc<RefCell>` is
   runtime RC + interior mutability, more ceremony than a kaikai `var`/effect")
   is the *same* point already made, with a real borrow-check failure behind
   it, in [case 1](../../cases/01-cyclic-graph/). Repeating it with a weaker
   example would pad the repo, not strengthen it.
