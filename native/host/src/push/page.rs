//! Settings → Notifications, the hub's half: the page's rows for every client and what a press does. The desktop draws the rows through the presenter
//! (`present/settings.rs` `notifications`, its presses sent here as `notify.page.act`); the phones ask `notify.page` and press with `notify.page.act {action, value, fields}`
//! (data/1 feature `notifysettings`). The hub owns the settings and the decision; a client only draws and presses. A phone is named on the page by a handle (a hash of its
//! push token), never the token, and the handle is all `notify-mute` takes.
use crate::{api::{Caller, Fut, RpcHandler}, bots::{rpc::Bots, world::Device}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// The handle the page knows a phone by: twelve hex digits of its token's hash.
pub fn handle(token: &str) -> String { Sha256::digest(token.as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect() }

/// The phones that hear a push, as `info["push.devices"]` (a JSON list of `{h, name, mute}`), sorted by handle so an unchanged set reads the same.
pub fn devices_info<'a>(devices: impl Iterator<Item = &'a Device>) -> String {
    let mut v: Vec<(String, Value)> = devices.filter(|d| d.kind == "alert" && d.platform == "ios").map(|d| {
        let h = handle(&d.token);
        let name = format!("iPhone{} {}", if d.env == "sandbox" { " (test build)" } else { "" }, &h[..4]);
        (h.clone(), json!({"h": h, "name": name, "mute": d.mute}))
    }).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    if v.is_empty() { return String::new() }
    Value::Array(v.into_iter().map(|x| x.1).collect()).to_string()
}

/// The settings a press of `setting` may write here: the alert kinds and urgent, each on or off.
const TOGGLES: [&str; 5] = ["notify.needs", "notify.settled", "notify.reminders", "notify.all", super::QUIET_URGENT];

/// The rows, for a client that asks.
pub async fn rows(hub: &Arc<Hub>, bots: &Bots) -> Vec<Value> {
    let devices = { let w = bots.world().await; devices_info(w.devices.values()) };
    let info = json!({"push.devices": devices});
    crate::statehost::policies().and_then(|p| p.with_state(|st| crate::present::settings::notifications(st, &json!({}), &info))).unwrap_or_default()
}

/// A press. `Ok` when it was kept; `Err` says why not, in the person's words.
pub async fn act(hub: &Arc<Hub>, bots: &Bots, action: &str, value: &str, fields: &Value) -> Result<(), String> {
    let now = now_ms() / 1000;
    let set = |k: &str, v: &str| json!({"$": "SettingSet", "key": k, "value": v, "at": now});
    let ev = match action {
        "setting" => {
            let (k, v) = value.split_once('=').ok_or("setting: key=value")?;
            if !TOGGLES.contains(&k) || !matches!(v, "on" | "off") { return Err("that setting is not one of Notifications".into()) }
            set(k, v)
        }
        "notify-quiet" => match value {
            "off" => set(super::QUIET, "off"),
            "on" => {
                let cur = hub.setting(super::QUIET).await.unwrap_or_default();
                set(super::QUIET, if super::window(&cur).is_some() { &cur } else { super::QUIET_DEFAULT })
            }
            "save" => {
                let f = |k: &str| fields[k].as_str().unwrap_or("");
                set(super::QUIET, &crate::present::settings::quiet_spec(f("qfrom"), f("qto")).ok_or("Quiet hours are HH:MM to HH:MM, 24 hours, and not the same time twice.")?)
            }
            _ => return Err("notify-quiet: on, off or save".into()),
        },
        "notify-mute" => {
            let (h, v) = value.split_once('=').ok_or("notify-mute: handle=on|off")?;
            let token = { let w = bots.world().await; w.devices.values().find(|d| handle(&d.token) == h).map(|d| d.token.clone()) }.ok_or("that phone is not registered any more")?;
            crate::net::devices::prefs(&json!({"token": token, "mute": v == "on"}), now).ok_or("bad device")?
        }
        _ => return Err("unknown action".into()),
    };
    hub.commit(&ev).await.map(|_| ()).map_err(|e| format!("{e:#}"))
}

/// `notify.page` → `{rows}`; `notify.page.act {action, value, fields?}` → `{}`.
pub struct NotifyPage(pub Arc<Bots>);
impl RpcHandler for NotifyPage {
    fn methods(&self) -> &'static [&'static str] { &["notify.page", "notify.page.act"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if method == "notify.page" { return vec![json!({"t":"reply","id":id,"ok":true,"rows":rows(hub, &self.0).await})] }
            let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match act(hub, &self.0, &s("action"), &s("value"), &p["fields"]).await {
                Ok(()) => vec![json!({"t":"reply","id":id,"ok":true})],
                Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})],
            }
        })
    }
}
