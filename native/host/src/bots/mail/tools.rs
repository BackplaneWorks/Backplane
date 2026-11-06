//! The mailroom's MCP tools: what a mail run, a coordinator's
//! back office and (with the mailroom on) any thread may call. What a thread lists follows what it is (`lists`); every call checks again on the state it runs on. Bodies stay in the journal.
//! Not served: `mail` to a room (it is `Bots.mail.room`: the net lane's rooms are not all here), `Bots.call.private`'s relay of notify_person is `notify_relay` below.
use super::{escalate::{disposition_key, disposition_pending}, requests, route::{Ctx, RouteOut, Step}, Origin};
use crate::{agents::mcp::ToolSet, api::Fut, bots::{clock::when_parse, rpc::{now_s, Bots}, sched_text::person_rule, world::World}};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

type Res = (bool, Value);
fn fail(m: &str) -> Res { (false, json!({"error":m})) }
fn gs<'a>(a: &'a Value, k: &str) -> &'a str { a.get(k).and_then(Value::as_str).unwrap_or("") }

const MAIL: [&str; 7] = ["mail", "mail_list", "mail_read", "mail_search", "mail_tag", "mail_snooze", "mail_done"];
const OWN: [&str; 13] = ["mail", "mail_list", "mail_read", "mail_search", "mail_tag", "mail_snooze", "mail_done", "decide", "escalate", "charter_set", "mail_dispose", "mail_context", "tell_person"];

