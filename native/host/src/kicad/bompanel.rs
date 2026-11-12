//! The BOM management panel (docs/MANAGEMENT.md): the project's schematic as BOM lines, live with the design, published as info
//! `mgmt.bom.<digest>` for the desktop and the phones.
//!
//! Where the parts come from (`source`): "kicad" while the project's GUI KiCad has its schematic open (GetItems KOT_SCH_SYMBOL over its API,
//! once a second: an edit in the editor, or an agent's through it, shows without a save), else "file" (the saved schematic and every sheet it
//! reaches, read again when any of them changes on disk: their stamps are polled twice a second). Either while a client has shown the project
//! in the last half hour.
//!
//! Published: {badge: {n, tone}, source, at, parts, missing_mpn, dnp, lines: [{id, refs, qty, value, footprint, mpn, mfr, fab, dnp}]}.
//! `id` is the line's first reference (what an edit names); `fab` is the fab's part number field (`LCSC`). The badge counts the lines
//! the manufacturing review would stop on (no MPN), once some line has an MPN or a fab number, as bom::rules does.
use super::{bom, mgmt, parts, plotread, watch};
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock, Weak}, time::{Duration, Instant}};

/// How long a project stays watched after a client last showed it.
const WATCH_FOR: Duration = Duration::from_secs(30 * 60);
const POLL: Duration = Duration::from_millis(500);
/// A GUI KiCad's schematic is read every this many polls (1 s).
const LIVE_EVERY: u64 = 2;
const MAX_SHEETS: usize = 64;
/// Lines past this are not sent (the info value's bound); `more` says how many were left out.
const MAX_LINES: usize = 400;

/// The fab's part number of a part: its `LCSC` field.
pub fn fab_of(p: &bom::Part) -> String { p.fields.iter().find(|f| f.k.eq_ignore_ascii_case("lcsc")).map(|f| f.v.clone()).unwrap_or_default() }

