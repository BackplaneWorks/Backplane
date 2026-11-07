//! The project card:
//! `fab.look {thread}` answers `{thread, intent, goal, parts, boards, label, estimate_problems, error, estimates, board_files, board_default, order}`
//! for the thread's folder, and `fab.intent {thread, text}` writes `.backplane/intent.md` and answers the same card. The clients draw the goal,
//! and the header's Buy it from it (the session's `fab.*` info keys are kept from this reply by the policy).
//!
//! No ordering, upload or payment happens here (those are settings/order.rs). The only network call is the optional "from $X" estimate (and a health check for a card with a KiCad
//! project's board and no fab file yet) to the order server, `curl` in a 0600 config as the JS hub does; it is skipped when Ordering or `order.estimates` is off, on a
//! copy of a log, and on a test hub (stand-in agents, no BACKPLANE_ORDER_SERVER), where the address is a dead local one.
use crate::{api::{Caller, Fut, RpcHandler}, files::kicad, hub::Hub};
use serde_json::{json, Map, Value};
use std::{io::Write, path::{Path, PathBuf}, sync::Arc}; use crate::plat::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

pub const DEFAULT_SERVER: &str = "https://2.29.106.88";
/// What the person is told when the server cannot be reached at all (curl's 000), not curl's own words.
pub const UNREACHABLE: &str = "the order server is not reachable: check this machine's internet connection, then Retry";

/// Intent.text: a line that is not blank and not a heading.
fn text_line(l: &str) -> bool { !l.trim().is_empty() && !l.trim().starts_with('#') }
/// Intent.goal: the first text line under "# Goal", else the first text line at all.
pub fn goal(text: &str) -> String {
    let mut after = false;
    for l in text.split('\n') {
        if after && text_line(l) { return l.trim().to_string() }
        after = after || l.trim().to_lowercase().starts_with("# goal");
    }
    text.split('\n').find(|l| text_line(l)).map(|l| l.trim().to_string()).unwrap_or_default()
}

/// Od.find: `*.fab.json` up to five folders down, not under extra/ or mfg/, no hidden folder, no node_modules, sorted bytewise, 40 at most.
pub fn find_fab(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
            let Ok(ft) = e.file_type() else { continue };
            let p = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if name.starts_with('.') { continue } // -not -path '*/.*': a hidden file or folder anywhere in the path
            if ft.is_dir() { if depth < 5 && name != "node_modules" && p != "extra" && p != "mfg" { walk(&e.path(), &p, depth + 1, out) } }
            else if name.ends_with(".fab.json") && !p.starts_with("extra/") && !p.starts_with("mfg/") { out.push(p) }
        }
    }
    let mut v = vec![]; walk(root, "", 1, &mut v); v.sort(); v.truncate(40); v
}

/// Order.part.validation
fn validation(path: &str, o: &Value) -> String {
    if o.get("kind").and_then(Value::as_str).unwrap_or("").trim().is_empty() { format!("{path}: missing kind") }
    else if o.get("features").map_or(true, Value::is_null) { format!("{path}: missing measured features") } else { String::new() }
}
fn bad_part(path: &str, why: &str) -> Value { json!({"name":path,"kind":"","path":path,"validation":format!("{path}: {why}")}) }
/// Order.parts: a fab file's part, none for one marked "order": false; a file that is not an object is a part with its problem.
pub fn parts(files: &[(String, String)]) -> Vec<Value> {
    let mut out = vec![];
    for (path, text) in files {
        match serde_json::from_str::<Value>(text) {
            Err(_) => out.push(bad_part(path, "could not read fab JSON")),
            Ok(o) if !o.is_object() => out.push(bad_part(path, "invalid fab JSON: expected an object")),
            Ok(o) => {
                if o.get("order").and_then(Value::as_bool) == Some(false) { continue }
                out.push(json!({"name": o.get("name").and_then(Value::as_str).unwrap_or(path), "kind": o.get("kind").and_then(Value::as_str).unwrap_or(""), "qty": o.get("qty").cloned().unwrap_or(json!(1)),
                    "options": o.get("options").cloned().unwrap_or(Value::Null), "features": o.get("features").cloned().unwrap_or(Value::Null), "path": path, "validation": validation(path, &o)}));
            }
        }
    }
    out
}

