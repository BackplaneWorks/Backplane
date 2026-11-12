//! The comment window: Ctrl+; wherever the keyboard is (in Backplane, or in the embedded editor through its reserved chords) opens it over the
//! KiCad pane at once; what is selected (references, pins, nets) and the screenshot fill in when the hub's capture comes back
//! (`editor.kicad.capture`, host kicad/comment.rs). Enter posts it into the pane's thread (`kicad.comment.post`: the hub words it, as for a phone's
//! viewer), Esc cancels; either way the keyboard goes back to KiCad. Square corners, theme colours, like the composer. Comments are hub state:
//! nothing here touches a design file.
use crate::{hub::Command, i18n::trs, screens::{composer::{Composer, ComposerEvent}, settings::Secret}, theme, Backplane as Win};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}, time::Instant};

/// The card's size (px).
pub const W: f32 = 440.;
pub const H: f32 = 132.;
/// An Enter before the capture came waits this long for it, then posts the note alone.
const WAIT_CAPTURE_MS: u64 = 2500;

pub struct KComment {
    pub project: String, pub frame: &'static str, pub thread: String, pub input: Entity<Composer>,
    /// the capture (null until it comes), why it could not be made, and whether Enter is waiting on it
    pub cap: Value, pub err: String, pub post_on_capture: bool,
    pub opened: Instant, painted: Arc<AtomicBool>,
}

fn list(c: &Value, k: &str) -> Vec<String> { c["names"][k].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect() }
/// What the card says is selected: the references, then pins and nets, at most `max` names and "+n" for the rest; None while the capture is
/// not in. Words are drawn through trs.
pub fn names_line(cap: &Value, max: usize) -> Option<String> {
    if cap.is_null() { return None }
    if cap["empty"] == true { /* i18n-ok: drawn through trs */ return Some(trs("Nothing selected: the area in view")) }
    let mut v = list(cap, "references"); v.extend(list(cap, "pins")); v.extend(list(cap, "nets").into_iter().map(|n| n.trim_start_matches('/').to_string()).filter(|n| !n.is_empty()));
    if v.is_empty() { /* i18n-ok: drawn through trs */ return Some(trs("Nothing selected: the area in view")) }
    let more = v.len().saturating_sub(max); v.truncate(max);
    Some(if more > 0 { format!("{} +{more}", v.join(" · ")) } else { v.join(" · ") }) // i18n-ok: names, and a count
}
/// Where the card goes: over the pane, centred across it and a third of the way down (the selection is usually mid-canvas, so the card keeps
/// off it); in the window's middle when the pane's place is not known. Always whole inside the window.
pub fn place(pane: Option<(f32, f32, f32, f32)>, win: (f32, f32)) -> (f32, f32) {
    let (x, y, w, h) = pane.unwrap_or((0., 0., win.0, win.1));
    let left = (x + (w - W) / 2.).clamp(0., (win.0 - W).max(0.));
    let top = (y + h * 0.18).clamp(0., (win.1 - H).max(0.));
    (left, top)
}
/// What Enter posts (`kicad.comment.post`): the capture's names, items and picture with the note; the note alone when no capture came.
/// None when there is nothing to say.
pub fn post_params(cap: &Value, note: &str, thread: &str, project: &str, frame: &str) -> Option<Value> {
    let note = note.trim();
    let has_names = !cap.is_null() && (cap["empty"] == true || ["references", "pins", "nets"].iter().any(|k| !list(cap, k).is_empty()));
    if note.is_empty() && !has_names { return None }
    let mut p = if cap.is_object() { cap.clone() } else { json!({"frame": if frame == "SCH" { "FK_SCH" } else { "FK_PCB" }, "project": project}) }; // i18n-ok: wire frame kinds
    p["note"] = json!(note); p["thread"] = json!(thread);
    Some(p)
}

fn ms(t: Instant) -> f64 { (t.elapsed().as_secs_f64() * 1e4).round() / 10. }
fn epoch_ms() -> u128 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()) }

