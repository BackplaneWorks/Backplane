//! `uber.sync`: the uberbot's ops between the owner's machines (docs/remote-architecture.md section 9; the ops and the lease are bots/uber.rs, their fold
//! state/uber.rs). Each machine numbers its own ops (`origin`, `n` from 1); a machine holds an origin's ops up to its count (`uber.vv.<uid>.<origin>`) and takes
//! only that origin's next one, so an exchange is: I say how far I hold every origin, you send what I lack and say how far you hold, and the next round
//! sends what you lack. The lease (who holds the conversation, by term) is an op like any other, so a move of the home reaches every machine the same way.
//!
//! Over the signed owner link (`hub.far_raw`, net/rpc.rs: the owner's own machines only): `uber.sync {uid, vv, ops, mid}` → `{uid, vv, ops, more, mid}`.
//! A machine whose uberbot is another one (or none) answers its own uid and takes nothing. Each machine runs a round with each of the owner's machines
//! every ROUND (and again at once while either side is behind); a round that gets through is also that machine's heartbeat.
use super::world::World;
use crate::hub::Hub;
use serde_json::{json, Map, Value};
use std::{collections::HashMap, sync::Arc};

/// The most ops one reply or request carries.
const BATCH: usize = 64;
// Leave room for the version vector and the envelope in the 256 KiB RPC body.
const PUSH_BYTES: usize = 192 << 10;
/// How often each linked machine is asked when nothing is behind.
const ROUND: std::time::Duration = std::time::Duration::from_secs(15);

fn get<'a>(w: &'a World, k: &str) -> &'a str {
    w.ub.settings.get(k).map(String::as_str).unwrap_or("")
}

