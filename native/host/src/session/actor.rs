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
pub struct ViewActor { view: String, target: Option<Value>, sel: Option<String>, keys: BTreeSet<String>, away: BTreeMap<String, String> }

/// The thread a target's draft and viewer belong to (V.Target.owner). Limits: a bot's thread (Target.thread) and a
/// project home's owner are not modelled, so those targets get no "@away.<t>" and no seeds.
fn owner(target: &Value) -> &str { if target["kind"] == "thread" { target["id"].as_str().unwrap_or("") } else { "" } }

impl ViewActor {
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
    let s = m.entry(client).or_default().reply(view, req, s);
    bound(&mut m, MAX_ACTORS);
    let mut out = pol.clone();
    out["session"] = s;
    out
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
