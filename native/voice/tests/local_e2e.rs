//! The real engine on recorded speech. Needs a downloaded folder, so it runs
//! only when both are set; `cargo test --offline` alone skips it:
//!   BACKPLANE_VOICE_LOCAL_DIR  a folder `local-install` filled
//!   BACKPLANE_VOICE_WAVS       a folder of 16 kHz mono WAVs with speech (the
//!                              model tarball's test_wavs/)
//! Run with --nocapture to see the words and the timings.

mod common;
use backplane_voice::audio::Resampler;
use backplane_voice::session::Decoder;
use backplane_voice::{local, sherpa};
use common::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn rig() -> Option<(String, PathBuf)> {
    match (std::env::var("BACKPLANE_VOICE_LOCAL_DIR"), std::env::var("BACKPLANE_VOICE_WAVS")) {
        (Ok(d), Ok(w)) => Some((d, PathBuf::from(w))),
        _ => {
            eprintln!("BACKPLANE_VOICE_LOCAL_DIR and BACKPLANE_VOICE_WAVS are not set: skipped");
            None
        }
    }
}

fn wavs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "wav")).collect();
    v.sort();
    assert!(!v.is_empty(), "no WAVs in {}", dir.display());
    v
}

/// The samples of a canonical WAV (the data chunk, wherever it is).
fn pcm_of(p: &Path) -> Vec<u8> {
    let b = std::fs::read(p).unwrap();
    let mut at = 12;
    while at + 8 <= b.len() {
        let n = u32::from_le_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
        if &b[at..at + 4] == b"data" {
            return b[at + 8..(at + 8 + n).min(b.len())].to_vec();
        }
        at += 8 + n + (n & 1);
    }
    panic!("no data chunk in {}", p.display());
}

fn floats(pcm: &[u8]) -> Vec<f32> {
    pcm.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect()
}

fn norm(s: &str) -> Vec<String> {
    s.to_lowercase().split_whitespace().map(|w| w.chars().filter(|c| c.is_alphanumeric() || *c == '\'').collect::<String>()).filter(|w| !w.is_empty()).collect()
}

/// How many of WANT's words are in GOT.
fn overlap(got: &str, want: &str) -> f64 {
    let (g, w) = (norm(got), norm(want));
    w.iter().filter(|x| g.contains(x)).count() as f64 / w.len().max(1) as f64
}

/// Decoding directly: the reference for what the session should get.
fn reference(dir: &str, wav: &Path) -> String {
    let api = sherpa::Api::load(Path::new(dir)).unwrap();
    let mut rec = sherpa::Recognizer::create(&api, Path::new(dir), "parakeet-v2", 4).unwrap();
    let words = rec.decode(&floats(&pcm_of(wav)));
    words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>().join(" ")
}

const NONE: &str = "http://127.0.0.1:9/v1";

#[test]
fn the_runtime_is_the_pinned_version_and_its_json_has_the_fields_words_are_made_from() {
    let Some((dir, w)) = rig() else { return };
    let t0 = Instant::now();
    let api = sherpa::Api::load(Path::new(&dir)).unwrap();
    let rec = sherpa::Recognizer::create(&api, Path::new(&dir), "parakeet-v2", 4).unwrap();
    eprintln!("load: {} ms", t0.elapsed().as_millis());
    let wav = &wavs(&w)[0];
    let s = floats(&pcm_of(wav));
    let t1 = Instant::now();
    let j = rec.decode_json(&s).unwrap();
    eprintln!("decode: {} ms for {:.2} s of audio", t1.elapsed().as_millis(), s.len() as f64 / 16000.0);
    let v: Value = serde_json::from_str(&j).unwrap();
    eprintln!("result keys: {:?}", v.as_object().unwrap().keys().collect::<Vec<_>>());
    eprintln!("raw: {}", j.chars().take(600).collect::<String>());
    let n = v["tokens"].as_array().expect("tokens").len();
    assert!(n > 0);
    assert_eq!(v["timestamps"].as_array().expect("timestamps").len(), n);
    assert_eq!(v["durations"].as_array().expect("durations").len(), n);
    assert_eq!(v["ys_log_probs"].as_array().expect("ys_log_probs").len(), n);
    let words = local::words_of(&v);
    assert!(!words.is_empty());
    // the words rebuild the text, and their timings and probabilities are real
    assert_eq!(norm(&words.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(" ")), norm(v["text"].as_str().unwrap()));
    assert!(words.windows(2).all(|p| p[0].1 <= p[1].1));
    assert!(words.iter().all(|x| x.2 > x.1 && (0..=100).contains(&x.3)));
    assert!(words.iter().any(|x| x.1 > 0) && words.iter().any(|x| x.3 < 100 || x.3 == 100));
    eprintln!("words: {:?}", words.iter().take(12).collect::<Vec<_>>());
}

