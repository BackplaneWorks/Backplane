//! What the sidebar shows of the owner's other machines: one display-only `State` per link, folded from the mirrored events (already
//! renamed to `<link>~<id>`). It is NOT the policy state: autosettle, turn-end, star and the rest read only `statehost::policies()`, so
//! they never act on another machine's threads, and nothing here reaches the log. Fed beside `Hub::fold_mirror` (start replay, pushes,
//! pulls) and given again after a rebuild (`Hub::forget_mirror`), exactly as the mirrors are. Lean, so a mirror is folded, not kept.
use crate::state::State;
use serde_json::Value;
use std::{collections::HashMap, sync::{Arc, OnceLock, RwLock}};

/// The links whose rows are shown: `(link id, machine name)` in display order. The net lane answers: verified, sync not off.
pub type Names = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

#[derive(Default)]
pub struct Mirrors { st: RwLock<HashMap<String, State>>, names: OnceLock<Names>, origins: RwLock<HashMap<String, String>>, away: RwLock<std::collections::HashSet<String>>, here: RwLock<String>,
    /// the links that said their projects' origins (an origin of one of theirs is known, "" when it has none); `asked` wakes the watch for a new one
    known: RwLock<std::collections::HashSet<String>>, asked: tokio::sync::Notify,
    /// moves when what the merged rows are made of moves (an origin learned, a machine away or back): `publish` redraws the clients then
    gen: std::sync::atomic::AtomicU64, published: std::sync::atomic::AtomicU64 }

impl Mirrors {
    pub fn set_names(&self, n: Names) { let _ = self.names.set(n); }
    /// This machine's name as the linked machines know it (`botnet::name`), for the sidebar's machine switcher.
    pub fn set_here(&self, name: &str) { *self.here.write().unwrap() = name.to_string(); }
    pub fn here(&self) -> String { self.here.read().unwrap().clone() }
    /// Fold renamed events of one link.
    pub fn fold(&self, link: &str, events: &[Value]) {
        let mut g = self.st.write().unwrap();
        let st = g.entry(link.to_string()).or_insert_with(|| { let mut s = State::new(); s.lean = true; s });
        for e in events { st.apply(e); }
    }
    /// Whether the mirror of `link` holds that machine's bot `bot` (its own id there), live.
    pub fn has_bot(&self, link: &str, bot: &str) -> bool { self.st.read().unwrap().get(link).map_or(false, |s| s.bots_nav.contains_key(&format!("{link}~{bot}")) || s.bots_nav.contains_key(bot)) }
    /// A link's history was replaced.
    pub fn forget(&self, link: &str) { self.st.write().unwrap().remove(link); }
    /// Every mirror is given again after a rebuild.
    pub fn clear(&self) { self.st.write().unwrap().clear(); }
    /// `(link, thread count, project count)` of what is folded, for tests and diagnostics.
    pub fn counts(&self) -> Vec<(String, usize, usize)> {
        let mut v: Vec<_> = self.st.read().unwrap().iter().map(|(l, s)| (l.clone(), s.threads.len(), s.projects.len())).collect(); v.sort(); v
    }
    /// The shown links with their machine's name, in display order (none before the net lane says).
    pub fn names(&self) -> Vec<(String, String)> { self.names.get().map(|n| n()).unwrap_or_default() }
    /// The shown machines' states, in `names` order: `f(&[(machine, state)])`. A link with no name callback yet (or not in it) is not shown.
    pub fn shown<R>(&self, f: impl FnOnce(&[(String, &State)]) -> R) -> R {
        let names = self.names.get().map(|n| n()).unwrap_or_default();
        let g = self.st.read().unwrap();
        let v: Vec<(String, &State)> = names.iter().filter_map(|(l, n)| g.get(l).map(|s| (n.clone(), s))).collect();
        f(&v)
    }
}

/// A far project's origin while its machine has not said it yet (`Mirrors::origin`).
pub const UNKNOWN: &str = "?";

