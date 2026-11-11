//! server/botnet's peer side: names and ids, secrets kept as files under <home>/secrets/peers, signed
//! requests out (curl with the signed headers on stdin and the body from a 0600 file, so a secret never reaches
//! argv or a log), and verifying signed requests in. Nothing here talks to the hub.
use super::sign;
use std::{path::{Path, PathBuf}, process::Stdio};
use tokio::{io::AsyncWriteExt, process::Command};

/// A reply from a peer: curl printed the body, then a line with the status code. Code 0: no request was made.
pub struct Resp { pub code: u16, pub body: String }

fn idc(c: char) -> bool { c.is_ascii_alphanumeric() || c == '-' || c == '_' }
/// An id that may name a file: letters, digits, - and _ (no dots, no slashes), 1 to 80 of them.
pub fn safe(id: &str) -> bool { !id.is_empty() && id.chars().count() <= 80 && id.chars().all(idc) }
/// A name another machine told us, kept to letters, digits, . - _ (so it can never hide an @ or a path), at most 40.
pub fn clean(s: &str) -> String { s.chars().filter(|c| idc(*c) || *c == '.').take(40).collect() }
/// An address we may send to: http(s), printable, no spaces, not too long.
pub fn url_ok(u: &str) -> bool { (u.starts_with("http://") || u.starts_with("https://")) && u.chars().count() <= 300 && u.chars().all(|c| (c as u32) > 32 && (c as u32) < 127) }
pub fn url_base(u: &str) -> &str { u.strip_suffix('/').unwrap_or(u) }

pub fn peer_path(home: &Path, id: &str) -> PathBuf { home.join("secrets/peers").join(id) }
pub fn mkdir(path: &Path) { use crate::plat::fs::DirBuilderExt; let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(path); }
/// A file only its owner reads (0600, set before anything is written).
pub fn write_secret(path: &Path, text: &str) -> bool { write_secret_bytes(path, text.as_bytes()) }
pub fn write_secret_bytes(path: &Path, bytes: &[u8]) -> bool {
    use std::io::Write; use crate::plat::fs::OpenOptionsExt;
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let ok = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp).and_then(|mut f| f.write_all(bytes).and_then(|_| f.sync_all())).is_ok();
    ok && crate::plat::fs::rename(&tmp, path).is_ok()
}
/// 64 hex digits: 32 random bytes.
pub fn fresh_secret() -> String { hex::encode(super::pairing::urandom(32)) }
/// A linked machine's secret, kept as {"secret": hex}.
pub fn peer_save(home: &Path, id: &str, secret: &str) -> bool {
    if !safe(id) { return false }
    mkdir(&home.join("secrets")); mkdir(&home.join("secrets/peers"));
    write_secret(&peer_path(home, id), &serde_json::json!({"secret": secret}).to_string())
}
/// A linked machine's secret; "" when there is none.
pub fn peer_secret(home: &Path, id: &str) -> String {
    if !safe(id) { return String::new() }
    std::fs::read_to_string(peer_path(home, id)).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|j| j["secret"].as_str().map(str::to_string)).unwrap_or_default()
}
pub fn peer_forget(home: &Path, id: &str) { if safe(id) { let _ = std::fs::remove_file(peer_path(home, id)); } }

/// Bn.check for a linked machine's request: the replay key of a genuine one ("" when it is not): fresh, matching,
/// compared in constant time. `key` names what was signed for (the peer id).
pub fn check_peer(secret: &str, now: u64, key: &str, ts: &str, sig: &str, body: &str) -> String {
    if secret.is_empty() || sig.is_empty() { return String::new() }
    let Some(ts) = (!ts.is_empty() && ts.bytes().all(|b| b.is_ascii_digit())).then(|| ts.parse::<u64>().ok()).flatten() else { return String::new() };
    if sign::check(secret, now, ts, body, sig) { format!("{key}:s:{sig}") } else { String::new() }
}

