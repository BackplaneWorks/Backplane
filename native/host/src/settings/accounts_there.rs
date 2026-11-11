//! Sign in there (docs/ALWAYS-ON.md, section 3): the person, on the client they are using, signs another of their machines (an always-on Boat) in to
//! each provider. The sign-in runs on that machine: each provider's own login command on a pty of its hub (term/), its link, its code and its last lines
//! sent back over the signed far link (net/rpc.rs, owner machines only) as `signin.state` and driven by `signin.act`. Claude also has the sync token,
//! made here and sent to every linked machine by itself (accounts_sync.rs). Account files are never copied between machines: only sync tokens travel.
//!
//! The machine asked (`serve`): `signin.state {fresh?}` -> `{providers, sync}`; `signin.act {provider, action, text?}` with action `start` (its login
//! command), `shell` (a login shell, for a provider with no command of its own), `keys` (a line typed, then Enter), `key` (one of KEYS), `stop`,
//! `install` (tools_install.rs) or `check`, answered with the state after it.
//! The machine asking (`act`, from Settings, Machines and setup through `machines.page.act`): which machine's panel is open, the last answer and the
//! Claude sync token, kept in the info `signin.there` that present/signin_there.rs draws; asked again every 2 s while something there runs.
use crate::hub::{now_ms, Hub};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::{Arc, Mutex}, time::Duration};

/// One provider a machine can be signed in to: its CLI, the arguments of its own login, and what the person does in it when that is not obvious.
pub struct Provider { pub id: &'static str, pub title: &'static str, pub login: &'static [&'static str], pub hint: &'static str }
pub const PROVIDERS: &[Provider] = &[
    Provider { id: "claude", title: "Claude", login: &["auth", "login", "--claudeai"], hint: "Open the link, sign in, then paste the code the page shows." },
    Provider { id: "codex", title: "Codex", login: &["login", "--device-auth"], hint: "Open the link, sign in with ChatGPT and enter the code." },
    Provider { id: "agy", title: "Antigravity", login: &[], hint: "agy asks you to sign in when it starts. Follow it here." },
    Provider { id: "grok", title: "Grok", login: &["login"], hint: "" },
    Provider { id: "muse", title: "Muse", login: &["login"], hint: "" },
    Provider { id: "opencode", title: "OpenCode", login: &["auth", "login"], hint: "" },
    Provider { id: "pi", title: "Pi", login: &[], hint: "Type /login, then follow it." },
    Provider { id: "omp", title: "Oh My Pi", login: &[], hint: "Type /login, then follow it." },
];
pub fn provider(id: &str) -> Option<&'static Provider> { PROVIDERS.iter().find(|p| p.id == id) }
/// The keys a press sends, by name.
pub const KEYS: &[(&str, &str)] = &[("enter", "\r"), ("up", "\x1b[A"), ("down", "\x1b[B"), ("esc", "\x1b"), ("tab", "\t"), ("ctrl-c", "\x03")];
/// The terminal a provider's sign-in runs in on the machine asked: a quiet one (term/mod.rs `quiet`), whose bytes reach no client there, since
/// what it echoes may be a key; only `session` reads it, redacted.
pub fn key_of(provider: &str) -> String { format!("q:signin-{provider}") }
/// The vault name a key typed for a provider's sign-in is kept under on the machine asked (the hub masks every vault value everywhere).
pub fn vault_name(provider: &str) -> String { format!("hub.signin.{provider}") }
/// Wide enough that a sign-in link is one line (Claude's is a few hundred characters); `link_in` joins one that wraps all the same.
const COLS: u16 = 500;
const ROWS: u16 = 30;
/// The lines of a sign-in's screen sent back: its end, where the prompt is.
const LINES: usize = 12;
/// A typed line at most this long.
const TYPED_MAX: usize = 4096;

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }

// ---- reading a sign-in's screen (pure) ----

