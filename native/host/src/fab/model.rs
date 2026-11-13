//! A fab's capabilities as data: every value with its unit, the page or API it was read from, the day it was read and how sure we are
//! of it (published: read off the fab's page as it is; derived: computed from what the page says, a mil turned into mm, a ring from a
//! diameter; inherited: the rigid board's value used for a kind whose own page says nothing else). Values differ per board kind and per
//! option tier (`When`: layer count, copper weight, thickness, finish, mask colour), and per cost: `free` is what the fab makes at its
//! base price, `extra` what it can make for more. Pure: storage is store.rs, gathering gather.rs. Laws `fab_model_*`.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const SCHEMA: u32 = 1;

fn is_default<T: Default + PartialEq>(v: &T) -> bool { *v == T::default() }
fn free() -> String { "free".into() }
fn published() -> String { "published".into() }
fn is_free(s: &String) -> bool { s == "free" }
fn is_published(s: &String) -> bool { s == "published" }

/// One page (or API) a profile is read from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Page {
    pub url: String,
    /// the day its values were last read (YYYY-MM-DD, UTC)
    pub fetched: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub etag: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub sha256: String,
    /// a table the page publishes only as an image: the image's address its values were read from (a new address is a new table)
    #[serde(default, skip_serializing_if = "String::is_empty")] pub image: String,
}

/// The options a value holds for. Ranges are inclusive; a missing field holds for any.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct When {
    #[serde(default, skip_serializing_if = "Option::is_none")] pub layers: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub copper_oz: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub thickness_mm: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub finish: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub color: Option<Vec<String>>,
}

/// The options a board is made with, as a fab.json says them (catalog.py's words), defaults filled in.
#[derive(Clone, Debug, PartialEq)]
pub struct Opts { pub layers: f64, pub copper_oz: f64, pub thickness_mm: f64, pub finish: String, pub color: String }
impl Default for Opts { fn default() -> Self { Opts { layers: 2.0, copper_oz: 1.0, thickness_mm: 1.6, finish: "hasl".into(), color: "green".into() } } }
impl Opts {
    /// From a fab.json's `options` (anything missing or of the wrong type keeps its default).
    pub fn from(o: &Value) -> Opts {
        let d = Opts::default();
        let n = |k: &str, d: f64| o.get(k).and_then(Value::as_f64).unwrap_or(d);
        let s = |k: &str, d: &str| o.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
        Opts { layers: n("layers", d.layers), copper_oz: n("copper_oz", d.copper_oz), thickness_mm: n("thickness_mm", d.thickness_mm), finish: s("finish", &d.finish), color: s("color", &d.color) }
    }
    pub fn json(&self) -> Value { json!({"layers": self.layers, "copper_oz": self.copper_oz, "thickness_mm": self.thickness_mm, "finish": self.finish, "color": self.color}) }
}

impl When {
    pub fn matches(&self, o: &Opts) -> bool {
        let r = |r: &Option<[f64; 2]>, v: f64| r.map_or(true, |[a, b]| v >= a - 1e-9 && v <= b + 1e-9);
        let s = |l: &Option<Vec<String>>, v: &str| l.as_ref().map_or(true, |l| l.iter().any(|x| x == v));
        r(&self.layers, o.layers) && r(&self.copper_oz, o.copper_oz) && r(&self.thickness_mm, o.thickness_mm) && s(&self.finish, &o.finish) && s(&self.color, &o.color)
    }
    /// How many conditions it has: the most specific matching value is the one that holds.
    pub fn specificity(&self) -> usize {
        self.layers.is_some() as usize + self.copper_oz.is_some() as usize + self.thickness_mm.is_some() as usize + self.finish.is_some() as usize + self.color.is_some() as usize
    }
    /// A stable text of the conditions (the identity of a value with its key and cost, for diffs).
    pub fn sig(&self) -> String { serde_json::to_string(self).unwrap_or_default() }
}

