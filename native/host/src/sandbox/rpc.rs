//! `sandbox.status`: which backends a use would try, and for Nitro the settings, the plan that would make its parent and what it costs.
//! Read-only: it never calls AWS (that needs `sandbox.nitro` on and a job), and it never runs the plan.
use super::{nitro, order, Use};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct SandboxRpc;
pub const KEYS: &[&str] = &["sandbox.nitro", "sandbox.nitro.threads", "sandbox.nitro.hub", "sandbox.boat", "sandbox.nitro.region", "sandbox.nitro.profile", "sandbox.nitro.instance",
    "sandbox.nitro.ssh", "sandbox.nitro.type", "sandbox.nitro.disk", "sandbox.nitro.idle", "sandbox.nitro.kms", "sandbox.nitro.mode"];

/// The answer, from the settings (a pure function: the RPC reads the settings, the test passes its own).
pub fn status(get: &dyn Fn(&str) -> Option<String>, boat_cli: bool) -> Value {
    let c = nitro::Config::from(get);
    let plan: Vec<String> = nitro::plan(&c).into_iter().map(|a| a.join(" ")).collect();
    json!({
        "order": {"build": order(get, Use::Build, boat_cli), "thread": order(get, Use::Thread, boat_cli), "hub": order(get, Use::Hub, boat_cli)},
        "nitro": {"on": c.on, "region": c.region, "instance": c.instance, "ssh": c.ssh, "type": c.itype, "mode": if c.direct { "direct" } else { "confidential" },
            "kms": c.kms, "idle": c.idle, "plan": plan, "cost": nitro::cost(&c.itype, c.disk_gib, 8.0, 4)},
    })
}

/// The parent's state as AWS sees it, or why it was not asked.
pub fn probe(c: nitro::Config) -> Value {
    let n = nitro::Nitro::new(c, &super::Real, "");
    if let Err(e) = super::Backend::ready(&n) { return json!({"backend": super::Backend::name(&n), "error": e}) }
    if !nitro::valid_instance(&n.cfg.instance) { return json!({"backend": "nitro", "ssh": n.cfg.ssh, "note": "SSH-only parent: AWS is not asked"}) }
    match n.describe() { Ok(h) => json!({"backend": "nitro", "state": h.state, "type": h.itype, "dns": h.dns}), Err(e) => json!({"backend": "nitro", "error": e}) }
}

impl RpcHandler for SandboxRpc {
    fn methods(&self) -> &'static [&'static str] { &["sandbox.status"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let mut have = std::collections::HashMap::new();
            for k in KEYS { if let Some(v) = hub.setting(k).await { have.insert(k.to_string(), v); } }
            let boat_cli = std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("boat").is_file()));
            let mut r = status(&|k| have.get(k).cloned(), boat_cli);
            // probe: ask AWS what the parent is doing (describe only: free, and it never starts or makes anything). Only when Nitro is on.
            if p.get("probe") == Some(&json!(true)) {
                let c = nitro::Config::from(&|k| have.get(k).cloned());
                r["probe"] = tokio::task::spawn_blocking(move || probe(c)).await.unwrap_or_else(|e| json!({"error": e.to_string()}));
            }
            r["t"] = json!("reply"); r["id"] = id.clone(); r["ok"] = json!(true);
            vec![r]
        })
    }
}
