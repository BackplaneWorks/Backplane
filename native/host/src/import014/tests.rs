//! The 0.14 import on the fixture home (test/fixtures/v014-home, made by test/fixtures/make-v014-home.py), copied to a temp folder first.
use super::*;
use crate::{api::Observer, hub::Cfg, presenter::Canned};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

fn fixture() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/fixtures/v014-home") }
fn tmp(tag: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-import014-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
/// The fixture copied to `<dir>/old`, with the bot's agent history filed under its folder there (the path depends on where the copy is).
fn old_home(dir: &Path) -> PathBuf {
    let old = dir.join("old"); files::copy_tree(&fixture(), &old, &[]).unwrap();
    let proj = old.join("providers/accounts-v1/5e70d94b-6252-4f29-8eed-d3bd8c4b961a/02b33ae0-d452-432d-8838-bc364d86404d/projects");
    let key = crate::bots::relocate::munge(&old.join("bots/b1790292962-8017").to_string_lossy());
    std::fs::create_dir_all(proj.join(&key)).unwrap(); std::fs::write(proj.join(&key).join("s-bot.jsonl"), "{\"bot\":true}\n").unwrap();
    old
}
/// Every file under a folder: path, size, content hash, modified time. Equal before and after means nothing in it was touched.
fn snapshot(d: &Path) -> Vec<(String, u64, String, std::time::SystemTime)> {
    use sha2::{Digest, Sha256};
    let mut v = vec![]; let mut stack = vec![d.to_path_buf()];
    while let Some(p) = stack.pop() { for e in std::fs::read_dir(&p).unwrap().flatten() { let m = e.metadata().unwrap(); if m.is_dir() { stack.push(e.path()) } else {
        v.push((e.path().strip_prefix(d).unwrap().to_string_lossy().into_owned(), m.len(), hex::encode(Sha256::digest(std::fs::read(e.path()).unwrap())), m.modified().unwrap())) } } }
    v.sort(); v
}
/// Sees every batch the ordinary way in: none of the import's lines may reach it.
struct Probe(AtomicUsize);
impl Observer for Probe { fn committed(&self, _h: &Arc<Hub>, events: &[Value], _f: u32) { self.0.fetch_add(events.iter().filter(|e| e["$"] != "SettingSet" && e["$"] != "PushOrdered").count(), SeqCst); } }

struct Rig { hub: Arc<Hub>, imp: Arc<Importer>, old: PathBuf, home: PathBuf, probe: Arc<Probe>, _reg: Arc<crate::net::pairing::Registry> }
async fn rig(tag: &str) -> Rig { let dir = tmp(tag); let old = old_home(&dir); rig_on(&dir, old).await }
async fn rig_on(dir: &Path, old: PathBuf) -> Rig {
    let dir = dir.to_path_buf(); let home = dir.join("new"); std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("token"), "0123456789abcdef0123456789abcdef\n").unwrap();
    let cfg = Cfg { home: home.clone(), version: "0.15.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
    let hub = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let _p = crate::statehost::start(hub.clone());
    let (reg, _routes) = crate::net::start::open(&home, "127.0.0.1", 3799).unwrap();
    crate::settings::accounts_hub::start_cli(&hub, None, crate::agents::proc::Cli::standin(&dir.join("agents")));
    let probe = Arc::new(Probe(AtomicUsize::new(0))); hub.register_observer(probe.clone());
    let imp = start_from(&hub, Some(old.clone()));
    Rig { hub, imp, old, home, probe, _reg: reg }
}
fn log(h: &Hub) -> Vec<String> { h.store.range(0, h.store.len()).unwrap() }
fn has(h: &Hub, pat: &str) -> usize { log(h).iter().filter(|l| l.contains(pat)).count() }

#[tokio::test] async fn a_0_14_home_comes_across_once_and_is_left_as_it_was() {
    let r = rig("full").await; let before = snapshot(&r.old);
    let n0 = r.hub.store.len();
    let scan = r.imp.scan(&r.hub, false).await;
    assert_eq!(scan["state"], "ready", "{scan}");
    let c = &scan["counts"];
    assert_eq!((c["projects"].as_u64(), c["threads"].as_u64(), c["bots"].as_u64(), c["rooms"].as_u64(), c["routines"].as_u64(), c["devices"].as_u64(), c["pushes"].as_u64()), (Some(1), Some(3), Some(1), Some(1), Some(1), Some(1), Some(1)), "{c}");
    assert_eq!((c["runs"].as_u64(), c["deleted"].as_u64(), c["removed"].as_u64()), (Some(1), Some(2), Some(1)), "{c}");
    // counting writes nothing: what the hub's own start may write meanwhile is a setting of its own, never a line of the 0.14 log (CI run 42452044244)
    let meanwhile: Vec<String> = r.hub.store.range(n0, r.hub.store.len()).unwrap();
    assert!(meanwhile.iter().all(|l| l.starts_with("{\"$\":\"SettingSet\"") && !l.contains("ui.theme") && !l.contains(DONE)), "counting wrote {meanwhile:?}");

    let res = r.imp.run(&r.hub, false).await;
    assert_eq!(res["state"], "done", "{res}");
    let h = &r.hub;
    // the person's threads, with their history; not the mail run, the deleted thread or the one in a removed project
    { let m = h.model().await;
      for t in ["t1790175755-10", "t1790175900-20", "t1790292962-8017"] { assert!(m.threads.contains_key(t), "{t}"); assert_ne!(m.threads[t].turn, "running", "{t} imported as running") }
      for t in ["run1792383741-196955", "t1790175910-24", "t1790175930-26"] { assert!(!m.threads.contains_key(t), "{t}") }
      assert!(m.projects.contains_key("p1790175753-8") && !m.projects.contains_key("p1790175760-9")); }
    assert_eq!(has(h, "\"msg\":\"m1790175780-13\""), 1); assert_eq!(has(h, "\\ufffd"), 1, "the lone surrogate is cleaned as the store reads it");
    for gone in ["\"TurnChanged\"", "\"AskOpened\"", "\"TurnQueued\"", "\"WakeSet\"", "\"LockChanged\"", "\"UsageSet\"", "\"TurnUsage\"", "\"MailPosted\"", "\"RunSet\"", "\"PeerSet\"", "\"BotWoke\"", "\"RoutineFired\"", "\"DeployBuilt\"", "\"CtxNoted\"", "\"HookSet\"", "\"torn", "m0123456789abcdef", "liveness.", "told.", "allowance.codex\",\"value\":\"{}"] {
        assert_eq!(has(h, gone), 0, "{gone} came across") }
    assert_eq!(has(h, "\"RoutineSet\""), 1); assert_eq!(has(h, "\"on\":false"), 1, "the routine arrives off"); assert_eq!(has(h, "\"on\":true"), 0);
    assert_eq!(has(h, "\"state\":\"interrupted\""), 1, "the task left running is interrupted");
    assert_eq!(has(h, "\"key\":\"ui.theme\",\"value\":\"dark\""), 1); assert_eq!(has(h, "\"key\":\"agent.provider\",\"value\":\"claude\""), 1); assert_eq!(has(h, "summary.at.t1790175755-10"), 1); assert_eq!(has(h, "summary.at.run"), 0);
    assert_eq!(has(h, "\"UberSet\""), 1); assert_eq!(has(h, &format!("\"token\":\"{}\"", "a".repeat(64))), 1); assert_eq!(has(h, &"b".repeat(64)), 0, "a dropped push token stays");
    assert_eq!(r.probe.0.load(SeqCst), 0, "no observer saw an imported line (no push, no feed, no alert)");
    // files: copied, never the logins without keys
    assert!(r.home.join("attachments/desktop_3787_t1790175755-10/datasheet.txt").is_file() && r.home.join("message-images/00a0.json").is_file());
    assert!(r.home.join("bots/b1790292962-8017/notes.md").is_file());
    let store = r.home.join("providers/accounts-v1/5e70d94b-6252-4f29-8eed-d3bd8c4b961a/02b33ae0-d452-432d-8838-bc364d86404d");
    assert!(store.join("projects/-home-a-Documents-Nordic/47119e0e-5017-4813-8353-70a01f309d22.jsonl").is_file() && !store.join(".credentials.json").exists());
    assert!(store.join("projects").join(crate::bots::relocate::munge(&r.home.join("bots/b1790292962-8017").to_string_lossy())).join("s-bot.jsonl").is_file(), "the bot's agent history follows its folder");
    assert!(!r.home.join("secrets/openai.key").exists() && !r.home.join("mail").exists() && !r.home.join("browser-profile").exists());
    let acc = crate::settings::accounts_hub::live(&r.home).unwrap(); assert_eq!(acc.ids(), vec!["5e70d94b-6252-4f29-8eed-d3bd8c4b961a".to_string()]);
    assert_eq!(res["signin"], json!(["Work"])); assert_eq!(res["counts"]["devices"], 1, "one paired device joined");
    let reg = crate::net::start::registry(&r.home).unwrap(); assert_eq!(reg.device_keys(), vec![("1".repeat(32), "2".repeat(64))], "the live device, not the revoked one");
    let notes: Vec<&str> = res["notes"].as_array().unwrap().iter().filter_map(|n| n["k"].as_str()).collect();
    for k in ["runs", "routines-off", "machines", "keys-stay", "google", "fresh", "sign-in"] { assert!(notes.contains(&k), "{k} in {notes:?}") }
    assert_eq!(h.setting(DONE).await.map(|v| v.contains("\"state\":\"done\"")), Some(true));

    // a second import adds nothing
    let n1 = h.store.len();
    let again = r.imp.run(&r.hub, false).await;
    assert_eq!(again["counts"]["lines"], 0, "{again}"); assert!(again["counts"]["already"].as_u64().unwrap() > 0);
    // only the result (a policy refolding may add a setting of its own at any time: never an imported line)
    let new: Vec<String> = h.store.range(n1, h.store.len()).unwrap();
    assert!(new.iter().all(|l| l.starts_with("{\"$\":\"SettingSet\"") && !l.contains("ui.theme")), "{new:?}"); assert_eq!(new.iter().filter(|l| l.contains(DONE)).count(), 1, "{new:?}");
    assert_eq!(reg.device_keys().len(), 1); assert_eq!(acc.ids().len(), 1);
    assert_eq!(snapshot(&r.old), before, "the 0.14 home is unchanged");
}

