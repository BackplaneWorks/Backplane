//! Google for bots (docs/GOOGLE.md; 0.14's server/google.bend and core/google.bend): several Google accounts signed in to the hub with OAuth for installed apps,
//! each bot granted the accounts it may use, and Gmail and Calendar tools for the bots' agents (tools.rs). Here: the sign-in attempts, access tokens and their
//! refresh, the `google` request the desktop sends, the `/oauth/google` route the browser comes back to, and the view the desktop and the phones draw.
pub mod cal;
pub mod gmail;
pub mod net;
pub mod oauth;
pub mod phone;
pub mod store;
pub mod tools;
#[cfg(test)]
mod tests;

use crate::{agents::Agents, api::{Caller, Fut, HttpHandler, HttpReply, RpcHandler}, auth::Req, bots::Bots, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock, Weak}};
use store::Acct;

/// How long a sign-in waits for the browser.
const WAIT_S: u64 = 900;

#[derive(Clone, Debug)]
pub struct Attempt { pub id: String, verifier: String, pub state: String, pub redirect: String, client: store::Client, pub at: u64, pub status: String, pub error: String, pub email: String, pub account: String }

pub struct Google {
    pub hub: Arc<Hub>, ag: Weak<Agents>, bots: Option<Arc<Bots>>, pub ends: net::Ends,
    attempts: Mutex<Vec<Attempt>>, access: Mutex<HashMap<String, (String, u64)>>,
    /// tests: a client in place of the environment's, the build's or the vault's
    pub client_override: Mutex<Option<store::Client>>,
    /// tests: the tailnet address in place of tailscale's
    pub here_override: Mutex<Option<String>>,
    /// bot → the account ids it may use, as last read: what a tool list is decided by (tools.rs `listed`)
    pub granted_cache: std::sync::RwLock<HashMap<String, Vec<String>>>,
}
static ME: OnceLock<Arc<Google>> = OnceLock::new();
pub fn get() -> Option<Arc<Google>> { ME.get().cloned() }
pub fn now_s() -> u64 { crate::hub::now_ms() / 1000 }

/// Start: the tools join the agents', the `google` request and the `/oauth/google` route are registered.
pub fn start(hub: &Arc<Hub>, ag: &Arc<Agents>, bots: Option<Arc<Bots>>) -> Arc<Google> {
    let g = Google::new(hub, ag, bots, net::Ends::from_env());
    let _ = ME.set(g.clone());
    g
}
impl Google {
    pub fn new(hub: &Arc<Hub>, ag: &Arc<Agents>, bots: Option<Arc<Bots>>, ends: net::Ends) -> Arc<Google> {
        let g = Arc::new(Google { hub: hub.clone(), ag: Arc::downgrade(ag), bots, ends, attempts: Mutex::new(vec![]), access: Mutex::new(HashMap::new()), client_override: Mutex::new(None), here_override: Mutex::new(None), granted_cache: Default::default() });
        ag.add_tools(Arc::new(tools::GoogleTools(Arc::downgrade(&g))));
        hub.register_rpc(Arc::new(GoogleRpc(Arc::downgrade(&g))));
        hub.register_http(Arc::new(Route(Arc::downgrade(&g))));
        { let g2 = g.clone(); tokio::spawn(async move { g2.reload_grants().await }); }
        g
    }
    /// The grants read again (after a grant, a removal, at start).
    pub async fn reload_grants(&self) {
        let mut m = HashMap::new(); let have: Vec<String> = store::accounts().into_iter().map(|a| a.id).collect();
        for (b, _) in self.bot_list().await { let g: Vec<String> = self.grants(&b).await.into_iter().filter(|x| have.contains(x)).collect(); if !g.is_empty() { m.insert(b, g); } }
        *self.granted_cache.write().unwrap() = m;
    }
    pub fn agents(&self) -> Option<Arc<Agents>> { self.ag.upgrade() }
    fn home(&self) -> std::path::PathBuf { self.hub.cfg.home.clone() }
    pub fn client(&self) -> store::Client { self.client_override.lock().unwrap().clone().unwrap_or_else(store::client) }

