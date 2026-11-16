//! `kicad_edit`: a whole KiCad edit in one tool call (docs/kicad-edit.md). An agent names parts by reference and gives
//! millimetres; the hub opens the document, reads what the edit needs, wraps the changes in one commit and saves. What comes
//! back is a short row per item, never KiCad's whole item (a footprint is 15 to 90 KB of JSON, a 94-part board's footprints
//! 1.8 MB), so a turn that took twenty requests over `kicad_ipc` takes three.
//!
//! The rules here are pure (rows, selectors, the raw messages an op becomes) and tested without a KiCad; [`run`] drives them
//! over any `call(type, message)`, which the MCP layer gives it as the same gated `kicad_ipc` every other call goes through
//! (the document's one-writer lock, the approval refusals, the co-editing actor name, the save's commit to main).
use serde_json::{json, Map, Value};
use std::{future::Future, pin::Pin};

const C: &str = "kiapi.common.commands.";
const B: &str = "kiapi.board.commands.";
const S: &str = "kiapi.schematic.commands.";
const BT: &str = "type.googleapis.com/kiapi.board.types.";
const ST: &str = "type.googleapis.com/kiapi.schematic.types.";
/// Rows one `find` answers with when no `limit` is given.
pub const ROWS: usize = 200;
/// The characters one `find` answers with at most (about six thousand tokens): a board's every pad near a point was fifty thousand, more than a
/// tool result may carry.
pub const FIND_CHARS: usize = 24_000;

pub type Fut<'a> = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>>;
/// One raw IPC command: (fully qualified type, message) -> the response's message.
pub trait Call: Sync {
    fn call<'a>(&'a self, ty: &'a str, msg: Value) -> Fut<'a>;
    /// The saved schematic's sheet files as text, for the read-back of what was set (None: not read back, as with a stand-in).
    fn sheet_texts(&self) -> Option<Vec<String>> { None }
    /// The saved schematic's netlist as {reference: {pin: net}} (kicad-cli's export); Null: none to be had.
    fn nets<'a>(&'a self) -> Fut<'a> { Box::pin(async { Ok(Value::Null) }) }
    /// KiCad's own check of the saved document (kicad-cli's DRC report of a board, ERC of a schematic, as JSON); Null: none to be had.
    fn check<'a>(&'a self) -> Fut<'a> { Box::pin(async { Ok(Value::Null) }) }
}
/// KiCad's task gate (a check or refill is running: AS_BUSY "a task is running: ...") is waited out, asked again every BUSY_STEP for at most
/// BUSY_WAIT, so a check that happens to run does not fail a whole edit. Any other refusal (an open dialog, which the answer names) is the answer.
pub const BUSY_STEP: std::time::Duration = std::time::Duration::from_millis(250);
pub const BUSY_WAIT: std::time::Duration = std::time::Duration::from_secs(60);
pub fn task_busy(e: &str) -> bool { e.contains("AS_BUSY") && e.contains("a task is running") }
struct Patient<'a> { c: &'a dyn Call, step: std::time::Duration, wait: std::time::Duration }
impl Call for Patient<'_> {
    fn call<'b>(&'b self, ty: &'b str, msg: Value) -> Fut<'b> { Box::pin(async move {
        let until = std::time::Instant::now() + self.wait;
        loop { match self.c.call(ty, msg.clone()).await { Err(e) if task_busy(&e) && std::time::Instant::now() < until => tokio::time::sleep(self.step).await, r => return r } }
    }) }
    fn sheet_texts(&self) -> Option<Vec<String>> { self.c.sheet_texts() }
    fn check<'a>(&'a self) -> Fut<'a> { self.c.check() }
    fn nets<'a>(&'a self) -> Fut<'a> { self.c.nets() }
}
/// Whether the ops change anything (a call of finds and raw reads alone does not).
pub fn writes(ops: &Value) -> bool {
    ops.as_array().into_iter().flatten().any(|o| match verb(o) { "find" | "" => false, "ipc" => !super::ipc::is_read(&full_type(o["ipc"]["type"].as_str().unwrap_or(""))), _ => true })
}
/// What a writing call is answered while the person works in this KiCad and the call does not say it has seen that (`person_seen`):
/// the agent asks before working over them, as it does after a `kicad_ipc` read shows `person` (docs/kicad-coedit.md).
pub fn person_first(person: &Value) -> String {
    format!("The person is working in this KiCad right now: {person}. Nothing was changed. If your edit touches those items or their area, ask them first in one short question and carry on with other work; if it does not, send the same call again with person_seen: true.")
}
/// The placed symbol's block in a sheet file (found by its instance's `(reference "R")`) has the property with that text.
pub fn file_prop(texts: &[String], reference: &str, name: &str, value: &str) -> bool {
    let esc = value.replace('\\', "\\\\").replace('"', "\\\"");
    let (want_ref, want) = (format!("(reference \"{reference}\")"), format!("(property \"{name}\" \"{esc}\""));
    texts.iter().any(|t| { let mut from = 0;
        while let Some(k) = t[from..].find(&want_ref) { let at = from + k; from = at + want_ref.len();
            // the symbol block around the instance: from the last top-level "\n\t(symbol" before it to the next one after
            let start = t[..at].rfind("\n\t(symbol").unwrap_or(0); let end = t[at..].find("\n\t(symbol").map_or(t.len(), |e| at + e);
            if t[start..end].contains(&want) { return true } }
        false })
}

/// A check report's findings as one line each: DRC violations, unconnected items and parity, or ERC violations of every sheet.
pub fn findings(report: &Value) -> Vec<String> {
    let line = |kind: &str, v: &Value| { let items: Vec<String> = v["items"].as_array().into_iter().flatten().map(|i| i["description"].as_str().unwrap_or("").chars().take(70).collect()).collect();
        let at = &v["items"][0]["pos"]; let ty = if kind.is_empty() { v["type"].as_str().unwrap_or("") } else { kind };
        format!("{ty}: {}{}", items.join(" / "), if at.is_object() { format!(" @{},{}", at["x"], at["y"]) } else { String::new() }) };
    let mut out = vec![];
    for v in report["violations"].as_array().into_iter().flatten() { out.push(line("", v)) }
    for v in report["unconnected_items"].as_array().into_iter().flatten() { out.push(line("unconnected", v)) }
    for v in report["schematic_parity"].as_array().into_iter().flatten() { out.push(line("", v)) }
    for s in report["sheets"].as_array().into_iter().flatten() { for v in s["violations"].as_array().into_iter().flatten() { out.push(line("", v)) } }
    out
}
/// A finding without what a refill or a save changes in its words (a track's length, the place): what makes two findings the same one.
fn finding_key(f: &str) -> String {
    let mut k = f.split(" @").next().unwrap_or(f).to_string();
    while let Some(i) = k.find(", length ") { let e = k[i..].find(" mm").map_or(k.len(), |e| i + e + 3); k.replace_range(i..e, "") }
    // (the two items of a finding come in either order)
    match k.split_once(": ") { Some((ty, items)) => { let mut v: Vec<&str> = items.split(" / ").collect(); v.sort(); format!("{ty}: {}", v.join(" / ")) } None => k }
}
/// What an edit changed in KiCad's check: the findings that are new (12 at most, then their count), how many went away and how many are left.
/// `touched` (nets and references the edit worked on; empty: everything) keeps a finding on something else out of `new`: a refill or the first
/// save of a file moves findings about on a large board (a pour's island elsewhere, a library warning), and those are `elsewhere`: their count and the first three.
pub fn check_delta(before: &Value, after: &Value, touched: &[String]) -> Value {
    let (mut was, now) = (findings(before).iter().map(|f| finding_key(f)).collect::<Vec<_>>(), findings(after)); let left = now.len(); let (mut new, mut elsewhere) = (vec![], vec![]);
    let on = |f: &str| touched.is_empty() || touched.iter().any(|t| f.contains(&format!("[{t}]")) || f.split(|c: char| !(c.is_alphanumeric() || c == '#' || c == '_')).any(|w| w == t));
    for f in now { match was.iter().position(|w| *w == finding_key(&f)) { Some(i) => { was.swap_remove(i); } None if on(&f) => new.push(f), None => elsewhere.push(f) } }
    let by = if after["sheets"].is_array() { "KiCad's ERC of the saved schematic" } else { "KiCad's DRC of the saved board, schematic parity included" };
    let mut o = json!({"by": by, "new": new.iter().take(12).collect::<Vec<_>>(), "left": left});
    if new.len() > 12 { o["more"] = json!(new.len() - 12) }
    if !elsewhere.is_empty() { o["elsewhere"] = json!({"count": elsewhere.len(), "first": elsewhere.iter().take(3).collect::<Vec<_>>(), "note": "on items this edit did not work on: a refill or the file's first save moves such findings about; they are this edit's only if its copper cut a pour there"}) }
    if !was.is_empty() { o["gone"] = json!(was.len()) }
    o
}

// ---- units and names -------------------------------------------------------------------------------------------------------

fn nm_of(v: &Value) -> f64 { match v { Value::String(s) => s.parse().unwrap_or(0.0), Value::Number(n) => n.as_f64().unwrap_or(0.0), _ => 0.0 } }
/// KiCad's nanometres (a string or a number, absent for 0) as millimetres, at most six decimals.
pub fn mm(v: &Value) -> f64 { (nm_of(v) / 1e6 * 1e6).round() / 1e6 }
pub fn nm(mm: f64) -> Value { json!(((mm * 1e6).round() as i64).to_string()) }
fn xy(v: &Value) -> Value { json!([mm(&v["x_nm"]), mm(&v["y_nm"])]) }
fn to_xy(x: f64, y: f64) -> Value { json!({"x_nm": nm(x), "y_nm": nm(y)}) }
fn pair(v: &Value) -> Option<(f64, f64)> { let a = v.as_array()?; Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)) }
fn point(v: &Value, what: &str) -> Result<Value, String> { pair(v).map(|(x, y)| to_xy(x, y)).ok_or(format!("{what} is [x, y] in mm")) }
/// "BL_F_Cu" <-> "F.Cu".
pub fn layer_name(l: &str) -> String { l.strip_prefix("BL_").unwrap_or(l).replace('_', ".") }
pub fn layer_enum(l: &str) -> String { if l.starts_with("BL_") { l.into() } else { format!("BL_{}", l.replace('.', "_")) } }
fn text3(v: &Value) -> &str { v["text"]["text"]["text"].as_str().unwrap_or("") } // a board Field
fn text2(v: &Value) -> &str { v["text"]["text"].as_str().unwrap_or("") } // a schematic field, a label
fn lib_id(v: &Value) -> String { format!("{}:{}", v["library_nickname"].as_str().unwrap_or(""), v["entry_name"].as_str().unwrap_or("")) }
fn sso(v: &Value) -> i64 { v.as_str().and_then(|s| s.strip_prefix("SSO_")).and_then(|s| s.parse().ok()).unwrap_or(0) }

/// `*` and `?` in a selector's text.
pub fn glob(pat: &str, s: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pat.chars().collect(), s.chars().collect());
    let (mut i, mut j, mut star, mut mark) = (0, 0, usize::MAX, 0);
    while j < t.len() {
        if i < p.len() && (p[i] == '?' || p[i] == t[j]) { i += 1; j += 1 }
        else if i < p.len() && p[i] == '*' { star = i; mark = j; i += 1 }
        else if star != usize::MAX { i = star + 1; mark += 1; j = mark }
        else { return false }
    }
    while i < p.len() && p[i] == '*' { i += 1 }
    i == p.len()
}

// ---- kinds -----------------------------------------------------------------------------------------------------------------

/// What a selector's `kind` reads: (KiCad object types, board or schematic).
pub fn kind_types(kind: &str) -> Option<(&'static [&'static str], bool)> {
    Some(match kind {
        "footprint" | "pad" => (&["KOT_PCB_FOOTPRINT"], true), "track" => (&["KOT_PCB_TRACE"], true), "arc" => (&["KOT_PCB_ARC"], true),
        "via" => (&["KOT_PCB_VIA"], true), "zone" => (&["KOT_PCB_ZONE"], true), "text" => (&["KOT_PCB_TEXT"], true), "shape" => (&["KOT_PCB_SHAPE"], true),
        "symbol" => (&["KOT_SCH_SYMBOL"], false), "wire" => (&["KOT_SCH_LINE"], false), "label" => (&["KOT_SCH_LABEL", "KOT_SCH_GLOBAL_LABEL", "KOT_SCH_HIER_LABEL"], false),
        "junction" => (&["KOT_SCH_JUNCTION"], false), "no_connect" => (&["KOT_SCH_NO_CONNECT"], false), "sheet" => (&["KOT_SCH_SHEET"], false),
        _ => return None,
    })
}
pub const KINDS: &str = "board: footprint, pad, track, arc, via, zone, text, shape, net; schematic: symbol, wire, label, junction, no_connect, sheet";

fn fields_of(kind: &str, raw: &Value) -> Value {
    let mut m = Map::new();
    if kind == "footprint" { for f in raw["definition"]["items"].as_array().into_iter().flatten().filter(|i| i["@type"].as_str().is_some_and(|t| t.ends_with(".Field"))) { if !text3(f).is_empty() { m.insert(f["name"].as_str().unwrap_or("").into(), json!(text3(f))); } } }
    else { for f in raw["user_fields"].as_array().into_iter().flatten().chain([&raw["footprint_field"]]) { if !text2(f).is_empty() { m.insert(f["name"].as_str().unwrap_or("Footprint").into(), json!(text2(f))); } } }
    Value::Object(m)
}
/// The short row of one raw item: what an agent reads, and (the same keys) what `set` writes.
pub fn row(kind: &str, raw: &Value, with_fields: bool) -> Value {
    let id = raw["id"]["value"].as_str().unwrap_or("");
    let mut r = match kind {
        "footprint" => json!({"id": id, "ref": text3(&raw["reference_field"]), "value": text3(&raw["value_field"]), "at": xy(&raw["position"]), "rot": raw["orientation"]["value_degrees"].as_f64().unwrap_or(0.0), "layer": layer_name(raw["layer"].as_str().unwrap_or(""))}),
        "track" | "arc" => json!({"id": id, "from": xy(&raw["start"]), "to": xy(&raw["end"]), "width": mm(&raw["width"]["value_nm"]), "layer": layer_name(raw["layer"].as_str().unwrap_or("")), "net": raw["net"]["name"].as_str().unwrap_or("")}),
        "via" => json!({"id": id, "at": xy(&raw["position"]), "size": mm(&raw["pad_stack"]["copper_layers"][0]["size"]["x_nm"]), "drill": mm(&raw["pad_stack"]["drill"]["diameter"]["x_nm"]), "net": raw["net"]["name"].as_str().unwrap_or("")}),
        "zone" => json!({"id": id, "name": raw["name"].as_str().unwrap_or(""), "net": raw["copper_settings"]["net"]["name"].as_str().unwrap_or(""), "layers": raw["layers"].as_array().into_iter().flatten().map(|l| layer_name(l.as_str().unwrap_or(""))).collect::<Vec<_>>(),
            "priority": raw["priority"].as_u64().unwrap_or(0), "clearance": mm(&raw["copper_settings"]["clearance"]["value_nm"]), "min_width": mm(&raw["copper_settings"]["min_thickness"]["value_nm"]),
            "corners": raw["outline"]["polygons"][0]["outline"]["nodes"].as_array().map_or(0, |n| n.len()), "type": raw["type"].as_str().unwrap_or("")}),
        "symbol" => json!({"id": id, "ref": text2(&raw["reference_field"]), "value": text2(&raw["value_field"]), "at": xy(&raw["position"]), "rot": sso(&raw["transform"]["orientation"]), "lib": lib_id(if raw["lib_id"].is_object() { &raw["lib_id"] } else { &raw["definition"]["id"] }),
            "unit": raw["unit"]["unit"].as_u64().unwrap_or(1), "sheet": raw["path"]["path_human_readable"].as_str().unwrap_or("/")}),
        "wire" => json!({"id": id, "from": xy(&raw["start"]), "to": xy(&raw["end"]), "type": raw["type"].as_str().unwrap_or("")}),
        "label" => json!({"id": id, "text": text2(raw), "at": xy(&raw["position"]), "type": raw["@type"].as_str().unwrap_or("").rsplit('.').next().unwrap_or("")}),
        "junction" | "no_connect" => json!({"id": id, "at": xy(&raw["position"])}),
        _ => return raw.clone(),
    };
    if with_fields && matches!(kind, "footprint" | "symbol") { r["fields"] = fields_of(kind, raw); }
    r
}
/// A footprint's pads as rows: where a track may land.
pub fn pad_rows(fp: &Value) -> Vec<Value> {
    let r = text3(&fp["reference_field"]);
    fp["definition"]["items"].as_array().into_iter().flatten().filter(|i| i["@type"].as_str().is_some_and(|t| t.ends_with(".Pad")))
        .map(|p| { let st = &p["pad_stack"]; let c = &st["copper_layers"][0];
            let layer = if p["type"] == "PT_PTH" { "*.Cu".to_string() } else { st["layers"].as_array().into_iter().flatten().filter_map(|l| l.as_str()).find(|l| l.ends_with("_Cu")).map(layer_name).unwrap_or_default() };
            let mut row = json!({"id": p["id"]["value"].as_str().unwrap_or(""), "ref": r, "pad": p["number"].as_str().unwrap_or(""), "net": p["net"]["name"].as_str().unwrap_or(""), "at": xy(&p["position"]), "pin": p["symbol_pin"]["name"].as_str().unwrap_or("")});
            // its copper: size, turn and side (what a hand-placed track must clear), so the board file is not read for them
            if c["size"].is_object() { row["size"] = json!([mm(&c["size"]["x_nm"]), mm(&c["size"]["y_nm"])]); row["rot"] = json!(st["angle"]["value_degrees"].as_f64().unwrap_or(0.0)); row["layer"] = json!(layer) }
            row }).collect()
}

fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-6 }
/// A wanted value against a row's: text with `*`/`?`; a list against one value is any-of, against a list (a place, layers) is that list;
/// one value against a list is "has it".
fn same(want: &Value, got: &Value) -> bool {
    match (want, got) {
        (Value::Array(w), Value::Array(g)) => w.len() == g.len() && w.iter().zip(g).all(|(x, y)| same(x, y)),
        (Value::Array(w), _) => w.iter().any(|x| same(x, got)),
        (_, Value::Array(g)) => g.iter().any(|y| same(want, y)),
        (Value::String(w), Value::String(g)) => glob(w, g),
        (Value::Number(w), Value::Number(g)) => close(w.as_f64().unwrap_or(0.0), g.as_f64().unwrap_or(1.0)),
        _ => want == got,
    }
}
/// Whether a row passes a selector: every key but `kind`, `fields`, `raw`, `limit` and `sheet` must match the row's (text with `*`/`?`,
/// a list for any-of, `near: [x, y, r]` for a place); a key the row lacks does not match.
pub fn matches(sel: &Value, row: &Value) -> bool {
    let Some(o) = sel.as_object() else { return true };
    o.iter().all(|(k, want)| match k.as_str() {
        "kind" | "fields" | "raw" | "limit" => true,
        "sheet" => row.get("sheet").map_or(true, |s| same(want, s) || want.as_str().is_some_and(|w| s.as_str().is_some_and(|s| s.trim_matches('/') == w.trim_matches('/')))),
        "near" => { let (Some((x, y)), r) = (pair(want), want[2].as_f64().unwrap_or(0.5)) else { return false };
            // a track or wire is near along its whole length, anything else at its place
            match (pair(&row["from"]), pair(&row["to"])) {
                (Some((ax, ay)), Some((bx, by))) => { let (dx, dy) = (bx - ax, by - ay); let l2 = dx * dx + dy * dy; let t = if l2 == 0.0 { 0.0 } else { (((x - ax) * dx + (y - ay) * dy) / l2).clamp(0.0, 1.0) };
                    ((ax + t * dx - x).powi(2) + (ay + t * dy - y).powi(2)).sqrt() <= r + 1e-9 }
                _ => pair(&row["at"]).is_some_and(|(a, b)| ((a - x).powi(2) + (b - y).powi(2)).sqrt() <= r + 1e-9),
            } }
        "id" => same(want, &row["id"]),
        _ => row.get(k).is_some_and(|got| same(want, got)),
    })
}

