//! How a session ends: a `stop` line on stdin, stdin's end, SIGTERM or SIGINT
//! (all alike), a limit, or a failure.

use crate::out::emit;
use serde_json::json;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;

#[derive(Clone)]
pub struct Stop {
    once: Arc<Mutex<bool>>,
    tx: Arc<watch::Sender<bool>>,
    announce: bool,
}

impl Stop {
    /// `announce`: print `stopping` when it happens.
    pub fn new(announce: bool) -> Self {
        Stop { once: Arc::new(Mutex::new(false)), tx: Arc::new(watch::channel(false).0), announce }
    }

    pub fn stop(&self) {
        let mut once = self.once.lock().unwrap();
        if *once {
            return;
        }
        *once = true;
        // before anyone can see it: `stopping` comes ahead of `end`
        if self.announce {
            emit(json!({"event": "stopping"}));
        }
        self.tx.send_replace(true);
    }

    pub fn stopped(&self) -> bool {
        *self.tx.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }
}

/// Resolves once stopped; safe to drop and call again.
pub async fn stopped(rx: &mut watch::Receiver<bool>) {
    let _ = rx.wait_for(|v| *v).await;
}

pub fn on_signals(stop: Stop) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let (Ok(mut term), Ok(mut int)) = (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) else { return };
            loop {
                tokio::select! {
                    _ = term.recv() => stop.stop(),
                    _ = int.recv() => stop.stop(),
                }
            }
        }
        #[cfg(not(unix))]
        while tokio::signal::ctrl_c().await.is_ok() {
            stop.stop();
        }
    });
}

pub fn after(stop: Stop, limit: Duration) {
    tokio::spawn(async move {
        tokio::time::sleep(limit).await;
        stop.stop();
    });
}

/// Does any complete line of BUF say `stop`? Returns the part after the last
/// newline, still to be finished.
pub fn stop_line(buf: &[u8]) -> (bool, Vec<u8>) {
    let cut = buf.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let said = buf[..cut].split(|b| *b == b'\n').any(|l| String::from_utf8_lossy(l).trim() == "stop");
    (said, buf[cut..].to_vec())
}

/// "stop" on stdin, or stdin's end, stops a session.
pub fn watch_stdin(stop: Stop) {
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut b = [0u8; 4096];
        let mut buf = vec![];
        loop {
            match stdin.read(&mut b) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    buf.extend_from_slice(&b[..n]);
                    let (said, rest) = stop_line(&buf);
                    buf = rest;
                    if said {
                        break;
                    }
                }
            }
        }
        stop.stop();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_line_counts_only_when_whole() {
        assert_eq!(stop_line(b"stop"), (false, b"stop".to_vec()));
        assert_eq!(stop_line(b"stop\n"), (true, vec![]));
        assert_eq!(stop_line(b"  stop \r\nmore"), (true, b"more".to_vec()));
        assert_eq!(stop_line(b"noise\nsto"), (false, b"sto".to_vec()));
    }
}
