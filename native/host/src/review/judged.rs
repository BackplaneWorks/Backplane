//! The design review's agent-judged checks, kept apart from the deterministic ones: a part's symbol and footprint against its datasheet
//! (pin count, pin numbers and names, pad count, pitch and package). The hub gathers the exact evidence from the design (`evidence`: the
//! symbol's pins from the schematic's own library copy, the footprint's pads from the board, the MPN and datasheet link) and hands it to an
//! agent (the tool `design_review {scope: "judged"}` answers it with what to do); the agent reads each datasheet (the parts service's part
//! and pinout data, the datasheet itself) and answers with `design_review_judged`: findings with a confidence and the datasheet page. They
//! become the source "judged" in the Checks list, marked agent-judged, and a section of their own in the run and review.md. On demand and
//! before an order; never on save. Pure but for `record`. Laws `review_judged_*`.
use crate::files::kicad::findings::{Anchor, Finding};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Pin { pub number: String, pub name: String, pub kind: String }

/// The pins of each library symbol a schematic carries (its `lib_symbols`), by lib id, every unit's together.
pub fn lib_pins(sch: &str) -> BTreeMap<String, (Vec<Pin>, String)> {
    let mut out = BTreeMap::new();
    let Some(start) = sch.find("(lib_symbols") else { return out };
    let body = &sch[start..];
    let end = body.find("\n\t)").unwrap_or(body.len()); let body = &body[..end];
    let heads: Vec<usize> = body.match_indices("\n\t\t(symbol \"").map(|(i, _)| i).collect();
    for (k, &h) in heads.iter().enumerate() {
        let block = &body[h..heads.get(k + 1).copied().unwrap_or(body.len())];
        let id = block.split('"').nth(1).unwrap_or("").to_string();
        let ds = block.find("(property \"Datasheet\" ").map(|i| block[i + 22..].split('"').nth(1).unwrap_or("").to_string()).unwrap_or_default();
        let mut pins = vec![];
        for p in block.split("(pin ").skip(1) {
            let kind = p.split_whitespace().next().unwrap_or("").to_string();
            let name = p.find("(name ").map(|i| p[i + 6..].split('"').nth(1).unwrap_or("").to_string()).unwrap_or_default();
            let number = p.find("(number ").map(|i| p[i + 8..].split('"').nth(1).unwrap_or("").to_string()).unwrap_or_default();
            if !number.is_empty() { pins.push(Pin { number, name, kind }) }
        }
        out.insert(id, (pins, if ds == "~" { String::new() } else { ds }));
    }
    out
}

/// A footprint's pads on the board: their numbers (in order, once each), how many, and the smallest distance between two pads' centres.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pads { pub numbers: Vec<String>, pub count: usize, pub pitch_mm: Option<f64>, pub footprint: String }
pub fn board_pads(pcb: &str) -> BTreeMap<String, Pads> {
    let mut out = BTreeMap::new();
    let open = "\n\t(footprint ";
    let mut at = 0;
    while let Some(i) = pcb[at..].find(open) {
        let s = at + i + 1; let e = pcb[s + 1..].find("\n\t(").map_or(pcb.len(), |j| s + 1 + j); let f = &pcb[s..e]; at = e;
        let name = f.split('"').nth(1).unwrap_or("").to_string();
        let r = f.find("(property \"Reference\" ").map(|i| f[i + 22..].split('"').nth(1).unwrap_or("").to_string()).unwrap_or_default();
        if r.is_empty() { continue }
        let mut pads: Vec<(String, f64, f64)> = vec![];
        for p in f.split("(pad ").skip(1) {
            let n = p.split('"').nth(1).unwrap_or("").to_string();
            let xy = p.find("(at ").map(|i| { let mut it = p[i + 4..].split(|c: char| c.is_whitespace() || c == ')').filter(|x| !x.is_empty()).map(|x| x.parse::<f64>().unwrap_or(0.0)); (it.next().unwrap_or(0.0), it.next().unwrap_or(0.0)) }).unwrap_or((0.0, 0.0));
            if p.contains(" np_thru_hole ") { continue }
            pads.push((n, xy.0, xy.1));
        }
        let mut pitch: Option<f64> = None;
        for a in 0..pads.len() { for b in a + 1..pads.len() {
            let d = ((pads[a].1 - pads[b].1).powi(2) + (pads[a].2 - pads[b].2).powi(2)).sqrt();
            if d > 1e-6 && pads[a].0 != pads[b].0 { pitch = Some(pitch.map_or(d, |p: f64| p.min(d))) }
        } }
        let mut numbers: Vec<String> = vec![]; for (n, _, _) in &pads { if !n.is_empty() && !numbers.contains(n) { numbers.push(n.clone()) } }
        out.insert(r, Pads { count: numbers.len(), numbers, pitch_mm: pitch.map(|p| (p * 1000.0).round() / 1000.0), footprint: name });
    }
    out
}

