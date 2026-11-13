//! DRC and ERC in the open editor (Backplane KiCad bp.13, capability `embed-checks`; the fork's BACKPLANE_IPC.md): `RunChecks` runs
//! on the document as it is in KiCad, unsaved edits included, with no second copy of the board; its `ET_PROGRESS` feeds the panel's bar
//! and its `ET_CHECKS_DONE` ends it; `GetMarkers` then reads what it found. The markers are turned back into the report kicad-cli
//! writes (`report`), read by the same `findings::drc`/`erc`, so a row has the same id whichever way the check ran (an exclusion or a
//! note made on a CLI result stays on its row). What the report needs that a marker may not carry is filled in, each part optional:
//!  - each item's description and place: the marker's `items_detail` when the runtime sends it, else the items looked up
//!    (`GetItemsById`, then the pads' footprints for their references) and described in kicad-cli's words (`describe`);
//!  - the sheet's name: a marker names its sheet by KIID path, the CLI by names ("/Debugger/"), read from the schematic files (`sheets`);
//! Unconnected items and parity findings are markers too (MARKER_RATSNEST, MARKER_PARITY), each in its list by `group` (bp.13 batch 3;
//! by type before). An exclusion's reason (`comment`) goes on its row. The editor's result can differ from the CLI's where they read
//! different things, which the panel says instead of showing findings come and go: unsaved edits ("live, unsaved"), the open
//! schematic for parity, the zones as filled.
//! A runtime without the capability, or an editor that is busy (a dialog, a tool), runs kicad-cli on the saved files as before.
use super::embed::{self, GuiRoute};
use super::findings::{self as fd, Finding};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub const CAP: &str = "embed-checks";
const RUN_SECS: u64 = 300;
/// A check in KiCad that shows no progress for this long is taken as stuck (a message box inside the task, which no one sees): it is
/// cancelled, kicad-cli runs it, and KiCad is not asked again for STUCK_OFF_MS. KiCad sends progress at once and then as it moves.
#[cfg(not(test))] const STALL_MS: u64 = 30_000;
#[cfg(test)] const STALL_MS: u64 = 300;
const STUCK_OFF_MS: u64 = 10 * 60_000;
static STUCK: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
fn stuck_until(sock: &str) -> u64 { STUCK.get_or_init(Default::default).lock().unwrap().get(sock).copied().unwrap_or(0) }

/// Whether the open editor runs checks itself (its `GetCapabilities`: the capability, and the commands when it lists them).
pub fn capable(caps: &Value) -> bool {
    let has = |k: &str, x: &str| caps[k].as_array().is_some_and(|a| a.iter().any(|c| c.as_str().is_some_and(|c| c == x || c.ends_with(&format!(".{x}")))));
    has("capabilities", CAP) && (caps["commands"].as_array().map_or(true, |a| a.is_empty()) || (has("commands", "RunChecks") && has("commands", "GetMarkers")))
}
/// The capability, asked once per editor process (its socket).
static CAPS: OnceLock<Mutex<HashMap<String, (String, bool, bool)>>> = OnceLock::new();
/// The open editor of the project when it runs checks itself.
pub async fn ready(hub: &std::sync::Arc<crate::hub::Hub>, pro: &str) -> Option<GuiRoute> {
    let route = embed::gui_route(pro)?;
    if crate::hub::now_ms() < stuck_until(&route.sock) { return None }
    let m = CAPS.get_or_init(Default::default);
    if let Some((sock, ok, _)) = m.lock().unwrap().get(pro).cloned() { if sock == route.sock { return ok.then_some(route) } }
    // the embedded GUI's own answer (the headless server's manifest never has the embed capabilities)
    let _ = hub;
    let o = embed::gui_call_as(&route, "kiapi.backplane.GetCapabilities", json!({}), embed::CHECKS_CLIENT).await;
    if !o["ok"].as_bool().unwrap_or(false) { return None } // no answer now: ask again next time
    let caps = o["message"].clone();
    let ok = capable(&caps);
    eprintln!("checks: {pro}: {}", if ok { "DRC and ERC run in the open KiCad (embed-checks)" } else { "the open KiCad has no embed-checks: kicad-cli on the saved files" });
    m.lock().unwrap().insert(pro.to_string(), (route.sock.clone(), ok, false));
    ok.then_some(route)
}
/// Whether the project's open editor was found to check itself (no asking: the events loop calls it).
pub fn known(pro: &str) -> bool { CAPS.get_or_init(Default::default).lock().unwrap().get(pro).is_some_and(|x| x.1) && embed::gui_route(pro).is_some_and(|r| CAPS.get().unwrap().lock().unwrap().get(pro).is_some_and(|x| x.0 == r.sock)) }
/// What the panel says of an editor's result, where it may differ from kicad-cli's (the fork's answer, bp.13).
pub const NOTE: &str = "Checked in KiCad: unsaved edits included, zones as filled, parity against the open schematic";
#[cfg(test)] pub fn set_ready(pro: &str, sock: &str, ok: bool) { CAPS.get_or_init(Default::default).lock().unwrap().insert(pro.to_string(), (sock.to_string(), ok, false)); }
/// The rules on which the editor and kicad-cli disagree about which items collide (bp.13 batch 3: Hubble_DK's silk-vs-mask test gives 21
/// pairs more in the editor than the CLI's 67; the fork's parity gate follows it up).
pub const EDITOR_ONLY: &[&str] = &["silk_over_copper"];
pub const EDITOR_ONLY_NOTE: &str = "Reported by KiCad's editor only: the last check of the saved files (kicad-cli) did not report it";
/// The note on an editor's rows of those rules that the last kicad-cli run of the same check did not have (`cli`: its ids; nothing when
/// there was none): shown, not hidden, so a row that goes away when KiCad closes says why. Once the two agree, no row has it.
pub fn mark_editor_only(fs: &mut [Finding], cli: &std::collections::HashSet<String>) {
    if cli.is_empty() { return }
    for f in fs.iter_mut().filter(|f| EDITOR_ONLY.contains(&f.rule.as_str()) && !cli.contains(&f.id)) {
        f.detail = if f.detail.is_empty() { EDITOR_ONLY_NOTE.into() } else { format!("{} · {EDITOR_ONLY_NOTE}", f.detail) };
    }
}

// Pure ---------------------------------------------------------------------------------------------------------------------------

fn kiid(v: &Value) -> String { v["value"].as_str().or(v.as_str()).unwrap_or("").to_string() }
fn int(v: &Value) -> Option<f64> { v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())) }
/// A `Vector2` in millimetres.
fn mm(v: &Value) -> Option<(f64, f64)> { Some((int(&v["x_nm"])? / 1e6, int(&v["y_nm"])? / 1e6)) }
/// `BL_F_Cu` → `F.Cu`, `BL_In1_Cu` → `In1.Cu`.
pub fn layer_name(l: &str) -> String { l.strip_prefix("BL_").unwrap_or(l).replacen('_', ".", 1) }
/// An item's words and place, as kicad-cli's report has them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item { pub description: String, pub at: Option<(f64, f64)> }
pub type Detail = HashMap<String, Item>;

