// BEFORE: everything is sync. A 3-layer call chain over a "fetch".
fn fetch(url: &str) -> String {
    format!("body of {url}")
}

fn parse(url: &str) -> usize {
    let body = fetch(url);
    body.len()
}

fn handle(url: &str) -> usize {
    parse(url) + 1
}

fn main() {
    println!("result={}", handle("http://example.com"));
}
