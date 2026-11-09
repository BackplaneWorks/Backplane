//! Setup's checks (`onboard.check`): what is installed, whether it runs, who is signed in. The answer is the info `onboard.checks`, which the pure step list
//! in `present/onboard.rs` reads, kept in the setting `onboard.checks`. `onboard.uber` makes the uberbot.
//! `onboard.view` and `onboard.act` are the phones' way in (data/1 has no presenter).
//! A check only reads: `--version`, `auth status`, `login status`. Nothing here signs in or installs.
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}, proc::{self, Kind}};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(15);
/// FreeCAD's console imports its whole Python stack before it answers.
const SLOW: Duration = Duration::from_secs(45);

/// What a program said: its exit, its stdout and stderr; None when it could not start or ran out of time.
async fn run(kind: Kind, cmd: &Path, args: &[&str], limit: Duration) -> Option<(i32, String, String)> {
    let mut c = proc::tokio_cmd(kind, cmd);
    c.args(args).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let out = tokio::time::timeout(limit, c.output()).await.ok()?.ok()?;
    Some((out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned()))
}

/// run, for FreeCAD: with no display and Qt offscreen (stepglb::headless)
async fn run_headless(cmd: &Path, args: &[&str], limit: Duration) -> Option<(i32, String, String)> {
    let mut c = proc::tokio_cmd(Kind::Local, cmd);
    crate::files::kicad::stepglb::headless(c.as_std_mut());
    c.args(args).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let out = tokio::time::timeout(limit, c.output()).await.ok()?.ok()?;
    Some((out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned()))
}

/// The first line of a version's output, its version number if it has one, within reason.
pub fn version_of(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let word = line.split_whitespace().find(|w| w.chars().next().map_or(false, |c| c.is_ascii_digit()) && w.contains('.'));
    word.unwrap_or(line).trim_matches(|c: char| c == ',' || c == '(' || c == ')').chars().take(40).collect()
}

/// `claude auth status`'s JSON, or the words of `codex login status`: signed in or not, and who.
pub fn claude_signed(code: i32, out: &str) -> (&'static str, String) {
    let v: Value = serde_json::from_str(out).unwrap_or(Value::Null);
    if v["loggedIn"] == true { return ("yes", v["email"].as_str().unwrap_or("").chars().take(80).collect()) }
    if v["loggedIn"] == false || code != 0 { ("no", String::new()) } else { ("unknown", String::new()) }
}
pub fn codex_signed(code: i32, out: &str, err: &str) -> &'static str {
    let t = format!("{out}{err}").to_lowercase();
    if t.contains("not logged in") { "no" } else if code == 0 && t.contains("logged in") { "yes" } else if code != 0 { "no" } else { "unknown" }
}

fn on_path(name: &str) -> Option<PathBuf> { std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|c| c.is_file())) }
fn first_of(typed: &str, setting: &str, env: &str, names: &[&str], places: Vec<PathBuf>) -> Option<PathBuf> {
    for given in [typed, setting] { if !given.is_empty() { return Some(given.into()) } }
    if let Ok(e) = std::env::var(env) { if !e.is_empty() { return Some(e.into()) } }
    names.iter().find_map(|n| on_path(n)).or_else(|| places.into_iter().find(|p| p.is_file()))
}

async fn kicad(hub: &Arc<Hub>, typed: &str) -> Value {
    let set = hub.setting("kicad.path").await.unwrap_or_default();
    let pf = crate::present::onboard::platform(&hub.cfg.os);
    let mut places: Vec<PathBuf> = pf.kicad_places.iter().map(PathBuf::from).collect();
    let managed = crate::files::kicad::ipc::runtime(hub).await.cli; if !managed.is_empty() { places.insert(0, managed.into()); }
    let Some(cli) = first_of(typed, &set, "BACKPLANE_KICAD_CLI", pf.kicad_names, places) else { return json!({"path": "", "runs": false}) };
    let path = cli.to_string_lossy().into_owned();
    match run(Kind::Local, &cli, &["--version"], TIMEOUT).await {
        Some((0, out, _)) => json!({"path": path, "version": version_of(&out), "runs": true}),
        Some((code, _, err)) => json!({"path": path, "runs": false, "why": why(code, &err)}),
        None => json!({"path": path, "runs": false, "why": "it did not start or did not answer"}),
    }
}
fn why(code: i32, err: &str) -> String { let l = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(120).collect::<String>(); if l.is_empty() { format!("exit code {code}") } else { format!("exit code {code}, {l}") } }

