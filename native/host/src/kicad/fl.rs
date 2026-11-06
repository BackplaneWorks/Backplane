//! Bend's F32 as the JS hub runs it: every operation rounds to single precision (Math.fround), which native f32 arithmetic
//! does for + - * / and sqrt; sin, cos and atan2 are the double functions of the single value, rounded once. `to_u32`, `read`
//! and `show` have the runtime's own rules (f32_read, f32_show), which differ from Rust's casts and Display.
use std::cell::RefCell;
use std::collections::HashMap;

pub const PI: f32 = std::f32::consts::PI;
pub fn sin(a: f32) -> f32 { (a as f64).sin() as f32 }
pub fn cos(a: f32) -> f32 { (a as f64).cos() as f32 }
pub fn atan2(y: f32, x: f32) -> f32 { (y as f64).atan2(x as f64) as f32 }
/// F32.round: half up, by floor(a + 0.5) in single precision.
pub fn round(a: f32) -> f32 { (a + 0.5).floor() }
/// F32.min, F32.max: `a < b`, with what that says of a NaN.
pub fn min(a: f32, b: f32) -> f32 { if a < b { a } else { b } }
pub fn max(a: f32, b: f32) -> f32 { if a < b { b } else { a } }
pub fn hypot(a: f32, b: f32) -> f32 { (a * a + b * b).sqrt() }
/// F32.to_u32: 1 <= x < 2^32 truncates, anything else (a fraction, a negative, NaN, too big) is 0.
pub fn to_u32(x: f32) -> u32 { if x >= 1.0 && (x as f64) < 4294967296.0 { x as u32 } else { 0 } }
pub fn from_u(n: u32) -> f32 { n as f32 }

/// F32.read (f32_read): optional leading white space, a sign, a decimal or inf/infinity/nan in any case; correctly rounded.
pub fn read(s: &str) -> Option<f32> { s.trim_start_matches(['\t', '\n', '\u{b}', '\u{c}', '\r', ' ']).parse::<f32>().ok() }
/// Sx.num: 0 for what does not read.
pub fn num(s: &str) -> f32 { read(s).unwrap_or(0.0) }

/// JavaScript's String(n) for sign, digits (no trailing zeros) and the decimal exponent of the first digit.
fn js_text(neg: bool, ds: &str, e: i32) -> String {
    let (k, n) = (ds.len() as i32, e + 1);
    let mut o = String::with_capacity(24);
    if neg { o.push('-') }
    if k <= n && n <= 21 { o.push_str(ds); for _ in 0..n - k { o.push('0') } }
    else if 0 < n && n <= 21 { o.push_str(&ds[..n as usize]); o.push('.'); o.push_str(&ds[n as usize..]) }
    else if -6 < n && n <= 0 { o.push_str("0."); for _ in 0..-n { o.push('0') } o.push_str(ds) }
    else {
        o.push_str(&ds[..1]);
        if k > 1 { o.push('.'); o.push_str(&ds[1..]) }
        o.push('e'); o.push(if n - 1 < 0 { '-' } else { '+' }); o.push_str(&(n - 1).abs().to_string())
    }
    o
}
fn show_slow(x: f32) -> String {
    if x.is_nan() { return "nan".into() }
    if x.is_infinite() { return if x < 0.0 { "-inf".into() } else { "inf".into() } }
    if x == 0.0 { return if x.is_sign_negative() { "-0".into() } else { "0".into() } }
    // every digit of the double the single is: toExponential rounds it half up
    let t = format!("{:.170e}", (x as f64).abs());
    let (m, e) = t.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    let all: Vec<u8> = m.bytes().filter(|b| *b != b'.').map(|b| b - b'0').collect();
    let mut s = String::new();
    for p in 1..=9usize {
        let mut d: Vec<u8> = all[..p].to_vec();
        let mut ex = e;
        if all[p] >= 5 {
            let mut i = p;
            loop {
                if i == 0 { d.insert(0, 1); d.pop(); ex += 1; break }
                i -= 1;
                if d[i] == 9 { d[i] = 0 } else { d[i] += 1; break }
            }
        }
        while d.len() > 1 && *d.last().unwrap() == 0 { d.pop(); }
        let ds: String = d.iter().map(|b| (b + b'0') as char).collect();
        s = js_text(x < 0.0, &ds, ex);
        if s.parse::<f32>().ok() == Some(x) { break }
    }
    s
}
thread_local! { static SHOWN: RefCell<HashMap<u32, String>> = RefCell::new(HashMap::new()); }
/// F32.show (f32_show): the fewest digits, up to 9, that read back as x. A board shows few distinct widths: kept by bits.
pub fn show(x: f32) -> String {
    SHOWN.with(|m| { let mut m = m.borrow_mut(); if m.len() > 4096 { m.clear() } m.entry(x.to_bits()).or_insert_with(|| show_slow(x)).clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reads_as_the_runtime_does() {
        for (s, v) in [("1.6", 1.6f32), (" \t2", 2.0), ("+.5", 0.5), ("5.", 5.0), ("1e3", 1000.0), ("1E-2", 0.01), ("-0.25", -0.25)] { assert_eq!(read(s), Some(v), "{s}") }
        for s in ["", " ", ".", "e5", "1e", "abc", "1 ", "0x10", "1_0", "--1", "١"] { assert_eq!(read(s), None, "{s}") }
        assert_eq!(read("inf"), Some(f32::INFINITY)); assert_eq!(read("-Infinity"), Some(f32::NEG_INFINITY)); assert!(read("NaN").unwrap().is_nan());
        assert_eq!(num("x"), 0.0); assert!(read("-0").unwrap().is_sign_negative());
    }
    #[test] fn shows_as_the_runtime_does() {
        for (x, s) in [(0.25f32, "0.25"), (0.15, "0.15"), (0.1524, "0.1524"), (1.6, "1.6"), (100.0, "100"), (1e21, "1e+21"), (1.5e-7, "1.5e-7"), (0.000001, "0.000001"), (-2.5, "-2.5"), (0.0, "0"), (123456790.0, "123456790"), (16777216.0, "16777216"), (0.125, "0.125"), (3.4028235e38, "3.4028235e+38")] { assert_eq!(show(x), s, "{x}") }
        assert_eq!(show(-0.0), "-0"); assert_eq!(show(f32::NAN), "nan"); assert_eq!(show(f32::INFINITY), "inf"); assert_eq!(show(f32::NEG_INFINITY), "-inf");
    }
    #[test] fn to_u32_as_the_runtime_does() {
        for (x, v) in [(0.5f32, 0u32), (-3.0, 0), (1.0, 1), (2.9, 2), (4294967296.0, 0), (4294967040.0, 4294967040), (f32::NAN, 0)] { assert_eq!(to_u32(x), v, "{x}") }
    }
}
