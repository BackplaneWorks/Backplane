//! Google's MCP tools (0.14's G.Google.tool_defs, Google.call, Gm.run, Gt.*): listed only in a thread that works for a bot (its own thread, a live run's) and only
//! while that bot is allowed at least one account; every call checks the bot and the account again. `gmail_send` and the calendar's create, update and delete
//! open an approval with exactly what will run (the whole email, the change in words) and run it only once the person allows it; Allow for this session skips
//! the next ones of that tool in that thread. In a mailroom run the charter has decided before the call reaches here (`Perm.run.google`): nobody is asked.
use super::{cal, gmail, net, oauth, store::Acct, Google};
use crate::{agents::mcp::ToolSet, api::Fut};
use serde_json::{json, Value};
use std::sync::Weak;

pub const NAMES: [&str; 9] = ["gmail_search", "gmail_read", "gmail_send", "gmail_draft", "calendar_events", "calendar_create", "calendar_update", "calendar_delete", "google_accounts"];

type Res = (bool, Value);
fn err(m: &str) -> Res { (false, Value::String(if m.starts_with("error: ") { m.to_string() } else { format!("error: {m}") })) }
fn ok(t: String) -> Res { (true, Value::String(t)) }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn n(v: &Value, k: &str, d: u64, hi: u64) -> u64 { let x = v.get(k).and_then(|x| x.as_u64().or_else(|| x.as_f64().map(|f| f as u64))).unwrap_or(d); x.clamp(1, hi) }

