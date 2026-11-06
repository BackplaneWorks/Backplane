//! The uberbot's ops (src/core/ubop.bend, ubersync.bend): one uberbot on the owner's machines. Each machine keeps a log of ops (`UberOp`, folded into
//! registers by the state lane's fold, which `World::ub` runs); what the uberbot does here (memory, space, page, name and look, routines) becomes
//! an op beside the change, a machine that joins gets the first ops, and the conversation's lease says which machine holds it.
//! The exchange between machines (`uber.sync` over /far/rpc, takeover, the holder's beat) belongs with the net lane's transport; the ops are made here.
use super::rpc::{now_s, Bots};
use super::world::{Routine, World};
use serde_json::{json, Value};

/// Ub.stamp.make: `<secs, 10 digits>.<counter, 6 digits>.<machine>`
fn stamp_make(secs: u64, ctr: u64, me: &str) -> String { format!("{secs:010}.{ctr:06}.{me}") }
/// Ub.stamp.next: later than the last one, whatever the clock says
pub fn stamp_next(last: &str, now: u64, me: &str) -> String {
    let secs = last.get(..10).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0); let ctr = last.get(11..17).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    if secs >= now { stamp_make(secs, ctr + 1, me) } else { stamp_make(now, 0, me) }
}
/// Ub.gid: a routine's id across machines ("<origin>:<local id>"; a local id with a machine in it is one)
pub fn gid(me: &str, id: &str) -> String { if id.contains(':') { id.to_string() } else { format!("{me}:{id}") } }
fn show(v: Value) -> String { serde_json::to_string(&v).unwrap_or_default() }

// the op bodies (Ub.b.*), fields in Bend's order
pub fn b_mem(key: &str, kind: &str, text: &str, tags: &str) -> String { show(json!({"k":"mem","key":key,"kind":kind,"text":text,"tags":tags})) }
pub fn b_forget(key: &str) -> String { show(json!({"k":"forget","key":key})) }
pub fn b_space(spec: &str) -> String { show(json!({"k":"space","spec":spec})) }
pub fn b_page(html: &str) -> String { show(json!({"k":"page","html":html})) }
pub fn b_who(name: &str, look: u64, persona: &str) -> String { show(json!({"k":"who","name":name,"look":look,"persona":persona})) }
pub fn b_routine(id: &str, name: &str, cron: &str, prompt: &str, on: bool, pin: &str) -> String { show(json!({"k":"routine","id":id,"name":name,"cron":cron,"prompt":prompt,"on":on,"pin":pin})) }
#[allow(clippy::too_many_arguments)]
pub fn b_routine_plan(id: &str, name: &str, cron: &str, prompt: &str, on: bool, pin: &str, once: u64, tz: &str, kind: &str) -> String {
    show(json!({"k":"routine","id":id,"name":name,"cron":cron,"prompt":prompt,"on":on,"pin":pin,"once":once,"tz":tz,"kind":kind}))
}
pub fn b_routine_del(id: &str) -> String { show(json!({"k":"routine-del","id":id})) }
pub fn b_fired(routine: &str, slot: u64) -> String { show(json!({"k":"fired","routine":routine,"slot":slot})) }
pub fn b_lease(holder: &str, term: u64) -> String { show(json!({"k":"lease","holder":holder,"term":term})) }
pub fn b_bind(machine: &str, bot: &str, thread: &str, name: &str) -> String { show(json!({"k":"bind","machine":machine,"bot":bot,"thread":thread,"name":name})) }

