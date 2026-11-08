//! Windows: a release unpacked from its zip installs itself where the guard can look after it (updates, restarts, rollback), when the person says so.
//! The layout is install.sh's: `<prefix>/builds/<tag>/` (the whole release), `<prefix>/CURRENT` (the tag), `<prefix>/backplane.exe` (the guard,
//! which with no arguments opens CURRENT's app: the Start-menu shortcut points at it, so it survives updates). `<prefix>` is
//! `%LOCALAPPDATA%\Backplane`. Uninstalling is docs/install.md's.
use std::path::{Path, PathBuf};

/// What an unpacked release has beside its client: the guard, the hub and the commit it was built from.
pub const RELEASE_FILES: [&str; 3] = ["backplane.exe", "backplane-hub.exe", "COMMIT"];

/// Whether to ask: a client started with no `--url` (`pending`), not already run by a guard, not pointed at a hub by `BACKPLANE_HUB_BIN`
/// (a test or a dev run), from a folder that is an unpacked release (`has` says which files are beside it), and not already in `<prefix>/builds`.
pub fn offer(pending: bool, guarded: bool, hub_bin_env: bool, under_prefix: bool, has: impl Fn(&str) -> bool) -> bool {
    pending && !guarded && !hub_bin_env && !under_prefix && RELEASE_FILES.iter().all(|f| has(f))
}

/// `--install [DIR]`, the headless install (CI, scripts): no question, DIR or the default prefix. None when the arguments ask for none.
pub fn install_arg(args: &[String]) -> Option<Option<PathBuf>> {
    match args.get(1).map(String::as_str) { Some("--install") => Some(args.get(2).filter(|d| !d.starts_with("--")).map(PathBuf::from)), _ => None }
}

/// The tag of this build's folder: `v` and the version release.yml stamped into Cargo.toml (the updater names its builds the same way).
pub fn tag() -> String { format!("v{}", env!("CARGO_PKG_VERSION")) }