/// The published lines of a set of parts: bom::group's lines in its order, each with the fab number its parts share (the first part's).
pub fn lines(ps: &[bom::Part]) -> Vec<Value> {
    let by: HashMap<&str, &bom::Part> = ps.iter().map(|p| (p.reference.as_str(), p)).collect();
    bom::group(ps).into_iter().map(|l| {
        let first = l.refs.first().cloned().unwrap_or_default();
        let fab = l.refs.iter().filter_map(|r| by.get(r.as_str())).map(|p| fab_of(p)).find(|f| !f.is_empty()).unwrap_or_default();
        json!({"id": first, "refs": l.refs, "qty": l.qty, "value": l.value, "footprint": l.footprint, "mpn": l.mpn, "mfr": l.mfr, "fab": fab, "dnp": l.dnp})
    }).collect()
}
/// Lines that need an MPN: none until some line has an MPN or a fab number (a BOM nobody has started on is not a hundred warnings);
/// DNP lines are not ordered, so they need none.
pub fn missing_mpn(ls: &[Value]) -> usize {
    let started = ls.iter().any(|l| l["mpn"].as_str().map_or(false, |s| !s.is_empty()) || l["fab"].as_str().map_or(false, |s| !s.is_empty()));
    if !started { return 0 }
    ls.iter().filter(|l| l["dnp"] != true && l["mpn"].as_str().map_or(true, str::is_empty)).count()
}
/// The panel's data for a set of parts. `boards`: how many boards are made (the fab file's qty); `quote`: the fab's stock for a lookup key
/// (kicad/parts.rs), None while it is being checked. Each line that can be looked up carries `stock` (parts::line_stock) for its quantity
/// on all the boards, and the BOM `cost`: the lines' unit prices times their quantities, for one board (`priced` lines of `lookups`).
pub fn data(ps: &[bom::Part], source: &str, at: u64, boards: u64, quote: &dyn Fn(&str) -> Option<parts::Quote>) -> Value {
    let mut ls = lines(ps);
    let miss = missing_mpn(&ls);
    let dnp = ls.iter().filter(|l| l["dnp"] == true).count();
    let (mut out, mut checking, mut looked, mut priced, mut cost) = (0usize, 0usize, 0usize, 0usize, 0f64);
    for l in ls.iter_mut() {
        if l["dnp"] == true { continue }
        let Some(k) = parts::key_of(l["fab"].as_str().unwrap_or(""), l["mpn"].as_str().unwrap_or("")) else { continue };
        let qty = l["qty"].as_u64().unwrap_or(1); let need = qty * boards.max(1);
        let st = parts::line_stock(quote(&k).as_ref(), need);
        looked += 1;
        match st["status"].as_str().unwrap_or("") { "out" => out += 1, "checking" => checking += 1, _ => {} }
        if let Some(p) = st["price"].as_f64() { priced += 1; cost += p * qty as f64; }
        l["key"] = json!(k); l["stock"] = st;
    }
    let more = ls.len().saturating_sub(MAX_LINES); ls.truncate(MAX_LINES);
    let n = miss + out;
    json!({"badge": {"n": n, "tone": if out > 0 { "bad" } else if n > 0 { "warn" } else { "" }}, "source": source, "at": at, "parts": ps.len(), "lines": ls,
        "missing_mpn": miss, "out": out, "checking": checking, "dnp": dnp, "more": more, "boards": boards.max(1), "lookups": looked, "priced": priced, "paused": checking > 0 && parts::paused(), "cost": (cost * 10000.0).round() / 10000.0})
}
/// A line's `lifecycle` {status: active|nrnd|eol|unknown, words (the distributor's own), source, checked} from its part's history, and
/// `library` {class: basic|preferred|extended, checked} from its stock answer; neither when the part was never checked.
pub fn annotate(l: &mut Value, hist: &HashMap<String, Vec<Value>>) {
    let Some(k) = l["key"].as_str().map(str::to_string) else { return };
    let ss = hist.get(&k).map(Vec::as_slice).unwrap_or(&[]);
    if let Some(r) = ss.iter().rev().find(|s| s["life_at"].is_u64()) {
        let words = r["lifecycle"].as_str().unwrap_or("");
        l["lifecycle"] = json!({"status": super::lifepanel::lifecycle_of(words), "words": words, "source": r["source"].as_str().unwrap_or("mouser"), "checked": r["life_at"]});
    }
    let st = &l["stock"];
    if let (Some(c), Some(at)) = (st["class"].as_str().filter(|c| !c.is_empty()), st["at"].as_u64()) { l["library"] = json!({"class": c, "checked": at}); }
    if let Some(at) = l["stock"]["at"].as_u64() { l["stock"]["checked"] = json!(at); }
}
/// How many boards the project makes: the `qty` of the first board fab file (`<name>.fab.json`, a board kind) beside the project, else 1.
pub fn boards_of(pro: &str) -> u64 {
    let Some(dir) = std::path::Path::new(pro).parent() else { return 1 };
    let mut fs: Vec<std::path::PathBuf> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".fab.json")).collect()).unwrap_or_default();
    fs.sort();
    fs.iter().filter_map(|p| serde_json::from_str::<Value>(&std::fs::read_to_string(p).ok()?).ok())
        .find(|v| matches!(v["kind"].as_str(), Some("pcb" | "flex" | "hdi" | "aluminum" | "copper_core" | "rogers" | "ptfe")))
        .and_then(|v| v["qty"].as_u64()).filter(|q| *q > 0).unwrap_or(1)
}

