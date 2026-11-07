//! The composer's actions in Rust, as pure functions over the request's session (serde Values, no
//! Session type), returning the patch to merge into the session and the commands for the client. `send` always answers: what cannot go says why (`say`) and the draft stays.
//! Ported: `draft`, `send` to a thread or a bot's, attachments, replies, `/btw` `/give` `/secret` `/intent`, the card's Edit (`fab-edit`), a bounded draft's refusal.
//! Commands beyond the hub's own (`send`): `compose` puts text in the composer, `say` shows one line (`tone` "note": not an error).
use serde_json::{json, Map, Value};

#[path = "mention.rs"] pub mod mention;

pub struct ActOut { pub session_patch: Value, pub cmds: Vec<Value> }

/// Att.nonblank: a thread's attachments, one path per line, blank lines gone.
pub fn att_paths(text: &str) -> Vec<String> { text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect() }

/// Rto.block: a reply is kept in the person's message itself, ahead of what they wrote: the quote is counted, never searched for.
pub fn reply_block(id: &str, quote: &str) -> String {
    let q: String = quote.chars().take(2000).collect();
    format!("<in_reply_to msg={id} chars={}>\n{q}\n</in_reply_to>", q.chars().count())
}
/// Rto.of: a message text split into (the message replied to, its quote, the body); None when it has no such block.
pub fn reply_split(text: &str) -> Option<(String, String, String)> {
    let rest = text.strip_prefix("<in_reply_to msg=")?;
    let (id, rest) = rest.split_once(" chars=")?;
    let (n, rest) = rest.split_once(">\n")?;
    let n: usize = n.parse().ok()?;
    let quote: String = rest.chars().take(n).collect();
    if quote.chars().count() != n { return None }
    let after = rest[quote.len()..].strip_prefix("\n</in_reply_to>")?;
    let body = after.strip_prefix("\n\n").unwrap_or_else(|| after.trim_start_matches('\n'));
    Some((id.to_string(), quote, body.to_string()))
}

/// Draft.safe: the thread ids a draft may be kept for (not a scratch key).
fn safe(id: &str) -> bool { !id.is_empty() && !id.starts_with('@') }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn draft_of<'a>(session: &'a Value, key: &str) -> &'a str { session["drafts"][key].as_str().unwrap_or("") }

/// A word typed with a sigil or `/` is a popup in play: the helper's Refs.want / Skills.want decide what that asks for.
#[allow(dead_code)] fn popup_word(text: &str) -> bool { matches!(super::slash::token(text).chars().next(), Some('/' | '$' | '@' | '>' | '%' | '#')) }

/// Ui.act "draft": the selected thread's draft set, the host asked to keep it (Draft.keep).
pub fn draft(hub: &str, session: &Value, value: &str) -> Option<ActOut> {
    let sel = s(session, "sel");
    // A word with a sigil (`@ > % # $ /`) is only text here: its popup rows came from helper (refs/skills fetch), which the GPUI hub does not run; asking it answered
    // PRESENTER_UNAVAILABLE on every keystroke. The slash commands are still offered (fields), the draft is kept as typed.
    let mut cmds = vec![];
    if safe(sel) { cmds.push(json!({"type": "keep", "thread": format!("{hub}|{sel}"), "text": value})) }
    Some(ActOut { session_patch: json!({"drafts": { sel: value }}), cmds })
}

/// The facts of the thread and the setting a send needs, read from the model by the caller.
pub struct SendCtx<'a> { pub hub: &'a str, pub cid: &'a str, pub now_s: u64, pub bot_thread: bool, pub bot: Option<String>, pub binding: Option<String> }

/// Accounts.rpc.params: a request about a thread bound to a named account carries that binding's setting text first (`accountBinding`), any other copy of the key dropped.
pub fn bind(p: Value, binding: Option<&str>) -> Value {
    let Some(b) = binding else { return p };
    let Value::Object(o) = p else { return p };
    if !o.contains_key("thread") { return Value::Object(o) }
    let mut out = Map::new(); out.insert("accountBinding".into(), json!(b));
    for (k, v) in o { if k != "accountBinding" { out.insert(k, v); } }
    Value::Object(out)
}

/// Ui.msg_id: "c" ++ cid ++ now ++ "-" ++ rid.
pub fn msg_id(cid: &str, now_s: u64, rid: u64) -> String { format!("c{cid}{now_s}-{rid}") }

