//! The KiCad every agent uses is exactly the one the hub has selected (`kicad::ipc::runtime`: the `kicad.path` setting, else the hub's own
//! install of Backplane KiCad, newest first). At each turn the hub resolves it once and hands it to the agent three ways that all agree:
//! the `<kicad_runtime>` prompt names its path and version, `BACKPLANE_KICAD_CLI` holds its path, and `<home>/shims` (first on the agent's
//! PATH) holds a `kicad-cli` that execs it, with `pcbnew`, `eeschema` and `kicad` beside it when the install has them.
use super::prompt::Kicad;
use crate::files::kicad::ipc::Env;
use std::path::{Path, PathBuf};

/// The programs given a shim when the selected install has them beside its `kicad-cli`.
pub const PROGRAMS: &[&str] = &["kicad-cli", "pcbnew", "eeschema", "kicad"];
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
    let out = crate::proc::std_cmd(crate::proc::Kind::Local, cli).arg("--version").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok();
    let v = out.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    SEEN.get_or_init(Default::default).lock().unwrap().insert(cli.into(), (at, v.clone()));
    v
}

/// One shim's text: exec the selected program with the agent's arguments.
pub fn shim_text(target: &Path) -> String {
    let t = target.to_string_lossy();
    if cfg!(windows) { format!("@echo off\r\nrem Backplane: the KiCad this hub has selected (rewritten at each turn)\r\n\"{t}\" %*\r\n") }
    else { format!("#!/bin/sh\n# Backplane: the KiCad this hub has selected (rewritten at each turn)\nexec '{}' \"$@\"\n", t.replace('\'', "'\\''")) }
}
fn shim_name(p: &str) -> String { if cfg!(windows) { format!("{p}.cmd") } else { p.to_string() } }

/// Write `dir`'s shims for the selected `cli`: one per program in PROGRAMS that the install has beside it; a shim for a program it lacks is
/// removed. Each is replaced whole (written aside, then renamed) and only when its text changes, so a turn running one never reads half of it.
pub fn write_shims(dir: &Path, cli: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let bin = cli.parent().unwrap_or(Path::new(""));
    let ext = cli.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    for p in PROGRAMS {
        let target = if *p == "kicad-cli" { cli.to_path_buf() } else { bin.join(format!("{p}{ext}")) };
        let at = dir.join(shim_name(p));
        if !target.is_file() { let _ = std::fs::remove_file(&at); continue }
        let text = shim_text(&target);
        if std::fs::read_to_string(&at).map_or(false, |t| t == text) { continue }
        let tmp = dir.join(format!(".{}.{}", shim_name(p), std::process::id()));
        std::fs::write(&tmp, &text)?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?; }
        std::fs::rename(&tmp, &at)?;
    }
    Ok(())
}

/// Where a hub keeps its agents' shims.
pub fn shims_dir(home: &Path) -> PathBuf { home.join("shims") }

/// A turn's KiCad from the hub's selected runtime `env`: the prompt's facts and, when a KiCad is there, the shims written and the env to run with.
/// A shim that cannot be written leaves the env with BACKPLANE_KICAD_CLI only (the prompt still names the full path).
pub fn select(env: &Env, shims: &Path, version_of: impl FnOnce(&str) -> Option<String>) -> Selected {
    let kicad = kicad_of(env, version_of);
    if env.cli.is_empty() { let _ = PROGRAMS.iter().try_for_each(|p| std::fs::remove_file(shims.join(shim_name(p))).or_else(|_| Ok::<_, ()>(()))); return Selected { kicad, env: vec![] } }
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

    fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-kicadsel-{n}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
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
        assert!(shims.join("pcbnew").is_file() && !shims.join("eeschema").exists(), "a shim for each program the install has");

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
    #[test] fn a_shim_quotes_its_path() { if cfg!(unix) { assert_eq!(shim_text(Path::new("/a b/it's/kicad-cli")).lines().last().unwrap(), r#"exec '/a b/it'\''s/kicad-cli' "$@""#) } }
}
