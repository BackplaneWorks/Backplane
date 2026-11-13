//! `history.steps {thread, kind, file}`: the design history of the file on show, as 0.14's history bar had it (core/hist.bend's Hist.track and
//! client.bend's Hs.bar). A step is a thread's Act of tone "step" (agents/hist.rs logs one each time the agent's edits change a KiCad file), its
//! record naming the commit, the one before, the git dir, the top and the files changed. The track is the steps that changed the file (a board
//! or a schematic), oldest first; each comes with its words ("Step 3 of 26", "Saved Nov 6 14:08 · After Bash · ...", what it followed in the
//! thread) and the version paths a client watches to show it (old: the step before, new: the step), so a client only draws. The bar's own words
//! (the live title "Live · 21 of 27 changes", the note under it) come along. `git`/`top`/`pcb`/`sch` are the project's, as before.
use crate::{files::git as fgit, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{atomic::{AtomicU64, Ordering}, Arc, Mutex, OnceLock}, time::{Duration, Instant}};
use tokio::sync::watch;

/// A thread entry as the history reads it: a message (role user/assistant) or an act (role = its tone, kind = the tool).
#[derive(Clone, Debug, Default)]
pub struct Src { pub id: String, pub msg: bool, pub role: String, pub kind: String, pub text: String, pub at: u64 }

/// A step's record read back (Step.read).
#[derive(Clone, Debug, PartialEq)]
pub struct Step { pub id: String, pub entry: String, pub c: String, pub p: String, pub g: String, pub top: String, pub files: Vec<(String, String)>, pub at: u64, pub base: bool, /** the step before is in `g` */ pub po: bool }

fn step_of(e: &Src) -> Option<Step> {
    if e.msg || e.role != "step" { return None }
    let v: Value = serde_json::from_str(&e.text).ok()?;
    let g = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let files = v["f"].as_array().into_iter().flatten().filter_map(|l| { let mut it = l.as_str()?.splitn(2, '\t'); let (st, rel) = (it.next()?, it.next()?); (!st.is_empty() && !rel.is_empty()).then(|| (st.to_string(), rel.to_string())) }).collect();
    let p = g("p");
    // Step.base: a thread's first step taken at a turn's start (or an old record that says only that it is first) is the design as it was before, no change
    let base = p.is_empty() && (v["w"] == "start" || v.get("w").is_none());
    if g("c").is_empty() { return None }
    Some(Step { id: e.id.clone(), entry: g("e"), c: g("c"), po: !p.is_empty(), p, g: g("g"), top: g("t"), files, at: e.at, base })
}

/// Hist.ext: 0 a board, 1 a schematic, 2 neither.
fn ext(path: &str) -> u8 { if path.ends_with(".kicad_pcb") { 0 } else if path.ends_with(".kicad_sch") { 1 } else { 2 } }
fn kind_no(kind: &str) -> u8 { if kind == "schematic" { 1 } else { 0 } }

/// Hist.track.file: the steps that changed the file `want` (its live path), or with no file known every step that changed a file of the kind
/// (a deletion shows nothing).
pub fn track<'a>(steps: &'a [Step], kind: u8, want: &str) -> Vec<&'a Step> {
    steps.iter().filter(|s| s.files.iter().any(|(st, rel)| st != "D" && ext(rel) == kind && (want.is_empty() || format!("{}/{}", s.top, rel) == want))).collect()
}

/// core/hist's version path of `file` (a live path under `top`) in `commit`: "git\t<gitdir>\t<commit>\t<len top>\t<top>/<rel>" ("" when there is none).
pub fn version(git: &str, top: &str, commit: &str, file: &str) -> String {
    match file.strip_prefix(&format!("{top}/")) { Some(rel) if !commit.is_empty() && !git.is_empty() && !rel.is_empty() => format!("git\t{git}\t{commit}\t{}\t{top}/{rel}", top.chars().count()), _ => String::new() }
}

/// Hist.line: a text's first line, trimmed and kept short.
fn line(s: &str) -> String { let l = s.trim().lines().next().unwrap_or("").trim(); if l.chars().count() > 160 { format!("{}…", l.chars().take(160).collect::<String>()) } else { l.to_string() } }

