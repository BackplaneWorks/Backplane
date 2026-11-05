//! The thread-row actions of the view protocol, as pure functions over the session (client.bend `Ui.act` for `Menu.*`, `Ren.*` and `Made.*`):
//! `row-menu`, `proj-menu`, `menu-close`, `row-pin`, `row-unpin`, `row-archive`, `row-unarchive`, `row-settle`, `row-unsettle`, `row-snooze`,
//! `row-rename`, `rename-save`, `rename-no` and `new-thread`. An action answers what Bend's presenter answers: a patch of the session (its scratch
//! drafts `@menu`, `@del`, `@ren`..., and the request counter `rid`) and the commands the client runs (the `send` of the hub request, a `focus`).
//! Not the dispatcher: the selection (`select`, `nextTarget`) and everything shared by all actions are the session module's; a row action takes the
//! thread from its `value` and never selects. `None` is "not mine" (the helper answers). Oracle: test/gpui/commands/fixtures/actions.json, recorded
//! from bp-policy by test/gpui/commands/actions-fixture.mjs.
use serde_json::{json, Map, Value};

pub struct Req<'a> {
    /// the hub the client names in a `send` command
    pub hub: &'a str,
    /// the session the request carries (`sel`, `rid`, `drafts`, `info`...)
    pub session: &'a Value,
    pub action: &'a str,
    pub value: &'a str,
    /// a thread's title as the sidebar shows it (Active.title): the field a rename starts from
    pub title_of: &'a dyn Fn(&str) -> String,
}

#[derive(Debug, Default, PartialEq)]
pub struct ActOut {
    /// `{"rid": n?, "drafts": {...}}`: only what the action sets; the presenter merges it into the session
    pub session_patch: Value,
    pub cmds: Vec<Value>,
    pub next_target: Option<Value>,
}

fn draft<'a>(s: &'a Value, k: &str) -> &'a str { s["drafts"][k].as_str().unwrap_or("") }

struct Acc<'a> { r: &'a Req<'a>, d: Map<String, Value>, rid: u64, cmds: Vec<Value> }
impl<'a> Acc<'a> {
    fn new(r: &'a Req<'a>) -> Acc<'a> { Acc { r, d: Map::new(), rid: r.session["rid"].as_u64().unwrap_or(1), cmds: vec![] } }
    fn set(&mut self, k: &str, v: &str) { self.d.insert(k.into(), json!(v)) ; }
    /// Ui.rpc: the request goes out numbered, the counter moves on
    fn rpc(&mut self, m: &str, p: Value) {
        let text = json!({"id": self.rid, "m": m, "p": p}).to_string();
        self.cmds.push(json!({"type": "send", "hub": self.r.hub, "text": text})); self.rid += 1;
    }
    fn out(self) -> ActOut {
        let mut patch = Map::new();
        if self.rid != self.r.session["rid"].as_u64().unwrap_or(1) { patch.insert("rid".into(), json!(self.rid)); }
        patch.insert("drafts".into(), Value::Object(self.d));
        ActOut { session_patch: Value::Object(patch), cmds: self.cmds, next_target: None }
    }
    /// Ren.clear
    fn ren_clear(&mut self) { for k in ["@ren", "@ren.kind", "@ren.owner", "@ren.inline", "@f.title"] { self.set(k, "") } }
}

/// Menu.set: one menu at a time; the same pick closes it; a pick forgets an armed Delete or Remove.
fn menu_set(a: &mut Acc, m: &str) {
    let now = if draft(a.r.session, "@menu") == m { "" } else { m };
    a.set("@del", ""); a.set("@rm", ""); a.set("@menu", now);
}

pub fn act(r: &Req) -> Option<ActOut> {
    let (v, mut a) = (r.value, Acc::new(r));
    match r.action {
        "row-menu" => menu_set(&mut a, &format!("t:{v}")),
        "proj-menu" => menu_set(&mut a, &format!("p:{v}")),
        "menu-close" => { a.set("@menu", ""); a.set("@del", ""); a.set("@rm", "") }
        // Menu.shut after the request; any action but a Delete forgets an armed one
        "row-pin" | "row-unpin" | "row-archive" | "row-unarchive" | "row-settle" | "row-unsettle" => {
            let m = match r.action { "row-pin" => "thread.pin", "row-unpin" => "thread.unpin", "row-archive" => "thread.archive", "row-unarchive" => "thread.unarchive", "row-settle" => "thread.settle", _ => "thread.unsettle" };
            a.rpc(m, json!({"thread": v})); a.set("@del", ""); a.set("@menu", "");
        }
        // "<thread>|<for>": 1h, 1w, tomorrow or off
        "row-snooze" => {
            let (th, span) = v.split_once('|')?;
            a.rpc("thread.snooze", json!({"thread": th, "for": span})); a.set("@del", ""); a.set("@menu", "");
        }
        // Ren.open: the thread's title becomes the field, the menu shuts, the title gets the keyboard
        "row-rename" => {
            a.set("@del", ""); a.set("@ren.inline", ""); a.set("@menu", ""); a.set("@ren", v); a.set("@ren.kind", "thread");
            a.set("@ren.owner", r.session["sel"].as_str().unwrap_or("")); a.set("@f.title", &(r.title_of)(v));
            a.cmds.push(json!({"type": "focus", "id": "thread-title"}));
        }
        // the phones send "id\u{1f}title"; a blank id or title is never sent
        "rename-save" => {
            let (id, title) = if v.contains('\u{1f}') { let mut p = v.split('\u{1f}'); (p.next().unwrap_or("").to_string(), p.collect::<Vec<_>>().join(" ")) } else { (draft(r.session, "@ren").to_string(), draft(r.session, "@f.title").to_string()) };
            let title = title.trim().to_string();
            a.set("@del", "");
            if id.is_empty() || title.is_empty() { a.ren_clear() } else {
                let kind = draft(r.session, "@ren.kind").to_string();
                match kind.as_str() {
                    "project" => { a.ren_clear(); a.rpc("project.rename", json!({"project": id, "title": title})) }
                    "artifact" => return None, // a project setting: the helper's
                    _ => { a.ren_clear(); a.rpc("thread.rename", json!({"thread": id, "title": title})) }
                }
            }
        }
        "rename-no" => { a.set("@del", ""); a.ren_clear() }
        // Made.want + Ui.new_thread.project: the next request makes a thread; its reply selects it (the session module's)
        "new-thread" => {
            if v.is_empty() { return None } // no project named: the new-bot form, the helper's
            let rid = a.rid.to_string();
            let at = format!("{}|{}", r.session["info"]["local.machine"].as_str().unwrap_or(""), r.session["info"]["local.generation"].as_str().unwrap_or(""));
            a.set("@del", ""); a.set("@menu", ""); a.set("@want.viewer", ""); a.set("@want.new", &rid); a.set("@want.new.at", &at);
            a.rpc("thread.create", json!({"project": v, "title": "New thread"}));
        }
        _ => return None,
    }
    Some(a.out())
}

