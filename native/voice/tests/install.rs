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
const MODEL_FILES: [(&str, &[u8]); 4] = [("encoder.int8.onnx", b"e"), ("decoder.int8.onnx", b"d"), ("joiner.int8.onnx", b"j"), ("tokens.txt", b"t")];

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
    // the same files packed again another way (upstream's repack), one tampered with, and the files one by one (the second source)
    let repack = tbz(&[("model-dir/README.md", b"new notes".as_slice()), ("model-dir/tokens.txt", b"t"), ("model-dir/joiner.int8.onnx", b"j"), ("model-dir/decoder.int8.onnx", b"d"), ("model-dir/encoder.int8.onnx", b"e")]);
    let tampered = tbz(&[("model-dir/encoder.int8.onnx", b"E".as_slice()), ("model-dir/decoder.int8.onnx", b"d"), ("model-dir/joiner.int8.onnx", b"j"), ("model-dir/tokens.txt", b"t")]);
    let vad = b"silero".to_vec();
    let mut files = HashMap::from([("/rt.tgz".to_string(), rt.clone()), ("/m.tar.bz2".to_string(), model.clone()), ("/vad.onnx".to_string(), vad.clone()), ("/repack.tar.bz2".to_string(), repack), ("/tampered.tar.bz2".to_string(), tampered)]);
    for (n, b) in MODEL_FILES { files.insert(format!("/hf/{n}"), b.to_vec()); files.insert(format!("/hf-bad/{n}"), if n == "tokens.txt" { b"T".to_vec() } else { b.to_vec() }); }
    let f = Files::start(files);
    let pins = Pins {
        runtimes: vec![(plat(), Pin { url: format!("{}/rt.tgz", f.base), size: None, sha256: None, sha512: Some(sha512(&rt)) })],
        models: vec![Model {
            id: "m".into(),
            label: "The Model".into(),
            dir: DIR.into(),
            size: model.len() as u64,
            pin: Pin { url: format!("{}/redirect/m.tar.bz2", f.base), size: Some(model.len() as u64), sha256: Some(sha256(&model)), sha512: None },
            mirror: None,
            files: MODEL_FILES.iter().map(|(n, b)| (n.to_string(), b.len() as u64, sha256(b))).collect(),
        }],
        vad: Pin { url: format!("{}/vad.onnx", f.base), size: Some(vad.len() as u64), sha256: Some(sha256(&vad)), sha512: None },
        vad_bundled: None,
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

/// The archive as upstream may repack it: no archive checksum to match, the pinned files decide.
fn unpinned(r: &mut Rig, archive: &str) {
    r.pins.models[0].pin = Pin { url: format!("{}/redirect/{archive}", r.files.base), size: None, sha256: None, sha512: None };
}

#[test]
fn a_repacked_archive_with_the_same_files_installs() {
    let mut r = rig();
    unpinned(&mut r, "repack.tar.bz2");
    let t = tempfile::tempdir().unwrap();
    run(t.path(), &r.pins).0.unwrap();
    assert_eq!(std::fs::read(t.path().join(DIR).join("encoder.int8.onnx")).unwrap(), b"e");
    assert_eq!(std::fs::read_to_string(t.path().join("ready-m")).unwrap(), "ok");
    assert!(no_stage(t.path()));
}

#[test]
fn a_tampered_file_is_refused_and_nothing_is_left_in_place() {
    let mut r = rig();
    unpinned(&mut r, "tampered.tar.bz2");
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &r.pins).0.unwrap_err(), "The Model: encoder.int8.onnx does not match its pin");
    assert!(!t.path().join(DIR).exists());
    assert!(!t.path().join("ready-m").exists());
    assert!(no_stage(t.path()));
    // a file missing from the archive is refused the same way
    let mut r = rig();
    r.pins.models[0].files.push(("extra.onnx".into(), 1, sha256(b"x")));
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &r.pins).0.unwrap_err(), "The Model: extra.onnx is not in the archive");
}

