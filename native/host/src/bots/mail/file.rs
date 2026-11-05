//! Filing (hub.bend `Mail.file`, `Mail.body`, `Mail.room.file`, `Mail.funnel`; mailowner.bend `Owner.track`): a mail's body is appended to its conversation's
//! journal, then its header events (and the owner ticket, for mail to the uberbot) are committed. Routing (waking triage and private threads) is slice 3:
//! `Router` is its seam. No text or address is logged here.
use super::{conv_event, header_event, store, Body, Conv, Hdr, Origin};
use crate::bots::{rpc::Bots, world::World};
use serde_json::{json, Value};

/// What happens after a mail is filed (Hub.mail.routed → B.Mail.route): triage runs and private threads. Slice 3.
pub trait Router: Send + Sync { fn routed(&self, body: &Body); }

/// ML.Mail.encode / ML.Conv.encode as JSON values (Body.mail.encode, Body.conv.encode)
pub fn mail_json(m: &Hdr) -> Value { json!({"id":m.id,"conv":m.conv,"from":m.from,"to":m.to,"kind":m.kind,"ref":m.r#ref,"want":m.want,"head":m.head,"size":m.size,"at":m.at}) }
pub fn conv_json(c: &Conv) -> Value { json!({"id":c.id,"subject":c.subject,"kind":c.kind,"parts":c.parts,"project":c.project,"tags":c.tags,"made":c.made,"last":c.last,"n":c.n}) }

/// What names an address in a conversation's subject: a bot's name, a thread's title, "Person", else the address
#[derive(Default)]
pub struct Names { pub bots: std::collections::HashMap<String, String>, pub threads: std::collections::HashMap<String, String> }
impl Names {
    /// Mail.name
    pub fn of(&self, addr: &str) -> String {
        if addr == "person" { return "Person".into() }
        if let Some(id) = addr.strip_prefix("bot:") { if let Some(n) = self.bots.get(id) { return n.clone() } }
        if let Some(id) = addr.strip_prefix("thread:") { if let Some(t) = self.threads.get(id) { return t.clone() } }
        addr.into()
    }
}
/// Mail.subject
pub fn subject(launch: bool, n: &Names, o: &Origin, to: &str) -> String { if launch { format!("Launch: {}", n.of(&o.from)) } else { format!("{} ↔ {}", n.of(&o.from), n.of(to)) } }
/// Mail.or
fn or(a: String, b: String) -> String { if a.is_empty() { b } else { a } }

impl World {
    /// Mail.addr: the address of the bot a thread belongs to, else of the thread
    pub fn mail_addr(&self, thread: &str) -> String { match self.bots.values().find(|b| !b.deleted && b.thread == thread) { Some(b) => format!("bot:{}", b.id), None => format!("thread:{thread}") } }
    /// Mail.sender: a post's sender as an address ("" is the person)
    pub fn mail_sender(&self, from: &str) -> String { if from.is_empty() { "person".into() } else { self.mail_addr(from) } }
}

/// Funnel (Mail.funnel): whether a message to `to` from this origin is delivered to the agent, filed as mail, or both
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Funnel { Deliver, File, Both }
/// `owned`: the uberbot's address and not from the person (always filed, never delivered); `mail_on`: the mailroom is on
pub fn funnel(w: &World, to: &str, o: &Origin) -> Funnel {
    let mail_on = w.setting("mail.on", "off") == "on"; let person = o.kind == "person";
    let ub = w.uber_bot();
    if !ub.is_empty() && to == format!("bot:{ub}") && !person { return Funnel::File }
    if mail_on && to.starts_with("bot:") && !person { return Funnel::File }
    if mail_on && !person { return Funnel::Both }
    Funnel::Deliver
}

impl Bots {
    pub(crate) async fn names_for(&self, w: &World) -> Names {
        let m = self.hub.model().await;
        Names { bots: w.bots.values().filter(|b| !b.deleted).map(|b| (b.id.clone(), b.name.clone())).collect(), threads: m.threads.iter().map(|(k, t)| (k.clone(), t.title.clone())).collect() }
    }
    async fn project_of(&self, w: &World, addr: &str) -> String {
        let th = if let Some(id) = addr.strip_prefix("thread:") { id.to_string() } else if let Some(id) = addr.strip_prefix("bot:") { w.bot(id).map(|b| b.thread.clone()).unwrap_or_default() } else { return String::new() };
        self.hub.model().await.threads.get(&th).map(|t| t.project.clone()).unwrap_or_default()
    }
    /// Mail.file: file a mail from `o` to `to`. Ok(false) when the id was filed already.
    pub async fn file_mail(&self, w: &World, now: u64, o: &Origin, to: &str, text: &str, msg: &str) -> Result<bool, String> {
        let conv_id = super::route(o, to); let n = self.names_for(w).await;
        let subj = subject(o.kind == "launch", &n, o, to);
        let old = w.mail.conv(&conv_id);
        let conv = if old.id.is_empty() {
            let project = or(self.project_of(w, &o.from).await, self.project_of(w, to).await);
            Conv { id: conv_id.clone(), subject: subj, kind: super::conv_mail_kind(&conv_id, &o.kind), parts: vec![o.from.clone(), to.to_string()], project, tags: vec![], made: now, last: now, n: 0 }
        } else { old };
        let mail = Hdr { id: msg.into(), conv: conv_id, from: o.from.clone(), to: to.into(), kind: o.kind.clone(), r#ref: o.r#ref.clone(), want: super::want(&o.kind, "", &super::flags(o, text)), head: super::head(text), size: super::bytes(text), at: now };
        self.file_body(w, Body { conv, mail, text: text.into() }).await
    }
    /// Mail.room.file: the one mail a room post makes (the room's conversation is new in the body, whatever the box has)
    pub async fn file_room_mail(&self, w: &World, now: u64, from: &str, room: &str, text: &str, msg: &str, parts: Vec<String>, subject: &str) -> Result<bool, String> {
        let o = Origin::new(from, "room", room); let to = format!("room:{room}"); let conv = super::route(&o, &to);
        let c = Conv { id: conv.clone(), subject: subject.into(), kind: super::conv_mail_kind(&conv, "room"), parts, project: String::new(), tags: vec![], made: now, last: now, n: 0 };
        let mail = Hdr { id: msg.into(), conv, from: from.into(), to, kind: "room".into(), r#ref: room.into(), want: "fyi".into(), head: super::head(text), size: super::bytes(text), at: now };
        self.file_body(w, Body { conv: c, mail, text: text.into() }).await
    }
    /// Mail.file.body + MailJob.append: the journal first, then ConvSet (a new conversation), MailPosted and the owner ticket, in one commit
    pub async fn file_body(&self, w: &World, body: Body) -> Result<bool, String> {
        if w.mail.has(&body.mail.id) { return Ok(false) }
        let home = self.hub.cfg.home.clone(); let b2 = body.clone();
        tokio::task::spawn_blocking(move || store::append(&home, &b2)).await.map_err(|e| e.to_string())??;
        let mut ev: Vec<Value> = vec![];
        if w.mail.conv(&body.conv.id).id.is_empty() { ev.push(conv_event(&body.conv)) }
        ev.push(header_event(&body.mail));
        ev.extend(self.owner_track(w, &body).await);
        self.hub.commit_many(&ev).await.map_err(|e| format!("{e:#}"))?;
        if let Some(r) = self.router.get() { r.routed(&body) }
        Ok(true)
    }
    /// Owner.track: mail addressed to the uberbot's triage is recorded as a ticket (journal coordinates and header only, never the text)
    async fn owner_track(&self, w: &World, b: &Body) -> Vec<Value> {
        let ub = w.uber_bot();
        if !w.triage_owner(&ub) { return vec![] }
        let addr = format!("bot:{ub}"); let m = &b.mail;
        let addressed = m.to == addr || b.conv.parts.iter().any(|p| *p == addr);
        let inbound = m.from != addr && (m.to == addr || m.to.starts_with("room:"));
        if !(addressed && inbound) || self.hub.setting(&format!("owner.mail.id.{}", m.id)).await.map_or(false, |v| !v.is_empty()) { return vec![] }
        let rec = json!({"uid":w.uid(),"origin":w.me(),"mail":mail_json(m),"conversation":conv_json(&b.conv),"id":m.id,"conv":m.conv,"stamp":format!("{}:{}:{}", m.at, self.hub.store.len(), w.lease().1)});
        let key = format!("owner:{}", json!([w.uid(), w.me(), m.id]));
        let r = rec.to_string();
        vec![json!({"$":"SettingSet","key":format!("owner.mail.id.{}", m.id),"value":key,"at":m.at}), json!({"$":"SettingSet","key":format!("owner.mail.local.{key}"),"value":r,"at":m.at}),
            json!({"$":"SettingSet","key":format!("owner.mail.pending.{key}"),"value":r,"at":m.at})]
    }
}
