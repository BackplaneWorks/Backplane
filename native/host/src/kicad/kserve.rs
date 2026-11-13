//! Rust twin of src/core/kserve: the pure policy of the hub's headless KiCad servers (one `kicad-cli api-server` per
//! project, at most two): where a server's socket lives, its command line, reading its ready line, the version check, the
//! 1/5/30 s restart schedule, which server a third project retires, the 18 commands a headless KiCad refuses, the bp.3
//! guards and the words shown. No IO and no process handling here (ipc.rs owns that); nothing signals a process.
//! In the final design these rules stay and run in the rules process; until it is wired this twin carries them, and each
//! test below names def (and the `kserve_*` law) it states. Times are epoch seconds like KS.* (Nat.sub saturates).
use serde_json::Value;

/// One project's server (KSrv). state: "starting", "ready", "failed" or "stopped".
#[derive(Clone, Debug, PartialEq)]
pub struct Srv { pub project: String, pub sock: String, pub state: String, pub pid: u32, pub started: u64, pub last: u64, pub fails: Vec<u64>, pub why: String, pub version: String }

pub const WINDOW: u64 = 300; // KS.window
pub const MIN_VERSION: &str = "10.99.0-bp.4"; // KH.min_version
pub const MIN_API: u64 = 2; // KH.min_api
pub const HELD: &str = "unsaved changes"; // KS.held
pub const FP_CAP: &str = "footprint-partial-update";
pub const SYM_CAP: &str = "symbol-from-libid";
pub const FP_TYPE: &str = "kiapi.board.types.FootprintInstance";
pub const SYM_TYPE: &str = "kiapi.schematic.types.SchematicSymbolInstance";

fn has(xs: &[String], w: &str) -> bool { xs.iter().any(|x| x == w) } // KS.has
fn last(p: &str, sep: char) -> &str { p.rsplit(sep).next().unwrap_or(p) } // KS.last

// Where a server lives -------------------------------------------------------------------------------------------------

fn hash(s: &str, m: u64, acc0: u64) -> u64 { s.chars().fold(acc0, |acc, c| (acc.wrapping_mul(m).wrapping_add(c as u64)) % (1u64 << 32)) } // KS.hash
fn hex(w: usize, n: u64) -> String { let mut o = String::new(); let mut n = n; for _ in 0..w { o.insert(0, std::char::from_digit((n % 16) as u32, 16).unwrap()); n /= 16 } o } // KS.hex
/// KS.digest: 16 hex digits of a project's absolute path.
pub fn digest(project: &str) -> String { format!("{}{}", hex(8, hash(project, 31, 7)), hex(8, hash(project, 131, 3))) }
/// KS.root
pub fn root(runtime_dir: &str, home: &str) -> String { if runtime_dir.is_empty() { format!("{home}/run/kicad/") } else { format!("{runtime_dir}/backplane/kicad/") } }
/// KS.dir: the project's own directory (mode 0700): its socket and lock file are the server's alone.
pub fn dir(runtime_dir: &str, home: &str, project: &str) -> String { format!("{}{}", root(runtime_dir, home), digest(project)) }

// Starting one ---------------------------------------------------------------------------------------------------------

