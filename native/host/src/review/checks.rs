//! The design review's deterministic checkers: each reads the design's files (the board, the fab file, the outputs in mfg/, the BOM
//! the hub publishes) and answers findings in lane E's model (kicad/findings.rs), severity "error" (must change), "warning" (should)
//! or "info" (a note), each anchored to what it is about and with a fix an agent can act on. Pure: texts and values in, findings out.
//! Laws `review_check_*`.
use crate::fab::{drc, model::{self, Limit, Opts, Profile, Sel}, project::Choice};
use crate::files::kicad::findings::{Anchor, Finding};
use serde_json::{json, Value};

pub fn finding(source: &str, severity: &str, rule: &str, title: String, detail: String, anchors: Vec<Anchor>, fix: &str) -> Finding {
    Finding { id: String::new(), source: source.into(), severity: severity.into(), rule: rule.into(), title, detail, anchors, fix: fix.into() }
}
pub fn file_anchor(path: &str) -> Anchor { Anchor { doc: "file".into(), text: path.into(), ..Default::default() } }
fn part_anchor(r: &str, layer: &str, x: f64, y: f64) -> Anchor { Anchor { doc: "pcb".into(), text: format!("Footprint {r}"), reference: r.into(), layer: layer.into(), x, y, at: true, ..Default::default() } }
fn mm(v: f64) -> String { let s = format!("{v:.4}"); format!("{} mm", s.trim_end_matches('0').trim_end_matches('.')) }
fn src(l: &Limit) -> String { format!("{}, read {}", l.source, l.fetched) }

// ---- what the board uses ----

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Part { pub reference: String, pub layer: String, pub x: f64, pub y: f64, pub rot: f64, pub smd: bool, pub value: String }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    pub copper_layers: usize, pub thickness: Option<f64>,
    /// the narrowest track and where, the smallest via (diameter, drill) and where, the smallest pad hole
    pub track_min: Option<(f64, Anchor)>, pub via_min: Option<(f64, f64, Anchor)>, pub drill_min: Option<(f64, Anchor)>,
    pub parts: Vec<Part>,
}
fn num_after(block: &str, key: &str) -> Option<f64> { let i = block.find(key)? + key.len(); block[i..].split(|c: char| c == ')' || c.is_whitespace()).find(|s| !s.is_empty())?.parse().ok() }
fn str_after(block: &str, key: &str) -> String { block.find(key).map(|i| block[i + key.len()..].split('"').nth(1).unwrap_or("").to_string()).unwrap_or_default() }
/// The top-level forms of a kind ("segment", "via", "footprint"): each from its start to the next top-level form.
fn blocks<'a>(text: &'a str, kind: &str) -> Vec<&'a str> {
    let open = format!("\n\t({kind}"); let mut v = vec![];
    let mut at = 0;
    while let Some(i) = text[at..].find(&open) {
        let s = at + i + 1; let e = text[s + 1..].find("\n\t(").map_or(text.len(), |j| s + 1 + j);
        v.push(&text[s..e]); at = e;
    }
    v
}
/// Scan a .kicad_pcb (stock or the fork's format) for what the review compares with the fab: layers, thickness, the narrowest
/// track, the smallest via and hole, and every footprint's side and place.
pub fn usage(pcb: &str) -> Usage {
    let mut u = Usage::default();
    let b = crate::fab::project::board_opts(pcb);
    u.copper_layers = b["layers"].as_u64().unwrap_or(0) as usize; u.thickness = b["thickness_mm"].as_f64();
    for s in blocks(pcb, "segment").into_iter().chain(blocks(pcb, "arc")) {
        if let Some(w) = num_after(s, "(width ") { if w > 0.0 && u.track_min.as_ref().map_or(true, |(m, _)| w < *m) {
            let a = Anchor { doc: "pcb".into(), text: format!("Track [{}] on {}", str_after(s, "(net "), str_after(s, "(layer ")), net: str_after(s, "(net "), layer: str_after(s, "(layer "), uuid: str_after(s, "(uuid "), x: num_after(s, "(start ").unwrap_or(0.0), y: s.find("(start ").and_then(|i| s[i + 7..].split_whitespace().nth(1)).and_then(|t| t.trim_end_matches(')').parse().ok()).unwrap_or(0.0), at: true, ..Default::default() };
            u.track_min = Some((w, a)) } }
    }
    for v in blocks(pcb, "via") {
        let (Some(d), Some(h)) = (num_after(v, "(size "), num_after(v, "(drill ")) else { continue };
        if u.via_min.as_ref().map_or(true, |(_, mh, _)| h < *mh) {
            let (x, y) = v.find("(at ").map(|i| { let mut it = v[i + 4..].split(|c: char| c.is_whitespace() || c == ')').filter(|s| !s.is_empty()); (it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0), it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0)) }).unwrap_or((0.0, 0.0));
            let micro = { let h = v.trim_start(); h.starts_with("(via micro") || h.starts_with("(via blind") || v.contains("(type micro)") || v.contains("(type blind)") };
            u.via_min = Some((d, h, Anchor { doc: "pcb".into(), text: format!("{}Via [{}]", if micro { "Micro " } else { "" }, str_after(v, "(net ")), net: str_after(v, "(net "), uuid: str_after(v, "(uuid "), x, y, at: true, ..Default::default() }))
        }
    }
    for f in blocks(pcb, "footprint") {
        let r = f.find("(property \"Reference\" ").map(|i| f[i + 22..].split('"').nth(1).unwrap_or("").to_string()).unwrap_or_default();
        let layer = f.find("(layer ").map(|i| f[i + 7..].split('"').nth(1).unwrap_or("").to_string()).unwrap_or_default();
        let place = f.find("(translate ").map(|i| i + 11).or_else(|| f.find("\n\t\t(at ").map(|i| i + 7));
        let mut it = place.map(|i| f[i..].split(|c: char| c.is_whitespace() || c == ')').filter(|s| !s.is_empty()).take(3).map(|s| s.parse::<f64>().unwrap_or(0.0)).collect::<Vec<_>>()).unwrap_or_default().into_iter();
        let (x, y) = (it.next().unwrap_or(0.0), it.next().unwrap_or(0.0));
        let rot = f.find("(rotate ").and_then(|i| f[i + 8..].split(')').next()?.trim().parse().ok()).or(it.next()).unwrap_or(0.0);
        let value = f.find("(property \"Value\" ").map(|i| f[i + 18..].split('"').nth(1).unwrap_or("").to_string()).unwrap_or_default();
        let smd = f.contains("(attr smd");
        for p in f.split("(pad ").skip(1) {
            if p.contains(" thru_hole ") || p.contains(" np_thru_hole ") { if let Some(d) = num_after(p, "(drill ").filter(|d| *d > 0.0) {
                if u.drill_min.as_ref().map_or(true, |(m, _)| d < *m) { u.drill_min = Some((d, Anchor { doc: "pcb".into(), text: format!("Pad {} of {r}", p.split('"').nth(1).unwrap_or("")), reference: r.clone(), pin: format!("{r}.{}", p.split('"').nth(1).unwrap_or("")), ..Default::default() })) }
            } }
        }
        if !r.is_empty() { u.parts.push(Part { reference: r, layer, x, y, rot, smd, value }) }
    }
    u
}