#[tokio::test] async fn keys_and_sign_ins_come_only_when_asked() {
    let r = rig("keys").await; let before = snapshot(&r.old);
    let res = r.imp.run(&r.hub, true).await; assert_eq!(res["state"], "done", "{res}");
    let store = r.home.join("providers/accounts-v1/5e70d94b-6252-4f29-8eed-d3bd8c4b961a/02b33ae0-d452-432d-8838-bc364d86404d");
    assert!(store.join(".credentials.json").is_file(), "the login comes with keys");
    assert_eq!(std::fs::read_to_string(r.home.join("secrets/openai.key")).unwrap(), "sk-fixture-not-real\n");
    assert!(r.home.join("secrets/hooks/h1790305792-9226").is_file() && r.home.join("secrets/hooks/h1790305792-9226.token").is_file());
    assert_eq!(has(&r.hub, "\"HookSet\""), 1, "the webhook comes with its secret");
    #[cfg(unix)] { use crate::plat::fs::PermissionsExt; assert_eq!(std::fs::metadata(r.home.join("secrets/openai.key")).unwrap().permissions().mode() & 0o777, 0o600); }
    assert!(res["notes"].as_array().unwrap().iter().all(|n| n["k"] != "keys-stay" && n["k"] != "sign-in"));
    assert_eq!(snapshot(&r.old), before);
}

