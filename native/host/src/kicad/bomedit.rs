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
/// (test/kicad_embed/embed_mgmt_e2e.sh M6: Backplane KiCad bp.10, bp.11, bp.12; bp.13 puts `person:` edits in the person's own undo by
/// design), else "panel". The panel offers its own Undo either way.
pub fn undo_by(version: &str, _as_person: bool) -> &'static str {
    let bp = version.split("-bp.").nth(1).and_then(|t| t.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
    if bp >= 10 { "kicad" } else { "panel" }
}

static AGENTS: OnceLock<Weak<crate::agents::Agents>> = OnceLock::new();
/// The agents (for the schematic's named lock); main.rs gives them once they run.
pub fn set_agents(a: &Arc<crate::agents::Agents>) { let _ = AGENTS.set(Arc::downgrade(a)); }

/// The schematic open in the project's GUI KiCad: its document specifier, None when it has none open.
async fn gui_doc(pro: &str) -> Option<(embed::GuiRoute, Value)> {
    let gui = embed::gui_route(pro)?;
    let docs = embed::gui_call(&gui, "kiapi.common.commands.GetOpenDocuments", json!({"type": "DOCTYPE_SCHEMATIC"})).await;
    let doc = docs["message"]["documents"].as_array()?.first()?.clone();
    Some((gui, doc))
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
async fn prepare<F, Fut>(call: &F, doc: &Value, refs: &[String], field: &str, value: &str) -> Result<(Vec<Value>, String), String>
where F: Fn(&'static str, Value) -> Fut, Fut: std::future::Future<Output = Value> {
    let got = call("kiapi.common.commands.GetItems", json!({"header": {"document": doc}, "types": ["KOT_SCH_SYMBOL"]})).await;
    let items = ok_of(&got)?["items"].as_array().cloned().unwrap_or_default();
    let was = items.iter().find(|it| refs.iter().any(|r| r == reference(it)) && current(it, field) != value).map(|it| current(it, field)).unwrap_or_default();
    Ok((plan(&items, refs, field, value), was))
}
/// Get, change and commit on one API (the GUI's, or the headless one through ipc::kicad_ipc). Answers how many symbols changed and the
/// field's value before (the first changed symbol's: a line's symbols share it).
async fn commit<F, Fut>(call: F, doc: Value, refs: &[String], field: &str, value: &str) -> Result<(usize, String), String>
where F: Fn(&'static str, Value) -> Fut, Fut: std::future::Future<Output = Value> {
    let (todo, was) = prepare(&call, &doc, refs, field, value).await?;
    if todo.is_empty() { return Ok((0, was)) }
    let begun = call("kiapi.common.commands.BeginCommit", json!({"header": {"document": doc}})).await;
    let id = ok_of(&begun)?["id"].clone();
    // from here the commit is ended on every path: committed, or dropped when anything fails (a commit left open holds the editor's
    // handler for that client)
    let up = call("kiapi.common.commands.UpdateItems", json!({"header": {"document": doc}, "items": todo})).await;
    if let Err(e) = ok_of(&up) { call("kiapi.common.commands.EndCommit", end_msg(&id, "CMA_DROP", "", &doc)).await; return Err(format!("UpdateItems: {e}")) }
    let end = call("kiapi.common.commands.EndCommit", end_msg(&id, "CMA_COMMIT", &message(field, refs, value), &doc)).await;
    if let Err(e) = ok_of(&end) { call("kiapi.common.commands.EndCommit", end_msg(&id, "CMA_DROP", "", &doc)).await; return Err(format!("EndCommit: {e}")) }
    Ok((todo.len(), was))
}

/// `mgmt.bom.edit`: {via: "kicad"|"file", changed}.
pub async fn edit(hub: &Arc<Hub>, pro: &str, sch: &str, p: &Value) -> Result<Value, String> {
    if crate::copy::on() { return Err("a hub on a copied log (BACKPLANE_COPY=1) writes no design".into()) }
    let (field, refs, value) = check(p)?;
    if let Some((gui, doc)) = gui_doc(pro).await {
        // as the person, never an agent's name (an actor of its own before bp.13, their own Ctrl+Z from it);
        // BACKPLANE_BOM_CLIENT=hub: the tests' run as the hub's own actor, to compare what KiCad's undo does with each
        let as_person = AS_PERSON && std::env::var("BACKPLANE_BOM_CLIENT").map_or(true, |v| v != "hub");
        let who = super::coedit::client_name(&if as_person { super::coedit::Who::Person } else { super::coedit::Who::Hub("bom") });
        let g = gui.clone(); let w2 = who.clone();
        let (todo, was) = prepare(&move |ty, msg| { let (g, w) = (g.clone(), w2.clone()); async move { embed::gui_call_as(&g, ty, msg, &w).await } }, &doc, &refs, &field, &value).await?;
        // one commit, ended or dropped on every path, with the document's header and this name on every call (embed::commit_as)
        if !todo.is_empty() { embed::commit_as(&gui, &who, &doc, todo.clone(), &message(&field, &refs, &value)).await?; embed::unsaved_update(pro, |u| u.touched()); }
        return Ok(json!({"via": "kicad", "changed": todo.len(), "field": field, "value": value, "was": was, "refs": refs, "undo": undo_by(&gui.version, as_person)}))
    }
    // no KiCad with it open: the headless one writes and saves, under the schematic's lock
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
    let r = async {
        let path = doc_path.clone(); let h = hub.clone();
        let call = move |ty: &'static str, msg: Value| { let (h, path) = (h.clone(), path.clone()); async move { match ipc::kicad_ipc(&h, &path, ty, msg).await { Ok(v) => json!({"ok": true, "message": v}), Err(e) => json!({"ok": false, "error": e}) } } };
        let docs = call("kiapi.common.commands.GetOpenDocuments", json!({"type": "DOCTYPE_SCHEMATIC"})).await;
        let doc = ok_of(&docs)?["documents"].as_array().and_then(|d| d.first()).cloned().ok_or("the headless KiCad has no schematic open")?;
        let n = commit(&call, doc.clone(), &refs, &field, &value).await?;
        if n.0 > 0 { ok_of(&call("kiapi.common.commands.SaveDocument", json!({"document": doc})).await).map_err(|e| format!("SaveDocument: {e}"))?; }
        Ok::<(usize, String), String>(n)
    }.await;
    ag.lock_http("release", &json!({"name": name, "holder": holder})).await;
    let (n, was) = r?;
    Ok(json!({"via": "file", "changed": n, "field": field, "value": value, "was": was, "refs": refs, "undo": "panel"}))
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
            let r = commit(fake(fail, log.clone()), doc.clone(), &refs, "mpn", "X").await;
            assert_eq!(r.is_ok(), fail.is_empty(), "{fail}: {r:?}");
            let got: Vec<(String, Value)> = log.lock().unwrap().iter().filter(|(t, _)| t.ends_with("EndCommit")).cloned().collect();
            assert_eq!(got.iter().map(|(_, m)| m["action"].as_str().unwrap().to_string()).collect::<Vec<_>>(), ends, "{fail}");
            assert!(got.iter().all(|(_, m)| m["header"]["document"] == doc && m["id"]["value"] == "c1"), "every EndCommit names the schematic: {got:?}");
        }
    }
    #[test] fn requests_are_checked() {
        assert!(check(&json!({"field": "colour", "refs": ["R1"], "value": "x"})).is_err());
        assert!(check(&json!({"field": "mpn", "refs": ["#PWR01", "x y"], "value": "x"})).is_err());
        assert!(check(&json!({"field": "mpn", "refs": ["R1"], "value": "a\nb"})).is_err());
        assert!(check(&json!({"field": "dnp", "refs": ["R1"], "value": "maybe"})).is_err());
        assert!(check(&json!({"field": "footprint", "refs": ["R1"], "value": "R_0603"})).is_err());
        assert_eq!(check(&json!({"field": "mpn", "refs": ["R1", "bad ref", "R2"], "value": " RC0603 "})).unwrap(), ("mpn".into(), vec!["R1".into(), "R2".into()], "RC0603".into()));
        assert_eq!(message("mpn", &["R1".into(), "R2".into()], "RC0603"), "BOM: MPN of R1, R2 = RC0603");
        for v in ["10.99.0-bp.10", "10.99.0-bp.11", "10.99.0-bp.12", "10.99.0-bp.12-851cbefda6", "10.99.0-bp.13"] { assert_eq!(undo_by(v, true), "kicad", "{v}"); assert_eq!(undo_by(v, false), "kicad", "{v}"); }
        for v in ["10.99.0-bp.9", "10.0.6", ""] { assert_eq!(undo_by(v, true), "panel", "{v}"); }
        assert_eq!(message("dnp", &["R1".into(), "R2".into(), "R3".into(), "R4".into(), "R5".into()], "yes"), "BOM: DNP of R1, R2, R3 and 2 more = yes");
    }
}