/// The sign-in link on a screen: the last https link, joined across lines where it filled a line to the edge (`cols`).
pub fn link_in(text: &str, cols: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = String::new();
    for (i, l) in lines.iter().enumerate() {
        let Some(at) = l.find("https://") else { continue };
        let mut u: String = l[at..].chars().take_while(|c| !c.is_whitespace()).collect();
        let mut j = i;
        // the link reached the edge: it goes on at the start of the next line
        while l.chars().count() >= cols && j + 1 < lines.len() && at + u.len() >= lines[j].len() {
            j += 1; let more: String = lines[j].chars().take_while(|c| !c.is_whitespace()).collect();
            if more.is_empty() { break }
            u.push_str(&more);
            if lines[j].chars().count() < cols { break }
        }
        let u = u.trim_end_matches(|c: char| matches!(c, '.' | ',' | ')' | ']' | '>' | '"' | '\'')).to_string();
        if u.len() > "https://".len() { found = u }
    }
    found.chars().take(4096).collect()
}
/// A device code on a screen: a word of capitals and digits in two or three groups joined by `-` ("ABCD-1234", "AB12-CD34-EF"), the last one shown.
pub fn code_in(text: &str) -> String {
    let mut found = String::new();
    for w in text.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | ':' | '(' | ')')) {
        let w = w.trim_end_matches(['.', ',']);
        let parts: Vec<&str> = w.split('-').collect();
        let ok = (2..=3).contains(&parts.len()) && parts.iter().all(|p| (3..=6).contains(&p.len()) && p.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
            && w.chars().any(|c| c.is_ascii_uppercase()) && !w.starts_with("HTTP");
        if ok { found = w.to_string() }
    }
    found
}
/// The end of a screen: its last `n` lines that are not blank, each cut to a readable length (a link is shown on its own).
pub fn tail(text: &str, n: usize) -> Vec<String> {
    let ls: Vec<String> = text.lines().map(|l| l.trim_end()).filter(|l| !l.trim().is_empty()).map(|l| if l.chars().count() > 160 { format!("{}…", l.chars().take(159).collect::<String>()) } else { l.to_string() }).collect();
    ls[ls.len().saturating_sub(n)..].to_vec()
}
const HIDDEN: &str = "[hidden]";
/// Prefixes of the keys providers hand out: a word starting with one, and 8 or more key characters after it, is hidden.
const KEY_PREFIXES: &[&str] = &["sk-ant-", "sk-", "xai-", "ghp_", "gho_", "ghs_", "ghu_", "github_pat_", "glpat-", "AIza", "xoxb-", "xoxp-"];
fn key_char(c: char) -> bool { c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '/' | '.') }
fn run_char(c: char) -> bool { c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '/') }
/// A sign-in's screen with what may be a secret hidden before it leaves the machine: each line typed there (`typed`, 4 characters or more),
/// a key of a known shape, what follows `Bearer`, and any run of 32 or more key characters (base64, hex, a token of no known shape).
pub fn redact(text: &str, typed: &[String]) -> String {
    let mut t = text.to_string();
    for v in typed.iter().filter(|v| v.chars().count() >= 4) { t = t.replace(v.as_str(), HIDDEN) }
    let mut out = String::with_capacity(t.len());
    let cs: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        // a run of key characters, as a word
        if run_char(cs[i]) && (i == 0 || !key_char(cs[i - 1])) {
            let mut j = i; while j < cs.len() && key_char(cs[j]) { j += 1 }
            let w: String = cs[i..j].iter().collect();
            let known = KEY_PREFIXES.iter().any(|p| w.starts_with(p) && w.len() >= p.len() + 8);
            let long = w.split('.').any(|part| part.chars().count() >= 32);
            if known || long { out.push_str(HIDDEN) } else { out.push_str(&w) }
            i = j; continue
        }
        out.push(cs[i]); i += 1;
    }
    // Bearer <token>, whatever its shape
    let mut res = String::with_capacity(out.len()); let mut rest = out.as_str();
    while let Some(at) = rest.find("Bearer ") {
        res.push_str(&rest[..at + 7]); let after = &rest[at + 7..];
        let end = after.find(char::is_whitespace).unwrap_or(after.len());
        if end > 0 && &after[..end] != HIDDEN { res.push_str(HIDDEN) } else { res.push_str(&after[..end]) }
        rest = &after[end..];
    }
    res.push_str(rest);
    crate::settings::mask::mask(&res)
}
/// What a sign-in's terminal says, as the client draws it, redacted (`redact`). `state` is the terminal's ("running", "exited 0", "stopped").
/// The link is read from the screen as it is (a long sign-in link would be hidden as a run), with only the typed lines and the vault's values taken out.
pub fn session(text: &str, state: &str, running: bool, cols: usize, typed: &[String]) -> Value {
    let url = if running { let mut u = link_in(text, cols); for v in typed.iter().filter(|v| v.chars().count() >= 4) { if u.contains(v.as_str()) { u.clear() } } crate::settings::mask::mask(&u) } else { String::new() };
    let clean = redact(text, typed);
    json!({"state": state, "running": running, "url": url, "code": if running { code_in(&clean) } else { String::new() }, "lines": tail(&clean, LINES)})
}