/// What the agent is handed, per part worth judging (ICs, connectors, transistors and anything with six pads or more), most pins first,
/// `max` of them: its reference, value, MPN, datasheet link, the symbol's pins and the footprint's pads. Parts without an MPN are left
/// out (the BOM check names them).
pub fn evidence(parts: &[crate::files::kicad::bom::Part], sch_texts: &[String], pcb: &str, max: usize) -> (Vec<Value>, usize) {
    let mut libs: BTreeMap<String, (Vec<Pin>, String)> = BTreeMap::new();
    for t in sch_texts { libs.extend(lib_pins(t)) }
    let pads = board_pads(pcb);
    let mut seen: Vec<String> = vec![]; let mut v: Vec<(usize, Value)> = vec![];
    for p in parts {
        if p.mpn.trim().is_empty() || p.dnp || seen.contains(&p.reference) { continue }
        let pre: String = p.reference.chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_ascii_uppercase();
        let (pins, ds) = libs.get(&p.lib).cloned().unwrap_or_default();
        let fp = pads.get(&p.reference).cloned().unwrap_or_default();
        let worth = matches!(pre.as_str(), "U" | "IC" | "J" | "P" | "CN" | "Q" | "Y" | "X") || pins.len().max(fp.count) >= 6;
        if !worth { continue }
        seen.push(p.reference.clone());
        v.push((pins.len().max(fp.count), json!({"reference": p.reference, "value": p.value, "mpn": p.mpn, "maker": p.mfr, "datasheet": ds, "symbol": p.lib,
            "symbol_pins": pins.iter().map(|x| json!([x.number, x.name, x.kind])).collect::<Vec<_>>(), "footprint": p.footprint,
            "pads": {"count": fp.count, "numbers": fp.numbers, "pitch_mm": fp.pitch_mm}})));
    }
    v.sort_by(|a, b| b.0.cmp(&a.0));
    let n = v.len();
    (v.into_iter().take(max).map(|x| x.1).collect(), n.saturating_sub(max))
}

/// What the panel's Datasheets button asks the pane's agent (the person's press: a message in their own thread).
pub const ASK: &str = "Check this design's parts against their datasheets: call design_review with scope \"judged\", work through every part it hands you, and record what you find with design_review_judged.";
pub const HOW: &str = "Check each part against its datasheet (the parts service: pcbparts jlc_get_part / jlc_get_pinout / cse_get_kicad by MPN, and the datasheet link): (1) the symbol: pin count, each pin number and name against the datasheet's pin table (an exposed pad counts); (2) the footprint: pad count and numbering, pitch and package outline against the datasheet's package drawing (and the recommended land pattern where it gives one). Report every mismatch, and only mismatches, with design_review_judged: severity \"must change\" (a wrong pin number, name swap, missing pin, wrong pitch or package), \"should\" (a name that differs in words only, a land pattern off the datasheet's recommendation) or \"note\"; with the datasheet's page and table or figure, and your confidence (0 to 1). Name every part you checked in `checked`, and those you could not (no datasheet found) in `unchecked`. Work only from evidence; never guess a pinout.";

