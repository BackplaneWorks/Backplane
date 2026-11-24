//! Downloads from the hub (docs/file-links.md): a file link of another computer's hub, a share, the Files tab's and Mech's Download. Each one is
//! fetched on a thread of its own by viewers/downloads.rs `fetch` (streamed to a `.part` file, resumed after errors) and drawn as a strip
//! along the bottom of the window: its name and percent while it comes, then Open and Show in folder, or why it failed and Retry. The window
//! repaints only when a download reports (at most every REPORT_MS), never on a timer.
use crate::{theme, viewers::downloads::{self, Event, Resume}, Backplane};
use gpui::{prelude::*, *};
use serde_json::json;
use std::path::PathBuf;

/// Where a download was asked for: the Files tab's says how it went in its own status line too (and its metric, which scripts read).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum From { Files, Other }
#[derive(Clone, Debug, PartialEq)]
pub enum State { Going, Done(PathBuf), Failed(String) }
#[derive(Clone, Debug)]
pub struct Dl { pub id: u64, pub url: String, pub name: String, pub got: u64, pub total: Option<u64>, pub state: State, pub from: From, resume: Resume, stop: std::sync::Arc<std::sync::atomic::AtomicBool> }
#[derive(Default)]
pub struct Downloads { pub list: Vec<Dl>, next: u64 }

/// How many downloads the strip shows; older finished ones go.
pub const SHOWN: usize = 3;
impl Downloads {
    pub fn add(&mut self, url: &str, name: &str, from: From) -> u64 {
        self.next += 1;
        self.list.push(Dl { id: self.next, url: url.into(), name: downloads::safe_name(name), got: 0, total: None, state: State::Going, from, resume: Resume::default(), stop: Default::default() });
        // keep every one still coming, and the newest finished ones up to SHOWN in all
        while self.list.len() > SHOWN { match self.list.iter().position(|d| d.state != State::Going) { Some(i) => { forget(&self.list.remove(i)); } None => break } }
        self.next
    }
    /// What a report changes; true when the strip draws differently.
    pub fn apply(&mut self, id: u64, e: Event) -> bool {
        let Some(d) = self.list.iter_mut().find(|d| d.id == id) else { return false };
        match e {
            Event::Progress { got, total } => { let before = line(d); d.got = got; d.total = total; line(d) != before }
            // the name it was saved as (`big (2).bin` when big.bin was there)
            Event::Done { path, bytes } => { if let Some(n) = path.file_name() { d.name = n.to_string_lossy().into_owned(); } d.got = bytes; d.total = Some(bytes); d.state = State::Done(path); d.resume = Resume::default(); true }
            Event::Failed { why, part, etag, got } => { d.got = got; d.state = State::Failed(why); d.resume = Resume { part, etag }; true }
        }
    }
    /// Gone from the strip; one still coming stops, its part file deleted.
    pub fn close(&mut self, id: u64) { if let Some(i) = self.list.iter().position(|d| d.id == id) { forget(&self.list.remove(i)); } }
    pub fn get(&self, id: u64) -> Option<&Dl> { self.list.iter().find(|d| d.id == id) }
}
/// A download leaves the list: one still coming stops (its thread deletes its part file), a failed one's part file goes now.
fn forget(d: &Dl) {
    d.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    if let (State::Failed(_), Some(p)) = (&d.state, &d.resume.part) { let _ = std::fs::remove_file(p); }
}
/// The strip's words for a download.
pub fn line(d: &Dl) -> String {
    use crate::i18n::{tf, trs};
    match &d.state {
        State::Going => match downloads::percent(d.got, d.total) {
            Some(p) => tf("gpui.dl.percent", &[("name", (&d.name).into()), ("pct", p.into())]),
            None => tf("gpui.dl.mb", &[("name", (&d.name).into()), ("mb", format!("{:.1}", d.got as f64 / 1e6).into())]),
        },
        State::Done(_) => tf("gpui.dl.saved", &[("name", (&d.name).into())]),
        State::Failed(why) => tf("gpui.dl.failed", &[("name", (&d.name).into()), ("why", trs(why).into())]),
    }
}