    // ---- bots and their grants ----

    /// Every live bot: (id, name).
    pub async fn bot_list(&self) -> Vec<(String, String)> {
        let Some(b) = &self.bots else { return vec![] };
        let w = b.world().await; let mut v: Vec<(String, String)> = w.bots.values().filter(|x| !x.deleted).map(|x| (x.id.clone(), x.name.clone())).collect(); v.sort_by(|a, c| a.1.to_lowercase().cmp(&c.1.to_lowercase())); v
    }
    /// The bot a thread works for (its own thread, a run's, a back office's), if any.
    pub async fn bot_of(&self, thread: &str) -> Option<(String, String)> { let b = self.bots.as_ref()?; let w = b.world().await; w.run_bot(thread).map(|x| (x.id.clone(), x.name.clone())) }
    pub async fn grants(&self, bot: &str) -> Vec<String> { store::grants_of(&self.hub.setting(&store::grant_key(bot)).await.unwrap_or_default()) }
    /// The accounts a bot may use, in the order they were signed in to (the first its default); a grant of an account removed since is not one.
    pub async fn bot_accounts(&self, bot: &str) -> Vec<Acct> { let g = self.grants(bot).await; store::accounts().into_iter().filter(|a| g.contains(&a.id)).collect() }
    pub async fn grant(&self, bot: &str, id: &str, on: bool) -> Result<(), String> {
        if !crate::bots::rpc::safe_id(bot) || !self.bot_list().await.iter().any(|(b, _)| b == bot) { return Err("no such bot".into()) }
        if on && store::get(id).is_none() { return Err("no such Google account".into()) }
        let t = store::granted(&self.hub.setting(&store::grant_key(bot)).await.unwrap_or_default(), id, on);
        self.hub.commit(&json!({"$":"SettingSet","key":store::grant_key(bot),"value":t,"at":now_s()})).await.map_err(|e| e.to_string())?;
        self.reload_grants().await; Ok(())
    }

    // ---- access tokens ----

    /// An access token for the account, good for a minute more: the one kept, else a refresh (and an expired sign-in when Google refuses the refresh token).
    pub async fn token(&self, a: &Acct) -> Result<String, String> {
        let now = now_s();
        if let Some((t, exp)) = self.access.lock().unwrap().get(&a.id).cloned() { if oauth::fresh(now, exp) { return Ok(t) } }
        if a.expired() { return Err(expired_text(&a.email)) }
        let c = self.client();
        if !c.ready() { return Err("Backplane has no Google OAuth client any more (google-client-id and google-client-secret in the vault): the person adds it again".into()) }
        let r = net::send(&self.home(), net::Req::form(self.ends.token(), oauth::refresh_body(&c.id, &c.secret, &a.refresh))).await;
        match oauth::tokens(&r.text(), now).filter(|_| r.ok()) {
            Some(t) => {
                self.access.lock().unwrap().insert(a.id.clone(), (t.access.clone(), t.expiry));
                // Google may hand out a new refresh token, and say again which scopes hold
                if (!t.refresh.is_empty() && t.refresh != a.refresh) || (!t.scopes.is_empty() && t.scopes != a.scopes) {
                    let mut b = a.clone(); if !t.refresh.is_empty() { b.refresh = t.refresh.clone() } if !t.scopes.is_empty() { b.scopes = t.scopes.clone() } let _ = store::put(&b);
                }
                Ok(t.access)
            }
            None if oauth::dead_grant(&r.code, &r.text()) => {
                let mut b = a.clone(); b.state = "expired".into(); b.why = oauth::api_error(&r.text()); let _ = store::put(&b);
                self.access.lock().unwrap().remove(&a.id);
                self.hub.push_all(&json!({"t":"google"}));
                Err(expired_text(&a.email))
            }
            None => Err(format!("{} (could not refresh Google access)", r.fail())),
        }
    }
    /// The access token was refused (401): forget it, so the next call refreshes.
    pub fn forget_access(&self, id: &str) { self.access.lock().unwrap().remove(id); }

