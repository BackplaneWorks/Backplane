//! Comments on what is selected in KiCad. Hub state only: nothing here is ever written into a design file.
//!
//! - KiCad's own "Comment to Backplane…" (the fork's ET_COMMENT, bp.9+): the person selected things, right-clicked and wrote a note in KiCad's
//!   dialog. The event's screenshot (KiCad keeps only its 32 newest) is copied beside the hub's data, and the comment is published as info
//!   `kicad.comment.<digest>` {seq, project, frame, note, names, items, bbox, sheet_path, image}; the window that shows the project's editor
//!   posts it into its open thread with `kicad.comment.post`.
//! - Backplane's comment window (Ctrl+; wherever the keyboard is): `editor.kicad.capture {project, frame}` answers what is selected now, the
//!   same shape without a note, with the screenshot copied. On bp.13 it asks CommentSelection{capture_only}: no event, no dialog. Before
//!   (bp.9-bp.12) it asks CommentSelection with no note, which also pushes an ET_COMMENT: that one is the capture's, and is swallowed here
//!   (`Pending`), so it is never posted as a comment of its own.
//! - `kicad.comment.post {thread, note, frame, names, items?, image?, empty?, sheet_path?, voice?}` posts a comment as the person's message
//!   (`text`): the note, the selection's names (the thread draws references as links that cross-probe), each item, the picture. A phone's
//!   viewer posts the same shape (names, an optional image, the note), so a comment reads the same wherever it was made.
use super::{embed, kserve as ks, proto};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, sync::{Arc, Mutex, Weak}};

static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);
pub fn start(hub: &Arc<Hub>) { *HUB.lock().unwrap() = Some(Arc::downgrade(hub)); hub.register_rpc(Arc::new(CommentRpc)); }

/// The chords the embedded editors hand to Backplane (SetReservedKeys, at each KiCad's start): the fork's own five, Ctrl+; for a comment
/// (no KiCad default and no Backplane shortcut uses it), and the management panels' Alt+Shift+B/C/L/F (BOM, Checks, Lifecycle, Fab: client
/// keys.rs `mgmt-*`, their default bindings), so they open the panel while KiCad has the keyboard. The order is the Windows message's index
/// (native_child.rs `reserved_chord_index`).
pub const RESERVED: [&str; 10] = ["Ctrl+K", "Ctrl+J", "Ctrl+Tab", "Ctrl+Shift+Tab", "Ctrl+`", "Ctrl+;", "Alt+Shift+B", "Alt+Shift+C", "Alt+Shift+L", "Alt+Shift+F"];

/// The chords to reserve on a runtime: `RESERVED`, and on bp.13 (its CommentSelection has `capture_only`, and ';' is KiCad's own "Comment to
/// Backplane…" key there) a bare ';' too, so that key opens Backplane's window and never KiCad's dialog. Index 10 on Windows.
pub fn reserved(semicolon_is_kicads: bool) -> Vec<&'static str> { let mut v = RESERVED.to_vec(); if semicolon_is_kicads { v.push(";") } v }

/// Whether `ev` is a comment (ET_COMMENT = 19).
pub fn is_comment(ev: &Value) -> bool { ev["type"] == "ET_COMMENT" || ev["type"].as_i64() == Some(19) }

fn seq_of(v: &Value) -> String { v.as_str().map(String::from).unwrap_or_else(|| v.as_u64().map_or_else(String::new, |n| n.to_string())) }

/// The published comment from an event (or a capture's answer in the same shape: {seq, frame: {kind}, comment}): the note, the selection's
/// names and up to 40 items, with `image` the copied screenshot.
pub fn published(pro: &str, ev: &Value, image: &str) -> Value {
    let c = &ev["comment"];
    let items: Vec<Value> = c["items"].as_array().into_iter().flatten().take(40).cloned().collect();
    json!({"seq": ev["seq"], "project": pro, "frame": ev["frame"]["kind"], "note": c["note"].as_str().or(ev["detail"].as_str()).unwrap_or(""),
           "names": c["names"], "items": items, "truncated": c["truncated"].as_bool().unwrap_or(false) || c["items"].as_array().map_or(0, Vec::len) > 40,
           "bbox": c["bbox"], "sheet_path": c["sheet_path"], "empty": c["empty_selection"].as_bool().unwrap_or(false), "image": image, "at": crate::hub::now_ms()})
}

