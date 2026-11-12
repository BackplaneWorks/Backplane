//! The end-of-turn model: Smart Turn v3.2 (pipecat-ai, BSD-2-Clause, an 8 MB int8 ONNX: a Whisper Tiny encoder and a linear head), which hears
//! from the last 8 s of audio whether the speaker has finished (intonation, a trailing "um" or "and…"). Chosen over LiveKit's turn-detector, whose
//! licence allows it only inside LiveKit Agents. On this desktop a check is ~10 ms of features and ~25 ms of model on one thread (2026-11-11).
//!
//! The features are Whisper's log-mel spectrogram, as transformers' WhisperFeatureExtractor(chunk_length=8) makes them: the audio left-padded with
//! zeros to 8 s, normalised to zero mean and unit variance, a 400-point periodic Hann STFT every 160 samples (centred, reflect-padded), the power in
//! 80 Slaney mel bands to 8 kHz, log10, floored 8 below its peak, (x + 4) / 4; 800 frames. The model runs through onnxruntime's C API, from the
//! runtime the helper already loads (`OrtGetApiBase`): the handful of entries used, by their index in OrtApi (onnxruntime_c_api.h, 1.28.2, the
//! version the pinned runtime carries; the table only ever grows, so the indices hold).

use std::f64::consts::PI;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;

pub const RATE: usize = 16000;
pub const SECONDS: usize = 8;
const N: usize = SECONDS * RATE;
const NFFT: usize = 400;
const HOP: usize = 160;
const BINS: usize = NFFT / 2 + 1;
pub const MELS: usize = 80;
pub const FRAMES: usize = N / HOP;
/// The model's file, its size and sha256 (huggingface.co/pipecat-ai/smart-turn-v3, 2026-01).
pub const FILE: &str = "smart-turn-v3.2-cpu.onnx";
pub const SIZE: u64 = 8679182;
pub const SHA256: &str = "2bb026316b14a660486a75b1733cd3fbab8c2fd0314dc9af7be49f8cca967e4f";
pub const URL: &str = "https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main/smart-turn-v3.2-cpu.onnx";

// ---- features

#[derive(Clone, Copy)]
struct C(f64, f64);
/// A mixed-radix FFT for 400 = 4·4·5·5 points (decimation in time, a plain DFT across each radix). `tw` holds the 400 twiddles.
fn fft(x: &[C], radices: &[usize], tw: &[C]) -> Vec<C> {
    let n = x.len(); if n == 1 { return x.to_vec() }
    let r = radices[0]; let m = n / r; let step = NFFT / n;
    let subs: Vec<Vec<C>> = (0..r).map(|j| fft(&x.iter().skip(j).step_by(r).copied().collect::<Vec<_>>(), &radices[1..], tw)).collect();
    let mut out = vec![C(0.0, 0.0); n];
    for k in 0..m { for q in 0..r {
        let mut acc = C(0.0, 0.0);
        for (j, s) in subs.iter().enumerate() { let w = tw[(j * (k + q * m) * step) % NFFT]; let v = s[k]; acc.0 += v.0 * w.0 - v.1 * w.1; acc.1 += v.0 * w.1 + v.1 * w.0; }
        out[k + q * m] = acc;
    } }
    out
}

/// The 80 × 201 Slaney mel filters, as transformers' mel_filter_bank(norm="slaney", mel_scale="slaney") makes them.
fn mel_filters() -> Vec<[f64; BINS]> {
    let hz_to_mel = |f: f64| if f < 1000.0 { 3.0 * f / 200.0 } else { 15.0 + (f / 1000.0).ln() * 27.0 / 6.4f64.ln() };
    let mel_to_hz = |m: f64| if m < 15.0 { 200.0 * m / 3.0 } else { 1000.0 * ((m - 15.0) * 6.4f64.ln() / 27.0).exp() };
    let (lo, hi) = (hz_to_mel(0.0), hz_to_mel(8000.0));
    let pts: Vec<f64> = (0..MELS + 2).map(|i| mel_to_hz(lo + (hi - lo) * i as f64 / (MELS + 1) as f64)).collect();
    let freqs: Vec<f64> = (0..BINS).map(|i| 8000.0 * i as f64 / (BINS - 1) as f64).collect();
    (0..MELS).map(|m| {
        let mut f = [0.0; BINS];
        let enorm = 2.0 / (pts[m + 2] - pts[m]);
        for (b, &fr) in freqs.iter().enumerate() {
            let down = (fr - pts[m]) / (pts[m + 1] - pts[m]); let up = (pts[m + 2] - fr) / (pts[m + 2] - pts[m + 1]);
            f[b] = down.min(up).max(0.0) * enorm;
        }
        f
    }).collect()
}