    // ---- sign-in ----

    /// Where other machines reach this hub, for a state to name ("" off the tailnet).
    async fn here(&self) -> String {
        if let Some(h) = self.here_override.lock().unwrap().clone() { return h }
        let st = crate::net::tailnet::status_json().await; let dns = crate::net::tailnet::dns_name(&st);
        let port = self.hub.cfg.listen.rsplit(':').next().unwrap_or("").to_string();
        if dns.is_empty() || port.is_empty() { String::new() } else { format!("http://{dns}:{port}") }
    }
    fn own_redirect(&self) -> String { format!("http://127.0.0.1:{}/oauth/google", self.hub.cfg.listen.rsplit(':').next().unwrap_or("")) }
    /// A sign-in begins: a new account (account "") or one again. `redirect` a client's own loopback, else the hub's. The attempt, its URL in `url(..)`.
    pub async fn begin(&self, redirect: &str, account: &str) -> Result<Attempt, String> {
        let c = self.client();
        if !c.ready() { return Err(no_client_text()) }
        if crate::copy::on() { return Err("a hub on a copied log (BACKPLANE_COPY=1) signs in to nothing".into()) }
        let again = if account.is_empty() { None } else { Some(store::get(account).ok_or("no such Google account")?) };
        let redirect = if oauth::loopback(redirect) { redirect.to_string() } else { self.own_redirect() };
        let (verifier, n) = (oauth::nonce(48)?, oauth::nonce(12)?);
        let state = oauth::state(&n, &self.here().await);
        let a = Attempt { id: format!("g{}", oauth::nonce(6)?.replace(['-', '_'], "x")), verifier, state, redirect, client: c, at: now_s(), status: "waiting".into(), error: String::new(), email: again.as_ref().map_or(String::new(), |x| x.email.clone()), account: account.into() };
        let mut v = self.attempts.lock().unwrap(); let now = now_s();
        v.retain(|x| now.saturating_sub(x.at) < WAIT_S * 2); v.push(a.clone()); if v.len() > 16 { let k = v.len() - 16; v.drain(..k); }
        Ok(a)
    }
    pub fn url(&self, a: &Attempt) -> String { oauth::auth_url(&self.ends.auth, &a.client.id, &a.redirect, &oauth::challenge(&a.verifier), &a.state, &a.email) }
    pub fn newest(&self) -> Option<Attempt> { let now = now_s(); self.attempts.lock().unwrap().iter().rev().find(|a| now.saturating_sub(a.at) < WAIT_S * 2).cloned() }
    fn set(&self, id: &str, f: impl FnOnce(&mut Attempt)) { if let Some(a) = self.attempts.lock().unwrap().iter_mut().find(|a| a.id == id) { f(a) } }
    pub fn cancel(&self, id: &str) { self.set(id, |a| if a.status == "waiting" { a.status = "failed".into(); a.error = "cancelled".into() }) }
    /// The waiting sign-in this redirect's state names (any when the person pasted only the code: then the newest waiting one).
    fn waiting_for(&self, state: &str, bare: bool, attempt: &str) -> Option<Attempt> {
        let now = now_s(); let v = self.attempts.lock().unwrap();
        v.iter().rev().filter(|a| a.status == "waiting" && now.saturating_sub(a.at) < WAIT_S).find(|a| if bare { attempt.is_empty() || a.id == attempt } else { a.state == state && (attempt.is_empty() || a.id == attempt) }).cloned()
    }
    pub fn mine(&self, state: &str) -> bool { self.attempts.lock().unwrap().iter().any(|a| a.state == state) }
    /// A sign-in ends: `text` the address the browser came back to (or its query, or the bare code). Checks the state, trades the code with the verifier,
    /// keeps the account in the vault. The address signed in to, or why not.
    pub async fn finish(&self, text: &str, attempt: &str) -> Result<String, String> {
        let Some(cs) = oauth::code_of(text) else {
            let e = oauth::error_of(text);
            let why = if e == "access_denied" { "Google was not allowed access".to_string() } else if e.is_empty() { "no authorization code in that text".to_string() } else { format!("Google said {e}") };
            if let Some(a) = self.waiting_for(&oauth::param(oauth::query(text), "state"), !attempt.is_empty(), attempt) { self.set(&a.id, |x| { x.status = "failed".into(); x.error = why.clone() }) }
            return Err(why)
        };
        let Some(a) = self.waiting_for(&cs.state, cs.bare, attempt) else { return Err("this sign-in was not started here, or it waited too long: start again".into()) };
        self.set(&a.id, |x| x.status = "trading".into());
        let r = self.trade(&a, &cs.code).await;
        match &r { Ok(e) => self.set(&a.id, |x| { x.status = "done".into(); x.email = e.clone() }), Err(m) => self.set(&a.id, |x| { x.status = "failed".into(); x.error = m.clone() }) }
        self.hub.push_all(&json!({"t":"google"}));
        r
    }
    async fn trade(&self, a: &Attempt, code: &str) -> Result<String, String> {
        let now = now_s();
        let r = net::send(&self.home(), net::Req::form(self.ends.token(), oauth::code_body(&a.client.id, &a.client.secret, code, &a.verifier, &a.redirect))).await;
        let Some(t) = oauth::tokens(&r.text(), now).filter(|_| r.ok()) else { return Err(r.fail()) };
        let mut email = t.email.clone();
        if email.is_empty() { let p = net::send(&self.home(), net::Req::get(self.ends.api("/gmail/v1/users/me/profile"), &t.access)).await; email = p.json()["emailAddress"].as_str().unwrap_or("").to_string(); }
        if email.is_empty() { return Err("Google did not say which account signed in".into()) }
        let id = store::id_of(&email);
        if !a.account.is_empty() && a.account != id { return Err(format!("that was {email}, not the account being signed in to again; sign in to it as a new account instead")) }
        let old = store::get(&id);
        let refresh = if t.refresh.is_empty() { old.as_ref().map(|o| o.refresh.clone()).unwrap_or_default() } else { t.refresh.clone() };
        if refresh.is_empty() { return Err("Google sent no refresh token; remove Backplane's access at https://myaccount.google.com/permissions and sign in again".into()) }
        let acct = Acct { id: id.clone(), email: email.clone(), refresh, scopes: if t.scopes.is_empty() { oauth::SCOPES.iter().map(|s| s.to_string()).collect() } else { t.scopes.clone() },
            client: a.client.id.clone(), at: old.as_ref().map_or(now, |o| o.at), state: "ok".into(), why: String::new() };
        store::put(&acct)?;
        self.access.lock().unwrap().insert(id, (t.access, t.expiry));
        Ok(email)
    }
    /// Remove: the refresh token revoked at Google (best effort), the account forgotten, and every bot's grant of it dropped.
    pub async fn remove(&self, id: &str) -> Result<(), String> {
        let a = store::get(id).ok_or("no such Google account")?;
        let _ = net::send(&self.home(), net::Req::form(self.ends.revoke(), oauth::pair("token", &a.refresh))).await;
        store::del(id)?; self.access.lock().unwrap().remove(id);
        let mut ev = vec![];
        for (b, _) in self.bot_list().await { let t = self.hub.setting(&store::grant_key(&b)).await.unwrap_or_default(); if store::grants_of(&t).contains(&id.to_string()) { ev.push(json!({"$":"SettingSet","key":store::grant_key(&b),"value":store::granted(&t, id, false),"at":now_s()})) } }
        if !ev.is_empty() { let _ = self.hub.commit_many(&ev).await; }
        self.reload_grants().await;
        self.hub.push_all(&json!({"t":"google"}));
        Ok(())
    }