#[test]
fn the_second_source_is_used_when_the_archive_fails_or_does_not_match() {
    for archive in ["gone.tar.bz2", "tampered.tar.bz2"] {
        let mut r = rig();
        unpinned(&mut r, archive);
        r.pins.models[0].mirror = Some(format!("{}/redirect/hf/", r.files.base));
        let t = tempfile::tempdir().unwrap();
        let (ok, seen) = run(t.path(), &r.pins);
        ok.unwrap_or_else(|e| panic!("{archive}: {e}"));
        for (n, b) in MODEL_FILES { assert_eq!(std::fs::read(t.path().join(DIR).join(n)).unwrap(), b, "{archive}: {n}"); }
        assert_eq!(std::fs::read_to_string(t.path().join("ready-m")).unwrap(), "ok");
        assert!(no_stage(t.path()));
        // its progress is one download of all the files
        let last = seen.iter().filter(|s| s.0 == "The Model" && s.3 == "download").next_back().unwrap();
        assert_eq!((last.1, last.2), (4, 4), "{archive}");
    }
    // both sources wrong: a hard failure that names both, nothing in place
    let mut r = rig();
    unpinned(&mut r, "tampered.tar.bz2");
    r.pins.models[0].mirror = Some(format!("{}/hf-bad/", r.files.base));
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &r.pins).0.unwrap_err(), "The Model: encoder.int8.onnx does not match its pin; second source: The Model: checksum mismatch (tokens.txt)");
    assert!(!t.path().join(DIR).exists());
    assert!(!t.path().join("ready-m").exists());
    assert!(no_stage(t.path()));
}

#[test]
fn a_gateway_timeout_is_tried_again_and_a_lasting_one_is_the_error() {
    std::env::set_var("BACKPLANE_VOICE_RETRY_MS", "5");
    // two 504s, then the file: installed, with no second source
    let mut r = rig();
    r.pins.vad.url = format!("{}/504/2/vad.onnx", r.files.base);
    let t = tempfile::tempdir().unwrap();
    run(t.path(), &r.pins).0.unwrap();
    assert_eq!(std::fs::read(vad_path(t.path())).unwrap(), b"silero");
    // 504s every time: the last answer is the error, after four tries
    let mut r = rig();
    r.pins.vad.url = format!("{}/504/99/vad.onnx", r.files.base);
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &r.pins).0.unwrap_err(), "voice activity model: HTTP 504");
    assert_eq!(r.files.hits.load(Ordering::SeqCst), 4);
    // a 404 is not tried again
    let mut r = rig();
    r.pins.vad.url = format!("{}/gone", r.files.base);
    let t = tempfile::tempdir().unwrap();
    run(t.path(), &r.pins).0.unwrap_err();
    assert_eq!(r.files.hits.load(Ordering::SeqCst), 1);
}

#[test]
fn the_vad_beside_the_helper_is_used_and_one_that_does_not_match_is_not() {
    // a release: the shipped copy, and no VAD to download
    let mut r = rig();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(b.path().join("silero_vad.onnx"), b"silero").unwrap();
    r.pins.vad_bundled = Some(b.path().join("silero_vad.onnx"));
    r.pins.vad.url = format!("{}/gone", r.files.base);
    let t = tempfile::tempdir().unwrap();
    let (ok, seen) = run(t.path(), &r.pins);
    ok.unwrap();
    assert_eq!(std::fs::read(vad_path(t.path())).unwrap(), b"silero");
    assert!(!seen.iter().any(|s| s.0 == "voice activity model"), "nothing downloaded for it: {seen:?}");
    assert!(b.path().join("silero_vad.onnx").exists(), "the shipped copy stays");
    assert_eq!(local::status_with(&tempfile::tempdir().unwrap().path(), &r.pins)["vadBundled"], serde_json::json!(true), "an empty folder's status says a copy is beside the helper");
    // a copy that is not the pinned file: downloaded instead
    let mut r = rig();
    std::fs::write(b.path().join("silero_vad.onnx"), b"silerx").unwrap();
    r.pins.vad_bundled = Some(b.path().join("silero_vad.onnx"));
    assert_eq!(local::status_with(&tempfile::tempdir().unwrap().path(), &r.pins)["vadBundled"], serde_json::json!(false));
    let t = tempfile::tempdir().unwrap();
    run(t.path(), &r.pins).0.unwrap();
    assert_eq!(std::fs::read(vad_path(t.path())).unwrap(), b"silero");
    // a build without one (dev): downloaded
    let mut r = rig();
    r.pins.vad_bundled = Some(b.path().join("none.onnx"));
    let t = tempfile::tempdir().unwrap();
    run(t.path(), &r.pins).0.unwrap();
    assert_eq!(std::fs::read(vad_path(t.path())).unwrap(), b"silero");
}

