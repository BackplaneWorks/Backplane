//! A live session: s16le mono PCM at 16 kHz in, hyp/done out.
//!
//! One model does both the live words and the final ones: while a segment
//! is open its audio so far is decoded again every DECODE_MS, and each decode
//! is sent as a `hyp` with every word's start, end and probability. The hub
//! freezes the words two decodes agree on, so what is shown never changes;
//! the segment's last decode is its `done`.
//!
//! The session is an actor on its own thread with one inbox, and decoding
//! has its own thread: the reader of the microphone only ever sends, so it is
//! never held up. Audio sent while the model loads waits in the inbox.

use crate::audio::js_round;
use crate::local::{cs, Word, RATE};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DECODE_EVERY: Duration = Duration::from_millis(300); // a live decode at most this often
const PRE_ROLL: f64 = 0.4; // seconds kept before speech starts
const TAIL: f64 = 0.25; // seconds kept after the VAD's end
pub const MAX_SEG: f64 = 20.0; // a segment is cut after this many seconds
pub const MIN_SPEECH: f64 = 0.25; // shorter speech is a click, not words
const WINDOW: usize = 512;
const SR: f64 = RATE as f64;

fn samples(sec: f64) -> i64 {
    js_round(sec * SR) as i64
}

pub trait Detector {
    fn accept(&mut self, w: &[f32]);
    fn speaking(&mut self) -> bool;
    fn ended(&mut self) -> usize;
}

pub trait Decoder: Send {
    fn decode(&mut self, samples: &[f32]) -> Vec<Word>;
}

pub trait Sink {
    fn hyp(&mut self, item: &str, words: &[Word], end: i64);
    fn done(&mut self, item: &str, words: &[Word], decode_ms: i64, final_ms: i64);
    /// A closed segment's start, in centiseconds from the start of the audio, told just before its `done`.
    fn at(&mut self, _from_cs: i64) {}
}

pub enum In {
    Pcm(Vec<u8>),
    Decoded(Reply),
    Finish,
}

enum Job {
    Hyp { item: String, from: i64, to: i64, samples: Vec<f32> },
    Done { item: String, from: i64, samples: Vec<f32>, min: f64, speech_end: Option<Instant> },
}

pub enum Reply {
    Hyp { item: String, from: i64, to: i64, words: Vec<Word> },
    Done { item: String, from: i64, words: Vec<Word>, decode_ms: i64, final_ms: i64 },
}

fn decoder_thread<D: Decoder + 'static>(mut d: D, jobs: Receiver<Job>, inbox: Sender<In>) {
    std::thread::spawn(move || {
        for job in jobs {
            let reply = match job {
                Job::Hyp { item, from, to, samples } => Reply::Hyp { item, from, to, words: d.decode(&samples) },
                Job::Done { item, from, samples, min, speech_end } => {
                    let t = Instant::now();
                    let words = if samples.len() as f64 > SR * min { d.decode(&samples) } else { vec![] };
                    let decode_ms = t.elapsed().as_millis() as i64;
                    Reply::Done { item, from, words, decode_ms, final_ms: speech_end.map_or(0, |e| e.elapsed().as_millis() as i64) }
                }
            };
            if inbox.send(In::Decoded(reply)).is_err() {
                break;
            }
        }
    });
}

fn base36(mut n: u128) -> String {
    let mut s = vec![];
    while n > 0 {
        s.push(char::from_digit((n % 36) as u32, 36).unwrap());
        n /= 36;
    }
    s.iter().rev().collect()
}

pub struct Session<V: Detector, S: Sink> {
    vad: V,
    out: S,
    jobs: Sender<Job>,
    live: bool,
    id: String,
    buf: Vec<f32>,
    n: usize,  // samples held in buf (from base)
    base: i64, // absolute sample index of buf[0]
    seg_start: i64, // absolute sample where the open segment starts, or -1
    win: Vec<f32>,
    decoding: bool,
    last_decode: Option<Instant>,
    seq: u64,
    carry: Option<u8>,
    pending: usize, // decodes not answered yet
}

