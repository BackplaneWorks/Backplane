//! `device.register` / `device.drop`: a phone registers the push token it holds for
//! alerts, Live Activities and activity starts; the events carry the token, and no client ever gets one (the wire
//! form blanks it, `Hub::wire_item`: laws `device_token_never_sent`, `device_drop_never_sent`; any line a net route gives a
//! client goes through it too). Pure
//! rules here; the handler commits the event hub commits.
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

/// Device.char: a push token or bundle id is letters, digits and . _ : - only (APNs tokens are hex, FCM tokens
/// base64url with a colon), so neither can bend a URL or a header.
fn ch(c: char) -> bool { c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-') }
/// Device.safe: not empty, at most 512 characters, only those.
pub fn safe(s: &str) -> bool { !s.is_empty() && s.chars().count() <= 512 && s.chars().all(ch) }
fn one_of(names: &[&str], s: &str) -> bool { names.contains(&s) }
fn get<'a>(p: &'a Value, k: &str, d: &'a str) -> &'a str { p.get(k).and_then(Value::as_str).unwrap_or(d) }
/// Device.ok: a safe token and bundle, a known platform and kind, and a known APNs environment (production by default).
pub fn ok(p: &Value) -> bool {
    safe(get(p, "token", "")) && safe(get(p, "bundle", "")) && one_of(&["ios", "android"], get(p, "platform", ""))
        && one_of(&["alert", "activity", "start"], get(p, "kind", "")) && one_of(&["sandbox", "production"], get(p, "env", "production"))
}
/// Device.register's event: None for "bad device".
pub fn registered(p: &Value, now: u64) -> Option<Value> {
    ok(p).then(|| json!({"$":"DeviceRegistered","token":get(p,"token",""),"platform":get(p,"platform",""),"kind":get(p,"kind",""),
        "thread":get(p,"thread",""),"env":get(p,"env","production"),"bundle":get(p,"bundle",""),"at":now}))
}
/// device.drop's event.
pub fn dropped(p: &Value, now: u64) -> Value { json!({"$":"DeviceDropped","token":get(p,"token",""),"at":now}) }

/// device.prefs' event: this phone's mute, by its token.
pub fn prefs(p: &Value, now: u64) -> Option<Value> { safe(get(p, "token", "")).then(|| json!({"$":"DevicePrefs","token":get(p,"token",""),"mute":p["mute"] == true,"at":now})) }

pub struct Devices;
impl RpcHandler for Devices {
    fn methods(&self) -> &'static [&'static str] { &["device.register", "device.drop", "device.prefs"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let now = crate::hub::now_ms() / 1000;
            let made = match method { "device.register" => registered(p, now), "device.prefs" => prefs(p, now), _ => Some(dropped(p, now)) };
            let Some(ev) = made else { return vec![json!({"t":"reply","id":id,"ok":false,"error":"bad device"})] };
            match hub.commit(&ev).await { Ok(_) => vec![json!({"t":"reply","id":id,"ok":true})], Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})] }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn good() -> Value { json!({"token":"ab12:CD-3_4.5","platform":"ios","kind":"alert","bundle":"dev.backplane.works","env":"sandbox","thread":"t1"}) }
    #[test] fn device_register_checks_every_field() {
        assert!(ok(&good()));
        let mut p = good(); p["env"] = json!("staging"); assert!(!ok(&p));
        for (k, v) in [("token", ""), ("token", "a b"), ("token", "a/b"), ("bundle", ""), ("bundle", "x;y"), ("platform", "web"), ("kind", "push"), ("platform", "")] {
            let mut p = good(); p[k] = json!(v); assert!(!ok(&p), "{k}={v:?}");
        }
        let mut p = good(); p["token"] = json!("a".repeat(513)); assert!(!ok(&p)); p["token"] = json!("a".repeat(512)); assert!(ok(&p));
        let mut p = good(); p.as_object_mut().unwrap().remove("env"); assert!(ok(&p), "production is the default");
        p["kind"] = json!("start"); assert!(ok(&p)); p["kind"] = json!("activity"); assert!(ok(&p));
    }
    #[test] fn device_register_event_matches() {
        assert_eq!(registered(&good(), 7).unwrap(), json!({"$":"DeviceRegistered","token":"ab12:CD-3_4.5","platform":"ios","kind":"alert","thread":"t1","env":"sandbox","bundle":"dev.backplane.works","at":7}));
        let mut p = good(); p.as_object_mut().unwrap().remove("env"); p.as_object_mut().unwrap().remove("thread");
        let e = registered(&p, 8).unwrap(); assert_eq!((e["env"].as_str(), e["thread"].as_str()), (Some("production"), Some("")));
        assert!(registered(&json!({}), 1).is_none());
    }
    #[test] fn device_prefs_need_a_safe_token() {
        assert_eq!(prefs(&json!({"token":"t1","mute":true}), 4).unwrap(), json!({"$":"DevicePrefs","token":"t1","mute":true,"at":4}));
        assert_eq!(prefs(&json!({"token":"t1"}), 4).unwrap()["mute"], false);
        assert!(prefs(&json!({"token":"a b","mute":true}), 4).is_none() && prefs(&json!({}), 4).is_none());
    }
    #[test] fn device_drop_event() { assert_eq!(dropped(&json!({"token":"t"}), 3), json!({"$":"DeviceDropped","token":"t","at":3})); assert_eq!(dropped(&json!({}), 3)["token"], ""); }
    #[tokio::test] async fn device_rpc_commits_and_answers() {
        use crate::{hub::{Cfg, Hub}, presenter::Canned};
        let h = std::env::temp_dir().join(format!("bp-devices-{}", std::process::id())); let _ = std::fs::remove_dir_all(&h); std::fs::create_dir_all(&h).unwrap();
        let hub = Hub::start(Cfg { home: h.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let who = Caller { gen: String::new(), client: 1 };
        let n0 = hub.store.len();
        let r = Devices.call(&hub, &who, &json!(5), "device.register", &good()).await;
        assert_eq!(r, vec![json!({"t":"reply","id":5,"ok":true})]); assert_eq!(hub.store.len(), n0 + 1);
        let line = hub.store.get(n0).unwrap(); assert!(line.contains("DeviceRegistered") && line.contains("ab12:CD-3_4.5"), "the log keeps the token");
        let r = Devices.call(&hub, &who, &json!(6), "device.register", &json!({"token":"x"})).await;
        assert_eq!(r, vec![json!({"t":"reply","id":6,"ok":false,"error":"bad device"})]); assert_eq!(hub.store.len(), n0 + 1, "a bad registration commits nothing");
        let r = Devices.call(&hub, &who, &json!(7), "device.drop", &json!({"token":"ab12:CD-3_4.5"})).await;
        assert_eq!(r[0]["ok"], true); assert!(hub.store.get(n0 + 1).unwrap().contains("DeviceDropped"));
        let _ = std::fs::remove_dir_all(h);
    }
}
