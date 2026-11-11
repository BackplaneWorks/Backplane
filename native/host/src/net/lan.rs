//! Local network listening (Settings → Devices → Add a device, Local network): beside the tailnet's, the hub may answer
//! on a local network address the person chose (0.14's `Lan.*`). The choice is kept in `<home>/lan-v1.json`
//! (`{"version":1,"ips":[...]}`, private, written whole) so a restart listens there again; an address the machine no
//! longer has stays kept and is tried at the next start.
//!
//! Nothing there is trusted: no loopback trust and no owner (`wire::serve_with(.., false)`), so as on the tailnet listener
//! for a device its owner does not own, every request but the pairing itself, `/hello` and the web page needs a paired
//! credential. It is plain HTTP on that network: the screens say so. Turning it off closes the listener and every
//! connection it accepted. A hub whose main listener already covers the address (`--host 0.0.0.0` or that address)
//! needs none. A hub on a copied log never opens one.
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::{Arc, Mutex, Weak}};

/// What a choice did: `{ok, ip, listening}` or `{error}` (`lan_unknown`: not one of this machine's local network
/// addresses; `lan_unavailable`: it could not be listened on; `copy`: a hub on a copied log).
pub struct Lan { home: PathBuf, host: String, port: u16, hub: Weak<Hub>, saved: Mutex<Vec<String>>, open: Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>> }

/// The kept addresses in a `lan-v1.json` body: IPv4 only, anything else dropped.
pub fn parse_saved(body: &str) -> Vec<String> {
    let Ok(d) = serde_json::from_str::<Value>(body) else { return vec![] };
    if d["version"] != 1 { return vec![] }
    let mut out: Vec<String> = vec![];
    for ip in d["ips"].as_array().into_iter().flatten().filter_map(Value::as_str) { if ip.parse::<std::net::Ipv4Addr>().is_ok() && !out.iter().any(|x| x == ip) { out.push(ip.to_string()) } }
    out
}
/// This machine's (interface, IPv4) pairs.
pub fn interfaces() -> Vec<(String, String)> { crate::plat::net::v4_interfaces() }

impl Lan {
    pub fn new(home: &std::path::Path, host: &str, port: u16, hub: Weak<Hub>) -> Lan {
        let saved = std::fs::read_to_string(home.join("lan-v1.json")).map(|b| parse_saved(&b)).unwrap_or_default();
        Lan { home: home.to_path_buf(), host: host.to_string(), port, hub, saved: Mutex::new(saved), open: Mutex::new(HashMap::new()) }
    }
    /// Whether the main listener already answers on `ip`.
    pub fn covered(&self, ip: &str) -> bool { self.host == "0.0.0.0" || self.host == ip }
    /// The addresses this hub listens on because the person let it (open now).
    pub fn bound(&self) -> Vec<String> { let mut v: Vec<String> = self.open.lock().unwrap().keys().cloned().collect(); v.sort(); v }
    /// The kept choices, each `{ip, url, listening}` (`/auth/devices`' `lan`, Settings → Devices → Local network).
    pub fn list(&self) -> Value {
        let open = self.open.lock().unwrap();
        json!(self.saved.lock().unwrap().iter().map(|ip| json!({"ip": ip, "url": format!("http://{ip}:{}", self.port), "listening": open.contains_key(ip) || self.covered(ip)})).collect::<Vec<_>>())
    }
    fn save(&self, ips: Vec<String>) -> Result<(), String> {
        use crate::plat::fs::OpenOptionsExt; use std::io::Write;
        let file = self.home.join("lan-v1.json");
        let tmp = self.home.join(format!("lan-v1.json.{}.tmp", hex::encode(super::pairing::urandom(8))));
        let body = json!({"version": 1, "ips": ips}).to_string();
        let r = (|| -> std::io::Result<()> {
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
            f.write_all(body.as_bytes())?; f.sync_all()?; drop(f);
            crate::plat::fs::rename(&tmp, &file)
        })();
        if r.is_err() { let _ = std::fs::remove_file(&tmp); return Err("storage_unavailable".into()) }
        *self.saved.lock().unwrap() = ips; Ok(())
    }
    /// "Let the hub answer on the local network": listen on `ip` too (kept for the next start). Call inside the runtime.
    pub fn on(&self, ip: &str) -> Value {
        if crate::copy::on() { return json!({"error": "copy"}) }
        if !super::plan::lan(&interfaces()).iter().any(|x| x == ip) { return json!({"error": "lan_unknown"}) }
        if self.covered(ip) || self.open.lock().unwrap().contains_key(ip) { return json!({"ok": true, "ip": ip, "listening": true}) }
        if !self.listen(ip) { return json!({"error": "lan_unavailable"}) }
        let mut ips = self.saved.lock().unwrap().clone();
        if !ips.iter().any(|x| x == ip) { ips.push(ip.to_string()); if let Err(e) = self.save(ips) { self.close(ip); return json!({"error": e}) } }
        json!({"ok": true, "ip": ip, "listening": true})
    }
    /// "Stop answering on the local network": forgotten, the listener and its connections closed.
    pub fn off(&self, ip: &str) -> Value {
        let ips: Vec<String> = self.saved.lock().unwrap().iter().filter(|x| *x != ip).cloned().collect();
        let changed = ips.len() != self.saved.lock().unwrap().len();
        if changed { if let Err(e) = self.save(ips) { return json!({"error": e}) } }
        self.close(ip);
        json!({"ok": true, "ip": ip, "listening": self.covered(ip)})
    }
    fn close(&self, ip: &str) { if let Some(tx) = self.open.lock().unwrap().remove(ip) { let _ = tx.send(true); } }
    /// The kept addresses, listened on again at the start (one that cannot be bound now stays kept).
    pub fn boot(&self) -> usize {
        if crate::copy::on() { return 0 }
        let ips = self.saved.lock().unwrap().clone();
        ips.iter().filter(|ip| !self.covered(ip)).filter(|ip| self.listen(ip)).count()
    }
    /// Bind `ip:port` now (so the answer says whether it took) and serve it in the background until `close`.
    fn listen(&self, ip: &str) -> bool {
        let Ok(rt) = tokio::runtime::Handle::try_current() else { return false };
        let Some(hub) = self.hub.upgrade() else { return false };
        let Ok(std_l) = std::net::TcpListener::bind((ip, self.port)) else { eprintln!("backplane-hub: could not listen on the local network address {ip}:{}", self.port); return false };
        if std_l.set_nonblocking(true).is_err() { return false }
        let _guard = rt.enter();
        let Ok(l) = tokio::net::TcpListener::from_std(std_l) else { return false };
        let (tx, rx) = tokio::sync::watch::channel(false);
        self.open.lock().unwrap().insert(ip.to_string(), tx);
        println!("Backplane on the local network: http://{ip}:{}/ (plain HTTP; devices pair in Settings)", self.port);
        rt.spawn(accept(hub, l, rx));
        true
    }
}

