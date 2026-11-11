//! Another machine of the person's as the always-on one (a home server, a desktop that stays on): paired by address and code, or picked among
//! the linked machines. It is called always-on only when it says it is a server (`bots.hello`'s `server`): its hub runs under systemd (the guard
//! as a service: `INVOCATION_ID` is set) or its "stays on" switch is on (`machine.stays_on`). Otherwise the rows say it runs while that machine is on.
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::{atomic::{AtomicBool, Ordering::Relaxed}, Arc};

pub const STAYS_ON: &str = "machine.stays_on";
pub const PICKED: &str = "alwayson.other";
static SWITCH: AtomicBool = AtomicBool::new(false);

/// This hub runs as a system service (systemd sets INVOCATION_ID for a unit's processes; the guard passes its environment on).
pub fn under_systemd() -> bool { std::env::var("INVOCATION_ID").map_or(false, |v| !v.trim().is_empty()) }
/// What `bots.hello` says of this machine: it stays on.
pub fn server_now() -> bool { under_systemd() || SWITCH.load(Relaxed) }
pub async fn load(hub: &Arc<Hub>) { SWITCH.store(hub.setting(STAYS_ON).await.as_deref() == Some("on"), Relaxed); }

/// The install line for another computer (the newest release's installer).
pub const INSTALL: &str = "curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh | sh";

async fn set(hub: &Arc<Hub>, k: &str, v: &str) -> Result<(), String> {
    hub.commit(&json!({"$":"SettingSet","key":k,"value":v,"at":crate::hub::now_ms() / 1000})).await.map(|_| ()).map_err(|e| format!("{e:#}"))
}

pub async fn act(hub: &Arc<Hub>, action: &str, value: &str, fields: &Value) -> Result<Value, String> {
    let f = |k: &str| fields[k].as_str().unwrap_or("").trim().to_string();
    match action {
        // this machine's own switch: it stays on (shown to the others as a server)
        "alwayson-other-stayson" => { let on = value == "on"; set(hub, STAYS_ON, if on { "on" } else { "" }).await?; SWITCH.store(on, Relaxed); Ok(json!({})) }
        // pair by address and code (the other machine made the code; it asks the person there to approve)
        "alwayson-other-pair" => {
            let p = crate::net::start::peering().ok_or("this hub's machine links are not running")?;
            let (addr, code) = (f("addr"), f("code"));
            if addr.is_empty() || code.is_empty() { return Err("enter that machine's address and its code".into()) }
            let (attempt, check) = p.pair_with(hub, &addr, &code).await?;
            Ok(json!({"attempt": attempt, "check": check}))
        }
        // a linked machine chosen: asked whether it stays on, and kept
        "alwayson-other-pick" => {
            let p = crate::net::start::peering().ok_or("this hub's machine links are not running")?;
            let Some(l) = p.table.with(|t| t.live().into_iter().find(|x| x.id == value)) else { return Err("no such linked machine".into()) };
            let hello = match crate::bots::api::of(hub) { Some(b) => b.hello_of(&l.id).await, None => None };
            let server = hello.as_ref().map_or(false, |h| h["server"] == true);
            let rec = json!({"link": l.id, "name": l.name, "url": l.url, "server": server, "asked": hello.is_some(), "at": crate::hub::now_ms() / 1000});
            set(hub, PICKED, &rec.to_string()).await?;
            set(hub, super::super::alwayson::CHOICE_KEY, "machine").await.ok();
            Ok(json!({"other": rec}))
        }
        "alwayson-other-clear" => { set(hub, PICKED, "").await?; Ok(json!({})) }
        _ => Err("unknown action".into()),
    }
}
