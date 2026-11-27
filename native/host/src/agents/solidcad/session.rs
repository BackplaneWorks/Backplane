//! One project's CAD session: the `solidcraft-cli mcp` child that serves it (bridged to the SolidCraft the pane runs when there is one,
//! so the person watches the edit; headless otherwise), the design file it works on, its checkpoints on disk, and what happens when a
//! call stalls or the child dies: the child is replaced, the design comes back from its last save, and the agent is told exactly what
//! was kept and what to check, instead of a tool call that never answers.
use super::{client::{self, CallErr, Client, Mode, Spawn}, harness::{self as h, Class}};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Mutex as StdMutex, time::Duration};

const START: Duration = Duration::from_secs(30);
const PROBE: Duration = Duration::from_secs(30);

/// Where the program and the pane's control channel come from (the runtime lane's functions; a test's stand-ins).
pub struct Seam {
    pub cli: Box<dyn Fn() -> Option<PathBuf> + Send + Sync>,
    /// (port, token) of the SolidCraft open on this project root, None when no pane runs
    pub control: Box<dyn Fn(&str) -> Option<(u16, String)> + Send + Sync>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Checkpoint { pub id: u64, pub at: u64, pub file: String, pub note: String, pub bytes: u64, pub hash: String }

#[derive(Default)]
struct Inner {
    client: Option<Client>,
    /// the design's own file (`.sc`), once it has one: what a restart reopens and what checkpoints copy
    design: Option<String>,
    /// what was opened last (a STEP has no design file until it is saved as one)
    source: Option<String>,
    edits_since_look: u32,
    repeats: h::Repeats,
    restarts: u32,
}

pub struct Session {
    pub root: PathBuf,
    /// `<home>/solidcad/<digest>`: checkpoints, never inside the project
    dir: PathBuf,
    inner: tokio::sync::Mutex<Inner>,
    /// a changing call whose caller went away before the answer (the turn was interrupted): said once, to the next caller
    interrupted: StdMutex<Option<String>>,
    /// a test's one budget for every call (None: harness::budget)
    pub fixed_budget: Option<Duration>,
}

/// Set when a changing call starts, cleared when it answers; left set when the caller is dropped mid-call.
struct InFlight<'a> { slot: &'a StdMutex<Option<String>>, armed: bool }
impl<'a> InFlight<'a> {
    fn new(slot: &'a StdMutex<Option<String>>, tool: &str, armed: bool) -> Self { if armed { *slot.lock().unwrap() = Some(tool.to_string()) } InFlight { slot, armed } }
    fn done(mut self) { if self.armed { *self.slot.lock().unwrap() = None } self.armed = false }
}

fn sha(bytes: &[u8]) -> String { use sha2::Digest; hex::encode(&sha2::Sha256::digest(bytes)[..8]) }
fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
fn fail(v: Value) -> (bool, Value) { (false, v) }
/// `v` with what the harness has to say beside it (an interrupted earlier call, a session that changed).
fn noted(mut v: Value, notes: &[String]) -> Value { if !notes.is_empty() { v["earlier"] = json!(notes) } v }

impl Session {
    pub fn new(root: PathBuf, dir: PathBuf) -> Session { Session { root, dir, inner: Default::default(), interrupted: Default::default(), fixed_budget: None } }
    fn budget(&self, tool: &str, class: Class) -> Duration { self.fixed_budget.unwrap_or_else(|| h::budget(tool, class)) }

