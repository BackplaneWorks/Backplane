//! The hub-to-hub link: signed intake and the `bp1:` invite. A link is made only by pairing (`pairflow.rs`): a machine code,
//! a request that waits for the person's approval, then the invite below. Nothing pairs by itself.
//! * `POST /bots/link` — signed with the invite's secret: the machine holding the invite says where it is; once.
//! * `signed_in` — the intake every other signed route shares (checked, fresh, not seen before).
use super::{botnet, machines, owner, pairflow::PairFlow, pairing::Registry, peers::PeerTable, routes::AuthRoutes, sign};
use crate::{api::HttpReply, hub::Hub};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::{Arc, Mutex}};

/// What a handler needs of a request. `trusted`: loopback or the owner's device (never a credential by itself).
/// `own_device`: the connection came through the TAILNET listener and its peer passed the whois verdict: the only
/// thing owner trust (`Auth.owner.pair`/`peer`) may rest on. A loopback socket is `trusted` and never `own_device`.
pub struct PReq { pub method: String, pub target: String, pub headers: Vec<(String, String)>, pub body: Vec<u8>, pub trusted: bool, pub own_device: bool }
impl PReq {
    pub fn route(&self) -> &str { self.target.split('?').next().unwrap_or("") }
    pub fn header(&self, n: &str) -> &str { owner::header(&self.headers, n) }
    pub fn param(&self, n: &str) -> String { owner::param(&self.target, n) }
}

pub struct Peering {
    pub home: PathBuf, pub reg: Arc<Registry>, pub table: Arc<PeerTable>, pub routes: Arc<AuthRoutes>,
    pub tailnet: bool, pub host: String, pub port: u16, pub via: Mutex<Vec<Value>>,
    own_hosts: Mutex<Vec<String>>, seen: Mutex<sign::Seen>, pub flow: PairFlow,
    pub(crate) now: Box<dyn Fn() -> u64 + Send + Sync>,
}

