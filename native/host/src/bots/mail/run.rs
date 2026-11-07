//! Running the router: a freshly
//! filed header is routed on the world as it is then, the changes are committed together, and the outs are performed. What a step of the log asks of the mailroom (a turn's end,
//! an ask) is noted as the world folds it and answered by `after_steps`. No message text or address is logged here.
use super::{file::Router, route::{Ctx, Facts, RouteOut, Step, Th}, store, Body};
use crate::bots::{rpc::{now_s, Bots}, world::{StepEnd, World}};
use serde_json::json;
use std::sync::{Arc, Weak};

/// Ephemeral.is: a mail run's thread or a back office
fn ephemeral(thread: &str) -> bool { thread.starts_with("run") || thread.starts_with("mailcoord-") }

/// The router the lane installs: every filed header is routed once, after its commit
pub struct MailRouter(pub Weak<Bots>);
impl Router for MailRouter {
    fn routed(&self, body: &Body) {
        if crate::copy::on() { return } // a hub on a copy of a log starts no run
        let (Some(b), Ok(_)) = (self.0.upgrade(), tokio::runtime::Handle::try_current()) else { return };
        let body = body.clone(); tokio::spawn(async move { b.route_body(body).await });
    }
    /// Mail.failure: only the kind of failure is said, never which conversation
    fn failed(&self, body: &Body, why: store::StoreError) {
        if crate::copy::on() { return }
        let (Some(b), Ok(_)) = (self.0.upgrade(), tokio::runtime::Handle::try_current()) else { return };
        let body = body.clone(); tokio::spawn(async move { let outs = b.decide(&[], |c| c.mail_failure(&body, &why.to_string())).await; b.perform(outs).await });
    }
}