fn def(name: &str, about: &str, props: Value, req: &[&str]) -> Value { json!({"name":name,"description":about,"inputSchema":{"type":"object","properties":props,"required":req}}) }
fn p(ty: &str, about: &str) -> Value { json!({"type":ty,"description":about}) }
/// Tool.def of a mailroom tool, in words
pub fn tool(name: &str) -> Value {
    match name {
        "mail" => def(name, "Send mail to an address, delivered at once and filed in its conversation.", json!({"to":p("string","thread:<id>, bot:<id> or room:<id>."),"text":p("string","The message."),"conv":p("string","Continue this conversation."),"subject":p("string","Subject for a new conversation."),"want":p("string","fyi or act.")}), &["text"]),
        "mail_list" => def(name, "List your mail conversations, newest first, optionally by status or project.", json!({"bot":p("string","Bot id; * for all mail (bots only)."),"status":p("string","unread, open, snoozed or done."),"project":p("string","Project id."),"n":p("number","Page size."),"skip":p("number","Rows to skip.")}), &[]),
        "mail_read" => def(name, "Read a conversation's message bodies and mark it seen.", json!({"conv":p("string","Conversation id."),"before":p("string","Mail id cursor."),"n":p("number","Page size (default 50).")}), &["conv"]),
        "mail_search" => def(name, "Search mail subjects, participants, tags and newest headers.", json!({"q":p("string","Query."),"bot":p("string","Bot id or *."),"n":p("number","Maximum results.")}), &["q"]),
        "mail_tag" => def(name, "Set your conversation's tags.", json!({"conv":p("string","Conversation id."),"tags":{"type":"array","items":{"type":"string"}}}), &["conv", "tags"]),
        "mail_snooze" => def(name, "Snooze a conversation until a time; the minute tick reopens it unread.", json!({"conv":p("string","Conversation id."),"until":p("number","Unix time in seconds."),"in":p("string","Relative time, e.g. 20m or 2h.")}), &["conv"]),
        "mail_done" => def(name, "Mark a conversation done.", json!({"conv":p("string","Conversation id.")}), &["conv"]),
        "decide" => def(name, "Record a decision and its reason in this run's conversation.", json!({"text":p("string","The decision."),"reason":p("string","Why.")}), &["text", "reason"]),
        "escalate" => def(name, "Privately ask the owning coordinator about a decision, blocker, meaningful completion or uncertain result. Coordinator triage routes even person requests through the owner, never directly alerts. Supply exact input in a coordinator run. key correlates the same exact event/version across reports, rooms and wakes; use a new key for a new result.", json!({"to":p("string","bot or person."),"text":p("string","Contextual assessment and actionable source links."),"input":p("string","Exact current input ID; required for coordinator triage."),"key":p("string","Stable exact event/version, e.g. source ask ID, thread turn ID or artifact commit; not a broad topic.")}), &["to", "text"]),
        "charter_set" => def(name, "Front of house only: replace your mailroom charter.", json!({"text":p("string","The full charter."),"reason":p("string","Why it changed.")}), &["text", "reason"]),
        "mail_dispose" => def(name, "Live Mailroom run only: durably handle ONE exact current notice. done/defer acknowledge a routine status or known hold without completing work or granting permission. reply files FYI to that input's existing local sender, without starting an agent. Pending/unknown replies are never automatically resent. Substantive decisions/escalations and external/product actions retain their existing review.", json!({"input":p("string","Exact current actionable input ID from the run brief or incoming mail heading."),"mode":p("string","done, defer, or reply. This does not mark the conversation or objective complete."),"reason":p("string","Why this notice needs no new action, is already handled, or remains deferred. Required, up to 2000 characters."),"text":p("string","For reply only: internal FYI text, up to 4000 characters. Target and conversation are fixed by the current input.")}), &["input", "mode", "reason"]),
        "mail_context" => def(name, "Read your coordinator's full stored conversation history, including older context beyond the brief. Owner identity is fixed by this live run; no target argument is accepted. With query, search text; before pages older rows by seq; use next from the result. Read-only; never acknowledges mail or answers approvals.", json!({"query":p("string","Text to find, or empty for recent history."),"before":p("integer","Read rows before this seq; zero means newest.")}), &[]),
        "tell_person" => def(name, "The ONLY explicit publication from a private coordinator turn into its original user conversation. All ordinary final/commentary text stays private; empty output is successful. Writes your own contextual text once per owner/event key, with provenance and a told ledger receipt. Never grants approval or completes source work. Target is authenticated owner, never caller supplied.", json!({"text":p("string","Your fresh contextual message with actionable source links."),"key":p("string","Stable exact event/version key; repeated calls do not post twice.")}), &["text", "key"]),
        _ => Value::Null,
    }
}
fn scope(w: &World, thread: &str) -> String { w.run_bot(thread).map_or_else(|| format!("thread:{thread}"), |b| b.id.clone()) }
/// Mcp.mail.uber
fn mail_uber(w: &World, thread: &str) -> bool { w.uber_is(&scope(w, thread)) }
/// Mcp.tools.for, as names: a thread with none of its own is under "*". A live run counts as running here: its tools are listed as its process starts, before its turn's first line.
pub fn lists(w: &World) -> HashMap<String, Vec<&'static str>> {
    let on = w.setting("mail.on", "off") == "on";
    let mut m: HashMap<String, Vec<&'static str>> = HashMap::new();
    if on { m.insert("*".into(), MAIL.iter().copied().chain(["decide", "escalate", "charter_set"]).collect()); }
    let mut threads: Vec<String> = w.mail.runs.values().filter(|r| r.live).map(|r| r.thread.clone()).collect();
    let tt = w.triage_thread(); if !tt.is_empty() { threads.push(tt) }
    for t in threads {
        let run = w.run_of(&t).filter(|r| r.live).is_some(); let private = w.private_is(&t);
        let mut v: Vec<&'static str> = vec![];
        if on { v.extend(MAIL); v.extend(["decide", "escalate", "charter_set"]); if run { v.extend(["mail_dispose", "mail_context"]) } }
        else if mail_uber(w, &t) { v.extend(["mail_list", "mail_read"]); if w.run_is(&t) && !private { v.extend(["decide", "escalate"]) } if run { v.extend(["mail_dispose", "mail_context"]) } }
        else if run { v.extend(["mail_dispose", "mail_context"]) }
        if private { v.extend(["mail_context", "tell_person"]) }
        m.insert(t, v);
    }
    m
}

pub struct MailTools(pub Arc<Bots>);
impl ToolSet for MailTools {
    fn list(&self, thread: &str) -> Vec<Value> {
        let l = self.0.mail_lists.read().unwrap();
        l.get(thread).or_else(|| l.get("*")).map_or_else(Vec::new, |v| v.iter().map(|n| tool(n)).collect())
    }
    fn owns(&self, name: &str) -> bool { OWN.contains(&name) }
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> Fut<'a, Res> { Box::pin(async move { self.0.mail_tool(thread, name, a).await }) }
}

