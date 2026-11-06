//! Segmenting for live dictation: a small energy VAD over 16-bit mono PCM.
//! gpt-live-transcribe has no server turn detection, so the client commits.
//! Before any speech, audio is held locally as pre-roll (never sent), so a
//! silent stretch sends nothing and commits nothing.

use std::collections::VecDeque;

pub const RATE: u32 = 24000;
pub const BYTES_PER_MS: usize = (RATE as usize * 2) / 1000; // 48

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Append(Vec<u8>),
    Commit,
    Clear,
}

#[derive(Clone, Debug)]
pub struct VadOpts {
    pub quiet_ms: f64,      // quiet after speech that ends a segment
    pub max_ms: f64,        // longest segment
    pub preroll_ms: f64,    // audio kept from before speech starts
    pub min_ms: f64,        // shortest segment worth committing
    pub min_speech_ms: f64, // less speech than this is a click: cleared, not committed
    pub level: f64,         // RMS floor for speech (int16 units)
    pub ratio: f64,         // speech is also above ratio x the noise floor
}

impl Default for VadOpts {
    fn default() -> Self {
        VadOpts { quiet_ms: 700.0, max_ms: 15000.0, preroll_ms: 300.0, min_ms: 100.0, min_speech_ms: 200.0, level: 500.0, ratio: 3.0 }
    }
}

pub fn rms(pcm: &[u8]) -> f64 {
    let n = pcm.len() >> 1;
    if n == 0 {
        return 0.0;
    }
    let mut s = 0.0;
    for c in pcm[..n * 2].chunks_exact(2) {
        let x = i16::from_le_bytes([c[0], c[1]]) as f64;
        s += x * x;
    }
    (s / n as f64).sqrt()
}

fn ms_of(len: usize) -> f64 {
    len as f64 / BYTES_PER_MS as f64
}

pub struct Segmenter {
    o: VadOpts,
    floor: f64,
    active: bool,
    preroll: VecDeque<Vec<u8>>,
    preroll_ms: f64,
    sent_ms: f64,
    quiet: f64,
    speech: f64,
    pub last_rms: f64,   // the last chunk's RMS
    pub last_loud: bool, // whether it counted as speech
}

impl Default for Segmenter {
    fn default() -> Self {
        Self::new(VadOpts::default())
    }
}

impl Segmenter {
    pub fn new(o: VadOpts) -> Self {
        Segmenter { o, floor: 150.0, active: false, preroll: VecDeque::new(), preroll_ms: 0.0, sent_ms: 0.0, quiet: 0.0, speech: 0.0, last_rms: 0.0, last_loud: false }
    }

    /// One chunk of whole samples in; what to send out.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Action> {
        let ms = ms_of(chunk.len());
        let r = rms(chunk);
        let loud = r > self.o.level.max(self.floor * self.o.ratio);
        self.last_rms = r;
        self.last_loud = loud;
        if !loud {
            self.floor = self.floor * 0.95 + r * 0.05;
        }
        if !self.active {
            if !loud {
                self.preroll.push_back(chunk.to_vec());
                self.preroll_ms += ms;
                while self.preroll.len() > 1 && self.preroll_ms - ms_of(self.preroll[0].len()) >= self.o.preroll_ms {
                    let first = self.preroll.pop_front().unwrap();
                    self.preroll_ms -= ms_of(first.len());
                }
                return vec![];
            }
            let mut a = Vec::with_capacity(chunk.len());
            for p in self.preroll.drain(..) {
                a.extend_from_slice(&p);
            }
            a.extend_from_slice(chunk);
            self.active = true;
            self.sent_ms = self.preroll_ms + ms;
            self.preroll_ms = 0.0;
            self.quiet = 0.0;
            self.speech = ms;
            return vec![Action::Append(a)];
        }
        self.sent_ms += ms;
        if loud {
            self.quiet = 0.0;
            self.speech += ms;
        } else {
            self.quiet += ms;
        }
        let mut out = vec![Action::Append(chunk.to_vec())];
        if self.quiet >= self.o.quiet_ms || self.sent_ms >= self.o.max_ms {
            out.push(self.end());
        }
        out
    }

    /// At stop: commit what's buffered, if it is worth it.
    pub fn flush(&mut self) -> Vec<Action> {
        if !self.active {
            return vec![];
        }
        if self.sent_ms < self.o.min_ms {
            self.reset();
            return vec![Action::Clear];
        }
        vec![self.end()]
    }

    fn end(&mut self) -> Action {
        let a = if self.speech >= self.o.min_speech_ms { Action::Commit } else { Action::Clear };
        self.reset();
        a
    }

    fn reset(&mut self) {
        self.active = false;
        self.sent_ms = 0.0;
        self.quiet = 0.0;
        self.speech = 0.0;
    }
}