async fn freecad(hub: &Arc<Hub>, typed: &str) -> Value {
    let set = hub.setting("freecad.path").await.unwrap_or_default();
    let found = crate::files::kicad::stepglb::freecad_cmd(&hub.cfg.home);
    let places: Vec<PathBuf> = if found.is_empty() { vec![] } else { vec![found.into()] };
    let Some(cmd) = first_of(typed, &set, "BACKPLANE_FREECAD", crate::present::onboard::platform(&hub.cfg.os).freecad_names, places) else { return json!({"path": "", "runs": false}) };
    let path = cmd.to_string_lossy().into_owned();
    // headless, and the AppImage's console program: a GUI asked for --version shows a box on the person's screen
    let mut args: Vec<&str> = crate::files::kicad::stepglb::console_args(&path).to_vec(); args.push("--version");
    match run_headless(&cmd, &args, SLOW).await {
        Some((0, out, _)) => json!({"path": path, "version": version_of(&out), "runs": true}),
        Some((code, _, err)) => json!({"path": path, "runs": false, "why": why(code, &err)}),
        None => json!({"path": path, "runs": false, "why": "it did not start or did not answer"}),
    }
}

async fn agents(cli: &crate::agents::proc::Cli) -> Value {
    let mut m = serde_json::Map::new();
    for a in crate::present::onboard::AGENTS {
        let Ok(found) = cli.resolve(a.id) else { m.insert(a.id.into(), json!({"path": ""})); continue };
        let (p, path) = (found.as_path(), found.to_string_lossy().into_owned());
        let version = run(Kind::Agent, p, &["--version"], TIMEOUT).await.filter(|r| r.0 == 0).map_or(String::new(), |r| version_of(&r.1));
        let (signed, who) = match a.id {
            "claude" => run(Kind::Agent, p, &["auth", "status"], TIMEOUT).await.map_or(("unknown", String::new()), |r| claude_signed(r.0, &r.1)),
            "codex" => (run(Kind::Agent, p, &["login", "status"], TIMEOUT).await.map_or("unknown", |r| codex_signed(r.0, &r.1, &r.2)), String::new()),
            _ => ("unknown", String::new()),
        };
        m.insert(a.id.into(), json!({"path": path, "version": version, "signed": signed, "who": who}));
    }
    Value::Object(m)
}

/// The connected named accounts by provider, each its label, from what the accounts watch publishes.
fn accounts(info: &Value) -> Value {
    let live: Value = info["accounts.live"].as_str().and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null);
    let mut out = json!({"claude": [], "codex": []});
    for p in live["profiles"].as_array().into_iter().flatten().filter(|p| p["state"] == "connected") {
        if let (Some(k), Some(l)) = (p["provider"].as_str(), p["label"].as_str()) { if let Some(a) = out[k].as_array_mut() { a.push(json!(l)); } }
    }
    out
}

/// The session a phone's request stands for: its step, nothing else (data/1 has no view session).
fn phone_session(p: &Value) -> Value { json!({"rid": 1, "drafts": {"@ob.step": p.get("step").and_then(Value::as_str).unwrap_or("")}, "open": {}}) }

