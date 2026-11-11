//! Open in KiCad (0.14's `kicad-open`, server/kicad's `KiCad.open`): exactly what the thread's viewer shows, in the KiCad that sits beside
//! the hub's `kicad-cli` (ipc::runtime), detached so it outlives the hub and is never signalled by it. The client sends its viewer key
//! (`<kind>|<root>|<file>`; else the hub uses the client's last `viewer.state` report): the board editor on the board shown (a board shown
//! with viewer_show or from another project included), the schematic editor on the schematic; a page that names no file opens its folder's
//! project board or schematic, resolved as the viewer resolves it (a hardware project's main checkout, canonical::design_root). With nothing
//! on show it says so. Every outcome is a line in the thread too (ActivityLogged, kind `kicad`).
use super::project::{self, Files};
use crate::hub::Hub;
use serde_json::json;
use std::{path::{Path, PathBuf}, process::Stdio, sync::Arc};

/// Which program on which file: ("kicad", the project) | ("pcbnew", the board) | ("eeschema", the schematic). None: no design here.
pub fn pick(f: &Files) -> Option<(&'static str, String)> {
    let has = |p: &str| !p.is_empty() && Path::new(p).is_file();
    if has(&f.pro) { Some(("kicad", f.pro.clone())) } else if has(&f.pcb) { Some(("pcbnew", f.pcb.clone())) } else if has(&f.sch) { Some(("eeschema", f.sch.clone())) } else { None }
}
/// The program beside `cli` (an install's bin/, a macOS bundle's Contents/MacOS, /usr/bin): `tool`, else `tool.exe`.
pub fn program(cli: &str, tool: &str) -> Option<PathBuf> {
    let dir = Path::new(cli).parent()?;
    [tool.to_string(), format!("{tool}.exe")].into_iter().map(|n| dir.join(n)).find(|p| p.is_file())
}
/// Start `tool` on `file`, detached: its own group, no pipes, the hub's working folder never held. Waited for off the hub's threads, so no zombie stays.
pub fn launch(bin: &Path, file: &str) -> Result<(), String> {
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, bin);
    c.arg(file).current_dir(Path::new(file).parent().unwrap_or(Path::new("/"))).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    crate::plat::proc::own_group(&mut c);
    let mut child = c.spawn().map_err(|e| format!("could not start {}: {e}", bin.display()))?;
    crate::plat::proc::adopt(child.id(), false);
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok(())
}
/// The whole request on known paths: what was opened ("pcbnew /x/b.kicad_pcb"), or why not.
pub fn open_in(cli: &str, root: &str) -> Result<String, String> {
    let Some((tool, file)) = pick(&project::read(root)) else { return Err("no KiCad project here".into()) };
    run(cli, tool, &file)
}
fn run(cli: &str, tool: &str, file: &str) -> Result<String, String> {
    if cli.is_empty() { return Err("KiCad was not found: set its path in Settings, CAD".into()) }
    let Some(bin) = program(cli, tool) else { return Err(format!("KiCad has no {tool} beside {cli}")) };
    launch(&bin, file).map(|_| format!("{tool} {file}"))
}
/// What the viewer draws, from its key (`<kind>|<root>|<file>`: 0 board, 1 schematic, 2 3D or a
/// Mech part, `split`), resolved as the viewer resolves it: the file it names, else the root's
/// project board or schematic for that page (a hardware project's main checkout, canonical::design_root).
/// Ok(program, file); Err says what is on show instead.
pub fn shown(key: &str) -> Result<(&'static str, String), String> {
    let mut it = key.splitn(3, '|');
    let (kind, root, file) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
    if kind.is_empty() || (root.is_empty() && file.is_empty()) { return Err(NOTHING.into()) }
    let is = |p: &str, x: &str| p.ends_with(x) && Path::new(p).is_file();
    if is(file, ".kicad_pcb") { return Ok(("pcbnew", file.to_string())) }
    if is(file, ".kicad_sch") { return Ok(("eeschema", file.to_string())) }
    if !file.is_empty() && kind == "2" { return Err(format!("The viewer shows a 3D model ({}), not a KiCad design", file.rsplit('/').next().unwrap_or(file))) }
    if !file.is_empty() { return Err(format!("The file the viewer shows is gone: {file}")) }
    let f = project::read(&super::canonical::design_root(root));
    let has = |p: &str| !p.is_empty() && Path::new(p).is_file();
    let pick_kind = match kind { "1" if has(&f.sch) => Some(("eeschema", f.sch.clone())), "0" | "2" if has(&f.pcb) => Some(("pcbnew", f.pcb.clone())), _ => None };
    pick_kind.or_else(|| pick(&f)).ok_or_else(|| format!("The viewer's folder has no KiCad project: {root}"))
}
const NOTHING: &str = "Nothing KiCad is on show in this thread's viewer: show a board or schematic first, then Open in KiCad opens it";
/// `kicad.open {thread, key}`: exactly what the thread's viewer shows (`key`, the client's viewer key; with none, its last
/// `viewer.state` report), else the thread's design; the runtime, the launch, and its line in the thread.
pub async fn open(hub: &Arc<Hub>, thread: &str, key: &str) -> Result<String, String> {
    if crate::copy::on() { return Err("a copy of the log opens nothing".into()) }
    let cli = super::ipc::runtime(hub).await.cli;
    let key = if key.is_empty() { super::watch::viewer_state(thread).1["key"].as_str().unwrap_or("").to_string() } else { key.to_string() };
    let root = crate::files::root_of(hub, thread).await.map(|r| r.to_string_lossy().to_string()).unwrap_or_default();
    let r = tokio::task::spawn_blocking(move || {
        if !key.is_empty() { return shown(&key).and_then(|(tool, file)| run(&cli, tool, &file)) }
        // no viewer report: the thread's own design, if it has one
        let f = project::read(&super::canonical::design_root(&root));
        match pick(&f) { Some((tool, file)) if !root.is_empty() => run(&cli, tool, &file), _ => Err(NOTHING.into()) }
    }).await.unwrap_or_else(|e| Err(e.to_string()));
    let now = crate::agents::now_ms();
    let (tone, summary) = match &r { Ok(what) => ("info", format!("Opened {} in KiCad", what.split_once(' ').map_or(what.as_str(), |(_, f)| f))), Err(why) => ("warn", format!("Open in KiCad: {why}")) };
    let _ = hub.commit(&json!({"$":"ActivityLogged","thread":thread,"act":format!("kicad-open:{now}"),"tone":tone,"kind":"kicad","summary":summary,"at":now})).await;
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("kopen-{n}")) }
    fn touch(p: PathBuf) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, "").unwrap() }
    #[test] fn the_project_first_then_the_board_then_the_schematic() {
        let r = tmp("pick"); let root = r.to_str().unwrap();
        assert_eq!(pick(&project::read(root)), None);
        touch(r.join("a.kicad_sch")); assert_eq!(pick(&project::read(root)).map(|x| x.0), Some("eeschema"));
        touch(r.join("a.kicad_pcb")); assert_eq!(pick(&project::read(root)).map(|x| x.0), Some("pcbnew"));
        touch(r.join("a.kicad_pro")); assert_eq!(pick(&project::read(root)), Some(("kicad", format!("{root}/a.kicad_pro"))));
    }
    #[test] fn it_opens_exactly_what_the_viewer_shows() {
        // a bot's thread showing another project's board (viewer_show): its own folder has no project
        let r = tmp("shown"); let root = r.to_str().unwrap();
        for f in ["boards/usb/usb.kicad_pro", "boards/usb/usb.kicad_pcb", "boards/usb/usb.kicad_sch", "mech/case.step"] { touch(r.join(f)) }
        assert_eq!(shown(&format!("0|{root}/boards/usb|{root}/boards/usb/usb.kicad_pcb")).unwrap(), ("pcbnew", format!("{root}/boards/usb/usb.kicad_pcb")));
        assert_eq!(shown(&format!("2|{root}|{root}/boards/usb/usb.kicad_pcb")).unwrap().0, "pcbnew", "the 3D view of a board opens that board");
        assert_eq!(shown(&format!("1|{root}|{root}/boards/usb/usb.kicad_sch")).unwrap(), ("eeschema", format!("{root}/boards/usb/usb.kicad_sch")));
        // the page with no file named: the root's project board, or schematic, as the viewer resolves it
        assert_eq!(shown(&format!("0|{root}|")).unwrap(), ("pcbnew", format!("{root}/boards/usb/usb.kicad_pcb")));
        assert_eq!(shown(&format!("1|{root}|")).unwrap(), ("eeschema", format!("{root}/boards/usb/usb.kicad_sch")));
        // nothing on show, a Mech part, a folder without a project: said plainly
        assert!(shown("").unwrap_err().contains("Nothing KiCad is on show"));
        assert!(shown(&format!("2|{root}|{root}/mech/case.step")).unwrap_err().contains("3D model (case.step)"));
        let e = tmp("empty"); assert!(shown(&format!("0|{}|", e.display())).unwrap_err().contains("has no KiCad project"));
    }
    #[test] fn what_is_missing_is_said() {
        let r = tmp("miss"); let root = r.to_str().unwrap();
        assert_eq!(open_in("/x/kicad-cli", root), Err("no KiCad project here".into()));
        touch(r.join("a.kicad_pro"));
        assert_eq!(open_in("", root), Err("KiCad was not found: set its path in Settings, CAD".into()));
        assert_eq!(open_in(&format!("{root}/bin/kicad-cli"), root), Err(format!("KiCad has no kicad beside {root}/bin/kicad-cli")));
    }
    #[cfg(unix)] #[test] fn the_program_beside_the_cli_gets_the_file_and_runs_on_its_own() {
        use std::os::unix::fs::PermissionsExt;
        let r = tmp("run"); let root = r.to_str().unwrap(); touch(r.join("hw/m.kicad_pro")); let out = r.join("got");
        let bin = r.join("bin"); std::fs::create_dir_all(&bin).unwrap(); touch(bin.join("kicad-cli"));
        std::fs::write(bin.join("kicad"), format!("#!/bin/sh\necho \"$1 $(pwd)\" > '{}'\n", out.display())).unwrap(); std::fs::set_permissions(bin.join("kicad"), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(open_in(&format!("{root}/bin/kicad-cli"), root), Ok(format!("kicad {root}/hw/m.kicad_pro")));
        for _ in 0..100 { if out.exists() && !std::fs::read_to_string(&out).unwrap().is_empty() { break } std::thread::sleep(std::time::Duration::from_millis(20)) }
        // the program's own `pwd` is the physical folder (macOS temp is /var -> /private/var): same folder, whichever spelling
        let got = std::fs::read_to_string(&out).unwrap(); let (file, cwd) = got.trim().split_once(' ').unwrap();
        assert_eq!(file, format!("{root}/hw/m.kicad_pro")); assert_eq!(std::fs::canonicalize(cwd).unwrap(), std::fs::canonicalize(format!("{root}/hw")).unwrap());
    }
}
