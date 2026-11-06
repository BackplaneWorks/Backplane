//! The mail requests a client makes: mail.list, mail.read,
//! mail.find, mail.mark, mail.tag, mail.send. They are the person's: always on, whatever `mail.on` says. Bodies are read from the journal (store.rs) only here.
//! Not ported (PARITY): mail.read of a room's conversation (RoomHistory), owner.mail (the owner's exchange with other machines).
use super::{file::subject, store, Body, Conv, Hdr, Origin};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use crate::bots::{rpc::{now_s, Bots}, world::World};
use serde_json::{json, Value};
use std::sync::Arc;

fn err(id: &Value, m: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":m}) }
fn ok(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":true}) }
fn data(id: &Value, v: Value) -> Value { json!({"t":"reply","id":id,"ok":true,"result":v}) }
fn gs<'a>(p: &'a Value, k: &str) -> &'a str { p.get(k).and_then(Value::as_str).unwrap_or("") }
/// J.Json.get_nat: a number, else the default
fn gn(p: &Value, k: &str, d: u64) -> u64 { match p.get(k) { Some(Value::Number(n)) => n.as_u64().unwrap_or(d), Some(Value::String(s)) if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) => s.parse().unwrap_or(d), _ => d } }
/// Mail.limit
fn limit(n: u64) -> usize { n.min(200) as usize }

/// Mail.row
fn row(w: &World, bot: &str, c: &Conv) -> Value {
    let mark = w.mail.mark_of(bot, &c.id); let last = w.mail.newest(&c.id);
    let tags: Vec<&String> = c.tags.iter().chain(mark.tags.iter()).collect();
    json!({"id":c.id,"subject":c.subject,"kind":c.kind,"parts":c.parts,"project":c.project,"tags":tags,"last":c.last,"n":c.n,"status":mark.status,"unread":mark.status == "unread","head":last.head,"from":last.from})
}
/// the conversations of a scope (Box.order_of), newest first, equal times by id
fn scope(w: &World, bot: &str) -> Vec<Conv> {
    let key = if bot.starts_with("thread:") { bot.to_string() } else { format!("bot:{bot}") };
    let mut v: Vec<Conv> = w.mail.convs.values().filter(|c| bot == "*" || c.parts.contains(&key)).cloned().collect();
    v.sort_by(|a, b| b.last.cmp(&a.last).then_with(|| a.id.cmp(&b.id))); v
}
fn has(s: &str, q: &str) -> bool { s.to_lowercase().contains(q) }
/// Box.search over short header metadata
fn search(w: &World, bot: &str, c: &Conv, q: &str) -> bool {
    q.is_empty() || has(&c.subject, q) || c.parts.iter().any(|p| has(p, q)) || c.tags.iter().any(|t| has(t, q)) || w.mail.mark_of(bot, &c.id).tags.iter().any(|t| has(t, q)) || has(&w.mail.newest(&c.id).head, q)
}

pub fn list(w: &World, p: &Value) -> Value {
    let bot = p.get("bot").and_then(Value::as_str).unwrap_or("*"); let (status, project) = (gs(p, "status"), gs(p, "project"));
    let all = scope(w, bot);
    let plain = status.is_empty() && project.is_empty();
    let hits: Vec<Conv> = all.into_iter().filter(|c| plain || ((status.is_empty() || w.mail.mark_of(bot, &c.id).status == status) && (project.is_empty() || c.project == project))).collect();
    let total = hits.len();
    let page: Vec<Value> = hits.iter().skip(gn(p, "skip", 0) as usize).take(limit(gn(p, "n", 50))).map(|c| row(w, bot, c)).collect();
    json!({"convs":page,"total":total})
}
pub fn find(w: &World, p: &Value) -> Value {
    let bot = p.get("bot").and_then(Value::as_str).unwrap_or("*"); let q = gs(p, "q").to_lowercase();
    let cs: Vec<Value> = scope(w, bot).iter().filter(|c| search(w, bot, c, &q)).take(limit(gn(p, "n", 50))).map(|c| row(w, bot, c)).collect();
    json!({"total":cs.len(),"convs":cs})
}
fn body_row(b: &Body) -> Value { let m = &b.mail; json!({"id":m.id,"from":m.from,"to":m.to,"kind":m.kind,"want":m.want,"at":m.at,"text":b.text}) }

