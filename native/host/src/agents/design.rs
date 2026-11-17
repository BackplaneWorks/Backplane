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
    /// The gate for an agent's own tool call (`kicad_ipc`, `kicad_edit`): a write to a document nobody holds takes its lock for this
    /// thread on the spot, as `lock_acquire` would (released when its turn ends), instead of an error that only says to call it.
    /// A document another thread holds is refused as before: nothing blocks, nobody is queued without asking.
    pub async fn kicad_gate_take(self: &Arc<Self>, thread: &str, path: &str, ty: &str) -> Result<String, String> {
        match self.kicad_gate(thread, path, ty).await {
            Err(why) if why.starts_with("writes to") => {
                let name = kc::doc_lock(&kc::doc_of(&kc::route(path)));
                let (ok, r) = self.tool_lock_acquire(thread, &json!({"name": name, "note": "editing"})).await;
                if ok && r["held"] == true { self.kicad_gate(thread, path, ty).await } else { Err(why) }
            }
            r => r,
        }
    }
    /// One command of an agent's for the project's KiCad: the approval refusals, the document's gate, the command, and a save's commit
    /// to main. Ok is `kicad_ipc`'s answer ({type, message, ...}).
    pub async fn kicad_call(self: &Arc<Self>, thread: &str, path: &str, ty: &str, msg: Value) -> Result<Value, String> {
        use crate::files::kicad::ipc;
        if let Some(why) = ipc::approval_refused(ty, &msg) { return Err(why) }
        // #1163, before the gate and any save: a review refuses writes, the turn's first write is snapshotted, a mass deletion waits for the person (snapguard.rs)
        self.mass_guard(thread, &kc::doc_of(&kc::route(path)), ty, &msg).await?;
        let routed = self.kicad_gate_take(thread, path, ty).await?;
        let v = ipc::kicad_ipc_in(&self.hub, Some(thread), &routed, ty, msg).await?;
        // a saved document is committed straight to main
        if ty.rsplit('.').next() == Some("SaveDocument") && v["not_saved"].is_null() { self.design_record_note(thread, "save", v["format_upgrade"].as_str().unwrap_or("")).await }
        Ok(v)
    }
    /// The MCP tool `kicad_ipc`: a short or misplaced command name is completed (`GetItems`, `kiapi.board.commands.CreateItems`), `$doc` and
    /// `$project` in the message stand for the open document and its project, and a command that finds its document closed opens it and
    /// runs again, so none of the three costs the agent a request.
    pub async fn tool_kicad_ipc(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        use crate::files::kicad::{embed, ipc, kedit};
        let path = a["path"].as_str().unwrap_or(""); let ty = kedit::full_type(a["type"].as_str().unwrap_or(""));
        let short = ty.rsplit('.').next().unwrap_or("").to_string();
        let pro = ipc::pro_of(path); let key = format!("{pro}|{thread}");
        let (ok, mut v) = self.tool_kicad_ipc_inner(thread, a).await;
        if !ok { return (ok, v) }
        // an agent's change in the person's open KiCad is saved by the hub when it lands (a commit, or an item write outside one), as
        // kicad_edit does: a crash never loses it (KiCad marks no document modified for an API commit, so it would neither autosave it nor
        // ask to save it). `save: false` keeps it unsaved for a batch of commits: the hub then counts it as unsaved (close, quit, the
        // closed-unexpectedly words) until a save
        if short == "BeginCommit" { open_commits().lock().unwrap().insert(key.clone()); }
        let in_commit = open_commits().lock().unwrap().contains(&key);
        if short == "EndCommit" { open_commits().lock().unwrap().remove(&key); }
        let lands = (short == "EndCommit" && !a["message"].to_string().contains("CMA_DROP")) || (ipc::ITEM_WRITES.contains(&short.as_str()) && !in_commit);
        if lands && embed::gui_route(&pro).is_some() && v.is_object() {
            if a["save"] == json!(false) { v["saved"] = json!(false); v["unsaved"] = json!("Not saved, as asked (save: false): the person's KiCad shows it, the file does not have it yet. Save it (SaveDocument, or a commit without save: false) before you end your turn."); return (true, v) }
            let kind = if path.ends_with(".kicad_sch") { "DOCTYPE_SCHEMATIC" } else { "DOCTYPE_PCB" };
            let persons = embed::unsaved_update(&pro, |u| u).person;
            match self.kicad_open(thread, path, kind).await {
                Ok(doc) => match self.kicad_call(thread, path, "kiapi.common.commands.SaveDocument", json!({"document": doc})).await {
                    Ok(s) => { v["saved"] = json!(ipc::doc_file(path, &pro)); if s["not_saved"].is_string() { v["save_note"] = s["not_saved"].clone() }
                        if persons { v["save_note"] = json!("The save also wrote the person's own unsaved edits in this document, as kicad_edit's does: tell them it is saved.") } }
                    Err(e) => { v["saved"] = json!(false); v["unsaved"] = json!(format!("The hub could not save it: {e}. It is in the person's KiCad but not on disk: say so.")) }
                },
                Err(e) => { v["saved"] = json!(false); v["unsaved"] = json!(format!("The hub could not save it: {e}")) }
            }
        }
        (true, v)
    }
    async fn tool_kicad_ipc_inner(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        use crate::files::kicad::kedit;
        let path = a["path"].as_str().unwrap_or(""); let ty = kedit::full_type(a["type"].as_str().unwrap_or(""));
        let mut msg = a.get("message").cloned().unwrap_or(json!({}));
        let doc_kind = if path.ends_with(".kicad_pcb") { "DOCTYPE_PCB" } else if path.ends_with(".kicad_sch") { "DOCTYPE_SCHEMATIC" } else { "" };
        let short = ty.rsplit('.').next().unwrap_or("").to_string();
        if !doc_kind.is_empty() && msg.to_string().contains("\"$") {
            let gui = crate::files::kicad::embed::gui_route(&crate::files::kicad::ipc::pro_of(path)).is_some();
            let doc = match self.kicad_call(thread, path, "kiapi.common.commands.GetOpenDocuments", json!({"type": doc_kind})).await { Ok(v) => v["message"]["documents"].get(0).cloned(), Err(_) => None };
            match doc { Some(d) => kedit::fill(&mut msg, &d),
                None if gui => return fail(&format!("{path} is not open in the person's KiCad, which has this project")),
                None => match self.kicad_open(thread, path, doc_kind).await { Ok(d) => kedit::fill(&mut msg, &d), Err(e) => return fail(&e) } }
        }
        match self.kicad_call(thread, path, &ty, msg.clone()).await {
            Ok(v) => (true, v),
            // its document is not open in this KiCad yet: open it and ask again
            // (never in the person's own KiCad: what it has open is theirs to decide, and no frame is shown or raised for an agent)
            // a command about a project sent without one (GetNetClasses {}): the document's own project, and asked again
            Err(e) if (e.contains("a project name and path must be specified") || e.contains("the requested project  is not open")) && !doc_kind.is_empty() && crate::files::kicad::embed::gui_route(&crate::files::kicad::ipc::pro_of(path)).is_none() => {
                match self.kicad_open(thread, path, doc_kind).await { Ok(d) => { let mut m = msg; if !m.is_object() { m = json!({}) } m["project"] = d["project"].clone();
                        match self.kicad_call(thread, path, &ty, m).await { Ok(v) => (true, v), Err(e) => fail(&e) } }
                    Err(_) => fail(&e) }
            }
            Err(e) if e.contains("no handler available for request") && !doc_kind.is_empty() && short != "OpenDocument"
                && crate::files::kicad::embed::gui_route(&crate::files::kicad::ipc::pro_of(path)).is_none() => {
                if let Err(e2) = self.kicad_open(thread, path, doc_kind).await { return fail(&format!("{e}; opening {path} failed: {e2}")) }
                match self.kicad_call(thread, path, &ty, msg).await { Ok(v) => (true, v), Err(e) => fail(&e) }
            }
            Err(e) => fail(&e),
        }
    }
    /// The open document of `path`'s kind (its specifier), opened if it is not.
    async fn kicad_open(self: &Arc<Self>, thread: &str, path: &str, kind: &str) -> Result<Value, String> {
        if let Ok(v) = self.kicad_call(thread, path, "kiapi.common.commands.GetOpenDocuments", json!({"type": kind})).await { if let Some(d) = v["message"]["documents"].get(0) { return Ok(d.clone()) } }
        Ok(self.kicad_call(thread, path, "kiapi.common.commands.OpenDocument", json!({"type": kind, "path": path})).await?["message"]["document"].clone())
    }
    /// The MCP tool `kicad_edit` (kicad/kedit.rs): a whole edit in one call, each of its commands through [`Self::kicad_call`].
    pub async fn tool_kicad_edit(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        use crate::files::kicad::kedit;
        struct Via<'a> { ag: &'a Arc<Agents>, thread: &'a str, path: &'a str, person: std::sync::Mutex<Value>, caps: Vec<String> }
        impl kedit::Call for Via<'_> {
            fn call<'b>(&'b self, ty: &'b str, msg: Value) -> kedit::Fut<'b> { Box::pin(async move {
                // the person's KiCad has the project: a document it has not opened is theirs to open (no window is opened or raised for an agent)
                if ty.ends_with(".OpenDocument") && crate::files::kicad::embed::gui_route(&crate::files::kicad::ipc::pro_of(self.path)).is_some() {
                    return Err(format!("{} is not open in the person's KiCad, which has this project: ask them to open it there (Backplane opens no window for an agent)", self.path))
                }
                let v = self.ag.kicad_call(self.thread, self.path, ty, msg).await?;
                if !v["person"].is_null() { *self.person.lock().unwrap() = v["person"].clone() }
                let mut m = v["message"].clone();
                for k in ["not_saved", "format_upgrade", "kicad_load_changes"] { if v[k].is_string() { if !m.is_object() { m = json!({}) } m[k] = v[k].clone() } }
                Ok(m)
            }) }
            fn caps(&self) -> Vec<String> { self.caps.clone() }
            fn sheet_texts(&self) -> Option<Vec<String>> {
                if !self.path.ends_with(".kicad_sch") { return None }
                fn walk(d: &std::path::Path, depth: u32, out: &mut Vec<String>) {
                    for e in std::fs::read_dir(d).into_iter().flatten().flatten() { let p = e.path();
                        if p.is_dir() { if depth < 3 && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.') || n.to_string_lossy().ends_with("-backups")) { walk(&p, depth + 1, out) } }
                        else if p.extension().is_some_and(|x| x == "kicad_sch") { if let Ok(t) = std::fs::read_to_string(&p) { out.push(t) } } }
                }
                let mut out = vec![]; walk(std::path::Path::new(&kc::route(self.path)).parent()?, 0, &mut out); Some(out)
            }
            fn nets<'b>(&'b self) -> kedit::Fut<'b> { Box::pin(async move { Ok(crate::files::kicad::checkrun::netlist_of(&self.ag.hub, &kc::route(self.path)).await.unwrap_or(Value::Null)) }) }
            fn check<'b>(&'b self) -> kedit::Fut<'b> { Box::pin(async move { Ok(crate::files::kicad::checkrun::report_of(&self.ag.hub, &kc::route(self.path)).await.unwrap_or(Value::Null)) }) }
        }
        let path = a["path"].as_str().unwrap_or("");
        if !crate::plat::fs::is_abs(path) { return fail("path must be the absolute path of the board (.kicad_pcb) or schematic (.kicad_sch) to edit") }
        // what an agent may never send is refused before anything is opened or locked
        for op in a["ops"].as_array().into_iter().flatten() {
            if let Some(why) = crate::files::kicad::ipc::approval_refused(&kedit::full_type(op["ipc"]["type"].as_str().unwrap_or("")), &op["ipc"]["message"]) { return fail(&why) }
        }
        // the person is at work in this KiCad: a change waits for the agent to have seen what they are doing, as after a kicad_ipc read's `person`
        if kedit::writes(&a["ops"]) && a["person_seen"].as_bool() != Some(true) {
            if let Some(p) = crate::files::kicad::coedit::person_now(&crate::files::kicad::ipc::pro_of(path)) { return (false, json!({"error": kedit::person_first(&p), "person": p})) }
        }
        let via = Via { ag: self, thread, path, person: std::sync::Mutex::new(Value::Null), caps: crate::files::kicad::ipc::runtime(&self.hub).await.caps };
        let r = kedit::run(&via, path, a).await;
        if kedit::writes(&a["ops"]) { crate::files::kicad::coedit::agent_wrote(&crate::files::kicad::ipc::pro_of(path)) }
        match r {
            Ok(mut v) => {
                let p = via.person.lock().unwrap().clone(); if !p.is_null() { v["person"] = p }
                // a saved edit is shown: the person's viewer turns to the document, as `viewer_show` after the save would have it (show: false leaves it)
                if v["saved"].as_str() == Some(path) && a["show"].as_bool().unwrap_or(true) { if self.viewer_show(thread, path, "", false).await.0 { v["shown"] = json!(true) } }
                (true, v)
            }
            Err(e) => fail(&e),
        }
    }
    /// Whether the thread's project has a board or schematic (its canonical design files).
    pub async fn has_design(&self, thread: &str) -> bool {
        let cwd = self.world().await.cwd(thread); if cwd.is_empty() { return false }
        tokio::task::spawn_blocking(move || { let f = crate::files::kicad::project::read(&kc::design_root(&cwd)); !f.pcb.is_empty() || !f.sch.is_empty() }).await.unwrap_or(false)
    }
    /// The MCP tool `viewer_show`: the person's viewer shows `path` (a canonical board named by its worktree twin shows main's live copy).
    /// `wait`: the answer carries what the viewer then shows (kind, source, part, sheet, layer, selection) as the client reports it; a client
    /// that does not answer in 2 s leaves the last report, marked. Without it the viewer is told and nothing is waited for.
    pub async fn viewer_show(self: &Arc<Self>, thread: &str, path: &str, kind: &str, wait: bool) -> (bool, Value) {
        let shown = kc::route(path);
        let cwd = self.world().await.cwd(thread); let roots = [kc::design_root(&cwd), cwd];
        if let Some(why) = crate::present::viewer::show_refusal(kind, &shown, &roots) { return fail(&why) }
        let before = crate::files::kicad::watch::viewer_state(thread).0;
        self.hub.push_all(&json!({"t":"view","thread":thread,"path":shown,"kind":kind}));
        // (the owner's windows on other machines show it too)
        { let (hub, ann) = (self.hub.clone(), json!({"thread":thread,"path":shown.clone(),"kind":kind})); tokio::spawn(async move { hub.far_announce("viewer.show", &ann).await }); }
        if !wait { return (true, json!({"shown":shown})) }
        let mut now = crate::files::kicad::watch::viewer_state(thread);
        for _ in 0..40 { if now.0 > before { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await; now = crate::files::kicad::watch::viewer_state(thread); }
        (true, json!({"shown":shown,"viewer":now.1,"settled":now.0 > before}))
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
    pub async fn design_record(self: &Arc<Self>, th: &str, why: &str) { self.design_record_note(th, why, "").await }
    /// The same, `note` a line of the commit's body (a save that also upgraded the file's format says so: savegate.rs).
    pub async fn design_record_note(self: &Arc<Self>, th: &str, why: &str, note: &str) {
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
        let msg = format!("design: {} ({th}){}", if title.is_empty() || title == "New thread" { "KiCad work".to_string() } else { title.chars().take(72).collect() }, if note.is_empty() { String::new() } else { format!("\n\n{note}") });
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
        self.turn_guard_end(th).await; // #1163: a mass loss or a review's change offers to restore to before the turn (snapguard.rs)
        self.design_record(th, "turn").await;
        self.doc_locks_free(th).await;
        // the open co-editing editors stop showing this agent at work (docs/kicad-coedit.md)
        crate::files::kicad::coedit::forget(th);
        for route in crate::files::kicad::embed::coedit_routes() {
            let _ = crate::files::kicad::embed::gui_call_as(&route, "kiapi.backplane.coedit.SetAgentActivity", crate::files::kicad::coedit::activity(th, "", "idle"), &crate::files::kicad::coedit::client_name(&crate::files::kicad::coedit::Who::Agent(th))).await;
        }
    }
    /// The thread's title ("" for none): the task an open co-editing KiCad shows for it.
    pub async fn thread_title(&self, th: &str) -> String { self.world().await.threads.get(th).map(|t| t.title.clone()).unwrap_or_default() }
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

/// The agents' open commits in the person's KiCad ("<project>|<thread>"): an item write inside one lands with its EndCommit.
fn open_commits() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static S: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}
