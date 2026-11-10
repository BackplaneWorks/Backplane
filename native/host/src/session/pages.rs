//! Diff and Review without the policy helper: the reads they wait for (git.diff, review.bom/checks/tick, git.act/git.revert) applied to the session, their actions (`rv-*`, `revert`,
//! the opening of the Review tab), the Dv.sync post-hook, and the two JSON objects the thread carries (`diff`, `viewer.review`). src/core/client is the reference:
//! `Ui.diff`/`Dv.*` (Diff), `Rp.*` (Review), `Ui.revert*`; the diff text is read as src/core/diff reads it. Pure functions over the request's session; the session keeps the same
//! keys `Ui` keeps (`diff:<t>`, `diff.none:<t>`, `diff.at:<t>`, `diff.auto:<t>`, `view.kind:<t>`, `view.path:<t>`, `rp.md:<t>:<page>` ..., the scratch `@rp.busy:<t>`, `@rp.req:<id>`,
//! `@revert`), so the helper, where there is one, reads the same state.
use super::{composer, patch, select};
use crate::md;
use serde_json::{json, Map, Value};

/// What an action answers (the same three fields as thread_actions::Out: the dispatcher reads them alike)
pub struct Out { pub session_patch: Value, pub cmds: Vec<Value>, pub look: bool }

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn info<'a>(session: &'a Value, k: &str) -> &'a str { s(&session["info"], k) }
fn draft<'a>(session: &'a Value, k: &str) -> &'a str { s(&session["drafts"], k) }

// ---- the unified diff ----

pub struct DLine { pub k: u32, pub t: String, pub o: usize, pub n: usize }
pub struct DHunk { pub header: String, pub lines: Vec<DLine> }
pub struct DFile { pub new: String, pub status: String, pub hunks: Vec<DHunk> }

struct PH { header: String, lines: Vec<DLine>, o: usize, n: usize, ro: usize, rn: usize }
struct PF { old: String, new: String, status: String, hunks: Vec<DHunk> }
#[derive(Clone, Copy, PartialEq)]
enum T { Ctx, Add, Del, Meta, Diff, Hunk, Old, New, Added, Deleted, RenFrom, RenTo, CopyFrom, CopyTo, Bin, Skip }

const BODY: [(&str, T); 4] = [(" ", T::Ctx), ("+", T::Add), ("-", T::Del), ("\\", T::Meta)];
const HEAD: [(&str, T); 13] = [("diff --git ", T::Diff), ("@@ ", T::Hunk), ("\\", T::Meta), ("--- ", T::Old), ("+++ ", T::New), ("new file mode", T::Added), ("deleted file mode", T::Deleted),
    ("rename from ", T::RenFrom), ("rename to ", T::RenTo), ("copy from ", T::CopyFrom), ("copy to ", T::CopyTo), ("Binary files ", T::Bin), ("GIT binary patch", T::Bin)];
fn tag(ts: &[(&str, T)], l: &str) -> T { ts.iter().find(|(p, _)| l.starts_with(p)).map_or(T::Skip, |x| x.1) }
/// Diff.kind: inside a hunk that still expects lines the body tags first (a blank line is context), else the header tags
fn kind(room: bool, l: &str) -> T {
    if !room { return tag(&HEAD, l) }
    match tag(&BODY, l) { T::Skip => if l.is_empty() { T::Ctx } else { tag(&HEAD, l) }, t => t }
}
fn drop_chars(l: &str, n: usize) -> String { l.chars().skip(n).collect() }
fn untab(x: &str) -> &str { x.strip_suffix('\t').unwrap_or(x) }
/// Diff.path: "a/x\t" -> "x"
fn path(x: &str) -> String { let x = untab(x); let c: Vec<char> = x.chars().collect(); if c.get(1) == Some(&'/') { c[2..].iter().collect() } else { x.to_string() } }
/// Diff.git_names: "a/P b/P" -> P (renames take their names from the rename lines)
fn git_names(rest: &str) -> (String, String) {
    let c: Vec<char> = rest.chars().collect(); let h = c.len().saturating_sub(5) / 2;
    let a: String = c.iter().skip(2).take(h).collect(); let b: String = c.iter().skip(h + 5).collect();
    if a == b { (a.clone(), a) } else { (rest.to_string(), rest.to_string()) }
}
fn num(x: &str) -> usize { x.parse().unwrap_or(0) }
/// Diff.rg: "-12,3" -> 12, 3; "+4" -> 4, 1
fn rg(w: Option<&&str>) -> (usize, usize) {
    let Some(w) = w else { return (0, 0) };
    let ps: Vec<&str> = w.chars().next().map_or("", |c| &w[c.len_utf8()..]).split(',').collect();
    match ps.as_slice() { [a, b, ..] => (num(a), num(b)), [a] => (num(a), 1), [] => (0, 0) }
}
fn close(h: PH) -> DHunk { DHunk { header: h.header, lines: h.lines } }
fn flush(f: PF, h: Option<PH>) -> PF { let mut f = f; if let Some(h) = h { f.hunks.push(close(h)) } f }
fn done(f: PF) -> DFile { DFile { new: f.new, status: f.status, hunks: f.hunks } }
fn or_new(f: Option<PF>) -> PF { f.unwrap_or(PF { old: String::new(), new: String::new(), status: "modified".into(), hunks: vec![] }) }

