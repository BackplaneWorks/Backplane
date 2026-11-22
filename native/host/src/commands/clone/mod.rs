//! Clone a Git repository as a project: Add project's Clone from git, `/project <url>`, the phones' and the web's Clone. The clone runs here,
//! on the hub's machine, as a job (`Job`): `git clone --progress` in a process group of its own, its progress read from stderr, cancellable,
//! with the person's own git and gh credentials and SSH agent (no token ever passes through a client or a chat). When it is done the folder
//! is added as a project (`project.add`), a KiCad or Altium project inside is found (`.backplane.json` set when there are several), and,
//! when asked, the first thread is made. Every change of a job reaches the views (`bump_views`) and the data/1 clients (`{"t":"clone"}`).
use crate::hub::Hub;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub mod gh;
pub mod view;
#[cfg(test)] mod tests;

/// A repository as typed: what `git clone` is given (`urls`, tried in order: the next only when the one before was refused for access),
/// the remote as projects are keyed (`origin`, `host/owner/repo`), the folder name it gets (`name`) and, on GitHub, `owner/repo`.
#[derive(Clone, Debug, PartialEq)]
pub struct Repo { pub urls: Vec<String>, pub origin: String, pub name: String, pub github: String }

fn seg_ok(s: &str) -> bool { !s.is_empty() && s != "." && s != ".." && !s.starts_with('-') && s.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) }
/// A folder name a clone can take: no separator, no `.`/`..`, none of the characters some systems refuse.
pub fn name_ok(n: &str) -> bool { !n.trim().is_empty() && n != "." && n != ".." && n.len() <= 255 && !n.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*')) }
fn strip_git(s: &str) -> &str { s.strip_suffix(".git").unwrap_or(s) }
fn name_of(path: &str) -> Option<String> { name_of_in(path, false) }
fn name_of_in(path: &str, backslash: bool) -> Option<String> {
    let trimmed = if backslash { path.trim_end_matches(['/', '\\']) } else { path.trim_end_matches('/') };
    let last = if backslash { trimmed.rsplit(['/', '\\', ':']).next()? } else { trimmed.rsplit(['/', ':']).next()? };
    let n = strip_git(last).to_string();
    (!n.is_empty() && n != "." && n != ".." && !n.contains(['\\', '\0'])).then_some(n)
}

/// What the repository field takes: an https or ssh URL (`https://host/o/r(.git)`, `ssh://git@host/o/r`, `git@host:o/r.git`), a bare
/// `host.tld/o/r`, GitHub's `owner/repo`, or a `file://` URL (a repository on this machine). None for anything else (a folder's path).
pub fn parse(text: &str) -> Option<Repo> {
    let t = text.trim().trim_end_matches('/');
    if t.is_empty() || t.chars().any(|c| c.is_whitespace() || c.is_control()) || t.starts_with('-') { return None }
    let gh_of = |origin: &str| origin.strip_prefix("github.com/").filter(|r| r.split('/').count() == 2).map(str::to_string).unwrap_or_default();
    if let Some(p) = t.strip_prefix("file://") {
        if !crate::plat::fs::is_abs(p) && !p.starts_with('/') { return None }
        return Some(Repo { urls: vec![t.to_string()], origin: t.to_string(), name: name_of_in(p, cfg!(windows))?, github: String::new() });
    }
    let url = ["https://", "http://", "ssh://", "git://"].iter().any(|s| t.starts_with(s));
    // a password or token in the address would be shown and kept: never taken (the credential helpers and the agent sign in)
    if url && t.split_once("://").map_or(false, |(_, r)| r.split('/').next().unwrap_or("").rsplit_once('@').map_or(false, |(u, _)| u.contains(':'))) { return None }
    let scp = !url && !t.contains("://") && t.split_once(':').is_some_and(|(h, p)| h.contains('@') && !h.contains('/') && !p.is_empty());
    if url || scp {
        let origin = crate::mirror::normalize_origin(t);
        if origin.split('/').count() < 3 || origin.contains("..") { return None }
        return Some(Repo { urls: vec![t.to_string()], name: name_of(&origin)?, github: gh_of(&origin), origin });
    }
    if t.contains(':') || t.starts_with(['~', '/', '.']) || t.contains("..") { return None }
    let parts: Vec<&str> = t.split('/').collect();
    // `owner/repo`: GitHub's, https first (gh's credentials), then ssh (the agent's keys)
    if parts.len() == 2 && parts.iter().all(|p| seg_ok(p)) && !parts[0].contains('.') {
        let (o, r) = (parts[0], strip_git(parts[1]));
        return Some(Repo { urls: vec![format!("https://github.com/{o}/{r}.git"), format!("git@github.com:{o}/{r}.git")], origin: format!("github.com/{o}/{r}"), name: name_of(r)?, github: format!("{o}/{r}") });
    }
    // `host.tld/owner/repo` (the form remotes are keyed by): https, then ssh
    if parts.len() >= 3 && parts[0].contains('.') && parts[0].chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') && parts[1..].iter().all(|p| seg_ok(p)) {
        let host = parts[0].to_lowercase(); let path = strip_git(&parts[1..].join("/")).to_string();
        let origin = format!("{host}/{path}");
        return Some(Repo { urls: vec![format!("https://{host}/{path}.git"), format!("git@{host}:{path}.git")], name: name_of(&path)?, github: gh_of(&origin), origin });
    }
    None
}
/// A repository named where a folder could be meant too (`/project <word>`): only an address that cannot be a folder (a URL, `git@host:...`,
/// `host.tld/o/r`), never `owner/repo`, which is also a folder under the home.
pub fn parse_address(text: &str) -> Option<Repo> {
    let t = text.trim();
    let two = !t.contains(':') && t.trim_end_matches('/').split('/').count() == 2;
    if two { return None }
    parse(t)
}
/// What is shown for a repository: `owner/repo` on GitHub, else its origin.
pub fn shown(r: &Repo) -> String { if r.github.is_empty() { r.origin.clone() } else { r.github.clone() } }

