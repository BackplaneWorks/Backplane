//! Disposable invitations and durable, individually revocable devices: server/effects/pairing.js's
//! `pairingStore`, field for field, over the same `<home>/devices-v1.json` (a registry either hub reads).
//! One hub owns the home lock; every operation runs under one mutex, so an invitation cannot be redeemed
//! twice across overlapping requests. Only hashes are stored. Only a recorded, unexpired, unused,
//! unrevoked invitation enrolls; revoking a used one revokes the device it paired.
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{collections::{HashMap, HashSet}, io::Write, path::{Path, PathBuf}, sync::Mutex};

use super::owner::same;

pub fn digest(x: &str) -> String { hex::encode(Sha256::digest(x.as_bytes())) }
fn is_hex(x: &Value, n: usize) -> bool { x.as_str().map_or(false, |s| s.len() == n && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))) }
fn is_peer(x: &str) -> bool { x.len() == 17 && x.starts_with('p') && x[1..].bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) }
fn num(x: &Value) -> bool { x.as_f64().map_or(false, f64::is_finite) }
fn clean(s: &str, max: usize) -> String { s.chars().filter(|c| !matches!(*c, '\u{0}'..='\u{1f}' | '\u{7f}')).take(max).collect() }
pub fn urandom(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut b)).expect("/dev/urandom");
    b
}

const KEEP: usize = 100;
const OPEN_LIMIT: usize = 16;
const DEVICE_LIMIT: usize = 256;
const INVITE_MS: u64 = 600_000;

struct Inner {
    devices: Vec<Value>, invites: Vec<Value>, attempts: Vec<u64>,
    clients: HashMap<u64, String>, retired: HashSet<u64>, swept: HashSet<String>,
}
pub struct Registry {
    home: PathBuf, root: String, file: PathBuf,
    now: Box<dyn Fn() -> u64 + Send + Sync>, random: Box<dyn Fn(usize) -> Vec<u8> + Send + Sync>,
    inner: Mutex<Inner>,
}

/// JS truthiness of a stored field (`!d.revoked`): a device's `revoked` is never validated at load, so
/// 1, "yes", {} and [] all count as revoked, as they do in effects/pairing.js.
pub fn truthy(v: &Value) -> bool {
    match v { Value::Null => false, Value::Bool(b) => *b, Value::Number(n) => n.as_f64().map_or(true, |f| f != 0.0 && !f.is_nan()), Value::String(s) => !s.is_empty(), _ => true }
}
fn live(d: &Value) -> bool { !truthy(&d["revoked"]) }
fn is_machine(x: &Value) -> bool { x["kind"] == "machine" }
fn open(x: &Value, at: u64) -> bool { x["used"] != true && x["revoked"] != true && x["expiresAt"].as_f64().unwrap_or(0.0) > at as f64 }
fn valid_invite(x: &Value) -> bool {
    let kind_ok = match x.get("kind") { None => true, Some(Value::String(k)) if k == "machine" => x["peer"].as_str().map_or(false, is_peer)
        && x.get("name").map_or(true, Value::is_string) && x.get("unlinked").map_or(true, Value::is_boolean), _ => false };
    kind_ok && is_hex(&x["id"], 32) && is_hex(&x["hash"], 64) && is_hex(&x["codeHash"], 64) && num(&x["createdAt"]) && num(&x["expiresAt"])
        && x["used"].is_boolean() && x["revoked"].is_boolean() && x.get("device").map_or(true, |d| is_hex(d, 32)) && x.get("usedAt").map_or(true, num)
}
fn valid_device(d: &Value) -> bool { is_hex(&d["id"], 32) && is_hex(&d["hash"], 64) && d["name"].is_string() && num(&d["lastSeen"]) }

