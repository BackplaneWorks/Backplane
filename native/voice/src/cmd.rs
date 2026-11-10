//! `file`, `devices`, the `local-*` commands, and what they share with `live`.

use crate::args::{keywords_of, Args};
use crate::devices::{list_devices, Device};
use crate::live::CHUNK;
use crate::local::{self, Pins};
use crate::multipart::{self, Part};
use crate::openai::{base_url, transcribe_file, Hooks, Opts, Realtime};
use crate::out::{emit, end, error, fail, log};
use crate::session;
use crate::sherpa;
use crate::util::which;
use crate::vad::BYTES_PER_MS;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FILE_LIMIT: Duration = Duration::from_secs(90);
const FILE_SEGMENT: usize = 15000 * BYTES_PER_MS; // at most 15 s per commit

/// The key and the hints, or the end of the run.
pub fn opts(a: &Args) -> Opts {
    let key = match &a.key_file {
        Some(f) => match std::fs::read(f) {
            Ok(b) => String::from_utf8_lossy(&b).trim().to_string(),
            Err(_) => fail(&format!("cannot read the key file {f}")),
        },
        None => std::env::var("OPENAI_API_KEY").unwrap_or_default().trim().to_string(),
    };
    if key.is_empty() {
        fail("no OpenAI API key (set one in Backplane's settings)");
    }
    let keywords = match &a.keywords_file {
        None => vec![],
        Some(f) => match std::fs::read(f) {
            Ok(b) => keywords_of(&String::from_utf8_lossy(&b)),
            Err(_) => {
                log(&format!("cannot read keywords file {f}; going without"));
                vec![]
            }
        },
    };
    Opts {
        key,
        base: base_url(std::env::var("OPENAI_BASE_URL").ok()),
        prompt: a.prompt.clone(),
        keywords,
        languages: a.languages.clone(),
        delay: a.delay.clone().unwrap_or_else(|| "low".to_string()),
    }
}

// ---- devices

pub async fn devices() -> ! {
    let d = list_devices().await;
    let list: Vec<Value> = d.devices.iter().map(Device::to_json).collect();
    emit(json!({"event": "devices", "default": d.default, "devices": list}));
    if d.via.is_empty() {
        error("no way to list audio devices (pactl, pw-dump or arecord)");
    }
    end()
}

// ---- file

fn exists(p: &str) -> bool {
    std::fs::metadata(p).is_ok_and(|m| !m.is_dir())
}

fn after_timeout() {
    tokio::spawn(async {
        tokio::time::sleep(FILE_LIMIT).await;
        fail("timed out after 90 s");
    });
}

pub fn basename(p: &str) -> &str {
    p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("")
}

pub fn ext_of(ctype: &str) -> &'static str {
    let t = ctype.to_lowercase();
    if t.contains("mp4") || t.contains("m4a") || t.contains("aac") {
        "m4a"
    } else if t.contains("wav") {
        "wav"
    } else if t.contains("mpeg") || t.contains("mp3") {
        "mp3"
    } else if t.contains("ogg") {
        "ogg"
    } else if t.contains("webm") {
        "webm"
    } else if t.contains("flac") {
        "flac"
    } else {
        "m4a"
    }
}

/// /\.[a-z0-9]+$/i
pub fn has_ext(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, e)| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// What a file's name says it is; a recording sent as it is has no other label.
fn type_of(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|x| x.1.to_lowercase()).as_deref() {
        Some("m4a" | "mp4") => "audio/mp4",
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("webm") => "audio/webm",
        Some("flac") => "audio/flac",
        _ => "application/octet-stream",
    }
}

async fn decode(path: &Path, rate: u32) -> Result<Vec<u8>, String> {
    let exe = which("ffmpeg").ok_or("ffmpeg: not found")?;
    let out = tokio::process::Command::new(exe)
        .args(["-nostdin", "-loglevel", "error", "-i"])
        .arg(path)
        .args(["-f", "s16le", "-ac", "1", "-ar", &rate.to_string(), "-"])
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let last = err.trim().rsplit('\n').next().unwrap_or("").to_string();
        return Err(format!("ffmpeg: {}", if last.is_empty() { format!("exit {}", out.status.code().unwrap_or(-1)) } else { last }));
    }
    Ok(out.stdout)
}

