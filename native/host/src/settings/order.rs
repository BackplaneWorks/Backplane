//! Ordering: what the agents call (docs/order.md; 0.14's src/core/order and src/server/orderio). `order_review` is every thread's while Ordering is on,
//! the rest are a buy thread's own (the setting `buy.<thread>`). Ordering off refuses all of them, even for an agent that remembers them from before.
//!
//! The design's fingerprint keeps a review current only while the files are as they were: `.backplane/review.json` keeps it with the verdict.
//!
//! `order_quote` sends nothing itself: it opens the approval card, and only the person's Send (`answered`, from `Agents::answer`) uploads, and only the files they were shown.
//! A quote's token is kept in `<home>/orders/<quote>.json` (0600) and nowhere else: the log holds the link as `order:<quote>` and the hub writes the real one only into what it
//! draws for a client (`pay_shown`), so no agent can read it back with thread_read and no log or far thread carries it. On a copy nothing here sends or follows anything.
use crate::agents::{lines::hub_id, now_s, Agents};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, io::{Read, Write}, path::{Path, PathBuf}, sync::{Arc, LazyLock, RwLock}, time::Duration};
use crate::plat::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

pub use super::order_view::*;
fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }

pub const OFF: &str = "Ordering is off; the person can turn it on in Settings";
pub const NOT_BUY: &str = "only a buy thread has this tool";
const QUOTE_ASKED: &str = "The person is asked on their screen whether to send these files (Send or Not now). End your turn now. Once they send, the files go to the order server and the quote is made (usually 1-2 minutes); the thread shows the person it is on its way, Backplane posts the quote here and you get a message with its total. There is nothing more for them to press: never ask them to send or approve again.";
const STATUS_UNSENT: &str = "The files are not sent yet: Backplane's question whether to send them (Send or Not now) is open on the person's screen. Say so in a line and end your turn; you get a message once they answer.";
const LOST: &str = "The person pressed Send, but Backplane restarted since it asked and no longer has that list of files, so nothing was sent. Call order_quote again with the same folder; they will be asked once more.";
const DECLINED: &str = "The person chose not to send the files (Not now). Ask whether anything should change first.";
const COPY_SENDS_NOTHING: &str = "a hub on a copied log (BACKPLANE_COPY=1) sends nothing to the order server";
const ASKED: &str = "Asked on the person's screen. End your turn now: their answer comes as their next message, however long they take.";
/// Order.skipped: a dismissed question, said as the person's message so its agent hears it (silence left the thread waiting).
const SKIPPED: &str = "I skipped this question.";
const ISSUES_KEPT: usize = 8;

// ---- the tools ----

fn prop(desc: &str) -> Value { json!({"type":"string","description":desc}) }
fn def(name: &str, desc: &str, props: Value, required: Value) -> Value { json!({"name":name,"description":desc,"inputSchema":{"type":"object","properties":props,"required":required}}) }
/// Order.review.tool: every thread's agent has this one (a buy thread checks, a review records).
fn review_tool() -> Value {
    def("order_review", "The project's manufacturing review. gate (a buy thread, before exporting): Backplane checks the review and, unless it is current and passed, shows the person why and asks them; answers what it did. check: whether the design (its boards, schematics, models and fab files) was reviewed since it last changed; answers state (none, stale or current), verdict, summary and issues. record: after a manufacturing review, keep its verdict (ok or issues), a one-line summary and the issues, one short line each, with the design as it is now.",
        json!({"action":prop("gate, check or record"),"verdict":prop("record: ok or issues"),"summary":prop("record: one line"),
            "issues":{"type":"array","items":{"type":"string"},"description":"record: what must change, one short line each (\"Shade: walls 0.6 mm, need 0.8\"), at most 8; none when ok."}}), json!(["action"]))
}
fn quote_tool() -> Value {
    def("order_quote", "Get a quote for an exported order: the folder (from this thread's folder, e.g. mfg/20261017-0930) holding manifest.json and the files it lists. Backplane shows the person every file it would send and asks them first, and answers at once: end your turn. Once they send, it uploads the files, waits for the quote (a minute or two), posts it in this thread with a Buy now link and sends you a message with its total and any problems (or that they chose not to send).",
        json!({"dir":prop("The order's folder, from this thread's folder.")}), json!(["dir"]))
}
fn status_tool() -> Value { def("order_status", "Where this thread's last quote or order stands: waiting, quoted (with its total), paid, placed, shipped (with tracking) or delivered.", json!({}), json!([])) }
fn ask_tool() -> Value {
    def("order_ask", "Ask the person one question, shown in its own card above the message box on every device: choices they can tap, or with typed a box to type the answer in (where it ships); they may always type their own answer. Answers at once: end your turn, and their answer comes as their next message, however long they take.",
        json!({"question":prop("The question, short and plain."),"options":{"type":"array","items":{"type":"string"},"description":"Two to four short choices, the likely one first; none with typed."},
            "typed":{"type":"boolean","description":"true when the answer is typed (the country it ships to, and for the US its state): the person gets a box with Continue."},"last":prop("With typed: the earlier order's answer (its country's name, or its US state), offered as a quick choice.")}), json!(["question"]))
}
/// Bots.order.tools: what a thread's agent is shown: none with Ordering off, a buy thread's own only in a buy thread.
pub fn defs(on: bool, buy: bool) -> Vec<Value> {
    if !on { return vec![] }
    let mut v = if buy { vec![quote_tool(), status_tool(), ask_tool()] } else { vec![] }; v.push(review_tool()); v
}
pub async fn listed(ag: &Arc<Agents>, thread: &str) -> Vec<Value> { let w = ag.world().await; defs(w.setting("order.enabled") != "off", !w.setting(&format!("buy.{thread}")).is_empty()) }

/// Hub.order.on / Hub.order.k: every order tool's call.
pub async fn call(ag: &Arc<Agents>, thread: &str, name: &str, a: &Value) -> (bool, Value) {
    let (on, buy, cwd) = { let w = ag.world().await; (w.setting("order.enabled") != "off", !w.setting(&format!("buy.{thread}")).is_empty(), w.cwd(thread)) };
    if !on { return fail(OFF) }
    if !buy && name != "order_review" { return fail(NOT_BUY) }
    match name {
        "order_ask" => question(ag, thread, a).await,
        "order_quote" => quote(ag, thread, &cwd, a).await,
        "order_status" => status(ag, thread).await,
        "order_review" => match sv(a, "action") {
            "gate" => gate(ag, thread, &cwd).await,
            "record" if crate::copy::on() => fail(crate::copy::HOME_ONLY),
            _ => review(&cwd, a).await,
        },
        _ => fail("unknown tool"),
    }
}

