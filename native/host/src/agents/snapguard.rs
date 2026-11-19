//! The hub's guard around an agent's turn (#1163), on files/snapshot.rs:
//! - before the agent gets the turn's input, the project folder is made a git repository if it is in none, and the work tree is
//!   snapshotted (`turn_guard_start`); the turn's first write through the hub (kicad_edit, a kicad_ipc write, a files_* write) snapshots
//!   again if anything changed since (`write_guard`). This is the hub's doing, never the agent's.
//! - a review request runs read-only: the hub refuses its writes to design files, and Claude is started with deny rules for them.
//! - a mass deletion through KiCad's IPC (net loss of more than a fifth of a document's items in one turn) waits for the person's yes
//!   (`mass_guard`); at the turn's end the design files are compared with the snapshot, and a mass loss, a deleted board or any change a
//!   review made opens an ask to restore (`turn_guard_end`).
//! - `/restore` (or `/restore <turn>`) from the person, and the ask's Restore, put the project back to before a turn as a new commit
//!   (`restore_turn`); the state before the restore is kept as a commit too.
use super::{lines::hub_id, now_s, Agents};
use crate::files::kicad::canonical as kc;
use crate::files::snapshot as sn;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// The ask a risky turn's end opens.
pub const RESTORE_TOOL: &str = "snapshot_restore";
pub const RESTORE_YES: &str = "Restore to before this turn";
pub const RESTORE_NO: &str = "Keep this turn's changes";
/// The ask a thread shows when git is missing, and what the turn says.
pub const GIT_TOOL: &str = "git_install";
pub const GIT_YES: &str = "Install Git";
pub const GIT_MISSING: &str = "Not started: git is not installed on this computer. Backplane snapshots the project with git before an agent may change it, so agents do not start without it. Press Install Git, then send your message again.";
/// The hub's own lines and settings (snapshots, review mode, restore asks), which the JS hub never wrote: left out of its parity tests.
pub fn is_guard_event(e: &Value) -> bool {
    (e["$"] == "ActivityLogged" && e["kind"] == "git" && e["act"].as_str().is_some_and(|a| a.starts_with("sg")))
        || (e["$"] == "SettingSet" && e["key"].as_str().is_some_and(|k| k.starts_with("review.ro.") || k.starts_with("restore.ask.")))
}
/// Whether git runs here (Windows: one installed since this hub started is put on its PATH first).
pub async fn git_runs() -> bool {
    crate::settings::tools_install::git_pick_up();
    crate::files::git::proc_ro("git", &["--version"], "/", &[]).await.code == 0
}
/// The ask a mass deletion waits on.
pub const MASS_TOOL: &str = "mass_delete";
/// What a review turn's Claude is told it may not use (deny rules apply in every permission mode).
pub const CLAUDE_DENY: &[&str] = &["Edit(**/*.kicad_pcb)", "Edit(**/*.kicad_sch)", "Edit(**/*.kicad_pro)", "Edit(**/*.kicad_sym)", "Edit(**/*.kicad_mod)", "Edit(**/*.kicad_dru)", "Edit(**/fp-lib-table)", "Edit(**/sym-lib-table)", "Edit(**/*.step)", "Edit(**/*.FCStd)", "Edit(**/*.fab.json)",
    "Write(**/*.kicad_pcb)", "Write(**/*.kicad_sch)", "Write(**/*.kicad_pro)", "Write(**/*.kicad_sym)", "Write(**/*.kicad_mod)", "Write(**/*.kicad_dru)",
    "Bash(rm:*)", "Bash(mv:*)", "Bash(git checkout:*)", "Bash(git restore:*)", "Bash(git reset:*)", "Bash(git clean:*)", "Bash(git stash:*)", "Bash(git rm:*)"];
/// The setting that says a thread's turns are reviews (read-only), "1" while they are.
pub fn review_key(th: &str) -> String { format!("review.ro.{th}") }

/// How long a turn waits for its pre-turn snapshot before the agent starts anyway (the hidden setting `snap.wait.ms` overrides it, for tests).
pub const SNAP_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// The longest a write through the hub waits for its turn's late snapshot.
pub const PENDING_CAP: std::time::Duration = std::time::Duration::from_secs(300);

