//! Download, check and unpack, against a local file server: never the real
//! pins, which the gated end-to-end run exercises.

mod common;
use backplane_voice::local::{self, c_library, install, onnx_library, plat, runtime_dir, vad_path, Model, Pin, Pins};
use base64::Engine as _;
use common::Files;
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::Ordering;

fn tar_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut b = tar::Builder::new(vec![]);
    for (path, data) in files {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, path, *data).unwrap();
    }
    b.into_inner().unwrap()
}

fn tgz(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(vec![], flate2::Compression::fast());
    e.write_all(&tar_of(files)).unwrap();
    e.finish().unwrap()
}

fn tbz(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut e = bzip2::write::BzEncoder::new(vec![], bzip2::Compression::fast());
    e.write_all(&tar_of(files)).unwrap();
    e.finish().unwrap()
}

fn sha256(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn sha512(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha512::digest(b))
}

const DIR: &str = "model-dir";

struct Rig {
    files: Files,
    pins: Pins,
}

/// A runtime, a model with test_wavs, a VAD; the model comes by a redirect
/// as GitHub's do.
fn rig() -> Rig {
    let (c, o) = (format!("package/{}", c_library()), format!("package/{}", onnx_library()));
    let rt = tgz(&[(c.as_str(), b"lib".as_slice()), (o.as_str(), b"onnx"), ("package/sherpa-onnx.node", b"addon")]);
    let model = tbz(&[
        ("model-dir/encoder.int8.onnx", b"e".as_slice()),
        ("model-dir/decoder.int8.onnx", b"d"),
        ("model-dir/joiner.int8.onnx", b"j"),
        ("model-dir/tokens.txt", b"t"),
        ("model-dir/test_wavs/0.wav", b"w"),
    ]);
    let vad = b"silero".to_vec();
    let files = HashMap::from([("/rt.tgz".to_string(), rt.clone()), ("/m.tar.bz2".to_string(), model.clone()), ("/vad.onnx".to_string(), vad.clone())]);
    let f = Files::start(files);
    let pins = Pins {
        runtimes: vec![(plat(), Pin { url: format!("{}/rt.tgz", f.base), size: None, sha256: None, sha512: Some(sha512(&rt)) })],
        models: vec![Model {
            id: "m".into(),
            label: "The Model".into(),
            dir: DIR.into(),
            pin: Pin { url: format!("{}/redirect/m.tar.bz2", f.base), size: Some(model.len() as u64), sha256: Some(sha256(&model)), sha512: None },
        }],
        vad: Pin { url: format!("{}/vad.onnx", f.base), size: Some(vad.len() as u64), sha256: Some(sha256(&vad)), sha512: None },
    };
    Rig { files: f, pins }
}

fn run(dir: &Path, pins: &Pins) -> (Result<(), String>, Vec<(String, u64, u64, String)>) {
    let mut seen = vec![];
    let mut p = |what: &str, done: u64, total: u64, phase: &str| seen.push((what.to_string(), done, total, phase.to_string()));
    let r = install(dir, "m", pins, &mut p);
    (r, seen)
}

fn no_stage(dir: &Path) -> bool {
    !std::fs::read_dir(dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with(".stage"))
}

#[test]
fn installs_every_piece_then_writes_ready() {
    let r = rig();
    let t = tempfile::tempdir().unwrap();
    let (ok, seen) = run(t.path(), &r.pins);
    ok.unwrap();
    assert!(runtime_dir(t.path()).join(c_library()).exists());
    assert!(runtime_dir(t.path()).join(onnx_library()).exists());
    assert_eq!(std::fs::read(vad_path(t.path())).unwrap(), b"silero");
    assert!(t.path().join(DIR).join("encoder.int8.onnx").exists());
    assert!(!t.path().join(DIR).join("test_wavs").exists());
    assert_eq!(std::fs::read_to_string(t.path().join("ready-m")).unwrap(), "ok");
    assert!(no_stage(t.path()));
    let whats: Vec<(&str, &str)> = seen.iter().map(|s| (s.0.as_str(), s.3.as_str())).collect();
    assert_eq!(whats[0], ("voice activity model", "download"));
    assert!(whats.contains(&("speech runtime", "download")));
    assert!(whats.contains(&("speech runtime", "unpack")));
    assert!(whats.contains(&("The Model", "download")));
    assert_eq!(*whats.last().unwrap(), ("The Model", "unpack"));
    // a download's last word is all of it
    let last = seen.iter().filter(|s| s.0 == "voice activity model").next_back().unwrap();
    assert_eq!((last.1, last.2), (6, 6));
}

#[test]
fn what_is_there_is_not_fetched_again() {
    let r = rig();
    let t = tempfile::tempdir().unwrap();
    run(t.path(), &r.pins).0.unwrap();
    let before = r.files.hits.load(Ordering::SeqCst);
    assert!(before >= 3);
    run(t.path(), &r.pins).0.unwrap();
    assert_eq!(r.files.hits.load(Ordering::SeqCst), before);
}

#[test]
fn a_wrong_checksum_leaves_nothing_in_place() {
    let mut r = rig();
    r.pins.models[0].pin.sha256 = Some("0".repeat(64));
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &r.pins).0.unwrap_err(), "The Model: checksum mismatch");
    assert!(!t.path().join(DIR).exists());
    assert!(!t.path().join("ready-m").exists());
    assert!(no_stage(t.path()));

    let mut r = rig();
    r.pins.runtimes[0].1.sha512 = Some("AAAA".into());
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &r.pins).0.unwrap_err(), "speech runtime: checksum mismatch");
    assert!(!runtime_dir(t.path()).exists());
}

#[test]
fn a_wrong_size_and_a_missing_file() {
    let mut r = rig();
    r.pins.vad.size = Some(5);
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &r.pins).0.unwrap_err(), "voice activity model: 6 bytes, expected 5");
    assert!(!vad_path(t.path()).exists());

    let mut r = rig();
    r.pins.vad.url = format!("{}/gone", r.files.base);
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &r.pins).0.unwrap_err(), "voice activity model: HTTP 404");
}

#[test]
fn an_unknown_model_or_platform() {
    let r = rig();
    let t = tempfile::tempdir().unwrap();
    let mut p = |_: &str, _: u64, _: u64, _: &str| {};
    assert_eq!(install(t.path(), "nope", &r.pins, &mut p).unwrap_err(), "unknown model nope");
    let mut other = r.pins.clone();
    other.runtimes[0].0 = "plan9-mips".into();
    assert_eq!(install(t.path(), "m", &other, &mut p).unwrap_err(), format!("local dictation is not available on {}", plat()));
}

#[test]
fn a_broken_archive_is_a_tar_error() {
    let mut r = rig();
    let bad = b"not a tarball".to_vec();
    r.pins.runtimes[0].1 = Pin { url: format!("{}/vad.onnx", r.files.base), size: None, sha256: Some(sha256(b"silero")), sha512: None };
    let t = tempfile::tempdir().unwrap();
    let e = run(t.path(), &r.pins).0.unwrap_err();
    assert!(e.starts_with("tar: "), "{e}");
    assert!(no_stage(t.path()));
    drop(bad);
}

#[test]
fn a_status_of_the_real_pins_is_unchanged_by_an_empty_folder() {
    let t = tempfile::tempdir().unwrap();
    let s = local::status(t.path());
    assert_eq!(s["models"].as_array().unwrap().len(), 2);
    assert_eq!(s["supported"], serde_json::json!(true));
}