/// One value of a capability.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Limit {
    pub key: String,
    pub value: Value,
    #[serde(default)] pub unit: String,
    #[serde(default = "free", skip_serializing_if = "is_free")] pub cost: String,
    #[serde(default, skip_serializing_if = "is_default")] pub when: When,
    /// the page it was read from (a key of Profile.pages)
    #[serde(default)] pub page: String,
    /// filled from the page by `resolve` when not given
    #[serde(default)] pub source: String,
    #[serde(default)] pub fetched: String,
    #[serde(default = "published", skip_serializing_if = "is_published")] pub confidence: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub note: String,
    /// what the fab recommends where it differs from the minimum
    #[serde(default, skip_serializing_if = "Option::is_none")] pub preferred: Option<f64>,
}
impl Limit {
    pub fn num(&self) -> Option<f64> { self.value.as_f64() }
    pub fn id(&self) -> String { format!("{}|{}|{}", self.key, self.cost, self.when.sig()) }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Section {
    /// a kind whose page lists only what differs: the rest is this kind's (marked inherited)
    #[serde(default, skip_serializing_if = "Option::is_none")] pub inherits: Option<String>,
    pub limits: Vec<Limit>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Ply {
    #[serde(rename = "type")] pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub material: String,
    pub thickness_mm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub er: Option<f64>,
}

/// A published stackup (the fab builds it as listed; impedance is calculated on it).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Stackup {
    pub id: String,
    pub layers: u32,
    pub thickness_mm: f64,
    #[serde(default)] pub outer_oz: f64,
    #[serde(default)] pub inner_oz: f64,
    #[serde(default, skip_serializing_if = "is_default")] pub default: bool,
    #[serde(default)] pub page: String,
    #[serde(default)] pub source: String,
    #[serde(default)] pub fetched: String,
    pub seq: Vec<Ply>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Profile {
    pub schema: u32,
    /// the fab's own identifier (data only; UI copy says "the fab")
    pub fab: String,
    /// the fab Backplane's order server orders from
    #[serde(default)] pub orders: bool,
    /// goes up by one with each stored change
    pub rev: u64,
    pub updated: String,
    pub pages: BTreeMap<String, Page>,
    pub kinds: BTreeMap<String, Section>,
    #[serde(default)] pub stackups: Vec<Stackup>,
    /// assembly services by name (economic, standard)
    #[serde(default)] pub assembly: BTreeMap<String, Section>,
}

pub const KINDS: [&str; 7] = ["pcb", "flex", "hdi", "aluminum", "copper_core", "rogers", "ptfe"];

/// What a key means, for people (the panel) and agents (the tool).
pub fn label(key: &str) -> &'static str {
    match key {
        "layers_max" => "most copper layers", "layers" => "layer counts", "track_min" => "track width", "space_min" => "copper to copper spacing",
        "drill_min" => "smallest drilled hole", "drill_max" => "largest drilled hole", "via_hole_min" => "via hole", "via_diameter_min" => "via diameter",
        "via_ring_min" => "via ring", "pth_ring_min" => "plated hole ring", "pth_hole_min" => "plated part hole", "npth_min" => "non-plated hole",
        "plated_slot_min" => "plated slot width", "npth_slot_min" => "non-plated slot width", "via_hole_to_hole_min" => "via hole to hole",
        "pad_hole_to_hole_min" => "pad hole to hole", "castellated_hole_min" => "castellated hole", "via_to_track_min" => "via hole to track",
        "pth_to_track_min" => "plated hole to track", "npth_to_track_min" => "non-plated hole to track", "pad_to_track_min" => "pad to track",
        "smd_pad_to_pad_min" => "pad to pad (different nets)", "copper_to_edge_min" => "copper to board edge", "copper_to_vcut_min" => "copper to V-cut",
        "silk_line_min" => "silkscreen line", "silk_text_height_min" => "silkscreen text height", "pad_to_silk_min" => "pad to silkscreen",
        "mask_bridge_min" => "solder mask bridge (pad gap)", "mask_expansion" => "solder mask expansion", "board_min" => "smallest board side",
        "board_max_x" => "largest board, long side", "board_max_y" => "largest board, short side", "thicknesses" => "board thicknesses",
        "finishes" => "surface finishes", "hole_tolerance_plus" => "hole tolerance (+)", "hole_tolerance_minus" => "hole tolerance (-)",
        "impedance_tolerance" => "impedance tolerance", "castellated" => "castellated holes", "via_types" => "via types",
        "microvia_hole_min" => "microvia hole", "microvia_hole_max" => "largest microvia hole", "buried_via_hole_min" => "buried via hole",
        "sides" => "sides placed", "thickness_min" => "thinnest board", "thickness_max" => "thickest board", "qty_min" => "fewest boards", "qty_max" => "most boards",
        "min_package" => "smallest package", "ic_pitch_min" => "IC pin pitch", "bga_pitch_min" => "BGA pitch", "reflow_c" => "reflow peak",
        "rails_needed" => "edge rails needed", "pth_to_inner_copper_min" => "plated hole to inner copper", "npth_pad_ring_min" => "non-plated hole ring", "fiducials_needed" => "fiducials needed",
        _ => "",
    }
}

impl Profile {
    pub fn parse(text: &str) -> Result<Profile, String> {
        let p: Profile = serde_json::from_str(text).map_err(|e| format!("not a fab profile: {e}"))?;
        if p.schema != SCHEMA { return Err(format!("fab profile schema {} (this Backplane reads {SCHEMA})", p.schema)) }
        let mut p = p; p.resolve(); Ok(p)
    }
    /// Each value carries its source and date: those not given are their page's.
    pub fn resolve(&mut self) {
        let pages = self.pages.clone();
        let fill = |src: &mut String, at: &mut String, page: &str| if let Some(pg) = pages.get(page) {
            if src.is_empty() { *src = pg.url.clone() } if at.is_empty() { *at = pg.fetched.clone() }
        };
        for s in self.kinds.values_mut().chain(self.assembly.values_mut()) { for l in s.limits.iter_mut() { fill(&mut l.source, &mut l.fetched, &l.page.clone()) } }
        for s in self.stackups.iter_mut() { fill(&mut s.source, &mut s.fetched, &s.page.clone()) }
    }
    /// Every value of the profile with where it sits ("kind pcb", "assembly economic", "stackup <id>").
    pub fn all(&self) -> Vec<(String, &Limit)> {
        let mut v: Vec<(String, &Limit)> = vec![];
        for (k, s) in &self.kinds { for l in &s.limits { v.push((format!("kind {k}"), l)) } }
        for (k, s) in &self.assembly { for l in &s.limits { v.push((format!("assembly {k}"), l)) } }
        v
    }
}

/// The values that hold for one board: per key the free one and the one that costs more (either may be missing).
#[derive(Clone, Debug, PartialEq)]
pub struct Sel { pub key: String, pub free: Option<Limit>, pub extra: Option<Limit>, pub inherited: bool }

fn pick<'a>(ls: &[&'a Limit], o: &Opts, cost: &str) -> Option<&'a Limit> {
    let mut best: Option<&Limit> = None;
    for l in ls.iter().copied().filter(|l| l.cost == cost && l.when.matches(o)) {
        if best.map_or(true, |b| l.when.specificity() > b.when.specificity()) { best = Some(l) }
    }
    best
}
fn select_in(s: &Section, o: &Opts) -> Vec<Sel> {
    let mut keys: Vec<&str> = vec![];
    for l in &s.limits { if !keys.contains(&l.key.as_str()) { keys.push(&l.key) } }
    keys.into_iter().filter_map(|k| {
        let ls: Vec<&Limit> = s.limits.iter().filter(|l| l.key == k).collect();
        let (f, e) = (pick(&ls, o, "free").cloned(), pick(&ls, o, "extra").cloned());
        if f.is_none() && e.is_none() { None } else { Some(Sel { key: k.into(), free: f, extra: e, inherited: false }) }
    }).collect()
}

