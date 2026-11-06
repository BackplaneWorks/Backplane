//! The level meter and the rate converter.

/// JavaScript's Math.round: halves go up, also below zero.
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// RMS (int16) to 0..100: -60 dBFS and below is 0, full scale 100.
pub fn level_of(rms: f64) -> i64 {
    if rms <= 0.0 {
        return 0;
    }
    let db = 20.0 * (rms / 32768.0).log10();
    js_round(((db + 60.0) * 100.0) / 60.0).clamp(0.0, 100.0) as i64
}

/// s16le mono from one rate to another, linearly (a client's 16 kHz for the
/// cloud's 24 kHz); keeps the odd byte and the last sample between chunks.
pub struct Resampler {
    carry: Vec<u8>,
    pos: f64, // the next output sample, in input samples from `last`
    last: i32,
    step: f64,
}

impl Resampler {
    pub fn new(from: f64, to: f64) -> Self {
        Resampler { carry: vec![], pos: 0.0, last: 0, step: from / to }
    }

    pub fn push(&mut self, b0: &[u8]) -> Vec<u8> {
        let mut b = std::mem::take(&mut self.carry);
        b.extend_from_slice(b0);
        let n = b.len() >> 1;
        self.carry = b[n * 2..].to_vec();
        // input sample i (-1 is `last`)
        let last = self.last;
        let at = |i: i64| -> i32 {
            if i < 0 {
                last
            } else {
                let o = i as usize * 2;
                i16::from_le_bytes([b[o], b[o + 1]]) as i32
            }
        };
        let mut out = Vec::new();
        // main.ts stops at pos <= n-1, which leaves a position in (n-1, n) to
        // the next piece, where at(-2) reads `last` for sample n-2: a wrong
        // sample at some piece boundaries. Here every position whose two
        // samples are in hand is done now, so pos never goes below 0.
        while self.pos < n as f64 {
            let whole = self.pos.floor();
            let i = whole as i64 - 1;
            let f = self.pos - whole;
            let x = js_round(at(i) as f64 * (1.0 - f) + at(i + 1) as f64 * f).clamp(-32768.0, 32767.0) as i16;
            out.extend_from_slice(&x.to_le_bytes());
            self.pos += self.step;
        }
        if n > 0 {
            self.last = at(n as i64 - 1);
            self.pos -= n as f64;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_minus_60_dbfs_is_0_full_scale_is_100() {
        assert_eq!(level_of(0.0), 0);
        assert_eq!(level_of(32768.0 / 1000.0), 0);
        assert_eq!(level_of(32768.0), 100);
        assert_eq!(level_of(32768.0 / 31.62), 50);
    }

    fn sine(n: usize) -> Vec<u8> {
        let mut b = Vec::with_capacity(n * 2);
        for i in 0..n {
            b.extend_from_slice(&(js_round(10000.0 * (i as f64 / 10.0).sin()) as i16).to_le_bytes());
        }
        b
    }

    #[test]
    fn resampler_16k_to_24k_keeps_the_length_ratio_and_the_signal() {
        let mut r = Resampler::new(16000.0, 24000.0);
        let n = 1600;
        let b = sine(n);
        // in odd-sized pieces, as uploads come
        let mut out = vec![];
        for c in b.chunks(333) {
            out.extend(r.push(c));
        }
        assert!(((out.len() / 2) as f64 - n as f64 * 1.5).abs() < 4.0);
        // the signal survives: output sample k sits at input k/1.5, one sample late
        for k in (30..out.len() / 2 - 3).step_by(37) {
            let got = i16::from_le_bytes([out[k * 2], out[k * 2 + 1]]) as f64;
            let want = 10000.0 * ((k as f64 / 1.5 - 1.0) / 10.0).sin();
            assert!((got - want).abs() < 300.0, "sample {k}: {got} vs {want}");
        }
    }

    #[test]
    fn resampler_the_pieces_do_not_change_the_result() {
        let b = sine(800);
        let whole = Resampler::new(16000.0, 24000.0).push(&b);
        let mut r = Resampler::new(16000.0, 24000.0);
        let parts: Vec<u8> = b.chunks(77).flat_map(|c| r.push(c)).collect();
        // as in main.ts, `pos` carries float rounding across pieces, so a
        // sample may round one off; nothing more
        assert_eq!(whole.len(), parts.len());
        for (k, (x, y)) in whole.chunks(2).zip(parts.chunks(2)).enumerate() {
            let (x, y) = (i16::from_le_bytes([x[0], x[1]]), i16::from_le_bytes([y[0], y[1]]));
            assert!((x as i32 - y as i32).abs() <= 1, "sample {k}: {x} vs {y}");
        }
    }

    #[test]
    fn resampler_down_halves_the_length() {
        let mut r = Resampler::new(48000.0, 24000.0);
        let out = r.push(&sine(1000));
        assert!(((out.len() / 2) as i64 - 500).abs() <= 2);
    }
}
