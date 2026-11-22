//! #1185: a narrow repair of window geometry an embedded Backplane KiCad saved into the person's own KiCad settings
//! (`<config>/kicad/10.99/pcbnew.json` and the other embedded editors'): side panels of 1.2e9 px and a 2e9 x 32245 px window at
//! x = -1.1e9 on Windows (bp-win's own rc.7 embed runs wrote the same: panels of ±1.5e9 px, the design-blocks panel 32765 px tall, a
//! display of 4294967295, Windows' minimized place -32000), the park position (-30000, -30000) on Linux. The person's standalone KiCad
//! then opens broken.
//!
//! The fork stops writing them (JSON_SETTINGS::SetKeepWindowGeometry, bp after 13.2); this undoes what an older runtime already wrote,
//! before an embedded KiCad starts and before Open in KiCad. Hub probes never write the person's settings (a63f0eff0), so this is held
//! to damage we caused: only the settings folder of our own runtime's version (10.99, never a stock KiCad's), only while no KiCad
//! process of any kind runs (one would write its own copy back when it closes), only the entries that are out of range, and nothing is
//! written when there are none. A file that does not parse is left alone. Each file is backed up once, before its first repair, and that
//! backup is kept. Every other key, its order and its numbers' text stay as they were (serde_json keeps order and number text here).
//!
//! What the person sees afterwards in stock KiCad: the window opens at KiCad's default size and place where its position or size was
//! dropped; a side panel whose width was dropped opens at its default width; an AUI layout keeps every pane, with only a dock or
//! floating size that was out of range back at its default; a library chooser or the design-blocks panel whose size was dropped opens at
//! its default; a window whose display was dropped opens on the first. Panels shown or hidden, grid, zoom, recent files are unchanged.
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// The settings files of the frames KiCad embeds (BACKPLANE_EMBED::KindOf; the simulator's window is in eeschema.json), plus
/// the project manager (`kicad.json`) and CvPcb (`cvpcb.json`).
pub const FILES: [&str; 8] = ["pcbnew.json", "eeschema.json", "symbol_editor.json", "fpedit.json", "3d_viewer.json", "gerbview.json", "kicad.json", "cvpcb.json"];

/// No screen layout puts a window, or a panel's edge, this far out (KiCad parks embedded frames at -30000; 4 monitors of 4K are 15360).
const LIMIT: f64 = 20_000.0;

/// KiCad's programs: while any of them runs (ours or the person's, any version), no file is touched.
const KICAD_PROGRAMS: [&str; 10] = ["kicad", "kicad.real", "kicad-cli", "kicad-cli.real", "pcbnew", "eeschema", "gerbview", "pl_editor", "pcb_calculator", "bitmap2component"];

fn big(v: &Value) -> bool { v.as_f64().is_some_and(|f| !f.is_finite() || f.abs() >= LIMIT) || (v.is_number() && v.as_f64().is_none()) }
fn any_big(v: &Value) -> bool {
    match v { Value::Object(o) => o.values().any(any_big), Value::Array(a) => a.iter().any(any_big), v => big(v) }
}
/// Sizes and widths: `-1` (KiCad's unset sentinel) and `0` (untouched sub-windows/toolbars) are kept; anything `< -1` or `>= LIMIT` is corrupt.
fn bad_size(v: &Value) -> bool { v.as_f64().is_some_and(|f| !f.is_finite() || f < -1.0 || f >= LIMIT) || (v.is_number() && v.as_f64().is_none()) }
fn any_bad_size(v: &Value) -> bool {
    match v { Value::Object(o) => o.values().any(any_bad_size), Value::Array(a) => a.iter().any(any_bad_size), v => bad_size(v) }
}
fn bad_display(v: &Value) -> bool {
    v.as_i64().is_some_and(|d| d < 0 || d >= 64)
        || v.as_u64().is_some_and(|d| d >= 64)
        || v.as_f64().is_some_and(|f| !f.is_finite() || f < 0.0 || f >= 64.0)
        || (v.is_number() && v.as_f64().is_none())
}
fn bad_rect(v: &Value) -> bool {
    match v {
        Value::Object(o) => o.iter().any(|(k, val)| match k.as_str() { "width" | "height" | "w" | "h" => any_bad_size(val), _ => any_big(val) }),
        _ => any_big(v),
    }
}

/// Remove, in place, each out-of-range member of an aui_state (wx 3.3's JSON layout): a dock's pixel `size`, a floating pane's `rect`.
/// A dock's `proportion` is a weight (often 100000) and `index`, `layer`, `row`, `direction`, `position` are counts: never touched.
fn repair_aui_state(v: &mut Value, path: &str, gone: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            let bad: Vec<String> = o.iter().filter(|(k, v)| match k.as_str() { "size" => any_bad_size(v), "rect" => bad_rect(v), _ => false }).map(|(k, _)| k.clone()).collect();
            for k in bad { o.remove(&k); gone.push(format!("{path}.{k}")) }
            for (k, sub) in o.iter_mut() { repair_aui_state(sub, &format!("{path}.{k}"), gone) }
        }
        Value::Array(a) => for (i, sub) in a.iter_mut().enumerate() { repair_aui_state(sub, &format!("{path}[{i}]"), gone) },
        _ => {}
    }
}

