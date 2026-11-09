//! Google on the phones' account screens (data/1 `accounts.page`, `account.view`, `account.act`; docs/GOOGLE.md, docs/PHONE-DATA.md): a third provider beside
//! Claude and Codex, its rows, buttons and words decided here. Profile ids are `google:<id>`; a sign-in not yet done is `google:new`.
use super::{store, Google};
use serde_json::{json, Value};

pub const NEW: &str = "google:new";
pub fn is(profile: &str) -> bool { profile.starts_with("google:") }
fn id_of(profile: &str) -> &str { profile.strip_prefix("google:").unwrap_or("") }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }

impl Google {
    /// The provider block `accounts.page` lists after Claude's and Codex's.
    pub async fn phone_provider(&self) -> Value {
        let v = self.view().await;
        let accounts: Vec<Value> = v["accounts"].as_array().cloned().unwrap_or_default().iter().map(|a| json!({"id":format!("google:{}", s(a, "id")),"provider":"google","label":s(a, "email"),"email":s(a, "email"),
            "state":s(a, "state"),"line":s(a, "line"),"tone":s(a, "tone"),"limits":s(a, "scopes"),"hold":""})).collect();
        json!({"provider":"google","title":"Google","add":if v["client"]["ready"] == true { "Add Google account" } else { "" },"accounts":accounts,"note":if v["client"]["ready"] == true { String::new() } else { s(&v["client"], "line").to_string() }})
    }
    /// `account.view` of a Google profile (None when it is gone).
    pub async fn phone_view(&self, profile: &str) -> Option<Value> {
        let v = self.view().await;
        let newest = self.newest();
        if profile == NEW {
            let a = newest.filter(|a| a.account.is_empty())?;
            return Some(json!({"id":NEW,"provider":"google","title":"Google","label":"New account","email":a.email,"plan":"","state":if a.status == "done" { "connected" } else { "connecting" },
                "line":if a.status == "done" { "connected" } else if a.status == "failed" { "not signed in" } else { "signing in" },"tone":if a.status == "done" { "ok" } else if a.status == "failed" { "dim" } else { "warn" },
                "message":"","limits":[],"hold":"","signin":self.signin_json(&a),"actions":[],"remove":"","loopback":true}))
        }
        let id = id_of(profile);
        let a = v["accounts"].as_array()?.iter().find(|x| s(x, "id") == id)?.clone();
        let bots: Vec<Value> = v["bots"].as_array().cloned().unwrap_or_default();
        let mut actions = vec![json!({"label":if s(&a, "state") == "expired" { "Sign in again" } else { "Sign in again" },"action":"signin","danger":false})];
        for b in &bots {
            let on = b["accounts"].as_array().map_or(false, |xs| xs.iter().any(|x| s(x, "account") == id && x["on"] == true));
            actions.push(json!({"label":format!("{}: {}", s(b, "name"), if on { "allowed" } else { "not allowed" }),"action":format!("bot:{}", s(b, "id")),"danger":false}));
        }
        actions.push(json!({"label":"Remove","action":"remove","danger":true}));
        let signin = newest.filter(|x| x.account == id).map(|x| self.signin_json(&x));
        let bots_line = s(&a, "bots");
        Some(json!({"id":profile,"provider":"google","title":"Google","label":s(&a, "email"),"email":s(&a, "email"),"plan":"","state":s(&a, "state"),"line":s(&a, "line"),"tone":s(&a, "tone"),
            "message":format!("{}{}", if bots_line.is_empty() { "No bot may use it yet: allow one below.".to_string() } else { format!("Bots: {bots_line}") }, if s(&a, "why").is_empty() { String::new() } else { format!("\n{}", s(&a, "why")) }),
            "limits":[s(&a, "scopes")],"hold":"","signin":signin,"actions":actions,"remove":format!("Remove {}? Backplane revokes its access at Google and no bot can use it.", s(&a, "email")),"loopback":true}))
    }
    /// `account.act` for Google: add, signin, code, cancel, remove, bot:<id>. Answers the account as `account.view` does, or the page after a remove (None).
    pub async fn phone_act(&self, p: &Value) -> Result<Option<Value>, String> {
        let (profile, action) = (s(p, "profile"), s(p, "action"));
        let id = id_of(profile).to_string();
        match action {
            "add" => { self.begin(s(p, "redirect"), "").await?; return Ok(self.phone_view(NEW).await) }
            "signin" => { let acct = if profile == NEW || profile.is_empty() { String::new() } else { id.clone() }; self.begin(s(p, "redirect"), &acct).await?; }
            "code" => {
                let att = self.newest().filter(|a| if profile == NEW { a.account.is_empty() } else { a.account == id }).map(|a| a.id).unwrap_or_default();
                let email = self.finish(s(p, "code"), &att).await?;
                if profile == NEW { return Ok(self.phone_view(&format!("google:{}", store::id_of(&email))).await.or(self.phone_view(NEW).await)) }
            }
            "cancel" => { if let Some(a) = self.newest() { self.cancel(&a.id) } }
            "remove" => { self.remove(&id).await?; return Ok(None) }
            x if x.starts_with("bot:") => {
                let bot = &x[4..]; let on = self.grants(bot).await.contains(&id);
                self.grant(bot, &id, !on).await?;
            }
            _ => return Err("action: add, signin, code, cancel, remove or bot:<id>".into()),
        }
        Ok(self.phone_view(if profile.is_empty() { NEW } else { profile }).await)
    }
}
