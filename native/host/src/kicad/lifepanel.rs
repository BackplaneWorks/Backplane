//! The Lifecycle management panel (docs/MANAGEMENT.md, "Lifecycle"): each BOM part's lifecycle (active, NRND, EOL, unknown) and its stock
//! and price over time, from the parts history (kicad/parthist.rs), which starts when Backplane first looks a part up. Published as info
//! `mgmt.life.<digest>`: {badge, counts: {active, nrnd, eol, unknown}, samples, since, parts: [{key, id, refs, mpn, lifecycle, stock, price,
//! n, first, last}], bom: {cost: [[day, $ a board]], instock: [[day, lines in stock]], lines}}. A part's own series come on demand:
//! `mgmt.life.part {project, key}` -> {key, samples: [{at, stock, price, lifecycle}]}.
//!
//! Lifecycle readings: the distributors' lookups (pcbparts' mouser_get_part, up to 10 parts a call; a shared daily allowance, so at most
//! LIFE_PER_DAY calls a day and each part once a day), recorded in the same history.
use super::{mgmt, parthist, parts};
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{collections::{BTreeMap, HashMap}, path::Path, sync::{Arc, Mutex, OnceLock, Weak}, time::Duration};

pub const LIFE_PER_DAY: usize = 40;
const LIFE_GAP: Duration = Duration::from_secs(60);
const DAY: u64 = 86400;
const MAX_PARTS: usize = 400;

/// A distributor's lifecycle words as one of active | nrnd | eol | unknown.
pub fn lifecycle_of(s: &str) -> &'static str {
    let l = s.to_ascii_lowercase();
    if l.is_empty() { "unknown" }
    else if l.contains("not recommended") || l.contains("nrnd") { "nrnd" }
    else if l.contains("end of life") || l.contains("obsolete") || l.contains("discontinued") || l.contains("eol") || l.contains("last time buy") { "eol" }
    else if l.contains("active") || l.contains("new product") || l.contains("production") { "active" }
    else { "unknown" }
}
/// The latest lifecycle a part's samples say ("unknown" when none has said one).
pub fn last_life(samples: &[Value]) -> &'static str { samples.iter().rev().find_map(|s| s["lifecycle"].as_str().filter(|l| !l.is_empty())).map(lifecycle_of).unwrap_or("unknown") }

/// The whole BOM over time, a point a day: the parts cost of one board (each line's latest price that day times its quantity, lines with
/// no price yet left out) and how many lines were in stock for `boards` boards. `lines`: (key, qty) of the BOM's lookable lines.
pub fn bom_series(lines: &[(String, u64)], hist: &HashMap<String, Vec<Value>>, boards: u64) -> (Vec<(u64, f64)>, Vec<(u64, u64)>) {
    let mut days: Vec<u64> = hist.values().flatten().filter_map(|s| s["at"].as_u64()).map(|a| a / DAY * DAY).collect();
    days.sort(); days.dedup();
    let (mut cost, mut instock) = (vec![], vec![]);
    for d in days {
        let end = d + DAY;
        let (mut c, mut n, mut any) = (0.0, 0u64, false);
        for (k, q) in lines {
            let before = |f: &str| hist.get(k).and_then(|v| v.iter().rev().find(|s| s["at"].as_u64().map_or(false, |a| a < end) && !s[f].is_null()).cloned());
            // the stock that day (the fab's recorded changes count too), and the last price known by then (only Backplane's own readings have one)
            let Some(s) = before("stock").filter(|s| s["stock"].is_i64()) else { continue };
            any = true;
            if let Some(p) = before("price").and_then(|s| s["price"].as_f64()) { c += p * *q as f64 }
            if s["stock"].as_i64().unwrap_or(-1) >= (*q * boards.max(1)) as i64 { n += 1 }
        }
        if any { cost.push((d, (c * 10000.0).round() / 10000.0)); instock.push((d, n)); }
    }
    (cost, instock)
}

