//! The REQ side of nng's SP protocol over `ipc://` unix sockets, in pure Rust: what KiCad's API server (`nng_rep0` listening
//! on `ipc://<path>`, libnng 1.11 in the bp.4 runtime) speaks, and all the Bun helper's `nng.ts`/`worker.ts` used of libnng:
//! `nng_req0_open`, `nng_dial`, `nng_send`, `nng_recv`, with `req:resend-time` an hour so that a request is never sent twice.
//!
//! On the wire (nng src/sp/transport/ipc, checked byte by byte against the real server):
//!   hello, each way, once:  00 'S' 'P' 00  <proto u16 BE>  00 00      REQ0 = 0x30, REP0 = 0x31
//!   a message, each way:    01  <length u64 BE>  <payload>
//!   a REQ0 payload:         <request id u32 BE, high bit set>  <body>; the reply carries the same id, then its body.
//! Deliberate differences from a libnng REQ socket: no background redial and no resend. A connection that ends while a
//! request is out is an error at once (the request may have reached KiCad: its outcome is unknown), where libnng would wait
//! out the timeout and could send it again on a new pipe. The caller drops a failed connection and dials again next time.
//! Nothing here signals a process.
use std::{hash::{BuildHasher, Hasher}, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use crate::plat::ipc::Stream;

pub const REQ0: u16 = 0x30;
pub const REP0: u16 = 0x31;
/// Bigger than any board KiCad could send and small enough that a corrupt length cannot run the hub out of memory (the helper had no bound).
pub const MAX_MESSAGE: u64 = 1 << 30;
/// nng's own bound on the hello exchange.
pub const NEGOTIATE: Duration = Duration::from_secs(10);
const MESSAGE: u8 = 1;
const ID_BIT: u32 = 0x8000_0000;

/// The eight bytes a side says first.
pub fn hello(proto: u16) -> [u8; 8] { let p = proto.to_be_bytes(); [0, b'S', b'P', 0, p[0], p[1], 0, 0] }
/// The peer's protocol from its hello.
pub fn peer_of(h: &[u8; 8]) -> Result<u16, String> {
    if h[0] != 0 || h[1] != b'S' || h[2] != b'P' || h[3] != 0 || h[6] != 0 || h[7] != 0 { return Err("Protocol error".into()) } // NNG_EPROTO's words
    Ok(u16::from_be_bytes([h[4], h[5]]))
}
/// The nine bytes before a message's payload.
pub fn head(len: u64) -> [u8; 9] { let l = len.to_be_bytes(); [MESSAGE, l[0], l[1], l[2], l[3], l[4], l[5], l[6], l[7]] }
/// A payload's length from its head.
pub fn length_of(h: &[u8; 9]) -> Result<u64, String> {
    if h[0] != MESSAGE { return Err("Protocol error".into()) }
    let len = u64::from_be_bytes([h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8]]);
    if len > MAX_MESSAGE { return Err(format!("KiCad's reply is {len} bytes, over the {MAX_MESSAGE} this hub takes")) }
    Ok(len)
}
/// One request as it goes on the wire: head, id, body.
pub fn frame(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(13 + body.len());
    v.extend_from_slice(&head(4 + body.len() as u64)); v.extend_from_slice(&id.to_be_bytes()); v.extend_from_slice(body); v
}
/// A reply payload's id and body; None when it is too short to hold an id or the id has no high bit (libnng drops those).
pub fn split(payload: &[u8]) -> Option<(u32, &[u8])> {
    let id = u32::from_be_bytes(payload.get(..4)?.try_into().ok()?);
    (id & ID_BIT != 0).then(|| (id, &payload[4..]))
}
/// The id after this one: the low 31 bits count, the high bit stays set.
pub fn next_id(id: u32) -> u32 { ID_BIT | (id.wrapping_add(1) & !ID_BIT) }
fn first_id() -> u32 { ID_BIT | (std::collections::hash_map::RandomState::new().build_hasher().finish() as u32) }

/// One message off the stream. EOF before or inside it is an error: there is no reply any more.
async fn read_message<S: AsyncRead + Unpin>(s: &mut S) -> Result<Vec<u8>, String> {
    let mut h = [0u8; 9];
    s.read_exact(&mut h).await.map_err(gone)?;
    let len = length_of(&h)?;
    let mut v = Vec::with_capacity(len.min(1 << 20) as usize); // grown as it arrives: the length is the peer's word
    let got = (&mut *s).take(len).read_to_end(&mut v).await.map_err(gone)?;
    if (got as u64) < len { return Err(CLOSED.into()) }
    Ok(v)
}
pub const CLOSED: &str = "KiCad closed the connection before answering; request outcome unknown";
fn gone(e: std::io::Error) -> String { if matches!(e.kind(), std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe) { CLOSED.into() } else { format!("KiCad IPC connection failed: {e}; request outcome unknown") } }