/// Fab.select: what holds for a board of this kind made with these options. A kind that inherits takes the other kind's value for
/// every key it does not list itself, marked inherited. An unknown kind holds nothing.
pub fn select(p: &Profile, kind: &str, o: &Opts) -> Vec<Sel> {
    let Some(s) = p.kinds.get(kind) else { return vec![] };
    let mut out = select_in(s, o);
    if let Some(base) = s.inherits.as_deref().and_then(|b| p.kinds.get(b)) {
        let own: Vec<String> = s.limits.iter().map(|l| l.key.clone()).collect();
        out.extend(select_in(base, o).into_iter().filter(|x| !own.contains(&x.key)).map(|mut x| { x.inherited = true; x }));
    }
    out
}
pub fn select_assembly(p: &Profile, service: &str, o: &Opts) -> Vec<Sel> { p.assembly.get(service).map(|s| select_in(s, o)).unwrap_or_default() }
/// One selected value by key (the free one, else the extra one).
pub fn get<'a>(sel: &'a [Sel], key: &str) -> Option<&'a Limit> { sel.iter().find(|s| s.key == key).and_then(|s| s.free.as_ref().or(s.extra.as_ref())) }
pub fn free_num(sel: &[Sel], key: &str) -> Option<f64> { sel.iter().find(|s| s.key == key).and_then(|s| s.free.as_ref()).and_then(Limit::num) }

