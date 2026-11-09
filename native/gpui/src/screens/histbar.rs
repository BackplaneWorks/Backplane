//! The design history bar under a board or a schematic, as 0.14 had it (web view.bend's View.hist, client.bend's Hs.bar): first / previous /
//! play / next / last, where the viewer is ("Live · 21 of 27 changes", "Step 3 of 26"), Highlight edits, Compare and Live; a scrubber with a
//! tick a step and the live file last; what the step followed in the thread ("Saved Nov 6 14:08 · After Bash · ..."), a press there going to
//! that point of the thread. Compare swaps it for the comparison's bar: its two sides, Close, the legend and Old / Diff / New; its menu lists the
//! newest steps and the project's versions (HEAD, branches, tags, the thread's turns). Everything is the hub's: `history.steps` gives the
//! file's track with its words and version paths (kicad/histsteps.rs), `history.refs` the versions, `kicad.watch` on a version path
//! (`git\t...`, `diff\n...`) draws one. This module keeps what is on show (`Cmp`, `Hist`, in the canvas's PlotView) and draws the bar.
use super::plotview::PlotView;
use crate::{art, hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

/// A step of the file's track as the hub words it: its commits and time, the version paths showing it (`old` the step before, "" for a first;
/// `new` the step), and its words for the scrubber's tick (`tip`), the bar's title and the line under it (`detail`), and the thread entry it followed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step { pub c: String, pub p: String, pub t: u64, pub old: String, pub new: String, pub label: String, pub tip: String, pub title: String, pub detail: String, pub entry: String, pub id: String, /** the thread's row it points at (a message, or the folded run it is in) */ pub row: String }
/// The track of the file on show and the bar's own words while the live file shows (`live`, `note`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Steps { pub git: String, pub top: String, pub pcb: String, pub sch: String, pub file: String, pub kind: String, pub live: String, pub note: String, pub steps: Vec<Step> }
pub fn steps_of(v: &Value) -> Option<Steps> {
    // the hub refused (a far machine's hub that has no history.steps, a thread gone): the bar says so instead of loading for ever
    if v.is_null() { return Some(Steps { live: "Live".into(), note: "The design history could not be read here.".into(), ..Steps::default() }) }
    if !v.is_object() { return None }
    let g = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let (git, top, file) = (g("git"), g("top"), g("file"));
    let steps: Vec<Step> = v["steps"].as_array().into_iter().flatten().enumerate().filter_map(|(i, s)| {
        let w = |k: &str| s[k].as_str().unwrap_or("").to_string();
        let (c, p) = (s["c"].as_str()?.to_string(), w("p"));
        // a hub from before the words were its (a far machine on an older release): the paths from the project's git, a plain label
        let (old, new) = if s.get("new").is_some() { (w("old"), w("new")) } else { (version(&git, &top, &p, &file), version(&git, &top, &c, &file)) };
        let label = if w("label").is_empty() { format!("Step {}", i + 1) } else { w("label") };
        Some(Step { t: s["t"].as_u64().unwrap_or(0), title: if w("title").is_empty() { label.clone() } else { w("title") }, tip: if w("tip").is_empty() { label.clone() } else { w("tip") },
            detail: w("detail"), entry: w("entry"), id: w("id"), row: w("row"), c, p, old, new, label })
    }).collect();
    let live = if g("live").is_empty() { if steps.is_empty() { "Live".to_string() } else { format!("Live · {} steps", steps.len()) } } else { g("live") };
    Some(Steps { pcb: g("pcb"), sch: g("sch"), kind: g("kind"), note: g("note"), live, git, top, file, steps })
}

/// core/hist's version path of `file` in `commit`: "git\t<gitdir>\t<commit>\t<len top>\t<top>/<rel>" ("" when the file is not under the top folder).
pub fn version(git: &str, top: &str, commit: &str, file: &str) -> String {
    match file.strip_prefix(&format!("{top}/")) { Some(rel) if !commit.is_empty() && !git.is_empty() => format!("git\t{git}\t{commit}\t{}\t{top}/{rel}", top.chars().count()), _ => String::new() }
}

/// What a comparison shows: `old` and `new` are sources (a version path, or the live file's), `mode` 0 Old, 1 Diff, 2 New, `bits` the layers its marks keep to (0 every layer).
#[derive(Clone, Debug, PartialEq)]
pub struct Cmp { pub base: String, pub label: String, pub old: String, pub new: String, pub mode: u8, pub bits: u32, /** the new side's name ("" the working copy) */ pub to: String }
/// The source watched for the comparison on show. Where there is no old version (a first step), what there is shows as it is.
pub fn shown(c: &Cmp) -> String {
    if c.old.is_empty() { return c.new.clone() }
    match c.mode { 0 => c.old.clone(), 2 => c.new.clone(), _ if c.bits != 0 => format!("diff\n{}\n{}\n{}", c.old, c.new, c.bits), _ => format!("diff\n{}\n{}", c.old, c.new) }
}

/// The bar's state. `at`: the step on show (an index into the track), none for the live file; `diff`: Highlight edits (a step shows as its
/// edits on the step before, not as the file it left); `asked`: the thread and viewer key the track was asked for; `sig`: the thread's step rows
/// last seen in a picture (a new one asks again).
#[derive(Default, Clone)]
pub struct Hist { pub steps: Option<Steps>, pub refs: Option<super::plot::Refs>, pub at: Option<usize>, pub play: u64, pub playing: bool, pub diff: bool, pub asked: String, pub thread: String, pub sig: u64,
    /** a step row pressed in the thread: its act id and the kinds still to look in, shown once its track comes */ pub pending: Option<(String, Vec<String>)>,
    /** the compare menu's field for a commit, branch or tag, and what the hub said of the last one typed */ pub ref_input: Option<Entity<super::composer::Composer>>, pub ref_err: String, /** the side the menu picks: 0 old, 1 new */ pub side: u8 }

fn open_file(app: &Backplane) -> Option<(String, String, String, String)> { // (kind, key, root, the file shown)
    let v = &app.data["thread"]["viewer"];
    let (open, key) = (v["open"].as_str().unwrap_or("").to_string(), v["key"].as_str().unwrap_or("").to_string());
    if !matches!(open.as_str(), "board" | "schematic") { return None }
    let mut it = key.splitn(3, '|'); let (_, root, own) = (it.next(), it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string());
    Some((open, key, root, own))
}
fn file_of(kind: &str, own: &str, pcb: &str, sch: &str) -> String { if !own.is_empty() { own.to_string() } else if kind == "board" { pcb.to_string() } else { sch.to_string() } }
pub fn ask_refs(app: &mut Backplane) { let t = app.selected.clone(); if t.is_empty() { return } let _ = app.tx.try_send(Command::Rpc("history.refs".into(), super::settings::Secret(json!({"thread": t})), "refs")); }
/// Ask for the track of the file on show (the hub picks the project's board or schematic when the key names none).
pub fn ask_steps(app: &mut Backplane) {
    let t = app.selected.clone(); if t.is_empty() { return }
    let (kind, own) = open_file(app).map(|(k, _, _, own)| (k, own)).unwrap_or_default();
    let _ = app.tx.try_send(Command::Rpc("history.steps".into(), super::settings::Secret(json!({"thread": t, "kind": kind, "file": own})), "steps"));
}
/// The track is the one for this thread's file on show: asked again when the viewer shows another file.
fn ensure(app: &mut Backplane) {
    let Some((_, key, _, _)) = open_file(app) else { return };
    let want = format!("{}\n{key}", app.selected);
    if app.screens.plotview.hist.asked == want { return }
    let h = &mut app.screens.plotview.hist; h.asked = want; h.steps = None; h.at = None; h.playing = false; h.play += 1;
    ask_steps(app);
}
/// A picture came: another thread starts its bar over; a new design step in this one (a step row, or a folded run's count of them) asks for the track again.
pub fn picture(app: &mut Backplane) {
    let th = super::attach::raw_id(app.data["thread"]["id"].as_str().unwrap_or("")).to_string();
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for e in app.data["thread"]["entries"].as_array().into_iter().flatten() {
        match e["kind"].as_str() { Some("step") => e["id"].as_str().hash(&mut h), Some("fold") => { e["id"].as_str().hash(&mut h); e["label"].as_str().hash(&mut h); } _ => {} }
    }
    let sig = h.finish();
    let hist = &mut app.screens.plotview.hist;
    if hist.thread != th { *hist = Hist { thread: th, sig, play: hist.play + 1, ..Hist::default() }; return }
    if hist.sig == sig { return }
    hist.sig = sig;
    if !hist.asked.is_empty() && open_file(app).is_some() { ask_steps(app) }
}

/// Watch `path` for the open file ("" the file itself, or its sheet): the hub draws it as a plot with this key suffix.
fn watch(app: &mut Backplane, path: &str) {
    let Some((kind, _, root, _)) = open_file(app) else { return };
    let _ = app.tx.try_send(Command::Rpc("kicad.watch".into(), super::settings::Secret(json!({"kind": kind, "root": root, "path": path, "tree": ""})), "cmp"));
}
/// Show the comparison (None: put it away and watch the file itself again).
pub fn set(app: &mut Backplane, c: Option<Cmp>) {
    let Some((kind, _, _, own)) = open_file(app) else { return };
    let bits = crate::viewers::layers::bits(&app.data["thread"]["viewer"]);
    let c = c.map(|c| Cmp { bits, ..c });
    let path = match &c { Some(c) => shown(c), None => if kind == "board" { String::new() } else { own } };
    // (the camera stays: versions of one file share its family, so the canvas keeps its view from step to step, as 0.14's merged scene did)
    app.screens.plotview.compare = c; app.screens.plotview.menu = 0;
    if app.screens.plotview.compare.is_none() { app.screens.plotview.hist.at = None; app.screens.plotview.hist.playing = false; }
    watch(app, &path);
}
/// The layer focus moved: a comparison on show keeps its marks to the new active layer (the web's rv-bits). The camera stays where it is.
pub fn follow_focus(app: &mut Backplane) {
    let Some((_, key, _, _)) = open_file(app) else { return };
    let bits = crate::viewers::layers::bits(&app.data["thread"]["viewer"]);
    let Some(c) = app.screens.plotview.compare.as_mut().filter(|c| c.base == key && c.bits != bits) else { return };
    let before = shown(c); c.bits = bits; let path = shown(c);
    if path != before { watch(app, &path); }
}
/// Compare the open file with `commit`: Diff first (old = the commit's file, new = the live one).
pub fn compare(app: &mut Backplane, commit: Option<(&str, &str)>) {
    let Some((kind, _, _, own)) = open_file(app) else { return };
    let Some((commit, label)) = commit else { return set(app, None) };
    let Some(r) = app.screens.plotview.hist.refs.clone() else { return };
    let file = file_of(&kind, &own, &r.pcb, &r.sch);
    let v = version(&r.git, &r.top, commit, &file); if v.is_empty() { return }
    let side = app.screens.plotview.hist.side; pick(app, side, v, label.to_string());
}
/// The file as step `i` left it, on the menu's side (the compare menu's step rows).
pub fn compare_step(app: &mut Backplane, i: usize) {
    let Some(s) = app.screens.plotview.hist.steps.as_ref().and_then(|h| h.steps.get(i).cloned()) else { return }; if s.new.is_empty() { return }
    let side = app.screens.plotview.hist.side; pick(app, side, s.new, s.label);
}
/// The live file on show (the working copy) and the viewer's key, when a board or schematic shows.
fn live(app: &Backplane) -> Option<(String, String)> {
    let (kind, key, _, own) = open_file(app)?;
    let (pcb, sch) = app.screens.plotview.hist.steps.as_ref().map(|h| (h.pcb.clone(), h.sch.clone())).or_else(|| app.screens.plotview.hist.refs.as_ref().map(|r| (r.pcb.clone(), r.sch.clone()))).unwrap_or_default();
    Some((key, file_of(&kind, &own, &pcb, &sch)))
}
/// A version picked for a side (0 old, 1 new; `path` a version path, or the live file for the working copy): the comparison on show keeps its
/// other side; with none on show the new side is the working copy. Diff first.
pub fn pick(app: &mut Backplane, side: u8, path: String, label: String) {
    let Some((key, file)) = live(app) else { return };
    let cur = app.screens.plotview.compare.clone().filter(|c| c.base == key && app.screens.plotview.hist.at.is_none());
    let (old, new, name, to) = match (side, cur) {
        (0, Some(c)) => (path, c.new, label, c.to),
        (0, None) => (path, file, label, String::new()),
        (_, Some(c)) => (c.old, path, c.label, if label.is_empty() { String::new() } else { label }),
        (_, None) => return,
    };
    if old.is_empty() || new.is_empty() { return }
    app.screens.plotview.hist.at = None; app.screens.plotview.hist.playing = false;
    set(app, Some(Cmp { base: key, label: name, old, new, mode: 1, bits: 0, to }));
}
/// The step `i` of the track on show (None: the live file): the file as the step left it, or with Highlight edits its edits on the step before.
pub fn select(app: &mut Backplane, i: Option<usize>) {
    let Some((_, key, _, _)) = open_file(app) else { return };
    let Some(h) = app.screens.plotview.hist.steps.clone() else { return };
    let Some(i) = i.filter(|i| *i < h.steps.len()) else { return set(app, None) };
    let s = &h.steps[i]; if s.new.is_empty() { return }
    let mode = if app.screens.plotview.hist.diff { 1 } else { 2 };
    app.screens.plotview.hist.at = Some(i);
    set(app, Some(Cmp { base: key, label: s.label.clone(), old: s.old.clone(), new: s.new.clone(), mode, bits: 0 , to: String::new() }));
    warm(app, &h, i, mode);
}
/// Hs.warm: the steps either side of the one on show are plotted ahead into the hub's kept versions (kicad.warm), so the next ‹ or › finds its
/// board ready (0.14 measured 1.1 s for a new step's board on neurband, 0.4 s for a kept one).
fn warm(app: &mut Backplane, h: &Steps, i: usize, mode: u8) {
    let Some((kind, _, root, _)) = open_file(app) else { return };
    let bits = crate::viewers::layers::bits(&app.data["thread"]["viewer"]);
    let (before, after) = near(h, i, mode, bits);
    if before.is_empty() && after.is_empty() { return }
    let _ = app.tx.try_send(Command::Rpc("kicad.warm".into(), super::settings::Secret(json!({"kind": kind, "root": root, "before": before, "after": after})), "warm"));
}
/// The sources the steps before and after `i` would watch (as `select` shows them), "" past either end.
pub fn near(h: &Steps, i: usize, mode: u8, bits: u32) -> (String, String) {
    let at = |j: Option<usize>| j.and_then(|j| h.steps.get(j)).filter(|s| !s.new.is_empty()).map_or(String::new(), |s| shown(&Cmp { base: String::new(), label: String::new(), old: s.old.clone(), new: s.new.clone(), mode, bits, to: String::new() }));
    (at(i.checked_sub(1)), at(Some(i + 1)))
}
/// Old / Diff / New of the comparison on show.
pub fn mode(app: &mut Backplane, m: u8) { if let Some(mut c) = app.screens.plotview.compare.clone() { c.mode = m; set(app, Some(c)); } }
/// Highlight edits: a step shows as what it changed (removed, changed, added) on the step before; from the live file it shows the newest step.
pub fn highlight(app: &mut Backplane) {
    let n = app.screens.plotview.hist.steps.as_ref().map_or(0, |h| h.steps.len()); if n == 0 { return }
    let h = &mut app.screens.plotview.hist; h.diff = !h.diff;
    let at = h.at; let on = h.diff;
    match at { Some(i) => select(app, Some(i)), None if on => select(app, Some(n - 1)), None => {} }
}
/// Hist.back / Hist.fwd: from the live file back is the newest step and forward stays live; past the newest step forward is live.
pub fn back(at: Option<usize>, n: usize) -> Option<usize> { if n == 0 { return None } match at { None => Some(n - 1), Some(i) => Some(i.saturating_sub(1)) } }
pub fn fwd(at: Option<usize>, n: usize) -> Option<usize> { match at { Some(i) if i + 1 < n => Some(i + 1), _ => None } }
fn stop(app: &mut Backplane) { app.screens.plotview.hist.playing = false; app.screens.plotview.hist.play += 1; }
/// Go to the point of the thread the step on show followed (when the rows on screen have it).
pub fn jump(app: &mut Backplane) {
    let Some(s) = app.screens.plotview.hist.at.and_then(|i| app.screens.plotview.hist.steps.as_ref().and_then(|h| h.steps.get(i).cloned())) else { return };
    let held = |app: &Backplane, id: &str| !id.is_empty() && app.data["thread"]["entries"].as_array().is_some_and(|r| r.iter().any(|e| e["id"].as_str() == Some(id)));
    // (only among the rows held: the timeline's paging lands a row read later in another place)
    for id in [s.row.as_str(), s.entry.as_str()] { if held(app, id) { return app.timeline_goto(id) } }
}

/// Step playback: from the step after the one on show (the first from live) every 1.2 s to the newest, then back to live. A press while it plays stops it.
pub fn play(app: &mut Backplane, cx: &mut Context<Backplane>) {
    if app.screens.plotview.hist.playing { return stop(app) }
    let n = app.screens.plotview.hist.steps.as_ref().map_or(0, |h| h.steps.len()); if n == 0 { return }
    app.screens.plotview.hist.playing = true; app.screens.plotview.hist.play += 1; let token = app.screens.plotview.hist.play;
    let first = match app.screens.plotview.hist.at { Some(i) if i + 1 < n => i + 1, _ => 0 };
    select(app, Some(first)); app.screens.plotview.hist.playing = true;
    cx.spawn(async move |weak, cx| loop {
        cx.background_executor().timer(std::time::Duration::from_millis(1200)).await;
        let go = weak.update(cx, |app, cx| {
            if app.screens.plotview.hist.play != token || !app.screens.plotview.hist.playing { return false }
            let n = app.screens.plotview.hist.steps.as_ref().map_or(0, |h| h.steps.len());
            match app.screens.plotview.hist.at { Some(i) if i + 1 < n => { select(app, Some(i + 1)); app.screens.plotview.hist.playing = true; } _ => { app.screens.plotview.hist.playing = false; select(app, None); } }
            cx.notify(); app.screens.plotview.hist.playing
        }).unwrap_or(false);
        if !go { break }
    }).detach();
}

/// The kinds of file a step row's words name, the board first ("Design change · heater.kicad_pcb, controller.kicad_sch (new)"); a removed file shows nothing.
pub fn row_kinds(text: &str) -> Vec<String> {
    let names: Vec<&str> = text.split(" · ").skip(1).flat_map(|f| f.split(", ")).filter(|f| !f.ends_with(" (removed)")).collect();
    [("board", ".kicad_pcb"), ("schematic", ".kicad_sch")].iter().filter(|(_, ext)| names.iter().any(|n| n.trim_end_matches(" (new)").ends_with(ext))).map(|(k, _)| k.to_string()).collect()
}
/// A step row in the thread pressed (0.14's hist-show): the viewer opens on the board (or the schematic) the step changed, at that step.
pub fn show_step(app: &mut Backplane, id: &str, text: &str) {
    let mut kinds = row_kinds(text); if kinds.is_empty() { return }
    let kind = kinds.remove(0);
    app.screens.plotview.hist.pending = Some((id.to_string(), kinds));
    let open = app.data["thread"]["viewer"]["open"].as_str().unwrap_or("").to_string();
    if open == kind && app.screens.plotview.hist.steps.is_some() { return arrived(app) }
    let _ = app.tx.try_send(super::viewer::view_kind(&kind));
}
/// A track came: a step row pressed is shown when it is on it; else the next kind it changed is opened, or nothing.
pub fn arrived(app: &mut Backplane) {
    let Some((id, mut rest)) = app.screens.plotview.hist.pending.clone() else { return };
    // (an answer for the kind shown before the press is not the one to look in)
    let open = app.data["thread"]["viewer"]["open"].as_str().unwrap_or("").to_string();
    if app.screens.plotview.hist.steps.as_ref().is_some_and(|h| !h.kind.is_empty() && h.kind != open) { return }
    let i = app.screens.plotview.hist.steps.as_ref().and_then(|h| h.steps.iter().position(|s| s.id == id));
    match i {
        Some(i) => { app.screens.plotview.hist.pending = None; stop(app); select(app, Some(i)) }
        None if !rest.is_empty() => { let k = rest.remove(0); app.screens.plotview.hist.pending = Some((id, rest)); let _ = app.tx.try_send(super::viewer::view_kind(&k)); }
        None => app.screens.plotview.hist.pending = None,
    }
}
/// A design step's row in the thread: its words with the history mark; a press shows that change in the viewer.
pub fn step_row(cx: &mut Context<Backplane>, light: bool, e: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (id, text) = (e["id"].as_str().unwrap_or("").to_string(), e["text"].as_str().unwrap_or("").to_string());
    let value = e["value"].as_str().filter(|v| !v.is_empty()).map_or(id.clone(), str::to_string);
    let can = !row_kinds(&text).is_empty();
    let d = div().id(SharedString::from(format!("e-{id}"))).flex().items_center().gap_2().py_1().font(art::mono_font()).text_size(px(12.)).text_color(c("dim"))
        .child(art::icon("history", 14., theme::color(light, "dim"))).child(div().min_w_0().child(text.clone()));
    if !can { return d.into_any_element() }
    d.cursor_pointer().hover(|s| s.text_color(c("text"))).tooltip(crate::keys::tip("Show this change in the viewer", "", light))
        .on_click(cx.listener(move |this, _, _, cx| { show_step(this, &value, &text); cx.notify(); })).into_any_element()
}

/// The compare menu opens or shuts (asking for the versions, and making its field the first time).
pub fn toggle_menu(app: &mut Backplane, side: u8, cx: &mut Context<Backplane>) {
    let m = if app.screens.plotview.menu == 1 && app.screens.plotview.hist.side == side { 0 } else { 1 }; app.screens.plotview.menu = m; app.screens.plotview.hist.side = side;
    if m != 1 { return }
    ask_refs(app); app.screens.plotview.hist.ref_err.clear();
    if app.screens.plotview.hist.ref_input.is_none() {
        let input = cx.new(|cx| super::composer::Composer::with_context(cx, "AskInput"));
        input.update(cx, |i, _| i.set_placeholder("Commit, branch or tag"));
        cx.subscribe(&input, |this, input, ev: &super::composer::ComposerEvent, cx| if matches!(ev, super::composer::ComposerEvent::Submit) {
            let text = input.read(cx).content.trim().to_string(); if text.is_empty() { return }
            let t = this.selected.clone(); if t.is_empty() { return }
            this.screens.plotview.hist.ref_err.clear();
            let _ = this.tx.try_send(Command::Rpc("history.resolve".into(), super::settings::Secret(json!({"thread": t, "side": if this.screens.plotview.hist.side == 1 { "b" } else { "a" }, "ref": text})), "resolve")); cx.notify();
        }).detach();
        app.screens.plotview.hist.ref_input = Some(input);
    }
}
/// The hub's answer to a typed version (RvJob.resolve): compared with the file on show, or why not, under the field.
pub fn resolved(app: &mut Backplane, r: &Value, cx: &mut Context<Backplane>) {
    let v = &r["resolved"];
    let (commit, git, top, name) = (v["commit"].as_str().unwrap_or(""), v["git"].as_str().unwrap_or(""), v["top"].as_str().unwrap_or(""), v["ref"].as_str().unwrap_or(""));
    let Some((_, file)) = live(app) else { return };
    let path = version(git, top, commit, &file);
    if commit.is_empty() || path.is_empty() {
        let why = r["error"].as_str().filter(|w| !w.is_empty()).map_or_else(|| if commit.is_empty() { "No commit, branch or tag by that name here".to_string() } else { "That version has no copy of this file".to_string() }, |w| w.lines().next().unwrap_or(w).to_string());
        app.screens.plotview.hist.ref_err = why; cx.notify(); return }
    if let Some(i) = app.screens.plotview.hist.ref_input.clone() { i.update(cx, |i, cx| i.set(String::new(), cx)); }
    app.screens.plotview.hist.ref_err.clear();
    pick(app, if v["side"] == "b" { 1 } else { 0 }, path, name.to_string());
}

/// Whether the frame on hand and the one asked for are both the viewer's file (`key`) or a version or comparison of it, so the one on hand
/// may stay on the canvas while the other loads (never another file's or another project's).
pub fn same_file(key: &str, have: &str, want: &str) -> bool {
    let of = |k: &str| k.strip_prefix(key).is_some_and(|r| r.is_empty() || r.starts_with("git\t") || r.starts_with("diff\n"));
    !key.is_empty() && of(have) && of(want)
}
/// Escape shuts the compare menu first (0.14's scrim took it).
pub fn escape(app: &mut Backplane) -> bool { if app.screens.plotview.menu == 0 { return false } app.screens.plotview.menu = 0; true }

/// The menu's rows of versions: (label, what it is, what a press does).
#[derive(Clone, Debug, PartialEq)]
pub enum Pick { Step(usize), Commit(String, String), /** the working copy */ Live }
/// Hs.menu: the newest steps of the track (newest first; older ones are on the scrubber), then the project's versions.
pub fn menu_rows(h: Option<&Steps>, r: Option<&super::plot::Refs>) -> Vec<(String, String, Pick)> {
    let mut out = vec![];
    if let Some(h) = h { for (i, s) in h.steps.iter().enumerate().rev().take(8) { out.push((s.label.clone(), s.detail.clone(), Pick::Step(i))); } }
    if let Some(r) = r {
        for (kind, name, commit) in &r.rows {
            let (label, sub) = match kind.as_str() {
                "turn" => { let n = name.rsplit('/').next().unwrap_or(name); (format!("Turn {n}"), if n == "0" { "this thread, before it began" } else { "this thread, at the end of a turn" }) }
                "head" => (name.clone(), "the commit checked out"), "branch" => (name.clone(), "branch"), "tag" => (name.clone(), "tag"),
                "base" => (format!("Branch point · {name}"), "where this branch left it"), _ => (name.clone(), "commit"),
            };
            out.push((label.clone(), sub.to_string(), Pick::Commit(commit.clone(), label)));
        }
    }
    out
}

fn btn(cx: &mut Context<Backplane>, light: bool, id: &str, label: &str, on: bool, enabled: bool, tip: &str, f: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id.to_string())).px_2().py(px(3.)).text_size(px(12.)).border_1().flex_none()
        .border_color(if on { c("accent") } else { c("line") }).bg(c("raise")).text_color(if !enabled { c("faint") } else if on { c("accent") } else { c("text") }).child(label.to_string());
    let d = if tip.is_empty() { d } else { d.tooltip(crate::keys::tip(tip, "", light)) };
    if !enabled { return d.into_any_element() }
    d.cursor_pointer().hover(|s| s.bg(c("panel"))).on_click(cx.listener(move |this, _, _, cx| { f(this, cx); cx.notify(); })).into_any_element()
}
fn icon_btn(cx: &mut Context<Backplane>, light: bool, id: &str, icon: &'static str, tip: &str, tone: &str, enabled: bool, f: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id.to_string())).size(px(26.)).flex().flex_none().items_center().justify_center()
        .child(art::icon(icon, 16., theme::color(light, if enabled { tone } else { "faint" }))).tooltip(crate::keys::tip(tip, "", light));
    if !enabled { return d.into_any_element() }
    d.cursor_pointer().hover(|s| s.bg(c("raise"))).on_click(cx.listener(move |this, _, _, cx| { f(this, cx); cx.notify(); })).into_any_element()
}
/// Removed / Changed / Added in the marks' colours (Look.mark.color).
fn legend(light: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let keys: [(u32, &str); 3] = if light { [(0xCD2F31, "Removed"), (0xB86D00, "Changed"), (0x18794E, "Added")] } else { [(0xE5484D, "Removed"), (0xF5A524, "Changed"), (0x30A46C, "Added")] };
    div().flex().flex_none().items_center().gap(px(14.)).text_size(px(12.)).text_color(c("dim"))
        .children(keys.into_iter().map(|(col, l)| div().flex().items_center().gap(px(6.)).child(div().size(px(10.)).bg(rgb(col))).child(l))).into_any_element()
}