/// The schematic files of a project: its root sheet and every sheet it reaches (once each, at most MAX_SHEETS), with their bytes.
pub fn read_sheets(root: &str) -> Vec<bom::Sheet> {
    let (mut todo, mut known, mut out): (Vec<String>, Vec<String>, Vec<bom::Sheet>) = (vec![root.to_string()], vec![], vec![]);
    while let Some(p) = (!todo.is_empty()).then(|| todo.remove(0)) {
        if out.len() >= MAX_SHEETS { break }
        let Ok(bs) = std::fs::read(&p) else { known.push(p); continue };
        for (_, f) in plotread::kids(&p, &bs).sheets { if !known.contains(&f) && !todo.contains(&f) && f != p { todo.push(f) } }
        known.push(p.clone());
        out.push(bom::Sheet { path: p, text: bs });
    }
    out
}
// Live from the GUI KiCad ----------------------------------------------------------------------------------------------

fn txt(f: &Value) -> String { let t = f["text"]["text"].as_str().unwrap_or(""); if t == "~" { String::new() } else { t.to_string() } }
/// One `SchematicSymbolInstance` (GetItems' JSON, the .proto's field names) as a BOM part; None for a power symbol or flag.
pub fn part_of_item(it: &Value) -> Option<bom::Part> {
    let reference = txt(&it["reference_field"]);
    if reference.is_empty() || reference.starts_with('#') { return None }
    let mut fields: Vec<bom::Fld> = vec![bom::Fld { k: "Reference".into(), v: reference.clone() }, bom::Fld { k: "Value".into(), v: txt(&it["value_field"]) }, bom::Fld { k: "Footprint".into(), v: txt(&it["footprint_field"]) }];
    fields.extend(it["user_fields"].as_array().into_iter().flatten().map(|f| bom::Fld { k: f["name"].as_str().unwrap_or("").to_string(), v: txt(f) }));
    let get = |names: &[&str]| names.iter().find_map(|n| fields.iter().find(|f| f.k.eq_ignore_ascii_case(n)).map(|f| f.v.clone()).filter(|v| !v.is_empty())).unwrap_or_default();
    let a = &it["attributes"];
    Some(bom::Part { reference, value: txt(&it["value_field"]), footprint: txt(&it["footprint_field"]),
        mpn: get(&["mpn", "manufacturer_part_number", "manufacturer part number", "mfr pn", "pn"]), mfr: get(&["manufacturer", "mfr"]),
        fields: fields.iter().filter(|f| matches!(f.k.to_ascii_lowercase().as_str(), "lcsc" | "mouser" | "digikey") && !f.v.is_empty()).cloned().collect(),
        dnp: a["do_not_populate"].as_bool().unwrap_or(false), inbom: !a["exclude_from_bill_of_materials"].as_bool().unwrap_or(false),
        lib: String::new(), sheet: String::new(), unit: it["unit"]["unit"].as_u64().unwrap_or(1) as u128 })
}
/// The parts the GUI KiCad holds for the project now (its open schematic, unsaved edits included); None when no GUI KiCad has the
/// schematic open or it would not answer.
pub async fn live_parts(pro: &str) -> Option<Vec<bom::Part>> {
    let gui = super::embed::gui_route(pro)?;
    let docs = super::embed::gui_call(&gui, "kiapi.common.commands.GetOpenDocuments", json!({"type": "DOCTYPE_SCHEMATIC"})).await;
    let doc = docs["message"]["documents"].as_array()?.first()?.clone();
    let r = super::embed::gui_call(&gui, "kiapi.common.commands.GetItems", json!({"header": {"document": doc}, "types": ["KOT_SCH_SYMBOL"]})).await;
    if r["ok"] != true { return None }
    let items = r["message"]["items"].as_array()?;
    // a multi-unit part is one part (its first unit), as in the files
    let mut seen = std::collections::HashSet::new();
    let ps: Vec<bom::Part> = items.iter().filter_map(part_of_item).filter(|p| p.inbom && seen.insert(p.reference.clone())).collect();
    Some(ps)
}

/// A project's name for its symbols' instances: the `.kicad_pro`'s stem.
pub fn project_name(pro: &str) -> String { std::path::Path::new(pro).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() }
fn stamps(paths: &[String]) -> Vec<String> { paths.iter().map(|p| format!("{:?}", watch::stamp(p))).collect() }

