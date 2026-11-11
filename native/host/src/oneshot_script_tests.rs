//! scripts/claude-oneshot.py and scripts/build-haiku.sh against a stand-in `claude`: the default login out for the week (a 429 result, as on Nov 11)
//! must not leave a build without its haiku; a named account of the hub's home answers, and the login that ran out is skipped for an hour.
use std::{path::{Path, PathBuf}, process::Command};

fn root() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..") }
/// A temp home with two named Claude accounts (one with a store folder, one a sync account the script must leave alone) and a stand-in claude:
/// with no CLAUDE_CONFIG_DIR it answers the default login's weekly limit, with one it answers a haiku and says which store it ran under.
fn rig(name: &str) -> (crate::plat::tmp::TempDir, PathBuf) {
    let d = crate::plat::tmp::dir(&format!("oneshot-{name}"));
    let acc = d.join("home/providers/accounts-v1"); std::fs::create_dir_all(acc.join("a1/s1")).unwrap(); std::fs::create_dir_all(acc.join("sy")).unwrap();
    std::fs::write(acc.join("profiles.json"), r#"{"version":1,"profiles":[{"id":"a1","provider":"claude","label":"work","epoch":2,"store":"s1"},{"id":"sy","provider":"claude","label":"sync","epoch":1,"store":"","kind":"sync"},{"id":"cx","provider":"codex","label":"c","epoch":1,"store":""}]}"#).unwrap();
    let fake = d.join("claude");
    std::fs::write(&fake, format!("#!/bin/sh\ncat > /dev/null\necho \"${{CLAUDE_CONFIG_DIR:-default}} $*\" >> {}\n\
if [ -z \"${{CLAUDE_CONFIG_DIR:-}}\" ]; then echo '{{\"type\":\"result\",\"is_error\":true,\"result\":\"You'\"'\"'ve hit your weekly limit\",\"api_error_status\":429}}'; exit 1; fi\n\
echo '{{\"type\":\"result\",\"is_error\":false,\"result\":\"Pins hold the threads\\\\nicons wake on every board\\\\nHaiku finds its voice\"}}'\n", d.join("calls").display())).unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    (d, fake)
}
fn calls(d: &Path) -> Vec<String> { std::fs::read_to_string(d.join("calls")).unwrap_or_default().lines().map(String::from).collect() }

#[test] fn a_one_shot_moves_past_a_login_out_for_the_week_and_skips_it_for_an_hour() {
    let (d, fake) = rig("helper");
    let run = || { let mut c = Command::new("python3"); c.arg("-I").arg(root().join("scripts/claude-oneshot.py")).args(["--model", "claude-haiku-5-5"])
        .env("BACKPLANE_HOME", d.join("home")).env("ONESHOT_CLAUDE", &fake).env("ONESHOT_STATE", d.join("state.json")).env_remove("CLAUDE_CONFIG_DIR")
        .stdin(std::process::Stdio::null()); c.output().unwrap() };
    let o = run(); assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(String::from_utf8_lossy(&o.stdout).lines().count(), 3, "the named account's answer");
    let c = calls(&d); assert_eq!(c.len(), 2, "{c:?}"); assert!(c[0].starts_with("default ") && c[1].contains("/a1/s1 ") && c[1].contains("--setting-sources user"), "default login first, then the account's store: {c:?}");
    assert!(!c.iter().any(|l| l.contains("/sy")), "a sync account is the hub's to run");
    let _ = run(); let c = calls(&d); assert_eq!(c.len(), 3, "the login that ran out is skipped for an hour: {c:?}"); assert!(c[2].contains("/a1/s1 "));
}
#[test] fn a_build_gets_its_haiku_when_the_default_login_is_out() {
    let (d, fake) = rig("haiku");
    let repo = d.join("repo"); std::fs::create_dir_all(&repo).unwrap();
    let git = |a: &[&str]| { let o = Command::new("git").arg("-C").arg(&repo).args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"]).args(a).output().unwrap(); assert!(o.status.success(), "{a:?}"); String::from_utf8_lossy(&o.stdout).trim().to_string() };
    git(&["init", "-q"]); git(&["commit", "-q", "--allow-empty", "-m", "one"]); let from = git(&["rev-parse", "HEAD"]); git(&["commit", "-q", "--allow-empty", "-m", "pins: pin threads"]); let to = git(&["rev-parse", "HEAD"]);
    let o = Command::new("bash").arg(root().join("scripts/build-haiku.sh")).arg(&repo).arg(&from).arg(&to)
        .env("BACKPLANE_HOME", d.join("home")).env("HAIKU_CLAUDE", &fake).env("ONESHOT_STATE", d.join("state.json")).env_remove("CLAUDE_CONFIG_DIR").output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout); assert_eq!(out.lines().collect::<Vec<_>>(), ["Pins hold the threads", "icons wake on every board", "Haiku finds its voice"], "{out:?} {}", String::from_utf8_lossy(&o.stderr));
}