/// Whisper's log-mel of the last 8 s of `audio` (16 kHz), 80 × 800, row-major by mel band.
pub fn features(audio: &[f32]) -> Vec<f32> {
    let tail = &audio[audio.len().saturating_sub(N)..];
    let mut x = vec![0f64; N]; for (i, &v) in tail.iter().enumerate() { x[N - tail.len() + i] = v as f64 }
    let mean = x.iter().sum::<f64>() / N as f64; let var = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / N as f64;
    let sd = (var + 1e-7).sqrt(); for v in x.iter_mut() { *v = (*v - mean) / sd }
    // centred frames: reflect-pad NFFT/2 on each side
    let pad = NFFT / 2; let at = |i: isize| -> f64 { let n = N as isize; let j = if i < 0 { -i } else if i >= n { 2 * (n - 1) - i } else { i }; x[j as usize] };
    let window: Vec<f64> = (0..NFFT).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / NFFT as f64).cos()).collect();
    let tw: Vec<C> = (0..NFFT).map(|k| { let a = -2.0 * PI * k as f64 / NFFT as f64; C(a.cos(), a.sin()) }).collect();
    let filters = mel_filters();
    let mut out = vec![0f64; MELS * FRAMES]; let mut frame = vec![C(0.0, 0.0); NFFT];
    for t in 0..FRAMES { // the STFT has FRAMES + 1 frames; Whisper drops the last
        let s = (t * HOP) as isize - pad as isize;
        for i in 0..NFFT { frame[i] = C(at(s + i as isize) * window[i], 0.0) }
        let spec = fft(&frame, &[4, 4, 5, 5], &tw);
        let power: Vec<f64> = spec[..BINS].iter().map(|c| c.0 * c.0 + c.1 * c.1).collect();
        for (m, f) in filters.iter().enumerate() { let e: f64 = f.iter().zip(&power).map(|(a, b)| a * b).sum(); out[m * FRAMES + t] = e.max(1e-10).log10() }
    }
    let peak = out.iter().cloned().fold(f64::MIN, f64::max);
    out.iter().map(|&v| ((v.max(peak - 8.0) + 4.0) / 4.0) as f32).collect()
}

// ---- onnxruntime

type Status = *mut c_void;
/// The OrtApi entries used, by index (see the module's note).
mod ix { pub const GET_ERROR_MESSAGE: usize = 2; pub const CREATE_ENV: usize = 3; pub const CREATE_SESSION: usize = 7; pub const RUN: usize = 9; pub const CREATE_SESSION_OPTIONS: usize = 10;
    pub const SET_GRAPH_OPT: usize = 23; pub const SET_INTRA: usize = 24; pub const SET_INTER: usize = 25; pub const CREATE_TENSOR: usize = 49; pub const TENSOR_DATA: usize = 51;
    pub const CREATE_CPU_MEMORY_INFO: usize = 69; pub const RELEASE_ENV: usize = 92; pub const RELEASE_STATUS: usize = 93; pub const RELEASE_MEMORY_INFO: usize = 94; pub const RELEASE_SESSION: usize = 95;
    pub const RELEASE_VALUE: usize = 96; pub const RELEASE_SESSION_OPTIONS: usize = 100; }
#[repr(C)]
struct ApiBase { get_api: unsafe extern "C" fn(u32) -> *const *const c_void, _version: unsafe extern "C" fn() -> *const c_char }

