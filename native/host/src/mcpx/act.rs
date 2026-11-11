//! Settings > MCP's presses that change the list (page.rs draws, `step` moves the page), and the requests `mcp.page` / `mcp.page.act` the phones
//! and the desktop's presenter send. A press is the person's own: it needs no confirm (an agent's change does: tools.rs).
use super::{import, model::Server, page::{self, Ui}, svc::{self, Mcpx}};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Map, Value};
use std::sync::Arc;

fn split3(v: &str) -> (&str, &str, &str) { let mut it = v.splitn(3, '|'); (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or("")) }
fn cleared(view: &str) -> Map<String, Value> {
    let mut m = Map::new(); m.insert("view".into(), json!(view)); m.insert("rm".into(), json!("")); m.insert("skip".into(), json!("")); m.insert("imp".into(), json!("")); m.insert("err".into(), json!(""));
    for f in page::FIELDS { m.insert(f.into(), Value::Null); } m
}

/// A press: the page state to merge, or why it was refused.
pub async fn act(m: &Arc<Mcpx>, action: &str, value: &str, ui: &Ui<'_>) -> Result<Map<String, Value>, String> {
    if let Some(p) = page::step(ui, action, value) { return Ok(p) }
    let info: Value = m.info().await;
    let r = match action {
        "mcp-add" => {
            let f = page::form_servers(ui, &m.list().ids()).map_err(|e| if e.is_empty() { "Paste a config snippet or a command".to_string() } else { e })?;
            let skip: Vec<usize> = ui_s(ui, "skip").split(',').filter_map(|x| x.parse().ok()).collect();
            let servers: Vec<Server> = f.servers.into_iter().enumerate().filter(|(i, _)| !skip.contains(i)).map(|(_, mut s)| { s.source = if ui_s(ui, "mode").is_empty() || ui_s(ui, "mode") == "paste" { "paste".into() } else { "manual".into() }; s }).collect();
            if servers.is_empty() { return Err("Nothing picked to add".into()) }
            let wanted: Vec<String> = servers.iter().flat_map(|s| s.secrets()).collect();
            let plain: Vec<(String, String)> = f.plain.into_iter().filter(|(n, _)| wanted.contains(n)).collect();
            let ids = m.add(servers, &plain, "person").await?;
            Ok(cleared(&if ids.len() == 1 { format!("open:{}", ids[0]) } else { String::new() }))
        }
        "mcp-gallery" => { let s = m.gallery_server(value, &ui_s(ui, "mgarg"))?; let ids = m.add(vec![s], &[], "person").await?; Ok(cleared(&format!("open:{}", ids[0]))) }
        "mcp-toggle" => { m.change(|l| { let s = l.servers.iter_mut().find(|s| s.id == value).ok_or("no such server")?; s.enabled = !s.enabled; Ok(()) }).await?; Ok(Map::new()) }
        "mcp-test" => { m.test_bg(value); Ok(Map::new()) }
        "mcp-remove" => {
            let gone = m.change(|l| { let i = l.servers.iter().position(|s| s.id == value).ok_or("no such server")?; Ok(l.servers.remove(i)) }).await?;
            let still: Vec<String> = m.list().servers.iter().flat_map(|s| s.secrets()).collect();
            for n in gone.secrets() { if n.starts_with(&format!("mcp-{}-", gone.id)) && !still.contains(&n) { let _ = crate::settings::vault::del(&n); } } // its own keys go with it
            Ok(cleared(""))
        }
        "mcp-dup" => {
            let id = m.change(|l| { let s = l.get(value).cloned().ok_or("no such server")?; let mut c = s.clone(); c.id = super::model::unique(&format!("{}-copy", s.id), &l.ids()); c.name = format!("{} copy", s.title()); c.sync = false; c.added_at = svc::now_s(); let id = c.id.clone(); l.servers.push(c); Ok(id) }).await?;
            Ok(cleared(&format!("open:{id}")))
        }
        "mcp-save" => {
            let old = m.list().get(value).cloned().ok_or("no such server")?;
            let (s, plain) = page::edited(&old, ui)?;
            m.store_secrets(&plain, &format!("MCP server {}", s.id), "person")?;
            let mut s = s; s.sync = false; // edited by hand: no longer follows its file
            m.change(|l| { if let Some(x) = l.servers.iter_mut().find(|x| x.id == old.id) { *x = s } Ok(()) }).await?;
            m.test_bg(value); Ok(cleared(&format!("open:{value}")))
        }
        "mcp-scope" => {
            let (id, what, x) = split3(value);
            m.change(|l| { let s = l.servers.iter_mut().find(|s| s.id == id).ok_or("no such server")?;
                let flip = |v: &mut Vec<String>, x: &str| { if x.is_empty() { v.clear() } else if let Some(i) = v.iter().position(|y| y == x) { v.remove(i); } else { v.push(x.into()) } };
                match what { "prov" => flip(&mut s.providers, x), "proj" => flip(&mut s.projects, x),
                    "bot" => match x { "*" => s.bots = None, "-" => s.bots = Some(vec![]), b => { let mut v = s.bots.clone().unwrap_or_default(); flip(&mut v, b); s.bots = Some(v) } },
                    _ => return Err("scope: prov, proj or bot".into()) }
                Ok(()) }).await?; Ok(Map::new())
        }
        "mcp-tool" => { let (id, tool, _) = split3(value); m.change(|l| { let s = l.servers.iter_mut().find(|s| s.id == id).ok_or("no such server")?; if let Some(i) = s.disabled_tools.iter().position(|t| t == tool) { s.disabled_tools.remove(i); } else { s.disabled_tools.push(tool.into()) } Ok(()) }).await?; Ok(Map::new()) }
        "mcp-pcbparts" => { set(&m.hub, svc::PCBPARTS_KEY, if value == "off" { "off" } else { "on" }).await?; m.publish().await; Ok(Map::new()) }
        "mcp-allow" => { let (p, v) = value.split_once('=').ok_or("project=allow|ask")?; if !info["projects"].as_array().into_iter().flatten().any(|x| x[0] == p) { return Err("no such project".into()) }
            set(&m.hub, &svc::allow_key(p), if v == "allow" { "allow" } else { "" }).await?; m.publish().await; Ok(Map::new()) }
        "mcp-scan" => { m.scan_imports().await; m.publish().await; Ok(Map::new()) }
        "mcp-import" => {
            if m.last_scan().is_null() { m.scan_imports().await; }
            let picked = page::import_picks(&m.last_scan(), &ui_s(ui, "imp"));
            let n = import_picked(m, &picked, ui_s(ui, "isync") != "off").await?;
            if n == 0 { return Err("Nothing picked to import".into()) }
            Ok(cleared(""))
        }
        "mcp-err" => { m.set_notice(""); Ok(Map::new()) }
        _ => Err(format!("unknown press {action}")),
    };
    r
}
fn ui_s(ui: &Ui, k: &str) -> String { (ui.get)(k).unwrap_or_default() }
async fn set(hub: &Arc<Hub>, k: &str, v: &str) -> Result<(), String> { hub.commit(&json!({"$":"SettingSet","key":k,"value":v,"at":svc::now_s()})).await.map(|_| ()).map_err(|e| format!("{e:#}")) }

