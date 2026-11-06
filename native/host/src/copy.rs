//! A hub on a copy of a real log (`BACKPLANE_COPY=1`): the safety gate. Ported from src/core/copy.bend and
//! src/core/copynet.bend (laws `copy_*`, `watch_copy_*`, `adopt_copy_none`). A copied log names real work, real
//! folders and real machines, so a copy acts on nothing outside its home: no watched work is launched, re-launched
//! or signalled (every watch reads as gone), no deploy, update or restart, no pushes, no far pushes or launch
//! reports to the owner's other machines, no git that writes, no webhook or bot replies, no network command at all,
//! agents only as stand-ins. One place decides; the hub, the agents and the files streams ask it.
//!
//!   * `on()` is the switch (env `BACKPLANE_COPY` equal to "1", read once).
//!   * `rpc_refused(method)` is checked by `Hub::request` before any handler runs: git that writes is told, not run.
//!   * `refused(cmd, args)` is checked by every process runner that can reach the network (`files::git::proc`).
//!   * `watch_argv` is the gate every watch script (launch, check, stop, intent) composes with.
//!   * `out_fate` is `Copy.out` for the outs a hub makes.
use std::sync::OnceLock;

/// Copy.on
pub fn is_on(v: &str) -> bool { v == "1" }
/// The switch, read once at start.
pub fn on() -> bool { static ON: OnceLock<bool> = OnceLock::new(); *ON.get_or_init(|| is_on(&std::env::var("BACKPLANE_COPY").unwrap_or_default())) }

/// Side-by-side mode (docs/gpui/SIDE-BY-SIDE.md): a copy on which the person's own `turn.start` may start a REAL agent (`proc::Kind::PersonTurn`). Needs `BACKPLANE_COPY=1`
/// as well; the flag alone allows nothing. Read once.
pub fn person_turn_allowed() -> bool { static ON: OnceLock<bool> = OnceLock::new(); *ON.get_or_init(|| on() && side_by_side_requested()) }
/// `BACKPLANE_SIDE_BY_SIDE=1` as given (exactly "1"), whatever else is set; `side_by_side_gate` refuses it without the copy switch.
pub fn side_by_side_requested() -> bool { static ON: OnceLock<bool> = OnceLock::new(); *ON.get_or_init(|| std::env::var("BACKPLANE_SIDE_BY_SIDE").map_or(false, |v| v == "1")) }
/// Checked at start with the gate: the flag without the copy switch is refused, so a live hub can never run in side-by-side mode.
pub fn side_by_side_gate(copy: bool, requested: bool) -> Result<(), String> {
    if requested && !copy { Err("backplane-hub: BACKPLANE_SIDE_BY_SIDE=1 without BACKPLANE_COPY=1: refused (side-by-side runs only on a copy)".to_string()) } else { Ok(()) }
}
/// The state policies run passive in side-by-side mode (they do nothing on their own); otherwise `BACKPLANE_PASSIVE` decides, as before.
pub fn passive_from(person_turn_allowed: bool) -> Option<bool> { person_turn_allowed.then_some(true) }

/// Copy.note: said in the first log lines.
pub const NOTE: &str = "BACKPLANE_COPY=1: a copy of a log; no watched work, deploys, pushes, far pushes, network commands or git that writes; agents are stand-in blocks";
/// What a client that asked for git that writes is told (it would wait for an answer that never comes).
pub const GIT_TOLD: &str = "a hub on a copied log (BACKPLANE_COPY=1) runs no git that writes";

fn has(xs: &[&str], s: &str) -> bool { xs.contains(&s) }

// ---- copynet.bend ------------------------------------------------------------------------------------------

/// Copy.net.git: the git subcommands that reach another machine.
fn net_git(args: &[&str]) -> bool { ["push", "fetch", "pull", "clone", "ls-remote"].iter().any(|s| has(args, s)) }
const NET_CMDS: [&str; 8] = ["curl", "wget", "gh", "ssh", "scp", "rsync", "tailscale", "nc"];

/// Copy.net: a network command, or git's.
pub fn net(cmd: &str, args: &[&str]) -> bool { if cmd == "git" { net_git(args) } else { has(&NET_CMDS, cmd) } }

