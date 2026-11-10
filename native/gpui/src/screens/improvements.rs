//! Improvements in the GPUI app (docs/improvements.md, docs/reporting/README.md): one panel, two destinations. The public GitHub issue is the
//! default: the description, evidence (words, a screenshot, a recording), optional details and diagnostics become an editable report that the
//! person reviews, then opens as a GitHub draft in their browser (nothing is submitted by Backplane). The internal destination starts a normal
//! thread with the request and its screenshot in the project the person picks (this machine's or a linked one's; the hub searches them and remembers the last:
//! `improvements.projects`), Backplane's own source project until they pick one. "Write report with agent" starts a visible thread whose answer
//! is loaded back as the report. The draft, its screenshot and recording stay on this device, in a private folder beside the token.
//! The report policy (body, link, template, prompt, diagnostics allowlist) is the hub's (`improvements.*`); this file keeps the person's draft,
//! runs the steps in order (each step is retried from where it stopped: keys and attachments keep their identity), and draws the panel.
use super::{capture, composer::{Composer, ComposerEvent}, improv_model::*};
use crate::{art, hub::Command, i18n::{tf, trs}, script::{Ctl, CtlEntry}, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc, time::Duration};

actions!(improvements, [Toggle]);

pub fn bind(cx: &mut App) { cx.bind_keys(bindings()); }
pub fn bindings() -> Vec<KeyBinding> {
    vec![KeyBinding::new("ctrl-alt-i", Toggle, None), KeyBinding::new("enter", super::composer::Newline, Some("ImpInput")), KeyBinding::new("ctrl-enter", super::composer::Go, Some("ImpInput"))]
}

/// What a reply is for. A step names the one that follows it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Next { Ctx, Diag(Then), Preview, Template, Prompt, WCreate, WCtx, WTurn, Load, ICreate, ICtx, IModes, ITurn, Picks, Remember }
#[derive(Clone, Copy, Debug, PartialEq)]
enum Then { Review, Write }
#[derive(Clone, Copy, Debug, PartialEq)]
enum Up { WImage, WVideo, IImage, IVideo }

struct Inputs { text: Entity<Composer>, evidence: Entity<Composer>, details: Entity<Composer>, review: Entity<Composer>, find: Entity<Composer> }

pub struct Imp {
    pub open: bool,
    pub draft: Draft,
    loaded: bool,
    inputs: Option<Inputs>,
    ctx: Value,
    projects: Vec<Project>,
    pub note: String,
    busy: bool,
    /// the panel is out of the way: a grab or a recording is taking this window's pixels
    hidden: bool,
    xid: Option<u32>,
    rec: Option<capture::Recording>,
    rec_text: String,
    next: HashMap<u64, Next>,
    seq: u64,
    dir: PathBuf,
    scope: String,
    target: Option<super::attach::Target>,
    diag: String,
    url: String,
    review_open: bool,
    flushing: bool,
    syncing: bool,
    shot: Option<Arc<Image>>,
    auto_shot: bool,
    origin: (String, String),
    evidence_ok: bool,
    previewed: bool,
    focus_pending: bool,
    /// A test hook only (script op `runtime`): the permission mode to give the hand-off thread before its turn. Unset, the thread has the normal configuration.
    runtime: Option<String>,
    modes_sent: bool,
    /// the hub's project rows for the search in `find` (newest question wins: `pick_seq`), and whether its remembered choice was taken for this draft
    picks: Vec<Value>,
    pick_seq: u64,
    picked: bool,
    /// the words last searched: the box notifies on every blink and focus, and only new words are asked (each question waits in `next`, which is bounded)
    find_q: Option<String>,
}

impl Imp {
    pub fn new(drafts_path: &Path, target: Option<super::attach::Target>) -> Self {
        let scope = target.as_ref().map_or("offline".to_string(), |t| t.addr.clone());
        Self { open: false, draft: Draft::default(), loaded: false, inputs: None, ctx: Value::Null, projects: vec![], note: String::new(), busy: false, hidden: false, xid: None, rec: None, rec_text: String::new(),
            next: HashMap::new(), seq: 0, dir: drafts_path.with_file_name("gpui-improvements"), scope, target, diag: String::new(), url: String::new(), review_open: false, flushing: false, syncing: false, shot: None, auto_shot: false, origin: (String::new(), String::new()), evidence_ok: false, previewed: false, focus_pending: false, runtime: None, modes_sent: false, picks: vec![], pick_seq: 0, picked: false, find_q: None }
    }
    fn file(&self, name: &str) -> PathBuf { self.dir.join(name) }
    /// The panel's own folder, private to this user.
    fn ensure_dir(&self) -> bool {
        crate::plat::private_dir(&self.dir)
    }
    /// Write the draft through a temp file (0600). An error is said, not swallowed: a handoff never goes ahead on an unsaved receipt.
    /// The screen recording in progress, if any, taken (its drop stops the recorder).
    pub fn take_recording(&mut self) -> Option<capture::Recording> { self.rec.take() }
    fn write_draft(&self) -> Result<(), String> {
        use std::io::Write;
        /* i18n-ok: drawn through tr */ if !self.ensure_dir() { return Err("Could not keep this draft on this device. Retry.".into()); }
        let mut d = self.draft.clone(); d.scope = self.scope.clone();
        /* i18n-ok: drawn through tr */ let text = serde_json::to_string(&d.to_json()).map_err(|_| "Could not keep this draft on this device. Retry.".to_string())?;
        let (tmp, dst) = (self.file("draft.tmp"), self.file("draft.json"));
        crate::plat::private_file(&tmp).and_then(|mut f| f.write_all(text.as_bytes())).and_then(|_| std::fs::rename(&tmp, &dst))
            /* i18n-ok: drawn through tr */ .map_err(|_| "Could not keep this draft on this device. Retry.".to_string())
    }
    fn read_draft(&self) -> Option<Draft> {
        if std::fs::metadata(self.file("draft.json")).map_or(true, |m| m.len() > 600_000) { return None }
        let d = Draft::from_json(&serde_json::from_str(&std::fs::read_to_string(self.file("draft.json")).ok()?).ok()?)?;
        (d.scope == self.scope).then_some(d)
    }
    fn forget_files(&mut self) { for n in ["draft.json", "shot.png", "shot.jpg", "shot.webp", "recording.webm"] { let _ = std::fs::remove_file(self.file(n)); } self.shot = None; }
    fn os() -> &'static str { std::env::consts::OS }
    fn version(&self) -> String { self.ctx["appVersion"].as_str().unwrap_or("unknown").to_string() }
    fn locked(&self) -> bool { self.busy || self.draft.locked() }
}

// ---- opening, closing, the draft's fields ----------------------------------------------------------------------------------------------------

fn inputs(this: &mut Backplane, cx: &mut Context<Backplane>) -> bool {
    if this.screens.imp.inputs.is_some() { return true }
    let make = |cx: &mut Context<Backplane>, hint: &str| { let e = cx.new(|cx| Composer::with_context(cx, "ImpInput")); e.update(cx, |i, _| i.set_placeholder(&trs(hint))); e };
    /* i18n-ok: drawn through tr */ let ins = Inputs { text: make(cx, "Describe the change…"), evidence: make(cx, "Describe the behavior in detail: what you did and what you saw."), details: make(cx, "Optional reproduction steps or other details…"), review: make(cx, "The public issue text"), find: make(cx, "Search projects…") };
    for (e, which) in [(&ins.text, 0u8), (&ins.evidence, 1), (&ins.details, 2), (&ins.review, 3)] {
        let which = which;
        cx.observe(e, move |this, e, cx| edited(this, which, e.read(cx).content.to_string(), cx)).detach();
        cx.subscribe(e, move |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { act(this, "primary", cx); }).detach();
    }
    cx.observe(&ins.find, move |this, e, cx| { let q = e.read(cx).content.to_string(); find(this, &q, cx) }).detach();
    cx.subscribe(&ins.find, move |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { if let Some(id) = this.screens.imp.picks.first().and_then(|p| p["id"].as_str()).map(str::to_string) { act(this, &format!("proj:{id}"), cx) } }).detach();
    this.screens.imp.inputs = Some(ins);
    true
}

