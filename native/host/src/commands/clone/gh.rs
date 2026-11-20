//! The GitHub CLI (`gh`) for clones: the person's repositories to suggest (every signed-in account's own and its organizations'), the
//! credential helper a GitHub https clone uses, and GitHub's device sign-in (a code the person enters at github.com/login/device, from any
//! client). Tokens stay inside gh and its children's environment; none reaches a client, the log or a chat.
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub fn gh_path() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("BACKPLANE_GH") { return (!p.is_empty()).then(|| std::path::PathBuf::from(p)).filter(|p| p.is_file()) } // tests: a stand-in, or "" for none
    crate::mcpx::svc::which("gh").or_else(|| ["/opt/homebrew/bin/gh", "/usr/local/bin/gh", "/usr/bin/gh"].iter().map(std::path::PathBuf::from).find(|p| p.is_file()))
}
/// `-c credential.https://github.com.helper=!'<gh>' auth git-credential` for a GitHub https address (after the person's own helpers; a
/// `https://<account>@github.com/...` address takes that account's token); None for anything else or without gh.
pub fn credential_helper(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split('/').next()?.rsplit('@').next()?;
    if !host.eq_ignore_ascii_case("github.com") { return None }
    let gh = gh_path()?; let gh = gh.to_string_lossy().replace('\\', "/");
    if gh.contains('\'') { return None }
    Some(format!("credential.https://github.com.helper=!'{gh}' auth git-credential"))
}
fn gh_cmd(gh: &std::path::Path) -> tokio::process::Command {
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Net, gh);
    c.env("GH_PROMPT_DISABLED", "1").env("NO_COLOR", "1").env("GH_NO_UPDATE_NOTIFIER", "1").stdin(std::process::Stdio::null()).kill_on_drop(true);
    c
}
async fn out(mut c: tokio::process::Command, secs: u64) -> Option<String> {
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    match tokio::time::timeout(Duration::from_secs(secs), c.output()).await { Ok(Ok(o)) if o.status.success() => Some(String::from_utf8_lossy(&o.stdout).into_owned()), _ => None }
}

