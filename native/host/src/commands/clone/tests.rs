//! Clone from git: the address forms, git's progress and failures in words, where a clone goes, the design found in it, gh's listings, and
//! whole clones of a local bare repository (the fixture) through `clone.start`: added as a project with its thread, a busy folder refused,
//! a hung server cancelled (its folder gone), a refusal for access said as one.
use super::*;
use crate::hub::{Cfg, Hub};
use serde_json::json;
use std::process::Command;

/// The folder used last is one for the process (`LAST_PARENT`): the tests that read it or clone (which sets it) take turns, so a whole clone's
/// folder is never the "used last" of the test that expects none yet (it failed the land gate when they ran at once).
/// The lock is the module's own (`test_parent`), so tests elsewhere that read the default (commands/actions.rs) take the same turns.
fn parent_turn() -> std::sync::MutexGuard<'static, ()> { test_parent(false) }

#[test] fn every_address_form_people_paste_is_a_repository_and_a_folder_is_not() {
    let r = parse("o/Board").unwrap();
    assert_eq!((r.origin.as_str(), r.name.as_str(), r.github.as_str()), ("github.com/o/Board", "Board", "o/Board"));
    assert_eq!(r.urls, ["https://github.com/o/Board.git", "git@github.com:o/Board.git"], "owner/repo: https (gh's credentials), then ssh");
    for (t, origin, name) in [("https://github.com/o/Board.git", "github.com/o/Board", "Board"), ("https://github.com/o/Board/", "github.com/o/Board", "Board"),
        ("git@github.com:o/Board.git", "github.com/o/Board", "Board"), ("ssh://git@gitlab.example.com/g/sub/kit.git", "gitlab.example.com/g/sub/kit", "kit"),
        ("gitlab.com/g/kit", "gitlab.com/g/kit", "kit"), ("https://alex@github.com/o/r.git", "github.com/o/r", "r")] {
        let r = parse(t).unwrap_or_else(|| panic!("{t}"));
        assert_eq!((r.origin.as_str(), r.name.as_str()), (origin, name), "{t}");
    }
    assert_eq!(parse("https://github.com/o/Board.git").unwrap().urls, ["https://github.com/o/Board.git"], "a URL is cloned as typed");
    assert_eq!(parse("file:///srv/git/kit.git").unwrap().name, "kit");
    assert_eq!(parse("https://alex:ghp_secret@github.com/o/r.git"), None, "a password in the address is never taken");
    assert_eq!(redact("fatal: unable to access 'https://alex:ghp_x@github.com/o/r.git/': 403"), "fatal: unable to access 'https://alex@github.com/o/r.git/': 403");
    for t in ["", "~/projects/board", "/srv/x", "./a", "board", "a b/c", "o/r/x", "-o/r", "../r", "https://github.com", "o/..", "x.com/../etc/r", "file://rel/x"] { assert_eq!(parse(t), None, "{t:?}"); }
    assert!(parse_address("o/Board").is_none(), "/project o/Board is a folder");
    assert!(parse_address("https://github.com/o/Board").is_some() && parse_address("github.com/o/Board").is_some());
}

#[test] fn git_progress_reads_as_phase_percent_and_rate() {
    assert_eq!(progress("Receiving objects:  45% (450/1000), 1.20 MiB | 600.00 KiB/s"), Some(("Receiving objects".into(), Some(45), "1.20 MiB, 600.00 KiB/s".into())));
    assert_eq!(progress("remote: Counting objects:  12% (3/25)"), Some(("Counting objects".into(), Some(12), String::new())));
    assert_eq!(progress("Resolving deltas: 100% (5/5), done."), Some(("Resolving deltas".into(), Some(100), String::new())));
    assert_eq!(progress("Cloning into 'kit'..."), Some(("Connecting".into(), None, String::new())));
    assert_eq!(progress("remote: Enumerating objects: 5, done."), Some(("Enumerating objects".into(), None, String::new())));
    for l in ["fatal: repository 'x' not found", "error: RPC failed", "warning: redirecting to https://x/", "hint: x"] { assert_eq!(progress(l), None, "{l}"); }
}