/// Every file an install leaves is there, not empty, and the hash its pin or the runtime's manifest gives.
fn all_hash_checked(dir: &Path, r: &Rig) {
    let vad = std::fs::read(vad_path(dir)).unwrap();
    assert!(!vad.is_empty() && Some(sha256(&vad)) == r.pins.vad.sha256);
    for (n, size, sha) in &r.pins.models[0].files {
        let b = std::fs::read(dir.join(DIR).join(n)).unwrap();
        assert!(!b.is_empty() && b.len() as u64 == *size && sha256(&b) == *sha, "{n}");
    }
    let rt = runtime_dir(dir);
    let list = std::fs::read_to_string(rt.join(".files")).unwrap();
    let names: Vec<&str> = list.lines().map(|l| l.splitn(3, ' ').nth(2).unwrap()).collect();
    let mut want = vec![c_library(), onnx_library(), "sherpa-onnx.node"];
    want.sort();
    assert_eq!(names, want);
    for l in list.lines() {
        let mut it = l.splitn(3, ' ');
        let (size, sha, name) = (it.next().unwrap().parse::<u64>().unwrap(), it.next().unwrap(), it.next().unwrap());
        let b = std::fs::read(rt.join(name)).unwrap();
        assert!(size > 0 && b.len() as u64 == size && sha256(&b) == sha, "{name}");
    }
    assert_eq!(std::fs::read_to_string(dir.join("ready-m")).unwrap(), "ok");
}

/// What a crash in the middle of an install left on 2026-11-09: the names in place and no bytes (the renames reached the
/// disk, the data did not), a dead install's staging folder, and a later install's ready mark. The next install mends it.
#[test]
fn an_install_into_a_fresh_home_is_whole_and_one_a_crash_emptied_is_mended() {
    let r = rig();
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("voice/local");
    run(&dir, &r.pins).0.unwrap();
    all_hash_checked(&dir, &r);

    for f in std::fs::read_dir(runtime_dir(&dir)).unwrap().flatten() {
        std::fs::File::create(f.path()).unwrap();
    }
    std::fs::File::create(vad_path(&dir)).unwrap();
    let dead = dir.join(".stage-999999999");
    std::fs::create_dir_all(dead.join("rt")).unwrap();
    std::fs::File::create(dead.join("runtime.tgz")).unwrap();
    let s = local::status_with(&dir, &r.pins);
    assert_eq!((&s["runtime"], &s["vad"]), (&serde_json::json!(false), &serde_json::json!(false)));
    assert!(!dir.join("ready-m").exists(), "status drops a mark over broken files");

    let before = r.files.hits.load(Ordering::SeqCst);
    run(&dir, &r.pins).0.unwrap();
    assert_eq!(r.files.hits.load(Ordering::SeqCst), before + 2, "the runtime and the VAD again, not the model");
    all_hash_checked(&dir, &r);
    assert!(no_stage(&dir), "a dead install's staging folder goes");
}

#[test]
fn a_runtime_package_with_an_empty_library_is_refused() {
    let mut r = rig();
    let (c, o) = (format!("package/{}", c_library()), format!("package/{}", onnx_library()));
    let rt = tgz(&[(c.as_str(), b"".as_slice()), (o.as_str(), b"onnx")]);
    let only = Files::start(HashMap::from([("/empty-rt.tgz".to_string(), rt.clone())]));
    r.pins.runtimes[0].1 = Pin { url: format!("{}/empty-rt.tgz", only.base), size: None, sha256: None, sha512: Some(sha512(&rt)) };
    let t = tempfile::tempdir().unwrap();
    let e = run(t.path(), &r.pins).0.unwrap_err();
    assert!(e.contains("missing or empty"), "{e}");
    assert!(!t.path().join("ready-m").exists() && !runtime_dir(t.path()).exists() && no_stage(t.path()));
}
