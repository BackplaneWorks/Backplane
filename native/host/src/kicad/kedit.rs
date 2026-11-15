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

pub type Fut<'a> = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>>;
/// One raw IPC command: (fully qualified type, message) -> the response's message.
pub trait Call: Sync {
    fn call<'a>(&'a self, ty: &'a str, msg: Value) -> Fut<'a>;
    /// The saved schematic's sheet files as text, for the read-back of what was set (None: not read back, as with a stand-in).
    fn sheet_texts(&self) -> Option<Vec<String>> { None }
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
    else { for f in raw["user_fields"].as_array().into_iter().flatten() { if !text2(f).is_empty() { m.insert(f["name"].as_str().unwrap_or("").into(), json!(text2(f))); } } }
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
        .map(|p| json!({"id": p["id"]["value"].as_str().unwrap_or(""), "ref": r, "pad": p["number"].as_str().unwrap_or(""), "net": p["net"]["name"].as_str().unwrap_or(""), "at": xy(&p["position"]), "pin": p["symbol_pin"]["name"].as_str().unwrap_or("")})).collect()
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
            ("footprint" | "via" | "symbol" | "label" | "junction" | "no_connect", "at") => raw["position"] = point(v, "at")?,
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
            ("symbol", "fields") => for (name, t) in v.as_object().ok_or("fields is {name: text}")? {
                let t = t.as_str().ok_or("a field's value is text")?; let new = sch_field(name, t, raw);
                if !raw["user_fields"].is_array() { raw["user_fields"] = json!([]) }
                let fs = raw["user_fields"].as_array_mut().unwrap();
                match fs.iter_mut().find(|f| f["name"] == json!(name)) { Some(f) => f["text"]["text"] = json!(t), None => fs.push(new) }
            },
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
    s["definition"] = json!({"id": if s["lib_id"].is_object() { s["lib_id"].clone() } else { template["definition"]["id"].clone() }});
    if let Some(r) = v["ref"].as_str() { s["reference_field"]["text"]["text"] = json!(r) }
    if let Some(x) = v["value"].as_str() { s["value_field"]["text"]["text"] = json!(x) }
    if let Some(fs) = v["fields"].as_object() { let extra: Vec<Value> = fs.iter().map(|(n, t)| sch_field(n, t.as_str().unwrap_or(""), &s)).collect(); s["user_fields"] = json!(extra) }
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
    /// What the saved sheet files must show (reference, property, text), and how many updates KiCad deferred (the person is editing those items).
    expect: Vec<(String, String, String)>, deferred: usize }
/// Rows a change answers with: what the items are now, so nothing is read again to check it.
const SHOWN: usize = 24;
/// The row kind of one of KiCad's own items, by its type.
fn kind_of(raw: &Value) -> &'static str {
    match raw["@type"].as_str().unwrap_or("").rsplit('.').next().unwrap_or("") {
        "Track" => "track", "Arc" => "arc", "Via" => "via", "Zone" => "zone", "FootprintInstance" => "footprint", "SchematicSymbolInstance" => "symbol", "SchematicLine" => "wire",
        "LocalLabel" | "GlobalLabel" | "HierarchicalLabel" => "label", "Junction" => "junction", "NoConnect" => "no_connect", _ => "",
    }
}