/// The stackups that fit a board (its layer count and thickness).
pub fn stackups<'a>(p: &'a Profile, o: &Opts) -> Vec<&'a Stackup> {
    p.stackups.iter().filter(|s| (s.layers as f64 - o.layers).abs() < 0.5 && (s.thickness_mm - o.thickness_mm).abs() < 0.05).collect()
}

/// One difference between two profiles.
#[derive(Clone, Debug, PartialEq)]
pub struct Change { pub at: String, pub key: String, pub cost: String, pub when: When, pub from: Option<Value>, pub to: Option<Value> }
impl Change {
    pub fn json(&self) -> Value { json!({"at": self.at, "key": self.key, "cost": self.cost, "when": self.when, "from": self.from, "to": self.to}) }
    pub fn line(&self) -> String {
        let s = |v: &Option<Value>| v.as_ref().map_or("none".into(), |v| v.to_string());
        let w = if self.when == When::default() { String::new() } else { format!(" {}", self.when.sig()) };
        format!("{} {} ({}{}): {} -> {}", self.at, self.key, self.cost, w, s(&self.from), s(&self.to))
    }
}

/// Fab.diff: values added, removed or changed, a value being its key, cost and conditions where it sits; stackups by id and their plies.
/// Dates and sources alone are no change.
pub fn diff(old: &Profile, new: &Profile) -> Vec<Change> {
    let index = |p: &Profile| -> BTreeMap<(String, String), Limit> { p.all().into_iter().map(|(at, l)| ((at, l.id()), l.clone())).collect() };
    let (a, b) = (index(old), index(new));
    let mut out = vec![];
    for (k, l) in &a {
        match b.get(k) {
            None => out.push(Change { at: k.0.clone(), key: l.key.clone(), cost: l.cost.clone(), when: l.when.clone(), from: Some(l.value.clone()), to: None }),
            Some(n) if !same(&n.value, &l.value) => out.push(Change { at: k.0.clone(), key: l.key.clone(), cost: l.cost.clone(), when: l.when.clone(), from: Some(l.value.clone()), to: Some(n.value.clone()) }),
            _ => {}
        }
    }
    for (k, n) in &b { if !a.contains_key(k) { out.push(Change { at: k.0.clone(), key: n.key.clone(), cost: n.cost.clone(), when: n.when.clone(), from: None, to: Some(n.value.clone()) }) } }
    let st = |p: &Profile| -> BTreeMap<String, Value> { p.stackups.iter().map(|s| (s.id.clone(), json!({"layers": s.layers, "thickness_mm": s.thickness_mm, "seq": s.seq}))).collect() };
    let (sa, sb) = (st(old), st(new));
    for (id, v) in &sa { match sb.get(id) { None => out.push(stack_change(id, Some(v), None)), Some(n) if !same(n, v) => out.push(stack_change(id, Some(v), Some(n))), _ => {} } }
    for (id, v) in &sb { if !sa.contains_key(id) { out.push(stack_change(id, None, Some(v))) } }
    out
}
fn stack_change(id: &str, a: Option<&Value>, b: Option<&Value>) -> Change { Change { at: format!("stackup {id}"), key: "seq".into(), cost: "free".into(), when: When::default(), from: a.cloned(), to: b.cloned() } }
/// Numbers compare by value (0.1 and 0.10 are the same), everything else by its JSON.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) { (Some(x), Some(y)) => (x - y).abs() < 1e-12, _ => x == y },
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| same(a, b)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map_or(false, |w| same(v, w))),
        _ => a == b,
    }
}

