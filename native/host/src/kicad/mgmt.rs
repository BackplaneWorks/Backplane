//! Management panels beside the design (docs/MANAGEMENT.md): a slim tab strip under the viewer pane (the embedded KiCad or Backplane's viewers)
//! opens one panel (BOM, Checks, Lifecycle...) at the side or the bottom. The hub keeps which panel is open, where and how big, per project
//! (`<home>/mgmt/panels.json`), and every panel's data, published as info so the desktop and the phones draw the same thing:
//!   - `mgmt.state.<digest>`  {project, open, side, size, panels: [{id, label}]}
//!   - `mgmt.<panel>.<digest>` {project, badge: {n, tone}, ...the panel's own fields}
//! RPCs: `mgmt.watch {project}` (a client shows this project's design: the state, and every panel starts publishing for it) ->
//! {digest, project, state}; `mgmt.panel {project, open?, side?, size?}` -> {state}.
//!
//! A panel is a `Panel` registered with `register` (the hub side) and a view in the client's `screens/mgmt.rs` `VIEWS` (same id).
use super::{embed, kserve as ks};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf, sync::{Arc, Mutex, OnceLock}};

/// The hub side of a management panel. `watch` is called whenever a client shows the project's design (and again on later shows): start or
/// refresh what the panel publishes for it with `publish` (it should be cheap when nothing changed; the tab's badge needs the data while the
/// panel is shut, too). `opened` is called when the person opens the panel on the project (refresh now).
pub trait Panel: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn watch(&self, hub: &Arc<Hub>, project: &str);
    fn opened(&self, hub: &Arc<Hub>, project: &str) { self.watch(hub, project) }
}

/// The tabs' order: known panels in this order, others after them by id.
const ORDER: &[&str] = &["bom", "checks", "life", "plan", "review"];
pub const SIDES: &[&str] = &["right", "bottom"];
pub const MIN_SIZE: u32 = 160;
pub const MAX_SIZE: u32 = 1600;
pub fn default_size(side: &str) -> u32 { if side == "bottom" { 260 } else { 400 } }

/// One project's panel state: the open panel ("" none), its side and size (px).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct St { #[serde(default)] pub open: String, #[serde(default = "right")] pub side: String, #[serde(default)] pub size: u32, #[serde(default)] pub project: String, #[serde(default)] pub at: u64 }
fn right() -> String { "right".into() }
impl Default for St { fn default() -> Self { St { open: String::new(), side: right(), size: 0, project: String::new(), at: 0 } } }
impl St {
    /// The size drawn: the saved one, else the side's default.
    pub fn size_or_default(&self) -> u32 { if self.size == 0 { default_size(&self.side) } else { self.size } }
}

/// A request's changes on a state: `open` names a known panel or "" (shut); `toggle` opens it, or shuts it when it is the one open;
/// `side` right|bottom (a new side takes that side's own size unless `size` is given); `size` is clamped. Unknown values change nothing.
pub fn apply(mut st: St, p: &Value, known: &[&str]) -> St {
    if let Some(t) = p["toggle"].as_str().filter(|t| known.contains(t)) { st.open = if st.open == t { String::new() } else { t.to_string() } }
    if let Some(o) = p["open"].as_str().filter(|o| o.is_empty() || known.contains(o)) { st.open = o.to_string() }
    if let Some(s) = p["side"].as_str().filter(|s| SIDES.contains(s)) { if st.side != s { st.side = s.to_string(); st.size = 0 } }
    if let Some(z) = p["size"].as_u64() { st.size = (z.min(u32::MAX as u64) as u32).clamp(MIN_SIZE, MAX_SIZE) }
    st
}
/// The registered panels in tab order.
pub fn ordered(ids: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut v = ids.to_vec();
    v.sort_by_key(|(id, _)| (ORDER.iter().position(|o| o == id).unwrap_or(ORDER.len()), *id));
    v.dedup_by_key(|(id, _)| *id);
    v
}
pub fn state_key(pro: &str) -> String { format!("mgmt.state.{}", ks::digest(pro)) }
pub fn data_key(panel: &str, pro: &str) -> String { format!("mgmt.{panel}.{}", ks::digest(pro)) }
pub fn state_json(st: &St, pro: &str, panels: &[(&'static str, &'static str)]) -> Value {
    json!({"project": pro, "open": st.open, "side": st.side, "size": st.size_or_default(),
        "panels": panels.iter().map(|(id, l)| json!({"id": id, "label": l})).collect::<Vec<_>>()})
}