/// Diff.parse: the files of a unified diff, in order
pub fn parse(text: &str) -> Vec<DFile> {
    let (mut files, mut file, mut hunk): (Vec<DFile>, Option<PF>, Option<PH>) = (vec![], None, None);
    for l in text.split('\n') {
        let room = hunk.as_ref().map_or(false, |h| h.ro + h.rn > 0);
        match kind(room, l) {
            T::Diff => {
                if let Some(f) = file.take() { files.push(done(flush(f, hunk.take()))) }
                hunk = None; let (a, b) = git_names(&drop_chars(l, 11));
                file = Some(PF { old: a, new: b, status: "modified".into(), hunks: vec![] });
            }
            T::Hunk => {
                let f = flush(or_new(file.take()), hunk.take()); file = Some(f);
                let ws: Vec<&str> = l.split(' ').collect();
                let ((o, ro), (n, rn)) = (rg(ws.get(1)), rg(ws.get(2)));
                hunk = Some(PH { header: l.to_string(), lines: vec![], o, n, ro, rn });
            }
            T::Old => { let x = drop_chars(l, 4); if untab(&x) != "/dev/null" { let mut f = or_new(file.take()); f.old = path(&x); file = Some(f) } }
            T::New => { let x = drop_chars(l, 4); if untab(&x) != "/dev/null" { let mut f = or_new(file.take()); f.new = path(&x); file = Some(f) } }
            T::Added => { let mut f = or_new(file.take()); f.status = "added".into(); file = Some(f) }
            T::Deleted => { let mut f = or_new(file.take()); f.status = "deleted".into(); file = Some(f) }
            T::RenFrom => { let mut f = or_new(file.take()); f.old = drop_chars(l, 12); f.status = "renamed".into(); file = Some(f) }
            T::RenTo => { let mut f = or_new(file.take()); f.new = drop_chars(l, 10); file = Some(f) }
            T::CopyFrom => { let mut f = or_new(file.take()); f.old = drop_chars(l, 10); f.status = "copied".into(); file = Some(f) }
            T::CopyTo => { let mut f = or_new(file.take()); f.new = drop_chars(l, 8); file = Some(f) }
            T::Bin => { let mut f = flush(or_new(file.take()), hunk.take()); f.hunks.push(DHunk { header: String::new(), lines: vec![DLine { k: 3, t: l.to_string(), o: 0, n: 0 }] }); file = Some(f) }
            T::Skip => {}
            t => if let Some(h) = hunk.as_mut() {
                match t {
                    T::Ctx => { h.lines.push(DLine { k: 0, t: drop_chars(l, 1), o: h.o, n: h.n }); h.o += 1; h.n += 1; h.ro = h.ro.saturating_sub(1); h.rn = h.rn.saturating_sub(1) }
                    T::Add => { h.lines.push(DLine { k: 1, t: drop_chars(l, 1), o: 0, n: h.n }); h.n += 1; h.rn = h.rn.saturating_sub(1) }
                    T::Del => { h.lines.push(DLine { k: 2, t: drop_chars(l, 1), o: h.o, n: 0 }); h.o += 1; h.ro = h.ro.saturating_sub(1) }
                    _ => h.lines.push(DLine { k: 3, t: l.to_string(), o: 0, n: 0 }),
                }
            },
        }
    }
    if let Some(f) = file.take() { files.push(done(flush(f, hunk.take()))) }
    files
}

/// Diff.summary: "3 files, +12 −4"; a capped diff still counts what it left out (its meta lines)
pub fn summary(files: &[DFile]) -> String {
    let (mut n, mut a, mut d) = (0usize, 0usize, 0usize);
    for f in files { n += 1; for h in &f.hunks { for l in &h.lines { match l.k {
        1 => a += 1, 2 => d += 1,
        3 => {
            let w: Vec<&str> = l.t.split(' ').collect();
            if l.t.starts_with("\\ Too big to show here: +") { a += num(&drop_chars(w.get(6).copied().unwrap_or(""), 1)); d += num(&drop_chars(w.get(7).copied().unwrap_or(""), 1)) }
            else if l.t.ends_with(" more changed files not shown here") { n += num(w.get(1).copied().unwrap_or("")) }
        }
        _ => {}
    } } } }
    format!("{n} {}, +{a} −{d}", if n == 1 { "file" } else { "files" })
}

/// The line rows one Diff page carries. The diff's own caps are bytes (scripts/diff_cap.sh: 64 KiB a file, 128 KiB in all), and a diff of short lines
/// (an order's Gerbers in mfg/: 9,000 lines in those 128 KiB) made a picture past the 512 KiB a client takes in one frame: the desktop dropped the
/// connection at every press of Diff. A file that no longer fits is one "Too big to show here" line, like a file past the byte cap; the summary counts it all.
pub const DIFF_ROWS: usize = 2500;
/// Mob.diff.at: what the thread carries as `diff` (null while Diff is not on show)
pub fn diff_object(text: &str, none: &str) -> Value {
    if !none.is_empty() { return json!({"summary": format!("{none} is not a git repository, so there is nothing to compare this thread's files with."), "files": []}) }
    let files = parse(text);
    let no = |n: usize| if n == 0 { String::new() } else { n.to_string() };
    let mut left = DIFF_ROWS;
    let rows: Vec<Value> = files.iter().map(|f| {
        let n: usize = f.hunks.iter().map(|h| h.lines.len() + usize::from(!h.header.is_empty())).sum();
        if n > left && n > 2 {
            let count = |k: u32| f.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.k == k).count();
            return json!({"name": f.new, "status": f.status, "lines": [{"k": 3, "t": format!("\\ Too big to show here: +{} -{} lines", count(1), count(2)), "o": "", "n": ""}]})
        }
        left = left.saturating_sub(n);
        let mut lines = Vec::new();
        for h in &f.hunks {
            if !h.header.is_empty() { lines.push(json!({"k": 4, "t": h.header, "o": "", "n": ""})) }
            for l in &h.lines { lines.push(json!({"k": l.k, "t": l.t, "o": no(l.o), "n": no(l.n)})) }
        }
        json!({"name": f.new, "status": f.status, "lines": lines})
    }).collect();
    json!({"summary": if files.is_empty() { "No changes since this thread began.".to_string() } else { summary(&files) }, "files": rows})
}

