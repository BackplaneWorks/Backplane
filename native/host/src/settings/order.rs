//! Ordering: what the agents call (docs/order.md; 0.14's src/core/order and src/server/orderio). `order_review` is every thread's while Ordering is on,
//! the rest are a buy thread's own (the setting `buy.<thread>`). Ordering off refuses all of them, even for an agent that remembers them from before.
//!
//! The design's fingerprint keeps a review current only while the files are as they were: `.backplane/review.json` keeps it with the verdict.
use crate::agents::{lines::hub_id, now_s, Agents};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::Read, os::unix::{ffi::OsStrExt, fs::PermissionsExt}, path::Path, sync::Arc};

fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }

pub const OFF: &str = "Ordering is off; the person can turn it on in Settings";
pub const NOT_BUY: &str = "only a buy thread has this tool";
const ASKED: &str = "Asked on the person's screen. End your turn now: their answer comes as their next message, however long they take.";
/// Order.skipped: a dismissed question, said as the person's message so its agent hears it (silence left the thread waiting).
const SKIPPED: &str = "I skipped this question.";
const LAST_MARK: &str = " · last time";
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
fn ask_tool() -> Value {
    def("order_ask", "Ask the person one question, shown in its own card above the message box on every device: choices they can tap, or with typed a box to type the answer in (where it ships); they may always type their own answer. Answers at once: end your turn, and their answer comes as their next message, however long they take.",
        json!({"question":prop("The question, short and plain."),"options":{"type":"array","items":{"type":"string"},"description":"Two to four short choices, the likely one first; none with typed."},
            "typed":{"type":"boolean","description":"true when the answer is typed (the country it ships to): the person gets a box with Continue."},"last":prop("With typed: the earlier order's answer (its country's name), offered as a quick choice.")}), json!(["question"]))
}
/// Bots.order.tools: what a thread's agent is shown: none with Ordering off, a buy thread's own only in a buy thread.
pub fn defs(on: bool, buy: bool) -> Vec<Value> {
    if !on { return vec![] }
    let mut v = if buy { vec![ask_tool()] } else { vec![] }; v.push(review_tool()); v
}
pub async fn listed(ag: &Arc<Agents>, thread: &str) -> Vec<Value> { let w = ag.world().await; defs(w.setting("order.enabled") != "off", !w.setting(&format!("buy.{thread}")).is_empty()) }

/// Hub.order.on / Hub.order.k: every order tool's call.
pub async fn call(ag: &Arc<Agents>, thread: &str, name: &str, a: &Value) -> (bool, Value) {
    let (on, buy, cwd) = { let w = ag.world().await; (w.setting("order.enabled") != "off", !w.setting(&format!("buy.{thread}")).is_empty(), w.cwd(thread)) };
    if !on { return fail(OFF) }
    if !buy && name != "order_review" { return fail(NOT_BUY) }
    match name {
        "order_ask" => question(ag, thread, a).await,
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
pub fn ask_value(choice: &str) -> &str { choice.strip_suffix(LAST_MARK).map_or(choice, str::trim) }
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
            let name = e.file_name(); let n = name.as_bytes();
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
        if let Ok(mut f) = std::fs::File::open(root.join(std::ffi::OsStr::from_bytes(&p[2..]))) { while let Ok(n) = f.read(&mut buf) { if n == 0 { break } h.update(&buf[..n]) } }
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
        std::os::unix::fs::symlink(r.join("plate.dxf"), r.join("link.dxf")).unwrap();
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
    #[test] fn order_tools_follow_ordering_and_the_buy_thread() {
        let names = |on, buy| defs(on, buy).iter().map(|t| sv(t, "name").to_string()).collect::<Vec<_>>();
        assert!(names(false, true).is_empty(), "Ordering off lists none"); assert_eq!(names(true, false), ["order_review"], "every thread reviews"); assert!(names(true, true).contains(&"order_ask".to_string()));
        for t in defs(true, true) { assert_eq!(t["inputSchema"]["type"], "object"); assert!(!sv(&t, "description").is_empty()); }
    }
}
