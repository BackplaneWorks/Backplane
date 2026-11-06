//! The ledger tools of every thread's agent, a bot's or a plain thread's: told_check and told_mark (what the person was
//! already told) and task_board_* (the shared task board). Records are the settings `told.<id>` and `board.<id>`, read from the model's index of the log; a change is a
//! SettingSet committed through `World::also` like every tool's. order is kept where a list has ties: a family is read in reverse key order, then sorted stably.
use super::rpc::Bots;
use super::world::World;
use crate::state::fam::turnend::{trim_ws as trim, key_norm};
use serde_json::{json, Value};

pub const JSON: &str = include_str!("ledger.json");
pub const NAMES: &[&str] = &["told_check", "told_mark", "task_board_list", "task_board_add", "task_board_update", "task_board_remove", "task_board_import"];
pub fn defs() -> Vec<Value> { serde_json::from_str(JSON).unwrap_or_default() }

type Res = (bool, Value);
fn fail(m: &str) -> Res { (false, json!({"error":m})) }
fn gs<'a>(a: &'a Value, k: &str) -> &'a str { a.get(k).and_then(Value::as_str).unwrap_or("") }
/// J.Json.get_str(o, k, d)
fn gsd<'a>(a: &'a Value, k: &str, d: &'a str) -> &'a str { a.get(k).and_then(Value::as_str).unwrap_or(d) }
/// J.Json.get_nat: a number, or a string of digits
fn gnat(a: &Value, k: &str, d: u64) -> u64 { match a.get(k) { Some(Value::Number(n)) => n.as_u64().unwrap_or(d), Some(Value::String(s)) if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) => s.parse().unwrap_or(d), _ => d } }
fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }
fn drop_n(s: &str, n: usize) -> &str { s.char_indices().nth(n).map_or("", |(i, _)| &s[i..]) }
fn len(s: &str) -> usize { s.chars().count() }
fn low(s: &str) -> String { s.to_lowercase() }
/// Rules.pats: separated by commas and/or spaces
fn pats(s: &str) -> Vec<String> { s.split(|c: char| c == ',' || c == ' ').filter(|x| !x.is_empty()).map(str::to_string).collect() }

// ---- what the person was told ----

#[derive(Clone, Debug, PartialEq)]
pub struct Told { pub key: String, pub text: String, pub channel: String, pub bot: String, pub at: u64 }

/// Told.words.run: lower-cased runs of ASCII letters and digits, 3 or more long (a word stops growing at 64), at most `max` (distinct ones when asked)
pub fn words_run(s: &str, distinct: bool, max: usize) -> Vec<String> {
    let (mut out, mut cur, mut n): (Vec<String>, String, usize) = (vec![], String::new(), 0);
    let mut end = |cur: &mut String, n: &mut usize| { if *n >= 3 && out.len() < max && !(distinct && out.contains(cur)) { out.push(cur.clone()) } cur.clear(); *n = 0; };
    for c in s.chars() {
        let l = c.to_ascii_lowercase();
        if l.is_ascii_lowercase() || l.is_ascii_digit() { if n < 64 { cur.push(l) } n += 1 } else { end(&mut cur, &mut n) }
    }
    end(&mut cur, &mut n); out
}
pub fn words(s: &str) -> Vec<String> { words_run(s, true, 40) }
/// Told.key.of: the key made from a text: its first 8 words
pub fn key_of(s: &str) -> String { key_norm(&words_run(s, false, 8).join("-")) }
/// Told.key.pick: the key given, normalized; else made from the text; else the text itself
pub fn key_pick(key: &str, text: &str) -> String {
    if !trim(key).is_empty() { return key_norm(key) }
    let made = key_of(text); if made.is_empty() { key_norm(text) } else { made }
}
/// Told.same: the two word sets overlap enough to be one thing told twice: the one word of a set of one, else at least 2 and 60% of the smaller set
pub fn same(q: &[String], r: &[String]) -> bool {
    let (m, o) = (q.len().min(r.len()), q.iter().filter(|w| r.contains(w)).count());
    match m { 0 => false, 1 => o >= 1, _ => o >= 2 && o * 5 >= m * 3 }
}
/// Told.channel.ok
pub fn channel_ok(c: &str) -> String {
    let l = low(&take(trim(c), 20));
    if l.is_empty() { "chat".into() } else if ["alert", "digest", "room", "chat", "email", "other"].contains(&l.as_str()) { l } else { "other".into() }
}
/// Told.text.cut
fn text_cut(t: &str) -> String { trim(&take(trim(t), 280)).to_string() }
/// Told.days: default 7, at most 30
fn days(d: u64) -> u64 { if d == 0 { 7 } else { d.min(30) } }
/// Told.check.go over records newest first: it ends at a record older than `since` or at the tenth found
pub fn check<'a>(all: &'a [Told], now: u64, key: &str, text: &str, d: u64) -> Vec<&'a Told> {
    let (since, k, qw) = (now.saturating_sub(days(d) * 86400), key_norm(key), words(text)); let mut out = vec![];
    for r in all {
        if r.at < since || out.len() >= 10 { break }
        if (!k.is_empty() && r.key == k) || same(&qw, &words(&r.text)) { out.push(r) }
    }
    out
}
/// Ledger.ago.k
fn ago(now: u64, at: u64) -> String {
    let d = now.saturating_sub(at);
    if d < 60 { "just now".into() } else if d < 3600 { format!("{}m ago", d / 60) } else if d < 86400 { format!("{}h ago", d / 3600) } else { format!("{}d ago", d / 86400) }
}

