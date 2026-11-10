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
    /// About what a download is, for the page.
    pub size: u64,
    /// The archive. Upstream repacks it now and then (the same files, a new archive: 2026-11-07), so its own size and
    /// checksum are not what is trusted: `files` are, checked after unpacking and before anything uses them.
    pub pin: Pin,
    /// A second source of the same files, one by one (`<mirror><name>`), when the archive fails or its files do not match.
    pub mirror: Option<String>,
    /// What must be in `dir` whatever the source: name, size, sha256.
    pub files: Vec<(String, u64, String)>,
}

/// Everything fetched; tests swap in their own.
#[derive(Clone, Debug)]
pub struct Pins {
    pub runtimes: Vec<(String, Pin)>,
    pub models: Vec<Model>,
    pub vad: Pin,
    /// The VAD a release ships beside the helper (native/voice/vad): taken when it matches `vad`, so only a build without it downloads one.
    pub vad_bundled: Option<PathBuf>,
}

const GH: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/";

fn npm(plat: &str, sha512: &str) -> (String, Pin) {
    let url = format!("https://registry.npmjs.org/sherpa-onnx-{plat}/-/sherpa-onnx-{plat}-2.0.1.tgz");
    (plat.to_string(), Pin { url, size: None, sha256: None, sha512: Some(sha512.to_string()) })
}

fn github(file: &str, size: u64, sha256: &str) -> Pin {
    Pin { url: format!("{GH}{file}"), size: Some(size), sha256: Some(sha256.to_string()), sha512: None }
}

/// A model: its archive on GitHub (unpinned: its files are), the same files on Hugging Face, and their pins.
fn parakeet(v: &str, label: &str, size: u64, files: [(&str, u64, &str); 4]) -> Model {
    let dir = format!("sherpa-onnx-nemo-parakeet-tdt-0.6b-{v}-int8");
    Model {
        id: format!("parakeet-{v}"),
        label: label.into(),
        size,
        pin: Pin { url: format!("{GH}{dir}.tar.bz2"), size: None, sha256: None, sha512: None },
        mirror: Some(format!("https://huggingface.co/csukuangfj/{dir}/resolve/main/")),
        files: files.iter().map(|(n, s, h)| (n.to_string(), *s, h.to_string())).collect(),
        dir,
    }
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
                parakeet("v2", "Parakeet TDT 0.6B v2 (English)", 482440461, [
                    ("encoder.int8.onnx", 652282309, "92a10bd9c23936ea0566c29f963bc57c2a5df4261b6a8921c39abea66557ca8b"),
                    ("decoder.int8.onnx", 7257753, "4d097bae3bac5c96018d5293ba39f1b657296102b142affda40378f38b020f41"),
                    ("joiner.int8.onnx", 1739080, "77a29f3e5ac7d37074e0af465391b80f314015d5b97f0f90c6c7767f1d56f428"),
                    ("tokens.txt", 9384, "ec182b70dd42113aff6c5372c75cac58c952443eb22322f57bbd7f53977d497d"),
                ]),
                parakeet("v3", "Parakeet TDT 0.6B v3 (25 languages)", 487211443, [
                    ("encoder.int8.onnx", 652282294, "1afbe69c1ffdabee94755f42e4daf1d93549131a3da11356bda84fcef0f2c479"),
                    ("decoder.int8.onnx", 11845274, "0222a86147299d4dee90edf95b6d13827edb42ad8b7c9cf0b2582f030937c0b5"),
                    ("joiner.int8.onnx", 6355277, "cbd02f3696e9944970111fb439d9c3fd365a80374565f13f2b9c7007d253fc98"),
                    ("tokens.txt", 93939, "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d"),
                ]),
            ],
            vad: github("silero_vad.onnx", 643854, "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"),
            vad_bundled: std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("silero_vad.onnx"))),
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

/// Model M is in DIR, every pinned file at its pinned size (quick: no hashing; the install hashed them).
fn installed(dir: &Path, m: &Model) -> bool {
    let d = dir.join(&m.dir);
    m.files.iter().all(|(n, size, _)| fs::metadata(d.join(n)).is_ok_and(|md| md.is_file() && md.len() == *size))
}

