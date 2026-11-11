//! A STEP file to a GLB, straight from the hub, no child. Meshing a B-rep is out of scope for this crate, so a host tool does it,
//! run as a subprocess (its own program, never a shell) and never linked. Which one is an explicit choice (`attempts`):
//!   * `BACKPLANE_STEP2GLB=<path>`: that `backplane-step2glb` only.
//!   * `BACKPLANE_STEP2GLB=off`: FreeCAD only.
//!   * unset: `backplane-step2glb` beside the hub, then FreeCAD only if it is absent or failed.
//! `backplane-step2glb` (native/step2glb) is OpenCascade compiled natively: it reads a STEP's colours through XCAF, heals and meshes its
//! solids on every core, and streams them (`convert_streaming`): a part in about 20 ms, a 9 MB board in about a second. FreeCAD is the
//! last resort, run as before (the GUI offscreen, the only FreeCAD build that reads a STEP's colours, then the console build, tints only):
//! its start alone takes four seconds.
//! Also the models cache: a converter writes in a folder of its own, and only a non-empty file that
//! converted is moved into the cache in one rename.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const FREECAD_GUI_PY: &str = include_str!("../files/scripts/freecad_gui.py");
const FREECAD_PY: &str = include_str!("../files/scripts/freecad_console.py");
const FREECAD_SECS: u64 = 600;
pub const NO_MESHER: &str = "a STEP needs backplane-step2glb beside the hub (or FreeCAD on the hub's machine)";
/// The longest a native conversion may take before it is stopped (a 9 MB board takes about a second).
const STEP2GLB_SECS: u64 = 600;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ran { pub code: i32, pub out: String }

/// Key.hash / Str.hash: FNV-1a of a text's code points
pub fn fnv(s: &str) -> u32 { s.chars().fold(2166136261u32, |h, c| (h ^ c as u32).wrapping_mul(16777619)) }

/// Pv.cache / Parts.cache: where converted models are kept, a native absolute path on every system (plat::dirs::models_cache: never `/tmp`
/// on Windows, where the hub and Git's shell read it as two different folders). Name a file in it with `cache_file`.
pub fn cache_dir() -> String { crate::plat::dirs::models_cache() }
/// `name` (a file or `sub/file`) in the models cache, with the system's separators.
pub fn cache_file(name: &str) -> String { Path::new(&cache_dir()).join(name).to_string_lossy().into_owned() }
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
/// ModelCache.publish: a finished converter's file into the cache, or why not: the path looked at and what the converter said
pub fn publish(r: Ran, from: &Path, out: &str) -> Ran {
    if r.code != 0 { return r }
    let said = |r: &Ran| { let t = r.out.trim(); if t.is_empty() { String::new() } else { format!("; the converter said: {}", t.chars().take(400).collect::<String>()) } };
    if !have(&from.to_string_lossy()) { return Ran { code: 1, out: format!("Could not cache 3D model: converter produced no model at {}{}", from.display(), said(&r)) } }
    match crate::plat::fs::rename(from, out) { Ok(()) => r, Err(e) => Ran { code: 1, out: format!("Could not cache 3D model: {} to {out}: {e}{}", from.display(), said(&r)) } }
}

