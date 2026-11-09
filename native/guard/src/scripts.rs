//! The scripts around the guard keep to what keeps the person's hub up (docs/HUB-RESILIENCE.md). Each rule here came from a stop of the live hub:
//! - a test's cleanup ends what it started through its own process group or a pidfile, never a name or a pattern (`pkill -f -- "$T/"` with an empty `$T`
//!   matches every process of the user, the live hub included; test/e2e/lib/own-group.sh);
//! - dev-cut.sh publishes no build without the guard (2026-11-09 10:54: a stale builder copy published hub-and-window builds, and the hub ran unguarded).
use std::{fs, path::{Path, PathBuf}, process::Command};

fn root() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap() }
fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-guard-scripts-{n}-{}", std::process::id())); let _ = fs::remove_dir_all(&d); fs::create_dir_all(&d).unwrap(); d }

fn walk(d: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path(); let n = e.file_name().to_string_lossy().to_string();
        if matches!(n.as_str(), "node_modules" | "target" | "build" | ".git") { continue }
        if p.is_dir() { walk(&p, out) } else if matches!(p.extension().and_then(|x| x.to_str()), Some("sh" | "bash" | "py" | "mjs" | "js" | "ts")) { out.push(p) }
    }
}

/// What a line does, without its comment: a whole-line `#` or `//` comment is nothing.
fn code(line: &str) -> &str { let t = line.trim_start(); if t.starts_with('#') || t.starts_with("//") { "" } else { line } }

fn kills_by_name(line: &str) -> bool {
    let l = code(line);
    let word = |w: &str| l.match_indices(w).any(|(i, _)| {
        let b = l[..i].chars().last().map_or(true, |c| !(c.is_alphanumeric() || c == '_' || c == '-'));
        let a = l[i + w.len()..].chars().next().map_or(true, |c| !(c.is_alphanumeric() || c == '_' || c == '-'));
        b && a });
    word("pkill") || word("killall") || (l.contains("kill") && l.contains("$(pgrep -f"))
}

#[test] fn the_rule_sees_a_kill_by_name_and_not_a_comment_or_a_group_kill() {
    for bad in [r#"trap 'pkill -f -- "$T/"' EXIT"#, "killall backplane-hub", r#"kill $HP $(pgrep -f -- "--home $O/hub/home")"#, "subprocess.run(['pkill','-f',home])"] { assert!(kills_by_name(bad), "{bad}") }
    for ok in ["# never pkill -f", r#"kill -TERM -- "-$g""#, "kill -9 $(pgrep -P $HP) $HP", "own_stop", "unpkill_x"] { assert!(!kills_by_name(ok), "{ok}") }
}

#[test] fn no_script_or_test_kills_by_name() {
    let r = root(); let mut files = vec![];
    for d in ["scripts", "test", "deploy", "tools"] { walk(&r.join(d), &mut files) }
    assert!(files.len() > 50, "the walk found {} files under {}", files.len(), r.display());
    let mut bad = vec![];
    for f in files { let Ok(t) = fs::read_to_string(&f) else { continue }; for (i, l) in t.lines().enumerate() { if kills_by_name(l) { bad.push(format!("{}:{}: {}", f.strip_prefix(&r).unwrap().display(), i + 1, l.trim())) } } }
    assert!(bad.is_empty(), "these end processes by name or pattern; end your own process group or pidfile instead (test/e2e/lib/own-group.sh):\n{}", bad.join("\n"));
}

#[test] fn own_stop_ends_the_group_and_its_children_and_nothing_else() {
    let d = tmp("own"); let lib = root().join("test/e2e/lib/own-group.sh");
    let mut other = Command::new("sleep").arg("30").spawn().unwrap();   // a process of the same user that the run did not start
    let s = format!(r#". "{}"; own sh -c 'sleep 300 & echo $! > "{d}/child"; wait'; sleep 0.5; echo $! > "{d}/group"; own_stop"#, lib.display(), d = d.display());
    assert!(Command::new("bash").arg("-c").arg(&s).status().unwrap().success());
    let alive = |f: &str| { let p = fs::read_to_string(d.join(f)).unwrap(); Path::new(&format!("/proc/{}", p.trim())).exists() };
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!alive("group") && !alive("child"), "own_stop left the group or its child running");
    assert!(other.try_wait().unwrap().is_none(), "own_stop ended a process outside its groups");
    let _ = other.kill(); let _ = other.wait(); let _ = fs::remove_dir_all(&d);
}

#[test] fn dev_cut_publishes_no_build_without_the_guard() {
    let d = tmp("cut"); let (src, dev) = (d.join("src"), d.join("dev")); fs::create_dir_all(&src).unwrap();
    use std::os::unix::fs::PermissionsExt;
    let prog = |n: &str| { let p = src.join(n); fs::write(&p, b"\x7fELF fake").unwrap(); fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap(); };
    prog("backplane-hub"); prog("backplane-gpui"); fs::write(src.join("COMMIT"), "abc123\n").unwrap();
    let cut = || Command::new(root().join("scripts/dev-cut.sh")).arg(&src).env("GPUI_DEV_DIR", &dev).env_remove("DEV_CUT_NO_GUARD").output().unwrap();
    let o = cut();
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("no guard"), "a build without the guard was published: {o:?}");
    assert!(!dev.join("CURRENT").exists(), "CURRENT was written for a build without the guard");
    prog("backplane");
    let o = cut();
    assert!(o.status.success(), "a whole build was refused: {o:?}");
    let cur = fs::read_to_string(dev.join("CURRENT")).unwrap();
    assert!(dev.join("builds").join(cur.trim()).join("backplane").is_file(), "the guard is not in the published build");
    let _ = fs::remove_dir_all(&d);
}
