//! /auth/enroll, /auth/invitations, /auth/devices, /auth/revoke, /auth/known: server's `Conn.devices.*`
//! over `Registry`. `handle` is pure of the socket: the hub gives it the request and the connection id,
//! and acts on the `Outcome` (answer, connections to close, a link to remove).
use super::{owner, pairing::{self, Registry}};
use crate::{api::HttpReply, auth::Req};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

pub struct Listener { pub routes: Vec<String>, pub machines: Value, pub peers: Vec<String> }
pub struct AuthRoutes {
    pub reg: Arc<Registry>, host: String, port: u16, state: Mutex<Listener>,
    qr: fn(&str) -> String,
    /// the local network listening the person chose (`net::lan`); none until the hub installs it
    lan: std::sync::OnceLock<Arc<super::lan::Lan>>,
    /// this machine's (interface, IPv4) pairs; a test gives its own
    ifaces: fn() -> Vec<(String, String)>,
}
pub struct Outcome { pub reply: HttpReply, pub close: Vec<u64>, pub unpeer: Option<String> }

pub const ROUTES: &[&str] = &["/auth/enroll", "/auth/invitations", "/auth/devices", "/auth/revoke", "/auth/known", "/auth/pair", "/auth/cache", "/auth/lan"];

fn json_reply(status: &'static str, body: &Value, extra: &str) -> HttpReply {
    HttpReply { status, ctype: "application/json".into(), extra: format!("Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n{extra}"), body: body.to_string().into_bytes() }
}

impl AuthRoutes {
    pub fn new(reg: Arc<Registry>, host: &str, port: u16, qr: fn(&str) -> String) -> AuthRoutes {
        AuthRoutes { reg, host: host.into(), port, qr, state: Mutex::new(Listener { routes: vec![], machines: json!({}), peers: vec![] }), lan: Default::default(), ifaces: super::lan::interfaces }
    }
    /// The same with the machine's interfaces as a test sets them.
    #[cfg(test)]
    pub fn with_ifaces(mut self, ifaces: fn() -> Vec<(String, String)>) -> AuthRoutes { self.ifaces = ifaces; self }
    pub fn set_lan(&self, lan: Arc<super::lan::Lan>) { let _ = self.lan.set(lan); }
    pub fn lan(&self) -> Option<Arc<super::lan::Lan>> { self.lan.get().cloned() }
    /// The registry's list with the local network addresses the person let the hub answer on (`lan`) and the main listener's host.
    pub fn devices(&self) -> Value {
        let mut j = self.reg.list();
        if j.is_object() { j["lan"] = self.lan().map_or(json!([]), |l| l.list()); j["host"] = json!(self.host); }
        j
    }
    /// Pairing.address: a listener-confirmed reachable origin (Serve or the tailnet bind), credentials stripped.
    pub fn address(&self, link: &str) {
        let origin = ["https://", "http://"].iter().find_map(|s| link.strip_prefix(s).map(|r| (*s, r)))
            .and_then(|(s, r)| { let a = r.split(|c| c == '/' || c == '?' || c == '#').next().unwrap_or(""); (!a.is_empty() && !a.contains('@')).then(|| format!("{s}{a}")) });
        if let Some(o) = origin { let mut g = self.state.lock().unwrap(); if !g.routes.contains(&o) { g.routes.insert(0, o) } }
    }
    /// Pairing.known: the owner's machines this hub found, and the urls of the links it verified as the owner's.
    /// The machines list as the finder last reported it.
    pub fn machines_value(&self) -> Value { self.state.lock().unwrap().machines.clone() }
    pub fn machines(&self, machines: Value, peers: Vec<String>) { let mut g = self.state.lock().unwrap(); g.machines = machines; g.peers = peers }

