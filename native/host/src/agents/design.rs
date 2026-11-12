//! Hardware threads (docs/git.md, "Design files on main"): a thread in a project with KiCad files
//! works in the main checkout for its design files, which the hub commits straight to main as it
//! goes (each turn's end, each SaveDocument), and in its own worktree for code, which lands the
//! normal way. One writer per board or schematic: each has a named lock (locks.rs), gated on the
//! hub's KiCad and file writes, released at the turn's end, and freed from a holder that sits idle
//! or is gone (a crash). The setting `kicad.main.<thread>` = "on" marks a thread that works this way.
use super::{lockrec::Party, lines::{hub_id, s}, now_s, Agents};
use crate::files::kicad::canonical as kc;
use serde_json::{json, Value};
use std::sync::Arc;

fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }
/// (agents/world.rs `cwd` reads the same key by its text: a fold input reaches nothing outside its inputs)
pub fn main_key(th: &str) -> String { format!("kicad.main.{th}") }

/// Commands that rewrite a whole document: the one place a writer still needs the document's lock with a co-editing KiCad.
pub fn structural(ty: &str) -> bool {
    matches!(ty.rsplit('.').next().unwrap_or(ty), "ImportNetlist" | "UpdatePCBFromSchematic" | "UpdateSchematicFromPCB" | "Annotate" | "AnnotateSchematic" | "RevertDocument" | "CloseDocument" | "OpenDocument" | "ImportSpecctraSession")
}

impl Agents {
    /// The thread works in the main checkout for design files.
    pub async fn on_main(&self, th: &str) -> bool { self.world().await.setting(&main_key(th)) == "on" }
    /// The main checkout of a thread's project ("" for none).
    async fn main_root(&self, th: &str) -> String {
        let w = self.world().await;
        let Some(t) = w.threads.get(th) else { return String::new() };
        let root = w.roots.get(&t.project).cloned().unwrap_or_default();
        if root.is_empty() { kc::main_of(&t.worktree) } else { kc::main_of(&root) }
    }
    async fn say(&self, th: &str, tone: &str, kind: &str, summary: String) {
        let now = now_s();
        self.commit_events(vec![json!({"$":"ActivityLogged","thread":th,"act":hub_id("kd", now, self.seq_n()),"tone":tone,"kind":kind,"summary":summary,"at":now})]).await
    }

    /// One writer per document: a write to a board or schematic goes ahead only for the thread
    /// holding that document's lock; any other is told who holds it, as a queued locker is —
    /// nothing blocks. Answers the main checkout's path the request must use. Reads are free.
    pub async fn kicad_gate(&self, thread: &str, path: &str, ty: &str) -> Result<String, String> {
        let routed = kc::route(path);
        if crate::copy::on() || crate::files::kicad::ipc::is_read(ty) { return Ok(routed) }
        let doc = kc::doc_of(&routed);
        if doc.is_empty() || kc::split_worktree(&doc).is_some() { return Ok(routed) } // none (the IPC check names that), or an explicit experiment copy
        // A co-editing KiCad has the document open: agents and the person edit it live, together (docs/kicad-coedit.md); only
        // whole-document commands still take the lock (an annotation or a netlist import rewrites everything).
        if crate::files::kicad::ipc::coedit_live(&routed) && !structural(ty) { return Ok(routed) }
        self.doc_gate(thread, &doc).await.map(|_| routed)
    }
    /// The same gate for a direct write of a board or schematic (`files_write`): Err names the holder.
    pub async fn doc_gate(&self, thread: &str, doc: &str) -> Result<(), String> {
        if crate::copy::on() || !kc::is_document(doc) { return Ok(()) }
        let name = kc::doc_lock(doc);
        match kc::write_blocked(&self.lock_get(&name).await, thread, &name, doc) { None => Ok(()), Some(why) => Err(why) }
    }

