# Case 5 — async coloring and capability injection

**Category B.** This is **not** an error-handling case. On errors, Rust's
`Result` + `?` is about as terse as kaikai's `Fail` + `!`, so there is no win
to claim there. The real difference is two related things Rust's type system
makes viral that kaikai's effect rows do not: **`async` coloring** and
**injecting a capability (a `fetch` / `audit` dependency) without rewriting
every signature on the path**.

## The claim

> In Rust, `async` is a viral type colour: when a leaf function becomes
> `async`, every caller up the chain must become `async`, gain `.await` at
> the call site, and the top must drive an executor. In kaikai an effect like
> `Fetch` rides the row the same way — but a `handle` **discharges** it, so
> above the handler the function is pure again: no colour reaches `main`, no
> executor. And swapping the real handler for a test stub changes **no**
> signature anywhere.
>
> The concession: Rust's `async`/trait bounds are **explicit and auditable**
> at every signature; kaikai infers effect rows in local bodies, so a
> function's true effect set can be less obvious at a glance.

## Rust — the `async` colour is viral

Start sync. [`rust/color_sync.rs`](rust/color_sync.rs) — compiles with plain
`rustc`, prints `result=27`:

```rust
fn fetch(url: &str) -> String { format!("body of {url}") }
fn parse(url: &str) -> usize { fetch(url).len() }
fn handle(url: &str) -> usize { parse(url) + 1 }
fn main() { println!("result={}", handle("http://example.com")); }
```

Now `fetch` must do real I/O, so it becomes `async`.
[`rust/color_async.rs`](rust/color_async.rs) — same business logic,
`result=27`:

```rust
async fn fetch(url: &str) -> String { format!("body of {url}") }
async fn parse(url: &str) -> usize { fetch(url).await.len() }
async fn handle(url: &str) -> usize { parse(url).await + 1 }
fn main() {
    let result = block_on(handle("http://example.com")); // executor required
    println!("result={}", result);
}
// + a std-only block_on with a no-op Waker — which itself needs `unsafe`.
```

The `diff` from sync to async touches **every signature on the path**, adds
`.await` at each call site, and forces an executor (`block_on`, here written
by hand with `unsafe` because a from-scratch executor needs it; in practice
you pull in `tokio`/`pollster`, which encapsulate that `unsafe`). None of the
*logic* changed — only the colour. That is the viral cost.

## kaikai — the effect row, discharged by a handler

[`kaikai/effect_color.kai`](kaikai/effect_color.kai) — compiles and runs,
prints `result=27`:

```kaikai
effect Fetch { get(url: String) : String }

fn fetch(url: String) : String / Fetch = Fetch.get(url)
fn parse(url: String) : Int / Fetch = fetch(url).length()
fn handle_req(url: String) : Int / Fetch = parse(url) + 1

fn driver(url: String) : Int / Stdout = {        # NOTE: no Fetch here
  handle { handle_req(url) }
  with Fetch { get(u, resume) -> resume("body of #{u}") }
}
```

Two honest observations:

1. **The row IS viral up to the handler.** `parse` and `handle_req` must
   carry `/ Fetch`, exactly as Rust's `parse`/`handle` had to carry `async`.
   kaikai does not escape that; effects propagate.
2. **The handler ends the propagation.** `driver`'s signature is `: Int /
   Stdout` — `Fetch` is *gone*. There is no executor and nothing async
   leaking to `main`. In Rust, `async` rides all the way to `main` and the
   executor; in kaikai, the row is discharged where you handle it.

## Injecting capabilities without touching signatures

[`kaikai/swap_handlers.kai`](kaikai/swap_handlers.kai) — the same
`pipeline : Int / Fetch + Audit + Fail` run under one set of handlers
(`Fetch` returns a canned body, `Audit` prints, `Fail` prints the error):

```text
[audit] fetching http://example.com
[audit] got 18 bytes
result=18
```

To run it under *test* doubles — a `Fetch` that returns fixtures, an `Audit`
that records into a list, a `Fail` that asserts — you change only the
`with { … }` blocks. `pipeline`, `fetch`, `parse` keep their exact
signatures. In Rust the equivalent injection is generics (`fn pipeline<F:
Fetcher, A: Auditor>(…)`) propagating bounds through every signature, or
`dyn Fetcher` paying a vtable at runtime.

## Reproduce

```sh
cd rust
rustc --edition 2021 color_sync.rs -o cs && ./cs      # result=27
rustc --edition 2021 color_async.rs -o ca && ./ca     # result=27
cd ../kaikai
kai run effect_color.kai                               # result=27
kai run swap_handlers.kai                              # [audit]… result=18
```

## Where Rust wins / the trade-off

- **Auditability.** Rust's `async` and trait bounds are written at every
  signature. You can read a function's type and know exactly what it needs
  and whether it suspends. kaikai infers rows in local bodies; a `pub`
  signature must annotate them, but inside a module the effect set is
  inferred, so "what can this actually do" can be less immediately visible.
- **`async` carries a real execution model.** Rust's colour is viral partly
  because it is *honest*: an `async fn` returns a state machine you can
  schedule, cancel, select over, and run on a chosen executor. kaikai's
  handler-based model is more ergonomic here but the scheduling story is the
  fiber runtime's, not something you assemble per-call.
- This case is about **ergonomics and propagation surface**, not capability.
  Rust can express all of it. The claim is narrowly that the *edit* to add an
  effect/await is smaller in kaikai and stops at the handler, and that
  swapping handlers needs no signature changes — both shown above, neither
  exaggerated into "Rust can't".
