//! The small Settings requests: `models.refresh`, `update.check`, `app.update`.
//!
//! This hub is a packaged binary (info `update: "package"`): it never replaces itself and looks for no release (the JS hub's managed install does
//! the same: `Media.check` answers an empty version, `Upgrade.go` does nothing). So `update.check` answers `{version: "", channel}` and `app.update`
//! only clears the `update.wait` mark, as the Bend hub does before its (here empty) upgrade. `models.refresh` sends the models info again; the CLI
//! probing it would redo (opencode, codex, grok, muse model listings) is not ported, so the list is the built-in one.
use super::models;
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct SettingsMisc;
impl RpcHandler for SettingsMisc {
    fn methods(&self) -> &'static [&'static str] { &["models.refresh", "update.check", "app.update"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, _p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            match method {
                "models.refresh" => { hub.push_all(&json!({"t":"info","info":{"models":models::info(),"models.all":models::all()}})); vec![json!({"t":"reply","id":id,"ok":true})] }
                "update.check" => {
                    let channel = hub.setting("update.channel").await.filter(|c| !c.is_empty()).unwrap_or_else(|| "stable".into());
                    vec![json!({"t":"reply","id":id,"ok":true,"result":{"version":"","channel":channel}})]
                }
                _ => {
                    // Upgrade.go on a packaged install: nothing is replaced, every client is told so
                    let cleared = if hub.setting("update.wait").await.unwrap_or_default().is_empty() { Ok(0) } else { hub.commit(&json!({"$":"SettingSet","key":"update.wait","value":"","at":now_ms() / 1000})).await };
                    hub.push_all(&json!({"t":"update","error":"this install updates through its package"}));
                    match cleared { Ok(_) => vec![json!({"t":"reply","id":id,"ok":true})], Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})] }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    #[tokio::test(flavor = "multi_thread")] async fn a_packaged_hub_finds_no_update_and_refreshes_the_models_it_has() {
        let home = std::env::temp_dir().join(format!("bp-misc-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let call = |m: &'static str| { let hub = hub.clone(); async move { SettingsMisc.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(4), m, &json!({})).await.remove(0) } };
        assert_eq!(call("update.check").await, json!({"t":"reply","id":4,"ok":true,"result":{"version":"","channel":"stable"}}));
        hub.commit(&json!({"$":"SettingSet","key":"update.channel","value":"beta","at":1})).await.unwrap(); assert_eq!(call("update.check").await["result"]["channel"], "beta");
        assert_eq!(call("app.update").await["ok"], true);
        assert_eq!(call("models.refresh").await, json!({"t":"reply","id":4,"ok":true}));
        let _ = std::fs::remove_dir_all(&home);
    }
}