/// One line for the person; nothing else changes, so the draft stays as it is. Every refusal goes this way: a request the hub cannot run is never the person's only answer.
fn say(text: &str) -> ActOut { ActOut { session_patch: json!({}), cmds: vec![json!({"type": "say", "text": text})] } }
/// What the composer holds now: `keep` for the kept copy, `compose` for the box itself.
fn compose(hub: &str, sel: &str, text: &str) -> Vec<Value> { vec![json!({"type": "keep", "thread": format!("{hub}|{sel}"), "text": text}), json!({"type": "compose", "thread": sel, "text": text})] }
/// The composer's own commands, none of them ever a message: `(word, the Send button's label, what it does)`. The clients match a draft against these (`command_is`).
pub const WORDS: [(&str, &str, &str); 4] = [("/btw", "Ask", super::slash::OWN[0].1), ("/give", "Give", super::slash::OWN[1].1), ("/secret", "Send", super::slash::OWN[2].1), ("/intent", "Save", "Save this project's intent")];
/// The same table as data, for the phones (`hello.commands`) and the desktop (`thread.commands`).
pub fn words() -> Value { Value::Array(WORDS.iter().map(|(w, l, d)| json!({"word": w, "label": l, "desc": d})).collect()) }
/// A draft read as one of the composer's commands, and what follows the word.
#[derive(Debug, PartialEq)] pub enum Word { Btw(String), Give, Secret(String), Intent(String) }
pub fn word_of(draft: &str) -> Option<Word> {
    let rest = |w: &str| draft.trim().get(w.len()..).unwrap_or("").trim().to_string();
    // Sbox.is, Intent.is, Give.is, Btw.is, in the order Ui.send asks them
    if command_is(draft, "/secret") { return Some(Word::Secret(super::slash::token(&rest("/secret")).to_string())) }
    if command_is(draft, "/intent") { return Some(Word::Intent(rest("/intent"))) }
    if command_is(draft, "/give") { return Some(Word::Give) }
    if command_is(draft, "/btw") { return Some(Word::Btw(rest("/btw"))) }
    None
}
/// Bound.refuse: a draft this view holds only bounded is never sent; `marker` starts with its size in bytes. A message's bound is 128 KB.
fn bound_says(marker: &str) -> String {
    let n: u64 = marker.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0);
    format!("This draft is {} KB. A message can be at most 128 KB.", n.div_ceil(1024))
}
pub const BTW_EMPTY: &str = "Type your question after /btw.";
/// Intent.edit: what Edit puts in the composer: `/intent` and the text to change, the template for a project with none.
pub fn intent_edit(intent: &str) -> String { format!("/intent\n{}", if intent.trim().is_empty() { "# Goal\n\n# Parts\n\n# Notes\n" } else { intent }) }

/// Intent.borrow, Intent.borrowed: Edit borrows the composer. The conversation's draft is kept once, also when Edit is pressed again before Save.
fn intent_borrow(hub: &str, session: &Value, sel: &str) -> ActOut {
    let d = &session["drafts"]; let current = draft_of(session, sel);
    let text = intent_edit(s(&session["info"], &format!("fab.intent:{sel}")));
    let mut drafts = Map::new(); drafts.insert(sel.to_string(), json!(text));
    if !(s(d, &format!("@intent.edit:{sel}")) == "1" && command_is(current, "/intent")) {
        // a bare `/intent` typed to get here is the command, not a draft to come back to
        drafts.insert(format!("@intent.draft:{sel}"), json!(if word_of(current).is_some() { "" } else { current })); drafts.insert(format!("@intent.edit:{sel}"), json!("1"));
    }
    let mut cmds = compose(hub, sel, &text); cmds.push(json!({"type": "focus", "id": "composer"}));
    ActOut { session_patch: json!({"drafts": drafts}), cmds }
}

/// A command of the composer run: the side question and the secret box leave the composer at once; a hand-off or a save keeps its text until the hub said yes (the reply empties it), so a refusal loses nothing.
fn run(cx: &SendCtx, session: &Value, sel: &str, w: Word) -> ActOut {
    let rid = session["rid"].as_u64().unwrap_or(0);
    let req = |m: &str, p: Value| json!({"type": "send", "hub": cx.hub, "text": json!({"id": rid, "m": m, "p": p}).to_string()});
    match w {
        Word::Btw(q) if q.is_empty() => say(BTW_EMPTY),
        Word::Btw(q) => {
            let mut drafts = Map::new(); drafts.insert(sel.to_string(), json!("")); drafts.insert(format!("@btw:{sel}"), json!(q)); drafts.insert(format!("@btw.a:{sel}"), json!(""));
            let mut cmds = compose(cx.hub, sel, ""); cmds.push(req("thread.btw", json!({"thread": sel, "q": q})));
            ActOut { session_patch: json!({"rid": rid + 1, "drafts": drafts}), cmds }
        }
        Word::Give => ActOut { session_patch: json!({"rid": rid + 1}), cmds: vec![req("bots.give", json!({"thread": sel, "text": draft_of(session, sel).trim()}))] },
        Word::Secret(name) => ActOut { session_patch: json!({"drafts": {sel: "", "@sbox": if name.is_empty() { "+" } else { name.as_str() }, "@sbox.th": sel, "@sbox.err": "", "@f.sname": name}}), cmds: compose(cx.hub, sel, "") },
        // `/intent` alone has nothing to save, and saving nothing would empty the file: it opens the intent to edit
        Word::Intent(body) if body.is_empty() => intent_borrow(cx.hub, session, sel),
        Word::Intent(body) => ActOut { session_patch: json!({"rid": rid + 1}), cmds: vec![req("fab.intent", json!({"thread": sel, "text": body}))] },
    }
}

