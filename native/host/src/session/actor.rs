//! The view actor's session bookkeeping (src/server/thin-actor.bend). The JS hub keeps one `Actor` per
//! connection: its own drafts and info persist between requests, and the reply's session is packed from that
//! actor, not from the picture's render copy. The native policy (native/bend/slim_serve.bend) is stateless: it
//! adopts the client's session onto a fresh Ui for each request, binds the target (`U.Bots.views`, which resets
//! `@bot.want`, `@thread.want`; `Ui.reselect` resets `@mech.at`, `@cmp.*`) and draws the page with
//! `@more`/`@away`/`@pill` set on the same Ui, so its reply session carries keys the actor never holds.
//! `ViewActor` reshapes that reply session into the actor's. One per connection, held by client id.
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet, HashMap}, sync::{LazyLock, Mutex}};

/// Actor.scope.keep (thin-actor.bend): false for the keys a move to another target forgets.
fn scope_kept(key: &str) -> bool {
    if key == "@f.pq" { return true }
    !(key.starts_with("@f.") || key.starts_with("@mr.") || key.starts_with("@cmp.resolve:") || SCOPED.contains(&key))
}
const SCOPED: [&str; 10] = ["@want.new", "@want.new.at", "@thread.want", "@bot.want", "@want.hub", "@want.gen", "@made.far", "@mech.at", "@fm.at", "@cmp.menu"];

/// Keys Target.bind / Ui.reselect set to "" on a Ui rebuilt for each request. An existing actor does not take
/// them from the fresh Ui (Actor.merge copies only Actor.local.key, @bot/@room/@remote/@btab/@sync and the
/// thread's draft), so an empty one the actor did not hold stays out.
fn rebuilt(key: &str) -> bool { key == "@mech.at" || key == "@cmp.menu" || key.starts_with("@cmp.resolve:") || key == "@bot.want" || key == "@thread.want" }

/// Render.ui (thin-view.bend) sets these on the copy it draws; the actor's own Ui never has them.
fn render_scratch(key: &str) -> bool { key.starts_with("@more.") || key == "@pill" }

/// The info keys Actor.seed.view gives a thread the actor has shown no viewer for (Actor.seed.key).
const VIEW_INFO: [&str; 5] = ["view.design:", "view.kind:", "view.path:", "view.sheet:", "view.tree:"];

#[derive(Default)]
pub struct ViewActor { view: String, target: Option<Value>, sel: Option<String>, keys: BTreeSet<String>, away: BTreeMap<String, String>, page: Option<(String, Value, u64)>, local: Option<(String, Value, Value)>,
    /// ids of the requests this view's commands told the client to send (`cmds` of type send), and of those the hub has since answered
    sent: BTreeSet<u64>, replied: BTreeSet<u64> }

/// The thread a target's draft and viewer belong to (V.Target.owner). Limits: a bot's thread (Target.thread) and a
/// project home's owner are not modelled, so those targets get no "@away.<t>" and no seeds.
fn owner(target: &Value) -> &str { if target["kind"] == "thread" { target["id"].as_str().unwrap_or("") } else { "" } }

impl ViewActor {
    /// The policy's per-view memory (`local`: the sidebar's page positions) for this request: what the last reply of the same
    /// view and target left, nothing after a view.open or a move.
    pub fn local_in(&self, view: &str, method: &str, target: &Value) -> Option<Value> {
        match &self.local { Some((v, t, l)) if method != "view.open" && v == view && t == target => Some(l.clone()), _ => None }
    }
    /// Keep the `local` a policy reply carried (and take it out of the reply, which the client never sees).
    pub fn local_out(&mut self, view: &str, target: &Value, pol: &mut Value) {
        let l = pol.as_object_mut().and_then(|o| o.remove("local"));
        self.local = l.filter(|l| l.is_object()).map(|l| (view.to_string(), target.clone(), l));
    }
    /// The older page this view stands on (`before`, 0 = the newest window). A view.page sets it from its cursor; later
    /// updates and actions of the same view and target keep it (the JS hub's actor holds the page it last served) until
    /// a view.open or another target ends it. Measured on recA/session-gpuid requests 41..45.
    pub fn page_at(&mut self, view: &str, method: &str, action: &str, target: &Value, asked: u64) -> u64 {
        let held = match &self.page { Some((v, t, b)) if v == view && t == target => *b, _ => 0 };
        // a sent message shows at the newest (Hub.thin.act.ordinary renders `send` and `send-alt` with anchor 0)
        let before = match method { "view.open" => 0, "view.page" => asked, _ if matches!(action, "send" | "send-alt") => 0, _ => held };
        self.page = if before == 0 { None } else { Some((view.to_string(), target.clone(), before)) };
        before
    }