/// A value as agents and clients get it.
pub fn limit_json(l: &Limit) -> Value {
    let mut v = json!({"value": l.value, "unit": l.unit, "source": l.source, "fetched": l.fetched, "confidence": l.confidence});
    if !l.note.is_empty() { v["note"] = json!(l.note) }
    if let Some(p) = l.preferred { v["preferred"] = json!(p) }
    v
}
pub fn sel_json(s: &Sel, changed: &[String]) -> Value {
    let mut v = json!({"key": s.key, "label": label(&s.key)});
    if let Some(f) = &s.free { v["free"] = limit_json(f) }
    if let Some(e) = &s.extra { v["extra"] = limit_json(e) }
    if s.inherited { v["inherited"] = json!(true) }
    if changed.contains(&s.key) { v["changed"] = json!(true) }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn baseline() -> Profile { Profile::parse(super::super::store::BASELINE[0].1).unwrap() }
    fn o(layers: f64, oz: f64) -> Opts { Opts { layers, copper_oz: oz, ..Opts::default() } }
    fn free(sel: &[Sel], k: &str) -> f64 { free_num(sel, k).unwrap_or(f64::NAN) }
    fn extra(sel: &[Sel], k: &str) -> f64 { sel.iter().find(|s| s.key == k).and_then(|s| s.extra.as_ref()).and_then(Limit::num).unwrap_or(f64::NAN) }

    #[test] fn fab_model_every_baseline_value_has_a_source_a_date_and_a_label() {
        for (fab, text) in super::super::store::BASELINE { let p = Profile::parse(text).unwrap(); assert_eq!(&p.fab, fab); check(&p) }
        for k in KINDS { assert!(baseline().kinds.contains_key(k), "{k}") }
    }
    fn check(p: &Profile) {
        assert_eq!(p.schema, SCHEMA);
        for (at, l) in p.all() {
            assert!(l.source.starts_with("https://"), "{at} {} has no source", l.key);
            assert_eq!(l.fetched.len(), 10, "{at} {} has no date", l.key);
            assert!(["published", "derived", "inherited"].contains(&l.confidence.as_str()), "{at} {}: {}", l.key, l.confidence);
            assert!(["free", "extra"].contains(&l.cost.as_str()), "{at} {}", l.key);
            assert!(!label(&l.key).is_empty(), "{} has no label", l.key);
            assert!(!l.note.is_empty(), "{at} {} says nothing of where on the page it is", l.key);
        }
        for s in &p.stackups { assert!(s.source.starts_with("https://") && s.fetched.len() == 10, "{}", s.id); let t: f64 = s.seq.iter().map(|p| p.thickness_mm).sum(); assert!((t - s.thickness_mm).abs() <= 0.2, "{} adds up to {t}", s.id) }
    }
    /// How old a bundled value may be before this test fails: a hub refreshes its own copy daily, but a new install, a copy hub and
    /// an offline one design against the bundled baseline until then. Read the pages again (fab_gather_live_refresh) and bring the
    /// baseline up to date before it runs out.
    const BASELINE_MAX_AGE_DAYS: i64 = 120;
    fn day(s: &str) -> i64 { let n = |a: usize, k: usize| s.get(a..a + k).and_then(|x| x.parse().ok()).unwrap_or(0); crate::google::cal::days(n(0, 4), n(5, 2), n(8, 2)) }
    #[test] fn fab_model_no_baseline_value_is_older_than_its_bound() {
        let today = day(&super::super::gather::today());
        let mut old = vec![];
        for (fab, text) in super::super::store::BASELINE {
            let p = Profile::parse(text).unwrap();
            let dates = p.all().into_iter().map(|(at, l)| (format!("{at} {}", l.key), l.fetched.clone())).chain(p.stackups.iter().map(|s| (format!("stackup {}", s.id), s.fetched.clone())));
            for (what, d) in dates { let age = today - day(&d); if age > BASELINE_MAX_AGE_DAYS { old.push(format!("{fab} {what}: read {d}, {age} days ago")) } }
        }
        assert!(old.is_empty(), "{} bundled values are older than {BASELINE_MAX_AGE_DAYS} days; read the fab's pages again (FAB_LIVE_HOME=<temp home> cargo test fab_gather_live_refresh -- --ignored) and update fab/baseline/:\n{}", old.len(), old.join("\n"));
    }
    #[test] fn fab_model_the_age_bound_sees_an_old_date() {
        assert_eq!(day("2026-11-12") - day("2026-07-15"), 120); assert_eq!(day("2027-03-12") - day("2026-11-12"), 120);
        assert!(day("2026-11-12") - day("2026-07-14") > BASELINE_MAX_AGE_DAYS);
    }
    #[test] fn fab_model_the_tier_follows_layers_copper_and_cost() {
        let p = baseline();
        let two = select(&p, "pcb", &o(2.0, 1.0)); let four = select(&p, "pcb", &o(4.0, 1.0));
        assert_eq!((free(&two, "track_min"), free(&four, "track_min")), (0.10, 0.09));
        assert!(extra(&two, "track_min").is_nan(), "no cheaper-limit-for-more on 2 layers"); assert_eq!(extra(&four, "track_min"), 0.075);
        assert_eq!((free(&two, "via_hole_min"), extra(&two, "via_hole_min"), free(&two, "via_diameter_min"), extra(&two, "via_diameter_min")), (0.2, 0.15, 0.45, 0.25));
        assert_eq!(free(&select(&p, "pcb", &o(2.0, 2.0)), "track_min"), 0.16); assert_eq!(free(&select(&p, "pcb", &o(6.0, 2.0)), "space_min"), 0.15);
        assert_eq!((free(&two, "pth_ring_min"), free(&four, "pth_ring_min"), free(&select(&p, "pcb", &o(2.0, 2.0)), "pth_ring_min")), (0.18, 0.15, 0.254));
        assert_eq!(free(&select(&p, "pcb", &o(1.0, 1.0)), "drill_min"), 0.3);
        // the most specific value wins: a thin ENIG board gets the 0.1 mm via for more
        let thin = Opts { thickness_mm: 0.8, finish: "enig".into(), ..o(2.0, 1.0) };
        assert_eq!(extra(&select(&p, "pcb", &thin), "via_hole_min"), 0.1); assert_eq!(extra(&two, "drill_min"), 0.15);
        let black = Opts { color: "black".into(), ..o(2.0, 1.0) }; assert_eq!((free(&two, "mask_bridge_min"), free(&select(&p, "pcb", &black), "mask_bridge_min")), (0.1, 0.13));
        assert_eq!(get(&select(&p, "pcb", &o(6.0, 1.0)), "finishes").unwrap().value, json!(["enig", "osp"]));
    }
    #[test] fn fab_model_a_kind_that_inherits_says_so_and_keeps_its_own() {
        let p = baseline(); let al = select(&p, "aluminum", &o(1.0, 1.0));
        let d = al.iter().find(|s| s.key == "drill_min").unwrap(); assert!(!d.inherited); assert_eq!(free(&al, "drill_min"), 0.65);
        let t = al.iter().find(|s| s.key == "track_min").unwrap(); assert!(t.inherited, "the rigid page's track limits, marked"); assert_eq!(free(&al, "track_min"), 0.10);
        assert!(select(&p, "flex", &o(2.0, 1.0)).iter().all(|s| !s.inherited), "flex has its own page and inherits nothing");
        assert!(select(&p, "nonsense", &o(2.0, 1.0)).is_empty());
    }
    #[test] fn fab_model_opts_come_from_a_fab_json() {
        assert_eq!(Opts::from(&json!({"layers": 4, "copper_oz": 2, "finish": "enig"})), Opts { layers: 4.0, copper_oz: 2.0, finish: "enig".into(), ..Opts::default() });
        assert_eq!(Opts::from(&json!({"layers": "four"})), Opts::default()); assert_eq!(Opts::from(&Value::Null), Opts::default());
    }
    #[test] fn fab_model_stackups_fit_the_board() {
        let p = baseline();
        let s = stackups(&p, &Opts { layers: 4.0, ..Opts::default() }); assert!(s.iter().any(|s| s.id == "JLC04161H-7628" && s.default));
        assert!(stackups(&p, &Opts::default()).is_empty(), "none published for 2 layers");
    }
    #[test] fn fab_model_diff_sees_values_not_dates() {
        let a = baseline(); let mut b = a.clone();
        for s in b.kinds.values_mut() { for l in s.limits.iter_mut() { l.fetched = "2027-01-01".into() } }
        b.pages.get_mut("rigid").unwrap().fetched = "2027-01-01".into();
        assert!(diff(&a, &b).is_empty(), "a new date alone is no change");
        let l = b.kinds.get_mut("pcb").unwrap().limits.iter_mut().find(|l| l.key == "copper_to_edge_min").unwrap(); l.value = json!(0.3);
        b.kinds.get_mut("pcb").unwrap().limits.push(Limit { key: "npth_min".into(), value: json!(0.4), unit: "mm".into(), cost: "extra".into(), when: When::default(), page: "rigid".into(), source: String::new(), fetched: String::new(), confidence: "published".into(), note: "n".into(), preferred: None });
        b.stackups.retain(|s| s.id != "JLC04161H-1080");
        let d = diff(&a, &b);
        assert_eq!(d.iter().map(Change::line).collect::<Vec<_>>(), [
            "kind pcb copper_to_edge_min (free): 0.2 -> 0.3", "kind pcb npth_min (extra): none -> 0.4",
            &format!("stackup JLC04161H-1080 seq (free): {} -> none", json!({"layers":4,"thickness_mm":1.6,"seq":a.stackups[2].seq}))]);
        let mut c = a.clone(); c.kinds.get_mut("pcb").unwrap().limits[0].value = serde_json::from_str("32.0").unwrap(); assert!(diff(&a, &c).is_empty(), "32 and 32.0 are one value");
    }
    #[test] fn fab_model_a_profile_of_another_schema_is_refused() {
        assert!(Profile::parse(&super::super::store::BASELINE[0].1.replacen("\"schema\": 1", "\"schema\": 2", 1)).unwrap_err().contains("schema 2"));
        assert!(Profile::parse("{}").is_err());
    }
}
