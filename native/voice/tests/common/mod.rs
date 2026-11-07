//! A local stand-in for the OpenAI API (the realtime transcription socket and
//! POST /audio/transcriptions; only the key "good" is accepted), a file
//! server for downloads, and a way to run the helper and read its events.
#![allow(dead_code)]

use backplane_voice::multipart::{self, Part};
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub const RATE: usize = 24000;

fn http(status: &str, ctype: &str, body: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    r.extend_from_slice(body);
    r
}

struct Head {
    method: String,
    path: String,
    query: String,
    headers: HashMap<String, String>,
    len: usize,
}

/// The request's head, left unread: the socket's handshake reads it itself.
async fn peek_head(s: &TcpStream) -> Option<Head> {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = s.peek(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        if let Some(e) = buf[..n].windows(4).position(|w| w == b"\r\n\r\n") {
            let text = String::from_utf8_lossy(&buf[..e]).into_owned();
            let mut lines = text.split("\r\n");
            let mut first = lines.next()?.split(' ');
            let (method, target) = (first.next()?.to_string(), first.next()?.to_string());
            let (path, query) = target.split_once('?').map_or((target.clone(), String::new()), |(p, q)| (p.to_string(), q.to_string()));
            let headers = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string())).collect();
            return Some(Head { method, path, query, headers, len: e + 4 });
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

async fn read_body(s: &mut TcpStream, h: &Head) -> Vec<u8> {
    let mut head = vec![0u8; h.len];
    let _ = s.read_exact(&mut head).await;
    let n: usize = h.headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut body = vec![0u8; n];
    let _ = s.read_exact(&mut body).await;
    body
}

#[derive(Default)]
pub struct Shared {
    pub ws: Mutex<Vec<Value>>,
    pub rest: Mutex<Vec<Vec<Part>>>,
    pub reject_hints: AtomicBool,
    pub refuse_ws: AtomicBool,
}

pub struct Mock {
    pub base: String,
    pub s: Arc<Shared>,
    _rt: tokio::runtime::Runtime,
}

impl Mock {
    pub fn start() -> Mock {
        Mock::start_on(0)
    }

    /// On a given port (0: any).
    pub fn start_on(port: u16) -> Mock {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        let s = Arc::new(Shared::default());
        let shared = s.clone();
        let l = rt.block_on(TcpListener::bind(("127.0.0.1", port))).unwrap();
        let base = format!("http://{}/v1", l.local_addr().unwrap());
        rt.spawn(async move {
            while let Ok((c, _)) = l.accept().await {
                tokio::spawn(serve(c, shared.clone()));
            }
        });
        Mock { base, s, _rt: rt }
    }

    pub fn ws(&self) -> Vec<Value> {
        self.s.ws.lock().unwrap().clone()
    }

    pub fn of_type(&self, t: &str) -> Vec<Value> {
        self.ws().into_iter().filter(|e| e["type"] == t).collect()
    }

    pub fn rest(&self) -> Vec<Vec<Part>> {
        self.s.rest.lock().unwrap().clone()
    }
}

async fn serve(mut c: TcpStream, m: Arc<Shared>) {
    let Some(h) = peek_head(&c).await else { return };
    if h.headers.get("authorization").map(String::as_str) != Some("Bearer good") {
        let _ = read_body(&mut c, &h).await;
        let _ = c.write_all(&http("401 Unauthorized", "application/json", json!({"error": {"message": "Incorrect API key"}}).to_string().as_bytes())).await;
        return;
    }
    if h.path == "/v1/realtime" {
        if m.refuse_ws.load(Ordering::SeqCst) || !h.query.split('&').any(|p| p == "intent=transcription") {
            let _ = read_body(&mut c, &h).await;
            let _ = c.write_all(&http("500 Internal Server Error", "text/plain", b"no")).await;
            return;
        }
        if let Ok(ws) = tokio_tungstenite::accept_async(c).await {
            socket(ws, m).await;
        }
        return;
    }
    let body = read_body(&mut c, &h).await;
    let answer = if h.path == "/v1/models" {
        http("200 OK", "application/json", json!({"data": []}).to_string().as_bytes())
    } else if h.path == "/v1/audio/transcriptions" && h.method == "POST" {
        let parts = multipart::parse(&body, h.headers.get("content-type").map_or("", String::as_str)).unwrap();
        let hints = parts.iter().any(|p| p.name == "keywords[]" || p.name == "languages[]");
        let size = parts.iter().find(|p| p.name == "file").map_or(0, |p| p.data.len());
        m.rest.lock().unwrap().push(parts);
        if m.reject_hints.load(Ordering::SeqCst) && hints {
            http("400 Bad Request", "application/json", json!({"error": {"message": "Unrecognized request argument supplied: keywords"}}).to_string().as_bytes())
        } else {
            http("200 OK", "application/json", json!({"text": format!("rest {size} bytes")}).to_string().as_bytes())
        }
    } else {
        http("404 Not Found", "text/plain", b"not found")
    };
    let _ = c.write_all(&answer).await;
}

async fn socket(ws: tokio_tungstenite::WebSocketStream<TcpStream>, m: Arc<Shared>) {
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(t) = rx.recv().await {
            if sink.send(Message::text(t)).await.is_err() {
                break;
            }
        }
    });
    // the 50 ms answers go out in commit order, as mock.ts's timers do
    let (later, mut later_rx) = mpsc::unbounded_channel::<(tokio::time::Instant, Vec<String>)>();
    let to = tx.clone();
    tokio::spawn(async move {
        while let Some((at, msgs)) = later_rx.recv().await {
            tokio::time::sleep_until(at).await;
            for t in msgs {
                let _ = to.send(t);
            }
        }
    });
    let (mut bytes, mut n) = (0usize, 0u32);
    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(raw) = msg else { continue };
        let ev: Value = serde_json::from_str(raw.as_str()).unwrap();
        m.ws.lock().unwrap().push(ev.clone());
        match ev["type"].as_str().unwrap_or("") {
            "input_audio_buffer.append" => bytes += base64::engine::general_purpose::STANDARD.decode(ev["audio"].as_str().unwrap()).unwrap().len(),
            "input_audio_buffer.clear" => bytes = 0,
            "input_audio_buffer.commit" => {
                if bytes < 4800 {
                    let _ = tx.send(json!({"type": "error", "error": {"message": "buffer too small", "event_id": ev["event_id"]}}).to_string());
                    continue;
                }
                n += 1;
                let id = format!("item_{n}");
                let head = format!("seg{n}");
                let text = format!("{head} {}ms", (bytes as f64 / 48.0).round());
                bytes = 0;
                let _ = tx.send(json!({"type": "input_audio_buffer.committed", "item_id": id}).to_string());
                let _ = tx.send(json!({"type": "conversation.item.input_audio_transcription.delta", "item_id": id, "delta": head}).to_string());
                let _ = later.send((
                    tokio::time::Instant::now() + Duration::from_millis(50),
                    vec![
                        json!({"type": "conversation.item.input_audio_transcription.delta", "item_id": id, "delta": text[head.len()..]}).to_string(),
                        json!({"type": "conversation.item.input_audio_transcription.completed", "item_id": id, "transcript": text}).to_string(),
                    ],
                ));
            }
            _ => {}
        }
    }
}

