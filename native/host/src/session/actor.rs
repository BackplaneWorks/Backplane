//! The view actor's session bookkeeping (src/server/thin-actor.bend). The JS hub keeps one `Actor` per
//! connection: its own drafts and info persist between requests, and the reply's session is packed from that
//! actor, not from the picture's render copy. The native policy (native/bend/slim_serve.bend) is stateless: it
//! adopts the client's session onto a fresh Ui for each request, binds the target (`U.Bots.views`, which resets
//! `@bot.want`, `@thread.want`; `Ui.reselect` resets `@mech.at`, `@cmp.*`) and draws the page with
//! `@more`/`@away`/`@pill` set on the same Ui, so its reply session carries keys the actor never holds.
//! `ViewActor` reshapes that reply session into the actor's. One per connection, held by client id.
use serde_json::{json, Value};
use std::{collections::{BTreeSet, HashMap}, sync::{LazyLock, Mutex}};

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
pub struct ViewActor { target: Option<Value>, keys: BTreeSet<String> }

/// The thread a target's draft and viewer belong to (V.Target.owner; a project home's owner is not modelled: "").
fn owner(target: &Value) -> &str { if target["kind"] == "thread" { target["id"].as_str().unwrap_or("") } else { "" } }

impl ViewActor {
    /// `req` is the view request (target, action, away, session.drafts), `pol` the policy's session for it.
    pub fn reply(&mut self, req: &Value, pol: &Value) -> Value {
        let Some(mut session) = pol.as_object().cloned() else { return pol.clone() };
        let target = req["target"].clone();
        let first = self.target.is_none();
        let moved = self.target.as_ref().map_or(false, |t| t["kind"] != target["kind"] || t["id"] != target["id"]);
        let th = owner(&target).to_string();
        let mut drafts = session.get("drafts").and_then(Value::as_object).cloned().unwrap_or_default();
        // "@away.<t>" is the page's own mark (Actor.away.input): there only when the request carried one.
        drafts.retain(|k, _| !render_scratch(k) && !k.starts_with("@away."));
        if !th.is_empty() && !th.starts_with('@') { if let Some(a) = req["away"].as_str().filter(|a| a.chars().count() <= 128) { drafts.insert(format!("@away.{th}"), json!(a)); } }
        if !first {
            // Actor.scope.drafts: another target forgets what was asked for the last
            if moved { drafts.retain(|k, _| scope_kept(k) || !self.keys.contains(k) && !rebuilt(k)); }
            // A selection runs Ui.reselect in the actor itself, which sets them; any other request only sees them on the rebuilt Ui.
            if req["action"].as_str() != Some("select") { drafts.retain(|k, v| !(rebuilt(k) && v.as_str() == Some("") && (moved || !self.keys.contains(k)))); }
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
        session.insert("drafts".into(), Value::Object(drafts));
        if session.contains_key("info") { session.insert("info".into(), Value::Object(info)); }
        Value::Object(session)
    }
}

static ACTORS: LazyLock<Mutex<HashMap<u64, ViewActor>>> = LazyLock::new(Default::default);

/// The policy's answer with its session made the connection's actor's.
pub fn apply(client: u64, req: &Value, pol: &Value) -> Value {
    let Some(s) = pol.get("session").filter(|s| s.is_object()) else { return pol.clone() };
    let s = ACTORS.lock().unwrap().entry(client).or_default().reply(req, s);
    let mut out = pol.clone();
    out["session"] = s;
    out
}
/// A connection ended.
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
    fn sel(id: &str) -> Value { let mut r = req("recent", id); r["action"] = json!("select"); r }
    fn keys(v: &Value) -> Vec<String> { let mut k: Vec<String> = v["drafts"].as_object().unwrap().keys().cloned().collect(); k.sort(); k }

    #[test] fn first_reply_has_the_actors_constant_keys_and_no_render_scratch() {
        let mut a = ViewActor::default();
        let s = a.reply(&req("recent", ""), &pol("", &[("@more.t1", "1"), ("@pill", ""), ("@away.t1", ""), ("@sync", "1")]));
        assert_eq!(keys(&s), ["@btab", "@draft.bound", "@presentation", "@sync"]);
    }
    #[test] fn the_second_reply_seeds_the_shown_thread() {
        let mut a = ViewActor::default();
        a.reply(&req("recent", ""), &pol("", &[]));
        let s = a.reply(&req("recent", ""), &pol("", &[]));
        assert!(keys(&s).contains(&"@sch:".to_string()) && keys(&s).contains(&"@split.sheet:".to_string()));
        assert_eq!(s["info"]["view.tree:"], json!(""));
    }
    #[test] fn a_move_forgets_the_scoped_keys_and_the_rebuild_does_not_bring_them_back() {
        let mut a = ViewActor::default();
        a.reply(&req("recent", ""), &pol("", &[("@mech.at", ""), ("@bot.want", ""), ("@cmp.menu", "")]));
        let s = a.reply(&req("thread", "t1"), &pol("t1", &[("@mech.at", ""), ("@bot.want", ""), ("@thread.want", ""), ("@cmp.resolve::a", ""), ("@more.t1", "1")]));
        for k in ["@mech.at", "@bot.want", "@thread.want", "@cmp.resolve::a", "@more.t1"] { assert!(!keys(&s).contains(&k.to_string()), "{k}") }
        assert!(keys(&s).contains(&"@sch:t1".to_string()));
        let s2 = a.reply(&req("thread", "t1"), &pol("t1", &[("@mech.at", "")]));
        assert!(!keys(&s2).contains(&"@mech.at".to_string()));
    }
    #[test] fn a_selection_keeps_what_reselect_sets_and_a_draft_does_not() {
        let mut a = ViewActor::default();
        a.reply(&req("recent", ""), &pol("", &[]));
        let s = a.reply(&sel(""), &pol("", &[("@mech.at", ""), ("@cmp.menu", "")]));
        assert!(keys(&s).contains(&"@mech.at".to_string()));
        let mut d = req("recent", ""); d["action"] = json!("draft");
        let s2 = a.reply(&d, &pol("", &[("@bot.want", "")]));
        assert!(!keys(&s2).contains(&"@bot.want".to_string()));
    }
    #[test] fn an_action_that_sets_a_scoped_key_keeps_it() {
        let mut a = ViewActor::default();
        a.reply(&req("thread", "t1"), &pol("t1", &[]));
        let s = a.reply(&req("thread", "t1"), &pol("t1", &[("@mech.at", "t1#1")]));
        assert_eq!(s["drafts"]["@mech.at"], json!("t1#1"));
    }
    #[test] fn the_away_mark_is_the_requests() {
        let mut a = ViewActor::default();
        let mut r = req("thread", "t1"); r["away"] = json!("m5");
        let s = a.reply(&r, &pol("t1", &[("@away.t1", "")]));
        assert_eq!(s["drafts"]["@away.t1"], json!("m5"));
        assert!(!keys(&a.reply(&req("thread", "t1"), &pol("t1", &[("@away.t1", "")]))).contains(&"@away.t1".to_string()));
    }
}
