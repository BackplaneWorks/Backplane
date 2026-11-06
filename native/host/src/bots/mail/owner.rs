//! owner.mail: waiting mail for the uberbot's triage follows the conversation, not the local bot
//! replica. A machine that is not the holder of the uberbot's conversation sends the waiting input to the holder with the signed `owner.mail` request over /far/rpc;
//! the holder imports it (journal body, header, a ticket and a receipt) and answers "committed", or "moved" when it no longer holds the lease. Only journal
//! coordinates and the original header/conversation enter the log; bodies stay in the journal. A run claims its input with RunSet (the mail lane's), and an input
//! that is running is never copied to a second machine.
//! The transport is the net lane's: `OwnerLink` (a signed POST to a linked machine) is installed with `Bots::set_owner_link`; with none, the outbox waits.
//! The receiving side is `Bots::owner_received(link, request)`, which the /far/rpc route calls with the request's link and `{"m":"owner.mail","p":{..}}`.
use super::{body_line, body_parse, store, Body, Conv, Hdr};
use crate::{api::Fut, bots::{rpc::{now_s, Bots}, world::World}};
use serde_json::{json, Value};
use std::sync::Arc;

/// A signed request to a linked machine: (HTTP status, body) or None when it could not be made (no route, the link is not the owner's, a timeout).
/// The implementation must send only to a link that is one of the owner's machines (Far.own.id: the request carries a ticket and the mail's text) and must end
/// every call in bounded time (the walk waits for it; it also cuts a call off after `POST_TIMEOUT`).
pub trait OwnerLink: Send + Sync {
    fn post<'a>(&'a self, link: &'a str, body: String) -> Fut<'a, Option<(u16, String)>>;
}

/// longest a walk waits on one request
pub const POST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

fn gs<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn gn(v: &Value, k: &str) -> u64 { v.get(k).and_then(Value::as_u64).unwrap_or(0) }
fn show(v: &Value) -> String { serde_json::to_string(v).unwrap_or_default() }
fn set(k: &str, v: &str, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }

// ---- keys (Owner.global, Owner.key, Owner.*.key) ----
pub fn global(uid: &str, origin: &str, id: &str) -> String { format!("owner:{}", show(&json!([uid, origin, id]))) }
pub fn key(r: &Value) -> String { global(gs(r, "uid"), gs(r, "origin"), gs(&r["mail"], "id")) }
pub fn pending_key(k: &str) -> String { format!("owner.mail.pending.{k}") }
pub fn map_key(id: &str) -> String { format!("owner.mail.id.{id}") }
pub fn local_key(k: &str) -> String { format!("owner.mail.local.{k}") }
pub fn receipt_key(sender: &str, term: u64, k: &str) -> String { format!("owner.mail.receipt.{}", show(&json!([sender, term, k]))) }
fn hold_key(bot: &str, conv: &str) -> String { format!("triage.hold.{}", show(&json!([bot, conv]))) }
fn queue_key(bot: &str, conv: &str) -> String { format!("triage.queue.{}", show(&json!([bot, conv]))) }

/// Owner.register: the mapping from the mail's id, the local copy and the pending ticket
pub fn register(r: &Value, now: u64) -> Vec<Value> { let k = key(r); vec![set(&map_key(gs(r, "id")), &k, now), set(&local_key(&k), &show(r), now), set(&pending_key(&k), &show(r), now)] }

/// Owner.remote: this machine has an uberbot of the same conversation and is not its holder
pub fn remote(w: &World) -> bool { w.ub_on() && !w.holder_me() }
pub fn addr(w: &World) -> String { format!("bot:{}", w.uber_bot()) }

/// Owner.response / Owner.response.is: only this exact protocol and key acknowledges (an older hub may answer 200 with an unknown-method error)
pub fn response(r: &Value, state: &str) -> String { show(&json!({"owner_mail":state,"key":key(r)})) }
pub fn response_is(text: &str, r: &Value, state: &str) -> bool { serde_json::from_str::<Value>(text).map_or(false, |j| gs(&j, "owner_mail") == state && gs(&j, "key") == key(r)) }

