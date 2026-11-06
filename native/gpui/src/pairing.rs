//! Pairing this client with a hub: what a link or short code says, the exchange for a device credential, where the credential is kept,
//! and the stream every direct hub request uses (plain, or TLS for a hub behind `tailscale serve`). The rules are the phones'
//! (mobile/ios Hub.swift, mobile/android Hub.kt). A paired credential has its own file: `~/.backplane-gpui/token` is a hub's own token.
use serde_json::{json, Value};
use std::{io::{self, Read, Write}, net::TcpStream, path::PathBuf, sync::{Arc, OnceLock}, time::Duration};

const SCHEMES: [&str; 2] = ["backplane://", "bpgpui://"];
pub fn is_link(s: &str) -> bool { SCHEMES.iter().any(|p| s.starts_with(p)) }

#[derive(Debug, PartialEq, Clone)]
pub struct Invitation { pub origin: String, pub secret: String }

fn unpct(s: &str) -> String {
    let b = s.as_bytes(); let (mut out, mut i) = (Vec::with_capacity(b.len()), 0);
    while i < b.len() {
        let hex = (b[i] == b'%' && i + 2 < b.len()).then(|| std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())).flatten();
        if let Some(x) = hex { out.push(x); i += 3 } else { out.push(b[i]); i += 1 }
    }
    String::from_utf8_lossy(&out).into_owned()
}
/// The link itself: an app link carries it in `url`.
fn unwrapped(text: &str) -> String {
    let raw = text.trim();
    if !is_link(raw) { return raw.to_string() }
    raw.split_once('?').map_or("", |(_, q)| q).split('&').find_map(|kv| kv.strip_prefix("url=")).map(unpct).unwrap_or_default()
}
/// `scheme://authority` of an http(s) URL with a host and no userinfo.
fn origin_of(raw: &str) -> Option<String> {
    let (scheme, rest) = raw.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" { return None }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit_once(':').filter(|(_, p)| p.bytes().all(|c| c.is_ascii_digit())).map_or(authority, |(h, _)| h);
    (!host.is_empty() && !authority.contains('@') && !authority.chars().any(char::is_whitespace)).then(|| format!("{scheme}://{authority}"))
}
fn fields(raw: &str) -> Vec<(String, String)> {
    raw.split_once('#').map_or("", |(_, f)| f).split('&').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), unpct(v))).collect()
}
fn good_secret(s: &str) -> bool {
    (s.len() == 32 && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))) || (s.len() == 8 && s.bytes().all(|c| matches!(c, b'A'..=b'Z' | b'2'..=b'9')))
}
/// A pairing link's hub and secret. None for anything else.
pub fn invitation(text: &str) -> Option<Invitation> {
    let raw = unwrapped(text);
    let origin = origin_of(&raw)?;
    let secret = fields(&raw).into_iter().find(|(k, _)| k == "invite" || k == "code")?.1;
    good_secret(&secret).then_some(Invitation { origin, secret })
}
/// A hub address and a short code, spaces, dashes and case forgiven.
pub fn from_code(address: &str, code: &str) -> Option<Invitation> {
    let code: String = code.chars().filter(|c| *c != ' ' && *c != '-').collect::<String>().to_uppercase();
    let address = address.trim();
    let origin = origin_of(&if address.contains("://") { address.to_string() } else { format!("http://{address}") })?;
    (code.len() == 8 && good_secret(&code)).then_some(Invitation { origin, secret: code })
}
/// What the form holds: the link if there is one, else the address and code. Err: what to say.
pub fn submit(link: &str, address: &str, code: &str) -> Result<Invitation, &'static str> {
    if !link.trim().is_empty() { return invitation(link).ok_or("That is not a pairing link. Copy it again from Settings → Devices.") }
    if address.trim().is_empty() || code.trim().is_empty() { return Err("Paste a pairing link, or enter the hub address and short code.") }
    from_code(address, code).ok_or("Check the hub address and the eight-character code.")
}
pub fn message(error: &str) -> &'static str {
    match error {
        "wrong_code" => "That code is not correct. Check it and try again.",
        "expired_code" => "This invitation expired. Create another in Settings → Devices.",
        "used_code" => "This invitation has already been used. Create a new one.",
        "revoked_code" => "This invitation was revoked. Create a new one in Settings → Devices.",
        "rate_limited" => "Too many attempts. Wait a minute and try again.",
        "device_limit" => "This hub has too many devices. Revoke one in Settings → Devices.",
        "storage_unavailable" => "The hub could not save this pairing. Try again.",
        _ => "The hub could not be reached. Check the address and that this machine has a route to it.",
    }
}
pub fn ws_of(origin: &str) -> String {
    origin.strip_prefix("https://").map(|r| format!("wss://{r}")).or_else(|| origin.strip_prefix("http://").map(|r| format!("ws://{r}"))).unwrap_or_else(|| origin.to_string())
}
pub fn http_of(ws: &str) -> String {
    ws.strip_prefix("wss://").map(|r| format!("https://{r}")).or_else(|| ws.strip_prefix("ws://").map(|r| format!("http://{r}"))).unwrap_or_else(|| ws.to_string())
}
/// host:port of an origin or socket address, with the scheme's port when it names none (a bracketed IPv6 address keeps its brackets).
pub fn host_port(url: &str) -> (String, bool) {
    let tls = url.starts_with("https://") || url.starts_with("wss://");
    let a = url.split_once("://").map_or(url, |(_, r)| r).trim_end_matches('/');
    let named = a.rsplit_once(':').map_or(false, |(_, p)| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()));
    (if named { a.to_string() } else { format!("{a}:{}", if tls { 443 } else { 80 }) }, tls)
}

