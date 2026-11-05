//! The composer's actions in Rust (src/core/client.bend: `Ui.draft`, `Ui.send.*`), as pure functions over the request's session (serde Values, no
//! Session type), returning the patch to merge into the session and the commands for the client, or None for a case not ported (the caller falls back to the helper).
//! Ported: a plain-text `draft` and a plain-text `send` to a thread (not a bot's). Not ported (None): a draft with a popup in play (a sigil word, `$`, an open
//! popup: Refs.want/Skills.want), `/btw` `/give` `/secret` `/intent`, attachments, replies, a bot's thread (Bots.say), a bounded draft, accounts params.
use serde_json::{json, Map, Value};

pub struct ActOut { pub session_patch: Value, pub cmds: Vec<Value> }

/// Draft.safe: the thread ids a draft may be kept for (not a scratch key).
fn safe(id: &str) -> bool { !id.is_empty() && !id.starts_with('@') }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn draft_of<'a>(session: &'a Value, key: &str) -> &'a str { session["drafts"][key].as_str().unwrap_or("") }

/// A word typed with a sigil or `/` is a popup in play: the helper's Refs.want / Skills.want decide what that asks for.
fn popup_word(text: &str) -> bool { matches!(super::slash::token(text).chars().next(), Some('/' | '$' | '@' | '>' | '%' | '#')) }

/// Ui.act "draft": the selected thread's draft set, the host asked to keep it (Draft.keep).
pub fn draft(hub: &str, session: &Value, value: &str) -> Option<ActOut> {
    let sel = s(session, "sel");
    // an open popup or a word that opens one: info `refs`/`refs.for`/`sk.off` and the skills fetch are the helper's
    if popup_word(value) || !s(&session["info"], "refs").is_empty() || !s(&session["info"], "refs.for").is_empty() { return None }
    let mut cmds = vec![];
    if safe(sel) { cmds.push(json!({"type": "keep", "thread": format!("{hub}|{sel}"), "text": value})) }
    Some(ActOut { session_patch: json!({"drafts": { sel: value }}), cmds })
}

/// The facts of the thread and the setting a send needs, read from the model by the caller.
pub struct SendCtx<'a> { pub hub: &'a str, pub cid: &'a str, pub now_s: u64, pub bot_thread: bool, pub account_params: bool }

/// Ui.msg_id: "c" ++ cid ++ now ++ "-" ++ rid.
pub fn msg_id(cid: &str, now_s: u64, rid: u64) -> String { format!("c{cid}{now_s}-{rid}") }