/// How a STEP is meshed.
#[derive(Clone, Debug, PartialEq)]
pub enum Mesher { Step2glb(String), FreeCad { gui: String, cmd: String } }
/// Mech.detail.args as numbers (mm, radians); None is step2glb's own default (FreeCAD meshes at NORMAL then)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail { pub linear: f64, pub angular: f64, pub relative: bool, pub fine: Option<[f64; 6]> }
/// What every STEP is meshed at first: 0.2 rad, which OpenCascade meshes as 5.7 degrees a segment at most (a hole round in 63 or more:
/// counted by `a_cylinder_is_round_enough_at_first_and_rounder_zoomed_in`), and a linear deflection a 500th of each edge's size
/// (OpenCascade's relative mode), so a small hole is as round as a big one and a big flat face stays cheap. As many triangles as 0.03 mm
/// made before; the 16-gon a hole was drawn as came from the hub's grid (`solid::fit`), not from here.
pub const NORMAL: Detail = Detail { linear: 0.002, angular: 0.2, relative: true, fine: None };
/// What the solids in a zoomed view are meshed at (`refined`): 0.1 rad (2.9 degrees a segment, twice the segments of `NORMAL` on every
/// curve: 126 a hole) and a 1000th of each edge. Not finer: the wire carries corners to 10 um, and a 3 mm fillet meshed at 0.05 rad
/// (0.07 mm segments) shaded in ridges. Never coarser than `NORMAL` either: a floor on segment length (MinSize) made small parts coarser.
pub const FINE: (f64, f64) = (0.001, 0.1);
/// FreeCAD's scripts take millimetres, not a share of each edge: the deflection they get for a relative detail.
const FREECAD_MM: f64 = 0.03;
impl Detail {
    /// `NORMAL` with the solids that meet `box` (x0 y0 z0 x1 y1 z1, the model's millimetres) meshed at `FINE`.
    pub fn refined(b: [f64; 6]) -> Detail { Detail { fine: Some(b), ..NORMAL } }
    /// The FreeCAD scripts' environment: BP_TOL the linear deflection (mm), BP_ANG the angular (radians).
    pub fn env(self) -> [(&'static str, String); 2] { [("BP_TOL", (if self.relative { FREECAD_MM } else { self.linear }).to_string()), ("BP_ANG", self.angular.to_string())] }
    /// backplane-step2glb's arguments for this detail.
    pub fn args(self) -> Vec<String> {
        let mut a = vec!["--linear".to_string(), self.linear.to_string()];
        if self.relative { a.push("--relative".into()) }
        a.extend(["--angular".to_string(), self.angular.to_string()]);
        if let Some(b) = self.fine { a.extend(["--fine".to_string(), b.map(|v| format!("{v:.3}")).join(","), "--fine-linear".into(), FINE.0.to_string(), "--fine-angular".into(), FINE.1.to_string()]) }
        a
    }
}

/// The tools to try, in order, from what this machine has: `env` is BACKPLANE_STEP2GLB, `beside` a step2glb found by the hub.
pub fn attempts(env: Option<&str>, gui: &str, cmd: &str, beside: &str) -> Vec<Mesher> {
    let fc = || if gui.is_empty() && cmd.is_empty() { vec![] } else { vec![Mesher::FreeCad { gui: gui.into(), cmd: cmd.into() }] };
    match env {
        Some("off") | Some("0") => fc(),
        Some(p) if !p.is_empty() => vec![Mesher::Step2glb(p.into())],
        _ => { let mut v = if beside.is_empty() { vec![] } else { vec![Mesher::Step2glb(beside.into())] }; v.extend(fc()); v }
    }
}

fn runnable(p: &Path) -> bool { use crate::plat::fs::PermissionsExt; std::fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false) }
fn on_path(c: &str) -> Option<String> {
    // a path, not a name: with a separator (Windows: either) or a drive
    if c.contains(crate::plat::fs::SEPS) || crate::plat::fs::is_abs(c) { return runnable(Path::new(c)).then(|| c.to_string()) }
    // Windows: a program on PATH is `<name>.exe`
    let names = [c.to_string(), format!("{c}{}", std::env::consts::EXE_SUFFIX)];
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).flat_map(|d| names.iter().map(move |n| d.join(n)).collect::<Vec<_>>()).find(|f| runnable(f))).map(|f| f.to_string_lossy().into_owned())
}
/// Windows: FreeCAD's installer puts it in `<Program Files>\FreeCAD <version>\bin` (or the user's `%LOCALAPPDATA%\Programs`), not on PATH:
/// `exe` in each such folder, the newest version first.
pub fn windows_freecad(exe: &str) -> Vec<String> {
    if !cfg!(windows) { return vec![] }
    let mut roots: Vec<std::path::PathBuf> = ["ProgramFiles", "ProgramW6432"].iter().filter_map(|v| std::env::var_os(v)).map(Into::into).collect();
    if let Some(l) = std::env::var_os("LOCALAPPDATA") { roots.push(Path::new(&l).join("Programs")) }
    let mut found: Vec<std::path::PathBuf> = roots.iter().filter_map(|r| std::fs::read_dir(r).ok()).flatten().flatten()
        .filter(|e| e.file_name().to_string_lossy().to_lowercase().starts_with("freecad")).map(|e| e.path().join("bin").join(exe)).filter(|p| p.is_file()).collect();
    found.sort(); found.dedup(); found.reverse();
    found.into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
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
    cands.extend(windows_freecad("freecadcmd.exe"));
    cands.iter().filter(|c| !c.is_empty()).find_map(|c| on_path(c)).unwrap_or_default()
}
/// The FreeCAD an agent is told about: the one the hub would launch (a setting that does not run is skipped), else what the host found at start.
pub fn told_freecad(home: &Path, host_fc: &str) -> String { let c = freecad_cmd(home); if c.is_empty() { host_fc.to_string() } else { c } }
/// freecadGui: the GUI program beside its console one, else on PATH
pub fn freecad_gui(cmd: &str) -> String {
    let dir = cmd.rsplit_once(crate::plat::fs::SEPS).map(|d| d.0.to_string()).unwrap_or_default();
    let mut near: Vec<String> = if dir.is_empty() { vec![] } else { vec![format!("{dir}/freecad"), format!("{dir}/FreeCAD"), format!("{}/MacOS/FreeCAD", dir.trim_end_matches("/Resources/bin"))] };
    if cfg!(windows) && !dir.is_empty() { near.insert(0, format!("{dir}\\freecad.exe")) }
    near.extend(["freecad".to_string(), "FreeCAD".to_string()]);
    near.iter().find_map(|c| on_path(c)).unwrap_or_default()
}
/// Helper.find: backplane-step2glb beside the hub, then at the install's top, then on PATH
fn step2glb_beside() -> String {
    let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    let name = format!("backplane-step2glb{}", std::env::consts::EXE_SUFFIX);   // Windows: the .exe
    let near = exe.iter().flat_map(|d| [d.join(&name), d.join("..").join(&name)]).find(|p| runnable(p));
    near.map(|p| p.to_string_lossy().into_owned()).or_else(|| on_path("backplane-step2glb")).unwrap_or_default()
}
/// The tools on this machine, in order: what the two entry points use.
pub fn machine(home: &Path) -> Vec<Mesher> {
    let cmd = freecad_cmd(home); let gui = freecad_gui(&cmd);
    attempts(std::env::var("BACKPLANE_STEP2GLB").ok().as_deref(), &gui, &cmd, &step2glb_beside())
}

