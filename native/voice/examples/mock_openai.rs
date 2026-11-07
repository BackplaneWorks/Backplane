//! A stand-in for the OpenAI API, for rigs (phones, the hub end to end): the
//! same mock as the tests (tests/common), on 127.0.0.1. Only the key `good`
//! is accepted; each commit of 0.1 s or more of audio is answered `segN <ms>ms`.
//!   cargo run --offline --example mock_openai -- [port]
//! It prints `OPENAI_BASE_URL=http://127.0.0.1:<port>/v1` and serves until killed.
#[path = "../tests/common/mod.rs"]
mod common;

fn main() {
    let port: u16 = std::env::args().nth(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    let m = common::Mock::start_on(port);
    println!("OPENAI_BASE_URL={}", m.base);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
