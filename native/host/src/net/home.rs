//! The home as a client draws it (docs/PHONE-DATA.md, The home; docs/remote-architecture.md section 9): where the person's uberbot and bots live,
//! seen from this hub. `{kind, id, name, online, awaySince?, queuedCount, canMoveHere, moving?}`, made from what the hub already keeps: the home's
//! place (`home.at`, net/homemove.rs; "" when it is here), the always-on Boat's link (`alwayson.boat`), whether that machine answers (`Mirrors`),
//! what waits for it (the held sends, net/farsend.rs) and a move in progress (`home.moving` on the machine coming home, the move's job on the one sending).
//! Clients decide nothing from it; they draw it.
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::Arc;

/// What the home object is made of, read from the hub (`read`), so the rules (`home_of`) are pure and tested.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Facts {
    /// `home.at`: the machine that holds the home, `{link, name, url, at}`; null when this one does
    pub at: Value,
    /// the always-on Boat's link (`alwayson.boat` `link`), "" for none
    pub boat_link: String,
    /// this machine: its id (the mesh's mid) and name
    pub me_id: String, pub me_name: String,
    /// the home machine's id, when the mesh knows it
    pub home_id: String,
    /// since when the home's link has not answered (unix ms); None while it answers
    pub away_since: Option<u64>,
    /// what this hub holds for the home's link
    pub queued: usize,
    /// a move: the machine coming home's `home.moving` (`{to, state, at, why}`), else the sending side's job (`{state, step, note}`) while it runs or failed
    pub moving: Value,
}

/// The home object from its facts.
pub fn home_of(f: &Facts) -> Value {
    let link = f.at["link"].as_str().unwrap_or("");
    let local = link.is_empty();
    let kind = if local { "local" } else if !f.boat_link.is_empty() && link == f.boat_link { "boat" } else { "peer" };
    let online = local || f.away_since.is_none();
    let moving = moving_of(&f.moving);
    let mut o = json!({
        "kind": kind,
        "id": if local { f.me_id.clone() } else { f.home_id.clone() },
        "name": if local { f.me_name.clone() } else { f.at["name"].as_str().unwrap_or("").to_string() },
        "online": online,
        "queuedCount": if local { 0 } else { f.queued },
        "canMoveHere": !local && online && moving.is_null(),
    });
    if let (false, Some(t)) = (online, f.away_since) { o["awaySince"] = json!(t); }
    if !moving.is_null() { o["moving"] = moving; }
    o
}
/// A move as the client draws it, `{to, state, step?, note?}`, from either side's record; null when none runs and none just failed.
fn moving_of(m: &Value) -> Value {
    let state = m["state"].as_str().unwrap_or("");
    if state != "moving" && state != "failed" { return Value::Null }
    let mut o = json!({"to": m["to"].as_str().unwrap_or(""), "state": state});
    if let Some(s) = m["step"].as_str().filter(|s| !s.is_empty()) { o["step"] = json!(s); }
    if let Some(n) = m["note"].as_str().or(m["why"].as_str()).filter(|s| !s.is_empty()) { o["note"] = json!(n); }
    o
}

/// This machine's name, read once (`botnet::name` runs `hostname`; the home is read every few seconds per connection).
static NAME: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
/// The facts as this hub has them now.
pub async fn read(hub: &Arc<Hub>) -> Facts {
    let at: Value = serde_json::from_str(&hub.setting("home.at").await.unwrap_or_default()).unwrap_or(Value::Null);
    let link = at["link"].as_str().unwrap_or("").to_string();
    let boat: Value = serde_json::from_str(&hub.setting(crate::net::alwayson::SETTING).await.unwrap_or_default()).unwrap_or(Value::Null);
    let bots = crate::bots::api::of(hub);
    let me_id = bots.as_ref().map(|b| b.my_mid()).unwrap_or_default();
    let home_id = if link.is_empty() { String::new() } else { bots.as_ref().and_then(|b| b.mid_known(&link)).unwrap_or_default() };
    let queued = if link.is_empty() { 0 } else { crate::net::farsend::global().map_or(0, |o| o.items().iter().filter(|h| h.link == link).count()) };
    // the machine coming home says so in `home.moving`; the one sending has the job (net/homemove.rs `home.status`)
    let here: Value = serde_json::from_str(&hub.setting("home.moving").await.unwrap_or_default()).unwrap_or(Value::Null);
    let moving = if here.is_object() { here } else { crate::net::homemove::job() };
    Facts { away_since: if link.is_empty() { None } else { hub.mirrors().away_since(&link) }, at, boat_link: boat["link"].as_str().unwrap_or("").to_string(),
        me_id, me_name: NAME.get_or_init(crate::net::botnet::name).await.clone(), home_id, queued, moving }
}
/// The home object now (`hello.home`, `k:"home"`, `shown.home`).
pub async fn now(hub: &Arc<Hub>) -> Value { home_of(&read(hub).await) }

#[cfg(test)]
mod tests {
    use super::*;
    fn peer(away: Option<u64>) -> Facts { Facts { at: json!({"link":"L1","name":"desktop","url":"https://d","at":1}), me_id: "m-here".into(), me_name: "laptop".into(), home_id: "m-desk".into(), away_since: away, ..Default::default() } }
    #[test] fn this_machine_is_the_home() {
        let h = home_of(&Facts { me_id: "m1".into(), me_name: "laptop".into(), queued: 3, away_since: Some(5), ..Default::default() });
        assert_eq!(h, json!({"kind":"local","id":"m1","name":"laptop","online":true,"queuedCount":0,"canMoveHere":false}), "here: online, nothing queued, nothing to move");
    }
    #[test] fn another_machine_holds_it_answering_or_away() {
        let h = home_of(&Facts { queued: 2, ..peer(None) });
        assert_eq!(h, json!({"kind":"peer","id":"m-desk","name":"desktop","online":true,"queuedCount":2,"canMoveHere":true}));
        let h = home_of(&Facts { queued: 2, ..peer(Some(1_700_000_000_000)) });
        assert_eq!((h["online"].clone(), h["awaySince"].clone(), h["canMoveHere"].clone()), (json!(false), json!(1_700_000_000_000u64), json!(false)), "away: no move while it does not answer");
        let h = home_of(&Facts { boat_link: "L1".into(), ..peer(None) }); assert_eq!(h["kind"], "boat", "the always-on Boat's link");
    }
    #[test] fn a_move_running_or_failed_is_said_and_offers_no_other() {
        let h = home_of(&Facts { moving: json!({"to":"laptop","state":"moving","at":5}), ..peer(None) });
        assert_eq!((h["moving"].clone(), h["canMoveHere"].clone()), (json!({"to":"laptop","state":"moving"}), json!(false)));
        let h = home_of(&Facts { moving: json!({"to":"laptop","state":"failed","why":"the check failed; the home stayed"}), ..peer(None) });
        assert_eq!(h["moving"], json!({"to":"laptop","state":"failed","note":"the check failed; the home stayed"}));
        let h = home_of(&Facts { moving: json!({"state":"moving","step":"log","note":"94000 events"}), ..Default::default() });
        assert_eq!(h["moving"], json!({"to":"","state":"moving","step":"log","note":"94000 events"}), "the sending side's job");
        assert!(home_of(&Facts { moving: json!({"state":"done"}), ..peer(None) }).get("moving").is_none(), "a finished move is no move");
    }
}