    /// The board or schematic a `files_write` of `rel` lands on (main's copy), if it is one.
    pub async fn files_doc(&self, thread: &str, rel: &str) -> Option<String> {
        let root = crate::files::root_of(&self.hub, thread).await?; let root = root.to_string_lossy().into_owned();
        let (r, p) = kc::direct(&root, rel);
        let abs = if p.starts_with('/') { p } else { format!("{r}/{p}") };
        kc::is_document(&abs).then_some(abs)
    }
    /// Commit the design files changed in the thread's main checkout straight to main (and push
    /// when it has an upstream), leaving any document another thread holds for that thread.
    /// `why`: "turn" (a turn's end) or "save" (a SaveDocument). Each commit is a line in the thread.
    pub async fn design_record(self: &Arc<Self>, th: &str, why: &str) {
        if crate::copy::on() { return }
        let root = self.main_root(th).await; if root.is_empty() || !kc::hardware(&root) { return }
        let (title, mine) = { let w = self.world().await; (w.threads.get(th).map(|t| t.title.clone()).unwrap_or_default(), w.setting(&main_key(th)) == "on") };
        if !mine && why == "turn" { return } // a thread still in an older worktree edits its own copies (bring_back moves them)
        let mut held: Vec<String> = vec![];
        for n in self.lock_names().await.into_iter().filter(|n| kc::is_doc_lock(n)) {
            let l = self.lock_get(&n).await;
            if let Some(h) = &l.holder { if h.thread != th { held.push(n) } }
        }
        // the documents of the project whose lock another thread holds
        let f = kc::for_cwd(&root);
        let skip: Vec<String> = [f.pcb, f.sch].into_iter().filter(|d| !d.is_empty() && held.contains(&kc::doc_lock(d))).collect();
        let msg = format!("design: {} ({th})", if title.is_empty() || title == "New thread" { "KiCad work".to_string() } else { title.chars().take(72).collect() });
        match kc::commit_design(&root, &msg, &skip).await {
            Ok(r) if r.commit.is_empty() => {}
            Ok(r) => {
                crate::settings::telemetry_usage::bump(crate::settings::telemetry_usage::BOARDS_CREATED, r.new_boards);
                // a STEP written this turn is meshed now, in the background, so its first showing reads it from the cache
                let steps: Vec<String> = r.files.iter().filter(|p| crate::files::kicad::solidread::is_step(p)).map(|p| format!("{root}/{p}")).collect();
                if !steps.is_empty() { let home = self.hub.cfg.home.clone(); tokio::task::spawn_blocking(move || { let tools = crate::files::kicad::stepglb::machine(&home); for p in steps { crate::files::kicad::solidread::warm(&p, &tools); } }); }
                let n = r.files.len();
                let pushed = if r.pushed.is_empty() { String::new() } else if r.pushed.starts_with("not pushed") { format!("; {}", r.pushed) } else { format!(", pushed to {}", r.pushed) };
                let left = if skip.is_empty() { String::new() } else { format!(" (left for its writer: {})", skip.iter().map(|d| d.rsplit('/').next().unwrap_or(d)).collect::<Vec<_>>().join(", ")) };
                self.say(th, "info", "git", format!("Committed {n} design file{} to main as {}{pushed}{left}: {}", if n == 1 { "" } else { "s" }, r.commit, r.files.iter().take(6).cloned().collect::<Vec<_>>().join(", "))).await
            }
            Err(e) => self.say(th, "warn", "git", format!("Design files not committed to main: {e}")).await,
        }
    }
    /// The documents' locks a thread holds go when its turn ends (a waiter is granted at once).
    pub async fn doc_locks_free(self: &Arc<Self>, th: &str) -> Vec<String> {
        let mut freed = vec![];
        for n in self.lock_names().await.into_iter().filter(|n| kc::is_doc_lock(n)) {
            let _g = self.work_lock.lock().await;
            let l = self.lock_get(&n).await;
            if l.owner() == th && self.lock_free(&n, th).await { freed.push(n) } // a thread waiting in the queue stays there: its grant wakes it
        }
        freed
    }
    /// A turn ended: its design work is committed to main, then its documents are let go.
    pub async fn design_turn_end(self: &Arc<Self>, th: &str) {
        if crate::copy::on() { return }
        self.design_record(th, "turn").await;
        self.doc_locks_free(th).await;
    }
    /// A hardware thread's code worktree is there before its turn: one removed after it settled is
    /// made again from its branch, without design files.
    pub async fn design_ensure(&self, th: &str) {
        if crate::copy::on() || !self.on_main(th).await { return }
        let (wt, branch) = { let w = self.world().await; w.threads.get(th).map(|t| (t.worktree.clone(), t.branch.clone())).unwrap_or_default() };
        if wt.is_empty() || std::path::Path::new(&wt).is_dir() || kc::split_worktree(&wt).is_none() { return }
        let main = kc::main_of(&wt);
        let top = crate::files::git::run_ro(&main, &["rev-parse", "--show-toplevel"]).await.out.trim().to_string();
        let wtop = kc::split_worktree(&wt).map(|(m, r)| { let first = r.split('/').next().unwrap_or(""); format!("{m}/.backplane/worktrees/{first}") }).unwrap_or_default();
        if top.is_empty() || wtop.is_empty() { return }
        let _ = crate::files::git::run(&top, &["worktree", "prune"]).await;
        let r = if branch.is_empty() { crate::files::git::run(&top, &["worktree", "add", "--detach", &wtop, "HEAD"]).await } else { crate::files::git::run(&top, &["worktree", "add", &wtop, &branch]).await };
        if r.code == 0 { let _ = kc::strip_design(&wtop).await; }
    }

