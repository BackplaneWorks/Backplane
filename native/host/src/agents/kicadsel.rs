//! The KiCad every agent uses is exactly the one the hub has selected (`kicad::ipc::runtime`: the `kicad.path` setting, else the hub's own
//! install of Backplane KiCad, newest first). At each turn the hub resolves it once and hands it to the agent three ways that all agree:
//! the `<kicad_runtime>` prompt names its path and version, `BACKPLANE_KICAD_CLI` holds its path, and `<home>/shims` (first on the agent's
//! PATH) holds a `kicad-cli` that execs it, with `pcbnew`, `eeschema` and `kicad` beside it when the install has them.
use super::prompt::Kicad;
use crate::files::kicad::ipc::Env;
use std::path::{Path, PathBuf};

/// The programs given a shim when the selected install has them beside its `kicad-cli`.
pub const PROGRAMS: &[&str] = &["kicad-cli"];
/// KiCad's GUI programs: each gets a shim that refuses (an agent must never put a KiCad window on the person's desktop: a second,
/// non-embedded KiCad showed its first-run "KiCad Setup" wizard over Backplane on bp-win, Nov 17). Ahead of any KiCad on the agent's PATH.
pub const GUI_PROGRAMS: &[&str] = &["kicad", "pcbnew", "eeschema", "gerbview", "pl_editor", "bitmap2component", "pcb_calculator"];
/// What a GUI shim says, and its exit code (1).
pub const GUI_REFUSAL: &str = "Backplane: agents do not open KiCad windows. Show a design with viewer_show, edit it with kicad_edit or kicad_ipc (the KiCad inside Backplane), or use kicad-cli.";
/// The file a refusing shim appends its caller to (parent pid, image, command line); the hub moves it into its log (drain_refusals).
pub const REFUSALS: &str = "refused.log";
/// A GUI program's shim: its caller written to `log`, the one line, exit 1, nothing started.
pub fn gui_shim_text(program: &str, log: &Path) -> String { if cfg!(windows) { cmd_gui_text(program, log) } else { sh_gui_text(program, &log.to_string_lossy()) } }
fn cmd_gui_text(program: &str, log: &Path) -> String {
    let l = log.to_string_lossy();
    format!("@echo off\r\nrem Backplane: {program} is not for agents\r\npowershell -NoProfile -NonInteractive -Command \"$s=Get-CimInstance Win32_Process -Filter ('ProcessId='+$PID); $c=Get-CimInstance Win32_Process -Filter ('ProcessId='+$s.ParentProcessId); $p=Get-CimInstance Win32_Process -Filter ('ProcessId='+$c.ParentProcessId); Add-Content -LiteralPath '{l}' -Value ('caller '+$p.ProcessId+' '+$p.Name+': '+$p.CommandLine+' :: {program} '+$c.CommandLine)\" >nul 2>&1\r\necho {GUI_REFUSAL} 1>&2\r\nexit /b 1\r\n")
}
/// The POSIX sh refusal (Linux, macOS, and Git Bash / MSYS2 / Cygwin on Windows, where `log` is in sh_path's form): LF line endings only.
fn sh_gui_text(program: &str, log: &str) -> String {
    format!("#!/bin/sh\n# Backplane: {program} is not for agents\n{{ printf 'caller %s: ' \"$PPID\"; tr '\\0' ' ' < /proc/$PPID/cmdline 2>/dev/null || ps -o command= -p $PPID 2>/dev/null | tr '\\n' ' '; printf ' :: {program} %s\\n' \"$*\"; }} >> {} 2>/dev/null\necho \"{GUI_REFUSAL}\" >&2\nexit 1\n", sh_quote(log))
}
/// `s` as one sh word: single-quoted, each `'` written `'\''` (nothing else is special inside single quotes: spaces, `$`, backticks,
/// non-ASCII, which is written as UTF-8).
fn sh_quote(s: &str) -> String { format!("'{}'", s.replace('\'', "'\\''")) }
/// A path as a POSIX sh on Windows takes it (#1167: Git Bash, MSYS2 and Cygwin, whose runtimes all accept a Win32 path in mixed form,
/// `C:/Users/...`, where `/c/...` is MSYS's alone and Cygwin's is `/cygdrive/c/...`): `\\?\` dropped, `\\?\UNC\srv` and `\\srv` as `//srv`,
/// backslashes as slashes. Elsewhere the path as it is.
pub fn sh_path(p: &str, windows: bool) -> String {
    if !windows { return p.to_string() }
    let p = if let Some(r) = p.strip_prefix(r"\\?\UNC\") { format!(r"\\{r}") } else { p.strip_prefix(r"\\?\").unwrap_or(p).to_string() };
    p.replace('\\', "/")
}
/// The refusals the shims in `dir` wrote since the last look, each a line in the hub's log (then the file is emptied).
pub fn drain_refusals(dir: &Path) -> Vec<String> {
    let f = dir.join(REFUSALS);
    let Ok(t) = std::fs::read_to_string(&f) else { return vec![] };
    let _ = std::fs::remove_file(&f);
    let v: Vec<String> = t.lines().filter(|l| !l.trim().is_empty()).map(|l| l.chars().take(600).collect()).collect();
    for l in &v { eprintln!("agents: a KiCad GUI shim refused to open a KiCad window: {l}"); }
    v
}
/// The agent's environment key that names the shims' folder; `proc::spawn_with` puts it first on PATH (and leaves the key itself out).
pub const SHIMS_KEY: &str = "BACKPLANE_KICAD_SHIMS";

/// What a turn is given: the prompt's KiCad facts, and the env (`BACKPLANE_KICAD_CLI`, the shims' folder) when a KiCad is selected.
#[derive(Clone, Debug, Default)]
pub struct Selected { pub kicad: Kicad, pub env: Vec<(String, String)> }

/// The prompt's facts for a runtime. A Backplane runtime's version and commit come from its manifest (its `--version` prints only KiCad's
/// number); one with no manifest is asked its `--version` (`version_of`, None when it does not run).
pub fn kicad_of(env: &Env, version_of: impl FnOnce(&str) -> Option<String>) -> Kicad {
    if env.cli.is_empty() { return Kicad { status: "missing".into(), ..Default::default() } }
    let commit = || std::fs::read_to_string(Path::new(&env.root).join("manifest.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|m| m["sourceCommit"].as_str().map(String::from)).filter(|s| !s.is_empty()).or_else(|| std::env::var("BACKPLANE_KICAD_COMMIT").ok()).unwrap_or_default();
    if env.version.contains("-bp.") { return Kicad { status: env.status.clone(), cli: env.cli.clone(), version: env.version.clone(), commit: commit(), why: String::new() } }
    match version_of(&env.cli) {
        Some(v) => Kicad { status: if v.contains("-bp.") { "ok" } else { "stock" }.into(), cli: env.cli.clone(), version: v, commit: commit(), why: String::new() },
        None => Kicad { status: "broken".into(), cli: env.cli.clone(), why: "it does not run".into(), ..Default::default() },
    }
}

/// `--version` of a CLI, remembered per path and modification time (a stock KiCad is asked once, not every turn).
pub fn version_of(cli: &str) -> Option<String> {
    use std::{collections::HashMap, sync::{Mutex, OnceLock}};
    static SEEN: OnceLock<Mutex<HashMap<String, (Option<std::time::SystemTime>, Option<String>)>>> = OnceLock::new();
    let at = std::fs::metadata(cli).and_then(|m| m.modified()).ok();
    if let Some((t, v)) = SEEN.get_or_init(Default::default).lock().unwrap().get(cli).cloned() { if t == at { return v } }
    let out = crate::files::kicad::ipc::cli_version_output(cli);
    let v = out.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    SEEN.get_or_init(Default::default).lock().unwrap().insert(cli.into(), (at, v.clone()));
    v
}

/// One shim's text: exec the selected program with the agent's arguments.
pub fn shim_text(target: &Path) -> String { if cfg!(windows) { cmd_exec_text(target) } else { sh_exec_text(&target.to_string_lossy()) } }
fn cmd_exec_text(target: &Path) -> String { format!("@echo off\r\nrem Backplane: the KiCad this hub has selected (rewritten at each turn)\r\n\"{}\" %*\r\n", target.to_string_lossy()) }
/// The POSIX sh exec shim: `exec`, so the program's exit code, stdin, stdout and stderr are the caller's; LF line endings only.
fn sh_exec_text(target: &str) -> String { format!("#!/bin/sh\n# Backplane: the KiCad this hub has selected (rewritten at each turn)\nexec {} \"$@\"\n", sh_quote(target)) }
/// The files of program `p`'s shim, (name, text): `target` None is a GUI program's refusal. On Windows a `.cmd` (what cmd and PowerShell
/// run: an extensionless file matches no PATHEXT entry) and, beside it, an extensionless POSIX sh shim for Git Bash / MSYS2 (#1167).
fn shim_files(p: &str, target: Option<&Path>, log: &Path, windows: bool) -> Vec<(String, String)> {
    let sh = match target { Some(t) => sh_exec_text(&sh_path(&t.to_string_lossy(), windows)), None => sh_gui_text(p, &sh_path(&log.to_string_lossy(), windows)) };
    if !windows { return vec![(p.to_string(), sh)] }
    let cmd = match target { Some(t) => cmd_exec_text(t), None => cmd_gui_text(p, log) };
    vec![(format!("{p}.cmd"), cmd), (p.to_string(), sh)]
}
/// A shim this hub wrote (now or in an earlier version): its second line is the `rem Backplane:` or `# Backplane:` mark.
fn is_ours(f: &Path) -> bool {
    use std::io::Read;
    let mut b = [0u8; 256]; let n = std::fs::File::open(f).and_then(|mut h| h.read(&mut b)).unwrap_or(0);
    String::from_utf8_lossy(&b[..n]).lines().nth(1).is_some_and(|l| l.starts_with("rem Backplane:") || l.starts_with("# Backplane:"))
}
/// Remove every shim of ours in `dir` (a program's name, or a file with our mark) that is not in `keep` (a program the install lost, one that left GUI_PROGRAMS, the sh shim of a
/// `.cmd` that goes): `refused.log` and files being written (`.name.pid`) are left.
fn sweep(dir: &Path, keep: &[String]) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let known = PROGRAMS.iter().chain(GUI_PROGRAMS).any(|p| n == *p || n == format!("{p}.cmd"));
        if n == REFUSALS || n.starts_with('.') || keep.contains(&n) || !e.path().is_file() || !(known || is_ours(&e.path())) { continue }
        let _ = std::fs::remove_file(e.path());
    }
}

/// Write `dir`'s shims for the selected `cli`: one per program in PROGRAMS that the install has beside it, a refusal per GUI program, and
/// any other shim of ours removed. Each is replaced whole (written aside, then renamed) and only when its text changes, so a turn running
/// one never reads half of it.
pub fn write_shims(dir: &Path, cli: &Path) -> std::io::Result<()> { write_shims_for(dir, cli, cfg!(windows)) }
fn write_shims_for(dir: &Path, cli: &Path, windows: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let wants = PROGRAMS.iter().map(|p| (*p, Some(cli.to_path_buf()))).chain(GUI_PROGRAMS.iter().map(|p| (*p, None)));
    let mut keep = vec![];
    for (p, target) in wants {
        if target.as_ref().is_some_and(|t| !t.is_file()) { continue }
        for (name, text) in shim_files(p, target.as_deref(), &dir.join(REFUSALS), windows) {
            let at = dir.join(&name); keep.push(name.clone());
            if std::fs::read_to_string(&at).map_or(false, |t| t == text) { continue }
            let tmp = dir.join(format!(".{name}.{}", std::process::id()));
            std::fs::write(&tmp, &text)?;
            #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?; }
            std::fs::rename(&tmp, &at)?;
        }
    }
    sweep(dir, &keep);
    Ok(())
}

/// Where a hub keeps its agents' shims.
pub fn shims_dir(home: &Path) -> PathBuf { home.join("shims") }

/// A turn's KiCad from the hub's selected runtime `env`: the prompt's facts and, when a KiCad is there, the shims written and the env to run with.
/// A shim that cannot be written leaves the env with BACKPLANE_KICAD_CLI only (the prompt still names the full path).
pub fn select(env: &Env, shims: &Path, version_of: impl FnOnce(&str) -> Option<String>) -> Selected {
    drain_refusals(shims);
    let kicad = kicad_of(env, version_of);
    if env.cli.is_empty() { sweep(shims, &[]); return Selected { kicad, env: vec![] } }
    let mut e = vec![("BACKPLANE_KICAD_CLI".to_string(), env.cli.clone())];
    match write_shims(shims, Path::new(&env.cli)) { Ok(()) => e.push((SHIMS_KEY.into(), shims.to_string_lossy().into_owned())), Err(x) => eprintln!("agents: the KiCad shims in {} could not be written: {x}", shims.display()) }
    Selected { kicad, env: e }
}

/// The environment a job the hub starts for an agent (`work_run`) gets from the turn's `env`: BACKPLANE_KICAD_CLI, and the shims first on `path`.
pub fn job_env(env: &[(String, String)], path: Option<std::ffi::OsString>) -> Vec<(String, std::ffi::OsString)> {
    let mut out: Vec<(String, std::ffi::OsString)> = env.iter().filter(|(k, _)| k != SHIMS_KEY).map(|(k, v)| (k.clone(), v.into())).collect();
    if let Some((_, d)) = env.iter().find(|(k, _)| k == SHIMS_KEY) {
        let mut v = vec![PathBuf::from(d)]; v.extend(std::env::split_paths(&path.unwrap_or_default()));
        if let Ok(j) = std::env::join_paths(v) { out.push(("PATH".into(), j)) }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::kicad::ipc::{candidates, pick, Places};

    fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("kicadsel-{n}")) }
    /// A stand-in runtime `<parent>/<version>/`: a manifest naming its version and a `bin/kicad-cli` that prints which one it is.
    fn runtime(parent: &Path, version: &str) -> PathBuf {
        let root = parent.join(version); std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(root.join("manifest.json"), format!(r#"{{"version":"{version}","apiVersion":2,"sourceCommit":"c0ffee-{version}","capabilities":["api-server"]}}"#)).unwrap();
        let cli = root.join("bin/kicad-cli");
        std::fs::write(&cli, format!("#!/bin/sh\necho \"{version} $*\"\n")).unwrap();
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        std::fs::write(root.join("bin/pcbnew"), "#!/bin/sh\n").unwrap();
        cli
    }

    /// The Nov 11 case: 0.14's stale home still holds bp.2 and bp.4, a stock 10.0.7 is on PATH, and the hub's own home has bp.8 and bp.9.
    /// The agent's prompt, its BACKPLANE_KICAD_CLI and the `kicad-cli` first on its PATH all name the hub's bp.9, never bend's fork or stock.
    #[cfg(unix)]
    #[tokio::test] async fn an_agent_gets_the_selected_fork_not_a_stale_one() {
        let d = tmp("sel"); let (user, hub) = (d.join("user"), d.join("user/.backplane-gpui-fresh/home"));
        let bend = user.join(".backplane-bend/kicad"); runtime(&bend, "10.99.0-bp.2"); runtime(&bend, "10.99.0-bp.4");
        runtime(&hub.join("kicad"), "10.99.0-bp.8"); let want = runtime(&hub.join("kicad"), "10.99.0-bp.9");
        let local = user.join(".local/bin"); std::fs::create_dir_all(&local).unwrap(); std::fs::write(local.join("kicad-cli"), "#!/bin/sh\necho 10.0.7\n").unwrap();
        let p = Places { data: user.join(".local/share").to_string_lossy().into(), hub_home: hub.clone(), install: hub.join("kicad"), system: true, path: vec![local.clone()], ..Default::default() };
        assert!(candidates(&p).iter().all(|c| !c.starts_with(user.join(".backplane-bend"))), "0.14's home is never a candidate");
        let env = pick(candidates(&p)); assert_eq!(env.cli, want.to_string_lossy(), "the hub's newest install wins over bp.8 and over stock on PATH");

        let shims = shims_dir(&hub); let s = select(&env, &shims, |_| panic!("a fork's version comes from its manifest"));
        assert_eq!((s.kicad.status.as_str(), s.kicad.version.as_str(), s.kicad.commit.as_str()), ("ok", "10.99.0-bp.9", "c0ffee-10.99.0-bp.9"));
        let words = super::super::prompt::kicad(&s.kicad, true);
        assert!(words.contains(&format!("`{}`", want.display())) && words.contains("Backplane KiCad 10.99.0-bp.9") && !words.contains(".backplane-bend") && !words.contains("10.99.0-bp.4") && !words.contains("10.0.7"), "{words}");
        assert!(words.contains("BACKPLANE_KICAD_CLI") && words.contains("`kicad-cli` on your PATH"), "{words}");
        assert!(shims.join("kicad-cli").is_file(), "kicad-cli's shim runs the selected install");
        assert!(GUI_PROGRAMS.iter().all(|p| std::fs::read_to_string(shims.join(p)).is_ok_and(|t| t.contains("not for agents"))), "every GUI program's shim refuses");

        // the agent's own process: its environment names bp.9, and plain `kicad-cli` runs it
        let script = "echo \"cli=$BACKPLANE_KICAD_CLI\"; echo \"shims=${BACKPLANE_KICAD_SHIMS-unset}\"; kicad-cli --version";
        let mut pr = crate::agents::proc::spawn(Path::new("/bin/sh"), &["-c".into(), script.into()], &d, Some(&local), &s.env).unwrap();
        let mut out = vec![]; while let Some(l) = pr.lines.recv().await { out.push(l) }
        assert_eq!(out, vec![format!("cli={}", want.display()), "shims=unset".into(), "10.99.0-bp.9 --version".into()], "the shim comes before the stock kicad-cli on PATH");

        // the selection moves (bp.9's folder gone): the next turn's shims follow it
        std::fs::remove_dir_all(hub.join("kicad/10.99.0-bp.9")).unwrap();
        let s2 = select(&pick(candidates(&p)), &shims, |_| None); assert_eq!(s2.kicad.version, "10.99.0-bp.8");
        assert!(std::fs::read_to_string(shims.join("kicad-cli")).unwrap().contains("10.99.0-bp.8/bin/kicad-cli"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test] fn a_stock_kicad_is_asked_its_version_and_one_that_does_not_run_is_broken() {
        let env = Env { cli: "/usr/bin/kicad-cli".into(), status: "stock".into(), ..Default::default() };
        assert_eq!(kicad_of(&env, |_| Some("10.0.7".into())).status, "stock"); assert_eq!(kicad_of(&env, |_| None).status, "broken");
        assert_eq!(kicad_of(&Env { status: "missing".into(), ..Default::default() }, |_| None).status, "missing");
        let d = tmp("none"); let shims = d.join("shims"); std::fs::create_dir_all(&shims).unwrap(); std::fs::write(shims.join("kicad-cli"), "old").unwrap();
        assert!(select(&Env { status: "missing".into(), ..Default::default() }, &shims, |_| None).env.is_empty() && !shims.join("kicad-cli").exists(), "no KiCad: no shim left to run a gone one");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test] fn a_job_gets_the_shims_first_on_its_path() {
        let e = vec![("BACKPLANE_KICAD_CLI".to_string(), "/k/bin/kicad-cli".to_string()), (SHIMS_KEY.to_string(), "/h/shims".to_string())];
        let j = job_env(&e, Some("/usr/bin:/bin".into()));
        assert_eq!(j, vec![("BACKPLANE_KICAD_CLI".to_string(), "/k/bin/kicad-cli".into()), ("PATH".to_string(), "/h/shims:/usr/bin:/bin".into())]);
        assert!(job_env(&[], Some("/usr/bin".into())).is_empty(), "no KiCad selected: the job's PATH is left alone");
    }
    /// Every GUI program's shim refuses with the one line and exit 1, on every platform: an agent never opens a KiCad window
    #[cfg(unix)]
    #[test] fn every_kicad_gui_shim_refuses_and_starts_nothing() {
        let d = crate::plat::tmp::dir("gui-shims"); let bin = d.join("bin"); std::fs::create_dir_all(&bin).unwrap();
        let cli = bin.join("kicad-cli"); std::fs::write(&cli, "#!/bin/sh\necho cli\n").unwrap();
        { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        // a real GUI program beside it, which must never run
        let marker = d.join("ran"); let gui = bin.join("pcbnew"); std::fs::write(&gui, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&gui, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        let shims = d.join("shims"); write_shims(&shims, &cli).unwrap();
        for p in GUI_PROGRAMS {
            let o = crate::proc::std_cmd(crate::proc::Kind::Local, shims.join(p)).arg("/x/a.kicad_pro").output().unwrap();
            assert_eq!(o.status.code(), Some(1), "{p} refuses");
            assert!(String::from_utf8_lossy(&o.stderr).contains("agents do not open KiCad windows"), "{p}: {:?}", o.stderr);
        }
        assert!(!marker.exists(), "no GUI program ran");
        let named = drain_refusals(&shims);
        assert_eq!(named.len(), GUI_PROGRAMS.len(), "each refusal names its caller: {named:?}");
        assert!(named.iter().all(|l| l.starts_with("caller ") && l.contains("/x/a.kicad_pro")), "{named:?}");
        assert!(drain_refusals(&shims).is_empty(), "moved into the hub's log once");
        let o = crate::proc::std_cmd(crate::proc::Kind::Local, shims.join("kicad-cli")).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "cli", "kicad-cli stays");
    }
    #[test] fn a_gui_shim_names_the_tools_to_use_instead() {
        for p in GUI_PROGRAMS { let t = gui_shim_text(p, Path::new("/h/shims/refused.log")); assert!(t.contains("viewer_show") && t.contains("kicad_edit") && t.contains("kicad_ipc"), "{p}"); assert!(t.contains("exit")); }
    }
    // #1167: on Windows each `.cmd` gets an extensionless POSIX sh shim beside it for Git Bash / MSYS2. Its text is built the same on every
    // platform (write_shims_for(.., windows: true)), so these run under Linux's /bin/sh; bp-win's live check runs them in Git Bash.
    #[cfg(unix)]
    fn exe(at: &Path, body: &str) { std::fs::write(at, body).unwrap(); use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    #[cfg(unix)]
    fn sh(shim: &Path, args: &[&str], stdin: &str) -> std::process::Output {
        use std::io::Write;
        let mut c = std::process::Command::new("/bin/sh").arg(shim).args(args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
        c.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
        c.wait_with_output().unwrap()
    }
    #[test] fn a_windows_path_is_in_the_mixed_form_every_posix_sh_on_windows_takes() {
        assert_eq!(sh_path(r"C:\Users\Jürgen O'Neil\AppData\Local\Backplane\kicad\10.99.0-bp.12\bin\kicad-cli.exe", true), "C:/Users/Jürgen O'Neil/AppData/Local/Backplane/kicad/10.99.0-bp.12/bin/kicad-cli.exe");
        assert_eq!(sh_path(r"\\?\C:\k\bin\kicad-cli.exe", true), "C:/k/bin/kicad-cli.exe");
        assert_eq!(sh_path(r"\\?\UNC\srv\share\bin\kicad-cli.exe", true), "//srv/share/bin/kicad-cli.exe");
        assert_eq!(sh_path(r"\\srv\share\kicad-cli.exe", true), "//srv/share/kicad-cli.exe");
        assert_eq!(sh_path(r"/a\b", false), r"/a\b", "elsewhere the path as it is");
    }
    #[test] fn on_windows_every_cmd_has_an_lf_only_sh_shim_beside_it() {
        let d = tmp("win-files"); let bin = d.join("bin"); std::fs::create_dir_all(&bin).unwrap();
        let cli = bin.join("kicad-cli.exe"); std::fs::write(&cli, "").unwrap();
        let shims = d.join("shims"); write_shims_for(&shims, &cli, true).unwrap();
        for p in PROGRAMS.iter().chain(GUI_PROGRAMS) {
            let cmd = std::fs::read_to_string(shims.join(format!("{p}.cmd"))).unwrap(); let sh = std::fs::read_to_string(shims.join(p)).unwrap();
            assert!(cmd.starts_with("@echo off\r\n") && cmd.ends_with("\r\n") && !cmd.replace("\r\n", "").contains('\n'), "{p}.cmd keeps CRLF");
            assert!(sh.starts_with("#!/bin/sh\n") && !sh.contains('\r'), "{p}: LF only, a shebang Git Bash sees: {sh:?}");
        }
        assert_eq!(std::fs::read_to_string(shims.join("kicad-cli")).unwrap().lines().last().unwrap(), format!("exec '{}' \"$@\"", cli.display()));
        assert!(GUI_PROGRAMS.iter().all(|p| std::fs::read_to_string(shims.join(p)).unwrap().contains("not for agents")), "every GUI sh shim refuses");
        let _ = std::fs::remove_dir_all(&d);
    }
    /// (1) quoting: an apostrophe, a space, non-ASCII, `$` and a backtick in the runtime's path, run under /bin/sh against a stand-in exe;
    /// (2) exec: argv with spaces and quotes arrives as given, stdin is piped through, stdout, stderr and a non-zero exit come back unchanged.
    #[cfg(unix)]
    #[test] fn the_sh_shim_runs_a_runtime_whose_path_has_quotes_spaces_and_non_ascii_and_passes_everything_through() {
        let d = tmp("win-quote"); let bin = d.join("Jürgen O'Neil $HOME `id` ü/bin"); std::fs::create_dir_all(&bin).unwrap();
        let cli = bin.join("kicad-cli.exe");
        exe(&cli, "#!/bin/sh\nfor a; do printf '[%s]\\n' \"$a\"; done\nprintf 'in=%s\\n' \"$(cat)\"\necho oops >&2\nexit 7\n");
        let shims = d.join("shims"); write_shims_for(&shims, &cli, true).unwrap();
        let t = std::fs::read_to_string(shims.join("kicad-cli")).unwrap();
        assert!(t.contains(r"'/") && t.contains(r"Jürgen O'\''Neil $HOME `id` ü/bin/kicad-cli.exe'"), "{t}");
        let o = sh(&shims.join("kicad-cli"), &["pcb", "a b", r#"say "hi""#, "it's", "", "$x `y`"], "piped in");
        assert_eq!(String::from_utf8_lossy(&o.stdout), "[pcb]\n[a b]\n[say \"hi\"]\n[it's]\n[]\n[$x `y`]\nin=piped in\n");
        assert_eq!((o.status.code(), String::from_utf8_lossy(&o.stderr).as_ref()), (Some(7), "oops\n"), "exit code and stderr pass through");
        let _ = std::fs::remove_dir_all(&d);
    }
    /// 6a9123e3d's refusal, kept by the sh shims: exit 1, the one line, nothing started, the caller in refused.log (whose path has a quote).
    #[cfg(unix)]
    #[test] fn a_windows_sh_gui_shim_refuses_like_its_cmd() {
        let d = tmp("win-gui"); let bin = d.join("bin"); std::fs::create_dir_all(&bin).unwrap();
        let cli = bin.join("kicad-cli.exe"); exe(&cli, "#!/bin/sh\n");
        let marker = d.join("ran"); exe(&bin.join("pcbnew.exe"), &format!("#!/bin/sh\ntouch '{}'\n", marker.display()));
        let shims = d.join("it's shims"); write_shims_for(&shims, &cli, true).unwrap();
        for p in GUI_PROGRAMS {
            let o = sh(&shims.join(p), &["/x/a b.kicad_pro"], "");
            assert_eq!(o.status.code(), Some(1), "{p} refuses");
            assert_eq!(String::from_utf8_lossy(&o.stderr).trim(), GUI_REFUSAL, "{p}");
        }
        assert!(!marker.exists(), "no GUI program ran");
        let named = drain_refusals(&shims);
        assert_eq!(named.len(), GUI_PROGRAMS.len(), "{named:?}"); assert!(named.iter().all(|l| l.starts_with("caller ") && l.contains("/x/a b.kicad_pro")), "{named:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
    /// (3) stale shims: a runtime that moves rewrites both; one whose kicad-cli is gone, or a program that left GUI_PROGRAMS, loses its sh
    /// shim with its .cmd; a hub from before #1167 (only .cmd files) gets its sh shims at the next turn; files not ours stay.
    #[test] fn stale_sh_shims_go_with_their_cmd_and_a_hub_with_only_cmds_is_fine() {
        let d = tmp("win-stale"); let shims = d.join("shims"); std::fs::create_dir_all(&shims).unwrap();
        let (a, b) = (d.join("bp.11/bin"), d.join("bp.12/bin")); std::fs::create_dir_all(&a).unwrap(); std::fs::create_dir_all(&b).unwrap();
        let (ca, cb) = (a.join("kicad-cli.exe"), b.join("kicad-cli.exe")); std::fs::write(&ca, "").unwrap(); std::fs::write(&cb, "").unwrap();
        // before #1167: the four .cmd files of the issue, no sh shim; and a GUI program since dropped, with both its files
        for p in ["kicad-cli", "kicad", "eeschema", "pcbnew"] { std::fs::write(shims.join(format!("{p}.cmd")), cmd_exec_text(&ca)).unwrap() }
        std::fs::write(shims.join("oldtool.cmd"), cmd_gui_text("oldtool", &shims.join(REFUSALS))).unwrap();
        std::fs::write(shims.join("oldtool"), sh_gui_text("oldtool", "/x/refused.log")).unwrap();
        std::fs::write(shims.join("notes.txt"), "the person's\nfile\n").unwrap(); std::fs::write(shims.join(REFUSALS), "caller 1: x\n").unwrap();
        write_shims_for(&shims, &ca, true).unwrap();
        assert!(std::fs::read_to_string(shims.join("kicad-cli")).unwrap().contains("bp.11/bin/kicad-cli.exe"), "the missing sh shim is written");
        assert!(std::fs::read_to_string(shims.join("pcbnew.cmd")).unwrap().contains("not for agents"), "an old GUI .cmd becomes the refusal");
        assert!(!shims.join("oldtool.cmd").exists() && !shims.join("oldtool").exists(), "a program that left GUI_PROGRAMS loses both files");
        assert!(shims.join("notes.txt").exists() && shims.join(REFUSALS).exists(), "files not ours stay");
        write_shims_for(&shims, &cb, true).unwrap();
        assert!(["kicad-cli", "kicad-cli.cmd"].iter().all(|n| std::fs::read_to_string(shims.join(n)).unwrap().contains("bp.12")), "the runtime moved: both follow");
        std::fs::remove_file(&cb).unwrap(); write_shims_for(&shims, &cb, true).unwrap();
        assert!(!shims.join("kicad-cli").exists() && !shims.join("kicad-cli.cmd").exists(), "kicad-cli gone: both shims go");
        assert!(shims.join("kicad").exists() && shims.join("kicad.cmd").exists(), "the refusals stay");
        std::fs::write(&cb, "").unwrap(); std::fs::remove_file(shims.join("kicad")).unwrap(); write_shims_for(&shims, &cb, true).unwrap();
        assert!(shims.join("kicad").exists(), "a deleted sh shim comes back at the next turn");
        assert!(select(&Env { status: "missing".into(), ..Default::default() }, &shims, |_| None).env.is_empty());
        let left: Vec<_> = std::fs::read_dir(&shims).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, vec!["notes.txt".to_string()], "no KiCad: every shim of ours goes, sh and .cmd");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_shim_quotes_its_path() { if cfg!(unix) { assert_eq!(shim_text(Path::new("/a b/it's/kicad-cli")).lines().last().unwrap(), r#"exec '/a b/it'\''s/kicad-cli' "$@""#) } }
}
