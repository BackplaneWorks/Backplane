//! Waveform measurements and compaction for `kicad_sim` (docs/kicad-sim.md): what an agent asks of a simulation's vectors, answered as a
//! number instead of ten thousand samples. Pure: a [`Sig`] is one vector over its sweep axis (time, frequency, a swept source), real or
//! complex; nothing here knows KiCad or ngspice. Samples are joined by straight lines (along log f for a frequency axis), so a crossing,
//! a value at x or a corner lands between samples, and a phase is unwrapped before anything reads it.
//!
//! Units: what the vector has (volts, amperes, seconds, hertz); gains in dB, phases in degrees, overshoot and duty in percent.
use serde_json::{json, Value};

/// One vector over its sweep axis. `im`: the imaginary part of a complex vector (an AC analysis). `log`: the axis is a frequency, so
/// places between samples are found along log x.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sig { pub x: Vec<f64>, pub y: Vec<f64>, pub im: Option<Vec<f64>>, pub log: bool }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dir { Rise, Fall, Any }

/// Half power in dB (10 log10 2): the "-3 dB" of a corner, so a one-pole corner is 1/(2 pi R C) exactly.
pub const HALF_POWER_DB: f64 = 3.010299956639812;

/// The measures by name, with what each takes: the tool's description and the docs are made from this table, and `measure` answers
/// exactly these.
pub const MEASURES: &[(&str, &str)] = &[
    ("at", "the value at x (`x`); a complex vector answers {db, deg}"),
    ("cross", "where it crosses `level` (`dir`: rise, fall, any; `n`: the nth, else the first 20)"),
    ("min", "lowest value"), ("max", "highest value"), ("mean", "mean over the window"), ("final", "the settled value at the end (mean of the last 2 %)"),
    ("pp", "peak to peak"), ("rms", "RMS over the window"), ("ripple", "peak to peak over the window (give `from`, `to`)"), ("ripple_rms", "RMS about the mean over the window"),
    ("peak", "{value, at}; a complex vector answers {db, at}"),
    ("overshoot", "percent of the step past its final value (`initial`, `final` override the first sample and the settled end)"),
    ("undershoot", "percent of the step past its initial value, the other way"),
    ("rise_time", "10 % to 90 % (`lo`, `hi` fractions; `low`, `high` levels)"), ("fall_time", "90 % to 10 %"),
    ("settling_time", "time after `t0` (else the window's start) from which it stays within `band` (0.02 of the step) or `tol` (absolute) of the final value"),
    ("frequency", "of a periodic signal, from its rising crossings of the mid level (`level`)"), ("period", "1 / frequency"), ("duty", "percent of a period above the mid level"),
    ("dc_gain", "gain at the lowest frequency, dB (`linear: true`: V/V)"),
    ("corner", "the -3 dB frequency: below the DC gain (low-pass), the high-frequency gain (high-pass), or {low, high} about the peak (band-pass); `drop` dB, `ref`: dc, hf, peak or a dB value"),
    ("bandwidth", "the -3 dB bandwidth: the corner of a low-pass, high - low of a band-pass"), ("center", "sqrt(low * high) of a band-pass"), ("q", "center / bandwidth"),
    ("unity_gain_freq", "where the gain falls through 0 dB (`level` dB)"),
    ("phase_margin", "180 + the loop gain's phase at unity gain, degrees (`inverted: true` when the loop gain includes the feedback's inversion and starts at 180)"),
    ("gain_margin", "minus the loop gain in dB where its phase falls through -180"),
    ("gain_at", "gain in dB at frequency `x`"), ("phase_at", "phase in degrees at frequency `x`"),
    ("noise_rms", "a noise density (V/rtHz; `power: true` for V^2/Hz) integrated from `from` to `to`, RMS"),
];

fn interp_x(x0: f64, x1: f64, t: f64, log: bool) -> f64 { if log && x0 > 0.0 && x1 > 0.0 { x0 * (x1 / x0).powf(t) } else { x0 + (x1 - x0) * t } }
fn frac_x(x0: f64, x1: f64, x: f64, log: bool) -> f64 { if x1 == x0 { 0.0 } else if log && x0 > 0.0 && x1 > 0.0 && x > 0.0 { (x / x0).ln() / (x1 / x0).ln() } else { (x - x0) / (x1 - x0) } }

/// y at `at`, on the straight line between the two samples around it; None outside the data.
pub fn value_at(x: &[f64], y: &[f64], at: f64, log: bool) -> Option<f64> {
    let n = x.len().min(y.len()); if n == 0 || at.is_nan() { return None }
    let slack = (x[n - 1] - x[0]).abs() * 1e-9;
    if at < x[0] - slack || at > x[n - 1] + slack { return None }
    let i = x[..n].partition_point(|v| *v <= at);
    if i == 0 { return Some(y[0]) } if i >= n { return Some(y[n - 1]) }
    Some(y[i - 1] + (y[i] - y[i - 1]) * frac_x(x[i - 1], x[i], at, log))
}
/// Where y passes `level`, between samples.
pub fn crossings(x: &[f64], y: &[f64], level: f64, dir: Dir, log: bool) -> Vec<f64> {
    let n = x.len().min(y.len()); let mut out = vec![];
    for i in 0..n.saturating_sub(1) {
        let (a, b) = (y[i] - level, y[i + 1] - level);
        let (up, down) = (a < 0.0 && b >= 0.0, a > 0.0 && b <= 0.0);
        if match dir { Dir::Rise => up, Dir::Fall => down, Dir::Any => up || down } { out.push(interp_x(x[i], x[i + 1], a / (a - b), log)) }
    }
    out
}
/// A phase in degrees without its jumps of 360.
pub fn unwrap_deg(raw: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(raw.len()); let mut off = 0.0;
    for (i, p) in raw.iter().enumerate() {
        if i > 0 { let d = p - raw[i - 1]; if d > 180.0 { off -= 360.0 } else if d < -180.0 { off += 360.0 } }
        out.push(p + off);
    }
    out
}
/// A number at `digits` significant digits (what an answer carries).
pub fn round_sig(v: f64, digits: usize) -> f64 { if v == 0.0 || !v.is_finite() { return v } format!("{:.*e}", digits.max(1) - 1, v).parse().unwrap_or(v) }

fn trapz(x: &[f64], y: &[f64]) -> f64 { (1..x.len().min(y.len())).map(|i| (x[i] - x[i - 1]) * (y[i] + y[i - 1]) / 2.0).sum() }
/// The integral of y squared, exact for the straight lines between samples.
fn trapz_sq(x: &[f64], y: &[f64]) -> f64 { (1..x.len().min(y.len())).map(|i| (x[i] - x[i - 1]) * (y[i] * y[i] + y[i] * y[i - 1] + y[i - 1] * y[i - 1]) / 3.0).sum() }
fn arg_by(y: &[f64], better: impl Fn(f64, f64) -> bool) -> usize { let mut k = 0; for i in 1..y.len() { if better(y[i], y[k]) { k = i } } k }
fn arg_max(y: &[f64]) -> usize { arg_by(y, |a, b| a > b) }
fn arg_min(y: &[f64]) -> usize { arg_by(y, |a, b| a < b) }

