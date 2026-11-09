//! Pairing two hubs: a machine code, a request that waits for a person, and then the link.
//!
//! * Machine A makes a code (`make_code`: a machine invite with a typed code, eight characters, ten minutes, once).
//! * Machine B enters A's address and the code (`pair_with`). `POST /bots/pair/request {code,name,url,check}` claims the code
//!   (wrong codes: ten tries a minute hub-wide) and WAITS on A as a pending request that shows B's name, address and a six-digit
//!   check code, which B shows too.
//! * The person approves on A (`approve`) or denies (`deny`); a request nobody answers in two minutes is refused. Only on approval
//!   does A keep a secret for the link and record it as made by an approved pairing (`peer.own.<id>` = `approved`); B, polling
//!   `POST /bots/pair/poll`, receives the `bp1:` invite once and joins with it (the signed `POST /bots/link`), recording the same.
//! Being on the same Tailscale account is not part of this. Requests live in memory: a restart refuses them all.
use super::{botnet, machines, peering::{refuse_loopback, text, json as jreply, PReq, Peering}, pairing::{digest, urandom}, sign};
use crate::{api::HttpReply, hub::Hub};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// How long a request waits for the person.
pub const WAIT_S: u64 = 120;
const MAX_PENDING: usize = 8;
/// How long a finished outgoing attempt stays on the page.
const KEEP_S: u64 = 600;

#[derive(Clone, PartialEq, Debug)]
enum In { Pending, Approved(String), Denied }
#[derive(Clone, Debug)]
struct Incoming { id: String, peer: String, name: String, url: String, check: String, at: u64, poll: String, state: In }
#[derive(Clone, PartialEq, Debug)]
enum Out { Waiting, Linked(String), Refused(String) }
#[derive(Clone, Debug)]
struct Outgoing { id: String, address: String, check: String, at: u64, state: Out }
#[derive(Clone, Debug)]
struct Made { code: String, peer: String, address: String, addresses: Vec<String>, expires_ms: u64 }
#[derive(Default)]
struct Inner { incoming: Vec<Incoming>, outgoing: Vec<Outgoing>, made: Option<Made> }
#[derive(Default)]
pub struct PairFlow { inner: Mutex<Inner> }

fn digits6() -> String { let b = urandom(4); format!("{:06}", u32::from_le_bytes([b[0], b[1], b[2], b[3]]) % 1_000_000) }
fn is_check(s: &str) -> bool { s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit()) }
/// An address as a person types it: `host:port` gets `http://`; a trailing slash goes. None when it is no http address.
pub fn normalize(a: &str) -> Option<String> {
    let a = a.trim().trim_end_matches('/');
    // a bare MagicDNS name (no port) is what `tailscale serve` answers on https
    let bare = a.split(['/', '?', '#']).next().unwrap_or("").to_ascii_lowercase();
    let scheme = if bare.ends_with(".ts.net") { "https" } else { "http" };
    let full = if a.starts_with("http://") || a.starts_with("https://") { a.to_string() } else { format!("{scheme}://{a}") };
    (botnet::url_ok(&full) && !full.contains(['?', '#']) && full.len() > 8).then_some(full)
}
/// What a refused claim says.
fn claim_reason(e: &str) -> (&'static str, &'static str) {
    match e {
        "rate_limited" => ("429 Too Many Requests", "too many tries: wait a minute"),
        "used_code" => ("403 Forbidden", "that code was used already: make a new one"),
        "expired_code" => ("403 Forbidden", "that code has expired (or too little time is left): make a new one"),
        "revoked_code" => ("403 Forbidden", "that code was revoked"),
        _ => ("403 Forbidden", "wrong code"),
    }
}