/// Whole PCM through gpt-live-transcribe: one commit per <= 15 s piece.
async fn live_file(o: &Opts, pcm: &[u8], deadline: Instant) -> Result<String, String> {
    if pcm.len() < CHUNK {
        return Ok(String::new());
    }
    let errs = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen = errs.clone();
    let hooks = Hooks { error: Some(Arc::new(move |m| seen.lock().unwrap().push(m.to_string()))), ..Default::default() };
    let rt = Realtime::connect(o, hooks).await?;
    let n = pcm.len().div_ceil(FILE_SEGMENT);
    let size = (pcm.len() as f64 / n as f64 / 2.0).ceil() as usize * 2;
    for piece in pcm.chunks(size) {
        for c in piece.chunks(10 * CHUNK) {
            rt.append(c);
        }
        rt.commit();
    }
    let left = deadline.saturating_duration_since(Instant::now()).as_millis() as i64 - 20000;
    let ok = rt.wait_all(left.max(1000) as u64).await;
    let text = rt.joined();
    rt.close().await;
    if let Some(e) = errs.lock().unwrap().first() {
        return Err(e.clone());
    }
    if !ok {
        return Err("no transcript from the realtime session".to_string());
    }
    Ok(text)
}

/// The `audio` part of a body in F, or the end of the run.
fn audio_of(body: &[u8], ctype: &str, on_bad: impl Fn(&str) -> String) -> Part {
    let parts = multipart::parse(body, ctype).unwrap_or_else(|e| fail(&on_bad(&e)));
    multipart::audio_part(parts).unwrap_or_else(|| fail("no audio part"))
}

fn is_multipart(ctype: &str) -> bool {
    ctype.to_lowercase().starts_with("multipart/form-data")
}

pub async fn file(a: Args) -> ! {
    if a.engine == "local" {
        local_file(a).await;
    }
    let o = opts(&a);
    let path = a.pos[0].clone();
    let deadline = Instant::now() + FILE_LIMIT;
    after_timeout();
    emit(json!({"event": "ready"}));
    if !exists(&path) {
        fail(&format!("no such file: {path}"));
    }
    let bytes = std::fs::read(&path).unwrap_or_else(|e| fail(&e.to_string()));
    let (audio, name, ctype, on_disk, temp): (Vec<u8>, String, String, PathBuf, bool) = if is_multipart(&a.content_type) {
        let part = audio_of(&bytes, &a.content_type, |e| format!("bad multipart body: {e}"));
        let mut name = part.filename.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| "audio.m4a".to_string());
        if !has_ext(&name) {
            name = format!("{name}.{}", ext_of(&part.content_type));
        }
        let t = std::env::temp_dir().join(format!("backplane-voice-{}-{}", std::process::id(), basename(&name)));
        (part.data, name, part.content_type, t, true)
    } else {
        let name = basename(&path).to_string();
        let ctype = if a.content_type.is_empty() { type_of(&name).to_string() } else { a.content_type.clone() };
        (bytes, name, ctype, PathBuf::from(&path), false)
    };

    let mut text = None;
    if which("ffmpeg").is_some() {
        let wrote = if temp { std::fs::write(&on_disk, &audio).map_err(|e| e.to_string()) } else { Ok(()) };
        let got = match wrote {
            Ok(()) => match decode(&on_disk, crate::vad::RATE).await {
                Ok(pcm) => live_file(&o, &pcm, deadline).await,
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        };
        if temp {
            let _ = std::fs::remove_file(&on_disk);
        }
        match got {
            Ok(t) => text = Some(t),
            Err(e) => log(&format!("realtime path failed ({e}); trying gpt-transcribe")),
        }
    }
    let text = match text {
        Some(t) => t,
        None => {
            let left = deadline.max(Instant::now() + Duration::from_secs(1));
            let r = tokio::task::spawn_blocking(move || transcribe_file(&o, &audio, &name, &ctype, left)).await;
            match r {
                Ok(Ok(t)) => t,
                Ok(Err(e)) => fail(&e),
                Err(e) => fail(&e.to_string()),
            }
        }
    };
    emit(json!({"event": "done", "item": "file", "text": text.trim()}));
    end()
}

