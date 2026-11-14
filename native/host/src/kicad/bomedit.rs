//! Editing the BOM from its panel (docs/MANAGEMENT.md, "BOM edits"): `mgmt.bom.edit {project, refs, field, value}` sets one field on every
//! symbol of a line. Fields: `mpn` (the symbol's MPN field, or the alias it already has), `fab` (its `LCSC` field), `value`, `footprint`,
//! `dnp` ("yes"/"no"). Only standard symbol fields and attributes are written, which stock KiCad reads as they are.
//!
//! Where it is written:
//!   - the project's GUI KiCad has the schematic open: one commit over its API (BeginCommit, UpdateItems, EndCommit) as the hub's own
//!     client, unsaved; the panel offers its own undo (`undo: "panel"`: the old value, `was`, written back) until the edit can go as the
//!     person to a runtime that puts it in their Ctrl+Z (bp.13, `AS_PERSON`); the person saves it with the rest;
//!   - else through the headless KiCad, holding the schematic's named lock (one writer) for the edit and its save; refused, naming the
//!     holder, while another writer has it.
use super::{canonical as kc, embed, ipc, xprobe};
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock, Weak};

pub const FIELDS: &[&str] = &["mpn", "fab", "value", "footprint", "dnp"];
const MPN: [&str; 5] = ["mpn", "manufacturer_part_number", "manufacturer part number", "mfr pn", "pn"];

fn reference(it: &Value) -> &str { it["reference_field"]["text"]["text"].as_str().unwrap_or("") }
/// The user field to write for `field` on this item: the index of one it has (MPN by any of its aliases), or None to add one.
fn user_field(it: &Value, field: &str) -> Option<usize> {
    it["user_fields"].as_array()?.iter().position(|f| { let n = f["name"].as_str().unwrap_or("").to_ascii_lowercase(); if field == "mpn" { MPN.contains(&n.as_str()) } else { n == "lcsc" } })
}
/// A field's text now ("" when it has none).
pub fn current(it: &Value, field: &str) -> String {
    match field {
        "value" => it["value_field"]["text"]["text"].as_str().unwrap_or("").into(),
        "footprint" => it["footprint_field"]["text"]["text"].as_str().unwrap_or("").into(),
        "dnp" => if it["attributes"]["do_not_populate"].as_bool().unwrap_or(false) { "yes".into() } else { "no".into() },
        _ => user_field(it, field).map(|i| it["user_fields"][i]["text"]["text"].as_str().unwrap_or("").to_string()).unwrap_or_default(),
    }
}
/// One item with `field` set to `value` (GetItems' JSON back for UpdateItems). A new MPN or LCSC field is hidden, placed where the reference is.
pub fn set_field(mut it: Value, field: &str, value: &str) -> Value {
    match field {
        "value" => it["value_field"]["text"]["text"] = json!(value),
        "footprint" => it["footprint_field"]["text"]["text"] = json!(value),
        "dnp" => it["attributes"]["do_not_populate"] = json!(matches!(value, "yes" | "true" | "1")),
        _ => match user_field(&it, field) {
            Some(i) => it["user_fields"][i]["text"]["text"] = json!(value),
            None => {
                let name = if field == "mpn" { "MPN" } else { "LCSC" };
                let mut text = json!({"text": value});
                if let Some(p) = it["reference_field"]["text"].get("position") { text["position"] = p.clone(); }
                if let Some(a) = it["reference_field"]["text"].get("attributes") { text["attributes"] = a.clone(); }
                let f = json!({"name": name, "text": text, "visible": false});
                match it["user_fields"].as_array_mut() { Some(a) => a.push(f), None => it["user_fields"] = json!([f]) }
            }
        },
    }
    it
}
/// The items to update: the symbols (each unit) whose reference is one of `refs` and whose field is not `value` already.
pub fn plan(items: &[Value], refs: &[String], field: &str, value: &str) -> Vec<Value> {
    items.iter().filter(|it| refs.iter().any(|r| r == reference(it)) && current(it, field) != value).map(|it| set_field(it.clone(), field, value)).collect()
}
/// The request's checks: a known field, references that are references, a value without line breaks (and yes/no for DNP).
pub fn check(p: &Value) -> Result<(String, Vec<String>, String), String> {
    let field = p["field"].as_str().unwrap_or("");
    if !FIELDS.contains(&field) { return Err(format!("field is one of {}", FIELDS.join(", "))) }
    let refs: Vec<String> = p["refs"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|r| xprobe::is_reference(r)).map(str::to_string).collect();
    if refs.is_empty() { return Err("refs: the line's references".into()) }
    let value = p["value"].as_str().unwrap_or("").trim().to_string();
    if value.contains(['\n', '\r', '\0']) || value.len() > 200 { return Err("value: one line, 200 characters at most".into()) }
    if field == "dnp" && !matches!(value.as_str(), "yes" | "no") { return Err("dnp is yes or no".into()) }
    if field == "footprint" && !value.is_empty() && !value.contains(':') { return Err("footprint is Library:Name".into()) }
    Ok((field.to_string(), refs, value))
}
/// The commit's words in KiCad's undo list.
pub fn message(field: &str, refs: &[String], value: &str) -> String {
    let what = match field { "mpn" => "MPN", "fab" => "fab part number", "dnp" => "DNP", "value" => "value", _ => "footprint" };
    let rs = if refs.len() > 4 { format!("{} and {} more", refs[..3].join(", "), refs.len() - 3) } else { refs.join(", ") };
    format!("BOM: {what} of {rs} = {}", if value.is_empty() { "(none)" } else { value })
}

