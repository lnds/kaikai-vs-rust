// AFTER: fetch becomes async (e.g. real network). The `async` color is
// VIRAL: parse must become async, handle must become async, main must
// drive an executor. Every signature on the call path changed, plus
// `.await` at every call site. No business logic changed — only the color.
//
// std-only block_on so this compiles with just `rustc` (no crates). The
// executor itself is part of the cost the claim is about.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

async fn fetch(url: &str) -> String {
    format!("body of {url}")
}

async fn parse(url: &str) -> usize {
    let body = fetch(url).await;
    body.len()
}

async fn handle(url: &str) -> usize {
    parse(url).await + 1
}

fn main() {
    let result = block_on(handle("http://example.com"));
    println!("result={}", result);
}

// --- minimal std-only executor (no-op waker, single poll loop) ---
fn block_on<F: Future>(mut fut: F) -> F::Output {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    // SAFETY: `fut` is owned and never moved after being pinned here.
    let mut fut = unsafe { Pin::new_unchecked(&mut fut) };
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => continue,
        }
    }
}

fn noop_waker() -> Waker {
    const VTABLE: RawWakerVTable =
        RawWakerVTable::new(|_| RAW, |_| {}, |_| {}, |_| {});
    const RAW: RawWaker = RawWaker::new(std::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(RAW) }
}