/// This device's name in Settings → Devices: printable ASCII, as a header must be.
pub fn name() -> String {
    let os = if cfg!(target_os = "macos") { "macOS" } else { "Linux" };
    let host = std::env::var("HOSTNAME").ok().or_else(|| std::fs::read_to_string("/etc/hostname").ok()).unwrap_or_default();
    let host: String = host.trim().chars().filter(|c| (' '..='~').contains(c)).take(40).collect();
    if host.is_empty() { format!("{os} desktop") } else { format!("{os} desktop - {host}") }
}
pub fn enroll_request(i: &Invitation, name: &str) -> String {
    let (addr, _) = host_port(&i.origin);
    format!("POST /auth/enroll HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {}\r\nX-Backplane-Client: native\r\nX-Backplane-Name: {name}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", i.secret)
}
/// The credential in the hub's answer, or what to say. Only a 200 counts: a redirect is never followed, so a link cannot send the secret on.
pub fn enroll_answer(status: u16, body: &[u8]) -> Result<String, String> {
    let j: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let token = j["token"].as_str().unwrap_or("");
    if status == 200 && token.len() == 32 && token.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) { return Ok(token.to_string()) }
    Err(message(j["error"].as_str().unwrap_or("hub_unreachable")).to_string())
}
/// Trade the invitation for a device credential. Blocking.
pub fn exchange(i: &Invitation, name: &str) -> Result<String, String> {
    let (addr, tls) = host_port(&i.origin);
    let run = || -> io::Result<Vec<u8>> {
        let mut s = open(&addr, tls)?; s.set_timeouts(Duration::from_secs(8))?;
        s.write_all(enroll_request(i, name).as_bytes())?;
        let mut b = Vec::new(); s.take(65536).read_to_end(&mut b)?; Ok(b)
    };
    let b = run().map_err(|_| message("hub_unreachable").to_string())?;
    let (status, body) = crate::screens::images::split(&b).ok_or_else(|| message("hub_unreachable").to_string())?;
    enroll_answer(status, body)
}

fn dir() -> Option<PathBuf> { crate::local::path()?.parent().map(PathBuf::from) }
pub fn token_path() -> Option<PathBuf> { Some(dir()?.join("device-token")) }
fn hub_path() -> Option<PathBuf> { Some(dir()?.join("pairing.json")) }
/// The hub this client paired with (ws:// or wss://), when its credential is here too.
pub fn load() -> Option<String> {
    let j: Value = serde_json::from_str(&std::fs::read_to_string(hub_path()?).ok()?).ok()?;
    let hub = j["hub"].as_str().filter(|h| h.starts_with("ws://") || h.starts_with("wss://"))?.to_string();
    token_path()?.is_file().then_some(hub)
}
fn write_private(path: &PathBuf, body: &str) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut o = std::fs::OpenOptions::new(); o.write(true).create(true).truncate(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    let mut f = o.open(&tmp)?; f.write_all(body.as_bytes())?; f.sync_all()?; drop(f);
    std::fs::rename(&tmp, path)
}
/// Keep the pairing: the credential first, so `load` never finds a hub without one.
pub fn save(origin: &str, token: &str) -> io::Result<()> {
    let (t, h) = (token_path().ok_or(io::ErrorKind::NotFound)?, hub_path().ok_or(io::ErrorKind::NotFound)?);
    if let Some(d) = t.parent() { std::fs::create_dir_all(d)?; }
    write_private(&t, token)?;
    write_private(&h, &json!({"hub": ws_of(origin)}).to_string())
}
/// The arguments to start again with once paired: what named the old hub or drove this run goes, so the saved pairing is what connects.
pub fn relaunch_args(args: &[String]) -> Vec<String> {
    let (mut out, mut it) = (vec![], args.iter());
    while let Some(a) = it.next() {
        match a.as_str() { "--url" | "--hub" | "--token-file" | "--script" | "--pair" => { it.next(); } x if is_link(x) => {} _ => out.push(a.clone()) }
    }
    out
}
/// Start again on the saved pairing: every address and credential the client holds comes from its configuration at start. Returns only on failure.
pub fn relaunch() -> io::Error {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exe = match std::env::current_exe() { Ok(e) => e, Err(e) => return e };
    let mut c = std::process::Command::new(exe); c.args(relaunch_args(&args)).env_remove("BACKPLANE_GPUI_SCRIPT");
    #[cfg(unix)] { use std::os::unix::process::CommandExt; c.exec() }
    #[cfg(not(unix))] { match c.spawn() { Ok(_) => std::process::exit(0), Err(e) => e } }
}

