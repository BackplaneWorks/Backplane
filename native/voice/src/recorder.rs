//! Where audio comes from: WASAPI on Windows, pw-record / parecord / arecord
//! on Linux, or raw PCM on stdin.

#[cfg(not(windows))]
use crate::out::log;
#[cfg(not(windows))]
use crate::util::which;
use std::io::Read;
#[cfg(not(windows))]
use std::process::Stdio;
#[cfg(not(windows))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(windows))]
use std::sync::Arc;
#[cfg(not(windows))]
use std::time::Duration;
#[cfg(not(windows))]
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, oneshot};
#[cfg(not(windows))]
use tokio::task::JoinHandle;

pub const ORDER: [&str; 3] = ["pw-record", "parecord", "arecord"];
#[cfg(not(windows))]
const OPEN_GRACE: Duration = Duration::from_millis(500); // a recorder alive this long with no audio yet counts as started

pub fn recorder_args(name: &str, device: &str, rate: u32) -> Vec<String> {
    let mut v: Vec<String> = match name {
        "pw-record" => ["--raw", "--rate", &rate.to_string(), "--channels", "1", "--format", "s16"].map(String::from).to_vec(),
        "parecord" => ["--raw".to_string(), format!("--rate={rate}"), "--channels=1".to_string(), "--format=s16le".to_string()].to_vec(),
        _ => ["-q", "-t", "raw", "-f", "S16_LE", "-r", &rate.to_string(), "-c", "1"].map(String::from).to_vec(),
    };
    if !device.is_empty() {
        match name {
            "pw-record" => v.extend(["--target".to_string(), device.to_string()]),
            "parecord" => v.push(format!("--device={device}")),
            _ => v.extend(["-D".to_string(), device.to_string()]),
        }
    }
    if name == "pw-record" {
        v.push("-".to_string());
    }
    v
}

/// None when SOURCE can be tried; else why not.
pub fn missing(source: &str) -> Option<String> {
    if source == "stdin" {
        return None;
    }
    #[cfg(windows)]
    {
        return match source {
            "auto" | "wasapi" => None,
            _ => Some(format!("{source} is not supported on Windows (use auto or wasapi)")),
        };
    }
    #[cfg(not(windows))]
    {
        if source == "wasapi" {
            return Some("wasapi is only available on Windows".to_string());
        }
        let names: Vec<&str> = if source == "auto" { ORDER.to_vec() } else { vec![source] };
        if names.iter().any(|n| which(n).is_some()) {
            return None;
        }
        Some(if source == "auto" { "no recorder found (pw-record, parecord or arecord)".to_string() } else { format!("{source} not found") })
    }
}

#[cfg(not(windows))]
struct Proc {
    pid: u32,
    exited: Arc<AtomicBool>,
    code: JoinHandle<i32>,
    stderr: JoinHandle<String>,
}

#[cfg_attr(not(windows), allow(dead_code))]
struct Native {
    stop: Box<dyn Fn() + Send + Sync>,
    why: Option<oneshot::Receiver<String>>,
}

enum Backend {
    Stdin,
    #[cfg(not(windows))]
    Proc(Proc),
    #[cfg_attr(not(windows), allow(dead_code))]
    Native(Native),
}

pub struct Source {
    pub name: String,
    rx: mpsc::Receiver<Vec<u8>>,
    first: Option<Vec<u8>>,
    backend: Backend,
}

impl Source {
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn native(
        name: impl Into<String>,
        rx: mpsc::Receiver<Vec<u8>>,
        stop: impl Fn() + Send + Sync + 'static,
        why: oneshot::Receiver<String>,
    ) -> Source {
        Source {
            name: name.into(),
            rx,
            first: None,
            backend: Backend::Native(Native { stop: Box::new(stop), why: Some(why) }),
        }
    }

    /// The next piece of audio; None at its end. Safe to cancel.
    pub async fn next(&mut self) -> Option<Vec<u8>> {
        if let Some(b) = self.first.take() {
            return Some(b);
        }
        self.rx.recv().await
    }

    /// What is already here, without waiting.
    pub fn ready(&mut self) -> Option<Vec<u8>> {
        self.first.take().or_else(|| self.rx.try_recv().ok())
    }

    pub fn is_stdin(&self) -> bool {
        matches!(self.backend, Backend::Stdin)
    }

    pub fn kill(&self) {
        match &self.backend {
            Backend::Stdin => {}
            #[cfg(not(windows))]
            Backend::Proc(p) => {
                #[cfg(unix)]
                if !p.exited.load(Ordering::SeqCst) {
                    unsafe { libc::kill(p.pid as i32, libc::SIGTERM) };
                }
            }
            Backend::Native(n) => (n.stop)(),
        }
    }