/// Disposition.args: nothing but the four named text fields, a known mode, a reason, a reply with text
fn dispose_args(a: &Value) -> bool {
    let Some(o) = a.as_object() else { return false };
    if !o.iter().all(|(k, v)| ["input", "mode", "reason", "text"].contains(&k.as_str()) && v.is_string()) { return false }
    let (mode, reason, text) = (gs(a, "mode"), gs(a, "reason").trim(), gs(a, "text"));
    ["done", "defer", "reply"].contains(&mode) && !reason.is_empty() && reason.chars().count() <= 2000 && text.chars().count() <= 4000 && (mode != "reply" || !text.trim().is_empty())
}
/// Private.args: only text and key, both given and within their bounds
fn private_args(a: &Value) -> bool {
    let Some(o) = a.as_object() else { return false };
    if !o.iter().all(|(k, v)| ["text", "key"].contains(&k.as_str()) && v.is_string()) { return false }
    let (t, k) = (gs(a, "text").trim(), gs(a, "key").trim()); !t.is_empty() && t.chars().count() <= 32000 && !k.is_empty() && k.chars().count() <= 200
}
const SCOPE: &str = "current notice only; no work completion or permission grant";
fn answer(r: &super::mailbox::Run, input: &str, state: &str) -> Value { json!({"input":input,"conv":r.conv,"state":state,"scope":SCOPE}) }
fn receipt(c: &Ctx, r: &super::mailbox::Run, a: &Value) -> Vec<Value> {
    let (input, mode, reason) = (gs(a, "input"), gs(a, "mode"), gs(a, "reason")); let key = disposition_key(&r.thread, input);
    vec![json!({"$":"SettingSet","key":key,"value":super::route::jshow(json!({"bot":r.bot,"conv":r.conv,"input":input,"mode":mode,"reason":reason})),"at":c.now}),
        json!({"$":"ActivityLogged","thread":r.thread,"act":key,"tone":"info","kind":"mail-disposition","summary":format!("{mode} notice {input}: {reason}"),"at":c.now})]
}
/// Mail.target: the thread of a thread: or bot: address
fn target(w: &World, to: &str) -> String { let id = |p: &str| to.strip_prefix(p).map(|x| x.split('/').next().unwrap_or("").to_string()); id("thread:").or_else(|| id("bot:").map(|b| w.bot(&b).map(|x| x.thread.clone()).unwrap_or_default())).unwrap_or_default() }
/// Mcp.dispose
fn dispose(c: &mut Ctx, thread: &str, a: &Value) -> (Res, Step) {
    let Some(r) = c.w.run_of(thread).cloned() else { return (fail("mail_dispose is only available in a live mailroom run"), Step::none()) };
    let input = gs(a, "input"); let owner = c.triage_owner(&r.bot); let done = !c.setting(&disposition_key(&r.thread, input), "").is_empty();
    let valid = (c.disposition_valid(&r, input) || (owner && c.disposition_live(&r) && done)) && dispose_args(a) && !(owner && c.ask_mail(&r.conv, input));
    if !valid { return ((false, json!({"error":"mail_dispose requires a live mailroom run, exact current input, done/defer/reply, and reason; no other fields are allowed. Nothing was handled. Use current_input only; newer arrivals belong to separate runs.","current_input":c.disposition_current(&r),"bound_input":r.mail})), Step::none()) }
    if done { return ((true, answer(&r, input, "already handled")), Step::none()) }
    if !c.setting(&disposition_pending(&r.thread, input), "").is_empty() { return ((true, answer(&r, input, "pending or unknown journal outcome; no resend")), Step::none()) }
    if gs(a, "mode") != "reply" {
        let cs = receipt(c, &r, a); c.replay(&cs); let next = c.disposition_current(&r);
        return ((true, json!({"input":input,"conv":r.conv,"state":"handled","scope":SCOPE,"next_input":next})), Step { cs, outs: vec![] })
    }
    // a reply files FYI only: the worker's inbound funnel would start a new agent turn
    let to = c.w.mail.headers(&r.conv).iter().find(|m| m.id == input).map(|m| m.from.clone()).unwrap_or_default();
    let th = target(c.w, &to);
    if !((to.starts_with("thread:") || to.starts_with("bot:")) && !to.contains('~') && !th.is_empty() && c.f.threads.contains_key(&th)) { return (fail("reply requires the current input's existing local thread or bot sender; use reviewed mail/escalate otherwise"), Step::none()) }
    let cs = vec![json!({"$":"SettingSet","key":disposition_pending(&r.thread, input),"value":"on","at":c.now})]; c.replay(&cs);
    let out = RouteOut::File { o: Origin::new(&format!("thread:{}", r.thread), "run-reply", input), addr: to, text: gs(a, "text").into(), msg: c.id("mail-disposition"), want: "fyi".into(), after: receipt(c, &r, a), conv: r.conv.clone() };
    ((true, answer(&r, input, "pending journal commit")), Step { cs, outs: vec![out] })
}
/// Mcp.run.call: decide and escalate, on the run the thread is
fn run_call(c: &mut Ctx, thread: &str, name: &str, a: &Value) -> (Res, Step) {
    let Some(r) = c.w.mail.run_of(thread).cloned() else { return (fail("This tool is only available in a mailroom run"), Step::none()) };
    if name == "decide" { let st = Bots::run_decide(c, &r, gs(a, "text"), gs(a, "reason")); return ((true, json!({"ok":true})), st) }
    let (to, owner) = (gs(a, "to"), c.triage_owner(&r.bot)); let input = gs(a, "input");
    if !(["bot", "person"].contains(&to) && (!owner || (!input.is_empty() && input == c.disposition_current(&r)))) { return (fail("to must be bot or person; coordinator triage must supply the exact current input ID"), Step::none()) }
    let key = gs(a, "key"); let st = c.run_escalate_topic(owner && !key.is_empty(), &r.bot, &r.conv, if to.is_empty() { "bot" } else { to }, gs(a, "text"), key);
    ((true, json!({"ok":true})), st)
}

