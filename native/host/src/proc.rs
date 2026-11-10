//! The ONE place a child process is made. Every lane builds its commands with `tokio_cmd` / `std_cmd` and says what the process is
//! for (`Kind`); on a copy of a real log (`BACKPLANE_COPY=1`, copy.rs) a command that acts outside the home is not run: the program
//! becomes a path that does not exist, so the caller's own "could not start" handling answers, with the reason in the error.
//! A source scan (tests below) fails the build of any other `Command::new` in non-test code.
use crate::copy;
use std::ffi::OsStr;

/// What a process is for. On a copy: Net, Terminal, Browser and Shell are refused outright, Agent only from the stand-in folder
/// (`BACKPLANE_AGENTS`), Local (git reads, kicad-cli, our own helpers) unless the program is a network command by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// our own helper children and local tools: the policy, the window program, plot, kicad-cli, git (its arguments are checked where known)
    Local,
    /// reaches another machine: curl, tailscale, ssh...
    Net,
    /// a user's terminal (a shell on a pty)
    Terminal,
    /// the browser helper
    Browser,
    /// a shell running a command line an agent or the user supplied (secret_run, hooks)
    Shell,
    /// an agent CLI (claude, codex, ...)
    Agent,
    /// a REAL agent CLI for a turn the person started (side-by-side mode only: `copy::person_turn_allowed()`); the only other kind that runs a real agent on a copy
    PersonTurn,
}

/// Set once in safe mode (guard.rs): no agent starts, for this reason.
pub static PAUSED: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Why a copy does not run this, or None. `on` is the copy switch (a parameter so the rule is testable).
pub fn refusal_with(on: bool, kind: Kind, program: &str, agents_dir: Option<&str>) -> Option<String> {
    if !on { return None }
    let base = program.rsplit('/').next().unwrap_or(program);
    let no = || Some(copy::refusal(base));
    match kind {
        Kind::Net | Kind::Terminal | Kind::Browser | Kind::Shell | Kind::PersonTurn => no(), // PersonTurn: refused here; the global `refusal` lets it through in side-by-side mode
        Kind::Agent => match agents_dir { Some(d) if !d.is_empty() && program.starts_with(&format!("{}/", d.trim_end_matches('/'))) => None, _ => no() },
        Kind::Local => if copy::refused_with(true, program, &[]) { no() } else { None },
    }
}
fn refusal(kind: Kind, program: &str) -> Option<String> {
    if let (Some(why), Kind::Agent | Kind::PersonTurn) = (PAUSED.get(), kind) { return Some(why.clone()) }
    if !copy::on() { return None }
    if kind == Kind::PersonTurn && copy::person_turn_allowed() { return None }
    refusal_with(true, kind, program, std::env::var("BACKPLANE_AGENTS").ok().as_deref())
}
/// The hub's own variables from its guard and updater: which builds folder it was started from, its commit, its guard, safe mode, injected faults, the update feed.
/// An agent, a shell an agent or the person runs, or a terminal never gets them: a test hub started from one would take the person's builds folder for its own,
/// and could update itself against it (or a `backplane hub` there would act on the person's hub). Its listener and its address and name on the network go too
/// (a test hub would bind the person's tailnet port, or claim the hub's URL), and the push key (it would notify the person's phones). Nothing an agent runs
/// needs them to reach the hub: the MCP bridge gets the hub's URL as an argument, and `backplane hub` finds it by --home or its pointer file.
/// BACKPLANE_NO_UPDATE stays: it only ever holds an update back.
pub const HUB_ONLY: &[&str] = &["BACKPLANE_BUILDS", "BACKPLANE_BUILD", "BACKPLANE_COMMIT", "BACKPLANE_GUARD", "BACKPLANE_HOME", "BACKPLANE_SAFE", "BACKPLANE_SAFE_SKIP",
    "BACKPLANE_CARRY_WAIT", "BACKPLANE_FAULT", "BACKPLANE_STEP_BUDGET", "BACKPLANE_UPDATE_FEED", "BACKPLANE_RELEASE_KEY", "BACKPLANE_TEST_BUILDS", "BACKPLANE_TEST_NO_UPDATE",
    "BACKPLANE_HTTPS_PORT", "BACKPLANE_URL", "BACKPLANE_NAME", "BACKPLANE_HOOK_URL", "BACKPLANE_PEERS"];