/// The accounts gh is signed in to on github.com, the active one first (`gh auth status`'s "account <name>" lines).
pub fn accounts_of(status: &str) -> Vec<String> {
    let mut v: Vec<(bool, String)> = vec![]; let mut cur: Option<String> = None;
    for l in status.lines() {
        let l = l.trim();
        if let Some(i) = l.find(" account ") { let name = l[i + 9..].split_whitespace().next().unwrap_or("").to_string(); if !name.is_empty() { cur = Some(name.clone()); v.push((false, name)); } }
        if l.contains("Active account: true") { if let Some(c) = &cur { if let Some(x) = v.iter_mut().find(|x| &x.1 == c) { x.0 = true } } }
    }
    v.sort_by_key(|x| !x.0); v.dedup_by(|a, b| a.1 == b.1); v.into_iter().map(|x| x.1).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row { pub name: String, pub description: String, pub private: bool, pub pushed: String, /** the account that sees it, when not the active one */ pub account: String }
impl Row {
    /// What the repository field takes for it: `owner/repo`, or `https://<account>@github.com/owner/repo.git` for another account's.
    pub fn value(&self) -> String { if self.account.is_empty() { self.name.clone() } else { format!("https://{}@github.com/{}.git", self.account, self.name) } }
}
#[derive(Default)]
struct Cache { rows: Vec<Row>, state: &'static str, at: Option<Instant>, loading: bool }
static CACHE: Mutex<Cache> = Mutex::new(Cache { rows: Vec::new(), state: "", at: None, loading: false });
const FRESH: Duration = Duration::from_secs(600);
static DONE: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// Read every account's repositories and its organizations' (in the background; the views are told when they are in).
pub fn refresh(force: bool) {
    { let mut c = CACHE.lock().unwrap(); if c.loading || (!force && c.at.map_or(false, |a| a.elapsed() < FRESH)) { return } c.loading = true; }
    tokio::spawn(async move {
        let (rows, state) = load().await;
        { let mut c = CACHE.lock().unwrap(); if state == "ok" || c.rows.is_empty() { c.rows = rows; } c.state = state; c.at = Some(Instant::now()); c.loading = false; }
        DONE.notify_waiters();
        if let Some(h) = super::hub() { h.bump_views(); let _ = h.frames.send(json!({"t": "clone.repos"})); }
    });
}
async fn load() -> (Vec<Row>, &'static str) {
    let Some(gh) = gh_path() else { return (vec![], "nogh") };
    let mut st = gh_cmd(&gh); st.args(["auth", "status", "--hostname", "github.com"]);
    st.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let status = match tokio::time::timeout(Duration::from_secs(15), st.output()).await { Ok(Ok(o)) => format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)), _ => return (vec![], "error") };
    let accounts = accounts_of(&status);
    if accounts.is_empty() { return (vec![], "signedout") }
    let mut tasks = vec![];
    for (n, acct) in accounts.iter().enumerate().take(4) {
        let (gh, acct, active) = (gh.clone(), acct.clone(), n == 0);
        tasks.push(tokio::spawn(async move {
            // another account's token, for its own listing only (gh 2.40+); the active account's is gh's own
            let token = if active { None } else { let mut c = gh_cmd(&gh); c.args(["auth", "token", "--hostname", "github.com", "--user", &acct]); out(c, 10).await.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) };
            if !active && token.is_none() { return vec![] }
            let with = |mut c: tokio::process::Command| { if let Some(t) = &token { c.env("GH_TOKEN", t); } c };
            let mut owners = vec![acct.clone()];
            let mut c = gh_cmd(&gh); c.args(["api", "user/orgs", "--paginate", "--jq", ".[].login"]);
            if let Some(o) = out(with(c), 20).await { owners.extend(o.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).take(30)); }
            let mut lists = vec![];
            for owner in owners {
                let mut c = gh_cmd(&gh); c.args(["repo", "list", &owner, "--limit", "300", "--json", "nameWithOwner,description,isPrivate,pushedAt"]);
                let Some(text) = out(with(c), 30).await else { continue };
                let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                lists.extend(v.as_array().into_iter().flatten().map(|r| Row { name: r["nameWithOwner"].as_str().unwrap_or("").to_string(), description: r["description"].as_str().unwrap_or("").to_string(),
                    private: r["isPrivate"] == true, pushed: r["pushedAt"].as_str().unwrap_or("").to_string(), account: if active { String::new() } else { acct.clone() } }).filter(|r| !r.name.is_empty()));
            }
            lists
        }));
    }
    let mut rows: Vec<Row> = vec![];
    for t in tasks { if let Ok(v) = t.await { for r in v { if !rows.iter().any(|x| x.name.eq_ignore_ascii_case(&r.name)) { rows.push(r) } } } }
    rows.sort_by(|a, b| b.pushed.cmp(&a.pushed).then(a.name.cmp(&b.name)));
    (rows, "ok")
}