impl Sig {
    pub fn real(x: Vec<f64>, y: Vec<f64>) -> Sig { Sig { x, y, im: None, log: false } }
    pub fn ac(f: Vec<f64>, re: Vec<f64>, im: Vec<f64>) -> Sig { Sig { x: f, y: re, im: Some(im), log: true } }
    pub fn len(&self) -> usize { self.x.len() }
    pub fn is_empty(&self) -> bool { self.x.is_empty() }
    fn ok(&self) -> Result<(), String> {
        if self.x.is_empty() { return Err("the vector has no points".into()) }
        if self.x.len() != self.y.len() || self.im.as_ref().is_some_and(|i| i.len() != self.x.len()) { return Err("the vector and its axis differ in length".into()) }
        Ok(())
    }
    fn rising(&self) -> bool { self.x.windows(2).all(|w| w[1] >= w[0]) }
    /// The part between `from` and `to` (each defaulting to the data's end), with a point on each edge.
    pub fn window(&self, from: Option<f64>, to: Option<f64>) -> Result<Sig, String> {
        self.ok()?;
        if from.is_none() && to.is_none() { return Ok(self.clone()) }
        if !self.rising() { return Err("the axis is not increasing (a nested sweep): a window needs one sweep".into()) }
        let (a, b) = (self.x[0], self.x[self.len() - 1]);
        let (lo, hi) = (from.unwrap_or(a).max(a), to.unwrap_or(b).min(b));
        if !(lo <= hi) { return Err(format!("the window {}..{} is outside the data ({a:e}..{b:e})", from.unwrap_or(a), to.unwrap_or(b))) }
        let mut w = Sig { x: vec![], y: vec![], im: self.im.as_ref().map(|_| vec![]), log: self.log };
        let mut push = |w: &mut Sig, x: f64| { w.x.push(x); w.y.push(value_at(&self.x, &self.y, x, self.log).unwrap_or(0.0));
            if let (Some(wi), Some(si)) = (w.im.as_mut(), self.im.as_ref()) { wi.push(value_at(&self.x, si, x, self.log).unwrap_or(0.0)) } };
        push(&mut w, lo);
        for i in 0..self.len() { if self.x[i] > lo && self.x[i] < hi { w.x.push(self.x[i]); w.y.push(self.y[i]); if let (Some(wi), Some(si)) = (w.im.as_mut(), self.im.as_ref()) { wi.push(si[i]) } } }
        if hi > lo { push(&mut w, hi) }
        Ok(w)
    }
    pub fn span(&self) -> f64 { self.x[self.len() - 1] - self.x[0] }
    pub fn min(&self) -> f64 { self.y[arg_min(&self.y)] }
    pub fn max(&self) -> f64 { self.y[arg_max(&self.y)] }
    /// The mean along the axis (each sample weighs what it covers); the plain mean when the axis has no extent.
    pub fn mean(&self) -> f64 { let s = self.span(); if s > 0.0 && self.rising() { trapz(&self.x, &self.y) / s } else { self.y.iter().sum::<f64>() / self.len() as f64 } }
    pub fn rms(&self) -> f64 { let s = self.span(); if s > 0.0 && self.rising() { (trapz_sq(&self.x, &self.y) / s).sqrt() } else { (self.y.iter().map(|v| v * v).sum::<f64>() / self.len() as f64).sqrt() } }
    /// RMS about the mean: a ripple's.
    pub fn ac_rms(&self) -> f64 { let (r, m) = (self.rms(), self.mean()); (r * r - m * m).max(0.0).sqrt() }
    /// The settled value at the end: the mean of the last 2 % of the axis, so a ripple or a last sample's noise does not decide it.
    pub fn settled(&self) -> f64 {
        let s = self.span(); if !(s > 0.0) || !self.rising() { return self.y[self.len() - 1] }
        self.window(Some(self.x[self.len() - 1] - 0.02 * s), None).map(|w| w.mean()).unwrap_or(self.y[self.len() - 1])
    }
    /// Gain in dB of each point (a real vector is taken as a magnitude).
    pub fn db(&self) -> Vec<f64> { (0..self.len()).map(|i| 20.0 * self.im.as_ref().map_or(self.y[i].abs(), |m| self.y[i].hypot(m[i])).log10()).collect() }
    /// Phase in degrees, unwrapped (0 for a real vector).
    pub fn phase(&self) -> Vec<f64> { match &self.im { Some(m) => unwrap_deg(&(0..self.len()).map(|i| m[i].atan2(self.y[i]).to_degrees()).collect::<Vec<_>>()), None => vec![0.0; self.len()] } }
}

fn num(p: &Value, k: &str) -> Option<f64> { p.get(k).and_then(Value::as_f64) }
fn need(p: &Value, k: &str, what: &str) -> Result<f64, String> { num(p, k).ok_or(format!("`{k}` is needed: {what}")) }

/// A step's two levels: where it starts (`initial`, else the first sample) and where it ends (`final`, else the settled end).
fn step(s: &Sig, p: &Value) -> Result<(f64, f64), String> {
    let (y0, yf) = (num(p, "initial").unwrap_or(s.y[0]), num(p, "final").unwrap_or_else(|| s.settled()));
    if (yf - y0).abs() <= 1e-12 * yf.abs().max(y0.abs()).max(1e-300) { return Err(format!("no step in the data: it starts and ends at {yf:e} (give `initial` and `final`, or a window with `from`, `to`)")) }
    Ok((y0, yf))
}
/// The two levels of an edge: `low`, `high`, else the step's when it goes the wanted way, else the data's lowest and highest (a pulse train).
fn edge_levels(s: &Sig, p: &Value, rising: bool) -> (f64, f64) {
    let st = step(s, &json!({})).ok().filter(|(a, b)| (b > a) == rising).map(|(a, b)| if rising { (a, b) } else { (b, a) });
    let (l, h) = st.unwrap_or((s.min(), s.max()));
    (num(p, "low").unwrap_or(l), num(p, "high").unwrap_or(h))
}
fn edge_time(s: &Sig, p: &Value, rising: bool) -> Result<f64, String> {
    let (l, h) = edge_levels(s, p, rising); let (lo, hi) = (num(p, "lo").unwrap_or(0.1), num(p, "hi").unwrap_or(0.9));
    if !(h > l) { return Err("no edge in the data: its low and high levels are the same".into()) }
    let (first, second, dir) = if rising { (l + lo * (h - l), l + hi * (h - l), Dir::Rise) } else { (l + hi * (h - l), l + lo * (h - l), Dir::Fall) };
    let word = if rising { "rising" } else { "falling" };
    let t1 = *crossings(&s.x, &s.y, first, dir, s.log).first().ok_or(format!("no {word} edge through {first:e} in the data"))?;
    let t2 = crossings(&s.x, &s.y, second, dir, s.log).into_iter().find(|t| *t >= t1).ok_or(format!("the {word} edge does not reach {second:e} in the data"))?;
    Ok(t2 - t1)
}
/// The rising crossings of the mid level that start a period: one counts only after the signal was a tenth of its swing below the level,
/// so noise about the level is not a period.
fn periods(s: &Sig, p: &Value) -> Result<(Vec<f64>, f64), String> {
    let (lo, hi) = (s.min(), s.max()); let level = num(p, "level").unwrap_or((lo + hi) / 2.0); let h = (hi - lo) * 0.1;
    if !(hi > lo) { return Err("the signal is constant: no period".into()) }
    let (mut armed, mut out) = (false, vec![]);
    for i in 0..s.len() - 1 {
        if s.y[i] < level - h { armed = true }
        if armed && s.y[i] < level && s.y[i + 1] >= level { out.push(interp_x(s.x[i], s.x[i + 1], (level - s.y[i]) / (s.y[i + 1] - s.y[i]), s.log)); armed = false }
    }
    if out.len() < 2 { return Err(format!("fewer than two periods in the data ({} rising crossing{} of {level:e}): simulate longer or give `level`", out.len(), if out.len() == 1 { "" } else { "s" })) }
    Ok((out, level))
}