// ---- one project across machines: the same git repository (its normalized origin remote) -------------------------------------
/// A remote URL as one string for both forms and any user or port: `git@github.com:o/r.git`, `ssh://git@github.com:22/o/r`,
/// `https://u@GitHub.com/o/r/` are all `github.com/o/r`. The host is lower-cased; the path keeps its case. A remote on this
/// machine's disk (a path or `file://`) names nothing another machine can share: "".
pub fn normalize_origin(url: &str) -> String {
    let u = url.trim();
    if u.is_empty() || u.starts_with('/') || u.starts_with('.') || u.starts_with("file:") { return String::new() }
    let (host, path) = match u.split_once("://") {
        Some((_, rest)) => { let (auth, path) = rest.split_once('/').unwrap_or((rest, "")); (auth.rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("").to_string(), path.to_string()) }
        None => match u.split_once(':') { // scp-like `[user@]host:path` (a Windows drive letter is a path, not a host)
            Some((h, p)) if h.len() > 1 && !h.contains('/') => (h.rsplit('@').next().unwrap_or("").to_string(), p.to_string()),
            _ => return String::new(),
        },
    };
    let mut path = path.trim_matches('/').to_string();
    while let Some(p) = path.strip_suffix(".git") { path = p.trim_end_matches('/').to_string() }
    let host = host.trim().to_lowercase();
    if host.is_empty() || path.is_empty() { return String::new() }
    format!("{host}/{path}")
}
/// The `url` of `[remote "origin"]` in a git config file's text ("" when there is none).
pub fn origin_in_config(text: &str) -> String {
    let mut inside = false;
    for l in text.lines() {
        let t = l.trim();
        if t.starts_with('[') { inside = t.replace(' ', "").to_lowercase() == "[remote\"origin\"]"; continue }
        if inside { if let Some((k, v)) = t.split_once('=') { if k.trim().eq_ignore_ascii_case("url") { return v.trim().trim_matches('"').to_string() } } }
    }
    String::new()
}
/// The git config that holds a folder's remotes: the nearest `.git` above it (a worktree's or submodule's `.git` file leads to its
/// git dir, whose `commondir` leads to the shared one).
fn config_of(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut d = Some(root);
    while let Some(dir) = d {
        let g = dir.join(".git");
        if g.is_dir() { return Some(g.join("config")) }
        if g.is_file() {
            let t = std::fs::read_to_string(&g).ok()?; let gd = t.trim().strip_prefix("gitdir:")?.trim();
            let gd = if std::path::Path::new(gd).is_absolute() { std::path::PathBuf::from(gd) } else { dir.join(gd) };
            let common = std::fs::read_to_string(gd.join("commondir")).ok().map(|c| { let c = c.trim(); if std::path::Path::new(c).is_absolute() { std::path::PathBuf::from(c) } else { gd.join(c) } }).unwrap_or(gd);
            return Some(common.join("config"))
        }
        d = dir.parent();
    }
    None
}
/// A project folder's normalized origin ("" for none), read from its git config (no process), kept for a minute.
pub fn origin_of_root(root: &str) -> String {
    static CACHE: OnceLock<RwLock<HashMap<String, (std::time::Instant, String)>>> = OnceLock::new();
    if root.is_empty() { return String::new() }
    let c = CACHE.get_or_init(Default::default);
    if let Some((at, o)) = c.read().unwrap().get(root) { if at.elapsed() < std::time::Duration::from_secs(60) { return o.clone() } }
    let o = config_of(std::path::Path::new(root)).and_then(|p| std::fs::read_to_string(p).ok()).map(|t| normalize_origin(&origin_in_config(&t))).unwrap_or_default();
    c.write().unwrap().insert(root.to_string(), (std::time::Instant::now(), o.clone()));
    o
}
/// Which linked machines' projects are shown as one with a project of this machine: `far` project id -> `local` project id, when
/// their origins are the same (and not ""). `local` and `far` are `(id, normalized origin)`; of several local projects of one
/// repository, the first in `local`'s order takes the linked ones.
pub fn unify(local: &[(String, String)], far: &[(String, String)]) -> HashMap<String, String> {
    let mut by: HashMap<&str, &str> = HashMap::new();
    for (id, o) in local { if !o.is_empty() { by.entry(o.as_str()).or_insert(id.as_str()); } }
    far.iter().filter_map(|(f, o)| by.get(o.as_str()).map(|l| (f.clone(), l.to_string()))).collect()
}

