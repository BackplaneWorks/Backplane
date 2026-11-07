//! Settings, Machines: the rows every client draws (the desktop through the presenter, the phones through `machines.page`) and
//! what a press does (`machines.page.act`). Rows are `{label, note, buttons:[{label, action, value, on}], fields?:[{name, hint,
//! secret, value}], tone?}`, voice.page's shape. Linked machines, requests to answer, machines found (suggestions: never trusted),
//! pair a machine (address + code), make a code. The data is `Peering::page_data`; the rows are a pure function of it.
use super::{peering::Peering, sign};
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::Arc;

fn btn(label: &str, action: &str, value: &str, on: bool) -> Value { json!({"label": label, "action": action, "value": value, "on": on}) }
fn row(label: &str, note: &str, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "buttons": buttons}) }
fn field(name: &str, hint: &str, value: &str) -> Value { json!({"name": name, "hint": hint, "secret": false, "value": value}) }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }

/// The rows. `draft`: the text typed in a field so far ("" for a client that keeps its own).
pub fn rows(d: &Value, draft: &dyn Fn(&str) -> String) -> Vec<Value> {
    let mut r = vec![];
    let linked = d["linked"].as_array().cloned().unwrap_or_default();
    if linked.is_empty() { r.push(row("Linked machines", "None yet. Make a code here and enter it on the other machine, or enter its code below.", vec![])) }
    for l in &linked {
        let (note, mut b) = match s(l, "state") {
            "paired" => (format!("{} · Paired", s(l, "url")), vec![]),
            "no" => (format!("{} · Not yours", s(l, "url")), vec![btn("Mine", "machine-mine", s(l, "id"), false)]),
            _ => (format!("{} · Unpaired: nobody approved this link. Confirm it is yours, or pair again with a code.", s(l, "url")), vec![btn("Mine", "machine-mine", s(l, "id"), false)]),
        };
        b.push(btn("Remove", "machine-remove", s(l, "id"), false));
        let mut x = row(s(l, "name"), &note, b);
        if s(l, "state") == "unpaired" { x["tone"] = json!("warn") }
        r.push(x);
    }
    for q in d["requests"].as_array().into_iter().flatten() {
        r.push(row(&format!("{} wants to pair", s(q, "name")), &format!("{} · check code {} (the same must show on that machine) · {}s left", s(q, "url"), s(q, "check"), q["left"].as_u64().unwrap_or(0)),
            vec![btn("Approve", "machine-approve", s(q, "id"), true), btn("Deny", "machine-deny", s(q, "id"), false)]));
    }
    for o in d["outgoing"].as_array().into_iter().flatten() {
        let st = s(o, "status");
        let note = if st == "waiting" { format!("Approve it on that machine. Check code {}.", s(o, "check")) } else { st.to_string() };
        let mut x = row(&format!("Pairing with {}", s(o, "address")), &note, vec![]);
        if st.starts_with("refused") { x["tone"] = json!("bad") }
        r.push(x);
    }
    for f in d["found"].as_array().into_iter().flatten() {
        let mut x = row(s(f, "name"), &format!("{} · found on the network{}", s(f, "url"), if f["same_user"] == false { ", another account's" } else { "" }), vec![btn("Pair", "machine-pair", s(f, "url"), true)]);
        x["fields"] = json!([field("fcode", "Code from that machine", &draft("fcode"))]);
        r.push(x);
    }
    let mut p = row("Pair a machine", "Enter the other machine's address and the code it made (a bp1: invite works in the address field). It asks you there to approve.", vec![btn("Pair", "machine-pair", "", true)]);
    p["fields"] = json!([field("maddr", "Address (http://box.example:3787)", &draft("maddr")), field("mcode", "Code", &draft("mcode"))]);
    r.push(p);
    let c = &d["code"];
    r.push(if c.is_object() {
        row("Machine code", &format!("{} · enter it with {} on the other machine · works once, {}s left. You approve its request here.", s(c, "code"), s(c, "address"), c["left"].as_u64().unwrap_or(0)), vec![btn("Make another", "machine-code", "", false)])
    } else { row("Make a code", "A code lets one machine ask to pair with this one. Nothing links until you approve its request here.", vec![btn("Make a code", "machine-code", "", false)]) });
    r
}

/// What a press does. `fields`: {maddr, mcode, fcode} as typed. Ok is the reply's extra (usually `{}`), Err the reason to show.
pub async fn act(p: &Arc<Peering>, hub: &Arc<Hub>, action: &str, value: &str, fields: &Value) -> Result<Value, String> {
    let f = |k: &str| fields[k].as_str().unwrap_or("").trim().to_string();
    match action {
        "machine-code" => p.make_code().await,
        "machine-approve" => p.approve(hub, value).await.map(|_| json!({})),
        "machine-deny" => p.deny(hub, value).await.map(|_| json!({})),
        "machine-remove" | "machine-mine" => {
            if !p.table.with(|t| t.peer(value).map_or(false, |r| !r.revoked)) { return Err("no such link".into()) }
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let ev = if action == "machine-remove" { json!({"$":"PeerSet","id":value,"name":"","url":"","revoked":true,"at":now}) } else { json!({"$":"SettingSet","key":format!("peer.own.{value}"),"value":"owner","at":now}) };
            hub.commit(&ev).await.map_err(|e| format!("{e:#}"))?;
            if action == "machine-remove" { p.unpeered(value) }
            Ok(json!({}))
        }
        "machine-pair" => {
            let (addr, code) = if value.is_empty() { (f("maddr"), f("mcode")) } else { (value.to_string(), f("fcode")) };
            if addr.starts_with("bp1:") || code.starts_with("bp1:") {
                let inv = if addr.starts_with("bp1:") { addr } else { code };
                let i = sign::invite_parse(&inv).ok_or("not an invite")?;
                return p.join(hub, &i, "").await.map(|(_, n)| json!({"linked": n}));
            }
            p.pair_with(hub, &addr, &code).await.map(|(id, check)| json!({"attempt": id, "check": check}))
        }
        _ => Err("unknown action".into()),
    }
}