// ---- where it goes ----

/// The folder clones went into last (set by each clone; read once from the log at start).
static LAST_PARENT: Mutex<String> = Mutex::new(String::new());
/// Tests only: `LAST_PARENT` is one per process (one hub in the product), but the test binary runs many hubs side by side. A test that clones
/// (`clone.start` sets it) or that reads the default (and so needs it empty) holds this for its run; a reader clears it first (`clear`).
#[cfg(test)] static TEST_PARENT: Mutex<()> = Mutex::new(());
#[cfg(test)] pub(crate) fn test_parent(clear: bool) -> std::sync::MutexGuard<'static, ()> {
    let g = TEST_PARENT.lock().unwrap_or_else(|e| e.into_inner()); if clear { LAST_PARENT.lock().unwrap_or_else(|e| e.into_inner()).clear(); } g
}
/// The folder a clone goes into unless the person picks another: the one used last, else ~/Documents, else the home folder.
pub fn default_parent(home: &str) -> String {
    let last = LAST_PARENT.lock().unwrap().clone();
    if !last.is_empty() && Path::new(&last).is_dir() { return last }
    let docs = Path::new(home).join("Documents");
    if docs.is_dir() { docs.to_string_lossy().into_owned() } else { home.to_string() }
}
/// How a destination folder stands: not there (`Free`), an empty folder (`Empty`: cloned into), a clone of this repository already (`Same`:
/// added as it is), or anything else there (`Busy`: refused, with the words).
#[derive(Debug, PartialEq)]
pub enum Dest { Free, Empty, Same, Busy(String) }
pub fn dest_state(dest: &Path, origin: &str, home: &str) -> Dest {
    let shown = tilde(&dest.to_string_lossy(), home);
    match std::fs::symlink_metadata(dest) {
        Err(_) => Dest::Free,
        Ok(m) if m.is_dir() => {
            let empty = std::fs::read_dir(dest).map(|mut r| r.next().is_none()).unwrap_or(false);
            if empty { Dest::Empty }
            else if !origin.is_empty() && dest.join(".git").exists() && crate::mirror::origin_of_root(&dest.to_string_lossy()) == origin { Dest::Same }
            else { Dest::Busy(format!("{shown} already exists and isn't empty. Pick another folder or name.")) }
        }
        Ok(_) => Dest::Busy(format!("{shown} is a file. Pick another folder or name.")),
    }
}
/// The folder a repository is cloned into by default: `<parent>/<name>`, or `<name>-2`, `-3`... past folders that hold something else.
pub fn default_dest(parent: &str, r: &Repo, home: &str) -> PathBuf {
    let base = Path::new(parent);
    for n in 1..100 {
        let d = base.join(if n == 1 { r.name.clone() } else { format!("{}-{n}", r.name) });
        if !matches!(dest_state(&d, &r.origin, home), Dest::Busy(_)) { return d }
    }
    base.join(&r.name)
}
/// A path as the person reads it: "~/..." under the home.
pub fn tilde(p: &str, home: &str) -> String { tilde_in(p, home, cfg!(windows)) }
fn tilde_in(p: &str, home: &str, win: bool) -> String {
    let p_s = if win { p.replace('\\', "/") } else { p.to_string() };
    let h_s = if win { home.replace('\\', "/") } else { home.to_string() };
    let h_trim = h_s.trim_end_matches('/');
    let drive_root = win && h_trim.len() == 2 && h_trim.as_bytes()[0].is_ascii_alphabetic() && h_trim.as_bytes()[1] == b':' && h_s.len() > 2;
    let h = if drive_root { &h_s[..3] } else { h_trim };
    if h.is_empty() { return p_s }
    let rest = if win {
        p_s.get(..h.len()).filter(|pre| pre.eq_ignore_ascii_case(h)).and_then(|_| p_s.get(h.len()..))
    } else {
        p_s.strip_prefix(h)
    };
    match rest {
        Some("") => "~".into(),
        Some(r) if drive_root => format!("~/{}", r.trim_start_matches('/')),
        Some(r) if r.starts_with('/') => format!("~{r}"),
        _ => p_s,
    }
}
fn norm_path(p: &str, win: bool) -> String {
    let s = if win { p.replace('\\', "/") } else { p.to_string() };
    if s.is_empty() { return String::new() }
    let prefix = if win && s.starts_with("//") { "//" } else if s.starts_with('/') { "/" } else { "" };
    let parts: Vec<&str> = s[prefix.len()..].split('/').filter(|seg| !seg.is_empty() && *seg != ".").collect();
    if parts.is_empty() { return prefix.to_string() }
    if win && prefix.is_empty() && parts.len() == 1 && parts[0].len() == 2 && parts[0].as_bytes()[0].is_ascii_alphabetic() && parts[0].as_bytes()[1] == b':' && s.contains(":/") {
        return format!("{}/", parts[0]);
    }
    format!("{prefix}{}", parts.join("/"))
}
fn path_eq_in(a: &str, b: &str, win: bool) -> bool {
    let (a, b) = (norm_path(a, win), norm_path(b, win));
    if win { a.eq_ignore_ascii_case(&b) } else { a == b }
}
fn path_eq(a: &str, b: &str) -> bool { path_eq_in(a, b, cfg!(windows)) }
/// Canonicalize the deepest existing ancestor of `p` and append any non-existent tail (handles 8.3 short paths, symlinks/junctions, and `.` segments before a clone creates its target folder).
fn canon_dest(p: &Path) -> String {
    let mut tail: Vec<std::ffi::OsString> = vec![];
    let mut cur = p;
    loop {
        if let Ok(c) = crate::plat::fs::canonicalize(cur) {
            return tail.into_iter().rev().fold(c, |a, b| a.join(b)).to_string_lossy().into_owned();
        }
        let Some(name) = cur.file_name() else { return p.to_string_lossy().into_owned() };
        tail.push(name.to_os_string());
        let Some(parent) = cur.parent() else { return p.to_string_lossy().into_owned() };
        cur = parent;
    }
}
/// A typed destination as an absolute path ("~/x" under the home); Err: the words.
pub fn resolve(p: &str, home: &str) -> Result<PathBuf, String> {
    let p = p.trim();
    if p.is_empty() { return Err("Say where to clone it.".into()) }
    let rest = p.strip_prefix("~/").or_else(|| cfg!(windows).then(|| p.strip_prefix("~\\")).flatten());
    let x = if p == "~" { PathBuf::from(home) } else if let Some(r) = rest { Path::new(home).join(r) } else { PathBuf::from(p) };
    if !x.is_absolute() { return Err("The folder must be a full path (or start with ~/).".into()) }
    if x.components().any(|c| matches!(c, std::path::Component::ParentDir)) { return Err("The folder's path can't hold \"..\".".into()) }
    Ok(x)
}

