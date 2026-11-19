//! scripts/codex-apikey.sh, the only way a test runs Codex on a paid OpenAI API key, against a stand-in `codex` that records what it was
//! started with: the pinned cheapest model is added or refused anything else, one turn per home, and the key reaches only that child.
//! Nov 19: a run that named no model let Codex take its default (gpt-6.2-sol) and spent about 130 requests on the person's key.
use std::{path::{Path, PathBuf}, process::Command};

fn script() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/codex-apikey.sh") }
fn pinned() -> String { let o = Command::new("sh").arg(script()).arg("--model").output().unwrap(); String::from_utf8(o.stdout).unwrap().trim().to_string() }
struct Rig { d: crate::plat::tmp::TempDir, fake: PathBuf }
fn rig(name: &str) -> Rig {
    let d = crate::plat::tmp::dir(&format!("cxkey-{name}"));
    std::fs::write(d.join("key"), "sk-test-not-real\n").unwrap();
    let fake = d.join("codex");
    std::fs::write(&fake, format!("#!/bin/sh\necho \"key=$OPENAI_API_KEY home=$CODEX_HOME args=$*\" >> '{}'\n", d.join("calls").display())).unwrap();
    { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    Rig { d, fake }
}
impl Rig {
    fn run(&self, args: &[&str]) -> (i32, String) {
        let o = Command::new("sh").arg(script()).args(args).env("CODEX_APIKEY_FILE", self.d.join("key")).env("CODEX_APIKEY_HOME", self.d.join("cx"))
            .env("CODEX_APIKEY_REAL", &self.fake).env_remove("CODEX_APIKEY_TURNS").env_remove("OPENAI_API_KEY").output().unwrap();
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stderr).to_string())
    }
    fn calls(&self) -> Vec<String> { std::fs::read_to_string(self.d.join("calls")).unwrap_or_default().lines().map(String::from).collect() }
}

#[test] fn the_pin_is_the_cheapest_model() { assert_eq!(pinned(), "gpt-5-nano") }

#[test] fn a_run_that_names_no_model_gets_the_pin_and_the_config_holds_it() {
    let r = rig("nomodel");
    let (code, err) = r.run(&["exec", "--json", "hello"]); assert_eq!(code, 0, "{err}");
    let c = r.calls(); assert_eq!(c.len(), 1, "{c:?}");
    assert!(c[0].contains("args=-c model=\"gpt-5-nano\" exec --json hello"), "the pin goes first: {c:?}");
    assert!(c[0].contains("key=sk-test-not-real") && c[0].contains(&format!("home={}", r.d.join("cx").display())), "{c:?}");
    let cfg = std::fs::read_to_string(r.d.join("cx/config.toml")).unwrap(); assert!(cfg.contains("model = \"gpt-5-nano\"") && cfg.contains("model_reasoning_effort = \"low\""), "{cfg}");
}

#[test] fn any_other_model_is_refused_before_codex_starts() {
    let r = rig("other");
    for a in [&["exec", "-m", "gpt-6.2-sol", "x"][..], &["exec", "--model", "gpt-6-luna", "x"], &["exec", "--model=gpt-6.2-sol", "x"], &["-c", "model=\"gpt-6.2-sol\"", "exec", "x"],
              &["-c", "model_provider=oss", "exec", "x"], &["exec", "-p", "big", "x"], &["exec", "--oss", "x"]] {
        let (code, err) = r.run(a); assert_eq!(code, 97, "{a:?}: {err}"); assert!(err.contains("refused"), "{a:?}: {err}");
    }
    assert!(r.calls().is_empty(), "codex never started: {:?}", r.calls());
    let (code, err) = r.run(&["exec", "-m", "gpt-5-nano", "x"]); assert_eq!(code, 0, "the pin itself may be named: {err}");
    assert!(r.calls()[0].contains("args=exec -m gpt-5-nano x"), "named once, not twice: {:?}", r.calls());
}

#[test] fn one_turn_per_run_unless_a_small_cap_is_asked_for() {
    let r = rig("turns");
    assert_eq!(r.run(&["exec", "one"]).0, 0);
    let (code, err) = r.run(&["exec", "resume", "s1", "two"]); assert_eq!(code, 98, "a second turn on the key is refused: {err}");
    assert_eq!(r.calls().len(), 1);
    assert_eq!(r.run(&["--version"]).0, 0, "a call that is no turn is not counted");
    let o = Command::new("sh").arg(script()).args(["exec", "x"]).env("CODEX_APIKEY_FILE", r.d.join("key")).env("CODEX_APIKEY_HOME", r.d.join("cx2"))
        .env("CODEX_APIKEY_REAL", &r.fake).env("CODEX_APIKEY_TURNS", "9").output().unwrap();
    assert_eq!(o.status.code(), Some(2), "more than 3 turns cannot be asked for");
}

#[test] fn it_needs_a_key_file_and_a_scratch_home_never_the_persons_codex_home() {
    let r = rig("needs");
    let o = Command::new("sh").arg(script()).arg("exec").env_remove("CODEX_APIKEY_FILE").env("CODEX_APIKEY_HOME", r.d.join("cx")).env("CODEX_APIKEY_REAL", &r.fake).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let home = std::env::var("HOME").unwrap();
    let o = Command::new("sh").arg(script()).arg("exec").env("CODEX_APIKEY_FILE", r.d.join("key")).env("CODEX_APIKEY_HOME", format!("{home}/.codex")).env("CODEX_APIKEY_REAL", &r.fake).output().unwrap();
    assert_eq!(o.status.code(), Some(2), "the person's own ~/.codex is refused");
    assert!(r.calls().is_empty());
}
