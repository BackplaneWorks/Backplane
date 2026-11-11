//! A thread's dictation as a client draws it (pure): the state, the fixes that can be undone, the words to clarify, and what each chip does. `view` is the one shape
//! every client shows; `Reader` turns the helper's events into edits of the draft and changes of this state.
use super::{place::{Edit, Place}, vdict::{self, Settle, VDict, VFix, VFx, VW}, words::{self as v, Unsure}};
use serde_json::{json, Value};

#[derive(Clone, Debug, Default)]
pub struct Dictation {
    /// "starting", "live", "stopping", "" when off
    pub state: String, pub device: String, pub engine: String, pub loaded: bool, pub writing: bool, pub level: u32, pub hint: String, pub err: String,
    pub fixes: Vec<VFx>, pub unsure: Vec<Unsure>, pub ignored: Vec<String>, pub fixing: bool,
}
fn btn(label: &str, action: &str, value: &str, primary: bool) -> Value { json!({"label": label, "action": action, "value": value, "primary": primary}) }

impl Dictation {
    pub fn on(&self) -> bool { !self.state.is_empty() }
    fn status(&self) -> String {
        match self.state.as_str() {
            "starting" => "Starting the microphone…".into(),
            "stopping" => "Finishing…".into(),
            _ if self.engine == "local" && !self.loaded => "Loading the model…".into(),
            _ if self.writing => "Writing".into(),
            _ if self.device.is_empty() => "Listening".into(),
            _ => format!("Listening · {}", self.device),
        }
    }
    fn note(&self) -> String {
        let dev = if self.device.is_empty() { "the microphone" } else { &self.device };
        if !self.err.is_empty() { return format!("Dictation: {}", self.err) }
        if !self.on() { return String::new() }
        match self.hint.as_str() {
            "no-audio" => format!("Nothing received from {dev} yet. Is it plugged in? Pick another in Settings, Voice."),
            "silent" => format!("Only silence from {dev} so far. Is it muted, or the wrong one?"),
            _ => String::new(),
        }
    }
    fn ask(&self) -> Value {
        let Some(u) = self.unsure.first() else { return Value::Null };
        let n = self.unsure.len();
        if self.fixing {
            let hint = "The right spelling (it joins the dictionary)";
            let seed = if u.guess.is_empty() { &u.word } else { &u.guess };
            return json!({"text": hint, "word": u.word, "guess": u.guess, "count": n, "fixing": true, "field": {"name": "vfix", "hint": hint, "secret": false, "value": seed},
                "buttons": [btn("Save", "voice-fixed", "", true), btn("Cancel", "voice-cancel", "", false)]});
        }
        let mut b = vec![]; if !u.guess.is_empty() { b.push(btn(&format!("Use {}", u.guess), "voice-use", "0", true)) }
        b.extend([btn("Fix…", "voice-fix", "0", false), btn("Add to dictionary", "voice-add", "0", false), btn("Skip", "voice-skip", "0", false)]);
        json!({"text": if n > 1 { format!("Heard “{}” (1 of {n})", u.word) } else { format!("Heard “{}”", u.word) }, "word": u.word, "guess": u.guess, "count": n, "fixing": false, "buttons": b})
    }
    /// What a client shows above the composer: a strip while on, a note when something is wrong, the fixes as chips that undo, the word to clarify.
    pub fn view(&self) -> Value {
        let fixes: Vec<Value> = self.fixes.iter().enumerate().map(|(i, f)| json!({"label": format!("{} ↶", if f.out.is_empty() { format!("{} dropped", f.heard) } else { format!("{} ← {}", f.out, f.heard) }),
            "tip": format!("Put back what was heard ({}), and stop fixing it this way", f.heard), "action": "voice-undo", "value": i.to_string(), "primary": false})).collect();
        json!({"on": self.on(), "state": self.state, "status": self.status(), "level": self.level, "note": self.note(), "bad": !self.err.is_empty(),
            "mic": "Dictate with this device's microphone", "stop": "Stop dictating", "fixes": fixes, "ask": self.ask()})
    }
    /// A session begins: a fresh strip, what was left to clarify stays until it is answered.
    pub fn begin(&mut self) { *self = Dictation { state: "starting".into(), unsure: std::mem::take(&mut self.unsure), ignored: std::mem::take(&mut self.ignored), ..Dictation::default() } }
    /// The session ended.
    pub fn idle(&mut self) { self.state.clear(); self.writing = false; self.level = 0; self.hint.clear(); self.loaded = false; }
    /// New words to clarify, after those still waiting, never one skipped, at most five.
    fn merge(&mut self, us: Vec<Unsure>) {
        for u in us { if self.unsure.len() < v::MAX_UNSURE && !self.unsure.iter().any(|x| x.word == u.word) && !v::has(&self.ignored, &u.word) { self.unsure.push(u) } }
    }
    /// A chip pressed. `draft` is the draft as the client has it, `get` reads a setting. Changes a client must make come back in `Acted`.
    pub fn act(&mut self, action: &str, value: &str, draft: &str, get: &dyn Fn(&str) -> String) -> Acted {
        let mut a = Acted::default(); let n: usize = value.parse().unwrap_or(0);
        let learn = |a: &mut Acted, heard: &str, out: &str, yes: bool| { let p = vdict::phrase(heard); if p.is_empty() || heard == out { return } let k = format!("{}{p}", vdict::L_PRE); let old = get(&k); a.settings.push((k, vdict::learn(&old, out, yes))) };
        let swap = |a: &mut Acted, w: &str, r: &str| { let t = v::fix(a.draft.as_deref().unwrap_or(draft), w, r); if t != draft { a.draft = Some(t) } };
        match action {
            "voice-use" => if n < self.unsure.len() { let u = self.unsure.remove(n); if !u.guess.is_empty() { swap(&mut a, &u.word, &u.guess) } learn(&mut a, &u.word, if u.guess.is_empty() { &u.word } else { &u.guess }, true); },
            "voice-add" => if n < self.unsure.len() { let u = self.unsure.remove(n); a.settings.push(entry_kv(&u.word)); },
            "voice-skip" => if n < self.unsure.len() { let u = self.unsure.remove(n); self.ignored.push(u.word); },
            "voice-fix" => self.fixing = !self.unsure.is_empty(),
            "voice-cancel" => self.fixing = false,
            "voice-fixed" => { self.fixing = false; let r = value.trim(); if !r.is_empty() && !self.unsure.is_empty() { let u = self.unsure.remove(0); swap(&mut a, &u.word, r); learn(&mut a, &u.word, r, true); a.settings.push(entry_kv(r)); } }
            "voice-undo" => if n < self.fixes.len() { let f = self.fixes.remove(n); if !f.out.is_empty() { swap(&mut a, &f.out, &f.heard) } learn(&mut a, &f.heard, &f.out, false); },
            "voice-err" => self.err.clear(),
            "voice-clear" => { self.fixes.clear(); self.unsure.clear(); self.ignored.clear(); self.fixing = false; }
            _ => {}
        }
        a
    }
}
/// A dictionary entry as its setting (an empty line removes it).
pub fn entry_kv(line: &str) -> (String, String) { (vdict::ekey_setting(line), line.trim().to_string()) }
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Acted { pub draft: Option<String>, pub settings: Vec<(String, String)> }