/// The project search: the hub's rows for these words (the newest question's answer is the one drawn).
fn find(this: &mut Backplane, q: &str, cx: &mut Context<Backplane>) {
    if !this.screens.imp.open || this.screens.imp.find_q.as_deref() == Some(q) { return }
    this.screens.imp.find_q = Some(q.to_string());
    ask(this, "improvements.projects", json!({"q": q}), Next::Picks, cx); this.screens.imp.pick_seq = this.screens.imp.seq;
}
/// The project the request goes to, as a row: this machine's from the context, a linked machine's from the search.
fn chosen(imp: &Imp) -> Option<Project> {
    let id = &imp.draft.project; if id.is_empty() { return None }
    imp.projects.iter().find(|p| &p.id == id).cloned().or_else(|| imp.picks.iter().find(|p| p["id"].as_str() == Some(id)).map(|p| Project { id: id.clone(), title: s(p, "title"), root: s(p, "root") }))
}
/// A box was edited by the person. A box set by the code (loading a draft) changes nothing.
fn edited(this: &mut Backplane, which: u8, text: String, cx: &mut Context<Backplane>) {
    let imp = &mut this.screens.imp;
    if imp.syncing || !imp.open || imp.draft.locked() { return }
    match which {
        0 => { if imp.draft.text == text { return } imp.draft.text = text; imp.draft.forget_report(); imp.review_open = false; imp.evidence_ok = false; }
        1 => { if imp.draft.evidence == text { return } imp.draft.evidence = text; imp.draft.forget_report(); imp.review_open = false; imp.evidence_ok = false; }
        2 => { if imp.draft.details == text { return } imp.draft.details = text; imp.draft.forget_report(); imp.review_open = false; imp.evidence_ok = false; }
        _ => { // the report itself, edited: its link is asked of the hub (newest answer wins)
            if imp.draft.issue_body == text { return }
            imp.draft.report = text.clone(); imp.draft.issue_body = text.clone(); imp.draft.opened = false;
            let body = text; ask(this, "improvements.preview", json!({"body": body}), Next::Preview, cx);
        }
    }
    touch(this, cx); cx.notify();
}

fn set_box(this: &mut Backplane, e: &Entity<Composer>, text: &str, cx: &mut Context<Backplane>) {
    this.screens.imp.syncing = true; let t = text.to_string(); e.update(cx, |i, cx| i.set(t, cx)); this.screens.imp.syncing = false;
}
fn fill_boxes(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(ins) = this.screens.imp.inputs.as_ref().map(|i| (i.text.clone(), i.evidence.clone(), i.details.clone(), i.review.clone())) else { return };
    let d = this.screens.imp.draft.clone();
    set_box(this, &ins.0, &d.text, cx); set_box(this, &ins.1, &d.evidence, cx); set_box(this, &ins.2, &d.details, cx);
    set_box(this, &ins.3, if d.issue_body.is_empty() { &d.report } else { &d.issue_body }, cx);
}

/// Persist soon (400 ms after the last change).
fn touch(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.screens.imp.flushing { return } this.screens.imp.flushing = true;
    cx.spawn(async move |weak, cx| { cx.background_executor().timer(Duration::from_millis(400)).await; let _ = weak.update(cx, |this, _| { this.screens.imp.flushing = false; let _ = this.screens.imp.write_draft(); }); }).detach();
}
fn save_now(this: &mut Backplane) -> Result<(), String> { this.screens.imp.write_draft() }

pub fn toggle(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.screens.imp.open { close(this, cx) } else { open(this, cx) }
}
fn close(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.screens.imp.rec.is_some() { return }   // a recording ends with its own Stop or Cancel
    let _ = save_now(this); this.screens.imp.open = false; this.screens.imp.hidden = false; cx.notify();
}
pub fn is_open(this: &Backplane) -> bool { this.screens.imp.open }
/// Esc: close the panel if it is the thing on top.
pub fn escape(this: &mut Backplane, cx: &mut Context<Backplane>) -> bool { if this.screens.imp.open && this.screens.imp.rec.is_none() { close(this, cx); true } else { false } }

fn open(this: &mut Backplane, cx: &mut Context<Backplane>) {
    inputs(this, cx);
    let origin_thread = this.selected.clone();
    let origin_name = string(&this.data["thread"], "title");
    let imp = &mut this.screens.imp;
    imp.open = true; imp.hidden = false; imp.origin = (origin_thread.clone(), origin_name);
    if !imp.loaded {
        imp.loaded = true;
        imp.draft = imp.read_draft().unwrap_or_default();
        // a recording is kept for 24 hours; an older one (or one whose file is gone) is dropped
        let rec = imp.file("recording.webm");
        let age = std::fs::metadata(&rec).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
        if !imp.draft.recording.is_empty() && !(age.map_or(false, |a| a < Duration::from_secs(86_400)) && capture::is_webm(&rec)) { let _ = std::fs::remove_file(&rec); imp.draft.recording.clear(); imp.draft.inc_rec = false; imp.draft.rec_reviewed = false; }
        if !imp.draft.image.is_empty() { match std::fs::read(&imp.draft.image) { Ok(b) if !b.is_empty() => imp.shot = Some(Arc::new(Image::from_bytes(format_of(&imp.draft.image), b))), _ => { imp.draft.image.clear(); imp.draft.inc_shot = false; } } }
    }
    /* i18n-ok: drawn through tr */ imp.note = if imp.draft.opened { "This draft was already opened on GitHub. Check GitHub before explicitly allowing another copy.".into() } else if imp.draft.internal { "Send starts a new thread in the chosen project.".into() } else { "Review the issue before opening GitHub.".into() };
    imp.review_open = !imp.draft.issue_body.is_empty() && !imp.draft.internal;
    fill_boxes(this, cx);
    this.screens.imp.focus_pending = true;
    ask(this, "improvements.context", json!({"sourceThread": origin_thread}), Next::Ctx, cx);
    let q = this.screens.imp.inputs.as_ref().map(|i| i.find.read(cx).content.to_string()).unwrap_or_default(); this.screens.imp.find_q = None; find(this, &q, cx);
    // a fresh draft starts with a picture of the app as it was; the panel hides for the grab, then returns
    if this.screens.imp.draft.image.is_empty() && !this.screens.imp.locked() && this.screens.imp.rec.is_none() && capture::unavailable().is_none() {
        this.screens.imp.auto_shot = true;
        take_shot(this, cx);
    }
    cx.notify();
}
fn format_of(path: &str) -> ImageFormat { match path.rsplit('.').next().unwrap_or("") { "jpg" | "jpeg" => ImageFormat::Jpeg, "webp" => ImageFormat::Webp, _ => ImageFormat::Png } }

// ---- asking the hub ---------------------------------------------------------------------------------------------------------------------------

fn ask(this: &mut Backplane, method: &str, params: Value, next: Next, cx: &mut Context<Backplane>) {
    let imp = &mut this.screens.imp; imp.seq += 1; let id = imp.seq;
    imp.next.insert(id, next); if imp.next.len() > 32 { imp.next.clear(); imp.next.insert(id, next); }
    /* i18n-ok: drawn through tr */ if this.tx.try_send(Command::Ask(method.to_string(), params, id)).is_err() { this.screens.imp.next.remove(&id); fail(this, "The hub is not reachable. Reconnect, then retry.".into(), cx); }
}
fn fail(this: &mut Backplane, why: String, cx: &mut Context<Backplane>) { let imp = &mut this.screens.imp; imp.busy = false; imp.note = why; cx.notify(); }

