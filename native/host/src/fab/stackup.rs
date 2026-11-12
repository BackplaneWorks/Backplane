//! The board's stackup from the fab's published ones: which fit (layer count and thickness), which the project should use (a proposal
//! the person approves: picked with the project as `options.stackup`), and per copper layer what the floorplanner and the router need:
//! the copper layers next to it with the dielectric between (height, Er) and the widths that give the project's impedances.
//! Impedances are closed-form estimates (microstrip: Hammerstad and Jensen with the copper's thickness; stripline: IPC-2141; edge-
//! coupled pairs: the usual coupling factors), without solder mask, so a few percent off the fab's own calculator, which stays the
//! reference for an order with impedance control. Pure. Laws `fab_stackup_*`.
use super::model::{Opts, Profile, Stackup};
use serde_json::{json, Value};

/// KiCad's names of a board's copper layers, top to bottom.
pub fn layer_names(n: usize) -> Vec<String> {
    (0..n).map(|i| if i == 0 { "F.Cu".to_string() } else if i + 1 == n { "B.Cu".to_string() } else { format!("In{i}.Cu") }).collect()
}
/// The dielectric between two plies of the sequence (exclusive): its height and the Er of the stack (as capacitors in series).
fn between(s: &Stackup, a: usize, b: usize) -> (f64, f64) {
    let ps = &s.seq[a + 1..b];
    let h: f64 = ps.iter().map(|p| p.thickness_mm).sum();
    let inv: f64 = ps.iter().map(|p| p.thickness_mm / p.er.unwrap_or(f64::NAN)).sum();
    (h, if inv > 0.0 { h / inv } else { f64::NAN })
}

pub fn microstrip(w: f64, h: f64, t: f64, er: f64) -> f64 {
    let we = w + (t / std::f64::consts::PI) * (1.0 + (2.0 * h / t).ln()); let u = we / h;
    let mut ee = (er + 1.0) / 2.0 + (er - 1.0) / 2.0 / (1.0 + 12.0 / u).sqrt();
    if u < 1.0 { ee += 0.04 * (1.0 - u).powi(2) * (er - 1.0) / 2.0 }
    if u <= 1.0 { 60.0 / ee.sqrt() * (8.0 / u + u / 4.0).ln() } else { 120.0 * std::f64::consts::PI / (ee.sqrt() * (u + 1.393 + 0.667 * (u + 1.444).ln())) }
}
pub fn stripline(w: f64, b: f64, t: f64, er: f64) -> f64 { 60.0 / er.sqrt() * (4.0 * b / (0.67 * std::f64::consts::PI * (0.8 * w + t))).ln() }
/// The width (mm) that gives `z` ohms, by bisection (impedance falls as the width grows); None outside 0.02..5 mm.
pub fn solve(f: impl Fn(f64) -> f64, z: f64) -> Option<f64> {
    let (mut lo, mut hi) = (0.02, 5.0);
    if !(f(lo) > z && f(hi) < z) { return None }
    for _ in 0..60 { let m = (lo + hi) / 2.0; if f(m) > z { lo = m } else { hi = m } }
    Some(((lo + hi) / 2.0 * 1000.0).round() / 1000.0)
}

/// What the project needs: single-ended impedances and differential ones, and the pair's gap.
#[derive(Clone, Debug, PartialEq)]
pub struct Needs { pub single: Vec<f64>, pub diff: Vec<f64>, pub gap_mm: f64 }
impl Needs {
    /// From the project's options: `impedance` (single-ended ohms), `diff_impedance`, `diff_gap_mm`; with none given, 50 and 90/100
    /// (USB, Ethernet) are shown without counting as needs.
    pub fn from(o: &Value, space_min: f64) -> (Needs, bool) {
        let list = |k: &str| o[k].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>()).or_else(|| o[k].as_f64().map(|v| vec![v]));
        let (s, d) = (list("impedance"), list("diff_impedance"));
        let given = s.is_some() || d.is_some();
        (Needs { single: s.unwrap_or(vec![50.0]), diff: d.unwrap_or(vec![90.0, 100.0]), gap_mm: o["diff_gap_mm"].as_f64().unwrap_or(space_min.max(0.127)) }, given)
    }
}

