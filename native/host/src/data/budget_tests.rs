//! Wire byte budgets: what a remote client is sent for the interactions it does most, on a synthetic home (docs/remote-architecture.md, Pillar 5).
//! Each interaction's bytes are measured as they go on the wire (the frame's payload: dict-CBOR for `?view=1`, JSON text or plain CBOR for `?data=1`), printed
//! (`cargo test budget -- --nocapture` shows the table) and held under a ceiling, so a change that makes one of them heavier again fails here first.
//! The ceilings are Phase 0's: compression and the bundled reads (Phase 1) tighten them.
//! The pictures and the sidebar read the state policies, of which a process has one (`statehost::policies`, the first hub's to start): so the measures run in a
//! child process of this test binary (`BP_BUDGET_CHILD`) whose hub is the only one, as the other single-process cases do (wire/tests.rs side_by_side_probe_child).
use super::*;
use crate::hub::Conn;

/// 8 projects, 64 threads; t000 and t001 have 50 messages and 50 tool calls each, every other thread a short exchange; two asks are open.
fn home_log() -> String {
    let mut l = String::new(); let mut at = 1_700_000_000u64; let mut line = |v: Value| { l.push_str(&v.to_string()); l.push('\n') };
    for p in 0..8 { line(json!({"$":"ProjectCreated","id":format!("p{p}"),"title":format!("project {p}"),"root":format!("/nowhere/project-{p}"),"at":at})); at += 1; }
    for t in 0..64 { line(json!({"$":"ThreadCreated","id":format!("t{t:03}"),"project":format!("p{}", t % 8),"title":format!("Thread number {t}"),"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":at})); at += 1; }
    for t in 0..64 {
        let n = if t < 2 { 50 } else { 2 };
        for i in 0..n {
            let th = format!("t{t:03}");
            line(json!({"$":"MessagePosted","thread":th,"msg":format!("m{t}-{i}u"),"role":"user","text":format!("Please look at step {i} of the board bring-up and tell me what you find."),"at":at})); at += 1;
            line(json!({"$":"ActivityLogged","thread":th,"act":format!("a{t}-{i}"),"tone":"tool","kind":"Bash","summary":format!("kicad-cli pcb drc boards/main/main.kicad_pcb --output /tmp/drc-{i}.json"),"at":at})); at += 1;
            line(json!({"$":"MessagePosted","thread":th,"msg":format!("m{t}-{i}a"),"role":"assistant","text":format!("Step {i}: the 3V3 rail is routed, DRC reports no errors, and the decoupling capacitors sit next to their pins."),"at":at})); at += 1;
        }
    }
    // the person was in t000's and t001's projects just now (present/sections.rs `bench_rank`): opening their threads here is no new visit, so going
    // between them changes nothing of the sidebar (a first visit moves a project onto the bench, once)
    { let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
      for p in ["p0", "p1"] { line(json!({"$":"SettingSet","key":format!("sidebar.visit.{p}"),"value":now.to_string(),"at":at})); at += 1; } }
    line(json!({"$":"AskOpened","id":"q81","thread":"t002","kind":"approval","tool":"Bash","detail":"make flash","options":"","at":at})); at += 1;
    line(json!({"$":"AskOpened","id":"q82","thread":"t003","kind":"approval","tool":"Bash","detail":"make flash","options":"","at":at}));
    l
}
fn plan_heavy_log() -> String {
    let mut l = home_log();
    let mut at = 1_700_050_000u64;
    let mut line = |mut v: Value| { v["at"] = json!(at); at += 1; l.push_str(&v.to_string()); l.push('\n'); };
    line(json!({"$":"ModesSet","id":"t002","runtime":"full-access","interaction":"plan","model":""}));
    line(json!({"$":"AskOpened","id":"qp1","thread":"t002","kind":"plan","tool":"ExitPlanMode","detail":"## Plan 1\n- Step A\n- Step B","options":""}));
    line(json!({"$":"ModesSet","id":"t003","runtime":"full-access","interaction":"plan","model":""}));
    line(json!({"$":"AskOpened","id":"qp2","thread":"t003","kind":"plan","tool":"ExitPlanMode","detail":"## Plan 2\n- Step C\n- Step D","options":""}));
    for th in ["t004", "t005"] { line(json!({"$":"ModesSet","id":th,"runtime":"full-access","interaction":"plan","model":""})); }
    for th in ["t006", "t007"] {
        line(json!({"$":"ModesSet","id":th,"runtime":"full-access","interaction":"plan","model":""}));
        line(json!({"$":"SettingSet","key":format!("plan.once.{th}"),"value":"1"}));
    }
    line(json!({"$":"ModesSet","id":"t008","runtime":"full-access","interaction":"plan","model":""}));
    line(json!({"$":"TurnChanged","thread":"t008","state":"running"}));
    line(json!({"$":"TurnChanged","thread":"t009","state":"running"}));
    line(json!({"$":"SettingSet","key":"plan.executing.t009","value":"1"}));
    l
}

