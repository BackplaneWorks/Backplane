//! The scripts around the guard keep to what keeps the person's hub up (docs/HUB-RESILIENCE.md). Each rule here came from a stop of the live hub:
//! - a test's cleanup ends what it started through its own process group or a pidfile, never a name or a pattern (`pkill -f -- "$T/"` with an empty `$T`
//!   matches every process of the user, the live hub included; test/e2e/lib/own-group.sh);
//! - dev-cut.sh publishes no build without the guard (2026-11-09 10:54: a stale builder copy published hub-and-window builds, and the hub ran unguarded).
use std::{fs, path::{Path, PathBuf}, process::Command};

fn root() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap() }
fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-guard-scripts-{n}-{}", std::process::id())); let _ = fs::remove_dir_all(&d); fs::create_dir_all(&d).unwrap(); d }

fn walk(d: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path(); let n = e.file_name().to_string_lossy().to_string();
        if matches!(n.as_str(), "node_modules" | "target" | "build" | ".git") { continue }
        if p.is_dir() { walk(&p, out) } else if matches!(p.extension().and_then(|x| x.to_str()), Some("sh" | "bash" | "py" | "mjs" | "js" | "ts")) { out.push(p) }
    }
}

/// What a line does, without its comment: a whole-line `#` or `//` comment is nothing.
fn code(line: &str) -> &str { let t = line.trim_start(); if t.starts_with('#') || t.starts_with("//") { "" } else { line } }

fn kills_by_name(line: &str) -> bool {
    let l = code(line);
    let word = |w: &str| l.match_indices(w).any(|(i, _)| {
        let b = l[..i].chars().last().map_or(true, |c| !(c.is_alphanumeric() || c == '_' || c == '-'));
        let a = l[i + w.len()..].chars().next().map_or(true, |c| !(c.is_alphanumeric() || c == '_' || c == '-'));
        b && a });
    word("pkill") || word("killall") || (l.contains("kill") && l.contains("$(pgrep -f"))
}

#[test] fn the_rule_sees_a_kill_by_name_and_not_a_comment_or_a_group_kill() {
    for bad in [r#"trap 'pkill -f -- "$T/"' EXIT"#, "killall backplane-hub", r#"kill $HP $(pgrep -f -- "--home $O/hub/home")"#, "subprocess.run(['pkill','-f',home])"] { assert!(kills_by_name(bad), "{bad}") }
    for ok in ["# never pkill -f", r#"kill -TERM -- "-$g""#, "kill -9 $(pgrep -P $HP) $HP", "own_stop", "unpkill_x"] { assert!(!kills_by_name(ok), "{ok}") }
}

#[test] fn no_script_or_test_kills_by_name() {
    let r = root(); let mut files = vec![];
    for d in ["scripts", "test", "deploy", "tools"] { walk(&r.join(d), &mut files) }
    assert!(files.len() > 50, "the walk found {} files under {}", files.len(), r.display());
    let mut bad = vec![];
    for f in files { let Ok(t) = fs::read_to_string(&f) else { continue }; for (i, l) in t.lines().enumerate() { if kills_by_name(l) { bad.push(format!("{}:{}: {}", f.strip_prefix(&r).unwrap().display(), i + 1, l.trim())) } } }
    assert!(bad.is_empty(), "these end processes by name or pattern; end your own process group or pidfile instead (test/e2e/lib/own-group.sh):\n{}", bad.join("\n"));
}

#[test] fn own_stop_ends_the_group_and_its_children_and_nothing_else() {
    let d = tmp("own"); let lib = root().join("test/e2e/lib/own-group.sh");
    let mut other = Command::new("sleep").arg("30").spawn().unwrap();   // a process of the same user that the run did not start
    let s = format!(r#". "{}"; own sh -c 'sleep 300 & echo $! > "{d}/child"; wait'; sleep 0.5; echo $! > "{d}/group"; own_stop"#, lib.display(), d = d.display());
    assert!(Command::new("bash").arg("-c").arg(&s).status().unwrap().success());
    let alive = |f: &str| { let p = fs::read_to_string(d.join(f)).unwrap(); Path::new(&format!("/proc/{}", p.trim())).exists() };
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!alive("group") && !alive("child"), "own_stop left the group or its child running");
    assert!(other.try_wait().unwrap().is_none(), "own_stop ended a process outside its groups");
    let _ = other.kill(); let _ = other.wait(); let _ = fs::remove_dir_all(&d);
}

