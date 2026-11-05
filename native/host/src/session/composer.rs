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