// ---- the question ----

/// Order.ask.tool
pub fn ask_tool_name(typed: bool) -> &'static str { if typed { "order_ask.typed" } else { "order_ask" } }
/// Order.ask.value: a quick choice "<it> · last time" is sent as <it>.
/// Order.ask.choices: the ask's options, a line each, the earlier answer first and not twice.
pub fn ask_choices(last: &str, options: &[String]) -> String {
    let last = last.trim();
    let rest = options.iter().map(|o| o.trim()).filter(|o| !o.is_empty() && *o != last).map(str::to_string);
    if last.is_empty() { rest.collect::<Vec<_>>().join("\n") } else { std::iter::once(format!("{last}{LAST_MARK}")).chain(rest).collect::<Vec<_>>().join("\n") }
}
/// Hub.answer.order: what the person's answer to an order question posts in its thread, as their message.
pub fn answer_text(answer: &str) -> &str { let t = answer.trim(); if t.is_empty() { SKIPPED } else { ask_value(t) } }

async fn open_ask(ag: &Arc<Agents>, thread: &str, tool: &str, detail: &str, options: &str, first: Vec<Value>) {
    let now = now_s(); let id = hub_id("q", now, ag.seq_n());
    let mut ev = first; ev.push(json!({"$":"AskOpened","id":id,"thread":thread,"kind":"input","tool":tool,"detail":detail,"options":options,"at":now}));
    ev.push(ag.pending_ev(thread, 1, "", now).await); ag.commit_events(ev).await;
}
/// Od.question: it parks nothing: the answer is posted in the thread as the person's message, however long they take.
async fn question(ag: &Arc<Agents>, thread: &str, a: &Value) -> (bool, Value) {
    let options: Vec<String> = a["options"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    open_ask(ag, thread, ask_tool_name(a["typed"].as_bool().unwrap_or(false)), sv(a, "question"), &ask_choices(sv(a, "last"), &options), vec![]).await;
    (true, json!(ASKED))
}

// ---- the review ----

/// Od.print: the design's fingerprint: the names and contents of every board, schematic, model and fab file outside mfg/, extra/, .git/ and .backplane/, in name order.
/// The same bytes 0.14's `find | sort | sha256sum | cut -c1-32` hashed, so a review 0.14 recorded stays current.
pub fn fingerprint(root: &Path) -> String {
    fn walk(dir: &Path, rel: &[u8], out: &mut Vec<Vec<u8>>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let name = crate::plat::fs::name_bytes(&e.file_name()); let n = name.as_slice();
            let mut p = rel.to_vec(); p.push(b'/'); p.extend_from_slice(n);
            if [&b"./mfg"[..], b"./extra", b"./.git", b"./.backplane"].contains(&p.as_slice()) || n == b"node_modules" || n.windows(6).any(|w| w == b"backup") { continue }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() { walk(&e.path(), &p, out) }
            else if ft.is_file() && !n.starts_with(b"_autosave-") && [&b".kicad_pcb"[..], b".kicad_sch", b".step", b".stp", b".dxf", b".fab.json"].iter().any(|s| n.ends_with(s)) { out.push(p) }
        }
    }
    let mut files = vec![]; walk(root, b".", &mut files); files.sort();
    let mut h = Sha256::new(); let mut buf = vec![0u8; 1 << 16];
    for p in files {
        h.update(&p); h.update(b"\n");
        if let Ok(mut f) = std::fs::File::open(root.join(crate::plat::fs::name_from(&p[2..]))) { while let Ok(n) = f.read(&mut buf) { if n == 0 { break } h.update(&buf[..n]) } }
    }
    hex::encode(h.finalize())[..32].to_string()
}
/// Order.review.state
pub fn review_state(fp: &str, rec: &Value) -> &'static str { let r = sv(rec, "fingerprint"); if r.is_empty() { "none" } else if r == fp { "current" } else { "stale" } }
/// Order.review.lines: an issues list's strings, trimmed, blank ones left out, at most eight.
fn review_lines(v: &Value) -> Vec<String> { v.as_array().into_iter().flatten().filter_map(Value::as_str).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).take(ISSUES_KEPT).collect() }
/// Order.review.said: what check answers.
pub fn review_said(fp: &str, rec: &Value) -> Value { json!({"state":review_state(fp, rec),"verdict":sv(rec, "verdict"),"summary":sv(rec, "summary"),"issues":review_lines(&rec["issues"]),"at":rec["at"].as_u64().unwrap_or(0)}) }
/// Order.review.verdict: ok or issues, nothing else.
fn verdict(v: &str) -> &'static str { if v.trim() == "ok" { "ok" } else { "issues" } }
/// Order.review.rec.with: what record keeps.
pub fn review_rec(fp: &str, a: &Value, at: u64) -> Value { json!({"fingerprint":fp,"verdict":verdict(sv(a, "verdict")),"summary":sv(a, "summary").trim(),"issues":review_lines(&a["issues"]),"at":at}) }
fn review_read(cwd: &Path) -> Value { std::fs::read_to_string(cwd.join(".backplane/review.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null) }
fn review_write(cwd: &Path, rec: &Value) -> std::io::Result<()> {
    let d = cwd.join(".backplane"); std::fs::create_dir_all(&d)?; let f = d.join("review.json");
    std::fs::write(&f, format!("{rec}\n"))?; std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644))
}
/// Od.review: checked or recorded in the thread's folder, off the hub's threads (a design can be large).
async fn review(cwd: &str, a: &Value) -> (bool, Value) {
    if cwd.is_empty() { return fail("this thread has no folder") }
    let (cwd, a, now) = (std::path::PathBuf::from(cwd), a.clone(), now_s());
    let r = tokio::task::spawn_blocking(move || {
        let fp = fingerprint(&cwd);
        if sv(&a, "action") != "record" { return (true, review_said(&fp, &review_read(&cwd))) }
        let rec = review_rec(&fp, &a, now);
        match review_write(&cwd, &rec) { Ok(()) => (true, json!({"recorded":true,"verdict":rec["verdict"]})), Err(_) => fail("the review could not be written") }
    }).await;
    r.unwrap_or_else(|_| fail("the review could not be read"))
}

/// Order.gate: 0 current and passed, 1 none or stale, 2 current with issues.
pub fn gate_kind(said: &Value) -> u8 { if sv(said, "state") != "current" { 1 } else if sv(said, "verdict") == "ok" { 0 } else { 2 } }
fn tidy(x: &str) -> String { let t = x.trim(); t.strip_suffix('.').unwrap_or(t).trim().to_string() }
/// Order.gate.points: the recorded issues, else the summary cut at its semicolons.
pub fn gate_points(summary: &str, issues: &Value) -> Vec<String> {
    let raw: Vec<&str> = issues.as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let none = issues.as_array().map_or(true, Vec::is_empty);
    if none { summary.split(';').map(tidy).filter(|x| !x.is_empty()).collect() } else { raw.into_iter().map(tidy).filter(|x| !x.is_empty()).collect() }
}
/// Order.gate.md
pub fn gate_md(said: &Value) -> String {
    let list: String = gate_points(sv(said, "summary"), &said["issues"]).iter().map(|x| format!("- {x}\n")).collect();
    format!("**The manufacturing review found what to fix before ordering:**\n\n{list}\nDetails: `.backplane/review.md`")
}
const GATE_REVIEW_Q: &str = "This design hasn't been checked for manufacturing since it changed. A review finds what a fab would refuse or charge more for.";
const GATE_FIX_Q: &str = "Fix these before ordering, or order as it is?";
/// Order.gate.said: what the agent hears (Backplane did the asking).
pub fn gate_said(kind: u8) -> &'static str {
    match kind {
        0 => "The manufacturing review is current and passed: go on with the order.",
        1 => "The design has no current manufacturing review: Backplane asked the person \"Review for manufacturing\" or \"Order without a review\". End your turn now; their answer comes as their next message.",
        _ => "The current review found issues: Backplane posted them in this thread and asked the person \"Fix them first\" or \"Order anyway\". Don't repeat them. End your turn now; their answer comes as their next message.",
    }
}
/// Od.gate.go then Hub.order.gate: the gate is the hub's: it tells the person where the review stands and asks, so an agent cannot skip it.
async fn gate(ag: &Arc<Agents>, thread: &str, cwd: &str) -> (bool, Value) {
    let (ok, said) = review(cwd, &json!({"action":"check"})).await;
    if !ok { return (ok, said) }
    let kind = gate_kind(&said);
    match kind {
        0 => {}
        1 => open_ask(ag, thread, "order_ask", GATE_REVIEW_Q, "Review for manufacturing\nOrder without a review", vec![]).await,
        _ => { let now = now_s(); let post = json!({"$":"MessagePosted","thread":thread,"msg":format!("gate:{}", hub_id("q", now, ag.seq_n())),"role":"assistant","text":gate_md(&said),"at":now});
            open_ask(ag, thread, "order_ask", GATE_FIX_Q, "Fix them first\nOrder anyway", vec![post]).await }
    }
    (true, json!(gate_said(kind)))
}