/// `thread.btw`: the side question of this thread and its answer, until closed (Btw.text).
pub fn btw_json(session: &Value, thread: &str) -> Value {
    let q = s(&session["drafts"], &format!("@btw:{thread}")); if q.is_empty() { return Value::Null }
    let a = s(&session["drafts"], &format!("@btw.a:{thread}"));
    json!({"q": q, "a": if a.is_empty() { "Thinking…" } else { a }})
}
/// Btw.answer: an answer that came back empty says where the model is chosen.
pub fn btw_answer(a: &str) -> String { let a = a.trim(); if a.is_empty() { "No answer came back (Settings, Writing: the model it asks).".into() } else { a.to_string() } }
/// `thread.secretBox`: the masked box of this thread: one opened here (`/secret`), else the first of the agents' requests in it (`asks`: `(name, purpose, thread)`).
/// No value is ever here: the field is the app's own, and Save goes straight to the hub (`secret.set`).
pub fn secret_box(session: &Value, thread: &str, title: &str, asks: &[(String, String, String)]) -> Value {
    let d = &session["drafts"];
    let (open, err) = (s(d, "@sbox"), s(d, "@sbox.err"));
    let pack = |name: &str, purpose: &str, fresh: bool, ask: bool| json!({"name": name, "purpose": purpose, "who": title, "thread": thread, "fresh": fresh, "ask": ask, "err": err,
        "save": format!("{name}\u{1f}{purpose}\u{1f}{thread}")});
    if !open.is_empty() && s(d, "@sbox.th") == thread {
        let asked = asks.iter().find(|(n, ..)| n == open);
        return pack(if open == "+" { "" } else { open }, asked.map_or(s(d, "@f.spurpose"), |a| a.1.as_str()), open == "+", asked.is_some());
    }
    match asks.iter().find(|a| a.2 == thread) { Some((n, p, _)) => pack(n, p, false, true), None => Value::Null }
}

/// Ui.act "send": the plain draft of the selected thread sent as `turn.start` (mode "steer": Follow.mode), the draft and its companions emptied, the request kept in the outbox.
/// A draft that is a command of the composer runs as that command. Never None: this is the person pressing Send, and there is no one else to answer them.
pub fn send(cx: &SendCtx, session: &Value) -> Option<ActOut> {
    let sel = s(session, "sel");
    let body = draft_of(session, sel).trim();
    let d = &session["drafts"];
    if !s(d, "@draft.bound").is_empty() { return Some(say(&bound_says(s(d, "@draft.bound")))) }
    // Att.join: what was written, then one `[attachment: path]` line per attachment
    let atts: Vec<String> = att_paths(s(d, &format!("@att:{sel}")));
    let lines = atts.iter().map(|p| format!("[attachment: {p}]")).collect::<Vec<_>>().join("\n");
    let text = if atts.is_empty() { body.to_string() } else if body.is_empty() { lines } else { format!("{body}\n\n{lines}") };
    // Rpl: Reply on a message keeps its id (`@reply:<thread>`) and its text (`@reply.full:<thread>`); the send puts the quote ahead of the body, the clients show only the body
    let (reply_id, reply_full) = (s(d, &format!("@reply:{sel}")), s(d, &format!("@reply.full:{sel}")));
    let text = if !text.is_empty() && !reply_id.is_empty() && !reply_full.is_empty() { format!("{}\n\n{text}", reply_block(reply_id, reply_full)) } else { text };
    let text = text.as_str();
    if text.is_empty() { return Some(ActOut { session_patch: json!({}), cmds: vec![] }) } // Ui.send.k: nothing typed sends nothing
    if sel.is_empty() || !safe(sel) { return Some(say("Open a thread first: this has nowhere to go yet.")) }
    if let Some(w) = word_of(body) { return Some(run(cx, session, sel, w)) }
    if cx.bot_thread && cx.bot.is_none() { return Some(say("This bot's thread is not ready, so nothing was sent. Your text is still here.")) }
    let rid = session["rid"].as_u64().unwrap_or(0);
    // Rpc.text: {"id":rid,"m":"turn.start","p":{thread,text,msg,mode}} in this key order
    // a bot's thread is talked to as the bot (Bots.say: its wake, its memory, the star count); the thread's account is the bot's own
    let (m, p) = match &cx.bot { Some(b) => ("bots.say", json!({"bot": b, "text": text, "msg": msg_id(cx.cid, cx.now_s, rid)})), None => ("turn.start", bind(json!({"thread": sel, "text": text, "msg": msg_id(cx.cid, cx.now_s, rid), "mode": "steer"}), cx.binding.as_deref())) };
    let request = format!("{{\"id\":{rid},\"m\":\"{m}\",\"p\":{}}}", serde_json::to_string(&p).ok()?);
    let mut drafts = Map::new();
    for k in [sel.to_string(), format!("@att:{sel}"), format!("@reply:{sel}"), format!("@reply.q:{sel}"), format!("@reply.full:{sel}")] { drafts.insert(k, json!("")); }
    // Draft.intent.clear: ordinary text sent over an intent edit ends it
    if s(d, &format!("@intent.edit:{sel}")) == "1" { for k in [format!("@intent.edit:{sel}"), format!("@intent.draft:{sel}")] { drafts.insert(k, json!("")); } }
    let mut outbox = Map::new(); outbox.insert(rid.to_string(), json!(request));
    Some(ActOut {
        session_patch: json!({"rid": rid + 1, "drafts": drafts, "outbox": outbox}),
        cmds: vec![json!({"type": "keep", "thread": format!("{}|{sel}", cx.hub), "text": ""}), json!({"type": "send", "hub": cx.hub, "text": request}), json!({"type": "scroll"})],
    })
}

/// Refs.blank: only spaces and 9..13 (the composer asks this of a pasted draft on every render, stopping at the first other character).
fn blank(t: &str) -> bool { t.chars().all(|c| c == ' ' || ('\u{9}'..='\u{d}').contains(&c)) }

