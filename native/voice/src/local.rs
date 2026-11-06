//! What the local engine downloads, and where it lives: <dir> is
//! <home>/voice/local. Same pins and same folders as 0.14, so an install made
//! by the Bun helper keeps working with nothing fetched again.

use crate::audio::js_round;
use crate::net;
use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256, Sha512};
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const RATE: u32 = 16000;
pub const RUNTIME_VERSION: &str = "2.0.1";

#[derive(Clone, Debug)]
pub struct Pin {
    pub url: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
    pub sha512: Option<String>, // base64, as npm gives it
}

#[derive(Clone, Debug)]
pub struct Model {
    pub id: String,
    pub label: String,
    pub dir: String,
    pub pin: Pin,
}

/// Everything fetched; tests swap in their own.
#[derive(Clone, Debug)]
pub struct Pins {
    pub runtimes: Vec<(String, Pin)>,
    pub models: Vec<Model>,
    pub vad: Pin,
}

const GH: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/";

fn npm(plat: &str, sha512: &str) -> (String, Pin) {
    let url = format!("https://registry.npmjs.org/sherpa-onnx-{plat}/-/sherpa-onnx-{plat}-2.0.1.tgz");
    (plat.to_string(), Pin { url, size: None, sha256: None, sha512: Some(sha512.to_string()) })
}

fn github(file: &str, size: u64, sha256: &str) -> Pin {
    Pin { url: format!("{GH}{file}"), size: Some(size), sha256: Some(sha256.to_string()), sha512: None }
}

impl Pins {
    pub fn pinned() -> Pins {
        Pins {
            runtimes: vec![
                npm("linux-x64", "w5mRRrIvPvcNWac7rQ8AN/t/cwElPGoRpH+ceCRBV7XfjCIQ9Rcr3Ig/kXcJLNOxgK4bGmef+Y47lIWiesDODw=="),
                npm("linux-arm64", "5CJvLjFYIg23oELCsT7l07fMLT3PdquZ3tqh7KZK3Pzi+c2YiPKRe1DDUxoNdtCaw3ZiS4aQSQPBblB9JStuDA=="),
                npm("darwin-arm64", "agXmbbwiVPUSMViMhWhYAt59iG21ui7x6HxQFU35XzHOfQe9x6Pg9V160Pzak/66x0ePMjixsLUtMaaUrLfCwQ=="),
                npm("darwin-x64", "SBnHa3LDOSI/5/rpBHGxKQDFq+yIdxowYY0InLzfojTUx+yGgqu7cYBrRxjzvHOiOKi0O4rmlFVZboyhIqpDgA=="),
                npm("win-x64", "7vwdAw9PGEb0JLSpK7UFxAdp1vabBHK9Nhw2jFRBWSFgtslUPHIB0T6syJfRgiUGyCeNW9fJvcFAk2F2u7hBEw=="),
            ],
            models: vec![
                Model {
                    id: "parakeet-v2".into(),
                    label: "Parakeet TDT 0.6B v2 (English)".into(),
                    dir: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8".into(),
                    pin: github("sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2", 482468385, "157c157bc51155e03e37d2466522a3a737dd9c72bb25f36eb18912964161e1ad"),
                },
                Model {
                    id: "parakeet-v3".into(),
                    label: "Parakeet TDT 0.6B v3 (25 languages)".into(),
                    dir: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8".into(),
                    pin: github("sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2", 487170055, "5793d0fd397c5778d2cf2126994d58e9d56b1be7c04d13c7a15bb1b4eafb16bf"),
                },
            ],
            vad: github("silero_vad.onnx", 643854, "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"),
        }
    }

    fn model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }

    fn runtime(&self) -> Option<&Pin> {
        let p = plat();
        self.runtimes.iter().find(|r| r.0 == p).map(|r| &r.1)
    }
}

/// `linux-x64`, as Node named them.
pub fn plat() -> String {
    let os = match std::env::consts::OS {
        "windows" => "win",
        "macos" => "darwin",
        o => o,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        a => a,
    };
    format!("{os}-{arch}")
}