#[tokio::test] async fn keys_chosen_later_bring_the_login_to_a_profile_imported_without_it() {
    let r = rig("later").await;
    r.imp.run(&r.hub, false).await;
    let store = r.home.join("providers/accounts-v1/5e70d94b-6252-4f29-8eed-d3bd8c4b961a/02b33ae0-d452-432d-8838-bc364d86404d");
    assert!(!store.join(".credentials.json").exists());
    let acc = crate::settings::accounts_hub::live(&r.home).unwrap();
    assert_eq!(acc.snapshot()["profiles"][0]["state"], "expired");
    let res = r.imp.run(&r.hub, true).await;
    assert!(store.join(".credentials.json").is_file(), "the login comes now");
    assert_eq!(res["accounts"], 1, "{res}"); assert_ne!(acc.snapshot()["profiles"][0]["state"], "expired");
    assert_eq!(acc.ids().len(), 1);
}

#[tokio::test] async fn an_import_after_0_14_kept_working_adds_only_what_is_new() {
    let r = rig("more").await;
    r.imp.run(&r.hub, false).await;
    let n = r.hub.store.len();
    // 0.14 went on: a reply in a thread already imported, and a torn line finished
    let mut f = std::fs::OpenOptions::new().append(true).open(r.old.join("events.jsonl")).unwrap();
    std::io::Write::write_all(&mut f, b"\",\"role\":\"user\",\"text\":\"one more\",\"at\":1790175999}\n{\"$\":\"MessagePosted\",\"thread\":\"t1790175755-10\",\"msg\":\"m1790176000-90\",\"role\":\"assistant\",\"text\":\"later\",\"at\":1790176000}\n").unwrap();
    let res = r.imp.run(&r.hub, false).await;
    assert_eq!(res["counts"]["messages"], 2, "{res}");
    let new: Vec<String> = r.hub.store.range(n, r.hub.store.len()).unwrap();
    assert_eq!(new.iter().filter(|l| !l.starts_with("{\"$\":\"SettingSet\"") && !l.starts_with("{\"$\":\"PushOrdered\"")).count(), 2, "the two messages and nothing else: {new:?}");
    assert_eq!(has(&r.hub, "m1790176000-90"), 1);
}

