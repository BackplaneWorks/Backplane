//! The APNs carrier: an ES256 token signed with `openssl`, sent with `curl` over HTTP/2. As the webhook does, the token and the device stay in a 0600 config in a 0700 folder,
//! never on a command line or in a log. The key file is named by the setting `push.apns.key` (only the path is stored), with `push.apns.keyid` and `push.apns.team`.
//! A copy of a log sends nothing. `BACKPLANE_PUSH_SINK=<folder>` (a test hub) writes each push there instead of sending it.
use super::{Carrier, Say, Sent};
use crate::{bots::world::Device, hub::{now_ms, Hub}};
use base64::Engine;
use serde_json::{json, Value};
use std::{io::Write, path::Path, sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Arc, Mutex}}; use crate::plat::fs::{DirBuilderExt, OpenOptionsExt};

/// Apple accepts a token for an hour, and refuses one refreshed too often: this is in between.
const TOKEN_S: u64 = 40 * 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Frame { pub push_type: &'static str, pub priority: u8, pub collapse: String, pub payload: Value }
impl Frame {
    /// An alert: `thread-id` groups by thread, the collapse id is the notification's identifier on the phone, and `key` is how a clear finds it.
    pub fn alert(s: &Say, badge: u64) -> Frame {
        Frame { push_type: "alert", priority: 10, collapse: s.key.clone(), payload: json!({"aps":{"alert":{"title":s.title,"body":s.body},"badge":badge,"sound":"default","thread-id":s.thread},
            "thread":s.thread,"ask":s.ask,"key":s.key,"kind":s.reason,"link":s.link,"hub":crate::push::hub_url()}) }
    }
    /// A silent clear: the badge, and the notifications to take away. It shows nothing.
    pub fn clear(keys: &[String], badge: u64) -> Frame { Frame { push_type: "background", priority: 5, collapse: String::new(), payload: json!({"aps":{"content-available":1,"badge":badge},"clear":keys}) } }
}

pub fn host(env: &str) -> &'static str { if env == "sandbox" { "https://api.sandbox.push.apple.com" } else { "https://api.push.apple.com" } }
/// What Apple's answer means. A token refused for good is retired; anything else may pass. `TopicDisallowed` is not the token's fault: the provider key may not push to this
/// topic (a key restricted to another app; Nov 10 every push was refused so, and the phone's token was retired each time), so it is kept and the answer logged.
pub fn outcome(status: &str, body: &str) -> Sent {
    let why = serde_json::from_str::<Value>(body).ok().and_then(|v| v["reason"].as_str().map(str::to_string)).unwrap_or_default();
    match status {
        "200" => Sent::Ok,
        "410" => Sent::Dead,
        "400" | "403" if matches!(why.as_str(), "BadDeviceToken" | "DeviceTokenNotForTopic" | "Unregistered") => Sent::Dead,
        _ => Sent::Later(format!("APNs {status} {why}")),
    }
}
fn b64(b: &[u8]) -> String { base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b) }
/// A DER ECDSA signature as the 64 bytes JWS wants.
pub fn raw_sig(der: &[u8]) -> Option<[u8; 64]> {
    let mut p = 2; if der.first()? != &0x30 { return None } if der.get(1)? & 0x80 != 0 { p += (der[1] & 0x7f) as usize }
    let mut out = [0u8; 64];
    for half in 0..2 {
        if der.get(p)? != &0x02 { return None } let n = *der.get(p + 1)? as usize; let v = der.get(p + 2..p + 2 + n)?; p += 2 + n;
        let v = &v[v.iter().position(|b| *b != 0).unwrap_or(v.len())..]; if v.len() > 32 { return None }
        out[half * 32 + 32 - v.len()..(half + 1) * 32].copy_from_slice(v);
    }
    Some(out)
}
/// The provider token.
pub fn token(key: &Path, kid: &str, team: &str, now: u64) -> Result<String, String> {
    let msg = format!("{}.{}", b64(json!({"alg":"ES256","kid":kid}).to_string().as_bytes()), b64(json!({"iss":team,"iat":now}).to_string().as_bytes()));
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, "openssl");
    c.args(["dgst", "-sha256", "-sign"]).arg(key).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    let mut ch = c.spawn().map_err(|_| "openssl is missing".to_string())?;
    ch.stdin.take().ok_or("no stdin")?.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;
    let o = ch.wait_with_output().map_err(|e| e.to_string())?;
    if !o.status.success() { return Err("could not sign with push.apns.key".into()) }
    Ok(format!("{msg}.{}", b64(&raw_sig(&o.stdout).ok_or("the signature did not parse")?)))
}
pub fn curl_config(url: &str, jwt: &str, topic: &str, f: &Frame, body: &Path, out: &Path) -> String {
    let mut c = format!("url = \"{url}\"\nheader = \"authorization: bearer {jwt}\"\nheader = \"apns-topic: {topic}\"\nheader = \"apns-push-type: {}\"\nheader = \"apns-priority: {}\"\n", f.push_type, f.priority);
    if !f.collapse.is_empty() { c += &format!("header = \"apns-collapse-id: {}\"\n", f.collapse) }
    c + &format!("data-binary = \"@{}\"\noutput = \"{}\"\nwrite-out = \"%{{http_code}} %header{{apns-id}}\"\n", body.display(), out.display())
}