/// Captures in flight before bp.13: their CommentSelection pushes an ET_COMMENT too, which may come before or after the answer.
#[derive(Default, Debug)]
pub struct Pending { inflight: HashMap<String, u32>, answered: HashSet<(String, String)>, early: HashSet<(String, String)> }
impl Pending {
    pub fn begin(&mut self, pro: &str) { *self.inflight.entry(pro.to_string()).or_default() += 1; }
    /// The answer came (seq "" when it failed): its event is swallowed when it comes, unless it already was.
    pub fn end(&mut self, pro: &str, seq: &str) {
        if let Some(n) = self.inflight.get_mut(pro) { *n = n.saturating_sub(1); if *n == 0 { self.inflight.remove(pro); } }
        if seq.is_empty() || seq == "0" { return }
        let k = (pro.to_string(), seq.to_string());
        if !self.early.remove(&k) { self.answered.insert(k); }
    }
    /// An ET_COMMENT: true when it is a capture's (never the person's dialog, never an agent's comment with a note).
    pub fn swallow(&mut self, pro: &str, ev: &Value) -> bool {
        let k = (pro.to_string(), seq_of(&ev["seq"]));
        if self.answered.remove(&k) { return true }
        let c = &ev["comment"];
        let mine = self.inflight.get(pro).is_some_and(|n| *n > 0) && c["from_api"] == true && c["note"].as_str().unwrap_or("").is_empty();
        if mine { self.early.insert(k); }
        mine
    }
}
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
fn pending<R>(f: impl FnOnce(&mut Pending) -> R) -> R { f(PENDING.lock().unwrap().get_or_insert_with(Default::default)) }

/// The screenshot copied beside the hub's data (KiCad keeps only its newest): its path, "" when there is none.
async fn copy_image(hub: &Arc<Hub>, pro: &str, tag: &str, src: &str) -> String {
    if src.is_empty() { return String::new() }
    let dir = hub.cfg.home.join("kicad-comments"); let to = dir.join(format!("{}-{tag}.png", ks::digest(pro)));
    let src = src.to_string();
    let ok = tokio::task::spawn_blocking({ let to = to.clone(); move || std::fs::create_dir_all(&dir).and_then(|_| std::fs::copy(&src, &to)).is_ok() }).await.unwrap_or(false);
    if ok { to.to_string_lossy().into_owned() } else { String::new() }
}

/// From the fork's events loop (embed.rs): true when it was a comment (handled here).
pub fn on_event(pro: &str, ev: &Value) -> bool {
    if !is_comment(ev) { return false }
    if pending(|p| p.swallow(pro, ev)) { return true }
    let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) else { return true };
    let (pro, ev) = (pro.to_string(), ev.clone());
    tokio::spawn(async move {
        let image = copy_image(&hub, &pro, &seq_of(&ev["seq"]), ev["comment"]["screenshot"].as_str().unwrap_or("")).await;
        hub.set_info(&format!("kicad.comment.{}", ks::digest(&pro)), &published(&pro, &ev, &image).to_string()).await;
    });
    true
}

/// The frame kind a capture asks for: the schematic editor for "SCH" (any case, or FK_SCH), else the board's.
pub fn kind_of(frame: &str) -> &'static str { if frame.eq_ignore_ascii_case("sch") || frame == "FK_SCH" || frame.eq_ignore_ascii_case("schematic") { "FK_SCH" } else { "FK_PCB" } }

/// What CommentSelection is asked: capture only when the runtime knows it (bp.13), else with no note (its event is swallowed).
pub fn request(kind: &str, capture_only: bool) -> Value { if capture_only { json!({"kind": kind, "capture_only": true}) } else { json!({"kind": kind, "note": ""}) } }