/// Per copper layer: its name and copper, the copper layers above and below with the dielectric between, and the widths for the needs
/// (a key per impedance: "50" single-ended, "90d" a pair at the gap). An outer layer is a microstrip over its neighbour, an inner one
/// a stripline between its two.
pub fn layers(s: &Stackup, n: &Needs) -> Vec<Value> {
    let cu: Vec<usize> = s.seq.iter().enumerate().filter(|(_, p)| p.kind == "copper").map(|(i, _)| i).collect();
    let names = layer_names(cu.len());
    let side = |a: usize, b: usize, name: &str| { let (h, er) = between(s, a, b); json!({"layer": name, "h_mm": (h * 10000.0).round() / 10000.0, "er": (er * 100.0).round() / 100.0}) };
    cu.iter().enumerate().map(|(k, &i)| {
        let t = s.seq[i].thickness_mm;
        let up = (k > 0).then(|| side(cu[k - 1], i, &names[k - 1]));
        let down = (k + 1 < cu.len()).then(|| side(i, cu[k + 1], &names[k + 1]));
        let mut w = serde_json::Map::new();
        let outer = k == 0 || k + 1 == cu.len();
        let (h1, er1) = up.as_ref().or(down.as_ref()).map(|v| (v["h_mm"].as_f64().unwrap_or(0.0), v["er"].as_f64().unwrap_or(f64::NAN))).unwrap_or((0.0, f64::NAN));
        if outer && h1 > 0.0 && er1.is_finite() {
            for z in &n.single { if let Some(x) = solve(|w| microstrip(w, h1, t, er1), *z) { w.insert(format!("{z}"), json!(x)); } }
            for z in &n.diff { if let Some(x) = solve(|w| 2.0 * microstrip(w, h1, t, er1) * (1.0 - 0.48 * (-0.96 * n.gap_mm / h1).exp()), *z) { w.insert(format!("{z}d"), json!(x)); } }
        } else if let (Some(a), Some(b)) = (&up, &down) {
            let (ha, hb) = (a["h_mm"].as_f64().unwrap_or(0.0), b["h_mm"].as_f64().unwrap_or(0.0)); let bb = ha + hb + t;
            let er = (ha + hb) / (ha / a["er"].as_f64().unwrap_or(f64::NAN) + hb / b["er"].as_f64().unwrap_or(f64::NAN));
            if er.is_finite() {
                for z in &n.single { if let Some(x) = solve(|w| stripline(w, bb, t, er), *z) { w.insert(format!("{z}"), json!(x)); } }
                for z in &n.diff { if let Some(x) = solve(|w| 2.0 * stripline(w, bb, t, er) * (1.0 - 0.347 * (-2.9 * n.gap_mm / bb).exp()), *z) { w.insert(format!("{z}d"), json!(x)); } }
            }
        }
        json!({"layer": names[k], "copper_mm": t, "above": up, "below": down, "kind": if outer { "microstrip" } else { "stripline" }, "widths": w})
    }).collect()
}

/// Fab.stackup.propose: the stackups that fit the board, best first, each with why. With impedance needs, one whose widths are all at
/// or over the fab's track minimum (and whose pair gap meets its spacing) comes first, then the fab's default, then the narrowest
/// 50-ohm outer line (the most room to route); without needs, the fab's default first.
pub fn propose(p: &Profile, o: &Opts, needs: &Needs, given: bool, track_min: f64, space_min: f64) -> Vec<Value> {
    let mut v: Vec<(bool, bool, f64, Value)> = super::model::stackups(p, o).into_iter().map(|s| {
        let ls = layers(s, needs);
        let ws: Vec<f64> = ls.iter().flat_map(|l| l["widths"].as_object().into_iter().flatten().filter_map(|(_, x)| x.as_f64()).collect::<Vec<_>>()).collect();
        let feasible = !ws.is_empty() && ws.iter().all(|w| *w >= track_min - 1e-9) && needs.gap_mm >= space_min - 1e-9;
        let w50 = ls.first().and_then(|l| l["widths"]["50"].as_f64()).unwrap_or(f64::MAX);
        let why = if !given { if s.default { "the fab's default for this board (no impedance needs given)".to_string() } else { "fits the board".to_string() } }
            else if feasible { format!("every width the project needs is at or over the fab's {track_min} mm track minimum") }
            else { format!("a width the project needs is under the fab's {track_min} mm track minimum, or the pair gap under its {space_min} mm spacing") };
        (feasible || !given, s.default, w50, json!({"id": s.id, "default": s.default, "feasible": feasible || !given, "why": why, "outer_50_mm": if w50 < f64::MAX { json!(w50) } else { Value::Null }}))
    }).collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then(if given { a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal).then(b.1.cmp(&a.1)) } else { b.1.cmp(&a.1).then(std::cmp::Ordering::Equal) }));
    v.into_iter().map(|x| x.3).collect()
}