// The registry and the saved states -------------------------------------------------------------------------------------

static PANELS: OnceLock<Mutex<Vec<Arc<dyn Panel>>>> = OnceLock::new();
fn panels() -> &'static Mutex<Vec<Arc<dyn Panel>>> { PANELS.get_or_init(Default::default) }
/// Adds a panel (once per id; a second registration of an id replaces the first).
pub fn register(p: Arc<dyn Panel>) { let mut v = panels().lock().unwrap(); v.retain(|x| x.id() != p.id()); v.push(p) }
fn list() -> Vec<(&'static str, &'static str)> { ordered(&panels().lock().unwrap().iter().map(|p| (p.id(), p.label())).collect::<Vec<_>>()) }
fn all() -> Vec<Arc<dyn Panel>> { panels().lock().unwrap().clone() }

const KEPT: usize = 200;
struct Saved { path: PathBuf, m: BTreeMap<String, St> }
static SAVED: OnceLock<Mutex<Saved>> = OnceLock::new();
fn saved() -> &'static Mutex<Saved> { SAVED.get_or_init(|| Mutex::new(Saved { path: PathBuf::new(), m: BTreeMap::new() })) }
fn load(home: &std::path::Path) {
    let path = home.join("mgmt").join("panels.json");
    let m: BTreeMap<String, St> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    *saved().lock().unwrap() = Saved { path, m };
}
pub fn state_of(pro: &str) -> St { saved().lock().unwrap().m.get(&ks::digest(pro)).cloned().unwrap_or_default() }
/// Whether a panel is open on the project now.
pub fn is_open(pro: &str, panel: &str) -> bool { state_of(pro).open == panel }
fn store(pro: &str, mut st: St) {
    st.project = pro.to_string(); st.at = super::ipc::now_s();
    let (path, text) = {
        let mut s = saved().lock().unwrap();
        let me = ks::digest(pro);
        s.m.insert(me.clone(), st);
        // the oldest go first past the bound (never the one just saved)
        while s.m.len() > KEPT { if let Some(k) = s.m.iter().filter(|(k, _)| **k != me).min_by_key(|(_, v)| v.at).map(|(k, _)| k.clone()) { s.m.remove(&k); } else { break } }
        (s.path.clone(), serde_json::to_string(&s.m).unwrap_or_default())
    };
    if path.as_os_str().is_empty() { return }
    if let Some(d) = path.parent() { let _ = std::fs::create_dir_all(d); }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() { let _ = std::fs::rename(&tmp, &path); }
}

/// What a panel calls to publish its data for a project (a JSON object; `project` is set here). Bounded by the hub's info (64 KiB):
/// a bigger value is refused (false) and the panel should send less.
pub async fn publish(hub: &Arc<Hub>, panel: &str, pro: &str, mut v: Value) -> bool {
    if let Some(o) = v.as_object_mut() { o.insert("project".into(), json!(pro)); }
    hub.set_info(&data_key(panel, pro), &v.to_string()).await
}
async fn publish_state(hub: &Arc<Hub>, pro: &str) -> Value {
    let v = state_json(&state_of(pro), pro, &list());
    hub.set_info(&state_key(pro), &v.to_string()).await;
    v
}

pub fn start(hub: &Arc<Hub>) { load(&hub.cfg.home); hub.register_rpc(Arc::new(MgmtRpc)); }