/// Imports the scan's servers named `path|index` (a real key found in one goes to the vault). The count imported.
pub async fn import_picked(m: &Arc<Mcpx>, picked: &[String], sync: bool) -> Result<usize, String> {
    let sc = m.scan_imports().await;
    let (mut servers, mut plain) = (vec![], vec![]);
    for (s, here) in &sc { for (i, x) in s.found.servers.iter().enumerate() {
        if here[i].is_some() || !picked.iter().any(|p| *p == format!("{}|{i}", s.path)) { continue }
        let mut x = x.clone(); x.sync = sync; let names = x.secrets();
        plain.extend(s.found.plain.iter().filter(|(n, _)| names.contains(n)).cloned()); servers.push(x);
    } }
    let n = servers.len(); if n > 0 { m.add(servers, &plain, "import").await?; m.scan_imports().await; m.publish().await; }
    Ok(n)
}

/// The page state and typed fields a phone sends, as page.rs reads them.
fn ui_of<'a>(p: &'a Value) -> impl Fn(&str) -> Option<String> + Sync + 'a {
    move |k: &str| { let src = if page::UI_KEYS.contains(&k) { &p["ui"] } else { &p["fields"] }; src.get(k).and_then(|v| v.as_str().map(str::to_string)) }
}

/// `mcp.page {ui?, fields?}` → `{rows}`; `mcp.page.act {action, value, ui?, fields?}` → `{ui, rows}` (the page state to keep), or `{ok:false, error}`.
pub struct PageRpc;
impl RpcHandler for PageRpc {
    fn methods(&self) -> &'static [&'static str] { &["mcp.page", "mcp.page.act"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let Some(m) = svc::get() else { return vec![json!({"t":"reply","id":id,"ok":false,"error":"MCP is not running on this hub"})] };
            let get = ui_of(p);
            let mut ui_state: Map<String, Value> = p["ui"].as_object().cloned().unwrap_or_default();
            let mut cleared: Vec<String> = vec![]; // what a press emptied is drawn empty in the rows it answers with
            if method == "mcp.page.act" {
                let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                let from_desktop = p["desktop"] == true;
                match act(&m, &s("action"), &s("value"), &Ui { get: &get }).await {
                    Ok(patch) => { for (k, v) in patch { if v.is_null() { cleared.push(k.clone()); ui_state.remove(&k); } else { ui_state.insert(k, v); } } if !from_desktop { m.set_notice("") } }
                    Err(e) => { if from_desktop { m.set_notice(&e); m.publish().await; } return vec![json!({"t":"reply","id":id,"ok":false,"error":e})] }
                }
                if s("action") == "mcp-view" && s("value") == "import" { m.scan_imports().await; }
            } else if p["ui"]["view"] == "import" && m.last_scan().is_null() { m.scan_imports().await; }
            let info = m.info().await;
            let ui_v = Value::Object(ui_state.clone());
            let mut fields = p["fields"].clone(); if let Some(o) = fields.as_object_mut() { for k in &cleared { o.remove(k); } }
            let get2 = move |k: &str| { let src = if page::UI_KEYS.contains(&k) { &ui_v } else { &fields }; src.get(k).and_then(|v| v.as_str().map(str::to_string)) };
            let rows = page::rows(&info, &Ui { get: &get2 }, svc::now_s());
            vec![json!({"t":"reply","id":id,"ok":true,"rows":rows,"ui":ui_state})]
        })
    }
}

#[cfg(test)]
mod tests {
    #[test] fn import_keys_name_a_path_and_an_index() { assert_eq!(super::split3("a|b|c|d"), ("a", "b", "c|d")); let _ = super::import::sources; }
}