// ---- ?view=1 (the remote desktop) ----
struct View { c: Conn, id: u64, rev: u64, token: String, by_target: HashMap<String, String>, z: crate::wire::z::Deflater, zbytes: usize }
impl View {
    /// One view request as the desktop sends it (native sidebar, keyed deltas offered against `base`), its reply's bytes on the wire and its delta ops (None: whole).
    async fn ask(&mut self, hub: &Hub, m: &str, target: Value, base: &str) -> (usize, Option<usize>) {
        self.id += 1; self.rev += 1;
        let p = json!({"version":1,"hub":"http://x","generation":self.c.gen,"epoch":hub.epoch,"requestRevision":self.rev,"target":target,"viewId":self.c.view_id,
            "session":{"sel":target["id"],"rid":1,"drafts":{},"open":{},"adding":false,"panel":false,"info":{}},"nativeSidebar":1,"viewDelta":{"version":1,"token":base}});
        let out = hub.request(&mut self.c, &json!({"id":(1u64 << 47) + self.id,"m":m,"p":p})).await;
        let bytes: usize = out.iter().map(|f| crate::cbor::encode(f).len()).sum();
        // the same frames through the connection's one deflate stream (`z=deflate`): what they cost compressed
        self.zbytes = out.iter().map(|f| self.z.frame(&crate::cbor::encode(f)).len()).sum();
        let r = &out.last().expect("a reply")["result"];
        if std::env::var_os("BUDGET_DUMP").is_some() { for op in r["viewDelta"]["ops"].as_array().into_iter().flatten() { eprintln!("OP {m} {target}: {} {} B", op[1], op.get(2).map_or(0, |x| x.to_string().len())) } }
        if let Some(t) = r["viewDelta"]["token"].as_str() { self.token = t.to_string(); self.by_target.insert(target.to_string(), t.to_string()); }
        (bytes, r["viewDelta"]["ops"].as_array().map(Vec::len))
    }
}
fn thread(id: &str) -> Value { json!({"kind":"thread","id":id}) }