/// The loaded model: one session, one thread, the whole graph optimised.
pub struct Turn { _lib: libloading::Library, api: *const *const c_void, env: *mut c_void, session: *mut c_void, mem: *mut c_void }
unsafe impl Send for Turn {}

macro_rules! f { ($t:ty, $s:expr, $i:expr) => { std::mem::transmute::<*const c_void, $t>(*$s.api.add($i)) } }

impl Turn {
    unsafe fn ok(&self, st: Status) -> Result<(), String> {
        if st.is_null() { return Ok(()) }
        let msg: unsafe extern "C" fn(Status) -> *const c_char = f!(_, self, ix::GET_ERROR_MESSAGE);
        let m = CStr::from_ptr(msg(st)).to_string_lossy().into_owned();
        let rel: unsafe extern "C" fn(Status) = f!(_, self, ix::RELEASE_STATUS); rel(st); Err(m)
    }
    /// The model at `model` with the onnxruntime library at `ort` (the runtime's).
    pub fn load(ort: &Path, model: &Path) -> Result<Turn, String> {
        unsafe {
            let lib = libloading::Library::new(ort).map_err(|e| e.to_string())?;
            let base: unsafe extern "C" fn() -> *const ApiBase = *lib.get(b"OrtGetApiBase\0").map_err(|e| e.to_string())?;
            let api = ((*base()).get_api)(1); // version 1: every entry used is from then
            if api.is_null() { return Err("onnxruntime gave no API".into()) }
            let mut t = Turn { _lib: lib, api, env: std::ptr::null_mut(), session: std::ptr::null_mut(), mem: std::ptr::null_mut() };
            let id = CString::new("backplane-turn").unwrap();
            let create_env: unsafe extern "C" fn(i32, *const c_char, *mut *mut c_void) -> Status = f!(_, t, ix::CREATE_ENV);
            let mut env = std::ptr::null_mut(); t.ok(create_env(3, id.as_ptr(), &mut env))?; t.env = env; // ORT_LOGGING_LEVEL_ERROR
            let mut so = std::ptr::null_mut();
            let cso: unsafe extern "C" fn(*mut *mut c_void) -> Status = f!(_, t, ix::CREATE_SESSION_OPTIONS); t.ok(cso(&mut so))?;
            let set_i: unsafe extern "C" fn(*mut c_void, i32) -> Status = f!(_, t, ix::SET_INTRA); t.ok(set_i(so, 1))?;
            let set_e: unsafe extern "C" fn(*mut c_void, i32) -> Status = f!(_, t, ix::SET_INTER); t.ok(set_e(so, 1))?;
            let set_o: unsafe extern "C" fn(*mut c_void, i32) -> Status = f!(_, t, ix::SET_GRAPH_OPT); t.ok(set_o(so, 99))?; // ORT_ENABLE_ALL
            #[cfg(unix)] let path = CString::new(model.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
            #[cfg(unix)] let pp = path.as_ptr() as *const c_void;
            #[cfg(windows)] let wide: Vec<u16> = { use std::os::windows::ffi::OsStrExt; model.as_os_str().encode_wide().chain(Some(0)).collect() };
            #[cfg(windows)] let pp = wide.as_ptr() as *const c_void;
            let cs: unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void, *mut *mut c_void) -> Status = f!(_, t, ix::CREATE_SESSION);
            let mut sess = std::ptr::null_mut(); let r = t.ok(cs(t.env, pp, so, &mut sess)); t.session = sess;
            let rso: unsafe extern "C" fn(*mut c_void) = f!(_, t, ix::RELEASE_SESSION_OPTIONS); rso(so); r?;
            let cmi: unsafe extern "C" fn(i32, i32, *mut *mut c_void) -> Status = f!(_, t, ix::CREATE_CPU_MEMORY_INFO); let mut mem = std::ptr::null_mut(); t.ok(cmi(0, 0, &mut mem))?; t.mem = mem; // device allocator, default memory
            Ok(t)
        }
    }
    /// The probability that the speaker has finished, from the audio so far (16 kHz; the last 8 s are heard).
    pub fn predict(&self, audio: &[f32]) -> Result<f32, String> { self.run(&features(audio)) }
    pub fn run(&self, feats: &[f32]) -> Result<f32, String> {
        unsafe {
            let shape: [i64; 3] = [1, MELS as i64, FRAMES as i64]; let mut input = std::ptr::null_mut();
            let ct: unsafe extern "C" fn(*const c_void, *mut c_void, usize, *const i64, usize, i32, *mut *mut c_void) -> Status = f!(_, self, ix::CREATE_TENSOR);
            self.ok(ct(self.mem, feats.as_ptr() as *mut c_void, feats.len() * 4, shape.as_ptr(), 3, 1, &mut input))?; // float
            let (iname, oname) = (CString::new("input_features").unwrap(), CString::new("logits").unwrap());
            let (ins, outs) = ([iname.as_ptr()], [oname.as_ptr()]); let mut out: [*mut c_void; 1] = [std::ptr::null_mut()];
            let run: unsafe extern "C" fn(*mut c_void, *const c_void, *const *const c_char, *const *const c_void, usize, *const *const c_char, usize, *mut *mut c_void) -> Status = f!(_, self, ix::RUN);
            let inputs = [input as *const c_void];
            let r = self.ok(run(self.session, std::ptr::null(), ins.as_ptr(), inputs.as_ptr(), 1, outs.as_ptr(), 1, out.as_mut_ptr()));
            let rv: unsafe extern "C" fn(*mut c_void) = f!(_, self, ix::RELEASE_VALUE); rv(input); r?;
            let mut p: *mut c_void = std::ptr::null_mut();
            let td: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> Status = f!(_, self, ix::TENSOR_DATA);
            let r = self.ok(td(out[0], &mut p)); let v = if r.is_ok() { *(p as *const f32) } else { 0.0 }; rv(out[0]); r?;
            Ok(v)
        }
    }
}
impl Drop for Turn {
    fn drop(&mut self) {
        unsafe {
            if !self.mem.is_null() { let r: unsafe extern "C" fn(*mut c_void) = f!(_, self, ix::RELEASE_MEMORY_INFO); r(self.mem) }
            if !self.session.is_null() { let r: unsafe extern "C" fn(*mut c_void) = f!(_, self, ix::RELEASE_SESSION); r(self.session) }
            if !self.env.is_null() { let r: unsafe extern "C" fn(*mut c_void) = f!(_, self, ix::RELEASE_ENV); r(self.env) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_fft_is_the_dft() {
        let tw: Vec<C> = (0..NFFT).map(|k| { let a = -2.0 * PI * k as f64 / NFFT as f64; C(a.cos(), a.sin()) }).collect();
        let x: Vec<C> = (0..NFFT).map(|i| C(((i * 7919) % 13) as f64 - 6.0, 0.0)).collect(); let y = fft(&x, &[4, 4, 5, 5], &tw);
        for k in [0, 1, 3, 57, 200, 399] {
            let (mut re, mut im) = (0.0, 0.0); for (n, v) in x.iter().enumerate() { let a = -2.0 * PI * (k * n) as f64 / NFFT as f64; re += v.0 * a.cos(); im += v.0 * a.sin(); }
            assert!((y[k].0 - re).abs() < 1e-6 && (y[k].1 - im).abs() < 1e-6, "bin {k}: {} {} vs {re} {im}", y[k].0, y[k].1);
        }
    }
    #[test] fn features_have_whispers_shape_and_range() {
        let a: Vec<f32> = (0..RATE * 3).map(|i| (2.0 * PI * 440.0 * i as f64 / RATE as f64).sin() as f32 * 0.3).collect();
        let f = features(&a); assert_eq!(f.len(), MELS * FRAMES);
        let max = f.iter().cloned().fold(f32::MIN, f32::max); let min = f.iter().cloned().fold(f32::MAX, f32::min);
        assert!((max - min) <= 2.0 + 1e-5, "floored 8 below the peak, /4: {min}..{max}");
    }
}
