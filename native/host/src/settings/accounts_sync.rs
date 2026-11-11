//! Sync accounts between the owner's machines (docs/multiple-provider-accounts.md, "Sync accounts"). A sync account is Claude Code's long-lived token
//! (`claude setup-token`, accounts_io.rs `SYNC`): it does not rotate, so every machine may use the same one at once, where a CLI login's refresh token would
//! log the other copies out. The machine that made one announces it, with its token, to each of the owner's linked machines over the signed far link
//! (net/rpc.rs: owner machines only): `accounts.sync {accounts: [...]}`, every sync account it made, each time that set changes, to a machine newly linked, and
//! again every ten minutes (a machine that restarted or lost it gets it back). The receiver adds or updates the same account id with the token in its own
//! vault and removes the ones that machine no longer names, so a removal or a renewal goes everywhere. A machine that removes one it was given asks the machine
//! that made it to remove it (`accounts.drop {id}`), which then tells the rest. No other account kind is ever sent: CLI logins stay on their machine.
use super::accounts_io::Accounts;
use crate::hub::{now_ms, Hub};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};

/// How often a machine that has it already is told again, unchanged (seconds).
pub const AGAIN: u64 = 600;
/// How soon a machine that did not take an announcement is tried again (seconds).
pub const RETRY: u64 = 30;

/// The announcement's body.
pub fn payload(items: &[Value]) -> Value { json!({"accounts": items}) }
/// What tells two announcements apart (the tokens are in it: kept in memory only).
pub fn digest(body: &Value) -> String { hex::encode(Sha256::digest(body.to_string().as_bytes())) }
/// Whether machine `link` is to be told now: `sent` is (digest, when) of the last one it took, `failed` when it last refused.
pub fn due(sent: Option<&(String, u64)>, failed: Option<u64>, d: &str, now: u64) -> bool {
    if failed.map_or(false, |f| now < f + RETRY) { return false }
    match sent { Some((x, at)) => x != d || now >= at + AGAIN, None => true }
}

/// A far request about sync accounts from one of the owner's machines (`link`, already checked by net/rpc.rs).
pub async fn serve(hub: &Arc<Hub>, link: &str, m: &str, p: &Value) -> Value {
    let Some(mgr) = hub.accounts.get().cloned() else { return json!({"t":"reply","ok":false,"error":"accounts are not running"}) };
    match m {
        "accounts.sync" => {
            let items = p["accounts"].as_array().cloned().unwrap_or_default();
            match mgr.sync_in(link, &items) {
                Ok(n) => { if n > 0 { eprintln!("accounts: {n} sync account change(s) from a linked machine") } json!({"t":"reply","ok":true,"result":{"changed":n}}) }
                Err(()) => json!({"t":"reply","ok":false,"error":"the sync accounts could not be kept here (is the vault locked?)"}),
            }
        }
        "accounts.drop" => {
            let id = p["id"].as_str().unwrap_or("");
            // only one this machine made: what another machine made is that machine's to remove
            if mgr.kind_of(id) != (super::accounts_io::SYNC.to_string(), String::new()) { return json!({"t":"reply","ok":true,"result":{"removed":false}}) }
            let r = mgr.request(&json!({"op":"remove","profile":id})).await;
            json!({"t":"reply","ok":r["error"].as_str().unwrap_or("").is_empty(),"result":{"removed":true},"error":r["error"]})
        }
        _ => json!({"t":"reply","ok":false,"error":"not an accounts request"}),
    }
}

/// Tells each linked machine (`machines`, the owner's own) the sync accounts `mgr` made, when due. `state`: per link, the last one taken and the last refusal.
pub async fn announce(hub: &Arc<Hub>, mgr: &Accounts, machines: &[String], sent: &mut HashMap<String, (String, u64)>, failed: &mut HashMap<String, u64>) {
    let Some(items) = mgr.sync_out() else { return };
    let body = payload(&items); let d = digest(&body); let now = now_ms() / 1000;
    sent.retain(|l, _| machines.contains(l)); failed.retain(|l, _| machines.contains(l));
    for link in machines {
        if !due(sent.get(link), failed.get(link).copied(), &d, now) { continue }
        let (code, text) = hub.far_raw(link, "accounts.sync", &body).await;
        let ok = code == 200 && serde_json::from_str::<Value>(&text).map_or(false, |v| v["ok"] == true);
        if ok { sent.insert(link.clone(), (d.clone(), now)); failed.remove(link); } else { failed.insert(link.clone(), now); }
    }
}

/// The watch that keeps the owner's machines told (accounts_hub.rs `start_cli` starts it). Never on a copy of a log: it reaches no other machine.
pub fn start(hub: &Arc<Hub>, mgr: Arc<Accounts>) {
    if crate::copy::on() { return }
    let h = Arc::downgrade(hub);
    tokio::spawn(async move {
        let (mut sent, mut failed) = (HashMap::new(), HashMap::new());
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let Some(hub) = h.upgrade() else { return };
            let machines: Vec<String> = hub.far_machines().into_iter().map(|(id, _, _)| id).collect();
            if machines.is_empty() { continue }
            announce(&hub, &mgr, &machines, &mut sent, &mut failed).await;
        }
    });
}

/// The home move (net/homemove.rs) carries the sync accounts this machine made, whole: their registry entries and tokens, the same announcement.
pub async fn push_to(hub: &Arc<Hub>, link: &str) -> Result<(), String> {
    let Some(mgr) = hub.accounts.get().cloned() else { return Ok(()) };
    let Some(items) = mgr.sync_out() else { return Err("the sync accounts' tokens cannot be read here (is the vault locked?)".into()) };
    if items.is_empty() { return Ok(()) }
    let (code, text) = hub.far_raw(link, "accounts.sync", &payload(&items)).await;
    if code == 200 && serde_json::from_str::<Value>(&text).map_or(false, |v| v["ok"] == true) { Ok(()) } else { Err(format!("the sync accounts did not go ({code})")) }
}

/// A sync account another machine made is removed here: that machine is asked to remove it (so it leaves every machine), best effort.
pub async fn dropped(hub: &Arc<Hub>, origin: &str, id: &str) { let _ = hub.far_raw(origin, "accounts.drop", &json!({"id": id})).await; }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_machine_is_told_when_the_set_changed_again_after_ten_minutes_and_not_while_it_refuses() {
        let d = "abc";
        assert!(due(None, None, d, 100), "a machine newly linked is told at once");
        assert!(!due(Some(&("abc".into(), 100)), None, d, 150), "it has this one");
        assert!(due(Some(&("old".into(), 100)), None, d, 150), "a change (a renewal, a removal) goes at once");
        assert!(due(Some(&("abc".into(), 100)), None, d, 100 + AGAIN), "and again after ten minutes");
        assert!(!due(None, Some(140), d, 150), "a machine that refused is left alone a while");
        assert!(due(None, Some(140), d, 140 + RETRY));
    }
    #[test] fn the_announcement_carries_the_accounts_and_its_digest_moves_with_a_token() {
        let a = payload(&[json!({"id":"x","token":"sk-ant-oat01-aaaa"})]); let b = payload(&[json!({"id":"x","token":"sk-ant-oat01-bbbb"})]);
        assert_eq!(a["accounts"][0]["id"], "x"); assert_ne!(digest(&a), digest(&b)); assert_eq!(digest(&a), digest(&a.clone()));
    }
}
