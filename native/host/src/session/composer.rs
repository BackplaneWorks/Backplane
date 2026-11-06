//! The composer's actions in Rust, as pure functions over the request's session (serde Values, no
//! Session type), returning the patch to merge into the session and the commands for the client, or None for a case not ported (the caller falls back to the helper).
//! Ported: a plain-text `draft` and a plain-text `send` to a thread (not a bot's). Not ported (None): a draft with a popup in play (a sigil word, `$`, an open
//! popup: Refs.want/Skills.want), `/btw` `/give` `/secret` `/intent`, attachments, replies, a bot's thread (Bots.say), a bounded draft, accounts params.
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

/// Ui.act "send": the plain draft of the selected thread sent as `turn.start` (mode "steer": Follow.mode), the draft and its companions emptied, the request kept in the outbox.
pub fn send(cx: &SendCtx, session: &Value) -> Option<ActOut> {
    let sel = s(session, "sel");
    let body = draft_of(session, sel).trim();
    let d = &session["drafts"];
    // Att.join: what was written, then one `[attachment: path]` line per attachment
    let atts: Vec<String> = att_paths(s(d, &format!("@att:{sel}")));
    let lines = atts.iter().map(|p| format!("[attachment: {p}]")).collect::<Vec<_>>().join("\n");
    let text = if atts.is_empty() { body.to_string() } else if body.is_empty() { lines } else { format!("{body}\n\n{lines}") };
    // Rpl: Reply on a message keeps its id (`@reply:<thread>`) and its text (`@reply.full:<thread>`); the send puts the quote ahead of the body, the clients show only the body
    let (reply_id, reply_full) = (s(d, &format!("@reply:{sel}")), s(d, &format!("@reply.full:{sel}")));
    let text = if !text.is_empty() && !reply_id.is_empty() && !reply_full.is_empty() { format!("{}\n\n{text}", reply_block(reply_id, reply_full)) } else { text };
    let text = text.as_str();
    if sel.is_empty() || !safe(sel) || text.is_empty() { return None }
    if ["/btw", "/give", "/secret", "/intent"].iter().any(|w| command_is(body, w)) || (cx.bot_thread && cx.bot.is_none()) { return None }
    if !s(d, &format!("@intent.edit:{sel}")).is_empty() || !s(d, "@draft.bound").is_empty() { return None }
    let rid = session["rid"].as_u64().unwrap_or(0);
    // Rpc.text: {"id":rid,"m":"turn.start","p":{thread,text,msg,mode}} in this key order
    // a bot's thread is talked to as the bot (Bots.say: its wake, its memory, the star count); the thread's account is the bot's own
    let (m, p) = match &cx.bot { Some(b) => ("bots.say", json!({"bot": b, "text": text, "msg": msg_id(cx.cid, cx.now_s, rid)})), None => ("turn.start", bind(json!({"thread": sel, "text": text, "msg": msg_id(cx.cid, cx.now_s, rid), "mode": "steer"}), cx.binding.as_deref())) };
    let request = format!("{{\"id\":{rid},\"m\":\"{m}\",\"p\":{}}}", serde_json::to_string(&p).ok()?);
    let mut drafts = Map::new();
    for k in [sel.to_string(), format!("@att:{sel}"), format!("@reply:{sel}"), format!("@reply.q:{sel}"), format!("@reply.full:{sel}")] { drafts.insert(k, json!("")); }
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
    fn account_binding(&self, _thread: &str) -> Option<String> { None } }

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
    #[test] fn only_the_helpers_own_commands_do_not_send() {
        let cx = SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: false, bot: None, binding: None };
        let with = |d: &str| json!({"sel": "t1700000002-3", "rid": 2, "drafts": {"t1700000002-3": d}, "info": {}});
        assert!(send(&cx, &with("/btw why")).is_none() && send(&cx, &with("/give >x")).is_none());
        assert!(send(&cx, &with("/compact")).is_some() && send(&cx, &with("see @bot %proj")).is_some());
    }
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
    #[test] fn the_action_table_routes_draft_send_and_send_alt_only() {
        let p = |a: &str, v: &str| json!({"session": session(), "action": a, "value": v, "cid": "parity-gpui", "hub": "http://127.0.0.1:3820", "now": 1793864523123u64});
        assert_eq!(act(&p("draft", "hello parity"), &Plain).unwrap().cmds.len(), 1);
        let send_o = act(&p("send", ""), &Plain).unwrap();
        assert_eq!(send_o.cmds[1]["text"].as_str().unwrap().contains("\"msg\":\"cparity-gpui1793864523-2\""), true, "now is milliseconds in the params, seconds in the msg id");
        assert_eq!(act(&p("send-alt", ""), &Plain).unwrap().cmds, send_o.cmds, "Follow.mode answers steer for both");
        assert!(act(&p("send", ""), &Bot).is_none(), "a bot's thread is Bots.say");
        for a in ["select", "row-pin", "interrupt", ""] { assert!(act(&p(a, "x"), &Plain).is_none(), "{a}") }
    }
    #[test] fn what_is_not_ported_is_none() {
        let cx = SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: false, bot: None, binding: None };
        let with = |k: &str, v: &str| { let mut s = session(); s["drafts"][k] = json!(v); s };
        assert!(send(&cx, &with("t1700000002-3", "   ")).is_none(), "blank");
        assert!(send(&cx, &with("t1700000002-3", "/btw why")).is_none(), "slash commands");
        assert!(send(&cx, &with("@att:t1700000002-3", "a")).is_some(), "attachments");
        assert!(send(&cx, &with("@reply:t1700000002-3", "m1")).is_some(), "a reply with no quote kept sends as plain text");
        assert!(send(&cx, &with("@draft.bound", "5 abc")).is_none(), "a bounded draft");
        assert!(send(&SendCtx { bot_thread: true, ..SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: false, bot: None, binding: None } }, &session()).is_none(), "a bot's thread");
    }
}