/// Whether the panel's edits go to the GUI as the person (`coedit::client_name(&Who::Person)`, "person:backplane-<pid>"): a co-editing
/// KiCad (bp.13 on) puts them in the person's own undo; before that the name is an actor of its own and the panel offers its own Undo.

pub const AS_PERSON: bool = true;
/// Who takes a GUI edit back: "kicad" on the runtimes where the e2e proved the person's Ctrl+Z takes the panel's commit back
/// (test/kicad_embed/embed_mgmt_e2e.sh M6: Backplane KiCad bp.10 on; bp.13 puts `person:` edits in the person's own undo by design),
/// else "panel". The panel offers its own Undo either way. By capability where there is one (`coedit`, bp.11 on), else the shared
/// version reader (kserve.rs version_ge) for bp.10.
pub fn undo_by(version: &str, caps: &[String]) -> &'static str {
    if caps.iter().any(|c| c == "coedit") || super::kserve::version_ge(version, "10.99.0-bp.10") { "kicad" } else { "panel" }
}

/// How many of an UpdateItems answer's items KiCad holds until the person finishes editing them (co-edit, bp.11 on: ISC_DEFERRED).
fn deferred_of(up: &Value) -> usize { up["updated_items"].as_array().map_or(0, |a| a.iter().filter(|r| r["status"]["code"] == "ISC_DEFERRED").count()) }
#[cfg(test)] #[test] fn deferred_items_are_counted() {
    assert_eq!(deferred_of(&json!({"updated_items": [{"status": {"code": "ISC_OK"}}, {"status": {"code": "ISC_DEFERRED", "error_message": "the person is editing this item"}}]})), 1);
    assert_eq!(deferred_of(&json!({})), 0);
}

/// Whether any of the project's KiCad frames is on screen (GetFrames' `shown`); a runtime without GetFrames counts as shown (never saved for the person).
async fn shown_to_person(gui: &embed::GuiRoute) -> bool {
    let o = embed::gui_call(gui, "kiapi.backplane.embed.GetFrames", json!({})).await;
    if o["ok"] != true { return true }
    embed::parse_get_frames(&o["message"]).2.iter().any(|f| f.shown)
}

static AGENTS: OnceLock<Weak<crate::agents::Agents>> = OnceLock::new();
/// The agents (for the schematic's named lock); main.rs gives them once they run.
pub fn set_agents(a: &Arc<crate::agents::Agents>) { let _ = AGENTS.set(Arc::downgrade(a)); }

/// The schematic the project's GUI KiCad has open. Err when it stays busy (a task running, AS_BUSY) past BUSY_WAIT: it may well have the
/// schematic open, so the headless path never writes the file behind its back; the panel shows the edit as waiting.
pub const BUSY_WAIT: std::time::Duration = std::time::Duration::from_secs(5);
pub const BUSY_PREFIX: &str = "waiting: ";
async fn gui_doc(pro: &str) -> Result<Option<(embed::GuiRoute, Value)>, String> {
    let Some(gui) = embed::gui_route(pro) else { return Ok(None) };
    let g = gui.clone();
    let doc = open_schematic(&move |ty, msg| { let g = g.clone(); async move { embed::gui_call(&g, ty, msg).await } }, BUSY_WAIT).await?;
    Ok(doc.map(|d| (gui, d)))
}
/// The open schematic through `call` (GetOpenDocuments), asked again every 250 ms while KiCad is busy, for at most `wait`.
async fn open_schematic<F, Fut>(call: &F, wait: std::time::Duration) -> Result<Option<Value>, String>
where F: Fn(&'static str, Value) -> Fut, Fut: std::future::Future<Output = Value> {
    let until = std::time::Instant::now() + wait;
    loop {
        let docs = call("kiapi.common.commands.GetOpenDocuments", json!({"type": "DOCTYPE_SCHEMATIC"})).await;
        match embed::open_docs(&docs) {
            embed::OpenDocs::Busy(_) if std::time::Instant::now() < until => tokio::time::sleep(std::time::Duration::from_millis(250)).await,
            embed::OpenDocs::Busy(why) => return Err(busy_text(&why)),
            embed::OpenDocs::Docs(d) => return Ok(d.first().cloned()),
            embed::OpenDocs::Unknown => return Ok(None),
        }
    }
}
#[cfg(test)] #[tokio::test] async fn a_busy_kicad_is_waited_for_then_refused_and_nothing_is_written() {
    let log = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let l = log.clone();
    let busy = move |ty: &'static str, _m: Value| { let l = l.clone(); async move { l.lock().unwrap().push(ty.to_string()); json!({"ok": false, "status": "AS_BUSY", "error": "a task is running"}) } };
    let t0 = std::time::Instant::now();
    let r = open_schematic(&busy, std::time::Duration::from_millis(600)).await;
    let e = r.expect_err("busy past the wait is a refusal, never 'no schematic open' (which would send the edit to the file)");
    assert!(e.starts_with(BUSY_PREFIX) && e.contains("a task is running"), "{e}");
    assert!(t0.elapsed() >= std::time::Duration::from_millis(600), "it waited");
    let asked = log.lock().unwrap().clone();
    assert!(asked.len() >= 2 && asked.iter().all(|t| t.ends_with("GetOpenDocuments")), "asked again while busy, and nothing else (no update, no save): {asked:?}");
    // KiCad free again within the wait: the schematic it has open
    let n = Arc::new(std::sync::atomic::AtomicUsize::new(0)); let n2 = n.clone();
    let later = move |_t: &'static str, _m: Value| { let n = n2.clone(); async move {
        if n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 { json!({"ok": false, "status": "AS_BUSY", "error": "x"}) }
        else { json!({"ok": true, "message": {"documents": [{"type": "DOCTYPE_SCHEMATIC"}]}}) } } };
    assert_eq!(open_schematic(&later, std::time::Duration::from_secs(5)).await.unwrap(), Some(json!({"type": "DOCTYPE_SCHEMATIC"})));
}
/// The refusal for a KiCad busy past the wait: what KiCad says it is doing, marked so the panel shows the edit as waiting, not failed.
pub fn busy_text(why: &str) -> String { format!("{BUSY_PREFIX}KiCad is busy ({why}); the change is not made yet: try again when it has finished") }
#[cfg(test)] #[test] fn a_busy_kicad_is_waiting_not_failed() {
    let t = busy_text("a task is running"); assert!(t.starts_with(BUSY_PREFIX) && t.contains("a task is running"), "{t}");
}