/// A direct request's connection to the hub.
pub enum Stream { Plain(TcpStream), Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>) }
impl Read for Stream {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(b),
            // a peer that hangs up without close_notify has still sent its whole answer: Connection: close ends every request here
            Stream::Tls(s) => match s.read(b) { Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(0), r => r },
        }
    }
}
impl Write for Stream {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> { match self { Stream::Plain(s) => s.write(b), Stream::Tls(s) => s.write(b) } }
    fn flush(&mut self) -> io::Result<()> { match self { Stream::Plain(s) => s.flush(), Stream::Tls(s) => s.flush() } }
}
impl Stream {
    pub fn set_timeouts(&self, d: Duration) -> io::Result<()> {
        let s = match self { Stream::Plain(s) => s, Stream::Tls(s) => &s.sock };
        s.set_read_timeout(Some(d))?; s.set_write_timeout(Some(d))
    }
}
fn tls_config() -> Arc<rustls::ClientConfig> {
    static C: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    C.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
        Arc::new(rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth())
    }).clone()
}
/// `addr` is host:port. TLS checks the name against the system's roots.
pub fn open(addr: &str, tls: bool) -> io::Result<Stream> {
    let tcp = TcpStream::connect(addr)?;
    if !tls { return Ok(Stream::Plain(tcp)) }
    let host = addr.rsplit_once(':').map_or(addr, |(h, _)| h).trim_matches(['[', ']']).to_string();
    let name = rustls::pki_types::ServerName::try_from(host).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let conn = rustls::ClientConnection::new(tls_config(), name).map_err(io::Error::other)?;
    Ok(Stream::Tls(Box::new(rustls::StreamOwned::new(conn, tcp))))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SECRET: &str = "0123456789abcdef0123456789abcdef";
    fn inv(origin: &str, secret: &str) -> Option<Invitation> { Some(Invitation { origin: origin.into(), secret: secret.into() }) }
    #[test] fn a_link_gives_its_hub_and_secret() {
        assert_eq!(invitation(&format!("http://100.1.2.3:3801/pair#invite={SECRET}&routes=%5B%5D")), inv("http://100.1.2.3:3801", SECRET));
        assert_eq!(invitation(&format!("  https://box.tail1.ts.net:8461/pair#invite={SECRET}\n")), inv("https://box.tail1.ts.net:8461", SECRET));
        assert_eq!(invitation("http://h:1/pair#code=ABCD2345"), inv("http://h:1", "ABCD2345"));
    }
    #[test] fn an_app_link_carries_the_link_in_url() {
        let enc = format!("http%3A%2F%2F10.0.0.1%3A3801%2Fpair%23invite%3D{SECRET}");
        assert_eq!(invitation(&format!("backplane://pair?url={enc}")), inv("http://10.0.0.1:3801", SECRET));
        assert_eq!(invitation(&format!("bpgpui://pair?x=1&url={enc}")), inv("http://10.0.0.1:3801", SECRET));
        assert_eq!(invitation("backplane://pair"), None);
    }
    #[test] fn what_is_not_a_pairing_link_is_refused() {
        for bad in ["", "hello", "ftp://h/pair#invite=0123456789abcdef0123456789abcdef", "http://u:p@h/pair#invite=0123456789abcdef0123456789abcdef", "http:///pair#invite=0123456789abcdef0123456789abcdef",
            "http://h/pair", "http://h/pair#invite=short", "http://h/pair#invite=0123456789ABCDEF0123456789ABCDEF", "http://h/#token=0123456789abcdef0123456789abcdef", "http://h/pair#code=ABCD1234"] {
            assert_eq!(invitation(bad), None, "{bad}");
        }
    }
    #[test] fn a_short_code_forgives_spaces_dashes_and_case() {
        assert_eq!(from_code("100.1.2.3:3801", "abcd-2345"), inv("http://100.1.2.3:3801", "ABCD2345"));
        assert_eq!(from_code(" https://box.ts.net:8461/ ", "ABCD 2345"), inv("https://box.ts.net:8461", "ABCD2345"));
        assert_eq!(from_code("h:1", "ABCD234"), None); assert_eq!(from_code("h:1", "ABCD1234"), None, "1 is not in the alphabet"); assert_eq!(from_code("", "ABCD2345"), None);
    }
    #[test] fn the_form_prefers_the_link_and_says_what_is_missing() {
        assert_eq!(submit(&format!("http://h:1/pair#invite={SECRET}"), "other:2", "ABCD2345"), Ok(inv("http://h:1", SECRET).unwrap()));
        assert_eq!(submit("", "h:1", "ABCD2345"), Ok(inv("http://h:1", "ABCD2345").unwrap()));
        assert!(submit("nonsense", "h:1", "ABCD2345").is_err(), "a bad link is not passed over for the code");
        assert!(submit("", "", "").is_err() && submit("", "h:1", "").is_err() && submit("", "h:1", "x").is_err());
    }
    #[test] fn every_refusal_has_the_phones_words() {
        assert_eq!(message("wrong_code"), "That code is not correct. Check it and try again.");
        assert_eq!(message("expired_code"), "This invitation expired. Create another in Settings → Devices.");
        assert_eq!(message("used_code"), "This invitation has already been used. Create a new one.");
        assert_eq!(message("revoked_code"), "This invitation was revoked. Create a new one in Settings → Devices.");
        assert_eq!(message("rate_limited"), "Too many attempts. Wait a minute and try again.");
        assert!(message("anything else").starts_with("The hub could not be reached"));
    }
    #[test] fn schemes_and_ports() {
        assert_eq!((ws_of("https://b.ts.net:8461"), ws_of("http://h:1")), ("wss://b.ts.net:8461".to_string(), "ws://h:1".to_string()));
        assert_eq!((http_of("wss://b.ts.net"), http_of("ws://h:1")), ("https://b.ts.net".to_string(), "http://h:1".to_string()));
        assert_eq!(host_port("https://b.ts.net"), ("b.ts.net:443".to_string(), true));
        assert_eq!(host_port("wss://b.ts.net:8461/"), ("b.ts.net:8461".to_string(), true));
        assert_eq!(host_port("ws://h:3801"), ("h:3801".to_string(), false));
        assert_eq!(host_port("http://h"), ("h:80".to_string(), false));
        assert_eq!(host_port("http://[fd7a::1]:3801"), ("[fd7a::1]:3801".to_string(), false));
        assert_eq!(host_port("https://[fd7a::1]"), ("[fd7a::1]:443".to_string(), true));
    }
    #[test] fn the_enroll_request_is_the_native_exchange_and_only_a_200_pairs() {
        let r = enroll_request(&inv("https://b.ts.net:8461", "ABCD2345").unwrap(), "Linux desktop");
        assert!(r.starts_with("POST /auth/enroll HTTP/1.1\r\nHost: b.ts.net:8461\r\n") && r.contains("Authorization: Bearer ABCD2345\r\n") && r.contains("X-Backplane-Client: native\r\n") && r.contains("X-Backplane-Name: Linux desktop\r\n") && r.ends_with("\r\n\r\n"));
        assert_eq!(enroll_answer(200, format!(r#"{{"token":"{SECRET}","id":"x"}}"#).as_bytes()), Ok(SECRET.to_string()));
        assert_eq!(enroll_answer(400, br#"{"error":"used_code"}"#), Err(message("used_code").to_string()));
        assert_eq!(enroll_answer(302, format!(r#"{{"token":"{SECRET}"}}"#).as_bytes()), Err(message("").to_string()), "a redirect is not an answer");
        assert_eq!(enroll_answer(200, br#"{"ok":true}"#), Err(message("").to_string()), "a browser's answer holds no credential");
        assert_eq!(enroll_answer(200, br#"{"token":"short"}"#), Err(message("").to_string()));
        assert_eq!(enroll_answer(200, b"<html>"), Err(message("").to_string()));
    }
    #[test] fn a_relaunch_drops_what_named_the_old_hub() {
        let v = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(relaunch_args(&v(&["--url", "http://h:1", "--token-file", "/t", "--theme", "dark", "--script", "/s", "backplane://pair?url=x", "--pair", "l", "--exit-after", "9"])), v(&["--theme", "dark", "--exit-after", "9"]));
    }
    #[test] fn a_device_name_can_go_in_a_header() { let n = name(); assert!(!n.is_empty() && n.is_ascii() && !n.contains(['\r', '\n'])); }
}