fn prop(ty: &str, d: &str) -> Value { json!({"type":ty,"description":d}) }
fn arr(d: &str) -> Value { json!({"type":"array","items":{"type":"string"},"description":d}) }
fn def(name: &str, d: &str, props: Value, req: &[&str]) -> Value { json!({"name":name,"description":d,"inputSchema":{"type":"object","properties":props,"required":req}}) }
fn with_account(mut p: Value) -> Value { p["account"] = prop("string", "Which Google account, by address (google_accounts lists the ones this bot may use); default the first."); p }
fn reminders() -> Value { json!({"type":"array","maxItems":5,"items":{"type":"object","properties":{"method":prop("string","popup or email."),"minutes":prop("number","Minutes before the start, 0 to 40320.")}},"description":"Alerts, at most 5: {method: popup|email, minutes} (a bare number is a popup that many minutes ahead). [] means no alerts; omitted keeps the calendar's defaults."}) }
fn compose_props() -> Value {
    with_account(json!({"to":prop("string","Recipients, comma-separated (a reply defaults to the original's Reply-To, else its sender)."),"subject":prop("string","Subject (a reply defaults to Re: the original's)."),
        "body":prop("string","The text; HTML when html is true. The account's signature is added after it."),"html":prop("boolean","The body is HTML (sent with a plain-text version too); default plain text."),
        "cc":prop("string","Cc recipients, comma-separated (optional)."),"bcc":prop("string","Bcc recipients, comma-separated (optional)."),
        "reply_to":prop("string","A message id (from gmail_search) to reply to, in its conversation (optional)."),"thread":prop("string","A conversation id to reply to; answers its last message (optional)."),
        "attachments":arr("Absolute paths of files to attach, each and all together at most 18 MB (optional)."),
        "draft":prop("string","A draft id: gmail_draft replaces it; gmail_send sends it as it is, with no other content arguments (optional).")}))
}
/// Google.tool_defs
pub fn defs() -> Vec<Value> {
    vec![
        def("gmail_search", "Search the user's Gmail (Gmail search syntax, e.g. 'from:alice is:unread newer_than:7d'). Lists id, date, from, subject and snippet; read one with gmail_read.", with_account(json!({"query":prop("string","Gmail search query; empty lists the newest mail."),"max":prop("number","How many (default 10, at most 25).")})), &[]),
        def("gmail_read", "Read one Gmail message by id: headers, attachment names and its text (at most 20 KB).", with_account(json!({"id":prop("string","A message id from gmail_search.")})), &["id"]),
        def("gmail_send", "Send an email from the user's Gmail, as a new message or a reply in its conversation, with attachments if asked. The user is asked to approve every send first and sees the whole email. With draft, sends a draft made by gmail_draft as it is.", compose_props(), &[]),
        def("gmail_draft", "Save an email as a draft in the user's Gmail (nothing is sent, the user is not asked): same arguments as gmail_send. The user reviews it in Gmail; send it with gmail_send {draft}. With draft, replaces that draft.", compose_props(), &[]),
        def("calendar_events", "List events on the user's primary Google Calendar, recurring ones expanded, soonest first. Each event shows its id, for calendar_update and calendar_delete.",
            with_account(json!({"from":prop("string","Start, RFC 3339 (e.g. 2026-09-24T09:00:00-07:00) or a date; default now."),"to":prop("string","End, RFC 3339 or a date; default a week after from."),"max":prop("number","How many (default 20, at most 50)."),"query":prop("string","Free-text filter (optional).")})), &[]),
        def("calendar_create", "Create an event on the user's primary Google Calendar; attendees get invitations. The user is asked to approve it first.",
            with_account(json!({"summary":prop("string","Title."),"start":prop("string","Start, RFC 3339 with an offset (e.g. 2026-09-24T09:00:00-07:00), or YYYY-MM-DD when all_day."),"end":prop("string","End, same form as start. For all_day, the day after the last day (defaults to one day)."),
                "description":prop("string","Details (optional)."),"attendees":arr("Attendee emails (optional)."),"all_day":prop("boolean","An all-day event (optional)."),"timeZone":prop("string","IANA time zone, e.g. America/Los_Angeles, for times without an offset (optional)."),
                "location":prop("string","Where (optional)."),"reminders":reminders()})), &["summary", "start", "end"]),
        def("calendar_update", "Change an event on the user's primary Google Calendar (ids are in calendar_events); only the fields given change. Giving start without end keeps the event's length. Guests are told. The user is asked to approve it first.",
            with_account(json!({"id":prop("string","The event id from calendar_events."),"summary":prop("string","New title."),"description":prop("string","New details."),"location":prop("string","New place."),
                "start":prop("string","New start, RFC 3339 with an offset, or YYYY-MM-DD when all_day."),"end":prop("string","New end, same form as start."),"all_day":prop("boolean","The times are dates (an all-day event)."),
                "timeZone":prop("string","IANA time zone for times without an offset."),"attendees":arr("Attendee emails; replaces the whole list ([] removes everyone)."),"reminders":reminders(),"default_reminders":prop("boolean","true gives the event the calendar's default alerts back.")})), &["id"]),
        def("calendar_delete", "Delete an event from the user's primary Google Calendar (ids are in calendar_events); guests are told. The user is asked to approve it first.", with_account(json!({"id":prop("string","The event id from calendar_events.")})), &["id"]),
        def("google_accounts", "List the Google accounts this bot may use, by address, with what each allows; the first is the default the other tools use.", json!({}), &[]),
    ]
}

pub struct GoogleTools(pub Weak<Google>);
impl ToolSet for GoogleTools {
    fn list(&self, thread: &str) -> Vec<Value> {
        let Some(g) = self.0.upgrade() else { return vec![] };
        if g.listed(thread) { defs() } else { vec![] }
    }
    fn owns(&self, name: &str) -> bool { NAMES.contains(&name) }
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> Fut<'a, Res> {
        Box::pin(async move { match self.0.upgrade() { Some(g) => g.call(thread, name, a).await, None => err("Google is not running") } })
    }
}

/// An approval's answer.
enum Asked { Yes, No }