fn live_run(dir: &str, wav: &Path, extra: &[&str], feed: impl FnOnce(&mut Run)) -> Run {
    let mut args = vec!["live", "--engine", "local", "--local-dir", dir, "--source", "stdin"];
    args.extend_from_slice(extra);
    let mut r = Run::new(&args, NONE, &[]);
    feed(&mut r);
    let code = r.done();
    assert_eq!(code, 0, "{:?}", r.names());
    let _ = wav;
    r
}

fn report(tag: &str, r: &Run) -> String {
    let dones = r.all("done");
    let text = dones.iter().map(|d| d["text"].as_str().unwrap()).collect::<Vec<_>>().join(" ");
    let loaded = r.next("loaded")["ms"].as_i64().unwrap();
    let hyps = r.all("hyp").len();
    let decode: Vec<i64> = dones.iter().map(|d| d["decode_ms"].as_i64().unwrap()).collect();
    let fin: Vec<i64> = dones.iter().map(|d| d["final_ms"].as_i64().unwrap()).collect();
    eprintln!("{tag}: loaded {loaded} ms, {hyps} hyp, decode_ms {decode:?}, final_ms {fin:?}: {text:?}");
    text
}

#[test]
fn live_in_real_time_hyps_settle_into_a_done_with_real_words() {
    let Some((dir, w)) = rig() else { return };
    for wav in wavs(&w).iter().take(2) {
        let want = reference(&dir, wav);
        let pcm = pcm_of(wav);
        // real time, 100 ms at a time, then a second of silence
        let all = [pcm, vec![0u8; 32000]].concat();
        let r = live_run(&dir, wav, &[], |r| {
            let t0 = Instant::now();
            for (i, c) in all.chunks(3200).enumerate() {
                r.write(c);
                let at = Duration::from_millis(100 * (i as u64 + 1));
                if let Some(d) = at.checked_sub(t0.elapsed()) {
                    std::thread::sleep(d);
                }
            }
            r.close_stdin();
        });
        let ev = r.names();
        assert_eq!(ev[0], "ready");
        assert_eq!(ev.last().unwrap(), "end");
        assert!(ev.contains(&"loaded".to_string()) && !ev.contains(&"error".to_string()));
        assert!(!r.all("hyp").is_empty());
        let hyp = &r.all("hyp")[0];
        assert!(hyp["item"].as_str().unwrap().starts_with('l') && hyp["end"].as_i64().unwrap() > 0);
        let word = &hyp["words"][0];
        assert!(word[0].is_string() && word[1].is_i64() && word[2].is_i64() && word[3].is_i64());
        let done = &r.all("done")[0];
        assert!(done["words"].as_array().unwrap().len() > 2);
        assert!(done["final_ms"].as_i64().unwrap() >= 0);
        let text = report(wav.file_name().unwrap().to_str().unwrap(), &r);
        assert!(overlap(&text, &want) >= 0.9, "{text:?} against {want:?}");
    }
}

#[test]
fn audio_sent_while_the_model_loads_is_held_and_decoded() {
    let Some((dir, w)) = rig() else { return };
    let wav = &wavs(&w)[0];
    let want = reference(&dir, wav);
    let pcm = pcm_of(wav);
    let r = live_run(&dir, wav, &[], |r| {
        // everything at once, and gone before the model can have loaded
        r.write(&pcm);
        r.write(&vec![0u8; 32000]);
        assert!(r.all("loaded").is_empty());
        r.close_stdin();
    });
    let ev = r.names();
    let at = |n: &str| ev.iter().position(|e| e == n).unwrap();
    assert!(at("stopping") < at("loaded"), "{ev:?}");
    let text = report("during load", &r);
    assert!(overlap(&text, &want) >= 0.9, "{text:?} against {want:?}");
}

