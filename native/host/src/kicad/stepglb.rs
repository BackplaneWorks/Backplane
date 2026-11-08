//! A STEP file to a GLB, straight from the hub, no child. Meshing a B-rep is out of scope for this crate, so a host tool does it,
//! run as a subprocess and never linked. Which one is an explicit choice (`attempts`):
//!   * `BACKPLANE_STEP2GLB=<path>`: that `backplane-step2glb` only. OpenCascade in WASM on Bun's runtime: the JS hub's meshes exactly.
//!   * `BACKPLANE_STEP2GLB=off`: FreeCAD only: no Bun anywhere, and a machine without FreeCAD shows no STEP.
//!   * unset: FreeCAD (Python, with native OpenCascade), then `backplane-step2glb` beside the hub only if FreeCAD is absent or failed.
//! FreeCAD is run as step2glb itself runs it (tools/step2glb: the GUI offscreen, the only build that reads a STEP's colours, then the
//! console build, tints only); the two scripts are step2glb.ts's text, extracted by test/files/extract_scripts.py.
//! Also the models cache: a converter writes in a folder of its own, and only a non-empty file that
//! converted is moved into the cache in one rename.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const FREECAD_GUI_PY: &str = include_str!("../files/scripts/freecad_gui.py");
const FREECAD_PY: &str = include_str!("../files/scripts/freecad_console.py");
const FREECAD_SECS: u64 = 600;
pub const NO_MESHER: &str = "a STEP needs FreeCAD on the hub's machine (or backplane-step2glb beside the hub)";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ran { pub code: i32, pub out: String }

/// Key.hash / Str.hash: FNV-1a of a text's code points
pub fn fnv(s: &str) -> u32 { s.chars().fold(2166136261u32, |h, c| (h ^ c as u32).wrapping_mul(16777619)) }

/// Pv.cache / Parts.cache: where converted models are kept
pub fn cache_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    format!("{}/backplane-bend/models", std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{home}/.cache")))
}
/// ModelCache.have: a non-empty regular file (a zero-byte output must not become a hit)
pub fn have(path: &str) -> bool { std::fs::metadata(path).map(|m| m.is_file() && m.len() > 0).unwrap_or(false) }