/// Ui.act "send": the plain draft of the selected thread sent as `turn.start` (mode "steer": Follow.mode), the draft and its companions emptied, the request kept in the outbox.
pub fn send(cx: &SendCtx, session: &Value) -> Option<ActOut> {
    let sel = s(session, "sel");
    let text = draft_of(session, sel).trim();
    if sel.is_empty() || !safe(sel) || text.is_empty() { return None }
    if text.starts_with('/') || cx.bot_thread || cx.account_params { return None }
    let d = &session["drafts"];
    if !s(d, &format!("@att:{sel}")).is_empty() || !s(d, &format!("@reply:{sel}")).is_empty() || !s(d, &format!("@intent.edit:{sel}")).is_empty() || !s(d, "@draft.bound").is_empty() { return None }
    let rid = session["rid"].as_u64().unwrap_or(0);
    // Rpc.text: {"id":rid,"m":"turn.start","p":{thread,text,msg,mode}} in this key order
    let request = format!("{{\"id\":{rid},\"m\":\"turn.start\",\"p\":{}}}", serde_json::to_string(&json!({"thread": sel, "text": text, "msg": msg_id(cx.cid, cx.now_s, rid), "mode": "steer"})).ok()?);
    let mut drafts = Map::new();
    for k in [sel.to_string(), format!("@att:{sel}"), format!("@reply:{sel}"), format!("@reply.q:{sel}")] { drafts.insert(k, json!("")); }
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
pub trait Facts { fn bot_thread(&self, thread: &str) -> bool; fn account_params(&self, thread: &str) -> bool; }

/// The composer's actions as the presenter's action table calls them: `params` is the object the hub builds for a view request (`session`, `action`, `value`, `cid`, `hub`,
/// `now` in milliseconds). `draft`, `send` and `send-alt` (the other follow-up mode: Follow.mode answers "steer" for both); everything else, and every case not ported, is None.
pub fn act(params: &Value, facts: &dyn Facts) -> Option<ActOut> {
    let (session, hub) = (&params["session"], s(params, "hub"));
    match s(params, "action") {
        "draft" => draft(hub, session, s(params, "value")),
        "send" | "send-alt" => {
            let sel = s(session, "sel");
            send(&SendCtx { hub, cid: s(params, "cid"), now_s: params["now"].as_u64().unwrap_or(0) / 1000, bot_thread: facts.bot_thread(sel), account_params: facts.account_params(sel) }, session)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // recorded by the JS hub (parity-960s/recA session-gpui): select, draft "hello parity", send
    fn session() -> Value { json!({"sel": "t1700000002-3", "rid": 2, "drafts": {"t1700000002-3": "hello parity", "@boot": ""}, "info": {}}) }
    #[test] fn a_plain_draft_is_kept_and_set() {
        let o = draft("http://127.0.0.1:3820", &session(), "hello parity").unwrap();
        assert_eq!(o.cmds, vec![json!({"type": "keep", "thread": "http://127.0.0.1:3820|t1700000002-3", "text": "hello parity"})]);
        assert_eq!(o.session_patch, json!({"drafts": {"t1700000002-3": "hello parity"}}));
    }
    #[test] fn a_scratch_selection_is_not_kept() { assert!(draft("h", &json!({"sel": "@x"}), "a").unwrap().cmds.is_empty()) }
    #[test] fn a_popup_in_play_is_the_helpers() {
        for v in ["/gi", "hi $sk", "see @bo", "to >th", "in %pr", "post #ro"] { assert!(draft("h", &session(), v).is_none(), "{v}") }
        let mut s = session(); s["info"]["refs"] = json!("x");
        assert!(draft("h", &s, "plain").is_none());
    }
    #[test] fn send_is_what_the_js_hub_answered() {
        let cx = SendCtx { hub: "http://127.0.0.1:3820", cid: "parity-gpui", now_s: 1793864523, bot_thread: false, account_params: false };
        let o = send(&cx, &session()).unwrap();
        assert_eq!(o.cmds, vec![
            json!({"type": "keep", "thread": "http://127.0.0.1:3820|t1700000002-3", "text": ""}),
            json!({"type": "send", "hub": "http://127.0.0.1:3820", "text": "{\"id\":2,\"m\":\"turn.start\",\"p\":{\"thread\":\"t1700000002-3\",\"text\":\"hello parity\",\"msg\":\"cparity-gpui1793864523-2\",\"mode\":\"steer\"}}"}),
            json!({"type": "scroll"})]);
        let p = &o.session_patch;
        assert_eq!(p["rid"], json!(3));
        assert_eq!(p["drafts"], json!({"t1700000002-3": "", "@att:t1700000002-3": "", "@reply:t1700000002-3": "", "@reply.q:t1700000002-3": ""}));
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
        for a in ["select", "row-pin", "skill", "interrupt", ""] { assert!(act(&p(a, "x"), &Plain).is_none(), "{a}") }
    }
    #[test] fn what_is_not_ported_is_none() {
        let cx = SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: false, account_params: false };
        let with = |k: &str, v: &str| { let mut s = session(); s["drafts"][k] = json!(v); s };
        assert!(send(&cx, &with("t1700000002-3", "   ")).is_none(), "blank");
        assert!(send(&cx, &with("t1700000002-3", "/btw why")).is_none(), "slash commands");
        assert!(send(&cx, &with("@att:t1700000002-3", "a")).is_none(), "attachments");
        assert!(send(&cx, &with("@reply:t1700000002-3", "m1")).is_none(), "replies");
        assert!(send(&cx, &with("@draft.bound", "5 abc")).is_none(), "a bounded draft");
        assert!(send(&SendCtx { bot_thread: true, ..SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: false, account_params: false } }, &session()).is_none(), "a bot's thread");
    }
}