// ---- writing ---------------------------------------------------------------------------------------------------------------

fn board_field(name: &str, text: &str, fp: &Value) -> Value {
    let back = fp["layer"].as_str() == Some("BL_B_Cu");
    json!({"@type": format!("{BT}Field"), "name": name, "text": {"layer": if back { "BL_B_Fab" } else { "BL_F_Fab" }, "text": {"text": text, "position": fp["position"].clone(),
        "attributes": {"visible": false, "mirrored": back, "keep_upright": true, "size": {"x_nm": "1000000", "y_nm": "1000000"}, "stroke_width": {"value_nm": "150000"}}}}})
}
fn sch_field(name: &str, text: &str, sym: &Value) -> Value {
    json!({"name": name, "visible": false, "text": {"text": text, "position": sym["position"].clone(), "attributes": {"visible": false, "size": {"x_nm": "1270000", "y_nm": "1270000"}}}})
}
/// A symbol's field by its name: Footprint, Datasheet and Description are KiCad's own fields of a symbol, the rest its user fields.
/// (A Footprint sent as a user field is not written: KiCad answers OK and the file does not have it.)
pub fn sym_field(raw: &mut Value, name: &str, t: &str) {
    let own = match name { "Footprint" => "footprint_field", "Datasheet" => "datasheet_field", "Description" => "description_field", _ => "" };
    if !own.is_empty() {
        if !raw[own].is_object() { raw[own] = json!({"name": name, "text": {"position": raw["position"].clone(), "attributes": {"visible": false, "size": {"x_nm": "1270000", "y_nm": "1270000"}}}}) }
        raw[own]["text"]["text"] = json!(t); return
    }
    let new = sch_field(name, t, raw);
    if !raw["user_fields"].is_array() { raw["user_fields"] = json!([]) }
    let fs = raw["user_fields"].as_array_mut().unwrap();
    match fs.iter_mut().find(|f| f["name"] == json!(name)) { Some(f) => f["text"]["text"] = json!(t), None => fs.push(new) }
}
/// A placed symbol's pins: (number, name, where a wire joins it, the direction away from the body), in the sheet's millimetres.
pub fn sym_pins(raw: &Value) -> Vec<(String, String, (f64, f64), (f64, f64))> {
    let (px, py) = (mm(&raw["position"]["x_nm"]), mm(&raw["position"]["y_nm"])); let (sn, cs) = (sso(&raw["transform"]["orientation"]) as f64).to_radians().sin_cos();
    let unit = raw["unit"]["unit"].as_u64().unwrap_or(1); let r3 = |v: f64| (v * 1e4).round() / 1e4;
    let turn = |x: f64, y: f64| (x * cs + y * sn, -x * sn + y * cs);
    raw["definition"]["items"].as_array().into_iter().flatten().filter(|i| i["item"]["@type"].as_str().is_some_and(|t| t.ends_with(".SchematicPin")) && matches!(i["unit"]["unit"].as_u64(), None | Some(0)) | (i["unit"]["unit"].as_u64() == Some(unit))).map(|i| { let p = &i["item"];
        let (x, y) = turn(mm(&p["position"]["x_nm"]), mm(&p["position"]["y_nm"]));
        let o = match p["orientation"].as_str().unwrap_or("") { "SPO_UP" => (0.0, 1.0), "SPO_DOWN" => (0.0, -1.0), "SPO_LEFT" => (1.0, 0.0), _ => (-1.0, 0.0) }; let (ox, oy) = turn(o.0, o.1);
        (p["number"].as_str().unwrap_or("").to_string(), p["name"].as_str().unwrap_or("").to_string(), (r3(px + x), r3(py + y)), (ox.round(), oy.round())) }).collect()
}
/// Apply a row's keys to the raw item (`set`). Says whether a footprint's body (its `definition`) changed, so the update may leave it out.
pub fn apply(kind: &str, raw: &mut Value, with: &Value) -> Result<bool, String> {
    let Some(o) = with.as_object() else { return Err("with is an object of row keys".into()) };
    let mut body = false;
    for (k, v) in o {
        let s = || v.as_str().map(str::to_string).ok_or(format!("{k} is text"));
        let n = || v.as_f64().ok_or(format!("{k} is a number (mm or degrees)"));
        match (kind, k.as_str()) {
            (_, "raw") => merge(raw, v),
            ("footprint", "ref") => raw["reference_field"]["text"]["text"]["text"] = json!(s()?),
            ("footprint", "value") => raw["value_field"]["text"]["text"]["text"] = json!(s()?),
            ("symbol", "at") => { let to = pair(v).ok_or("at is [x, y] in mm")?; let (dx, dy) = (to.0 - mm(&raw["position"]["x_nm"]), to.1 - mm(&raw["position"]["y_nm"]));
                // its texts go with it: each field has its own place on the sheet
                let shift = |f: &mut Value| if f["text"]["position"].is_object() { let p = &f["text"]["position"]; f["text"]["position"] = to_xy(mm(&p["x_nm"]) + dx, mm(&p["y_nm"]) + dy) };
                for k in ["reference_field", "value_field", "footprint_field", "datasheet_field", "description_field"] { shift(&mut raw[k]) }
                for f in raw["user_fields"].as_array_mut().into_iter().flatten() { shift(f) }
                raw["position"] = to_xy(to.0, to.1) }
            ("footprint" | "via" | "label" | "junction" | "no_connect", "at") => raw["position"] = point(v, "at")?,
            ("footprint", "rot") => raw["orientation"] = json!({"value_degrees": n()?}),
            ("footprint", "locked") => raw["locked"] = json!(if v.as_bool().unwrap_or(false) { "LS_LOCKED" } else { "LS_UNLOCKED" }),
            ("footprint", "fields") => { body = true;
                for (name, t) in v.as_object().ok_or("fields is {name: text}")? {
                    let t = t.as_str().ok_or("a field's value is text")?; let new = board_field(name, t, raw);
                    let items = raw["definition"]["items"].as_array_mut().ok_or("the footprint has no body")?;
                    match items.iter_mut().find(|i| i["@type"].as_str().is_some_and(|x| x.ends_with(".Field")) && i["name"] == json!(name)) { Some(f) => f["text"]["text"]["text"] = json!(t), None => items.push(new) }
                } }
            ("track" | "arc" | "wire", "from") => raw["start"] = point(v, "from")?,
            ("track" | "arc" | "wire", "to") => raw["end"] = point(v, "to")?,
            ("track" | "arc", "width") => raw["width"] = json!({"value_nm": nm(n()?)}),
            ("track" | "arc", "layer") => raw["layer"] = json!(layer_enum(&s()?)),
            ("track" | "arc" | "via", "net") => raw["net"] = json!({"name": s()?}),
            ("via", "size") => { let d = to_xy(n()?, n()?); for l in raw["pad_stack"]["copper_layers"].as_array_mut().into_iter().flatten() { l["size"] = d.clone() } }
            ("via", "drill") => raw["pad_stack"]["drill"]["diameter"] = to_xy(n()?, n()?),
            ("zone", "name") => raw["name"] = json!(s()?),
            ("zone", "net") => raw["copper_settings"]["net"] = json!({"name": s()?}),
            ("zone", "priority") => raw["priority"] = json!(n()? as u64),
            ("zone", "clearance") => raw["copper_settings"]["clearance"] = json!({"value_nm": nm(n()?)}),
            ("zone", "min_width") => raw["copper_settings"]["min_thickness"] = json!({"value_nm": nm(n()?)}),
            ("zone", "layers") => raw["layers"] = json!(v.as_array().ok_or("layers is a list")?.iter().map(|l| layer_enum(l.as_str().unwrap_or(""))).collect::<Vec<_>>()),
            ("symbol", "ref") => raw["reference_field"]["text"]["text"] = json!(s()?),
            ("symbol", "value") => raw["value_field"]["text"]["text"] = json!(s()?),
            ("symbol", "rot") => raw["transform"]["orientation"] = json!(format!("SSO_{}", n()? as i64)),
            ("symbol", "fields") => for (name, t) in v.as_object().ok_or("fields is {name: text}")? { sym_field(raw, name, t.as_str().ok_or("a field's value is text")?) },
            ("symbol", "footprint") => sym_field(raw, "Footprint", &s()?),
            ("label", "text") => raw["text"]["text"] = json!(s()?),
            _ => return Err(format!("{kind} has no settable `{k}` (its row's keys, or raw: {{...}} merged into KiCad's own item)")),
        }
    }
    Ok(body)
}
/// Deep merge: objects by key, everything else replaced.
pub fn merge(into: &mut Value, from: &Value) {
    match (into.as_object_mut(), from.as_object()) {
        (Some(a), Some(b)) => for (k, v) in b { merge(a.entry(k.clone()).or_insert(Value::Null), v) },
        _ => *into = from.clone(),
    }
}
/// The item an update sends: zones without their fills (KiCad refills), a footprint or symbol without its body unless that changed.
fn slim(kind: &str, mut raw: Value, body: bool) -> Value {
    if let Some(o) = raw.as_object_mut() {
        if kind == "zone" { o.remove("filled_polygons"); }
        if (kind == "footprint" && !body) || kind == "symbol" { o.remove("definition"); }
    }
    raw
}

/// The raw items one `add` entry becomes (board or schematic).
pub fn build(item: &Value) -> Result<Vec<Value>, String> {
    if item["@type"].is_string() { return Ok(vec![item.clone()]) }
    let Some((k, v)) = item.as_object().and_then(|o| o.iter().find(|(k, _)| k.as_str() != "sheet")) else { return Err("an item to add is {track|via|wire|label|junction|no_connect|symbol: ...} or KiCad's own item with @type".into()) };
    let text = |key: &str| v[key].as_str().unwrap_or("").to_string();
    Ok(match k.as_str() {
        "track" => vec![json!({"@type": format!("{BT}Track"), "start": point(&v["from"], "from")?, "end": point(&v["to"], "to")?, "width": {"value_nm": nm(v["width"].as_f64().ok_or("track.width in mm")?)},
            "layer": layer_enum(v["layer"].as_str().ok_or("track.layer, e.g. F.Cu")?), "net": {"name": text("net")}})],
        "via" => { let (size, drill) = (v["size"].as_f64().ok_or("via.size in mm")?, v["drill"].as_f64().ok_or("via.drill in mm")?);
            let ls: Vec<String> = v["layers"].as_array().map(|a| a.iter().map(|l| layer_enum(l.as_str().unwrap_or(""))).collect()).unwrap_or_else(|| vec!["BL_F_Cu".into(), "BL_B_Cu".into()]);
            vec![json!({"@type": format!("{BT}Via"), "position": point(&v["at"], "at")?, "type": if v["layers"].is_array() { "VT_BLIND_BURIED" } else { "VT_THROUGH" }, "net": {"name": text("net")},
                "pad_stack": {"type": "PST_NORMAL", "layers": ls, "drill": {"start_layer": ls[0], "end_layer": ls[ls.len() - 1], "diameter": to_xy(drill, drill), "shape": "DS_CIRCLE"}, "copper_layers": [{"layer": "BL_F_Cu", "shape": "PSS_CIRCLE", "size": to_xy(size, size)}]}})] }
        "wire" => { let pts = v.as_array().ok_or("wire is [[x, y], [x, y], ...]")?; if pts.len() < 2 { return Err("a wire has two points or more".into()) }
            pts.windows(2).map(|w| Ok(json!({"@type": format!("{ST}SchematicLine"), "type": "SLT_WIRE", "start": point(&w[0], "a wire point")?, "end": point(&w[1], "a wire point")?}))).collect::<Result<Vec<_>, String>>()? }
        "label" => { let ty = match v["type"].as_str().unwrap_or("local") { "global" => "GlobalLabel", "hier" | "hierarchical" => "HierarchicalLabel", _ => "LocalLabel" }; let at = point(&v["at"], "at")?;
            vec![json!({"@type": format!("{ST}{ty}"), "position": at, "text": {"position": at, "text": text("text"), "attributes": {"size": {"x_nm": "1270000", "y_nm": "1270000"}}}})] }
        "junction" => vec![json!({"@type": format!("{ST}Junction"), "position": point(v, "junction")?})],
        "no_connect" => vec![json!({"@type": format!("{ST}NoConnect"), "position": point(v, "no_connect")?})],
        "symbol" => { let lib = text("lib"); let (nick, entry) = lib.split_once(':').ok_or("symbol.lib is Library:Symbol")?; let id = json!({"library_nickname": nick, "entry_name": entry});
            let mut s = json!({"@type": format!("{ST}SchematicSymbolInstance"), "position": point(&v["at"], "at")?, "definition": {"id": id}, "lib_id": id, "unit": {"unit": v["unit"].as_u64().unwrap_or(1)},
                "transform": {"orientation": format!("SSO_{}", v["rot"].as_i64().unwrap_or(0))}, "reference_field": {"name": "Reference", "text": {"text": text("ref")}}});
            if v["value"].is_string() { s["value_field"] = json!({"name": "Value", "text": {"text": text("value")}}) }
            if let Some(fs) = v["fields"].as_object() { s["user_fields"] = json!(fs.iter().map(|(n, t)| sch_field(n, t.as_str().unwrap_or(""), &s)).collect::<Vec<_>>()) }
            vec![s] }
        other => return Err(format!("cannot add `{other}`: track, via, wire, label, junction, no_connect, symbol, or KiCad's own item with @type")),
    })
}

/// A new symbol from KiCad's own placement of it (`Run::templates`) and what the add entry asks: its reference, value and fields.
pub fn placed(template: &Value, v: &Value) -> Value {
    let mut s = template.clone();
    if let Some(o) = s.as_object_mut() { o.remove("id"); o.remove("path"); }
    // KiCad's own whole definition goes back with it: a symbol that extends another (AMS1117-3.3) sent as its library id alone gets a
    // sheet copy that ERC calls a library mismatch
    if let Some(r) = v["ref"].as_str() { s["reference_field"]["text"]["text"] = json!(r) }
    if let Some(x) = v["value"].as_str() { s["value_field"]["text"]["text"] = json!(x) }
    for (n, t) in v["fields"].as_object().into_iter().flatten() { sym_field(&mut s, n, t.as_str().unwrap_or("")) }
    if let Some(f) = v["footprint"].as_str() { sym_field(&mut s, "Footprint", f) }
    s
}

/// SetNetClasses + SetNetClassAssignments for one `netclass` op (millimetres in, merged with what the project has).
pub fn netclass_msgs(v: &Value, project: &Value) -> Result<Vec<(String, Value)>, String> {
    let name = v["name"].as_str().ok_or("netclass.name")?; let mut board = Map::new();
    for (k, raw) in [("clearance", "clearance"), ("track_width", "track_width"), ("diff_pair_width", "diff_pair_track_width"), ("diff_pair_gap", "diff_pair_gap"), ("diff_pair_via_gap", "diff_pair_via_gap")] {
        if let Some(x) = v[k].as_f64() { board.insert(raw.into(), json!({"value_nm": nm(x)})); }
    }
    if let (Some(d), Some(h)) = (v["via_diameter"].as_f64(), v["via_drill"].as_f64()) { board.insert("via_stack".into(), json!({"drill": {"diameter": to_xy(h, h)}, "copper_layers": [{"layer": "BL_F_Cu", "shape": "PSS_CIRCLE", "size": to_xy(d, d)}]})); }
    let mut out = vec![];
    if !board.is_empty() || v["patterns"].is_null() { out.push((format!("{C}SetNetClasses"), json!({"project": project, "merge_mode": "MMM_MERGE", "net_classes": [{"name": name, "type": "NCT_EXPLICIT", "board": board}]}))) }
    if let Some(ps) = v["patterns"].as_array() {
        out.push((format!("{C}SetNetClassAssignments"), json!({"project": project, "merge_mode": "MMM_MERGE", "pattern_assignments": ps.iter().map(|p| json!({"pattern": p, "netclass": name})).collect::<Vec<_>>()})));
    }
    Ok(out)
}
/// A net class as the project has it, in the `netclass` op's own keys.
pub fn netclass_row(name: &Value, classes: &Value, assignments: &Value) -> Value {
    let mut r = json!({"name": name});
    if let Some(c) = classes["net_classes"].as_array().into_iter().flatten().find(|c| c["name"] == *name) {
        for (k, raw) in [("clearance", "clearance"), ("track_width", "track_width"), ("diff_pair_width", "diff_pair_track_width"), ("diff_pair_gap", "diff_pair_gap"), ("diff_pair_via_gap", "diff_pair_via_gap")] {
            if c["board"][raw].is_object() { r[k] = json!(mm(&c["board"][raw]["value_nm"])) }
        }
    }
    r["patterns"] = json!(assignments["pattern_assignments"].as_array().into_iter().flatten().filter(|a| a["netclass"] == *name).map(|a| a["pattern"].clone()).collect::<Vec<_>>());
    r
}
/// One custom rule for EditCustomRules: {name, condition, constraints: [{type: "clearance", min|opt|max: mm}], layer?, severity?}.
pub fn rule_msg(v: &Value) -> Result<Value, String> {
    let mut r = json!({"name": v["name"].as_str().ok_or("rule.name")?, "condition": v["condition"].as_str().unwrap_or("")});
    let mut cs = vec![];
    for c in v["constraints"].as_array().ok_or("rule.constraints is a list of {type, min|opt|max}")? {
        if c["type"].as_str().is_some_and(|t| t.starts_with("CRCT_")) { cs.push(c.clone()); continue }
        let ty = c["type"].as_str().ok_or("a constraint's type, e.g. clearance")?; let mut num = Map::new();
        // lengths in mm; counts (via_count, min_resolved_spokes) and ratios as they are
        let plain = matches!(ty, "via_count" | "min_resolved_spokes" | "solder_paste_rel_margin" | "hole_aspect_ratio" | "microvia_stack_depth" | "track_angle");
        for k in ["min", "opt", "max"] { if let Some(x) = c[k].as_f64() { num.insert(k.into(), json!(if plain { x.round() as i64 } else { (x * 1e6).round() as i64 })); } }
        cs.push(json!({"type": format!("CRCT_{}", ty.to_uppercase()), "numeric": num}));
    }
    r["constraints"] = json!(cs);
    if let Some(l) = v["layer"].as_str() { r["single_layer"] = json!(layer_enum(l)) }
    if let Some(s) = v["severity"].as_str() { r["severity"] = json!(format!("RS_{}", s.to_uppercase())) }
    Ok(r)
}