    // ---- what the clients draw ----

    pub async fn view(&self) -> Value {
        let (c, bots, accts) = (self.client(), self.bot_list().await, store::accounts());
        let mut grants: HashMap<String, Vec<String>> = HashMap::new(); for (b, _) in &bots { grants.insert(b.clone(), self.grants(b).await); }
        let names_for = |id: &str| bots.iter().filter(|(b, _)| grants.get(b).map_or(false, |g| g.iter().any(|x| x == id))).map(|(_, n)| n.clone()).collect::<Vec<_>>().join(", ");
        let accounts: Vec<Value> = accts.iter().map(|a| { let (line, tone) = state_line(a); json!({"id":a.id,"email":a.email,"scopes":oauth::scope_words(&a.scopes),"state":if a.expired() { "expired" } else { "connected" },"line":line,"tone":tone,"bots":names_for(&a.id),"why":a.why}) }).collect();
        let bots_v: Vec<Value> = bots.iter().map(|(b, n)| json!({"id":b,"name":n,"accounts":accts.iter().map(|a| json!({"account":a.id,"email":a.email,"on":grants.get(b).map_or(false, |g| g.contains(&a.id))})).collect::<Vec<_>>()})).collect();
        json!({"client":{"ready":c.ready(),"line":client_line(&c)},"add":"Add Google account","accounts":accounts,"bots":bots_v,"signin":self.newest().map(|a| self.signin_json(&a)),
            "note":"Bots use only the accounts allowed them here. Sending mail and changing the calendar always ask you first."})
    }
    pub fn signin_json(&self, a: &Attempt) -> Value {
        let busy = a.status == "waiting" || a.status == "trading";
        let text = match a.status.as_str() {
            "done" => format!("Signed in as {}.", a.email),
            "failed" => "The sign-in did not finish.".to_string(),
            "trading" => "Finishing the sign-in…".to_string(),
            _ => "Allow access on Google's page. If the page that opens after it does not load, paste its address here.".to_string(),
        };
        json!({"attempt":a.id,"url":if busy { self.url(a) } else { String::new() },"redirect":a.redirect,"state":match a.status.as_str() { "trading" => "waiting", s => s },"text":text,"error":a.error,"paste":a.status == "waiting","busy":busy,"done":a.status == "done","email":a.email})
    }
}
pub fn expired_text(email: &str) -> String { format!("The Google sign-in for {email} has expired: the person signs in again in Settings, Accounts, Google") }
pub fn no_client_text() -> String { format!("Backplane has no Google OAuth client yet: it needs {} and {} in the vault (a Google Cloud OAuth client of type Desktop app)", store::CLIENT_ID, store::CLIENT_SECRET) }
fn client_line(c: &store::Client) -> String {
    match (c.ready(), c.from) { (false, _) => no_client_text(), (true, "environment") => "Google client: from the hub's environment".into(), (true, "build") => "Google client: built in".into(), _ => format!("Google client: {} in the vault", store::CLIENT_ID) }
}
pub fn state_line(a: &Acct) -> (&'static str, &'static str) { if a.expired() { ("sign-in expired", "bad") } else { ("connected", "ok") } }