/// wx 3.2's perspective string ("name=..;bestw=1239146100;prop=100000|dock_size(4,0,0)=788723092|"): the `dock_size(..)=N` entries and
/// pane size/float items (PERSPECTIVE_SIZES / PERSPECTIVE_POS) out of range are removed (wx then uses its default for each). None: nothing changed.
fn repair_perspective(s: &str) -> Option<(String, Vec<String>)> {
    let parse_num = |n: &str| {
        let digits = n.strip_prefix('-').unwrap_or(n);
        (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then(|| n.parse::<f64>().ok()).flatten()
    };
    let big_pos = |n: &str| parse_num(n).is_some_and(|f| !f.is_finite() || f.abs() >= LIMIT);
    let bad_sz = |n: &str| parse_num(n).is_some_and(|f| !f.is_finite() || f < -1.0 || f >= LIMIT);
    let mut gone = vec![];
    let parts: Vec<String> = s.split('|').filter_map(|part| {
        if part.starts_with("dock_size(") {
            let n = part.rsplit('=').next().unwrap_or("");
            if bad_sz(n) { gone.push(part.to_string()); return None }
            return Some(part.to_string())
        }
        let items: Vec<&str> = part.split(';').filter(|item| {
            let (k, n) = item.split_once('=').unwrap_or((item, ""));
            let drop = (PERSPECTIVE_SIZES.contains(&k) && bad_sz(n)) || (PERSPECTIVE_POS.contains(&k) && big_pos(n));
            if drop { gone.push(item.to_string()) }
            !drop
        }).collect();
        Some(items.join(";"))
    }).collect();
    (!gone.is_empty()).then(|| (parts.join("|"), gone))
}

/// A wx 3.2 perspective pane's pixel size items (`state` is a bit field, `prop` a weight, `dir`/`layer`/`row`/`pos` counts: never touched).
const PERSPECTIVE_SIZES: [&str; 8] = ["bestw", "besth", "minw", "minh", "maxw", "maxh", "floatw", "floath"];
/// A wx 3.2 perspective pane's floating position items (signed monitor coordinates).
const PERSPECTIVE_POS: [&str; 2] = ["floatx", "floaty"];

/// A library chooser's (`*_chooser`) pixel members.
const CHOOSER_SIZES: [&str; 6] = ["width", "height", "sash_h", "sash_v", "sash_pos_h", "sash_pos_v"];

fn aui_size_key(k: &str) -> bool { ["_width", "_height", "_size"].iter().any(|s| k.ends_with(s)) }
fn bad_proportion(v: &Value) -> bool {
    v.as_f64().is_some_and(|f| !f.is_finite() || (f != -1.0 && !(0.0..=1.0).contains(&f))) || (v.is_number() && v.as_f64().is_none())
}

/// Remove the out-of-range geometry in one settings document; returns what was removed ("window.pos_x", "aui.right_panel_width").
pub fn repair_doc(doc: &mut Value) -> Vec<String> {
    let mut gone = vec![];
    fn windows(o: &mut Map<String, Value>, path: &str, gone: &mut Vec<String>) {
        let at = |k: &str| if path.is_empty() { k.to_string() } else { format!("{path}.{k}") };
        let is_window = path == "window"
            || path.ends_with(".window")
            || ["pos_x", "pos_y", "size_x", "size_y", "aui_state", "perspective"].iter().any(|k| o.contains_key(*k));
        if is_window {
            for k in ["pos_x", "pos_y"] { if o.get(k).is_some_and(big) { o.remove(k); gone.push(at(k)) } }
            for k in ["size_x", "size_y"] { if o.get(k).is_some_and(bad_size) { o.remove(k); gone.push(at(k)) } }
            // a frame that is a child window has no display: wxNOT_FOUND saved as 4294967295 or -1 (no machine has 64 monitors)
            if o.get("display").is_some_and(bad_display) { o.remove("display"); gone.push(at("display")) }
            if let Some(st) = o.get_mut("aui_state") { repair_aui_state(st, &at("aui_state"), gone) }
            if let Some((fixed, what)) = o.get("perspective").and_then(Value::as_str).and_then(repair_perspective) {
                o.insert("perspective".into(), Value::String(fixed));
                gone.extend(what.into_iter().map(|w| format!("{} {w}", at("perspective"))));
            }
        }
        for (k, sub) in o.iter_mut() {
            // the fork's own copy of an embedded frame's geometry (bp after 13.2) is the fork's to keep in range
            if k == "embed" || k == "aui_state" { continue }
            if let Some(so) = sub.as_object_mut() { windows(so, &at(k), gone) }
        }
    }
    let Some(top) = doc.as_object_mut() else { return gone };
    windows(top, "", &mut gone);
    // the library choosers' sizes (design_block_chooser is also the design-blocks panel docked in the frame: 32765 px tall on bp-win)
    for (name, c) in top.iter_mut().filter(|(k, _)| k.ends_with("_chooser")) {
        let Some(c) = c.as_object_mut() else { continue };
        let bad: Vec<String> = c.iter().filter(|(k, v)| CHOOSER_SIZES.contains(&k.as_str()) && bad_size(v)).map(|(k, _)| k.clone()).collect();
        for k in bad { c.remove(&k); gone.push(format!("{name}.{k}")) }
    }
    if let Some(aui) = top.get_mut("aui").and_then(Value::as_object_mut) {
        let bad: Vec<String> = aui.iter().filter(|(k, v)| if k.ends_with("_pos") { any_big(v) } else if k.ends_with("_proportion") { bad_proportion(v) } else if aui_size_key(k) { any_bad_size(v) } else { false }).map(|(k, _)| k.clone()).collect();
        for k in bad { aui.remove(&k); gone.push(format!("aui.{k}")) }
    }
    gone
}

fn next_tmp_seq() -> u64 {
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Write `text` over `f` atomically (a temporary file beside it, renamed over it), with `f`'s permissions.
fn replace(f: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let perms = std::fs::metadata(f)?.permissions();
    let tmp = f.with_extension(format!("json.bp-tmp{}-{}", std::process::id(), next_tmp_seq()));
    let r = (|| {
        let mut out = std::fs::File::create(&tmp)?;
        out.write_all(text.as_bytes())?;
        out.sync_all()?;
        drop(out);
        std::fs::set_permissions(&tmp, perms)?;
        crate::plat::fs::rename(&tmp, f)
    })();
    if r.is_err() { let _ = std::fs::remove_file(&tmp); }
    r
}

struct RemoveOnDrop<'a>(&'a Path);
impl Drop for RemoveOnDrop<'_> {
    fn drop(&mut self) { let _ = std::fs::remove_file(self.0); }
}