/// The VAD in DIR is the pinned file (643 kB: always hashed).
fn vad_ok(dir: &Path, pins: &Pins) -> bool {
    file_ok(&vad_path(dir), pins.vad.size, pins.vad.sha256.as_deref())
}

fn file_ok(p: &Path, size: Option<u64>, sha: Option<&str>) -> bool {
    let Ok(md) = fs::metadata(p) else { return false };
    md.is_file() && md.len() > 0 && size.is_none_or(|s| s == md.len()) && sha.is_none_or(|h| sha256_of(p).is_ok_and(|x| x == h))
}

/// What the unpacked runtime holds, written by the install beside its files: `<size> <sha256> <name>` a line.
const MANIFEST: &str = ".files";

/// The runtime in DIR is whole: every file its manifest names at its size (and sha256 when FULL). A runtime from 0.14 has
/// no manifest: its C library and every file beside it must not be empty (a crash before the data reached the disk
/// leaves the names and no bytes: 2026-11-09).
fn runtime_ok(dir: &Path, full: bool) -> bool {
    let rt = runtime_dir(dir);
    if !fs::metadata(rt.join(c_library())).is_ok_and(|m| m.is_file() && m.len() > 0) {
        return false;
    }
    let Ok(list) = fs::read_to_string(rt.join(MANIFEST)) else {
        let Ok(rd) = fs::read_dir(&rt) else { return false };
        return rd.flatten().all(|e| e.metadata().is_ok_and(|m| !m.is_file() || m.len() > 0));
    };
    let mut n = 0;
    for l in list.lines().filter(|l| !l.is_empty()) {
        let mut it = l.splitn(3, ' ');
        let (Some(size), Some(sha), Some(name)) = (it.next().and_then(|v| v.parse::<u64>().ok()), it.next(), it.next()) else { return false };
        let p = rt.join(name);
        let whole = if size == 0 { p.is_file() } else { file_ok(&p, Some(size), full.then_some(sha)) };
        if !whole {
            return false;
        }
        n += 1;
    }
    n > 0
}

/// Writes the manifest of runtime folder RT, every file in it hashed.
fn write_manifest(rt: &Path) -> Result<(), String> {
    let mut out = String::new();
    let mut names: Vec<String> = vec![];
    walk_files(rt, rt, &mut names);
    names.sort();
    for n in names.iter().filter(|n| *n != MANIFEST) {
        let p = rt.join(n);
        let len = fs::metadata(&p).map_err(|e| e.to_string())?.len();
        out.push_str(&format!("{len} {} {n}\n", sha256_of(&p).map_err(|e| e.to_string())?));
    }
    write_durably(&rt.join(MANIFEST), out.as_bytes())
}

/// The files under D, as paths from ROOT with `/`.
fn walk_files(root: &Path, d: &Path, out: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => walk_files(root, &p, out),
            Ok(t) if t.is_file() => {
                if let Ok(r) = p.strip_prefix(root) {
                    out.push(r.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
                }
            }
            _ => {}
        }
    }
}

/// Every file under D on the disk, not only in the page cache: a rename is kept by a crash, data not yet written is not.
fn sync_tree(d: &Path) -> Result<(), String> {
    let mut names = vec![];
    walk_files(d, d, &mut names);
    for n in names {
        sync_file(&d.join(n))?;
    }
    sync_dir(d);
    Ok(())
}

fn sync_file(p: &Path) -> Result<(), String> {
    fs::OpenOptions::new().write(true).open(p).and_then(|f| f.sync_all()).map_err(|e| format!("{}: {e}", p.display()))
}

/// A folder's entries on the disk (where a platform can: Windows has no handle on a folder for this).
fn sync_dir(d: &Path) {
    #[cfg(unix)]
    let _ = fs::File::open(d).and_then(|f| f.sync_all());
    #[cfg(not(unix))]
    let _ = d;
}