/// `$doc` and `$project` in a raw message stand for the open document and its project.
pub fn fill(msg: &mut Value, doc: &Value) {
    match msg {
        Value::String(s) if s == "$doc" => *msg = doc.clone(),
        Value::String(s) if s == "$project" => *msg = doc["project"].clone(),
        Value::Array(a) => a.iter_mut().for_each(|x| fill(x, doc)),
        Value::Object(o) => o.values_mut().for_each(|x| fill(x, doc)),
        _ => {}
    }
}
/// A raw response made short: created and updated items as their ids and status, empties dropped.
pub fn brief(mut msg: Value) -> Value {
    for k in ["created_items", "updated_items"] {
        if let Some(a) = msg.get_mut(k).and_then(Value::as_array_mut) {
            for i in a.iter_mut() { let (id, st) = (i["item"]["id"]["value"].clone(), i["status"].clone()); *i = if st["code"] == "ISC_OK" { id } else { json!({"id": id, "status": st}) } }
        }
    }
    if msg["@type"] == "google.protobuf.Empty" { return json!({}) }
    if let Some(o) = msg.as_object_mut() { o.remove("header"); }
    msg
}
/// The full command name of a short one (`GetItems`, or a wrong package before a right name).
pub fn full_type(ty: &str) -> String {
    let short = ty.rsplit('.').next().unwrap_or(ty);
    let pkg = match short {
        "GetNets" | "GetItemsByNet" | "GetItemsByNetClass" | "GetConnectedItems" | "GetNetClassForNets" | "RefillZones" | "GetBoardStackup" | "GetBoardEnabledLayers" | "SetBoardEnabledLayers" | "GetBoardDesignRules" | "SetBoardDesignRules"
            | "GetCustomDesignRules" | "SetCustomDesignRules" | "GetBoardOrigin" | "SetBoardOrigin" | "GetPadShapeAsPolygon" | "FlipItems" | "PlaceFootprintFromLibrary" | "GetBoardBoundingBox" | "ImportNetlist" | "GetGraphicsDefaults" => B,
        "GetSchematicHierarchy" | "GetSchematicNetlist" | "PlaceSymbolFromLibrary" => S,
        "EditCustomRules" | "GetNotices" | "GetAgentChanges" | "UndoAgentChange" => "kiapi.backplane.coedit.",
        "SketchRoute" | "GetCapabilities" => "kiapi.backplane.",
        "GetItems" | "GetItemsById" | "CreateItems" | "UpdateItems" | "DeleteItems" | "BeginCommit" | "EndCommit" | "OpenDocument" | "CloseDocument" | "SaveDocument" | "GetOpenDocuments" | "GetNetClasses" | "SetNetClasses"
            | "GetNetClassAssignments" | "SetNetClassAssignments" | "GetBoundingBox" | "GetTitleBlockInfo" | "SetTitleBlockInfo" | "GetTextVariables" | "SetTextVariables" | "GetVersion" | "Ping" => C,
        _ => return ty.to_string(),
    };
    format!("{pkg}{short}")
}

// ---- the driver ------------------------------------------------------------------------------------------------------------

struct Run<'a> { c: &'a dyn Call, doc: Value, board: bool, commit: Option<Value>, sheets: Option<Vec<(String, Value)>>, refill: Vec<Value>, wrote: bool, cache: std::collections::HashMap<String, Vec<(Value, Value)>>,
    /// The commit's name, the op (1-based) whose `find` ended a commit early, and each new symbol as KiCad itself places it (by its add entry's text).
    message: String, split: usize, templates: std::collections::HashMap<String, Value>,
    /// The new symbols of this call by reference (their templates: where their pins are), the parts it added or exchanged (the answer names
    /// their nets), whether a library has answered (then "not found" is final), and how long a library's load is waited for between asks.
    planned: std::collections::HashMap<String, Value>, added: Vec<String>, libs: bool, lib_step: std::time::Duration,
    /// The highest #PWR number in use (new power symbols count on from it).
    pwr: Option<u64>,
    /// The nets and references this call worked on (the check's `new` is about these), and whether something it did has no such list.
    touched: Vec<String>, touch_all: bool,
    /// Where a pad stood before its footprint was exchanged in this call (net, x, y): an old track's end there is cut back when the part is routed.
    vacated: Vec<(String, f64, f64)>,
    /// What the saved sheet files must show (reference, property, text), and how many updates KiCad deferred (the person is editing those items).
    expect: Vec<(String, String, String)>, deferred: usize,
    /// Copper moved or was added (a route, a placement, a track): every zone is refilled before the save, so no pour is left over the new copper.
    refill_all: bool }
/// Rows a change answers with: what the items are now, so nothing is read again to check it.
const SHOWN: usize = 24;
/// The row kind of one of KiCad's own items, by its type.
fn kind_of(raw: &Value) -> &'static str {
    match raw["@type"].as_str().unwrap_or("").rsplit('.').next().unwrap_or("") {
        "Track" => "track", "Arc" => "arc", "Via" => "via", "Zone" => "zone", "FootprintInstance" => "footprint", "SchematicSymbolInstance" => "symbol", "SchematicLine" => "wire",
        "LocalLabel" | "GlobalLabel" | "HierarchicalLabel" => "label", "Junction" => "junction", "NoConnect" => "no_connect", _ => "",
    }
}

fn verb(op: &Value) -> &str { const V: &[&str] = &["find", "move", "set", "add", "delete", "ipc", "netclass", "rule", "refill", "route", "place"]; op.as_object().and_then(|o| V.iter().find(|v| o.contains_key(**v))).copied().unwrap_or("") }
/// A selector from an op's target: an object as it is; text or a list of texts are references of the document's parts.
fn selector(target: &Value, board: bool) -> Value {
    if target.is_object() { let mut t = target.clone(); if t["kind"].is_null() { t["kind"] = json!(if board { "footprint" } else { "symbol" }) } t }
    else { json!({"kind": if board { "footprint" } else { "symbol" }, "ref": target}) }
}
fn status_bad(r: &Value, key: &str) -> Option<String> {
    r[key].as_array().into_iter().flatten().find_map(|i| { let c = i["status"]["code"].as_str().unwrap_or(""); (!matches!(c, "ISC_OK" | "ISC_DEFERRED" | "")).then(|| format!("{c} {}", i["status"]["error_message"].as_str().unwrap_or(""))) })
}