#[tokio::test] async fn the_routes_count_then_import() {
    let r = rig("http").await;
    let req = |t: &str| Req { method: "GET".into(), target: t.into(), headers: vec![], body: vec![], peer: None, trusted: true };
    let got: Value = serde_json::from_slice(&HttpHandler::call(&*r.imp, &r.hub, &req("/import/0.14")).await.body).unwrap();
    assert_eq!(got["state"], "ready"); assert_eq!(got["counts"]["threads"], 3);
    let ran: Value = serde_json::from_slice(&HttpHandler::call(&*r.imp, &r.hub, &req("/import/0.14/run?secrets=1")).await.body).unwrap();
    assert_eq!((ran["state"].as_str(), ran["secrets"].as_bool()), (Some("done"), Some(true)));
    let now: Value = serde_json::from_slice(&HttpHandler::call(&*r.imp, &r.hub, &req("/import/0.14")).await.body).unwrap();
    assert_eq!(now["state"], "done", "after an import the route says so");
    assert!(r.hub.info_now()[INFO].as_str().unwrap().contains("\"state\":\"done\""));
}

#[test] fn the_0_14_home_is_found_in_each_systems_user_folder() {
    let d = tmp("find"); let user = d.join("Users").join("A Person"); let old = user.join(".backplane-bend"); std::fs::create_dir_all(&old).unwrap();
    let elsewhere = d.join("not-temp-home"); let one = |c: PathBuf| vec![c];
    assert_eq!(source_in(&elsewhere, None, one(old.clone())), None, "no events.jsonl: not a 0.14 home");
    std::fs::write(old.join("events.jsonl"), "").unwrap();
    assert_eq!(source_in(&elsewhere, Some(old.clone()), vec![]), Some(old.clone()), "BACKPLANE_IMPORT_FROM");
    assert_eq!(source_in(&old, Some(old.clone()), vec![]), None, "the home itself is not its own source");
    assert_eq!(source_in(&old.join("hub"), Some(old.clone()), vec![]), None, "nor a folder inside it");
    assert_eq!(source_in(&d.join("h"), None, one(old.clone())), None, "a temp home looks only at BACKPLANE_IMPORT_FROM");
    // where 0.14 put it: <user folder>/.backplane-bend everywhere (Linux ~, macOS /Users/x, Windows %USERPROFILE%), and on Windows the Cygwin user folders
    assert_eq!(candidates(false, Some("/home/a".into()), None, Some("a".into()), None), vec![PathBuf::from("/home/a/.backplane-bend")], "Linux and macOS: the user folder only");
    let w = candidates(true, Some(PathBuf::from(r"C:\Users\a")), Some(PathBuf::from(r"C:\Users\a\AppData\Local")), Some("a".into()), Some(PathBuf::from(r"D:\cyg")));
    let s: Vec<String> = w.iter().map(|p| p.to_string_lossy().replace('/', "\\")).collect();
    assert_eq!(s, [r"C:\Users\a\.backplane-bend", r"D:\cyg\home\a\.backplane-bend", r"C:\Users\a\AppData\Local\Backplane\cygwin64\home\a\.backplane-bend", r"C:\cygwin64\home\a\.backplane-bend"]);
    // of two 0.14 homes, the one written last
    let cyg = d.join("cyg/home/a/.backplane-bend"); std::fs::create_dir_all(&cyg).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20)); std::fs::write(cyg.join("events.jsonl"), "{}\n").unwrap();
    let not_temp = PathBuf::from("/nonexistent-bp-home/hub");
    if !not_temp.starts_with(std::env::temp_dir()) { assert_eq!(source_in(&not_temp, None, vec![old.clone(), cyg.clone()]), Some(cyg.clone())); }
}

