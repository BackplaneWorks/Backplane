//! The `altium_write` spec: one JSON object that makes or edits Altium files with no Altium running. Its parts run in
//! this order, so one call can make a footprint, the symbol that uses it and a sheet that places it:
//!
//! - `pcblib`: `{"path", "footprints": [..], "remove": [names]}`: footprints from package dimensions (`package` chip,
//!   soic/sop/tssop/msop/gullwing, sot23, qfn/dfn, header, custom pads), added or replaced by name.
//! - `schlib`: `{"path", "symbols": [..], "remove": [names]}`: symbols from pins by side (a box sized to them), from a
//!   template (resistor, capacitor, diode, led), or from pins at points with shapes; parameters; footprint links.
//! - `schdoc`: `{"path", "size", "remove", "place", "set", "wires", "labels", "power", "junctions", "no_erc", "connect"}`.
//! - `project`: a .PrjPcb the written files are added to (made when missing).
//!
//! Paths are relative to the project folder. Schematic coordinates are mils (Y up), board ones mm. The result says
//! what each file now holds, read back through Backplane's own readers.
use super::fp;
use super::pcblib::{self, Footprint, Line, Pad, PadShape};
use super::schdoc::Sheet;
use super::schlib::{self, box_layout, electrical, Pin, Shape, Symbol};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn f(v: &Value, k: &str) -> Option<f64> { v.get(k).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|t| t.trim().parse().ok()))) }
fn need(v: &Value, k: &str, what: &str) -> Result<f64, String> { f(v, k).ok_or_else(|| format!("{what}: `{k}` (a number) is needed")) }
fn pair(v: &Value) -> Option<(f64, f64)> { let a = v.as_array()?; Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)) }
fn pt(v: &Value, k: &str, what: &str) -> Result<(f64, f64), String> { v.get(k).and_then(pair).ok_or_else(|| format!("{what}: `{k}` must be [x, y]")) }
fn strs(v: &Value, k: &str) -> Vec<String> { v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() }
fn quarter(v: &Value, k: &str) -> Result<u8, String> {
    let d = f(v, k).unwrap_or(0.0);
    let q = (d / 90.0).round();
    if (q * 90.0 - d).abs() > 1e-6 { return Err(format!("`{k}` must be 0, 90, 180 or 270 (got {d})")) }
    Ok((q as i64).rem_euclid(4) as u8)
}

/// A spec path inside the project folder.
fn resolve(base: &Path, p: &str, ext: &str) -> Result<PathBuf, String> {
    if p.trim().is_empty() { return Err(format!("a `path` to a .{ext} is needed")) }
    let q = Path::new(p.trim());
    let full = if q.is_absolute() { q.to_path_buf() } else { base.join(q) };
    let ok = full.extension().map_or(false, |e| e.to_string_lossy().eq_ignore_ascii_case(ext));
    if !ok { return Err(format!("{p}: must be a .{ext} file")) }
    // no climbing out of the project
    let mut norm = PathBuf::new();
    for c in full.components() { match c { std::path::Component::ParentDir => { norm.pop(); } std::path::Component::CurDir => {} c => norm.push(c) } }
    if !norm.starts_with(base) { return Err(format!("{p}: outside the project folder {}", base.display())) }
    Ok(norm)
}
fn existing(p: &Path) -> Result<Option<Vec<u8>>, String> { if p.is_file() { std::fs::read(p).map(Some).map_err(|e| format!("{}: {e}", p.display())) } else { Ok(None) } }
fn seed(p: &Path) -> String { format!("{}{}", p.display(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())) }

// Footprints ------------------------------------------------------------------------------------------------------------