/// Regroups a byte stream into chunks of `size` bytes (the last may be short
/// but always holds whole samples).
pub struct Rechunker {
    buf: Vec<u8>,
    size: usize,
}

impl Rechunker {
    pub fn new(size: usize) -> Self {
        Rechunker { buf: vec![], size }
    }

    pub fn push(&mut self, b: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(b);
        let mut out = vec![];
        let mut o = 0;
        while self.buf.len() - o >= self.size {
            out.push(self.buf[o..o + self.size].to_vec());
            o += self.size;
        }
        self.buf.drain(..o);
        out
    }

    pub fn rest(&mut self) -> Vec<Vec<u8>> {
        let n = self.buf.len() & !1;
        let r = if n > 0 { vec![self.buf[..n].to_vec()] } else { vec![] };
        self.buf.clear();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(ms: usize, amp: f64) -> Vec<u8> {
        let n = RATE as usize * ms / 1000;
        let mut b = Vec::with_capacity(n * 2);
        for i in 0..n {
            let x = (amp * (2.0 * std::f64::consts::PI * 220.0 * i as f64 / RATE as f64).sin()).round() as i16;
            b.extend_from_slice(&x.to_le_bytes());
        }
        b
    }
    fn quiet(ms: usize) -> Vec<u8> {
        vec![0; RATE as usize * ms / 1000 * 2]
    }
    fn chunks(s: &mut Segmenter, b: &[u8]) -> Vec<Action> {
        b.chunks(4800).flat_map(|c| s.push(c)).collect()
    }
    fn count(a: &[Action], want: &Action) -> usize {
        a.iter().filter(|x| *x == want).count()
    }

    #[test]
    fn silence_sends_nothing_a_click_is_cleared_speech_then_quiet_commits() {
        let mut s = Segmenter::default();
        assert!(chunks(&mut s, &quiet(3000)).is_empty());
        assert!(s.flush().is_empty());
        let click = chunks(&mut s, &[tone(100, 8000.0), quiet(1000)].concat());
        assert!(count(&click, &Action::Clear) > 0);
        assert_eq!(count(&click, &Action::Commit), 0);
        let speech = chunks(&mut s, &[tone(1000, 8000.0), quiet(800)].concat());
        assert_eq!(count(&speech, &Action::Commit), 1);
        let long = chunks(&mut s, &tone(16000, 8000.0));
        assert_eq!(count(&long, &Action::Commit), 1); // cut at 15 s
        assert_eq!(s.flush(), vec![Action::Commit]);
    }

    #[test]
    fn preroll_keeps_about_300_ms_before_speech() {
        let mut s = Segmenter::default();
        assert!(chunks(&mut s, &quiet(2000)).is_empty());
        let a = s.push(&tone(100, 8000.0));
        let Action::Append(b) = &a[0] else { panic!("no append") };
        assert_eq!(b.len(), 400 * BYTES_PER_MS); // three chunks of pre-roll and the speech
    }

    #[test]
    fn a_short_tail_at_stop_is_cleared() {
        let mut s = Segmenter::default();
        s.push(&tone(50, 8000.0));
        assert_eq!(s.flush(), vec![Action::Clear]); // 50 ms is under min_ms
    }

    #[test]
    fn rms_of_a_constant_and_of_nothing() {
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(rms(&[0x10]), 0.0);
        assert_eq!(rms(&[0xe8, 0x03, 0x18, 0xfc]), 1000.0); // 1000, -1000
    }

    #[test]
    fn rechunker_regroups_and_keeps_whole_samples_at_the_end() {
        let mut r = Rechunker::new(4);
        assert_eq!(r.push(&[1, 2, 3]), Vec::<Vec<u8>>::new());
        assert_eq!(r.push(&[4, 5, 6, 7, 8, 9]), vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8]]);
        r.push(&[10, 11]);
        assert_eq!(r.rest(), vec![vec![9, 10]]); // 11 is half a sample
        assert_eq!(r.rest(), Vec::<Vec<u8>>::new());
    }
}