struct Watched { sch: String, seen: Instant, paths: Vec<String>, stamps: Vec<String>, last: Value, ps: Vec<bom::Part>, source: String }
static W: OnceLock<Mutex<HashMap<String, Watched>>> = OnceLock::new();
fn w() -> &'static Mutex<HashMap<String, Watched>> { W.get_or_init(Default::default) }
static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);

/// The parts of a project now, from its files.
pub fn parts_now(pro: &str, sch: &str) -> (Vec<bom::Part>, Vec<String>) {
    let shs = read_sheets(sch);
    let paths = shs.iter().map(|s| s.path.clone()).collect();
    (bom::parts(&project_name(pro), &shs), paths)
}
/// Read and publish a project's BOM now: from the GUI KiCad when it has the schematic open (its unsaved edits too), else from the files.
/// Published only when it changed (the time it was read aside).
pub async fn refresh(hub: &Arc<Hub>, pro: &str) {
    let Some((sch, known, old)) = w().lock().unwrap().get(pro).map(|x| (x.sch.clone(), x.paths.clone(), x.stamps.clone())) else { return };
    let live = live_parts(pro).await.filter(|l| !l.is_empty());
    // the files are read when they changed (or never were): their sheets are what is watched while KiCad is closed
    let files = if known.is_empty() || stamps(&known) != old || live.is_none() {
        let (p2, s2) = (pro.to_string(), sch.clone());
        tokio::task::spawn_blocking(move || parts_now(&p2, &s2)).await.ok()
    } else { None };
    let (ps, source) = match (live, &files) { (Some(l), _) => (l, "kicad"), (None, Some((f, _))) => (f.clone(), "file"), (None, None) => return };
    if let Some(x) = w().lock().unwrap().get_mut(pro) { if let Some((_, paths)) = files { x.stamps = stamps(&paths); x.paths = paths; } x.ps = ps; x.source = source.into(); }
    publish(hub, pro).await;
}
/// Publish a project's BOM from the parts last read (again when a stock answer came in), when it changed (the time it was read aside).
pub async fn publish(hub: &Arc<Hub>, pro: &str) {
    let Some((ps, source)) = w().lock().unwrap().get(pro).map(|x| (x.ps.clone(), x.source.clone())) else { return };
    let (p2, boards) = (pro.to_string(), { let p = pro.to_string(); tokio::task::spawn_blocking(move || boards_of(&p)).await.unwrap_or(1) });
    let mut d = data(&ps, &source, 0, boards, &|k| parts::quote(k));
    let keys: Vec<String> = d["lines"].as_array().into_iter().flatten().filter_map(|l| l["key"].as_str().map(str::to_string)).collect();
    parts::want(&keys);
    // each line's lifecycle and library class with when they were checked (the design review reads them here, never the parts search)
    let home = hub.cfg.home.clone(); let ks = keys.clone();
    let hist: HashMap<String, Vec<Value>> = tokio::task::spawn_blocking(move || ks.into_iter().map(|k| { let v = super::parthist::read(&home, &k); (k, v) }).collect()).await.unwrap_or_default();
    if let Some(ls) = d["lines"].as_array_mut() { for l in ls.iter_mut() { annotate(l, &hist) } }
    let same = { let mut m = w().lock().unwrap(); match m.get_mut(&p2) { Some(x) => { let same = x.last == d; x.last = d.clone(); same } None => true } };
    // the lifecycle panel follows each BOM read and each stock answer (its history moved), even when the BOM's own data did not
    super::lifepanel::publish(hub, pro, &d, &hist).await;
    if same && hub.info_now()[mgmt::data_key("bom", pro)].is_string() { return }
    d["at"] = json!(super::ipc::now_s());
    mgmt::publish(hub, "bom", pro, d).await;
}