/// The C library in the runtime folder, and the ONNX Runtime beside it.
pub fn c_library() -> &'static str {
    if cfg!(windows) {
        "sherpa-onnx-c-api.dll"
    } else if cfg!(target_os = "macos") {
        "libsherpa-onnx-c-api.dylib"
    } else {
        "libsherpa-onnx-c-api.so"
    }
}

pub fn onnx_library() -> &'static str {
    if cfg!(windows) {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    }
}

pub fn runtime_dir(dir: &Path) -> PathBuf {
    dir.join(format!("runtime-{RUNTIME_VERSION}-{}", plat()))
}

pub fn vad_path(dir: &Path) -> PathBuf {
    dir.join("silero_vad.onnx")
}

/// Where model ID's files are, if ID is a model.
pub fn model_dir(dir: &Path, id: &str) -> Option<PathBuf> {
    Pins::pinned().model(id).map(|m| dir.join(&m.dir))
}

fn installed(dir: &Path, m: &Model) -> bool {
    let d = dir.join(&m.dir);
    d.join("encoder.int8.onnx").exists() && d.join("tokens.txt").exists()
}

/// What is installed under DIR.
pub fn status(dir: &Path) -> Value {
    status_of(dir, &Pins::pinned())
}

fn status_of(dir: &Path, pins: &Pins) -> Value {
    let models: Vec<Value> = pins.models.iter().map(|m| json!({"id": m.id, "label": m.label, "size": m.pin.size.unwrap_or(0), "installed": installed(dir, m)})).collect();
    json!({
        "platform": plat(),
        "supported": pins.runtime().is_some(),
        "runtime": runtime_dir(dir).join(c_library()).exists(),
        "vad": vad_path(dir).exists(),
        "models": models,
    })
}

/// Why model ID cannot run, if it cannot.
pub fn ready(dir: &Path, id: &str) -> Option<String> {
    let pins = Pins::pinned();
    if pins.runtime().is_none() {
        return Some(format!("local dictation is not available on {}", plat()));
    }
    let Some(m) = pins.model(id) else { return Some(format!("unknown model {id}")) };
    if !runtime_dir(dir).join(c_library()).exists() || !vad_path(dir).exists() || !installed(dir, m) {
        return Some("the local model is not downloaded (Settings → Voice)".into());
    }
    None
}

// ---- install

/// what, done, total, phase
pub type Progress<'a> = &'a mut dyn FnMut(&str, u64, u64, &str);