impl Peering {
    fn now_s(&self) -> u64 { (self.now)() }
    /// Drop what is over: requests nobody answered (refused), answered ones nobody collected, old attempts.
    fn sweep(&self, g: &mut Inner) {
        let now = self.now_s();
        for r in g.incoming.iter_mut() { if r.state == In::Pending && now > r.at + WAIT_S { r.state = In::Denied } }
        g.incoming.retain(|r| now <= r.at + WAIT_S + 300);
        g.outgoing.retain(|o| o.state == Out::Waiting || now <= o.at + KEEP_S);
        if g.made.as_ref().map_or(false, |m| m.expires_ms <= now * 1000) { g.made = None }
    }
    async fn announce(&self, hub: &Arc<Hub>) {
        let n = { let mut g = self.flow.inner.lock().unwrap(); self.sweep(&mut g); g.incoming.iter().filter(|r| r.state == In::Pending).count() };
        hub.set_info("machines.requests", &n.to_string()).await;
    }

    // ----- machine A: the code, the waiting request, the answer -----

    /// A machine code for another machine to type, with the address to type it with.
    pub async fn make_code(&self) -> Result<Value, String> {
        let peer = format!("p{}", hex::encode(urandom(8)));
        let r = self.reg.machine_code(&peer);
        if r["ok"] != true {
            return Err(if r["error"] == "too_many_invitations" { "too many open codes or invites: wait for one to expire".into() } else { "could not record the code".into() });
        }
        let address = botnet::url_here(self.tailnet, &self.host, self.port).await;
        let code = r["code"].as_str().unwrap_or("").to_string();
        let expires = r["expiresAt"].as_u64().unwrap_or(0);
        let addresses = self.routes.addresses(&address);
        self.flow.inner.lock().unwrap().made = Some(Made { code: code.clone(), peer, address: address.clone(), addresses: addresses.clone(), expires_ms: expires });
        Ok(json!({"code": code, "address": address, "addresses": addresses, "expiresAt": expires}))
    }

    /// `POST /bots/pair/request`: a machine asks to pair with a code. It waits; nothing is linked.
    pub(crate) async fn request(&self, hub: &Arc<Hub>, r: &PReq) -> HttpReply {
        let Ok(o) = serde_json::from_slice::<Value>(&r.body) else { return text("400 Bad Request", "not JSON") };
        let s = |k: &str| o[k].as_str().unwrap_or("").to_string();
        let (code, url, check) = (s("code"), s("url"), s("check"));
        let name = botnet::clean(&s("name"));
        if !botnet::url_ok(&url) || url.contains(['?', '#']) { return text("400 Bad Request", "no http address") }
        if !is_check(&check) { return text("400 Bad Request", "no check code") }
        let here = botnet::url_here(self.tailnet, &self.host, self.port).await;
        if let Some(why) = refuse_loopback(&url, &here) { return text("403 Forbidden", why) }
        {
            let mut g = self.flow.inner.lock().unwrap(); self.sweep(&mut g);
            if g.incoming.iter().filter(|x| x.state == In::Pending).count() >= MAX_PENDING { return text("429 Too Many Requests", "too many requests are waiting: answer them first") }
        }
        let claim = self.reg.machine_claim(&code);
        if claim["ok"] != true { let (st, why) = claim_reason(claim["error"].as_str().unwrap_or("")); return text(st, why) }
        let (id, poll) = (hex::encode(urandom(8)), hex::encode(urandom(16)));
        let shown = if name.is_empty() { machines::hostport(&url).to_string() } else { name };
        let rec = Incoming { id: id.clone(), peer: claim["peer"].as_str().unwrap_or("").into(), name: shown, url, check, at: self.now_s(), poll: digest(&poll), state: In::Pending };
        self.flow.inner.lock().unwrap().incoming.push(rec);
        self.announce(hub).await;
        jreply("200 OK", &json!({"id": id, "poll": poll, "wait": WAIT_S}))
    }

