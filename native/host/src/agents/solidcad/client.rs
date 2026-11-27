//! One `solidcraft-cli mcp` child and the JSON-RPC it speaks on stdio (one message per line). Replies are routed by id from a reader
//! task, so a caller that goes away (its turn interrupted) or runs out of time never leaves a half-read line for the next one: the late
//! reply is read and dropped. stderr is kept (its last lines) to say why a child died.
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Arc, Mutex as StdMutex}, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::{Child, ChildStdin}, sync::oneshot};

/// How a child is started: headless (its own design session) or bridged to the SolidCraft the pane runs (its control port).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode { Headless, Bridged { port: u16, token: String } }
impl Mode {
    pub fn word(&self) -> &'static str { match self { Mode::Headless => "headless", Mode::Bridged { .. } => "pane" } }
}

/// What to run. `program` is `solidcraft-cli`; a test names a stand-in.
#[derive(Clone, Debug)]
pub struct Spawn { pub program: PathBuf, pub mode: Mode, pub allow: Vec<PathBuf> }

/// The command line after the program (the token never rides here: other users can read a process list).
pub fn args(s: &Spawn) -> Vec<String> {
    let mut a = vec!["mcp".to_string()];
    match &s.mode { Mode::Headless => a.push("--headless".into()), Mode::Bridged { port, .. } => { a.push("--connect".into()); a.push(format!("127.0.0.1:{port}")) } }
    a.push("--compact".into());
    for p in &s.allow { a.push("--allow-path".into()); a.push(p.to_string_lossy().into_owned()) }
    a
}

#[derive(Debug, PartialEq)]
pub enum CallErr {
    /// no answer inside the budget: the child may still be working on it
    Timeout,
    /// the child is gone (stderr's last lines)
    Exited(String),
    /// JSON-RPC's own error (an unknown tool, arguments that fail the listed schema): nothing ran
    Rpc(String),
}

type Waiting = Arc<StdMutex<HashMap<u64, oneshot::Sender<Value>>>>;

pub struct Client {
    child: StdMutex<Option<Child>>,
    stdin: tokio::sync::Mutex<ChildStdin>,
    waiting: Waiting,
    next: AtomicU64,
    gone: Arc<AtomicBool>,
    stderr: Arc<StdMutex<String>>,
    pub mode: Mode,
    /// the server's `instructions` (what it says of itself at initialize)
    pub instructions: String,
}

const STDERR_KEEP: usize = 2000;
const MAX_LINE: usize = 64 << 20; // a screenshot's base64 rides in one line

