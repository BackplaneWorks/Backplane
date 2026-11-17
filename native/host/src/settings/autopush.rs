//! `git.auto_push` (#1171): whether the hub pushes the design commits it makes at a turn's end or a save (kicad/canonical.rs `commit_design`) to
//! the branch's upstream. Off unless the person turns it on ("on"); auto-commit itself is unchanged and stays local. Nothing else in the hub
//! pushes on its own: Commit & push, PR and a release's Publish are presses (files/git.rs, files/rv.rs).
//! With it off, a project's page says how many commits its main checkout has that the upstream lacks, with Push (`project.push`).
//! An install that predates the setting pushed on its own; on its first start with this version (`boot`) it is marked for a one-time notice
//! (`git.auto_push.notice`: "due"; a new install, or one where the setting was already chosen: "none"). The notice is offered while it is due and
//! the setting unchosen; drawn, Close or Settings writes "closed", and it never comes back.
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

pub const KEY: &str = "git.auto_push";
pub const NOTICE: &str = "git.auto_push.notice";
pub const TOKEN: &str = "git-autopush-notice-1";
pub const TEXT: &str = "Backplane no longer pushes automatically. Turn it on in Settings if you want it.";

/// Push on its own: only when the person turned it on.
pub fn on(v: &str) -> bool { v == "on" }
/// What the first start of this version writes to `NOTICE` (None: it was written before). `existing`: the home was in use before (settings/telemetry.rs
/// `existing_hub`); `chosen`: the setting as it stands, a value the person (or their own setup) set already.
pub fn boot_mark(existing: bool, notice: &str, chosen: &str) -> Option<&'static str> {
    if !notice.is_empty() { return None }
    Some(if existing && chosen.is_empty() { "due" } else { "none" })
}
/// The notice as the clients draw it (screens/tmnotice.rs), null for none.
pub fn offer(notice: &str, chosen: &str) -> Value {
    if notice != "due" || !chosen.is_empty() { return Value::Null }
    json!({"token": TOKEN, "text": TEXT, "settings": "Settings", "act": "gitpush.notice.act", "page": "git"})
}
/// What a press on the notice writes: Close and Settings close it for good (drawing it changes nothing: it stays until it is answered).
pub fn act(notice: &str, token: &str, action: &str) -> Option<&'static str> {
    (notice == "due" && token == TOKEN && matches!(action, "close" | "settings")).then_some("closed")
}

async fn get(hub: &Hub, k: &str) -> String { hub.setting(k).await.unwrap_or_default() }
/// The first start with this version: an install that predates the setting is marked for the notice.
pub async fn boot(hub: &Arc<Hub>) {
    if crate::copy::on() { return }
    let existing = crate::settings::telemetry::existing_hub(hub).await;
    if let Some(m) = boot_mark(existing, &get(hub, NOTICE).await, &get(hub, KEY).await) {
        let _ = hub.commit_many(&[json!({"$":"SettingSet","key":NOTICE,"value":m,"at":now_ms() / 1000})]).await;
    }
}
pub async fn offer_now(hub: &Hub) -> Value { offer(&get(hub, NOTICE).await, &get(hub, KEY).await) }
/// After a change: every client hears the notice as it now stands.
pub async fn changed(hub: &Arc<Hub>) { let o = offer_now(hub).await; hub.push_all(&json!({"t":"gitpush.notice","notice":o})); }

/// `project.push {project}`: the project page's Push, the main checkout's branch to its upstream.
async fn push_project(hub: &Arc<Hub>, project: &str) -> Result<Value, String> {
    let root = { let m = hub.model().await; m.projects.get(project).filter(|p| !p.removed).map(|p| p.root.clone()) }.ok_or("unknown project")?;
    let (top, branch) = crate::files::kicad::canonical::main_branch(&root).await.ok_or("the project's folder is not a git checkout on a branch")?;
    let to = crate::files::kicad::canonical::push_upstream(&top, &branch).await;
    if to.is_empty() { return Err(format!("{branch} has no upstream to push to")) }
    if let Some(why) = to.strip_prefix("not pushed: ") { return Err(format!("not pushed: {why}")) }
    let left = crate::files::kicad::canonical::unpushed(&root).await.map_or(0, |(n, _)| n);
    Ok(json!({"pushed": to, "unpushed": left}))
}