/// One turn's guard state.
#[derive(Debug, Default, Clone)]
pub struct TurnGuard {
    /// the repository's top and the turn's snapshot ("" when there is none)
    pub top: String, pub turn: u64, pub reference: String,
    pub review: bool, pub wrote: bool,
    /// per document: (items when first touched, net items deleted, deleted since the open BeginCommit), and whether the person said yes
    pub docs: HashMap<String, (usize, i64, i64)>, pub confirmed: Vec<String>,
    /// the turn was started by the person's own message (they are there to answer a mass-deletion ask)
    pub person: bool,
    /// the turn's snapshot is still being taken after the agent started (`snap_wait`): true once it ended. A write through the hub waits for it,
    /// so the snapshot never holds the agent's own edit.
    pub pending: Option<tokio::sync::watch::Receiver<bool>>,
}
/// How long a mass deletion waits for the person's answer before it is refused. While it waits no KiCad request is sent (the guard runs
/// before the document's gate and the call); a document lock this turn already took stays held, as it does for the rest of the turn.
pub const MASS_WAIT: std::time::Duration = std::time::Duration::from_secs(300);

/// Whether the next turn is a review: a request for one starts it, and it lasts through the follow-ups until a message asks for a change.
pub fn review_next(prev: bool, text: &str) -> bool { sn::is_review(text) || (prev && !asks_change(text)) }
fn asks_change(text: &str) -> bool { !sn::is_review(&format!("review {text}")) }
/// What a review turn may still send to KiCad: reads, opening a document, the view (layers, selection), checks and exports.
pub fn review_allows(ty: &str) -> bool {
    let n = ty.rsplit('.').next().unwrap_or(ty);
    crate::files::kicad::ipc::is_read(ty) || matches!(n, "OpenDocument" | "SetActiveLayer" | "SetVisibleLayers" | "HitTest" | "RunChecks" | "RunDrc" | "RunErc") || n.contains("Selection") || n.starts_with("Export") || n.starts_with("Plot")
}
/// `/restore` or `/restore <turn>` from the person: Some(turn, 0 for the last one).
pub fn restore_command(text: &str) -> Option<u64> {
    let t = text.trim(); let rest = t.strip_prefix("/restore")?;
    if rest.is_empty() { return Some(0) }
    if !rest.starts_with(' ') { return None }
    rest.trim().parse().ok()
}
/// Items a KiCad document's DeleteItems / CreateItems message names.
fn n_items(msg: &Value, k: &str) -> i64 { msg[k].as_array().map_or(0, |a| a.len() as i64) }

impl Agents {
    async fn guard_say(&self, th: &str, tone: &str, summary: String) {
        let now = now_s();
        self.commit_events(vec![json!({"$":"ActivityLogged","thread":th,"act":hub_id("sg", now, self.seq_n()),"tone":tone,"kind":"git","summary":summary,"at":now})]).await
    }
    pub fn turn_guard(&self, th: &str) -> Option<TurnGuard> { self.turn_guards.lock().unwrap().get(th).cloned() }
    /// Settings > Projects and git > Keep Backplane's commits off my branch (settings/offbranch.rs, #1188): snapshots go on Backplane's refs
    /// even in a repository it made.
    async fn off_branch(&self) -> bool { crate::settings::offbranch::on(self.world().await.setting(crate::settings::offbranch::KEY)) }
    /// Whether this thread's turn is a review (read-only).
    pub async fn in_review(&self, th: &str) -> bool { self.turn_guard(th).map_or_else(|| false, |g| g.review) || self.world().await.setting(&review_key(th)) == "1" }

