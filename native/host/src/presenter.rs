//! The presenter: the one part of the hub that draws pictures. Behind a trait so it can be
//! replaced screen by screen. `BpPolicy` is the native Bend process (native/bend/slim_serve.bend,
//! "bp-policy"): wire frames in (F), one picture out per request (V). No JS anywhere.
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{future::Future, path::Path, pin::Pin, process::Stdio, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Child;
use tokio::sync::Mutex;

pub type Fut<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub trait Presenter: Send + Sync {
    fn name(&self) -> &'static str;
    /// Fold one hub wire frame (log, changes, delta, info, live...) into the presenter's state.
    fn fold<'a>(&'a self, frame: &'a Value) -> Fut<'a, ()>;
    /// view.open / view.update / view.action parameters -> {remoteReady, shown|nextTarget, session, cmds}
    fn view<'a>(&'a self, params: &'a Value) -> Fut<'a, Value>;
    fn ping(&self) -> Fut<'_, ()>;
    /// Whether the presenter wants the hub's log folded into it at start.
    fn wants_log(&self) -> bool { true }
    /// The presenter lost its state (its process died): `restart` it, then the hub folds the log into it again.
    fn is_down(&self) -> bool { false }
    fn restart(&self) -> Fut<'_, ()> { Box::pin(async { Ok(()) }) }
}

/// JSON text made pure ASCII: non-ASCII becomes \uXXXX (UTF-16 pairs), as the policy requires.
pub fn ascii_json(v: &Value) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if (c as u32) < 127 { out.push(c) } else {
            let mut b = [0u16; 2];
            for u in c.encode_utf16(&mut b) { out.push_str(&format!("\\u{:04x}", u)) }
        }
    }
    out
}

struct Link { read: BufReader<tokio::net::tcp::OwnedReadHalf>, write: tokio::net::tcp::OwnedWriteHalf }
/// The policy child. When it dies (kill -9, a crash, EOF) the link is dropped, every call answers "policy down" (the hub answers
/// PRESENTER_UNAVAILABLE, retryable), and the hub's supervisor calls `restart`, then folds the whole log into the new child from the
/// writer task (Hub::resync_presenter) so it holds exactly what the old one did.
pub struct BpPolicy { bin: std::path::PathBuf, link: Mutex<Option<Link>>, child: std::sync::Mutex<Option<Child>>, down: std::sync::atomic::AtomicBool }

impl BpPolicy {
    pub async fn spawn(bin: &Path) -> Result<BpPolicy> {
        let (link, child) = Self::start(bin).await?;
        Ok(BpPolicy { bin: bin.to_path_buf(), link: Mutex::new(Some(link)), child: std::sync::Mutex::new(Some(child)), down: false.into() })
    }
    async fn start(bin: &Path) -> Result<(Link, Child)> {
        let port = { let l = std::net::TcpListener::bind("127.0.0.1:0")?; l.local_addr()?.port() };
        let mut child = crate::proc::tokio_cmd(crate::proc::Kind::Local, bin).arg(port.to_string()).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true)
            .spawn().with_context(|| format!("start {}", bin.display()))?;
        let mut out = BufReader::new(child.stdout.take().context("policy stdout")?);
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(60), out.read_line(&mut line)).await.context("policy start timed out")??;
        if !line.contains("listening") { bail!("policy said {line:?}") }
        tokio::spawn(async move { let mut sink = String::new(); while out.read_line(&mut sink).await.map(|n| n > 0).unwrap_or(false) { sink.clear() } });
        let mut last = None;
        for _ in 0..50 {
            match TcpStream::connect(("127.0.0.1", port)).await { Ok(s) => { last = Some(s); break } Err(_) => tokio::time::sleep(Duration::from_millis(100)).await }
        }
        let (r, w) = last.context("connect to policy")?.into_split();
        s_nodelay(&w);
        Ok((Link { read: BufReader::new(r), write: w }, child))
    }
    async fn call(&self, op: &str, body: &str) -> Result<String> {
        let mut g = self.link.lock().await;
        let Some(l) = g.as_mut() else { bail!("policy down") };
        let r: Result<String> = async {
            l.write.write_all(format!("{op} {body}\n").as_bytes()).await?;
            let mut line = String::new();
            let n = tokio::time::timeout(Duration::from_secs(120), l.read.read_line(&mut line)).await.context("policy timed out")??;
            if n == 0 { bail!("policy closed") }
            Ok(line.trim_end().to_string())
        }.await;
        if r.is_err() { *g = None; self.down.store(true, std::sync::atomic::Ordering::Relaxed); } // the next call finds it down; the supervisor restarts it
        r
    }
}
fn s_nodelay(_w: &tokio::net::tcp::OwnedWriteHalf) {}

impl Presenter for BpPolicy {
    fn name(&self) -> &'static str { "bp-policy (native Bend)" }
    fn fold<'a>(&'a self, frame: &'a Value) -> Fut<'a, ()> {
        Box::pin(async move { let r = self.call("F", &ascii_json(frame)).await?; if r != "ok" { bail!("policy fold: {r}") } Ok(()) })
    }
    fn view<'a>(&'a self, params: &'a Value) -> Fut<'a, Value> {
        Box::pin(async move { let r = self.call("V", &ascii_json(params)).await?; Ok(serde_json::from_str(&r).with_context(|| format!("policy reply: {}", &r[..r.len().min(200)]))?) })
    }
    fn ping(&self) -> Fut<'_, ()> { Box::pin(async move { if self.call("P", "").await? != "pong" { bail!("no pong") } Ok(()) }) }
    fn is_down(&self) -> bool { self.down.load(std::sync::atomic::Ordering::Relaxed) }
    fn restart(&self) -> Fut<'_, ()> {
        Box::pin(async move {
            if let Some(mut old) = self.child.lock().unwrap().take() { let _ = old.start_kill(); }
            let (link, child) = Self::start(&self.bin).await?;
            *self.child.lock().unwrap() = Some(child);
            *self.link.lock().await = Some(link);
            self.down.store(false, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        })
    }
}

/// A presenter with canned answers, for tests and for running the hub without the policy binary.
pub struct Canned { pub reply: Value }
impl Presenter for Canned {
    fn name(&self) -> &'static str { "canned" }
    fn wants_log(&self) -> bool { false }
    fn fold<'a>(&'a self, _f: &'a Value) -> Fut<'a, ()> { Box::pin(async { Ok(()) }) }
    fn view<'a>(&'a self, _p: &'a Value) -> Fut<'a, Value> { let r = self.reply.clone(); Box::pin(async move { Ok(r) }) }
    fn ping(&self) -> Fut<'_, ()> { Box::pin(async { Ok(()) }) }
}

#[cfg(test)]
mod tests {
    use super::*; use serde_json::json;
    #[test] fn ascii_only_with_surrogate_pairs() {
        let t = ascii_json(&json!({"a":"é日🙂"})); assert!(t.is_ascii()); assert_eq!(serde_json::from_str::<Value>(&t).unwrap(), json!({"a":"é日🙂"}));
    }
}
