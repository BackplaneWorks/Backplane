//! Cross-probing between the chat, Backplane's own viewers and the embedded KiCad (docs/embedded-editors.md, "Cross-probing").
//!
//! - `editor.kicad.select {project | thread, references[], nets[], pins[], zoom, show?: "PCB"|"SCH", from?: "viewer"}`: the names are selected in the
//!   project's open KiCad editors and, unless they came from a viewer tap (which keeps its own probe), lit in Backplane's viewers too (the hub's
//!   board probe, `probe:<root>|<board>`, which every page lights). `show` switches the pane to that editor first. The answer says what KiCad
//!   found: `{kicad, found, not_found, probe}`; `kicad: false` when no KiCad is open for the project (the viewers still light it).
//!   bp.9 runtimes: `kiapi.backplane.embed.SelectItems {kind, references, nets, pins, zoom}` to each open PCB/SCH frame.
//!   bp.8 runtimes: `ShowFrame {kind, item: <first reference>}` (KIWAY cross-probe: select and centre) on the shown editor, or on `show`.
//! - `kicad.names {project | thread}`: the project's reference designators and net names (the board's footprints and nets, the schematic's symbols and
//!   labels, every sheet), for the timeline to make them clickable.
//! - KiCad's `ET_SELECTION_CHANGED` (bp.9, the person's own selection): published as info `kicad.sel.<digest>` and set as the board probe, so the
//!   viewers light it. KiCad's PCB<->SCH cross-probe is KiCad's own (KIWAY), so the other frame is left alone.
use super::{embed, ipc, kserve as ks, project, sexpr};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::{BTreeSet, HashMap}, path::Path, sync::{Arc, Mutex, OnceLock, Weak}};

// Pure rules -----------------------------------------------------------------------------------------------------------------

/// Names to select: reference designators ("U3"), net names ("GND", "/SDA") and pins ("U3.4").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Names { pub references: Vec<String>, pub nets: Vec<String>, pub pins: Vec<String> }

const MAX_NAMES: usize = 256;

/// A reference designator: letters (or `#`) then a number, maybe a unit letter ("U3", "R12", "SW1", "U3A"). Power symbols (`#PWR01`) are not parts.
pub fn is_reference(s: &str) -> bool {
    let b = s.as_bytes();
    let letters = b.iter().take_while(|c| c.is_ascii_alphabetic()).count();
    let digits = b[letters..].iter().take_while(|c| c.is_ascii_digit()).count();
    let tail = &b[letters + digits..];
    (1..=6).contains(&letters) && digits >= 1 && digits <= 6 && tail.len() <= 1 && tail.iter().all(|c| c.is_ascii_uppercase())
}
/// A pin: `<reference>.<pad number>` ("U3.4", "J1.A12").
pub fn is_pin(s: &str) -> bool {
    match s.split_once('.') { Some((r, n)) => is_reference(r) && !n.is_empty() && n.len() <= 16 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'), None => false }
}
fn clean_net(s: &str) -> bool { !s.is_empty() && s.len() <= 200 && !s.contains(['\n', '\r', '\0', ';', '\u{1}']) }

fn strings(v: &Value) -> Vec<String> {
    match v { Value::Array(a) => a.iter().filter_map(|x| x.as_str()).map(|x| x.trim().to_string()).collect(), Value::String(s) => vec![s.trim().to_string()], _ => vec![] }
}
fn dedup(v: Vec<String>, ok: impl Fn(&str) -> bool) -> Vec<String> {
    let mut seen = BTreeSet::new();
    v.into_iter().filter(|x| ok(x) && seen.insert(x.clone())).take(MAX_NAMES).collect()
}
/// The names of a request (or an event's `selection`): trimmed, checked, without repeats.
pub fn names_of(p: &Value) -> Names {
    Names {
        references: dedup(strings(&p["references"]), is_reference),
        nets: dedup(strings(&p["nets"]), clean_net),
        pins: dedup(strings(&p["pins"]), is_pin),
    }
}
impl Names {
    pub fn is_empty(&self) -> bool { self.references.is_empty() && self.nets.is_empty() && self.pins.is_empty() }
    pub fn to_json(&self) -> Value { json!({"references": self.references, "nets": self.nets, "pins": self.pins}) }
    /// The part bp.8 can cross-probe (one reference): the first reference, else the first pin's part.
    pub fn first_part(&self) -> Option<String> {
        self.references.first().cloned().or_else(|| self.pins.first().and_then(|p| p.split_once('.')).map(|(r, _)| r.to_string()))
    }
}

/// `show`: "PCB" or "SCH" (also "board"/"schematic"), "" for none.
pub fn show_frame(show: &str) -> &'static str {
    match show.trim().to_ascii_lowercase().as_str() { "pcb" | "board" | "fk_pcb" => "PCB", "sch" | "schematic" | "fk_sch" => "SCH", _ => "" }
}

