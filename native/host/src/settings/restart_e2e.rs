//! A start never fails on account availability while any route to a working login exists. Nov 7: every restart with turns cut on a saved account (Bub's profile, the
//! default for new threads) logged "could not start claude: Accounts are not available; select the default account" for each carried turn, a minute apart, and a resend a
//! minute later worked. The agents' recovery carried the turns on inside `Agents::start`, each carry's spawn waited a minute for the account manager, and the manager was
//! handed to the agents only by `settings::start`, after `Agents::start` returned: a deadlock the wait could only time out of (the hub log: "agents is not running: no
//! answer in 120 s", "agents started" at +120015 ms).
//! Here, a real hub on a temp home, the real agents, commands and account manager in the order `main` starts them, and the failover stand-in CLI. Stand-ins only.
use crate::{agents::{proc::Cli, Agents}, hub::{Cfg, Hub}, presenter::Canned, settings::{accounts as ac, accounts_hub, accounts_io::Accounts}};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant}};

const STANDIN: &str = include_str!("../../../../test/agents/fixtures/failover-standin.py");
const BUB: &str = "b1790292962-8017";
const BUB_T: &str = "t1790292962-8017";
const NOT_AVAILABLE: &str = "Accounts are not available";

fn fresh(name: &str) -> PathBuf { static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0); let p = std::env::temp_dir().join(format!("bp-rs-{}-{name}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed))); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
fn now() -> u64 { crate::hub::now_ms() / 1000 }
fn tid(i: usize) -> String { format!("t17000020{i:02}-{i}") }
fn cfg(home: &Path) -> Cfg { Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() } }

/// The stand-ins, their record folder and a work folder.
struct Bins { rec: PathBuf, bin: PathBuf, work: PathBuf }
fn bins(name: &str) -> Bins {
    use crate::plat::fs::PermissionsExt;
    let (rec, bin, work) = (fresh(&format!("{name}-rec")), fresh(&format!("{name}-bin")), fresh(&format!("{name}-work")));
    for cli in ["claude", "codex"] { let p = bin.join(cli); std::fs::write(&p, STANDIN.replace("@DIR@", &crate::plat::fs::script_path(&rec))).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    Bins { rec, bin, work }
}
impl Bins {
    fn mode(&self, who: &str, m: &str) { std::fs::write(self.rec.join(format!("mode.{who}")), m).unwrap() }
    fn turns(&self) -> Vec<Value> { std::fs::read_to_string(self.rec.join("turns.jsonl")).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect() }
}

/// A home as the live one was before a restart: a saved Claude profile (epoch 2) that is the default for new threads, Bub (the uberbot) and `n` worker threads bound to it,
/// and every one of their turns running when the hub stopped. Returns the profile.
async fn saved_home(home: &Path, b: &Bins, n: usize, extra_profiles: &[&str]) -> Vec<(String, u64)> {
    let cli = Cli::standin(&b.bin);
    let mgr = Arc::new(Accounts::open(home, Arc::new(move |p: &str| cli.resolve(p))));
    let mut ps = vec![mgr.plant("claude", "Work", "w@x.com", Value::Null).await];
    for (i, label) in extra_profiles.iter().enumerate() { ps.push(mgr.plant("claude", label, &format!("p{i}@x.com"), Value::Null).await) }
    let (id, epoch) = ps[0].clone();
    let mut l = format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"f\",\"root\":{},\"at\":1700000000}}\n", serde_json::to_string(&b.work.to_string_lossy()).unwrap());
    let set = |k: &str, v: &str| format!("{}\n", json!({"$":"SettingSet","key":k,"value":v,"at":1700000002}));
    l += &set("account.default.claude", &ac::binding_show(&id, epoch));
    let mut ths: Vec<String> = vec![BUB_T.to_string()]; ths.extend((0..n).map(tid));
    for (i, th) in ths.iter().enumerate() {
        l += &format!("{}\n", json!({"$":"ThreadCreated","id":th,"project":"p1","title":format!("thread {i}"),"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000001}));
        l += &set(&ac::binding_key(th, "claude"), &ac::binding_show(&id, epoch));
    }
    l += &format!("{}\n", json!({"$":"BotSet","id":BUB,"name":"Bub","thread":BUB_T,"look":1,"persona":"","at":1700000003}));
    l += &set("uber.bot", BUB);
    // the worker threads have run longer than Bub: oldest first would put Bub last
    for (i, th) in ths.iter().enumerate() { let at = if th == BUB_T { 1700000200 } else { 1700000100 + i as u64 };
        l += &format!("{}\n{}\n", json!({"$":"MessagePosted","thread":th,"msg":format!("u{i}"),"role":"user","text":"do the work","at":at}), json!({"$":"TurnChanged","thread":th,"state":"running","at":at})); }
    std::fs::write(home.join("events.jsonl"), l).unwrap();
    ps
}

struct Up { hub: Arc<Hub>, ag: Arc<Agents>, mgr: Arc<Accounts>, took: Duration, from: usize }
/// The hub's start as `main` runs it: the hub, the agents (whose recovery carries the cut turns on), the commands, then settings' account manager.
async fn boot(home: &Path, b: &Bins) -> Up {
    let hub = Hub::start(cfg(home), Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let (t0, from) = (Instant::now(), hub.store.len());
    let ag = tokio::time::timeout(Duration::from_secs(60), Agents::start_with(hub.clone(), Cli::standin(&b.bin))).await
        .expect("the agents' start never ended: its recovery waited on the account manager it is only given once it has returned").unwrap();
    let took = t0.elapsed();
    crate::commands::start_with(hub.clone(), Some(Arc::new(ag.clone())));
    let mgr = accounts_hub::start_cli(&hub, Some(ag.clone()), Cli::standin(&b.bin));
    Up { hub, ag, mgr, took, from }
}
fn log(hub: &Arc<Hub>) -> Vec<Value> { hub.store.range(0, usize::MAX).unwrap().iter().filter_map(|l| serde_json::from_str(l).ok()).collect() }
fn raw(hub: &Arc<Hub>) -> String { hub.store.range(0, usize::MAX).unwrap().join("\n") }
fn lines(hub: &Arc<Hub>, th: &str) -> Vec<String> { log(hub).iter().filter(|e| e["$"] == "ActivityLogged" && e["thread"] == th).filter_map(|e| e["summary"].as_str().map(str::to_string)).collect() }
fn carried(hub: &Arc<Hub>) -> Vec<String> { carried_since(hub, 0) }
fn carried_since(hub: &Arc<Hub>, from: usize) -> Vec<String> { log(hub)[from..].iter().filter(|e| e["$"] == "MessagePosted" && e["role"] == "user" && e["text"].as_str().unwrap_or("").starts_with(crate::agents::world::CARRY_TEXT)).filter_map(|e| e["thread"].as_str().map(str::to_string)).collect() }
async fn turn_of(hub: &Arc<Hub>, th: &str) -> String { hub.model().await.threads.get(th).map(|t| t.turn.clone()).unwrap_or_default() }
/// Until every one of `ths` is at rest in `state`.
async fn all(hub: &Arc<Hub>, ths: &[String], state: &str, secs: u64) {
    let end = Instant::now() + Duration::from_secs(secs);
    loop {
        let mut left = vec![]; for th in ths { let s = turn_of(hub, th).await; if s != state { left.push(format!("{th}: {s} {:?}", lines(hub, th))) } }
        if left.is_empty() { return }
        assert!(Instant::now() < end, "not {state}:\n{}", left.join("\n"));
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
/// Until no thread's provider process holds any saved account (`Accounts::runs_on`): a stopped turn's child is gone, so a disconnect is not refused or raced.
async fn quiet(mgr: &Arc<Accounts>, ths: &[String]) {
    let end = Instant::now() + Duration::from_secs(60);
    while ths.iter().any(|t| mgr.runs_on(t).is_some()) { assert!(Instant::now() < end, "a child still holds an account: {:?}", ths.iter().map(|t| (t, mgr.runs_on(t))).collect::<Vec<_>>()); tokio::time::sleep(Duration::from_millis(20)).await }
}
fn threads(n: usize) -> Vec<String> { let mut v = vec![BUB_T.to_string()]; v.extend((0..n).map(tid)); v }
/// A fresh home holding what `from` holds (its log and its private account registry and stores): the next start of the hub. The old hub keeps its own files, so nothing
/// it still writes on its way out lands in the new one's log.
fn restart_copy(from: &Path, to: &Path) {
    std::fs::copy(from.join("events.jsonl"), to.join("events.jsonl")).unwrap();
    fn copy_dir(a: &Path, b: &Path) { use crate::plat::fs::PermissionsExt; std::fs::create_dir_all(b).unwrap(); std::fs::set_permissions(b, std::fs::Permissions::from_mode(0o700)).unwrap();
        for e in std::fs::read_dir(a).unwrap().flatten() { let (p, q) = (e.path(), b.join(e.file_name())); if p.is_dir() { copy_dir(&p, &q) } else if !e.file_name().to_string_lossy().ends_with(".tmp") { std::fs::copy(&p, &q).unwrap(); } } }
    copy_dir(&from.join("providers"), &to.join("providers"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_restart_with_turns_cut_on_a_saved_account_carries_every_one_on_under_it_with_bub_first() {
    let b = bins("one"); let home = fresh("one-home");
    let ps = saved_home(&home, &b, 6, &[]).await;
    let up = boot(&home, &b).await;
    assert!(up.took < Duration::from_secs(30), "the agents' start does not wait on the turns it carries on: {:?}", up.took);
    let ths = threads(6);
    all(&up.hub, &ths, "completed", 60).await;
    let text = raw(&up.hub); assert!(!text.contains(NOT_AVAILABLE), "no start failed on the accounts:\n{text}");
    let c = carried(&up.hub); assert_eq!(c.len(), ths.len(), "every cut turn carried on once: {c:?}"); assert_eq!(c[0], BUB_T, "the uberbot first: {c:?}");
    let t = b.turns(); assert!(t.iter().all(|x| x["who"] == "w@x.com"), "every one ran under the saved profile, none on another login: {t:#?}");
    assert_eq!(up.mgr.snapshot()["profiles"][0]["id"], ps[0].0.as_str());
    up.ag.shutdown(); let _ = std::fs::remove_dir_all(&home);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_status_check_after_a_restart_is_waited_out_not_refused() {
    let b = bins("slow"); let home = fresh("slow-home");
    saved_home(&home, &b, 2, &[]).await;
    std::fs::write(b.rec.join("authslow"), "2.5").unwrap(); // the inventory's first check of the profile takes a while
    let up = boot(&home, &b).await;
    assert!(up.took < Duration::from_secs(30), "{:?}", up.took);
    all(&up.hub, &threads(2), "completed", 60).await;
    assert!(!raw(&up.hub).contains(NOT_AVAILABLE)); assert!(b.turns().iter().all(|x| x["who"] == "w@x.com"), "{:#?}", b.turns());
    up.ag.shutdown(); let _ = std::fs::remove_dir_all(&home);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bound_profile_that_cannot_start_falls_back_to_another_account_then_to_the_cli_login_and_says_so_once() {
    let b = bins("fb"); let home = fresh("fb-home");
    let ps = saved_home(&home, &b, 1, &["Personal"]).await;
    let up = boot(&home, &b).await;
    all(&up.hub, &threads(1), "completed", 60).await;
    // Work is signed out on the side (its store is gone): the next start cannot run on it
    let gone = up.mgr.snapshot()["profiles"][0]["id"].as_str().unwrap().to_string(); assert_eq!(gone, ps[0].0);
    for th in threads(1) { up.ag.stop(&th).await.unwrap() } quiet(&up.mgr, &threads(1)).await; // no child of it still runs
    let r = up.mgr.request(&json!({"op":"disconnect","profile":gone})).await; assert_eq!(r["error"], "", "{r}");
    let before = b.turns().len();
    up.ag.deliver_person(&tid(0), "m-fb1", "go on", vec![]).await.unwrap();
    all(&up.hub, &[tid(0)], "completed", 30).await;
    let t = b.turns(); assert_eq!(t.len(), before + 1); assert_eq!(t[before]["who"], "p0@x.com", "another saved account took the turn: {t:#?}");
    let said: Vec<String> = lines(&up.hub, &tid(0)).into_iter().filter(|l| l.starts_with("Started on")).collect();
    assert_eq!(said, vec!["Started on Personal: Work cannot start a turn now (Reconnect and select this account before starting a turn)".to_string()], "one line");
    // and with Personal gone too, the computer's own Claude login
    up.ag.stop(&tid(0)).await.unwrap(); quiet(&up.mgr, &threads(1)).await;
    let r = up.mgr.request(&json!({"op":"disconnect","profile":ps[1].0})).await; assert_eq!(r["error"], "", "{r}");
    std::fs::write(b.rec.join("default.login"), json!({"email":"me@x.com"}).to_string()).unwrap(); // the CLI's own login is signed in
    up.ag.deliver_person(&tid(0), "m-fb2", "and on", vec![]).await.unwrap();
    let end = Instant::now() + Duration::from_secs(120); while b.turns().len() < before + 2 { assert!(Instant::now() < end, "{:?}", lines(&up.hub, &tid(0))); tokio::time::sleep(Duration::from_millis(100)).await }
    all(&up.hub, &[tid(0)], "completed", 30).await;
    assert_eq!(b.turns()[before + 1]["who"], "me@x.com", "the CLI's default login: {:#?}", b.turns());
    assert!(lines(&up.hub, &tid(0)).iter().any(|l| l == "Started on this computer's own Claude login: Work cannot start a turn now (Reconnect and select this account before starting a turn)"), "{:?}", lines(&up.hub, &tid(0)));
    assert!(!raw(&up.hub).contains(NOT_AVAILABLE));
    up.ag.shutdown(); let _ = std::fs::remove_dir_all(&home);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_start_that_still_cannot_get_an_account_retries_by_itself_and_only_then_alerts_once() {
    let b = bins("rt"); let home = fresh("rt-home");
    saved_home(&home, &b, 2, &[]).await;
    let up = boot(&home, &b).await;
    all(&up.hub, &threads(2), "completed", 60).await;
    // two starts in a row cannot get an account at all: the turn waits and tries again by itself, and runs
    for th in threads(2) { up.ag.stop(&th).await.unwrap() } // a start, not a message into a live process
    crate::agents::account_fail_next(&up.hub, &tid(0), 2);
    up.ag.deliver_person(&tid(0), "m-rt1", "go", vec![]).await.unwrap();
    all(&up.hub, &[tid(0)], "completed", 30).await;
    let l = lines(&up.hub, &tid(0)); assert_eq!(l.iter().filter(|x| x.starts_with("Waiting for an account to start on")).count(), 2, "{l:?}");
    assert!(!log(&up.hub).iter().any(|e| e["thread"] == tid(0).as_str() && e["$"] == "TurnChanged" && e["state"] == "failed"), "no failed turn for a human to resend");
    assert!(up.hub.setting(&accounts_hub::alert_key("claude")).await.unwrap_or_default().is_empty(), "no alert while it got through");
    // two threads that never get one: each fails only after its retries, and the person hears it once
    up.ag.stop(&tid(0)).await.unwrap();
    crate::agents::account_fail_next(&up.hub, &tid(0), 1000); crate::agents::account_fail_next(&up.hub, &tid(1), 1000);
    up.ag.deliver_person(&tid(0), "m-rt2", "go", vec![]).await.unwrap(); up.ag.deliver_person(&tid(1), "m-rt3", "go", vec![]).await.unwrap();
    all(&up.hub, &[tid(0), tid(1)], "failed", 60).await;
    let alerts: Vec<Value> = log(&up.hub).into_iter().filter(|e| e["$"] == "SettingSet" && e["key"] == accounts_hub::alert_key("claude") && e["value"] != "").collect();
    assert_eq!(alerts.len(), 1, "one alert: {alerts:#?}");
    let a: Value = serde_json::from_str(alerts[0]["value"].as_str().unwrap()).unwrap(); assert!(a["text"].as_str().unwrap().starts_with("Claude sign-in needed"), "{a}");
    crate::agents::account_fail_next(&up.hub, &tid(0), 0); crate::agents::account_fail_next(&up.hub, &tid(1), 0);
    up.ag.shutdown(); let _ = std::fs::remove_dir_all(&home);
}

/// The restart storm: twenty restarts, each while ten worker threads and Bub are mid-turn on the saved profile, Bub with mail arriving and a mailroom lane started the moment
/// the hub is back, the account's status check slow on half of them, and a stale hold on the profile. Every turn goes on, none fails on the accounts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_storm_of_twenty_restarts_mid_turn_never_fails_a_start_on_the_accounts() {
    let b = bins("storm"); let n = 10;
    let mut home = fresh("storm-home-0");
    let ps = saved_home(&home, &b, n, &[]).await;
    b.mode("w@x.com", "slow:1.2");
    let ths = threads(n); let mut homes = vec![];
    for r in 0..20 {
        if r % 2 == 1 { std::fs::write(b.rec.join("authslow"), "0.8").unwrap() } else { let _ = std::fs::remove_file(b.rec.join("authslow")); }
        let up = boot(&home, &b).await;
        // the start must not wait on a carried turn (the Nov 7 bug: each waited a minute for an account manager that only came once the start returned): that is a minute or more, so 30 s only says
        // "it hung", and a loaded machine (a full gate, a Windows runner) that takes a few seconds is not it
        assert!(up.took < Duration::from_secs(30), "restart {r}: the agents' start took {:?}", up.took);
        if r == 3 { // a hold the profile ran out on hours ago, never cleared
            up.hub.commit(&json!({"$":"SettingSet","key":crate::settings::fallback::hold_key("claude", &ps[0].0, ps[0].1),"value":json!({"scope":"claude","reset":now() - 3600,"at":now() - 7200}).to_string(),"at":now()})).await.unwrap(); }
        // Bub's mail, and a mailroom lane, the moment the hub is back
        let id = format!("mail-{r}");
        { let ag = up.ag.clone(); tokio::spawn(async move { let _ = ag.deliver(BUB_T, &id, "a mail for you", Some(("bot".into(), "mail".into(), String::new())), vec![]).await; }); }
        let lane = format!("mailcoord-{BUB}-turn{r}");
        up.hub.commit(&json!({"$":"ThreadCreated","id":lane,"project":"p1","title":"lane","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":now()})).await.unwrap();
        up.hub.commit(&json!({"$":"SettingSet","key":ac::binding_key(&lane, "claude"),"value":ac::binding_show(&ps[0].0, ps[0].1),"at":now()})).await.unwrap();
        { let ag = up.ag.clone(); tokio::spawn(async move { let _ = ag.deliver(&lane, &format!("lane-{r}"), "sort the mail", None, vec![]).await; }); }
        // mid-turn: those carried on so far are running and have said something (a turn carried on and cut again before it did anything is left stopped by design:
        // world.rs carry_twice_once), and Bub has its mail
        let t0 = Instant::now(); let mut held = false; let mut last_n = usize::MAX; let mut steady = Instant::now();
        loop {
            // past the first look, no further carry starts: the restart comes only once the carried set has stopped growing and every one of them has said something
            if !held && t0.elapsed() > Duration::from_millis(700) { up.ag.carry_hold(); held = true; steady = Instant::now(); }
            let ev = log(&up.hub); let since = &ev[up.from..];
            // the carried threads from the same snapshot as `since` (a second look at the log may see a carry the first did not, and the search below would find none)
            let c: Vec<String> = since.iter().filter(|e| e["$"] == "MessagePosted" && e["role"] == "user" && e["text"].as_str().unwrap_or("").starts_with(crate::agents::world::CARRY_TEXT)).filter_map(|e| e["thread"].as_str().map(str::to_string)).collect();
            let busy = c.iter().all(|th| { let at = since.iter().rposition(|e| e["$"] == "MessagePosted" && e["thread"] == th.as_str() && e["text"].as_str().unwrap_or("").starts_with(crate::agents::world::CARRY_TEXT)).unwrap(); since[at..].iter().any(|e| e["$"] == "MessagePosted" && e["thread"] == th.as_str() && e["role"] == "assistant") });
            let mailed = since.iter().any(|e| e["$"] == "MessagePosted" && e["msg"] == format!("mail-{r}").as_str());
            if c.len() != last_n { last_n = c.len(); steady = Instant::now(); }
            if held && busy && mailed && !c.is_empty() && steady.elapsed() > Duration::from_millis(400) { break }
            assert!(t0.elapsed() < Duration::from_secs(300), "restart {r}: the carried turns never got going: {c:?} {:?}", lines(&up.hub, BUB_T));
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let text = raw(&up.hub); assert!(!text.contains(NOT_AVAILABLE), "restart {r}: a start failed on the accounts");
        let ls = turn_of(&up.hub, &format!("mailcoord-{BUB}-turn{r}")).await; assert!(ls == "running" || ls == "completed", "restart {r}: the mailroom lane started: {ls}");
        let c = carried_since(&up.hub, up.from); if c.iter().any(|t| t == BUB_T) { assert_eq!(c[0], BUB_T, "restart {r}: Bub goes on first: {c:?}"); }
        up.ag.shutdown();
        let next = fresh(&format!("storm-home-{}", r + 1)); restart_copy(&home, &next); homes.push(home); home = next;
    }
    let _ = std::fs::remove_file(b.rec.join("authslow"));
    let up = boot(&home, &b).await;
    all(&up.hub, &ths, "completed", 300).await;
    for h in homes.iter().chain([&home]) { let t = std::fs::read_to_string(h.join("events.jsonl")).unwrap(); assert!(!t.contains(NOT_AVAILABLE), "{}: a start failed on the accounts", h.display()); }
    let ev = log(&up.hub);
    for th in &ths { assert!(!ev.iter().any(|e| e["thread"] == th.as_str() && e["$"] == "TurnChanged" && e["state"] == "failed"), "{th} failed: {:?}", lines(&up.hub, th)) }
    let ran: Vec<String> = b.turns().iter().filter_map(|t| t["text"].as_str().map(str::to_string)).collect();
    // every mail is in Bub's thread for good; whether its agent read one before the next cut is timing (a message on a running turn's input that a restart cuts is not
    // carried on in its words: the carry says "continue")
    let posted = (0..20).filter(|r| ev.iter().any(|e| e["$"] == "MessagePosted" && e["thread"] == BUB_T && e["msg"] == format!("mail-{r}").as_str())).count();
    assert_eq!(posted, 20, "every one of Bub's mails is in its thread");
    assert!(b.turns().iter().all(|x| x["who"] == "w@x.com"), "every turn on the saved profile");
    let fails: usize = homes.iter().chain([&home]).map(|h| std::fs::read_to_string(h.join("events.jsonl")).unwrap().matches(NOT_AVAILABLE).count()).sum();
    eprintln!("storm: 20 restarts, {} threads, {} carries, {} turns started, {} with Bub's mail, {} mailroom lanes, every thread completed, {fails} account failures", ths.len(), carried(&up.hub).len(), b.turns().len(), ran.iter().filter(|t| t.contains("a mail for you")).count(), ran.iter().filter(|t| t.contains("sort the mail")).count());
    up.ag.shutdown(); for h in homes { let _ = std::fs::remove_dir_all(h); } let _ = std::fs::remove_dir_all(&home);
}

// ---- the pieces, alone ----
#[test] fn an_account_error_is_told_from_any_other_start_failure() {
    for e in ["Accounts are not available; select the default account", "Reconnect and select this account before starting a turn", "This account's status check did not finish; start the turn again", "Account identity changed; select this account again", "Account storage failed"] { assert!(crate::agents::account_err(e), "{e}") }
    for e in ["claude is not installed", "this thread's worktree is not ready", "agent exited with code 1", crate::settings::accounts_io::EXPIRED] { assert!(!crate::agents::account_err(e), "{e}: not an account error to retry (an expired sign-in goes to the failover)") }
}
#[test] fn retries_wait_longer_each_time_and_never_long() {
    let w: Vec<Duration> = (0..8).map(crate::agents::account_backoff).collect();
    assert!(w.windows(2).all(|p| p[0] <= p[1]), "{w:?}"); assert!(w.iter().all(|d| *d <= Duration::from_millis(400)), "a test waits little: {w:?}");
    assert!(crate::agents::ACCOUNT_RETRIES_BUB > crate::agents::ACCOUNT_RETRIES, "the uberbot gets the most tries");
}
#[test] fn the_start_alert_is_the_sign_in_record_once() {
    let a = accounts_hub::start_alert(&|_| String::new(), "claude", "w", "\u{201c}Bub\u{201d}", "Reconnect", 9).unwrap();
    assert_eq!((a["text"].as_str().unwrap(), a["link"].as_str().unwrap(), a["provider"].as_str().unwrap()), ("Claude sign-in needed: no account could start \u{201c}Bub\u{201d} (Reconnect)", "account:w", "claude"));
    let told = a.to_string(); assert!(accounts_hub::start_alert(&|k| if k == accounts_hub::alert_key("claude") { told.clone() } else { String::new() }, "claude", "w", "x", "y", 10).is_none(), "one until it is dealt with");
    assert_eq!(accounts_hub::start_alert(&|_| String::new(), "claude", "", "x", "y", 9).unwrap()["link"], "", "the CLI's own login has no account page");
}
#[test] fn the_uberbot_and_its_mailroom_lanes_go_first() {
    let mut w = crate::agents::world::World::default();
    for l in [json!({"$":"BotSet","id":BUB,"name":"Bub","thread":BUB_T,"look":1,"persona":"","at":2}), json!({"$":"BotSet","id":"b2","name":"Other","thread":"t-other","look":1,"persona":"","at":2}), json!({"$":"SettingSet","key":"uber.bot","value":BUB,"at":3})] { w.apply(&l.to_string()) }
    assert!(Agents::priority(&w, BUB_T)); assert!(Agents::priority(&w, &format!("mailcoord-{BUB}-turn1794092771-329343")));
    assert!(!Agents::priority(&w, "t-other"), "another bot is not the uberbot"); assert!(!Agents::priority(&w, &tid(0)));
    w.apply(&json!({"$":"SettingSet","key":"uber.bot","value":"","at":4}).to_string()); assert!(!Agents::priority(&w, BUB_T), "no uberbot, no priority");
}

/// The watchdog's guard (scripts/hub-watchdog.sh `account_check`, POST /dev/account-retry): a turn whose start failed on the accounts, with nothing newer since, starts again;
/// one that failed otherwise, or that someone already sent again, is left alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_watchdogs_retry_starts_again_only_the_turns_that_failed_on_the_accounts() {
    let b = bins("wd"); let home = fresh("wd-home");
    saved_home(&home, &b, 3, &[]).await;
    let up = boot(&home, &b).await;
    all(&up.hub, &threads(3), "completed", 60).await;
    for th in threads(3) { up.ag.stop(&th).await.unwrap() }
    let failed = |th: String, why: &str| { let (hub, why) = (up.hub.clone(), why.to_string()); async move { hub.commit_many(&[json!({"$":"ActivityLogged","thread":th,"act":"spawn-1","tone":"error","kind":"spawn","summary":format!("could not start claude: {why}"),"at":now()}), json!({"$":"TurnChanged","thread":th,"state":"failed","at":now()})]).await.unwrap(); } };
    failed(tid(0), "Accounts are not available; select the default account").await;
    failed(tid(1), "this thread's worktree is not ready").await;
    failed(tid(2), "Accounts are not available; select the default account").await;
    up.hub.commit(&json!({"$":"MessagePosted","thread":tid(2),"msg":"later","role":"assistant","text":"something newer","at":now()})).await.unwrap();
    let before = b.turns().len();
    // through the route, as the watchdog asks
    let req = crate::auth::Req { method: "POST".into(), target: "/dev/account-retry".into(), headers: vec![], body: vec![], peer: None, trusted: false };
    let h = crate::agents::devturns::AccountRetry(up.ag.clone()); use crate::api::HttpHandler;
    let r = h.call(&up.hub, &req).await; let v: Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(v["retried"], json!([tid(0)]), "{v}");
    all(&up.hub, &[tid(0)], "completed", 30).await;
    assert_eq!(b.turns().len(), before + 1); assert!(b.turns()[before]["text"].as_str().unwrap().contains(crate::agents::ACCOUNT_RETRY_TEXT));
    assert_eq!(turn_of(&up.hub, &tid(1)).await, "failed", "not an account failure: left to the person");
    up.ag.shutdown(); let _ = std::fs::remove_dir_all(&home);
}

/// The retire watch (accounts_hub::found) cuts a running turn whose account was signed out or changed under it. A turn shows running from the moment its message is taken,
/// before its spawn has tried the account; one sent right after Work was signed out was cut by the watch in that window ("verified identity changed or disconnected") while
/// its start was taking Personal, and ended interrupted (restart_e2e's fallback test under load, macOS CI). Here the window is held open on purpose: Personal's sign-in check
/// is slow, and the watch runs inside it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_retire_watch_cuts_a_turn_running_on_the_changed_account_but_not_one_still_choosing_its_login() {
    let b = bins("rw"); let home = fresh("rw-home");
    let ps = saved_home(&home, &b, 1, &["Personal"]).await;
    let up = boot(&home, &b).await;
    all(&up.hub, &threads(1), "completed", 60).await;
    for th in threads(1) { up.ag.stop(&th).await.unwrap() } quiet(&up.mgr, &threads(1)).await;
    let cut = |l: &String| l.contains("verified identity changed");
    // a long turn runs on Work, and the identity behind Work moves: that turn is cut
    std::fs::write(b.rec.join("mode.w@x.com"), "slow:30").unwrap();
    let before = b.turns().len(); // counted before the send: the stand-in may record its turn before deliver returns (a full gate timed out here, restart_e2e.rs:308)
    up.ag.deliver_person(&tid(0), "m-rw1", "go on", vec![]).await.unwrap();
    // the stand-in has its turn (it records it before the slow part): the process runs on Work now, not only about to
    let end = Instant::now() + Duration::from_secs(30); while b.turns().len() <= before { assert!(Instant::now() < end, "{:?} {:?}", up.mgr.runs_on(&tid(0)), lines(&up.hub, &tid(0))); tokio::time::sleep(Duration::from_millis(20)).await }
    assert_eq!(b.turns()[before]["who"], "w@x.com"); assert_eq!(up.mgr.runs_on(&tid(0)).as_deref(), Some(ps[0].0.as_str()), "and the hub knows it runs on Work");
    let store = std::fs::read_dir(home.join("providers/accounts-v1").join(&ps[0].0)).unwrap().flatten().map(|e| e.path()).find(|p| p.join("login.json").exists()).expect("Work's store");
    std::fs::write(store.join("login.json"), json!({"email":"someone-else@x.com"}).to_string()).unwrap();
    let s = up.mgr.request(&json!({"op":"status","profile":ps[0].0})).await; assert_eq!(s["error"], "", "{s}");
    accounts_hub::found(&up.hub, &Some(up.ag.clone()), &up.mgr.snapshot()).await;
    assert!(lines(&up.hub, &tid(0)).iter().any(cut), "the turn on the changed account is cut: {:?}", lines(&up.hub, &tid(0)));
    assert_eq!(turn_of(&up.hub, &tid(0)).await, "interrupted");
    quiet(&up.mgr, &threads(1)).await; let _ = std::fs::remove_file(b.rec.join("mode.w@x.com"));
    let cuts = lines(&up.hub, &tid(0)).iter().filter(|l| cut(l)).count();
    // the next start: Work is not the account the thread chose any more, so it goes to Personal, where a routine check is running: the start waits for it (launch_plan
    // waits out a check), running and on no login, and the watch runs then
    assert!(up.mgr.hold_check(&ps[1].0).await, "Personal comes free for the check");
    let before = b.turns().len();
    // deliver returns once the start has its login, so it is sent from a task: the start is held on Personal's check while the watch runs
    let sent = { let ag = up.ag.clone(); tokio::spawn(async move { ag.deliver_person(&tid(0), "m-rw2", "and on", vec![]).await }) };
    let end = Instant::now() + Duration::from_secs(30); while turn_of(&up.hub, &tid(0)).await != "running" { assert!(Instant::now() < end, "{:?}", lines(&up.hub, &tid(0))); tokio::time::sleep(Duration::from_millis(20)).await }
    assert!(!sent.is_finished(), "the start waits on Personal's check");
    assert_eq!(up.mgr.runs_on(&tid(0)), None, "still choosing its login: {:?}", lines(&up.hub, &tid(0)));
    accounts_hub::found(&up.hub, &Some(up.ag.clone()), &up.mgr.snapshot()).await; // the watch, in the window
    assert_eq!(lines(&up.hub, &tid(0)).iter().filter(|l| cut(l)).count(), cuts, "a turn on no login yet is not cut: {:?}", lines(&up.hub, &tid(0)));
    assert_eq!(turn_of(&up.hub, &tid(0)).await, "running");
    assert!(!sent.is_finished(), "and is still waiting: the watch ran inside the window");
    up.mgr.release_check(&ps[1].0); sent.await.unwrap().unwrap();
    all(&up.hub, &[tid(0)], "completed", 60).await;
    assert_eq!(b.turns()[before..].iter().map(|t| t["who"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>(), vec!["p0@x.com".to_string()], "it went on under Personal: {:#?}", b.turns());
    assert_eq!(lines(&up.hub, &tid(0)).iter().filter(|l| l.starts_with("Started on")).count(), 1, "said once: {:?}", lines(&up.hub, &tid(0)));
    up.ag.shutdown(); let _ = std::fs::remove_dir_all(&home);
}