/// The panel's data from the BOM's lines (as published: id, refs, mpn, key, qty) and each key's samples.
pub fn data(lines: &[Value], hist: &HashMap<String, Vec<Value>>, boards: u64) -> Value {
    let mut counts: BTreeMap<&str, usize> = [("active", 0), ("nrnd", 0), ("eol", 0), ("unknown", 0)].into_iter().collect();
    let (mut ps, mut total, mut since) = (vec![], 0usize, u64::MAX);
    for l in lines {
        let Some(k) = l["key"].as_str() else { continue };
        let ss = hist.get(k).cloned().unwrap_or_default();
        let life = last_life(&ss); *counts.get_mut(life).unwrap() += 1;
        total += ss.len();
        if let Some(a) = ss.first().and_then(|s| s["at"].as_u64()) { since = since.min(a) }
        let last = ss.iter().rev().find(|s| s["stock"].is_i64());
        if ps.len() < MAX_PARTS {
            ps.push(json!({"key": k, "id": l["id"], "refs": l["refs"], "mpn": if l["mpn"].as_str().map_or(true, str::is_empty) { last.map_or(json!(""), |s| s["model"].clone()) } else { l["mpn"].clone() },
                "lifecycle": life, "stock": last.map_or(json!(null), |s| s["stock"].clone()), "price": last.map_or(json!(null), |s| s["price"].clone()), "n": ss.len(),
                "first": ss.first().map_or(json!(null), |s| s["at"].clone()), "last": ss.last().map_or(json!(null), |s| s["at"].clone())}));
        }
    }
    let keyed: Vec<(String, u64)> = lines.iter().filter_map(|l| Some((l["key"].as_str()?.to_string(), l["qty"].as_u64().unwrap_or(1)))).collect();
    let (cost, instock) = bom_series(&keyed, hist, boards);
    let bad = counts["eol"]; let n = bad + counts["nrnd"];
    json!({"badge": {"n": n, "tone": if bad > 0 { "bad" } else if n > 0 { "warn" } else { "" }}, "counts": counts, "samples": total, "since": if since == u64::MAX { json!(null) } else { json!(since) },
        "parts": ps, "boards": boards.max(1), "bom": {"cost": cost, "instock": instock, "lines": keyed.len()}})
}

/// Publish the panel for a project from its BOM's published data (bompanel calls it after each BOM it publishes).
pub async fn publish(hub: &Arc<Hub>, pro: &str, bom: &Value, hist: &HashMap<String, Vec<Value>>) {
    let lines: Vec<Value> = bom["lines"].as_array().cloned().unwrap_or_default();
    let boards = bom["boards"].as_u64().unwrap_or(1);
    mgmt::publish(hub, "life", pro, data(&lines, hist, boards)).await;
    { let mut w = wanted().lock().unwrap(); for l in &lines { if let (Some(k), Some(m)) = (l["key"].as_str(), l["mpn"].as_str().filter(|m| !m.is_empty())) { w.insert(k.to_string(), m.to_string()); } } }
}

pub struct LifePanel;
impl mgmt::Panel for LifePanel {
    fn id(&self) -> &'static str { "life" }
    fn label(&self) -> &'static str { "Lifecycle" }
    /// The BOM panel publishes this one with it (it has the lines): a project shown is the BOM's watch.
    fn watch(&self, _hub: &Arc<Hub>, _project: &str) {}
}

// Lifecycle readings ---------------------------------------------------------------------------------------------------

/// Parts (key -> MPN) of the BOMs shown, for lifecycle readings.
fn wanted() -> &'static Mutex<HashMap<String, String>> { static W: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new(); W.get_or_init(Default::default) }
/// The parts due a lifecycle reading: none in the last day, at most 10 (one Mouser call).
pub fn due(home: &Path, wanted: &HashMap<String, String>, now: u64) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = wanted.iter().filter(|(k, _)| { let ss = parthist::read(home, k); !ss.iter().rev().any(|s| s["life_at"].as_u64().map_or(false, |a| now.saturating_sub(a) < DAY)) }).map(|(k, m)| (k.clone(), m.clone())).collect();
    v.sort(); v.truncate(10); v
}
/// A Mouser answer's lifecycle for each asked part: (key, words) for those it matched (words "" when Mouser states none).
pub fn readings(v: &Value, asked: &[(String, String)]) -> Vec<(String, String)> {
    let rs = v["results"].as_array().cloned().unwrap_or_default();
    asked.iter().filter_map(|(k, m)| rs.iter().find(|r| parts::same_mpn(r["mfr_part_number"].as_str().unwrap_or(""), m)).map(|r| (k.clone(), r["lifecycle"].as_str().unwrap_or("").to_string()))).collect()
}

