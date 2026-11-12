//! The resident recognizer: `serve` loads the model once and decodes for every
//! `live` and `file` that names its socket (`--server`), so a session starts
//! without the second it takes to build the recognizer. One process, one
//! recognizer, decodes one at a time.
//!
//! On the socket, a client writes `bpv1 <model>\n` and reads `ok\n` (or
//! `no <why>\n`); then each decode is a frame: the sample count (u32 LE) and
//! the samples (f32 LE), answered by the length (u32 LE) and sherpa-onnx's
//! JSON. The server ends when its stdin ends (the hub that started it went
//! away) or when a line `stop` comes.

use crate::args::Args;
use crate::local::{self, Word};
#[cfg_attr(not(unix), allow(unused_imports))]
use crate::out::{fail, log};
use crate::session::Decoder;
use crate::sherpa::{Api, Recognizer};
use std::path::Path;
use std::sync::Arc;

pub const THREADS: i32 = 4;
const HELLO: &str = "bpv1";
/// 20 s is the longest segment; a frame well over that is not ours.
const MAX_SAMPLES: u32 = 16000 * 120;
/// A frame whose count has this bit is an end-of-turn question (turn.rs), not a decode.
const TURN_BIT: u32 = 1 << 31;

/// Gives freed heap back to the system: building the recognizer leaves about
/// 300 MB of parsing buffers behind (2026-11-10, Parakeet v2 int8).
pub fn trim() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        malloc_trim(0);
    }
}

/// This process's resident memory in kB (Linux; 0 elsewhere).
pub fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()))
        .unwrap_or(0)
}

/// What a session decodes with: the resident recognizer through its socket,
/// or one built here (no server, or it went away mid-session).
pub enum Rec {
    Here(Recognizer),
    #[cfg(unix)]
    There(Remote),
}

impl Decoder for Rec {
    fn decode(&mut self, samples: &[f32]) -> Vec<Word> {
        #[cfg(unix)]
        if let Rec::There(c) = self {
            match c.decode_json(samples) {
                Ok(j) => return words(&j),
                Err(e) => {
                    log(&format!("the resident recognizer failed ({e}); loading one here"));
                    match Recognizer::create(&c.api, &c.dir, &c.model, THREADS) {
                        Ok(r) => *self = Rec::Here(r),
                        Err(e) => {
                            log(&format!("could not load the local model ({e})"));
                            return vec![];
                        }
                    }
                }
            }
        }
        match self {
            Rec::Here(r) => r.decode(samples),
            #[cfg(unix)]
            Rec::There(_) => vec![],
        }
    }
    fn turn(&mut self, audio: &[f32]) -> Option<f32> {
        #[cfg(unix)]
        if let Rec::There(c) = self {
            if let Ok(j) = c.turn_json(audio) { return serde_json::from_str::<serde_json::Value>(&j).ok().and_then(|v| v["p"].as_f64()).map(|p| p as f32) }
        }
        local_turn(audio)
    }
}

#[cfg(unix)]
fn words(j: &str) -> Vec<Word> {
    match serde_json::from_str(j) {
        Ok(v) => local::words_of(&v),
        Err(e) => {
            log(&format!("decode failed: {e}"));
            vec![]
        }
    }
}

/// Where this process's own end-of-turn model comes from (set once, by `sherpa::engine`), and the model once loaded (None: there is none).
static TURN_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
static TURN_HERE: std::sync::Mutex<Option<Option<crate::turn::Turn>>> = std::sync::Mutex::new(None);
pub fn turn_from(dir: &Path) { let _ = TURN_DIR.set(dir.to_path_buf()); }
/// The model in `dir`, when it is there (runtime and pinned file).
pub fn load_turn(dir: &Path) -> Option<crate::turn::Turn> {
    if !local::turn_ok(dir) { return None }
    match crate::turn::Turn::load(&local::runtime_dir(dir).join(local::onnx_library()), &local::turn_path(dir)) { Ok(t) => Some(t), Err(e) => { log(&format!("end-of-turn model: {e}")); None } }
}
fn local_turn(audio: &[f32]) -> Option<f32> {
    let mut g = TURN_HERE.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() { *g = Some(TURN_DIR.get().and_then(|d| load_turn(d))) }
    g.as_ref()?.as_ref()?.predict(audio).ok()
}

/// The recognizer for model ID: the server's at SERVER when it answers for
/// that model, else built here. True when it is the server's.
pub fn recognizer(api: &Arc<Api>, dir: &Path, id: &str, server: &str) -> Result<(Rec, bool), String> {
    #[cfg(unix)]
    if !server.is_empty() {
        match Remote::connect(api, dir, id, server) {
            Ok(r) => return Ok((Rec::There(r), true)),
            Err(e) => log(&format!("no resident recognizer at {server} ({e}); loading one here")),
        }
    }
    let _ = server;
    Ok((Rec::Here(Recognizer::create(api, dir, id, THREADS)?), false))
}

#[cfg(unix)]
pub use unix::*;

#[cfg(unix)]
mod unix {
    use super::*;
    use crate::out::emit;
    use serde_json::json;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::Mutex;

    pub struct Remote {
        s: UnixStream,
        pub(super) api: Arc<Api>,
        pub(super) dir: PathBuf,
        pub(super) model: String,
    }

