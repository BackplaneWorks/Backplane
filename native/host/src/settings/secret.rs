//! The secret box: names, what may be stored,
//! handles, the requests `secret.set/del/list/dismiss`, the three tools `secret_request/secret_list/secret_run`, and the `<secrets>` prompt
//! block. `secret.set`'s value goes from the parsed request straight to the vault: it is in no event, no reply and no log line.
use super::{mask, vault};
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

/// Sec.name: lower case letters, digits, `-`, `_` and `.`, anything else made `-`, at most 64 ("Tailscale Bub" is "tailscale-bub").
pub fn name(raw: &str) -> String {
    raw.trim().chars().take(64).map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' { c } else if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { '-' }).collect()
}

/// Sec.bad.value: why a value cannot be stored ("" when it can): 8 to 4096 printable ASCII characters with no space, quote or backslash.
pub fn bad_value(v: &str) -> String {
    let n = v.chars().count();
    if n < 8 { "A secret is at least 8 characters".into() }
    else if n > 4096 { "A secret is at most 4096 characters".into() }
    else if v.chars().all(|c| ('!'..='~').contains(&c) && c != '"' && c != '\\') { String::new() }
    else { "Only letters, digits and punctuation: no spaces, quotes or backslashes".into() }
}
pub fn bad(name_raw: &str, value: &str) -> String { if name(name_raw).is_empty() { "Give the secret a name".into() } else { bad_value(value.trim()) } }

/// Sec.handle: what an agent gets for a secret, its name and whether it is set, never more (law sec_handle_shape).
pub fn handle(name: &str, set: bool) -> Value { json!({"secret": name, "set": set}) }
pub fn ask_key(name: &str) -> String { format!("secret.ask.{name}") }
pub fn ask_body(purpose: &str, thread: &str, by: &str, at: u64) -> String { json!({"purpose": purpose.chars().take(400).collect::<String>(), "thread": thread, "by": by, "at": at}).to_string() }

/// Sec.prompt: the `<secrets>` block every agent gets: the names set, how to use one, and to ask with secret_request instead of in chat.
pub fn prompt() -> String {
    let ns = vault::names();
    let set = if ns.is_empty() { "None is set yet. ".to_string() } else { format!("Set now: {}. ", ns.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")) };
    format!("<secrets>Backplane keeps secrets (API keys, auth keys, tokens, passwords) in a vault you never see into. {set}\
Use one only through the secret_run tool: `env {{VAR: name}}` puts it in that command's environment, `files {{VAR: name}}` writes it to a private temporary file whose path is in VAR (e.g. `tailscale up --auth-key=file:$TS`, `gh auth login --with-token < $GH`); its output comes back with any secret masked. \
Never ask the person to paste a key, token or password in chat, and never put one in a message, a file or a command line: call secret_request {{name, purpose}} and end your turn; a message tells you when it is set.</secrets>")
}

/// A vault write the vault answered "ok" (durable) or "committed <why>" (saved and in use, the folder flush failed).
pub fn saved_is(r: &str) -> bool { r == "ok" || r.starts_with("committed ") }

fn err(id: &Value, m: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":m}) }
fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }

/// SecRpc.list.text: the vault's status and metadata (JSON texts), never a value.
fn list_reply(id: &Value) -> Value {
    let st = vault::status();
    if st["flush"].as_str().map_or(false, |f| f.starts_with("unflushed")) { eprintln!("vault: the last save is not known to be durable: {}", st["flush"].as_str().unwrap_or("")); }
    json!({"t":"reply","id":id,"ok":true,"secrets":vault::list().to_string(),"vault":st.to_string()})
}

/// Secret.saved: the request closed, and the thread that asked for it and the one it was saved from (/secret in a chat) told its handle.
async fn saved(hub: &Arc<Hub>, agents: &Option<Arc<crate::agents::Agents>>, name: &str, from: &str) {
    let key = ask_key(name);
    let asked = hub.setting(&key).await.unwrap_or_default();
    let asker = serde_json::from_str::<Value>(&asked).ok().map(|v| sv(&v, "thread").to_string()).unwrap_or_default();
    let at = now_ms() / 1000;
    if !asked.is_empty() { let _ = hub.commit(&json!({"$":"SettingSet","key":key,"value":"","at":at})).await; }
    let from = if from == asker { "" } else { from };
    for to in [asker.as_str(), from] {
        if to.is_empty() { continue }
        let live = { let m = hub.model().await; m.threads.get(to).map_or(false, |t| !t.deleted) };
        if !live { continue }
        let text = format!("{}{}", handle(name, true), crate::present::thread::SAVED_NOTE);
        if let Some(a) = agents { let _ = a.deliver_as("user", to, &format!("t:{to}:sec-{name}-{at}"), &text, None, vec![]).await; }
    }
}

pub struct SecretRpc { pub agents: Option<Arc<crate::agents::Agents>> }
impl RpcHandler for SecretRpc {
    fn methods(&self) -> &'static [&'static str] { &["secret.set", "secret.del", "secret.list", "secret.dismiss"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let nm = name(sv(p, "name"));
            match method {
                "secret.set" => {
                    let b = bad(sv(p, "name"), sv(p, "value"));
                    if !b.is_empty() { return vec![err(id, &b)] }
                    let (value, purpose) = (sv(p, "value").trim().to_string(), sv(p, "purpose").chars().take(400).collect::<String>());
                    let n2 = nm.clone();
                    let r = tokio::task::spawn_blocking(move || vault::put(&n2, &value, &purpose, "person")).await.unwrap_or_else(|_| "fail the vault did not answer".into());
                    if !saved_is(&r) { return vec![err(id, r.strip_prefix("fail ").unwrap_or(&r))] }
                    saved(hub, &self.agents, &nm, sv(p, "thread")).await;
                    vec![list_reply(id)]
                }
                "secret.del" => {
                    let n2 = nm.clone();
                    let r = tokio::task::spawn_blocking(move || vault::del(&n2)).await.unwrap_or_else(|_| "fail the vault did not answer".into());
                    if saved_is(&r) { vec![list_reply(id)] } else { vec![err(id, r.strip_prefix("fail ").unwrap_or(&r))] }
                }
                "secret.list" => vec![list_reply(id)],
                "secret.dismiss" => {
                    if hub.setting(&ask_key(&nm)).await.unwrap_or_default().is_empty() { return vec![json!({"t":"reply","id":id,"ok":true})] }
                    let _ = hub.commit(&json!({"$":"SettingSet","key":ask_key(&nm),"value":"","at":now_ms() / 1000})).await;
                    vec![json!({"t":"reply","id":id,"ok":true})]
                }
                _ => vec![err(id, "unknown secret request")],
            }
        })
    }
}