impl<V: Detector, S: Sink> Session<V, S> {
    fn new(vad: V, out: S, jobs: Sender<Job>, live: bool) -> Self {
        let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
        Session {
            vad,
            out,
            jobs,
            live,
            id: format!("l{}", base36(ms)),
            buf: vec![0.0; RATE as usize * 120],
            n: 0,
            base: 0,
            seg_start: -1,
            win: Vec::with_capacity(WINDOW),
            decoding: false,
            last_decode: None,
            seq: 0,
            carry: None,
            pending: 0,
        }
    }

    fn now(&self) -> i64 {
        self.base + self.n as i64
    }

    fn push(&mut self, pcm: &[u8]) {
        let mut b = pcm;
        if let (Some(lo), Some((&hi, rest))) = (self.carry, b.split_first()) {
            self.carry = None;
            self.sample(i16::from_le_bytes([lo, hi]));
            b = rest;
        }
        let mut it = b.chunks_exact(2);
        for c in &mut it {
            self.sample(i16::from_le_bytes([c[0], c[1]]));
        }
        if let [odd] = it.remainder() {
            self.carry = Some(*odd);
        }
        self.tick();
    }

    fn sample(&mut self, v: i16) {
        let x = v as f32 / 32768.0;
        if self.n == self.buf.len() {
            self.compact();
        }
        self.buf[self.n] = x;
        self.n += 1;
        self.win.push(x);
        if self.win.len() == WINDOW {
            self.vad.accept(&self.win);
            self.win.clear();
            let speaking = self.vad.speaking();
            let ended = self.vad.ended();
            self.step(speaking, ended);
        }
    }

    /// Drop audio no open segment needs (keep the pre-roll).
    fn compact(&mut self) {
        let keep_from = if self.seg_start >= 0 { self.seg_start } else { self.now() - samples(PRE_ROLL) };
        let drop = (keep_from - self.base).clamp(0, self.n as i64) as usize;
        if drop == 0 {
            let n = self.buf.len();
            self.buf.resize(n * 2, 0.0);
            return;
        }
        self.buf.copy_within(drop..self.n, 0);
        self.n -= drop;
        self.base += drop as i64;
    }

    fn step(&mut self, speaking: bool, ended: usize) {
        if self.seg_start < 0 && speaking {
            self.seg_start = self.base.max(self.now() - samples(PRE_ROLL + MIN_SPEECH));
        }
        if self.seg_start >= 0 && ended > 0 {
            // a segment cut at MAX_SEG while still speaking goes on as the next
            self.close(if speaking { 0.5 } else { TAIL });
            if speaking {
                self.seg_start = self.now();
            }
        }
    }

    fn item(&self) -> String {
        format!("{}-{}", self.id, self.seq)
    }

    fn audio(&self, from: i64, to: i64) -> Vec<f32> {
        let at = |i: i64| (i - self.base).clamp(0, self.n as i64) as usize;
        self.buf[at(from)..at(to).max(at(from))].to_vec()
    }

    fn send(&mut self, j: Job) {
        self.pending += 1;
        let _ = self.jobs.send(j);
    }

    fn tick(&mut self) {
        if !self.live || self.seg_start < 0 || self.decoding || self.last_decode.is_some_and(|t| t.elapsed() < DECODE_EVERY) {
            return;
        }
        let (from, to) = (self.seg_start, self.now());
        if ((to - from) as f64) < SR * 0.3 {
            return;
        }
        self.decoding = true;
        self.last_decode = Some(Instant::now());
        let (item, samples) = (self.item(), self.audio(from, to));
        self.send(Job::Hyp { item, from, to, samples });
    }

