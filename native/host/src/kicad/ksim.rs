//! `kicad_sim`: a simulation step in one tool call (docs/kicad-sim.md), beside `kicad_edit` and in its style. An agent names parts by
//! reference and signals as the Simulator does (`V(out)`, `I(R1)`); the hub opens the schematic, sends the fork's simulation commands
//! (`kiapi.backplane.sim`, the runtime's capability `sim-run`), waits for ngspice, and answers numbers: operating-point values, measures
//! (ksim_wave.rs), a table for a sweep, the value a tune found. Vectors stay in the hub unless asked for, and then come as a few points.
//!
//! [`run`] drives the ops over the same `Call` `kicad_edit` uses (the document's one-writer lock, the co-editing actor name, the save's
//! commit to main), so it is tested here against a stand-in KiCad. The table [`OPS`] is what the tool's description and the docs are
//! made from: an op's name and words are there, its work in [`Sim::op`].
use super::{kedit::{self, Call}, ksim_parts as kp, ksim_wave::{self as kw, Sig}};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// The runtime's capability for the simulation commands, and the first Backplane KiCad that has it.
pub const CAP: &str = "sim-run";
pub const NEEDS: &str = "bp.13.2";
const C: &str = "kiapi.common.commands.";
const P: &str = "kiapi.backplane.sim.";
/// What an older KiCad is answered.
pub fn no_sim(version: &str) -> String {
    format!("this KiCad has no simulation{}: kicad_sim needs Backplane KiCad {NEEDS} or newer (Settings, KiCad)", if version.is_empty() { String::new() } else { format!(" (it is {version})") })
}

/// The ops: (name, whether it changes the design, its shape, what it does).
pub const OPS: &[(&str, bool, &str, &str)] = &[
    ("models", false, "{models: true | \"all\" | refs}", "what would not simulate and why, with a model found on this machine for each where there is one; \"all\" or references list every part's model"),
    ("model", true, "{model: refs, device, type?, params?} | {model: refs, lib, name?, pins?} | {model: refs, raw: {primitive, model, library?}} | {model: refs, clear: true}",
        "assign a model as KiCad's dialog would: a built-in device with its parameters, a library file's .subckt or .model with a pin map {pin number: node} (by name when the names match, else by order, answered), or raw SPICE"),
    ("exclude", true, "{exclude: refs | group | {ref|lib|value: pattern}}", "exclude from simulation: references, a group (connectors, mounting_holes, test_points, fiducials, mechanical, jumpers), \"no_model\" (every part of those groups that has no model), or a pattern; answers the parts it took"),
    ("include", true, "{include: refs | ...}", "the reverse of exclude"),
    ("import", true, "{import: {file | text, url, licence?, as?}, to?: refs, name?, pins?}", "keep a fetched model file in the project's sim/models/ with its source URL and licence line, answer its .subckt/.model names and nodes, and assign it to `to`"),
    ("source", true, "{source: {type: dc|sin|pulse|pwl|exp|am|sffm, current?, ref?, pos, neg?, value?, params?, ac?, at?, bom?}}", "add a source symbol (Simulation_SPICE) between two nets, out of the BOM and off the board (bom: true keeps it in); neg defaults to GND"),
    ("probe", true, "{probe: signals | {pin: \"R1.2\", name}}", "name what to read: signals join the saved analysis' traces and this call's runs; a pin gets a net label first"),
    ("analysis", true, "{analysis: \".ac dec 20 1 1meg\", signals?, tab?} | {analysis: true}", "save the analysis with the design (what the person's Simulator opens with); true reads what is saved"),
    ("run", false, "{run: \".tran 1u 10m\" | true, set?: {R1: \"2k\", \"V1.dc\": 3.3}, signals?, measures?, waves?, directives?, options?, retry?, netlist?, log?, timeout?, show?}",
        "run an analysis (true: the saved one) and answer operating-point values, measures and a line per signal; `set` values are for this run only; `waves: true` (or a point count) adds each signal as a few points; `directives` are netlist lines for this run (.options, .ic, .include, .model), `options` KiCad's netlist options ({save_all_currents: false}); a transient that does not converge is tried again with gear, a looser reltol and a smaller step (`needed` says which; `retry: false` leaves it)"),
    ("loopgain", false, "{loopgain: \"U1.2\", run?: \".ac dec 50 1 100meg\", set?, directives?, waves?}", "a feedback loop's gain, broken at a pin (reference.pin number) in the netlist only: crossover frequency, phase margin, gain margin and DC loop gain (Tian's method: voltage and current injection combined, the operating point kept); `waves` adds the curve"),
    ("sweep", false, "{sweep: \"R1\" | \"V1.dc\" | {param: name}, values | from, to, steps?, log? | series, run, measures, set?}", "one run per value, the measures as a table; nothing is written"),
    ("tune", false, "{tune: \"R1\", measure, target, from, to, series?, tol?, max_runs?, run, set?, write?}", "the value that brings a measure to its target (a search over runs), from an E series when named; `write: true` puts it on the schematic"),
    ("stats", false, "{stats: true | {thread?}}", "the simulation log's totals: actions, runs, ngspice time, failures, answer sizes and token cost per turn"),
];
const OP_NAMES: &str = "models, model, exclude, include, import, source, probe, analysis, run, loopgain, sweep, tune, stats";
/// The op's name: the key that says what it is. A tune and a sweep carry a `run` of their own, so they are looked for first.
pub fn verb(op: &Value) -> &'static str { let Some(o) = op.as_object() else { return "" }; ["tune", "sweep", "loopgain"].into_iter().find(|v| o.contains_key(*v)).or_else(|| OPS.iter().find(|v| o.contains_key(v.0)).map(|v| v.0)).unwrap_or("") }
/// Whether the ops change the design (its schematic, its saved analysis, its model files): what takes the document's lock and waits for the person.
pub fn writes(ops: &Value) -> bool {
    ops.as_array().into_iter().flatten().any(|o| match verb(o) { "tune" => o["write"] == true, "import" => !o["to"].is_null(), "analysis" => o["analysis"] != true, "probe" | "model" | "exclude" | "include" | "source" => true, _ => false })
}
/// The measures by name, for the tool's description.
pub fn measure_names() -> String { kw::MEASURES.iter().map(|m| m.0).collect::<Vec<_>>().join(", ") }

/// What the driver needs of the machine: the project's folder (model files, `sim/models/`), the folders of the libraries that ship
/// models (each with what the lookup calls it), the time, and the simulation log to read for `stats`.
#[derive(Clone, Debug, Default)]
pub struct Ctx { pub project_dir: PathBuf, pub libs: Vec<(PathBuf, &'static str)>, pub now: u64, pub log: Option<PathBuf>,
    /// The person's KiCad has the project: a `run` is shown in its Simulator unless the op says `show: false` (a sweep's and a tune's runs never are).
    pub gui: bool }

const BUSY: &str = "a simulation of this project is running (the person's, or another agent's): this one was not started. Ask again when it ends";
/// How long one request waits for a run before asking again (the API serves one request at a time), and what a run gets when none is given.
const WAIT_MS: u64 = 5000;
const TIMEOUT_MS: u64 = 120_000;
const DIGITS: usize = 6;

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn r6(v: f64) -> Value { json!(kw::round_sig(v, DIGITS)) }
fn round_all(v: &Value) -> Value { match v { Value::Number(n) => n.as_f64().map_or(v.clone(), r6), Value::Array(a) => Value::Array(a.iter().map(round_all).collect()), Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), round_all(x))).collect()), _ => v.clone() } }
/// References from text ("R1", "R1 R2", "R1,R2") or a list.
fn refs_of(v: &Value) -> Vec<String> {
    match v { Value::String(t) => t.split([',', ' ']).filter(|x| !x.is_empty()).map(String::from).collect(), Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(), _ => vec![] }
}
fn list(v: &[String], max: usize) -> Value { if v.len() <= max { json!(v) } else { json!({"count": v.len(), "first": v[..max]}) } }
/// A model's pin map as KiCad writes it: "1=+ 2=-".
fn pins_text(m: &Value) -> String { m["pins"].as_array().into_iter().flatten().filter(|p| !s(p, "model_node").is_empty()).map(|p| format!("{}={}", s(p, "pin_number"), s(p, "model_node"))).collect::<Vec<_>>().join(" ") }
/// One symbol's model as a short row.
fn model_row(m: &Value) -> Value {
    let mut r = json!({"ref": s(m, "reference"), "value": s(m, "value")});
    let dev = [s(m, "device"), s(m, "type")].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" ");
    if !dev.is_empty() { r["device"] = json!(dev) }
    if !s(m, "model_name").is_empty() { r["model"] = json!(s(m, "model_name")) }
    if !s(m, "library").is_empty() { r["lib"] = json!(s(m, "library")) }
    let pins = pins_text(m); if !pins.is_empty() && (m["pins"].as_array().map_or(0, |p| p.len()) > 2 || s(m, "source") == "MS_LIBRARY") { r["pins"] = json!(pins) }
    if m["exclude_from_sim"] == true || m["dnp"] == true { r["excluded"] = json!(true) }
    r
}
fn has_error(m: &Value) -> bool { !matches!(s(m, "error_kind"), "" | "ME_NONE") }
fn model_problem(m: &Value) -> String { kp::problem_line(&json!({"kind": s(m, "error_kind"), "reference": s(m, "reference"), "message": s(m, "error")})) }

// ---- signals ---------------------------------------------------------------------------------------------------------------

fn key(name: &str) -> String { name.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase() }
/// Whether a vector or value of a run is what was asked for: its ngspice name or KiCad's, in any case; a net alone for its voltage; and
/// a root sheet's net with or without its "/".
fn same_signal(asked: &str, v: &Value) -> bool {
    let a = key(asked); let (n, g) = (key(s(v, "name")), key(s(v, "signal")));
    // (ngspice names a node's voltage by the node alone: "/out")
    let inner = a.strip_prefix("v(").and_then(|x| x.strip_suffix(')')).unwrap_or("").to_string();
    let forms = [a.clone(), format!("v({a})"), format!("v(/{a})"), format!("/{a}"), a.replacen("(", "(/", 1), a.replacen("(/", "(", 1), inner.clone(), if inner.is_empty() { String::new() } else { format!("/{inner}") }, inner.trim_start_matches('/').to_string(), a.trim_start_matches('/').to_string()];
    forms.iter().any(|f| !f.is_empty() && (*f == n || *f == g))
}
/// The top-level `/` or `-` of a signal expression between two `X(...)` terms ("V(out)/V(in)", "V(a)-V(b)").
fn split_expr(e: &str) -> Option<(&str, char, &str)> {
    let mut depth = 0i32;
    for (i, c) in e.char_indices() { match c { '(' => depth += 1, ')' => depth -= 1,
        '/' | '-' if depth == 0 && i > 0 => { let (l, r) = (e[..i].trim(), e[i + 1..].trim()); if l.ends_with(')') && r.contains('(') { return Some((l, c, r)) } }
        _ => {} } }
    None
}
/// The vectors a signal expression reads.
fn leaves(e: &str) -> Vec<String> { match split_expr(e) { Some((l, _, r)) => { let mut v = leaves(l); v.extend(leaves(r)); v } None => vec![e.trim().to_string()] } }
/// The sweep axis of a run: its first vector with an axis kind, else its first vector.
fn axis(resp: &Value) -> Option<&Value> {
    let vs = resp["vectors"].as_array()?;
    vs.iter().find(|v| matches!(s(v, "kind"), "VK_TIME" | "VK_FREQUENCY" | "VK_SWEEP") && v["real"].as_array().is_some_and(|r| !r.is_empty())).or_else(|| vs.iter().find(|v| v["real"].as_array().is_some_and(|r| !r.is_empty())))
}
fn floats(v: &Value) -> Vec<f64> { v.as_array().into_iter().flatten().map(|x| match x { Value::String(t) => t.parse().unwrap_or(f64::NAN), o => o.as_f64().unwrap_or(f64::NAN) }).collect() }
fn names_of(resp: &Value) -> String {
    let mut n: Vec<&str> = resp["vectors"].as_array().into_iter().flatten().chain(resp["values"].as_array().into_iter().flatten()).map(|v| if s(v, "signal").is_empty() { s(v, "name") } else { s(v, "signal") }).filter(|x| !x.is_empty()).collect();
    n.dedup(); let more = n.len().saturating_sub(16); format!("{}{}", n.iter().take(16).copied().collect::<Vec<_>>().join(", "), if more > 0 { format!(" and {more} more") } else { String::new() })
}
/// One signal of a run over its axis: a vector, a single value (an operating point) as one point, or `A/B`, `A-B` of two.
pub fn sig_of(resp: &Value, expr: &str) -> Result<Sig, String> {
    if let Some((l, op, r)) = split_expr(expr) {
        let (a, b) = (sig_of(resp, l)?, sig_of(resp, r)?);
        if a.len() != b.len() { return Err(format!("{l} and {r} differ in length")) }
        let (ai, bi) = (a.im.clone().unwrap_or_else(|| vec![0.0; a.len()]), b.im.clone().unwrap_or_else(|| vec![0.0; b.len()])); let complex = a.im.is_some() || b.im.is_some();
        let (mut re, mut im) = (vec![], vec![]);
        for i in 0..a.len() { let (x, y) = if op == '-' { (a.y[i] - b.y[i], ai[i] - bi[i]) } else { let d = b.y[i] * b.y[i] + bi[i] * bi[i]; ((a.y[i] * b.y[i] + ai[i] * bi[i]) / d, (ai[i] * b.y[i] - a.y[i] * bi[i]) / d) }; re.push(x); im.push(y) }
        return Ok(Sig { x: a.x, y: re, im: complex.then_some(im), log: a.log })
    }
    if let Some(v) = resp["vectors"].as_array().into_iter().flatten().find(|v| same_signal(expr, v) && v["real"].as_array().is_some_and(|r| !r.is_empty())) {
        let ax = axis(resp).ok_or("the run answered no sweep axis")?; let (x, y) = (floats(&ax["real"]), floats(&v["real"]));
        if x.len() != y.len() { return Err(format!("{expr} has {} points and the axis {}", y.len(), x.len())) }
        let im = (v["complex"] == true || v["imag"].as_array().is_some_and(|i| !i.is_empty())).then(|| { let mut i = floats(&v["imag"]); i.resize(y.len(), 0.0); i });
        return Ok(Sig { x, y, im, log: s(ax, "kind") == "VK_FREQUENCY" })
    }
    if let Some(v) = resp["values"].as_array().into_iter().flatten().find(|v| same_signal(expr, v)) { return Ok(Sig::real(vec![0.0], vec![v["value"].as_f64().unwrap_or(f64::NAN)])) }
    Err(format!("no `{expr}` in this run; it has {}", names_of(resp)))
}

// ---- measures --------------------------------------------------------------------------------------------------------------