impl Run<'_> {
    async fn open(c: &dyn Call, path: &str) -> Result<Value, String> {
        let ty = if path.ends_with(".kicad_pcb") { "DOCTYPE_PCB" } else if path.ends_with(".kicad_sch") { "DOCTYPE_SCHEMATIC" } else { return Err("path is the board (.kicad_pcb) or the schematic (.kicad_sch) to edit".into()) };
        if let Ok(r) = c.call(&format!("{C}GetOpenDocuments"), json!({"type": ty})).await { if let Some(d) = r["documents"].get(0) { return Ok(d.clone()) } }
        Ok(c.call(&format!("{C}OpenDocument"), json!({"type": ty, "path": path})).await?["document"].clone())
    }
    fn header(&self, sheet: &Value) -> Value { let mut d = self.doc.clone(); if !sheet.is_null() { d["sheet_path"] = sheet.clone() } json!({"document": d}) }
    async fn sheets(&mut self) -> Result<Vec<(String, Value)>, String> {
        if self.sheets.is_none() {
            fn walk(n: &Value, out: &mut Vec<(String, Value)>) { out.push((n["path"]["path_human_readable"].as_str().unwrap_or("/").into(), json!({"path": n["path"]["path"].clone()}))); for k in n["children"].as_array().into_iter().flatten() { walk(k, out) } }
            let h = self.c.call(&format!("{S}GetSchematicHierarchy"), json!({"document": self.doc})).await?; let mut v = vec![];
            for t in h["top_level_sheets"].as_array().into_iter().flatten() { walk(t, &mut v) }
            self.sheets = Some(v);
        }
        Ok(self.sheets.clone().unwrap_or_default())
    }
    /// The sheet a `sheet` key names ("/", "/psu/" or "psu"): its path for a header; Null is the root.
    async fn sheet(&mut self, name: &Value) -> Result<Value, String> {
        let Some(n) = name.as_str().filter(|n| !self.board && n.trim_matches('/') != "") else { return Ok(Value::Null) };
        let all = self.sheets().await?;
        all.iter().find(|(h, _)| h.trim_matches('/') == n.trim_matches('/') || h.trim_matches('/').rsplit('/').next() == Some(n.trim_matches('/'))).map(|(_, p)| p.clone())
            .ok_or(format!("no sheet `{n}`: {}", all.iter().map(|(h, _)| h.as_str()).collect::<Vec<_>>().join(", ")))
    }
    /// Every raw item of a kind, with the sheet it is on (Null on a board; a symbol's own path).
    async fn items(&mut self, kind: &str) -> Result<Vec<(Value, Value)>, String> {
        if let Some(v) = self.cache.get(kind) { return Ok(v.clone()) }
        let (types, board) = kind_types(kind).ok_or(format!("unknown kind `{kind}` ({KINDS})"))?;
        if board != self.board { return Err(format!("`{kind}` is a {} kind; this document is the {}", if board { "board" } else { "schematic" }, if self.board { "board" } else { "schematic" })) }
        let mut out = vec![];
        if self.board || kind == "symbol" {
            let r = self.c.call(&format!("{C}GetItems"), json!({"header": self.header(&Value::Null), "types": types})).await?;
            for i in r["items"].as_array().into_iter().flatten() { let sheet = if kind == "symbol" { json!({"path": i["path"]["path"].clone()}) } else { Value::Null }; out.push((i.clone(), sheet)) }
        } else {
            for (_, p) in self.sheets().await? {
                let r = self.c.call(&format!("{C}GetItems"), json!({"header": self.header(&p), "types": types})).await?;
                for i in r["items"].as_array().into_iter().flatten() { out.push((i.clone(), p.clone())) }
            }
        }
        self.cache.insert(kind.into(), out.clone());
        Ok(out)
    }
    /// The raw items a selector picks, each with its sheet.
    async fn pick(&mut self, sel: &Value) -> Result<Vec<(Value, Value)>, String> {
        let kind = sel["kind"].as_str().unwrap_or("").to_string();
        let human: std::collections::HashMap<String, String> = if self.board || sel["sheet"].is_null() { Default::default() } else { self.sheets().await?.into_iter().map(|(h, p)| (p.to_string(), h)).collect() };
        Ok(self.items(&kind).await?.into_iter().filter(|(raw, sheet)| { let mut r = row(&kind, raw, true); if r.get("sheet").is_none() { if let Some(h) = human.get(&sheet.to_string()) { r["sheet"] = json!(h) } } matches(sel, &r) }).collect())
    }
    async fn begin(&mut self) -> Result<(), String> {
        if self.commit.is_none() { self.commit = Some(self.c.call(&format!("{C}BeginCommit"), json!({"header": self.header(&Value::Null)})).await?["id"].clone()) }
        self.wrote = true; self.cache.clear(); Ok(())
    }
    async fn end(&mut self, action: &str, message: &str) -> Result<(), String> {
        let Some(id) = self.commit.take() else { return Ok(()) };
        self.c.call(&format!("{C}EndCommit"), json!({"header": self.header(&Value::Null), "id": id, "action": action, "message": message})).await.map(|_| ())
    }
    /// One thing placed by KiCad from its library in a commit that is dropped: the item as KiCad makes it (a symbol's reference and value where
    /// the library has them, a footprint's pads where they land). Libraries load on demand: one not there yet is asked for and waited for (10 s).
    async fn from_library(&mut self, ty: &str, msg: Value) -> Result<Value, String> {
        for round in 0..22 {
            let id = self.c.call(&format!("{C}BeginCommit"), json!({"header": self.header(&Value::Null)})).await?["id"].clone();
            let made = self.c.call(ty, msg.clone()).await;
            self.c.call(&format!("{C}EndCommit"), json!({"header": self.header(&Value::Null), "id": id, "action": "CMA_DROP"})).await?;
            match made { Ok(v) => { self.libs = true; return Ok(v["item"].clone()) }
                Err(e) if e.contains("not found") && !self.libs && round < 21 => { if round == 0 { let _ = self.c.call(&format!("{C}LoadAllLibraries"), json!({})).await; } tokio::time::sleep(self.lib_step).await }
                Err(e) => return Err(e) }
        }
        Err("the library did not load".into())
    }
    async fn symbol_template(&mut self, it: &Value, sheet: &Value) -> Result<Value, String> {
        let v = &it["symbol"]; let lib = v["lib"].as_str().unwrap_or(""); let (nick, entry) = lib.split_once(':').ok_or("symbol.lib is Library:Symbol")?;
        let mut msg = json!({"header": self.header(sheet), "lib_id": {"library_nickname": nick, "entry_name": entry}, "position": point(&v["at"], "at")?, "orientation": format!("SSO_{}", v["rot"].as_i64().unwrap_or(0)), "unit": {"unit": v["unit"].as_u64().unwrap_or(1)}});
        if let Some(r) = v["ref"].as_str() { msg["reference"] = json!(r) }
        self.from_library(&format!("{S}PlaceSymbolFromLibrary"), msg).await.map_err(|e| format!("symbol {lib}: {e}"))
    }
    /// A point given as a pin, "U7.3" (by number, or by the pin's name): where a wire joins that pin.
    async fn pin(&mut self, name: &str) -> Result<(String, String, (f64, f64), (f64, f64)), String> {
        let (r, p) = name.rsplit_once('.').ok_or(format!("`{name}` is not a point [x, y] or a pin REF.PIN"))?;
        if !self.added.iter().any(|a| a == r) { self.added.push(r.to_string()) }
        let mut pins: Vec<(String, String, (f64, f64), (f64, f64))> = self.planned.get(r).map(sym_pins).unwrap_or_default();
        if pins.is_empty() { pins = self.items("symbol").await?.iter().filter(|(s, _)| text2(&s["reference_field"]) == r).flat_map(|(s, _)| sym_pins(s)).collect(); if pins.is_empty() { return Err(format!("no symbol `{r}` (for the pin `{name}`)")) } }
        pins.iter().find(|x| x.0 == p).or_else(|| pins.iter().find(|x| x.1 == p)).cloned().ok_or(format!("{r} has no pin `{p}`: {}", pins.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(", ")))
    }
    /// One `add` with every new symbol placed once by KiCad (its template), pins given as points resolved ("U7.3" in a wire, a label's `at`),
    /// and a symbol's `pins: {pin: net}` made into what joins each pin to its net: a short wire and the net's power symbol (`power:GND`),
    /// or a label where the power library has no such symbol.
    async fn expand(&mut self, op: &Value, list: Vec<Value>) -> Result<Vec<Value>, String> {
        let mut out = vec![];
        for mut it in list {
            let sheet = self.sheet(if it["sheet"].is_null() { &op["sheet"] } else { &it["sheet"] }).await?;
            // pins as points
            if it["wire"].is_array() { let mut pts: Vec<Value> = vec![]; let named = it["wire"].as_array().unwrap().iter().any(Value::is_string);
                for w in it["wire"].as_array().unwrap().clone() { let q = match w.as_str() { Some(n) => { let p = self.pin(n).await?.2; json!([p.0, p.1]) } None => w };
                    // between pins a wire runs square: a corner where the two ends share neither x nor y
                    if named { if let (Some((ax, ay)), Some((bx, by))) = (pts.last().and_then(pair), pair(&q)) { if (ax - bx).abs() > 1e-6 && (ay - by).abs() > 1e-6 { pts.push(json!([bx, ay])) } } }
                    pts.push(q) }
                it["wire"] = json!(pts) }
            for k in ["label", "symbol"] { if let Some(n) = it[k]["at"].as_str().map(str::to_string) { let p = self.pin(&n).await?.2; it[k]["at"] = json!([p.0, p.1]) } }
            for k in ["junction", "no_connect"] { if let Some(n) = it[k].as_str().map(str::to_string) { let p = self.pin(&n).await?.2; it[k] = json!([p.0, p.1]) } }
            if !it["symbol"].is_object() { out.push(it); continue }
            let t = self.symbol_template(&it, &sheet).await?;
            if let Some(r) = it["symbol"]["ref"].as_str() { self.planned.insert(r.to_string(), t.clone()); }
            self.templates.insert(it.to_string(), t.clone()); let pins = it["symbol"]["pins"].clone(); out.push(it.clone());
            for (pin, net) in pins.as_object().into_iter().flatten() {
                let net = net.as_str().ok_or("pins is {pin: net name}")?; let all = sym_pins(&t);
                let Some(p) = all.iter().find(|x| x.0 == *pin).or_else(|| all.iter().find(|x| x.1 == *pin)) else { return Err(format!("{} has no pin `{pin}`: {}", it["symbol"]["lib"].as_str().unwrap_or(""), all.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(", "))) };
                let (a, q) = (p.2, ((p.2 .0 + p.3 .0 * 2.54) * 1e4_f64, (p.2 .1 + p.3 .1 * 2.54) * 1e4_f64)); let q = (q.0.round() / 1e4, q.1.round() / 1e4);
                let mut wire = json!({"wire": [[a.0, a.1], [q.0, q.1]]}); if !it["sheet"].is_null() { wire["sheet"] = it["sheet"].clone() } out.push(wire);
                // the next free #PWR number
                if self.pwr.is_none() { self.pwr = Some(self.items("symbol").await?.iter().filter_map(|(s, _)| text2(&s["reference_field"]).strip_prefix("#PWR").and_then(|n| n.parse::<u64>().ok())).max().unwrap_or(0)) }
                let n = self.pwr.unwrap() + 1;
                let mut sym = json!({"symbol": {"lib": format!("power:{net}"), "at": [q.0, q.1], "ref": format!("#PWR{n:03}")}}); if !it["sheet"].is_null() { sym["sheet"] = it["sheet"].clone() }
                match self.symbol_template(&sym, &sheet).await {
                    Ok(pt) => { self.pwr = Some(n); self.templates.insert(sym.to_string(), pt); out.push(sym) }
                    Err(e) if e.contains("not found") => { let mut l = json!({"label": {"text": net, "at": [q.0, q.1]}}); if !it["sheet"].is_null() { l["sheet"] = it["sheet"].clone() } out.push(l) }
                    Err(e) => return Err(e) }
            }
        }
        Ok(out)
    }
    /// The box (x0, y0, x1, y1) of what a schematic `add` entry draws: a symbol by its pins, a wire by its points, a label by its text.
    fn entry_box(&self, it: &Value) -> Option<(f64, f64, f64, f64)> {
        let around = |pts: &[(f64, f64)], pad: f64| pts.iter().fold(None, |m: Option<(f64, f64, f64, f64)>, p| Some(match m { None => (p.0 - pad, p.1 - pad, p.0 + pad, p.1 + pad), Some(b) => (b.0.min(p.0 - pad), b.1.min(p.1 - pad), b.2.max(p.0 + pad), b.3.max(p.1 + pad)) }));
        if it["symbol"].is_object() { let mut pts: Vec<(f64, f64)> = pair(&it["symbol"]["at"]).into_iter().collect(); if let Some(t) = self.templates.get(&it.to_string()) { pts.extend(sym_pins(t).iter().map(|p| p.2)) } return around(&pts, 2.0) }
        if let Some(w) = it["wire"].as_array() { return around(&w.iter().filter_map(pair).collect::<Vec<_>>(), 0.3) }
        if it["label"].is_object() { let (x, y) = pair(&it["label"]["at"])?; let l = it["label"]["text"].as_str().unwrap_or("").len() as f64 * 1.1; return Some((x - 0.5, y - 1.5, x + l, y + 0.5)) }
        for k in ["junction", "no_connect"] { if let Some(p) = pair(&it[k]) { return around(&[p], 0.3) } }
        None
    }
    /// What is on a sheet already, as boxes with a few words for each: symbols, wires, labels, junctions and sheet symbols.
    async fn sheet_boxes(&mut self, sheet: &Value) -> Result<Vec<((f64, f64, f64, f64), String)>, String> {
        let want = if sheet.is_null() { self.sheets().await?.first().map(|s| s.1["path"].clone()).unwrap_or(Value::Null) } else { sheet["path"].clone() };
        let mut out = vec![]; let r2 = |v: f64| (v * 100.0).round() / 100.0;
        for kind in ["symbol", "wire", "label", "junction", "sheet"] { for (raw, sh) in self.items(kind).await? {
            if !want.is_null() && sh["path"] != want { continue }
            let rw = row(kind, &raw, false); let at = pair(&rw["at"]);
            match kind {
                "symbol" => { let mut pts: Vec<(f64, f64)> = at.into_iter().collect(); pts.extend(sym_pins(&raw).iter().map(|p| p.2)); if pts.is_empty() { continue }
                    let b = pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, p| (m.0.min(p.0), m.1.min(p.1), m.2.max(p.0), m.3.max(p.1))); out.push(((b.0 - 2.0, b.1 - 2.0, b.2 + 2.0, b.3 + 2.0), format!("{}", rw["ref"].as_str().unwrap_or("a symbol")))) }
                "wire" => if let (Some(a), Some(z)) = (pair(&rw["from"]), pair(&rw["to"])) { out.push(((a.0.min(z.0) - 0.3, a.1.min(z.1) - 0.3, a.0.max(z.0) + 0.3, a.1.max(z.1) + 0.3), format!("the wire {},{} to {},{}", r2(a.0), r2(a.1), r2(z.0), r2(z.1)))) },
                "label" => if let Some((x, y)) = at { let t = rw["text"].as_str().unwrap_or(""); out.push(((x - 0.5, y - 1.5, x + t.len() as f64 * 1.1, y + 0.5), format!("the label {t}"))) },
                "junction" => if let Some((x, y)) = at { out.push(((x - 0.3, y - 0.3, x + 0.3, y + 0.3), format!("the junction {},{}", r2(x), r2(y)))) },
                _ => { let (x, y, w, h) = (mm(&raw["position"]["x_nm"]), mm(&raw["position"]["y_nm"]), mm(&raw["size"]["x_nm"]), mm(&raw["size"]["y_nm"])); out.push(((x, y, x + w, y + h), "a sheet symbol".into())) }
            }
        } }
        Ok(out)
    }
    /// A board footprint exchanged for another library footprint: KiCad places the new one where the old stands (pad 1 on the old pad 1,
    /// the same side and rotation), and the old part keeps its identity (reference, value, fields, its symbol) and each pad its net by number.
    async fn swap(&mut self, old: &Value, lib: &str) -> Result<Value, String> {
        let (nick, entry) = lib.split_once(':').ok_or("footprint is Library:Footprint")?;
        let pad1 = |fp: &Value| { let pads: Vec<&Value> = fp["definition"]["items"].as_array().into_iter().flatten().filter(|i| i["@type"].as_str().is_some_and(|t| t.ends_with(".Pad"))).collect();
            pads.iter().find(|p| p["number"] == "1").or(pads.first()).map(|p| (mm(&p["position"]["x_nm"]), mm(&p["position"]["y_nm"]))) };
        let (ox, oy) = (mm(&old["position"]["x_nm"]), mm(&old["position"]["y_nm"]));
        let hdr = self.header(&Value::Null); let msg = |x: f64, y: f64| { let mut m = json!({"header": hdr, "lib_id": {"library_nickname": nick, "entry_name": entry}, "position": to_xy(x, y), "layer": old["layer"]}); if old["orientation"]["value_degrees"].is_number() { m["orientation"] = old["orientation"].clone() } m };
        let ty = format!("{B}PlaceFootprintFromLibrary"); let first = msg(ox, oy);
        let mut t = self.from_library(&ty, first).await.map_err(|e| format!("footprint {lib}: {e}"))?;
        if let (Some(a), Some(b)) = (pad1(old), pad1(&t)) { let (dx, dy) = (a.0 - b.0, a.1 - b.1); if dx.abs() > 1e-6 || dy.abs() > 1e-6 { let again = msg(ox + dx, oy + dy); t = self.from_library(&ty, again).await? } }
        let nets: std::collections::HashMap<String, Value> = old["definition"]["items"].as_array().into_iter().flatten().filter(|i| i["@type"].as_str().is_some_and(|t| t.ends_with(".Pad"))).map(|p| (p["number"].as_str().unwrap_or("").to_string(), p["net"].clone())).collect();
        let field = |i: &&Value| i["@type"].as_str().is_some_and(|t| t.ends_with(".Field"));
        let mut items: Vec<Value> = t["definition"]["items"].as_array().into_iter().flatten().filter(|i| !field(i)).cloned().collect();
        for p in items.iter_mut().filter(|i| i["@type"].as_str().is_some_and(|t| t.ends_with(".Pad"))) { if let Some(n) = nets.get(p["number"].as_str().unwrap_or("")) { p["net"] = n.clone() } }
        items.extend(old["definition"]["items"].as_array().into_iter().flatten().filter(field).cloned());
        let mut new = old.clone(); new["position"] = t["position"].clone(); if t["attributes"].is_object() { new["attributes"] = t["attributes"].clone() }
        let places = |fp: &Value| -> Vec<(String, f64, f64)> { fp["definition"]["items"].as_array().into_iter().flatten().filter(|i| i["@type"].as_str().is_some_and(|t| t.ends_with(".Pad"))).map(|p| (p["number"].as_str().unwrap_or("").to_string(), mm(&p["position"]["x_nm"]), mm(&p["position"]["y_nm"]))).collect() };
        let now = places(&t); for (num, x, y) in places(old) { if !now.iter().any(|n| n.0 == num && (n.1 - x).abs() < 1e-4 && (n.2 - y).abs() < 1e-4) { if let Some(net) = nets.get(&num).and_then(|n| n["name"].as_str()).filter(|n| !n.is_empty()) { self.vacated.push((net.to_string(), x, y)) } } }
        new["definition"] = t["definition"].clone(); new["definition"]["items"] = json!(items);
        Ok(new)
    }
    /// KiCad shows a commit's changes only once it ends: what reads the document after a change (a find, a route, a placement) ends the commit so far.
    async fn settle(&mut self, i: usize) -> Result<(), String> {
        if self.commit.is_some() { let m = self.message.clone(); self.end("CMA_COMMIT", &m).await?; self.split = i + 1; self.cache.clear() }
        Ok(())
    }
    /// The board as shapes (kroute.rs), each net's class, and the copper layers' names top to bottom.
    async fn model(&mut self) -> Result<(super::kroute::Board, Value), String> {
        let en = self.c.call(&format!("{B}GetBoardEnabledLayers"), json!({"board": self.doc})).await?;
        let enabled: Vec<String> = en["layers"].as_array().into_iter().flatten().filter_map(|l| l.as_str().map(str::to_string)).collect();
        let mut raw: Vec<Vec<Value>> = vec![];
        for k in ["footprint", "track", "arc", "via", "zone", "shape"] { raw.push(self.items(k).await?.into_iter().map(|(v, _)| v).collect()) }
        let rules = self.c.call(&format!("{B}GetBoardDesignRules"), json!({"board": self.doc})).await?;
        let nets = self.c.call(&format!("{B}GetNets"), json!({"board": self.doc})).await?;
        let names: Vec<Value> = nets["nets"].as_array().into_iter().flatten().filter(|n| n["name"].as_str().is_some_and(|s| !s.is_empty())).map(|n| json!({"name": n["name"]})).collect();
        let classes = self.c.call(&format!("{B}GetNetClassForNets"), json!({"net": names})).await?["classes"].clone();
        Ok((super::kroute::Board::from_raw(&enabled, &raw[0], &raw[1], &raw[2], &raw[3], &raw[4], &raw[5], &rules["rules"]["constraints"], &classes), classes))
    }
    /// Update raw items, each on its own sheet.
    async fn update(&mut self, kind: &str, items: Vec<(Value, Value, bool)>) -> Result<Value, String> {
        self.begin().await?; let n = items.len(); let mut by: Vec<(Value, Vec<Value>)> = vec![];
        let fields = items.iter().any(|(_, _, body)| *body) || kind == "symbol";
        let rows: Vec<Value> = items.iter().take(SHOWN).map(|(raw, _, _)| { let mut r = row(kind, raw, fields); if let Some(o) = r.as_object_mut() { o.remove("id"); } r }).collect();
        for (raw, sheet, body) in items {
            if kind == "zone" { self.refill.push(raw["id"].clone()) }
            let it = slim(kind, raw, body);
            match by.iter_mut().find(|(s, _)| *s == sheet) { Some((_, v)) => v.push(it), None => by.push((sheet, vec![it])) }
        }
        for (sheet, its) in by {
            let r = self.c.call(&format!("{C}UpdateItems"), json!({"header": self.header(&sheet), "items": its})).await?;
            if let Some(bad) = status_bad(&r, "updated_items") { return Err(bad) }
            self.deferred += r["updated_items"].as_array().into_iter().flatten().filter(|i| i["status"]["code"] == "ISC_DEFERRED").count();
        }
        Ok(if n <= SHOWN { json!(rows) } else { json!(n) })
    }
    async fn update_body(&mut self, kind: &str, items: Vec<(Value, Value)>) -> Result<Value, String> { self.update(kind, items.into_iter().map(|(r, s)| (r, s, true)).collect()).await }
    async fn op(&mut self, i: usize, op: &Value) -> Result<Value, String> {
        // what the check's `new` is about: the nets a route names, the parts placed, moved or set by reference, a new track's net; anything else
        // (a delete, a rule, a selector) has no such list, and then every new finding is this edit's
        match verb(op) { "find" | "route" | "place" => {}
            v @ ("move" | "set") => match &op[v] { Value::String(r) => self.touched.push(r.clone()), Value::Array(a) => self.touched.extend(a.iter().filter_map(|x| x.as_str().map(str::to_string))), _ => self.touch_all = true },
            "add" => for it in match &op["add"] { Value::Array(a) => a.clone(), o => vec![o.clone()] } { match it["track"]["net"].as_str().or(it["via"]["net"].as_str()).or(it["symbol"]["ref"].as_str()) { Some(n) if !n.is_empty() => self.touched.push(n.to_string()), _ => self.touch_all = true } },
            _ => self.touch_all = true }
        match verb(op) {
            "find" => {
                let sel = if let Some(k) = op["find"].as_str() { json!({"kind": k}) } else { selector(&op["find"], self.board) }; let kind = sel["kind"].as_str().unwrap_or("").to_string(); let limit = sel["limit"].as_u64().map_or(ROWS, |n| n as usize);
                if kind == "net" { let r = self.c.call(&format!("{B}GetNets"), json!({"board": self.doc})).await?;
                    let names: Vec<&str> = r["nets"].as_array().into_iter().flatten().filter_map(|n| n["name"].as_str()).filter(|n| !n.is_empty() && sel["name"].as_str().map_or(true, |p| glob(p, n))).collect(); return Ok(json!({"nets": names})) }
                // KiCad shows a commit's changes only once it ends: a find after a change ends the commit so far, and its rows are the document as changed
                self.settle(i).await?;
                let wf = sel["fields"].as_bool().unwrap_or(false);
                let rows: Vec<Value> = if kind == "pad" { let fsel = json!({"kind": "footprint", "ref": sel["ref"].clone()}); let fps = if sel["ref"].is_null() { self.items("footprint").await? } else { self.pick(&fsel).await? };
                        fps.iter().flat_map(|(f, _)| pad_rows(f)).filter(|r| matches(&sel, r)).collect() }
                    else if sel["raw"].as_bool().unwrap_or(false) { self.pick(&sel).await?.into_iter().map(|(raw, _)| raw).collect() }
                    else { self.pick(&sel).await?.iter().map(|(raw, _)| row(&kind, raw, wf)).collect() };
                let n = rows.len(); let (mut kept, mut size) = (vec![], 0usize);
                for r in rows.into_iter().take(limit) { size += r.to_string().len(); if size > FIND_CHARS && !kept.is_empty() { break } kept.push(r) }
                let shown = kept.len(); let mut out = json!({"rows": kept});
                if n > shown { out["more"] = json!(format!("{} more: narrow the selector (ref, net, layer, a smaller near){}", n - shown, if shown < limit.min(n) { "" } else { " or give limit" })) }
                Ok(out)
            }
            v @ ("move" | "set") => {
                let sel = selector(&op[v], self.board); let kind = sel["kind"].as_str().unwrap_or("").to_string();
                let mut picked = self.pick(&sel).await?;
                // a symbol this call added a moment ago is in the document only once its commit has ended
                let mine = match &sel["ref"] { Value::String(r) => self.planned.contains_key(r), Value::Array(a) => a.iter().filter_map(Value::as_str).any(|r| self.planned.contains_key(r)), _ => false };
                if mine && self.commit.is_some() { self.settle(i).await?; picked = self.pick(&sel).await? }
                if picked.is_empty() { return Err(format!("nothing matches {sel}")) }
                // every reference asked for by name must be there: a typo is an error, not a smaller edit
                if let Some(want) = sel["ref"].as_array() { for w in want.iter().filter_map(Value::as_str).filter(|w| !w.contains(['*', '?'])) { if !picked.iter().any(|(r, _)| row(&kind, r, false)["ref"] == json!(w)) { return Err(format!("no {kind} `{w}`")) } } }
                let mut with = op.as_object().map(|o| o.iter().filter(|(k, _)| if v == "move" { !matches!(k.as_str(), "move" | "by" | "to") } else { k.as_str() != "set" }).map(|(k, v)| (k.clone(), v.clone())).collect::<Map<_, _>>()).unwrap_or_default();
                if v == "move" { if let Some(to) = op.get("to") { with.insert("at".into(), to.clone()); } if op.get("by").is_none() && with.is_empty() { return Err("move takes by: [dx, dy], to: [x, y] and/or rot".into()) } }
                else if with.is_empty() { return Err("set takes the row keys to change (value, fields, at, rot, name, clearance, ...)".into()) }
                let mut ups = vec![];
                let swap = if kind == "footprint" { with.remove("footprint") } else { None };
                if let Some(lib) = &swap { let lib = lib.as_str().ok_or("footprint is Library:Footprint")?.to_string(); self.settle(i).await?; self.refill_all = true;
                    let mut done = vec![]; for (raw, sheet) in &picked { self.added.push(text3(&raw["reference_field"]).to_string()); done.push((self.swap(raw, &lib).await?, sheet.clone())) }
                    if with.is_empty() { let pads: Vec<Value> = done.iter().flat_map(|(f, _)| pad_rows(f)).map(|mut p| { if let Some(o) = p.as_object_mut() { o.remove("id"); o.remove("pin"); } p }).collect();
                        let n = done.len(); self.update_body("footprint", done).await?; return Ok(json!({"swapped": n, "footprint": lib, "pads": pads})) }
                    return Err("footprint goes in a set of its own (then set the other keys)".into()) }
                for (mut raw, sheet) in picked {
                    let mut w = with.clone();
                    if let Some((dx, dy)) = op.get("by").map(|b| pair(b).ok_or("by is [dx, dy] in mm")).transpose()? { let at = row(&kind, &raw, false); let k = if at.get("at").is_some() { "at" } else { return Err(format!("a {kind} moves with set: from/to")) };
                        let (x, y) = pair(&at[k]).unwrap_or((0.0, 0.0)); w.insert("at".into(), json!([x + dx, y + dy])); }
                    let body = apply(&kind, &mut raw, &Value::Object(w))?;
                    if kind == "symbol" { let r = text2(&raw["reference_field"]).to_string(); self.planned.remove(&r); if (v == "move" || with.contains_key("at") || with.contains_key("rot")) && !self.added.contains(&r) { self.added.push(r.clone()) }
                        if let Some(x) = with.get("value").and_then(Value::as_str) { self.expect.push((r.clone(), "Value".into(), x.into())) }
                        if let Some(x) = with.get("footprint").and_then(Value::as_str) { self.expect.push((r.clone(), "Footprint".into(), x.into())) }
                        for (n, t) in with.get("fields").and_then(Value::as_object).into_iter().flatten() { self.expect.push((r.clone(), n.clone(), t.as_str().unwrap_or("").into())) } }
                    ups.push((raw, sheet, body));
                }
                let before = self.deferred; let rows = self.update(&kind, ups).await?;
                // the person is editing some of these right now: KiCad applies the change when they finish, merged with theirs (GetNotices says how it went)
                Ok(if self.deferred > before { json!({"updated": rows, "deferred": self.deferred - before}) } else { json!({"updated": rows}) })
            }
            "add" => {
                let mut list = match &op["add"] { Value::Array(a) => a.clone(), o => vec![o.clone()] }; let mut note = Map::new();
                if !self.board {
                    // a symbol is placed by KiCad first and a pin is read from the document: both need the changes so far to have landed
                    if list.iter().any(|it| it["symbol"].is_object() || it.to_string().contains('.')) { self.settle(i).await? }
                    let (asked, pwr0) = (list.clone(), self.pwr); list = self.expand(op, asked.clone()).await?;
                    if list.iter().any(|it| it["symbol"].is_object()) {
                        // where it lands against what the sheet has: with fit: [x0, y0, x1, y1] the group moves, as one, on the 1.27 mm grid to the
                        // nearest free room inside that area; without, what it lies over is named
                        let sheet = self.sheet(&op["sheet"]).await?; let there = self.sheet_boxes(&sheet).await?;
                        let boxes: Vec<(f64, f64, f64, f64)> = list.iter().filter_map(|it| self.entry_box(it)).collect();
                        let hit = |b: &(f64, f64, f64, f64), o: &(f64, f64, f64, f64), dx: f64, dy: f64| b.0 + dx < o.2 && b.2 + dx > o.0 && b.1 + dy < o.3 && b.3 + dy > o.1;
                        let over = |dx: f64, dy: f64| -> Vec<&str> { let mut v: Vec<&str> = there.iter().filter(|(o, _)| boxes.iter().any(|b| hit(b, o, dx, dy))).map(|(_, w)| w.as_str()).collect(); v.dedup(); v };
                        let g = boxes.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, b| (m.0.min(b.0), m.1.min(b.1), m.2.max(b.2), m.3.max(b.3)));
                        // the area it must stay in: `fit`, else the sheet as far as it is drawn on (so it never leaves the page)
                        let given = op["fit"].as_array().filter(|a| a.len() == 4).map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect::<Vec<_>>());
                        let area = given.clone().unwrap_or_else(|| { let e = there.iter().fold(g, |m, (b, _)| (m.0.min(b.0), m.1.min(b.1), m.2.max(b.2), m.3.max(b.3))); vec![e.0, e.1, e.2, e.3] });
                        let inside = |dx: f64, dy: f64| g.0 + dx >= area[0] && g.1 + dy >= area[1] && g.2 + dx <= area[2] && g.3 + dy <= area[3];
                        let was = over(0.0, 0.0);
                        if op["fit"] != json!(false) && (!was.is_empty() || (given.is_some() && !inside(0.0, 0.0))) {
                            let n = (((area[2] - area[0]).max(area[3] - area[1]) / 1.27).ceil() as i64).clamp(1, 300); let mut best: Option<(f64, f64)> = None;
                            for ix in -n..=n { for iy in -n..=n { let (dx, dy) = (ix as f64 * 1.27, iy as f64 * 1.27); if best.map_or(false, |b| dx.abs() + dy.abs() >= b.0.abs() + b.1.abs()) || !inside(dx, dy) || !over(dx, dy).is_empty() { continue } best = Some((dx, dy)) } }
                            match best {
                                Some((dx, dy)) => { let r4 = |v: f64| (v * 1e4).round() / 1e4;
                                    let mv = |v: &mut Value| if let Some((x, y)) = pair(v) { *v = json!([r4(x + dx), r4(y + dy)]) };
                                    let mut moved = asked; for it in moved.iter_mut() { for k in ["symbol", "label"] { if it[k]["at"].is_array() { mv(&mut it[k]["at"]) } } for k in ["junction", "no_connect"] { if it[k].is_array() { mv(&mut it[k]) } }
                                        if let Some(w) = it["wire"].as_array_mut() { for q in w.iter_mut().filter(|q| q.is_array()) { mv(q) } } }
                                    note.insert("moved_to_free_room".into(), json!({"by": [r4(dx), r4(dy)], "it_lay_over": was.iter().take(4).collect::<Vec<_>>(), "now": "lies over nothing on the sheet"}));
                                    self.pwr = pwr0; list = self.expand(op, moved).await?; }
                                None if given.is_some() => return Err(format!("no free room for this group inside {area:?} (it is {:.1} x {:.1} mm): give a larger area or a smaller group", g.2 - g.0, g.3 - g.1)),
                                None => { note.insert("lies_over".into(), json!(was.iter().take(8).collect::<Vec<_>>())); } }
                        } else if was.is_empty() { note.insert("room".into(), json!("lies over nothing on the sheet")); } else { note.insert("lies_over".into(), json!(was.iter().take(8).collect::<Vec<_>>())); }
                    }
                }
                self.begin().await?; let mut ids = vec![]; let mut by: Vec<(Value, Vec<Value>)> = vec![]; let mut quiet = 0usize;
                for it in &list { let sheet = self.sheet(if it["sheet"].is_null() { &op["sheet"] } else { &it["sheet"] }).await?;
                    let raws = match self.templates.get(&it.to_string()) { Some(t) => vec![placed(t, &it["symbol"])], None => build(it)? }; match by.iter_mut().find(|(s, _)| *s == sheet) { Some((_, v)) => v.extend(raws), None => by.push((sheet, raws)) } }
                for (sheet, raws) in by {
                    let r = self.c.call(&format!("{C}CreateItems"), json!({"header": self.header(&sheet), "items": raws})).await?;
                    if let Some(bad) = status_bad(&r, "created_items") { return Err(bad) }
                    for i in r["created_items"].as_array().into_iter().flatten() { let k = kind_of(&i["item"]); if k.is_empty() { ids.push(json!({"id": i["item"]["id"]["value"]})); continue }
                        let mut rw = row(k, &i["item"], k == "symbol");
                        // a new part answers with where its pins are (what a wire joins); power symbols and the wires between are counted, not listed
                        if k == "symbol" { let rf = rw["ref"].as_str().unwrap_or("").to_string(); if rf.starts_with('#') { quiet += 1; continue }
                            let t = self.planned.get(&rf).cloned().unwrap_or(Value::Null); let ps = sym_pins(&t); if !ps.is_empty() && ps.len() <= 24 { rw["pins"] = Value::Object(ps.into_iter().map(|p| (p.0, json!([p.2 .0, p.2 .1]))).collect()) }
                            if let Some(o) = rw.as_object_mut() { o.remove("id"); o.remove("unit"); } self.added.push(rf) }
                        else if list.len() > 6 && matches!(k, "wire" | "junction" | "label") { quiet += 1; continue }
                        ids.push(rw) }
                }
                if self.board && list.iter().any(|it| it["track"].is_object() || it["via"].is_object()) { self.refill_all = true }
                let mut out = json!({"created": ids}); if quiet > 0 { out["and"] = json!(format!("{quiet} wires, labels and power symbols")) }
                for (k, v) in note { out[k.as_str()] = v }
                Ok(out)
            }
            "delete" => {
                let t = &op["delete"];
                let ids: Vec<(Value, Value)> = if t.is_object() { let sel = selector(t, self.board); if sel.as_object().is_some_and(|o| o.len() < 2) { return Err("delete needs more than a kind: say which (id, ref, net, near, ...)".into()) }
                        self.pick(&sel).await?.into_iter().map(|(raw, sheet)| (raw["id"].clone(), sheet)).collect() }
                    else { let sheet = self.sheet(&op["sheet"]).await?; match t { Value::Array(a) => a.iter().map(|i| (json!({"value": i}), sheet.clone())).collect(), i => vec![(json!({"value": i}), sheet)] } };
                if ids.is_empty() { return Err(format!("nothing matches {t}")) }
                self.begin().await?; let n = ids.len(); let mut by: Vec<(Value, Vec<Value>)> = vec![];
                for (id, sheet) in ids { match by.iter_mut().find(|(s, _)| *s == sheet) { Some((_, v)) => v.push(id), None => by.push((sheet, vec![id])) } }
                for (sheet, ids) in by {
                    let r = self.c.call(&format!("{C}DeleteItems"), json!({"header": self.header(&sheet), "item_ids": ids})).await?;
                    if let Some(bad) = r["deleted_items"].as_array().into_iter().flatten().find(|d| !matches!(d["status"].as_str().unwrap_or("IDS_OK"), "IDS_OK" | "IDS_DEFERRED")) { return Err(format!("{} {}", bad["status"].as_str().unwrap_or(""), bad["id"]["value"].as_str().unwrap_or(""))) }
                }
                Ok(json!({"deleted": n}))
            }
            "route" => {
                use super::kroute as kr;
                let pats: Vec<String> = match &op["route"] { Value::String(s) => vec![s.clone()], Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(), _ => vec![] };
                if pats.is_empty() || !self.board { return Err("route takes a board's net names: \"/LED*\" or [\"/SDA\", \"/SCL\"]".into()) }
                self.settle(i).await?; let (mut board, classes) = self.model().await?;
                let mut nets: Vec<String> = classes.as_object().into_iter().flatten().map(|(n, _)| n.clone()).filter(|n| pats.iter().any(|p| glob(p, n))).collect();
                let mut of_parts: Vec<String> = vec![];
                // a part ("R3") or one of its pads ("R3.2") stands for the nets on it
                let known = nets.clone(); for p in pats.iter().filter(|p| !known.iter().any(|n| glob(p, n))) { let (r, pad) = p.rsplit_once('.').map_or((p.as_str(), None), |(r, n)| (r, Some(n)));
                    let of: Vec<String> = board.items.iter().filter(|i| i.kind == kr::Kind::Pad && !i.net.is_empty() && ((i.of.0 == *p) || (i.of.0 == r && pad == Some(i.of.1.as_str())))).map(|i| i.net.clone()).collect();
                    if of.is_empty() { return Err(format!("no net or part matches `{p}`")) } for n in of { if !of_parts.contains(&n) { of_parts.push(n.clone()) } if !nets.contains(&n) { nets.push(n) } } }
                // a part named here has its pads made clean: another net's track that now lies on one of them (a footprint was exchanged, a part
                // moved) is taken up with its whole net and routed again
                let mut crossing: Vec<String> = vec![];
                for p in &pats { for pad in board.items.iter().filter(|i| i.kind == kr::Kind::Pad && i.of.0 == *p) { let need = board.net_clearance(&pad.net); let pts = pad.shape.samples(0.1);
                    for o in board.items.iter().filter(|o| matches!(o.kind, kr::Kind::Track | kr::Kind::Via) && !o.net.is_empty() && o.net != pad.net && o.layers & pad.layers != 0) {
                        if !crossing.contains(&o.net) && (pts.iter().any(|q| o.shape.dist(*q) < need - 1e-6) || pad.shape.dist(o.shape.anchor(pts[0])) < need - 1e-6) { crossing.push(o.net.clone()) } } } }
                for n in &crossing { if !nets.contains(n) { nets.push(n.clone()) } }
                // ripup: the nets' own tracks and vias go first, and they are routed afresh
                let mut ripped = 0usize; let rip: Vec<String> = if op["ripup"].as_bool().unwrap_or(false) { nets.clone() } else { crossing.clone() };
                if !rip.is_empty() { let nets = &rip; let mut ids = vec![];
                    for k in ["track", "arc", "via"] { for (raw, _) in self.items(k).await? { if raw["net"]["name"].as_str().is_some_and(|n| nets.iter().any(|x| x == n)) { ids.push(raw["id"].clone()) } } }
                    if !ids.is_empty() { ripped = ids.len(); self.begin().await?; self.refill_all = true; self.c.call(&format!("{C}DeleteItems"), json!({"header": self.header(&Value::Null), "item_ids": ids})).await?; }
                    board.items.retain(|it| !(matches!(it.kind, kr::Kind::Track | kr::Kind::Via) && nets.contains(&it.net))) }
                let layers = op["layers"].as_array().into_iter().flatten().map(|l| { let l = l.as_str().unwrap_or(""); board.layer(l).ok_or(format!("no copper layer `{l}`: {}", board.layers.join(", "))) }).collect::<Result<Vec<_>, _>>()?;
                let via = op["via"]["size"].as_f64().zip(op["via"]["drill"].as_f64()); let vias = op["vias"].as_bool().unwrap_or(true);
                let width = |n: &str| op["width"].as_f64().unwrap_or_else(|| mm(&classes[n]["board"]["track_width"]["value_nm"]));
                // short nets first; a net that finds no way goes first in the next try (three at most), and the try with the fewest left over stays
                let span = |n: &String| { let bb = board.items.iter().filter(|i| i.net == *n).map(|i| i.shape.bbox()).fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, x| (m.0.min(x.0), m.1.min(x.1), m.2.max(x.2), m.3.max(x.3))); (bb.2 - bb.0).hypot(bb.3 - bb.1) };
                nets.sort_by(|a, z| span(a).partial_cmp(&span(z)).unwrap_or(std::cmp::Ordering::Equal));
                let ws: Vec<f64> = nets.iter().map(|n| width(n)).collect(); if let Some(k) = ws.iter().position(|w| *w <= 0.0) { return Err(format!("{} has no track width: give width", nets[k])) }
                let wof: std::collections::HashMap<String, f64> = nets.iter().cloned().zip(ws).collect(); let copper = board.layers.clone();
                let stands: Vec<kr::Item> = board.items.iter().filter(|i| matches!(i.kind, kr::Kind::Pad | kr::Kind::Via) && self.vacated.iter().any(|v| v.0 == i.net)).cloned().collect();
                type Res = Vec<(String, Result<kr::Routed, (kr::Routed, String)>)>;
                let (res, wof) = tokio::task::spawn_blocking(move || { let mut order = nets; let mut best: Option<Res> = None;
                    let bad = |r: &Res| r.iter().filter(|x| x.1.is_err()).count();
                    for _ in 0..4 { let mut b = board.clone();
                        // a net with no way on the signal layers is tried once more with the plane layers too
                        let res: Res = order.iter().map(|n| { let snap = b.clone(); let o = kr::Opts { width: wof[n], layers: layers.clone(), vias, via, planes: false };
                            let r = match kr::route_net(&mut b, n, &o) { Err(_) if layers.is_empty() && b.planes != 0 => { b = snap; kr::route_net(&mut b, n, &kr::Opts { planes: true, ..o }) } r => r }; (n.clone(), r) }).collect();
                        let n = bad(&res);
                        let mut next: Vec<String> = res.iter().filter(|x| x.1.is_err()).map(|x| x.0.clone()).collect(); next.extend(res.iter().filter(|x| x.1.is_ok()).map(|x| x.0.clone()));
                        if best.as_ref().map_or(true, |x| n < bad(x)) { best = Some(res) }
                        if order.len() < 2 { break }
                        if n == 0 || next == order { break } order = next }
                    (best.unwrap_or_default(), wof) }).await.map_err(|e| format!("the router stopped: {e}"))?;
                let (mut items, mut routed, mut failed) = (vec![], vec![], vec![]); self.touched.extend(wof.keys().cloned());
                for (net, r) in res { let (r, why) = match r { Ok(r) => (r, None), Err((r, e)) => (r, Some(e)) };
                    for (a, z, l) in &r.tracks { items.push(json!({"@type": format!("{BT}Track"), "start": to_xy(a.0, a.1), "end": to_xy(z.0, z.1), "width": {"value_nm": nm(wof[&net])}, "layer": layer_enum(&copper[*l]), "net": {"name": net}})) }
                    for (p, s) in &r.vias { let (a, z) = (layer_enum(&copper[s.from]), layer_enum(&copper[s.to]));
                        // like one of the board's own vias of that kind where it has one (its filling, capping and tenting), else a plain one
                        let mut v = if s.like.is_object() && via.is_none() { let mut v = s.like.clone(); if let Some(o) = v.as_object_mut() { o.remove("id"); o.remove("parent"); } v }
                            else { json!({"@type": format!("{BT}Via"), "type": s.ty, "pad_stack": {"type": "PST_NORMAL", "layers": [a, z], "drill": {"start_layer": a, "end_layer": z, "diameter": to_xy(s.drill, s.drill), "shape": "DS_CIRCLE"}, "copper_layers": [{"layer": "BL_F_Cu", "shape": "PSS_CIRCLE", "size": to_xy(s.size, s.size)}]}}) };
                        v["position"] = to_xy(p.0, p.1); v["net"] = json!({"name": net}); items.push(v) }
                    let mut ls: Vec<&str> = r.tracks.iter().map(|t| copper[t.2].as_str()).collect(); ls.sort(); ls.dedup();
                    let row = json!({"net": net, "width": wof[&net], "length": (r.length * 1000.0).round() / 1000.0, "vias": r.vias.len(), "layers": ls});
                    match why { None => routed.push(row), Some(e) => failed.push(json!({"net": net, "why": e})) } }
                // a part's nets (its pads moved: a footprint was exchanged): what is left of an old track past where it still joins something is
                // cut back to there, or taken away when it joins nothing (KiCad calls it a dangling track)
                let mut trimmed = 0usize;
                if !self.vacated.is_empty() {
                    let mut open: Vec<(String, kr::P)> = self.vacated.iter().map(|v| (v.0.clone(), kr::P(v.1, v.2))).collect();
                    let pt = |v: &Value| kr::P(mm(&v["x_nm"]), mm(&v["y_nm"]));
                    let mut old: Vec<(Value, kr::P, kr::P, f64, String, bool)> = self.items("track").await?.into_iter().map(|(r, _)| r).filter(|r| r["net"]["name"].as_str().is_some_and(|n| self.vacated.iter().any(|x| x.0 == n) && !rip.iter().any(|x| x == n)))
                        .map(|r| { let (a, z, w, l) = (pt(&r["start"]), pt(&r["end"]), mm(&r["width"]["value_nm"]), format!("{}|{}", r["net"]["name"].as_str().unwrap_or(""), r["layer"].as_str().unwrap_or(""))); (r, a, z, w, l, true) }).collect();
                    let new: Vec<(kr::P, kr::P, f64, String)> = items.iter().filter(|i| i["@type"].as_str().is_some_and(|t| t.ends_with(".Track"))).map(|i| (pt(&i["start"]), pt(&i["end"]), mm(&i["width"]["value_nm"]), format!("{}|{}", i["net"]["name"].as_str().unwrap_or(""), i["layer"].as_str().unwrap_or("")))).collect();
                    let vias: Vec<(kr::P, f64, String)> = items.iter().filter(|i| i["@type"].as_str().is_some_and(|t| t.ends_with(".Via"))).map(|i| (pt(&i["position"]), mm(&i["pad_stack"]["copper_layers"][0]["size"]["x_nm"]) / 2.0, i["net"]["name"].as_str().unwrap_or("").to_string())).collect();
                    let (mut gone, mut cut): (Vec<Value>, Vec<Value>) = (vec![], vec![]);
                    for _ in 0..40 { let mut changed = false;
                        for k in 0..old.len() { if !old[k].5 { continue } let (a, z, w, key) = (old[k].1, old[k].2, old[k].3, old[k].4.clone()); let net = key.split('|').next().unwrap_or("").to_string();
                            // only a track with an end where a pad stood (or where a track taken away for that ended)
                            let ends: Vec<bool> = [a, z].iter().map(|e| open.iter().any(|o| o.0 == net && o.1.dist(*e) < 0.02)).collect(); if !ends[0] && !ends[1] { continue } let li = copper.iter().position(|c| layer_enum(c) == key.split('|').nth(1).unwrap_or(""));
                            // where along it (0 at its start, 1 at its end) something of its net joins it: another track's end, a pad, a via
                            let mut at: Vec<f64> = vec![]; let len = a.dist(z).max(1e-9); let along = |q: kr::P| (((q.0 - a.0) * (z.0 - a.0) + (q.1 - a.1) * (z.1 - a.1)) / (len * len)).clamp(0.0, 1.0);
                            for (j, o) in old.iter().enumerate() { if j != k && o.5 && o.4 == key { for q in [o.1, o.2] { if kr::seg_dist(q, a, z) <= w / 2.0 + 1e-3 { at.push(along(q)) } } for (q, t) in [(a, 0.0), (z, 1.0)] { if kr::seg_dist(q, o.1, o.2) <= o.3 / 2.0 + 1e-3 { at.push(t) } } } }
                            for o in new.iter().filter(|o| o.3 == key) { for q in [o.0, o.1] { if kr::seg_dist(q, a, z) <= w / 2.0 + 1e-3 { at.push(along(q)) } } for (q, t) in [(a, 0.0), (z, 1.0)] { if kr::seg_dist(q, o.0, o.1) <= o.2 / 2.0 + 1e-3 { at.push(t) } } }
                            for s in stands.iter().filter(|s| s.net == net && li.map_or(true, |l| s.layers & 1 << l != 0)) { let c = s.shape.anchor(a); let n = kr::P(a.0 + (z.0 - a.0) * along(c), a.1 + (z.1 - a.1) * along(c)); if s.shape.dist(n) <= 1e-3 { at.push(along(c)) } for (q, t) in [(a, 0.0), (z, 1.0)] { if s.shape.dist(q) <= 1e-3 { at.push(t) } } }
                            for (c, r, vn) in vias.iter().filter(|v| v.2 == net) { if kr::seg_dist(*c, a, z) <= *r { at.push(along(*c)) } }
                            let (lo, hi) = at.iter().fold((f64::MAX, f64::MIN), |m, t| (m.0.min(*t), m.1.max(*t)));
                            // (the end that stood on the pad is the one cut back; the other end stays as it is)
                            let (lo, hi) = (if ends[0] { lo } else { 0.0 }, if ends[1] { hi } else { 1.0 });
                            if at.is_empty() || (hi - lo) * len < 0.01 { old[k].5 = false; gone.push(old[k].0["id"].clone()); changed = true; for (e, was) in [(a, ends[0]), (z, ends[1])] { if !was { open.push((net.clone(), e)) } } continue }
                            if (ends[0] && lo * len > 0.01) || (ends[1] && (1.0 - hi) * len > 0.01) {
                                let (na, nz) = (kr::P(a.0 + (z.0 - a.0) * lo, a.1 + (z.1 - a.1) * lo), kr::P(a.0 + (z.0 - a.0) * hi, a.1 + (z.1 - a.1) * hi));
                                old[k].1 = na; old[k].2 = nz; old[k].0["start"] = to_xy(na.0, na.1); old[k].0["end"] = to_xy(nz.0, nz.1); cut.retain(|c: &Value| c["id"] != old[k].0["id"]); cut.push(old[k].0.clone()); changed = true }
                        }
                        if !changed { break } }
                    cut.retain(|c| !gone.contains(&c["id"])); trimmed = gone.len() + cut.len(); self.vacated.clear();
                    if trimmed > 0 { self.begin().await?; self.refill_all = true;
                        if !cut.is_empty() { let r = self.c.call(&format!("{C}UpdateItems"), json!({"header": self.header(&Value::Null), "items": cut})).await?; if let Some(bad) = status_bad(&r, "updated_items") { return Err(bad) } }
                        if !gone.is_empty() { self.c.call(&format!("{C}DeleteItems"), json!({"header": self.header(&Value::Null), "item_ids": gone})).await?; } }
                }
                // a new track that ends part way along another of its net: that one is broken there, as KiCad's own router breaks a track at a
                // junction (an end on the middle of a track is a dangling end to KiCad's check)
                let mut broken: Vec<Value> = vec![];
                if !items.is_empty() {
                    let pt = |v: &Value| kr::P(mm(&v["x_nm"]), mm(&v["y_nm"])); let is_track = |i: &Value| i["@type"].as_str().is_some_and(|t| t.ends_with(".Track"));
                    let ends: Vec<(kr::P, String, String)> = items.iter().filter(|i| is_track(i)).flat_map(|i| [(pt(&i["start"]), i["net"]["name"].as_str().unwrap_or("").to_string(), i["layer"].as_str().unwrap_or("").to_string()), (pt(&i["end"]), i["net"]["name"].as_str().unwrap_or("").to_string(), i["layer"].as_str().unwrap_or("").to_string())]).collect();
                    let mut lines: Vec<(Value, bool)> = self.items("track").await?.into_iter().map(|(r, _)| (r, true)).filter(|(r, _)| ends.iter().any(|e| r["net"]["name"] == e.1.as_str())).collect();
                    lines.extend(items.iter().filter(|i| is_track(i)).cloned().map(|i| (i, false)));
                    let mut k = 0; let mut extra: Vec<Value> = vec![]; let mut changed: Vec<usize> = vec![];
                    while k < lines.len() { let (a, z) = (pt(&lines[k].0["start"]), pt(&lines[k].0["end"]));
                        let hit = ends.iter().find(|e| lines[k].0["net"]["name"] == e.1.as_str() && lines[k].0["layer"] == e.2.as_str() && kr::seg_dist(e.0, a, z) < 1e-3 && e.0.dist(a) > 0.01 && e.0.dist(z) > 0.01).map(|e| e.0);
                        match hit { Some(m) => { let mut rest = lines[k].0.clone(); if let Some(o) = rest.as_object_mut() { o.remove("id"); } rest["start"] = to_xy(m.0, m.1); lines[k].0["end"] = to_xy(m.0, m.1);
                                if lines[k].1 && !changed.contains(&k) { changed.push(k) } lines.push((rest.clone(), false)); extra.push(rest) }
                            None => k += 1 } }
                    broken = changed.iter().map(|k| lines[*k].0.clone()).collect();
                    // the new tracks as broken (those of this call), and the far halves of the old ones
                    let n_new = items.iter().filter(|i| is_track(i)).count(); let old_n = lines.len() - n_new - extra.len();
                    let mut fresh: Vec<Value> = lines[old_n..old_n + n_new].iter().map(|l| l.0.clone()).collect(); fresh.extend(extra); fresh.extend(items.iter().filter(|i| !is_track(i)).cloned()); items = fresh;
                }
                if !items.is_empty() { self.begin().await?; self.refill_all = true;
                    if !broken.is_empty() { let r = self.c.call(&format!("{C}UpdateItems"), json!({"header": self.header(&Value::Null), "items": broken})).await?; if let Some(bad) = status_bad(&r, "updated_items") { return Err(bad) } }
                    let r = self.c.call(&format!("{C}CreateItems"), json!({"header": self.header(&Value::Null), "items": items})).await?; if let Some(bad) = status_bad(&r, "created_items") { return Err(bad) } }
                // nets that were already whole are counted, not listed
                let whole = routed.iter().filter(|r| r["length"] == 0.0).count(); routed.retain(|r| r["length"] != 0.0);
                let mut out = json!({"routed": routed}); if ripped > 0 { out["ripped_up"] = json!(ripped) } if !crossing.is_empty() { out["rerouted_off_the_pads"] = json!(crossing) } if trimmed > 0 { out["old_stubs_trimmed"] = json!(trimmed) } if whole > 0 { out["already_connected"] = json!(whole) } if !failed.is_empty() { out["failed"] = json!(failed) }
                Ok(out)
            }
            "place" => {
                use super::kroute as kr;
                let pairs: Vec<(String, String)> = match &op["place"] { Value::Object(o) => o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect(), Value::String(s) => vec![(s.clone(), op["near"].as_str().unwrap_or("").to_string())], _ => vec![] };
                if pairs.is_empty() || !self.board { return Err("place takes a board's parts and the pin each goes next to: {place: {\"C5\": \"U1.26\"}, within: 2}".into()) }
                let within = op["within"].as_f64().unwrap_or(2.0); self.settle(i).await?; let (mut board, _) = self.model().await?;
                let fps = self.items("footprint").await?; let fp = |r: &str| fps.iter().find(|(f, _)| text3(&f["reference_field"]) == r).map(|(f, _)| f.clone()).ok_or(format!("no footprint `{r}`"));
                let norm = |d: f64| { let x = d.rem_euclid(360.0); if x > 180.0 { x - 360.0 } else { x } };
                let (mut ups, mut rows) = (vec![], vec![]);
                for (r, target) in pairs {
                    let (tref, tpad) = target.rsplit_once('.').ok_or(format!("{r}: the pin it goes next to is REF.PAD, e.g. U1.26"))?; self.touched.push(r.clone());
                    let t = board.items.iter().find(|i| i.kind == kr::Kind::Pad && i.of.0 == tref && i.of.1 == tpad).ok_or(format!("no pad `{target}`"))?.clone();
                    let mut raw = fp(&r)?; let at = kr::P(mm(&raw["position"]["x_nm"]), mm(&raw["position"]["y_nm"])); let rot = raw["orientation"]["value_degrees"].as_f64().unwrap_or(0.0);
                    let trot = fp(tref)?["orientation"]["value_degrees"].as_f64().unwrap_or(0.0);
                    let mine: Vec<&kr::Item> = board.items.iter().filter(|i| i.kind == kr::Kind::Pad && i.of.0 == r).collect();
                    let pad = match op["pad"].as_str() { Some(p) => p.to_string(), None => mine.iter().find(|i| !t.net.is_empty() && i.net == t.net).map(|i| i.of.1.clone()).ok_or(format!("{r} has no pad on {target}'s net `{}`: say which with pad", t.net))? };
                    if mine.iter().any(|i| i.layers & t.layers == 0) { return Err(format!("{r} is not on {target}'s side of the board: flip it first (ipc FlipItems)")) }
                    let turns: Vec<f64> = (0..4).map(|k| norm(trot + 90.0 * k as f64 - rot)).collect();
                    let (by, turn, d) = kr::place(&board, &r, &pad, at, t.shape.anchor(at), within, &turns)?;
                    for it in board.items.iter_mut().filter(|i| i.kind == kr::Kind::Pad && i.of.0 == r) { it.shape = it.shape.moved(at, turn, by) }
                    for c in board.courtyards.iter_mut().filter(|c| c.0 == r) { if let kr::Shape::Poly(v) = kr::Shape::Poly(c.2.clone()).moved(at, turn, by) { c.2 = v } }
                    let (x, y, nr) = (((at.0 + by.0) * 1e4).round() / 1e4, ((at.1 + by.1) * 1e4).round() / 1e4, norm(rot + turn));
                    apply("footprint", &mut raw, &json!({"at": [x, y], "rot": nr}))?;
                    rows.push(json!({"ref": r, "at": [x, y], "rot": nr, "layer": layer_name(raw["layer"].as_str().unwrap_or("")), "pad": pad, "near": target, "distance": (d * 1000.0).round() / 1000.0})); ups.push((raw, Value::Null, false));
                }
                self.update("footprint", ups).await?; self.refill_all = true;
                Ok(json!({"placed": rows}))
            }
            "netclass" => { self.wrote = true; for (ty, msg) in netclass_msgs(&op["netclass"], &self.doc["project"])? { self.c.call(&ty, msg).await?; }
                // answered with the class as the project now has it
                let name = op["netclass"]["name"].clone(); let p = json!({"project": self.doc["project"]});
                let (cs, asg) = (self.c.call(&format!("{C}GetNetClasses"), p.clone()).await.unwrap_or_default(), self.c.call(&format!("{C}GetNetClassAssignments"), p).await.unwrap_or_default());
                Ok(json!({"netclass": netclass_row(&name, &cs, &asg)})) }
            "rule" => { self.wrote = true; let list = match &op["rule"] { Value::Array(a) => a.clone(), o => vec![o.clone()] };
                let (rm, up): (Vec<&Value>, Vec<&Value>) = list.iter().partition(|r| r["remove"].is_string());
                let r = self.c.call("kiapi.backplane.coedit.EditCustomRules", json!({"board": self.doc, "upsert": up.into_iter().map(rule_msg).collect::<Result<Vec<_>, _>>()?, "remove": rm.iter().map(|r| r["remove"].clone()).collect::<Vec<_>>()})).await?;
                if r["status"] == "CRS_INVALID" { return Err(format!("the rules do not parse: {}", r["error_text"].as_str().unwrap_or(""))) }
                Ok(json!({"rules": r["rules"].as_array().into_iter().flatten().filter_map(|x| x["name"].as_str()).collect::<Vec<_>>()})) }
            "refill" => { self.wrote = true; self.end("CMA_COMMIT", "").await?; let zs = match &op["refill"] { Value::Array(a) => a.iter().map(|i| json!({"value": i})).collect(), _ => vec![] };
                self.c.call(&format!("{B}RefillZones"), json!({"board": self.doc, "zones": zs})).await?; Ok(json!({"refilled": true})) }
            "ipc" => { let ty = full_type(op["ipc"]["type"].as_str().ok_or("ipc is {type, message}")?); let mut msg = op["ipc"]["message"].clone(); if msg.is_null() { msg = json!({}) } fill(&mut msg, &self.doc);
                if !super::ipc::is_read(&ty) { self.wrote = true; if matches!(ty.rsplit('.').next(), Some("CreateItems" | "UpdateItems" | "DeleteItems")) { self.begin().await? } }
                Ok(brief(self.c.call(&ty, msg).await?)) }
            _ => Err("an op is one of find, move, set, add, delete, route, place, netclass, rule, refill, ipc".into()),
        }
    }
}