/// KS.desc: the runtime's API schema only when it ships one ("api-descriptor"), else "".
pub fn desc(root: &str, caps: &[String]) -> String { if has(caps, "api-descriptor") { format!("{root}/share/backplane/api.desc") } else { String::new() } }
/// KS.caps.safe: --idle-exit is safe only from API 2 on.
pub fn caps_safe(xs: &[String], api: u64) -> Vec<String> { if api >= 2 { xs.to_vec() } else { xs.iter().filter(|x| *x != "idle-exit").cloned().collect() } }
/// KS.launcher: the runtime's bin/kicad-cli, never `kicad-cli.real` (only the launcher sets KICAD10_*_DIR).
pub fn launcher(cli: &str) -> String { if cli.ends_with("kicad-cli.real") { cli[..cli.len() - 5].to_string() } else { cli.to_string() } }
/// KS.argv: exits by itself when the hub is gone (--parent-pid), and after ten idle minutes when the runtime can.
pub fn argv(cli: &str, pro: &str, dir: &str, hub_pid: u32, caps: &[String]) -> Vec<String> {
    let mut v: Vec<String> = [launcher(cli), "api-server".into(), pro.into(), "--socket".into(), format!("{dir}/api.sock"), "--parent-pid".into(), hub_pid.to_string()].into();
    if has(caps, "idle-exit") { v.push("--idle-exit".into()); v.push("600".into()) }
    v
}
const READY: &str = "KiCad API server listening at ";
/// KS.ready: the socket a server printed when it started listening (may be api-<pid>.sock).
pub fn ready(line: &str) -> Option<String> {
    let rest = line.strip_prefix(READY)?.trim();
    let p = rest.strip_prefix("ipc://").unwrap_or(rest);
    if p.is_empty() { None } else { Some(p.to_string()) }
}
/// KS.suffix: what a build's "-bp.N" is.
pub fn suffix(m: &str) -> String { let l = last(m, '-'); if l.starts_with("bp.") { format!("-{l}") } else { m.to_string() } }
/// KS.version_ok: the running server's full version ends with the manifest's -bp.N.
pub fn version_ok(full: &str, manifest: &str) -> bool { full.ends_with(&suffix(manifest)) }

// Crashes --------------------------------------------------------------------------------------------------------------

/// KS.keep: the failures of the last 300 s.
pub fn keep(fs: &[u64], now: u64) -> Vec<u64> { fs.iter().copied().filter(|f| now.saturating_sub(*f) < WINDOW).collect() }
pub fn recent(fs: &[u64], now: u64) -> usize { keep(fs, now).len() } // KS.recent
/// KS.delay / KS.retry: seconds to wait after the failure just recorded: 1, 5, 30 for the first, second, third within 300 s; None after the fourth.
pub fn retry(fails: &[u64], now: u64) -> Option<u64> { match recent(fails, now) { 0 | 1 => Some(1), 2 => Some(5), 3 => Some(30), _ => None } }
/// KS.fails.add: a failure is recorded unless the server left cleanly (code 0: idle-exit or Shutdown).
pub fn fails_add(code: i32, fs: &[u64], now: u64) -> Vec<u64> { if code == 0 { fs.to_vec() } else { let mut v = vec![now]; v.extend(keep(fs, now)); v } }

// The records ----------------------------------------------------------------------------------------------------------

impl Srv {
    /// KS.new: a server asked for just now.
    pub fn new(project: &str, now: u64) -> Srv { Srv { project: project.into(), sock: String::new(), state: "starting".into(), pid: 0, started: now, last: now, fails: vec![], why: String::new(), version: String::new() } }
    /// A project with no record (KServe.get): stopped.
    pub fn none(project: &str) -> Srv { Srv::new(project, 0).set("stopped", "") }
    pub fn set(mut self, state: &str, why: &str) -> Srv { self.state = state.into(); self.why = why.into(); self } // KS.set
    /// KS.touch: asked for again at this time (a held server is released).
    pub fn touch(mut self, now: u64) -> Srv { self.last = now; if self.why == HELD { self.why.clear() } self }
    /// KS.hold: it refused Shutdown: keep it, never force it.
    pub fn hold(mut self) -> Srv { self.why = HELD.into(); self }
    /// KS.up: it answered Ping and GetVersion.
    pub fn up(mut self, sock: &str, version: &str, now: u64) -> Srv { self.sock = sock.into(); self.state = "ready".into(); self.last = now; self.why.clear(); self.version = version.into(); self }
    /// KS.starting: started again; no socket yet; the failures stay on record.
    pub fn starting(mut self, now: u64) -> Srv { self.sock.clear(); self.state = "starting".into(); self.started = now; self.last = now; self.why.clear(); self }
    pub fn listening(mut self, sock: &str) -> Srv { self.sock = sock.into(); self } // KS.listening
    /// KS.cleared: Retry: failures forgotten, left stopped so the next request starts it.
    pub fn cleared(mut self) -> Srv { self.state = "stopped".into(); self.fails.clear(); self.why.clear(); self }
    /// KS.exited: the process ended with this code: stopped on a clean exit, else the crash is recorded and it waits to start again ("starting") or gives up ("failed").
    pub fn exited(mut self, code: i32, now: u64, tail: &str) -> Srv {
        self.fails = fails_add(code, &self.fails, now);
        let state = if code == 0 { "stopped" } else if retry(&self.fails, now).is_some() { "starting" } else { "failed" };
        self.why = match state { "failed" => format!("stopped four times in five minutes: {tail}"), "starting" => format!("restarting after a crash: {tail}"), _ => String::new() };
        self.state = state.into(); self
    }
}