fn fetch_to(pin: &Pin, path: &Path, what: &str, progress: Progress) -> Result<(), String> {
    progress(what, 0, pin.size.unwrap_or(0), "download");
    let res = net::agent(None).get(&pin.url).call().map_err(|e| net::error_msg(&e))?;
    if !res.status().is_success() {
        return Err(format!("{what}: HTTP {}", res.status().as_u16()));
    }
    let length = res.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
    let total = pin.size.or(length).unwrap_or(0);
    // a model is far over ureq's default limit on a body
    let mut body = res.into_body().into_with_config().limit(u64::MAX).reader();
    let mut h256 = Sha256::new();
    let mut h512 = Sha512::new();
    let mut out = BufWriter::new(fs::File::create(path).map_err(|e| e.to_string())?);
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last: Option<Instant> = None;
    loop {
        let n = body.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h256.update(&buf[..n]);
        h512.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if last.is_none_or(|t| t.elapsed().as_millis() > 250) {
            last = Some(Instant::now());
            progress(what, done, total, "download");
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    progress(what, done, total, "download");
    if pin.size.is_some_and(|s| s != done) {
        return Err(format!("{what}: {done} bytes, expected {}", pin.size.unwrap()));
    }
    let hex: String = h256.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if pin.sha256.as_ref().is_some_and(|h| *h != hex) {
        return Err(format!("{what}: checksum mismatch"));
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(h512.finalize());
    if pin.sha512.as_ref().is_some_and(|h| *h != b64) {
        return Err(format!("{what}: checksum mismatch"));
    }
    Ok(())
}

/// Unpacks in-process; the tar crate refuses paths that climb out of INTO.
fn untar(archive: &Path, into: &Path, bz2: bool) -> Result<(), String> {
    let tar_err = |e: std::io::Error| format!("tar: {e}");
    fs::create_dir_all(into).map_err(tar_err)?;
    let f = std::io::BufReader::new(fs::File::open(archive).map_err(tar_err)?);
    let r: Box<dyn Read> = if bz2 { Box::new(bzip2::read::MultiBzDecoder::new(f)) } else { Box::new(flate2::read::MultiGzDecoder::new(f)) };
    tar::Archive::new(r).unpack(into).map_err(tar_err)
}

fn rm(p: &Path) {
    if p.is_dir() {
        let _ = fs::remove_dir_all(p);
    } else {
        let _ = fs::remove_file(p);
    }
}

fn pieces(dir: &Path, stage: &Path, pins: &Pins, m: &Model, progress: Progress) -> Result<(), String> {
    let io = |e: std::io::Error| e.to_string();
    if !vad_path(dir).exists() {
        let p = stage.join("silero_vad.onnx");
        fetch_to(&pins.vad, &p, "voice activity model", progress)?;
        fs::rename(&p, vad_path(dir)).map_err(io)?;
    }
    if !runtime_dir(dir).join(c_library()).exists() {
        let pin = pins.runtime().ok_or("no runtime for this platform")?;
        let tgz = stage.join("runtime.tgz");
        fetch_to(pin, &tgz, "speech runtime", progress)?;
        progress("speech runtime", 0, 0, "unpack");
        untar(&tgz, &stage.join("rt"), false)?;
        rm(&runtime_dir(dir));
        fs::rename(stage.join("rt").join("package"), runtime_dir(dir)).map_err(io)?;
    }
    if !installed(dir, m) {
        let tb = stage.join("model.tar.bz2");
        fetch_to(&m.pin, &tb, &m.label, progress)?;
        progress(&m.label, 0, 0, "unpack");
        untar(&tb, &stage.join("m"), true)?;
        rm(&stage.join("m").join(&m.dir).join("test_wavs"));
        rm(&dir.join(&m.dir));
        fs::rename(stage.join("m").join(&m.dir), dir.join(&m.dir)).map_err(io)?;
    }
    Ok(())
}

/// Fetch whatever is missing for model ID into DIR; each piece lands in a
/// staging folder and is moved into place only once its checksum matched.
pub fn install(dir: &Path, id: &str, pins: &Pins, progress: Progress) -> Result<(), String> {
    if pins.runtime().is_none() {
        return Err(format!("local dictation is not available on {}", plat()));
    }
    let Some(m) = pins.model(id) else { return Err(format!("unknown model {id}")) };
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let stage = dir.join(format!(".stage-{}", std::process::id()));
    rm(&stage);
    fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    let r = pieces(dir, &stage, pins, m, progress);
    rm(&stage);
    r?;
    // the hub reads this to know the model is usable
    fs::write(dir.join(format!("ready-{id}")), "ok").map_err(|e| e.to_string())
}

pub fn remove(dir: &Path, id: &str) {
    let pins = Pins::pinned();
    if let Some(m) = pins.model(id) {
        rm(&dir.join(format!("ready-{id}")));
        rm(&dir.join(&m.dir));
    }
    // the runtime and VAD go with the last model
    if !pins.models.iter().any(|m| installed(dir, m)) {
        rm(&runtime_dir(dir));
        rm(&vad_path(dir));
    }
}

pub fn disk_bytes(dir: &Path) -> u64 {
    fn walk(p: &Path) -> u64 {
        let Ok(rd) = fs::read_dir(p) else { return 0 };
        rd.flatten()
            .map(|e| {
                let q = e.path();
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    walk(&q)
                } else {
                    fs::metadata(&q).map_or(0, |m| m.len())
                }
            })
            .sum()
    }
    walk(dir)
}

// ---- words

/// A word: text, start and end in centiseconds from the segment's start, and
/// the probability (percent) of its least sure piece.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word(pub String, pub i64, pub i64, pub i64);

impl Word {
    pub fn to_json(&self) -> Value {
        json!([self.0, self.1, self.2, self.3])
    }
}

pub fn cs(sec: f64) -> i64 {
    js_round(sec * 100.0).max(0.0) as i64
}

fn pct(p: f64) -> i64 {
    js_round(p * 100.0).clamp(0.0, 100.0) as i64
}

// JavaScript's \s; Rust's whitespace is the same set but for the BOM
fn space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

fn trimmed(s: &str) -> &str {
    s.trim_matches(space)
}

/// The words of one result of SherpaOnnxGetOfflineStreamResultAsJson.
pub fn words_of(r: &Value) -> Vec<Word> {
    let nums = |k: &str| -> Vec<f64> { r.get(k).and_then(Value::as_array).map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default() };
    let toks: Vec<&str> = r.get("tokens").and_then(Value::as_array).map(|a| a.iter().map(|v| v.as_str().unwrap_or("")).collect()).unwrap_or_default();
    let (ts, ds, lp) = (nums("timestamps"), nums("durations"), nums("ys_log_probs"));
    let mut out: Vec<Word> = vec![];
    let mut gap = true; // the next piece starts a word
    for (i, t) in toks.iter().enumerate() {
        let start = ts.get(i).copied().unwrap_or(0.0);
        let d = ds.get(i).copied().unwrap_or(0.0);
        let end = start + if d != 0.0 { d } else { 0.08 };
        let p = lp.get(i).map_or(1.0, |x| x.exp());
        // a piece of only spaces (" " before a digit) opens the next word
        if trimmed(t).is_empty() {
            gap = true;
            continue;
        }
        // a piece of only punctuation (" ." at the end) closes the last word
        let punct = t.chars().all(|c| space(c) || ".,!?;:".contains(c));
        let starts_spaced = t.chars().next().is_some_and(space);
        match out.last_mut() {
            Some(last) if punct || !(gap || starts_spaced) => {
                last.0.push_str(trimmed(t));
                last.2 = last.2.max(cs(end));
                last.3 = last.3.min(pct(p));
            }
            _ => out.push(Word(trimmed(t).to_string(), cs(start), cs(end), pct(p))),
        }
        gap = t.chars().last().is_some_and(space);
    }
    out.retain(|w| !w.0.is_empty());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(t: &str, a: i64, b: i64, p: i64) -> Word {
        Word(t.into(), a, b, p)
    }

    #[test]
    fn words_from_pieces_spaces_start_words_punctuation_joins_timings_in_cs_percent() {
        let r = json!({
            "tokens": [" Open", " the", " bo", "ard", ".", " ", "8", " K", "a", "i"],
            "timestamps": [0, 0.24, 0.32, 0.4, 0.56, 0.6, 0.64, 0.8, 0.88, 0.96],
            "durations": [0.24, 0.08, 0.08, 0.16, 0.04, 0.04, 0.08, 0.08, 0.08, 0.08],
            "ys_log_probs": [0, 0, 0.9f64.ln(), 0, 0, 0, 0, 0.5f64.ln(), 0, 0],
        });
        assert_eq!(words_of(&r), vec![w("Open", 0, 24, 100), w("the", 24, 32, 100), w("board.", 32, 60, 90), w("8", 64, 72, 100), w("Kai", 80, 104, 50)]);
    }

    #[test]
    fn words_without_durations_or_probabilities() {
        let r = json!({"tokens": [" hi", "!"], "timestamps": [1.0, 1.2]});
        assert_eq!(words_of(&r), vec![w("hi!", 100, 128, 100)]);
        assert_eq!(words_of(&json!({})), vec![]);
        assert_eq!(words_of(&json!({"tokens": [" ", "."], "timestamps": [0, 0.1]})), vec![w(".", 10, 18, 100)]);
    }

    #[test]
    fn the_pins_are_0_14s() {
        let p = Pins::pinned();
        assert_eq!(p.runtimes.len(), 5);
        assert_eq!(p.model("parakeet-v2").unwrap().pin.size, Some(482468385));
        assert_eq!(p.model("parakeet-v3").unwrap().dir, "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8");
        assert_eq!(p.vad.sha256.as_deref(), Some("9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"));
        assert_eq!(p.vad.url, "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx");
        assert!(p.runtimes[0].1.url.ends_with("sherpa-onnx-linux-x64/-/sherpa-onnx-linux-x64-2.0.1.tgz"));
    }

    /// A folder as 0.14 left it: the addon and the C library side by side.
    fn install_of_0_14(dir: &Path) {
        let rt = runtime_dir(dir);
        fs::create_dir_all(&rt).unwrap();
        for f in ["sherpa-onnx.node", c_library(), onnx_library()] {
            fs::write(rt.join(f), "x").unwrap();
        }
        fs::write(vad_path(dir), "x").unwrap();
        let m = dir.join("sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8");
        fs::create_dir_all(&m).unwrap();
        for f in ["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"] {
            fs::write(m.join(f), "x").unwrap();
        }
        fs::write(dir.join("ready-parakeet-v2"), "ok").unwrap();
    }

    #[test]
    fn an_install_from_0_14_is_ready_and_a_lone_addon_is_not() {
        let t = tempfile::tempdir().unwrap();
        install_of_0_14(t.path());
        assert_eq!(ready(t.path(), "parakeet-v2"), None);
        let s = status(t.path());
        assert_eq!((&s["runtime"], &s["vad"], &s["models"][0]["installed"], &s["models"][1]["installed"]), (&json!(true), &json!(true), &json!(true), &json!(false)));
        assert_eq!(s["models"][0]["size"], json!(482468385));
        assert_eq!(s["models"][0]["label"], json!("Parakeet TDT 0.6B v2 (English)"));
        // the runtime check is the C library, not sherpa-onnx.node
        fs::remove_file(runtime_dir(t.path()).join(c_library())).unwrap();
        assert_eq!(status(t.path())["runtime"], json!(false));
        assert_eq!(ready(t.path(), "parakeet-v2").unwrap(), "the local model is not downloaded (Settings → Voice)");
    }

    #[test]
    fn ready_names_an_unknown_model() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(ready(t.path(), "nope").unwrap(), "unknown model nope");
    }

    #[test]
    fn remove_takes_the_runtime_with_the_last_model() {
        let t = tempfile::tempdir().unwrap();
        install_of_0_14(t.path());
        let v3 = t.path().join("sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8");
        fs::create_dir_all(&v3).unwrap();
        fs::write(v3.join("encoder.int8.onnx"), "x").unwrap();
        fs::write(v3.join("tokens.txt"), "x").unwrap();
        remove(t.path(), "parakeet-v2");
        assert!(!t.path().join("ready-parakeet-v2").exists());
        assert!(runtime_dir(t.path()).exists());
        remove(t.path(), "parakeet-v3");
        assert!(!runtime_dir(t.path()).exists());
        assert!(!vad_path(t.path()).exists());
    }

    #[test]
    fn disk_bytes_counts_files_all_the_way_down() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("a/b")).unwrap();
        fs::write(t.path().join("a/b/f"), "12345").unwrap();
        fs::write(t.path().join("g"), "678").unwrap();
        assert_eq!(disk_bytes(t.path()), 8);
        assert_eq!(disk_bytes(&t.path().join("none")), 0);
    }

    #[test]
    fn untar_refuses_a_path_out_of_the_folder() {
        let t = tempfile::tempdir().unwrap();
        // a hand-made header: the tar crate's builder will not write this name
        let mut h = tar::Header::new_gnu();
        h.as_old_mut().name[..9].copy_from_slice(b"../escape");
        h.set_size(1);
        h.set_mode(0o644);
        h.set_cksum();
        let mut raw = h.as_bytes().to_vec();
        raw.extend_from_slice(&[b'x']);
        raw.resize(512 * 2, 0);
        raw.resize(512 * 4, 0);
        let mut gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::fast());
        gz.write_all(&raw).unwrap();
        let p = t.path().join("a.tgz");
        fs::write(&p, gz.finish().unwrap()).unwrap();
        let into = t.path().join("deep/into");
        let _ = untar(&p, &into, false);
        assert!(!t.path().join("deep/escape").exists());
    }
}
