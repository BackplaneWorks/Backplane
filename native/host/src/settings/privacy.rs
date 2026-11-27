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
/// The last directory name of a project root path (`/a/b/my-proj/` -> `my-proj`); empty for a drive root or blank.
pub fn folder_basename(root: &str) -> String {
    let trimmed = root.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed.ends_with(':') { return String::new() }
    trimmed.rsplit(['/', '\\']).next().unwrap_or("").trim().to_string()
}
fn push_project(v: &mut Vec<(String, String)>, title: &str, folder: &str) {
    let (t, f) = (title.trim(), folder.trim());
    if t.is_empty() && f.is_empty() { return }
    if !t.is_empty() {
        if let Some(existing) = v.iter_mut().find(|x| x.0.eq_ignore_ascii_case(t)) {
            if existing.1.is_empty() && !f.is_empty() { existing.1 = f.to_string(); }
            return;
        }
    }
    if v.iter().any(|x| x.0.eq_ignore_ascii_case(t) && x.1.eq_ignore_ascii_case(f)) { return }
    v.push((t.to_string(), f.to_string()));
}
/// The answer, from what the hub knows: its accounts inventory (the setting's JSON), allowance/CLI emails, Google accounts' emails,
/// machine names, projects and groups, and this computer's home, login and host name.
pub fn ids(inventory: &Value, extra_emails: &[String], google: &[String], machines: &[String], projects: &[(String, String)], groups: &[String], home: &str, user: &str, host: &str) -> Value {
    let (mut emails, mut names, mut users, mut homes, mut labels) = (vec![], vec![], vec![], vec![], vec![]);
    for p in inventory["profiles"].as_array().into_iter().flatten() {
        if let Some(e) = p["identity"]["email"].as_str() { push(&mut emails, e) }
        if let Some(e) = p["allowance"]["email"].as_str() { push(&mut emails, e) }
        if let Some(n) = p["identity"]["name"].as_str() { push(&mut names, n) }
        if let Some(l) = p["label"].as_str() { push(&mut labels, l) }
    }
    for (_, d) in inventory["defaultLogin"].as_object().into_iter().flatten() {
        if let Some(e) = d["email"].as_str() { push(&mut emails, e) }
        if let Some(n) = d["name"].as_str() { push(&mut names, n) }
    }
    for e in extra_emails { push(&mut emails, e) }
    for e in google { push(&mut emails, e) }
    for m in machines { push(&mut names, m) }
    push(&mut names, host); push(&mut users, user); push(&mut homes, home);
    let mut projs = vec![];
    for (t, f) in projects { push_project(&mut projs, t, f); }
    projs.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()).then(a.1.to_ascii_lowercase().cmp(&b.1.to_ascii_lowercase())));
    let mut grps: Vec<String> = vec![];
    for g in groups {
        let g = g.trim();
        if !g.is_empty() && !grps.iter().any(|x| x.eq_ignore_ascii_case(g)) { grps.push(g.to_string()); }
    }
    grps.sort_by_key(|g| g.to_ascii_lowercase());
    let projs_json: Vec<Value> = projs.into_iter().map(|(title, folder)| json!({"title": title, "folder": folder})).collect();
    json!({"emails": emails, "names": names, "users": users, "homes": homes, "labels": labels, "projects": projs_json, "groups": grps})
}