/// The hub's answer to a request this panel made.
pub fn answered(this: &mut Backplane, id: u64, reply: Value, cx: &mut Context<Backplane>) {
    let Some(next) = this.screens.imp.next.remove(&id) else { return };
    let r = result_of(&reply).clone();
    let ok = reply["ok"] != false && r["error"].is_null();
    /* i18n-ok: drawn through tr */ if !ok { if matches!(next, Next::Preview | Next::Ctx) { this.screens.imp.busy = false; } return fail(this, refusal(&reply, "The hub could not accept this request. Retry."), cx) }
    match next {
        Next::Ctx => {
            let imp = &mut this.screens.imp;
            imp.projects = projects(&r); imp.ctx = r.clone();
            let found = backplane_project(&imp.projects).map(|p| p.id.clone());
            // Backplane's own source until the person picks (the search's answer brings their last choice; a linked machine's project is not in this list)
            if !imp.draft.locked() && imp.draft.key.is_empty() && imp.draft.project.is_empty() { if let Some(id) = found.clone() { imp.draft.project = id; } }
            if imp.draft.writer_project.is_empty() {
                let work: Vec<&Project> = imp.projects.iter().filter(|p| p.id != "scratch" && !p.root.is_empty()).collect();
                let src = s(&r, "project");
                // the Backplane source project first (backplane-gpui), then the project the person is in, then the only one there is
                imp.draft.writer_project = backplane_project(&imp.projects).filter(|b| work.iter().any(|p| p.id == b.id)).map(|p| p.id.clone())
                    .or_else(|| work.iter().find(|p| p.id == src).map(|p| p.id.clone())).or_else(|| (work.len() == 1).then(|| work[0].id.clone())).unwrap_or_default();
            }
        }
        Next::Diag(then) => {
            this.screens.imp.diag = r["report"].as_str().unwrap_or("").to_string();
            /* i18n-ok: drawn through tr */ if this.screens.imp.diag.is_empty() { return fail(this, "Diagnostics are unavailable. Uncheck diagnostics to continue.".into(), cx) }
            match then { Then::Review => preview_step(this, cx), Then::Write => prompt_step(this, cx) }
        }
        Next::Preview => {
            let (body, url) = (s(&r, "body"), s(&r, "url"));
            let imp = &mut this.screens.imp;
            // an older answer for text the person has since edited is dropped
            if !imp.draft.report.is_empty() && imp.draft.report != body { return }
            /* i18n-ok: drawn through tr */ if body.is_empty() { return fail(this, "Could not prepare the exact issue text. Your draft is kept.".into(), cx) }
            imp.url = url; imp.busy = false; imp.previewed = true;
            if imp.draft.issue_body != body { imp.draft.opened = false; }
            imp.draft.issue_body = body.clone(); imp.review_open = true;
            /* i18n-ok: drawn through tr */ imp.note = if imp.url.is_empty() { "This report is too long for a GitHub link. Copy or save the complete text, then paste it into GitHub. Nothing is truncated.".into() } else { "Review the exact public text below. Open GitHub draft sends this text to GitHub; sign in with your account and press Submit there. Backplane has not submitted an issue.".into() };
            if let Some(e) = imp.inputs.as_ref().map(|i| i.review.clone()) { let current = e.read(cx).content.to_string(); if current != body { set_box(this, &e, &body, cx); } }
            let _ = save_now(this);
        }
        Next::Template => {
            let t = s(&r, "template");
            let imp = &mut this.screens.imp;
            imp.draft.forget_report(); imp.draft.report = t.clone(); imp.draft.issue_body = t.clone(); imp.previewed = false; imp.review_open = true; imp.evidence_ok = false; imp.busy = false;
            /* i18n-ok: drawn through tr */ imp.note = "Fill the unknown fields in this editable template, then press Review issue.".into();
            if let Some(e) = imp.inputs.as_ref().map(|i| i.review.clone()) { set_box(this, &e, &t, cx); }
            let _ = save_now(this);
        }
        Next::Prompt => { let prompt = s(&r, "prompt"); writer_init(this, prompt, cx); }
        Next::WCreate => {
            let thread = this.screens.imp.draft.writer.as_ref().map(|w| w.thread()).unwrap_or_default();
            /* i18n-ok: drawn through tr */ if s(&r, "thread") != thread { return fail(this, "The hub did not confirm the report thread. Retry the same writer.".into(), cx) }
            if let Some(w) = this.screens.imp.draft.writer.as_mut() { w.created = true; }
            let _ = save_now(this); advance_writer(this, cx);
        }
        Next::WCtx => match r["accountBinding"].as_str() {
            Some(b) if r["thread"] == json!(this.screens.imp.draft.writer.as_ref().map(|w| w.thread()).unwrap_or_default()) => { if let Some(w) = this.screens.imp.draft.writer.as_mut() { w.binding = Some(b.to_string()); } let _ = save_now(this); advance_writer(this, cx); }
            /* i18n-ok: drawn through tr */ _ => return fail(this, "The hub did not confirm the report account. Retry.".into(), cx),
        },
        Next::WTurn => {
            if let Some(w) = this.screens.imp.draft.writer.as_mut() { w.started = true; }
            let imp = &mut this.screens.imp; imp.busy = false;
            /* i18n-ok: drawn through tr */ imp.note = "Report agent started in the selected project. Load the report when it finishes, or open the thread to answer its questions.".into();
            let _ = save_now(this);
        }
        Next::Load => {
            let body = s(&r, "body");
            /* i18n-ok: drawn through tr */ if r["done"] != true { return fail(this, "The report agent is still working. Open its thread for questions, then load the report when it finishes.".into(), cx) }
            /* i18n-ok: drawn through tr */ if body.trim().is_empty() || body.len() > MAX_BODY { return fail(this, "No complete report is available. Open the report thread and check the agent response.".into(), cx) }
            let imp = &mut this.screens.imp; imp.busy = false;
            imp.draft.report = body.clone(); imp.draft.issue_body.clear(); imp.draft.opened = false; imp.review_open = true; imp.previewed = false; imp.evidence_ok = false;
            /* i18n-ok: drawn through tr */ imp.note = "Agent report loaded. Edit it, remove private information, then press Review issue.".into();
            if let Some(e) = imp.inputs.as_ref().map(|i| i.review.clone()) { set_box(this, &e, &body, cx); }
            let _ = save_now(this);
        }
        Next::ICreate => {
            let want = made_thread(&this.screens.imp.draft.project, &this.screens.imp.draft.key);
            /* i18n-ok: drawn through tr */ if s(&r, "thread") != want { return fail(this, "The hub did not confirm this conversation. Retry Send.".into(), cx) }
            this.screens.imp.draft.thread = want; let _ = save_now(this); advance_internal(this, cx);
        }
        Next::ICtx => match r["accountBinding"].as_str() {
            Some(b) => { this.screens.imp.draft.binding = Some(b.to_string()); let _ = save_now(this); advance_internal(this, cx); }
            /* i18n-ok: drawn through tr */ None => return fail(this, "The hub did not confirm this conversation’s account. Retry Send.".into(), cx),
        },
        Next::IModes => { advance_internal(this, cx); }
        Next::Picks => {
            let imp = &mut this.screens.imp; if id != imp.pick_seq { return }
            imp.picks = r["projects"].as_array().cloned().unwrap_or_default();
            let last = s(&r, "project");
            if !imp.picked && !imp.draft.locked() && imp.draft.key.is_empty() { imp.picked = true; if !last.is_empty() { imp.draft.project = last; } }
        }
        Next::Remember => {}
        Next::ITurn => {
            let thread = this.screens.imp.draft.thread.clone();
            this.screens.imp.draft.accepted = true; this.screens.imp.picked = false; let _ = save_now(this);
            let _ = this.tx.try_send(Command::Select(thread));
            /* i18n-ok: drawn through tr */ let imp = &mut this.screens.imp; imp.forget_files(); imp.draft = Draft::default(); imp.busy = false; imp.open = false; imp.note = "Request sent. Opening the new thread.".into();
            fill_boxes(this, cx);
        }
    }
    cx.notify();
}
fn s(v: &Value, k: &str) -> String { v[k].as_str().unwrap_or("").to_string() }
/// How many of the search's projects the panel shows.
const PICK_ROWS: usize = 8;

// ---- the public report ------------------------------------------------------------------------------------------------------------------------