fn verb(op: &Value) -> &str { const V: &[&str] = &["find", "move", "set", "add", "delete", "ipc", "netclass", "rule", "refill"]; op.as_object().and_then(|o| V.iter().find(|v| o.contains_key(**v))).copied().unwrap_or("") }
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
    /// Each symbol the ops add, placed once by KiCad from its library alone in a commit that is dropped: where KiCad puts the reference and
    /// value, shown or hidden as the library has them. (Fields sent without a place land hidden at the sheet's origin.)
    async fn templates(&mut self, ops: &[Value]) -> Result<(), String> {
        for op in ops.iter().filter(|o| verb(o) == "add") {
            let list = match &op["add"] { Value::Array(a) => a.clone(), o => vec![o.clone()] };
            for it in list.iter().filter(|it| it["symbol"].is_object()) {
                let mut bare = build(it)?.remove(0);
                if let Some(o) = bare.as_object_mut() { for k in ["reference_field", "value_field", "user_fields"] { o.remove(k); } }
                let sheet = self.sheet(if it["sheet"].is_null() { &op["sheet"] } else { &it["sheet"] }).await?;
                let id = self.c.call(&format!("{C}BeginCommit"), json!({"header": self.header(&Value::Null)})).await?["id"].clone();
                let made = self.c.call(&format!("{C}CreateItems"), json!({"header": self.header(&sheet), "items": [bare]})).await;
                self.c.call(&format!("{C}EndCommit"), json!({"header": self.header(&Value::Null), "id": id, "action": "CMA_DROP"})).await?;
                let made = made?; if let Some(bad) = status_bad(&made, "created_items") { return Err(format!("symbol {}: {bad}", it["symbol"]["lib"].as_str().unwrap_or(""))) }
                self.templates.insert(it.to_string(), made["created_items"][0]["item"].clone());
            }
        }
        Ok(())
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
    async fn op(&mut self, i: usize, op: &Value) -> Result<Value, String> {
        match verb(op) {
            "find" => {
                let sel = if let Some(k) = op["find"].as_str() { json!({"kind": k}) } else { selector(&op["find"], self.board) }; let kind = sel["kind"].as_str().unwrap_or("").to_string(); let limit = sel["limit"].as_u64().map_or(ROWS, |n| n as usize);
                if kind == "net" { let r = self.c.call(&format!("{B}GetNets"), json!({"board": self.doc})).await?;
                    let names: Vec<&str> = r["nets"].as_array().into_iter().flatten().filter_map(|n| n["name"].as_str()).filter(|n| !n.is_empty() && sel["name"].as_str().map_or(true, |p| glob(p, n))).collect(); return Ok(json!({"nets": names})) }
                // KiCad shows a commit's changes only once it ends: a find after a change ends the commit so far, and its rows are the document as changed
                if self.commit.is_some() { let m = self.message.clone(); self.end("CMA_COMMIT", &m).await?; self.split = i + 1 }
                let wf = sel["fields"].as_bool().unwrap_or(false);
                let rows: Vec<Value> = if kind == "pad" { let fsel = json!({"kind": "footprint", "ref": sel["ref"].clone()}); let fps = if sel["ref"].is_null() { self.items("footprint").await? } else { self.pick(&fsel).await? };
                        fps.iter().flat_map(|(f, _)| pad_rows(f)).filter(|r| matches(&sel, r)).collect() }
                    else if sel["raw"].as_bool().unwrap_or(false) { self.pick(&sel).await?.into_iter().map(|(raw, _)| raw).collect() }
                    else { self.pick(&sel).await?.iter().map(|(raw, _)| row(&kind, raw, wf)).collect() };
                let n = rows.len(); let mut out = json!({"rows": rows.into_iter().take(limit).collect::<Vec<_>>()});
                if n > limit { out["more"] = json!(format!("{} more: narrow the selector or give limit", n - limit)) }
                Ok(out)
            }
            v @ ("move" | "set") => {
                let sel = selector(&op[v], self.board); let kind = sel["kind"].as_str().unwrap_or("").to_string();
                let picked = self.pick(&sel).await?;
                if picked.is_empty() { return Err(format!("nothing matches {sel}")) }
                // every reference asked for by name must be there: a typo is an error, not a smaller edit
                if let Some(want) = sel["ref"].as_array() { for w in want.iter().filter_map(Value::as_str).filter(|w| !w.contains(['*', '?'])) { if !picked.iter().any(|(r, _)| row(&kind, r, false)["ref"] == json!(w)) { return Err(format!("no {kind} `{w}`")) } } }
                let mut with = op.as_object().map(|o| o.iter().filter(|(k, _)| if v == "move" { !matches!(k.as_str(), "move" | "by" | "to") } else { k.as_str() != "set" }).map(|(k, v)| (k.clone(), v.clone())).collect::<Map<_, _>>()).unwrap_or_default();
                if v == "move" { if let Some(to) = op.get("to") { with.insert("at".into(), to.clone()); } if op.get("by").is_none() && with.is_empty() { return Err("move takes by: [dx, dy], to: [x, y] and/or rot".into()) } }
                else if with.is_empty() { return Err("set takes the row keys to change (value, fields, at, rot, name, clearance, ...)".into()) }
                let mut ups = vec![];
                for (mut raw, sheet) in picked {
                    let mut w = with.clone();
                    if let Some((dx, dy)) = op.get("by").map(|b| pair(b).ok_or("by is [dx, dy] in mm")).transpose()? { let at = row(&kind, &raw, false); let k = if at.get("at").is_some() { "at" } else { return Err(format!("a {kind} moves with set: from/to")) };
                        let (x, y) = pair(&at[k]).unwrap_or((0.0, 0.0)); w.insert("at".into(), json!([x + dx, y + dy])); }
                    let body = apply(&kind, &mut raw, &Value::Object(w))?;
                    if kind == "symbol" { let r = text2(&raw["reference_field"]).to_string();
                        if let Some(x) = with.get("value").and_then(Value::as_str) { self.expect.push((r.clone(), "Value".into(), x.into())) }
                        for (n, t) in with.get("fields").and_then(Value::as_object).into_iter().flatten() { self.expect.push((r.clone(), n.clone(), t.as_str().unwrap_or("").into())) } }
                    ups.push((raw, sheet, body));
                }
                let before = self.deferred; let rows = self.update(&kind, ups).await?;
                // the person is editing some of these right now: KiCad applies the change when they finish, merged with theirs (GetNotices says how it went)
                Ok(if self.deferred > before { json!({"updated": rows, "deferred": self.deferred - before}) } else { json!({"updated": rows}) })
            }
            "add" => {
                let list = match &op["add"] { Value::Array(a) => a.clone(), o => vec![o.clone()] }; self.begin().await?; let mut ids = vec![]; let mut by: Vec<(Value, Vec<Value>)> = vec![];
                for it in &list { let sheet = self.sheet(if it["sheet"].is_null() { &op["sheet"] } else { &it["sheet"] }).await?;
                    let raws = match self.templates.get(&it.to_string()) { Some(t) => vec![placed(t, &it["symbol"])], None => build(it)? }; match by.iter_mut().find(|(s, _)| *s == sheet) { Some((_, v)) => v.extend(raws), None => by.push((sheet, raws)) } }
                for (sheet, raws) in by {
                    let r = self.c.call(&format!("{C}CreateItems"), json!({"header": self.header(&sheet), "items": raws})).await?;
                    if let Some(bad) = status_bad(&r, "created_items") { return Err(bad) }
                    ids.extend(r["created_items"].as_array().into_iter().flatten().map(|i| { let k = kind_of(&i["item"]); if k.is_empty() { json!({"id": i["item"]["id"]["value"]}) } else { row(k, &i["item"], k == "symbol") } }));
                }
                Ok(json!({"created": ids}))
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
            _ => Err("an op is one of find, move, set, add, delete, netclass, rule, refill, ipc".into()),
        }
    }
}

/// The whole edit: `{path, ops: [...], message?, save?}` -> `{results: [...], saved}`. An op that fails drops the commit: nothing of this
/// call's item changes stays (net classes, rules and raw `ipc` commands that already ran are named in the error).
pub async fn run(c: &dyn Call, path: &str, a: &Value) -> Result<Value, String> {
    let ops = match &a["ops"] { Value::Array(o) if !o.is_empty() => o.clone(), _ => return Err("ops is a list of edits: find, move, set, add, delete, netclass, rule, refill, ipc".into()) };
    if let Some((i, _)) = ops.iter().enumerate().find(|(_, o)| verb(o).is_empty()) { return Err(format!("op {}: an op is one of find, move, set, add, delete, netclass, rule, refill, ipc", i + 1)) }
    let patient = Patient { c, step: BUSY_STEP, wait: BUSY_WAIT }; let c: &dyn Call = &patient;
    let doc = Run::open(c, path).await?;
    let mut r = Run { c, expect: vec![], deferred: 0, board: path.ends_with(".kicad_pcb"), doc, commit: None, sheets: None, refill: vec![], wrote: false, cache: Default::default(),
        message: a["message"].as_str().unwrap_or("kicad_edit").to_string(), split: 0, templates: Default::default() };
    r.templates(&ops).await?;
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
                "CreateItems" => json!({"created_items": msg["items"].as_array().unwrap().iter().enumerate().map(|(n, _)| json!({"status": {"code": "ISC_OK"}, "item": {"id": {"value": format!("new-{n}")}}})).collect::<Vec<_>>()}),
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
        assert_eq!(names(&f), ["GetOpenDocuments", "OpenDocument", "GetItems", "BeginCommit", "UpdateItems", "GetItems", "UpdateItems", "CreateItems", "EndCommit", "SaveDocument"]);
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
        assert_eq!((s.get("id"), s["definition"].clone(), s["reference_field"].clone(), text2(&s["value_field"]), text2(&s["user_fields"][0])), (None, json!({"id": {"library_nickname": "Device", "entry_name": "R"}}), json!({"visible": true, "text": {"position": {"x_nm": "5"}, "text": "R7"}}), "1k", "X"));
        let f = fake(None); go(&f, json!({"ops": [{"add": [{"symbol": {"lib": "Device:R", "ref": "R7", "at": [1, 2]}}, {"wire": [[1, 2], [1, 5]]}]}]})).unwrap();
        assert_eq!(names(&f), ["GetOpenDocuments", "OpenDocument", "BeginCommit", "CreateItems", "EndCommit", "BeginCommit", "CreateItems", "EndCommit", "SaveDocument"]);
        let log = f.log.lock().unwrap(); assert_eq!((log[4].1["action"].clone(), log[7].1["action"].clone(), log[6].1["items"].as_array().unwrap().len()), (json!("CMA_DROP"), json!("CMA_COMMIT"), 2));
        assert_eq!((log[3].1["items"][0].get("reference_field"), text2(&log[6].1["items"][0]["reference_field"])), (None, "R7"), "the probe is the bare library symbol; the real one carries the reference");
    }
    #[test] fn raw_commands_run_in_order_with_the_document_filled_in() {
        let f = fake(None);
        let out = go(&f, json!({"ops": [{"ipc": {"type": "GetNets", "message": {"board": "$doc"}}}, {"ipc": {"type": "kiapi.board.commands.CreateItems", "message": {"header": {"document": "$doc"}, "items": [{"@type": "x"}]}}}], "save": false})).unwrap();
        assert_eq!(out, json!({"results": [{}, {"created_items": ["new-0"]}]}));
        assert_eq!(names(&f), ["GetOpenDocuments", "OpenDocument", "GetNets", "BeginCommit", "CreateItems", "EndCommit"]);
        assert_eq!(sent(&f, "GetNets")["board"]["board_filename"], "b.kicad_pcb");
    }
}