/// The SelectItems body for one editor (bp.9).
pub fn select_msg(kind: &str, n: &Names, zoom: bool) -> Value {
    json!({"kind": if kind == "SCH" { "FK_SCH" } else { "FK_PCB" }, "references": n.references, "nets": n.nets, "pins": n.pins, "zoom": zoom})
}

/// The editors a selection goes to: the open PCB and SCH frames, the one to show first. bp.8 (one ShowFrame with an item, which also maps
/// it) only the shown one, or the one to show.
pub fn targets(open: &[(String, bool)], show: &str, bp9: bool) -> Vec<&'static str> {
    let has = |k: &str| open.iter().any(|(f, _)| f == k);
    let shown = open.iter().find(|(f, s)| *s && (f == "PCB" || f == "SCH")).map(|(f, _)| if f == "SCH" { "SCH" } else { "PCB" });
    if !bp9 {
        return if !show.is_empty() { vec![if show == "SCH" { "SCH" } else { "PCB" }] } else { shown.into_iter().collect() }
    }
    let mut out: Vec<&'static str> = vec![];
    if show == "SCH" || show == "PCB" { out.push(if show == "SCH" { "SCH" } else { "PCB" }); }
    for k in ["PCB", "SCH"] { if has(k) && !out.contains(&k) { out.push(k) } }
    out
}

/// What SelectItems answered, merged over the editors: a name is found when any editor found it, not found when none did.
pub fn merge_found(replies: &[Value]) -> (Names, Names) {
    let mut found = Names::default(); let mut missing = Names::default();
    for r in replies {
        let f = names_of(&r["found"]); let m = names_of(&r["not_found"]);
        found.references.extend(f.references); found.nets.extend(f.nets); found.pins.extend(f.pins);
        missing.references.extend(m.references); missing.nets.extend(m.nets); missing.pins.extend(m.pins);
    }
    let d = |v: Vec<String>| dedup(v, |_| true);
    let found = Names { references: d(found.references), nets: d(found.nets), pins: d(found.pins) };
    let not = |v: Vec<String>, f: &[String]| d(v.into_iter().filter(|x| !f.contains(x)).collect());
    let missing = Names { references: not(missing.references, &found.references), nets: not(missing.nets, &found.nets), pins: not(missing.pins, &found.pins) };
    (found, missing)
}

/// The board probe for names, as kicad.info's text ("title;k=v;..."; screens/plot.rs `card`): a part lights by its `ref` on the board and the
/// schematic; a net by `net` on the board, and on the schematic through the board's drawing (`g=<board key>\u{1}`). "" for nothing.
pub fn probe_text(n: &Names, board_key: &str) -> String {
    let clean = |s: &str| s.replace([';', '\n', '\u{1}'], ",");
    if let Some(r) = n.first_part() {
        let mut t = format!("footprint {};ref={}", clean(&r), clean(&r));
        if let Some(p) = n.pins.iter().find(|p| p.starts_with(&format!("{r}."))) { t.push_str(&format!(";pin={}", clean(p.split_once('.').map(|x| x.1).unwrap_or("")))); }
        return t
    }
    if let Some(net) = n.nets.first() {
        let mut t = format!("net {};net={}", clean(net), clean(net));
        if !board_key.is_empty() { t.push_str(&format!(";g={board_key}\u{1}")); }
        return t
    }
    String::new()
}