    fn secret(req: &Req) -> String {
        let b = req.header("authorization");
        if let Some(t) = b.strip_prefix("Bearer ") { return t.to_string() }
        let cookie = req.header("cookie").split(';').find_map(|c| c.trim().strip_prefix("bp_token=").map(str::to_string)).unwrap_or_default();
        if cookie.is_empty() { owner::param(&req.target, "token") } else { cookie }
    }
    /// A private invitation made from inside the hub (Settings → Devices → Add a device): the same one `POST /auth/invitations` makes, with the QR code of its link.
    /// Its `addresses` (`plan::plan`) carry no QR code here: the page draws the one for the address it shows.
    pub fn invite(&self) -> Value {
        // the hub's own address stands in for the Host header an HTTP request would have carried (a hub bound to everything names its interfaces itself)
        let authority = if self.host == "0.0.0.0" { String::new() } else { format!("{}:{}", self.host, self.port) };
        self.create(&authority, false)
    }
    /// The invitation and every address it may be shown with (`plan::plan`): the tailnet, each local network address, this computer.
    fn create(&self, authority: &str, https: bool) -> Value {
        let routes = self.state.lock().unwrap().routes.clone();
        let bound = self.lan().map(|l| l.bound()).unwrap_or_default();
        let (default, choices) = super::plan::plan(&routes, &self.host, self.port, authority, https, &(self.ifaces)(), &bound);
        self.reg.create_with(&default, &choices)
    }
    /// Every address another machine can reach this hub on, most reachable first (`plan::reachable`); `first` is the hub's own word for it.
    pub fn addresses(&self, first: &str) -> Vec<String> {
        let routes = self.state.lock().unwrap().routes.clone();
        let bound = self.lan().map(|l| l.bound()).unwrap_or_default();
        super::plan::reachable(first, &routes, &self.host, self.port, &(self.ifaces)(), &bound)
    }
    /// The QR code of the default link and of each address's own link.
    fn with_qrs(&self, mut j: Value) -> Value {
        let link = j["link"].as_str().unwrap_or("").to_string();
        if link.is_empty() { return j }
        j["qr"] = json!((self.qr)(&link));
        if let Some(xs) = j["addresses"].as_array_mut() { for x in xs.iter_mut() { let l = x["link"].as_str().unwrap_or("").to_string(); x["qr"] = json!((self.qr)(&l)); } }
        j
    }
    /// POST /auth/lan (`X-Backplane-Lan: <address>`, `X-Backplane-Lan-State: on|off`) and Settings → Devices: let the hub answer on a local network address too, or stop.
    pub fn lan_set(&self, ip: &str, on: bool) -> Value {
        let Some(lan) = self.lan() else { return json!({"error": "lan_unavailable"}) };
        if on { lan.on(ip) } else { lan.off(ip) }
    }

    /// None when `req` is not an /auth route. `client` is the connection id (Pairing.resolve remembers /ws ones).
    pub fn handle(&self, req: &Req, client: u64) -> Option<Outcome> {
        let route = req.route();
        if !ROUTES.contains(&route) { return None }
        let hs = &req.headers;
        if route == "/auth/pair" || route == "/auth/cache" {
            // the credential the request presents, resolved (the hub token or a paired device's own)
            let token = self.reg.resolve(&Self::secret(req), client, route);
            let https = req.header("origin").starts_with("https://");
            let done = |reply: HttpReply| Some(Outcome { reply, close: vec![], unpeer: None });
            return if route == "/auth/pair" {
                // Conn.pair.reply: pairing proves the shared secret in the bearer; it never inherits a localhost exemption
                if req.method == "POST" && owner::pair(hs, &token) { done(HttpReply { status: "204 No Content", ctype: "text/plain".into(), extra: owner::pair_cookies(&token, https), body: vec![] }) }
                else { done(HttpReply::text("401 Unauthorized", "pairing refused")) }
            } else if owner::cache_request(&req.method, hs, &token) {
                // Conn.cache.reply: public, domain-separated data; never a credential
                done(json_reply("200 OK", &json!({"version": 1, "cacheKey": owner::cache_key(&token)}), ""))
            } else { done(HttpReply { status: "401 Unauthorized", ctype: "text/plain".into(), extra: "Cache-Control: no-store\r\n".into(), body: b"cache identity refused".to_vec() }) };
        }
        let denied = || Outcome { reply: json_reply("401 Unauthorized", &json!({"error":"pairing_required"}), ""), close: vec![], unpeer: None };
        let (op, secret) = if route == "/auth/enroll" {
            if !(req.method == "POST" && owner::browser(hs)) { return Some(denied()) }
            ("enroll", req.header("authorization").chars().skip(7).collect::<String>())
        } else {
            let token = self.reg.resolve(&Self::secret(req), client, route);
            let want = match route { "/auth/devices" => "GET", _ => "POST" };
            if req.method != want || !owner::cache_request("GET", hs, &token) { return Some(denied()) }
            match route {
                "/auth/invitations" => ("create", String::new()),
                "/auth/lan" => (if req.header("x-backplane-lan-state") == "off" { "lan-off" } else { "lan-on" }, req.header("x-backplane-lan").to_string()),
                "/auth/devices" => ("list", String::new()),
                "/auth/revoke" => if req.header("x-backplane-invitation").is_empty() { ("revoke", req.header("x-backplane-device").to_string()) } else { ("uninvite", req.header("x-backplane-invitation").to_string()) },
                _ => ("known", req.header("x-backplane-origin").to_string()),
            }
        };
        let https = req.header("origin").starts_with("https://") || (req.header("origin").is_empty() && req.header("x-backplane-scheme") == "https");
        let j = match op {
            "enroll" => self.reg.enroll(&secret, req.header("x-backplane-name")),
            "list" => self.devices(),
            "lan-on" | "lan-off" => self.lan_set(&secret, op == "lan-on"),
            "revoke" => self.reg.revoke(&secret),
            "uninvite" => self.reg.uninvite(&secret),
            "create" => self.create(req.header("host"), https),
            _ => { let g = self.state.lock().unwrap(); let mut rs = g.routes.clone(); rs.push(format!("http://{}:{}", self.host, self.port));
                json!({"known": pairing::known(&secret, &rs, &g.machines, &g.peers)}) }
        };
        let (close, unpeer) = revoked(op, &j);
        let native = req.header("origin").is_empty() && req.header("x-backplane-client") == "native";
        let error = j["error"].as_str().unwrap_or("");
        let credential = j["token"].as_str().unwrap_or("");
        let link = j["link"].as_str().unwrap_or("");
        let body = if op == "enroll" {
            if native || !error.is_empty() { j.clone() } else { json!({"ok": true, "device": j["id"].as_str().unwrap_or("")}) }
        } else if link.is_empty() { j.clone() } else { self.with_qrs(j.clone()) };
        let status = if error.is_empty() { "200 OK" } else if error == "rate_limited" { "429 Too Many Requests" } else { "400 Bad Request" };
        let cookies = if credential.is_empty() { String::new() } else { owner::pair_cookies(credential, https) };
        Some(Outcome { reply: json_reply(status, &body, &cookies), close, unpeer })
    }
}