#[derive(Default)]
pub struct Apns { jwt: Mutex<(String, u64)>, told: AtomicBool, sunk: AtomicU64 }
impl Apns {
    pub fn new() -> Apns { Apns::default() }
    fn jwt(&self, key: &str, kid: &str, team: &str, now: u64) -> Result<String, String> {
        let mut g = self.jwt.lock().unwrap();
        if g.0.is_empty() || now >= g.1 + TOKEN_S { *g = (token(Path::new(key), kid, team, now)?, now) }
        Ok(g.0.clone())
    }
}
/// Apple's answer: the status, the body, and the `apns-id` it gave the push.
fn post(home: &Path, url: &str, jwt: &str, topic: &str, f: &Frame) -> Result<(String, String, String), String> {
    let stamp = now_ms(); let tmp = home.join("tmp"); std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let d = tmp.join(format!("push-{stamp}-{:x}", std::process::id()));
    std::fs::DirBuilder::new().mode(0o700).create(&d).map_err(|e| e.to_string())?;
    let (body, out, cfg) = (d.join("body.json"), d.join("out"), d.join("curl"));
    let w = |p: &Path, t: &str| std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(p).and_then(|mut x| x.write_all(t.as_bytes()));
    let r = (|| {
        w(&body, &f.payload.to_string()).map_err(|e| e.to_string())?;
        w(&cfg, &curl_config(url, jwt, topic, f, &body, &out)).map_err(|e| e.to_string())?;
        let o = crate::proc::std_cmd(crate::proc::Kind::Net, "curl").args(["-sS", "--http2", "--max-time", "10", "--retry", "2", "--retry-max-time", "20", "--proto", "=https", "--max-redirs", "0", "-X", "POST", "-K"]).arg(&cfg).current_dir(&d).output().map_err(|e| e.to_string())?;
        let w = String::from_utf8_lossy(&o.stdout).trim().to_string(); let (code, id) = w.split_once(' ').unwrap_or((w.as_str(), ""));
        Ok((code.to_string(), std::fs::read_to_string(&out).unwrap_or_default(), id.trim().to_string()))
    })();
    let _ = std::fs::remove_dir_all(&d);
    r
}
impl Carrier for Apns {
    fn send(self: Arc<Self>, hub: Arc<Hub>, d: Device, f: Frame) -> futures_util::future::BoxFuture<'static, Sent> {
        Box::pin(async move {
            if crate::copy::on() { return Sent::Later("a copy of a log sends nothing".into()) }
            if let Ok(dir) = std::env::var("BACKPLANE_PUSH_SINK") {
                let n = self.sunk.fetch_add(1, Ordering::SeqCst);
                let line = json!({"type":f.push_type,"priority":f.priority,"collapse":f.collapse,"topic":d.bundle,"env":d.env,"payload":f.payload});
                return match std::fs::write(Path::new(&dir).join(format!("{n:04}-{}.json", f.push_type)), line.to_string()) { Ok(_) => Sent::Ok, Err(e) => Sent::Later(e.to_string()) }
            }
            let Some((key, kid, team, _)) = credentials(&hub).await else {
                if !self.told.swap(true, Ordering::SeqCst) { eprintln!("backplane: push: phones are registered but push.apns.key, push.apns.keyid and push.apns.team are not all set (nor ~/.config/backplane/apns.env), or the key file is missing") }
                return Sent::Later("APNs is not configured".into())
            };
            let jwt = match self.jwt(&key, &kid, &team, now_ms() / 1000) { Ok(j) => j, Err(e) => return Sent::Later(e) };
            let (url, home, topic) = (format!("{}/3/device/{}", host(&d.env), d.token), hub.cfg.home.clone(), d.bundle.clone());
            let what = format!("{} {} to {} ({} {})", f.push_type, f.collapse, super::page::handle(&d.token), d.env, d.bundle);
            match tokio::task::spawn_blocking(move || post(&home, &url, &jwt, &topic, &f)).await {
                // every send and Apple's answer, in the log (the device by its handle, never its token)
                Ok(Ok((code, body, id))) => { let s = outcome(&code, &body); eprintln!("{}", said(&what, &code, &body, &id, &s)); s }
                Ok(Err(e)) => { eprintln!("push: APNs {what}: not sent: {e}"); Sent::Later(e) }
                Err(_) => Sent::Later("the delivery task failed".into()),
            }
        })
    }
}

