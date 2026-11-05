// Effects-host spike. Rust owns sockets and SQLite; the policy (the real /hello
// answer, the history page's rows -> blocks -> ids) is native Bend in a child
// process (native/bend/policy.bend), handed one request file per call.
use rusqlite::{Connection, OpenFlags};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Instant;

struct Cfg { db: String, policy: String, version: String, commit: String, busy: String, tmp: String }
#[derive(Default)]
struct Stats { n: AtomicU64, sql_us: AtomicU64, ser_us: AtomicU64, policy_us: AtomicU64, fmt_us: AtomicU64,
               hello_n: AtomicU64, hello_us: AtomicU64, req_bytes: AtomicU64, rep_bytes: AtomicU64 }
static SEQ: AtomicU64 = AtomicU64::new(0);
thread_local! { static DB: RefCell<Option<Connection>> = RefCell::new(None); }

const PAGE_SQL: &str = "SELECT * FROM (SELECT * FROM (SELECT seq, ord, id, thread, src, kind, role, akind, CASE WHEN kind = 'msg' THEN text ELSE substr(text, 1, 513) END AS text, at FROM entry WHERE thread = ?1 AND seq < ?2 ORDER BY seq DESC LIMIT ?3) UNION ALL SELECT * FROM (SELECT seq, ord, id, thread, src, kind, role, akind, CASE WHEN kind = 'msg' THEN text ELSE substr(text, 1, 513) END AS text, at FROM entry WHERE src = ?1 AND src <> '' AND seq < ?2 ORDER BY seq DESC LIMIT ?3)) ORDER BY seq DESC LIMIT ?3";
const SEQ_AT: &str = "SELECT coalesce((SELECT seq FROM entry WHERE ord >= ?1 ORDER BY ord LIMIT 1), 140737488355328)";
const TOP: i64 = 140737488355328;

fn esc(s: &str, out: &mut String) {
    for c in s.chars() { match c { '\\' => out.push_str("\\\\"), '\t' => out.push_str("\\t"), '\n' => out.push_str("\\n"), c => out.push(c) } }
}

// one call to the native Bend policy: request text in a file, answer on stdout
fn policy(cfg: &Cfg, request: &str) -> String {
    let path = format!("{}/req-{}-{}", cfg.tmp, std::process::id(), SEQ.fetch_add(1, Relaxed));
    std::fs::write(&path, request).expect("write request");
    let out = Command::new(&cfg.policy).arg(&path).output().expect("spawn policy");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn hello_body(cfg: &Cfg, st: &Stats) -> Vec<u8> {
    let t = Instant::now();
    let hex = policy(cfg, &format!("hello\t{}\t{}\t{}\n", cfg.version, cfg.busy, cfg.commit));
    let hex = hex.trim();
    let bytes: Vec<u8> = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap_or(0)).collect();
    st.hello_n.fetch_add(1, Relaxed); st.hello_us.fetch_add(t.elapsed().as_micros() as u64, Relaxed);
    bytes
}

fn history(cfg: &Cfg, st: &Stats, thread: &str) -> String {
    let t0 = Instant::now();
    let mut req = String::with_capacity(64 * 1024);
    req.push_str("page\t"); req.push_str(thread); req.push('\n');
    DB.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() { *slot = Some(Connection::open_with_flags(&cfg.db, OpenFlags::SQLITE_OPEN_READ_ONLY).expect("open db")); }
        let db = slot.as_ref().unwrap();
        let seq: i64 = db.query_row(SEQ_AT, [TOP], |r| r.get(0)).unwrap();
        let mut q = db.prepare_cached(PAGE_SQL).unwrap();
        let mut rows = q.query(rusqlite::params![thread, seq, 49i64]).unwrap();
        let t1 = Instant::now();
        st.sql_us.fetch_add(t1.duration_since(t0).as_micros() as u64, Relaxed);
        while let Some(r) = rows.next().unwrap() {
            for i in 0..10 {
                if i > 0 { req.push('\t'); }
                let v: rusqlite::types::Value = r.get(i).unwrap();
                match v {
                    rusqlite::types::Value::Integer(n) => req.push_str(&n.to_string()),
                    rusqlite::types::Value::Text(s) => esc(&s, &mut req),
                    _ => {}
                }
            }
            req.push('\n');
        }
    });
    let t2 = Instant::now();
    st.req_bytes.fetch_add(req.len() as u64, Relaxed);
    let out = policy(cfg, &req);
    let t3 = Instant::now();
    st.policy_us.fetch_add(t3.duration_since(t2).as_micros() as u64, Relaxed);
    // format the canonical answer: ids, rows newest first
    let mut ids = "";
    let mut rows: Vec<(u64, Vec<u64>)> = vec![];
    for l in out.lines() {
        if let Some(r) = l.strip_prefix("ids ") { ids = r; }
        else if let Some(r) = l.strip_prefix("row ") {
            let mut p = r.split(' ');
            let ord: u64 = p.next().unwrap().parse().unwrap();
            rows.push((ord, p.map(|x| x.parse().unwrap()).collect()));
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    let mut js = format!("{{\"thread\":{},\"ids\":[{}],\"rows\":[", json_str(thread), ids);
    for (i, (ord, lens)) in rows.iter().enumerate() {
        if i > 0 { js.push(','); }
        js.push_str(&format!("[{},[{}]]", ord, lens.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")));
    }
    js.push_str("]}");
    st.fmt_us.fetch_add(t3.elapsed().as_micros() as u64, Relaxed);
    st.ser_us.fetch_add(t2.duration_since(t0).as_micros() as u64 - 0, Relaxed);
    st.rep_bytes.fetch_add(out.len() as u64, Relaxed);
    st.n.fetch_add(1, Relaxed);
    js
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() { match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), c if (c as u32) < 32 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) } }
    o.push('"'); o
}