/// The view's `stackup`: the picked one (options.stackup) or the first proposal, its layers, and the proposals.
pub fn view(p: &Profile, o: &Opts, options: &Value, track_min: f64, space_min: f64) -> Value {
    let (needs, given) = Needs::from(options, space_min);
    let props = propose(p, o, &needs, given, track_min, space_min);
    let picked = options["stackup"].as_str().filter(|id| p.stackups.iter().any(|s| s.id == *id));
    let id = picked.map(str::to_string).or_else(|| props.first().and_then(|x| x["id"].as_str().map(str::to_string)));
    let Some(s) = id.as_deref().and_then(|id| p.stackups.iter().find(|s| s.id == id)) else { return json!({"chosen": null, "proposals": props, "layers": []}) };
    json!({"chosen": s.id, "by": if picked.is_some() { "picked" } else { "proposed" }, "thickness_mm": s.thickness_mm, "source": s.source, "fetched": s.fetched,
        "needs": {"single": needs.single, "diff": needs.diff, "gap_mm": needs.gap_mm, "given": given}, "layers": layers(s, &needs), "proposals": props.into_iter().take(8).collect::<Vec<_>>(),
        "estimate": "closed-form, no solder mask; the fab's impedance calculator is the reference"})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fab::store;
    fn four() -> Opts { Opts { layers: 4.0, ..Opts::default() } }
    #[test] fn fab_stackup_widths_are_the_textbook_ones() {
        let w = solve(|w| microstrip(w, 0.2104, 0.035, 4.4), 50.0).unwrap(); assert!((w - 0.367).abs() < 0.01, "{w}");
        let w = solve(|w| microstrip(w, 0.0994, 0.035, 4.1), 50.0).unwrap(); assert!((w - 0.171).abs() < 0.01, "{w}");
        assert!(solve(|w| microstrip(w, 0.2, 0.035, 4.4), 500.0).is_none(), "no width gives 500 ohms");
        assert_eq!(layer_names(4), ["F.Cu", "In1.Cu", "In2.Cu", "B.Cu"]);
    }
    #[test] fn fab_stackup_layers_name_their_reference_planes_and_widths() {
        let p = store::baseline("jlcpcb").unwrap(); let s = p.stackups.iter().find(|s| s.id == "JLC04161H-7628").unwrap();
        let (n, _) = Needs::from(&json!({}), 0.09);
        let ls = layers(s, &n);
        assert_eq!(ls.iter().map(|l| l["layer"].as_str().unwrap()).collect::<Vec<_>>(), ["F.Cu", "In1.Cu", "In2.Cu", "B.Cu"]);
        assert_eq!((ls[0]["above"].clone(), ls[0]["below"]["layer"].clone(), ls[0]["below"]["h_mm"].clone(), ls[0]["below"]["er"].clone()), (Value::Null, json!("In1.Cu"), json!(0.2104), json!(4.4)));
        assert_eq!((ls[1]["above"]["layer"].clone(), ls[1]["below"]["layer"].clone(), ls[1]["below"]["h_mm"].clone()), (json!("F.Cu"), json!("In2.Cu"), json!(1.065)));
        assert!((ls[0]["widths"]["50"].as_f64().unwrap() - 0.367).abs() < 0.01); assert!(ls[0]["widths"]["90d"].as_f64().unwrap() < ls[0]["widths"]["50"].as_f64().unwrap());
        assert!(ls[1]["widths"]["50"].as_f64().is_some(), "an inner layer is a stripline between its two: {}", ls[1]);
        let six = p.stackups.iter().find(|s| s.layers == 6).unwrap(); assert_eq!(layers(six, &n).len(), 6);
    }
    #[test] fn fab_stackup_the_proposal_follows_the_needs() {
        let p = store::baseline("jlcpcb").unwrap();
        let (n, given) = Needs::from(&json!({}), 0.09); assert!(!given);
        assert_eq!(propose(&p, &four(), &n, given, 0.09, 0.09)[0]["id"], "JLC04161H-7628", "without needs, the fab's default");
        let (n, given) = Needs::from(&json!({"impedance": 50, "diff_impedance": [90]}), 0.09); assert!(given);
        let ps = propose(&p, &four(), &n, given, 0.09, 0.09);
        assert_eq!(ps[0]["feasible"], true); assert!(ps[0]["outer_50_mm"].as_f64().unwrap() < 0.367, "thinner prepreg: narrower lines, more room: {}", ps[0]);
        // a fab track minimum over every narrow width rules the thin ones out
        let ps = propose(&p, &four(), &n, given, 0.2, 0.09); assert!(ps.iter().filter(|x| x["feasible"] == true).all(|x| x["outer_50_mm"].as_f64().unwrap() >= 0.2));
        assert!(propose(&p, &Opts::default(), &n, given, 0.09, 0.09).is_empty(), "none published for 2 layers");
        let v = view(&p, &four(), &json!({"stackup": "JLC04161H-3313"}), 0.09, 0.09); assert_eq!((v["chosen"].clone(), v["by"].clone()), (json!("JLC04161H-3313"), json!("picked")));
        assert_eq!(view(&p, &four(), &json!({"stackup": "nonsense"}), 0.09, 0.09)["by"], "proposed");
    }
}