fn rel_under(root: &str, path: &str) -> Option<String> {
    let is_sep = |b: u8| b == b'/' || b == b'\\';
    let r = root.trim_end_matches(['/', '\\']);
    if r.is_empty() || path.len() <= r.len() { return None }
    let same = r.bytes().enumerate().all(|(i, a)| {
        let b = path.as_bytes()[i];
        a == b || (is_sep(a) && is_sep(b)) || (i == 0 && r.as_bytes().get(1) == Some(&b':') && a.eq_ignore_ascii_case(&b))
    });
    if !same || !is_sep(path.as_bytes()[r.len()]) { return None }
    Some(path[r.len() + 1..].trim_start_matches(['/', '\\']).replace('\\', "/"))
}

/// The settings that keep a board's probe (present/viewer.rs `probe_key`): for every project root holding the KiCad project, its own board
/// ("<root>|") and the board by its path from the root.
pub fn probe_keys(roots: &[String], pcb: &str) -> Vec<String> {
    let mut out = vec![];
    for r in roots {
        let rt = r.trim_end_matches(['/', '\\']); if rt.is_empty() { continue }
        let Some(rel) = rel_under(rt, pcb) else { continue };
        out.push(format!("probe:{rt}|"));
        if !rel.is_empty() { out.push(format!("probe:{rt}|{rel}")); }
    }
    out
}

/// An ET_SELECTION_CHANGED event: the editor (PCB or SCH) and the person's selection (empty: cleared).
pub fn selection_event(ev: &Value) -> Option<(&'static str, Names)> {
    let ty = &ev["type"];
    if !(ty == "ET_SELECTION_CHANGED" || ty.as_i64() == Some(14)) { return None }
    let kind = embed::kind_of_fk_val(&ev["frame"]["kind"]);
    let kind = if kind == "SCH" { "SCH" } else { "PCB" };
    Some((kind, names_of(&ev["selection"])))
}

// The project's names (`kicad.names`) -----------------------------------------------------------------------------------------

/// A board's reference designators and net names.
pub fn board_names(b: &[u8], refs: &mut BTreeSet<String>, nets: &mut BTreeSet<String>) {
    for f in sexpr::forms(b).unwrap_or_default() {
        let x = sexpr::parse(f);
        match x.name() {
            "footprint" | "module" => {
                let r = super::board::prop(&x, "Reference");
                let r = if r.is_empty() { x.all("fp_text").find(|t| t.arg(0) == "reference").map(|t| t.arg(1)).unwrap_or("") } else { r };
                if is_reference(r) { refs.insert(r.to_string()); }
            }
            // (net 3 "GND") in KiCad 9 and before, (net "GND") after
            "net" => { let n = if x.args().len() >= 2 { x.arg(1) } else { x.arg(0) }; if clean_net(n) { nets.insert(n.to_string()); } }
            _ => {}
        }
    }
}
/// A schematic sheet's references and labels, and the sheet files it names (relative to its folder).
pub fn sheet_names(b: &[u8], refs: &mut BTreeSet<String>, nets: &mut BTreeSet<String>) -> Vec<String> {
    let mut subs = vec![];
    for f in sexpr::forms(b).unwrap_or_default() {
        let x = sexpr::parse(f);
        match x.name() {
            "symbol" => {
                // every instance's reference (a sheet used twice has two), else the symbol's own
                let mut any = false;
                for inst in x.all("instances") { for p in inst.all("project") { for path in p.all("path") { let r = path.get("reference").arg(0); if is_reference(r) { refs.insert(r.to_string()); any = true; } } } }
                if !any { let r = super::board::prop(&x, "Reference"); if is_reference(r) { refs.insert(r.to_string()); } }
            }
            "label" | "global_label" | "hierarchical_label" => { let n = x.arg(0); if clean_net(n) { nets.insert(n.to_string()); } }
            "sheet" => { let f = super::schematic::sheet_prop(&x, "Sheetfile", "Sheet file"); if !f.is_empty() { subs.push(f.to_string()); } }
            _ => {}
        }
    }
    subs
}
/// The project's names: its board's and every sheet's (at most 64 sheets).
pub fn project_names(pcb: &str, sch: &str) -> (Vec<String>, Vec<String>) {
    let (mut refs, mut nets) = (BTreeSet::new(), BTreeSet::new());
    if !pcb.is_empty() { if let Ok(b) = std::fs::read(pcb) { board_names(&b, &mut refs, &mut nets); } }
    let mut todo = if sch.is_empty() { vec![] } else { vec![std::path::PathBuf::from(sch)] };
    let mut seen = BTreeSet::new();
    while let Some(p) = todo.pop() {
        if seen.len() >= 64 || !seen.insert(p.clone()) { continue }
        let Ok(b) = std::fs::read(&p) else { continue };
        let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
        for s in sheet_names(&b, &mut refs, &mut nets) { if !s.contains("..") && !s.starts_with(['/', '\\']) && !crate::plat::fs::is_abs(&s) { todo.push(dir.join(s)); } }
    }
    let mut refs: Vec<String> = refs.into_iter().collect(); refs.sort_by(|a, b| super::bom::cmp_ref(a, b));
    (refs, nets.into_iter().collect())
}

