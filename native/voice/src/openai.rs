//! OpenAI speech to text: the Realtime transcription session
//! (gpt-live-transcribe) and the REST batch endpoint (gpt-transcribe).

use crate::devices::text_of;
use crate::multipart::Writer;
use crate::net;
use crate::vad::Action;
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Map, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Notify};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async_tls_with_config, Connector, MaybeTlsStream, WebSocketStream};

pub const LIVE_MODEL: &str = "gpt-live-transcribe";
pub const FILE_MODEL: &str = "gpt-transcribe";
pub const DELAYS: [&str; 5] = ["minimal", "low", "medium", "high", "xhigh"];
const CONNECT_LIMIT: Duration = Duration::from_secs(15);
const CLOSE_FLUSH: Duration = Duration::from_millis(1000);
const MODELS_LIMIT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Default)]
pub struct Opts {
    pub key: String,
    pub base: String, // e.g. https://api.openai.com/v1, no trailing slash
    pub prompt: String,
    pub keywords: Vec<String>,
    pub languages: Vec<String>,
    pub delay: String,
}

pub fn base_url(env_value: Option<String>) -> String {
    let b = env_value.filter(|b| !b.is_empty()).unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    b.trim_end_matches('/').to_string()
}

pub fn ws_url(base: &str) -> String {
    let s = base.strip_prefix("http").map_or_else(|| base.to_string(), |r| format!("ws{r}"));
    s + "/realtime?intent=transcription"
}

pub fn session_update(o: &Opts) -> Value {
    let mut t = Map::new();
    t.insert("model".into(), json!(LIVE_MODEL));
    if !o.prompt.is_empty() {
        t.insert("prompt".into(), json!(o.prompt));
    }
    if !o.keywords.is_empty() {
        t.insert("keywords".into(), json!(o.keywords));
    }
    if !o.languages.is_empty() {
        t.insert("languages".into(), json!(o.languages));
    }
    if !o.delay.is_empty() {
        t.insert("delay".into(), json!(o.delay));
    }
    json!({
        "type": "session.update",
        "session": {
            "type": "transcription",
            "audio": {
                "input": {
                    "format": {"type": "audio/pcm", "rate": 24000},
                    "transcription": t,
                    "turn_detection": null,
                },
            },
        },
    })
}

/// Why a connection failed, in words: ask a cheap REST endpoint whether the
/// key is the reason.
fn why_refused_blocking(o: &Opts) -> String {
    let url = format!("{}/models", o.base);
    match net::agent(Some(MODELS_LIMIT)).get(&url).header("Authorization", format!("Bearer {}", o.key)).call() {
        Ok(r) => match r.status().as_u16() {
            401 => "OpenAI rejected the API key (401)".into(),
            403 => "the API key may not use this model (403)".into(),
            429 => "OpenAI rate limit or quota exceeded (429)".into(),
            _ => "could not open the OpenAI realtime session".into(),
        },
        Err(e) => format!("could not reach OpenAI: {}", net::error_msg(&e)),
    }
}

async fn why_refused(o: &Opts) -> String {
    let o = o.clone();
    tokio::task::spawn_blocking(move || why_refused_blocking(&o)).await.unwrap_or_else(|e| e.to_string())
}

type Delta = Arc<dyn Fn(&str, &str) + Send + Sync>;
type OnError = Arc<dyn Fn(&str) + Send + Sync>;
type Closed = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Default)]
pub struct Hooks {
    pub delta: Option<Delta>,
    pub done: Option<Delta>,
    pub error: Option<OnError>,
    pub closed: Option<Closed>,
}

#[derive(Default)]
struct State {
    commits: u64, // commits sent
    settled: u64, // commits answered (completed, failed, or refused)
    order: Vec<String>, // item ids in commit order
    texts: Vec<(String, String)>, // in the order they finished
    open: bool,
}

