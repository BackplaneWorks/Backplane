//! The thread screen's own buttons, each Bend's `Ui.act` case (client.bend) as a pure function of the request's session: `answer`, `interrupt`, `copy`, `panel`, `dismiss`, `fork`,
//! `diff`, `git`, `term-toggle` and the phones' `find-open`. Each is None for what it does not own; the answer carries the session patch and the commands, and whether the shared
//! post-hook's `Fab.sync` tail follows (`look`). `Menu.after` (a non-menu action shuts an open menu unless it is the armed Delete's second press) is applied here for all of them.
//! `syncs_pending` is the other half of the post-hook: `Mpage.sync` / `Dv.sync` / `Fv.sync` add commands for the viewer on show, which are not ported: the dispatcher answers None then.
use super::dispatch::Facts;
use serde_json::{json, Value};

pub struct Out { pub session_patch: Value, pub cmds: Vec<Value>, pub look: bool }

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn draft<'a>(session: &'a Value, k: &str) -> &'a str { s(&session["drafts"], k) }
fn info<'a>(session: &'a Value, k: &str) -> &'a str { s(&session["info"], k) }

/// Mech.is_model: a STEP (.step, .stp) or a GLB, by extension without regard to case.
fn is_model(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    [".step", ".stp", ".glb"].iter().any(|e| l.ends_with(e))
}

/// Mpage.sync (the page on show asks for its parts when the thread or view.n moved from "@mech.at"), Dv.sync (an open Diff follows the thread's changes) and Fv.sync (a thread
/// elsewhere) have something to add for this session: the dispatcher cannot produce it.
pub fn syncs_pending(session: &Value) -> bool {
    let sel = s(session, "sel");
    if sel.is_empty() { return false }
    if sel.contains('~') { return true }
    let kind = info(session, &format!("view.kind:{sel}"));
    let mech = kind == "mech" || (kind == "3d" && is_model(info(session, &format!("view.path:{sel}"))));
    if mech && format!("{sel}#{}", info(session, &format!("view.n:{sel}"))) != draft(session, "@mech.at") { return true }
    session["panel"].as_bool().unwrap_or(false) && kind == "diff"
}

/// Hist.family: a version's or a comparison's live file ("git\t<gitdir>\t<commit>\t<len top>\t<top>/<rel>": the fifth tab field; "diff\n<old>\n<new>": the new one's), else the path.
fn family(p: &str) -> String {
    fn abs(p: &str) -> String { if p.starts_with("git\t") { p.split('\t').nth(4).unwrap_or("").to_string() } else { p.to_string() } }
    if p.starts_with("diff\n") { abs(p.split('\n').nth(2).unwrap_or("")) } else { abs(p) }
}

struct B<'a> { hub: &'a str, session: &'a Value, rid: u64, drafts: serde_json::Map<String, Value>, info: serde_json::Map<String, Value>, patch: serde_json::Map<String, Value>, cmds: Vec<Value> }
impl<'a> B<'a> {
    fn new(hub: &'a str, session: &'a Value) -> B<'a> { B { hub, session, rid: session["rid"].as_u64().unwrap_or(1), drafts: Default::default(), info: Default::default(), patch: Default::default(), cmds: vec![] } }
    fn rpc(&mut self, m: &str, p: Value) {
        let text = json!({"id": self.rid, "m": m, "p": p}).to_string();
        self.cmds.push(json!({"type": "send", "hub": self.hub, "text": text})); self.rid += 1;
    }
    fn out(mut self, look: bool) -> Out {
        // Menu.after: an open menu shuts after any action that does not keep it (the armed Delete's second press does, and no action here is one)
        if !draft(self.session, "@menu").is_empty() { self.drafts.insert("@menu".into(), json!("")); }
        if self.rid != self.session["rid"].as_u64().unwrap_or(1) { self.patch.insert("rid".into(), json!(self.rid)); }
        if !self.drafts.is_empty() { self.patch.insert("drafts".into(), Value::Object(self.drafts)); }
        if !self.info.is_empty() { self.patch.insert("info".into(), Value::Object(self.info)); }
        Out { session_patch: Value::Object(self.patch), cmds: self.cmds, look }
    }
}