/// The project's root schematic.
pub fn sch_of(pro: &str) -> String {
    let sch = super::project::read(std::path::Path::new(pro).parent().and_then(|p| p.to_str()).unwrap_or("")).sch;
    if sch.is_empty() { std::path::Path::new(pro).with_extension("kicad_sch").to_string_lossy().into_owned() } else { sch }
}

pub struct BomPanel;
impl mgmt::Panel for BomPanel {
    fn id(&self) -> &'static str { "bom" }
    fn label(&self) -> &'static str { "BOM" }
    fn watch(&self, hub: &Arc<Hub>, pro: &str) {
        let sch = sch_of(pro);
        let fresh = { let mut m = w().lock().unwrap(); let fresh = !m.contains_key(pro); let e = m.entry(pro.to_string()).or_insert(Watched { sch: sch.clone(), seen: Instant::now(), paths: vec![], stamps: vec![], last: Value::Null, ps: vec![], source: String::new() }); e.seen = Instant::now(); e.sch = sch; fresh };
        let (hub, pro) = (hub.clone(), pro.to_string());
        // a project shown again publishes again (a client that joined has it from the hub's info; this is for one the hub forgot)
        tokio::spawn(async move { if fresh || !hub.info_now()[mgmt::data_key("bom", &pro)].is_string() { refresh(&hub, &pro).await } });
    }
}

