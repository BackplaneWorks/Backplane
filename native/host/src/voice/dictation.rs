//! A thread's dictation as a client draws it (pure): the state, the fixes that can be undone, the words to clarify, and what each chip does. `view` is the one shape
//! every client shows; `Reader` turns the helper's events into edits of the draft and changes of this state.
use super::{cmds::{Cmd, Cmds}, place::{Edit, Place}, vdict::{self, Settle, VDict, VFix, VFx, VW}, words::{self as v, Unsure}};
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
pub enum Out { Edit(Edit), View, Level(u32), /** a voice command for the client: "send" or "cancel" (cmds.rs), after the cut that ends the message */ Command(&'static str) }

/// A live session's reader: the settling segment, the cloud's text so far, the segment's corrected words and guesses so far, the tail last sent.
/// `cuts`: how many sends cut this session, and `seq` the edits written: every edit carries both, so a client drops an edit made before its cut and says
/// which edit it had last when it sent. `hist`: how far each recent edit had written its segment (None: the segment was done), to cut exactly there.
pub struct Reader { d: VDict, timed: bool, place: Place, set: Settle, acc: String, seg: Vec<VW>, ask: Vec<Unsure>, tail: String, fixes: Vec<VFx>,
    pub cuts: u64, pub seq: u64, hist: std::collections::VecDeque<(u64, String, Option<vdict::Mark>)>, done: (String, Vec<VW>),
    /// the voice commands on; when words were last heard; whether any were written since the last cut (auto-send has something to send)
    cmds: Cmds, heard: Option<std::time::Instant>, wrote: bool }
const HIST: usize = 64;
impl Reader {
    pub fn new(d: VDict, timed: bool, draft: &str, caret: usize) -> Reader {
        Reader { d, timed, place: Place::new(draft, caret), set: Settle::default(), acc: String::new(), seg: vec![], ask: vec![], tail: String::new(), fixes: vec![], cuts: 0, seq: 0, hist: Default::default(), done: Default::default(), cmds: Cmds::default(), heard: None, wrote: false }
    }
    pub fn with_cmds(mut self, c: Cmds) -> Reader { self.cmds = c; self }
    /// The message ends here by a voice command: the same as a client's cut, made by the hub (the client takes its count from the command's frame).
    fn cut_here(&mut self) { self.cuts += 1; self.place = Place::new("", 0); self.seg.clear(); self.ask.clear(); self.fixes.clear(); self.tail.clear(); self.wrote = false; }
    fn edit(&mut self, item: &str, e: Edit) -> Out { self.seq += 1; self.hist.push_back((self.seq, item.to_string(), None)); if self.hist.len() > HIST { self.hist.pop_front(); } Out::Edit(e) }
    /// Auto-send (cmds.rs `AUTO`): after that many seconds with no words heard, no segment open and words written since the last send, the message goes.
    pub fn idle(&mut self, now: std::time::Instant) -> Vec<Out> {
        let s = self.cmds.auto_s; if s == 0 || !self.wrote || !self.set.item.is_empty() || self.place.is_empty() { return vec![] }
        if self.heard.map_or(true, |h| now.duration_since(h).as_secs() < s as u64) { return vec![] }
        self.cut_here(); vec![Out::Command("send")]
    }
    /// The message was sent while dictating: cut `n` (the client's count), with edit `seq` the last the client had written (0: none). Every word up to that
    /// edit went with the message; the next draft is empty, its mark at 0, and starts with what was heard after it, including words of edits the client
    /// dropped because they came after its send (that one comes back as an edit). No word of the sent message is written again.
    pub fn cut(&mut self, n: u64, seq: u64, st: &mut Dictation) -> Vec<Out> {
        if n <= self.cuts { return vec![] }
        self.cuts = n; self.place = Place::new("", 0); self.seg.clear(); self.ask.clear(); self.fixes.clear(); self.tail.clear(); self.wrote = false;
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
        // a command at the very end of the utterance (the segment's last decode, after its pause): its words are not written, or are taken back
        let cmd = if fin { let all: Vec<&str> = self.seg.iter().chain(fx.words.iter()).map(|w| w.w.as_str()).collect(); self.cmds.at_end(&all) } else { None };
        let mut fx = fx; let mut unsay = 0;
        if let Some((_, k)) = cmd { let f = fx.words.len(); if k <= f { fx.words.truncate(f - k) } else { fx.words.clear(); unsay = k - f } }
        let commit = vdict::text(&fx.words); let first = self.seg.is_empty();
        if !commit.trim().is_empty() { self.wrote = true }
        let e = if unsay > 0 { self.place.unsay(unsay) } else { self.place.words(first, v::firm(self.d.terms(), &commit), &commit, &tail) };
        self.seq += 1; let m = (!fin).then(|| self.set.mark(self.timed, &x.tail)); self.hist.push_back((self.seq, item.to_string(), m)); if self.hist.len() > HIST { self.hist.pop_front(); }
        self.seg.extend(fx.words); self.ask.extend(fx.ask); self.fixes.extend(fx.fixes);
        let was = st.writing; st.writing = !tail.trim().is_empty();
        let mut out = vec![Out::Edit(e)];
        if fin {
            let all = VFix { words: std::mem::take(&mut self.seg), fixes: vec![], ask: std::mem::take(&mut self.ask) };
            st.merge(self.d.asks(&st.ignored, &all)); st.fixes = std::mem::take(&mut self.fixes);
            self.acc.clear(); self.tail.clear(); out.push(Out::View);
            match cmd.map(|c| c.0) {
                Some(Cmd::Scratch) => { let e = self.place.scratch(); out.push(self.edit(item, e)) }
                Some(Cmd::NewLine) => { let e = self.place.newline(); out.push(self.edit(item, e)) }
                Some(c @ (Cmd::Send | Cmd::Cancel)) => { self.cut_here(); out.push(Out::Command(c.name())) }
                None => {}
            }
        } else { self.tail = tail; if was != st.writing { out.push(Out::View) } }
        out
    }
    /// One of the helper's lines (tools: `native/voice`): what to tell the client; `st` is the thread's dictation.
    pub fn step(&mut self, line: &str, st: &mut Dictation, engine: &str) -> Vec<Out> {
        let Ok(o) = serde_json::from_str::<Value>(line) else { return vec![] };
        let s = |k: &str| o[k].as_str().unwrap_or("").to_string();
        if matches!(o["event"].as_str(), Some("hyp" | "delta" | "done")) { self.heard = Some(std::time::Instant::now()) }
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
    /// `went`: what voice commands did to the draft ("send:<text>", "cancel:<text>").
    struct Cl { text: String, mark: usize, mine: u64, seq: u64, went: Vec<String> }
    impl Cl {
        fn new() -> Cl { Cl { text: String::new(), mark: 0, mine: 0, seq: 0, went: vec![] } }
        fn take(&mut self, r: &Reader, outs: Vec<Out>) { for o in outs { match &o {
            Out::Edit(e) => if r.cuts >= self.mine { (self.text, self.mark, _) = apply(&self.text, self.mark, e); self.seq = r.seq; },
            // a voice command: the draft goes (into the undo window), the box is empty, the count is the hub's
            Out::Command(c) => { self.went.push(format!("{c}:{}", std::mem::take(&mut self.text))); self.mark = 0; self.mine = self.mine.max(r.cuts); }
            _ => {}
        } } }
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
    fn cmds(kv: &[(&str, &str)]) -> Cmds { Cmds::of(&kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()) }
    fn seg(item: &str, words: &[(&str, u32, u32)]) -> Value { Value::Array(words.iter().map(|(w, s, e)| json!([w, s, e, 99])).collect()).pipe(|w| json!({"event":"done","item":item,"text":"","words":w})) }
    trait Pipe: Sized { fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T { f(self) } } impl Pipe for Value {}
    #[test] fn message_complete_at_the_end_of_an_utterance_sends_the_draft_without_it_and_what_follows_starts_the_next() {
        let (mut r, mut st, mut c) = (Reader::new(dict(), true, "", 0).with_cmds(cmds(&[])), Dictation::default(), Cl::new());
        let w = json!([["Buy",0,20,99],["milk.",20,50,99],["Message",80,110,99],["complete.",110,150,99]]);
        // "Message" is committed before the pause (two hypotheses agree, well before the audio's end); the segment's end takes it back
        c.hear(&mut r, &mut st, &[json!({"event":"hyp","item":"l0","words":w,"end":200}), json!({"event":"hyp","item":"l0","words":w,"end":210}), json!({"event":"done","item":"l0","text":"","words":w})]);
        assert_eq!(c.went, ["send:Buy milk."], "sent without the phrase"); assert_eq!(c.text, "");
        c.hear(&mut r, &mut st, &[seg("l1", &[("And", 300, 320), ("eggs.", 320, 350)])]); assert_eq!(c.text, "And eggs.", "the next draft has only what came after");
        c.hear(&mut r, &mut st, &[seg("l2", &[("Send", 400, 420), ("it.", 420, 440)])]); assert_eq!(c.went[1], "send:And eggs.");
    }
    #[test] fn send_it_mid_sentence_is_words_and_only_a_pause_after_it_sends() {
        let (mut r, mut st, mut c) = (Reader::new(dict(), true, "", 0).with_cmds(cmds(&[])), Dictation::default(), Cl::new());
        c.hear(&mut r, &mut st, &[seg("l0", &[("Send", 0, 20), ("it", 20, 30), ("to", 30, 40), ("Kati.", 40, 70)]), seg("l1", &[("I", 100, 110), ("sent", 110, 130), ("it.", 130, 150)])]);
        assert!(c.went.is_empty(), "{:?}", c.went); assert_eq!(c.text, "Send it to Kati. I sent it.");
        // the phrase heard in a hypothesis, the speaker going on: nothing until the segment ends, and it ends elsewhere
        c.hear(&mut r, &mut st, &[json!({"event":"hyp","item":"l2","words":[["message",200,220,99],["complete",220,250,99]],"end":260}), seg("l2", &[("message", 200, 220), ("complete", 220, 250), ("by", 250, 260), ("Friday.", 260, 300)])]);
        assert!(c.went.is_empty()); assert!(c.text.ends_with("message complete by Friday."), "{:?}", c.text);
    }
    #[test] fn sending_by_voice_turned_off_writes_the_words() {
        let (mut r, mut st, mut c) = (Reader::new(dict(), true, "", 0).with_cmds(cmds(&[("voice.cmd.send", "-")])), Dictation::default(), Cl::new());
        c.hear(&mut r, &mut st, &[seg("l0", &[("Message", 0, 20), ("complete.", 20, 40)])]); assert!(c.went.is_empty()); assert_eq!(c.text, "Message complete.");
    }
    #[test] fn scratch_that_new_line_and_cancel_message_when_turned_on() {
        let on = cmds(&[("voice.cmd.scratch", "on"), ("voice.cmd.newline", "on"), ("voice.cmd.cancel", "on")]);
        let (mut r, mut st, mut c) = (Reader::new(dict(), true, "", 0).with_cmds(on), Dictation::default(), Cl::new());
        c.hear(&mut r, &mut st, &[seg("l0", &[("First", 0, 20), ("one.", 20, 40)]), seg("l1", &[("Wrong", 50, 70), ("one.", 70, 90)]), seg("l2", &[("Scratch", 100, 120), ("that.", 120, 140)])]);
        assert_eq!(c.text, "First one.");
        c.hear(&mut r, &mut st, &[seg("l3", &[("New", 150, 160), ("line.", 160, 180)]), seg("l4", &[("Second", 200, 220), ("line.", 220, 240)])]);
        assert_eq!(c.text, "First one.\nSecond line.");
        c.hear(&mut r, &mut st, &[seg("l5", &[("Cancel", 300, 320), ("message.", 320, 340)])]); assert_eq!(c.went, ["cancel:First one.\nSecond line."]); assert_eq!(c.text, "");
        c.hear(&mut r, &mut st, &[seg("l6", &[("Fresh.", 400, 420)])]); assert_eq!(c.text, "Fresh.");
    }
    #[test] fn auto_send_after_the_silence_set_and_only_with_words_to_send() {
        let (mut r, mut st, mut c) = (Reader::new(dict(), true, "", 0).with_cmds(cmds(&[("voice.cmd.auto", "3")])), Dictation::default(), Cl::new());
        let t0 = std::time::Instant::now();
        assert!(r.idle(t0 + std::time::Duration::from_secs(9)).is_empty(), "nothing said: nothing sent");
        c.hear(&mut r, &mut st, &[seg("l0", &[("Ship", 0, 20), ("it", 20, 30), ("today.", 30, 60)])]);
        let now = std::time::Instant::now();
        assert!(r.idle(now + std::time::Duration::from_secs(2)).is_empty(), "not yet");
        let o = r.idle(now + std::time::Duration::from_secs(4)); c.take(&r, o); assert_eq!(c.went, ["send:Ship it today."]);
        assert!(r.idle(now + std::time::Duration::from_secs(20)).is_empty(), "once");
        let (mut r2, mut st2, mut c2) = (Reader::new(dict(), true, "", 0).with_cmds(cmds(&[])), Dictation::default(), Cl::new());
        c2.hear(&mut r2, &mut st2, &[seg("l0", &[("Hi.", 0, 20)])]); assert!(r2.idle(std::time::Instant::now() + std::time::Duration::from_secs(60)).is_empty(), "off by default");
    }
    #[test] fn a_client_send_after_a_voice_send_counts_on_from_the_hubs_cut() {
        let (mut r, mut st, mut c) = (Reader::new(dict(), true, "", 0).with_cmds(cmds(&[])), Dictation::default(), Cl::new());
        c.hear(&mut r, &mut st, &[seg("l0", &[("One.", 0, 20), ("Send", 30, 40), ("it.", 40, 50)])]); assert_eq!((c.mine, r.cuts), (1, 1));
        c.hear(&mut r, &mut st, &[seg("l1", &[("Two.", 60, 80)])]); let (sent, n, seq) = c.send(); assert_eq!((sent.as_str(), n), ("Two.", 2));
        let o = r.cut(n, seq, &mut st); c.take(&r, o); c.hear(&mut r, &mut st, &[seg("l2", &[("Three.", 100, 120)])]); assert_eq!(c.text, "Three.");
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