// ---- the board ----

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Item { pub id: String, pub title: String, pub status: String, pub owner: String, pub project: String, pub due: String, pub group: String, pub links: Vec<String>, pub notes: String, pub at: u64, pub updated: u64, pub by: String }

/// Tb.status.norm: lower case; an unknown or an empty one is todo
pub fn status_norm(s: &str) -> String { let l = low(&take(trim(s), 20)); if ["doing", "blocked", "todo", "done", "dropped"].contains(&l.as_str()) { l } else { "todo".into() } }
/// Tb.rank: doing, blocked, todo, then done and dropped
fn rank(s: &str) -> u8 { match s { "doing" => 0, "blocked" => 1, "todo" => 2, "done" => 3, _ => 4 } }
fn digits(s: &str) -> bool { s.bytes().all(|c| c.is_ascii_digit()) }
/// Tb.date: YYYY-MM-DD, the month 1 to 12, the day 1 to 31
pub fn date(s: &str) -> bool {
    if len(s) != 10 || !s.is_ascii() { return false }
    let (y, m, d) = (&s[..4], &s[5..7], &s[8..]); let n = |x: &str| x.parse::<u32>().unwrap_or(0);
    digits(y) && digits(m) && digits(d) && &s[4..5] == "-" && &s[7..8] == "-" && (1..=12).contains(&n(m)) && (1..=31).contains(&n(d))
}
fn strs(v: Option<&Value>) -> Vec<String> { v.and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default() }
impl Item {
    /// Tb.of.stored
    pub fn stored(id: &str, o: &Value) -> Item {
        Item { id: id.into(), title: gs(o, "title").into(), status: status_norm(gsd(o, "status", "todo")), owner: gs(o, "owner").into(), project: gs(o, "project").into(), due: gs(o, "due").into(), group: gs(o, "group").into(),
            links: strs(o.get("links")), notes: gs(o, "notes").into(), at: gnat(o, "at", 0), updated: gnat(o, "updated", 0), by: gs(o, "by").into() }
    }
    /// Tb.raw
    pub fn raw(&self) -> Value { json!({"id":self.id,"title":self.title,"status":self.status,"owner":self.owner,"project":self.project,"due":self.due,"group":self.group,"links":self.links,"notes":self.notes,"at":self.at,"updated":self.updated,"by":self.by}) }
    fn set(&self) -> Value { json!({"$":"SettingSet","key":format!("board.{}",self.id),"value":self.raw().to_string(),"at":self.updated}) }
}
/// Tb.order: open before closed; within a status the earliest due first, none last, then the oldest first
pub fn order(xs: &mut [Item]) {
    let due = |d: &str| if d.is_empty() { "9999-99-99".to_string() } else { d.to_string() };
    xs.sort_by(|a, b| rank(&a.status).cmp(&rank(&b.status)).then_with(|| due(&a.due).cmp(&due(&b.due))).then_with(|| a.at.cmp(&b.at)));
}
/// Tb.owner.is: "person" is the person's own
fn owner_is(want: &str, have: &str) -> bool { want.is_empty() || want == have || (want == "person" && have.is_empty()) }
/// Tb.filter: empty means any; the owner and the project are ids; `status` may list several
pub fn filter(xs: Vec<Item>, status: &str, owner: &str, project: &str, group: &str) -> Vec<Item> {
    let sts: Vec<String> = pats(status).iter().map(|s| status_norm(s)).collect();
    xs.into_iter().filter(|t| (sts.is_empty() || sts.contains(&t.status)) && owner_is(owner, &t.owner) && (project.is_empty() || project == t.project) && (group.is_empty() || group == t.group)).collect()
}
/// Tb.clean: blanks dropped, each cut to 500 characters, at most 20
fn clean(xs: Vec<String>) -> Vec<String> { xs.iter().filter(|h| !trim(h).is_empty()).take(20).map(|h| take(trim(h), 500)).collect() }
/// Tb.links.of: given as an array, or as a string split on spaces and commas
fn links_of(v: Option<&Value>, old: &[String]) -> Vec<String> {
    match v { None => old.to_vec(), Some(j) => { let text = j.as_str().unwrap_or(""); clean(if text.is_empty() { strs(Some(j)) } else { pats(text) }) } }
}
fn notes_add(base: &str, extra: &str) -> String { if extra.is_empty() { base.into() } else if base.is_empty() { extra.into() } else { format!("{base}\n{extra}") } }
/// Rules.err.first
fn first(es: &[&str]) -> String { es.iter().find(|e| !e.is_empty()).map_or(String::new(), |e| e.to_string()) }
/// RS: a name resolved to an id, or why not
type Rs = (String, String);
/// Rules.bot.res: a bot given by name (any case) or id
fn bot_res(w: &World, name: &str) -> Rs {
    if name.is_empty() { return Default::default() }
    match w.named(name).or_else(|| w.bot(name).filter(|b| !b.deleted)) { Some(b) => (b.id.clone(), String::new()), None => (String::new(), format!("no bot \"{name}\" here")) }
}
/// Rules.proj.find: a live project by id, or by name in any case
pub fn proj_find(w: &World, name: &str) -> Option<String> { let l = low(name); w.projects.iter().find(|(id, p)| !p.2 && (id.as_str() == name || low(&p.0) == l)).map(|(id, _)| id.clone()) }
fn proj_res(w: &World, name: &str) -> Rs {
    if name.is_empty() { return Default::default() }
    match proj_find(w, name) { Some(i) => (i, String::new()), None => (String::new(), format!("no project \"{name}\"")) }
}
/// Tb.owner.res: the person's own, or a bot by name or id
fn owner_res(w: &World, name: &str) -> Rs { if low(name) == "person" { Default::default() } else { bot_res(w, name) } }
/// Ledger.own: the owner a list asks for: a bot, me, person, or any
fn own(w: &World, bot: &str, o: &str) -> Rs {
    let o = trim(o);
    match low(o).as_str() {
        "me" => if bot.is_empty() { (String::new(), "you are not a bot, so there is no \"me\"".into()) } else { (bot.into(), String::new()) },
        "person" => ("person".into(), String::new()),
        _ => bot_res(w, o),
    }
}
/// Ledger.me: "me" in a request's owner is the calling bot (the person where there is none)
fn me(a: &Value, bot: &str) -> Value {
    let mut o = a.clone();
    if low(trim(gs(a, "owner"))) == "me" { if let Some(m) = o.as_object_mut() { m.insert("owner".into(), json!(if bot.is_empty() { "person" } else { bot })); } }
    o
}
/// Tb.make: the item that results from the request on `old`: the fields given replace the old ones
pub fn make(w: &World, now: u64, by: &str, o: &Value, old: &Item) -> Result<Item, String> {
    let title = trim(gsd(o, "title", &old.title)).to_string(); let due = trim(gsd(o, "due", &old.due)).to_string();
    let notes = notes_add(trim(gsd(o, "notes", &old.notes)), trim(gs(o, "notes_append")));
    let ow = if o.get("owner").is_some() { owner_res(w, trim(gs(o, "owner"))) } else { (old.owner.clone(), String::new()) };
    let pr = if o.get("project").is_some() { proj_res(w, trim(gs(o, "project"))) } else { (old.project.clone(), String::new()) };
    let title_err = if title.is_empty() { "an item needs a title" } else if len(&title) > 300 { "a title is at most 300 characters" } else { "" };
    let due_err = if due.is_empty() || date(&due) { "" } else { "a due date is YYYY-MM-DD" };
    let notes_err = if len(&notes) > 4000 { "notes are at most 4000 characters" } else { "" };
    let err = first(&[title_err, due_err, notes_err, &ow.1, &pr.1]);
    if !err.is_empty() { return Err(err) }
    Ok(Item { id: old.id.clone(), title, status: status_norm(gsd(o, "status", &old.status)), owner: ow.0, project: pr.0, due, group: take(trim(gsd(o, "group", &old.group)), 80), links: links_of(o.get("links"), &old.links), notes, at: old.at, updated: now, by: by.into() })
}

