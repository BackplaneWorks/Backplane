//! ML.Box: the mailroom folded from the log (ConvSet, MailPosted, MailMarked, MailTagged, a room's posts and members). Conversations, the headers of each
//! (newest first), the ids seen, and each bot's marks. Orderings and counts the requests need (slice 2) are computed from these on demand.
use super::{head, Conv, Hdr};
use std::collections::{HashMap, HashSet};

/// ML.Mark: one bot's state on a conversation
#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Mark { pub bot: String, pub conv: String, pub status: String, pub seen: u64, pub until: u64, pub tags: Vec<String> }

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Box {
    pub convs: HashMap<String, Conv>,
    /// a conversation's headers, newest first (Headers.add)
    pub mails: HashMap<String, Vec<Hdr>>,
    ids: HashSet<String>,
    /// "<bot>/<conv>" -> mark
    pub marks: HashMap<String, Mark>,
    /// Box.names: lower-cased bot name -> id, and id -> lower-cased name
    names: HashMap<String, String>,
}

fn parts_add(mut xs: Vec<String>, k: &str) -> Vec<String> { if !xs.iter().any(|x| x == k) { xs.insert(0, k.to_string()) } xs }
/// BT.Room.members.of: one entry per line, empty lines dropped
pub fn lines_of(s: &str) -> Vec<String> { s.split('\n').filter(|x| !x.is_empty()).map(str::to_string).collect() }
/// M.MailRoom.parts: its bots, then the person (the recursion ends in ["person"])
pub fn room_parts(members: &[String]) -> Vec<String> { members.iter().map(|m| format!("bot:{m}")).chain(std::iter::once("person".to_string())).collect() }

impl Box {
    pub fn conv(&self, id: &str) -> Conv { self.convs.get(id).cloned().unwrap_or_default() }
    pub fn has(&self, id: &str) -> bool { self.ids.contains(id) }
    pub fn headers(&self, conv: &str) -> &[Hdr] { self.mails.get(conv).map_or(&[], Vec::as_slice) }
    /// Box.newest
    pub fn newest(&self, conv: &str) -> Hdr { self.headers(conv).first().cloned().unwrap_or_default() }
    pub fn mark_of(&self, bot: &str, conv: &str) -> Mark { self.marks.get(&format!("{bot}/{conv}")).cloned().unwrap_or_default() }
    fn mark(&mut self, m: Mark) { self.marks.insert(format!("{}/{}", m.bot, m.conv), m); }
    pub fn bot_named(&mut self, id: &str, name: &str) { self.bot_deleted(id); let l = name.to_lowercase(); self.names.insert(format!("id:{id}"), l.clone()); self.names.insert(format!("name:{l}"), id.into()); }
    pub fn bot_deleted(&mut self, id: &str) { if let Some(n) = self.names.remove(&format!("id:{id}")) { self.names.remove(&format!("name:{n}")); } }
    /// Box.sender: the address a name in a room post stands for
    pub fn sender(&self, from: &str) -> String {
        if from == "you" || from == "person" { return "person".into() }
        match self.names.get(&format!("name:{}", from.to_lowercase())) { Some(id) => format!("bot:{id}"), None => if self.names.contains_key(&format!("id:{from}")) { format!("bot:{from}") } else { from.into() } }
    }