impl Client {
    /// Starts the child and does the MCP handshake. Err: why it could not be started or did not answer.
    pub async fn start(s: &Spawn, init_budget: Duration) -> Result<Client, String> {
        let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, &s.program);
        c.args(args(s)).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
        // never a window on the person's screen from the hub's own environment: a headless session draws on a headless GPU device
        c.env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY");
        if let Mode::Bridged { token, .. } = &s.mode { c.env("SOLIDCRAFT_CONTROL_TOKEN", token); }
        let mut child = c.spawn().map_err(|e| format!("{} could not be started: {e}", s.program.display()))?;
        let (stdin, stdout, stderr) = (child.stdin.take().ok_or("no stdin")?, child.stdout.take().ok_or("no stdout")?, child.stderr.take().ok_or("no stderr")?);
        let waiting: Waiting = Default::default(); let gone = Arc::new(AtomicBool::new(false)); let errs = Arc::new(StdMutex::new(String::new()));
        { let (w, g) = (waiting.clone(), gone.clone());
          tokio::spawn(async move {
            let mut r = BufReader::new(stdout); let mut line = Vec::new();
            loop {
                line.clear();
                match (&mut r).take(MAX_LINE as u64).read_until(b'\n', &mut line).await { Ok(0) | Err(_) => break, Ok(_) => {} }
                let Ok(v) = serde_json::from_slice::<Value>(&line) else { continue };
                let Some(id) = v.get("id").and_then(Value::as_u64) else { continue }; // a notification
                if let Some(tx) = w.lock().unwrap().remove(&id) { let _ = tx.send(v); }
            }
            g.store(true, Ordering::SeqCst); w.lock().unwrap().clear(); // every waiter hears the hang-up
          }); }
        { let e = errs.clone();
          tokio::spawn(async move {
            let mut r = BufReader::new(stderr).lines();
            while let Ok(Some(l)) = r.next_line().await { let mut s = e.lock().unwrap(); s.push_str(&l); s.push('\n'); if s.len() > STDERR_KEEP { let cut = s.len() - STDERR_KEEP; let cut = (cut..s.len()).find(|i| s.is_char_boundary(*i)).unwrap_or(s.len()); s.drain(..cut); } }
          }); }
        let mut cl = Client { child: StdMutex::new(Some(child)), stdin: tokio::sync::Mutex::new(stdin), waiting, next: AtomicU64::new(1), gone, stderr: errs, mode: s.mode.clone(), instructions: String::new() };
        let init = cl.request("initialize", json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"backplane","version":env!("CARGO_PKG_VERSION")}}), init_budget).await
            .map_err(|e| format!("the SolidCraft MCP server did not start: {}", say(&e)))?;
        cl.instructions = init.pointer("/instructions").and_then(Value::as_str).unwrap_or("").to_string();
        cl.notify("notifications/initialized").await;
        Ok(cl)
    }

    async fn write(&self, v: &Value) -> Result<(), CallErr> {
        let mut line = v.to_string(); line.push('\n');
        let mut w = self.stdin.lock().await;
        if w.write_all(line.as_bytes()).await.is_err() || w.flush().await.is_err() { return Err(CallErr::Exited(self.stderr_tail())) }
        Ok(())
    }
    async fn notify(&self, method: &str) { let _ = self.write(&json!({"jsonrpc":"2.0","method":method})).await; }

    /// One request; its `result`. A reply that comes after the budget is dropped by the reader.
    pub async fn request(&self, method: &str, params: Value, budget: Duration) -> Result<Value, CallErr> {
        if self.gone.load(Ordering::SeqCst) { return Err(CallErr::Exited(self.stderr_tail())) }
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel(); self.waiting.lock().unwrap().insert(id, tx);
        let forget = Forget(&self.waiting, id); // a caller dropped mid-wait leaves no entry behind
        self.write(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})).await?;
        let got = tokio::time::timeout(budget, rx).await;
        drop(forget);
        match got {
            Err(_) => Err(CallErr::Timeout),
            Ok(Err(_)) => { tokio::time::sleep(Duration::from_millis(50)).await; Err(CallErr::Exited(self.stderr_tail())) } // stderr's last words land first
            Ok(Ok(v)) => match v.get("error") { Some(e) => Err(CallErr::Rpc(e.get("message").and_then(Value::as_str).unwrap_or("error").to_string())), None => Ok(v.get("result").cloned().unwrap_or(Value::Null)) },
        }
    }
    /// `tools/call`: the result object (`content`, `isError`).
    pub async fn call(&self, tool: &str, args: &Value, budget: Duration) -> Result<Value, CallErr> {
        let args = if args.is_object() { args.clone() } else { json!({}) };
        self.request("tools/call", json!({"name":tool,"arguments":args}), budget).await
    }
    pub async fn tools(&self, budget: Duration) -> Result<Vec<Value>, CallErr> {
        Ok(self.request("tools/list", json!({}), budget).await?.get("tools").and_then(Value::as_array).cloned().unwrap_or_default())
    }
    pub fn alive(&self) -> bool { !self.gone.load(Ordering::SeqCst) }
    pub fn stderr_tail(&self) -> String { let s = self.stderr.lock().unwrap(); let t: Vec<&str> = s.lines().rev().take(6).collect(); t.into_iter().rev().collect::<Vec<_>>().join("\n") }
    /// Kills the child (a stalled kernel never ends by itself) and reaps it.
    pub fn kill(&self) { if let Some(c) = self.child.lock().unwrap().take() { crate::proc::kill_and_reap(c) } self.gone.store(true, Ordering::SeqCst); self.waiting.lock().unwrap().clear(); }
}
impl Drop for Client { fn drop(&mut self) { if let Some(mut c) = self.child.lock().unwrap().take() { let _ = c.start_kill(); if tokio::runtime::Handle::try_current().is_ok() { crate::proc::kill_and_reap(c) } } } }

struct Forget<'a>(&'a Waiting, u64);
impl Drop for Forget<'_> { fn drop(&mut self) { self.0.lock().unwrap().remove(&self.1); } }

pub fn say(e: &CallErr) -> String {
    match e { CallErr::Timeout => "no answer in time".into(), CallErr::Exited(s) if s.trim().is_empty() => "the process ended".into(), CallErr::Exited(s) => format!("the process ended: {}", s.trim()), CallErr::Rpc(m) => m.clone() }
}

/// The first text block of a tool result, parsed as JSON when it is JSON.
pub fn first_json(result: &Value) -> Option<Value> {
    result.get("content")?.as_array()?.iter().find(|b| b["type"] == "text").and_then(|b| b["text"].as_str()).and_then(|t| serde_json::from_str(t).ok())
}
pub fn first_text(result: &Value) -> String {
    result.get("content").and_then(Value::as_array).and_then(|a| a.iter().find(|b| b["type"] == "text")).and_then(|b| b["text"].as_str()).unwrap_or("").to_string()
}
pub fn is_error(result: &Value) -> bool { result.get("isError").and_then(Value::as_bool).unwrap_or(false) }