pub struct Realtime {
    out: mpsc::UnboundedSender<Message>,
    state: Mutex<State>,
    changed: Notify,
    hooks: Hooks,
    writer: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

async fn open(o: &Opts) -> Result<Socket, String> {
    let mut req = ws_url(&o.base).into_client_request().map_err(|e| e.to_string())?;
    let bearer = format!("Bearer {}", o.key).parse().map_err(|_| "the key is not valid in a header".to_string())?;
    req.headers_mut().insert("Authorization", bearer);
    let tls = Connector::Rustls(net::tls_config());
    let (ws, _) = connect_async_tls_with_config(req, None, false, Some(tls)).await.map_err(|e| e.to_string())?;
    Ok(ws)
}

impl Realtime {
    /// Connects and writes the session update; Err is a human message.
    pub async fn connect(o: &Opts, hooks: Hooks) -> Result<Arc<Realtime>, String> {
        let mut ws = match tokio::time::timeout(CONNECT_LIMIT, open(o)).await {
            Ok(Ok(ws)) => ws,
            Ok(Err(e)) => {
                crate::out::log(&format!("the realtime socket did not open: {e}"));
                return Err(why_refused(o).await);
            }
            Err(_) => return Err(why_refused(o).await),
        };
        // The session update is written (and flushed) before connect returns, not queued: a session stopped at once still has its
        // settings on the socket however long the writer would take to get to them.
        match tokio::time::timeout(CONNECT_LIMIT, ws.send(Message::text(session_update(o).to_string()))).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(format!("the realtime session could not be set up ({e})")),
            Err(_) => return Err("the realtime session could not be set up (it timed out)".to_string()),
        }
        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let rt = Arc::new(Realtime { out: tx, state: Mutex::new(State { open: true, ..Default::default() }), changed: Notify::new(), hooks, writer: Mutex::new(None) });
        let writer = tokio::spawn(async move {
            while let Some(m) = rx.recv().await {
                let closing = matches!(m, Message::Close(_));
                if sink.send(m).await.is_err() || closing {
                    break;
                }
            }
        });
        *rt.writer.lock().unwrap() = Some(writer);
        let reader = rt.clone();
        tokio::spawn(async move {
            while let Some(Ok(m)) = stream.next().await {
                match m {
                    Message::Text(t) => reader.on_message(t.as_str()),
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            reader.state.lock().unwrap().open = false;
            reader.changed.notify_waiters();
            if let Some(f) = &reader.hooks.closed {
                f();
            }
        });
        Ok(rt)
    }

    fn send(&self, ev: &Value) {
        if self.state.lock().unwrap().open {
            let _ = self.out.send(Message::text(ev.to_string()));
        }
    }

    pub fn append(&self, pcm: &[u8]) {
        let audio = base64::engine::general_purpose::STANDARD.encode(pcm);
        self.send(&json!({"type": "input_audio_buffer.append", "audio": audio}));
    }

    pub fn commit(&self) {
        let n = {
            let mut s = self.state.lock().unwrap();
            s.commits += 1;
            s.commits
        };
        self.send(&json!({"type": "input_audio_buffer.commit", "event_id": format!("commit-{n}")}));
    }

    pub fn clear(&self) {
        self.send(&json!({"type": "input_audio_buffer.clear"}));
    }

    pub fn apply(&self, a: &Action) {
        match a {
            Action::Append(b) => self.append(b),
            Action::Commit => self.commit(),
            Action::Clear => self.clear(),
        }
    }

    /// True once every commit is answered, false on timeout or close.
    pub async fn wait_all(&self, ms: u64) -> bool {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let s = self.state.lock().unwrap();
                if s.settled >= s.commits {
                    return true;
                }
                if !s.open {
                    return false;
                }
            }
            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                let s = self.state.lock().unwrap();
                return s.settled >= s.commits;
            }
        }
    }

    /// The finished transcripts in commit order.
    pub fn joined(&self) -> String {
        let s = self.state.lock().unwrap();
        joined(&s.order, &s.texts)
    }

    /// Closes the socket once what is queued has gone out: the process exits
    /// right after, which would drop whatever the writer had not sent yet.
    pub async fn close(&self) {
        self.state.lock().unwrap().open = false;
        let _ = self.out.send(Message::Close(None));
        let w = self.writer.lock().unwrap().take();
        if let Some(w) = w {
            let _ = tokio::time::timeout(CLOSE_FLUSH, w).await;
        }
    }

    fn on_message(&self, data: &str) {
        let Ok(ev) = serde_json::from_str::<Value>(data) else { return };
        let item = || text_of(ev.get("item_id")).unwrap_or_default();
        let message = |or: &str| text_of(ev.get("error").and_then(|e| e.get("message"))).unwrap_or_else(|| or.to_string());
        match ev.get("type").and_then(Value::as_str).unwrap_or("") {
            "input_audio_buffer.committed" => {
                let id = item();
                let mut s = self.state.lock().unwrap();
                if !id.is_empty() && !s.order.contains(&id) {
                    s.order.push(id);
                }
            }
            "conversation.item.input_audio_transcription.delta" => {
                if let Some(f) = &self.hooks.delta {
                    f(&item(), &text_of(ev.get("delta")).unwrap_or_default());
                }
            }
            "conversation.item.input_audio_transcription.completed" => {
                let (id, text) = (item(), text_of(ev.get("transcript")).unwrap_or_default());
                {
                    let mut s = self.state.lock().unwrap();
                    match s.texts.iter_mut().find(|t| t.0 == id) {
                        Some(t) => t.1 = text.clone(),
                        None => s.texts.push((id.clone(), text.clone())),
                    }
                    s.settled += 1;
                }
                if let Some(f) = &self.hooks.done {
                    f(&id, &text);
                }
                self.changed.notify_waiters();
            }
            "conversation.item.input_audio_transcription.failed" => {
                self.state.lock().unwrap().settled += 1;
                if let Some(f) = &self.hooks.error {
                    f(&message("transcription failed"));
                }
                self.changed.notify_waiters();
            }
            "error" => {
                // a refused commit (e.g. an empty buffer) is answered by this error
                let id = text_of(ev.get("error").and_then(|e| e.get("event_id"))).unwrap_or_default();
                if id.starts_with("commit-") {
                    self.state.lock().unwrap().settled += 1;
                }
                if let Some(f) = &self.hooks.error {
                    f(&message("OpenAI error"));
                }
                self.changed.notify_waiters();
            }
            _ => {}
        }
    }
}