pub(crate) fn text(status: &'static str, body: &str) -> HttpReply { HttpReply::text(status, body) }
pub(crate) fn json(status: &'static str, v: &Value) -> HttpReply { HttpReply { status, ctype: "application/json".into(), extra: String::new(), body: v.to_string().into_bytes() } }
/// Att.unpct: a percent-encoded text, bytes as UTF-8.
pub fn unpct(s: &str) -> String {
    let b = s.as_bytes(); let mut out = Vec::with_capacity(b.len()); let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hexv(b[i + 1]), hexv(b[i + 2])) { out.push(h * 16 + l); i += 3; continue }
        }
        out.push(b[i]); i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn hexv(c: u8) -> Option<u8> { (c as char).to_digit(16).map(|d| d as u8) }

impl Peering {
    pub fn new(home: PathBuf, reg: Arc<Registry>, table: Arc<PeerTable>, routes: Arc<AuthRoutes>, tailnet: bool, host: &str, port: u16) -> Peering {
        Peering { home, reg, table, routes, tailnet, host: host.into(), port, via: Mutex::new(vec![]), own_hosts: Mutex::new(vec![]), seen: Mutex::new(sign::Seen::default()), flow: PairFlow::default(),
            now: Box::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())) }
    }
    pub fn with_clock(mut self, now: Box<dyn Fn() -> u64 + Send + Sync>) -> Peering { self.now = now; self }
    /// The listener's own authorities (Auth.owner.hosts): from its tailnet address and Self.DNSName only.
    pub fn set_own_hosts(&self, ip: &str, dns: &str) { *self.own_hosts.lock().unwrap() = owner::owner_hosts(ip, dns, &self.port.to_string()) }
    #[cfg(test)]
    pub fn own_hosts_for_test(&self) -> Vec<String> { self.own_hosts.lock().unwrap().clone() }
    pub(crate) fn token(&self) -> String { std::fs::read_to_string(self.home.join("token")).unwrap_or_default().trim().to_string() }
    pub(crate) fn credential(&self, r: &PReq) -> bool {
        let secret = bearer_or_cookie_or_query(r);
        let t = self.reg.resolve(&secret, 0, r.route());
        owner::allowed(&r.headers, &r.target, &t)
    }

    /// None when the route is not this module's.
    pub async fn handle(&self, hub: &Arc<Hub>, r: &PReq) -> Option<HttpReply> {
        if r.method != "POST" { return None }
        match r.route() {
            "/bots/pair/request" => Some(self.request(hub, r).await),
            "/bots/pair/poll" => Some(self.poll(r)),
            "/bots/pair/local" => Some(self.local(hub, r).await),
            "/bots/link" => Some(self.link(hub, r).await),
            _ => None,
        }
    }

    /// BotJob.invite (bots.invite): an invite for another machine to paste: a new peer id and secret, recorded (ten minutes,
    /// once) and kept here. `given` is the address to name instead of this hub's own. Ok((invite, peer)) or why not.
    pub async fn invite_for(&self, given: &str) -> Result<(String, String), String> {
        let here = if given.trim().is_empty() { botnet::url_here(self.tailnet, &self.host, self.port).await } else { given.trim().to_string() };
        let peer = format!("p{}", hex::encode(super::pairing::urandom(8)));
        let secret = botnet::fresh_secret();
        let rec = self.reg.machine(&peer);
        if rec["ok"] != true {
            return Err(if rec["error"] == "too_many_invitations" { "too many open invites: wait for one to expire, or revoke one in Settings > Devices".into() } else { "could not record the invite".into() });
        }
        if !botnet::peer_save(&self.home, &peer, &secret) { return Err("could not keep the invite's secret".into()) }
        Ok((sign::invite_make(&here, &peer, &secret), peer))
    }
    /// BotJob.unpeer: the link's secret goes, and the invite that made it shows as unlinked in Settings > Devices.
    pub fn unpeered(&self, peer: &str) { botnet::peer_forget(&self.home, peer); let _ = self.reg.unlinked(peer); }
    // ----- signed intake -----

    /// A signed request from a linked machine, checked: the sender is a plain id, the body within 256 KB, the
    /// signature fresh and genuine for that peer's secret, and not seen before. Ok(peer id) or the refusal to send.
    pub fn signed_in(&self, kind: &str, r: &PReq) -> Result<String, HttpReply> {
        let peer = r.header("x-backplane-peer").to_string();
        if !botnet::safe(&peer) { return Err(text("401 Unauthorized", "unknown sender")) }
        if r.body.len() > 262144 { return Err(text("413 Payload Too Large", "at most 256 KB, or 2 MB for a form")) }
        let now = (self.now)();
        let body = String::from_utf8_lossy(&r.body);
        let key = botnet::check_peer(&botnet::peer_secret(&self.home, &peer), now, &format!("{kind}:{peer}"), r.header("x-backplane-timestamp"), r.header("x-backplane-signature"), &body);
        if key.is_empty() { return Err(text("401 Unauthorized", if r.header("x-backplane-signature").is_empty() { "no credentials: send Authorization: Bearer <the webhook's token>, or sign the request" } else { "bad or stale signature" })) }
        if !self.seen.lock().unwrap().accept(&key, now) { return Err(text("401 Unauthorized", "already seen")) }
        Ok(peer)
    }

    /// Hub.bot.link: the machine that holds an invite says where it is; once only.
    async fn link(&self, hub: &Arc<Hub>, r: &PReq) -> HttpReply {
        let peer = match self.signed_in("link", r) { Ok(p) => p, Err(e) => return e };
        if self.table.with(|p| p.peer(&peer).map_or(false, |x| !x.revoked)) { return text("409 Conflict", "already linked") }
        let Ok(o) = serde_json::from_slice::<Value>(&r.body) else { return text("400 Bad Request", "not JSON") };
        let url = o["url"].as_str().unwrap_or("").to_string();
        if !botnet::url_ok(&url) { return text("400 Bad Request", "no http address") }
        let name = botnet::clean(o["name"].as_str().unwrap_or(""));
        let here = botnet::url_here(self.tailnet, &self.host, self.port).await;
        if let Some(why) = refuse_loopback(&url, &here) { return text("403 Forbidden", why) }
        let res = self.reg.link(&peer, &name);
        if res["ok"] == true {
            let me = botnet::name().await;
            let now = (self.now)();
            let _ = hub.commit(&json!({"$":"PeerSet","id":peer,"name":if name.is_empty() { peer.clone() } else { name },"url":url,"revoked":false,"at":now})).await;
            json("200 OK", &json!({"name": me}))
        } else {
            let err = res["error"].as_str().unwrap_or("storage_unavailable");
            if matches!(err, "expired_code" | "revoked_code" | "used_code" | "unknown_invitation") { botnet::peer_forget(&self.home, &peer) }
            text("401 Unauthorized", match err {
                "expired_code" => "this invite has expired: make a new one", "revoked_code" => "this invite was revoked",
                "used_code" => "this invite was used already", _ => "this hub has no record of this invite: make a new one" })
        }
    }

    // ----- the joining side -----

    /// BotJob.join: keep the invite's secret, tell the inviter where we are (signed POST /bots/link), and link once
    /// it answers with its name. Err: why it did not link.
    pub async fn join(&self, hub: &Arc<Hub>, i: &sign::Inv, how: &str) -> Result<(String, String), String> {
        if !botnet::url_ok(&i.url) || !botnet::safe(&i.id) { return Err("not a usable invite".into()) }
        if self.table.with(|p| p.peer(&i.id).map_or(false, |x| !x.revoked)) { return Err("already linked to that machine".into()) }
        let here = botnet::url_here(self.tailnet, &self.host, self.port).await;
        let here = announce(&self.home, &here, &i.url).await.map_err(String::from)?;
        if !botnet::peer_save(&self.home, &i.id, &i.secret) { return Err("not a usable invite".into()) }
        let me = botnet::name().await;
        let now = (self.now)();
        let body = json!({"url": here, "name": me}).to_string();
        let r = botnet::signed(&self.home, &i.url, &i.id, &i.secret, now, "/bots/link", true, &body).await;
        let name = botnet::clean(serde_json::from_str::<Value>(&r.body).ok().and_then(|j| j["name"].as_str().map(str::to_string)).as_deref().unwrap_or(""));
        if r.code == 200 {
            let shown = if name.is_empty() { i.id.clone() } else { name };
            let mut evs = vec![];
            if !how.is_empty() { evs.push(json!({"$":"SettingSet","key":format!("peer.own.{}", i.id),"value":how,"at":now})) }
            evs.push(json!({"$":"PeerSet","id":i.id,"name":shown,"url":i.url,"revoked":false,"at":now}));
            let _ = hub.commit_many(&evs).await;
            Ok((i.id.clone(), shown))
        } else {
            botnet::peer_forget(&self.home, &i.id);
            Err(format!("the other machine did not link ({}) {}", r.code, r.body.trim().chars().take(200).collect::<String>()))
        }
    }
}