fn write_durably(p: &Path, b: &[u8]) -> Result<(), String> {
    let tmp = p.with_extension("tmp");
    let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
    f.write_all(b).and_then(|_| f.sync_all()).map_err(|e| e.to_string())?;
    drop(f);
    fs::rename(&tmp, p).map_err(|e| e.to_string())?;
    if let Some(d) = p.parent() {
        sync_dir(d);
    }
    Ok(())
}

/// What is installed under DIR.
pub fn status(dir: &Path) -> Value {
    status_with(dir, &Pins::pinned())
}

/// `status` against PINS (tests).
pub fn status_with(dir: &Path, pins: &Pins) -> Value {
    let (rt, vad) = (runtime_ok(dir, false), vad_ok(dir, pins));
    // a ready mark over files that are not whole is dropped: the hub reads the mark to pick the local engine
    for m in &pins.models {
        if !(rt && vad && installed(dir, m)) {
            rm(&dir.join(format!("ready-{}", m.id)));
        }
    }
    let models: Vec<Value> = pins.models.iter().map(|m| json!({"id": m.id, "label": m.label, "size": m.size, "installed": installed(dir, m)})).collect();
    json!({
        "platform": plat(),
        "supported": pins.runtime().is_some(),
        "runtime": rt,
        "vad": vad,
        "vadBundled": bundled_ok(pins), // a release's copy beside the helper, which an install takes instead of a download
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
    let (rt, vad, model) = (runtime_dir(dir).join(c_library()).exists(), vad_path(dir).exists(), dir.join(&m.dir).join("tokens.txt").exists());
    if !rt || !vad || !model {
        return Some("the local model is not downloaded (Settings → Voice)".into());
    }
    if !runtime_ok(dir, false) || !vad_ok(dir, &pins) || !installed(dir, m) {
        rm(&dir.join(format!("ready-{id}")));
        return Some("the local model's files are incomplete or damaged: download it again (Settings → Voice)".into());
    }
    None
}

// ---- install

/// what, done, total, phase
pub type Progress<'a> = &'a mut dyn FnMut(&str, u64, u64, &str);

/// A download, tried again after a server's 5xx or a dropped connection (GitHub's release files answer 504 now and then);
/// a 4xx or a checksum that does not match is final.
fn fetch_to(pin: &Pin, path: &Path, what: &str, progress: Progress) -> Result<(), String> {
    let mut wait = std::env::var("BACKPLANE_VOICE_RETRY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(RETRY_WAIT_MS); // tests wait less
    for _ in 1..FETCH_TRIES {
        match fetch_once(pin, path, what, progress) {
            Err((e, true)) => { eprintln!("{e}: trying again in {wait} ms"); std::thread::sleep(std::time::Duration::from_millis(wait)); wait *= 3 }
            r => return r.map_err(|e| e.0),
        }
    }
    fetch_once(pin, path, what, progress).map_err(|e| e.0)
}

const FETCH_TRIES: u32 = 4;
const RETRY_WAIT_MS: u64 = 2000;

fn fetch_once(pin: &Pin, path: &Path, what: &str, progress: Progress) -> Result<(), (String, bool)> {
    progress(what, 0, pin.size.unwrap_or(0), "download");
    let res = match net::agent(None).get(&pin.url).call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(c)) => return Err((format!("{what}: HTTP {c}"), c >= 500)),
        Err(e) => return Err((net::error_msg(&e), true)),
    };
    if !res.status().is_success() {
        let c = res.status().as_u16();
        return Err((format!("{what}: HTTP {c}"), c >= 500));
    }
    let fin = |e: String| (e, false);
    let drop = |e: std::io::Error| (format!("{what}: {e}"), true);
    let length = res.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
    let total = pin.size.or(length).unwrap_or(0);
    // a model is far over ureq's default limit on a body
    let mut body = res.into_body().into_with_config().limit(u64::MAX).reader();
    let mut h256 = Sha256::new();
    let mut h512 = Sha512::new();
    let mut out = BufWriter::new(fs::File::create(path).map_err(|e| fin(e.to_string()))?);
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last: Option<Instant> = None;
    loop {
        let n = body.read(&mut buf).map_err(drop)?;
        if n == 0 {
            break;
        }
        h256.update(&buf[..n]);
        h512.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| fin(e.to_string()))?;
        done += n as u64;
        if last.is_none_or(|t| t.elapsed().as_millis() > 250) {
            last = Some(Instant::now());
            progress(what, done, total, "download");
        }
    }
    out.into_inner().map_err(|e| fin(e.to_string()))?.sync_all().map_err(|e| fin(e.to_string()))?;
    progress(what, done, total, "download");
    if pin.size.is_some_and(|s| s != done) {
        return Err(fin(format!("{what}: {done} bytes, expected {}", pin.size.unwrap())));
    }
    let hex: String = h256.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if pin.sha256.as_ref().is_some_and(|h| *h != hex) {
        return Err(fin(format!("{what}: checksum mismatch")));
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(h512.finalize());
    if pin.sha512.as_ref().is_some_and(|h| *h != b64) {
        return Err(fin(format!("{what}: checksum mismatch")));
    }
    Ok(())
}

/// Unpacks in-process; the tar crate refuses paths that climb out of INTO.
fn untar(archive: &Path, into: &Path, bz2: bool) -> Result<(), String> {
    let tar_err = |e: std::io::Error| format!("tar: {e}");
    fs::create_dir_all(into).map_err(tar_err)?;
    let f = std::io::BufReader::new(fs::File::open(archive).map_err(tar_err)?);
    let r: Box<dyn Read> = if bz2 { Box::new(bzip2::read::MultiBzDecoder::new(f)) } else { Box::new(flate2::read::MultiGzDecoder::new(f)) };
    tar::Archive::new(r).unpack(into).map_err(tar_err)?;
    sync_tree(into)
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
    if !vad_ok(dir, pins) {
        let p = stage.join("silero_vad.onnx");
        if !bundled_vad(pins, &p) {
            fetch_to(&pins.vad, &p, "voice activity model", progress)?;
        }
        sync_file(&p)?;
        fs::rename(&p, vad_path(dir)).map_err(io)?;
        sync_dir(dir);
    }
    if !runtime_ok(dir, true) {
        let pin = pins.runtime().ok_or("no runtime for this platform")?;
        let tgz = stage.join("runtime.tgz");
        fetch_to(pin, &tgz, "speech runtime", progress)?;
        progress("speech runtime", 0, 0, "unpack");
        untar(&tgz, &stage.join("rt"), false)?;
        let pkg = stage.join("rt").join("package");
        if !fs::metadata(pkg.join(c_library())).is_ok_and(|m| m.len() > 0) {
            return Err(format!("speech runtime: {} is missing or empty in the package", c_library()));
        }
        write_manifest(&pkg)?;
        rm(&runtime_dir(dir));
        fs::rename(&pkg, runtime_dir(dir)).map_err(io)?;
        sync_dir(dir);
    }
    if check_files(&dir.join(&m.dir), m).is_err() {
        let got = match from_archive(stage, m, progress) {
            Ok(d) => d,
            Err(e) => match &m.mirror {
                Some(base) => from_mirror(base, stage, m, progress).map_err(|e2| format!("{e}; second source: {e2}"))?,
                None => return Err(e),
            },
        };
        sync_tree(&got)?;
        rm(&dir.join(&m.dir));
        fs::rename(&got, dir.join(&m.dir)).map_err(io)?;
        sync_dir(dir);
    }
    // all of it again where it now is, before the ready mark says so
    if !vad_ok(dir, pins) {
        return Err("voice activity model: the installed file does not match its pin".into());
    }
    if !runtime_ok(dir, true) {
        return Err("speech runtime: the installed files do not match what was unpacked".into());
    }
    check_files(&dir.join(&m.dir), m)
}

/// The VAD shipped beside the helper, copied to P, when it is there and matches the pin (else it is downloaded).
fn bundled_vad(pins: &Pins, p: &Path) -> bool {
    let Some(b) = pins.vad_bundled.as_deref().filter(|b| b.exists()) else { return false };
    if !bundled_ok(pins) {
        eprintln!("{} does not match its pin: downloading the voice activity model", b.display());
        return false;
    }
    fs::copy(b, p).is_ok()
}

/// Whether the VAD beside the helper is there and is the pinned file.
fn bundled_ok(pins: &Pins) -> bool {
    let Some(b) = pins.vad_bundled.as_deref() else { return false };
    let Ok(md) = fs::metadata(b) else { return false };
    pins.vad.size.is_none_or(|s| s == md.len()) && pins.vad.sha256.as_ref().is_none_or(|h| sha256_of(b).ok().as_ref() == Some(h))
}

/// The model's archive, unpacked in the staging folder, its files checked: the folder they are in.
fn from_archive(stage: &Path, m: &Model, progress: Progress) -> Result<PathBuf, String> {
    let tb = stage.join("model.tar.bz2");
    fetch_to(&m.pin, &tb, &m.label, progress)?;
    progress(&m.label, 0, 0, "unpack");
    let un = untar(&tb, &stage.join("m"), true);
    rm(&tb);
    un?;
    let d = stage.join("m").join(&m.dir);
    rm(&d.join("test_wavs"));
    check_files(&d, m)?;
    Ok(d)
}

/// The model's files one by one from its second source, each checked as it comes.
fn from_mirror(base: &str, stage: &Path, m: &Model, progress: Progress) -> Result<PathBuf, String> {
    let d = stage.join("m2").join(&m.dir);
    rm(&d);
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let total: u64 = m.files.iter().map(|f| f.1).sum();
    let mut off = 0u64;
    for (name, size, sha) in &m.files {
        let pin = Pin { url: format!("{base}{name}"), size: Some(*size), sha256: Some(sha.clone()), sha512: None };
        let mut each = |what: &str, done: u64, _: u64, phase: &str| progress(what, off + done, total, phase);
        fetch_to(&pin, &d.join(name), &m.label, &mut each).map_err(|e| format!("{e} ({name})"))?;
        off += size;
    }
    Ok(d)
}

/// Every pinned file of M in D, with its size and sha256.
fn check_files(d: &Path, m: &Model) -> Result<(), String> {
    for (name, size, sha) in &m.files {
        let p = d.join(name);
        let Ok(md) = fs::metadata(&p) else { return Err(format!("{}: {name} is not in the archive", m.label)) };
        if md.len() != *size || sha256_of(&p).map_err(|e| e.to_string())? != *sha {
            return Err(format!("{}: {name} does not match its pin", m.label));
        }
    }
    Ok(())
}

fn sha256_of(p: &Path) -> std::io::Result<String> {
    let mut f = fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Fetch whatever is missing for model ID into DIR; each piece lands in a
/// staging folder and is moved into place only once its checksum matched.
pub fn install(dir: &Path, id: &str, pins: &Pins, progress: Progress) -> Result<(), String> {
    if pins.runtime().is_none() {
        return Err(format!("local dictation is not available on {}", plat()));
    }
    let Some(m) = pins.model(id) else { return Err(format!("unknown model {id}")) };
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let ready_mark = dir.join(format!("ready-{id}"));
    rm(&ready_mark); // written again only once every piece checks
    clear_stale_stages(dir);
    let stage = dir.join(format!(".stage-{}", std::process::id()));
    rm(&stage);
    fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    let r = pieces(dir, &stage, pins, m, progress);
    rm(&stage);
    r?;
    // the hub reads this to know the model is usable
    write_durably(&ready_mark, b"ok")
}

/// Staging folders of installs that are not running any more (a crash, a kill) go.
fn clear_stale_stages(dir: &Path) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(pid) = name.strip_prefix(".stage-").and_then(|p| p.parse::<u32>().ok()) else { continue };
        if pid != std::process::id() && !alive(pid) {
            rm(&e.path());
        }
    }
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    // signal 0 checks without sending; EPERM is a process that is there
    (unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }) || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn alive(_: u32) -> bool {
    true // left for the person to clear: no cheap check here
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
        assert_eq!(p.model("parakeet-v2").unwrap().size, 482440461);
        assert!(p.models.iter().all(|m| m.files.len() == 4 && m.mirror.as_deref().is_some_and(|u| u.contains(&m.dir)) && m.pin.sha256.is_none()), "every model pins its files and has a second source");
        assert_eq!(p.model("parakeet-v3").unwrap().dir, "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8");
        assert_eq!(p.vad.sha256.as_deref(), Some("9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"));
        assert_eq!(p.vad.url, "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx");
        assert!(p.vad_bundled.as_deref().is_some_and(|b| b.ends_with("silero_vad.onnx")), "beside the helper");
    }

    #[test]
    fn the_vad_a_release_ships_matches_the_pin() {
        let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("vad/silero_vad.onnx");
        let p = Pins::pinned();
        assert_eq!(fs::metadata(&f).unwrap().len(), p.vad.size.unwrap());
        assert_eq!(sha256_of(&f).unwrap(), p.vad.sha256.unwrap());
        assert!(Path::new(env!("CARGO_MANIFEST_DIR")).join("vad/silero_vad.LICENSE").exists(), "its licence goes with it");
        assert!(p.runtimes[0].1.url.ends_with("sherpa-onnx-linux-x64/-/sherpa-onnx-linux-x64-2.0.1.tgz"));
    }

    /// A folder as 0.14 left it: the addon and the C library side by side.
    fn install_of_0_14(dir: &Path) {
        let rt = runtime_dir(dir);
        fs::create_dir_all(&rt).unwrap();
        for f in ["sherpa-onnx.node", c_library(), onnx_library()] {
            fs::write(rt.join(f), "x").unwrap();
        }
        fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("vad/silero_vad.onnx"), vad_path(dir)).unwrap();
        model_files(dir, "parakeet-v2");
        fs::write(dir.join("ready-parakeet-v2"), "ok").unwrap();
    }

    /// Model ID's files at their pinned sizes (sparse: nothing written).
    fn model_files(dir: &Path, id: &str) {
        let p = Pins::pinned();
        let m = p.model(id).unwrap();
        let d = dir.join(&m.dir);
        fs::create_dir_all(&d).unwrap();
        for (n, size, _) in &m.files {
            fs::File::create(d.join(n)).unwrap().set_len(*size).unwrap();
        }
    }

    /// What a crash during an install left (2026-11-09): every name in place, no bytes, and a later install's ready mark.
    #[test]
    fn files_a_crash_left_empty_are_not_ready_and_lose_the_mark() {
        let t = tempfile::tempdir().unwrap();
        install_of_0_14(t.path());
        for f in fs::read_dir(runtime_dir(t.path())).unwrap().flatten() {
            fs::File::create(f.path()).unwrap();
        }
        fs::File::create(vad_path(t.path())).unwrap();
        assert_eq!(ready(t.path(), "parakeet-v2").unwrap(), "the local model's files are incomplete or damaged: download it again (Settings → Voice)");
        assert!(!t.path().join("ready-parakeet-v2").exists(), "the hub picks the local engine by the mark");
        fs::write(t.path().join("ready-parakeet-v2"), "ok").unwrap();
        let s = status(t.path());
        assert_eq!((&s["runtime"], &s["vad"], &s["models"][0]["installed"]), (&json!(false), &json!(false), &json!(true)));
        assert!(!t.path().join("ready-parakeet-v2").exists());
        // a model file cut short is not installed
        let enc = t.path().join("sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8/encoder.int8.onnx");
        fs::File::create(&enc).unwrap().set_len(1000).unwrap();
        assert_eq!(status(t.path())["models"][0]["installed"], json!(false));
    }

    #[test]
    fn an_install_from_0_14_is_ready_and_a_lone_addon_is_not() {
        let t = tempfile::tempdir().unwrap();
        install_of_0_14(t.path());
        assert_eq!(ready(t.path(), "parakeet-v2"), None);
        let s = status(t.path());
        assert_eq!((&s["runtime"], &s["vad"], &s["models"][0]["installed"], &s["models"][1]["installed"]), (&json!(true), &json!(true), &json!(true), &json!(false)));
        assert_eq!(s["models"][0]["size"], json!(482440461));
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
        model_files(t.path(), "parakeet-v3");
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
