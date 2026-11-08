//! Pairing this client with a hub: what a link or short code says, the exchange for a device credential, where the credential is kept,
//! and the stream every direct hub request uses (plain, or TLS for a hub behind `tailscale serve`). The rules are the phones'
//! (mobile/ios Hub.swift, mobile/android Hub.kt). A paired credential has its own file: `~/.backplane-gpui/token` is a hub's own token.
use serde_json::{json, Value};
use std::{io::{self, Read, Write}, net::TcpStream, path::PathBuf, sync::{Arc, OnceLock}, time::Duration};

const SCHEMES: [&str; 3] = ["backplane://", "bpgpui://", "backplane-gpui://"];
pub fn is_link(s: &str) -> bool { SCHEMES.iter().any(|p| s.starts_with(p)) }

#[derive(Debug, PartialEq, Clone)]
pub struct Invitation { pub origin: String, pub secret: String, /** every address to try, in order (the link's own first) */ pub routes: Vec<String>, /** a link, not an address typed by the person */ pub from_link: bool }

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
pub fn origin_of(raw: &str) -> Option<String> {
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
    good_secret(&secret).then(|| Invitation { routes: routes(text), origin, secret, from_link: true })
}
/// The addresses to try an invitation at, in order: the link's own, then each other plain http(s) address its `routes` field names (the phones' rule).
pub fn routes(text: &str) -> Vec<String> {
    let raw = unwrapped(text);
    let mut out: Vec<String> = origin_of(&raw).into_iter().collect();
    for (k, v) in fields(&raw) {
        if k != "routes" { continue }
        let Ok(Value::Array(xs)) = serde_json::from_str::<Value>(&v) else { continue };
        for o in xs.iter().filter_map(|x| x.as_str()).filter_map(origin_of) { if !out.contains(&o) { out.push(o) } }
    }
    out
}
/// Whether an address is the hub's own computer, which a different machine can never reach.
pub fn loopback(origin: &str) -> bool {
    let (host, _) = host_port(origin);
    let h = host.rsplit_once(':').map_or(host.as_str(), |(h, _)| h);
    h == "localhost" || h == "[::1]" || (h.starts_with("127.") && h.bytes().all(|c| c.is_ascii_digit() || c == b'.'))
}
pub fn only_local(routes: &[String]) -> bool { !routes.is_empty() && routes.iter().all(|r| loopback(r)) }
/// Whether something answers at one of these loopback addresses on this computer: a link that names only its hub's own computer is fine on that
/// computer (the desktop client beside its hub). A refused connection comes back at once; half a second at most per address.
pub fn answers_here(routes: &[String]) -> bool {
    routes.iter().filter(|r| loopback(r)).any(|r| {
        let (addr, _) = host_port(r);
        let host = addr.rsplit_once(':').map_or(addr.as_str(), |(h, _)| h);
        let addr = if host == "localhost" { addr.replacen("localhost", "127.0.0.1", 1) } else { addr.replace(['[', ']'], "") };
        let addr = if addr.matches(':').count() > 1 { addr.rsplit_once(':').map(|(h, p)| format!("[{h}]:{p}")).unwrap_or(addr) } else { addr };
        addr.parse::<std::net::SocketAddr>().map_or(false, |a| std::net::TcpStream::connect_timeout(&a, Duration::from_millis(500)).is_ok())
    })
}
/// Why a link cannot pair (None: it can). A link naming only its hub's own computer is refused unless that hub is on this computer: the
/// phones refuse it outright (no phone is its hub's computer); this desktop client first tries it here. An address the person typed is their word.
pub fn refusal(i: &Invitation) -> Option<&'static str> {
    (i.from_link && only_local(&i.routes) && !answers_here(&i.routes)).then_some("This invitation names only its hub's own computer (127.0.0.1), so this device cannot reach it. Start the hub with Tailscale on, or on an address this device can reach, then make a new invitation.")
}
/// A hub address and a short code, spaces, dashes and case forgiven.
pub fn from_code(address: &str, code: &str) -> Option<Invitation> {
    let code: String = code.chars().filter(|c| *c != ' ' && *c != '-').collect::<String>().to_uppercase();
    let address = address.trim();
    let origin = origin_of(&if address.contains("://") { address.to_string() } else { format!("http://{address}") })?;
    (code.len() == 8 && good_secret(&code)).then(|| Invitation { routes: vec![origin.clone()], origin, secret: code, from_link: false })
}
/// What the form holds: the link if there is one, else the address and code. Err: what to say.
pub fn submit(link: &str, address: &str, code: &str) -> Result<Invitation, &'static str> {
    if !link.trim().is_empty() {
        let i = invitation(link).ok_or("That is not a pairing link. Copy it again from Settings → Devices.")?;
        return match refusal(&i) { Some(why) => Err(why), None => Ok(i) };
    }
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
/// Trade the invitation for a device credential at one address. Blocking.
fn exchange_at(origin: &str, i: &Invitation, name: &str) -> Result<String, String> {
    let (addr, tls) = host_port(origin);
    let one = Invitation { origin: origin.to_string(), ..i.clone() };
    let run = || -> io::Result<Vec<u8>> {
        let mut s = open(&addr, tls)?; s.set_timeouts(Duration::from_secs(8))?;
        s.write_all(enroll_request(&one, name).as_bytes())?;
        let mut b = Vec::new(); s.take(65536).read_to_end(&mut b)?; Ok(b)
    };
    let b = run().map_err(|_| message("hub_unreachable").to_string())?;
    let (status, body) = crate::screens::images::split(&b).ok_or_else(|| message("hub_unreachable").to_string())?;
    enroll_answer(status, body)
}
/// Trade the invitation for a device credential: each address it names is tried in order; the first that answers decides. Answers the address that worked and the credential. Blocking.
pub fn exchange(i: &Invitation, name: &str) -> Result<(String, String), String> {
    let mut last = message("hub_unreachable").to_string();
    let list = if i.routes.is_empty() { vec![i.origin.clone()] } else { i.routes.clone() };
    for o in list {
        match exchange_at(&o, i, name) { Ok(t) => return Ok((o, t)), Err(e) if e == message("hub_unreachable") => last = e, Err(e) => return Err(e) }
    }
    Err(last)
}