/// The corners of a frequency response about its reference level.
#[derive(Debug, Clone, PartialEq)]
pub struct Band { pub reference_db: f64, pub low: Option<f64>, pub high: Option<f64>, pub peak_db: f64, pub peak_at: f64 }
/// Where the gain is `drop` dB under the reference: the DC gain when the response starts within `drop` of its peak (a low-pass), the
/// high-frequency gain when it ends there (a high-pass), else the peak (a band-pass); `ref` says otherwise.
pub fn band(s: &Sig, p: &Value) -> Result<Band, String> {
    s.ok()?; let db = s.db(); let n = db.len(); let k = arg_max(&db); let drop = num(p, "drop").unwrap_or(HALF_POWER_DB);
    let reference = match &p["ref"] { Value::Number(v) => v.as_f64().unwrap_or(db[k]), Value::String(w) => match w.as_str() { "dc" => db[0], "hf" => db[n - 1], "peak" => db[k], o => return Err(format!("ref is dc, hf, peak or a dB value, not `{o}`")) },
        _ => if db[k] - db[0] <= drop { db[0] } else if db[k] - db[n - 1] <= drop { db[n - 1] } else { db[k] } };
    let level = reference - drop;
    let high = crossings(&s.x[k..], &db[k..], level, Dir::Fall, true).first().copied();
    let low = crossings(&s.x[..=k], &db[..=k], level, Dir::Rise, true).last().copied();
    if low.is_none() && high.is_none() { return Err(format!("the gain never falls {drop:.2} dB under {reference:.2} dB between {:e} and {:e} Hz: sweep further", s.x[0], s.x[n - 1])) }
    Ok(Band { reference_db: reference, low, high, peak_db: db[k], peak_at: s.x[k] })
}
/// A loop gain's phase for a margin: unwrapped, with the feedback's inversion taken out when the vector includes it, and brought to where
/// it starts between -270 and 90 degrees (a loop gain starts at 0, -90 or -180).
pub fn loop_phase(s: &Sig, inverted: bool) -> Vec<f64> {
    let mut ph = s.phase(); if ph.is_empty() { return ph }
    let first = ph[0] + if inverted { 180.0 } else { 0.0 };
    let turn = -360.0 * ((first - 90.0) / 360.0).ceil() + if inverted { 180.0 } else { 0.0 };
    for v in ph.iter_mut() { *v += turn }
    ph
}
fn unity(s: &Sig, p: &Value) -> Result<f64, String> {
    let db = s.db(); let level = num(p, "level").unwrap_or(0.0);
    crossings(&s.x, &db, level, Dir::Fall, true).first().copied().ok_or_else(|| { let (lo, hi) = (db[arg_min(&db)], db[arg_max(&db)]);
        format!("the gain never falls through {level} dB between {:e} and {:e} Hz (it runs from {lo:.1} to {hi:.1} dB): {}", s.x[0], s.x[s.len() - 1], if lo > level { "sweep to a higher frequency" } else if hi < level { "the loop gain is below unity everywhere" } else { "it only rises through it" }) })
}
/// A noise density integrated between two frequencies, RMS. Between samples the density follows a power law (a straight line on the
/// log-log plot), so a 1/f or a one-pole roll-off is integrated as it is, not as a staircase of a few points a decade.
pub fn noise_rms(s: &Sig, from: Option<f64>, to: Option<f64>, power: bool) -> Result<f64, String> {
    let w = s.window(from, to)?; let d: Vec<f64> = if power { w.y.clone() } else { w.y.iter().map(|v| v * v).collect() };
    let mut sum = 0.0;
    for i in 1..w.len() {
        let (f1, f2, s1, s2) = (w.x[i - 1], w.x[i], d[i - 1], d[i]);
        if f2 <= f1 { continue }
        sum += if f1 > 0.0 && s1 > 0.0 && s2 > 0.0 { let a = (s2 / s1).ln() / (f2 / f1).ln(); if (a + 1.0).abs() < 1e-9 { s1 * f1 * (f2 / f1).ln() } else { s1 * f1 / (a + 1.0) * ((f2 / f1).powf(a + 1.0) - 1.0) } }
            else { (f2 - f1) * (s1 + s2) / 2.0 };
    }
    Ok(sum.sqrt())
}

