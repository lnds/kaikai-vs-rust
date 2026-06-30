# Discarded — self-referential struct

**Candidate claim (Category A):** "A struct that holds a reference into its
own field needs `Pin` + `unsafe` in Rust; kaikai does it directly."

**Verdict: discarded.** Not because the claim is false on the Rust side — it
is true — but because there is **no honest kaikai side to show**. The reason
is more interesting than a win would have been.

## The Rust side is real

[`selfref_fails.rs`](selfref_fails.rs) — a struct owning a `String` and a
slice into that same `String`:

```rust
struct SelfRef {
    data: String,
    slice: &'static str, // wants to point into `data`
}

fn main() {
    let s = String::from("hello world");
    let r = &s[0..5];
    let sr = SelfRef { data: s, slice: r };
    println!("{} / {}", sr.data, sr.slice);
}
```

`rustc --edition 2024 selfref_fails.rs`:

```text
error[E0597]: `s` does not live long enough
error[E0505]: cannot move out of `s` because it is borrowed
```

Moving `s` into the struct invalidates any reference into `s`, because the
reference is tied to a stack address that the move changes. This is the
classic problem that drives `Pin`, `ouroboros`, and `unsafe` self-referential
types in real Rust.

## Why there is no kaikai counterpart

The bug exists in Rust because Rust has the feature that creates it: a
**reference to the interior of a value** (`&s[0..5]`) whose validity depends
on that value's address staying put. kaikai has no such thing. There are no
borrows, no interior pointers, no addresses you can name. Searching the
language reference confirms it:

```text
$ kai info -k pointer   →  no topics matched 'pointer'
$ kai info -k borrow    →  no topics matched 'borrow'
$ kai info -k address   →  no topics matched 'address'
```

So you **cannot even write the buggy program** in kaikai — the construct that
makes it ill-formed does not exist. A "parser that owns its input and tracks
a cursor" is written with an integer offset into the value (a 1-cell
`Array[Int]`), which is exactly what the Rust community recommends as the
*fix* for self-reference too. Both languages end up at the same place
(indices, not interior pointers); kaikai just never had the pointer to begin
with.

## Why this is not a kaikai win

Calling this a kaikai advantage would be dishonest. kaikai does not *solve*
self-reference; it lacks the primitive that makes self-reference a problem.
The same absence has a cost, and that cost is its own (honest) Rust win:

- **No interior pointers means no control over layout.** When you genuinely
  need a pointer into a value — a `#[repr(C)]` struct for FFI, a flat buffer
  with offsets computed once, zero-copy parsing that hands out borrowed
  sub-slices — Rust gives you that with full safety and zero RC. kaikai
  cannot express it; it routes everything through values behind Perceus
  reference counting.

That is the dual of [case 4 / case 6](../../README.md): the feature whose
*absence* removes the self-reference footgun is the same feature whose
*presence* lets Rust win on layout and zero-cost borrowing. Honest comparison
keeps both halves.