    /// `POST /bots/pair/poll {id, poll}`: pending, or the answer once.
    pub(crate) fn poll(&self, r: &PReq) -> HttpReply {
        let Ok(o) = serde_json::from_slice::<Value>(&r.body) else { return text("400 Bad Request", "not JSON") };
        let (id, poll) = (o["id"].as_str().unwrap_or(""), digest(o["poll"].as_str().unwrap_or("")));
        let mut g = self.flow.inner.lock().unwrap(); self.sweep(&mut g);
        let Some(at) = g.incoming.iter().position(|x| x.id == id && super::owner::same(&x.poll, &poll)) else { return jreply("404 Not Found", &json!({"state": "unknown"})) };
        match g.incoming[at].state.clone() {
            In::Pending => jreply("200 OK", &json!({"state": "pending"})),
            In::Approved(inv) => { g.incoming.remove(at); jreply("200 OK", &json!({"state": "approved", "invite": inv})) }
            In::Denied => { g.incoming.remove(at); jreply("200 OK", &json!({"state": "denied"})) }
        }
    }

    /// The person approves: the secret is kept here, the link is recorded as made by an approved pairing, and the machine
    /// that asked can collect its invite.
    pub async fn approve(&self, hub: &Arc<Hub>, id: &str) -> Result<(), String> {
        let (peer, here) = {
            let mut g = self.flow.inner.lock().unwrap(); self.sweep(&mut g);
            let Some(r) = g.incoming.iter().find(|x| x.id == id && x.state == In::Pending) else { return Err("that request has expired, or was answered".into()) };
            (r.peer.clone(), ())
        };
        let _ = here;
        let addr = botnet::url_here(self.tailnet, &self.host, self.port).await;
        let secret = botnet::fresh_secret();
        if !botnet::peer_save(&self.home, &peer, &secret) { return Err("could not keep the link's secret".into()) }
        let now = self.now_s();
        if hub.commit(&json!({"$":"SettingSet","key":format!("peer.own.{peer}"),"value":"approved","at":now})).await.is_err() {
            botnet::peer_forget(&self.home, &peer); return Err("could not record the approval".into());
        }
        {
            let mut g = self.flow.inner.lock().unwrap();
            match g.incoming.iter_mut().find(|x| x.id == id) { Some(r) => r.state = In::Approved(sign::invite_make(&addr, &peer, &secret)), None => return Err("that request has expired".into()) }
        }
        self.announce(hub).await;
        Ok(())
    }
    pub async fn deny(&self, hub: &Arc<Hub>, id: &str) -> Result<(), String> {
        {
            let mut g = self.flow.inner.lock().unwrap(); self.sweep(&mut g);
            match g.incoming.iter_mut().find(|x| x.id == id && x.state == In::Pending) { Some(r) => r.state = In::Denied, None => return Err("that request has expired, or was answered".into()) }
        }
        self.announce(hub).await;
        Ok(())
    }

    // ----- machine B: ask, wait, join -----