fn ok_of(v: &Value) -> Result<&Value, String> { if v["ok"] == true { Ok(&v["message"]) } else { Err(v["error"].as_str().or(v["why"].as_str()).unwrap_or("no answer").to_string()) } }

/// An EndCommit for the document the commit was begun on: its header names the document, or KiCad's API takes a header-less commit
/// request to the board editor (its pre-11 behaviour) and the schematic's commit is never ended.
pub fn end_msg(id: &Value, action: &str, message: &str, doc: &Value) -> Value {
    let mut m = json!({"id": id, "action": action, "header": {"document": doc}});
    if !message.is_empty() { m["message"] = json!(message) }
    m
}
/// The symbols to update (GetItems' own, changed) and the field's value before (the first changed symbol's: a line's symbols share it).
/// The schematic's sheet instances: the root document, then each sheet's (the parent's sheet path and the sheet's id), breadth first, at
/// most 400. A symbol in a sub-sheet is answered only for its own sheet's document: asked with the root's, a BOM edit of it found
/// nothing (complex_hierarchy's RV201, Nov 14).
async fn sheet_docs<F, Fut>(call: &F, root: &Value, root_file: Option<&str>) -> Vec<Value>
where F: Fn(&'static str, Value) -> Fut, Fut: std::future::Future<Output = Value> {
    // a headless KiCad's document has no sheet path: the root's is its file's uuid (a child is the root's path and the sheet's id)
    let mut root = root.clone();
    if root["sheet_path"]["path"].as_array().map_or(true, Vec::is_empty) {
        // the project's top-level sheet first (KiCad keys the hierarchy by it: Hubble_DK's differs from its file's uuid), else the file's
        let top = root_file.and_then(|f| std::fs::read_to_string(std::path::Path::new(f).with_extension("kicad_pro")).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["sheets"][0][0].as_str().map(str::to_string));
        if let Some(u) = top.or_else(|| root_file.and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| root_uuid(&t))) { root["sheet_path"]["path"] = json!([{"value": u}]); }
    }
    let mut docs = vec![root.clone()]; let mut i = 0;
    while i < docs.len() && docs.len() < 400 {
        let d = docs[i].clone(); i += 1;
        let got = call("kiapi.common.commands.GetItems", json!({"header": {"document": d}, "types": ["KOT_SCH_SHEET"]})).await;
        let Ok(m) = ok_of(&got) else { continue };
        let path: Vec<Value> = d["sheet_path"]["path"].as_array().cloned().unwrap_or_default();
        for sh in m["items"].as_array().into_iter().flatten() {
            let Some(id) = sh["id"]["value"].as_str() else { continue };
            if path.iter().any(|p| p["value"] == id) || path.len() > 32 { continue }
            let mut c = d.clone(); let mut p2 = path.clone(); p2.push(json!({"value": id}));
            c["sheet_path"] = json!({"path": p2});
            docs.push(c);
        }
    }
    docs
}
/// A schematic file's own uuid (its root sheet's): the first `(uuid ...)` at its top level.
pub fn root_uuid(text: &str) -> Option<String> {
    let i = text.find("\n\t(uuid \"")? + "\n\t(uuid \"".len(); text[i..].split('"').next().map(str::to_string)
}
/// What an edit does: per sheet document, the symbols to update (each symbol once: a sheet used several times has one symbol and several
/// instances, and KiCad keeps its fields per symbol, its Reference per instance); the field's value before; the references found; the
/// other instances' references that take the same value (`shared`).
pub struct Plan { pub todo: Vec<(Value, Vec<Value>)>, pub was: String, pub found: Vec<String>, pub shared: Vec<String> }
pub fn plan_sheets(sheets: &[(Value, Vec<Value>)], refs: &[String], field: &str, value: &str) -> Plan {
    use std::collections::{BTreeSet, HashMap};
    let id = |it: &Value| it["id"]["value"].as_str().unwrap_or("").to_string();
    let mut refs_of: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (_, items) in sheets { for it in items { refs_of.entry(id(it)).or_default().insert(reference(it).to_string()); } }
    let (mut todo, mut done, mut found, mut shared, mut was) = (vec![], BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), String::new());
    for (doc, items) in sheets {
        let mut mine = vec![];
        for it in items.iter().filter(|it| refs.iter().any(|r| r == reference(it))) {
            found.insert(reference(it).to_string());
            for r in refs_of.get(&id(it)).into_iter().flatten() { if !refs.contains(r) { shared.insert(r.clone()); } }
            if current(it, field) != value && done.insert(id(it)) { if was.is_empty() { was = current(it, field); } mine.push(set_field(it.clone(), field, value)); }
        }
        if !mine.is_empty() { todo.push((doc.clone(), mine)); }
    }
    Plan { todo, was, found: found.into_iter().collect(), shared: shared.into_iter().collect() }
}
/// Every sheet instance's symbols, read through `call`.
async fn read_sheets<F, Fut>(call: &F, root: &Value, root_file: Option<&str>) -> Result<Vec<(Value, Vec<Value>)>, String>
where F: Fn(&'static str, Value) -> Fut, Fut: std::future::Future<Output = Value> {
    let mut out = vec![];
    for d in sheet_docs(call, root, root_file).await {
        let got = call("kiapi.common.commands.GetItems", json!({"header": {"document": d}, "types": ["KOT_SCH_SYMBOL"]})).await;
        out.push((d, ok_of(&got)?["items"].as_array().cloned().unwrap_or_default()));
    }
    Ok(out)
}
async fn prepare<F, Fut>(call: &F, root: &Value, root_file: Option<&str>, refs: &[String], field: &str, value: &str) -> Result<Plan, String>
where F: Fn(&'static str, Value) -> Fut, Fut: std::future::Future<Output = Value> {
    let p = plan_sheets(&read_sheets(call, root, root_file).await?, refs, field, value);
    let missing: Vec<&String> = refs.iter().filter(|r| !p.found.contains(r)).collect();
    if !missing.is_empty() { return Err(format!("no symbol {} in the schematic (none of its sheets has it)", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))) }
    Ok(p)
}
/// After the edit, in KiCad: every reference's field reads `value`, else which do not (the edit is never said made when it is not).
async fn read_back<F, Fut>(call: &F, root: &Value, root_file: Option<&str>, refs: &[String], field: &str, value: &str) -> Result<(), String>
where F: Fn(&'static str, Value) -> Fut, Fut: std::future::Future<Output = Value> {
    let sheets = read_sheets(call, root, root_file).await?;
    let wrong: Vec<String> = refs.iter().filter(|r| !sheets.iter().any(|(_, items)| items.iter().any(|it| reference(it) == r.as_str() && current(it, field) == value))).cloned().collect();
    if wrong.is_empty() { Ok(()) } else { Err(format!("KiCad did not take the change for {}", wrong.join(", "))) }
}
/// The saved files: whether every reference's symbol carries `field` = `value` in the schematic's sheet files (the file read back, never
/// the API's word alone). A symbol block is found by its instance's `(reference "R")`; its field by the property's name.
pub fn file_shows(texts: &[String], reference: &str, field: &str, value: &str) -> bool {
    let esc = value.replace('\\', "\\\\").replace('"', "\\\"");
    texts.iter().any(|t| symbol_blocks(t).into_iter().any(|b| b.contains(&format!("(reference \"{reference}\")")) && block_has(b, field, &esc, value)))
}
fn block_has(b: &str, field: &str, esc: &str, value: &str) -> bool {
    match field {
        "dnp" => if matches!(value, "yes" | "true" | "1") { b.contains("(dnp yes)") } else { !b.contains("(dnp yes)") },
        "value" => b.contains(&format!("(property \"Value\" \"{esc}\"")),
        "footprint" => b.contains(&format!("(property \"Footprint\" \"{esc}\"")),
        _ => {
            let names: &[&str] = if field == "mpn" { &MPN } else { &["lcsc"] };
            b.match_indices("(property \"").any(|(i, _)| { let rest = &b[i + 11..]; let Some(e) = rest.find('"') else { return false };
                names.contains(&rest[..e].to_ascii_lowercase().as_str()) && rest[e..].starts_with(&format!("\" \"{esc}\"")) })
        }
    }
}
/// The schematic file's placed symbols: each `(symbol` block at the sheet's top level (not the library's, inside `(lib_symbols`).
fn symbol_blocks(t: &str) -> Vec<&str> {
    let lib = t.find("(lib_symbols").map(|i| (i, i + match_close(&t[i..]))).unwrap_or((0, 0));
    let mut out = vec![]; let mut from = 0;
    while let Some(k) = t[from..].find("(symbol") {
        let i = from + k; from = i + 7;
        if i >= lib.0 && i < lib.1 { continue }
        // a placed symbol opens with its lib_id, or with a lib_name before it (a symbol whose library copy was renamed: ecc83's R1)
        let head = t[i + 7..].trim_start(); if !head.starts_with("(lib_id") && !head.starts_with("(lib_name") { continue }
        let n = match_close(&t[i..]); out.push(&t[i..i + n]); from = i + n;
    }
    out
}
/// The length of the balanced parenthesised expression at the start of `t` (strings with escapes skipped).
fn match_close(t: &str) -> usize {
    let (mut depth, mut instr, mut esc) = (0i32, false, false);
    for (i, c) in t.char_indices() {
        if instr { if esc { esc = false } else if c == '\\' { esc = true } else if c == '"' { instr = false } continue }
        match c { '"' => instr = true, '(' => depth += 1, ')' => { depth -= 1; if depth == 0 { return i + 1 } } _ => {} }
    }
    t.len()
}
/// The schematic's sheet files (the root and every .kicad_sch beside it).
fn sheet_texts(sch: &str) -> Vec<String> { super::savegate::files_of(sch).iter().filter_map(|f| std::fs::read_to_string(f).ok()).collect() }
/// What the panel says when other instances take the value too.
pub fn shared_note(refs: &[String], shared: &[String]) -> Option<String> {
    (!shared.is_empty()).then(|| format!("{} sits in a sheet used more than once, and KiCad keeps one value per symbol there: {} has it too", refs.join(", "), shared.join(", ")))
}
/// Commit `todo` per sheet document on one API (the headless one through ipc::kicad_ipc).
async fn commit<F, Fut>(call: &F, todo: &[(Value, Vec<Value>)], field: &str, refs: &[String], value: &str) -> Result<usize, String>
where F: Fn(&'static str, Value) -> Fut, Fut: std::future::Future<Output = Value> {
    let mut n = 0;
    for (doc, items) in todo {
        let begun = call("kiapi.common.commands.BeginCommit", json!({"header": {"document": doc}})).await;
        let id = ok_of(&begun)?["id"].clone();
        // ended on every path: committed, or dropped when anything fails (a commit left open holds the editor's handler for that client)
        let up = call("kiapi.common.commands.UpdateItems", json!({"header": {"document": doc}, "items": items})).await;
        if let Err(e) = ok_of(&up) { call("kiapi.common.commands.EndCommit", end_msg(&id, "CMA_DROP", "", doc)).await; return Err(format!("UpdateItems: {e}")) }
        let end = call("kiapi.common.commands.EndCommit", end_msg(&id, "CMA_COMMIT", &message(field, refs, value), doc)).await;
        if let Err(e) = ok_of(&end) { call("kiapi.common.commands.EndCommit", end_msg(&id, "CMA_DROP", "", doc)).await; return Err(format!("EndCommit: {e}")) }
        n += items.len();
    }
    Ok(n)
}