/// A recording through the local engine: ffmpeg to 16 kHz PCM, then the
/// segments one by one.
async fn local_file(a: Args) -> ! {
    let dir = PathBuf::from(&a.local_dir);
    if let Some(why) = local::ready(&dir, &a.model) {
        fail(&why);
    }
    after_timeout();
    emit(json!({"event": "ready"}));
    let path = a.pos[0].clone();
    if !exists(&path) {
        fail(&format!("no such file: {path}"));
    }
    if which("ffmpeg").is_none() {
        fail("ffmpeg is needed to read recordings");
    }
    let mut on_disk = PathBuf::from(&path);
    let mut temp = false;
    if is_multipart(&a.content_type) {
        let body = std::fs::read(&path).unwrap_or_else(|e| fail(&e.to_string()));
        let part = audio_of(&body, &a.content_type, |e| e.to_string());
        on_disk = std::env::temp_dir().join(format!("backplane-voice-{}.audio", std::process::id()));
        temp = true;
        if let Err(e) = std::fs::write(&on_disk, &part.data) {
            fail(&e.to_string());
        }
    }
    let pcm = decode(&on_disk, local::RATE).await;
    if temp {
        let _ = std::fs::remove_file(&on_disk);
    }
    let pcm = pcm.unwrap_or_else(|e| fail(&e));
    let model = a.model.clone();
    let r = tokio::task::spawn_blocking(move || {
        let (rec, vad) = sherpa::engine(&dir, &model, 4)?;
        Ok::<Vec<session::Seg>, String>(session::segments(vad, rec, &pcm))
    })
    .await;
    match r {
        Ok(Ok(segs)) => {
            let text = segs.iter().map(|s| s.3.as_str()).collect::<Vec<_>>().join(" ");
            let segments: Vec<Value> = segs.iter().map(|s| json!([s.0, s.1, s.2, s.3])).collect();
            emit(json!({"event": "done", "item": "file", "text": text.trim(), "segments": segments}))
        }
        Ok(Err(e)) => fail(&e),
        Err(e) => fail(&e.to_string()),
    }
    end()
}

// ---- the local-* commands

fn say_local(dir: &Path) {
    let mut ev = Map::new();
    ev.insert("event".to_string(), json!("local"));
    if let Value::Object(s) = local::status(dir) {
        ev.extend(s);
    }
    ev.insert("bytes".to_string(), json!(local::disk_bytes(dir)));
    emit(Value::Object(ev));
}

pub fn local_cmd(a: Args) -> ! {
    let dir = PathBuf::from(&a.local_dir);
    match a.cmd.as_str() {
        "local-status" => {}
        "local-remove" => local::remove(&dir, &a.model),
        _ => {
            let mut progress = |what: &str, done: u64, total: u64, phase: &str| emit(json!({"event": "progress", "what": what, "done": done, "total": total, "phase": phase}));
            if let Err(e) = local::install(&dir, &a.model, &Pins::pinned(), &mut progress) {
                error(&e);
            }
        }
    }
    say_local(&dir);
    end()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_types() {
        assert_eq!(ext_of("audio/mp4"), "m4a");
        assert_eq!(ext_of("audio/x-wav"), "wav");
        assert_eq!(ext_of("audio/mpeg"), "mp3");
        assert_eq!(ext_of("AUDIO/OGG"), "ogg");
        assert_eq!(ext_of("audio/webm"), "webm");
        assert_eq!(ext_of("audio/flac"), "flac");
        assert_eq!(ext_of(""), "m4a");
        assert!(has_ext("note.WAV"));
        assert!(!has_ext("note"));
        assert!(!has_ext("note."));
        assert!(!has_ext("note.a-b"));
        assert_eq!(basename("../../x.wav"), "x.wav");
        assert_eq!(basename("a\\b.wav"), "b.wav");
        assert_eq!(type_of("a.M4A"), "audio/mp4");
        assert_eq!(type_of("a"), "application/octet-stream");
    }
}