/// What the hub must do once the registry has answered `op`: the connections to close and the machine link to remove. Every door to a revoke acts on this
/// (POST /auth/revoke here, Settings → Devices through `start::revoke`): a revoked credential that leaves its socket open still reads the hub.
pub fn revoked(op: &str, j: &Value) -> (Vec<u64>, Option<String>) {
    let close = j["clients"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect();
    (close, (op == "uninvite").then(|| j["peer"].as_str().filter(|p| !p.is_empty()).map(str::to_string)).flatten())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn setup(tag: &str, host: &str) -> (AuthRoutes, PathBuf) {
        let dir = std::env::temp_dir().join(format!("bp-net-routes-{tag}-{}-{}", std::process::id(), hex::encode(pairing::urandom(4))));
        std::fs::create_dir_all(&dir).unwrap();
        let r = Arc::new(Registry::open(&dir, "root").unwrap());
        let a = AuthRoutes::new(r, host, 3787, |l| format!("QR:{l}")).with_ifaces(Vec::new); a.address("https://me.tail.ts.net/ignored?x=1#y");
        (a, dir)
    }
    fn req(method: &str, target: &str, hs: &[(&str, &str)]) -> Req { Req::new(method, target, hs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()) }
    fn body(o: &Outcome) -> Value { serde_json::from_slice(&o.reply.body).unwrap() }
    const ROOT: [(&str, &str); 2] = [("Authorization", "Bearer root"), ("Host", "me.tail.ts.net")];

    #[test] fn pair_flow_over_http() {
        let (a, dir) = setup("flow", "127.0.0.1");
        let inv = a.handle(&req("POST", "/auth/invitations", &ROOT), 1).unwrap();
        assert_eq!(inv.reply.status, "200 OK"); let b = body(&inv);
        assert!(b["link"].as_str().unwrap().starts_with("https://me.tail.ts.net/pair#invite="), "{b}");
        assert_eq!(b["qr"].as_str().unwrap(), format!("QR:{}", b["link"].as_str().unwrap()));
        assert!(inv.reply.extra.contains("Cache-Control: no-store") && inv.reply.extra.contains("Referrer-Policy: no-referrer"));
        let code = b["code"].as_str().unwrap();
        let e = a.handle(&req("POST", "/auth/enroll", &[("Authorization", &format!("Bearer {code}")), ("X-Backplane-Client", "native"), ("X-Backplane-Name", "Pixel")]), 2).unwrap();
        let eb = body(&e); let tok = eb["token"].as_str().unwrap().to_string();
        assert!(e.reply.extra.contains(&format!("bp_token={tok}")) && e.reply.extra.contains("bp_cache_v1="));
        // the device can list devices with its own credential
        let l = a.handle(&req("GET", "/auth/devices", &[("Authorization", &format!("Bearer {tok}"))]), 3).unwrap();
        assert_eq!(body(&l)["devices"][0]["name"], "Pixel");
        // and a revoked one cannot
        let id = eb["id"].as_str().unwrap();
        let rv = a.handle(&req("POST", "/auth/revoke", &[("Authorization", "Bearer root"), ("X-Backplane-Device", id)]), 1).unwrap();
        assert_eq!(body(&rv)["ok"], true);
        assert_eq!(a.handle(&req("GET", "/auth/devices", &[("Authorization", &format!("Bearer {tok}"))]), 3).unwrap().reply.status, "401 Unauthorized");
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn pair_cookie_route_proves_the_secret() {
        let (a, dir) = setup("cookie", "127.0.0.1");
        let ok = a.handle(&req("POST", "/auth/pair", &[("Authorization", "Bearer root")]), 1).unwrap().reply;
        assert_eq!(ok.status, "204 No Content"); assert!(ok.extra.contains("bp_token=root") && ok.extra.contains("bp_cache_v1=") && ok.extra.contains("HttpOnly") && !ok.extra.contains("Secure"));
        let sec = a.handle(&req("POST", "/auth/pair", &[("Authorization", "Bearer root"), ("Origin", "https://h"), ("Host", "h")]), 1).unwrap().reply;
        assert!(sec.extra.contains("; Secure"));
        for (m, hs) in [("POST", vec![]), ("GET", vec![("Authorization", "Bearer root")]), ("POST", vec![("Authorization", "Bearer wrong")]), ("POST", vec![("Host", "localhost")]), ("POST", vec![("Cookie", "bp_token=root")])] {
            assert_eq!(a.handle(&req(m, "/auth/pair", &hs), 1).unwrap().reply.status, "401 Unauthorized", "{m} {hs:?}");
        }
        assert_eq!(a.handle(&req("POST", "/auth/pair?token=root", &[]), 1).unwrap().reply.status, "401 Unauthorized", "no query token");
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn pair_cache_identity_is_for_paired_browsers() {
        let (a, dir) = setup("cache", "127.0.0.1");
        let r = a.handle(&req("GET", "/auth/cache", &[("Cookie", "bp_token=root")]), 1).unwrap().reply;
        assert_eq!(r.status, "200 OK"); let b: Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(b, json!({"version": 1, "cacheKey": owner::cache_key("root")})); assert_ne!(b["cacheKey"], "root");
        for (m, t, hs) in [("GET", "/auth/cache", vec![]), ("GET", "/auth/cache?token=root", vec![]), ("POST", "/auth/cache", vec![("Authorization", "Bearer root")]), ("GET", "/auth/cache", vec![("Authorization", "Bearer root"), ("Sec-Fetch-Site", "cross-site")])] {
            assert_eq!(a.handle(&req(m, t, &hs), 1).unwrap().reply.status, "401 Unauthorized", "{m} {t} {hs:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn pair_browser_enroll_hides_the_credential_in_a_cookie() {
        let (a, dir) = setup("browser", "127.0.0.1");
        let code = body(&a.handle(&req("POST", "/auth/invitations", &ROOT), 1).unwrap())["code"].as_str().unwrap().to_string();
        let e = a.handle(&req("POST", "/auth/enroll", &[("Authorization", &format!("Bearer {code}")), ("Origin", "https://me.tail.ts.net"), ("Host", "me.tail.ts.net")]), 2).unwrap();
        let b = body(&e); assert!(b.get("token").is_none() && b["ok"] == true, "{b}");
        assert!(e.reply.extra.contains("HttpOnly") && e.reply.extra.contains("; Secure"));
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn auth_routes_refuse_without_a_credential() {
        let (a, dir) = setup("deny", "127.0.0.1");
        for (m, p) in [("POST", "/auth/invitations"), ("GET", "/auth/devices"), ("POST", "/auth/revoke"), ("POST", "/auth/known")] {
            for hs in [vec![], vec![("Host", "localhost")], vec![("Authorization", "Bearer wrong")]] {
                let o = a.handle(&req(m, p, &hs), 1).unwrap(); assert_eq!(o.reply.status, "401 Unauthorized", "{m} {p} {hs:?}");
                assert_eq!(body(&o)["error"], "pairing_required");
            }
        }
        // the query token is no credential for these routes, and neither is a cross-site request
        assert_eq!(a.handle(&req("GET", "/auth/devices?token=root", &[]), 1).unwrap().reply.status, "401 Unauthorized");
        assert_eq!(a.handle(&req("GET", "/auth/devices", &[("Authorization", "Bearer root"), ("Sec-Fetch-Site", "cross-site")]), 1).unwrap().reply.status, "401 Unauthorized");
        assert_eq!(a.handle(&req("GET", "/auth/devices", &[("Authorization", "Bearer root"), ("Origin", "https://evil.example"), ("Host", "me")]), 1).unwrap().reply.status, "401 Unauthorized");
        // wrong method with the right credential
        assert_eq!(a.handle(&req("GET", "/auth/invitations", &ROOT), 1).unwrap().reply.status, "401 Unauthorized");
        assert_eq!(a.handle(&req("GET", "/auth/enroll", &[("Authorization", "Bearer x")]), 1).unwrap().reply.status, "401 Unauthorized");
        assert!(a.handle(&req("GET", "/other", &ROOT), 1).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn pair_enroll_errors_map_to_statuses() {
        let (a, dir) = setup("status", "127.0.0.1");
        let mut last = String::new();
        for _ in 0..10 { let o = a.handle(&req("POST", "/auth/enroll", &[("Authorization", "Bearer nope-nope")]), 1).unwrap(); assert_eq!(o.reply.status, "400 Bad Request"); last = body(&o).to_string(); }
        assert!(last.contains("wrong_code"));
        assert_eq!(a.handle(&req("POST", "/auth/enroll", &[("Authorization", "Bearer nope-nope")]), 1).unwrap().reply.status, "429 Too Many Requests");
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn pair_invitation_routes_prefer_remote_addresses() {
        let (a, dir) = setup("addr", "127.0.0.1");
        let b = body(&a.handle(&req("POST", "/auth/invitations", &[("Authorization", "Bearer root"), ("Host", "localhost:3787")]), 1).unwrap());
        assert_eq!(b["routes"], json!(["https://me.tail.ts.net"]), "the loopback host header is not offered while a remote address exists");
        let (c, dir2) = setup("addr2", "127.0.0.1");
        let r = c.create("evil host;", false); assert!(!r.to_string().contains("evil"), "{r}");
        let _ = (std::fs::remove_dir_all(dir), std::fs::remove_dir_all(dir2));
    }
    /// Settings → Add a device on a hub bound to 127.0.0.1 with no tailnet listener (`--no-tailscale`): the link names only this computer, and says so
    /// (`localOnly`, which Settings and Setup turn into a warning); once the tailnet listener confirms its address, the link names that first.
    #[test] fn pair_a_loopback_hub_invites_with_local_only_until_it_has_a_tailnet_address() {
        let dir = std::env::temp_dir().join(format!("bp-net-routes-local-{}-{}", std::process::id(), hex::encode(pairing::urandom(4))));
        std::fs::create_dir_all(&dir).unwrap();
        let a = AuthRoutes::new(Arc::new(Registry::open(&dir, "root").unwrap()), "127.0.0.1", 3797, |l| format!("QR:{l}")).with_ifaces(Vec::new);
        let j = a.invite();
        assert_eq!(j["localOnly"], true, "{j}"); assert_eq!(j["routes"], json!(["http://127.0.0.1:3797"]));
        a.address("http://100.64.84.41:3797");
        let j = a.invite();
        assert_eq!(j["localOnly"], false, "{j}"); assert!(j["link"].as_str().unwrap().starts_with("http://100.64.84.41:3797/pair#invite="), "{j}");
        let _ = std::fs::remove_dir_all(dir);
    }
    fn home_ifaces() -> Vec<(String, String)> { vec![("lo".into(), "127.0.0.1".into()), ("enp9s0".into(), "192.168.77.2".into()), ("docker0".into(), "172.17.0.1".into()), ("ts0".into(), "100.101.102.103".into())] }
    /// One invitation, shown at each address (Tailscale, the local network, this computer): each its own link and QR code, the same code;
    /// the older fields stay the default address's.
    #[test] fn pair_an_invitation_names_every_address_it_can_be_shown_with() {
        let dir = std::env::temp_dir().join(format!("bp-net-routes-addrs-{}-{}", std::process::id(), hex::encode(pairing::urandom(4))));
        std::fs::create_dir_all(&dir).unwrap();
        let a = AuthRoutes::new(Arc::new(Registry::open(&dir, "root").unwrap()), "127.0.0.1", 3871, |l| format!("QR:{l}")).with_ifaces(home_ifaces);
        a.address("http://100.101.102.103:3871");
        let b = body(&a.handle(&req("POST", "/auth/invitations", &[("Authorization", "Bearer root"), ("Host", "127.0.0.1:3871")]), 1).unwrap());
        let xs = b["addresses"].as_array().unwrap();
        let k: Vec<(&str, &str, bool)> = xs.iter().map(|x| (x["kind"].as_str().unwrap(), x["url"].as_str().unwrap(), x["listening"].as_bool().unwrap())).collect();
        assert_eq!(k, [("tailnet", "http://100.101.102.103:3871", true), ("lan", "http://192.168.77.2:3871", false), ("local", "http://127.0.0.1:3871", true)]);
        let token = b["link"].as_str().unwrap().split("invite=").nth(1).unwrap().split('&').next().unwrap().to_string();
        for x in xs {
            let l = x["link"].as_str().unwrap();
            assert!(l.starts_with(&format!("{}/pair#invite={token}&routes=", x["url"].as_str().unwrap())), "{l}");
            assert!(!l.contains("127.0.0.1") || x["kind"] == "local", "loopback is never a hint for another device: {l}");
            assert_eq!(x["qr"].as_str().unwrap(), format!("QR:{l}"));
        }
        assert_eq!(b["link"], xs[0]["link"], "the default is the first remote address that answers");
        assert_eq!((b["routes"].clone(), b["localOnly"].clone()), (json!(["http://100.101.102.103:3871"]), json!(false)));
        // made inside the hub (Settings): the same addresses, no QR codes (the page draws the shown one)
        let j = a.invite();
        assert_eq!(j["addresses"].as_array().unwrap().len(), 3); assert!(j.get("qr").is_none() && j["addresses"][0].get("qr").is_none(), "{j}");
        let _ = std::fs::remove_dir_all(dir);
    }
    /// POST /auth/lan is authorized as /auth/invitations is; without the hub's listener machinery it says it cannot listen.
    #[test] fn pair_lan_route_needs_a_credential() {
        let (a, dir) = setup("lan", "127.0.0.1");
        let hs = |c: &'static str| vec![("Authorization", c), ("X-Backplane-Lan", "192.168.77.2"), ("X-Backplane-Lan-State", "on")];
        assert_eq!(a.handle(&req("POST", "/auth/lan", &hs("Bearer wrong")), 1).unwrap().reply.status, "401 Unauthorized");
        assert_eq!(a.handle(&req("GET", "/auth/lan", &hs("Bearer root")), 1).unwrap().reply.status, "401 Unauthorized");
        let o = a.handle(&req("POST", "/auth/lan", &hs("Bearer root")), 1).unwrap();
        assert_eq!((o.reply.status, body(&o)["error"].clone()), ("400 Bad Request", json!("lan_unavailable")));
        let l = body(&a.handle(&req("GET", "/auth/devices", &[("Authorization", "Bearer root")]), 1).unwrap());
        assert_eq!((l["lan"].clone(), l["host"].clone()), (json!([]), json!("127.0.0.1")));
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn pair_known_over_http() {
        let (a, dir) = setup("known", "127.0.0.1");
        a.machines(json!({"self":{"url":"https://me.tail.ts.net","name":"me","domain":"tail.ts.net"}}), vec![]);
        let k = |o: &str| body(&a.handle(&req("POST", "/auth/known", &[("Authorization", "Bearer root"), ("X-Backplane-Origin", o)]), 1).unwrap())["known"].clone();
        assert_eq!(k("https://me.tail.ts.net"), true); assert_eq!(k("https://evil.example"), false);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn pair_revoke_names_connections_to_close() {
        let (a, dir) = setup("close", "127.0.0.1");
        let code = body(&a.handle(&req("POST", "/auth/invitations", &ROOT), 1).unwrap())["code"].as_str().unwrap().to_string();
        let eb = body(&a.handle(&req("POST", "/auth/enroll", &[("Authorization", &format!("Bearer {code}")), ("X-Backplane-Client", "native")]), 2).unwrap());
        let (tok, id) = (eb["token"].as_str().unwrap().to_string(), eb["id"].as_str().unwrap().to_string());
        a.reg.resolve(&tok, 42, "/ws");
        let o = a.handle(&req("POST", "/auth/revoke", &[("Authorization", "Bearer root"), ("X-Backplane-Device", &id)]), 1).unwrap();
        assert_eq!(o.close, vec![42]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
