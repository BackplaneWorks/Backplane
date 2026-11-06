//! Where audio comes from: pw-record, parecord, arecord (Linux tools), or
//! raw PCM on stdin.

use crate::out::log;
use crate::util::which;
use std::io::Read;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub const ORDER: [&str; 3] = ["pw-record", "parecord", "arecord"];
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
    let names: Vec<&str> = if source == "auto" { ORDER.to_vec() } else { vec![source] };
    if names.iter().any(|n| which(n).is_some()) {
        return None;
    }
    Some(if source == "auto" { "no recorder found (pw-record, parecord or arecord)".to_string() } else { format!("{source} not found") })
}

struct Proc {
    pid: u32,
    exited: Arc<AtomicBool>,
    code: JoinHandle<i32>,
    stderr: JoinHandle<String>,
}

pub struct Source {
    pub name: String,
    rx: mpsc::Receiver<Vec<u8>>,
    first: Option<Vec<u8>>,
    proc: Option<Proc>,
}

impl Source {
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
        self.proc.is_none()
    }

    pub fn kill(&self) {
        #[cfg(unix)]
        if let Some(p) = &self.proc {
            if !p.exited.load(Ordering::SeqCst) {
                unsafe { libc::kill(p.pid as i32, libc::SIGTERM) };
            }
        }
    }

    /// Why the recorder ended: its last line on stderr, else its exit code.
    pub async fn why(&mut self) -> String {
        let Some(p) = self.proc.take() else { return "stdin closed".to_string() };
        let err = p.stderr.await.unwrap_or_default();
        let line = err.trim().rsplit('\n').next().unwrap_or("").trim().to_string();
        if !line.is_empty() {
            return format!("{}: {line}", self.name);
        }
        format!("{}: exit {}", self.name, p.code.await.unwrap_or(-1))
    }
}

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
    let mut s = Source { name: name.to_string(), rx: mpsc::channel(1).1, first: None, proc: Some(Proc { pid, exited, code, stderr }) };
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
    Source { name: "stdin".to_string(), rx, first: None, proc: None }
}

/// The source to read; Err is the message to give up with.
pub async fn open_source(which: &str, device: &str, rate: u32) -> Result<Source, String> {
    if which == "stdin" {
        return Ok(stdin_source());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_command_lines() {
        assert_eq!(recorder_args("pw-record", "", 24000).join(" "), "--raw --rate 24000 --channels 1 --format s16 -");
        assert_eq!(recorder_args("pw-record", "mic2", 16000).join(" "), "--raw --rate 16000 --channels 1 --format s16 --target mic2 -");
        assert_eq!(recorder_args("parecord", "m", 24000).join(" "), "--raw --rate=24000 --channels=1 --format=s16le --device=m");
        assert_eq!(recorder_args("arecord", "hw:1", 16000).join(" "), "-q -t raw -f S16_LE -r 16000 -c 1 -D hw:1");
        assert_eq!(recorder_args("arecord", "", 16000).join(" "), "-q -t raw -f S16_LE -r 16000 -c 1");
    }
}