fn http(status: &str, ctype: &str, body: &[u8]) -> Vec<u8> {
    // the same header lines, in the same order, as the JS hub's W.Http.response
    let mut v = format!("HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n", status, ctype, body.len()).into_bytes();
    v.extend_from_slice(body); v
}

fn query_thread(target: &str) -> String {
    target.split('?').nth(1).unwrap_or("").split('&').find_map(|kv| kv.strip_prefix("thread=")).unwrap_or("").to_string()
}

fn handle(mut s: TcpStream, cfg: Arc<Cfg>, st: Arc<Stats>) {
    let mut peek = [0u8; 2048];
    let n = match s.peek(&mut peek) { Ok(n) => n, Err(_) => return };
    let head = String::from_utf8_lossy(&peek[..n]).to_lowercase();
    if head.contains("upgrade: websocket") {
        let mut ws = match tungstenite::accept(s) { Ok(w) => w, Err(_) => return };
        while let Ok(m) = ws.read() {
            match m {
                tungstenite::Message::Text(t) => {
                    let reply = if let Some(th) = t.strip_prefix("history ") { history(&cfg, &st, th) } else { "error".into() };
                    if ws.send(tungstenite::Message::Text(reply)).is_err() { break; }
                }
                tungstenite::Message::Close(_) => break,
                _ => {}
            }
        }
        return;
    }
    let mut buf = vec![0u8; 8192]; let mut len = 0;
    loop {
        let k = match s.read(&mut buf[len..]) { Ok(0) | Err(_) => return, Ok(k) => k };
        len += k;
        if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") || len == buf.len() { break; }
    }
    let text = String::from_utf8_lossy(&buf[..len]).into_owned();
    let line = text.lines().next().unwrap_or("");
    let target = line.split(' ').nth(1).unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    let reply = match path {
        "/hello" => http("200 OK", "application/cbor", &hello_body(&cfg, &st)),
        "/history" => http("200 OK", "application/json", history(&cfg, &st, &query_thread(target)).as_bytes()),
        "/stats" => {
            let g = |a: &AtomicU64| a.load(Relaxed);
            let n = g(&st.n).max(1); let hn = g(&st.hello_n).max(1);
            let rss = std::fs::read_to_string("/proc/self/status").unwrap_or_default().lines().filter(|l| l.starts_with("VmHWM") || l.starts_with("VmRSS")).collect::<Vec<_>>().join(" ").replace('\t', " ");
            let b = format!("{{\"history_n\":{},\"avg_sql_us\":{},\"avg_serialize_and_sql_us\":{},\"avg_policy_child_us\":{},\"avg_format_us\":{},\"avg_request_bytes\":{},\"avg_reply_bytes\":{},\"hello_n\":{},\"avg_hello_policy_us\":{},\"proc\":{}}}",
                g(&st.n), g(&st.sql_us) / n, g(&st.ser_us) / n, g(&st.policy_us) / n, g(&st.fmt_us) / n, g(&st.req_bytes) / n, g(&st.rep_bytes) / n, g(&st.hello_n), g(&st.hello_us) / hn, json_str(&rss));
            http("200 OK", "application/json", b.as_bytes())
        }
        _ => http("404 Not Found", "text/plain", b"not found"),
    };
    let _ = s.write_all(&reply);
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str, d: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned().unwrap_or(d.to_string());
    let cfg = Arc::new(Cfg { db: get("--db", "store.db"), policy: get("--policy", "policy"), version: get("--version", "0.14.0"),
        commit: get("--commit", "unknown"), busy: get("--busy", "0"), tmp: get("--tmp", "/tmp") });
    let st = Arc::new(Stats::default());
    let l = TcpListener::bind(format!("127.0.0.1:{}", get("--port", "3811"))).expect("bind");
    eprintln!("bphost listening on {}", l.local_addr().unwrap());
    for s in l.incoming().flatten() {
        let (c, t) = (cfg.clone(), st.clone());
        std::thread::spawn(move || handle(s, c, t));
    }
}
