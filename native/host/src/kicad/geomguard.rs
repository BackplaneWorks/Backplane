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

/// The settings files of the frames KiCad embeds (BACKPLANE_EMBED::KindOf; the simulator's window is in eeschema.json).
pub const FILES: [&str; 6] = ["pcbnew.json", "eeschema.json", "symbol_editor.json", "fpedit.json", "3d_viewer.json", "gerbview.json"];

/// No screen layout puts a window, or a panel's edge, this far out (KiCad parks embedded frames at -30000; 4 monitors of 4K are 15360).
const LIMIT: f64 = 20_000.0;

/// KiCad's programs: while any of them runs (ours or the person's, any version), no file is touched.
const KICAD_PROGRAMS: [&str; 10] = ["kicad", "kicad.real", "kicad-cli", "kicad-cli.real", "pcbnew", "eeschema", "gerbview", "pl_editor", "pcb_calculator", "bitmap2component"];

fn big(v: &Value) -> bool { v.as_f64().is_some_and(|f| f.abs() >= LIMIT) || (v.is_number() && v.as_f64().is_none()) }
fn any_big(v: &Value) -> bool {
    match v { Value::Object(o) => o.values().any(any_big), Value::Array(a) => a.iter().any(any_big), v => big(v) }
}

/// Remove, in place, each out-of-range member of an aui_state (wx 3.3's JSON layout): a dock's pixel `size`, a floating pane's `rect`.
/// A dock's `proportion` is a weight (often 100000) and `index`, `layer`, `row`, `direction`, `position` are counts: never touched.
fn repair_aui_state(v: &mut Value, path: &str, gone: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            let bad: Vec<String> = o.iter().filter(|(k, v)| match k.as_str() { "size" => any_big(v), "rect" => any_big(v), _ => false }).map(|(k, _)| k.clone()).collect();
            for k in bad { o.remove(&k); gone.push(format!("{path}.{k}")) }
            for (k, sub) in o.iter_mut() { repair_aui_state(sub, &format!("{path}.{k}"), gone) }
        }
        Value::Array(a) => for (i, sub) in a.iter_mut().enumerate() { repair_aui_state(sub, &format!("{path}[{i}]"), gone) },
        _ => {}
    }
}

/// wx 3.2's perspective string ("name=..;bestw=1239146100;prop=100000|dock_size(4,0,0)=788723092|"): the `dock_size(..)=N` entries and
/// pane size items (PERSPECTIVE_SIZES) out of range are removed (wx then uses its default for each). None: nothing changed.
fn repair_perspective(s: &str) -> Option<(String, Vec<String>)> {
    let num_big = |n: &str| !n.is_empty() && n.trim_start_matches('-').chars().all(|c| c.is_ascii_digit()) && n.len() > 1 && n.parse::<f64>().is_ok_and(|f| f.abs() >= LIMIT);
    let mut gone = vec![];
    let parts: Vec<String> = s.split('|').filter_map(|part| {
        if part.starts_with("dock_size(") {
            let n = part.rsplit('=').next().unwrap_or("");
            if num_big(n) { gone.push(part.to_string()); return None }
            return Some(part.to_string())
        }
        let items: Vec<&str> = part.split(';').filter(|item| {
            let (k, n) = item.split_once('=').unwrap_or((item, ""));
            let drop = PERSPECTIVE_SIZES.contains(&k) && num_big(n);
            if drop { gone.push(item.to_string()) }
            !drop
        }).collect();
        Some(items.join(";"))
    }).collect();
    (!gone.is_empty()).then(|| (parts.join("|"), gone))
}

/// A wx 3.2 perspective pane's pixel items (`state` is a bit field, `prop` a weight, `dir`/`layer`/`row`/`pos` counts: never touched).
const PERSPECTIVE_SIZES: [&str; 10] = ["bestw", "besth", "minw", "minh", "maxw", "maxh", "floatx", "floaty", "floatw", "floath"];

/// A library chooser's (`*_chooser`) pixel members.
const CHOOSER_SIZES: [&str; 6] = ["width", "height", "sash_h", "sash_v", "sash_pos_h", "sash_pos_v"];

fn aui_size_key(k: &str) -> bool { ["_width", "_height", "_size", "_pos"].iter().any(|s| k.ends_with(s)) }