    /// `view` is the view's id, `req` the view request (target, away, session.drafts), `pol` the policy's session.
    pub fn reply(&mut self, view: &str, req: &Value, pol: &Value) -> Value {
        let Some(mut session) = pol.as_object().cloned() else { return pol.clone() };
        let target = req["target"].clone();
        let first = self.target.is_none();
        // Actor.refresh `same`: the same view AND the same target; anything else is a navigation boundary.
        let moved = !first && (self.view != view || self.target.as_ref().map_or(false, |t| t["kind"] != target["kind"] || t["id"] != target["id"]));
        // Ui.reselect fires when the selection changes, whatever action did it.
        let sel = session.get("sel").and_then(Value::as_str).unwrap_or("").to_string();
        let reselected = self.sel.as_deref().map_or(true, |o| o != sel);
        let th = owner(&target).to_string();
        let mut drafts = session.get("drafts").and_then(Value::as_object).cloned().unwrap_or_default();
        drafts.retain(|k, _| !render_scratch(k) && !k.starts_with("@away."));
        // Actor.away.input: a request without the mark leaves it as it was; the marks are kept per thread (Actor.scope.keep keeps @away.*).
        if !th.is_empty() && !th.starts_with('@') { if let Some(a) = req["away"].as_str().filter(|a| a.chars().count() <= 128) { self.away.insert(th.clone(), a.to_string()); } }
        for (t, a) in &self.away { drafts.insert(format!("@away.{t}"), json!(a)); }
        if !first {
            // Actor.scope.drafts: another view or target forgets what was asked for the last
            if moved { drafts.retain(|k, _| scope_kept(k) || !self.keys.contains(k) && !rebuilt(k)); }
            // A changed selection ran Ui.reselect in the actor itself, which sets them; otherwise they are only on the rebuilt Ui.
            if !reselected { drafts.retain(|k, v| !(rebuilt(k) && v.as_str() == Some("") && (moved || !self.keys.contains(k)))); }
        }
        // Actor.bound.input, Session.input and Actor.selector always give these.
        let bound = req["session"]["drafts"]["@draft.bound"].as_str().filter(|b| b.chars().count() <= 128).unwrap_or("");
        drafts.insert("@draft.bound".into(), json!(bound));
        for k in ["@btab", "@presentation"] { drafts.entry(k).or_insert_with(|| json!("")); }
        // Rp.reply and Telemetry.reply (client.bend) run on the reply of every request the actor's own commands sent, and each leaves the request's
        // id behind, emptied ("@rp.req:<id>" is the page a review request asked for, cleared when its reply ends it: Rp.failed always sets it "").
        for id in &self.replied { drafts.insert(format!("@rp.req:{id}"), json!("")); drafts.insert(format!("@telemetry.req:{id}"), json!("")); }
        let mut info = session.get("info").and_then(Value::as_object).cloned().unwrap_or_default();
        // Actor.refresh: a thread (or none) this actor has shown no viewer for takes the page's record, "" when it has none.
        if !first && !self.keys.contains(&format!("view.kind:{th}")) {
            for k in ["@sch:", "@split.sheet:"] { drafts.entry(format!("{k}{th}")).or_insert_with(|| json!("")); }
            for k in VIEW_INFO { info.entry(format!("{k}{th}")).or_insert_with(|| json!("")); }
        }
        self.keys = drafts.keys().cloned().chain(info.keys().cloned()).collect();
        self.target = Some(target);
        self.view = view.to_string();
        self.sel = Some(sel);
        session.insert("drafts".into(), Value::Object(drafts));
        if session.contains_key("info") { session.insert("info".into(), Value::Object(info)); }
        Value::Object(session)
    }
}

static ACTORS: LazyLock<Mutex<HashMap<u64, ViewActor>>> = LazyLock::new(Default::default);

/// A connection that never says goodbye cannot grow the map past this: the oldest client ids go first.
const MAX_ACTORS: usize = 4096;

