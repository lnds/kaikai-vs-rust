# CLAUDE.md — kaikai-vs-rust

Adversarial, honest comparison repo between **kaikai** and **Rust**. Every
claim is proven by compiling/running code, never asserted. Local-only git (no
remote, no push — the owner reviews and publishes).

## What this repo is

- `cases/NN-*/` — one folder per blinded case. Each has `README.md`, `rust/`
  (the safe version that fails **and** the version Rust forces), `kaikai/`
  (the `.kai` that compiles and runs). The case README pastes the real `rustc`
  error and the real kaikai output.
- `discarded/` — candidate claims that turned out false under compilation,
  documented not deleted.
- Verified against **rustc 1.92.0 (edition 2024)** and **kaikai 0.94.0**.

### Case status (who wins)

| # | Case | Verdict |
|---|------|---------|
| 1 | cyclic-graph | kaikai (ergonomics) — `&` refs rejected E0506; kaikai uses `Mutable` effect |
| 3 | shared-mutable-state | mixed → Rust — `&mut` rejected E0499; kaikai actor works, shared-`var`-over-fibers **segfaults (#997)** |
| 4 | tree-rewrite-reuse | split — unique-owner reuse is a kaikai win; **shared reuse corrupts on native (#995)**, C backend correct |
| 5 | effects-vs-boilerplate | kaikai (ergonomics) — viral `async` vs discharged effect row |
| 6 | flat-layout-no-rc | **Rust** — `Vec<Point>` zero RC vs ~10k RC ops |
| — | self-referential | discarded — kaikai has no interior references; problem can't be expressed |
| — | shared-mutable-cache | discarded — `Rc<RefCell<HashMap>>` is safe, claim false |

Case numbering skips 2 (self-referential was discarded).

## ⏳ Waiting on upstream fixes — the reason to revisit

Building this surfaced **three reproducible kaikai 0.94 bugs**, filed at
`github.com/lnds/kaikai`. When any lands, re-verify and update the affected
case. Check state with `gh issue view <N> --repo lnds/kaikai --json state`.

- **#995** — native (LLVM) backend reuses recursive-ADT nodes in place when the
  value is **shared**, corrupting the original. C backend is correct.
  - Affects **case 4**. MCVE: `cases/04-tree-rewrite-reuse/kaikai/aliased_corruption.kai`.
  - When fixed: native should print `orig=3 new=5` (today it prints `orig=5 new=5`).
    Then case 4's "shared path is a Rust win" flips toward kaikai — rewrite the
    verdict, the index row, and the root bottom-line accordingly.
- **#996** — C backend emits `redefinition of kai_<var>` for a `var` named
  `x`, `state`, or `log`. Root cause: the `State` handler C template closes
  with literal `kai_x`/`kai_state`/`kai_log` instead of the real cell name.
  - Not currently surfaced in a case (we avoid those names); a fix just removes
    a footnote-worthy gotcha.
- **#997** — SIGSEGV when a spawned fiber captures a `var` (State cell), both
  backends. Reading the cell is enough; `await` not required.
  - Affects **case 3**. Repro kept as
    `cases/03-shared-mutable-state/kaikai/shared_state_fibers_CRASHES.kai`.
  - When fixed: that file should run and print `counter=15`. Then case 3 can
    use the shared-`var`-over-fibers form as the primary example instead of the
    actor, and the "mixed → Rust" verdict softens (Rust still wins on real
    parallelism — keep that concession).

## How to re-verify everything

Run the full suite (re-create it if gone — it just builds + runs every example
and checks the expected error/output/exit):
the canonical commands are in each case's `## Reproduce` section. Quick loop:

```sh
# Rust side — edition 2024 (latest), the fairness baseline
rustc --edition 2024 <case>/rust/<file>.rs -o /tmp/x   # or -O for case 6
# kaikai side — test BOTH backends where a bug is backend-specific
kai build --backend=native <case>/kaikai/<file>.kai -o /tmp/k && /tmp/k
kai build --backend=c      <case>/kaikai/<file>.kai -o /tmp/k && /tmp/k
```

True exit codes: run the built binary directly (`./bin; echo $?`), not through
a pipe — `139` = SIGSEGV. RC behavior: `KAI_TRACE_RC=1 ./bin` prints
`reuse_in_place`, `incref_total`, `decref_total`, `alloc_total` to **stderr**.

## Environment gotchas

- The user's shell prints a noisy `zoxide` doctor warning on every `kai`/`git`
  invocation (from `~/.zshrc` line ~322). It is harmless. Filter it:
  `2>&1 | grep -vE "zoxide|ajeetdsouza|persists|initialized|Disable|Please ensure"`.
  A clean wrapper script (`#!/bin/zsh -f` + `export _ZO_DOCTOR=0` + `exec kai`)
  also works.
- **`kai info <topic>` is the source of truth for kaikai syntax** — do not
  extrapolate from Haskell/ML/Rust. `kai info syntax` has a NOT-IN-KAIKAI
  section of false friends. `kai doc <module>` for stdlib.
- kaikai facts that bit during this work: `[T]` is a linked list, `Array[T]` is
  the indexable array (`array_make`, `from_list`); tuples are `Pair`/`Triple`
  records (`.fst`/`.snd`); mutating an `Array` cell or `Ref` needs the
  `Mutable` effect in the row; `var` desugars to a `State` handler; the
  idiomatic concurrent-state form is an **actor** (`with_mailbox`), not a
  shared `var`.

## Conventions

- Commits: Conventional, English, no Claude attribution.
- **Local only** — never `git push` or `gh repo create` for this repo. The
  owner publishes. (The three bug issues in `lnds/kaikai` were filed on
  explicit request — that is a different repo.)
- READMEs are in English (the comparison is for a skeptical Rust audience).
- Honesty rule: if a safe Rust version compiles, the claim is false → move it
  to `discarded/`. Every case keeps a "Where Rust wins / trade-off" section.