fn mtime(p: &str) -> u128 { std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos()) }
type NamesCache = HashMap<String, ((u128, u128), Value)>;
static NAMES: OnceLock<Mutex<NamesCache>> = OnceLock::new();

/// `kicad.names`' answer for a project, kept while its board and root sheet are unchanged.
pub fn names_answer(files: &project::Files) -> Value {
    let stamp = (mtime(&files.pcb), mtime(&files.sch));
    let cache = NAMES.get_or_init(Default::default);
    if let Some((s, v)) = cache.lock().unwrap().get(&files.pro) { if *s == stamp { return v.clone() } }
    let (refs, nets) = project_names(&files.pcb, &files.sch);
    let v = json!({"pro": files.pro, "pcb": files.pcb, "sch": files.sch, "references": refs, "nets": nets});
    cache.lock().unwrap().insert(files.pro.clone(), (stamp, v.clone()));
    v
}

// Hub side ------------------------------------------------------------------------------------------------------------------------

static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);

pub fn start(hub: &Arc<Hub>) {
    *HUB.lock().unwrap() = Some(Arc::downgrade(hub));
    hub.register_rpc(Arc::new(XprobeRpc));
}

/// The KiCad project's files, from any path in it (main's canonical copy).
pub fn files_of(path: &str) -> Option<project::Files> {
    let pro = ipc::pro_of(path);
    if pro.is_empty() { return None }
    let norm = |s: &str| {
        let mut n = s.replace('\\', "/");
        if n.len() >= 2 && n.as_bytes()[0].is_ascii_alphabetic() && n.as_bytes()[1] == b':' { n.make_ascii_lowercase(); }
        n
    };
    let dir = Path::new(&pro).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
    let mut f = project::read(&dir);
    if f.pro.is_empty() || norm(&f.pro) != norm(&pro) {
        let p = Path::new(&pro);
        let (pcb, sch) = (p.with_extension("kicad_pcb"), p.with_extension("kicad_sch"));
        f = project::Files { pro: pro.clone(), pcb: if pcb.is_file() { pcb.to_string_lossy().into() } else { String::new() }, sch: if sch.is_file() { pcb_or_sch_str(&sch) } else { String::new() }, all: vec![] };
    }
    Some(f)
}
fn pcb_or_sch_str(p: &Path) -> String { p.to_string_lossy().into() }