fn joined(order: &[String], texts: &[(String, String)]) -> String {
    let text = |k: &str| texts.iter().find(|t| t.0 == k).map_or("", |t| t.1.trim());
    let rest = texts.iter().map(|t| t.0.as_str()).filter(|k| !order.iter().any(|o| o == k));
    let ids = order.iter().map(String::as_str).chain(rest);
    ids.map(text).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ")
}

pub fn rest_error(status: u16, body: &str) -> String {
    if status == 401 {
        return "OpenAI rejected the API key (401)".into();
    }
    let m = serde_json::from_str::<Value>(body).ok().and_then(|v| text_of(v.get("error").and_then(|e| e.get("message")))).unwrap_or_default();
    if m.is_empty() {
        format!("OpenAI answered {status}")
    } else {
        format!("OpenAI answered {status}: {m}")
    }
}

/// A 400 about the hints: /keyword|language/i.
pub fn about_hints(body: &str) -> bool {
    let b = body.to_lowercase();
    b.contains("keyword") || b.contains("language")
}

fn boundary() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("----backplane-voice-{}-{t:x}", std::process::id())
}

/// One POST; Err is a network failure, Ok the status and body of any answer.
fn post(o: &Opts, audio: &[u8], name: &str, ctype: &str, hints: bool, deadline: Instant) -> Result<(u16, String), String> {
    let mut f = Writer::new(boundary());
    f.file("file", name, ctype, audio);
    f.field("model", FILE_MODEL);
    f.field("response_format", "json");
    if !o.prompt.is_empty() {
        f.field("prompt", &o.prompt);
    }
    if hints {
        for k in &o.keywords {
            f.field("keywords[]", k);
        }
        for l in &o.languages {
            f.field("languages[]", l);
        }
    }
    let ct = f.content_type();
    let url = format!("{}/audio/transcriptions", o.base);
    let left = deadline.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
    let r = net::agent(Some(left)).post(&url).header("Authorization", format!("Bearer {}", o.key)).header("Content-Type", ct).send(f.finish());
    match r {
        Ok(mut res) => {
            let status = res.status().as_u16();
            Ok((status, res.body_mut().read_to_string().map_err(|e| net::error_msg(&e))?))
        }
        Err(e) => Err(net::error_msg(&e)),
    }
}