    /// Ask the hub at `address` to pair with `code`. Ok((attempt id, check code)): the check code is shown here and on the other
    /// machine's request; the link is made in the background if the person there approves (`attempt` tells how it went).
    pub async fn pair_with(self: &Arc<Self>, hub: &Arc<Hub>, address: &str, code: &str) -> Result<(String, String), String> {
        let Some(base) = normalize(address) else { return Err("that is not an address (like http://box.example:3787)".into()) };
        if code.trim().is_empty() { return Err("enter the code the other machine shows".into()) }
        let here = botnet::url_here(self.tailnet, &self.host, self.port).await;
        if let Some(why) = refuse_loopback(&here, &base) { return Err(why.into()) }
        if self.table.with(|p| p.live().iter().any(|x| machines::hostport(&x.url) == machines::hostport(&base))) { return Err("already linked to that machine".into()) }
        let (me, check) = (botnet::name().await, digits6());
        let body = json!({"code": code.trim(), "name": me, "url": here, "check": check}).to_string();
        let r = botnet::post_json(&self.home, &format!("{base}/bots/pair/request"), &body).await;
        if r.code != 200 {
            return Err(match r.code { 0 => format!("{base} does not answer"), c => format!("{} ({c})", r.body.trim().chars().take(200).collect::<String>()) });
        }
        let j: Value = serde_json::from_str(&r.body).map_err(|_| "the other machine answered nothing usable".to_string())?;
        let (rid, poll) = (j["id"].as_str().unwrap_or("").to_string(), j["poll"].as_str().unwrap_or("").to_string());
        if rid.is_empty() || poll.is_empty() { return Err("the other machine answered nothing usable".into()) }
        let id = hex::encode(urandom(6));
        self.flow.inner.lock().unwrap().outgoing.push(Outgoing { id: id.clone(), address: base.clone(), check: check.clone(), at: self.now_s(), state: Out::Waiting });
        let (me, hub) = (self.clone(), hub.clone());
        let attempt = id.clone();
        tokio::spawn(async move { let end = me.wait_for(&hub, &base, &rid, &poll).await; me.finish(&attempt, end) });
        Ok((id, check))
    }
    fn finish(&self, id: &str, end: Out) {
        let mut g = self.flow.inner.lock().unwrap();
        let now = self.now_s();
        if let Some(o) = g.outgoing.iter_mut().find(|o| o.id == id) { o.state = end; o.at = now }
    }
    async fn wait_for(&self, hub: &Arc<Hub>, base: &str, rid: &str, poll: &str) -> Out {
        let every = std::time::Duration::from_millis(if cfg!(test) { 60 } else { 1500 });
        let started = std::time::Instant::now();
        let body = json!({"id": rid, "poll": poll}).to_string();
        while started.elapsed().as_secs() <= WAIT_S + 15 {
            tokio::time::sleep(every).await;
            let r = botnet::post_json(&self.home, &format!("{base}/bots/pair/poll"), &body).await;
            if r.code == 0 { continue }
            let j: Value = serde_json::from_str(&r.body).unwrap_or(Value::Null);
            match (r.code, j["state"].as_str().unwrap_or("")) {
                (200, "pending") => continue,
                (200, "approved") => {
                    let Some(mut inv) = sign::invite_parse(j["invite"].as_str().unwrap_or("")) else { return Out::Refused("the other machine sent an unusable invite".into()) };
                    inv.url = base.to_string(); // (the address just reached is the one that works from here)
                    return match self.join(hub, &inv, "approved").await { Ok((_, name)) => Out::Linked(name), Err(e) => Out::Refused(e) };
                }
                (200, "denied") => return Out::Refused("the other machine refused, or did not answer in time".into()),
                _ => return Out::Refused("the other machine forgot the request".into()),
            }
        }
        Out::Refused("not approved in time".into())
    }
    /// How an attempt stands: {state: waiting|linked|refused, name?, error?}.
    pub fn attempt(&self, id: &str) -> Value {
        let g = self.flow.inner.lock().unwrap();
        match g.outgoing.iter().find(|o| o.id == id).map(|o| &o.state) {
            Some(Out::Waiting) => json!({"state": "waiting"}),
            Some(Out::Linked(n)) => json!({"state": "linked", "name": n}),
            Some(Out::Refused(e)) => json!({"state": "refused", "error": e}),
            None => json!({"state": "refused", "error": "unknown attempt"}),
        }
    }

    // ----- what the pages show -----