    fn close(&mut self, extra: f64) {
        let from = self.seg_start;
        let to = self.now().min(self.now() - samples(0.5) + samples(extra));
        let item = self.item();
        self.seg_start = -1;
        self.seq += 1;
        let samples = self.audio(from, to.max(from + 1));
        self.send(Job::Done { item, from, samples, min: 0.2, speech_end: Some(Instant::now()) });
    }

    fn on_reply(&mut self, r: Reply) {
        self.pending -= 1;
        match r {
            Reply::Hyp { item, from, to, words } => {
                // a decode that finished after its segment closed says nothing new
                if self.item() == item && self.seg_start == from {
                    self.out.hyp(&item, &words, cs((to - from) as f64 / SR));
                }
                self.decoding = false;
            }
            Reply::Done { item, from, words, decode_ms, final_ms } => { self.out.at(cs(from as f64 / SR)); self.out.done(&item, &words, decode_ms, final_ms) }
        }
    }

    /// The end: whatever segment is open closes with all its audio.
    fn finish(&mut self) {
        if self.seg_start >= 0 {
            let (from, to) = (self.seg_start, self.now());
            let item = self.item();
            self.seg_start = -1;
            self.seq += 1;
            let samples = self.audio(from, to);
            self.send(Job::Done { item, from, samples, min: MIN_SPEECH, speech_end: None });
        }
    }
}