/// POST /audio/transcriptions with gpt-transcribe. Keywords and language
/// hints go as repeated keywords[] / languages[] fields; a 400 about them
/// retries once without them. Blocking.
pub fn transcribe_file(o: &Opts, audio: &[u8], name: &str, ctype: &str, deadline: Instant) -> Result<String, String> {
    let hinted = !o.keywords.is_empty() || !o.languages.is_empty();
    let (mut status, mut body) = post(o, audio, name, ctype, hinted, deadline)?;
    if status == 400 && hinted && about_hints(&body) {
        let cut: String = body.chars().take(200).collect();
        crate::out::log(&format!("retrying without keywords/languages: {cut}"));
        (status, body) = post(o, audio, name, ctype, false, deadline)?;
    }
    if !(200..300).contains(&status) {
        return Err(rest_error(status, &body));
    }
    let v: Value = serde_json::from_str(&body).map_err(|_| "OpenAI answered with no transcript".to_string())?;
    Ok(text_of(v.get("text")).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(base_url(None), "https://api.openai.com/v1");
        assert_eq!(base_url(Some("".into())), "https://api.openai.com/v1");
        assert_eq!(base_url(Some("http://127.0.0.1:9/v1//".into())), "http://127.0.0.1:9/v1");
        assert_eq!(ws_url("https://api.openai.com/v1"), "wss://api.openai.com/v1/realtime?intent=transcription");
        assert_eq!(ws_url("http://127.0.0.1:9/v1"), "ws://127.0.0.1:9/v1/realtime?intent=transcription");
    }

    #[test]
    fn the_session_update_leaves_out_what_is_empty() {
        let o = Opts { delay: "low".into(), ..Default::default() };
        let u = session_update(&o);
        assert_eq!(u["session"]["audio"]["input"]["transcription"], json!({"model": LIVE_MODEL, "delay": "low"}));
        assert!(u["session"]["audio"]["input"]["turn_detection"].is_null());
        assert_eq!(u["session"]["audio"]["input"]["format"], json!({"type": "audio/pcm", "rate": 24000}));
    }

    #[test]
    fn rest_errors_read_as_they_did() {
        assert_eq!(rest_error(401, ""), "OpenAI rejected the API key (401)");
        assert_eq!(rest_error(500, "oops"), "OpenAI answered 500");
        assert_eq!(rest_error(400, r#"{"error":{"message":"bad"}}"#), "OpenAI answered 400: bad");
    }

    #[test]
    fn hints_are_recognised_in_any_case() {
        assert!(about_hints("Unrecognized request argument supplied: Keywords"));
        assert!(about_hints("LANGUAGE"));
        assert!(!about_hints("file too large"));
    }

    #[test]
    fn transcripts_go_in_commit_order_stragglers_last() {
        let order = vec!["a".to_string(), "b".to_string()];
        let texts = vec![("c".to_string(), "three".to_string()), ("b".to_string(), " two ".to_string()), ("a".to_string(), "one".to_string()), ("d".to_string(), " ".to_string())];
        assert_eq!(joined(&order, &texts), "one two three");
    }
}
