//! History and comparison on the Board and Schematic pages: the file against a version (HEAD, a branch, a tag, a
//! commit) or one design-history step against the one before it, each shown as Old (that version), New (the other) or Diff (removed, changed and added on layers 26-28), and step
//! playback through the thread's steps. Everything is the hub's: `history.refs` / `history.steps` name the versions, `kicad.watch` on a version path (`git\t...`, `diff\n...`) draws them.
//! This module only builds the version paths, keeps what is on show (`Cmp`, in the canvas's PlotView) and draws the bar.
use super::plotview::PlotView;
use crate::{hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Step { pub c: String, pub p: String, pub t: u64 }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Steps { pub git: String, pub top: String, pub pcb: String, pub sch: String, pub steps: Vec<Step> }
pub fn steps_of(v: &Value) -> Option<Steps> {
    if !v.is_object() { return None }
    let g = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let steps = v["steps"].as_array().into_iter().flatten().filter_map(|s| Some(Step { c: s["c"].as_str()?.to_string(), p: s["p"].as_str().unwrap_or("").to_string(), t: s["t"].as_u64().unwrap_or(0) })).collect();
    Some(Steps { git: g("git"), top: g("top"), pcb: g("pcb"), sch: g("sch"), steps })
}

/// core/hist's version path of `file` in `commit`: "git\t<gitdir>\t<commit>\t<len top>\t<top>/<rel>" ("" when the file is not under the top folder).
pub fn version(git: &str, top: &str, commit: &str, file: &str) -> String {
    match file.strip_prefix(&format!("{top}/")) { Some(rel) if !commit.is_empty() && !git.is_empty() => format!("git\t{git}\t{commit}\t{}\t{top}/{rel}", top.chars().count()), _ => String::new() }
}

/// What a comparison shows: `old` and `new` are sources (a version path, or the live file's), `mode` 0 Old, 1 Diff, 2 New, `bits` the layers its marks keep to (0 every layer).
#[derive(Clone, Debug, PartialEq)]
pub struct Cmp { pub base: String, pub label: String, pub old: String, pub new: String, pub mode: u8, pub bits: u32 }
/// The source watched for the comparison on show. Where there is no old version (a first step), what there is shows as it is.
pub fn shown(c: &Cmp) -> String {
    if c.old.is_empty() { return c.new.clone() }
    match c.mode { 0 => c.old.clone(), 2 => c.new.clone(), _ if c.bits != 0 => format!("diff\n{}\n{}\n{}", c.old, c.new, c.bits), _ => format!("diff\n{}\n{}", c.old, c.new) }
}

#[derive(Default, Clone)]
pub struct Hist { pub steps: Option<Steps>, pub refs: Option<super::plot::Refs>, /** the step on show (an index into steps), none for the live file */ pub at: Option<usize>, pub play: u64, pub playing: bool }

fn open_file(app: &Backplane) -> Option<(String, String, String, String)> { // (kind, key, root, the file shown)
    let v = &app.data["thread"]["viewer"];
    let (open, key) = (v["open"].as_str().unwrap_or("").to_string(), v["key"].as_str().unwrap_or("").to_string());
    if !matches!(open.as_str(), "board" | "schematic") { return None }
    let mut it = key.splitn(3, '|'); let (_, root, own) = (it.next(), it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string());
    Some((open, key, root, own))
}
fn file_of(app: &Backplane, kind: &str, own: &str, pcb: &str, sch: &str) -> String { let _ = app; if !own.is_empty() { own.to_string() } else if kind == "board" { pcb.to_string() } else { sch.to_string() } }
pub fn ask_refs(app: &mut Backplane) { let t = app.selected.clone(); if t.is_empty() { return } let _ = app.tx.try_send(Command::Rpc("history.refs".into(), super::settings::Secret(json!({"thread": t})), "refs")); }
pub fn ask_steps(app: &mut Backplane) { let t = app.selected.clone(); if t.is_empty() { return } let _ = app.tx.try_send(Command::Rpc("history.steps".into(), super::settings::Secret(json!({"thread": t})), "steps")); }

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
    app.screens.plotview.compare = c; app.screens.plotview.menu = 0; app.screens.plotview.view = None;
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
    let Some((kind, key, _, own)) = open_file(app) else { return };
    let Some((commit, label)) = commit else { return set(app, None) };
    let Some(r) = app.screens.plotview.hist.refs.clone() else { return };
    let file = file_of(app, &kind, &own, &r.pcb, &r.sch);
    let old = version(&r.git, &r.top, commit, &file); if old.is_empty() { return }
    app.screens.plotview.hist.at = None;
    set(app, Some(Cmp { base: key, label: label.to_string(), old, new: file, mode: 1, bits: 0 }));
}
/// The step `i` of the thread's history on show (None: the live file): Diff of it against the step before, the way the bar's ticks go.
pub fn select(app: &mut Backplane, i: Option<usize>) {
    let Some((kind, key, _, own)) = open_file(app) else { return };
    let Some(h) = app.screens.plotview.hist.steps.clone() else { return };
    let Some(i) = i.filter(|i| *i < h.steps.len()) else { return set(app, None) };
    let file = file_of(app, &kind, &own, &h.pcb, &h.sch);
    let (s, mode) = (&h.steps[i], app.screens.plotview.compare.as_ref().map_or(1, |c| c.mode));
    let (old, new) = (version(&h.git, &h.top, &s.p, &file), version(&h.git, &h.top, &s.c, &file)); if new.is_empty() { return }
    app.screens.plotview.hist.at = Some(i);
    set(app, Some(Cmp { base: key, label: format!("step {}", i + 1), old, new, mode, bits: 0 }));
}
/// Old / Diff / New of the comparison on show.
pub fn mode(app: &mut Backplane, m: u8) { if let Some(mut c) = app.screens.plotview.compare.clone() { c.mode = m; set(app, Some(c)); } }