// ---- jobs ----

#[derive(Clone, Debug, PartialEq)]
pub enum State { Running, Done, Failed, Cancelled }
impl State { pub fn word(&self) -> &'static str { match self { State::Running => "running", State::Done => "done", State::Failed => "failed", State::Cancelled => "cancelled" } } }

#[derive(Clone)]
pub struct Job {
    pub id: String, pub repo: String, pub origin: String, pub dest: String, dest_key: String, pub name: String, pub state: State,
    pub phase: String, pub pct: Option<u32>, pub detail: String, pub error: String,
    /// refused for access: the client offers Sign in to GitHub (`github`: the repository is GitHub's)
    pub auth: bool, pub github: bool,
    /// a note the person reads before the window goes to the clone (LFS, `.backplane.json` set...): Open waits for them
    pub hold: bool,
    pub notes: Vec<String>, pub project: String, pub thread: String, pub started: u64, pub ended: u64,
    /// what the job was started with, for Try again
    pub spec: Value,
    cancel: Arc<tokio::sync::Notify>, cancelled: Arc<AtomicBool>,
}
impl Job {
    pub fn json(&self, home: &str) -> Value {
        json!({"id": self.id, "repo": self.repo, "origin": self.origin, "dest": self.dest, "destShown": tilde(&self.dest, home), "name": self.name,
            "state": self.state.word(), "phase": self.phase, "pct": self.pct, "detail": self.detail, "error": self.error, "auth": self.auth,
            "signin": self.auth && self.github, "notes": self.notes, "hold": self.hold, "project": self.project, "thread": self.thread, "started": self.started, "ended": self.ended})
    }
}
static JOBS: Mutex<Vec<Job>> = Mutex::new(Vec::new());
static HUB: OnceLock<Weak<Hub>> = OnceLock::new();
static SEQ: AtomicU64 = AtomicU64::new(1);
pub fn hub() -> Option<Arc<Hub>> { HUB.get().and_then(Weak::upgrade) }
fn home() -> String { crate::plat::dirs::home_or_empty() }

/// A fresh job id (`c<ms>-<n>`), unique in this hub.
pub fn new_id() -> String { format!("c{}-{}", crate::hub::now_ms(), SEQ.fetch_add(1, Relaxed)) }
fn id_ok(id: &str) -> bool { !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') }
pub fn job(id: &str) -> Option<Job> { JOBS.lock().unwrap().iter().find(|j| j.id == id).cloned() }
pub fn job_json(id: &str) -> Option<Value> { job(id).map(|j| j.json(&home())) }
pub fn jobs_json() -> Value { let h = home(); json!(JOBS.lock().unwrap().iter().map(|j| j.json(&h)).collect::<Vec<_>>()) }

/// Last time each job told the clients (progress is told at most every `TELL_EVERY`; a new phase or state at once).
static TOLD: Mutex<Vec<(String, std::time::Instant, String)>> = Mutex::new(Vec::new());
const TELL_EVERY: std::time::Duration = std::time::Duration::from_millis(200);
fn tell(id: &str, force: bool) {
    let Some(j) = job(id) else { return };
    let key = format!("{}|{}", j.state.word(), j.phase);
    { let mut t = TOLD.lock().unwrap();
      match t.iter_mut().find(|x| x.0 == id) {
          Some(x) => { if !force && x.2 == key && x.1.elapsed() < TELL_EVERY { return } x.1 = std::time::Instant::now(); x.2 = key; }
          None => { if t.len() > 64 { t.remove(0); } t.push((id.to_string(), std::time::Instant::now(), key)); }
      } }
    if let Some(h) = hub() { h.bump_views(); let _ = h.frames.send(json!({"t": "clone", "job": j.json(&home())})); }
}
fn update(id: &str, f: impl FnOnce(&mut Job)) { if let Some(j) = JOBS.lock().unwrap().iter_mut().find(|j| j.id == id) { f(j) } }

/// What starts a job. `dest` empty: `<default parent>/<name>` (`name` empty: the repository's).
#[derive(Clone, Debug, Default)]
pub struct Start { pub id: String, pub repo: String, pub dest: String, pub parent: String, pub name: String, pub thread: bool, pub text: String }
impl Start {
    pub fn of(p: &Value) -> Start {
        let s = |k: &str| p[k].as_str().unwrap_or("").trim().to_string();
        Start { id: s("id"), repo: s("repo"), dest: s("dest"), parent: s("parent"), name: s("name"), thread: p["thread"].as_bool().unwrap_or(true), text: s("text") }
    }
    fn spec(&self) -> Value { json!({"repo": self.repo, "dest": self.dest, "parent": self.parent, "name": self.name, "thread": self.thread, "text": self.text}) }
}
/// Where a start goes: `dest`, else `parent`/`name`, else the default. Err: the words.
pub fn dest_of(s: &Start, r: &Repo, home: &str) -> Result<PathBuf, String> {
    if !s.dest.is_empty() { return resolve(&s.dest, home) }
    let name = if s.name.is_empty() { r.name.clone() } else { s.name.clone() };
    if !name_ok(&name) { return Err(format!("{name:?} can't be a folder's name.")) }
    if s.parent.is_empty() && s.name.is_empty() { return Ok(default_dest(&default_parent(home), r, home)) }
    let parent = if s.parent.is_empty() { PathBuf::from(default_parent(home)) } else { resolve(&s.parent, home)? };
    Ok(parent.join(name))
}

/// Start a clone. The job is there (running) when this returns; Err: refused before anything ran (the words).
pub fn start(hub: &Arc<Hub>, s: Start) -> Result<Value, String> {
    let _ = HUB.set(Arc::downgrade(hub));
    // refused with an id the client chose (it follows that job already): a failed job of that id says why
    start_in(hub, s.clone()).map_err(|why| {
        if id_ok(&s.id) {
            let r = parse(&s.repo); let mut spec = s.spec(); spec["repo"] = json!(redact(&s.repo));
            let mut g = JOBS.lock().unwrap();
            if g.iter().any(|j| j.id == s.id) { return why }
            push(&mut g, Job { id: s.id.clone(), repo: r.as_ref().map(shown).unwrap_or_else(|| redact(&s.repo)), origin: r.as_ref().map(|r| r.origin.clone()).unwrap_or_default(), dest: s.dest.clone(), dest_key: String::new(), name: String::new(),
                state: State::Failed, phase: String::new(), pct: None, detail: String::new(), error: why.clone(), auth: false, github: false, hold: false, notes: vec![],
                project: String::new(), thread: String::new(), started: crate::hub::now_ms(), ended: crate::hub::now_ms(), spec, cancel: Arc::new(tokio::sync::Notify::new()), cancelled: Arc::new(AtomicBool::new(false)) });
            drop(g); tell(&s.id, true);
        }
        why
    })
}
/// Keep a job (the running ones and the last 16 others).
fn push(g: &mut Vec<Job>, j: Job) {
    while g.iter().filter(|j| j.state != State::Running).count() >= 16 { if let Some(i) = g.iter().position(|j| j.state != State::Running) { g.remove(i); } else { break } }
    g.push(j);
}
fn start_in(hub: &Arc<Hub>, s: Start) -> Result<Value, String> {
    let r = parse(&s.repo).ok_or_else(|| not_an_address(&s.repo))?;
    let home = home();
    let dest = dest_of(&s, &r, &home)?;
    let dest_s = dest.to_string_lossy().into_owned();
    let dest_key = canon_dest(&dest);
    let st = dest_state(&dest, &r.origin, &home);
    if let Dest::Busy(why) = &st { return Err(why.clone()) }
    let id = if id_ok(&s.id) { s.id.clone() } else { new_id() };
    {
        let mut g = JOBS.lock().unwrap();
        if g.iter().any(|j| j.state == State::Running && (path_eq(&j.dest, &dest_s) || path_eq(&j.dest_key, &dest_key) || j.id == id)) { return Err(format!("{} is being cloned already.", tilde(&dest_s, &home))) }
        if g.iter().any(|j| j.id == id) { return Err("That clone was started already.".into()) }
        push(&mut g, Job { id: id.clone(), repo: shown(&r), origin: r.origin.clone(), dest: dest_s.clone(), dest_key, name: dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            state: State::Running, phase: "Starting".into(), pct: None, detail: String::new(), error: String::new(), auth: false, github: !r.github.is_empty(), hold: false, notes: vec![],
            project: String::new(), thread: String::new(), started: crate::hub::now_ms(), ended: 0, spec: s.spec(), cancel: Arc::new(tokio::sync::Notify::new()), cancelled: Arc::new(AtomicBool::new(false)) });
    }
    if let Some(p) = dest.parent() { let p = p.to_string_lossy().into_owned(); *LAST_PARENT.lock().unwrap() = p.clone(); let h = hub.clone(); tokio::spawn(async move { let _ = h.commit(&json!({"$":"SettingSet","key":"clone.parent","value":p,"at":crate::hub::now_ms() / 1000})).await; }); }
    let (h, id2) = (hub.clone(), id.clone());
    tokio::spawn(async move { run(h, id2, r, dest, st == Dest::Same, s).await });
    tell(&id, true);
    Ok(job_json(&id).unwrap_or(Value::Null))
}
/// Stop a running clone (its whole process group); what it had made goes.
pub fn cancel(id: &str) -> bool {
    let Some(j) = job(id) else { return false };
    // the clone is published and being added: too late to undo (Cancel would say it stopped while the project appears)
    let mut ok = false;
    update(id, |j| if j.state == State::Running && j.phase != ADDING { j.cancelled.store(true, Relaxed); ok = true; });
    if ok { j.cancel.notify_one(); }
    ok
}
/// Start a finished job again, as it was started (Try again).
pub fn retry(hub: &Arc<Hub>, id: &str) -> Result<Value, String> {
    let j = job(id).ok_or("no such clone")?;
    if j.state == State::Running { return Err("It's still running.".into()) }
    let mut s = Start::of(&j.spec); s.dest = j.dest.clone(); s.id = new_id();
    start(hub, s)
}

fn fail(id: &str, why: String, auth: bool) { update(id, |j| { j.state = State::Failed; j.error = why; j.auth = auth; j.ended = crate::hub::now_ms(); j.pct = None; }); tell(id, true); }

const ADDING: &str = "Adding the project";
fn cancelled(id: &str) { update(id, |j| { j.state = State::Cancelled; j.ended = crate::hub::now_ms(); j.pct = None; j.phase = "Cancelled".into(); }); tell(id, true); }

/// The clone goes into a folder of its own beside the destination (`.bp-clone-<job>`, made here and only here, so a failure or Cancel
/// removes only what it made), and is moved to the destination once complete: then only if the destination is still not there or still empty.
async fn run(hub: Arc<Hub>, id: String, r: Repo, dest: PathBuf, same: bool, s: Start) {
    let Some(j) = job(&id) else { return };
    if !same {
        let Some(parent) = dest.parent().map(Path::to_path_buf) else { return fail(&id, "That folder has no parent.".into(), false) };
        if let Err(e) = std::fs::create_dir_all(&parent) { return fail(&id, format!("Couldn't make {}: {e}", tilde(&parent.to_string_lossy(), &home())), false) }
        let stage = parent.join(format!(".bp-clone-{id}"));
        let mut last = (String::new(), false);
        let mut done = false;
        for (n, url) in r.urls.iter().enumerate() {
            if let Err(e) = std::fs::create_dir(&stage) { return fail(&id, format!("Couldn't make a folder in {}: {e}", tilde(&parent.to_string_lossy(), &home())), false) }
            update(&id, |j| { j.phase = "Connecting".into(); j.pct = None; j.detail = String::new(); }); tell(&id, true);
            let mut cmd = git_cmd(url);
            cmd.args(["clone", "--progress", "--recurse-submodules", "--", url]).arg(&stage);
            match watch(&id, cmd, &j.cancel, &j.cancelled).await {
                Ok(()) => { done = true; break }
                Err(None) => { let _ = std::fs::remove_dir_all(&stage); return cancelled(&id) }
                Err(Some(tail)) => {
                    let _ = std::fs::remove_dir_all(&stage);
                    let host = r.origin.split('/').next().unwrap_or("").to_string();
                    last = explain(&tail, &shown(&r), &host);
                    // refused for access over https: the ssh address may be let in by the agent's key (and the other way round)
                    if !(last.1 && n + 1 < r.urls.len()) { break }
                }
            }
        }
        if !done { return fail(&id, last.0, last.1) }
        // Git LFS: the large files, after the checkout (the clone itself skipped them, so its progress is the repository's)
        if uses_lfs(&stage) {
            if lfs_installed().await {
                update(&id, |j| { j.phase = "Downloading large files (LFS)".into(); j.pct = None; j.detail = String::new(); }); tell(&id, true);
                let mut cmd = git_cmd(&r.urls[0]); cmd.current_dir(&stage).args(["lfs", "pull"]);
                match watch(&id, cmd, &j.cancel, &j.cancelled).await {
                    Ok(()) => {}
                    Err(None) => { let _ = std::fs::remove_dir_all(&stage); return cancelled(&id) }
                    Err(Some(tail)) => { let w = redact(tail.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim()); update(&id, |j| { j.hold = true; j.notes.push(format!("Its large files (Git LFS) didn't download ({w}). Run git lfs pull in the folder to get them.")) }); }
                }
            } else {
                update(&id, |j| { j.hold = true; j.notes.push("It keeps large files in Git LFS, which isn't installed on this machine: those files are placeholders until you install git-lfs and run git lfs pull in the folder.".into()) });
            }
        }
        // from here on it is not undone: Cancel is refused once the phase says so (`cancel`)
        let mut go = false;
        update(&id, |j| if !j.cancelled.load(Relaxed) { j.phase = ADDING.into(); j.pct = None; j.detail = String::new(); go = true; });
        if !go { let _ = std::fs::remove_dir_all(&stage); return cancelled(&id) }
        tell(&id, true);
        // an empty folder the person picked gives way (remove_dir removes only an empty one); anything there now stays, and the clone does not go in
        let gave_way = dest.is_dir() && std::fs::remove_dir(&dest).is_ok();
        let moved = if std::fs::symlink_metadata(&dest).is_ok() { Err(None) } else { std::fs::rename(&stage, &dest).map_err(Some) };
        if let Err(e) = moved {
            if gave_way { let _ = std::fs::create_dir(&dest); } // the empty folder the person picked is put back
            let _ = std::fs::remove_dir_all(&stage);
            let shown_dest = tilde(&dest.to_string_lossy(), &home());
            return fail(&id, match e { None => format!("{shown_dest} was taken while it cloned (something is in it now). Pick another folder or name."), Some(e) => format!("Couldn't move the clone into {shown_dest}: {e}") }, false)
        }
    } else {
        let mut go = false;
        update(&id, |j| if !j.cancelled.load(Relaxed) { j.hold = true; j.phase = ADDING.into(); j.notes.push(format!("{} was already a clone of {}: it's added as it is.", tilde(&dest.to_string_lossy(), &home()), shown(&r))); go = true; });
        if !go { return cancelled(&id) }
        tell(&id, true);
    }
    // the design inside: said, and `.backplane.json` set when there is more than one to pick from
    let (dest2, name) = (dest.clone(), r.name.clone());
    let notes = tokio::task::spawn_blocking(move || design(&dest2, &name)).await.unwrap_or_default();
    update(&id, |j| { if notes.len() > 1 || notes.iter().any(|n| n.contains(".backplane.json")) { j.hold = true; } j.notes.extend(notes) });
    match add(&hub, &dest, s.thread, &s.text).await {
        Ok((project, thread, note)) => {
            update(&id, |j| { j.project = project; j.thread = thread; if !note.is_empty() { j.hold = true; j.notes.push(note) } j.state = State::Done; j.phase = "Done".into(); j.pct = Some(100); j.ended = crate::hub::now_ms(); });
            tell(&id, true);
        }
        Err(e) => fail(&id, format!("It's cloned into {}, but it wasn't added as a project: {e}", tilde(&dest.to_string_lossy(), &home())), false),
    }
}

/// `git` with the person's credentials and no prompt: GitHub's https through gh (when it is here), ssh through the agent with no question
/// asked (BatchMode; a host seen the first time is taken, as `accept-new` does), the messages in English (they are read), LFS files later.
fn git_cmd(url: &str) -> tokio::process::Command {
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Net, "git");
    c.env("GIT_TERMINAL_PROMPT", "0").env("GCM_INTERACTIVE", "never").env("GIT_LFS_SKIP_SMUDGE", "1").env("LC_ALL", "C").env("LANGUAGE", "en");
    if std::env::var_os("GIT_SSH_COMMAND").is_none() && std::env::var_os("GIT_SSH").is_none() { c.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new"); }
    if std::env::var_os("SSH_AUTH_SOCK").is_none() { if let Some(sock) = agent_socket() { c.env("SSH_AUTH_SOCK", sock); } }
    if let Some(helper) = gh::credential_helper(url) { c.arg("-c").arg(helper); }
    c
}
/// An SSH agent this hub was started without (a hub started by a service manager has no SSH_AUTH_SOCK): the desktop's usual sockets.
fn agent_socket() -> Option<String> {
    let rt = std::env::var("XDG_RUNTIME_DIR").ok()?;
    ["ssh-agent.socket", "keyring/ssh", "gcr/ssh", "openssh_agent", "gnupg/S.gpg-agent.ssh"].iter().map(|s| format!("{rt}/{s}")).find(|p| Path::new(p).exists())
}
/// Run `cmd` for job `id`, its stderr read for progress. Ok: it succeeded; Err(None): cancelled (the group is ended); Err(Some(tail)): failed,
/// with the last lines it said.
async fn watch(id: &str, mut cmd: tokio::process::Command, cancel: &tokio::sync::Notify, cancelled: &AtomicBool) -> Result<(), Option<String>> {
    use tokio::io::AsyncReadExt;
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    crate::plat::proc::agent_group(&mut cmd); // its own group, which dies with the hub on Linux
    let mut child = cmd.spawn().map_err(|e| Some(format!("fatal: could not start git: {e}")))?;
    let pid = child.id().unwrap_or(0);
    if pid > 0 { crate::plat::proc::adopt(pid, true) }
    // the whole group: TERM, a moment, then KILL (git's helpers too: remote-https, index-pack, ssh), then the leader is reaped
    async fn stop(pid: u32, child: &mut tokio::process::Child) {
        // the leader is reaped only after the last signal, so the group's id is still its own when it is signalled
        if pid > 0 { crate::plat::proc::kill_group(pid, crate::plat::proc::TERM); }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        if pid > 0 { crate::plat::proc::kill_group(pid, crate::plat::proc::KILL); }
        let _ = child.wait().await;
    }
    let mut err = child.stderr.take();
    let mut tail: std::collections::VecDeque<String> = Default::default();
    let mut line: Vec<u8> = vec![]; let mut buf = vec![0u8; 8192];
    let take = |l: &[u8], tail: &mut std::collections::VecDeque<String>| {
        let l = String::from_utf8_lossy(l).trim().to_string(); if l.is_empty() { return }
        if let Some((phase, pct, detail)) = progress(&l) { update(id, |j| { j.phase = phase; j.pct = pct; j.detail = redact(&detail); }); tell(id, false); }
        else { if tail.len() >= 16 { tail.pop_front(); } tail.push_back(l); }
    };
    if cancelled.load(Relaxed) { stop(pid, &mut child).await; return Err(None) }
    while let Some(e) = err.as_mut() {
        tokio::select! {
            _ = cancel.notified() => { stop(pid, &mut child).await; return Err(None) }
            n = e.read(&mut buf) => match n {
                Ok(0) | Err(_) => { err = None; }
                Ok(n) => for b in &buf[..n] { if *b == b'\r' || *b == b'\n' { take(&line, &mut tail); line.clear(); } else if line.len() < 4096 { line.push(*b); } },
            },
        }
    }
    take(&line, &mut tail);
    let st = tokio::select! {
        _ = cancel.notified() => { stop(pid, &mut child).await; return Err(None) }
        s = child.wait() => s,
    };
    if cancelled.load(Relaxed) { return Err(None) }
    match st { Ok(s) if s.success() => Ok(()), _ => Err(Some(tail.into_iter().collect::<Vec<_>>().join("\n"))) }
}

/// A line of git's progress: (phase, percent, the rest: "12.40 MiB, 3.10 MiB/s"). `remote: Counting objects:  45% (9/20)`,
/// `Receiving objects:  45% (450/1000), 1.20 MiB | 600.00 KiB/s`, `Resolving deltas: 100% (5/5), done.`, `Updating files:  30% (3/10)`,
/// git-lfs's `Downloading LFS objects:  50% (1/2), 5 MB | 1 MB/s`; `Cloning into 'x'...` is "Connecting". None for any other line.
pub fn progress(line: &str) -> Option<(String, Option<u32>, String)> {
    let l = line.trim(); let l = l.strip_prefix("remote:").map(str::trim).unwrap_or(l);
    if l.starts_with("Cloning into") { return Some(("Connecting".into(), None, String::new())) }
    let (phase, rest) = l.split_once(':')?;
    let phase = phase.trim();
    if phase.is_empty() || phase.len() > 40 || !phase.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') || matches!(phase, "fatal" | "error" | "warning" | "hint" | "ERROR") { return None }
    let rest = rest.trim();
    if !rest.starts_with(|c: char| c.is_ascii_digit()) { return None }
    let pct = rest.split_once('%').and_then(|(n, _)| n.trim().parse::<u32>().ok()).map(|p| p.min(100));
    let detail = rest.split_once("), ").map(|(_, d)| d.trim().trim_end_matches("done.").trim().trim_end_matches(',').replace(" | ", ", ")).unwrap_or_default();
    Some((phase.to_string(), pct, detail))
}

/// Why a repository field is refused, quoting it only when it holds no password.
pub fn not_an_address(t: &str) -> String {
    if redact(t) != t { return "An address with a password or token in it isn't taken: sign in with gh or an SSH key instead, and paste the address without it.".into() }
    format!("{t:?} isn't a repository address: paste an https or ssh URL, or owner/repo.")
}
/// `scheme://user:secret@host` as `scheme://user@host`, wherever git put an address in what it said.
pub fn redact(s: &str) -> String {
    let mut out = String::new(); let mut rest = s;
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3); out.push_str(head);
        let end = tail.find(|c: char| c == '/' || c.is_whitespace() || c == '\'' ).unwrap_or(tail.len());
        let auth = &tail[..end];
        match auth.rsplit_once('@') { Some((u, h)) if u.contains(':') => { out.push_str(u.split(':').next().unwrap_or("")); out.push('@'); out.push_str(h); } _ => out.push_str(auth) }
        rest = &tail[end..];
    }
    out.push_str(rest); out
}
/// Why a clone failed, in the person's words, and whether it was refused for access (a sign-in may let it in).
pub fn explain(tail: &str, repo: &str, host: &str) -> (String, bool) {
    let tail = &redact(tail);
    let has = |w: &str| tail.to_lowercase().contains(&w.to_lowercase());
    let site = if host == "github.com" { "GitHub".to_string() } else { host.to_string() };
    if has("Repository not found") || has("not found") && has("repository") { return (format!("{repo} wasn't found. If it's private, sign in to {site} on this machine with an account that can see it."), true) }
    if has("Permission denied (publickey)") { return (format!("{site} didn't accept this machine's SSH key for {repo}. Sign in to {site}, or add the key to your account."), true) }
    if has("Authentication failed") || has("could not read Username") || has("could not read Password") || has("terminal prompts disabled") || has("Invalid username or password") || has("The requested URL returned error: 403") || has("The requested URL returned error: 401") {
        return (format!("{site} needs you to sign in to clone {repo}."), true)
    }
    if has("Host key verification failed") { return (format!("SSH couldn't check {host}'s host key. Connect to it once with ssh in a terminal, then try again."), false) }
    if has("Could not resolve host") || has("Could not resolve hostname") || has("Network is unreachable") || has("Connection timed out") || has("Failed to connect") { return (format!("Couldn't reach {host}. Check the network, then try again."), false) }
    if has("No space left on device") { return ("The disk is full: free some space, then try again.".into(), false) }
    if has("does not appear to be a git repository") { return (format!("{repo} isn't a Git repository."), false) }
    let last = tail.lines().rev().find(|l| { let l = l.trim(); l.starts_with("fatal:") || l.starts_with("error:") }).or_else(|| tail.lines().rev().find(|l| !l.trim().is_empty())).unwrap_or("git stopped");
    (format!("The clone failed: {}", last.trim().trim_start_matches("fatal:").trim_start_matches("error:").trim()), false)
}