/// A shell line without its leading `cd <folder> &&`: agents start nearly every command with it, so three steps in a row read
/// "After Bash · cd /home/…/worktrees/…" and the bar is too short to reach what was run.
fn past_cd(l: &str) -> &str {
    let l = l.trim();
    let Some(r) = l.strip_prefix("cd ") else { return l };
    let r = r.trim_start();
    let end = match r.chars().next() { Some(q @ ('"' | '\'')) => r[1..].find(q).map(|i| i + 2), _ => r.find(|c: char| c.is_whitespace() || c == ';' || c == '&') };
    let Some(end) = end else { return l };
    let (gap, rest) = (&r[end..r.len() - r[end..].trim_start().len()], r[end..].trim_start());
    let next = rest.strip_prefix("&&").or_else(|| rest.strip_prefix(';')).or(if gap.contains('\n') { Some(rest) } else { None });
    match next { Some(x) if !x.trim().is_empty() => x.trim_start(), _ => l }
}
/// Hs.source: what the step followed in the thread.
fn source(es: &HashMap<&str, &Src>, entry: &str) -> String {
    if entry.is_empty() { return "Turn ended".into() }
    match es.get(entry) {
        Some(e) if e.msg && e.role == "user" => join("Turn started", &line(&e.text)),
        Some(e) if e.msg => join("After reply", &line(&e.text)),
        Some(e) => { let k = crate::present::thread::shown_name(&e.kind); let t = if e.text == e.kind { String::new() } else { line(past_cd(&e.text)) }; join(&format!("After {k}"), &t) }
        None => "Source unavailable".into(),
    }
}
fn join(a: &str, b: &str) -> String { if b.is_empty() { a.to_string() } else { format!("{a} · {b}") } }

/// SFile.names: "heater.kicad_pcb (new), power.kicad_sch (removed)"
fn names(fs: &[(String, String)]) -> String {
    fs.iter().map(|(st, rel)| format!("{}{}", rel.rsplit('/').next().unwrap_or(rel), match st.as_str() { "D" => " (removed)", "A" => " (new)", _ => "" })).collect::<Vec<_>>().join(", ")
}

/// Hs.live.words: the live bar's title, its track's changes and the thread's all when those are more.
pub fn live_words(c: usize, all: usize) -> String {
    if c == 0 { "Live".into() } else if c < all { format!("Live · {c} of {all} changes") } else { format!("Live · {c} {}", if c == 1 { "change" } else { "changes" }) }
}

/// What the live title's numbers are (its tooltip): the ticks are every snapshot of the file, the changes leave out the design as it was before a turn.
pub fn live_tip(n: usize, c: usize, total: usize) -> String {
    if n == 0 { return String::new() }
    let snaps = format!("{n} {} of this file", if n == 1 { "snapshot" } else { "snapshots" });
    let changes = if c < total { format!("{c} of the thread's {total} design changes") } else { format!("{c} design {}", if c == 1 { "change" } else { "changes" }) };
    let before = n - c;
    if before == 0 { format!("{snaps}: {changes}") } else { format!("{snaps}: {changes}, and the design as it was before {before} {}", if before == 1 { "turn" } else { "turns" }) }
}
/// The bar for `file` (kind 0 a board, 1 a schematic) from the thread's entries, oldest first. `when` names a time as the person reads it.
pub fn bar(es: &[Src], kind: u8, file: &str, when: &dyn Fn(u64) -> String) -> Value { bar_of(es.iter().filter_map(step_of).collect(), es, kind, file, when) }
/// The same from steps already read (and placed: `locate`).
pub fn bar_of(all: Vec<Step>, es: &[Src], kind: u8, file: &str, when: &dyn Fn(u64) -> String) -> Value {
    let by: HashMap<&str, &Src> = es.iter().map(|e| (e.id.as_str(), e)).collect();
    // the thread's row a step points at: a message is its own row; a tool call or a step sits in a run the thread folds, "fold:<its first>"
    let mut fold: HashMap<&str, String> = HashMap::new(); let mut first: Option<&str> = None;
    for e in es {
        let base = step_of(e).is_some_and(|s| s.base);
        if !e.msg && (e.role == "tool" || e.role == "step") && !base { let f = *first.get_or_insert(e.id.as_str()); fold.insert(e.id.as_str(), format!("fold:{f}")); }
        else if !base { first = None }
    }
    let row = |s: &Step| -> String { match by.get(s.entry.as_str()) { Some(e) if e.msg => e.id.clone(), _ => fold.get(s.entry.as_str()).or_else(|| fold.get(s.id.as_str())).cloned().unwrap_or_default() } };
    let tr = track(&all, kind, file);
    let n = tr.len();
    let rows: Vec<Value> = tr.iter().enumerate().map(|(i, s)| {
        // the file as this step left it: the one on show when the step changed it, else the first of the kind it changed (Step.file)
        let f = if !file.is_empty() { file.to_string() } else { s.files.iter().find(|(st, rel)| st != "D" && ext(rel) == kind).map(|(_, rel)| format!("{}/{rel}", s.top)).unwrap_or_default() };
        let at = if s.at == 0 { "Time unavailable".to_string() } else { when(s.at) };
        let title = if s.p.is_empty() { format!("Step {} of {n} · before the agent's edits", i + 1) } else { format!("Step {} of {n}", i + 1) };
        json!({"c": s.c, "p": s.p, "t": s.at, "id": s.id, "entry": s.entry, "base": s.base,
            "old": if s.po { version(&s.g, &s.top, &s.p, &f) } else { String::new() }, "new": version(&s.g, &s.top, &s.c, &f),
            "label": format!("Step {}", i + 1), "tip": format!("Step {} · {at}", i + 1), "title": title,
            "detail": format!("Saved {at} · {}", source(&by, &s.entry)), "files": names(&s.files), "row": row(s)})
    }).collect();
    let (c, total) = (tr.iter().filter(|s| !s.base).count(), all.iter().filter(|s| !s.base).count());
    json!({"steps": rows, "live": live_words(c, total), "liveTip": live_tip(n, c, total),
        "note": if n == 0 { "The live file. Compare it with any commit, branch or tag." } else { "The live file. Each tick is a saved snapshot; pick one or play the history." }})
}