fn pad_shape(w: &str) -> Result<PadShape, String> {
    Ok(match w.trim().to_ascii_lowercase().as_str() {
        "" | "rect" | "rectangle" | "square" => PadShape::Rect, "round" | "circle" | "oval" | "obround" => PadShape::Round, "oct" | "octagon" | "octagonal" => PadShape::Oct,
        "roundrect" | "rounded" | "rounded_rect" | "roundedrectangle" => PadShape::RoundRect(25),
        x => return Err(format!("pad shape {x:?}: rect, round, roundrect or oct")),
    })
}
pub fn footprint(v: &Value) -> Result<Footprint, String> {
    let pkg = s(v, "package").trim().to_ascii_lowercase();
    let name = s(v, "name").trim().to_string();
    let what = if name.is_empty() { format!("footprint ({pkg})") } else { format!("footprint {name}") };
    let h = f(v, "height").unwrap_or(0.0);
    let body = v.get("body").and_then(pair);
    let mut fpr = match pkg.as_str() {
        "chip" | "resistor" | "capacitor" | "inductor" => {
            let (l, w, t) = match v.get("size").and_then(Value::as_str) {
                Some(code) => fp::chip_size(code).ok_or_else(|| format!("{what}: unknown chip size {code:?} (0201, 0402, 0603, 0805, 1206, 1210, 1812, 2010, 2512, or give body [l, w] and terminal)"))?,
                None => { let (l, w) = body.ok_or_else(|| format!("{what}: `size` (\"0603\") or `body` [length, width] is needed"))?; (l, w, need(v, "terminal", &what)?) }
            };
            let prefix = match (s(v, "kind").to_ascii_lowercase().as_str(), pkg.as_str()) { ("capacitor", _) | (_, "capacitor") => "CAP", ("inductor", _) | (_, "inductor") => "IND", _ => "RES" };
            fp::chip(prefix, l, w, t, if h > 0.0 { h } else { (w * 0.7 * 100.0).round() / 100.0 })
        }
        "soic" | "sop" | "tssop" | "msop" | "ssop" | "gullwing" => {
            let n = need(v, "pins", &what)? as usize;
            if n < 2 || n % 2 != 0 { return Err(format!("{what}: `pins` must be even")) }
            let pitch = f(v, "pitch").unwrap_or(1.27);
            let span = need(v, "span", &what)?;
            let foot = f(v, "foot").unwrap_or(if pitch >= 1.0 { 0.835 } else { 0.6 });
            let lead = f(v, "lead").unwrap_or(if pitch >= 1.0 { 0.42 } else { pitch * 0.45 });
            let b = body.ok_or_else(|| format!("{what}: `body` [width, length] is needed"))?;
            let kind = if pkg == "gullwing" { "SOP".to_string() } else { pkg.to_ascii_uppercase() };
            let hh = if h > 0.0 { h } else { 1.75 };
            fp::gullwing(&fp::gullwing_name(&kind, pitch, span, hh, n), n / 2, n / 2, pitch, span, foot, lead, b, hh)
        }
        "sot23" | "sot-23" => {
            let n = f(v, "pins").unwrap_or(3.0) as usize;
            let (l, r, span, foot, b, hh) = match n { 3 => (2, 1, 2.37, 0.4, (1.3, 2.92), 1.12), 5 => (3, 2, 2.8, 0.45, (1.6, 2.9), 1.45), 6 => (3, 3, 2.8, 0.45, (1.6, 2.9), 1.45), _ => return Err(format!("{what}: SOT-23 has 3, 5 or 6 pins")) };
            let span = f(v, "span").unwrap_or(span);
            let hh = if h > 0.0 { h } else { hh };
            fp::gullwing(&format!("SOT95P{:.0}X{:.0}-{n}N", span * 100.0, hh * 100.0), l, r, 0.95, span, f(v, "foot").unwrap_or(foot), f(v, "lead").unwrap_or(0.4), body.unwrap_or(b), hh)
        }
        "qfn" | "dfn" | "son" => {
            let b = body.ok_or_else(|| format!("{what}: `body` [x, y] is needed"))?;
            let pitch = need(v, "pitch", &what)?;
            let (nx, ny) = match (f(v, "nx"), f(v, "ny")) {
                (Some(a), Some(c)) => (a as usize, c as usize),
                _ => { let n = need(v, "pins", &what)? as usize; if pkg == "qfn" { if n % 4 != 0 { return Err(format!("{what}: a QFN's `pins` divide by 4 (or give nx and ny)")) } (n / 4, n / 4) } else { (n / 2, 0) } }
            };
            let ep = v.get("ep").and_then(pair);
            let hh = if h > 0.0 { h } else { 0.9 };
            let n = 2 * (nx + ny) + ep.is_some() as usize;
            let kind = if ny == 0 { "SON" } else { "QFN" };
            fp::qfn(&format!("{kind}{:.0}P{:.0}X{:.0}X{:.0}-{n}N", pitch * 100.0, b.0 * 100.0, b.1 * 100.0, hh * 100.0), nx, ny, pitch, b, f(v, "term").unwrap_or(0.4), f(v, "lead").unwrap_or(pitch * 0.5), ep, hh)
        }
        "header" | "pin_header" => {
            let rows = f(v, "rows").unwrap_or(1.0) as usize;
            let cols = f(v, "cols").map(|c| c as usize).or_else(|| f(v, "pins").map(|p| (p as usize).div_ceil(rows.max(1)))).ok_or_else(|| format!("{what}: `pins` or `cols` is needed"))?;
            let pitch = f(v, "pitch").unwrap_or(2.54);
            fp::header(&format!("HDR{}X{}P{:.0}", rows, cols, pitch * 100.0), rows.max(1), cols.max(1), pitch, f(v, "drill").unwrap_or(1.0), f(v, "pad").unwrap_or(1.7), if h > 0.0 { h } else { 8.5 })
        }
        "custom" | "" => {
            let pads = v.get("pads").and_then(Value::as_array).ok_or_else(|| format!("{what}: `package` (chip, soic, sot23, qfn, dfn, header) or `pads` is needed"))?;
            let mut ps = vec![];
            for (i, p) in pads.iter().enumerate() {
                let n = p.get("number").map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).unwrap_or_else(|| (i + 1).to_string());
                let pw = need(p, "w", &format!("{what} pad {n}")).or_else(|_| need(p, "size", &format!("{what} pad {n}")))?;
                let ph = f(p, "h").unwrap_or(pw);
                let hole = f(p, "hole").unwrap_or(0.0);
                let shape = pad_shape(s(p, "shape"))?;
                let mut pad = if hole > 0.0 { let mut t = Pad::thru(&n, need(p, "x", &what)?, need(p, "y", &what)?, pw, hole, shape); t.h = ph; t } else { Pad::smd(&n, need(p, "x", &what)?, need(p, "y", &what)?, pw, ph, shape) };
                pad.rot = f(p, "rotation").unwrap_or(0.0);
                pad.slot = f(p, "slot").unwrap_or(0.0);
                pad.plated = p.get("plated").and_then(Value::as_bool).unwrap_or(true);
                if hole <= 0.0 { if let Some(l) = p.get("layer").and_then(Value::as_str) { pad.layer = match pcblib::layer(l) { Some(pcblib::BOTTOM) => pcblib::BOTTOM, Some(pcblib::TOP) => pcblib::TOP, _ => return Err(format!("{what} pad {n}: a surface pad is on top or bottom")) } } }
                ps.push(pad);
            }
            if ps.is_empty() { return Err(format!("{what}: no pads")) }
            fp::custom(if name.is_empty() { "FOOTPRINT" } else { &name }, ps, body, h)
        }
        x => return Err(format!("{what}: unknown package {x:?}: chip, soic, sop, tssop, msop, sot23, qfn, dfn, header or custom")),
    };
    if !name.is_empty() { fpr.name = name }
    if let Some(d) = v.get("description").and_then(Value::as_str) { fpr.description = d.into() }
    // extra drawing: lines on any layer
    for l in v.get("lines").and_then(Value::as_array).into_iter().flatten() {
        let layer = pcblib::layer(s(l, "layer")).ok_or_else(|| format!("{}: line layer {:?} (top_overlay, mech13, ...)", fpr.name, s(l, "layer")))?;
        fpr.lines.push(Line { layer, a: pt(l, "from", &fpr.name)?, b: pt(l, "to", &fpr.name)?, w: f(l, "width").unwrap_or(0.15) });
    }
    if fpr.name.len() > 255 { return Err("footprint name too long".into()) }
    Ok(fpr)
}