/// One measure of a run: the hub's own by name over a signal, or an ngspice `meas` statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Meas { Hub { measure: String, of: String, params: Value }, Raw(String) }
fn is_measure(name: &str) -> bool { kw::MEASURES.iter().any(|m| m.0 == name) || name == "f3db" }
/// `measures: {name: spec}`. A spec is `{corner: "V(out)", ...parameters}` (or `{measure, of, ...}`), the same in words
/// ("corner V(out)", "rms V(out) from=1m to=2m"), or the rest of an ngspice `meas` statement after its name ("TRIG v(in) VAL=0.5 RISE=1 TARG ...").
pub fn parse_measures(v: &Value) -> Result<Vec<(String, Meas)>, String> {
    let Some(o) = v.as_object() else { return if v.is_null() { Ok(vec![]) } else { Err("measures is {name: spec}, a spec being {corner: \"V(out)\"}, \"corner V(out)\" or the rest of an ngspice meas statement".into()) } };
    let mut out = vec![];
    for (name, spec) in o {
        out.push((name.clone(), match spec {
            Value::Object(m) => {
                let (measure, of) = match (m.get("measure").and_then(Value::as_str), m.get("of").and_then(Value::as_str)) { (Some(k), Some(of)) => (k.to_string(), of.to_string()),
                    _ => m.iter().find(|(k, x)| is_measure(k) && x.is_string()).map(|(k, x)| (k.clone(), x.as_str().unwrap_or("").to_string())).ok_or(format!("measure `{name}`: name one of {} with its signal, e.g. {{\"corner\": \"V(out)\"}}", measure_names()))? };
                if !is_measure(&measure) { return Err(format!("measure `{name}`: no measure `{measure}` ({})", measure_names())) }
                let params: Map<String, Value> = m.iter().filter(|(k, _)| *k != &measure && k.as_str() != "measure" && k.as_str() != "of").map(|(k, x)| (k.clone(), if x.is_string() && !matches!(k.as_str(), "dir" | "ref") { kp::num_of(x).map_or(x.clone(), |n| json!(n)) } else { x.clone() })).collect();
                Meas::Hub { measure, of, params: Value::Object(params) } }
            Value::String(t) => { let mut words = t.split_whitespace(); let first = words.next().unwrap_or("");
                if is_measure(&first.to_ascii_lowercase()) && !t.contains(" TRIG ") && !t.to_ascii_uppercase().contains(" WHEN ") {
                    let rest: Vec<&str> = words.collect(); let of: Vec<&str> = rest.iter().take_while(|w| !w.contains('=')).copied().collect();
                    if of.is_empty() { return Err(format!("measure `{name}`: `{first}` of which signal? e.g. \"{first} V(out)\"")) }
                    let params: Map<String, Value> = rest.iter().filter_map(|w| w.split_once('=')).map(|(k, x)| (k.to_string(), match x { "true" => json!(true), "false" => json!(false), _ => kp::parse_value(x).map_or(json!(x), |n| json!(n)) })).collect();
                    Meas::Hub { measure: first.to_ascii_lowercase(), of: of.join(""), params: Value::Object(params) }
                } else { let t = t.trim().trim_start_matches(".meas").trim_start_matches("meas").trim();
                    let mut w = t.splitn(2, ' '); let (a, rest) = (w.next().unwrap_or(""), w.next().unwrap_or(""));
                    Meas::Raw(if matches!(a.to_ascii_lowercase().as_str(), "tran" | "ac" | "dc" | "op" | "sp" | "noise") { format!("{a} {name} {rest}") } else { format!("{name} {t}") }) } }
            _ => return Err(format!("measure `{name}`: a spec is {{corner: \"V(out)\"}}, \"corner V(out)\" or the rest of an ngspice meas statement")),
        }))
    }
    Ok(out)
}
/// The measures of a finished run: {name: number}, a measure that could not be taken as "failed: why".
pub fn measures_of(resp: &Value, meas: &[(String, Meas)]) -> Value {
    let mut out = Map::new();
    for (name, m) in meas {
        let v = match m {
            Meas::Hub { measure, of, params } => sig_of(resp, of).and_then(|sig| kw::measure(measure, params, &sig)).map(|v| round_all(&v)),
            Meas::Raw(_) => match resp["measures"].as_array().into_iter().flatten().find(|x| s(x, "name").eq_ignore_ascii_case(name)) {
                Some(x) if x["failed"] != true => Ok(r6(x["value"].as_f64().unwrap_or(f64::NAN))),
                Some(x) => Err(if s(x, "error").is_empty() { "ngspice gave no value (the condition never happened in the data)".to_string() } else { s(x, "error").chars().take(160).collect() }),
                None => Err("ngspice answered no such measure".into()) },
        };
        out.insert(name.clone(), v.unwrap_or_else(|e| json!(format!("failed: {e}"))));
    }
    Value::Object(out)
}
/// A run's problems as lines: the fork's, else what ngspice's console says, else the status in words. Errors first, eight at most.
pub fn problem_lines(resp: &Value) -> Vec<String> {
    let mut ps: Vec<&Value> = resp["problems"].as_array().into_iter().flatten().collect(); ps.sort_by_key(|p| s(p, "severity") == "PS_WARNING");
    let mut out: Vec<String> = vec![];
    for p in ps { let mut q = p.clone(); if !s(p, "net").is_empty() { q["node"] = p["net"].clone() } let l = kp::problem_line(&q); if !out.contains(&l) { out.push(l) } }
    if out.is_empty() && s(resp, "status") != "SIM_OK" { for p in kp::problems_of_log(s(resp, "log")) { let l = kp::problem_line(&p); if !out.contains(&l) { out.push(l) } } }
    if out.is_empty() && s(resp, "status") != "SIM_OK" { out.push(kp::status_words(s(resp, "status")).to_string()) }
    out.truncate(8); out
}
/// A failed run's lines: its problems, and what was tried for it.
fn failure_lines(resp: &Value) -> Vec<String> {
    let mut l = problem_lines(resp);
    if let Some(t) = resp["tried"].as_array().filter(|t| !t.is_empty()) { l.push(format!("tried without success: {}", t.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("; then "))) }
    l
}
/// What a transient that did not converge is tried again with, in order (label, the options line); the last also shortens the step.
pub const RETRY: &[(&str, &str)] = &[("gear integration", ".options method=gear"), ("gear with reltol 0.01", ".options method=gear reltol=0.01"), ("gear, reltol 0.01, looser tolerances and a maximum step of the print step", ".options method=gear reltol=0.01 abstol=1e-9 vntol=1e-4")];
/// A `.tran` line with its maximum step no longer than its print step (`.tran 1u 10m` -> `.tran 1u 10m 0 1u`); other lines as they are.
pub fn smaller_step(command: &str) -> String {
    let mut lines = command.lines(); let first = lines.next().unwrap_or(""); let w: Vec<&str> = first.split_whitespace().collect();
    if w.len() < 3 { return command.to_string() }
    let uic = w.iter().skip(3).any(|x| x.eq_ignore_ascii_case("uic")); let nums: Vec<&str> = w[3..].iter().filter(|x| !x.eq_ignore_ascii_case("uic")).copied().collect();
    let (start, max) = (nums.first().copied().unwrap_or("0"), nums.get(1).copied());
    let max = match (max.and_then(kp::parse_value), kp::parse_value(w[1])) { (Some(m), Some(t)) if m <= t => max.unwrap(), _ => w[1] };
    let mut out = format!("{} {} {} {start} {max}{}", w[0], w[1], w[2], if uic { " uic" } else { "" });
    for l in lines { out.push('\n'); out.push_str(l) }
    out
}
/// An analysis line as ngspice takes it: its leading dot added, its first word one of ngspice's analyses.
pub fn command_of(text: &str) -> Result<String, String> {
    let t = text.trim(); if t.is_empty() { return Err("an analysis is a line such as \".tran 1u 10m\", \".ac dec 20 1 1meg\", \".op\", \".dc V1 0 5 0.1\"".into()) }
    let t = if t.starts_with('.') { t.to_string() } else { format!(".{t}") };
    let word = t[1..].split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    if !matches!(word.as_str(), "op" | "tran" | "ac" | "dc" | "noise" | "pz" | "tf" | "sp" | "disto" | "sens") { return Err(format!("`.{word}` is not an analysis: .op, .tran, .ac, .dc, .noise, .pz, .tf, .sp (an FFT is a .tran line, then a line \"fft v(out)\")")) }
    Ok(t)
}
/// `set: {R1: "2k", "V1.dc": 3.3}` as overrides: a part's value, or one of its parameters after a dot.
pub fn overrides_of(set: &Value) -> Result<Vec<Value>, String> {
    let Some(o) = set.as_object() else { return if set.is_null() { Ok(vec![]) } else { Err("set is {reference: value}, e.g. {\"R1\": \"2k\", \"V1.dc\": 3.3}".into()) } };
    Ok(o.iter().map(|(k, v)| { let (r, f) = k.split_once('.').unwrap_or((k, "")); json!({"reference": r, "field": f, "value": match v { Value::String(t) => t.clone(), Value::Number(n) => kp::plain(n.as_f64().unwrap_or(0.0)), o => o.to_string() }}) }).collect())
}
fn override_of(target: &str, v: f64) -> Value { let (r, f) = target.split_once('.').unwrap_or((target, "")); json!({"reference": r, "field": f, "value": kp::plain(v)}) }

// ---- the driver ------------------------------------------------------------------------------------------------------------

