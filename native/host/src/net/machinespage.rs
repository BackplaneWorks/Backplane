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
            // one of the owner's machines: its agents can be signed in from here (present/signin_there.rs)
            "paired" => (format!("{} · Paired", s(l, "url")), vec![crate::present::signin_there::open_button(s(l, "id"))]),
            "no" => (format!("{} · Not yours", s(l, "url")), vec![btn("Mine", "machine-mine", s(l, "id"), false)]),
            _ => (format!("{} · Unpaired: nobody approved this link. Confirm it is yours, or pair again with a code.", s(l, "url")), vec![btn("Mine", "machine-mine", s(l, "id"), false)]),
        };
        b.push(btn("Remove", "machine-remove", s(l, "id"), false));
        let mut x = row(s(l, "name"), &note, b);
        if s(l, "state") == "unpaired" { x["tone"] = json!("warn") }
        r.push(x);
    }
    // machines the linked ones are linked to: one press asks for a code through them, the person approves on that machine
    for v in d["via"].as_array().into_iter().flatten() {
        if linked.iter().any(|l| super::machines::hostport(s(l, "url")) == super::machines::hostport(s(v, "url"))) { continue }
        r.push(row(if s(v, "name").is_empty() { s(v, "url") } else { s(v, "name") }, &format!("{} · linked to {}, not to this machine yet. Pairing asks you to approve on that machine.", s(v, "url"), s(v, "via_name")),
            vec![btn("Pair", "machine-introduce", &format!("{}\u{1f}{}", s(v, "via"), s(v, "url")), true)]));
    }
    if !linked.is_empty() {
        let mut x = row("Rename a machine", "How this machine calls another: one word or a few, never the same as another's. Agents and the composer (~name) use it.", vec![btn("Rename", "machine-rename", "", true)]);
        x["fields"] = json!([field("rfrom", "Machine now called", &draft("rfrom")), field("rname", "New name", &draft("rname"))]);
        r.push(x);
    }
    for q in d["requests"].as_array().into_iter().flatten() {
        r.push(row(&format!("{} wants to pair", s(q, "name")), &format!("{} · check code {} (it must match the one on that machine) · {}s left", s(q, "url"), s(q, "check"), q["left"].as_u64().unwrap_or(0)),
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
    let mut p = row("Pair a machine", "Enter the other machine's address and the code it made. It then asks you there to approve.", vec![btn("Pair", "machine-pair", "", true)]);
    p["fields"] = json!([field("maddr", "Address (https://box.example.ts.net or http://box:3787)", &draft("maddr")), field("mcode", "Code", &draft("mcode"))]);
    r.push(p);
    let c = &d["code"];
    r.push(if c.is_object() {
        {
            let addrs: Vec<&str> = c["addresses"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            let at = if addrs.is_empty() { s(c, "address").to_string() } else { addrs.join(" or ") };
            row("Machine code", &format!("{} · enter it with {} on the other machine · works once, {}s left. You approve its request here.", s(c, "code"), at, c["left"].as_u64().unwrap_or(0)), vec![btn("Make another", "machine-code", "", false)])
        }
    } else { row("Make a code", "A code lets one machine ask to pair with this one. Nothing links until you approve its request here.", vec![btn("Make a code", "machine-code", "", false)]) });
    r
}

/// What a press does. `fields`: {maddr, mcode, fcode} as typed. Ok is the reply's extra (usually `{}`), Err the reason to show.
pub async fn act(p: &Arc<Peering>, hub: &Arc<Hub>, action: &str, value: &str, fields: &Value) -> Result<Value, String> {
    let f = |k: &str| fields[k].as_str().unwrap_or("").trim().to_string();
    match action {
        "machine-code" => p.make_code().await,
        a if a.starts_with("machine-signin") => crate::settings::accounts_there::act(hub, a, value, fields).await,
        // a masked box's Send on the sign-in panel: the typed text is in `fields` of this request only
        "secret-save" if crate::settings::accounts_there::is_save(value) => crate::settings::accounts_there::save(hub, value, fields).await,
        "machine-home" => super::homemove::start(hub, value).await,
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
        "machine-introduce" => {
            let (via, url) = value.split_once('\u{1f}').ok_or("which machine?")?;
            p.introduce(hub, via, url).await.map(|(id, check)| json!({"attempt": id, "check": check}))
        }
        "machine-rename" => {
            let (from, to) = (f("rfrom"), f("rname"));
            let rows: Vec<(String, String)> = p.table.with(|t| t.live().into_iter().map(|x| (x.id, x.name)).collect());
            let Some(id) = rows.iter().find(|(i, n)| *i == from || crate::bots::mesh::word(n) == crate::bots::mesh::word(&from)).map(|r| r.0.clone()) else { return Err(format!("no linked machine is called {from}")) };
            let to = crate::net::botnet::clean(&to);
            crate::bots::mesh::name_free(&rows, &id, &to, &crate::net::botnet::name().await)?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            hub.commit(&json!({"$":"PeerSet","id":id,"name":to,"url":"","revoked":false,"at":now})).await.map_err(|e| format!("{e:#}"))?;
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