    /// Before a turn's input reaches the agent: the folder made a repository if it is in none, the review mode decided from the request
    /// (`text`, None to keep the last one), and the work tree snapshotted. Never fails the turn: what could not be done is said.
    /// Err: git is not on this computer, so the turn does not start (nothing could be snapshotted); the thread has an Install Git ask.
    pub async fn turn_guard_start(self: &Arc<Self>, th: &str, cwd: &str, text: Option<&str>) -> Result<(), String> { self.turn_guard_start_by(th, cwd, text, false).await }
    /// `turn_guard_start`; `person`: the person's own message started this turn (they can answer a mass-deletion ask).
    pub async fn turn_guard_start_by(self: &Arc<Self>, th: &str, cwd: &str, text: Option<&str>, person: bool) -> Result<(), String> {
        if crate::copy::on() || cwd.is_empty() { return Ok(()) }
        // a bot's own thread works in its own folder (bots/rpc.rs makes it a repository), not a project: never blocked for git, never snapshotted
        if self.world().await.bot_of.contains_key(th) { return Ok(()) }
        if !git_runs().await { self.git_missing(th).await; return Err(GIT_MISSING.into()) }
        self.close_git_asks(th).await; // installed meanwhile (setup, or this ask's own press): the ask is done
        self.guard_begin(th, cwd, text, true).await;
        if let Some(g) = self.turn_guards.lock().unwrap().get_mut(th) { g.person = person }
        Ok(())
    }
    /// `turn_guard_start`; `new_turn` false for a guard taken late, at a write of a turn that has none (an earlier turn's restore ask stays open).
    async fn guard_begin(self: &Arc<Self>, th: &str, cwd: &str, text: Option<&str>, new_turn: bool) {
        if crate::copy::on() || cwd.is_empty() { return }
        let prev = self.world().await.setting(&review_key(th)) == "1";
        let review = text.map_or(prev, |t| review_next(prev, t));
        if review != prev { let now = now_s(); self.commit_events(vec![json!({"$":"SettingSet","key":review_key(th),"value":if review { "1" } else { "" },"at":now})]).await }
        if review && !prev { self.guard_say(th, "info", "This request is a review: the design files are read-only for it (Backplane refuses writes to them). Ask for a change in a message and that turn may edit.".into()).await }
        if new_turn { self.close_restore_asks(th).await }
        let root = kc::design_root(cwd);
        if !std::path::Path::new(&root).is_dir() { self.turn_guards.lock().unwrap().insert(th.to_string(), TurnGuard { review, ..Default::default() }); return } // no folder: nothing to snapshot (the turn says why it cannot run there)
        let g = TurnGuard { review, ..Default::default() };
        // The snapshot of a big folder (a first commit of a folder in no repository, `git add -A` over a large tree, a slow disk, Windows process
        // spawns) can take minutes (#1233, #1246): the agent waits for it only up to `snap_wait`; past that it starts, and the snapshot finishes in
        // its own task, which then fills this turn's guard (a write before that takes its own snapshot, as for a turn that has none).
        let wait = self.world().await.setting("snap.wait.ms").parse().map_or(SNAP_WAIT, std::time::Duration::from_millis);
        let (me, th2, root2) = (self.clone(), th.to_string(), root.clone());
        let mut task = tokio::spawn(async move { me.take_snapshot(&th2, &root2).await });
        let mut g = g;
        let g = match tokio::time::timeout(wait, &mut task).await {
            Ok(r) => match r.ok().flatten() { Some((top, turn, reference)) => TurnGuard { top, turn, reference, ..g }, None => g },
            Err(_) => {
                self.guard_say(th, "info", format!("The snapshot of this project is taking more than {} s (a large folder). The agent starts now and the snapshot finishes meanwhile.", wait.as_secs_f32())).await;
                let (me, th2) = (self.clone(), th.to_string());
                let (tx, rx) = tokio::sync::watch::channel(false);
                g.pending = Some(rx);
                tokio::spawn(async move {
                    if let Some((top, turn, reference)) = task.await.ok().flatten() {
                        if let Some(g) = me.turn_guards.lock().unwrap().get_mut(&th2) { if g.top.is_empty() { g.top = top; g.turn = turn; g.reference = reference } }
                    }
                    let _ = tx.send(true);
                });
                g
            }
        };
        self.turn_guards.lock().unwrap().insert(th.to_string(), g);
    }