fn join(a: &str, b: &str) -> String { if a.is_empty() { b.into() } else if b.is_empty() { a.into() } else { format!("{a}; {b}") } }
fn problem(p: &Value, why: &str) -> String {
    format!("No estimate for {} ({}){}", p["name"].as_str().filter(|s| !s.is_empty()).or(p["path"].as_str()).unwrap_or("part"), p["path"].as_str().unwrap_or("fab file"), if why.is_empty() { String::new() } else { format!(": {why}") })
}
fn row(p: &Value, r: &Value) -> String {
    let name: String = p["name"].as_str().unwrap_or("").chars().take(64).collect();
    let probs: Vec<String> = r["problems"].as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
    if name == r["name"].as_str().unwrap_or("") && p["kind"] == r["kind"] && r["from"].is_number() && probs.is_empty() { String::new() } else { problem(p, &probs.join("; ")) }
}
/// Estimate.problems: every part must come back priced, or the sum is not the project's price.
fn problems(ps: &[Value], rs: &[Value]) -> String {
    let mut s = String::new();
    for (i, p) in ps.iter().enumerate() { s = join(&s, &match rs.get(i) { Some(r) => row(p, r), None => problem(p, "not returned by the estimate service") }); }
    if rs.len() > ps.len() { s = join(&s, "Unexpected estimate parts") }
    s
}
/// Estimate.note
pub fn note(ps: &[Value], est: &Value, on: bool) -> String {
    let v = ps.iter().fold(String::new(), |a, p| join(&a, p["validation"].as_str().unwrap_or("")));
    let rs = est["parts"].as_array().cloned().unwrap_or_default();
    join(&v, &if on && est["error"].as_str().unwrap_or("").is_empty() { problems(ps, &rs) } else { String::new() })
}
/// Estimate.label then Estimate.snapshot: a cached number is shown as the last estimate.
pub fn label(est_label: &str, note: &str) -> String { let l = if note.is_empty() { est_label.to_string() } else { "estimate incomplete".into() }; if l.starts_with("from ") { format!("last estimate: {l}") } else { l } }

/// The card (Od.card).
pub fn card(thread: &str, intent: &str, ps: &[Value], boards: usize, est: &Value, on: bool, board_files: &[String], board_default: &str, order: &str) -> Value {
    let n = note(ps, est, on);
    let rows: Vec<Value> = ps.iter().map(|p| json!({"name":p["name"].as_str().unwrap_or(""),"kind":p["kind"].as_str().unwrap_or(""),"path":p["path"].as_str().unwrap_or("")})).collect();
    json!({"thread":thread,"intent":intent,"goal":goal(intent),"parts":rows,"boards":boards,"label":label(est["label"].as_str().unwrap_or(""), &n),"estimate_problems":n,
        "error":est["error"].as_str().unwrap_or(""),"estimates":on,"board_files":board_files,"board_default":board_default,"order":order})
}