// Symbols ---------------------------------------------------------------------------------------------------------------

fn side(w: &str) -> Result<usize, String> {
    Ok(match w.trim().to_ascii_lowercase().as_str() { "left" | "l" | "w" | "west" => 0, "right" | "r" | "e" | "east" => 1, "top" | "t" | "n" | "up" => 2, "bottom" | "b" | "s" | "down" => 3, x => return Err(format!("pin side {x:?}: left, right, top or bottom")) })
}
fn pin_of(p: &Value, i: usize, what: &str) -> Result<Pin, String> {
    let num = p.get("number").map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).unwrap_or_else(|| (i + 1).to_string());
    let name = p.get("name").and_then(Value::as_str).unwrap_or(&num).to_string();
    let mut pin = Pin::new(&num, &name, 0.0, 0.0, 0);
    if let Some(t) = p.get("type").and_then(Value::as_str) { pin.electrical = electrical(t).ok_or_else(|| format!("{what} pin {num}: type {t:?}: {}", schlib::ELECTRICAL.join(", ")))? }
    pin.hidden = p.get("hidden").and_then(Value::as_bool).unwrap_or(false);
    pin.show_name = p.get("show_name").and_then(Value::as_bool).unwrap_or(true);
    pin.part = f(p, "part").unwrap_or(1.0) as i64;
    pin.description = s(p, "description").into();
    if p.get("inverted").and_then(Value::as_bool).unwrap_or(false) { pin.outer_edge = 1 }
    if p.get("clock").and_then(Value::as_bool).unwrap_or(false) { pin.inner_edge = 3 }
    Ok(pin)
}
/// A symbol from its spec.
pub fn symbol(v: &Value, fp_lib: &str) -> Result<Symbol, String> {
    let name = s(v, "name").trim().to_string();
    if name.is_empty() { return Err("a symbol needs a `name`".into()) }
    let what = format!("symbol {name}");
    let len = f(v, "pin_length").unwrap_or(200.0);
    let mut sym = Symbol { name: name.clone(), description: s(v, "description").into(), designator: s(v, "designator").into(), comment: s(v, "comment").into(), parts: 1, ..Default::default() };
    if sym.comment.is_empty() { sym.comment = name.clone() }
    let template = s(v, "template").trim().to_ascii_lowercase();
    if !template.is_empty() {
        let two = |sym: &mut Symbol| { let mut a = Pin::new("1", "1", 0.0, 0.0, 2); a.len = 100.0; a.show_name = false; a.show_number = false; a.x = 100.0; let mut b = Pin::new("2", "2", 0.0, 0.0, 0); b.len = 100.0; b.show_name = false; b.show_number = false; b.x = 300.0; sym.pins = vec![a, b]; };
        two(&mut sym);
        let (shapes, des): (Vec<Shape>, &str) = match template.as_str() {
            "resistor" => (vec![Shape::Rect { x1: 100.0, y1: -40.0, x2: 300.0, y2: 40.0, filled: false }], "R?"),
            "capacitor" => (vec![Shape::Line { pts: vec![(180.0, -60.0), (180.0, 60.0)] }, Shape::Line { pts: vec![(220.0, -60.0), (220.0, 60.0)] }, Shape::Line { pts: vec![(100.0, 0.0), (180.0, 0.0)] }, Shape::Line { pts: vec![(220.0, 0.0), (300.0, 0.0)] }], "C?"),
            "inductor" => ((0..4).map(|i| Shape::Arc { cx: 125.0 + 50.0 * i as f64, cy: 0.0, r: 25.0, a0: 0.0, a1: 180.0 }).collect(), "L?"),
            "diode" | "led" => {
                let mut v = vec![Shape::Polygon { pts: vec![(160.0, 50.0), (160.0, -50.0), (240.0, 0.0)], filled: true }, Shape::Line { pts: vec![(240.0, 50.0), (240.0, -50.0)] }, Shape::Line { pts: vec![(100.0, 0.0), (300.0, 0.0)] }];
                if template == "led" { v.push(Shape::Line { pts: vec![(200.0, 70.0), (240.0, 110.0)] }); v.push(Shape::Line { pts: vec![(230.0, 60.0), (270.0, 100.0)] }); }
                sym.pins[0].name = "A".into(); sym.pins[1].name = "K".into();
                (v, if template == "led" { "LED?" } else { "D?" })
            }
            x => return Err(format!("{what}: unknown template {x:?}: resistor, capacitor, inductor, diode, led")),
        };
        sym.shapes = shapes.into_iter().map(|s| (s, 1)).collect();
        if sym.designator.is_empty() { sym.designator = des.into() }
    } else {
        let pins = v.get("pins").and_then(Value::as_array).ok_or_else(|| format!("{what}: `pins` (or a `template`) is needed"))?;
        let placed = pins.iter().any(|p| p.get("at").is_some());
        if placed {
            // pins at given ends, pointing the way given; shapes drawn as given
            for (i, p) in pins.iter().enumerate() {
                let mut pin = pin_of(p, i, &what)?;
                let (ex, ey) = pt(p, "at", &what)?;
                pin.dir = match s(p, "points").to_ascii_lowercase().as_str() { "right" => 0, "up" => 1, "left" | "" => 2, "down" => 3, x => return Err(format!("{what} pin {}: `points` {x:?}: the way it points out of the body: left, right, up, down", pin.number)) };
                pin.len = f(p, "length").unwrap_or(len);
                let (dx, dy) = schlib::DIRS[pin.dir as usize];
                pin.x = ex - dx * pin.len; pin.y = ey - dy * pin.len;
                sym.pins.push(pin);
            }
            for sh in v.get("shapes").and_then(Value::as_array).into_iter().flatten() { sym.shapes.push((shape(sh, &what)?, f(sh, "part").unwrap_or(1.0) as i64)) }
        } else {
            // pins by side, per part: a box sized to them
            let parts = pins.iter().filter_map(|p| f(p, "part")).fold(1.0f64, f64::max) as i64;
            for part in 1..=parts {
                let mut sides: [Vec<Option<Pin>>; 4] = Default::default();
                for (i, p) in pins.iter().enumerate() {
                    if f(p, "part").unwrap_or(1.0) as i64 != part { continue }
                    let sd = side(p.get("side").and_then(Value::as_str).unwrap_or("left"))?;
                    if p.get("gap").and_then(Value::as_bool).unwrap_or(false) { sides[sd].push(None); continue }
                    sides[sd].push(Some(pin_of(p, i, &what)?));
                }
                let (ps, body) = box_layout(sides, len);
                sym.pins.extend(ps);
                sym.shapes.push((body, part));
            }
            sym.parts = parts as u32;
        }
        if sym.designator.is_empty() { sym.designator = "U?".into() }
    }
    if let Some(ps) = v.get("parameters").and_then(Value::as_object) { for (k, x) in ps { sym.params.push((k.clone(), x.as_str().map(String::from).unwrap_or_else(|| x.to_string()))) } }
    let fps: Vec<String> = match v.get("footprint") { Some(Value::String(x)) => vec![x.clone()], Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(), _ => vec![] };
    let lib = v.get("footprint_library").and_then(Value::as_str).unwrap_or(fp_lib);
    sym.footprints = fps.into_iter().map(|n| schlib::Footprint { name: n, library: lib.to_string(), description: String::new() }).collect();
    // pins must be distinct and on the 100 mil grid at their ends (Altium's wires snap to it)
    let mut seen = std::collections::HashSet::new();
    for p in &sym.pins { if !seen.insert((p.part, p.number.clone())) { return Err(format!("{what}: pin number {} is used twice", p.number)) } }
    Ok(sym)
}
fn shape(v: &Value, what: &str) -> Result<Shape, String> {
    let pts = |k: &str| -> Result<Vec<(f64, f64)>, String> { v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(pair).collect()).filter(|p: &Vec<(f64, f64)>| p.len() >= 2).ok_or_else(|| format!("{what}: shape `{k}` is a list of [x, y] (two or more)")) };
    let filled = v.get("filled").and_then(Value::as_bool).unwrap_or(false);
    Ok(match s(v, "kind").to_ascii_lowercase().as_str() {
        "rect" | "rectangle" => { let (a, b) = (pt(v, "from", what)?, pt(v, "to", what)?); Shape::Rect { x1: a.0, y1: a.1, x2: b.0, y2: b.1, filled } }
        "line" | "polyline" => Shape::Line { pts: pts("points")? },
        "polygon" => Shape::Polygon { pts: pts("points")?, filled },
        "arc" | "circle" => { let c = pt(v, "center", what)?; Shape::Arc { cx: c.0, cy: c.1, r: need(v, "radius", what)?, a0: f(v, "start").unwrap_or(0.0), a1: f(v, "end").unwrap_or(360.0) } }
        "ellipse" => { let c = pt(v, "center", what)?; Shape::Ellipse { cx: c.0, cy: c.1, rx: need(v, "rx", what)?, ry: need(v, "ry", what)?, filled } }
        "text" => { let a = pt(v, "at", what)?; Shape::Text { x: a.0, y: a.1, text: s(v, "text").into() } }
        x => return Err(format!("{what}: shape kind {x:?}: rect, line, polygon, arc, ellipse, text")),
    })
}