// ---- a markdown list ----

/// Imp: one item read from a list
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Imp { pub title: String, pub status: String, pub due: String, pub group: String, pub project: String, pub notes: String }
fn lead(s: &str, f: impl Fn(char) -> bool) -> usize { s.chars().take_while(|c| f(*c)).count() }
/// Tb.head: 1 to 6 # and then a space (or the end)
fn head(rest: &str) -> bool { let n = lead(rest, |c| c == '#'); let after = drop_n(rest, n); (1..=6).contains(&n) && (after.is_empty() || after.starts_with(' ')) }
fn head_text(rest: &str) -> &str { trim(drop_n(rest, lead(rest, |c| c == '#'))) }
/// Tb.bullet: the characters before a checkbox: "- ", "* ", "+ " or "1. "
fn bullet(rest: &str) -> usize {
    if ["- ", "* ", "+ "].contains(&take(rest, 2).as_str()) { return 2 }
    let d = lead(rest, |c| c.is_ascii_digit()); if d > 0 && take(drop_n(rest, d), 2) == ". " { d + 2 } else { 0 }
}
/// Tb.mark: the status a checkbox says, "" if it is none
fn mark(b: &str) -> &'static str { match b { "[ ]" => "todo", "[x]" | "[X]" => "done", "[~]" | "[/]" => "doing", "[!]" => "blocked", "[-]" => "dropped", _ => "" } }
/// Tb.due.split: a title with a due date at its end: " (due YYYY-MM-DD)" or " due:YYYY-MM-DD"
fn due_split(t: &str) -> (String, String) {
    let n = len(t); let cut = |tail: &str| trim(&take(t, n - len(tail))).to_string();
    let a = drop_n(t, n.saturating_sub(17));
    if n >= 17 && a.starts_with(" (due ") && a.ends_with(')') && date(&take(drop_n(a, 6), 10)) { return (cut(a), take(drop_n(a, 6), 10)) }
    let b = drop_n(t, n.saturating_sub(15));
    if n >= 15 && b.starts_with(" due:") && date(drop_n(b, 5)) { return (cut(b), drop_n(b, 5).to_string()) }
    (t.to_string(), String::new())
}
/// Tb.item.of: the checkbox item a trimmed line is, if it is one
pub fn item_of(rest: &str) -> Option<Imp> {
    let n = bullet(rest); if n == 0 { return None }
    let r1 = drop_n(rest, n).trim_start_matches(|c: char| c == ' ' || ('\t'..='\r').contains(&c));
    let status = mark(&take(r1, 3)); let after = drop_n(r1, 3);
    if status.is_empty() || !(len(r1) == 3 || after.starts_with(' ')) { return None }
    let (title, due) = due_split(trim(after)); if title.is_empty() { return None }
    Some(Imp { title: take(&title, 300), status: status.into(), due, ..Default::default() })
}
/// Tb.norm: lower case, single spaces: two titles are the same when these match
pub fn title_norm(s: &str) -> String { s.split(|c| matches!(c, ' ' | '\t' | '\n' | '\r')).filter(|x| !x.is_empty()).map(str::to_ascii_lowercase).collect::<Vec<_>>().join(" ") }
fn dupkey(g: &str, t: &str) -> String { format!("{}\n{}", title_norm(g), title_norm(t)) }
/// Tb.imp.go: the items of a list and the lines it skipped (at most 50, in order): `#` headings set the group (one that names a project also the project, unless the call
/// fixed one), checkbox items become items, indented lines under an item add to its notes, anything else is skipped
pub fn parse(md: &str, w: &World, init: &str, fixed: bool) -> (Vec<Imp>, Vec<String>) {
    let (mut g, mut p, mut can) = (String::new(), init.to_string(), false); let (mut items, mut skipped): (Vec<Imp>, Vec<String>) = (vec![], vec![]);
    for line in md.split('\n') {
        let (ind, rest) = (lead(line, |c| c == ' ' || c == '\t'), trim(line));
        if rest.is_empty() { continue }
        if let Some(i) = item_of(rest) { can = true; items.push(Imp { group: g.clone(), project: p.clone(), ..i }); continue }
        if ind == 0 && head(rest) { g = head_text(rest).to_string(); if !fixed { p = proj_find(w, &g).unwrap_or_else(|| init.to_string()) } can = false; continue }
        if ind >= 2 && can { if let Some(i) = items.last_mut() { i.notes = take(&notes_add(&i.notes, rest), 4000) } continue }
        can = false; if skipped.len() < 50 { skipped.push(rest.to_string()) }
    }
    (items, skipped)
}