impl World {
    /// Ub.ops.with: these bodies as this machine's next ops under `uid` (none without a machine id or a uid)
    pub fn ops_with(&self, uid: &str, now: u64, bodies: &[String]) -> Vec<Value> {
        let me = self.me(); if me.is_empty() || uid.is_empty() { return vec![] }
        let (mut n, mut last) = (self.vv(uid, &me) + 1, self.hlc(uid)); let mut out = vec![];
        for b in bodies { let s = stamp_next(&last, now, &me); out.push(json!({"$":"UberOp","uid":uid,"origin":me,"n":n,"stamp":s,"op":b,"at":now})); last = s; n += 1 }
        out
    }
    pub fn ops(&self, now: u64, bodies: &[String]) -> Vec<Value> { self.ops_with(&self.uid(), now, bodies) }
    fn routine_pin_body(&self, r: &Routine, pin: &str) -> String { b_routine_plan(&gid(&self.me(), &r.id), &r.name, &r.cron, &r.prompt, r.on, pin, r.once, &r.tz, &r.kind) }

    /// Ub.fires: a pinned routine only on its machine, any other only on the conversation's holder, and a slot only once (a slot not later than one that ran is over)
    pub fn fires(&self, gid: &str, slot: u64) -> bool {
        let pin = self.pin(gid); let fired: u64 = self.ub.settings.get(&format!("uber.fired.{}.{gid}", self.uid())).and_then(|v| v.parse().ok()).unwrap_or(0);
        slot > fired && if pin.is_empty() { self.holder_me() } else { pin == self.me() }
    }
    /// Ub.bodies: the op bodies a request's changes call for (none for what is not the uberbot's)
    pub fn bodies(&self, cs: &[Value]) -> Vec<String> {
        let g = |c: &Value, k: &str| c.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let me = self.me(); let mut out = vec![]; let mut i = 0;
        while i < cs.len() {
            let c = &cs[i]; let k = c["$"].as_str().unwrap_or("");
            match k {
                "MemorySaved" if self.uber_is(&g(c, "bot")) => out.push(b_mem(&g(c, "key"), &g(c, "kind"), &g(c, "text"), &g(c, "tags"))),
                "MemoryForgot" if self.uber_is(&g(c, "bot")) => out.push(b_forget(&g(c, "key"))),
                "SpaceSet" if self.uber_is(&g(c, "bot")) => out.push(b_space(&g(c, "spec"))),
                "SpacePageSet" if self.uber_is(&g(c, "bot")) => out.push(b_page(&g(c, "html"))),
                "BotSet" if self.uber_is(&g(c, "id")) => out.push(b_who(&g(c, "name"), c["look"].as_u64().unwrap_or(0), &g(c, "persona"))),
                "RoutineSet" if self.uber_is(&g(c, "bot")) => {
                    let (id, on) = (g(c, "id"), c["on"].as_bool().unwrap_or(false)); let pin = self.pin(&gid(&me, &id));
                    if let Some(p) = cs.get(i + 1).filter(|p| p["$"] == "RoutinePlan") { // the set and its plan are one op
                        out.push(b_routine_plan(&gid(&me, &id), &g(c, "name"), &g(c, "cron"), &g(c, "prompt"), on, &pin, p["once"].as_u64().unwrap_or(0), &g(p, "tz"), &g(p, "kind"))); i += 1;
                    } else { out.push(b_routine(&gid(&me, &id), &g(c, "name"), &g(c, "cron"), &g(c, "prompt"), on, &pin)) }
                }
                "RoutineDeleted" if self.routines.get(&g(c, "id")).map_or(false, |r| self.uber_is(&r.bot)) => out.push(b_routine_del(&gid(&me, &g(c, "id")))),
                _ => {}
            }
            i += 1;
        }
        out
    }
    /// Ub.also: the changes and, after them, the ops they call for
    pub fn also(&self, now: u64, cs: Vec<Value>) -> Vec<Value> { let ops = self.ops(now, &self.bodies(&cs)); cs.into_iter().chain(ops).collect() }