// ---- (a) the DRC rules' fab section, (b) the fab's options and limits ----

/// The project's DRC rules against the fab: a missing or stale section (one click fixes it), the person's rules that allow less,
/// the board setup minimums under the fab's.
pub fn drc_section(plan: &drc::Plan, dru_text: &str, dru_rel: &str) -> Vec<Finding> {
    let mut v = vec![];
    let cur = drc::section(dru_text);
    if cur != Some(plan.section.as_str()) {
        let (t, d) = if cur.is_none() { ("The fab's limits are not in the project's DRC rules", "DRC does not check the board against the fab: tracks, holes, rings and clearances the fab cannot make pass unseen.") }
            else { ("The project's DRC rules have older fab limits", "The fab's profile, the board's options or its net classes changed since the section was written.") };
        v.push(finding("fab", "warning", "fab_rules_section", t.into(), d.into(), vec![file_anchor(dru_rel)], "Call fab_profile with action \"drc\" (the Fab panel's Write): it writes the fab's section and keeps every rule of the project's own"));
    }
    for c in &plan.conflicts { v.push(finding("fab", "warning", "fab_rule_conflict", format!("A project rule allows less than the fab: {c}"), "Where this rule matches, DRC passes what the fab cannot make (the project's rules win where they match).".into(), vec![file_anchor(dru_rel)], "Raise the rule's minimum to the fab's, or narrow its condition to where the design really needs it and confirm the fab can make it")) }
    for c in &plan.costs_more { v.push(finding("fab", "info", "fab_rule_costs_more", format!("A project rule allows what costs more: {c}"), "Not a must-change: the fab makes it, at a higher price.".into(), vec![file_anchor(dru_rel)], "Keep it if the design needs it; otherwise raise it to the fab's base-price limit")) }
    for s in &plan.suggestions {
        v.push(finding("fab", "info", "fab_setup_minimum", format!("Board setup minimum {} is {} mm, the fab's is {} mm", s["key"].as_str().unwrap_or(""), s["project"], s["fab"]),
            format!("Source: {}, read {}. The fab's section in the DRC rules already checks it; raising the board setup's own minimum keeps it if the section is ever removed.", s["source"].as_str().unwrap_or(""), s["fetched"].as_str().unwrap_or("")), vec![], "The person applies it from the Fab panel (Apply); an agent names it, never changes the board setup itself"))
    }
    v
}