impl Bots {
    /// Bots.call.private, Bots.call.scoped for notify_person (tool_call_at asks this first; None: its plain path): a back office publishes with tell_person only, a managed thread's
    /// notice goes through the coordinator
    pub async fn notify_relay(self: &Arc<Self>, thread: &str, a: &Value) -> Option<Res> {
        let (private, managed, run) = { let w = self.world().await; (w.private_is(thread), w.managed(thread), w.run_of(thread).cloned()) };
        if private { return Some(fail("Private coordinator: use tell_person(text, key) to publish into the original user conversation. No message was sent and no approval or source input was handled.")) }
        if !managed { return None }
        let (reason, text) = (gs(a, "reason").to_string(), gs(a, "text").to_string());
        let (res, outs) = self.decide_with(&[], |c| match &run {
            Some(r) => { let st = c.run_escalate(&r.bot, &r.conv, "bot", &text); ((true, json!({"ok":true,"note":"escalated privately to the coordinator; no direct person alert"})), st) }
            None => { let (o, tt) = (Origin::new(&c.w.mail_addr(thread), "bot", ""), c.w.triage_thread()); let st = c.operational(o, &tt, &format!("{reason}: {text}"), &c.id("bot-notice"), "act", vec![]); ((true, json!({"ok":true,"note":"queued for coordinator triage; no direct person alert"})), st) }
        }).await;
        self.perform(outs).await; Some(res)
    }
    pub(crate) async fn mail_tool(self: &Arc<Self>, thread: &str, name: &str, a: &Value) -> Res {
        match name {
            "mail_dispose" => { let (r, outs) = self.decide_with(&[], |c| dispose(c, thread, a)).await; self.perform(outs).await; r }
            "tell_person" => {
                let (r, outs) = self.decide_with(&[], |c| {
                    let allowed = c.w.private_is(thread) && c.th(thread).turn == "running" && private_args(a);
                    let st = if allowed { c.private_tell(thread, gs(a, "text").trim(), gs(a, "key").trim()) } else { Step::none() };
                    ((allowed, json!({"ok":allowed,"scope":"Explicit publication to this private coordinator's original conversation only; no approval or source work changed"})), st)
                }).await; self.perform(outs).await; r
            }
            "decide" | "escalate" | "charter_set" => {
                let on = { let w = self.world().await; w.setting("mail.on", "off") == "on" || (mail_uber(&w, thread) && w.run_is(thread)) };
                if !on { return fail("mail is off") }
                if name == "charter_set" { return self.charter_set(thread, a).await }
                let (r, outs) = self.decide_with(&[], |c| run_call(c, thread, name, a)).await; self.perform(outs).await; r
            }
            "mail_context" => self.mail_context(thread, a).await,
            _ => self.mail_box(thread, name, a).await,
        }
    }
    /// Mcp.charter.got: the front of house's own bot; the charter is also written beside its work
    async fn charter_set(&self, thread: &str, a: &Value) -> Res {
        let w = self.world().await; let now = now_s();
        let Some(b) = w.bots.values().find(|b| !b.deleted && b.thread == thread).cloned() else { return fail("charter_set is for the front of house bot only") };
        let ev = [json!({"$":"CharterSet","bot":b.id,"text":gs(a, "text"),"by":"bot","at":now}), json!({"$":"ActivityLogged","thread":b.thread,"act":self.hub_id("charter", now),"tone":"info","kind":"charter","summary":gs(a, "reason"),"at":now})];
        if let Err(e) = self.hub.commit_many(&ev).await { return fail(&format!("{e:#}")) }
        let m = self.hub.model().await; let dir = m.threads.get(&b.thread).map(|t| t.worktree.clone()).unwrap_or_default(); drop(m);
        if !dir.is_empty() { let (p, t) = (std::path::Path::new(&dir).join("CHARTER.md"), gs(a, "text").to_string()); let _ = tokio::task::spawn_blocking(move || std::fs::write(p, t)).await; }
        (true, json!({"ok":true}))
    }
    /// Mcp.mail.call: the box itself; with the mailroom off the uberbot still lists and reads its mail
    async fn mail_box(self: &Arc<Self>, thread: &str, name: &str, a: &Value) -> Res {
        let (ok, p, conv_ok) = { let w = self.world().await;
            let (uber, sc) = (mail_uber(&w, thread), scope(&w, thread)); let on = w.setting("mail.on", "off") == "on";
            let free = ["mail", "mail_list", "mail_search"].contains(&name);
            let key = if sc.starts_with("thread:") { sc.clone() } else { format!("bot:{sc}") };
            let allowed = uber || w.mail.conv(gs(a, "conv")).parts.contains(&key);
            let gn = |k: &str, d: u64| a.get(k).and_then(Value::as_u64).unwrap_or(d);
            let p = json!({"bot":if uber { a.get("bot").and_then(Value::as_str).unwrap_or(&sc) } else { &sc },"status":gs(a, "status"),"project":gs(a, "project"),"q":gs(a, "q"),"skip":gn("skip", 0),"n":gn("n", gn("limit", 50)),"conv":gs(a, "conv"),"before":gs(a, "before"),"tags":a.get("tags").cloned().unwrap_or(Value::Null)});
            (on || (uber && ["mail_list", "mail_read"].contains(&name)), p, free || allowed) };
        if !(ok && conv_ok) { return fail("mail is off or the conversation is not yours") }
        match name {
            "mail_list" => (true, requests::list(&*self.world().await, &p)),
            "mail_search" => (true, requests::find(&*self.world().await, &p)),
            "mail_read" => { let r = self.mail_read(&json!(0), &p).await.remove(0); if r["ok"] == true { (true, r["result"].clone()) } else { (false, json!({"error":r["error"]})) } }
            "mail_tag" => { let tags: Vec<&str> = p["tags"].as_array().map(|x| x.iter().filter_map(Value::as_str).collect()).unwrap_or_default(); self.mail_commit(json!({"$":"MailTagged","bot":p["bot"],"conv":p["conv"],"tags":tags.join("\n"),"at":now_s()})).await }
            "mail_snooze" | "mail_done" => { let w = self.world().await; let now = now_s(); let sc = scope(&w, thread);
                let until = if name == "mail_done" { 0 } else { match a.get("until").and_then(Value::as_u64).filter(|u| *u > 0) { Some(u) => u, None => { let z = crate::bots::clock::Zones { settings: &w.settings }; when_parse(&format!("in {}", gs(a, "in")), now, &person_rule(&z, now)).unwrap_or(0) } } };
                let c = w.mail.conv(gs(a, "conv")); drop(w);
                self.mail_commit(json!({"$":"MailMarked","bot":sc,"conv":gs(a, "conv"),"status":if name == "mail_done" { "done" } else { "snoozed" },"seen":c.last,"until":until,"at":now})).await }
            "mail" => self.mail_send_tool(thread, a).await,
            _ => fail("unknown tool"),
        }
    }
    async fn mail_commit(&self, e: Value) -> Res { match self.hub.commit(&e).await { Ok(_) => (true, json!({"ok":true})), Err(e) => fail(&format!("{e:#}")) } }
    /// Mail.send from a thread: a bot's thread with the mailroom on gets the mail only; any other is delivered to, through the funnel, then filed in the conversation
    async fn mail_send_tool(self: &Arc<Self>, thread: &str, a: &Value) -> Res {
        let w = self.world().await; let now = now_s();
        let run = w.mail.run_of(thread).cloned();
        let reply_to = |r: &super::mailbox::Run| w.mail.conv(&r.conv).parts.iter().find(|h| **h != format!("bot:{}", r.bot) && **h != format!("thread:{}", r.thread)).cloned().unwrap_or_default();
        let to = if a.get("to").is_some() { gs(a, "to").to_string() } else { run.as_ref().map(reply_to).unwrap_or_default() };
        let conv = if a.get("conv").is_some() { gs(a, "conv").to_string() } else { run.as_ref().map(|r| r.conv.clone()).unwrap_or_default() };
        if to.starts_with("room:") { return fail("mail to a room is not served here yet; use room_post") }
        let th = target(&w, &to); let text = gs(a, "text");
        if th.is_empty() || !self.hub.model().await.threads.contains_key(&th) { return (true, json!({"ok":true})) } // Mail.send.valid: an unknown target is no step
        let (from, addr, msg) = (w.mail_addr(thread), w.mail_addr(&th), self.hub_id("mail", now));
        let quiet = w.setting("mail.on", "off") == "on" && w.is_bot_thread(&th); let o = Origin::new(&from, "thread", "");
        let cid = if conv.is_empty() { super::route(&o, &addr) } else { conv.clone() };
        let filed = self.file_in(&w, now, &o, &addr, text, &msg, gs(a, "want"), vec![], &cid).await; drop(w);
        if let Err(e) = filed { return fail(&e) }
        if !quiet { let id = format!("t:{thread}:{msg}"); if let Err(e) = self.inbox_deliver_raw(&th, &id, text, Some((from, "thread".into(), String::new()))).await { return fail(&e) } }
        (true, json!({"ok":true}))
    }
    /// Context.read: the owner's whole conversation, a page at a time; no other thread can be named
    async fn mail_context(&self, caller: &str, a: &Value) -> Res {
        let owner = { let w = self.world().await; let live = |t: &str| t == "running";
            let m = self.hub.model().await; let turn = m.threads.get(caller).map(|t| t.turn.clone()).unwrap_or_default(); drop(m);
            if w.private_is(caller) && live(&turn) { w.triage_thread() } else { match w.mail.run_of(caller) { Some(r) if r.live && live(&turn) && w.run_of(caller).is_some() => w.bot(&r.bot).map(|b| b.thread.clone()).unwrap_or_default(), _ => String::new() } } };
        let shape = a.as_object().map_or(a.is_null(), |o| o.iter().all(|(k, v)| (k == "query" && v.is_string()) || (k == "before" && v.is_number())));
        if owner.is_empty() || !shape { return (true, json!({"error":"mail_context requires a live owner-scoped mailroom run and only query/before arguments"})) }
        let q: String = gs(a, "query").chars().take(200).collect::<String>().to_lowercase(); let before = a.get("before").and_then(Value::as_u64).unwrap_or(0);
        let (mut rows, mut at, mut more): (Vec<Value>, u64, bool) = (vec![], 0, true);
        while rows.len() < 21 && more {
            let page = self.hub.model().await.page(&self.hub.store, &owner, at, 32);
            let Ok(page) = page else { return (true, json!({"error":"Owner history unavailable; do not infer missing context or acknowledge an important ask. Escalate to the coordinator."})) };
            for r in page.rows.iter().filter(|r| r.kind == "msg" && r.thread == owner && (before == 0 || r.seq < before) && (q.is_empty() || r.text.to_lowercase().contains(&q))) { if rows.len() < 21 && !rows.iter().any(|x| x["seq"] == r.seq) { rows.push(json!({"seq":r.seq,"id":r.id,"role":r.role,"text":r.text,"at":r.at})) } }
            let low = page.rows.iter().map(|r| r.ord).min().unwrap_or(0); more = low > 1 && low != at && !page.rows.is_empty() && page.below > 0; at = low;
        }
        let next = if rows.len() > 20 { rows[19]["seq"].as_u64().unwrap_or(0) } else { 0 }; rows.truncate(20);
        (true, json!({"owner":owner,"link":super::route::ref_uri("thread", &owner),"messages":rows,"next":next}))
    }
}
