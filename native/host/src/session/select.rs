//! The `select` action (src/core/client.bend: `Ui.select` -> `Ui.select.nav`, `Ui.reselect`, the focus and scroll commands, then the post-hook `Fab.sync`): a pure function
//! over the request's session, returning the session patch, the commands and the target the view moves to (`Target.after`). Ported: a local thread picked with no viewer open and no
//! pending draft check. Not ported (None; the caller falls back to the helper): project homes, far threads (`~`), a thread whose viewer panel is open (`panel:<id>`: Viewer restore, Dv/Mpage
//! syncs), a draft marked read back from disk (`@kept:<id>`: Draft.stale), no thread named.
use serde_json::{json, Map, Value};

/// What the model says: the presenter implements it over its metadata projection.
pub trait Facts {
    fn thread_exists(&self, id: &str) -> bool;
    /// Fab.turn: "" for a thread with no project, else the turn's name (C.Turn.name: idle, running, ...).
    fn turn_name(&self, id: &str) -> String;
    /// Fab.enabled (Ordering on) and the setting `order.estimates` is not "off".
    fn order_enabled(&self) -> bool;
    fn estimates_on(&self) -> bool;
}

pub struct SelectOut { pub session_patch: Value, pub cmds: Vec<Value>, pub next_target: Value }

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }

/// Fab.at: what was last asked for, the thread and where its turn stands.
pub fn fab_at(f: &dyn Facts, id: &str) -> String {
    format!("{}{}#{}#{}", if f.order_enabled() { "on" } else { "off" }, if f.estimates_on() { "on" } else { "off" }, id, f.turn_name(id))
}