    /// The folder `root` made a repository if it is in none, and its work tree snapshotted for `th`'s next turn: (top, turn, reference), None when
    /// there is no repository or no snapshot (said in the thread).
    async fn take_snapshot(self: &Arc<Self>, th: &str, root: &str) -> Option<(String, u64, String)> {
        let uh = std::path::PathBuf::from(&self.host.user_home);
        let off = self.off_branch().await;
        let made = sn::ensure_repo(root, Some(&uh), off).await;
        match &made { Ok(e) => { let l = e.line(); if !l.is_empty() { self.guard_say(th, if e.not_made.is_empty() { "info" } else { "warn" }, l).await } } Err(e) => self.guard_say(th, "warn", format!("Not made a git repository: {e}")).await }
        let top = sn::top_of(root).await?;
        let turn = sn::next_turn(&top, th).await;
        let title = self.thread_title(th).await;
        match sn::snapshot(&top, th, &sn::snap_ref(th, turn), &sn::snap_msg(th, turn, &title, ""), off).await {
            // every turn's snapshot is logged, a quiet line (present::thread::quiet_act): the person's message offers Restore to before this from it
            Ok(s) => { self.guard_say(th, "info", s.line()).await; Some((top, turn, s.reference)) }
            Err(e) => { self.guard_say(th, "warn", format!("No snapshot before this turn: {e}")).await; None }
        }
    }

    /// Before a write through the hub to `path` (absolute): a review refuses it when it is a design file; the turn's first write snapshots
    /// first (again, when the person or the agent changed something since the turn's snapshot).
    pub async fn write_guard(self: &Arc<Self>, th: &str, path: &str) -> Result<(), String> {
        if crate::copy::on() { return Ok(()) }
        let rel = path.rsplit_once("/.backplane/worktrees/").map(|x| x.1.split_once('/').map_or("", |y| y.1).to_string()).unwrap_or_else(|| path.to_string());
        let design = kc::is_design(&rel) || kc::is_document(path) || path.ends_with(".kicad_pro");
        if design && self.in_review(th).await { return Err(sn::review_refusal(path.rsplit('/').next().unwrap_or(path))) }
        // the turn's snapshot is still running (it outlasted `snap_wait`): the agent's edit waits for it, so the snapshot is of the state before the turn
        if let Some(mut rx) = self.turn_guard(th).and_then(|g| g.pending) { let _ = tokio::time::timeout(PENDING_CAP, rx.wait_for(|d| *d)).await; }
        let g = self.turn_guard(th);
        if g.as_ref().is_some_and(|g| g.wrote) { return Ok(()) }
        match g {
            Some(g) if !g.top.is_empty() => {
                let title = self.thread_title(th).await;
                let r = sn::snapshot(&g.top, th, &format!("{}-edit", g.reference), &sn::snap_msg(th, g.turn, &title, "before its first edit"), self.off_branch().await).await;
                if let Err(e) = r { self.guard_say(th, "warn", format!("No snapshot before this turn's first edit: {e}")).await }
            }
            // no snapshot of this turn yet (a turn that started before this hub did): take it now
            _ if !git_runs().await => { self.git_missing(th).await; return Err(GIT_MISSING.into()) }
            _ => { let cwd = self.world().await.cwd(th); let review = self.in_review(th).await; self.guard_begin(th, &cwd, None, false).await; if let Some(x) = self.turn_guards.lock().unwrap().get_mut(th) { x.review = review } }
        }
        if let Some(x) = self.turn_guards.lock().unwrap().get_mut(th) { x.wrote = true }
        Ok(())
    }