#[test] fn failures_are_said_and_access_refusals_offer_a_sign_in() {
    let (w, auth) = explain("remote: Repository not found.\nfatal: repository 'https://github.com/o/secret.git/' not found", "o/secret", "github.com");
    assert!(auth && w.contains("sign in to GitHub"), "{w}");
    assert!(explain("fatal: could not read Username for 'https://github.com': terminal prompts disabled", "o/r", "github.com").1);
    assert!(explain("git@github.com: Permission denied (publickey).", "o/r", "github.com").1);
    let (w, auth) = explain("fatal: unable to access 'https://x.example/o/r/': Could not resolve host: x.example", "o/r", "x.example");
    assert!(!auth && w.contains("Couldn't reach x.example"), "{w}");
    assert_eq!(explain("fatal: something odd", "o/r", "h").0, "The clone failed: something odd");
}

#[test] fn a_clone_goes_to_the_folder_used_last_and_never_into_one_that_holds_something() {
    let _turn = parent_turn(); LAST_PARENT.lock().unwrap().clear();
    let home = crate::plat::tmp::dir("clone-dest"); let h = home.to_str().unwrap();
    let r = parse("o/Board").unwrap();
    assert_eq!(default_parent(h), h, "no Documents: the home");
    std::fs::create_dir_all(home.join("Documents/Board/src")).unwrap();
    assert_eq!(default_parent(h), format!("{h}/Documents"));
    assert_eq!(default_dest(&format!("{h}/Documents"), &r, h), home.join("Documents/Board-2"), "a folder of that name holding something else: the next name");
    assert!(matches!(dest_state(&home.join("Documents/Board"), &r.origin, h), Dest::Busy(w) if w == "~/Documents/Board already exists and isn't empty. Pick another folder or name."));
    std::fs::create_dir_all(home.join("empty")).unwrap();
    assert_eq!(dest_state(&home.join("empty"), &r.origin, h), Dest::Empty);
    assert_eq!(dest_state(&home.join("none"), &r.origin, h), Dest::Free);
    std::fs::create_dir_all(home.join("same/.git")).unwrap(); std::fs::write(home.join("same/.git/config"), "[remote \"origin\"]\n\turl = git@github.com:o/Board.git\n").unwrap();
    assert_eq!(dest_state(&home.join("same"), &r.origin, h), Dest::Same);
    std::fs::create_dir_all(home.join("same/docs")).unwrap(); std::fs::write(home.join("same/docs/x.md"), "x").unwrap();
    assert!(matches!(dest_state(&home.join("same/docs"), &r.origin, h), Dest::Busy(_)), "a folder inside that clone is not the clone");
    assert_eq!(resolve("~/x", h).unwrap(), home.join("x")); assert!(resolve("rel/x", h).is_err() && resolve("/a/../b", h).is_err());
    let s = Start { repo: "o/Board".into(), parent: format!("{h}/empty"), name: "Mine".into(), ..Default::default() };
    assert_eq!(dest_of(&s, &r, h).unwrap(), home.join("empty/Mine"), "a folder name of the person's own");
    assert!(dest_of(&Start { name: "a/b".into(), ..s.clone() }, &r, h).is_err());
}

#[test] fn several_kicad_projects_get_a_backplane_json_and_one_is_only_said() {
    let d = crate::plat::tmp::dir("clone-design");
    let touch = |p: &str| { let f = d.join(p); std::fs::create_dir_all(f.parent().unwrap()).unwrap(); std::fs::write(f, "{}").unwrap(); };
    touch("boards/kit/kit.kicad_pro");
    assert_eq!(design(&d, "kit"), ["Found a KiCad project: boards/kit/kit.kicad_pro."]); assert!(!d.join(".backplane.json").exists());
    touch("boards/aux/aux.kicad_pro");
    let n = design(&d, "kit"); assert!(n[0].starts_with("Found 2 KiCad projects; boards/kit/kit.kicad_pro"), "{n:?}");
    let j: Value = serde_json::from_str(&std::fs::read_to_string(d.join(".backplane.json")).unwrap()).unwrap(); assert_eq!(j["project"], "boards/kit/kit.kicad_pro", "named like the repository");
    assert!(design(&d, "kit").is_empty(), "a .backplane.json there is kept");
    #[cfg(unix)] { // a link the repository put there is never followed out of the clone
        let e = crate::plat::tmp::dir("clone-design-link"); let out = crate::plat::tmp::dir("clone-design-out");
        for p in ["a/a.kicad_pro", "b/b.kicad_pro"] { let f = e.join(p); std::fs::create_dir_all(f.parent().unwrap()).unwrap(); std::fs::write(f, "{}").unwrap(); }
        std::os::unix::fs::symlink(out.join("written"), e.join(".backplane.json")).unwrap();
        assert!(design(&e, "kit").is_empty()); assert!(!out.join("written").exists());
    }
}

