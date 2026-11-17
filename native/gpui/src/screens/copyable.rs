//! The copy button: one small icon wherever text is there to be copied (a code block, an inline command, a path, an id, a commit, a pairing
//! code), as GitHub draws on a README's code. It shows while the pointer is over what it copies (or it has the keyboard), copies the exact
//! text on a click, Enter or Ctrl/Cmd+C (no markdown, no trailing newline, a block whole), and turns into a check for a moment.
//! A shell command gets Run in terminal beside it: the chat's terminal opens with the command typed in, not run (screens/term.rs).
//! The clipboard is the platform's (vendor/gpui: X11, Wayland, Windows, macOS), written from the click or key press that asked for it.
//! What is masked on screen in streamer mode stays masked there; the copy is the real text, which the person asked for.
use crate::{art, theme, Backplane};
use gpui::{prelude::*, *};
use std::{cell::RefCell, collections::HashMap, time::{Duration, Instant}};

actions!(copyable, [CopyIt, RunIt]);
/// The keys of a copy button that has the keyboard (Enter, Ctrl/Cmd+C) and of Run in terminal (Enter). Bound in their own contexts, deeper than
/// the window's Ctrl+C (copy the timeline's selection), so they win there and nowhere else.
pub fn bindings() -> Vec<KeyBinding> {
    vec![KeyBinding::new("enter", CopyIt, Some("Copyable")), KeyBinding::new("secondary-c", CopyIt, Some("Copyable")), KeyBinding::new("enter", RunIt, Some("CopyRun"))]
}

/// How long the check mark stays after a copy.
pub const SHOWN: Duration = Duration::from_millis(1500);
/// Buttons remembered for their keyboard and their check mark (the oldest go past this).
const KEEP: usize = 512;

