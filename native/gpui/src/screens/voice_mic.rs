//! This machine's microphone for a hub on another machine (docs/voice.md, "Another machine"): `backplane-voice record` gives 16 kHz mono s16le on stdout, and it goes in order, in about
//! 200 ms pieces, to the address the hub named (`POST /voice/in?s=<key>`, the last with `&end=1`). A hub on this machine hears the microphone itself, so none of this runs.
use crate::Backplane as Win;
use std::{io::{Read, Write}, process::{Child, Command, Stdio}, sync::mpsc, time::Duration};

const PIECE: usize = 6400; // 200 ms

pub struct Upload { child: Child, path: mpsc::Sender<String> }

/// A hub reached on this machine records its own microphone.
pub fn hub_here(url: &str) -> bool { let h = url.split("//").nth(1).unwrap_or(url); let h = h.rsplit_once(':').map_or(h, |(a, _)| a); matches!(h, "127.0.0.1" | "localhost" | "[::1]") }
fn helper() -> Option<std::path::PathBuf> { std::env::var_os("BACKPLANE_VOICE_BIN").map(Into::into).or_else(|| { let b = std::env::current_exe().ok()?.parent()?.join("backplane-voice"); b.exists().then_some(b) }) }
/// One POST; the hub closes after its reply.
fn post(addr: &str, target: &str, token: &str, body: &[u8]) -> bool {
    let Ok(mut c) = std::net::TcpStream::connect(addr) else { return false };
    let _ = c.set_read_timeout(Some(Duration::from_secs(10)));
    /* i18n-ok: an HTTP request */ let head = format!("POST {target} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    if c.write_all(head.as_bytes()).and_then(|()| c.write_all(body)).is_err() { return false }
    let mut r = [0u8; 16]; c.read(&mut r).map_or(false, |n| n >= 12 && &r[9..12] == b"200")
}
impl Win {
    /// Starts this machine's recorder when the hub is elsewhere: what is heard before the address comes waits for it. False: the hub records.
    pub fn voice_mic_begin(&mut self) -> bool {
        let Some(t) = self.screens.attacher.target.clone() else { return false };
        if hub_here(&format!("ws://{}", t.addr)) { return false }
        let Some(bin) = helper() else { return false };
        let Ok(mut child) = Command::new(bin).args(["record", "--rate", "16000"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else { return false };
        let Some(mut out) = child.stdout.take() else { return false };
        let (tx, rx) = mpsc::channel::<String>(); let (addr, token) = (t.addr.clone(), t.token.clone());
        std::thread::spawn(move || {
            let (mut held, mut buf, mut path): (Vec<u8>, [u8; PIECE], Option<String>) = (vec![], [0; PIECE], None);
            loop {
                let n = out.read(&mut buf).unwrap_or(0);
                if path.is_none() { path = rx.try_recv().ok() }
                held.extend_from_slice(&buf[..n]);
                if n == 0 { if let Some(p) = &path { post(&addr, &format!("{p}&end=1"), &token, &held); } return }
                if let Some(p) = &path { if held.len() >= PIECE { if !post(&addr, p, &token, &held) { return } held.clear(); } }
            }
        });
        *self.voice_upload() = Some(Upload { child, path: tx }); true
    }
    /// The hub's start reply: where the audio goes.
    pub fn voice_mic_send(&mut self, path: &str) { if let Some(u) = self.voice_upload() { let _ = u.path.send(path.to_string()); } }
    /// The person stopped: the recorder's end is the upload's end mark.
    pub fn voice_mic_end(&mut self) { if let Some(u) = self.voice_upload() { drop(u.child.stdin.take()); } }
    /// The session is over.
    pub fn voice_mic_stop(&mut self) { if let Some(mut u) = self.voice_upload().take() { drop(u.child.stdin.take()); std::thread::spawn(move || { std::thread::sleep(Duration::from_secs(2)); let _ = u.child.kill(); let _ = u.child.wait(); }); } }
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    #[test] fn which_hubs_are_here() { assert!(hub_here("ws://127.0.0.1:3799")); assert!(hub_here("ws://localhost:3799")); assert!(!hub_here("ws://100.64.84.41:3799")); assert!(!hub_here("ws://boat.tail.ts.net:3787")); }
}
