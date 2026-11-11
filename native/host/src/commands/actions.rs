//! The thread-row actions of the view protocol, as pure functions over the session:
//! `row-menu`, `proj-menu`, `menu-close`, `row-pin`, `row-unpin`, `row-archive`, `row-unarchive`, `row-settle`, `row-unsettle`, `row-snooze`,
//! `row-rename`, `rename-save`, `rename-no` and `new-thread`, and the selected thread's own `pin`, `unpin`, `settle`, `unsettle`, `archive`, `unarchive`, `snooze`. An action answers what presenter answers: a patch of the session (its scratch
//! drafts `@menu`, `@del`, `@ren`..., and the request counter `rid`) and the commands the client runs (the `send` of the hub request, a `focus`).
//! Not the dispatcher: the selection (`select`, `nextTarget`) and everything shared by all actions are the session module's; a row action takes the
//! thread from its `value` and never selects. `None` is "not mine" (the helper answers). Oracle: test/commands/fixtures/actions.json, recorded
//! from bp-policy by test/commands/actions-fixture.mjs.
use serde_json::{json, Map, Value};

#[derive(Clone, Copy)]
pub struct Req<'a> {
    /// the hub the client names in a `send` command
    pub hub: &'a str,
    /// the session the request carries (`sel`, `rid`, `drafts`, `info`...)
    pub session: &'a Value,
    /// the thread the view shows (the request's target): the one the header's buttons and keys act on
    pub sel: &'a str,
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

struct Acc<'a> { r: &'a Req<'a>, d: Map<String, Value>, rid: u64, cmds: Vec<Value>, unselect: bool }
impl<'a> Acc<'a> {
    fn new(r: &'a Req<'a>) -> Acc<'a> { Acc { r, d: Map::new(), rid: r.session["rid"].as_u64().unwrap_or(1), cmds: vec![], unselect: false } }
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
        // Ui.unselect: the thread on show was deleted; back to the start (the dispatcher keeps a room or bot context it knows of)
        if self.unselect { patch.insert("sel".into(), json!("")); }
        ActOut { session_patch: Value::Object(patch), cmds: self.cmds, next_target: self.unselect.then(|| json!({"kind": "recent", "id": ""})) }
    }
    /// Ren.clear
    fn ren_clear(&mut self) { for k in ["@ren", "@ren.kind", "@ren.owner", "@ren.inline", "@f.title"] { self.set(k, "") } }
}

/// Menu.set: one menu at a time; the same pick closes it; a pick forgets an armed Delete or Remove.
fn menu_set(a: &mut Acc, m: &str) {
    let now = if draft(a.r.session, "@menu") == m { "" } else { m };
    a.set("@del", ""); a.set("@rm", ""); a.set("@menu", now);
}

/// A value qualified by its hub ("http://127.0.0.1:3834|t3", what the sidebar of a client with several hubs sends): the hub and the rest.
fn qualified(v: &str) -> Option<(&str, &str)> { if v.starts_with("http://") || v.starts_with("https://") { v.split_once('|') } else { None } }