impl Google {
    /// The tool list decides from what was last folded (bots' threads, the grants kept beside them); `call` decides again.
    pub fn listed(&self, thread: &str) -> bool {
        let Some(b) = &self.bots else { return false };
        b.thread_bot(thread).map_or(false, |bot| self.granted_cache.read().unwrap().get(&bot).map_or(false, |v| !v.is_empty()))
    }
    pub async fn call(&self, thread: &str, name: &str, a: &Value) -> Res {
        let Some((bot, bot_name)) = self.bot_of(thread).await else { return err("only a bot has Google's tools (and a run working for one)") };
        let mine = self.bot_accounts(&bot).await;
        if mine.is_empty() { return err(&format!("no Google account is allowed for {bot_name}: the person allows one in Settings, Accounts, Google")) }
        if name == "google_accounts" { return ok(accounts_text(&mine)) }
        let want = s(a, "account").trim();
        let acct = if want.is_empty() { mine[0].clone() } else {
            match mine.iter().find(|x| x.email.eq_ignore_ascii_case(want)) { Some(x) => x.clone(), None => return err(&format!("{bot_name} may not use a Google account {want}; it may use: {}", mine.iter().map(|x| x.email.as_str()).collect::<Vec<_>>().join(", "))) }
        };
        if acct.expired() { return err(&super::expired_text(&acct.email)) }
        let gm = name.starts_with("gmail_");
        if gm && !oauth::has_gmail(&acct.scopes) { return err(&format!("{} did not allow Gmail when it signed in: the person signs in again and ticks Gmail", acct.email)) }
        if !gm && !oauth::has_calendar(&acct.scopes) { return err(&format!("{} did not allow Calendar when it signed in: the person signs in again and ticks Calendar", acct.email)) }
        if matches!(name, "calendar_create" | "calendar_update") { if let Some(e) = cal::check(a) { return err(&e) } }
        let run = self.in_run(thread).await;
        let r = match name {
            "gmail_search" => self.search(&acct, a).await,
            "gmail_read" => self.read(&acct, a).await,
            "gmail_send" | "gmail_draft" => self.mail(thread, name, &acct, a, run).await,
            "calendar_events" => self.events(&acct, a).await,
            "calendar_create" => self.create(thread, &acct, a, run).await,
            "calendar_update" | "calendar_delete" => self.change(thread, name, &acct, a, run).await,
            _ => Err(format!("no Google tool named {name}")),
        };
        match r { Ok(t) => ok(t), Err(e) => err(&e) }
    }
    async fn in_run(&self, thread: &str) -> bool { match &self.bots { Some(b) => b.world().await.run_of(thread).is_some(), None => false } }

    /// A request as the account, refreshed once when the token kept was refused.
    async fn api(&self, a: &Acct, mk: impl Fn(&str) -> net::Req<'static>) -> Result<net::Resp, String> {
        let tok = self.token(a).await?;
        let r = net::send(&self.hub.cfg.home, mk(&tok)).await;
        if r.code != "401" { return Ok(r) }
        self.forget_access(&a.id);
        let tok = self.token(a).await?;
        Ok(net::send(&self.hub.cfg.home, mk(&tok)).await)
    }
    async fn get(&self, a: &Acct, path: &str) -> Result<net::Resp, String> { let url = self.ends.api(path); self.api(a, |t| net::Req::get(url.clone(), t)).await }
    async fn get_ok(&self, a: &Acct, path: &str) -> Result<Value, String> { let r = self.get(a, path).await?; if r.ok() { Ok(r.json()) } else { Err(r.fail()) } }

    /// The person's answer to an approval: a session grant skips it; else an ask is opened and waited for. In a run the charter has decided.
    async fn ask(&self, thread: &str, tool: &str, detail: &str, run: bool) -> Result<Asked, String> {
        if run { return Ok(Asked::Yes) }
        let ag = self.agents().ok_or("the agents are not running")?;
        if ag.world().await.sessions.contains(&(thread.to_string(), tool.to_string())) { return Ok(Asked::Yes) }
        let now = super::now_s(); let id = crate::agents::lines::hub_id("q", now, ag.seq_n());
        let (tx, rx) = tokio::sync::oneshot::channel();
        ag.parked.lock().unwrap().insert(id.clone(), crate::agents::mcp::Parked { thread: thread.into(), tool: tool.into(), input: Value::Null, id: Value::Null, tx });
        let pend = ag.pending_ev(thread, 1, "", now).await;
        ag.commit_events(vec![json!({"$":"AskOpened","id":id,"thread":thread,"kind":"approval","tool":tool,"detail":detail,"options":"","at":now}), pend]).await;
        match rx.await { Ok(a) if a == "accept" || a == "session" => Ok(Asked::Yes), Ok(_) => Ok(Asked::No), Err(_) => Err("the question was closed before the person answered it (Backplane restarted?): nothing was done".into()) }
    }

    // ---- Gmail ----