fn bearer_or_cookie_or_query(r: &PReq) -> String {
    let b = r.header("authorization");
    if let Some(t) = b.strip_prefix("Bearer ") { return t.to_string() }
    let c = r.header("cookie").split(';').find_map(|c| c.trim().strip_prefix("bp_token=").map(str::to_string)).unwrap_or_default();
    if c.is_empty() { r.param("token") } else { c }
}
/// The address a hub that nobody can reach gives the machine it links to: "reach me over my reverse link" (net/alwayson/tunnel.rs). It is a
/// well-formed address that never resolves (`.invalid`), so a call that does not go over the link fails at once instead of reaching anything.
pub const TUNNEL_URL: &str = "https://over-its-reverse-link.invalid";
/// The address to announce to `target`: `here`, or, when `here` is only this computer and `target` takes the reverse link (its `/far/tunnel`
/// answers an unsigned request with "not a linked machine": a build that has it), `TUNNEL_URL`; everything that machine sends here then comes
/// over this hub's poll. Err: neither (the old refusal: an older machine could never reach back).
pub async fn announce(home: &std::path::Path, here: &str, target: &str) -> Result<String, &'static str> {
    let Some(why) = refuse_loopback(here, target) else { return Ok(here.to_string()) };
    let r = botnet::post_json(home, &format!("{}/far/tunnel", botnet::url_base(target)), "{}").await;
    if r.code == 401 && r.body.contains("not a linked machine") { Ok(TUNNEL_URL.to_string()) } else { Err(why) }
}
/// A peer address that is the reverse link's, not a place to call.
pub fn over_link(url: &str) -> bool { url == TUNNEL_URL }
/// Why a hub may not announce `here` to the hub at `target`: it would hand a machine across the network an address that is
/// only this computer ("127.0.0.1"), a link that could never reach back.
pub fn refuse_loopback(here: &str, target: &str) -> Option<&'static str> {
    (machines::loopback(here) && !machines::loopback(target)).then_some("this hub is only reachable on its own loopback address (127.0.0.1), so the other machine could not link back: start it with Tailscale on, or set BACKPLANE_URL to an address that machine can reach")
}

#[cfg(test)]
pub(crate) mod tests;
