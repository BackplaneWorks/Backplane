//! Account failover end to end: a real hub on a temp home, the real agents actors, commands and MCP server, and stand-in CLIs (test/agents/fixtures/failover-standin.py) that fake a limit or a
//! failed login mid-turn and record who ran each turn, the session it was asked to resume and the prompt it got. Stand-ins only: no model runs.
use crate::{agents::{proc::Cli, Agents}, api::Caller, hub::{Cfg, Hub}, presenter::Canned, settings::{accounts as ac, accounts_hub, accounts_io::Accounts, driver, fallback as fb}};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc, time::{Duration, Instant}};

const STANDIN: &str = include_str!("../../../../test/agents/fixtures/failover-standin.py");
const T: &str = "t1700000001-2";
/// How long a wait on state may take before it is called stuck. A deadline costs nothing when the state arrives (every wait returns the moment it does) and only matters when it does
/// not, so it is long: a gate on a box with three builds running starts a stand-in CLI in seconds, not milliseconds.
const PATIENCE: u64 = 180;
struct Rig { hub: Arc<Hub>, ag: Arc<Agents>, mgr: Arc<Accounts>, dir: PathBuf }

fn fresh(name: &str) -> PathBuf { static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0); let p = std::env::temp_dir().join(format!("bp-fo-{}-{name}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed))); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
fn now() -> u64 { crate::hub::now_ms() / 1000 }
async fn rig(name: &str, provider: &str) -> Rig {
    use crate::plat::fs::PermissionsExt;
    let (dir, home, work, bin) = (fresh(&format!("{name}-rec")), fresh(&format!("{name}-home")), fresh(&format!("{name}-work")), fresh(&format!("{name}-bin")));
    for cli in ["claude", "codex"] { let p = bin.join(cli); std::fs::write(&p, STANDIN.replace("@DIR@", &crate::plat::fs::script_path(&dir))).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    let log = format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"f\",\"root\":{root},\"at\":1700000000}}\n{{\"$\":\"ThreadCreated\",\"id\":\"{T}\",\"project\":\"p1\",\"title\":\"Fixture\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"{provider}\",\"model\":\"\",\"at\":1700000001}}\n", root = serde_json::to_string(&work.to_string_lossy()).unwrap());
    std::fs::write(home.join("events.jsonl"), log).unwrap();
    std::fs::write(home.join("token"), "tok").unwrap(); // a phone's socket (data/1) presents it
    let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_with(hub.clone(), Cli::standin(&bin)).await.unwrap();
    crate::commands::start_with(hub.clone(), Some(Arc::new(ag.clone())));
    let mgr = accounts_hub::start_cli(&hub, Some(ag.clone()), Cli::standin(&bin));
    // one provider's rules unless a test sets the order: the machine's own CLIs (a models probe) must not add a provider to these
    hub.commit(&json!({"$":"SettingSet","key":"fallback.chain","value":provider,"at":now()})).await.unwrap();
    Rig { hub, ag, mgr, dir }
}
impl Rig {
    /// Two accounts of the provider with allowance left, Work the fuller, the thread on Work.
    async fn two(&self, provider: &str) -> ((String, u64), (String, u64)) {
        let al = |used: f64| json!({"provider":provider,"state":"ok","phase":"complete","at":now(),"windows":[{"scope":"five_hour","used_percent":used}]});
        let w = self.mgr.plant(provider, "Work", "w@x.com", al(50.0)).await; let p = self.mgr.plant(provider, "Personal", "p@x.com", al(10.0)).await;
        self.settled(&[&w.0, &p.0]).await;
        self.bind(T, provider, &w).await; (w, p)
    }
    /// Claude's Work (the thread's) and Personal, and a Codex account, in the order Claude then Codex.
    async fn across(&self) -> ((String, u64), (String, u64), (String, u64)) {
        let al = |p: &str, used: f64| json!({"provider":p,"state":"ok","phase":"complete","at":now(),"windows":[{"scope":"five_hour","used_percent":used}]});
        let w = self.mgr.plant("claude", "Work", "w@x.com", al("claude", 50.0)).await; let p = self.mgr.plant("claude", "Personal", "p@x.com", al("claude", 10.0)).await;
        let c = self.mgr.plant("codex", "Spare", "c@x.com", al("codex", 5.0)).await;
        self.settled(&[&w.0, &p.0, &c.0]).await; self.bind(T, "claude", &w).await;
        self.hub.commit(&json!({"$":"SettingSet","key":"fallback.chain","value":"claude,codex","at":now()})).await.unwrap();
        (w, p, c)
    }
    async fn provider(&self, th: &str) -> String { self.hub.model().await.threads.get(th).map(|t| t.provider.clone()).unwrap_or_default() }
    async fn settled(&self, ids: &[&String]) { let end = Instant::now() + Duration::from_secs(PATIENCE); loop { let inv = self.hub.setting("accounts.inventory").await.unwrap_or_default(); if ids.iter().all(|i| ac::ready(&ac::profile(&inv, i), ac::profile(&inv, i)["provider"].as_str().unwrap_or(""), 2)) { return } assert!(Instant::now() < end, "the inventory never told of them: {inv}"); tokio::time::sleep(Duration::from_millis(50)).await } }
    async fn bind(&self, th: &str, provider: &str, a: &(String, u64)) { self.hub.commit(&json!({"$":"SettingSet","key":ac::binding_key(th, provider),"value":ac::binding_show(&a.0, a.1),"at":now()})).await.unwrap(); }
    fn mode(&self, who: &str, m: &str) { std::fs::write(self.dir.join(format!("mode.{who}")), m).unwrap() }
    fn turns(&self) -> Vec<Value> { std::fs::read_to_string(self.dir.join("turns.jsonl")).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect() }
    async fn say(&self, th: &str, msg: &str, text: &str) { self.ag.deliver_person(th, msg, text, vec![]).await.unwrap() }
    fn log(&self) -> Vec<Value> { self.hub.store.range(0, usize::MAX).unwrap().iter().filter_map(|l| serde_json::from_str(l).ok()).collect() }
    fn lines(&self, th: &str) -> Vec<String> { self.log().iter().filter(|e| e["$"] == "ActivityLogged" && e["thread"] == th).filter_map(|e| e["summary"].as_str().map(str::to_string)).collect() }
    async fn turn_of(&self, th: &str) -> String { self.hub.model().await.threads.get(th).map(|t| t.turn.clone()).unwrap_or_default() }
    /// Until the thread's turn is at rest in `state` and `n` turns ran.
    async fn rest(&self, th: &str, state: &str, n: usize) { let end = Instant::now() + Duration::from_secs(PATIENCE); loop { if self.turns().len() >= n && self.turn_of(th).await == state { tokio::time::sleep(Duration::from_millis(150)).await; if self.turn_of(th).await == state { return } } assert!(Instant::now() < end, "turn {}, {} of {n} turns; {:?}\n{:#?}", self.turn_of(th).await, self.turns().len(), self.lines(th), self.turns()); tokio::time::sleep(Duration::from_millis(40)).await } }
    async fn binding(&self, th: &str, provider: &str) -> String { let b: Value = serde_json::from_str(&self.hub.setting(&ac::binding_key(th, provider)).await.unwrap_or_default()).unwrap_or(Value::Null); b["id"].as_str().unwrap_or("").to_string() }
    /// An MCP tool, through the real server, as an agent of `th`.
    async fn tool(&self, th: &str, name: &str, args: Value) -> (bool, Value) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let m = self.ag.mcp.get().unwrap(); let url = m.url(th, ""); let path = format!("/{}", url.splitn(4, '/').nth(3).unwrap());
        let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}).to_string();
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", m.port)).await.unwrap();
        s.write_all(format!("POST {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        let mut out = String::new(); s.read_to_string(&mut out).await.unwrap();
        let r: Value = serde_json::from_str(out.split("\r\n\r\n").nth(1).unwrap_or("null")).unwrap_or(Value::Null);
        (r["result"]["isError"] != true, serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap_or("null")).unwrap_or(Value::Null))
    }
    /// The person's pick through `accounts.select`, as their client does it.
    async fn pick(&self, th: &str, a: &(String, u64)) -> Value {
        let who = Caller { gen: "g".into(), client: 77001 }; accounts_hub::grant_client("g", 77001);
        let provider = { self.hub.model().await.threads.get(th).unwrap().provider.clone() };
        let expected = self.hub.setting(&ac::binding_key(th, &provider)).await.unwrap_or_default();
        self.hub.rpc_for("accounts.select").unwrap().call(&self.hub, &who, &json!(1), "accounts.select", &json!({"thread":th,"profile":a.0,"epoch":a.1,"expected":expected})).await.remove(0)
    }
    async fn zone(&self) -> String { self.hub.setting("tz.machine").await.unwrap_or_default() }
    async fn tick(&self, at: u64) {
        let ag = self.ag.clone();
        driver::run_resumes(false, &self.hub, at, |th, msg, ev| { let ag = ag.clone(); async move { driver::resume_on(&ag, &th, &msg, ev, at).await } }).await;
        driver::sweep_holds(&self.hub, at).await; driver::returns(&self.hub, at).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_limit_mid_turn_moves_the_thread_to_the_next_account_and_the_turn_goes_on_with_its_context() {
    let r = rig("limit", "claude").await; let (w, p) = r.two("claude").await; let reset = now() + 3600;
    r.say(T, "m1", "FIRST-REQUEST remember the pad size").await; r.rest(T, "completed", 1).await;
    r.mode("w@x.com", &format!("limit:{reset}"));
    r.hub.commit(&json!({"$":"TurnQueued","thread":T,"msg":"q1","text":"QUEUED-BY-PERSON","at":now()})).await.unwrap();
    r.say(T, "m2", "SECOND-REQUEST route the board").await; r.rest(T, "completed", 3).await;
    let ts = r.turns();
    assert_eq!((ts[1]["who"].as_str().unwrap(), ts[2]["who"].as_str().unwrap()), ("w@x.com", "p@x.com"), "{ts:#?}");
    assert!(ts[1]["resume"].as_str().unwrap().starts_with("s-w@x.com") || ts[1]["resume"] == "", "the same process");
    assert_eq!(ts[2]["resume"], "", "a session lives in one account's store: the CLI cannot resume it under another login");
    let got = ts[2]["text"].as_str().unwrap();
    for must in ["FIRST-REQUEST remember the pad size", "answered by w@x.com", "SECOND-REQUEST route the board", "PARTIAL-w@x.com", "Continue the interrupted work"] { assert!(got.contains(must), "the rebuilt prompt lacks {must:?}:\n{got}") }
    assert_eq!(r.binding(T, "claude").await, p.0);
    let say = format!("Switched to Personal: Work ran out (resets {})", crate::agents::rules::when(&r.zone().await, reset, now()));
    assert!(r.lines(T).contains(&say), "{:?}", r.lines(T));
    let log = r.log();
    assert!(!log.iter().any(|e| e["$"] == "TurnChanged" && e["state"] == "failed"), "a turn that goes on never reads as failed");
    assert!(!log.iter().any(|e| e["$"] == "ActivityLogged" && e["tone"] == "error"), "{:?}", r.lines(T));
    assert!(!log.iter().any(|e| e["$"] == "QueueTaken"), "what the person queued is not this request's");
    let hold: Value = serde_json::from_str(&r.hub.setting(&fb::hold_key("claude", &w.0, w.1)).await.unwrap()).unwrap(); assert_eq!(hold["reset"], reset);
    let who = Caller { gen: "g".into(), client: 77002 }; accounts_hub::grant_client("g", 77002);
    let snap = r.hub.rpc_for("accounts").unwrap().call(&r.hub, &who, &json!(1), "accounts", &json!({"op":"list","thread":T})).await.remove(0);
    assert_eq!(snap["result"]["holds"][&w.0]["until"], reset, "{snap}"); assert_eq!(snap["result"]["holds"][&w.0]["text"], accounts_hub::limited_text(&r.zone().await, reset, now())); assert!(snap["result"]["holds"][&p.0].is_null());
    let pref: Value = serde_json::from_str(&r.hub.setting(&fb::preference_key(T)).await.unwrap()).unwrap(); assert_eq!(pref["binding"], ac::binding_show(&w.0, w.1), "where it goes back to");
    // the old account's session is kept under its own key, and the account that took over is what the next message runs on, in its own session
    assert!(!r.hub.setting(&ac::session_key(T, "claude", &w.0, w.1)).await.unwrap_or_default().is_empty());
    r.say(T, "m3", "THIRD").await; r.rest(T, "completed", 4).await; let ts = r.turns();
    assert_eq!(ts[3]["who"], "p@x.com"); assert!(!ts[3]["text"].as_str().unwrap().contains("FIRST-REQUEST"), "it has the conversation now: nothing is handed over twice");
}
/// Nov 10, the Calendar bot: on the CLI's own login, Claude Code answered "You've hit your weekly limit · resets Nov 12, 3am" and then cancelled its own command, and the turn
/// stopped there as Interrupted, as if the person had pressed Stop. It ran out: the request goes on under the next account, says so once, and Settings marks the login limited.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_limit_the_cli_ends_by_cancelling_its_command_fails_over_from_the_default_login() {
    let r = rig("cancel", "claude").await; let (w, p) = r.two("claude").await; let reset = now() + 7200;
    r.bind(T, "claude", &(String::new(), 0)).await; // the thread on the CLI's own login, as the Calendar bot was
    r.mode("default", &format!("cancel:{reset}"));
    r.say(T, "m1", "CHECK-THE-CALENDAR for today").await; r.rest(T, "completed", 2).await;
    let ts = r.turns(); assert_eq!((ts[0]["who"].as_str().unwrap(), ts[1]["who"].as_str().unwrap()), ("default", "p@x.com"), "the account with the most allowance left: {ts:#?}");
    assert!(ts[1]["text"].as_str().unwrap().contains("CHECK-THE-CALENDAR for today"), "the request goes on with what it lost:\n{}", ts[1]["text"]);
    assert_eq!(r.binding(T, "claude").await, p.0);
    let say = format!("Switched to Personal: Default account ran out (resets {})", crate::agents::rules::when(&r.zone().await, reset, now()));
    let lines = r.lines(T); assert_eq!(lines.iter().filter(|l| l.starts_with("Switched to")).count(), 1, "one line: {lines:?}"); assert!(lines.contains(&say), "{lines:?}");
    let log = r.log();
    assert!(!log.iter().any(|e| e["$"] == "TurnChanged" && e["thread"] == T && matches!(e["state"].as_str(), Some("interrupted" | "failed")) && e["failover"] != true), "the turn never reads as stopped or failed to whoever waits on it");
    let hold: Value = serde_json::from_str(&r.hub.setting(&fb::hold_key("claude", "", 0)).await.unwrap()).unwrap(); assert_eq!(hold["reset"], reset, "the reset the limit gave");
    let who = Caller { gen: "g".into(), client: 77003 }; accounts_hub::grant_client("g", 77003);
    let snap = r.hub.rpc_for("accounts").unwrap().call(&r.hub, &who, &json!(1), "accounts", &json!({"op":"list"})).await.remove(0);
    assert_eq!(snap["result"]["holds"][""]["text"], accounts_hub::limited_text(&r.zone().await, reset, now()), "Settings: the Default account is limited until its reset: {snap}");
    assert!(snap["result"]["holds"][""]["text"].as_str().unwrap().starts_with("limited until "));
    assert!(snap["result"]["holds"][&w.0].is_null() && snap["result"]["holds"][&p.0].is_null());
    // the reset comes: the hold is over and the thread goes back to the login it left
    std::fs::remove_file(r.dir.join("mode.default")).unwrap(); r.tick(reset + 1).await;
    assert_eq!(r.binding(T, "claude").await, "", "{:?}", r.lines(T));
    let snap = r.hub.rpc_for("accounts").unwrap().call(&r.hub, &who, &json!(2), "accounts", &json!({"op":"list"})).await.remove(0); let _ = snap;
}
/// The same ending with every account out: said once, with the reset, and nothing runs again before it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_cancelled_limit_with_every_account_out_is_said_once_with_the_reset_and_never_loops() {
    let r = rig("cancelall", "claude").await; let (_w, _p) = r.two("claude").await; let (rd, rw, rp) = (now() + 9000, now() + 7200, now() + 3600);
    r.bind(T, "claude", &(String::new(), 0)).await;
    r.mode("default", &format!("cancel:{rd}")); r.mode("w@x.com", &format!("cancel:{rw}")); r.mode("p@x.com", &format!("cancel:{rp}"));
    r.say(T, "m1", "THE-REQUEST").await; r.rest(T, "interrupted", 3).await; tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(r.turns().len(), 3, "each login once: {:#?}", r.turns());
    let waits: Vec<String> = r.lines(T).into_iter().filter(|l| l.starts_with("Every claude account is out")).collect();
    assert_eq!(waits, vec![format!("Every claude account is out (Default account, Work, Personal). Continuing at {}, when the first one resets.", crate::agents::rules::when(&r.zone().await, rp, now()))], "{:?}", r.lines(T));
    r.tick(rp - 5).await; tokio::time::sleep(Duration::from_millis(300)).await; assert_eq!(r.turns().len(), 3, "not before the reset");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn the_thread_goes_back_to_its_account_when_it_resets_and_resumes_its_own_session() {
    let r = rig("back", "claude").await; let (w, p) = r.two("claude").await; let reset = now() + 3600;
    r.say(T, "m1", "ONE").await; r.rest(T, "completed", 1).await; let sid = r.hub.model().await.threads.get(T).unwrap().session.clone(); assert!(!sid.is_empty());
    r.mode("w@x.com", &format!("limit:{reset}")); r.say(T, "m2", "TWO").await; r.rest(T, "completed", 3).await;
    r.tick(reset - 10).await; assert_eq!(r.binding(T, "claude").await, p.0, "not before its reset");
    std::fs::remove_file(r.dir.join("mode.w@x.com")).unwrap();
    // the hold's reset comes: as far as the record goes it is over
    r.tick(reset + 1).await; assert_eq!(r.binding(T, "claude").await, w.0);
    assert!(r.lines(T).iter().any(|l| l == "Back on Work: it has reset"), "{:?}", r.lines(T)); assert_eq!(r.hub.setting(&fb::preference_key(T)).await.unwrap_or_default(), "");
    r.say(T, "m3", "THREE").await; r.rest(T, "completed", 4).await; let ts = r.turns();
    assert_eq!((ts[3]["who"].as_str().unwrap(), ts[3]["resume"].as_str().unwrap()), ("w@x.com", sid.as_str()), "its own session, in its own store: {:?}\n{:?}", r.lines(T), r.log().iter().filter(|e| e["$"] == "SettingSet" && e["key"].as_str().map_or(false, |k| k.starts_with("account."))).map(|e| format!("{}={}", e["key"], e["value"])).collect::<Vec<_>>());
    let got = ts[3]["text"].as_str().unwrap(); assert!(got.contains("answered by p@x.com") && got.contains("THREE"), "what happened meanwhile is handed over:\n{got}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn every_account_out_is_said_plainly_never_loops_and_goes_on_at_the_earliest_reset() {
    let r = rig("allout", "claude").await; let (w, p) = r.two("claude").await; let (rw, rp) = (now() + 7200, now() + 3600);
    r.mode("w@x.com", &format!("limit:{rw}")); r.mode("p@x.com", &format!("limit:{rp}"));
    r.say(T, "m1", "THE-REQUEST").await; r.rest(T, "interrupted", 2).await; tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(r.turns().len(), 2, "each account once: {:#?}", r.turns());
    let say = format!("Every claude account is out (Work, Personal). Continuing at {}, when the first one resets.", crate::agents::rules::when(&r.zone().await, rp, now()));
    assert!(r.lines(T).contains(&say), "{:?}", r.lines(T));
    let rec: Value = serde_json::from_str(&r.hub.setting(&format!("provider.resume.{T}")).await.unwrap()).unwrap(); assert_eq!(rec["reset"], rp);
    r.tick(rp - 5).await; assert_eq!(r.turns().len(), 2, "not before it");
    std::fs::remove_file(r.dir.join("mode.p@x.com")).unwrap();
    r.tick(rp + 1).await; r.rest(T, "completed", 3).await; let ts = r.turns();
    assert_eq!(ts[2]["who"], "p@x.com", "the account that reset first, wherever the thread stood"); assert_eq!(r.binding(T, "claude").await, p.0); let _ = w;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn every_account_out_with_no_reset_known_says_when_it_tries_again_and_a_turn_is_the_test() {
    let r = rig("noreset", "claude").await; let (_w, p) = r.two("claude").await; r.mode("w@x.com", "limit-noreset"); r.mode("p@x.com", "limit-noreset");
    let t0 = now(); r.say(T, "m1", "GO").await; r.rest(T, "interrupted", 2).await; tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(r.turns().len(), 2);
    assert!(r.lines(T).iter().any(|l| l.starts_with("Every claude account is out (Work, Personal). Continuing at ")), "a limit that named no reset is tried again an hour after it was met: {:?}", r.lines(T));
    r.tick(t0 + 600).await; assert_eq!(r.turns().len(), 2, "not before then");
    std::fs::remove_file(r.dir.join("mode.p@x.com")).unwrap(); // Personal has allowance again
    r.tick(t0 + fb::UNKNOWN_HOLD_S + 5).await; r.rest(T, "completed", 4).await;
    let ts = r.turns(); assert_eq!((ts[2]["who"].as_str().unwrap(), ts[3]["who"].as_str().unwrap()), ("w@x.com", "p@x.com"), "both are tried again: Work, first in saved order, is still out and held again; Personal works: {ts:#?}");
    assert_eq!(r.binding(T, "claude").await, p.0);
    assert_eq!(r.hub.setting(&fb::hold_key("claude", &p.0, p.1)).await.unwrap_or_default(), "", "the completed turn ended its hold");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_failed_login_mid_turn_moves_the_thread_and_a_later_check_clears_it() {
    let r = rig("auth", "claude").await; let (w, p) = r.two("claude").await; r.mode("w@x.com", "auth");
    r.say(T, "m1", "AUTH-REQUEST").await; r.rest(T, "completed", 2).await;
    assert_eq!(r.binding(T, "claude").await, p.0); assert!(r.lines(T).iter().any(|l| l == "Switched to Personal: Work could not sign in"), "{:?}", r.lines(T));
    let k = fb::hold_key("claude", &w.0, w.1); let at: u64 = serde_json::from_str::<Value>(&r.hub.setting(&k).await.unwrap()).unwrap()["at"].as_u64().unwrap();
    r.tick(now()).await; assert!(!r.hub.setting(&k).await.unwrap_or_default().is_empty(), "nothing newer has been learned");
    tokio::time::sleep(Duration::from_millis(1100)).await; let _ = r.mgr.request(&json!({"op":"status","profile":w.0})).await;
    let end = Instant::now() + Duration::from_secs(PATIENCE); loop { let inv = r.hub.setting("accounts.inventory").await.unwrap_or_default(); if ac::profile(&inv, &w.0)["checkedAt"].as_u64().unwrap_or(0) > at { break } assert!(Instant::now() < end); tokio::time::sleep(Duration::from_millis(50)).await }
    r.tick(now()).await; assert_eq!(r.hub.setting(&k).await.unwrap_or_default(), "", "its sign-in was checked after it failed");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn an_old_cli_a_process_that_dies_and_a_turn_that_ended_well_are_each_read_right() {
    let r = rig("shapes", "claude").await; let (w, p) = r.two("claude").await; r.mode("w@x.com", "warn");
    r.say(T, "m1", "ONE").await; r.rest(T, "completed", 1).await;
    assert_eq!(r.binding(T, "claude").await, w.0, "a limit that did not end the turn moves nothing"); assert_eq!(r.hub.setting(&format!("account.failover.pending.{T}")).await.unwrap_or_default(), "");
    assert_eq!(r.hub.setting(&fb::hold_key("claude", &w.0, w.1)).await.unwrap_or_default(), "");
    r.mode("w@x.com", "exit-limit"); r.ag.stop(T).await.unwrap(); r.say(T, "m2", "DIES").await; r.rest(T, "completed", 3).await;
    assert_eq!(r.binding(T, "claude").await, p.0, "{:?}", r.lines(T)); assert!(!r.log().iter().any(|e| e["$"] == "TurnChanged" && e["state"] == "failed"), "{:?}", r.lines(T));
    let t2 = "t1700000099-9"; r.hub.commit(&json!({"$":"ThreadCreated","id":t2,"project":"p1","title":"Old","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":now()})).await.unwrap();
    r.bind(t2, "claude", &p).await; r.mode("p@x.com", "limit-old"); r.mode("w@x.com", "ok"); r.hub.commit(&json!({"$":"SettingSet","key":fb::hold_key("claude", &w.0, w.1),"value":"","at":now()})).await.unwrap();
    r.say(t2, "n1", "OLD-CLI").await; r.rest(t2, "completed", 5).await; assert_eq!(r.binding(t2, "claude").await, w.0, "{:?}", r.lines(t2));
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_codex_thread_runs_under_its_account_and_fails_over_the_same_way() {
    let r = rig("codex", "codex").await; let (_, p) = r.two("codex").await; r.mode("w@x.com", &format!("limit:{}", now() + 900));
    r.say(T, "m1", "CODEX-REQUEST").await; r.rest(T, "completed", 2).await; let ts = r.turns();
    assert_eq!((ts[0]["who"].as_str().unwrap(), ts[1]["who"].as_str().unwrap()), ("w@x.com", "p@x.com"), "each under its own CODEX_HOME: {ts:#?}");
    assert_eq!(r.binding(T, "codex").await, p.0); let got = ts[1]["text"].as_str().unwrap(); assert!(got.contains("CODEX-REQUEST"), "{got}");
    assert!(r.lines(T).iter().any(|l| l.starts_with("Switched to Personal: Work ran out (resets ")), "{:?}", r.lines(T)); assert!(!r.log().iter().any(|e| e["$"] == "TurnChanged" && e["state"] == "failed"));
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn the_persons_pick_hands_the_conversation_over_with_the_next_message() {
    let r = rig("pick", "claude").await; let (_, p) = r.two("claude").await;
    r.say(T, "m1", "BEFORE-THE-PICK").await; r.rest(T, "completed", 1).await;
    let reply = r.pick(T, &p).await; assert_eq!(reply["ok"], true, "{reply}");
    r.say(T, "m2", "AFTER-THE-PICK").await; r.rest(T, "completed", 2).await; let t = &r.turns()[1];
    assert_eq!(t["who"], "p@x.com"); let got = t["text"].as_str().unwrap(); assert!(got.contains("BEFORE-THE-PICK") && got.contains("AFTER-THE-PICK"), "{got}");
    let chosen: Value = serde_json::from_str(&r.hub.setting(&ac::chosen_key(T, "claude")).await.unwrap()).unwrap(); assert_eq!((chosen["by"].as_str().unwrap(), chosen["id"].as_str().unwrap()), ("person", p.0.as_str()));
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_delegated_child_that_fails_over_is_not_reported_as_ended() {
    let r = rig("task", "claude").await; let (_, p) = r.two("claude").await; r.mode("p@x.com", "ok"); r.mode("w@x.com", &format!("limit:{}", now() + 900));
    let (ok, v) = r.tool(T, "delegate_task", json!({"prompt":"CHILD-WORK","account":"w@x.com"})).await; assert!(ok, "{v}"); assert_eq!(v["account"], "Work");
    let child = v["childThreadId"].as_str().unwrap().to_string(); r.rest(&child, "completed", 2).await; tokio::time::sleep(Duration::from_millis(400)).await;
    let log = r.log(); let states: Vec<&str> = log.iter().filter(|e| e["$"] == "TaskStateSet").filter_map(|e| e["state"].as_str()).collect();
    assert!(!states.contains(&"interrupted") && !states.contains(&"failed"), "{states:?}"); assert_eq!(states.last(), Some(&"done"), "the task ends once, when the child has finished: {states:?}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn agents_list_accounts_and_launch_on_one_by_email_or_label_and_who_chose_is_recorded() {
    let r = rig("mcp", "claude").await; let (w, p) = r.two("claude").await;
    let (ok, v) = r.tool(T, "account_list", json!({})).await; assert!(ok, "{v}");
    let acc = v["accounts"].as_array().unwrap(); let work = acc.iter().find(|a| a["label"] == "Work").unwrap();
    assert_eq!((work["email"].as_str().unwrap(), work["remainingPercent"].as_f64().unwrap(), work["current"].as_bool().unwrap(), work["held"].as_bool().unwrap()), ("w@x.com", 50.0, true, false), "{work}");
    assert_eq!(acc.iter().find(|a| a["label"] == "Personal").unwrap()["remainingPercent"], 90.0); assert!(acc.iter().any(|a| a["label"] == "Default account" && a["provider"] == "codex")); assert!(!v.to_string().contains("store") && !v.to_string().contains("providers/accounts"), "no path of a store: {v}");
    let (ok, v) = r.tool(T, "thread_launch", json!({"prompt":"LAUNCHED","account":"p@x.com","title":"On personal"})).await; assert!(ok, "{v}"); assert_eq!((v["account"].as_str().unwrap(), v["provider"].as_str().unwrap()), ("Personal", "claude"));
    let th = v["threadId"].as_str().unwrap().to_string(); r.rest(&th, "completed", 1).await; assert_eq!(r.turns()[0]["who"], "p@x.com", "it ran on that login");
    let chosen: Value = serde_json::from_str(&r.hub.setting(&ac::chosen_key(&th, "claude")).await.unwrap()).unwrap(); assert_eq!((chosen["by"].as_str().unwrap(), chosen["who"].as_str().unwrap()), ("agent", "The agent of \"Fixture\""));
    assert!(r.lines(&th).iter().any(|l| l == "The agent of \"Fixture\" started this thread on Personal"), "{:?}", r.lines(&th));
    for (args, why) in [(json!({"prompt":"x","account":"nobody"}), "No account is called nobody"), (json!({"prompt":"x","account":"Work","provider":"codex"}), "No account is called Work")] { let (ok, v) = r.tool(T, "thread_launch", args).await; assert!(!ok && v["error"].as_str().unwrap().contains(why), "{v}") }
    // a new thread with none named freezes the provider's default, as one the person makes does
    r.hub.commit(&json!({"$":"SettingSet","key":"account.default.claude","value":ac::binding_show(&w.0, w.1),"at":now()})).await.unwrap();
    let (_, v) = r.tool(T, "thread_launch", json!({"prompt":"DEFAULTED"})).await; let d = v["threadId"].as_str().unwrap().to_string(); r.rest(&d, "completed", 2).await; assert_eq!(r.turns()[1]["who"], "w@x.com"); assert_eq!(r.hub.setting(&ac::chosen_key(&d, "claude")).await.unwrap_or_default(), "", "nobody chose it");
    // thread_update moves an unpinned thread, and says who
    let (ok, v) = r.tool(&th, "thread_update", json!({"threadId":d,"account":"Work"})).await; assert!(ok, "{v}"); let (ok, v) = r.tool(&th, "thread_update", json!({"threadId":d,"account":"Personal"})).await; assert!(ok, "{v}"); assert_eq!(r.binding(&d, "claude").await, p.0);
    assert!(r.lines(&d).iter().any(|l| l.ends_with("selected Personal; conversation context is handed to this account with the next message.")), "{:?}", r.lines(&d));
    let (ok, v) = r.tool(T, "thread_update", json!({"threadId":d,"account":"Work","title":"x"})).await; assert!(!ok && v["error"].as_str().unwrap().contains("cannot be combined"), "{v}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_bot_cannot_move_a_thread_the_person_pinned_unless_the_person_asked() {
    let r = rig("pin", "claude").await; let (w, p) = r.two("claude").await; let t2 = "t1700000099-9";
    r.hub.commit(&json!({"$":"ThreadCreated","id":t2,"project":"p1","title":"Helper","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":now()})).await.unwrap();
    r.hub.commit(&json!({"$":"BotSet","id":"bub","name":"Bub","thread":t2,"look":1,"persona":"","at":now()})).await.unwrap();
    assert_eq!(r.pick(T, &p).await["ok"], true); assert_eq!(r.pick(T, &w).await["ok"], true); // the person's own pick of Work is the pin, whatever came before
    let (ok, v) = r.tool(t2, "thread_update", json!({"threadId":T,"account":"Personal"})).await; assert!(!ok, "{v}"); assert!(v["error"].as_str().unwrap().contains("The person chose Work for this thread"), "{v}"); assert_eq!(r.binding(T, "claude").await, w.0);
    // an agent saying so does not count: its message into the thread is marked as its own
    assert!(r.tool(t2, "thread_send", json!({"threadId":T,"text":"please use Personal"})).await.0); r.rest(T, "completed", 1).await;
    let (ok, v) = r.tool(t2, "thread_update", json!({"threadId":T,"account":"Personal"})).await; assert!(!ok, "a message from an agent is not the person's: {v}");
    // nor does the client-reachable door: a connection cannot speak as the driver or as a bot
    let who = Caller { gen: "g".into(), client: 77001 }; accounts_hub::grant_client("g", 77001);
    let f = r.hub.rpc_for("accounts.choose").unwrap().call(&r.hub, &who, &json!(1), "accounts.choose", &json!({"thread":T,"profile":p.0,"epoch":p.1,"by":"failover"})).await.remove(0); assert_eq!(f["error"], "unknown method");
    // the person's own words, naming the account, do
    r.say(T, "m9", "move this to Personal please").await; r.rest(T, "completed", 2).await;
    let (ok, v) = r.tool(t2, "thread_update", json!({"threadId":T,"account":"Personal"})).await; assert!(ok, "{v}"); assert_eq!(v["by"], "bot"); assert_eq!(r.binding(T, "claude").await, p.0);
    assert!(r.lines(T).iter().any(|l| l == "Bub selected Personal; conversation context is handed to this account with the next message."), "{:?}", r.lines(T));
    let c: Value = serde_json::from_str(&r.hub.setting(&ac::chosen_key(T, "claude")).await.unwrap()).unwrap(); assert_eq!((c["by"].as_str().unwrap(), c["who"].as_str().unwrap()), ("bot", "Bub"));
    // a failover is no override: the pin stands and is where the thread goes back to
    assert_eq!(r.pick(T, &p).await["ok"], true); r.mode("p@x.com", &format!("limit:{}", now() + 900)); r.say(T, "m10", "GO").await; r.rest(T, "completed", 4).await;
    assert_eq!(r.binding(T, "claude").await, w.0); let c: Value = serde_json::from_str(&r.hub.setting(&ac::chosen_key(T, "claude")).await.unwrap()).unwrap(); assert_eq!((c["by"].as_str().unwrap(), c["id"].as_str().unwrap()), ("person", p.0.as_str()));
    let (ok, _) = r.tool(t2, "thread_update", json!({"threadId":T,"account":"Work"})).await; assert!(!ok, "the bot still may not touch it, though Work is where it stands");
}

/// Nov 7: the only Claude account's OAuth session died. Every turn failed with "Failed to authenticate: OAuth session expired and could not be refreshed" while its status check
/// said "connected", and the person, away with their phone, could not sign it in again. Here, with the stand-in: the thread stops cleanly and says why, the account says it is
/// expired whatever `auth status` says, one alert is recorded, the phone signs it in through the data/1 socket (link, pasted code), and the thread goes on by itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn an_expired_sign_in_stops_the_thread_alerts_once_and_a_sign_in_from_the_phone_brings_it_back() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let r = rig("expiry", "claude").await;
    let al = json!({"provider":"claude","state":"ok","phase":"complete","at":now(),"windows":[{"scope":"five_hour","used_percent":20.0}]});
    let w = r.mgr.plant("claude", "Work", "w@x.com", al).await; r.settled(&[&w.0]).await; r.bind(T, "claude", &w).await;
    r.say(T, "m1", "FIRST-REQUEST").await; r.rest(T, "completed", 1).await;
    let old_home = r.turns()[0]["home"].as_str().unwrap().to_string();
    std::fs::write(std::path::Path::new(&old_home).join("dead"), "").unwrap(); // its OAuth session died; the credentials file still names the account
    r.say(T, "m2", "SECOND-REQUEST route the board").await; r.rest(T, "interrupted", 2).await;
    assert_eq!(r.mgr.state_of(&w.0), "expired");
    let said = r.lines(T); assert!(said.iter().any(|l| l == "Work's sign-in expired and no other claude account can take over. Continuing once you sign in again."), "{said:?}");
    let rec: Value = serde_json::from_str(&r.hub.setting(&format!("provider.resume.{T}")).await.unwrap()).unwrap(); assert_eq!(rec["signin"], w.0.as_str());
    // honest: the status check asks the provider, which refuses; the hold stays, and the inventory says expired
    let _ = r.mgr.request(&json!({"op":"status","profile":w.0})).await; assert_eq!(r.mgr.state_of(&w.0), "expired", "auth status names the account, the provider refuses it");
    r.tick(now()).await; assert!(!r.hub.setting(&fb::hold_key("claude", &w.0, w.1)).await.unwrap_or_default().is_empty(), "nothing newer has been learned");
    let end = Instant::now() + Duration::from_secs(PATIENCE); loop { if ac::profile(&r.hub.setting("accounts.inventory").await.unwrap_or_default(), &w.0)["state"] == "expired" { break } assert!(Instant::now() < end, "the inventory never said it"); tokio::time::sleep(Duration::from_millis(50)).await }
    // a new message on it refuses at the start, in the same words, and waits as well (no turn runs on a dead login)
    let waits = |r: &Rig| r.lines(T).iter().filter(|l| l.starts_with("Work's sign-in expired")).count();
    r.say(T, "m3", "THIRD-REQUEST").await;
    let end = Instant::now() + Duration::from_secs(PATIENCE); while waits(&r) < 2 { assert!(Instant::now() < end, "{:?}", r.lines(T)); tokio::time::sleep(Duration::from_millis(50)).await }
    r.rest(T, "interrupted", 2).await; assert_eq!(r.turns().len(), 2, "no process was started on it");
    // one alert, linked to the account's sign-in
    let alerts = |l: &[Value]| l.iter().filter(|e| e["$"] == "SettingSet" && e["key"] == accounts_hub::alert_key("claude") && e["value"] != "").cloned().collect::<Vec<_>>();
    let end = Instant::now() + Duration::from_secs(PATIENCE); while alerts(&r.log()).is_empty() { assert!(Instant::now() < end, "no alert"); tokio::time::sleep(Duration::from_millis(50)).await }
    let a: Value = serde_json::from_str(alerts(&r.log())[0]["value"].as_str().unwrap()).unwrap();
    assert_eq!((a["text"].as_str().unwrap(), a["link"].as_str().unwrap()), ("Claude sign-in expired: tap to sign in again", format!("account:{}", w.0).as_str()));

    // the phone: a data/1 socket, as the app opens it
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    { let hub = r.hub.clone(); tokio::spawn(async move { loop { let Ok((st, _)) = l.accept().await else { return }; let h = hub.clone(); tokio::spawn(async move { let _ = crate::wire::serve_with(h, st, false).await; }); } }); }
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws?data=1&enc=json&token=tok")).await.unwrap();
    let mut n = 0u64;
    let mut call = |m: &'static str, p: Value| { n += 1; let id = n; let msg = json!({"id":id,"m":m,"p":p}).to_string(); (id, msg) };
    async fn reply(ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, id: u64, msg: String) -> Value {
        ws.send(Message::Text(msg.into())).await.unwrap();
        loop { let m = tokio::time::timeout(Duration::from_secs(PATIENCE), ws.next()).await.expect("an answer").unwrap().unwrap(); if let Message::Text(t) = m { let v: Value = serde_json::from_str(t.as_str()).unwrap(); if v["t"] == "reply" && v["id"] == id { return v } } }
    }
    let hello = loop { let m = ws.next().await.unwrap().unwrap(); if let Message::Text(t) = m { break serde_json::from_str::<Value>(t.as_str()).unwrap() } };
    assert!(hello["features"].as_array().unwrap().contains(&json!("accounts")));
    let (id, m) = call("accounts.page", json!({})); let page = reply(&mut ws, id, m).await; assert_eq!(page["ok"], true, "{page}");
    let row = &page["result"]["providers"][0]["accounts"][0]; assert_eq!((row["label"].as_str().unwrap(), row["line"].as_str().unwrap(), row["tone"].as_str().unwrap()), ("Work", "sign-in expired", "bad"));
    let (id, m) = call("account.act", json!({"profile":w.0,"action":"signin"})); let v = reply(&mut ws, id, m).await; assert_eq!(v["ok"], true, "{v}");
    let mut view = v["result"]["account"].clone();
    let end = Instant::now() + Duration::from_secs(PATIENCE);
    while view["signin"]["paste"] != true { assert!(Instant::now() < end, "{view}"); tokio::time::sleep(Duration::from_millis(100)).await; let (id, m) = call("account.view", json!({"profile":w.0})); view = reply(&mut ws, id, m).await["result"]["account"].clone(); }
    assert!(view["signin"]["url"].as_str().unwrap().starts_with("https://claude.com/cai/oauth/authorize?code=true"), "{view}");
    assert_eq!(view["actions"], json!([{"label":"Cancel sign-in","action":"cancel","danger":false}]));
    let (id, m) = call("account.act", json!({"profile":w.0,"action":"code","code":"WRONG#OTHER-ATTEMPT"})); let bad = reply(&mut ws, id, m).await; assert_eq!(bad["ok"], false, "a code of another attempt: {bad}");
    let (id, m) = call("account.act", json!({"profile":w.0,"action":"code","code":"CODE#STATE9"})); assert_eq!(reply(&mut ws, id, m).await["ok"], true);
    while view["state"] != "connected" { assert!(Instant::now() < end, "{view}"); tokio::time::sleep(Duration::from_millis(100)).await; let (id, m) = call("account.view", json!({"profile":w.0})); view = reply(&mut ws, id, m).await["result"]["account"].clone(); }
    assert_eq!((view["signin"]["text"].as_str().unwrap(), view["line"].as_str().unwrap()), ("Signed in.", "connected"));

    // the thread goes on by itself, on the account's new sign-in, and the alert is over
    r.rest(T, "completed", 3).await;
    let ts = r.turns(); assert_eq!(ts[2]["who"], "w@x.com"); assert_ne!(ts[2]["home"].as_str().unwrap(), old_home, "a new sign-in, a store of its own");
    assert!(ts[2]["text"].as_str().unwrap().contains("signed in again"), "{}", ts[2]["text"]);
    let inv = r.hub.setting("accounts.inventory").await.unwrap(); let e = ac::profile(&inv, &w.0)["epoch"].as_u64().unwrap(); assert!(e > w.1);
    let b: Value = serde_json::from_str(&r.hub.setting(&ac::binding_key(T, "claude")).await.unwrap()).unwrap(); assert_eq!((b["id"].as_str().unwrap(), b["epoch"].as_u64().unwrap()), (w.0.as_str(), e));
    assert!(r.lines(T).iter().any(|l| l == "Work is signed in again; continuing from the last step"), "{:?}", r.lines(T));
    assert_eq!(r.hub.setting(&accounts_hub::alert_key("claude")).await.unwrap_or_default(), "", "the alert is dealt with");
    assert_eq!(alerts(&r.log()).len(), 1, "one alert in all");
    assert_eq!(r.hub.setting(&format!("provider.resume.{T}")).await.unwrap_or_default(), "");
}

/// Nov 7: after a restart every Claude login was held. A named account and the CLI's own login each kept a weekly-limit hold with no reset from the day before, though both had
/// allowance and turns on them had gone on working; account_list said held, heldUntil 0. Here: stale holds on both, readings with allowance left: the tick ends both; and a
/// hold met minutes ago ends when a turn completes on that login.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn stale_holds_end_on_a_reading_with_allowance_or_a_completed_turn_and_never_hold_every_login() {
    let r = rig("stale", "claude").await;
    let al = |used: f64| json!({"provider":"claude","state":"ok","phase":"complete","at":now(),"attempted_at":now(),"source":"claude-cli-usage","windows":[{"scope":"five_hour","used_percent":used},{"scope":"seven_day","used_percent":30.0}]});
    let w = r.mgr.plant("claude", "kellyhlord", "k@x.com", al(20.0)).await; r.settled(&[&w.0]).await; r.bind(T, "claude", &w).await;
    let old = now() - 86_000; // "You've hit your weekly limit · resets Nov 12, 3am (America/New_York)", written with reset 0 the day before
    let stale = json!({"scope":"claude","reason":"You've hit your weekly limit","reset":0,"at":old}).to_string();
    let (kw, kd) = (fb::hold_key("claude", &w.0, w.1), fb::hold_key("claude", "", 0));
    for k in [&kw, &kd] { r.hub.commit(&json!({"$":"SettingSet","key":k,"value":stale,"at":old})).await.unwrap(); }
    r.hub.commit(&json!({"$":"SettingSet","key":"allowance.claude","value":al(28.0).to_string(),"at":now()})).await.unwrap(); // the default login: 72% left
    let (_, list) = r.tool(T, "account_list", json!({})).await;
    let rows = list["accounts"].as_array().cloned().unwrap_or_default(); assert!(!rows.is_empty(), "{list}");
    assert!(rows.iter().all(|a| a["held"] != true), "a hold a day old with no reset is over by itself, whatever the tick did: {list}");
    r.tick(now()).await;
    assert_eq!(r.hub.setting(&kw).await.unwrap_or_default(), "", "the named account's reading shows allowance");
    assert_eq!(r.hub.setting(&kd).await.unwrap_or_default(), "", "so does the CLI's own login's");
    // a hold met just now, with no reading newer than it: the next turn that completes on the login ends it
    let fresh = json!({"scope":"claude","reason":"You've hit your limit","reset":0,"at":now() + 5}).to_string();
    r.hub.commit(&json!({"$":"SettingSet","key":kw,"value":fresh,"at":now()})).await.unwrap();
    r.tick(now()).await; assert!(!r.hub.setting(&kw).await.unwrap_or_default().is_empty(), "nothing newer has been learned");
    r.say(T, "m1", "WORKS-AGAIN").await; r.rest(T, "completed", 1).await;
    let end = Instant::now() + Duration::from_secs(PATIENCE); while !r.hub.setting(&kw).await.unwrap_or_default().is_empty() { assert!(Instant::now() < end, "the completed turn did not end the hold"); tokio::time::sleep(Duration::from_millis(50)).await }
}

/// Nov 10, the order the person set: every account of the provider first, then the next provider, with the conversation handed over and one line per switch; the way back
/// up once the provider above has reset.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_limit_tries_every_account_of_the_provider_then_the_next_provider_and_comes_back_when_one_resets() {
    let r = rig("order", "claude").await; let (w, p, c) = r.across().await; let (rw, rp) = (now() + 7200, now() + 3600);
    r.say(T, "m1", "FIRST-REQUEST remember the pad size").await; r.rest(T, "completed", 1).await;
    r.mode("w@x.com", &format!("limit:{rw}")); r.mode("p@x.com", &format!("limit:{rp}"));
    r.say(T, "m2", "SECOND-REQUEST route the board").await; r.rest(T, "completed", 4).await;
    let ts = r.turns(); let who: Vec<&str> = ts.iter().map(|t| t["who"].as_str().unwrap()).collect();
    assert_eq!(who, ["w@x.com", "w@x.com", "p@x.com", "c@x.com"], "Work, then Personal, then Codex: {ts:#?}");
    assert_eq!((r.provider(T).await, r.binding(T, "codex").await), ("codex".to_string(), c.0.clone()));
    let got = ts[3]["text"].as_str().unwrap();
    for must in ["FIRST-REQUEST remember the pad size", "SECOND-REQUEST route the board"] { assert!(got.contains(must), "Codex gets the conversation: {must:?} missing:\n{got}") }
    let z = r.zone().await;
    let switches: Vec<String> = r.lines(T).into_iter().filter(|l| l.starts_with("Switched to")).collect();
    assert_eq!(switches, vec![format!("Switched to Personal: Work ran out (resets {})", crate::agents::rules::when(&z, rw, now())),
        format!("Switched to Codex (Spare): Claude (Personal) ran out (resets {})", crate::agents::rules::when(&z, rp, now()))], "one line per switch");
    assert!(!r.log().iter().any(|e| e["$"] == "TurnChanged" && e["thread"] == T && e["state"] == "failed"), "{:?}", r.lines(T));
    // not before Claude has reset
    r.tick(rp - 10).await; assert_eq!(r.provider(T).await, "codex");
    // Personal resets: between turns the thread goes back up to Claude, on Personal; the next message runs there with what Codex did handed over
    std::fs::remove_file(r.dir.join("mode.p@x.com")).unwrap(); r.tick(rp + 1).await;
    assert_eq!((r.provider(T).await, r.binding(T, "claude").await), ("claude".to_string(), p.0.clone()), "{:?}", r.lines(T));
    assert!(r.lines(T).iter().any(|l| l == "Back on Claude (Personal): it has reset"), "{:?}", r.lines(T));
    assert_eq!(r.hub.setting(&format!("fallback.intent.{T}")).await.unwrap_or_default(), "", "back where it started");
    r.say(T, "m3", "THIRD").await; r.rest(T, "completed", 5).await; let ts = r.turns();
    assert_eq!(ts[4]["who"], "p@x.com"); assert!(ts[4]["text"].as_str().unwrap().contains("answered by c@x.com"), "what Codex did goes with it:\n{}", ts[4]["text"]);
    let _ = w;
}
/// Every account of every provider out: said once, with the earliest reset of them all, nothing runs before it, and then it goes on by itself on the account that reset.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn with_every_account_of_every_provider_out_it_says_so_once_and_resumes_at_the_earliest_reset() {
    let r = rig("orderall", "claude").await; let (_w, p, _c) = r.across().await; let (rw, rp, rc) = (now() + 7200, now() + 5400, now() + 3600);
    r.mode("w@x.com", &format!("limit:{rw}")); r.mode("p@x.com", &format!("limit:{rp}")); r.mode("c@x.com", &format!("limit:{rc}"));
    r.say(T, "m1", "THE-REQUEST").await; r.rest(T, "interrupted", 3).await; tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(r.turns().len(), 3, "each account once: {:#?}", r.turns());
    let waits: Vec<String> = r.lines(T).into_iter().filter(|l| l.starts_with("Every ")).collect();
    assert_eq!(waits, vec![format!("Every account is out (Codex: Spare; Claude: Work, Personal). Continuing at {}, when the first one resets.", crate::agents::rules::when(&r.zone().await, rc, now()))], "once: {:?}", r.lines(T));
    let rec: Value = serde_json::from_str(&r.hub.setting(&format!("provider.resume.{T}")).await.unwrap()).unwrap(); assert_eq!(rec["reset"], rc);
    r.tick(rc - 5).await; tokio::time::sleep(Duration::from_millis(300)).await; assert_eq!(r.turns().len(), 3, "not before the reset");
    std::fs::remove_file(r.dir.join("mode.c@x.com")).unwrap();
    r.tick(rc + 1).await; r.rest(T, "completed", 4).await;
    let ts = r.turns(); assert_eq!(ts[3]["who"], "c@x.com", "the account that reset first: {ts:#?}");
    assert!(ts[2]["text"].as_str().unwrap().contains("THE-REQUEST"), "Codex was handed the request when it took over:\n{}", ts[2]["text"]);
    assert!(ts[3]["resume"].as_str().unwrap().starts_with("cs-c@x.com") && ts[3]["text"].as_str().unwrap().starts_with("[Backplane] The provider's reported limit reset has arrived."), "and goes on in that session: {:#?}", ts[3]);
    // later Claude's Personal resets too: the thread goes back up
    std::fs::remove_file(r.dir.join("mode.p@x.com")).unwrap(); r.tick(rp + 1).await;
    assert_eq!((r.provider(T).await, r.binding(T, "claude").await), ("claude".to_string(), p.0.clone()), "{:?}", r.lines(T));
}
/// Nov 11 (the person's rule: stay on the account asked for while it can run; otherwise the next account of the same provider; never a failed turn to find out): the
/// account the thread is bound to is known to be out (its hold). The turn starts on the next healthy account at once, says so in one line, and goes back when it resets.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_launch_on_an_account_known_to_be_out_starts_on_the_next_healthy_one_with_no_failed_turn() {
    let r = rig("healthy", "claude").await; let (w, p) = r.two("claude").await; let reset = now() + 3600;
    r.hub.commit(&json!({"$":"SettingSet","key":fb::hold_key("claude", &w.0, w.1),"value":json!({"scope":"claude","reason":"You've hit your weekly limit","reset":reset,"at":now()}).to_string(),"at":now()})).await.unwrap();
    r.mode("w@x.com", &format!("limit:{reset}")); // were it started there, it would fail
    r.say(T, "m1", "LAUNCH-REQUEST").await; r.rest(T, "completed", 1).await; tokio::time::sleep(Duration::from_millis(300)).await;
    let ts = r.turns(); assert_eq!(ts.len(), 1, "no turn spent on Work: {ts:#?}"); assert_eq!(ts[0]["who"], "p@x.com"); assert!(ts[0]["text"].as_str().unwrap().contains("LAUNCH-REQUEST"));
    assert_eq!(r.binding(T, "claude").await, p.0);
    let say = format!("Started on Personal: Work is out until {}", crate::agents::rules::when(&r.zone().await, reset, now()));
    assert!(r.lines(T).contains(&say), "{:?}", r.lines(T));
    assert!(!r.log().iter().any(|e| e["$"] == "TurnChanged" && e["thread"] == T && matches!(e["state"].as_str(), Some("failed" | "interrupted"))), "no failed or stopped turn first");
    // Work resets: between turns the thread goes back to the account it asked for
    std::fs::remove_file(r.dir.join("mode.w@x.com")).unwrap(); r.tick(reset + 1).await;
    assert_eq!(r.binding(T, "claude").await, w.0, "{:?}", r.lines(T));
}
/// The hub remembers an account's health from its own reading: a full weekly window is a hold (reason, since, until) that Settings shows and every start skips, before any
/// turn has run on it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_full_window_in_a_reading_marks_the_account_out_settings_says_so_and_no_turn_starts_there() {
    let r = rig("health", "claude").await; let reset = now() + 7200;
    let w = r.mgr.plant("claude", "Work", "w@x.com", json!({"provider":"claude","state":"ok","phase":"complete","at":now() - 3000,"windows":[{"scope":"seven_day","used_percent":100.0,"resets_at":reset}]})).await;
    let p = r.mgr.plant("claude", "Personal", "p@x.com", json!({"provider":"claude","state":"ok","phase":"complete","at":now(),"windows":[{"scope":"five_hour","used_percent":10.0}]})).await;
    r.settled(&[&w.0, &p.0]).await; r.bind(T, "claude", &w).await;
    r.tick(now()).await;
    let hold: Value = serde_json::from_str(&r.hub.setting(&fb::hold_key("claude", &w.0, w.1)).await.unwrap()).unwrap();
    assert_eq!((hold["reason"].as_str(), hold["reset"].as_u64()), (Some("Usage window seven_day is full"), Some(reset)), "kept as a setting: it outlives a restart");
    let who = Caller { gen: "g".into(), client: 77011 }; accounts_hub::grant_client("g", 77011);
    let snap = r.hub.rpc_for("accounts").unwrap().call(&r.hub, &who, &json!(1), "accounts", &json!({"op":"list"})).await.remove(0);
    assert_eq!(snap["result"]["holds"][&w.0]["text"], accounts_hub::limited_text(&r.zone().await, reset, now()), "Settings: limited until its reset: {snap}"); assert!(snap["result"]["holds"][&p.0].is_null());
    r.say(T, "m1", "GO").await; r.rest(T, "completed", 1).await; let ts = r.turns();
    assert_eq!((ts.len(), ts[0]["who"].as_str().unwrap()), (1, "p@x.com"), "{ts:#?}");
}
/// Nov 11, t1794379217-272 exactly: claude-opus-5-5 on a catalog of aliases, the default account's first turn hits its weekly limit. It goes to the provider's next account
/// with usage left (it went to Codex), keeps claude-opus-5-5 (it came back on "opus"), only leaves Claude once every Claude account is out, never hops back within the
/// request, and climbs back to the exact model once Claude has reset.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn nov11_the_limit_goes_to_the_next_claude_account_keeps_the_model_and_leaves_claude_only_when_all_are_out() {
    let r = rig("nov11", "claude").await; let (w, p, c) = r.across().await; let (rw, rp) = (now() + 7200, now() + 3600);
    for a in [&w, &p] { r.mgr.set_catalog(&a.0, &["default", "opus", "fable", "sonnet", "haiku"]); }
    let t2 = "t1794379217-272";
    r.hub.commit(&json!({"$":"ThreadCreated","id":t2,"project":"p1","title":"KiCad","env":"local","branch":"","worktree":"","provider":"claude","model":"claude-opus-5-5","at":now()})).await.unwrap();
    r.bind(t2, "claude", &w).await;
    r.mode("w@x.com", &format!("limit:{rw}"));
    r.say(t2, "n1", "KICAD-GOAL make it first-party").await; r.rest(t2, "completed", 2).await;
    let ts = r.turns(); assert_eq!((ts[0]["who"].as_str().unwrap(), ts[1]["who"].as_str().unwrap()), ("w@x.com", "p@x.com"), "Personal, not Codex: {ts:#?}");
    let th = |m: &crate::model::Model| m.threads.get(t2).map(|t| (t.provider.clone(), t.model.clone())).unwrap();
    assert_eq!(th(&*r.hub.model().await), ("claude".to_string(), "claude-opus-5-5".to_string()), "the model asked for, on the next account");
    assert!(!r.log().iter().any(|e| e["$"] == "ProviderSet" && e["id"] == t2), "no provider change while a Claude account has usage left");
    // Personal runs out too: now, and only now, Codex; this request never goes back to Claude
    r.mode("p@x.com", &format!("limit:{rp}"));
    r.say(t2, "n2", "SECOND").await; r.rest(t2, "completed", 4).await; tokio::time::sleep(Duration::from_millis(400)).await;
    let ts = r.turns(); assert_eq!((ts[2]["who"].as_str().unwrap(), ts[3]["who"].as_str().unwrap(), ts.len()), ("p@x.com", "c@x.com", 4), "{ts:#?}");
    assert_eq!(th(&*r.hub.model().await).0, "codex"); assert_eq!(r.binding(t2, "codex").await, c.0);
    let tried: Vec<String> = serde_json::from_str(&r.hub.setting(&driver::tried_key(t2)).await.unwrap()).unwrap();
    for must in [fb::tried_id(&p.0, p.1), driver::left_id("claude")] { assert!(tried.contains(&must), "{tried:?}") }
    // Claude's Personal resets: between turns it climbs back, on claude-opus-5-5 exactly
    std::fs::remove_file(r.dir.join("mode.p@x.com")).unwrap(); r.tick(rp + 1).await;
    assert_eq!(th(&*r.hub.model().await), ("claude".to_string(), "claude-opus-5-5".to_string()), "{:?}", r.lines(t2));
    assert_eq!(r.binding(t2, "claude").await, p.0);
}
/// thread_update {provider, model, account} from another thread (a lead moving a lane), between turns: the conversation goes with it as a switch by hand does; a choice
/// made this way stays (the chain does not move it back up by itself), and a thread the chain moved down is brought back by it with the chain's bookkeeping cleared.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_lead_moves_a_thread_to_another_provider_and_back_and_the_conversation_goes_with_it() {
    let r = rig("lead", "claude").await; let (w, p, c) = r.across().await; const LEAD: &str = "t1700000099-9";
    r.hub.commit(&json!({"$":"ThreadCreated","id":LEAD,"project":"p1","title":"Lead","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":now()})).await.unwrap();
    r.say(T, "m1", "FIRST-REQUEST remember the pad size").await; r.rest(T, "completed", 1).await;
    // refused: what is not a provider, a mix with a rename, a thread's own turn
    let (ok, v) = r.tool(LEAD, "thread_update", json!({"threadId":T,"provider":"nope"})).await; assert!(!ok && v["error"].as_str().unwrap().contains("not a provider"), "{v}");
    let (ok, v) = r.tool(LEAD, "thread_update", json!({"threadId":T,"provider":"codex","title":"x"})).await; assert!(!ok && v["error"].as_str().unwrap().contains("cannot be combined"), "{v}");
    // to Codex: the saved status comes back, nothing runs until the next message, which carries the conversation so far
    let (ok, v) = r.tool(LEAD, "thread_update", json!({"threadId":T,"provider":"codex","account":"Spare"})).await; assert!(ok, "{v}");
    assert_eq!((v["threadId"].as_str(), v["provider"].as_str(), v["model"].as_str(), v["account"].as_str()), (Some(T), Some("codex"), Some(""), Some("Spare")), "{v}"); assert_eq!(r.binding(T, "codex").await, c.0);
    assert_eq!(r.turns().len(), 1, "a switch starts no turn"); assert_eq!(r.provider(T).await, "codex");
    r.say(T, "m2", "SECOND-REQUEST route the board").await; r.rest(T, "completed", 2).await;
    let t = &r.turns()[1]; assert_eq!(t["who"], "c@x.com"); assert!(t["text"].as_str().unwrap().contains("FIRST-REQUEST remember the pad size"), "Codex gets the conversation:\n{}", t["text"]);
    // a choice, not a fallback: a reset later does not move it back up
    assert_eq!(r.hub.setting(&format!("fallback.intent.{T}")).await.unwrap_or_default(), ""); r.tick(now() + 7200).await; assert_eq!(r.provider(T).await, "codex", "{:?}", r.lines(T));
    // a model alone keeps the provider
    let (ok, v) = r.tool(LEAD, "thread_update", json!({"threadId":T,"model":"gpt-test"})).await; assert!(ok, "{v}"); assert_eq!((v["provider"].as_str(), v["model"].as_str()), (Some("codex"), Some("gpt-test")), "{v}");
    // back to Claude on a named account, in one call; what Codex did goes along
    let (ok, v) = r.tool(LEAD, "thread_update", json!({"threadId":T,"provider":"claude","account":"Personal"})).await; assert!(ok, "{v}");
    assert_eq!((v["provider"].as_str(), v["model"].as_str(), v["account"].as_str()), (Some("claude"), Some(""), Some("Personal")), "{v}"); assert_eq!(r.binding(T, "claude").await, p.0);
    r.say(T, "m3", "THIRD").await; r.rest(T, "completed", 3).await; let t = &r.turns()[2];
    assert_eq!(t["who"], "p@x.com"); assert!(t["text"].as_str().unwrap().contains("SECOND-REQUEST"), "what Codex did goes with it:\n{}", t["text"]);
    // the chain moves it down at a limit, as before; the lead brings it back up, and the chain's bookkeeping goes with that choice
    let rs = now() + 3600; r.mode("w@x.com", &format!("limit:{rs}")); r.mode("p@x.com", &format!("limit:{rs}"));
    r.say(T, "m4", "FOURTH").await; r.rest(T, "completed", 6).await; assert_eq!(r.provider(T).await, "codex", "{:?}", r.lines(T));
    assert_ne!(r.hub.setting(&format!("fallback.intent.{T}")).await.unwrap_or_default(), "", "the chain remembers where it came from");
    std::fs::remove_file(r.dir.join("mode.w@x.com")).unwrap();
    let (ok, v) = r.tool(LEAD, "thread_update", json!({"threadId":T,"provider":"claude","account":"Work"})).await; assert!(ok, "{v}");
    assert_eq!((r.provider(T).await, r.binding(T, "claude").await), ("claude".to_string(), w.0.clone()));
    assert_eq!(r.hub.setting(&format!("fallback.intent.{T}")).await.unwrap_or_default(), "", "the lead's choice clears the chain's");
    r.say(T, "m5", "FIFTH").await; r.rest(T, "completed", 7).await; let t = &r.turns()[6];
    assert_eq!(t["who"], "w@x.com"); assert!(t["text"].as_str().unwrap().contains("answered by c@x.com"), "what Codex did after the fallback goes back with it:\n{}", t["text"]);
}