/// The items the runtime described itself (a marker's `items_detail`: {id, description, at_nm}).
pub fn detail_of_markers(markers: &[Value]) -> Detail {
    let mut d = Detail::new();
    for m in markers { for i in m["items_detail"].as_array().into_iter().flatten() {
        let id = kiid(&i["id"]); if id.is_empty() { continue }
        d.insert(id, Item { description: i["description"].as_str().unwrap_or("").to_string(), at: mm(&i["at_nm"]) });
    } }
    d
}
/// Whether every item of the markers is described already.
pub fn described(markers: &[Value], d: &Detail) -> bool { markers.iter().flat_map(|m| m["items"].as_array().into_iter().flatten()).all(|i| d.contains_key(&kiid(i))) }
/// The KIIDs the markers name and `d` does not describe.
pub fn missing(markers: &[Value], d: &Detail) -> Vec<String> {
    let mut v: Vec<String> = markers.iter().flat_map(|m| m["items"].as_array().cloned().unwrap_or_default()).map(|i| kiid(&i)).filter(|i| !i.is_empty() && !d.contains_key(i)).collect();
    v.sort(); v.dedup(); v
}
fn net(it: &Value) -> String { let n = it["net"]["name"].as_str().unwrap_or(""); if n.is_empty() { "<no net>".into() } else { n.into() } }
fn ty(it: &Value) -> &str { it["@type"].as_str().or(it["type_url"].as_str()).unwrap_or("").rsplit('.').next().unwrap_or("") }
/// Board items (`GetItemsById`'s JSON) described in kicad-cli's words, so the report's reader finds the same net, layer, reference and
/// pin: "Track [GND] on F.Cu", "Via [GND] on F.Cu - B.Cu", "Pad 1 [VBUS] of J5 on F.Cu", "Footprint U3", "Zone [GND] on B.Cu".
/// `refs`: a footprint's KIID → its reference (a pad names its footprint by `parent`).
pub fn describe(items: &[Value], refs: &HashMap<String, String>) -> Detail {
    let mut d = Detail::new();
    for it in items {
        let id = kiid(&it["id"]); if id.is_empty() { continue }
        let layers = |v: &Value| v.as_array().map(|a| a.iter().filter_map(|l| l.as_str()).map(layer_name).collect::<Vec<_>>()).unwrap_or_default();
        let (description, at) = match ty(it) {
            "Track" => (format!("Track [{}] on {}", net(it), layer_name(it["layer"].as_str().unwrap_or(""))), mm(&it["start"])),
            "Arc" => (format!("Track (arc) [{}] on {}", net(it), layer_name(it["layer"].as_str().unwrap_or(""))), mm(&it["start"])),
            "Via" => { let l = layers(&it["pad_stack"]["layers"]); (format!("Via [{}] on {}", net(it), match (l.first(), l.last()) { (Some(a), Some(b)) if a != b => format!("{a} - {b}"), (Some(a), _) => a.clone(), _ => String::new() }), mm(&it["position"])) }
            "Pad" => {
                let r = refs.get(&kiid(&it["parent"])).cloned().unwrap_or_default();
                let l = layers(&it["pad_stack"]["layers"]).into_iter().next().unwrap_or_default();
                let of = if r.is_empty() { String::new() } else { format!(" of {r}") };
                (format!("Pad {} [{}]{of} on {l}", it["number"].as_str().unwrap_or(""), net(it)), None) // a pad's place is its footprint's frame
            }
            "FootprintInstance" => (format!("Footprint {}", reference(it)), mm(&it["position"])),
            "Zone" => (format!("Zone [{}] on {}", it["copper_settings"]["net"]["name"].as_str().filter(|n| !n.is_empty()).unwrap_or("<no net>"), layers(&it["layers"]).join(", ")), None),
            _ => continue,
        };
        d.insert(id, Item { description, at });
    }
    d
}
/// A footprint's reference (its field's text).
pub fn reference(fp: &Value) -> String { fp["reference_field"]["text"]["text"]["text"].as_str().or(fp["reference_field"]["text"]["text"].as_str()).unwrap_or("").to_string() }
/// The footprints the pads among `items` sit on (their `parent`s).
pub fn pad_parents(items: &[Value]) -> Vec<String> { let mut v: Vec<String> = items.iter().filter(|i| ty(i) == "Pad").map(|i| kiid(&i["parent"])).filter(|p| !p.is_empty()).collect(); v.sort(); v.dedup(); v }

/// A project's sheets: their KIID path ("/<uuid>/", as a marker names it; the root's own uuid in front too) → their names' path
/// ("/Debugger/", as kicad-cli's ERC report names it). `read`: a schematic file's text by path.
pub fn sheets(root: &str, read: &dyn Fn(&str) -> Option<String>) -> HashMap<String, String> {
    let mut m = HashMap::new(); m.insert("/".to_string(), "/".to_string());
    let Some(text) = read(root) else { return m };
    let x = super::sexpr::parse(text.as_bytes());
    let root_uuid = x.attr("uuid").to_string();
    if !root_uuid.is_empty() { m.insert(format!("/{root_uuid}/"), "/".into()); }
    fn walk(file: &str, text: &str, kiids: &str, names: &str, read: &dyn Fn(&str) -> Option<String>, root_uuid: &str, m: &mut HashMap<String, String>, depth: u32) {
        if depth > 8 { return }
        let x = super::sexpr::parse(text.as_bytes());
        for s in x.all("sheet") {
            let (u, name, f) = (s.attr("uuid"), super::schematic::sheet_prop(s, "Sheetname", "Sheet name"), super::schematic::sheet_prop(s, "Sheetfile", "Sheet file"));
            if u.is_empty() { continue }
            let (k, n) = (format!("{kiids}{u}/"), format!("{names}{name}/"));
            m.insert(k.clone(), n.clone());
            if !root_uuid.is_empty() { m.insert(format!("/{root_uuid}{k}"), n.clone()); }
            let path = if crate::plat::fs::is_abs(f) { f.to_string() } else { format!("{}{f}", file.rfind('/').map(|i| &file[..=i]).unwrap_or("")) };
            if let Some(t) = read(&path) { walk(&path, &t, &k, &n, read, root_uuid, m, depth + 1) }
        }
    }
    walk(root, &text, "/", "/", read, &root_uuid, &mut m, 0);
    m
}
/// The other way: a names' path → its KIID path (SelectItems' `sheet_path`), the shortest (without the root's uuid).
pub fn sheet_kiid(sheets: &HashMap<String, String>, name: &str) -> String {
    sheets.iter().filter(|(_, n)| *n == name).map(|(k, _)| k.clone()).min_by_key(|k| k.len()).unwrap_or_else(|| "/".into())
}