// The run ---------------------------------------------------------------------------------------------------------------

/// The paths a spec writes (as given, project-relative), for locks and snapshots before it runs.
pub fn targets(spec: &Value) -> Vec<String> {
    let mut v: Vec<String> = ["pcblib", "schlib", "schdoc"].iter().filter_map(|k| spec.get(*k)).map(|p| s(p, "path").to_string()).filter(|p| !p.is_empty()).collect();
    if let Some(p) = spec.get("project").and_then(Value::as_str) { v.push(p.to_string()) }
    v
}

/// The files a spec writes, resolved in the project folder (those that do not resolve are left to `run` to refuse).
pub fn target_paths(spec: &Value, base: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = [("pcblib", "PcbLib"), ("schlib", "SchLib"), ("schdoc", "SchDoc")].iter().filter_map(|(k, e)| spec.get(*k).and_then(|p| resolve(base, s(p, "path"), e).ok())).collect();
    if let Some(p) = spec.get("project").and_then(Value::as_str).and_then(|p| resolve(base, p, "PrjPcb").ok()) { v.push(p) }
    v
}
/// The documents a run added to a project: (the project's path, the added documents' paths).
pub fn project_added(result: &Value) -> Vec<(String, Vec<String>)> {
    let Some(p) = result.get("project") else { return vec![] };
    let prj = s(p, "path");
    let dir = Path::new(prj).parent().unwrap_or(Path::new(""));
    let docs: Vec<String> = strs(p, "added").iter().map(|r| dir.join(r.replace('\\', std::path::MAIN_SEPARATOR_STR)).to_string_lossy().into_owned()).collect();
    if docs.is_empty() { vec![] } else { vec![(prj.to_string(), docs)] }
}

