//! Routines and reminders. A routine is a prompt a bot
//! receives on a schedule or once; a reminder (kind "remind") is delivered to the person with no agent turn. The rules of time are clock.rs.
use super::clock::{when_independent, when_parse, Cron, Plan, TzRule, Zones, WHEN_TEXT};
use super::rpc::{now_s, Bots};
use super::world::{Routine, World};
use serde_json::{json, Value};
use std::sync::Arc;

fn ok(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":true}) }
fn err(id: &Value, e: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":e}) }
fn gs<'a>(p: &'a Value, k: &str) -> &'a str { p.get(k).and_then(Value::as_str).unwrap_or("") }

/// What a routine's schedule or time asks for (RPick): the cron line ("" for a one-time routine), the UTC instant of a one-time one (0: repeats) and its zone
#[derive(Debug, PartialEq)]
pub enum Pick { Plan { cron: String, once: u64, tz: String }, Bad(String) }

/// Bots.plan.tz: none = what it was (a new routine: the person's), "machine" = the machine's own offset (stored as ""), "person", or a zone name
fn plan_tz(arg: &str, is_new: bool, cur: &str) -> Option<String> {
    if arg.is_empty() { Some(if is_new { "person".into() } else { cur.into() }) } else if arg == "machine" { Some(String::new()) } else if arg == "person" { Some("person".into()) } else if Zones::valid(arg) { Some(arg.into()) } else { None }
}
/// Bots.plan: the plan for a routine from its arguments (`ex`: the routine as it is, None when new or removed)
pub fn plan(z: &Zones, now: u64, tz_arg: &str, sch: &str, at: &str, ex: Option<&Routine>) -> Pick {
    let (sch, at) = (sch.trim(), at.trim());
    let Some(tz) = plan_tz(tz_arg.trim(), ex.is_none(), ex.map_or("", |r| r.tz.as_str())) else { return Pick::Bad("that is not a time zone name (like Europe/Helsinki; or person, or machine)".into()) };
    match (sch.is_empty(), at.is_empty()) {
        (true, true) => Pick::Bad("give a schedule (every 15m, daily 09:30...) or a one-time time (at)".into()),
        (false, false) => Pick::Bad("give a schedule or a time (at), not both".into()),
        (false, true) => if Cron::parse(sch).is_some() { Pick::Plan { cron: Cron::text(sch), once: 0, tz } }
            else { Pick::Bad("I can't read that schedule (try: every 15m, daily 09:30, weekdays 08:00, @hourly, or a cron line), or give a one-time time with at".into()) },
        (true, false) => {
            // Bots.plan.read: a legacy routine's wall time is read in the machine's zone
            let read = if tz.is_empty() { z.settings.get("tz.machine").cloned().unwrap_or_default() } else { tz.clone() };
            let rule = z.rule_of(&read).or_else(|| if when_independent(at) { Some(TzRule::utc0()) } else { None });
            match rule {
                // the app's one "When" box sends what is no schedule as a time: words that ask for a repeat are told how a repeat is written, not how a date is
                Some(r) => match when_parse(at, now, &r) { Some(t) => Pick::Plan { cron: String::new(), once: t, tz }, None => Pick::Bad(if repeats(at) { REPEAT_TEXT.to_string() } else { format!("I can't read that time. {WHEN_TEXT}") }) },
                None => Pick::Bad("I don't have that time zone's rules yet; try again in a minute (or give the time with its offset, or as in 20m)".into()),
            }
        }
    }
}
/// what a time that cannot be read is told when it asks for a repeat
const REPEAT_TEXT: &str = "I can't read that schedule. I can read: every 15m, every 2 hours, hourly, daily 09:30, weekdays 08:00, or a cron line.";
/// Bots.plan.repeats: the text asks for something that repeats (its first word says so)
fn repeats(at: &str) -> bool { let l = at.trim().to_lowercase(); let w = l.split(' ').next().unwrap_or(""); ["every", "each", "daily", "hourly", "weekly", "monthly", "weekdays", "weekends", "nightly"].contains(&w) || w.starts_with('@') }
/// Alert.text: one line, cut to max characters
fn alert_text(t: &str, max: usize) -> String { t.trim().chars().take(max).map(|c| if c == '\t' || c == '\n' { ' ' } else { c }).collect() }
/// the machine's offset at `now` for legacy routines (Tz{off, east}): from the zone the hub logged, else UTC
pub fn machine_off(z: &Zones, now: u64) -> (u64, bool) {
    let o = z.rule(z.settings.get("tz.machine").map_or("UTC", String::as_str)).unwrap_or_else(TzRule::utc0).off(now).off; (o.unsigned_abs(), o >= 0)
}
fn plan_of(r: &Routine) -> Plan<'_> { Plan { cron: &r.cron, on: r.on, deleted: r.deleted, done: r.done, last: r.last, once: r.once, tz: &r.tz, slot: r.slot } }

/// A mail a firing files instead of delivering (the mailroom is on, or the bot is the uberbot: Mail.funnel)
pub struct Filing { pub origin: super::mail::Origin, pub to: String, pub text: String, pub msg: String }
/// One delivery a firing makes after its events are logged
pub struct Wake { pub thread: String, pub msg: String, pub text: String, pub origin: Option<(String, String, String)> }

impl Bots {
    /// bots.routine {bot, routine?, name, schedule|cron | at, tz?, prompt, on?, keep?}
    pub(super) async fn routine(&self, id: &Value, p: &Value) -> Vec<Value> {
        let bot = gs(p, "bot");
        let w = self.world().await;
        if !w.live(bot) { return vec![err(id, "no such bot")] }
        let now = now_s(); let rid_in = gs(p, "routine");
        let rid = if rid_in.is_empty() { self.hub_id("o", now) } else { rid_in.to_string() };
        let name = p.get("name").and_then(Value::as_str).unwrap_or("routine");
        let prompt = gs(p, "prompt"); let on = p.get("on").and_then(Value::as_bool).unwrap_or(true);
        let ex = w.routines.get(rid_in).filter(|r| !r.deleted);
        if p.get("keep").and_then(Value::as_bool).unwrap_or(false) { // a toggle keeps the plan and the schedule as they are: only RoutineSet
            let cron = w.routines.get(rid_in).map_or("", |r| r.cron.as_str());
            return self.commit_reply_pub(id, w.also(now, vec![json!({"$":"RoutineSet","id":rid_in,"bot":bot,"name":name,"cron":cron,"prompt":prompt,"on":on,"at":now})])).await
        }
        let sch = if gs(p, "cron").is_empty() { gs(p, "schedule") } else { gs(p, "cron") };
        let z = Zones { settings: &w.settings };
        match plan(&z, now, gs(p, "tz"), sch, gs(p, "at"), ex) {
            Pick::Bad(m) => vec![err(id, &m)],
            Pick::Plan { cron, once, tz } => {
                let kind = w.routines.get(rid_in).map_or("", |r| r.kind.as_str());
                self.commit_reply_pub(id, w.also(now, vec![json!({"$":"RoutineSet","id":rid,"bot":bot,"name":name,"cron":cron,"prompt":prompt,"on":on,"at":now}),
                    json!({"$":"RoutinePlan","id":rid,"once":once,"tz":tz,"kind":kind,"at":now})])).await
            }
        }
    }

    /// The events a routine firing logs, and the message its bot hears. `Bots.fire`: a reminder is delivered to the person, a routine wakes its bot at hop 0.
    fn fire(&self, w: &World, r: &Routine, now: u64, i: usize, alert_prev: &str, notify_reminders: &str) -> Option<(Vec<Value>, Option<Wake>, Option<Filing>)> {
        let z = Zones { settings: &w.settings };
        let more = if r.once > 0 { vec![json!({"$":"RoutineDone","id":r.id,"at":now})] } else { let wall = plan_of(r).wall(&z, now); if wall > 0 { vec![json!({"$":"RoutineSlot","id":r.id,"slot":wall,"at":now})] } else { vec![] } };
        let fired = std::iter::once(json!({"$":"RoutineFired","id":r.id,"at":now})).chain(more);
        let bot = w.bot(&r.bot).filter(|b| !b.deleted);
        if r.kind == "remind" {
            let mut ev: Vec<Value> = vec![];
            if let Some(b) = bot { ev.push(json!({"$":"Reminded","id":r.id,"bot":b.id,"thread":b.thread,"text":r.prompt,"at":now})) }
            ev.extend(fired);
            let thread = bot.map_or("", |b| b.thread.as_str());
            let on = !matches!(notify_reminders, "off"); // Policy.on "reminders": default on
            let memory = alert_prev.split('\t').nth(2).unwrap_or("");
            let stamp = format!("{now}.{}", self.hub.store.len());
            let line = if on { format!("reminder\tremind-{}\t{memory}\t{stamp}\t{}", r.id, alert_text(&r.prompt, 500)) } else { format!("\t\t{memory}\t{stamp}\t") };
            ev.push(json!({"$":"SettingSet","key":format!("alert.{thread}"),"value":line,"at":now}));
            if let (true, Some(s)) = (on, crate::push::Source::reminder(w, r)) { ev.push(s.order(&format!("remind-{}-{now}", r.id), "reminder", thread, "", &r.prompt, now)) }
            return Some((ev, None, None));
        }
        let mut ev: Vec<Value> = fired.collect();
        if w.setting("mail.on", "off") == "on" || w.triage_owner(&r.bot) { // Bots.wake.origin: the mailroom files it, with the routine's id as its reference
            let filing = bot.map(|b| { ev.push(json!({"$":"BotWoke","bot":b.id,"hop":0,"at":now}));
                Filing { origin: super::mail::Origin::new(&format!("routine:{}", r.id), "routine", &r.id), to: format!("bot:{}", b.id), text: format!("[routine: {}]\n{}", r.name, r.prompt), msg: format!("m{now}-{}.{i}", self.hub.store.len()) } });
            return Some((ev, None, filing));
        }
        let wake = bot.map(|b| {
            ev.push(json!({"$":"BotWoke","bot":b.id,"hop":0,"at":now}));
            let text = format!("[routine: {}]\n{}", r.name, r.prompt);
            Wake { thread: b.thread.clone(), msg: format!("m{now}-{}.{i}", self.hub.store.len()), origin: Some((format!("routine:{}", r.name), "routine".into(), r.name.clone())), text }
        });
        Some((ev, wake, None))
    }

    /// The firing of one routine: `w` is the world it was decided on, let go before any delivery (an agent's actor must never wait on a lock held for it)
    async fn fire_commit(&self, r: &Routine, w: tokio::sync::MutexGuard<'_, World>, now: u64, i: usize, gate: bool) -> Result<bool, String> {
        let alert_prev = if r.kind == "remind" { let th = w.bot(&r.bot).map(|b| b.thread.clone()).unwrap_or_default(); self.hub.setting(&format!("alert.{th}")).await.unwrap_or_default() } else { String::new() };
        let nr = self.hub.setting("notify.reminders").await.unwrap_or_default();
        let Some((mut ev, wake, filing)) = self.fire(&w, r, now, i, &alert_prev, &nr) else { return Ok(false) };
        if gate { // the uberbot's routine: a "fired" op tells the other machines which slot ran (Bots.tick.uber)
            let z = Zones { settings: &w.settings }; let (mo, me) = machine_off(&z, now);
            let slot = plan_of(r).slot_next(&z, mo, me);
            ev.extend(w.ops(now, &[super::uber::b_fired(&super::uber::gid(&w.me(), &r.id), slot)]));
        }
        self.hub.commit_many(&ev).await.map_err(|e| format!("{e:#}"))?;
        if let Some(f) = filing { if let Err(e) = self.file_mail(&w, now, &f.origin, &f.to, &f.text, &f.msg).await { eprintln!("bots: could not file a mail: {e}") } }
        drop(w);
        if let Some(k) = wake { let _ = self.inbox_deliver(&k).await; }
        Ok(true)
    }

    /// bots.routine_run {routine}: run now; a done or removed routine is not run again
    pub(super) async fn routine_run(&self, id: &Value, routine: &str) -> Vec<Value> {
        let w = self.world().await;
        let Some(r) = w.routines.get(routine).cloned().filter(|r| !r.deleted && !r.done) else { return vec![err(id, "That routine is done or removed; edit its time to schedule it again.")] };
        let now = now_s();
        match self.fire_commit(&r, w, now, 0, false).await { Ok(_) => vec![ok(id)], Err(e) => vec![err(id, &e)] }
    }

    /// Bots.tick.routines: every routine due at `now` runs once. Each is decided on the world the one before it left (Bots.tick.state): an uberbot's op counter moves within a tick,
    /// and a routine overdue by hours is one firing: its `last` moves to now, and for the uberbot the slot it answers is the next after the last run, so it never fires per missed minute.
    pub async fn tick_at(&self, now: u64) -> usize {
        if crate::copy::on() { return 0 } // a hub on a copy of a log fires no routine, reminder or wake
        let ids: Vec<String> = { let w = self.world().await; let mut v: Vec<String> = w.routines.keys().cloned().collect(); v.sort(); v }; // Map.values: by id
        let mut n = 0;
        for (i, id) in ids.iter().enumerate() {
            let w = self.world().await;
            let z = Zones { settings: &w.settings }; let (mo, me) = machine_off(&z, now);
            let Some(r) = w.routines.get(id).filter(|r| plan_of(r).due(&z, now, mo, me)).cloned() else { continue };
            let uber = w.uber_is(&r.bot);
            if uber && !w.fires(&super::uber::gid(&w.me(), &r.id), plan_of(&r).slot_next(&z, mo, me)) { continue }
            if let Ok(true) = self.fire_commit(&r, w, now, i, uber).await { n += 1 }
        }
        n
    }

    /// Mail.due: with the mailroom on, a snooze that is over reopens its conversation unread
    pub async fn mail_due(&self, now: u64) {
        if crate::copy::on() { return }
        let w = self.world().await; if w.setting("mail.on", "off") != "on" { return }
        let evs: Vec<Value> = w.mail.due(now).into_iter().map(|m| json!({"$":"MailMarked","bot":m.bot,"conv":m.conv,"status":"unread","seen":m.seen,"until":0,"at":now})).collect();
        if !evs.is_empty() { if let Err(e) = self.hub.commit_many(&evs).await { eprintln!("mail: {e:#}") } }
    }
    /// The soonest instant after `now`, no later than `until`, a live routine is due (Sched.soon); 0: none
    pub async fn soon(&self, now: u64, until: u64) -> u64 {
        let w = self.world().await; let z = Zones { settings: &w.settings }; let (mo, me) = machine_off(&z, now);
        w.routines.values().map(|r| plan_of(r).next(&z, now, mo, me)).filter(|t| *t > now && *t <= until).min().unwrap_or(0)
    }

    /// The minute tick, and a wake for a routine whose time comes between two ticks
    pub fn start_ticker(self: &Arc<Self>) {
        if crate::copy::on() || crate::guard::safe() { return } // nothing runs by itself on a copy of a log, or in safe mode
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                let Some(b) = me.upgrade() else { return };
                let now = now_s(); let minute = (now / 60 + 1) * 60;
                let at = match b.soon(now, minute).await { 0 => minute, t => t };
                drop(b);
                tokio::time::sleep(std::time::Duration::from_secs(at.saturating_sub(now_s()).max(1))).await;
                let Some(b) = me.upgrade() else { return };
                b.tick_at(now_s()).await;
                b.mail_due(now_s()).await;
                b.uber_heal(now_s()).await;
                b.uber_init(now_s()).await;
            }
        });
    }
}
