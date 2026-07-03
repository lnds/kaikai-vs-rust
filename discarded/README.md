# Discarded cases

Candidate claims that did **not** survive compilation, kept here instead of
deleted. Conceding a refuted claim is part of the methodology: a repo that
hid its misses would be exactly the propaganda this one is trying not to be.

| Candidate | Why discarded |
|-----------|---------------|
| [self-referential](self-referential/) | Rust genuinely fails (E0597/E0505), but there is **no honest kaikai counterpart**: kaikai has no interior references, so the buggy program cannot even be written. Not a kaikai win — the feature that creates the problem is simply absent (and its absence is itself a Rust win for layout/FFI, see [case 6](../cases/06-flat-layout-no-rc/)). |
| [shared-mutable-cache](shared-mutable-cache/) | The "shared mutable cache between callbacks needs `unsafe`" claim is **false**: `Rc<RefCell<HashMap>>` is safe, idiomatic Rust and compiles fine. It is also just a re-run of [case 1](../cases/01-cyclic-graph/)'s `Rc<RefCell>` point, so it adds nothing. |

## Honorable mention — a kaikai claim that flipped

The strongest "kaikai wins" candidate, the tree-rewrite / Perceus-reuse case,
**flipped into a Rust win and then back** across compiler versions. On kaikai
0.94 the native (LLVM) backend corrupted shared data during in-place reuse for
user ADTs (making it a correctness win for Rust); the `--backend=c` build was
already correct. That bug ([lnds/kaikai#995](https://github.com/lnds/kaikai/issues/995))
is **fixed as of 0.98**, and [case 4](../cases/04-tree-rewrite-reuse/) is now a
clean kaikai win, re-verified. The case was kept (not discarded) throughout,
because tracking the flip is exactly what "run every claim" buys.