#[test] fn the_plan_counts_without_holding_lines() {
    let d = tmp("plan"); let old = old_home(&d);
    let p = plan::plan(&old.join("events.jsonl"), &plan::Have::default(), plan::Opts { secrets: false, keep: false }).unwrap();
    assert!(p.lines.is_empty() && p.counts.lines > 0);
    let k = plan::plan(&old.join("events.jsonl"), &plan::Have::default(), plan::Opts { secrets: false, keep: true }).unwrap();
    assert_eq!(k.counts, p.counts); assert_eq!(k.lines.len(), p.counts.lines);
    assert!(k.lines.iter().all(|l| serde_json::from_str::<Value>(l).is_ok()), "every line is whole JSON");
    assert_eq!(plan::whole_size(&old.join("events.jsonl")).unwrap() as usize, std::fs::read_to_string(old.join("events.jsonl")).unwrap().rfind('\n').unwrap() + 1);
}

/// A local check on a copy of a real 0.14 home, made read-only (never committed, never the home itself):
/// BP_IMPORT_REAL=<copy> cargo test real_home -- --ignored --nocapture. It counts, imports into a test hub, imports again and checks the copy is as it was.
#[tokio::test(flavor = "multi_thread")] #[ignore] async fn real_home() {
    let old = PathBuf::from(std::env::var("BP_IMPORT_REAL").expect("BP_IMPORT_REAL"));
    assert!(!old.ends_with(".backplane-bend") && !old.starts_with(user_folder().unwrap().join(".backplane-bend")), "a copy only");
    let before = snapshot(&old);
    let r = rig_on(&tmp("real"), old.clone()).await;
    let t = std::time::Instant::now(); let scan = r.imp.scan(&r.hub, false).await;
    println!("REAL scan {:?}: {}", t.elapsed(), scan);
    let t = std::time::Instant::now(); let res = r.imp.run(&r.hub, false).await;
    println!("REAL run {:?}: {}", t.elapsed(), res);
    let m = r.hub.model().await; println!("REAL new home: {} projects, {} threads, log {} lines", m.projects.len(), m.threads.len(), r.hub.store.len()); drop(m);
    assert_eq!(r.probe.0.load(SeqCst), 0, "no observer saw an imported line");
    let n = r.hub.store.len(); let t = std::time::Instant::now(); let again = r.imp.run(&r.hub, false).await;
    println!("REAL again {:?}: lines {} already {}", t.elapsed(), again["counts"]["lines"], again["counts"]["already"]);
    assert_eq!(again["counts"]["lines"], 0); assert_eq!(r.hub.store.len(), n + 1);
    assert_eq!(snapshot(&old), before, "the copy is unchanged");
    println!("REAL ok");
}

/// A 0.14 home of `p` projects, `t` threads and about `l` lines, from the fixture writer's large mode.
fn big_home(dir: &Path, p: u32, t: u32, l: u32) -> PathBuf {
    let old = dir.join("big"); let gen = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/fixtures/make-v014-home.py");
    let st = std::process::Command::new("python3").arg("-I").arg(&gen).arg(&old).arg("--large").args([p.to_string(), t.to_string(), l.to_string()]).current_dir(dir).status().unwrap();
    assert!(st.success()); old
}