/// `mgmt.life.part {project, key}`: a part's samples.
pub fn part_samples(home: &Path, key: &str) -> Value {
    json!({"key": key, "samples": parthist::read(home, key).iter().map(|s| json!({"at": s["at"], "stock": s["stock"], "price": s["price"], "lifecycle": s["lifecycle"], "class": s["class"]})).collect::<Vec<_>>()})
}

static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);
pub fn start(hub: &Arc<Hub>) {
    *HUB.lock().unwrap() = Some(Arc::downgrade(hub));
    mgmt::register(Arc::new(LifePanel));
    hub.register_rpc(Arc::new(LifeRpc));
    if crate::copy::on() { return }
    tokio::spawn(async move {
        let (mut day, mut calls) = (0u64, 0usize);
        // BACKPLANE_PARTS_LIFE_GAP_S: the tests' shorter wait between lifecycle readings
        let gap = std::env::var("BACKPLANE_PARTS_LIFE_GAP_S").ok().and_then(|s| s.parse().ok()).map(Duration::from_secs).unwrap_or(LIFE_GAP);
        loop {
            tokio::time::sleep(gap).await;
            let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) else { return };
            let now = super::ipc::now_s();
            if now / DAY != day { day = now / DAY; calls = 0; }
            if calls >= LIFE_PER_DAY || hub.setting(crate::mcpx::svc::PCBPARTS_KEY).await.as_deref() == Some("off") { continue }
            let w = wanted().lock().unwrap().clone();
            let home = hub.cfg.home.clone();
            let asked = { let h = home.clone(); tokio::task::spawn_blocking(move || due(&h, &w, now)).await.unwrap_or_default() };
            if asked.is_empty() { continue }
            calls += 1;
            let q = asked.iter().map(|(_, m)| m.as_str()).collect::<Vec<_>>().join("|");
            match crate::mcpx::health::call_tool(&parts::url(), "mouser_get_part", &json!({"part_number": q})).await {
                Ok(v) => {
                    let got = readings(&v, &asked);
                    for (k, _) in &asked {
                        let life = got.iter().find(|(g, _)| g == k).map(|(_, l)| l.clone()).unwrap_or_default();
                        // the reading goes on the part's latest stock sample's values, so the history stays one row a time
                        let last = parthist::read(&home, k).last().cloned().unwrap_or(json!({}));
                        let mut s = last.clone(); s["at"] = json!(now); s["life_at"] = json!(now); s["key"] = json!(k); s["lifecycle"] = json!(life); s["source"] = json!("mouser");
                        parthist::append(&home, k, &s);
                    }
                    parts::bump();
                }
                // a quota or an outage: no more today
                Err(_) => { calls = LIFE_PER_DAY }
            }
        }
    });
}