thread_local! {
    static COPIED: RefCell<HashMap<SharedString, Instant>> = RefCell::new(HashMap::new());
    static FOCUS: RefCell<HashMap<SharedString, (FocusHandle, u64)>> = RefCell::new(HashMap::new());
    static TICK: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The exact text a copy puts on the clipboard: what is shown, with no line breaks after it (a block's last newline is not part of it).
pub fn exact(text: &str) -> String { text.trim_end_matches(['\n', '\r']).to_string() }

/// Whether the button `id` was used within SHOWN of `now` (it shows the check).
pub fn copied_at(at: Option<Instant>, now: Instant) -> bool { at.is_some_and(|t| now.saturating_duration_since(t) < SHOWN) }
pub fn just_copied(id: &str) -> bool { COPIED.with(|c| copied_at(c.borrow().get(id).copied(), Instant::now())) }

/// The keyboard of the button `id`, made once and kept while it is drawn.
pub fn focus_of(id: &SharedString, cx: &mut App) -> FocusHandle {
    let tick = TICK.with(|t| { t.set(t.get() + 1); t.get() });
    FOCUS.with(|f| {
        let mut f = f.borrow_mut();
        if let Some((h, used)) = f.get_mut(id) { *used = tick; return h.clone() }
        if f.len() >= KEEP { if let Some(old) = f.iter().min_by_key(|(_, (_, u))| *u).map(|(k, _)| k.clone()) { f.remove(&old); } }
        let h = cx.focus_handle().tab_stop(true);
        f.insert(id.clone(), (h.clone(), tick)); h
    })
}
pub fn focused(id: &SharedString, window: &Window) -> bool { FOCUS.with(|f| f.borrow().get(id).is_some_and(|(h, _)| h.is_focused(window))) }

/// Copy `text` for the button `id`: the clipboard, the check mark, and one more frame when the check is to go (no repaint loop).
pub fn copy(id: &SharedString, text: &str, window: &mut Window, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(exact(text)));
    COPIED.with(|c| { let mut c = c.borrow_mut(); let now = Instant::now(); c.retain(|_, t| copied_at(Some(*t), now)); c.insert(id.clone(), now); });
    window.refresh();
    let w = window.window_handle();
    cx.spawn(async move |cx| { cx.background_executor().timer(SHOWN + Duration::from_millis(60)).await; let _ = w.update(cx, |_, window, _| window.refresh()); }).detach();
}

pub struct Opts {
    /// The word beside the icon ("Copy" on a code block's bar); icon alone otherwise.
    pub word: bool,
    /// Drawn whatever the pointer does (a code block's bar, a key in Settings); else only while `shown` says.
    pub always: bool,
}

/// The copy button `id` for `text`. Its rectangle is reported as `copy:<id>` for the window tests.
pub fn button(light: bool, id: impl Into<SharedString>, text: impl Into<SharedString>, opts: Opts, cx: &mut App) -> Stateful<Div> {
    let (id, text): (SharedString, SharedString) = (id.into(), text.into());
    let c = |n: &str| theme::color(light, n);
    let done = just_copied(&id);
    let focus = focus_of(&id, cx);
    let (i1, t1, i2, t2) = (id.clone(), text.clone(), id.clone(), text.clone());
    let probe = { let name = format!("copy:{id}"); canvas(|_, _, _| (), move |b, _, w, _| { if art::rects_on() { art::report(&name, b, w.scale_factor()) } }).absolute().size_full() };
    let label = if done { crate::i18n::trs("Copied") } else { crate::i18n::trs("Copy") };
    div().id(SharedString::from(format!("copy-{id}"))).relative().flex().items_center().gap_1().px_1().h(crate::scale::tpx(18.)).flex_shrink_0()
        .cursor_pointer().text_size(crate::scale::tpx(11.)).text_color(c(if done { "ok" } else { "dim" })).hover(|s| s.text_color(c("text")).bg(c("raise")))
        .track_focus(&focus)
        // hidden until the pointer is over what it copies (`value`'s group) or it has the keyboard; shown while its check is
        .when(!opts.always && !done, |d| d.invisible().focus(|s| s.visible().border_1().border_color(c("accent"))))
        .when(opts.always || done, |d| d.focus(|s| s.border_1().border_color(c("accent"))))
        .when(!opts.word, |d| d.tooltip(crate::keys::tip(&label, "", light)))
        .child(art::icon(if done { "check" } else { "copy" }, 13., c(if done { "ok" } else { "dim" })))
        .when(opts.word, |d| d.child(label.clone()))
        .child(probe)
        .on_mouse_down(MouseButton::Left, move |_, window, cx| { window.focus(&focus); cx.stop_propagation(); })
        .on_click(move |_, window, cx| { cx.stop_propagation(); copy(&i1, &t1, window, cx) })
        .key_context("Copyable").on_action(move |_: &CopyIt, window, cx| copy(&i2, &t2, window, cx))
}

/// Run in terminal beside a command's copy button: the chat's terminal opens with `cmd` typed in, waiting for the person's Enter.
pub fn run_button(light: bool, id: impl Into<SharedString>, cmd: impl Into<SharedString>, word: bool, app: WeakEntity<Backplane>, cx: &mut App) -> Stateful<Div> {
    let (id, cmd): (SharedString, SharedString) = (id.into(), cmd.into());
    let c = |n: &str| theme::color(light, n);
    let focus = focus_of(&SharedString::from(format!("run:{id}")), cx);
    let (a1, c1, a2, c2) = (app.clone(), cmd.clone(), app, cmd.clone());
    let probe = { let name = format!("run:{id}"); canvas(|_, _, _| (), move |b, _, w, _| { if art::rects_on() { art::report(&name, b, w.scale_factor()) } }).absolute().size_full() };
    let label = crate::i18n::trs("Run in terminal");
    div().id(SharedString::from(format!("run-{id}"))).relative().flex().items_center().gap_1().px_1().h(crate::scale::tpx(18.)).flex_shrink_0()
        .cursor_pointer().text_size(crate::scale::tpx(11.)).text_color(c("dim")).hover(|s| s.text_color(c("text")).bg(c("raise")))
        .track_focus(&focus).focus(|s| s.border_1().border_color(c("accent")))
        .when(!word, |d| d.tooltip(crate::keys::tip(&label, "", light)))
        .child(art::icon("terminal", 13., c("dim")))
        .when(word, |d| d.child(label.clone()))
        .child(probe)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| { cx.stop_propagation(); let _ = a1.update(cx, |this, cx| this.run_in_terminal(&c1, window, cx)); })
        .key_context("CopyRun").on_action(move |_: &RunIt, window, cx| { let _ = a2.update(cx, |this, cx| this.run_in_terminal(&c2, window, cx)); })
}