/// The lines typed into each sign-in here (per home and provider, the newest few), so its screen never shows them back.
static TYPED: Mutex<Vec<(PathBuf, String, Vec<String>)>> = Mutex::new(Vec::new());
fn typed_of(hub: &Hub, prov: &str) -> Vec<String> { TYPED.lock().unwrap().iter().find(|(h, p, _)| *h == hub.cfg.home && p == prov).map(|x| x.2.clone()).unwrap_or_default() }
fn remember_typed(hub: &Hub, prov: &str, line: &str) {
    let mut g = TYPED.lock().unwrap();
    if !g.iter().any(|(h, p, _)| *h == hub.cfg.home && p == prov) { g.push((hub.cfg.home.clone(), prov.to_string(), vec![])) }
    let v = &mut g.iter_mut().find(|(h, p, _)| *h == hub.cfg.home && p == prov).unwrap().2;
    if !v.iter().any(|x| x == line) { v.push(line.to_string()) } while v.len() > 16 { v.remove(0); }
}

// ---- the machine asked ----

/// The checks of the machine asked, per home: (when, providers). Kept a minute: `--version` and `auth status` take a while.
static CHECKED: Mutex<Vec<(PathBuf, u64, Value)>> = Mutex::new(Vec::new());
const CHECK_FOR_MS: u64 = 60_000;
const TIMEOUT: Duration = Duration::from_secs(15);

async fn run(cmd: &std::path::Path, args: &[&str]) -> Option<(i32, String, String)> {
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Agent, cmd);
    c.args(args).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let out = tokio::time::timeout(TIMEOUT, c.output()).await.ok()?.ok()?;
    Some((out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned()))
}
/// One provider here: where its CLI is, its version, whether it is signed in and as whom (Claude and Codex say; the others cannot be asked).
async fn check_one(cli: &crate::agents::proc::Cli, p: &Provider) -> Value {
    use super::onboard::{claude_signed, codex_signed, version_of};
    let Ok(path) = cli.resolve(p.id) else { return json!({"id": p.id, "path": ""}) };
    let version = run(&path, &["--version"]).await.filter(|r| r.0 == 0).map_or(String::new(), |r| version_of(&r.1));
    let (signed, who) = match p.id {
        "claude" => run(&path, &["auth", "status"]).await.map_or(("unknown", String::new()), |r| claude_signed(r.0, &r.1)),
        "codex" => (run(&path, &["login", "status"]).await.map_or("unknown", |r| codex_signed(r.0, &r.1, &r.2)), String::new()),
        _ => ("unknown", String::new()),
    };
    json!({"id": p.id, "path": path.to_string_lossy(), "version": version, "signed": signed, "who": who})
}
async fn checks(hub: &Arc<Hub>, fresh: bool) -> Value {
    let now = now_ms();
    if !fresh { if let Some(v) = CHECKED.lock().unwrap().iter().find(|(h, at, _)| *h == hub.cfg.home && now < at + CHECK_FOR_MS).map(|x| x.2.clone()) { return v } }
    let cli = crate::agents::proc::Cli::from_env();
    let v = Value::Array(futures_util::future::join_all(PROVIDERS.iter().map(|p| check_one(&cli, p))).await);
    let mut g = CHECKED.lock().unwrap(); g.retain(|(h, _, _)| *h != hub.cfg.home); g.push((hub.cfg.home.clone(), now, v.clone()));
    v
}
fn forget_checks(hub: &Hub) { CHECKED.lock().unwrap().retain(|(h, _, _)| *h != hub.cfg.home) }

/// The state of this machine for a client elsewhere: each provider with its check, its install and its sign-in's terminal; the Claude sync tokens here.
pub async fn state(hub: &Arc<Hub>, fresh: bool) -> Value {
    let found = checks(hub, fresh).await;
    let installs: Value = serde_json::from_str(s(&hub.info_now(), "onboard.installs")).unwrap_or(Value::Null);
    let terms = crate::term::get(hub);
    let mut providers = vec![];
    for p in PROVIDERS {
        let c = found.as_array().and_then(|a| a.iter().find(|x| s(x, "id") == p.id)).cloned().unwrap_or(json!({}));
        let typed = typed_of(hub, p.id);
        let sess = match &terms { Some(t) => t.screen(&key_of(p.id)).await.map(|(text, st, running, cols)| session(&text, &st, running, cols, &typed)), None => None };
        providers.push(json!({"id": p.id, "title": p.title, "hint": p.hint, "installed": !s(&c, "path").is_empty(), "version": s(&c, "version"), "signed": if s(&c, "signed").is_empty() { "unknown" } else { s(&c, "signed") },
            "who": s(&c, "who"), "installable": super::tools_install::plan(p.id, &hub.cfg.os, &hub.cfg.arch, false, false).map_or(false, |x| !matches!(x, super::tools_install::Plan::Link(_))),
            "install": installs.get(p.id).cloned().unwrap_or(Value::Null), "session": sess.unwrap_or(Value::Null)}));
    }
    let sync: Vec<Value> = match hub.accounts.get() {
        Some(m) => m.request(&json!({"op": "list"})).await["profiles"].as_array().into_iter().flatten().filter(|x| s(x, "kind") == super::accounts_io::SYNC)
            .map(|x| json!({"id": s(x, "id"), "label": s(x, "label"), "email": s(&x["identity"], "email"), "state": s(x, "state")})).collect(),
        None => vec![],
    };
    json!({"providers": providers, "sync": sync, "os": hub.cfg.os})
}