/// a folder of this attempt's own beside `out`: two writers of one model never touch each other's bytes
pub fn stage(out: &str, tag: &str) -> std::io::Result<PathBuf> {
    use crate::plat::fs::DirBuilderExt;
    static N: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let dir = PathBuf::from(format!("{out}.{tag}{}-{}-{t}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
    Ok(dir)
}
/// ModelCache.publish: a finished converter's file into the cache, or why not
pub fn publish(r: Ran, from: &Path, out: &str) -> Ran {
    if r.code != 0 { return r }
    if !have(&from.to_string_lossy()) { return Ran { code: 1, out: "Could not cache 3D model: converter produced no model".into() } }
    match crate::plat::fs::rename(from, out) { Ok(()) => r, Err(e) => Ran { code: 1, out: format!("Could not cache 3D model: {e}") } }
}

/// How a STEP is meshed.
#[derive(Clone, Debug, PartialEq)]
pub enum Mesher { Step2glb(String), FreeCad { gui: String, cmd: String } }
/// Mech.detail.args as numbers (mm, radians); None is step2glb's own default, what `--render` asks
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail { pub linear: f64, pub angular: f64 }
pub const NORMAL: Detail = Detail { linear: 0.03, angular: 0.2 };

/// The tools to try, in order, from what this machine has: `env` is BACKPLANE_STEP2GLB, `beside` a step2glb found by the hub.
pub fn attempts(env: Option<&str>, gui: &str, cmd: &str, beside: &str) -> Vec<Mesher> {
    let fc = || if gui.is_empty() && cmd.is_empty() { vec![] } else { vec![Mesher::FreeCad { gui: gui.into(), cmd: cmd.into() }] };
    match env {
        Some("off") | Some("0") => fc(),
        Some(p) if !p.is_empty() => vec![Mesher::Step2glb(p.into())],
        _ => { let mut v = fc(); if !beside.is_empty() { v.push(Mesher::Step2glb(beside.into())) } v }
    }
}

fn runnable(p: &Path) -> bool { use crate::plat::fs::PermissionsExt; std::fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false) }
fn on_path(c: &str) -> Option<String> {
    if c.contains('/') { return runnable(Path::new(c)).then(|| c.to_string()) }
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(c)).find(|f| runnable(f))).map(|f| f.to_string_lossy().into_owned())
}
/// macOS app bundles: `rel` under /Applications and ~/Applications (a user-level install, no admin rights), in that order.
pub fn mac_apps(rel: &str) -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut v = vec![format!("/Applications/{rel}")];
    if !home.is_empty() { v.push(format!("{home}/Applications/{rel}")) }
    v
}
/// KiCad's command line inside its macOS bundle: the unified installer's KiCad/KiCad.app, or a bare KiCad.app.
pub fn kicad_mac_clis() -> Vec<String> { ["KiCad/KiCad.app/Contents/MacOS/kicad-cli", "KiCad.app/Contents/MacOS/kicad-cli"].iter().flat_map(|r| mac_apps(r)).collect() }
/// FreeCAD.find: the `freecad.path` setting, BACKPLANE_FREECAD, the managed install under the hub's home, then FreeCAD's command names on PATH, then macOS's app
/// The `freecad.path` setting (Settings > CAD), kept here by the watcher in ipc.rs so every launch of FreeCAD reads it without a hub in hand.
static FC_SETTING: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());
pub fn set_freecad_setting(v: &str) { *FC_SETTING.write().unwrap_or_else(|e| e.into_inner()) = v.trim().to_string() }
pub fn freecad_setting() -> String { FC_SETTING.read().unwrap_or_else(|e| e.into_inner()).clone() }
pub fn freecad_cmd(home: &Path) -> String {
    let managed = std::fs::read_to_string(home.join("freecad/command")).ok().and_then(|t| t.lines().next().map(str::to_string)).unwrap_or_default();
    let env = std::env::var("BACKPLANE_FREECAD").unwrap_or_default();
    let mut cands: Vec<String> = vec![freecad_setting(), env, managed, "freecadcmd".into(), "FreeCADCmd".into(), "freecad.cmd".into()];
    cands.extend(mac_apps("FreeCAD.app/Contents/Resources/bin/freecadcmd"));
    cands.iter().filter(|c| !c.is_empty()).find_map(|c| on_path(c)).unwrap_or_default()
}
/// The FreeCAD an agent is told about: the one the hub would launch (a setting that does not run is skipped), else what the host found at start.
pub fn told_freecad(home: &Path, host_fc: &str) -> String { let c = freecad_cmd(home); if c.is_empty() { host_fc.to_string() } else { c } }
/// freecadGui: the GUI program beside its console one, else on PATH
pub fn freecad_gui(cmd: &str) -> String {
    let dir = cmd.rsplit_once('/').map(|d| d.0.to_string()).unwrap_or_default();
    let mut near: Vec<String> = if dir.is_empty() { vec![] } else { vec![format!("{dir}/freecad"), format!("{dir}/FreeCAD"), format!("{}/MacOS/FreeCAD", dir.trim_end_matches("/Resources/bin"))] };
    near.extend(["freecad".to_string(), "FreeCAD".to_string()]);
    near.iter().find_map(|c| on_path(c)).unwrap_or_default()
}
/// Helper.find: backplane-step2glb beside the hub, then at the install's top, then on PATH
fn step2glb_beside() -> String {
    let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    let near = exe.iter().flat_map(|d| [d.join("backplane-step2glb"), d.join("../backplane-step2glb")]).find(|p| runnable(p));
    near.map(|p| p.to_string_lossy().into_owned()).or_else(|| on_path("backplane-step2glb")).unwrap_or_default()
}
/// The tools on this machine, in order: what the two entry points use.
pub fn machine(home: &Path) -> Vec<Mesher> {
    let cmd = freecad_cmd(home); let gui = freecad_gui(&cmd);
    attempts(std::env::var("BACKPLANE_STEP2GLB").ok().as_deref(), &gui, &cmd, &step2glb_beside())
}