pub fn act(req: &Req) -> Option<ActOut> {
    // a qualified id names its hub: the request goes there and the thread is the id (the routing of Rpc.text strips the prefix)
    let raw = req.value;
    let (hub, v) = qualified(raw).unwrap_or((req.hub, raw));
    let r = &Req { hub, value: v, ..*req };
    let mut a = Acc::new(r);
    match r.action {
        "row-menu" => menu_set(&mut a, &format!("t:{v}")),
        "proj-menu" => menu_set(&mut a, &format!("p:{v}")),
        "menu-close" => { a.set("@menu", ""); a.set("@del", ""); a.set("@rm", "") }
        // Menu.shut after the request; any action but a Delete forgets an armed one
        "row-pin" | "row-unpin" | "row-archive" | "row-unarchive" | "row-settle" | "row-unsettle" => {
            let m = match r.action { "row-pin" => "thread.pin", "row-unpin" => "thread.unpin", "row-archive" => "thread.archive", "row-unarchive" => "thread.unarchive", "row-settle" => "thread.settle", _ => "thread.unsettle" };
            a.rpc(m, json!({"thread": v})); a.set("@del", ""); a.set("@menu", "");
        }
        // the selected thread's own buttons and keys (Ui.on_thread); the thread is the view's
        "pin" | "unpin" | "archive" | "unarchive" | "settle" | "unsettle" => {
            let m = format!("thread.{}", r.action);
            a.rpc(&m, json!({"thread": r.sel})); a.set("@del", ""); a.set("@menu", "");
        }
        "snooze" => { a.rpc("thread.snooze", json!({"thread": r.sel, "for": v})); a.set("@del", ""); a.set("@menu", ""); }
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
        // a subthread to the top level of its project (thread.detach)
        "row-detach" => { if v.is_empty() { return None } a.rpc("thread.detach", json!({"thread": v})); a.set("@del", ""); a.set("@menu", ""); }
        // a thread under another of its project (`<thread>|<parent>`, thread.reparent; the hub refuses a cycle or another project)
        "row-reparent" => {
            let Some((th, parent)) = v.split_once('|') else { return None };
            if th.is_empty() || parent.is_empty() || th == parent { return None }
            a.rpc("thread.reparent", json!({"thread": th, "parent": parent})); a.set("@del", ""); a.set("@menu", "");
        }
        // the project's menu: Rename opens the same rename box a thread's does, on the project (rename-save sends project.rename)
        "proj-rename" => {
            if v.is_empty() { return None }
            a.set("@del", ""); a.set("@rm", ""); a.set("@ren.inline", ""); a.set("@menu", ""); a.set("@look", ""); a.set("@ren", v); a.set("@ren.kind", "project");
            a.set("@ren.owner", r.session["sel"].as_str().unwrap_or("")); a.set("@f.title", &(r.title_of)(v));
            a.cmds.push(json!({"type": "focus", "id": "thread-title"}));
        }
        // the project's badge editor (@look is the project it is open for): the same press shuts it; look-set writes `project.color.<id>=<tint>` or `project.icon.<id>=<icon>` and leaves it open
        "look" => { if v.is_empty() { return None } let now = if draft(r.session, "@look") == v { "" } else { v }; a.set("@look", now); a.set("@menu", ""); a.set("@del", ""); a.set("@rm", ""); }
        "look-set" => { let (key, val) = crate::present::badge::setting_of(v)?; a.rpc("setting.set", json!({"key": key, "value": val})); }
        // Pick again: the person's icon and colour go, and the hub's model picks anew (agents/iconpick.rs `project.pick`)
        "look-again" => { if v.is_empty() { return None } a.rpc("project.pick", json!({"project": v})); }
        // Regenerate title: the hub's title model writes a new one (thread.retitle)
        "row-retitle" => { a.rpc("thread.retitle", json!({"thread": v})); a.set("@del", ""); a.set("@menu", ""); }
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
        // Deleting asks first (Ui.row_delete / Ui.delete): the first press arms @del with what it was given, the same again deletes and goes back to the start when the thread was the selected one
        "row-delete" | "delete" => {
            let (id, armed_with) = if r.action == "delete" { (r.sel, r.sel) } else { (v, raw) };
            if !id.is_empty() && draft(r.session, "@del") == armed_with {
                a.set("@del", ""); a.rpc("thread.delete", json!({"thread": id})); a.set("@menu", "");
                a.unselect = id == r.sel;
            } else { a.set("@del", armed_with) } // the menu stays open while an armed Delete waits for its second press (Menu.keep)
        }
        // the project menu's Remove (the client asks twice) and a thread's New subthread (`<parent>|<project>`: the new thread is selected as New thread's is)
        // the machine picker's Clone it here (present/screen.rs `picker`): this hub clones the remote and adds it (net/registry.rs `project.clone`)
        "proj-clone" => { if v.is_empty() { return None } a.rpc("project.clone", json!({"origin": v})); a.set("@menu", ""); }
        // Link with… (a project with no remote): this project shows as one with the other (`project.link.<id>` = the other's key, present/merged.rs); "" unlinks
        "proj-link" => { let (id, key) = v.split_once('|')?; if id.is_empty() { return None } a.rpc("setting.set", json!({"key": format!("project.link.{id}"), "value": key})); a.set("@menu", ""); }
        "proj-remove" => { if v.is_empty() { return None } a.rpc("project.remove", json!({"project": v})); a.set("@menu", ""); a.set("@del", ""); }
        "thread-child" => {
            let (parent, project) = v.split_once('|')?;
            let rid = a.rid.to_string();
            let at = format!("{}|{}", r.session["info"]["local.machine"].as_str().unwrap_or(""), r.session["info"]["local.generation"].as_str().unwrap_or(""));
            a.set("@del", ""); a.set("@menu", ""); a.set("@want.viewer", ""); a.set("@want.new", &rid); a.set("@want.new.at", &at);
            a.rpc("thread.create", json!({"project": project, "title": "New thread", "parent": parent}));
        }
        "delete-no" => { a.set("@del", ""); a.set("@menu", "") }
        "rename-no" => { a.set("@del", ""); a.ren_clear() }
        // Made.want + Ui.new_thread.project: the next request makes a thread; its reply selects it (the session module's)
        "new-thread" => {
            if v.is_empty() { return None } // no project named: the new-bot form, the helper's
            let rid = a.rid.to_string();
            let at = format!("{}|{}", r.session["info"]["local.machine"].as_str().unwrap_or(""), r.session["info"]["local.generation"].as_str().unwrap_or(""));
            a.set("@del", ""); a.set("@menu", ""); a.set("@want.viewer", ""); a.set("@want.new", &rid); a.set("@want.new.at", &at);
            a.rpc("thread.create", json!({"project": v, "title": "New thread"}));
        }
        // Add project: the folder picker (present/picker.rs draws it from `@proj.open`, `@proj.text`). Typing and going into a folder set the text;
        // Add and Create send the request that makes the project (and its first thread, which the reply selects: Made.want).
        "picker-open" => { a.set("@proj.open", "1"); a.set("@proj.text", "~/"); a.set("@proj.err", ""); a.set("@menu", "") }
        "picker-type" => { a.set("@proj.text", v); a.set("@proj.err", "") }
        "proj-cd" => { let t = if v.ends_with('/') { v.to_string() } else { format!("{v}/") }; a.set("@proj.text", &t); a.set("@proj.err", "") }
        "proj-close" => { a.set("@proj.open", ""); a.set("@proj.text", ""); a.set("@proj.err", "") }
        "proj-add" | "proj-new" if !v.trim().is_empty() => {
            let rid = a.rid.to_string();
            let at = format!("{}|{}", r.session["info"]["local.machine"].as_str().unwrap_or(""), r.session["info"]["local.generation"].as_str().unwrap_or(""));
            a.set("@want.new", &rid); a.set("@want.new.at", &at);
            a.rpc(if r.action == "proj-add" { "project.add" } else { "project.new" }, json!({"path": v.trim()}));
            a.set("@proj.open", ""); a.set("@proj.text", ""); a.set("@proj.err", "");
        }
        _ => return None,
    }
    Some(a.out())
}

