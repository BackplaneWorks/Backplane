//! core/tailnet and server's `Machines.*`: the owner's machines on the tailnet (`tailscale status --json`),
//! hubs named in BACKPLANE_PEERS, which of them run Backplane (GET /hello), and who owns a node
//! (`tailscale whois --json`). Parsing is pure; the probes and the finder loop are the only IO. Laws `tailnet_*`.
use serde_json::{json, Value};

/// A machine a client can switch to.
#[derive(Debug, PartialEq, Clone)]
pub struct Machine { pub name: String, pub ip: String, pub online: bool }

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
/// The login that owns a node, from `tailscale whois --json <ip>`.
pub fn login(whois: &str) -> String {
    serde_json::from_str::<Value>(whois).ok().map(|j| s(&j["UserProfile"], "LoginName").to_string()).unwrap_or_default()
}
/// A peer is let in when it has an owner and that owner is ours (tagged devices have no person behind them).
pub fn trusts(owner: &str, login: &str) -> bool { !owner.is_empty() && owner == login && login != "tagged-devices" }

fn host(p: &Value) -> String {
    let h = s(p, "HostName");
    if !h.is_empty() { h.to_string() } else { s(p, "DNSName").split('.').next().unwrap_or("").to_string() }
}
fn machine(p: &Value) -> Machine {
    Machine { name: host(p), ip: p["TailscaleIPs"].as_array().and_then(|a| a.first()).and_then(Value::as_str).unwrap_or("").to_string(), online: p["Online"].as_bool().unwrap_or(false) }
}
fn raw(v: &Value) -> String { match v { Value::Null => String::new(), Value::String(x) => x.clone(), o => o.to_string() } }
/// The owner's other machines, from `tailscale status --json`: the peers whose UserID is this node's.
pub fn peers(status: &str) -> Vec<Machine> {
    let Ok(j) = serde_json::from_str::<Value>(status) else { return vec![] };
    let uid = raw(&j["Self"]["UserID"]);
    if uid.is_empty() { return vec![] }
    j["Peer"].as_object().into_iter().flatten().filter(|(_, p)| raw(&p["UserID"]) == uid).map(|(_, p)| machine(p)).collect()
}
/// Only machines that are up (and have an address) are worth asking.
pub fn online(ms: Vec<Machine>) -> Vec<Machine> { ms.into_iter().filter(|m| m.online && !m.ip.is_empty()).collect() }
/// This node itself (its name and first address).
pub fn this(status: &str) -> Machine {
    let j: Value = serde_json::from_str(status).unwrap_or(Value::Null);
    let mut m = machine(&j["Self"]); m.online = true; m
}
/// The tailnet's MagicDNS domain ("box.tail1.ts.net." → "tail1.ts.net").
pub fn domain(status: &str) -> String {
    let j: Value = serde_json::from_str(status).unwrap_or(Value::Null);
    let dns = s(&j["Self"], "DNSName"); let dns = dns.strip_suffix('.').unwrap_or(dns);
    dns.split('.').skip(1).collect::<Vec<_>>().join(".")
}
/// Self.DNSName without the trailing dot.
pub fn dns_name(status: &str) -> String {
    let j: Value = serde_json::from_str(status).unwrap_or(Value::Null);
    s(&j["Self"], "DNSName").trim_end_matches('.').to_string()
}
/// The address a hub names itself by when it asks another of the owner's machines to pair: the one its machines
/// list found only while it is on the tailnet (a --no-tailscale hub uses its listen address).
pub fn announce(on: bool, url: &str) -> String { if on { url.to_string() } else { String::new() } }
/// The pairing link a phone or browser pastes: the hub's address with the token after "#", so it never reaches
/// a server's logs.
pub fn link(base: &str, token: &str) -> String { format!("{base}/#token={token}") }
/// What the hub knows of its tailnet side: the state ("" not yet known, "unavailable", "reachable"), a newline, then
/// the pairing link ("" while there is none). A hub started with --host 0.0.0.0 is reachable and has no pairing link.
pub fn report(state: &str, link: &str) -> String { format!("{state}\n{link}") }
/// The main listener already covers the tailnet address (--host 0.0.0.0, or the address itself).
pub fn covered(host: &str, ip: &str) -> bool { host == "0.0.0.0" || host == ip }

/// The hub's HTTPS address from `tailscale serve status --json`: the "Web" entry whose "/" handler proxies to
/// this port ("https://host", with ":port" unless it is 443); "" when no entry does.
pub fn serve(status: &str, port: &str) -> String {
    let j: Value = serde_json::from_str(status).unwrap_or(Value::Null);
    for (hp, w) in j["Web"].as_object().into_iter().flatten() {
        let proxy = s(&w["Handlers"]["/"], "Proxy");
        if proxy.ends_with(&format!(":{port}")) || proxy.ends_with(&format!(":{port}/")) {
            return match hp.strip_suffix(":443") { Some(h) => format!("https://{h}"), None => format!("https://{hp}") };
        }
    }
    String::new()
}