pub struct MgmtRpc;
impl RpcHandler for MgmtRpc {
    fn methods(&self) -> &'static [&'static str] { &["mgmt.watch", "mgmt.panel"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let err = |e: &str| vec![json!({"t":"reply","id":id,"ok":false,"error":e})];
            let pro = match embed::resolve_pro(p["project"].as_str().unwrap_or("")) { Ok(p) => p, Err(e) => return err(&e) };
            let dir = std::path::Path::new(&pro).parent().and_then(|p| p.to_str()).unwrap_or("").to_string();
            if !super::known_root(hub, &dir, "").await { return err("not in a project this hub knows") }
            let ps = all();
            if m == "mgmt.panel" {
                let was = state_of(&pro);
                let known: Vec<&str> = ps.iter().map(|x| x.id()).collect();
                let now = apply(was.clone(), p, &known);
                store(&pro, now.clone());
                let st = publish_state(hub, &pro).await;
                if !now.open.is_empty() && now.open != was.open { if let Some(x) = ps.iter().find(|x| x.id() == now.open) { x.opened(hub, &pro) } }
                return vec![json!({"t":"reply","id":id,"ok":true,"result":{"digest": ks::digest(&pro), "state": st}})]
            }
            let st = publish_state(hub, &pro).await;
            for x in &ps { x.watch(hub, &pro) }
            vec![json!({"t":"reply","id":id,"ok":true,"result":{"digest": ks::digest(&pro), "project": pro, "state": st}})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const K: &[&str] = &["bom", "checks", "life"];
    #[test] fn a_tab_opens_its_panel_and_a_second_press_shuts_it() {
        let s = apply(St::default(), &json!({"toggle": "bom"}), K); assert_eq!(s.open, "bom");
        let s = apply(s, &json!({"toggle": "checks"}), K); assert_eq!(s.open, "checks");
        let s = apply(s, &json!({"toggle": "checks"}), K); assert_eq!(s.open, "");
        let s = apply(s, &json!({"open": "life"}), K); assert_eq!(s.open, "life");
        let s = apply(s, &json!({"open": "nope"}), K); assert_eq!(s.open, "life");
        let s = apply(s, &json!({"toggle": "nope"}), K); assert_eq!(s.open, "life");
        assert_eq!(apply(s, &json!({"open": ""}), K).open, "");
    }
    #[test] fn sides_and_sizes() {
        let s = apply(St::default(), &json!({"size": 520}), K); assert_eq!((s.side.as_str(), s.size), ("right", 520));
        let s = apply(s, &json!({"side": "bottom"}), K); assert_eq!((s.side.as_str(), s.size_or_default()), ("bottom", 260), "a new side takes its own default");
        let s = apply(s, &json!({"side": "left"}), K); assert_eq!(s.side, "bottom");
        assert_eq!(apply(s.clone(), &json!({"size": 5}), K).size, MIN_SIZE); assert_eq!(apply(s.clone(), &json!({"size": 99999}), K).size, MAX_SIZE);
        let s = apply(s, &json!({"side": "right", "size": 333}), K); assert_eq!((s.side.as_str(), s.size), ("right", 333));
        assert_eq!(St::default().size_or_default(), 400);
    }
    #[test] fn tabs_in_order_whatever_the_registration_order() {
        assert_eq!(ordered(&[("zz", "Z"), ("life", "Lifecycle"), ("bom", "BOM"), ("checks", "Checks"), ("bom", "BOM")]).iter().map(|x| x.0).collect::<Vec<_>>(), ["bom", "checks", "life", "zz"]);
    }
    #[test] fn the_state_as_published() {
        let v = state_json(&St { open: "bom".into(), ..St::default() }, "/p/x.kicad_pro", &[("bom", "BOM")]);
        assert_eq!(v, json!({"project": "/p/x.kicad_pro", "open": "bom", "side": "right", "size": 400, "panels": [{"id": "bom", "label": "BOM"}]}));
        assert!(state_key("/p/x.kicad_pro").starts_with("mgmt.state.") && data_key("bom", "/p/x.kicad_pro").starts_with("mgmt.bom."));
        // an old file without the newer fields still reads
        let s: St = serde_json::from_str(r#"{"open":"bom"}"#).unwrap(); assert_eq!((s.side.as_str(), s.size), ("right", 0));
    }
    #[test] fn saved_states_survive_a_restart_and_stay_bounded() {
        let d = crate::plat::tmp::dir("mgmt-save"); load(&d);
        for i in 0..(KEPT + 5) { store(&format!("/p{i}/x.kicad_pro"), St { open: "bom".into(), ..St::default() }) }
        load(&d); assert_eq!(saved().lock().unwrap().m.len(), KEPT);
        store("/q/x.kicad_pro", St { open: "life".into(), side: "bottom".into(), size: 300, ..St::default() });
        load(&d); let s = state_of("/q/x.kicad_pro"); assert_eq!((s.open.as_str(), s.side.as_str(), s.size), ("life", "bottom", 300));
        assert!(is_open("/q/x.kicad_pro", "life") && !is_open("/q/x.kicad_pro", "bom"));
    }
}