impl Bots {
    pub(crate) async fn mail_read(&self, id: &Value, p: &Value) -> Vec<Value> {
        let (conv, bot, n) = (gs(p, "conv"), gs(p, "bot"), limit(gn(p, "n", 50))); let before = gs(p, "before").to_string();
        // under the world lock: the seen mark, and a copy of what the read needs (the conversation, the page's headers, the room's log lines).
        // The files are read in a blocking task over that copy (the closure is 'static, so the guard cannot go into it), and the lock is taken again only for the row.
        let snap = {
            let w = self.world().await;
            if !bot.is_empty() { // Mail.seen: reading opens it for that bot
                let now = now_s(); let c = w.mail.conv(conv);
                if let Err(e) = self.hub.commit(&json!({"$":"MailMarked","bot":bot,"conv":conv,"status":"open","seen":c.last,"until":0,"at":now})).await { return vec![err(id, &format!("{e:#}"))] }
            }
            if conv.starts_with("room:") { match room_snapshot(&w, conv, &before, n) { Ok(s) => Some(s), Err(e) => return vec![err(id, &e)] } } else { None }
        };
        let (store_, home, c2) = (self.hub.store.clone(), self.hub.cfg.home.clone(), conv.to_string());
        let read = tokio::task::spawn_blocking(move || match snap { Some(s) => read_room(&store_, &home, &c2, s), None => read_plain(&home, &c2, &before, n) }).await;
        let (mails, more) = match read { Ok(Ok(v)) => v, Ok(Err(e)) => return vec![err(id, &e)], Err(e) => return vec![err(id, &e.to_string())] };
        let w = self.world().await;
        vec![data(id, json!({"conv":row(&w, bot, &w.mail.conv(conv)),"mails":mails,"more":more}))]
    }

    /// Mail.ui.request: the target must be a thread of this hub (a bot's address is its thread). A bot's thread: the mailbox only. Any other: the message is
    /// delivered to the thread as the person's first, then filed (Mail.send.worker's order), and the reply comes last, so a failed delivery or filing is told.
    ///
    async fn mail_send(&self, id: &Value, p: &Value) -> Vec<Value> {
        let to = gs(p, "to");
        let w = self.world().await;
        let thread = if let Some(t) = to.strip_prefix("thread:") { t.split('/').next().unwrap_or("").to_string() } else if let Some(b) = to.strip_prefix("bot:") { w.bot(b.split('/').next().unwrap_or("")).map(|b| b.thread.clone()).unwrap_or_default() } else { String::new() };
        if thread.is_empty() || !self.hub.model().await.threads.contains_key(&thread) { return vec![err(id, "invalid mail target")] }
        let now = now_s(); let msg = self.hub_id("mail", now); let text = gs(p, "text");
        let addr = w.mail_addr(&thread);
        let is_bot = w.bots.values().any(|b| !b.deleted && b.thread == thread);
        let o = Origin::new("person", "thread", "");
        let cid = { let c = gs(p, "conv"); if c.is_empty() { super::route(&o, &addr) } else { c.to_string() } };
        let old = w.mail.conv(&cid);
        let conv = if old.id.is_empty() {
            let n = self.names_for(&w).await;
            let subj = { let s = gs(p, "subject"); if s.is_empty() { subject(false, &n, &o, &addr) } else { s.to_string() } };
            Conv { id: cid.clone(), subject: subj, kind: super::conv_mail_kind(&cid, "thread"), parts: vec![o.from.clone(), addr.clone()], project: self.project_of(&w, &addr).await, tags: vec![], made: now, last: now, n: 0 }
        } else { old };
        let mail = Hdr { id: msg.clone(), conv: cid, from: "person".into(), to: addr, kind: "thread".into(), r#ref: String::new(), want: super::want("thread", gs(p, "want"), &super::flags(&o, text)), head: super::head(text), size: super::bytes(text), at: now };
        let body = Body { conv, mail, text: text.into() };
        let already = w.mail.has(&msg);
        drop(w);
        if !is_bot && !already { if let Err(e) = self.inbox_deliver_raw(&thread, &msg, text, None).await { return vec![err(id, &format!("could not deliver the message: {e}"))] } }
        let w = self.world().await;
        if let Err(e) = self.file_body(&w, body).await { return vec![err(id, &e)] }
        vec![ok(id)]
    }
}

struct MailRpc(Arc<Bots>);
impl RpcHandler for MailRpc {
    fn methods(&self) -> &'static [&'static str] { &["mail.list", "mail.read", "mail.find", "mail.mark", "mail.tag", "mail.send"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let b = &self.0;
            match method {
                "mail.list" => { let w = b.world().await; vec![data(id, list(&w, p))] }
                "mail.find" => { let w = b.world().await; vec![data(id, find(&w, p))] }
                "mail.read" => b.mail_read(id, p).await,
                "mail.mark" => {
                    let status = p.get("status").and_then(Value::as_str).unwrap_or("open");
                    if !["unread", "open", "snoozed", "done"].contains(&gs(p, "status")) { return vec![err(id, "invalid mail status")] }
                    let w = b.world().await; let now = now_s(); let c = w.mail.conv(gs(p, "conv"));
                    match b.hub.commit_many(&[json!({"$":"MailMarked","bot":gs(p, "bot"),"conv":gs(p, "conv"),"status":status,"seen":c.last,"until":gn(p, "until", 0),"at":now})]).await { Ok(_) => vec![ok(id)], Err(e) => vec![err(id, &format!("{e:#}"))] }
                }
                "mail.tag" => {
                    let _w = b.world().await; let now = now_s();
                    let tags: Vec<&str> = p.get("tags").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                    match b.hub.commit_many(&[json!({"$":"MailTagged","bot":gs(p, "bot"),"conv":gs(p, "conv"),"tags":tags.join("\n"),"at":now})]).await { Ok(_) => vec![ok(id)], Err(e) => vec![err(id, &format!("{e:#}"))] }
                }
                "mail.send" => b.mail_send(id, p).await,
                _ => vec![err(id, "unknown mail request")],
            }
        })
    }
}
pub fn register(hub: &Arc<Hub>, b: &Arc<Bots>) { hub.register_rpc(Arc::new(MailRpc(b.clone()))); }