/// What one helper line gives a client.
#[derive(Debug, PartialEq, Eq)]
pub enum Out { Edit(Edit), View, Level(u32) }

/// A live session's reader: the settling segment, the cloud's text so far, the segment's corrected words and guesses so far, the tail last sent.
/// `cuts`: how many sends cut this session, and `seq` the edits written: every edit carries both, so a client drops an edit made before its cut and says
/// which edit it had last when it sent. `hist`: how far each recent edit had written its segment (None: the segment was done), to cut exactly there.
pub struct Reader { d: VDict, timed: bool, place: Place, set: Settle, acc: String, seg: Vec<VW>, ask: Vec<Unsure>, tail: String, fixes: Vec<VFx>,
    pub cuts: u64, pub seq: u64, hist: std::collections::VecDeque<(u64, String, Option<vdict::Mark>)>, done: (String, Vec<VW>) }
const HIST: usize = 64;
impl Reader {
    pub fn new(d: VDict, timed: bool, draft: &str, caret: usize) -> Reader {
        Reader { d, timed, place: Place::new(draft, caret), set: Settle::default(), acc: String::new(), seg: vec![], ask: vec![], tail: String::new(), fixes: vec![], cuts: 0, seq: 0, hist: Default::default(), done: Default::default() }
    }
    /// The message was sent while dictating: cut `n` (the client's count), with edit `seq` the last the client had written (0: none). Every word up to that
    /// edit went with the message; the next draft is empty, its mark at 0, and starts with what was heard after it, including words of edits the client
    /// dropped because they came after its send (that one comes back as an edit). No word of the sent message is written again.
    pub fn cut(&mut self, n: u64, seq: u64, st: &mut Dictation) -> Vec<Out> {
        if n <= self.cuts { return vec![] }
        self.cuts = n; self.place = Place::new("", 0); self.seg.clear(); self.ask.clear(); self.fixes.clear(); self.tail.clear();
        let at = self.hist.iter().find(|h| h.0 == seq).cloned(); let open = self.set.item.clone();
        let all = vdict::Mark::default();
        match at {
            Some((_, item, Some(m))) if item == open => self.set.rewind(&m),
            Some((_, item, Some(m))) if item == self.done.0 => {
                // that segment finished after the send: the rest of it opens the next draft, and the one open now follows it whole
                if !open.is_empty() { self.set.rewind(&all) }
                let rest = Settle::rest(self.timed, &self.done.1, &m);
                if !rest.is_empty() { let item = self.done.0.clone(); return self.words(&item, vdict::Step { commit: rest, tail: vec![] }, true, st) }
            }
            // gone from the history (very old): what is open now was not written
            None if seq != 0 => { let m = self.set.mark(self.timed, &[]); self.set.rewind(&m) }
            _ => if !open.is_empty() { self.set.rewind(&all) },
        }
        vec![]
    }
    fn words(&mut self, item: &str, x: vdict::Step, fin: bool, st: &mut Dictation) -> Vec<Out> {
        let fx = self.d.fix(&x.commit); let tail = vdict::text(&self.d.fix(&x.tail).words);
        if !fin && fx.words.is_empty() && tail == self.tail { return vec![] }
        let commit = vdict::text(&fx.words); let first = self.seg.is_empty();
        let e = self.place.words(first, v::firm(self.d.terms(), &commit), &commit, &tail);
        self.seq += 1; let m = (!fin).then(|| self.set.mark(self.timed, &x.tail)); self.hist.push_back((self.seq, item.to_string(), m)); if self.hist.len() > HIST { self.hist.pop_front(); }
        self.seg.extend(fx.words); self.ask.extend(fx.ask); self.fixes.extend(fx.fixes);
        let was = st.writing; st.writing = !tail.trim().is_empty();
        let mut out = vec![Out::Edit(e)];
        if fin {
            let all = VFix { words: std::mem::take(&mut self.seg), fixes: vec![], ask: std::mem::take(&mut self.ask) };
            st.merge(self.d.asks(&st.ignored, &all)); st.fixes = std::mem::take(&mut self.fixes);
            self.acc.clear(); self.tail.clear(); out.push(Out::View);
        } else { self.tail = tail; if was != st.writing { out.push(Out::View) } }
        out
    }
    /// One of the helper's lines (tools: `native/voice`): what to tell the client; `st` is the thread's dictation.
    pub fn step(&mut self, line: &str, st: &mut Dictation, engine: &str) -> Vec<Out> {
        let Ok(o) = serde_json::from_str::<Value>(line) else { return vec![] };
        let s = |k: &str| o[k].as_str().unwrap_or("").to_string();
        match s("event").as_str() {
            "ready" => { st.state = "live".into(); st.device = if s("label").is_empty() { s("device") } else { s("label") }; st.hint.clear(); st.engine = engine.to_string(); vec![Out::View] }
            "delta" => {
                let item = s("item"); self.acc = if self.set.item == item { format!("{}{}", self.acc, s("text")) } else { s("text") };
                let x = self.set.hyp(false, &item, vdict::words_text(&self.acc), 0, &self.d.waits().to_vec()); self.words(&item, x, false, st)
            }
            "hyp" => { let item = s("item"); let x = self.set.hyp(true, &item, vdict::words_json(&o["words"]), o["end"].as_u64().unwrap_or(0) as u32, &self.d.waits().to_vec()); self.words(&item, x, false, st) }
            "done" => { let item = s("item"); let ws = if self.timed { vdict::words_json(&o["words"]) } else { vdict::words_text(&s("text")) }; let x = self.set.done(self.timed, &item, &ws); self.done = (item.clone(), ws); self.words(&item, x, true, st) }
            "stopping" => { st.state = "stopping".into(); vec![Out::View] }
            "error" => { st.err = if s("error").is_empty() { "dictation failed".into() } else { s("error") }; vec![Out::View] }
            "level" => { st.level = o["v"].as_u64().unwrap_or(0) as u32; vec![Out::Level(st.level)] }
            "hint" => { st.hint = if s("hint") == "ok" { String::new() } else { s("hint") }; vec![Out::View] }
            "loaded" => { st.loaded = true; vec![Out::View] }
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*; use super::super::place::apply;
    fn dict() -> VDict { VDict::of(&["KiCad = key cad", "SPI"], &[("backplain", "Backplane\t2\t0")] as &[(&str, &str)]) }
    fn play(r: &mut Reader, st: &mut Dictation, text: &mut String, lines: &[Value]) -> Vec<Out> {
        let (mut mark, mut all) = (text.chars().count(), vec![]);
        for l in lines { for o in r.step(&l.to_string(), st, "local") { if let Out::Edit(e) = &o { (*text, mark, _) = apply(text, mark, e); } all.push(o); } }
        all
    }
    #[test] fn the_cloud_engines_deltas_settle_and_its_done_commits_the_rest() {
        let (mut r, mut st, mut t) = (Reader::new(dict(), false, "", 0), Dictation::default(), String::new());
        play(&mut r, &mut st, &mut t, &[json!({"event":"ready","recorder":"pw-record","device":"mic0","label":"Desk mic"}), json!({"event":"delta","item":"a","text":"Open the"}), json!({"event":"delta","item":"a","text":" board in"}), json!({"event":"delta","item":"a","text":" key cad"}), json!({"event":"done","item":"a","text":"Open the board in key cad."})]);
        assert_eq!(t, "Open the board in KiCad."); assert_eq!((st.state.as_str(), st.device.as_str()), ("live", "Desk mic"));
        assert_eq!(st.fixes, vec![VFx { heard: "key cad".into(), out: "KiCad".into() }]);
        play(&mut r, &mut st, &mut t, &[json!({"event":"done","item":"b","text":"And then."})]); assert_eq!(t, "Open the board in KiCad. And then.");
    }
    #[test] fn the_local_engines_hypotheses_keep_committed_words_where_they_are() {
        let (mut r, mut st, mut t) = (Reader::new(dict(), true, "see ", 4), Dictation::default(), "see ".to_string());
        let w = |v: Value| json!({"event":"hyp","item":"l0","words":v,"end":200});
        let a = json!([["Open",0,30,95],["the",30,50,99],["board",60,90,99]]);
        let outs = play(&mut r, &mut st, &mut t, &[w(a.clone()), w(a), json!({"event":"done","item":"l0","text":"","words":[["Open",0,30,95],["the",30,50,99],["board.",60,90,99]]})]);
        assert_eq!(t, "see open the board."); assert!(outs.iter().filter(|o| matches!(o, Out::Edit(_))).count() >= 2);
    }
    /// A client: applies an edit only when it is of its own cut or later (the frame's `cut`), and keeps the number of the last it applied (`seq`).
    struct Cl { text: String, mark: usize, mine: u64, seq: u64 }
    impl Cl {
        fn new() -> Cl { Cl { text: String::new(), mark: 0, mine: 0, seq: 0 } }
        fn take(&mut self, r: &Reader, outs: Vec<Out>) { for o in outs { if let Out::Edit(e) = &o { if r.cuts >= self.mine { (self.text, self.mark, _) = apply(&self.text, self.mark, e); self.seq = r.seq; } } } }
        fn hear(&mut self, r: &mut Reader, st: &mut Dictation, lines: &[Value]) { for l in lines { let o = r.step(&l.to_string(), st, "local"); self.take(r, o); } }
        /// Send: the draft goes with the message, the box is empty, the mark at 0; the hub hears of it (`voice-clear` with cut and seq) a little later.
        fn send(&mut self) -> (String, u64, u64) { self.mine += 1; self.mark = 0; (std::mem::take(&mut self.text), self.mine, self.seq) }
    }
    fn hyp(ws: Value, end: u32) -> Value { json!({"event":"hyp","item":"l0","words":ws,"end":end}) }
    fn said(n: usize) -> Value {
        let all = [("I",0,10),("bought",10,40),("some",40,60),("stuff.",60,100),("I've",130,150),("put",150,170),("a",170,180),("new",180,200),("SIM",200,230),("card.",230,260)];
        Value::Array(all[..n].iter().map(|(w, s, e)| json!([w, s, e, 99])).collect())
    }
    #[test] fn a_message_sent_mid_sentence_takes_its_words_and_the_next_draft_starts_with_what_follows() {
        // the person's Nov 11: "...stuff." sent while still talking, and the next draft began "stuff. I've put a new SIM card"
        let (mut r, mut st, mut c) = (Reader::new(dict(), true, "", 0), Dictation::default(), Cl::new());
        c.hear(&mut r, &mut st, &[hyp(said(4), 105), hyp(said(4), 110)]); assert!(c.text.ends_with("stuff."), "the tail is shown: {:?}", c.text);
        let (sent, n, seq) = c.send();
        // edits the hub made before it heard of the send reach the client after it: dropped, not written into the new draft
        c.hear(&mut r, &mut st, &[hyp(said(5), 160), hyp(said(5), 170)]);
        let o = r.cut(n, seq, &mut st); c.take(&r, o);
        c.hear(&mut r, &mut st, &[hyp(said(10), 270), hyp(said(10), 280), json!({"event":"done","item":"l0","text":"","words":said(10)})]);
        assert_eq!(sent, "I bought some stuff.");
        assert_eq!(c.text, "I've put a new SIM card.", "no word of the sent message carried over, none heard after it lost");
        c.hear(&mut r, &mut st, &[json!({"event":"done","item":"l1","text":"","words":[["Thanks.",300,330,99]]})]);
        assert_eq!(c.text, "I've put a new SIM card. Thanks.", "the next segment follows as ever");
    }
    #[test] fn a_segment_that_ends_before_the_hub_hears_of_the_send_gives_its_rest_to_the_next_draft() {
        let (mut r, mut st, mut c) = (Reader::new(dict(), true, "", 0), Dictation::default(), Cl::new());
        c.hear(&mut r, &mut st, &[hyp(said(4), 105), hyp(said(4), 110)]);
        let (sent, n, seq) = c.send();
        c.hear(&mut r, &mut st, &[json!({"event":"done","item":"l0","text":"","words":said(6)}), json!({"event":"hyp","item":"l1","words":[["new",300,320,99]],"end":330})]);
        let o = r.cut(n, seq, &mut st); c.take(&r, o);
        c.hear(&mut r, &mut st, &[json!({"event":"done","item":"l1","text":"","words":[["New",300,320,99],["day.",320,340,99]]})]);
        assert_eq!((sent.as_str(), c.text.as_str()), ("I bought some stuff.", "I've put new day."), "the rest of the finished segment, then the next");
    }
    #[test] fn sent_with_nothing_written_yet_the_whole_segment_is_the_next_drafts() {
        let (mut r, mut st, mut c) = (Reader::new(dict(), true, "", 0), Dictation::default(), Cl::new());
        let (_, n, seq) = c.send(); c.hear(&mut r, &mut st, &[hyp(said(2), 50)]); let o = r.cut(n, seq, &mut st); c.take(&r, o); assert_eq!(r.cut(n, seq, &mut st), vec![], "a cut is once");
        c.hear(&mut r, &mut st, &[json!({"event":"done","item":"l0","text":"","words":said(4)})]); assert_eq!(c.text, "I bought some stuff.");
    }
    #[test] fn the_cloud_engine_cut_mid_segment_counts_the_words_already_sent() {
        let (mut r, mut st, mut c) = (Reader::new(dict(), false, "", 0), Dictation::default(), Cl::new());
        let d = |x: &str| json!({"event":"delta","item":"a","text":x});
        c.hear(&mut r, &mut st, &[d("Buy some"), d(" stuff.")]); assert_eq!(c.text, "Buy some stuff.");
        let (_, n, seq) = c.send(); c.hear(&mut r, &mut st, &[d(" Then")]); let o = r.cut(n, seq, &mut st); c.take(&r, o);
        c.hear(&mut r, &mut st, &[d(" call"), d(" Bob"), json!({"event":"done","item":"a","text":"Buy some stuff. Then call Bob."})]);
        assert_eq!(c.text, "Then call Bob.");
    }
    #[test] fn an_unchanged_hypothesis_says_nothing() {
        let (mut r, mut st) = (Reader::new(dict(), true, "", 0), Dictation::default());
        let h = json!({"event":"hyp","item":"x","words":[["hello",0,50,99]],"end":60}).to_string();
        assert!(!r.step(&h, &mut st, "local").is_empty()); assert!(r.step(&h, &mut st, "local").is_empty());
    }
    #[test] fn state_words_and_notes() {
        let mut st = Dictation::default(); st.begin(); assert_eq!(st.view()["status"], "Starting the microphone…"); assert_eq!(st.view()["on"], true);
        let mut r = Reader::new(dict(), true, "", 0);
        r.step(r#"{"event":"ready","device":"m"}"#, &mut st, "local"); assert_eq!(st.view()["status"], "Loading the model…");
        r.step(r#"{"event":"loaded","ms":900}"#, &mut st, "local"); assert_eq!(st.view()["status"], "Listening · m");
        r.step(r#"{"event":"hint","hint":"silent"}"#, &mut st, "local"); assert!(st.view()["note"].as_str().unwrap().starts_with("Only silence from m"));
        r.step(r#"{"event":"hint","hint":"ok"}"#, &mut st, "local"); assert_eq!(st.view()["note"], "");
        assert_eq!(r.step(r#"{"event":"level","v":42}"#, &mut st, "local"), vec![Out::Level(42)]);
        r.step(r#"{"event":"stopping"}"#, &mut st, "local"); assert_eq!(st.view()["status"], "Finishing…");
        r.step(r#"{"event":"error","error":"OpenAI rejected the API key (401)"}"#, &mut st, "local"); assert_eq!(st.view()["bad"], true); assert_eq!(st.view()["note"], "Dictation: OpenAI rejected the API key (401)");
        st.idle(); assert_eq!(st.view()["on"], false); assert_eq!(st.view()["bad"], true, "the error stays until it is closed");
        st.act("voice-err", "", "", &|_| String::new()); assert_eq!(st.view()["note"], "");
    }
    fn asked(word: &str, guess: &str) -> Dictation { Dictation { unsure: vec![Unsure { word: word.into(), guess: guess.into() }], ..Dictation::default() } }
    const NONE: fn(&str) -> String = |_| String::new();
    #[test] fn taking_a_guess_replaces_the_word_and_learns_it_once() {
        let mut st = asked("kick ad", "KiCad"); assert_eq!(st.view()["ask"]["buttons"][0]["label"], "Use KiCad");
        let a = st.act("voice-use", "0", "open kick ad now", &NONE);
        assert_eq!(a.draft.as_deref(), Some("open KiCad now")); assert_eq!(a.settings, vec![("voice.l.kick ad".to_string(), "KiCad\t1\t0".to_string())]); assert!(st.unsure.is_empty());
        let mut st = asked("kick ad", "KiCad"); let a = st.act("voice-use", "0", "kick ad", &|k| if k == "voice.l.kick ad" { "KiCad\t1\t0".into() } else { String::new() });
        assert_eq!(a.settings[0].1, "KiCad\t2\t0", "the second time it is trusted");
    }
    #[test] fn typing_the_right_word_replaces_it_joins_the_dictionary_and_is_learned() {
        let mut st = asked("jason", ""); st.act("voice-fix", "0", "", &NONE); assert_eq!(st.view()["ask"]["fixing"], true); assert_eq!(st.view()["ask"]["field"]["value"], "jason");
        let a = st.act("voice-fixed", " JSON ", "send jason now", &NONE);
        assert_eq!(a.draft.as_deref(), Some("send JSON now")); assert_eq!(a.settings, vec![("voice.l.jason".to_string(), "JSON\t1\t0".to_string()), ("voice.e.json".to_string(), "JSON".to_string())]);
        let mut st = asked("jason", ""); st.act("voice-fix", "0", "", &NONE); assert_eq!(st.act("voice-fixed", "  ", "x", &NONE), Acted::default()); assert!(!st.fixing); assert_eq!(st.unsure.len(), 1, "nothing typed changes nothing");
    }
    #[test] fn adding_and_skipping() {
        let mut st = asked("R12", ""); assert_eq!(st.act("voice-add", "0", "", &NONE).settings, vec![("voice.e.r12".to_string(), "R12".to_string())]);
        let mut st = asked("R12", ""); st.act("voice-skip", "0", "", &NONE); st.merge(vec![Unsure { word: "r12".into(), guess: "".into() }]); assert!(st.unsure.is_empty(), "a skipped word is not asked again");
    }
    #[test] fn undoing_a_fix_puts_back_what_was_heard_and_counts_against_it() {
        let mut st = Dictation { fixes: vec![VFx { heard: "key cad".into(), out: "KiCad".into() }, VFx { heard: "um".into(), out: "".into() }], ..Dictation::default() };
        assert_eq!(st.view()["fixes"][0]["label"], "KiCad ← key cad ↶"); assert_eq!(st.view()["fixes"][1]["label"], "um dropped ↶");
        let a = st.act("voice-undo", "0", "open KiCad", &|_| "KiCad\t2\t0".into()); assert_eq!(a.draft.as_deref(), Some("open key cad")); assert_eq!(a.settings[0], ("voice.l.key cad".to_string(), "KiCad\t2\t1".to_string()));
        let a = st.act("voice-undo", "0", "open", &NONE); assert_eq!(a.draft, None, "a dropped word has nowhere to go back to"); assert!(st.fixes.is_empty());
    }
    #[test] fn at_most_five_wait_and_sending_clears_them() {
        let mut st = Dictation::default(); st.merge((0..9).map(|i| Unsure { word: format!("w{i}"), guess: "".into() }).collect()); assert_eq!(st.unsure.len(), 5); assert_eq!(st.view()["ask"]["text"], "Heard “w0” (1 of 5)");
        st.act("voice-clear", "", "", &NONE); assert!(st.view()["ask"].is_null());
    }
}