/// Programs that run other programs: what they run is looked for in their arguments (a script, a command line).
const WRAPPERS: [&str; 9] = ["sh", "bash", "dash", "env", "nice", "timeout", "nohup", "xargs", "setsid"];
fn words(s: &str) -> impl Iterator<Item = &str> { s.split(|c: char| c.is_whitespace() || ";|&()<>\"'`$={}".contains(c)).filter(|w| !w.is_empty()) }

/// Copy.refused: whether a copy hub refuses to run `cmd args`. Beyond Bend's check (which sees the program a Bend
/// job names) this also reads through shell wrappers, since the Rust runners reach git through `sh -c`.
pub fn refused(cmd: &str, args: &[&str]) -> bool {
    if !on() { return false }
    refused_with(true, cmd, args)
}
pub fn refused_with(copy: bool, cmd: &str, args: &[&str]) -> bool {
    if !copy { return false }
    let base = cmd.rsplit('/').next().unwrap_or(cmd);
    if net(base, args) { return true }
    if !has(&WRAPPERS, base) { return false }
    let toks: Vec<&str> = args.iter().flat_map(|a| words(a)).map(|w| w.rsplit('/').next().unwrap_or(w)).collect();
    toks.iter().any(|t| has(&NET_CMDS, t)) || (toks.contains(&"git") && net_git(&toks))
}
/// Copy.refusal
pub fn refusal(cmd: &str) -> String { format!("backplane: a hub on a copied log (BACKPLANE_COPY=1) does not run {cmd}") }

// ---- copy.bend ---------------------------------------------------------------------------------------------

/// Copy.git.reads: git jobs that only read the project. Viewer metadata and exports may populate the copy's own cache.
pub fn git_reads(job: &str) -> bool { has(&["diff", "status", "name", "files", "kicad", "refs", "dir", "mech", "pdf", "boards", "rv.components", "rv.renderAsset"], job) }
/// What a copy tells a client whose request would write into a project folder (outside its home).
pub const HOME_ONLY: &str = "a hub on a copied log (BACKPLANE_COPY=1) writes nothing outside its home";
/// Copy.fx.home: bot effects inside the home (scratch folders, the zone rule).
pub fn fx_home(kind: &str) -> bool { has(&["scratch", "scratch-gone", "zone", "home"], kind) }

/// What a copy does of one out.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Fate {
    Keep,
    Drop,
    /// the client that asked is told (not left waiting) with `GIT_TOLD`
    Told,
    /// a new thread's worktree runs on "/" (no repository), so the server says at once that it works in the project folder
    Nowhere,
}
/// Outs that act outside the home: dropped whole by a copy (Copy.out's first block).
const OUTSIDE: [&str; 24] = ["RunStart", "RunNext", "OwnerMail", "CharterWrite", "LockGuard", "Upgrade", "UpgradeForce", "Restart", "KiCadInstall", "KiCadRetry", "TurnEnded", "IslandSync", "TermRun", "BotReply", "WatchStop", "LaunchFar", "Alerted", "WakePoll", "DeployProbe", "DeployGo", "ShipRun", "ShipNext", "Teach", "FarQueued"];

/// Copy.out: `kind` is the out's constructor name, `job` a Git out's job or a BotFx's kind, `server` whether a Git out
/// is the server's own (client 0).
pub fn out_fate(copy: bool, kind: &str, job: &str, server: bool) -> Fate {
    if !copy { return Fate::Keep }
    if has(&OUTSIDE, kind) { return Fate::Drop }
    match kind {
        "Git" if git_reads(job) => Fate::Keep,
        "Git" if job == "worktree" && server => Fate::Nowhere,
        "Git" => if server { Fate::Drop } else { Fate::Told },
        "BotFx" => if fx_home(job) { Fate::Keep } else { Fate::Drop },
        _ => Fate::Keep,
    }
}

/// The RPC methods that run git that writes or act on the machine (file.open is git job "open"; a release tags and
/// publishes): refused on a copy and told, before any handler runs.
pub fn rpc_refused(method: &str) -> bool { on() && rpc_refused_with(true, method) }
pub fn rpc_refused_with(copy: bool, method: &str) -> bool { copy && has(&["git.act", "git.revert", "file.open", "review.release"], method) }

// ---- watch gate ---------------------------------------------------------------------------------------------