    async fn search(&self, a: &Acct, args: &Value) -> Result<String, String> {
        let j = self.get_ok(a, &gmail::search_path(s(args, "query"), n(args, "max", 10, 25))).await?;
        let ids = gmail::ids(&j);
        if ids.is_empty() { return Ok("No messages.".into()) }
        let paths: Vec<String> = ids.iter().map(|id| gmail::get_path(id, false)).collect();
        let fs = paths.iter().map(|p| self.get(a, p));
        let ms: Vec<Value> = futures_util::future::join_all(fs).await.into_iter().filter_map(|r| r.ok()).filter(|r| r.ok()).map(|r| r.json()).filter(|m| m["id"].is_string()).collect();
        Ok(gmail::msg_list(&ms))
    }
    async fn read(&self, a: &Acct, args: &Value) -> Result<String, String> {
        let id = s(args, "id").trim(); if id.is_empty() { return Err("id is required".into()) }
        Ok(gmail::msg_full(&self.get_ok(a, &gmail::get_path(id, true)).await?))
    }
    async fn mail(&self, thread: &str, tool: &str, a: &Acct, args: &Value, run: bool) -> Result<String, String> {
        let draft = s(args, "draft").trim().to_string();
        if tool == "gmail_send" && !draft.is_empty() {
            if gmail::content(args) { return Err("send a draft as it is, or update it with gmail_draft first".into()) }
            let d = self.get_ok(a, &gmail::draft_path(&draft)).await.map_err(|e| format!("could not read the draft: {e}"))?;
            let mid = d["message"]["id"].as_str().unwrap_or("").to_string();
            if let Asked::No = self.ask(thread, tool, &gmail::draft_review(&a.email, &d), run).await? { return Ok("The person declined this.".into()) }
            // a draft is sent only as it was reviewed: updating a draft gives it a new message id
            let now = self.get_ok(a, &gmail::draft_path(&draft)).await.map(|d| d["message"]["id"].as_str().unwrap_or("").to_string()).unwrap_or_default();
            if now != mid { return Err("the draft changed after the person reviewed it; ask to send it again".into()) }
            let url = self.ends.api(&gmail::draft_send_path()); let body = json!({"id":draft});
            let r = self.api(a, |t| net::Req::json("POST", url.clone(), t, &body)).await?;
            return if r.ok() { Ok(gmail::sent(&r.json())) } else { Err(r.fail()) }
        }
        let (reply, th) = (s(args, "reply_to").trim().to_string(), s(args, "thread").trim().to_string());
        let orig = if !reply.is_empty() { Some(gmail::orig(&self.get_ok(a, &gmail::msg_meta_path(&reply)).await.map_err(|e| format!("could not read the message to reply to: {e}"))?)) }
            else if !th.is_empty() { Some(gmail::orig_thread(&self.get_ok(a, &gmail::thread_path(&th)).await.map_err(|e| format!("could not read the message to reply to: {e}"))?)) } else { None };
        if orig.as_ref().map_or(false, |o| o.msgid.is_empty()) { return Err("the message to reply to has no Message-ID".into()) }
        let sig = self.hub.setting(&format!("gmail.sig.{}", a.email)).await.unwrap_or_default();
        let m = gmail::msg_of(args, &a.email, orig.as_ref(), &sig);
        if let Some(p) = gmail::problem(&m, s(args, "body")) { return Err(p) }
        let atts = { let ps = m.paths.clone(); tokio::task::spawn_blocking(move || gmail::read_atts(&ps)).await.map_err(|_| "could not read the attachments".to_string())?.map_err(|e| format!("attachment: {e}"))? };
        let nonce = oauth::nonce(9)?.replace(['-', '_'], "x");
        let raw = gmail::compose(&m, &atts, &nonce);
        let (meta, path, method) = if tool == "gmail_draft" { (gmail::json_draft(&draft, &m.thread), gmail::upload_draft(&draft), if draft.is_empty() { "POST" } else { "PUT" }) } else { (gmail::json_send(&m.thread), gmail::upload_send(), "POST") };
        let (ctype, body) = gmail::upload(&meta, &raw, &nonce);
        if tool == "gmail_send" {
            if let Asked::No = self.ask(thread, tool, &gmail::review("", &a.email, &m, &gmail::att_line(&atts)), run).await? { return Ok("The person declined this.".into()) }
        }
        let url = self.ends.api(&path);
        let r = self.api(a, |t| net::Req { method, url: url.clone(), headers: vec![format!("Authorization: Bearer {t}"), ctype.clone()], body: Some(body.clone()), secs: 120 }).await?;
        if !r.ok() { return Err(r.fail()) }
        Ok(if tool == "gmail_draft" { gmail::draft_saved(&r.json()) } else { gmail::sent(&r.json()) })
    }