/// A comment as the person's message: their note, then what they had selected in KiCad (references, pins and nets, which the thread draws as
/// links that cross-probe; each item's layer or sheet and position in mm), then the picture (`[attachment: <path>]` so the thread, the phones
/// and the agent all get it). English: it is the person's message to an agent, as they wrote it. `c`: {note, frame, names, items, empty,
/// sheet_path, truncated, image}; any may be missing (a phone's viewer gives names, the note and maybe an image).
pub fn text(c: &Value) -> String {
    let list = |k: &str| c["names"][k].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>();
    let (refs, nets, pins) = (list("references"), list("nets"), list("pins"));
    let ed = if c["frame"] == "FK_SCH" || c["frame"] == 2 || c["frame"] == "schematic" { "schematic" } else { "board" };
    let mut t = c["note"].as_str().unwrap_or("").trim().to_string();
    if c["empty"] == true { t.push_str(&format!("\n\n(KiCad {ed}, nothing selected: the area in view)")); }
    else if !(refs.is_empty() && pins.is_empty() && nets.is_empty()) {
        let mut sel = Vec::new();
        if !refs.is_empty() { sel.push(refs.join(", ")); }
        if !pins.is_empty() { sel.push(format!("pins {}", pins.join(", "))); }
        if !nets.is_empty() { sel.push(format!("nets {}", nets.join(", "))); }
        t.push_str(&format!("\n\nSelected in the KiCad {ed}: {}", sel.join("; ")));
        if let Some(sp) = c["sheet_path"].as_str().filter(|s| !s.is_empty() && *s != "/") { t.push_str(&format!(" (sheet {sp})")); }
    }
    let items = c["items"].as_array().cloned().unwrap_or_default();
    for it in items.iter().take(12) {
        let name = [it["reference"].as_str(), it["pin"].as_str(), it["net"].as_str()].into_iter().flatten().find(|s| !s.is_empty()).unwrap_or("");
        let at = match (it["x_mm"].as_f64(), it["y_mm"].as_f64()) { (Some(x), Some(y)) => format!(" at ({x:.2}, {y:.2}) mm"), _ => String::new() };
        let place = it["layer"].as_str().or(it["sheet_path"].as_str()).filter(|s| !s.is_empty()).map(|l| format!(" on {l}")).unwrap_or_default();
        t.push_str(&format!("\n- {} {name}{place}{at}", it["kind"].as_str().unwrap_or("item").to_lowercase()));
    }
    if items.len() > 12 || c["truncated"] == true { t.push_str(&format!("\n- … and more ({} items listed of the selection)", items.len().min(12))); }
    if let Some(img) = c["image"].as_str().filter(|s| !s.is_empty()) { t.push_str(&format!("\n\n[attachment: {img}]")); }
    t.trim_start().to_string()
}

/// A picture a post may attach: only one in the hub's own data (the copied screenshots, a phone's upload), never any file a caller names.
pub fn image_ok(home: &std::path::Path, img: &str) -> bool {
    if img.is_empty() { return true }
    let p = std::path::Path::new(img);
    p.is_absolute() && !img.split(['/', '\\']).any(|x| x == "..") && p.starts_with(home)
}