/// Watch.argv.k: every watch script is the gate composed with its planner: on a copy none (no launch, check, stop or
/// intent shell: every watch reads as gone, nothing the log names is signalled).
pub fn watch_argv(copy: bool, xs: Vec<String>) -> Vec<String> { if copy { vec![] } else { xs } }
/// adopt_copy_none: a copy takes over or stops none of the agents its home's records name.
pub fn adopt_entries<T>(copy: bool, xs: Vec<T>) -> Vec<T> { if copy { vec![] } else { xs } }

// ---- the gate on a home --------------------------------------------------------------------------------------

/// A log this large was not made by a test.
pub const BIG_LOG: u64 = 256 << 20;
/// A log this hub did not make (no marker) past this size is someone's data, whatever it holds: a plain truncated copy of a real log has no `machine.id` in it.
pub const UNMARKED_MAX: u64 = 1 << 20;
const SCAN: u64 = 4 << 20;
/// The file beside the log that says this hub made the home (so a restart on its own log is no copy).
pub const MARKER: &str = "hub.owner";

fn contains(hay: &[u8], needle: &[u8]) -> bool { hay.windows(needle.len()).any(|w| w == needle) }

/// Whether the home's log looks like a real hub's (a copy of someone's data) and says why. A symlinked log is always
/// suspect (it points at data that is not in this home). Otherwise a home this hub made (the marker) is its own; any other
/// holding a log of more than `UNMARKED_MAX`, or one with a real hub's own settings (`machine.id` near its start, the daily `tz.checked` near its end), a JS
/// hub's `store.db` or live checkpoints, or a log past `BIG_LOG` is a copy and needs `BACKPLANE_COPY=1`.
pub fn looks_real(home: &std::path::Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let log = home.join("events.jsonl");
    let meta = std::fs::symlink_metadata(&log).ok()?;
    if meta.file_type().is_symlink() { return Some(format!("events.jsonl is a symlink to {}", std::fs::read_link(&log).map(|p| p.display().to_string()).unwrap_or_default())) }
    if home.join(MARKER).exists() { return None }
    if home.join("store.db").exists() || home.join("live-checkpoints").exists() { return Some("the home holds another hub's store.db or live checkpoints".into()) }
    if meta.len() > BIG_LOG { return Some(format!("events.jsonl is {} MiB", meta.len() >> 20)) }
    if meta.len() > UNMARKED_MAX { return Some(format!("events.jsonl is {} KiB and this hub did not make the home", meta.len() >> 10)) }
    let mut f = std::fs::File::open(&log).ok()?;
    let mut head = vec![0u8; SCAN.min(meta.len()) as usize];
    f.read_exact(&mut head).ok()?;
    if contains(&head, b"\"machine.id\"") { return Some("the log holds a hub's machine.id".into()) }
    let from = meta.len().saturating_sub(SCAN);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut tail = Vec::new(); f.read_to_end(&mut tail).ok()?;
    if contains(&tail, b"\"tz.checked\"") { return Some("the log holds a hub's tz.checked".into()) }
    None
}

/// Checked first thing in `main`, before the home is touched: a home that looks real is refused unless the copy switch is on.
pub fn gate(home: &std::path::Path, copy: bool) -> Result<(), String> {
    if copy { return Ok(()) }
    match looks_real(home) {
        None => Ok(()),
        Some(why) => Err(format!("backplane-hub: {} looks like a copy of a real hub's data ({why}). Refusing to run on it without BACKPLANE_COPY=1 (a copy acts on nothing outside its home: no agents but stand-ins, no watched work, no network, no git that writes).", home.display())),
    }
}

/// A home this hub starts on is its own from now on.
pub fn mark(home: &std::path::Path) { let p = home.join(MARKER); if !p.exists() { let _ = std::fs::write(p, format!("backplane-hub {}\n", std::process::id())); } }