/// The editor's markers as kicad-cli's report (`kind` "drc" or "erc"): what `findings::drc`/`erc` read. Host markers are left out;
/// an item the detail does not describe keeps its KIID (its row's id does not need the words, but for unconnected items: by net).
pub fn report(kind: &str, markers: &[Value], d: &Detail, sheets: &HashMap<String, String>) -> Value {
    let mut by_list: HashMap<&str, Vec<Value>> = HashMap::new();
    let mut by_sheet: Vec<(String, Vec<Value>)> = vec![];
    for m in markers {
        if !m["host"].is_null() && (m["host"]["set"].as_str().is_some_and(|s| !s.is_empty())) { continue }
        let at = mm(&m["at_nm"]);
        // kicad-cli writes an exclusion without its items (its row keyed by its words), so the editor's must be too, or its id would move
        let excluded = m["excluded"].as_bool().unwrap_or(false) || m["severity"] == "exclusion";
        let items: Vec<Value> = m["items"].as_array().into_iter().flatten().filter(|_| !excluded).map(|i| {
            let id = kiid(i); let it = d.get(&id).cloned().unwrap_or_default();
            let mut o = json!({"uuid": id, "description": it.description});
            if let Some((x, y)) = it.at.or(at) { o["pos"] = json!({"x": x, "y": y}) }
            o
        }).collect();
        let t = m["type"].as_str().unwrap_or("");
        let mut v = json!({"type": t, "severity": m["severity"].as_str().unwrap_or("warning"), "description": m["message"].as_str().unwrap_or(""), "items": items});
        if m["excluded"].as_bool().unwrap_or(false) { v["excluded"] = json!(true) }
        if kind == "drc" {
            let list = match m["group"].as_str().unwrap_or("") { "unconnected" => "unconnected_items", "parity" => "schematic_parity", "violation" => "violations",
                _ => if t == "unconnected_items" { "unconnected_items" } else if parity(t) { "schematic_parity" } else { "violations" } };
            by_list.entry(list).or_default().push(v);
        } else {
            let name = sheet_name(sheets, m["sheet_path"].as_str().unwrap_or(""));
            seen_sheet(&name, m["sheet_path"].as_str().unwrap_or(""));
            match by_sheet.iter_mut().find(|(n, _)| *n == name) { Some((_, vs)) => vs.push(v), None => by_sheet.push((name, vec![v])) }
        }
    }
    if kind == "drc" {
        let get = |k: &str| Value::Array(by_list.get(k).cloned().unwrap_or_default());
        json!({"coordinate_units": "mm", "violations": get("violations"), "unconnected_items": get("unconnected_items"), "schematic_parity": get("schematic_parity")})
    } else {
        json!({"coordinate_units": "mm", "sheets": by_sheet.into_iter().map(|(n, vs)| json!({"path": n, "violations": vs})).collect::<Vec<_>>()})
    }
}
/// A marker's sheet (its KIID path) by name. KiCad may put KIIDs in front that no sheet file names (the project's own root, the root
/// schematic's uuid): they are dropped until the rest is a known chain of sheets; a path that ends in no known sheet is the root.
pub fn sheet_name(sheets: &HashMap<String, String>, kiid_path: &str) -> String {
    let parts: Vec<&str> = kiid_path.split('/').filter(|p| !p.is_empty()).collect();
    for i in 0..parts.len() { if let Some(n) = sheets.get(&format!("/{}/", parts[i..].join("/"))) { return n.clone() } }
    "/".into()
}
/// The editor's own KIID path of each sheet name it has given (SelectItems takes the path as KiCad writes it).
static SEEN_SHEETS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
/// The editor's own KIID path for a sheet name, when a run gave it (no need to read the schematic files then).
pub fn seen_path(name: &str) -> Option<String> { SEEN_SHEETS.get_or_init(Default::default).lock().unwrap().get(name).cloned() }
fn seen_sheet(name: &str, kiid_path: &str) { if !kiid_path.is_empty() { SEEN_SHEETS.get_or_init(Default::default).lock().unwrap().insert(name.to_string(), kiid_path.to_string()); } }
/// A KIID path as the sheet map keys it: "/a/b/" (KiCad writes "/a/b").
fn slashed(p: &str) -> String { let t = p.trim_matches('/'); if t.is_empty() { "/".into() } else { format!("/{t}/") } }
/// The schematic parity checks' types (kicad-cli lists them apart).
fn parity(t: &str) -> bool { matches!(t, "missing_footprint" | "extra_footprint" | "duplicate_footprints" | "net_conflict" | "footprint_symbol_mismatch" | "footprint_filters_mismatch" | "schematic_parity") }

/// A KiCad marker click (ET_MARKER_CLICKED's `drc_marker`) as the id its row has, among `rows` (the source's findings).
pub fn row_of(kind: &str, marker: &Value, sheets: &HashMap<String, String>, rows: &[Finding]) -> Option<String> {
    let rep = report(kind, std::slice::from_ref(marker), &Detail::new(), sheets);
    let f = if kind == "drc" { fd::drc(&rep.to_string()) } else { fd::erc(&rep.to_string()) }?.into_iter().next()?;
    let base = fd::base_id(&f);
    rows.iter().find(|r| r.id == base).or_else(|| rows.iter().find(|r| r.id.starts_with(&base))).map(|r| r.id.clone())
}

/// An exclusion's reason (a marker's `comment`) on its row's detail.
pub fn reasons(kind: &str, markers: &[Value], sheets: &HashMap<String, String>, fs: &mut [Finding]) {
    for m in markers.iter().filter(|m| m["comment"].as_str().is_some_and(|c| !c.trim().is_empty())) {
        let Some(id) = row_of(kind, m, sheets, fs) else { continue };
        if let Some(f) = fs.iter_mut().find(|f| f.id == id) { let c = m["comment"].as_str().unwrap_or("").trim(); f.detail = if f.detail.is_empty() { format!("Excluded: {c}") } else { format!("{} · Excluded: {c}", f.detail) } }
    }
}
/// SelectItems for a finding in the open editor: its exact items, the box round their places (1 mm at least each way), its sheet.
pub fn select_items(f: &Finding, sheets: &HashMap<String, String>) -> Option<Value> {
    let doc = f.anchors.first().map(|a| a.doc.as_str()).unwrap_or("");
    if doc != "pcb" && doc != "sch" { return None }
    let ids: Vec<Value> = f.anchors.iter().filter(|a| a.doc == doc && !a.uuid.is_empty()).map(|a| json!({"value": a.uuid})).collect();
    let at: Vec<(f64, f64)> = f.anchors.iter().filter(|a| a.doc == doc && a.at).map(|a| (a.x, a.y)).collect();
    if ids.is_empty() && at.is_empty() { return None }
    let mut m = json!({"kind": if doc == "pcb" { "FK_PCB" } else { "FK_SCH" }, "ids": ids, "zoom": true});
    if !at.is_empty() {
        let (x0, x1) = at.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
        let (y0, y1) = at.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        let nm = |v: f64| (v * 1e6).round() as i64;
        m["zoom_box_nm"] = json!({"position": {"x_nm": nm(x0 - 1.0), "y_nm": nm(y0 - 1.0)}, "size": {"x_nm": nm(x1 - x0 + 2.0), "y_nm": nm(y1 - y0 + 2.0)}});
        if ids.is_empty() { m["at_nm"] = json!({"x_nm": nm((x0 + x1) / 2.0), "y_nm": nm((y0 + y1) / 2.0)}) }
    }
    if doc == "sch" { if let Some(s) = f.anchors.first().map(|a| a.sheet.as_str()).filter(|s| !s.is_empty()) {
        let seen = SEEN_SHEETS.get_or_init(Default::default).lock().unwrap().get(s).cloned();
        m["sheet_path"] = json!(seen.unwrap_or_else(|| sheet_kiid(sheets, s)))
    } }
    Some(m)
}