    /// Ub.init.bodies: everything an uberbot has, as its first ops (name and look, memories, space, page, routines, the lease at term 1, where it lives)
    pub fn init_bodies(&self, bot: &str) -> Vec<String> {
        let Some(b) = self.bots.get(bot) else { return vec![] };
        let me = self.me(); let mut out = vec![b_who(&b.name, b.look, &b.persona)];
        // World.mems: newest first (Mem.newer, by the time last saved; the Bend sort leaves equal times in no stated order: the key decides here)
        let mut mems: Vec<_> = self.mems.values().filter(|m| m.bot == bot).collect(); mems.sort_by(|a, c| c.updated.cmp(&a.updated).then_with(|| a.key.cmp(&c.key)));
        out.extend(mems.iter().map(|m| b_mem(&m.key, &m.kind, &m.text, &m.tags)));
        if let Some(sp) = self.spaces.get(bot).filter(|s| !s.is_empty()) { out.push(b_space(sp)) }
        if let Some((h, _)) = self.pages.get(bot).filter(|(h, _)| !h.is_empty()) { out.push(b_page(h)) }
        // Map.values: by id (the JS hub's first ops list rz, ra, rm, rb as ra, rb, rm, rz)
        let mut rs: Vec<_> = self.routines.values().filter(|r| r.bot == bot && !r.deleted).collect(); rs.sort_by(|a, c| a.id.cmp(&c.id));
        out.extend(rs.iter().map(|r| self.routine_pin_body(r, "")));
        out.push(b_lease(&me, 1)); out.push(b_bind(&me, bot, &b.thread, &b.name));
        out
    }
}