/// What an agent's answer becomes: findings of the source "judged" (anchored to the part and pin), and the run's judged section.
pub fn findings_of(a: &Value) -> Result<Vec<Finding>, String> {
    let fs = a["findings"].as_array().ok_or("findings must be a list")?;
    let mut out = vec![];
    for f in fs.iter().take(200) {
        let r = f["reference"].as_str().unwrap_or("").trim(); let title = f["title"].as_str().unwrap_or("").trim();
        if r.is_empty() || title.is_empty() { return Err("each finding needs reference and title".into()) }
        let sev = match f["severity"].as_str().unwrap_or("") { "must change" | "error" => "error", "should" | "warning" => "warning", _ => "info" };
        let conf = f["confidence"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0);
        let page = f["page"].as_str().map(str::to_string).or_else(|| f["page"].as_u64().map(|n| n.to_string())).unwrap_or_default();
        if page.is_empty() { return Err(format!("{r}: give the datasheet page (and table or figure) the finding rests on")) }
        let pin = f["pin"].as_str().unwrap_or("");
        let a = Anchor { doc: "sch".into(), text: format!("Symbol {r}{}", if pin.is_empty() { String::new() } else { format!(" Pin {pin}") }), reference: r.into(), pin: if pin.is_empty() { String::new() } else { format!("{r}.{pin}") }, ..Default::default() };
        out.push(Finding { id: String::new(), source: "judged".into(), severity: sev.into(), rule: f["rule"].as_str().unwrap_or("datasheet").into(), title: format!("{r}: {title}"),
            detail: format!("{} Datasheet: {page}. Confidence {:.0}%. Agent-judged.", f["detail"].as_str().unwrap_or("").trim(), conf * 100.0), anchors: vec![a],
            fix: f["fix"].as_str().unwrap_or("Correct the symbol or footprint to the datasheet, then run the judged review again").into() });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SCH: &str = "(kicad_sch\n\t(lib_symbols\n\t\t(symbol \"lib:LDO\"\n\t\t\t(property \"Datasheet\" \"https://x/ldo.pdf\")\n\t\t\t(symbol \"LDO_1_1\"\n\t\t\t\t(pin power_in line\n\t\t\t\t\t(name \"VIN\")\n\t\t\t\t\t(number \"1\")\n\t\t\t\t)\n\t\t\t\t(pin power_out line\n\t\t\t\t\t(name \"VOUT\")\n\t\t\t\t\t(number \"5\")\n\t\t\t\t)\n\t\t\t)\n\t\t\t(symbol \"LDO_2_1\"\n\t\t\t\t(pin power_in line\n\t\t\t\t\t(name \"GND\")\n\t\t\t\t\t(number \"2\")\n\t\t\t\t)\n\t\t\t)\n\t\t)\n\t\t(symbol \"lib:R\"\n\t\t\t(property \"Datasheet\" \"~\")\n\t\t)\n\t)\n)\n";
    const PCB: &str = "(kicad_pcb\n\t(footprint \"Package_TO_SOT_SMD:SOT-23-5\"\n\t\t(layer \"F.Cu\")\n\t\t(property \"Reference\" \"U1\"\n\t\t)\n\t\t(pad \"1\" smd rect (at -0.95 1.1) (size 0.6 1))\n\t\t(pad \"2\" smd rect (at 0 1.1) (size 0.6 1))\n\t\t(pad \"3\" smd rect (at 0.95 1.1) (size 0.6 1))\n\t\t(pad \"4\" smd rect (at 0.95 -1.1) (size 0.6 1))\n\t\t(pad \"5\" smd rect (at -0.95 -1.1) (size 0.6 1))\n\t)\n)\n";
    fn part(r: &str, lib: &str, mpn: &str) -> crate::files::kicad::bom::Part { crate::files::kicad::bom::Part { reference: r.into(), value: "v".into(), footprint: "f".into(), mpn: mpn.into(), mfr: String::new(), fields: vec![], dnp: false, inbom: true, lib: lib.into(), sheet: String::new(), unit: 1 } }
    #[test] fn review_judged_the_evidence_is_the_symbols_pins_and_the_footprints_pads() {
        let l = lib_pins(SCH);
        assert_eq!(l["lib:LDO"].0.iter().map(|p| (p.number.as_str(), p.name.as_str())).collect::<Vec<_>>(), [("1", "VIN"), ("5", "VOUT"), ("2", "GND")], "every unit's pins");
        assert_eq!(l["lib:LDO"].1, "https://x/ldo.pdf"); assert_eq!(l["lib:R"].1, "", "~ is no datasheet");
        let p = board_pads(PCB); assert_eq!((p["U1"].count, p["U1"].pitch_mm), (5, Some(0.95)));
        let (ev, more) = evidence(&[part("U1", "lib:LDO", "AP2112K"), part("R1", "lib:R", "RC0402"), part("U2", "lib:LDO", "")], &[SCH.into()], PCB, 10);
        assert_eq!(more, 0); assert_eq!(ev.len(), 1, "a resistor is not worth it, a part without an MPN is the BOM check's");
        assert_eq!((ev[0]["reference"].clone(), ev[0]["pads"]["count"].clone(), ev[0]["symbol_pins"].as_array().unwrap().len()), (json!("U1"), json!(5), 3));
    }
    #[test] fn review_judged_an_answer_becomes_labelled_findings_with_page_and_confidence() {
        let fs = findings_of(&json!({"findings": [{"reference": "U1", "pin": "5", "severity": "must change", "title": "pin 5 is VOUT in the symbol, EN in the datasheet", "page": "p. 3, Table 1", "confidence": 0.9}]})).unwrap();
        assert_eq!((fs[0].source.as_str(), fs[0].severity.as_str(), fs[0].anchors[0].pin.as_str()), ("judged", "error", "U1.5"));
        assert!(fs[0].detail.contains("p. 3, Table 1") && fs[0].detail.contains("90%") && fs[0].detail.contains("Agent-judged"), "{}", fs[0].detail);
        assert!(findings_of(&json!({"findings": [{"reference": "U1", "title": "x"}]})).unwrap_err().contains("page"), "no page, no finding");
    }
}