impl Win {
    /// Ctrl+;: the window opens now over the pane (the capture fills it in when it comes); pressed again while it is open, nothing.
    pub fn kcomment_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screens.kcomment.is_some() { return }
        let Some(p) = self.screens.kicad.clone() else { eprintln!("GPUI_LOG {}", json!({"kcomment": "no-pane"})); cx.propagate(); return };
        let Some((project, frame)) = p.read(cx).comment_target() else { eprintln!("GPUI_LOG {}", json!({"kcomment": "no-editor"})); cx.propagate(); return };
        let thread = self.data["thread"]["id"].as_str().unwrap_or("").rsplit('|').next().unwrap_or("").to_string();
        let input = cx.new(|cx| { let mut c = Composer::with_context(cx, "AskInput"); /* i18n-ok: drawn through trs by the composer */ c.set_placeholder("Comment"); c });
        cx.subscribe(&input, |this, _, e: &ComposerEvent, cx| if matches!(e, ComposerEvent::Submit) { this.kcomment_post(cx) }).detach();
        window.focus(&input.focus_handle(cx)); p.update(cx, |p, _| p.hold_keys(true));
        let _ = self.tx.try_send(Command::Rpc("editor.kicad.capture".into(), Secret(json!({"project": project, "frame": frame})), "kicad"));
        eprintln!("GPUI_LOG {}", json!({"kcomment": "open", "frame": frame, "at_ms": epoch_ms() as u64}));
        self.screens.kcomment = Some(KComment { project, frame, thread, input, cap: Value::Null, err: String::new(), post_on_capture: false, opened: Instant::now(), painted: Arc::default() });
        cx.notify();
    }
    /// The hub's answer to the capture or to a post.
    pub fn kcomment_reply(&mut self, method: &str, reply: Value, cx: &mut Context<Self>) {
        let why = if reply["ok"] == true { String::new() } else { crate::hub::rpc_error(&reply) };
        if method == "kicad.comment.post" {
            if !why.is_empty() { self.note = crate::i18n::tf("gpui.kcomment.not.posted", &[("why", trs(&why).into())]); }
            return cx.notify()
        }
        let Some(k) = self.screens.kcomment.as_mut() else { return };
        if !k.cap.is_null() || !k.err.is_empty() { return }
        if why.is_empty() { k.cap = reply["result"].clone(); } else { k.err = why; }
        eprintln!("GPUI_LOG {}", json!({"kcomment": "capture", "ms": ms(k.opened), "hub_ms": reply["result"]["ms"], "capture_only": reply["result"]["capture_only"], "ok": k.err.is_empty(),
            "references": reply["result"]["names"]["references"], "image": !reply["result"]["image"].as_str().unwrap_or("").is_empty()}));
        if k.post_on_capture { self.kcomment_post(cx) }
        cx.notify();
    }
    /// Enter: posted into the thread, the window shut, the keyboard back to KiCad. Before the capture is in, it waits for it (a moment at most).
    pub fn kcomment_post(&mut self, cx: &mut Context<Self>) {
        let Some(k) = self.screens.kcomment.as_mut() else { return };
        let note = k.input.read(cx).content.to_string();
        if k.cap.is_null() && k.err.is_empty() && !k.post_on_capture && k.opened.elapsed().as_millis() < WAIT_CAPTURE_MS as u128 {
            k.post_on_capture = true;
            let wait = WAIT_CAPTURE_MS.saturating_sub(k.opened.elapsed().as_millis() as u64);
            cx.spawn(async move |w, cx| { cx.background_executor().timer(std::time::Duration::from_millis(wait)).await;
                let _ = w.update(cx, |w, cx| if w.screens.kcomment.as_ref().is_some_and(|k| k.post_on_capture) { if let Some(k) = w.screens.kcomment.as_mut() { k.err = "no capture".into(); /* i18n-ok: a flag, never drawn */ } w.kcomment_post(cx) }); }).detach();
            return
        }
        let Some(p) = post_params(&k.cap, &note, &k.thread, &k.project, k.frame) else { return };
        if k.thread.is_empty() { self.note = trs("A comment from KiCad needs an open thread."); return cx.notify() }
        eprintln!("GPUI_LOG {}", json!({"kcomment": "post", "ms": ms(k.opened), "note_chars": note.chars().count(), "references": p["names"]["references"], "image": p["image"]}));
        let _ = self.tx.try_send(Command::Rpc("kicad.comment.post".into(), Secret(p), "kicad"));
        self.kcomment_close(cx);
    }
    /// Shut (Esc, or after a post): the keyboard back to the editor.
    pub fn kcomment_close(&mut self, cx: &mut Context<Self>) {
        if self.screens.kcomment.take().is_none() { return }
        if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, cx| p.give_keys(cx)); }
        eprintln!("GPUI_LOG {}", json!({"kcomment": "closed"}));
        cx.notify();
    }
    /// Esc: true when it shut the window.
    pub fn kcomment_escape(&mut self, cx: &mut Context<Self>) -> bool { let open = self.screens.kcomment.is_some(); if open { self.kcomment_close(cx) } open }
}

