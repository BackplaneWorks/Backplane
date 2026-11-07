//! Where dictated words go in a draft the client keeps (pure). The hub holds a shadow of the draft from the moment dictation starts and answers each settled step
//! with one `Edit`, relative to a mark the client keeps, so a client needs no rule of its own: `apply` below is the whole of it, and typing before the mark only moves
//! the mark. Committed words never move; only the tail after them is rewritten (docs/voice.md, "Live words, calmly").
use super::words::{chars, len, put};

/// Replace the `del` characters after the mark with `ins`; then the mark moves `adv` on and the caret goes `to` past the old mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit { pub del: usize, pub ins: String, pub adv: usize, pub to: usize }

/// The shadow: the draft, where the committed words end, and how long the tail after them is.
#[derive(Clone, Debug, Default)]
pub struct Place { draft: String, pos: usize, tail: usize }

impl Place {
    pub fn new(draft: &str, caret: usize) -> Place { Place { draft: draft.to_string(), pos: caret.min(len(draft)), tail: 0 } }
    /// A settled step: `first` when nothing of this segment was committed before, `firm` when the committed words open with a dictionary term.
    pub fn words(&mut self, first: bool, firm: bool, commit: &str, tail: &str) -> Edit {
        let cs = chars(&self.draft); let p = self.pos.min(cs.len()); let end = (p + self.tail).min(cs.len());
        let (del, after) = (end - p, cs[end..].iter().collect::<String>());
        let d: String = cs[..p].iter().chain(cs[end..].iter()).collect();
        let c = put(firm || !first, &d, p, commit);
        let t = put(!(first && commit.is_empty()), &c.text, c.caret, tail);
        let to = if tail.trim().is_empty() { c.caret } else { t.caret };
        let ins: String = { let n = chars(&t.text); n[p..n.len() - len(&after)].iter().collect() };
        let e = Edit { del, adv: c.caret - p, to: to - p, ins };
        self.tail = len(&t.text) - len(&c.text); self.pos = c.caret; self.draft = t.text;
        e
    }
}

/// What a client does with an `Edit`: the new draft, mark and caret. A mark past the end is the end.
pub fn apply(draft: &str, mark: usize, e: &Edit) -> (String, usize, usize) {
    let cs = chars(draft); let m = mark.min(cs.len()); let end = (m + e.del).min(cs.len());
    let text: String = cs[..m].iter().copied().chain(e.ins.chars()).chain(cs[end..].iter().copied()).collect();
    (text, m + e.adv, m + e.to)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Steps through a dictation the way a client sees it: the shadow and the client's own draft must never differ.
    fn run(draft: &str, caret: usize, steps: &[(bool, bool, &str, &str)]) -> (String, usize) {
        let (mut p, mut text, mut mark, mut at) = (Place::new(draft, caret), draft.to_string(), caret, caret);
        for (first, firm, commit, tail) in steps { let e = p.words(*first, *firm, commit, tail); (text, mark, at) = apply(&text, mark, &e); assert_eq!(text, p.draft, "the client follows the shadow"); }
        let _ = mark; (text, at)
    }
    #[test] fn a_segment_reads_on_from_the_draft_and_its_tail_is_rewritten_in_place() {
        assert_eq!(run("", 0, &[(true, false, "", "Open the"), (true, false, "Open", "the board"), (false, false, "the board", "in Kai"), (false, false, "in KiCad.", "")]), ("Open the board in KiCad.".into(), 24));
        assert_eq!(run("fix the regulator", 17, &[(true, false, "", "Hello from"), (true, false, "Hello from voice.", "")]), ("fix the regulator hello from voice.".into(), 35));
    }
    #[test] fn words_said_at_the_caret_leave_what_follows_alone() {
        assert_eq!(run("open board", 5, &[(true, false, "The", "KiCad"), (false, false, "KiCad.", "")]).0, "open the KiCad board");
        assert_eq!(run("open, then", 4, &[(true, false, "The board", "")]), ("open the board, then".into(), 14));
    }
    #[test] fn a_second_segment_follows_the_first() {
        assert_eq!(run("", 0, &[(true, false, "One two.", ""), (true, false, "Three four.", "")]).0, "One two. Three four.");
        assert_eq!(run("", 0, &[(true, false, "One two", ""), (true, false, "Three four.", "")]).0, "One two three four.");
    }
    #[test] fn a_dictionary_term_keeps_its_capital_and_a_later_commit_keeps_its_own_case() {
        assert_eq!(run("open", 4, &[(true, true, "Backplane now", "")]).0, "open Backplane now");
        assert_eq!(run("so", 2, &[(true, false, "It", ""), (false, false, "Works", "")]).0, "so it Works");
    }
    #[test] fn typing_before_the_mark_keeps_the_edit_at_the_mark() {
        let mut p = Place::new("ab", 2); let e = p.words(true, false, "Now", "then"); let (text, mark, _) = apply("ab", 2, &e); assert_eq!(text, "ab now then");
        let (typed, moved) = (format!("XX{text}"), mark + 2); let e = p.words(false, false, "then.", "");
        assert_eq!(apply(&typed, moved, &e).0, "XXab now then.");
    }
    /// test/fixtures/voice-edits.json is what every client checks itself against: its `want`s are the hub's own (VOICE_BLESS=1 writes them).
    #[test] fn the_clients_fixture_is_the_hubs_own_edits() {
        use serde_json::{json, Value};
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/fixtures/voice-edits.json");
        let mut f: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let bless = std::env::var_os("VOICE_BLESS").is_some();
        for c in f["edits"].as_array_mut().unwrap() {
            let (draft, caret) = (c["draft"].as_str().unwrap().to_string(), c["caret"].as_u64().unwrap() as usize);
            let (mut p, mut text, mut mark, mut at, mut out) = (Place::new(&draft, caret), draft.clone(), caret, caret, vec![]);
            for s in c["steps"].as_array().unwrap() {
                let e = p.words(s["first"] == true, s["firm"] == true, s["commit"].as_str().unwrap(), s["tail"].as_str().unwrap());
                out.push(json!({"del": e.del, "ins": e.ins, "adv": e.adv, "to": e.to}));
                (text, mark, at) = apply(&text, mark, &e);
                assert_eq!(text, p.draft, "{}", c["name"]);
            }
            let want = json!({"edits": out, "text": text, "mark": mark, "caret": at});
            if bless { c["want"] = want } else { assert_eq!(c["want"], want, "{} (VOICE_BLESS=1 rewrites the fixture)", c["name"]) }
        }
        for c in f["apply"].as_array().unwrap() {
            let e = &c["edit"]; let n = |k: &str| e[k].as_u64().unwrap() as usize;
            let (text, mark, at) = apply(c["text"].as_str().unwrap(), c["mark"].as_u64().unwrap() as usize, &Edit { del: n("del"), ins: e["ins"].as_str().unwrap().into(), adv: n("adv"), to: n("to") });
            assert_eq!(c["want"], json!({"text": text, "mark": mark, "caret": at}), "{}", c["name"]);
        }
        if bless { std::fs::write(path, serde_json::to_string_pretty(&f).unwrap() + "\n").unwrap() }
    }
    #[test] fn a_mark_past_the_end_is_the_end() { assert_eq!(apply("ab", 9, &Edit { del: 3, ins: "c".into(), adv: 1, to: 1 }), ("abc".into(), 3, 3)); }
}