    /// Before a KiCad IPC write: the review and first-write guard, then the mass-deletion count (net items deleted from the document this
    /// turn; a commit dropped with CMA_DROP gives its deletions back). Past a fifth of the document's items and the floor, the person is
    /// asked and the call waits for their answer; an agent cannot say yes for them.
    pub async fn mass_guard(self: &Arc<Self>, th: &str, doc: &str, ty: &str, msg: &Value) -> Result<(), String> {
        if crate::copy::on() || doc.is_empty() { return Ok(()) }
        let n = ty.rsplit('.').next().unwrap_or(ty);
        if self.in_review(th).await && !review_allows(ty) { return Err(sn::review_refusal(doc.rsplit('/').next().unwrap_or(doc))) }
        if review_allows(ty) { return Ok(()) }
        self.write_guard(th, doc).await?;
        let (del, add) = match n { "DeleteItems" => (n_items(msg, "item_ids"), 0), "CreateItems" => (0, n_items(msg, "items")), _ => (0, 0) };
        let schematic = doc.ends_with(".kicad_sch");
        let known = self.turn_guards.lock().unwrap().get(th).is_some_and(|g| g.docs.contains_key(doc));
        let base = if known { 0 } else { let d = doc.to_string(); tokio::task::spawn_blocking(move || std::fs::read_to_string(&d).map(|t| sn::count_items(&t, schematic)).unwrap_or(0)).await.unwrap_or(0) };
        let (before, net, confirmed) = {
            let mut m = self.turn_guards.lock().unwrap(); let g = m.entry(th.to_string()).or_default();
            let e = g.docs.entry(doc.to_string()).or_insert((base, 0, 0));
            match n {
                "BeginCommit" => e.2 = 0,
                "EndCommit" => { if msg.to_string().contains("CMA_DROP") { e.1 -= e.2 } e.2 = 0 }
                _ => {}
            }
            (e.0, e.1 + del - add, g.confirmed.contains(&doc.to_string()))
        };
        if del > 0 && !confirmed {
            if let Some(line) = sn::mass_loss(doc.rsplit('/').next().unwrap_or(doc), before, (before as i64 - net).max(0) as usize) {
                // nobody to answer (a bot's turn, a delegated subthread, a wake, a queued or relayed message): refused at once, never waited on
                let person = self.turn_guard(th).is_some_and(|g| g.person) && { let w = self.world().await; !w.bot_of.contains_key(th) && !w.parents.contains_key(th) };
                if !person { return Err(format!("Refused: {line}. A deletion this large needs the person's yes, and the person did not start this turn, so nobody is there to give it. Nothing was deleted. Tell the person what you want to remove and why; they can ask for it in a message of their own.")) }
                match self.ask_person(th, MASS_TOOL, &format!("The agent wants to delete a large part of the design in one turn: {line}. A snapshot from before this turn is kept either way. Allow it?")).await {
                    Ok(true) => { if let Some(g) = self.turn_guards.lock().unwrap().get_mut(th) { g.confirmed.push(doc.to_string()) } }
                    Ok(false) => return Err(format!("Refused: {line}. The person declined this mass deletion; nothing was deleted. Say what you meant to change instead.")),
                    Err(e) => return Err(format!("Refused: {line}, and the person did not answer ({e}). Nothing was deleted.")),
                }
            }
        }
        if let Some(e) = self.turn_guards.lock().unwrap().get_mut(th).and_then(|g| g.docs.get_mut(doc)) { e.1 += del - add; e.2 += del - add }
        Ok(())
    }
    /// A `files_*` write of `paths` (project-relative, "" skipped): `write_guard` for each, resolved the way the files tools resolve them.
    pub async fn files_write_guard(self: &Arc<Self>, th: &str, paths: &[&str]) -> Result<(), String> {
        let Some(root) = crate::files::root_of(&self.hub, th).await else { return Ok(()) }; let root = root.to_string_lossy().into_owned();
        for p in paths.iter().filter(|p| !p.is_empty()) {
            let (r, rel) = kc::direct(&root, p);
            self.write_guard(th, &if rel.starts_with('/') { rel } else { format!("{r}/{rel}") }).await?;
        }
        Ok(())
    }
    /// An approval the person answers (accept or session: yes). Waits for the answer.
    async fn ask_person(&self, th: &str, tool: &str, detail: &str) -> Result<bool, String> {
        let now = now_s(); let id = hub_id("q", now, self.seq_n());
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.parked.lock().unwrap().insert(id.clone(), super::mcp::Parked { thread: th.into(), tool: tool.into(), input: Value::Null, id: Value::Null, tx });
        let pend = self.pending_ev(th, 1, "", now).await;
        self.commit_events(vec![json!({"$":"AskOpened","id":id,"thread":th,"kind":"approval","tool":tool,"detail":detail,"options":"","at":now}), pend]).await;
        match tokio::time::timeout(MASS_WAIT, rx).await {
            Ok(Ok(a)) => Ok(a == "accept" || a == "session"),
            Ok(Err(_)) => Err("the question was closed before the person answered".into()),
            // nobody answered in time: the ask closes and the call is refused
            Err(_) => {
                self.parked.lock().unwrap().remove(&id);
                let now = now_s(); let pend = self.pending_ev(th, 0, &id, now).await;
                self.commit_events(vec![json!({"$":"AskClosed","id":id,"answer":"no answer","at":now}), pend]).await;
                Err(format!("no answer within {} minutes", MASS_WAIT.as_secs() / 60))
            }
        }
    }