#[test]
fn live_at_a_clients_rate_is_converted_to_the_engines() {
    let Some((dir, w)) = rig() else { return };
    let wav = &wavs(&w)[0];
    let want = reference(&dir, wav);
    let at_48k = Resampler::new(16000.0, 48000.0).push(&[pcm_of(wav), vec![0u8; 32000]].concat());
    let r = live_run(&dir, wav, &["--rate", "48000"], |r| {
        r.write(&at_48k);
        r.close_stdin();
    });
    let text = report("48 kHz client", &r);
    assert!(overlap(&text, &want) >= 0.8, "{text:?} against {want:?}");
}

#[test]
fn file_gives_the_words_of_each_recording() {
    let Some((dir, w)) = rig() else { return };
    for wav in wavs(&w).iter().take(3) {
        let want = reference(&dir, wav);
        let p = wav.to_str().unwrap();
        let t0 = Instant::now();
        let mut r = Run::new(&["file", p, "--engine", "local", "--local-dir", &dir], NONE, &[]);
        assert_eq!(r.done(), 0);
        assert_eq!(r.names(), vec!["ready", "done", "end"]);
        let e = &r.events()[1];
        assert_eq!(e["item"], "file");
        let text = e["text"].as_str().unwrap();
        eprintln!("file {}: {} ms: {text:?}", wav.file_name().unwrap().to_str().unwrap(), t0.elapsed().as_millis());
        assert!(overlap(text, &want) >= 0.9, "{text:?} against {want:?}");
    }
}

#[test]
fn file_reads_a_rings_multipart_body() {
    let Some((dir, w)) = rig() else { return };
    let wav = &wavs(&w)[0];
    let want = reference(&dir, wav);
    let mut f = backplane_voice::multipart::Writer::new("ringboundary".to_string());
    f.file("audio", "note.wav", "audio/wav", &std::fs::read(wav).unwrap());
    f.field("client", "pebble");
    let ct = f.content_type();
    let t = tempfile::tempdir().unwrap();
    let p = write_file(t.path(), "body.bin", &f.finish());
    let mut r = Run::new(&["file", &p, "--engine", "local", "--local-dir", &dir, "--content-type", &ct], NONE, &[]);
    assert_eq!(r.done(), 0);
    let text = r.events()[1]["text"].as_str().unwrap().to_string();
    assert!(overlap(&text, &want) >= 0.9, "{text:?} against {want:?}");
}

#[test]
fn a_resident_server_decodes_for_live_and_file_and_a_session_without_it_loads_its_own() {
    let Some((dir, w)) = rig() else { return };
    let wav = &wavs(&w)[0];
    let want = reference(&dir, wav);
    let t = tempfile::tempdir().unwrap();
    let sock = t.path().join("engine.sock");
    let sock = sock.to_str().unwrap();
    let mut srv = Run::new(&["serve", "--local-dir", &dir, "--socket", sock], NONE, &[]);
    let l = srv.wait_for("loaded", Duration::from_secs(60)).expect("loaded");
    eprintln!("serve: loaded in {} ms, {} MB resident", l["ms"], l["rss_kb"].as_u64().unwrap() / 1024);
    let all = [pcm_of(wav), vec![0u8; 32000]].concat();
    // a live session and a ring's file at once, both through the one recognizer
    let p = wav.to_str().unwrap();
    let mut f = Run::new(&["file", p, "--engine", "local", "--local-dir", &dir, "--server", sock], NONE, &[]);
    let r = live_run(&dir, wav, &["--server", sock], |r| {
        r.write(&all);
        r.close_stdin();
    });
    let loaded = r.next("loaded");
    assert_eq!(loaded["resident"], true, "{loaded}");
    assert!(loaded["ms"].as_i64().unwrap() < 200, "{loaded}");
    let text = report("resident", &r);
    assert!(overlap(&text, &want) >= 0.9, "{text:?} against {want:?}");
    assert_eq!(f.done(), 0);
    let ft = f.events()[1]["text"].as_str().unwrap().to_string();
    assert!(overlap(&ft, &want) >= 0.9, "{ft:?} against {want:?}");
    // no server there: the session builds its own
    let r = live_run(&dir, wav, &["--server", &format!("{sock}.none")], |r| {
        r.write(&all);
        r.close_stdin();
    });
    assert_eq!(r.next("loaded")["resident"], false);
    let text = report("no server", &r);
    assert!(overlap(&text, &want) >= 0.9, "{text:?} against {want:?}");
    // its stdin's end is the server's
    srv.close_stdin();
    assert_eq!(srv.done(), 0);
    assert!(!Path::new(sock).exists(), "the socket is removed");
}