impl Bots {
    /// the values of a family of settings, by key: a removed record has no value in the model's index
    async fn family(&self, pre: &str) -> Vec<(String, String)> {
        let lines: Vec<(String, u32)> = { let m = self.hub.model().await; m.settings_with_prefix(pre).map(|(k, s)| (k.to_string(), s)).collect() };
        lines.into_iter().filter_map(|(k, s)| { let v: Value = serde_json::from_str(&self.hub.store.get(s as usize - 1).ok()?).ok()?; Some((k, v["value"].as_str()?.to_string())) }).collect()
    }
    /// Told.all: every record, newest first
    pub(super) async fn told_all(&self) -> Vec<Told> {
        let mut v: Vec<Told> = self.family("told.").await.iter().rev().filter_map(|(_, x)| { let o: Value = serde_json::from_str(x).ok()?;
            Some(Told { key: gs(&o, "key").into(), text: gs(&o, "text").into(), channel: gsd(&o, "channel", "other").into(), bot: gs(&o, "bot").into(), at: gnat(&o, "at", 0) }) }).collect();
        v.sort_by(|a, b| b.at.cmp(&a.at)); v
    }
    /// Tb.all: every item (Tb.order sorts them)
    pub(super) async fn board_all(&self) -> Vec<Item> {
        self.family("board.").await.iter().rev().filter_map(|(k, x)| { let o: Value = serde_json::from_str(x).ok()?; Some(Item::stored(k.strip_prefix("board.")?, &o)) }).collect()
    }
    fn tb_json(w: &World, t: &Item) -> Value {
        let mut o = serde_json::Map::new(); o.insert("ownerName".into(), json!(Self::owner_name(w, &t.owner))); o.insert("projectName".into(), json!(Self::proj_name(w, &t.project)));
        if let Value::Object(r) = t.raw() { o.extend(r) } Value::Object(o)
    }
    /// Tb.owner.name
    fn owner_name(w: &World, id: &str) -> String { if id.is_empty() { "person".into() } else { w.bot(id).map_or(String::new(), |b| b.name.clone()) } }
    /// Tb.proj.name
    fn proj_name(w: &World, id: &str) -> String { if id.is_empty() { String::new() } else { w.projects.get(id).filter(|p| !p.2).map_or(id.to_string(), |p| p.0.clone()) } }
    /// Tb.text.one: [status] title (id; @owner; %project; due D; group)
    fn tb_line(w: &World, t: &Item) -> String {
        let owner = if t.owner.is_empty() { String::new() } else { format!("@{}", Self::owner_name(w, &t.owner)) };
        let proj = Self::proj_name(w, &t.project); let proj = if proj.is_empty() { proj } else { format!("%{proj}") };
        let due = if t.due.is_empty() { String::new() } else { format!("due {}", t.due) };
        let parts: Vec<&str> = [t.id.as_str(), &owner, &proj, &due, &t.group].into_iter().filter(|x| !x.is_empty()).collect();
        format!("[{}] {} ({})", t.status, t.title, parts.join("; "))
    }