/// Lays the release in `src` out under `prefix` and answers the installed client. The build is copied beside `builds/<tag>` and renamed into
/// place (whole or not at all); an existing `builds/<tag>` is kept. The guard is replaced by rename (Windows lets a running program be renamed,
/// not overwritten). CURRENT is written only when there is none: a later version on an installed machine moves through the guard
/// (`backplane hub update`), which checks it and can go back.
pub fn install(src: &Path, prefix: &Path, tag: &str) -> std::io::Result<PathBuf> {
    let builds = prefix.join("builds"); std::fs::create_dir_all(&builds)?;
    let dest = builds.join(tag);
    if !dest.join("backplane-gpui.exe").is_file() {
        let tmp = builds.join(format!(".install-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        copy_tree(src, &tmp)?;
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::rename(&tmp, &dest)?;
    }
    let guard = prefix.join("backplane.exe");
    let fresh = prefix.join(format!(".backplane-{}.exe", std::process::id()));
    std::fs::copy(dest.join("backplane.exe"), &fresh)?;
    if guard.exists() { let old = prefix.join("backplane.exe.old"); let _ = std::fs::remove_file(&old); std::fs::rename(&guard, &old)?; }
    std::fs::rename(&fresh, &guard)?;
    let current = prefix.join("CURRENT");
    if !current.is_file() { let t = prefix.join(".CURRENT.tmp"); std::fs::write(&t, format!("{tag}\n"))?; std::fs::rename(&t, &current)?; }
    Ok(dest.join("backplane-gpui.exe"))
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?; let t = e.file_type()?;
        if t.is_symlink() { continue } // a release has none; never follow one out of the folder
        if t.is_dir() { copy_tree(&e.path(), &to.join(e.file_name()))? } else { std::fs::copy(e.path(), to.join(e.file_name()))?; }
    }
    Ok(())
}

/// `backplane-gpui.exe --install [DIR]`: lay this unpacked release out in DIR (default `%LOCALAPPDATA%\Backplane`, which also gets the
/// Start-menu entry) with no question, print the installed client's path and end: 0 done, 2 not an unpacked release, 1 the copy failed.
/// Then `DIR\backplane.exe hub run --home H --port N --builds DIR -- --foreground` starts the guard for a home, as the client would.
#[cfg(windows)]
pub fn headless(dir: Option<PathBuf>) -> i32 {
    let Ok(me) = std::env::current_exe() else { return 1 };
    let Some(src) = me.parent() else { return 1 };
    if !RELEASE_FILES.iter().all(|f| src.join(f).is_file()) { eprintln!("--install: {} is not an unpacked release (it needs {})", src.display(), RELEASE_FILES.join(", ")); return 2 }
    let default = std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Backplane"));
    let Some(prefix) = dir.or_else(|| default.clone()) else { eprintln!("--install: no folder given and no LOCALAPPDATA"); return 2 };
    match install(src, &prefix, &tag()) {
        Ok(app) => { if Some(&prefix) == default.as_ref() { shortcut(&prefix) } println!("{}", app.display()); 0 }
        Err(e) => { eprintln!("--install: {e}"); 1 }
    }
}

/// Windows, before the window opens: when `offer` says so, ask; on Install, install and start the installed client with the same arguments.
/// Answers true when this process should end (the installed one runs). Not now, or a failed install, runs this one as it is.
#[cfg(windows)]
pub fn maybe_install(pending: bool) -> bool {
    let Ok(me) = std::env::current_exe() else { return false };
    let Some(dir) = me.parent() else { return false };
    let Some(prefix) = std::env::var_os("LOCALAPPDATA").filter(|p| !p.is_empty()).map(|p| PathBuf::from(p).join("Backplane")) else { return false };
    let under = dir.starts_with(prefix.join("builds"));
    let guarded = crate::launch::guarded_install(&me).is_some();
    if !offer(pending, guarded, std::env::var_os("BACKPLANE_HUB_BIN").is_some(), under, |f| dir.join(f).is_file()) { return false }
    if !ask(&prefix) { return false }
    let app = match install(dir, &prefix, &tag()) {
        Ok(a) => a,
        Err(e) => { say(&format!("Backplane could not install to {}: {e}. It runs from here this time.", prefix.display())); return false }
    };
    shortcut(&prefix);
    use std::os::windows::process::CommandExt;
    std::process::Command::new(&app).args(std::env::args_os().skip(1)).creation_flags(0x0000_0200).spawn().is_ok()
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }

/// One question with two buttons, Install and Not now (a task dialog: comctl32 v6, which gpui's manifest asks for).
#[cfg(windows)]
fn ask(prefix: &Path) -> bool {
    use windows_sys::Win32::UI::Controls::{TaskDialogIndirect, TASKDIALOGCONFIG, TASKDIALOG_BUTTON};
    let (title, main, body) = (wide("Backplane"), wide(&format!("Install Backplane to {}?", prefix.display())), wide("Needed for updates. Not now runs it from this folder and asks again next time."));
    let (b1, b2) = (wide("Install"), wide("Not now"));
    let buttons = [TASKDIALOG_BUTTON { nButtonID: 100, pszButtonText: b1.as_ptr() }, TASKDIALOG_BUTTON { nButtonID: 101, pszButtonText: b2.as_ptr() }];
    let mut c: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    c.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
    c.pszWindowTitle = title.as_ptr(); c.pszMainInstruction = main.as_ptr(); c.pszContent = body.as_ptr();
    c.cButtons = buttons.len() as u32; c.pButtons = buttons.as_ptr(); c.nDefaultButton = 100;
    let mut pressed = 0i32;
    let hr = unsafe { TaskDialogIndirect(&c, &mut pressed, std::ptr::null_mut(), std::ptr::null_mut()) };
    hr == 0 && pressed == 100
}

#[cfg(windows)]
fn say(text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK};
    let (t, m) = (wide("Backplane"), wide(text));
    unsafe { MessageBoxW(std::ptr::null_mut(), m.as_ptr(), t.as_ptr(), MB_OK); }
}

