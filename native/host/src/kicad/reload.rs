//! The embedded KiCad and files changed on disk (Backplane KiCad bp.9: the fork watches the open board, schematic and its sheets). A file
//! written by an agent, a script or git is reloaded in place when nothing is unsaved (ET_DOCUMENT_RELOADED); over the person's unsaved
//! edits nothing is reloaded (ET_RELOAD_CONFLICT) and KiCad asks them in its own infobar. This module keeps what is waiting on the person
//! per project and publishes it as info `kicad.reload.<digest>` {project, conflicts: [file], last: {file, event, at}}, so the pane's row
//! says so too (and its Keep yours / Reload buttons answer through `editor.kicad.resolve` -> ResolveConflict).
use super::{embed, kserve as ks};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::{BTreeSet, HashMap}, sync::{Arc, Mutex, OnceLock, Weak}};

fn norm_file(p: &str) -> String {
    let mut s = p.replace('\\', "/");
    if s.as_bytes().get(1) == Some(&b':') && s.as_bytes()[0].is_ascii_alphabetic() {
        let lo = (s.as_bytes()[0].to_ascii_lowercase() as char).to_string();
        s.replace_range(0..1, &lo);
    }
    s
}

/// Per project: the files KiCad holds back because the person has unsaved edits in them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pending { pub conflicts: BTreeSet<String>, pub last: Option<(String, &'static str)> }
impl Pending {
    /// An event from the fork: a conflict is waiting; a reload (by itself, or the person's Reload) clears that file.
    pub fn on(mut self, event: &str, file: &str) -> Self {
        let nf = norm_file(file);
        match event {
            "conflict" => {
                if !self.conflicts.iter().any(|x| norm_file(x) == nf) { self.conflicts.insert(file.to_string()); }
                self.last = Some((file.to_string(), "conflict"));
            }
            "reloaded" => { self.conflicts.retain(|x| norm_file(x) != nf); self.last = Some((file.to_string(), "reloaded")); }
            "kept" => { self.conflicts.retain(|x| norm_file(x) != nf); self.last = Some((file.to_string(), "kept")); }
            _ => {}
        }
        self
    }
    pub fn to_json(&self, pro: &str, at: u64) -> Value {
        json!({"project": pro, "conflicts": self.conflicts.iter().collect::<Vec<_>>(),
               "last": self.last.as_ref().map(|(f, e)| json!({"file": f, "event": e, "at": at}))})
    }
}
/// The event kind of a fork event (ET_DOCUMENT_RELOADED=15, ET_RELOAD_CONFLICT=16), None for any other.
pub fn kind(ev: &Value) -> Option<&'static str> {
    match (ev["type"].as_str().unwrap_or(""), ev["type"].as_i64().unwrap_or(0)) { ("ET_DOCUMENT_RELOADED", _) | (_, 15) => Some("reloaded"), ("ET_RELOAD_CONFLICT", _) | (_, 16) => Some("conflict"), _ => None }
}

static STATE: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
fn state() -> &'static Mutex<HashMap<String, Pending>> { STATE.get_or_init(Default::default) }
static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);

pub fn start(hub: &Arc<Hub>) { *HUB.lock().unwrap() = Some(Arc::downgrade(hub)); hub.register_rpc(Arc::new(ReloadRpc)); }

fn publish(pro: &str, event: &str, file: &str) {
    let p = { let mut m = state().lock().unwrap(); let n = m.remove(pro).unwrap_or_default().on(event, file); m.insert(pro.to_string(), n.clone()); n };
    let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) else { return };
    let (pro, v) = (pro.to_string(), p.to_json(pro, crate::hub::now_ms()));
    tokio::spawn(async move { hub.set_info(&format!("kicad.reload.{}", ks::digest(&pro)), &v.to_string()).await; });
}
/// From the fork's events loop (embed.rs): true when the event was a reload one (handled here).
pub fn on_event(pro: &str, ev: &Value) -> bool {
    let Some(k) = kind(ev) else { return false };
    publish(pro, k, ev["detail"].as_str().unwrap_or(""));
    true
}

/// `editor.kicad.resolve {project, file, choice: keep|reload}`: the person's answer from Backplane's row (KiCad's own infobar does the same).
pub struct ReloadRpc;
impl RpcHandler for ReloadRpc {
    fn methods(&self) -> &'static [&'static str] { &["editor.kicad.resolve"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let err = |e: &str| vec![json!({"t":"reply","id":id,"ok":false,"error":e})];
            let (raw, file, choice) = (p["project"].as_str().unwrap_or(""), p["file"].as_str().unwrap_or(""), p["choice"].as_str().unwrap_or(""));
            let cc = match choice { "keep" => "CC_KEEP", "reload" => "CC_RELOAD", _ => return err("choice is keep or reload") };
            let pro = match embed::resolve_pro(raw) { Ok(p) => p, Err(e) => return err(&e) };
            if !super::known_root(hub, std::path::Path::new(&pro).parent().and_then(|p| p.to_str()).unwrap_or(""), "").await { return err("not in a project this hub knows") }
            let Some(gui) = embed::gui_route(&pro) else { return err("KiCad is not running") };
            let o = embed::gui_call(&gui, "kiapi.backplane.embed.ResolveConflict", json!({"path": file, "choice": cc})).await;
            if !o["ok"].as_bool().unwrap_or(false) { return err(&format!("ResolveConflict: {}", o["error"].as_str().unwrap_or(""))) }
            publish(&pro, if choice == "keep" { "kept" } else { "reloaded" }, file);
            vec![json!({"t":"reply","id":id,"ok":true,"result":o["message"].clone()})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn conflicts_wait_until_the_person_answers_or_kicad_reloads() {
        let p = Pending::default().on("conflict", "/p/a.kicad_pcb").on("conflict", "/p/a.kicad_sch");
        assert_eq!(p.conflicts.len(), 2);
        let p = p.on("kept", "/p/a.kicad_pcb"); assert_eq!(p.conflicts.iter().collect::<Vec<_>>(), ["/p/a.kicad_sch"]);
        let p = p.on("reloaded", "/p/a.kicad_sch"); assert!(p.conflicts.is_empty()); assert_eq!(p.last, Some(("/p/a.kicad_sch".into(), "reloaded")));
        assert_eq!(p.to_json("/p/a.kicad_pro", 5)["last"]["event"], "reloaded");
        let w = Pending::default().on("conflict", r"C:\bp\proj\a.kicad_pcb").on("kept", "c:/bp/proj/a.kicad_pcb");
        assert!(w.conflicts.is_empty());
    }
    #[test] fn only_the_forks_reload_events_are_ours() {
        assert_eq!(kind(&json!({"type": "ET_RELOAD_CONFLICT"})), Some("conflict")); assert_eq!(kind(&json!({"type": 15})), Some("reloaded"));
        assert_eq!(kind(&json!({"type": "ET_SELECTION_CHANGED"})), None);
    }
}