/// Step playback: from the step after the one on show (the first from live) every 1.2 s to the newest, then back to live. A press while it plays stops it.
pub fn play(app: &mut Backplane, cx: &mut Context<Backplane>) {
    if app.screens.plotview.hist.playing { app.screens.plotview.hist.playing = false; app.screens.plotview.hist.play += 1; return }
    let n = app.screens.plotview.hist.steps.as_ref().map_or(0, |h| h.steps.len()); if n == 0 { return }
    app.screens.plotview.hist.playing = true; app.screens.plotview.hist.play += 1; let token = app.screens.plotview.hist.play;
    let first = match app.screens.plotview.hist.at { Some(i) if i + 1 < n => i + 1, _ => 0 };
    select(app, Some(first));
    cx.spawn(async move |weak, cx| loop {
        cx.background_executor().timer(std::time::Duration::from_millis(1200)).await;
        let go = weak.update(cx, |app, cx| {
            if app.screens.plotview.hist.play != token || !app.screens.plotview.hist.playing { return false }
            let n = app.screens.plotview.hist.steps.as_ref().map_or(0, |h| h.steps.len());
            match app.screens.plotview.hist.at { Some(i) if i + 1 < n => select(app, Some(i + 1)), _ => { app.screens.plotview.hist.playing = false; select(app, None); } }
            cx.notify(); app.screens.plotview.hist.playing
        }).unwrap_or(false);
        if !go { break }
    }).detach();
}

fn chip(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, f: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).px_2().py_px().text_size(px(12.)).cursor_pointer().border_1();
    (if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) }).child(label)
        .on_click(cx.listener(move |this, _, _, cx| { f(this, cx); cx.notify(); })).into_any_element()
}

