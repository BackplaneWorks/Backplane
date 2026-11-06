//! The hub's own KiCad IPC transport: what the Bun helper's main.ts and worker.ts did (tools/kipc), inside the hub, so that no
//! Bun or JS process runs under it. It takes the helper's NDJSON objects and answers the same objects, without the pipe:
//!   {"id","sock","type","message","timeout"?,"desc"?} -> {"id","ok":true,"type","message"} | {"id","ok":false,"status"?,"error"}
//!   {"id","op":"close","sock"} drops a socket's connection
//!   {"id","op":"spawn","key","argv","memory"?,"env"?} starts a KiCad server -> {"id","ok":true}, then
//!   {"event":"line","key","text"} and {"event":"exit","key","code","tail"}
//! One task per socket owns its connection: requests go out one at a time in order, a failed one drops the connection and
//! the next dials again, and the token KiCad hands out is kept per socket. The `lib` field is ignored: there is no libnng.
//! NOTHING HERE SENDS A SIGNAL TO ANY PROCESS: a child is only ever `wait`ed for, and `kill_on_drop` stays off. Servers end
//! themselves (`--parent-pid`, `--idle-exit`, `kiapi.backplane.Shutdown`). Unix only, like the hub (the helper also did Windows' paths).
use super::{nng, proto::{self, Reply, Schema}};
use serde_json::{json, Value};
use std::{collections::{HashMap, VecDeque}, path::Path, process::Stdio, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncRead, BufReader}, process::Command, sync::{mpsc, oneshot}};

/// Where answers and events go, as the objects the helper would have written.
pub type Sink = Arc<dyn Fn(Value) + Send + Sync>;
/// What a closed connection's requests hear (replies.ts): they may have reached KiCad, so an error is not a rollback.
pub const CONN_CLOSED: &str = "KiCad IPC connection closed; request outcome unknown";
const READY: &str = "KiCad API server listening at ";
const TIMEOUT_MS: u64 = 30000;

fn fail(id: Value, error: &str, status: Option<&str>) -> Value { match status { Some(s) => json!({"id":id,"ok":false,"status":s,"error":error}), None => json!({"id":id,"ok":false,"error":error}) } }

struct Job { id: Value, schema: Arc<Schema>, ty: String, message: Value, timeout: u64 }
struct Slot { jobs: mpsc::UnboundedSender<Job>, shut: oneshot::Sender<()> }

/// One hub's transport. `handle` never blocks: the work runs on tasks and answers through the sink. Call it on the runtime.
pub struct Native { socks: HashMap<String, Slot>, desc: String, client: String, sink: Sink }
impl Native {
    /// `desc`: the descriptor set for requests that name none (the helper's --desc).
    pub fn new(desc: &str, sink: Sink) -> Native { Native { socks: HashMap::new(), desc: desc.into(), client: format!("backplane-{}", std::process::id()), sink } }

    pub fn handle(&mut self, r: Value) {
        let id = r.get("id").cloned().unwrap_or(Value::Null);
        let op = r["op"].as_str().unwrap_or(""); let sock = r["sock"].as_str().unwrap_or("").to_string();
        if op != "spawn" && sock.is_empty() { return (self.sink)(fail(id, "no sock", None)) }
        if op == "close" {
            if let Some(s) = self.socks.remove(&sock) { let _ = s.shut.send(()); } // its task answers what it holds and drops the connection
            return (self.sink)(json!({"id":id,"ok":true}))
        }
        if op == "spawn" { return self.spawn(id, &r) }
        let ty = r["type"].as_str().unwrap_or("").to_string();
        if ty.is_empty() { return (self.sink)(fail(id, "no type", None)) }
        let schema = proto::load(r.get("desc").and_then(Value::as_str).unwrap_or(&self.desc));
        if !schema.knows(&ty) { return (self.sink)(fail(id, proto::NO_SCHEMA, None)) }
        let job = Job { id, schema, ty, timeout: r["timeout"].as_u64().unwrap_or(TIMEOUT_MS), message: r.get("message").cloned().unwrap_or(Value::Null) };
        let slot = self.socks.entry(sock.clone()).or_insert_with(|| {
            let (jobs, rx) = mpsc::unbounded_channel(); let (shut, down) = oneshot::channel();
            tokio::spawn(worker(sock, rx, down, self.client.clone(), self.sink.clone()));
            Slot { jobs, shut }
        });
        if let Err(e) = slot.jobs.send(job) { (self.sink)(fail(e.0.id, CONN_CLOSED, None)) }
    }