pub(crate) fn dir() -> Option<PathBuf> { crate::local::path()?.parent().map(PathBuf::from) }
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
    let mut c = std::process::Command::new(exe); c.args(crate::hubs::relaunch_with(&args, &crate::hubs::load())).env_remove("BACKPLANE_GPUI_SCRIPT");
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
pub fn open(addr: &str, tls: bool) -> io::Result<Stream> { wrap(addr, TcpStream::connect(addr)?, tls) }
/// A connected socket to `addr`, in TLS when `tls`.
pub fn wrap(addr: &str, tcp: TcpStream, tls: bool) -> io::Result<Stream> {
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
    fn inv(origin: &str, secret: &str) -> Option<Invitation> { Some(Invitation { origin: origin.into(), secret: secret.into(), routes: vec![origin.into()], from_link: true }) }
    fn code(origin: &str, secret: &str) -> Option<Invitation> { inv(origin, secret).map(|i| Invitation { from_link: false, ..i }) }
    #[test] fn a_link_gives_its_hub_and_secret() {
        assert_eq!(invitation(&format!("http://100.1.2.3:3801/pair#invite={SECRET}&routes=%5B%5D")), inv("http://100.1.2.3:3801", SECRET));
        assert_eq!(invitation(&format!("  https://box.tail1.ts.net:8461/pair#invite={SECRET}\n")), inv("https://box.tail1.ts.net:8461", SECRET));
        assert_eq!(invitation("http://h:1/pair#code=ABCD2345"), inv("http://h:1", "ABCD2345"));
    }
    #[test] fn an_app_link_carries_the_link_in_url() {
        let enc = format!("http%3A%2F%2F10.0.0.1%3A3801%2Fpair%23invite%3D{SECRET}");
        assert_eq!(invitation(&format!("backplane://pair?url={enc}")), inv("http://10.0.0.1:3801", SECRET));
        assert_eq!(invitation(&format!("bpgpui://pair?x=1&url={enc}")), inv("http://10.0.0.1:3801", SECRET));
        assert_eq!(invitation(&format!("backplane-gpui://pair?url={enc}")), inv("http://10.0.0.1:3801", SECRET), "each app's own scheme");
        assert_eq!(invitation("backplane://pair"), None);
    }
    #[test] fn what_is_not_a_pairing_link_is_refused() {
        for bad in ["", "hello", "ftp://h/pair#invite=0123456789abcdef0123456789abcdef", "http://u:p@h/pair#invite=0123456789abcdef0123456789abcdef", "http:///pair#invite=0123456789abcdef0123456789abcdef",
            "http://h/pair", "http://h/pair#invite=short", "http://h/pair#invite=0123456789ABCDEF0123456789ABCDEF", "http://h/#token=0123456789abcdef0123456789abcdef", "http://h/pair#code=ABCD1234"] {
            assert_eq!(invitation(bad), None, "{bad}");
        }
    }
    #[test] fn a_short_code_forgives_spaces_dashes_and_case() {
        assert_eq!(from_code("100.1.2.3:3801", "abcd-2345"), code("http://100.1.2.3:3801", "ABCD2345"));
        assert_eq!(from_code(" https://box.ts.net:8461/ ", "ABCD 2345"), code("https://box.ts.net:8461", "ABCD2345"));
        assert_eq!(from_code("h:1", "ABCD234"), None); assert_eq!(from_code("h:1", "ABCD1234"), None, "1 is not in the alphabet"); assert_eq!(from_code("", "ABCD2345"), None);
    }
    #[test] fn the_form_prefers_the_link_and_says_what_is_missing() {
        assert_eq!(submit(&format!("http://h:1/pair#invite={SECRET}"), "other:2", "ABCD2345"), Ok(inv("http://h:1", SECRET).unwrap()));
        assert_eq!(submit("", "h:1", "ABCD2345"), Ok(code("http://h:1", "ABCD2345").unwrap()));
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
    #[test] fn a_link_naming_only_the_hubs_own_computer_is_refused_when_no_hub_answers_here_with_the_reason() {
        let closed = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();   // (bound, then dropped: nothing listens)
        let l = format!("http://127.0.0.1:{closed}/pair#invite={SECRET}&routes=%5B%22http%3A%2F%2F127.0.0.1%3A{closed}%22%5D");
        assert!(submit(&l, "", "").unwrap_err().contains("own computer"), "no hub on this computer: refused, with the phones' words");
        assert!(submit(&format!("http://localhost:{closed}/pair#invite={SECRET}"), "", "").unwrap_err().contains("own computer"), "localhost too");
        let ok = format!("http://127.0.0.1:3797/pair#invite={SECRET}&routes=%5B%22http%3A%2F%2F100.64.1.2%3A3797%22%5D");
        assert_eq!(submit(&ok, "", "").unwrap().routes, vec!["http://127.0.0.1:3797", "http://100.64.1.2:3797"], "one reachable address is enough");
        assert!(submit("", "127.0.0.1:3801", "ABCD2345").is_ok(), "an address the person typed is their word");
        assert!(!loopback("http://localhost.evil.com:1") && loopback("http://localhost:1") && loopback("http://[::1]:1") && !loopback("http://127.evil.com:1"));
    }
    #[test] fn a_link_naming_only_the_hubs_own_computer_pairs_when_that_hub_is_on_this_computer() {
        // a one-shot hub on this computer's loopback: the desktop client beside its hub takes the link and pairs
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap(); let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || { for _ in 0..2 { let Ok((mut s, _)) = l.accept() else { return }; let mut b = [0u8; 2048]; let n = s.read(&mut b).unwrap_or(0);
            if n == 0 { continue }   // (the reachability probe connects and closes)
            let body = format!(r#"{{"token":"{SECRET}"}}"#); let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()); return } });
        let link = format!("http://127.0.0.1:{port}/pair#invite={SECRET}&routes=%5B%22http%3A%2F%2F127.0.0.1%3A{port}%22%5D");
        let i = submit(&link, "", "").expect("a hub answers here: the link is taken");
        assert_eq!(exchange(&i, "t"), Ok((format!("http://127.0.0.1:{port}"), SECRET.to_string())), "and it pairs");
        h.join().unwrap();
    }
    #[test] fn every_address_is_tried_in_order_and_only_an_unreachable_one_moves_on() {
        // nothing listens on the first (a closed port); the second is a one-shot hub answering the exchange
        let closed = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap(); let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || { let (mut s, _) = l.accept().unwrap(); let mut b = [0u8; 2048]; let _ = s.read(&mut b);
            let body = format!(r#"{{"token":"{SECRET}"}}"#); let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()); });
        let mut i = invitation(&format!("http://127.0.0.1:{closed}/pair#invite={SECRET}")).unwrap();
        i.routes = vec![format!("http://127.0.0.1:{closed}"), format!("http://127.0.0.1:{port}")];
        assert_eq!(exchange(&i, "t"), Ok((format!("http://127.0.0.1:{port}"), SECRET.to_string())));
        h.join().unwrap();
        i.routes = vec![format!("http://127.0.0.1:{closed}")];
        assert_eq!(exchange(&i, "t"), Err(message("hub_unreachable").to_string()));
    }
    #[test] fn a_device_name_can_go_in_a_header() { let n = name(); assert!(!n.is_empty() && n.is_ascii() && !n.contains(['\r', '\n'])); }
}