/// Owner.attempt: the recipient is persisted before IO, so an ambiguous request is retried at that recipient
pub fn attempt(r: &Value, to: &str, term: u64, stamp: &str) -> Value {
    json!({"uid":r["uid"],"origin":r["origin"],"mail":r["mail"],"conversation":r["conversation"],"id":r["id"],"conv":r["conv"],"stamp":stamp,"to":to,"lease":term})
}

/// percent-decoding of a ref's segment (Ref.unpct): invalid escapes stay as they are
fn unpct(s: &str) -> String {
    let b = s.as_bytes(); let mut out = Vec::with_capacity(b.len()); let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() { if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) { out.push(v); i += 3; continue } }
        out.push(b[i]); i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
/// Ref.of_addr, as far as the owner needs: (kind, first id) of a checked address ("person" and anything malformed: None)
pub(super) fn ref_of(a: &str) -> Option<(String, String)> {
    if a == "person" { return None }
    let (k, tail) = a.split_once(':')?;
    if !["thread", "bot", "room", "project", "routine", "task", "pr", "work", "file", "machine", "conv", "mail", "mailroom", "decision", "charter"].contains(&k) { return None }
    let parts: Vec<&str> = tail.trim_end_matches('/').split('/').collect();
    if parts.iter().any(|p| p.is_empty()) { return None }
    let ok = match parts.len() { 1 => !["pr", "file", "mail", "decision"].contains(&k), 2 => ["pr", "file", "mail", "decision"].contains(&k), _ => k == "file" };
    if !ok { return None }
    Some((k.to_string(), unpct(parts[0])))
}

impl World {
    fn link_of(&self, machine: &str) -> String { self.ub.settings.get(&format!("uber.link.{}.{}", self.uid(), machine)).cloned().unwrap_or_default() }
    /// Owner.source.id: an id on the sending machine named so this one can tell it apart
    fn source_id(&self, origin: &str, id: &str) -> String {
        if id.is_empty() || id.contains('~') || origin == self.me() { return id.to_string() }
        let l = self.link_of(origin); format!("{}~{id}", if l.is_empty() { origin } else { &l })
    }
    fn source_addr(&self, origin: &str, fallback: &str) -> String { match ref_of(fallback) { Some((k, id)) => format!("{k}:{}", self.source_id(origin, &id)), None => fallback.to_string() } }
    /// Owner.import.mail / .conv: the ticket's mail and conversation as this machine names things (the owner is this machine's uberbot)
    fn import_mail(&self, m: &Hdr, origin: &str, id: &str, conv: &str) -> Hdr {
        let launchlike = ["thread", "launch", "work", "lock"].contains(&m.kind.as_str());
        Hdr { id: id.into(), conv: conv.into(), from: self.source_addr(origin, &m.from), to: addr(self), kind: m.kind.clone(), r#ref: if launchlike { self.source_id(origin, &m.r#ref) } else { m.r#ref.clone() },
            want: m.want.clone(), head: m.head.clone(), size: m.size, at: m.at }
    }
    fn import_conv(&self, c: &Conv, origin: &str, conv: &str, oldowner: &str) -> Conv {
        let mut parts: Vec<String> = c.parts.iter().map(|h| if oldowner.starts_with("bot:") && h == oldowner { addr(self) } else { self.source_addr(origin, h) }).collect();
        let own = addr(self); if !parts.contains(&own) { parts.insert(0, own) }
        Conv { id: conv.into(), subject: c.subject.clone(), kind: c.kind.clone(), parts, project: self.source_id(origin, &c.project), tags: c.tags.clone(), made: c.made, last: c.last, n: c.n }
    }
    /// Ub.link of the sender is the link the request arrived on
    fn owner_peer(&self, link: &str, p: &Value) -> bool { self.ub_on() && gs(p, "uid") == self.uid() && !link.is_empty() && self.link_of(gs(p, "sender")) == link }
    /// Owner.accept: the holder, at the lease term the sender used
    fn owner_accept(&self, link: &str, p: &Value) -> bool { self.ub_on() && self.holder_me() && gs(p, "uid") == self.uid() && gn(p, "term") == self.lease().1 && self.owner_peer(link, p) }
}

