//! `kipc-probe [--desc FILE]`: tools/kipc's NDJSON on stdin/stdout, over the hub's own modules. Ends when stdin closes.
//! `kipc-probe --golden CASES --desc FILE`: checks protobufjs's answers (test/files/kipc_golden.ts) against proto.rs for a
//! descriptor set that cannot ship in this repository, so that test runs where the runtime is. Prints a count; exit 1 on a difference.
#[path = "../../../../../native/host/src/kicad/kipc.rs"] mod kipc;
#[path = "../../../../../native/host/src/kicad/nng.rs"] mod nng;
#[path = "../../../../../native/host/src/kicad/proto.rs"] mod proto;
use kipc::Native;
use serde_json::Value;
use std::{io::Write, sync::Arc};
use tokio::io::{AsyncBufReadExt, BufReader};

fn opt(args: &[String], n: &str) -> String { args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned().unwrap_or_default() }
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => a.as_f64() == b.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().zip(y.iter()).all(|((k, p), (l, q))| k == l && same(p, q)),
        _ => a == b,
    }
}
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }
fn unhex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
fn golden(file: &str, desc: &str) -> i32 {
    let s = proto::from_bytes(&std::fs::read(desc).expect("descriptor set")).expect("a valid descriptor set");
    let cases: Value = serde_json::from_str(&std::fs::read_to_string(file).expect("cases")).expect("JSON");
    let (mut ok, mut bad) = (0, 0);
    for c in cases.as_array().unwrap() {
        let ty = c["type"].as_str().unwrap(); let t = s.message(ty).unwrap_or_else(|| panic!("no {ty}"));
        let mut good = true;
        if let Some(i) = c.get("in") {
            match (proto::encode_message(&t, i), c["hex"].as_str(), c["error"].as_str()) {
                (Ok(b), Some(h), _) if hex(&b) == h => {}
                (Err(e), _, Some(w)) if e == w => {}
                (r, h, e) => { good = false; println!("DIFF {ty} {i}: got {:?}, wanted {h:?} {e:?}", r.map(|b| hex(&b))) }
            }
        }
        if let (Some(h), Some(o)) = (c["hex"].as_str(), c.get("out")) {
            match proto::decode_message(&t, &unhex(h)) { Ok(g) if same(&g, o) => {} g => { good = false; println!("DIFF {ty} {h}: got {g:?}, wanted {o}") } }
        }
        if good { ok += 1 } else { bad += 1 }
    }
    println!("golden: {ok} same, {bad} different"); (bad != 0) as i32
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let desc = opt(&args, "--desc");
    if args.iter().any(|a| a == "--golden") { std::process::exit(golden(&opt(&args, "--golden"), &desc)) }
    let mut n = Native::new(&desc, Arc::new(|v: Value| { let mut o = std::io::stdout().lock(); let _ = writeln!(o, "{v}"); let _ = o.flush(); }));
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(l)) = lines.next_line().await {
        if l.trim().is_empty() { continue }
        match serde_json::from_str::<Value>(&l) { Ok(v) => n.handle(v), Err(_) => println!("{}", serde_json::json!({"id":null,"ok":false,"error":"bad request line"})) }
    }
    std::process::exit(0); // stdin closed: the hub is gone; servers end by --parent-pid
}