    fn spawn(&mut self, id: Value, r: &Value) {
        let key = r["key"].as_str().unwrap_or("").to_string();
        let argv: Vec<String> = r["argv"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        if key.is_empty() || argv.is_empty() || argv.len() != r["argv"].as_array().map_or(0, Vec::len) { return (self.sink)(fail(id, "spawn needs key and argv", None)) }
        let mut env = vec![];
        match r.get("env") {
            None | Some(Value::Null) => {}
            Some(Value::Object(o)) => for (k, v) in o {
                let Some(v) = v.as_str().filter(|v| (k == "BACKPLANE_KICAD_LIBRARY" || k == "KICAD_CONFIG_HOME") && !v.contains('\0')) else { return (self.sink)(fail(id, "invalid KiCad environment", None)) };
                if !v.is_empty() { env.push((k.clone(), v.to_string())) }
            },
            Some(_) => return (self.sink)(fail(id, "invalid KiCad environment", None)),
        }
        start_server(key, argv, r["memory"].as_str().map(String::from), env, self.sink.clone());
        (self.sink)(json!({"id":id,"ok":true}))
    }
}

// One socket ------------------------------------------------------------------------------------------------------------

async fn worker(sock: String, mut rx: mpsc::UnboundedReceiver<Job>, mut down: oneshot::Receiver<()>, client: String, sink: Sink) {
    let (mut conn, mut token) = (None, String::new());
    loop {
        let job = tokio::select! { biased; _ = &mut down => break, j = rx.recv() => match j { Some(j) => j, None => break } };
        let id = job.id.clone();
        tokio::select! {
            biased;
            _ = &mut down => { sink(fail(id, CONN_CLOSED, None)); break }
            o = one(&sock, job, &mut conn, &mut token, &client) => sink(o),
        }
    }
    drop(conn); rx.close();
    while let Ok(j) = rx.try_recv() { sink(fail(j.id, CONN_CLOSED, None)) }
}
/// One request. The codec runs off the runtime's threads: a big board's reply takes a while, and a panic there must answer this request, not lose it.
async fn one(sock: &str, job: Job, conn: &mut Option<nng::Req>, token: &mut String, client: &str) -> Value {
    let Job { id, schema, ty, message, timeout } = job;
    let (s, t, tok, cl) = (schema.clone(), ty.clone(), token.clone(), client.to_string());
    let bytes = match tokio::task::spawn_blocking(move || proto::encode_request(&s, &t, &message, &tok, &cl)).await.unwrap_or_else(|e| Err(e.to_string())) {
        Ok(b) => b,
        Err(e) => return fail(id, &format!("bad message for {ty}: {e}"), Some("AS_BAD_REQUEST")),
    };
    let reply = match tokio::time::timeout(Duration::from_millis(timeout), exchange(sock, conn, &bytes)).await {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => { *conn = None; return fail(id, &e, None) }
        Err(_) => { *conn = None; return fail(id, &format!("KiCad did not answer in {} s", timeout as f64 / 1000.0), None) }
    };
    match tokio::task::spawn_blocking(move || proto::decode_response(&schema, &reply)).await.unwrap_or_else(|e| Err(e.to_string())) {
        Ok(rep) => {
            if !rep.token().is_empty() { *token = rep.token().to_string() }
            match rep { Reply::Ok { ty, message, .. } => json!({"id":id,"ok":true,"type":ty,"message":message}), Reply::Err { status, error, .. } => fail(id, &error, Some(&status)) }
        }
        Err(e) => fail(id, &format!("bad reply: {e}"), None),
    }
}
async fn exchange(sock: &str, conn: &mut Option<nng::Req>, bytes: &[u8]) -> Result<Vec<u8>, String> {
    if conn.is_none() { *conn = Some(nng::Req::dial(sock).await?) }
    match conn { Some(c) => c.request(bytes).await, None => Err(CONN_CLOSED.into()) }
}

// Starting a server -----------------------------------------------------------------------------------------------------

fn which(cmd: &str) -> Option<String> { std::env::var("PATH").ok()?.split(':').map(|d| format!("{d}/{cmd}")).find(|f| Path::new(f).exists()) }
/// Inside a systemd scope capped at `memory` when this machine has a user manager.
fn wrap(argv: Vec<String>, memory: Option<&str>) -> Vec<String> {
    let bus = std::env::var("XDG_RUNTIME_DIR").ok().filter(|r| !r.is_empty()).map(|r| format!("{r}/bus"));
    match (memory, which("systemd-run"), bus) {
        (Some(m), Some(sd), Some(b)) if cfg!(target_os = "linux") && Path::new(&b).exists() => { let mut v = vec![sd, "--user".into(), "--scope".into(), "--quiet".into(), "-p".into(), format!("MemoryMax={m}"), "--".into()]; v.extend(argv); v }
        _ => argv,
    }
}
/// KiCad may choose api-<pid>.sock when the requested path is occupied. Each socket with what tells it from one bound before.
fn sockets_at(requested: &str) -> HashMap<String, String> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let mut out = HashMap::new();
    let at = Path::new(requested); let (Some(dir), Some(base)) = (at.parent(), at.file_name()) else { return out };
    let Ok(rd) = std::fs::read_dir(dir) else { return out }; // no socket directory yet
    for e in rd.flatten() {
        let name = e.file_name(); let n = name.to_string_lossy();
        let fallback = n.strip_prefix("api-").and_then(|r| r.strip_suffix(".sock")).is_some_and(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()));
        if name != base && !fallback { continue }
        let p = e.path();
        if let Ok(st) = std::fs::metadata(&p) { // the server can replace a stale socket during this scan
            if st.file_type().is_socket() { let born = st.created().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(st.ctime_nsec() as u128); out.insert(p.to_string_lossy().to_string(), format!("{}:{}:{born}", st.dev(), st.ino())); }
        }
    }
    out
}
fn socket_arg(argv: &[String]) -> String { argv.iter().position(|a| a == "--socket").and_then(|i| argv.get(i + 1)).cloned().unwrap_or_default() }