/// A quote id as a file name: the server's word is not trusted to stay inside `<home>/orders`.
pub fn id_ok(id: &str) -> bool { id.starts_with("q_") && id.len() <= 80 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') }
/// Od.pay
fn pay_of(server: &str, qid: &str, token: &str) -> String { format!("{server}/q/{qid}?t={token}") }
#[derive(Clone)]
struct Rec { id: String, token: String, thread: String, server: String }
/// Od.keep: the record a quote leaves, the token's one home: 0600 in a 0700 folder.
fn keep(home: &Path, r: &Rec, status: &str) -> std::io::Result<()> {
    let d = home.join("orders"); std::fs::DirBuilder::new().mode(0o700).recursive(true).create(&d)?; std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700))?;
    let (f, tmp) = (d.join(format!("{}.json", r.id)), d.join(format!(".{}.tmp", r.id)));
    let _ = std::fs::remove_file(&tmp);
    std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?.write_all(json!({"id":r.id,"token":r.token,"thread":r.thread,"server":r.server,"status":status}).to_string().as_bytes())?;
    crate::plat::fs::rename(&tmp, &f)?; PAY.write().unwrap().insert(r.id.clone(), pay_of(&r.server, &r.id, &r.token)); Ok(())
}
fn rec_of(j: &Value) -> Option<(Rec, String)> { let (id, token) = (sv(j, "id"), sv(j, "token")); if !id_ok(id) || token.is_empty() { return None } Some((Rec { id: id.into(), token: token.into(), thread: sv(j, "thread").into(), server: sv(j, "server").into() }, sv(j, "status").to_string())) }
/// Every record, newest first (200 at most, as `Od.resume` read).
fn records(home: &Path) -> Vec<(Rec, String)> {
    let mut fs: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(home.join("orders")).map(|r| r.flatten().filter(|e| { let n = e.file_name().to_string_lossy().to_string(); n.starts_with("q_") && n.ends_with(".json") })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect()).unwrap_or_default();
    fs.sort_by(|a, b| b.0.cmp(&a.0)); fs.truncate(200);
    fs.into_iter().filter_map(|(_, p)| rec_of(&serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?)).collect()
}

// ---- asking to send ----