// Room for two ---------------------------------------------------------------------------------------------------------

pub fn active(st: &str) -> bool { st == "starting" || st == "ready" } // KS.active
pub fn count(xs: &[Srv]) -> usize { xs.iter().filter(|s| active(&s.state)).count() } // KS.count
/// KS.evict: with two servers starting or ready and a third wanted: the project of the least recently used ready one ("" when none can be retired; a held one never is).
pub fn evict(srvs: &[Srv]) -> String {
    if count(srvs) < 2 { return String::new() }
    let mut best: Option<(&str, u64)> = None;
    for s in srvs { if s.state == "ready" && s.why != HELD && best.map_or(true, |(_, at)| s.last < at) { best = Some((&s.project, s.last)) } }
    best.map(|(p, _)| p.to_string()).unwrap_or_default()
}
/// KS.state.of: a project's state ("" when no record); the first record wins.
pub fn state_of(xs: &[Srv], project: &str) -> String { xs.iter().find(|s| s.project == project).map(|s| s.state.clone()).unwrap_or_default() }
/// KS.plan: "ready", "wait", "failed", "start", "evict:<project>" or "busy".
pub fn plan(srvs: &[Srv], project: &str, caps: &[String]) -> String {
    match state_of(srvs, project).as_str() {
        "ready" => "ready".into(), "starting" => "wait".into(), "failed" => "failed".into(),
        _ => if count(srvs) < 2 { "start".into() } else { let v = evict(srvs); if has(caps, "shutdown") && !v.is_empty() { format!("evict:{v}") } else { "busy".into() } },
    }
}

// Commands a headless server cannot answer -----------------------------------------------------------------------------

/// KS.refused: the 18 editor commands upstream's headless api-server answers with AS_UNIMPLEMENTED.
pub const REFUSED: [&str; 18] = ["GetSelection", "AddToSelection", "RemoveFromSelection", "ClearSelection", "SaveSelectionToString", "SyncSelection", "FocusOnItems",
    "GetVisibleLayers", "SetVisibleLayers", "GetActiveLayer", "SetActiveLayer", "GetBoardEditorAppearanceSettings", "SetBoardEditorAppearanceSettings",
    "HighlightNets", "RunAction", "RevertDocument", "OpenLibraryItem", "InteractiveMoveItems"];
pub fn headless_refused(ty: &str) -> bool { REFUSED.contains(&last(ty, '.')) } // KS.headless_refused
pub const REFUSAL: &str = "not available in headless KiCad: pass explicit item IDs (get them with GetItems)"; // KS.refusal
pub const BUSY: &str = "IPC busy: 2 KiCad servers already run for other projects"; // KS.busy
pub const FP_REFUSAL: &str = "bp.3 would wipe this footprint's pads and 3D models: send the full FootprintInstance from GetItems with only the fields you change";
pub const SYM_REFUSAL: &str = "bp.3 would make an empty symbol from a library id: call LoadAllLibraries, wait until GetLibraryStatuses shows the library loaded, then use PlaceSymbolFromLibrary";

fn defined(v: Option<&Value>) -> bool { !matches!(v, None | Some(Value::Null)) } // KS.defined
fn tyof(it: &Value) -> &str { it.get("@type").and_then(Value::as_str).unwrap_or("") }
fn bare(it: &Value, want: &str) -> bool { tyof(it).ends_with(want) && !defined(it.get("definition")) } // KS.bare
fn items(msg: &Value) -> &[Value] { msg.get("items").and_then(Value::as_array).map(|v| v.as_slice()).unwrap_or(&[]) }
fn sym_library_only(it: &Value) -> bool { // KS.sym.library_only
    tyof(it).ends_with(SYM_TYPE) && defined(it.get("definition").and_then(|d| d.get("id"))) && items(it.get("definition").unwrap_or(&Value::Null)).is_empty()
}
/// KS.guard: why the hub refuses a command at once ("" when it goes through): bp.3 runtimes wipe a footprint updated without its definition and make hollow symbols from a library id.
pub fn guard(ty: &str, msg: &Value, caps: &[String]) -> String {
    if ty == "kiapi.common.commands.UpdateItems" && !has(caps, FP_CAP) && items(msg).iter().any(|it| bare(it, FP_TYPE)) { return FP_REFUSAL.into() }
    if ty == "kiapi.common.commands.CreateItems" && !has(caps, SYM_CAP) && items(msg).iter().any(|it| bare(it, SYM_TYPE) || sym_library_only(it)) { return SYM_REFUSAL.into() }
    String::new()
}