/// Fallback when hard links are unavailable: serialize first-backup creation across processes with an exclusive lock file.
fn publish_backup_via_lock(tmp: &Path, backup: &Path) -> std::io::Result<()> {
    let lk = backup.with_extension("json.bp-bklk");
    for attempt in 0..20 {
        if backup.exists() {
            let _ = std::fs::remove_file(tmp);
            return Ok(());
        }
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&lk) {
            Ok(f) => {
                drop(f);
                let _guard = RemoveOnDrop(&lk);
                if !backup.exists() {
                    let r = crate::plat::fs::rename(tmp, backup);
                    if r.is_err() { let _ = std::fs::remove_file(tmp); }
                    return r;
                }
                let _ = std::fs::remove_file(tmp);
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && attempt < 19 => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => {
                let _ = std::fs::remove_file(tmp);
                return if backup.exists() { Ok(()) } else { Err(e) };
            }
        }
    }
    let _ = std::fs::remove_file(tmp);
    if backup.exists() { Ok(()) } else { Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "backup lock busy")) }
}

/// Write the first backup of `f` atomically via a synced temporary file so an interrupted write never leaves a partial backup.
fn write_backup_once(backup: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    if backup.exists() { return Ok(()) }
    std::fs::create_dir_all(backup.parent().unwrap_or(Path::new(".")))?;
    let tmp = backup.with_extension(format!("json.bp-tmp{}-{}", std::process::id(), next_tmp_seq()));
    let write_res = (|| {
        let mut out = std::fs::File::create(&tmp)?;
        out.write_all(text.as_bytes())?;
        out.sync_all()
    })();
    if let Err(e) = write_res {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    match std::fs::hard_link(&tmp, backup) {
        Ok(()) => { let _ = std::fs::remove_file(&tmp); Ok(()) }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => { let _ = std::fs::remove_file(&tmp); Ok(()) }
        Err(_) => publish_backup_via_lock(&tmp, backup),
    }
}

/// Repair every embedded editor's settings file in `dir` (one version's folder, `<config>/kicad/10.99`). Before a file's first repair it
/// is copied to `backups/<file>` (kept; a later repair finds it there and leaves it). Returns one line per file changed or refused.
#[cfg(test)]
pub fn repair(dir: &Path, backups: &Path) -> Vec<String> { repair_when(dir, backups, || true) }

/// `repair`, asking `may_write` before backing up and before replacing each file that needs a repair whether it may write at all.
pub fn repair_when(dir: &Path, backups: &Path, mut may_write: impl FnMut() -> bool) -> Vec<String> {
    let mut said = vec![];
    for name in FILES {
        let f = dir.join(name);
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let Some(mut doc) = serde_json::from_str::<Value>(&text).ok().filter(Value::is_object) else { continue };
        let gone = repair_doc(&mut doc);
        if gone.is_empty() { continue }
        if !may_write() { return said }
        let backup = backups.join(name);
        if let Err(e) = write_backup_once(&backup, &text) {
            said.push(format!("{}: not repaired, its backup failed: {e}", f.display())); continue
        }
        // KiCad writes 2-space JSON ending in a newline; so do we
        let mut out = serde_json::to_string_pretty(&doc).unwrap_or_default();
        if text.ends_with('\n') { out.push('\n') }
        // read again just before replacing: a file KiCad saved meanwhile is looked at next time
        if std::fs::read_to_string(&f).ok().as_deref() != Some(text.as_str()) { continue }
        if !may_write() { return said }
        if let Err(e) = replace(&f, &out) { said.push(format!("{}: not repaired, could not write: {e}", f.display())); continue }
        said.push(format!("{}: dropped {} (backup {})", f.display(), gone.join(", "), backup.display()));
    }
    said
}

/// Every process's program name (crate::plat::proc::PROCESS_LIST through the process gate), or None when they cannot be listed.
fn process_names() -> Option<Vec<String>> {
    let (prog, args) = crate::plat::proc::PROCESS_LIST;
    let out = crate::proc::std_cmd(crate::proc::Kind::Local, prog).args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
    out.status.success().then(|| crate::plat::proc::image_names(&String::from_utf8_lossy(&out.stdout)))
}

/// Whether any KiCad program runs, of any user (None when the processes cannot be listed: then nothing is repaired either).
fn kicad_running(names: &[String]) -> bool { names.iter().any(|n| KICAD_PROGRAMS.contains(&n.as_str())) }

/// The folder to repair for runtime `version` under the settings root `config_root`: only a Backplane runtime's (10.99).
pub fn dir_for(version: &str, config_root: &Path) -> Option<PathBuf> {
    (super::libtables::backplane_version(version)).then(|| super::libtables::settings_version(version)).flatten().map(|v| config_root.join(v))
}

static LAUNCH_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ACTIVE_LAUNCHES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static SPAWNING_LAUNCHES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Held across pre-launch preparation (`ACTIVE_LAUNCHES`) and child spawn (`SPAWNING_LAUNCHES`) so exit repairs cannot overlap spawns while overlapping fork launches can still repair cleanly under `guard_lock()`.
pub struct LaunchPermit {
    spawning: bool,
}
impl LaunchPermit {
    fn mark_spawning(&mut self) {
        if !self.spawning {
            self.spawning = true;
            SPAWNING_LAUNCHES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}
impl Drop for LaunchPermit {
    fn drop(&mut self) {
        if self.spawning {
            SPAWNING_LAUNCHES.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
        ACTIVE_LAUNCHES.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Monotonic launch epoch captured before scheduling a deferred exit-time repair.
pub fn epoch() -> u64 { LAUNCH_EPOCH.load(std::sync::atomic::Ordering::SeqCst) }

/// Begin a KiCad launch: advances the launch epoch (cancelling queued/in-progress exit repairs) and holds an active launch permit until dropped.
pub fn begin_launch() -> LaunchPermit {
    LAUNCH_EPOCH.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    ACTIVE_LAUNCHES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    LaunchPermit { spawning: false }
}

/// Wait for any in-progress repair holding `guard_lock()` to finish, then mark `permit` as entering child spawn before releasing the lock.
pub fn fence_deferred(permit: &mut LaunchPermit) {
    let _lock = guard_lock().lock().unwrap_or_else(|e| e.into_inner());
    permit.mark_spawning();
}

fn guard_lock() -> &'static std::sync::Mutex<()> {
    static L: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    L.get_or_init(|| std::sync::Mutex::new(()))
}

/// Run pre-launch geometry repair while no launch has entered spawn yet, then transition `permit` to spawning before releasing `guard_lock()`.
pub fn guard_for_launch(permit: &mut LaunchPermit, version: &str, hub_home: &Path) {
    use std::sync::atomic::Ordering::SeqCst;
    let _lock = guard_lock().lock().unwrap_or_else(|e| e.into_inner());
    if SPAWNING_LAUNCHES.load(SeqCst) == 0 {
        guard_locked(version, hub_home, || SPAWNING_LAUNCHES.load(SeqCst) == 0);
    }
    permit.mark_spawning();
}

/// Exit-time repair that runs only while the launch epoch matches `expected_epoch` and no launch permit is active or spawning.
pub fn guard_if_epoch(expected_epoch: u64, version: &str, hub_home: &Path) {
    use std::sync::atomic::Ordering::SeqCst;
    let still_idle = || epoch() == expected_epoch && ACTIVE_LAUNCHES.load(SeqCst) == 0 && SPAWNING_LAUNCHES.load(SeqCst) == 0;
    if !still_idle() { return }
    let _lock = guard_lock().lock().unwrap_or_else(|e| e.into_inner());
    if !still_idle() { return }
    guard_locked(version, hub_home, still_idle);
}

fn guard_locked(version: &str, hub_home: &Path, mut still_allowed: impl FnMut() -> bool) {
    if crate::copy::on() { return }
    let Some(dir) = super::libtables::config_root().and_then(|root| dir_for(version, &root)) else { return };
    #[cfg(test)]
    {
        let base = crate::plat::tmp::base();
        let dir_c = crate::plat::fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
        let base_c = crate::plat::fs::canonicalize(&base).unwrap_or(base);
        if !dir_c.starts_with(&base_c) { return }
    }
    let Some(ver) = dir.file_name().map(|v| v.to_string_lossy().to_string()) else { return };
    // the process list is read right before writing each dirty file (a clean folder pays nothing, and a new launch cancels the rest)
    let idle = || {
        if !still_allowed() { return false }
        match process_names() {
            Some(names) if !kicad_running(&names) => true,
            Some(_) => { eprintln!("kicad: window geometry (#1185): {} needs a repair; left for the next start, a KiCad is running", dir.display()); false }
            None => false,
        }
    };
    for line in repair_when(&dir, &hub_home.join("backups").join("kicad-geometry").join(ver), idle) { eprintln!("kicad: window geometry (#1185): {line}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This desktop's `~/.config/kicad/10.99/eeschema.json`, 2026-11-18: an embedded session's park position, saved as the person's.
    const LINUX_PARKED: &str = include_str!("geomguard_fixtures/linux-eeschema.json");
    /// #1185's pcbnew.json (the person's Windows machine), its geometry as reported, the rest from a stock 10.99 file.
    const WINDOWS_PCBNEW: &str = include_str!("geomguard_fixtures/windows-pcbnew.json");
    /// #1185's eeschema.json likewise.
    const WINDOWS_EESCHEMA: &str = include_str!("geomguard_fixtures/windows-eeschema.json");

    /// bp-win's own embed runs of rc.7 (2026-11-17, C:\bp\runs\b14b-rc7-libs and -keys): real files an embedded KiCad wrote on Windows.
    const BPWIN_PCBNEW: &str = include_str!("geomguard_fixtures/bpwin-rc7-pcbnew.json");
    const BPWIN_EESCHEMA: &str = include_str!("geomguard_fixtures/bpwin-rc7-eeschema.json");
    /// The same machine's other run: the frame saved at Windows' minimized-window place (-32000, -32000).
    const BPWIN_MINIMIZED: &str = include_str!("geomguard_fixtures/bpwin-rc7-minimized-pcbnew.json");

    fn doc(t: &str) -> Value { serde_json::from_str(t).unwrap() }

    #[test]
    fn the_windows_damage_is_dropped_and_nothing_else() {
        let mut d = doc(WINDOWS_PCBNEW);
        let gone = repair_doc(&mut d);
        assert_eq!(gone, ["window.pos_x", "window.size_x", "window.size_y", "window.aui_state.panes[0].dock.size", "aui.properties_panel_width", "aui.right_panel_width"], "{gone:?}");
        let mut want = doc(WINDOWS_PCBNEW);
        for k in ["pos_x", "size_x", "size_y"] { want["window"].as_object_mut().unwrap().remove(k); }
        want["window"]["aui_state"]["panes"][0]["dock"].as_object_mut().unwrap().remove("size");
        for k in ["right_panel_width", "properties_panel_width"] { want["aui"].as_object_mut().unwrap().remove(k); }
        assert_eq!(d, want, "everything else is as it was");
        assert_eq!(d["window"]["pos_y"], 10, "a sane value beside absurd ones is kept");
        assert_eq!(d["window"]["aui_state"]["panes"][0]["dock"]["proportion"], 100000, "a dock's weight is not a size");
        assert!(repair_doc(&mut d).is_empty(), "a repaired file needs nothing more");

        let mut e = doc(WINDOWS_EESCHEMA);
        let gone = repair_doc(&mut e);
        assert_eq!(gone, ["window.pos_x", "aui.hierarchy_panel_docked_width", "aui.properties_panel_width"], "{gone:?}");
    }

    /// Every number in `d` that looks like a pixel geometry value out of range (what is left after a repair must have none).
    fn absurd(d: &Value, path: &str, out: &mut Vec<String>) {
        match d {
            Value::Object(o) => for (k, v) in o {
                if k == "embed" || k == "proportion" || k == "state" || k == "index" || k == "version" { continue }
                absurd(v, &format!("{path}.{k}"), out)
            },
            Value::Array(a) => for (i, v) in a.iter().enumerate() { absurd(v, &format!("{path}[{i}]"), out) },
            v if big(v) => out.push(path.to_string()),
            _ => {}
        }
    }

    #[test]
    fn real_windows_damage_from_bp_win_is_repaired_exactly() {
        for (name, text, must) in [
            ("pcbnew", BPWIN_PCBNEW, &["window.pos_x", "window.size_x", "window.display", "design_block_chooser.height", "aui.design_blocks_panel_docked_width", "aui.properties_panel_width",
                                        "aui.right_panel_width", "aui.search_panel_height", "aui.search_panel_width"][..]),
            ("eeschema", BPWIN_EESCHEMA, &["window.pos_x", "window.display", "design_block_chooser.height", "aui.design_blocks_panel_docked_width", "aui.properties_panel_width",
                                            "aui.remote_symbol_panel_docked_width", "aui.search_panel_height", "aui.search_panel_width"][..]),
            ("minimized", BPWIN_MINIMIZED, &["window.pos_x", "window.pos_y", "window.display"][..]),
        ] {
            let before = doc(text);
            let mut d = before.clone();
            let gone = repair_doc(&mut d);
            for m in must { assert!(gone.iter().any(|g| g == m), "{name}: {m} not dropped: {gone:?}"); }
            let mut left = vec![]; absurd(&d, "", &mut left);
            assert!(left.is_empty(), "{name}: still out of range: {left:?}");
            // nothing but the dropped entries changed
            let mut want = before.clone();
            for g in &gone {
                let (parent, key) = g.rsplit_once('.').unwrap();
                let mut o = &mut want;
                for k in parent.split('.') { o = &mut o[k]; }
                o.as_object_mut().unwrap().remove(key);
            }
            assert_eq!(d, want, "{name}: only {gone:?} changed");
        }
    }

    #[test]
    fn the_linux_park_position_is_dropped() {
        let mut d = doc(LINUX_PARKED);
        assert_eq!(repair_doc(&mut d), ["window.pos_x", "window.pos_y"]);
        assert_eq!(d["window"]["size_x"], 1395);
        assert_eq!(d["window"]["maximized"], true);
    }

    #[test]
    fn sane_layouts_are_untouched() {
        // a second monitor left of the first, maximized there; a 4K window; KiCad's -1 "unset" panel sizes; a wx 3.2 perspective
        let mut d = serde_json::json!({
            "window": {"pos_x": -2560, "pos_y": -300, "size_x": 3840, "size_y": 2160, "maximized": true, "display": 1,
                       "perspective": "layout2|name=LayersManager;caption=;state=2099196;dir=2;layer=0;row=0;pos=0;prop=100000;bestw=274;besth=-1;minw=-1|dock_size(2,0,0)=276|",
                       "aui_state": {"format": "kicad.wxaui", "version": 1, "panes": [{"name": "x", "dock": {"direction": 2, "layer": 0, "row": 0, "position": 0, "proportion": 100000, "size": 274}}]}},
            "simulator": {"window": {"pos_x": 40, "pos_y": 40, "size_x": 500, "size_y": 400}},
            "aui": {"right_panel_width": 274, "design_blocks_panel_float_width": -1, "properties_splitter_proportion": 0.5, "net_nav_panel_docked_size": {"width": 296, "height": 411}},
            "embed_like": {"embed": {"pos_x": -30000}}
        });
        let before = d.clone();
        assert!(repair_doc(&mut d).is_empty());
        assert_eq!(d, before);
    }

    #[test]
    fn a_perspective_loses_only_its_absurd_entries() {
        let (fixed, gone) = repair_perspective("layout2|name=Props;prop=100000;bestw=1239146100;besth=300|dock_size(4,0,0)=788723092|dock_size(2,0,0)=276|").unwrap();
        assert_eq!(fixed, "layout2|name=Props;prop=100000;besth=300|dock_size(2,0,0)=276|");
        assert_eq!(gone, ["bestw=1239146100", "dock_size(4,0,0)=788723092"]);
        assert!(repair_perspective("layout2|name=1239146100;state=2099196;prop=100000|").is_none(), "a pane's name, state bits and weight are not sizes");
    }

    #[test]
    fn repair_backs_up_once_writes_atomically_and_leaves_the_rest() {
        let t = crate::plat::tmp::dir("geomguard");
        let (dir, bk) = (t.path().join("10.99"), t.path().join("backups"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pcbnew.json"), WINDOWS_PCBNEW).unwrap();
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(dir.join("pcbnew.json"), std::fs::Permissions::from_mode(0o600)).unwrap(); }
        let clean = "{\n  \"window\": {\n    \"pos_x\": 40,\n    \"pos_y\": 40,\n    \"size_x\": 900,\n    \"size_y\": 720\n  },\n  \"x\": 1.50\n}\n";
        std::fs::write(dir.join("eeschema.json"), clean).unwrap();
        std::fs::write(dir.join("3d_viewer.json"), "{ \"window\": { \"pos_x\": -30000, ").unwrap(); // not JSON: left alone
        std::fs::write(dir.join("kicad_common.json"), "{\"window\": {\"pos_x\": -30000, \"size_x\": 1, \"size_y\": 1}}").unwrap(); // not ours to touch
        let said = repair(&dir, &bk);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(said[0].contains("pcbnew.json: dropped window.pos_x, window.size_x, window.size_y"), "{said:?}");
        assert_eq!(std::fs::read_to_string(bk.join("pcbnew.json")).unwrap(), WINDOWS_PCBNEW, "the backup is the file as it was");
        let now = std::fs::read_to_string(dir.join("pcbnew.json")).unwrap();
        assert!(now.ends_with("}\n"));
        assert!(now.contains("\"line_width\": 1.0,"), "a number's text is kept (1.0, not 1)");
        let keys = |t: &str| doc(t).as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(&now), keys(WINDOWS_PCBNEW), "every key, in its order");
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(dir.join("pcbnew.json")).unwrap().permissions().mode() & 0o777, 0o600); }
        assert_eq!(std::fs::read_to_string(dir.join("eeschema.json")).unwrap(), clean, "a clean file is not written");
        assert_eq!(std::fs::read_to_string(dir.join("3d_viewer.json")).unwrap(), "{ \"window\": { \"pos_x\": -30000, ");
        assert!(std::fs::read_to_string(dir.join("kicad_common.json")).unwrap().contains("-30000"));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 4, "no temporary file left");
        assert!(repair(&dir, &bk).is_empty(), "a second pass changes nothing");
        // damaged again (an older runtime's next exit): repaired again, the first backup kept
        std::fs::write(dir.join("pcbnew.json"), WINDOWS_PCBNEW.replace("\"pos_y\": 10", "\"pos_y\": -30000")).unwrap();
        assert_eq!(repair(&dir, &bk).len(), 1);
        assert_eq!(std::fs::read_to_string(bk.join("pcbnew.json")).unwrap(), WINDOWS_PCBNEW, "the first backup stays");
    }

    #[test]
    fn only_our_runtimes_folder_and_only_with_no_kicad_running() {
        let root = Path::new("/c");
        assert_eq!(dir_for("10.99.0-bp.13.1", root), Some(root.join("10.99")));
        assert_eq!(dir_for("10.0.8", root), None, "a stock KiCad's settings are never touched");
        assert_eq!(dir_for("", root), None);
        let n = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(kicad_running(&n(&["bash", "kicad.real"])));
        assert!(kicad_running(&n(&["kicad-cli"])), "our headless KiCad saves its settings when it stops, too");
        assert!(kicad_running(&n(&["pcbnew"])));
        assert!(!kicad_running(&n(&["bash", "backplane-hub", "kicad-helper"])));
        #[cfg(unix)] assert_eq!(crate::plat::proc::image_names("  1 x\n/usr/bin/KiCad.real\nbash\n"), ["1 x", "kicad.real", "bash"]);
        assert!(process_names().is_some_and(|n| !n.is_empty()), "this machine's processes can be listed");
    }

    #[test]
    fn nothing_is_written_while_a_kicad_runs_and_the_process_list_is_read_only_for_a_repair() {
        let t = crate::plat::tmp::dir("geomguard");
        let (dir, bk) = (t.path().join("10.99"), t.path().join("backups"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("eeschema.json"), LINUX_PARKED.replace("-30000", "40")).unwrap();
        let asked = std::cell::Cell::new(0);
        assert!(repair_when(&dir, &bk, || { asked.set(asked.get() + 1); true }).is_empty());
        assert_eq!(asked.get(), 0, "a clean folder never lists processes");
        std::fs::write(dir.join("eeschema.json"), LINUX_PARKED).unwrap();
        std::fs::write(dir.join("pcbnew.json"), WINDOWS_PCBNEW).unwrap();
        assert!(repair_when(&dir, &bk, || { asked.set(asked.get() + 1); false }).is_empty());
        assert_eq!(asked.get(), 1, "asked once for both files");
        assert_eq!(std::fs::read_to_string(dir.join("eeschema.json")).unwrap(), LINUX_PARKED, "a KiCad runs: nothing written");
        assert!(!bk.exists(), "and no backup made");
    }

    #[test]
    fn partial_windows_negative_sizes_and_signed_display_are_repaired_while_sentinels_stay() {
        let mut d = serde_json::json!({
            "window": {
                "pos_x": -30000,
                "pos_y": 100,
                "display": -1,
                "aui_state": {
                    "format": "kicad.wxaui",
                    "version": 1,
                    "panes": [
                        {"name": "Layers", "dock": {"size": -50, "proportion": 100000}},
                        {"name": "Toolbar", "dock": {"size": 0, "proportion": 100000}},
                        {"name": "FloatOk", "floating": {"rect": {"x": -1920, "y": 50, "width": 300, "height": 400}}},
                        {"name": "FloatBad", "floating": {"rect": {"x": 50, "y": 50, "width": -100, "height": 400}}}
                    ]
                },
                "perspective": "layout2|name=Props;prop=100000;bestw=-500;besth=-1;floatx=-1920;floaty=50|dock_size(2,0,0)=-500|dock_size(1,6,0)=0|"
            },
            "design_block_chooser": {"width": -50, "height": -1, "sash_pos_h": 0},
            "aui": {
                "right_panel_width": -500,
                "properties_splitter_proportion": 100,
                "bad_neg_proportion": -0.5,
                "ok_proportion": 0.5,
                "zero_proportion": 0.0,
                "one_proportion": 1.0,
                "unset_proportion": -1,
                "design_blocks_panel_float_width": -1,
                "net_nav_panel_float_pos": {"x": -1200, "y": -50},
                "net_nav_panel_docked_size": {"width": -50, "height": 400}
            }
        });
        let gone = repair_doc(&mut d);
        for key in [
            "window.pos_x",
            "window.display",
            "window.aui_state.panes[0].dock.size",
            "window.aui_state.panes[3].floating.rect",
            "design_block_chooser.width",
            "aui.right_panel_width",
            "aui.properties_splitter_proportion",
            "aui.bad_neg_proportion",
            "aui.net_nav_panel_docked_size",
        ] {
            assert!(gone.iter().any(|g| g == key), "missing {key}: {gone:?}");
        }
        assert!(gone.iter().any(|g| g.contains("bestw=-500")), "{gone:?}");
        assert!(gone.iter().any(|g| g.contains("dock_size(2,0,0)=-500")), "{gone:?}");
        assert_eq!(d["window"]["pos_y"], 100);
        assert_eq!(d["window"]["aui_state"]["panes"][1]["dock"]["size"], 0, "0 toolbar dock size stays");
        assert_eq!(d["window"]["aui_state"]["panes"][2]["floating"]["rect"]["x"], -1920, "multi-monitor negative float rect stays");
        assert_eq!(d["window"]["perspective"], "layout2|name=Props;prop=100000;besth=-1;floatx=-1920;floaty=50|dock_size(1,6,0)=0|");
        assert_eq!(d["design_block_chooser"]["height"], -1, "-1 sentinel stays");
        assert_eq!(d["design_block_chooser"]["sash_pos_h"], 0, "0 stays");
        assert_eq!(d["aui"]["ok_proportion"], 0.5);
        assert_eq!(d["aui"]["zero_proportion"], 0.0);
        assert_eq!(d["aui"]["one_proportion"], 1.0);
        assert_eq!(d["aui"]["unset_proportion"], -1);
        assert_eq!(d["aui"]["design_blocks_panel_float_width"], -1, "-1 sentinel stays");
        assert_eq!(d["aui"]["net_nav_panel_float_pos"]["x"], -1200, "multi-monitor negative panel pos stays");
        assert!(repair_doc(&mut d).is_empty(), "clean after repair");
    }

    #[test]
    fn kicad_and_cvpcb_json_are_repaired_while_kicad_common_is_untouched() {
        let t = crate::plat::tmp::dir("geomguard-files");
        let (dir, bk) = (t.path().join("10.99"), t.path().join("backups"));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = "{\n  \"window\": {\n    \"pos_x\": -30000,\n    \"pos_y\": 40,\n    \"size_x\": 900,\n    \"size_y\": 720\n  }\n}\n";
        std::fs::write(dir.join("kicad.json"), bad).unwrap();
        std::fs::write(dir.join("cvpcb.json"), bad).unwrap();
        std::fs::write(dir.join("kicad_common.json"), bad).unwrap();
        let said = repair(&dir, &bk);
        assert_eq!(said.len(), 2, "{said:?}");
        assert!(bk.join("kicad.json").is_file() && bk.join("cvpcb.json").is_file());
        assert!(!bk.join("kicad_common.json").exists(), "kicad_common.json stays untouched");
        assert!(!std::fs::read_to_string(dir.join("kicad.json")).unwrap().contains("-30000"));
        assert!(!std::fs::read_to_string(dir.join("cvpcb.json")).unwrap().contains("-30000"));
        assert!(std::fs::read_to_string(dir.join("kicad_common.json")).unwrap().contains("-30000"));
    }

    #[test]
    fn concurrent_repairs_leave_valid_json_intact_backup_and_no_temp_files() {
        let t = crate::plat::tmp::dir("geomguard-concurrent");
        let (dir, bk) = (t.path().join("10.99"), t.path().join("backups"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pcbnew.json"), WINDOWS_PCBNEW).unwrap();
        let h1 = std::thread::spawn({ let (d, b) = (dir.clone(), bk.clone()); move || repair(&d, &b) });
        let h2 = std::thread::spawn({ let (d, b) = (dir.clone(), bk.clone()); move || repair(&d, &b) });
        let (s1, s2) = (h1.join().unwrap(), h2.join().unwrap());
        assert!(!s1.is_empty() || !s2.is_empty());
        let repaired = std::fs::read_to_string(dir.join("pcbnew.json")).unwrap();
        let mut parsed = doc(&repaired);
        assert!(repair_doc(&mut parsed).is_empty(), "repaired JSON must be valid and clean: {repaired}");
        assert_eq!(std::fs::read_to_string(bk.join("pcbnew.json")).unwrap(), WINDOWS_PCBNEW);
        let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["pcbnew.json"], "no leftover temp files: {names:?}");
    }

    #[test]
    fn repair_when_rechecks_may_write_before_backup_and_replace_and_launch_permit_tracks_active() {
        let t = crate::plat::tmp::dir("geomguard-recheck");
        let (dir, bk) = (t.path().join("10.99"), t.path().join("backups"));
        std::fs::create_dir_all(&dir).unwrap();

        // Stopping before replace must leave pcbnew.json untouched.
        std::fs::write(dir.join("pcbnew.json"), WINDOWS_PCBNEW).unwrap();
        let calls = std::cell::Cell::new(0);
        let said_abort = repair_when(&dir, &bk, || {
            let n = calls.get() + 1;
            calls.set(n);
            n == 1
        });
        assert_eq!(calls.get(), 2, "must re-check may_write before backup and before replace");
        assert!(said_abort.is_empty(), "{said_abort:?}");
        assert_eq!(std::fs::read_to_string(dir.join("pcbnew.json")).unwrap(), WINDOWS_PCBNEW);
        assert_eq!(std::fs::read_to_string(bk.join("pcbnew.json")).unwrap(), WINDOWS_PCBNEW);

        // Atomic backup helper must leave an existing first backup untouched and leave no temp files.
        write_backup_once(&bk.join("pcbnew.json"), "should-not-overwrite").unwrap();
        assert_eq!(std::fs::read_to_string(bk.join("pcbnew.json")).unwrap(), WINDOWS_PCBNEW);
        let bk_names: Vec<String> = std::fs::read_dir(&bk).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(bk_names, ["pcbnew.json"], "no leftover backup temp files: {bk_names:?}");

        // Repairing file 1 (two checks) and stopping before file 2 leaves file 2 untouched with no backup.
        std::fs::write(dir.join("eeschema.json"), LINUX_PARKED).unwrap();
        let calls = std::cell::Cell::new(0);
        let said = repair_when(&dir, &bk, || {
            let n = calls.get() + 1;
            calls.set(n);
            n <= 2
        });
        assert_eq!(calls.get(), 3, "must re-check may_write before each dirty file's backup and replace");
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(said[0].contains("pcbnew.json"), "{said:?}");
        assert!(!bk.join("eeschema.json").exists(), "second file must not be touched or backed up once may_write returns false");
        assert_eq!(std::fs::read_to_string(dir.join("eeschema.json")).unwrap(), LINUX_PARKED);

        use std::sync::atomic::Ordering::SeqCst;
        let e = epoch();
        let mut p1 = begin_launch();
        let mut p2 = begin_launch();
        assert_ne!(epoch(), e);
        assert!(ACTIVE_LAUNCHES.load(SeqCst) >= 2);
        assert!(!p1.spawning && !p2.spawning);
        guard_for_launch(&mut p1, "10.0.8", &t.path().join("hub"));
        assert!(p1.spawning && !p2.spawning);
        assert!(SPAWNING_LAUNCHES.load(SeqCst) >= 1);
        guard_for_launch(&mut p2, "10.0.8", &t.path().join("hub"));
        assert!(p2.spawning);
        assert!(SPAWNING_LAUNCHES.load(SeqCst) >= 2);
        drop(p1);
        drop(p2);

        // Lock-file backup fallback must publish once, leave an existing backup untouched, and fail cleanly when lock is held.
        let lock_dir = t.path().join("bk-lock");
        std::fs::create_dir_all(&lock_dir).unwrap();
        let bk_file = lock_dir.join("pcbnew.json");
        let t1 = lock_dir.join("tmp1.json");
        std::fs::write(&t1, "first").unwrap();
        publish_backup_via_lock(&t1, &bk_file).unwrap();
        assert_eq!(std::fs::read_to_string(&bk_file).unwrap(), "first");
        let t2 = lock_dir.join("tmp2.json");
        std::fs::write(&t2, "second").unwrap();
        publish_backup_via_lock(&t2, &bk_file).unwrap();
        assert_eq!(std::fs::read_to_string(&bk_file).unwrap(), "first");

        let bk_busy = lock_dir.join("eeschema.json");
        let lk = bk_busy.with_extension("json.bp-bklk");
        std::fs::write(&lk, "").unwrap();
        let t3 = lock_dir.join("tmp3.json");
        std::fs::write(&t3, "third").unwrap();
        assert!(publish_backup_via_lock(&t3, &bk_busy).is_err());
        assert!(!bk_busy.exists());
        assert!(!t3.exists());
        std::fs::remove_file(&lk).unwrap();
        let rem: Vec<String> = std::fs::read_dir(&lock_dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(rem, ["pcbnew.json"], "no leftover lock or temp files: {rem:?}");
    }
}