struct Sim<'a> { c: &'a dyn Call, path: &'a str, ctx: &'a Ctx, doc: Value,
    /// Whether the document was changed through the simulation commands (saved at the end), the path a nested edit already saved, and
    /// whether a model or an exclusion changed (the answer then says what still does not simulate).
    wrote: bool, saved: Option<String>, models_changed: bool,
    /// Signals named by this call's probes (its runs read them too), each op's log entry, and what the current op ran and looked up.
    probes: Vec<String>, log: Vec<Value>, runs: Vec<Value>, lookups: Vec<Value>,
    /// The pins the next runs take off their nets (a loop gain's cut), and what a run that did not converge was tried again with.
    breaks: Vec<Value>, retries: Vec<String>,
    /// The model files of the project and the libraries, read once a call.
    scanned: Option<Vec<(String, &'static str, Vec<kp::ModelDef>)>> }

impl Sim<'_> {
    /// One command; KiCad's task gate (a check or a refill is running) is waited out as `kicad_edit` does.
    async fn call(&self, name: &str, msg: Value) -> Result<Value, String> {
        let ty = if name.contains('.') { name.to_string() } else { format!("{P}{name}") }; let until = std::time::Instant::now() + kedit::BUSY_WAIT;
        loop { match self.c.call(&ty, msg.clone()).await { Err(e) if kedit::task_busy(&e) && std::time::Instant::now() < until => tokio::time::sleep(kedit::BUSY_STEP).await, r => return r } }
    }
    async fn models(&self, refs: &[String], only_problems: bool) -> Result<Value, String> {
        let r = self.call("GetSimulationModels", json!({"document": self.doc, "references": refs, "only_problems": only_problems})).await?;
        if let Some(nf) = r["not_found"].as_array().filter(|a| !a.is_empty()) { return Err(format!("no part {} in this schematic", nf.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))) }
        Ok(r)
    }
    /// The model files on this machine: the project's own, then the libraries'.
    fn scanned(&mut self) -> &Vec<(String, &'static str, Vec<kp::ModelDef>)> {
        if self.scanned.is_none() {
            let mut all = vec![];
            for (p, defs) in kp::scan(&self.ctx.project_dir, 3) { all.push((p.strip_prefix(&self.ctx.project_dir).unwrap_or(&p).to_string_lossy().into_owned(), "project", defs)) }
            for (dir, what) in &self.ctx.libs { for (p, defs) in kp::scan(dir, 2) { all.push((p.to_string_lossy().into_owned(), *what, defs)) } }
            self.scanned = Some(all);
        }
        self.scanned.as_ref().unwrap()
    }
    /// The symbols' MPN and Manufacturer fields by reference (the simulation commands answer only the Sim.* fields).
    async fn part_numbers(&self) -> std::collections::HashMap<String, (String, String)> {
        let mut out = std::collections::HashMap::new();
        let Ok(r) = self.call(&format!("{C}GetItems"), json!({"header": {"document": self.doc}, "types": ["KOT_SCH_SYMBOL"]})).await else { return out };
        for i in r["items"].as_array().into_iter().flatten() {
            let field = |names: &[&str]| i["user_fields"].as_array().into_iter().flatten().find(|f| names.iter().any(|n| s(f, "name").eq_ignore_ascii_case(n))).map(|f| f["text"]["text"].as_str().unwrap_or("").to_string()).unwrap_or_default();
            let (mpn, maker) = (field(&["MPN", "Manufacturer Part Number", "Manufacturer_Part_Number", "PartNumber", "Part Number", "MFR_PN"]), field(&["Manufacturer", "Mfr", "MFR"]));
            if !mpn.is_empty() { out.insert(i["reference_field"]["text"]["text"].as_str().unwrap_or("").to_string(), (mpn, maker)); }
        }
        out
    }
    /// The references an exclude or include names: references (with `*`), a group's parts, or the parts a pattern matches.
    async fn pick(&self, target: &Value) -> Result<Vec<String>, String> {
        let words = refs_of(target);
        if !target.is_object() && !words.is_empty() && words.iter().all(|w| !w.contains(['*', '?']) && w != "no_model" && !kp::GROUPS.iter().any(|g| g.0 == w)) { return Ok(words) }
        let all = self.models(&[], false).await?; let rows: Vec<&Value> = all["models"].as_array().into_iter().flatten().collect();
        let mut out: Vec<String> = vec![];
        fn add(out: &mut Vec<String>, r: &str) { if !out.iter().any(|x| x == r) { out.push(r.to_string()) } }
        if let Some(sel) = target.as_object() {
            for k in sel.keys() { if !matches!(k.as_str(), "ref" | "lib" | "value") { return Err(format!("a pattern is {{ref|lib|value: text with * and ?}}, not `{k}`")) } }
            for m in &rows { if sel.iter().all(|(k, want)| { let got = s(m, match k.as_str() { "ref" => "reference", "lib" => "lib_id", _ => "value" }); refs_of(want).iter().any(|w| kedit::glob(w, got)) }) { add(&mut out, s(m, "reference")) } }
        } else if words.is_empty() { return Err(format!("name references, a group ({}) or a pattern {{ref|lib|value}}", kp::GROUPS.iter().map(|g| g.0).collect::<Vec<_>>().join(", "))) }
        for w in &words {
            if w == "no_model" { for m in &rows { if s(m, "error_kind") == "ME_NO_MODEL" && kp::NOT_ELECTRICAL.iter().any(|g| kp::in_group(g, s(m, "reference"), s(m, "lib_id")) == Some(true)) { add(&mut out, s(m, "reference")) } } }
            else if kp::GROUPS.iter().any(|g| g.0 == w) { for m in &rows { if kp::in_group(w, s(m, "reference"), s(m, "lib_id")) == Some(true) { add(&mut out, s(m, "reference")) } } }
            else { let n = out.len(); for m in &rows { if kedit::glob(w, s(m, "reference")) { add(&mut out, s(m, "reference")) } } if out.len() == n && !w.contains(['*', '?']) { return Err(format!("no part {w} in this schematic")) } }
        }
        Ok(out)
    }
    /// AssignSimulationModel, its failures as one error.
    async fn assign(&mut self, mut msg: Value) -> Result<Value, String> {
        msg["document"] = self.doc.clone();
        let r = self.call("AssignSimulationModel", msg).await?;
        let bad: Vec<String> = r["results"].as_array().into_iter().flatten().filter(|x| !s(x, "error").is_empty() || has_error(x)).map(|x| kp::problem_line(&json!({"kind": s(x, "error_kind"), "reference": s(x, "reference"), "message": s(x, "error")}))).collect();
        if !bad.is_empty() { return Err(format!("{}. Nothing was written", bad.join("; "))) }
        if r["changed"].as_i64().unwrap_or(0) > 0 || r["results"].as_array().is_some_and(|a| a.iter().any(|x| x["changed"] == true)) { self.wrote = true; self.models_changed = true }
        Ok(r)
    }
    /// A nested `kicad_edit` (a source symbol, a label, a written value): its own commit and save, without its check.
    async fn edit(&mut self, ops: Value, message: &str) -> Result<Value, String> {
        let r = kedit::run(self.c, self.path, &json!({"ops": ops, "message": message, "check": false})).await?;
        if let Some(p) = r["saved"].as_str() { self.saved = Some(p.to_string()) }
        Ok(r)
    }
    /// The model op's library form: the file checked, the name settled, the pins by name where the names say.
    async fn assign_library(&mut self, refs: &[String], lib: &str, name: Option<&str>, pins: &Value, params: &Value) -> Result<Value, String> {
        let abs = if Path::new(lib).is_absolute() || lib.contains("${") { PathBuf::from(lib) } else { self.ctx.project_dir.join(lib) };
        let stored = abs.strip_prefix(&self.ctx.project_dir).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| lib.to_string());
        let defs = if lib.contains("${") { vec![] } else { let text = std::fs::read(&abs).map_err(|e| format!("cannot read the model file {}: {e}", abs.display()))?; let text = String::from_utf8_lossy(&text).into_owned();
            if let Some(why) = kp::unusable(&text) { return Err(why) } kp::models_in(&text) };
        let name = match name { Some(n) => n.to_string(), None => { let subs: Vec<&kp::ModelDef> = defs.iter().filter(|d| d.subckt).collect();
            match (subs.len(), defs.len()) { (1, _) => subs[0].name.clone(), (0, 1) => defs[0].name.clone(), _ => return Err(format!("which model of {stored}? name one: {}", defs.iter().take(20).map(|d| d.name.as_str()).collect::<Vec<_>>().join(", "))) } } };
        let def = defs.iter().find(|d| d.name.eq_ignore_ascii_case(&name)).cloned();
        if def.is_none() && !defs.is_empty() { return Err(format!("{stored} has no model `{name}`; it has {}", defs.iter().take(20).map(|d| d.name.as_str()).collect::<Vec<_>>().join(", "))) }
        // the pin map: as given; else by the names when the symbol's pin names are the subcircuit's nodes; else KiCad's own (by order)
        let mut map: Vec<Value> = pins.as_object().into_iter().flatten().map(|(p, n)| json!({"pin_number": p, "model_node": n.as_str().map(String::from).unwrap_or_else(|| n.to_string())})).collect();
        let mut how = if map.is_empty() { "order" } else { "given" };
        if let (true, Some(d)) = (map.is_empty(), def.as_ref().filter(|d| d.subckt)) {
            if let Ok(now) = self.models(&refs[..1], false).await { let sym: Vec<(String, String)> = now["models"][0]["pins"].as_array().into_iter().flatten().map(|p| (s(p, "pin_number").to_string(), s(p, "pin_name").to_string())).collect();
                if let Some(m) = kp::pins_by_name(&d.pins, &sym) { map = m.into_iter().map(|(p, n)| json!({"pin_number": p, "model_node": n})).collect(); how = "name" } }
        }
        let params: Vec<Value> = params.as_object().into_iter().flatten().map(|(k, v)| json!({"name": k, "value": v.as_str().map(String::from).unwrap_or_else(|| v.to_string())})).collect();
        let r = self.assign(json!({"references": refs, "library": {"library": stored, "name": name, "params": params}, "pins": map})).await?;
        let mut out = json!({"model": r["results"].as_array().into_iter().flatten().map(|x| model_row(&x["model"])).collect::<Vec<_>>()});
        if how == "name" { out["pins_by"] = json!("name") }
        if how == "order" && def.as_ref().is_some_and(|d| d.subckt && d.pins.len() > 2) {
            out["pins_by"] = json!("order"); out["nodes"] = json!(def.unwrap().pins);
            out["check"] = json!("symbol pin N went to the model's node N: read the nodes' meaning in the model file's comments and give pins: {pin number: node} if the symbol's numbering differs") }
        Ok(out)
    }
    /// One run to its end: RunSimulation, then GetSimulationResult while ngspice works. Ok is the response, whatever its status.
    async fn exec(&mut self, command: &str, directives: &Value, overrides: Vec<Value>, signals: &[String], raw: &[String], op: &Value) -> Result<Value, String> {
        let mut r = self.once(command, directives, overrides.clone(), signals, raw, op).await?;
        // a transient that did not converge is tried again, each step adding to the last, and the answer says what it took
        if s(&r, "status") == "SIM_NO_CONVERGENCE" && command.trim_start().to_ascii_lowercase().starts_with(".tran") && op["retry"] != false {
            let mut tried = vec![];
            for (n, (label, options)) in RETRY.iter().enumerate() {
                let mut dir = directives.as_array().cloned().unwrap_or_default(); dir.push(json!(options));
                let cmd = if n == RETRY.len() - 1 { smaller_step(command) } else { command.to_string() };
                tried.push(label.to_string()); self.retries.push(label.to_string());
                let again = self.once(&cmd, &json!(dir), overrides.clone(), signals, raw, op).await?;
                if s(&again, "status") == "SIM_OK" { r = again; r["needed"] = json!(format!("{label}: it did not converge without ({options}{})", if cmd != command { format!(", {cmd}") } else { String::new() })); break }
                if n == RETRY.len() - 1 { r = again }
            }
            if s(&r, "status") != "SIM_OK" { r["tried"] = json!(tried) }
        }
        Ok(r)
    }
    async fn once(&mut self, command: &str, directives: &Value, overrides: Vec<Value>, signals: &[String], raw: &[String], op: &Value) -> Result<Value, String> {
        let timeout = op["timeout"].as_f64().map_or(TIMEOUT_MS, |t| (t * 1000.0) as u64).clamp(1000, 3_600_000);
        let mut msg = json!({"document": self.doc, "command": command, "directives": directives.as_array().cloned().unwrap_or_default(), "overrides": overrides, "signals": signals, "measures": raw,
            "include_netlist": op["netlist"] == true, "show": verb(op) == "run" && op["show"].as_bool().unwrap_or(self.ctx.gui), "timeout_ms": timeout, "wait_ms": WAIT_MS, "max_points": op["max_points"].as_u64().unwrap_or(0)});
        if op["options"].is_object() { msg["options"] = op["options"].clone() }
        if !self.breaks.is_empty() { msg["breaks"] = json!(self.breaks); msg["show"] = json!(false) }
        let started = std::time::Instant::now();
        let mut r = match self.call("RunSimulation", msg).await { Err(e) if e.contains("AS_BUSY") => return Err(BUSY.into()), r => r? };
        while s(&r, "status") == "SIM_RUNNING" {
            let id = r["run_id"].clone();
            if started.elapsed().as_millis() as u64 > timeout + 15_000 { let _ = self.call("StopSimulation", json!({"document": self.doc, "run_id": id})).await; }
            if started.elapsed().as_millis() as u64 > timeout + 30_000 { return Err("the run did not stop when asked: KiCad's simulator is still busy with it".into()) }
            r = self.call("GetSimulationResult", json!({"document": self.doc, "run_id": id, "wait_ms": WAIT_MS})).await?;
        }
        let t = &r["timings"];
        self.runs.push(json!({"status": s(&r, "status"), "analysis": s(&r, "analysis"), "netlist_ms": t["netlist_ms"], "load_ms": t["load_ms"], "run_ms": t["run_ms"], "total_ms": t["total_ms"], "points": r["points"], "hub_ms": started.elapsed().as_millis() as u64}));
        Ok(r)
    }
    /// The parts of a run every op shares: its command (`true`: the saved one), directives, fixed overrides, measures and the signals those read.
    fn plan(&self, op: &Value, run: &Value, extra: &[String]) -> Result<(String, Vec<Value>, Vec<(String, Meas)>, Vec<String>, Vec<String>), String> {
        let command = match run { Value::String(t) => command_of(t)?, Value::Bool(true) | Value::Null => String::new(), _ => return Err("run is an analysis line (\".tran 1u 10m\") or true for the saved one".into()) };
        let meas = parse_measures(&op["measures"])?;
        let mut signals: Vec<String> = vec![]; let mut add = |n: &str| if !signals.iter().any(|x| key(x) == key(n)) { signals.push(n.to_string()) };
        for n in extra { add(n) }
        for (_, m) in &meas { if let Meas::Hub { of, .. } = m { for l in leaves(of) { add(&l) } } }
        let raw = meas.iter().filter_map(|(_, m)| match m { Meas::Raw(t) => Some(t.clone()), _ => None }).collect();
        Ok((command, overrides_of(&op["set"])?, meas, signals, raw))
    }
    /// A failed run in one line.
    fn failed(resp: &Value) -> String { format!("{}: {}", kp::status_words(s(resp, "status")), failure_lines(resp).join("; ")) }

    async fn op(&mut self, op: &Value) -> Result<Value, String> {
        match verb(op) {
            "models" => { let t = &op["models"]; let refs = if t.is_object() { refs_of(&t["refs"]) } else { refs_of(t).into_iter().filter(|w| w != "all" && w != "problems").collect() };
                let every = !refs.is_empty() || t == "all" || t["all"] == true;
                let r = self.models(&refs, !every).await?; let rows: Vec<&Value> = r["models"].as_array().into_iter().flatten().collect();
                let wrong: Vec<&&Value> = rows.iter().filter(|m| has_error(m)).collect();
                let numbers = if wrong.iter().any(|m| matches!(s(m, "error_kind"), "ME_NO_MODEL" | "ME_LIBRARY_NOT_FOUND" | "ME_MODEL_NOT_IN_LIBRARY")) { self.part_numbers().await } else { Default::default() };
                let (mut parts, mut vendor) = (vec![], false);
                for m in &rows {
                    if !has_error(m) { if every { parts.push(model_row(m)) } continue }
                    let mut row = json!({"ref": s(m, "reference"), "value": s(m, "value"), "lib": s(m, "lib_id"), "why": model_problem(m)});
                    if matches!(s(m, "error_kind"), "ME_NO_MODEL" | "ME_LIBRARY_NOT_FOUND" | "ME_MODEL_NOT_IN_LIBRARY") {
                        let (mpn, maker) = numbers.get(s(m, "reference")).cloned().unwrap_or_default();
                        if !mpn.is_empty() { row["mpn"] = json!(mpn); if !maker.is_empty() { row["maker"] = json!(maker) } }
                        let entry = s(m, "lib_id").rsplit(':').next().unwrap_or(""); let wanted = m["fields"].as_array().into_iter().flatten().find(|f| s(f, "name") == "Sim.Name").map(|f| s(f, "value")).unwrap_or("");
                        let names = [mpn.as_str(), s(m, "value"), entry, wanted];
                        let found = kp::lookup(&names, self.scanned());
                        self.lookups.push(json!({"ref": s(m, "reference"), "names": names.iter().filter(|n| !n.is_empty()).collect::<Vec<_>>(), "found": found.iter().map(|f| json!({"file": f.file, "name": f.def.name, "from": f.source})).collect::<Vec<_>>()}));
                        if found.is_empty() { vendor = true } else { row["found"] = json!(found.iter().map(kp::Found::row).collect::<Vec<_>>()) }
                    }
                    parts.push(row);
                }
                let more = parts.len().saturating_sub(60); parts.truncate(60);
                let mut out = json!({"symbols": r["symbols"], "excluded": r["excluded"], "problems": r["problems"].as_i64().unwrap_or(wrong.len() as i64), "parts": parts});
                if more > 0 { out["more"] = json!(more) }
                if vendor { out["next"] = json!(VENDOR_RULE) }
                Ok(json!({"models": out})) }
            "model" => { let refs = refs_of(&op["model"]); if refs.is_empty() { return Err("model names the parts: {model: \"U1\", ...}".into()) }
                let pins: Vec<Value> = op["pins"].as_object().into_iter().flatten().map(|(p, n)| json!({"pin_number": p, "model_node": n.as_str().map(String::from).unwrap_or_else(|| n.to_string())})).collect();
                let params = |v: &Value| -> Vec<Value> { v.as_object().into_iter().flatten().map(|(k, x)| json!({"name": k, "value": x.as_str().map(String::from).unwrap_or_else(|| x.to_string())})).collect() };
                if let Some(lib) = op["lib"].as_str() { return self.assign_library(&refs, lib, op["name"].as_str(), &op["pins"], &op["params"]).await }
                let msg = if op["clear"] == true { json!({"references": refs, "clear": true}) }
                    else if let Some(d) = op["device"].as_str() { json!({"references": refs, "builtin": {"device": d.to_ascii_uppercase(), "type": op["type"].as_str().unwrap_or("").to_ascii_uppercase(), "params": params(&op["params"])}, "pins": pins}) }
                    else if op["raw"].is_object() { json!({"references": refs, "raw": {"primitive": s(&op["raw"], "primitive"), "model": s(&op["raw"], "model"), "library": s(&op["raw"], "library")}, "pins": pins}) }
                    else { return Err("a model is one of: device (a built-in: R, C, L, D, V, I, NPN, NMOS, SW ...) with params; lib (a model file) with name; raw: {primitive, model}; clear: true".into()) };
                let r = self.assign(msg).await?;
                Ok(json!({"model": r["results"].as_array().into_iter().flatten().map(|x| model_row(&x["model"])).collect::<Vec<_>>()})) }
            v @ ("exclude" | "include") => { let refs = self.pick(&op[v]).await?;
                if refs.is_empty() { return Ok(json!({(if v == "exclude" { "excluded" } else { "included" }): [], "note": "no part matches"})) }
                self.assign(json!({"references": refs, "exclude_from_sim": v == "exclude"})).await?;
                Ok(json!({(if v == "exclude" { "excluded" } else { "included" }): list(&refs, 40)})) }
            "import" => { let i = &op["import"];
                let text = match (i["text"].as_str(), i["file"].as_str()) { (Some(t), _) => t.to_string(),
                    (None, Some(f)) => { let p = if Path::new(f).is_absolute() { PathBuf::from(f) } else { self.ctx.project_dir.join(f) };
                        if p.metadata().map_or(0, |m| m.len()) > 8 << 20 { return Err(format!("{f} is over 8 MB: not a model file (an archive? unpack it and give the .lib inside)")) }
                        String::from_utf8_lossy(&std::fs::read(&p).map_err(|e| format!("cannot read {}: {e}", p.display()))?).into_owned() }
                    _ => return Err("import takes the model as `file` (a path) or `text`, with `url` (where it came from) and `licence` (the source's licence line)".into()) };
                let first = kp::models_in(&text).first().map(|d| d.name.clone()).unwrap_or_default();
                let name = kp::store_name(s(i, "as"), if i["file"].is_string() && s(i, "as").is_empty() { s(i, "file") } else { s(i, "url") }, &first);
                let (stored, defs) = kp::store(&self.ctx.project_dir, &name, &text, s(i, "url"), s(i, "licence"), self.ctx.now)?;
                self.scanned = None; self.lookups.push(json!({"stored": stored, "url": s(i, "url"), "from": "vendor"}));
                let mut out = json!({"stored": stored, "source": format!("{stored}.source.json"), "models": defs.iter().take(12).map(kp::ModelDef::row).collect::<Vec<_>>()});
                if defs.len() > 12 { out["more"] = json!(defs.len() - 12) }
                let warn = kp::file_warnings(&text); if !warn.is_empty() { out["warnings"] = json!(warn) }
                if s(i, "licence").is_empty() { out["licence"] = json!("none given: add the source's licence line to the note if it states one") }
                let to = refs_of(&op["to"]);
                if !to.is_empty() { let a = self.assign_library(&to, &stored, op["name"].as_str(), &op["pins"], &op["params"]).await?; for (k, v) in a.as_object().into_iter().flatten() { out[if k == "model" { "assigned" } else { k.as_str() }] = v.clone() } }
                Ok(json!({"import": out})) }
            "source" => { let v = &op["source"]; let ty = v["type"].as_str().unwrap_or("dc").to_ascii_uppercase(); let letter = if v["current"] == true { "I" } else { "V" };
                if !matches!(ty.as_str(), "DC" | "SIN" | "PULSE" | "PWL" | "EXP" | "AM" | "SFFM" | "TRNOISE" | "TRRANDOM") { return Err(format!("no source type `{}`: dc, sin, pulse, pwl, exp, am, sffm, trnoise, trrandom", ty.to_ascii_lowercase())) }
                let pos = v["pos"].as_str().ok_or("source.pos: the net its + pin goes to")?; let neg = v["neg"].as_str().unwrap_or("GND");
                let reference = match v["ref"].as_str() { Some(r) => r.to_string(), None => { let all = self.models(&[], false).await?;
                    let used: Vec<u64> = all["models"].as_array().into_iter().flatten().filter_map(|m| s(m, "reference").strip_prefix(letter).and_then(|n| n.parse().ok())).collect(); format!("{letter}{}", used.iter().max().map_or(1, |n| n + 1)) } };
                let mut params: Map<String, Value> = v["params"].as_object().cloned().unwrap_or_default();
                if let Some(x) = v.get("value").filter(|x| !x.is_null()) { params.insert("dc".into(), x.clone()); }
                if let Some(x) = v.get("ac").filter(|x| !x.is_null()) { params.insert("ac".into(), x.clone()); }
                let at = if v["at"].is_array() { v["at"].clone() } else { json!([38.1, 50.8]) };
                let e = self.edit(json!([{"add": [{"symbol": {"lib": format!("Simulation_SPICE:{letter}{ty}"), "ref": reference, "at": at, "pins": {"1": pos, "2": neg}}}]}]), &format!("kicad_sim: source {reference}")).await?;
                // a source is for the simulation only: out of the BOM and off the board, unless asked otherwise
                if v["bom"] != true { self.edit(json!([{"set": reference, "raw": {"attributes": {"exclude_from_bill_of_materials": true, "exclude_from_board": true}}}]), &format!("kicad_sim: source {reference} is not a part")).await?; }
                let mut out = json!({"ref": reference, "pos": pos, "neg": neg});
                if let Some(to) = e["results"][0]["moved"].as_array().or(e["results"][0]["created"][0]["at"].as_array()) { out["at"] = json!(to) }
                if !params.is_empty() || ty != "DC" {
                    let ps: Vec<Value> = params.iter().map(|(k, x)| json!({"name": k, "value": x.as_str().map(String::from).unwrap_or_else(|| kp::num_of(x).map_or(x.to_string(), kp::plain))})).collect();
                    let r = self.assign(json!({"references": [reference], "builtin": {"device": letter, "type": ty, "params": ps}})).await?;
                    let m = &r["results"][0]["model"]; out["model"] = json!(format!("{} {}", s(m, "device"), s(m, "type")).trim());
                    let line = s(m, "netlist_line"); if !line.is_empty() { out["line"] = json!(line) }
                }
                Ok(json!({"source": out})) }
            "probe" => { let t = &op["probe"]; let mut names = refs_of(t); let mut out = json!({});
                if let Some(pin) = t["pin"].as_str() { let name = t["name"].as_str().ok_or("a probe on a pin needs a name for its net: {pin: \"R1.2\", name: \"MID\"}")?;
                    self.edit(json!([{"add": [{"label": {"text": name, "at": pin}}]}]), &format!("kicad_sim: probe {name}")).await?;
                    names.push(format!("V({name})")); out["label"] = json!({"text": name, "at": pin}) }
                if names.is_empty() { return Err("probe names signals (\"V(out)\", [\"V(out)\", \"I(R1)\"]) or a pin to label: {pin: \"R1.2\", name: \"MID\"}".into()) }
                for n in &names { if !self.probes.iter().any(|p| key(p) == key(n)) { self.probes.push(n.clone()) } }
                // they join the saved analysis' traces, when one is saved
                let setup = self.call("GetSimulationSetup", json!({"document": self.doc})).await?;
                match setup["setup"]["tabs"].as_array().and_then(|t| t.first()) {
                    Some(tab) => { let mut sigs: Vec<String> = tab["signals"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
                        for n in &names { if !sigs.iter().any(|x| key(x) == key(n)) { sigs.push(n.clone()) } }
                        let r = self.call("SetSimulationSetup", json!({"document": self.doc, "command": s(tab, "command"), "options": tab["options"], "signals": sigs, "measurements": tab["measurements"], "tab": 0})).await?;
                        out["signals"] = r["setup"]["tabs"][0]["signals"].clone(); out["saved_with"] = json!(s(tab, "command")) }
                    None => { out["signals"] = json!(names); out["note"] = json!("no analysis is saved yet: these are read by this call's runs; an analysis op in this call saves them with it") }
                }
                Ok(json!({"probe": out})) }
            "analysis" => { let t = &op["analysis"];
                if *t == json!(true) { let r = self.call("GetSimulationSetup", json!({"document": self.doc})).await?; let su = &r["setup"];
                    let mut out = json!({"tabs": su["tabs"].as_array().into_iter().flatten().map(|x| json!({"command": s(x, "command"), "signals": x["signals"]})).collect::<Vec<_>>()});
                    if !s(su, "schematic_command").is_empty() { out["on_schematic"] = json!(s(su, "schematic_command")) }
                    if su["schematic_directives"].as_array().is_some_and(|d| !d.is_empty()) { out["directives"] = su["schematic_directives"].clone() }
                    return Ok(json!({"analysis": out})) }
                let command = command_of(t.as_str().ok_or("analysis is a line (\".ac dec 20 1 1meg\") or true to read what is saved")?)?;
                let mut signals = refs_of(&op["signals"]); for p in &self.probes { if !signals.iter().any(|x| key(x) == key(p)) { signals.push(p.clone()) } }
                let mut msg = json!({"document": self.doc, "command": command, "signals": signals});
                if op["options"].is_object() { msg["options"] = op["options"].clone() } if let Some(n) = op["tab"].as_u64() { msg["tab"] = json!(n) } if let Some(c) = op["compatibility"].as_str() { msg["compatibility"] = json!(c) }
                let r = self.call("SetSimulationSetup", msg).await?; let tab = r["tab"].as_u64().unwrap_or(0) as usize;
                Ok(json!({"analysis": {"tab": tab, "command": s(&r["setup"]["tabs"][tab], "command"), "signals": r["setup"]["tabs"][tab]["signals"], "changed": r["changed"] == true}})) }
            "run" => { let mut asked = refs_of(&op["signals"]); for p in &self.probes { if !asked.iter().any(|x| key(x) == key(p)) { asked.push(p.clone()) } }
                // the saved analysis is read with its saved traces, when the call names none
                if asked.is_empty() && matches!(op["run"], Value::Bool(true)) { if let Ok(su) = self.call("GetSimulationSetup", json!({"document": self.doc})).await { asked = refs_of(&su["setup"]["tabs"][0]["signals"]) } }
                let (command, overrides, meas, signals, raw) = self.plan(op, &op["run"], &asked)?;
                let r = self.exec(&command, &op["directives"], overrides, &signals, &raw, op).await?;
                let mut out = json!({"analysis": s(&r, "analysis")});
                if command.is_empty() { out["command"] = json!(s(&r, "command")) }
                if s(&r, "status") != "SIM_OK" { out["failed"] = json!(kp::status_words(s(&r, "status"))); out["problems"] = json!(failure_lines(&r)) }
                else {
                    if r["needed"].is_string() { out["needed"] = r["needed"].clone() }
                    out["points"] = r["points"].clone();
                    // single numbers (an operating point, .tf): the asked ones, else all (forty at most)
                    let vals: Vec<&Value> = r["values"].as_array().into_iter().flatten().filter(|v| asked.is_empty() || asked.iter().any(|a| same_signal(a, v))).collect();
                    if !vals.is_empty() { out["values"] = Value::Object(vals.iter().take(40).map(|v| ((if s(v, "signal").is_empty() { s(v, "name") } else { s(v, "signal") }).to_string(), r6(v["value"].as_f64().unwrap_or(f64::NAN)))).collect());
                        if vals.len() > 40 { out["more_values"] = json!(vals.len() - 40) } }
                    let (mut lines, mut waves) = (Map::new(), Map::new()); let n = match &op["waves"] { Value::Number(n) => n.as_u64().unwrap_or(40).clamp(4, 400) as usize, Value::Bool(true) => 40, _ => 0 };
                    for a in &asked { match sig_of(&r, a) { Ok(sig) if sig.len() > 1 => { lines.insert(a.clone(), kw::summary(&sig, 4)); if n > 0 { waves.insert(a.clone(), kw::compact(&sig, n, 4)); } }
                        Ok(_) => {} Err(e) => { lines.insert(a.clone(), json!(format!("failed: {e}"))); } } }
                    if !lines.is_empty() { out["signals"] = Value::Object(lines) } if !waves.is_empty() { out["waves"] = Value::Object(waves) }
                    if !meas.is_empty() { out["measures"] = measures_of(&r, &meas) }
                    if asked.is_empty() && meas.is_empty() && vals.is_empty() { out["has"] = json!(names_of(&r)); out["note"] = json!("name signals or measures to read them") }
                    let warn = problem_lines(&r); if !warn.is_empty() { out["warnings"] = json!(warn.iter().take(5).collect::<Vec<_>>()) }
                }
                if let Some(ms) = r["timings"]["total_ms"].as_f64() { out["ms"] = json!(ms.round()) }
                if r["shown"] == true { out["shown"] = json!(true) }
                if op["netlist"] == true || (s(&r, "status") == "SIM_NETLIST_ERROR" && op["netlist"] != false && s(&r, "netlist").len() < 4000) { out["netlist"] = r["netlist"].clone() }
                if op["log"] == true { out["log"] = r["log"].clone() }
                Ok(json!({"run": out})) }
            "loopgain" => { let at = op["loopgain"].as_str().or(op["loopgain"]["at"].as_str()).ok_or("loopgain names the pin where the loop is cut: \"U1.2\" (reference.pin number)")?;
                let (reference, pin) = at.rsplit_once('.').ok_or(format!("`{at}`: the loop is cut at a pin, reference.pin number, e.g. \"U1.2\" (an op-amp's inverting input, a regulator's FB pin)"))?;
                let command = match &op["run"] { Value::String(t) => command_of(t)?, _ => ".ac dec 50 1 100meg".to_string() };
                if !command.to_ascii_lowercase().starts_with(".ac") { return Err("a loop gain is an .ac analysis: run: \".ac dec 50 1 100meg\"".into()) }
                // a voltage source in the cut (+ at the pin) and a current source into the pin: 0 V and 0 A at DC, so the operating point is the closed loop's
                let cut = |v: u8, i: u8| json!([{"reference": reference, "pin": pin, "insert": format!("Vlg_{{name}} {{pin}} {{net}} dc 0 ac {v}\nIlg_{{name}} 0 {{pin}} dc 0 ac {i}")}]);
                let fixed = overrides_of(&op["set"])?;
                let nl = self.call("GetSimulationNetlist", json!({"document": self.doc, "command": command, "overrides": fixed, "directives": op["directives"].as_array().cloned().unwrap_or_default(), "breaks": cut(0, 0)})).await?;
                let b = &nl["broken"][0];
                if s(b, "pin_node").is_empty() { let why: Vec<String> = nl["problems"].as_array().into_iter().flatten().map(kp::problem_line).collect(); return Err(if why.is_empty() { format!("{at} is not a pin of this schematic") } else { why.join("; ") }) }
                let node = s(b, "pin_node").to_string(); let branch = format!("{}#branch", s(b, "insert").split_whitespace().next().unwrap_or("").to_ascii_lowercase());
                let signals = [node.clone(), branch.clone()]; let mut got: Vec<(Sig, Sig)> = vec![]; let started = std::time::Instant::now();
                // three runs: neither source (what the circuit's own AC sources put at the cut, taken out of the other two), the voltage, the current
                for (v, i) in [(0, 0), (1, 0), (0, 1)] {
                    self.breaks = cut(v, i).as_array().cloned().unwrap_or_default();
                    let r = self.exec(&command, &op["directives"], fixed.clone(), &signals, &[], op).await; self.breaks.clear(); let r = r?;
                    if s(&r, "status") != "SIM_OK" { return Err(Self::failed(&r)) }
                    got.push((sig_of(&r, &node)?, sig_of(&r, &branch)?));
                }
                let t = kw::tian(&kw::minus(&got[1].0, &got[0].0), &kw::minus(&got[1].1, &got[0].1), &kw::minus(&got[2].0, &got[0].0), &kw::minus(&got[2].1, &got[0].1))?;
                let m = |name: &str| match kw::measure(name, &json!({}), &t) { Ok(v) => round_all(&v), Err(e) => json!(format!("none: {e}")) };
                let mut out = json!({"at": at, "crossover": m("unity_gain_freq"), "phase_margin": m("phase_margin"), "gain_margin": m("gain_margin"), "dc_gain_db": m("dc_gain"), "runs": 3, "ms": started.elapsed().as_millis() as u64});
                if t.y.iter().zip(t.im.as_ref().unwrap()).any(|(a, b)| !a.is_finite() || !b.is_finite()) { out["note"] = json!("the loop gain is not a number at some frequencies: nothing flows through this pin, so no loop passes it; cut the loop at another pin") }
                let n = match &op["waves"] { Value::Number(n) => n.as_u64().unwrap_or(40).clamp(4, 400) as usize, Value::Bool(true) => 40, _ => 0 }; if n > 0 { out["curve"] = kw::compact(&t, n, 4) }
                Ok(json!({"loopgain": out})) }
            "sweep" => { let target = match &op["sweep"] { Value::String(t) => t.clone(), o if o["param"].is_string() => format!("param:{}", s(o, "param")), _ => return Err("sweep names what to step: \"R1\", \"V1.dc\" or {param: name}".into()) };
                let values = kp::sweep_values(op)?; if values.len() > 200 { return Err(format!("{} steps: 200 at most in one sweep", values.len())) }
                let (command, fixed, meas, signals, raw) = self.plan(op, &op["run"], &[])?;
                if meas.is_empty() { return Err("a sweep answers measures per step: give measures, e.g. {\"fc\": \"corner V(out)\"}".into()) }
                let (mut rows, mut failed) = (vec![], vec![]); let started = std::time::Instant::now();
                for v in &values {
                    let (mut ov, mut dir) = (fixed.clone(), op["directives"].as_array().cloned().unwrap_or_default());
                    match target.strip_prefix("param:") { Some(p) => dir.push(json!(format!(".param {p}={}", kp::plain(*v)))), None => ov.push(override_of(&target, *v)) }
                    let r = self.exec(&command, &json!(dir), ov, &signals, &raw, op).await?;
                    let mut row = vec![r6(*v)];
                    if s(&r, "status") == "SIM_OK" { let m = measures_of(&r, &meas); for (name, _) in &meas { row.push(m[name].clone()) } }
                    else { for _ in &meas { row.push(Value::Null) } if failed.len() < 5 { failed.push(json!({"at": r6(*v), "why": Self::failed(&r)})) } }
                    rows.push(Value::Array(row));
                }
                let mut cols = vec![target.trim_start_matches("param:").to_string()]; cols.extend(meas.iter().map(|m| m.0.clone()));
                let mut out = json!({"columns": cols, "rows": rows, "runs": values.len(), "ms": started.elapsed().as_millis() as u64});
                if !failed.is_empty() { out["failed"] = json!(failed) }
                Ok(json!({"sweep": out})) }
            "tune" => { let target = op["tune"].as_str().ok_or("tune names the part (or its parameter) to change: \"R1\", \"V1.dc\"")?.to_string();
                let (from, to) = (kp::num_of(&op["from"]).ok_or("tune needs the range to search: from and to")?, kp::num_of(&op["to"]).ok_or("tune needs the range to search: from and to")?);
                let goal = kp::num_of(&op["target"]).ok_or("tune needs the target: the value the measure is to reach")?;
                let series = match op["series"].as_str() { Some(n) => Some(kp::e_series(n).ok_or(format!("no series `{n}`: E3, E6, E12, E24, E48, E96, E192"))?), None => None };
                if op["measure"].is_null() { return Err("tune needs the measure to bring to the target, e.g. measure: \"corner V(out)\"".into()) }
                let spec = json!({"m": op["measure"]}); let (command, fixed, meas, signals, raw) = self.plan(&json!({"measures": spec, "set": op["set"]}), &op["run"], &[])?;
                if op["write"] == true && target.contains('.') { return Err(format!("write puts a part's value on the schematic; {target} is a parameter: tune without write, then set it with model (params)")) }
                let tol = op["tol"].as_f64().unwrap_or(if series.is_some() { 0.005 } else { 0.001 }); let max = op["max_runs"].as_u64().unwrap_or(16).clamp(3, 60) as usize;
                let mut seen: Vec<(f64, f64)> = vec![];
                let mut t = kp::Tuner::new(from, to, goal, tol, max);
                let (x, converged) = loop { match t.next() {
                    kp::Step::Try(x) => { let y = self.tune_at(&command, &op["directives"], &fixed, &target, x, &meas, &signals, &raw, op).await?; seen.push((x, y)); t.tell(x, y) }
                    kp::Step::Done { x, converged, .. } => break (x, converged), kp::Step::Fail(e) => return Err(e) } };
                // from a series: the value on each side of the one found, and the nearer result of the two
                let mut best = (x, seen.iter().find(|p| p.0 == x).map_or(f64::NAN, |p| p.1));
                if let Some(sv) = &series { let (lo, hi) = kp::neighbours(sv, x); let mut cand: Vec<(f64, f64)> = vec![];
                    for v in [lo, hi].into_iter().flatten().filter(|v| *v >= from.min(to) * (1.0 - 1e-9) && *v <= from.max(to) * (1.0 + 1e-9)) {
                        let y = match seen.iter().find(|p| (p.0 - v).abs() <= 1e-9 * v.abs()) { Some(p) => p.1, None => { let y = self.tune_at(&command, &op["directives"], &fixed, &target, v, &meas, &signals, &raw, op).await?; seen.push((v, y)); y } };
                        cand.push((v, y)) }
                    best = cand.into_iter().min_by(|a, b| (a.1 - goal).abs().total_cmp(&(b.1 - goal).abs())).ok_or(format!("no {} value between {} and {}", s(op, "series"), kp::format_value(from), kp::format_value(to)))?; }
                let text = kp::format_value(best.0);
                let mut out = json!({"part": target, "value": text, "measure": r6(best.1), "target": goal, "error_pct": kw::round_sig(if goal == 0.0 { best.1 } else { (best.1 - goal) / goal * 100.0 }, 3), "runs": seen.len()});
                if series.is_some() { out["series"] = op["series"].clone(); out["exact"] = json!(kp::format_value(x)) }
                if !converged { out["note"] = json!(format!("the search stopped after {} runs without reaching the tolerance: the measure may jump or turn back in this range", seen.len())) }
                if op["write"] == true { self.edit(json!([{"set": target, "value": text}]), &format!("kicad_sim: {target} = {text}")).await?; out["written"] = json!(true) }
                Ok(json!({"tune": out})) }
            "stats" => { let p = self.ctx.log.as_ref().ok_or("this hub keeps no simulation log")?; Ok(json!({"stats": super::ksim_log::summary(&super::ksim_log::read(p), op["stats"]["thread"].as_str().unwrap_or(""))})) }
            _ => Err(format!("an op is one of {OP_NAMES}")),
        }
    }
    /// The measure of a tune at one value: a run with that override.
    #[allow(clippy::too_many_arguments)]
    async fn tune_at(&mut self, command: &str, directives: &Value, fixed: &[Value], target: &str, x: f64, meas: &[(String, Meas)], signals: &[String], raw: &[String], op: &Value) -> Result<f64, String> {
        let mut ov = fixed.to_vec(); ov.push(override_of(target, x));
        let r = self.exec(command, directives, ov, signals, raw, op).await?;
        if s(&r, "status") != "SIM_OK" { return Err(format!("at {target} = {}: {}", kp::format_value(x), Self::failed(&r))) }
        let m = measures_of(&r, meas);
        m["m"].as_f64().ok_or(format!("at {target} = {}: the measure is {}", kp::format_value(x), if m["m"].is_string() { m["m"].as_str().unwrap_or("").to_string() } else { format!("{} (a tune needs one number: `corner` of a band-pass and `peak` answer two)", m["m"]) }))
    }
}