/// Od.safe: a path from the order's folder that stays inside it, plain characters only (no `"`, `\`, `;`: they end up in curl's config).
pub fn safe(p: &str) -> bool { !p.is_empty() && !crate::plat::fs::is_abs(p) && !p.contains("..") && p.chars().all(|c| c as u32 >= 32 && !matches!(c, '"' | '\\' | ';' | '\u{7f}')) }
/// A link anywhere on the way: a link named like a design file could carry any file of the person's out, under a name they would approve.
fn linked(cwd: &Path, dir: &str) -> bool {
    fn below(d: &Path) -> bool { std::fs::read_dir(d).map_or(false, |r| r.flatten().any(|e| e.file_type().map_or(true, |t| t.is_symlink() || (t.is_dir() && below(&e.path()))))) }
    let mut p = cwd.to_path_buf();
    for c in dir.split('/').filter(|c| !c.is_empty() && *c != ".") { p.push(c); if std::fs::symlink_metadata(&p).map_or(true, |m| m.file_type().is_symlink()) { return true } }
    below(&p)
}
fn sha(b: &[u8]) -> String { hex::encode(Sha256::digest(b)) }
/// Order.ask.detail: what the person approves before anything is sent: how many files, from where, to where they ship, and each file (the first 40).
pub fn ask_detail(dir: &str, m: &Value, ps: &[String]) -> String {
    let parts: Vec<String> = m["parts"].as_array().into_iter().flatten().map(|p| {
        let qty = match &p["qty"] { Value::Null => "1".to_string(), Value::String(s) => s.clone(), v => v.to_string() };
        let name = p["name"].as_str().unwrap_or("?"); let asm = match &p["assemble"] { Value::Null => String::new(), Value::String(s) => s.clone(), v => v.to_string() };
        if asm.is_empty() || asm == "0" { format!("{name} ×{qty}") } else { format!("{name} ×{qty} ({asm} assembled)") } }).collect();
    let n = ps.len(); let mut list: String = ps.iter().take(40).map(|p| format!("\n{p}")).collect(); if n > 40 { list.push_str("\n…") }
    let to = match (m["ship_to"].as_str().unwrap_or("?"), m["ship_region"].as_str().map(str::trim).filter(|r| !r.is_empty())) { (c, Some(r)) => format!("{c} ({r})"), (c, None) => c.to_string() };
    format!("Send {n} {} from {dir} to Backplane's quote service? They are kept only to make what you order.\nShips to {to} · {}\n{list}", if n == 1 { "file" } else { "files" }, parts.join(", "))
}
/// The manifest's files (`"files": {path: sha256}`), when the folder may be sent.
fn sendable(cwd: &Path, dir: &str) -> Option<(Value, Vec<String>, String)> {
    let abs = cwd.join(dir); let bytes = std::fs::read(abs.join("manifest.json")).ok()?; let m: Value = serde_json::from_slice(&bytes).ok()?;
    let ps: Vec<String> = m["files"].as_object()?.keys().cloned().collect();
    (safe(dir) && !ps.is_empty() && ps.iter().all(|p| safe(p) && abs.join(p).is_file()) && !linked(cwd, dir)).then(|| (m, ps, sha(&bytes)))
}
/// What the person is deciding on: the folder, the files as listed, the manifest they saw.
pub struct Asked { thread: String, cwd: PathBuf, dir: String, files: Vec<String>, manifest: String, server: String }
/// Od.asked: the approval is kept on disk too (`<home>/orders/ask-<id>.json`, 0600), so a hub restarted (or updated) while the person looks at the files
/// keeps the card and its Send still sends exactly what they saw. Nothing secret is in it: the folder, the file names, the manifest's hash, the server.
fn asked_path(home: &Path, ask: &str) -> Option<PathBuf> { (ask.len() <= 80 && !ask.is_empty() && ask.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')).then(|| home.join("orders").join(format!("ask-{ask}.json"))) }
fn asked_keep(home: &Path, ask: &str, q: &Asked) -> std::io::Result<()> {
    let Some(f) = asked_path(home, ask) else { return Ok(()) }; let d = home.join("orders"); std::fs::DirBuilder::new().mode(0o700).recursive(true).create(&d)?;
    let j = json!({"thread":q.thread,"cwd":q.cwd.to_string_lossy(),"dir":q.dir,"files":q.files,"manifest":q.manifest,"server":q.server}).to_string();
    let tmp = d.join(format!(".ask-{ask}.tmp")); let _ = std::fs::remove_file(&tmp);
    std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?.write_all(j.as_bytes())?; crate::plat::fs::rename(&tmp, &f)
}
fn asked_take(home: &Path, ask: &str) -> Option<Asked> {
    let f = asked_path(home, ask)?; let j: Value = serde_json::from_str(&std::fs::read_to_string(&f).ok()?).ok()?; let _ = std::fs::remove_file(&f);
    let files: Vec<String> = j["files"].as_array()?.iter().filter_map(Value::as_str).map(str::to_string).collect();
    Some(Asked { thread: sv(&j, "thread").into(), cwd: PathBuf::from(sv(&j, "cwd")), dir: sv(&j, "dir").into(), files, manifest: sv(&j, "manifest").into(), server: sv(&j, "server").into() })
}
/// Rec.asks: an order's approval stays open across a restart, as its questions do (its Send is kept on disk).
pub fn survives_restart(tool: &str) -> bool { tool.starts_with("order_ask") || tool == "order_quote" }
/// Od.quote: reads the manifest, refuses what must not go, and asks. It never waits on the person (an MCP call held open dies), and nothing leaves here.
async fn quote(ag: &Arc<Agents>, thread: &str, cwd: &str, a: &Value) -> (bool, Value) {
    if crate::copy::on() { return fail(COPY_SENDS_NOTHING) }
    let dir = sv(a, "dir").trim().to_string(); let root = PathBuf::from(cwd);
    let (d2, r2) = (dir.clone(), root.clone());
    let got = tokio::task::spawn_blocking(move || sendable(&r2, &d2)).await.ok().flatten();
    let Some((m, files, manifest)) = got else { return fail(&format!("no manifest.json with files to send in {dir} (paths must stay inside the folder, plain characters only, and the folder must hold plain files, no links)")) };
    let server = crate::settings::fab::server_of(&ag.hub.setting("order.server").await.unwrap_or_default());
    let now = now_s(); let ask = hub_id("q", now, ag.seq_n()); let detail = ask_detail(&dir, &m, &files);
    let open: Vec<String> = ag.world().await.asks.keys().cloned().collect();
    let asked = Asked { thread: thread.into(), cwd: root, dir, files, manifest, server }; let _ = asked_keep(&ag.hub.cfg.home, &ask, &asked);
    { let mut q = ag.order_asks.lock().unwrap(); q.retain(|k, _| open.contains(k)); q.insert(ask.clone(), asked); }
    let pend = ag.pending_ev(thread, 1, "", now).await;
    ag.commit_events(vec![json!({"$":"AskOpened","id":ask,"thread":thread,"kind":"approval","tool":"order_quote","detail":detail,"options":"","at":now}), pend]).await;
    (true, json!(QUOTE_ASKED))
}
/// Od.wait: the person answered the files' approval. Send uploads; Not now tells the agent; a retired ask has no answer, and tells no one.
pub fn answered(ag: &Arc<Agents>, ask: &str, thread: &str, answer: &str) {
    let mem = ag.order_asks.lock().unwrap().remove(ask); let disk = asked_take(&ag.hub.cfg.home, ask);
    let Some(q) = mem.or(disk) else {
        // an approval from before a restart whose list was not kept (an older hub's): the person's Send is not lost on the agent
        if matches!(answer, "accept" | "session") && !thread.is_empty() { let (ag, th) = (ag.clone(), thread.to_string()); tokio::spawn(async move { wake(&ag, &th, LOST).await }); }
        return
    };
    let ag = ag.clone(); let answer = answer.to_string();
    tokio::spawn(async move { match answer.as_str() { "accept" | "session" => go(&ag, q).await, "decline" | "deny" => wake(&ag, &q.thread, DECLINED).await, _ => {} } });
}

// ---- sending and following ----

fn heads(install: &str) -> Vec<String> { vec![format!("X-Backplane-Install: {install}"), "Accept: application/json".into()] }
/// Od.upload: one multipart request from the order's folder, by the checked relative paths; its URL and headers in a 0600 config, never on the command line.
fn upload(home: &Path, server: &str, install: &str, dir: &Path, ps: &[String]) -> Value {
    let q = crate::settings::webhook::q; let sec = home.join("secrets");
    if std::fs::DirBuilder::new().mode(0o700).recursive(true).create(&sec).is_err() { return json!({"error":"the upload could not be prepared"}) }
    let nonce = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())); let (cfg, out) = (sec.join(format!("o-{nonce}.curl")), sec.join(format!("o-{nonce}.out")));
    let mut c = format!("url = {}\n", q(&format!("{server}/v1/quotes"))); for h in heads(install) { c.push_str(&format!("header = {}\n", q(&h))) }
    c.push_str(&format!("form = {}\n", q("manifest=<manifest.json"))); for p in ps { c.push_str(&format!("form = {}\n", q(&format!("file=@\"{p}\";filename=\"{p}\"")))) }
    c.push_str(&format!("output = {}\nwrite-out = \"%{{http_code}}\"\n", q(&out.to_string_lossy())));
    let wrote = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&cfg).and_then(|mut f| f.write_all(c.as_bytes()));
    let code = if wrote.is_ok() { crate::proc::std_cmd(crate::proc::Kind::Net, "curl").args(["-sS", "--max-time", "300", "-K"]).arg(&cfg).current_dir(dir).output().ok()
        .map(|o| { let s = String::from_utf8_lossy(&o.stdout).trim().to_string(); let c: String = s.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect(); if c.len() == 3 { c } else { "000".into() } }).unwrap_or_else(|| "000".into()) } else { "000".into() };
    let body = std::fs::read_to_string(&out).unwrap_or_default(); let _ = std::fs::remove_file(&cfg); let _ = std::fs::remove_file(&out);
    crate::settings::fab::est_got(&code, &body)
}
async fn get(home: &Path, r: &Rec) -> Value {
    let (home, r) = (home.to_path_buf(), r.clone());
    tokio::task::spawn_blocking(move || { let hs = [format!("Authorization: Bearer {}", r.token), format!("X-Backplane-Install: {}", crate::settings::fab::install_id(&home))];
        let (c, b) = crate::settings::fab::curl(&home, &format!("{}/v1/quotes/{}", r.server, r.id), &hs, None); crate::settings::fab::est_got(&c, &b) }).await.unwrap_or(Value::Null)
}
/// Hub.order.wake: the agent hears what came of it as a system note (the conversation hides it), and a quote on its way no longer is.
async fn wake(ag: &Arc<Agents>, thread: &str, text: &str) {
    let now = now_s(); let key = format!("order.wait.{thread}"); let waiting = !ag.hub.setting(&key).await.unwrap_or_default().is_empty();
    let unwait: Vec<Value> = if waiting { vec![json!({"$":"SettingSet","key":key,"value":"","at":now})] } else { vec![] };
    if ag.deliver_as("system", thread, &hub_id("m", now, ag.seq_n()), &format!("[Backplane] {text}"), None, unwait.clone()).await.is_err() && !unwait.is_empty() { ag.commit_events(unwait).await }
}
/// Hub.order.quoted: the quote for the card, by Order.sum.keeps; the same again writes nothing.
async fn card_set(ag: &Arc<Agents>, thread: &str, s: &str, id: &str, fresh: bool) {
    let key = format!("order.quote.{thread}"); let cur = ag.hub.setting(&key).await.unwrap_or_default();
    if sum_keeps(fresh, &cur, id) && cur != s { ag.commit_events(vec![json!({"$":"SettingSet","key":key,"value":s,"at":now_s()})]).await }
}
/// Hub.order.note: an order's progress in its thread, with its note alert.
async fn progress(ag: &Arc<Agents>, r: &Rec, st: &str, tracking: &str) {
    let text = status_text(st, tracking); if text.is_empty() { return }
    let (now, key) = (now_s(), format!("o:{}:{st}", r.id)); let ping = status_note(st, tracking);
    let mut ev = vec![json!({"$":"MessagePosted","thread":r.thread,"msg":key,"role":"assistant","text":text,"at":now})];
    if !ping.is_empty() { let (n, prev) = (ag.hub.setting("notify.settled").await.unwrap_or_default(), ag.hub.setting(&format!("alert.{}", r.thread)).await.unwrap_or_default());
        ev.extend(crate::state::fam::turnend::order_note(&n, &prev, ag.seq_n(), now, &r.thread, &key, &ping)); }
    ag.commit_events(ev).await;
}
#[cfg(test)] const TICK: Duration = Duration::from_millis(40);
#[cfg(not(test))] const TICK: Duration = Duration::from_secs(3);
#[cfg(test)] fn pace(_: Duration) -> Duration { TICK }
#[cfg(not(test))] fn pace(d: Duration) -> Duration { d }
/// Od.watch: one look every minute while unpaid, every ten after; a change is said once in the buy thread and kept in the record.
async fn watch(ag: Arc<Agents>, r: Rec, mut last: String) {
    let home = ag.hub.cfg.home.clone();
    for _ in 0..9000 {
        tokio::time::sleep(pace(every(&last))).await;
        let q = get(&home, &r).await; let st = state(&q);
        if st.is_empty() { continue } // no answer: the server may be away; ask again
        // the card first, then the step's message: whoever reads the message finds the card already at that step
        if st != last { card_set(&ag, &r.thread, &sum(&q), &r.id, false).await; progress(&ag, &r, &st, sv(&q["order"], "tracking")).await; let _ = keep(&home, &r, &st); last = st.clone(); }
        if done(&st) { return }
    }
}
/// Od.quoted: post it, keep it for the card, tell the agent, and follow it.
async fn quoted(ag: &Arc<Agents>, r: Rec, q: Value) {
    if sv(&q, "status").is_empty() { return wake(ag, &r.thread, &format!("The quote could not be worked out: {}.", q["error"].as_str().unwrap_or("no answer from the order server"))).await }
    let md = quote_md(&q);
    if !md.trim().is_empty() { ag.commit_events(vec![json!({"$":"MessagePosted","thread":r.thread,"msg":format!("q:{}", r.id),"role":"assistant","text":md,"at":now_s()})]).await }
    let mut s = q.clone(); s["id"] = json!(r.id); card_set(ag, &r.thread, &sum(&s), &r.id, true).await;
    let _ = keep(&ag.hub.cfg.home, &r, "quoted");
    wake(ag, &r.thread, &format!("The quote is posted in this thread. {}", said(&q))).await;
    tokio::spawn(watch(ag.clone(), r, "quoted".into()));
}
/// Od.poll then Od.quoted: until it is worked out. A moment the server cannot be reached does not end it; only the time does, and a quote that slow is still posted when it comes.
async fn poll(ag: Arc<Agents>, r: Rec) {
    let home = ag.hub.cfg.home.clone();
    for i in 0..600 {
        tokio::time::sleep(if i < 60 { TICK } else { TICK * 10 }).await;
        let q = get(&home, &r).await;
        if pending(&q) || q["error"] == crate::settings::fab::UNREACHABLE { continue }
        return quoted(&ag, r, q).await
    }
    wake(&ag, &r.thread, "The quote could not be worked out: the order server did not finish it in time; ask order_status in a few minutes.").await
}
/// Od.go: the person said yes. The thread waits on the quote from here until the agent is told what came of it.
async fn go(ag: &Arc<Agents>, q: Asked) {
    let home = ag.hub.cfg.home.clone(); let now = now_s(); let th = q.thread.clone();
    ag.commit_events(vec![json!({"$":"SettingSet","key":format!("order.wait.{th}"),"value":now.to_string(),"at":now})]).await;
    let (h2, s2, cwd, dir, files, seen) = (home.clone(), q.server.clone(), q.cwd.clone(), q.dir.clone(), q.files.clone(), q.manifest.clone());
    let up = tokio::task::spawn_blocking(move || {
        // the approval covers what the person saw: anything moved since is not sent (the server holds each file to the manifest's hash)
        match sendable(&cwd, &dir) { Some((_, ps, m)) if ps == files && m == seen => {} _ => return json!({"error":"the folder changed after the person approved it, so nothing was sent: export it again and ask once more"}) }
        upload(&h2, &s2, &crate::settings::fab::install_id(&h2), &cwd.join(&dir), &files)
    }).await.unwrap_or(Value::Null);
    let (id, token) = (sv(&up, "id"), sv(&up, "token"));
    if token.is_empty() || !id_ok(id) { return wake(ag, &th, &format!("The files were not taken: {}.", up["error"].as_str().unwrap_or("no answer"))).await }
    let r = Rec { id: id.into(), token: token.into(), thread: th, server: q.server.clone() };
    let _ = keep(&home, &r, "quoting"); poll(ag.clone(), r).await;
}
/// Od.status.now: from the thread first: a quote being made is never called unpayable.
async fn status(ag: &Arc<Agents>, thread: &str) -> (bool, Value) {
    let files = ag.world().await.asks.values().any(|a| a.thread == thread && a.tool == "order_quote");
    if files { return (true, json!(STATUS_UNSENT)) }
    if !ag.hub.setting(&format!("order.wait.{thread}")).await.unwrap_or_default().is_empty() { return (true, json!(STATUS_SENT)) }
    let home = ag.hub.cfg.home.clone();
    let Some((r, _)) = records(&home).into_iter().find(|(r, _)| r.thread == thread) else { return (true, json!("no quote yet in this thread")) };
    if crate::copy::on() { return fail(COPY_SENDS_NOTHING) }
    (true, json!(said(&get(&home, &r).await).to_string()))
}
/// Od.resume: at start, every order not done is followed again, and a quote the hub was getting is waited on and posted. On a copy: nothing, not even the links.
pub fn resume(ag: Arc<Agents>) {
    if crate::copy::on() { return }
    let home = ag.hub.cfg.home.clone();
    tokio::spawn(async move {
        for (r, st) in records(&home) {
            PAY.write().unwrap().insert(r.id.clone(), pay_of(&r.server, &r.id, &r.token));
            if done(&st) { continue }
            if st == "quoting" { let now = now_s(); ag.commit_events(vec![json!({"$":"SettingSet","key":format!("order.wait.{}", r.thread),"value":now.to_string(),"at":now})]).await; tokio::spawn(poll(ag.clone(), r)); continue }
            // its card is filled from a first look whatever its state (one missed stays on Details while its reply has Buy now), and a step it took
            // while the hub was away is said now, not a watch's pause later (paid, placed or shipped could otherwise wait ten minutes for its message)
            let q = get(&home, &r).await; let now = state(&q);
            if !now.is_empty() { card_set(&ag, &r.thread, &sum(&q), &r.id, false).await }
            let st = if now.is_empty() || now == st { st } else { progress(&ag, &r, &now, sv(&q["order"], "tracking")).await; let _ = keep(&home, &r, &now); now };
            if done(&st) { continue }
            tokio::spawn(watch(ag.clone(), r, st));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> std::path::PathBuf { let p = std::env::temp_dir().join(format!("bp-order-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn put(p: std::path::PathBuf, t: &str) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, t).unwrap() }
    const PRINT_SH: &str = "find . \\( -path ./mfg -o -path ./extra -o -path ./.git -o -path ./.backplane -o -name node_modules -o -name '*backup*' \\) -prune -o -type f -not -name '_autosave-*' \\( -name '*.kicad_pcb' -o -name '*.kicad_sch' -o -name '*.step' -o -name '*.stp' -o -name '*.dxf' -o -name '*.fab.json' \\) -print 2>/dev/null | LC_ALL=C sort | while IFS= read -r f; do printf '%s\\n' \"$f\"; cat \"$f\"; done | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-32";
    #[test] fn the_fingerprint_is_the_one_a_review_of_0_14_recorded() {
        let r = tmp("print");
        for (f, t) in [("boards/a/a.kicad_pcb", "pcb"), ("boards/a/a.kicad_sch", "sch"), ("mech/case.step", "step"), ("mech/case.STEP", "not this"), ("plate.dxf", "dxf"), ("a.fab.json", "{}"), ("Z.fab.json", "{}"), ("mfg/x.kicad_pcb", "out"), ("extra/j.step", "out"),
            (".git/h.step", "out"), (".backplane/b.step", "out"), ("node_modules/n/z.step", "out"), ("deep/node_modules/z.step", "out"), ("old-backup-1/a.kicad_pcb", "out"), ("backups.kicad_pcb", "out"), ("_autosave-a.kicad_sch", "out"), ("notes.txt", "out"), ("deep/.hidden/ok.step", "in")] { put(r.join(f), t) }
        crate::plat::fs::symlink(r.join("plate.dxf"), r.join("link.dxf")).unwrap();
        let o = std::process::Command::new("sh").args(["-c", PRINT_SH]).current_dir(&r).output().unwrap();
        assert_eq!(fingerprint(&r), String::from_utf8_lossy(&o.stdout).trim());
        let before = fingerprint(&r); put(r.join("notes.txt"), "changed"); put(r.join("mfg/x.kicad_pcb"), "changed"); assert_eq!(fingerprint(&r), before, "what is not the design does not stale a review");
        put(r.join("plate.dxf"), "changed"); assert_ne!(fingerprint(&r), before);
        let e = tmp("empty"); assert_eq!(fingerprint(&e), "e3b0c44298fc1c149afbf4c8996fb924");
    }
    #[test] fn a_review_is_current_only_for_the_design_it_saw() {
        assert_eq!(review_state("a", &Value::Null), "none"); assert_eq!(review_state("a", &json!({"fingerprint":"a"})), "current"); assert_eq!(review_state("b", &json!({"fingerprint":"a"})), "stale");
        let rec = review_rec("fp", &json!({"verdict":" ok ","summary":"  fine  ","issues":["  a  ","",7,"b"]}), 9);
        assert_eq!(rec, json!({"fingerprint":"fp","verdict":"ok","summary":"fine","issues":["a","b"],"at":9}));
        assert_eq!(review_rec("fp", &json!({"verdict":"bad"}), 9)["verdict"], "issues", "a verdict is ok or issues, nothing else");
        let many: Vec<String> = (0..12).map(|i| format!("i{i}")).collect(); assert_eq!(review_rec("fp", &json!({"issues":many}), 1)["issues"].as_array().unwrap().len(), 8);
        assert_eq!(review_said("fp", &rec), json!({"state":"current","verdict":"ok","summary":"fine","issues":["a","b"],"at":9}));
        assert_eq!(review_said("x", &json!({"fingerprint":"old","verdict":"ok","summary":"s","at":3}))["issues"], json!([]), "a review kept by 0.14 has no issues key");
    }
    #[test] fn the_gate_is_the_reviews_state_and_verdict() {
        assert_eq!(gate_kind(&json!({"state":"current","verdict":"ok"})), 0); assert_eq!(gate_kind(&json!({"state":"current","verdict":"issues"})), 2);
        assert_eq!(gate_kind(&json!({"state":"stale","verdict":"ok"})), 1); assert_eq!(gate_kind(&json!({"state":"none"})), 1);
        assert_eq!(gate_points("a; b.;", &json!([])), ["a", "b"], "no issues recorded: the summary cut at its semicolons");
        assert_eq!(gate_points("sum", &json!(["one. ", " two"])), ["one", "two"]);
        assert_eq!(gate_md(&json!({"summary":"walls thin","issues":[]})), "**The manufacturing review found what to fix before ordering:**\n\n- walls thin\n\nDetails: `.backplane/review.md`");
    }
    #[test] fn an_order_questions_answer_is_the_persons_message() {
        assert_eq!(ask_value("Germany · last time"), "Germany"); assert_eq!(ask_value("Germany"), "Germany");
        assert_eq!(answer_text("  Germany · last time "), "Germany"); assert_eq!(answer_text(" 3 boards "), "3 boards"); assert_eq!(answer_text("  "), SKIPPED);
        assert_eq!(ask_choices("Germany", &["France".into(), " Germany ".into(), "".into()]), "Germany · last time\nFrance"); assert_eq!(ask_choices("  ", &["a".into(), " b ".into()]), "a\nb"); assert_eq!(ask_choices("", &[]), "");
        assert_eq!((ask_tool_name(true), ask_tool_name(false)), ("order_ask.typed", "order_ask"));
    }
    fn ready(pilot: bool) -> Value { json!({"id":"q_1","status":"ready","pilot":pilot,"quote":{"total":112.4,"lead_days":9,"pay_label":"Buy now · $112.40","markdown":"| a | b |","problems":[],"warnings":["w"],"shipping":{"ddp":true}}}) }
    #[test] fn a_quote_says_what_it_covers_and_its_link_never_holds_the_token() {
        let q = ready(false); assert!(payable(&q) && !pending(&q)); assert_eq!(state(&q), "ready");
        assert_eq!(quote_md(&q), "| a | b |\n\nIncludes import VAT and duties: nothing to pay on delivery.\n\n[Buy now · $112.40](order:q_1)");
        let mut bad = ready(false); bad["quote"]["problems"] = json!(["No price for Germany"]); bad["quote"]["shipping"]["ddp"] = json!(false); assert!(!payable(&bad));
        assert_eq!(quote_md(&bad), "| a | b |\n\nExcludes customs: the courier will contact you to pay any duties and taxes before delivery.", "no link for a quote that cannot be paid");
        assert!(said(&bad)["note"].as_str().unwrap().contains("NO Buy now link")); assert!(said(&json!({"status":"analysing"}))["note"].as_str().unwrap().contains("being made"), "a quote still being made is never called unpayable");
        let t = said(&q).to_string(); assert!(!t.contains("order:") && !t.contains("?t="));
        assert_eq!(total_of("Buy now · $5.00", &json!(9)), "$5.00"); assert_eq!(total_of("", &json!(9)), "$9"); assert_eq!(total_of("", &Value::Null), "");
    }
    #[test] fn a_pilots_payment_is_no_order_and_alerts_no_one() {
        let mut q = ready(true); q["order"] = json!({"status":"paid"}); assert_eq!(state(&q), "pilot"); assert!(done("pilot")); assert_eq!(status_note("pilot", ""), ""); assert!(status_text("pilot", "").contains("your card was not charged"));
        let mut q = ready(false); q["order"] = json!({"status":"cancelled"}); assert_eq!(state(&q), "ready", "a checkout that expired unpaid is no order");
        q["order"] = json!({"status":"shipped"}); assert_eq!(state(&q), "shipped"); assert_eq!(status_note("shipped", "T1"), "Shipped. Tracking: T1"); assert_eq!(status_text("shipped", ""), "**Shipped.**");
        assert_eq!(status_note("expired", ""), "", "Expired alerts no one"); assert!(!status_text("expired", "").is_empty());
        assert_eq!(every("ready"), Duration::from_secs(60)); assert_eq!(every("placed"), Duration::from_secs(600));
    }
    #[test] fn the_card_follows_the_order() {
        let marks = |c: &Value| c["steps"].as_array().unwrap().iter().map(|s| s["mark"].as_str().unwrap().to_string()).collect::<Vec<_>>().join(",");
        let c = card("Lamp", false, false, false, true, ""); assert_eq!(c["line"], "Getting the order ready…"); assert_eq!(marks(&c), "on,,,,,"); assert_eq!(c["name"], "Lamp");
        assert_eq!(card("L", false, false, false, false, "")["line"], "Reply below to go on."); assert_eq!(card("L", true, false, false, false, "")["line"], "Backplane needs your answer below.");
        let c = card("L", false, true, false, false, ""); assert_eq!(marks(&c), "done,on,,,,"); assert_eq!(c["line"], "Check the files below and send them for a quote.");
        let c = card("L", false, false, true, false, ""); assert_eq!(c["line"], "Getting your quote… usually 1-2 min"); assert_eq!(c["waiting"], true);
        let r = Rec { id: "q_card".into(), token: "tok".into(), thread: "t".into(), server: "http://s".into() }; PAY.write().unwrap().insert(r.id.clone(), pay_of(&r.server, &r.id, &r.token));
        let mut q = ready(false); q["id"] = json!("q_card"); let s = sum(&q); assert!(!s.contains("tok"), "the setting holds no token");
        let c = card("L", false, false, false, false, &s); assert_eq!(marks(&c), "done,done,done,on,,"); assert_eq!((c["total"].as_str(), c["pay"].as_str(), c["label"].as_str()), (Some("$112.40"), Some("http://s/q/q_card?t=tok"), Some("Buy now · $112.40")));
        assert_eq!(c["customs"], "Includes import VAT and duties: nothing to pay on delivery.");
        let mut bad = ready(false); bad["id"] = json!("q_card"); bad["quote"]["problems"] = json!(["No price for Mars"]);
        assert_eq!(card("L", false, false, false, false, &sum(&bad))["line"], "The quote has something to fix first: see it above.", "the posted quote is above the card, on every client");
        q["order"] = json!({"status":"placed"}); let c = card("L", false, false, false, false, &sum(&q)); assert_eq!(marks(&c), "done,done,done,done,on,"); assert_eq!(c["pay"], "", "paid: no Buy now"); assert_eq!(c["line"], "Ordered: your order is in production.");
        q["order"] = json!({"status":"delivered"}); assert_eq!(marks(&card("L", false, false, false, false, &sum(&q))), "done,done,done,done,done,done");
    }
    #[test] fn a_newer_quote_takes_the_card_and_an_older_ones_watcher_does_not() {
        assert!(sum_keeps(true, r#"{"id":"q_2"}"#, "q_1")); assert!(sum_keeps(false, "", "q_1")); assert!(sum_keeps(false, r#"{"id":"q_1"}"#, "q_1")); assert!(!sum_keeps(false, r#"{"id":"q_2"}"#, "q_1"));
    }
    #[test] fn the_real_link_is_only_in_what_is_drawn() {
        PAY.write().unwrap().insert("q_shown".into(), "http://s/q/q_shown?t=tok".into());
        assert_eq!(pay_shown("a\n\n[Buy now · $1](order:q_shown) b"), "a\n\n[Buy now · $1](http://s/q/q_shown?t=tok) b"); assert_eq!(pay_shown("[x](order:q_none)"), "[x](order:q_none)", "unknown stays inert"); assert_eq!(pay_shown("plain (order:q_shown)"), "plain (order:q_shown)");
    }
    #[test] fn only_plain_paths_and_plain_files_are_sent() {
        for p in ["a.zip", "main/gerbers.zip", "with space.csv"] { assert!(safe(p), "{p}") }
        for p in ["", "/etc/passwd", "../x", "a/../../b", "a\"b", "a\\b", "a;b", "a\nb", "a\u{7f}"] { assert!(!safe(p), "{p:?}") }
        let r = tmp("send"); let f = "mfg/1/a.zip"; put(r.join(f), "z"); let m = json!({"ship_to":"Germany","parts":[{"name":"Main","qty":5,"assemble":2}],"files":{"a.zip":"h"}}); put(r.join("mfg/1/manifest.json"), &m.to_string());
        let (_, ps, h) = sendable(&r, "mfg/1").unwrap(); assert_eq!(ps, ["a.zip"]); assert_eq!(h.len(), 64);
        assert_eq!(ask_detail("mfg/1", &m, &ps), "Send 1 file from mfg/1 to Backplane's quote service? They are kept only to make what you order.\nShips to Germany · Main ×5 (2 assembled)\n\na.zip");
        let us = json!({"ship_to":"US","ship_region":"GA","parts":[{"name":"Main","qty":5}],"files":{"a.zip":"h"}}); assert!(ask_detail("mfg/1", &us, &ps).contains("\nShips to US (GA) · Main ×5\n"), "the state it ships to is shown with the country");
        assert!(sendable(&r, "mfg/../mfg/1").is_none()); assert!(sendable(&r, "mfg/2").is_none(), "no manifest");
        crate::plat::fs::symlink("/etc/passwd", r.join("mfg/1/b.zip")).unwrap(); assert!(sendable(&r, "mfg/1").is_none(), "a folder holding a link is refused, listed or not"); std::fs::remove_file(r.join("mfg/1/b.zip")).unwrap();
        let o = tmp("elsewhere"); put(o.join("3/manifest.json"), &m.to_string()); put(o.join("3/a.zip"), "z"); crate::plat::fs::symlink(&o, r.join("linked")).unwrap(); assert!(sendable(&r, "linked/3").is_none(), "nor a link on the way there");
        put(r.join("mfg/4/manifest.json"), r#"{"files":{"gone.zip":"h"}}"#); assert!(sendable(&r, "mfg/4").is_none(), "a listed file that is not there"); put(r.join("mfg/5/manifest.json"), r#"{"files":{}}"#); assert!(sendable(&r, "mfg/5").is_none());
    }
    #[test] fn a_record_is_private_and_a_server_id_cannot_leave_the_folder() {
        for id in ["../x", "q_../x", "q_a/b", "x", "", "q_a.b"] { assert!(!id_ok(id), "{id}") } assert!(id_ok("q_Ab-1_c"));
        let h = tmp("rec"); let r = Rec { id: "q_rec".into(), token: "tok".into(), thread: "t1".into(), server: "http://s".into() }; keep(&h, &r, "quoting").unwrap();
        let f = h.join("orders/q_rec.json"); assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600), 0o600); assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(h.join("orders")).unwrap().permissions().mode() & 0o777, 0o700), 0o700);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), r#"{"id":"q_rec","token":"tok","thread":"t1","server":"http://s","status":"quoting"}"#, "the shape fab.look's search and 0.14's records have");
        keep(&h, &r, "quoted").unwrap(); let v = records(&h); assert_eq!((v.len(), v[0].1.as_str()), (1, "quoted")); assert_eq!(pay_link("q_rec").unwrap(), "http://s/q/q_rec?t=tok");
        std::fs::write(h.join("orders/q_bad.json"), r#"{"id":"../../x","token":"t"}"#).unwrap(); assert_eq!(records(&h).len(), 1, "a record with a bad id is not followed");
    }
    #[test] fn order_tools_follow_ordering_and_the_buy_thread() {
        let names = |on, buy| defs(on, buy).iter().map(|t| sv(t, "name").to_string()).collect::<Vec<_>>();
        assert!(names(false, true).is_empty(), "Ordering off lists none"); assert_eq!(names(true, false), ["order_review"], "every thread reviews"); for n in ["order_quote", "order_status", "order_ask", "order_review"] { assert!(names(true, true).contains(&n.to_string()), "{n}") }
        for t in defs(true, true) { assert_eq!(t["inputSchema"]["type"], "object"); assert!(!sv(&t, "description").is_empty()); }
    }
}
