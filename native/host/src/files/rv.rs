//! Design review's jobs (server/review.bend's RvJob, Rvc, Rvr; docs/design-review.md), run in the hub: no `backplane --review` child and
//! no Bun. The rules are the pure twins in kicad/{bom,checks,cad,release,review}.rs; the shell text is the Bend hub's (scripts/*.sh,
//! extracted by test/gpui/files/extract_scripts.py). Every route resolves the thread's folder first (threadpath.rs) and answers the refusal
//! with none, before any work. Deliberate differences from the JS hub, all refusals:
//!   * a version a request names (`git\t<dir>\t<commit>...`) must name the thread's own repository and a commit id: the JS hub runs
//!     `git show` in any folder, and a commit word starting with "-" is an option to it
//!   * `.backplane/checklist.md` is not written through a link
//!   * review.comment needs a thread with a folder (the JS hub logs a comment on any id)
//!   * a hub on a copied log ticks nothing, releases nothing and runs no project's own checks (core/copy.bend's rule: it acts outside its home nowhere)
//!   * Not ported: the bundled library overlay (effects/kicad_library.js), which only a build with a baked library payload has.
use super::git::{self, proc, proc_ro, Ran};
use super::kicad::bj::{self, J};
use super::kicad::{bom, cad, checks, ipc, plotread, project, release, review};
use super::{fm, threadpath};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const PREP: &str = include_str!("scripts/review_prep.sh");
const BOUND: &str = include_str!("scripts/review_bound.sh");
const REL_PREP: &str = include_str!("scripts/release_prep.sh");
const REL_OUTPUT: &str = include_str!("scripts/release_output.sh");
const REL_FILES: &str = include_str!("scripts/release_files.sh");
const REL_COMMIT: &str = include_str!("scripts/release_commit.sh");
const REL_SOURCE: &str = include_str!("scripts/release_source.sh");
const REL_TAG: &str = include_str!("scripts/release_tag.sh");
const PR: &str = include_str!("scripts/review_pr.sh");
const MISSING: &str = "KiCad is not installed on this machine (no kicad-cli): get it from kicad.org/download or its package manager";
const WAIT_OK: &str = "this project's own checks wait for your OK";

fn copy() -> bool { crate::copy::on() }
fn lines(s: &str) -> Vec<String> { bj::trim(s).split('\n').map(str::to_string).collect() }
fn at(xs: &[String], i: usize) -> &str { xs.get(i).map(String::as_str).unwrap_or("") }
fn or<'a>(text: &'a str, d: &'a str) -> &'a str { if bj::trim(text).is_empty() { d } else { text } }
fn cut(s: &str, n: usize) -> String { bj::trim(&bj::take(s, n)).to_string() }
fn text_of(path: &str) -> String { std::fs::read(path).map(|b| super::kicad::sexpr::decode(&b)).unwrap_or_default() }
fn err(why: impl Into<String>) -> J { J::obj(&[("err", J::Str(why.into()))]) }
async fn sh(script: &str, args: &[&str], cwd: &str) -> Ran { let mut a = vec!["-c", script, "sh"]; a.extend(args); proc("sh", &a, cwd, &[]).await }
/// A listed read (or a write only into a scratch folder of its own): the only kind of script a copy runs here. `sh` is a write or unknown, refused on a copy.
async fn sh_ro(script: &str, args: &[&str], cwd: &str) -> Ran { let mut a = vec!["-c", script, "sh"]; a.extend(args); proc_ro("sh", &a, cwd, &[]).await }
/// KiCad.run
/// kicad-cli for a CHECK: its report goes to the check's own scratch folder. A write (release exports) uses `kicad`, refused on a copy.
async fn kicad_check(argv: &[String]) -> Ran { kicad_with(argv, true).await }
async fn kicad(argv: &[String]) -> Ran { kicad_with(argv, false).await }
async fn kicad_with(argv: &[String], ro: bool) -> Ran {
    let Some((c, rest)) = argv.split_first() else { return Ran { code: 127, out: MISSING.into() } };
    if c.is_empty() { return Ran { code: 127, out: MISSING.into() } }
    let a = rest.iter().map(String::as_str).collect::<Vec<_>>();
    if ro { proc_ro(c, &a, "/", &[]).await } else { proc(c, &a, "/", &[]).await }
}

// Versions: core/hist.bend
fn field(p: &str, i: usize) -> &str { p.split('\t').nth(i).unwrap_or("") }
/// Hist.git
pub fn hist_git(g: &str, c: &str, top: &str, rel: &str) -> String { format!("git\t{g}\t{c}\t{}\t{top}/{rel}", bj::len(top)) }
/// Hist.rebase: another live file in the same commit as version p; abs itself when p is live
pub fn rebase(p: &str, abs: &str) -> String { if plotread::kind_of(p) == 1 { format!("git\t{}\t{}\t{}\t{abs}", field(p, 1), field(p, 2), field(p, 3)) } else { abs.to_string() } }
fn commit_word(c: &str) -> bool { (4..=64).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_hexdigit()) }
/// A version a request names is read only from the thread's own repository (or its .backplane/history.git), at a commit id.
async fn version_ok(hub: &Arc<Hub>, cwd: &str, p: &str) -> bool {
    if plotread::kind_of(p) != 1 { return true }
    if !commit_word(field(p, 2)) { return false }
    let gd = field(p, 1);
    if gd == format!("{cwd}/.backplane/history.git") || super::kicad::known_root(hub, cwd, p).await { return true }
    let own = sh_ro("exec 2>/dev/null\ncd \"$1\" || exit 1\ncd \"$(git rev-parse --git-common-dir)\" && pwd", &[cwd], "/").await;
    own.code == 0 && own.out.trim() == gd
}