    /// One tools/call of a ledger tool from the agent of `thread`, a bot's or not (Ledger.call)
    pub(super) async fn ledger_call(&self, w: &World, thread: &str, name: &str, a: &Value, now: u64) -> Res {
        let bot = w.bots.values().find(|b| !b.deleted && b.thread == thread).map_or(String::new(), |b| b.id.clone());
        let by = if bot.is_empty() { format!("thread:{thread}") } else { bot.clone() };
        let id = trim(gs(a, "id")).to_string();
        match name {
            "told_check" => {
                if trim(gs(a, "key")).is_empty() && trim(gs(a, "text")).is_empty() { return fail("give a key or a text to check") }
                let all = self.told_all().await;
                let rows: Vec<Value> = check(&all, now, gs(a, "key"), gs(a, "text"), gnat(a, "days", 0)).into_iter().map(|t| {
                    let name = if t.bot.is_empty() { String::new() } else { w.bot(&t.bot).map_or(String::new(), |b| b.name.clone()) };
                    json!({"key":t.key,"text":t.text,"channel":t.channel,"bot":name,"at":t.at,"ago":ago(now, t.at)}) }).collect();
                (true, json!({"told":!rows.is_empty(),"matches":rows}))
            }
            "told_mark" => {
                let (key, text) = (gs(a, "key"), gs(a, "text"));
                if trim(text).is_empty() { return fail("a text is needed") }
                let k = key_pick(key, text); let rid = self.hub_id("k", now);
                let rec = json!({"key":k,"text":text_cut(text),"channel":channel_ok(gs(a, "channel")),"bot":bot,"at":now});
                self.ledger_save(w, now, vec![json!({"$":"SettingSet","key":format!("told.{rid}"),"value":rec.to_string(),"at":now})], json!({"key":k})).await
            }
            "task_board_list" => {
                let (ow, pr) = (own(w, &bot, gs(a, "owner")), proj_res(w, trim(gs(a, "project"))));
                let err = first(&[&ow.1, &pr.1]); if !err.is_empty() { return fail(&err) }
                let status = trim(gs(a, "status")); let all = a.get("all").and_then(Value::as_bool).unwrap_or(false);
                let mut xs = filter(self.board_all().await, status, &ow.0, &pr.0, trim(gs(a, "group")));
                if status.is_empty() && !all { xs.retain(|t| rank(&t.status) < 3) } // Tb.open
                order(&mut xs);
                (true, json!({"text":xs.iter().map(|t| Self::tb_line(w, t)).collect::<Vec<_>>().join("\n"),"items":xs.iter().map(|t| Self::tb_json(w, t)).collect::<Vec<_>>()}))
            }
            "task_board_add" => {
                let old = Item { id: self.hub_id("b", now), status: "todo".into(), at: now, updated: now, ..Default::default() }; // Tb.blank
                match make(w, now, &by, &me(a, &bot), &old) { Ok(t) => self.ledger_save(w, now, vec![t.set()], json!({"id":old.id})).await, Err(e) => fail(&e) }
            }
            "task_board_update" => {
                if id.is_empty() { return fail("an id is needed") }
                let Some(old) = self.board_all().await.into_iter().find(|t| t.id == id) else { return fail(&format!("no item {id}")) };
                match make(w, now, &by, &me(a, &bot), &old) { Ok(t) => self.ledger_save(w, now, vec![t.set()], json!({"id":id})).await, Err(e) => fail(&e) }
            }
            "task_board_remove" => {
                if id.is_empty() { return fail("an id is needed") }
                if !self.board_all().await.iter().any(|t| t.id == id) { return fail(&format!("no item {id}")) }
                self.ledger_save(w, now, vec![json!({"$":"SettingSet","key":format!("board.{id}"),"value":"","at":now})], json!({"id":id})).await
            }
            "task_board_import" => self.tb_import(w, now, &by, &bot, a).await,
            _ => fail("unknown tool"),
        }
    }
    async fn ledger_save(&self, w: &World, now: u64, evs: Vec<Value>, body: Value) -> Res { match self.hub.commit_many(&w.also(now, evs)).await { Ok(_) => (true, body), Err(e) => fail(&format!("{e:#}")) } }

