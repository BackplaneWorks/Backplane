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