#[test] fn gh_accounts_suggestions_and_device_codes() {
    let st = "github.com\n  ✓ Logged in to github.com account sam (keyring)\n  - Active account: false\n\n  ✓ Logged in to github.com account alex (keyring)\n  - Active account: true\n";
    assert_eq!(gh::accounts_of(st), ["alex", "sam"], "the active one first");
    let row = |n: &str, d: &str, p: &str| gh::Row { name: n.into(), description: d.into(), private: false, pushed: p.into(), account: String::new() };
    let rows = vec![row("org/main-board", "", "2026-11-03"), row("alex/board-lib", "", "2026-11-02"), row("alex/notes", "board notes", "2026-11-01")];
    let names = |q: &str| gh::matching(&rows, q, 10).0.into_iter().map(|r| r.name).collect::<Vec<_>>();
    assert_eq!(names("board"), ["alex/board-lib", "org/main-board", "alex/notes"], "a name that starts with it, then one that holds it, then a description");
    assert_eq!(names(""), ["org/main-board", "alex/board-lib", "alex/notes"], "nothing typed: newest first");
    assert!(names("https://github.com/o/r").is_empty());
    assert_eq!(gh::Row { account: "sam".into(), ..row("sam/x", "", "") }.value(), "https://sam@github.com/sam/x.git", "another account's: that account's token");
    assert_eq!(gh::device_code("! First copy your one-time code: 0D65-1DB7\nOpen this URL to continue in your web browser: https://github.com/login/device\n"), ("0D65-1DB7".into(), "https://github.com/login/device".into()));
    assert_eq!(gh::credential_helper("https://gitlab.com/o/r.git"), None); assert_eq!(gh::credential_helper("git@github.com:o/r.git"), None);
}

// ---- whole clones, of a bare repository on disk ----

fn git(dir: &std::path::Path, args: &[&str]) {
    let o = Command::new("git").current_dir(dir).args(args).env("GIT_AUTHOR_NAME", "Alex").env("GIT_AUTHOR_EMAIL", "person@example.com").env("GIT_COMMITTER_NAME", "Alex").env("GIT_COMMITTER_EMAIL", "person@example.com").output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}