/// A hub that may be there: a name and its base URL.
#[derive(Debug, PartialEq, Clone)]
pub struct Cand { pub name: String, pub url: String, pub named: bool }
pub fn cands(ms: &[Machine], port: u16) -> Vec<Cand> { ms.iter().map(|m| Cand { name: m.name.clone(), url: format!("http://{}:{port}", m.ip), named: false }).collect() }
/// Hubs named in BACKPLANE_PEERS ("http://host:port,..."), for machines that are not on the tailnet.
pub fn named(extra: &str) -> Vec<Cand> {
    extra.split(',').map(str::trim).filter(|u| !u.is_empty()).map(|u| Cand {
        name: u.strip_prefix("http://").unwrap_or(u).split(':').next().unwrap_or(u).to_string(), url: u.to_string(), named: true }).collect()
}
/// `{"self": {name, url, domain}, "peers": [{name, url, version, named}]}` (self's url is its loopback address
/// when it has no tailnet one, so two hubs named in each other's BACKPLANE_PEERS still agree which asks to pair).
pub fn machines_json(status: &str, port: u16, found: &[(Cand, String)]) -> Value {
    let me = this(status);
    json!({"self": {"name": me.name, "url": format!("http://{}:{port}", if me.ip.is_empty() { "127.0.0.1" } else { &me.ip }), "domain": domain(status)},
        "peers": found.iter().filter(|(_, v)| !v.is_empty()).map(|(c, v)| json!({"name": c.name, "url": c.url, "version": v, "named": c.named})).collect::<Vec<_>>()})
}

