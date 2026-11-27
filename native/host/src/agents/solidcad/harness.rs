//! The decisions of the CAD harness, as pure functions with tests: which calls change the design (and so get a checkpoint first and a
//! digest after), how long each may take before the kernel counts as stalled, when the same failing call has been sent too often, what
//! the agent is told next (look at the model after it changed), and the compact summaries of parameters and mass properties.
use serde_json::{json, Map, Value};
use std::{path::{Path, PathBuf}, time::Duration};

/// Every tool of ours starts with this (the upstream tool `measure` is `solidcraft_measure`).
pub const PREFIX: &str = "solidcraft_";
/// Upstream tools an agent in Backplane does not get: they act on the person's app, not the design.
pub const HIDDEN: &[&str] = &["quit_app", "preferences", "agent_cursor", "crash_reports", "documents"];
/// The harness's own tools.
pub const OWN: &[&str] = &["solidcraft_status", "solidcraft_checkpoint", "solidcraft_summary", "solidcraft_look", "solidcraft_call"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// reads the design: no checkpoint, may be asked again after a restart
    Read,
    /// changes the design: a checkpoint first, a digest after
    Edit,
    /// replaces the design in the session (open, new_design): a checkpoint first, the design's file is tracked after
    Replace,
    /// writes files and leaves the design as it is (save, export)
    Write,
    /// draws the design (screenshot, compare_picture): counts as a look
    Look,
    /// about a request, not the design (pending, cancel, running)
    Control,
}

/// Commands run through `execute` that only read (prefixes; everything else through `execute` or `batch` counts as an edit).
const READ_COMMANDS: &[&str] = &["inspect.", "view.", "select.", "parameters.list", "parameters.users", "parameters.graph", "parameters.complete", "engine.describe", "document.list", "doc.recovery_list", "sketch.inspect", "render.status", "render.preflight", "measure."];

pub fn class(tool: &str, args: &Value) -> Class {
    match tool {
        "version" | "list_commands" | "describe_command" | "inspect_design" | "measure" | "body_topology" | "skill_reference" | "check_parameters" => Class::Read,
        "pending" | "cancel" | "running" => Class::Control,
        "screenshot" | "compare_picture" => Class::Look,
        "save" | "export" => Class::Write,
        "open" | "new_design" => Class::Replace,
        "execute" => { let c = args.get("command").and_then(Value::as_str).unwrap_or("");
            if matches!(c, "doc.open" | "doc.new" | "doc.recover" | "file.revert_saved") { Class::Replace }
            else if matches!(c, "file.save" | "file.save_as" | "file.export" | "file.save_mesh") { Class::Write }
            else if READ_COMMANDS.iter().any(|p| c.starts_with(p)) { Class::Read } else { Class::Edit } }
        _ => Class::Edit, // batch, set_parameter, undo, redo, plugin tools: assume the design changes
    }
}

/// How long a call may take before it counts as stuck. Opening a large STEP assembly and meshing a fine export are the slow ones.
pub fn budget(tool: &str, class: Class) -> Duration {
    Duration::from_secs(match (tool, class) {
        ("open", _) | ("export", _) | ("batch", _) => 600,
        ("pending", _) => 90, // it waits up to 60 s by itself
        (_, Class::Control) => 20,
        (_, Class::Read) => 120,
        (_, Class::Look) => 180,
        _ => 300,
    })
}

/// The same call failing again and again (the agent is stuck, not the kernel): counted by tool and arguments, reset by any success.
#[derive(Default, Debug)]
pub struct Repeats { last: String, n: u32 }
pub const REPEAT_LIMIT: u32 = 3;
impl Repeats {
    /// Records one outcome; Some(n) when this exact call has now failed `n` >= REPEAT_LIMIT times in a row.
    pub fn record(&mut self, tool: &str, args: &Value, ok: bool) -> Option<u32> {
        if ok { self.last.clear(); self.n = 0; return None }
        let key = format!("{tool}\n{args}");
        if key == self.last { self.n += 1 } else { self.last = key; self.n = 1 }
        (self.n >= REPEAT_LIMIT).then_some(self.n)
    }
}
pub fn repeat_advice(tool: &str, n: u32) -> String {
    format!("This exact {PREFIX}{tool} call has failed {n} times in a row: sending it again will not help. Read the error, check the command's parameters with {PREFIX}describe_command (or {PREFIX}skill_reference), inspect the design with {PREFIX}summary, or go back with {PREFIX}checkpoint {{action:\"restore\"}} and take another route.")
}

