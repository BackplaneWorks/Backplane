//! KiCad's long jobs in the embedded editor (Backplane KiCad bp.12: loading a schematic or board, libraries, zone fill, DRC, plotting).
//! In `--embedded` KiCad opens no progress window: it sends ET_PROGRESS {id, title, detail, fraction (-1 indeterminate), done, cancellable,
//! cancelled}. This keeps the jobs running per project and publishes them as info `kicad.progress.<digest>` {project, jobs: [...]} (at most
//! ~10 a second per job, KiCad's own throttle), which the editor's row draws as a slim bar; its × answers through `editor.kicad.cancel`
//! -> CancelProgress.
use super::{embed, kserve as ks};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, HashMap}, sync::{Arc, Mutex, OnceLock, Weak}};

/// One running job as the pane draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Job { pub title: String, pub detail: String, pub fraction: f64, pub cancellable: bool }

/// The jobs after one ET_PROGRESS: a done job leaves, any other is set to what the event says.
pub fn apply(mut jobs: BTreeMap<u64, Job>, p: &Value) -> BTreeMap<u64, Job> {
    let id = p["id"].as_u64().or_else(|| p["id"].as_str().and_then(|s| s.parse().ok())).unwrap_or(0);
    if p["done"].as_bool().unwrap_or(false) { jobs.remove(&id); return jobs }
    jobs.insert(id, Job { title: p["title"].as_str().unwrap_or("").into(), detail: p["detail"].as_str().unwrap_or("").into(),
        fraction: p["fraction"].as_f64().unwrap_or(-1.0), cancellable: p["cancellable"].as_bool().unwrap_or(false) });
    jobs
}
pub fn to_json(pro: &str, jobs: &BTreeMap<u64, Job>) -> Value {
    json!({"project": pro, "jobs": jobs.iter().map(|(id, j)| json!({"id": id, "title": j.title, "detail": j.detail, "fraction": j.fraction, "cancellable": j.cancellable})).collect::<Vec<_>>()})
}
pub fn is_progress(ev: &Value) -> bool { ev["type"] == "ET_PROGRESS" || ev["type"].as_i64() == Some(24) }

static STATE: OnceLock<Mutex<HashMap<String, BTreeMap<u64, Job>>>> = OnceLock::new();
fn state() -> &'static Mutex<HashMap<String, BTreeMap<u64, Job>>> { STATE.get_or_init(Default::default) }
static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);
pub fn start(hub: &Arc<Hub>) { *HUB.lock().unwrap() = Some(Arc::downgrade(hub)); hub.register_rpc(Arc::new(ProgressRpc)); }

/// From the fork's events loop (embed.rs): true when it was a progress event (handled here).
pub fn on_event(pro: &str, ev: &Value) -> bool {
    if !is_progress(ev) { return false }
    let v = { let mut m = state().lock().unwrap(); let n = apply(m.remove(pro).unwrap_or_default(), &ev["progress"]); let v = to_json(pro, &n); m.insert(pro.to_string(), n); v };
    if let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) {
        let pro = pro.to_string();
        tokio::spawn(async move { hub.set_info(&format!("kicad.progress.{}", ks::digest(&pro)), &v.to_string()).await; });
    }
    true
}
/// KiCad went away or was restarted: nothing of its runs any more.
pub fn clear(pro: &str) { state().lock().unwrap().remove(pro); }

/// `editor.kicad.cancel {project, id}`: the × on the editor row's bar.
pub struct ProgressRpc;
impl RpcHandler for ProgressRpc {
    fn methods(&self) -> &'static [&'static str] { &["editor.kicad.cancel"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let err = |e: &str| vec![json!({"t":"reply","id":id,"ok":false,"error":e})];
            let pro = match embed::resolve_pro(p["project"].as_str().unwrap_or("")) { Ok(p) => p, Err(e) => return err(&e) };
            if !super::known_root(hub, std::path::Path::new(&pro).parent().and_then(|p| p.to_str()).unwrap_or(""), "").await { return err("not in a project this hub knows") }
            let Some(gui) = embed::gui_route(&pro) else { return err("KiCad is not running") };
            let o = embed::gui_call(&gui, "kiapi.backplane.embed.CancelProgress", json!({"id": p["id"].as_u64().unwrap_or(0)})).await;
            if o["ok"].as_bool().unwrap_or(false) { vec![json!({"t":"reply","id":id,"ok":true,"result":o["message"].clone()})] } else { err(&format!("CancelProgress: {}", o["error"].as_str().unwrap_or(""))) }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_job_shows_until_its_done_event() {
        let j = apply(BTreeMap::new(), &json!({"id": 3, "title": "Filling zones", "fraction": 0.25, "cancellable": true}));
        assert_eq!(j[&3].title, "Filling zones"); assert!(j[&3].cancellable);
        let j = apply(j, &json!({"id": "3", "title": "Filling zones", "detail": "GND", "fraction": 0.75}));
        assert_eq!(j[&3].fraction, 0.75); assert_eq!(j[&3].detail, "GND");
        let j = apply(j, &json!({"id": 3, "done": true, "cancelled": true})); assert!(j.is_empty());
        assert!(is_progress(&json!({"type": 24})) && !is_progress(&json!({"type": "ET_COMMENT"})));
    }
}