/// The rows that match `q`: a name that starts with it first (the repository's, then the owner's), then any that hold it; newest push first
/// within each. Empty `q`: the newest. Text that is a whole address matches nothing (it is cloned as typed).
pub fn matching(rows: &[Row], q: &str, limit: usize) -> (Vec<Row>, usize) {
    let q = q.trim().to_lowercase();
    if q.contains("://") || q.contains('@') || q.contains(':') { return (vec![], 0) }
    let q = q.strip_prefix("github.com/").unwrap_or(&q).to_string();
    let rank = |r: &Row| -> Option<u8> {
        let n = r.name.to_lowercase(); let repo = n.rsplit('/').next().unwrap_or(&n);
        if q.is_empty() { return Some(0) }
        if repo.starts_with(&q) || n.starts_with(&q) { Some(0) } else if n.contains(&q) { Some(1) } else if r.description.to_lowercase().contains(&q) { Some(2) } else { None }
    };
    let mut m: Vec<(u8, &Row)> = rows.iter().filter_map(|r| rank(r).map(|k| (k, r))).collect();
    m.sort_by_key(|x| x.0); // stable: newest first within a rank
    let n = m.len(); (m.into_iter().take(limit).map(|x| x.1.clone()).collect(), n)
}
fn rows_json(rows: &[Row]) -> Vec<Value> { rows.iter().map(|r| json!({"name": r.name, "value": r.value(), "description": r.description, "private": r.private, "account": r.account})).collect() }
/// What is known now (a read is started when there is none, or it is old): (rows, state, loading).
pub fn repos_now(q: &str, limit: usize) -> (Vec<Row>, usize, &'static str, bool) {
    refresh(false);
    let c = CACHE.lock().unwrap(); let (m, n) = matching(&c.rows, q, limit); (m, n, c.state, c.loading)
}
/// `clone.repos`: waits (up to 20 s) for the first read.
pub async fn repos_wait(q: &str, limit: usize, force: bool, wait: bool) -> Value {
    refresh(force);
    let first = wait && { let c = CACHE.lock().unwrap(); c.at.is_none() || force };
    if first { let w = DONE.notified(); if CACHE.lock().unwrap().loading { let _ = tokio::time::timeout(Duration::from_secs(20), w).await; } }
    let c = CACHE.lock().unwrap(); let (m, n) = matching(&c.rows, q, limit.clamp(1, 100));
    json!({"repos": rows_json(&m), "more": n.saturating_sub(m.len()), "state": c.state, "loading": c.loading, "hint": state_hint(c.state)})
}
/// What a client shows under the field when there are no suggestions to show, for the state of gh.
pub fn state_hint(state: &str) -> &'static str {
    match state { "nogh" => "Install the GitHub CLI (gh) on this machine to see your GitHub repositories here.", "signedout" => "Sign in to GitHub to see your repositories here.", "error" => "Couldn't list your GitHub repositories.", _ => "" }
}

// ---- sign-in ----

