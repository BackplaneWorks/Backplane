//! Attachments: paste, drop and the file picker upload a file with one `POST /attach` to the thread's hub (the
//! phones' way) and report the result as the view actions `attach-up` and `attach-end`. The hub keeps the
//! list; the composer draws the hub's `thread.attaching` chips, with the picture we hold for a preview,
//! and Remove is the action `detach`. Sending is the normal `send`: the hub adds the attached paths.
use super::composer::ComposerEvent;
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::{collections::HashMap, io::{Read, Write}, net::TcpStream, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};
use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub struct Target { pub addr: String, pub token: String, pub tls: bool }

impl Target {
    /// `ws://host:port` (or wss://, a hub behind `tailscale serve`) and the token file's contents.
    pub fn from(hub: &str, token_file: &Path) -> Option<Self> {
        if !["ws://", "wss://", "http://", "https://"].iter().any(|p| hub.starts_with(p)) { return None }
        let (addr, tls) = crate::pairing::host_port(hub);
        let token = std::fs::read_to_string(token_file).ok()?.trim().to_string();
        (token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && !token.is_empty()).then_some(Self { addr, token, tls })
    }
    /// A connection for one direct request.
    pub fn stream(&self) -> std::io::Result<crate::pairing::Stream> { crate::pairing::open(&self.addr, self.tls) }
    /// The hub's address as a URL.
    pub fn base(&self) -> String { format!("{}://{}", if self.tls { "https" } else { "http" }, self.addr) }
}

/// Percent-encode a query value.
pub fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// The request head of a POST /attach.
pub fn head(t: &Target, thread: &str, key: &str, name: &str, len: u64) -> String {
    format!("POST /attach?thread={}&key={}&name={}&token={} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        enc(thread), enc(key), enc(name), t.token, t.addr, len)
}

/// The body of an HTTP response, de-chunked when the reply is `Transfer-Encoding: chunked`.
pub fn body_of(response: &str) -> String {
    let Some((head, body)) = response.split_once("\r\n\r\n") else { return String::new() };
    if !head.to_ascii_lowercase().contains("transfer-encoding: chunked") { return body.to_string(); }
    let (mut out, mut rest) = (String::new(), body);
    while let Some((size, tail)) = rest.split_once("\r\n") {
        let Ok(n) = usize::from_str_radix(size.split(';').next().unwrap_or("").trim(), 16) else { break };
        if n == 0 || tail.len() < n || !tail.is_char_boundary(n) { break; }
        out.push_str(&tail[..n]); rest = tail[n..].strip_prefix("\r\n").unwrap_or(&tail[n..]);
    }
    out
}

/// Read one HTTP response, stopping as soon as it is complete: by its Content-Length, by the last chunk, or at the end of the
/// stream. A hub that answers and keeps the connection open must not hold the upload (and the attach-end) for the read timeout.
pub fn read_response(s: &mut impl Read) -> std::io::Result<Vec<u8>> {
    let (mut buf, mut chunk) = (Vec::new(), [0u8; 4096]);
    loop {
        if complete(&buf) { return Ok(buf); }
        if buf.len() > (1 << 20) { return Ok(buf); }
        match s.read(&mut chunk) { Ok(0) => return Ok(buf), Ok(n) => buf.extend_from_slice(&chunk[..n]), Err(e) if !buf.is_empty() && matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return Ok(buf), Err(e) => return Err(e) }
    }
}
fn complete(buf: &[u8]) -> bool {
    let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") else { return false };
    let head = String::from_utf8_lossy(&buf[..at]).to_ascii_lowercase();
    let body = &buf[at + 4..];
    if head.contains("transfer-encoding: chunked") { return body.ends_with(b"0\r\n\r\n"); }
    match head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse::<usize>().ok()) { Some(n) => body.len() >= n, None => false }
}

pub enum Body { Bytes(Vec<u8>), File(PathBuf) }

pub const DEFAULT_CAP: u64 = 536_870_912;
pub const MAX_FILES: usize = 8;
const PREVIEW_MAX: u64 = 8 << 20;

/// The thread on screen, shared with upload threads: a finished upload is reported only if its thread still is.
pub type Current = Arc<Mutex<String>>;

