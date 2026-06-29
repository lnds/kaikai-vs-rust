// Attempt: a struct holding a reference into its own data.
struct SelfRef {
    data: String,
    // a slice pointing into `data`
    slice: &'static str,
}

fn main() {
    let s = String::from("hello world");
    let r = &s[0..5];
    let sr = SelfRef { data: s, slice: r };
    println!("{} / {}", sr.data, sr.slice);
}