/// Merge a patch into a session (the presenter's job; here for the tests).
pub fn merge(session: &Value, patch: &Value) -> Value {
    let mut s = session.clone();
    if let Some(r) = patch.get("rid") { s["rid"] = r.clone() }
    if let (Some(d), Some(p)) = (s.get_mut("drafts").and_then(Value::as_object_mut), patch["drafts"].as_object()) { for (k, x) in p { d.insert(k.clone(), x.clone()); } }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIX: &str = include_str!("../../../../test/gpui/commands/fixtures/actions.json");
    // Bend's presenter answers each recorded action with this session change and these commands: same keys, same values, same requests
    #[test] fn every_recorded_row_action_is_answered_as_the_bend_presenter_answers_it() {
        let fx: Value = serde_json::from_str(FIX).unwrap();
        let titles = fx["titles"].clone();
        let title_of = |t: &str| titles[t].as_str().unwrap_or("").to_string();
        let (mut mine, mut passed) = (0, 0);
        for s in fx["steps"].as_array().unwrap() {
            let (action, value) = (s["action"].as_str().unwrap(), s["value"].as_str().unwrap());
            let req = Req { hub: "h", session: &s["session_in"], action, value, title_of: &title_of };
            let Some(o) = act(&req) else { passed += 1; assert!(matches!(action, "noop") || (action == "row-delete") || (action == "new-thread" && value.is_empty()), "{action} {value:?} fell through"); continue };
            mine += 1;
            let want = &s["session"]; let was = &s["session_in"];
            let got = merge(was, &o.session_patch);
            // every draft the presenter changed is changed the same way here, and nothing here disagrees with it
            for (k, x) in want["drafts"].as_object().unwrap() { if was["drafts"][k] != *x { assert_eq!(got["drafts"][k], *x, "{action} {value:?}: draft {k}") } }
            for (k, x) in o.session_patch["drafts"].as_object().unwrap() { assert_eq!(want["drafts"][k], *x, "{action} {value:?}: draft {k} set here but not so there") }
            assert_eq!(got["rid"], want["rid"], "{action} {value:?}: rid");
            assert_eq!(Value::Array(o.cmds.clone()), s["cmds"], "{action} {value:?}: cmds");
            assert_eq!(o.next_target, if s["nextTarget"].is_null() { None } else { Some(s["nextTarget"].clone()) }, "{action} {value:?}: nextTarget");
            passed += 1;
        }
        assert!(mine >= 22, "{mine} actions of the recorded ones are served here"); assert_eq!(passed, fx["steps"].as_array().unwrap().len());
    }
    #[test] fn a_rename_without_a_title_or_a_thread_sends_nothing_and_an_artifact_is_not_mine() {
        let s = json!({"sel":"","rid":4,"drafts":{"@ren":"t1","@f.title":"  ","@ren.kind":"thread"},"info":{}}); let t = |_: &str| String::new();
        let o = act(&Req { hub: "h", session: &s, action: "rename-save", value: "", title_of: &t }).unwrap(); assert!(o.cmds.is_empty()); assert!(o.session_patch.get("rid").is_none());
        let s2 = json!({"sel":"","rid":4,"drafts":{"@ren":"a","@f.title":"x","@ren.kind":"artifact"},"info":{}});
        assert!(act(&Req { hub: "h", session: &s2, action: "rename-save", value: "", title_of: &t }).is_none());
        assert!(act(&Req { hub: "h", session: &s, action: "row-snooze", value: "t1", title_of: &t }).is_none(), "a snooze without its span is not mine");
        let o = act(&Req { hub: "h", session: &json!({"sel":"","rid":4,"drafts":{},"info":{"local.machine":"m","local.generation":"7"}}), action: "new-thread", value: "p1", title_of: &t }).unwrap();
        assert_eq!(o.session_patch["drafts"]["@want.new.at"], "m|7"); assert_eq!(o.session_patch["drafts"]["@want.new"], "4");
    }
}