/// Connections from one local network listener, each untrusted; all of them end when the address is turned off.
async fn accept(hub: Arc<Hub>, l: tokio::net::TcpListener, mut stop: tokio::sync::watch::Receiver<bool>) {
    let gate = Arc::new(tokio::sync::Semaphore::new(128));
    loop {
        let got = tokio::select! { r = l.accept() => r, _ = stop.changed() => return };
        let Ok((s, _)) = got else { tokio::time::sleep(std::time::Duration::from_millis(50)).await; continue };
        // at most 128 connections at once; more are dropped, not held open
        let Ok(permit) = gate.clone().try_acquire_owned() else { continue };
        s.set_nodelay(true).ok();
        let (hub, mut stop) = (hub.clone(), stop.clone());
        tokio::spawn(async move {
            tokio::select! { _ = crate::wire::serve_with(hub, s, false) => {}, _ = stop.changed() => {} }
            drop(permit);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn kept_addresses_are_ipv4_once() {
        assert_eq!(parse_saved(r#"{"version":1,"ips":["192.168.1.9","nope","192.168.1.9","10.0.0.5",3]}"#), ["192.168.1.9", "10.0.0.5"]);
        assert!(parse_saved(r#"{"version":2,"ips":["192.168.1.9"]}"#).is_empty());
        assert!(parse_saved("garbage").is_empty());
    }
    /// On, kept, bound and answering without trust; off, forgotten and closed (connections too); a restart listens again.
    #[tokio::test] async fn on_keeps_listens_and_off_forgets_and_closes() {
        use crate::{hub::Cfg, presenter::Canned};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        // the test needs a local network address of this machine's own; a machine with none (a CI box with loopback only) skips
        let Some(ip) = super::super::plan::lan(&interfaces()).into_iter().next() else { eprintln!("no local network address here: skipped"); return };
        let dir = crate::plat::tmp::dir("lan");
        let hub = Hub::start(Cfg { home: dir.to_path_buf(), version: "0.15.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let port = std::net::TcpListener::bind((ip.as_str(), 0)).unwrap().local_addr().unwrap().port();
        let lan = Lan::new(&dir, "127.0.0.1", port, Arc::downgrade(&hub));
        assert_eq!(lan.on("203.0.113.77")["error"], "lan_unknown", "only this machine's own local network addresses");
        let r = lan.on(&ip); assert_eq!((r["ok"].as_bool(), r["listening"].as_bool()), (Some(true), Some(true)), "{r}");
        assert_eq!(parse_saved(&std::fs::read_to_string(dir.join("lan-v1.json")).unwrap()), [ip.clone()]);
        assert_eq!(lan.list()[0]["listening"], true);
        let mut held = tokio::net::TcpStream::connect((ip.as_str(), port)).await.expect("it answers there");
        let mut s = tokio::net::TcpStream::connect((ip.as_str(), port)).await.unwrap();
        s.write_all(format!("GET /auth/devices HTTP/1.1\r\nHost: {ip}:{port}\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        let mut b = vec![]; s.read_to_end(&mut b).await.unwrap(); let t = String::from_utf8_lossy(&b);
        assert!(t.starts_with("HTTP/1.1 401") || t.starts_with("HTTP/1.1 404"), "no trust there: {t}");
        let r = lan.off(&ip); assert_eq!(r["listening"], false);
        assert!(parse_saved(&std::fs::read_to_string(dir.join("lan-v1.json")).unwrap()).is_empty());
        let mut one = [0u8; 1];
        let n = tokio::time::timeout(std::time::Duration::from_secs(5), held.read(&mut one)).await.expect("the held connection is closed").unwrap_or(0);
        assert_eq!(n, 0);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(tokio::net::TcpStream::connect((ip.as_str(), port)).await.is_err(), "the listener is closed");
        // kept across a restart
        assert_eq!(lan.on(&ip)["ok"], true); lan.close(&ip);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let again = Lan::new(&dir, "127.0.0.1", port, Arc::downgrade(&hub));
        assert_eq!(again.boot(), 1); assert_eq!(again.bound(), [ip.clone()]);
        again.off(&ip);
    }
}
