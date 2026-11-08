//! Extending the mesh: when A is linked to B, A can see B's other machines (`bots.hello` lists them) and pair with one in a single approval.
//! Nothing is trusted because B says so: B only helps A get a **machine code** from C (`bots.mesh_code`, over the signed link B has with C, which
//! is one of the owner's own), A then pairs with C by that code exactly as a person would, and the request waits on C for the person's approval.
//! A code costs C nothing it would not give a person who asked (ten minutes, once, a request still to approve).
use super::{farsync::FarSync, machines::hostport, peering::Peering};
use crate::{api::HttpReply, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

fn json_reply(v: &Value) -> HttpReply { HttpReply { status: "200 OK", ctype: "application/json".into(), extra: String::new(), body: v.to_string().into_bytes() } }

/// `bots.mesh_code` on the machine asked (the route has vouched for the caller as one of the owner's own). Without `url`: a machine code made here.
/// With `url`: asked of the linked machine at that address (one hop only: that machine is sent no `url`), and its code and address come back.
pub async fn serve_code(sync: &Arc<FarSync>, hub: &Arc<Hub>, p: &Value) -> HttpReply {
    let url = p["url"].as_str().unwrap_or("");
    if url.is_empty() {
        return match sync.peering.make_code().await { Ok(j) => json_reply(&j), Err(e) => HttpReply::text("409 Conflict", &e) };
    }
    let Some(b) = crate::bots::api::of(hub) else { return HttpReply::text("503 Service Unavailable", "bots are not running") };
    match b.mesh_code(None, url).await { Ok((code, address)) => json_reply(&json!({"code": code, "address": address})), Err(e) => HttpReply::text("502 Bad Gateway", &e) }
}

impl Peering {
    /// Pair with the machine at `url`, which the linked machine `via` is linked to: its code comes through `via` (see the module), then the ordinary
    /// request that waits for approval on that machine. Ok((attempt, check code)) as `pair_with`.
    pub async fn introduce(self: &Arc<Self>, hub: &Arc<Hub>, via: &str, url: &str) -> Result<(String, String), String> {
        let Some(b) = crate::bots::api::of(hub) else { return Err("bots are not running".into()) };
        let (code, address) = b.mesh_code(Some(via), url).await?;
        // the address in the code is the machine's own word for itself; it has to be the one that was asked for
        if hostport(&address).split(':').next() != hostport(url).split(':').next() { return Err("that machine named another address than the one asked for".into()) }
        self.pair_with(hub, &address, &code).await
    }
}

// ---- a link's address changes: the peer says so, signed, and this machine keeps the new one ----------------------------------------------------------

/// What was last told to each link, and when: an address is sent again when it changes and every ten minutes.
static TOLD: std::sync::Mutex<Vec<(String, String, u64)>> = std::sync::Mutex::new(Vec::new());
fn due(told: &mut Vec<(String, String, u64)>, link: &str, url: &str, now: u64) -> bool {
    match told.iter_mut().find(|t| t.0 == link) {
        Some(t) if t.1 == url && now < t.2 + 600 => false,
        Some(t) => { t.1 = url.to_string(); t.2 = now; true }
        None => { told.push((link.to_string(), url.to_string(), now)); true }
    }
}
/// Tell each approved link where this hub is now (`peer.address`, a signed /far/rpc to the address that link has for us): a hub that listens on loopback
/// behind `tailscale serve`, or that moved, is otherwise only at the address it gave when it was paired. A loopback address is told only to a link that is on this computer too.
pub async fn announce(hub: &Arc<Hub>, peering: &Peering) {
    if crate::copy::on() { return }
    let here = super::botnet::url_here(peering.tailnet, &peering.host, peering.port).await;
    let here_loop = super::machines::loopback(&here);
    let now = (peering.now)();
    let links: Vec<(String, String)> = peering.table.with(|p| p.live().into_iter().filter(|x| p.verified(&x.id) && !p.sync_off(&x.name)).map(|x| (x.id, x.url)).collect());
    for (id, url) in links {
        if here_loop && !super::machines::loopback(&url) { continue } // a loopback address means nothing to a machine that is not on this computer
        if !due(&mut TOLD.lock().unwrap(), &id, &here, now) { continue }
        let body = serde_json::json!({"id": 0, "m": "peer.address", "p": {"url": here}}).to_string();
        let r = super::botnet::to_peer(&peering.home, &url, &id, now, "/far/rpc", true, &super::rpc::nonced(&body)).await;
        if r.code != 200 { TOLD.lock().unwrap().retain(|t| t.0 != id); } // not taken (away, or an older build): asked again at the next pass
    }
}
/// `peer.address` on the machine told: the link (signed, so the same peer id) says it is now at `url`. Taken for an approved link only, and only when
/// that address answers /hello and is one this machine could reach (not a loopback one for a machine that is not on loopback itself).
pub async fn address_received(sync: &Arc<FarSync>, hub: &Arc<Hub>, link: &str, p: &Value) -> HttpReply {
    let url = p["url"].as_str().unwrap_or("").trim_end_matches('/').to_string();
    let peering = &sync.peering;
    if !peering.table.with(|t| t.verified(link) && t.peer(link).map_or(false, |r| !r.revoked)) { return HttpReply::text("403 Forbidden", "not an approved link") }
    if !super::botnet::url_ok(&url) || url.contains(['?', '#']) { return HttpReply::text("400 Bad Request", "no http address") }
    let here = super::botnet::url_here(peering.tailnet, &peering.host, peering.port).await;
    if let Some(why) = super::peering::refuse_loopback(&url, &here) { return HttpReply::text("403 Forbidden", why) }
    if peering.table.with(|t| t.peer(link).map_or(false, |r| r.url == url)) { return json_reply(&json!({"ok": true, "changed": false})) }
    if !super::peerhttp::link_up(&url).await { return HttpReply::text("409 Conflict", "that address does not answer") }
    match hub.commit(&json!({"$":"PeerSet","id":link,"name":"","url":url,"revoked":false,"at":(peering.now)()})).await {
        Ok(_) => json_reply(&json!({"ok": true, "changed": true})), Err(e) => HttpReply::text("503 Service Unavailable", &format!("{e:#}")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn an_address_is_told_again_when_it_changes_or_after_ten_minutes() {
        let mut t = vec![];
        assert!(due(&mut t, "L", "https://a", 100) && !due(&mut t, "L", "https://a", 200) && due(&mut t, "L", "https://b", 210) && !due(&mut t, "L", "https://b", 700) && due(&mut t, "L", "https://b", 811));
        assert!(due(&mut t, "M", "https://b", 300), "each link has its own");
    }
}