// BOMs
/// Rvc.parts: a version's schematic, every sheet it reaches with its bytes (the root first read, at most 64 files)
fn parts_of(root: &str) -> Vec<bom::Part> {
    if root.is_empty() { return vec![] }
    let (mut todo, mut known, mut shs): (Vec<String>, Vec<String>, Vec<bom::Sheet>) = (vec![root.to_string()], vec![], vec![]);
    for _ in 0..64 {
        if todo.is_empty() { break }
        let p = todo.remove(0);
        let bs = plotread::version_env(&p, &git::GIT_ENV);
        let kd = plotread::kids(&p, &bs);
        known.push(p.clone());
        for (_, f) in kd.sheets { if !known.contains(&f) && !todo.contains(&f) { todo.push(f) } }
        shs.push(bom::Sheet { path: p, text: bs });
    }
    bom::parts("", &shs)
}
async fn parts(root: &str) -> Vec<bom::Part> { let r = root.to_string(); tokio::task::spawn_blocking(move || parts_of(&r)).await.unwrap_or_default() }
/// Rvc.bom.k: OLD "" is one BOM of NEW, else what changed from OLD to NEW
async fn bom_text(a: &str, b: &str) -> String {
    if a.is_empty() { bom::one(&parts(b).await) } else { let os = parts(a).await; bom::diff_md(&os, &parts(b).await) }
}
fn report(thread: &str, page: &str, md: &str, why: Option<&str>) -> Value {
    match why { None => json!({"report":{"thread":thread,"page":page,"md":md}}), Some(e) => json!({"report":{"thread":thread,"page":page,"md":md,"err":e}}) }
}
/// RvJob.bom.root: a working-copy request may name another board's root schematic, inside this thread's folder
fn bom_root(cwd: &str, b: &str, fallback: &str) -> String {
    match b.strip_prefix(&format!("{cwd}/")) { Some(rel) if b.ends_with(".kicad_sch") && fm::safe(rel) => b.to_string(), _ => fallback.to_string() }
}
async fn bom_job(hub: &Arc<Hub>, thread: &str, cwd: &str, a: &str, b: &str) -> Value {
    let sch = bom_root(cwd, b, &project::read(cwd).sch);
    if sch.is_empty() { return report(thread, "bom", "", Some("This project has no schematic.")) }
    let (av, bv) = (if a.is_empty() { String::new() } else { rebase(a, &sch) }, if b.is_empty() { sch.clone() } else { rebase(b, &sch) });
    if !version_ok(hub, cwd, &av).await || !version_ok(hub, cwd, &bv).await { return report(thread, "bom", "", Some("that version is not in this project")) }
    report(thread, "bom", bj::trim_end(&bom_text(&av, &bv).await), None)
}