/// Serves FILES by path, /redirect/<path> by a 302, counting what is asked.
pub struct Files {
    pub base: String,
    pub hits: Arc<AtomicUsize>,
    _rt: tokio::runtime::Runtime,
}

impl Files {
    pub fn start(files: HashMap<String, Vec<u8>>) -> Files {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        let l = rt.block_on(TcpListener::bind("127.0.0.1:0")).unwrap();
        let addr: SocketAddr = l.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let count = hits.clone();
        let files = Arc::new(files);
        rt.spawn(async move {
            while let Ok((mut c, _)) = l.accept().await {
                let (files, count) = (files.clone(), count.clone());
                tokio::spawn(async move {
                    let Some(h) = peek_head(&c).await else { return };
                    let _ = read_body(&mut c, &h).await;
                    count.fetch_add(1, Ordering::SeqCst);
                    let answer = if let Some(rest) = h.path.strip_prefix("/redirect") {
                        format!("HTTP/1.1 302 Found\r\nLocation: {rest}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
                    } else if let Some(b) = files.get(&h.path) {
                        http("200 OK", "application/octet-stream", b)
                    } else {
                        http("404 Not Found", "text/plain", b"not found")
                    };
                    let _ = c.write_all(&answer).await;
                });
            }
        });
        Files { base: format!("http://{addr}"), hits, _rt: rt }
    }
}

// ---- audio

pub fn tone(ms: usize, amp: f64) -> Vec<u8> {
    let n = RATE * ms / 1000;
    let mut b = Vec::with_capacity(n * 2);
    for i in 0..n {
        let x = (amp * (2.0 * std::f64::consts::PI * 220.0 * i as f64 / RATE as f64).sin()).round() as i16;
        b.extend_from_slice(&x.to_le_bytes());
    }
    b
}

pub fn quiet(ms: usize) -> Vec<u8> {
    vec![0; RATE * ms / 1000 * 2]
}

pub fn wav(pcm: &[u8], rate: u32) -> Vec<u8> {
    let mut h = Vec::with_capacity(44 + pcm.len());
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&rate.to_le_bytes());
    h.extend_from_slice(&(rate * 2).to_le_bytes());
    h.extend_from_slice(&2u16.to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    h.extend_from_slice(pcm);
    h
}

// ---- running the helper

// tests only (the mock_openai example has no binary to run)
pub const EXE: &str = match option_env!("CARGO_BIN_EXE_backplane-voice") { Some(p) => p, None => "" };

#[derive(Default)]
struct Log {
    events: Mutex<Vec<Value>>,
    more: Condvar,
    closed: AtomicBool,
}

pub struct Run {
    child: Child,
    stdin: Option<ChildStdin>,
    log: Arc<Log>,
    reader: Option<std::thread::JoinHandle<()>>,
    stderr: Option<std::thread::JoinHandle<String>>,
}

impl Run {
    /// Only PATH, the key, the base and TMPDIR go through, each overridable
    /// (None takes one away).
    pub fn new(args: &[&str], base: &str, env: &[(&str, Option<&str>)]) -> Run {
        let mut vars: HashMap<String, String> = HashMap::new();
        vars.insert("PATH".into(), std::env::var("PATH").unwrap());
        vars.insert("OPENAI_API_KEY".into(), "good".into());
        vars.insert("OPENAI_BASE_URL".into(), base.into());
        vars.insert("TMPDIR".into(), std::env::temp_dir().to_string_lossy().into_owned());
        for (k, v) in env {
            match v {
                Some(v) => vars.insert(k.to_string(), v.to_string()),
                None => vars.remove(*k),
            };
        }
        let mut child = Command::new(EXE).args(args).env_clear().envs(vars).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let log = Arc::new(Log::default());
        let mut out = child.stdout.take().unwrap();
        let l = log.clone();
        let reader = std::thread::spawn(move || {
            let mut buf = vec![];
            let mut b = [0u8; 8192];
            while let Ok(n) = out.read(&mut b) {
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&b[..n]);
                while let Some(i) = buf.iter().position(|c| *c == b'\n') {
                    let line: Vec<u8> = buf.drain(..=i).collect();
                    // every stdout line must be JSON
                    let e: Value = serde_json::from_slice(&line).unwrap_or_else(|_| panic!("not JSON: {}", String::from_utf8_lossy(&line)));
                    l.events.lock().unwrap().push(e);
                    l.more.notify_all();
                }
            }
            l.closed.store(true, Ordering::SeqCst);
            l.more.notify_all();
        });
        let mut err = child.stderr.take().unwrap();
        let stderr = std::thread::spawn(move || {
            let mut s = vec![];
            let _ = err.read_to_end(&mut s);
            String::from_utf8_lossy(&s).into_owned()
        });
        let stdin = child.stdin.take();
        Run { child, stdin, log, reader: Some(reader), stderr: Some(stderr) }
    }