    /// The Machines page's data: linked machines (paired or not), requests to answer, attempts made from here, machines found, the open code.
    pub fn page_data(&self) -> Value {
        let now = self.now_s();
        let (reqs, outs, made) = { let mut g = self.flow.inner.lock().unwrap(); self.sweep(&mut g); (g.incoming.clone(), g.outgoing.clone(), g.made.clone()) };
        let linked: Vec<Value> = self.table.with(|p| p.live().into_iter().map(|x| {
            let st = if p.verified(&x.id) { "paired" } else if p.own_how(&x.id) == "no" { "no" } else { "unpaired" };
            json!({"id": x.id, "name": x.name, "url": x.url, "state": st})
        }).collect());
        let mj = self.routes.machines_value();
        let me = mj["self"]["url"].as_str().unwrap_or("");
        let found: Vec<Value> = mj["peers"].as_array().into_iter().flatten()
            .filter(|m| !linked.iter().any(|l| machines::hostport(l["url"].as_str().unwrap_or("")) == machines::hostport(m["url"].as_str().unwrap_or(""))) && m["url"].as_str().unwrap_or("") != me)
            .map(|m| json!({"name": m["name"], "url": m["url"], "same_user": m["same_user"] != false})).collect();
        json!({
            "linked": linked, "found": found,
            "requests": reqs.iter().filter(|r| r.state == In::Pending).map(|r| json!({"id": r.id, "name": r.name, "url": r.url, "check": r.check, "left": (r.at + WAIT_S).saturating_sub(now)})).collect::<Vec<_>>(),
            "outgoing": outs.iter().map(|o| { let mut v = json!({"id": o.id, "address": o.address, "check": o.check}); v["status"] = self_state(&o.state); v }).collect::<Vec<_>>(),
            "via": self.via.lock().unwrap().clone(),
            "code": made.map(|m| json!({"code": m.code, "address": m.address, "addresses": m.addresses, "left": (m.expires_ms / 1000).saturating_sub(now), "peer": m.peer})),
        })
    }

    // ----- the CLI: POST /bots/pair/local, with the hub's credential -----

    /// `backplane pair ...` on the machine of the hub: the hub's own token admits it, nothing else.
    pub(crate) async fn local(&self, hub: &Arc<Hub>, r: &PReq) -> HttpReply {
        if !self.credential(r) { return text("401 Unauthorized", "this Backplane needs its pairing token") }
        let Ok(o) = serde_json::from_slice::<Value>(&r.body) else { return text("400 Bad Request", "not JSON") };
        let s = |k: &str| o[k].as_str().unwrap_or("").to_string();
        let fail = |e: String| jreply("200 OK", &json!({"ok": false, "error": e}));
        match o["op"].as_str().unwrap_or("") {
            "device" => { let Some(rt) = super::start::routes() else { return fail("no registry".into()) }; let mut j = rt.invite(); if j["error"].is_string() { return fail(j["error"].as_str().unwrap_or("").into()) } j["ok"] = json!(true); jreply("200 OK", &j) }
            "machine" => match self.make_code().await { Ok(mut j) => { j["ok"] = json!(true); jreply("200 OK", &j) } Err(e) => fail(e) },
            "join" => {
                let (a, c) = (s("address"), s("code"));
                if a.starts_with("bp1:") || c.starts_with("bp1:") {
                    let inv = if a.starts_with("bp1:") { a } else { c };
                    return match sign::invite_parse(&inv) { None => fail("not an invite".into()), Some(i) => match self.join(hub, &i, "").await { Ok((_, n)) => jreply("200 OK", &json!({"ok": true, "linked": n})), Err(e) => fail(e) } };
                }
                // (the hub is behind an Arc owned by the server; pair_with needs it, so the route reaches it through start::peering)
                let Some(me) = super::start::peering() else { return fail("pairing is not running".into()) };
                match me.pair_with(hub, &a, &c).await { Ok((id, check)) => jreply("200 OK", &json!({"ok": true, "id": id, "check": check})), Err(e) => fail(e) }
            }
            "status" => { let mut j = self.attempt(&s("id")); j["ok"] = json!(true); jreply("200 OK", &j) }
            "requests" => { let d = self.page_data(); jreply("200 OK", &json!({"ok": true, "requests": d["requests"], "outgoing": d["outgoing"]})) }
            "approve" => match self.approve(hub, &s("id")).await { Ok(()) => jreply("200 OK", &json!({"ok": true})), Err(e) => fail(e) },
            "deny" => match self.deny(hub, &s("id")).await { Ok(()) => jreply("200 OK", &json!({"ok": true})), Err(e) => fail(e) },
            _ => text("400 Bad Request", "unknown op"),
        }
    }
}
fn self_state(s: &Out) -> Value {
    match s { Out::Waiting => json!("waiting"), Out::Linked(n) => json!(format!("linked: {n}")), Out::Refused(e) => json!(format!("refused: {e}")) }
}

#[cfg(test)]
mod tests;