/// Od.ids: only a thread id's characters (letters, digits, dashes).
fn ids_ok(x: &str) -> bool { !x.is_empty() && x.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') }
/// Od.order: the newest order file (`<home>/orders/q_*.json`, newest first) that names one of the buy threads and is paid, placed, shipped or delivered: its status.
pub fn order_status(orders: &Path, ids: &[String]) -> String {
    let ids: Vec<&String> = ids.iter().filter(|i| ids_ok(i)).collect();
    if ids.is_empty() { return String::new() }
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(orders).map(|r| r.flatten().filter(|e| { let n = e.file_name().to_string_lossy().to_string(); n.starts_with("q_") && n.ends_with(".json") })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect()).unwrap_or_default();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, f) in files {
        let Ok(t) = std::fs::read_to_string(&f) else { continue };
        if !ids.iter().any(|i| t.contains(&format!("\"thread\":\"{i}\""))) { continue }
        let paid = ["paid", "placed", "shipped", "delivered"].iter().any(|s| t.contains(&format!("\"status\":\"{s}\"")));
        if paid { return serde_json::from_str::<Value>(&t).ok().and_then(|v| v["status"].as_str().map(str::to_string)).unwrap_or_default() }
    }
    String::new()
}

// ---- the order server (estimates only) ----

pub(crate) fn server_of(setting: &str) -> String {
    let s = setting.trim(); let s = if s.is_empty() { DEFAULT_SERVER } else { s };
    let env = std::env::var("BACKPLANE_ORDER_SERVER").unwrap_or_default();
    if s == DEFAULT_SERVER && !env.is_empty() { env }
    // a test hub (stand-in agents) with no address of its own never reaches the real server
    else if s == DEFAULT_SERVER && std::env::var_os("BACKPLANE_AGENTS").is_some() { "http://127.0.0.1:9".into() } else { s.to_string() }
}
pub(crate) fn install_id(home: &Path) -> String {
    let dir = home.join("secrets"); let f = dir.join("install-id");
    if let Ok(t) = std::fs::read_to_string(&f) { if t.trim().len() >= 16 { return t.trim().to_string() } }
    let mut b = [0u8; 12]; if let Ok(mut r) = std::fs::File::open("/dev/urandom") { let _ = std::io::Read::read_exact(&mut r, &mut b); }
    let id = hex::encode(b);
    let _ = std::fs::DirBuilder::new().mode(0o700).recursive(true).create(&dir);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&f).and_then(|mut h| h.write_all(id.as_bytes())); id
}
/// Od.est.got: the server's answer as the card's fields, or why there is none.
pub(crate) fn est_got(code: &str, body: &str) -> Value {
    if code.starts_with('2') { return serde_json::from_str(body).unwrap_or(Value::Null) }
    let said = serde_json::from_str::<Value>(body).ok().and_then(|v| v["error"].as_str().map(str::to_string));
    json!({"error": if code == "000" { UNREACHABLE.to_string() } else { said.unwrap_or_else(|| format!("the order server said {code}")) }})
}
/// One request through a curl config in the secrets folder (the URL, headers and body never on a command line). (status code, body)
pub(crate) fn curl(home: &Path, url: &str, headers: &[String], body: Option<&str>) -> (String, String) {
    let dir = home.join("secrets");
    if std::fs::DirBuilder::new().mode(0o700).recursive(true).create(&dir).is_err() { return ("000".into(), String::new()) }
    let nonce = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    let (cfg, out, data) = (dir.join(format!("o-{nonce}.curl")), dir.join(format!("o-{nonce}.out")), dir.join(format!("o-{nonce}.json")));
    let w = |p: &Path, t: &str| std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(p).and_then(|mut f| f.write_all(t.as_bytes()));
    let mut c = super::webhook::curl_config(url, headers, &data.to_string_lossy(), &out.to_string_lossy());
    if body.is_none() { c = c.lines().filter(|l| !l.starts_with("data-binary")).collect::<Vec<_>>().join("\n") + "\n" }
    let r = (|| -> Option<(String, String)> {
        w(&data, body.unwrap_or("")).ok()?; w(&cfg, &c).ok()?;
        let o = crate::proc::std_cmd(crate::proc::Kind::Net, "curl").args(["-sS", "--max-time", "20", "-K"]).arg(&cfg).output().ok()?;
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
        let code: String = s.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect();
        Some((if code.len() == 3 { code } else { "000".into() }, std::fs::read_to_string(&out).unwrap_or_default()))
    })();
    for p in [&cfg, &out, &data] { let _ = std::fs::remove_file(p); }
    r.unwrap_or(("000".into(), String::new()))
}