fn review(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if let Err(e) = this.screens.imp.draft.check_public() { return fail(this, e.into(), cx) }
    /* i18n-ok: drawn through tr */ this.screens.imp.busy = true; this.screens.imp.note = "Preparing the report…".into();
    if this.screens.imp.draft.inc_diag { ask(this, "improvements.diagnostics", json!({}), Next::Diag(Then::Review), cx) } else { this.screens.imp.diag.clear(); preview_step(this, cx) }
}
fn diag_text(imp: &Imp) -> String { if imp.draft.inc_diag { imp.diag.clone() } else { String::new() } }
fn preview_step(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let imp = &this.screens.imp;
    if !imp.draft.report.is_empty() { let body = imp.draft.report.clone(); return ask(this, "improvements.preview", json!({"body": body}), Next::Preview, cx) }
    let d = &imp.draft;
    let p = json!({"text": format!("{}\n\nEvidence\n{}", d.text, d.evidence_text()), "os": Imp::os(), "version": imp.version(), "details": if d.inc_details { d.details.clone() } else { String::new() }, "diagnostics": diag_text(imp)});
    ask(this, "improvements.preview", p, Next::Preview, cx)
}
fn write_report(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if let Err(e) = this.screens.imp.draft.check_writer() { return fail(this, e.into(), cx) }
    /* i18n-ok: drawn through tr */ this.screens.imp.busy = true; this.screens.imp.note = "Starting the report agent…".into();
    if this.screens.imp.draft.writer.is_some() { return advance_writer(this, cx) }   // a retry of the same writer
    if this.screens.imp.draft.inc_diag { ask(this, "improvements.diagnostics", json!({}), Next::Diag(Then::Write), cx) } else { this.screens.imp.diag.clear(); prompt_step(this, cx) }
}
fn prompt_step(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let imp = &this.screens.imp; let d = &imp.draft;
    /* i18n-ok: evidence text sent to the report agent */ let evidence = format!("{}{}", d.evidence_text(), if d.inc_details { format!("\nUser-elected details\n{}", d.details) } else { String::new() });
    let p = json!({"description": d.text, "os": Imp::os(), "version": imp.version(), "evidence": evidence, "diagnostics": diag_text(imp)});
    ask(this, "improvements.prompt", p, Next::Prompt, cx)
}
fn writer_init(this: &mut Backplane, prompt: String, cx: &mut Context<Backplane>) {
    /* i18n-ok: drawn through tr */ if prompt.is_empty() || prompt.len() > MAX_TEXT { return fail(this, "The report description is too long for the agent. Shorten it or fill the template locally.".into(), cx) }
    let d = &mut this.screens.imp.draft;
    d.writer = Some(Writer { key: new_key(), project: d.writer_project.clone(), prompt, with_image: d.inc_shot && !d.image.is_empty(), with_video: d.inc_rec && d.rec_reviewed && !d.recording.is_empty(), ..Writer::default() });
    let _ = save_now(this); advance_writer(this, cx);
}
/// The next missing step of the report writer: create its thread, its account, its attachments, then the turn.
fn advance_writer(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(w) = this.screens.imp.draft.writer.clone() else { return };
    let thread = w.thread();
    /* i18n-ok: a thread title sent to the hub */ if !w.created { return ask(this, "thread.create", json!({"project": w.project, "title": "Write an issue report", "creationKey": w.key}), Next::WCreate, cx) }
    if w.binding.is_none() { return ask(this, "improvements.context", json!({"sourceThread": thread}), Next::WCtx, cx) }
    if w.with_image && w.path.is_empty() { let (p, k) = (this.screens.imp.draft.image.clone(), w.key.clone()); return upload(this, Up::WImage, thread, k, p, cx) }
    if w.with_video && w.video_path.is_empty() {
        let k = if w.video_key.is_empty() { let k = new_key(); if let Some(w) = this.screens.imp.draft.writer.as_mut() { w.video_key = k.clone(); } let _ = save_now(this); k } else { w.video_key.clone() };
        let p = this.screens.imp.draft.recording.clone(); return upload(this, Up::WVideo, thread, k, p, cx)
    }
    let message = if w.message.is_empty() { writer_message(&w.prompt, &w.path, &w.video_path) } else { w.message.clone() };
    if let Some(w) = this.screens.imp.draft.writer.as_mut() { w.message = message.clone(); }
    let _ = save_now(this);
    ask(this, "turn.start", json!({"thread": thread, "text": message, "msg": format!("issue-writer-{}", w.key), "accountBinding": w.binding}), Next::WTurn, cx)
}
fn load_report(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(w) = this.screens.imp.draft.writer.clone() else { return };
    /* i18n-ok: drawn through tr */ this.screens.imp.busy = true; this.screens.imp.note = "Loading the agent report…".into();
    ask(this, "improvements.report", json!({"thread": w.thread(), "accountBinding": w.binding}), Next::Load, cx)
}

// ---- the internal request ---------------------------------------------------------------------------------------------------------------------

fn send_internal(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.screens.imp.draft.accepted { let t = this.screens.imp.draft.thread.clone(); let _ = this.tx.try_send(Command::Select(t)); return close(this, cx) }
    if let Err(e) = this.screens.imp.draft.check_internal() { return fail(this, e.into(), cx) }
    if this.screens.imp.draft.key.is_empty() { this.screens.imp.draft.key = new_key(); }
    /* i18n-ok: drawn through tr */ this.screens.imp.busy = true; this.screens.imp.note = "Sending…".into();
    if let Err(e) = save_now(this) { return fail(this, e, cx) }
    advance_internal(this, cx)
}
fn advance_internal(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let d = this.screens.imp.draft.clone();
    // a linked machine's project: its thread runs there, under that machine's own sign-in; files on this computer stay here
    let far = is_far(&d.project);
    /* i18n-ok: drawn through tr; the title is sent to the hub */ if d.thread.is_empty() { this.screens.imp.note = "Creating the thread…".into(); return ask(this, "thread.create", json!({"project": d.project, "title": "New thread", "creationKey": d.key, "branch": work_branch(&d.key)}), Next::ICreate, cx) }
    if far && d.binding.is_none() { this.screens.imp.draft.binding = Some(String::new()); }
    if !far && d.binding.is_none() { return ask(this, "improvements.context", json!({"sourceThread": d.thread}), Next::ICtx, cx) }
    if let Some(rt) = this.screens.imp.runtime.clone() { if !this.screens.imp.modes_sent { this.screens.imp.modes_sent = true; return ask(this, "thread.modes", json!({"thread": d.thread, "runtime": rt}), Next::IModes, cx) } }
    /* i18n-ok: drawn through tr */ if !far && !d.image.is_empty() && d.path.is_empty() { this.screens.imp.note = "Uploading the screenshot…".into(); return upload(this, Up::IImage, d.thread.clone(), d.key.clone(), d.image.clone(), cx) }
    let with_video = !far && d.inc_rec && d.rec_reviewed && !d.recording.is_empty();
    if with_video && d.video_path.is_empty() {
        let k = if d.video_key.is_empty() { let k = new_key(); this.screens.imp.draft.video_key = k.clone(); let _ = save_now(this); k } else { d.video_key.clone() };
        /* i18n-ok: drawn through tr */ this.screens.imp.note = "Uploading the selected recording…".into(); return upload(this, Up::IVideo, d.thread.clone(), k, d.recording.clone(), cx)
    }
    let message = if d.message.is_empty() {
        let imp = &this.screens.imp; let target = chosen(imp);
        let (t, n) = imp.origin.clone();
        /* i18n-ok: a view name in the context data */ let c = context(&t, &n, "Thread", &imp.ctx, target.as_ref());
        let own = backplane_project(&imp.projects).map_or(false, |b| b.id == d.project);
        let (path, video) = (if far { "" } else { d.path.as_str() }, if with_video { d.video_path.as_str() } else { "" });
        if own { improvement_message_with(&d.text, &c, &handoff(&work_branch(&d.key)), path, video) }
        else { let title = target.map(|p| p.title).unwrap_or_default(); improvement_message_in(&d.text, &c, &title, &handoff_in(&work_branch(&d.key), &title), path, video) }
    } else { d.message.clone() };
    /* i18n-ok: drawn through tr */ if message.len() > MAX_MESSAGE { return fail(this, "This request is too long. Shorten it before sending.".into(), cx) }
    this.screens.imp.draft.message = message.clone();
    if let Err(e) = save_now(this) { return fail(this, e, cx) }
    /* i18n-ok: drawn through tr */ this.screens.imp.note = "Starting the request…".into();
    let mut p = json!({"thread": d.thread, "text": message, "msg": format!("improvement-request-{}", d.key)}); if !far { p["accountBinding"] = json!(d.binding) }
    ask(this, "turn.start", p, Next::ITurn, cx)
}

// ---- attachments (the hub's POST /attach, off the UI thread) ---------------------------------------------------------------------------------

fn upload(this: &mut Backplane, kind: Up, thread: String, key: String, file: String, cx: &mut Context<Backplane>) {
    /* i18n-ok: drawn through tr */ let Some(t) = this.screens.imp.target.clone() else { return fail(this, "Attachments need a hub on plain http://; this one is not.".into(), cx) };
    let name = format!("{}{}", if matches!(kind, Up::WVideo | Up::IVideo) { "backplane-recording" } else { "backplane" }, match file.rsplit('.').next().unwrap_or("png") { "jpg" | "jpeg" => ".jpg", "webp" => ".webp", "webm" => ".webm", _ => ".png" });
    let th = thread.clone();
    cx.spawn(async move |weak, cx| {
        let result = cx.background_executor().spawn(async move { upload_file(&t, &th, &key, &name, Path::new(&file)) }).await;
        let _ = weak.update(cx, |this, cx| uploaded(this, kind, result, cx));
    }).detach();
}
/// POST /attach with the file as the body; the hub answers `{thread, attached}`.
pub fn upload_file(t: &super::attach::Target, thread: &str, key: &str, name: &str, file: &Path) -> Result<String, String> {
    use std::io::{Read, Write};
    /* i18n-ok: drawn through tr */ let size = std::fs::metadata(file).map_err(|_| "The file is gone. Choose it again.".to_string())?.len();
    /* i18n-ok: drawn through tr */ if size == 0 || size > 64 * 1024 * 1024 { return Err("This file cannot be sent.".into()) }
    /* i18n-ok: drawn through tr */ let mut s = t.stream().map_err(|_| "The image did not arrive. Reconnect, then retry Send.".to_string())?;
    let _ = s.set_timeouts(Duration::from_secs(60));
    s.write_all(super::attach::head(t, thread, key, name, size).as_bytes()).map_err(|e| e.to_string())?;
    let n = std::io::copy(&mut std::fs::File::open(file).map_err(|e| e.to_string())?.take(size), &mut s).map_err(|e| e.to_string())?;
    /* i18n-ok: drawn through tr */ if n != size { return Err("The file changed while sending. Retry.".into()) }
    let body = super::attach::body_of(&String::from_utf8_lossy(&super::attach::read_response(&mut s).map_err(|e| e.to_string())?));
    /* i18n-ok: drawn through tr */ let v: Value = serde_json::from_str(&body).map_err(|_| "The image did not arrive. Retry Send.".to_string())?;
    /* i18n-ok: drawn through tr */ match (v["thread"].as_str(), v["attached"].as_str()) { (Some(t2), Some(p)) if t2 == thread => Ok(p.to_string()), _ => Err(v["text"].as_str().unwrap_or("The image did not arrive. Retry Send.").to_string()) }
}
fn uploaded(this: &mut Backplane, kind: Up, r: Result<String, String>, cx: &mut Context<Backplane>) {
    let path = match r { Ok(p) => p, Err(e) => return fail(this, e, cx) };
    let d = &mut this.screens.imp.draft;
    match kind {
        Up::WImage => if let Some(w) = d.writer.as_mut() { w.path = path },
        Up::WVideo => if let Some(w) = d.writer.as_mut() { w.video_path = path },
        Up::IImage => d.path = path,
        Up::IVideo => d.video_path = path,
    }
    let _ = save_now(this);
    if matches!(kind, Up::WImage | Up::WVideo) { advance_writer(this, cx) } else { advance_internal(this, cx) }
}