/// A bare repository holding a KiCad project, as `file://`.
fn fixture(tag: &str) -> (crate::plat::tmp::TempDir, String) {
    let d = crate::plat::tmp::dir(&format!("clone-fx-{tag}"));
    let w = d.join("work"); std::fs::create_dir_all(w.join("boards/kit")).unwrap();
    std::fs::write(w.join("boards/kit/kit.kicad_pro"), "{}").unwrap(); std::fs::write(w.join("README.md"), "# kit\n").unwrap();
    git(&w, &["init", "-q", "-b", "main"]); git(&w, &["add", "."]); git(&w, &["commit", "-qm", "first"]);
    git(&d, &["clone", "-q", "--bare", "work", "kit.git"]);
    let url = format!("file://{}", d.join("kit.git").to_string_lossy());
    (d, url)
}
async fn hub(tag: &str) -> crate::hub::TestHub {
    let dir = crate::plat::tmp::dir(&format!("clone-hub-{tag}"));
    std::fs::write(dir.join("events.jsonl"), "").unwrap(); std::fs::write(dir.join("token"), "tok").unwrap();
    let h = Hub::start(Cfg { home: dir.to_path_buf(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
    crate::commands::worktree::SUPPRESS.store(true, Relaxed);
    crate::commands::start(h.clone()); crate::hub::TestHub::new(h, dir)
}
async fn until_over(id: &str) -> Value {
    for _ in 0..3000 { let j = job_json(id).unwrap(); if j["state"] != "running" { return j } tokio::time::sleep(std::time::Duration::from_millis(10)).await }
    panic!("the clone never ended: {:?}", job_json(id))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bare_repository_is_cloned_added_with_its_thread_and_its_folder_then_refused() {
    let _turn = parent_turn();
    let h = hub("whole").await; let (fx, url) = fixture("whole");
    let dest = fx.join("into/kit-copy");
    let j = call(&h, "clone.start", &json!({"repo": url, "dest": dest.to_string_lossy(), "thread": true})).await.unwrap();
    assert_eq!(j["state"], "running");
    let j = until_over(j["id"].as_str().unwrap()).await;
    assert_eq!(j["state"], "done", "{j}");
    assert!(dest.join("boards/kit/kit.kicad_pro").is_file() && dest.join(".git").is_dir(), "the files are there, a parent made for it");
    assert!(j["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("KiCad project: boards/kit/kit.kicad_pro")), "{j}");
    let (pid, th) = (j["project"].as_str().unwrap().to_string(), j["thread"].as_str().unwrap().to_string());
    { let g = h.model().await; let p = g.projects.get(&pid).expect("the project is added"); assert_eq!(p.root.trim_end_matches('/'), crate::plat::fs::canonicalize(dest.to_str().unwrap()).unwrap().to_string_lossy().trim_end_matches('/'));
      assert_eq!(g.threads.get(&th).map(|t| t.project.clone()), Some(pid.clone()), "its first thread"); }
    // the same folder again: refused before anything runs, in words
    let e = call(&h, "clone.start", &json!({"repo": "o/other", "dest": dest.to_string_lossy()})).await.unwrap_err();
    assert!(e.ends_with("already exists and isn't empty. Pick another folder or name."), "{e}");
    // refused with the client's own id: a failed job of that id says why (the client follows it already)
    let e2 = call(&h, "clone.start", &json!({"id": "w1-refused", "repo": "o/other", "dest": dest.to_string_lossy()})).await.unwrap_err();
    let j2 = job_json("w1-refused").unwrap(); assert_eq!((j2["state"].as_str(), j2["error"].as_str()), (Some("failed"), Some(e2.as_str())));
    assert!(!fx.read_dir().unwrap().flatten().any(|e| e.file_name().to_string_lossy().contains(".clone-")), "no staging folder is left");
    let d = call(&h, "clone.dest", &json!({"repo": url, "dest": dest.to_string_lossy()})).await.unwrap(); assert_eq!(d["ok"], false, "{d}");
    // no first thread asked for: the project alone
    let j = call(&h, "clone.start", &json!({"repo": url, "dest": fx.join("bare-add").to_string_lossy(), "thread": false})).await.unwrap();
    let j = until_over(j["id"].as_str().unwrap()).await; assert_eq!((j["state"].as_str(), j["thread"].as_str()), (Some("done"), Some("")), "{j}");
}

/// A server that takes the connection and never answers (a clone that hangs), or answers every request with 401.
async fn server(answer: Option<&'static str>) -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    tokio::spawn(async move { loop { let Ok((mut s, _)) = l.accept().await else { return }; tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt}; let mut b = [0u8; 4096]; let _ = s.read(&mut b).await;
        match answer { Some(a) => { let _ = s.write_all(a.as_bytes()).await; } None => tokio::time::sleep(std::time::Duration::from_secs(120)).await }
    }); } });
    port
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_hung_clone_is_cancelled_and_leaves_no_folder() {
    let _turn = parent_turn();
    let h = hub("cancel").await; let port = server(None).await;
    let fx = crate::plat::tmp::dir("clone-cancel"); let dest = fx.join("hung");
    let j = call(&h, "clone.start", &json!({"repo": format!("http://127.0.0.1:{port}/o/hung.git"), "dest": dest.to_string_lossy()})).await.unwrap();
    let id = j["id"].as_str().unwrap().to_string();
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert_eq!(job_json(&id).unwrap()["state"], "running");
    call(&h, "clone.cancel", &json!({"id": id})).await.unwrap();
    let j = until_over(&id).await; assert_eq!(j["state"], "cancelled", "{j}");
    assert!(!dest.exists(), "the folder it made is gone");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refusal_for_access_is_said_as_one() {
    let _turn = parent_turn();
    let h = hub("auth").await;
    let port = server(Some("HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"x\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")).await;
    let fx = crate::plat::tmp::dir("clone-auth"); let dest = fx.join("secret");
    let j = call(&h, "clone.start", &json!({"repo": format!("http://127.0.0.1:{port}/o/secret.git"), "dest": dest.to_string_lossy()})).await.unwrap();
    let j = until_over(j["id"].as_str().unwrap()).await;
    assert_eq!((j["state"].as_str(), j["auth"].as_bool()), (Some("failed"), Some(true)), "{j}");
    assert!(j["error"].as_str().unwrap().contains("needs you to sign in"), "{j}");
    assert!(!dest.exists());
    // Try again starts it anew, as it was asked
    let again = call(&h, "clone.retry", &json!({"id": j["id"]})).await.unwrap(); assert_ne!(again["id"], j["id"]); assert_eq!(again["dest"], j["dest"]);
    until_over(again["id"].as_str().unwrap()).await;
}