pub struct LifeRpc;
impl crate::api::RpcHandler for LifeRpc {
    fn methods(&self) -> &'static [&'static str] { &["mgmt.life.part"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a crate::api::Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let key = p["key"].as_str().unwrap_or("");
            if !(key.starts_with("fab:") || key.starts_with("mpn:")) || key.len() > 120 { return vec![json!({"t":"reply","id":id,"ok":false,"error":"key: a part's fab:<number> or mpn:<number>"})] }
            let (home, k) = (hub.cfg.home.clone(), key.to_string());
            let v = tokio::task::spawn_blocking(move || part_samples(&home, &k)).await.unwrap_or(json!({}));
            vec![json!({"t":"reply","id":id,"ok":true,"result":v})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn lifecycle_words() {
        for (w, l) in [("Not Recommended for New Designs", "nrnd"), ("End of Life", "eol"), ("Obsolete", "eol"), ("Discontinued at Digi-Key", "eol"), ("Active", "active"), ("New Product", "active"), ("", "unknown"), ("Something", "unknown")] { assert_eq!(lifecycle_of(w), l, "{w}") }
        assert_eq!(last_life(&[json!({"lifecycle": "Active"}), json!({"lifecycle": ""}), json!({})]), "active"); assert_eq!(last_life(&[]), "unknown");
    }
    #[test] fn the_bom_over_time_a_point_a_day() {
        let s = |at: u64, stock: i64, price: f64| json!({"at": at, "stock": stock, "price": price});
        let mut h = HashMap::new();
        h.insert("mpn:A".to_string(), vec![s(DAY * 10 + 5, 100, 0.10), s(DAY * 11 + 5, 1, 0.12)]);
        h.insert("mpn:B".to_string(), vec![s(DAY * 11 + 9, 50, 1.0)]);
        h.get_mut("mpn:A").unwrap().push(json!({"at": DAY * 12 + 1, "stock": 30, "source": "fab_history"}));
        let (cost, instock) = bom_series(&[("mpn:A".into(), 2), ("mpn:B".into(), 1)], &h, 1);
        assert_eq!((cost[2], instock[2]), ((DAY * 12, 1.24), (DAY * 12, 2)), "a fab history day keeps the last price and counts its stock");
        let (cost, instock) = (cost[..2].to_vec(), instock[..2].to_vec());
        assert_eq!(cost, vec![(DAY * 10, 0.2), (DAY * 11, 1.24)]);
        assert_eq!(instock, vec![(DAY * 10, 1), (DAY * 11, 1)], "A dropped below the 2 a board needs on day 11; B arrived");
    }
    #[test] fn the_panels_data() {
        let mut h = HashMap::new();
        h.insert("mpn:A".to_string(), vec![json!({"at": 100, "stock": 5, "price": 0.1, "model": "A"}), json!({"at": 200, "stock": 5, "price": 0.1, "lifecycle": "Not Recommended for New Designs", "life_at": 200})]);
        h.insert("mpn:B".to_string(), vec![json!({"at": 150, "stock": 9, "price": 2.0, "lifecycle": "Obsolete"})]);
        let lines = vec![json!({"id": "R1", "refs": ["R1"], "mpn": "A", "key": "mpn:A", "qty": 1}), json!({"id": "U1", "refs": ["U1"], "mpn": "B", "key": "mpn:B", "qty": 1}), json!({"id": "C1", "refs": ["C1"], "mpn": "", "qty": 1}), json!({"id": "C2", "refs": ["C2"], "mpn": "Z", "key": "mpn:Z", "qty": 1})];
        let d = data(&lines, &h, 1);
        assert_eq!(d["counts"], json!({"active": 0, "eol": 1, "nrnd": 1, "unknown": 1}));
        assert_eq!((d["badge"]["n"].as_u64(), d["badge"]["tone"].as_str(), d["samples"].as_u64(), d["since"].as_u64()), (Some(2), Some("bad"), Some(3), Some(100)));
        assert_eq!(d["parts"].as_array().unwrap().len(), 3); assert_eq!(d["parts"][0]["lifecycle"], "nrnd"); assert_eq!(d["parts"][2]["n"], 0);
    }
    #[test] fn mouser_readings_and_whats_due() {
        let asked = vec![("mpn:A".to_string(), "AP2553W6-7".to_string()), ("mpn:B".to_string(), "X1".to_string())];
        let v = json!({"results": [{"mfr_part_number": "AP2553W6-7", "lifecycle": "Not Recommended for New Designs"}, {"mfr_part_number": "other", "lifecycle": null}]});
        assert_eq!(readings(&v, &asked), vec![("mpn:A".to_string(), "Not Recommended for New Designs".to_string())]);
        let d = crate::plat::tmp::dir("lifedue"); let w: HashMap<String, String> = asked.iter().cloned().collect();
        assert_eq!(due(&d, &w, 1000).len(), 2);
        parthist::append(&d, "mpn:A", &json!({"at": 900, "key": "mpn:A", "life_at": 900}));
        assert_eq!(due(&d, &w, 1000), vec![("mpn:B".to_string(), "X1".to_string())], "read today");
        assert_eq!(due(&d, &w, 900 + DAY).len(), 2, "again a day later");
    }
}