/// A press made on another machine, carried out here.
async fn act_here(hub: &Arc<Hub>, prov: &str, action: &str, text: &str) -> Result<(), String> {
    if crate::copy::on() { return Err(crate::copy::refusal("a sign-in")) }
    let p = provider(prov).ok_or("not a provider Backplane signs in")?;
    let key = key_of(p.id);
    let terms = || crate::term::get(hub).ok_or_else(|| "terminals are not running on that machine".to_string());
    match action {
        "start" | "shell" => {
            let t = terms()?;
            let (prog, args): (String, Vec<String>) = if action == "shell" { (String::new(), vec![]) } else {
                let path = crate::agents::proc::Cli::from_env().resolve(p.id).map_err(|_| format!("{} is not installed there", p.title))?;
                (path.to_string_lossy().into_owned(), p.login.iter().map(|a| a.to_string()).collect())
            };
            let cwd = hub.cfg.home.to_string_lossy().into_owned();
            t.run_alone(&key, &format!("Sign in to {}", p.title), &cwd, &prog, &args, COLS, ROWS).await.map(|_| ())
        }
        // a line typed, or a key: the key is kept in this machine's vault first (so every mask here covers it) and typed from there
        "keys" | "secret" => {
            if text.len() > TYPED_MAX || text.contains(['\r', '\n', '\0']) { return Err("type one line".into()) }
            let t = terms()?;
            let line = if action == "secret" {
                if text.trim().is_empty() { return Err("paste the key first".into()) }
                let (name, v) = (vault_name(p.id), text.trim().to_string());
                let purpose = format!("{} sign-in, typed from another of your machines", p.title);
                let r = { let (n, v) = (name.clone(), v.clone()); tokio::task::spawn_blocking(move || crate::settings::vault::own_put(&n, &v, &purpose)).await.unwrap_or_else(|_| "fail".into()) };
                if r.starts_with("fail") { return Err("the key could not be kept in that machine's vault (is it locked?)".into()) }
                crate::settings::vault::own_get(&name).ok_or("the key could not be read back from that machine's vault")?
            } else { text.to_string() };
            remember_typed(hub, p.id, line.trim());
            let mut b = line.into_bytes(); b.push(b'\r');
            if t.input(&key, b).await { Ok(()) } else { Err("that sign-in is not running".into()) }
        }
        "key" => {
            let k = KEYS.iter().find(|(n, _)| *n == text).ok_or("not a key")?;
            if terms()?.input(&key, k.1.as_bytes().to_vec()).await { Ok(()) } else { Err("that sign-in is not running".into()) }
        }
        "stop" => { terms()?.kill(&key).await; forget_checks(hub); Ok(()) }
        "check" => { forget_checks(hub); Ok(()) }
        "install" => {
            let checks: Value = hub.setting("onboard.checks").await.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
            super::tools_install::start(hub, p.id, checks["winget"] == true, checks["node"] == true, |h| Box::pin(async move { forget_checks(&h) })).await.map(|_| ())
        }
        _ => Err("action: start, shell, keys, secret, key, stop, install or check".into()),
    }
}

/// `signin.state` and `signin.act` from one of the owner's machines (net/rpc.rs checked `link`).
pub async fn serve(hub: &Arc<Hub>, _link: &str, m: &str, p: &Value) -> Value {
    match m {
        "signin.state" => json!({"t":"reply","ok":true,"result": state(hub, p["fresh"] == true).await}),
        "signin.act" => match act_here(hub, s(p, "provider"), s(p, "action"), s(p, "text")).await {
            // a sign-in just started prints its link within a moment: the answer waits for it a little
            Ok(()) => { if s(p, "action") == "start" { tokio::time::sleep(Duration::from_millis(1200)).await } json!({"t":"reply","ok":true,"result": state(hub, s(p, "action") == "check").await}) }
            Err(e) => json!({"t":"reply","ok":false,"error": e}),
        },
        _ => json!({"t":"reply","ok":false,"error":"not a sign-in request"}),
    }
}

// ---- the machine asking ----