/// Btw.is / Give.is / Intent.is: the draft is the command `w` (`/btw`, `/give`, `/intent`): the word alone (leading and trailing blanks aside), or `w ` / `w\n` first.
pub fn command_is(draft: &str, w: &str) -> bool {
    let lead = draft.trim_start_matches(|c| c == ' ' || ('\u{9}'..='\u{d}').contains(&c));
    (lead.starts_with(w) && blank(&lead[w.len()..])) || draft.starts_with(&format!("{w} ")) || draft.starts_with(&format!("{w}\n"))
}

/// The composer's rows for a slash command being typed (Composer.slash: `Slash.local`: the commands whose name starts with what follows the `/`, the first marked): `{name, desc, on}`.
pub fn slash_rows(claude: bool, draft: &str) -> Value {
    if !super::slash::typing(draft) { return json!([]) }
    let q = super::slash::token(draft).strip_prefix('/').unwrap_or("");
    Value::Array(super::slash::cmds(claude, q).iter().enumerate().map(|(i, (n, a))| json!({"name": n, "desc": a, "on": i == 0})).collect())
}

/// Composer.of for the thread view: `{draft, skills, send, sendAct}`. `running`: the turn runs (state "run"); `attaching`: files are being attached; `own_skills`: the rows the
/// thread already carries (the hub's `$` / reference popup: the caller's, `[]` for a plain thread). English only: another interface language is None (the helper's).
pub fn fields(draft: &str, running: bool, attaching: bool, claude: bool, own_skills: &Value, lang: &str) -> Option<Value> {
    if !matches!(lang, "" | "en") { return None }
    // Follow.stop: Stop while a turn runs and nothing is typed or attached
    let stop = running && blank(draft) && !attaching;
    let label = if stop { "Stop" } else if command_is(draft, "/btw") { "Ask" } else if command_is(draft, "/give") { "Give" } else if command_is(draft, "/intent") { "Save" } else if running { "Steer" } else { "Send" };
    let skills = if super::slash::typing(draft) { slash_rows(claude, draft) } else { own_skills.clone() };
    Some(json!({"draft": draft, "skills": skills, "send": label, "sendAct": if stop { "interrupt" } else { "send" }}))
}

/// What the model knows about the selected thread that a send needs: the presenter implements it over its metadata projection.
pub trait Facts { fn bot_thread(&self, thread: &str) -> bool; /// the bot whose thread this is
    fn bot_of(&self, _thread: &str) -> Option<String> { None } fn account_params(&self, thread: &str) -> bool; /// the thread's named account binding, as its setting text (Accounts.owner); None when it has none (Accounts.named)
    fn account_binding(&self, _thread: &str) -> Option<String> { None }
    /// the thread's provider ("claude" when unknown)
    fn provider_of(&self, _thread: &str) -> String { "claude".into() } }