/// Runs a spec against the project folder `base`. Ok: what was written (each file read back). The files written, for the
/// caller's locks and commits, are in `written`.
pub fn run(spec: &Value, base: &Path) -> Result<(Value, Vec<PathBuf>), String> {
    let mut out = serde_json::Map::new();
    let mut written: Vec<PathBuf> = vec![];
    let mut fp_lib_name = String::new();
    if let Some(p) = spec.get("pcblib") {
        let path = resolve(base, s(p, "path"), "PcbLib")?;
        let fps = p.get("footprints").and_then(Value::as_array).map(|a| a.iter().map(footprint).collect::<Result<Vec<_>, _>>()).transpose()?.unwrap_or_default();
        let old = existing(&path)?;
        let bytes = pcblib::write(old.as_deref(), &fps, &strs(p, "remove"), &seed(&path))?;
        let back = pcblib::read(&bytes)?;
        super::save(&path.to_string_lossy(), &bytes)?;
        fp_lib_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        out.insert("pcblib".into(), json!({"path": path.to_string_lossy(), "made": old.is_none(), "footprints": back.iter().map(|f| {
            let pads = f.pads();
            let made = fps.iter().any(|x| x.name == f.name);
            let ext = pads.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, p| (b.0.min(p.1 - p.3 / 2.0), b.1.min(p.2 - p.4 / 2.0), b.2.max(p.1 + p.3 / 2.0), b.3.max(p.2 + p.4 / 2.0)));
            if made { json!({"name": f.name, "written": true, "description": f.description, "pads": pads.iter().map(|p| json!({"number": p.0, "x": r3(p.1), "y": r3(p.2), "w": r3(p.3), "h": r3(p.4), "hole": r3(p.5)})).collect::<Vec<_>>(),
                "copper_extent_mm": [r3(ext.0), r3(ext.1), r3(ext.2), r3(ext.3)], "height_mm": r3(f.height)}) } else { json!({"name": f.name}) }
        }).collect::<Vec<_>>()}));
        written.push(path);
    }
    if let Some(p) = spec.get("schlib") {
        let path = resolve(base, s(p, "path"), "SchLib")?;
        let syms = p.get("symbols").and_then(Value::as_array).map(|a| a.iter().map(|v| symbol(v, &fp_lib_name)).collect::<Result<Vec<_>, _>>()).transpose()?.unwrap_or_default();
        let old = existing(&path)?;
        let bytes = schlib::write(old.as_deref(), &syms, &strs(p, "remove"), &seed(&path))?;
        let back = schlib::read(&bytes)?;
        super::save(&path.to_string_lossy(), &bytes)?;
        out.insert("schlib".into(), json!({"path": path.to_string_lossy(), "made": old.is_none(), "symbols": back.iter().map(|l| {
            if syms.iter().any(|x| x.name == l.name) {
                json!({"name": l.name, "written": true, "designator": l.designator(), "footprints": l.footprints().iter().map(|f| f.0.clone()).collect::<Vec<_>>(),
                    "pins": l.pins().iter().map(|p| { let e = p.end(); json!({"number": p.number, "name": p.name, "end_mils": [e.0, e.1], "part": p.part}) }).collect::<Vec<_>>()})
            } else { json!({"name": l.name}) }
        }).collect::<Vec<_>>()}));
        written.push(path);
    }
    if let Some(p) = spec.get("schdoc") {
        let path = resolve(base, s(p, "path"), "SchDoc")?;
        let old = existing(&path)?;
        let mut sh = match &old { Some(b) => Sheet::read(b, &seed(&path))?, None => Sheet::new(p.get("size").and_then(Value::as_str).unwrap_or("A4"), &seed(&path))? };
        if let Some(r) = p.get("remove") {
            for d in strs(r, "parts") { sh.remove_part(&d)? }
            for n in strs(r, "nets") { sh.remove_objects(Some(&n), None); }
            for a in r.get("at").and_then(Value::as_array).into_iter().flatten().filter_map(pair) { sh.remove_objects(None, Some(a)); }
        }
        let mut libs: std::collections::HashMap<PathBuf, Vec<schlib::LibSym>> = std::collections::HashMap::new();
        for (i, pl) in p.get("place").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let what = format!("place[{i}]");
            let lp = resolve(base, s(pl, "library"), "SchLib").map_err(|e| format!("{what}: {e}"))?;
            if !libs.contains_key(&lp) { let b = std::fs::read(&lp).map_err(|e| format!("{what}: {}: {e}", lp.display()))?; libs.insert(lp.clone(), schlib::read(&b)?); }
            let want = s(pl, "symbol");
            let sym = libs[&lp].iter().find(|x| x.name.eq_ignore_ascii_case(want)).ok_or_else(|| format!("{what}: {} has no symbol {want:?} (it has: {})", lp.display(), libs[&lp].iter().map(|x| x.name.as_str()).collect::<Vec<_>>().join(", ")))?;
            let lib_name = lp.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let des = s(pl, "designator");
            sh.place(sym, &lib_name, des, pt(pl, "at", &what)?, quarter(pl, "rotation")?, pl.get("mirror").and_then(Value::as_bool).unwrap_or(false), f(pl, "part").unwrap_or(1.0) as i64).map_err(|e| format!("{what}: {e}"))?;
            if let Some(c) = pl.get("comment").and_then(Value::as_str) { sh.set_param(des, "Comment", c, None)? }
            if let Some(ps) = pl.get("parameters").and_then(Value::as_object) { for (k, x) in ps { sh.set_param(des, k, &x.as_str().map(String::from).unwrap_or_else(|| x.to_string()), None)? } }
        }
        for st in p.get("set").and_then(Value::as_array).into_iter().flatten() { sh.set_param(s(st, "part"), s(st, "name"), s(st, "value"), st.get("visible").and_then(Value::as_bool))? }
        for w in p.get("wires").and_then(Value::as_array).into_iter().flatten() { let ps: Vec<(f64, f64)> = w.as_array().map(|a| a.iter().filter_map(pair).collect()).unwrap_or_default(); sh.wire(&ps)?; }
        for l in p.get("labels").and_then(Value::as_array).into_iter().flatten() { sh.label(s(l, "net"), pt(l, "at", "label")?, quarter(l, "rotation")?); }
        for w in p.get("power").and_then(Value::as_array).into_iter().flatten() {
            let net = s(w, "net");
            let style = w.get("style").and_then(Value::as_str).map(String::from).unwrap_or_else(|| if is_ground(net) { "ground".into() } else { "bar".into() });
            let rot = if w.get("rotation").is_some() { quarter(w, "rotation")? } else if is_ground(net) { 3 } else { 1 };
            sh.power(net, &style, pt(w, "at", "power")?, rot)?;
        }
        for j in p.get("junctions").and_then(Value::as_array).into_iter().flatten().filter_map(pair) { sh.junction(j); }
        for j in p.get("no_erc").and_then(Value::as_array).into_iter().flatten().filter_map(pair) { sh.no_erc(j); }
        for (i, c) in p.get("connect").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let pin = s(c, "pin");
            let (d, n) = pin.split_once('.').ok_or_else(|| format!("connect[{i}]: `pin` is \"U1.3\" (designator.pin number or name)"))?;
            let net = s(c, "net");
            if net.is_empty() { return Err(format!("connect[{i}]: `net` is needed")) }
            let power = match c.get("power") { Some(Value::String(x)) => Some(x.clone()), Some(Value::Bool(true)) => Some(if is_ground(net) { "ground".into() } else { "bar".into() }), Some(Value::Bool(false)) => None, _ => is_ground(net).then(|| "ground".to_string()) };
            sh.connect(d, n, net, power.as_deref(), f(c, "stub").unwrap_or(100.0)).map_err(|e| format!("connect[{i}]: {e}"))?;
        }
        let bytes = sh.bytes()?;
        let parts = super::super::sch::parts(&bytes);
        let nets = super::super::sch::nets(&bytes);
        super::save(&path.to_string_lossy(), &bytes)?;
        out.insert("schdoc".into(), json!({"path": path.to_string_lossy(), "made": old.is_none(), "parts": parts.iter().map(|p| json!({"designator": p.refdes, "symbol": p.lib_ref, "comment": p.value, "footprint": p.footprint})).collect::<Vec<_>>(), "nets": nets}));
        written.push(path);
    }
    if let Some(pj) = spec.get("project").and_then(Value::as_str) {
        let path = resolve(base, pj, "PrjPcb")?;
        let added = add_to_project(&path, &written)?;
        out.insert("project".into(), json!({"path": path.to_string_lossy(), "added": added}));
        written.push(path);
    }
    if out.is_empty() { return Err("nothing to do: give `pcblib`, `schlib`, `schdoc` or `project`".into()) }
    Ok((Value::Object(out), written))
}
fn r3(v: f64) -> f64 { (v * 1000.0).round() / 1000.0 }
fn is_ground(net: &str) -> bool { let n = net.to_ascii_uppercase(); n.contains("GND") || n == "VSS" || n == "0V" || n.ends_with("_GND") }