/// Everything a card needs of the hub (Buy.look's options).
pub struct Opts { pub estimates: bool, pub server: String, pub buy_threads: Vec<String> }
pub async fn opts(hub: &Arc<Hub>, thread: &str) -> Opts {
    let on = hub.setting("order.enabled").await.unwrap_or_default() != "off";
    let est = on && hub.setting("order.estimates").await.filter(|s| !s.is_empty()).unwrap_or_else(|| "on".into()) != "off";
    let server = server_of(&hub.setting("order.server").await.unwrap_or_default());
    // the project's buy threads (their ids), so the card can say what of it was ordered
    let ids: Vec<String> = {
        let m = hub.model().await;
        let project = m.threads.get(thread).map(|t| t.project.clone()).unwrap_or_default();
        let mut v: Vec<String> = m.settings_with_prefix("buy.").filter_map(|(k, _)| k.strip_prefix("buy.")).filter(|id| m.threads.get(*id).map_or(false, |t| t.project == project)).map(str::to_string).collect();
        v.sort(); v
    };
    Opts { estimates: est, server, buy_threads: ids }
}

/// Od.look: the card for a thread's folder (no folder: the empty card).
pub async fn look(hub: &Arc<Hub>, thread: &str) -> Value {
    let o = opts(hub, thread).await;
    let cwd = crate::files::root_of(hub, thread).await;
    let home = hub.cfg.home.clone();
    let (th, buy, est_on, server) = (thread.to_string(), o.buy_threads.clone(), o.estimates, o.server.clone());
    tokio::task::spawn_blocking(move || look_in(&home, cwd.as_deref(), &th, est_on, &server, &buy)).await.unwrap_or_else(|_| card(thread, "", &[], 0, &Value::Null, true, &[], "", ""))
}
pub fn look_in(home: &Path, cwd: Option<&Path>, thread: &str, est_on: bool, server: &str, buy: &[String]) -> Value {
    let Some(cwd) = cwd else { return card(thread, "", &[], 0, &Value::Null, est_on, &[], "", "") };
    let intent = std::fs::read_to_string(cwd.join(".backplane/intent.md")).unwrap_or_default();
    let files: Vec<(String, String)> = find_fab(cwd).into_iter().map(|p| { let t = std::fs::read_to_string(cwd.join(&p)).unwrap_or_default(); (p, t) }).collect();
    let ps = parts(&files);
    let all = kicad::boards::find(cwd);
    let kf = kicad::project::read(&cwd.to_string_lossy());
    let prefix = format!("{}/", cwd.to_string_lossy());
    let bd = kf.pcb.strip_prefix(&prefix).map(str::to_string).unwrap_or_default();
    let boards = kicad::boards::pick(&all, &bd);
    let order = order_status(&home.join("orders"), buy);
    let net = est_on && !crate::copy::on();
    let own = kicad::boards::own(&all); // only these are something to order; `boards` is what the viewer's picker offers
    let est = if ps.is_empty() {
        if net && !own.is_empty() { let (c, b) = curl(home, &format!("{server}/v1/health"), &["Accept: application/json".into()], None); est_got(&c, &b) } else { Value::Null }
    } else if net {
        let body = json!({"parts": ps.iter().map(|p| { let mut m = Map::new(); for k in ["name", "kind", "qty", "options", "features"] { m.insert(k.into(), p[k].clone()); } Value::Object(m) }).collect::<Vec<_>>()}).to_string();
        let (c, b) = curl(home, &format!("{server}/v1/estimate"), &["Content-Type: application/json".into(), format!("X-Backplane-Install: {}", install_id(home))], Some(&body)); est_got(&c, &b)
    } else { Value::Null };
    card(thread, intent.trim(), &ps, if ps.is_empty() { own.len() } else { 0 }, &est, est_on, &boards, &bd, &order)
}