fn uses_lfs(dest: &Path) -> bool {
    std::fs::read_to_string(dest.join(".gitattributes")).map_or(false, |t| t.contains("filter=lfs")) || dest.join(".lfsconfig").is_file()
}
async fn lfs_installed() -> bool {
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, "git"); c.args(["lfs", "version"]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    matches!(tokio::time::timeout(std::time::Duration::from_secs(10), c.status()).await, Ok(Ok(s)) if s.success())
}

/// The KiCad or Altium project in a clone, said; when there are several, the one the hub's discovery would take (or the one named like the
/// repository) is set in `.backplane.json` so the project's viewers open it. A `.backplane.json` the repository has already is kept.
pub fn design(root: &Path, name: &str) -> Vec<String> {
    let mut out = vec![];
    if std::fs::symlink_metadata(root.join(".backplane.json")).is_ok() { return out } // the repository's own (a link too): kept, never followed
    let kicad = crate::files::kicad::project::found(root);
    let altium = if kicad.is_empty() { crate::files::kicad::project::altium_found(root) } else { vec![] };
    let (list, what) = if !kicad.is_empty() { (kicad, "KiCad") } else { (altium, "Altium") };
    let stem = |p: &str| p.rsplit('/').next().unwrap_or(p).rsplit_once('.').map(|x| x.0).unwrap_or(p).to_lowercase();
    match list.len() {
        0 => {}
        1 => out.push(format!("Found a {what} project: {}.", list[0])),
        n => {
            // discovery's own pick (a project in the top folder, else the shallowest), unless one is named like the repository
            let top = list.iter().find(|p| !p.contains('/'));
            let pick = top.or_else(|| list.iter().find(|p| stem(p) == name.to_lowercase())).unwrap_or(&list[0]).clone();
            let text = serde_json::to_string_pretty(&json!({"project": pick})).unwrap_or_default() + "\n";
            // made only when nothing is there (create_new follows no link and truncates nothing)
            let wrote = std::fs::OpenOptions::new().write(true).create_new(true).open(root.join(".backplane.json")).and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()));
            match wrote {
                Ok(()) => out.push(format!("Found {n} {what} projects; {pick} is the one Backplane opens (set in .backplane.json, change it there).")),
                Err(_) => out.push(format!("Found {n} {what} projects; Backplane opens {pick}.")),
            }
        }
    }
    out
}