/// `value` is `<hub>|<id>` (the id alone is accepted too); `hub` is the request's hub (the `send` command names it).
pub fn select(f: &dyn Facts, hub: &str, session: &Value, value: &str) -> Option<SelectOut> {
    let id = value.rsplit('|').next().unwrap_or("");
    let (old, d, info) = (s(session, "sel"), &session["drafts"], &session["info"]);
    if id.is_empty() || id.contains('~') || id.starts_with("project-home-") || id.starts_with('@') || !f.thread_exists(id) { return None }
    // (a panel to restore is the helper's, except Diff and Review, which come back with their thread: pages.rs)
    let own_panel = matches!(s(info, &format!("view.kind:{id}")), "diff" | "review");
    if (s(info, &format!("panel:{id}")) == "1" && !own_panel) || !s(d, &format!("@kept:{id}")).is_empty() { return None }
    let rid = session["rid"].as_u64().unwrap_or(0);
    let mut drafts = Map::new();
    // Ui.select.k
    for k in ["@revert", "@del", "@bot", "@room", "@remote", "@bot.want", "@thread.want", "@intent"] { drafts.insert(k.into(), json!("")); }
    let mut info_patch = Map::new();
    // Ui.reselect: only when the selection changes
    if old != id {
        for k in ["@mech.at".to_string(), "@cmp.menu".into(), format!("@cmp.resolve:{old}:a"), format!("@cmp.resolve:{old}:b")] { drafts.insert(k, json!("")); }
        info_patch.insert(format!("panel:{old}"), json!(if session["panel"].as_bool().unwrap_or(false) { "1" } else { "" }));   // Viewer.flag
        info_patch.insert(format!("hist.play:{old}"), json!(""));
    }
    let mut cmds = vec![json!({"type": "focus", "id": "composer"}), json!({"type": "scroll"})];
    // Fab.sync (Fab.want): a look is asked for when where the thread stands changed since the last ask
    let at = fab_at(f, id);
    let want = at != s(d, "@fab.at") && !at.ends_with('#') && (!at.ends_with("#running") || s(info, &format!("fab.line:{id}")).is_empty());
    let mut next_rid = rid;
    if want {
        drafts.insert("@fab.at".into(), json!(at));
        cmds.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"fab.look\",\"p\":{{\"thread\":{}}}}}", serde_json::to_string(id).ok()?)}));
        next_rid += 1;
    }
    let mut patch = json!({"sel": id, "drafts": drafts, "info": info_patch, "open": {"settings": false}});
    // Ui.reselect: the new thread's panel is the one it left
    if old != id && session["panel"].as_bool().unwrap_or(false) != (s(info, &format!("panel:{id}")) == "1") { patch["panel"] = json!(s(info, &format!("panel:{id}")) == "1"); }
    if want { patch["rid"] = json!(next_rid) }
    Some(SelectOut { session_patch: patch, cmds, next_target: json!({"kind": "thread", "id": id}) })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct M { running: bool }
    impl Facts for M {
        fn thread_exists(&self, id: &str) -> bool { id == "t1700000002-3" }
        fn turn_name(&self, _: &str) -> String { if self.running { "running".into() } else { "idle".into() } }
        fn order_enabled(&self) -> bool { true }
        fn estimates_on(&self) -> bool { true }
    }
    // recorded by the JS hub (parity-960s/recA session-gpui): select from "recent" with rid 1
    fn session() -> Value { json!({"sel": "", "rid": 1, "panel": false, "drafts": {"@boot": ""}, "info": {}}) }
    const HUB: &str = "http://127.0.0.1:3820";
    #[test] fn select_is_what_the_js_hub_answered() {
        let o = select(&M { running: false }, HUB, &session(), "http://127.0.0.1:3820|t1700000002-3").unwrap();
        assert_eq!(o.cmds, vec![json!({"type": "focus", "id": "composer"}), json!({"type": "scroll"}),
            json!({"type": "send", "hub": HUB, "text": "{\"id\":1,\"m\":\"fab.look\",\"p\":{\"thread\":\"t1700000002-3\"}}"})]);
        assert_eq!(o.next_target, json!({"kind": "thread", "id": "t1700000002-3"}));
        let p = &o.session_patch;
        assert_eq!((p["sel"].as_str(), p["rid"].as_u64()), (Some("t1700000002-3"), Some(2)));
        let d = &p["drafts"];
        assert_eq!(d["@fab.at"], json!("onon#t1700000002-3#idle"));
        for k in ["@cmp.menu", "@cmp.resolve::a", "@cmp.resolve::b", "@intent", "@revert", "@del"] { assert_eq!(d[k], json!(""), "{k}") }
        assert_eq!(p["info"]["panel:"], json!(""));
        assert_eq!(p["open"]["settings"], json!(false));
    }
    #[test] fn the_same_place_asks_for_no_look() {
        let mut s = session(); s["sel"] = json!("t1700000002-3"); s["drafts"]["@fab.at"] = json!("onon#t1700000002-3#idle");
        let o = select(&M { running: false }, HUB, &s, "t1700000002-3").unwrap();
        assert_eq!(o.cmds.len(), 2, "focus and scroll only");
        assert!(o.session_patch.get("rid").is_none() && o.session_patch["drafts"].get("@fab.at").is_none());
        assert_eq!(o.session_patch["info"], json!({}), "same selection: no reselect");
    }
    #[test] fn a_running_turn_with_a_card_waits() {
        let mut s = session(); s["info"]["fab.line:t1700000002-3"] = json!("a goal");
        let o = select(&M { running: true }, HUB, &s, "t1700000002-3").unwrap();
        assert_eq!(o.cmds.len(), 2, "Fab.want: not while running with a card already shown");
        let s2 = session();
        assert_eq!(select(&M { running: true }, HUB, &s2, "t1700000002-3").unwrap().cmds.len(), 3, "but a running thread with no card yet is looked up");
    }
    #[test] fn what_is_not_ported_is_none() {
        let f = M { running: false };
        for v in ["", "h|link~t9", "h|project-home-p1", "h|@x", "h|unknown"] { assert!(select(&f, HUB, &session(), v).is_none(), "{v}") }
        let mut s = session(); s["info"]["panel:t1700000002-3"] = json!("1");
        assert!(select(&f, HUB, &s, "t1700000002-3").is_none(), "a viewer panel to restore");
        s["info"]["view.kind:t1700000002-3"] = json!("diff"); s["sel"] = json!("tother");
        assert_eq!(select(&f, HUB, &s, "t1700000002-3").unwrap().session_patch["panel"], json!(true), "Diff and Review come back with their thread");
        let mut k = session(); k["drafts"]["@kept:t1700000002-3"] = json!("x");
        assert!(select(&f, HUB, &k, "t1700000002-3").is_none(), "Draft.stale");
    }
}