/// The answer to `fab.look` / `fab.intent` (the hub calls this for `fab.look` itself, to keep the read for the view's policy, and the handler below for `fab.intent`).
pub async fn reply(hub: &Arc<Hub>, id: &Value, method: &str, p: &Value) -> Value {
    let thread = p.get("thread").and_then(Value::as_str).unwrap_or("");
    if method == "fab.intent" {
        // the person's own words for .backplane/intent.md, then the card; a copy writes nothing into the project
        if crate::copy::on() { return json!({"t":"reply","id":id,"ok":false,"error":crate::copy::HOME_ONLY}) }
        if let Some(cwd) = crate::files::root_of(hub, thread).await {
            let text = p.get("text").and_then(Value::as_str).unwrap_or("").trim().to_string();
            let c2 = cwd.clone();
            let w = tokio::task::spawn_blocking(move || -> std::io::Result<()> { let d = c2.join(".backplane"); std::fs::create_dir_all(&d)?; let f = d.join("intent.md"); std::fs::write(&f, format!("{text}\n"))?; std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)) }).await;
            if !matches!(w, Ok(Ok(()))) { return json!({"t":"reply","id":id,"ok":false,"error":"the intent could not be written"}) }
        }
    }
    json!({"t":"reply","id":id,"ok":true,"result":look(hub, thread).await})
}