// The run --------------------------------------------------------------------------------------------------------------------------

/// A run that waits for its ET_CHECKS_DONE (by progress id), and one done before its waiter came.
#[derive(Default)]
struct Waits { waiting: HashMap<String, tokio::sync::oneshot::Sender<Value>>, done: HashMap<String, Value>, fraction: HashMap<String, (String, f64)>, seen: HashMap<String, std::time::Instant> }
static WAITS: OnceLock<Mutex<Waits>> = OnceLock::new();
fn waits() -> std::sync::MutexGuard<'static, Waits> { WAITS.get_or_init(Default::default).lock().unwrap() }
fn pid_of(v: &Value) -> String { match v { Value::String(s) => s.clone(), Value::Number(n) => n.to_string(), _ => String::new() } }

/// From the editor's events (embed.rs): a check's progress and its end. Answers whether the event was one of ours.
pub fn on_event(ev: &Value) -> bool {
    let t = &ev["type"];
    if t == "ET_CHECKS_DONE" || t.as_i64() == Some(27) {
        let id = pid_of(&ev["checks"]["progress_id"]);
        let mut w = waits();
        w.fraction.remove(&id);
        match w.waiting.remove(&id) { Some(tx) => { let _ = tx.send(ev["checks"].clone()); } None => { w.done.insert(id, ev["checks"].clone()); if w.done.len() > 32 { let k = w.done.keys().next().cloned().unwrap_or_default(); w.done.remove(&k); } } }
        return true
    }
    if t == "ET_PROGRESS" || t.as_i64() == Some(24) {
        let p = &ev["progress"]; let id = pid_of(&p["id"]);
        let mut w = waits();
        if w.waiting.contains_key(&id) { w.seen.insert(id.clone(), std::time::Instant::now()); }
        if w.waiting.contains_key(&id) && !p["done"].as_bool().unwrap_or(false) { let f = p["fraction"].as_f64().unwrap_or(-1.0); w.fraction.insert(id, (p["title"].as_str().unwrap_or("").to_string(), f)); }
    }
    false
}
/// How far the running check of `id` is (0..1, -1 unknown), for the panel's bar.
pub fn fraction(id: &str) -> Option<f64> { waits().fraction.get(id).map(|x| x.1) }

/// How a run in the editor ended: its findings and whether they are of unsaved edits; cancelled; failed; or run it with kicad-cli.
pub enum Ran { Found(Vec<Finding>, bool), Cancelled, Failed(String), Cli(String) }

