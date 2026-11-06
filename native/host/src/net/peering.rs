//! The hub-to-hub handshake and signed intake (server.bend: `Hub.bot.pair*`, `Pair.*`, `Hub.bot.link`,
//! `Conn.bot.signed`, `Hub.bot_verified`/`Hub.bot.seen`):
//!
//! * `POST /bots/pair?name=&url=` — one of the owner's machines asks to link, admitted by (a) a credential,
//!   (b) `Auth.owner.pair` (the whois-verified socket peer, an exact own authority, no Origin or proxy identity), or
//!   (c) a pairing proof (`&to=&at=&proof=`) with no credential, which this hub checks by calling the asker back at
//!   the address ITS machines list has for it (`POST /bots/pair/check`), never the one the ask names.
//!   The answer is a fresh invite `{"invite":"bp1:<url>:<peer>:<secret>"}`, or 403/409 with the reason.
//! * `POST /bots/pair/check` — 204 when this hub made that proof within the last minute, else 403.
//! * `POST /bots/link` — signed with the invite's secret: the machine holding the invite says where it is; once.
//! * `signed_in` — the intake every other signed route shares (checked, fresh, not seen before).
//! Auth and owner-trust decisions are `owner.rs`'s; nothing here weakens them.
use super::{botnet, machines::{self, Peer}, owner, pairing::Registry, peers::PeerTable, routes::AuthRoutes, sign};
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
    pub tailnet: bool, pub host: String, pub port: u16,
    own_hosts: Mutex<Vec<String>>, seen: Mutex<sign::Seen>,
    now: Box<dyn Fn() -> u64 + Send + Sync>,
}