/// One measure by its name (`MEASURES`) with its parameters; `from` and `to` window any of them first.
pub fn measure(name: &str, p: &Value, sig: &Sig) -> Result<Value, String> {
    sig.ok()?;
    if name == "noise_rms" { return noise_rms(sig, num(p, "from"), num(p, "to"), p["power"] == true).map(|v| json!(v)) }
    let s = &sig.window(num(p, "from"), num(p, "to"))?; let ac = s.im.is_some();
    let n = |v: f64| if v.is_finite() { Ok(json!(v)) } else { Err(format!("{name} is not a number here (a zero or an empty vector)")) };
    let at = |what: &str| -> Result<f64, String> { let x = need(p, "x", what)?; if x < s.x[0] || x > s.x[s.len() - 1] { return Err(format!("x = {x:e} is outside the data ({:e}..{:e})", s.x[0], s.x[s.len() - 1])) } Ok(x) };
    match name {
        "at" => { let x = at("where to read the value")?;
            if ac { Ok(json!({"db": value_at(&s.x, &s.db(), x, true), "deg": value_at(&s.x, &s.phase(), x, true)})) } else { n(value_at(&s.x, &s.y, x, s.log).unwrap_or(f64::NAN)) } }
        "gain_at" => n(value_at(&s.x, &s.db(), at("the frequency")?, true).unwrap_or(f64::NAN)),
        "phase_at" => n(value_at(&s.x, &s.phase(), at("the frequency")?, true).unwrap_or(f64::NAN)),
        "cross" => { let level = need(p, "level", "the level it crosses")?;
            let dir = match p["dir"].as_str().unwrap_or("any") { "rise" | "rising" | "up" => Dir::Rise, "fall" | "falling" | "down" => Dir::Fall, _ => Dir::Any };
            let y = if ac { s.db() } else { s.y.clone() }; let all = crossings(&s.x, &y, level, dir, s.log);
            match p["n"].as_u64() { Some(k) => all.get((k.max(1) - 1) as usize).map(|v| json!(v)).ok_or(format!("it crosses {level:e} {} time{} in the data, not {k}", all.len(), if all.len() == 1 { "" } else { "s" })),
                None => Ok(json!(all.iter().take(20).collect::<Vec<_>>())) } }
        "min" => n(s.min()), "max" => n(s.max()), "mean" => n(s.mean()), "final" => n(s.settled()), "rms" => n(s.rms()),
        "pp" | "ripple" => n(s.max() - s.min()), "ripple_rms" => n(s.ac_rms()),
        "peak" => if ac { let db = s.db(); let k = arg_max(&db); Ok(json!({"db": db[k], "at": s.x[k]})) } else { let k = arg_by(&s.y, |a, b| a.abs() > b.abs()); Ok(json!({"value": s.y[k], "at": s.x[k]})) },
        "overshoot" => { let (y0, yf) = step(s, p)?; n((if yf > y0 { s.max() - yf } else { yf - s.min() }).max(0.0) / (yf - y0).abs() * 100.0) }
        "undershoot" => { let (y0, yf) = step(s, p)?; n((if yf > y0 { y0 - s.min() } else { s.max() - y0 }).max(0.0) / (yf - y0).abs() * 100.0) }
        "rise_time" => n(edge_time(s, p, true)?), "fall_time" => n(edge_time(s, p, false)?),
        "settling_time" => { let (y0, yf) = step(s, p)?; let tol = num(p, "tol").unwrap_or(num(p, "band").unwrap_or(0.02) * (yf - y0).abs());
            let t0 = num(p, "t0").unwrap_or(s.x[0]);
            match (0..s.len()).rev().find(|i| (s.y[*i] - yf).abs() > tol) {
                None => n(0.0),
                Some(i) if i + 1 >= s.len() => Err(format!("not settled within {tol:e} of {yf:e} by the end of the data: simulate longer")),
                Some(i) => { let (a, b) = ((s.y[i] - yf).abs() - tol, (s.y[i + 1] - yf).abs() - tol); n(interp_x(s.x[i], s.x[i + 1], a / (a - b), s.log) - t0) } } }
        "frequency" | "period" | "duty" => { let (t, level) = periods(s, p)?; let (first, last, k) = (t[0], t[t.len() - 1], (t.len() - 1) as f64);
            match name { "frequency" => n(k / (last - first)), "period" => n((last - first) / k),
                _ => { let falls = crossings(&s.x, &s.y, level, Dir::Fall, s.log); let high: f64 = t.windows(2).map(|w| falls.iter().find(|f| **f > w[0] && **f <= w[1]).map_or(w[1] - w[0], |f| f - w[0])).sum(); n(high / (last - first) * 100.0) } } }
        "dc_gain" => { let g = s.db()[0]; n(if p["linear"] == true { 10f64.powf(g / 20.0) } else { g }) }
        "corner" | "f3db" => { let b = band(s, p)?; match (b.low, b.high) { (Some(l), Some(h)) => Ok(json!({"low": l, "high": h})), (l, h) => n(l.or(h).unwrap_or(f64::NAN)) } }
        "bandwidth" | "center" | "q" => { let b = band(s, p)?;
            match (b.low, b.high, name) { (None, Some(h), "bandwidth") => n(h), (Some(l), Some(h), "bandwidth") => n(h - l), (Some(l), Some(h), "center") => n((l * h).sqrt()), (Some(l), Some(h), _) => n((l * h).sqrt() / (h - l)),
                (Some(_), None, _) => Err("a high-pass: the gain does not come down again in the data, so it has a corner but no bandwidth".into()),
                _ => Err(format!("{name} needs a band-pass (a corner each side of the peak); this response has one corner: ask for `corner`")) } }
        "unity_gain_freq" => n(unity(s, p)?),
        "phase_margin" => { let f = unity(s, p)?; n(180.0 + value_at(&s.x, &loop_phase(s, p["inverted"] == true), f, true).unwrap_or(f64::NAN)) }
        "gain_margin" => { let ph = loop_phase(s, p["inverted"] == true);
            let f = crossings(&s.x, &ph, -180.0, Dir::Fall, true).first().copied().ok_or(format!("the phase never falls through -180 degrees between {:e} and {:e} Hz (it ends at {:.1}): the gain margin is unbounded there, or sweep further", s.x[0], s.x[s.len() - 1], ph[ph.len() - 1]))?;
            n(-value_at(&s.x, &s.db(), f, true).unwrap_or(f64::NAN)) }
        other => Err(format!("no measure `{other}`: {}", MEASURES.iter().map(|m| m.0).collect::<Vec<_>>().join(", "))),
    }
}

// ---- compaction ------------------------------------------------------------------------------------------------------------