/// `mgmt.bom.edit`: {via: "kicad"|"file", changed}.
pub async fn edit(hub: &Arc<Hub>, pro: &str, sch: &str, p: &Value) -> Result<Value, String> {
    if crate::copy::on() { return Err("a hub on a copied log (BACKPLANE_COPY=1) writes no design".into()) }
    let (field, refs, value) = check(p)?;
    if let Some((gui, doc)) = gui_doc(pro).await? {
        // as the person, never an agent's name (an actor of its own before bp.13, their own Ctrl+Z from it);
        // BACKPLANE_BOM_CLIENT=hub: the tests' run as the hub's own actor, to compare what KiCad's undo does with each
        let as_person = AS_PERSON && std::env::var("BACKPLANE_BOM_CLIENT").map_or(true, |v| v != "hub");
        let who = super::coedit::client_name(&if as_person { super::coedit::Who::Person } else { super::coedit::Who::Hub("bom") });
        let g = gui.clone(); let w2 = who.clone();
        let call = move |ty: &'static str, msg: Value| { let (g, w) = (g.clone(), w2.clone()); async move { embed::gui_call_as(&g, ty, msg, &w).await } };
        let root_file = super::project::read(&std::path::Path::new(pro).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()).sch;
        let plan = prepare(&call, &doc, Some(&root_file), &refs, &field, &value).await?;
        let (todo, was) = (plan.todo.iter().map(|(_, i)| i.len()).sum::<usize>(), plan.was.clone());
        // one commit per sheet document, ended or dropped on every path, with that document's header and this name on every call
        let mut deferred = 0;
        let mut saved = false;
        let mut upgrade = None;
        if todo > 0 {
            for (d, items) in &plan.todo { deferred += deferred_of(&embed::commit_as(&gui, &who, d, items.clone(), &message(&field, &refs, &value)).await?); }
            // KiCad's word for it: every reference reads the value now (a deferred edit is applied when the person finishes there)
            if deferred == 0 { read_back(&call, &doc, Some(&root_file), &refs, &field, &value).await?; }
            // a KiCad the person has not on screen (pre-warmed, or its view closed): nobody would save the change there, and it would go with
            // that KiCad; it is saved at once, as the headless path does
            // (co-editing runtimes only: there a save of the person's own unsaved edits too is by design; before, it is never done for them)
            if deferred == 0 && gui.caps.iter().any(|c| c == "coedit") && !shown_to_person(&gui).await {
                let sch = doc["board_filename"].as_str().map(str::to_string).unwrap_or_default();
                let sch = if sch.is_empty() { super::project::read(&std::path::Path::new(pro).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()).sch } else { sch };
                upgrade = super::savegate::upgrade_of(&gui, &sch).await;
                saved = embed::gui_call_as(&gui, "kiapi.common.commands.SaveDocument", json!({"document": doc}), &who).await["ok"] == true;
                if !saved { upgrade = None; }
                // saved only when the saved files show it
                if saved { let (t, r, f, v) = (sheet_texts(&sch), refs.clone(), field.clone(), value.clone());
                    let ok = tokio::task::spawn_blocking(move || r.iter().all(|x| file_shows(&t, x, &f, &v))).await.unwrap_or(false);
                    if !ok { return Err(format!("KiCad saved the schematic, but the saved file does not show {} = {value} for {}: not kept", field, refs.join(", "))) } }
            }
            if !saved { embed::unsaved_update(pro, |u| u.touched()); }
        }
        // `deferred`: the person is editing those symbols in KiCad; KiCad applies the change when they finish (a notice follows): not read back yet
        return Ok(json!({"via": if saved { "file" } else { "kicad" }, "format_upgrade": upgrade, "changed": todo, "deferred": deferred, "field": field, "value": value, "was": was, "refs": refs, "shared_with": plan.shared, "shared_note": shared_note(&refs, &plan.shared), "undo": undo_by(&gui.version, &gui.caps)}))
    }
    // no KiCad with it open (none, or one with only the board): the headless one writes and saves, under the schematic's lock
    let ag = AGENTS.get().and_then(Weak::upgrade).ok_or("the hub's agents are not running")?;
    let doc_path = kc::route(sch);
    let name = kc::doc_lock(&doc_path);
    let holder = format!("pid:{}:bom", std::process::id());
    let got = ag.lock_http("acquire", &json!({"name": name, "holder": holder, "note": "BOM edit"})).await;
    if got["held"] != true {
        ag.lock_http("cancel", &json!({"name": name, "holder": holder})).await;
        let who = got["holder"].as_str().or(got["owner"].as_str()).unwrap_or("another writer");
        return Err(format!("the schematic is being edited by {who}; try again when it is free"))
    }
    // kicad_ipc answers {type, message}: the calls below read the message
    let r = async {
        let path = doc_path.clone(); let h = hub.clone();
        let call = move |ty: &'static str, msg: Value| { let (h, path) = (h.clone(), path.clone()); async move { match ipc::kicad_ipc_headless(&h, &path, ty, msg).await { Ok(v) => json!({"ok": true, "message": v["message"]}), Err(e) => json!({"ok": false, "error": e}) } } };
        // a headless server with nothing open handles no document command, GetOpenDocuments included: it opens the schematic
        let docs = call("kiapi.common.commands.GetOpenDocuments", json!({"type": "DOCTYPE_SCHEMATIC"})).await;
        let doc = match ok_of(&docs).ok().and_then(|m| m["documents"].as_array()).and_then(|d| d.first()).cloned() {
            Some(d) => d,
            None => {
                let o = call("kiapi.common.commands.OpenDocument", json!({"type": "DOCTYPE_SCHEMATIC", "path": doc_path})).await;
                let d = ok_of(&o).map_err(|e| format!("OpenDocument: {e}"))?["document"].clone();
                if !d.is_object() { return Err(format!("the headless KiCad opened no schematic: {o}")) }
                d
            }
        };
        let plan = prepare(&call, &doc, Some(&doc_path), &refs, &field, &value).await?;
        let n = commit(&call, &plan.todo, &field, &refs, &value).await?;
        if n > 0 {
            read_back(&call, &doc, Some(&doc_path), &refs, &field, &value).await?;
            ok_of(&call("kiapi.common.commands.SaveDocument", json!({"document": doc})).await).map_err(|e| format!("SaveDocument: {e}"))?;
            let (t, r, f, v) = (sheet_texts(&doc_path), refs.clone(), field.clone(), value.clone());
            if !tokio::task::spawn_blocking(move || r.iter().all(|x| file_shows(&t, x, &f, &v))).await.unwrap_or(false) {
                return Err(format!("the schematic was saved, but the saved file does not show {field} = {value} for {}: not kept", refs.join(", ")))
            }
        }
        Ok::<(usize, String, Vec<String>), String>((n, plan.was, plan.shared))
    }.await;
    ag.lock_http("release", &json!({"name": name, "holder": holder})).await;
    let (n, was, shared) = r?;
    Ok(json!({"via": "file", "changed": n, "field": field, "value": value, "was": was, "refs": refs, "shared_with": shared, "shared_note": shared_note(&refs, &shared), "undo": "panel"}))
}

