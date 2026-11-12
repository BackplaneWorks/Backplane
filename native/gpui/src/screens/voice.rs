//! Dictation in the window (docs/voice.md). The hub decides everything: what the strip says, where the words go, what a chip does. This keeps a thread's mark (where dictated
//! words end, in characters), applies the hub's edits at it, and draws the hub's `view`. Square corners, theme tokens only.
use crate::{hub::Command, screens::{composer::{Composer, ComposerEvent}, settings::Secret}, theme, Backplane as Win};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Default)]
/// `cuts`: this session's sends while dictating, per thread; `seqs`: the number of the last edit written; `held`: edits that came after a send, kept until
/// the box is empty (the hub's answer to the send), then written at the new mark 0 (voice/dictation.rs `Reader::cut`).
pub struct Voice { pub views: HashMap<String, Value>, marks: HashMap<String, usize>, pub level: u32, pub fix: Option<Entity<Composer>>, upload: Option<super::voice_mic::Upload>,
    cuts: HashMap<String, u64>, seqs: HashMap<String, u64>, held: HashMap<String, (std::time::Instant, Vec<Value>)>,
    /// a voice command's message in its undo window; when the send button flashed for it; the line said after it ("Sent by voice") and until when
    pub pending: Option<Pending>, next: u64, pub flash: Option<std::time::Instant>, pub said: Option<(String, std::time::Instant)> }
/// A draft a voice command took (cmds.rs): `send` goes when the window ends, `cancel` is gone then; Undo puts the text back in front of the draft.
#[derive(Clone, Debug)]
pub struct Pending { pub id: u64, pub thread: String, pub text: String, pub cmd: String }
/// The undo window, and how long the send button flashes and the line after it stays.
pub const UNDO: std::time::Duration = std::time::Duration::from_secs(3);
pub const FLASH: std::time::Duration = std::time::Duration::from_millis(600);
const SAID: std::time::Duration = std::time::Duration::from_secs(2);
/// Undo: the taken text back in front of what was dictated since, and where the mark moves (as typing before it would move it).
pub fn undo_text(taken: &str, now: &str, mark: usize) -> (String, usize) {
    if now.trim().is_empty() { return (taken.to_string(), taken.chars().count()) }
    let sep = if taken.ends_with('\n') { "" } else { " " }; let t = format!("{taken}{sep}{now}");
    let n = t.chars().count() - now.chars().count(); (t, mark + n)
}
/// An edit held after a send waits at most this long for the box to empty; past it the send did not go and the edits are written at the end.
const HOLD: std::time::Duration = std::time::Duration::from_secs(3);
/// Whether an edit frame was made before this client's last send while dictating (the hub's `cut` count is behind ours): it belongs to nothing now.
pub fn stale(f: &Value, mine: u64) -> bool { f["cut"].as_u64().map_or(false, |c| c < mine) }