/// The path a request names: `project` (or `path`), else the design root of `thread` (its project's root; a worktree's main copy).
async fn named_path(hub: &Arc<Hub>, p: &Value) -> String {
    let raw = p["project"].as_str().or_else(|| p["path"].as_str()).unwrap_or("");
    if !raw.is_empty() { return raw.to_string() }
    let tid = p["thread"].as_str().unwrap_or(""); if tid.is_empty() { return String::new() }
    let m = hub.model().await;
    let Some(t) = m.threads.get(tid) else { return String::new() };
    if !t.worktree.is_empty() { return super::canonical::design_root(&t.worktree) }
    m.projects.get(&t.project).filter(|p| !p.removed).map(|p| p.root.clone()).unwrap_or_default()
}

async fn roots(hub: &Arc<Hub>) -> Vec<String> { let m = hub.model().await; m.projects.values().filter(|p| !p.removed).map(|p| p.root.clone()).collect() }

/// The board probe set for every root holding the project (the viewers light it).
async fn set_probe(hub: &Arc<Hub>, files: &project::Files, n: &Names) -> String {
    let rs = roots(hub).await;
    let board_key = rs.iter().find(|r| !files.pcb.is_empty() && rel_under(r, &files.pcb).is_some()).map(|r| format!("0|{}|{}", r.trim_end_matches(['/', '\\']), files.pcb)).unwrap_or_default();
    let text = probe_text(n, &board_key);
    let at = crate::hub::now_ms() / 1000;
    let anchor = if files.pcb.is_empty() { files.sch.clone() } else { files.pcb.clone() };
    let batch: Vec<Value> = probe_keys(&rs, &anchor).into_iter().map(|k| json!({"$":"SettingSet","key":k,"value":text,"at":at})).collect();
    if !batch.is_empty() { let _ = hub.commit_many(&batch).await; }
    text
}

/// The open frames of the project's GUI KiCad: (kind, shown).
async fn open_frames(route: &embed::GuiRoute) -> Vec<(String, bool)> {
    let o = embed::gui_call(route, "kiapi.backplane.embed.GetFrames", json!({})).await;
    if !o["ok"].as_bool().unwrap_or(false) { return vec![] }
    embed::parse_get_frames(&o["message"]).2.into_iter().map(|f| (f.kind, f.shown)).collect()
}

/// Whether the GUI's runtime has bp.9's SelectItems.
fn has_select_items(route: &embed::GuiRoute) -> bool {
    route.mode == "fork" && !route.desc.is_empty() && super::proto::load(&route.desc).has_own("kiapi.backplane.embed.SelectItems")
}

/// Select `n` in the project's open KiCad: Ok({kicad, found, not_found, via}); kicad false when none is open.
pub async fn select_in_kicad(pro: &str, n: &Names, zoom: bool, show: &str) -> Result<Value, String> {
    let Some(route) = embed::gui_route(pro) else { return Ok(json!({"kicad": false})) };
    let bp9 = has_select_items(&route);
    let open = if route.mode == "fork" { open_frames(&route).await } else { vec![] };
    if !show.is_empty() && bp9 { embed::show_for(pro, show, "").await?; }
    if !bp9 {
        // bp.8 (and stock): ShowFrame's item cross-probes one part through KIWAY (select and centre); a net or nothing found is not selected
        let item = n.first_part();
        let target = targets(&open, show, false).first().copied().unwrap_or("");
        if route.mode != "fork" || target.is_empty() {
            if !show.is_empty() { embed::show_for(pro, show, "").await?; }
            return Ok(json!({"kicad": true, "via": "none", "found": Names::default().to_json(), "not_found": n.to_json()}))
        }
        embed::show_for(pro, target, item.as_deref().unwrap_or("")).await?;
        let found = Names { references: item.clone().into_iter().collect(), ..Default::default() };
        let mut rest = n.clone(); rest.references.retain(|r| Some(r) != item.as_ref());
        return Ok(json!({"kicad": true, "via": "ShowFrame", "frames": [target], "found": found.to_json(), "not_found": rest.to_json()}))
    }
    let mut replies = vec![]; let mut sent = vec![];
    for kind in targets(&open, show, true) {
        let mut tries = 0;
        loop {
            let o = embed::gui_call(&route, "kiapi.backplane.embed.SelectItems", select_msg(kind, n, zoom)).await;
            if o["ok"].as_bool().unwrap_or(false) { replies.push(o["message"].clone()); sent.push(kind); break }
            let st = o["status"].as_str().unwrap_or("");
            if st == "AS_BUSY" { return Err(format!("KiCad is busy: {}", o["error"].as_str().unwrap_or("a dialog is open"))) }
            // a frame not open yet answers AS_UNHANDLED; AS_NOT_READY while it opens
            if st == "AS_NOT_READY" && tries < 5 { tries += 1; tokio::time::sleep(std::time::Duration::from_millis(150)).await; continue }
            break
        }
    }
    let (found, missing) = if replies.is_empty() { (Names::default(), n.clone()) } else { merge_found(&replies) };
    Ok(json!({"kicad": true, "via": "SelectItems", "frames": sent, "found": found.to_json(), "not_found": missing.to_json()}))
}