    /// Box.conv_put: store a conversation; a bot that left its parts loses its mark, a bot in them without one gets an open mark
    fn conv_put(&mut self, c: Conv) {
        let old = self.conv(&c.id);
        for p in &old.parts { if let Some(bot) = p.strip_prefix("bot:") { if !c.parts.contains(p) { self.marks.remove(&format!("{bot}/{}", old.id)); } } }
        self.convs.insert(c.id.clone(), c.clone());
        for p in &c.parts { if let Some(bot) = p.strip_prefix("bot:") { if self.mark_of(bot, &c.id).bot.is_empty() { self.mark(Mark { bot: bot.into(), conv: c.id.clone(), status: "open".into(), ..Default::default() }) } } }
    }
    /// Box.conv_set (ConvSet): a conversation's metadata; made and last start when it is new
    pub fn conv_set(&mut self, id: &str, subject: &str, kind: &str, parts: Vec<String>, project: &str, tags: Vec<String>, at: u64) {
        let old = self.conv(id); let new = old.id.is_empty();
        self.conv_put(Conv { id: id.into(), subject: subject.into(), kind: kind.into(), parts, project: project.into(), tags, made: if new { at } else { old.made }, last: if new { at } else { old.last }, n: old.n });
    }
    /// Box.mail (MailPosted): once per id; the conversation counts it and every bot in it but the sender has it unread
    pub fn mail(&mut self, m: Hdr) {
        if self.has(&m.id) { return }
        let old = self.conv(&m.conv); let missing = old.id.is_empty();
        let parts = if missing { parts_add(parts_add(old.parts.clone(), &m.from), &m.to) } else { old.parts.clone() };
        let c = Conv { id: m.conv.clone(), subject: if missing { m.head.clone() } else { old.subject.clone() }, kind: if missing { super::conv_mail_kind(&m.conv, &m.kind) } else { old.kind.clone() },
            parts, project: old.project.clone(), tags: old.tags.clone(), made: if missing { m.at } else { old.made }, last: m.at.max(old.last), n: old.n + 1 };
        self.conv_put(c.clone());
        let hs = self.mails.entry(m.conv.clone()).or_default();
        match hs.first() { None => hs.push(m.clone()), Some(h) => if m.at >= h.at { hs.insert(0, m.clone()) } else { hs.insert(1, m.clone()) } } // Headers.add: a mail not newer than the head goes second
        self.ids.insert(m.id.clone());
        for p in &c.parts { if let Some(bot) = p.strip_prefix("bot:") { if *p != m.from { let o = self.mark_of(bot, &c.id); self.mark(Mark { bot: bot.into(), conv: c.id.clone(), status: "unread".into(), seen: o.seen, until: 0, tags: o.tags }) } } }
    }
    /// Box.marked (MailMarked), Box.tagged (MailTagged)
    pub fn marked(&mut self, bot: &str, conv: &str, status: &str, seen: u64, until: u64) { let o = self.mark_of(bot, conv); self.mark(Mark { bot: bot.into(), conv: conv.into(), status: status.into(), seen, until, tags: o.tags }) }
    pub fn tagged(&mut self, bot: &str, conv: &str, tags: Vec<String>) { let o = self.mark_of(bot, conv); self.mark(Mark { bot: bot.into(), conv: conv.into(), status: o.status, seen: o.seen, until: o.until, tags }) }
    /// MailRoom.post: a room post is a mail of its room's conversation while the mailroom is off (with it on, Mail.room files it)
    pub fn room_post(&mut self, id: &str, room: &str, from: &str, text: &str, at: u64) {
        let cid = format!("room:{room}");
        if room.starts_with("dm:") && self.conv(&cid).id.is_empty() { // a direct room has no RoomSet: both participants once
            let ps: Vec<String> = room[3..].split(':').map(str::to_string).collect();
            let parts = if let [a, b] = ps.as_slice() { vec![self.sender(a), self.sender(b)] } else { vec![] };
            self.conv_set(&cid, room, "room", parts, "", vec![], at);
        }
        let s = self.sender(from);
        self.mail(Hdr { id: id.into(), conv: cid.clone(), from: s, to: cid, kind: "room".into(), r#ref: room.into(), want: "fyi".into(), head: head(text), size: text.chars().count() as u64, at });
    }
    /// RoomSet: the room's conversation
    pub fn room_set(&mut self, id: &str, name: &str, members: &str, at: u64) { self.conv_set(&format!("room:{id}"), name, "room", room_parts(&lines_of(members)), "", vec![], at) }
}