// ---- the session ----

/// Whether Diff is on show for the thread: its kind, and the panel open (the session's, or the thread's own)
pub fn diff_on(session: &Value, sel: &str) -> bool {
    !sel.is_empty() && info(session, &format!("view.kind:{sel}")) == "diff" && (session["panel"].as_bool().unwrap_or(false) || info(session, &format!("panel:{sel}")) == "1")
}
/// The special kinds this module draws; the drawings and the thread's Browser are viewer.rs's
pub fn kind_of<'a>(session: &'a Value, sel: &str) -> &'a str {
    match info(session, &format!("view.kind:{sel}")) { k @ ("diff" | "review") => k, _ => "" }
}
/// Dv.at: where the thread's turn stands when its diff was asked (the steps' count is not in this state)
pub fn dv_at(f: &dyn select::Facts, th: &str) -> String { format!("{}|0", f.turn_name(th)) }

struct B { rid: u64, hub: String, patch_d: Map<String, Value>, patch_i: Map<String, Value>, cmds: Vec<Value>, panel: Option<bool> }
impl B {
    fn new(hub: &str, session: &Value) -> B { B { rid: session["rid"].as_u64().unwrap_or(1), hub: hub.into(), patch_d: Map::new(), patch_i: Map::new(), cmds: vec![], panel: None } }
    fn rpc(&mut self, m: &str, p: Value) -> u64 {
        let id = self.rid; self.rid += 1;
        self.cmds.push(json!({"type": "send", "hub": self.hub, "text": json!({"id": id, "m": m, "p": p}).to_string()}));
        id
    }
    fn patch(&self, session: &Value) -> Value {
        let mut p = Map::new();
        if self.rid != session["rid"].as_u64().unwrap_or(1) { p.insert("rid".into(), json!(self.rid)); }
        if let Some(v) = self.panel { p.insert("panel".into(), json!(v)); }
        if !self.patch_d.is_empty() { p.insert("drafts".into(), Value::Object(self.patch_d.clone())); }
        if !self.patch_i.is_empty() { p.insert("info".into(), Value::Object(self.patch_i.clone())); }
        Value::Object(p)
    }
    fn view(&mut self, session: &Value, sel: &str, kind: &str, path: &str) {
        // Ui.view_put + Viewer.put: the thread's viewer on the kind, its panel open, the count of changes up one
        let n = info(session, &format!("view.n:{sel}")).parse::<u64>().unwrap_or(0) + 1;
        self.panel = Some(true);
        self.patch_i.insert(format!("view.kind:{sel}"), json!(kind)); self.patch_i.insert(format!("view.path:{sel}"), json!(path));
        self.patch_i.insert(format!("view.n:{sel}"), json!(n.to_string())); self.patch_i.insert(format!("panel:{sel}"), json!("1"));
        self.patch_d.insert("@revert".into(), json!(""));
        // a drawing open before it: its watch stops, and the drawing is no longer the one on show
        if !draft(session, "@view.at").is_empty() {
            let root = draft(session, "@view.at").split('|').nth(1).unwrap_or("");   // "0|<root>|": the key of the source being watched
            let p = json!({"kind": "", "root": root, "path": "", "tree": ""});
            self.rpc("kicad.watch", p); self.patch_d.insert("@view.at".into(), json!(""));
        }
        self.patch_d.insert(format!("@view:{sel}"), json!(""));
    }
}

/// Dv.ask: Diff asked for (by hand: it opens the viewer on it when the reply comes; `auto`: it only puts the text in place)
fn ask_diff(b: &mut B, sel: &str, auto: bool) {
    b.patch_i.insert(format!("diff.auto:{sel}"), json!(if auto { "1" } else { "" }));
    b.rpc("git.diff", json!({"thread": sel, "from": 0, "to": -1}));
}