/// What a part without a model is answered: the rule for finding one (the tool's description and the prompt say the same).
pub const VENDOR_RULE: &str = "for each part with no model found here: get its maker's SPICE model for the MPN from the maker's own site and give it with import (file or text, url, licence). If the maker has none, or getting it needs a login, a licence acceptance or a purchase, stop and ask the person, naming the part and what you found; never put a generic or another part's model in its place without their word";

/// The whole call: `{path, ops: [...], save?}` -> `{results: [one per op], saved, left}` and the log entry of each op. A failed
/// simulation is its op's result (`failed`, `problems`), not an error: the ops after it are not run, and what was written before it
/// stays and is saved. An op that cannot be done at all is the error, naming what was already done.
pub async fn run(c: &dyn Call, path: &str, a: &Value, ctx: &Ctx) -> (Result<Value, String>, Vec<Value>) {
    let ops = match &a["ops"] { Value::Array(o) if !o.is_empty() => o.clone(), _ => return (Err(format!("ops is a list of simulation steps: {OP_NAMES}")), vec![]) };
    if let Some((i, _)) = ops.iter().enumerate().find(|(_, o)| verb(o).is_empty()) { return (Err(format!("op {}: an op is one of {OP_NAMES}", i + 1)), vec![]) }
    if !path.ends_with(".kicad_sch") { return (Err("path is the schematic (.kicad_sch) to simulate".into()), vec![]) }
    if !c.caps().iter().any(|x| x == CAP) { return (Err(no_sim("")), vec![]) }
    let doc = match open(c, path).await { Ok(d) => d, Err(e) => return (Err(e), vec![]) };
    let mut sim = Sim { c, path, ctx, doc, wrote: false, saved: None, models_changed: false, probes: vec![], log: vec![], runs: vec![], lookups: vec![], breaks: vec![], retries: vec![], scanned: None };
    let (mut results, mut failure, mut skipped) = (vec![], None, 0);
    for (i, op) in ops.iter().enumerate() {
        let started = std::time::Instant::now(); sim.runs.clear(); sim.lookups.clear(); sim.retries.clear();
        let r = sim.op(op).await;
        let sum = |k: &str| sim.runs.iter().filter_map(|x| x[k].as_f64()).sum::<f64>().round();
        let mut entry = json!({"op": verb(op), "ok": r.is_ok(), "hub_ms": started.elapsed().as_millis() as u64, "runs": sim.runs.len()});
        if !sim.runs.is_empty() { entry["analysis"] = sim.runs[0]["analysis"].clone(); entry["status"] = json!(sim.runs.iter().map(|x| s(x, "status")).find(|x| *x != "SIM_OK").unwrap_or("SIM_OK"));
            entry["timings"] = json!({"netlist_ms": sum("netlist_ms"), "load_ms": sum("load_ms"), "run_ms": sum("run_ms"), "total_ms": sum("total_ms")}); entry["ngspice_ms"] = json!(sum("load_ms") + sum("run_ms")); entry["points"] = sim.runs[0]["points"].clone() }
        if !sim.lookups.is_empty() { entry["lookups"] = json!(sim.lookups) }
        if !sim.retries.is_empty() { entry["retries"] = json!(sim.retries) }
        match r {
            Ok(v) => { entry["bytes"] = json!(v.to_string().len());
                let failed = v["run"]["failed"].is_string(); if failed { entry["problems"] = v["run"]["problems"].clone() }
                sim.log.push(entry); results.push(v);
                if failed { skipped = ops.len() - i - 1; break } }
            Err(e) => { entry["error"] = json!(e.chars().take(300).collect::<String>()); sim.log.push(entry); failure = Some(format!("op {} ({}): {e}", i + 1, verb(op))); break }
        }
    }
    // what the simulation commands changed is saved, whatever came after
    let mut out = json!({"results": results});
    if sim.wrote && a["save"].as_bool().unwrap_or(true) {
        match sim.call(&format!("{C}SaveDocument"), json!({"document": sim.doc})).await {
            Ok(sv) => { if sv["not_saved"].is_string() && sim.saved.is_none() { out["saved"] = json!(false); out["not_saved"] = sv["not_saved"].clone() } else { out["saved"] = json!(path) }
                for k in ["format_upgrade", "kicad_load_changes", "detached_kicad"] { if sv[k].is_string() { out[k] = sv[k].clone() } } }
            Err(e) => { out["saved"] = json!(false); out["not_saved"] = json!(e) }
        }
    } else if let Some(p) = &sim.saved { out["saved"] = json!(p) }
    if skipped > 0 { out["skipped"] = json!(format!("the {} op{} after the failed run {} not run", skipped, if skipped == 1 { "" } else { "s" }, if skipped == 1 { "was" } else { "were" })) }
    // after a model or an exclusion changed: what still would not simulate
    if sim.models_changed { if let Ok(r) = sim.models(&[], true).await { let left: Vec<String> = r["models"].as_array().into_iter().flatten().filter(|m| has_error(m)).map(model_problem).collect();
        out["left"] = if left.len() > 12 { json!({"count": left.len(), "first": left[..12]}) } else { json!(left) } } }
    let log = std::mem::take(&mut sim.log);
    match failure {
        None => (Ok(out), log),
        Some(e) => { let done: Vec<&str> = ops.iter().take(results.len()).map(verb).filter(|v| OPS.iter().any(|o| o.0 == *v && o.1)).collect();
            (Err(format!("{e}.{}", if done.is_empty() { String::new() } else { format!(" Already done and {}: {}.", if out["saved"].is_string() { "saved" } else { "kept" }, done.join(", ")) })), log) }
    }
}
async fn open(c: &dyn Call, path: &str) -> Result<Value, String> {
    if let Ok(r) = c.call(&format!("{C}GetOpenDocuments"), json!({"type": "DOCTYPE_SCHEMATIC"})).await { if let Some(d) = r["documents"].get(0) { return Ok(d.clone()) } }
    Ok(c.call(&format!("{C}OpenDocument"), json!({"type": "DOCTYPE_SCHEMATIC", "path": path})).await?["document"].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kedit::Fut;
    use std::{f64::consts::PI, sync::Mutex};

    /// A stand-in KiCad with the fork's simulation commands (api/proto/backplane/sim.proto) over a small circuit it solves in closed
    /// form: V1 into R1 (10k) to OUT, C1 (10n) from OUT to ground, a divider R2/R3 from VCC, and U1 (an op-amp) with no model. Values
    /// come from the schematic's parts or a run's overrides, so sweeps and tunes move the answers as the real circuit would.
    struct Fake { made: Mutex<Vec<Value>>, log: Mutex<Vec<(String, Value)>>, caps: Vec<String>, parts: Mutex<std::collections::HashMap<String, Value>>, setup: Mutex<Vec<Value>>, mode: &'static str, polls: Mutex<u32>, save: Value }
    fn part(reference: &str, value: &str, lib: &str, device: &str) -> Value {
        json!({"reference": reference, "value": value, "lib_id": lib, "source": if device.is_empty() { "MS_NONE" } else { "MS_INFERRED" }, "device": device,
            "pins": [{"pin_number": "1", "pin_name": if reference == "U1" { "IN+" } else { "~" }, "model_node": if device.is_empty() { "" } else { "+" }}, {"pin_number": "2", "pin_name": if reference == "U1" { "IN-" } else { "~" }, "model_node": if device.is_empty() { "" } else { "-" }}],
            "error_kind": if device.is_empty() { "ME_NO_MODEL" } else { "ME_NONE" }, "error": if device.is_empty() { format!("{reference} has no simulation model") } else { String::new() }})
    }
    fn fake() -> Fake {
        let parts = [part("R1", "10k", "Device:R", "R"), part("C1", "10n", "Device:C", "C"), part("R2", "10k", "Device:R", "R"), part("R3", "10k", "Device:R", "R"), part("V1", "1", "Simulation_SPICE:VDC", "V"),
            part("U1", "LM358", "Amplifier_Operational:LM358", ""), part("J1", "Conn_01x02", "Connector_Generic:Conn_01x02", ""), part("TP1", "TestPoint", "Connector:TestPoint", ""), part("H1", "MountingHole", "Mechanical:MountingHole", "")];
        Fake { made: Mutex::new(vec![]), log: Mutex::new(vec![]), caps: vec![CAP.into()], parts: Mutex::new(parts.into_iter().map(|p| (s(&p, "reference").to_string(), p)).collect()), setup: Mutex::new(vec![]), mode: "", polls: Mutex::new(0), save: json!({}) }
    }
    impl Fake {
        fn value(&self, r: &str, overrides: &Value) -> f64 {
            let o = overrides.as_array().into_iter().flatten().find(|o| s(o, "reference") == r && matches!(s(o, "field"), "" | "Value")).map(|o| s(o, "value").to_string());
            kp::parse_value(&o.unwrap_or_else(|| s(&self.parts.lock().unwrap()[r], "value").to_string())).unwrap()
        }
        fn simulate(&self, msg: &Value) -> Value {
            let ov = &msg["overrides"]; let (r1, c1, r2, r3) = (self.value("R1", ov), self.value("C1", ov), self.value("R2", ov), self.value("R3", ov));
            let cmd = if s(msg, "command").is_empty() { self.setup.lock().unwrap().first().map(|t| s(t, "command").to_string()).unwrap_or(".op".into()) } else { s(msg, "command").to_string() };
            let word = cmd[1..].split_whitespace().next().unwrap_or("").to_string(); let wants = |n: &str| msg["signals"].as_array().into_iter().flatten().any(|x| x == "*" || same_signal(x.as_str().unwrap_or(""), &json!({"name": n})));
            let mut out = json!({"status": "SIM_OK", "analysis": word.to_ascii_uppercase(), "command": cmd, "timings": {"netlist_ms": 2.0, "load_ms": 3.0, "run_ms": 5.0, "total_ms": 11.0}, "ngspice_version": "45", "run_id": "7", "nodes": 4, "devices": 5});
            let vector = |name: &str, signal: &str, kind: &str, re: Vec<f64>, im: Option<Vec<f64>>, data: bool| { let mut v = json!({"name": name, "signal": signal, "kind": kind, "length": re.len()}); if data { v["real"] = json!(re); if let Some(i) = im { v["complex"] = json!(true); v["imag"] = json!(i) } } v };
            match word.as_str() {
                "op" => { out["points"] = json!(1); out["values"] = json!([{"name": "v(/out)", "signal": "V(/out)", "value": 1.0}, {"name": "v(/mid)", "signal": "V(/mid)", "value": 5.0 * r3 / (r2 + r3)}, {"name": "v(/vcc)", "signal": "V(/vcc)", "value": 5.0}, {"name": "i(v1)", "signal": "I(V1)", "value": 0.0}]) }
                "ac" => { let f: Vec<f64> = (0..=140).map(|i| 10f64.powf(i as f64 / 20.0)).collect(); let (re, im): (Vec<f64>, Vec<f64>) = f.iter().map(|f| { let w = 2.0 * PI * f * r1 * c1; (1.0 / (1.0 + w * w), -w / (1.0 + w * w)) }).unzip();
                    out["points"] = json!(f.len()); out["vectors"] = json!([vector("frequency", "", "VK_FREQUENCY", f.clone(), Some(vec![0.0; f.len()]), true), vector("v(/out)", "V(/out)", "VK_VOLTAGE", re, Some(im), wants("v(/out)")), vector("v(/in)", "V(/in)", "VK_VOLTAGE", vec![1.0; f.len()], Some(vec![0.0; f.len()]), wants("v(/in)"))]) }
                "tran" => { let tau = r1 * c1; let t: Vec<f64> = (0..=2000).map(|i| i as f64 * 10.0 * tau / 2000.0).collect(); let y: Vec<f64> = t.iter().map(|t| 1.0 - (-t / tau).exp()).collect();
                    out["points"] = json!(t.len()); out["vectors"] = json!([vector("time", "", "VK_TIME", t, None, true), vector("v(/out)", "V(/out)", "VK_VOLTAGE", y, None, wants("v(/out)"))]);
                    out["measures"] = json!(msg["measures"].as_array().into_iter().flatten().map(|m| { let name = m.as_str().unwrap().split_whitespace().find(|w| !matches!(*w, "tran")).unwrap(); if m.as_str().unwrap().contains("never") { json!({"name": name, "failed": true, "error": "measure failed: out of interval"}) } else { json!({"name": name, "value": tau * 9f64.ln()}) } }).collect::<Vec<_>>()) }
                _ => {}
            }
            if msg["include_netlist"] == true { out["netlist"] = json!(".title t\nR1 /in /out 10k\n.end\n") }
            out["log"] = json!("Circuit: t\nDoing analysis at TEMP = 27.000000\n");
            out
        }
    }
    impl Call for Fake {
        fn call<'a>(&'a self, ty: &'a str, msg: Value) -> Fut<'a> { Box::pin(async move {
            let short = ty.rsplit('.').next().unwrap_or(""); self.log.lock().unwrap().push((short.to_string(), msg.clone()));
            let doc = json!({"type": "DOCTYPE_SCHEMATIC", "project": {"name": "t", "path": "/p"}});
            Ok(match short {
                "GetOpenDocuments" => json!({"documents": [doc]}),
                "GetSimulationModels" => { let parts = self.parts.lock().unwrap(); let mut rows: Vec<Value> = parts.values().cloned().collect(); rows.sort_by(|a, b| s(a, "reference").cmp(s(b, "reference")));
                    let asked = refs_of(&msg["references"]); let not_found: Vec<&String> = asked.iter().filter(|r| !parts.contains_key(*r)).collect();
                    let active = |m: &&Value| m["exclude_from_sim"] != true;
                    let problems = rows.iter().filter(active).filter(|m| has_error(m)).count();
                    let models: Vec<Value> = rows.iter().filter(|m| asked.is_empty() || asked.contains(&s(m, "reference").to_string())).filter(|m| msg["only_problems"] != true || (has_error(m) && m["exclude_from_sim"] != true)).map(|m| { let mut m = m.clone(); if m["exclude_from_sim"] == true { m["error_kind"] = json!("ME_NONE"); m["error"] = json!("") } m }).collect();
                    json!({"models": models, "symbols": rows.len(), "excluded": rows.iter().filter(|m| m["exclude_from_sim"] == true).count(), "problems": problems, "not_found": not_found}) }
                "AssignSimulationModel" => { let mut parts = self.parts.lock().unwrap(); let mut results = vec![];
                    for r in refs_of(&msg["references"]) { let Some(p) = parts.get_mut(&r) else { results.push(json!({"reference": r, "error_kind": "ME_OTHER", "error": format!("no symbol {r}")})); continue };
                        if let Some(x) = msg["exclude_from_sim"].as_bool() { p["exclude_from_sim"] = json!(x) }
                        if msg["library"].is_object() { if s(&msg["library"], "name") == "MISSING" { results.push(json!({"reference": r, "error_kind": "ME_MODEL_NOT_IN_LIBRARY", "error": "model MISSING not found in library"})); continue }
                            p["source"] = json!("MS_LIBRARY"); p["device"] = json!("SUBCKT"); p["model_name"] = msg["library"]["name"].clone(); p["library"] = msg["library"]["library"].clone(); p["error_kind"] = json!("ME_NONE"); p["error"] = json!("");
                            if msg["pins"].as_array().is_some_and(|a| !a.is_empty()) { p["pins"] = msg["pins"].clone() } else { for (n, pin) in p["pins"].as_array_mut().unwrap().iter_mut().enumerate() { pin["model_node"] = json!((n + 1).to_string()) } } }
                        if msg["builtin"].is_object() { p["source"] = json!("MS_BUILTIN"); p["device"] = msg["builtin"]["device"].clone(); p["type"] = msg["builtin"]["type"].clone(); p["error_kind"] = json!("ME_NONE"); p["error"] = json!("");
                            p["netlist_line"] = json!(format!("{r} + - {} {}", s(&msg["builtin"], "type"), msg["builtin"]["params"].as_array().into_iter().flatten().map(|x| format!("{}={}", s(x, "name"), s(x, "value"))).collect::<Vec<_>>().join(" "))) }
                        if msg["clear"] == true { p["source"] = json!("MS_NONE"); p["device"] = json!(""); p["error_kind"] = json!("ME_NO_MODEL"); p["error"] = json!("no model") }
                        results.push(json!({"reference": r, "changed": true, "model": p.clone()})) }
                    let failed = results.iter().filter(|x| !s(x, "error").is_empty()).count();
                    json!({"changed": results.len() - failed, "failed": failed, "results": results}) }
                "GetSimulationSetup" => json!({"setup": {"tabs": self.setup.lock().unwrap().clone(), "schematic_command": ""}}),
                "SetSimulationSetup" => { let mut tabs = self.setup.lock().unwrap(); let tab = json!({"analysis": "x", "command": msg["command"], "signals": msg["signals"], "options": msg["options"]});
                    let at = msg["tab"].as_u64().map(|n| n as usize).filter(|n| *n < tabs.len()); let n = match at { Some(n) => { tabs[n] = tab; n } None => { tabs.push(tab); tabs.len() - 1 } };
                    json!({"setup": {"tabs": tabs.clone()}, "tab": n, "changed": true}) }
                "GetSimulationNetlist" => { let b = &msg["breaks"][0]; if s(b, "reference") != "U1" { json!({"netlist": "", "problems": [{"kind": "PK_NETLIST", "severity": "PS_ERROR", "message": format!("break: the schematic has no symbol {}", s(b, "reference"))}]}) }
                    else { json!({"netlist": ".title t\n.end\n", "broken": [{"reference": "U1", "pin": s(b, "pin"), "net_node": "/fb", "pin_node": format!("brk_U1_{}", s(b, "pin")), "insert": s(b, "insert").replace("{name}", &format!("U1_{}", s(b, "pin")))}]}) } }
                // a loop cut at U1's pin: an amplifier -A(s) of v(pin), A = 8 / (1 + s/p)^3 at 1 kHz, behind 1k into 1k; the circuit's own AC source adds the same to every run
                "RunSimulation" if msg["breaks"].is_array() => { let ins = s(&msg["breaks"][0], "insert"); let ac: Vec<f64> = ins.lines().map(|l| l.rsplit(' ').next().unwrap().parse().unwrap()).collect();
                    let f: Vec<f64> = (0..=300).map(|i| 10f64.powf(i as f64 / 50.0)).collect(); let (mut vr, mut vi, mut ir, mut ii) = (vec![], vec![], vec![], vec![]);
                    for f in &f { let c = |re: f64, im: f64| num(re, im); let q = c(1.0, f / 1e3); let a = c(8.0, 0.0).div(q.mul(q).mul(q)); let one = c(1.0, 0.0);
                        let v1 = one.div(one.add(a).add(one)); let i1 = c(0.0, 0.0).sub(v1.div(c(1e3, 0.0))); let v2 = one.div(c(1e-3, 0.0).add(one.add(a).div(c(1e3, 0.0)))); let i2 = one.sub(v2.div(c(1e3, 0.0)));
                        let v = c(0.3, -0.1).add(v1.mul(c(ac[0], 0.0))).add(v2.mul(c(ac[1], 0.0))); let i = c(2e-4, 1e-4).add(i1.mul(c(ac[0], 0.0))).add(i2.mul(c(ac[1], 0.0)));
                        vr.push(v.0); vi.push(v.1); ir.push(i.0); ii.push(i.1) }
                    json!({"status": "SIM_OK", "analysis": "AC", "points": f.len(), "timings": {"run_ms": 2.0, "total_ms": 3.0}, "vectors": [{"name": "frequency", "kind": "VK_FREQUENCY", "real": f}, {"name": "brk_u1_2", "complex": true, "real": vr, "imag": vi}, {"name": "vlg_u1_2#branch", "complex": true, "real": ir, "imag": ii}]}) }
                "RunSimulation" if self.mode == "stiff" && !msg["directives"].to_string().contains("reltol=0.01") => json!({"status": "SIM_NO_CONVERGENCE", "analysis": "TRAN", "log": "doAnalyses: TRAN:  Timestep too small; time = 2e-06, timestep = 1e-18\n", "timings": {"run_ms": 9.0, "total_ms": 10.0}}),
                "RunSimulation" => match self.mode {
                    "busy" => return Err("KiCad answered AS_BUSY: a simulation is running".into()),
                    "slow" => json!({"status": "SIM_RUNNING", "run_id": "7", "progress": 0.1}),
                    "float" => json!({"status": "SIM_FLOATING_NODE", "analysis": "OP", "problems": [{"kind": "PK_FLOATING_NODE", "severity": "PS_ERROR", "node": "net-_c1-pad2_", "net": "Net-(C1-Pad2)", "message": "no DC path to ground"}, {"kind": "PK_MISSING_MODEL", "severity": "PS_ERROR", "reference": "U1", "message": "U1 has no model"}],
                        "log": "Circuit: t\nWarning: singular matrix:  check node net-_c1-pad2_\nrun simulation(s) aborted\n", "timings": {"netlist_ms": 1.0, "total_ms": 4.0}}),
                    "stiff" => self.simulate(&msg),
                    "noconv" => json!({"status": "SIM_NO_CONVERGENCE", "analysis": "TRAN", "log": "Circuit: t\ndoAnalyses: TRAN:  Timestep too small; time = 1.5e-05, timestep = 1.25e-19: trouble with node \"/sw\"\nrun simulation(s) aborted\n", "timings": {"run_ms": 40.0, "total_ms": 44.0}}),
                    _ => self.simulate(&msg) },
                "GetSimulationResult" => { let mut p = self.polls.lock().unwrap(); *p += 1; if *p < 3 { json!({"status": "SIM_RUNNING", "run_id": "7"}) } else { self.simulate(&json!({"command": ".op"})) } }
                "SaveDocument" => self.save.clone(),
                // what a nested kicad_edit needs of a schematic: its sheets, its items, KiCad's own placement of a library symbol
                "GetSchematicHierarchy" => json!({"top_level_sheets": [{"path": {"path": [{"value": "root"}], "path_human_readable": "/"}}]}),
                "GetItems" => if msg["types"][0] == "KOT_SCH_SYMBOL" { let mut items = json!([
                    {"id": {"value": "s-r1"}, "path": {"path": [{"value": "root"}], "path_human_readable": "/"}, "position": {"x_nm": "100000000", "y_nm": "100000000"}, "reference_field": {"text": {"text": "R1"}}, "value_field": {"text": {"text": "10k"}}, "unit": {"unit": 1}, "transform": {"orientation": "SSO_0"},
                        "definition": {"items": [{"item": {"@type": "type.googleapis.com/kiapi.schematic.types.SchematicPin", "number": "1", "name": "~", "position": {"y_nm": "-3810000"}, "orientation": "SPO_DOWN"}, "unit": {"unit": 1}}, {"item": {"@type": "type.googleapis.com/kiapi.schematic.types.SchematicPin", "number": "2", "name": "~", "position": {"y_nm": "3810000"}, "orientation": "SPO_UP"}, "unit": {"unit": 1}}]}},
                    {"id": {"value": "s-u1"}, "path": {"path": [{"value": "root"}], "path_human_readable": "/"}, "position": {"x_nm": "120000000", "y_nm": "100000000"}, "reference_field": {"text": {"text": "U1"}}, "value_field": {"text": {"text": "LM358"}}, "unit": {"unit": 1},
                        "user_fields": [{"name": "MPN", "text": {"text": "LM358DR"}}, {"name": "Manufacturer", "text": {"text": "Texas Instruments"}}]}]); items.as_array_mut().unwrap().extend(self.made.lock().unwrap().clone()); json!({"items": items}) } else { json!({"items": []}) },
                "BeginCommit" => json!({"id": {"value": "commit-1"}}),
                "PlaceSymbolFromLibrary" => { let pin = |n: &str, y: i64, o: &str| json!({"item": {"@type": "type.googleapis.com/kiapi.schematic.types.SchematicPin", "number": n, "name": if n == "1" { "+" } else { "-" }, "position": {"y_nm": (y * 1000000).to_string()}, "orientation": o}, "unit": {"unit": 1}});
                    json!({"item": {"id": {"value": "probe"}, "path": {}, "position": msg["position"], "transform": {"orientation": msg["orientation"]}, "unit": {"unit": 1}, "definition": {"id": msg["lib_id"], "items": [pin("1", -5, "SPO_DOWN"), pin("2", 5, "SPO_UP")]},
                        "reference_field": {"visible": true, "text": {"position": {"x_nm": "5"}, "text": msg["reference"]}}, "value_field": {"visible": true, "text": {"position": {"x_nm": "6"}, "text": "1"}}, "@type": "type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance"}}) }
                "CreateItems" => { for i in msg["items"].as_array().unwrap() { if i["@type"].as_str().is_some_and(|t| t.ends_with("SchematicSymbolInstance")) { let r = i["reference_field"]["text"]["text"].as_str().unwrap_or("");
                        if !r.starts_with('#') { let mut it = i.clone(); it["id"] = json!({"value": format!("made-{r}")}); it["path"] = json!({"path": [{"value": "root"}], "path_human_readable": "/"}); self.made.lock().unwrap().push(it);
                            self.parts.lock().unwrap().insert(r.to_string(), part(r, "1", "Simulation_SPICE:VDC", "V")); } } }
                    json!({"created_items": msg["items"].as_array().unwrap().iter().enumerate().map(|(n, i)| { let mut it = i.clone(); it["id"] = json!({"value": format!("new-{n}")}); json!({"status": {"code": "ISC_OK"}, "item": it}) }).collect::<Vec<_>>()}) }
                "UpdateItems" => { for i in msg["items"].as_array().unwrap() { if i["id"]["value"] == "s-r1" { self.parts.lock().unwrap().get_mut("R1").unwrap()["value"] = i["value_field"]["text"]["text"].clone() } }
                    json!({"updated_items": msg["items"].as_array().unwrap().iter().map(|i| json!({"status": {"code": "ISC_OK"}, "item": i})).collect::<Vec<_>>()}) }
                _ => json!({}),
            })
        }) }
        fn caps(&self) -> Vec<String> { self.caps.clone() }
    }
    /// A complex number, for the stand-in's loop.
    #[derive(Clone, Copy)] struct Cx(f64, f64);
    fn num(re: f64, im: f64) -> Cx { Cx(re, im) }
    impl Cx { fn add(self, o: Cx) -> Cx { Cx(self.0 + o.0, self.1 + o.1) } fn sub(self, o: Cx) -> Cx { Cx(self.0 - o.0, self.1 - o.1) } fn mul(self, o: Cx) -> Cx { Cx(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0) }
        fn div(self, o: Cx) -> Cx { let d = o.0 * o.0 + o.1 * o.1; Cx((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d) } }
    fn tmp(tag: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("ksim-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
    fn go_in(f: &Fake, dir: &Path, ops: Value) -> (Result<Value, String>, Vec<Value>) {
        let ctx = Ctx { project_dir: dir.to_path_buf(), libs: vec![], now: 1_800_000_000, log: None, gui: false };
        tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(run(f, "/p/t.kicad_sch", &json!({"ops": ops}), &ctx))
    }
    fn go(f: &Fake, ops: Value) -> Value { go_in(f, Path::new("/nonexistent-ksim"), ops).0.unwrap_or_else(|e| panic!("{e}")) }
    fn names(f: &Fake) -> Vec<String> { f.log.lock().unwrap().iter().map(|(t, _)| t.clone()).collect() }
    fn sent(f: &Fake, ty: &str) -> Vec<Value> { f.log.lock().unwrap().iter().filter(|(t, _)| t == ty).map(|(_, m)| m.clone()).collect() }
    fn near(got: &Value, want: f64, rel: f64) -> bool { got.as_f64().is_some_and(|g| (g - want).abs() <= rel * want.abs()) }
    const FC: f64 = 1591.549430918953; // 1 / (2 pi 10k 10n)

    #[test] fn the_op_table_and_what_writes() {
        for (name, _, shape, what) in OPS { assert!(shape.contains(name) && !what.is_empty() && OP_NAMES.contains(name), "{name}") }
        assert_eq!(OP_NAMES.split(", ").count(), OPS.len());
        assert_eq!((verb(&json!({"run": ".op"})), verb(&json!({"tune": "R1", "run": ".ac"})), verb(&json!({"import": {}, "to": "U1"})), verb(&json!({"nothing": 1}))), ("run", "tune", "import", ""));
        for (ops, w) in [(json!([{"run": ".op"}, {"models": true}, {"sweep": "R1"}, {"analysis": true}, {"stats": true}]), false), (json!([{"tune": "R1"}]), false), (json!([{"tune": "R1", "write": true}]), true), (json!([{"model": "U1"}]), true),
            (json!([{"exclude": "connectors"}]), true), (json!([{"import": {"text": ""}}]), false), (json!([{"import": {}, "to": "U1"}]), true), (json!([{"analysis": ".op"}]), true), (json!([{"probe": "V(out)"}]), true)] { assert_eq!(writes(&ops), w, "{ops}") }
    }
    #[test] fn an_older_kicad_is_told_it_has_no_simulation() {
        let mut f = fake(); f.caps = vec!["coedit".into()];
        let (r, log) = go_in(&f, Path::new("/x"), json!([{"run": ".op"}]));
        assert_eq!(r.unwrap_err(), "this KiCad has no simulation: kicad_sim needs Backplane KiCad bp.13.2 or newer (Settings, KiCad)"); assert!(log.is_empty() && names(&f).is_empty(), "nothing was asked of it");
        assert!(no_sim("10.99.0-bp.12.1").contains("(it is 10.99.0-bp.12.1)"));
        let f = fake(); assert!(go_in(&f, Path::new("/x"), json!([])).0.unwrap_err().contains("ops is a list") && go_in(&f, Path::new("/x"), json!([{"run": ".op"}, {"walk": 1}])).0.unwrap_err().starts_with("op 2: an op is one of models"));
    }
    #[test] fn signals_are_found_by_any_of_their_names() {
        let v = json!({"name": "v(/out)", "signal": "V(/out)"});
        for a in ["V(/out)", "v(/OUT)", "V(out)", "out", "/out", " V( out )"] { assert!(same_signal(a, &v), "{a}") }
        for a in ["V(out)", "out", "V(/out)", "/out"] { assert!(same_signal(a, &json!({"name": "/out", "signal": "V(/out)"})) && same_signal(a, &json!({"name": "out"})), "{a}") }
        assert!(!same_signal("V(in)", &v) && !same_signal("I(out)", &v) && same_signal("I(R1)", &json!({"name": "i(r1)", "signal": "I(R1)"})) && same_signal("onoise_spectrum", &json!({"name": "onoise_spectrum"})));
        assert_eq!((split_expr("V(/out)/V(/in)"), split_expr("V(a)-V(b)"), split_expr("V(/a/b)"), split_expr("/a/b"), split_expr("V(a-b)")), (Some(("V(/out)", '/', "V(/in)")), Some(("V(a)", '-', "V(b)")), None, None, None));
        assert_eq!(leaves("V(out)/V(in)"), vec!["V(out)", "V(in)"]);
        let resp = json!({"vectors": [{"name": "frequency", "kind": "VK_FREQUENCY", "real": [1.0, 10.0]}, {"name": "v(/out)", "signal": "V(/out)", "complex": true, "real": [0.0, 4.0], "imag": [2.0, 0.0]}, {"name": "v(/in)", "signal": "V(/in)", "complex": true, "real": [2.0, 2.0], "imag": [0.0, 0.0]}, {"name": "v(/x)", "signal": "V(/x)", "length": 2}],
            "values": [{"name": "v(/mid)", "signal": "V(/mid)", "value": 2.5}]});
        let g = sig_of(&resp, "V(out)/V(in)").unwrap(); assert_eq!((g.x.clone(), g.y.clone(), g.im.clone(), g.log), (vec![1.0, 10.0], vec![0.0, 2.0], Some(vec![1.0, 0.0]), true));
        assert_eq!(sig_of(&resp, "V(out)-V(in)").unwrap().y, vec![-2.0, 2.0]); assert_eq!(sig_of(&resp, "mid").unwrap().y, vec![2.5]);
        assert_eq!(sig_of(&resp, "V(nope)").unwrap_err(), "no `V(nope)` in this run; it has frequency, V(/out), V(/in), V(/x), V(/mid)");
        assert!(sig_of(&resp, "V(x)").is_err(), "a vector that came without data is not there");
    }
    #[test] fn measure_specs_in_three_forms() {
        let m = parse_measures(&json!({"fc": {"corner": "V(out)"}, "pm": {"measure": "phase_margin", "of": "V(a)/V(b)", "inverted": true}, "os": "overshoot V(out) final=3.3 from=1m", "tr": "TRIG v(in) VAL=0.5 RISE=1 TARG v(out) VAL=0.5 RISE=1",
            "f3": "ac WHEN vdb(out)=-3", "t": {"settling_time": "V(out)", "band": "1%", "t0": "1m"}})).unwrap();
        let get = |n: &str| m.iter().find(|x| x.0 == n).unwrap().1.clone();
        assert_eq!(get("fc"), Meas::Hub { measure: "corner".into(), of: "V(out)".into(), params: json!({}) });
        assert_eq!(get("pm"), Meas::Hub { measure: "phase_margin".into(), of: "V(a)/V(b)".into(), params: json!({"inverted": true}) });
        assert_eq!(get("os"), Meas::Hub { measure: "overshoot".into(), of: "V(out)".into(), params: json!({"final": 3.3, "from": 0.001}) });
        assert_eq!(get("t"), Meas::Hub { measure: "settling_time".into(), of: "V(out)".into(), params: json!({"band": 1.0, "t0": 0.001}) });
        assert_eq!((get("tr"), get("f3")), (Meas::Raw("tr TRIG v(in) VAL=0.5 RISE=1 TARG v(out) VAL=0.5 RISE=1".into()), Meas::Raw("ac f3 WHEN vdb(out)=-3".into())));
        assert!(parse_measures(&json!({"x": {"wobble": "V(out)"}})).unwrap_err().contains("name one of at, cross") && parse_measures(&json!({"x": "corner"})).unwrap_err().contains("of which signal") && parse_measures(&json!(["corner"])).is_err() && parse_measures(&Value::Null).unwrap().is_empty());
        assert_eq!(overrides_of(&json!({"R1": "2k", "V1.dc": 3.3})).unwrap(), vec![json!({"reference": "R1", "field": "", "value": "2k"}), json!({"reference": "V1", "field": "dc", "value": "3.3"})]);
        assert_eq!((command_of("tran 1u 10m"), command_of(" .AC dec 20 1 1meg "), command_of(".op")), (Ok(".tran 1u 10m".into()), Ok(".AC dec 20 1 1meg".into()), Ok(".op".into())));
        assert!(command_of(".plot v(out)").unwrap_err().contains("is not an analysis") && command_of("").is_err());
    }
    #[test] fn models_says_what_would_not_simulate_and_where_a_model_is() {
        let d = tmp("models"); std::fs::write(d.join("opamps.lib"), ".subckt LM358 1 2 3 4 5\n.ends\n").unwrap();
        let f = fake(); let (r, log) = go_in(&f, &d, json!([{"models": true}])); let m = &r.unwrap()["results"][0]["models"];
        assert_eq!((m["symbols"].as_i64(), m["problems"].as_i64(), m["parts"].as_array().map(|p| p.len())), (Some(9), Some(4), Some(4)));
        let u1 = m["parts"].as_array().unwrap().iter().find(|p| p["ref"] == "U1").unwrap();
        assert_eq!(u1, &json!({"ref": "U1", "value": "LM358", "lib": "Amplifier_Operational:LM358", "why": "missing model for U1: assign one (model) or exclude it (exclude)", "mpn": "LM358DR", "maker": "Texas Instruments",
            "found": [{"name": "LM358", "subckt": ["1", "2", "3", "4", "5"], "file": "opamps.lib", "from": "project"}]}));
        // the connector has none here either: the answer says once how a model is found, and when to stop and ask
        assert!(m["next"].as_str().unwrap().contains("stop and ask the person") && m["parts"].as_array().unwrap().iter().any(|p| p["ref"] == "J1" && p.get("found").is_none()));
        assert_eq!((log.len(), log[0]["op"].as_str(), log[0]["lookups"].as_array().map(|l| l.len()), log[0]["lookups"].as_array().unwrap().iter().find(|l| l["ref"] == "U1").map(|l| l["found"][0]["from"].clone())), (1, Some("models"), Some(4), Some(json!("project"))));
        assert!(sent(&f, "GetSimulationModels")[0]["only_problems"] == true && log[0]["bytes"].as_u64().unwrap() < 1500);
        // every part, or the named ones, as a short row each
        let all = go(&f, json!([{"models": "all"}])); let rows = all["results"][0]["models"]["parts"].as_array().unwrap(); assert_eq!(rows.len(), 9);
        assert_eq!(rows.iter().find(|p| p["ref"] == "R1").unwrap(), &json!({"ref": "R1", "value": "10k", "device": "R"}));
        assert_eq!(go(&f, json!([{"models": ["C1"]}]))["results"][0]["models"]["parts"], json!([{"ref": "C1", "value": "10n", "device": "C"}]));
        assert_eq!(go_in(&f, &d, json!([{"models": "R99"}])).0.unwrap_err(), "op 1 (models): no part R99 in this schematic.");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn excluding_groups_and_assigning_models_saves_and_says_what_is_left() {
        let f = fake(); let out = go(&f, json!([{"exclude": ["connectors", "test_points", "mounting_holes"]}, {"model": "U1", "device": "r", "params": {"r": "1Meg"}}]));
        assert_eq!(out["results"][0], json!({"excluded": ["J1", "TP1", "H1"]}));
        assert_eq!(out["results"][1], json!({"model": [{"ref": "U1", "value": "LM358", "device": "R"}]}));
        assert_eq!((out["saved"].clone(), out["left"].clone()), (json!("/p/t.kicad_sch"), json!([])));
        let a = sent(&f, "AssignSimulationModel"); assert_eq!((a[0]["references"].clone(), a[0]["exclude_from_sim"].clone(), a[1]["builtin"].clone()), (json!(["J1", "TP1", "H1"]), json!(true), json!({"device": "R", "type": "", "params": [{"name": "r", "value": "1Meg"}]})));
        assert_eq!(names(&f).iter().filter(|n| *n == "SaveDocument").count(), 1);
        // patterns, and the way back; a part that still has no model is what is left
        let f = fake(); let out = go(&f, json!([{"exclude": {"ref": "J*"}}, {"exclude": "TP*"}, {"include": "TP1"}, {"exclude": {"lib": "Mechanical:*"}}]));
        assert_eq!((out["results"][0].clone(), out["results"][1].clone(), out["results"][2].clone(), out["results"][3].clone()), (json!({"excluded": ["J1"]}), json!({"excluded": ["TP1"]}), json!({"included": ["TP1"]}), json!({"excluded": ["H1"]})));
        assert_eq!(out["left"], json!(["missing model for TP1: assign one (model) or exclude it (exclude)", "missing model for U1: assign one (model) or exclude it (exclude)"]));
        // every part of the board's-sake groups that has no model, in one op: never the op-amp
        assert_eq!(go(&fake(), json!([{"exclude": "no_model"}]))["results"][0], json!({"excluded": ["H1", "J1", "TP1"]}));
        assert_eq!(go(&fake(), json!([{"exclude": "fiducials"}]))["results"][0], json!({"excluded": [], "note": "no part matches"}));
        let e = go_in(&fake(), Path::new("/x"), json!([{"exclude": "connectors"}, {"exclude": "Q7"}])).0.unwrap_err(); assert!(e.contains("op 2 (exclude):") && e.contains("no symbol Q7") && e.ends_with("Already done and saved: exclude."), "{e}");
        assert!(go_in(&fake(), Path::new("/x"), json!([{"model": "U1"}])).0.unwrap_err().contains("a model is one of: device"));
        // a save KiCad did not make is said
        let mut f = fake(); f.save = json!({"not_saved": "KiCad says the document is not modified: nothing was written"});
        let out = go(&f, json!([{"exclude": "J1"}])); assert_eq!((out["saved"].clone(), out["not_saved"].as_str().map(|t| t.contains("not modified"))), (json!(false), Some(true)));
    }
    #[test] fn a_library_model_goes_by_pin_names_or_by_order() {
        let d = tmp("lib"); std::fs::create_dir_all(d.join("sim/models")).unwrap();
        std::fs::write(d.join("sim/models/lm358.lib"), "* in+ in- vcc vee out\n.subckt LM358 IN+ IN-\n.ends\n.subckt LM358_5 1 2 3 4 5\n.ends\n").unwrap();
        let f = fake(); let (r, _) = go_in(&f, &d, json!([{"model": "U1", "lib": "sim/models/lm358.lib", "name": "LM358"}])); let out = r.unwrap();
        assert_eq!(out["results"][0], json!({"model": [{"ref": "U1", "value": "LM358", "device": "SUBCKT", "model": "LM358", "lib": "sim/models/lm358.lib", "pins": "1=IN+ 2=IN-"}], "pins_by": "name"}));
        let a = &sent(&f, "AssignSimulationModel")[0]; assert_eq!((a["library"].clone(), a["pins"].clone()), (json!({"library": "sim/models/lm358.lib", "name": "LM358", "params": []}), json!([{"pin_number": "1", "model_node": "IN+"}, {"pin_number": "2", "model_node": "IN-"}])));
        // numbered nodes: by order, said, with the nodes to check against the file; an absolute path inside the project is stored relative
        let f = fake(); let (r, _) = go_in(&f, &d, json!([{"model": "U1", "lib": d.join("sim/models/lm358.lib").to_string_lossy(), "name": "lm358_5"}])); let out = r.unwrap();
        assert_eq!((out["results"][0]["pins_by"].clone(), out["results"][0]["nodes"].clone(), out["results"][0]["model"][0]["lib"].clone()), (json!("order"), json!(["1", "2", "3", "4", "5"]), json!("sim/models/lm358.lib")));
        assert!(out["results"][0]["check"].as_str().unwrap().contains("give pins"));
        // a given map is sent as given; two models and no name asks which; a wrong name lists them; no file says so
        let f = fake(); go_in(&f, &d, json!([{"model": "U1", "lib": "sim/models/lm358.lib", "name": "LM358", "pins": {"2": "IN+", "1": "IN-"}}])).0.unwrap();
        assert_eq!(sent(&f, "AssignSimulationModel")[0]["pins"], json!([{"pin_number": "2", "model_node": "IN+"}, {"pin_number": "1", "model_node": "IN-"}]));
        assert!(go_in(&fake(), &d, json!([{"model": "U1", "lib": "sim/models/lm358.lib"}])).0.unwrap_err().contains("which model of sim/models/lm358.lib? name one: LM358, LM358_5"));
        assert!(go_in(&fake(), &d, json!([{"model": "U1", "lib": "sim/models/lm358.lib", "name": "TL072"}])).0.unwrap_err().contains("has no model `TL072`; it has LM358, LM358_5"));
        assert!(go_in(&fake(), &d, json!([{"model": "U1", "lib": "nope.lib"}])).0.unwrap_err().contains("cannot read the model file"));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_fetched_model_is_kept_with_its_source_and_assigned() {
        let d = tmp("import"); let f = fake();
        let (r, log) = go_in(&f, &d, json!([{"import": {"text": "* TI\n.subckt LM358 IN+ IN-\n.ends\n", "url": "https://www.ti.com/lit/zip/snom123", "licence": "TI SPICE model licence, see file header"}, "to": "U1"}]));
        let out = r.unwrap(); let i = &out["results"][0]["import"];
        assert_eq!((i["stored"].as_str(), i["source"].as_str(), i["models"].clone(), i["assigned"][0]["model"].as_str(), i["pins_by"].as_str()), (Some("sim/models/LM358.lib"), Some("sim/models/LM358.lib.source.json"), json!([{"name": "LM358", "subckt": ["IN+", "IN-"]}]), Some("LM358"), Some("name")));
        let note: Value = serde_json::from_str(&std::fs::read_to_string(d.join("sim/models/LM358.lib.source.json")).unwrap()).unwrap();
        assert_eq!((note["url"].as_str(), note["licence"].as_str(), note["fetched_at"].as_u64()), (Some("https://www.ti.com/lit/zip/snom123"), Some("TI SPICE model licence, see file header"), Some(1_800_000_000)));
        assert_eq!((out["saved"].clone(), out["left"].as_array().map(|l| l.iter().any(|x| x.as_str().unwrap().contains("U1")))), (json!("/p/t.kicad_sch"), Some(false)));
        assert_eq!(log[0]["lookups"][0], json!({"stored": "sim/models/LM358.lib", "url": "https://www.ti.com/lit/zip/snom123", "from": "vendor"}));
        // a file, kept under its own name, not assigned; then models finds it for U1
        std::fs::write(d.join("dl.cir"), ".model 1N4148 D(Is=1n)\n").unwrap();
        let (r, _) = go_in(&fake(), &d, json!([{"import": {"file": "dl.cir", "url": "https://vendor.example/1n4148.cir"}}, {"models": true}])); let out = r.unwrap();
        assert_eq!((out["results"][0]["import"]["stored"].as_str(), out["results"][0]["import"]["licence"].as_str().map(|l| l.starts_with("none given")), out.get("saved")), (Some("sim/models/dl.cir"), Some(true), None));
        assert_eq!(out["results"][1]["models"]["parts"].as_array().unwrap().iter().find(|p| p["ref"] == "U1").unwrap()["found"][0]["file"], json!("sim/models/LM358.lib"));
        // what is not a model stops there, in the words that say to ask the person
        for (text, want) in [("<html><body>Sign in to download</body></html>", "stop and ask the person"), ("$CDNENCSTART_ADV3 xx", "encrypted"), ("see datasheet", "no .subckt or .model")] {
            let e = go_in(&fake(), &d, json!([{"import": {"text": text, "url": "https://x"}, "to": "U1"}])).0.unwrap_err(); assert!(e.contains(want), "{e}") }
        assert!(go_in(&fake(), &d, json!([{"import": {"text": ".model X D\n"}}])).0.unwrap_err().contains("where the model came from") && go_in(&fake(), &d, json!([{"import": {"url": "https://x"}}])).0.unwrap_err().contains("`file` (a path) or `text`"));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_run_answers_numbers_not_vectors() {
        let f = fake(); let (r, log) = go_in(&f, Path::new("/x"), json!([{"run": ".ac dec 20 1 10meg", "signals": ["V(out)"], "measures": {"fc": "corner V(out)", "dc": {"dc_gain": "V(out)/V(in)"}, "g10k": "gain_at V(out) x=10k"}}]));
        let out = r.unwrap(); let run = &out["results"][0]["run"];
        assert!(near(&run["measures"]["fc"], FC, 2e-3) && run["measures"]["dc"].as_f64().unwrap().abs() < 1e-3 && near(&run["measures"]["g10k"], -10.0 * (1.0 + (1e4 / FC).powi(2)).log10(), 2e-3), "{run}");
        assert_eq!((run["analysis"].as_str(), run["points"].as_u64(), run["signals"]["V(out)"]["n"].as_u64(), run["signals"]["V(out)"]["first_db"].as_f64(), run["ms"].as_f64(), run.get("waves"), run.get("log"), run.get("netlist"), out.get("saved")), (Some("AC"), Some(141), Some(141), Some(-0.0), Some(11.0), None, None, None, None));
        assert!(out.to_string().len() < 500, "{} bytes", out.to_string().len());
        let m = &sent(&f, "RunSimulation")[0]; assert_eq!((m["command"].as_str(), m["signals"].clone(), m["wait_ms"].as_u64(), m["timeout_ms"].as_u64(), m["measures"].clone()), (Some(".ac dec 20 1 10meg"), json!(["V(out)", "V(in)"]), Some(5000), Some(120_000), json!([])));
        assert_eq!((log[0]["op"].as_str(), log[0]["analysis"].as_str(), log[0]["status"].as_str(), log[0]["runs"].as_u64(), log[0]["ngspice_ms"].as_f64(), log[0]["timings"]["netlist_ms"].as_f64(), log[0]["bytes"].as_u64() == Some(out["results"][0].to_string().len() as u64)), (Some("run"), Some("AC"), Some("SIM_OK"), Some(1), Some(8.0), Some(2.0), true));
        // waves when asked, as a few points; the netlist and the log when asked
        let out = go(&fake(), json!([{"run": ".tran 1u 1m", "signals": "V(out)", "waves": 20, "netlist": true, "log": true, "measures": {"tr": "rise_time V(out)", "ts": {"settling_time": "V(out)"}, "ng": "TRIG v(/out) VAL=0.1 RISE=1 TARG v(/out) VAL=0.9 RISE=1", "bad": "WHEN v(/out)=9 never"}}]));
        let run = &out["results"][0]["run"]; let w = &run["waves"]["V(out)"];
        assert!(w["x"].as_array().unwrap().len() <= 20 && w["of"] == 2001 && near(&run["measures"]["tr"], 1e-4 * 9f64.ln(), 3e-3) && near(&run["measures"]["ts"], 1e-4 * 50f64.ln(), 5e-3) && near(&run["measures"]["ng"], 1e-4 * 9f64.ln(), 1e-5), "{run}");
        assert_eq!((run["measures"]["bad"].as_str(), run["netlist"].as_str().map(|n| n.starts_with(".title")), run["log"].as_str().map(|l| l.contains("Doing analysis")), run["signals"]["V(out)"]["max"].as_f64().map(|m| m > 0.99)), (Some("failed: measure failed: out of interval"), Some(true), Some(true), Some(true)));
        // an operating point: the asked values, or all; overrides are this run's only
        let f = fake(); let out = go(&f, json!([{"run": ".op", "signals": ["mid"], "set": {"R3": "30k"}}, {"run": ".op"}]));
        assert_eq!((out["results"][0]["run"]["values"].clone(), out["results"][1]["run"]["values"].clone()), (json!({"V(/mid)": 3.75}), json!({"V(/out)": 1.0, "V(/mid)": 2.5, "V(/vcc)": 5.0, "I(V1)": 0.0})));
        assert_eq!(sent(&f, "RunSimulation")[0]["overrides"], json!([{"reference": "R3", "field": "", "value": "30k"}]));
        // a signal the run does not have is said in its place, with what it has; no signal named says what there is
        let out = go(&fake(), json!([{"run": ".ac dec 20 1 10meg", "signals": ["V(nope)"], "measures": {"x": "corner V(nope)"}}])); let run = &out["results"][0]["run"];
        assert!(run["signals"]["V(nope)"].as_str().unwrap().starts_with("failed: no `V(nope)` in this run; it has frequency") && run["measures"]["x"].as_str().unwrap().starts_with("failed: no `V(nope)`"));
        let out = go(&fake(), json!([{"run": ".ac dec 20 1 10meg"}])); assert_eq!(out["results"][0]["run"]["has"], json!("frequency, V(/out), V(/in)"));
        assert!(go_in(&fake(), Path::new("/x"), json!([{"run": ".plot x"}])).0.unwrap_err().contains("is not an analysis"));
    }
    #[test] fn a_saved_analysis_and_probes() {
        let f = fake(); let out = go(&f, json!([{"probe": ["V(out)", "I(R1)"]}, {"analysis": "ac dec 20 1 10meg", "signals": ["V(in)"]}, {"probe": "V(mid)"}, {"analysis": true}, {"run": true, "measures": {"fc": "corner V(out)"}}]));
        assert!(out["results"][0]["probe"]["note"].as_str().unwrap().contains("no analysis is saved yet"));
        assert_eq!(out["results"][1], json!({"analysis": {"tab": 0, "command": ".ac dec 20 1 10meg", "signals": ["V(in)", "V(out)", "I(R1)"], "changed": true}}));
        assert_eq!(out["results"][2]["probe"], json!({"signals": ["V(in)", "V(out)", "I(R1)", "V(mid)"], "saved_with": ".ac dec 20 1 10meg"}));
        assert_eq!(out["results"][3], json!({"analysis": {"tabs": [{"command": ".ac dec 20 1 10meg", "signals": ["V(in)", "V(out)", "I(R1)", "V(mid)"]}]}}));
        // the saved analysis runs, reading this call's probes too
        let run = &out["results"][4]["run"]; assert!(near(&run["measures"]["fc"], FC, 2e-3) && run["command"] == ".ac dec 20 1 10meg" && run["signals"]["V(out)"]["n"] == 141, "{run}");
        assert_eq!(sent(&f, "RunSimulation")[0]["command"], json!(""));
        // a probe on a pin labels its net first (a nested kicad_edit), then reads it
        let f = fake(); let out = go(&f, json!([{"probe": {"pin": "R1.2", "name": "MID"}}]));
        assert_eq!((out["results"][0]["probe"]["label"].clone(), out["results"][0]["probe"]["signals"].clone(), out["saved"].clone()), (json!({"text": "MID", "at": "R1.2"}), json!(["V(MID)"]), json!("/p/t.kicad_sch")));
        assert!(names(&f).contains(&"CreateItems".to_string()));
    }
    #[test] fn a_source_is_a_symbol_between_two_nets_with_its_model() {
        let f = fake(); let out = go(&f, json!([{"source": {"type": "sin", "pos": "IN", "params": {"ampl": 1, "f": "1k"}, "ac": 1}}, {"source": {"pos": "VCC", "value": 5, "ref": "V9"}}, {"source": {"type": "pulse", "current": true, "pos": "LOAD", "neg": "GND", "params": {"y2": "100m"}}}]));
        let s0 = &out["results"][0]["source"]; assert_eq!((s0["ref"].as_str(), s0["pos"].as_str(), s0["neg"].as_str(), s0["model"].as_str(), s0["line"].as_str()), (Some("V2"), Some("IN"), Some("GND"), Some("V SIN"), Some("V2 + - SIN ampl=1 f=1k ac=1")));
        assert_eq!((out["results"][1]["source"]["ref"].as_str(), out["results"][2]["source"]["ref"].as_str(), out["results"][2]["source"]["model"].as_str()), (Some("V9"), Some("I1"), Some("I PULSE")));
        let placed = sent(&f, "PlaceSymbolFromLibrary"); assert_eq!((placed[0]["lib_id"].clone(), placed.iter().any(|p| p["lib_id"]["entry_name"] == "IPULSE")), (json!({"library_nickname": "Simulation_SPICE", "entry_name": "VSIN"}), true));
        let a = sent(&f, "AssignSimulationModel"); assert_eq!((a.len(), a[1]["builtin"].clone()), (3, json!({"device": "V", "type": "DC", "params": [{"name": "dc", "value": "5"}]})));
        assert_eq!(out["saved"], json!("/p/t.kicad_sch"));
        let up = sent(&f, "UpdateItems"); assert_eq!((up.len(), up[0]["items"][0]["attributes"]["exclude_from_bill_of_materials"].clone(), up[0]["items"][0]["attributes"]["exclude_from_board"].clone()), (3, json!(true), json!(true)));
        let f = fake(); go(&f, json!([{"source": {"pos": "VCC", "value": 5, "bom": true}}])); assert!(sent(&f, "UpdateItems").is_empty());
        assert!(go_in(&fake(), Path::new("/x"), json!([{"source": {"type": "square", "pos": "IN"}}])).0.unwrap_err().contains("no source type `square`") && go_in(&fake(), Path::new("/x"), json!([{"source": {}}])).0.unwrap_err().contains("source.pos"));
    }
    #[test] fn a_sweep_is_one_table() {
        let f = fake(); let (r, log) = go_in(&f, Path::new("/x"), json!([{"sweep": "R1", "values": ["1k", "10k", "100k"], "run": ".ac dec 20 1 10meg", "measures": {"fc": "corner V(out)", "g1k": "gain_at V(out) x=1k"}}]));
        let out = r.unwrap(); let t = &out["results"][0]["sweep"];
        assert_eq!((t["columns"].clone(), t["runs"].as_u64(), t["rows"].as_array().map(|r| r.len())), (json!(["R1", "fc", "g1k"]), Some(3), Some(3)));
        for (i, r1) in [1e3, 1e4, 1e5].iter().enumerate() { assert!(t["rows"][i][0] == json!(*r1) && near(&t["rows"][i][1], 1.0 / (2.0 * PI * r1 * 10e-9), 2e-3), "{t}") }
        assert_eq!(sent(&f, "RunSimulation").iter().map(|m| m["overrides"][0]["value"].clone()).collect::<Vec<_>>(), vec![json!("1000"), json!("10000"), json!("100000")]);
        assert_eq!((log[0]["runs"].as_u64(), log[0]["ngspice_ms"].as_f64(), out.get("saved"), names(&f).contains(&"SaveDocument".to_string())), (Some(3), Some(24.0), None, false));
        // a range from a series, with a fixed override beside it; a .param
        let f = fake(); let out = go(&f, json!([{"sweep": "C1", "from": "1n", "to": "10n", "series": "E3", "set": {"R1": "1k"}, "run": ".ac dec 20 1 10meg", "measures": {"fc": "corner V(out)"}}]));
        assert_eq!(out["results"][0]["sweep"]["rows"].as_array().unwrap().iter().map(|r| r[0].clone()).collect::<Vec<_>>(), vec![json!(1e-9), json!(2.2e-9), json!(4.7e-9), json!(1e-8)]);
        assert_eq!(sent(&f, "RunSimulation")[1]["overrides"], json!([{"reference": "R1", "field": "", "value": "1k"}, {"reference": "C1", "field": "", "value": "0.0000000022"}]));
        let f = fake(); go(&f, json!([{"sweep": {"param": "gain"}, "values": [1, 2], "run": ".op", "measures": {"v": "final V(mid)"}}]));
        assert_eq!(sent(&f, "RunSimulation")[1]["directives"], json!([".param gain=2"]));
        assert!(go_in(&fake(), Path::new("/x"), json!([{"sweep": "R1", "values": [1], "run": ".op"}])).0.unwrap_err().contains("a sweep answers measures per step"));
        // a step that fails is a row of nothing and a line, not the end of the table
        let mut f = fake(); f.mode = "noconv"; let out = go(&f, json!([{"sweep": "R1", "values": [1, 2], "run": ".tran 1u 1m", "measures": {"tr": "rise_time V(out)"}}])); let t = &out["results"][0]["sweep"];
        assert!(t["rows"][0] == json!([1.0, null]) && t["failed"][0]["why"].as_str().unwrap().starts_with("no convergence: no convergence at t=1.5e-05 (near /sw)"), "{t}");
    }
    #[test] fn a_tune_finds_the_value_and_writes_it_when_asked() {
        // R1 for a 1 kHz corner with 10 nF: 15.915k; from E96 that is 15.8k (1007 Hz) or 16.2k (982 Hz): the nearer result
        let f = fake(); let (r, log) = go_in(&f, Path::new("/x"), json!([{"tune": "R1", "measure": "corner V(out)", "target": 1000, "from": "1k", "to": "100k", "run": ".ac dec 20 1 10meg"}]));
        let out = r.unwrap(); let t = &out["results"][0]["tune"];
        assert!(near(&json!(kp::parse_value(t["value"].as_str().unwrap()).unwrap()), 15915.5, 2e-3) && t["error_pct"].as_f64().unwrap().abs() < 0.11 && t["runs"].as_u64().unwrap() <= 5 && t.get("written").is_none(), "{t}");
        assert_eq!((log[0]["runs"].as_u64() == t["runs"].as_u64(), out.get("saved"), s(&f.parts.lock().unwrap()["R1"], "value").to_string()), (true, None, "10k".to_string()));
        let f = fake(); let out = go(&f, json!([{"tune": "R1", "measure": {"corner": "V(out)"}, "target": 1000, "from": "1k", "to": "100k", "series": "E96", "write": true, "run": ".ac dec 20 1 10meg"}])); let t = &out["results"][0]["tune"];
        assert_eq!((t["value"].as_str(), t["series"].as_str(), t["written"].as_bool(), t["part"].as_str(), out["saved"].as_str()), (Some("15.8k"), Some("E96"), Some(true), Some("R1"), Some("/p/t.kicad_sch")));
        assert!(near(&t["measure"], 1.0 / (2.0 * PI * 15.8e3 * 10e-9), 2e-3) && t["exact"].as_str().unwrap().starts_with("15.9") && t["runs"].as_u64().unwrap() <= 7, "{t}");
        assert_eq!(s(&f.parts.lock().unwrap()["R1"], "value"), "15.8k");
        // a divider to 3.3 V from E24; a target the range cannot reach says what the range gives; a parameter is not written
        let out = go(&fake(), json!([{"tune": "R3", "measure": "final V(mid)", "target": 3.3, "from": "1k", "to": "100k", "series": "E24", "run": ".op"}]));
        assert_eq!(out["results"][0]["tune"]["value"], json!("20k"));
        let e = go_in(&fake(), Path::new("/x"), json!([{"tune": "R1", "measure": "corner V(out)", "target": 5, "from": "1k", "to": "100k", "run": ".ac dec 20 1 10meg"}])).0.unwrap_err(); assert!(e.contains("the target 5 is not between them"), "{e}");
        assert!(go_in(&fake(), Path::new("/x"), json!([{"tune": "V1.dc", "measure": "final V(mid)", "target": 1, "from": 1, "to": 2, "run": ".op", "write": true}])).0.unwrap_err().contains("is a parameter"));
        assert!(go_in(&fake(), Path::new("/x"), json!([{"tune": "R1", "target": 1, "from": 1, "to": 2, "run": ".op"}])).0.unwrap_err().contains("tune needs the measure"));
    }
    #[test] fn a_loop_gain_is_read_at_a_cut_in_the_netlist() {
        // T = 4 / (1 + s/p)^3 (the amplifier's 8 halved by 1k into 1k): the phase is -180 at p sqrt(3), where |T| = 1/2
        let f = fake(); let (r, log) = go_in(&f, Path::new("/x"), json!([{"loopgain": "U1.2"}])); let out = r.unwrap(); let l = &out["results"][0]["loopgain"];
        let wu = 1e3 * (4f64.powf(2.0 / 3.0) - 1.0).sqrt();
        assert!(near(&l["crossover"], wu, 3e-3) && near(&l["phase_margin"], 180.0 - 3.0 * (wu / 1e3).atan().to_degrees(), 5e-3) && near(&l["gain_margin"], 20.0 * 2f64.log10(), 5e-3) && near(&l["dc_gain_db"], 20.0 * 4f64.log10(), 1e-3), "{l}");
        assert_eq!((l["at"].as_str(), l["runs"].as_u64(), l.get("curve"), out.get("saved"), log[0]["runs"].as_u64()), (Some("U1.2"), Some(3), None, None, Some(3)));
        let runs = sent(&f, "RunSimulation"); assert_eq!((runs.len(), runs[1]["breaks"][0]["insert"].as_str(), runs[2]["breaks"][0]["insert"].as_str(), runs[1]["signals"].clone(), runs[1]["show"].clone(), runs[1]["command"].as_str()),
            (3, Some("Vlg_{name} {pin} {net} dc 0 ac 1\nIlg_{name} 0 {pin} dc 0 ac 0"), Some("Vlg_{name} {pin} {net} dc 0 ac 0\nIlg_{name} 0 {pin} dc 0 ac 1"), json!(["brk_U1_2", "vlg_u1_2#branch"]), json!(false), Some(".ac dec 50 1 100meg")));
        let out = go(&fake(), json!([{"loopgain": "U1.2", "waves": 12, "run": "ac dec 20 1 1meg"}])); let c = &out["results"][0]["loopgain"]["curve"]; assert!(c["f"].as_array().unwrap().len() <= 12 && c["db"][0].as_f64().unwrap() > 12.0, "{c}");
        assert!(go_in(&fake(), Path::new("/x"), json!([{"loopgain": "Q9.1"}])).0.unwrap_err().contains("the schematic has no symbol Q9") && go_in(&fake(), Path::new("/x"), json!([{"loopgain": "U1"}])).0.unwrap_err().contains("reference.pin number")
            && go_in(&fake(), Path::new("/x"), json!([{"loopgain": "U1.2", "run": ".tran 1u 1m"}])).0.unwrap_err().contains("is an .ac analysis"));
    }
    #[test] fn a_transient_that_does_not_converge_is_tried_again() {
        let mut f = fake(); f.mode = "stiff"; let (r, log) = go_in(&f, Path::new("/x"), json!([{"run": ".tran 1u 1m", "signals": "V(out)", "directives": [".include x.lib"], "options": {"save_all_currents": false}, "measures": {"fin": "final V(out)"}}]));
        let out = r.unwrap(); let run = &out["results"][0]["run"];
        assert_eq!((run["needed"].as_str(), run.get("failed"), run["measures"]["fin"].as_f64().map(|v| v > 0.99)), (Some("gear with reltol 0.01: it did not converge without (.options method=gear reltol=0.01)"), None, Some(true)));
        let sent = sent(&f, "RunSimulation"); assert_eq!((sent.len(), sent[0]["directives"].clone(), sent[2]["directives"].clone(), sent[0]["options"].clone()), (3, json!([".include x.lib"]), json!([".include x.lib", ".options method=gear reltol=0.01"]), json!({"save_all_currents": false})));
        assert_eq!((log[0]["runs"].as_u64(), log[0]["retries"].clone(), log[0]["status"].as_str()), (Some(3), json!(["gear integration", "gear with reltol 0.01"]), Some("SIM_NO_CONVERGENCE")));
        // nothing helps: the failure says what was tried; retry: false leaves it at one run; other analyses are not tried again
        let mut f = fake(); f.mode = "noconv"; let out = go(&f, json!([{"run": ".tran 1u 1m"}])); let p = out["results"][0]["run"]["problems"].as_array().unwrap();
        assert!(p.last().unwrap().as_str().unwrap().starts_with("tried without success: gear integration; then gear with reltol 0.01; then gear, reltol 0.01, looser tolerances") && self::sent(&f, "RunSimulation").len() == 4 && self::sent(&f, "RunSimulation")[3]["command"] == ".tran 1u 1m 0 1u", "{p:?}");
        let mut f = fake(); f.mode = "noconv"; go(&f, json!([{"run": ".tran 1u 1m", "retry": false}, {"run": ".op"}])); assert_eq!(self::sent(&f, "RunSimulation").len(), 1);
        assert_eq!((smaller_step(".tran 1u 10m"), smaller_step(".tran 1u 10m 0 10u uic"), smaller_step(".tran 1u 10m 1m 0.1u"), smaller_step(".tran 1u 10m\nfft v(out)"), smaller_step(".op")), (".tran 1u 10m 0 1u".into(), ".tran 1u 10m 0 1u uic".into(), ".tran 1u 10m 1m 0.1u".into(), ".tran 1u 10m 0 1u\nfft v(out)".into(), ".op".into()));
    }
    #[test] fn failures_are_one_line_each_with_what_to_do() {
        let mut f = fake(); f.mode = "float"; let (r, log) = go_in(&f, Path::new("/x"), json!([{"exclude": "J1"}, {"run": ".op", "signals": ["mid"]}, {"run": ".ac dec 10 1 1k"}]));
        let out = r.unwrap(); let run = &out["results"][1]["run"];
        assert_eq!(run, &json!({"analysis": "OP", "failed": "a node floats", "problems": ["node Net-(C1-Pad2) floats: nothing sets its DC level. Give it a path to ground (a resistor, a source), or exclude what hangs there", "missing model for U1: assign one (model) or exclude it (exclude)"], "ms": 4.0}));
        // the ops after a failed run are not run; what was written before it is saved; never ngspice's console unless asked
        assert_eq!((out["results"].as_array().unwrap().len(), out["skipped"].as_str(), out["saved"].as_str(), out.to_string().contains("singular matrix")), (2, Some("the 1 op after the failed run was not run"), Some("/p/t.kicad_sch"), false));
        assert_eq!((log.len(), log[1]["status"].as_str(), log[1]["problems"].as_array().map(|p| p.len())), (2, Some("SIM_FLOATING_NODE"), Some(2)));
        let out = go(&f, json!([{"run": ".op", "log": true}])); assert!(out["results"][0]["run"]["log"].as_str().unwrap().contains("singular matrix"));
        // no problems from the fork: ngspice's own words are read
        let mut f = fake(); f.mode = "noconv"; let out = go(&f, json!([{"run": ".tran 1u 1m", "retry": false}]));
        assert_eq!(out["results"][0]["run"]["problems"], json!(["no convergence at t=1.5e-05 (near /sw): shorten the maximum time step (the third value of .tran), give edges a rise time, or add directives: .options method=gear reltol=1e-3"]));
        // another run is in progress; a long run is collected when it ends
        let mut f = fake(); f.mode = "busy"; assert_eq!(go_in(&f, Path::new("/x"), json!([{"run": ".op"}])).0.unwrap_err(), format!("op 1 (run): {BUSY}."));
        let mut f = fake(); f.mode = "slow"; let out = go(&f, json!([{"run": ".op", "signals": "mid"}]));
        assert_eq!((out["results"][0]["run"]["values"].clone(), sent(&f, "GetSimulationResult").len(), sent(&f, "GetSimulationResult")[0]["run_id"].clone()), (json!({"V(/mid)": 2.5}), 3, json!("7")));
    }
}