/// nng's words for a failed connect; a missing socket is "nobody is listening", as in nng's ipc dialer.
fn dial_words(e: &std::io::Error) -> String {
    use std::io::ErrorKind::*;
    match e.kind() { NotFound | ConnectionRefused => "Connection refused".into(), PermissionDenied => "Permission denied".into(), _ => e.to_string() }
}

/// A connected REQ0 socket. One request at a time; a failed one leaves the connection unusable, so drop it.
pub struct Req { s: Stream, id: u32 }
impl Req {
    /// Connect and exchange hellos, as `nng_dial` does before it returns.
    pub async fn dial(path: &str) -> Result<Req, String> {
        let say = |w: String| format!("dial ipc://{path}: {w}");
        let s = crate::plat::ipc::connect(path).await.map_err(|e| say(dial_words(&e)))?;
        match tokio::time::timeout(NEGOTIATE, Self::hellos(s)).await {
            Ok(r) => r.map_err(say),
            Err(_) => Err(say("Timed out".into())),
        }
    }
    async fn hellos(mut s: Stream) -> Result<Req, String> {
        s.write_all(&hello(REQ0)).await.map_err(|e| e.to_string())?;
        let mut h = [0u8; 8];
        s.read_exact(&mut h).await.map_err(|e| e.to_string())?;
        if peer_of(&h)? != REP0 { return Err("Protocol error".into()) } // req0_pipe_start: only a REP0 is a peer
        Ok(Req { s, id: first_id() })
    }
    /// Send one request and wait for its reply. A reply under another id is dropped, as libnng does.
    pub async fn request(&mut self, body: &[u8]) -> Result<Vec<u8>, String> {
        self.id = next_id(self.id);
        self.s.write_all(&frame(self.id, body)).await.map_err(gone)?;
        loop {
            let m = read_message(&mut self.s).await?;
            if let Some((id, reply)) = split(&m) { if id == self.id { return Ok(reply.to_vec()) } }
        }
    }
}

