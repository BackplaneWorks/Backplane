//! The Settings > Machines requests: `bots.invite` (make an invite to paste on another machine), `bots.join` (use one),
//! `bots.unpeer` (remove a link), `bots.peer_own` (the owner says whether a link is one of their machines). The same
//! machinery as the automatic pairing (`peering.rs`); the replies are hub's (`BotJob.reply`).
use super::peering::Peering;
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct PeerRpc(pub Arc<Peering>);
fn ok(id: &Value, extra: Value) -> Vec<Value> { let mut r = json!({"t":"reply","id":id,"ok":true}); if let (Some(o), Some(e)) = (r.as_object_mut(), extra.as_object()) { for (k, v) in e { o.insert(k.clone(), v.clone()); } } vec![r] }
fn err(id: &Value, why: &str) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":false,"error":why})] }
fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }

impl RpcHandler for PeerRpc {
    fn methods(&self) -> &'static [&'static str] { &["bots.invite", "bots.join", "bots.unpeer", "bots.peer_own", "machines.page", "machines.page.act", "machines.code", "machines.pair", "machines.attempt", "machines.requests", "machines.approve", "machines.deny", "mesh.machines"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _w: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match method {
                "bots.invite" => match self.0.invite_for(&s("url")).await { Ok((inv, peer)) => ok(id, json!({"invite": inv, "peer": peer})), Err(e) => err(id, &e) },
                "machines.page" => ok(id, json!({"rows": super::machinespage::rows(&self.0.page_data(), &|_| String::new())})),
                "machines.page.act" => match super::machinespage::act(&self.0, hub, &s("action"), &s("value"), &p["fields"]).await { Ok(x) => ok(id, x), Err(e) => err(id, &e) },
                "machines.code" => match self.0.make_code().await { Ok(x) => ok(id, x), Err(e) => err(id, &e) },
                "machines.pair" => match self.0.pair_with(hub, &s("address"), &s("code")).await { Ok((a, c)) => ok(id, json!({"attempt": a, "check": c})), Err(e) => err(id, &e) },
                "machines.attempt" => ok(id, self.0.attempt(&s("attempt"))),
                "machines.requests" => { let d = self.0.page_data(); ok(id, json!({"requests": d["requests"], "outgoing": d["outgoing"]})) }
                "machines.approve" => match self.0.approve(hub, &s("request")).await { Ok(()) => ok(id, json!({})), Err(e) => err(id, &e) },
                "machines.deny" => match self.0.deny(hub, &s("request")).await { Ok(()) => ok(id, json!({})), Err(e) => err(id, &e) },
                // the paired machines by name (and machine id), and what a text's `~machine` words mean to an agent: for clients, tests and `backplane pair`
                "mesh.machines" => match crate::bots::api::of(hub) {
                    None => err(id, "bots are not running"),
                    Some(b) => { let rows = b.mesh_rows();
                        let ms: Vec<Value> = rows.iter().map(|m| json!({"name": m.name, "word": crate::bots::mesh::word(&m.name), "link": m.link, "mid": b.mid_known(&m.link).unwrap_or_default(), "away": m.away, "projects": m.projects.iter().map(|p| json!({"id": p.0, "title": p.1})).collect::<Vec<_>>()})).collect();
                        ok(id, json!({"mid": b.my_mid(), "machines": ms, "lines": crate::agents::refs::machine_lines(&rows, &s("text")), "prompt": b.machines_prompt()})) }
                },
                "bots.join" => match super::sign::invite_parse(&s("invite")) {
                    None => err(id, "not an invite"),
                    Some(i) => match self.0.join(hub, &i, "").await { Ok((peer, name)) => ok(id, json!({"peer": peer, "name": name})), Err(e) => err(id, &e) },
                },
                _ => {
                    let peer = s("peer");
                    if !self.0.table.with(|t| t.peer(&peer).map_or(false, |r| !r.revoked)) { return err(id, "no such link") }
                    let ev = if method == "bots.unpeer" { json!({"$":"PeerSet","id":peer,"name":"","url":"","revoked":true,"at":now()}) }
                        else { json!({"$":"SettingSet","key":format!("peer.own.{peer}"),"value":if p["own"] == true { "owner" } else { "no" },"at":now()}) };
                    match hub.commit(&ev).await {
                        Ok(_) => { if method == "bots.unpeer" { self.0.unpeered(&peer) } ok(id, json!({})) }
                        Err(e) => err(id, &format!("{e:#}")),
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::peering::tests::{know, node, settle, url};
    async fn call(r: &PeerRpc, hub: &Arc<Hub>, m: &str, p: Value) -> Value { r.call(hub, &Caller { gen: String::new(), client: 1 }, &json!(7), m, &p).await.remove(0) }

    #[tokio::test] async fn pair_a_person_pastes_an_invite_and_the_machines_link() {
        let (a, b) = (node("ra", false).await, node("rb", false).await);
        know(&a, &b); know(&b, &a);
        let (ra, rb) = (PeerRpc(a.p.clone()), PeerRpc(b.p.clone()));
        let inv = call(&ra, &a.hub, "bots.invite", json!({})).await;
        assert_eq!(inv["ok"], true); let invite = inv["invite"].as_str().unwrap();
        assert!(invite.starts_with(&format!("bp1:{}:", url(&a))) && inv["peer"].as_str().unwrap().starts_with('p'), "{inv}");
        let joined = call(&rb, &b.hub, "bots.join", json!({"invite": invite})).await;
        assert_eq!(joined["ok"], true, "{joined}"); settle().await;
        let id = inv["peer"].as_str().unwrap().to_string();
        assert!(a.p.table.with(|t| t.peer(&id).map_or(false, |r| !r.revoked)) && b.p.table.with(|t| t.peer(&id).map_or(false, |r| !r.revoked)), "both hold the link");
        // a pasted invite is anyone's: not the owner's until the owner says so
        assert!(!a.p.table.with(|t| t.verified(&id)), "a pasted invite is not a pairing");
        assert_eq!(call(&rb, &b.hub, "bots.join", json!({"invite": invite})).await["error"], "already linked to that machine");
        assert_eq!(call(&rb, &b.hub, "bots.join", json!({"invite": "nonsense"})).await["error"], "not an invite");
        // the owner says Mine / Not mine
        assert_eq!(call(&ra, &a.hub, "bots.peer_own", json!({"peer": id, "own": true})).await["ok"], true); assert!(a.p.table.with(|t| t.verified(&id)));
        call(&ra, &a.hub, "bots.peer_own", json!({"peer": id, "own": false})).await; assert!(!a.p.table.with(|t| t.verified(&id)) && a.p.table.with(|t| t.own_how(&id).to_string()) == "no");
        assert_eq!(call(&ra, &a.hub, "bots.peer_own", json!({"peer": "pnope"})).await["error"], "no such link");
        // unpeer removes the link and its secret, and the invite shows as unlinked
        assert!(crate::net::botnet::peer_secret(&a.home, &id).len() == 64);
        assert_eq!(call(&ra, &a.hub, "bots.unpeer", json!({"peer": id})).await["ok"], true);
        assert!(a.p.table.with(|t| t.peer(&id).map_or(true, |r| r.revoked)) && crate::net::botnet::peer_secret(&a.home, &id).is_empty());
        assert_eq!(call(&ra, &a.hub, "bots.unpeer", json!({"peer": id})).await["error"], "no such link", "a removed link is gone");
        assert_eq!(a.p.reg.list()["invitations"][0]["deviceRevoked"], true);
    }
    #[tokio::test] async fn pair_open_invites_are_capped_and_an_address_may_be_named() {
        let a = node("rc", false).await; let r = PeerRpc(a.p.clone());
        let inv = call(&r, &a.hub, "bots.invite", json!({"url": "http://box.example:3787"})).await;
        assert!(inv["invite"].as_str().unwrap().starts_with("bp1:http://box.example:3787:"), "{inv}");
        for _ in 0..15 { assert_eq!(call(&r, &a.hub, "bots.invite", json!({})).await["ok"], true); }
        assert_eq!(call(&r, &a.hub, "bots.invite", json!({})).await["error"], "too many open invites: wait for one to expire, or revoke one in Settings > Devices");
    }
}