/// The log line for one send: what went, Apple's status, reason and apns-id, and what the hub does with it.
pub fn said(what: &str, code: &str, body: &str, id: &str, s: &Sent) -> String {
    let why = serde_json::from_str::<Value>(body).ok().and_then(|v| v["reason"].as_str().map(str::to_string)).unwrap_or_default();
    let then = match s { Sent::Ok => "", Sent::Dead => ": the token is retired", Sent::Later(_) => ": kept" };
    format!("push: APNs {what}: {code}{}{} apns-id {}{then}", if why.is_empty() { "" } else { " " }, why, if id.is_empty() { "-" } else { id })
}
/// Where the fallback is read: `BACKPLANE_APNS_ENV`, else `~/.config/backplane/apns.env` (the key file defaults to `apns/AuthKey_<key id>.p8` in the same folder).
pub fn env_path() -> Option<std::path::PathBuf> {
    std::env::var_os("BACKPLANE_APNS_ENV").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config/backplane/apns.env")))
}
/// `export NAME=value` lines (quotes allowed) as pairs; anything else is skipped.
fn env_pairs(text: &str) -> Vec<(String, String)> {
    text.lines().filter_map(|l| {
        let l = l.trim().strip_prefix("export ").unwrap_or(l.trim()); if l.starts_with('#') { return None }
        let (k, v) = l.split_once('=')?; Some((k.trim().to_string(), v.trim().trim_matches(|c| c == '"' || c == '\'').to_string()))
    }).collect()
}
/// The three values push needs: each setting when it is set, else the env file's (`BACKPLANE_APNS_KEY_ID`, `BACKPLANE_APNS_TEAM_ID`, `BACKPLANE_APNS_PRIVATE_KEY_PATH`, else the
/// key file beside it named for the key id). None unless all three are there and the key file exists. The last is where they came from; the values are never logged.
pub fn resolve(set: (String, String, String), env: Option<(&str, &Path)>) -> Option<(String, String, String, &'static str)> {
    let (mut key, mut kid, mut team) = set; let mut from = "settings";
    if let Some((text, dir)) = env {
        let pairs = env_pairs(text); let get = |k: &str| pairs.iter().find(|p| p.0 == k).map(|p| p.1.clone()).unwrap_or_default();
        let used = key.is_empty() || kid.is_empty() || team.is_empty();
        if kid.is_empty() { kid = get("BACKPLANE_APNS_KEY_ID") }
        if team.is_empty() { team = get("BACKPLANE_APNS_TEAM_ID") }
        if key.is_empty() { key = get("BACKPLANE_APNS_PRIVATE_KEY_PATH"); if key.is_empty() && !kid.is_empty() { key = dir.join("apns").join(format!("AuthKey_{kid}.p8")).to_string_lossy().into_owned() } }
        if used { from = "apns.env" }
    }
    (!key.is_empty() && !kid.is_empty() && !team.is_empty() && Path::new(&key).is_file()).then_some((key, kid, team, from))
}
/// The hub's push credentials: its settings, then the env file.
pub async fn credentials(hub: &Arc<Hub>) -> Option<(String, String, String, &'static str)> {
    let get = |k: &'static str| { let hub = hub.clone(); async move { hub.setting(k).await.unwrap_or_default() } };
    let set = (get("push.apns.key").await, get("push.apns.keyid").await, get("push.apns.team").await);
    let env = env_path().and_then(|p| std::fs::read_to_string(&p).ok().map(|t| (t, p)));
    resolve(set, env.as_ref().map(|(t, p)| (t.as_str(), p.parent().unwrap_or(Path::new(".")))))
}