// ---- screenshot and recording -----------------------------------------------------------------------------------------------------------------

/// This app's window id, looked up when first needed (a grab or a recording) and kept.
fn window_id(this: &mut Backplane) -> Option<u32> { if this.screens.imp.xid.is_none() { this.screens.imp.xid = capture::own_window(); } this.screens.imp.xid }

/// Called while drawing: gives the description box the focus after an open.
pub fn tick(this: &mut Backplane, window: &mut Window, cx: &mut Context<Backplane>) {
    if this.screens.imp.focus_pending && this.screens.imp.open && !this.screens.imp.hidden {
        this.screens.imp.focus_pending = false;
        if let Some(e) = this.screens.imp.inputs.as_ref().map(|i| i.text.clone()) { e.focus_handle(cx).focus(window); }
    }
}

fn take_shot(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.screens.imp.locked() { return }
    /* i18n-ok: drawn through tr */ if let Some(why) = capture::unavailable() { return fail(this, format!("{why} Choose an image instead."), cx) }
    /* i18n-ok: drawn through tr */ let Some(xid) = window_id(this) else { return fail(this, "This window cannot be captured here. Choose an image instead.".into(), cx) };
    /* i18n-ok: drawn through tr */ if !this.screens.imp.ensure_dir() { return fail(this, "Could not keep this draft on this device. Retry.".into(), cx) }
    let out = this.screens.imp.file("shot.png");
    /* i18n-ok: drawn through tr */ this.screens.imp.hidden = true; this.screens.imp.note = "Capturing…".into(); cx.notify();
    cx.spawn(async move |weak, cx| {
        cx.background_executor().timer(Duration::from_millis(400)).await;   // the panel is gone from the next frames
        let result = cx.background_executor().spawn(async move { capture::shot(xid, &out).map(|b| (b, out)) }).await;
        let _ = weak.update(cx, |this, cx| {
            let imp = &mut this.screens.imp; imp.hidden = false;
            match result {
                Ok((bytes, path)) => {
                    let auto = std::mem::take(&mut this.screens.imp.auto_shot);
                    set_image(this, path.to_string_lossy().into_owned(), bytes, cx);
                    if auto { if this.screens.imp.draft.internal { this.screens.imp.draft.inc_shot = true; } let _ = save_now(this); }
                    /* i18n-ok: drawn through tr */ this.screens.imp.note = if auto { "Screenshot captured on open.".into() } else { "Screenshot refreshed.".into() };
                }
                /* i18n-ok: drawn through tr */ Err(e) => { this.screens.imp.auto_shot = false; this.screens.imp.note = format!("{e} Your draft and previous image are kept."); }
            }
            cx.notify();
        });
    }).detach();
}
fn set_image(this: &mut Backplane, path: String, bytes: Vec<u8>, cx: &mut Context<Backplane>) {
    let imp = &mut this.screens.imp;
    imp.shot = Some(Arc::new(Image::from_bytes(format_of(&path), bytes)));
    imp.draft.image = path; imp.draft.path.clear(); imp.draft.forget_report(); imp.review_open = false;
    let _ = save_now(this); cx.notify();
}
fn choose_image(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.screens.imp.locked() { return }
    let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
    cx.spawn(async move |weak, cx| { if let Ok(Ok(Some(paths))) = rx.await { if let Some(p) = paths.into_iter().next() { let _ = weak.update(cx, |this, cx| use_image_file(this, &p, cx)); } } }).detach();
}
/// A chosen image: PNG, JPEG or WebP up to 10 MB, copied into the draft folder (the draft owns its image).
pub fn use_image_file(this: &mut Backplane, p: &Path, cx: &mut Context<Backplane>) {
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    /* i18n-ok: drawn through tr */ let ext = match ext.as_str() { "png" => "png", "jpg" | "jpeg" => "jpg", "webp" => "webp", _ => return fail(this, "Choose a PNG, JPEG or WebP image.".into(), cx) };
    let bytes = match std::fs::metadata(p) { Ok(m) if m.len() > 0 && m.len() <= 10 * 1024 * 1024 => std::fs::read(p).unwrap_or_default(), _ => Vec::new() };
    /* i18n-ok: drawn through tr */ if bytes.is_empty() { return fail(this, "Choose an image up to 10 MB.".into(), cx) }
    /* i18n-ok: drawn through tr */ if !this.screens.imp.ensure_dir() { return fail(this, "Could not keep this draft on this device. Retry.".into(), cx) }
    let dst = this.screens.imp.file(&format!("shot.{ext}"));
    /* i18n-ok: drawn through tr */ if std::fs::write(&dst, &bytes).is_err() { return fail(this, "Could not keep this image on this device. Retry.".into(), cx) }
    /* i18n-ok: drawn through tr */ set_image(this, dst.to_string_lossy().into_owned(), bytes, cx); this.screens.imp.note = "Image refreshed.".into();
}

fn record_start(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let imp = &mut this.screens.imp;
    if imp.locked() || !imp.draft.key.is_empty() || imp.rec.is_some() { return }
    if let Some(why) = capture::unavailable() { return fail(this, why.into(), cx) }
    /* i18n-ok: drawn through tr */ let Some(xid) = window_id(this) else { return fail(this, "This window cannot be recorded here.".into(), cx) };
    /* i18n-ok: drawn through tr */ if !this.screens.imp.ensure_dir() { return fail(this, "Could not keep this draft on this device. Retry.".into(), cx) }
    match capture::Recording::start(xid, &this.screens.imp.file("recording-new.webm")) {
        Ok(r) => { let imp = &mut this.screens.imp; imp.rec = Some(r); imp.hidden = true; imp.rec_text = tf("gpui.imp.rec.start", &[("max", (capture::MAX_SECS as u64).into())]); poll_recording(cx); }
        Err(e) => this.screens.imp.note = e,
    }
    cx.notify();
}
/// The indicator's clock: every 250 ms while recording; ffmpeg ending itself (time or size cap) finishes the clip.
fn poll_recording(cx: &mut Context<Backplane>) {
    cx.spawn(async move |weak, cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(250)).await;
            let Ok(more) = weak.update(cx, |this, cx| {
                let imp = &mut this.screens.imp;
                let Some(r) = imp.rec.as_mut() else { return false };
                let (secs, bytes, running) = (r.elapsed().as_secs(), r.bytes(), r.running());
                imp.rec_text = tf("gpui.imp.rec.progress", &[("secs", secs.into()), ("max", (capture::MAX_SECS as u64).into()), ("mib", format!("{:.1}", bytes as f64 / 1048576.0).into())]);
                cx.notify();
                if !running || secs >= capture::MAX_SECS as u64 { record_stop(this, cx); false } else { true }
            }) else { return };
            if !more { return }
        }
    }).detach();
}
fn record_stop(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(r) = this.screens.imp.rec.take() else { return };
    this.screens.imp.rec_text = trs("Finishing recording…");
    let out = this.screens.imp.file("recording.webm"); let from = r.path.clone();
    cx.spawn(async move |weak, cx| {
        /* i18n-ok: drawn through tr */ let result = cx.background_executor().spawn(async move { r.stop().and_then(|ms| std::fs::rename(&from, &out).map(|_| (ms, out)).map_err(|_| "Could not keep the recording on this device.".to_string())) }).await;
        let _ = weak.update(cx, |this, cx| {
            let imp = &mut this.screens.imp; imp.hidden = false; imp.rec_text.clear();
            match result {
                Ok((ms, path)) => { imp.draft.recording = path.to_string_lossy().into_owned(); imp.draft.recording_ms = ms; imp.draft.inc_rec = false; imp.draft.rec_reviewed = false; imp.draft.forget_report(); imp.review_open = false;
                    /* i18n-ok: drawn through tr */ imp.note = "Recording saved locally. Play it, then select it to attach. Clips expire after 24 hours.".into(); let _ = save_now(this); }
                Err(e) => imp.note = e,
            }
            cx.notify();
        });
    }).detach();
}
fn record_cancel(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if let Some(r) = this.screens.imp.rec.take() { r.cancel(); }
    /* i18n-ok: drawn through tr */ let imp = &mut this.screens.imp; imp.hidden = false; imp.rec_text.clear(); imp.note = "Recording canceled. Your draft is kept.".into(); cx.notify();
}