#[cfg(test)]
mod tests {
    use super::*;
    // watch_copy_gate_none, watch_live_gate_kept, watch_copy_launch_none, watch_copy_check_none, watch_copy_stop_none, watch_copy_intent_none
    #[test] fn watch_gate_none_on_a_copy_and_the_list_kept_off() {
        let xs = vec!["sh".to_string(), "-c".into(), "kill 1".into()];
        assert!(watch_argv(true, xs.clone()).is_empty()); assert_eq!(watch_argv(false, xs.clone()), xs); assert!(watch_argv(true, vec![]).is_empty());
    }
    // adopt_copy_none
    #[test] fn a_copy_adopts_none() { assert!(adopt_entries(true, vec![1, 2]).is_empty()); assert_eq!(adopt_entries(false, vec![1, 2]), vec![1, 2]); }
    // copy_off_runs_all
    #[test] fn a_hub_that_is_no_copy_runs_every_out() {
        for (k, j) in [("DeployGo", ""), ("Git", "act"), ("BotReply", ""), ("Send", ""), ("BotFx", "send")] { assert_eq!(out_fate(false, k, j, false), Fate::Keep) }
    }
    // copy_drops_outside
    #[test] fn a_copy_drops_deploys_restarts_pushes_and_effects_outside_its_home() {
        for k in ["DeployGo", "ShipRun", "ShipNext", "DeployProbe", "Restart", "Upgrade", "UpgradeForce", "LaunchFar", "Alerted", "IslandSync", "WatchStop", "LockGuard", "BotReply"] { assert_eq!(out_fate(true, k, "", false), Fate::Drop, "{k}") }
        assert_eq!(out_fate(true, "Git", "step", true, ), Fate::Drop);
        assert_eq!(out_fate(true, "BotFx", "send", false), Fate::Drop);
    }
    // copy_keeps_inside, copy_keeps_viewer_reads
    #[test] fn a_copy_keeps_replies_reading_git_and_its_homes_folders() {
        assert_eq!(out_fate(true, "Send", "", false), Fate::Keep);
        for j in ["diff", "boards", "rv.components", "rv.renderAsset", "status", "refs"] { assert_eq!(out_fate(true, "Git", j, false), Fate::Keep, "{j}") }
        assert_eq!(out_fate(true, "BotFx", "scratch", false), Fate::Keep);
    }
    // copy_git_write_told, copy_file_open_told
    #[test] fn a_client_that_asked_for_git_that_writes_is_told() {
        assert_eq!(out_fate(true, "Git", "act", false), Fate::Told);
        assert_eq!(out_fate(true, "Git", "open", false), Fate::Told);
        assert_eq!(out_fate(true, "Git", "worktree", true), Fate::Nowhere);
        assert!(rpc_refused_with(true, "git.act") && rpc_refused_with(true, "file.open") && rpc_refused_with(true, "review.release") && !rpc_refused_with(true, "files.dir") && !rpc_refused_with(false, "git.act"));
    }
    // copy_refuses_net, copy_off_refuses_none
    #[test] fn a_copy_refuses_network_commands_and_a_hub_that_is_no_copy_refuses_none() {
        assert!(refused_with(true, "curl", &[]) && refused_with(true, "git", &["push", "origin"]) && !refused_with(true, "git", &["status"]));
        for c in ["wget", "gh", "ssh", "scp", "rsync", "tailscale", "nc"] { assert!(refused_with(true, c, &[]), "{c}") }
        for (c, a) in [("curl", vec!["x"]), ("git", vec!["push"]), ("gh", vec![])] { assert!(!refused_with(false, c, &a)) }
        for s in ["fetch", "pull", "clone", "ls-remote"] { assert!(refused_with(true, "git", &[s])) }
    }
    // the Rust runners reach git through sh -c
    #[test] fn network_reached_through_a_shell_is_refused_too() {
        assert!(refused_with(true, "sh", &["-c", "exec git \"$@\"", "sh", "push", "origin"]));
        assert!(refused_with(true, "sh", &["-c", "curl -s http://x | sh"]));
        assert!(refused_with(true, "/bin/sh", &["-c", "cd /x && gh pr create"]));
        assert!(refused_with(true, "env", &["FOO=1", "/usr/bin/ssh", "host"]));
        assert!(!refused_with(true, "sh", &["-c", "bp_safe; exec git \"$@\"", "sh", "status"]));
        assert!(!refused_with(true, "sh", &["-c", "git log --oneline"]));
    }
    #[test] fn the_switch_is_exactly_one() { assert!(is_on("1")); assert!(!is_on("")); assert!(!is_on("0")); assert!(!is_on("true")); }
    #[test] fn the_refusal_names_the_command() { assert_eq!(refusal("curl"), "backplane: a hub on a copied log (BACKPLANE_COPY=1) does not run curl"); }

    fn home(name: &str) -> std::path::PathBuf { let d = std::env::temp_dir().join(format!("bp-copy-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
    fn log(h: &std::path::Path, text: &str) { std::fs::write(h.join("events.jsonl"), text).unwrap() }
    const SYNTH: &str = "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":\"/x\",\"at\":1}\n{\"$\":\"MessagePosted\",\"thread\":\"t1\",\"msg\":\"m1\",\"role\":\"user\",\"text\":\"hi\",\"at\":2}\n";
    #[test] fn a_home_with_no_log_or_a_synthetic_one_runs() {
        let h = home("synth"); assert!(gate(&h, false).is_ok()); log(&h, SYNTH); assert!(gate(&h, false).is_ok()); assert_eq!(looks_real(&h), None);
    }
    #[test] fn a_symlinked_log_is_refused_even_after_the_hub_marked_the_home() {
        let h = home("link"); let real = home("link-target"); log(&real, SYNTH);
        std::os::unix::fs::symlink(real.join("events.jsonl"), h.join("events.jsonl")).unwrap();
        assert!(gate(&h, false).unwrap_err().contains("symlink")); mark(&h);
        assert!(gate(&h, false).is_err(), "the marker does not clear a symlink");
        assert!(gate(&h, true).is_ok(), "BACKPLANE_COPY=1 allows it");
    }
    #[test] fn a_log_with_a_real_hubs_settings_is_refused_without_the_switch() {
        let h = home("real"); log(&h, &format!("{SYNTH}{{\"$\":\"SettingSet\",\"key\":\"machine.id\",\"value\":\"m1\",\"at\":3}}\n"));
        let e = gate(&h, false).unwrap_err(); assert!(e.contains("BACKPLANE_COPY=1") && e.contains("machine.id"), "{e}");
        assert!(gate(&h, true).is_ok());
        let t = home("real-tail"); log(&t, &format!("{SYNTH}{{\"$\":\"SettingSet\",\"key\":\"tz.checked\",\"value\":\"1\",\"at\":3}}\n")); assert!(gate(&t, false).is_err());
    }
    // a plain truncated copy of a real log (no machine.id in it) is still someone's data; the hub's own marked home, and a tiny synthetic log, are not
    #[test] fn an_unmarked_log_past_a_megabyte_is_a_copy_whatever_it_holds() {
        let h = home("trunc"); let line = "{\"$\":\"ActivityLogged\",\"thread\":\"t1\",\"act\":\"a\",\"summary\":\"x\",\"at\":1}\n";
        std::fs::write(h.join("events.jsonl"), line.repeat((UNMARKED_MAX as usize / line.len()) + 10)).unwrap();
        let why = gate(&h, false).unwrap_err(); assert!(why.contains("did not make the home"), "{why}");
        assert!(gate(&h, true).is_ok(), "the copy switch still runs it");
        mark(&h); assert!(gate(&h, false).is_ok(), "a home this hub made is its own, however big");
        let s = home("tiny"); log(&s, SYNTH); assert!(gate(&s, false).is_ok());
    }
    #[test] fn a_js_hubs_store_or_a_huge_log_marks_a_copy() {
        let h = home("store"); log(&h, SYNTH); std::fs::write(h.join("store.db"), b"x").unwrap(); assert!(gate(&h, false).is_err());
        let b = home("big"); let f = std::fs::File::create(b.join("events.jsonl")).unwrap(); f.set_len(BIG_LOG + 1).unwrap(); assert!(gate(&b, false).unwrap_err().contains("MiB"));
    }
    #[test] fn a_home_the_hub_made_restarts_without_the_switch() {
        let h = home("own"); log(&h, &format!("{SYNTH}{{\"$\":\"SettingSet\",\"key\":\"machine.id\",\"value\":\"m1\",\"at\":3}}\n"));
        assert!(gate(&h, false).is_err()); mark(&h); assert!(gate(&h, false).is_ok(), "marked: its own log");
    }
    #[test] fn side_by_side_needs_the_copy_switch_and_makes_the_policies_passive() {
        assert!(side_by_side_gate(true, true).is_ok() && side_by_side_gate(true, false).is_ok() && side_by_side_gate(false, false).is_ok());
        assert!(side_by_side_gate(false, true).unwrap_err().contains("without BACKPLANE_COPY=1"), "the flag alone is refused");
        assert_eq!(passive_from(true), Some(true), "side-by-side: passive"); assert_eq!(passive_from(false), None, "otherwise the env decides");
    }
}
