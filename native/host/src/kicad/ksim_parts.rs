//! What `kicad_sim` (docs/kicad-sim.md) knows without a KiCad: component values and the E series, the search a `tune` runs, SPICE model
//! files (their `.subckt` and `.model` names and pin order, where a part's model is found on this machine, how a fetched one is kept
//! with its source) and a simulation's problems as one line each with what to do next. Pure, tested here.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

// ---- values ----------------------------------------------------------------------------------------------------------------

/// A component value or a SPICE number as a number: `4.7k`, `100n`, `1Meg`, `2u2`, `4k7`, `0R1`, `10kOhm`, `100nF`, `1e-6`, `3.3V`.
/// `M` is mega and `m` milli, as KiCad reads a value field; `meg` in any case is mega, as SPICE writes it.
pub fn parse_value(text: &str) -> Option<f64> {
    let t = text.trim().replace('µ', "u").replace('μ', "u").replace('Ω', "R").replace(',', ".");
    if t.is_empty() { return None }
    if let Ok(v) = t.parse::<f64>() { return v.is_finite().then_some(v) }
    // the number, then what follows it
    let b = t.as_bytes(); let mut i = 0;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') { i += 1 }
    let digits = i; while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') { i += 1 }
    if i == digits { return None }
    // an exponent only when digits follow it (`1e3`, never the `E` of a unit)
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') { let mut j = i + 1; if j < b.len() && (b[j] == b'-' || b[j] == b'+') { j += 1 } let k = j; while j < b.len() && b[j].is_ascii_digit() { j += 1 } if j > k { i = j } }
    let head: f64 = t[..i].parse().ok()?; let rest = &t[i..];
    if rest.is_empty() { return Some(head) }
    let (mult, used) = if rest.len() >= 3 && rest[..3].eq_ignore_ascii_case("meg") { (1e6, 3) } else { match rest.as_bytes()[0] {
        b'f' => (1e-15, 1), b'p' | b'P' => (1e-12, 1), b'n' | b'N' => (1e-9, 1), b'u' | b'U' => (1e-6, 1), b'm' => (1e-3, 1), b'k' | b'K' => (1e3, 1), b'M' => (1e6, 1), b'G' | b'g' => (1e9, 1), b'T' | b't' => (1e12, 1),
        b'R' | b'r' => (1.0, 1), _ => (1.0, 0) } };
    let tail = &rest[used..];
    // `4k7`, `2u2`, `0R1`: the letter stands where the decimal point is
    if used == 1 && !tail.is_empty() && tail.bytes().all(|c| c.is_ascii_digit()) && !t[..i].contains('.') { return format!("{}.{tail}", &t[..i]).parse::<f64>().ok().map(|v| v * mult) }
    // a unit may follow (F, H, Hz, Ohm, V, A, s); anything with a digit in it is not a value
    if tail.bytes().any(|c| c.is_ascii_digit()) || !tail.chars().all(|c| c.is_ascii_alphabetic() || c == '%') { return None }
    Some(head * mult)
}
/// A JSON value as a number: a number, or a value's text.
pub fn num_of(v: &Value) -> Option<f64> { match v { Value::Number(n) => n.as_f64(), Value::String(s) => parse_value(s), _ => None } }
/// A number as a value field has it: `1.58k`, `100n`, `4.7u`, `1M`, `0.22`.
pub fn format_value(v: f64) -> String {
    if v == 0.0 || !v.is_finite() { return format!("{v}") }
    const P: [(f64, &str); 9] = [(1e12, "T"), (1e9, "G"), (1e6, "M"), (1e3, "k"), (1.0, ""), (1e-3, "m"), (1e-6, "u"), (1e-9, "n"), (1e-12, "p")];
    let a = v.abs(); let (scale, suffix) = P.iter().find(|(s, _)| a >= *s * 0.9999995).copied().unwrap_or((1e-15, "f"));
    let m = super::ksim_wave::round_sig(v / scale, 6);
    format!("{m}{suffix}")
}
/// A number for an override: plain, so neither KiCad nor ngspice reads a suffix its own way (`M` is mega to one and milli to the other).
pub fn plain(v: f64) -> String { format!("{}", super::ksim_wave::round_sig(v, 9)) }

const E24: [f64; 24] = [1.0, 1.1, 1.2, 1.3, 1.5, 1.6, 1.8, 2.0, 2.2, 2.4, 2.7, 3.0, 3.3, 3.6, 3.9, 4.3, 4.7, 5.1, 5.6, 6.2, 6.8, 7.5, 8.2, 9.1];
/// One decade of a preferred-number series (E3 to E192), from 1 up to under 10.
pub fn e_series(name: &str) -> Option<Vec<f64>> {
    let n: usize = name.trim().trim_start_matches(['E', 'e']).parse().ok()?;
    match n {
        3 | 6 | 12 | 24 => Some(E24.iter().step_by(24 / n).copied().collect()),
        // rounded to three figures; the one the standard writes otherwise is E192's 9.20
        48 | 96 | 192 => Some((0..n).map(|i| { let v = (10f64.powf(i as f64 / n as f64) * 100.0).round() / 100.0; if n == 192 && i == 185 { 9.2 } else { v } }).collect()),
        _ => None,
    }
}
/// The series' values from `from` to `to`, in order.
pub fn series_values(series: &[f64], from: f64, to: f64) -> Vec<f64> {
    if !(from > 0.0) || !(to >= from) { return vec![] }
    let mut out = vec![];
    for d in (from.log10().floor() as i32 - 1)..=(to.log10().floor() as i32 + 1) { for m in series { let v = super::ksim_wave::round_sig(m * 10f64.powi(d), 6); if v >= from * (1.0 - 1e-9) && v <= to * (1.0 + 1e-9) { out.push(v) } } }
    out
}
/// The series' values on each side of `x` (the one at or under it, the one over it).
pub fn neighbours(series: &[f64], x: f64) -> (Option<f64>, Option<f64>) {
    let all = series_values(series, x / 10.0, x * 10.0);
    (all.iter().rev().find(|v| **v <= x * (1.0 + 1e-9)).copied(), all.iter().find(|v| **v > x * (1.0 + 1e-9)).copied())
}
/// The values a sweep steps through: a list, a series between two values, or `steps` values from `from` to `to` (evenly along a log
/// axis when `log`, which is also the default for a range of a decade or more).
pub fn sweep_values(v: &Value) -> Result<Vec<f64>, String> {
    if let Some(a) = v["values"].as_array() { let out: Vec<f64> = a.iter().filter_map(num_of).collect(); if out.len() != a.len() || out.is_empty() { return Err("values is a list of numbers or values (\"4.7k\")".into()) } return Ok(out) }
    let (from, to) = (num_of(&v["from"]).ok_or("a sweep has `values`, or `from` and `to`")?, num_of(&v["to"]).ok_or("a sweep has `values`, or `from` and `to`")?);
    if let Some(s) = v["series"].as_str() { let out = series_values(&e_series(s).ok_or(format!("no series `{s}`: E3, E6, E12, E24, E48, E96, E192"))?, from.min(to), from.max(to));
        if out.is_empty() { return Err(format!("no {s} value between {from} and {to}")) } return Ok(out) }
    let n = v["steps"].as_u64().unwrap_or(11).clamp(2, 200) as usize;
    let log = v["log"].as_bool().unwrap_or(from > 0.0 && to > 0.0 && (to / from >= 10.0 || from / to >= 10.0));
    if log && !(from > 0.0 && to > 0.0) { return Err("a log sweep needs from and to above zero".into()) }
    Ok((0..n).map(|i| { let t = i as f64 / (n - 1) as f64; super::ksim_wave::round_sig(if log { from * (to / from).powf(t) } else { from + (to - from) * t }, 6) }).collect())
}