/// Rp.title for a page ("BOM · Working copy", "Checks · Working copy"); the Comments and Release pages are the thread's own
fn page_title(page: &str) -> String {
    match page { "bom" => "BOM · Working copy".into(), "checks" => "Checks · Working copy".into(), _ => String::new() }
}
/// Rp.ask: the page a review request asks for, marked by its id (`@rp.req:<id>`: a request that fails has no report to end the page's reading)
fn ask_page(b: &mut B, sel: &str, page: &str) { ask_page_ab(b, sel, page, "") }
/// `ab`: for the BOM, the two versions it compares ("<old>\n<new>", a design-history comparison's sides; "" the BOM as it is)
fn ask_page_ab(b: &mut B, sel: &str, page: &str, ab: &str) {
    let m = match page { "bom" => "review.bom", "checks" => "review.checks", _ => return };
    b.patch_d.insert(format!("@rp.busy:{sel}"), json!(page));
    b.patch_i.insert(format!("rp.title:{sel}:{page}"), json!(page_title(page)));
    let (va, vb) = ab.split_once('\n').unwrap_or(("", ""));
    let p = if page == "bom" { json!({"thread": sel, "a": va, "b": vb}) } else { json!({"thread": sel, "commit": "", "trigger": "manual"}) };
    let id = b.rpc(m, p);
    b.patch_d.insert(format!("@rp.req:{id}"), json!(format!("{sel}:{page}")));
}
/// Rp.open: the Review tab on a page, and the page asked for
pub fn open_review(session: &Value, hub: &str, sel: &str, page: &str) -> (Value, Vec<Value>) { open_review_ab(session, hub, sel, page, "") }
pub fn open_review_ab(session: &Value, hub: &str, sel: &str, page: &str, ab: &str) -> (Value, Vec<Value>) {
    let mut b = B::new(hub, session);
    // the watch is stopped for the drawing's own root: kicad.watch names no root once the drawing is put away
    b.view(session, sel, "review", page); ask_page_ab(&mut b, sel, page, ab);
    (b.patch(session), b.cmds)
}
/// The thread's viewer left for something else (a drawing, or closed): Diff and Review go
pub fn leave(session: &Value, sel: &str) -> Value {
    if kind_of(session, sel).is_empty() { return json!({}) }
    json!({"panel": false, "info": {format!("view.kind:{sel}"): "", format!("view.path:{sel}"): "", format!("panel:{sel}"): "", format!("diff.auto:{sel}"): ""}, "drafts": {"@revert": ""}})
}

/// Dv.sync (the post-hook): an open Diff follows its thread; asked again when the turn moved since it was asked
pub fn dv_sync(f: &dyn select::Facts, hub: &str, session: &mut Value, cmds: &mut Vec<Value>) {
    let sel = s(session, "sel").to_string();
    if !diff_on(session, &sel) { return }
    // (a thread of another machine: git.diff goes to that machine, net/rpc.rs; its turn is not in this hub's facts, so it is asked once on show)
    let at = dv_at(f, &sel);
    if info(session, &format!("diff.at:{sel}")) == at { return }
    let mut b = B::new(hub, session);
    b.patch_i.insert(format!("diff.at:{sel}"), json!(at)); ask_diff(&mut b, &sel, true);
    let p = b.patch(session); patch::merge(session, &p); cmds.extend(b.cmds);
}
/// A Diff on show is answered by this module's post-hook, not by the helper
pub fn diff_only(session: &Value) -> bool {
    let sel = s(session, "sel");
    diff_on(session, sel) && !matches!(info(session, &format!("view.kind:{sel}")), "mech")
}

/// The actions this module owns: `rv-page`, `rv-run`, `rv-bom`, `rv-tick`, `revert`, `revert-no`
pub fn act(hub: &str, session: &Value, sel: &str, action: &str, value: &str) -> Option<Out> {
    if sel.is_empty() || sel.contains('~') { return None }
    let mut b = B::new(hub, session);
    match action {
        "rv-page" | "rv-bom" => {
            let page = if action == "rv-bom" || value.is_empty() { "bom" } else { value };
            if !matches!(page, "bom" | "checks" | "comments" | "release") { return None }
            // rv-bom "<old>\n<new>": the parts that changed between a comparison's two versions (0.14's compare bar BOM)
            let ab = if action == "rv-bom" && value.contains('\n') { value } else { "" };
            let (p, cmds) = open_review_ab(session, hub, sel, page, ab);
            return Some(Out { session_patch: p, cmds, look: true });
        }
        "rv-run" => {
            let page = if info(session, &format!("view.kind:{sel}")) == "review" { match info(session, &format!("view.path:{sel}")) { "" => "bom", p => p } } else { return None };
            ask_page(&mut b, sel, page);
        }
        "rv-tick" => { b.rpc("review.tick", json!({"thread": sel, "index": value.parse::<u64>().unwrap_or(0)})); }
        // Ui.revert: "ask" arms it for the thread (the diff stays on show while the person decides), "confirm" sends it, a turn number is a revert already confirmed
        "revert" => match value {
            "confirm" => {
                if !reverting(session, sel) { return None }
                let turn = match draft(session, "@revert.turn") { "" => "0", t => t };
                b.patch_d.insert("@revert".into(), json!(""));
                b.rpc("git.revert", json!({"thread": sel, "turn": turn.parse::<u64>().unwrap_or(0)}));
            }
            "ask" => {
                b.view(session, sel, "diff", "");
                b.patch_d.insert("@revert".into(), json!(sel)); b.patch_d.insert("@revert.turn".into(), json!("0"));
                if info(session, &format!("diff:{sel}")).is_empty() && info(session, &format!("diff.none:{sel}")).is_empty() { ask_diff(&mut b, sel, true); }
            }
            n => { b.patch_d.insert("@revert".into(), json!("")); b.rpc("git.revert", json!({"thread": sel, "turn": n.parse::<u64>().unwrap_or(0)})); }
        },
        "revert-no" => { b.patch_d.insert("@revert".into(), json!("")); }
        _ => return None,
    }
    let p = b.patch(session);
    Some(Out { session_patch: p, cmds: b.cmds, look: true })
}
/// Ui.reverting: armed for this thread, with Diff on show
pub fn reverting(session: &Value, sel: &str) -> bool { !sel.is_empty() && draft(session, "@revert") == sel && diff_on(session, sel) }