impl Bots {
    pub fn set_owner_link(&self, l: Arc<dyn OwnerLink>) { let _ = self.owner_link.set(l); }

    async fn setting_or_empty(&self, k: &str) -> String { self.hub.setting(k).await.unwrap_or_default() }
    /// Owner.current: the ticket is still the one pending (a stale worker cannot clear one imported back after a lease move)
    async fn owner_current(&self, r: &Value) -> bool { self.setting_or_empty(&pending_key(&key(r))).await == show(r) }
    fn owner_stamp(&self, w: &World, now: u64) -> String { format!("{now}:{}:{}", self.hub.store.len(), w.lease().1) }

    /// Owner.records: at most sixteen pending tickets, by key
    pub async fn owner_records(&self) -> Vec<Value> {
        let m = self.hub.model().await; let seqs: Vec<u32> = m.settings_with_prefix("owner.mail.pending.").map(|(_, s)| s).collect(); drop(m);
        let mut out = vec![];
        for s in seqs { if out.len() >= 16 { break }
            let Some(v) = self.hub.store.get(s as usize - 1).ok().and_then(|l| serde_json::from_str::<Value>(&l).ok()) else { continue };
            let Some(t) = v["value"].as_str().filter(|t| !t.is_empty()) else { continue };
            if let Ok(r) = serde_json::from_str::<Value>(t) { out.push(r) }
        }
        out
    }

