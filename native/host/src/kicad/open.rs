//! Open in KiCad (0.14's `kicad-open`, server/kicad's `KiCad.open`): the thread's design in the KiCad that sits beside the hub's
//! `kicad-cli` (ipc::runtime), detached so it outlives the hub and is never signalled by it. The project manager on the `.kicad_pro`;
//! with none, the board editor on the board, else the schematic editor. It opens the files the thread's viewer shows (its own folder,
//! a worktree's copy for a worktree thread). Every outcome is a line in the thread too (ActivityLogged, kind `kicad`).
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
    if cli.is_empty() { return Err("KiCad was not found: set its path in Settings, CAD".into()) }
    let Some(bin) = program(cli, tool) else { return Err(format!("KiCad has no {tool} beside {cli}")) };
    launch(&bin, &file).map(|_| format!("{tool} {file}"))
}
/// `kicad.open {thread}`: the thread's folder, the runtime, the launch, and its line in the thread.
pub async fn open(hub: &Arc<Hub>, thread: &str) -> Result<String, String> {
    if crate::copy::on() { return Err("a copy of the log opens nothing".into()) }
    let Some(root) = crate::files::root_of(hub, thread).await else { return Err("this thread has no folder".into()) };
    let cli = super::ipc::runtime(hub).await.cli;
    let root = root.to_string_lossy().to_string();
    let r = tokio::task::spawn_blocking(move || open_in(&cli, &root)).await.unwrap_or_else(|e| Err(e.to_string()));
    let now = crate::agents::now_ms();
    let (tone, summary) = match &r { Ok(what) => ("info", format!("Opened {} in KiCad", what.split_once(' ').map_or(what.as_str(), |(_, f)| f))), Err(why) => ("warn", format!("Open in KiCad: {why}")) };
    let _ = hub.commit(&json!({"$":"ActivityLogged","thread":thread,"act":format!("kicad-open:{now}"),"tone":tone,"kind":"kicad","summary":summary,"at":now})).await;
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-kopen-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn touch(p: PathBuf) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, "").unwrap() }
    #[test] fn the_project_first_then_the_board_then_the_schematic() {
        let r = tmp("pick"); let root = r.to_str().unwrap();
        assert_eq!(pick(&project::read(root)), None);
        touch(r.join("a.kicad_sch")); assert_eq!(pick(&project::read(root)).map(|x| x.0), Some("eeschema"));
        touch(r.join("a.kicad_pcb")); assert_eq!(pick(&project::read(root)).map(|x| x.0), Some("pcbnew"));
        touch(r.join("a.kicad_pro")); assert_eq!(pick(&project::read(root)), Some(("kicad", format!("{root}/a.kicad_pro"))));
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
        assert_eq!(std::fs::read_to_string(&out).unwrap().trim(), format!("{root}/hw/m.kicad_pro {root}/hw"));
    }
}