/// The hub as main.rs starts it: the Rust screens (the native sidebar and threads) behind the presenter, the state policies booted; serving data/1 too.
async fn real_hub_with(name: &str, log: String) -> (crate::hub::TestHub, u16) {
    let dir = crate::plat::tmp::dir(name); std::fs::write(dir.join("token"), "tok").unwrap(); std::fs::write(dir.join("events.jsonl"), log).unwrap();
    let cfg = Cfg { home: dir.to_path_buf(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
    let recent = crate::present::screen::Recent::new();
    let hub = Hub::start(cfg, Arc::new(crate::present::Routed::new(Arc::new(crate::present::NotPorted), vec![Box::new(recent.clone())], &dir, false))).await.unwrap();
    recent.bind(&hub); let _p = crate::statehost::start(hub.clone());
    for _ in 0..500 { if crate::statehost::policies().map_or(false, |p| p.booted()) { break } tokio::time::sleep(Duration::from_millis(10)).await }
    assert!(crate::statehost::policies().map_or(false, |p| p.booted()), "the state policies booted");
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    { let hub = hub.clone(); tokio::spawn(async move { loop { let (st, _) = l.accept().await.unwrap(); let h = hub.clone(); tokio::spawn(async move { let _ = wire::serve_with(h, st, false).await; }); } }); }
    (crate::hub::TestHub { hub, _tmp: dir }, port)
}
async fn real_hub(name: &str) -> (crate::hub::TestHub, u16) { real_hub_with(name, home_log()).await }
#[test] fn budgets() {
    let o = std::process::Command::new(std::env::current_exe().unwrap()).args(["data::tests::budget::budget_child", "--exact", "--nocapture", "--test-threads=1"]).env("BP_BUDGET_CHILD", "1").output().unwrap();
    let (out, err) = (String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    for l in err.lines().chain(out.lines()).filter(|l| l.starts_with("BUDGET")) { eprintln!("{l}") }
    assert!(o.status.success() && out.contains("budget_child_ok"), "{out}{err}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn budget_child() {
    if std::env::var_os("BP_BUDGET_CHILD").is_none() { return }
    let (h, port) = real_hub("data-budget").await;
    view_budget(&h.hub).await; data_budget(port).await; phase1_budget(port).await;
    println!("budget_child_ok");
}
#[test] fn plan_mode_budgets() {
    let o = std::process::Command::new(std::env::current_exe().unwrap()).args(["data::tests::budget::plan_budget_child", "--exact", "--nocapture", "--test-threads=1"]).env("BP_PLAN_BUDGET_CHILD", "1").output().unwrap();
    let (out, err) = (String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    for l in err.lines().chain(out.lines()).filter(|l| l.starts_with("BUDGET")) { eprintln!("{l}") }
    assert!(o.status.success() && out.contains("plan_budget_child_ok"), "{out}{err}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn plan_budget_child() {
    if std::env::var_os("BP_PLAN_BUDGET_CHILD").is_none() { return }
    let (_h, port) = real_hub_with("data-plan-budget", plan_heavy_log()).await;
    data_budget(port).await;
    println!("plan_budget_child_ok");
}
/// `?view=1`: a thread switch is ops from a body the client holds, a return to a held thread and an update with nothing changed are near nothing.
async fn view_budget(hub: &Arc<Hub>) {
    let (c, _greeting) = hub.connect(true);
    let mut v = View { c, id: 0, rev: 0, token: String::new(), by_target: HashMap::new(), z: Default::default(), zbytes: 0 };
    let (cold_recent, _) = v.ask(&hub, "view.open", json!({"kind":"recent","id":""}), "").await;
    let base = v.token.clone(); let (cold_open, _) = v.ask(&hub, "view.open", thread("t000"), &base).await;
    let base = v.token.clone(); let (switch, switch_ops) = v.ask(&hub, "view.open", thread("t001"), &base).await;
    let switch_z = v.zbytes;
    let back_base = v.by_target[&thread("t000").to_string()].clone(); let (back, back_ops) = v.ask(&hub, "view.open", thread("t000"), &back_base).await; let back_z = v.zbytes;
    let base = v.token.clone(); let (noop, noop_ops) = v.ask(&hub, "view.update", thread("t000"), &base).await; let noop_z = v.zbytes;
    let (whole, _) = v.ask(&hub, "view.open", thread("t001"), "").await;
    eprintln!("BUDGET view=1: cold recent {cold_recent} B, open t000 {cold_open} B, switch t000->t001 {switch} B ({switch_ops:?} ops; whole {whole} B), back to t000 on its own base {back} B ({back_ops:?} ops), unchanged view.update {noop} B ({noop_ops:?} ops); deflated: switch {switch_z} B, back {back_z} B, update {noop_z} B");
    assert!(back_z <= WARM_Z_MAX && noop_z <= WARM_Z_MAX, "deflated, a return to a held thread {back_z} B and an unchanged update {noop_z} B, at most {WARM_Z_MAX}");
    assert!(switch_ops.is_some() && back_ops.is_some(), "a thread switch is ops from a body the client holds, not the whole picture");
    assert!(switch * 2 < whole && switch <= SWITCH_MAX, "a switch costs well under the whole picture: {switch} B of {whole} B, at most {SWITCH_MAX}");
    assert!(back_ops.map_or(false, |n| n <= 4), "back to a thread whose picture the client holds, nothing changed: only the session's own fields ({back_ops:?} ops)");
    assert!(back <= BACK_MAX, "back to a held thread: {back} B, at most {BACK_MAX}");
    assert_eq!(noop_ops, Some(0), "an update with nothing changed has no ops");
    assert!(noop <= NOOP_MAX, "an unchanged view.update: {noop} B, at most {NOOP_MAX}");
}
/// Phase 0's ceilings (measured: 26.7 KB for a switch to a thread not held, 769 B back to one held, 241 B an unchanged update); Phase 1 holds a warm switch to 500 B.
const SWITCH_MAX: usize = 32 * 1024;
/// Phase 1: a warm switch (a held thread, or nothing changed), compressed.
const WARM_Z_MAX: usize = 500;
const BACK_MAX: usize = 1024;
const NOOP_MAX: usize = 512;

// ---- ?data=1 (phones and the web) ----
/// The JSON text frames' bytes of one request's reply.
async fn sized(ws: &mut Ws, id: u64, m: &str, p: Value) -> (Value, usize) {
    ws.send(Message::Text(json!({"id":id,"m":m,"p":p}).to_string().into())).await.unwrap();
    loop {
        let m = tokio::time::timeout(Duration::from_secs(10), ws.next()).await.expect("a frame in time").unwrap().unwrap();
        let (v, n) = match m { Message::Text(t) => (serde_json::from_str::<Value>(t.as_str()).unwrap(), t.len()), Message::Binary(b) => (crate::cbor::decode(&b).unwrap(), b.len()), _ => continue };
        if v["t"] == "reply" && v["id"] == id { return (v, n) }
    }
}
/// What a phone sends to open a thread (the parts of it this hub answers without its agents and viewers): the bytes of the replies.
async fn open_thread(ws: &mut Ws, id: &mut u64, th: &str) -> usize {
    let mut total = 0;
    for (m, p) in [("subscribe", json!({"topic":"thread","thread":th})), ("thread.entries", json!({"thread":th,"limit":40})), ("thread.buy", json!({"thread":th}))] {
        *id += 1; let (r, n) = sized(ws, *id, m, p).await; assert_eq!(r["ok"], true, "{m}: {r}"); total += n;
    }
    total
}
/// `?data=1`: the cold lists and a thread's open, within their ceilings; the lists asked again (asks and previews kept) the same.
async fn data_budget(port: u16) {
    let mut ws = open(port, "&enc=json").await; let _hello = next(&mut ws).await;
    let mut lists = 0; let mut id = 0; let mut first = vec![];
    for m in ["projects.list", "threads.list", "sidebar.sections"] { id += 1; let (r, n) = sized(&mut ws, id, m, json!({})).await; assert_eq!(r["ok"], true, "{m}: {r}"); lists += n; first.push(r["result"].clone()); }
    let t0 = std::time::Instant::now(); let open0 = open_thread(&mut ws, &mut id, "t000").await; let cold_ms = t0.elapsed().as_millis();
    let open1 = open_thread(&mut ws, &mut id, "t001").await;
    let t1 = std::time::Instant::now(); let back = open_thread(&mut ws, &mut id, "t000").await; let back_ms = t1.elapsed().as_millis();
    // the lists again: the asks and previews are kept, nothing changed, the same bytes
    let mut again = 0; let mut second = vec![];
    for m in ["projects.list", "threads.list", "sidebar.sections"] { id += 1; let (r, n) = sized(&mut ws, id, m, json!({})).await; again += n; second.push(r["result"].clone()); }
    eprintln!("BUDGET data/1 json: cold lists {lists} B (again {again} B), open t000 {open0} B in {cold_ms} ms, switch to t001 {open1} B, back to t000 {back} B in {back_ms} ms");
    assert_eq!(first, second, "the same lists: what is kept answers as a fresh read does");
    assert!(lists <= LISTS_MAX, "cold lists: {lists} B, at most {LISTS_MAX}");
    assert!(open0 <= OPEN_MAX && open1 <= OPEN_MAX && back <= OPEN_MAX, "a thread open: {open0}/{open1}/{back} B, at most {OPEN_MAX}");
}
/// Phase 0's ceilings (measured: 31.6 KB and 6.6 KB, JSON); Phase 1's compression and `shell.sync`/`thread.open` bring them to 6 KB and 3 KB.
const LISTS_MAX: usize = 36 * 1024;
const OPEN_MAX: usize = 8 * 1024;

// ---- Phase 1: compressed, one round trip (docs/PHONE-DATA.md, Compression and one-round-trip reads) ----
/// A `z=deflate` socket's reply to one request: the reply, and the compressed bytes of every frame up to it.
struct Zws { ws: Ws, z: flate2::Decompress }
impl Zws {
    async fn ask(&mut self, id: u64, m: &str, p: Value) -> (Value, usize) {
        use flate2::FlushDecompress;
        self.ws.send(Message::Text(json!({"id":id,"m":m,"p":p}).to_string().into())).await.unwrap();
        let mut n = 0;
        loop {
            let f = tokio::time::timeout(Duration::from_secs(10), self.ws.next()).await.expect("a frame in time").unwrap().unwrap();
            let Message::Binary(b) = f else { continue }; n += b.len();
            let mut out = Vec::with_capacity(b.len() * 8 + 1024); let start = self.z.total_in();
            loop { let done = (self.z.total_in() - start) as usize; if out.capacity() - out.len() < 64 { out.reserve(out.capacity()) }
                self.z.decompress_vec(&b[done..], &mut out, FlushDecompress::Sync).unwrap(); if (self.z.total_in() - start) as usize == b.len() && out.len() < out.capacity() { break } }
            let v: Value = serde_json::from_slice(&out).unwrap();
            if v["t"] == "reply" && v["id"] == id { assert_eq!(v["ok"], true, "{m}: {v}"); return (v["result"].clone(), n) }
        }
    }
}
/// The client's `have` after a reply: its part hashes, and each entry's own.
fn have_after(r: &Value, held: &mut serde_json::Map<String, Value>) {
    for (k, v) in r["h"].as_object().into_iter().flatten() { held.insert(k.clone(), v.clone()); }
    for e in r["entries"].as_array().into_iter().flatten() { held.insert(e["id"].as_str().unwrap_or("").into(), e["h"].clone()); }
}
async fn phase1_budget(port: u16) {
    let mut c = Zws { ws: open(port, "&enc=json&z=deflate").await, z: flate2::Decompress::new(false) };
    let hello = match c.ws.next().await.unwrap().unwrap() { Message::Text(t) => serde_json::from_str::<Value>(t.as_str()).unwrap(), m => panic!("{m:?}") }; assert_eq!(hello["z"], "deflate");
    let (shell, shell_z) = c.ask(1, "shell.sync", json!({})).await;
    let (_, shell_warm_z) = c.ask(2, "shell.sync", json!({"have": shell["h"]})).await;
    let (t0, open_z) = c.ask(3, "thread.open", json!({"thread":"t000"})).await;
    let (t1, switch_z) = c.ask(4, "thread.open", json!({"thread":"t001"})).await;
    let mut held = serde_json::Map::new(); have_after(&t0, &mut held);
    let (back, back_z) = c.ask(5, "thread.open", json!({"thread":"t000","have":held})).await;
    let _ = t1;
    eprintln!("BUDGET phase 1 data/1 json+deflate: cold shell.sync {shell_z} B (warm {shell_warm_z} B), cold thread.open t000 {open_z} B, t001 {switch_z} B, back to t000 held {back_z} B");
    assert_eq!(back["entries"], json!([]), "held and unchanged: nothing of the page again");
    assert!(shell_z <= SHELL_Z_MAX, "a cold sidebar: {shell_z} B, at most {SHELL_Z_MAX}");
    assert!(open_z <= OPEN_Z_MAX && switch_z <= OPEN_Z_MAX, "a cold thread open: {open_z} / {switch_z} B, at most {OPEN_Z_MAX}");
    assert!(back_z <= WARM_Z_MAX && shell_warm_z <= WARM_Z_MAX, "warm: thread {back_z} B, sidebar {shell_warm_z} B, at most {WARM_Z_MAX}");
}
/// Phase 1's ceilings (docs/remote-architecture.md, Pillar 5): a cold sidebar 6 KB, a cold thread open 3 KB, compressed.
const SHELL_Z_MAX: usize = 6 * 1024;
const OPEN_Z_MAX: usize = 3 * 1024;