/// Runs a session until `Finish` and everything it set off is done.
pub fn run<V: Detector, D: Decoder + 'static, S: Sink>(inbox: Receiver<In>, reply_to: Sender<In>, vad: V, decoder: D, out: S, live: bool) {
    let (jobs, jobs_rx) = channel();
    decoder_thread(decoder, jobs_rx, reply_to);
    let mut s = Session::new(vad, out, jobs, live);
    for m in &inbox {
        match m {
            In::Pcm(b) => s.push(&b),
            In::Decoded(r) => s.on_reply(r),
            In::Finish => {
                s.finish();
                break;
            }
        }
    }
    while s.pending > 0 {
        match inbox.recv() {
            Ok(In::Decoded(r)) => s.on_reply(r),
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

/// A segment of a whole recording: start and end in centiseconds from the start of the audio, the probability (percent) of its least sure word, and its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seg(pub i64, pub i64, pub i64, pub String);

/// A whole recording (s16le at RATE): segments by the VAD, decoded one by one.
pub fn transcribe<V: Detector, D: Decoder + 'static>(vad: V, decoder: D, pcm: &[u8]) -> String { segments(vad, decoder, pcm).iter().map(|s| s.3.as_str()).collect::<Vec<_>>().join(" ") }
/// The same, segment by segment, with when each was said and how sure the engine was.
pub fn segments<V: Detector, D: Decoder + 'static>(vad: V, decoder: D, pcm: &[u8]) -> Vec<Seg> {
    #[derive(Default)]
    struct Parts(std::sync::Arc<std::sync::Mutex<Vec<Seg>>>, i64);
    impl Sink for Parts {
        fn hyp(&mut self, _: &str, _: &[Word], _: i64) {}
        fn at(&mut self, from_cs: i64) { self.1 = from_cs }
        fn done(&mut self, _: &str, words: &[Word], _: i64, _: i64) {
            let t: Vec<&str> = words.iter().map(|w| w.0.as_str()).collect();
            let text = t.join(" ");
            if text.is_empty() { return }
            let end = words.iter().map(|w| w.2).max().unwrap_or(0);
            let conf = words.iter().map(|w| w.3).min().unwrap_or(0);
            self.0.lock().unwrap().push(Seg(self.1 + words.first().map_or(0, |w| w.1), self.1 + end, conf, text));
        }
    }
    let parts = Parts::default();
    let got = parts.0.clone();
    let (tx, rx) = channel();
    for c in pcm.chunks(32000) {
        let _ = tx.send(In::Pcm(c.to_vec()));
    }
    let _ = tx.send(In::Finish);
    run(rx, tx, vad, decoder, parts, false);
    let got = got.lock().unwrap();
    got.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A detector by script: loud windows are speech, and half a second of
    /// quiet after speech closes a segment, as Silero does.
    #[derive(Default)]
    struct Loud {
        talking: bool,
        quiet: usize,
        loud: usize,
        closed: usize,
    }
    impl Detector for Loud {
        fn accept(&mut self, w: &[f32]) {
            let loud = w.iter().any(|x| x.abs() > 0.1);
            if loud {
                self.loud += 1;
                self.quiet = 0;
                if self.loud >= 8 {
                    self.talking = true; // 0.25 s
                }
            } else {
                self.quiet += 1;
                if self.talking && self.quiet >= 16 {
                    self.talking = false;
                    self.closed += 1;
                }
                if !self.talking {
                    self.loud = 0;
                }
            }
        }
        fn speaking(&mut self) -> bool {
            self.talking
        }
        fn ended(&mut self) -> usize {
            std::mem::take(&mut self.closed)
        }
    }

    /// Says how long each decode's audio was.
    struct Lengths(Duration);
    impl Decoder for Lengths {
        fn decode(&mut self, s: &[f32]) -> Vec<Word> {
            std::thread::sleep(self.0);
            vec![Word(format!("{}", s.len()), 0, 1, 100)]
        }
    }

    #[derive(Clone, Default)]
    struct Log(Arc<Mutex<Vec<(String, String, String)>>>);
    impl Sink for Log {
        fn hyp(&mut self, item: &str, w: &[Word], end: i64) {
            self.0.lock().unwrap().push(("hyp".into(), item.into(), format!("{} {end}", w[0].0)));
        }
        fn done(&mut self, item: &str, w: &[Word], _: i64, _: i64) {
            self.0.lock().unwrap().push(("done".into(), item.into(), w.first().map_or(String::new(), |x| x.0.clone())));
        }
    }

    fn pcm(sec: f64, loud: bool) -> Vec<u8> {
        let v: i16 = if loud { 16000 } else { 0 };
        (0..(sec * SR) as usize).flat_map(|_| v.to_le_bytes()).collect()
    }

    fn feed(parts: &[(f64, bool)], decoder: Duration, live: bool) -> Vec<(String, String, String)> {
        let log = Log::default();
        let got = log.0.clone();
        let (tx, rx) = channel();
        let tx2 = tx.clone();
        let t = std::thread::spawn(move || run(rx, tx2, Loud::default(), Lengths(decoder), log, live));
        for (sec, loud) in parts {
            for c in pcm(*sec, *loud).chunks(3200) {
                tx.send(In::Pcm(c.to_vec())).unwrap();
                if live {
                    std::thread::sleep(Duration::from_millis(10)); // ten times real time
                }
            }
        }
        tx.send(In::Finish).unwrap();
        t.join().unwrap();
        let v = got.lock().unwrap().clone();
        v
    }

    #[test]
    fn hyps_then_one_done_per_segment_and_nothing_for_silence() {
        let ev = feed(&[(1.0, false), (3.0, true), (1.0, false)], Duration::from_millis(1), true);
        let hyps: Vec<_> = ev.iter().filter(|e| e.0 == "hyp").collect();
        let dones: Vec<_> = ev.iter().filter(|e| e.0 == "done").collect();
        assert!(!hyps.is_empty());
        assert_eq!(dones.len(), 1);
        assert!(dones[0].1.ends_with("-0"));
        assert!(hyps.iter().all(|h| h.1 == dones[0].1));
        // the done holds the pre-roll, the speech and the tail
        let n: f64 = dones[0].2.parse().unwrap();
        assert!((3.0 * SR..4.2 * SR).contains(&n), "{n}");
        assert_eq!(ev.last().unwrap().0, "done");
        assert!(feed(&[(2.0, false)], Duration::ZERO, true).is_empty());
    }

    #[test]
    fn stopping_mid_speech_closes_the_open_segment_with_all_its_audio() {
        let ev = feed(&[(2.0, true)], Duration::ZERO, false);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].0, "done");
        let n: f64 = ev[0].2.parse().unwrap();
        assert!((1.6 * SR..2.01 * SR).contains(&n), "{n}");
    }

    #[test]
    fn a_decode_behind_the_audio_is_dropped_not_sent_late() {
        // slow decodes: a hyp still running when its segment closes says nothing
        let ev = feed(&[(1.0, true), (1.0, false)], Duration::from_millis(400), true);
        let done_at = ev.iter().position(|e| e.0 == "done").unwrap();
        assert_eq!(ev.iter().skip(done_at + 1).filter(|e| e.0 == "hyp").count(), 0);
    }

    #[test]
    fn items_count_up_by_segment() {
        let ev = feed(&[(1.0, true), (1.0, false), (1.0, true), (1.0, false)], Duration::ZERO, false);
        let items: Vec<String> = ev.iter().filter(|e| e.0 == "done").map(|e| e.1.rsplit('-').next().unwrap().to_string()).collect();
        assert_eq!(items, vec!["0", "1"]);
        assert!(ev[0].1.starts_with('l'));
    }

    #[test]
    fn a_click_under_a_fifth_of_a_second_decodes_nothing() {
        let (tx, _rx) = channel();
        let mut s = Session::new(Loud::default(), Log::default(), tx, false);
        s.n = 100;
        s.seg_start = 0;
        s.close(TAIL);
        match s.jobs.send(Job::Done { item: String::new(), from: 0, samples: vec![], min: 0.2, speech_end: None }) {
            Ok(()) => assert_eq!(s.pending, 1),
            Err(_) => panic!(),
        }
    }

    #[test]
    fn an_odd_byte_waits_for_its_other_half() {
        let (tx, _rx) = channel();
        let mut s = Session::new(Loud::default(), Log::default(), tx, false);
        s.push(&[0x00]);
        assert_eq!(s.n, 0);
        s.push(&[0x40, 0x00, 0xc0]);
        assert_eq!(s.n, 2);
        assert_eq!(s.buf[0], 0.5);
        assert_eq!(s.buf[1], -0.5);
    }

    #[test]
    fn a_long_silence_stays_in_bounded_memory() {
        let (tx, _rx) = channel();
        let mut s = Session::new(Loud::default(), Log::default(), tx, false);
        let cap = s.buf.len();
        let quiet = pcm(10.0, false);
        for _ in 0..14 {
            s.push(&quiet);
        }
        assert_eq!(s.buf.len(), cap);
        assert_eq!(s.now(), 140 * RATE as i64);
    }

    #[test]
    fn a_long_segment_grows_the_buffer_and_loses_nothing() {
        let (tx, _rx) = channel();
        let mut s = Session::new(Loud::default(), Log::default(), tx, false);
        let loud = pcm(10.0, true);
        for _ in 0..13 {
            s.push(&loud);
        }
        assert!(s.seg_start >= 0);
        assert!(s.buf.len() > RATE as usize * 120);
        assert_eq!(s.base, 0);
    }

    #[test]
    fn a_whole_recording_joins_its_segments() {
        let audio = [pcm(1.0, true), pcm(1.0, false), pcm(1.0, true)].concat();
        let t = transcribe(Loud::default(), Lengths(Duration::ZERO), &audio);
        assert_eq!(t.split(' ').count(), 2);
        let segs = segments(Loud::default(), Lengths(Duration::ZERO), &audio);
        assert_eq!(segs.len(), 2);
        assert!(segs[0].0 < 50 && segs[1].0 >= 150 && segs[1].0 < 250, "each segment says when it was said: {segs:?}");
        assert_eq!(segs[0].2, 100);
    }

    #[test]
    fn base36_matches_javascript() {
        assert_eq!(base36(1_700_000_000_000), "loyw3v28");
    }
}