impl Bots {
    /// The facts a step reads from the hub model: threads, project roots, the coordinator's recent messages, and of the threads in `report` their last reply and error
    pub(crate) async fn facts(&self, w: &World, report: &[String]) -> Facts {
        let names = self.names_for(w).await; let ub = w.uber_bot();
        let th = w.bots.get(&ub).map(|b| b.thread.clone()).unwrap_or_default();
        let mut set = std::collections::HashMap::new();
        let mut keys: Vec<String> = vec!["notify.needs".into(), "notify.settled".into()];
        keys.extend(w.bots.values().filter(|b| !b.deleted && !b.thread.is_empty()).map(|b| format!("alert.{}", b.thread)));
        if !th.is_empty() { keys.push(format!("effort.{th}")); for p in ["claude", "codex"] { keys.push(format!("account.thread.{th}.{p}")) } }
        for k in keys { if let Some(v) = self.hub.setting(&k).await { set.insert(k, v); } }
        let m = self.hub.model().await;
        let threads = m.threads.iter().map(|(k, t)| (k.clone(), Th { title: t.title.clone(), project: t.project.clone(), provider: t.provider.clone(), model: t.model.clone(), worktree: t.worktree.clone(), turn: t.turn.clone() })).collect();
        let roots = m.projects.iter().map(|(k, p)| (k.clone(), p.root.clone())).collect();
        let recent = if th.is_empty() { vec![] } else { m.page(&self.hub.store, &th, 0, 32).map(|p| p.rows.into_iter().filter(|r| r.kind == "msg" && r.thread == th).map(|r| r.text).collect()).unwrap_or_default() };
        let mut last = std::collections::HashMap::new();
        for t in report {
            let rows = m.page(&self.hub.store, t, 0, 32).map(|p| p.rows).unwrap_or_default();
            let reply = rows.iter().find(|r| r.kind == "msg" && r.role == "assistant").map(|r| r.text.clone()).unwrap_or_default();
            let error = rows.iter().find(|r| r.kind != "msg" && r.role == "error").map(|r| r.text.clone()).unwrap_or_default();
            last.insert(t.clone(), (reply, error));
        }
        Facts { names, threads, roots, recent, set, last }
    }
    /// Decide on the world as it is, commit the step, let the world go
    pub(crate) async fn decide(self: &Arc<Self>, report: &[String], f: impl FnOnce(&mut Ctx) -> Step) -> Vec<RouteOut> { self.decide_with(report, |c| ((), f(c))).await.1 }
    /// the same, for a decision that also answers its caller (a tool)
    pub(crate) async fn decide_with<T>(self: &Arc<Self>, report: &[String], f: impl FnOnce(&mut Ctx) -> (T, Step)) -> (T, Vec<RouteOut>) { self.decide_acc(report, true, f).await }
    /// `accounts`: Accounts.created binds what the step creates; a step's own after-work (Run.after) comes past it in , and a back office it makes has only its explicit binding
    async fn decide_acc<T>(self: &Arc<Self>, report: &[String], accounts: bool, f: impl FnOnce(&mut Ctx) -> (T, Step)) -> (T, Vec<RouteOut>) {
        let w = self.world().await; let facts = self.facts(&w, report).await;
        let (now, seq) = (now_s(), self.hub.store.len() as u64);
        let mut ctx = Ctx::new(&w, &facts, now, seq); let (t, st) = f(&mut ctx);
        let cs = if accounts { self.accounts_created(st.cs).await } else { st.cs }; // Accounts.created: a run's thread starts on the default account
        if !cs.is_empty() && self.hub.commit_many(&cs).await.is_err() { eprintln!("mail: routing could not be committed"); return (t, vec![]) }
        (t, st.outs)
    }
    /// Mail.route for a header just filed
    pub async fn route_body(self: &Arc<Self>, body: Body) { let outs = self.decide(&[], |c| c.route(&body)).await; self.perform(outs).await }
    /// Run.queue.start for RunNext
    pub async fn run_next(self: &Arc<Self>, bot: String, conv: String) { let outs = self.decide(&[], |c| c.queue_start(&bot, &conv)).await; self.perform(outs).await }
    pub(crate) fn perform(self: &Arc<Self>, outs: Vec<RouteOut>) -> crate::api::Fut<'_, ()> { Box::pin(async move {
        for o in outs {
            match o {
                RouteOut::RunStart { thread, conv, head } => { let me = self.clone(); tokio::spawn(async move { me.run_start(thread, conv, head).await }); }
                RouteOut::RunNext { bot, conv } => { let me = self.clone(); tokio::spawn(async move { me.run_next(bot, conv).await }); }
                RouteOut::OwnerMail => self.owner_kick(),
                RouteOut::Deliver { thread, msg, text, origin, system } => { if let Err(e) = self.inbox_deliver_role(system, &thread, &msg, &text, Some(origin)).await { eprintln!("mail: delivery to a run failed: {e}") } }
                RouteOut::Note { thread, id, text } => { if let Err(e) = self.inbox_note(&thread, &id, &text).await { eprintln!("mail: a note could not ride: {e}") } }
                RouteOut::Ride { thread, id, text } => { if let Err(e) = self.inbox_ride(&thread, &id, &text).await { eprintln!("mail: a note could not ride: {e}") } }
                RouteOut::File { o, addr, text, msg, want, after, conv } => { let w = self.world().await; if let Err(e) = self.file_in(&w, now_s(), &o, &addr, &text, &msg, &want, after, &conv).await { eprintln!("bots: could not file a mail: {e}") } }
            }
        }
    }) }
    /// What the log's new lines ask of the mailroom (Run.after, Run.fallback): a run's end, a managed thread's ask, another bot's report, a lane cut off
    pub async fn after_steps(self: &Arc<Self>) {
        let (steps, on) = { let mut w = self.world().await; (std::mem::take(&mut w.steps), w.setting("mail.on", "off") == "on" || !w.uber_bot().is_empty()) };
        if crate::copy::on() || steps.is_empty() { return }
        let report: Vec<String> = steps.iter().filter_map(|s| match s { StepEnd::Turn { thread, .. } | StepEnd::Fallback { thread, .. } => Some(thread.clone()), _ => None }).collect();
        let ended: Vec<String> = steps.iter().filter_map(|s| match s { StepEnd::Turn { thread, state, .. } if (state == "completed" || state == "failed") && ephemeral(thread) => Some(thread.clone()), _ => None }).collect();
        let (_, outs) = self.decide_acc(&report, false, |c| {
            let mut st = Step::none();
            for s in steps { match s {
                StepEnd::Turn { thread, state, run } => {
                    if on && (state == "completed" || state == "failed") {
                        if let Some(r) = &run { let e = c.run_ended(r, &state); st = st.cat(e) }
                        let b = c.triage_bot_end(&thread, &state); st = st.cat(b);
                        let t = c.triage_thread_end(&thread, &state); st = st.cat(t);
                    }
                    let l = c.private_lost(&thread, &state); st = st.cat(l);
                }
                StepEnd::Ask { id, thread, tool, detail } => { if on && tool != "settings_proposal" { let a = c.triage_ask(&id, &thread, &tool, &detail); st = st.cat(a) } }
                StepEnd::Fallback { thread, run } => { if let Some(r) = &run { let turn = c.th(&thread).turn; let e = c.run_ended(r, &turn); st = st.cat(e) } }
            } }
            ((), st)
        }).await;
        self.perform(outs).await;
        // a mail run or back office that ended keeps no agent process: one is left per run otherwise, and an uberbot's routines start several an hour. A later delivery resumes it by its session.
        for th in ended { let idle = self.hub.model().await.threads.get(&th).map_or(true, |t| t.turn != "running"); if idle { self.inbox.stop(&th).await } }
    }
    /// Follows the log for `after_steps`. The start's fold owes nothing: what it folded is dropped first.
    pub fn start_after(self: &Arc<Self>) {
        if crate::copy::on() { return }
        let (me, mut rx) = (Arc::downgrade(self), self.hub.subscribe());
        tokio::spawn(async move { loop {
            match rx.recv().await { Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {} Err(_) => return }
            let Some(b) = me.upgrade() else { return }; b.after_steps().await;
        } });
    }
    /// Run.recover, at a start: `cut` are the threads the restart interrupted. A run among them is told as cut off; a bound run whose end no step answered (a crash between the turn's end and
    /// its answer: answering again changes nothing once answered) is answered now; with the carry on a queue with nothing live goes on (Run.queue.recover).
    pub async fn run_recover(self: &Arc<Self>, cut: &[String]) {
        if crate::copy::on() || crate::guard::safe() { return }
        let (carry, runs): (bool, Vec<_>) = { let w = self.world().await; let on = w.setting("mail.on", "off") == "on" || !w.uber_bot().is_empty(); if !on { return } (self.hub.setting("restart.continue").await.map_or(true, |v| v != "off"), w.mail.runs.values().cloned().collect()) };
        let report: Vec<String> = cut.to_vec();
        let outs = self.decide(&report, |c| {
            let mut st = Step::none();
            for r in &runs {
                let turn = c.th(&r.thread).turn;
                if cut.contains(&r.thread) { if !c.w.private_is(&r.thread) && c.runs_on(&r.bot) { let s = c.run_recover(&crate::bots::mail::mailbox::Run { live: true, ..r.clone() }, carry); st = st.cat(s) } let l = c.private_lost(&r.thread, "interrupted"); st = st.cat(l) }
                else if !r.live && c.triage_owner(&r.bot) && c.triage_bound(r) && (turn == "completed" || turn == "failed") && !c.disposition_current(r).is_empty() { let s = c.run_ended(&crate::bots::mail::mailbox::Run { live: true, ..r.clone() }, &turn); st = st.cat(s) }
            }
            if carry { for (bot, conv) in c.queues() { st.outs.push(RouteOut::RunNext { bot, conv }) } }
            st
        }).await;
        self.perform(outs).await
    }
    /// RunJob.read: the bound input's exact body, else the conversation's last 20 (newest first); then Run.start
    async fn run_start(self: &Arc<Self>, th: String, conv: String, head: String) {
        let bound = self.world().await.setting(&format!("run.bound.{th}"), "");
        let home = self.hub.cfg.home.clone(); let (c2, b2) = (conv.clone(), bound.clone());
        let read = tokio::task::spawn_blocking(move || {
            let p = store::path(&home, &c2); if !p.exists() { return Err(format!("missing {}", p.display())) }
            let bs = store::read(&p);
            if b2.is_empty() { Ok(bs.into_iter().rev().take(20).collect::<Vec<_>>()) } else { bs.into_iter().find(|b| b.mail.id == b2).map(|b| vec![b]).ok_or_else(|| "triggering mail body unavailable".to_string()) }
        }).await.unwrap_or_else(|_| Err("read failed".into()));
        let now = now_s();
        match read {
            Err(e) => { let id = crate::agents::lines::hub_id("run-read", now, self.hub.store.len() as u64);
                let _ = self.hub.commit_many(&[json!({"$":"ActivityLogged","thread":th,"act":id,"tone":"error","kind":"mail","summary":format!("triggering mail body unavailable: {e}"),"at":now}),
                    json!({"$":"TurnChanged","thread":th,"state":"failed","at":now}), json!({"$":"ThreadSettled","id":th,"manual":false,"at":now})]).await; }
            Ok(bs) => self.run_ready(th, head, bs).await,
        }
    }
    /// Run.start.on (Run.ready): the first turn of a live run still waiting for its brief
    async fn run_ready(self: &Arc<Self>, th: String, head: String, bs: Vec<Body>) {
        let (ready, now, seq) = { let w = self.world().await; (w.mail.run_of(&th).map_or(false, |r| r.live) && w.setting(&format!("run.pending.{th}"), "") == "on", now_s(), self.hub.store.len() as u64) };
        if !ready { return }
        // Run.pending.text: what reached the run while it waited for its brief, newest first
        let pending: String = self.hub.model().await.page(&self.hub.store, &th, 0, 32).map(|p| p.rows.iter().filter(|r| r.kind == "msg" && r.thread == th).map(|r| format!("{}\n", super::route::take_chars(&r.text, 2000))).collect()).unwrap_or_default();
        let bodies: String = bs.iter().map(|b| format!("\n[mail {} from {}]\n{}", b.mail.id, b.mail.from, super::route::take_chars(&b.text, 4000))).collect();
        let text = format!("{head}{}\n<mail_history newest-first>\n{}\n</mail_history>", super::route::take_chars(&pending, 2000), super::route::take_chars(&bodies, 8000));
        if self.hub.commit(&json!({"$":"SettingSet","key":format!("run.pending.{th}"),"value":"","at":now})).await.is_err() { return }
        let msg = crate::agents::lines::hub_id("brief", now, seq);
        if let Err(e) = self.inbox_deliver_raw(&th, &msg, &text, None).await { eprintln!("mail: a run could not be started: {e}") }
    }
}
