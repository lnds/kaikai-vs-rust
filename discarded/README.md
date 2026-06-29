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
**partly flipped into a Rust win** when run: kaikai 0.93 corrupts shared data
during in-place reuse for user ADTs. It was kept (not discarded) and reported
honestly in [case 4](../cases/04-tree-rewrite-reuse/), because the failure is
informative rather than fatal to the comparison.