fn bound(m: &mut HashMap<u64, ViewActor>, keep: usize) {
    while m.len() > keep { let Some(oldest) = m.keys().min().copied() else { break }; m.remove(&oldest); }
}

/// The policy's answer with its session made the connection's actor's.
pub fn apply(client: u64, view: &str, req: &Value, pol: &Value) -> Value {
    let Some(s) = pol.get("session").filter(|s| s.is_object()) else { return pol.clone() };
    let mut m = ACTORS.lock().unwrap();
    let a = m.entry(client).or_default();
    for id in sent_ids(pol) { a.sent.insert(id); }
    let s = a.reply(view, req, s);
    bound(&mut m, MAX_ACTORS);
    let mut out = pol.clone();
    out["session"] = s;
    out
}
/// The ids of the requests a policy answer's `cmds` tell the client to send (`{"type":"send","text":"{\"id\":1,\"m\":..}"}`).
fn sent_ids(pol: &Value) -> Vec<u64> {
    pol.get("cmds").and_then(Value::as_array).into_iter().flatten().filter(|c| c["type"] == "send")
        .filter_map(|c| c["text"].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()).and_then(|j| j["id"].as_u64())).collect()
}
/// The hub answered request `id` of this connection: when it is one the view's commands sent, the actor now holds its marks (next reply's session).
pub fn replied(client: u64, id: u64) {
    if let Some(a) = ACTORS.lock().unwrap().get_mut(&client) { if a.sent.remove(&id) { a.replied.insert(id); } }
}
/// `local` to send the policy with this request.
pub fn local_in(client: u64, view: &str, method: &str, target: &Value) -> Option<Value> {
    ACTORS.lock().unwrap().get(&client).and_then(|a| a.local_in(view, method, target))
}
/// The policy's answer with `local` kept and removed.
pub fn local_out(client: u64, view: &str, target: &Value, pol: &mut Value) {
    ACTORS.lock().unwrap().entry(client).or_default().local_out(view, target, pol)
}
/// Only the hub sets `local` on a V request: a client's own field is dropped, then the held one (if any) is put in.
pub fn set_local(params: &mut Value, held: Option<Value>) {
    let Some(o) = params.as_object_mut() else { return };
    o.remove("local");
    if let Some(l) = held { o.insert("local".into(), l); }
}
/// `before` for this request: see `ViewActor::page_at`.
pub fn page(client: u64, view: &str, method: &str, action: &str, target: &Value, asked: u64) -> u64 {
    ACTORS.lock().unwrap().entry(client).or_default().page_at(view, method, action, target, asked)
}
/// A connection ended (`Hub::detach`, from the connection's drop guard).
pub fn forget(client: u64) { ACTORS.lock().unwrap().remove(&client); }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;
    fn pol(sel: &str, extra: &[(&str, &str)]) -> Value {
        let mut d = Map::new();
        for (k, v) in extra { d.insert(k.to_string(), json!(v)); }
        json!({"sel": sel, "drafts": d, "info": {}})
    }
    fn req(kind: &str, id: &str) -> Value { json!({"target": {"kind": kind, "id": id}, "session": {"drafts": {}}}) }
    fn keys(v: &Value) -> Vec<String> { let mut k: Vec<String> = v["drafts"].as_object().unwrap().keys().cloned().collect(); k.sort(); k }
    fn has(v: &Value, k: &str) -> bool { keys(v).contains(&k.to_string()) }

    /// law hist_picker_scope: Actor.scope.keep("@cmp.menu") == False (and the other scoped keys)
    // @rp.req:<id> and @telemetry.req:<id> (session parity, recA/session-mobile 30..): only for requests the view's own commands sent, only once answered
    #[test] fn answered_command_requests_leave_their_marks() {
        let cmds = json!({"cmds":[{"type":"focus","id":"composer"},{"type":"send","hub":"h","text":"{\"id\":1,\"m\":\"fab.look\",\"p\":{}}"},{"type":"send","hub":"h","text":"not json"}]});
        assert_eq!(sent_ids(&cmds), vec![1]);
        let mut a = ViewActor::default(); a.sent.insert(1);
        assert!(!has(&a.reply("v", &req("thread", "t1"), &pol("t1", &[])), "@rp.req:1"), "not before the reply");
        a.replied.insert(1);
        let s = a.reply("v", &req("thread", "t1"), &pol("t1", &[])); assert!(has(&s, "@rp.req:1") && has(&s, "@telemetry.req:1") && s["drafts"]["@rp.req:1"] == "");
        let s = a.reply("v", &req("thread", "t2"), &pol("t2", &[])); assert!(has(&s, "@rp.req:1"), "kept across a move (Actor.scope.keep)");
        replied(77, 1); // an unknown connection: nothing
    }
    #[test] fn hist_picker_scope() {
        assert!(!scope_kept("@cmp.menu"));
        for k in ["@cmp.resolve:t1:a", "@mech.at", "@thread.want", "@f.title", "@mr.x"] { assert!(!scope_kept(k), "{k}") }
        assert!(scope_kept("@f.pq") && scope_kept("@sync") && scope_kept("@away.t1"));
    }
    #[test] fn first_reply_has_the_actors_constant_keys_and_no_render_scratch() {
        let mut a = ViewActor::default();
        let s = a.reply("v1", &req("recent", ""), &pol("", &[("@more.t1", "1"), ("@pill", ""), ("@away.t1", ""), ("@sync", "1")]));
        assert_eq!(keys(&s), ["@btab", "@draft.bound", "@presentation", "@sync"]);
    }
    #[test] fn the_second_reply_seeds_the_shown_thread() {
        let mut a = ViewActor::default();
        a.reply("v1", &req("recent", ""), &pol("", &[]));
        let s = a.reply("v1", &req("recent", ""), &pol("", &[]));
        assert!(has(&s, "@sch:") && has(&s, "@split.sheet:"));
        assert_eq!(s["info"]["view.tree:"], json!(""));
    }
    #[test] fn a_move_forgets_the_scoped_keys_and_the_rebuild_does_not_bring_them_back() {
        let mut a = ViewActor::default();
        a.reply("v1", &req("recent", ""), &pol("", &[("@mech.at", ""), ("@bot.want", ""), ("@cmp.menu", "")]));
        let s = a.reply("v1", &req("thread", "t1"), &pol("t1", &[("@mech.at", ""), ("@bot.want", ""), ("@thread.want", ""), ("@cmp.resolve::a", ""), ("@more.t1", "1")]));
        // the selection changed here, so Ui.reselect's keys stay; the move still cleared the earlier ones
        assert!(has(&s, "@sch:t1") && !has(&s, "@more.t1"));
        let s2 = a.reply("v1", &req("thread", "t1"), &pol("t1", &[("@mech.at", "")]));
        assert!(!has(&s2, "@mech.at"));
    }
    #[test] fn a_new_view_on_the_same_target_clears_the_scoped_keys_and_forms() {
        let mut a = ViewActor::default();
        a.reply("v1", &req("thread", "t1"), &pol("t1", &[]));
        let s = a.reply("v1", &req("thread", "t1"), &pol("t1", &[("@f.title", "x"), ("@mech.at", "t1#1")]));
        assert!(has(&s, "@f.title") && has(&s, "@mech.at"));
        let s2 = a.reply("v2", &req("thread", "t1"), &pol("t1", &[("@f.title", "x"), ("@mech.at", "t1#1")]));
        assert!(!has(&s2, "@f.title") && !has(&s2, "@mech.at"));
    }
    #[test] fn a_changed_selection_keeps_what_reselect_sets_whatever_the_action() {
        let mut a = ViewActor::default();
        a.reply("v1", &req("recent", ""), &pol("", &[]));
        let mut r = req("recent", ""); r["action"] = json!("home-open");
        let s = a.reply("v1", &r, &pol("t9", &[("@mech.at", ""), ("@cmp.menu", "")]));
        assert!(has(&s, "@mech.at"));
        let mut d = req("recent", ""); d["action"] = json!("draft");
        let s2 = a.reply("v1", &d, &pol("t9", &[("@bot.want", "")]));
        assert!(!has(&s2, "@bot.want"));
    }
    #[test] fn an_action_that_sets_a_scoped_key_keeps_it() {
        let mut a = ViewActor::default();
        a.reply("v1", &req("thread", "t1"), &pol("t1", &[]));
        let s = a.reply("v1", &req("thread", "t1"), &pol("t1", &[("@mech.at", "t1#1")]));
        assert_eq!(s["drafts"]["@mech.at"], json!("t1#1"));
    }
    #[test] fn the_away_mark_stays_until_a_request_changes_it() {
        let mut a = ViewActor::default();
        let mut r = req("thread", "t1"); r["away"] = json!("m5");
        assert_eq!(a.reply("v1", &r, &pol("t1", &[("@away.t1", "")]))["drafts"]["@away.t1"], json!("m5"));
        assert_eq!(a.reply("v1", &req("thread", "t1"), &pol("t1", &[]))["drafts"]["@away.t1"], json!("m5"));
        let mut back = req("thread", "t1"); back["away"] = json!("");
        assert_eq!(a.reply("v1", &back, &pol("t1", &[]))["drafts"]["@away.t1"], json!(""));
    }
    #[test] fn a_page_position_holds_for_the_view_until_open_or_a_move() {
        let mut a = ViewActor::default();
        let t = json!({"kind": "thread", "id": "t1"});
        assert_eq!(a.page_at("v1", "view.update", "", &t, 0), 0);
        assert_eq!(a.page_at("v1", "view.page", "", &t, 131), 131);
        assert_eq!(a.page_at("v1", "view.update", "", &t, 0), 131);
        assert_eq!(a.page_at("v1", "view.action", "draft", &t, 0), 131);
        assert_eq!(a.page_at("v1", "view.page", "", &t, 115), 115);
        // a message sent from an older page shows at the newest, and stays there
        assert_eq!(a.page_at("v1", "view.action", "send", &t, 0), 0);
        assert_eq!(a.page_at("v1", "view.update", "", &t, 0), 0);
        assert_eq!(a.page_at("v1", "view.page", "", &t, 115), 115);
        assert_eq!(a.page_at("v1", "view.action", "send-alt", &t, 0), 0);
        assert_eq!(a.page_at("v1", "view.page", "", &t, 115), 115);
        assert_eq!(a.page_at("v1", "view.update", "", &json!({"kind": "thread", "id": "t2"}), 0), 0);
        assert_eq!(a.page_at("v1", "view.update", "", &t, 0), 0);
        a.page_at("v1", "view.page", "", &t, 99);
        assert_eq!(a.page_at("v1", "view.open", "", &t, 0), 0);
        assert_eq!(a.page_at("v1", "view.update", "", &t, 0), 0);
    }
    #[test] fn the_policys_local_memory_stays_with_the_view_and_never_reaches_the_client() {
        let mut a = ViewActor::default();
        let t = json!({"kind": "thread", "id": "t1"});
        assert!(a.local_in("v1", "view.update", &t).is_none());
        let mut reply = json!({"remoteReady": true, "local": {"sidebar": {"offset": 48}}, "session": {}});
        a.local_out("v1", &t, &mut reply);
        assert!(reply.get("local").is_none());
        assert_eq!(a.local_in("v1", "view.update", &t), Some(json!({"sidebar": {"offset": 48}})));
        assert!(a.local_in("v1", "view.open", &t).is_none());
        assert!(a.local_in("v2", "view.update", &t).is_none());
        assert!(a.local_in("v1", "view.update", &json!({"kind": "thread", "id": "t2"})).is_none());
        a.local_out("v1", &t, &mut json!({"remoteReady": true}));
        assert!(a.local_in("v1", "view.update", &t).is_none());
    }
    #[test] fn a_clients_own_local_never_reaches_the_policy() {
        let mut p = json!({"target": {}, "local": {"local.native.sidebar.pages": "x"}});
        set_local(&mut p, None);
        assert!(p.get("local").is_none());
        set_local(&mut p, Some(json!({"local.native.sidebar.pages": "held"})));
        assert_eq!(p["local"]["local.native.sidebar.pages"], json!("held"));
    }
    #[test] fn forget_and_the_bound_keep_the_map_flat() {
        let mut m: HashMap<u64, ViewActor> = HashMap::new();
        for c in 0..10_000u64 { m.entry(c).or_default(); bound(&mut m, MAX_ACTORS); }
        assert_eq!(m.len(), MAX_ACTORS);
        assert!(m.contains_key(&9_999) && !m.contains_key(&0));
        let req = req("recent", "");
        apply(900_001, "v1", &req, &json!({"session": pol("", &[])}));
        assert!(ACTORS.lock().unwrap().contains_key(&900_001));
        forget(900_001);
        assert!(!ACTORS.lock().unwrap().contains_key(&900_001));
    }
}
