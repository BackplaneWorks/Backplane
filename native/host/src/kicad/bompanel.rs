//! The BOM management panel (docs/MANAGEMENT.md): the project's schematic as BOM lines, live with the design, published as info
//! `mgmt.bom.<digest>` for the desktop and the phones.
//!
//! Where the parts come from (`source`): "file" (the saved schematic and every sheet it reaches, read again when any of them changes
//! on disk: their stamps are polled once a second while a client has shown the project in the last half hour).
//!
//! Published: {badge: {n, tone}, source, at, parts, missing_mpn, dnp, lines: [{id, refs, qty, value, footprint, mpn, mfr, fab, dnp}]}.
//! `id` is the line's first reference (what an edit names); `fab` is the fab's part number field (`LCSC`). The badge counts the lines
//! the manufacturing review would stop on (no MPN), once some line has an MPN or a fab number, as bom::rules does.
use super::{bom, mgmt, plotread, watch};
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock, Weak}, time::{Duration, Instant}};

/// How long a project stays watched after a client last showed it.
const WATCH_FOR: Duration = Duration::from_secs(30 * 60);
const POLL: Duration = Duration::from_secs(1);
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
/// The panel's data for a set of parts.
pub fn data(ps: &[bom::Part], source: &str, at: u64) -> Value {
    let mut ls = lines(ps);
    let miss = missing_mpn(&ls);
    let dnp = ls.iter().filter(|l| l["dnp"] == true).count();
    let more = ls.len().saturating_sub(MAX_LINES); ls.truncate(MAX_LINES);
    json!({"badge": {"n": miss, "tone": if miss > 0 { "warn" } else { "" }}, "source": source, "at": at, "parts": ps.len(), "lines": ls, "missing_mpn": miss, "dnp": dnp, "more": more})
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
    let ps: Vec<bom::Part> = items.iter().filter_map(part_of_item).filter(|p| p.inbom).collect();
    Some(ps)
}

/// A project's name for its symbols' instances: the `.kicad_pro`'s stem.
pub fn project_name(pro: &str) -> String { std::path::Path::new(pro).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() }
fn stamps(paths: &[String]) -> Vec<String> { paths.iter().map(|p| format!("{:?}", watch::stamp(p))).collect() }

struct Watched { sch: String, seen: Instant, paths: Vec<String>, stamps: Vec<String> }
static W: OnceLock<Mutex<HashMap<String, Watched>>> = OnceLock::new();
fn w() -> &'static Mutex<HashMap<String, Watched>> { W.get_or_init(Default::default) }
static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);

/// The parts of a project now, from its files.
pub fn parts_now(pro: &str, sch: &str) -> (Vec<bom::Part>, Vec<String>) {
    let shs = read_sheets(sch);
    let paths = shs.iter().map(|s| s.path.clone()).collect();
    (bom::parts(&project_name(pro), &shs), paths)
}
/// Read and publish a project's BOM now.
pub async fn refresh(hub: &Arc<Hub>, pro: &str) {
    let Some(sch) = w().lock().unwrap().get(pro).map(|x| x.sch.clone()) else { return };
    let (p2, s2) = (pro.to_string(), sch.clone());
    let Ok((ps, paths)) = tokio::task::spawn_blocking(move || parts_now(&p2, &s2)).await else { return };
    let st = stamps(&paths);
    if let Some(x) = w().lock().unwrap().get_mut(pro) { x.paths = paths; x.stamps = st; }
    mgmt::publish(hub, "bom", pro, data(&ps, "file", super::ipc::now_s())).await;
}

pub struct BomPanel;
impl mgmt::Panel for BomPanel {
    fn id(&self) -> &'static str { "bom" }
    fn label(&self) -> &'static str { "BOM" }
    fn watch(&self, hub: &Arc<Hub>, pro: &str) {
        let sch = super::project::read(std::path::Path::new(pro).parent().and_then(|p| p.to_str()).unwrap_or("")).sch;
        let sch = if sch.is_empty() { std::path::Path::new(pro).with_extension("kicad_sch").to_string_lossy().into_owned() } else { sch };
        let fresh = { let mut m = w().lock().unwrap(); let fresh = !m.contains_key(pro); let e = m.entry(pro.to_string()).or_insert(Watched { sch: sch.clone(), seen: Instant::now(), paths: vec![], stamps: vec![] }); e.seen = Instant::now(); e.sch = sch; fresh };
        let (hub, pro) = (hub.clone(), pro.to_string());
        // a project shown again publishes again (a client that joined has it from the hub's info; this is for one the hub forgot)
        tokio::spawn(async move { if fresh || !hub.info_now()[mgmt::data_key("bom", &pro)].is_string() { refresh(&hub, &pro).await } });
    }
}

pub fn start(hub: &Arc<Hub>) {
    *HUB.lock().unwrap() = Some(Arc::downgrade(hub));
    mgmt::register(Arc::new(BomPanel));
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(POLL).await;
            let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) else { return };
            let due: Vec<String> = {
                let mut m = w().lock().unwrap();
                m.retain(|_, x| x.seen.elapsed() < WATCH_FOR);
                m.iter().filter(|(_, x)| x.paths.is_empty() || stamps(&x.paths) != x.stamps).map(|(k, _)| k.clone()).collect()
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
        let d = data(&started, "file", 5); assert_eq!((d["badge"]["n"].as_u64(), d["badge"]["tone"].as_str(), d["dnp"].as_u64(), d["parts"].as_u64()), (Some(1), Some("warn"), Some(1), Some(3)));
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