// ---- opening other programs and saving files --------------------------------------------------------------------------------------------------

/// The program that opens a link or a file: `BACKPLANE_GPUI_OPEN` (tests point it at a recorder), else xdg-open (open on a Mac).
pub fn opener() -> String { std::env::var("BACKPLANE_GPUI_OPEN").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| if cfg!(target_os = "macos") { "open".into() } else { "xdg-open".into() }) }
pub(crate) fn open_target(what: &str) -> Result<(), String> {
    /* i18n-ok: drawn through tr */ std::process::Command::new(opener()).arg(what).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map(|_| ()).map_err(|_| "Could not open it. Copy the text and paste it into GitHub.".to_string())
}
/// Where a saved file goes: `BACKPLANE_GPUI_SAVE_DIR`, else ~/Downloads (else the home folder). Never over an existing file.
fn save_file(name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    /* i18n-ok: drawn through tr */ let dir = std::env::var_os("BACKPLANE_GPUI_SAVE_DIR").map(PathBuf::from).or_else(|| crate::plat::user_home().map(|h| { let d = h.join("Downloads"); if d.is_dir() { d } else { h } })).ok_or("No folder to save in.")?;
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    for i in 0..1000 {
        let p = dir.join(if i == 0 { name.to_string() } else { format!("{stem}-{i}.{ext}") });
        /* i18n-ok: drawn through tr */ if let Ok(mut f) = std::fs::OpenOptions::new().write(true).create_new(true).open(&p) { use std::io::Write; return f.write_all(bytes).map(|_| p).map_err(|_| "Could not save the file.".to_string()) }
    }
    /* i18n-ok: drawn through tr */ Err("Could not save the file.".into())
}

// ---- one place for every press ----------------------------------------------------------------------------------------------------------------

pub fn act(this: &mut Backplane, id: &str, cx: &mut Context<Backplane>) {
    let locked = this.screens.imp.locked();
    let flip = |this: &mut Backplane, f: fn(&mut Draft)| { f(&mut this.screens.imp.draft); };
    match id {
        "toggle" => return toggle(this, cx),
        "close" => return close(this, cx),
        "dest-issue" | "dest-internal" if !locked && this.screens.imp.draft.key.is_empty() => {
            let d = &mut this.screens.imp.draft; d.internal = id == "dest-internal"; d.inc_rec = false; d.forget_report(); this.screens.imp.review_open = false;
            /* i18n-ok: drawn through tr */ this.screens.imp.note = if id == "dest-internal" { "Send starts a new thread in the chosen project.".into() } else { "Review the issue before opening GitHub.".into() };
        }
        "chk-shot" if !locked => { flip(this, |d| d.inc_shot = !d.inc_shot); this.screens.imp.draft.forget_report(); this.screens.imp.review_open = false; this.screens.imp.evidence_ok = false; }
        "chk-diag" if !locked => { flip(this, |d| d.inc_diag = !d.inc_diag); this.screens.imp.draft.forget_report(); this.screens.imp.review_open = false; this.screens.imp.evidence_ok = false; this.screens.imp.diag.clear(); }
        "chk-details" if !locked => { flip(this, |d| d.inc_details = !d.inc_details); this.screens.imp.draft.forget_report(); this.screens.imp.review_open = false; this.screens.imp.evidence_ok = false; }
        "chk-rec" if !locked && this.screens.imp.draft.rec_reviewed && !this.screens.imp.draft.recording.is_empty() => { flip(this, |d| d.inc_rec = !d.inc_rec); this.screens.imp.draft.forget_report(); this.screens.imp.review_open = false; this.screens.imp.evidence_ok = false; }
        "chk-evidence" => { let imp = &mut this.screens.imp; imp.evidence_ok = !imp.evidence_ok; }
        "capture" => return take_shot(this, cx),
        "choose" => return choose_image(this, cx),
        "record" => return record_start(this, cx),
        "rec-stop" => return record_stop(this, cx),
        "rec-cancel" => return record_cancel(this, cx),
        "rec-play" => {
            let p = this.screens.imp.draft.recording.clone();
            /* i18n-ok: drawn through tr */ if !p.is_empty() { match open_target(&p) { Ok(()) => { this.screens.imp.draft.rec_reviewed = true; this.screens.imp.note = "Playing the recording. Select it when you have reviewed the whole clip.".into(); } Err(e) => this.screens.imp.note = e } }
        }
        "rec-discard" if !locked && this.screens.imp.draft.key.is_empty() => {
            let _ = std::fs::remove_file(this.screens.imp.file("recording.webm"));
            let d = &mut this.screens.imp.draft; d.recording.clear(); d.inc_rec = false; d.rec_reviewed = false; d.forget_report(); this.screens.imp.review_open = false;
        }
        /* i18n-ok: drawn through tr */ "save-image" => { let imp = &mut this.screens.imp; if let Ok(b) = std::fs::read(&imp.draft.image) { let ext = imp.draft.image.rsplit('.').next().unwrap_or("png").to_string(); imp.note = match save_file(&format!("backplane-screenshot.{ext}"), &b) { Ok(p) => format!("Saved {}", p.display()), Err(e) => e }; } }
        "primary" => { if this.screens.imp.busy { return } if this.screens.imp.draft.internal { send_internal(this, cx) } else { review(this, cx) } return }
        "write" if !locked => return write_report(this, cx),
        "load" if !locked => return load_report(this, cx),
        "open-writer" => { if let Some(w) = this.screens.imp.draft.writer.as_ref() { let t = w.thread(); let _ = this.tx.try_send(Command::Select(t)); close(this, cx); } return }
        "template" if !locked => { this.screens.imp.busy = true; ask(this, "improvements.template", json!({}), Next::Template, cx); return }
        /* i18n-ok: drawn through tr */ "copy" => { let body = this.screens.imp.draft.issue_body.clone(); if !body.is_empty() { cx.write_to_clipboard(ClipboardItem::new_string(body)); this.screens.imp.note = "Exact issue text copied.".into(); } }
        /* i18n-ok: drawn through tr */ "save" => { let imp = &mut this.screens.imp; if !imp.draft.issue_body.is_empty() { imp.note = match save_file("backplane-issue.txt", imp.draft.issue_body.as_bytes()) { Ok(p) => format!("Saved {}", p.display()), Err(e) => e }; } }
        /* i18n-ok: drawn through tr */ "allow-again" if this.screens.imp.draft.opened => { this.screens.imp.draft.opened = false; this.screens.imp.note = "Review the report and press Open GitHub draft when ready.".into(); }
        "open-github" => return open_github(this, cx),
        _ if id.starts_with("proj:") && !locked && this.screens.imp.draft.key.is_empty() => {
            let p = id[5..].to_string(); this.screens.imp.draft.project = p.clone(); this.screens.imp.picked = true;
            ask(this, "improvements.project", json!({"project": p}), Next::Remember, cx); // remembered for the next request
        }
        _ if id.starts_with("wproj:") && !locked => { let d = &mut this.screens.imp.draft; d.writer_project = id[6..].to_string(); d.forget_report(); this.screens.imp.review_open = false; }
        _ => {}
    }
    touch(this, cx); cx.notify();
}

/// Open GitHub's new-issue form with the exact body: the receipt is saved first, and a failed save or open leaves nothing opened.
fn open_github(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let imp = &mut this.screens.imp;
    if imp.busy || imp.draft.opened || imp.draft.issue_body.is_empty() { return }
    /* i18n-ok: drawn through tr */ if !imp.evidence_ok || !imp.draft.has_evidence() { imp.note = "Confirm that the report has a thorough description or attachable evidence image.".into(); cx.notify(); return }
    let url = if imp.url.is_empty() { "https://github.com/BackplaneWorks/Backplane/issues/new".to_string() } else { imp.url.clone() };
    imp.draft.opened = true;
    if let Err(e) = save_now(this) { this.screens.imp.draft.opened = false; return fail(this, e, cx) }
    match open_target(&url) {
        /* i18n-ok: drawn through tr */ Ok(()) => this.screens.imp.note = "GitHub draft opened. Sign in with your GitHub account and press Submit there. Backplane has not submitted an issue.".into(),
        Err(e) => { this.screens.imp.draft.opened = false; let _ = save_now(this); this.screens.imp.note = e; }
    }
    cx.notify();
}