// Design checks
/// Rvc.bound.load: every sheet of a snapshot is a regular file inside it
async fn bound_load(root: &str, sch: &str) -> String {
    let (mut todo, mut known): (Vec<String>, Vec<String>) = (vec![sch.to_string()], vec![]);
    for _ in 0..64 {
        if todo.is_empty() { return String::new() }
        let p = todo.remove(0);
        let r = sh_ro(BOUND, &[root, &p], "/").await;
        if r.code != 0 { return bj::trim(&r.out).to_string() }
        let q = p.clone();
        let kd = tokio::task::spawn_blocking(move || plotread::kids(&q, &plotread::version_env(&q, &git::GIT_ENV))).await;
        known.push(p);
        if let Ok(kd) = kd { for (_, f) in kd.sheets { if !known.contains(&f) && !todo.contains(&f) { todo.push(f) } } }
    }
    if todo.is_empty() { String::new() } else { "release schematic hierarchy exceeds 64 files".into() }
}
/// Rvc.bound: the configured files of a snapshot do not leave it
async fn bound(root: &str, f: &project::Files) -> String {
    if root.is_empty() { return String::new() }
    let r = sh_ro(BOUND, &[root, &f.pro, &f.pcb, &f.sch], "/").await;
    if r.code != 0 { return bj::trim(&r.out).to_string() }
    if f.sch.is_empty() { String::new() } else { bound_load(root, &f.sch).await }
}
/// Rvc.chk: one KiCad check: its findings, or an explicit execution failure
async fn one_check(c: &cad::Chk, cli: &str, out: &str, pcb: &str, sch: &str) -> Vec<checks::Finding> {
    if (if c.id == "erc" { sch } else { pcb }).is_empty() { return vec![] }
    let r = kicad_check(&cad::fill(c.argv, &[("cli", cli), ("pcb", pcb), ("sch", sch), ("out", out)])).await;
    let text = text_of(&format!("{out}/{}", c.report));
    if r.code != 0 { return vec![checks::finding("execution", "error", c.id, &format!("check command failed (exit {}): {}", r.code, cut(&r.out, 300)), "")] }
    if !checks::report_valid(c.id, &text) { return vec![checks::finding("execution", "error", c.id, "missing or malformed check report", "")] }
    if c.id == "erc" { checks::erc(&text) } else { checks::drc(&text) }
}
fn tasks_json(ts: &[checks::Task]) -> J { J::Arr(ts.iter().map(|t| J::obj(&[("text", J::str(&t.text)), ("done", J::Flag(t.done)), ("group", J::str(&t.group))])).collect()) }
/// Rvc.checks.inputs
async fn checks_inputs(cli: &str, out: &str, dir: &str, home: &str, trigger: &str, f: &project::Files) -> J {
    let mut fs = vec![];
    for c in &cad::CHECKS { fs.extend(one_check(c, cli, out, &f.pcb, &f.sch).await) }
    let ps = parts(&f.sch).await;
    fs.extend(bom::rules(&ps).iter().map(|b| checks::finding("bom", b.sev, b.id, &format!("{}: {}", b.reference, b.msg), "")));
    let (cl, cj) = (text_of(&format!("{dir}/.backplane/checklist.md")), text_of(&format!("{dir}/.backplane/checks.json")));
    let cfg = checks::config(&cj);
    let cs = checks::for_trigger(&cfg.checks, trigger);
    let hash = hex::encode(Sha256::digest(cj.as_bytes()));
    // the OK is for the whole file (its hash), so the question shows every command in it, whatever the trigger: none is allowed unseen
    let trusted = text_of(&format!("{home}/checks.trusted")).split('\n').any(|l| bj::trim(l) == hash);
    let ok = cs.is_empty() || (trusted && !copy());
    let mut runs = vec![];
    if ok { for c in &cs { let r = proc("sh", &["-c", &c.run], dir, &[]).await; runs.push(checks::Run { name: c.name.clone(), ok: r.code == 0, out: cut(&r.out, 4000) }) } }
    let tasks = checks::parse(or(&cl, checks::DEFAULT));
    let cmds: String = cfg.checks.iter().map(|c| format!("{}: {}\n", c.name, c.run)).collect();
    let pending_md = if ok { String::new() } else { format!("**This project's own checks wait for your OK.** `.backplane/checks.json` runs these commands; Backplane asks you in the thread before it runs them:\n\n```\n{cmds}```\n\n") };
    J::obj(&[("err", J::Str(if ok { checks::execution(&fs) } else { WAIT_OK.into() })), ("md", J::Str(pending_md + &checks::md(&fs, &[], &runs))),
        ("gate", J::Str(if ok { checks::gate_all(&fs, &tasks, &runs, &cfg.gate) } else { WAIT_OK.into() })), ("line", J::Str(checks::line(&fs))), ("tasks", tasks_json(&tasks)),
        ("pending", J::Str(if ok { String::new() } else { hash })), ("pendrun", J::Str(if ok { String::new() } else { cmds }))])
}
/// Rvc.check.run: the project in DIR (COMMIT "" its files as they are; else that commit, taken out with git archive into a folder of its own and removed after)
async fn check_run(cli: &str, dir: &str, commit: &str, home: &str, trigger: &str) -> J {
    let r = sh_ro(PREP, &[dir, commit], "/").await;
    if r.code != 0 { return err(or(bj::trim(&r.out), "could not take out that commit")) }
    let ls = lines(&r.out);
    let (tmp, d, root) = (at(&ls, 0), at(&ls, 1), at(&ls, 2));
    let f = project::read(d);
    let why = bound(root, &f).await;
    let o = if why.is_empty() { checks_inputs(cli, tmp, d, home, trigger, &f).await } else { err(why) };
    proc_ro("rm", &["-rf", tmp], "/", &[]).await;
    o
}
/// How far back the log is read for a question still open. The read model keeps no asks; a question older than this (a restart closes them) is only asked twice.
const ASK_WINDOW: usize = 4096;
/// Checks.asked: a question with this id prefix is open, by the AskOpened and AskClosed lines given
fn asked_in(lines: &[String], pre: &str) -> bool {
    let mut open: Vec<String> = vec![];
    for l in lines.iter().filter(|l| l.contains(pre)) {
        let Ok(e) = serde_json::from_str::<Value>(l) else { continue };
        let id = e["id"].as_str().unwrap_or("");
        if !id.starts_with(pre) { continue }
        match e["$"].as_str() { Some("AskOpened") => open.push(id.to_string()), Some("AskClosed") => open.retain(|x| x != id), _ => {} }
    }
    !open.is_empty()
}
/// Hub.checks.ask: the question for a project's own checks, one open at a time for the same checks.json
async fn ask(hub: &Arc<Hub>, thread: &str, hash: &str, text: &str) {
    let pre = format!("checks:{hash}:");
    let n = hub.store.len();
    let asked = asked_in(&hub.store.range(n.saturating_sub(ASK_WINDOW), n).unwrap_or_default(), &pre);
    if thread.is_empty() || asked { return }
    let now = now_s();
    let detail = format!("Run this project's own checks? .backplane/checks.json runs these commands with sh, in the project's folder:\n\n{text}\nAllow them only if you trust where this project came from. Then press Refresh on Checks.");
    let _ = hub.commit(&json!({"$":"AskOpened","id":format!("{pre}{now}"),"thread":thread,"kind":"input","tool":"checks_ok","detail":detail,"options":format!("{CHECKS_YES}\nNot now"),"at":now})).await;
}
pub const CHECKS_YES: &str = "Run them";
/// Checks.answer.k's trust: "Run them" on an ask `checks:<sha256>:<n>` adds that sha256 to <home>/checks.trusted (server.bend's ChecksTrust). The
/// answer's AskClosed is the caller's. True when `ask` is such an ask, so the caller logs nothing more for it.
pub fn checks_answer(home: &str, ask: &str, answer: &str) -> bool {
    let Some(rest) = ask.strip_prefix("checks:") else { return false };
    if bj::trim(answer) != CHECKS_YES { return true }
    let hash = bj::take(rest, 64);
    if !hash.is_empty() && hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(format!("{home}/checks.trusted")) { let _ = writeln!(f, "{hash}"); }
    }
    true
}
fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }
/// RvJob.checks.k
fn checks_report(thread: &str, o: &J) -> Value {
    json!({"report":{"thread":thread,"page":"checks","md":o.get_str("md",""),"err":o.get_str("err",""),"gate":o.get_str("gate",""),"line":o.get_str("line",""),
        "tasks":o.get("tasks").map(J::value).unwrap_or(json!([])),"pending":o.get_str("pending",""),"pendrun":o.get_str("pendrun","")}})
}

// Checklist
/// RvJob.tick: a box ticked or cleared in the project's .backplane/checklist.md (made from the default list the first time); the boxes as they are now
fn tick(cwd: &str, index: u64) -> Option<String> {
    let (dir, path) = (format!("{cwd}/.backplane"), format!("{cwd}/.backplane/checklist.md"));
    let link = |p: &str| std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false);
    if link(&dir) || link(&path) { return None }
    let old = text_of(&path);
    let text = checks::toggle(or(&old, checks::DEFAULT), index);
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::write(&path, &text).ok()?;
    Some(text)
}