/// Where each step's commit is. A step names the history.git it was taken in; when a hub did not know that folder (its trust registry
/// had no line for it, or named it on another device number) the capture moved it aside as `history.git.untrusted-<time>` and began a
/// new one, so older steps are found there. `have(dir, commits)` answers which of the commits a git dir has. A step found nowhere is
/// dropped (there is nothing to show); a step whose step before is not beside it shows as the file it left, with no edits marked.
pub fn place(steps: Vec<Step>, have: &mut dyn FnMut(&str, &[String]) -> std::collections::HashSet<String>, siblings: &dyn Fn(&str) -> Vec<String>) -> Vec<Step> {
    use std::collections::{BTreeMap, HashSet};
    let mut by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in &steps { let v = by.entry(s.g.clone()).or_default(); v.push(s.c.clone()); if !s.p.is_empty() { v.push(s.p.clone()) } }
    let mut at: HashMap<(String, String), String> = HashMap::new(); // (recorded dir, commit) -> the dir that has it
    for (g, mut want) in by { want.sort(); want.dedup();
        let mut dirs = vec![g.clone()]; dirs.extend(siblings(&g));
        for d in dirs { if want.is_empty() { break }
            let got: HashSet<String> = have(&d, &want);
            want.retain(|c| { let hit = got.contains(c); if hit { at.insert((g.clone(), c.clone()), d.clone()); } !hit });
        }
    }
    steps.into_iter().filter_map(|mut s| {
        let d = at.get(&(s.g.clone(), s.c.clone()))?.clone();
        s.po = !s.p.is_empty() && at.get(&(s.g.clone(), s.p.clone())) == Some(&d);
        s.g = d; Some(s)
    }).collect()
}
/// The folders a capture moved `g` aside to (`<g>.untrusted-<time>`), newest first.
fn moved_aside(g: &str) -> Vec<String> {
    let p = std::path::Path::new(g); let (Some(dir), Some(name)) = (p.parent(), p.file_name().and_then(|n| n.to_str())) else { return vec![] };
    let pre = format!("{name}.untrusted-");
    let mut v: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| { let n = e.file_name().to_string_lossy().into_owned();
        (n.strip_prefix(&pre).is_some_and(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())) && e.path().is_dir()).then(|| e.path().to_string_lossy().into_owned()) }).collect();
    v.sort(); v.reverse(); v
}
/// One git read of a history (#1110): past this it is killed and its folder counts as having none of the commits.
const GIT_LIMIT: Duration = Duration::from_secs(10);
/// All of one `history.steps`' git: past this it answers with the steps found so far (a folder not yet read shows none).
const STEPS_LIMIT: Duration = Duration::from_secs(30);
/// Which of `commits` the git dir `g` has (a read: rev-list of the commits themselves, missing ones ignored), None when it was not read by
/// `until` (or within GIT_LIMIT). Killed with its future: a request given up or replaced leaves no git running.
async fn have(g: &str, commits: &[String], until: Instant) -> Option<std::collections::HashSet<String>> {
    let left = until.saturating_duration_since(Instant::now()).min(GIT_LIMIT);
    if left.is_zero() { return None }
    let mut args: Vec<&str> = vec!["--git-dir", g, "rev-list", "--no-walk", "--ignore-missing"]; args.extend(commits.iter().map(String::as_str));
    let r = fgit::read_within(left, "git", &args, "/", &[]).await;
    if r.code == 124 { return None }
    if r.code != 0 { return Some(Default::default()) }
    Some(r.out.lines().map(|l| l.trim().to_string()).filter(|l| l.len() >= 7).collect())
}