// A test's stand-in peer (`stub` below) and the frame round trips. The wire these state was read from the real server first.
#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::plat::ipc::{Listener as UnixListener, Served as UnixStream};

    /// A unix REP server for tests: `answer` gets each request's id and body, and writes what it likes.
    pub async fn read_req(s: &mut UnixStream) -> Option<(u32, Vec<u8>)> { let m = read_message(s).await.ok()?; let (id, b) = split(&m)?; Some((id, b.to_vec())) }
    pub async fn write_rep(s: &mut UnixStream, id: u32, body: &[u8]) { s.write_all(&frame(id, body)).await.unwrap() }
    pub async fn accept(l: &UnixListener) -> (UnixStream, [u8; 8]) {
        let mut s = crate::plat::ipc::accept(l).await.unwrap();
        s.write_all(&hello(REP0)).await.unwrap();
        let mut h = [0u8; 8]; s.read_exact(&mut h).await.unwrap(); (s, h)
    }
    pub fn sock(name: &str) -> (String, UnixListener, crate::plat::tmp::TempDir) {
        let d = crate::plat::tmp::short_dir(&format!("nng-{name}"));
        let p = d.join("api.sock"); let p: String = p.to_string_lossy().into(); let l = crate::plat::ipc::bind(&p).unwrap(); (p, l, d)
    }

    #[test] fn hello_bytes() {
        assert_eq!(hello(REQ0), [0x00, 0x53, 0x50, 0x00, 0x00, 0x30, 0x00, 0x00]);
        assert_eq!(hello(REP0), [0x00, 0x53, 0x50, 0x00, 0x00, 0x31, 0x00, 0x00]); // read off the real server's first bytes: 0053500000310000
        assert_eq!(peer_of(&hello(REP0)), Ok(REP0));
        for bad in [[1, b'S', b'P', 0, 0, 0x31, 0, 0], [0, b's', b'P', 0, 0, 0x31, 0, 0], [0, b'S', b'P', 1, 0, 0x31, 0, 0], [0, b'S', b'P', 0, 0, 0x31, 1, 0], [0, b'S', b'P', 0, 0, 0x31, 0, 1]] { assert!(peer_of(&bad).is_err(), "{bad:?}") }
    }
    #[test] fn frame_round_trip() {
        let bodies: [&[u8]; 3] = [b"", b"x", &[0u8; 300]];
        for body in bodies {
            let id = 0x8000_0123; let w = frame(id, body);
            assert_eq!(w[0], 1); assert_eq!(w.len(), 13 + body.len());
            let h: [u8; 9] = w[..9].try_into().unwrap();
            assert_eq!(length_of(&h), Ok(4 + body.len() as u64));
            assert_eq!(split(&w[9..]), Some((id, body)));
        }
        assert_eq!(frame(0x8000_0001, b"ab"), vec![1, 0, 0, 0, 0, 0, 0, 0, 6, 0x80, 0, 0, 1, b'a', b'b']);
        assert_eq!(split(b"abc"), None); assert_eq!(split(&[0, 0, 0, 1, 9]), None);
    }
    #[test] fn frame_bad_heads() {
        let mut h = head(4); h[0] = 2; assert!(length_of(&h).is_err());
        assert!(length_of(&head(MAX_MESSAGE + 1)).is_err()); assert!(length_of(&head(MAX_MESSAGE)).is_ok());
    }
    #[test] fn ids_keep_the_high_bit() { assert_eq!(next_id(0x8000_0001), 0x8000_0002); assert_eq!(next_id(0xffff_ffff), 0x8000_0000); assert!(first_id() & ID_BIT != 0) }

    #[tokio::test] async fn dial_says_hello_and_round_trips() {
        let (path, l, _d) = sock("rt");
        let srv = tokio::spawn(async move {
            let (mut s, h) = accept(&l).await; assert_eq!(h, hello(REQ0));
            for _ in 0..3 { let (id, b) = read_req(&mut s).await.unwrap(); assert!(id & ID_BIT != 0); let mut r = b.clone(); r.reverse(); write_rep(&mut s, id, &r).await }
        });
        let mut r = Req::dial(&path).await.unwrap();
        let big = vec![7u8; 3 << 20]; let bodies: [&[u8]; 3] = [b"abc", b"", &big];
        for b in bodies { let mut want = b.to_vec(); want.reverse(); assert_eq!(r.request(b).await.unwrap(), want) }
        srv.await.unwrap();
    }
    #[tokio::test] async fn a_stale_id_is_dropped() {
        let (path, l, _d) = sock("stale");
        tokio::spawn(async move { let (mut s, _) = accept(&l).await; let (id, _) = read_req(&mut s).await.unwrap(); write_rep(&mut s, id ^ 1, b"old").await; s.write_all(&[1, 0, 0, 0, 0, 0, 0, 0, 2, 9, 9]).await.unwrap(); write_rep(&mut s, id, b"new").await; let _ = read_req(&mut s).await; });
        let mut r = Req::dial(&path).await.unwrap();
        assert_eq!(r.request(b"q").await.unwrap(), b"new");
    }
    #[tokio::test] async fn a_closed_connection_is_an_error_at_once() {
        let (path, l, _d) = sock("eof");
        tokio::spawn(async move { let (mut s, _) = accept(&l).await; let _ = read_req(&mut s).await; drop(s); let (mut s, _) = accept(&l).await; let (id, _) = read_req(&mut s).await.unwrap(); s.write_all(&head(100)).await.unwrap(); s.write_all(&id.to_be_bytes()).await.unwrap(); });
        let mut r = Req::dial(&path).await.unwrap();
        assert_eq!(r.request(b"q").await.unwrap_err(), CLOSED);
        let mut r = Req::dial(&path).await.unwrap(); // the next one dials again
        assert_eq!(r.request(b"q").await.unwrap_err(), CLOSED); // cut inside a message
    }
    #[tokio::test] async fn dial_refuses_what_is_not_a_rep() {
        let (path, l, _d) = sock("proto");
        tokio::spawn(async move { loop { let mut s = crate::plat::ipc::accept(&l).await.unwrap(); s.write_all(&hello(0x21)).await.unwrap(); let mut h = [0u8; 8]; let _ = s.read_exact(&mut h).await; } });
        assert_eq!(Req::dial(&path).await.err().unwrap(), format!("dial ipc://{path}: Protocol error"));
        let e = Req::dial("/nonexistent/api.sock").await.err().unwrap();
        assert_eq!(e, "dial ipc:///nonexistent/api.sock: Connection refused");
    }
}