    pub fn write(&mut self, b: &[u8]) {
        let _ = self.stdin.as_mut().unwrap().write_all(b);
    }

    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }

    pub fn term(&self) {
        unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn events(&self) -> Vec<Value> {
        self.log.events.lock().unwrap().clone()
    }

    /// The event names, levels left out.
    pub fn names(&self) -> Vec<String> {
        self.events().iter().map(|e| e["event"].as_str().unwrap().to_string()).filter(|e| e != "level").collect()
    }

    pub fn all(&self, ev: &str) -> Vec<Value> {
        self.events().into_iter().filter(|e| e["event"] == ev).collect()
    }

    /// The first event named EV, waiting for it.
    pub fn next(&self, ev: &str) -> Value {
        self.wait_for(ev, Duration::from_secs(30)).unwrap_or_else(|| panic!("no {ev} event; got {:?}", self.events()))
    }

    pub fn wait_for(&self, ev: &str, limit: Duration) -> Option<Value> {
        let end = Instant::now() + limit;
        let mut g = self.log.events.lock().unwrap();
        loop {
            if let Some(e) = g.iter().find(|e| e["event"] == ev) {
                return Some(e.clone());
            }
            if self.log.closed.load(Ordering::SeqCst) || Instant::now() >= end {
                return None;
            }
            g = self.log.more.wait_timeout(g, Duration::from_millis(100)).unwrap().0;
        }
    }

    /// Waits for the exit; the exit code. stdin stays open, as in
    /// voice.test.ts: its end is a stop, which a test asks for by
    /// close_stdin(). Gives up after a minute rather than hang the run.
    pub fn done(&mut self) -> i32 {
        let end = Instant::now() + Duration::from_secs(60);
        let s = loop {
            if let Some(s) = self.child.try_wait().unwrap() {
                break s;
            }
            if Instant::now() >= end {
                let _ = self.child.kill();
                panic!("the helper did not exit within 60 s; got {:?}", self.events());
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        self.stdin = None;
        if let Some(r) = self.reader.take() {
            r.join().expect("the event reader");
        }
        if let Some(e) = self.stderr.take() {
            let t = e.join().unwrap();
            if !t.is_empty() {
                eprint!("{t}");
            }
        }
        s.code().unwrap_or(-1)
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A folder of scripts for PATH, and files beside them.
pub struct Bins {
    pub dir: tempfile::TempDir,
}

impl Bins {
    pub fn new(files: &[(&str, String)]) -> Bins {
        let dir = tempfile::tempdir().unwrap();
        let b = Bins { dir };
        for (name, body) in files {
            b.script(name, body);
        }
        b
    }

    pub fn script(&self, name: &str, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        let p = self.dir.path().join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    pub fn path(&self) -> &str {
        self.dir.path().to_str().unwrap()
    }

    pub fn pcm(&self, name: &str, b: &[u8]) -> PathBuf {
        let p = self.dir.path().join(name);
        std::fs::write(&p, b).unwrap();
        p
    }

    pub fn args(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("args")).unwrap_or_default()
    }
}

/// A recorder's script: writes its arguments to <bin>/args, plays, then
/// waits to be killed.
pub fn recorder_body(play: &str) -> String {
    format!("[ \"$1\" = -L ] && exit 1\necho \"$@\" > \"$(/usr/bin/dirname \"$0\")/args\"\n{play}\nexec /bin/sleep 60")
}

/// A fake recorder that plays speech unless PLAY says otherwise.
pub fn fake_recorder(name: &str, play: Option<&str>) -> Bins {
    let b = Bins::new(&[]);
    let speech = b.pcm("speech.pcm", &[quiet(200), tone(1200, 8000.0)].concat());
    let body = match play {
        Some(p) => recorder_body(p),
        None => recorder_body(&format!("/bin/cat '{}'", speech.display())),
    };
    b.script(name, &body);
    b
}

pub fn pactl_json() -> String {
    json!([
        {"name": "alsa_input.usb-mic.mono", "description": "USB Mic Mono", "monitor_of_sink": null, "properties": {}},
        {"name": "alsa_output.hdmi.monitor", "description": "Monitor of HDMI", "monitor_of_sink": "alsa_output.hdmi", "properties": {"device.class": "monitor"}},
        {"name": "alsa_input.webcam.stereo", "description": "Webcam Stereo", "monitor_of_sink": "n/a", "properties": {}},
    ])
    .to_string()
}

/// pactl answering like PulseAudio (or pipewire-pulse) does
pub fn fake_pactl() -> String {
    format!("if [ \"$1\" = get-default-source ]; then echo alsa_input.usb-mic.mono; else /bin/cat <<'J'\n{}\nJ\nfi", pactl_json())
}

pub fn write_file(dir: &Path, name: &str, b: &[u8]) -> String {
    let p = dir.join(name);
    std::fs::write(&p, b).unwrap();
    p.to_str().unwrap().to_string()
}

/// A Pebble Index style webhook body: multipart with an audio part.
pub fn pebble_body(with_audio: bool) -> (Vec<u8>, String) {
    let mut w = multipart::Writer::new("pebbleboundary".to_string());
    if with_audio {
        let pcm = [tone(2000, 8000.0), quiet(300), tone(1000, 8000.0)].concat();
        w.file("audio", "note.wav", "audio/wav", &wav(&pcm, RATE as u32));
    }
    w.field("transcription", "on-ring text");
    w.field("recordedAt", "2026-09-25T10:00:00Z");
    w.field("client", "pebble");
    let ct = w.content_type();
    (w.finish(), ct)
}

pub fn fields<'a>(parts: &'a [Part], name: &str) -> Vec<&'a Part> {
    parts.iter().filter(|p| p.name == name).collect()
}

pub fn field_text(parts: &[Part], name: &str) -> Option<String> {
    fields(parts, name).first().map(|p| String::from_utf8_lossy(&p.data).into_owned())
}