    /// OwnerJob.records: each pending ticket is read back from the journal and either routed here, sent to its holder, or planned again.
    /// Nothing on a hub started on a copy of a log (Copy.on: OwnerJob.start.on), one walker at a time (Fh.busy), and every decision about a ticket is made and
    /// committed under the world lock: Owner.current is only meaningful if the check and the write are one step, as in actor.
    pub async fn owner_outbox(&self) {
        if crate::copy::on() { return }
        if self.owner_busy.swap(true, std::sync::atomic::Ordering::AcqRel) { return }
        struct Idle<'a>(&'a std::sync::atomic::AtomicBool); impl Drop for Idle<'_> { fn drop(&mut self) { self.0.store(false, std::sync::atomic::Ordering::Release) } }
        let _idle = Idle(&self.owner_busy);
        for r in self.owner_records().await {
            let (home, conv, id) = (self.hub.cfg.home.clone(), gs(&r, "conv").to_string(), gs(&r, "id").to_string());
            let ids: std::collections::HashSet<String> = [id].into();
            let b = tokio::task::spawn_blocking(move || store::exact(&store::path(&home, &conv), &ids).into_values().next()).await.ok().flatten();
            let Some(b) = b else { continue };
            let to = gs(&r, "to").to_string();
            if to.is_empty() {
                let w = self.world().await;
                if remote(&w) { let evs = self.owner_plan(&w, &r).await; self.owner_commit(evs).await }
                else if self.owner_current(&r).await { drop(w); if let Some(rt) = self.router.get() { rt.routed(&b) } } // Hub.owner.ready: Mail.route
                continue
            }
            let (link, req) = { let w = self.world().await; (w.link_of(&to), self.owner_request(&w, &r, &b).await) };
            let Some(l) = self.owner_link.get() else { continue };
            if link.is_empty() { continue }
            let ans = tokio::time::timeout(POST_TIMEOUT, l.post(&link, req)).await.ok().flatten();
            match ans {
                Some((200, body)) if response_is(&body, &r, "committed") => { let w = self.world().await; let evs = self.owner_sent(&w, &r).await; self.owner_commit(evs).await }
                Some((409, body)) if response_is(&body, &r, "moved") => { let w = self.world().await; let evs = self.owner_plan(&w, &r).await; self.owner_commit(evs).await }
                _ => {}
            }
        }
    }
    /// Owner.legacy (OwnerJob's start): every queued input of the uberbot has a ticket: one written before tickets existed gets its first, one a stop put back gets its own again
    pub async fn owner_legacy(&self) {
        if crate::copy::on() { return }
        let w = self.world().await; let (now, seq) = (now_s(), self.hub.store.len());
        let mut keys: Vec<&String> = w.settings.keys().filter(|k| k.starts_with("triage.queue.")).collect(); keys.sort(); keys.reverse();
        let mut evs = vec![];
        for k in keys {
            let Ok(parts) = serde_json::from_str::<Vec<String>>(&k[13..]) else { continue }; let [bot, conv] = parts.as_slice() else { continue };
            if !w.triage_owner(bot) { continue }
            let ids: Vec<String> = serde_json::from_str(&w.setting(k, "[]")).unwrap_or_default();
            for id in ids {
                let Some(m) = w.mail.headers(conv).iter().find(|h| h.id == id).cloned() else { continue };
                let key = w.setting(&map_key(&id), "");
                if key.is_empty() { evs.extend(self.owner_track_of(&w, &Body { conv: w.mail.conv(conv), mail: m, text: String::new() }).await) }
                else if w.setting(&pending_key(&key), "").is_empty() { // Owner.waiting.known
                    let prior: Value = serde_json::from_str(&w.setting(&local_key(&key), "")).unwrap_or(Value::Null);
                    evs.extend(register(&attempt(&prior, "", w.lease().1, &format!("{now}:{seq}")), now));
                }
            }
        }
        if !evs.is_empty() { self.owner_commit(evs).await }
    }
    /// run the outbox in the background (the mail lane calls this when it files mail that belongs to the owner's other machine)
    pub fn owner_kick(self: &Arc<Self>) { let me = self.clone(); tokio::spawn(async move { me.owner_outbox().await }); }
    async fn owner_commit(&self, evs: Vec<Value>) { if !evs.is_empty() { if let Err(e) = self.hub.commit_many(&evs).await { eprintln!("owner.mail: {e:#}") } } }

    /// Owner.plan: choose the recipient (the holder, for a machine that is not it) and persist it before IO
    async fn owner_plan(&self, w: &World, r: &Value) -> Vec<Value> {
        if !self.owner_current(r).await { return vec![] }
        let lease = w.lease(); let next = attempt(r, if remote(w) { &lease.0 } else { "" }, lease.1, &format!("{}:{}", now_s(), self.hub.store.len()));
        vec![set(&pending_key(&key(r)), &show(&next), now_s())]
    }
    /// Owner.sent: clear the ticket and take the input out of the triage queue
    async fn owner_sent(&self, w: &World, r: &Value) -> Vec<Value> {
        if !self.owner_current(r).await { return vec![] }
        let (bot, conv, now) = (w.uber_bot(), gs(r, "conv"), now_s());
        let q: Vec<String> = serde_json::from_str::<Vec<Value>>(&self.setting_or_empty(&queue_key(&bot, conv)).await).unwrap_or_default().iter().filter_map(|x| x.as_str().map(str::to_string)).filter(|x| x != gs(r, "id")).collect();
        vec![set(&pending_key(&key(r)), "", now), set(&queue_key(&bot, conv), &show(&json!(q)), now)]
    }
    /// Owner.request: the signed body (the transport adds its nonce)
    async fn owner_request(&self, w: &World, r: &Value, b: &Body) -> String {
        let body: Value = serde_json::from_str(body_line(b).trim_end()).unwrap_or(Value::Null);
        let hold = self.setting_or_empty(&hold_key(&w.uber_bot(), gs(r, "conv"))).await;
        let term = r.get("lease").and_then(Value::as_u64).unwrap_or_else(|| w.lease().1);
        show(&json!({"m":"owner.mail","p":{"sender":w.me(),"uid":w.uid(),"term":term,"ticket":r,"body":body,"hold":hold}}))
    }

    /// Owner.import.body: the mail as this machine's uberbot sees it, None when the request is not a valid ticket for it
    async fn owner_import(&self, w: &World, p: &Value) -> Option<Body> {
        let r = p.get("ticket")?; let k = key(r);
        let prior: Value = serde_json::from_str(&self.setting_or_empty(&local_key(&k)).await).unwrap_or(Value::Null);
        let origin = gs(r, "origin").to_string();
        let id = if gs(&prior, "id").is_empty() { k.clone() } else { gs(&prior, "id").to_string() };
        let conv = if gs(&prior, "conv").is_empty() { format!("owner:{}", show(&json!([gs(r, "uid"), origin, gs(&r["conversation"], "id")]))) } else { gs(&prior, "conv").to_string() };
        let b = body_parse(&show(p.get("body")?))?;
        let m = body_parse(&show(&json!({"v":1,"conv":r["conversation"],"mail":r["mail"],"text":""})))?; // the ticket's own mail and conversation, decoded the way a body's are
        if gs(r, "uid") != w.uid() || b.mail.id != gs(r, "id") || b.mail.conv != gs(r, "conv") { return None }
        Some(Body { conv: w.import_conv(&m.conv, &origin, &conv, &m.mail.to), mail: w.import_mail(&m.mail, &origin, &id, &conv), text: b.text })
    }
    /// Owner.import.changes: the ticket as imported, the receipt, and the sender's hold on the conversation
    fn owner_import_changes(&self, w: &World, p: &Value, b: &Body, now: u64) -> Vec<Value> {
        let r = &p["ticket"];
        let rec = json!({"uid":r["uid"],"origin":r["origin"],"mail":r["mail"],"conversation":r["conversation"],"id":b.mail.id,"conv":b.mail.conv,"stamp":self.owner_stamp(w, now)});
        let mut evs = register(&rec, now);
        evs.push(set(&receipt_key(gs(p, "sender"), gn(p, "term"), &key(&rec)), "committed", now));
        if !gs(p, "hold").is_empty() { evs.push(set(&hold_key(&w.uber_bot(), &b.mail.conv), gs(p, "hold"), now)) }
        evs
    }

    /// Hub.owner.received: the holder's side of the request. (status, body)
    pub async fn owner_received(&self, link: &str, o: &Value) -> (u16, String) {
        let p = &o["p"]; let w = self.world().await; let now = now_s();
        let r = p.get("ticket").cloned().unwrap_or(Value::Null);
        if !w.owner_peer(link, p) { return (403, "owner identity mismatch".into()) }
        let rk = receipt_key(gs(p, "sender"), gn(p, "term"), &key(&r));
        let seen = || async { self.setting_or_empty(&rk).await != "" };
        if seen().await { return (200, response(&r, "committed")) }
        if !w.owner_accept(link, p) { return (409, response(&r, "moved")) }
        let Some(b) = self.owner_import(&w, p).await else { return (400, "invalid owner mail".into()) };
        let changes = self.owner_import_changes(&w, p, &b, now);
        if w.mail.has(&b.mail.id) { // already filed here (an input that came back): only the ticket and receipt, and route it again
            if let Err(e) = self.hub.commit_many(&changes).await { eprintln!("owner.mail: {e:#}"); return (503, "mail not committed".into()) }
            drop(w); if let Some(rt) = self.router.get() { rt.routed(&b) }
        } else if let Err(e) = self.file_body_with(&w, b, Some(changes), vec![]).await { eprintln!("owner.mail: {e}"); return (503, "mail not committed".into()) } else { drop(w) }
        if seen().await { (200, response(&r, "committed")) } else { (503, "mail not committed".into()) }
    }
}