/// A path argument as the server needs it: absolute, a relative one from the project's design folder. `..` is left to the server's own
/// folder rules (it resolves before it compares).
pub fn abs_path(root: &Path, p: &str) -> String {
    let p = p.trim();
    if p.is_empty() || crate::plat::fs::is_abs(p) { return p.to_string() }
    if let Some(rest) = p.strip_prefix("~/") { if let Some(h) = std::env::var_os("HOME") { return PathBuf::from(h).join(rest).to_string_lossy().into_owned() } }
    root.join(p).to_string_lossy().into_owned()
}
/// `args` with its file arguments made absolute: the tool's own `path` / `reference`, and the `path` of an `execute` or `batch` step.
pub fn resolve_paths(root: &Path, tool: &str, args: &Value) -> Value {
    let mut a = args.clone();
    let fix = |o: &mut Map<String, Value>, k: &str| { if let Some(s) = o.get(k).and_then(Value::as_str) { let n = abs_path(root, s); o.insert(k.into(), Value::String(n)); } };
    let Some(o) = a.as_object_mut() else { return a };
    match tool {
        "open" | "save" | "export" | "screenshot" => fix(o, "path"),
        "compare_picture" => fix(o, "reference"),
        "execute" => { if let Some(p) = o.get_mut("params").and_then(Value::as_object_mut) { fix(p, "path") } }
        "batch" => { fix(o, "path"); if let Some(steps) = o.get_mut("commands").and_then(Value::as_array_mut) { for s in steps { if let Some(p) = s.get_mut("params").and_then(Value::as_object_mut) { fix(p, "path") } } } }
        _ => {}
    }
    a
}
/// The files a call writes inside the project (for the turn's snapshot), as absolute paths.
pub fn written_paths(tool: &str, args: &Value) -> Vec<String> {
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
    match tool {
        "save" | "export" => s(args, "path").into_iter().collect(),
        "screenshot" => s(args, "path").into_iter().collect(),
        "execute" if matches!(class(tool, args), Class::Write) => args.get("params").and_then(|p| s(p, "path")).into_iter().collect(),
        _ => vec![],
    }
}

/// What `inspect_design` and `measure {summary}` say, boiled down: the block every edit's answer carries, so the agent sees what its
/// change did without a second call.
pub fn digest(inspect: Option<&Value>, measure: Option<&Value>) -> Value {
    let mut d = Map::new();
    if let Some(i) = inspect {
        if let Some(p) = i.get("path").and_then(Value::as_str) { d.insert("file".into(), json!(p)); }
        d.insert("unsaved".into(), json!(i.get("dirty").and_then(Value::as_bool).unwrap_or(false)));
        let tl = i.get("timeline").and_then(Value::as_array).cloned().unwrap_or_default();
        d.insert("features".into(), json!(tl.len()));
        let bad: Vec<Value> = tl.iter().filter(|f| f.get("error").map_or(false, |e| !e.is_null()) || f.get("warning").map_or(false, |e| !e.is_null()))
            .map(|f| json!({"feature": f.get("name").cloned().unwrap_or(Value::Null), "error": f.get("error").cloned().unwrap_or(Value::Null), "warning": f.get("warning").cloned().unwrap_or(Value::Null)})).collect();
        if !bad.is_empty() { d.insert("failing".into(), Value::Array(bad)); }
    }
    if let Some(m) = measure {
        d.insert("bodies".into(), m.get("body_count").cloned().unwrap_or(json!(0)));
        if let Some(v) = m.pointer("/total/volume_mm3").and_then(Value::as_f64) { d.insert("volume_mm3".into(), json!(round(v, 3))); }
        if let Some(b) = bbox_of(m) { d.insert("bbox".into(), b); }
        let open = m.get("bodies").and_then(Value::as_array).map_or(0, |a| a.iter().filter(|b| b.get("closed").and_then(Value::as_bool) == Some(false)).count());
        if open > 0 { d.insert("open_bodies".into(), json!(open)); }
    }
    Value::Object(d)
}
fn round(v: f64, places: i32) -> f64 { let k = 10f64.powi(places); let r = (v * k).round() / k; if r == 0.0 { 0.0 } else { r } }
/// The box round every measured body: {min, max, size}.
fn bbox_of(m: &Value) -> Option<Value> {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]); let mut any = false;
    for b in m.get("bodies")?.as_array()? {
        let (Some(mn), Some(mx)) = (b.pointer("/bbox/min").and_then(Value::as_array), b.pointer("/bbox/max").and_then(Value::as_array)) else { continue };
        if mn.len() != 3 || mx.len() != 3 { continue }
        for i in 0..3 { let (a, z) = (mn[i].as_f64()?, mx[i].as_f64()?); lo[i] = lo[i].min(a); hi[i] = hi[i].max(z); } any = true;
    }
    if !any { return None }
    let r = |a: [f64; 3]| [round(a[0], 3), round(a[1], 3), round(a[2], 3)];
    Some(json!({"min": r(lo), "max": r(hi), "size": r([hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]])}))
}