/// `editor.kicad.capture` and `kicad.comment.post`.
pub struct CommentRpc;
impl RpcHandler for CommentRpc {
    fn methods(&self) -> &'static [&'static str] { &["editor.kicad.capture", "kicad.comment.post"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let err = |e: &str| vec![json!({"t":"reply","id":id,"ok":false,"error":e})];
            if m == "kicad.comment.post" {
                let thread = p["thread"].as_str().unwrap_or("");
                if thread.is_empty() { return err("a comment needs a thread") }
                let mut c = p.clone();
                if !image_ok(&hub.cfg.home, c["image"].as_str().unwrap_or("")) { c["image"] = json!(""); }
                let body = text(&c);
                if body.is_empty() { return err("nothing to post") }
                let Some(h) = hub.rpc_for("turn.start") else { return err("no agents") };
                let mut tp = json!({"thread": thread, "text": body, "mode": "steer"});
                if p["voice"].is_object() { tp["voice"] = p["voice"].clone(); }
                return h.call(hub, who, id, "turn.start", &tp).await
            }
            let pro = match embed::resolve_pro(p["project"].as_str().unwrap_or("")) { Ok(p) => p, Err(e) => return err(&e) };
            if !super::known_root(hub, std::path::Path::new(&pro).parent().and_then(|p| p.to_str()).unwrap_or(""), "").await { return err("not in a project this hub knows") }
            let Some(gui) = embed::gui_route(&pro) else { return err("KiCad is not running") };
            let schema = proto::load(&gui.desc);
            if gui.mode != "fork" || !schema.has_own("kiapi.backplane.embed.CommentSelection") { return err("this KiCad cannot comment (needs Backplane KiCad bp.9)") }
            let only = schema.has_field("kiapi.backplane.embed.CommentSelection", "capture_only");
            let kind = kind_of(p["frame"].as_str().unwrap_or(""));
            let t0 = std::time::Instant::now();
            if !only { pending(|q| q.begin(&pro)); }
            let o = embed::gui_call(&gui, "kiapi.backplane.embed.CommentSelection", request(kind, only)).await;
            let seq = seq_of(&o["message"]["seq"]);
            if !only { pending(|q| q.end(&pro, &seq)); }
            if !o["ok"].as_bool().unwrap_or(false) { return err(&format!("CommentSelection: {}", o["error"].as_str().unwrap_or(""))) }
            let ms = t0.elapsed().as_millis() as u64;
            let ev = json!({"seq": seq, "frame": {"kind": kind}, "comment": o["message"]["comment"]});
            let tag = if seq.is_empty() || seq == "0" { format!("c{}", crate::hub::now_ms()) } else { seq.clone() };
            let image = copy_image(hub, &pro, &tag, o["message"]["comment"]["screenshot"].as_str().unwrap_or("")).await;
            let mut r = published(&pro, &ev, &image); r["capture_only"] = json!(only); r["ms"] = json!(ms);
            vec![json!({"t":"reply","id":id,"ok":true,"result":r})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_comment_event_is_published_with_its_note_names_and_copied_image() {
        let ev = json!({"seq": "4", "type": "ET_COMMENT", "detail": "move this", "frame": {"kind": "FK_PCB"},
            "comment": {"note": "move this", "names": {"references": ["U3"], "nets": ["GND"]}, "items": [{"kind": "FOOTPRINT", "reference": "U3", "layer": "F.Cu", "x_mm": 10.0, "y_mm": 20.0}], "bbox": {"x_min": 9.0}}});
        assert!(is_comment(&ev) && !is_comment(&json!({"type": "ET_SELECTION_CHANGED"})));
        let p = published("/p/a.kicad_pro", &ev, "/h/kicad-comments/x.png");
        assert_eq!(p["note"], "move this"); assert_eq!(p["names"]["references"][0], "U3"); assert_eq!(p["image"], "/h/kicad-comments/x.png"); assert_eq!(p["frame"], "FK_PCB");
    }
    fn ev(seq: &str, from_api: bool, note: &str) -> Value { json!({"seq": seq, "type": "ET_COMMENT", "comment": {"note": note, "from_api": from_api}}) }
    #[test] fn a_captures_own_event_is_swallowed_whether_it_comes_before_or_after_the_answer() {
        let mut p = Pending::default();
        // after the answer
        p.begin("a"); p.end("a", "7"); assert!(p.swallow("a", &ev("7", true, "")), "the answer's seq");
        assert!(!p.swallow("a", &ev("7", true, "")), "once");
        // before it: from the API, no note, while one is in flight
        p.begin("a"); assert!(p.swallow("a", &ev("8", true, ""))); p.end("a", "8");
        assert!(!p.swallow("a", &ev("8", true, "")), "the answer does not swallow a later one of the same seq");
        assert!(p.answered.is_empty() && p.early.is_empty() && p.inflight.is_empty(), "nothing kept: {p:?}");
    }
    #[test] fn the_persons_dialog_and_an_agents_note_are_never_swallowed() {
        let mut p = Pending::default(); p.begin("a");
        assert!(!p.swallow("a", &ev("3", false, "")), "KiCad's own dialog (not from the API)");
        assert!(!p.swallow("a", &ev("4", true, "move U3")), "an agent's CommentSelection with a note");
        assert!(!p.swallow("b", &ev("5", true, "")), "another project");
        p.end("a", ""); assert!(!p.swallow("a", &ev("6", true, "")), "nothing in flight");
    }
    #[test] fn capture_only_where_the_runtime_has_it_and_a_noteless_comment_before() {
        assert_eq!(request("FK_SCH", true), json!({"kind": "FK_SCH", "capture_only": true}));
        assert_eq!(request("FK_PCB", false), json!({"kind": "FK_PCB", "note": ""}));
        assert_eq!((kind_of("SCH"), kind_of("sch"), kind_of("FK_SCH"), kind_of("PCB"), kind_of("")), ("FK_SCH", "FK_SCH", "FK_SCH", "FK_PCB", "FK_PCB"));
        assert_eq!(RESERVED[5], "Ctrl+;"); assert_eq!(RESERVED.len(), 10, "the Windows index of each stays");
        assert_eq!(RESERVED[6..], ["Alt+Shift+B", "Alt+Shift+C", "Alt+Shift+L", "Alt+Shift+F"], "the management panels' keys reach Backplane from KiCad");
        assert_eq!(reserved(false).len(), 10); assert_eq!(reserved(true)[10], ";", "bp.13: KiCad's own ';' comes to Backplane");
    }
    #[test] fn a_comment_reads_as_the_persons_message_with_its_references_and_picture() {
        let c = json!({"frame": "FK_PCB", "note": "Move U3 closer to J1", "names": {"references": ["U3"], "nets": ["GND"], "pins": []},
            "items": [{"kind": "FOOTPRINT", "reference": "U3", "layer": "F.Cu", "x_mm": 10.0, "y_mm": 20.5}], "image": "/h/kicad-comments/a-4.png"});
        let t = text(&c);
        assert!(t.starts_with("Move U3 closer to J1\n\nSelected in the KiCad board: U3; nets GND"), "{t}");
        assert!(t.contains("- footprint U3 on F.Cu at (10.00, 20.50) mm") && t.ends_with("\n\n[attachment: /h/kicad-comments/a-4.png]"), "{t}");
        assert!(text(&json!({"frame": "FK_SCH", "note": "here", "empty": true})).contains("nothing selected"));
    }
    #[test] fn a_phone_viewers_comment_has_names_a_note_and_maybe_a_picture() {
        let t = text(&json!({"frame": "schematic", "note": "Is R4 right?", "names": {"references": ["R4"]}}));
        assert_eq!(t, "Is R4 right?\n\nSelected in the KiCad schematic: R4");
        let t = text(&json!({"note": "", "names": {"references": ["U1", "U2"]}, "image": "/h/attachments/p.png"}));
        assert_eq!(t, "Selected in the KiCad board: U1, U2\n\n[attachment: /h/attachments/p.png]", "no note: no leading blank lines");
        assert_eq!(text(&json!({"note": "  "})), "");
    }
    #[test] fn a_post_attaches_only_pictures_in_the_hubs_own_data() {
        let home = std::path::Path::new("/h/home");
        assert!(image_ok(home, "") && image_ok(home, "/h/home/kicad-comments/a-4.png"));
        assert!(!image_ok(home, "/etc/passwd") && !image_ok(home, "/h/home/../x.png") && !image_ok(home, "kicad-comments/a.png") && !image_ok(home, "/h/homeother/a.png"));
    }
}