impl Mirrors {
    /// The origins a linked machine says its projects have (machine_list's `origin`), by the mirror's id `<link>~<id>`; replaces that link's.
    pub fn set_origins(&self, link: &str, projects: &[(String, String)]) {
        let mut g = self.origins.write().unwrap();
        let pre = format!("{link}~");
        let before: Vec<(String, String)> = { let mut v: Vec<_> = g.iter().filter(|(k, _)| k.starts_with(&pre)).map(|(k, v)| (k.clone(), v.clone())).collect(); v.sort(); v };
        g.retain(|k, _| !k.starts_with(&pre));
        for (id, o) in projects { if !o.is_empty() { g.insert(format!("{pre}{id}"), o.clone()); } }
        let after: Vec<(String, String)> = { let mut v: Vec<_> = g.iter().filter(|(k, _)| k.starts_with(&pre)).map(|(k, v)| (k.clone(), v.clone())).collect(); v.sort(); v };
        if self.known.write().unwrap().insert(link.to_string()) || before != after { self.bump() }
    }
    /// A mirrored project's origin: "" when its machine said it has none, `UNKNOWN` ("?") while that machine has not said yet (asked at once:
    /// `present::merged` never keys such a project by machine/id, which would show one repository twice; it takes the registry's key or waits).
    pub fn origin(&self, far_id: &str) -> String {
        if let Some(o) = self.origins.read().unwrap().get(far_id) { return o.clone() }
        let link = far_id.split_once('~').map_or("", |x| x.0);
        if self.known.read().unwrap().contains(link) { return String::new() }
        self.asked.notify_one();
        UNKNOWN.to_string()
    }
    /// Whether a linked machine answered when last asked (the origin watch asks each one every minute).
    pub fn set_away(&self, link: &str, away: bool) { let mut g = self.away.write().unwrap(); let was = g.contains(link); if away { g.insert(link.to_string()); } else { g.remove(link); } if was != away { eprintln!("machines: {link} {}", if away { "is away" } else { "answers" }); drop(g); self.bump() } }
    pub fn bump(&self) { self.gen.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
    /// The merged rows changed with no log line of this hub saying so (an origin learned, a machine away or back): every client draws again
    /// (`mirrors.gen` info; the phones' lists are pushed by data/1 `far_pushes`).
    pub async fn publish(&self, hub: &crate::hub::Hub) {
        use std::sync::atomic::Ordering::Relaxed;
        let g = self.gen.load(Relaxed);
        if self.published.swap(g, Relaxed) != g { hub.set_info("mirrors.gen", &g.to_string()).await; }
    }
    /// The marker of a shown machine, by its name (`mark_of`).
    pub fn mark(&self, machine: &str) -> Value {
        let names = self.names.get().map(|n| n()).unwrap_or_default();
        let away = names.iter().any(|(l, n)| n == machine && self.away.read().unwrap().contains(l));
        mark_of(machine, away)
    }
}
/// A machine's name as short as a sidebar row shows it: its first DNS label (`framework-1.tail.ts.net` -> `framework-1`); a long
/// one is its initial.
pub fn short_name(name: &str) -> String {
    let first = name.split('.').next().unwrap_or("").trim();
    if first.chars().count() <= 12 { first.to_string() } else { first.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default() }
}
/// The marker on a thread of another machine, wherever threads are listed (sidebar rows, the Active view, the thread header):
/// `short` on the row, `name` in the header and on hover, `away` when that machine did not answer (drawn dimmer, "away" after it).
/// A thread of this machine has none. `label` ("on desktop") goes after a project's name (its sidebar header, the window's breadcrumb), `runs`
/// ("runs on desktop") under the composer: where an agent sent from there runs. One wording everywhere, the phones' too (docs/PHONE-DATA.md).
pub fn mark_of(machine: &str, away: bool) -> Value {
    if machine.is_empty() { return Value::Null }
    let short = short_name(machine);
    serde_json::json!({"name": machine, "short": short, "away": away, "label": format!("on {short}"), "runs": format!("runs on {short}")})
}

/// The thread a link names (`backplane://thread/<id>`), as this hub opens it: one of this machine by its id; one of a linked machine as
/// `<link>~<id>`, whether the link names that machine by its link id or by its name (`framework-1~t9`, the full or the short name), once
/// this hub mirrors it; this machine's own name before `~` is this machine. `far` is the shown machines' (name, mirror), `here` this one's name.
pub fn thread_ref(id: &str, here: &str, local: &dyn Fn(&str) -> bool, far: &[(String, &crate::state::State)]) -> Option<String> {
    let live = |st: &crate::state::State, k: &str| st.threads.get(k).map_or(false, |t| !t.deleted);
    let Some((m, t)) = id.split_once('~') else { return local(id).then(|| id.to_string()) };
    if m.is_empty() || t.is_empty() { return None }
    if far.iter().any(|(_, st)| live(st, id)) { return Some(id.to_string()) }
    let named = |n: &str| !n.is_empty() && (n.eq_ignore_ascii_case(m) || short_name(n).eq_ignore_ascii_case(m));
    if let Some((_, st)) = far.iter().find(|(n, _)| named(n)) {
        return st.threads.keys().find(|k| k.split_once('~').map_or(false, |(_, x)| x == t) && live(st, k)).cloned()
    }
    (named(here) && local(t)).then(|| t.to_string())
}

/// Ask one linked machine for its projects' origins (machine_list), bounded. Away only when it does not answer at all (no reply on the link): a
/// machine that answers and refuses (an older build, not shared) is up, and its origins come from the rows it sends (net/registry.rs).
pub async fn ask_origins(hub: &crate::hub::Hub, link: &str) {
    let t0 = std::time::Instant::now();
    let r = match tokio::time::timeout(std::time::Duration::from_secs(15), hub.far_call(link, "machine_list", "machine_list", &serde_json::json!({}))).await { Ok(r) => r, Err(_) => Err("that machine is away".to_string()) };
    match &r {
        Err(e) if e == "that machine is away" => { if !hub.mirrors().away.read().unwrap().contains(link) { eprintln!("machines: {link}: machine_list got no answer after {} ms", t0.elapsed().as_millis()); } hub.mirrors().set_away(link, true) }
        // (it answers but will not say: its origins are taken as none, unless its registry rows say them; never waited for)
        Err(e) => { hub.mirrors().set_away(link, false); if hub.mirrors().known.write().unwrap().insert(link.to_string()) { hub.mirrors().bump(); eprintln!("machines: {link} answers but not machine_list: {e}"); } }
        Ok(_) => hub.mirrors().set_away(link, false),
    }
    if let Ok((true, b)) = r {
        let ps: Vec<(String, String)> = b["projects"].as_array().into_iter().flatten().map(|p| (p["id"].as_str().unwrap_or("").to_string(), p["origin"].as_str().unwrap_or("").to_string())).filter(|(i, _)| !i.is_empty()).collect();
        hub.mirrors().set_origins(link, &ps);
    }
}
/// The shown links whose origins are not known yet (a link just made), and not away: what a list about to be drawn asks for first.
pub fn unknown_links(hub: &crate::hub::Hub) -> Vec<String> {
    let m = hub.mirrors(); let k = m.known.read().unwrap(); let a = m.away.read().unwrap();
    m.names().into_iter().map(|(l, _)| l).filter(|l| !k.contains(l) && !a.contains(l)).collect()
}
/// A machine known only from the registry (no mirror here yet) that is linked and answers is on, not off: its chips say so (`present::merged`).
pub fn live_chips(m: &Mirrors, chips: &mut [Value]) {
    let names = m.names(); let away = m.away.read().unwrap();
    for c in chips.iter_mut() {
        if c["state"] != "off" { continue }
        let n = c["name"].as_str().unwrap_or("");
        if names.iter().any(|(l, nm)| nm == n && !away.contains(l)) { c["state"] = serde_json::json!("on"); c["dim"] = serde_json::json!(false); c["away"] = serde_json::json!(false); c["note"] = serde_json::json!(""); }
    }
}
/// A machine's origins as its own registry rows say them (`registry.put` from it: the path that works when this hub cannot call it).
impl Mirrors { pub fn origins_from_rows(&self, link: &str, rows: &Value) {
    let ps: Vec<(String, String)> = rows["projects"].as_array().into_iter().flatten().map(|p| (p["id"].as_str().unwrap_or("").to_string(), p["origin"].as_str().unwrap_or("").to_string())).filter(|(i, _)| !i.is_empty()).collect();
    if !ps.is_empty() { self.set_origins(link, &ps) }
} }

/// Ask each linked machine for its projects' origins (machine_list) now and every minute, so the sidebar can show a repository
/// that is on both machines as one project, and sync this hub's rows with the registry on the home. Display only: nothing here is a decision.
pub fn watch_origins(hub: Arc<crate::hub::Hub>) {
    tokio::spawn(async move {
        hub.mirrors().set_here(&crate::net::botnet::name().await);
        loop {
            for (link, _, _) in hub.far_machines() { ask_origins(&hub, &link).await; }
            // the registry on the home: this hub's rows go there, the merged list comes back (net/registry.rs)
            let _ = tokio::time::timeout(std::time::Duration::from_secs(30), crate::net::registry::tick(&hub, &hub.mirrors().here())).await;
            hub.mirrors().publish(&hub).await;
            // every minute, or at once when the sidebar met a project whose machine has not said its origin yet (a link just made)
            // (an early ask at most every 5 s: a machine that does not answer keeps its origins unknown, and every drawing asks again)
            let began = std::time::Instant::now();
            // (a machine linked just now is asked, and sent this hub's rows, within 2 s: its copies are keyed and listed before its history arrives)
            let links: std::collections::HashSet<String> = hub.far_machines().into_iter().map(|x| x.0).collect();
            let new_link = async { loop { tokio::time::sleep(std::time::Duration::from_secs(2)).await; if hub.far_machines().into_iter().any(|x| !links.contains(&x.0)) { break } } };
            tokio::select! { _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}, _ = hub.mirrors().asked.notified() => {}, _ = new_link => {} }
            let wait = std::time::Duration::from_secs(5).saturating_sub(began.elapsed()).max(std::time::Duration::from_millis(300)); tokio::time::sleep(wait).await;
        }
    });
}