impl Backplane {
    /// Download `url` (a path on the hub) to this computer's Downloads folder as `name`.
    pub(crate) fn download(&mut self, url: String, name: String, from: From, cx: &mut Context<Self>) {
        if !super::images::safe_url(&url) { return }
        let id = self.screens.downloads.add(&url, &name, from);
        self.download_run(id, cx);
    }
    fn download_run(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(d) = self.screens.downloads.get(id).cloned() else { return };
        /* i18n-ok: English kept, drawn through trs */ let Some(t) = self.screens.attacher.target.clone() else { self.screens.downloads.apply(id, Event::Failed { why: "No hub address to download from".into(), part: None, etag: String::new(), got: 0 }); cx.notify(); return };
        let src = downloads::Source::of(&t);
        let (tx, rx) = async_channel::bounded::<Event>(4);
        let (url, name, resume, stop) = (d.url.clone(), d.name.clone(), d.resume.clone(), d.stop.clone());
        // a file of hundreds of MB takes a while: its own thread, not one of the window's few background ones
        std::thread::spawn(move || downloads::fetch(&src, &url, &downloads::downloads_folder(), &name, resume, &stop, |e| match e {
            // progress may be dropped while the window is behind; the end never is
            Event::Progress { .. } => { let _ = tx.try_send(e); }
            _ => { let _ = tx.send_blocking(e); }
        }));
        cx.spawn(async move |weak, cx| {
            while let Ok(e) = rx.recv().await { if weak.update(cx, |this, cx| this.download_event(id, e, cx)).is_err() { break } }
        }).detach();
        cx.notify();
    }
    fn download_event(&mut self, id: u64, e: Event, cx: &mut Context<Self>) {
        let end = match &e { Event::Done { path, bytes } => Some(Ok((path.clone(), *bytes))), Event::Failed { why, .. } => Some(Err(why.clone())), _ => None };
        let changed = self.screens.downloads.apply(id, e);
        if let (Some(r), Some(d)) = (end, self.screens.downloads.get(id).cloned()) {
            let ok = r.is_ok();
            self.m("download", json!({"ok": ok, "name": d.name, "path": r.as_ref().ok().map(|(p, _)| p.display().to_string()), "bytes": r.as_ref().ok().map(|x| x.1), "error": r.as_ref().err()}));
            if d.from == From::Files { super::filesui::download_done(self, &r, cx); }
        }
        if changed { cx.notify(); }
    }
    pub(crate) fn download_press(&mut self, id: u64, what: &str, cx: &mut Context<Self>) {
        let path = match self.screens.downloads.get(id).map(|d| d.state.clone()) { Some(State::Done(p)) => Some(p), _ => None };
        match (what, path) {
            ("open", Some(p)) => cx.open_with_system(&p),
            ("reveal", Some(p)) => cx.reveal_path(&p),
            // only a failed one starts again (two presses on one frame start it once)
            ("retry", _) => { let failed = self.screens.downloads.list.iter_mut().find(|d| d.id == id && matches!(d.state, State::Failed(_)));
                if let Some(d) = failed { d.state = State::Going; self.download_run(id, cx); } }
            ("close", _) => self.screens.downloads.close(id),
            _ => {}
        }
        cx.notify();
    }
}