// Words ----------------------------------------------------------------------------------------------------------------

/// KS.why.health: why IPC cannot be used from the runtime's health ("" when it can).
pub fn why_health(status: &str, version: &str, why: &str) -> String {
    match status {
        "missing" => "KiCad is not installed".into(),
        "stock" => format!("this is stock KiCad {version}, not Backplane KiCad"),
        "old" => format!("Backplane KiCad is too old ({version}, needs {MIN_VERSION})"),
        "broken" => format!("KiCad doesn't run: {why}"),
        "checking" => "Backplane is still checking KiCad".into(),
        _ => String::new(),
    }
}
/// KS.gate: why IPC is not used right now ("" when it is): the setting is off, else the runtime's health.
pub fn gate(status: &str, version: &str, why: &str, setting: &str) -> String { if setting == "off" { "IPC is turned off in Settings".into() } else { why_health(status, version, why) } }
/// KS.unavailable: an error that names the way round: edit the files.
pub fn unavailable(why: &str, cli: &str) -> String { format!("KiCad IPC unavailable: {why}. Fallback: edit the file directly or use `{} pcb …`; re-read the file after.", if cli.is_empty() { "kicad-cli" } else { cli }) }
pub fn activity(why: &str) -> String { format!("KiCad: fell back to files: {why}") } // KS.activity
fn clean(s: &str) -> String { s.chars().map(|c| if c == '\n' || c == '\r' || c == '|' { ' ' } else { c }).collect() } // KS.clean
/// KS.line: "project|state|why" (no pids); KS.lines: the info message's "kicad.ipc.servers".
pub fn line(s: &Srv) -> String { format!("{}|{}|{}", s.project, s.state, clean(&s.why.chars().take(160).collect::<String>())) }
pub fn lines(xs: &[Srv]) -> String { xs.iter().map(line).collect::<Vec<_>>().join("\n") }
/// KS.name: a project's name: its .kicad_pro without the folder and extension.
pub fn name(p: &str) -> String { let n = last(p, '/'); n.strip_suffix(".kicad_pro").unwrap_or(n).to_string() }
/// KS.note: what Settings says under the KiCad IPC row ("" when no server has run).
pub fn note(text: &str) -> String {
    let parts: Vec<String> = text.lines().map(|l| { let ps: Vec<&str> = l.split('|').collect(); let (p, st, w) = (ps.first().copied().unwrap_or(""), ps.get(1).copied().unwrap_or(""), ps.get(2).copied().unwrap_or(""));
        if p.is_empty() { String::new() } else { format!("KiCad server: {st} for {}{}", name(p), if w.is_empty() { String::new() } else { format!(": {w}") }) } }).collect();
    parts.join(" · ").chars().take(200).collect()
}
/// KH.status: the runtime's health from what was found.
pub fn status(found: bool, runs: bool, fork: bool, version: &str, api: u64) -> &'static str {
    if !found { "missing" } else if !runs { "broken" } else if !fork { "stock" } else if version_ge(version, MIN_VERSION) && api >= MIN_API { "ok" } else { "old" }
}
fn num(s: &str) -> u64 { s.chars().take_while(|c| c.is_ascii_digit()).fold(0, |n, c| n * 10 + c.to_digit(10).unwrap() as u64) } // KH.num
/// The `-bp.N` part, with a hotfix's further parts (`-bp.12.1` is [12, 1]; trailing zeros dropped, so 12.0 is 12): 12.1 orders after 12 and before 13.
/// A test build's suffix after it (`-bp.12-851cbefda6`) is not a part: that build reads as bp.12. A `git describe` name has two
/// (`10.99.0-bp.7-15-g8d903542-dirty-bp.13`): the last one is the build's, bp.13. None: [0].
/// The one reader of a Backplane KiCad's number: everything that gates on it (bp_number) uses this, never its own parser.
pub fn bp(v: &str) -> Vec<u64> { if let Some(n) = v.split('-').rev().find_map(|l| l.strip_prefix("bp.")) { let mut v: Vec<u64> = n.split('.').map(num).collect(); while v.len() > 1 && v.last() == Some(&0) { v.pop(); } v } else { vec![0] } } // KH.bp
/// The `bp.` number alone (13 for bp.13, bp.13.1 and a bp.13 test build; 0 for none), for gates that go by release.
pub fn bp_number(v: &str) -> u64 { bp(v)[0] }
pub fn version_ge(v: &str, min: &str) -> bool { // KH.version.ge
    // a release tag or describe name ("backplane-kicad-v10.99.0-...") reads as its version
    let t = |s: &str| -> String { s.strip_prefix("backplane-kicad-v").unwrap_or(s).to_string() }; let (v, min) = (&t(v)[..], &t(min)[..]);
    let p = |s: &str, i: usize| s.split('.').nth(i).map_or(0, num);
    let (a, b) = ((p(v, 0), p(v, 1), p(v, 2), bp(v)), (p(min, 0), p(min, 1), p(min, 2), bp(min)));
    a >= b
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn s(x: &[&str]) -> Vec<String> { x.iter().map(|y| y.to_string()).collect() }
    fn up(p: &str, last: u64) -> Srv { Srv::new(p, 1).up(&format!("/s/{p}"), "v", last) }

    #[test]
    fn a_hotfix_orders_after_its_release_and_before_the_next() {
        assert!(version_ge("10.99.0-bp.12.1", "10.99.0-bp.12") && !version_ge("10.99.0-bp.12", "10.99.0-bp.12.1"));
        assert!(version_ge("10.99.0-bp.13", "10.99.0-bp.12.1") && !version_ge("10.99.0-bp.12.1", "10.99.0-bp.13"));
        assert!(version_ge("10.99.0-bp.12.1", "10.99.0-bp.12.1") && version_ge("10.99.0-bp.12.1", "10.99.0-bp.8") && version_ge("10.99.0-bp.10", "10.99.0-bp.4"));
        assert!(version_ge("10.99.0-bp.12.0", "10.99.0-bp.12") && version_ge("10.99.0-bp.12", "10.99.0-bp.12.0"));
        // a test build of a release reads as that release: under its hotfix and the next release, over the one before, never as bp.0
        let t = "10.99.0-bp.12-851cbefda6";
        assert!(version_ge("10.99.0-bp.12.1", t) && !version_ge(t, "10.99.0-bp.12.1") && version_ge("10.99.0-bp.13", t) && !version_ge(t, "10.99.0-bp.13"));
        assert!(version_ge(t, "10.99.0-bp.12") && version_ge("10.99.0-bp.12", t) && version_ge(t, "10.99.0-bp.11") && version_ge(t, "10.99.0-bp.4"));
        assert!(version_ge("10.99.0-unknown-bp.3", "10.99.0-bp.3") && !version_ge("10.99.0", "10.99.0-bp.1"));
        // a git-describe name of a bp.13 test build has two "bp." parts: the last is the build's
        for d in ["10.99.0-bp.7-15-g8d903542-dirty-bp.13", "backplane-kicad-v10.99.0-bp.7-15-g8d903542-dirty-bp.13"] {
            assert_eq!(bp_number(d), 13, "{d}");
            assert!(version_ge(d, "10.99.0-bp.13") && version_ge("10.99.0-bp.13", d) && version_ge(d, "10.99.0-bp.12.1") && !version_ge(d, "10.99.0-bp.14"), "{d}");
        }
        assert_eq!((bp_number("10.99.0-bp.12-851cbefda6"), bp_number("10.99.0-bp.12.1"), bp_number("10.99.0-bp.10"), bp_number("10.0.6")), (12, 12, 10, 0));
        assert!(version_ge("backplane-kicad-v10.99.0-bp.12", "10.99.0-bp.8") && !version_ge("backplane-kicad-v10.99.0-bp.4", "10.99.0-bp.8"));
    }

    // KS.argv: kserve_argv_parent_pid_with_idle, _without_idle, _socket_in_dir, _idle_with_cap, _no_idle_without_cap
    #[test] fn kserve_argv() {
        let with = argv("c", "/p/a.kicad_pro", "/d", 0, &s(&["idle-exit"])); let without = argv("c", "/p/a.kicad_pro", "/d", 0, &[]);
        assert!(has(&with, "--parent-pid")); assert!(has(&without, "--parent-pid"));
        assert!(argv("c", "/p/a.kicad_pro", "/d/x", 0, &[])[4].starts_with("/d/x/"));
        assert!(has(&with, "--idle-exit")); assert!(!has(&without, "--idle-exit"));
        assert_eq!(launcher("/r/bin/kicad-cli.real"), "/r/bin/kicad-cli"); assert_eq!(with[0], "c");
    }
    // KS.ready: kserve_ready_line, kserve_ready_other_line
    #[test] fn kserve_ready() {
        assert_eq!(ready("KiCad API server listening at /d/api-7.sock"), Some("/d/api-7.sock".into())); assert_eq!(ready("loading the project"), None);
        assert_eq!(ready("KiCad API server listening at ipc:///d/a.sock"), Some("/d/a.sock".into())); assert_eq!(ready("KiCad API server listening at "), None);
    }
    // KS.version_ok: kserve_version_ok_exact, _ok_describe, _other_build, _longer_build, _not_fork
    #[test] fn kserve_version() {
        assert!(version_ok("10.99.0-bp.1", "10.99.0-bp.1")); assert!(version_ok("10.99.0-unknown-bp.1", "10.99.0-bp.1"));
        assert!(!version_ok("10.99.0-bp.2", "10.99.0-bp.1")); assert!(!version_ok("10.99.0-bp.11", "10.99.0-bp.1")); assert!(!version_ok("10.99.0", "10.99.0-bp.1"));
    }
    // KS.retry: kserve_retry_first, _second, _third, _gives_up, _old_failures_forgotten
    #[test] fn kserve_retry() {
        assert_eq!(retry(&[100], 110), Some(1)); assert_eq!(retry(&[101, 100], 110), Some(5)); assert_eq!(retry(&[102, 101, 100], 110), Some(30));
        assert_eq!(retry(&[103, 102, 101, 100], 110), None); assert_eq!(retry(&[990, 102, 101, 100], 1000), Some(1));
    }
    // KS.fails.add: kserve_clean_exit_not_a_failure, kserve_crash_is_a_failure
    #[test] fn kserve_fails() { assert_eq!(fails_add(0, &[5], 10), vec![5]); assert_eq!(fails_add(1, &[5], 10), vec![10, 5]) }
    // KS.exited: kserve_clean_exit_stops, kserve_three_crashes_restart, kserve_fourth_crash_fails
    #[test] fn kserve_exited() {
        assert_eq!(up("A", 5).exited(0, 20, "").state, "stopped");
        let a = Srv::new("A", 1).exited(1, 100, "x").exited(1, 110, "x").exited(1, 120, "x"); assert_eq!(a.state, "starting"); assert!(a.why.starts_with("restarting after a crash"));
        let f = a.exited(1, 130, "x"); assert_eq!(f.state, "failed"); assert!(f.why.starts_with("stopped four times in five minutes"));
    }
    // KS.evict / KS.plan: kserve_evict_least_recent, kserve_no_evict_with_room, kserve_third_project_evicts, kserve_third_project_busy_without_shutdown, kserve_running_server_is_used
    #[test] fn kserve_room_for_two() {
        let two = [up("A", 5), up("B", 3)];
        assert_eq!(evict(&two), "B"); assert_eq!(evict(&[up("A", 5)]), "");
        assert_eq!(plan(&two, "C", &s(&["shutdown"])), "evict:B"); assert_eq!(plan(&two, "C", &[]), "busy"); assert_eq!(plan(&two, "A", &[]), "ready");
        assert_eq!(plan(&[], "C", &[]), "start"); assert_eq!(count(&two), 2);
        // a server holding unsaved changes is never the victim
        let held = [up("A", 5), up("B", 3).hold()]; assert_eq!(evict(&held), "A"); assert_eq!(touched_releases(), "");
    }
    fn touched_releases() -> String { up("B", 3).hold().touch(9).why }
    // KS.headless_refused / KS.refused: kserve_refused_selection, _refused_view, _refused_run_action, _get_items_allowed, _eighteen_refused
    #[test] fn kserve_refused() {
        assert!(headless_refused("kiapi.common.commands.GetSelection")); assert!(headless_refused("kiapi.board.commands.SetVisibleLayers"));
        assert!(headless_refused("kiapi.common.commands.RunAction")); assert!(!headless_refused("kiapi.board.commands.GetItems")); assert_eq!(REFUSED.len(), 18);
    }
    // KS.gate / KS.unavailable: kserve_gate_off, kserve_gate_ok, kserve_unavailable_names_fallback
    #[test] fn kserve_gate_words() {
        assert_eq!(gate("ok", "10.99.0-bp.1", "", "off"), "IPC is turned off in Settings"); assert_eq!(gate("ok", "10.99.0-bp.1", "", "on"), "");
        assert!(unavailable("x", "c").contains("Fallback: edit the file directly")); assert!(unavailable("x", "").contains("`kicad-cli pcb"));
        assert_eq!(gate("old", "10.0.6", "", "on"), "Backplane KiCad is too old (10.0.6, needs 10.99.0-bp.4)");
    }
    // KH.min_version, KH.status: kicad_min_version, kicad_old_runtime_is_old, kicad_new_runtime_is_ok
    #[test] fn kicad_runtime_status() {
        assert_eq!(MIN_VERSION, "10.99.0-bp.4"); assert_eq!(status(true, true, true, "10.0.6", 1), "old"); assert_eq!(status(true, true, true, "10.99.0-bp.4", 2), "ok");
        assert_eq!(status(true, true, true, "10.99.0-bp.2", 2), "old"); assert_eq!(status(false, true, true, "", 0), "missing"); assert_eq!(status(true, true, false, "10.0.6", 0), "stock");
    }
    // bp.3 guards (no kserve_* law states them; examples only): a bare FootprintInstance / library-only symbol is refused unless the runtime has the capability
    #[test] fn guards() {
        let bare_fp = json!({"items":[{"@type":"type.googleapis.com/kiapi.board.types.FootprintInstance","id":{"value":"x"}}]});
        assert_eq!(guard("kiapi.common.commands.UpdateItems", &bare_fp, &[]), FP_REFUSAL); assert_eq!(guard("kiapi.common.commands.UpdateItems", &bare_fp, &s(&[FP_CAP])), "");
        let lib_sym = json!({"items":[{"@type":"x.kiapi.schematic.types.SchematicSymbolInstance","definition":{"id":{"name":"R"}}}]});
        assert_eq!(guard("kiapi.common.commands.CreateItems", &lib_sym, &[]), SYM_REFUSAL); assert_eq!(guard("kiapi.common.commands.CreateItems", &lib_sym, &s(&[SYM_CAP])), "");
        assert_eq!(guard("kiapi.common.commands.GetItems", &bare_fp, &[]), "");
    }
    #[test] fn digest_and_words() {
        assert_eq!(digest("").len(), 16); assert_eq!(digest("/p/a.kicad_pro"), digest("/p/a.kicad_pro")); assert_ne!(digest("/p/a"), digest("/p/b"));
        assert_eq!(dir("/run/user/1", "/h", "/p"), format!("/run/user/1/backplane/kicad/{}", digest("/p"))); assert!(dir("", "/h", "/p").starts_with("/h/run/kicad/"));
        assert_eq!(name("/a/b/proj.kicad_pro"), "proj"); assert_eq!(line(&Srv::new("/p", 1).set("failed", "a|b\nc")), "/p|failed|a b c");
        assert_eq!(note("/a/x.kicad_pro|failed|boom\n/b/y.kicad_pro|ready|"), "KiCad server: failed for x: boom · KiCad server: ready for y");
        assert_eq!(desc("/r", &s(&["api-descriptor"])), "/r/share/backplane/api.desc"); assert_eq!(desc("/r", &[]), "");
        assert_eq!(caps_safe(&s(&["idle-exit", "shutdown"]), 1), s(&["shutdown"])); assert_eq!(caps_safe(&s(&["idle-exit"]), 2), s(&["idle-exit"]));
    }
}