/// The bar: History and Compare buttons; while a comparison shows, its label, Old/Diff/New and ×; the open menu (the steps' ticks with previous/next/play, or the versions).
/// `trailing`: the viewer pane's own controls, at the row's end (they wrap under the chips in a narrow pane).
pub fn bar_el(cx: &mut Context<Backplane>, light: bool, pv: &PlotView, trailing: Option<AnyElement>) -> AnyElement {
    let hist = &pv.hist;
    let c = |n: &str| theme::color(light, n);
    let mut row = div().flex().flex_wrap().items_center().gap_1().px_3().py_1();
    row = row.child(chip(cx, light, "hist-btn".into(), "History".into(), pv.menu == 2, |app, _| { let m = if app.screens.plotview.menu == 2 { 0 } else { 2 }; app.screens.plotview.menu = m; if m == 2 { ask_steps(app) } }));
    row = row.child(chip(cx, light, "compare-btn".into(), "Compare".into(), pv.menu == 1, |app, _| { let m = if app.screens.plotview.menu == 1 { 0 } else { 1 }; app.screens.plotview.menu = m; if m == 1 { ask_refs(app) } }));
    if let Some(cmp) = &pv.compare {
        row = row.child(div().px_1().text_size(px(12.)).text_color(c("accent")).child(cmp.label.clone()));
        for (i, l) in ["Old", "Diff", "New"].iter().enumerate() { let m = i as u8; row = row.child(chip(cx, light, format!("cmp-mode-{i}"), l.to_string(), cmp.mode == m, move |app, _| mode(app, m))); }
        row = row.child(chip(cx, light, "cmp-off".into(), "×".into(), false, |app, _| set(app, None)));
    }
    if let Some(t) = trailing { row = row.child(div().flex_1()).child(t); }
    let mut col = div().flex().flex_col().child(row);
    if pv.menu == 2 {
        let mut ticks = div().flex().flex_wrap().items_center().gap_1().px_3().pb_1();
        match &hist.steps {
            None => { ticks = ticks.child(div().text_size(px(12.)).text_color(c("dim")).child("Loading the history…")); }
            Some(h) if h.steps.is_empty() => { ticks = ticks.child(div().text_size(px(12.)).text_color(c("dim")).child("No steps yet: the agent's edits to the design are recorded here")); }
            Some(h) => {
                let n = h.steps.len();
                ticks = ticks.child(chip(cx, light, "step-prev".into(), "◀".into(), false, move |app, _| { let i = match app.screens.plotview.hist.at { Some(i) if i > 0 => Some(i - 1), Some(_) => Some(0), None => Some(n - 1) }; select(app, i) }));
                ticks = ticks.child(chip(cx, light, "step-play".into(), if hist.playing { "❚❚".into() } else { "▶".into() }, hist.playing, |app, cx| play(app, cx)));
                ticks = ticks.child(chip(cx, light, "step-next".into(), "▶|".into(), false, move |app, _| { let i = match app.screens.plotview.hist.at { Some(i) if i + 1 < n => Some(i + 1), _ => None }; select(app, i) }));
                ticks = ticks.child(chip(cx, light, "step-live".into(), "Live".into(), hist.at.is_none(), |app, _| select(app, None)));
                for i in 0..n { ticks = ticks.child(chip(cx, light, format!("step-{i}"), format!("{}", i + 1), hist.at == Some(i), move |app, _| select(app, Some(i)))); }
            }
        }
        col = col.child(ticks);
    }
    if pv.menu == 1 {
        let mut list = div().id("compare-list").flex().flex_col().px_3().pb_1().text_size(px(12.)).max_h(px(220.)).overflow_y_scroll();
        match &hist.refs {
            None => { list = list.child(div().text_color(c("dim")).child("Loading versions…")); }
            Some(r) if r.rows.is_empty() => { list = list.child(div().text_color(c("dim")).child("No versions in this project's history yet")); }
            Some(r) => for (i, (kind, name, commit)) in r.rows.iter().enumerate() {
                let (commit, name2) = (commit.clone(), name.clone());
                list = list.child(div().id(SharedString::from(format!("cmp-{i}"))).flex().gap_2().py_px().cursor_pointer().hover(|s| s.bg(c("raise"))).child(div().w(px(46.)).text_color(c("dim")).child(kind.clone())).child(div().flex_1().min_w_0().child(name.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| { compare(this, Some((&commit, &name2))); cx.notify(); })));
            },
        }
        col = col.child(list);
    }
    col.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_version_is_the_hubs_git_path_and_a_comparison_shows_old_diff_or_new() {
        assert_eq!(version("/p/.git", "/p", "abc", "/p/b.kicad_pcb"), "git\t/p/.git\tabc\t2\t/p/b.kicad_pcb");
        assert_eq!(version("/p/.git", "/p", "abc", "/x/b.kicad_pcb"), ""); assert_eq!(version("/p/.git", "/p", "", "/p/b.kicad_pcb"), "");
        let mut c = Cmp { base: "0|/p|".into(), label: "step 2".into(), old: "OLD".into(), new: "NEW".into(), mode: 1, bits: 0 };
        assert_eq!(shown(&c), "diff\nOLD\nNEW"); c.bits = 4096; assert_eq!(shown(&c), "diff\nOLD\nNEW\n4096", "marks keep to the focus"); c.mode = 0; assert_eq!(shown(&c), "OLD", "only a Diff has marks"); c.bits = 0; assert_eq!(shown(&c), "OLD"); c.mode = 2; assert_eq!(shown(&c), "NEW");
        c.old.clear(); c.mode = 0; assert_eq!(shown(&c), "NEW", "a first step has no old version: what there is shows");
    }
    #[test] fn steps_parse_from_the_hubs_answer() {
        let s = steps_of(&json!({"git": "/p/.git", "top": "/p", "pcb": "/p/b.kicad_pcb", "sch": "", "steps": [{"c": "a1", "p": "", "t": 5}, {"c": "b2", "p": "a1", "t": 9}]})).unwrap();
        assert_eq!((s.steps.len(), s.steps[1].p.as_str()), (2, "a1")); assert!(steps_of(&Value::Null).is_none());
    }
}