/// KiCad's ET_SELECTION_CHANGED (the person's selection in the PCB or schematic editor): info `kicad.sel.<digest>` and the board probe.
pub fn kicad_selected(pro: &str, ev: &Value) {
    let Some((kind, n)) = selection_event(ev) else { return };
    let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) else { return };
    let pro = pro.to_string();
    tokio::spawn(async move {
        let Some(files) = files_of(&pro) else { return };
        let probe = set_probe(&hub, &files, &n).await;
        let v = json!({"project": pro, "frame": kind, "selection": n.to_json(), "probe": probe, "at": crate::hub::now_ms()});
        hub.set_info(&format!("kicad.sel.{}", ks::digest(&pro)), &v.to_string()).await;
    });
}

/// `editor.kicad.select` from anywhere in the hub (the MCP tool `kicad_select`): KiCad's selection, and the viewers' probe unless `from_viewer`.
pub async fn select(hub: &Arc<Hub>, p: &Value) -> Result<Value, String> {
    let raw = named_path(hub, p).await; let raw = raw.as_str();
    if !crate::plat::fs::is_abs(raw) { return Err("project must be an absolute path (.kicad_pro, .kicad_pcb, .kicad_sch, or project folder)".into()) }
    let Some(files) = files_of(raw) else { return Err(format!("no KiCad project (.kicad_pro) found for {raw}")) };
    let root = Path::new(&files.pro).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
    if !super::known_root(hub, &root, "").await { return Err(format!("{} is not in a project this hub knows", files.pro)) }
    let n = names_of(p);
    let show = show_frame(p["show"].as_str().unwrap_or(""));
    if n.is_empty() && show.is_empty() { return Err("nothing to select: give references (U3), nets (GND) or pins (U3.4)".into()) }
    let zoom = p["zoom"].as_bool().unwrap_or(true);
    let probe = if p["from"] == "viewer" { String::new() } else { set_probe(hub, &files, &n).await };
    let mut out = select_in_kicad(&files.pro, &n, zoom, show).await?;
    out["project"] = json!(files.pro); out["probe"] = json!(probe);
    Ok(out)
}