/// The panel open here: the machine (link and name), its last answer, what went wrong, and the Claude sync token being made here.
#[derive(Default, Clone)]
struct Panel { link: String, name: String, data: Value, error: String, at: u64, sync: String, polling: bool, keymode: Vec<String> }
static PANELS: Mutex<Vec<(PathBuf, Panel)>> = Mutex::new(Vec::new());
fn panel(hub: &Hub) -> Panel { PANELS.lock().unwrap().iter().find(|(h, _)| *h == hub.cfg.home).map(|x| x.1.clone()).unwrap_or_default() }
fn put(hub: &Hub, f: impl FnOnce(&mut Panel)) -> Panel {
    let mut g = PANELS.lock().unwrap();
    if !g.iter().any(|(h, _)| *h == hub.cfg.home) { g.push((hub.cfg.home.clone(), Panel::default())) }
    let p = &mut g.iter_mut().find(|(h, _)| *h == hub.cfg.home).unwrap().1; f(p); p.clone()
}

/// What a far answer means: the result, or the words for why there is none.
pub fn answer(code: u16, text: &str) -> Result<Value, String> {
    match code {
        0 => Err("that machine is away".into()),
        200 => { let v: Value = serde_json::from_str(text).map_err(|_| "that machine answered something unreadable".to_string())?;
            if v["ok"] == true { Ok(v["result"].clone()) } else { let e = s(&v, "error"); Err(if e.is_empty() { "that machine refused".into() } else { e.to_string() }) } }
        // a hub from before this: the generic far path answers that it does not share it
        403 | 501 => Err(crate::net::rpc::OLDER.into()),
        _ => Err(format!("that machine did not take it ({code})")),
    }
}
async fn ask(hub: &Arc<Hub>, link: &str, m: &str, p: &Value) -> Result<Value, String> { let (code, text) = hub.far_raw(link, m, p).await; answer(code, &text) }

/// This machine's Claude sync tokens, and the sign-in of the one being made here (accounts_phone.rs words it).
async fn local(hub: &Arc<Hub>, making: &str) -> Value {
    let Some(m) = hub.accounts.get().cloned() else { return json!({"sync": [], "attempt": null}) };
    let snap = m.request(&json!({"op": "list"})).await;
    let sync: Vec<Value> = snap["profiles"].as_array().into_iter().flatten().filter(|x| s(x, "kind") == super::accounts_io::SYNC)
        .map(|x| json!({"id": s(x, "id"), "label": s(x, "label"), "email": s(&x["identity"], "email"), "state": s(x, "state")})).collect();
    let attempt = if making.is_empty() { Value::Null } else { snap["attempts"].as_array().and_then(|a| a.iter().filter(|x| s(x, "profile") == making).last()).map_or(Value::Null, |a| super::accounts_phone::signin(a, "claude")) };
    json!({"sync": sync, "attempt": attempt, "making": making})
}
async fn publish(hub: &Arc<Hub>) {
    let p = panel(hub);
    let v = if p.link.is_empty() { Value::Null } else { json!({"link": p.link, "name": p.name, "data": p.data, "error": p.error, "at": p.at, "local": local(hub, &p.sync).await, "keymode": p.keymode}) };
    hub.set_info("signin.there", &v.to_string()).await;
}
/// Whether anything shown runs (a sign-in there, an install there, the sync token's sign-in here): then the panel is asked again soon.
pub fn busy(info: &Value) -> bool {
    let running = info["data"]["providers"].as_array().into_iter().flatten().any(|p| p["session"]["running"] == true || s(&p["install"], "state") == "running");
    // the sync token's sign-in is done but its check here has not said connected yet: still busy, or a page in between says "Make a sync token"
    let making = s(&info["local"], "making");
    let checking = info["local"]["attempt"]["done"] == true && info["local"]["sync"].as_array().into_iter().flatten().any(|x| s(x, "id") == making && s(x, "state") != "connected");
    running || info["local"]["attempt"]["busy"] == true || checking
}
/// Whether something that ran in `before` (a sign-in, an install) has ended in `after`: its end changes who is signed in, which the answer
/// read with the minute's cached checks does not say yet.
pub fn ended(before: &Value, after: &Value) -> bool {
    let ran = |d: &Value, id: &str| d["providers"].as_array().into_iter().flatten().find(|p| s(p, "id") == id).map_or(false, |p| p["session"]["running"] == true || s(&p["install"], "state") == "running");
    before["providers"].as_array().into_iter().flatten().any(|p| ran(before, s(p, "id")) && !ran(after, s(p, "id")))
}
/// The machine's answer, made whole: when something that ran has just ended, it is asked again with fresh checks before anyone is shown it,
/// so no page says "not busy" with the status from before the run.
async fn settled(hub: &Arc<Hub>, link: &str, before: &Value, r: Result<Value, String>) -> Result<Value, String> {
    match r { Ok(d) if ended(before, &d) => ask(hub, link, "signin.state", &json!({"fresh": true})).await.or(Ok(d)), r => r }
}
async fn refresh(hub: &Arc<Hub>, fresh: bool) {
    let p = panel(hub); if p.link.is_empty() { return }
    let r = ask(hub, &p.link, "signin.state", &json!({"fresh": fresh})).await;
    let r = settled(hub, &p.link, &p.data, r).await;
    put(hub, |x| if x.link == p.link { match r { Ok(d) => { x.data = d; x.error.clear(); x.at = now_ms() / 1000 } Err(e) => x.error = e } });
    publish(hub).await;
}
/// Asked again every 2 s while something runs, then once more (fresh: a sign-in that ended changes who is signed in), then left.
fn poll(hub: &Arc<Hub>) {
    if put(hub, |_| {}).polling { return }
    put(hub, |x| x.polling = true);
    let h = Arc::downgrade(hub);
    tokio::spawn(async move {
        let mut was = true;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let Some(hub) = h.upgrade() else { return };
            let open = !panel(&hub).link.is_empty();
            let now: Value = serde_json::from_str(s(&hub.info_now(), "signin.there")).unwrap_or(Value::Null);
            let b = open && busy(&now);
            if !b { if was && open { refresh(&hub, true).await; let again: Value = serde_json::from_str(s(&hub.info_now(), "signin.there")).unwrap_or(Value::Null); if busy(&again) { continue } } put(&hub, |x| x.polling = false); return }
            was = b; refresh(&hub, false).await;
        }
    });
}