// ---- the search of a tune ----------------------------------------------------------------------------------------------------

/// What a tune does next.
#[derive(Debug, Clone, PartialEq)]
pub enum Step { Try(f64), Done { x: f64, y: f64, converged: bool }, Fail(String) }
/// The value of x in from..to that brings a measure to its target: both ends first, then false position between the two points that
/// bracket the target (Illinois: an end kept twice counts half, so a curved measure does not stall). For a range above zero it works
/// along log x, and along log y too while the measure and the target are above zero, so a power law (a corner against R, a gain against
/// a ratio) is met in one step. It is told each result and asked for the next value, so the runs are the caller's.
#[derive(Debug, Clone)]
pub struct Tuner { from: f64, to: f64, target: f64, tol: f64, max: usize, log: bool, seen: Vec<(f64, f64)>, a: (f64, f64, f64), b: (f64, f64, f64), kept: i8 }
impl Tuner {
    /// `tol`: how near the target counts as there, relative to it (absolute when the target is 0).
    pub fn new(from: f64, to: f64, target: f64, tol: f64, max: usize) -> Tuner { Tuner { from, to, target, tol, max: max.max(3), log: from > 0.0 && to > 0.0, seen: vec![], a: (0.0, 0.0, 1.0), b: (0.0, 0.0, 1.0), kept: 0 } }
    fn near(&self, y: f64) -> bool { (y - self.target).abs() <= self.tol * if self.target == 0.0 { 1.0 } else { self.target.abs() } }
    fn u(&self, x: f64) -> f64 { if self.log { x.ln() } else { x } }
    fn x(&self, u: f64) -> f64 { if self.log { u.exp() } else { u } }
    /// How far a result is from the target: along log y when that can be (see above).
    fn off(&self, y: f64) -> f64 { if self.log && self.target > 0.0 && self.seen.iter().all(|p| p.1 > 0.0) { (y / self.target).ln() } else { y - self.target } }
    pub fn runs(&self) -> usize { self.seen.len() }
    fn best(&self) -> (f64, f64) { self.seen.iter().copied().min_by(|p, q| (p.1 - self.target).abs().total_cmp(&(q.1 - self.target).abs())).unwrap_or((self.from, f64::NAN)) }
    /// The measure at `x` was `y`.
    pub fn tell(&mut self, x: f64, y: f64) {
        self.seen.push((x, y));
        match self.seen.len() { 1 => self.a = (x, y, 1.0), 2 => self.b = (x, y, 1.0),
            _ => if (y > self.target) == (self.a.1 > self.target) { self.a = (x, y, 1.0); if self.kept == 1 { self.b.2 /= 2.0 } self.kept = 1 } else { self.b = (x, y, 1.0); if self.kept == -1 { self.a.2 /= 2.0 } self.kept = -1 } }
    }
    pub fn next(&self) -> Step {
        match self.seen.len() { 0 => return Step::Try(self.from), 1 => return if self.near(self.seen[0].1) { Step::Done { x: self.seen[0].0, y: self.seen[0].1, converged: true } } else { Step::Try(self.to) }, _ => {} }
        let (bx, by) = self.best();
        if self.near(by) { return Step::Done { x: bx, y: by, converged: true } }
        if !by.is_finite() || self.seen.iter().any(|p| !p.1.is_finite()) { return Step::Fail("the measure is not a number at one end of the range".into()) }
        if (self.a.1 > self.target) == (self.b.1 > self.target) { let (p, q) = (self.seen[0], self.seen[1]);
            return Step::Fail(format!("over {} to {} the measure runs from {:.6e} to {:.6e}: the target {} is not between them. Widen from/to, or change another part", format_value(p.0), format_value(q.0), p.1, q.1, self.target)) }
        let (ua, ub) = (self.u(self.a.0), self.u(self.b.0)); let (fa, fb) = (self.off(self.a.1) * self.a.2, self.off(self.b.1) * self.b.2);
        if self.seen.len() >= self.max || (ua - ub).abs() <= 1e-9 * ua.abs().max(ub.abs()).max(1e-300) { return Step::Done { x: bx, y: by, converged: false } }
        Step::Try(self.x((ua * fb - ub * fa) / (fb - fa)))
    }
}

// ---- model files -------------------------------------------------------------------------------------------------------------

/// A `.subckt` (its nodes in order) or a `.model` (its device type) of a SPICE file.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDef { pub name: String, pub subckt: bool, pub pins: Vec<String>, pub ty: String }
impl ModelDef { pub fn row(&self) -> Value { if self.subckt { json!({"name": self.name, "subckt": self.pins}) } else { json!({"name": self.name, "model": self.ty}) } } }