pub struct XprobeRpc;
impl RpcHandler for XprobeRpc {
    fn methods(&self) -> &'static [&'static str] { &["editor.kicad.select", "kicad.names"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let reply = |r: Result<Value, String>| match r {
                Ok(v) => { let mut o = json!({"t":"reply","id":id,"ok":true}); if let Some(m) = v.as_object() { for (k, x) in m { o[k] = x.clone(); } } vec![o] }
                Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})],
            };
            match method {
                "editor.kicad.select" => {
                    if crate::copy::on() { return reply(Err("a hub on a copied log (BACKPLANE_COPY=1) does not control GUI KiCad".into())) }
                    reply(select(hub, p).await)
                }
                "kicad.names" => {
                    let raw = named_path(hub, p).await; let raw = raw.as_str();
                    if raw.is_empty() { return reply(Ok(json!({"references": [], "nets": [], "pro": "", "thread": p["thread"]}))) }
                    if !crate::plat::fs::is_abs(raw) { return reply(Err("project must be an absolute path".into())) }
                    let Some(files) = files_of(raw) else { return reply(Ok(json!({"references": [], "nets": [], "pro": ""}))) };
                    let root = Path::new(&files.pro).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                    if !super::known_root(hub, &root, "").await { return reply(Err(format!("{} is not in a project this hub knows", files.pro))) }
                    let mut v = tokio::task::spawn_blocking(move || names_answer(&files)).await.unwrap_or_else(|_| json!({}));
                    v["project"] = json!(raw); v["thread"] = p["thread"].clone();
                    reply(Ok(v))
                }
                _ => reply(Err("unknown method".into())),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn references_pins_and_nets_are_told_apart() {
        for r in ["U3", "R12", "SW1", "U3A", "J10", "TP1"] { assert!(is_reference(r), "{r}") }
        for r in ["", "3U", "U", "GND", "U3ab", "#PWR01", "VERYLONGREF1", "u3.4", "U3.4"] { assert!(!is_reference(r), "{r}") }
        assert!(is_pin("U3.4") && is_pin("J1.A12") && !is_pin("U3.") && !is_pin("GND.1") && !is_pin("U3"));
        let n = names_of(&json!({"references": ["U3", " R12 ", "U3", "nope!"], "nets": ["GND", "", "/SDA", "a;b"], "pins": ["U3.4", "x"]}));
        assert_eq!(n, Names { references: vec!["U3".into(), "R12".into()], nets: vec!["GND".into(), "/SDA".into()], pins: vec!["U3.4".into()] });
        assert_eq!(names_of(&json!({"references": "U1"})).references, vec!["U1"]);
        assert_eq!(Names { pins: vec!["U7.2".into()], ..Default::default() }.first_part().as_deref(), Some("U7"));
    }

    #[test] fn the_editors_a_selection_goes_to() {
        let open = vec![("PCB".to_string(), true), ("SCH".to_string(), false), ("PCB_3D".to_string(), false)];
        assert_eq!(targets(&open, "", true), vec!["PCB", "SCH"]);
        assert_eq!(targets(&open, "SCH", true), vec!["SCH", "PCB"]);
        assert_eq!(targets(&open[..1], "", true), vec!["PCB"]);
        // bp.8's ShowFrame maps the frame it probes: only the shown one, or the one asked for
        assert_eq!(targets(&open, "", false), vec!["PCB"]);
        assert_eq!(targets(&open, "SCH", false), vec!["SCH"]);
        assert!(targets(&[("PCB_3D".to_string(), true)], "", false).is_empty());
        assert_eq!(show_frame("schematic"), "SCH"); assert_eq!(show_frame("PCB"), "PCB"); assert_eq!(show_frame(""), ""); assert_eq!(show_frame("3d"), "");
        let n = Names { references: vec!["U1".into()], nets: vec!["GND".into()], pins: vec![] };
        assert_eq!(select_msg("SCH", &n, true), json!({"kind":"FK_SCH","references":["U1"],"nets":["GND"],"pins":[],"zoom":true}));
    }

    #[test] fn found_in_either_editor_is_found() {
        let (f, m) = merge_found(&[
            json!({"found": {"references": ["U1"]}, "not_found": {"references": ["R9"], "nets": ["GND"]}}),
            json!({"found": {"nets": ["GND"]}, "not_found": {"references": ["U1", "R9"]}}),
        ]);
        assert_eq!(f.references, vec!["U1"]); assert_eq!(f.nets, vec!["GND"]);
        assert_eq!(m.references, vec!["R9"]); assert!(m.nets.is_empty());
    }

    #[test] fn the_viewers_probe_for_names() {
        let part = Names { references: vec!["U3".into(), "R1".into()], ..Default::default() };
        assert_eq!(probe_text(&part, "0|/p|/p/a.kicad_pcb"), "footprint U3;ref=U3");
        let pin = Names { pins: vec!["U3.4".into()], ..Default::default() };
        assert_eq!(probe_text(&pin, ""), "footprint U3;ref=U3;pin=4");
        let net = Names { nets: vec!["/SDA".into()], ..Default::default() };
        assert_eq!(probe_text(&net, "0|/p|/p/a.kicad_pcb"), "net /SDA;net=/SDA;g=0|/p|/p/a.kicad_pcb\u{1}");
        assert_eq!(probe_text(&Names::default(), ""), "");
        let keys = probe_keys(&["/p".into(), "/q".into(), "/p/boards".into()], "/p/boards/usb/usb.kicad_pcb");
        assert_eq!(keys, vec!["probe:/p|", "probe:/p|boards/usb/usb.kicad_pcb", "probe:/p/boards|", "probe:/p/boards|usb/usb.kicad_pcb"]);
        let wkeys = probe_keys(&[r"C:\bp\proj".into(), r"C:\bp\proj\boards\".into(), r"c:\bp\other".into()], r"c:\bp\proj\boards\usb\usb.kicad_pcb");
        assert_eq!(wkeys, vec![r"probe:C:\bp\proj|", r"probe:C:\bp\proj|boards/usb/usb.kicad_pcb", r"probe:C:\bp\proj\boards|", r"probe:C:\bp\proj\boards|usb/usb.kicad_pcb"]);
    }

    #[test] fn a_selection_event_is_read() {
        let (k, n) = selection_event(&json!({"seq": 4, "type": "ET_SELECTION_CHANGED", "frame": {"kind": "FK_SCH"}, "selection": {"references": ["U1"], "pins": ["U1.3"]}})).unwrap();
        assert_eq!(k, "SCH"); assert_eq!(n.references, vec!["U1"]); assert_eq!(n.pins, vec!["U1.3"]);
        let (k, n) = selection_event(&json!({"type": 14, "frame": {"kind": 1}})).unwrap();
        assert_eq!(k, "PCB"); assert!(n.is_empty());
        assert!(selection_event(&json!({"type": "ET_FRAME_SHOWN"})).is_none());
    }

    #[test] fn a_projects_names_come_from_its_board_and_every_sheet() {
        let d = crate::plat::tmp::dir("xprobe");
        std::fs::write(d.join("a.kicad_pcb"), r#"(kicad_pcb (version 20240108) (net 0 "") (net 1 "GND") (net 2 "/SDA")
  (footprint "R_0603" (layer "F.Cu") (property "Reference" "R12") (pad "1" smd rect (net 1 "GND")))
  (footprint "SOT23" (layer "F.Cu") (property "Reference" "U3")))"#).unwrap();
        std::fs::write(d.join("a.kicad_sch"), r##"(kicad_sch (version 20231120)
  (lib_symbols (symbol "Device:R" (property "Reference" "R")))
  (symbol (lib_id "Device:R") (property "Reference" "R1") (instances (project "a" (path "/x" (reference "R1") (unit 1)))))
  (symbol (lib_id "power:GND") (property "Reference" "#PWR01"))
  (label "SCL" (at 0 0 0)) (global_label "VBUS" (shape input))
  (sheet (property "Sheetname" "io") (property "Sheetfile" "io.kicad_sch")))"##).unwrap();
        std::fs::write(d.join("io.kicad_sch"), r#"(kicad_sch (symbol (property "Reference" "J1")) (hierarchical_label "MOSI"))"#).unwrap();
        let (refs, nets) = project_names(&d.join("a.kicad_pcb").to_string_lossy(), &d.join("a.kicad_sch").to_string_lossy());
        assert_eq!(refs, vec!["J1", "R1", "R12", "U3"]);
        assert_eq!(nets, vec!["/SDA", "GND", "MOSI", "SCL", "VBUS"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
