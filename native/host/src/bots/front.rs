//! The ring's front door (docs/BOAT-HUB.md): one webhook address that never changes, whichever of the person's hubs owns the ring. The
//! always-on hub (boat-hub) is the front: the Pebble app posts there, always. While another linked machine owns the hook, the front passes each
//! delivery to it unchanged (headers and body; the owner checks the token as it always does) and answers with the owner's answer. Once the hook
//! is here (its secret files are), the front handles it itself. Settings: `ring.front` on every hub (the front's URL base, which the Ring page shows)
//! and, on the front, `ring.front.to.<hook>` (the link of the machine that owns that hook now; "" when this hub does).
use crate::{hub::Hub, net::botnet};
use serde_json::{json, Value};
use std::sync::Arc;

/// The headers a delivery carries on to its owner: the credentials and what the ring sends (`x-index-*`), nothing of the connection.
pub fn carried(headers: &[(String, String)]) -> String {
    let mut out = String::new();
    for (k, v) in headers {
        let l = k.to_ascii_lowercase();
        if v.contains('\n') || v.contains('\r') { continue }
        if l == "authorization" || l == "content-type" || l.starts_with("x-index-") || l.starts_with("x-backplane-") || l == "x-audio-size" { out.push_str(&format!("{k}: {v}\n")) }
    }
    out
}
/// The address of `link`'s hook `hid`: its own URL from the link table, "" when the link is not one of the owner's machines.
pub fn owner_url(machines: &[(String, String, String)], link: &str, hid: &str) -> String {
    machines.iter().find(|(id, _, _)| id == link).map(|(_, _, url)| format!("{}/hook/{hid}", url.trim_end_matches('/'))).unwrap_or_default()
}
/// The base the Ring page shows: `ring.front` when set, else this hub's own.
pub async fn base(hub: &Hub) -> String {
    let f = hub.setting("ring.front").await.unwrap_or_default();
    let f = f.trim().trim_end_matches('/');
    if f.starts_with("https://") || f.starts_with("http://") { f.to_string() } else { super::ring::base(hub).await }
}
/// A delivery for a hook another machine owns: (status line code, body), or None when this hub owns it (or no front was set up for it).
pub async fn pass(hub: &Arc<Hub>, hid: &str, headers: &[(String, String)], body: &[u8]) -> Option<(u16, String)> {
    let link = hub.setting(&format!("ring.front.to.{hid}")).await.unwrap_or_default();
    if link.is_empty() { return None }
    let url = owner_url(&hub.far_machines(), &link, hid);
    if url.is_empty() { return Some((502, "the machine that owns this webhook is not linked here".into())) }
    let home = hub.cfg.home.clone();
    botnet::mkdir(&home.join("secrets")); botnet::mkdir(&home.join("secrets/tmp"));
    let file = home.join("secrets/tmp").join(hex::encode(crate::net::pairing::urandom(8)));
    if !botnet::write_secret_bytes(&file, body) { return Some((503, "could not keep the delivery".into())) }
    let mut a = botnet::args(Some(&file), &url);
    if let Some(i) = a.iter().position(|x| x == "--max-filesize") { a[i + 1] = "33554432".into() }
    if let Some(i) = a.iter().position(|x| x == "-m") { a[i + 1] = "60".into() }
    let r = botnet::curl(a, carried(headers)).await;
    let _ = std::fs::remove_file(&file);
    // the owner away: 503, which the Pebble app retries (every minute, ten times)
    Some(if r.code == 0 { (503, "the machine that owns this webhook is away; try again".into()) } else { (r.code, r.body) })
}
/// The status line for a code the owner answered (the Pebble app reads only success or not).
pub fn status(code: u16) -> &'static str {
    match code { 200 => "200 OK", 201 => "201 Created", 202 => "202 Accepted", 204 => "204 No Content", 400 => "400 Bad Request", 401 => "401 Unauthorized", 404 => "404 Not Found",
        409 => "409 Conflict", 413 => "413 Payload Too Large", 502 => "502 Bad Gateway", 503 => "503 Service Unavailable", c if (200..300).contains(&c) => "200 OK", _ => "502 Bad Gateway" }
}
/// The far request `ring.front {hook, owner}` from the machine that owns `hook`: this hub fronts it for that machine (`on` false: no longer).
pub async fn serve(hub: &Arc<Hub>, link: &str, p: &Value) -> Value {
    let hid = p["hook"].as_str().unwrap_or("");
    if !botnet::safe(hid) || hid.is_empty() { return json!({"ok": false, "error": "no such hook"}) }
    let on = p["on"].as_bool().unwrap_or(true);
    let base = super::ring::base(hub).await;
    let _ = hub.commit(&json!({"$":"SettingSet","key":format!("ring.front.to.{hid}"),"value": if on { link } else { "" },"at": crate::hub::now_ms() / 1000})).await;
    let _ = hub.commit(&json!({"$":"SettingSet","key":"ring.front","value": base,"at": crate::hub::now_ms() / 1000})).await;
    json!({"ok": true, "url": format!("{base}/hook/{hid}"), "base": base})
}

#[cfg(test)]
mod tests {
    #[test] fn a_delivery_carries_its_credentials_and_the_rings_headers_only() {
        let h: Vec<(String, String)> = [("Authorization", "Bearer t"), ("Host", "x"), ("Content-Type", "multipart/form-data; boundary=b"), ("X-Index-Delivery", "d1"), ("Cookie", "c"), ("X-Bad", "a\nb"), ("x-index-evil", "a\r\nX: y")]
            .iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        assert_eq!(super::carried(&h), "Authorization: Bearer t\nContent-Type: multipart/form-data; boundary=b\nX-Index-Delivery: d1\n");
    }
    #[test] fn the_owner_is_reached_at_its_own_address() {
        let m = vec![("L1".to_string(), "desktop".to_string(), "http://desktop.ts.net:3797/".to_string())];
        assert_eq!(super::owner_url(&m, "L1", "h1"), "http://desktop.ts.net:3797/hook/h1");
        assert_eq!(super::owner_url(&m, "L2", "h1"), "");
    }
}