struct Run { key: String, said: AtomicBool, done: AtomicBool, tail: Mutex<VecDeque<String>>, sink: Sink }
impl Run {
    fn keep(&self, l: &str) { let mut t = self.tail.lock().unwrap(); t.push_back(l.to_string()); if t.len() > 12 { t.pop_front(); } }
    fn line(&self, text: String) { (self.sink)(json!({"event":"line","key":self.key,"text":text})) }
    fn end(&self, code: i32, extra: Option<String>) {
        if self.done.swap(true, Ordering::SeqCst) { return }
        let t: Vec<String> = extra.into_iter().chain(self.tail.lock().unwrap().iter().cloned()).collect();
        let t = t.join("\n"); let n = t.chars().count();
        (self.sink)(json!({"event":"exit","key":self.key,"code":code,"tail":t.chars().skip(n.saturating_sub(400)).collect::<String>()}))
    }
    fn out_line(&self, l: String) { // the ready line counts once, whoever saw it first
        self.keep(&l);
        if l.starts_with(READY) && self.said.swap(true, Ordering::SeqCst) { return }
        self.line(l)
    }
}
/// Lines of a pipe to its end. KiCad's output need not be UTF-8, and the pipe must be drained whatever it holds.
async fn lines<R: AsyncRead + Unpin>(r: R, mut each: impl FnMut(String)) {
    let (mut b, mut v) = (BufReader::new(r), Vec::new());
    loop {
        v.clear();
        match b.read_until(b'\n', &mut v).await { Ok(0) | Err(_) => return, Ok(_) => {} }
        if v.last() == Some(&b'\n') { v.pop(); } if v.last() == Some(&b'\r') { v.pop(); }
        each(String::from_utf8_lossy(&v).into_owned());
    }
}
fn start_server(key: String, argv0: Vec<String>, memory: Option<String>, env: Vec<(String, String)>, sink: Sink) {
    let at = socket_arg(&argv0); let before = sockets_at(&at);
    let argv = wrap(argv0, memory.as_deref());
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, &argv[0]);
    c.args(&argv[1..]).envs(env).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).current_dir("/").kill_on_drop(false);
    let run = Arc::new(Run { key, said: AtomicBool::new(false), done: AtomicBool::new(false), tail: Mutex::new(VecDeque::new()), sink });
    tokio::spawn(async move {
        let mut child = match c.spawn() { Ok(c) => c, Err(e) => return run.end(127, Some(e.to_string())) };
        let (out, err) = (child.stdout.take(), child.stderr.take());
        if !at.is_empty() { // KiCad buffers stdout on a pipe: report its freshly bound socket; the hub still requires Ping and a matching GetVersion
            let run = run.clone();
            tokio::spawn(async move {
                while !run.done.load(Ordering::SeqCst) && !run.said.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if let Some((path, _)) = sockets_at(&at).into_iter().find(|(p, i)| before.get(p) != Some(i)) {
                        if !run.done.load(Ordering::SeqCst) && !run.said.swap(true, Ordering::SeqCst) { run.line(format!("{READY}{path}")) }
                        return
                    }
                }
            });
        }
        let (a, b) = (run.clone(), run.clone());
        let so = tokio::spawn(async move { if let Some(o) = out { lines(o, |l| a.out_line(l)).await } });
        let se = tokio::spawn(async move { if let Some(e) = err { lines(e, |l| b.keep(&l)).await } });
        let code = child.wait().await.map(|s| s.code().unwrap_or(128)).unwrap_or(1); // no code: a signal ended it, and not one of ours
        let _ = so.await; let _ = se.await;
        run.end(code, None);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::nng::tests as stand_in;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

    fn native(desc: &str) -> (Native, UnboundedReceiver<Value>) { let (t, r) = unbounded_channel(); (Native::new(desc, Arc::new(move |v| { let _ = t.send(v); })), r) }
    async fn next(r: &mut UnboundedReceiver<Value>) -> Value { tokio::time::timeout(Duration::from_secs(10), r.recv()).await.expect("an answer in time").unwrap() }
    fn hex_of(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
    /// The real server's answer to GetVersion (read off the wire, bp.4) under the token "tk".
    fn version_reply() -> Vec<u8> { hex_of("0a040a02746b120208011a5c0a3c747970652e676f6f676c65617069732e636f6d2f6b696170692e636f6d6d6f6e2e636f6d6d616e64732e47657456657273696f6e526573706f6e7365121c0a1a080a1063221431302e39392e302d756e6b6e6f776e2d62702e34") }

    #[tokio::test] async fn request_and_token_through_a_stand_in() {
        let (path, l) = stand_in::sock("native");
        let srv = tokio::spawn(async move {
            let (mut s, _) = stand_in::accept(&l).await;
            let (id, first) = stand_in::read_req(&mut s).await.unwrap(); stand_in::write_rep(&mut s, id, &version_reply()).await;
            let (id, second) = stand_in::read_req(&mut s).await.unwrap(); stand_in::write_rep(&mut s, id, &version_reply()).await;
            (first, second)
        });
        let (mut n, mut r) = native("");
        n.handle(json!({"id":1,"sock":path,"type":"kiapi.common.commands.GetVersion","message":{}}));
        let o = next(&mut r).await;
        assert_eq!(o, json!({"id":1,"ok":true,"type":"kiapi.common.commands.GetVersionResponse","message":{"version":{"major":10,"minor":99,"full_version":"10.99.0-unknown-bp.4"}}}));
        n.handle(json!({"id":"two","sock":path,"type":"kiapi.common.commands.GetVersion"}));
        assert_eq!(next(&mut r).await["id"], json!("two"));
        let (first, second) = srv.await.unwrap();
        assert!(!first.windows(2).any(|w| w == b"tk"), "no token before KiCad gave one"); assert!(second.windows(2).any(|w| w == b"tk"), "the token goes back");
    }
    #[tokio::test] async fn the_helpers_errors() {
        let (mut n, mut r) = native("");
        n.handle(json!({"id":1,"type":"x"})); assert_eq!(next(&mut r).await, json!({"id":1,"ok":false,"error":"no sock"}));
        n.handle(json!({"id":2,"sock":"/x"})); assert_eq!(next(&mut r).await, json!({"id":2,"ok":false,"error":"no type"}));
        n.handle(json!({"id":3,"sock":"/x","type":"kiapi.board.commands.GetItems"})); assert_eq!(next(&mut r).await, json!({"id":3,"ok":false,"error":proto::NO_SCHEMA}));
        n.handle(json!({"id":4,"sock":"/nonexistent/s","type":"kiapi.common.commands.Ping"})); assert_eq!(next(&mut r).await, json!({"id":4,"ok":false,"error":"dial ipc:///nonexistent/s: Connection refused"}));
        n.handle(json!({"id":5,"op":"close","sock":"/nonexistent/s"})); assert_eq!(next(&mut r).await, json!({"id":5,"ok":true}));
        n.handle(json!({"id":6,"op":"close","sock":"/never/used"})); assert_eq!(next(&mut r).await, json!({"id":6,"ok":true}));
        n.handle(json!({"id":7,"op":"spawn","key":"k"})); assert_eq!(next(&mut r).await, json!({"id":7,"ok":false,"error":"spawn needs key and argv"}));
        n.handle(json!({"id":8,"op":"spawn","key":"k","argv":["x"],"env":{"PATH":"/x"}})); assert_eq!(next(&mut r).await, json!({"id":8,"ok":false,"error":"invalid KiCad environment"}));
        n.handle(json!({"id":9,"sock":"/x","type":"kiapi.common.commands.GetVersion","message":5})); let o = next(&mut r).await;
        assert_eq!(o, json!({"id":9,"ok":false,"status":"AS_BAD_REQUEST","error":"bad message for kiapi.common.commands.GetVersion: .kiapi.common.commands.GetVersion: object expected"}));
    }
    #[tokio::test] async fn a_timeout_drops_the_connection_and_the_next_dials_again() {
        let (path, l) = stand_in::sock("late");
        let srv = tokio::spawn(async move {
            let (mut s, _) = stand_in::accept(&l).await; let _ = stand_in::read_req(&mut s).await; // no answer; the client hangs up
            let (mut s2, _) = stand_in::accept(&l).await; let (id, _) = stand_in::read_req(&mut s2).await.unwrap(); stand_in::write_rep(&mut s2, id, &version_reply()).await; drop(s);
        });
        let (mut n, mut r) = native("");
        n.handle(json!({"id":1,"sock":path,"type":"kiapi.common.commands.Ping","timeout":200})); assert_eq!(next(&mut r).await, json!({"id":1,"ok":false,"error":"KiCad did not answer in 0.2 s"}));
        n.handle(json!({"id":2,"sock":path,"type":"kiapi.common.commands.GetVersion"})); assert_eq!(next(&mut r).await["ok"], json!(true));
        srv.await.unwrap();
    }
    #[tokio::test] async fn close_answers_what_it_holds() {
        let (path, l) = stand_in::sock("close");
        tokio::spawn(async move { let (mut s, _) = stand_in::accept(&l).await; let _ = stand_in::read_req(&mut s).await; std::future::pending::<()>().await; });
        let (mut n, mut r) = native("");
        for id in 1..=2 { n.handle(json!({"id":id,"sock":path,"type":"kiapi.common.commands.Ping"})) }
        tokio::time::sleep(Duration::from_millis(100)).await;
        n.handle(json!({"id":3,"op":"close","sock":path}));
        let mut got: Vec<Value> = vec![next(&mut r).await, next(&mut r).await, next(&mut r).await]; got.sort_by_key(|o| o["id"].as_i64());
        assert_eq!(got, vec![json!({"id":1,"ok":false,"error":CONN_CLOSED}), json!({"id":2,"ok":false,"error":CONN_CLOSED}), json!({"id":3,"ok":true})]);
    }
    fn script(d: &str, body: &str) -> String { let p = format!("{d}/fake"); std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap(); use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); p }
    fn tmp(name: &str) -> String { let d = std::env::temp_dir().join(format!("kipc-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d.to_string_lossy().into() }
    #[tokio::test] async fn spawn_reports_lines_and_exit() {
        let d = tmp("spawn");
        let f = script(&d, "echo hello\nprintf 'KiCad API server listening at ipc://%s\\n' \"$2\"\necho bad >&2\nprintf 'caf\\377\\n'\nexit 3");
        let (mut n, mut r) = native("");
        n.handle(json!({"id":1,"op":"spawn","key":"p","argv":[f,"--socket",format!("{d}/api.sock")]}));
        assert_eq!(next(&mut r).await, json!({"id":1,"ok":true}));
        let mut seen = vec![]; loop { let o = next(&mut r).await; let end = o["event"] == "exit"; seen.push(o); if end { break } }
        assert_eq!(seen[0], json!({"event":"line","key":"p","text":"hello"}));
        assert_eq!(seen[1], json!({"event":"line","key":"p","text":format!("KiCad API server listening at ipc://{d}/api.sock")}));
        assert_eq!(seen.iter().filter(|o| o["text"].as_str().is_some_and(|t| t.starts_with(READY))).count(), 1);
        let end = seen.last().unwrap(); assert_eq!(end["code"], json!(3)); assert!(end["tail"].as_str().unwrap().contains("bad") && end["tail"].as_str().unwrap().contains("hello"), "{end}");
    }
    #[tokio::test] async fn spawn_that_cannot_start_says_127() {
        let (mut n, mut r) = native("");
        n.handle(json!({"id":1,"op":"spawn","key":"p","argv":["/nonexistent/kicad-cli"]}));
        assert_eq!(next(&mut r).await, json!({"id":1,"ok":true}));
        let o = next(&mut r).await; assert_eq!((o["event"].as_str(), o["code"].as_i64()), (Some("exit"), Some(127)));
    }
    /// A server whose stdout is buffered (real KiCad's) is found by the socket it binds: only one bound after this launch counts.
    #[tokio::test] async fn a_fresh_socket_stands_in_for_the_ready_line() {
        let d = tmp("fresh"); let sock = format!("{d}/api.sock"); let alt = format!("{d}/api-999.sock");
        let old = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        let f = script(&d, "sleep 1");
        let (mut n, mut r) = native("");
        n.handle(json!({"id":1,"op":"spawn","key":"p","argv":[f,"--socket",sock]}));
        assert_eq!(next(&mut r).await["ok"], json!(true));
        tokio::time::sleep(Duration::from_millis(350)).await;
        assert!(r.try_recv().is_err(), "a socket from before the launch is not ready");
        let new = std::os::unix::net::UnixListener::bind(&alt).unwrap();
        assert_eq!(next(&mut r).await, json!({"event":"line","key":"p","text":format!("{READY}{alt}")}));
        assert_eq!(next(&mut r).await["event"], json!("exit")); drop((old, new));
    }
    #[test] fn sockets_at_sees_only_the_servers_names() {
        let d = tmp("names"); let p = |n: &str| format!("{d}/{n}");
        let _a = std::os::unix::net::UnixListener::bind(p("api.sock")).unwrap(); let _b = std::os::unix::net::UnixListener::bind(p("api-12.sock")).unwrap(); let _c = std::os::unix::net::UnixListener::bind(p("other.sock")).unwrap(); let _d = std::os::unix::net::UnixListener::bind(p("api-x.sock")).unwrap(); std::fs::write(p("api-13.sock"), "").unwrap();
        let mut k: Vec<String> = sockets_at(&p("api.sock")).into_keys().collect(); k.sort(); assert_eq!(k, vec![p("api-12.sock"), p("api.sock")]);
        assert!(sockets_at("/nonexistent/x/api.sock").is_empty());
    }
    #[test] fn wrap_only_with_a_memory_cap() { assert_eq!(wrap(vec!["a".into()], None), vec!["a".to_string()]); }
    #[test] fn socket_argument() { assert_eq!(socket_arg(&["x".into(), "--socket".into(), "/s".into()]), "/s"); assert_eq!(socket_arg(&["x".into()]), ""); }
}