/// At most `n` sample indices that keep the shape: the first and the last, and the lowest and highest of every series in each of the
/// equal parts the axis is cut into (equal in log x for a frequency axis), so no peak or dip is lost to the sampling.
pub fn pick(x: &[f64], ys: &[&[f64]], n: usize, log: bool) -> Vec<usize> {
    let len = x.len(); if len <= n.max(2) { return (0..len).collect() }
    let per = 2 * ys.len().max(1); let buckets = (n.saturating_sub(2) / per).max(1);
    let rising = x.windows(2).all(|w| w[1] >= w[0]) && x[len - 1] > x[0]; let log = log && rising && x[0] > 0.0;
    let place = |i: usize| -> usize { let t = if !rising { i as f64 / len as f64 } else if log { (x[i] / x[0]).ln() / (x[len - 1] / x[0]).ln() } else { (x[i] - x[0]) / (x[len - 1] - x[0]) }; ((t * buckets as f64) as usize).min(buckets - 1) };
    let mut ext: Vec<Option<(usize, usize)>> = vec![None; buckets * ys.len().max(1)];
    for i in 0..len { let b = place(i);
        for (k, y) in ys.iter().enumerate() { let e = &mut ext[b * ys.len() + k]; *e = Some(match *e { None => (i, i), Some((lo, hi)) => (if y[i] < y[lo] { i } else { lo }, if y[i] > y[hi] { i } else { hi }) }) } }
    let mut out: Vec<usize> = ext.into_iter().flatten().flat_map(|(a, b)| [a, b]).chain([0, len - 1]).collect();
    out.sort_unstable(); out.dedup(); out
}
/// A vector as at most `n` points, in columns: `{x, y}`, or `{f, db, deg}` of a complex one; `of` is how many it had.
pub fn compact(s: &Sig, n: usize, digits: usize) -> Value {
    let r = |v: &[f64], idx: &[usize]| idx.iter().map(|i| round_sig(v[*i], digits)).collect::<Vec<_>>();
    if s.im.is_some() { let (db, ph) = (s.db(), s.phase()); let idx = pick(&s.x, &[&db, &ph], n, true);
        return json!({"f": r(&s.x, &idx), "db": idx.iter().map(|i| (db[*i] * 100.0).round() / 100.0).collect::<Vec<_>>(), "deg": idx.iter().map(|i| (ph[*i] * 10.0).round() / 10.0).collect::<Vec<_>>(), "of": s.len()}) }
    let idx = pick(&s.x, &[&s.y], n, s.log);
    json!({"x": r(&s.x, &idx), "y": r(&s.y, &idx), "of": s.len()})
}
/// What a vector is, in a line: its count and range, and its lowest, highest, mean and settled value (a complex one: its gain at each
/// end and its peak).
pub fn summary(s: &Sig, digits: usize) -> Value {
    if s.is_empty() { return json!({"n": 0}) }
    let r = |v: f64| round_sig(v, digits);
    if s.im.is_some() { let db = s.db(); let k = arg_max(&db); let d = |v: f64| (v * 100.0).round() / 100.0;
        return json!({"n": s.len(), "from": r(s.x[0]), "to": r(s.x[s.len() - 1]), "first_db": d(db[0]), "last_db": d(db[db.len() - 1]), "peak_db": d(db[k]), "peak_at": r(s.x[k])}) }
    json!({"n": s.len(), "from": r(s.x[0]), "to": r(s.x[s.len() - 1]), "min": r(s.min()), "max": r(s.max()), "mean": r(s.mean()), "final": r(s.settled())})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn lin(a: f64, b: f64, n: usize) -> Vec<f64> { (0..n).map(|i| a + (b - a) * i as f64 / (n - 1) as f64).collect() }
    fn logs(a: f64, b: f64, per_decade: usize) -> Vec<f64> { let n = ((b / a).log10() * per_decade as f64).round() as usize; (0..=n).map(|i| a * 10f64.powf(i as f64 / per_decade as f64)).collect() }
    fn of(x: Vec<f64>, f: impl Fn(f64) -> f64) -> Sig { let y = x.iter().map(|v| f(*v)).collect(); Sig::real(x, y) }
    /// A transfer function H(s) sampled along f: `h` answers (re, im) at w = 2 pi f.
    fn tf(f: Vec<f64>, h: impl Fn(f64) -> (f64, f64)) -> Sig { let (re, im) = f.iter().map(|v| h(2.0 * PI * v)).unzip(); Sig::ac(f, re, im) }
    fn div(a: (f64, f64), b: (f64, f64)) -> (f64, f64) { let d = b.0 * b.0 + b.1 * b.1; ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d) }
    fn mul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) { (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0) }
    fn m(name: &str, p: Value, s: &Sig) -> f64 { measure(name, &p, s).unwrap_or_else(|e| panic!("{name}: {e}")).as_f64().unwrap_or_else(|| panic!("{name} is not a number")) }
    fn near(got: f64, want: f64, rel: f64) -> bool { (got - want).abs() <= rel * want.abs().max(1e-300) }
    macro_rules! close { ($got:expr, $want:expr, $rel:expr) => {{ let (g, w) = ($got, $want); assert!(near(g, w, $rel), "{} = {g:e}, want {w:e} within {}", stringify!($got), $rel) }} }

    #[test] fn values_and_crossings_land_between_samples() {
        let s = Sig::real(vec![0.0, 1.0, 2.0, 4.0], vec![0.0, 10.0, 0.0, -20.0]);
        assert_eq!((value_at(&s.x, &s.y, 0.25, false), value_at(&s.x, &s.y, 3.0, false), value_at(&s.x, &s.y, 4.0, false), value_at(&s.x, &s.y, 4.1, false), value_at(&s.x, &s.y, 0.0, false)), (Some(2.5), Some(-10.0), Some(-20.0), None, Some(0.0)));
        assert_eq!(crossings(&s.x, &s.y, 5.0, Dir::Any, false), vec![0.5, 1.5]);
        assert_eq!((crossings(&s.x, &s.y, 5.0, Dir::Rise, false), crossings(&s.x, &s.y, 5.0, Dir::Fall, false), crossings(&s.x, &s.y, -10.0, Dir::Fall, false)), (vec![0.5], vec![1.5], vec![3.0]));
        // along a frequency axis the place between two samples is on log f: half way from 10 to 1000 is 100
        assert_eq!(value_at(&[10.0, 1000.0], &[0.0, -40.0], 100.0, true), Some(-20.0));
        close!(crossings(&[10.0, 1000.0], &[0.0, -40.0], -20.0, Dir::Fall, true)[0], 100.0, 1e-12);
        assert_eq!(measure("cross", &json!({"level": 5, "dir": "fall", "n": 1}), &s), Ok(json!(1.5)));
        assert_eq!(measure("cross", &json!({"level": 5}), &s), Ok(json!([0.5, 1.5])));
        assert!(measure("cross", &json!({"level": 5, "n": 3}), &s).unwrap_err().contains("2 times"));
        assert!(measure("at", &json!({"x": 9}), &s).unwrap_err().contains("outside the data"));
        assert!(measure("at", &json!({}), &s).unwrap_err().contains("`x` is needed"));
        assert!(measure("wobble", &json!({}), &s).unwrap_err().contains("no measure `wobble`: at, cross"));
        for (name, _) in MEASURES { let r = measure(name, &json!({"x": 1, "level": 1}), &s); assert!(r.as_ref().err().map_or(true, |e| !e.starts_with("no measure")), "{name} is in the table but not answered") }
    }
    #[test] fn a_window_has_a_point_on_each_edge() {
        let s = Sig::real(vec![0.0, 1.0, 2.0, 3.0], vec![0.0, 10.0, 20.0, 30.0]);
        let w = s.window(Some(0.5), Some(2.5)).unwrap(); assert_eq!((w.x.clone(), w.y.clone()), (vec![0.5, 1.0, 2.0, 2.5], vec![5.0, 10.0, 20.0, 25.0]));
        assert_eq!(s.window(Some(-5.0), Some(1.0)).unwrap().x, vec![0.0, 1.0]);
        assert!(s.window(Some(4.0), None).unwrap_err().contains("outside the data"));
        assert!(Sig::real(vec![0.0, 1.0, 0.0, 1.0], vec![1.0; 4]).window(Some(0.5), None).unwrap_err().contains("nested sweep"));
        assert_eq!((m("mean", json!({"from": 1, "to": 3}), &s), m("max", json!({"to": 1.5}), &s), m("min", json!({"from": 0.5}), &s)), (20.0, 15.0, 5.0));
    }
    #[test] fn phase_is_unwrapped() {
        assert_eq!(unwrap_deg(&[-170.0, 175.0, 160.0, -179.0, 0.0]), vec![-170.0, -185.0, -200.0, -179.0, 0.0]);
        // three poles: the phase runs from 0 to -270 through atan2's jump at -180
        let s = tf(logs(1.0, 1e6, 20), |w| { let p = (1.0, w / (2.0 * PI * 1e3)); div((1.0, 0.0), mul(p, mul(p, p))) });
        let ph = s.phase(); assert!(ph[0] > -1.0 && ph[0] <= 0.0); close!(ph[ph.len() - 1], -270.0, 1e-2); assert!(ph.windows(2).all(|w| w[1] <= w[0]));
        close!(m("phase_at", json!({"x": 1e3}), &s), -135.0, 1e-3);
    }

    // ---- RC, closed form ----------------------------------------------------------------------------------------------------
    #[test] fn rc_low_pass_in_frequency() {
        let (r, c) = (1e3, 100e-9); let fc = 1.0 / (2.0 * PI * r * c);
        let s = tf(logs(1.0, 1e7, 20), |w| div((1.0, 0.0), (1.0, w * r * c)));
        close!(m("corner", json!({}), &s), fc, 2e-3); close!(m("bandwidth", json!({}), &s), fc, 2e-3);
        assert!(m("dc_gain", json!({}), &s).abs() < 1e-3); close!(m("dc_gain", json!({"linear": true}), &s), 1.0, 1e-4);
        close!(m("gain_at", json!({"x": fc}), &s), -HALF_POWER_DB, 5e-3); close!(m("phase_at", json!({"x": fc}), &s), -45.0, 2e-3);
        // a decade above the corner: -20.04 dB, and the -20 dB point by `drop`
        close!(m("gain_at", json!({"x": 10.0 * fc}), &s), -10.0 * 101f64.log10(), 2e-3);
        close!(m("corner", json!({"drop": 20}), &s), fc * 99f64.sqrt(), 2e-3);
        assert_eq!(measure("at", &json!({"x": 1.0}), &s).unwrap()["deg"].as_f64().map(|d| d.round()), Some(-0.0));
        assert!(measure("q", &json!({}), &s).unwrap_err().contains("needs a band-pass"));
        // five points a decade still find it within a percent
        close!(m("corner", json!({}), &tf(logs(1.0, 1e7, 5), |w| div((1.0, 0.0), (1.0, w * r * c)))), fc, 1e-2);
        // the high-pass of the same parts has the same corner, read under its high-frequency gain
        let hp = tf(logs(1.0, 1e7, 20), |w| div((0.0, w * r * c), (1.0, w * r * c)));
        close!(m("corner", json!({}), &hp), fc, 2e-3); assert!(measure("bandwidth", &json!({}), &hp).unwrap_err().contains("high-pass"));
        assert!(measure("corner", &json!({}), &tf(logs(1.0, 10.0, 20), |w| div((1.0, 0.0), (1.0, w * r * c)))).unwrap_err().contains("sweep further"));
    }
    #[test] fn rc_step_in_time() {
        let tau = 1e-3; let s = of(lin(0.0, 10.0 * tau, 2001), |t| 5.0 * (1.0 - (-t / tau).exp()));
        close!(m("rise_time", json!({}), &s), tau * 9f64.ln(), 2e-3);
        close!(m("rise_time", json!({"lo": 0.2, "hi": 0.8}), &s), tau * 4f64.ln(), 2e-3);
        close!(m("settling_time", json!({}), &s), tau * 50f64.ln(), 3e-3);
        close!(m("settling_time", json!({"band": 0.05}), &s), tau * 20f64.ln(), 3e-3);
        close!(m("settling_time", json!({"tol": 0.005, "final": 5.0}), &s), tau * 1000f64.ln(), 3e-3);
        close!(m("final", json!({}), &s), 5.0, 1e-4); close!(m("at", json!({"x": tau}), &s), 5.0 * (1.0 - (-1f64).exp()), 1e-5);
        close!(m("cross", json!({"level": 2.5, "n": 1}), &s), tau * 2f64.ln(), 1e-4);
        assert!(m("overshoot", json!({}), &s) < 1e-2 && m("undershoot", json!({}), &s) == 0.0);
        // the discharge: a fall time, and the same settling
        let d = of(lin(0.0, 10.0 * tau, 2001), |t| 5.0 * (-t / tau).exp());
        close!(m("fall_time", json!({}), &d), tau * 9f64.ln(), 2e-3); close!(m("settling_time", json!({}), &d), tau * 50f64.ln(), 3e-3);
        assert!(measure("rise_time", &json!({}), &d).unwrap_err().contains("no rising edge"));
        // cut short, it has not settled, and says so
        assert!(measure("settling_time", &json!({"final": 5.0}), &of(lin(0.0, 2.0 * tau, 200), |t| 5.0 * (1.0 - (-t / tau).exp()))).unwrap_err().contains("simulate longer"));
        assert!(measure("overshoot", &json!({}), &of(lin(0.0, 1.0, 10), |_| 3.3)).unwrap_err().contains("no step"));
    }

    // ---- second order, closed form ------------------------------------------------------------------------------------------
    fn second(z: f64, wn: f64) -> impl Fn(f64) -> f64 { move |t| { let wd = wn * (1.0 - z * z).sqrt(); 1.0 - (-z * wn * t).exp() * ((wd * t).cos() + z / (1.0 - z * z).sqrt() * (wd * t).sin()) } }
    #[test] fn second_order_step() {
        for z in [0.2f64, 0.5, 0.7] {
            let wn = 2.0 * PI * 1e3; let s = of(lin(0.0, 40.0 / wn / z.min(0.5), 4001), second(z, wn));
            close!(m("overshoot", json!({"final": 1.0}), &s), 100.0 * (-PI * z / (1.0 - z * z).sqrt()).exp(), 5e-3);
            let pk = measure("peak", &json!({}), &s).unwrap(); close!(pk["at"].as_f64().unwrap(), PI / (wn * (1.0 - z * z).sqrt()), 1e-2); close!(pk["value"].as_f64().unwrap(), 1.0 + (-PI * z / (1.0 - z * z).sqrt()).exp(), 1e-3);
            // settling against the same response sampled a hundred times finer
            let fine = of(lin(0.0, 40.0 / wn / z.min(0.5), 400_001), second(z, wn)); let last = (0..fine.len()).rev().find(|i| (fine.y[*i] - 1.0).abs() > 0.02).unwrap();
            close!(m("settling_time", json!({"final": 1.0}), &s), fine.x[last], 2e-3);
            close!(m("final", json!({}), &s), 1.0, 2e-3);
        }
        // the ringing's frequency from a decaying sine about its level
        let (z, wn) = (0.05, 2.0 * PI * 1e3); let s = of(lin(0.0, 20e-3, 8001), second(z, wn));
        close!(m("frequency", json!({"level": 1.0, "from": 1e-3}), &s), wn * (1.0f64 - z * z).sqrt() / (2.0 * PI), 2e-3);
        // a falling step overshoots under its final value, and a dip under the start of a rising one is its undershoot
        let f = of(lin(0.0, 0.02, 4001), |t| 3.0 - 2.0 * second(0.3, 2.0 * PI * 1e3)(t));
        close!(m("overshoot", json!({"final": 1.0}), &f), 100.0 * (-PI * 0.3 / (1.0f64 - 0.09).sqrt()).exp(), 5e-3);
        let u = Sig::real(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![0.0, -0.25, 0.5, 1.1, 1.0, 1.0, 1.0]);
        assert_eq!((m("undershoot", json!({"final": 1.0}), &u), (m("overshoot", json!({"final": 1.0}), &u) * 1e6).round() / 1e6), (25.0, 10.0));
    }
    #[test] fn second_order_low_pass_in_frequency() {
        let (z, f0) = (0.3, 1e4); let wn = 2.0 * PI * f0;
        let s = tf(logs(10.0, 1e7, 50), |w| div((1.0, 0.0), (1.0 - (w / wn) * (w / wn), 2.0 * z * w / wn)));
        let pk = measure("peak", &json!({}), &s).unwrap();
        close!(pk["db"].as_f64().unwrap(), 20.0 * (1.0 / (2.0 * z * (1.0f64 - z * z).sqrt())).log10(), 2e-3); close!(pk["at"].as_f64().unwrap(), f0 * (1.0f64 - 2.0 * z * z).sqrt(), 3e-2);
        // its -3 dB bandwidth is read under the DC gain, past the peak
        let bw = f0 * (1.0 - 2.0 * z * z + (4.0 * z.powi(4) - 4.0 * z * z + 2.0).sqrt()).sqrt();
        close!(m("bandwidth", json!({"ref": "dc"}), &s), bw, 2e-3); close!(m("corner", json!({"ref": 0}), &s), bw, 2e-3);
        close!(m("phase_at", json!({"x": f0}), &s), -90.0, 2e-3);
    }
    #[test] fn rlc_band_pass() {
        // series RLC, the output across R: f0 = 1/(2 pi sqrt(LC)), bandwidth R/(2 pi L), Q = f0 / bandwidth
        let (r, l, c) = (50.0, 1e-3, 100e-9); let (f0, bw) = (1.0 / (2.0 * PI * (l * c as f64).sqrt()), r / (2.0 * PI * l));
        let s = tf(logs(100.0, 1e7, 100), |w| div((0.0, w * r * c), (1.0 - w * w * l * c, w * r * c)));
        close!(m("bandwidth", json!({}), &s), bw, 3e-3); close!(m("center", json!({}), &s), f0, 1e-3); close!(m("q", json!({}), &s), f0 / bw, 3e-3);
        let k = measure("corner", &json!({}), &s).unwrap(); let (lo, hi) = (k["low"].as_f64().unwrap(), k["high"].as_f64().unwrap());
        close!(lo, -bw / 2.0 + (bw * bw / 4.0 + f0 * f0).sqrt(), 2e-3); close!(hi, bw / 2.0 + (bw * bw / 4.0 + f0 * f0).sqrt(), 2e-3);
        let pk = measure("peak", &json!({}), &s).unwrap(); assert!(pk["db"].as_f64().unwrap().abs() < 0.01); close!(pk["at"].as_f64().unwrap(), f0, 2e-2);
        // the capacitor's voltage of the same circuit rings: an under-damped step at 1/(2 pi sqrt(LC)) sqrt(1 - z^2), z = R/2 sqrt(C/L)
        let z = r / 2.0 * (c / l as f64).sqrt(); let wn = 2.0 * PI * f0; let t = of(lin(0.0, 2e-3, 20_001), second(z, wn));
        close!(m("overshoot", json!({"final": 1.0}), &t), 100.0 * (-PI * z / (1.0 - z * z).sqrt()).exp(), 2e-3);
        close!(m("frequency", json!({"level": 1.0}), &t), f0 * (1.0 - z * z).sqrt(), 2e-3);
    }

    // ---- a loop gain ----------------------------------------------------------------------------------------------------------
    #[test] fn margins_of_a_loop_gain() {
        // T = K / (s (1 + s/p)): unity at wc^2 = p^2 (sqrt(1 + 4 K^2/p^2) - 1) / 2, phase margin 90 - atan(wc/p), no gain margin
        let (k, p) = (2.0 * PI * 1e4, 2.0 * PI * 2e4); let wc = p * (((1.0 + 4.0 * k * k / (p * p)).sqrt() - 1.0) / 2.0).sqrt();
        let t = |w: f64| div((k, 0.0), mul((0.0, w), (1.0, w / p)));
        let s = tf(logs(10.0, 1e7, 20), t);
        close!(m("unity_gain_freq", json!({}), &s), wc / (2.0 * PI), 2e-3); close!(m("phase_margin", json!({}), &s), 90.0 - (wc / p).atan().to_degrees(), 2e-3);
        assert!(measure("gain_margin", &json!({}), &s).unwrap_err().contains("never falls through -180"));
        // measured with the feedback's inversion in it (-T, starting at +90): the same margin, when told
        let inv = tf(logs(10.0, 1e7, 20), |w| { let (a, b) = t(w); (-a, -b) });
        close!(m("phase_margin", json!({"inverted": true}), &inv), 90.0 - (wc / p).atan().to_degrees(), 2e-3);
        // three equal poles, T = K / (1 + s/p)^3: the phase is -180 at w = p sqrt(3), where |T| = K/8
        let (k3, p3) = (4.0, 2.0 * PI * 1e3); let t3 = |w: f64| { let q = (1.0, w / p3); div((k3, 0.0), mul(q, mul(q, q))) };
        let s3 = tf(logs(1.0, 1e6, 20), t3);
        close!(m("gain_margin", json!({}), &s3), 20.0 * (8.0f64 / k3).log10(), 5e-3);
        let wu = p3 * (k3.powf(2.0 / 3.0) - 1.0f64).sqrt(); close!(m("unity_gain_freq", json!({}), &s3), wu / (2.0 * PI), 2e-3);
        close!(m("phase_margin", json!({}), &s3), 180.0 - 3.0 * (wu / p3).atan().to_degrees(), 5e-3);
        close!(m("dc_gain", json!({}), &s3), 20.0 * k3.log10(), 1e-4);
        let inv3 = tf(logs(1.0, 1e6, 20), |w| { let (a, b) = t3(w); (-a, -b) });
        close!(m("gain_margin", json!({"inverted": true}), &inv3), 20.0 * (8.0f64 / k3).log10(), 5e-3);
        // a loop that never reaches unity says which way it missed
        assert!(measure("phase_margin", &json!({}), &tf(logs(1.0, 1e3, 20), |w| div((1e6, 0.0), (1.0, w / p3)))).unwrap_err().contains("sweep to a higher frequency"));
        assert!(measure("unity_gain_freq", &json!({}), &tf(logs(1.0, 1e3, 20), |w| div((0.5, 0.0), (1.0, w / p3)))).unwrap_err().contains("below unity everywhere"));
    }

    // ---- ripple, periodic signals, noise --------------------------------------------------------------------------------------
    #[test] fn ripple_and_periodic_signals() {
        let (a, dc, f) = (0.05, 3.3, 250e3); let s = of(lin(0.0, 40e-6, 4001), |t| dc + a * (2.0 * PI * f * t).sin());
        close!(m("ripple", json!({}), &s), 2.0 * a, 1e-3); close!(m("pp", json!({"from": 10e-6, "to": 30e-6}), &s), 2.0 * a, 1e-3);
        close!(m("ripple_rms", json!({}), &s), a / 2f64.sqrt(), 1e-3); close!(m("rms", json!({}), &s), (dc * dc + a * a / 2.0).sqrt(), 1e-6); close!(m("mean", json!({}), &s), dc, 1e-6);
        close!(m("frequency", json!({}), &s), f, 1e-4); close!(m("period", json!({}), &s), 1.0 / f, 1e-4); close!(m("duty", json!({}), &s), 50.0, 2e-3);
        // unevenly sampled (ngspice's own time steps), the mean and the RMS weigh each sample by what it covers
        let mut x = lin(0.0, 20e-6, 400); x.extend(lin(20.01e-6, 40e-6, 4000)); let u = of(x, |t| dc + a * (2.0 * PI * f * t).sin());
        close!(m("mean", json!({}), &u), dc, 1e-5); close!(m("ripple_rms", json!({}), &u), a / 2f64.sqrt(), 5e-3);
        // a 30 % pulse train with noise on its levels: the frequency and duty hold, and its edges have a rise and a fall time
        let pulse = |t: f64| { let ph = (t * 1e3).fract(); let noise = 0.02 * (t * 7.7e5).sin(); noise + if ph < 0.01 { ph / 0.01 } else if ph < 0.3 { 1.0 } else if ph < 0.31 { 1.0 - (ph - 0.3) / 0.01 } else { 0.0 } };
        let q = of(lin(0.0, 10e-3, 100_001), pulse);
        close!(m("frequency", json!({}), &q), 1e3, 1e-3); close!(m("duty", json!({}), &q), 30.0, 2e-2);
        close!(m("rise_time", json!({"low": 0, "high": 1}), &q), 8e-6, 5e-2); close!(m("fall_time", json!({"low": 0, "high": 1}), &q), 8e-6, 5e-2);
        assert!(measure("frequency", &json!({}), &of(lin(0.0, 1.5e-6, 100), |t| (2.0 * PI * f * t).sin())).unwrap_err().contains("fewer than two periods"));
        assert!(measure("frequency", &json!({}), &of(lin(0.0, 1.0, 10), |_| 1.0)).unwrap_err().contains("constant"));
    }
    #[test] fn integrated_noise() {
        // a resistor's noise through its RC: 4kTR / (1 + (f/fc)^2), in all kT/C, and 4kTR fc (atan(f2/fc) - atan(f1/fc)) over a band
        let (k, t, r, c) = (1.380649e-23, 300.0, 10e3, 1e-9); let fc = 1.0 / (2.0 * PI * r * c);
        let s = of(logs(1e-2, 1e10, 10), |f| (4.0 * k * t * r / (1.0 + (f / fc) * (f / fc))).sqrt()); let s = Sig { log: true, ..s };
        close!(m("noise_rms", json!({}), &s), (k * t / c).sqrt(), 2e-3);
        close!(m("noise_rms", json!({"from": 10.0, "to": 20e3}), &s), (4.0 * k * t * r * fc * ((20e3 / fc).atan() - (10.0 / fc).atan())).sqrt(), 2e-3);
        // white noise is density times the root of the band, and a 1/f density over a decade is its 1 Hz value times sqrt(ln 10)
        close!(m("noise_rms", json!({"from": 100.0, "to": 10100.0}), &of(logs(1.0, 1e6, 5), |_| 4e-9)), 4e-9 * 100.0, 1e-9);
        close!(m("noise_rms", json!({"from": 1.0, "to": 10.0}), &of(logs(0.1, 1e3, 3), |f| 1e-6 / f.sqrt())), 1e-6 * 10f64.ln().sqrt(), 1e-9);
        let p = of(logs(1.0, 1e6, 5), |_| 16e-18); close!(m("noise_rms", json!({"from": 100.0, "to": 10100.0, "power": true}), &p), 4e-9 * 100.0, 1e-9);
    }

    // ---- compaction -----------------------------------------------------------------------------------------------------------
    #[test] fn compaction_keeps_the_extremes() {
        // a long flat line with one narrow spike and one narrow dip: both survive forty points
        let x = lin(0.0, 1.0, 100_001); let mut y = vec![1.0; x.len()]; y[33_333] = 9.0; y[77_777] = -4.0; let s = Sig::real(x, y);
        let c = compact(&s, 40, 4); let (cx, cy) = (c["x"].as_array().unwrap(), c["y"].as_array().unwrap());
        assert!(cx.len() <= 40 && cx.len() == cy.len() && cx.len() >= 20, "{}", cx.len());
        assert_eq!((cx[0].as_f64(), cx[cx.len() - 1].as_f64(), c["of"].as_u64()), (Some(0.0), Some(1.0), Some(100_001)));
        assert!(cy.contains(&json!(9.0)) && cy.contains(&json!(-4.0)));
        assert!(cx.windows(2).all(|w| w[0].as_f64() < w[1].as_f64()));
        assert_eq!(summary(&s, 4), json!({"n": 100_001, "from": 0.0, "to": 1.0, "min": -4.0, "max": 9.0, "mean": 1.0, "final": 1.0}));
        // a short vector goes as it is, at the asked digits
        assert_eq!(compact(&Sig::real(vec![0.0, 1e-3, 2e-3], vec![1.23456789, -0.000123456, 5.0]), 40, 4), json!({"x": [0.0, 0.001, 0.002], "y": [1.235, -0.0001235, 5.0], "of": 3}));
        assert_eq!((round_sig(12345.678, 3), round_sig(-0.00123456, 2), round_sig(0.0, 3), round_sig(0.1 + 0.2, 6)), (12300.0, -0.0012, 0.0, 0.3));
        assert_eq!(summary(&Sig::default(), 4), json!({"n": 0}));
    }
    #[test] fn compaction_of_a_frequency_response_is_spread_over_the_decades() {
        // a sharp resonance on a thousand points a decade: the peak survives, and every decade keeps points
        let (r, l, c) = (5.0, 1e-3, 100e-9); let f0 = 1.0 / (2.0 * PI * (l * c as f64).sqrt());
        let s = tf(logs(1.0, 1e8, 1000), |w| div((0.0, w * r * c), (1.0 - w * w * l * c, w * r * c)));
        let v = compact(&s, 60, 4); let (f, db, deg) = (v["f"].as_array().unwrap(), v["db"].as_array().unwrap(), v["deg"].as_array().unwrap());
        assert!(f.len() <= 60 && f.len() == db.len() && db.len() == deg.len());
        let top = db.iter().map(|d| d.as_f64().unwrap()).fold(f64::MIN, f64::max); assert!(top > -0.01, "{top}");
        let at = f[db.iter().position(|d| d.as_f64() == Some(top)).unwrap()].as_f64().unwrap(); close!(at, f0, 5e-3);
        for d in 0..8 { let (a, b) = (10f64.powi(d), 10f64.powi(d + 1)); assert!(f.iter().filter(|x| { let x = x.as_f64().unwrap(); x >= a && x <= b }).count() >= 3, "decade {d}") }
        assert_eq!((deg[0].as_f64(), deg[deg.len() - 1].as_f64()), (Some(90.0), Some(-90.0)));
        let sm = summary(&s, 4); assert_eq!((sm["n"].as_u64(), sm["peak_db"].as_f64().map(|d| d.abs() < 0.01), sm["from"].as_f64()), (Some(8001), Some(true), Some(1.0)));
        // a nested sweep (the axis starts over) is cut by position, and still within the count
        let x: Vec<f64> = (0..3000).map(|i| (i % 1000) as f64).collect(); let y: Vec<f64> = (0..3000).map(|i| i as f64).collect();
        let idx = pick(&x, &[&y], 30, false); assert!(idx.len() <= 30 && idx[0] == 0 && idx[idx.len() - 1] == 2999);
    }
}