// ---- the e2e harness ---------------------------------------------------------------------------------------------------------------------------

/// Script steps `{"imp": "<op> <rest>"}`: `open`, `text|evidence|details <text>`, `shot <png file>` (use an image), and `read <what>` (a line of state to stderr).
pub fn script_op(this: &mut Backplane, op: &str, cx: &mut Context<Backplane>) {
    let (cmd, rest) = op.split_once(' ').unwrap_or((op, ""));
    match cmd {
        "open" => { if !this.screens.imp.open { open(this, cx) } }
        "text" | "evidence" | "details" => {
            inputs(this, cx);
            let ins = this.screens.imp.inputs.as_ref().map(|i| match cmd { "text" => i.text.clone(), "evidence" => i.evidence.clone(), _ => i.details.clone() });
            if let Some(e) = ins { let t = rest.replace("\\n", "\n"); e.update(cx, |i, cx| i.set(t, cx)); }
        }
        // a picture of this window only (the same ffmpeg x11grab as the panel's Capture), into $BACKPLANE_GPUI_SHOTS/<name>.png
        "snap" => if let (Some(xid), Some(dir)) = (window_id(this), std::env::var_os("BACKPLANE_GPUI_SHOTS").map(PathBuf::from)) {
            let name: String = rest.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).collect();
            if !name.is_empty() { std::thread::spawn(move || { let _ = capture::shot(xid, &dir.join(format!("{name}.png"))); }); }
        }
        "image" => { let p = PathBuf::from(rest); use_image_file(this, &p, cx) }
        // test hooks for the readiness run: a permission mode for the hand-off thread, and answering the first open ask with its primary button
        "runtime" => { this.screens.imp.runtime = Some(rest.to_string()); this.screens.imp.modes_sent = false; }
        "approve" => {
            let asks = super::asks::parse(&this.data["thread"]["asks"]);
            if let Some(a) = asks.first() { if let Some(b) = a.buttons.iter().find(|b| b.primary).or(a.buttons.first()) {
                let _ = this.tx.try_send(Command::Action("answer".into(), b.value.clone())); this.m("improvements", json!({"approved": a.head, "button": b.label}));
            } }
        }
        "state" => { let d = &this.screens.imp.draft; this.m("improvements", json!({"internal": d.internal, "evidence": d.has_evidence(), "report": d.issue_body.len(), "loaded": d.report.len(), "opened": d.opened, "note": this.screens.imp.note, "busy": this.screens.imp.busy,
            "writer": d.writer.as_ref().map(|w| json!({"created": w.created, "started": w.started, "path": w.path, "video": w.video_path})), "thread": d.thread, "image": !d.image.is_empty(), "recording": !d.recording.is_empty(), "url": this.screens.imp.url.len(), "project": d.project})); }
        _ => {}
    }
    cx.notify();
}

// ---- drawing ----------------------------------------------------------------------------------------------------------------------------------

fn probe(e: Stateful<Div>, id: &str) -> Stateful<Div> { if art::rects_on() { e.relative().child(art::probe_rect(format!("imp:{id}"))) } else { e } }

/// The controls the panel offers now, for the harness (`{"click": "imp:<id>"}`).
pub fn controls(this: &Backplane, out: &mut Vec<CtlEntry>) {
    /* i18n-ok: e2e control labels */ out.push(CtlEntry { id: "ui:Improvements".into(), label: "Improvements".into(), ctl: Ctl::Imp("toggle".into()) });
    let imp = &this.screens.imp;
    /* i18n-ok: e2e control labels */ if imp.rec.is_some() { for (id, l) in [("rec-stop", "Stop recording"), ("rec-cancel", "Cancel recording")] { out.push(CtlEntry { id: format!("imp:{id}"), label: l.into(), ctl: Ctl::Imp(id.into()) }); } }
    if !imp.open || imp.hidden { return }
    for (id, l) in all_controls(imp) { out.push(CtlEntry { id: format!("imp:{id}"), label: l, ctl: Ctl::Imp(id) }); }
}
fn all_controls(imp: &Imp) -> Vec<(String, String)> {
    let d = &imp.draft;
    /* i18n-ok: e2e control labels */ let mut v: Vec<(&str, &str)> = vec![("close", "Close Improvements"), ("dest-issue", "Public GitHub issue"), ("dest-internal", "Internal implementation thread"), ("capture", "Capture again"), ("choose", "Choose image")];
    /* i18n-ok: e2e control labels */ if d.internal { v.push(("primary", "Send")); } else { v.extend([("chk-shot", "Select a screenshot"), ("chk-diag", "Include diagnostics"), ("chk-details", "Include details"), ("write", "Write report with agent"), ("load", "Load agent report"), ("template", "Use report template"),
        /* i18n-ok: e2e control labels */ ("primary", "Review issue"), ("copy", "Copy issue text"), ("save", "Save report"), ("chk-evidence", "Confirm evidence"), ("open-github", "Open GitHub draft")]); }
    /* i18n-ok: e2e control labels */ if d.writer.is_some() { v.push(("open-writer", "Open report-writing thread")); }
    /* i18n-ok: e2e control labels */ if capture::unavailable().is_none() { v.push(("record", "Record screen")); }
    /* i18n-ok: e2e control labels */ if !d.recording.is_empty() { v.extend([("rec-play", "Play recording"), ("chk-rec", "Attach recording"), ("rec-discard", "Discard recording")]); }
    let mut out: Vec<(String, String)> = v.into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    /* i18n-ok: e2e control labels */ if d.internal { out.extend(imp.picks.iter().take(PICK_ROWS).map(|p| (format!("proj:{}", s(p, "id")), format!("Project {}", s(p, "title"))))); } else { out.extend(imp.projects.iter().map(|p| (format!("wproj:{}", p.id), format!("Project {}", p.title)))); }
    out
}

fn button(cx: &mut Context<Backplane>, light: bool, id: &str, label: &str, enabled: bool, primary: bool, on: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let key = id.to_string();
    let d = probe(div().id(SharedString::from(format!("imp-{id}"))), id).px_3().py_1().text_size(px(12.)).border_1();
    let d = if primary { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else if on { d.bg(c("raise")).text_color(c("text")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).text_color(c("text")) };
    if enabled { d.cursor_pointer().hover(|s| s.opacity(0.85)).child(label.to_string()).on_click(cx.listener(move |this, _, _, cx| act(this, &key, cx))).into_any_element() } else { d.opacity(0.45).child(label.to_string()).into_any_element() }
}
fn check(cx: &mut Context<Backplane>, light: bool, id: &str, label: &str, on: bool, enabled: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let key = id.to_string();
    let d = probe(div().id(SharedString::from(format!("imp-{id}"))), id).flex().gap_2().items_start().text_size(px(12.)).child(div().flex_shrink_0().text_color(c(if on { "accent" } else { "dim" })).child(if on { "[x]" } else { "[ ]" })).child(div().flex_1().child(label.to_string()));
    if enabled { d.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| act(this, &key, cx))).into_any_element() } else { d.opacity(0.55).into_any_element() }
}
fn field(light: bool, label: &str, input: &Entity<Composer>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().flex().flex_col().gap_1().child(div().text_size(px(12.)).text_color(c("dim")).child(label.to_string())).child(div().w_full().px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(input.clone())).into_any_element()
}
fn para(light: bool, text: &str) -> AnyElement { div().text_size(px(11.)).text_color(theme::color(light, "dim")).child(text.to_string()).into_any_element() }

/// The recording indicator: visible whenever a recording runs, whatever else is on screen.
pub fn indicator(cx: &mut Context<Backplane>, light: bool, imp: &Imp) -> Option<AnyElement> {
    imp.rec.as_ref()?;
    let c = |n: &str| theme::color(light, n);
    Some(div().absolute().top_0().left_0().right_0().h(px(32.)).bg(c("bad")).text_color(c("on-accent")).flex().items_center().gap_3().px_3().child(div().flex_1().child(imp.rec_text.clone()))
        .child(button(cx, light, "rec-stop", &trs("Stop recording"), true, false, false)).child(button(cx, light, "rec-cancel", &trs("Cancel recording"), true, false, false)).into_any_element())
}

