//! Voice commands (pure, docs/voice.md "Voice commands"): what said at the very end of an utterance (the segment the VAD closed after a pause) does
//! something instead of being written. "Message complete" (and "send it", "send message", or the person's own phrases) sends the message, or steers
//! the running turn: the same Send as the button. Off unless turned on: "scratch that" (the last sentence goes), "new line", "cancel message" (the
//! draft goes), and a send after some seconds of silence. Only the end of an utterance counts, so "send it to Kati" never sends.
use super::page::Prefs;
use super::words::{alnum, lower};

pub const SEND: &str = "voice.cmd.send";
pub const SCRATCH: &str = "voice.cmd.scratch";
pub const NEWLINE: &str = "voice.cmd.newline";
pub const CANCEL: &str = "voice.cmd.cancel";
pub const AUTO: &str = "voice.cmd.auto";
/// The send phrases when the person has not set their own.
pub const SEND_DEFAULT: &str = "message complete, send it, send message";
/// Auto-send's choices, seconds of silence.
pub const AUTO_CHOICES: [u32; 3] = [3, 5, 10];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd { Send, Scratch, NewLine, Cancel }
impl Cmd { pub fn name(self) -> &'static str { match self { Cmd::Send => "send", Cmd::Scratch => "scratch", Cmd::NewLine => "newline", Cmd::Cancel => "cancel" } } }

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cmds { pub phrases: Vec<(Cmd, Vec<String>)>, pub auto_s: u32 }

/// A word as compared: lower case, letters and digits only ("Complete." is "complete").
pub fn norm(w: &str) -> String { lower(w).chars().filter(|c| alnum(*c)).collect() }
/// The phrases a setting holds: comma or line separated, each its normalised words.
pub fn phrases(text: &str) -> Vec<Vec<String>> {
    text.split([',', '\n', ';']).map(|p| p.split_whitespace().map(norm).filter(|w| !w.is_empty()).collect::<Vec<_>>()).filter(|p| !p.is_empty()).collect()
}
/// The send phrases as the setting says: its own, the defaults when it is unset, none when it is "-" (sending by voice turned off).
pub fn send_text(p: &Prefs) -> String { match p.get(SEND).map(|s| s.trim()) { None | Some("") => SEND_DEFAULT.to_string(), Some("-") => String::new(), Some(s) => s.to_string() } }
fn on(p: &Prefs, k: &str) -> bool { p.get(k).map_or(false, |v| v == "on") }

impl Cmds {
    pub fn of(p: &Prefs) -> Cmds {
        let mut v: Vec<(Cmd, Vec<String>)> = phrases(&send_text(p)).into_iter().map(|x| (Cmd::Send, x)).collect();
        if on(p, SCRATCH) { v.push((Cmd::Scratch, phrases("scratch that")[0].clone())) }
        if on(p, NEWLINE) { v.push((Cmd::NewLine, phrases("new line")[0].clone())); v.push((Cmd::NewLine, phrases("newline")[0].clone())) }
        if on(p, CANCEL) { v.push((Cmd::Cancel, phrases("cancel message")[0].clone())) }
        // the longest first: "send message" before "send it" cannot clash, but a person's "send" must not win over "send it"
        v.sort_by_key(|(_, x)| std::cmp::Reverse(x.len()));
        Cmds { phrases: v, auto_s: p.get(AUTO).and_then(|s| s.parse().ok()).unwrap_or(0) }
    }
    /// The command the utterance `words` ends with, and how many of its words are the command's.
    pub fn at_end<S: AsRef<str>>(&self, words: &[S]) -> Option<(Cmd, usize)> {
        let ws: Vec<String> = words.iter().map(|w| norm(w.as_ref())).filter(|w| !w.is_empty()).collect();
        // a word that was only punctuation is not counted, so the count is of the words as given
        let given: Vec<usize> = words.iter().enumerate().filter(|(_, w)| !norm(w.as_ref()).is_empty()).map(|(i, _)| i).collect();
        for (c, p) in &self.phrases {
            if p.len() > ws.len() { continue }
            let tail = &ws[ws.len() - p.len()..];
            if tail.iter().zip(p).all(|(h, w)| same(h, w)) { return Some((*c, words.len() - given[ws.len() - p.len()])) }
        }
        None
    }
    /// What a recording (a ring's note) says without a send phrase at its end: the note is already its own message.
    pub fn strip_send(&self, text: &str) -> String {
        let ws: Vec<&str> = text.split_whitespace().collect();
        match self.at_end(&ws) { Some((Cmd::Send, k)) => ws[..ws.len() - k].join(" ").trim_end_matches([',', ';', ':', '-']).trim().to_string(), _ => text.trim().to_string() }
    }
}
/// A heard word is the phrase's word: the same, or the same with a heard ending (complete → completed, completes), only for words long enough that
/// the ending cannot make another word ("send" is never "sends" or "sent": "I sent it." is not a command).
fn same(heard: &str, want: &str) -> bool {
    if heard == want { return true }
    want.chars().count() >= 6 && heard.strip_prefix(want).map_or(false, |rest| matches!(rest, "d" | "s" | "ed" | "es"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prefs(kv: &[(&str, &str)]) -> Prefs { kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    fn ws(s: &str) -> Vec<String> { s.split_whitespace().map(String::from).collect() }
    fn end(c: &Cmds, s: &str) -> Option<(Cmd, usize)> { c.at_end(&ws(s)) }
    #[test] fn the_send_phrases_match_at_the_end_of_an_utterance_whatever_the_engine_made_of_them() {
        let c = Cmds::of(&prefs(&[]));
        for s in ["Message complete.", "message complete", "Message completed.", "MESSAGE COMPLETE!", "So that's it. Message complete.", "Send it.", "send message", "Okay, send it."] {
            assert_eq!(end(&c, s).map(|x| x.0), Some(Cmd::Send), "{s}");
        }
        assert_eq!(end(&c, "So that's it. Message complete."), Some((Cmd::Send, 2)));
        assert_eq!(end(&c, "Okay, send it ."), Some((Cmd::Send, 3)), "a lone mark after it is the command's too");
    }
    #[test] fn never_in_the_middle_never_a_lookalike() {
        let c = Cmds::of(&prefs(&[]));
        for s in ["Send it to Kati.", "I'll send it to Kati tomorrow", "The message completes the set and more", "I sent it.", "Sends it.", "message", "complete", "", "Is the message complete or not"] {
            assert_eq!(end(&c, s), None, "{s:?}");
        }
    }
    #[test] fn the_person_sets_their_own_phrases_or_turns_sending_by_voice_off() {
        let c = Cmds::of(&prefs(&[(SEND, "over and out, ship it")]));
        assert_eq!(end(&c, "That's all, over and out."), Some((Cmd::Send, 3))); assert_eq!(end(&c, "Ship it!"), Some((Cmd::Send, 2))); assert_eq!(end(&c, "Message complete."), None, "their own replace the defaults");
        assert!(Cmds::of(&prefs(&[(SEND, "-")])).phrases.is_empty());
        assert_eq!(Cmds::of(&prefs(&[(SEND, "")])), Cmds::of(&prefs(&[])), "blank is the default");
    }
    #[test] fn the_extras_are_off_until_turned_on() {
        let off = Cmds::of(&prefs(&[]));
        for s in ["Scratch that.", "New line.", "Cancel message."] { assert_eq!(end(&off, s), None, "{s}") }
        let on = Cmds::of(&prefs(&[(SCRATCH, "on"), (NEWLINE, "on"), (CANCEL, "on"), (AUTO, "5")]));
        assert_eq!(end(&on, "No wait, scratch that."), Some((Cmd::Scratch, 2))); assert_eq!(end(&on, "New line"), Some((Cmd::NewLine, 2))); assert_eq!(end(&on, "Newline."), Some((Cmd::NewLine, 1)));
        assert_eq!(end(&on, "Cancel message."), Some((Cmd::Cancel, 2))); assert_eq!(on.auto_s, 5);
    }
    #[test] fn a_rings_note_loses_its_send_phrase() {
        let c = Cmds::of(&prefs(&[]));
        assert_eq!(c.strip_send("Buy milk on the way home. Message complete."), "Buy milk on the way home."); assert_eq!(c.strip_send("Buy milk, send it."), "Buy milk");
        assert_eq!(c.strip_send("Send it to Kati."), "Send it to Kati.");
    }
}