/// Run `kind` ("drc" or "erc") in the editor of `route`; `pcb`/`sch`: the project's files (the sheets' names); `on_start`: the
/// progress id, once the editor took the run.
pub async fn run(route: &GuiRoute, kind: &str, sch: &str, cancel: &tokio::sync::Notify, on_start: impl FnOnce(&str)) -> Ran {
    let fk = if kind == "drc" { "FK_PCB" } else { "FK_SCH" };
    // DRC's schematic parity opens KiCad's schematic editor, which asks to be shown: not the person's switch (the pane stays where it is)
    embed::hold_shows(&route.pro, Duration::from_secs(120));
    struct Release(String);
    impl Drop for Release { fn drop(&mut self) { embed::release_shows(&self.0); } }
    let _release = Release(route.pro.clone());
    let o = embed::gui_call_as(route, "kiapi.backplane.embed.RunChecks", json!({"kind": fk, "parity": kind == "drc"}), embed::CHECKS_CLIENT).await;
    if !o["ok"].as_bool().unwrap_or(false) {
        let why = o["error"].as_str().or(o["why"].as_str()).unwrap_or("no answer").to_string();
        return Ran::Cli(format!("KiCad did not run it ({why})"))
    }
    let id = pid_of(&o["message"]["progress_id"]);
    if id.is_empty() || id == "0" { return Ran::Cli("KiCad gave the run no id".into()) }
    let rx = { let mut w = waits(); match w.done.remove(&id) { Some(c) => { let (tx, rx) = tokio::sync::oneshot::channel(); let _ = tx.send(c); rx } None => { let (tx, rx) = tokio::sync::oneshot::channel(); w.waiting.insert(id.clone(), tx); rx } } };
    on_start(&id);
    struct Forget(String);
    impl Drop for Forget { fn drop(&mut self) { let mut w = waits(); w.waiting.remove(&self.0); w.fraction.remove(&self.0); w.seen.remove(&self.0); } }
    let _forget = Forget(id.clone());
    tokio::pin!(rx);
    let t0 = std::time::Instant::now();
    waits().seen.insert(id.clone(), t0);
    // every way out but its end cancels it in KiCad: while it runs, every other client's request is AS_BUSY
    let stop = |why: String| { let id = id.clone(); async move {
        let _ = embed::gui_call_as(route, "kiapi.backplane.embed.CancelProgress", json!({"id": id}), embed::CHECKS_CLIENT).await;
        why
    } };
    let mut tick = tokio::time::interval(Duration::from_millis((STALL_MS / 4).max(50)));
    let checks = loop {
        tokio::select! {
            c = &mut rx => match c { Ok(c) => break c, Err(_) => { let _ = stop(String::new()).await; return Ran::Failed("KiCad's check ended without a result".into()) } },
            _ = cancel.notified() => {
                let _ = stop(String::new()).await;
                let _ = tokio::time::timeout(Duration::from_secs(10), &mut rx).await;
                return Ran::Cancelled
            }
            _ = tick.tick() => {
                let last = waits().seen.get(&id).copied().unwrap_or(t0);
                let stalled = last.elapsed() >= Duration::from_millis(STALL_MS);
                let long = t0.elapsed() >= Duration::from_secs(RUN_SECS);
                if stalled || long {
                    let why = if long { format!("{} in KiCad took longer than {RUN_SECS} s", kind.to_uppercase()) } else { format!("{} in KiCad showed no progress for {} s (a message box in KiCad?)", kind.to_uppercase(), STALL_MS / 1000) };
                    let why = stop(why).await;
                    let _ = tokio::time::timeout(Duration::from_secs(3), &mut rx).await;
                    STUCK.get_or_init(Default::default).lock().unwrap().insert(route.sock.clone(), crate::hub::now_ms() + STUCK_OFF_MS);
                    eprintln!("checks: {why}: cancelled; kicad-cli for the next {} min", STUCK_OFF_MS / 60_000);
                    return Ran::Cli(why)
                }
            }
        }
    };
    if checks["cancelled"].as_bool().unwrap_or(false) { return Ran::Cancelled }
    if let Some(e) = checks["error"].as_str().filter(|e| !e.is_empty()) { return Ran::Failed(format!("KiCad could not run {}: {e}", kind.to_uppercase())) }
    let g = embed::gui_call_as(route, "kiapi.backplane.embed.GetMarkers", json!({"kind": fk}), embed::CHECKS_CLIENT).await;
    if !g["ok"].as_bool().unwrap_or(false) { return Ran::Cli(format!("KiCad gave no markers ({})", g["error"].as_str().unwrap_or("no answer"))) }
    let markers: Vec<Value> = g["message"]["markers"].as_array().cloned().unwrap_or_default();
    let mut d = detail_of_markers(&markers);
    if kind == "drc" && !described(&markers, &d) { d.extend(look_up(route, &missing(&markers, &d)).await) }
    let sch = sch.to_string();
    let sheets = if kind == "erc" { tokio::task::spawn_blocking(move || sheets(&sch, &|p| std::fs::read_to_string(p).ok())).await.unwrap_or_default() } else { HashMap::new() };
    let rep = report(kind, &markers, &d, &sheets).to_string();
    match if kind == "drc" { fd::drc(&rep) } else { fd::erc(&rep) } {
        Some(mut fs) => { reasons(kind, &markers, &sheets, &mut fs); Ran::Found(fs, checks["document_modified"].as_bool().unwrap_or(false)) }
        None => Ran::Failed("KiCad's markers could not be read".into()),
    }
}
/// The board items of `ids` described (GetItemsById; the pads' footprints for their references). Nothing when KiCad would not say.
async fn look_up(route: &GuiRoute, ids: &[String]) -> Detail {
    if ids.is_empty() { return Detail::new() }
    let header = json!({"document": {"type": "DOCTYPE_PCB"}});
    let get = |ids: Vec<String>| { let h = header.clone(); async move {
        let o = embed::gui_call_as(route, "kiapi.common.commands.GetItemsById", json!({"header": h, "items": ids.iter().map(|i| json!({"value": i})).collect::<Vec<_>>()}), embed::CHECKS_CLIENT).await;
        o["message"]["items"].as_array().cloned().unwrap_or_default()
    } };
    let items = get(ids.to_vec()).await;
    let parents = pad_parents(&items);
    let refs: HashMap<String, String> = if parents.is_empty() { HashMap::new() } else { get(parents).await.iter().map(|f| (kiid(&f["id"]), reference(f))).collect() };
    describe(&items, &refs)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// kicad-cli's DRC report of a board, and the same violations as the editor's markers (KIIDs only, nanometres, the editor's words).
    const CLI_DRC: &str = r#"{"coordinate_units":"mm","violations":[
      {"description":"Clearance violation (netclass 'Ground' clearance 0.1000 mm; actual 0.0250 mm)","items":[{"description":"Via [GND] on F.Cu - B.Cu","pos":{"x":103.5,"y":131.5},"uuid":"v1"},{"description":"Pad 1 [/SHPHLD] of SW3 on F.Cu","pos":{"x":103.725,"y":132.175},"uuid":"p1"}],"severity":"error","type":"clearance"},
      {"description":"Track has unconnected end","items":[{"description":"Track [VDD_SENS] on F.Cu, length 1.4500 mm","pos":{"x":1.0,"y":2.0},"uuid":"t1"}],"severity":"warning","type":"track_dangling"},
      {"description":"Silkscreen clearance","items":[],"severity":"warning","type":"silk_overlap","excluded":true}],
     "unconnected_items":[{"description":"Missing connection between items","items":[{"description":"PTH pad SH [GND] of J5","uuid":"j5sh"},{"description":"Zone [GND] on B.Cu, priority 0","uuid":"z"}],"severity":"error","type":"unconnected_items"}],
     "schematic_parity":[{"description":"Missing footprint","items":[{"description":"Symbol R9","uuid":"r9"}],"severity":"warning","type":"missing_footprint"}]}"#;
    fn markers() -> Vec<Value> { serde_json::from_str(r#"[
      {"type":"clearance","severity":"error","message":"Clearance violation (netclass 'Ground' clearance 0.1000 mm; actual 0.0250 mm)","at_nm":{"x_nm":"103500000","y_nm":"131500000"},"items":[{"value":"v1"},{"value":"p1"}],"id":{"value":"m1"},"layer":"F.Cu"},
      {"type":"track_dangling","severity":"warning","message":"Track has unconnected end","at_nm":{"x_nm":"1000000","y_nm":"2000000"},"items":[{"value":"t1"}],"id":{"value":"m2"}},
      {"type":"silk_overlap","severity":"exclusion","excluded":true,"message":"Silkscreen clearance","at_nm":{"x_nm":"5000000","y_nm":"5000000"},"items":[{"value":"c76"}],"id":{"value":"m3"}},
      {"type":"unconnected_items","severity":"error","message":"Missing connection between items","at_nm":{"x_nm":"0","y_nm":"0"},"items":[{"value":"j5sh"},{"value":"z"}],"id":{"value":"m4"}},
      {"type":"missing_footprint","severity":"warning","message":"Missing footprint","items":[{"value":"r9"}],"id":{"value":"m5"}},
      {"type":"","severity":"warning","message":"review","host":{"set":"review","id":"x"},"items":[]}]"#).unwrap() }
    /// The editor's items (GetItemsById's JSON) for the fallback.
    fn items() -> Vec<Value> { serde_json::from_str(r#"[
      {"@type":"type.googleapis.com/kiapi.board.types.Via","id":{"value":"v1"},"position":{"x_nm":"103500000","y_nm":"131500000"},"pad_stack":{"layers":["BL_F_Cu","BL_B_Cu"]},"net":{"name":"GND"}},
      {"@type":"type.googleapis.com/kiapi.board.types.Pad","id":{"value":"p1"},"number":"1","net":{"name":"/SHPHLD"},"pad_stack":{"layers":["BL_F_Cu"]},"parent":{"value":"sw3"}},
      {"@type":"type.googleapis.com/kiapi.board.types.Pad","id":{"value":"j5sh"},"number":"SH","net":{"name":"GND"},"pad_stack":{"layers":["BL_F_Cu","BL_B_Cu"]},"parent":{"value":"j5"}},
      {"@type":"type.googleapis.com/kiapi.board.types.Track","id":{"value":"t1"},"start":{"x_nm":"1000000","y_nm":"2000000"},"layer":"BL_F_Cu","net":{"name":"VDD_SENS"}},
      {"@type":"type.googleapis.com/kiapi.board.types.FootprintInstance","id":{"value":"c76"},"reference_field":{"text":{"text":{"text":"C76"}}}},
      {"@type":"type.googleapis.com/kiapi.board.types.Zone","id":{"value":"z"},"layers":["BL_B_Cu"],"copper_settings":{"net":{"name":"GND"}}}]"#).unwrap() }
    fn refs() -> HashMap<String, String> { [("sw3", "SW3"), ("j5", "J5")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() }
    fn key(fs: &[Finding]) -> Vec<(String, String, String)> { let mut v: Vec<_> = fs.iter().map(|f| (f.id.clone(), f.severity.clone(), f.rule.clone())).collect(); v.sort(); v }

    /// One board through both paths: the CLI's report, and the editor's markers (with the runtime's own detail; with the GetItems
    /// fallback; with nothing but KIIDs): every row keeps its id, severity and rule; the names the panel shows and probes by stay too.
    #[test] fn an_editor_run_gives_the_rows_a_cli_run_gives() {
        let cli = fd::drc(CLI_DRC).unwrap();
        let ms = markers();
        // the fallback: the items looked up and described
        let d = describe(&items(), &refs());
        assert_eq!(d["p1"].description, "Pad 1 [/SHPHLD] of SW3 on F.Cu"); assert_eq!(d["v1"].description, "Via [GND] on F.Cu - B.Cu"); assert_eq!(d["z"].description, "Zone [GND] on B.Cu");
        let ed = fd::drc(&report("drc", &ms, &d, &HashMap::new()).to_string()).unwrap();
        assert_eq!(key(&ed), key(&cli), "the same rows");
        let pad = |fs: &[Finding]| fs.iter().find(|f| f.rule == "clearance").unwrap().anchors.iter().find(|a| a.uuid == "p1").map(|a| (a.reference.clone(), a.pin.clone(), a.net.clone(), a.layer.clone())).unwrap();
        assert_eq!(pad(&ed), pad(&cli), "the pad's reference, pin, net and layer");
        assert_eq!(fd::select_params(ed.iter().find(|f| f.rule == "unconnected_items").unwrap())["nets"], fd::select_params(cli.iter().find(|f| f.rule == "unconnected_items").unwrap())["nets"]);
        // the runtime's own detail (items_detail, in its words): the same
        let mut with = ms.clone();
        let words: HashMap<&str, (&str, (i64, i64))> = [("v1", ("Via [GND] on F.Cu - B.Cu", (103500000, 131500000))), ("p1", ("Pad 1 [/SHPHLD] of SW3 on F.Cu", (103725000, 132175000))), ("t1", ("Track [VDD_SENS] on F.Cu, length 1.4500 mm", (1000000, 2000000))),
            ("c76", ("Footprint C76", (5000000, 5000000))), ("j5sh", ("PTH pad SH [GND] of J5", (0, 0))), ("z", ("Zone [GND] on B.Cu, priority 0", (0, 0))), ("r9", ("Symbol R9", (0, 0)))].into_iter().collect();
        for m in with.iter_mut() { let det: Vec<Value> = m["items"].as_array().cloned().unwrap_or_default().iter().map(|i| { let (w, (x, y)) = words[kiid(i).as_str()]; json!({"id": i, "description": w, "at_nm": {"x_nm": x, "y_nm": y}}) }).collect(); m["items_detail"] = json!(det); }
        let d2 = detail_of_markers(&with); assert!(described(&with, &d2)); assert!(missing(&with, &d2).is_empty());
        let ed2 = fd::drc(&report("drc", &with, &d2, &HashMap::new()).to_string()).unwrap();
        assert_eq!(key(&ed2), key(&cli)); assert_eq!(pad(&ed2), pad(&cli));
        let det = |fs: &[Finding]| fs.iter().map(|f| f.detail.clone()).collect::<Vec<_>>();
        assert_eq!(det(&ed2), det(&cli), "the runtime's words are the CLI's");
        // KIIDs only: the ids still match, but for the unconnected row, which kicad-cli keys by net (so the fallback is needed there)
        let bare = fd::drc(&report("drc", &ms, &Detail::new(), &HashMap::new()).to_string()).unwrap();
        let no_net = |v: Vec<(String, String, String)>| v.into_iter().filter(|r| r.2 != "unconnected_items").collect::<Vec<_>>();
        assert_eq!(no_net(key(&bare)), no_net(key(&cli)));
        assert_eq!(missing(&ms, &Detail::new()), ["c76", "j5sh", "p1", "r9", "t1", "v1", "z"]);
        // a host marker is never a finding; the place comes in millimetres
        assert_eq!(ed.len(), 5);
        let t = ed.iter().find(|f| f.rule == "track_dangling").unwrap(); assert_eq!((t.anchors[0].x, t.anchors[0].y, t.anchors[0].at), (1.0, 2.0, true));
    }
    /// batch 3: each marker says its list (`group`), whatever its type; an exclusion's reason is on its row.
    #[test] fn a_markers_group_picks_its_list_and_an_exclusions_reason_is_on_its_row() {
        let mut ms = markers();
        ms[1]["group"] = json!("parity"); ms[3]["group"] = json!("unconnected"); ms[2]["comment"] = json!("silk under the shield can, fine");
        let rep = report("drc", &ms, &Detail::new(), &HashMap::new());
        assert_eq!(rep["schematic_parity"].as_array().unwrap().iter().map(|v| v["type"].as_str().unwrap()).collect::<Vec<_>>(), ["track_dangling", "missing_footprint"]);
        assert_eq!(rep["unconnected_items"].as_array().unwrap().len(), 1);
        let mut fs = fd::drc(&rep.to_string()).unwrap();
        reasons("drc", &ms, &HashMap::new(), &mut fs);
        let f = fs.iter().find(|f| f.rule == "silk_overlap").unwrap();
        assert_eq!(f.severity, "excluded"); assert!(f.detail.ends_with("Excluded: silk under the shield can, fine"), "{}", f.detail);
    }
    /// ERC: a marker names its sheet by KIID path (with or without the root's uuid), the CLI by names; the rows are the same.
    #[test] fn an_editor_erc_names_its_sheets_as_the_cli_does() {
        let files: HashMap<&str, &str> = [("/p/root.kicad_sch", r#"(kicad_sch (uuid "R0") (sheet (uuid "S1") (property "Sheetname" "Debugger") (property "Sheetfile" "dbg.kicad_sch")))"#),
            ("/p/dbg.kicad_sch", r#"(kicad_sch (uuid "D0") (sheet (uuid "S2") (property "Sheetname" "Probe") (property "Sheetfile" "sub/probe.kicad_sch")))"#), ("/p/sub/probe.kicad_sch", "(kicad_sch (uuid \"P0\"))")].into_iter().collect();
        let sh = sheets("/p/root.kicad_sch", &|p| files.get(p).map(|s| s.to_string()));
        assert_eq!((sh["/"].as_str(), sh["/S1/"].as_str(), sh["/R0/S1/"].as_str(), sh["/S1/S2/"].as_str(), sh["/R0/S1/S2/"].as_str()), ("/", "/Debugger/", "/Debugger/", "/Debugger/Probe/", "/Debugger/Probe/"));
        assert_eq!(sheet_kiid(&sh, "/Debugger/Probe/"), "/S1/S2/");
        // Hubble_DK on bp.13: the project's own root KIID in front, no trailing slash
        assert_eq!((sheet_name(&sh, "/V9/S1/S2").as_str(), sheet_name(&sh, "/V9/R0/S1").as_str(), sheet_name(&sh, "/V9").as_str(), sheet_name(&sh, "").as_str()), ("/Debugger/Probe/", "/Debugger/", "/", "/"));
        let cli = fd::erc(r#"{"coordinate_units":"mm","sheets":[{"path":"/","violations":[{"description":"Label not connected","items":[{"description":"Label 'SWD.CLK'","pos":{"x":63.5,"y":114.3},"uuid":"l1"}],"severity":"error","type":"label_dangling"}]},
            {"path":"/Debugger/","violations":[{"description":"Input pin not driven by any Output pins","items":[{"description":"Symbol U5 Pin F2 [~{RESET}, Input, Line]","pos":{"x":129.54,"y":137.16},"uuid":"p1"}],"severity":"error","type":"pin_not_driven"}]}]}"#).unwrap();
        let ms: Vec<Value> = serde_json::from_str(r#"[{"type":"label_dangling","severity":"error","message":"Label not connected","sheet_path":"/R0","at_nm":{"x_nm":63500000,"y_nm":114300000},"items":[{"value":"l1"}]},
            {"type":"pin_not_driven","severity":"error","message":"Input pin not driven by any Output pins","sheet_path":"/S1","at_nm":{"x_nm":129540000,"y_nm":137160000},"items":[{"value":"p1"}]}]"#).unwrap();
        let ed = fd::erc(&report("erc", &ms, &Detail::new(), &sh).to_string()).unwrap();
        assert_eq!(key(&ed), key(&cli));
        // a click on KiCad's own marker finds its row
        assert_eq!(row_of("erc", &ms[1], &sh, &cli).as_deref(), Some(cli.iter().find(|f| f.rule == "pin_not_driven").unwrap().id.as_str()));
        // the cross-probe goes to the item on its sheet, the box round its place
        let s = select_items(cli.iter().find(|f| f.rule == "pin_not_driven").unwrap(), &sh).unwrap();
        assert_eq!((s["kind"].as_str(), s["ids"][0]["value"].as_str(), s["sheet_path"].as_str()), (Some("FK_SCH"), Some("p1"), Some("/S1")), "the editor's own path for the sheet");
        assert_eq!(s["zoom_box_nm"]["position"]["x_nm"], 128540000); assert_eq!(s["zoom_box_nm"]["size"]["x_nm"], 2000000);
    }
    /// Collar's ERC on bp.13: three exclusions with the same words, which kicad-cli writes without items (rows keyed by words: x, x~2,
    /// x~3), on the root sheet named by its KIID without a trailing slash; the editor's markers carry their items. The same rows.
    #[test] fn excluded_markers_are_keyed_by_their_words_as_the_cli_keys_them() {
        let cli = fd::erc(r#"{"coordinate_units":"mm","sheets":[{"path":"/","uuid_path":"/R0","violations":[
            {"description":"Input Power pin not driven by any Output Power pins","items":[],"severity":"error","type":"power_pin_not_driven","excluded":true},
            {"description":"Input Power pin not driven by any Output Power pins","items":[],"severity":"error","type":"power_pin_not_driven","excluded":true},
            {"description":"Input Power pin not driven by any Output Power pins","items":[],"severity":"error","type":"power_pin_not_driven","excluded":true},
            {"description":"Input pin not driven by any Output pins","items":[],"severity":"error","type":"pin_not_driven","excluded":true}]}]}"#).unwrap();
        let ms: Vec<Value> = ["a", "b", "c"].iter().map(|u| json!({"type": "power_pin_not_driven", "severity": "exclusion", "excluded": true, "message": "Input Power pin not driven by any Output Power pins", "sheet_path": "/R0", "items": [{"value": u}], "at_nm": {"x_nm": 1, "y_nm": 2}}))
            .chain([json!({"type": "pin_not_driven", "severity": "exclusion", "excluded": true, "message": "Input pin not driven by any Output pins", "sheet_path": "/R0", "items": [{"value": "d"}]})]).collect();
        let sh = sheets("/p/r.kicad_sch", &|_| Some("(kicad_sch (uuid \"R0\"))".into()));
        let ed = fd::erc(&report("erc", &ms, &Detail::new(), &sh).to_string()).unwrap();
        assert_eq!(key(&ed), key(&cli));
        assert!(cli.iter().any(|f| f.id.ends_with("~3")));
    }
    /// The fork's batch 5: ERC exclusions for sheets not in the loaded schematic come in the report on the root sheet, excluded, with no
    /// items and no place (and `unresolved_sheet_path`). Such rows have steady ids (by their words, numbered) and nothing to probe.
    #[test] fn exclusions_for_sheets_not_loaded_have_ids_and_no_probe() {
        let rep = r#"{"coordinate_units":"mm","sheets":[{"path":"/","uuid_path":"/R0","violations":[
            {"description":"Input pin not driven by any Output pins","items":[],"severity":"error","type":"pin_not_driven","excluded":true,"unresolved_sheet_path":"/gone1"},
            {"description":"Input pin not driven by any Output pins","items":[],"severity":"error","type":"pin_not_driven","excluded":true,"unresolved_sheet_path":"/gone2"}]}]}"#;
        let a = fd::erc(rep).unwrap(); let b = fd::erc(rep).unwrap();
        assert_eq!(key(&a), key(&b), "the same ids run after run");
        assert_eq!(a.iter().map(|f| f.severity.as_str()).collect::<Vec<_>>(), ["excluded", "excluded"]);
        assert_ne!(a[0].id, a[1].id);
        for f in &a { assert!(select_items(f, &HashMap::new()).is_none(), "no items, no place: nothing to select"); let p = fd::select_params(f); assert!(["references", "pins", "nets"].iter().all(|k| p[*k].as_array().unwrap().is_empty())) }
    }
    #[test] fn a_drc_marker_click_finds_its_row_and_a_track_finding_probes_its_exact_items() {
        let cli = fd::drc(CLI_DRC).unwrap();
        let m = &markers()[0];
        let id = row_of("drc", m, &HashMap::new(), &cli).unwrap();
        assert_eq!(cli.iter().find(|f| f.id == id).unwrap().rule, "clearance");
        let s = select_items(cli.iter().find(|f| f.rule == "track_dangling").unwrap(), &HashMap::new()).unwrap();
        assert_eq!((s["kind"].as_str(), s["ids"].clone()), (Some("FK_PCB"), json!([{"value": "t1"}])));
        assert!(s["sheet_path"].is_null() && s["at_nm"].is_null());
    }
    /// The editor's rows of a rule it disagrees on with kicad-cli carry a note when the last CLI run had no such row; the CLI's own rows
    /// of that rule, other rules, and every row when no CLI run is known, carry none; ids are left as they were.
    #[test] fn rows_only_the_editor_reports_say_so() {
        let f = |id: &str, rule: &str| Finding { id: id.into(), source: "drc".into(), severity: "warning".into(), rule: rule.into(), title: "t".into(), detail: "Text 'U3'".into(), anchors: vec![], fix: String::new() };
        let mk = || vec![f("drc:cli", "silk_over_copper"), f("drc:new", "silk_over_copper"), f("drc:c", "clearance")];
        let cli: std::collections::HashSet<String> = ["drc:cli", "drc:other"].iter().map(|s| s.to_string()).collect();
        let mut fs = mk(); mark_editor_only(&mut fs, &cli);
        assert_eq!(fs.iter().map(|f| f.detail.ends_with(EDITOR_ONLY_NOTE)).collect::<Vec<_>>(), [false, true, false]);
        assert_eq!(fs.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["drc:cli", "drc:new", "drc:c"]);
        let mut fs = mk(); mark_editor_only(&mut fs, &Default::default()); assert!(fs.iter().all(|f| f.detail == "Text 'U3'"));
    }
    #[test] fn the_capability_is_the_editors_own() {
        assert!(capable(&json!({"capabilities": ["embed", "embed-checks"], "commands": ["kiapi.backplane.embed.RunChecks", "kiapi.backplane.embed.GetMarkers"]})));
        assert!(capable(&json!({"capabilities": ["embed-checks"]})), "no command list: the capability says it");
        assert!(!capable(&json!({"capabilities": ["embed-checks"], "commands": ["kiapi.backplane.embed.RunChecks"]})), "a listed command missing");
        assert!(!capable(&json!({"capabilities": ["embed", "embed-host-markers"], "commands": ["kiapi.backplane.embed.SetHostMarkers"]})));
        assert_eq!(layer_name("BL_In1_Cu"), "In1.Cu"); assert_eq!(layer_name("BL_Edge_Cuts"), "Edge.Cuts");
    }

    /// A KiCad whose check never ends and never moves (lane D, batch 4: a message box inside the task): the run is cancelled in KiCad
    /// within the stall time, kicad-cli takes it, nothing is left waiting, and KiCad is not asked again for a while.
    #[tokio::test] async fn a_check_that_never_ends_is_cancelled_and_kicad_left_alone() {
        let (route, mut rx) = embed::stand_in_route("/p/stuck.kicad_pro");
        let seen = std::sync::Arc::new(Mutex::new(Vec::<(String, Value)>::new())); let s2 = seen.clone();
        tokio::spawn(async move { while let Some((ty, msg, reply)) = rx.recv().await {
            s2.lock().unwrap().push((ty.clone(), msg));
            let _ = reply.send(if ty.ends_with("RunChecks") { json!({"ok": true, "message": {"progress_id": "901"}}) } else { json!({"ok": true, "message": {}}) });
        } });
        let t0 = std::time::Instant::now();
        let r = tokio::time::timeout(Duration::from_secs(10), run(&route, "drc", "", &tokio::sync::Notify::new(), |_| {})).await.expect("the run ends");
        assert!(matches!(&r, Ran::Cli(w) if w.contains("no progress")), "kicad-cli takes it");
        assert!(t0.elapsed() < Duration::from_secs(5));
        assert!(seen.lock().unwrap().iter().any(|(t, m)| t.ends_with("CancelProgress") && pid_of(&m["id"]) == "901"), "cancelled in KiCad");
        assert!(waits().waiting.get("901").is_none() && waits().seen.get("901").is_none(), "nothing left waiting");
        assert!(stuck_until("/stand-in") > crate::hub::now_ms(), "KiCad is left alone a while");
        STUCK.get_or_init(Default::default).lock().unwrap().remove("/stand-in");
    }
    /// A stand-in editor: RunChecks answers with an id, its progress and ET_CHECKS_DONE come as events, GetMarkers and GetItemsById
    /// answer from the fixtures. The run finds what the CLI finds; a busy editor sends the run to kicad-cli; a cancel cancels it in KiCad.
    #[tokio::test] async fn a_run_in_a_stand_in_editor() {
        let (route, mut rx) = embed::stand_in_route("/p/x.kicad_pro");
        let busy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let b2 = busy.clone();
        let seen = std::sync::Arc::new(Mutex::new(Vec::<(String, Value)>::new())); let s2 = seen.clone();
        tokio::spawn(async move {
            let mut n = 40u64;
            while let Some((ty, msg, reply)) = rx.recv().await {
                s2.lock().unwrap().push((ty.clone(), msg.clone()));
                let short = ty.rsplit('.').next().unwrap_or("").to_string();
                match short.as_str() {
                    "RunChecks" if b2.load(std::sync::atomic::Ordering::SeqCst) => { let _ = reply.send(json!({"ok": false, "error": "AS_BUSY: Board Setup"})); }
                    "RunChecks" => {
                        n += 1; let id = n.to_string();
                        let _ = reply.send(json!({"ok": true, "message": {"progress_id": id}}));
                        let cancel_me = msg["kind"] == "FK_SCH";
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            on_event(&json!({"type": "ET_PROGRESS", "progress": {"id": id, "title": "DRC", "fraction": 0.5}}));
                            if cancel_me { return }
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            on_event(&json!({"type": "ET_CHECKS_DONE", "checks": {"kind": "FK_PCB", "errors": 3, "warnings": 2, "unconnected": 1, "progress_id": id, "document_modified": true}}));
                        });
                    }
                    "CancelProgress" => { let id = pid_of(&msg["id"]); let _ = reply.send(json!({"ok": true, "message": {"cancelling": true}}));
                        on_event(&json!({"type": "ET_CHECKS_DONE", "checks": {"kind": "FK_SCH", "cancelled": true, "progress_id": id}})); }
                    "GetMarkers" => { let _ = reply.send(json!({"ok": true, "message": {"markers": markers()}})); }
                    "GetItemsById" => {
                        let want: Vec<String> = msg["items"].as_array().unwrap().iter().map(kiid).collect();
                        let fps = vec![json!({"@type": "kiapi.board.types.FootprintInstance", "id": {"value": "sw3"}, "reference_field": {"text": {"text": {"text": "SW3"}}}}), json!({"@type": "kiapi.board.types.FootprintInstance", "id": {"value": "j5"}, "reference_field": {"text": {"text": {"text": "J5"}}}})];
                        let all: Vec<Value> = items().into_iter().chain(fps).filter(|i| want.contains(&kiid(&i["id"]))).collect();
                        let _ = reply.send(json!({"ok": true, "message": {"items": all}}));
                    }
                    _ => { let _ = reply.send(json!({"ok": false, "error": "unhandled"})); }
                }
            }
        });
        let cancel = tokio::sync::Notify::new();
        let mut started = String::new();
        let mut mid = None;
        let r = { let fut = run(&route, "drc", "", &cancel, |id| started = id.to_string()); tokio::pin!(fut);
            loop { tokio::select! { r = &mut fut => break r, _ = tokio::time::sleep(Duration::from_millis(5)) => { if mid.is_none() { mid = fraction("41") } } } } };
        let Ran::Found(fs, unsaved) = r else { panic!("the run found nothing") };
        assert_eq!(started, "41"); assert!(unsaved, "the editor's result is of its unsaved edits"); assert_eq!(mid, Some(0.5), "its progress is the bar's");
        assert_eq!(key(&fs), key(&fd::drc(CLI_DRC).unwrap()), "the CLI's rows, through the GetItems fallback");
        let sent: Vec<String> = seen.lock().unwrap().iter().map(|(t, _)| t.rsplit('.').next().unwrap().to_string()).collect();
        assert_eq!(sent, ["RunChecks", "GetMarkers", "GetItemsById", "GetItemsById"], "the items, then the pads' footprints");
        assert_eq!(seen.lock().unwrap()[0].1, json!({"kind": "FK_PCB", "parity": true}));
        assert_eq!(seen.lock().unwrap()[3].1["items"], json!([{"value": "j5"}, {"value": "sw3"}]));
        // busy: kicad-cli runs it
        busy.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(run(&route, "drc", "", &cancel, |_| {}).await, Ran::Cli(w) if w.contains("AS_BUSY")));
        busy.store(false, std::sync::atomic::Ordering::SeqCst);
        // a cancel (a newer revision) cancels it in KiCad
        let c2 = std::sync::Arc::new(tokio::sync::Notify::new()); let c3 = c2.clone();
        tokio::spawn(async move { tokio::time::sleep(Duration::from_millis(60)).await; c3.notify_one() });
        assert!(matches!(run(&route, "erc", "", &c2, |_| {}).await, Ran::Cancelled));
        assert!(seen.lock().unwrap().iter().any(|(t, m)| t.ends_with("CancelProgress") && pid_of(&m["id"]) == "42"));
        assert!(waits().waiting.get("42").is_none() && waits().fraction.get("42").is_none(), "nothing left waiting");
    }
}