pub struct OnboardRpc;
impl OnboardRpc {
    async fn check(hub: &Arc<Hub>, p: &Value) {
        let g = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let info = hub.info_now();
        let cli = crate::agents::proc::Cli::from_env();
        let (typed_k, typed_f) = (g("kicad"), g("freecad"));
        let (a, k, f) = tokio::join!(agents(&cli), kicad(hub, &typed_k), freecad(hub, &typed_f));
        let (winget, node) = Self::have(&hub.cfg.os).await;
        let devices = crate::net::start::routes().map_or(0, |r| r.reg.list()["devices"].as_array().map_or(0, |d| d.iter().filter(|x| x["revoked"] != true && x["revoked"] != "true").count()));
        let exe = std::env::current_exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let on_path = on_path(crate::present::onboard::platform(&hub.cfg.os).launcher).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let checks = json!({"exe": exe, "onPath": on_path, "agents": a, "accounts": accounts(&info), "kicad": k, "freecad": f, "devices": devices, "tailscale": info["tailscale"], "winget": winget, "node": node});
        // a path typed here goes where FreeCAD is looked for, then Settings, CAD reads the same machine
        if checks["freecad"]["runs"] == true && !g("freecad").is_empty() { let d = hub.cfg.home.join("freecad"); let _ = std::fs::create_dir_all(&d); let _ = std::fs::write(d.join("command"), g("freecad")); }
        crate::files::kicad::ipc::publish_cad(hub).await;
        Self::auto_kicad(hub).await;
        // a setting, not info: its log line redraws every client, and the answer outlives a restart. Only a change is logged.
        let text = checks.to_string();
        if hub.setting("onboard.checks").await.as_deref() != Some(text.as_str()) { let _ = hub.commit(&json!({"$":"SettingSet","key":"onboard.checks","value":text,"at":now_ms() / 1000})).await; }
    }
    /// Whether winget (Windows only) and Node answer: Windows' buttons and the Codex fallback read them.
    async fn have(os: &str) -> (bool, bool) {
        let w = async { os == "windows" && run(Kind::Local, Path::new("winget"), &["--version"], TIMEOUT).await.map_or(false, |r| r.0 == 0) };
        let n = async { run(Kind::Local, Path::new("node"), &["--version"], TIMEOUT).await.map_or(false, |r| r.0 == 0) };
        tokio::join!(w, n)
    }
    /// `onboard.install {tool}`: the press on a tool's Install button.
    async fn install(hub: &Arc<Hub>, tool: &str) -> Result<String, String> {
        let checks: Value = hub.setting("onboard.checks").await.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        super::tools_install::start(hub, tool, checks["winget"] == true, checks["node"] == true, |h| Box::pin(async move { OnboardRpc::check(&h, &json!({})).await })).await
    }
    /// On the KiCad step with no KiCad found and a package for this computer, the install starts by itself, once per version (a failure shows Try again
    /// and is not retried): the person reached the step, the step says what is downloaded, and the install checks the digest and stages as before.
    async fn auto_kicad(hub: &Arc<Hub>) {
        use crate::files::kicad::install::{decide, package, Action};
        if hub.setting("onboard.step").await.as_deref() != Some("kicad") { return }
        let info = hub.info_now();
        let state: Value = serde_json::from_str(info["kicad.install"].as_str().unwrap_or("")).unwrap_or_default();
        let Some(p) = package() else { return };
        if decide(Some(p), info["kicad.status"].as_str().unwrap_or(""), state["state"].as_str().unwrap_or("")) != Action::Auto { return }
        if hub.setting("onboard.kicad.auto").await.as_deref() == Some(p.version) { return }
        let _ = hub.commit(&json!({"$":"SettingSet","key":"onboard.kicad.auto","value":p.version,"at":now_ms() / 1000})).await;
        if let Err(why) = crate::files::kicad::install::install(hub, true).await { eprintln!("setup: KiCad install not started: {why}"); }
    }
    async fn inner(hub: &Arc<Hub>, who: &Caller, m: &'static str, p: Value) -> Value {
        let Some(h) = hub.rpc_for(m) else { return json!({"ok": false, "error": "not available on this hub"}) };
        h.call(hub, who, &json!(0), m, &p).await.into_iter().next().unwrap_or(Value::Null)
    }
    /// The uberbot for a home with no bots: Bub, or a name that is free.
    async fn uber(hub: &Arc<Hub>, who: &Caller) {
        for name in ["Bub", "Bub2", "Bub3"] { if Self::inner(hub, who, "bots.create", json!({"name": name, "uber": true})).await["ok"] == true { break } }
    }
    /// A phone's turn: the step for `step`, or a press on it, carried out with this caller's rights. Setup's requests go where a view's would.
    async fn phone(hub: &Arc<Hub>, who: &Caller, p: &Value, act: bool) -> Value {
        use crate::present::onboard as ob;
        let Some(pol) = crate::statehost::policies() else { return json!({"tour": null}) };
        let (session, info, c) = (phone_session(p), hub.info_now(), ob::Client { phone: true, os: String::new(), launcher: false });
        let home = ob::home_dir();
        let mut out = session.clone();
        if act {
            let (a, v) = (p["action"].as_str().unwrap_or(""), p["value"].as_str().unwrap_or(""));
            let done = pol.with_state(|st| ob::act(st, "", &session, &info, &c, &home, a, v));
            if let Some(Some(o)) = done {
                crate::session::patch::merge(&mut out, &o.patch);
                for cmd in &o.cmds { if let Ok(r) = serde_json::from_str::<Value>(cmd["text"].as_str().unwrap_or("")) { if let Some(h) = r["m"].as_str().and_then(|m| hub.rpc_for(m)) { let m = r["m"].as_str().unwrap_or(""); h.call(hub, who, &r["id"], m, &r["p"]).await; } } }
            }
        }
        let tour = pol.with_state(|st| if p.get("step").is_none() && !act { ob::view(st, &out, &info, &c, &home) } else if out["drafts"]["@ob.closed"] == "1" || (act && out["drafts"]["@ob.step"] == "" && p["action"] != "ob-open") { None } else { Some(ob::page(st, &out, &info, &c, &home)) }).flatten();
        json!({"tour": tour.unwrap_or(Value::Null)})
    }
}
impl RpcHandler for OnboardRpc {
    fn methods(&self) -> &'static [&'static str] { &["onboard.check", "onboard.install", "onboard.uber", "onboard.view", "onboard.act"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let ok = json!({"t":"reply","id":id,"ok":true});
            match method {
                "onboard.check" => { let (h, q) = (hub.clone(), p.clone()); tokio::spawn(async move { OnboardRpc::check(&h, &q).await }); vec![ok] }
                "onboard.install" => match OnboardRpc::install(hub, p["tool"].as_str().unwrap_or("")).await { Ok(m) => vec![json!({"t":"reply","id":id,"ok":true,"result":{"message":m}})], Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})] },
                "onboard.uber" => { OnboardRpc::uber(hub, who).await; vec![ok] }
                _ => vec![json!({"t":"reply","id":id,"ok":true,"result":OnboardRpc::phone(hub, who, p, method == "onboard.act").await})],
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn versions_are_read_from_each_programs_first_line() {
        assert_eq!(version_of("10.0.7\n"), "10.0.7");
        assert_eq!(version_of("2.1.268 (Claude Code)\n"), "2.1.268");
        assert_eq!(version_of("FreeCAD 1.0.2 Revision: 39319 (Git)\n"), "1.0.2");
        assert_eq!(version_of("\n\ncodex-cli 0.9.1\n"), "0.9.1");
        assert_eq!(version_of(""), "");
    }
    #[test] fn claude_status_needs_its_own_answer() {
        assert_eq!(claude_signed(0, r#"{"loggedIn":true,"email":"a@b.c"}"#), ("yes", "a@b.c".to_string()));
        assert_eq!(claude_signed(1, r#"{"loggedIn":false}"#).0, "no");
        assert_eq!(claude_signed(0, "something else").0, "unknown", "a program that runs is not a sign-in");
    }
    #[test] fn codex_status_reads_its_words() {
        assert_eq!(codex_signed(0, "Logged in using ChatGPT\n", ""), "yes");
        assert_eq!(codex_signed(1, "", "Not logged in\n"), "no");
        assert_eq!(codex_signed(0, "hello", ""), "unknown");
    }
    #[test] fn a_typed_path_wins_and_a_setting_comes_next() {
        assert_eq!(first_of("/a", "/b", "BACKPLANE_NOPE_ENV", &[], vec![]), Some("/a".into()));
        assert_eq!(first_of("", "/b", "BACKPLANE_NOPE_ENV", &[], vec![]), Some("/b".into()));
        assert_eq!(first_of("", "", "BACKPLANE_NOPE_ENV", &["no-such-program-here"], vec![]), None);
    }
    #[test] fn only_connected_accounts_count() {
        let live = json!({"accounts.live": json!({"profiles":[{"provider":"claude","label":"Work","state":"connected"},{"provider":"claude","label":"Old","state":"disconnected"},{"provider":"codex","label":"Me","state":"connected"}]}).to_string()});
        assert_eq!(accounts(&live), json!({"claude":["Work"],"codex":["Me"]}));
    }
}