/// A masked box's Send (`secret-save`, its value packed `@signin\u{1f}<link>\u{1f}<provider>`, `@signin-key\u{1f}...` or `@signin-sync`): the typed
/// text is in `fields` only, for this one request; it is never a draft, never in the panel's state and never in an error.
pub fn is_save(value: &str) -> bool { value == "@signin-sync" || value.starts_with("@signin\u{1f}") || value.starts_with("@signin-key\u{1f}") }
pub async fn save(hub: &Arc<Hub>, value: &str, fields: &Value) -> Result<Value, String> {
    if value == "@signin-sync" { return act(hub, "machine-signin-sync-code", "", fields).await }
    if let Some(v) = value.strip_prefix("@signin-key\u{1f}") { return act(hub, "machine-signin-secret", v, fields).await }
    act(hub, "machine-signin-keys", value.strip_prefix("@signin\u{1f}").unwrap_or(""), fields).await
}

fn parts(value: &str) -> (String, String, String) { let mut i = value.split('\u{1f}'); (i.next().unwrap_or("").into(), i.next().unwrap_or("").into(), i.next().unwrap_or("").into()) }

/// A press on the panel (`machine-signin*`, through `machines.page.act`). `fields` carries `sikeys` (a line to type there) and `sicode` (the sync token's code).
pub async fn act(hub: &Arc<Hub>, action: &str, value: &str, fields: &Value) -> Result<Value, String> {
    let (link, prov, extra) = parts(value);
    match action {
        "machine-signin-open" => {
            let Some((id, name, _)) = hub.far_machines().into_iter().find(|(id, _, _)| *id == link) else { return Err("that machine is not linked".into()) };
            put(hub, |x| { if x.link != id { *x = Panel { polling: x.polling, ..Panel::default() } } x.link = id.clone(); x.name = name.clone(); x.error.clear() });
            publish(hub).await; refresh(hub, true).await; poll(hub); Ok(json!({}))
        }
        "machine-signin-close" => { put(hub, |x| { let polling = x.polling; *x = Panel { polling, ..Panel::default() } }); publish(hub).await; Ok(json!({})) }
        // the Claude sync token, made here: one approval in the browser, then it reaches every linked machine (accounts_sync.rs)
        "machine-signin-sync" => {
            let m = hub.accounts.get().cloned().ok_or("accounts are not running here")?;
            let before = m.request(&json!({"op": "list"})).await;
            let label = super::accounts_phone::new_sync_label(&before, "");
            let r = m.request(&json!({"op": "add", "provider": "claude", "label": label, "kind": super::accounts_io::SYNC, "start": true})).await;
            let made = s(&r, "made").to_string(); if made.is_empty() { return Err(if s(&r, "error").is_empty() { "the sync token could not be started".into() } else { s(&r, "error").into() }) }
            put(hub, |x| x.sync = made); publish(hub).await; poll(hub); Ok(json!({}))
        }
        // "This is a key": the box's next Send keeps the line in that machine's vault and types it from there
        "machine-signin-keymode" => { put(hub, |x| if x.keymode.contains(&prov) { x.keymode.retain(|k| *k != prov) } else { x.keymode.push(prov.clone()) }); publish(hub).await; Ok(json!({})) }
        "machine-signin-sync-code" | "machine-signin-sync-cancel" => {
            let m = hub.accounts.get().cloned().ok_or("accounts are not running here")?;
            let id = panel(hub).sync; if id.is_empty() { return Err("no sync token is being made".into()) }
            let snap = m.request(&json!({"op": "list"})).await;
            let att = snap["attempts"].as_array().and_then(|a| a.iter().filter(|x| s(x, "profile") == id).last()).map(|x| s(x, "id").to_string()).unwrap_or_default();
            let q = if action.ends_with("code") { json!({"op": "respond", "profile": id, "attempt": att, "code": s(fields, "sicode").trim()}) } else { json!({"op": "cancel", "profile": id, "attempt": att}) };
            let r = m.request(&q).await;
            put(hub, |x| x.error = s(&r, "error").to_string()); publish(hub).await; poll(hub);
            if !s(&r, "error").is_empty() { return Err(s(&r, "error").into()) } Ok(json!({}))
        }
        "machine-signin" | "machine-signin-shell" | "machine-signin-keys" | "machine-signin-secret" | "machine-signin-key" | "machine-signin-stop" | "machine-signin-install" | "machine-signin-check" => {
            if link.is_empty() || provider(&prov).is_none() { return Err("which machine and which agent?".into()) }
            let (a, text) = match action {
                "machine-signin" => ("start", String::new()), "machine-signin-shell" => ("shell", String::new()), "machine-signin-stop" => ("stop", String::new()),
                "machine-signin-install" => ("install", String::new()), "machine-signin-check" => ("check", String::new()),
                "machine-signin-key" => ("key", extra), "machine-signin-secret" => ("secret", s(fields, "sikeys").to_string()), _ => ("keys", s(fields, "sikeys").to_string()),
            };
            let before = panel(hub).data;
            let r = ask(hub, &link, "signin.act", &json!({"provider": prov, "action": a, "text": text})).await;
            let r = settled(hub, &link, &before, r).await;
            put(hub, |x| if x.link == link { match &r { Ok(d) => { x.data = d.clone(); x.error.clear(); x.at = now_ms() / 1000 } Err(e) => x.error = e.clone() } });
            publish(hub).await; poll(hub);
            r.map(|_| json!({}))
        }
        _ => Err("not a sign-in press".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_link_is_found_whole_even_when_it_wraps() {
        let screen = "Browser didn't open? Use the url below to sign in:\n\nhttps://claude.com/cai/oauth/authorize?code=true&client_id=abc&state=XYZ\n\nPaste code here if prompted >";
        assert_eq!(link_in(screen, 500), "https://claude.com/cai/oauth/authorize?code=true&client_id=abc&state=XYZ");
        let wrapped = "Visit https://auth.example.com/abcdefgh\nijklmnop and sign in."; // 39 columns, the link filling the first line to its edge
        assert_eq!(link_in(wrapped, 39), "https://auth.example.com/abcdefghijklmnop");
        assert_eq!(link_in("Visit https://a.example.com/x.\nnext", 80), "https://a.example.com/x", "the full stop after it is not the link's");
        assert_eq!(link_in("nothing to open", 80), "");
    }
    #[test] fn a_device_code_is_told_from_other_words() {
        let codex = "Welcome to Codex\n1. Open this link in your browser and sign in to your account\n   https://auth.openai.com/codex/device\n2. Enter this one-time code (expires in 15 minutes)\n   ABCD-12345\n";
        assert_eq!(code_in(codex), "ABCD-1234".to_string() + "5");
        assert_eq!(code_in("Your code: WDJB-MJHT."), "WDJB-MJHT");
        assert_eq!(code_in("HTTP-200 and x-ray and 2026-11-11"), "", "a date or a word with lower case is no code");
        assert_eq!(code_in("no code at all"), "");
    }
    #[test] fn a_session_shows_its_link_and_code_only_while_it_runs() {
        let t = "Open https://auth.openai.com/codex/device\nEnter ABCD-EFGH";
        let on = session(t, "running", true, 500, &[]); assert_eq!((on["url"].as_str().unwrap(), on["code"].as_str().unwrap()), ("https://auth.openai.com/codex/device", "ABCD-EFGH"));
        let off = session(t, "exited 0", false, 500, &[]); assert_eq!((off["url"].as_str().unwrap(), off["code"].as_str().unwrap(), off["state"].as_str().unwrap()), ("", "", "exited 0"));
        assert_eq!(off["lines"].as_array().unwrap().len(), 2);
    }
    #[test] fn what_may_be_a_secret_never_leaves_the_screen() {
        let keys = ["sk-ant-api03-abcdefghijklmnopqrstuv", "sk-proj-ABCDEFGH12345678", "xai-0123456789abcdef", "ghp_ABCDEFGHIJKLMNOP1234", "gho_abcdefgh1234", "AIzaSyA-1234567890abcdefgh"];
        for k in keys { let r = redact(&format!("Your key {k} was saved."), &[]); assert!(!r.contains(k) && r.contains(HIDDEN) && r.starts_with("Your key "), "{k}: {r}") }
        let b64 = "dGhpcyBpcyBhIHZlcnkgbG9uZyBzZWNyZXQgdG9rZW4gaW4gYmFzZTY0"; assert_eq!(redact(&format!("token={b64}"), &[]), format!("token={HIDDEN}"));
        let hex = "0123456789abcdef0123456789abcdef"; assert!(!redact(hex, &[]).contains(hex));
        assert_eq!(redact("Authorization: Bearer abc.def", &[]), format!("Authorization: Bearer {HIDDEN}"));
        assert_eq!(redact("> my-own-word\nok", &["my-own-word".into()]), format!("> {HIDDEN}\nok"), "the line just typed, echoed");
        assert_eq!(redact("Enter this code QWER-TY123, then sk- and ask-me", &[]), "Enter this code QWER-TY123, then sk- and ask-me", "ordinary words stay");
        assert_eq!(redact("1", &["1".into()]), "1", "a short answer (a menu's 1) is not hidden everywhere");
    }
    #[test] fn a_session_hides_an_echoed_key_but_keeps_the_sign_in_link() {
        let screen = "Visit https://claude.com/cai/oauth/authorize?code_challenge=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ&state=S\nAPI key: sk-ant-api03-SECRETSECRETSECRET\n> typed-secret-value";
        let v = session(screen, "running", true, 500, &["typed-secret-value".into()]);
        let all = v.to_string();
        assert!(!all.contains("SECRETSECRET") && !all.contains("typed-secret-value"), "{all}");
        assert!(v["url"].as_str().unwrap().ends_with("&state=S"), "the link is whole: {}", v["url"]);
    }
    #[test] fn a_masked_send_is_told_by_its_value() {
        assert!(is_save("@signin\u{1f}p1\u{1f}codex") && is_save("@signin-key\u{1f}p1\u{1f}grok") && is_save("@signin-sync"));
        assert!(!is_save("@apikey\u{1f}OPENAI") && !is_save("signin"));
    }
    #[test] fn the_tail_is_the_last_lines_that_say_something() {
        let t = (1..=20).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n\n");
        let v = tail(&t, 3); assert_eq!(v, ["line 18", "line 19", "line 20"]);
        assert!(tail(&"x".repeat(400), 1)[0].ends_with('…'));
    }
    #[test] fn a_far_answer_says_why_it_is_none() {
        assert_eq!(answer(0, ""), Err("that machine is away".into()));
        assert_eq!(answer(403, "not something this machine shares"), Err(crate::net::rpc::OLDER.into()), "a hub from before this needs the update");
        assert_eq!(answer(200, r#"{"t":"reply","ok":true,"result":{"providers":[]}}"#), Ok(json!({"providers":[]})));
        assert_eq!(answer(200, r#"{"t":"reply","ok":false,"error":"Grok is not installed there"}"#), Err("Grok is not installed there".into()));
    }
    #[test] fn a_run_that_ended_asks_for_fresh_checks_before_it_is_shown() {
        let run = json!({"providers":[{"id":"codex","session":{"running":true}},{"id":"grok","install":{"state":"running"}}]});
        let over = json!({"providers":[{"id":"codex","session":{"running":false,"state":"exited 0"}},{"id":"grok","install":{"state":"running"}}]});
        assert!(ended(&run, &over), "the sign-in ended: busy goes false only with the status read after it");
        assert!(!ended(&run, &run) && !ended(&over, &over) && !ended(&json!(null), &over));
        let installed = json!({"providers":[{"id":"codex","session":{"running":true}},{"id":"grok","install":{"state":"done"}}]});
        assert!(ended(&run, &installed), "an install that finished too");
    }
    #[test] fn busy_while_a_sign_in_or_an_install_runs_there_or_the_token_here() {
        assert!(!busy(&json!({"data":{"providers":[{"session":{"running":false}}]}})));
        assert!(busy(&json!({"data":{"providers":[{"session":{"running":true}}]}})));
        assert!(busy(&json!({"data":{"providers":[{"install":{"state":"running"}}]}})));
        assert!(busy(&json!({"local":{"attempt":{"busy":true}}})));
        assert!(busy(&json!({"local":{"making":"s1","attempt":{"done":true},"sync":[{"id":"s1","state":"unchecked"}]}})), "made, not checked yet");
        assert!(!busy(&json!({"local":{"making":"s1","attempt":{"done":true},"sync":[{"id":"s1","state":"connected"}]}})));
    }
    #[test] fn every_provider_has_one_terminal_and_no_key_is_free_text() {
        let keys: std::collections::HashSet<String> = PROVIDERS.iter().map(|p| key_of(p.id)).collect(); assert_eq!(keys.len(), PROVIDERS.len());
        assert!(crate::term::quiet(&key_of("claude")), "no client there sees its bytes");
        assert!(KEYS.iter().all(|(_, b)| b.len() <= 3));
    }
}