/// What a room read copies under the lock: the conversation, the page's headers (newest first), whether headers remain, and the room's log lines
struct RoomSnap { c: Conv, page: Vec<Hdr>, more: bool, lines: Vec<usize> }
/// RoomHistory.page: the headers after the cursor
fn room_snapshot(w: &World, conv: &str, before: &str, n: usize) -> Result<RoomSnap, String> {
    let unknown = || format!("{conv}: unknown room conversation or mail cursor");
    let c = w.mail.conv(conv); if c.id.is_empty() { return Err(unknown()) }
    let hs = w.mail.headers(conv);
    let rest = if before.is_empty() { hs } else { &hs[hs.iter().position(|h| h.id == before).ok_or_else(unknown)? + 1..] };
    let page: Vec<Hdr> = rest.iter().take(n).cloned().collect(); let more = rest.len() > n;
    // which log lines hold a wanted text is the log's to say: the room's line numbers are copied (numbers, not text)
    Ok(RoomSnap { c, page, more, lines: w.room_lines.get(&conv[5..]).cloned().unwrap_or_default() })
}
type Read = (Vec<Value>, bool);
/// MailJob.room.*: each header's text from the room's RoomPosted line (a legacy post, searched newest first until all are found) or else from the journal;
/// a header whose text is in neither is an error, never a made-up body
fn read_room(store_: &crate::store::Store, home: &std::path::Path, conv: &str, s: RoomSnap) -> Result<Read, String> {
    let mut want: std::collections::HashSet<String> = s.page.iter().map(|h| h.id.clone()).collect();
    let mut texts: std::collections::HashMap<String, String> = Default::default();
    for line in s.lines.iter().rev() {
        if want.is_empty() { break }
        let Ok(raw) = store_.get(line - 1) else { continue };
        let Ok(e) = serde_json::from_str::<Value>(&raw) else { continue };
        let pid = e["id"].as_str().unwrap_or("");
        if want.remove(pid) { texts.insert(pid.to_string(), e["text"].as_str().unwrap_or("").to_string()); }
    }
    let from_journal = if want.is_empty() { Default::default() } else { store::exact(&store::path(home, conv), &want) };
    let mut mails = vec![];
    for h in &s.page {
        let text = match texts.get(&h.id) { Some(t) => t.clone(), None => match from_journal.get(&h.id) { Some(b) => b.text.clone(), None => return Err(format!("{conv}: mail body unavailable: {}", h.id)) } };
        mails.push(body_row(&Body { conv: s.c.clone(), mail: h.clone(), text }));
    }
    Ok((mails, s.more))
}
/// MailStore.read for a conversation that is no room's
fn read_plain(home: &std::path::Path, conv: &str, before: &str, n: usize) -> Result<Read, String> {
    let path = store::path(home, conv);
    let (bs, more) = match store::page(&path, before, n) {
        Ok(Some(p)) => p, Ok(None) => (vec![], false), // no journal yet: the conversation with no bodies
        Err(store::PageError::Cursor) => return Err(format!("{}: unknown mail cursor", path.display())),
        Err(store::PageError::Store(e)) => return Err(format!("{}: {e}", path.display())),
    };
    Ok((bs.iter().map(body_row).collect(), more))
}