/// GET /hello on a hub: the version it names (CBOR; JSON from a hub not updated yet); "" for no answer. http only.
pub async fn hello(url: &str) -> String { hello_with(crate::copy::on(), url).await }
///
pub async fn hello_with(copy: bool, url: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    if copy { return String::new() }
    let Some(rest) = url.strip_prefix("http://") else { return String::new() };
    let hp = rest.split('/').next().unwrap_or("");
    let go = async {
        let mut st = tokio::net::TcpStream::connect(hp).await.ok()?;
        st.write_all(format!("GET /hello HTTP/1.1\r\nHost: {hp}\r\nConnection: close\r\n\r\n").as_bytes()).await.ok()?;
        let mut buf = vec![]; st.take(65536).read_to_end(&mut buf).await.ok()?;
        let at = buf.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
        let body = &buf[at..];
        let j = crate::cbor::decode(body).ok().or_else(|| serde_json::from_slice(body).ok())?;
        Some(j["backplane"].as_str().unwrap_or("").to_string())
    };
    tokio::time::timeout(std::time::Duration::from_secs(3), go).await.ok().flatten().unwrap_or_default()
}
/// Every Machines.every(): the owner's online tailnet peers and BACKPLANE_PEERS, each asked GET /hello at once.
pub async fn find(status: &str, port: u16, extra: &str) -> Value {
    let mut all = cands(&online(peers(status)), port); all.extend(named(extra));
    let probes = all.iter().map(|c| hello(&c.url));
    let versions = futures_util::future::join_all(probes).await;
    let found: Vec<(Cand, String)> = all.into_iter().zip(versions).collect();
    machines_json(status, port, &found)
}
pub async fn status_json() -> String {
    use tokio::process::Command;
    if crate::copy::on() { return String::new() }
    match tokio::time::timeout(std::time::Duration::from_secs(10), crate::proc::tokio_cmd(crate::proc::Kind::Net, "tailscale").args(["status", "--json"]).stderr(std::process::Stdio::null()).output()).await {
        Ok(Ok(o)) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(), _ => String::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    const STATUS: &str = r#"{"Self":{"HostName":"laptop","DNSName":"laptop.tail1.ts.net.","UserID":42,"TailscaleIPs":["100.1.0.1","fd7a::1"]},
      "Peer":{"k1":{"HostName":"desk","DNSName":"desk.tail1.ts.net.","UserID":42,"Online":true,"TailscaleIPs":["100.2.0.1"]},
              "k2":{"HostName":"phone","UserID":42,"Online":false,"TailscaleIPs":["100.3.0.1"]},
              "k3":{"HostName":"friend","UserID":7,"Online":true,"TailscaleIPs":["100.4.0.1"]},
              "k4":{"DNSName":"nas.tail1.ts.net.","UserID":42,"Online":true,"TailscaleIPs":[]}}}"#;

    #[test] fn tailnet_no_owner_no_trust() {
        assert!(!trusts("", "") && !trusts("", "a@b") && !trusts("a@b", ""));
        assert!(trusts("a@b", "a@b") && !trusts("a@b", "c@d"));
    }
    #[test] fn tailnet_tagged_devices_have_no_person() { assert!(!trusts("tagged-devices", "tagged-devices")); }
    #[test] fn tailnet_login_from_whois() {
        assert_eq!(login(r#"{"UserProfile":{"LoginName":"me@x.com"},"Node":{}}"#), "me@x.com");
        assert_eq!((login("not json"), login("{}")), (String::new(), String::new()));
    }
    #[test] fn tailnet_peers_are_the_owners_own() {
        let ps = peers(STATUS);
        assert_eq!(ps.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["desk", "phone", "nas"], "friend (another user) is not the owner's");
        let on = online(ps);
        assert_eq!(on, vec![Machine { name: "desk".into(), ip: "100.2.0.1".into(), online: true }], "offline and addressless ones are not asked");
        assert!(peers(r#"{"Self":{},"Peer":{"k":{"UserID":1}}}"#).is_empty(), "no self user: no owner's machines");
        assert!(peers("garbage").is_empty());
    }
    #[test] fn tailnet_domain_and_self() {
        assert_eq!(domain(STATUS), "tail1.ts.net");
        assert_eq!(this(STATUS), Machine { name: "laptop".into(), ip: "100.1.0.1".into(), online: true });
        assert_eq!(dns_name(STATUS), "laptop.tail1.ts.net");
        assert_eq!(domain("{}"), "");
    }
    #[test] fn tailnet_link_and_report() {
        assert_eq!(link("http://100.1.0.1:3787", "tok"), "http://100.1.0.1:3787/#token=tok");
        assert_eq!(report("reachable", ""), "reachable\n");
        assert!(covered("0.0.0.0", "100.1.0.1") && covered("100.1.0.1", "100.1.0.1") && !covered("127.0.0.1", "100.1.0.1"));
        assert_eq!((announce(true, "u"), announce(false, "u")), ("u".to_string(), String::new()));
    }
    #[test] fn tailnet_serve_https() {
        let j = r#"{"Web":{"laptop.tail1.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3787"}}},"x.ts.net:8443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:9"}}}}}"#;
        assert_eq!(serve(j, "3787"), "https://laptop.tail1.ts.net");
        let j2 = r#"{"Web":{"laptop.tail1.ts.net:8443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3787/"}}}}}"#;
        assert_eq!(serve(j2, "3787"), "https://laptop.tail1.ts.net:8443");
        assert_eq!(serve(j, "1"), "");
    }
    #[test] fn tailnet_named_peers_and_machines_json() {
        let n = named(" http://127.0.0.1:4002, ,http://box:3787 ");
        assert_eq!(n[0], Cand { name: "127.0.0.1".into(), url: "http://127.0.0.1:4002".into(), named: true });
        assert_eq!(n[1].name, "box"); assert_eq!(n.len(), 2);
        let found = vec![(cands(&online(peers(STATUS)), 3787).remove(0), "0.14.0".to_string()), (n[0].clone(), String::new()), (n[1].clone(), "0.13".into())];
        let j = machines_json(STATUS, 3787, &found);
        assert_eq!(j["self"], json!({"name":"laptop","url":"http://100.1.0.1:3787","domain":"tail1.ts.net"}));
        assert_eq!(j["peers"].as_array().unwrap().len(), 2, "a hub that did not answer is not listed");
        assert_eq!(j["peers"][1]["named"], true);
        assert_eq!(machines_json("", 4001, &[])["self"]["url"], "http://127.0.0.1:4001", "no tailnet address: loopback");
    }
    #[tokio::test] async fn tailnet_hello_reads_cbor_and_json() {
        use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};
        for body in [crate::cbor::encode(&json!({"backplane":"9.9.9"})), br#"{"backplane":"8.8.8"}"#.to_vec()] {
            let want = if body[0] == b'{' { "8.8.8" } else { "9.9.9" };
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let url = format!("http://{}", l.local_addr().unwrap());
            tokio::spawn(async move { let (mut c, _) = l.accept().await.unwrap(); let mut b = [0u8; 512]; let _ = c.read(&mut b).await;
                let _ = c.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes()).await; let _ = c.write_all(&body).await; });
            assert_eq!(hello(&url).await, want);
        }
        assert_eq!(hello("http://127.0.0.1:1").await, "", "nothing there");
        // a copied log makes no connection at all
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let url = format!("http://{}", l.local_addr().unwrap());
        assert_eq!(hello_with(true, &url).await, "");
        assert!(tokio::time::timeout(std::time::Duration::from_millis(200), l.accept()).await.is_err(), "nothing connected");
        assert_eq!(hello("https://x").await, "", "http only");
    }

    #[test] fn tailnet_domain_law() { assert_eq!(domain(r#"{"Self":{"DNSName":"desk.tail1.ts.net."}}"#), "tail1.ts.net"); }
    #[test] fn tailnet_link_token() { assert_eq!(link("http://100.64.0.1:3787", "ab12"), "http://100.64.0.1:3787/#token=ab12"); }
    #[test] fn tailnet_announce_off() { assert_eq!([announce(true, "http://100.64.0.1:3787"), announce(false, "http://100.64.0.1:3787")], ["http://100.64.0.1:3787", ""]); }
    #[test] fn tailnet_serve_law_fixture() {
        let j = r#"{"Web":{"a.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3773"}}},"a.ts.net:8443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3787"}}}}}"#;
        assert_eq!(serve(j, "3787"), "https://a.ts.net:8443");
    }
}