fn text(status: &'static str, body: &str) -> HttpReply { HttpReply::text(status, body) }
fn json(status: &'static str, v: &Value) -> HttpReply { HttpReply { status, ctype: "application/json".into(), extra: String::new(), body: v.to_string().into_bytes() } }
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
        Peering { home, reg, table, routes, tailnet, host: host.into(), port, own_hosts: Mutex::new(vec![]), seen: Mutex::new(sign::Seen::default()),
            now: Box::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())) }
    }
    pub fn with_clock(mut self, now: Box<dyn Fn() -> u64 + Send + Sync>) -> Peering { self.now = now; self }
    /// The listener's own authorities (Auth.owner.hosts): from its tailnet address and Self.DNSName only.
    pub fn set_own_hosts(&self, ip: &str, dns: &str) { *self.own_hosts.lock().unwrap() = owner::owner_hosts(ip, dns, &self.port.to_string()) }
    #[cfg(test)]
    pub fn own_hosts_for_test(&self) -> Vec<String> { self.own_hosts.lock().unwrap().clone() }
    fn token(&self) -> String { std::fs::read_to_string(self.home.join("token")).unwrap_or_default().trim().to_string() }
    fn credential(&self, r: &PReq) -> bool {
        let secret = bearer_or_cookie_or_query(r);
        let t = self.reg.resolve(&secret, 0, r.route());
        owner::allowed(&r.headers, &r.target, &t)
    }

    /// None when the route is not this module's.
    pub async fn handle(&self, hub: &Arc<Hub>, r: &PReq) -> Option<HttpReply> {
        if r.method != "POST" { return None }
        match r.route() {
            "/bots/pair" => Some(self.pair(hub, r).await),
            "/bots/pair/check" => Some(self.check(r)),
            "/bots/link" => Some(self.link(hub, r).await),
            _ => None,
        }
    }

    // ----- /bots/pair -----

    /// Conn.owner.gate / Conn.route.bot case 11: a credential or the whois-verified owner is `own`; a proof with no
    /// credential is `lan`; anything else is refused.
    async fn pair(&self, hub: &Arc<Hub>, r: &PReq) -> HttpReply {
        let own_hosts = self.own_hosts.lock().unwrap().clone();
        let own = self.credential(r) || owner::owner_pair(r.own_device, &own_hosts, &r.headers, &r.method, r.route());
        let asked = owner::browser(&r.headers) && r.header("origin").is_empty() && !r.param("proof").is_empty();
        if !own && !asked { return text("401 Unauthorized", "this Backplane needs its pairing token") }
        let (name, url) = (botnet::clean(&unpct(&r.param("name"))), unpct(&r.param("url")));
        let (j, peers, sync_off, own_no) = self.view();
        let code = machines::answer(&j, &peers, &|n| sync_off.contains(&n.to_lowercase()), &|id| own_no.contains(id), &name, &url);
        match (code, own) {
            (0, true) => self.invite(hub, true).await,
            (0, false) => {
                // the asker is called back at the address this hub's machines list has for it
                let found = machines::machine(&j, &name, &url).unwrap_or_default();
                let (to, at, proof) = (r.param("to"), r.param("at"), r.param("proof"));
                let ok = plain(&to, &at, &proof) && !found.is_empty() && machines::port(&to) == self.port.to_string();
                if !ok { return text("403 Forbidden", "not a pairing request for this hub") }
                let q = format!("/bots/pair/check?to={to}&at={at}&proof={proof}");
                let resp = botnet::plain(&format!("{}{q}", botnet::url_base(&found))).await;
                if resp.code == 204 { self.invite(hub, false).await } else { text("403 Forbidden", "the asking hub did not confirm this request") }
            }
            (1, _) => text("403 Forbidden", "sync with that machine is off here"),
            (3, _) => text("403 Forbidden", "not one of the machines this hub found"),
            _ => text("409 Conflict", "already linked"),
        }
    }
    /// The machines list, live peers, and the two settings the rules read.
    fn view(&self) -> (Value, Vec<Peer>, std::collections::HashSet<String>, std::collections::HashSet<String>) {
        let j = self.routes.machines_value();
        self.table.with(|p| {
            let live = p.live();
            let peers = live.iter().map(|x| Peer { id: x.id.clone(), name: x.name.clone(), url: x.url.clone() }).collect();
            let sync_off = p.sync_off_names();
            let own_no = live.iter().filter(|x| p.own_how(&x.id) == "no").map(|x| x.id.clone()).collect();
            (j, peers, sync_off, own_no)
        })
    }
    /// Pair.invite: a new peer id and secret, recorded and kept here; the answer carries the invite. `own`: the link is
    /// recorded as one of the owner's machines before the answer goes.
    async fn invite(&self, hub: &Arc<Hub>, own: bool) -> HttpReply {
        let here = botnet::url_here(self.tailnet, &self.host, self.port).await;
        let peer = format!("p{}", hex::encode(super::pairing::urandom(8)));
        let secret = botnet::fresh_secret();
        let now = (self.now)();
        let rec = self.reg.machine(&peer);
        let why = if rec["ok"] == true { "" } else { "could not record the invite" };
        if why.is_empty() && own { let _ = hub.commit(&json!({"$":"SettingSet","key":format!("peer.own.{peer}"),"value":"pair","at":now})).await; }
        let kept = why.is_empty() && botnet::peer_save(&self.home, &peer, &secret);
        if kept { json("200 OK", &json!({"invite": sign::invite_make(&here, &peer, &secret)})) } else { text("500 Internal Server Error", "could not keep the invite's secret") }
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
    /// Conn.pair.check: 204 when this hub made that proof within the last minute, else 403 (and it says nothing more).
    fn check(&self, r: &PReq) -> HttpReply {
        let at = r.param("at").parse::<u64>().unwrap_or(0);
        if owner::pair_checked(&self.token(), &r.param("to"), at, (self.now)(), &r.param("proof")) {
            HttpReply { status: "204 No Content", ctype: "text/plain".into(), extra: "Cache-Control: no-store\r\n".into(), body: vec![] }
        } else { text("403 Forbidden", "not a pairing request of this hub's") }
    }

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

    // ----- the asking side -----

    /// Pair.ask: POST /bots/pair at `url` with a proof made from this hub's token, then join the invite it answers.
    pub async fn ask(&self, hub: &Arc<Hub>, url: &str, announced: &str, own: bool) {
        let here = if announced.is_empty() { botnet::url_here(self.tailnet, &self.host, self.port).await } else { announced.to_string() };
        let me = botnet::name().await;
        let now = (self.now)();
        let to = botnet::url_base(url).to_string();
        let q = format!("{to}/bots/pair?name={me}&url={here}&to={to}&at={now}&proof={}", owner::pair_proof(&self.token(), &to, now));
        let r = botnet::plain(&q).await;
        if r.code != 200 { return }
        let inv = serde_json::from_str::<Value>(&r.body).ok().and_then(|j| sign::invite_parse(j["invite"].as_str().unwrap_or("")));
        if let Some(i) = inv { let _ = self.join(hub, &i, own).await; }
    }
    /// BotJob.join: keep the invite's secret, tell the inviter where we are (signed POST /bots/link), and link once
    /// it answers with its name. Err: why it did not link.
    pub async fn join(&self, hub: &Arc<Hub>, i: &sign::Inv, own: bool) -> Result<(String, String), String> {
        if !botnet::url_ok(&i.url) || !botnet::safe(&i.id) { return Err("not a usable invite".into()) }
        if self.table.with(|p| p.peer(&i.id).map_or(false, |x| !x.revoked)) { return Err("already linked to that machine".into()) }
        if !botnet::peer_save(&self.home, &i.id, &i.secret) { return Err("not a usable invite".into()) }
        let here = botnet::url_here(self.tailnet, &self.host, self.port).await;
        let me = botnet::name().await;
        let now = (self.now)();
        let body = json!({"url": here, "name": me}).to_string();
        let r = botnet::signed(&self.home, &i.url, &i.id, &i.secret, now, "/bots/link", true, &body).await;
        let name = botnet::clean(serde_json::from_str::<Value>(&r.body).ok().and_then(|j| j["name"].as_str().map(str::to_string)).as_deref().unwrap_or(""));
        if r.code == 200 {
            let shown = if name.is_empty() { i.id.clone() } else { name };
            let mut evs = vec![];
            if own { evs.push(json!({"$":"SettingSet","key":format!("peer.own.{}", i.id),"value":"pair","at":now})) }
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
/// Pair.plain: the proof's fields as the asker sent them: an http address with no query or fragment of its own, a
/// time, a hex digest.
pub fn plain(to: &str, at: &str, proof: &str) -> bool {
    botnet::url_ok(to) && !to.contains(['?', '&', '#']) && at.parse::<u64>().map_or(false, |n| n.to_string() == at)
        && proof.len() == 64 && botnet::safe(proof)
}

#[cfg(test)]
pub(crate) mod tests;