fn ok(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":true}) }
fn err(id: &Value, e: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":e}) }

impl World {
    /// an uberbot with no thread: its id, its name, and the threads its own bind ops name (any machine's), best first
    pub fn heal_wants(&self) -> Option<(String, String, Vec<String>)> {
        let bot = self.uber_bot(); let b = self.bots.get(&bot)?; if !b.thread.is_empty() { return None }
        let uid = self.uid(); let pre = format!("uber.r.{uid}.bind:");
        let mut ts: Vec<String> = self.ub.settings.iter().filter(|(k, _)| k.starts_with(&pre)).filter_map(|(_, v)| {
            let o: Value = serde_json::from_str(v).ok()?; (o["bot"].as_str() == Some(&bot)).then(|| o["thread"].as_str().unwrap_or("").to_string()) }).filter(|t| !t.is_empty()).collect();
        ts.sort(); Some((bot, b.name.clone(), ts))
    }
}

impl Bots {
    /// the uberbot has no conversation thread (an imported history ends with a BotSet that has none, and the sidebar then takes it for a bot on another machine): give it one
    pub async fn uber_heal(&self, now: u64) -> bool {
        if crate::copy::on() { return false }
        let w = self.world().await;
        let Some((bot, name, binds)) = w.heal_wants() else { return false };
        let Some(b) = w.bot(&bot) else { return false };
        let th = { let m = self.hub.model().await;
            binds.into_iter().find(|t| m.threads.contains_key(t)).or_else(|| { let mut c: Vec<_> = m.threads.iter().filter(|(_, t)| t.title == name && !t.deleted).map(|(i, _)| i.clone()).collect(); c.sort(); c.into_iter().next() }) };
        let Some(th) = th else { return false };
        let ev = vec![json!({"$":"BotSet","id":bot,"name":b.name,"thread":th,"look":b.look,"persona":b.persona,"at":now})];
        let ev = w.also(now, ev); drop(w);
        self.hub.commit_many(&ev).await.is_ok()
    }
    /// Uber.init (the minute): the uberbot's first ops, while this machine has no bind op
    pub async fn uber_init(&self, now: u64) -> usize { if crate::copy::on() { return 0 } self.uber_init_now(now).await }
    /// the first ops without the copy gate (a test fixture on a copy hub builds the uberbot's state with it)
    pub(crate) async fn uber_init_now(&self, now: u64) -> usize {
        let w = self.world().await;
        // whenever this machine has no bind op, not when its op count is 0: a memory saved in the first minute is op 1, and the who, lease and bind would never be logged (the Bend hub's bug, fixed here on purpose)
        if !(w.ub_on() && w.reg(&w.uid(), &format!("bind:{}", w.me())).is_empty()) { return 0 }
        let ops = w.ops(now, &w.init_bodies(&w.ub.settings.get("uber.bot").cloned().unwrap_or_default()));
        let n = ops.len(); if n > 0 && self.hub.commit_many(&ops).await.is_err() { return 0 } n
    }
    /// bots.pin {routine, pin}: pin one of the uberbot's routines to a machine ("here" is this one, "" takes the pin off)
    pub(super) async fn pin(&self, id: &Value, p: &Value) -> Vec<Value> {
        let w = self.world().await; let now = now_s();
        let (rid, pin) = (p["routine"].as_str().unwrap_or(""), p["pin"].as_str().unwrap_or(""));
        let pin = if pin == "here" { w.me() } else { pin.to_string() };
        let ops = match w.routines.get(rid).filter(|r| w.uber_is(&r.bot)) { Some(r) => w.ops(now, &[w.routine_pin_body(r, &pin)]), None => vec![] };
        if ops.is_empty() { return vec![err(id, "only the uberbot's routines can be pinned")] }
        self.commit_reply_pub(id, ops).await
    }
    /// bots.uber_home {}: this machine takes the conversation now (the lease moves here one term on)
    pub(super) async fn uber_home(&self, id: &Value) -> Vec<Value> {
        let w = self.world().await; let now = now_s();
        if !w.ub_on() { return vec![err(id, "this machine has no uberbot")] }
        if w.holder_me() { return vec![ok(id)] }
        let ops = w.ops(now, &[b_lease(&w.me(), w.lease().1 + 1)]);
        self.commit_reply_pub(id, ops).await // the old holder's last entries are queued ahead of the next message through the mirror (net lane), not here
    }
    /// bots.uber_join {uid}: join the uberbot another machine offered (setting uber.offer): a replica bot tied to the uid
    pub(super) async fn uber_join(self: &std::sync::Arc<Self>, id: &Value, uid: &str) -> Vec<Value> {
        let w = self.world().await; let now = now_s();
        if w.me().is_empty() { return vec![err(id, "this machine has no id yet; try again in a moment")] }
        let offer: Value = serde_json::from_str(&w.ub.settings.get("uber.offer").cloned().unwrap_or_default()).unwrap_or(Value::Null);
        let on_offer = offer["uid"].as_str() == Some(uid) && !uid.is_empty();
        if !on_offer { return vec![err(id, "that uberbot is not on offer here")] }
        if !w.uber_bot().is_empty() { return vec![err(id, "this machine has an uberbot already; make it not the uberbot first")] }
        let (name, look) = (offer["name"].as_str().unwrap_or("").to_string(), offer["look"].as_u64().unwrap_or(0));
        if !super::rpc::name_ok(&name) || w.named(&name).is_some() { return vec![err(id, &format!("this machine has a bot called {name} already; rename it first"))] }
        let (bot, th) = (self.hub_id("b", now), self.hub_id("t", now));
        let (provider, model) = (w.setting("agent.provider", "claude"), w.setting("agent.model", ""));
        let mut ev = vec![json!({"$":"ThreadCreated","id":th,"project":"","title":name,"env":"local","branch":"","worktree":"","provider":provider,"model":model,"at":now}),
            json!({"$":"BotSet","id":bot,"name":name,"thread":th,"look":look,"persona":"","at":now}),
            json!({"$":"UberSet","bot":bot,"uid":uid,"at":now}), json!({"$":"SettingSet","key":"uber.offer","value":"","at":now})];
        let mut ev = self.accounts_created(ev).await;
        ev.extend(w.ops_with(uid, now, &[b_bind(&w.me(), &bot, &th, &name)]));
        let r = self.hub.commit_many(&ev).await;
        drop(w);
        if let Err(e) = r { return vec![err(id, &format!("{e:#}"))] }
        let (me, b2, t2) = (self.clone(), bot.clone(), th.clone());
        tokio::spawn(async move { me.home_pub(&b2, &t2).await });
        vec![json!({"t":"reply","id":id,"ok":true,"bot":bot})]
    }
}