    /// Why the recorder ended: its last line on stderr, else its exit code or backend error.
    pub async fn why(&mut self) -> String {
        match &mut self.backend {
            Backend::Stdin => "stdin closed".to_string(),
            #[cfg(not(windows))]
            Backend::Proc(_) => {
                let Backend::Proc(p) = std::mem::replace(&mut self.backend, Backend::Stdin) else {
                    unreachable!();
                };
                let err = p.stderr.await.unwrap_or_default();
                let line = err.trim().rsplit('\n').next().unwrap_or("").trim().to_string();
                if !line.is_empty() {
                    return format!("{}: {line}", self.name);
                }
                format!("{}: exit {}", self.name, p.code.await.unwrap_or(-1))
            }
            Backend::Native(n) => {
                let reason = match n.why.take() {
                    Some(rx) => rx.await.unwrap_or_default(),
                    None => String::new(),
                };
                if reason.is_empty() {
                    format!("{}: stopped", self.name)
                } else {
                    format!("{}: {reason}", self.name)
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn code_of(s: std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = s.signal() {
            return 128 + sig;
        }
    }
    s.code().unwrap_or(-1)
}

#[cfg(not(windows))]
async fn recorder(name: &str, device: &str, rate: u32) -> Result<Source, String> {
    let Some(exe) = which(name) else { return Err(format!("{name} not found")) };
    let mut child = tokio::process::Command::new(exe)
        .args(recorder_args(name, device, rate))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{name}: {e}"))?;
    let pid = child.id().unwrap_or(0);
    let mut out = child.stdout.take().ok_or("no stdout")?;
    let mut err = child.stderr.take().ok_or("no stderr")?;
    let stderr = tokio::spawn(async move {
        let mut b = vec![];
        let _ = err.read_to_end(&mut b).await;
        String::from_utf8_lossy(&b).into_owned()
    });
    let exited = Arc::new(AtomicBool::new(false));
    let flag = exited.clone();
    let code = tokio::spawn(async move {
        let s = child.wait().await;
        flag.store(true, Ordering::SeqCst);
        s.map_or(-1, code_of)
    });
    let (tx, mut rx) = mpsc::channel(256);
    tokio::spawn(async move {
        let mut b = vec![0u8; 16 * 1024];
        while let Ok(n) = out.read(&mut b).await {
            if n == 0 || tx.send(b[..n].to_vec()).await.is_err() {
                break;
            }
        }
    });
    let mut s = Source {
        name: name.to_string(),
        rx: mpsc::channel(1).1,
        first: None,
        backend: Backend::Proc(Proc { pid, exited, code, stderr }),
    };
    tokio::select! {
        r = rx.recv() => match r {
            Some(b) => s.first = Some(b),
            None => return Err(s.why().await),
        },
        _ = tokio::time::sleep(OPEN_GRACE) => {}
    }
    s.rx = rx;
    Ok(s)
}

fn stdin_source() -> Source {
    let (tx, rx) = mpsc::channel(256);
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut b = vec![0u8; 16 * 1024];
        while let Ok(n) = stdin.read(&mut b) {
            if n == 0 || tx.blocking_send(b[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    Source { name: "stdin".to_string(), rx, first: None, backend: Backend::Stdin }
}

/// The source to read; Err is the message to give up with.
pub async fn open_source(which: &str, device: &str, rate: u32) -> Result<Source, String> {
    if which == "stdin" {
        return Ok(stdin_source());
    }
    #[cfg(windows)]
    {
        return match which {
            "auto" | "wasapi" => crate::wasapi::open(device, rate).await.map_err(|e| format!("could not record from the microphone ({e})")),
            _ => Err(format!("{which} is not supported on Windows (use auto or wasapi)")),
        };
    }
    #[cfg(not(windows))]
    {
        if which == "wasapi" {
            return Err("wasapi is only available on Windows".to_string());
        }
        let names: Vec<&str> = if which == "auto" { ORDER.to_vec() } else { vec![which] };
        let mut whys: Vec<String> = vec![];
        for n in names {
            match recorder(n, device, rate).await {
                Ok(s) => return Ok(s),
                Err(w) => {
                    log(&w);
                    whys.push(w);
                }
            }
        }
        let told: Vec<String> = whys.into_iter().filter(|w| !w.ends_with("not found")).collect();
        Err(format!("could not record from the microphone ({})", told.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn recorder_command_lines() {
        assert_eq!(recorder_args("pw-record", "", 24000).join(" "), "--raw --rate 24000 --channels 1 --format s16 -");
        assert_eq!(recorder_args("pw-record", "mic2", 16000).join(" "), "--raw --rate 16000 --channels 1 --format s16 --target mic2 -");
        assert_eq!(recorder_args("parecord", "m", 24000).join(" "), "--raw --rate=24000 --channels=1 --format=s16le --device=m");
        assert_eq!(recorder_args("arecord", "hw:1", 16000).join(" "), "-q -t raw -f S16_LE -r 16000 -c 1 -D hw:1");
        assert_eq!(recorder_args("arecord", "", 16000).join(" "), "-q -t raw -f S16_LE -r 16000 -c 1");
    }

    #[cfg(not(windows))]
    #[test]
    fn wasapi_source_rejected_on_non_windows() {
        assert_eq!(missing("wasapi").as_deref(), Some("wasapi is only available on Windows"));
    }

    #[tokio::test]
    async fn native_source_streams_stops_and_reports_reason() {
        let (tx, rx) = mpsc::channel(4);
        let (why_tx, why_rx) = oneshot::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_cb = stopped.clone();
        let mut src = Source::native(
            "wasapi",
            rx,
            move || stopped_cb.store(true, Ordering::SeqCst),
            why_rx,
        );
        assert!(!src.is_stdin());
        tx.send(vec![1, 2, 3, 4]).await.unwrap();
        assert_eq!(src.next().await, Some(vec![1, 2, 3, 4]));
        src.kill();
        assert!(stopped.load(Ordering::SeqCst));
        let _ = why_tx.send("microphone was disconnected".to_string());
        drop(tx);
        assert_eq!(src.next().await, None);
        assert_eq!(src.why().await, "wasapi: microphone was disconnected");
        assert_eq!(src.why().await, "wasapi: stopped");
    }
}
