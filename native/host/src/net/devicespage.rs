//! Settings → Devices on a phone (data/1 feature `devices`): `devices.page {state?}` and `devices.page.act {action, value, state?}`,
//! each answering `{rows, state}`. The rows are the desktop's (`present::settings::devices`), so the phone draws what the hub decides:
//! the address choice, the link with its QR code and short code, the local network offer, the paired devices and the open links.
//! `state` is the page's own drafts (the invitation made here, the address shown), handed to the phone and back as one opaque string,
//! so the hub keeps nothing per phone. A data/1 socket holds the hub's token or a paired device's (full access), so the invitation in it
//! is that device's own to hand on.
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

/// The drafts a state may carry (anything else in it is dropped).
const KEYS: &[&str] = &["@invite", "@pair.url", "@lan.err"];
/// A state string back as a session (`{"drafts": {...}}`): only the page's own drafts, strings of a bounded size.
pub fn session_of(state: &str) -> Value {
    let v: Value = serde_json::from_str(state).unwrap_or(Value::Null);
    let mut d = serde_json::Map::new();
    for k in KEYS { if let Some(x) = v[*k].as_str().filter(|x| x.len() <= 16384) { d.insert(k.to_string(), json!(x)); } }
    json!({"drafts": d})
}
fn state_of(session: &Value) -> String {
    let mut d = serde_json::Map::new();
    for k in KEYS { if let Some(x) = session["drafts"][*k].as_str().filter(|x| !x.is_empty()) { d.insert(k.to_string(), json!(x)); } }
    Value::Object(d).to_string()
}
/// The page for a session: `{rows, state}`.
pub fn page(session: &Value) -> Value {
    let info = json!({"devices": super::start::routes().map_or(Value::Null, |r| r.devices())});
    let rows = crate::statehost::policies().and_then(|p| p.with_state(|st| crate::present::settings::devices(st, session, &info))).unwrap_or_default();
    json!({"rows": rows, "state": state_of(session)})
}

pub struct DevicesPage;
impl RpcHandler for DevicesPage {
    fn methods(&self) -> &'static [&'static str] { &["devices.page", "devices.page.act"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let mut session = session_of(&s("state"));
            if method == "devices.page.act" {
                let action = s("action");
                if !action.starts_with("device-") { return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown action"})] }
                let (a, v) = (action.clone(), s("value"));
                // the registry and the listener are written to disk: off the async threads
                let patch = tokio::task::spawn_blocking(move || crate::present::settings::device_act(&a, &v)).await.ok().flatten();
                match patch { Some(patch) => crate::session::patch::merge(&mut session, &patch), None => return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown action"})] }
            }
            let mut r = json!({"t":"reply","id":id,"ok":true});
            let pg = page(&session);
            r["rows"] = pg["rows"].clone(); r["state"] = pg["state"].clone();
            vec![r]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_state_carries_only_the_pages_drafts() {
        let s = session_of(r#"{"@invite":"{\"link\":\"x\"}","@pair.url":"http://192.168.1.9:3787","@other":"no","@lan.err":7}"#);
        assert_eq!(s, json!({"drafts": {"@invite": "{\"link\":\"x\"}", "@pair.url": "http://192.168.1.9:3787"}}));
        assert_eq!(session_of("garbage"), json!({"drafts": {}}));
        let back: Value = serde_json::from_str(&state_of(&s)).unwrap();
        assert_eq!(back, json!({"@invite": "{\"link\":\"x\"}", "@pair.url": "http://192.168.1.9:3787"}));
        assert_eq!(state_of(&json!({})), "{}");
    }
}