/// The composer's actions as the presenter's action table calls them: `params` is the object the hub builds for a view request (`session`, `action`, `value`, `cid`, `hub`,
/// `now` in milliseconds). `draft`, `send` and `send-alt` (the other follow-up mode: Follow.mode answers "steer" for both); everything else, and every case not ported, is None.
pub fn act(params: &Value, facts: &dyn Facts) -> Option<ActOut> {
    let (session, hub) = (&params["session"], s(params, "hub"));
    match s(params, "action") {
        "draft" => draft(hub, session, s(params, "value")),
        // a popup row chosen (a reference, a `/command` or a `$skill`): the word being typed becomes the row's word
        "skill" => {
            let sel = s(session, "sel"); let word = s(params, "value");
            if !safe(sel) { return None }
            if word.is_empty() { return Some(ActOut { session_patch: json!({}), cmds: vec![] }) } // Skills.pick found no row: nothing to complete (Ui.skill.k), and not an error
            // the install row: choosing it asks (the draft's word becomes `$install?<url>`, whose row confirms); the confirm sends the install and drops the word
            if let Some(url) = word.strip_prefix("$install:") {
                let text = mention::complete(draft_of(session, sel), &format!("$install?{url}")).trim_end().to_string();
                return Some(ActOut { session_patch: json!({"drafts": { sel: text }}), cmds: vec![json!({"type": "keep", "thread": format!("{hub}|{sel}"), "text": text})] });
            }
            if let Some((url, replace)) = word.strip_prefix("$install!").map(|u| (u, false)).or_else(|| word.strip_prefix("$replace!").map(|u| (u, true))) {
                let d = draft_of(session, sel); let t = mention::token(d); let text = d[..d.len() - t.len()].trim_end().to_string();
                let req = json!({"id": session["rid"].as_u64().unwrap_or(0), "m": "skill.install", "p": {"url": url, "provider": facts.provider_of(sel), "replace": replace}});
                return Some(ActOut { session_patch: json!({"rid": session["rid"].as_u64().unwrap_or(0) + 1, "drafts": { sel: text }}), cmds: vec![json!({"type": "keep", "thread": format!("{hub}|{sel}"), "text": text}), json!({"type": "send", "hub": hub, "text": req.to_string()})] });
            }
            let text = mention::complete(draft_of(session, sel), word);
            Some(ActOut { session_patch: json!({"drafts": { sel: text }}), cmds: vec![json!({"type": "keep", "thread": format!("{hub}|{sel}"), "text": text})] })
        }
        // Reply on a message: `<id>\u{1f}<its text>`; the composer shows the first line until reply-drop or the next send
        "reply" => {
            let sel = s(session, "sel");
            let (id, full) = s(params, "value").split_once('\u{1f}')?;
            if !safe(sel) || id.is_empty() || full.trim().is_empty() { return None }
            let first = full.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
            Some(ActOut { session_patch: json!({"drafts": {format!("@reply:{sel}"): id, format!("@reply.q:{sel}"): first, format!("@reply.full:{sel}"): full}}), cmds: vec![json!({"type": "focus", "id": "composer"})] })
        }
        "reply-drop" => {
            let sel = s(session, "sel"); if !safe(sel) { return None }
            Some(ActOut { session_patch: json!({"drafts": {format!("@reply:{sel}"): "", format!("@reply.q:{sel}"): "", format!("@reply.full:{sel}"): ""}}), cmds: vec![] })
        }
        // Esc on a popup (Ui.skill_close): the popup stays shut for this draft; the draft it was shut on is remembered (`@sk.off`), another edit opens it again
        "skill-close" => {
            let sel = s(session, "sel"); if !safe(sel) { return None }
            Some(ActOut { session_patch: json!({"drafts": {"@sk.off": draft_of(session, sel)}}), cmds: vec![] })
        }
        // the card above the composer put away (Btw.close)
        "btw-close" => {
            let sel = s(session, "sel"); if !safe(sel) { return None }
            Some(ActOut { session_patch: json!({"drafts": {format!("@btw:{sel}"): "", format!("@btw.a:{sel}"): ""}}), cmds: vec![] })
        }
        // the project card's Edit: the intent in the composer (Fab.act 1)
        "fab-edit" => { let sel = s(session, "sel"); if !safe(sel) { return None } Some(intent_borrow(hub, session, sel)) }
        // the secret box (Sbox.act). No action carries a value: Save is the app's own request, and these only open, shut and say why
        "secret-open-here" => {
            let sel = s(session, "sel"); if !safe(sel) { return None }
            let name = s(params, "value");
            Some(ActOut { session_patch: json!({"drafts": {"@sbox": if name.is_empty() { "+" } else { name }, "@sbox.th": sel, "@sbox.err": "", "@f.sname": name}}), cmds: vec![] })
        }
        "secret-shut" | "secret-sent" => Some(ActOut { session_patch: json!({"drafts": {"@sbox": "", "@sbox.th": "", "@sbox.err": "", "@f.sname": "", "@f.spurpose": ""}}), cmds: vec![] }),
        "secret-err" => Some(ActOut { session_patch: json!({"drafts": {"@sbox.err": s(params, "value")}}), cmds: vec![] }),
        "secret-dismiss" => {
            let rid = session["rid"].as_u64().unwrap_or(0);
            Some(ActOut { session_patch: json!({"rid": rid + 1, "drafts": {"@sbox": "", "@sbox.th": "", "@sbox.err": "", "@f.sname": ""}}),
                cmds: vec![json!({"type": "send", "hub": hub, "text": json!({"id": rid, "m": "secret.dismiss", "p": {"name": s(params, "value")}}).to_string()})] })
        }
        "send" | "send-alt" => {
            let sel = s(session, "sel");
            send(&SendCtx { hub, cid: s(params, "cid"), now_s: params["now"].as_u64().unwrap_or(0) / 1000, bot_thread: facts.bot_thread(sel), bot: facts.bot_of(sel), binding: facts.account_binding(sel) }, session)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test] fn a_reply_is_a_counted_block_the_clients_take_apart() {
        let b = reply_block("m1", "line one\nline two");
        let msg = format!("{b}\n\nmy answer");
        assert_eq!(reply_split(&msg), Some(("m1".into(), "line one\nline two".into(), "my answer".into())));
        assert_eq!(reply_split("plain"), None);
        assert_eq!(reply_split("<in_reply_to msg=m1 chars=99>\nshort\n</in_reply_to>\n\nx"), None, "a quote shorter than it says is not a block");
        let b = send(&SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: true, bot: Some("b1".into()), binding: None }, &json!({"sel": "t1", "rid": 2, "drafts": {"t1": "hi"}, "info": {}})).unwrap();
        assert!(b.cmds[1]["text"].as_str().unwrap().contains("\"m\":\"bots.say\"") && b.cmds[1]["text"].as_str().unwrap().contains("\"bot\":\"b1\""), "{:?}", b.cmds[1]);
        let cx = SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: false, bot: None, binding: None };
        let s = json!({"sel": "t1", "rid": 2, "drafts": {"t1": "ok", "@reply:t1": "m7", "@reply.q:t1": "hi", "@reply.full:t1": "hi there"}, "info": {}});
        let o = send(&cx, &s).unwrap();
        assert!(o.cmds[1]["text"].as_str().unwrap().contains(r"<in_reply_to msg=m7 chars=8>\nhi there\n</in_reply_to>\n\nok"), "{:?}", o.cmds[1]);
        assert_eq!(o.session_patch["drafts"]["@reply:t1"], "");
        let p = |a: &str, v: &str| json!({"session": s, "action": a, "value": v, "hub": "h"});
        struct Plain; impl Facts for Plain { fn bot_thread(&self, _: &str) -> bool { false } fn account_params(&self, _: &str) -> bool { false } }
        let r = act(&p("reply", "m9\u{1f}first\nsecond"), &Plain).unwrap();
        assert_eq!((r.session_patch["drafts"]["@reply:t1"].as_str(), r.session_patch["drafts"]["@reply.q:t1"].as_str()), (Some("m9"), Some("first")));
        assert_eq!(act(&p("reply-drop", ""), &Plain).unwrap().session_patch["drafts"]["@reply.full:t1"], "");
    }
    use super::*;
    // recorded by the JS hub (parity-960s/recA session-gpui): select, draft "hello parity", send
    fn session() -> Value { json!({"sel": "t1700000002-3", "rid": 2, "drafts": {"t1700000002-3": "hello parity", "@boot": ""}, "info": {}}) }
    #[test] fn a_plain_draft_is_kept_and_set() {
        let o = draft("http://127.0.0.1:3820", &session(), "hello parity").unwrap();
        assert_eq!(o.cmds, vec![json!({"type": "keep", "thread": "http://127.0.0.1:3820|t1700000002-3", "text": "hello parity"})]);
        assert_eq!(o.session_patch, json!({"drafts": {"t1700000002-3": "hello parity"}}));
    }
    #[test] fn a_scratch_selection_is_not_kept() { assert!(draft("h", &json!({"sel": "@x"}), "a").unwrap().cmds.is_empty()) }
    #[test] fn a_sigil_word_is_kept_like_any_text() {
        for v in ["/gi", "hi $sk", "see @bo", "to >th", "in %pr", "post #ro", "a ^b"] { assert!(draft("h", &session(), v).is_some(), "{v}") }
    }
    fn sent_method(o: &ActOut) -> Option<(String, Value)> {
        o.cmds.iter().find(|c| c["type"] == "send").map(|c| { let r: Value = serde_json::from_str(c["text"].as_str().unwrap()).unwrap(); (r["m"].as_str().unwrap().to_string(), r["p"].clone()) })
    }
    fn says(o: &ActOut) -> &str { o.cmds.iter().find(|c| c["type"] == "say").and_then(|c| c["text"].as_str()).unwrap_or("") }
    const T: &str = "t1700000002-3";
    fn cx() -> SendCtx<'static> { SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: false, bot: None, binding: None } }
    fn with(d: &str) -> Value { json!({"sel": T, "rid": 2, "drafts": {T: d}, "info": {}}) }
    #[test] fn the_composers_words_are_read_from_the_draft() {
        assert_eq!(word_of("/btw  why now \n"), Some(Word::Btw("why now".into()))); assert_eq!(word_of("/btw"), Some(Word::Btw("".into())));
        assert_eq!(word_of("/give >x note"), Some(Word::Give)); assert_eq!(word_of("/secret"), Some(Word::Secret("".into()))); assert_eq!(word_of("/secret github-token"), Some(Word::Secret("github-token".into())));
        assert_eq!(word_of("/intent\n# Goal\nlamp"), Some(Word::Intent("# Goal\nlamp".into()))); assert_eq!(word_of("/compact"), None); assert_eq!(word_of("hi /btw"), None); assert_eq!(word_of("/btwx"), None);
    }
    #[test] fn a_side_question_goes_to_the_hub_and_never_to_the_thread() {
        let o = send(&cx(), &with("/btw why")).unwrap();
        let (m, p) = sent_method(&o).unwrap(); assert_eq!((m.as_str(), p["q"].as_str(), p["thread"].as_str()), ("thread.btw", Some("why"), Some(T)));
        assert_eq!(o.session_patch["drafts"][format!("@btw:{T}")], "why"); assert_eq!(o.session_patch["drafts"][T], "", "the draft goes with it");
        assert!(o.cmds.iter().any(|c| c["type"] == "compose" && c["text"] == ""), "the box is emptied now, not when the answer comes");
        assert_eq!(btw_json(&json!({"drafts": {format!("@btw:{T}"): "why", format!("@btw.a:{T}"): ""}}), T), json!({"q": "why", "a": "Thinking…"}));
        assert_eq!(btw_json(&json!({"drafts": {format!("@btw:{T}"): "why", format!("@btw.a:{T}"): "because"}}), T)["a"], "because"); assert_eq!(btw_json(&json!({"drafts": {}}), T), Value::Null);
        assert!(btw_answer("  ").contains("Settings, Writing")); assert_eq!(btw_answer(" a "), "a");
    }
    #[test] fn a_refused_command_says_why_and_keeps_the_draft() {
        for (draft, says_) in [("/btw", BTW_EMPTY)] {
            let o = send(&cx(), &with(draft)).unwrap();
            assert_eq!(says(&o), says_); assert_eq!(o.session_patch, json!({}), "nothing changes, so the draft stays"); assert!(sent_method(&o).is_none());
        }
        let mut b = with("long"); b["drafts"]["@draft.bound"] = json!("200000 abc");
        let o = send(&cx(), &b).unwrap(); assert_eq!(says(&o), "This draft is 196 KB. A message can be at most 128 KB."); assert_eq!(o.session_patch, json!({}));
        let o = send(&SendCtx { bot_thread: true, ..cx() }, &session()).unwrap(); assert!(says(&o).contains("nothing was sent")); assert_eq!(o.session_patch, json!({}));
        let o = send(&cx(), &json!({"sel": "", "rid": 2, "drafts": {"": "hi"}})).unwrap(); assert!(!says(&o).is_empty());
    }
    #[test] fn give_keeps_its_text_until_the_hub_took_it() {
        let o = send(&cx(), &with("/give >x see this")).unwrap();
        let (m, p) = sent_method(&o).unwrap(); assert_eq!((m.as_str(), p["text"].as_str()), ("bots.give", Some("/give >x see this")));
        assert!(o.session_patch["drafts"].is_null() && o.cmds.len() == 1, "a refusal must find the text still there");
    }
    #[test] fn secret_opens_the_box_and_sends_nothing() {
        let o = send(&cx(), &with("/secret gh")).unwrap(); assert!(sent_method(&o).is_none());
        let mut s = with(""); crate::session::patch::merge(&mut s, &o.session_patch);
        let b = secret_box(&s, T, "A thread", &[]);
        assert_eq!((b["name"].as_str(), b["fresh"].as_bool(), b["who"].as_str(), b["save"].as_str()), (Some("gh"), Some(false), Some("A thread"), Some(format!("gh\u{1f}\u{1f}{T}").as_str())));
        assert_eq!(secret_box(&s, "other", "x", &[]), Value::Null, "only in the thread it was opened in");
        let s = with(""); let o = send(&cx(), &with("/secret")).unwrap(); let mut s2 = s.clone(); crate::session::patch::merge(&mut s2, &o.session_patch); assert_eq!(secret_box(&s2, T, "t", &[])["fresh"], true);
        let asks = [("db".to_string(), "the database".to_string(), T.to_string())];
        let a = secret_box(&s, T, "t", &asks); assert_eq!((a["name"].as_str(), a["purpose"].as_str(), a["ask"].as_bool()), (Some("db"), Some("the database"), Some(true)), "an agent's request shows by itself");
    }
    #[test] fn an_intent_is_saved_through_its_request_and_edit_borrows_the_composer() {
        let o = send(&cx(), &with("/intent # Goal\nlamp")).unwrap();
        let (m, p) = sent_method(&o).unwrap(); assert_eq!((m.as_str(), p["text"].as_str()), ("fab.intent", Some("# Goal\nlamp")));
        assert!(o.session_patch["drafts"].is_null(), "the text stays until the hub wrote it");
        let mut s = with("half a thought"); s["info"] = json!({format!("fab.intent:{T}"): "# Goal\nlamp"});
        let e = act(&json!({"session": s, "action": "fab-edit", "value": "", "hub": "h"}), &Plain).unwrap();
        assert_eq!(e.session_patch["drafts"][T], "/intent\n# Goal\nlamp"); assert_eq!(e.session_patch["drafts"][format!("@intent.draft:{T}")], "half a thought"); assert_eq!(e.session_patch["drafts"][format!("@intent.edit:{T}")], "1");
        assert!(e.cmds.iter().any(|c| c["type"] == "compose" && c["text"] == "/intent\n# Goal\nlamp"));
        // Edit again before Save keeps the conversation's draft it kept the first time
        let mut s2 = s.clone(); crate::session::patch::merge(&mut s2, &e.session_patch);
        let again = act(&json!({"session": s2, "action": "fab-edit", "value": "", "hub": "h"}), &Plain).unwrap(); assert!(again.session_patch["drafts"].get(format!("@intent.draft:{T}")).is_none());
        // `/intent` alone saves nothing: emptying a project's intent takes more than a word
        let bare = send(&cx(), &with("/intent")).unwrap(); assert!(sent_method(&bare).is_none()); assert_eq!(bare.session_patch["drafts"][T], intent_edit("")); assert_eq!(bare.session_patch["drafts"][format!("@intent.draft:{T}")], "");
        // ordinary text sent over an abandoned edit ends it
        let mut o = with("just chat"); o["drafts"][format!("@intent.edit:{T}")] = json!("1"); o["drafts"][format!("@intent.draft:{T}")] = json!("old");
        let p = send(&cx(), &o).unwrap(); assert_eq!(sent_method(&p).unwrap().0, "turn.start"); assert_eq!(p.session_patch["drafts"][format!("@intent.edit:{T}")], ""); assert_eq!(p.session_patch["drafts"][format!("@intent.draft:{T}")], "");
    }
    #[test] fn other_slash_words_are_messages() { assert_eq!(sent_method(&send(&cx(), &with("/compact")).unwrap()).unwrap().0, "turn.start"); assert_eq!(sent_method(&send(&cx(), &with("see @bot %proj")).unwrap()).unwrap().0, "turn.start"); }
    #[test] fn send_is_what_the_js_hub_answered() {
        let cx = SendCtx { hub: "http://127.0.0.1:3820", cid: "parity-gpui", now_s: 1793864523, bot_thread: false, bot: None, binding: None };
        let o = send(&cx, &session()).unwrap();
        assert_eq!(o.cmds, vec![
            json!({"type": "keep", "thread": "http://127.0.0.1:3820|t1700000002-3", "text": ""}),
            json!({"type": "send", "hub": "http://127.0.0.1:3820", "text": "{\"id\":2,\"m\":\"turn.start\",\"p\":{\"thread\":\"t1700000002-3\",\"text\":\"hello parity\",\"msg\":\"cparity-gpui1793864523-2\",\"mode\":\"steer\"}}"}),
            json!({"type": "scroll"})]);
        let p = &o.session_patch;
        assert_eq!(p["rid"], json!(3));
        assert_eq!(p["drafts"], json!({"t1700000002-3": "", "@att:t1700000002-3": "", "@reply:t1700000002-3": "", "@reply.q:t1700000002-3": "", "@reply.full:t1700000002-3": ""}));
        assert_eq!(p["outbox"]["2"], o.cmds[1]["text"]);
    }
    #[test] fn command_words_are_the_drafts_first_word_alone() {
        assert!(command_is("/btw", "/btw") && command_is("  /btw  \n", "/btw") && command_is("/btw why", "/btw") && command_is("/btw\nwhy", "/btw"));
        assert!(!command_is("/btwx", "/btw") && !command_is("hi /btw", "/btw") && !command_is("/btw\twhy", "/btw") && !command_is("", "/btw"));
    }
    #[test] fn the_send_button_follows_the_draft() {
        let f = |d: &str, run: bool, att: bool| fields(d, run, att, true, &json!([]), "en").unwrap();
        assert_eq!((f("hi", false, false)["send"].as_str(), f("hi", false, false)["sendAct"].as_str()), (Some("Send"), Some("send")));
        assert_eq!(f("hi", true, false)["send"], json!("Steer"));
        assert_eq!((f("", true, false)["send"].as_str(), f("  ", true, false)["sendAct"].as_str()), (Some("Stop"), Some("interrupt")));
        assert_eq!(f("", true, true)["send"], json!("Steer"), "attaching: not blank for the button");
        assert_eq!((f("/btw q", false, false)["send"].as_str(), f("/give >t", false, false)["send"].as_str(), f("/intent x", true, false)["send"].as_str()), (Some("Ask"), Some("Give"), Some("Save")));
        assert!(fields("hi", false, false, true, &json!([]), "ja").is_none(), "another language is the helper's");
    }
    #[test] fn a_slash_being_typed_offers_its_commands_the_first_marked() {
        let r = slash_rows(true, "/c");
        assert_eq!(r, json!([{"name": "/compact", "desc": "Summarize the conversation to free context", "on": true}, {"name": "/context", "desc": "Show what fills the context window", "on": false}]));
        assert_eq!(slash_rows(false, "/")[0]["name"], json!("/btw"));
        assert_eq!(slash_rows(true, "/give >a"), json!([]));
        let own = json!([{"name": "x", "desc": "", "on": true}]);
        assert_eq!(fields("hello", false, false, false, &own, "en").unwrap()["skills"], own);
        assert_eq!(fields("/b", false, false, false, &own, "en").unwrap()["skills"][0]["name"], json!("/btw"));
    }
    struct Plain;
    impl Facts for Plain { fn bot_thread(&self, _: &str) -> bool { false } fn account_params(&self, _: &str) -> bool { false } }
    struct Bot;
    impl Facts for Bot { fn bot_thread(&self, _: &str) -> bool { true } fn account_params(&self, _: &str) -> bool { false } }
    #[test] fn an_install_link_is_asked_about_before_it_is_sent() {
        let sess = |d: &str| json!({"sel": "t1", "rid": 4, "drafts": {"t1": d}});
        let p = |d: &str, w: &str| json!({"hub": "h", "action": "skill", "value": w, "session": sess(d)});
        let a = act(&p("see $https://x.org/a/unslop/SKILL.md", "$install:https://x.org/a/unslop/SKILL.md"), &Plain).unwrap();
        assert_eq!(a.session_patch["drafts"]["t1"], "see $install?https://x.org/a/unslop/SKILL.md"); assert!(a.cmds.iter().all(|c| c["type"] != "send"), "nothing is fetched before the confirm");
        let b = act(&p("see $install?https://x.org/a/unslop/SKILL.md", "$install!https://x.org/a/unslop/SKILL.md"), &Plain).unwrap();
        assert_eq!(b.session_patch["drafts"]["t1"], "see"); let send = b.cmds.iter().find(|c| c["type"] == "send").expect("the install request");
        let req: Value = serde_json::from_str(send["text"].as_str().unwrap()).unwrap(); assert_eq!((req["m"].as_str(), req["p"]["url"].as_str(), req["p"]["provider"].as_str()), (Some("skill.install"), Some("https://x.org/a/unslop/SKILL.md"), Some("claude")));
    }
    #[test] fn the_action_table_routes_draft_send_and_send_alt_only() {
        let p = |a: &str, v: &str| json!({"session": session(), "action": a, "value": v, "cid": "parity-gpui", "hub": "http://127.0.0.1:3820", "now": 1793864523123u64});
        assert_eq!(act(&p("draft", "hello parity"), &Plain).unwrap().cmds.len(), 1);
        let send_o = act(&p("send", ""), &Plain).unwrap();
        assert_eq!(send_o.cmds[1]["text"].as_str().unwrap().contains("\"msg\":\"cparity-gpui1793864523-2\""), true, "now is milliseconds in the params, seconds in the msg id");
        assert_eq!(act(&p("send-alt", ""), &Plain).unwrap().cmds, send_o.cmds, "Follow.mode answers steer for both");
        assert!(says(&act(&p("send", ""), &Bot).unwrap()).contains("nothing was sent"), "a bot's thread with no bot says so");
        for a in ["select", "row-pin", "interrupt", ""] { assert!(act(&p(a, "x"), &Plain).is_none(), "{a}") }
    }
    #[test] fn send_always_answers() {
        let cx = SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: false, bot: None, binding: None };
        let with = |k: &str, v: &str| { let mut s = session(); s["drafts"][k] = json!(v); s };
        let blank = send(&cx, &with("t1700000002-3", "   ")).unwrap(); assert!(blank.cmds.is_empty(), "blank sends nothing and says nothing");
        assert!(send(&cx, &with("@att:t1700000002-3", "a")).is_some(), "attachments");
        assert!(send(&cx, &with("@reply:t1700000002-3", "m1")).is_some(), "a reply with no quote kept sends as plain text");
    }
}