    // ---- Calendar ----

    async fn events(&self, a: &Acct, args: &Value) -> Result<String, String> {
        let (from, to, now) = (s(args, "from").trim(), s(args, "to").trim(), super::now_s());
        let f = if from.is_empty() { cal::iso(now) } else { from.to_string() };
        Ok(cal::events(&self.get_ok(a, &cal::list_path(&f, &cal::until(from, to, now), n(args, "max", 20, 50), s(args, "query"))).await?))
    }
    fn acct_line(args: &Value) -> String { if s(args, "account").is_empty() { String::new() } else { format!("Account: {}\n", s(args, "account")) } }
    async fn create(&self, thread: &str, a: &Acct, args: &Value, run: bool) -> Result<String, String> {
        let all = args.get("all_day").and_then(Value::as_bool).unwrap_or(false);
        if s(args, "start").trim().is_empty() || (s(args, "end").trim().is_empty() && !all) { return Err("start and end is required".into()) }
        if let Asked::No = self.ask(thread, "calendar_create", &format!("{}{}", Self::acct_line(args), cal::create_detail(args)), run).await? { return Ok("The person declined this.".into()) }
        let body = cal::create_body(args); let url = self.ends.api(&cal::insert_path(!cal::emails(args.get("attendees")).is_empty()));
        let r = self.api(a, |t| net::Req::json("POST", url.clone(), t, &body)).await?;
        if r.ok() { Ok(cal::created(&r.json())) } else { Err(r.fail()) }
    }
    /// calendar_update and calendar_delete: the event fetched first, the arguments settled and shown in words, then exactly those run
    async fn change(&self, thread: &str, tool: &str, a: &Acct, args: &Value, run: bool) -> Result<String, String> {
        let id = s(args, "id").trim().to_string();
        if id.is_empty() { return Err("id is required".into()) }
        if !cal::id_ok(&id) { return Err("invalid event id".into()) }
        let update = tool == "calendar_update";
        if update && cal::nothing(args) { return Err("nothing to change".into()) }
        let r = self.get(a, &cal::event_path(&id)).await?;
        if cal::gone(&r.code) { return Err(cal::nosuch(&id)) }
        if !r.ok() { return Err(r.fail()) }
        let old = r.json();
        let (res, detail) = if update { let res = cal::resolve(&old, args); let d = cal::change_detail(&old, &res); (res, d) } else { (json!({"_notify": old["attendees"].as_array().map_or(false, |x| !x.is_empty())}), cal::delete_detail(&old)) };
        if let Asked::No = self.ask(thread, tool, &format!("{}{detail}", Self::acct_line(args)), run).await? { return Ok("The person declined this.".into()) }
        let url = self.ends.api(&cal::patch_path(&id, cal::told(&res)));
        let r = if update { let body = cal::patch_body(&res); self.api(a, |t| net::Req::json("PATCH", url.clone(), t, &body)).await? }
            else { self.api(a, |t| { let mut q = net::Req::get(url.clone(), t); q.method = "DELETE"; q }).await? };
        if cal::gone(&r.code) { return Err(cal::nosuch(&id)) }
        if !r.ok() { return Err(r.fail()) }
        Ok(if update { cal::updated(&r.json()) } else { cal::deleted(&old) })
    }
}

/// google_accounts' answer
pub fn accounts_text(xs: &[Acct]) -> String {
    format!("Google accounts this bot may use (the first is the default):\n{}", xs.iter().map(|a| format!("{} ({}{})", a.email, oauth::scope_words(&a.scopes), if a.expired() { "; sign-in expired: the person signs in again" } else { "" })).collect::<Vec<_>>().join("\n"))
}