/// The run now going for each request key, and where its answer comes.
struct Run { n: u64, stop: tokio::task::AbortHandle, answer: watch::Receiver<Option<Value>> }
fn runs() -> &'static Mutex<HashMap<String, Run>> { static R: OnceLock<Mutex<HashMap<String, Run>>> = OnceLock::new(); R.get_or_init(Default::default) }
/// `work` for `key`, replacing a run still going for the same key (#1110: a viewer asking again and again piled up git processes): the older
/// run is stopped (its future dropped, so its git is killed) and its caller gets the newer run's answer. None only when no run answered.
pub async fn latest<F>(key: String, work: F) -> Option<Value> where F: std::future::Future<Output = Value> + Send + 'static {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let (tx, mut rx) = watch::channel(None);
    let task = tokio::spawn(async move { let v = work.await; let _ = tx.send(Some(v)); });
    if let Some(old) = runs().lock().unwrap().insert(key.clone(), Run { n, stop: task.abort_handle(), answer: rx.clone() }) { old.stop.abort() }
    let mut mine = n;
    let out = loop {
        if let Ok(v) = rx.wait_for(Option::is_some).await { break v.clone() }
        // stopped by a newer request for the same key: wait for its answer instead (never past what it is bounded by)
        let next = { let g = runs().lock().unwrap(); g.get(&key).filter(|r| r.n != mine).map(|r| (r.n, r.answer.clone())) };
        match next { Some((n2, r2)) => { mine = n2; rx = r2 } None => break None }
    };
    { let mut g = runs().lock().unwrap(); if g.get(&key).is_some_and(|r| r.n == n) { g.remove(&key); } }
    out
}

/// A thread's entries read off the log (the model's lock held only to copy their places).
async fn entries(hub: &Arc<Hub>, thread: &str) -> Vec<Src> {
    let ents = hub.model().await.own_ents(thread);
    let mut out = Vec::with_capacity(ents.len());
    for e in ents {
        let Ok(l) = hub.store.get(e.seq as usize - 1) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&l) else { continue };
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        let at = v["at"].as_u64().unwrap_or(0);
        out.push(match v["$"].as_str().unwrap_or("") {
            "MessagePosted" => Src { id: s("msg"), msg: true, role: s("role"), kind: String::new(), text: s("text"), at },
            "ActivityLogged" => Src { id: s("act"), msg: false, role: s("tone"), kind: s("kind"), text: s("summary"), at },
            _ => continue,
        });
    }
    out
}