/// The card over the pane, when the window is open.
pub fn view(w: &Win, cx: &mut Context<Win>, light: bool, win: Size<Pixels>) -> Option<AnyElement> {
    let k = w.screens.kcomment.as_ref()?;
    let c = |n: &str| theme::color(light, n);
    let pane = w.screens.kicad.as_ref().and_then(|p| p.read(cx).at_px).map(|b| (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height)));
    let (x, y) = place(pane, (f32::from(win.width), f32::from(win.height)));
    let (line, tone) = match names_line(&k.cap, 8) {
        Some(l) => (l, "text"),
        None if !k.err.is_empty() => (trs("KiCad could not say what is selected; the note goes alone"), "bad"),
        None => (trs("Reading the selection…"), "faint"),
    };
    let img = k.cap["image"].as_str().filter(|s| !s.is_empty()).map(|p| format!("/img?path={}", super::attach::enc(p)));
    let thumb = img.and_then(|u| match w.screens.images.get(w.screens.attacher.target.as_ref(), &u) {
        super::images::Slot::Ready(im) => Some(div().w(px(96.)).h(px(72.)).flex_none().overflow_hidden().bg(c("raise")).child(gpui::img(ImageSource::Image(im)).w(px(96.)).h(px(72.)).object_fit(ObjectFit::Contain)).into_any_element()),
        super::images::Slot::Loading => Some(div().w(px(96.)).h(px(72.)).flex_none().bg(c("raise")).into_any_element()),
        super::images::Slot::Failed => None,
    });
    let (opened, painted) = (k.opened, k.painted.clone());
    let card = div().id("kcomment").key_context("KComment").absolute().left(px(x)).top(px(y)).w(px(W)).flex().flex_col().gap_2().p_3()
        .bg(c("panel")).border_1().border_color(c("accent")).text_color(c("text")).text_size(px(13.)).occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        // the first paint after the key: how long the window took to appear (and where it is, for the e2e)
        .child(canvas(|_, _, _| (), move |b, _, w, _| { if !painted.swap(true, Ordering::Relaxed) { eprintln!("GPUI_LOG {}", json!({"kcomment": "painted", "ms": ms(opened), "at_ms": epoch_ms() as u64})); } crate::art::report("kcomment", b, w.scale_factor()); }).absolute().size_full())
        .child(div().id("kcomment-names").text_size(px(12.)).text_color(c(tone)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(line))
        .child(div().flex().gap_2().items_start().children(thumb)
            .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                .child(div().min_h(px(52.)).px_2().py_1().bg(c("bg")).border_1().border_color(c("line")).child(k.input.clone()))
                .child(div().text_size(px(11.)).text_color(c("faint")).child(trs("Enter posts to the thread · Esc cancels")))));
    Some(card.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn the_card_names_the_selection_once_the_capture_is_in() {
        assert_eq!(names_line(&Value::Null, 8), None, "nothing yet: the card says it is reading");
        let cap = json!({"names": {"references": ["U3", "R4"], "pins": ["U3.4"], "nets": ["/SDA"]}});
        assert_eq!(names_line(&cap, 8).unwrap(), "U3 · R4 · U3.4 · SDA");
        assert_eq!(names_line(&cap, 2).unwrap(), "U3 · R4 +2");
        assert!(names_line(&json!({"empty": true}), 8).unwrap().contains("Nothing selected"));
    }
    #[test] fn the_card_sits_over_the_pane_and_inside_the_window() {
        assert_eq!(place(Some((300., 40., 900., 700.)), (1400., 900.)), (300. + (900. - W) / 2., 40. + 700. * 0.18));
        let (x, y) = place(Some((1200., 850., 400., 400.)), (1400., 900.)); assert!(x + W <= 1400. && y + H <= 900.);
        assert_eq!(place(None, (1000., 800.)).0, (1000. - W) / 2.);
    }
    #[test] fn enter_posts_the_capture_with_the_note_or_the_note_alone() {
        let cap = json!({"project": "/p/a.kicad_pro", "frame": "FK_PCB", "names": {"references": ["U3"]}, "image": "/h/kicad-comments/a-c1.png", "note": ""});
        let p = post_params(&cap, " move it ", "t1", "/p/a.kicad_pro", "PCB").unwrap();
        assert_eq!((p["note"].as_str(), p["thread"].as_str(), p["names"]["references"][0].as_str(), p["image"].as_str()), (Some("move it"), Some("t1"), Some("U3"), Some("/h/kicad-comments/a-c1.png")));
        assert!(post_params(&cap, "", "t1", "", "PCB").is_some(), "the names alone are a comment");
        let alone = post_params(&Value::Null, "why so wide?", "t1", "/p/a.kicad_pro", "SCH").unwrap();
        assert_eq!((alone["frame"].as_str(), alone["note"].as_str()), (Some("FK_SCH"), Some("why so wide?")));
        assert!(post_params(&Value::Null, "  ", "t1", "", "PCB").is_none() && post_params(&json!({"names": {}}), "", "t1", "", "PCB").is_none(), "nothing to say");
    }
}