// Releases
fn rel_report(thread: &str, md: &str, why: &str) -> Value { json!({"report":{"thread":thread,"page":"release","md":md,"err":why}}) }
/// Rvr.run.cmd: an export counts when its command worked and left new nonempty output
async fn export_step(argv: &[String], label: &str) -> String {
    if argv.is_empty() { return format!("{label} failed: no export command") }
    let path = argv.iter().position(|a| a == "-o").and_then(|i| argv.get(i + 1)).cloned().unwrap_or_default();
    let before = sh(REL_OUTPUT, &[&path], "/").await;
    if before.code != 0 { return release::command_error(&format!("{label} output inspection"), before.code, &before.out) }
    let ran = kicad(argv).await;
    if ran.code != 0 { return release::command_error(label, ran.code, &ran.out) }
    let after = sh(REL_OUTPUT, &[&path], "/").await;
    if after.code != 0 { return release::command_error(&format!("{label} output inspection"), after.code, &after.out) }
    if bj::trim(&after.out).is_empty() || before.out == after.out { format!("{label} produced no new nonempty output") } else { String::new() }
}
/// Rvr.outputs.done: the BOM change, the notes, the file list, the metadata, and the staged folder renamed into place
async fn publish_stage(ls: &[String], tag: &str, sch: &str) -> J {
    let (t, src, gd, top, c, prev, pc, fin, stage, object) = (at(ls, 0), at(ls, 1), at(ls, 2), at(ls, 3), at(ls, 4), at(ls, 5), at(ls, 6), at(ls, 7), at(ls, 8), at(ls, 10));
    let rel = bj::drop(sch, 1 + bj::len(src));
    let prefix = bj::drop(src, 5 + bj::len(t));
    let now = hist_git(gd, c, top, &format!("{prefix}{rel}"));
    let was = if pc.is_empty() { String::new() } else { hist_git(gd, pc, top, &format!("{prefix}{rel}")) };
    let bom = bom_text(&was, &now).await;
    let notes = release::notes(tag, prev, c, if was.is_empty() { "" } else { &bom }, "");
    let lst = sh(REL_FILES, &[stage], "/").await;
    if lst.code != 0 { return err(release::command_error("list release outputs", lst.code, &lst.out)) }
    let files: Vec<String> = bj::trim(&lst.out).split('\n').filter(|x| !bj::trim(x).is_empty()).map(str::to_string).collect();
    let when = proc("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"], "/", &[]).await;
    if when.code != 0 { return err(release::command_error("release timestamp", when.code, &when.out)) }
    let written = std::fs::write(format!("{stage}/notes.md"), &notes).is_ok() & std::fs::write(format!("{stage}/manifest.json"), release::manifest(tag, c, bj::trim(&when.out), &files)).is_ok();
    if !written { return err("release metadata could not be written; no outputs published, retry the same tag") }
    let done = sh(REL_COMMIT, &[stage, fin, gd, tag, c], "/").await;
    if done.code != 0 { return err(release::command_error("publish output directory", done.code, &done.out)) }
    J::obj(&[("dir", J::str(fin)), ("files", J::Arr(files.into_iter().map(J::Str).collect())), ("notes", J::Str(notes)), ("prev", J::str(prev)), ("commit", J::str(c)), ("tagObject", J::str(object))])
}
/// Rvr.in: the outputs built from the snapshot into a private staging folder; both it and the snapshot go, and the lock with them
async fn release_in(ls: &[String], tag: &str, cli: &str) -> J {
    let f = project::read(at(ls, 1));
    let stage = at(ls, 8);
    let steps = release::plan_at(&release::defaults(), stage, cli, &f.pcb, &f.sch);
    let why = bound(&format!("{}/src", at(ls, 0)), &f).await;
    let made = proc("mkdir", &["-p", &format!("{stage}/gerbers")], "/", &[]).await;
    let o = if !why.is_empty() { err(why) }
        else if made.code != 0 { err(release::command_error("create export directory", made.code, &made.out)) }
        else if steps.is_empty() { err("no supported design files or export steps; no outputs published") }
        else {
            let mut e = String::new();
            for s in &steps { e = export_step(&s.argv, &format!("{} ({})", s.label, s.id)).await; if !e.is_empty() { break } }
            if e.is_empty() { publish_stage(ls, tag, &f.sch).await } else { err(format!("{e}; no outputs published, retry the same tag")) }
        };
    proc("rm", &["-rf", at(ls, 0), stage], "/", &[]).await;
    proc("rmdir", &[at(ls, 9)], "/", &[]).await;
    o
}
/// Rvr.child.at: the project in DIR at TAG, taken out with git archive; the tool's outputs, notes and a manifest into DIR/.backplane/releases/TAG
async fn release_child(cli: &str, dir: &str, tag: &str, commit: &str) -> J {
    let r = sh(REL_PREP, &[dir, tag, commit], "/").await;
    if r.code != 0 { return err(or(bj::trim(&r.out), "could not take out that tag")) }
    release_in(&lines(&r.out), tag, cli).await
}
fn first_field(out: &str) -> String { bj::trim(out).split('\t').next().unwrap_or("").to_string() }
/// Rvr.pub: only when asked (the person's click) the tag is pushed by its object, verified, and a GitHub release made with the files attached
async fn publish(cwd: &str, tag: &str, o: &J) -> String {
    let object = o.get_str("tagObject", "");
    let dir = o.get_str("dir", "");
    let refname = format!("refs/tags/{tag}");
    let pushed = git::push(cwd, &["push", "origin", &format!("{object}:{refname}")]).await;
    if pushed.code != 0 { return format!("Not published: the tag could not be pushed: {}", cut(&pushed.out, 300)) }
    let seen = git::push(cwd, &["ls-remote", "--refs", "--tags", "origin", &refname]).await;
    if seen.code != 0 { return format!("Not published: remote tag could not be verified: {}", cut(&seen.out, 300)) }
    if first_field(&seen.out) != object { return "Not published: remote tag does not match the validated release object".into() }
    let files: Vec<String> = o.get_obj("files").elems().iter().map(|x| format!("{dir}/{}", if let J::Str(s) = x { s.as_str() } else { "" })).collect();
    let gh = release::gh(tag, &format!("{dir}/notes.md"), &files);
    let made = git::plain(cwd, "gh", &gh[1..].iter().map(String::as_str).collect::<Vec<_>>()).await;
    let said = if made.code == 0 { format!("Published: {}", git::last_line(&made.out)) } else { format!("Not published on GitHub: {}", cut(&made.out, 300)) };
    if made.code != 0 { return said }
    let again = git::push(cwd, &["ls-remote", "--refs", "--tags", "origin", &refname]).await;
    if again.code == 0 && first_field(&again.out) == object { said } else { "Publication no longer matches the validated release object; review the remote tag and release".into() }
}
/// Rvr.output.error
fn output_error(o: &J) -> String {
    let e = o.get_str("err", "");
    if !e.is_empty() { return e.to_string() }
    let complete = checks::report_string(o, "dir") && checks::report_string(o, "notes") && checks::report_array(o, "files") && release::object_valid(o.get_str("tagObject", ""));
    if complete { String::new() } else { "release child returned no complete output manifest".into() }
}
/// Rvr.done.md
fn done_md(o: &J, published: &str) -> String {
    let files: Vec<String> = o.get_obj("files").elems().iter().map(|x| format!("- `{}`", if let J::Str(s) = x { s.as_str() } else { "" })).collect();
    format!("{}\n\n**Files** in `{}`\n\n{}{}", o.get_str("notes", ""), o.get_str("dir", ""), files.join("\n"), if published.is_empty() { String::new() } else { format!("\n\n{published}") })
}
/// RvJob.release: a release of the thread's folder at a resolved immutable commit
async fn release_job(cli: &str, thread: &str, cwd: &str, tag: &str, publish_it: bool, force: bool, home: &str) -> Value {
    if !release::valid(tag) || cwd.is_empty() { return rel_report(thread, "", "invalid release tag or project folder") }
    if copy() { return rel_report(thread, "", "No release: a hub on a copied log makes no releases") }
    let r = sh(REL_SOURCE, &[cwd], "/").await;
    if r.code != 0 { return rel_report(thread, "", &format!("No release: {}", bj::trim(&r.out))) }
    let commit = bj::trim(&r.out).to_string();
    let o = check_run(cli, cwd, &commit, home, "release").await;
    // an explicit empty gate is a successful check; a missing one, child errors and infrastructure failures are never bypassed by force
    let e = o.get_str("err", "");
    let why = if !e.is_empty() { e.to_string() } else if checks::report_string(&o, "gate") { String::new() } else { "checks returned no valid gate".into() };
    if !why.is_empty() { return rel_report(thread, "", &format!("No release: checks could not complete: {why}")) }
    let gate = o.get_str("gate", "");
    if !gate.is_empty() && !force { return rel_report(thread, "", &format!("No release: the checks fail ({gate}). Fix them, or release anyway.")) }
    let t = sh(REL_TAG, &[cwd, tag, &commit], "/").await;
    if t.code != 0 { return rel_report(thread, "", &format!("No release: the tag could not be made: {}", cut(&t.out, 300))) }
    let out = release_child(cli, cwd, tag, &commit).await;
    let why = output_error(&out);
    if !why.is_empty() { return rel_report(thread, "", &format!("The tag {tag} exists, but its outputs were not published: {why}")) }
    let said = if publish_it { publish(cwd, tag, &out).await } else { String::new() };
    rel_report(thread, &done_md(&out, &said), if said.starts_with("Not published") || said.starts_with("Publication no longer") { &said } else { "" })
}