/// `el` (the page while its plot loads, or why it cannot show) with the bar under it when `on`, so a version that will not load can be left.
pub fn under(cx: &mut Context<Backplane>, light: bool, pv: &PlotView, key: &str, on: bool, el: AnyElement) -> AnyElement {
    if !on { return el }
    div().size_full().flex().flex_col().child(div().flex_1().min_h_0().child(el)).child(bar_el(cx, light, pv, key)).into_any_element()
}
/// The hub's reasons a plot is not there, in words ("@bp.file-unread", "@bp.file-missing:<name>"); anything else as it came.
pub fn none_words(why: &str) -> String {
    if why == "@bp.file-unread" { return "This version of the file could not be read.".into() }
    if let Some(n) = why.strip_prefix("@bp.file-missing:") { return format!("{n} is not there: it was moved or deleted.") }
    why.to_string()
}

/// The bar under the board or schematic (`key`: the viewer's key, the file on show). With a comparison from Compare on show it is the
/// comparison's bar, else the history's; the versions menu opens over the canvas above it.
pub fn bar_el(cx: &mut Context<Backplane>, light: bool, pv: &PlotView, key: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let hist = &pv.hist;
    // the track asked for is another thread's or file's: ask for this one (after this frame; the bar shows Loading meanwhile)
    if !hist.asked.ends_with(&format!("\n{key}")) || hist.asked.is_empty() { cx.spawn(async move |weak, cx| { let _ = weak.update(cx, |app, cx| { ensure(app); cx.notify(); }); }).detach(); }
    let n = hist.steps.as_ref().map_or(0, |h| h.steps.len());
    let at = hist.at.filter(|i| *i < n);
    let step = at.and_then(|i| hist.steps.as_ref().map(|h| h.steps[i].clone()));
    let cmp = pv.compare.as_ref().filter(|c| c.base == key);
    let comparing = cmp.is_some() && at.is_none();
    let mut bar = div().id("hist-bar").relative().flex().flex_col().flex_none().gap(px(6.)).px(px(10.)).pt(px(6.)).pb(px(8.)).border_t_1().border_color(c("line")).bg(c("bg")).text_size(px(12.));
    if let (true, Some(cmp)) = (comparing, cmp) {
        // the comparison: its sides (the old one opens the versions menu), Close; then the legend and Old / Diff / New
        let row = div().flex().items_center().gap(px(4.)).min_w_0()
            .child(btn(cx, light, "cmp-side-a", &format!("Old: {}", cmp.label), pv.menu == 1 && hist.side == 0, true, "Pick the old version", |app, cx| toggle_menu(app, 0, cx)))
            .child(btn(cx, light, "cmp-side-b", &format!("New: {}", if cmp.to.is_empty() { "Working copy" } else { cmp.to.as_str() }), pv.menu == 1 && hist.side == 1, true, "Pick the new version", |app, cx| toggle_menu(app, 1, cx)))
            .child(div().flex_1())
            .child({ let ab = format!("{}\n{}", cmp.old, cmp.new); btn(cx, light, "cmp-bom", "BOM", false, true, "The parts that changed between the two versions", move |app, _| { let _ = app.tx.try_send(Command::Action("rv-bom".into(), ab.clone())); }) })
            .child(btn(cx, light, "cmp-off", "Close", false, true, "Back to the file as it is now", |app, _| set(app, None)));
        let mut modes = div().flex().items_center().gap(px(4.)).min_w_0();
        modes = modes.child(if cmp.mode == 1 { legend(light) } else { div().into_any_element() }).child(div().flex_1());
        for (i, l) in ["Old", "Diff", "New"].iter().enumerate() { let m = i as u8; modes = modes.child(btn(cx, light, &format!("cmp-mode-{i}"), l, cmp.mode == m, true, &format!("Show the {} version", l.to_lowercase()), move |app, _| mode(app, m))); }
        bar = bar.child(row).child(modes);
    } else {
        let loading = hist.steps.is_none();
        let title = match (&step, &hist.steps) { (Some(s), _) => s.title.clone(), (None, Some(h)) => h.live.clone(), (None, None) => "Live".to_string() };
        let on = |app: &Backplane| app.screens.plotview.hist.steps.as_ref().map_or(0, |h| h.steps.len());
        let row = div().flex().items_center().gap(px(4.)).min_w_0()
            .child(icon_btn(cx, light, "hist-first", "skip-back", "The first step", "dim", n > 0, move |app, _| { stop(app); if on(app) > 0 { select(app, Some(0)) } }))
            .child(icon_btn(cx, light, "hist-prev", "chevron-left", "The step before", "dim", n > 0, move |app, _| { stop(app); let n = on(app); let a = app.screens.plotview.hist.at; select(app, back(a, n)) }))
            .child(icon_btn(cx, light, "hist-play", if hist.playing { "pause" } else { "play" }, if hist.playing { "Pause" } else { "Play the steps" }, "accent", n > 0, |app, cx| play(app, cx)))
            .child(icon_btn(cx, light, "hist-next", "chevron-right", "The step after", "dim", n > 0, move |app, _| { stop(app); let n = on(app); let a = app.screens.plotview.hist.at; select(app, fwd(a, n)) }))
            .child(icon_btn(cx, light, "hist-last", "skip-forward", "The last step", "dim", n > 0, move |app, _| { stop(app); let n = on(app); if n > 0 { select(app, Some(n - 1)) } }))
            .child(div().id("hist-title").mx(px(8.)).min_w_0().flex_shrink().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c("text")).child(title))
            .child(div().flex_1())
            .child(btn(cx, light, "hist-diff", "Highlight edits", hist.diff && at.is_some(), n > 0, "Compare a snapshot with the one before: red removed, yellow changed, green added. From Live, show the latest snapshot.", |app, _| { stop(app); highlight(app) }))
            .child(btn(cx, light, "compare-btn", "Compare", pv.menu == 1, true, "Compare two versions of the file", |app, cx| toggle_menu(app, 0, cx)))
            .child(btn(cx, light, "step-live", "Live", at.is_none(), at.is_some(), "Back to the file as it is now", |app, _| { stop(app); select(app, None) }));
        // the scrubber: a tick a step, the live file last, each where it falls in the track
        let span = n.max(1) as f32;
        let mut scrub = div().id("hist-scrub").relative().h(px(18.)).mx(px(8.)).child(div().absolute().left_0().right_0().top(px(8.)).h(px(2.)).bg(c("line")));
        let tips: Vec<(String, bool)> = (0..=n).map(|i| if i == n { ("The live file".to_string(), at.is_none()) } else { (hist.steps.as_ref().map(|h| h.steps[i].tip.clone()).unwrap_or_default(), at == Some(i)) }).collect();
        for (i, (tip, cur)) in tips.into_iter().enumerate() {
            let mark = if cur { div().w(px(4.)).h(px(16.)).mt(px(-2.)).bg(c("accent")) } else { div().w(px(2.)).h(px(12.)).bg(c("dim")) };
            scrub = scrub.child(div().id(SharedString::from(format!("step-{i}"))).absolute().left(relative(i as f32 / span)).top(px(3.)).ml(px(-5.)).w(px(10.)).h(px(12.)).flex().justify_center().cursor_pointer()
                .child(mark).tooltip(crate::keys::tip(&tip, "", light))
                .on_click(cx.listener(move |this, _, _, cx| { stop(this); select(this, if i == n { None } else { Some(i) }); cx.notify(); })));
        }
        // what the step followed (a press goes there in the thread), or the live file's note; the legend at its end while edits are highlighted
        let said = match (&step, &hist.steps) {
            (Some(s), _) => div().id("hist-said").flex().flex_1().min_w_0().items_center().gap(px(6.)).cursor_pointer().text_color(c("dim")).hover(|d| d.text_color(c("text")))
                .child(art::icon("message-square", 14., theme::color(light, "dim")))
                .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(s.detail.clone()))
                .tooltip(crate::keys::tip(&format!("{}\nGo to this point of the thread", s.detail), "", light))
                .on_click(cx.listener(|this, _, _, cx| { jump(this); cx.notify(); })).into_any_element(),
            (None, h) => div().id("hist-said").flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c("faint"))
                .child(if loading { "Loading the history…".to_string() } else { h.as_ref().map_or(String::new(), |h| h.note.clone()) }).into_any_element(),
        };
        let mut said_row = div().flex().items_center().gap(px(12.)).min_w_0().h(px(16.)).child(said);
        if hist.diff && step.as_ref().is_some_and(|s| !s.old.is_empty()) { said_row = said_row.child(legend(light)); }
        bar = bar.child(row).child(scrub).child(said_row);
    }
    if pv.menu == 1 {
        let mut rows = menu_rows(hist.steps.as_ref(), hist.refs.as_ref());
        if hist.side == 1 { rows.insert(0, ("Working copy".into(), "the files as they are now".into(), Pick::Live)); }
        let mut list = div().id("compare-list").absolute().left(px(12.)).bottom(relative(1.)).mb(px(4.)).w(px(460.)).max_h(px(360.)).overflow_y_scroll().flex().flex_col()
            .bg(c("raise")).border_1().border_color(c("line")).occlude()
            .child(div().px_3().py(px(6.)).text_color(c("faint")).child(if hist.side == 1 { "Compare with (new)" } else { "Compare from (old)" }))
            .children(hist.ref_input.clone().map(|i| div().mx_3().mb(px(6.)).px_2().py_1().border_1().border_color(c("line")).bg(c("bg")).child(i)))
            .children((!hist.ref_err.is_empty()).then(|| div().px_3().pb(px(6.)).text_color(c("bad")).child(hist.ref_err.clone())));
        if rows.is_empty() { list = list.child(div().px_3().py(px(6.)).border_t_1().border_color(c("line")).text_color(c("dim")).child(if hist.refs.is_none() { "Loading versions…" } else { "No versions in this project's history yet" })); }
        for (i, (label, sub, pick)) in rows.into_iter().enumerate() {
            list = list.child(div().id(SharedString::from(format!("cmp-{i}"))).flex().items_baseline().gap(px(10.)).px_3().py(px(6.)).border_t_1().border_color(c("line")).cursor_pointer().hover(|s| s.bg(c("panel")))
                .child(div().flex_none().max_w(px(220.)).overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c("text")).child(label))
                .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c("faint")).child(sub))
                .on_click(cx.listener(move |this, _, _, cx| { stop(this); match &pick { Pick::Step(i) => compare_step(this, *i), Pick::Commit(c, l) => compare(this, Some((c, l))), Pick::Live => if let Some((_, f)) = live(this) { self::pick(this, 1, f, String::new()) } } cx.notify(); })));
        }
        bar = bar.child(list);
    }
    bar.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_version_is_the_hubs_git_path_and_a_comparison_shows_old_diff_or_new() {
        assert_eq!(version("/p/.git", "/p", "abc", "/p/b.kicad_pcb"), "git\t/p/.git\tabc\t2\t/p/b.kicad_pcb");
        assert_eq!(version("/p/.git", "/p", "abc", "/x/b.kicad_pcb"), ""); assert_eq!(version("/p/.git", "/p", "", "/p/b.kicad_pcb"), "");
        let mut c = Cmp { base: "0|/p|".into(), label: "step 2".into(), old: "OLD".into(), new: "NEW".into(), mode: 1, bits: 0, to: String::new() };
        assert_eq!(shown(&c), "diff\nOLD\nNEW"); c.bits = 4096; assert_eq!(shown(&c), "diff\nOLD\nNEW\n4096", "marks keep to the focus"); c.mode = 0; assert_eq!(shown(&c), "OLD", "only a Diff has marks"); c.bits = 0; assert_eq!(shown(&c), "OLD"); c.mode = 2; assert_eq!(shown(&c), "NEW");
        c.old.clear(); c.mode = 0; assert_eq!(shown(&c), "NEW", "a first step has no old version: what there is shows");
    }
    #[test] fn steps_take_the_hubs_words_and_an_older_hub_still_gives_a_track() {
        let s = steps_of(&json!({"git": "/p/.git", "top": "/p", "pcb": "/p/b.kicad_pcb", "sch": "", "file": "/p/b.kicad_pcb", "live": "Live · 1 of 2 changes", "note": "The live file.",
            "steps": [{"c": "a1", "p": "", "t": 5, "old": "", "new": "NEW1", "label": "Step 1", "tip": "Step 1 · 14:08", "title": "Step 1 of 2 · before the agent's edits", "detail": "Saved 14:08 · Turn started · add an LDO", "entry": "m1", "id": "step:a1"},
                      {"c": "b2", "p": "a1", "t": 9, "old": "OLD2", "new": "NEW2", "label": "Step 2", "tip": "Step 2 · 14:09", "title": "Step 2 of 2", "detail": "Saved 14:09 · After Edit", "entry": "tu1", "id": "step:b2"}]})).unwrap();
        assert_eq!((s.steps.len(), s.steps[1].old.as_str(), s.steps[1].new.as_str(), s.live.as_str()), (2, "OLD2", "NEW2", "Live · 1 of 2 changes"));
        assert_eq!(s.steps[0].title, "Step 1 of 2 · before the agent's edits"); assert_eq!(s.steps[1].entry, "tu1");
        assert_eq!(steps_of(&Value::Null).map(|s| (s.steps.len(), s.note)), Some((0, "The design history could not be read here.".to_string())), "a refusal is said, not loaded for ever"); assert!(steps_of(&json!("x")).is_none());
        let old = steps_of(&json!({"git": "/p/.git", "top": "/p", "pcb": "/p/b.kicad_pcb", "file": "/p/b.kicad_pcb", "steps": [{"c": "a1", "p": "", "t": 5}, {"c": "b2", "p": "a1", "t": 9}]})).unwrap();
        assert_eq!(old.steps[1].new, "git\t/p/.git\tb2\t2\t/p/b.kicad_pcb"); assert_eq!(old.steps[1].old, "git\t/p/.git\ta1\t2\t/p/b.kicad_pcb"); assert_eq!(old.steps[0].old, "");
        assert_eq!((old.steps[1].label.as_str(), old.live.as_str()), ("Step 2", "Live · 2 steps"));
    }
    #[test] fn the_hubs_reasons_read_as_words() {
        assert_eq!(none_words("@bp.file-unread"), "This version of the file could not be read."); assert_eq!(none_words("@bp.file-missing:a.kicad_pcb"), "a.kicad_pcb is not there: it was moved or deleted.");
        assert_eq!(none_words("Invalid geometry"), "Invalid geometry");
    }
    #[test] fn a_step_row_opens_the_kinds_it_changed_board_first() {
        assert_eq!(row_kinds("Design change · heater.kicad_pcb"), ["board"]);
        assert_eq!(row_kinds("Design change · a.kicad_sch (new), b.kicad_pcb"), ["board", "schematic"]);
        assert_eq!(row_kinds("Design change · old.kicad_pcb (removed), s.kicad_sch"), ["schematic"], "a removed file shows nothing");
        assert!(row_kinds("Design change").is_empty()); assert!(row_kinds("Design change · notes.md").is_empty());
    }
    #[test] fn the_steps_either_side_are_named_as_they_would_show() {
        let h = Steps { steps: vec![Step { new: "N1".into(), ..Step::default() }, Step { old: "N1".into(), new: "N2".into(), ..Step::default() }, Step { old: "N2".into(), new: "N3".into(), ..Step::default() }], ..Steps::default() };
        assert_eq!(near(&h, 1, 1, 0), ("N1".to_string(), "diff\nN2\nN3".to_string()), "a first step has no old: it shows as it is");
        assert_eq!(near(&h, 0, 2, 0), (String::new(), "N2".to_string())); assert_eq!(near(&h, 2, 1, 8), ("diff\nN1\nN2\n8".to_string(), String::new()));
    }
    #[test] fn a_version_of_the_file_on_show_stays_while_the_next_loads() {
        let k = "0|/p|";
        assert!(same_file(k, "0|/p|", "0|/p|git\t/p/.git\tc1\t2\t/p/b.kicad_pcb")); assert!(same_file(k, "0|/p|diff\nA\nB", "0|/p|"));
        assert!(!same_file(k, "0|/p|/p/other.kicad_pcb", "0|/p|"), "another board of the project is not kept"); assert!(!same_file(k, "0|/q|", "0|/p|"), "nor another project's");
        assert!(!same_file("", "x", "y"));
    }
    #[test] fn the_arrows_go_as_0_14s_did() {
        assert_eq!(back(None, 3), Some(2), "from live, back is the newest step"); assert_eq!(back(Some(2), 3), Some(1)); assert_eq!(back(Some(0), 3), Some(0)); assert_eq!(back(None, 0), None);
        assert_eq!(fwd(None, 3), None, "forward from live stays live"); assert_eq!(fwd(Some(1), 3), Some(2)); assert_eq!(fwd(Some(2), 3), None, "past the newest step is live");
    }
    #[test] fn the_compare_menu_lists_the_newest_steps_then_the_versions() {
        let h = Steps { steps: (0..10).map(|i| Step { label: format!("Step {}", i + 1), detail: format!("d{i}"), ..Step::default() }).collect(), ..Steps::default() };
        let r = super::super::plot::Refs { git: String::new(), top: String::new(), pcb: String::new(), sch: String::new(), rows: vec![("head".into(), "HEAD".into(), "c1".into()), ("turn".into(), "turn/0".into(), "c2".into()), ("turn".into(), "turn/3".into(), "c3".into())] };
        let rows = menu_rows(Some(&h), Some(&r));
        assert_eq!(rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["Step 10", "Step 9", "Step 8", "Step 7", "Step 6", "Step 5", "Step 4", "Step 3", "HEAD", "Turn 0", "Turn 3"]);
        assert_eq!(rows[0].2, Pick::Step(9)); assert_eq!(rows[9].1, "this thread, before it began"); assert_eq!(rows[10].2, Pick::Commit("c3".into(), "Turn 3".into()));
    }
}