// ---- the tools (agents/tools wires these into the MCP list) ----

/// secret_list: handles with purpose and last use.
pub fn tool_list() -> (bool, Value) {
    let l: Vec<Value> = vault::list().as_array().cloned().unwrap_or_default().iter().map(|m| json!({"secret": sv(m, "name"), "set": true, "purpose": sv(m, "purpose"), "usedAt": sv(m, "usedAt")})).collect();
    (true, Value::Array(l))
}

/// secret_request {name, purpose}: the handle when it is set; else the request the clients show, logged as the setting `secret.ask.<name>`.
pub async fn tool_request(hub: &Arc<Hub>, thread: &str, a: &Value) -> (bool, Value) {
    let nm = name(sv(a, "name"));
    if nm.is_empty() { return (false, json!({"error":"name is required"})) }
    if vault::names().contains(&nm) { return (true, handle(&nm, true)) }
    let at = now_ms() / 1000;
    let _ = hub.commit(&json!({"$":"SettingSet","key":ask_key(&nm),"value":ask_body(sv(a, "purpose"), thread, thread, at),"at":at})).await;
    (true, json!({"secret":nm,"set":false,"requested":true,"note":"The person sees a masked box for it in this chat. End your turn; a message tells you when it is set."}))
}

/// secret_run {command, env?, files?, cwd?, timeout?}: `{code, timedOut, stdout, stderr}` or `{error}`, masked. Refused on a copy.
pub async fn tool_run(thread: &str, cwd: &str, a: &Value) -> (bool, Value) {
    if crate::copy::on() { return (false, json!({"error":"a hub on a copied log (BACKPLANE_COPY=1) runs no secret commands"})) }
    let spec = json!({"command": sv(a, "command"), "env": a.get("env").cloned().unwrap_or(json!({})), "files": a.get("files").cloned().unwrap_or(json!({})),
        "cwd": if sv(a, "cwd").is_empty() { cwd } else { sv(a, "cwd") }, "timeout": a.get("timeout").cloned().unwrap_or(json!(120)), "thread": thread});
    let mut r = vault::run(spec).await;
    mask::json(&mut r);
    (r.get("error").is_none(), r)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn sec_name_is_the_name() { assert_eq!(name("Tailscale Bub"), "tailscale-bub"); assert_eq!(name("  A.b_c  "), "a.b_c"); assert_eq!(name(&"x".repeat(80)).len(), 64); }
    #[test] fn values_follow_sec_bad() {
        assert_eq!(bad_value("short"), "A secret is at least 8 characters"); assert_eq!(bad_value(&"a".repeat(4097)), "A secret is at most 4096 characters");
        assert!(bad_value("has space1").starts_with("Only letters")); assert!(bad_value("quo\"te1234").starts_with("Only letters")); assert!(bad_value("back\\slash1").starts_with("Only letters"));
        assert_eq!(bad_value("tskey-auth-ABC123"), ""); assert_eq!(bad("  ", "abcdefgh"), "Give the secret a name");
    }
    #[test] fn sec_handle_shape() { assert_eq!(handle("a", true).to_string(), r#"{"secret":"a","set":true}"#); assert_eq!(handle("b", false).as_object().unwrap().len(), 2); }
    #[test] fn the_prompt_names_the_secrets_and_never_a_value() {
        let p = prompt(); assert!(p.starts_with("<secrets>Backplane keeps secrets") && p.ends_with("</secrets>") && p.contains("`env {VAR: name}`") && p.contains("secret_request {name, purpose}"));
    }
    #[test] fn a_request_body_names_the_secret_never_a_value() { let b: Value = serde_json::from_str(&ask_body(&"p".repeat(500), "t1", "t1", 7)).unwrap(); assert_eq!(b["purpose"].as_str().unwrap().len(), 400); assert_eq!(b["at"], 7); }
}