// ---- the `google` request ----

struct GoogleRpc(Weak<Google>);
impl RpcHandler for GoogleRpc {
    fn methods(&self) -> &'static [&'static str] { &["google"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let e = |m: &str| vec![json!({"t":"reply","id":id,"ok":false,"error":m})];
            if !crate::settings::accounts_hub::is_granted(who) { return e("Google accounts are managed from Backplane's own windows and phones") }
            let Some(g) = self.0.upgrade() else { return e("Google is not running") };
            match g.op(p).await { Ok(v) => vec![json!({"t":"reply","id":id,"ok":true,"result":v})], Err(m) => e(&m) }
        })
    }
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
impl Google {
    /// One `google` op, then the view (the error is said in it as `error` when the op failed but the view stands).
    pub async fn op(&self, p: &Value) -> Result<Value, String> {
        let r: Result<(), String> = match s(p, "op") {
            "view" | "" => Ok(()),
            "signin" => self.begin(s(p, "redirect"), s(p, "account")).await.map(|_| ()),
            "code" => self.finish(s(p, "code"), s(p, "attempt")).await.map(|_| ()),
            "cancel" => { self.cancel(s(p, "attempt")); Ok(()) }
            "remove" => self.remove(s(p, "account")).await,
            "grant" => self.grant(s(p, "bot"), s(p, "account"), p.get("on").and_then(Value::as_bool).unwrap_or(false)).await,
            _ => Err("op: view, signin, code, cancel, remove or grant".into()),
        };
        let mut v = self.view().await;
        if let Err(m) = r { if ["signin", "remove", "grant"].contains(&s(p, "op")) { return Err(m) } v["error"] = json!(m) }
        Ok(v)
    }
}