/// `solidcraft_summary`: the parameters that drive the design, its features (the failing ones named), and each body's mass properties.
pub fn summary(inspect: &Value, measure: &Value) -> Value {
    let params: Vec<Value> = inspect.get("params").and_then(Value::as_array).map(|a| a.iter().map(|p| {
        let mut o = json!({"name": p["name"], "expression": p["expression"], "value": p["value"], "unit": p["unit"]});
        if p.get("comment").and_then(Value::as_str).map_or(false, |c| !c.is_empty()) { o["comment"] = p["comment"].clone() }
        if p.get("error").map_or(false, |e| !e.is_null()) { o["error"] = p["error"].clone() }
        o }).collect()).unwrap_or_default();
    let features: Vec<Value> = inspect.get("timeline").and_then(Value::as_array).cloned().unwrap_or_default();
    let bodies: Vec<Value> = measure.get("bodies").and_then(Value::as_array).map(|a| a.iter().map(|b| {
        let mut o = Map::new();
        for k in ["name", "closed", "material", "bbox"] { if let Some(v) = b.get(k) { o.insert(k.into(), v.clone()); } }
        for (k, places) in [("volume_mm3", 3), ("area_mm2", 3), ("mass_g", 4), ("density_g_cm3", 4)] { if let Some(v) = b.get(k).and_then(Value::as_f64) { o.insert(k.into(), json!(round(v, places))); } }
        if let Some(c) = b.get("center_of_mass").and_then(Value::as_array) { o.insert("center_of_mass".into(), Value::Array(c.iter().map(|x| json!(round(x.as_f64().unwrap_or(0.0), 3))).collect())); }
        Value::Object(o) }).collect()).unwrap_or_default();
    let mut total = Map::new();
    for (k, places) in [("volume_mm3", 3), ("area_mm2", 3), ("mass_g", 4)] { if let Some(v) = measure.pointer(&format!("/total/{k}")).and_then(Value::as_f64) { total.insert(k.into(), json!(round(v, places))); } }
    if let Some(b) = bbox_of(measure) { total.insert("bbox".into(), b); }
    json!({
        "file": inspect.get("path").cloned().unwrap_or(Value::Null), "name": inspect.get("name").cloned().unwrap_or(Value::Null),
        "unsaved": inspect.get("dirty").and_then(Value::as_bool).unwrap_or(false), "units": inspect.get("units").cloned().unwrap_or(json!("mm")),
        "parameters": params, "features": features, "bodies": bodies, "total": Value::Object(total),
    })
}

/// How many edits may pile up before the answer says to look at the model.
pub const LOOK_AFTER: u32 = 1;
/// What to do next, after an edit that went through: the model → render → inspect loop, said once it is due.
pub fn next_hint(edits_since_look: u32, digest: &Value, has_file: bool) -> String {
    let mut s = Vec::new();
    if digest.get("failing").is_some() { s.push(format!("A feature fails: fix it before building on it (execute timeline.edit, or {PREFIX}checkpoint restore).")) }
    if digest.get("open_bodies").is_some() { s.push("A body is open (not a closed solid): it has no volume and will not print or export as a solid.".to_string()) }
    if edits_since_look >= LOOK_AFTER { s.push(format!("The geometry changed {edits_since_look} time(s) since you last saw it: call {PREFIX}look and check the picture against the intent before the next stage.")) }
    if !has_file { s.push(format!("This design has no file yet: {PREFIX}save {{path:\"mech/<name>.sc\"}} keeps it (and lets Backplane restore it if the kernel stalls).")) }
    s.join(" ")
}

/// A tool definition as agents see it: prefixed, its description capped (the bundled skill and `solidcraft_skill_reference` hold the long form).
pub const DESC_MAX: usize = 1400;
pub fn listed(def: &Value) -> Option<Value> {
    let name = def.get("name").and_then(Value::as_str)?;
    if HIDDEN.contains(&name) || name.contains('.') { return None } // (a plugin's `<plugin>.<entry>` goes through solidcraft_call)
    let mut d = def.clone();
    d["name"] = json!(format!("{PREFIX}{name}"));
    if let Some(desc) = def.get("description").and_then(Value::as_str) {
        if desc.len() > DESC_MAX { let cut = (0..=DESC_MAX).rev().find(|i| desc.is_char_boundary(*i)).unwrap_or(0); d["description"] = json!(format!("{}… (more: {PREFIX}skill_reference)", &desc[..cut])); }
    }
    if let Some(o) = d.as_object_mut() { o.remove("title"); o.remove("outputSchema"); }
    Some(d)
}
/// The upstream name of one of our tool names, None for the harness's own.
pub fn upstream(name: &str) -> Option<&str> { if OWN.contains(&name) { None } else { name.strip_prefix(PREFIX) } }