/// RvJob.pr: after Backplane opens a pull request, its BOM change (where the branch left main to what it pushed) goes on it as a comment; what to tell the thread ("" nothing)
pub async fn pr_note(cwd: &str, url: &str) -> String {
    let sch = project::read(cwd).sch;
    let r = sh(PR, &[cwd], "/").await;
    if r.code != 0 { return String::new() }
    let ls = lines(&r.out);
    let top = at(&ls, 1);
    if sch.is_empty() || !sch.starts_with(&format!("{top}/")) { return String::new() }
    let spec = |c: &str| hist_git(at(&ls, 0), c, top, &bj::drop(&sch, 1 + bj::len(top)));
    let md = bom_text(&spec(at(&ls, 2)), &spec(at(&ls, 3))).await;
    if bj::trim(&md).is_empty() || md.starts_with("No changes") { return String::new() }
    let body = format!("**Parts changed**\n\n{}", bj::trim_end(&md));
    if git::plain(cwd, "gh", &["pr", "comment", url, "--body", &body]).await.code == 0 { "parts change posted on the pull request".into() } else { String::new() }
}

pub struct Rv;
impl RpcHandler for Rv {
    fn methods(&self) -> &'static [&'static str] { &["review.bom", "review.checks", "review.tick", "review.comment", "review.release"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let j = J::of(p);
            let thread = j.get_str("thread", "");
            let reply = |ok: bool, r: Value| vec![json!({"t":"reply","id":id,"ok":ok,"result":r})];
            if method == "review.comment" { // Rv.comment.k: the request is judged before anything is looked up
                let op = j.get_str("op", "");
                if !review::op_ok(op, &j) || thread.is_empty() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"a comment needs its text (or the comment it is about)"})] }
                if super::root_of(hub, thread).await.is_none() { return vec![threadpath::refusal(id)] }
                let (now, seq) = (now_s(), hub.store.len());
                let aid = format!("r{now}-{seq}");
                let ev = json!({"$":"ActivityLogged","thread":thread,"act":aid,"tone":review::TONE,"kind":op,"summary":review::op_summary(op, &aid, &j, "person"),"at":now});
                return match hub.commit(&ev).await { Ok(_) => vec![json!({"t":"reply","id":id,"ok":true})], Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e.to_string()})] };
            }
            // Hub.git -> Hub.folder.job: with no resolved folder nothing is scheduled (law thread_folder_refuses_job)
            let Some(cwd_p) = super::root_of(hub, thread).await else { return vec![threadpath::refusal(id)] };
            let cwd = cwd_p.to_string_lossy().into_owned();
            let home = hub.cfg.home.to_string_lossy().into_owned();
            match method {
                "review.bom" => reply(true, bom_job(hub, thread, &cwd, j.get_str("a", ""), j.get_str("b", "")).await),
                "review.checks" => {
                    let commit = j.get_str("commit", "");
                    if commit.starts_with('-') { return reply(false, json!({"error":"no project folder"})) }
                    let cli = ipc::runtime(hub).await.cli;
                    let o = check_run(&cli, &cwd, commit, &home, or(j.get_str("trigger", "manual"), "manual")).await;
                    let pending = o.get_str("pending", "");
                    if !pending.is_empty() { ask(hub, thread, pending, o.get_str("pendrun", "")).await }
                    reply(true, checks_report(thread, &o))
                }
                "review.tick" => {
                    let index = bj::nat(j.raw("index", "0")).unwrap_or(0);
                    if copy() { return reply(false, json!({"error":"a hub on a copied log writes no checklist"})) }
                    let (c, i) = (cwd.clone(), index);
                    match tokio::task::spawn_blocking(move || tick(&c, i)).await.ok().flatten() {
                        Some(text) => reply(true, json!({"ticked":{"thread":thread,"tasks":tasks_json(&checks::parse(&text)).value()}})),
                        None => reply(false, json!({"error":"the checklist could not be written"})),
                    }
                }
                _ => { // review.release
                    let cli = ipc::runtime(hub).await.cli;
                    reply(true, release_job(&cli, thread, &cwd, j.get_str("tag", ""), j.get_bool("publish", false), j.get_bool("force", false), &home).await)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> String { let p = std::env::temp_dir().join(format!("bp-rv-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p.to_string_lossy().into_owned() }
    #[test] fn versions_rebase_on_their_commit() {
        assert_eq!(rebase("/p/x.kicad_sch", "/p/a.kicad_sch"), "/p/a.kicad_sch"); assert_eq!(rebase("git\t/g\tc\t2\t/t/x", "/t/y"), "git\t/g\tc\t2\t/t/y"); assert_eq!(rebase("diff\na\nb", "/t/y"), "/t/y");
        assert_eq!(hist_git("/g", "c", "/tøp", "a/b"), "git\t/g\tc\t4\t/tøp/a/b");
        for (c, ok) in [("abcd", true), ("0123456789abcdef0123456789abcdef01234567", true), ("abc", false), ("--output=/tmp/x", false), ("HEAD", false), ("", false), ("a b c", false)] { assert_eq!(commit_word(c), ok, "{c}") }
    }
    #[test] fn a_bom_root_stays_in_the_folder() {
        assert_eq!(bom_root("/p", "/p/sub/a.kicad_sch", "/p/r.kicad_sch"), "/p/sub/a.kicad_sch");
        for b in ["", "/p/../etc/a.kicad_sch", "/q/a.kicad_sch", "/p/a.kicad_pcb", "git\t/g\tc\t2\t/p/a.kicad_sch", "/pa.kicad_sch"] { assert_eq!(bom_root("/p", b, "/p/r.kicad_sch"), "/p/r.kicad_sch", "{b}") }
    }
    #[test] fn ticking_writes_the_default_list_and_never_through_a_link() {
        let d = tmp("tick"); let t = tick(&d, 1).unwrap();
        assert!(t.starts_with("# Power\n- [ ] Every rail") && t.contains("- [x] Decoupling")); assert_eq!(std::fs::read_to_string(format!("{d}/.backplane/checklist.md")).unwrap(), t);
        let t2 = tick(&d, 1).unwrap(); assert!(t2.contains("- [ ] Decoupling"));
        let out = tmp("tick-out"); let l = tmp("tick-link"); std::fs::create_dir_all(format!("{l}/.backplane")).unwrap();
        std::os::unix::fs::symlink(format!("{out}/victim"), format!("{l}/.backplane/checklist.md")).unwrap(); assert_eq!(tick(&l, 0), None); assert!(!std::path::Path::new(&format!("{out}/victim")).exists());
        let l2 = tmp("tick-link2"); std::os::unix::fs::symlink(&out, format!("{l2}/.backplane")).unwrap(); assert_eq!(tick(&l2, 0), None); assert!(std::fs::read_dir(&out).unwrap().next().is_none());
    }
    #[test] fn one_question_at_a_time_for_the_same_checks() {
        let (o, c) = (r#"{"$":"AskOpened","id":"checks:ab:1","thread":"t"}"#.to_string(), r#"{"$":"AskClosed","id":"checks:ab:1","answer":"Not now"}"#.to_string());
        assert!(asked_in(&[o.clone()], "checks:ab:")); assert!(!asked_in(&[o.clone(), c.clone()], "checks:ab:")); assert!(!asked_in(&[o.clone()], "checks:cd:")); assert!(!asked_in(&[], "checks:ab:"));
    }
    #[test] fn a_yes_trusts_only_the_hash_of_a_checks_ask() {
        let h = tmp("trust"); let hash = "a".repeat(64);
        assert!(!checks_answer(&h, "other:1", "Run them")); assert!(checks_answer(&h, &format!("checks:{hash}:9"), "Not now")); assert!(!std::path::Path::new(&format!("{h}/checks.trusted")).exists());
        assert!(checks_answer(&h, &format!("checks:{hash}:9"), " Run them ")); assert_eq!(std::fs::read_to_string(format!("{h}/checks.trusted")).unwrap(), format!("{hash}\n"));
        assert!(checks_answer(&h, "checks:../../x:9", "Run them")); assert_eq!(std::fs::read_to_string(format!("{h}/checks.trusted")).unwrap(), format!("{hash}\n"));
    }
    #[tokio::test] async fn the_checks_run_with_no_cli_and_say_it_is_an_execution_failure() {
        let d = tmp("nocli"); let home = tmp("nocli-home");
        std::fs::write(format!("{d}/a.kicad_pro"), "{}").unwrap(); std::fs::write(format!("{d}/a.kicad_sch"), "(kicad_sch)").unwrap();
        let o = check_run("", &d, "", &home, "manual").await;
        assert_eq!(o.get_str("err", ""), format!("erc: check command failed (exit 127): {MISSING}\n")); assert_eq!(o.get_str("gate", "x"), ""); assert_eq!(o.get_obj("tasks").elems().len(), 15);
    }
    #[tokio::test] async fn own_checks_wait_for_their_hash_and_then_run() {
        let d = tmp("own"); let home = tmp("own-home"); std::fs::create_dir_all(format!("{d}/.backplane")).unwrap();
        let cj = r#"{"checks":[{"name":"touch","run":"echo ran > ran.txt; echo done"}]}"#; std::fs::write(format!("{d}/.backplane/checks.json"), cj).unwrap();
        let o = check_run("", &d, "", &home, "manual").await;
        let hash = hex::encode(Sha256::digest(cj.as_bytes()));
        assert_eq!((o.get_str("pending", ""), o.get_str("err", ""), o.get_str("gate", "")), (hash.as_str(), WAIT_OK, WAIT_OK)); assert!(o.get_str("pendrun", "").contains("touch: echo ran")); assert!(!std::path::Path::new(&format!("{d}/ran.txt")).exists());
        assert!(o.get_str("md", "").starts_with("**This project's own checks wait for your OK.**"));
        assert!(checks_answer(&home, &format!("checks:{hash}:1"), "Run them"));
        let o = check_run("", &d, "", &home, "manual").await;
        assert_eq!(o.get_str("pending", ""), ""); assert!(std::path::Path::new(&format!("{d}/ran.txt")).exists()); assert!(o.get_str("md", "").contains("| touch | pass | done |"), "{}", o.get_str("md", ""));
        std::fs::write(format!("{d}/.backplane/checks.json"), r#"{"checks":[{"run":"false"}]}"#).unwrap();
        assert!(!check_run("", &d, "", &home, "manual").await.get_str("pending", "").is_empty()); // a changed file asks again
    }
    #[tokio::test] async fn a_snapshot_with_a_link_or_an_escaping_sheet_is_refused() {
        let env = [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")];
        let d = tmp("snap"); let home = tmp("snap-home");
        let g = |a: Vec<&'static str>, d: String| async move { proc("git", &a, &d, &env).await };
        g(vec!["init", "-q"], d.clone()).await; std::fs::write(format!("{d}/a.kicad_pro"), "{}").unwrap();
        std::fs::write(format!("{d}/a.kicad_sch"), "(kicad_sch (sheet (property \"Sheetname\" \"x\") (property \"Sheetfile\" \"../../outside.kicad_sch\")))").unwrap();
        g(vec!["add", "."], d.clone()).await; g(vec!["commit", "-qm", "i"], d.clone()).await;
        let c = g(vec!["rev-parse", "HEAD"], d.clone()).await.out.trim().to_string();
        let o = check_run("", &d, &c, &home, "manual").await; assert!(o.get_str("err", "").contains("release input"), "{o:?}");
        std::os::unix::fs::symlink("/etc/passwd", format!("{d}/l")).unwrap(); g(vec!["add", "."], d.clone()).await; g(vec!["commit", "-qm", "l"], d.clone()).await;
        let c = g(vec!["rev-parse", "HEAD"], d.clone()).await.out.trim().to_string();
        let o = check_run("", &d, &c, &home, "manual").await; assert!(o.get_str("err", "").contains("snapshot contains symlinks"), "{o:?}");
        assert_eq!(check_run("", &d, "nosuchcommit", &home, "manual").await.get("gate"), None);
    }
    /// a stand-in kicad-cli: every export writes what -o names, ERC and DRC write empty reports
    fn fake_cli(dir: &str) -> String {
        let p = format!("{dir}/kicad-cli");
        std::fs::write(&p, "#!/bin/sh\nout=; prev=; for a in \"$@\"; do [ \"$prev\" = -o ] && out=$a; prev=$a; done\ncase \"$1 $2\" in\n'sch erc') echo '{\"sheets\":[]}' > \"$out\";;\n'pcb drc') echo '{\"violations\":[],\"unconnected_items\":[],\"schematic_parity\":[]}' > \"$out\";;\n*) case \"$out\" in */) mkdir -p \"$out\"; echo x >> \"$out/$3.gbr\";; *) echo x > \"$out\";; esac;;\nesac\n").unwrap();
        use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); p
    }
    fn sym(r: &str, v: &str) -> String { format!("(symbol (lib_id \"Device:R\") (property \"Reference\" \"{r}\") (property \"Value\" \"{v}\") (property \"Footprint\" \"R_0603\"))\n") }
    #[tokio::test] async fn a_release_tags_exports_and_publishes_only_when_asked() {
        let env = [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")];
        for (k, v) in env { std::env::set_var(k, v) }
        let (d, home, bin, bare) = (tmp("rel"), tmp("rel-home"), tmp("rel-bin"), tmp("rel-bare")); let cli = fake_cli(&bin);
        let g = |a: &[&str]| { let o = std::process::Command::new("git").args(a).current_dir(&d).envs(env).output().unwrap(); String::from_utf8_lossy(&o.stdout).trim().to_string() };
        g(&["init", "-q", "-b", "main"]); std::process::Command::new("git").args(["init", "-q", "--bare"]).current_dir(&bare).output().unwrap(); g(&["remote", "add", "origin", &bare]);
        std::fs::write(format!("{d}/.gitignore"), ".backplane/releases\n").unwrap();
        std::fs::write(format!("{d}/b.kicad_pro"), "{}").unwrap(); std::fs::write(format!("{d}/b.kicad_pcb"), "(kicad_pcb)").unwrap(); std::fs::write(format!("{d}/b.kicad_sch"), format!("(kicad_sch {})", sym("R1", "1k"))).unwrap();
        g(&["add", "."]); g(&["commit", "-qm", "one"]);
        for bad in ["a b", "-x", ""] { let r = release_job(&cli, "t", &d, bad, false, false, &home).await; assert_eq!(r["report"]["err"], "invalid release tag or project folder") }
        std::fs::write(format!("{d}/dirty.txt"), "x").unwrap();
        let r = release_job(&cli, "t", &d, "v1", false, false, &home).await; assert!(r["report"]["err"].as_str().unwrap().contains("commit or stash working changes"), "{r}"); std::fs::remove_file(format!("{d}/dirty.txt")).unwrap();
        let r = release_job(&cli, "t", &d, "v1", false, false, &home).await; assert_eq!(r["report"]["err"], "", "{r}");
        let md = r["report"]["md"].as_str().unwrap(); assert!(md.starts_with("# v1\n\nCommit `") && md.contains("First release.") && md.contains("- `b-bom.csv`") && md.contains("- `gerbers/gerbers.gbr`"), "{md}");
        let fin = format!("{d}/.backplane/releases/v1"); assert!(std::path::Path::new(&format!("{fin}/notes.md")).exists());
        let m: Value = serde_json::from_str(&std::fs::read_to_string(format!("{fin}/manifest.json")).unwrap()).unwrap(); assert_eq!(m["tag"], "v1"); assert_eq!(m["commit"], g(&["rev-parse", "HEAD"]).as_str());
        assert_eq!(g(&["ls-remote", "--tags", "origin"]), ""); // nothing leaves the machine unasked
        let r = release_job(&cli, "t", &d, "v1", false, false, &home).await; assert!(r["report"]["err"].as_str().unwrap().contains("already exist"), "{r}");
        // the second release: notes carry the BOM change, the tag goes out by its object, gh is the stand-in
        std::fs::write(format!("{d}/b.kicad_sch"), format!("(kicad_sch {}{})", sym("R1", "2k"), sym("R2", "2k"))).unwrap(); g(&["add", "."]); g(&["commit", "-qm", "two"]);
        std::fs::write(format!("{bin}/gh"), "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\necho https://example.invalid/releases/v2\n").unwrap();
        use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(format!("{bin}/gh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("PATH", format!("{bin}:{}", std::env::var("PATH").unwrap()));
        let r = release_job(&cli, "t", &d, "v2", true, false, &home).await; assert_eq!(r["report"]["err"], "", "{r}");
        let md = r["report"]["md"].as_str().unwrap(); assert!(md.contains("Changes since v1.") && md.contains("## BOM changes") && md.contains("| R1 | changed | value: 1k | value: 2k |") && md.ends_with("Published: https://example.invalid/releases/v2"), "{md}");
        let tag = g(&["rev-parse", "refs/tags/v2"]); assert!(g(&["ls-remote", "--tags", "origin"]).contains(&tag));
        let args = std::fs::read_to_string(format!("{bin}/gh.args")).unwrap(); assert!(args.starts_with("release\ncreate\nv2\n--title\nv2\n--notes-file\n") && args.contains("--verify-tag") && args.contains("/.backplane/releases/v2/b-bom.csv"), "{args}");
        // a tag that names another commit is refused
        std::fs::write(format!("{d}/b.kicad_pro"), "{ }").unwrap(); g(&["add", "."]); g(&["commit", "-qm", "three"]);
        let r = release_job(&cli, "t", &d, "v2", false, false, &home).await; assert!(r["report"]["err"].as_str().unwrap().contains("tag already names a different commit"), "{r}");
    }
    #[tokio::test] async fn a_failed_gate_stops_a_release_unless_forced_and_an_execution_failure_always() {
        for (k, v) in [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")] { std::env::set_var(k, v) }
        let (d, home, bin) = (tmp("gate"), tmp("gate-home"), tmp("gate-bin")); let cli = fake_cli(&bin);
        std::fs::write(&cli, "#!/bin/sh\nout=; prev=; for a in \"$@\"; do [ \"$prev\" = -o ] && out=$a; prev=$a; done\ncase \"$1 $2\" in\n'sch erc') echo '{\"sheets\":[{\"path\":\"/\",\"violations\":[{\"type\":\"t\",\"severity\":\"error\",\"description\":\"d\",\"items\":[]}]}]}' > \"$out\";;\n'pcb drc') echo '{\"violations\":[],\"unconnected_items\":[],\"schematic_parity\":[]}' > \"$out\";;\n*) case \"$out\" in */) mkdir -p \"$out\"; echo x >> \"$out/$3.gbr\";; *) echo x > \"$out\";; esac;;\nesac\n").unwrap();
        for a in [&["init", "-q"][..]] { proc("git", a, &d, &[]).await; }
        std::fs::write(format!("{d}/.gitignore"), ".backplane/releases\n").unwrap(); std::fs::write(format!("{d}/b.kicad_pro"), "{}").unwrap(); std::fs::write(format!("{d}/b.kicad_pcb"), "(kicad_pcb)").unwrap(); std::fs::write(format!("{d}/b.kicad_sch"), "(kicad_sch)").unwrap();
        proc("git", &["add", "."], &d, &[]).await; proc("git", &["commit", "-qm", "i"], &d, &[]).await;
        let r = release_job(&cli, "t", &d, "v1", false, false, &home).await; assert_eq!(r["report"]["err"], "No release: the checks fail (ERC: 1 error). Fix them, or release anyway.");
        assert_eq!(proc("git", &["tag"], &d, &[]).await.out, "");
        let r = release_job("/nonexistent/kicad-cli", "t", &d, "v1", false, true, &home).await; assert!(r["report"]["err"].as_str().unwrap().starts_with("No release: checks could not complete: erc: check command failed"), "{r}");
        let r = release_job(&cli, "t", &d, "v1", false, true, &home).await; assert_eq!(r["report"]["err"], "", "{r}");
    }
}