    /// `kicad_bring_back`: an older hardware worktree's design changes go back to the main checkout
    /// (main's copies backed up first), are committed to main, and the thread works in the main
    /// checkout from then on. Only when asked (the person said yes to the offer): never on its own.
    pub async fn tool_bring_back(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        if crate::copy::on() { return fail("a copy of a log changes no folders") }
        let target = if s(a, "thread").is_empty() { thread.to_string() } else { s(a, "thread").to_string() };
        let (wt, title) = { let w = self.world().await; match w.threads.get(&target) { Some(t) => (t.worktree.clone(), t.title.clone()), None => return fail("unknown thread") } };
        if wt.is_empty() || kc::split_worktree(&wt).is_none() { return fail("that thread has no worktree: its design is already in the main checkout") }
        let main = kc::main_of(&wt);
        if !kc::hardware(&main) { return fail("that project has no KiCad files") }
        let files = kc::worktree_changes(&main, &wt);
        if s(a, "mode") != "apply" {
            return (true, json!({"thread":target,"worktree":wt,"main":main,"files":files,"next":"call again with mode \"apply\" once the person agrees: main's copies are backed up first"}))
        }
        // a document someone else is writing is not overwritten under them
        for d in files.iter().filter(|f| kc::is_document(f)) {
            let abs = format!("{main}/{d}");
            if let Err(why) = self.doc_gate(thread, &abs).await { if !why.starts_with("writes to") { return fail(&why) } }
        }
        let stamp = chrono_stamp();
        let r = { let (m, w, t) = (main.clone(), wt.clone(), target.clone()); tokio::task::spawn_blocking(move || kc::bring_back(&m, &w, &t, &stamp)).await.unwrap_or_else(|e| Err(e.to_string())) };
        let (backup, moved) = match r { Ok(x) => x, Err(e) => return fail(&format!("nothing was changed on main past the backup: {e}")) };
        let stripped = kc::strip_design(&wt).await.unwrap_or(0);
        let now = now_s();
        self.commit_events(vec![json!({"$":"SettingSet","key":main_key(&target),"value":"on","at":now}), json!({"$":"SettingSet","key":format!("kicad.offer.{target}"),"value":"done","at":now})]).await;
        let msg = format!("design: brought back from {} ({target})", if title.is_empty() { target.clone() } else { title.chars().take(60).collect() });
        let rec = kc::commit_design(&main, &msg, &[]).await;
        let line = match &rec { Ok(r) if !r.commit.is_empty() => format!("Brought {} design file{} back to main as {} (backup {backup}); this thread now works in the main checkout for design files", moved.len(), if moved.len() == 1 { "" } else { "s" }, r.commit), Ok(_) => format!("No design changes to bring back; this thread now works in the main checkout for design files"), Err(e) => format!("Brought {} design files back to main (backup {backup}) but did not commit: {e}", moved.len()) };
        self.say(&target, "info", "git", line.clone()).await;
        (true, json!({"thread":target,"backup":backup,"files":moved,"stripped":stripped,"commit":rec.as_ref().map(|r| r.commit.clone()).unwrap_or_default(),"said":line}))
    }
    /// The offer, once per older hardware thread whose worktree has design changes main lacks:
    /// a line in the thread saying what differs and how to bring it back. Never applied on its own.
    pub async fn design_offer(self: &Arc<Self>, th: &str) {
        if crate::copy::on() || self.on_main(th).await { return }
        let (wt, offered) = { let w = self.world().await; (w.threads.get(th).map(|t| t.worktree.clone()).unwrap_or_default(), w.setting(&format!("kicad.offer.{th}")).to_string()) };
        if wt.is_empty() || !offered.is_empty() || kc::split_worktree(&wt).is_none() { return }
        let main = kc::main_of(&wt);
        let files = { let (m, w) = (main.clone(), wt.clone()); tokio::task::spawn_blocking(move || if kc::hardware(&m) { kc::worktree_changes(&m, &w) } else { vec![] }).await.unwrap_or_default() };
        let now = now_s();
        self.commit_events(vec![json!({"$":"SettingSet","key":format!("kicad.offer.{th}"),"value":if files.is_empty() { "none" } else { "offered" },"at":now})]).await;
        if files.is_empty() { return }
        self.say(th, "warn", "git", format!("This thread's worktree has design changes the main checkout does not ({}). KiCad work now lives on main: ask the agent to bring them back (kicad_bring_back; main's copies are backed up first). Nothing changes until you say so.", files.iter().take(5).cloned().collect::<Vec<_>>().join(", "))).await;
    }
}

/// From locks_tick, for each held lock: a document's lock whose thread has no turn running and no
/// work of its own for IDLE_GRACE seconds (it went idle, or the hub or its agent crashed while it
/// held it) is freed, its waiters granted. true when it was.
pub(super) async fn doc_lock_idle(ag: &Arc<Agents>, name: &str, h: &Party, now: u64) -> bool {
    if !kc::is_doc_lock(name) || h.kind != "thread" { return false }
    let idle = !ag.party_busy(h).await;
    if !kc::idle_long(name, &h.id, idle, now) { return false }
    if !ag.lock_free(name, &h.id).await { return false }
    ag.lock_tell(&h.thread, name, &format!("Lock {name} was released: your turn had ended and nothing of yours was running.")).await;
    true
}

fn chrono_stamp() -> String {
    let t = now_s(); let (d, s) = (t / 86400, t % 86400);
    // civil date from days since 1970 (Howard Hinnant's algorithm)
    let z = d as i64 + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097; let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153; let dd = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y + (m <= 2) as i64, m, dd, s / 3600, s % 3600 / 60, s % 60)
}