/// Remove the out-of-range geometry in one settings document; returns what was removed ("window.pos_x", "aui.right_panel_width").
pub fn repair_doc(doc: &mut Value) -> Vec<String> {
    let mut gone = vec![];
    fn windows(o: &mut Map<String, Value>, path: &str, gone: &mut Vec<String>) {
        let at = |k: &str| if path.is_empty() { k.to_string() } else { format!("{path}.{k}") };
        if o.contains_key("size_x") && o.contains_key("size_y") {
            for k in ["pos_x", "pos_y"] { if o.get(k).is_some_and(big) { o.remove(k); gone.push(at(k)) } }
            for k in ["size_x", "size_y"] { if o.get(k).is_some_and(big) { o.remove(k); gone.push(at(k)) } }
            // a frame that is a child window has no display: wxNOT_FOUND saved as 4294967295 (no machine has 64 monitors)
            if o.get("display").and_then(Value::as_u64).is_some_and(|d| d >= 64) { o.remove("display"); gone.push(at("display")) }
            if let Some(st) = o.get_mut("aui_state") { repair_aui_state(st, &at("aui_state"), gone) }
            if let Some((fixed, what)) = o.get("perspective").and_then(Value::as_str).and_then(repair_perspective) {
                o.insert("perspective".into(), Value::String(fixed));
                gone.extend(what.into_iter().map(|w| format!("{} {w}", at("perspective"))));
            }
        }
        for (k, sub) in o.iter_mut() {
            // the fork's own copy of an embedded frame's geometry (bp after 13.2) is the fork's to keep in range
            if k == "embed" { continue }
            if let Some(so) = sub.as_object_mut() { windows(so, &at(k), gone) }
        }
    }
    let Some(top) = doc.as_object_mut() else { return gone };
    windows(top, "", &mut gone);
    // the library choosers' sizes (design_block_chooser is also the design-blocks panel docked in the frame: 32765 px tall on bp-win)
    for (name, c) in top.iter_mut().filter(|(k, _)| k.ends_with("_chooser")) {
        let Some(c) = c.as_object_mut() else { continue };
        let bad: Vec<String> = c.iter().filter(|(k, v)| CHOOSER_SIZES.contains(&k.as_str()) && big(v)).map(|(k, _)| k.clone()).collect();
        for k in bad { c.remove(&k); gone.push(format!("{name}.{k}")) }
    }
    if let Some(aui) = top.get_mut("aui").and_then(Value::as_object_mut) {
        let bad: Vec<String> = aui.iter().filter(|(k, v)| aui_size_key(k) && any_big(v)).map(|(k, _)| k.clone()).collect();
        for k in bad { aui.remove(&k); gone.push(format!("aui.{k}")) }
    }
    gone
}

/// Write `text` over `f` atomically (a temporary file beside it, renamed over it), with `f`'s permissions.
fn replace(f: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let perms = std::fs::metadata(f)?.permissions();
    let tmp = f.with_extension(format!("json.bp-tmp{}", std::process::id()));
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

/// Repair every embedded editor's settings file in `dir` (one version's folder, `<config>/kicad/10.99`). Before a file's first repair it
/// is copied to `backups/<file>` (kept; a later repair finds it there and leaves it). Returns one line per file changed or refused.
#[cfg(test)]
pub fn repair(dir: &Path, backups: &Path) -> Vec<String> { repair_when(dir, backups, || true) }

/// `repair`, asking `may_write` (once, and only when some file needs a repair) whether it may write at all.
pub fn repair_when(dir: &Path, backups: &Path, may_write: impl FnOnce() -> bool) -> Vec<String> {
    let mut said = vec![];
    let mut may_write = Some(may_write);
    let mut allowed = None;
    for name in FILES {
        let f = dir.join(name);
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let Some(mut doc) = serde_json::from_str::<Value>(&text).ok().filter(Value::is_object) else { continue };
        let gone = repair_doc(&mut doc);
        if gone.is_empty() { continue }
        if !*allowed.get_or_insert_with(|| may_write.take().is_some_and(|f| f())) { return said }
        let backup = backups.join(name);
        if !backup.exists() {
            if let Err(e) = std::fs::create_dir_all(backups).and_then(|_| std::fs::write(&backup, &text)) {
                said.push(format!("{}: not repaired, its backup failed: {e}", f.display())); continue
            }
        }
        // KiCad writes 2-space JSON ending in a newline; so do we
        let mut out = serde_json::to_string_pretty(&doc).unwrap_or_default();
        if text.ends_with('\n') { out.push('\n') }
        // read again just before replacing: a file KiCad saved meanwhile is looked at next time
        if std::fs::read_to_string(&f).ok().as_deref() != Some(text.as_str()) { continue }
        if let Err(e) = replace(&f, &out) { said.push(format!("{}: not repaired, could not write: {e}", f.display())); continue }
        said.push(format!("{}: dropped {} (backup {})", f.display(), gone.join(", "), backup.display()));
    }
    said
}

/// Whether any KiCad program runs, of any user (None when the processes cannot be listed: then nothing is repaired either).
fn kicad_running(names: &[String]) -> bool { names.iter().any(|n| KICAD_PROGRAMS.contains(&n.as_str())) }

/// The folder to repair for runtime `version` under the settings root `config_root`: only a Backplane runtime's (10.99).
pub fn dir_for(version: &str, config_root: &Path) -> Option<PathBuf> {
    (super::libtables::backplane_version(version)).then(|| super::libtables::settings_version(version)).flatten().map(|v| config_root.join(v))
}

/// Before an embedded KiCad of runtime `version` starts, or Open in KiCad: repair its settings folder when no KiCad runs. Each line of
/// what was done goes to the hub log.
pub fn guard(version: &str, hub_home: &Path) {
    if crate::copy::on() { return }
    let Some(dir) = super::libtables::config_root().and_then(|root| dir_for(version, &root)) else { return };
    let Some(ver) = dir.file_name().map(|v| v.to_string_lossy().to_string()) else { return };
    // the process list is read only when a file needs a repair (an embed start pays nothing otherwise)
    let idle = || match crate::plat::proc::image_names() {
        Some(names) if !kicad_running(&names) => true,
        Some(_) => { eprintln!("kicad: window geometry (#1185): {} needs a repair; left for the next start, a KiCad is running", dir.display()); false }
        None => false,
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
}