impl Registry {
    /// Errors (and starts nothing) on a registry that is not valid or belongs to another hub token: existing
    /// pairings are preserved, never overwritten.
    pub fn open(home: &Path, root: &str) -> Result<Registry, String> {
        Self::with(home, root, Box::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)), Box::new(urandom))
    }
    /// A registry that holds nothing and persists nothing: what the routes get when the real file cannot be read. The file is not touched.
    pub fn empty(home: &Path) -> Registry {
        let file = home.join("guard").join("devices-unreadable.json");
        Registry { home: home.into(), root: String::new(), file, now: Box::new(|| 0), random: Box::new(urandom), inner: Mutex::new(Inner { devices: vec![], invites: vec![], attempts: vec![], clients: HashMap::new(), retired: HashSet::new(), swept: HashSet::new() }) }
    }
    pub fn with(home: &Path, root: &str, now: Box<dyn Fn() -> u64 + Send + Sync>, random: Box<dyn Fn(usize) -> Vec<u8> + Send + Sync>) -> Result<Registry, String> {
        let file = home.join("devices-v1.json");
        let (mut devices, mut invites) = (vec![], vec![]);
        if file.exists() {
            let bad = || "Invalid device registry".to_string();
            let data: Value = serde_json::from_str(&std::fs::read_to_string(&file).map_err(|_| bad())?).map_err(|_| bad())?;
            let ds = data["devices"].as_array().ok_or_else(bad)?;
            if data["version"] != 1 || data["owner"] != digest(root).as_str() || !ds.iter().all(valid_device) { return Err(bad()) }
            if let Some(is) = data.get("invitations") { if !is.as_array().map_or(false, |a| a.iter().all(valid_invite)) { return Err(bad()) } }
            devices = ds.clone(); invites = data["invitations"].as_array().cloned().unwrap_or_default();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).map_err(|_| "Cannot read or initialize device registry; existing pairings were preserved".to_string())?;
        }
        Ok(Registry { home: home.into(), root: root.into(), file, now, random, inner: Mutex::new(Inner { devices, invites, attempts: vec![], clients: HashMap::new(), retired: HashSet::new(), swept: HashSet::new() }) })
    }
    fn rand_hex(&self, n: usize) -> String { hex::encode((self.random)(n)) }

    /// Atomic write: temp file (0600, exclusive), fsync, rename, fsync the folder. State changes only after.
    fn persist(&self, g: &mut Inner, devices: Vec<Value>, invites: Vec<Value>) -> Result<(), String> {
        use std::os::unix::fs::OpenOptionsExt;
        let tmp = self.file.with_extension(format!("json.{}.tmp", self.rand_hex(8)));
        let body = json!({"version": 1, "owner": digest(&self.root), "devices": devices, "invitations": invites}).to_string();
        let r = (|| -> std::io::Result<()> {
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
            f.write_all(body.as_bytes())?; f.sync_all()?; drop(f);
            std::fs::rename(&tmp, &self.file)?;
            std::fs::File::open(&self.home)?.sync_all()
        })();
        if r.is_err() { let _ = std::fs::remove_file(&tmp); return Err("storage_unavailable".into()) }
        g.devices = devices; g.invites = invites; Ok(())
    }
    /// The newest 100 closed invitations stay listed; an open one is never dropped.
    fn trim(&self, xs: Vec<Value>) -> Vec<Value> {
        let at = (self.now)();
        let mut old: Vec<usize> = (0..xs.len()).filter(|&i| !open(&xs[i], at)).collect();
        old.sort_by(|&a, &b| xs[b]["createdAt"].as_f64().partial_cmp(&xs[a]["createdAt"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
        old.truncate(KEEP);
        xs.iter().enumerate().filter(|(i, x)| open(x, at) || old.contains(i)).map(|(_, x)| x.clone()).collect()
    }
    fn err(e: &str) -> Value { json!({"error": e}) }

    /// Pairing.resolve: "" when no credential, the hub token itself, or a paired device's own 32-hex token.
    /// A device seen again after a minute is stamped; a /ws connection is remembered for its revocation.
    pub fn resolve(&self, token: &str, client: u64, route: &str) -> String {
        if token.is_empty() || same(&self.root, token) { return self.root.clone() }
        if token.len() != 32 || !token.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) { return String::new() }
        let mut g = self.inner.lock().unwrap();
        let h = digest(token);
        let Some(d) = g.devices.iter().find(|d| live(d) && same(d["hash"].as_str().unwrap_or(""), &h)).cloned() else { return String::new() };
        let at = (self.now)();
        if at as f64 - d["lastSeen"].as_f64().unwrap_or(0.0) >= 60000.0 {
            let next = g.devices.iter().map(|x| if x["id"] == d["id"] { let mut y = x.clone(); y["lastSeen"] = json!(at); y } else { x.clone() }).collect();
            let inv = g.invites.clone(); if self.persist(&mut g, next, inv).is_err() { return String::new() } // pairing_resolve's catch: a stamp that cannot be written refuses the device
        }
        if route == "/ws" { g.clients.insert(client, d["id"].as_str().unwrap_or("").to_string()); }
        token.to_string()
    }
    /// Whether a credential holds right now: the hub token or a live device's. A device is stamped as
    /// `resolve` stamps it.
    pub fn holds(&self, token: &str) -> bool { !token.is_empty() && self.resolve(token, 0, "") == token }

    /// POST /auth/invitations: a private invitation, recorded before it is handed out, good ten minutes.
    pub fn create(&self, routes: &[String]) -> Value {
        let mut g = self.inner.lock().unwrap();
        let at = (self.now)();
        if g.invites.iter().filter(|x| !is_machine(x) && open(x, at)).count() >= OPEN_LIMIT { return Self::err("too_many_invitations") }
        let token = self.rand_hex(16);
        const ALPHA: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let code: String = (self.random)(8).iter().map(|x| ALPHA[(*x & 31) as usize] as char).collect();
        let mut unique: Vec<String> = vec![];
        for r in routes { if good_route(r) && !unique.contains(r) { unique.push(r.clone()) } }
        if unique.is_empty() { return Self::err("no_reachable_address") }
        let entry = json!({"id": self.rand_hex(16), "hash": digest(&token), "codeHash": digest(&code), "createdAt": at, "expiresAt": at + INVITE_MS, "used": false, "revoked": false});
        let mut next = g.invites.clone(); next.push(entry.clone());
        let (devs, next) = (g.devices.clone(), self.trim(next));
        if let Err(e) = self.persist(&mut g, devs, next) { return Self::err(&e) }
        let local_only = unique.iter().all(|x| local_origin(x));
        let frag = format!("invite={token}&routes={}", pct(&Value::from(unique.clone()).to_string()));
        json!({"id": entry["id"], "code": code, "expiresAt": at + INVITE_MS, "routes": unique, "localOnly": local_only, "link": format!("{}/pair#{frag}", unique[0])})
    }

    /// POST /auth/enroll: the invitation's secret (or its short code) for a new device credential. Ten
    /// guesses a minute hub-wide, whatever the address.
    pub fn enroll(&self, secret: &str, name: &str) -> Value {
        let mut g = self.inner.lock().unwrap();
        let at = (self.now)();
        g.attempts.retain(|x| *x + 60000 > at);
        if g.attempts.len() >= 10 { return Self::err("rate_limited") }
        g.attempts.push(at);
        let compact: String = secret.chars().filter(|c| *c != ' ' && *c != '-').collect::<String>().to_uppercase();
        let (hs, hc) = (digest(secret), digest(&compact));
        let Some(inv) = g.invites.iter().find(|x| !is_machine(x) && (same(x["hash"].as_str().unwrap_or(""), &hs) || same(x["codeHash"].as_str().unwrap_or(""), &hc))).cloned() else { return Self::err("wrong_code") };
        if inv["revoked"] == true { return Self::err("revoked_code") }
        if inv["used"] == true { return Self::err("used_code") }
        if inv["expiresAt"].as_f64().unwrap_or(0.0) <= at as f64 { return Self::err("expired_code") }
        if g.devices.iter().filter(|d| live(d)).count() >= DEVICE_LIMIT { return Self::err("device_limit") }
        let token = self.rand_hex(16);
        let name = if name.is_empty() { "New device" } else { name };
        let entry = json!({"id": self.rand_hex(16), "hash": digest(&token), "name": clean(name, 80), "createdAt": at, "lastSeen": at, "revoked": false});
        let mut devs = g.devices.clone(); devs.push(entry.clone());
        let invs = g.invites.iter().map(|x| if x["id"] == inv["id"] { let mut y = x.clone(); y["used"] = json!(true); y["usedAt"] = json!(at); y["device"] = entry["id"].clone(); y } else { x.clone() }).collect();
        // the device and its invitation's use are written together
        if let Err(e) = self.persist(&mut g, devs, invs) { return Self::err(&e) }
        json!({"token": token, "id": entry["id"]})
    }

    /// Where the secret of a machine invite nobody can use any more goes (expired or revoked unused).
    fn sweep(&self, g: &mut Inner) {
        let at = (self.now)();
        let dead: Vec<String> = g.invites.iter().filter(|x| is_machine(x) && x["used"] != true && (x["revoked"] == true || x["expiresAt"].as_f64().unwrap_or(0.0) <= at as f64))
            .filter_map(|x| x["peer"].as_str().map(str::to_string)).filter(|p| !g.swept.contains(p)).collect();
        for p in dead { if std::fs::remove_file(self.home.join("secrets/peers").join(&p)).map_or_else(|e| e.kind() == std::io::ErrorKind::NotFound, |_| true) { g.swept.insert(p); } }
    }
    /// A hub-to-hub invite for the link `peer`, recorded before it is handed out.
    pub fn machine(&self, peer: &str) -> Value {
        let mut g = self.inner.lock().unwrap();
        let at = (self.now)();
        if !is_peer(peer) || g.invites.iter().any(|x| is_machine(x) && x["peer"] == peer) { return Self::err("invalid_request") }
        if g.invites.iter().filter(|x| is_machine(x) && open(x, at)).count() >= OPEN_LIMIT { return Self::err("too_many_invitations") }
        let entry = json!({"id": self.rand_hex(16), "kind": "machine", "peer": peer, "hash": digest(&self.rand_hex(32)), "codeHash": digest(&self.rand_hex(32)), "createdAt": at, "expiresAt": at + INVITE_MS, "used": false, "revoked": false});
        let mut next = g.invites.clone(); next.push(entry.clone());
        let (devs, next) = (g.devices.clone(), self.trim(next));
        if let Err(e) = self.persist(&mut g, devs, next) { return Self::err(&e) }
        self.sweep(&mut g);
        json!({"ok": true, "id": entry["id"], "expiresAt": entry["expiresAt"]})
    }
    /// A machine links with the invite for `peer` once, before it expires, unless revoked.
    pub fn link(&self, peer: &str, name: &str) -> Value {
        let mut g = self.inner.lock().unwrap();
        let at = (self.now)();
        let Some(inv) = g.invites.iter().find(|x| is_machine(x) && x["peer"] == peer).cloned() else { return Self::err("unknown_invitation") };
        if inv["revoked"] == true { return Self::err("revoked_code") }
        if inv["used"] == true { return Self::err("used_code") }
        if inv["expiresAt"].as_f64().unwrap_or(0.0) <= at as f64 { return Self::err("expired_code") }
        let invs = g.invites.iter().map(|x| if x["id"] == inv["id"] { let mut y = x.clone(); y["used"] = json!(true); y["usedAt"] = json!(at); y["name"] = json!(clean(name, 80)); y } else { x.clone() }).collect();
        let devs = g.devices.clone();
        match self.persist(&mut g, devs, invs) { Ok(()) => json!({"ok": true}), Err(e) => Self::err(&e) }
    }
    /// The link a machine invite made was removed (bots.unpeer).
    pub fn unlinked(&self, peer: &str) -> Value {
        let mut g = self.inner.lock().unwrap();
        if !g.invites.iter().any(|x| is_machine(x) && x["peer"] == peer && x["used"] == true && x["unlinked"] != true) { return json!({"ok": true}) }
        let invs = g.invites.iter().map(|x| if is_machine(x) && x["peer"] == peer { let mut y = x.clone(); y["unlinked"] = json!(true); y } else { x.clone() }).collect();
        let devs = g.devices.clone();
        match self.persist(&mut g, devs, invs) { Ok(()) => json!({"ok": true}), Err(e) => Self::err(&e) }
    }

    fn state(x: &Value, at: u64) -> &'static str {
        if x["revoked"] == true { "revoked" } else if x["used"] == true { "used" } else if x["expiresAt"].as_f64().unwrap_or(0.0) <= at as f64 { "expired" } else { "open" }
    }
    /// GET /auth/devices: devices without their hashes, invitations newest first with what they paired.
    pub fn list(&self) -> Value {
        let mut g = self.inner.lock().unwrap();
        self.sweep(&mut g);
        let at = (self.now)();
        let names: HashMap<&str, &Value> = g.devices.iter().filter_map(|d| d["id"].as_str().map(|i| (i, d))).collect();
        let mut sorted: Vec<&Value> = g.invites.iter().collect();
        sorted.sort_by(|a, b| b["createdAt"].as_f64().partial_cmp(&a["createdAt"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
        let invitations: Vec<Value> = sorted.into_iter().map(|x| {
            let mut m = Map::new();
            m.insert("id".into(), x["id"].clone());
            m.insert("kind".into(), json!(if is_machine(x) { "machine" } else { "device" }));
            m.insert("createdAt".into(), x["createdAt"].clone()); m.insert("expiresAt".into(), x["expiresAt"].clone());
            m.insert("state".into(), json!(Self::state(x, at)));
            if x.get("usedAt").map_or(false, |u| !u.is_null() && u != 0) { m.insert("usedAt".into(), x["usedAt"].clone()); }
            if is_machine(x) {
                if x["used"] == true { m.insert("device".into(), x["peer"].clone()); m.insert("deviceName".into(), json!(x["name"].as_str().unwrap_or(""))); m.insert("deviceRevoked".into(), json!(x["unlinked"] == true)); }
            } else if let Some(d) = x["device"].as_str().filter(|d| !d.is_empty()) {
                let dev = names.get(d);
                m.insert("device".into(), json!(d)); m.insert("deviceName".into(), json!(dev.and_then(|v| v["name"].as_str()).unwrap_or("")));
                m.insert("deviceRevoked".into(), json!(dev.map_or(false, |v| truthy(&v["revoked"]))));
            }
            Value::Object(m)
        }).collect();
        let devices: Vec<Value> = g.devices.iter().map(|d| { let mut m = d.as_object().cloned().unwrap_or_default(); m.remove("hash"); Value::Object(m) }).collect();
        json!({"devices": devices, "invitations": invitations})
    }
    fn retire(g: &mut Inner, ids: &[String]) -> Vec<u64> {
        let gone: Vec<u64> = g.clients.iter().filter(|(_, d)| ids.contains(d)).map(|(c, _)| *c).collect();
        for c in &gone { g.retired.insert(*c); }
        gone
    }
    /// POST /auth/revoke with a device id: its credential stops working, its open connections are named.
    pub fn revoke(&self, id: &str) -> Value {
        let mut g = self.inner.lock().unwrap();
        if !g.devices.iter().any(|d| d["id"] == id) { return Self::err("unknown_device") }
        let devs = g.devices.iter().map(|d| if d["id"] == id { let mut y = d.clone(); y["revoked"] = json!(true); y } else { d.clone() }).collect();
        let invs = g.invites.clone();
        if let Err(e) = self.persist(&mut g, devs, invs) { return Self::err(&e) }
        json!({"ok": true, "clients": Self::retire(&mut g, &[id.to_string()])})
    }
    /// POST /auth/revoke with an invitation id: unused, it never enrolls; used, the device it paired loses
    /// access too. A machine invite's secret goes, and the link it made is removed by the caller (`peer`).
    pub fn uninvite(&self, id: &str) -> Value {
        let mut g = self.inner.lock().unwrap();
        let Some(inv) = g.invites.iter().find(|x| x["id"] == id).cloned() else { return Self::err("unknown_invitation") };
        let at = (self.now)();
        if is_machine(&inv) {
            let linked = inv["used"] == true && inv["unlinked"] != true;
            let invs = g.invites.iter().map(|x| if x["id"] == id { let mut y = x.clone(); y["revoked"] = json!(true); y["revokedAt"] = json!(at); if linked { y["unlinked"] = json!(true) } y } else { x.clone() }).collect();
            let devs = g.devices.clone();
            if let Err(e) = self.persist(&mut g, devs, invs) { return Self::err(&e) }
            let _ = std::fs::remove_file(self.home.join("secrets/peers").join(inv["peer"].as_str().unwrap_or("-")));
            let mut o = json!({"ok": true, "clients": []});
            if linked { o["peer"] = inv["peer"].clone() }
            return o
        }
        let paired: Vec<String> = inv["device"].as_str().filter(|d| g.devices.iter().any(|x| x["id"] == *d)).map(|d| vec![d.to_string()]).unwrap_or_default();
        let devs = g.devices.iter().map(|d| if d["id"].as_str().map_or(false, |i| paired.iter().any(|p| p == i)) { let mut y = d.clone(); y["revoked"] = json!(true); y } else { d.clone() }).collect();
        let invs = g.invites.iter().map(|x| if x["id"] == id { let mut y = x.clone(); y["revoked"] = json!(true); y["revokedAt"] = json!(at); y } else { x.clone() }).collect();
        if let Err(e) = self.persist(&mut g, devs, invs) { return Self::err(&e) }
        json!({"ok": true, "clients": Self::retire(&mut g, &paired)})
    }
    /// Whether a connection admitted before a revocation may still be published.
    pub fn active(&self, client: u64) -> bool { !self.inner.lock().unwrap().retired.contains(&client) }
    pub fn forget(&self, client: u64) { self.inner.lock().unwrap().clients.remove(&client); }
}

/// `new URL`'s authority, strictly: a host of letters, digits, `.` and `-` or a bracketed IPv6, then an
/// optional port of digits not above 65535. Anything else throws in JS, so it is None here.
pub fn authority(a: &str) -> Option<(String, Option<u16>)> {
    let (host, port) = if let Some(r) = a.strip_prefix('[') {
        let (h, rest) = r.split_once(']')?;
        if h.is_empty() || !h.bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.') || !h.contains(':') { return None }
        (format!("[{}]", h.to_lowercase()), if rest.is_empty() { None } else { Some(rest.strip_prefix(':')?) })
    } else {
        let (h, p) = match a.split_once(':') { Some((h, p)) => (h, Some(p)), None => (a, None) };
        if h.is_empty() || !h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') { return None }
        (h.to_lowercase(), p)
    };
    let port = match port { None => None, Some("") => None, Some(p) => { if !p.bytes().all(|b| b.is_ascii_digit()) || p.len() > 5 { return None } Some(p.parse::<u32>().ok().filter(|n| *n <= 65535)? as u16) } };
    Some((host, port))
}
/// (scheme, host, port with the scheme's default dropped) of an http(s) URL with no userinfo.
fn parse_origin(x: &str) -> Option<(&'static str, String, Option<u16>)> {
    let lower = x.get(..8).unwrap_or(x).to_lowercase(); // (a URL's scheme is case-insensitive)
    let x = &format!("{}{}", lower, x.get(8..).unwrap_or(""));
    let (s, def, rest) = if let Some(r) = x.strip_prefix("http://") { ("http:", 80, r) } else if let Some(r) = x.strip_prefix("https://") { ("https:", 443, r) } else { return None };
    let a = rest.split(|c| c == '/' || c == '?' || c == '#').next().unwrap_or("");
    let (h, p) = authority(a)?;
    Some((s, h, p.filter(|p| *p != def)))
}
/// http(s) only, no userinfo, no fragment, no query, a well-formed authority.
fn good_route(x: &str) -> bool {
    !x.contains('#') && !x.contains('?') && !x.chars().any(|c| c.is_whitespace() || c.is_control()) && parse_origin(x).is_some()
}
fn local_origin(x: &str) -> bool {
    let r = x.strip_prefix("http://").or_else(|| x.strip_prefix("https://")).unwrap_or("");
    r.starts_with("localhost") || r.starts_with("127.") || r.starts_with("[::1]")
}
/// URLSearchParams encoding of a value (application/x-www-form-urlencoded).
fn pct(s: &str) -> String {
    s.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => (b as char).to_string(), b' ' => "+".into(), _ => format!("%{b:02X}") }).collect()
}

/// pairingKnown: whether an origin is one of the owner's hubs as this hub knows them: its own listener
/// routes, the owner's machines it found, and the linked hubs it verified as the owner's. A desktop app
/// asks before following a pairing link to any other origin.
pub fn known(origin: &str, routes: &[String], machines: &Value, peers: &[String]) -> bool {
    let Some((scheme, host, port)) = parse_origin(origin) else { return false };
    let key = |s: &str, h: &str, p: Option<u16>| format!("{s}//{h}{}", p.map_or(String::new(), |p| format!(":{p}")));
    let selfm = &machines["self"];
    let mut urls: Vec<String> = routes.iter().cloned().chain(selfm["url"].as_str().map(str::to_string)).collect();
    for m in machines["peers"].as_array().into_iter().flatten() { if let Some(u) = m["url"].as_str() { urls.push(u.into()) } }
    urls.extend(peers.iter().cloned());
    let want = key(scheme, &host, port);
    if urls.iter().filter(|u| !u.is_empty()).filter_map(|u| parse_origin(u)).any(|(s, h, p)| key(s, &h, p) == want) { return true }
    let domain = selfm["domain"].as_str().unwrap_or("").to_lowercase(); let domain = domain.trim_end_matches('.');
    let mut names: Vec<String> = selfm["name"].as_str().into_iter().map(str::to_lowercase).collect();
    for m in machines["peers"].as_array().into_iter().flatten() { if let Some(n) = m["name"].as_str() { names.push(n.to_lowercase()) } }
    !domain.is_empty() && host.ends_with(&format!(".{domain}")) && host.split('.').next().map_or(false, |first| names.iter().any(|n| !n.is_empty() && n == first))
}

#[cfg(test)]
mod tests;
