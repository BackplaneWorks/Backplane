//! Filing (hub.bend `Mail.file`, `Mail.body`, `Mail.room.file`, `Mail.funnel`; mailowner.bend `Owner.track`): a mail's body is appended to its conversation's
//! journal, then its header events (and the owner ticket, for mail to the uberbot) are committed. Routing (waking triage and private threads) is slice 3:
//! `Router` is its seam. No text or address is logged here.
use super::{conv_event, header_event, store, Body, Conv, Hdr, Origin};
use crate::bots::{rpc::Bots, world::World};
use serde_json::{json, Value};
use std::path::PathBuf;

/// What happens after a mail is filed (Hub.mail.routed → B.Mail.route): triage runs and private threads. Slice 3.
pub trait Router: Send + Sync {
    fn routed(&self, body: &Body);
    /// the journal refused the body (B.Mail.failure: for mail to the uberbot, triage is told the delivery failed)
    fn failed(&self, body: &Body, why: store::StoreError);
}

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

impl Bots {
    pub(crate) async fn names_for(&self, w: &World) -> Names {
        let m = self.hub.model().await;
        Names { bots: w.bots.values().filter(|b| !b.deleted).map(|b| (b.id.clone(), b.name.clone())).collect(), threads: m.threads.iter().map(|(k, t)| (k.clone(), t.title.clone())).collect() }
    }
    pub(crate) async fn project_of(&self, w: &World, addr: &str) -> String {
        let th = if let Some(id) = addr.strip_prefix("thread:") { id.to_string() } else if let Some(id) = addr.strip_prefix("bot:") { w.bot(id).map(|b| b.thread.clone()).unwrap_or_default() } else { return String::new() };
        self.hub.model().await.threads.get(&th).map(|t| t.project.clone()).unwrap_or_default()
    }
    /// Mail.file: file a mail from `o` to `to`. Ok(false) when the id was filed already.
    pub async fn file_mail(&self, w: &World, now: u64, o: &Origin, to: &str, text: &str, msg: &str) -> Result<bool, String> { self.file_operational(w, now, o, to, text, msg, "", vec![]).await }
    /// Operational.mail: `want`: "fyi", "act", or "" for the kind's own; `after`: the producer's acknowledgements, committed with the header (alone when the header is there already)
    #[allow(clippy::too_many_arguments)]
    pub async fn file_operational(&self, w: &World, now: u64, o: &Origin, to: &str, text: &str, msg: &str, want: &str, after: Vec<Value>) -> Result<bool, String> { self.file_in(w, now, o, to, text, msg, want, after, "").await }
    /// `conv`: the conversation it joins, "" for Mail.route's
    #[allow(clippy::too_many_arguments)]
    pub async fn file_in(&self, w: &World, now: u64, o: &Origin, to: &str, text: &str, msg: &str, want: &str, after: Vec<Value>, conv: &str) -> Result<bool, String> {
        if w.mail.has(msg) { if !after.is_empty() { self.hub.commit_many(&after).await.map_err(|e| format!("{e:#}"))?; } return Ok(false) }
        let conv_id = if conv.is_empty() { super::route(o, to) } else { conv.to_string() }; let n = self.names_for(w).await;
        let subj = subject(o.kind == "launch", &n, o, to);
        let old = w.mail.conv(&conv_id);
        let conv = if old.id.is_empty() {
            let project = or(self.project_of(w, &o.from).await, self.project_of(w, to).await);
            Conv { id: conv_id.clone(), subject: subj, kind: super::conv_mail_kind(&conv_id, &o.kind), parts: vec![o.from.clone(), to.to_string()], project, tags: vec![], made: now, last: now, n: 0 }
        } else { old };
        let mail = Hdr { id: msg.into(), conv: conv_id, from: o.from.clone(), to: to.into(), kind: o.kind.clone(), r#ref: o.r#ref.clone(), want: super::want(&o.kind, want, &super::flags(o, text)), head: super::head(text), size: super::bytes(text), at: now };
        self.file_body_with(w, Body { conv, mail, text: text.into() }, None, after).await
    }
    /// Mail.room.file: the one mail a room post makes (the room's conversation is new in the body, whatever the box has)
    pub async fn file_room_mail(&self, w: &World, now: u64, from: &str, room: &str, text: &str, msg: &str, parts: Vec<String>, subject: &str) -> Result<bool, String> {
        let o = Origin::new(from, "room", room); let to = format!("room:{room}"); let conv = super::route(&o, &to);
        let c = Conv { id: conv.clone(), subject: subject.into(), kind: super::conv_mail_kind(&conv, "room"), parts, project: String::new(), tags: vec![], made: now, last: now, n: 0 };
        let mail = Hdr { id: msg.into(), conv, from: from.into(), to, kind: "room".into(), r#ref: room.into(), want: "fyi".into(), head: super::head(text), size: super::bytes(text), at: now };
        let body = Body { conv: c, mail, text: text.into() };
        // with the mailroom off the post's own RoomPosted already put its header in the box (MailRoom.post), so `has` is no sign it was filed: the uberbot in a room hears it only through this filing, and Box.mail ignores the repeat
        if w.setting("mail.on", "off") == "on" { self.file_body(w, body).await } else { self.file_new(w, body, None, vec![]).await }
    }
    /// Mail.file.body + MailJob.append: the journal first, then ConvSet (a new conversation), MailPosted and the owner ticket, in one commit
    pub async fn file_body(&self, w: &World, body: Body) -> Result<bool, String> { self.file_body_with(w, body, None, vec![]).await }
    /// `extra`: the changes that replace Owner.track's (an imported owner mail brings its own ticket and receipt); `after`: Operational.after's, last in the commit
    pub async fn file_body_with(&self, w: &World, body: Body, extra: Option<Vec<Value>>, after: Vec<Value>) -> Result<bool, String> {
        if w.mail.has(&body.mail.id) { return Ok(false) }
        self.file_new(w, body, extra, after).await
    }
    async fn file_new(&self, w: &World, body: Body, extra: Option<Vec<Value>>, after: Vec<Value>) -> Result<bool, String> {
        let home = self.hub.cfg.home.clone(); let b2 = body.clone();
        if let Err(e) = tokio::task::spawn_blocking(move || store::append(&home, &b2)).await.map_err(|e| e.to_string())? {
            // MailJob.failed: counted and logged (the kind only); the changes that led here stay committed, as in Bend; mail to the uberbot's triage is told by the router
            eprintln!("mail: append failed: {e}"); if let Some(r) = self.router.get() { r.failed(&body, e) } return Err(e.to_string())
        }
        let mut ev: Vec<Value> = vec![];
        if w.mail.conv(&body.conv.id).id.is_empty() { ev.push(conv_event(&body.conv)) }
        ev.push(header_event(&body.mail));
        match extra { Some(x) => ev.extend(x), None => ev.extend(self.owner_track(w, &body).await) }
        ev.extend(after);
        self.hub.commit_many(&ev).await.map_err(|e| format!("{e:#}"))?;
        if let Some(r) = self.router.get() { r.routed(&body) }
        Ok(true)
    }
    /// Owner.track: mail addressed to the uberbot's triage is recorded as a ticket (journal coordinates and header only, never the text)
    pub(super) async fn owner_track_of(&self, w: &World, b: &Body) -> Vec<Value> { self.owner_track(w, b).await }
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

impl Bots {
    /// MailStore.recover + Hub.mail.recovered: a body that reached the journal but whose header never reached the log (a crash between the two) gets its
    /// ConvSet (if the conversation is unknown), MailPosted and, for the uberbot's, an owner ticket now (the outbox walk routes it). 
    pub async fn mail_recover(&self) -> usize {
        if crate::copy::on() { return 0 } // it commits headers: not on a copy of a log
        let w = self.world().await; let dir = self.hub.cfg.home.join("mail");
        let mut names: Vec<PathBuf> = tokio::task::spawn_blocking(move || std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "jsonl")).collect()).unwrap_or_default()).await.unwrap_or_default();
        names.sort();
        let (mut ev, mut seen, mut convs): (Vec<Value>, std::collections::HashSet<String>, std::collections::HashSet<String>) = (vec![], Default::default(), Default::default());
        for p in names { // one journal at a time, one line at a time: no body text survives, only the headers the box lacks
            let hs = tokio::task::spawn_blocking(move || store::headers(&p)).await.unwrap_or_default();
            for (c, m) in hs {
                if w.mail.has(&m.id) || !seen.insert(m.id.clone()) { continue }
                if w.mail.conv(&c.id).id.is_empty() && convs.insert(c.id.clone()) { ev.push(conv_event(&c)) }
                ev.push(header_event(&m));
                let b = Body { conv: c, mail: m, text: String::new() }; ev.extend(self.owner_track(&w, &b).await); // Owner.recovered: a body that survived a torn commit is new input
            }
        }
        let n = ev.iter().filter(|e| e["$"] == "MailPosted").count();
        if n > 0 && self.hub.commit_many(&ev).await.is_err() { return 0 }
        n
    }
}