// ---- the route the browser comes back to ----

struct Route(Weak<Google>);
fn page(status: &'static str, msg: &str) -> HttpReply {
    let esc = msg.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    HttpReply { status, ctype: "text/html; charset=utf-8".into(), extra: "Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n".into(),
        body: format!("<!doctype html><meta charset=\"utf-8\"><title>Backplane</title><body style=\"font:14px system-ui,sans-serif;margin:40px\"><p>{esc}</p></body>").into_bytes() }
}
/// Auth.oauth.navigation: Google's redirect is a cross-site top-level navigation with no pairing credential. Only a plain document GET, with no Origin, to a
/// loopback Host, may reach the state and PKCE check this way (a hop forwarded to this hub comes from the owner's own device over the tailnet).
pub fn navigation(req: &Req) -> bool {
    let h = |k: &str| req.header(k).to_lowercase();
    let doc = (h("sec-fetch-mode") == "navigate" && h("sec-fetch-dest") == "document") || (h("sec-fetch-mode").is_empty() && h("sec-fetch-dest").is_empty() && h("sec-fetch-site").is_empty());
    let host = h("host"); let hn = host.rsplit_once(':').map_or(host.as_str(), |(a, _)| a);
    let local_host = hn == "127.0.0.1" || hn == "localhost";
    let loop_peer = req.peer.map_or(true, |p| p.ip().is_loopback());
    let hop = oauth::param(oauth::query(&req.target), "hop") == "1";
    req.method == "GET" && h("origin").is_empty() && doc && ((local_host && loop_peer) || (hop && req.trusted && hn.ends_with(".ts.net")))
}
impl HttpHandler for Route {
    fn routes(&self) -> &'static [&'static str] { &["/oauth/google"] }
    fn own_auth(&self) -> bool { true }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            if !navigation(req) { return HttpReply::text("403 Forbidden", "not a sign-in") }
            let Some(g) = self.0.upgrade() else { return page("503 Service Unavailable", "Backplane's Google sign-in is not running.") };
            let q = oauth::query(&req.target).to_string();
            if let Some(to) = oauth::hop(&q, g.mine(&oauth::param(&q, "state"))) {
                return HttpReply { status: "302 Found", ctype: "text/plain".into(), extra: format!("Location: {to}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n"), body: b"to the hub that started this sign-in".to_vec() }
            }
            match g.finish(&req.target, "").await {
                Ok(email) => page("200 OK", &format!("Signed in to Google as {email}. You can close this tab.")),
                Err(m) => page("400 Bad Request", &format!("The Google sign-in did not finish: {m}")),
            }
        })
    }
}
