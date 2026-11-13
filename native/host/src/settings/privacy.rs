//! `privacy.ids`: the person's own identifiers, for a client's streamer mode to hide wherever they are drawn (the desktop client's
//! `streamer.rs`). Account labels and emails (Claude/Codex profiles and the CLIs' own logins), Google accounts, the machines this
//! hub knows (linked machines and tailnet peers, by name and host), and this hub's home folder, login and host name. No secret value:
//! those are masked by the hub itself before anything reaches a client (`mask.rs`).
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

type Names = Box<dyn Fn() -> Vec<String> + Send + Sync>;
static MACHINES: OnceLock<Names> = OnceLock::new();
/// The networking lane hands over how to name the machines it knows (net/mod.rs), once it is up.
pub fn set_machines(f: Names) { let _ = MACHINES.set(f); }

/// The host of a URL ("https://box.tail1.ts.net:443/x" -> "box.tail1.ts.net"); "" for none.
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let hp = rest.split(['/', '?', '#']).next().unwrap_or("");
    let hp = hp.rsplit_once('@').map_or(hp, |(_, h)| h);
    if hp.starts_with('[') { return hp.trim_start_matches('[').split(']').next().unwrap_or("").to_string() }
    hp.split(':').next().unwrap_or("").to_string()
}
/// Machine names and hosts from the routes' machines (`{"self":{name,url,domain},"peers":[{name,url}]}`).
pub fn machine_names(machines: &Value) -> Vec<String> {
    let mut out = vec![];
    for m in std::iter::once(&machines["self"]).chain(machines["peers"].as_array().into_iter().flatten()) {
        for k in ["name", "domain"] { if let Some(s) = m[k].as_str() { out.push(s.to_string()) } }
        if let Some(u) = m["url"].as_str() { out.push(host_of(u)) }
    }
    out
}
fn push(v: &mut Vec<String>, s: &str) {
    let s = s.trim();
    // an address is masked by the client's own patterns; loopback names nothing
    if s.is_empty() || matches!(s, "localhost" | "127.0.0.1" | "::1" | "0.0.0.0") || v.iter().any(|x| x.eq_ignore_ascii_case(s)) { return }
    v.push(s.to_string());
}
/// The answer, from what the hub knows: its accounts inventory (the setting's JSON), Google accounts' emails, machine names, and this
/// computer's home, login and host name.
pub fn ids(inventory: &Value, google: &[String], machines: &[String], home: &str, user: &str, host: &str) -> Value {
    let (mut emails, mut names, mut users, mut homes, mut labels) = (vec![], vec![], vec![], vec![], vec![]);
    for p in inventory["profiles"].as_array().into_iter().flatten() {
        if let Some(e) = p["identity"]["email"].as_str() { push(&mut emails, e) }
        if let Some(n) = p["identity"]["name"].as_str() { push(&mut names, n) }
        if let Some(l) = p["label"].as_str() { push(&mut labels, l) }
    }
    for (_, d) in inventory["defaultLogin"].as_object().into_iter().flatten() { if let Some(e) = d["email"].as_str() { push(&mut emails, e) } }
    for e in google { push(&mut emails, e) }
    for m in machines { push(&mut names, m) }
    push(&mut names, host); push(&mut users, user); push(&mut homes, home);
    json!({"emails": emails, "names": names, "users": users, "homes": homes, "labels": labels})
}

pub struct PrivacyIds;
impl RpcHandler for PrivacyIds {
    fn methods(&self) -> &'static [&'static str] { &["privacy.ids"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _m: &'a str, _p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let inventory = serde_json::from_str::<Value>(&hub.setting("accounts.inventory").await.unwrap_or_default()).unwrap_or(Value::Null);
            let google: Vec<String> = tokio::task::spawn_blocking(|| crate::google::store::accounts().into_iter().map(|a| a.email).collect()).await.unwrap_or_default();
            let machines = MACHINES.get().map(|f| f()).unwrap_or_default();
            let home = crate::plat::dirs::home().unwrap_or_default();
            let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
            let host = host_name();
            vec![json!({"t":"reply","id":id,"ok":true,"result":ids(&inventory, &google, &machines, &home, &user, &host)})]
        })
    }
}

fn host_name() -> String {
    #[cfg(unix)] {
        let mut b = [0u8; 256];
        // SAFETY: the buffer is ours and its length is passed.
        if unsafe { libc::gethostname(b.as_mut_ptr() as *mut libc::c_char, b.len()) } != 0 { return String::new() }
        let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        String::from_utf8_lossy(&b[..n]).split('.').next().unwrap_or("").to_string()
    }
    #[cfg(windows)] { std::env::var("COMPUTERNAME").unwrap_or_default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_ids_gather_every_account_machine_and_this_computer_once() {
        let inv = json!({"profiles":[{"id":"w","label":"Work","identity":{"email":"me@work.com","name":"Pat Example"}},{"id":"x","identity":{"email":"ME@work.com"}}],
            "defaultLogin":{"claude":{"state":"known","email":"cli@home.org"},"codex":{"state":"unknown"}}});
        let machines = machine_names(&json!({"self":{"name":"desk","url":"https://desk.tail9.ts.net","domain":"tail9.ts.net"},"peers":[{"name":"boat-hub","url":"http://100.64.1.2:3799"},{"name":"lap","url":"http://localhost:1"}]}));
        let v = ids(&inv, &["g@gmail.com".into()], &machines, "/home/pat", "pat", "desk");
        assert_eq!(v["emails"], json!(["me@work.com", "cli@home.org", "g@gmail.com"]));
        assert_eq!(v["names"], json!(["Pat Example", "desk", "tail9.ts.net", "desk.tail9.ts.net", "boat-hub", "100.64.1.2", "lap"]));
        assert_eq!((v["users"].clone(), v["homes"].clone(), v["labels"].clone()), (json!(["pat"]), json!(["/home/pat"]), json!(["Work"])));
    }
    #[test] fn a_hosts_name_is_read_from_a_url() {
        assert_eq!(host_of("https://box.tail1.ts.net:443/x"), "box.tail1.ts.net"); assert_eq!(host_of("http://u:p@h:1"), "h"); assert_eq!(host_of("http://[fd7a::1]:80/"), "fd7a::1"); assert_eq!(host_of("plain"), "plain");
    }
}