#[test] fn dev_cut_publishes_no_build_without_the_guard() {
    let d = tmp("cut"); let (src, dev) = (d.join("src"), d.join("dev")); fs::create_dir_all(&src).unwrap();
    use std::os::unix::fs::PermissionsExt;
    let prog = |n: &str| { let p = src.join(n); fs::write(&p, b"\x7fELF fake").unwrap(); fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap(); };
    prog("backplane-hub"); prog("backplane-gpui"); fs::write(src.join("COMMIT"), "abc123\n").unwrap();
    let cut = || Command::new(root().join("scripts/dev-cut.sh")).arg(&src).env("GPUI_DEV_DIR", &dev).env_remove("DEV_CUT_NO_GUARD").output().unwrap();
    let o = cut();
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("no guard"), "a build without the guard was published: {o:?}");
    assert!(!dev.join("CURRENT").exists(), "CURRENT was written for a build without the guard");
    prog("backplane");
    let o = cut();
    assert!(o.status.success(), "a whole build was refused: {o:?}");
    let cur = fs::read_to_string(dev.join("CURRENT")).unwrap();
    assert!(dev.join("builds").join(cur.trim()).join("backplane").is_file(), "the guard is not in the published build");
    let _ = fs::remove_dir_all(&d);
}

/// The disk janitor (scripts/worktree-gc.sh; Nov 11: 157 GB of ~/.cache/bp-tmp and 137 GB of finished lanes' target/ dirs filled the disk): landed worktrees go, idle
/// build dirs and temp entries go, sccache is trimmed to its cap, and nothing a process holds, nothing unlanded and nothing fresh is touched.
#[test] fn the_janitor_frees_what_is_done_and_keeps_what_is_used() {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp("jan"); let repo = d.join("repo"); let bt = d.join("bptmp"); let scc = d.join("scc");
    let git = |dir: &Path, a: &[&str]| { let o = Command::new("git").arg("-C").arg(dir).args(a).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap(); assert!(o.status.success(), "git {a:?}: {o:?}"); String::from_utf8_lossy(&o.stdout).trim().to_string() };
    fs::create_dir_all(&repo).unwrap(); git(&repo, &["init", "-q", "-b", "main"]); git(&repo, &["config", "core.hooksPath", "/dev/null"]);
    fs::write(repo.join("a"), "a").unwrap(); fs::write(repo.join(".gitignore"), "target/\n").unwrap(); git(&repo, &["add", "a", ".gitignore"]); git(&repo, &["commit", "-qm", "a"]);
    let wt = |n: &str| repo.join(".backplane/worktrees").join(n);
    let add = |n: &str| { git(&repo, &["worktree", "add", "-q", "-b", n, wt(n).to_str().unwrap()]); };
    let commit = |n: &str| { fs::write(wt(n).join(n), n).unwrap(); git(&wt(n), &["add", n]); git(&wt(n), &["commit", "-qm", n]); };
    let target = |n: &str| { let t = wt(n).join("native/host/target/fast/deps"); fs::create_dir_all(&t).unwrap(); fs::write(t.join("lib.rlib"), vec![0u8; 1 << 20]).unwrap(); };
    // landed: a commit of its own that origin/main now has; fresh: made from main, nothing done yet; mine: unlanded work; dirty: landed but with an edit; busy: landed, a shell in it
    for n in ["landed", "fresh", "mine", "dirty", "busy", "recent"] { add(n) }
    for n in ["landed", "dirty", "busy"] { commit(n) }
    git(&repo, &["checkout", "-q", "--detach"]);
    for n in ["landed", "dirty", "busy"] { git(&repo, &["merge", "-q", "--no-edit", "--no-ff", n]); }
    git(&repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    commit("mine"); fs::write(wt("dirty").join("edit"), "x").unwrap();
    for n in ["landed", "fresh", "mine", "dirty", "busy", "recent"] { target(n) }
    // bp-tmp: an old scratch target, an old one a process has open, a fresh one, the sccache server's dir, an old file
    for (n, f) in [("aot-build", "big"), ("held", "log"), ("new", "x"), ("sccache", "s"), ("claude", "c")] { fs::create_dir_all(bt.join(n)).unwrap(); fs::write(bt.join(n).join(f), vec![0u8; 1 << 20]).unwrap(); }
    fs::write(bt.join("old.log"), "x").unwrap();
    fs::create_dir_all(&scc).unwrap(); for i in 0..6 { fs::write(scc.join(format!("o{i}")), vec![1u8; 1 << 20]).unwrap(); }
    // age everything but what must look fresh (mtimes only: GC_CTIME=0, a test cannot age a ctime)
    let age = |p: &Path, when: &str| { let o = Command::new("find").arg(p).args(["-depth", "-exec", "touch", "-h", "-d", when, "{}", "+"]).output().unwrap(); assert!(o.status.success()); };
    age(&repo, "3 days ago"); age(&bt, "3 days ago"); for i in 0..6 { age(&scc.join(format!("o{i}")), &format!("{} days ago", 10 - i)); }
    age(&wt("recent").join("native/host/target"), "2 hours ago"); age(&bt.join("new"), "1 hour ago");
    let mut shell = Command::new("sleep").arg("120").current_dir(wt("busy")).spawn().unwrap();
    let mut open = Command::new("sleep").arg("120").stdin(fs::File::open(bt.join("held/log")).unwrap()).spawn().unwrap();
    let script = root().join("scripts/worktree-gc.sh"); fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).ok();
    let run = |apply: bool| { let mut c = Command::new(&script); if apply { c.arg("--apply"); }
        let o = c.args(["--sccache-size", "3M"]).current_dir(&d).env("GC_REPO", &repo).env("GC_UPSTREAM", "origin/main").env("BP_TMP", &bt).env("GC_TMP_BASES", &bt)
            .env("GC_LOCK", d.join("lock")).env("SCCACHE_DIR", &scc).env("GC_CTIME", "0").env_remove("TMPDIR").env_remove("CLAUDE_CODE_TMPDIR").output().unwrap();
        assert!(o.status.success(), "{o:?}"); String::from_utf8_lossy(&o.stdout).to_string() };
    let dry = run(false); let dry2 = run(false);
    assert!(dry2.contains("worktree") && dry2.contains("/landed"), "a run does not make a worktree look used (git status writes no index):\n{dry2}");
    assert!(dry.contains("would have freed") && wt("landed").exists() && bt.join("aot-build").exists(), "a dry run changes nothing:\n{dry}");
    let out = run(true);
    let _ = shell.kill(); let _ = open.kill(); let _ = shell.wait(); let _ = open.wait();
    let tg = |n: &str| wt(n).join("native/host/target").exists();
    assert!(!wt("landed").exists(), "a landed, clean, idle worktree is removed:\n{out}");
    assert!(!git(&repo, &["branch", "--list", "landed"]).contains("landed"), "with its branch");
    assert!(wt("fresh").exists() && !tg("fresh"), "a fresh branch is no landing: the worktree stays, its idle target goes:\n{out}");
    assert!(wt("mine").exists() && !tg("mine"), "unlanded work stays; its target, idle a day, goes");
    assert!(wt("dirty").join("edit").exists() && !tg("dirty"), "a landed branch with an edit keeps the edit and loses its target");
    assert!(wt("busy").exists() && tg("busy"), "a worktree a process works in is left whole:\n{out}");
    assert!(tg("recent"), "a target built two hours ago stays");
    assert!(!bt.join("aot-build").exists() && !bt.join("old.log").exists(), "idle bp-tmp entries go:\n{out}");
    assert!(bt.join("held/log").exists() && bt.join("new").exists() && bt.join("sccache").exists() && bt.join("claude").exists(), "held, fresh, sccache's and claude's stay:\n{out}");
    let left: Vec<String> = fs::read_dir(&scc).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert!(left.len() <= 3 && left.iter().any(|n| n == "o5") && !left.iter().any(|n| n == "o0"), "sccache trimmed to its cap, oldest first: {left:?}\n{out}");
    assert!(out.contains("janitor: freed") && out.contains("aot-build") && out.contains("landed"), "it says what it freed:\n{out}");
    let _ = fs::remove_dir_all(&d);
}
