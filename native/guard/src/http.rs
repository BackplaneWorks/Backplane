//! Plain HTTP/1.1 over std: the probe the guard sends to its hub, the client of `backplane hub --url`, and the small server rescue answers with.
use sha2::{Digest, Sha256};
use std::{io::{Read, Write}, net::{TcpListener, TcpStream, ToSocketAddrs}, sync::{atomic::{AtomicBool, Ordering::SeqCst}, Arc}, time::Duration};

pub fn call(addr: &str, method: &str, path: &str, token: &str, body: &str, timeout: Duration) -> Option<(u16, String)> {
    let a = addr.to_socket_addrs().ok()?.next()?;
    let mut s = TcpStream::connect_timeout(&a, timeout).ok()?;
    s.set_read_timeout(Some(timeout)).ok()?; s.set_write_timeout(Some(timeout)).ok()?;
    let auth = if token.is_empty() { String::new() } else { format!("Authorization: Bearer {token}\r\n") };
    s.write_all(format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).ok()?;
    let mut out = Vec::new(); let _ = s.take(1 << 20).read_to_end(&mut out);
    let t = String::from_utf8_lossy(&out); let code = t.split_whitespace().nth(1)?.parse().ok()?;
    Some((code, t.split_once("\r\n\r\n").map_or(String::new(), |x| x.1.to_string())))
}

pub fn sha(x: &str) -> String { Sha256::digest(x.as_bytes()).iter().map(|b| format!("{b:02x}")).collect() }
/// HMAC-SHA256, hex: the hub's peer signature (`net/sign.rs`) over `<ts>.<body>`.
pub fn hmac(key: &[u8], msg: &[u8]) -> String {
    let mut k = if key.len() > 64 { Sha256::digest(key).to_vec() } else { key.to_vec() }; k.resize(64, 0);
    let (ip, op): (Vec<u8>, Vec<u8>) = (k.iter().map(|b| b ^ 0x36).collect(), k.iter().map(|b| b ^ 0x5c).collect());
    let inner = Sha256::new().chain_update(&ip).chain_update(msg).finalize();
    Sha256::new().chain_update(&op).chain_update(inner).finalize().iter().map(|b| format!("{b:02x}")).collect()
}
pub fn same(a: &str, b: &str) -> bool { a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0 }

pub struct Req { pub method: String, pub path: String, pub query: String, pub headers: Vec<(String, String)>, pub body: String }
impl Req {
    pub fn header(&self, n: &str) -> &str { self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map_or("", |(_, v)| v) }
    pub fn credential(&self) -> String {
        if let Some(b) = self.header("authorization").strip_prefix("Bearer ") { return b.trim().to_string() }
        self.query.split('&').find_map(|kv| kv.strip_prefix("token=")).unwrap_or("").to_string()
    }
}
fn read(s: &mut TcpStream) -> Option<Req> {
    let (mut buf, mut tmp) = (Vec::new(), [0u8; 4096]);
    let end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") { break i + 4 }
        if buf.len() > 32 << 10 { return None }
        let n = s.read(&mut tmp).ok()?; if n == 0 { return None } buf.extend(&tmp[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..end]).into_owned();
    let mut l = head.lines(); let mut first = l.next()?.split_whitespace();
    let (method, target) = (first.next()?.to_string(), first.next()?.to_string());
    let headers: Vec<(String, String)> = l.filter_map(|x| x.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let want = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(0).min(64 << 10);
    let mut body = buf[end..].to_vec();
    while body.len() < want { let n = s.read(&mut tmp).ok()?; if n == 0 { break } body.extend(&tmp[..n]); }
    body.truncate(want);
    let (path, query) = target.split_once('?').map_or((target.clone(), String::new()), |(p, q)| (p.to_string(), q.to_string()));
    Some(Req { method, path, query, headers, body: String::from_utf8_lossy(&body).into_owned() })
}

pub type Handler = Arc<dyn Fn(&Req) -> (u16, String) + Send + Sync>;
/// Answering on some addresses until dropped.
pub struct Server { stop: Arc<AtomicBool>, threads: Vec<std::thread::JoinHandle<()>>, pub bound: Vec<String>, pub failed: Vec<String> }
impl Server {
    pub fn start(addrs: &[String], h: Handler) -> Server {
        let (stop, mut threads, mut bound, mut failed) = (Arc::new(AtomicBool::new(false)), vec![], vec![], vec![]);
        for a in addrs {
            match TcpListener::bind(a) {
                Ok(l) => {
                    let _ = l.set_nonblocking(true); bound.push(a.clone());
                    let (stop, h) = (stop.clone(), h.clone());
                    threads.push(std::thread::spawn(move || while !stop.load(SeqCst) {
                        match l.accept() {
                            Ok((mut s, _)) => { let h = h.clone(); std::thread::spawn(move || {
                                let _ = s.set_nonblocking(false); let _ = s.set_read_timeout(Some(Duration::from_secs(5))); let _ = s.set_write_timeout(Some(Duration::from_secs(5)));
                                let Some(r) = read(&mut s) else { return };
                                let (code, body) = h(&r);
                                let text = match code { 200 => "OK", 202 => "Accepted", 401 => "Unauthorized", 404 => "Not Found", 409 => "Conflict", 429 => "Too Many Requests", _ => "Service Unavailable" };
                                let _ = s.write_all(format!("HTTP/1.1 {code} {text}\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
                            }); }
                            Err(_) => std::thread::sleep(Duration::from_millis(50)),
                        }
                    }));
                }
                Err(e) => failed.push(format!("{a}: {e}")),
            }
        }
        Server { stop, threads, bound, failed }
    }
}
impl Drop for Server { fn drop(&mut self) { self.stop.store(true, SeqCst); for t in self.threads.drain(..) { let _ = t.join(); } } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn hmac_matches_the_published_vector() { // RFC 4231 test case 2
        assert_eq!(hmac(b"Jefe", b"what do ya want for nothing?"), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    }
    #[test] fn a_server_answers_and_lets_go_of_its_port() {
        let port = { let l = TcpListener::bind("127.0.0.1:0").unwrap(); l.local_addr().unwrap().port() };
        let a = format!("127.0.0.1:{port}");
        let s = Server::start(&[a.clone()], Arc::new(|r: &Req| (503, format!("{{\"p\":\"{}\",\"c\":\"{}\",\"b\":\"{}\"}}", r.path, r.credential(), r.body))));
        let (code, body) = call(&a, "POST", "/x?y=1", "tok", "hi", Duration::from_secs(2)).unwrap();
        assert_eq!(code, 503); assert_eq!(body, r#"{"p":"/x","c":"tok","b":"hi"}"#);
        drop(s); assert!(TcpListener::bind(&a).is_ok(), "the port is free for the hub again");
    }
    #[test] fn nothing_listening_is_none_not_a_hang() { assert!(call("127.0.0.1:1", "GET", "/", "", "", Duration::from_millis(200)).is_none()); }
}