/// Why a file's text is no model ngspice can read: a web page (a login or licence page saved in its place), an encrypted model, or
/// not text at all. Each is the person's to settle, so each says to stop and ask.
pub fn unusable(text: &str) -> Option<String> {
    let head: String = text.chars().take(4000).collect::<String>().to_ascii_lowercase();
    if head.contains("<html") || head.contains("<!doctype") || head.contains("<head>") { return Some("this is a web page, not a SPICE model (a login, licence or download page saved in its place): stop and ask the person to get the file".into()) }
    if text.contains("$CDNENCSTART") || head.contains("**$encrypted_lib") || head.contains("* ltspice encrypted file") || head.contains(".prot") && !head.contains(".unprot") && head.matches(|c: char| c.is_control() && c != '\n' && c != '\r' && c != '\t').count() > 8 { return Some("this model is encrypted for its vendor's simulator; ngspice cannot read it: stop and ask the person (another model, or the vendor's unencrypted one)".into()) }
    if text.chars().take(4000).filter(|c| *c == '\u{fffd}' || (c.is_control() && !matches!(c, '\n' | '\r' | '\t'))).count() > 20 { return Some("this is not text (an archive or a binary model): unpack it and give the .lib, .mod, .cir or .sub inside".into()) }
    None
}
/// The `.subckt` and `.model` definitions of a SPICE file, in order (continuation lines joined, comments dropped, nested ones included).
pub fn models_in(text: &str) -> Vec<ModelDef> {
    let mut lines: Vec<String> = vec![];
    for l in text.lines() { let l = l.trim(); if l.is_empty() || l.starts_with('*') { continue }
        let l = l.split(" ;").next().unwrap_or(l).split(" $ ").next().unwrap_or(l).trim_end();
        match l.strip_prefix('+') { Some(more) if !lines.is_empty() => { let last = lines.last_mut().unwrap(); last.push(' '); last.push_str(more.trim()) } _ => lines.push(l.to_string()) } }
    let mut out = vec![];
    for l in lines {
        let low = l.to_ascii_lowercase();
        if low.starts_with(".subckt ") { let mut it = l.split_whitespace().skip(1); let Some(name) = it.next() else { continue };
            let pins: Vec<String> = it.take_while(|t| !t.contains('=') && !t.eq_ignore_ascii_case("params:")).map(String::from).collect();
            out.push(ModelDef { name: name.into(), subckt: true, pins, ty: String::new() }) }
        else if low.starts_with(".model ") { let rest = l[7..].trim().replacen('(', " ", 1); let mut it = rest.split_whitespace(); let Some(name) = it.next() else { continue };
            let ty = it.next().unwrap_or("").trim_end_matches(')').to_ascii_uppercase();
            if !out.iter().any(|m: &ModelDef| !m.subckt && m.name.eq_ignore_ascii_case(name)) { out.push(ModelDef { name: name.into(), subckt: false, pins: vec![], ty }) } }
    }
    out
}
fn norm(s: &str) -> String { s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect() }
/// How well a model's name fits a part's name (its value, MPN or library entry): 2 the same, 1 one starts with the other (LM358DR and
/// LM358; TL072 and TL072_TI) with four characters or more in common, 0 not.
pub fn name_fit(model: &str, part: &str) -> u8 {
    let (m, p) = (norm(model), norm(part)); if m.is_empty() || p.is_empty() { return 0 }
    if m == p { 2 } else if (p.starts_with(&m) && m.len() >= 4) || (m.starts_with(&p) && p.len() >= 4) { 1 } else { 0 }
}
const MODEL_EXT: &[&str] = &["lib", "mod", "cir", "sub", "spice", "sp", "model", "ckt"];
pub fn is_model_file(p: &Path) -> bool { p.extension().and_then(|e| e.to_str()).is_some_and(|e| MODEL_EXT.iter().any(|x| e.eq_ignore_ascii_case(x))) }
/// The model files of a folder: its own, and those under `sim/`, `models/`, `spice/` and `sim/models/` (to `depth`), at most 400 files
/// of 8 MB each, with what each defines.
pub fn scan(dir: &Path, depth: u32) -> Vec<(PathBuf, Vec<ModelDef>)> {
    fn walk(d: &Path, depth: u32, all: bool, out: &mut Vec<PathBuf>) {
        let mut es: Vec<PathBuf> = std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).collect(); es.sort();
        for p in es { if out.len() >= 400 { return }
            let name = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if p.is_dir() { if depth > 0 && !name.starts_with('.') && (all || matches!(name.as_str(), "sim" | "models" | "spice" | "simulation" | "lib" | "libs")) { walk(&p, depth - 1, true, out) } }
            else if is_model_file(&p) && p.metadata().map_or(false, |m| m.len() <= 8 << 20) { out.push(p) } }
    }
    let mut files = vec![]; walk(dir, depth, false, &mut files);
    files.into_iter().filter_map(|p| { let t = String::from_utf8_lossy(&std::fs::read(&p).ok()?).into_owned(); if unusable(&t).is_some() { return None } let m = models_in(&t); (!m.is_empty()).then_some((p, m)) }).collect()
}
/// Where a model was found: the file, the definition, how well its name fits, and which step of the lookup found it.
#[derive(Debug, Clone, PartialEq)]
pub struct Found { pub file: String, pub def: ModelDef, pub fit: u8, pub source: &'static str }
impl Found { pub fn row(&self) -> Value { let mut r = self.def.row(); r["file"] = json!(self.file); r["from"] = json!(self.source); if self.fit < 2 { r["name_fit"] = json!("near") } r } }
/// The models that fit one of a part's names among scanned files (`source`: project, library, kicad), best first, three at most.
pub fn lookup(names: &[&str], scanned: &[(String, &'static str, Vec<ModelDef>)]) -> Vec<Found> {
    let mut out: Vec<Found> = vec![];
    for (file, source, defs) in scanned { for d in defs { let fit = names.iter().map(|n| name_fit(&d.name, n)).max().unwrap_or(0);
        if fit > 0 && !out.iter().any(|f| f.file == *file && f.def.name == d.name) { out.push(Found { file: file.clone(), def: d.clone(), fit, source }) } } }
    out.sort_by(|a, b| b.fit.cmp(&a.fit).then((a.source != "project").cmp(&(b.source != "project")))); out.truncate(3); out
}
/// A file name for a fetched model: what was asked, else the last part of its URL, else its first definition, kept to plain characters
/// and given `.lib` when it has no model extension.
pub fn store_name(asked: &str, url: &str, first: &str) -> String {
    let base = if !asked.is_empty() { asked } else { let u = url.split(['?', '#']).next().unwrap_or("").trim_end_matches('/').rsplit('/').next().unwrap_or(""); if u.contains('.') && !u.is_empty() { u } else { first } };
    let mut n: String = base.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+') { c } else { '_' }).collect();
    n = n.trim_matches(['.', '_']).to_string(); if n.is_empty() { n = "model".into() }
    if !is_model_file(Path::new(&n)) { n.push_str(".lib") }
    n
}
/// Keep a fetched model in the project: `sim/models/<name>` with `<name>.source.json` beside it (where it came from, its licence line,
/// its digest, what it defines). The text is written as it came. Answers the path from the project's folder and the definitions.
pub fn store(project_dir: &Path, name: &str, text: &str, url: &str, licence: &str, at: u64) -> Result<(String, Vec<ModelDef>), String> {
    if let Some(why) = unusable(text) { return Err(why) }
    let defs = models_in(text);
    if defs.is_empty() { return Err("no .subckt or .model in this text: it is not a SPICE model file (a datasheet, a readme, or a model in another simulator's format)".into()) }
    if url.is_empty() { return Err("say where the model came from (`url`): the project keeps each model's source".into()) }
    let dir = project_dir.join("sim").join("models"); std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let file = dir.join(name);
    if let Ok(old) = std::fs::read(&file) { if old != text.as_bytes() { return Err(format!("sim/models/{name} is there with other content: give another name (`as`)")) } }
    std::fs::write(&file, text).map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    use sha2::{Digest, Sha256};
    let digest: String = Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    let note = json!({"file": name, "url": url, "licence": if licence.is_empty() { "not stated by the source" } else { licence }, "fetched_at": at, "sha256": digest, "models": defs.iter().map(ModelDef::row).collect::<Vec<_>>()});
    std::fs::write(dir.join(format!("{name}.source.json")), format!("{}\n", serde_json::to_string_pretty(&note).unwrap_or_default())).map_err(|e| format!("cannot write the source note: {e}"))?;
    Ok((format!("sim/models/{name}"), defs))
}
/// A subcircuit's nodes onto a symbol's pins by name, when every node has exactly one pin of its name (`IN+`, `OUT`, `V+`): {pin number:
/// node}. None when the names do not settle it, and the order of the pins is what is left.
pub fn pins_by_name(nodes: &[String], pins: &[(String, String)]) -> Option<Vec<(String, String)>> {
    let key = |s: &str| s.to_ascii_uppercase().replace(['_', ' '], "");
    let mut out = vec![];
    for n in nodes { let mut hit = pins.iter().filter(|(_, name)| key(name) == key(n)); let p = hit.next()?; if hit.next().is_some() { return None } out.push((p.0.clone(), n.clone())) }
    let mut seen: Vec<&String> = out.iter().map(|(p, _)| p).collect(); seen.sort(); seen.dedup();
    (seen.len() == out.len() && !out.is_empty()).then_some(out)
}

// ---- which parts do not simulate ---------------------------------------------------------------------------------------------

/// The named groups of parts an `exclude` takes: (name, reference prefixes, library id patterns).
pub const GROUPS: &[(&str, &[&str], &[&str])] = &[
    ("connectors", &["J", "P", "CN", "CON", "X"], &["Connector*:*", "*:Conn_*", "*:USB_*", "*:Barrel_Jack*"]),
    ("mounting_holes", &["H", "MH", "MK"], &["Mechanical:MountingHole*", "*:MountingHole*"]),
    ("test_points", &["TP"], &["Connector:TestPoint*", "*:TestPoint*"]),
    ("fiducials", &["FID"], &["Mechanical:Fiducial*", "*:Fiducial*"]),
    ("mechanical", &["HS", "MP", "LOGO", "G"], &["Mechanical:*", "Graphic:*"]),
    ("jumpers", &["JP", "SJ"], &["Jumper:*"]),
];
/// Whether a part is of a named group: by its library id when that says, else by its reference's letters.
pub fn in_group(group: &str, reference: &str, lib_id: &str) -> Option<bool> {
    let (_, prefixes, libs) = GROUPS.iter().find(|g| g.0 == group)?;
    let letters: String = reference.chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_ascii_uppercase();
    Some(libs.iter().any(|p| super::kedit::glob(p, lib_id)) || (prefixes.contains(&letters.as_str()) && !(group == "connectors" && letters == "X" && lib_id.to_ascii_lowercase().contains("crystal"))))
}

// ---- problems ----------------------------------------------------------------------------------------------------------------

fn after<'a>(text: &'a str, key: &str) -> Option<&'a str> { let low = text.to_ascii_lowercase(); let i = low.find(key)?; text[i + key.len()..].trim_start_matches([' ', ':', '=']).split(|c: char| c.is_whitespace() || matches!(c, ',' | ';')).next().filter(|s| !s.is_empty()) }
/// One problem of a simulation as one line with what to do next. `p` is the fork's `{kind, reference, node, message}`; a kind this hub
/// does not know, or none, is read from the message's own words (ngspice's).
pub fn problem_line(p: &Value) -> String {
    let g = |k: &str| p[k].as_str().unwrap_or("").trim();
    let (kind, r, node, msg) = (g("kind").to_ascii_lowercase(), g("reference"), g("node"), g("message")); let low = msg.to_ascii_lowercase();
    let part = if r.is_empty() { "a part".to_string() } else { r.to_string() };
    let is = |words: &[&str]| words.iter().any(|w| kind.contains(w));
    let says = |words: &[&str]| words.iter().any(|w| low.contains(w));
    let short: String = msg.lines().next().unwrap_or("").chars().take(160).collect();
    if is(&["library_not_found", "lib_not_found", "no_library"]) || says(&["could not find library", "library file not found", "cannot open library", "no such file"]) {
        return format!("model library of {part} not found{}: give its path with model (lib), or import the vendor's file", if short.is_empty() { String::new() } else { format!(" ({short})") }) }
    if is(&["model_not_in_library", "not_in_library", "model_not_found"]) || says(&["not found in library", "unknown subckt", "could not find a valid modelname", "unable to find definition of model", "unknown model"]) {
        return format!("the model named for {part} is not in its library{}: name one the file has (import lists them), or give another file", after(msg, "model").map(|m| format!(" ({m})")).unwrap_or_default()) }
    if is(&["pin"]) || says(&["pin count", "number of pins", "too few nodes", "incorrect number of nodes", "wrong number of"]) {
        return format!("pins of {part} do not match its model's nodes: give the map with model (pins: {{pin: node}})") }
    if is(&["bad_param"]) { return format!("a value or parameter of {part} does not parse{}: set it with model (params), or correct the part's value", if short.is_empty() { String::new() } else { format!(" ({short})") }) }
    if is(&["no_ground"]) || says(&["no ground", "no node 0"]) { return "the circuit has no ground (node 0): put a GND power symbol on the return net".into() }
    if is(&["measure"]) { return format!("a measure gave no value{}", if short.is_empty() { String::new() } else { format!(": {short}") }) }
    if is(&["missing_model", "no_model", "model"]) || says(&["no model", "has no simulation model", "missing model"]) {
        return format!("missing model for {part}: assign one (model) or exclude it (exclude)") }
    if is(&["float"]) || says(&["floating", "no dc path to ground", "no dc path", "has no dc value"]) {
        let n = if node.is_empty() { after(msg, "node").unwrap_or("a node").to_string() } else { node.to_string() };
        return format!("node {n} floats: nothing sets its DC level. Give it a path to ground (a resistor, a source), or exclude what hangs there") }
    if is(&["singular"]) || says(&["singular matrix", "matrix is singular"]) {
        let n = if node.is_empty() { after(msg, "check node").or(after(msg, "check nodes")).unwrap_or("").to_string() } else { node.to_string() };
        return format!("singular matrix{}: a loop of sources or inductors, or a node with no DC path. Add a small series resistance or a path to ground there", if n.is_empty() { String::new() } else { format!(" at {n}") }) }
    if is(&["convergence", "converge", "timestep"]) || says(&["no convergence", "timestep too small", "time step too small", "iteration limit", "failed to converge", "gmin stepping failed", "source stepping failed"]) {
        let t = ["time =", "time=", " t =", " t="].iter().find_map(|k| after(msg, k)).filter(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit())).map(|t| format!(" at t={}", t.trim_end_matches(['.', ':'])));
        let n = if node.is_empty() { after(msg, "trouble with node").or(after(msg, "trouble with")).map(|n| n.trim_matches(['"', '\'']).to_string()).unwrap_or_default() } else { node.to_string() };
        let op = low.contains("gmin stepping") || low.contains("source stepping") || low.contains("operating point") || kind.ends_with("_op");
        return format!("no convergence{}{}: {}", t.unwrap_or_default(), if n.is_empty() { String::new() } else { format!(" (near {n})") },
            if op { "the DC operating point was not found. Set starting voltages (directives: .nodeset or .ic), or ramp the supplies in a .tran with uic" } else { "shorten the maximum time step (the third value of .tran), give edges a rise time, or add directives: .options method=gear reltol=1e-3" }) }
    if is(&["timeout"]) || says(&["timed out", "timeout"]) { return "the run did not finish in its time: a shorter stop time or a coarser step, or a longer timeout".into() }
    if is(&["abort"]) { return "the run was stopped (by the person, in the Simulator)".into() }
    if is(&["busy"]) || says(&["as_busy"]) { return "a simulation of this project is running: this one was not started; ask again when it ends".into() }
    if is(&["netlist", "syntax", "directive"]) || says(&["syntax error", "unknown parameter", "unrecognized parameter", "unknown device", "parse error"]) {
        return format!("netlist error{}: {short}", if r.is_empty() { String::new() } else { format!(" at {r}") }) }
    if short.is_empty() { format!("{}{}", if kind.is_empty() { "simulation problem".to_string() } else { kind.replace('_', " ") }, if r.is_empty() { String::new() } else { format!(" at {r}") }) } else if r.is_empty() || short.contains(r) { short } else { format!("{r}: {short}") }
}
/// Problems read from ngspice's console when the fork gave none of its own for a failed run: the lines that say what went wrong.
pub fn problems_of_log(log: &str) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for l in log.lines() { let t = l.trim(); let low = t.to_ascii_lowercase();
        let hit = ["singular matrix", "timestep too small", "no convergence", "iteration limit", "unknown subckt", "could not find a valid modelname", "unable to find definition of model", "no dc path", "floating", "gmin stepping failed", "source stepping failed", "syntax error", "unknown parameter", "unknown device", "no such file", "fatal error", "error:"];
        if hit.iter().any(|h| low.contains(h)) && !out.iter().any(|o| o["message"] == t) { out.push(json!({"message": t})) }
        if out.len() >= 8 { break } }
    out
}
/// A failed run's status in words, when no problem says more.
pub fn status_words(status: &str) -> &'static str {
    match status { "SIM_OK" | "" => "ok", "SIM_NETLIST_ERROR" => "the netlist has errors", "SIM_MODEL_ERROR" => "a model is missing or wrong", "SIM_NO_CONVERGENCE" => "no convergence", "SIM_FLOATING_NODE" => "a node floats",
        "SIM_SINGULAR_MATRIX" => "singular matrix", "SIM_TIMEOUT" => "the run did not finish in its time", "SIM_ABORTED" => "the run was stopped", _ => "the simulation failed" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn values_read_and_written() {
        for (t, v) in [("4.7k", 4700.0), ("100n", 1e-7), ("100nF", 1e-7), ("1Meg", 1e6), ("1MEG", 1e6), ("1M", 1e6), ("1m", 1e-3), ("2u2", 2.2e-6), ("4k7", 4700.0), ("0R1", 0.1), ("4R7", 4.7), ("10kOhm", 1e4), ("10 ", 10.0), ("1e-6", 1e-6),
            ("3.3V", 3.3), ("1µF", 1e-6), ("47p", 47e-12), ("10f", 1e-14), ("1F", 1.0), ("2.2uH", 2.2e-6), ("-5", -5.0), ("1E3", 1000.0), ("100Hz", 100.0), ("1,5k", 1500.0), ("22R", 22.0), ("1G", 1e9), ("5%", 5.0)] {
            let got = parse_value(t).unwrap_or_else(|| panic!("{t} did not parse")); assert!((got - v).abs() <= 1e-9 * v.abs(), "{t}: {got} != {v}") }
        for t in ["", "abc", "LM358", "10k/5", "R1", "1k2k", "DNP", "k"] { assert_eq!(parse_value(t), None, "{t}") }
        assert_eq!((num_of(&json!(12.5)), num_of(&json!("1k")), num_of(&json!(null))), (Some(12.5), Some(1000.0), None));
        for (v, t) in [(1580.0, "1.58k"), (1e-7, "100n"), (4.7e-6, "4.7u"), (1e6, "1M"), (0.22, "220m"), (1.0, "1"), (47e-12, "47p"), (999.9999999, "1k"), (12345.678, "12.3457k"), (-3300.0, "-3.3k"), (2.2e-15, "2.2f")] {
            assert_eq!(format_value(v), t); assert!((parse_value(t).unwrap() - v).abs() <= 1e-5 * v.abs(), "{t} reads back") }
        assert_eq!((plain(1580.0), plain(1e-7), plain(0.1 + 0.2), plain(1e6)), ("1580".to_string(), "0.0000001".to_string(), "0.3".to_string(), "1000000".to_string()));
    }
    #[test] fn the_e_series() {
        assert_eq!(e_series("E6"), Some(vec![1.0, 1.5, 2.2, 3.3, 4.7, 6.8])); assert_eq!(e_series("e12").unwrap().len(), 12); assert_eq!(e_series("E24").unwrap()[23], 9.1);
        let e96 = e_series("E96").unwrap(); assert_eq!((e96.len(), e96[0], e96[1], e96[19], e96[95]), (96, 1.0, 1.02, 1.58, 9.76));
        let e192 = e_series("E192").unwrap(); assert_eq!((e192.len(), e192[185], e192[191]), (192, 9.2, 9.88)); assert_eq!(e_series("E48").unwrap()[47], 9.53);
        assert_eq!((e_series("E7"), e_series("x")), (None, None));
        assert_eq!(series_values(&e_series("E6").unwrap(), 900.0, 5000.0), vec![1000.0, 1500.0, 2200.0, 3300.0, 4700.0]);
        assert_eq!(neighbours(&e96, 1591.5), (Some(1580.0), Some(1620.0))); assert_eq!(neighbours(&e_series("E24").unwrap(), 1000.0), (Some(1000.0), Some(1100.0)));
        assert_eq!(neighbours(&e_series("E12").unwrap(), 9.9e-9), (Some(8.2e-9), Some(1e-8)));
        assert_eq!(sweep_values(&json!({"values": ["1k", 2200, "4k7"]})), Ok(vec![1000.0, 2200.0, 4700.0]));
        assert_eq!(sweep_values(&json!({"from": "1k", "to": "10k", "series": "E6"})), Ok(vec![1000.0, 1500.0, 2200.0, 3300.0, 4700.0, 6800.0, 10000.0]));
        assert_eq!(sweep_values(&json!({"from": 1, "to": 5, "steps": 5})), Ok(vec![1.0, 2.0, 3.0, 4.0, 5.0]));
        assert_eq!(sweep_values(&json!({"from": 100, "to": "100k", "steps": 4})), Ok(vec![100.0, 1000.0, 10000.0, 100000.0]));
        assert!(sweep_values(&json!({"from": 1})).is_err() && sweep_values(&json!({"values": ["x"]})).is_err() && sweep_values(&json!({"from": 1, "to": 2, "series": "E5"})).unwrap_err().contains("no series"));
    }
    /// Run a tuner against a function; the value it settles on and the runs it took.
    fn tune(f: impl Fn(f64) -> f64, from: f64, to: f64, target: f64, tol: f64) -> Result<(f64, usize, bool), String> {
        let mut t = Tuner::new(from, to, target, tol, 24);
        loop { match t.next() { Step::Try(x) => { let slack = (to - from).abs() * 1e-6; assert!(x >= from.min(to) - slack && x <= from.max(to) + slack, "{x} is outside the range"); t.tell(x, f(x)) } Step::Done { x, converged, .. } => return Ok((x, t.runs(), converged)), Step::Fail(e) => return Err(e) } }
    }
    #[test] fn a_tune_finds_the_value() {
        use std::f64::consts::PI;
        // the R of an RC low-pass for a 1 kHz corner with 100 nF: 1/(2 pi f C)
        let c = 100e-9; let (r, runs, ok) = tune(|r| 1.0 / (2.0 * PI * r * c), 100.0, 100e3, 1000.0, 1e-3).unwrap();
        assert!(ok && (r - 1.0 / (2.0 * PI * 1000.0 * c)).abs() / r < 1e-3 && runs <= 8, "{r} in {runs} runs");
        // a divider's lower resistor for 3.3 V from 5 V over 10k; an overshoot against damping (falling, curved); a target of zero
        let (r2, runs, _) = tune(|r2| 5.0 * r2 / (10e3 + r2), 1e3, 100e3, 3.3, 1e-4).unwrap(); assert!((r2 - 19411.76).abs() < 5.0 && runs <= 10, "{r2} in {runs}");
        let os = |z: f64| 100.0 * (-PI * z / (1.0 - z * z).sqrt()).exp(); let (z, runs, _) = tune(os, 0.05, 0.95, 10.0, 1e-3).unwrap(); assert!((os(z) - 10.0).abs() < 0.011 && runs <= 12, "{z} in {runs}");
        let (x, _, ok) = tune(|x| x * x * x - 8.0, -1.0, 5.0, 0.0, 1e-6).unwrap(); assert!(ok && (x - 2.0).abs() < 1e-5, "{x}");
        // an end that is already there costs one run; a target outside the range says what the range gives
        assert_eq!(tune(|x| x, 5.0, 10.0, 5.0, 1e-3).map(|r| (r.0, r.1)), Ok((5.0, 1)));
        let e = tune(|r| 1.0 / (2.0 * PI * r * c), 100.0, 1000.0, 10.0, 1e-3).unwrap_err(); assert!(e.contains("100 to 1k") && e.contains("the target 10 is not between them"), "{e}");
        assert!(tune(|_| f64::NAN, 1.0, 2.0, 1.0, 1e-3).unwrap_err().contains("not a number"));
        // a step function cannot be met: the search ends at its run limit with the nearest point, and says it did not converge
        let (_, runs, ok) = tune(|x| if x < 3.0 { 0.0 } else { 10.0 }, 1.0, 10.0, 5.0, 1e-3).unwrap(); assert!(!ok && runs <= 24);
    }
    const LIB: &str = "* LM358 model\n.SUBCKT LM358 1 2 3 4 5\n* nodes: in+ in- vcc vee out\nR1 1 2 1k\n.model DX D(Is=800.0E-18)\n.ends\n\n.subckt TL072  IN+ IN- V+ V-\n+ OUT  PARAMS: GAIN=1\n.ends\n.MODEL 1N4148 D (IS=4.352E-9 N=1.906)\n.model BC547B NPN(IS=7.59E-15\n+ BF=330)\n.model lookup ako:BC547B NPN\n";
    #[test] fn a_model_files_definitions() {
        let m = models_in(LIB);
        assert_eq!(m.iter().map(|d| d.row()).collect::<Vec<_>>(), vec![json!({"name": "LM358", "subckt": ["1", "2", "3", "4", "5"]}), json!({"name": "DX", "model": "D"}), json!({"name": "TL072", "subckt": ["IN+", "IN-", "V+", "V-", "OUT"]}),
            json!({"name": "1N4148", "model": "D"}), json!({"name": "BC547B", "model": "NPN"}), json!({"name": "lookup", "model": "AKO:BC547B"})]);
        assert!(models_in("just a readme\nwith words").is_empty());
        assert!(unusable("<!DOCTYPE html><html><body>Please sign in</body></html>").unwrap().contains("web page") && unusable("* lib\n$CDNENCSTART_ADV3\nabcdef").unwrap().contains("encrypted") && unusable(&"PK\u{3}\u{4}\u{0}\u{1}\u{2}".repeat(10)).unwrap().contains("not text") && unusable(LIB).is_none());
        assert_eq!((name_fit("LM358", "LM358"), name_fit("lm358", "LM358DR"), name_fit("TL072_TI", "TL072"), name_fit("D", "D1"), name_fit("BC547B", "BC547C"), name_fit("1N4148", "1N4148W-7-F")), (2, 1, 1, 0, 0, 1));
        assert_eq!(pins_by_name(&["IN+".into(), "IN-".into(), "V+".into(), "V-".into(), "OUT".into()], &[("1".into(), "OUT".into()), ("2".into(), "IN-".into()), ("3".into(), "IN+".into()), ("4".into(), "V-".into()), ("8".into(), "V+".into())]),
            Some(vec![("3".into(), "IN+".into()), ("2".into(), "IN-".into()), ("8".into(), "V+".into()), ("4".into(), "V-".into()), ("1".into(), "OUT".into())]));
        assert_eq!(pins_by_name(&["1".into(), "2".into()], &[("1".into(), "A".into()), ("2".into(), "K".into())]), None);
        assert_eq!(pins_by_name(&["OUT".into()], &[("1".into(), "OUT".into()), ("7".into(), "OUT".into())]), None, "two pins of the name do not settle it");
    }
    #[test] fn models_are_found_in_the_project_and_a_fetched_one_is_kept_with_its_source() {
        let d = std::env::temp_dir().join(format!("ksim-parts-{}-{:?}", std::process::id(), std::thread::current().id())); let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("models/ti")).unwrap(); std::fs::create_dir_all(d.join("firmware/lib")).unwrap(); std::fs::create_dir_all(d.join(".git")).unwrap();
        std::fs::write(d.join("opamps.lib"), LIB).unwrap(); std::fs::write(d.join("models/ti/tps.cir"), ".subckt TPS54331 VIN EN SS VSENSE COMP GND PH BOOT\n.ends\n").unwrap();
        std::fs::write(d.join("firmware/lib/x.lib"), ".model HIDDEN D\n").unwrap(); std::fs::write(d.join(".git/x.lib"), ".model GITTY D\n").unwrap(); std::fs::write(d.join("notes.txt"), ".model NOTE D\n").unwrap(); std::fs::write(d.join("page.lib"), "<html>login</html>").unwrap();
        let found = scan(&d, 3); let names: Vec<String> = found.iter().flat_map(|(_, m)| m.iter().map(|x| x.name.clone())).collect();
        assert!(names.contains(&"LM358".to_string()) && names.contains(&"TPS54331".to_string()) && !names.iter().any(|n| n == "HIDDEN" || n == "GITTY" || n == "NOTE"), "{names:?}");
        let scanned: Vec<(String, &'static str, Vec<ModelDef>)> = found.iter().map(|(p, m)| (p.strip_prefix(&d).unwrap().to_string_lossy().into_owned(), "project", m.clone())).collect();
        let hit = lookup(&["LM358DR", "LM358"], &scanned); assert_eq!((hit.len(), hit[0].row()), (1, json!({"name": "LM358", "subckt": ["1", "2", "3", "4", "5"], "file": "opamps.lib", "from": "project"})));
        assert_eq!(lookup(&["TPS54331DR"], &scanned)[0].row()["name_fit"], json!("near")); assert!(lookup(&["NE555", ""], &scanned).is_empty());
        // a fetched model: stored as it came, with a note of its source; the same text again is fine, other text under the name is refused
        assert_eq!((store_name("", "https://www.ti.com/lit/zip/sloj045?x=1", "LM358"), store_name("", "https://vendor.example/models/OPA333.LIB", "OPA333"), store_name("my model.txt", "", ""), store_name("", "", "")), ("LM358.lib".into(), "OPA333.LIB".into(), "my_model.txt.lib".into(), "model.lib".into()));
        let (path, defs) = store(&d, "LM358.lib", LIB, "https://vendor.example/lm358.lib", "TI model licence: free to use for simulation", 1_800_000_000).unwrap();
        assert_eq!((path.as_str(), defs.len(), std::fs::read_to_string(d.join("sim/models/LM358.lib")).unwrap().as_str()), ("sim/models/LM358.lib", 6, LIB));
        let note: Value = serde_json::from_str(&std::fs::read_to_string(d.join("sim/models/LM358.lib.source.json")).unwrap()).unwrap();
        assert_eq!((note["url"].as_str(), note["licence"].as_str(), note["sha256"].as_str().map(str::len), note["models"][0]["name"].as_str(), note["fetched_at"].as_u64()), (Some("https://vendor.example/lm358.lib"), Some("TI model licence: free to use for simulation"), Some(64), Some("LM358"), Some(1_800_000_000)));
        assert!(store(&d, "LM358.lib", LIB, "https://vendor.example/lm358.lib", "", 1).is_ok());
        assert!(store(&d, "LM358.lib", ".model X D\n", "https://x", "", 1).unwrap_err().contains("other content"));
        assert!(store(&d, "a.lib", "<html>Sign in to download</html>", "https://x", "", 1).unwrap_err().contains("stop and ask the person"));
        assert!(store(&d, "a.lib", "A datasheet in words", "https://x", "", 1).unwrap_err().contains("no .subckt or .model"));
        assert!(store(&d, "a.lib", ".model X D\n", "", "", 1).unwrap_err().contains("where the model came from"));
        // the stored model is found by the next scan, as the project's
        assert!(scan(&d, 3).iter().any(|(p, _)| p.ends_with("sim/models/LM358.lib")));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn groups_of_parts_that_do_not_simulate() {
        assert_eq!((in_group("connectors", "J3", "Connector_Generic:Conn_01x04"), in_group("connectors", "U1", "Connector:USB_C_Receptacle"), in_group("connectors", "R1", "Device:R"), in_group("connectors", "X1", "Device:Crystal")), (Some(true), Some(true), Some(false), Some(false)));
        assert_eq!((in_group("mounting_holes", "H2", "Mechanical:MountingHole_Pad"), in_group("test_points", "TP7", "Connector:TestPoint"), in_group("test_points", "T1", "Device:Transformer"), in_group("fiducials", "FID1", "")), (Some(true), Some(true), Some(false), Some(true)));
        assert_eq!(in_group("capacitors", "C1", "Device:C"), None);
    }
    #[test] fn problems_are_one_line_with_what_to_do() {
        let l = |v: Value| problem_line(&v);
        assert_eq!(l(json!({"kind": "SPK_MISSING_MODEL", "reference": "U3", "message": "U3 has no simulation model"})), "missing model for U3: assign one (model) or exclude it (exclude)");
        assert_eq!(l(json!({"kind": "SPK_FLOATING_NODE", "node": "N003"})), "node N003 floats: nothing sets its DC level. Give it a path to ground (a resistor, a source), or exclude what hangs there");
        assert!(l(json!({"kind": "SPK_NO_CONVERGENCE", "message": "doAnalyses: TRAN:  Timestep too small; time = 1.2345e-05, timestep = 1.25e-19: trouble with node \"sw\""})).starts_with("no convergence at t=1.2345e-05"));
        assert!(l(json!({"message": "Error: no convergence in gmin stepping failed"})).contains("DC operating point was not found"));
        assert!(l(json!({"kind": "SPK_LIBRARY_NOT_FOUND", "reference": "U1", "message": "Could not find library file 'opamp.lib'"})).starts_with("model library of U1 not found (Could not find library file 'opamp.lib'): give its path"));
        assert_eq!(l(json!({"kind": "SPK_MODEL_NOT_IN_LIBRARY", "reference": "Q2", "message": "model BC847 not found in library"})), "the model named for Q2 is not in its library (BC847): name one the file has (import lists them), or give another file");
        assert_eq!(l(json!({"kind": "SPK_PIN_COUNT_MISMATCH", "reference": "U2"})), "pins of U2 do not match its model's nodes: give the map with model (pins: {pin: node})");
        assert!(l(json!({"message": "Warning: singular matrix:  check node out"})).starts_with("singular matrix at out: a loop of sources or inductors"));
        assert_eq!((l(json!({"kind": "PK_NO_GROUND"})), l(json!({"kind": "ME_BAD_PARAMS", "reference": "R4", "message": "cannot read 'abc'"})), l(json!({"kind": "PK_MEASURE_FAILED", "message": "tr: out of interval"}))),
            ("the circuit has no ground (node 0): put a GND power symbol on the return net".into(), "a value or parameter of R4 does not parse (cannot read 'abc'): set it with model (params), or correct the part's value".into(), "a measure gave no value: tr: out of interval".to_string()));
        assert_eq!(l(json!({"kind": "SPK_NETLIST", "reference": "V1", "message": "unknown parameter (foo)"})), "netlist error at V1: unknown parameter (foo)");
        assert_eq!((l(json!({"kind": "SPK_TIMEOUT"})).contains("did not finish"), l(json!({"kind": "SPK_SOMETHING_NEW", "reference": "R1"})), l(json!({"reference": "R9", "message": "odd thing"})), l(json!({}))), (true, "spk something new at R1".into(), "R9: odd thing".into(), "simulation problem".into()));
        for v in [json!({"kind": "SPK_NO_CONVERGENCE", "message": "x\ny\nz ".repeat(200)}), json!({"message": "a ".repeat(900)})] { let s = l(v); assert!(!s.contains('\n') && s.len() < 400, "{s}") }
        let log = "Circuit: t\nDoing analysis at TEMP = 27\nWarning: singular matrix:  check node out\nNote: starting dynamic gmin stepping\nError: no convergence\nError: no convergence\nrun simulation(s) aborted\n";
        assert_eq!(problems_of_log(log), vec![json!({"message": "Warning: singular matrix:  check node out"}), json!({"message": "Error: no convergence"})]);
        assert_eq!((status_words("SIM_SINGULAR_MATRIX"), status_words("SIM_OK"), status_words("SIM_NEW")), ("singular matrix", "ok", "the simulation failed"));
    }
}