/// Why a request failed: the reply's own words.
fn why_of(r: &Value) -> String { r["error"].as_str().or_else(|| r["result"]["error"].as_str()).or_else(|| r["error"]["message"].as_str()).unwrap_or("The request failed.").to_string() }
/// The reads a view request carries, applied to the session (Ui.reply.diff, Rp.reply, Rp.failed, Rp.ticked); the commands a read makes (a Diff on show asked again after a
/// commit or a revert). `reads` are `{kind, method?, id, ok, result, error?}`; a method not named is told by the shape of its result.
pub fn apply_reads(f: &dyn select::Facts, hub: &str, session: &mut Value, reads: &[Value]) -> Vec<Value> {
    let mut cmds = Vec::new();
    for r in reads {
        let sel = s(session, "sel").to_string();
        let (res, ok, id) = (&r["result"], r["ok"].as_bool().unwrap_or(true), r["id"].as_u64().unwrap_or(0));
        let method = match r["method"].as_str() { Some(m) => m, None if res.get("report").is_some() => "review.bom", None if res.get("ticked").is_some() => "review.tick", None if res.get("diff").is_some() => "git.diff", None => "" };
        let mut p = json!({});
        if ok && res.get("board_list").is_some() { if let Some(k) = crate::present::ready::keep_boards(res) { patch::merge(session, &k); } continue }
        // a side question's answer (Btw.got): for the question still on show, in whatever thread; a failure says why in its place
        if r["kind"] == "thread.btw" {
            let th = s(res, "thread"); let q = s(res, "q");
            if !th.is_empty() && draft(session, &format!("@btw:{th}")) == q {
                let a = if ok { composer::btw_answer(s(res, "btw")) } else { why_of(r) };
                patch::merge(session, &json!({"drafts": {format!("@btw.a:{th}"): a}}));
            }
            continue
        }
        // the intent written (Intent.saved.got, Intent.return): the card is the answer's, the conversation gets its composer back; a refusal is the client's to show, and the text stays
        if r["kind"] == "fab.intent" {
            if !ok { continue }
            if let Some(k) = crate::present::ready::keep(res) { patch::merge(session, &k); }
            let th = s(res, "thread").to_string(); let back = draft(session, &format!("@intent.draft:{th}")).to_string();
            patch::merge(session, &json!({"drafts": {th.clone(): back, format!("@intent.draft:{th}"): "", format!("@intent.edit:{th}"): ""}}));
            cmds.push(json!({"type": "keep", "thread": format!("{hub}|{th}"), "text": back})); cmds.push(json!({"type": "compose", "thread": th, "text": back}));
            cmds.push(json!({"type": "say", "tone": "note", "text": "Intent saved"}));
            continue
        }
        if r["kind"] == "fab.look" && ok { if let Some(k) = crate::present::ready::keep(res) { patch::merge(session, &k); } continue }
        match method {
            "git.diff" if res.get("diff").is_some() => {
                let th = match s(res, "thread") { "" => sel.as_str(), t => t }.to_string();
                let mut i = Map::new(); let mut d = Map::new();
                i.insert(format!("diff:{th}"), json!(s(res, "diff"))); i.insert(format!("diff.none:{th}"), json!(s(res, "nogit")));
                if info(session, &format!("diff.auto:{th}")) == "1" { i.insert(format!("diff.auto:{th}"), json!("")); }
                else {
                    // an answer to Diff asked for by hand opens its thread's viewer on it
                    let n = info(session, &format!("view.n:{th}")).parse::<u64>().unwrap_or(0) + 1;
                    i.insert(format!("diff.at:{th}"), json!(dv_at(f, &th))); i.insert(format!("panel:{th}"), json!("1"));
                    i.insert(format!("view.kind:{th}"), json!("diff")); i.insert(format!("view.path:{th}"), json!("")); i.insert(format!("view.n:{th}"), json!(n.to_string()));
                    p = json!({"info": i, "drafts": d});
                    if th == sel { p["panel"] = json!(true); }
                    // the drawing that was on show is put away (its watch stopped)
                    if th == sel && !draft(session, "@view.at").is_empty() {
                        let rid = session["rid"].as_u64().unwrap_or(1);
                        let root = draft(session, "@view.at").split('|').nth(1).unwrap_or("").to_string();
                        cmds.push(json!({"type": "send", "hub": hub, "text": json!({"id": rid, "m": "kicad.watch", "p": {"kind": "", "root": root, "path": "", "tree": ""}}).to_string()}));
                        p["rid"] = json!(rid + 1); p["drafts"]["@view.at"] = json!(""); p["drafts"][format!("@view:{th}")] = json!("");
                    }
                    patch::merge(session, &p); continue;
                }
                p = json!({"info": i, "drafts": d});
            }
            "review.bom" | "review.checks" | "review.release" if res.get("report").is_some() => {
                let rep = &res["report"]; let k = format!("{}:{}", s(rep, "thread"), s(rep, "page"));
                let mut i = Map::new();
                i.insert(format!("rp.md:{k}"), json!(s(rep, "md"))); i.insert(format!("rp.err:{k}"), json!(s(rep, "err")));
                i.insert(format!("rp.gate:{k}"), json!(s(rep, "gate"))); i.insert(format!("rp.line:{k}"), json!(s(rep, "line")));
                i.insert(format!("rp.tasks:{k}"), json!(rep.get("tasks").filter(|t| t.is_array()).map_or("[]".to_string(), |t| t.to_string())));
                p = json!({"info": i, "drafts": {format!("@rp.busy:{}", s(rep, "thread")): "", format!("@rp.req:{id}"): ""}});
            }
            "review.tick" if res.get("ticked").is_some() => {
                let t = &res["ticked"];
                p = json!({"info": {format!("rp.tasks:{}:checks", s(t, "thread")): t.get("tasks").filter(|x| x.is_array()).map_or("[]".to_string(), |x| x.to_string())}, "drafts": {format!("@rp.req:{id}"): ""}});
            }
            // a review request that failed: its page stops reading and says why
            "review.bom" | "review.checks" | "review.tick" | "review.release" if !ok => {
                let key = draft(session, &format!("@rp.req:{id}")).to_string();
                if !key.is_empty() {
                    let why = r["error"].as_str().or_else(|| res["error"].as_str()).or_else(|| r["error"]["message"].as_str()).unwrap_or("The request failed.");
                    let th = key.split(':').next().unwrap_or("").to_string();
                    p = json!({"info": {format!("rp.err:{key}"): why}, "drafts": {format!("@rp.busy:{th}"): "", format!("@rp.req:{id}"): ""}});
                }
            }
            // a routine save: accepted, the form closes; refused, it stays open with the hub's reason (`routine.error`)
            "bots.routine" => {
                if !draft(session, &format!("@o.req:{id}")).is_empty() {
                    let mut d = json!({format!("@o.req:{id}"): ""});
                    if ok { for k in ["@onew", "@f.oid", "@f.oname", "@f.ocron", "@f.oprompt", "@f.oerr"] { d[k] = json!(""); } }
                    else { d["@f.oerr"] = json!(r["error"].as_str().or_else(|| res["error"].as_str()).or_else(|| r["error"]["message"].as_str()).unwrap_or("The routine was not saved.")); }
                    p = json!({"drafts": d});
                }
            }
            // a commit or a revert changes what Diff shows: a Diff on show is asked again
            "git.act" | "git.revert" => {
                if diff_on(session, &sel) {
                    let mut b = B::new(hub, session); ask_diff(&mut b, &sel, true);
                    p = b.patch(session); cmds.extend(b.cmds);
                }
            }
            _ => {}
        }
        patch::merge(session, &p);
    }
    cmds
}