/// The whole edit: `{path, ops: [...], message?, save?}` -> `{results: [...], saved}`. An op that fails drops the commit: nothing of this
/// call's item changes stays (net classes, rules and raw `ipc` commands that already ran are named in the error).
pub async fn run(c: &dyn Call, path: &str, a: &Value) -> Result<Value, String> {
    let ops = match &a["ops"] { Value::Array(o) if !o.is_empty() => o.clone(), _ => return Err("ops is a list of edits: find, move, set, add, delete, route, place, netclass, rule, refill, ipc".into()) };
    if let Some((i, _)) = ops.iter().enumerate().find(|(_, o)| verb(o).is_empty()) { return Err(format!("op {}: an op is one of find, move, set, add, delete, route, place, netclass, rule, refill, ipc", i + 1)) }
    let patient = Patient { c, step: BUSY_STEP, wait: BUSY_WAIT }; let c: &dyn Call = &patient;
    let doc = Run::open(c, path).await?;
    let mut r = Run { c, expect: vec![], deferred: 0, refill_all: false, planned: Default::default(), added: vec![], libs: false, pwr: None, touched: vec![], touch_all: false, vacated: vec![], lib_step: std::time::Duration::from_millis(if cfg!(test) { 1 } else { 500 }), board: path.ends_with(".kicad_pcb"), doc, commit: None, sheets: None, refill: vec![], wrote: false, cache: Default::default(),
        message: a["message"].as_str().unwrap_or("kicad_edit").to_string(), split: 0, templates: Default::default() };
    // KiCad's own check before the edit, so the answer can say what the edit added to it (check: false leaves it out)
    let checking = writes(&a["ops"]) && a["save"].as_bool().unwrap_or(true) && a["check"].as_bool().unwrap_or(true);
    let before = if checking { c.check().await.unwrap_or(Value::Null) } else { Value::Null };

    let mut results = vec![];
    for (i, op) in ops.iter().enumerate() {
        match r.op(i, op).await {
            Ok(v) => results.push(v),
            Err(e) => {
                let _ = r.end("CMA_DROP", "").await;
                let ran: Vec<&str> = ops[..i].iter().map(verb).filter(|v| matches!(*v, "netclass" | "rule" | "refill" | "ipc")).collect();
                let ran = if ran.is_empty() { String::new() } else { format!(" (already applied, not undone: {})", ran.join(", ")) };
                // a find after a change ended that commit: what came before it stays, and is saved so the file and the document agree
                if r.split > 0 && a["save"].as_bool().unwrap_or(true) {
                    let _ = r.c.call(&format!("{C}SaveDocument"), json!({"document": r.doc})).await;
                    return Err(format!("op {} ({}): {e}. The changes before op {}'s find were already committed and are saved; the later ones were dropped{ran}.", i + 1, verb(op), r.split));
                }
                return Err(format!("op {} ({}): {e}. Nothing was saved; this call's item changes were dropped{ran}.", i + 1, verb(op)));
            }
        }
    }
    let message = r.message.clone();
    r.end("CMA_COMMIT", &message).await?;
    if r.refill_all { r.refill.clear(); r.c.call(&format!("{B}RefillZones"), json!({"board": r.doc, "zones": []})).await?; }
    if !r.refill.is_empty() { let zs = std::mem::take(&mut r.refill); r.c.call(&format!("{B}RefillZones"), json!({"board": r.doc, "zones": zs})).await?; }
    let mut out = json!({"results": results});
    if r.wrote && a["save"].as_bool().unwrap_or(true) {
        let s = r.c.call(&format!("{C}SaveDocument"), json!({"document": r.doc})).await?;
        // the save gate's word (nothing to write; only KiCad's own load change) is not a save; a save says what else it wrote
        if s["not_saved"].is_string() { out["saved"] = json!(false); out["not_saved"] = s["not_saved"].clone() } else { out["saved"] = json!(path) }
        for k in ["format_upgrade", "kicad_load_changes"] { if s[k].is_string() { out[k] = s[k].clone() } }
        // what was set on symbols is read back from the saved sheet files, never taken on the API's word (a sub-sheet's symbol most of all)
        if out["saved"].is_string() && r.deferred == 0 { if let Some(texts) = r.c.sheet_texts() {
            let wrong: Vec<String> = r.expect.iter().filter(|(rf, n, v)| !file_prop(&texts, rf, n, v)).map(|(rf, n, _)| format!("{rf} {n}")).collect();
            if !wrong.is_empty() { return Err(format!("saved, but the schematic's files do not show: {}. KiCad did not take those; read them with find and set them again.", wrong.join(", "))) }
        } }
        // the parts this call added, with the net each pin is now on (the saved schematic's own netlist)
        if out["saved"].is_string() && !r.board && !r.added.is_empty() { if let Ok(all) = r.c.nets().await { let m: Map<String, Value> = r.added.iter().filter(|a| all[a.as_str()].is_object()).map(|a| (a.clone(), all[a.as_str()].clone())).collect(); if !m.is_empty() { out["nets"] = Value::Object(m) } } }
        if out["saved"].is_string() && !before.is_null() { if let Ok(now) = r.c.check().await { if !now.is_null() { out["check"] = check_delta(&before, &now, if r.touch_all || !r.board { &[] } else { &r.touched }) } } }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test] fn units_and_names() {
        assert_eq!(mm(&json!("157250000")), 157.25); assert_eq!(nm(98.5), json!("98500000")); assert_eq!(nm(-0.000001), json!("-1"));
        assert_eq!((layer_name("BL_In1_Cu"), layer_enum("Edge.Cuts"), layer_enum("BL_F_Cu")), ("In1.Cu".into(), "BL_Edge_Cuts".into(), "BL_F_Cu".into()));
        assert!(glob("/U*D+", "/U12D+") && glob("R?", "R1") && !glob("R?", "R10") && glob("*", "") && !glob("C1", "C10") && glob("LOGO*", "LOGO4"));
        assert_eq!(full_type("kiapi.board.commands.CreateItems"), "kiapi.common.commands.CreateItems");
        assert_eq!((full_type("RefillZones"), full_type("EditCustomRules"), full_type("kiapi.x.Unknown")), ("kiapi.board.commands.RefillZones".into(), "kiapi.backplane.coedit.EditCustomRules".into(), "kiapi.x.Unknown".into()));
    }
    fn fp(r: &str, v: &str, x: i64, y: i64) -> Value {
        json!({"id": {"value": format!("id-{r}")}, "position": {"x_nm": x.to_string(), "y_nm": y.to_string()}, "orientation": {"value_degrees": -90.0}, "layer": "BL_B_Cu",
            "reference_field": {"text": {"text": {"text": r}}}, "value_field": {"text": {"text": {"text": v}}},
            "definition": {"items": [{"@type": format!("{BT}Field"), "name": "MPN", "text": {"text": {"text": "X1"}}}, {"@type": format!("{BT}Pad"), "id": {"value": "p1"}, "number": "1", "net": {"name": "GND"}, "position": {"x_nm": "1000000", "y_nm": "2000000"}}]}})
    }
    #[test] fn rows_are_short_and_selectors_match_them() {
        let f = fp("R1", "10k", 150400000, 96750000); let r = row("footprint", &f, true);
        assert_eq!(r, json!({"id": "id-R1", "ref": "R1", "value": "10k", "at": [150.4, 96.75], "rot": -90.0, "layer": "B.Cu", "fields": {"MPN": "X1"}}));
        assert!(row("footprint", &f, false).to_string().len() < 100 && row("footprint", &f, false).get("fields").is_none());
        for (sel, want) in [(json!({"ref": "R*"}), true), (json!({"ref": ["C1", "R1"]}), true), (json!({"ref": "R2"}), false), (json!({"layer": "B.Cu", "value": "10k"}), true), (json!({"near": [150.0, 96.75, 0.5]}), true),
            (json!({"near": [150.0, 96.75, 0.3]}), false), (json!({"at": [150.4, 96.75]}), true), (json!({"rot": -90}), true), (json!({"net": "GND"}), false), (json!({"kind": "footprint", "fields": true, "limit": 3}), true)] {
            assert_eq!(matches(&sel, &r), want, "{sel}");
        }
        assert_eq!(pad_rows(&f), vec![json!({"id": "p1", "ref": "R1", "pad": "1", "net": "GND", "at": [1.0, 2.0], "pin": ""})]);
        let z = json!({"id": {"value": "z"}, "layers": ["BL_B_Cu"], "copper_settings": {"net": {"name": "+1V8"}, "clearance": {"value_nm": "150000"}, "min_thickness": {"value_nm": "150000"}}, "outline": {"polygons": [{"outline": {"nodes": [1, 2, 3]}}]}, "filled_polygons": [1]});
        assert_eq!(row("zone", &z, false), json!({"id": "z", "name": "", "net": "+1V8", "layers": ["B.Cu"], "priority": 0, "clearance": 0.15, "min_width": 0.15, "corners": 3, "type": ""}));
        assert!(matches(&json!({"layers": ["B.Cu"]}), &row("zone", &z, false)) && matches(&json!({"layers": "B.Cu"}), &json!({"layers": ["B.Cu", "F.Cu"]})) && !matches(&json!({"layers": ["B.Cu"]}), &json!({"layers": ["B.Cu", "F.Cu"]})));
    }
    #[test] fn set_changes_only_what_it_names() {
        let mut f = fp("R1", "10k", 1000000, 2000000);
        assert_eq!(apply("footprint", &mut f, &json!({"at": [3, 4.5], "rot": 90, "value": "10k 1%"})), Ok(false));
        assert_eq!((f["position"].clone(), f["orientation"]["value_degrees"].clone(), text3(&f["value_field"]), f["layer"].clone()), (json!({"x_nm": "3000000", "y_nm": "4500000"}), json!(90.0), "10k 1%", json!("BL_B_Cu")));
        assert_eq!(apply("footprint", &mut f, &json!({"fields": {"MPN": "RC0603", "LCSC": "C1"}})), Ok(true));
        let its = f["definition"]["items"].as_array().unwrap(); assert_eq!((its.len(), text3(&its[0]), its[2]["text"]["layer"].clone(), text3(&its[2])), (3, "RC0603", json!("BL_B_Fab"), "C1"));
        assert!(slim("footprint", f.clone(), false).get("definition").is_none() && slim("footprint", f.clone(), true).get("definition").is_some());
        let mut z = json!({"copper_settings": {"clearance": {"value_nm": "150000"}, "island_mode": "IRM_ALWAYS"}, "filled_polygons": [1]});
        apply("zone", &mut z, &json!({"clearance": 0.2, "min_width": 0.25, "name": "PWR", "raw": {"copper_settings": {"island_mode": "IRM_NEVER"}}})).unwrap();
        assert_eq!(slim("zone", z, false), json!({"name": "PWR", "copper_settings": {"clearance": {"value_nm": "200000"}, "min_thickness": {"value_nm": "250000"}, "island_mode": "IRM_NEVER"}}));
        let mut s = json!({"position": {"x_nm": "1", "y_nm": "2"}, "value_field": {"text": {"text": "10K", "attributes": {"visible": true}}}, "user_fields": [{"name": "MPN", "text": {"text": "old"}}], "definition": {"big": 1}});
        apply("symbol", &mut s, &json!({"value": "10k", "fields": {"MPN": "new", "Tol": "1%"}})).unwrap();
        assert_eq!((text2(&s["value_field"]), s["value_field"]["text"]["attributes"]["visible"].clone(), text2(&s["user_fields"][0]), text2(&s["user_fields"][1]), s["user_fields"][1]["visible"].clone()), ("10k", json!(true), "new", "1%", json!(false)));
        assert!(slim("symbol", s, false).get("definition").is_none());
        assert!(apply("track", &mut json!({}), &json!({"colour": "red"})).unwrap_err().contains("no settable `colour`"));
    }
    #[test] fn add_builds_kicads_items() {
        let t = build(&json!({"track": {"from": [157.25, 100.25], "to": [157.25, 98.5], "width": 0.25, "layer": "B.Cu", "net": "GND"}})).unwrap();
        assert_eq!(t, vec![json!({"@type": "type.googleapis.com/kiapi.board.types.Track", "start": {"x_nm": "157250000", "y_nm": "100250000"}, "end": {"x_nm": "157250000", "y_nm": "98500000"}, "width": {"value_nm": "250000"}, "layer": "BL_B_Cu", "net": {"name": "GND"}})]);
        let v = &build(&json!({"via": {"at": [1, 2], "size": 0.6, "drill": 0.3, "net": "GND"}})).unwrap()[0];
        assert_eq!((v["type"].clone(), v["pad_stack"]["drill"]["diameter"].clone(), v["pad_stack"]["copper_layers"][0]["size"]["x_nm"].clone(), v["pad_stack"]["layers"].clone()), (json!("VT_THROUGH"), json!({"x_nm": "300000", "y_nm": "300000"}), json!("600000"), json!(["BL_F_Cu", "BL_B_Cu"])));
        assert_eq!(build(&json!({"wire": [[0, 0], [0, 5.08], [2.54, 5.08]]})).unwrap().len(), 2);
        let s = &build(&json!({"symbol": {"lib": "Connector:TestPoint", "ref": "TP1", "value": "CLK", "at": [250.19, 60.96]}})).unwrap()[0];
        assert_eq!((s["definition"]["id"].clone(), text2(&s["reference_field"]), text2(&s["value_field"]), s["transform"]["orientation"].clone()), (json!({"library_nickname": "Connector", "entry_name": "TestPoint"}), "TP1", "CLK", json!("SSO_0")));
        assert_eq!(build(&json!({"label": {"text": "CLK", "at": [1, 2], "type": "global"}})).unwrap()[0]["@type"], json!("type.googleapis.com/kiapi.schematic.types.GlobalLabel"));
        assert_eq!(build(&json!({"@type": "x", "a": 1})).unwrap(), vec![json!({"@type": "x", "a": 1})]);
        assert!(build(&json!({"track": {"from": [1, 2]}})).is_err() && build(&json!({"blob": 1})).unwrap_err().contains("cannot add `blob`"));
    }
    #[test] fn netclasses_rules_and_raw_messages() {
        let p = json!({"name": "P", "path": "/p"});
        let m = netclass_msgs(&json!({"name": "USB", "clearance": 0.15, "track_width": 0.2, "diff_pair_width": 0.2, "diff_pair_gap": 0.18, "patterns": ["/U*D+", "/D-"]}), &p).unwrap();
        assert_eq!(m[0], ("kiapi.common.commands.SetNetClasses".into(), json!({"project": p, "merge_mode": "MMM_MERGE", "net_classes": [{"name": "USB", "type": "NCT_EXPLICIT", "board": {"clearance": {"value_nm": "150000"}, "track_width": {"value_nm": "200000"}, "diff_pair_track_width": {"value_nm": "200000"}, "diff_pair_gap": {"value_nm": "180000"}}}]})));
        assert_eq!(m[1].1["pattern_assignments"], json!([{"pattern": "/U*D+", "netclass": "USB"}, {"pattern": "/D-", "netclass": "USB"}]));
        assert_eq!(netclass_msgs(&json!({"name": "USB", "patterns": ["/D+"]}), &p).unwrap().len(), 1, "patterns alone do not touch the class");
        assert_eq!(netclass_row(&json!("USB"), &json!({"net_classes": [{"name": "Default"}, {"name": "USB", "board": {"clearance": {"value_nm": "150000"}, "diff_pair_track_width": {"value_nm": "200000"}}}]}), &json!({"pattern_assignments": [{"pattern": "/D+", "netclass": "USB"}, {"pattern": "X", "netclass": "Default"}]})),
            json!({"name": "USB", "clearance": 0.15, "diff_pair_width": 0.2, "patterns": ["/D+"]}));
        assert_eq!(rule_msg(&json!({"name": "usb", "condition": "A.NetClass == 'USB'", "constraints": [{"type": "clearance", "min": 0.2}, {"type": "via_count", "max": 2}], "layer": "F.Cu"})).unwrap(),
            json!({"name": "usb", "condition": "A.NetClass == 'USB'", "constraints": [{"type": "CRCT_CLEARANCE", "numeric": {"min": 200000}}, {"type": "CRCT_VIA_COUNT", "numeric": {"max": 2}}], "single_layer": "BL_F_Cu"}));
        let mut msg = json!({"board": "$doc", "x": [{"project": "$project"}]}); fill(&mut msg, &json!({"type": "DOCTYPE_PCB", "project": p}));
        assert_eq!(msg, json!({"board": {"type": "DOCTYPE_PCB", "project": p}, "x": [{"project": p}]}));
        assert_eq!(brief(json!({"header": {}, "status": "IRS_OK", "created_items": [{"status": {"code": "ISC_OK"}, "item": {"id": {"value": "a"}, "big": 1}}, {"status": {"code": "ISC_INVALID_DATA"}, "item": {}}]})), json!({"status": "IRS_OK", "created_items": ["a", {"id": null, "status": {"code": "ISC_INVALID_DATA"}}]}));
        assert_eq!(brief(json!({"@type": "google.protobuf.Empty", "bytes": ""})), json!({}));
    }

    /// A stand-in KiCad: a board with two footprints, a track and a zone; it records every command.
    struct Fake { log: Mutex<Vec<(String, Value)>>, open: Mutex<bool>, fail: Option<&'static str>, busy: Mutex<u32>, save: Value, texts: Option<Vec<String>> }
    impl Call for Fake {
        fn call<'a>(&'a self, ty: &'a str, msg: Value) -> Fut<'a> { Box::pin(async move {
            self.log.lock().unwrap().push((ty.rsplit('.').next().unwrap_or("").to_string(), msg.clone()));
            if self.fail.is_some_and(|f| ty.ends_with(f)) { return Err("KiCad answered AS_BAD_REQUEST: no".to_string()) }
            if ty.ends_with("UpdateItems") { let mut b = self.busy.lock().unwrap(); if *b > 0 { *b -= 1; return Err("KiCad answered AS_BUSY: a task is running: DRC (token 7)".into()) } }
            if ty.ends_with("SaveDocument") { return Ok(self.save.clone()) }
            Ok(match ty.rsplit('.').next().unwrap_or("") {
                "GetOpenDocuments" => if *self.open.lock().unwrap() { json!({"documents": [{"type": "DOCTYPE_PCB", "board_filename": "b.kicad_pcb", "project": {"name": "b", "path": "/p"}}]}) } else { return Err("no handler".into()) },
                "OpenDocument" => { *self.open.lock().unwrap() = true; json!({"document": {"type": "DOCTYPE_PCB", "board_filename": "b.kicad_pcb", "project": {"name": "b", "path": "/p"}}}) }
                "GetItems" => match msg["types"][0].as_str().unwrap_or("") {
                    "KOT_PCB_FOOTPRINT" => json!({"items": [fp("R1", "10k", 1000000, 2000000), fp("R2", "10k", 3000000, 2000000), fp("C1", "1u", 5000000, 5000000)]}),
                    "KOT_PCB_TRACE" => json!({"items": [{"id": {"value": "t1"}, "start": {"x_nm": "146200000", "y_nm": "112400000"}, "end": {"x_nm": "153800000", "y_nm": "112400000"}, "width": {"value_nm": "200000"}, "layer": "BL_F_Cu", "net": {"name": "GND"}}]}),
                    "KOT_SCH_SYMBOL" => json!({"items": [{"id": {"value": "s1"}, "path": {"path": [{"value": "root"}, {"value": "sub"}], "path_human_readable": "/sub/"}, "position": {}, "reference_field": {"text": {"text": "R1"}}, "value_field": {"text": {"text": "10K"}}}]}),
                    "KOT_PCB_ZONE" => json!({"items": [{"id": {"value": "z1"}, "copper_settings": {"net": {"name": "+1V8"}}, "filled_polygons": [1, 2]}]}),
                    _ => json!({"items": []}),
                },
                "BeginCommit" => json!({"id": {"value": "commit-1"}}),
                "UpdateItems" => json!({"updated_items": msg["items"].as_array().unwrap().iter().map(|i| json!({"status": {"code": "ISC_OK"}, "item": i})).collect::<Vec<_>>()}),
                // (a schematic item comes back whole, as KiCad answers; a board item as its id, which is all these tests read of it)
                "CreateItems" => json!({"created_items": msg["items"].as_array().unwrap().iter().enumerate().map(|(n, i)| { let mut it = if i["@type"].as_str().is_some_and(|t| t.contains("schematic")) { i.clone() } else { json!({}) }; it["id"] = json!({"value": format!("new-{n}")}); json!({"status": {"code": "ISC_OK"}, "item": it}) }).collect::<Vec<_>>()}),
                // KiCad's own placement of a library symbol: a two-pin part (pin 1 above, pin 2 below); the power library has GND only
                "PlaceSymbolFromLibrary" => { if msg["lib_id"]["library_nickname"] == "power" && msg["lib_id"]["entry_name"] != "GND" { return Err("KiCad answered AS_BAD_REQUEST: symbol not found".into()) }
                    let pin = |n: &str, y: i64, o: &str| json!({"item": {"@type": "type.googleapis.com/kiapi.schematic.types.SchematicPin", "number": n, "name": if n == "1" { "A" } else { "K" }, "position": {"y_nm": (y * 1000000).to_string()}, "orientation": o}, "unit": {"unit": 1}});
                    json!({"item": {"id": {"value": "probe"}, "path": {}, "position": msg["position"], "transform": {"orientation": msg["orientation"]}, "unit": {"unit": 1}, "definition": {"id": msg["lib_id"], "items": [pin("1", -4, "SPO_DOWN"), pin("2", 4, "SPO_UP")]},
                        "reference_field": {"visible": true, "text": {"position": {"x_nm": "5"}, "text": msg["reference"]}}, "value_field": {"visible": true, "text": {"position": {"x_nm": "6"}, "text": "R"}}, "@type": "type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance"}}) }
                "DeleteItems" => json!({"deleted_items": msg["item_ids"].as_array().unwrap().iter().map(|i| json!({"id": i, "status": "IDS_OK"})).collect::<Vec<_>>()}),
                _ => json!({}),
            })
        }) }
        fn sheet_texts(&self) -> Option<Vec<String>> { self.texts.clone() }
    }
    fn fake(fail: Option<&'static str>) -> Fake { Fake { log: Mutex::new(vec![]), open: Mutex::new(false), fail, busy: Mutex::new(0), save: json!({}), texts: None } }
    fn names(f: &Fake) -> Vec<String> { f.log.lock().unwrap().iter().map(|(t, _)| t.clone()).collect() }
    fn sent(f: &Fake, ty: &str) -> Value { f.log.lock().unwrap().iter().find(|(t, _)| t == ty).map(|(_, m)| m.clone()).unwrap_or(Value::Null) }
    fn go(f: &Fake, a: Value) -> Result<Value, String> { go_on(f, "/p/b.kicad_pcb", a) }
    fn go_on(f: &Fake, path: &str, a: Value) -> Result<Value, String> { tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(run(f, path, &a)) }

    #[test] fn the_save_gate_and_kicads_task_gate_are_kept() {
        // the save gate's "nothing written" is not a save; a real save says what else it wrote
        let mut f = fake(None); f.save = json!({"not_saved": "KiCad says the document is not modified: nothing was written"});
        let out = go(&f, json!({"ops": [{"move": "R1", "by": [1, 0]}]})).unwrap();
        assert_eq!((out["saved"].clone(), out["not_saved"].clone()), (json!(false), json!("KiCad says the document is not modified: nothing was written")));
        let mut f = fake(None); f.save = json!({"format_upgrade": "upgraded the file format", "kicad_load_changes": "KiCad migrated 3 models"});
        let out = go(&f, json!({"ops": [{"move": "R1", "by": [1, 0]}]})).unwrap();
        assert_eq!((out["saved"].clone(), out["format_upgrade"].clone(), out["kicad_load_changes"].clone()), (json!("/p/b.kicad_pcb"), json!("upgraded the file format"), json!("KiCad migrated 3 models")));
        // a check is running: the write waits and goes again; the edit is one commit and is saved
        let f = fake(None); *f.busy.lock().unwrap() = 2;
        assert_eq!(go(&f, json!({"ops": [{"move": "R1", "by": [1, 0]}]})).unwrap()["saved"], "/p/b.kicad_pcb");
        assert_eq!(names(&f), ["GetOpenDocuments", "OpenDocument", "GetItems", "BeginCommit", "UpdateItems", "UpdateItems", "UpdateItems", "EndCommit", "SaveDocument"]);
        // an open dialog's refusal names the dialog and is the answer at once; a refused command (the plan's approval, in the gate above this) is too
        assert!(task_busy("KiCad answered AS_BUSY: a task is running: DRC") && !task_busy("KiCad answered AS_BUSY: the dialog \"Zone Properties\" is open"));
        // nothing here closes a document, shows a frame or selects: only these commands ever leave an edit
        let f = fake(None); go(&f, json!({"ops": [{"find": "zone"}, {"set": {"kind": "zone", "net": "+1V8"}, "name": "Z"}, {"delete": {"kind": "track", "net": "GND"}}, {"netclass": {"name": "N", "clearance": 0.2}}, {"refill": true}]})).unwrap();
        const SENT: &[&str] = &["GetOpenDocuments", "OpenDocument", "GetItems", "BeginCommit", "UpdateItems", "DeleteItems", "EndCommit", "SetNetClasses", "GetNetClasses", "GetNetClassAssignments", "RefillZones", "SaveDocument"];
        assert!(names(&f).iter().all(|n| SENT.contains(&n.as_str())), "{:?}", names(&f));
    }
    #[test] fn what_is_set_on_a_sub_sheets_symbol_is_read_back_from_the_saved_files() {
        let sheet = |v: &str, mpn: &str| format!("(kicad_sch\n\t(lib_symbols\n\t\t(symbol \"R\")\n\t)\n\t(symbol\n\t\t(lib_id \"Device:R\")\n\t\t(property \"Value\" \"{v}\")\n\t\t(property \"MPN\" \"{mpn}\")\n\t\t(instances (path \"/a/b\" (reference \"R1\")))\n\t)\n\t(symbol\n\t\t(lib_id \"Device:R\")\n\t\t(property \"Value\" \"1k\")\n\t\t(instances (path \"/a/b\" (reference \"R2\")))\n\t)\n)");
        let texts = vec!["(kicad_sch)".to_string(), sheet("10k", "RC \\\"0603\\\"")];
        assert!(file_prop(&texts, "R1", "Value", "10k") && file_prop(&texts, "R1", "MPN", "RC \"0603\"") && !file_prop(&texts, "R1", "Value", "1k") && !file_prop(&texts, "R2", "MPN", "RC \"0603\"") && !file_prop(&texts, "R9", "Value", "10k"));
        let op = json!({"ops": [{"set": "R1", "value": "10k", "fields": {"MPN": "X1"}}]});
        // the file has it: done, and the update went to the symbol's own sheet
        let mut f = fake(None); f.texts = Some(vec![sheet("10k", "X1")]);
        assert_eq!(go_on(&f, "/p/b.kicad_sch", op.clone()).unwrap()["saved"], "/p/b.kicad_sch");
        assert_eq!(sent(&f, "UpdateItems")["header"]["document"]["sheet_path"], json!({"path": [{"value": "root"}, {"value": "sub"}]}));
        // KiCad said OK but the saved sheet still has the old text: never reported as done
        let mut f = fake(None); f.texts = Some(vec![sheet("10K", "X1")]);
        let e = go_on(&f, "/p/b.kicad_sch", op).unwrap_err(); assert!(e.starts_with("saved, but the schematic's files do not show: R1 Value"), "{e}");
    }
    #[test] fn a_writing_call_is_told_of_the_person_first() {
        assert!(writes(&json!([{"find": "zone"}, {"move": "R1", "by": [1, 0]}])) && writes(&json!([{"ipc": {"type": "RefillZones"}}])) && !writes(&json!([{"find": "zone"}, {"ipc": {"type": "GetNets"}}])));
        let t = person_first(&json!({"selection": {"items": 2}})); assert!(t.contains("\"items\":2") && t.contains("Nothing was changed") && t.contains("person_seen: true"), "{t}");
    }

    #[test] fn one_call_opens_reads_commits_and_saves() {
        let f = fake(None);
        let out = go(&f, json!({"ops": [{"move": ["R1", "R2"], "by": [0, 1]}, {"set": "C1", "rot": 90, "value": "2u2"}, {"add": [{"track": {"from": [1, 2], "to": [3, 2], "width": 0.2, "layer": "F.Cu", "net": "GND"}}]}], "message": "tidy"})).unwrap();
        assert_eq!(out, json!({"results": [{"updated": [{"ref": "R1", "value": "10k", "at": [1.0, 3.0], "rot": -90.0, "layer": "B.Cu"}, {"ref": "R2", "value": "10k", "at": [3.0, 3.0], "rot": -90.0, "layer": "B.Cu"}]},
            {"updated": [{"ref": "C1", "value": "2u2", "at": [5.0, 5.0], "rot": 90.0, "layer": "B.Cu"}]}, {"created": [{"id": "new-0"}]}], "saved": "/p/b.kicad_pcb"}), "a change answers with what the items are now");
        // the document is opened once, the footprints are read once before the commit starts, and one commit holds every change
        assert_eq!(names(&f), ["GetOpenDocuments", "OpenDocument", "GetItems", "BeginCommit", "UpdateItems", "GetItems", "UpdateItems", "CreateItems", "EndCommit", "RefillZones", "SaveDocument"], "a new track stands in a pour: every zone is refilled before the save");
        let up = sent(&f, "UpdateItems");
        assert_eq!((up["items"][0]["position"].clone(), up["items"][1]["position"]["y_nm"].clone(), up["items"][0].get("definition"), up["header"]["document"]["board_filename"].clone()), (json!({"x_nm": "1000000", "y_nm": "3000000"}), json!("3000000"), None, json!("b.kicad_pcb")));
        assert_eq!(sent(&f, "EndCommit"), json!({"header": {"document": {"type": "DOCTYPE_PCB", "board_filename": "b.kicad_pcb", "project": {"name": "b", "path": "/p"}}}, "id": {"value": "commit-1"}, "action": "CMA_COMMIT", "message": "tidy"}));
    }
    #[test] fn a_find_alone_reads_and_saves_nothing() {
        let f = fake(None);
        let out = go(&f, json!({"ops": [{"find": {"kind": "footprint", "ref": "R*"}}, {"find": {"kind": "track", "near": [150, 112.4, 4]}}, {"find": {"kind": "pad", "ref": "C1"}}]})).unwrap();
        assert_eq!(out["results"][0]["rows"].as_array().unwrap().len(), 2); assert_eq!(out["results"][1]["rows"][0]["id"], "t1", "a track is near anywhere along it");
        let mid = go(&fake(None), json!({"ops": [{"find": {"kind": "track", "near": [150, 112.45, 0.1]}}, {"find": {"kind": "track", "near": [150, 113, 0.1]}}, {"find": {"kind": "track", "near": [154.5, 112.4, 0.1]}}]})).unwrap();
        assert_eq!((mid["results"][0]["rows"].as_array().unwrap().len(), mid["results"][1]["rows"].as_array().unwrap().len(), mid["results"][2]["rows"].as_array().unwrap().len()), (1, 0, 0)); assert_eq!(out["results"][2]["rows"][0]["ref"], "C1");
        assert!(out.get("saved").is_none() && !names(&f).iter().any(|n| matches!(n.as_str(), "BeginCommit" | "SaveDocument")), "{:?}", names(&f));
    }
    #[test] fn a_failed_op_drops_the_commit_and_saves_nothing() {
        let f = fake(None);
        let e = go(&f, json!({"ops": [{"move": "R1", "by": [1, 0]}, {"set": ["R1", "R9"], "value": "1k"}]})).unwrap_err();
        assert!(e.starts_with("op 2 (set): no footprint `R9`. Nothing was saved"), "{e}");
        assert_eq!(sent(&f, "EndCommit")["action"], "CMA_DROP"); assert!(!names(&f).contains(&"SaveDocument".to_string()));
        let f = fake(Some("CreateItems")); let e = go(&f, json!({"ops": [{"netclass": {"name": "USB", "clearance": 0.2}}, {"add": {"via": {"at": [1, 1], "size": 0.6, "drill": 0.3, "net": "GND"}}}]})).unwrap_err();
        assert!(e.contains("op 2 (add): KiCad answered AS_BAD_REQUEST: no") && e.contains("already applied, not undone: netclass"), "{e}");
        assert!(go(&fake(None), json!({"ops": [{"paint": 1}]})).unwrap_err().starts_with("op 1: an op is one of") && go(&fake(None), json!({})).is_err());
        assert!(go(&fake(None), json!({"ops": [{"delete": {"kind": "track"}}]})).unwrap_err().contains("delete needs more than a kind"));
    }
    #[test] fn zones_are_sent_without_fills_and_refilled_and_deletes_go_by_selector() {
        let f = fake(None);
        let out = go(&f, json!({"ops": [{"set": {"kind": "zone", "net": "+1V8"}, "clearance": 0.2, "name": "PWR_1V8"}, {"delete": {"kind": "track", "from": [146.2, 112.4]}}]})).unwrap();
        assert_eq!(out["results"], json!([{"updated": [{"name": "PWR_1V8", "net": "+1V8", "layers": [], "priority": 0, "clearance": 0.2, "min_width": 0.0, "corners": 0, "type": ""}]}, {"deleted": 1}]));
        assert_eq!(sent(&f, "UpdateItems")["items"][0], json!({"id": {"value": "z1"}, "name": "PWR_1V8", "copper_settings": {"net": {"name": "+1V8"}, "clearance": {"value_nm": "200000"}}}));
        assert_eq!(sent(&f, "DeleteItems")["item_ids"], json!([{"value": "t1"}]));
        assert_eq!(names(&f)[names(&f).len() - 3..], ["EndCommit", "RefillZones", "SaveDocument"]); assert_eq!(sent(&f, "RefillZones")["zones"], json!([{"value": "z1"}]));
    }
    #[test] fn a_find_after_a_change_sees_it_and_a_new_symbol_is_placed_by_kicad() {
        let f = fake(None);
        let out = go(&f, json!({"ops": [{"move": "R1", "by": [1, 0]}, {"find": {"kind": "footprint", "ref": "R1"}}, {"set": "R9", "value": "x"}]})).unwrap_err();
        assert_eq!(names(&f), ["GetOpenDocuments", "OpenDocument", "GetItems", "BeginCommit", "UpdateItems", "EndCommit", "GetItems", "SaveDocument"], "the find ended the commit and read the footprints again; the failed op had no commit to drop");
        assert!(out.contains("op 3 (set)") && out.contains("before op 2's find were already committed and are saved"), "{out}");
        // a symbol: KiCad places it from the library in a dropped commit, and the real one keeps that placement with the asked texts
        let t = json!({"id": {"value": "probe"}, "path": {}, "position": {"x_nm": "1", "y_nm": "2"}, "lib_id": {"library_nickname": "Device", "entry_name": "R"}, "definition": {"items": [1, 2, 3]},
            "reference_field": {"visible": true, "text": {"position": {"x_nm": "5"}, "text": "R"}}, "value_field": {"visible": true, "text": {"position": {"x_nm": "6"}, "text": "R"}}});
        let s = placed(&t, &json!({"lib": "Device:R", "ref": "R7", "value": "1k", "fields": {"MPN": "X"}}));
        assert_eq!((s.get("id"), s["definition"].clone(), s["reference_field"].clone(), text2(&s["value_field"]), text2(&s["user_fields"][0])), (None, json!({"items": [1, 2, 3]}), json!({"visible": true, "text": {"position": {"x_nm": "5"}, "text": "R7"}}), "1k", "X"), "KiCad's whole definition goes back (a symbol that extends another is right only so)");
        // Footprint is KiCad's own field of a symbol, never a user field (which KiCad would not write)
        let s = placed(&t, &json!({"lib": "Device:R", "ref": "R7", "footprint": "Lib:FP", "fields": {"Datasheet": "d"}}));
        assert_eq!((text2(&s["footprint_field"]), text2(&s["datasheet_field"]), s.get("user_fields")), ("Lib:FP", "d", None));
        let f = fake(None); go_on(&f, "/p/b.kicad_sch", json!({"ops": [{"add": [{"symbol": {"lib": "Device:R", "ref": "R7", "at": [1, 2]}}, {"wire": [[1, 2], [1, 5]]}]}]})).unwrap();
        assert_eq!(names(&f), ["GetOpenDocuments", "OpenDocument", "BeginCommit", "PlaceSymbolFromLibrary", "EndCommit", "GetSchematicHierarchy", "GetItems", "BeginCommit", "CreateItems", "EndCommit", "SaveDocument"], "placed by KiCad, then the sheet is read once (what the new part would lie over), then the one commit");
        let log = f.log.lock().unwrap(); assert_eq!((log[4].1["action"].clone(), log[9].1["action"].clone(), log[8].1["items"].as_array().unwrap().len()), (json!("CMA_DROP"), json!("CMA_COMMIT"), 2));
        assert_eq!((log[3].1["reference"].clone(), text2(&log[8].1["items"][0]["reference_field"]), log[8].1["items"][0].get("id")), (json!("R7"), "R7", None), "KiCad places it from the library in a commit that is dropped; the real one is that placement");
    }
    #[test] fn a_symbols_pins_join_their_nets_and_a_wire_runs_between_pins() {
        // where the pins are: the library's place turned with the symbol (KiCad's angle, y down)
        let sym = |rot: &str| json!({"position": {"x_nm": "10000000", "y_nm": "20000000"}, "transform": {"orientation": rot}, "unit": {"unit": 1}, "definition": {"items": [
            {"item": {"@type": "x.SchematicPin", "number": "1", "name": "A", "position": {"y_nm": "-4000000"}, "orientation": "SPO_DOWN"}, "unit": {"unit": 1}}, {"item": {"@type": "x.SchematicPin", "number": "9", "position": {}, "orientation": "SPO_UP"}, "unit": {"unit": 2}}]}});
        assert_eq!(sym_pins(&sym("SSO_0")), vec![("1".to_string(), "A".to_string(), (10.0, 16.0), (0.0, -1.0))], "the other unit's pin is not this symbol's");
        assert_eq!(sym_pins(&sym("SSO_90"))[0].2, (6.0, 20.0)); assert_eq!(sym_pins(&sym("SSO_90"))[0].3, (-1.0, 0.0));
        // pins: {pin: net}: a short wire off each pin and the net's power symbol at its end; a net the power library lacks gets a label
        let f = fake(None); let out = go_on(&f, "/p/b.kicad_sch", json!({"ops": [{"add": [{"symbol": {"lib": "Device:D", "ref": "D7", "at": [10, 20], "footprint": "L:F", "pins": {"A": "+3V3", "2": "GND"}}}, {"wire": ["D7.1", [14, 10]]}, {"label": {"text": "X", "at": "D7.K"}}]}]})).unwrap();
        let made = sent(&f, "CreateItems"); let items = made["items"].as_array().unwrap();
        let kinds: Vec<&str> = items.iter().map(|i| i["@type"].as_str().unwrap().rsplit('.').next().unwrap()).collect();
        assert_eq!(kinds, ["SchematicSymbolInstance", "SchematicLine", "LocalLabel", "SchematicLine", "SchematicSymbolInstance", "SchematicLine", "SchematicLine", "LocalLabel"]);
        assert_eq!((text2(&items[0]["footprint_field"]), items[1]["start"].clone(), items[1]["end"].clone(), text2(&items[2]), items[2]["position"].clone()), ("L:F", to_xy(10.0, 16.0), to_xy(10.0, 13.46), "+3V3", to_xy(10.0, 13.46)), "pin A (1) is above: its wire goes up 2.54 mm to a label, the power library having no +3V3");
        assert_eq!((items[3]["end"].clone(), items[4]["position"].clone(), text2(&items[4]["reference_field"]), lib_id(&items[4]["definition"]["id"])), (to_xy(10.0, 26.54), to_xy(10.0, 26.54), "#PWR001", "power:GND".to_string()));
        // a wire named by a pin starts on the pin and runs square to the point; a label's place may be a pin
        assert_eq!((items[5]["start"].clone(), items[5]["end"].clone(), items[6]["end"].clone(), items[7]["position"].clone()), (to_xy(10.0, 16.0), to_xy(14.0, 16.0), to_xy(14.0, 10.0), to_xy(10.0, 24.0)));
        // the answer: the part with its pins' places; the rest counted
        assert_eq!(out["results"][0]["created"][0]["pins"], json!({"1": [10.0, 16.0], "2": [10.0, 24.0]}));
        let e = go_on(&fake(None), "/p/b.kicad_sch", json!({"ops": [{"add": [{"wire": ["Q9.1", [1, 1]]}]}]})).unwrap_err(); assert!(e.contains("no symbol `Q9`"), "{e}");
        let e = go_on(&fake(None), "/p/b.kicad_sch", json!({"ops": [{"add": [{"symbol": {"lib": "Device:D", "ref": "D7", "at": [1, 1], "pins": {"7": "GND"}}}]}]})).unwrap_err(); assert!(e.contains("has no pin `7`: 1, 2"), "{e}");
    }
    #[test] fn the_answer_says_what_the_edit_added_to_kicads_check() {
        let v = |ty: &str, a: &str| json!({"type": ty, "items": [{"description": a, "pos": {"x": 1.5, "y": 2}}]});
        let before = json!({"violations": [v("clearance", "Track [A]"), v("clearance", "Track [A]")], "unconnected_items": [v("x", "Pad 1 [N] of R1"), v("x", "Pad 1 [M] of R2")]});
        let after = json!({"violations": [v("clearance", "Track [A]"), v("shorting_items", "Via [B]")], "unconnected_items": [v("x", "Pad 1 [M] of R2")]});
        assert_eq!(check_delta(&before, &after, &[]), json!({"by": "KiCad's DRC of the saved board, schematic parity included", "new": ["shorting_items: Via [B] @1.5,2"], "left": 3, "gone": 2}));
        assert_eq!(findings(&json!({"sheets": [{"violations": [v("pin_not_connected", "Symbol U7 Pin 1")]}, {"violations": []}]})), ["pin_not_connected: Symbol U7 Pin 1 @1.5,2"]);
        let many = json!({"violations": (0..15).map(|i| v("clearance", &format!("T{i}"))).collect::<Vec<_>>()});
        let d = check_delta(&json!({}), &many, &[]); assert_eq!((d["new"].as_array().unwrap().len(), d["more"].clone(), d["left"].clone()), (12, json!(3), json!(15)));
        // a refill moves findings about on a large board: the same finding with another length or place is the same one, and one on a net
        // the edit did not work on is counted, not listed
        let u = |a: &str, x: f64| json!({"type": "unconnected_items", "items": [{"description": a, "pos": {"x": x, "y": 1}}]});
        let (b4, aft) = (json!({"unconnected_items": [u("Track [/SCL] on F.Cu, length 2.8500 mm", 1.0)]}), json!({"unconnected_items": [u("Track [/SCL] on F.Cu, length 0.2915 mm", 7.0), u("Zone [GND] on In5.Cu", 2.0), u("Pad 1 [/LED1] of R7", 3.0)]}));
        let d = check_delta(&b4, &aft, &["/LED1".to_string()]); assert_eq!((d["new"].clone(), d["left"].clone(), d.get("gone")), (json!(["unconnected: Pad 1 [/LED1] of R7 @3.0,1"]), json!(3), None), "{d}");
        assert_eq!((d["elsewhere"]["count"].clone(), d["elsewhere"]["first"].clone()), (json!(1), json!(["unconnected: Zone [GND] on In5.Cu @2.0,1"])));
        assert_eq!(check_delta(&b4, &aft, &["R7".to_string()])["new"].as_array().unwrap().len(), 1, "a part by its reference"); assert_eq!(check_delta(&b4, &aft, &[])["new"].as_array().unwrap().len(), 2, "no list: every new finding");
        // the driver: the check runs before the edit and after the save, and only a call that writes and saves is checked
        struct Checked(Fake, Mutex<u32>);
        impl Call for Checked { fn call<'a>(&'a self, ty: &'a str, msg: Value) -> Fut<'a> { self.0.call(ty, msg) }
            fn check<'a>(&'a self) -> Fut<'a> { Box::pin(async move { let mut n = self.1.lock().unwrap(); *n += 1; Ok(if *n == 1 { json!({"violations": []}) } else { json!({"violations": [{"type": "clearance", "items": [{"description": "Track [GND]"}]}]}) }) }) } }
        let c = Checked(fake(None), Mutex::new(0)); let rt = || tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        let out = rt().block_on(run(&c, "/p/b.kicad_pcb", &json!({"ops": [{"move": "R1", "by": [1, 0]}]}))).unwrap();
        assert_eq!((out["check"].clone(), *c.1.lock().unwrap()), (json!({"by": "KiCad's DRC of the saved board, schematic parity included", "new": [], "left": 1, "elsewhere": {"count": 1, "first": ["clearance: Track [GND]"], "note": "on items this edit did not work on: a refill or the file's first save moves such findings about; they are this edit's only if its copper cut a pour there"}}), 2), "R1 was moved: a finding that does not name R1 is set apart, not hidden");
        let c = Checked(fake(None), Mutex::new(0)); let out = rt().block_on(run(&c, "/p/b.kicad_pcb", &json!({"ops": [{"find": "footprint"}]}))).unwrap(); assert_eq!((out.get("check"), *c.1.lock().unwrap()), (None, 0));
        let c = Checked(fake(None), Mutex::new(0)); rt().block_on(run(&c, "/p/b.kicad_pcb", &json!({"ops": [{"move": "R1", "by": [1, 0]}], "check": false}))).unwrap(); assert_eq!(*c.1.lock().unwrap(), 0);
    }
    #[test] fn raw_commands_run_in_order_with_the_document_filled_in() {
        let f = fake(None);
        let out = go(&f, json!({"ops": [{"ipc": {"type": "GetNets", "message": {"board": "$doc"}}}, {"ipc": {"type": "kiapi.board.commands.CreateItems", "message": {"header": {"document": "$doc"}, "items": [{"@type": "x"}]}}}], "save": false})).unwrap();
        assert_eq!(out, json!({"results": [{}, {"created_items": ["new-0"]}]}));
        assert_eq!(names(&f), ["GetOpenDocuments", "OpenDocument", "GetNets", "BeginCommit", "CreateItems", "EndCommit"]);
        assert_eq!(sent(&f, "GetNets")["board"]["board_filename"], "b.kicad_pcb");
    }
}