    fn index_path(&self) -> PathBuf { self.dir.join("checkpoints.json") }
    pub fn checkpoints(&self) -> Vec<Checkpoint> { std::fs::read(self.index_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default() }
    fn ck_file(&self, id: u64) -> PathBuf { self.dir.join(format!("{id}.sc")) }
    fn put_index(&self, v: &[Checkpoint]) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        crate::agents::bundled::put(&self.index_path(), &serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }
    /// Copies the design's file as it is on disk into the session's folder. None: nothing new to keep (no file yet, or the same bytes as
    /// the newest checkpoint of that file).
    pub fn checkpoint_file(&self, file: &str, note: &str, force: bool) -> Result<Option<Checkpoint>, String> {
        if crate::copy::on() { return Ok(None) }
        let Ok(bytes) = std::fs::read(file) else { return Ok(None) };
        let hash = sha(&bytes); let mut all = self.checkpoints();
        if !force && all.iter().rev().find(|c| c.file == file).map_or(false, |c| c.hash == hash) { return Ok(None) }
        let id = all.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        std::fs::write(self.ck_file(id), &bytes).map_err(|e| format!("the checkpoint could not be written: {e}"))?;
        let ck = Checkpoint { id, at: now(), file: file.to_string(), note: note.chars().take(200).collect(), bytes: bytes.len() as u64, hash };
        all.push(ck.clone());
        let ids: Vec<u64> = all.iter().map(|c| c.id).collect();
        for gone in h::prune(&ids) { let _ = std::fs::remove_file(self.ck_file(gone)); all.retain(|c| c.id != gone); }
        self.put_index(&all)?;
        Ok(Some(ck))
    }
    /// Puts checkpoint `id` (the newest when None) back as its design file, keeping what is there now as a checkpoint of its own.
    fn restore_file(&self, id: Option<u64>) -> Result<(Checkpoint, Option<Checkpoint>), String> {
        if crate::copy::on() { return Err(crate::copy::HOME_ONLY.into()) }
        let all = self.checkpoints();
        let ck = match id { Some(i) => all.iter().find(|c| c.id == i), None => all.last() }.cloned().ok_or_else(|| match id { Some(i) => format!("no checkpoint {i} (solidcraft_checkpoint {{action:\"list\"}} shows them)"), None => "there is no checkpoint yet".to_string() })?;
        let bytes = std::fs::read(self.ck_file(ck.id)).map_err(|e| format!("checkpoint {} is gone: {e}", ck.id))?;
        let kept = self.checkpoint_file(&ck.file, &format!("before restoring checkpoint {}", ck.id), false)?;
        crate::agents::bundled::put(Path::new(&ck.file), &bytes).map_err(|e| format!("{} could not be written: {e}", ck.file))?;
        Ok((ck, kept))
    }

    fn want(&self, seam: &Seam) -> Mode { match (seam.control)(&self.root.to_string_lossy()) { Some((port, token)) => Mode::Bridged { port, token }, None => Mode::Headless } }

    /// A live child in the mode this moment calls for. A new headless one reopens the design. Says what it did in `notes`.
    async fn ensure(&self, seam: &Seam, inner: &mut Inner, notes: &mut Vec<String>) -> Result<(), String> {
        let want = self.want(seam);
        if let Some(c) = &inner.client { if c.alive() && c.mode == want { return Ok(()) } }
        let had = inner.client.take();
        let Some(program) = (seam.cli)() else { return Err("SolidCraft is not installed on this machine yet: open a CAD file in Backplane's viewer (viewer_show) to install it, or put solidcraft-cli on PATH.".into()) };
        let _ = std::fs::create_dir_all(&self.dir);
        let spawn = Spawn { program, mode: want.clone(), allow: vec![self.root.clone(), self.dir.clone()] };
        let c = Client::start(&spawn, START).await?;
        if let Some(old) = had { if old.mode != want { notes.push(match &want {
            Mode::Bridged { .. } => "SolidCraft is now open on this project: these tools act on the design in its window, and the person sees each change.".to_string(),
            Mode::Headless => "SolidCraft's window was closed: the session is headless again, on the design's last saved file.".to_string() }) } }
        if want == Mode::Headless {
            if let Some(p) = inner.design.clone().or_else(|| inner.source.clone()).filter(|p| Path::new(p).is_file()) {
                match c.call("open", &json!({"path": p}), self.budget("open", Class::Replace)).await {
                    Ok(r) if !client::is_error(&r) => notes.push(format!("The session was started on {p}.")),
                    Ok(r) => notes.push(format!("The session was started, but {p} could not be reopened: {}", client::first_text(&r))),
                    Err(e) => notes.push(format!("The session was started, but {p} could not be reopened: {}", client::say(&e))),
                }
            }
        }
        inner.client = Some(c);
        Ok(())
    }

    /// The upstream tool list (a child is started for it when none runs).
    pub async fn tools(&self, seam: &Seam) -> Result<Vec<Value>, String> {
        let mut guard_ = self.inner.lock().await; let inner = &mut *guard_; let mut notes = vec![];
        self.ensure(seam, inner, &mut notes).await?;
        inner.client.as_ref().unwrap().tools(PROBE).await.map_err(|e| client::say(&e))
    }

    async fn probe(c: &Client) -> (Option<Value>, Option<Value>) {
        let i = c.call("inspect_design", &json!({"measure":false,"sketches":false,"timeline":"summary","bodies":"names","joints":false}), PROBE).await.ok().filter(|r| !client::is_error(r)).and_then(|r| client::first_json(&r));
        let m = c.call("measure", &json!({"summary":true}), PROBE).await.ok().filter(|r| !client::is_error(r)).and_then(|r| client::first_json(&r));
        (i, m)
    }

    /// A stalled or dead child is replaced; what the agent is told of it.
    async fn recover(&self, seam: &Seam, inner: &mut Inner, tool: &str, class: Class, stalled: bool, detail: &str) -> Value {
        let old = inner.client.take(); inner.restarts += 1;
        let mode = old.as_ref().map(|c| c.mode.clone()).unwrap_or(Mode::Headless);
        let mut rec = serde_json::Map::new();
        if let Some(c) = &old {
            if stalled { if let Mode::Bridged { .. } = &mode {
                // the app goes on with the request: ask it to stop, through a server of our own (the stalled one answers nothing)
                if let Some(program) = (seam.cli)() {
                    if let Ok(side) = Client::start(&Spawn { program, mode: mode.clone(), allow: vec![self.root.clone()] }, START).await {
                        let said = side.call("cancel", &json!({}), self.budget("cancel", Class::Control)).await.map(|r| client::first_text(&r)).unwrap_or_else(|e| client::say(&e));
                        rec.insert("cancel".into(), json!(said)); side.kill();
                    }
                }
            } }
            c.kill();
        }
        drop(old);
        let mut notes = vec![];
        let started = self.ensure(seam, inner, &mut notes).await;
        rec.insert("session".into(), json!(match &started { Ok(()) => "restarted".to_string(), Err(e) => format!("could not be restarted: {e}") }));
        let changing = matches!(class, Class::Edit | Class::Replace | Class::Write);
        let design = match (&mode, &inner.design) {
            (Mode::Bridged { .. }, _) => "The design is the one in SolidCraft's window: the call may or may not have been applied there.".to_string(),
            (Mode::Headless, Some(p)) => format!("The design was reopened from its last save ({p}): {}.", if changing { "the change this call was making is not in it" } else { "nothing was lost" }),
            (Mode::Headless, None) => match &inner.source { Some(p) => format!("The design had never been saved as a .sc file: it was reopened from {p}, and the edits made since are lost."), None => "The design had never been saved: the session starts empty.".to_string() },
        };
        rec.insert("design".into(), json!(design));
        if !notes.is_empty() { rec.insert("notes".into(), json!(notes)); }
        let what = if stalled { format!("{}{tool} did not answer within {} s: the kernel stalled on it and the session was replaced", h::PREFIX, self.budget(tool, class).as_secs()) } else { format!("SolidCraft's MCP server ended during {}{tool} ({detail}) and was replaced", h::PREFIX) };
        json!({"error": what, "recovery": Value::Object(rec),
            "next": format!("Call {p}summary to see the design as it is now before sending any change again. If the same call stalls twice, it is the geometry: make the step smaller (fewer edges in a fillet, a simpler profile, a coarser export quality) or take another route.", p = h::PREFIX)})
    }

    /// One upstream tool call, with everything the harness does around it.
    pub async fn call(&self, seam: &Seam, tool: &str, args: &Value, guard: &(dyn Fn(Vec<String>) -> crate::api::Fut<'static, Result<(), String>> + Send + Sync)) -> (bool, Value) {
        let args = h::resolve_paths(&self.root, tool, args); let class = h::class(tool, &args);
        if crate::copy::on() && matches!(class, Class::Write) { return fail(json!({"error": crate::copy::HOME_ONLY})) }
        let mut guard_ = self.inner.lock().await; let inner = &mut *guard_; let mut notes: Vec<String> = vec![];
        if let Some(prev) = self.interrupted.lock().unwrap().take() { notes.push(format!("The earlier {}{prev} call was interrupted before it answered: it may or may not have been applied. Check the design ({}summary) before relying on it.", h::PREFIX, h::PREFIX)) }
        if let Err(e) = self.ensure(seam, inner, &mut notes).await { return fail(json!({"error": e})) }
        let changing = matches!(class, Class::Edit | Class::Replace);
        // the turn's snapshot of the project files this call writes (the design's own file, saved after an edit; a save's or export's target)
        let mut touched = h::written_paths(tool, &args);
        if changing { touched.extend(inner.design.clone()) }
        touched.retain(|p| Path::new(p).starts_with(&self.root));
        if !touched.is_empty() { if let Err(why) = guard(touched).await { return fail(json!({"error": why})) } }
        let mut ck = None;
        if changing { if let Some(f) = inner.design.clone() { match self.checkpoint_file(&f, &format!("before {tool}"), false) { Ok(c) => ck = c, Err(e) => notes.push(e) } } }
        let flight = InFlight::new(&self.interrupted, tool, matches!(class, Class::Edit | Class::Replace | Class::Write));
        let got = inner.client.as_ref().unwrap().call(tool, &args, self.budget(tool, class)).await;
        flight.done();
        let res = match got {
            Ok(r) => r,
            Err(CallErr::Rpc(m)) => { let mut v = json!({"error": m}); if let Some(n) = inner.repeats.record(tool, &args, false) { v["stuck"] = json!(h::repeat_advice(tool, n)) } return fail(noted(v, &notes)) }
            Err(CallErr::Timeout) => return fail(noted(self.recover(seam, inner, tool, class, true, "").await, &notes)),
            Err(CallErr::Exited(why)) => {
                let told = self.recover(seam, inner, tool, class, false, &why).await;
                // a read loses nothing by being asked again of the new child
                let again = if matches!(class, Class::Read | Class::Look) { match &inner.client { Some(c) => c.call(tool, &args, self.budget(tool, class)).await.ok(), None => None } } else { None };
                match again { Some(r) => { notes.push("SolidCraft's MCP server had ended and was restarted before this answer.".into()); r } None => return fail(noted(told, &notes)) }
            }
        };
        let ok = !client::is_error(&res);
        let mut blocks = res.get("content").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut bp = serde_json::Map::new();
        if let Some(n) = inner.repeats.record(tool, &args, ok) { bp.insert("stuck".into(), json!(h::repeat_advice(tool, n))); }
        if ok { match class {
            Class::Look => inner.edits_since_look = 0,
            Class::Edit | Class::Replace | Class::Write => {
                let c = inner.client.as_ref().unwrap();
                let (mut i, m) = Self::probe(c).await;
                let path = i.as_ref().and_then(|i| i.get("path")).and_then(Value::as_str).map(str::to_string);
                if class == Class::Replace { inner.source = args.get("path").or_else(|| args.pointer("/params/path")).and_then(Value::as_str).map(str::to_string); inner.design = None; inner.edits_since_look = 1 }
                if path.is_some() { inner.design = path.clone() }
                if class == Class::Edit {
                    inner.edits_since_look += 1;
                    // an edit is kept at once: the file is what a restart reopens and what the viewers follow
                    let dirty = i.as_ref().and_then(|i| i.get("dirty")).and_then(Value::as_bool).unwrap_or(false);
                    if let (Some(p), true, false) = (&path, dirty, crate::copy::on()) {
                        match c.call("save", &json!({}), self.budget("save", Class::Write)).await {
                            Ok(r) if !client::is_error(&r) => { bp.insert("saved".into(), json!(p)); if let Some(o) = i.as_mut().and_then(Value::as_object_mut) { o.insert("dirty".into(), json!(false)); } }
                            Ok(r) => { bp.insert("not_saved".into(), json!(client::first_text(&r))); }
                            Err(e) => { bp.insert("not_saved".into(), json!(client::say(&e))); }
                        }
                    }
                }
                let d = h::digest(i.as_ref(), m.as_ref());
                if class != Class::Write { let hint = h::next_hint(inner.edits_since_look, &d, inner.design.is_some()); if !hint.is_empty() { bp.insert("next".into(), json!(hint)); } }
                bp.insert("design".into(), d);
            }
            _ => {}
        } }
        if let Some(c) = ck { bp.insert("checkpoint".into(), json!({"id": c.id, "of": c.file})); }
        if !notes.is_empty() { bp.insert("notes".into(), json!(notes)); }
        if !bp.is_empty() { blocks.push(json!({"type":"text","text": json!({"backplane": Value::Object(bp)}).to_string()})); }
        (ok, json!({"$content": blocks}))
    }

    /// `solidcraft_checkpoint`
    pub async fn checkpoint(&self, seam: &Seam, a: &Value) -> (bool, Value) {
        let action = a.get("action").and_then(Value::as_str).unwrap_or("list");
        let list = |s: &Session| json!(s.checkpoints().iter().rev().map(|c| json!({"id": c.id, "at": c.at, "file": c.file, "note": c.note, "bytes": c.bytes})).collect::<Vec<_>>());
        match action {
            "list" => (true, json!({"checkpoints": list(self)})),
            "save" => {
                let mut guard_ = self.inner.lock().await; let inner = &mut *guard_; let mut notes = vec![];
                if let Err(e) = self.ensure(seam, inner, &mut notes).await { return fail(json!({"error": e})) }
                let c = inner.client.as_ref().unwrap();
                let (i, _) = Self::probe(c).await;
                let Some(p) = i.as_ref().and_then(|i| i.get("path")).and_then(Value::as_str).map(str::to_string) else { return fail(json!({"error": format!("the design has no file yet: {}save {{path}} first, then checkpoint it", h::PREFIX)})) };
                if i.as_ref().and_then(|i| i.get("dirty")).and_then(Value::as_bool).unwrap_or(false) && !crate::copy::on() {
                    match c.call("save", &json!({}), self.budget("save", Class::Write)).await { Ok(r) if !client::is_error(&r) => {}, Ok(r) => return fail(json!({"error": client::first_text(&r)})), Err(e) => return fail(json!({"error": client::say(&e)})) }
                }
                inner.design = Some(p.clone());
                match self.checkpoint_file(&p, a.get("note").and_then(Value::as_str).unwrap_or("saved by the agent"), true) {
                    Ok(Some(c)) => (true, json!({"checkpoint": {"id": c.id, "file": c.file, "bytes": c.bytes}})),
                    Ok(None) => fail(json!({"error": format!("{p} could not be read")})), Err(e) => fail(json!({"error": e})) }
            }
            "restore" => {
                let mut guard_ = self.inner.lock().await; let inner = &mut *guard_;
                let (ck, kept) = match self.restore_file(a.get("id").and_then(Value::as_u64)) { Ok(x) => x, Err(e) => return fail(json!({"error": e})) };
                let mut notes = vec![];
                if let Err(e) = self.ensure(seam, inner, &mut notes).await { return fail(json!({"error": e, "restored_file": ck.file})) }
                let c = inner.client.as_ref().unwrap();
                let opened = c.call("open", &json!({"path": ck.file}), self.budget("open", Class::Replace)).await;
                let (i, m) = Self::probe(c).await;
                inner.design = Some(ck.file.clone()); inner.source = Some(ck.file.clone()); inner.edits_since_look = 1; inner.repeats = Default::default();
                let mut out = json!({"restored": {"id": ck.id, "file": ck.file, "note": ck.note}, "design": h::digest(i.as_ref(), m.as_ref())});
                if let Some(k) = kept { out["kept"] = json!({"id": k.id, "note": k.note}) }
                match opened { Ok(r) if !client::is_error(&r) => (true, out), Ok(r) => { out["error"] = json!(format!("the file was restored, but SolidCraft did not reopen it: {}", client::first_text(&r))); fail(out) } Err(e) => { out["error"] = json!(format!("the file was restored, but SolidCraft did not reopen it: {}", client::say(&e))); fail(out) } }
            }
            _ => fail(json!({"error": "action is list, save or restore"})),
        }
    }

    /// `solidcraft_summary`
    pub async fn summary(&self, seam: &Seam) -> (bool, Value) {
        let mut guard_ = self.inner.lock().await; let inner = &mut *guard_; let mut notes = vec![];
        if let Err(e) = self.ensure(seam, inner, &mut notes).await { return fail(json!({"error": e})) }
        let c = inner.client.as_ref().unwrap();
        let i = c.call("inspect_design", &json!({"measure":false,"sketches":false,"timeline":"summary","bodies":"names","joints":false}), PROBE).await;
        let m = c.call("measure", &json!({}), self.budget("measure", Class::Read)).await;
        match (i, m) {
            (Ok(i), Ok(m)) if !client::is_error(&i) && !client::is_error(&m) => {
                let (i, m) = (client::first_json(&i).unwrap_or(Value::Null), client::first_json(&m).unwrap_or(Value::Null));
                let mut s = h::summary(&i, &m);
                s["session"] = json!(c.mode.word()); s["checkpoints"] = json!(self.checkpoints().len());
                if !notes.is_empty() { s["notes"] = json!(notes) }
                (true, s)
            }
            (Ok(i), _) if client::is_error(&i) => fail(json!({"error": client::first_text(&i)})),
            (_, Ok(m)) if client::is_error(&m) => fail(json!({"error": client::first_text(&m)})),
            (Err(e), _) | (_, Err(e)) => { let stalled = e == CallErr::Timeout; fail(self.recover(seam, inner, "summary", Class::Read, stalled, &client::say(&e)).await) }
            _ => fail(json!({"error": "the design could not be read"})),
        }
    }

    /// `solidcraft_look`: the design drawn from several sides in one answer, with its digest: the render and inspect half of the loop.
    pub async fn look(&self, seam: &Seam, a: &Value) -> (bool, Value) {
        let views: Vec<String> = a.get("views").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).take(6).collect()).filter(|v: &Vec<String>| !v.is_empty())
            .unwrap_or_else(|| vec!["iso".into(), "front".into(), "top".into(), "right".into()]);
        let dim = |k: &str, d: u64| a.get(k).and_then(Value::as_u64).unwrap_or(d).clamp(160, 1600);
        let (w, hgt) = (dim("width", 560), dim("height", 420));
        let mut guard_ = self.inner.lock().await; let inner = &mut *guard_; let mut notes = vec![];
        if let Err(e) = self.ensure(seam, inner, &mut notes).await { return fail(json!({"error": e})) }
        let mut blocks = vec![]; let mut seen = 0;
        for v in &views {
            let c = inner.client.as_ref().unwrap();
            match c.call("screenshot", &json!({"view": v, "width": w, "height": hgt, "source": "model"}), self.budget("screenshot", Class::Look)).await {
                Ok(r) if !client::is_error(&r) => { blocks.push(json!({"type":"text","text": format!("view: {v}")})); blocks.extend(r.get("content").and_then(Value::as_array).into_iter().flatten().filter(|b| b["type"] == "image").cloned()); seen += 1 }
                Ok(r) => blocks.push(json!({"type":"text","text": format!("view {v}: {}", client::first_text(&r))})),
                Err(e) => { let stalled = e == CallErr::Timeout; return fail(self.recover(seam, inner, "look", Class::Look, stalled, &client::say(&e)).await) }
            }
        }
        let (i, m) = Self::probe(inner.client.as_ref().unwrap()).await;
        if seen > 0 { inner.edits_since_look = 0 }
        let mut bp = json!({"design": h::digest(i.as_ref(), m.as_ref()), "check": "Compare each view with the intent: proportions, feature positions, holes through, nothing missing or doubled. Fix what is wrong before going on."});
        if !notes.is_empty() { bp["notes"] = json!(notes) }
        blocks.push(json!({"type":"text","text": json!({"backplane": bp}).to_string()}));
        (seen > 0, json!({"$content": blocks}))
    }

    /// `solidcraft_status`: nothing is started for it.
    pub async fn status(&self, seam: &Seam) -> Value {
        let inner = self.inner.lock().await;
        json!({
            "running": inner.client.as_ref().map_or(false, Client::alive),
            "session": inner.client.as_ref().map(|c| c.mode.word()).unwrap_or(self.want(seam).word()),
            "design": inner.design, "opened": inner.source, "restarts": inner.restarts,
            "edits_since_look": inner.edits_since_look, "checkpoints": self.checkpoints().len(),
            "folder": self.root.to_string_lossy(),
        })
    }
    /// Ends the child (the hub is stopping, or the project went away).
    pub async fn stop(&self) { if let Some(c) = self.inner.lock().await.client.take() { c.kill() } }
}