// ---- what the thread carries ----

/// `thread.diff`: null unless Diff is on show; the armed Revert rides with it
pub fn diff_json(session: &Value, sel: &str) -> Value {
    if !diff_on(session, sel) { return Value::Null }
    let mut o = diff_object(info(session, &format!("diff:{sel}")), info(session, &format!("diff.none:{sel}")));
    o["reverting"] = json!(reverting(session, sel));
    o
}

fn tab(page: &str, cur: &str) -> Value {
    let label = match page { "bom" => "BOM", "checks" => "Checks", "comments" => "Comments", _ => "Release" };
    json!({"page": page, "label": label, "on": page == cur})
}
/// `viewer.review`: null unless the Review tab is on (Rv.json: the pages, the title, the report's Markdown as the timeline's nodes, what to say without one)
pub fn review_json(session: &Value, sel: &str) -> Value {
    if sel.is_empty() || info(session, &format!("view.kind:{sel}")) != "review" { return Value::Null }
    let page = match info(session, &format!("view.path:{sel}")) { "" => "bom", p => p };
    let key = |k: &str| format!("{k}:{sel}:{page}");
    let (md_text, err) = (info(session, &key("rp.md")), info(session, &key("rp.err")));
    let busy = draft(session, &format!("@rp.busy:{sel}")) == page;
    let title = match page { "comments" => "Comments · 0 open".to_string(), "release" => String::new(), _ => info(session, &key("rp.title")).to_string() };
    let note = if busy { "Reading the design…".to_string() } else if !err.is_empty() { err.to_string() } else if !md_text.is_empty() { String::new() } else { match page {
        "comments" => "No comments yet. Right-click a place on the board or schematic (long-press on a phone) to comment there.".into(),
        "release" => "Tag this design and make its fabrication outputs: Gerbers, drill, CPL, BOM, PDFs and STEP, in .backplane/releases.".into(),
        _ => "Nothing yet.".into() } };
    let blocks = if md_text.is_empty() { vec![] } else { md::nodes_json(&md::render(md_text)) };
    let checks = if page == "checks" {
        let tasks: Value = serde_json::from_str(info(session, &key("rp.tasks"))).unwrap_or(json!([]));
        let rows: Vec<Value> = tasks.as_array().into_iter().flatten().enumerate().map(|(i, t)| json!({"index": i, "text": s(t, "text"), "done": t["done"].as_bool().unwrap_or(false), "group": s(t, "group")})).collect();
        json!({"gate": info(session, &key("rp.gate")), "line": info(session, &key("rp.line")), "tasks": rows})
    } else { Value::Null };
    let tabs: Vec<Value> = ["bom", "checks", "comments", "release"].iter().map(|p| tab(p, page)).collect();
    json!({"tabs": tabs, "title": title, "busy": busy, "note": note, "blocks": blocks, "checks": checks, "release": null})
}