/// Add the clone as a project (with its first thread when `thread`; `text`: that thread's first message). (project id, thread id, a note).
async fn add(hub: &Arc<Hub>, dest: &Path, thread: bool, text: &str) -> Result<(String, String, String), String> {
    let path = dest.to_string_lossy().into_owned();
    let mut c = crate::hub::Conn { local_view_socket: false, sent: vec![], client: 0, gen: String::new(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
    let r = hub.request_here(&mut c, &json!({"id": 0, "m": "project.add", "p": {"path": path, "starter": thread}})).await.into_iter().find(|v| v["t"] == "reply").unwrap_or(Value::Null);
    let want = crate::plat::fs::canonicalize(&path).map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|_| path.clone());
    let th = r["thread"].as_str().unwrap_or("").to_string();
    let exact_eq = |a: &str, b: &str| norm_path(a, cfg!(windows)) == norm_path(b, cfg!(windows));
    let find_in = |g: &crate::model::Model, th: &str| -> Option<String> {
        if !th.is_empty() {
            if let Some(t) = g.threads.get(th).filter(|t| !t.project.is_empty()) { return Some(t.project.clone()) }
        }
        g.projects.iter().find(|(_, x)| !x.removed && (exact_eq(&x.root, &want) || exact_eq(&x.root, &path))).map(|(id, _)| id.clone())
            .or_else(|| g.projects.iter().find(|(_, x)| !x.removed && (path_eq(&x.root, &want) || path_eq(&x.root, &path))).map(|(id, _)| id.clone()))
    };
    let find_canon = |roots: Vec<(String, String)>, want: String| -> Option<String> {
        roots.into_iter().find_map(|(id, root)| {
            crate::plat::fs::canonicalize(&root).ok().filter(|c| path_eq(&c.to_string_lossy(), &want)).map(|_| id)
        })
    };
    if r["ok"] != true {
        // a folder that is a project already (a clone added before): that project
        let (hit, roots) = {
            let g = hub.model().await;
            (find_in(&g, ""), g.projects.iter().filter(|(_, x)| !x.removed).map(|(id, x)| (id.clone(), x.root.clone())).collect::<Vec<_>>())
        };
        if let Some(id) = hit.or_else(|| find_canon(roots, want.clone())) { return Ok((id, String::new(), String::new())) }
        return Err(r["error"].as_str().unwrap_or("the project was not added").to_string())
    }
    let mut project = String::new();
    for _ in 0..40 {
        let (hit, roots) = {
            let g = hub.model().await;
            (find_in(&g, &th), g.projects.iter().filter(|(_, x)| !x.removed).map(|(id, x)| (id.clone(), x.root.clone())).collect::<Vec<_>>())
        };
        if let Some(id) = hit { project = id; break }
        if !roots.is_empty() {
            if let Some(id) = find_canon(roots, want.clone()) { project = id; break }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let mut note = String::new();
    if !th.is_empty() && !text.trim().is_empty() {
        let msg = format!("cl-{}", th.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>());
        let who = crate::api::Caller { gen: String::new(), client: 0 };
        let r = match hub.rpc_for("turn.start") { Some(h) => h.call(hub, &who, &json!(0), "turn.start", &json!({"thread": th, "text": text.trim(), "msg": msg})).await.into_iter().next().unwrap_or(Value::Null), None => json!({"ok": false, "error": "no agent module is running"}) };
        if r["ok"] != true { note = format!("The first message didn't go: {}", r["error"].as_str().unwrap_or("send it again")); }
    }
    Ok((project, th, note))
}

// ---- requests ----

/// `clone.*`, for every client (the wire and data/1 alike):
/// `clone.start {repo, dest | parent, name, thread, text, id}` the job; `clone.status {id}` one job (no id: all); `clone.cancel {id}`;
/// `clone.retry {id}`; `clone.repos {q}` the person's GitHub repositories that match; `clone.dest {repo, dest | parent, name}` where it would
/// go and whether it can; `clone.signin {cancel}` GitHub's device sign-in through gh (its code and address).
pub async fn call(hub: &Arc<Hub>, m: &str, p: &Value) -> Result<Value, String> {
    let _ = HUB.set(Arc::downgrade(hub));
    let id = p["id"].as_str().unwrap_or("");
    match m {
        "clone.start" => start(hub, Start::of(p)),
        "clone.status" => if id.is_empty() { Ok(json!({"jobs": jobs_json()})) } else { job_json(id).ok_or_else(|| "no such clone".into()) },
        "clone.cancel" => { let stopped = cancel(id); let mut j = job_json(id).ok_or("no such clone")?; j["cancelAccepted"] = json!(stopped); Ok(j) }
        "clone.retry" => retry(hub, id),
        // (data/1 answers it in a task of its own and may wait for the first read; the wire, which answers in line, is never held: `wait` false)
        "clone.repos" => Ok(gh::repos_wait(p["q"].as_str().unwrap_or(""), p["limit"].as_u64().unwrap_or(20) as usize, p["refresh"] == true, p["wait"] != false).await),
        "clone.dest" => Ok(dest_json(&Start::of(p))),
        "clone.signin" => Ok(if p["cancel"] == true { gh::signin_cancel() } else { gh::signin(p["restart"] == true) }),
        _ => Err("unknown method".into()),
    }
}
/// `clone.dest`: the default (or asked) destination of a repository and how it stands: `ok` (clone there), `same` (added as it is), or the refusal.
pub fn dest_json(s: &Start) -> Value {
    let home = home();
    let Some(r) = parse(&s.repo) else { return json!({"ok": false, "error": if s.repo.is_empty() { String::new() } else { not_an_address(&s.repo) }}) };
    let parent = default_parent(&home);
    let base = json!({"repo": shown(&r), "origin": r.origin, "name": r.name, "defaultParent": parent, "defaultParentShown": tilde(&parent, &home)});
    let mut out = base;
    match dest_of(s, &r, &home) {
        Err(e) => { out["ok"] = json!(false); out["error"] = json!(e); }
        Ok(d) => {
            let ds = d.to_string_lossy().into_owned();
            out["dest"] = json!(ds); out["destShown"] = json!(tilde(&ds, &home));
            out["parent"] = json!(d.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default());
            out["folder"] = json!(d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            match dest_state(&d, &r.origin, &home) {
                Dest::Busy(w) => { out["ok"] = json!(false); out["error"] = json!(w); }
                Dest::Same => { out["ok"] = json!(true); out["same"] = json!(true); }
                _ => { out["ok"] = json!(true); }
            }
        }
    }
    out
}

pub struct CloneRpc;
impl crate::api::RpcHandler for CloneRpc {
    fn methods(&self) -> &'static [&'static str] { &["clone.start", "clone.status", "clone.cancel", "clone.retry", "clone.repos", "clone.dest", "clone.signin"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a crate::api::Caller, id: &'a Value, m: &'a str, p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> {
        Box::pin(async move { let mut p = if p.is_object() { p.clone() } else { json!({}) }; if m == "clone.repos" { p["wait"] = json!(false); } match call(hub, m, &p).await { Ok(v) => vec![json!({"t":"reply","id":id,"ok":true,"result":v})], Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})] } })
    }
}
/// Registered with the commands lane; the folder clones went into last is read back from the log.
pub fn install(hub: &Arc<Hub>) {
    let _ = HUB.set(Arc::downgrade(hub));
    hub.register_rpc(Arc::new(CloneRpc));
    let h = hub.clone();
    tokio::spawn(async move { if let Some(p) = h.setting("clone.parent").await { let mut g = LAST_PARENT.lock().unwrap(); if g.is_empty() { *g = p; } } });
}