    /// Tb.import: an item whose group and title are already on the board, or earlier in the list, is not added again
    async fn tb_import(&self, w: &World, now: u64, by: &str, bot: &str, a: &Value) -> Res {
        let md = gs(a, "markdown"); if trim(md).is_empty() { return fail("the markdown list is needed") }
        let project = trim(gs(a, "project")); let o = me(a, bot);
        let (pr, ow) = (proj_res(w, project), owner_res(w, trim(gs(&o, "owner"))));
        let err = first(&[&pr.1, &ow.1]); if !err.is_empty() { return fail(&err) }
        let (items, skipped) = parse(md, w, &pr.0, !project.is_empty());
        let mut seen: Vec<String> = self.board_all().await.iter().map(|t| dupkey(&t.group, &t.title)).collect();
        let base = self.hub_id("b", now); let (mut evs, mut added, mut dups) = (vec![], 0u64, 0u64);
        for (idx, i) in items.into_iter().enumerate() {
            let t = Item { id: format!("{base}-{idx}"), title: i.title, status: i.status, owner: ow.0.clone(), project: i.project, due: i.due, group: take(&i.group, 80), links: vec![], notes: take(&i.notes, 4000), at: now, updated: now, by: by.into() };
            let k = dupkey(&t.group, &t.title);
            if seen.contains(&k) { dups += 1 } else { seen.push(k); evs.push(t.set()); added += 1 }
        }
        self.ledger_save(w, now, evs, json!({"added":added,"duplicates":dups,"skipped":skipped})).await
    }
}
