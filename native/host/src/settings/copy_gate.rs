//! Test-only: the settings actors on a hub that runs on a copy of a real log (`BACKPLANE_COPY=1`, and so in side-by-side too) act on nothing outside its home.
//! `copy::on()` is read once per process, so each copy case runs in a CHILD test process (the same pattern as proc.rs and agents/core_e2e.rs). Each case plants fake
//! `claude`, `codex`, `curl`, `secret-tool` and `security` on PATH that record any call, so "nothing real ran" is observed, not assumed; where it can, the same probe
//! runs off a copy to show the fakes are reachable (the check can fail).
#[cfg(test)]
use crate::{hub::{Cfg, Hub}, presenter::Canned};
use serde_json::{json, Value};
use std::{os::unix::fs::PermissionsExt, path::{Path, PathBuf}, sync::Arc};

const PROBE: &str = "BP_SETTINGS_COPY_PROBE";
fn home(tag: &str) -> PathBuf { let h = std::env::temp_dir().join(format!("bp-scg-{}-{tag}", std::process::id())); let _ = std::fs::remove_dir_all(&h); std::fs::create_dir_all(&h).unwrap(); h }
async fn hub_in(h: &Path) -> Arc<Hub> { Hub::start(Cfg { home: h.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap() }
/// Fake tools on PATH (returns the marker file they append to). Restore PATH with the returned guard value.
fn fakes(h: &Path) -> (PathBuf, std::ffi::OsString) {
    let dir = h.join("fakebin"); std::fs::create_dir_all(&dir).unwrap(); let mark = h.join("called");
    for n in ["claude", "codex", "curl", "secret-tool", "security"] { let f = dir.join(n); std::fs::write(&f, format!("#!/bin/sh\necho {n} >> {}\nexit 1\n", mark.display())).unwrap(); std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    let old = std::env::var_os("PATH").unwrap_or_default(); let mut p = vec![dir]; p.extend(std::env::split_paths(&old)); std::env::set_var("PATH", std::env::join_paths(p).unwrap());
    (mark, old)
}
/// A log with a project, an interrupted claude/opus thread and a ready resume record (reset long past).
async fn resume_log(hub: &Arc<Hub>) {
    hub.commit_many(&[json!({"$":"ProjectCreated","id":"p1","title":"fixture","root":"/tmp","at":1}),
        json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"opus","at":2}),
        json!({"$":"TurnChanged","thread":"t1","state":"interrupted","at":3}),
        json!({"$":"SettingSet","key":"provider.resume.t1","value":json!({"target":"claude/opus","reset":100,"reason":"x"}).to_string(),"at":4})]).await.unwrap();
}
fn child(test: &str, probe: &str, copy: bool) {
    let me = std::env::current_exe().unwrap(); let mut c = std::process::Command::new(me);
    c.args([&format!("settings::copy_gate::tests::{test}"), "--exact", "--nocapture", "--test-threads=1"]).env(PROBE, probe).env_remove("BACKPLANE_AGENTS");
    if copy { c.env("BACKPLANE_COPY", "1"); } else { c.env_remove("BACKPLANE_COPY"); }
    let o = c.output().unwrap(); assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains(&format!("{test} ... ok")), "the child ran the probe: {}", String::from_utf8_lossy(&o.stdout));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::driver;

    // ---- ResumeTicker -------------------------------------------------------------------------------------------
    #[tokio::test(flavor = "multi_thread")] async fn resume_probe() {
        let mode = std::env::var(PROBE).unwrap_or_default(); if mode != "resume-copy" && mode != "resume-real" { return }
        let h = home(&mode); let hub = hub_in(&h).await; resume_log(&hub).await;
        let n = hub.store.len();
        assert_eq!(driver::ready_resumes(&hub, 1_000).await, vec![("provider.resume.t1".to_string(), "t1".to_string())], "the record is ready: the probe has teeth");
        let delivered = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let d = delivered.clone(); let hub2 = hub.clone();
        driver::run_resumes(crate::copy::on(), &hub, 1_000, move |th, _msg, ev| { let (d, hub) = (d.clone(), hub2.clone()); async move { d.lock().unwrap().push(th); hub.commit_many(&ev).await.unwrap(); } }).await;
        if mode == "resume-copy" {
            assert!(crate::copy::on());
            assert!(delivered.lock().unwrap().is_empty(), "nothing delivered on a copy"); assert_eq!(hub.store.len(), n, "the log did not grow");
            assert!(hub.setting("provider.resume.t1").await.is_some_and(|v| !v.is_empty()), "the record is still there");
            // the registered ticker itself, as the hub runs it
            use crate::api::Ticker; assert!(driver::ResumeTicker.tick(&hub, 1_000).await.is_empty()); assert_eq!(hub.store.len(), n);
        } else {
            assert!(!crate::copy::on());
            assert_eq!(*delivered.lock().unwrap(), vec!["t1".to_string()], "off a copy the same log delivers"); assert!(hub.store.len() > n);
            assert!(hub.setting("provider.resume.t1").await.unwrap_or_default().is_empty(), "and clears the record in the same commit");
        }
        let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn a_copy_hub_resumes_nothing_and_the_same_log_off_a_copy_does() { child("resume_probe", "resume-copy", true); child("resume_probe", "resume-real", false); }

    // ---- telemetry, the update check, webhook, the fab estimate: nothing sent ------------------------------------
    #[tokio::test(flavor = "multi_thread")] async fn nothing_sent_probe() {
        if std::env::var(PROBE).as_deref() != Ok("nothing-sent") { return }
        assert!(crate::copy::on());
        let h = home("sent"); let (mark, old) = fakes(&h); let hub = hub_in(&h).await; let n = hub.store.len();
        // the telemetry choice is only a setting; there is no sender in the Rust hub, and the opt-out reads as off
        hub.commit_many(&[json!({"$":"SettingSet","key":"push.webhook.url","value":"http://127.0.0.1:9/x","at":1})]).await.unwrap();
        let r = crate::settings::webhook::deliver(&hub, "t1", "done", "completed", "", "", "ok", "T").await; assert_eq!(r, Err("a copy of a log posts nothing".to_string()));
        // the process gate: a curl made through proc (the one way settings makes one) does not start
        assert!(crate::proc::std_cmd(crate::proc::Kind::Net, "curl").arg("http://127.0.0.1:9/").output().is_err(), "a copy does not start curl");
        assert!(!mark.exists(), "no fake curl was called"); assert_eq!(hub.store.len(), n + 1, "only the setting just written");
        std::env::set_var("PATH", old); let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn a_copy_hub_sends_no_telemetry_update_check_webhook_or_estimate() { child("nothing_sent_probe", "nothing-sent", true); }
    /// The sources: settings never reaches the network or runs a program except through crate::proc (so the copy gate sees it), and the Net users are named.
    #[test] fn settings_reaches_the_network_only_through_the_process_gate() {
        let mut bad = vec![]; let mut net_users = vec![];
        for e in std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/settings")).unwrap().flatten() {
            let p = e.path(); if p.extension().map_or(true, |x| x != "rs") || matches!(p.file_name().and_then(|n| n.to_str()), Some("e2e.rs" | "copy_gate.rs")) { continue } // test-only files; every process made in code goes through crate::proc (proc.rs scans for Command::new)
            let name = p.file_name().unwrap().to_string_lossy().to_string(); let text = std::fs::read_to_string(&p).unwrap();
            let code = text.split("\n#[cfg(test)]").next().unwrap_or(&text);
            for sym in ["TcpStream", "reqwest", "ureq", "hyper::", "UdpSocket"] { if code.contains(sym) { bad.push(format!("{name}: {sym}")) } }
            if code.contains("Kind::Net") { net_users.push(name) }
        }
        assert!(bad.is_empty(), "{bad:?}"); net_users.sort();
        assert_eq!(net_users, ["fab.rs", "skill_install.rs", "webhook.rs"], "the only settings code that makes a Kind::Net process (skill_install.rs: a skill the person confirmed, refused on a copy); there is no telemetry sender and no update check in the Rust hub");
        assert!(!std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/settings/telemetry.rs")).unwrap().contains("proc::"), "telemetry.rs starts no process");
    }

    // ---- deploy.* ---------------------------------------------------------------------------------------------------
    #[tokio::test(flavor = "multi_thread")] async fn deploy_probe() {
        if std::env::var(PROBE).as_deref() != Ok("deploy") { return }
        assert!(crate::copy::on());
        let h = home("deploy"); let hub = hub_in(&h).await; let n = hub.store.len(); let (mut c, _) = hub.connect(false);
        for (i, m) in ["deploy.restart", "deploy.cancel", "deploy.status", "deploy.main", "deploy_restart"].iter().enumerate() {
            let r = hub.request(&mut c, &json!({"id": i + 1, "m": m, "p": {"note": "x"}})).await; let r = &r[0];
            assert!(r["ok"] == false && !r["error"].as_str().unwrap_or("").is_empty(), "{m} is refused: {r}");
        }
        assert_eq!(hub.store.len(), n, "a refused deploy writes nothing"); let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn deploy_requests_are_refused_on_a_copy() { child("deploy_probe", "deploy", true); }

    // ---- one_shot, accounts and probes: no real agent CLI -----------------------------------------------------------
    #[tokio::test(flavor = "multi_thread")] async fn no_real_cli_probe() {
        let mode = std::env::var(PROBE).unwrap_or_default(); if mode != "no-cli" && mode != "no-cli-real" { return }
        let h = home(&mode); let (mark, old) = fakes(&h);
        let cli = crate::agents::proc::Cli::from_env();
        // the resolver one_shot, the accounts manager and the CLI probes all use: a copy without a stand-in folder has no agent to run
        let (c, x) = (cli.resolve("claude"), cli.resolve("codex"));
        let mgr = Arc::new(crate::settings::accounts_io::Accounts::open(&h, Arc::new({ let cli = cli.clone(); move |p: &str| cli.resolve(p) })));
        let s = mgr.request(&json!({"op":"add","provider":"claude","label":"Work"})).await; let id = s["profiles"][0]["id"].as_str().unwrap_or("").to_string();
        let _ = mgr.request(&json!({"op":"status","profile":id})).await; let _ = mgr.request(&json!({"op":"login","profile":id})).await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // the models info lists nothing by probing: it is data, and starts no process
        let _ = crate::settings::models::info();
        if mode == "no-cli" {
            assert!(crate::copy::on()); assert!(c.is_err() && x.is_err(), "no stand-in folder: nothing resolves on a copy");
            assert!(!mark.exists(), "neither the accounts status nor login ran a real claude or codex");
        } else {
            assert!(!crate::copy::on()); assert!(c.is_ok() && x.is_ok(), "off a copy the fakes on PATH resolve: the probe has teeth");
            assert!(mark.exists(), "off a copy the accounts status really ran the CLI");
        }
        std::env::set_var("PATH", old); let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn on_a_copy_one_shot_accounts_and_probes_run_no_real_cli() { child("no_real_cli_probe", "no-cli", true); child("no_real_cli_probe", "no-cli-real", false); }
}