pub struct FabRpc;
impl RpcHandler for FabRpc {
    fn methods(&self) -> &'static [&'static str] { &["fab.look", "fab.intent"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move { vec![reply(hub, id, method, p).await] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    fn tmp(n: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-fab-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn put(p: PathBuf, t: &str) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, t).unwrap() }
    #[test] fn the_goal_is_the_first_line_under_goal_else_the_first_text_line() {
        assert_eq!(goal("# Intent\n\n# Goal\n\nA lamp that dims.\nmore"), "A lamp that dims."); assert_eq!(goal("just words\nand more"), "just words"); assert_eq!(goal("# H\n#### x\n"), ""); assert_eq!(goal(""), "");
        assert_eq!(goal("intro text\n# GOAL\n  the real one  "), "the real one");
    }
    #[test] fn fab_files_are_found_where_the_hub_looks() {
        let r = tmp("find");
        for f in ["a.fab.json", "boards/b/main.fab.json", "extra/jig.fab.json", "mfg/x.fab.json", ".hidden/h.fab.json", "node_modules/n/z.fab.json", "mech/.sneaky.fab.json", "z.txt", "d1/d2/d3/d4/d5.fab.json", "d1/d2/d3/d4/d5/deep.fab.json"] { put(r.join(f), "{}") }
        assert_eq!(find_fab(&r), ["a.fab.json", "boards/b/main.fab.json", "d1/d2/d3/d4/d5.fab.json"]);
    }
    #[test] fn parts_carry_their_problems_and_skip_what_is_not_ordered() {
        let ps = parts(&[("a.fab.json".into(), r#"{"name":"Main","kind":"pcb","qty":5,"features":{"width_mm":10}}"#.into()), ("b.fab.json".into(), r#"{"kind":"3dp"}"#.into()), ("c.fab.json".into(), r#"{"kind":"cnc","features":{},"order":false}"#.into()),
            ("d.fab.json".into(), "not json".into()), ("e.fab.json".into(), "[1]".into()), ("f.fab.json".into(), r#"{"features":{}}"#.into())]);
        let v: Vec<(&str, &str)> = ps.iter().map(|p| (p["path"].as_str().unwrap(), p["validation"].as_str().unwrap())).collect();
        assert_eq!(v, [("a.fab.json", ""), ("b.fab.json", "b.fab.json: missing measured features"), ("d.fab.json", "d.fab.json: could not read fab JSON"), ("e.fab.json", "e.fab.json: invalid fab JSON: expected an object"), ("f.fab.json", "f.fab.json: missing kind")]);
        assert_eq!((ps[0]["name"].clone(), ps[0]["qty"].clone(), ps[1]["name"].clone(), ps[1]["qty"].clone()), (json!("Main"), json!(5), json!("b.fab.json"), json!(1)));
    }
    #[test] fn an_estimate_must_price_every_part_or_the_label_says_incomplete() {
        let ps = parts(&[("a.fab.json".into(), r#"{"name":"Main","kind":"pcb","features":{}}"#.into()), ("b.fab.json".into(), r#"{"name":"Cover","kind":"3dp","features":{}}"#.into())]);
        let full = json!({"label":"from $12","parts":[{"name":"Main","kind":"pcb","from":8,"problems":[]},{"name":"Cover","kind":"3dp","from":4,"problems":[]}]});
        assert_eq!(note(&ps, &full, true), ""); assert_eq!(label("from $12", ""), "last estimate: from $12");
        let short = json!({"label":"from $8","parts":[{"name":"Main","kind":"pcb","from":8,"problems":[]}]});
        assert_eq!(note(&ps, &short, true), "No estimate for Cover (b.fab.json): not returned by the estimate service"); assert_eq!(label("from $8", &note(&ps, &short, true)), "estimate incomplete");
        let prob = json!({"parts":[{"name":"Main","kind":"pcb","from":null,"problems":["too big","odd"]},{"name":"Cover","kind":"3dp","from":4,"problems":[]}]});
        assert_eq!(note(&ps, &prob, true), "No estimate for Main (a.fab.json): too big; odd");
        assert_eq!(note(&ps, &json!({"error":"x"}), true), "", "a failed estimate says its error, not a part problem"); assert_eq!(note(&ps, &Value::Null, false), "");
    }
    #[test] fn the_newest_paid_order_of_the_projects_buy_threads_is_its_state() {
        let o = tmp("orders"); put(o.join("q_1.json"), r#"{"id":"q_1","thread":"b1","status":"shipped"}"#);
        std::thread::sleep(std::time::Duration::from_millis(20)); put(o.join("q_2.json"), r#"{"id":"q_2","thread":"b1","status":"quoted"}"#); put(o.join("q_3.json"), r#"{"id":"q_3","thread":"other","status":"paid"}"#);
        assert_eq!(order_status(&o, &["b1".into()]), "shipped", "a newer unpaid quote does not hide it"); assert_eq!(order_status(&o, &["nope".into()]), ""); assert_eq!(order_status(&o, &["b1/../x".into()]), ""); assert_eq!(order_status(&o, &[]), "");
    }
    #[test] fn a_repository_with_board_fixtures_has_nothing_to_order() {
        let (home, work) = (tmp("fixhome"), tmp("fixwork")); put(work.join("test/files/fixtures/a.kicad_pcb"), "(kicad_pcb)");
        let c = look_in(&home, Some(&work), "t1", true, "http://127.0.0.1:9", &[]);
        assert_eq!((c["boards"].clone(), c["error"].clone()), (json!(0), json!("")), "no Buy it, and the order server is not asked");
        assert_eq!(c["board_files"], json!(["test/files/fixtures/a.kicad_pcb"]), "the viewer's picker still offers it");
        put(work.join("boards/b/b.kicad_pcb"), "(kicad_pcb)"); put(work.join("boards/b/b.kicad_pro"), "{}");
        assert_eq!(look_in(&home, Some(&work), "t1", false, "http://127.0.0.1:9", &[])["boards"], json!(1), "a KiCad project's own board does count");
    }
    static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());
    #[test] fn the_order_server_of_a_test_hub_is_never_the_real_one() {
        let _l = ENV.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("BACKPLANE_ORDER_SERVER"); std::env::set_var("BACKPLANE_AGENTS", "/tmp/x");
        assert_eq!(server_of(""), "http://127.0.0.1:9"); assert_eq!(server_of("https://my.server"), "https://my.server");
        std::env::set_var("BACKPLANE_ORDER_SERVER", "http://127.0.0.1:1"); assert_eq!(server_of("  "), "http://127.0.0.1:1"); std::env::remove_var("BACKPLANE_ORDER_SERVER");
    }
    #[tokio::test(flavor = "multi_thread")] async fn the_card_of_a_project_and_the_intent_the_person_writes() {
        let _l = ENV.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("BACKPLANE_AGENTS", "/tmp/x"); std::env::remove_var("BACKPLANE_ORDER_SERVER");
        let home = tmp("home"); let work = tmp("work");
        put(work.join("boards/amp/amp.kicad_pro"), "{}"); put(work.join("boards/amp/amp.kicad_pcb"), "(kicad_pcb)"); put(work.join("amp.fab.json"), r#"{"name":"Amp","kind":"pcb","qty":3,"features":{"width_mm":40}}"#);
        std::fs::write(home.join("events.jsonl"), format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"amp\",\"root\":{},\"at\":1}}\n{{\"$\":\"ThreadCreated\",\"id\":\"t1\",\"project\":\"p1\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":2}}\n", serde_json::to_string(&work.to_string_lossy()).unwrap())).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let call = |m: &'static str, p: Value| { let hub = hub.clone(); async move { FabRpc.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(4), m, &p).await.remove(0) } };
        let r = call("fab.look", json!({"thread":"t1"})).await; let c = &r["result"];
        assert_eq!((r["ok"].clone(), c["thread"].clone(), c["goal"].clone(), c["boards"].clone(), c["estimates"].clone(), c["order"].clone()), (json!(true), json!("t1"), json!(""), json!(0), json!(true), json!("")));
        assert_eq!(c["parts"], json!([{"name":"Amp","kind":"pcb","path":"amp.fab.json"}]));
        assert_eq!(c["error"], UNREACHABLE, "a test hub's order server is a dead local address"); assert_eq!(c["label"], ""); assert_eq!(c["board_files"], json!(["boards/amp/amp.kicad_pcb"]));
        let keys: Vec<&str> = c.as_object().unwrap().keys().map(String::as_str).collect(); assert_eq!(keys, ["thread","intent","goal","parts","boards","label","estimate_problems","error","estimates","board_files","board_default","order"]);
        // the intent the person writes: the file, then the card
        let r = call("fab.intent", json!({"thread":"t1","text":"  # Goal\n\nAn amplifier for the desk.  "})).await; assert_eq!(r["result"]["goal"], "An amplifier for the desk.");
        assert_eq!(std::fs::read_to_string(work.join(".backplane/intent.md")).unwrap(), "# Goal\n\nAn amplifier for the desk.\n"); assert_eq!(std::fs::metadata(work.join(".backplane/intent.md")).unwrap().permissions().mode() & 0o777, 0o644);
        // estimates off: nothing is asked, the card has no error
        hub.commit(&json!({"$":"SettingSet","key":"order.estimates","value":"off","at":3})).await.unwrap();
        let c = call("fab.look", json!({"thread":"t1"})).await["result"].clone(); assert_eq!((c["error"].clone(), c["estimates"].clone()), (json!(""), json!(false)));
        // a thread with no folder: the empty card
        let c = call("fab.look", json!({"thread":"missing"})).await["result"].clone(); assert_eq!((c["parts"].clone(), c["intent"].clone(), c["boards"].clone()), (json!([]), json!(""), json!(0)));
        assert!(!std::fs::read_dir(home.join("secrets")).map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with("o-"))).unwrap_or(false), "no curl file is left behind");
        let _ = std::fs::remove_dir_all(&home); let _ = std::fs::remove_dir_all(&work);
    }
}
