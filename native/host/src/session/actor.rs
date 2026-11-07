//! The view actor's session bookkeeping. The JS hub keeps one `Actor` per
//! connection: its own drafts and info persist between requests, and the reply's session is packed from that
//! actor, not from the picture's render copy. The native policy is stateless: it
//! adopts the client's session onto a fresh Ui for each request, binds the target (`U.Bots.views`, which resets
//! `@bot.want`, `@thread.want`; `Ui.reselect` resets `@mech.at`, `@cmp.*`) and draws the page with
//! `@more`/`@away`/`@pill` set on the same Ui, so its reply session carries keys the actor never holds.
//! `ViewActor` reshapes that reply session into the actor's. One per connection, held by client id.
use serde_json::{json, Map, Value};
use std::{collections::{BTreeMap, BTreeSet, HashMap}, sync::{LazyLock, Mutex}};

/// Actor.scope.keep: false for the keys a move to another target forgets.
fn scope_kept(key: &str) -> bool {
    if key == "@f.pq" { return true }
    !(key.starts_with("@f.") || key.starts_with("@mr.") || key.starts_with("@cmp.resolve:") || SCOPED.contains(&key))
}
const SCOPED: [&str; 10] = ["@want.new", "@want.new.at", "@thread.want", "@bot.want", "@want.hub", "@want.gen", "@made.far", "@mech.at", "@fm.at", "@cmp.menu"];

/// Keys Target.bind / Ui.reselect set to "" on a Ui rebuilt for each request. An existing actor does not take
/// them from the fresh Ui (Actor.merge copies only Actor.local.key, @bot/@room/@remote/@btab/@sync and the
/// thread's draft), so an empty one the actor did not hold stays out.
fn rebuilt(key: &str) -> bool { key == "@mech.at" || key == "@cmp.menu" || key.starts_with("@cmp.resolve:") || key == "@bot.want" || key == "@thread.want" }

/// Render.ui sets these on the copy it draws; the actor's own Ui never has them.
fn render_scratch(key: &str) -> bool { key.starts_with("@more.") || key == "@pill" }

/// The info keys Actor.seed.view gives a thread the actor has shown no viewer for (Actor.seed.key).
const VIEW_INFO: [&str; 5] = ["view.design:", "view.kind:", "view.path:", "view.sheet:", "view.tree:"];

#[derive(Default)]
pub struct ViewActor { view: String, target: Option<Value>, sel: Option<String>, keys: BTreeSet<String>, away: BTreeMap<String, String>, page: Option<(String, Value, u64)>, local: Option<(String, Value, Value)>, held_drafts: Map<String, Value>, held_info: Map<String, Value>, rid: u64, panel: bool, hub_view: bool, reads: Vec<(Option<(String, Value)>, Value)>, bot: Option<(String, String)>,
    /// ids of the requests this view's commands told the client to send (`cmds` of type send), and of those the hub has since answered
    sent: BTreeSet<u64>, replied: BTreeSet<u64>, late: BTreeSet<u64> }