pub struct PrivacyIds;
impl RpcHandler for PrivacyIds {
    fn methods(&self) -> &'static [&'static str] { &["privacy.ids"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _m: &'a str, _p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let inventory = serde_json::from_str::<Value>(&hub.setting("accounts.inventory").await.unwrap_or_default()).unwrap_or(Value::Null);
            let google: Vec<String> = tokio::task::spawn_blocking(|| crate::google::store::accounts().into_iter().map(|a| a.email).collect()).await.unwrap_or_default();
            let machines = MACHINES.get().map(|f| f()).unwrap_or_default();
            let (mut projects, allowance_keys, group_keys) = {
                let m = hub.model().await;
                let projects: Vec<(String, String)> = m.projects.iter()
                    .filter(|(pid, p)| !p.removed && pid.as_str() != crate::sidebar::SCRATCH && !crate::sidebar::is_chats(pid))
                    .map(|(_, p)| (p.title.clone(), folder_basename(&p.root)))
                    .collect();
                let allowance_keys: Vec<String> = m.settings_with_prefix("allowance.").map(|(k, _)| k.to_string()).collect();
                let group_keys: Vec<String> = m.settings_with_prefix("sidebar.group.name.")
                    .chain(m.settings_with_prefix("sidebar.group.pick."))
                    .map(|(k, _)| k.to_string())
                    .collect();
                (projects, allowance_keys, group_keys)
            };
            let mut extra_emails = vec![];
            for k in allowance_keys {
                if let Some(raw) = hub.setting(&k).await {
                    if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                        if let Some(e) = v["email"].as_str() { extra_emails.push(e.to_string()); }
                    }
                }
            }
            if let Some(raw) = hub.setting("models").await {
                if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                    for arr_key in ["accts", "signed"] {
                        for item in v["agy"][arr_key].as_array().into_iter().flatten() {
                            if let Some(s) = item.as_str().filter(|s| s.contains('@')) {
                                extra_emails.push(s.to_string());
                            }
                        }
                    }
                }
            }
            let mut groups = vec![];
            for k in group_keys {
                if let Some(g) = hub.setting(&k).await { groups.push(g); }
            }
            hub.mirrors().shown(|far| {
                for (_, st) in far {
                    for (pid, p) in &st.projects {
                        if !p.removed && pid.as_str() != crate::sidebar::SCRATCH && !crate::sidebar::is_chats(pid) {
                            projects.push((p.title.clone(), folder_basename(&p.root)));
                        }
                    }
                    for (k, v) in &st.settings {
                        if k.starts_with("sidebar.group.name.") || k.starts_with("sidebar.group.pick.") {
                            groups.push(v.clone());
                        }
                    }
                }
            });
            let home = crate::plat::dirs::home().unwrap_or_default();
            let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
            let host = host_name();
            vec![json!({"t":"reply","id":id,"ok":true,"result":ids(&inventory, &extra_emails, &google, &machines, &projects, &groups, &home, &user, &host)})]
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
        let inv = json!({"profiles":[{"id":"w","label":"Work","identity":{"email":"me@work.com","name":"Pat Example"},"allowance":{"email":"allow@work.com"}},{"id":"x","identity":{"email":"ME@work.com"}}],
            "defaultLogin":{"claude":{"state":"known","email":"cli@home.org","name":"Cli Person"},"codex":{"state":"unknown"}}});
        let machines = machine_names(&json!({"self":{"name":"desk","url":"https://desk.tail9.ts.net","domain":"tail9.ts.net"},"peers":[{"name":"boat-hub","url":"http://100.64.1.2:3799"},{"name":"lap","url":"http://localhost:1"}]}));
        let projects = vec![("Orion Health".into(), "orion-health-fw".into()), ("Acme_DK".into(), "".into()), ("acme_dk".into(), "acme_dk".into())];
        let groups = vec!["ClientAlpha".into(), "clientalpha".into(), "AlphaGroup".into()];
        let v = ids(&inv, &["extra@example.org".into(), "ME@work.com".into()], &["g@gmail.com".into()], &machines, &projects, &groups, "/home/pat", "pat", "desk");
        assert_eq!(v["emails"], json!(["me@work.com", "allow@work.com", "cli@home.org", "extra@example.org", "g@gmail.com"]));
        assert_eq!(v["names"], json!(["Pat Example", "Cli Person", "desk", "tail9.ts.net", "desk.tail9.ts.net", "boat-hub", "100.64.1.2", "lap"]));
        assert_eq!((v["users"].clone(), v["homes"].clone(), v["labels"].clone()), (json!(["pat"]), json!(["/home/pat"]), json!(["Work"])));
        assert_eq!(v["projects"], json!([{"title":"Acme_DK","folder":"acme_dk"},{"title":"Orion Health","folder":"orion-health-fw"}]));
        assert_eq!(v["groups"], json!(["AlphaGroup", "ClientAlpha"]));
    }
    #[test] fn folder_basename_extracts_last_component() {
        assert_eq!(folder_basename("/home/user/clients/orion-health-fw/"), "orion-health-fw");
        assert_eq!(folder_basename(r"C:\Users\Alex\Acme_DK\"), "Acme_DK");
        assert_eq!(folder_basename("/"), "");
        assert_eq!(folder_basename("C:\\"), "");
    }
    #[test] fn a_hosts_name_is_read_from_a_url() {
        assert_eq!(host_of("https://box.tail1.ts.net:443/x"), "box.tail1.ts.net"); assert_eq!(host_of("http://u:p@h:1"), "h"); assert_eq!(host_of("http://[fd7a::1]:80/"), "fd7a::1"); assert_eq!(host_of("plain"), "plain");
    }
}