/// How far this machine holds each origin of `uid`: `{origin: n}`.
pub fn vv_of(w: &World, uid: &str) -> Map<String, Value> {
    get(w, &format!("uber.origins.{uid}"))
        .split(' ')
        .filter(|o| !o.is_empty())
        .map(|o| (o.to_string(), json!(w.vv(uid, o))))
        .collect()
}
/// The ops of `uid` this machine holds beyond `theirs` (a peer's counts), oldest first per origin, at most `cap`; and whether more were left out.
pub fn ops_after(
    w: &World,
    uid: &str,
    theirs: &Map<String, Value>,
    cap: usize,
) -> (Vec<Value>, bool) {
    let mine = vv_of(w, uid);
    let mut out = vec![];
    let mut origins: Vec<&String> = mine.keys().collect();
    origins.sort();
    for o in origins {
        let (have, held) = (
            theirs.get(o).and_then(Value::as_u64).unwrap_or(0),
            mine[o].as_u64().unwrap_or(0),
        );
        if have >= held {
            continue;
        }
        for n in have + 1..=held {
            if out.len() >= cap {
                return (out, true);
            }
            // the fold keeps every op it took as a line (an op beaten later is kept as a "nop" line, so the numbering has no gap)
            let line: Value = serde_json::from_str(get(w, &format!("uber.op.{uid}.{o}.{n}")))
                .unwrap_or(Value::Null);
            if line.is_null() {
                break;
            } // not held after all: stop this origin here, the peer asks again
            out.push(json!({"$":"UberOp","uid":uid,"origin":o,"n":n,"stamp":line["s"],"op":line["op"],"at":line["at"]}));
        }
    }
    (out, false)
}
/// Of `ops` (a peer's), the ones this machine can take now: of `uid`, each origin's next ones in order from what it holds; repeats and gaps left out.
pub fn takeable(w: &World, uid: &str, ops: &[Value]) -> Vec<Value> {
    let mut held: HashMap<String, u64> = vv_of(w, uid)
        .into_iter()
        .map(|(o, n)| (o, n.as_u64().unwrap_or(0)))
        .collect();
    let mut v: Vec<&Value> = ops
        .iter()
        .filter(|e| {
            e["$"] == "UberOp"
                && e["uid"] == uid
                && e["origin"].as_str().is_some_and(super::rpc::safe_id)
                && e["origin"] != w.me()
                && e["op"].is_string()
                && e["stamp"].is_string()
        })
        .collect();
    v.sort_by(|a, b| {
        (a["origin"].as_str(), a["n"].as_u64()).cmp(&(b["origin"].as_str(), b["n"].as_u64()))
    });
    let mut out = vec![];
    for e in v {
        let o = e["origin"].as_str().unwrap_or("").to_string();
        let n = e["n"].as_u64().unwrap_or(0);
        let h = held.entry(o).or_insert(0);
        if h.checked_add(1) == Some(n) {
            *h = n;
            out.push(json!({"$":"UberOp","uid":uid,"origin":e["origin"],"n":n,"stamp":e["stamp"].as_str().unwrap_or(""),"op":e["op"],"at":e["at"].as_u64().unwrap_or(0)}));
        }
    }
    out
}
/// Hold the bots world guard through the commit, as local op writers do.
async fn take(
    hub: &Arc<Hub>,
    bots: &super::rpc::Bots,
    uid: &str,
    ops: &[Value],
) -> Result<usize, String> {
    let w = bots.world().await;
    if w.uid() != uid {
        return Ok(0);
    }
    let events = takeable(&w, uid, ops);
    if !events.is_empty() {
        hub.commit_many(&events)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    Ok(events.len())
}

/// The owning side of `uber.sync` (net/rpc.rs, after its owner-only check): take what the peer sent, answer what it lacks.
pub async fn serve(hub: &Arc<Hub>, p: &Value) -> Value {
    let Some(bots) = super::api::of(hub) else {
        return json!({"t":"reply","ok":false,"error":"bots are not running"});
    };
    let uid = bots.world().await.uid();
    if uid.is_empty() || p["uid"].as_str() != Some(uid.as_str()) {
        return json!({"t":"reply","ok":true,"result":{"uid":uid,"vv":{},"ops":[],"more":false,"mid":bots.my_mid()}});
    }
    if let Some(ops) = p["ops"].as_array() {
        if ops.len() > BATCH {
            return json!({"t":"reply","ok":false,"error":"too many UberOps"});
        }
        if let Err(error) = take(hub, &bots, &uid, ops).await {
            return json!({"t":"reply","ok":false,"error":error});
        }
    }
    let theirs = p["vv"].as_object().cloned().unwrap_or_default();
    let w = bots.world().await;
    if w.uid() != uid {
        return json!({"t":"reply","ok":false,"error":"uberbot changed during sync"});
    }
    let (out, more) = ops_after(&w, &uid, &theirs, BATCH);
    json!({"t":"reply","ok":true,"result":{"uid":uid,"vv":vv_of(&w, &uid),"ops":out,"more":more,"mid":w.me()}})
}

/// One machine's rounds with `link` until neither side is behind (at most a few at once). What the peer held last is `seen`.
pub(crate) async fn sync_with(
    hub: &Arc<Hub>,
    link: &str,
    peer: &mut Option<(String, Map<String, Value>)>,
) -> Result<usize, String> {
    let bots = super::api::of(hub).ok_or("bots are not running")?;
    let mut took = 0;
    for _ in 0..8 {
        let (uid, vv, mut ops, mid) = {
            let w = bots.world().await;
            let uid = w.uid();
            let ops = match peer.as_ref() {
                Some((peer_uid, counts)) if peer_uid == &uid => {
                    ops_after(&w, &uid, counts, BATCH).0
                }
                _ => Vec::new(),
            };
            (uid.clone(), vv_of(&w, &uid), ops, w.me())
        };
        if uid.is_empty() {
            return Ok(took);
        }
        // Oversized events still travel in the response to the other machine's pull.
        let mut bytes = 0;
        let count = ops
            .iter()
            .take_while(|op| {
                bytes += op.to_string().len() + 1;
                bytes <= PUSH_BYTES
            })
            .count();
        ops.truncate(count);
        let (code, body) = hub
            .far_raw(
                link,
                "uber.sync",
                &json!({"uid":uid,"vv":vv,"ops":ops,"mid":mid}),
            )
            .await;
        if crate::net::rpc::unavailable(code) || code == 501 {
            return Ok(took);
        }
        if code != 200 {
            return Err(format!("uber.sync to {link}: {code}"));
        }
        let reply: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        if reply["ok"] != true {
            return Err(reply["error"]
                .as_str()
                .unwrap_or("invalid uber.sync reply")
                .to_string());
        }
        let result = &reply["result"];
        if result["uid"].as_str() != Some(uid.as_str()) {
            *peer = None;
            return Ok(took);
        }
        let ops = result["ops"].as_array().ok_or("missing uber.sync ops")?;
        let received = take(hub, &bots, &uid, ops).await?;
        took += received;
        let counts = result["vv"].as_object().ok_or("missing uber.sync counts")?;
        let can_push = {
            let w = bots.world().await;
            let next = ops_after(&w, &uid, counts, 1).0;
            next.first()
                .is_some_and(|op| op.to_string().len() < PUSH_BYTES)
        };
        let was_known = peer.is_some();
        *peer = Some((uid, counts.clone()));
        if !(result["more"] == true && received > 0 || can_push && (!was_known || count > 0)) {
            break;
        }
    }
    Ok(took)
}

/// The rounds with each of the owner's machines (started with the bots). Never on a copy of a log: it reaches no other machine.
pub fn start(hub: &Arc<Hub>) {
    if crate::copy::on() {
        return;
    }
    let h = Arc::downgrade(hub);
    tokio::spawn(async move {
        let mut seen = HashMap::new();
        loop {
            tokio::time::sleep(ROUND).await;
            let Some(hub) = h.upgrade() else { return };
            let links: Vec<String> = hub
                .far_machines()
                .into_iter()
                .map(|(id, _, _)| id)
                .collect();
            seen.retain(|l, _| links.contains(l));
            let home: Value = serde_json::from_str(&hub.setting("home.at").await.unwrap_or_default())
                .unwrap_or(Value::Null);
            let home_link = home["link"].as_str().unwrap_or("");
            if !home_link.is_empty() {
                // A retired source has no uid to sync, but its clients still need the home's presence, version, and shared settings.
                let (hcode, hbody) = hub.far_raw_paired(home_link, "home.state", &json!({})).await;
                if hcode == 200 {
                    if let Ok(v) = serde_json::from_str::<Value>(&hbody) {
                        if let Some(res) = v.get("result") {
                            crate::net::peerver::record_json(&hub.cfg.home, home_link, res);
                        }
                    }
                }
                if let Err(e) = crate::net::settings_sync::sync_with(&hub, home_link).await {
                    eprintln!("settings: {e}");
                }
            }
            for link in links {
                if let Err(e) = sync_with(&hub, &link, seen.entry(link.clone()).or_default()).await
                {
                    eprintln!("bots: {e}");
                }
                if link != home_link {
                    if let Err(e) = crate::net::settings_sync::sync_with(&hub, &link).await {
                        eprintln!("settings: {e}");
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A world that holds these ops (folded as the state lane does), as machine `me` of uberbot `u`.
    fn world(me: &str, ops: &[(&str, u64, &str, &str)]) -> World {
        let mut w = World::default();
        w.ub.apply_line(
            &json!({"$":"SettingSet","key":"machine.id","value":me,"at":1}).to_string(),
        );
        w.ub.apply_line(&json!({"$":"SettingSet","key":"uber.uid","value":"u","at":1}).to_string());
        for (o, n, s, op) in ops {
            w.ub.apply_line(
                &json!({"$":"UberOp","uid":"u","origin":o,"n":n,"stamp":s,"op":op,"at":5})
                    .to_string(),
            );
        }
        w
    }
    fn op(o: &str, n: u64, s: &str, body: &str) -> Value {
        json!({"$":"UberOp","uid":"u","origin":o,"n":n,"stamp":s,"op":body,"at":5})
    }
    #[test]
    fn a_machine_sends_what_the_other_lacks_in_order_and_takes_only_what_is_next() {
        let a = world(
            "ma",
            &[
                (
                    "ma",
                    1,
                    "0000000001.000000.ma",
                    r#"{"k":"mem","key":"k1","kind":"fact","text":"one","tags":""}"#,
                ),
                (
                    "ma",
                    2,
                    "0000000002.000000.ma",
                    r#"{"k":"space","spec":"s"}"#,
                ),
                (
                    "mb",
                    1,
                    "0000000001.000001.mb",
                    r#"{"k":"page","html":"p"}"#,
                ),
            ],
        );
        assert_eq!(
            vv_of(&a, "u"),
            json!({"ma":2,"mb":1}).as_object().unwrap().clone()
        );
        let (ops, more) = ops_after(&a, "u", json!({"mb":1}).as_object().unwrap(), 10);
        assert!(!more);
        assert_eq!(
            ops.iter()
                .map(|e| (e["origin"].as_str().unwrap(), e["n"].as_u64().unwrap()))
                .collect::<Vec<_>>(),
            [("ma", 1), ("ma", 2)],
            "what B lacks: A's own two"
        );
        assert_eq!(ops[1]["op"], r#"{"k":"space","spec":"s"}"#);
        let (cut, more) = ops_after(&a, "u", &Map::new(), 2);
        assert!(more && cut.len() == 2, "a batch is bounded");
        let b = world(
            "mb",
            &[(
                "mb",
                1,
                "0000000001.000001.mb",
                r#"{"k":"page","html":"p"}"#,
            )],
        );
        let mixed = vec![
            op("ma", 2, "x", "{}"),
            op("ma", 1, "y", "{}"),
            op("mb", 1, "z", "{}"),
            op("ma", 4, "w", "{}"),
            json!({"$":"UberOp","uid":"other","origin":"ma","n":3,"stamp":"v","op":"{}"}),
        ];
        let t = takeable(&b, "u", &mixed);
        assert_eq!(
            t.iter()
                .map(|e| (e["origin"].as_str().unwrap(), e["n"].as_u64().unwrap()))
                .collect::<Vec<_>>(),
            [("ma", 1), ("ma", 2)],
            "in order, held ones and gaps and another uberbot's left out"
        );
    }
    #[test]
    fn two_machines_meet_in_the_middle_and_the_lease_goes_by_term() {
        let lease = |h: &str, t: u64| format!(r#"{{"k":"lease","holder":"{h}","term":{t}}}"#);
        let mut a = world("ma", &[("ma", 1, "0000000001.000000.ma", &lease("ma", 1))]);
        let mut b = world("mb", &[("mb", 1, "0000000002.000000.mb", &lease("mb", 2))]);
        // a round each way: A sends what B lacks, B what A lacks; both fold the other's
        let (to_b, _) = ops_after(&a, "u", &vv_of(&b, "u"), 10);
        let (to_a, _) = ops_after(&b, "u", &vv_of(&a, "u"), 10);
        for e in takeable(&b, "u", &to_b) {
            b.ub.apply_line(&e.to_string());
        }
        for e in takeable(&a, "u", &to_a) {
            a.ub.apply_line(&e.to_string());
        }
        assert_eq!(vv_of(&a, "u"), vv_of(&b, "u"), "both hold everything");
        assert_eq!(
            (a.lease(), b.lease()),
            (("mb".to_string(), 2), ("mb".to_string(), 2)),
            "the higher term holds the conversation on both"
        );
        assert!(
            ops_after(&a, "u", &vv_of(&b, "u"), 10).0.is_empty(),
            "nothing left to send"
        );
    }
}