pub struct AutoPushRpc;
impl RpcHandler for AutoPushRpc {
    fn methods(&self) -> &'static [&'static str] { &["gitpush.notice", "gitpush.notice.act", "project.push"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let g = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match m {
                "gitpush.notice" => vec![json!({"t":"reply","id":id,"ok":true,"result":{"notice":offer_now(hub).await}})],
                "gitpush.notice.act" => {
                    if let Some(v) = act(&get(hub, NOTICE).await, &g("token"), &g("action")) {
                        if let Err(e) = hub.commit_many(&[json!({"$":"SettingSet","key":NOTICE,"value":v,"at":now_ms() / 1000})]).await { return vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})] }
                        changed(hub).await;
                    }
                    vec![json!({"t":"reply","id":id,"ok":true,"result":{}})]
                }
                _ => match push_project(hub, &g("project")).await {
                    Ok(r) => vec![json!({"t":"reply","id":id,"ok":true,"result":r})],
                    Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})],
                },
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn off_unless_turned_on() { assert!(!on("") && !on("off") && on("on")); }
    #[test] fn an_upgraded_install_gets_off_and_the_notice_once_a_new_one_neither() {
        // existing, never chose: OFF (unset) and the notice is due
        assert_eq!(boot_mark(true, "", ""), Some("due")); assert!(!on(""));
        // existing that chose already (the person's own hubs set on): kept, no notice
        assert_eq!(boot_mark(true, "", "on"), Some("none")); assert!(on("on"));
        // a new install: off, no notice; and the mark is written once
        assert_eq!(boot_mark(false, "", ""), Some("none"));
        for n in ["due", "none", "closed"] { assert_eq!(boot_mark(true, n, ""), None, "{n}") }
        // offered while due and unchosen
        assert_eq!(offer("due", "")["token"], TOKEN); assert_eq!(offer("due", "")["text"], TEXT);
        assert!(offer("due", "off").is_null() && offer("due", "on").is_null() && offer("none", "").is_null() && offer("closed", "").is_null());
        // Close or Settings closes it for good; drawing it or a stale token does not
        assert_eq!(act("due", TOKEN, "close"), Some("closed")); assert_eq!(act("due", TOKEN, "settings"), Some("closed"));
        assert_eq!(act("due", TOKEN, "shown"), None); assert_eq!(act("due", "other", "close"), None); assert_eq!(act("closed", TOKEN, "close"), None);
        assert!(offer("closed", "").is_null(), "never again");
    }
    async fn start(home: &std::path::Path) -> Arc<Hub> {
        Hub::start(crate::hub::Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap()
    }
    async fn call(hub: &Arc<Hub>, m: &str, p: Value) -> Value { AutoPushRpc.call(hub, &Caller { gen: "1".into(), client: 1 }, &json!(5), m, &p).await.remove(0) }
    #[tokio::test(flavor = "multi_thread")] async fn the_migration_sets_off_and_shows_the_notice_once_across_restarts() {
        let home = crate::plat::tmp::dir("autopush-mig");
        let hub = start(&home).await;
        // a home in use before this version (setup done), whose setting was never chosen
        hub.commit_many(&[json!({"$":"SettingSet","key":"onboard.done","value":"1","at":1})]).await.unwrap();
        boot(&hub).await;
        assert!(!on(&get(&hub, KEY).await), "OFF after the update");
        let n = call(&hub, "gitpush.notice", json!({})).await; assert_eq!(n["result"]["notice"]["token"], TOKEN, "{n}");
        // drawn and the app restarted: still there until answered
        call(&hub, "gitpush.notice.act", json!({"token":TOKEN,"action":"shown"})).await;
        drop(hub); let hub = start(&home).await; boot(&hub).await;
        assert_eq!(call(&hub, "gitpush.notice", json!({})).await["result"]["notice"]["token"], TOKEN);
        // closed: never again, not after a restart either
        call(&hub, "gitpush.notice.act", json!({"token":TOKEN,"action":"close"})).await;
        assert!(call(&hub, "gitpush.notice", json!({})).await["result"]["notice"].is_null());
        drop(hub); let hub = start(&home).await; boot(&hub).await;
        assert!(call(&hub, "gitpush.notice", json!({})).await["result"]["notice"].is_null());
        assert_eq!(get(&hub, NOTICE).await, "closed");
        // a brand-new home: no notice
        let fresh = crate::plat::tmp::dir("autopush-new"); let h2 = start(&fresh).await; boot(&h2).await;
        assert_eq!(get(&h2, NOTICE).await, "none"); assert!(call(&h2, "gitpush.notice", json!({})).await["result"]["notice"].is_null());
    }
}