/// A labelled copy press of the hub's (Settings' Copy link, Copy code, setup's Copy beside a command: action `copy`): the same copy, check
/// and keyboard as `button`, on the chip `d` the caller styled. The label says Copied for a moment.
pub fn chip(d: Stateful<Div>, light: bool, label: String, id: impl Into<SharedString>, text: impl Into<SharedString>, cx: &mut App) -> Stateful<Div> {
    let (id, text): (SharedString, SharedString) = (id.into(), text.into());
    let c = |n: &str| theme::color(light, n);
    let done = just_copied(&id);
    let focus = focus_of(&id, cx);
    let (i1, t1, i2, t2) = (id.clone(), text.clone(), id, text);
    d.flex().items_center().gap_1().track_focus(&focus).focus(|s| s.border_1().border_color(c("accent")))
        .child(art::icon(if done { "check" } else { "copy" }, 12., c(if done { "ok" } else { "dim" })))
        .child(if done { crate::i18n::trs("Copied") } else { label })
        .on_mouse_down(MouseButton::Left, move |_, window, _| window.focus(&focus))
        .on_click(move |_, window, cx| { cx.stop_propagation(); copy(&i1, &t1, window, cx) })
        .key_context("Copyable").on_action(move |_: &CopyIt, window, cx| copy(&i2, &t2, window, cx))
}

/// The pieces of a plain text to copy, as the hub lists them ({text, cmd}: a room's post, a notice): each one shown in code type (masked in
/// streamer mode) with its copy button, and Run in terminal beside a command. None when there are none.
pub fn pieces_with(light: bool, key: &str, list: &serde_json::Value, app: &WeakEntity<Backplane>, cx: &mut App) -> Option<Div> {
    let c = |n: &str| theme::color(light, n);
    let items: Vec<AnyElement> = list.as_array()?.iter().take(16).enumerate().map(|(i, k)| {
        let (t, cmd) = (k["text"].as_str().unwrap_or("").to_string(), k["cmd"] == true); let id = format!("{key}-{i}");
        let shown = div().px_1().font(art::mono_font()).text_size(crate::scale::tpx(11.)).text_color(c("text")).bg(c("raise")).border_1().border_color(c("line")).max_w(px(560.)).overflow_hidden().whitespace_nowrap().text_ellipsis()
            .child(crate::streamer::same_width(t.lines().next().unwrap_or(""), false).unwrap_or_else(|| t.lines().next().unwrap_or("").to_string()) + if t.contains('\n') { " …" } else { "" });
        div().flex().items_center().gap_1().min_w_0()
            .child(value(light, id.clone(), shown, t.clone(), true, cx))
            .when(cmd, |d| d.child(run_button(light, id.clone(), t.clone(), false, app.clone(), cx))).into_any_element()
    }).collect();
    (!items.is_empty()).then(|| div().flex().flex_col().gap_1().py_1().children(items))
}
pub fn pieces(cx: &mut Context<Backplane>, light: bool, key: &str, list: &serde_json::Value) -> Option<Div> { let app = cx.entity().downgrade(); pieces_with(light, key, list, &app, cx) }

/// A value shown to be copied (a path, an id, a link, a pairing code): `shown` (masked in streamer mode by the caller, or the text itself), and
/// its copy button, which appears while the pointer is over the row or the button has the keyboard. `always`: the button stays (a touch screen's way).
pub fn value(light: bool, id: impl Into<SharedString>, shown: impl IntoElement, text: impl Into<SharedString>, always: bool, cx: &mut App) -> Stateful<Div> {
    let id: SharedString = id.into();
    let group = SharedString::from(format!("cpv-{id}"));
    let b = button(light, id.clone(), text, Opts { word: false, always }, cx).group_hover(group.clone(), |s| s.visible());
    div().id(SharedString::from(format!("cpvrow-{id}"))).group(group).flex().items_center().gap_1().min_w_0()
        .child(div().min_w_0().child(shown))
        .child(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_copy_is_the_text_with_no_line_break_after_it() {
        assert_eq!(exact("sudo apt-get install -y x\n"), "sudo apt-get install -y x");
        assert_eq!(exact("a\nb\n\n"), "a\nb");
        assert_eq!(exact("  keep spaces  "), "  keep spaces  ");
        assert_eq!(exact("line\r\n"), "line");
    }
    #[test] fn the_check_shows_for_a_moment() {
        let t = Instant::now();
        assert!(copied_at(Some(t), t + Duration::from_millis(100)));
        assert!(!copied_at(Some(t), t + SHOWN));
        assert!(!copied_at(None, t));
    }
}