/// `history.steps`: one run per thread and file at a time (a repeat replaces the one still going), its git bounded in time.
pub async fn steps(hub: &Arc<Hub>, thread: &str, kind: &str, file: &str) -> Value {
    let (h, t, k, f) = (hub.clone(), thread.to_string(), kind.to_string(), file.to_string());
    latest(format!("{thread}\n{kind}\n{file}"), async move { steps_now(&h, &t, &k, &f, Instant::now() + STEPS_LIMIT).await }).await
        .unwrap_or_else(|| json!({"thread": thread, "steps": [], "live": "Live", "note": ""}))
}
async fn steps_now(hub: &Arc<Hub>, thread: &str, kind: &str, file: &str, until: Instant) -> Value {
    let Some(cwd) = crate::files::root_of(hub, thread).await else { return json!({"thread": thread, "steps": [], "live": "Live", "note": ""}) };
    let cwd = cwd.to_string_lossy().into_owned();
    let out = tokio::time::timeout(GIT_LIMIT, fgit::refs(&cwd, thread)).await.unwrap_or_default(); let mut ls = out.split('\n');
    let (gd, top) = (ls.next().unwrap_or("").to_string(), ls.next().unwrap_or("").to_string());
    let f = super::project::read(&cwd);
    let k = kind_no(kind);
    let file = if !file.is_empty() && !file.contains('\t') && !file.contains('\n') { file.to_string() } else if k == 1 { f.sch.clone() } else { f.pcb.clone() };
    let es = entries(hub, thread).await;
    let zone = hub.setting("tz.machine").await.unwrap_or_default(); let now = crate::agents::now_s();
    // where each step's commit is now (git, off the async workers' hot path: one rev-list a folder)
    let steps: Vec<Step> = es.iter().filter_map(step_of).collect();
    let mut dirs: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for s in &steps { for d in std::iter::once(s.g.clone()).chain(moved_aside(&s.g)) { let v = dirs.entry(d).or_default(); v.push(s.c.clone()); if !s.p.is_empty() { v.push(s.p.clone()) } } }
    let mut known: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for (d, mut cs) in dirs { cs.sort(); cs.dedup(); if let Some(h) = have(&d, &cs, until).await { known.insert(d, h); } }
    let steps = place(steps, &mut |d, want| known.get(d).map(|h| want.iter().filter(|c| h.contains(*c)).cloned().collect()).unwrap_or_default(), &moved_aside);
    let mut v = bar_of(steps, &es, k, &file, &|t| crate::agents::rules::when(&zone, t, now));
    for (key, val) in [("thread", thread.to_string()), ("git", gd), ("top", top), ("pcb", f.pcb), ("sch", f.sch), ("kind", if k == 1 { "schematic".into() } else { "board".into() }), ("file", file)] { v[key] = json!(val); }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    fn msg(id: &str, role: &str, text: &str, at: u64) -> Src { Src { id: id.into(), msg: true, role: role.into(), kind: String::new(), text: text.into(), at } }
    fn act(id: &str, tone: &str, kind: &str, text: &str, at: u64) -> Src { Src { id: id.into(), msg: false, role: tone.into(), kind: kind.into(), text: text.into(), at } }
    fn step(c: &str, p: &str, e: &str, w: &str, files: &[&str], at: u64) -> Src {
        act(&format!("step:{c}"), "step", "kicad", &json!({"e": e, "c": c, "p": p, "g": "/p/.git", "t": "/p", "f": files, "w": w}).to_string(), at)
    }
    fn log() -> Vec<Src> { vec![
        msg("m1", "user", "add an LDO\nand route it", 100),
        step("c0", "", "m1", "start", &["A\tb.kicad_pcb", "A\ts.kicad_sch"], 101),
        act("tu1", "tool", "Edit", "b.kicad_pcb: widen +5V", 110),
        step("c1", "c0", "tu1", "", &["M\tb.kicad_pcb"], 111),
        act("tu2", "tool", "mcp__backplane__kicad_sch_edit", "mcp__backplane__kicad_sch_edit", 120),
        step("c2", "c1", "tu2", "", &["M\ts.kicad_sch"], 121),
        msg("a1", "assistant", "Done: the LDO is in.", 130),
        step("c3", "c2", "", "", &["M\tb.kicad_pcb", "D\tother.kicad_pcb"], 131),
    ] }
    /// e2e n1110: Compare listed "Step 4 … After Bash · cd /home/kostas/.cache/bp-tmp/n1110-2…" three times: the steps could not be told apart.
    #[test] fn a_shell_step_is_named_by_what_ran_not_by_the_folder_it_ran_in() {
        assert_eq!(past_cd("cd /w/lamp/.backplane/worktrees/backplane-a1f790d1 && python3 tie_holes.py"), "python3 tie_holes.py");
        assert_eq!(past_cd("cd '/w/my lamp' && kicad-cli pcb drc b.kicad_pcb"), "kicad-cli pcb drc b.kicad_pcb");
        assert_eq!(past_cd("cd \"/w/my lamp\"; ls"), "ls");
        assert_eq!(past_cd("cd /w/lamp/.backplane/worktrees/backplane-a1f790d1; python3 -I - <<'E'\nimport re"), "python3 -I - <<'E'\nimport re", "the form the agents of n1110 used");
        assert_eq!(past_cd("cd /w/lamp&&ls"), "ls"); assert_eq!(past_cd("cd /w/lamp\nkicad-cli pcb drc b.kicad_pcb"), "kicad-cli pcb drc b.kicad_pcb");
        for kept in ["cd /w/lamp", "cd /w/lamp &&", "cd /w/lamp;", "cd /w/lamp ls", "cd /w/lamp & ls", "cdparanoia -B", "ls && cd /w", "cd 'unclosed && ls", ""] { assert_eq!(past_cd(kept), kept, "{kept:?} stays as it is") }
        let mut l = log(); l[2] = act("tu1", "tool", "Bash", "cd /p && python3 widen.py\nsecond line", 110);
        let v = bar(&l, 0, "/p/b.kicad_pcb", &|t| format!("T{t}"));
        assert_eq!(v["steps"][1]["detail"], "Saved T111 · After Bash · python3 widen.py");
    }
    #[test] fn the_bar_names_each_step_of_the_file_by_what_it_followed() {
        let v = bar(&log(), 0, "/p/b.kicad_pcb", &|t| format!("T{t}"));
        let s = v["steps"].as_array().unwrap();
        assert_eq!(s.iter().map(|x| x["c"].as_str().unwrap()).collect::<Vec<_>>(), ["c0", "c1", "c3"], "only the board's steps");
        assert_eq!(s[0]["title"], "Step 1 of 3 · before the agent's edits"); assert_eq!(s[1]["title"], "Step 2 of 3"); assert_eq!(s[1]["label"], "Step 2"); assert_eq!(s[1]["tip"], "Step 2 · T111");
        assert_eq!(s[0]["detail"], "Saved T101 · Turn started · add an LDO");
        assert_eq!(s[1]["detail"], "Saved T111 · After Edit · b.kicad_pcb: widen +5V");
        assert_eq!(s[2]["detail"], "Saved T131 · Turn ended"); assert_eq!(s[2]["files"], "b.kicad_pcb, other.kicad_pcb (removed)");
        assert_eq!(s[1]["old"], "git\t/p/.git\tc0\t2\t/p/b.kicad_pcb"); assert_eq!(s[1]["new"], "git\t/p/.git\tc1\t2\t/p/b.kicad_pcb");
        assert_eq!(s[0]["old"], "", "the first step has no version before it");
        assert_eq!(s[1]["entry"], "tu1"); assert_eq!(s[0]["base"], true);
        assert_eq!((s[0]["row"].as_str(), s[1]["row"].as_str(), s[2]["row"].as_str()), (Some("m1"), Some("fold:tu1"), Some("fold:step:c3")), "a message is its row; a tool call or a turn's end is in its folded run");
        assert_eq!(v["live"], "Live · 2 of 3 changes", "the design as it was before is no change; the schematic's change counts for the thread");
        assert_eq!(v["liveTip"], "3 snapshots of this file: 2 of the thread's 3 design changes, and the design as it was before 1 turn");
        assert_eq!(live_tip(26, 21, 27), "26 snapshots of this file: 21 of the thread's 27 design changes, and the design as it was before 5 turns"); assert_eq!(live_tip(2, 2, 2), "2 snapshots of this file: 2 design changes"); assert_eq!(live_tip(0, 0, 4), "");
        let sch = bar(&log(), 1, "", &|t| format!("T{t}"));
        let s = sch["steps"].as_array().unwrap();
        assert_eq!(s.len(), 2); assert_eq!(s[1]["detail"], "Saved T121 · After kicad_sch_edit", "a tool call whose summary is its name says only the name");
        assert_eq!(s[1]["new"], "git\t/p/.git\tc2\t2\t/p/s.kicad_sch", "with no file known, the file of the kind the step changed");
        let none = bar(&log(), 0, "/p/elsewhere.kicad_pcb", &|t| format!("{t}"));
        assert!(none["steps"].as_array().unwrap().is_empty()); assert_eq!(none["live"], "Live"); assert_eq!(none["note"], "The live file. Compare it with any commit, branch or tag.");
    }
    #[test] fn a_step_is_shown_from_where_its_commit_is_now() {
        let mk = |c: &str, p: &str| Step { id: format!("step:{c}"), entry: String::new(), c: c.into(), p: p.into(), g: "/p/.backplane/history.git".into(), top: "/p".into(), files: vec![("M".into(), "b.kicad_pcb".into())], at: 1, base: false, po: !p.is_empty() };
        let steps = vec![mk("a1", ""), mk("a2", "a1"), mk("b1", ""), mk("b2", "b1"), mk("gone", "b2")];
        let held: HashMap<&str, Vec<&str>> = [("/p/.backplane/history.git", vec!["b2"]), ("/p/.backplane/history.git.untrusted-9", vec!["a1", "a2", "b1"])].into_iter().collect();
        let mut asked = vec![];
        let out = place(steps, &mut |d, want| { asked.push(d.to_string()); want.iter().filter(|c| held.get(d).is_some_and(|h| h.contains(&c.as_str()))).cloned().collect() }, &|g| vec![format!("{g}.untrusted-9")]);
        assert_eq!(out.iter().map(|s| (s.c.as_str(), s.g.rsplit('/').next().unwrap(), s.po)).collect::<Vec<_>>(),
            [("a1", "history.git.untrusted-9", false), ("a2", "history.git.untrusted-9", true), ("b1", "history.git.untrusted-9", false), ("b2", "history.git", false)],
            "a moved-aside history still shows; a step whose step before was moved away shows without marks; a commit found nowhere is no step");
        let v = bar_of(out, &[], 0, "/p/b.kicad_pcb", &|t| format!("T{t}"));
        assert_eq!(v["steps"][1]["old"], "git\t/p/.backplane/history.git.untrusted-9\ta1\t2\t/p/b.kicad_pcb"); assert_eq!(v["steps"][3]["old"], ""); assert_eq!(v["steps"][3]["title"], "Step 4 of 4");
    }
    #[test] fn an_after_reply_step_and_an_unknown_source_say_so() {
        let es = vec![msg("a1", "assistant", "  Moved U3.\nmore", 5), step("c1", "c0", "a1", "", &["M\tb.kicad_pcb"], 6), step("c2", "c1", "gone", "", &["M\tb.kicad_pcb"], 0)];
        let v = bar(&es, 0, "", &|t| format!("T{t}"));
        assert_eq!(v["steps"][0]["detail"], "Saved T6 · After reply · Moved U3."); assert_eq!(v["steps"][1]["detail"], "Saved Time unavailable · Source unavailable");
        assert_eq!(v["live"], "Live · 2 changes"); assert_eq!(live_words(1, 1), "Live · 1 change");
    }
    /// #1110: a repeated history.steps for the same thread and file replaces the one still going: the older is stopped (its future, and so its git,
    /// dropped) and its caller gets the newer answer; another key is left alone.
    #[tokio::test(flavor = "multi_thread")] async fn a_repeated_request_replaces_the_one_still_going() {
        struct Dropped(Arc<std::sync::atomic::AtomicBool>); impl Drop for Dropped { fn drop(&mut self) { self.0.store(true, Ordering::SeqCst) } }
        let gone = Arc::new(std::sync::atomic::AtomicBool::new(false)); let g2 = gone.clone();
        let key = format!("t-{}\nboard\n", std::process::id());
        let first = tokio::spawn(latest(key.clone(), async move { let _d = Dropped(g2); std::future::pending::<()>().await; json!("never") }));
        let other = tokio::spawn(latest(format!("{key}other"), async { tokio::time::sleep(Duration::from_millis(300)).await; json!("other") }));
        tokio::time::sleep(Duration::from_millis(100)).await; assert!(!first.is_finished());
        let second = latest(key.clone(), async { tokio::time::sleep(Duration::from_millis(50)).await; json!("fresh") }).await;
        assert_eq!(second, Some(json!("fresh")));
        assert!(gone.load(Ordering::SeqCst), "the older run was stopped");
        assert_eq!(tokio::time::timeout(Duration::from_secs(5), first).await.unwrap().unwrap(), Some(json!("fresh")), "its caller has the newer answer");
        assert_eq!(other.await.unwrap(), Some(json!("other")), "another file's request is not touched");
        assert!(!runs().lock().unwrap().contains_key(&key), "nothing is left behind");
    }
    /// #1110: a history read past its deadline is no read (its folder shows none of the steps), not a wait.
    #[tokio::test] async fn a_history_read_past_its_deadline_is_skipped() {
        let t0 = Instant::now();
        assert_eq!(have("/nonexistent/history.git", &["abc1234".into()], Instant::now()).await, None);
        assert!(t0.elapsed() < Duration::from_secs(1));
    }
}