#[cfg(test)]
mod origin_tests {
    use super::*;
    #[test] fn every_form_of_one_remote_is_one_origin() {
        let want = "github.com/BackplaneWorks/Backplane";
        for u in ["git@github.com:BackplaneWorks/Backplane.git", "https://github.com/BackplaneWorks/Backplane", "https://github.com/BackplaneWorks/Backplane.git",
                  "https://github.com/BackplaneWorks/Backplane/", "https://i2cjak@GitHub.com/BackplaneWorks/Backplane.git/", "ssh://git@github.com:22/BackplaneWorks/Backplane.git",
                  "ssh://git@GITHUB.COM/BackplaneWorks/Backplane", "  git@github.com:/BackplaneWorks/Backplane.git\n", "https://u:pw@github.com:443/BackplaneWorks/Backplane"] {
            assert_eq!(normalize_origin(u), want, "{u}");
        }
        assert_ne!(normalize_origin("git@github.com:BackplaneWorks/Other.git"), want);
        for u in ["", "/home/h/repo.git", "../up", "file:///srv/r.git", "C:/r", "https://github.com/"] { assert_eq!(normalize_origin(u), "", "{u:?}"); }
    }
    #[test] fn the_origin_is_read_from_the_config_and_through_a_worktree() {
        let cfg = "[core]\n\tbare = false\n[remote \"upstream\"]\n\turl = git@x.org:a/b\n[remote \"origin\"]\n\turl = https://github.com/BackplaneWorks/Backplane.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n";
        assert_eq!(origin_in_config(cfg), "https://github.com/BackplaneWorks/Backplane.git"); assert_eq!(origin_in_config("[remote \"upstream\"]\nurl = a:b\n"), "");
        let d = std::env::temp_dir().join(format!("bp-origin-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        let (main, wt) = (d.join("main"), d.join("main/.backplane/worktrees/w"));
        std::fs::create_dir_all(main.join(".git/worktrees/w")).unwrap(); std::fs::create_dir_all(wt.join("sub")).unwrap();
        std::fs::write(main.join(".git/config"), cfg).unwrap(); std::fs::write(main.join(".git/worktrees/w/commondir"), "../..\n").unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", main.join(".git/worktrees/w").display())).unwrap();
        for r in [main.clone(), wt.clone(), wt.join("sub")] { assert_eq!(origin_of_root(&r.to_string_lossy()), "github.com/BackplaneWorks/Backplane", "{r:?}"); }
        assert_eq!(origin_of_root(&d.to_string_lossy()), ""); assert_eq!(origin_of_root(""), "");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn projects_of_one_repository_are_one_and_others_stay_apart() {
        let s = |v: &[(&str, &str)]| v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
        let u = unify(&s(&[("p1", "github.com/o/r"), ("p2", ""), ("p3", "github.com/o/r"), ("p4", "gitlab.com/o/x")]), &s(&[("L~a", "github.com/o/r"), ("L~b", ""), ("L~c", "github.com/o/q"), ("M~d", "gitlab.com/o/x")]));
        assert_eq!(u.len(), 2); assert_eq!(u["L~a"], "p1"); assert_eq!(u["M~d"], "p4");
        let m = Mirrors::default(); m.set_origins("L", &s(&[("a", "github.com/o/r"), ("b", "")])); assert_eq!((m.origin("L~a").as_str(), m.origin("L~b").as_str()), ("github.com/o/r", ""));
        m.set_origins("L", &s(&[("c", "x/y")])); assert_eq!((m.origin("L~a").as_str(), m.origin("L~c").as_str()), ("", "x/y"));
        m.set_names(Arc::new(|| vec![("L".to_string(), "framework-1".to_string())]));
        assert_eq!(m.mark("framework-1"), serde_json::json!({"name":"framework-1","short":"framework-1","away":false,"label":"on framework-1","runs":"runs on framework-1"}));
        m.set_away("L", true); assert_eq!(m.mark("framework-1")["away"], true); m.set_away("L", false); assert_eq!(m.mark("framework-1")["away"], false);
    }
    #[test] fn a_thread_link_opens_by_link_id_or_machine_name_and_this_machines_own_name_is_here() {
        let mut f = crate::state::State::new(); f.lean = true;
        for e in [serde_json::json!({"$":"ProjectCreated","id":"L~p1","title":"P","root":"/r","at":1}), serde_json::json!({"$":"ThreadCreated","id":"L~t9","project":"L~p1","title":"T","at":2})] { f.apply(&e); }
        let far = vec![("framework-1.tail.ts.net".to_string(), &f)];
        let local = |i: &str| i == "t1";
        let r = |id: &str| thread_ref(id, "desk", &local, &far);
        assert_eq!(r("t1").as_deref(), Some("t1")); assert_eq!(r("t2"), None);
        assert_eq!(r("L~t9").as_deref(), Some("L~t9"), "by its link id");
        assert_eq!(r("framework-1~t9").as_deref(), Some("L~t9"), "by the machine's short name"); assert_eq!(r("framework-1.tail.ts.net~t9").as_deref(), Some("L~t9"));
        assert_eq!(r("framework-1~t8"), None, "not mirrored (yet)"); assert_eq!(r("lab~t9"), None, "no such machine");
        assert_eq!(r("desk~t1").as_deref(), Some("t1"), "this machine by its own name"); assert_eq!(r("~t1"), None);
    }
    #[test] fn a_marker_is_short_on_the_row_and_full_in_the_header_and_none_for_this_machine() {
        assert_eq!(mark_of("", false), Value::Null);
        assert_eq!(mark_of("framework-1.tail9e0cf2.ts.net", true), serde_json::json!({"name":"framework-1.tail9e0cf2.ts.net","short":"framework-1","away":true,"label":"on framework-1","runs":"runs on framework-1"}));
        assert_eq!(short_name("a-very-long-machine-name"), "A"); assert_eq!(short_name("desk"), "desk");
    }
}