/// The Start-menu entry: `<prefix>\backplane.exe` (the guard opens CURRENT's app), started minimised so its console stays out of sight.
#[cfg(windows)]
fn shortcut(prefix: &Path) {
    let Some(appdata) = std::env::var_os("APPDATA") else { return };
    let lnk = PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Backplane.lnk");
    let q = |p: &Path| p.display().to_string().replace('\'', "''");
    let ps = format!("$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath = '{}'; $s.WorkingDirectory = '{}'; $s.WindowStyle = 7; $s.IconLocation = '{}'; $s.Description = 'Backplane'; $s.Save()",
        q(&lnk), q(&prefix.join("backplane.exe")), q(prefix), q(&prefix.join("builds").join(tag()).join("backplane-gpui.exe")));
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", &ps]).creation_flags(0x0800_0000).status();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-inst-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }

    #[test] fn only_an_unpacked_release_run_on_its_own_is_offered_the_install() {
        let all = |_: &str| true;
        assert!(offer(true, false, false, false, all));
        assert!(!offer(false, false, false, false, all), "--url: a hub was given");
        assert!(!offer(true, true, false, false, all), "already run by a guard");
        assert!(!offer(true, false, true, false, all), "BACKPLANE_HUB_BIN: a test or a dev run");
        assert!(!offer(true, false, false, true, all), "already in <prefix>/builds");
        assert!(!offer(true, false, false, false, |f: &str| f != "COMMIT"), "a build tree has no COMMIT beside the client");
        assert!(!offer(true, false, false, false, |f: &str| f != "backplane.exe"), "no guard to install");
    }

    #[test] fn install_takes_a_folder_or_none() {
        let a = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(install_arg(&a(&["gpui", "--install", "D:/bp"])), Some(Some(PathBuf::from("D:/bp"))));
        assert_eq!(install_arg(&a(&["gpui", "--install"])), Some(None));
        assert_eq!(install_arg(&a(&["gpui", "--url", "x"])), None);
        assert_eq!(install_arg(&a(&["gpui"])), None);
    }
    #[test] fn the_install_is_install_sh_s_layout_and_keeps_what_is_there() {
        let (src, pre) = (tmp("src"), tmp("pre"));
        for f in ["backplane-gpui.exe", "backplane-hub.exe", "backplane.exe", "COMMIT"] { std::fs::write(src.join(f), f).unwrap(); }
        std::fs::create_dir_all(src.join("assets/icon")).unwrap(); std::fs::write(src.join("assets/icon/a.svg"), "svg").unwrap();
        std::fs::create_dir_all(src.join("licenses/rust")).unwrap(); std::fs::write(src.join("licenses/rust/hub.txt"), "notices").unwrap();
        let app = install(&src, &pre, "v0.15.0").unwrap();
        assert_eq!(app, pre.join("builds/v0.15.0/backplane-gpui.exe")); assert!(app.is_file());
        assert_eq!(std::fs::read_to_string(pre.join("builds/v0.15.0/assets/icon/a.svg")).unwrap(), "svg");
        assert_eq!(std::fs::read_to_string(pre.join("builds/v0.15.0/licenses/rust/hub.txt")).unwrap(), "notices", "the notices come with the build (Settings > About names them)");
        assert_eq!(std::fs::read_to_string(pre.join("backplane.exe")).unwrap(), "backplane.exe");
        assert_eq!(std::fs::read_to_string(pre.join("CURRENT")).unwrap(), "v0.15.0\n");
        // a second release: its build is added, the guard replaced, CURRENT left to the guard to move
        std::fs::write(src.join("backplane.exe"), "guard 2").unwrap();
        install(&src, &pre, "v0.15.1").unwrap();
        assert!(pre.join("builds/v0.15.1/backplane-gpui.exe").is_file() && pre.join("builds/v0.15.0/backplane-gpui.exe").is_file());
        assert_eq!(std::fs::read_to_string(pre.join("backplane.exe")).unwrap(), "guard 2");
        assert_eq!(std::fs::read_to_string(pre.join("CURRENT")).unwrap(), "v0.15.0\n", "the guard moves CURRENT, after its health check");
        let left: Vec<String> = std::fs::read_dir(pre.join("builds")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(left.iter().all(|n| !n.starts_with('.')), "no scratch left: {left:?}");
        #[cfg(windows)] assert!(crate::launch::guarded_install(&app).is_some(), "the installed client is run by the guard");
        let _ = (std::fs::remove_dir_all(&src), std::fs::remove_dir_all(&pre));
    }
}