pub struct BomEditRpc;
impl crate::api::RpcHandler for BomEditRpc {
    fn methods(&self) -> &'static [&'static str] { &["mgmt.bom.edit", "mgmt.bom.alts"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a crate::api::Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let err = |e: &str| vec![json!({"t":"reply","id":id,"ok":false,"error":e})];
            let pro = match embed::resolve_pro(p["project"].as_str().unwrap_or("")) { Ok(p) => p, Err(e) => return err(&e) };
            let dir = std::path::Path::new(&pro).parent().and_then(|p| p.to_str()).unwrap_or("").to_string();
            if !super::known_root(hub, &dir, "").await { return err("not in a project this hub knows") }
            if _m == "mgmt.bom.alts" {
                let fab = p["fab"].as_str().unwrap_or("").trim().to_ascii_uppercase();
                if fab.is_empty() || !fab.starts_with('C') { return err("fab: the line's fab part number") }
                return match super::parts::alternatives(hub, &fab, p["need"].as_u64().unwrap_or(1)).await { Ok(v) => vec![json!({"t":"reply","id":id,"ok":true,"result":v})], Err(e) => err(&e) }
            }
            let sch = super::bompanel::sch_of(&pro);
            match edit(hub, &pro, &sch, p).await {
                Ok(v) => { let (h, pr) = (hub.clone(), pro.clone()); tokio::spawn(async move { super::bompanel::refresh(&h, &pr).await }); vec![json!({"t":"reply","id":id,"ok":true,"result":v})] }
                Err(e) => err(&e),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f(n: &str, t: &str) -> Value { json!({"name": n, "text": {"position": {"x_nm": "5"}, "text": t}}) }
    fn sym(r: &str, unit: u64, extra: Vec<Value>) -> Value { json!({"@type": "type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance", "id": {"value": format!("{r}-{unit}")}, "reference_field": f("Reference", r), "value_field": f("Value", "10k"), "footprint_field": f("Footprint", "R:R_0603"), "unit": {"unit": unit}, "user_fields": extra}) }
    #[test] fn an_edit_maps_to_the_symbols_of_the_line() {
        let items = vec![sym("R1", 1, vec![]), sym("R2", 1, vec![f("MPN", "OLD")]), sym("R3", 1, vec![f("Manufacturer Part Number", "X")]), sym("U1", 1, vec![]), sym("U1", 2, vec![])];
        let refs = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // R1 gains an MPN field (hidden, at its reference), R2's is changed, R3's alias is the one written
        let todo = plan(&items, &refs(&["R1", "R2", "R3"]), "mpn", "RC0603");
        assert_eq!(todo.len(), 3);
        assert_eq!(todo[0]["user_fields"], json!([{"name": "MPN", "text": {"text": "RC0603", "position": {"x_nm": "5"}}, "visible": false}]));
        assert_eq!(todo[1]["user_fields"][0]["text"]["text"], "RC0603"); assert_eq!(todo[2]["user_fields"][0]["name"], "Manufacturer Part Number");
        assert_eq!(todo[0]["@type"], items[0]["@type"], "the item goes back as it came, its type and id kept"); assert_eq!(todo[0]["id"], items[0]["id"]);
        // a value it already has is no change
        assert!(plan(&todo, &refs(&["R1"]), "mpn", "RC0603").is_empty());
        // every unit of a multi-unit part
        assert_eq!(plan(&items, &refs(&["U1"]), "value", "LM358").len(), 2);
        let d = plan(&items, &refs(&["U1"]), "dnp", "yes"); assert_eq!(d[0]["attributes"]["do_not_populate"], true); assert_eq!(current(&d[0], "dnp"), "yes");
        let fab = plan(&items, &refs(&["R1"]), "fab", "C25804"); assert_eq!(fab[0]["user_fields"][0]["name"], "LCSC");
        assert_eq!(plan(&items, &refs(&["R1"]), "footprint", "R:R_0402")[0]["footprint_field"]["text"]["text"], "R:R_0402");
    }
    /// A stand-in API: records each call, answers ok, or fails the named command.
    fn fake(fail: &'static str, log: Arc<std::sync::Mutex<Vec<(String, Value)>>>) -> impl Fn(&'static str, Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Value> + Send>> {
        move |ty, msg| { let log = log.clone(); Box::pin(async move {
            log.lock().unwrap().push((ty.to_string(), msg));
            if ty.ends_with(fail) && !fail.is_empty() && log.lock().unwrap().iter().filter(|(t, _)| t.ends_with(fail)).count() == 1 { return json!({"ok": false, "error": "no"}) }
            match ty { "kiapi.common.commands.GetItems" => json!({"ok": true, "message": {"items": [sym("R1", 1, vec![])]}}), "kiapi.common.commands.BeginCommit" => json!({"ok": true, "message": {"id": {"value": "c1"}}}), _ => json!({"ok": true, "message": {}}) }
        }) }
    }
    #[tokio::test] async fn a_commit_is_always_ended_on_the_schematic() {
        let doc = json!({"type": "DOCTYPE_SCHEMATIC", "sheet_path": {}});
        let refs = vec!["R1".to_string()];
        for (fail, ends) in [("", vec!["CMA_COMMIT"]), ("UpdateItems", vec!["CMA_DROP"]), ("EndCommit", vec!["CMA_COMMIT", "CMA_DROP"])] {
            let log = Arc::new(std::sync::Mutex::new(vec![]));
            let todo = vec![(doc.clone(), vec![sym("R1", 1, vec![])])];
            let r = commit(&fake(fail, log.clone()), &todo, "mpn", &refs, "X").await;
            assert_eq!(r.is_ok(), fail.is_empty(), "{fail}: {r:?}");
            let got: Vec<(String, Value)> = log.lock().unwrap().iter().filter(|(t, _)| t.ends_with("EndCommit")).cloned().collect();
            assert_eq!(got.iter().map(|(_, m)| m["action"].as_str().unwrap().to_string()).collect::<Vec<_>>(), ends, "{fail}");
            assert!(got.iter().all(|(_, m)| m["header"]["document"] == doc && m["id"]["value"] == "c1"), "every EndCommit names the schematic: {got:?}");
        }
    }
    /// A sheet used twice: one symbol (one id), an instance per sheet with its own reference; the edit writes the symbol once, through
    /// one sheet's document, and names the other instance as sharing it. A symbol in a sheet used once is found under that sheet.
    #[test] fn a_symbol_in_a_sheet_used_twice_is_written_once_and_shares_its_value() {
        let r = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let inst = |rf: &str, id: &str| { let mut s = sym(rf, 1, vec![]); s["id"] = json!({"value": id}); s };
        let (root, a, b, c) = (json!({"sheet_path": {"path": [{"value": "root"}]}}), json!({"sheet_path": {"path": [{"value": "root"}, {"value": "A"}]}}), json!({"sheet_path": {"path": [{"value": "root"}, {"value": "B"}]}}), json!({"sheet_path": {"path": [{"value": "root"}, {"value": "C"}]}}));
        let sheets = vec![(root.clone(), vec![inst("C103", "c103")]), (a.clone(), vec![inst("RV201", "rv")]), (b.clone(), vec![inst("RV301", "rv")]), (c.clone(), vec![inst("R7", "r7")])];
        let p = plan_sheets(&sheets, &r(&["RV201"]), "mpn", "X");
        assert_eq!(p.todo.len(), 1, "the shared symbol once"); assert_eq!(p.todo[0].0, a, "through the sheet of the instance edited");
        assert_eq!(p.shared, r(&["RV301"])); assert!(shared_note(&r(&["RV201"]), &p.shared).unwrap().contains("RV301 has it too"));
        let both = plan_sheets(&sheets, &r(&["RV201", "RV301"]), "mpn", "X"); assert_eq!(both.todo.iter().map(|t| t.1.len()).sum::<usize>(), 1); assert!(both.shared.is_empty());
        let once = plan_sheets(&sheets, &r(&["R7"]), "mpn", "X"); assert_eq!(once.todo[0].0, c, "a sheet used once: its own document"); assert!(once.shared.is_empty());
        let flat = plan_sheets(&sheets, &r(&["C103"]), "mpn", "X"); assert_eq!(flat.todo[0].0, root);
        assert!(plan_sheets(&sheets, &r(&["Q9"]), "mpn", "X").found.is_empty(), "not there: nothing found (the edit refuses, naming it)");
    }
    /// The saved file is read back: the symbol whose instance is RV201 carries the MPN (in a shared sheet, one property for both instances).
    #[test] fn the_saved_file_is_read_for_the_value() {
        let t = "(kicad_sch\n\t(lib_symbols\n\t\t(symbol \"Device:R\"\n\t\t\t(property \"Reference\" \"R\")\n\t\t)\n\t)\n\t(symbol\n\t\t(lib_id \"Device:R_POT\")\n\t\t(property \"Reference\" \"RV201\")\n\t\t(property \"MPN\" \"X1\"\n\t\t\t(hide yes)\n\t\t)\n\t\t(dnp yes)\n\t\t(instances\n\t\t\t(project \"p\"\n\t\t\t\t(path \"/a\" (reference \"RV201\") (unit 1))\n\t\t\t\t(path \"/b\" (reference \"RV301\") (unit 1))\n\t\t\t)\n\t\t)\n\t)\n)".to_string();
        let ts = vec![t];
        assert!(file_shows(&ts, "RV201", "mpn", "X1") && file_shows(&ts, "RV301", "mpn", "X1"), "one property, both instances");
        assert!(!file_shows(&ts, "RV201", "mpn", "X2")); assert!(!file_shows(&ts, "R9", "mpn", "X1"));
        assert!(file_shows(&ts, "RV201", "dnp", "yes") && !file_shows(&ts, "RV201", "dnp", "no"));
        assert!(!file_shows(&ts, "R", "mpn", "X1"), "the library's own symbol is not a placed one");
        let named = vec![ts[0].replace("(lib_id \"Device:R_POT\")", "(lib_name \"R_1\")\n\t\t(lib_id \"Device:R_POT\")")];
        assert!(file_shows(&named, "RV201", "mpn", "X1"), "a placed symbol with a lib_name first");
        assert_eq!(root_uuid("(kicad_sch\n\t(version 1)\n\t(uuid \"5b96\")\n\t(symbol (uuid \"x\"))"), Some("5b96".to_string()));
    }
    #[test] fn requests_are_checked() {
        assert!(check(&json!({"field": "colour", "refs": ["R1"], "value": "x"})).is_err());
        assert!(check(&json!({"field": "mpn", "refs": ["#PWR01", "x y"], "value": "x"})).is_err());
        assert!(check(&json!({"field": "mpn", "refs": ["R1"], "value": "a\nb"})).is_err());
        assert!(check(&json!({"field": "dnp", "refs": ["R1"], "value": "maybe"})).is_err());
        assert!(check(&json!({"field": "footprint", "refs": ["R1"], "value": "R_0603"})).is_err());
        assert_eq!(check(&json!({"field": "mpn", "refs": ["R1", "bad ref", "R2"], "value": " RC0603 "})).unwrap(), ("mpn".into(), vec!["R1".into(), "R2".into()], "RC0603".into()));
        assert_eq!(message("mpn", &["R1".into(), "R2".into()], "RC0603"), "BOM: MPN of R1, R2 = RC0603");
        for v in ["10.99.0-bp.10", "10.99.0-bp.11", "10.99.0-bp.12", "10.99.0-bp.12-851cbefda6", "10.99.0-bp.13"] { assert_eq!(undo_by(v, &[]), "kicad", "{v}"); }
        for v in ["10.99.0-bp.9", "10.0.6", ""] { assert_eq!(undo_by(v, &[]), "panel", "{v}"); }
        // a dev build's describe string (Boat's bp.13 batch 3) is decided by its capability, whatever the version reader makes of it
        assert_eq!(undo_by("backplane-kicad-v10.99.0-bp.7-15-g8d903542-dirty-bp.13", &["coedit".to_string()]), "kicad");
        assert_eq!(message("dnp", &["R1".into(), "R2".into(), "R3".into(), "R4".into(), "R5".into()], "yes"), "BOM: DNP of R1, R2, R3 and 2 more = yes");
    }
}