fn chars(s: &str) -> Vec<char> { s.chars().collect() }
fn byte_at(s: &str, c: usize) -> usize { s.char_indices().nth(c).map_or(s.len(), |(i, _)| i) }
/// The hub's edit applied at the mark: the byte range it replaces, the new mark and the new caret (characters). A mark past the end is the end.
/// `back`: characters before the mark replaced too (a voice command taking words back); the mark and caret are then counted from where that begins.
pub fn edit_range(text: &str, mark: usize, del: usize, adv: usize, to: usize, back: usize) -> (std::ops::Range<usize>, usize, usize) {
    let n = text.chars().count(); let m = mark.min(n); let end = (m + del).min(n); let s = m.saturating_sub(back);
    (byte_at(text, s)..byte_at(text, end), s + adv, s + to)
}
/// The person typed: the mark moves with what changed before it.
pub fn shift(old: &str, new: &str, mark: usize) -> usize {
    let (a, b) = (chars(old), chars(new));
    let p = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if p >= mark { return mark }
    (mark + b.len()).saturating_sub(a.len()).max(p)
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn rpc(w: &Win, p: Value) { let _ = w.tx.try_send(Command::Rpc("bots.voice".into(), Secret(p), "voice")); }
impl Win {
    fn voice_text(&self, thread: &str) -> String { self.drafts.get(thread).cloned().unwrap_or_default() }
    /// Sets a thread's draft from the hub's words: the open thread's box (its undo keeps it), or the kept draft of one not on screen.
    fn voice_put(&mut self, thread: &str, range: std::ops::Range<usize>, ins: &str, caret: usize, cx: &mut Context<Self>) {
        let mut text = self.voice_text(thread); if thread == self.selected { text = self.input.read(cx).content.to_string(); }
        text.replace_range(range.clone(), ins);
        if text.is_empty() { self.drafts.remove(thread); } else { self.drafts.insert(thread.to_string(), text.clone()); } // before the box changes: draft_changed sees no typing here
        self.screens.drafts.set(thread, &text);
        if thread == self.selected { let at = byte_at(&text, caret); self.input.update(cx, |i, cx| i.dictate(range, ins, at, cx)); self.send_draft(text, cx); }
    }
    /// A `{"t":"voice"}` frame from the hub.
    pub fn voice_frame(&mut self, f: Value, cx: &mut Context<Self>) {
        let thread = s(&f, "thread").to_string();
        match s(&f, "ev") {
            "view" => {
                let on = f["view"]["on"] == true; if !on { let v = &mut self.screens.voice; v.marks.remove(&thread); v.cuts.remove(&thread); v.seqs.remove(&thread); v.held.remove(&thread); v.level = 0; self.voice_mic_stop(); }
                if f["view"]["ask"]["fixing"] == true { self.voice_fix_open(s(&f["view"]["ask"]["field"], "value").to_string(), cx); } else { self.screens.voice.fix = None; }
                self.screens.voice.views.insert(thread, f["view"].clone());
            }
            "level" => self.screens.voice.level = f["v"].as_u64().unwrap_or(0) as u32,
            "edit" => {
                if stale(&f, self.screens.voice.cuts.get(&thread).copied().unwrap_or(0)) { return cx.notify() }
                if let Some((at, q)) = self.screens.voice.held.get_mut(&thread) { if at.elapsed() < HOLD { q.push(f); return cx.notify() } }
                self.voice_release(&thread, true, cx); self.voice_edit(&thread, &f, cx);
            }
            "draft" => {
                let (old, new) = (if thread == self.selected { self.input.read(cx).content.to_string() } else { self.voice_text(&thread) }, s(&f, "text").to_string());
                if let Some(m) = self.screens.voice.marks.get_mut(&thread) { *m = shift(&old, &new, *m); }
                let caret = new.chars().count(); self.voice_put(&thread, 0..old.len(), &new, caret, cx);
            }
            "audio" => self.voice_mic_send(s(&f, "path")),
            "command" => self.voice_command(&thread, &f, cx),
            _ => {}
        }
        cx.notify();
    }
    fn voice_edit(&mut self, thread: &str, f: &Value, cx: &mut Context<Self>) {
        let text = if thread == self.selected { self.input.read(cx).content.to_string() } else { self.voice_text(thread) };
        let mark = self.screens.voice.marks.get(thread).copied().unwrap_or(text.chars().count());
        let n = |k: &str| f[k].as_u64().unwrap_or(0) as usize;
        let (range, m, caret) = edit_range(&text, mark, n("del"), n("adv"), n("to"), n("back"));
        self.screens.voice.marks.insert(thread.to_string(), m);
        if let Some(q) = f["seq"].as_u64() { self.screens.voice.seqs.insert(thread.to_string(), q); }
        self.voice_put(thread, range, s(f, "ins"), caret, cx);
    }
    /// The edits held since a send, written now: into the empty box (`sent`), or at the end of what is there when the send never went.
    fn voice_release(&mut self, thread: &str, late: bool, cx: &mut Context<Self>) {
        let Some((_, q)) = self.screens.voice.held.remove(thread) else { return };
        if late { let n = if thread == self.selected { self.input.read(cx).content.chars().count() } else { self.voice_text(thread).chars().count() }; self.screens.voice.marks.insert(thread.to_string(), n); }
        for f in q { self.voice_edit(thread, &f, cx) }
    }
    /// Send (or Steer) while dictating: what is in the box goes with the message, and so do the words the hub sent up to here; what is said after them
    /// starts the next draft. `hold`: the box empties only when the hub has the message, so edits meanwhile wait (`voice_sent` writes them).
    pub fn voice_cut(&mut self, hold: bool) {
        let t = self.selected.clone(); if !self.screens.voice.views.get(&t).map_or(false, |v| v["on"] == true) { return }
        let v = &mut self.screens.voice; let n = { let c = v.cuts.entry(t.clone()).or_insert(0); *c += 1; *c }; let seq = v.seqs.get(&t).copied().unwrap_or(0);
        v.marks.insert(t.clone(), 0); if hold { v.held.insert(t.clone(), (std::time::Instant::now(), vec![])); }
        rpc(self, json!({"op": "act", "thread": t, "action": "voice-clear", "value": "", "draft": "", "cut": n, "seq": seq}));
    }
    /// A voice command (the hub's `command` frame): "send" or "cancel". The whole draft is taken from the box now (dictation goes on into the empty
    /// box), the send button flashes, and the strip says so with Undo; when the window ends a send goes through the outbox (turn.start, mode steer: it
    /// steers a running turn as the button does, and waits for the hub when it is away), a cancel is dropped.
    fn voice_command(&mut self, thread: &str, f: &Value, cx: &mut Context<Self>) {
        let cmd = s(f, "cmd").to_string(); if !matches!(cmd.as_str(), "send" | "cancel") { return }
        let v = &mut self.screens.voice; let c = v.cuts.entry(thread.to_string()).or_insert(0); *c = (*c).max(f["cut"].as_u64().unwrap_or(0));
        v.marks.insert(thread.to_string(), 0); v.held.remove(thread);
        if let Some(p) = self.screens.voice.pending.as_ref().map(|p| p.id) { self.voice_deliver(p, cx) } // the one before goes now
        let text = if thread == self.selected { self.input.read(cx).content.to_string() } else { self.voice_text(thread) };
        if text.trim().is_empty() { return }
        self.voice_put(thread, 0..text.len(), "", 0, cx); self.screens.voice.marks.insert(thread.to_string(), 0);
        let v = &mut self.screens.voice; v.next += 1; let id = v.next;
        v.pending = Some(Pending { id, thread: thread.to_string(), text, cmd: cmd.clone() }); if cmd == "send" { v.flash = Some(std::time::Instant::now()) }
        cx.spawn(async move |w, cx| { cx.background_executor().timer(FLASH).await; let _ = w.update(cx, |_, cx| cx.notify()); cx.background_executor().timer(UNDO - FLASH).await; let _ = w.update(cx, |w, cx| w.voice_deliver(id, cx)); }).detach();
    }
    /// The undo window of command `id` ended (or another came): a send goes, a cancel is gone.
    pub fn voice_deliver(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(p) = self.screens.voice.pending.take_if(|p| p.id == id) else { return };
        let line = if p.cmd == "send" {
            let th = p.thread.rsplit('|').next().unwrap_or("").to_string();
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
            if p.thread == self.selected { self.echo_sent(&p.text); }
            /* i18n-ok: drawn through trs in pending_view */ self.screens.unsent.push(&th, &p.text, now); self.send_unsent(); "Sent by voice"
        /* i18n-ok: drawn through trs in pending_view */ } else { "Draft cleared by voice" };
        self.screens.voice.said = Some((line.to_string(), std::time::Instant::now()));
        cx.spawn(async move |w, cx| { cx.background_executor().timer(SAID).await; let _ = w.update(cx, |_, cx| cx.notify()); }).detach();
        cx.notify();
    }
    /// Undo: the message stays; its text goes back in front of whatever was dictated since.
    pub fn voice_undo(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.screens.voice.pending.take() else { return };
        let now = if p.thread == self.selected { self.input.read(cx).content.to_string() } else { self.voice_text(&p.thread) };
        let mark = self.screens.voice.marks.get(&p.thread).copied().unwrap_or(0);
        let (text, m) = undo_text(&p.text, &now, mark); let caret = text.chars().count();
        self.voice_put(&p.thread, 0..now.len(), &text, caret, cx);
        if self.screens.voice.marks.contains_key(&p.thread) { self.screens.voice.marks.insert(p.thread.clone(), m); }
        self.screens.voice.flash = None; cx.notify();
    }
    /// The person typed in the open thread: `draft_changed` calls this with the old and new text.
    pub fn voice_typed(&mut self, old: &str, new: &str) { let t = self.selected.clone(); if let Some(m) = self.screens.voice.marks.get_mut(&t) { *m = shift(old, new, *m); } }
    /// The mic chip: start listening into the open thread, or stop.
    pub fn voice_press(&mut self, cx: &mut Context<Self>) {
        let thread = self.selected.clone(); if thread.is_empty() { return }
        if self.screens.voice.views.get(&thread).map_or(false, |v| v["on"] == true) { self.voice_mic_end(); rpc(self, json!({"op": "stop", "thread": thread})); return }
        let i = self.input.read(cx); let text = i.content.to_string(); let head = i.editor().head(); let caret = text[..head.min(text.len())].chars().count();
        self.screens.voice.marks.insert(thread.clone(), caret); self.screens.voice.cuts.remove(&thread); self.screens.voice.seqs.remove(&thread); self.screens.voice.held.remove(&thread);
        let src = if self.voice_mic_begin() { "client" } else { "hub" };
        rpc(self, json!({"op": "start", "thread": thread, "draft": text, "caret": caret, "src": src})); cx.notify();
    }
    pub fn voice_act(&mut self, action: &str, value: &str, cx: &mut Context<Self>) {
        let t = self.selected.clone(); let draft = self.input.read(cx).content.to_string();
        rpc(self, json!({"op": "act", "thread": t, "action": action, "value": value, "draft": draft}));
    }
    /// The message went: what was left to clarify in it goes too.
    pub fn voice_sent(&mut self, cx: &mut Context<Self>) { let t = self.selected.clone(); self.voice_release(&t, false, cx); let v = self.screens.voice.views.get(&self.selected); if v.map_or(false, |v| !v["ask"].is_null() || v["fixes"].as_array().map_or(false, |a| !a.is_empty())) { self.voice_act("voice-clear", "", cx); } }
    fn voice_fix_open(&mut self, seed: String, cx: &mut Context<Self>) {
        if self.screens.voice.fix.is_some() { return }
        /* i18n-ok: the hint, drawn through trs by the composer */ let input = cx.new(|cx| { let mut c = Composer::with_context(cx, "AskInput"); c.set_placeholder("The right spelling"); c.set(seed, cx); c });
        cx.subscribe(&input, |this, i, e: &ComposerEvent, cx| if matches!(e, ComposerEvent::Submit) { let t = i.read(cx).content.to_string(); this.voice_act("voice-fixed", &t, cx); }).detach();
        self.screens.voice.fix = Some(input);
    }
}

fn button(cx: &mut Context<Win>, light: bool, id: SharedString, label: String, primary: bool, tip: bool, f: impl Fn(&mut Win, &mut Context<Win>) + 'static) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n); let _ = tip;
    div().id(id).px_2().py_1().cursor_pointer().border_1().border_color(c(if primary { "accent" } else { "line" })).bg(c(if primary { "accent" } else { "panel" })).text_color(c(if primary { "on-accent" } else { "text" }))
        .text_size(px(12.)).whitespace_nowrap().child(crate::i18n::trs(&label)).on_click(cx.listener(move |this, _, _, cx| f(this, cx)))
}
/// What shows above the composer for this thread: a strip while on, a note when something is wrong, the fixes as chips that undo, the word to clarify.
pub fn view(cx: &mut Context<Win>, light: bool, v: &Value, level: u32, fix: Option<&Entity<Composer>>) -> Option<AnyElement> {
    let (note, on, ask) = (s(v, "note"), v["on"] == true, &v["ask"]); let fixes = v["fixes"].as_array().cloned().unwrap_or_default();
    if !on && note.is_empty() && fixes.is_empty() && ask.is_null() { return None }
    let c = |n: &str| theme::color(light, n);
    let mut col = div().id("voice").flex().flex_col().gap_1().text_size(px(12.));
    if !note.is_empty() {
        col = col.child(div().flex().items_center().gap_2().child(div().flex_1().min_w_0().text_color(c(if v["bad"] == true { "bad" } else { "dim" })).child(crate::i18n::trs(note)))
            /* i18n-ok: drawn through trs in button */ .child(button(cx, light, "voice-err".into(), "Close".into(), false, false, |w, cx| w.voice_act("voice-err", "", cx))));
    }
    if on {
        col = col.child(div().id("voice-strip").flex().items_center().gap_2().child(div().size(px(8.)).bg(c("bad"))).child(div().text_color(c("dim")).child(crate::i18n::trs(s(v, "status"))))
            .child(div().w(px(72.)).h(px(4.)).bg(c("line")).child(div().h_full().w(px(0.72 * level.min(100) as f32)).bg(c("accent"))))
            .child(div().flex_1()).child(button(cx, light, "voice-stop".into(), s(v, "stop").to_string(), false, false, |w, cx| w.voice_press(cx))));
    }
    if !fixes.is_empty() {
        let mut row = div().flex().flex_wrap().gap_1();
        for (i, f) in fixes.iter().enumerate() { let (a, val) = (s(f, "action").to_string(), s(f, "value").to_string()); row = row.child(button(cx, light, format!("voice-fix-{i}").into(), s(f, "label").to_string(), false, true, move |w, cx| w.voice_act(&a, &val, cx))); }
        col = col.child(row);
    }
    if !ask.is_null() {
        let mut row = div().flex().flex_wrap().items_center().gap_1().child(div().text_color(c("dim")).child(crate::i18n::trs(s(ask, "text"))));
        if let (true, Some(i)) = (ask["fixing"] == true, fix) { row = row.child(div().w(px(220.)).px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(i.clone())); }
        for (n, b) in ask["buttons"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let (a, val, input) = (s(b, "action").to_string(), s(b, "value").to_string(), fix.cloned());
            row = row.child(button(cx, light, format!("voice-ask-{n}").into(), s(b, "label").to_string(), b["primary"] == true, false, move |w, cx| { let typed = if a == "voice-fixed" { input.as_ref().map(|i| i.read(cx).content.to_string()) } else { None }; w.voice_act(&a, typed.as_deref().unwrap_or(&val), cx) }));
        }
        col = col.child(row);
    }
    Some(col.into_any_element())
}
/// Over the composer after a voice command: its undo window ("Sending by voice…" with Undo), then for a moment what happened.
pub fn pending_view(cx: &mut Context<Win>, light: bool, v: &Voice, thread: &str) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    if let Some(p) = v.pending.as_ref().filter(|p| p.thread == thread) {
        /* i18n-ok: drawn through trs below */ let line = if p.cmd == "send" { "Sending by voice…" } else { "Draft cleared by voice" };
        return Some(div().id("voice-pending").flex().items_center().gap_2().text_size(px(12.)).child(div().size(px(8.)).bg(c(if p.cmd == "send" { "ok" } else { "warn" }))).child(div().flex_1().text_color(c("dim")).child(crate::i18n::trs(line)))
            /* i18n-ok: drawn through trs in button */ .child(button(cx, light, "voice-undo".into(), "Undo".into(), true, false, |w, cx| w.voice_undo(cx))).into_any_element());
    }
    let (line, at) = v.said.as_ref()?; if at.elapsed() > SAID { return None }
    Some(div().id("voice-said").text_size(px(12.)).text_color(c("dim")).child(crate::i18n::trs(line)).into_any_element())
}
/// The send button flashes for a voice send.
pub fn flashing(v: &Voice) -> bool { v.flash.map_or(false, |t| t.elapsed() < FLASH) }
impl Win { pub(crate) fn voice_upload(&mut self) -> &mut Option<super::voice_mic::Upload> { &mut self.screens.voice.upload } }

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    fn run(text: &str, mark: usize, del: usize, ins: &str, adv: usize, to: usize) -> (String, usize, usize) { runb(text, mark, del, ins, adv, to, 0) }
    fn runb(text: &str, mark: usize, del: usize, ins: &str, adv: usize, to: usize, back: usize) -> (String, usize, usize) { let (r, m, c) = edit_range(text, mark, del, adv, to, back); let mut t = text.to_string(); t.replace_range(r, ins); (t, m, c) }
    // the hub's own vectors (voice/place.rs): an edit is replace `del` at the mark, move the mark `adv`, the caret `to`
    #[test] fn undo_puts_the_message_back_in_front_of_what_was_said_since() {
        assert_eq!(undo_text("Buy milk.", "", 0), ("Buy milk.".into(), 9));
        assert_eq!(undo_text("Buy milk.", "And eggs.", 9), ("Buy milk. And eggs.".into(), 19), "the mark moves as typing before it would");
        assert_eq!(undo_text("Lines\n", "next", 4), ("Lines\nnext".into(), 10));
    }
    #[test] fn a_voice_send_goes_as_the_button_does_steering_a_running_turn() {
        // the outbox's request: turn.start, mode steer, to the command's thread whichever is open (the hub steers a running turn, else starts one)
        let mut u = crate::screens::unsent::Unsent::default(); let msg = u.push("t9", "Buy milk.", 1);
        let i = u.next().unwrap(); assert_eq!(crate::screens::unsent::Unsent::request(&i), json!({"thread": "t9", "text": "Buy milk.", "msg": msg, "mode": "steer"}));
    }
    #[test] fn an_edit_made_before_this_clients_send_is_dropped() {
        assert!(stale(&json!({"cut": 0}), 1), "made before the hub heard of the send"); assert!(!stale(&json!({"cut": 1}), 1)); assert!(!stale(&json!({"cut": 2}), 1));
        assert!(!stale(&json!({}), 3), "a hub without cuts: every edit is written");
    }
    #[test] fn an_edit_replaces_the_tail_at_the_mark() {
        assert_eq!(run("see ", 4, 0, "open the board", 4, 14), ("see open the board".into(), 8, 18));
        assert_eq!(run("see open the board", 8, 10, " the board in Kai", 10, 17), ("see open the board in Kai".into(), 18, 25));
        assert_eq!(run("héllo wörld", 5, 6, "", 0, 0), ("héllo".into(), 5, 5), "characters, not bytes");
    }
    /// The clients' shared fixture (test/fixtures/voice-edits.json, its edits generated by the hub): the window must land where the hub's shadow does.
    #[test] fn the_shared_fixture() {
        let f: serde_json::Value = serde_json::from_str(include_str!("../../../../test/fixtures/voice-edits.json")).unwrap();
        let u = |v: &serde_json::Value| v.as_u64().unwrap() as usize;
        for c in f["edits"].as_array().unwrap() {
            let w = &c["want"]; let (mut text, mut mark, mut at) = (c["draft"].as_str().unwrap().to_string(), u(&c["caret"]), u(&c["caret"]));
            for e in w["edits"].as_array().unwrap() { (text, mark, at) = run(&text, mark, u(&e["del"]), e["ins"].as_str().unwrap(), u(&e["adv"]), u(&e["to"])); }
            assert_eq!((text.as_str(), mark, at), (w["text"].as_str().unwrap(), u(&w["mark"]), u(&w["caret"])), "{}", c["name"]);
        }
        for c in f["apply"].as_array().unwrap() {
            let (e, w) = (&c["edit"], &c["want"]);
            assert_eq!(runb(c["text"].as_str().unwrap(), u(&c["mark"]), u(&e["del"]), e["ins"].as_str().unwrap(), u(&e["adv"]), u(&e["to"]), e["back"].as_u64().unwrap_or(0) as usize), (w["text"].as_str().unwrap().to_string(), u(&w["mark"]), u(&w["caret"])), "{}", c["name"]);
        }
        for c in f["shift"].as_array().unwrap() { assert_eq!(shift(c["old"].as_str().unwrap(), c["new"].as_str().unwrap(), u(&c["mark"])), u(&c["want"]), "{}", c["name"]); }
    }
    #[test] fn a_mark_past_the_end_is_the_end() { assert_eq!(run("ab", 9, 3, "c", 1, 1), ("abc".into(), 3, 3)); }
    #[test] fn typing_before_the_mark_moves_it_and_after_it_does_not() {
        assert_eq!(shift("hello world", "XXhello world", 5), 7); assert_eq!(shift("hello world", "hello worldXX", 5), 5); assert_eq!(shift("hello world", "ello world", 5), 4);
        assert_eq!(shift("hello", "", 3), 0, "all of it gone");
    }
}