/// The thread a target's draft and viewer belong to (V.Target.owner): a thread's own id, a project's home owner
/// (`PH.Home.owner`: "project-home-<project>"). Limit: a bot's chat thread (Target.thread: `U.Bots.chat.of`) needs the bots
/// lane's state, so a bot target gets no "@away.<t>", no seeds, and `@btab` stays "" (thin-view Route.choose: the tab
/// the route names, else the bot's default); a room has no owner.
fn owner(target: &Value, bot: Option<&(String, String)>) -> String {
    let id = target["id"].as_str().unwrap_or("");
    match target["kind"].as_str() {
        Some("thread") => id.to_string(),
        Some("project") => format!("project-home-{id}"),
        // the bot's chat thread, looked up by the hub (bots::chat_thread) for this request's target
        Some("bot") => bot.filter(|(b, _)| b == id).map(|(_, t)| t.clone()).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Route.valid: the request's session.route names this bot target's tab: exactly kind/bot/tab strings, kind
/// "bot-tab", the same bot, a tab of Route.tab's list. Limit: Route.restore also asks BU.nav.accept(BU.tabs.for(u, bot), tab),
/// the tabs this bot offers, which the bots lane does not expose; an unoffered tab would fall back to BU.tab.default there.
const BOT_TABS: [&str; 10] = ["chat", "mail", "status", "space", "browser", "tasks", "memory", "routines", "hooks", "settings"];
pub(super) fn route_tab(req: &Value) -> Option<String> {
    let r = req["session"]["route"].as_object()?;
    let t = &req["target"];
    let ok = r.len() == 3 && r.values().all(Value::is_string) && r.get("kind")? == "bot-tab" && t["kind"] == "bot"
        && r.get("bot")?.as_str().filter(|b| !b.is_empty()) == t["id"].as_str() && BOT_TABS.contains(&r.get("tab")?.as_str()?);
    if ok { r.get("tab")?.as_str().map(str::to_string) } else { None }
}

/// Actor.safe_input / Actor.local.key: the only drafts a page may supply (besides the thread's own text).
const LOCAL_KEYS: [&str; 7] = ["@lang", "@lang.os", "@lang.tag", "@lang.preview", "@local.view", "@local.usage", "@desktop.browser"];

/// W.Wire.within: lengths are counted in UTF-8 bytes.
fn within(v: &Value, n: usize) -> Option<&str> { v.as_str().filter(|t| t.len() <= n) }

impl ViewActor {
    /// Actor.safe_input + Actor.merge + Actor.seed.view, for the session sent to the stateless policy. A fresh actor takes
    /// the allowed page input as it is (thread draft, bounded local choices, `@draft.bound`, the away mark, the sheets,
    /// the intent edit, fold/subs/panel/view record). A running one keeps its own drafts and info (markers, forms, stores,
    /// the outbox) and takes from the page only the thread's draft, the local choices, `@draft.bound` (always set, "" when
    /// absent or over its limit), the away mark and fold/subs; the selector keys come from the target (Actor.selector),
    /// never from the page; the sheets and the viewer record only when the actor has shown no viewer for the thread. A
    /// move forgets the scoped drafts and pauses the old selection's history playback before anything is drawn. The
    /// actor's own `sel` stays (same view), its `rid` is the larger and its panel stays after a hub-sent view.
    pub fn input(&self, view: &str, req: &Value) -> Value {
        let mut session = req["session"].as_object().cloned().unwrap_or_default();
        let target = &req["target"];
        let first = self.target.is_none();
        let th = owner(target, self.bot.as_ref());
        let thread_ok = !th.is_empty() && !th.starts_with('@');
        let cd = session.get("drafts").and_then(Value::as_object).cloned().unwrap_or_default();
        let ci = session.get("info").and_then(Value::as_object).cloned().unwrap_or_default();
        let same = !first && self.view == view && self.target.as_ref().map_or(false, |t| t["kind"] == target["kind"] && t["id"] == target["id"]);
        // what the page may give, as Actor.safe_input
        let mut safe = Map::new();
        if thread_ok { if let Some(v) = cd.get(&th).filter(|v| v.is_string()) { safe.insert(th.clone(), v.clone()); } }
        for k in LOCAL_KEYS { if let Some(t) = cd.get(k).and_then(|v| within(v, 128)) { safe.insert(k.into(), json!(t)); } }
        safe.insert("@draft.bound".into(), json!(cd.get("@draft.bound").and_then(|v| within(v, 128)).unwrap_or("")));
        if thread_ok { if let Some(t) = req.get("away").and_then(|v| within(v, 128)) { safe.insert(format!("@away.{th}"), json!(t)); } }
        let sheets: Vec<(String, String)> = [format!("@sch:{th}"), format!("@split.sheet:{th}")].into_iter()
            .filter_map(|k| cd.get(&k).and_then(|v| within(v, 1024)).map(|t| (k, t.to_string()))).collect();
        let seed_info = |ci: &Map<String, Value>| -> Vec<(String, Value)> { ci.iter().filter(|(k, _)| VIEW_INFO.iter().any(|p| **k == format!("{p}{th}"))).map(|(k, v)| (k.clone(), v.clone())).collect() };
        let pref = |k: &str| k.starts_with("fold:") || k.starts_with("subs:");
        let drafts: Map<String, Value>; let info: Map<String, Value>;
        if first {
            let mut d = safe;
            for (k, t) in sheets { d.insert(k, json!(t)); }
            for k in [format!("@intent.edit:{th}"), format!("@intent.draft:{th}")] { if let Some(t) = cd.get(&k).and_then(|v| within(v, 131072)) { d.insert(k, json!(t)); } }
            let mut i = Map::new();
            for (k, v) in &ci { if pref(k) || *k == format!("panel:{th}") { i.insert(k.clone(), v.clone()); } }
            for (k, v) in seed_info(&ci) { i.insert(k, v); }
            drafts = d; info = i;
        } else {
            let mut d = self.held_drafts.clone();
            let mut i = self.held_info.clone();
            i.retain(|k, _| !k.starts_with("drive.") && k != "local.hub.view");   // Actor.info.current, Actor.refresh's Map.del
            if !same {
                d.retain(|k, _| scope_kept(k));                                    // Actor.scope.drafts
                if let Some(old) = &self.sel { i.insert(format!("hist.play:{old}"), json!("")); }   // Actor.history.pause
            }
            for (k, v) in safe { d.insert(k, v); }
            // Actor.selector: from the target, "" when empty
            let kind = target["kind"].as_str().unwrap_or(""); let id = target["id"].as_str().unwrap_or("");
            d.insert("@bot".into(), json!(if kind == "bot" { id } else { "" }));
            d.insert("@room".into(), json!(if kind == "room" { id } else { "" }));
            d.insert("@remote".into(), json!(""));
            d.insert("@btab".into(), json!(route_tab(req).unwrap_or_default()));
            d.insert("@sync".into(), json!("1"));
            for (k, v) in &ci { if pref(k) { i.insert(k.clone(), v.clone()); } }
            // Actor.seed.view: a thread the actor has shown no viewer for (its view.kind is empty) takes the page's record
            if i.get(&format!("view.kind:{th}")).and_then(Value::as_str).map_or(true, str::is_empty) {
                for k in [format!("@sch:{th}"), format!("@split.sheet:{th}")] { d.insert(k.clone(), json!(sheets.iter().find(|(s, _)| *s == k).map(|(_, t)| t.as_str()).unwrap_or(""))); }
                for p in VIEW_INFO { let k = format!("{p}{th}"); i.insert(k.clone(), ci.get(&k).cloned().unwrap_or_else(|| json!(""))); }
            }
            drafts = d; info = i;
        }
        if same {
            if let Some(sel) = &self.sel { session.insert("sel".into(), json!(sel)); }
            let rid = session.get("rid").and_then(Value::as_u64).unwrap_or(0).max(self.rid);
            session.insert("rid".into(), json!(rid));
            if self.hub_view { session.insert("panel".into(), json!(self.panel)); }
        }
        session.insert("drafts".into(), Value::Object(drafts));
        session.insert("info".into(), Value::Object(info));
        session.insert("outbox".into(), json!({}));
        Value::Object(session)
    }
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
    /// A reply to one of the hub's own reads for this connection (`fab.look`, `review.page`, `telemetry.share`), kept for the
    /// policy. Bound to no view yet: like the marks of a job-answered request (`replied_late`), the reply joins the actor that
    /// the NEXT served view request leaves (the JS hub answers a job after the view request queued behind it was served, so a
    /// fab.look sent with a view.open lands on the new view, not the one it was sent from). Bounded: the oldest go first.
    /// The same for a whole reply frame: its `ok`, `result` and top-level `error` (a refusal has no result), as the policy rebuilds the frame.
    pub fn record_reply(&mut self, kind: &str, reply: &Value) {
        let mut e = json!({"kind": kind, "id": reply["id"].as_u64().unwrap_or(0), "ok": reply["ok"].as_bool().unwrap_or(true), "result": reply.get("result").cloned().unwrap_or(json!({}))});
        if let Some(err) = reply.get("error") { e["error"] = err.clone(); }
        self.reads.push((None, e));
        let n = self.reads.len();
        if n > MAX_READS { self.reads.drain(..n - MAX_READS); }
    }
    /// `record_reply` for a reply to a request this view's own commands sent, bound to the view and target it was sent from: the very next view request carries it
    /// (an unbound reply waits for a second one, and a client that asks once after the reply, GPUI, never shows it).
    pub fn record_reply_bound(&mut self, kind: &str, reply: &Value, view: &str, target: &Value) {
        self.record_reply(kind, reply);
        if let Some(last) = self.reads.last_mut() { last.0 = Some((view.to_string(), target.clone())); }
    }
    pub fn record_read(&mut self, kind: &str, id: u64, ok: bool, result: Value) {
        self.reads.push((None, json!({"kind": kind, "id": id, "ok": ok, "result": result})));
        let n = self.reads.len();
        if n > MAX_READS { self.reads.drain(..n - MAX_READS); }
    }
    /// The reads for this view request of `view` and `target`: those bound to them by an earlier request are taken (and
    /// cleared); reads bound to another view or target are dropped (actor drops its reads on a move, `Bool.pick(same,
    /// reads, Nil)`); the unbound ones bind to this request's view and target and are delivered with the next one.
    pub fn take_reads(&mut self, view: &str, target: &Value) -> Vec<Value> {
        let same = |t: &Value| t["kind"] == target["kind"] && t["id"] == target["id"];
        let mut out = Vec::new(); let mut keep = Vec::new();
        for (bound, r) in std::mem::take(&mut self.reads) {
            match bound {
                Some((v, t)) if v == view && same(&t) => out.push(r),
                Some(_) => {}
                None => keep.push((Some((view.to_string(), target.clone())), r)),
            }
        }
        self.reads = keep;
        out
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
        let th = owner(&target, self.bot.as_ref());
        let mut drafts = session.get("drafts").and_then(Value::as_object).cloned().unwrap_or_default();
        drafts.retain(|k, _| !render_scratch(k) && !k.starts_with("@away."));
        // Actor.away.input: a request without the mark leaves it as it was; the marks are kept per thread (Actor.scope.keep keeps @away.*).
        if !th.is_empty() && !th.starts_with('@') { if let Some(a) = req["away"].as_str().filter(|a| a.len() <= 128) { self.away.insert(th.clone(), a.to_string()); } }
        for (t, a) in &self.away { drafts.insert(format!("@away.{t}"), json!(a)); }
        if !first {
            // Actor.scope.drafts: another view or target forgets what was asked for the last
            if moved { drafts.retain(|k, _| scope_kept(k) || !self.keys.contains(k) && !rebuilt(k)); }
            // A changed selection ran Ui.reselect in the actor itself, which sets them; otherwise they are only on the rebuilt Ui.
            if !reselected { drafts.retain(|k, v| !(rebuilt(k) && v.as_str() == Some("") && (moved || !self.keys.contains(k)))); }
        }
        // Actor.bound.input, Session.input and Actor.selector always give these.
        let bound = req["session"]["drafts"]["@draft.bound"].as_str().filter(|b| b.len() <= 128).unwrap_or("");
        drafts.insert("@draft.bound".into(), json!(bound));
        for k in ["@btab", "@presentation"] { drafts.entry(k).or_insert_with(|| json!("")); }
        // Rp.reply and Telemetry.reply run on the reply of every request the actor's own commands sent, and each leaves the request's
        // id behind, emptied ("@rp.req:<id>" is the page a review request asked for, cleared when its reply ends it: Rp.failed always sets it "").
        for id in &self.replied { drafts.insert(format!("@rp.req:{id}"), json!("")); drafts.insert(format!("@telemetry.req:{id}"), json!("")); }
        // an answer from a hub job (fab.look, files.dir, git.* ...) reaches the JS actor after the view request that was already being served, so it
        // shows from the request after that one (recA/session-gpui: view.open sent right after fab.look carries none; the next update does)
        let late = std::mem::take(&mut self.late); self.replied.extend(late); cap(&mut self.replied);
        let mut info = session.get("info").and_then(Value::as_object).cloned().unwrap_or_default();
        // Actor.refresh: a thread (or none) this actor has shown no viewer for takes the page's record, "" when it has none.
        if !first && !self.keys.contains(&format!("view.kind:{th}")) {
            for k in ["@sch:", "@split.sheet:"] { drafts.entry(format!("{k}{th}")).or_insert_with(|| json!("")); }
            for k in VIEW_INFO { info.entry(format!("{k}{th}")).or_insert_with(|| json!("")); }
        }
        self.keys = drafts.keys().cloned().chain(info.keys().cloned()).collect();
        self.held_drafts = drafts.clone();
        self.hub_view = info.get("local.hub.view").and_then(Value::as_str) == Some("1");
        self.held_info = info.clone();
        self.rid = session.get("rid").and_then(Value::as_u64).unwrap_or(self.rid);
        self.panel = session.get("panel").and_then(Value::as_bool).unwrap_or(self.panel);
        self.target = Some(target);
        self.view = view.to_string();
        self.sel = Some(sel);
        session.insert("drafts".into(), Value::Object(drafts));
        if session.contains_key("info") { session.insert("info".into(), Value::Object(info)); }
        Value::Object(session)
    }
}

static ACTORS: LazyLock<Mutex<HashMap<u64, ViewActor>>> = LazyLock::new(Default::default);

/// Most reads held per view between two requests.
const MAX_READS: usize = 16;

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
    for id in sent_ids(pol) { a.sent.insert(id); cap(&mut a.sent); }
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
/// `replied` for a request a hub job answers (see `is_job`): its marks wait for one served view request.
pub fn replied_late(client: u64, id: u64) {
    if let Some(a) = ACTORS.lock().unwrap().get_mut(&client) { if a.sent.remove(&id) { a.late.insert(id); } }
}
/// The methods hub answers from a job (Git/Media/Jobs outs: the reply arrives after the synchronous requests queued behind it).
pub fn is_job(method: &str) -> bool {
    matches!(method, "fab.look" | "fab.intent" | "order.buy" | "boards.list" | "pdf.page" | "file.open" | "project.new" | "project.create" | "fs.list" | "fs.mkdir" | "fs.find")
        || ["files.", "git.", "history.", "kicad.", "mech.", "review."].iter().any(|p| method.starts_with(p))
}
pub fn replied(client: u64, id: u64) {
    if let Some(a) = ACTORS.lock().unwrap().get_mut(&client) { if a.sent.remove(&id) { a.replied.insert(id); cap(&mut a.replied); } }
}
/// The newest ids only: a long connection keeps at most this many requests' marks (each answered one is two drafts resent to the policy).
const MARKS_KEPT: usize = 256;
fn cap(s: &mut BTreeSet<u64>) { while s.len() > MARKS_KEPT { s.pop_first(); } }
/// The session to send the policy: see `ViewActor::input`.
pub fn input(client: u64, view: &str, req: &Value) -> Value {
    ACTORS.lock().unwrap().entry(client).or_default().input(view, req)
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
/// Actor.native.page_result.page: a read-only `native-sidebar-page` reply is not a channel for what the actor holds beyond what
/// the page captured. The session's drafts, info and outbox keep only the keys of the page's own session (a mask, never
/// data), and error/note go. `req` is the request as the client sent it.
pub fn mask_page_reply(req: &Value, reply: &mut Value) {
    if req["action"].as_str() != Some("native-sidebar-page") || req["nativeSidebar"] != json!(1) { return }
    let Some(s) = reply.get_mut("session").and_then(Value::as_object_mut) else { return };
    s.remove("error"); s.remove("note");
    for f in ["drafts", "info", "outbox"] {
        let mask = req["session"][f].as_object();
        if let Some(Value::Object(o)) = s.get_mut(f) { o.retain(|k, _| mask.map_or(false, |m| m.contains_key(k))); }
    }
}
/// The hub answered one of its own reads for this connection's view: keep the reply for the policy (see `ViewActor::record_read`).
/// `kind` is "fab.look", "review.page" or "telemetry.share"; `id` is the RPC id of the reply as the hub sent it (0 for fab.look); `ok` is the reply's ok (a failed Rp reply
/// clears its `@rp.req:N` differently); `result` is the reply's `result` (for a failure, what the hub sent in its place);
/// `result` is the RPC reply's `result` as the hub sent it. The
/// reply is the hub's own, never the client's.
/// `record_read` bound at once to the view and target it was asked for: delivered with that view's next request. The window asks for one view update after
/// a job's answer; an unbound read would only bind to that one and wait for another request that may never come (Buy it never showed).
pub fn record_read_bound(client: u64, view: &str, target: &Value, kind: &str, id: u64, ok: bool, result: Value) {
    let mut m = ACTORS.lock().unwrap(); let a = m.entry(client).or_default();
    a.record_read(kind, id, ok, result);
    if let Some(last) = a.reads.last_mut() { last.0 = Some((view.to_string(), target.clone())); }
}
pub fn record_read(client: u64, _view_id: &str, kind: &str, id: u64, ok: bool, result: Value) {
    ACTORS.lock().unwrap().entry(client).or_default().record_read(kind, id, ok, result);
}
/// `record_read` for a whole reply frame (keeps its top-level `error`).
pub fn record_reply_bound(client: u64, kind: &str, reply: &Value, view: &str, target: &Value) { ACTORS.lock().unwrap().entry(client).or_default().record_reply_bound(kind, reply, view, target); }
/// `record_reply_bound` that also names the method the reply answers (`method`: git.diff, git.act, review.bom ...): the kind is the one the helper takes, the method is what the Rust presenter reads.
pub fn record_reply_bound_m(client: u64, kind: &str, method: &str, reply: &Value, view: &str, target: &Value) {
    let mut m = ACTORS.lock().unwrap(); let a = m.entry(client).or_default();
    a.record_reply_bound(kind, reply, view, target);
    if let Some(last) = a.reads.last_mut() { last.1["method"] = json!(method); }
}
pub fn record_reply(client: u64, kind: &str, reply: &Value) { ACTORS.lock().unwrap().entry(client).or_default().record_reply(kind, reply); }
/// The reads recorded since the last V request, for `reads` in the next one.
pub fn take_reads(client: u64, view_id: &str, target: &Value) -> Vec<Value> {
    ACTORS.lock().unwrap().get_mut(&client).map(|a| a.take_reads(view_id, target)).unwrap_or_default()
}
/// The hub's lookup for a bot target (bots::chat_thread): the bot id and its chat thread, held for this request.
pub fn set_bot(client: u64, bot: Option<(String, String)>) {
    ACTORS.lock().unwrap().entry(client).or_default().bot = bot;
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
        // a job's answer waits for one served view request
        let mut b = ViewActor::default(); b.sent.insert(5); b.late.insert(5);
        assert!(!has(&b.reply("v", &req("thread", "t1"), &pol("t1", &[])), "@rp.req:5"), "the request already being served does not carry it");
        assert!(has(&b.reply("v", &req("thread", "t1"), &pol("t1", &[])), "@rp.req:5"), "the next one does");
        assert!(is_job("fab.look") && is_job("files.dir") && is_job("git.status") && !is_job("turn.start") && !is_job("project.add") && !is_job("setting.set"));
        let mut many: BTreeSet<u64> = (1..=1000).collect(); cap(&mut many); assert_eq!((many.len(), many.first().copied(), many.last().copied()), (256, Some(745), Some(1000)));
    }
    #[test] fn a_recorded_reply_keeps_its_error() {
        let mut a = ViewActor::default(); let t = json!({"kind": "thread", "id": "t1"});
        a.record_reply("files.dir", &json!({"t":"reply","id":9,"ok":false,"error":"No folder for this thread or project: unknown, deleted, removed or unavailable"}));
        a.record_reply("files.dir", &json!({"t":"reply","id":10,"ok":true,"result":{"fmEntries":"f\t1\tr.txt\n","thread":"t1"}}));
        assert!(a.take_reads("v", &t).is_empty()); let r = a.take_reads("v", &t);
        assert_eq!((r[0]["id"].clone(), r[0]["ok"].clone(), r[0]["error"].as_str().unwrap().len() > 10), (json!(9), json!(false), true)); assert!(r[1].get("error").is_none() && r[1]["result"]["fmEntries"] == "f\t1\tr.txt\n");
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
    fn running(a: &mut ViewActor, view: &str, t: &str) -> Value {
        let r = req("thread", t);
        a.reply(view, &r, &json!({"sel": t, "rid": 5, "panel": false, "drafts": {"@rp.req:1": "", "@f.title": "mine", "@mech.at": "t#1", "@cmp.menu": "m"}, "info": {"fab.line:t1": "held", "drive.x": "1", "view.kind:t1": "board"}}))
    }
    #[test] fn a_fresh_actor_takes_the_allowed_page_input_only() {
        let a = ViewActor::default();
        let mut r = req("thread", "t1");
        r["session"] = json!({"sel": "t1", "drafts": {"t1": "hello", "@lang": "en", "@f.title": "x", "@rp.req:9": "1", "@bot": "b", "@sch:t1": "2", "@intent.edit:t1": "e"},
            "info": {"fold:a": "1", "view.kind:t1": "board", "fab.line:t1": "forged", "panel:t1": "1"}, "outbox": {"x": 1}});
        let s = a.input("v1", &r);
        let d = &s["drafts"];
        assert_eq!(d["t1"], json!("hello")); assert_eq!(d["@lang"], json!("en")); assert_eq!(d["@sch:t1"], json!("2")); assert_eq!(d["@intent.edit:t1"], json!("e"));
        assert!(d.get("@f.title").is_none() && d.get("@bot").is_none());
        assert!(d.get("@rp.req:9").is_none(), "request markers are never read from the wire");
        assert_eq!(s["info"]["fold:a"], json!("1")); assert_eq!(s["info"]["view.kind:t1"], json!("board")); assert_eq!(s["info"]["panel:t1"], json!("1"));
        assert!(s["info"].get("fab.line:t1").is_none(), "fab.* is never read from the wire");
        assert_eq!(s["outbox"], json!({}));
    }
    #[test] fn a_running_actor_ignores_the_page_for_selector_markers_forms_and_sheets() {
        let mut a = ViewActor::default();
        running(&mut a, "v1", "t1");
        let mut r = req("thread", "t1");
        r["session"] = json!({"sel": "other", "rid": 2, "drafts": {"t1": "typed", "@bot": "b", "@room": "r", "@rp.req:1": "forged", "@f.title": "forged", "@sch:t1": "9", "@intent.edit:t1": "e"},
            "info": {"fab.line:t1": "forged", "view.kind:t1": "forged", "panel:t1": "0", "fold:z": "1"}});
        let s = a.input("v1", &r);
        let d = &s["drafts"];
        assert_eq!(d["@bot"], json!("")); assert_eq!(d["@room"], json!("")); assert_eq!(d["@sync"], json!("1"));
        assert_eq!(d["@rp.req:1"], json!("")); assert_eq!(d["@f.title"], json!("mine"));
        // a forged fab.* / request marker the actor does not hold never reaches the policy, on any request
        let q = { let mut q = req("thread", "t1"); q["session"] = json!({"drafts": {"@rp.req:7": "", "@telemetry.req:7": ""}, "info": {"fab.new:t1": "c", "fab.line:t1": "client"}}); q };
        let sq = a.input("v1", &q);
        assert!(sq["drafts"].get("@rp.req:7").is_none() && sq["drafts"].get("@telemetry.req:7").is_none() && sq["info"].get("fab.new:t1").is_none());
        assert_eq!(sq["info"]["fab.line:t1"], json!("held"));
        assert!(d.get("@sch:t1").is_none(), "the viewer exists: the sheet is not page input");
        assert!(d.get("@intent.edit:t1").is_none(), "intent edits are for a fresh actor");
        assert_eq!(d["t1"], json!("typed"));
        assert_eq!(s["info"]["fab.line:t1"], json!("held")); assert_eq!(s["info"]["view.kind:t1"], json!("board")); assert!(s["info"].get("panel:t1").is_none());
        assert_eq!(s["info"]["fold:z"], json!("1"));
        assert!(s["info"].get("drive.x").is_none(), "Actor.info.current drops drive. keys");
        assert_eq!(s["sel"], json!("t1"), "the actor keeps its sel on the same view");
        assert_eq!(s["rid"], json!(5), "rid is the larger");
        // a bot target sets @bot from the target
        let mut b = req("bot", "bb"); b["session"] = json!({"drafts": {}});
        assert_eq!(a.input("v1", &b)["drafts"]["@bot"], json!("bb"));
    }
    #[test] fn limits_count_bytes_and_the_bound_mark_never_outlives_the_request() {
        let mut a = ViewActor::default();
        running(&mut a, "v1", "t1");
        let cjk43 = "字".repeat(43); // 43 chars, 129 bytes
        let ok42 = "字".repeat(42);  // 126 bytes
        let mut r = req("thread", "t1");
        r["session"] = json!({"drafts": {"@lang": cjk43, "@lang.os": ok42, "@draft.bound": "5 abc"}});
        r["away"] = json!("字".repeat(43));
        let s = a.input("v1", &r);
        assert!(s["drafts"].get("@lang").is_none() || s["drafts"]["@lang"] != json!(cjk43.clone()));
        assert_eq!(s["drafts"]["@lang.os"], json!(ok42));
        assert_eq!(s["drafts"]["@draft.bound"], json!("5 abc"));
        assert!(s["drafts"].get("@away.t1").is_none());
        let again = a.input("v1", &req("thread", "t1"));
        assert_eq!(again["drafts"]["@draft.bound"], json!(""), "absent mark clears the held one");
        let mut big = req("thread", "t1"); big["session"] = json!({"drafts": {"@draft.bound": "x".repeat(129)}});
        assert_eq!(a.input("v1", &big)["drafts"]["@draft.bound"], json!(""));
    }
    #[test] fn a_move_clears_the_scoped_drafts_before_the_policy_sees_them() {
        let mut a = ViewActor::default();
        running(&mut a, "v1", "t1");
        let s = a.input("v1", &req("thread", "t2"));
        assert!(s["drafts"].get("@f.title").is_none() && s["drafts"].get("@mech.at").is_none() && s["drafts"].get("@cmp.menu").is_none());
        assert_eq!(s["drafts"]["@rp.req:1"], json!(""), "markers stay");
        assert_eq!(s["info"]["hist.play:t1"], json!(""));
        assert_eq!(s["info"]["view.kind:t2"], json!(""), "the new thread has no viewer: seeded empty");
        // the same target on another view is a move too
        let s2 = a.input("v2", &req("thread", "t1"));
        assert!(s2["drafts"].get("@f.title").is_none());
    }
    #[test] fn the_sheets_and_record_seed_a_thread_without_a_viewer() {
        let mut a = ViewActor::default();
        running(&mut a, "v1", "t1");
        let mut r = req("thread", "t2");
        r["session"] = json!({"drafts": {"@sch:t2": "3", "@split.sheet:t2": "4"}, "info": {"view.kind:t2": "board", "view.path:t2": "a.kicad_pcb", "panel:t2": "1"}});
        let s = a.input("v1", &r);
        assert_eq!(s["drafts"]["@sch:t2"], json!("3")); assert_eq!(s["drafts"]["@split.sheet:t2"], json!("4"));
        assert_eq!(s["info"]["view.kind:t2"], json!("board")); assert_eq!(s["info"]["view.design:t2"], json!(""));
        assert!(s["info"].get("panel:t2").is_none(), "panel is for a fresh actor only");
    }
    #[test] fn a_page_action_reply_keeps_only_the_pages_own_keys() {
        let mut req = req("thread", "t1");
        req["action"] = json!("native-sidebar-page"); req["nativeSidebar"] = json!(1);
        req["session"] = json!({"drafts": {"t1": "x"}, "info": {"fold:a": "1"}, "outbox": {}});
        let mut r = json!({"session": {"error": "e", "note": "n", "drafts": {"t1": "x", "@rp.req:1": ""}, "info": {"fold:a": "1", "fab.line:t1": "held"}, "outbox": {"o": "1"}}});
        mask_page_reply(&req, &mut r);
        assert_eq!(r["session"], json!({"drafts": {"t1": "x"}, "info": {"fold:a": "1"}, "outbox": {}}));
        let mut other = req.clone(); other["action"] = json!("draft");
        let mut r2 = json!({"session": {"drafts": {"@rp.req:1": ""}}});
        mask_page_reply(&other, &mut r2);
        assert!(r2["session"]["drafts"].get("@rp.req:1").is_some());
    }
    #[test] fn a_read_joins_the_actor_the_next_served_request_leaves_and_is_bounded() {
        let mut a = ViewActor::default();
        let t1 = json!({"kind": "thread", "id": "t1"}); let t2 = json!({"kind": "thread", "id": "t2"});
        for i in 0..40 { a.record_read("fab.look", 0, true, json!({"n": i})); }
        assert!(a.take_reads("v1", &t1).is_empty(), "the request being served does not carry it");
        let r = a.take_reads("v1", &t1);
        assert_eq!(r.len(), MAX_READS);
        assert_eq!(r[0]["result"]["n"], json!(24)); assert_eq!(r[15]["result"]["n"], json!(39));
        assert_eq!(r[0]["kind"], json!("fab.look")); assert_eq!(r[0]["id"], json!(0)); assert_eq!(r[0]["ok"], json!(true));
        assert!(a.take_reads("v1", &t1).is_empty(), "once");
        // a fab.look sent with a view.open: recorded while the old view is current, it lands on the new one
        a.record_read("fab.look", 1, true, json!({}));
        assert!(a.take_reads("v2", &t2).is_empty());            // the view.open
        assert_eq!(a.take_reads("v2", &t2).len(), 1);            // the next update carries it
        // a read bound to a view or target that then moved is dropped
        a.record_read("fab.look", 2, false, json!({}));
        assert!(a.take_reads("v2", &t2).is_empty());
        assert!(a.take_reads("v2", &t1).is_empty());
        assert!(a.take_reads("v2", &t2).is_empty());
        assert!(a.take_reads("v3", &t2).is_empty());
    }
    #[test] fn a_page_action_before_the_first_update_gets_the_empty_maps_back() {
        // android: a native-sidebar-page sent before any view.update; the request's captured session is empty, so must the reply's maps be
        let mut req = req("recent", "");
        req["action"] = json!("native-sidebar-page"); req["nativeSidebar"] = json!(1);
        req["session"] = json!({"sel": "", "drafts": {}, "info": {}, "outbox": {}});
        let pol = json!({"remoteReady": true, "session": {"sel": "", "error": "e", "note": "n", "drafts": {}, "info": {}, "outbox": {}}});
        let mut out = apply(900_777, "v1", &req, &pol);
        assert!(out["session"]["drafts"].as_object().unwrap().len() > 0, "the actor's constants are in the unmasked reply");
        mask_page_reply(&req, &mut out);
        assert_eq!(out["session"]["drafts"], json!({})); assert_eq!(out["session"]["info"], json!({})); assert_eq!(out["session"]["outbox"], json!({}));
        assert!(out["session"].get("error").is_none() && out["session"].get("note").is_none());
        forget(900_777);
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