/// The person's requirement: a long import shows itself the whole time. Progress is published at least every 2 s (the ticker's every second), the hub answers a
/// write and a read within a second meanwhile, /healthz says migrating with a fold held (the guard keeps the build unproven), and the summary is right.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_long_import_reports_progress_every_second_and_the_hub_answers_meanwhile() {
    let dir = tmp("long"); let old = big_home(&dir, 10, 500, 30000);
    let r = rig_on(&dir, old).await; r.imp.throttle_ms.store(500, std::sync::atomic::Ordering::Relaxed);
    let (h, imp) = (r.hub.clone(), r.imp.clone());
    let run = tokio::spawn(async move { imp.run(&h, false).await });
    let t0 = std::time::Instant::now(); let (mut last, mut last_at, mut gaps, mut seen_running, mut slow) = (String::new(), t0, vec![], 0, vec![]);
    let (mut health_ok, mut phases, mut frozen) = (0, std::collections::BTreeSet::new(), 0);
    while !run.is_finished() {
        let now = r.hub.info_now()[INFO].as_str().unwrap_or("").to_string();
        if now != last { let v: Value = serde_json::from_str(&now).unwrap_or(Value::Null);
            if v["state"] == "running" { seen_running += 1; gaps.push(last_at.elapsed().as_millis()); phases.insert(v["phase"].as_str().unwrap_or("").to_string()); }
            last = now; last_at = std::time::Instant::now(); }
        if seen_running > 0 && !run.is_finished() {
            let a = std::time::Instant::now(); r.hub.commit(&json!({"$":"SettingSet","key":"ui.probe","value":format!("{}", a.elapsed().as_nanos()),"at":1})).await.unwrap(); let w = a.elapsed();
            let a = std::time::Instant::now(); let n = r.hub.model().await.threads.len(); let rd = a.elapsed();
            if w.as_millis() > 1000 || rd.as_millis() > 1000 { slow.push((w.as_millis(), rd.as_millis(), n)); }
            let (_, hz) = crate::guard::health(); if hz["migrating"]["state"] == "running" && hz["folding"].as_u64().unwrap_or(0) > 0 { health_ok += 1; }
            if !crate::statehost::booted(&r.hub) { frozen += 1; } // the views read the policies' state: it must answer the whole time
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let res = run.await.unwrap();
    let took = t0.elapsed();
    assert!(took.as_secs() >= 4, "the throttle made it long enough to watch: {took:?}");
    assert!(seen_running >= 4, "{seen_running} running updates in {took:?}");
    assert!(gaps.iter().skip(1).all(|g| *g <= 2000), "an update at least every 2 s: {gaps:?}");
    assert!(slow.is_empty(), "a write or a read took over a second mid-import: {slow:?}");
    assert!(health_ok > 0, "/healthz said migrating with a fold held");
    assert_eq!(frozen, 0, "the views' state stopped answering during the import (the clients would freeze)");
    assert!(phases.contains("writing") && phases.contains("reading"), "{phases:?}");
    assert!(crate::import014::migrating().is_none() && crate::guard::health().1.get("migrating").is_none(), "nothing migrating once done");
    let c = &res["counts"]; assert_eq!((res["state"].as_str(), c["projects"].as_u64(), c["threads"].as_u64()), (Some("done"), Some(10 + 1), Some(500 + 3)), "the large part and the small fixture's own: {res}");
    assert_eq!(r.hub.model().await.threads.len(), 500 + 3, "every thread that came is in the model");
}

/// After 0.14's Update (BACKPLANE_IMPORT_AUTO) the import begins on its own and ends with the summary; a start after it has finished does not run it again.
#[tokio::test] async fn after_an_update_the_import_runs_by_itself_once() {
    let r = rig("auto").await; // (rig made it without auto)
    let imp = start_auto(&r.hub, Some(r.old.clone()), true);
    let mut done = false; for _ in 0..200 { if r.hub.setting(DONE).await.is_some() { done = true; break } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
    assert!(done, "the import ran by itself"); let n = r.hub.store.len();
    let _again = start_auto(&r.hub, Some(r.old.clone()), true); tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(r.hub.store.range(n, r.hub.store.len()).unwrap().iter().all(|l| !l.contains(DONE)), "a finished import is not run again at the next start");
    drop(imp);
}
