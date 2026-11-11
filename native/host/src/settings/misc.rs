//! The small Settings requests: `models.refresh` sends the models info again and asks the CLIs once more (models_probe.rs); a list that changed
//! follows on its own. (`update.check` and `app.update` are in `update/svc.rs`.)
use super::models;
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct SettingsMisc;
impl RpcHandler for SettingsMisc {
    fn methods(&self) -> &'static [&'static str] { &["models.refresh"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, _p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let _ = method;
            hub.push_all(&json!({"t":"info","info":{"models":models::info(),"models.all":models::all()}}));
            if models::live() { let h = hub.clone(); tokio::spawn(async move { super::models_probe::refresh(&h).await; }); }
            vec![json!({"t":"reply","id":id,"ok":true})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    #[tokio::test(flavor = "multi_thread")] async fn a_hub_refreshes_the_models_it_has() {
        let home = crate::plat::tmp::dir("misc");
        let hub = Hub::start(Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let call = |m: &'static str| { let hub = hub.clone(); async move { SettingsMisc.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(4), m, &json!({})).await.remove(0) } };
        assert_eq!(call("models.refresh").await, json!({"t":"reply","id":4,"ok":true}));
        let _ = std::fs::remove_dir_all(&home);
    }
}