/// A tool run straight, no shell between (a shell reads a path its own way: Git's on Windows takes `/tmp` for %TEMP%, issue 1102), at a lower
/// priority when `nice`: `nice -n 10` on Unix, below-normal priority on Windows (it has no `nice`). Its folder is the root: every path given is absolute.
fn command(nice: bool, program: &str) -> Command {
    #[cfg(unix)] let mut c = { let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, if nice { "nice" } else { program }); if nice { c.args(["-n", "10", program]); } c };
    #[cfg(windows)] let mut c = { use std::os::windows::process::CommandExt; const BELOW_NORMAL: u32 = 0x0000_4000;
        let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, program); if nice { c.creation_flags(crate::plat::proc::NO_WINDOW | BELOW_NORMAL); } c };
    c.stdin(Stdio::null()).current_dir(std::path::MAIN_SEPARATOR_STR); c
}
/// a tool's exit code once it ends, or None when it had `secs` and did not (it is stopped then: its own PID, from this spawn)
fn wait(c: &mut Command, secs: u64) -> Option<i32> {
    let mut p = c.spawn().ok()?; let end = Instant::now() + Duration::from_secs(secs);
    loop {
        match p.try_wait() { Ok(Some(s)) => return Some(s.code().unwrap_or(1)), Ok(None) => {} Err(_) => return None }
        if Instant::now() >= end { let _ = p.kill(); let _ = p.wait(); return None }
        std::thread::sleep(Duration::from_millis(50));
    }
}
/// FreeCAD's GUI, offscreen, only reads and meshes: it gets no display and no GPU. With the session's DISPLAY and Qt's gtk3 theme it opened
/// the person's display and an OpenGL context on the GPU that draws their desktop, once a process (three convert at once); on 2026-11-09 an
/// NVIDIA driver ran out of address space as they did and stalled the whole desktop. Without them it meshes the same (Qt says it has no GL and
/// goes on). Unix only: the variables are X11's, Wayland's, Qt's, glvnd's and Mesa's; elsewhere nothing changes.
pub fn soft_gl(c: &mut Command) {
    #[cfg(all(unix, not(target_os = "macos")))] {
        c.env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY").env_remove("QT_QPA_PLATFORMTHEME");
        c.env("LIBGL_ALWAYS_SOFTWARE", "1").env("__GLX_VENDOR_LIBRARY_NAME", "mesa").env("QT_QUICK_BACKEND", "software");
        if let Some(f) = ["/usr/share/glvnd/egl_vendor.d/50_mesa.json", "/etc/glvnd/egl_vendor.d/50_mesa.json"].into_iter().find(|f| Path::new(f).is_file()) { c.env("__EGL_VENDOR_LIBRARY_FILENAMES", f); }
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))] { let _ = c; }
}
/// Every FreeCAD started here, GUI or console, a probe or a conversion: soft_gl's no-display, and Qt's offscreen platform. A FreeCAD that gets the
/// session's DISPLAY can put a window on the person's screen: `freecadcmd --version` that turned out to be the GUI showed its "FreeCAD 1.1.0
/// Revision…" box there, again at each check.
pub fn headless(c: &mut Command) { soft_gl(c); c.env("QT_QPA_PLATFORM", "offscreen"); }
/// FreeCAD's AppImage (the file, or `freecadcmd` linked to it) runs its GUI whatever name it is called by: its AppRun looks only at its first
/// argument, which picks a program of its own (`freecadcmd`) when it names one. True when `cmd` is such a file: its type-2 AppImage mark, `AI\x02`
/// at byte 8 of the ELF.
pub fn is_appimage(cmd: &str) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(cmd) else { return false };
    let mut b = [0u8; 11];
    f.read_exact(&mut b).is_ok() && &b[..4] == b"\x7fELF" && &b[8..11] == b"AI\x02"
}
/// The arguments that come before a console run's own: `freecadcmd` when `cmd` is the AppImage, so its console program runs and not the GUI.
pub fn console_args(cmd: &str) -> &'static [&'static str] { if is_appimage(cmd) { &["freecadcmd"] } else { &[] } }
/// viaFreecadGui: the file meshed by FreeCAD's GUI, offscreen, with its colours: its triangle count, 0 when it failed
fn via_gui(gui: &str, input: &str, out: &str, d: Detail, nice: bool) -> u64 {
    if gui.is_empty() { return 0 }
    let py = format!("{out}.gui.py");
    if std::fs::write(&py, FREECAD_GUI_PY).is_err() { return 0 }
    let mut c = command(nice, gui);
    headless(&mut c);
    c.arg(&py).envs([("BP_IN", input), ("BP_OUT", out)]).envs(d.env()).stdout(Stdio::null()).stderr(Stdio::null());
    wait(&mut c, FREECAD_SECS);
    let n = std::fs::read_to_string(format!("{out}.n")).unwrap_or_default().split(' ').next().and_then(|x| x.trim().parse::<u64>().ok()).unwrap_or(0);
    let _ = std::fs::remove_file(&py); let _ = std::fs::remove_file(format!("{out}.n"));
    if n > 0 && have(out) { n } else { 0 }
}
/// viaFreecad's console half: its triangle count, 0 when it failed
fn via_console(cmd: &str, input: &str, out: &str, d: Detail, nice: bool) -> u64 {
    if cmd.is_empty() { return 0 }
    let (py, log) = (format!("{out}.py"), format!("{out}.log"));
    let Ok(file) = std::fs::File::create(&log) else { return 0 };
    if std::fs::write(&py, FREECAD_PY).is_err() { return 0 }
    let mut c = command(nice, cmd);
    headless(&mut c);
    c.args(console_args(cmd)).args(["-c", "import os; exec(open(os.environ['BP_PY']).read())"]).envs([("BP_PY", py.as_str()), ("BP_IN", input), ("BP_OUT", out)]).envs(d.env()).stdout(file).stderr(Stdio::null());
    let code = wait(&mut c, FREECAD_SECS);
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&py); let _ = std::fs::remove_file(&log);
    let n = text.split("TRIS ").nth(1).and_then(|t| t.split_whitespace().next()).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    if code == Some(0) && n > 0 && have(out) { n } else { 0 }
}
/// a tool's stdout and stderr together (one pipe, in the order written), as Proc.run gives them; exit 127 when it cannot start. The tool is
/// started itself, with `argv` as given: the paths it writes are the ones the hub then reads.
pub fn run_merged(argv: &[&str], nice: bool) -> Ran {
    let Some((prog, args)) = argv.split_first() else { return Ran { code: 127, out: "no program".into() } };
    let started = (|| {
        let (mut r, w) = std::io::pipe()?;
        let mut c = command(nice, prog); c.args(args).stdout(w.try_clone()?).stderr(w);
        let mut p = c.spawn()?; drop(c); // the pipe's writing ends are the child's alone: its exit ends the read
        let mut out = Vec::new(); let _ = std::io::Read::read_to_end(&mut r, &mut out);
        Ok::<_, std::io::Error>((p.wait()?, out))
    })();
    match started { Ok((s, o)) => Ran { code: s.code().unwrap_or(1), out: String::from_utf8_lossy(&o).into_owned() }, Err(e) => Ran { code: 127, out: format!("{prog}: {e}") } }
}
/// What a streaming conversion says as it goes: how many leaves (placed solids) the model has, then each batch of them as a GLB of
/// its own (the same format as the whole), in the order they were meshed.
#[derive(Clone, Debug, PartialEq)]
pub enum Event { Of { leaves: u32, faces: u32 }, Part { tris: u64, leaves: u32, glb: PathBuf } }
/// One line of backplane-step2glb's stdout, read (`TIME`, `TRIS` and anything else: None).
pub fn event(line: &str) -> Option<Event> {
    let mut w = line.splitn(4, ' ');
    match w.next()? {
        "OF" => Some(Event::Of { leaves: w.next()?.parse().ok()?, faces: w.next()?.parse().ok()? }),
        "PART" => { let tris = w.next()?.parse().ok()?; let leaves = w.next()?.parse().ok()?; let glb = w.next()?.trim_end(); (!glb.is_empty()).then(|| Event::Part { tris, leaves, glb: glb.into() }) }
        _ => None,
    }
}
/// backplane-step2glb on `src` into `stage`, its stdout read line by line as it is written: each `Event` to `on` the moment it is said
/// (the batch's GLB is there until this returns), and all its output in the answer. Stopped after STEP2GLB_SECS.
fn step2glb_run(tool: &str, src: &str, stage: &str, detail: Option<Detail>, nice: bool, mut on: Option<&mut (dyn FnMut(Event) + '_)>) -> Ran {
    use std::io::BufRead;
    let parts = format!("{stage}.parts");
    let mut c = command(nice, tool); c.args([src, stage]);
    if let Some(d) = detail { c.args(d.args()); }
    if on.is_some() { if std::fs::create_dir_all(&parts).is_err() { on = None } else { c.args(["--parts", &parts]); } }
    let started = (|| {
        let (r, w) = std::io::pipe()?;
        c.stdout(w.try_clone()?).stderr(w);
        let p = std::sync::Arc::new(std::sync::Mutex::new(c.spawn()?));
        drop(c); // the pipe's writing ends are the child's alone: its exit ends the read
        // the guard: stops the tool at its time, or ends the moment the tool's output does (no polling: a part's 20 ms are not waited on)
        let (done, ended) = std::sync::mpsc::channel::<()>();
        let guard = { let p = p.clone(); std::thread::spawn(move || {
            if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = ended.recv_timeout(Duration::from_secs(STEP2GLB_SECS)) { let _ = p.lock().unwrap_or_else(|e| e.into_inner()).kill(); }
        }) };
        let mut out = String::new();
        for line in std::io::BufReader::new(r).lines() {
            let Ok(line) = line else { break };
            if let (Some(f), Some(e)) = (on.as_mut(), event(&line)) { f(e) }
            out.push_str(&line); out.push('\n');
        }
        let _ = done.send(()); let _ = guard.join();
        let s = p.lock().unwrap_or_else(|e| e.into_inner()).wait();
        Ok::<_, std::io::Error>((s?, out))
    })();
    let _ = std::fs::remove_dir_all(&parts);
    match started { Ok((s, o)) => Ran { code: s.code().unwrap_or(1), out: o }, Err(e) => Ran { code: 127, out: format!("{tool}: {e}") } }
}
/// `src` meshed into `stage` by the first tool that can, at `detail` (None: the tool's default).
pub fn convert(tools: &[Mesher], src: &str, stage: &str, detail: Option<Detail>, nice: bool) -> Ran { convert_streaming(tools, src, stage, detail, nice, None) }
/// `convert`, with the model's parts to `on` as they are meshed (backplane-step2glb streams; FreeCAD says nothing until the end). A tool
/// that fails after it streamed is followed by the next, whose model replaces what was streamed.
pub fn convert_streaming(tools: &[Mesher], src: &str, stage: &str, detail: Option<Detail>, nice: bool, mut on: Option<&mut dyn FnMut(Event)>) -> Ran {
    let mut last = Ran { code: 127, out: NO_MESHER.into() };
    for t in tools {
        match t {
            Mesher::FreeCad { gui, cmd } => {
                let d = detail.unwrap_or(NORMAL);
                if via_gui(gui, src, stage, d, nice) > 0 || via_console(cmd, src, stage, d, nice) > 0 { return Ran::default() }
                last = Ran { code: 1, out: format!("FreeCAD could not mesh {src}") };
            }
            Mesher::Step2glb(tool) => {
                let r = step2glb_run(tool, src, stage, detail, nice, on.as_deref_mut());
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
    #[test] fn an_appimage_freecadcmd_runs_its_console_program_headless() {
        let d = crate::plat::tmp::dir("fcai");
        let (ai, elf, sh) = (d.join("FreeCAD.AppImage"), d.join("freecadcmd"), d.join("wrapper"));
        std::fs::write(&ai, b"\x7fELF\x02\x01\x01\x00AI\x02rest").unwrap(); std::fs::write(&elf, b"\x7fELF\x02\x01\x01\x00\x00\x00\x00rest").unwrap(); std::fs::write(&sh, "#!/bin/sh\n").unwrap();
        assert_eq!(console_args(&ai.to_string_lossy()), ["freecadcmd"]); assert!(console_args(&elf.to_string_lossy()).is_empty() && console_args(&sh.to_string_lossy()).is_empty());
        assert!(console_args(&d.join("none").to_string_lossy()).is_empty());
        #[cfg(unix)] { let l = d.join("link"); std::os::unix::fs::symlink(&ai, &l).unwrap(); assert_eq!(console_args(&l.to_string_lossy()), ["freecadcmd"], "a link to the AppImage is the AppImage"); }
        let mut c = Command::new("true"); c.env("DISPLAY", ":0"); headless(&mut c);
        let env: Vec<(String, Option<String>)> = c.get_envs().map(|(k, v)| (k.to_string_lossy().into(), v.map(|v| v.to_string_lossy().into()))).collect();
        assert!(env.contains(&("QT_QPA_PLATFORM".into(), Some("offscreen".into()))), "{env:?}");
        #[cfg(all(unix, not(target_os = "macos")))] assert!(env.contains(&("DISPLAY".into(), None)), "no display: {env:?}");
    }
    #[test] fn the_freecad_path_setting_is_tried_first() {
        use crate::plat::fs::PermissionsExt;
        let d = crate::plat::tmp::dir("fcset");
        let f = d.join("mycad"); std::fs::write(&f, "#!/bin/sh\n").unwrap(); std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        set_freecad_setting(&format!(" {} ", f.display())); assert_eq!(freecad_cmd(&d), f.to_string_lossy());
        set_freecad_setting("/no/such/freecad"); assert_ne!(freecad_cmd(&d), "/no/such/freecad", "a path that does not run is skipped");
        set_freecad_setting(&f.display().to_string()); assert_eq!(told_freecad(&d, "/host/found"), f.to_string_lossy(), "an agent is told the configured path");
        set_freecad_setting("/no/such/freecad"); assert_ne!(told_freecad(&d, "/host/found"), "/no/such/freecad", "an agent is not told a path that does not run");
        set_freecad_setting("");
    }
    fn fc() -> Mesher { Mesher::FreeCad { gui: "/g".into(), cmd: "/c".into() } }
    #[test] fn the_mesher_is_an_explicit_choice() {
        let s = |p: &str| Mesher::Step2glb(p.into());
        assert_eq!(attempts(None, "/g", "/c", "/s"), vec![s("/s"), fc()]); // the native mesher first; FreeCAD only when it is absent or failed
        assert_eq!(attempts(None, "", "", "/s"), vec![s("/s")]);
        assert_eq!(attempts(None, "", "", ""), vec![]);
        assert_eq!(attempts(Some("/x/tool"), "/g", "/c", "/s"), vec![s("/x/tool")]); // named: that tool only
        assert_eq!(attempts(Some("off"), "/g", "/c", "/s"), vec![fc()]); // FreeCAD alone
        assert_eq!(attempts(Some("off"), "", "", "/s"), vec![]);
        assert_eq!(attempts(Some(""), "/g", "/c", "/s"), vec![s("/s"), fc()]);
    }
    #[test] fn a_detail_is_the_meshers_arguments_and_freecads_millimetres() {
        assert_eq!(NORMAL.args().join(" "), "--linear 0.002 --relative --angular 0.2");
        assert_eq!(Detail::refined([0.0, 1.0, 2.0, 3.5, 4.0, 5.25]).args().join(" "), "--linear 0.002 --relative --angular 0.2 --fine 0.000,1.000,2.000,3.500,4.000,5.250 --fine-linear 0.001 --fine-angular 0.1");
        assert_eq!(NORMAL.env(), [("BP_TOL", "0.03".to_string()), ("BP_ANG", "0.2".to_string())], "FreeCAD takes millimetres");
    }
    #[test] fn the_native_meshers_lines_are_read() {
        assert_eq!(event("OF 76 1348"), Some(Event::Of { leaves: 76, faces: 1348 }));
        assert_eq!(event("PART 20378 68 /c/my parts/0000.glb"), Some(Event::Part { tris: 20378, leaves: 68, glb: "/c/my parts/0000.glb".into() }), "a path with spaces is whole");
        for l in ["TIME read 12", "TRIS 9", "PART 1 2", "PART x 2 /a", "OF 1", ""] { assert_eq!(event(l), None, "{l}") }
    }
    #[cfg(unix)]
    #[test] fn a_stream_says_each_part_while_the_tool_runs_and_its_folder_goes_after() {
        use crate::plat::fs::PermissionsExt;
        let d = dir("stream"); let tool = d.join("t");
        // a stand-in: says its batches, each written before it is said, then the model
        std::fs::write(&tool, "#!/bin/sh
while [ $# -gt 0 ]; do [ \"$1\" = --parts ] && P=$2; shift; done
[ -n \"$P\" ] || exit 0
echo 'TIME read 1'
echo 'OF 2 9'
printf a > \"$P/0000.glb\"
echo \"PART 12 1 $P/0000.glb\"
printf b > \"$P/0001.glb\"
echo \"PART 30 1 $P/0001.glb\"
echo 'TRIS 42'
").unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        // "Text file busy": the script written a moment ago is still open for writing in a child another test thread just forked, until that child's exec (a few ms, longer on a loaded box).
        // Run it without --parts (it exits at once) until it starts.
        for _ in 0..400 { match std::process::Command::new(&tool).status() { Err(e) if e.raw_os_error() == Some(crate::plat::errno::ETXTBSY) => std::thread::sleep(std::time::Duration::from_millis(25)), _ => break } }
        let out = d.join("o.glb").to_string_lossy().into_owned();
        let mut seen = vec![];
        let mut on = |e: Event| { if let Event::Part { glb, .. } = &e { seen.push(std::fs::read_to_string(glb).unwrap_or_default()) } seen.push(format!("{e:?}").chars().take(4).collect()) };
        let r = convert_streaming(&[Mesher::Step2glb(tool.to_string_lossy().into())], "/a.step", &out, Some(NORMAL), false, Some(&mut on));
        assert_eq!(r.code, 0, "{}", r.out);
        assert_eq!(seen, ["Of {", "a", "Part", "b", "Part"], "each batch readable when it is said");
        assert!(r.out.contains("TRIS 42"));
        assert!(!Path::new(&format!("{out}.parts")).exists(), "the batches' folder goes once the tool is done");
    }
    #[test] fn no_tool_says_what_is_missing() { let r = convert(&[], "/a.step", "/o.glb", Some(NORMAL), false); assert_eq!((r.code, r.out.as_str()), (127, NO_MESHER)); }
    fn dir(tag: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("stepglb-{tag}")) }
    #[test] fn a_tool_gets_the_detail_and_the_next_is_tried_after_a_failure() {
        use crate::plat::fs::PermissionsExt;
        let d = dir("tool"); let tool = d.join("t"); let log = d.join("log");
        std::fs::write(&tool, format!("#!/bin/sh\necho \"$@\" > {}\nprintf x > \"$2\"\necho said\n", crate::plat::fs::script_path(&log))).unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        let out = d.join("o.glb"); let out = out.to_string_lossy();
        let t = tool.to_string_lossy();
        let r = convert(&[Mesher::Step2glb("/nonexistent/tool".into()), Mesher::Step2glb(t.to_string())], "/a.step", &out, Some(NORMAL), false);
        assert_eq!((r.code, r.out.trim()), (0, "said"));
        assert_eq!(std::fs::read_to_string(&log).unwrap().trim(), format!("/a.step {out} --linear 0.002 --relative --angular 0.2"));
        let r = convert(&[Mesher::Step2glb(t.to_string())], "/a.step", &out, None, true); assert_eq!(r.code, 0); // nice
        assert_eq!(std::fs::read_to_string(&log).unwrap().trim(), format!("/a.step {out}"));
    }
    #[test] fn a_converter_runs_itself_and_writes_where_the_hub_looks_spaces_and_all() {
        // no shell between: the output path the tool gets is the one `publish` checks (issue 1102: Git's sh read /tmp as %TEMP%)
        use crate::plat::fs::PermissionsExt;
        let base = dir("direct"); let d = base.join("a folder"); std::fs::create_dir_all(&d).unwrap(); let tool = d.join("my tool");
        std::fs::write(&tool, "#!/bin/sh\nprintf glb > \"$2\"\necho out\necho err >&2\n").unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        let stage = stage(&d.join("m.glb").to_string_lossy(), "export-").unwrap(); let from = stage.join("model.glb"); let out = d.join("cache dir").join("m.glb");
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        let r = convert(&[Mesher::Step2glb(tool.to_string_lossy().into())], &d.join("in put.step").to_string_lossy(), &from.to_string_lossy(), None, true);
        assert_eq!(r.code, 0, "{}", r.out); assert!(r.out.contains("out") && r.out.contains("err"), "stdout and stderr together: {:?}", r.out);
        assert_eq!(publish(r, &from, &out.to_string_lossy()).code, 0); assert_eq!(std::fs::read_to_string(&out).unwrap(), "glb");
        let r = run_merged(&[&d.join("no such tool").to_string_lossy()], false); assert_eq!(r.code, 127); assert!(r.out.contains("no such tool"), "{}", r.out);
    }
    #[test] fn a_converter_that_wrote_nothing_is_not_published() {
        let d = dir("pub"); let (from, out) = (d.join("m"), d.join("o.glb")); let o = out.to_string_lossy();
        assert_eq!(publish(Ran { code: 3, out: "no".into() }, &from, &o), Ran { code: 3, out: "no".into() });
        assert_eq!(publish(Ran::default(), &from, &o).code, 1); assert!(!out.exists());
        std::fs::write(&from, "").unwrap(); assert_eq!(publish(Ran::default(), &from, &o).out, format!("Could not cache 3D model: converter produced no model at {}", from.display())); assert!(!have(&o));
        let said = publish(Ran { code: 0, out: "wrote C:\\elsewhere\\m.glb\n".into() }, &from, &o).out;
        assert_eq!(said, format!("Could not cache 3D model: converter produced no model at {}; the converter said: wrote C:\\elsewhere\\m.glb", from.display()), "the path looked at and what the converter said");
        std::fs::write(&from, "x").unwrap(); assert_eq!(publish(Ran::default(), &from, &o), Ran::default()); assert!(have(&o) && !from.exists());
    }
    #[test] fn two_stages_of_one_model_are_two_folders() { let d = dir("st"); let o = d.join("o").to_string_lossy().into_owned(); assert_ne!(stage(&o, "export-").unwrap(), stage(&o, "export-").unwrap()); }
    #[cfg(all(unix, not(target_os = "macos")))]
    #[test] fn freecad_meshes_with_software_gl() {
        let mut c = Command::new("true"); soft_gl(&mut c);
        let env: std::collections::HashMap<_, _> = c.get_envs().map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned()))).collect();
        assert_eq!(env["LIBGL_ALWAYS_SOFTWARE"].as_deref(), Some("1")); assert_eq!(env["__GLX_VENDOR_LIBRARY_NAME"].as_deref(), Some("mesa"));
        for k in ["DISPLAY", "WAYLAND_DISPLAY", "QT_QPA_PLATFORMTHEME"] { assert_eq!(env[k], None, "{k}: removed, no display"); }
    }
    #[test] fn fnv_is_key_hash() { assert_eq!(fnv(""), 2166136261); assert_eq!(fnv("a"), 0xe40c292c); }
    #[test] fn mac_app_bundles_are_looked_for_system_wide_and_in_the_users_applications() {
        let v = mac_apps("FreeCAD.app/Contents/Resources/bin/freecadcmd"); assert_eq!(v[0], "/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd");
        let k = kicad_mac_clis(); assert!(k.iter().any(|p| p.ends_with("/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli")) && k.iter().any(|p| p.ends_with("/Applications/KiCad.app/Contents/MacOS/kicad-cli")));
    }
}