    /// A turn ended: its design files are compared with its snapshot. A mass loss, a board or schematic gone, or any design change a review
    /// made opens an ask to restore to before the turn. The guard is cleared.
    pub async fn turn_guard_end(self: &Arc<Self>, th: &str) {
        let Some(g) = self.turn_guards.lock().unwrap().remove(th) else { return };
        self.turn_guard_end_with(th, g).await
    }
    /// `turn_guard_end` for a guard already taken out.
    pub async fn turn_guard_end_with(self: &Arc<Self>, th: &str, g: TurnGuard) {
        if crate::copy::on() || g.top.is_empty() { return }
        let lines = design_losses(&g.top, &g.reference).await;
        let changed = if g.review { review_changes(&g.top, &g.reference).await } else { vec![] };
        if lines.is_empty() && changed.is_empty() { return }
        let mut why = vec![];
        if !lines.is_empty() { why.push(format!("This turn removed a large part of the design: {}.", lines.join("; "))) }
        if !changed.is_empty() { why.push(format!("This turn was a review but changed design files: {}.", changed.iter().take(6).cloned().collect::<Vec<_>>().join(", "))) }
        let detail = format!("{} The project as it was before this turn is snapshot {} (turn {}). Restoring makes a new commit, and this turn's state is kept as a commit too.", why.join(" "), short_of(&g.top, &g.reference).await, g.turn);
        let now = now_s(); let id = hub_id("q", now, self.seq_n());
        let pend = self.pending_ev(th, 1, "", now).await;
        self.commit_events(vec![
            json!({"$":"SettingSet","key":format!("restore.ask.{id}"),"value":format!("{}\n{}", g.top, g.turn),"at":now}),
            json!({"$":"AskOpened","id":id,"thread":th,"kind":"input","tool":RESTORE_TOOL,"detail":detail,"options":format!("{RESTORE_YES}\n{RESTORE_NO}"),"at":now}), pend]).await;
    }
    /// Git is missing: an Install Git ask in the thread (once while it is open).
    async fn git_missing(&self, th: &str) {
        if self.world().await.asks.values().any(|a| a.thread == th && a.tool == GIT_TOOL) { return }
        let now = now_s(); let id = hub_id("q", now, self.seq_n());
        let pend = self.pending_ev(th, 1, "", now).await;
        self.commit_events(vec![json!({"$":"AskOpened","id":id,"thread":th,"kind":"input","tool":GIT_TOOL,"detail":format!("{GIT_MISSING} {}", crate::present::onboard::GIT_WHY),"options":format!("{GIT_YES}\nNot now"),"at":now}), pend]).await;
    }
    /// Git runs now: this thread's open Install Git ask closes (else it stays "Needs you", and its phone alert open, after Git is installed).
    async fn close_git_asks(&self, th: &str) {
        let open: Vec<String> = { let w = self.world().await; w.asks.iter().filter(|(_, a)| a.thread == th && a.tool == GIT_TOOL).map(|(i, _)| i.clone()).collect() };
        for id in open { let now = now_s(); let pend = self.pending_ev(th, 0, &id, now).await; self.commit_events(vec![json!({"$":"AskClosed","id":id,"answer":"Git is installed","at":now}), pend]).await }
    }
    /// The person answered the Install Git ask.
    pub async fn git_answer(self: &Arc<Self>, ask: &str, th: &str, answer: &str) -> Result<(), String> {
        let now = now_s(); let pend = self.pending_ev(th, 0, ask, now).await;
        self.commit_events(vec![json!({"$":"AskClosed","id":ask,"answer":answer,"at":now}), pend]).await;
        if answer != GIT_YES { return Ok(()) }
        match crate::settings::onboard::install_tool(&self.hub, "git").await {
            Ok(_) => self.guard_say(th, "info", "Installing Git. Setup's Design tools step shows how it goes; send your message again once it is done.".into()).await,
            Err(e) => self.guard_say(th, "error", format!("Git did not start installing: {e}. Install it from https://git-scm.com/downloads, then send your message again.")).await,
        }
        Ok(())
    }
    /// A new turn starts: an earlier turn's restore ask that is still open closes as kept (its snapshot stays; `/restore <turn>` still works).
    async fn close_restore_asks(&self, th: &str) {
        let open: Vec<String> = { let w = self.world().await; w.asks.iter().filter(|(_, a)| a.thread == th && a.tool == RESTORE_TOOL).map(|(i, _)| i.clone()).collect() };
        for id in open { let now = now_s(); let pend = self.pending_ev(th, 0, &id, now).await; self.commit_events(vec![json!({"$":"AskClosed","id":id,"answer":RESTORE_NO,"at":now}), pend]).await }
    }
    /// The person answered a restore ask.
    pub async fn restore_answer(self: &Arc<Self>, ask: &str, th: &str, answer: &str) -> Result<(), String> {
        let now = now_s();
        let at = self.world().await.setting(&format!("restore.ask.{ask}")).to_string();
        let pend = self.pending_ev(th, 0, ask, now).await;
        self.commit_events(vec![json!({"$":"AskClosed","id":ask,"answer":answer,"at":now}), pend]).await;
        if answer != RESTORE_YES { self.guard_say(th, "info", "Kept this turn's changes. The snapshot from before it stays: /restore brings it back.".into()).await; return Ok(()) }
        let (top, turn) = at.split_once('\n').map(|(a, b)| (a.to_string(), b.parse().unwrap_or(0))).unwrap_or_default();
        self.restore_turn(th, &top, turn).await.map(|_| ())
    }
    /// `/restore [turn]`: the thread's project back to before that turn (0: its last), said in the thread.
    pub async fn restore_command(self: &Arc<Self>, th: &str, turn: u64) -> Result<(), String> {
        let cwd = self.world().await.cwd(th);
        let Some(top) = sn::top_of(&kc::design_root(&cwd)).await else { self.guard_say(th, "warn", "Nothing to restore: this project is not a git repository yet (its next turn makes it one).".into()).await; return Ok(()) };
        let turn = if turn == 0 { sn::snaps(&top, th).await.last().map_or(0, |x| x.0) } else { turn };
        if turn == 0 { self.guard_say(th, "warn", "Nothing to restore: this thread has no snapshot yet.".into()).await; return Ok(()) }
        self.restore_turn(th, &top, turn).await.map(|_| ())
    }
    /// `thread.restore {thread, turn, commit}`: a user message's Restore to before this. `turn` 0 (a snapshot line from before it named its turn):
    /// the thread's snapshot whose commit is `commit`.
    pub async fn restore_pressed(self: &Arc<Self>, th: &str, turn: u64, commit: &str) -> Result<(), String> {
        if turn > 0 { return self.restore_command(th, turn).await }
        let cwd = self.world().await.cwd(th);
        let top = sn::top_of(&kc::design_root(&cwd)).await.ok_or("this project is not a git repository")?;
        let c = commit.trim();
        let n = if c.len() >= 4 && c.chars().all(|x| x.is_ascii_hexdigit()) { sn::snaps(&top, th).await.into_iter().filter(|(_, full)| full.starts_with(c)).map(|x| x.0).last() } else { None };
        match n { Some(n) => self.restore_command(th, n).await, None => Err(format!("no snapshot {c} of this thread")) }
    }
    async fn restore_turn(self: &Arc<Self>, th: &str, top: &str, turn: u64) -> Result<sn::Restored, String> {
        if self.world().await.threads.get(th).is_some_and(|t| t.turn == "running") { let m = "Not restored: the thread's turn is still running. Stop it first, then restore.".to_string(); self.guard_say(th, "warn", m.clone()).await; return Err(m) }
        let r = sn::restore(top, th, &sn::snap_ref(th, turn), &format!("{th} to before turn {turn}"), self.off_branch().await).await;
        match &r {
            Ok(x) => self.guard_say(th, "info", format!("Restored the project to before turn {turn} (snapshot {}) as {}. The state before the restore is kept as {}.{}", x.to, x.commit, x.saved, if x.removed > 0 { format!(" Removed {} file{} added since.", x.removed, if x.removed == 1 { "" } else { "s" }) } else { String::new() })).await,
            Err(e) => self.guard_say(th, "error", format!("Not restored: {e}")).await,
        }
        r
    }
}