pub fn strip(cx: &mut Context<Backplane>, list: &[Dl], light: bool) -> Option<AnyElement> {
    if list.is_empty() { return None }
    use crate::i18n::trs;
    let c = |k: &str| theme::color(light, k);
    let mut col = div().id("downloads").flex_shrink_0().flex().flex_col().bg(c("raise")).border_t_1().border_color(c("accent"));
    for d in list {
        let id = d.id;
        let btn = |key: &'static str, label: &str, strong: bool| div().id(SharedString::from(format!("dl-{key}-{id}"))).px_2().h(px(16.)).flex().items_center().cursor_pointer().text_size(crate::scale::tpx(11.)).font_weight(FontWeight::BOLD)
            .bg(if strong { c("accent") } else { c("raise") }).text_color(if strong { c("on-accent") } else { c("dim") }).child(trs(label))
            .on_click(cx.listener(move |this, _, _, cx| this.download_press(id, key, cx)));
        let mut row = div().flex().items_center().gap_1().flex_shrink_0();
        /* i18n-ok: drawn through trs in btn */ match &d.state {
            State::Done(_) => { row = row.child(btn("open", "Open", true)).child(btn("reveal", "Show in folder", false)); } // i18n-ok: English kept, drawn through trs; or HTTP
            State::Failed(_) => { row = row.child(btn("retry", "Retry", true)); } // i18n-ok: English kept, drawn through trs; or HTTP
            State::Going => {}
        }
        /* i18n-ok: drawn through trs in btn */ row = row.child(btn("close", "Close", false));
        let bar = match downloads::percent(d.got, d.total) { Some(p) if d.state == State::Going => Some(div().absolute().left_0().bottom_0().h(px(2.)).w(relative(p as f32 / 100.)).bg(c("accent"))), _ => None };
        col = col.child(div().relative().min_h(crate::scale::tpx(22.)).flex().items_center().justify_between().gap_2().px_3().py_1()
            .child(div().flex_1().min_w_0().text_size(crate::scale::tpx(11.)).text_color(c("text")).truncate().child(line(d))).child(row).children(bar));
    }
    Some(col.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_download_reports_its_percent_then_how_it_ended() {
        let mut l = Downloads::default();
        let id = l.add("/file?path=%2Fa.zip", "a.zip", From::Other);
        assert!(l.apply(id, Event::Progress { got: 50, total: Some(200) })); assert_eq!(line(l.get(id).unwrap()), "a.zip · 25%");
        assert!(!l.apply(id, Event::Progress { got: 51, total: Some(200) }), "the same words: nothing to draw");
        assert!(l.apply(id, Event::Failed { why: "the download ended early".into(), part: Some("/d/a.zip.part".into()), etag: "\"t\"".into(), got: 120 }));
        assert_eq!(line(l.get(id).unwrap()), "a.zip: download failed: the download ended early");
        assert_eq!(l.get(id).unwrap().resume.etag, "\"t\"", "Retry goes on from the part file");
        assert!(l.apply(id, Event::Done { path: "/d/a (2).zip".into(), bytes: 200 })); assert_eq!(line(l.get(id).unwrap()), "Saved a (2).zip", "the name it got");
        assert!(!l.apply(99, Event::Progress { got: 1, total: None }), "a download closed meanwhile");
    }
    #[test] fn the_strip_keeps_what_is_coming_and_the_newest_finished() {
        let mut l = Downloads::default();
        let a = l.add("/x", "a", From::Other); l.apply(a, Event::Done { path: "/d/a".into(), bytes: 1 });
        let b = l.add("/x", "b", From::Other); let c = l.add("/x", "c", From::Files); let d = l.add("/x", "d", From::Other);
        assert_eq!(l.list.iter().map(|d| d.id).collect::<Vec<_>>(), [b, c, d], "the finished one went first");
        let e = l.add("/x", "e", From::Other); assert_eq!(l.list.len(), 4, "every one still coming stays");
        l.close(e); assert!(l.get(e).is_none());
        let dir = crate::tmp::uncreated("dl-forget"); std::fs::create_dir_all(&*dir).unwrap(); let part = dir.join("f.bin.part"); std::fs::write(&part, "half").unwrap();
        l.apply(d, Event::Failed { why: "x".into(), part: Some(part.clone()), etag: "\"t\"".into(), got: 4 }); l.close(d); assert!(!part.exists(), "a failed download closed leaves no part file");
        assert_eq!(line(l.get(b).unwrap()), "b · 0.0 MB", "no size yet");
    }
}