pub fn start(hub: &Arc<Hub>) {
    *HUB.lock().unwrap() = Some(Arc::downgrade(hub));
    mgmt::register(Arc::new(BomPanel));
    hub.register_rpc(Arc::new(super::bomedit::BomEditRpc));
    tokio::spawn(async move {
        let (mut tick, mut gen): (u64, u64) = (0, 0);
        loop {
            tokio::time::sleep(POLL).await; tick += 1;
            // a stock answer came in: every watched BOM shows it
            if parts::generation() != gen {
                gen = parts::generation();
                let hub = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade);
                if let Some(hub) = hub { let ks: Vec<String> = w().lock().unwrap().keys().cloned().collect(); for k in ks { publish(&hub, &k).await } }
            }
            let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) else { return };
            let due: Vec<String> = {
                let mut m = w().lock().unwrap();
                m.retain(|_, x| x.seen.elapsed() < WATCH_FOR);
                // a saved sheet; or, while a GUI KiCad has the project, every LIVE_EVERY (what the person edits shows without a save)
                let live = tick % LIVE_EVERY == 0;
                m.iter().filter(|(k, x)| x.paths.is_empty() || stamps(&x.paths) != x.stamps || (live && super::embed::gui_route(k).is_some())).map(|(k, _)| k.clone()).collect()
            };
            for pro in due { refresh(&hub, &pro).await }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn part(r: &str, v: &str, f: &str, mpn: &str, fab: &str, dnp: bool) -> bom::Part {
        bom::Part { reference: r.into(), value: v.into(), footprint: f.into(), mpn: mpn.into(), mfr: String::new(), fields: if fab.is_empty() { vec![] } else { vec![bom::Fld { k: "LCSC".into(), v: fab.into() }] }, dnp, inbom: true, lib: String::new(), sheet: String::new(), unit: 1 }
    }
    #[test] fn lines_group_as_a_bom_with_the_fab_number_and_dnp() {
        let ps = vec![part("R2", "10k", "0603", "", "C25804", false), part("R1", "10k", "0603", "", "", false), part("C1", "1u", "0402", "GRM1", "", false), part("R3", "10k", "0603", "", "", true)];
        let ls = lines(&ps);
        let s: Vec<String> = ls.iter().map(|l| format!("{}:{}:{}:{}:{}", l["id"].as_str().unwrap(), l["refs"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect::<Vec<_>>().join(","), l["qty"], l["fab"].as_str().unwrap(), l["dnp"])).collect();
        assert_eq!(s, ["C1:C1:1::false", "R1:R1,R2:2:C25804:false", "R3:R3:1::true"]);
    }
    #[test] fn missing_mpns_count_once_the_bom_is_started_and_not_for_dnp() {
        let none = vec![part("R1", "10k", "0603", "", "", false), part("C1", "1u", "0402", "", "", false)];
        assert_eq!(missing_mpn(&lines(&none)), 0);
        let started = vec![part("R1", "10k", "0603", "", "", false), part("C1", "1u", "0402", "GRM1", "", false), part("R9", "1k", "0603", "", "", true)];
        assert_eq!(missing_mpn(&lines(&started)), 1);
        let d = data(&started, "file", 5, 1, &|_| None); assert_eq!((d["badge"]["n"].as_u64(), d["badge"]["tone"].as_str(), d["dnp"].as_u64(), d["parts"].as_u64()), (Some(1), Some("warn"), Some(1), Some(3)));
        let fab_only = vec![part("R1", "10k", "0603", "", "C1", false), part("C1", "1u", "0402", "", "", false)]; assert_eq!(missing_mpn(&lines(&fab_only)), 2);
    }
    #[test] fn a_live_symbol_reads_as_a_part() {
        let f = |n: &str, t: &str| json!({"name": n, "text": {"position": {"x_nm": "1"}, "text": t}});
        let it = json!({"id": {"value": "u1"}, "reference_field": f("Reference", "R7"), "value_field": f("Value", "10k"), "footprint_field": f("Footprint", "Resistor_SMD:R_0603"),
            "attributes": {"do_not_populate": true}, "unit": {"unit": 1}, "user_fields": [f("MPN", "RC0603FR-0710KL"), f("Manufacturer", "Yageo"), f("LCSC", "C98220"), f("Note", "x")]});
        let p = part_of_item(&it).unwrap();
        assert_eq!((p.reference.as_str(), p.value.as_str(), p.footprint.as_str(), p.mpn.as_str(), p.mfr.as_str(), p.dnp, p.inbom), ("R7", "10k", "Resistor_SMD:R_0603", "RC0603FR-0710KL", "Yageo", true, true));
        assert_eq!(fab_of(&p), "C98220");
        assert!(part_of_item(&json!({"reference_field": f("Reference", "#PWR01")})).is_none());
        let off = json!({"reference_field": f("Reference", "H1"), "attributes": {"exclude_from_bill_of_materials": true}}); assert!(!part_of_item(&off).unwrap().inbom);
    }
    #[test] fn lines_carry_their_stock_and_the_badge_counts_what_cannot_be_ordered() {
        let ps = vec![part("R1", "10k", "0603", "RC1", "", false), part("R2", "10k", "0603", "RC1", "", false), part("C1", "1u", "0402", "", "C5", false), part("U1", "X", "QFN", "", "", false), part("R9", "0R", "0402", "RC9", "", true)];
        let q = |k: &str| match k { "mpn:RC1" => Some(parts::Quote { stock: 1000, breaks: vec![(1, 0.01), (100, 0.005)], class: "basic".into(), ..Default::default() }), "fab:C5" => Some(parts::Quote { stock: 3, breaks: vec![(1, 0.2)], ..Default::default() }), _ => None };
        let d = data(&ps, "file", 0, 10, &q);
        let by = |id: &str| d["lines"].as_array().unwrap().iter().find(|l| l["id"] == id).unwrap().clone();
        assert_eq!(by("R1")["stock"]["status"], "in"); assert_eq!(by("R1")["stock"]["need"], 20); assert_eq!(by("R1")["stock"]["price"], 0.01);
        assert_eq!(by("C1")["stock"]["status"], "out", "3 in stock, 10 needed"); assert_eq!(by("C1")["key"], "fab:C5");
        assert!(by("U1")["stock"].is_null(), "nothing to look up"); assert!(by("R9")["stock"].is_null(), "DNP is not ordered");
        assert_eq!((d["out"].as_u64(), d["missing_mpn"].as_u64(), d["badge"]["n"].as_u64(), d["badge"]["tone"].as_str()), (Some(1), Some(2), Some(3), Some("bad")));
        assert_eq!((d["boards"].as_u64(), d["lookups"].as_u64(), d["priced"].as_u64(), d["cost"].as_f64()), (Some(10), Some(2), Some(2), Some(0.22)));
        let checking = data(&ps, "file", 0, 1, &|_| None); assert_eq!(checking["checking"].as_u64(), Some(2));
    }
    #[test] fn a_line_says_its_lifecycle_and_library_with_when_they_were_checked() {
        let mut l = json!({"key": "mpn:A", "stock": {"status": "in", "class": "basic", "at": 50}});
        let mut h = HashMap::new(); h.insert("mpn:A".to_string(), vec![json!({"at": 10, "stock": 5}), json!({"at": 40, "life_at": 40, "lifecycle": "Obsolete", "source": "mouser"}), json!({"at": 50, "stock": 4})]);
        annotate(&mut l, &h);
        assert_eq!(l["lifecycle"], json!({"status": "eol", "words": "Obsolete", "source": "mouser", "checked": 40}));
        assert_eq!(l["library"], json!({"class": "basic", "checked": 50})); assert_eq!(l["stock"]["checked"], 50);
        let mut n = json!({"key": "mpn:B", "stock": {"status": "checking"}}); annotate(&mut n, &h);
        assert!(n["lifecycle"].is_null() && n["library"].is_null(), "never checked: nothing said");
    }
    #[test] fn the_boards_made_come_from_the_fab_file() {
        let d = crate::plat::tmp::dir("bomqty"); let pro = d.join("x.kicad_pro");
        assert_eq!(boards_of(pro.to_str().unwrap()), 1);
        std::fs::write(d.join("case.fab.json"), r#"{"kind":"3dp","qty":50}"#).unwrap(); assert_eq!(boards_of(pro.to_str().unwrap()), 1);
        std::fs::write(d.join("x.fab.json"), r#"{"kind":"pcb","qty":25}"#).unwrap(); assert_eq!(boards_of(pro.to_str().unwrap()), 25);
    }
    #[test] fn sheets_are_followed_once_each() {
        let d = crate::plat::tmp::dir("bompanel"); let root = d.join("top.kicad_sch"); let sub = d.join("sub.kicad_sch");
        let sym = |r: &str| format!("(symbol (lib_id \"Device:R\") (at 0 0 0) (unit 1) (in_bom yes) (dnp no) (uuid \"u{r}\") (property \"Reference\" \"{r}\" (at 0 0 0)) (property \"Value\" \"1k\" (at 0 0 0)) (property \"Footprint\" \"R_0603\" (at 0 0 0)))");
        let sheet = "(sheet (at 0 0) (size 10 10) (uuid \"s1\") (property \"Sheetname\" \"S\" (at 0 0 0)) (property \"Sheetfile\" \"sub.kicad_sch\" (at 0 0 0)))";
        std::fs::write(&root, format!("(kicad_sch (version 20250114) {} {sheet} {sheet})", sym("R1"))).unwrap();
        std::fs::write(&sub, format!("(kicad_sch (version 20250114) {})", sym("R2"))).unwrap();
        let (ps, paths) = parts_now(d.join("top.kicad_pro").to_str().unwrap(), root.to_str().unwrap());
        assert_eq!(paths.len(), 2); assert_eq!(ps.iter().map(|p| p.reference.as_str()).collect::<Vec<_>>(), ["R1", "R2"]);
        let before = stamps(&paths); std::thread::sleep(Duration::from_millis(20)); std::fs::write(&sub, format!("(kicad_sch (version 20250114) {} {})", sym("R2"), sym("R3"))).unwrap();
        assert_ne!(before, stamps(&paths), "a saved sheet is seen");
    }
}