/// Runs on a thread of its own, never the UI's: announce, upload, report. Like the phones: `attach-up` first,
/// nothing sent over the thread's cap (the hub's engine words the refusal), `attach-end` only while the same
/// thread is on screen. Blocking sends: a full queue waits, it does not lose the end.
pub fn upload_blocking(tx: &Sender<Command>, t: &Target, thread: &str, name: &str, body: Body, cap: u64, current: &Current) {
    let size = match &body { Body::Bytes(b) => b.len() as u64, Body::File(p) => std::fs::metadata(p).map_or(0, |m| m.len()) };
    if size == 0 { return; }
    let _ = tx.blocking_send(Command::Action("attach-up".into(), json!({"name": name, "size": size}).to_string()));
    if size > cap { return; }
    let key = format!("{:08x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u32));
    let run = || -> std::io::Result<String> {
        let mut s = t.stream()?; s.set_timeouts(Duration::from_secs(120))?;
        s.write_all(head(t, thread, &key, name, size).as_bytes())?;
        match body {
            Body::Bytes(b) => s.write_all(&b)?,
            // exactly `size` bytes: a file that grows or shrinks must not break the Content-Length
            Body::File(p) => { let n = std::io::copy(&mut std::fs::File::open(p)?.take(size), &mut s)?; if n != size { return Err(std::io::ErrorKind::UnexpectedEof.into()); } }
        }
        read_response(&mut s).map(|b| String::from_utf8_lossy(&b).into_owned())
    };
    let answer = run().ok().map(|r| body_of(&r)).filter(|b| serde_json::from_str::<Value>(b).is_ok())
        .unwrap_or_else(|| json!({"uiError": "upload.network", "thread": thread, "upload": true}).to_string());
    let on_screen = current.lock().map_or(false, |c| *c == thread);
    let sent = on_screen && tx.blocking_send(Command::Action("attach-end".into(), answer.clone())).is_ok();
    // what happened to the upload's end, without the path's directory (a diagnostic line for the harness)
    eprintln!("GPUI_LOG {{\"attach_end\":{{\"sent\":{sent},\"thread_current\":{on_screen},\"answer_bytes\":{},\"ok\":{}}}}}", answer.len(), serde_json::from_str::<Value>(&answer).map_or(false, |v| v["attached"].is_string()));
}

pub fn upload(tx: Sender<Command>, t: Target, thread: String, name: String, body: Body, cap: u64, current: Current) {
    std::thread::spawn(move || upload_blocking(&tx, &t, &thread, &name, body, cap, &current));
}

/// Previews of what was just attached, by file name (the chips come from the hub). Shared with the upload threads.
#[derive(Default, Clone)]
pub struct Previews { by_name: Arc<Mutex<HashMap<String, Arc<Image>>>> }
impl Previews {
    pub fn add(&self, name: &str, ext: &str, bytes: &[u8]) {
        let format = match ext { "png" => ImageFormat::Png, "jpg" | "jpeg" => ImageFormat::Jpeg, "gif" => ImageFormat::Gif, "webp" => ImageFormat::Webp, _ => return };
        if bytes.len() as u64 > PREVIEW_MAX { return; }
        let Ok(mut m) = self.by_name.lock() else { return };
        if m.len() >= 16 { m.clear(); }
        m.insert(name.to_string(), Arc::new(Image::from_bytes(format, bytes.to_vec())));
    }
    pub fn get(&self, name: &str) -> Option<Arc<Image>> { self.by_name.lock().ok()?.get(name).cloned() }
}

/// A thread's own id: what follows the last `|` of a qualified one (no id contains `|`).
pub fn raw_id(id: &str) -> &str { id.rsplit('|').next().unwrap_or(id) }

pub fn ext_of_name(name: &str) -> String { name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default() }

/// Everything attaching needs; one call from main.rs per composer event.
pub struct Attacher { pub target: Option<Target>, pub previews: Previews, pub current: Current }
impl Attacher {
    pub fn new(target: Option<Target>) -> Self { Self { target, previews: Previews::default(), current: Arc::default() } }
    pub fn show(&self, thread: &str) { if let Ok(mut c) = self.current.lock() { *c = raw_id(thread).to_string(); } }
    /// Whether something can be attached to `thread` now; Err is a sentence for the error line.
    pub fn ready(&self, thread: &str) -> Result<(), String> {
        if self.target.is_none() { return Err("Attachments need a hub on plain http://; this one is not.".into()); }
        if thread.is_empty() { return Err("Open a thread to attach a file.".into()); }
        Ok(())
    }
    /// Start the upload(s) for a composer event. Err is a sentence for the error line; the UI thread reads no file.
    pub fn event(&self, tx: &Sender<Command>, thread: &str, cap: u64, ev: &ComposerEvent) -> Result<(), String> {
        self.ready(thread)?;
        let Some(t) = self.target.clone() else { return Ok(()) };
        // the hub's own id: a screen over several hubs names a thread "<hub>|<id>", and the hub (and `show`) know only the id
        let (tx, thread, previews, current) = (tx.clone(), raw_id(thread).to_string(), self.previews.clone(), self.current.clone());
        match ev {
            ComposerEvent::Image { bytes, ext } => {
                let (bytes, ext) = (bytes.clone(), *ext);
                std::thread::spawn(move || {
                    let name = format!("pasted-{}.{ext}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()));
                    previews.add(&name, ext, &bytes);
                    upload_blocking(&tx, &t, &thread, &name, Body::Bytes(bytes), cap, &current);
                });
                Ok(())
            }
            ComposerEvent::Drawing(rgba) => {
                let rgba = rgba.clone();
                std::thread::spawn(move || {
                    let Some(bytes) = super::pad::png(&rgba) else { return };
                    let name = format!("drawing-{}.png", super::pad::stamp());
                    previews.add(&name, "png", &bytes);
                    upload_blocking(&tx, &t, &thread, &name, Body::Bytes(bytes), cap, &current);
                });
                Ok(())
            }
            ComposerEvent::Files(paths) => {
                let skipped = paths.len().saturating_sub(MAX_FILES);
                let paths: Vec<PathBuf> = paths.iter().take(MAX_FILES).cloned().collect();
                std::thread::spawn(move || for p in paths {
                    let Ok(m) = std::fs::metadata(&p) else { continue };
                    if !m.is_file() { continue; }
                    let name = p.file_name().map_or("file".into(), |n| n.to_string_lossy().to_string());
                    let ext = ext_of_name(&name);
                    if m.len() <= PREVIEW_MAX && matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp") { if let Ok(b) = std::fs::read(&p) { previews.add(&name, &ext, &b); } }
                    upload_blocking(&tx, &t, &thread, &name, Body::File(p), cap, &current);
                });
                if skipped > 0 { Err(format!("Attached the first {MAX_FILES} files; {skipped} more were not.")) } else { Ok(()) }
            }
            ComposerEvent::Submit | ComposerEvent::SubmitBack | ComposerEvent::CopyTimeline | ComposerEvent::PopupMove(_) | ComposerEvent::PopupPick => Ok(()),
        }
    }
}

/// The chips above the composer: thumbnail (when we hold one) or a file mark, the name, and Remove.
pub fn chips(cx: &mut Context<Backplane>, light: bool, previews: &Previews, thread: &Value) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let items = thread["attaching"].as_array().cloned().unwrap_or_default();
    let uploading = string(thread, "uploading");
    if items.is_empty() && uploading.is_empty() { return None; }
    let mut row = div().id("att-row").max_h(px(140.)).overflow_y_scroll().flex().flex_wrap().gap_2();
    for (i, it) in items.iter().enumerate() {
        let (label, path) = (string(it, "label"), string(it, "path"));
        let thumb = previews.get(&label).filter(|_| it["image"].as_bool().unwrap_or(false));
        let mark = match thumb {
            Some(img) => div().size(px(64.)).flex_shrink_0().flex().items_center().justify_center().bg(c("raise")).child(gpui::img(ImageSource::Image(img)).size(px(64.)).object_fit(ObjectFit::Contain)).into_any_element(),
            None => div().size(px(40.)).flex_shrink_0().flex().items_center().justify_center().bg(c("raise")).text_size(px(10.)).text_color(c("dim")).child(if it["image"].as_bool().unwrap_or(false) { "IMG" } else { "FILE" }).into_any_element(),
        };
        row = row.child(div().id(("att", i)).flex().items_center().gap_2().pr_2().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.))
            .child(mark).child(div().max_w(px(180.)).overflow_hidden().whitespace_nowrap().child(label))
            .child(div().id(("att-x", i)).px_1().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("bad"))).child("×")
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("detach".into(), path.clone())); cx.notify(); }))));
    }
    if !uploading.is_empty() { row = row.child(div().px_2().py_1().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("dim")).child(format!("Uploading {uploading}…"))); }
    Some(row.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test; // gpui::* also exports a `test` macro
    /// The failure found in the capped attach run: the screen's thread id was qualified, `show` stored the other form, the upload's end was
    /// held back as "another thread" and the POST named a thread the hub does not know.
    #[test] fn a_qualified_thread_id_is_the_same_thread() {
        assert_eq!(raw_id("127.0.0.1:3834|t1"), "t1"); assert_eq!(raw_id("t1"), "t1"); assert_eq!(raw_id(""), "");
        let a = Attacher::new(None); a.show("127.0.0.1:3834|t1");
        assert_eq!(*a.current.lock().unwrap(), "t1");
        let b = Attacher::new(None); b.show("t1"); assert_eq!(*b.current.lock().unwrap(), *a.current.lock().unwrap());
    }
    /// Opening a bot's chat after a thread never ran `thread_switch`: `current` kept the old thread and the upload's end was dropped (`thread_current:false`),
    /// so a pasted picture was saved but never attached. Every picture now names the thread on screen (glue.rs's `picture_arrived`); `show` must follow it each time.
    #[test] fn the_current_thread_follows_every_show() {
        let a = Attacher::new(None); a.show("t-kit"); a.show("h:1|t-miso");
        assert_eq!(*a.current.lock().unwrap(), "t-miso");
        let done = |thread: &str| *a.current.lock().unwrap() == thread; // upload_blocking's test of whether its thread is still the one shown
        assert!(done("t-miso") && !done("t-kit"));
    }
    #[test] fn the_request_head_is_a_post_with_length_and_an_encoded_name() {
        let t = Target { addr: "127.0.0.1:3799".into(), token: "tok".into(), tls: false };
        let h = head(&t, "t1", "0a0b0c0d", "my pic (1).png", 42);
        assert!(h.starts_with("POST /attach?thread=t1&key=0a0b0c0d&name=my%20pic%20%281%29.png&token=tok HTTP/1.1\r\n"), "{h}");
        assert!(h.contains("Content-Length: 42\r\n") && h.ends_with("\r\n\r\n") && h.contains("Host: 127.0.0.1:3799"));
    }
    #[test] fn target_needs_a_clean_token() {
        let d = std::env::temp_dir().join(format!("gpui-att-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap();
        let f = d.join("token"); std::fs::write(&f, "abc-123\n").unwrap();
        assert_eq!(Target::from("ws://h:1/", &f).unwrap().addr, "h:1");
        std::fs::write(&f, "bad token&x").unwrap(); assert!(Target::from("ws://h:1", &f).is_none());
        let _ = std::fs::remove_dir_all(d);
    }
    #[test] fn answer_is_the_http_body() { assert_eq!(body_of("HTTP/1.1 200 OK\r\nA: b\r\n\r\n{\"attached\":\"/p\"}"), "{\"attached\":\"/p\"}"); }
    /// A hub that replies and then holds the connection open (HTTP keep-alive) must not stall the attach-end.
    #[test] fn the_reply_is_read_by_its_length_without_waiting_for_the_close() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let addr = l.local_addr().unwrap().to_string();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let server = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap(); let mut b = [0u8; 4096]; let _ = s.read(&mut b);
            let body = "{\"attached\":\"/p\"}"; write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
            let _ = done_rx.recv_timeout(Duration::from_secs(10));      // keeps the socket open
        });
        let mut s = TcpStream::connect(&addr).unwrap(); s.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap(); s.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
        let t0 = std::time::Instant::now(); let r = read_response(&mut s).unwrap();
        assert!(t0.elapsed() < Duration::from_secs(3), "waited for the close"); assert_eq!(body_of(&String::from_utf8_lossy(&r)), "{\"attached\":\"/p\"}");
        let _ = done_tx.send(()); server.join().unwrap();
    }
    #[test] fn completeness_by_length_chunks_and_partial_heads() {
        assert!(!complete(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nab")); assert!(complete(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nab"));
        assert!(complete(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\n\r\n")); assert!(!complete(b"HTTP/1.1 200 OK\r\nTransfer-Enc"));
    }
    #[test] fn a_chunked_answer_is_joined() {
        let r = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n7\r\n{\"att\":\r\n5\r\n\"/p\"}\r\n0\r\n\r\n";
        assert_eq!(body_of(r), "{\"att\":\"/p\"}".to_string());
    }
    /// A real round trip against a one-shot local server: the bytes after the head arrive intact.
    #[test] fn uploads_bytes_and_reports_attach_end() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let addr = l.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap(); let mut buf = Vec::new(); let mut chunk = [0u8; 4096];
            loop { let n = s.read(&mut chunk).unwrap(); buf.extend_from_slice(&chunk[..n]); if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") { if buf.len() >= p + 4 + 5 { break; } } }
            let text = String::from_utf8_lossy(&buf).to_string(); assert!(text.ends_with("hello"), "{text}");
            let body = "{\"attached\":\"/a/b.png\",\"thread\":\"t1\"}";
            write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let current: Current = Arc::new(Mutex::new("t1".into()));
        upload(tx, Target { addr, token: "t".into(), tls: false }, "t1".into(), "b.png".into(), Body::Bytes(b"hello".to_vec()), DEFAULT_CAP, current);
        let wait = |rx: &mut tokio::sync::mpsc::Receiver<Command>| { for _ in 0..200 { if let Ok(c) = rx.try_recv() { return c; } std::thread::sleep(Duration::from_millis(25)); } panic!("no command") };
        match wait(&mut rx) { Command::Action(a, v) => { assert_eq!(a, "attach-up"); assert!(v.contains("\"size\":5")); } c => panic!("{c:?}") }
        match wait(&mut rx) { Command::Action(a, v) => { assert_eq!(a, "attach-end"); assert!(v.contains("/a/b.png")); } c => panic!("{c:?}") }
        server.join().unwrap();
    }
    #[test] fn over_the_cap_announces_but_never_uploads_and_a_switch_drops_the_end() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let t = Target { addr: "127.0.0.1:1".into(), token: "t".into(), tls: false }; let cur: Current = Arc::new(Mutex::new("t1".into()));
        upload_blocking(&tx, &t, "t1", "big.bin", Body::Bytes(vec![1; 10]), 5, &cur);
        assert!(matches!(rx.try_recv(), Ok(Command::Action(a, _)) if a == "attach-up")); assert!(rx.try_recv().is_err(), "uploaded past the cap");
        *cur.lock().unwrap() = "t2".into();   // the person switched threads; the connection refuses, so it is the network error, but it is not delivered to t2
        upload_blocking(&tx, &t, "t1", "a.bin", Body::Bytes(vec![1; 3]), 100, &cur);
        assert!(matches!(rx.try_recv(), Ok(Command::Action(a, _)) if a == "attach-up")); assert!(rx.try_recv().is_err(), "attach-end leaked into another thread");
    }
    #[test] fn a_missing_target_or_thread_says_why_and_extra_files_are_counted() {
        let a = Attacher::new(None); let (tx, _rx) = tokio::sync::mpsc::channel(1);
        assert!(a.event(&tx, "t1", DEFAULT_CAP, &ComposerEvent::Files(vec![])).unwrap_err().contains("plain http"));
        let a = Attacher::new(Some(Target { addr: "127.0.0.1:1".into(), token: "t".into(), tls: false }));
        assert!(a.event(&tx, "", DEFAULT_CAP, &ComposerEvent::Files(vec![])).unwrap_err().contains("thread"));
        let many: Vec<PathBuf> = (0..11).map(|i| PathBuf::from(format!("/nonexistent/{i}"))).collect();
        assert!(a.event(&tx, "t1", DEFAULT_CAP, &ComposerEvent::Files(many)).unwrap_err().contains("3 more"));
    }
}