#[derive(Clone, Default)]
struct SignIn { state: &'static str, code: String, url: String, error: String, pid: u32, /** each start's number: a finished older one changes nothing */ attempt: u64 }
static SIGNIN: Mutex<SignIn> = Mutex::new(SignIn { state: "", code: String::new(), url: String::new(), error: String::new(), pid: 0, attempt: 0 });
fn signin_json() -> Value { let s = SIGNIN.lock().unwrap().clone(); json!({"state": s.state, "code": s.code, "url": s.url, "error": s.error}) }
pub fn signin_now() -> Value { signin_json() }
fn told() { if let Some(h) = super::hub() { h.bump_views(); let _ = h.frames.send(json!({"t": "clone.signin", "signin": signin_json()})); } }
/// The one-time code and the address in what `gh auth login --web` says.
pub fn device_code(text: &str) -> (String, String) {
    let code = text.split("one-time code:").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or("").to_string();
    let url = text.split_whitespace().find(|w| w.starts_with("https://") && w.contains("/login/device")).unwrap_or("").to_string();
    (code, url)
}
/// Start GitHub's device sign-in (`gh auth login --web` with no browser opened on this machine): its code and address come back in the
/// state (`waiting`), then `done` (the repository list is read again) or `failed`. One at a time; `restart` starts over.
pub fn signin(restart: bool) -> Value {
    let Some(gh) = gh_path() else { let mut s = SIGNIN.lock().unwrap(); let a = s.attempt + 1; *s = SignIn { state: "failed", error: "The GitHub CLI (gh) isn't installed on this machine. Install it, or add an SSH key to your GitHub account.".into(), attempt: a, ..Default::default() }; drop(s); return signin_json() };
    // the check, the older attempt's end and this one's number in one hold of the lock: two presses at once start one sign-in
    let (mine, old) = {
        let mut s = SIGNIN.lock().unwrap();
        if s.state == "starting" || (s.state == "waiting" && !restart) { drop(s); return signin_json() }
        let old = if matches!(s.state, "waiting") { s.pid } else { 0 };
        let a = s.attempt + 1; *s = SignIn { state: "starting", attempt: a, ..Default::default() }; (a, old)
    };
    if old > 0 { crate::plat::proc::kill_group(old, crate::plat::proc::TERM); }
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut c = gh_cmd(&gh);
        c.args(["auth", "login", "--hostname", "github.com", "--git-protocol", "https", "--web"]);
        #[cfg(unix)] c.env("GH_BROWSER", "true"); // the person opens the address on the device they are using: nothing opens on this machine
        c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        crate::plat::proc::agent_group(&mut c); // its own group, which dies with the hub on Linux
        let mut child = match c.spawn() { Ok(ch) => ch, Err(e) => { let mut s = SIGNIN.lock().unwrap(); if s.attempt == mine && s.state == "starting" { *s = SignIn { state: "failed", error: format!("gh didn't start: {e}"), attempt: mine, ..Default::default() }; } drop(s); return told() } };
        let pid = child.id().unwrap_or(0); if pid > 0 { crate::plat::proc::adopt(pid, true) }
        // cancelled or started again before it was running: it goes at once
        let gone = { let mut s = SIGNIN.lock().unwrap(); let g = s.attempt != mine || s.state == "cancelled"; if !g { s.pid = pid; } g };
        if gone { crate::plat::proc::kill_group(pid, crate::plat::proc::TERM); let _ = child.wait().await; return }
        let (mut a, mut b) = (child.stdout.take(), child.stderr.take());
        let mut text = String::new(); let (mut ba, mut bb) = (vec![0u8; 2048], vec![0u8; 2048]);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(900);
        loop {
            tokio::select! {
                n = async { match a.as_mut() { Some(x) => x.read(&mut ba).await, None => std::future::pending().await } } => match n { Ok(0) | Err(_) => a = None, Ok(n) => text.push_str(&String::from_utf8_lossy(&ba[..n])) },
                n = async { match b.as_mut() { Some(x) => x.read(&mut bb).await, None => std::future::pending().await } } => match n { Ok(0) | Err(_) => b = None, Ok(n) => text.push_str(&String::from_utf8_lossy(&bb[..n])) },
                _ = tokio::time::sleep_until(deadline) => { crate::plat::proc::kill_group(pid, crate::plat::proc::TERM); break }
            }
            let (code, url) = device_code(&text);
            let (stale, fresh) = { let mut s = SIGNIN.lock().unwrap(); let stale = s.attempt != mine; let mut fresh = false;
                if !stale && s.state == "starting" && !code.is_empty() { s.state = "waiting"; s.code = code; s.url = if url.is_empty() { "https://github.com/login/device".into() } else { url }; fresh = true; } (stale, fresh) };
            if stale { crate::plat::proc::kill_group(pid, crate::plat::proc::TERM); break }
            if fresh { told(); }
            if a.is_none() && b.is_none() { break }
        }
        let ok = matches!(child.wait().await, Ok(s) if s.success());
        { let mut s = SIGNIN.lock().unwrap();
          if s.state == "cancelled" || s.attempt != mine { return }
          *s = if ok { SignIn { state: "done", attempt: mine, ..Default::default() } } else { SignIn { state: "failed", error: text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("The sign-in didn't finish.").trim().to_string(), attempt: mine, ..Default::default() } }; }
        if ok { refresh(true); }
        told();
    });
    signin_json()
}
pub fn signin_cancel() -> Value {
    let pid = { let mut s = SIGNIN.lock().unwrap(); let p = s.pid; if matches!(s.state, "starting" | "waiting") { let a = s.attempt; *s = SignIn { state: "cancelled", attempt: a, ..Default::default() }; } p };
    if pid > 0 { crate::plat::proc::kill_group(pid, crate::plat::proc::TERM); }
    told(); signin_json()
}