async fn short_of(top: &str, r: &str) -> String { crate::files::git::run_ro(top, &["rev-parse", "--short", r]).await.out.trim().to_string() }
/// The boards and schematics a snapshot has that lost a mass of their items on disk since (or are gone).
pub async fn design_losses(top: &str, reference: &str) -> Vec<String> {
    use crate::files::git;
    let r = git::run_ro(top, &["ls-tree", "-r", "-z", "--name-only", reference]).await;
    let docs: Vec<String> = r.out.split('\0').filter(|p| kc::is_document(p) && kc::is_design(p)).map(str::to_string).collect();
    let gd = git::run_ro(top, &["rev-parse", "--absolute-git-dir"]).await.out.trim().to_string();
    let mut out = vec![];
    for d in docs {
        let Some(old) = git::show(&gd, &format!("{reference}:{d}")).await else { continue };
        let sch = d.ends_with(".kicad_sch");
        let before = sn::count_items(&String::from_utf8_lossy(&old), sch);
        let now = std::fs::read_to_string(std::path::Path::new(top).join(&d)).ok();
        match now {
            None if before > 0 => out.push(format!("{d} was deleted")),
            Some(t) => { if let Some(l) = sn::mass_loss(&d, before, sn::count_items(&t, sch)) { out.push(l) } }
            None => {}
        }
    }
    out
}
/// The design files that differ on disk from a snapshot (repo-relative, added and removed ones too): what a review must not have changed.
pub async fn review_changes(top: &str, reference: &str) -> Vec<String> {
    let Ok(now) = sn::tree_now(top, "review").await else { return vec![] };
    let r = crate::files::git::run_ro(top, &["diff", "--name-only", "-z", "--no-renames", reference, &now, "--"]).await;
    r.out.split('\0').filter(|p| !p.is_empty() && kc::is_design(p)).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn review_mode_starts_with_a_review_request_and_ends_with_a_change_request() {
        assert!(review_next(false, "review my project and make a task list"));
        assert!(review_next(true, "thanks, and the schematic?"), "a follow-up stays a review");
        assert!(!review_next(true, "ok, go ahead and fix the first three"), "a change request ends it");
        assert!(!review_next(false, "route the USB pair"));
    }
    #[test] fn a_review_may_read_check_and_export_but_not_write() {
        for ty in ["kiapi.common.commands.GetItems", "kiapi.common.commands.GetOpenDocuments", "kiapi.common.commands.OpenDocument", "kiapi.common.commands.SetVisibleLayers", "kiapi.board.commands.RunChecks", "kiapi.common.commands.ExportGerbers"] { assert!(review_allows(ty), "{ty}") }
        for ty in ["kiapi.common.commands.DeleteItems", "kiapi.common.commands.CreateItems", "kiapi.common.commands.UpdateItems", "kiapi.common.commands.SaveDocument", "kiapi.common.commands.BeginCommit", "kiapi.common.commands.RunAction", "kiapi.board.commands.RefillZones", "kiapi.common.commands.SetNetClasses"] { assert!(!review_allows(ty), "{ty}") }
    }
    #[test] fn restore_commands() {
        assert_eq!(restore_command("/restore"), Some(0)); assert_eq!(restore_command(" /restore 3 "), Some(3));
        assert_eq!(restore_command("/restored"), None); assert_eq!(restore_command("please /restore"), None); assert_eq!(restore_command("/restore x"), None);
    }
}