/// Merge a patch into a session (the presenter's job; here for the tests).
pub fn merge(session: &Value, patch: &Value) -> Value {
    let mut s = session.clone();
    if let Some(r) = patch.get("rid") { s["rid"] = r.clone() }
    if let Some(x) = patch.get("sel") { s["sel"] = x.clone() }
    if let (Some(d), Some(p)) = (s.get_mut("drafts").and_then(Value::as_object_mut), patch["drafts"].as_object()) { for (k, x) in p { d.insert(k.clone(), x.clone()); } }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIX: &str = include_str!("../../../../test/commands/fixtures/actions.json");
    #[test] fn the_project_menu_renames_edits_the_badge_and_a_subthread_goes_to_the_top() {
        let sess = json!({"rid": 3, "sel": "t1", "drafts": {}, "info": {}});
        let run = |action: &str, value: &str, session: &Value| act(&Req { hub: "h", session, sel: "t1", action, value, title_of: &|id| if id == "p1" { "Board".into() } else { String::new() } });
        let o = run("proj-rename", "p1", &sess).unwrap(); let d = &o.session_patch["drafts"];
        assert_eq!((d["@ren"].as_str(), d["@ren.kind"].as_str(), d["@f.title"].as_str()), (Some("p1"), Some("project"), Some("Board")));
        let mut s2 = sess.clone(); s2["drafts"] = json!({"@ren": "p1", "@ren.kind": "project", "@f.title": "Board 2"});
        let o = run("rename-save", "", &s2).unwrap(); assert!(o.cmds[0]["text"].as_str().unwrap().contains("\"project.rename\""));
        assert!(run("proj-rename", "", &sess).is_none());
        let o = run("look", "p1", &sess).unwrap(); assert_eq!(o.session_patch["drafts"]["@look"], "p1");
        let mut s3 = sess.clone(); s3["drafts"] = json!({"@look": "p1"}); assert_eq!(run("look", "p1", &s3).unwrap().session_patch["drafts"]["@look"], "", "the same press shuts it");
        let o = run("look-set", "project.color.p1=pink", &sess).unwrap();
        assert_eq!(o.cmds[0]["text"].as_str().unwrap(), "{\"id\":3,\"m\":\"setting.set\",\"p\":{\"key\":\"project.color.p1\",\"value\":\"pink\"}}");
        assert!(run("look-again", "p1", &sess).unwrap().cmds[0]["text"].as_str().unwrap().contains("\"m\":\"project.pick\",\"p\":{\"project\":\"p1\"}")); assert!(run("look-again", "", &sess).is_none());
        assert!(run("look-set", "agent.provider=codex", &sess).is_none() && run("look-set", "project.icon.p1=skull", &sess).is_none(), "only the badge settings, only listed values");
        let o = run("row-detach", "t2", &sess).unwrap(); assert_eq!(o.cmds[0]["text"].as_str().unwrap(), "{\"id\":3,\"m\":\"thread.detach\",\"p\":{\"thread\":\"t2\"}}");
        let o = run("row-reparent", "t2|t1", &sess).unwrap(); assert_eq!(o.cmds[0]["text"].as_str().unwrap(), "{\"id\":3,\"m\":\"thread.reparent\",\"p\":{\"thread\":\"t2\",\"parent\":\"t1\"}}");
        assert!(run("row-reparent", "t2", &sess).is_none() && run("row-reparent", "t2|t2", &sess).is_none() && run("row-reparent", "|t1", &sess).is_none(), "a move needs a thread and another parent");
    }
    // Add project: open, type, go in, add; the add sends project.add with the path and arms the want for the first thread; close clears
    #[test] fn the_folder_picker_actions_set_the_text_and_add_sends_the_request() {
        let sess = json!({"rid": 3, "drafts": {}, "info": {"local.machine": "m", "local.generation": "g"}});
        let run = |action: &str, value: &str, session: &Value| act(&Req { hub: "h", session, sel: "", action, value, title_of: &|_| String::new() }).unwrap();
        let o = run("picker-open", "", &sess); assert_eq!((o.session_patch["drafts"]["@proj.open"].as_str(), o.session_patch["drafts"]["@proj.text"].as_str()), (Some("1"), Some("~/")));
        assert_eq!(run("picker-type", "~/pro", &sess).session_patch["drafts"]["@proj.text"], "~/pro");
        assert_eq!(run("proj-cd", "/home/x/proj", &sess).session_patch["drafts"]["@proj.text"], "/home/x/proj/");
        let o = run("proj-add", "/home/x/proj", &sess); assert_eq!(o.cmds.len(), 1);
        let sent: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap(); assert_eq!((sent["m"].as_str(), sent["p"]["path"].as_str()), (Some("project.add"), Some("/home/x/proj")));
        assert_eq!((o.session_patch["drafts"]["@proj.open"].as_str(), o.session_patch["drafts"]["@want.new"].as_str()), (Some(""), Some("3")));
        assert_eq!(serde_json::from_str::<Value>(run("proj-new", "/home/x/n", &sess).cmds[0]["text"].as_str().unwrap()).unwrap()["m"], "project.new");
        assert!(act(&Req { hub: "h", session: &sess, sel: "", action: "proj-add", value: "  ", title_of: &|_| String::new() }).is_none(), "no path, no request");
        assert_eq!(run("proj-close", "", &sess).session_patch["drafts"]["@proj.open"], "");
    }
    // presenter answers each recorded action with this session change and these commands: same keys, same values, same requests
    #[test] fn every_recorded_row_action_is_answered_as_the_presenter_answers_it() {
        let fx: Value = serde_json::from_str(FIX).unwrap();
        let titles = fx["titles"].clone();
        let title_of = |t: &str| titles[t].as_str().unwrap_or("").to_string();
        let (mut mine, mut passed) = (0, 0);
        for s in fx["steps"].as_array().unwrap() {
            let (action, value) = (s["action"].as_str().unwrap(), s["value"].as_str().unwrap());
            let sel = s["target"]["id"].as_str().unwrap_or(""); let req = Req { hub: "h", session: &s["session_in"], sel, action, value, title_of: &title_of };
            let Some(o) = act(&req) else { passed += 1; assert!(matches!(action, "noop") || (action == "row-delete") || (action == "new-thread" && value.is_empty()), "{action} {value:?} fell through"); continue };
            mine += 1;
            let want = &s["session"]; let was = &s["session_in"];
            let got = merge(was, &o.session_patch);
            // The shared post-hook (Fab.sync / Dv.sync / Mpage.sync, the dispatcher's) may add `fab.look` requests after mine and set `@fab.at`; nothing else may differ.
            let want_cmds = s["cmds"].as_array().unwrap(); let mine_n = o.cmds.len();
            assert_eq!(&want_cmds[..mine_n], &o.cmds[..], "{action} {value:?}: cmds");
            let extra = &want_cmds[mine_n..];
            for c in extra { assert!(c["type"] == "send" && c["text"].as_str().unwrap().contains("\"m\":\"fab.look\""), "{action} {value:?}: unexpected extra command {c}") }
            for (k, x) in want["drafts"].as_object().unwrap() { if was["drafts"][k] != *x && k != "@fab.at" { assert_eq!(got["drafts"][k], *x, "{action} {value:?}: draft {k}") } }
            for (k, x) in o.session_patch["drafts"].as_object().unwrap() { assert_eq!(want["drafts"][k], *x, "{action} {value:?}: draft {k} set here but not so there") }
            assert_eq!(got["rid"].as_u64().unwrap() + extra.len() as u64, want["rid"].as_u64().unwrap(), "{action} {value:?}: rid");
            assert_eq!(o.next_target, if s["nextTarget"].is_null() { None } else { Some(s["nextTarget"].clone()) }, "{action} {value:?}: nextTarget");
            passed += 1;
        }
        assert!(mine >= 33, "{mine} actions of the recorded ones are served here"); assert_eq!(passed, fx["steps"].as_array().unwrap().len());
    }
    // Ui.row_delete / Ui.delete
    #[test] fn delete_asks_first_and_the_same_press_again_deletes_with_a_hubs_qualified_id_too() {
        let t = |_: &str| String::new();
        let step = |sess: &Value, sel: &str, action: &str, value: &str| { let o = act(&Req { hub: "h", session: sess, sel, action, value, title_of: &t }).unwrap(); (merge(sess, &o.session_patch), o) };
        let s0 = json!({"sel":"","rid":3,"drafts":{"@menu":"","@del":""},"info":{}});
        let q = "http://127.0.0.1:3834|t3";
        let (s1, o1) = step(&s0, "", "row-delete", q); assert_eq!(s1["drafts"]["@del"], q); assert!(o1.cmds.is_empty() && o1.next_target.is_none());
        let (s2, o2) = step(&s1, "", "row-delete", q);
        assert_eq!(o2.cmds, vec![json!({"type":"send","hub":"http://127.0.0.1:3834","text":"{\"id\":3,\"m\":\"thread.delete\",\"p\":{\"thread\":\"t3\"}}"})]); assert_eq!(s2["drafts"]["@del"], ""); assert_eq!(s2["rid"], 4); assert!(o2.next_target.is_none(), "not the selected thread: the selection stays");
        // another thread's press in between disarms nothing here but re-arms: only the identical value deletes
        let (s3, o3) = step(&s1, "", "row-delete", "http://127.0.0.1:3834|t4"); assert!(o3.cmds.is_empty()); assert_eq!(s3["drafts"]["@del"], "http://127.0.0.1:3834|t4");
        // the selected thread: deleting goes back to the start
        let (s4, _) = step(&s0, "t3", "row-delete", "t3"); let (s5, o5) = step(&s4, "t3", "row-delete", "t3");
        assert_eq!(o5.session_patch["sel"], ""); assert_eq!(o5.next_target, Some(json!({"kind":"recent","id":""}))); assert_eq!(s5["sel"], ""); assert_eq!(o5.cmds.len(), 1);
        // the header's delete arms with the selected thread and deletes on the second press; delete-no disarms
        let (h1, ho1) = step(&s0, "t3", "delete", ""); assert_eq!(h1["drafts"]["@del"], "t3"); assert!(ho1.cmds.is_empty());
        let (_, ho2) = step(&h1, "t3", "delete", ""); assert!(ho2.cmds[0]["text"].as_str().unwrap().contains("thread.delete")); assert_eq!(ho2.next_target, Some(json!({"kind":"recent","id":""})));
        let (n1, _) = step(&h1, "t3", "delete-no", ""); assert_eq!(n1["drafts"]["@del"], "");
        // nothing selected: the header's delete arms nothing and never deletes
        let (_, none) = step(&s0, "", "delete", ""); assert!(none.cmds.is_empty());
        // a row's other buttons take a qualified id too
        let (_, p) = step(&s0, "", "row-pin", q); assert_eq!(p.cmds[0]["hub"], "http://127.0.0.1:3834"); assert!(p.cmds[0]["text"].as_str().unwrap().contains("\"thread\":\"t3\""));
    }
    #[test] fn a_rename_without_a_title_or_a_thread_sends_nothing_and_an_artifact_is_not_mine() {
        let s = json!({"sel":"","rid":4,"drafts":{"@ren":"t1","@f.title":"  ","@ren.kind":"thread"},"info":{}}); let t = |_: &str| String::new();
        let o = act(&Req { hub: "h", session: &s, sel: "", action: "rename-save", value: "", title_of: &t }).unwrap(); assert!(o.cmds.is_empty()); assert!(o.session_patch.get("rid").is_none());
        let s2 = json!({"sel":"","rid":4,"drafts":{"@ren":"a","@f.title":"x","@ren.kind":"artifact"},"info":{}});
        assert!(act(&Req { hub: "h", session: &s2, sel: "", action: "rename-save", value: "", title_of: &t }).is_none());
        assert!(act(&Req { hub: "h", session: &s, sel: "", action: "row-snooze", value: "t1", title_of: &t }).is_none(), "a snooze without its span is not mine");
        let o = act(&Req { hub: "h", session: &json!({"sel":"","rid":4,"drafts":{},"info":{"local.machine":"m","local.generation":"7"}}), sel: "", action: "new-thread", value: "p1", title_of: &t }).unwrap();
        assert_eq!(o.session_patch["drafts"]["@want.new.at"], "m|7"); assert_eq!(o.session_patch["drafts"]["@want.new"], "4");
    }
}