fn get<'a>(sel: &'a [Sel], k: &str) -> Option<&'a Sel> { sel.iter().find(|s| s.key == k) }
/// One measured value against a limit: under the free one but within the one that costs more is a note with the cost; under both a
/// must-change; between the fab's and Backplane's recommendation a note.
fn against(out: &mut Vec<Finding>, sel: &[Sel], key: &str, what: &str, v: f64, a: Anchor, recommend: Option<f64>) {
    let Some(s) = get(sel, key) else { return };
    let free = s.free.as_ref().and_then(|l| l.num().map(|n| (n, l))); let extra = s.extra.as_ref().and_then(|l| l.num().map(|n| (n, l)));
    let fix = "Widen it to the fab's free limit, or keep it and accept the extra cost (the fab's profile, tier \"extra\")";
    match (free, extra) {
        (Some((f, l)), _) if v >= f - 1e-9 => if let Some(r) = recommend.filter(|r| v < r - 1e-9) {
            out.push(finding("fab", "info", "fab_recommended", format!("{what} {} meets the fab's {}, under the recommended {}", mm(v), mm(f), mm(r)), format!("Not a must-change. Source of the fab's limit: {}.", src(l)), vec![a], "Widen it where there is room"))
        },
        (Some((f, _)), Some((e, le))) if v >= e - 1e-9 => out.push(finding("fab", "info", "fab_costs_more", format!("{what} {} is under the fab's {} and costs more", mm(v), mm(f)), format!("The fab makes down to {} for more: {}. {}", mm(e), src(le), le.note), vec![a], fix)),
        (Some((f, l)), _) => out.push(finding("fab", "error", "fab_limit", format!("{what} {} is under what the fab can make ({})", mm(v), mm(extra.map_or(f, |x| x.0))), format!("Source: {}. {}", src(l), l.note), vec![a], "Make it at least the fab's limit")),
        (None, Some((e, le))) if v < e - 1e-9 => out.push(finding("fab", "error", "fab_limit", format!("{what} {} is under what the fab can make ({})", mm(v), mm(e)), format!("Source: {}.", src(le)), vec![a], "Make it at least the fab's limit")),
        _ => {}
    }
}
/// The board's options and what it uses against the fab's profile (what DRC rules cannot say): layer count, thickness, finish and
/// colour offered for this board, its size, the narrowest track, the smallest via and hole.
pub fn fab_checks(p: &Profile, c: &Choice, fab_json: &Value, fab_rel: &str, u: &Usage) -> Vec<Finding> {
    let mut v = vec![]; let o: Opts = c.opts(); let sel = model::select(p, &c.kind, &o);
    let fa = || vec![file_anchor(fab_rel)];
    if let Some(m) = get(&sel, "layers_max").and_then(|s| s.free.as_ref()).and_then(Limit::num) { if o.layers > m { v.push(finding("fab", "error", "fab_option", format!("{} copper layers: the fab makes this board kind with at most {m}", o.layers), String::new(), fa(), "Choose another fab or board kind, or fewer layers")) } }
    if u.copper_layers > 0 && (u.copper_layers as f64 - o.layers).abs() > 0.5 { v.push(finding("fab", "error", "fab_option_board", format!("The fab file says {} layers, the board has {}", o.layers, u.copper_layers), "The fab would make what the fab file says.".into(), fa(), "Set options.layers in the fab file to the board's copper layer count")) }
    if let Some(t) = u.thickness.filter(|_| !fab_json["options"]["thickness_mm"].is_null()) { if (t - o.thickness_mm).abs() > 0.05 { v.push(finding("fab", "warning", "fab_option_board", format!("The fab file says {} thick, the board's stackup {}", mm(o.thickness_mm), mm(t)), String::new(), fa(), "Make the fab file's thickness_mm and the board's stackup thickness agree")) } }
    for (key, val) in [("thicknesses", fab_json["options"]["thickness_mm"].clone()), ("finishes", fab_json["options"]["finish"].clone())] {
        if val.is_null() { continue }
        if let Some(l) = get(&sel, key).and_then(|s| s.free.as_ref()) { if let Some(a) = l.value.as_array() { if !a.iter().any(|x| x == &val || x.as_f64().zip(val.as_f64()).map_or(false, |(a, b)| (a - b).abs() < 1e-9)) {
            v.push(finding("fab", "error", "fab_option", format!("{} {val} is not one the fab offers for this board ({})", if key == "finishes" { "Finish" } else { "Thickness" }, a.iter().map(|x| x.to_string().trim_matches('"').to_string()).collect::<Vec<_>>().join(", ")), format!("Source: {}. {}", src(l), l.note), fa(), "Choose one the fab offers"))
        } } }
    }
    let (w, h) = (fab_json["features"]["width_mm"].as_f64(), fab_json["features"]["height_mm"].as_f64());
    if let (Some(w), Some(h)) = (w, h) {
        let (long, short) = if w >= h { (w, h) } else { (h, w) };
        if let Some(l) = get(&sel, "board_min").and_then(|s| s.free.as_ref()) { if short < l.num().unwrap_or(0.0) { v.push(finding("fab", "error", "fab_board_size", format!("The board's {} side is under the fab's smallest, {}", mm(short), mm(l.num().unwrap_or(0.0))), format!("Source: {}.", src(l)), fa(), "Panelize it, or make it bigger")) } }
        let (mx, my) = (get(&sel, "board_max_x").and_then(|s| s.free.as_ref()).and_then(Limit::num), get(&sel, "board_max_y").and_then(|s| s.free.as_ref()).and_then(Limit::num));
        if let (Some(mx), Some(my)) = (mx, my) { if long > mx || short > my { v.push(finding("fab", "error", "fab_board_size", format!("The board, {} by {}, is over the fab's largest {} by {}", mm(long), mm(short), mm(mx), mm(my)), String::new(), fa(), "Make it smaller or split it")) } }
    }
    if let Some((w, a)) = &u.track_min { against(&mut v, &sel, "track_min", "The narrowest track,", *w, a.clone(), Some(0.127)) }
    if let Some((d, h, a)) = &u.via_min {
        let micro = a.text.starts_with("Micro");
        if micro { against(&mut v, &sel, "microvia_hole_min", "The smallest microvia hole,", *h, a.clone(), None) } else {
            against(&mut v, &sel, "via_hole_min", "The smallest via hole,", *h, a.clone(), Some(0.3));
            against(&mut v, &sel, "via_diameter_min", "The smallest via diameter,", *d, a.clone(), Some(0.45));
        }
    }
    if let Some((d, a)) = &u.drill_min { against(&mut v, &sel, "drill_min", "The smallest pad hole,", *d, a.clone(), None) }
    v
}