    impl Remote {
        pub fn connect(api: &Arc<Api>, dir: &Path, model: &str, path: &str) -> Result<Remote, String> {
            let mut s = UnixStream::connect(path).map_err(|e| e.to_string())?;
            s.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
            s.write_all(format!("{HELLO} {model}\n").as_bytes()).map_err(|e| e.to_string())?;
            let mut line = String::new();
            BufReader::new(&mut s).read_line(&mut line).map_err(|e| e.to_string())?;
            if line.trim() != "ok" {
                return Err(if line.is_empty() { "it closed".into() } else { line.trim().to_string() });
            }
            // a decode waits behind another session's: 20 s of audio is well under a second
            s.set_read_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
            Ok(Remote { s, api: api.clone(), dir: dir.to_path_buf(), model: model.to_string() })
        }

        pub fn decode_json(&mut self, samples: &[f32]) -> Result<String, String> { self.ask(samples, 0) }
        /// The end-of-turn model's answer from the server: `{"p": 0.93}`, or `{}` when it has none.
        pub fn turn_json(&mut self, audio: &[f32]) -> Result<String, String> { self.ask(audio, TURN_BIT) }
        fn ask(&mut self, samples: &[f32], bit: u32) -> Result<String, String> {
            let mut b = Vec::with_capacity(4 + samples.len() * 4);
            b.extend_from_slice(&(samples.len() as u32 | bit).to_le_bytes());
            for x in samples {
                b.extend_from_slice(&x.to_le_bytes());
            }
            self.s.write_all(&b).map_err(|e| e.to_string())?;
            let mut n = [0u8; 4];
            self.s.read_exact(&mut n).map_err(|e| e.to_string())?;
            let mut j = vec![0u8; u32::from_le_bytes(n) as usize];
            self.s.read_exact(&mut j).map_err(|e| e.to_string())?;
            String::from_utf8(j).map_err(|e| e.to_string())
        }
    }

    fn client(mut s: UnixStream, rec: &Mutex<Recognizer>, turn: Option<&Mutex<crate::turn::Turn>>, model: &str) {
        let mut r = BufReader::new(match s.try_clone() {
            Ok(c) => c,
            Err(_) => return,
        });
        let mut hello = String::new();
        if r.read_line(&mut hello).is_err() {
            return;
        }
        let want = hello.trim().strip_prefix(HELLO).map(str::trim);
        if want != Some(model) {
            let _ = s.write_all(format!("no this server has {model}\n").as_bytes());
            return;
        }
        if s.write_all(b"ok\n").is_err() {
            return;
        }
        loop {
            let mut n = [0u8; 4];
            if r.read_exact(&mut n).is_err() {
                return;
            }
            let n = u32::from_le_bytes(n); let is_turn = n & TURN_BIT != 0; let n = n & !TURN_BIT;
            if n > MAX_SAMPLES {
                return;
            }
            let mut raw = vec![0u8; n as usize * 4];
            if r.read_exact(&mut raw).is_err() {
                return;
            }
            let samples: Vec<f32> = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            drop(raw);
            let j = if is_turn {
                match turn.map(|t| t.lock().unwrap_or_else(|e| e.into_inner()).predict(&samples)) { Some(Ok(p)) => json!({"p": p}).to_string(), Some(Err(e)) => { log(&format!("end-of-turn model failed: {e}")); "{}".into() } None => "{}".into() }
            } else {
                rec.lock().unwrap_or_else(|e| e.into_inner()).decode_json(&samples).unwrap_or_else(|e| {
                    log(&format!("decode failed: {e}"));
                    "{}".to_string()
                })
            };
            let mut out = (j.len() as u32).to_le_bytes().to_vec();
            out.extend_from_slice(j.as_bytes());
            if s.write_all(&out).is_err() {
                return;
            }
        }
    }

    /// `serve`: loads the model, says `loaded`, then answers on the socket until stdin ends.
    pub fn serve(a: Args) -> ! {
        let dir = PathBuf::from(&a.local_dir);
        if let Some(why) = local::ready(&dir, &a.model) {
            fail(&why);
        }
        let t0 = Instant::now();
        let api = Api::load(&dir).unwrap_or_else(|e| fail(&e));
        let rec = Recognizer::create(&api, &dir, &a.model, THREADS).unwrap_or_else(|e| fail(&format!("could not load the local model ({e})")));
        // the first decode pays for its buffers; a second of silence pays it now
        let _ = rec.decode_json(&vec![0.0; 16000]);
        trim();
        let ms = t0.elapsed().as_millis() as u64;
        let _ = std::fs::remove_file(&a.socket);
        let l = UnixListener::bind(&a.socket).unwrap_or_else(|e| fail(&format!("cannot listen on {} ({e})", a.socket)));
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&a.socket, std::fs::Permissions::from_mode(0o600));
        }
        let has_turn = local::turn_ok(&dir);
        emit(json!({"event": "loaded", "ms": ms, "rss_kb": rss_kb(), "model": a.model, "turn": has_turn}));
        let sock = a.socket.clone();
        std::thread::spawn(move || {
            let mut line = String::new();
            let mut i = std::io::stdin().lock();
            while i.read_line(&mut line).is_ok_and(|n| n > 0) && line.trim() != "stop" {
                line.clear();
            }
            let _ = std::fs::remove_file(&sock);
            crate::out::end()
        });
        let rec: &'static Mutex<Recognizer> = Box::leak(Box::new(Mutex::new(rec)));
        let turn: Option<&'static Mutex<crate::turn::Turn>> = load_turn(&dir).map(|t| &*Box::leak(Box::new(Mutex::new(t))));
        let model: &'static str = Box::leak(a.model.clone().into_boxed_str());
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                client(s, rec, turn, model);
                trim();
            });
        }
        fail("the socket closed")
    }
}

#[cfg(not(unix))]
pub fn serve(_: Args) -> ! {
    fail("serve is for Linux and macOS")
}