#[cfg(test)]
mod tests {
    use super::*;
    struct M;
    impl select::Facts for M { fn thread_exists(&self, _: &str) -> bool { true } fn turn_name(&self, _: &str) -> String { "idle".into() } fn order_enabled(&self) -> bool { true } fn estimates_on(&self) -> bool { true } }
    const T: &str = "t1700000002-3";
    fn sess() -> Value { json!({"sel": T, "rid": 5, "panel": false, "drafts": {}, "info": {}}) }
    const DIFF: &str = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,4 @@\n one\n-two\n+2\n three\n+four\ndiff --git a/gone.txt b/gone.txt\ndeleted file mode 100644\nindex 3..0\n--- a/gone.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\n";
    #[test] fn a_unified_diff_is_read_file_by_file_with_numbered_lines() {
        let o = diff_object(DIFF, "");
        assert_eq!(o["summary"], json!("2 files, +2 −2"));
        let f = &o["files"][0];
        assert_eq!((f["name"].as_str(), f["status"].as_str()), (Some("a.txt"), Some("modified")));
        assert_eq!(f["lines"][0], json!({"k": 4, "t": "@@ -1,3 +1,4 @@", "o": "", "n": ""}));
        assert_eq!(f["lines"][2], json!({"k": 2, "t": "two", "o": "2", "n": ""}));
        assert_eq!(f["lines"][3], json!({"k": 1, "t": "2", "o": "", "n": "2"}));
        assert_eq!(f["lines"][5], json!({"k": 1, "t": "four", "o": "", "n": "4"}));
        assert_eq!((o["files"][1]["name"].as_str(), o["files"][1]["status"].as_str()), (Some("gone.txt"), Some("deleted")));
    }
    #[test] fn an_empty_diff_and_a_folder_without_git_say_so() {
        assert_eq!(diff_object("", "")["summary"], json!("No changes since this thread began."));
        assert!(diff_object("", "/w/p")["summary"].as_str().unwrap().starts_with("/w/p is not a git repository"));
    }
    /// e2e n1110: after Buy it exported 23 Gerbers into mfg/, every press of Diff logged "hub frame exceeds 512 KiB" in the desktop and reconnected; Diff never opened.
    #[test] fn a_diff_of_many_short_lines_stays_a_page_a_client_can_take() {
        let file = |name: &str, n: usize| format!("diff --git a/{name} b/{name}\nnew file mode 100644\n--- /dev/null\n+++ b/{name}\n@@ -0,0 +1,{n} @@\n{}", "+X1Y2D01*\n".repeat(n));
        let text = format!("{}{}{}{}", file("mfg/a.gbr", 2000), file("mfg/b.gbr", 2000), file("mfg/c.gbr", 400), file("mfg/d.gbr", 2000));
        let o = diff_object(&text, ""); let fs = o["files"].as_array().unwrap();
        let rows = |i: usize| fs[i]["lines"].as_array().unwrap().len();
        assert_eq!((rows(0), rows(1), rows(2), rows(3)), (2001, 1, 401, 1), "what fits is whole, in order; a file that does not is one line");
        assert_eq!(fs[1]["lines"][0], json!({"k": 3, "t": "\\ Too big to show here: +2000 -0 lines", "o": "", "n": ""}));
        assert_eq!((fs[1]["name"].as_str(), fs[1]["status"].as_str()), (Some("mfg/b.gbr"), Some("added")), "still named, with its state");
        assert!(fs.iter().map(|f| f["lines"].as_array().unwrap().len()).sum::<usize>() <= DIFF_ROWS + fs.len());
        assert_eq!(o["summary"], json!("4 files, +6400 −0"), "the summary counts every line");
        assert!(serde_json::to_vec(&o).unwrap().len() < 200 * 1024, "{} bytes", serde_json::to_vec(&o).unwrap().len());
        // a file the script already capped (two rows) passes as it is, whatever is left
        let capped = format!("{}diff --git a/big b/big\n--- a/big\n+++ b/big\n@@ -0,0 +0,0 @@\n\\ Too big to show here: +619 -595 lines\n", file("mfg/a.gbr", 2500));
        let c = diff_object(&capped, ""); assert_eq!(c["files"][1]["lines"].as_array().unwrap().len(), 2); assert_eq!(c["summary"], json!("2 files, +3119 −595"));
    }
    #[test] fn a_binary_file_is_one_meta_line_and_a_capped_diff_counts_what_it_left_out() {
        let o = diff_object("diff --git a/x.png b/x.png\nBinary files a/x.png and b/x.png differ\n", "");
        assert_eq!(o["files"][0]["lines"][0]["k"], json!(3));
        let c = diff_object("diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-x\n+y\n\\ Too big to show here: +10 -3 lines\n\\ 4 more changed files not shown here\n", "");
        assert_eq!(c["summary"], json!("5 files, +11 −4"));
    }
    #[test] fn the_diff_asked_by_hand_opens_the_viewer_and_the_one_asked_by_the_hook_does_not() {
        let mut s1 = sess();
        let read = json!({"kind": "history.refs", "method": "git.diff", "id": 5, "ok": true, "result": {"diff": DIFF, "thread": T}});
        apply_reads(&M, "h", &mut s1, &[read.clone()]);
        assert!(diff_json(&s1, T)["files"].as_array().unwrap().len() == 2 && s1["panel"] == json!(true));
        assert_eq!(info(&s1, &format!("view.kind:{T}")), "diff");
        // the hook's answer (diff.auto 1) only puts the text in place
        let mut s2 = sess(); s2["info"][format!("diff.auto:{T}")] = json!("1");
        apply_reads(&M, "h", &mut s2, &[read]);
        assert!(diff_json(&s2, T).is_null() && info(&s2, &format!("diff:{T}")) == DIFF && info(&s2, &format!("diff.auto:{T}")).is_empty());
    }
    #[test] fn a_diff_on_show_follows_its_thread_and_a_commit_asks_it_again() {
        let mut s1 = sess(); apply_reads(&M, "h", &mut s1, &[json!({"method": "git.diff", "id": 5, "ok": true, "result": {"diff": "", "thread": T}})]);
        let mut cmds = vec![]; s1["info"][format!("diff.at:{T}")] = json!("running|0");
        dv_sync(&M, "h", &mut s1, &mut cmds);
        assert_eq!(cmds.len(), 1); assert!(cmds[0]["text"].as_str().unwrap().contains("\"m\":\"git.diff\""));
        assert_eq!(info(&s1, &format!("diff.auto:{T}")), "1"); assert_eq!(info(&s1, &format!("diff.at:{T}")), "idle|0");
        let mut again = vec![]; dv_sync(&M, "h", &mut s1, &mut again); assert!(again.is_empty(), "asked once for where the turn stands");
        let c = apply_reads(&M, "h", &mut s1, &[json!({"method": "git.act", "id": 6, "ok": true, "result": {"text": "ok", "thread": T}})]);
        assert_eq!(c.len(), 1, "the commit changed what Diff shows");
    }
    #[test] fn revert_asks_first_then_sends_the_thread_back_to_its_start() {
        let mut s1 = sess(); apply_reads(&M, "h", &mut s1, &[json!({"method": "git.diff", "id": 5, "ok": true, "result": {"diff": DIFF, "thread": T}})]);
        assert!(act("h", &s1, T, "revert", "confirm").is_none(), "nothing armed: nothing to confirm");
        let o = act("h", &s1, T, "revert", "ask").unwrap(); patch::merge(&mut s1, &o.session_patch);
        assert!(reverting(&s1, T) && diff_json(&s1, T)["reverting"] == json!(true));
        let o = act("h", &s1, T, "revert", "confirm").unwrap();
        assert!(o.cmds[0]["text"].as_str().unwrap().contains("\"m\":\"git.revert\"") && o.cmds[0]["text"].as_str().unwrap().contains("\"turn\":0"));
        patch::merge(&mut s1, &o.session_patch); assert!(!reverting(&s1, T));
        let o = act("h", &s1, T, "revert", "ask").unwrap(); patch::merge(&mut s1, &o.session_patch);
        let o = act("h", &s1, T, "revert-no", "").unwrap(); patch::merge(&mut s1, &o.session_patch); assert!(!reverting(&s1, T));
    }
    #[test] fn a_comparisons_bom_asks_for_the_parts_between_its_two_versions() {
        let s1 = sess();
        let o = act("h", &s1, T, "rv-bom", "git\t/p/.git\tabc\t2\t/p/b.kicad_pcb\n/p/b.kicad_pcb").unwrap();
        let t = o.cmds[0]["text"].as_str().unwrap();
        assert!(t.contains("\"m\":\"review.bom\"") && t.contains("abc") && t.contains("\"b\":\"/p/b.kicad_pcb\""), "{t}");
        let plain = act("h", &s1, T, "rv-bom", "").unwrap(); assert!(plain.cmds[0]["text"].as_str().unwrap().contains("\"a\":\"\""), "the BOM as it is");
    }
    #[test] fn the_review_tab_asks_for_the_bom_and_shows_the_report_when_it_comes() {
        let mut s1 = sess();
        let o = act("h", &s1, T, "rv-page", "bom").unwrap(); patch::merge(&mut s1, &o.session_patch);
        assert!(o.cmds[0]["text"].as_str().unwrap().contains("\"m\":\"review.bom\""));
        let r = review_json(&s1, T);
        assert_eq!((r["busy"].clone(), r["note"].clone(), r["title"].clone()), (json!(true), json!("Reading the design…"), json!("BOM · Working copy")));
        assert_eq!(draft(&s1, "@rp.req:5"), format!("{T}:bom"));
        apply_reads(&M, "h", &mut s1, &[json!({"method": "review.bom", "id": 5, "ok": true, "result": {"report": {"thread": T, "page": "bom", "md": "| Ref | Value |\n|---|---|\n| R1 | 10k |\n", "err": ""}}})]);
        let r = review_json(&s1, T);
        assert_eq!((r["busy"].clone(), r["note"].clone()), (json!(false), json!("")));
        assert!(!r["blocks"].as_array().unwrap().is_empty());
        assert_eq!(r["tabs"][0], json!({"page": "bom", "label": "BOM", "on": true}));
    }
    #[test] fn a_review_request_that_failed_stops_the_page_reading_and_says_why() {
        let mut s1 = sess(); let o = act("h", &s1, T, "rv-page", "checks").unwrap(); patch::merge(&mut s1, &o.session_patch);
        apply_reads(&M, "h", &mut s1, &[json!({"method": "review.checks", "id": 5, "ok": false, "error": "no project folder", "result": {}})]);
        let r = review_json(&s1, T); assert_eq!((r["busy"].clone(), r["note"].clone()), (json!(false), json!("no project folder")));
    }
    #[test] fn leaving_for_a_drawing_puts_diff_and_review_away() {
        let mut s1 = sess(); let o = act("h", &s1, T, "rv-page", "bom").unwrap(); patch::merge(&mut s1, &o.session_patch);
        assert_eq!(kind_of(&s1, T), "review");
        let p = leave(&s1, T); patch::merge(&mut s1, &p); assert_eq!(kind_of(&s1, T), ""); assert_eq!(s1["panel"], json!(false)); assert!(review_json(&s1, T).is_null());
    }
}