pub fn act<F: Facts>(f: &F, hub: &str, session: &Value, sel: &str, action: &str, value: &str) -> Option<Out> {
    let mut b = B::new(hub, session);
    match action {
        // Ui.answer: "id|answer…" -> ask.answer {id, answer}; no value, no params
        "answer" => {
            let mut parts = value.split('|');
            let id = parts.next().unwrap_or("");
            let p = if value.is_empty() { json!({}) } else { json!({"id": id, "answer": parts.collect::<Vec<_>>().join("|")}) };
            b.rpc("ask.answer", p);
        }
        // Upload.start: a file about to upload is named in `@att.up:<thread>`; one over the cap shows the refusal instead
        "attach-up" => {
            if sel.is_empty() { return None }
            let v: Value = serde_json::from_str(value).unwrap_or(Value::Null);
            let (name, size) = (v["name"].as_str().filter(|n| !n.is_empty()).unwrap_or("file"), v["size"].as_u64().unwrap_or(0));
            let cap = f.attach_cap();
            if size > cap { b.patch.insert("error".into(), json!(crate::files::attach::failure(sel, "upload.cap", &(cap / 1048576).to_string())["text"])); }
            else { b.drafts.insert(format!("@att.up:{sel}"), json!(name)); }
        }
        // Upload.end: the hub's answer to the POST /attach; the path joins the thread's attachments (once), or the failure shows
        "attach-end" => {
            let v: Value = serde_json::from_str(value).ok().filter(Value::is_object).unwrap_or_else(|| crate::files::attach::failure(sel, "upload.failed", ""));
            let thread = v["thread"].as_str().unwrap_or("").rsplit('|').next().unwrap_or("").to_string();
            if thread.is_empty() || thread.starts_with('@') { return None }
            b.drafts.insert(format!("@att.up:{thread}"), json!(""));
            match v["attached"].as_str().filter(|p| !p.is_empty()) {
                Some(path) => {
                    let mut ps: Vec<String> = super::composer::att_paths(draft(session, &format!("@att:{thread}"))).into_iter().filter(|x| x != path).collect();
                    ps.push(path.to_string());
                    b.drafts.insert(format!("@att:{thread}"), json!(ps.join("\n")));
                }
                None if v["upload"].as_bool().unwrap_or(false) => { b.patch.insert("error".into(), json!(v["text"].as_str().unwrap_or("the upload failed"))); }
                None => {}
            }
        }
        // Att.drop: Remove on a chip
        "detach" => {
            if sel.is_empty() { return None }
            let ps: Vec<String> = super::composer::att_paths(draft(session, &format!("@att:{sel}"))).into_iter().filter(|x| x != value).collect();
            b.drafts.insert(format!("@att:{sel}"), json!(ps.join("\n")));
        }
        // ThreadTree.dispatch "thread-tree": a row's children opened or shut, kept as the setting `thread.tree.<id>` ("1"/"0"); value `<id>|1` opens, `<id>|0` shuts
        "thread-tree" => {
            let (id, open) = value.rsplit_once('|').unwrap_or((value, "1"));
            let id = id.rsplit('|').next().unwrap_or(id);
            if id.is_empty() || id.starts_with('@') || !matches!(open, "0" | "1") { return None }
            b.rpc("setting.set", json!({"key": format!("thread.tree.{id}"), "value": open}));
        }
        "interrupt" => { if sel.is_empty() { return None } b.rpc("turn.interrupt", json!({"thread": sel})) }
        "copy" => {}
        // Ui.toggle_panel: opened or shut by hand (the hub's own view leaves it alone now); shut forgets the Mech parts asked for
        "panel" => {
            let open = session["panel"].as_bool().unwrap_or(false);
            b.patch.insert("panel".into(), json!(!open));
            b.drafts.insert("@revert".into(), json!(""));
            if open { b.drafts.insert("@mech.at".into(), json!("")); }
            b.info.insert(format!("panel.hand:{sel}"), json!("1"));
        }
        // Ui.dismiss: the error line goes
        "dismiss" => { b.patch.insert("error".into(), json!("")); }
        // Dv.ask: Diff asked for by hand
        "diff" => {
            if sel.is_empty() { return None }
            b.info.insert(format!("diff.auto:{sel}"), json!(""));
            b.rpc("git.diff", json!({"thread": sel, "from": 0, "to": -1}));
        }
        "git" => { if sel.is_empty() { return None } b.rpc("git.act", json!({"thread": sel, "action": value})) }
        // Ui.term_toggle: shut (the composer gets the keyboard) or open (term.open with the size it has room for)
        "term-toggle" => {
            if sel.is_empty() { return None }
            if session["open"]["term"].as_bool().unwrap_or(false) {
                b.patch.insert("open".into(), json!({"term": false}));
                b.cmds.push(json!({"type": "focus", "id": "composer"}));
            } else {
                // Term.size.k: two parts each read on their own (a part that is not a number is 100 / 24); fewer than two parts is 100x24
                let parts: Vec<&str> = value.split('x').collect();
                let (c, r) = if parts.len() >= 2 { (parts[0].parse::<u32>().unwrap_or(100), parts[1].parse::<u32>().unwrap_or(24)) } else { (100, 24) };
                b.patch.insert("open".into(), json!({"term": true}));
                b.rpc("term.open", json!({"thread": sel, "cols": c.max(20), "rows": r.max(5)}));
                b.cmds.push(json!({"type": "focus", "id": "term"}));
            }
        }
        // Ui.fork.with: the next request makes a thread; the viewer on show is recorded for it (Viewer.record)
        "fork" => {
            let th = if value.is_empty() { sel } else { value };
            if th.is_empty() || th.starts_with("http") || th.contains('~') || th.starts_with("project-home-") || th.starts_with('@') || !crate::session::select::Facts::thread_exists(f, th) { return None }
            let open = if th == sel { if session["panel"].as_bool().unwrap_or(false) { "1" } else { "" } } else { info(session, &format!("panel:{th}")) };
            let live = |k: &str| family(info(session, &format!("{k}:{th}")));
            let record = json!({"open": open, "thread": th, "root": f.root_of(th), "kind": info(session, &format!("view.kind:{th}")),
                "path": live("view.path"), "design": live("view.design"), "tree": live("view.tree"), "drawingSheet": live("view.sheet"),
                "sheet": draft(session, &format!("@sch:{th}")), "split": draft(session, &format!("@split.sheet:{th}"))}).to_string();
            b.drafts.insert("@want.viewer".into(), json!(record));
            b.drafts.insert("@want.new".into(), json!(b.rid.to_string()));
            b.drafts.insert("@want.new.at".into(), json!(format!("{}|{}", info(session, "local.machine"), info(session, "local.generation"))));
            b.rpc("thread.fork", json!({"thread": th}));
        }
        // the phones' thread search / file picker: the palette opens (files also asks for the thread's files, through the table)
        "find-open" => {
            if value != "search" && value != "files" { return None }
            b.drafts.insert("@palette".into(), json!("")); b.drafts.insert("@palette.i".into(), json!("0")); b.drafts.insert("@palette.mode".into(), json!(value));
            b.patch.insert("open".into(), json!({"palette": true}));
            if value == "files" {
                if sel.is_empty() { return None }
                b.rpc("files.list", json!({"thread": sel}));
                return Some(b.out(true));
            }
            return Some(b.out(false)); // search: Act{Palette.open, Nil}, no post-hook
        }
        _ => return None,
    }
    Some(b.out(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{composer, select};
    struct F;
    impl composer::Facts for F { fn bot_thread(&self, _: &str) -> bool { false } fn account_params(&self, _: &str) -> bool { false } }
    impl select::Facts for F {
        fn thread_exists(&self, id: &str) -> bool { id.starts_with('t') } fn turn_name(&self, _: &str) -> String { "idle".into() }
        fn order_enabled(&self) -> bool { true } fn estimates_on(&self) -> bool { true }
    }
    impl Facts for F { fn title_of(&self, _: &str) -> String { String::new() } fn root_of(&self, _: &str) -> String { "/w/r".into() } }
    #[test] fn a_row_chevron_opens_and_shuts_its_children() {
        let s = json!({"sel": "t9", "rid": 4, "drafts": {}, "info": {}});
        let o = act(&F, "h", &s, "t9", "thread-tree", "h|t1|1").unwrap();
        assert_eq!(o.cmds[0]["text"], json!(r#"{"id":4,"m":"setting.set","p":{"key":"thread.tree.t1","value":"1"}}"#));
        let o = act(&F, "h", &s, "", "thread-tree", "t1|0").unwrap();
        assert!(o.cmds[0]["text"].as_str().unwrap().contains(r#""thread.tree.t1","value":"0""#));
        assert!(act(&F, "h", &s, "t9", "thread-tree", "t1|2").is_none());
    }

    fn merged(s: &Value, o: Out) -> Value { let mut s = s.clone(); crate::session::patch::merge(&mut s, &o.session_patch); s }
    #[test] fn a_pasted_picture_uploads_attaches_sends_and_detaches() {
        let s0 = json!({"sel": "t1", "rid": 3, "drafts": {"t1": "look"}, "info": {}});
        let s1 = merged(&s0, act(&F, "h", &s0, "t1", "attach-up", r#"{"name":"pasted-1.png","size":10}"#).unwrap());
        assert_eq!(s1["drafts"]["@att.up:t1"], "pasted-1.png");
        let s2 = merged(&s1, act(&F, "h", &s1, "t1", "attach-end", r#"{"attached":"/a/t1/k9-pasted-1.png","thread":"t1"}"#).unwrap());
        assert_eq!((s2["drafts"]["@att.up:t1"].as_str(), s2["drafts"]["@att:t1"].as_str()), (Some(""), Some("/a/t1/k9-pasted-1.png")));
        let cx = composer::SendCtx { hub: "h", cid: "c", now_s: 1, bot_thread: false, binding: None };
        let sent = composer::send(&cx, &s2).unwrap();
        assert!(sent.cmds[1]["text"].as_str().unwrap().contains(r"look\n\n[attachment: /a/t1/k9-pasted-1.png]"), "{:?}", sent.cmds[1]);
        let s3 = merged(&s2, act(&F, "h", &s2, "t1", "detach", "/a/t1/k9-pasted-1.png").unwrap());
        assert_eq!(s3["drafts"]["@att:t1"], "");
        let s4 = merged(&s0, act(&F, "h", &s0, "t1", "attach-end", r#"{"thread":"t1","upload":true,"text":"the upload failed","uiError":"upload.failed"}"#).unwrap());
        assert_eq!(s4["error"], "the upload failed");
        let over = act(&F, "h", &s0, "t1", "attach-up", &format!(r#"{{"name":"b","size":{}}}"#, 600u64 << 20)).unwrap();
        assert!(over.session_patch["error"].as_str().unwrap().contains("512 MB"));
    }    const T: &str = "t1700000002-3";
    fn sess() -> Value { json!({"sel": T, "rid": 5, "panel": false, "drafts": {}, "info": {}, "open": {}}) }
    fn run(a: &str, v: &str, s: Value) -> Option<Out> { act(&F, "h", &s, T, a, v) }
    fn text(o: &Out, i: usize) -> String { o.cmds[i]["text"].as_str().unwrap().to_string() }

    #[test] fn panel_toggles_and_remembers_the_hand() {
        let o = run("panel", "", sess()).unwrap();
        assert_eq!(o.session_patch["panel"], json!(true)); assert_eq!(o.session_patch["info"][format!("panel.hand:{T}")], json!("1")); assert!(o.cmds.is_empty());
        assert!(o.session_patch["drafts"].get("@mech.at").is_none());
        let mut s = sess(); s["panel"] = json!(true);
        let o = run("panel", "", s).unwrap();
        assert_eq!(o.session_patch["panel"], json!(false)); assert_eq!(o.session_patch["drafts"]["@mech.at"], json!(""));
    }
    #[test] fn an_open_menu_shuts_after_the_action() {
        let mut s = sess(); s["drafts"]["@menu"] = json!("t:x");
        assert_eq!(run("dismiss", "", s).unwrap().session_patch["drafts"]["@menu"], json!(""));
        assert_eq!(run("dismiss", "", sess()).unwrap().session_patch["error"], json!(""));
    }
    #[test] fn diff_and_git_are_one_request_each() {
        let o = run("diff", "", sess()).unwrap();
        assert_eq!(text(&o, 0), "{\"id\":5,\"m\":\"git.diff\",\"p\":{\"thread\":\"t1700000002-3\",\"from\":0,\"to\":-1}}");
        assert_eq!(o.session_patch["info"][format!("diff.auto:{T}")], json!("")); assert_eq!(o.session_patch["rid"], json!(6));
        let o = run("git", "commit", sess()).unwrap();
        assert_eq!(text(&o, 0), "{\"id\":5,\"m\":\"git.act\",\"p\":{\"thread\":\"t1700000002-3\",\"action\":\"commit\"}}");
    }
    #[test] fn the_terminal_opens_with_its_size_and_shuts_to_the_composer() {
        let o = run("term-toggle", "80x30", sess()).unwrap();
        assert_eq!(text(&o, 0), "{\"id\":5,\"m\":\"term.open\",\"p\":{\"thread\":\"t1700000002-3\",\"cols\":80,\"rows\":30}}");
        assert_eq!(o.cmds[1], json!({"type": "focus", "id": "term"})); assert_eq!(o.session_patch["open"]["term"], json!(true));
        assert!(text(&run("term-toggle", "5x2", sess()).unwrap(), 0).contains("\"cols\":20,\"rows\":5"), "at least 20x5");
        assert!(text(&run("term-toggle", "", sess()).unwrap(), 0).contains("\"cols\":100,\"rows\":24"));
        let mut s = sess(); s["open"]["term"] = json!(true);
        let o = run("term-toggle", "80x30", s).unwrap();
        assert_eq!(o.cmds, vec![json!({"type": "focus", "id": "composer"})]); assert_eq!(o.session_patch["open"]["term"], json!(false));
    }
    #[test] fn a_fork_records_the_viewer_and_waits_for_the_new_thread() {
        let mut s = sess(); s["panel"] = json!(true);
        s["info"][format!("view.kind:{T}")] = json!("board"); s["info"][format!("view.path:{T}")] = json!("git\t/g\tabc\t2\t/w/b.kicad_pcb");
        s["info"]["local.machine"] = json!("m"); s["info"]["local.generation"] = json!("g1");
        let o = run("fork", "", s).unwrap();
        assert_eq!(text(&o, 0), "{\"id\":5,\"m\":\"thread.fork\",\"p\":{\"thread\":\"t1700000002-3\"}}");
        let d = &o.session_patch["drafts"];
        assert_eq!(d["@want.new"], json!("5")); assert_eq!(d["@want.new.at"], json!("m|g1"));
        let rec: Value = serde_json::from_str(d["@want.viewer"].as_str().unwrap()).unwrap();
        assert_eq!((rec["open"].clone(), rec["root"].clone(), rec["kind"].clone()), (json!("1"), json!("/w/r"), json!("board")));
        assert_eq!(rec["path"], json!("/w/b.kicad_pcb"), "a version's live file");
        assert!(run("fork", "http://h|t9", sess()).is_none());
    }
    #[test] fn find_open_modes() {
        let o = run("find-open", "search", sess()).unwrap();
        assert!(!o.look && o.cmds.is_empty() && o.session_patch["open"]["palette"] == json!(true) && o.session_patch["drafts"]["@palette.mode"] == json!("search"));
        let o = run("find-open", "files", sess()).unwrap();
        assert!(o.look); assert!(text(&o, 0).contains("\"m\":\"files.list\""));
        assert!(run("find-open", "x", sess()).is_none());
    }
    #[test] fn syncs_are_pending_for_a_mech_page_not_yet_asked_a_diff_on_show_and_far_threads() {
        let mut s = sess();
        assert!(!syncs_pending(&s));
        s["info"][format!("view.kind:{T}")] = json!("mech"); s["info"][format!("view.n:{T}")] = json!("2");
        assert!(syncs_pending(&s)); s["drafts"]["@mech.at"] = json!(format!("{T}#2")); assert!(!syncs_pending(&s));
        s["info"][format!("view.kind:{T}")] = json!("3d"); s["info"][format!("view.path:{T}")] = json!("/p/a.STEP"); s["drafts"]["@mech.at"] = json!("");
        assert!(syncs_pending(&s));
        let mut d = sess(); d["panel"] = json!(true); d["info"][format!("view.kind:{T}")] = json!("diff");
        assert!(syncs_pending(&d)); d["panel"] = json!(false); assert!(!syncs_pending(&d));
        let mut far = sess(); far["sel"] = json!("l~t1"); assert!(syncs_pending(&far));
    }
}