pub fn view(cx: &mut Context<Backplane>, light: bool, imp: &Imp) -> Option<AnyElement> {
    if !imp.open || imp.hidden { return None }
    let c = |n: &str| theme::color(light, n);
    let d = &imp.draft; let ins = imp.inputs.as_ref()?;
    let locked = imp.locked(); let public = !d.internal;
    let mut col = div().flex().flex_col().flex_shrink_0().gap_3().w_full().max_w(px(680.)).px_4().py_3();
    col = col.child(div().flex().gap_2().child(button(cx, light, "dest-issue", &trs("Public GitHub issue"), !locked && d.key.is_empty(), false, public)).child(button(cx, light, "dest-internal", &trs("Internal implementation thread"), !locked && d.key.is_empty(), false, !public)));
    col = col.child(field(light, &trs("What would you like to improve?"), &ins.text));
    if !public {
        // the project it goes to: the hub's search over every machine's projects, the chosen one marked (and named when the search hides it)
        let free = !locked && d.key.is_empty();
        /* i18n-ok: a title and a machine name */ let label = |p: &Value| { let short = s(p, "short"); if short.is_empty() { s(p, "title") } else { format!("{} · {short}", s(p, "title")) } };
        let rows: Vec<AnyElement> = imp.picks.iter().take(PICK_ROWS).map(|p| button(cx, light, &format!("proj:{}", s(p, "id")), &label(p), free, false, s(p, "id") == d.project)).collect();
        let shown = imp.picks.iter().take(PICK_ROWS).any(|p| s(p, "id") == d.project);
        let current = chosen(imp).filter(|_| !shown).map(|p| div().text_size(px(12.)).child(tf("gpui.imp.project.chosen", &[("title", p.title.into())])));
        col = col.child(div().flex().flex_col().gap_1().child(div().text_size(px(12.)).text_color(c("dim")).child(trs("Send to project")))
            .children(free.then(|| div().w_full().px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(ins.find.clone()))).child(div().flex().flex_wrap().gap_2().children(rows)).children(current)
            .children((imp.picks.is_empty() && imp.pick_seq > 0).then(|| para(light, &trs("No project matches.")))));
    } else {
        col = col.child(para(light, &trs("Public on BackplaneWorks/Backplane, attributed to your GitHub account. Sign in on GitHub and press Submit there. Review your text and images for private information.")));
        col = col.child(field(light, &trs("Behavior evidence"), &ins.evidence));
        col = col.child(check(cx, light, "chk-shot", &trs("Select a screenshot (manual attachment on GitHub)"), d.inc_shot, !locked));
        col = col.child(check(cx, light, "chk-diag", &trs("Include sanitized diagnostics in the public issue"), d.inc_diag, !locked));
        col = col.child(check(cx, light, "chk-details", &trs("Include additional details I enter"), d.inc_details, !locked));
        if d.inc_details { col = col.child(field(light, &trs("Additional public details"), &ins.details)); }
        if d.inc_diag && !imp.diag.is_empty() { col = col.child(div().flex_shrink_0().h(px(120.)).overflow_hidden().p_2().border_1().border_color(c("line")).bg(c("view")).text_size(px(11.)).child(imp.diag.clone())); }
        let writers: Vec<&Project> = imp.projects.iter().filter(|p| p.id != "scratch" && !p.root.is_empty()).collect();
        col = col.child(div().flex().flex_col().gap_1().child(div().text_size(px(12.)).text_color(c("dim")).child(trs("Report agent project"))).child(div().flex().flex_wrap().gap_2().children(writers.iter().map(|p| button(cx, light, &format!("wproj:{}", p.id), &p.title, !locked, false, p.id == d.writer_project)).collect::<Vec<_>>())));
        col = col.child(div().flex().flex_wrap().gap_2().child(button(cx, light, "write", &trs("Write report with agent"), !locked && !d.writer.as_ref().map_or(false, |w| w.started), false, false))
            .children(d.writer.as_ref().map(|_| button(cx, light, "load", &trs("Load agent report"), !locked, false, false))).children(d.writer.as_ref().map(|_| button(cx, light, "open-writer", &trs("Open report-writing thread"), !locked, false, false)))
            .child(button(cx, light, "template", &trs("Use report template"), !locked, false, false)));
        if imp.review_open {
            col = col.child(field(light, &trs("Editable public issue body"), &ins.review));
        }
        if imp.review_open && imp.previewed {
            col = col.child(div().flex().flex_wrap().gap_2().child(button(cx, light, "copy", &trs("Copy issue text"), true, false, false)).child(button(cx, light, "save", &trs("Save report"), true, false, false))
                .children((d.inc_shot && !d.image.is_empty()).then(|| button(cx, light, "save-image", &trs("Save selected image"), true, false, false)))
                .children(d.opened.then(|| button(cx, light, "allow-again", &trs("Allow another GitHub draft"), !imp.busy, false, false))));
            col = col.child(check(cx, light, "chk-evidence", &trs("My report has a thorough behavior description, or I will attach the selected image or recording on GitHub."), imp.evidence_ok, !imp.busy));
            col = col.child(button(cx, light, "open-github", &trs("Open GitHub draft"), !imp.busy && !d.opened && imp.evidence_ok, true, false));
        }
    }
    // the screenshot
    if public && !d.inc_shot { /* no image area until one is selected */ } else {
        col = col.child(div().flex().flex_col().gap_2().child(div().flex().gap_2().child(button(cx, light, "capture", &trs("Capture again"), !locked, false, false)).child(button(cx, light, "choose", &trs("Choose image"), !locked, false, false)))
            .children(imp.shot.clone().map(|i| div().max_w(px(480.)).max_h(px(300.)).border_1().border_color(c("line")).child(img(ImageSource::Image(i)).max_w(px(480.)).max_h(px(300.)).object_fit(ObjectFit::Contain)))));
    }
    // the recording
    match capture::unavailable() {
        Some(why) => col = col.child(div().flex().flex_col().gap_1().child(button(cx, light, "record", &trs("Record screen"), false, false, false)).child(para(light, &trs(why)))),
        None => {
            col = col.child(div().flex().flex_col().gap_2().child(button(cx, light, "record", &trs("Record screen"), !locked && d.key.is_empty(), false, false))
                .child(para(light, &trs("Video of this window only, no audio. Up to 2 minutes and 9 MiB. Private copies expire after 24 hours. A recording may show private messages, credentials or other private information; it cannot be redacted.")))
                .children((!d.recording.is_empty()).then(|| div().flex().flex_col().gap_2().child(div().text_size(px(12.)).child(tf("gpui.imp.rec.saved", &[("secs", (d.recording_ms / 1000).into())])))
                    .child(div().flex().gap_2().child(button(cx, light, "rec-play", &trs("Play recording"), true, false, false)).child(button(cx, light, "rec-discard", &trs("Discard recording"), !locked && d.key.is_empty(), false, false)))
                    .child(check(cx, light, "chk-rec", &if public { trs("Select this recording for the public report and agent. Play it first.") } else { trs("Attach this recording to the local agent thread. Play it first.") }, d.inc_rec, !locked && d.rec_reviewed)))));
        }
    }
    col = col.child(div().text_size(px(12.)).text_color(c("dim")).child(if public { trs("OS family and app release are included. Images and recordings stay local until you send them to the agent or attach them yourself on GitHub. Ctrl+Enter reviews the issue.") } else if d.image.is_empty() && !(d.inc_rec && !d.recording.is_empty()) { trs("Choose an image or preview and select a recording before sending.") } else if d.project.is_empty() { trs("Choose a project before sending.") } else if is_far(&d.project) { trs("The thread runs on the other machine. Images and recordings stay on this computer. Ctrl+Enter sends this request.") } else { trs("Ctrl+Enter sends this request.") }));
    // the note and the primary button stay pinned below the scrolling body, visible however small the window
    let foot = div().flex_shrink_0().px_4().py_2().border_t_1().border_color(c("line")).flex().items_center().gap_3()
        .child(div().id("imp-note").flex_1().min_w_0().text_size(px(12.)).child(note_text(&imp.note)))
        .child(div().flex_shrink_0().child(button(cx, light, "primary", &trs(d.send_label(imp.busy)), !imp.busy, true, false)));
    // a floating window above the app: centred, bordered, shadowed, with the page dimmed behind it (a click on the dim leaves the panel as it is)
    Some(div().id("improvements-float").absolute().inset_0().occlude().bg(gpui::hsla(0., 0., 0., 0.35)).flex().items_center().justify_center().p_6()
      .child(div().id("improvements").w(px(760.)).max_w_full().max_h_full().h(px(760.)).overflow_hidden().bg(c("bg")).border_1().border_color(c("line")).shadow_lg().flex().flex_col()
        /* i18n-ok: a shortcut */ .child(div().h(px(36.)).flex_shrink_0().px_3().flex().items_center().gap_3().border_b_1().border_color(c("line")).child(div().flex_1().font_weight(FontWeight::BOLD).child(trs("Improvements"))).child(div().text_color(c("faint")).text_size(px(11.)).child("Ctrl+Alt+I"))
            .child(button(cx, light, "close", &trs("Close"), true, false, false)))
        .child(div().id("improvements-body").flex_1().min_h_0().overflow_y_scroll().flex().justify_center().items_start().child(col))
        .child(foot)).into_any_element())
}