fn low(c: &mut Command, nice: bool, program: &str) { if nice { c.args(["-n", "10", program]); } }
fn command(nice: bool, program: &str) -> Command { let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, if nice { "nice" } else { program }); low(&mut c, nice, program); c.stdin(Stdio::null()).current_dir("/"); c }
/// a tool's exit code once it ends, or None when it had `secs` and did not (it is stopped then: its own PID, from this spawn)
fn wait(c: &mut Command, secs: u64) -> Option<i32> {
    let mut p = c.spawn().ok()?; let end = Instant::now() + Duration::from_secs(secs);
    loop {
        match p.try_wait() { Ok(Some(s)) => return Some(s.code().unwrap_or(1)), Ok(None) => {} Err(_) => return None }
        if Instant::now() >= end { let _ = p.kill(); let _ = p.wait(); return None }
        std::thread::sleep(Duration::from_millis(50));
    }
}
/// viaFreecadGui: the file meshed by FreeCAD's GUI, offscreen, with its colours: its triangle count, 0 when it failed
fn via_gui(gui: &str, input: &str, out: &str, tol: &str, nice: bool) -> u64 {
    if gui.is_empty() { return 0 }
    let py = format!("{out}.gui.py");
    if std::fs::write(&py, FREECAD_GUI_PY).is_err() { return 0 }
    let mut c = command(nice, gui);
    c.arg(&py).envs([("QT_QPA_PLATFORM", "offscreen"), ("BP_IN", input), ("BP_OUT", out), ("BP_TOL", tol)]).stdout(Stdio::null()).stderr(Stdio::null());
    wait(&mut c, FREECAD_SECS);
    let n = std::fs::read_to_string(format!("{out}.n")).unwrap_or_default().split(' ').next().and_then(|x| x.trim().parse::<u64>().ok()).unwrap_or(0);
    let _ = std::fs::remove_file(&py); let _ = std::fs::remove_file(format!("{out}.n"));
    if n > 0 && have(out) { n } else { 0 }
}
/// viaFreecad's console half: its triangle count, 0 when it failed
fn via_console(cmd: &str, input: &str, out: &str, tol: &str, nice: bool) -> u64 {
    if cmd.is_empty() { return 0 }
    let (py, log) = (format!("{out}.py"), format!("{out}.log"));
    let Ok(file) = std::fs::File::create(&log) else { return 0 };
    if std::fs::write(&py, FREECAD_PY).is_err() { return 0 }
    let mut c = command(nice, cmd);
    c.args(["-c", "import os; exec(open(os.environ['BP_PY']).read())"]).envs([("BP_PY", py.as_str()), ("BP_IN", input), ("BP_OUT", out), ("BP_TOL", tol)]).stdout(file).stderr(Stdio::null());
    let code = wait(&mut c, FREECAD_SECS);
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&py); let _ = std::fs::remove_file(&log);
    let n = text.split("TRIS ").nth(1).and_then(|t| t.split_whitespace().next()).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    if code == Some(0) && n > 0 && have(out) { n } else { 0 }
}
/// a tool's stdout and stderr together, as Proc.run gives them; exit 127 when it cannot start
pub fn run_merged(argv: &[&str], nice: bool) -> Ran {
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, "sh");
    c.args(["-c", if nice { "exec nice -n 10 \"$@\" 2>&1" } else { "exec \"$@\" 2>&1" }, "sh"]).args(argv).stdin(Stdio::null()).current_dir("/");
    match c.output() { Ok(o) => Ran { code: o.status.code().unwrap_or(1), out: String::from_utf8_lossy(&o.stdout).into_owned() }, Err(e) => Ran { code: 127, out: e.to_string() } }
}
/// `src` meshed into `stage` by the first tool that can, at `detail` (None: the tool's default).
pub fn convert(tools: &[Mesher], src: &str, stage: &str, detail: Option<Detail>, nice: bool) -> Ran {
    let mut last = Ran { code: 127, out: NO_MESHER.into() };
    for t in tools {
        match t {
            Mesher::FreeCad { gui, cmd } => {
                let tol = detail.map_or(0.1, |d| d.linear).max(0.25).to_string();
                if via_gui(gui, src, stage, &tol, nice) > 0 || via_console(cmd, src, stage, &tol, nice) > 0 { return Ran::default() }
                last = Ran { code: 1, out: format!("FreeCAD could not mesh {src}") };
            }
            Mesher::Step2glb(tool) => {
                let (l, a) = detail.map_or((String::new(), String::new()), |d| (d.linear.to_string(), d.angular.to_string()));
                let mut argv = vec![tool.as_str(), src, stage];
                if detail.is_some() { argv.extend(["--linear", l.as_str(), "--angular", a.as_str()]) }
                let r = run_merged(&argv, nice);
                if r.code == 0 { return r }
                last = r;
            }
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_freecad_path_setting_is_tried_first() {
        use crate::plat::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("bp-fcset-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap();
        let f = d.join("mycad"); std::fs::write(&f, "#!/bin/sh\n").unwrap(); std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        set_freecad_setting(&format!(" {} ", f.display())); assert_eq!(freecad_cmd(&d), f.to_string_lossy());
        set_freecad_setting("/no/such/freecad"); assert_ne!(freecad_cmd(&d), "/no/such/freecad", "a path that does not run is skipped");
        set_freecad_setting(&f.display().to_string()); assert_eq!(told_freecad(&d, "/host/found"), f.to_string_lossy(), "an agent is told the configured path");
        set_freecad_setting("/no/such/freecad"); assert_ne!(told_freecad(&d, "/host/found"), "/no/such/freecad", "an agent is not told a path that does not run");
        set_freecad_setting(""); let _ = std::fs::remove_dir_all(&d);
    }
    fn fc() -> Mesher { Mesher::FreeCad { gui: "/g".into(), cmd: "/c".into() } }
    #[test] fn the_mesher_is_an_explicit_choice() {
        let s = |p: &str| Mesher::Step2glb(p.into());
        assert_eq!(attempts(None, "/g", "/c", "/s"), vec![fc(), s("/s")]); // FreeCAD first; step2glb only when FreeCAD is absent or failed
        assert_eq!(attempts(None, "", "", "/s"), vec![s("/s")]);
        assert_eq!(attempts(None, "", "", ""), vec![]);
        assert_eq!(attempts(Some("/x/tool"), "/g", "/c", "/s"), vec![s("/x/tool")]); // named: that tool only
        assert_eq!(attempts(Some("off"), "/g", "/c", "/s"), vec![fc()]); // no Bun at all
        assert_eq!(attempts(Some("off"), "", "", "/s"), vec![]);
        assert_eq!(attempts(Some(""), "/g", "/c", "/s"), vec![fc(), s("/s")]);
    }
    #[test] fn no_tool_says_what_is_missing() { let r = convert(&[], "/a.step", "/o.glb", Some(NORMAL), false); assert_eq!((r.code, r.out.as_str()), (127, NO_MESHER)); }
    fn dir(tag: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-stepglb-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
    #[test] fn a_tool_gets_the_detail_and_the_next_is_tried_after_a_failure() {
        use crate::plat::fs::PermissionsExt;
        let d = dir("tool"); let tool = d.join("t"); let log = d.join("log");
        std::fs::write(&tool, format!("#!/bin/sh\necho \"$@\" > {}\nprintf x > \"$2\"\necho said\n", crate::plat::fs::script_path(&log))).unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        let out = d.join("o.glb"); let out = out.to_string_lossy();
        let t = tool.to_string_lossy();
        let r = convert(&[Mesher::Step2glb("/nonexistent/tool".into()), Mesher::Step2glb(t.to_string())], "/a.step", &out, Some(NORMAL), false);
        assert_eq!((r.code, r.out.trim()), (0, "said"));
        assert_eq!(std::fs::read_to_string(&log).unwrap().trim(), format!("/a.step {out} --linear 0.03 --angular 0.2"));
        let r = convert(&[Mesher::Step2glb(t.to_string())], "/a.step", &out, None, true); assert_eq!(r.code, 0); // nice
        assert_eq!(std::fs::read_to_string(&log).unwrap().trim(), format!("/a.step {out}"));
    }
    #[test] fn a_converter_that_wrote_nothing_is_not_published() {
        let d = dir("pub"); let (from, out) = (d.join("m"), d.join("o.glb")); let o = out.to_string_lossy();
        assert_eq!(publish(Ran { code: 3, out: "no".into() }, &from, &o), Ran { code: 3, out: "no".into() });
        assert_eq!(publish(Ran::default(), &from, &o).code, 1); assert!(!out.exists());
        std::fs::write(&from, "").unwrap(); assert_eq!(publish(Ran::default(), &from, &o).out, "Could not cache 3D model: converter produced no model"); assert!(!have(&o));
        std::fs::write(&from, "x").unwrap(); assert_eq!(publish(Ran::default(), &from, &o), Ran::default()); assert!(have(&o) && !from.exists());
    }
    #[test] fn two_stages_of_one_model_are_two_folders() { let d = dir("st"); let o = d.join("o").to_string_lossy().into_owned(); assert_ne!(stage(&o, "export-").unwrap(), stage(&o, "export-").unwrap()); }
    #[test] fn fnv_is_key_hash() { assert_eq!(fnv(""), 2166136261); assert_eq!(fnv("a"), 0xe40c292c); }
    #[test] fn mac_app_bundles_are_looked_for_system_wide_and_in_the_users_applications() {
        let v = mac_apps("FreeCAD.app/Contents/Resources/bin/freecadcmd"); assert_eq!(v[0], "/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd");
        let k = kicad_mac_clis(); assert!(k.iter().any(|p| p.ends_with("/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli")) && k.iter().any(|p| p.ends_with("/Applications/KiCad.app/Contents/MacOS/kicad-cli")));
    }
}