/// Adds documents to a project file (made when missing), relative to its folder with Windows separators. Returns the added.
pub fn add_to_project(prj: &Path, docs: &[PathBuf]) -> Result<Vec<String>, String> {
    let raw = std::fs::read(prj).ok();
    let text = raw.as_ref().map(|b| String::from_utf8_lossy(b.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(b)).into_owned()).unwrap_or_else(|| "[Design]\r\nVersion=1.0\r\nHierarchyMode=0\r\nChannelRoomNamingStyle=0\r\nOutputPath=Project Outputs\r\n".into());
    let have: Vec<String> = super::super::prj::documents(&text).iter().map(|d| d.to_lowercase()).collect();
    let dir = prj.parent().unwrap_or(Path::new(""));
    let mut n = text.lines().filter_map(|l| l.trim().strip_prefix("[Document").and_then(|r| r.strip_suffix(']')).and_then(|d| d.parse::<u32>().ok())).max().unwrap_or(0);
    let mut add = String::new();
    let mut added = vec![];
    for d in docs {
        let Ok(rel) = d.strip_prefix(dir) else { continue };
        let rel = rel.to_string_lossy().replace('/', "\\");
        if have.contains(&rel.replace('\\', "/").to_lowercase()) || added.contains(&rel) { continue }
        n += 1;
        add.push_str(&format!("\r\n[Document{n}]\r\nDocumentPath={rel}\r\nAnnotationEnabled=1\r\nAnnotateStartValue=1\r\nAnnotationIndexControlEnabled=0\r\nAnnotateSuffix=\r\nAnnotateScope=All\r\nAnnotateOrder=-1\r\nDoLibraryUpdate=1\r\nDoDatabaseUpdate=1\r\nClassGenCCAutoEnabled=1\r\nClassGenCCAutoRoomEnabled=1\r\nClassGenNCAutoScope=None\r\nDItemRevisionGUID=\r\nGenerateClassCluster=0\r\n"));
        added.push(rel);
    }
    if added.is_empty() && raw.is_some() { return Ok(added) }
    // the new sections go after the last document section (else after the first section); the rest stays as it was
    let lines: Vec<&str> = text.lines().collect();
    let starts: Vec<usize> = (0..lines.len()).filter(|&i| lines[i].trim_start().starts_with('[')).collect();
    let is_doc = |i: usize| lines[i].trim().to_ascii_lowercase().strip_prefix("[document").map_or(false, |r| r.trim_end_matches(']').chars().all(|c| c.is_ascii_digit()) && r.len() > 1);
    let after = starts.iter().rev().find(|&&i| is_doc(i)).or(starts.first()).copied();
    let cut = match after { Some(i) => starts.iter().find(|&&j| j > i).copied().unwrap_or(lines.len()), None => lines.len() };
    let mut head: Vec<&str> = lines[..cut].to_vec();
    while head.last().map_or(false, |l| l.trim().is_empty()) { head.pop(); }
    let mut body = head.join("\r\n");
    body.push_str(&add);
    if cut < lines.len() { body.push_str("\r\n"); body.push_str(&lines[cut..].join("\r\n")); }
    body.push_str("\r\n");
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend(body.into_bytes());
    super::save(&prj.to_string_lossy(), &bytes)?;
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp() -> crate::plat::tmp::TempDir { let d = crate::plat::tmp::dir("altw"); std::fs::create_dir_all(&d).unwrap(); d }
    #[test] fn whole_spec() {
        let t = tmp();
        let base = t.path().to_path_buf();
        let spec = json!({
            "pcblib": {"path": "lib/Parts.PcbLib", "footprints": [
                {"package": "sot23", "pins": 5},
                {"package": "chip", "size": "0603", "kind": "resistor"},
                {"name": "PADS2", "package": "custom", "pads": [{"number": "1", "x": -1, "y": 0, "w": 1, "h": 1.2, "shape": "roundrect"}, {"number": "2", "x": 1, "y": 0, "w": 1, "h": 1.2}], "body": [3, 2], "height": 1}
            ]},
            "schlib": {"path": "lib/Parts.SchLib", "symbols": [
                {"name": "LDO_5", "footprint": "SOT95P280X145-5N", "parameters": {"Manufacturer": "Acme"}, "pins": [
                    {"number": 1, "name": "VIN", "side": "left", "type": "power"}, {"number": 3, "name": "EN", "side": "left", "type": "input"},
                    {"number": 5, "name": "VOUT", "side": "right", "type": "power"}, {"number": 2, "name": "GND", "side": "bottom", "type": "power"}, {"number": 4, "name": "NC", "side": "right", "type": "passive"}]},
                {"name": "RES", "template": "resistor", "footprint": "RESC1608X56N"}
            ]},
            "schdoc": {"path": "Power.SchDoc", "size": "A4", "place": [
                {"symbol": "LDO_5", "library": "lib/Parts.SchLib", "designator": "U1", "at": [2000, 4000]},
                {"symbol": "RES", "library": "lib/Parts.SchLib", "designator": "R1", "at": [4000, 4000], "rotation": 90, "comment": "10k"}
            ], "connect": [
                {"pin": "U1.VIN", "net": "VBAT"}, {"pin": "U1.EN", "net": "VBAT"}, {"pin": "U1.GND", "net": "GND"}, {"pin": "U1.5", "net": "3V3", "power": "bar"},
                {"pin": "U1.4", "net": "NC"}, {"pin": "R1.1", "net": "3V3", "power": true}, {"pin": "R1.2", "net": "LED_A"}
            ]},
            "project": "Power.PrjPcb"
        });
        let (r, written) = run(&spec, &base).unwrap();
        assert_eq!(written.len(), 4);
        let fps = r["pcblib"]["footprints"].as_array().unwrap();
        assert_eq!(fps.iter().map(|f| f["name"].as_str().unwrap()).collect::<Vec<_>>(), vec!["SOT95P280X145-5N", "RESC1608X56N", "PADS2"]);
        assert_eq!(fps[0]["pads"].as_array().unwrap().len(), 5);
        let syms = r["schlib"]["symbols"].as_array().unwrap();
        assert_eq!(syms[0]["footprints"], json!(["SOT95P280X145-5N"]));
        let parts = r["schdoc"]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1]["comment"], "10k");
        let nets: Vec<&str> = r["schdoc"]["nets"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        for n in ["VBAT", "GND", "3V3", "LED_A"] { assert!(nets.contains(&n), "{n} in {nets:?}") }
        let prj = std::fs::read_to_string(base.join("Power.PrjPcb")).unwrap();
        assert_eq!(super::super::super::prj::documents(&prj), vec!["lib/Parts.PcbLib", "lib/Parts.SchLib", "Power.SchDoc"]);
        // a second run edits: replaces a symbol, adds a part, nothing duplicated in the project
        let (r2, _) = run(&json!({"schdoc": {"path": "Power.SchDoc", "remove": {"parts": ["R1"]}, "place": [{"symbol": "RES", "library": "lib/Parts.SchLib", "designator": "R2", "at": [5000, 4000]}]}, "project": "Power.PrjPcb"}), &base).unwrap();
        assert_eq!(r2["schdoc"]["parts"].as_array().unwrap().iter().map(|p| p["designator"].as_str().unwrap()).collect::<Vec<_>>(), vec!["U1", "R2"]);
        assert_eq!(r2["project"]["added"], json!([]));
        // errors name the problem
        let e = run(&json!({"schdoc": {"path": "Power.SchDoc", "connect": [{"pin": "U9.1", "net": "X"}]}}), &base).unwrap_err();
        assert!(e.contains("U9"), "{e}");
        assert!(run(&json!({"schlib": {"path": "../x.SchLib"}}), &base).unwrap_err().contains("outside"));
    }
    /// BP_ALTIUM_WRITE_OUT=<dir>: writes a demo project there (every package, template and sheet feature) to open in Altium.
    #[test] fn demo_project() {
        let Some(dir) = std::env::var_os("BP_ALTIUM_WRITE_OUT").map(PathBuf::from) else { return };
        std::fs::create_dir_all(&dir).unwrap();
        let spec = json!({
            "pcblib": {"path": "Demo.PcbLib", "footprints": [
                {"package": "chip", "size": "0603", "kind": "resistor"}, {"package": "chip", "size": "0402", "kind": "capacitor"},
                {"package": "soic", "pins": 8, "pitch": 1.27, "span": 6.0, "body": [3.9, 4.9], "height": 1.75},
                {"package": "sot23", "pins": 5}, {"package": "sot23", "pins": 3},
                {"package": "qfn", "pins": 16, "pitch": 0.5, "body": [3, 3], "ep": [1.7, 1.7]},
                {"package": "header", "rows": 1, "pins": 4},
                {"name": "TP_SLOT", "package": "custom", "pads": [{"number": "1", "x": 0, "y": 0, "w": 2.2, "h": 1.4, "hole": 0.8, "slot": 1.6, "shape": "round"}, {"number": "2", "x": 3, "y": 0, "w": 1.5, "hole": 0.9, "shape": "rect"}], "body": [5.5, 2.5], "height": 3}
            ]},
            "schlib": {"path": "Demo.SchLib", "symbols": [
                {"name": "LDO_5", "description": "5-pin LDO", "footprint": "SOT95P280X145-5N", "parameters": {"Manufacturer": "Acme", "Manufacturer Part Number": "ACME-LDO-33"}, "pins": [
                    {"number": 1, "name": "VIN", "side": "left", "type": "power"}, {"number": 3, "name": "EN", "side": "left", "type": "input"},
                    {"number": 5, "name": "VOUT", "side": "right", "type": "power"}, {"number": 4, "name": "NC", "side": "right"}, {"number": 2, "name": "GND", "side": "bottom", "type": "power"}]},
                {"name": "MCU_DUAL", "description": "two-part test", "footprint": "QFN50P300X300X90-17N", "pins": [
                    {"number": 1, "name": "PA0", "side": "left", "type": "io"}, {"number": 2, "name": "PA1", "side": "left", "type": "io"}, {"number": 3, "name": "RST", "side": "left", "type": "input", "inverted": true},
                    {"number": 4, "name": "VDD", "side": "top", "type": "power", "part": 2}, {"number": 17, "name": "VSS", "side": "bottom", "type": "power", "part": 2}]},
                {"name": "RES", "template": "resistor", "footprint": "RESC1608X56N"}, {"name": "CAP", "template": "capacitor", "footprint": "CAPC1005X35N"},
                {"name": "LED", "template": "led", "footprint": "RESC1608X56N"},
                {"name": "CONN4", "footprint": "HDR1X4P254", "designator": "J?", "pins": [{"number": 1, "at": [0, 0], "points": "left"}, {"number": 2, "at": [0, -100], "points": "left"}, {"number": 3, "at": [0, -200], "points": "left"}, {"number": 4, "at": [0, -300], "points": "left"}],
                 "shapes": [{"kind": "rect", "from": [200, 100], "to": [500, -400], "filled": true}, {"kind": "text", "at": [250, 0], "text": "CONN"}, {"kind": "arc", "center": [450, -350], "radius": 20}]}
            ]},
            "schdoc": {"path": "Demo.SchDoc", "size": "A3", "place": [
                {"symbol": "LDO_5", "library": "Demo.SchLib", "designator": "U1", "at": [2000, 5000]},
                {"symbol": "RES", "library": "Demo.SchLib", "designator": "R1", "at": [4000, 5000], "rotation": 90, "comment": "10k"},
                {"symbol": "LED", "library": "Demo.SchLib", "designator": "D1", "at": [4000, 4000], "rotation": 270},
                {"symbol": "CAP", "library": "Demo.SchLib", "designator": "C1", "at": [1000, 4500], "rotation": 90, "comment": "1u"},
                {"symbol": "MCU_DUAL", "library": "Demo.SchLib", "designator": "U2", "at": [6000, 5000]},
                {"symbol": "MCU_DUAL", "library": "Demo.SchLib", "designator": "U2", "at": [6000, 3000], "part": 2},
                {"symbol": "CONN4", "library": "Demo.SchLib", "designator": "J1", "at": [8000, 5000], "mirror": true}
            ], "connect": [
                {"pin": "U1.VIN", "net": "VBAT"}, {"pin": "U1.EN", "net": "VBAT"}, {"pin": "U1.GND", "net": "GND"}, {"pin": "U1.VOUT", "net": "3V3", "power": "bar"},
                {"pin": "U1.NC", "net": "NC"}, {"pin": "R1.1", "net": "3V3", "power": true}, {"pin": "R1.2", "net": "LED_A"}, {"pin": "D1.A", "net": "LED_A"}, {"pin": "D1.K", "net": "GND"},
                {"pin": "C1.1", "net": "GND"}, {"pin": "C1.2", "net": "VBAT"}, {"pin": "U2.PA0", "net": "SDA"}, {"pin": "U2.PA1", "net": "SCL"}, {"pin": "U2.RST", "net": "3V3", "power": "bar"},
                {"pin": "U2.VDD", "net": "3V3", "power": "bar"}, {"pin": "U2.VSS", "net": "GND"},
                {"pin": "J1.1", "net": "VBAT"}, {"pin": "J1.2", "net": "SDA"}, {"pin": "J1.3", "net": "SCL"}, {"pin": "J1.4", "net": "GND"}
            ]},
            "project": "Demo.PrjPcb"
        });
        let (r, _) = run(&spec, &dir).unwrap();
        println!("{}", serde_json::to_string_pretty(&r).unwrap());
        // pictures through the viewers' readers
        let sch = std::fs::read(dir.join("Demo.SchDoc")).unwrap();
        crate::files::altium::debug_svg(&crate::files::altium::sch::items(&sch).unwrap(), 1, &dir.join("Demo.SchDoc.svg").to_string_lossy());
        for f in pcblib::read(&std::fs::read(dir.join("Demo.PcbLib")).unwrap()).unwrap() { crate::files::altium::debug_svg(&f.items(), 0, &dir.join(format!("{}.svg", f.name)).to_string_lossy()) }
    }
    /// BP_ALTIUM_SPEC=<spec.json> BP_ALTIUM_WRITE_OUT=<dir>: runs that spec in that folder and prints the result (trying specs by hand).
    #[test] fn spec_file() {
        let (Some(f), Some(dir)) = (std::env::var_os("BP_ALTIUM_SPEC"), std::env::var_os("BP_ALTIUM_WRITE_OUT")) else { return };
        let spec: Value = serde_json::from_slice(&std::fs::read(f).unwrap()).unwrap();
        match run(&spec, Path::new(&dir)) { Ok((r, _)) => println!("{}", serde_json::to_string_pretty(&r).unwrap()), Err(e) => println!("ERROR {e}") }
    }
    #[test] fn project_edit_keeps_the_rest() {
        let t = tmp();
        let base = t.path().to_path_buf();
        let prj = base.join("P.PrjPcb");
        std::fs::write(&prj, "\u{feff}[Design]\r\nVersion=1.0\r\n\r\n[Document1]\r\nDocumentPath=A.SchDoc\r\n\r\n[Parameter1]\r\nName=X\r\nValue=1\r\n").unwrap();
        let added = add_to_project(&prj, &[base.join("lib").join("B.SchLib"), base.join("A.SchDoc")]).unwrap();
        assert_eq!(added, vec!["lib\\B.SchLib"]);
        let t = std::fs::read_to_string(&prj).unwrap();
        assert_eq!(super::super::super::prj::documents(&t), vec!["A.SchDoc", "lib/B.SchLib"]);
        assert_eq!(super::super::super::prj::parameters(&t), vec![("X".to_string(), "1".to_string())]);
        assert!(t.find("[Document2]").unwrap() < t.find("[Parameter1]").unwrap());
    }
}