/// Whether a project folder gets the CAD tools up front: it holds CAD files or a `mech` folder, or nothing of KiCad or Altium yet (a new
/// project, where the first request may be "model a part"), or SolidCraft is open on it.
pub fn wanted(has_cad_files: bool, has_mech_dir: bool, has_board_files: bool, pane_open: bool) -> bool { has_cad_files || has_mech_dir || pane_open || !has_board_files }

/// Checkpoints kept per design, oldest dropped first.
pub const KEEP: usize = 24;
/// The checkpoint ids to delete when `ids` (ascending) grows past KEEP.
pub fn prune(ids: &[u64]) -> Vec<u64> { if ids.len() > KEEP { ids[..ids.len() - KEEP].to_vec() } else { vec![] } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes() {
        assert_eq!(class("measure", &json!({})), Class::Read);
        assert_eq!(class("batch", &json!({})), Class::Edit);
        assert_eq!(class("set_parameter", &json!({})), Class::Edit);
        assert_eq!(class("execute", &json!({"command":"solid.box"})), Class::Edit);
        assert_eq!(class("execute", &json!({"command":"inspect.silhouette"})), Class::Read);
        assert_eq!(class("execute", &json!({"command":"doc.open"})), Class::Replace);
        assert_eq!(class("execute", &json!({"command":"file.save_as"})), Class::Write);
        assert_eq!(class("open", &json!({})), Class::Replace);
        assert_eq!(class("export", &json!({})), Class::Write);
        assert_eq!(class("screenshot", &json!({})), Class::Look);
        assert_eq!(class("cancel", &json!({})), Class::Control);
        assert_eq!(class("someplugin_tool", &json!({})), Class::Edit, "an unknown tool is assumed to change the design");
    }

    #[test]
    fn budgets_are_bounded_and_slow_calls_get_longer() {
        assert!(budget("measure", Class::Read) < budget("open", Class::Replace));
        assert!(budget("running", Class::Control) <= Duration::from_secs(20));
        assert!(budget("pending", Class::Control) > Duration::from_secs(60), "pending waits a minute by itself");
    }

    #[test]
    fn the_same_failing_call_is_noticed_and_a_success_resets_it() {
        let mut r = Repeats::default(); let a = json!({"command":"solid.fillet","params":{"radius":500}});
        assert_eq!(r.record("execute", &a, false), None);
        assert_eq!(r.record("execute", &a, false), None);
        assert_eq!(r.record("execute", &a, false), Some(3));
        assert_eq!(r.record("execute", &a, false), Some(4));
        assert_eq!(r.record("execute", &json!({"command":"solid.fillet","params":{"radius":2}}), false), None, "other arguments start over");
        assert_eq!(r.record("measure", &json!({}), true), None);
        assert_eq!(r.record("execute", &a, false), None);
        assert!(repeat_advice("execute", 3).contains("solidcraft_checkpoint"));
    }

    #[test]
    fn relative_paths_are_from_the_design_folder() {
        let root = Path::new("/home/user/proj");
        assert_eq!(abs_path(root, "mech/a.sc"), "/home/user/proj/mech/a.sc");
        assert_eq!(abs_path(root, "/tmp/a.sc"), "/tmp/a.sc");
        assert_eq!(abs_path(root, ""), "");
        let a = resolve_paths(root, "batch", &json!({"commands":[{"command":"file.save_as","params":{"path":"mech/b.sc"}},{"command":"solid.box","params":{"length":1}}]}));
        assert_eq!(a["commands"][0]["params"]["path"], "/home/user/proj/mech/b.sc");
        assert_eq!(resolve_paths(root, "compare_picture", &json!({"reference":"ref.png"}))["reference"], "/home/user/proj/ref.png");
        assert_eq!(resolve_paths(root, "measure", &json!({"path":"x"}))["path"], "x", "only the tools that take files");
        assert_eq!(written_paths("export", &json!({"path":"/p/a.step"})), vec!["/p/a.step"]);
        assert_eq!(written_paths("execute", &json!({"command":"file.save_as","params":{"path":"/p/a.sc"}})), vec!["/p/a.sc"]);
        assert!(written_paths("execute", &json!({"command":"solid.box","params":{"path":"/p/a.sc"}})).is_empty());
    }

    fn measured() -> Value { json!({"bodies":[
        {"name":"Body1","closed":true,"bbox":{"min":[0.0,0.0,0.0],"max":[40.0,20.0,10.0]},"volume_mm3":7929.314165294229,"area_mm2":2772.986722862693,"mass_g":10.228815273229555,"density_g_cm3":1.29,"material":"Default","center_of_mass":[20.16693962006313,10.077794665994903,5.0],"kernel":{"faces":8}},
        {"name":"Lid","closed":false,"bbox":{"min":[-5.0,0.0,10.0],"max":[45.0,20.0,12.0]}}],
        "body_count":2,"total":{"volume_mm3":7929.314165294229,"area_mm2":2772.986722862693,"mass_g":10.228815273229555}}) }
    fn inspected() -> Value { json!({"path":"/p/mech/a.sc","name":"a","dirty":true,"units":"mm",
        "params":[{"name":"w","expression":"40","value":40.0,"unit":"mm","comment":"","error":null}],
        "timeline":[{"id":1,"name":"Base","type":"BoxFeature"},{"id":2,"name":"Round","type":"FilletFeature","error":"no edges"}]}) }

    #[test]
    fn the_digest_says_what_an_edit_left() {
        let d = digest(Some(&inspected()), Some(&measured()));
        assert_eq!(d["file"], "/p/mech/a.sc"); assert_eq!(d["unsaved"], true); assert_eq!(d["features"], 2); assert_eq!(d["bodies"], 2);
        assert_eq!(d["volume_mm3"], 7929.314); assert_eq!(d["bbox"]["size"], json!([50.0, 20.0, 12.0])); assert_eq!(d["open_bodies"], 1);
        assert_eq!(d["failing"][0]["feature"], "Round");
        let hint = next_hint(1, &d, true);
        assert!(hint.contains("A feature fails") && hint.contains("solidcraft_look") && hint.contains("open"), "{hint}");
        assert_eq!(next_hint(0, &json!({}), true), "");
        assert!(next_hint(0, &json!({}), false).contains("no file yet"));
        assert_eq!(digest(None, None), json!({}));
    }

    #[test]
    fn the_summary_holds_parameters_and_mass_properties() {
        let s = summary(&inspected(), &measured());
        assert_eq!(s["parameters"][0], json!({"name":"w","expression":"40","value":40.0,"unit":"mm"}));
        assert_eq!(s["bodies"][0]["mass_g"], 10.2288); assert_eq!(s["bodies"][0]["center_of_mass"], json!([20.167, 10.078, 5.0]));
        assert!(s["bodies"][0].get("kernel").is_none(), "kernel counts are left out");
        assert_eq!(s["total"]["volume_mm3"], 7929.314); assert_eq!(s["features"].as_array().unwrap().len(), 2); assert_eq!(s["unsaved"], true);
    }

    #[test]
    fn listed_tools_are_prefixed_capped_and_never_the_apps_own() {
        let long = "x".repeat(5000);
        let d = listed(&json!({"name":"export","title":"Export","description":long,"inputSchema":{"type":"object"}})).unwrap();
        assert_eq!(d["name"], "solidcraft_export"); assert!(d["description"].as_str().unwrap().len() < DESC_MAX + 80); assert!(d.get("title").is_none());
        assert!(listed(&json!({"name":"quit_app"})).is_none());
        assert!(listed(&json!({"name":"gears.spur"})).is_none());
        assert_eq!(upstream("solidcraft_measure"), Some("measure")); assert_eq!(upstream("solidcraft_look"), None); assert_eq!(upstream("kicad_edit"), None);
    }

    #[test]
    fn which_projects_get_the_tools() {
        assert!(wanted(true, false, true, false), "a board project with a STEP file");
        assert!(wanted(false, true, true, false), "a mech folder");
        assert!(wanted(false, false, false, false), "a new, empty project");
        assert!(wanted(false, false, true, true), "SolidCraft is open on it");
        assert!(!wanted(false, false, true, false), "a board project with nothing mechanical");
    }

    #[test]
    fn old_checkpoints_go_first() {
        let ids: Vec<u64> = (1..=(KEEP as u64 + 3)).collect();
        assert_eq!(prune(&ids), vec![1, 2, 3]); assert!(prune(&ids[..KEEP]).is_empty());
    }
}