// ---- (c) the BOM ----

/// The BOM lines (the hub's published `mgmt.bom.<digest>` lines) against what ordering needs: an MPN on every line that is placed; with
/// stock data (`stock`, `lifecycle`, `library` on a line, from the BOM's own publisher; never asked of the parts service here) the
/// stock at the project's quantity, a lifecycle that is not ending, and extended parts named. Answers the findings and what it could not check.
pub fn bom(lines: &[Value], boards: u64) -> (Vec<Finding>, Vec<String>) {
    let mut v = vec![]; let mut skipped = vec![];
    let refs = |l: &Value| l["refs"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
    let anchors = |l: &Value| refs(l).into_iter().map(|r| Anchor { doc: "sch".into(), text: format!("Symbol {r}"), reference: r, ..Default::default() }).collect::<Vec<_>>();
    let has_stock = lines.iter().any(|l| l["stock"].is_object() || l["lifecycle"].is_object());
    let mut unchecked = 0usize;
    for l in lines.iter().filter(|l| l["dnp"] != true) {
        let rs = refs(l).join(", "); let mpn = l["mpn"].as_str().unwrap_or("").trim();
        if mpn.is_empty() { v.push(finding("bom", "error", "bom_no_mpn", format!("{rs}: no manufacturer part number"), format!("{} {} has no MPN field, so it cannot be bought or assembled.", l["value"].as_str().unwrap_or(""), l["footprint"].as_str().unwrap_or("")), anchors(l), "Find the part (prefer the fab's basic or preferred parts: pcbparts jlc_search) and set the symbol's MPN field")); continue }
        let s = &l["stock"];
        let need = s["need"].as_u64().unwrap_or(l["qty"].as_u64().unwrap_or(1) * boards.max(1));
        match s["status"].as_str().unwrap_or("") {
            "out" => v.push(finding("bom", "error", "bom_stock", format!("{rs} ({mpn}) is out of stock: {need} needed"), format!("Checked {}.", s["checked"]), anchors(l), "Choose an alternative in stock (pcbparts jlc_find_alternatives) and update the MPN")),
            "low" => v.push(finding("bom", "error", "bom_stock", format!("{rs} ({mpn}): {} in stock, {need} needed", s["stock"]), format!("Checked {}.", s["checked"]), anchors(l), "Choose an alternative in stock (pcbparts jlc_find_alternatives) and update the MPN")),
            "in" => {}
            _ => unchecked += 1,
        }
        let lc = &l["lifecycle"]; let words = lc["words"].as_str().unwrap_or("");
        match lc["status"].as_str().unwrap_or("") {
            "nrnd" => v.push(finding("bom", "warning", "bom_lifecycle", format!("{rs} ({mpn}) is not recommended for new designs{}", if words.is_empty() { String::new() } else { format!(" ({words})") }), format!("Source: {}.", lc["source"].as_str().unwrap_or("")), anchors(l), "Choose an active alternative")),
            "eol" => v.push(finding("bom", "error", "bom_lifecycle", format!("{rs} ({mpn}) is {}", if words.is_empty() { "at its end of life" } else { words }), format!("Source: {}.", lc["source"].as_str().unwrap_or("")), anchors(l), "Choose an active alternative")),
            _ => {}
        }
        if l["library"]["class"] == "extended" { v.push(finding("bom", "info", "bom_extended", format!("{rs} ({mpn}) is an extended part of the fab's stock"), "The fab charges a loading fee per extended part.".into(), anchors(l), "A basic or preferred equivalent saves the fee (pcbparts jlc_find_alternatives)")) }
    }
    if has_stock && unchecked > 0 { skipped.push(format!("BOM stock: {unchecked} lines not checked yet")) }
    if lines.is_empty() { skipped.push("BOM: no BOM published for this project (the BOM panel's data); open the design so it is read".into()) }
    else if !has_stock { skipped.push("BOM stock, lifecycle and the fab's library class: no stock data published for this BOM yet".into()) }
    (v, skipped)
}

// ---- (d) the fab outputs ----

/// One set of outputs in mfg/: its folder and files (names and texts of the drill files).
pub struct Outputs { pub dir: String, pub files: Vec<String>, pub drills: Vec<(String, String)> }
/// A layer's Gerber in a set: KiCad's names ("-F_Cu.gtl", "-In1_Cu.g1", "-Edge_Cuts.gm1") or Protel extensions.
fn has_layer(files: &[String], kicad: &str, protel: &str) -> bool {
    files.iter().any(|f| { let l = f.to_ascii_lowercase(); l.contains(&format!("-{}", kicad.to_ascii_lowercase())) || l.ends_with(&format!(".{protel}")) })
}
/// The newest outputs against the board: present, every copper layer, outline, mask and silkscreen, drill files, holes within the fab's.
pub fn outputs(o: Option<&Outputs>, copper: usize, sel: &[Sel]) -> Vec<Finding> {
    let mut v = vec![];
    let Some(o) = o else {
        v.push(finding("outputs", "info", "outputs_none", "No Gerber and drill files in mfg/ yet".into(), "A buy thread exports them when it orders; nothing to check before.".into(), vec![], "Export Gerbers and drill files into mfg/<date>/ (kicad-cli pcb export gerbers / drill)"));
        return v
    };
    let a = || vec![file_anchor(&o.dir)];
    let mut missing: Vec<String> = vec![];
    for i in 0..copper {
        let (k, p) = if i == 0 { ("F_Cu".to_string(), "gtl".to_string()) } else if i + 1 == copper { ("B_Cu".to_string(), "gbl".to_string()) } else { (format!("In{i}_Cu"), format!("g{i}")) };
        if !has_layer(&o.files, &k, &p) { missing.push(k.replace('_', ".")) }
    }
    for (k, p) in [("Edge_Cuts", "gm1"), ("F_Mask", "gts"), ("B_Mask", "gbs")] { if !has_layer(&o.files, k, p) && !(k == "Edge_Cuts" && has_layer(&o.files, "Edge_Cuts", "gko")) { missing.push(k.replace('_', ".")) } }
    if !missing.is_empty() { v.push(finding("outputs", "error", "outputs_layers", format!("The outputs in {} miss {}", o.dir, missing.join(", ")), format!("The board has {copper} copper layers."), a(), "Export every copper layer, the outline (Edge.Cuts), both masks and silkscreens again")) }
    if o.drills.is_empty() { v.push(finding("outputs", "error", "outputs_drill", format!("No drill file in {}", o.dir), String::new(), a(), "Export the drill files (kicad-cli pcb export drill)")) }
    // the smallest tool in the drill files against the fab's smallest hole (laser microvia files against the microvia minimum)
    let floor = |k: &str| get(sel, k).and_then(|s| s.extra.as_ref().or(s.free.as_ref())).and_then(Limit::num);
    for (name, text) in &o.drills {
        let tools: Vec<f64> = text.lines().filter(|l| l.starts_with('T') && l.contains('C')).filter_map(|l| l.split('C').nth(1)?.trim().parse().ok()).collect();
        let Some(min) = tools.iter().cloned().fold(None, |m: Option<f64>, t| Some(m.map_or(t, |m| m.min(t)))) else { continue };
        let laser = !name.contains("PTH") && !name.contains("NPTH") && name.contains('-') && name.to_ascii_lowercase().contains("in");
        let lim = if laser { floor("microvia_hole_min").or(floor("drill_min")) } else { floor("drill_min") };
        if let Some(l) = lim { if min < l - 1e-9 { v.push(finding("outputs", "error", "outputs_drill_size", format!("{name} drills {}, under the fab's smallest {}", mm(min), mm(l)), String::new(), vec![file_anchor(&format!("{}/{name}", o.dir))], "Enlarge those holes on the board and export again")) } }
    }
    v
}
/// An output set re-exported from the board now, against the set in mfg/: the layers whose drawing differs (dates and tool names left out).
pub fn stale_layers(exported: &[(String, String)], kept: &[(String, String)]) -> Vec<String> {
    let norm = |t: &str| t.lines().filter(|l| !l.contains("CreationDate") && !l.starts_with("G04") && !l.contains("GenerationSoftware") && !l.starts_with("; #@!") && !l.starts_with(";")).collect::<Vec<_>>().join("\n");
    let tail = |n: &str| n.rsplit_once('-').map_or(n.to_string(), |(_, t)| t.to_string());
    exported.iter().filter_map(|(n, t)| { let k = kept.iter().find(|(m, _)| tail(m) == tail(n))?; (norm(t) != norm(&k.1)).then(|| tail(n)) }).collect()
}

// ---- (e) assembly ----

/// Placements against the board: parts on both sides (the economic service places one), the placement file in the outputs (every
/// placed part of the board in it, each at the board's side), fiducials when the standard service is needed.
pub fn assembly(u: &Usage, pos: Option<(&str, &str)>, assembled: bool) -> Vec<Finding> {
    let mut v = vec![];
    if !assembled || u.parts.is_empty() { return v }
    let back: Vec<&Part> = u.parts.iter().filter(|p| p.layer == "B.Cu" && p.smd).collect();
    let front = u.parts.iter().filter(|p| p.layer == "F.Cu" && p.smd).count();
    if !back.is_empty() && front > 0 {
        v.push(finding("assembly", "info", "asm_two_sides", format!("Parts on both sides ({front} on top, {} on the bottom)", back.len()), "Only the standard assembly service places both sides: it costs more and needs edge rails and fiducials.".into(),
            back.iter().take(12).map(|p| part_anchor(&p.reference, &p.layer, p.x, p.y)).collect(), "Move the bottom parts to the top where they fit, or order the standard service"));
        if !u.parts.iter().any(|p| p.reference.to_ascii_uppercase().starts_with("FID")) { v.push(finding("assembly", "warning", "asm_fiducials", "No fiducials, and the standard assembly service needs them".into(), "Three or four 1 mm copper fiducials in a 2 mm mask opening, 3.35 mm or more from the edge (or on the panel's rails).".into(), vec![], "Add fiducials (Fiducial footprints) or ask for them on the panel rails")) }
    }
    if let Some((name, text)) = pos {
        let mut rows: Vec<(String, String)> = vec![];
        for l in text.lines().skip(1) { let c: Vec<&str> = l.split(',').map(|x| x.trim().trim_matches('"')).collect(); if c.len() >= 4 { rows.push((c[0].to_string(), c[3].to_string())) } }
        let missing: Vec<&Part> = u.parts.iter().filter(|p| p.smd && !p.reference.to_ascii_uppercase().starts_with("FID") && !rows.iter().any(|(r, _)| r == &p.reference)).collect();
        if !missing.is_empty() { v.push(finding("assembly", "warning", "asm_pos_missing", format!("{} placed parts are not in {name}: {}", missing.len(), missing.iter().take(8).map(|p| p.reference.as_str()).collect::<Vec<_>>().join(", ")), "The placement file is older than the board, or these parts are excluded.".into(), missing.iter().take(12).map(|p| part_anchor(&p.reference, &p.layer, p.x, p.y)).collect(), "Export the placement file again from the saved board")) }
        let wrong: Vec<String> = rows.iter().filter_map(|(r, side)| { let p = u.parts.iter().find(|p| &p.reference == r)?; let bot = side.to_ascii_lowercase().starts_with('b'); ((p.layer == "B.Cu") != bot).then(|| r.clone()) }).collect();
        if !wrong.is_empty() { v.push(finding("assembly", "error", "asm_pos_side", format!("{name} puts {} on the other side than the board: {}", wrong.len(), wrong.iter().take(8).cloned().collect::<Vec<_>>().join(", ")), String::new(), vec![file_anchor(name)], "Export the placement file again from the saved board")) }
    }
    v
}

// ---- (f) mechanical parts ----

/// Parts made elsewhere than the board (fab files of 3dp, cnc, sheetmetal): the checks a fab file already carries (a SendCutSend part's
/// DFM result with each rule's source). Geometry rules (walls, radii) need the model: they are the agent-judged review's.
pub fn mech(files: &[(String, Value)]) -> (Vec<Finding>, Vec<String>) {
    let mut v = vec![]; let mut skipped = vec![];
    for (path, j) in files {
        let k = j["kind"].as_str().unwrap_or("");
        if !matches!(k, "3dp" | "cnc" | "sheetmetal") || j["order"] == false { continue }
        let rows = j.pointer("/sendcutsend/dfm/rows").and_then(Value::as_array);
        match rows {
            Some(rows) => for r in rows.iter().filter(|r| matches!(r["status"].as_str(), Some("FAIL") | Some("WARN"))) {
                v.push(finding("mech", if r["status"] == "FAIL" { "error" } else { "warning" }, "mech_dfm", format!("{}: {}", j["name"].as_str().unwrap_or(path), r["rule"].as_str().unwrap_or("")), format!("{} Source: {}", r["detail"].as_str().unwrap_or(""), r["source"].as_str().unwrap_or("")), vec![file_anchor(path)], "Change the part model, run the sendcutsend skill's dfm again and write its fab file"))
            },
            None => skipped.push(format!("{path}: walls, flanges and inside radii need the model (agent-judged review)")),
        }
    }
    (v, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fab::store;
    const PCB: &str = "(kicad_pcb\n\t(general\n\t\t(thickness 1.6)\n\t)\n\t(layers\n\t\t(0 \"F.Cu\" signal)\n\t\t(2 \"B.Cu\" signal)\n\t)\n\t(footprint \"R:R_0402\"\n\t\t(layer \"F.Cu\")\n\t\t(at 10 20 90)\n\t\t(property \"Reference\" \"R1\"\n\t\t)\n\t\t(attr smd)\n\t\t(pad \"1\" smd rect)\n\t)\n\t(footprint \"C:C_0402\"\n\t\t(layer \"B.Cu\")\n\t\t(transform\n\t\t\t(translate 5 6)\n\t\t\t(rotate 180)\n\t\t)\n\t\t(property \"Reference\" \"C1\"\n\t\t)\n\t\t(attr smd)\n\t)\n\t(footprint \"J:Hdr\"\n\t\t(layer \"F.Cu\")\n\t\t(at 1 2)\n\t\t(property \"Reference\" \"J1\"\n\t\t)\n\t\t(pad \"1\" thru_hole circle (at 0 0) (size 1.7 1.7) (drill 0.4))\n\t)\n\t(segment\n\t\t(start 1 2)\n\t\t(end 3 4)\n\t\t(width 0.09)\n\t\t(layer \"F.Cu\")\n\t\t(net \"SIG\")\n\t\t(uuid \"aa\")\n\t)\n\t(via\n\t\t(at 7 8)\n\t\t(size 0.4)\n\t\t(drill 0.2)\n\t\t(layers \"F.Cu\" \"B.Cu\")\n\t\t(net \"GND\")\n\t\t(uuid \"bb\")\n\t)\n)\n";
    fn choice(o: Value) -> Choice { Choice { fab: "jlcpcb".into(), kind: "pcb".into(), options: o, by: "fab.json".into(), rev_seen: 0, from: "a.fab.json".into(), tier: "free".into() } }
    #[test] fn review_check_the_board_is_read_in_either_format() {
        let u = usage(PCB);
        assert_eq!((u.copper_layers, u.thickness), (2, Some(1.6)));
        assert_eq!(u.track_min.as_ref().map(|t| (t.0, t.1.net.clone(), t.1.x, t.1.y)), Some((0.09, "SIG".to_string(), 1.0, 2.0)));
        assert_eq!(u.via_min.as_ref().map(|v| (v.0, v.1, v.2.x)), Some((0.4, 0.2, 7.0)));
        assert_eq!(u.drill_min.as_ref().map(|d| (d.0, d.1.pin.clone())), Some((0.4, "J1.1".to_string())));
        assert_eq!(u.parts.iter().map(|p| (p.reference.as_str(), p.layer.as_str(), p.x, p.rot, p.smd)).collect::<Vec<_>>(), [("R1", "F.Cu", 10.0, 90.0, true), ("C1", "B.Cu", 5.0, 180.0, true), ("J1", "F.Cu", 1.0, 0.0, false)]);
    }
    #[test] fn review_check_fab_options_size_and_what_the_board_uses() {
        let p = store::baseline("jlcpcb").unwrap(); let u = usage(PCB);
        let fj = json!({"features": {"width_mm": 700, "height_mm": 20}});
        let fs = fab_checks(&p, &choice(json!({"layers": 2, "thickness_mm": 1.6, "finish": "enig"})), &fj, "a.fab.json", &u);
        let rules: Vec<(&str, &str)> = fs.iter().map(|f| (f.rule.as_str(), f.severity.as_str())).collect();
        assert!(rules.contains(&("fab_board_size", "error")), "{rules:?}");
        assert!(rules.contains(&("fab_limit", "error")), "a 0.09 mm track on 2 layers is under the fab's 0.10: {rules:?}");
        assert!(fs.iter().any(|f| f.rule == "fab_recommended" && f.title.contains("via hole")), "0.2 mm meets the fab, under the recommended 0.3: a note");
        let t = fs.iter().find(|f| f.rule == "fab_limit").unwrap(); assert_eq!(t.anchors[0].net, "SIG"); assert!(t.detail.contains("https://"), "{}", t.detail);
        let four = fab_checks(&p, &choice(json!({"layers": 4, "finish": "pink"})), &json!({"options": {"finish": "pink"}}), "a.fab.json", &u);
        assert!(four.iter().any(|f| f.rule == "fab_option_board"), "the fab file says 4, the board 2"); assert!(four.iter().any(|f| f.rule == "fab_option" && f.title.starts_with("Finish")));
        assert!(!four.iter().any(|f| f.rule == "fab_limit"), "0.09 mm is the 4-layer free limit");
    }
    #[test] fn review_check_the_drc_section_and_its_conflicts() {
        let p = store::baseline("jlcpcb").unwrap(); let c = choice(json!({"layers": 2}));
        let pr = drc::project_rules(r#"{"board":{"design_settings":{"rules":{"min_clearance":0.0}}},"net_settings":{"classes":[{"name":"Default","clearance":0.2}]}}"#);
        let own = "(version 1)\n(rule tight (constraint clearance (min 0.05mm)))\n";
        let plan = drc::plan(&p, &c, &pr, own);
        let fs = drc_section(&plan, own, "x.kicad_dru");
        assert_eq!(fs.iter().map(|f| f.rule.as_str()).collect::<Vec<_>>(), ["fab_rules_section", "fab_rule_conflict", "fab_setup_minimum"]);
        assert!(drc_section(&plan, &drc::merge(own, &plan.section), "x.kicad_dru").iter().all(|f| f.rule != "fab_rules_section"), "in sync: no finding");
    }
    #[test] fn review_check_the_bom_needs_mpns_and_says_what_it_could_not_check() {
        let lines = vec![json!({"refs": ["R1", "R2"], "qty": 2, "value": "10k", "mpn": "", "dnp": false}), json!({"refs": ["R9"], "qty": 1, "mpn": "", "dnp": true}), json!({"refs": ["U1"], "qty": 1, "mpn": "NRF54", "dnp": false})];
        let (fs, sk) = bom(&lines, 5);
        assert_eq!(fs.iter().map(|f| (f.rule.as_str(), f.anchors.len())).collect::<Vec<_>>(), [("bom_no_mpn", 2)], "a DNP line needs no MPN"); assert_eq!(sk.len(), 1);
        let lines = vec![json!({"refs": ["U1"], "qty": 2, "mpn": "X", "stock": {"status": "low", "stock": 7, "need": 10, "checked": 1}, "lifecycle": {"status": "nrnd", "words": "NRND", "source": "Mouser"}, "library": {"class": "extended"}})];
        let (fs, sk) = bom(&lines, 5);
        assert_eq!(fs.iter().map(|f| (f.rule.as_str(), f.severity.as_str())).collect::<Vec<_>>(), [("bom_stock", "error"), ("bom_lifecycle", "warning"), ("bom_extended", "info")]); assert!(sk.is_empty());
        assert_eq!(bom(&[], 5).1.len(), 1);
    }
    #[test] fn review_check_outputs_layers_drills_and_staleness() {
        let p = store::baseline("jlcpcb").unwrap(); let sel = model::select(&p, "pcb", &Opts::default());
        let o = Outputs { dir: "mfg/x".into(), files: ["b-F_Cu.gtl", "b-B_Cu.gbl", "b-F_Mask.gts", "b-B_Mask.gbs"].map(String::from).to_vec(), drills: vec![("b-PTH.drl".into(), "M48\nT1C0.120\nT2C0.300\n%".into())] };
        let fs = outputs(Some(&o), 2, &sel);
        assert_eq!(fs.iter().map(|f| f.rule.as_str()).collect::<Vec<_>>(), ["outputs_layers", "outputs_drill_size"]); assert!(fs[0].title.ends_with("Edge.Cuts"), "{}", fs[0].title);
        assert_eq!(outputs(None, 2, &sel)[0].severity, "info");
        let a = vec![("x-F_Cu.gtl".to_string(), "G04 date 1*\n%TF.CreationDate,1*%\nX1Y1D01*".to_string()), ("x-B_Cu.gbl".to_string(), "X1Y1D01*".to_string())];
        let b = vec![("y-F_Cu.gtl".to_string(), "G04 date 2*\n%TF.CreationDate,2*%\nX1Y1D01*".to_string()), ("y-B_Cu.gbl".to_string(), "X2Y1D01*".to_string())];
        assert_eq!(stale_layers(&a, &b), ["B_Cu.gbl"], "dates are no difference; drawing is");
    }
    #[test] fn review_check_assembly_sides_fiducials_and_the_placement_file() {
        let u = usage(PCB);
        let fs = assembly(&u, Some(("mfg/x/pos.csv", "Designator,Mid X,Mid Y,Layer,Rotation\nR1,10,20,Bottom,90\n")), true);
        assert_eq!(fs.iter().map(|f| f.rule.as_str()).collect::<Vec<_>>(), ["asm_two_sides", "asm_fiducials", "asm_pos_missing", "asm_pos_side"]);
        assert_eq!(fs[0].anchors[0].reference, "C1"); assert!(assembly(&u, None, false).is_empty(), "not assembled: nothing");
    }
    #[test] fn review_check_mech_parts_carry_their_dfm_and_the_rest_waits_for_the_model() {
        let (fs, sk) = mech(&[("mech/b.fab.json".into(), json!({"kind": "sheetmetal", "name": "Bracket", "sendcutsend": {"dfm": {"rows": [{"status": "FAIL", "rule": "bend 1 hole distance", "detail": "2.0 mm < 6.45 mm", "source": "specs ALU-063 bending_specs.min_flange_length_before_bend"}, {"status": "PASS", "rule": "min hole"}]}}})), ("mech/c.fab.json".into(), json!({"kind": "3dp"})), ("boards/a.fab.json".into(), json!({"kind": "pcb"}))]);
        assert_eq!(fs.len(), 1); assert!(fs[0].detail.contains("bending_specs")); assert_eq!(sk.len(), 1);
    }
}