pub fn hub_only(name: &str) -> bool { HUB_ONLY.contains(&name) || name.starts_with("BACKPLANE_GUARD_") || name.starts_with("BACKPLANE_APNS_") }
/// The kinds that run what an agent or a person supplies.
fn scrubbed(kind: Kind) -> bool { matches!(kind, Kind::Agent | Kind::PersonTurn | Kind::Shell | Kind::Terminal) }
fn hub_only_present() -> Vec<std::ffi::OsString> { std::env::vars_os().map(|(k, _)| k).filter(|k| k.to_str().map_or(false, hub_only)).collect() }

fn dead(program: &str, why: &str) -> String { eprintln!("{why}"); format!("/nonexistent/backplane-copy-refused/{}", program.rsplit('/').next().unwrap_or(program)) }

/// `tokio::process::Command::new`, gated.
pub fn tokio_cmd<S: AsRef<OsStr>>(kind: Kind, program: S) -> tokio::process::Command {
    let p = program.as_ref().to_string_lossy().into_owned();
    let mut c = match refusal(kind, &p) { None => { let (prog, pre) = crate::plat::hub::launch(program.as_ref()); let mut c = tokio::process::Command::new(prog); c.args(pre); c } Some(why) => tokio::process::Command::new(dead(&p, &why)) };
    // Windows: no console window for what the hub starts, whether or not the hub has a console to share (a guard's, a task's); own_group keeps it
    #[cfg(windows)] c.creation_flags(crate::plat::proc::NO_WINDOW);
    if scrubbed(kind) { for k in hub_only_present() { c.env_remove(k); } }
    c
}
/// `std::process::Command::new`, gated.
pub fn std_cmd<S: AsRef<OsStr>>(kind: Kind, program: S) -> std::process::Command {
    let p = program.as_ref().to_string_lossy().into_owned();
    let mut c = match refusal(kind, &p) { None => { let (prog, pre) = crate::plat::hub::launch(program.as_ref()); let mut c = std::process::Command::new(prog); c.args(pre); c } Some(why) => std::process::Command::new(dead(&p, &why)) };
    #[cfg(windows)] { use std::os::windows::process::CommandExt; c.creation_flags(crate::plat::proc::NO_WINDOW); }
    if scrubbed(kind) { for k in hub_only_present() { c.env_remove(k); } }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    // side-by-side: the person's turn is the one real agent on a copy, and nothing else opens up with it
    #[test] fn a_person_turn_is_refused_on_a_copy_by_the_pure_rule_and_free_off_a_copy() {
        assert!(refusal_with(true, Kind::PersonTurn, "/home/u/.local/bin/claude", None).is_some(), "refused unless side-by-side lets it through (the global `refusal`)");
        assert!(refusal_with(false, Kind::PersonTurn, "claude", None).is_none(), "not a copy: no gate");
    }
    #[test] fn hub_only_names() {
        for k in ["BACKPLANE_BUILDS", "BACKPLANE_COMMIT", "BACKPLANE_GUARD", "BACKPLANE_GUARD_STABLE", "BACKPLANE_SAFE", "BACKPLANE_UPDATE_FEED", "BACKPLANE_HTTPS_PORT", "BACKPLANE_URL", "BACKPLANE_APNS_KEY_ID"] { assert!(hub_only(k), "{k}"); }
        for k in ["BACKPLANE_NO_UPDATE", "BACKPLANE_AGENT_OWNER", "BACKPLANE_AGENTS", "PATH", "BACKPLANE_GUARDIAN"] { assert!(!hub_only(k), "{k}"); }
    }
    /// What prints the environment: /usr/bin/env, or on Windows Git's shell's `env`.
    const ENV: (&str, &[&str]) = if cfg!(windows) { ("sh", &["-c", "env"]) } else { ("/usr/bin/env", &[]) };
    fn env_cmd(mut c: tokio::process::Command) -> tokio::process::Command { c.args(ENV.1); c }
    #[tokio::test] async fn hub_env_probe_child() { // run by the test below with the hub's variables set, as a guarded dev hub has them
        if std::env::var("BP_HUB_ENV_PROBE").is_err() { return }
        let leaked = |out: &[u8]| String::from_utf8_lossy(out).lines().filter(|l| l.split('=').next().map_or(false, hub_only)).map(str::to_string).collect::<Vec<_>>();
        for k in [Kind::Agent, Kind::PersonTurn, Kind::Shell, Kind::Terminal] {
            let o = env_cmd(tokio_cmd(k, ENV.0)).output().await.unwrap(); assert!(o.status.success(), "{k:?}");
            assert!(leaked(&o.stdout).is_empty(), "{k:?} saw {:?}", leaked(&o.stdout));
            assert!(String::from_utf8_lossy(&o.stdout).lines().any(|l| l == "BACKPLANE_NO_UPDATE=1"), "{k:?}: the hold on updates stays");
            let o = { let mut c = std_cmd(k, ENV.0); c.args(ENV.1); c }.output().unwrap(); assert!(leaked(&o.stdout).is_empty(), "std {k:?} saw {:?}", leaked(&o.stdout));
        }
        let o = { let mut c = std_cmd(Kind::Local, ENV.0); c.args(ENV.1); c }.output().unwrap(); assert!(!leaked(&o.stdout).is_empty(), "our own helpers still get them");
        println!("hub_env_probe_ok");
    }
    #[test] fn an_agent_shell_or_terminal_never_sees_the_guards_or_updaters_variables() {
        let me = std::env::current_exe().unwrap();
        let mut c = std::process::Command::new(me); c.args(["proc::tests::hub_env_probe_child", "--exact", "--nocapture", "--test-threads=1"]).env("BP_HUB_ENV_PROBE", "1").env_remove("BACKPLANE_COPY");
        for (k, v) in [("BACKPLANE_BUILDS", "/home/x/.backplane-gpui-dev"), ("BACKPLANE_BUILD", "20261106-1"), ("BACKPLANE_COMMIT", "abc1234"), ("BACKPLANE_GUARD", "1"), ("BACKPLANE_GUARD_STABLE", "60"),
            ("BACKPLANE_SAFE", "1"), ("BACKPLANE_SAFE_SKIP", "bots"), ("BACKPLANE_CARRY_WAIT", "60"), ("BACKPLANE_FAULT", "panic:bots"), ("BACKPLANE_UPDATE_FEED", "http://x"), ("BACKPLANE_NO_UPDATE", "1"),
            ("BACKPLANE_HTTPS_PORT", "8451"), ("BACKPLANE_URL", "https://desktop.example:8451"), ("BACKPLANE_NAME", "desktop"), ("BACKPLANE_HOOK_URL", "http://127.0.0.1:3797"), ("BACKPLANE_PEERS", "a,b"),
            ("BACKPLANE_APNS_KEY_ID", "K"), ("BACKPLANE_APNS_PRIVATE_KEY_PATH", "/home/x/AuthKey.p8")] { c.env(k, v); }
        let o = c.output().unwrap();
        assert!(o.status.success() && String::from_utf8_lossy(&o.stdout).contains("hub_env_probe_ok"), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    }
    #[test] fn side_by_side_probe_child() { // run by the tests below with BACKPLANE_COPY=1 and the flag as given
        let Ok(mode) = std::env::var("BP_SBS_PROBE") else { return };
        assert!(copy::on());
        let person = refusal(Kind::PersonTurn, "/home/u/.local/bin/claude").is_none();
        assert_eq!(person, mode == "on", "PersonTurn runs only with the flag");
        assert_eq!(copy::person_turn_allowed(), mode == "on");
        for (k, p) in [(Kind::Agent, "/home/u/.local/bin/claude"), (Kind::Net, "curl"), (Kind::Shell, "sh"), (Kind::Terminal, "/bin/bash"), (Kind::Browser, "b"), (Kind::Local, "gh")] {
            assert!(refusal(k, p).is_some(), "{k:?} {p} stays refused in side-by-side mode");
        }
        println!("sbs_probe_ok {mode}");
    }
    fn sbs_child(mode: &str, flag: Option<&str>) -> String {
        let mut c = std::process::Command::new(std::env::current_exe().unwrap()); c.args(["proc::tests::side_by_side_probe_child", "--exact", "--nocapture", "--test-threads=1"]).env("BACKPLANE_COPY", "1").env("BP_SBS_PROBE", mode).env_remove("BACKPLANE_AGENTS");
        match flag { Some(f) => { c.env("BACKPLANE_SIDE_BY_SIDE", f); } None => { c.env_remove("BACKPLANE_SIDE_BY_SIDE"); } }
        let o = c.output().unwrap(); assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)); String::from_utf8_lossy(&o.stdout).into_owned()
    }
    #[test] fn with_the_side_by_side_flag_only_a_person_turn_may_run_a_real_agent_and_every_other_gate_holds() { assert!(sbs_child("on", Some("1")).contains(&format!("sbs_probe_ok on"))) }
    #[test] fn without_the_flag_or_with_a_wrong_value_a_person_turn_is_refused_too() { assert!(sbs_child("off", None).contains("sbs_probe_ok off")); assert!(sbs_child("off", Some("yes")).contains("sbs_probe_ok off")); assert!(sbs_child("off", Some("0")).contains("sbs_probe_ok off")) }

    // a copy runs nothing that acts outside its home
    #[test] fn a_copy_refuses_network_terminal_browser_shell_and_foreign_agents() {
        for (k, p) in [(Kind::Net, "curl"), (Kind::Net, "/usr/bin/tailscale"), (Kind::Terminal, "/bin/bash"), (Kind::Browser, "/x/backplane-browser"), (Kind::Shell, "/bin/sh"),
            (Kind::Local, "curl"), (Kind::Local, "/usr/bin/ssh"), (Kind::Local, "gh"), (Kind::Agent, "/usr/bin/claude"), (Kind::Agent, "/home/u/.local/bin/codex")] {
            assert!(refusal_with(true, k, p, None).is_some(), "{k:?} {p}");
            assert!(refusal_with(true, k, p, Some("/tmp/standins")).is_some() || k == Kind::Agent && p.starts_with("/tmp/standins/"), "{k:?} {p}");
        }
        assert!(refusal_with(true, Kind::Agent, "/tmp/standins/claude", Some("/tmp/standins")).is_none(), "a stand-in agent runs");
        assert!(refusal_with(true, Kind::Agent, "/tmp/standins-evil/claude", Some("/tmp/standins")).is_some(), "a sibling folder is not the stand-in folder");
        for (k, p) in [(Kind::Local, "git"), (Kind::Local, "/usr/bin/kicad-cli"), (Kind::Local, "sh"), (Kind::Local, "/x/backplane-window"), (Kind::Local, "nice")] { assert_eq!(refusal_with(true, k, p, None), None, "{k:?} {p}") }
    }
    #[test] fn a_hub_that_is_no_copy_refuses_nothing() {
        for (k, p) in [(Kind::Net, "curl"), (Kind::Terminal, "bash"), (Kind::Browser, "b"), (Kind::Shell, "sh"), (Kind::Agent, "claude"), (Kind::Local, "ssh")] { assert_eq!(refusal_with(false, k, p, None), None) }
    }
    #[test] fn a_refused_command_cannot_start_and_says_why() {
        let p = dead("curl", "x"); assert!(p.starts_with("/nonexistent/backplane-copy-refused/"));
        assert!(std::process::Command::new(&p).output().is_err());
    }

    /// The switch end to end: a child test process started with BACKPLANE_COPY=1 (copy::on reads it once) cannot make any of the
    /// outside-reaching children; a normal process can still start local ones.
    #[test] fn copy_child_probe() {
        if std::env::var("BP_COPY_PROBE").ok().as_deref() != Some("1") { return }
        assert!(copy::on(), "BACKPLANE_COPY=1 reached the child");
        for (k, p) in [(Kind::Net, "curl"), (Kind::Net, "tailscale"), (Kind::Terminal, "/bin/sh"), (Kind::Browser, "/bin/true"), (Kind::Shell, "/bin/sh"), (Kind::Agent, "/bin/true"), (Kind::Local, "ssh")] {
            assert!(std_cmd(k, p).output().is_err(), "{k:?} {p} ran on a copy");
            assert!(tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(tokio_cmd(k, p).output()).is_err(), "{k:?} {p} ran on a copy (tokio)");
        }
        assert!(std_cmd(Kind::Local, "true").output().map_or(false, |o| o.status.success()), "a local tool still runs");
    }
    #[test] fn a_copy_run_does_nothing_outside_its_home() {
        let me = std::env::current_exe().unwrap();
        let o = std::process::Command::new(me).args(["proc::tests::copy_child_probe", "--exact", "--nocapture", "--test-threads=1"])
            .env("BACKPLANE_COPY", "1").env("BP_COPY_PROBE", "1").env_remove("BACKPLANE_AGENTS").output().unwrap();
        assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    }

    /// The gate is the only way to make a process: no other non-test `Command::new` anywhere under src/ (src/bin has its own crates' helpers).
    #[test] fn every_child_process_is_made_through_proc() {
        fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) { for e in std::fs::read_dir(d).unwrap().flatten() { let p = e.path(); if p.is_dir() { if p.file_name().map_or(false, |n| n == "bin") { continue } walk(&p, out) } else if p.extension().map_or(false, |x| x == "rs") { out.push(p) } } }
        let mut files = vec![]; walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut bad = vec![];
        for f in files {
            if f.ends_with("proc.rs") { continue }
            let text = std::fs::read_to_string(&f).unwrap();
            let name = f.to_string_lossy().replace('\\', "/");
            if name.ends_with("tests.rs") || name.contains("/tests/") { continue }
            // everything after the first #[cfg(test)] line is test code
            let code = text.split("\n#[cfg(test)]").next().unwrap_or(&text);
            for (i, l) in code.lines().enumerate() { if l.contains("Command::new(") && !l.trim_start().starts_with("//") { bad.push(format!("{}:{}", name, i + 1)) } }
        }
        assert!(bad.is_empty(), "build children with crate::proc::{{tokio_cmd, std_cmd}}: {bad:?}");
    }

    /// A raw socket to another machine is not a child process, so the gate above does not see it: every non-test connect is on this list,
    /// and each entry says why it is fine on a copy. A new one must be added here by a reviewer, not by accident.
    #[test] fn every_outbound_socket_is_on_the_allowlist() {
        // presenter.rs: loopback to our own policy child; kicad/nng.rs: the KiCad unix socket; net/tailnet.rs: `hello_with` returns at once on a copy;
        // agents/mcp_stdio.rs: the `--mcp-stdio` bridge an agent starts, loopback only (`endpoint` refuses any other host) to the hub's own MCP server
        const ALLOWED: [&str; 4] = ["presenter.rs", "kicad/nng.rs", "net/tailnet.rs", "agents/mcp_stdio.rs"];
        fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) { for e in std::fs::read_dir(d).unwrap().flatten() { let p = e.path(); if p.is_dir() { if p.file_name().map_or(false, |n| n == "bin") { continue } walk(&p, out) } else if p.extension().map_or(false, |x| x == "rs") { out.push(p) } } }
        let mut files = vec![]; walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut bad = vec![];
        for f in files {
            let name = f.to_string_lossy().replace('\\', "/");
            if name.ends_with("tests.rs") || name.ends_with("e2e.rs") || name.contains("/tests/") || ALLOWED.iter().any(|a| name.ends_with(a)) { continue }
            let text = std::fs::read_to_string(&f).unwrap();
            let code = text.split("\n#[cfg(test)]").next().unwrap_or(&text);
            for (i, l) in code.lines().enumerate() {
                let l = l.trim_start(); if l.starts_with("//") { continue }
                if ["TcpStream::connect", "UdpSocket", "TcpSocket", "connect_async", "UnixStream::connect"].iter().any(|k| l.contains(k)) { bad.push(format!("{name}:{}", i + 1)) }
            }
        }
        assert!(bad.is_empty(), "an outbound socket outside the allowlist (add it to proc.rs's list with a copy-guard): {bad:?}");
    }
}