pub fn headers(peer: &str, now: u64, sig: &str) -> String {
    format!("X-Backplane-Peer: {peer}\nX-Backplane-Timestamp: {now}\nX-Backplane-Signature: {sig}\nContent-Type: application/json\n")
}
/// No redirects, http(s) only, bounded time and size; POST sends the body from `file`.
pub fn args(post_file: Option<&Path>, url: &str) -> Vec<String> {
    let mut a: Vec<String> = ["-sS", "--proto", "=http,https", "-m", "15", "--max-filesize", "1048576", "-X", if post_file.is_some() { "POST" } else { "GET" }, "-H", "@-"].map(String::from).into();
    if let Some(f) = post_file { a.push("--data-binary".into()); a.push(format!("@{}", f.display())) }
    a.extend(["-w", "\n%{http_code}", "--url"].map(String::from)); a.push(url.into()); a
}
fn parse_resp(out: &[u8]) -> Resp {
    let t = String::from_utf8_lossy(out); let t = t.as_ref();
    match t.rsplit_once('\n') { Some((body, code)) => Resp { code: code.trim().parse().unwrap_or(0), body: body.to_string() }, None => Resp { code: 0, body: t.to_string() } }
}
/// curl with its headers on stdin; stderr dropped; none from a hub on a copied log.
pub async fn curl(args: Vec<String>, headers: String) -> Resp {
    if super::super::copy::on() { return Resp { code: 0, body: "a hub on a copied log does not run curl".into() } }
    let Ok(mut c) = crate::proc::tokio_cmd(crate::proc::Kind::Net, "curl").args(&args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn() else { return Resp { code: 0, body: "curl is not installed".into() } };
    if let Some(mut i) = c.stdin.take() { let _ = i.write_all(headers.as_bytes()).await; }
    match c.wait_with_output().await { Ok(o) => parse_resp(&o.stdout), Err(_) => Resp { code: 0, body: String::new() } }
}
/// A request to a linked machine, signed with its secret (POST with the body, or GET over the empty body); code 0
/// when it could not be made.
pub async fn signed(home: &Path, url: &str, peer: &str, secret: &str, now: u64, path: &str, post: bool, body: &str) -> Resp {
    if !url_ok(url) || secret.is_empty() { return Resp { code: 0, body: "no secret, or not an http address".into() } }
    let target = format!("{}{path}", url_base(url));
    let sig = sign::header(secret, now, body);
    // a linked machine this hub cannot reach, polling it (net/alwayson/tunnel.rs): the call goes back over its poll instead
    if path != "/far/tunnel" {
        if let Some((code, body)) = super::alwayson::tunnel::ends().call(peer, if post { "POST" } else { "GET" }, path, &headers(peer, now, &sig), if post { body } else { "" }).await { return Resp { code, body } }
    }
    if !post { return curl(args(None, &target), headers(peer, now, &sig)).await }
    mkdir(&home.join("secrets")); mkdir(&home.join("secrets/tmp"));
    let file = home.join("secrets/tmp").join(hex::encode(super::pairing::urandom(8)));
    if !write_secret(&file, body) { return Resp { code: 0, body: "could not keep the body".into() } }
    let r = curl(args(Some(&file), &target), headers(peer, now, &sig)).await;
    let _ = std::fs::remove_file(&file); r
}
/// A signed request to a peer whose secret is in its file.
pub async fn to_peer(home: &Path, url: &str, peer: &str, now: u64, path: &str, post: bool, body: &str) -> Resp {
    signed(home, url, peer, &peer_secret(home, peer), now, path, post, body).await
}
/// An unsigned POST of a JSON body (the pairing request and its poll: a machine code, not a signature, admits them); code 0
/// when it could not be made.
pub async fn post_json(home: &Path, url: &str, body: &str) -> Resp {
    if !url_ok(url) { return Resp { code: 0, body: "not an http address".into() } }
    mkdir(&home.join("secrets")); mkdir(&home.join("secrets/tmp"));
    let file = home.join("secrets/tmp").join(hex::encode(super::pairing::urandom(8)));
    if !write_secret(&file, body) { return Resp { code: 0, body: "could not keep the body".into() } }
    let mut a = args(Some(&file), url); a.insert(a.len() - 2, "-m".into()); a.insert(a.len() - 2, "10".into());
    let r = curl(a, "Content-Type: application/json\n".into()).await;
    let _ = std::fs::remove_file(&file); r
}

/// Where other machines reach this hub: BACKPLANE_URL when set, else its MagicDNS name on the tailnet (the hub listens there on the same port), else the
/// https address `tailscale serve` gives a hub that listens only on loopback (it is not at its own port on the tailnet), else its listen address.
pub async fn url_here(tailnet: bool, host: &str, port: u16) -> String {
    let env = std::env::var("BACKPLANE_URL").unwrap_or_default();
    let dns = if env.trim().is_empty() && tailnet { tailscale_dns().await } else { String::new() };
    let serve = if env.trim().is_empty() && !tailnet && super::machines::loopback(&format!("http://{host}")) { served_url(port).await } else { String::new() };
    choose_url(&env, tailnet, &dns, &serve, host, port)
}
/// The pure choice of `url_here`: `env` BACKPLANE_URL, `dns` the MagicDNS name ("" none), `serve` the https address `tailscale serve` maps to this port ("" none).
pub fn choose_url(env: &str, tailnet: bool, dns: &str, serve: &str, host: &str, port: u16) -> String {
    if !env.trim().is_empty() { return env.trim().to_string() }
    if tailnet && !dns.is_empty() { return format!("http://{dns}:{port}") }
    if !serve.is_empty() { return serve.to_string() }
    format!("http://{host}:{port}")
}
/// The https address `tailscale serve` has for this port ("" when none, or no tailscale); asked at most every 30 seconds.
pub(crate) async fn served_url(port: u16) -> String {
    static LAST: std::sync::Mutex<Option<(std::time::Instant, u16, String)>> = std::sync::Mutex::new(None);
    if let Some((t, p, u)) = LAST.lock().unwrap().clone() { if p == port && t.elapsed() < std::time::Duration::from_secs(30) { return u } }
    let out = tokio::time::timeout(std::time::Duration::from_secs(5), crate::proc::tokio_cmd(crate::proc::Kind::Net, "tailscale").args(["serve", "status", "--json"]).stderr(Stdio::null()).output()).await;
    let u = match out { Ok(Ok(o)) if o.status.success() => super::tailnet::serve(&String::from_utf8_lossy(&o.stdout), &port.to_string()), _ => String::new() };
    *LAST.lock().unwrap() = Some((std::time::Instant::now(), port, u.clone())); u
}
/// Self.DNSName of `tailscale status --json`, without the trailing dot; "" when there is none.
pub async fn tailscale_dns() -> String {
    let Ok(o) = tokio::time::timeout(std::time::Duration::from_secs(5), crate::proc::tokio_cmd(crate::proc::Kind::Net, "tailscale").args(["status", "--json"]).stderr(Stdio::null()).output()).await else { return String::new() };
    let Ok(o) = o else { return String::new() };
    serde_json::from_slice::<serde_json::Value>(&o.stdout).ok().and_then(|j| j["Self"]["DNSName"].as_str().map(|s| s.trim_end_matches('.').to_string())).unwrap_or_default()
}
/// This machine's name for others: BACKPLANE_NAME, else its host name.
pub async fn name() -> String {
    if let Ok(e) = std::env::var("BACKPLANE_NAME") { if !e.trim().is_empty() { return clean(e.trim()) } }
    let h = crate::proc::tokio_cmd(crate::proc::Kind::Local, "hostname").stderr(Stdio::null()).output().await.ok().map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").trim().to_string()).unwrap_or_default();
    clean(&h)
}

/// An unsigned POST with no body (tests: a route that must refuse it).
#[cfg(test)]
pub async fn plain(url: &str) -> Resp {
    if !url_ok(url) { return Resp { code: 0, body: "not an http address".into() } }
    let a = ["-sS", "--proto", "=http,https", "-m", "15", "--max-filesize", "65536", "-X", "POST", "-H", "@-", "-w", "\n%{http_code}", "--url"].map(String::from).into_iter().chain([url.to_string()]).collect();
    curl(a, "Content-Length: 0\n".into()).await
}

#[cfg(test)]
mod tests {
    #[test] fn a_loopback_hub_behind_tailscale_serve_names_its_https_address() {
        use super::choose_url as c;
        assert_eq!(c("", false, "", "https://box.ts.net:8451", "127.0.0.1", 3797), "https://box.ts.net:8451");
        assert_eq!(c("http://pinned:1", false, "", "https://box.ts.net:8451", "127.0.0.1", 3797), "http://pinned:1", "BACKPLANE_URL first");
        assert_eq!(c("", true, "box.ts.net", "https://box.ts.net:8451", "127.0.0.1", 3797), "http://box.ts.net:3797", "on the tailnet it listens there itself");
        assert_eq!(c("", false, "", "", "127.0.0.1", 3797), "http://127.0.0.1:3797");
    }
    use super::*;
    fn tmp(tag: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("botnet-{tag}")) }

    #[test] fn bot_ids_and_names_are_cleaned() {
        assert!(safe("p0123") && safe("a-b_C") && !safe("") && !safe("a.b") && !safe("a/b") && !safe(&"a".repeat(81)) && safe(&"a".repeat(80)));
        assert_eq!(clean("Desk 1/../@x.y"), "Desk1..x.y");
        assert_eq!(clean(&"a".repeat(60)).len(), 40);
        assert!(url_ok("http://h:1") && url_ok("https://h") && !url_ok("ftp://h") && !url_ok("http://h x") && !url_ok(&format!("http://{}", "a".repeat(300))) && !url_ok("http://h\n"));
        assert_eq!((url_base("http://h/"), url_base("http://h")), ("http://h", "http://h"));
    }
    #[test] fn peer_secrets_are_files_only_the_owner_reads() {
        use crate::plat::fs::PermissionsExt;
        let h = tmp("sec"); let s = fresh_secret();
        assert_eq!(s.len(), 64);
        assert!(peer_save(&h, "p1", &s) && !peer_save(&h, "../p1", &s));
        assert_eq!(peer_secret(&h, "p1"), s);
        assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(peer_path(&h, "p1")).unwrap().permissions().mode() & 0o777, 0o600), 0o600);
        assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(h.join("secrets/peers")).unwrap().permissions().mode() & 0o777, 0o700), 0o700);
        assert_eq!(peer_secret(&h, "../p1"), "");
        peer_forget(&h, "p1"); assert_eq!(peer_secret(&h, "p1"), "");
    }
    #[test] fn bot_check_peer_accepts_only_fresh_matching_signatures() {
        let s = fresh_secret(); let body = "{\"a\":1}";
        let sig = sign::header(&s, 1000, body);
        assert_eq!(check_peer(&s, 1000, "p1", "1000", &sig, body), format!("p1:s:{sig}"));
        assert_eq!(check_peer(&s, 1400, "p1", "1000", &sig, body), "", "stale");
        assert_eq!(check_peer(&s, 1000, "p1", "1000", &sig, "{}"), "", "another body");
        assert_eq!(check_peer("", 1000, "p1", "1000", &sig, body), "", "no secret lets nothing in");
        assert_eq!(check_peer(&s, 1000, "p1", "1000", "", body), "");
        assert_eq!(check_peer(&s, 1000, "p1", "10x0", &sig, body), "", "a timestamp that is not digits");
        assert_eq!(check_peer(&s, 1000, "p1", "", &sig, body), "");
    }
    #[test] fn bot_curl_args_keep_secrets_off_argv() {
        let a = args(Some(Path::new("/h/secrets/tmp/x")), "http://h:1/far/rpc");
        assert!(a.contains(&"@-".to_string()) && a.contains(&"--data-binary".to_string()) && a.contains(&"@/h/secrets/tmp/x".to_string()));
        assert_eq!(a.last().unwrap(), "http://h:1/far/rpc");
        assert!(!a.iter().any(|x| x.contains("sha256=") || x.contains("X-Backplane")));
        assert_eq!(headers("p1", 5, "sha256=ab"), "X-Backplane-Peer: p1\nX-Backplane-Timestamp: 5\nX-Backplane-Signature: sha256=ab\nContent-Type: application/json\n");
        assert!(args(None, "http://h").contains(&"GET".to_string()));
    }
    #[test] fn bot_resp_is_body_then_code() {
        let r = parse_resp(b"{\"invite\":\"x\"}\n200"); assert_eq!((r.code, r.body.as_str()), (200, "{\"invite\":\"x\"}"));
        let r = parse_resp(b"line1\nline2\n403"); assert_eq!((r.code, r.body.as_str()), (403, "line1\nline2"));
        assert_eq!(parse_resp(b"").code, 0);
    }
}
