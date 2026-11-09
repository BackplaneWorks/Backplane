//! `backplane hub ...`: one command to look after a hub, the same on Windows, Linux and macOS.
mod http; mod pair; mod plan; mod run; mod store; mod sys;
use serde_json::{json, Value};
use std::{fs::{self, File}, path::PathBuf, process::{Command, Stdio}, time::Duration};
use store::{Dir, now};

const USAGE: &str = "usage: backplane                            the desktop app of the build that runs
       backplane --headless [--home DIR] [--port N] [--foreground]   the hub alone, for phones and other computers, in the background
       backplane pair [--machine | --join ADDRESS CODE | --requests | --approve ID | --deny ID]   pair a device or a machine (backplane pair --help)
       backplane hub status                 what the hub is doing, what failed last
       backplane hub restart [--now]          restart it (--now: cut running turns; they carry on after)
       backplane hub safe [--now]             restart with agents, routines and bots paused
       backplane hub normal [--now]           restart out of safe mode
       backplane hub rollback [--now]         mark this build bad, go back to the last good one
       backplane hub use BUILD [--now]        run this build; takes its bad mark off
       backplane hub update BUILD             run this build on probation; one failure goes back
       backplane hub stop                     stop the hub and its guard (only at the machine)
       backplane hub log                      where the logs are, and the last incidents
       backplane hub run --home DIR --port N [--host A] [--builds DIR] [--build ID | --bin DIR] [--log F] [--pidfile F] [--safe] [--default] [-- HUB ARGS]
Which hub: --home DIR, else $BACKPLANE_HOME, else the one started with --default.
Another machine: --url http://HOST:PORT --token-file F (restart, safe, normal, rollback, status).
";

struct Cli { home: Option<PathBuf>, url: String, token_file: Option<PathBuf>, now: bool, json: bool, words: Vec<String> }

fn value(a: &[String], i: &mut usize) -> String { *i += 1; a.get(*i).cloned().unwrap_or_else(|| { eprint!("{USAGE}"); std::process::exit(2) }) }

fn run_opts(a: &[String]) -> run::Opts {
    let mut o = run::Opts { home: PathBuf::new(), port: 0, host: "127.0.0.1".into(), builds: None, build: String::new(), log: None, pidfile: None, mem: String::new(), safe: false, default_home: false, hub_args: vec![] };
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "--home" => o.home = value(a, &mut i).into(), "--port" => o.port = value(a, &mut i).parse().unwrap_or(0), "--host" => o.host = value(a, &mut i),
            "--builds" => o.builds = Some(value(a, &mut i).into()), "--build" => o.build = value(a, &mut i), "--bin" => { o.build = value(a, &mut i); }
            "--log" => o.log = Some(value(a, &mut i).into()), "--pidfile" => o.pidfile = Some(value(a, &mut i).into()), "--mem" => o.mem = value(a, &mut i),
            "--safe" => o.safe = true, "--default" => o.default_home = true,
            "--" => { o.hub_args = a[i + 1..].to_vec(); break }
            _ => { eprint!("{USAGE}"); std::process::exit(2) }
        }
        i += 1;
    }
    if o.home.as_os_str().is_empty() || o.port == 0 { eprint!("{USAGE}"); std::process::exit(2) }
    o
}

fn home_of(c: &Cli) -> Option<PathBuf> {
    c.home.clone().or_else(|| std::env::var_os("BACKPLANE_HOME").map(PathBuf::from)).or_else(|| store::pointer().and_then(|p| fs::read_to_string(p).ok()).map(|s| PathBuf::from(s.trim())))
}
fn guard_up(d: &Dir) -> bool { File::open(d.path("lock")).map_or(false, |f| match f.try_lock() { Ok(()) => { let _ = f.unlock(); false } Err(_) => true }) }

/// `backplane hub restart` when no guard runs: start one again from what the last one left, away from this terminal.
fn start_guard(d: &Dir, safe: bool) -> Result<(), String> {
    let l = d.json("launch.json");
    if l.is_null() { return Err("this hub was not started under a guard: start it once with `backplane hub run`".into()) }
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let s = |k: &str| l[k].as_str().unwrap_or("").to_string();
    let mut args: Vec<String> = vec!["hub".into(), "run".into(), "--home".into(), s("home"), "--port".into(), l["port"].to_string(), "--host".into(), s("host")];
    for (k, f) in [("builds", "--builds"), ("log", "--log"), ("pidfile", "--pidfile")] { if !s(k).is_empty() { args.push(f.into()); args.push(s(k)); } }
    let st = d.json("status.json"); if let Some(b) = st["build"].as_str() { if !b.is_empty() { args.push("--build".into()); args.push(b.into()); } }
    if safe { args.push("--safe".into()) } if l["default"] == true { args.push("--default".into()) }
    let extra: Vec<String> = l["hubArgs"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect();
    if !extra.is_empty() { args.push("--".into()); args.extend(extra); }
    // a scope of its own (not the guard's), with no memory cap: `--mem` is accepted and ignored
    let mut c = if cfg!(target_os = "linux") && Command::new("systemd-run").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().map_or(false, |x| x.success()) {
        let mut c = Command::new("systemd-run"); c.args(["--user", "--scope", "-q", "--"]).arg(&me); c
    } else { Command::new(&me) };
    c.args(&args).env_remove("BACKPLANE_AGENT_OWNER").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    for (k, v) in l["env"].as_object().into_iter().flatten() { if let Some(v) = v.as_str() { c.env(k, v); } }
    sys::detach(&mut c);
    c.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn local_token(home: &std::path::Path) -> String { fs::read_to_string(home.join("token")).unwrap_or_default().trim().to_string() }

fn turns(addr: &str, token: &str) -> Option<u64> {
    let (c, b) = http::call(addr, "GET", "/dev/turns", token, "", Duration::from_secs(3))?;
    if c != 200 { return None }
    serde_json::from_str::<Value>(&b).ok()?["running"].as_u64()
}

fn local_action(c: &Cli, action: &str, build: &str) -> i32 {
    let Some(home) = home_of(c) else { eprintln!("which hub? --home DIR, or BACKPLANE_HOME"); return 2 };
    let d = Dir::of(&home);
    if !guard_up(&d) {
        if action == "stop" { println!("no guard runs for {}", home.display()); return 0 }
        return match start_guard(&d, action == "safe") { Ok(()) => { println!("no guard was running: started one"); wait_up(&d) } Err(e) => { eprintln!("{e}"); 1 } };
    }
    let st = d.json("status.json");
    if !c.now && matches!(action, "restart" | "normal" | "safe" | "rollback" | "use" | "update") {
        if let Some(n) = turns(&format!("127.0.0.1:{}", st["port"]), &local_token(&home)) { if n > 0 { eprintln!("{n} running turn(s) would be cut (they carry on after the restart unless restart.continue is off). Again with --now to go ahead."); return 4 } }
    }
    let id = format!("c{}-{}", now(), std::process::id());
    if let Err(e) = d.ask(action, build, "local", &id) { eprintln!("cannot reach the guard: {e}"); return 1 }
    for _ in 0..100 {
        let lr = d.json("status.json")["lastRequest"].clone();
        if lr["id"] == id.as_str() { println!("{}", lr["text"].as_str().unwrap_or("")); if lr["ok"] != true { return 1 } return if action == "stop" { 0 } else { wait_up(&d) } }
        std::thread::sleep(Duration::from_millis(100));
    }
    eprintln!("the guard did not answer: is it stuck? `backplane hub status`"); 1
}

/// Say how it came out: the guard's own word, once it settles.
fn wait_up(d: &Dir) -> i32 {
    let (t0, mut last) = (now(), String::new());
    std::thread::sleep(Duration::from_millis(1500));
    while now() - t0 < 180 {
        let s = d.json("status.json"); let t = s["text"].as_str().unwrap_or("").to_string();
        if t != last { println!("{t}"); last = t; }
        if matches!(s["state"].as_str(), Some("running" | "safe")) { return 0 }
        if s["state"] == "rescue" { return 1 }
        std::thread::sleep(Duration::from_millis(300));
    }
    eprintln!("still not up after three minutes: see `backplane hub log`"); 1
}

fn remote(c: &Cli, action: &str, build: &str) -> i32 {
    let addr = c.url.trim_start_matches("http://").trim_end_matches('/').to_string();
    if c.url.starts_with("https://") { eprintln!("--url takes the machine's plain tailnet address (http://HOST:PORT)"); return 2 }
    let token = c.token_file.as_ref().and_then(|p| fs::read_to_string(p).ok()).unwrap_or_default().trim().to_string();
    if action == "status" {
        return match http::call(&addr, "GET", "/hub/status", &token, "", Duration::from_secs(8)) { Some((200, b)) => { print_status(&serde_json::from_str(&b).unwrap_or(Value::Null), c.json); 0 } Some((code, b)) => { eprintln!("{code} {b}"); 1 } None => { eprintln!("{addr} does not answer"); 1 } };
    }
    if action == "stop" { eprintln!("stopping the hub is only possible at its machine"); return 1 }
    match http::call(&addr, "POST", "/hub/control", &token, &json!({"action": action, "build": build, "force": c.now}).to_string(), Duration::from_secs(10)) {
        Some((202 | 200, b)) => { println!("{}", serde_json::from_str::<Value>(&b).ok().and_then(|v| v["text"].as_str().map(str::to_string)).unwrap_or(b)); 0 }
        Some((code, b)) => { eprintln!("{code} {b}"); 1 }
        None => { eprintln!("{addr} does not answer"); 1 }
    }
}

fn print_status(v: &Value, as_json: bool) {
    if as_json { println!("{v}"); return }
    let g = &v["guard"];
    println!("{}", g["text"].as_str().unwrap_or("no guard for this hub"));
    if let Some(p) = g["hubPid"].as_u64() { if p != 0 { println!("hub pid {p}, guard pid {}, port {}", g["guardPid"], g["port"]); } }
    if let Some(b) = g["bad"].as_array() { if !b.is_empty() { println!("bad builds: {}", b.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")); } }
    for i in v["incidents"].as_array().into_iter().flatten() { println!("{}: {}: {} ({})", i["at"], i["what"].as_str().unwrap_or(""), i["did"].as_str().unwrap_or(""), i["why"].as_str().unwrap_or("")); }
}

fn hub(a: &[String]) -> i32 {
    if a.first().map(String::as_str) == Some("run") { return run::run(run_opts(&a[1..])) }
    let mut c = Cli { home: None, url: String::new(), token_file: None, now: false, json: false, words: vec![] };
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "--home" => c.home = Some(value(a, &mut i).into()), "--url" => c.url = value(a, &mut i), "--token-file" => c.token_file = Some(value(a, &mut i).into()),
            "--now" => c.now = true, "--json" => c.json = true, "-h" | "--help" => { print!("{USAGE}"); return 0 }
            w if w.starts_with("--") => { eprint!("{USAGE}"); return 2 }
            w => c.words.push(w.to_string()),
        }
        i += 1;
    }
    let (word, build) = (c.words.first().cloned().unwrap_or_else(|| "status".into()), c.words.get(1).cloned().unwrap_or_default());
    let action = match word.as_str() { "normal" => "restart", w => w }.to_string();
    if !c.url.is_empty() { if matches!(action.as_str(), "status" | "restart" | "safe" | "rollback" | "use" | "update" | "stop") { return remote(&c, &action, &build) } eprint!("{USAGE}"); return 2 }
    match action.as_str() {
        "status" | "log" => {
            let Some(home) = home_of(&c) else { eprintln!("which hub? --home DIR, or BACKPLANE_HOME"); return 2 };
            let d = Dir::of(&home);
            let up = guard_up(&d);
            let mut g = d.json("status.json");
            if !up { if let Some(o) = g.as_object_mut() { o.insert("text".into(), json!("No guard is running: `backplane hub restart` starts the hub")); o.insert("hubPid".into(), json!(0)); } }
            print_status(&json!({"guard": g, "incidents": d.incidents(if action == "log" { 20 } else { 3 })}), c.json);
            if action == "log" { let l = d.json("launch.json"); println!("hub log: {}", l["log"].as_str().unwrap_or("(its terminal)")); println!("guard folder: {}", d.0.display()); }
            0
        }
        "restart" | "safe" | "rollback" | "stop" => local_action(&c, &action, ""),
        "use" | "update" if !build.is_empty() => local_action(&c, &action, &build),
        _ => { eprint!("{USAGE}"); 2 }
    }
}

/// `backplane` with no arguments opens the desktop app of the build that runs (`<prefix>/builds/<CURRENT>/backplane-gpui`, deploy/install.sh.in's layout).
/// 0.14's `backplane` opened its app, and the link and menu entry an updated 0.14 install keeps still call this name.
fn open_app() -> i32 {
    let me = match std::env::current_exe() { Ok(p) => p, Err(e) => { eprintln!("{e}"); return 1 } };
    let prefix = me.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let cur = fs::read_to_string(prefix.join("CURRENT")).unwrap_or_default().trim().to_string();
    let app = prefix.join("builds").join(&cur).join(if cfg!(windows) { "backplane-gpui.exe" } else { "backplane-gpui" });
    if cur.is_empty() || !app.is_file() { eprint!("{USAGE}"); return 2 }
    #[cfg(unix)] { use std::os::unix::process::CommandExt; let e = Command::new(&app).exec(); eprintln!("cannot open {}: {e}", app.display()); 1 }
    // Windows: start the app and end at once, so the console this program has (the Start-menu shortcut opens it minimised) closes with it
    #[cfg(not(unix))] { Command::new(&app).spawn().map_or_else(|e| { eprintln!("cannot open {}: {e}", app.display()); 1 }, |_| 0) }
}

/// `backplane --headless`: the hub alone, no window (a server, or a desktop whose phones should reach it with the app closed), as 0.14's
/// `backplane --headless` did. It runs under the guard like the one the desktop app starts, in the background unless `--foreground`, and the
/// command ends once it answers. A hub already running for that home is left as it is; one that ran before starts as it ran (`hub restart`).
fn headless(a: &[String]) -> i32 {
    let (mut home, mut port, mut fg) = (None::<PathBuf>, 0u16, false);
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "--headless" => {}, "--foreground" => fg = true,
            "--home" => home = Some(value(a, &mut i).into()), "--port" => port = value(a, &mut i).parse().unwrap_or(0),
            _ => { eprint!("{USAGE}"); return 2 }
        }
        i += 1;
    }
    let user = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
    let home = home.or_else(|| std::env::var_os("BACKPLANE_HOME").map(PathBuf::from)).or_else(|| store::pointer().and_then(|p| fs::read_to_string(p).ok()).map(|s| PathBuf::from(s.trim())))
        .unwrap_or_else(|| match std::env::consts::OS { "macos" => user.join("Library/Application Support/Backplane/hub"), "windows" => user.join("AppData").join("Roaming").join("Backplane").join("hub"), _ => user.join(".backplane/hub") });
    let d = Dir::of(&home);
    if guard_up(&d) { let st = d.json("status.json"); println!("The hub already runs for {} (port {}).", home.display(), st["port"]); return 0 }
    let started = if !d.json("launch.json").is_null() && port == 0 && !fg { start_guard(&d, false) } else {
        let me = match std::env::current_exe() { Ok(p) => p, Err(e) => { eprintln!("{e}"); return 1 } };
        let prefix = me.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        if !prefix.join("CURRENT").is_file() { eprintln!("no builds next to {}: run the hub with `backplane hub run`", me.display()); return 2 }
        let port = if port != 0 { port } else { match (3800..=3899).find(|p| std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok()) { Some(p) => p, None => { eprintln!("no free port between 3800 and 3899: give --port"); return 1 } } };
        if let Err(e) = fs::create_dir_all(&home) { eprintln!("cannot make {}: {e}", home.display()); return 1 }
        let args: Vec<String> = ["run", "--home", &home.to_string_lossy(), "--port", &port.to_string(), "--host", "127.0.0.1", "--builds", &prefix.to_string_lossy(), "--default", "--", "--foreground"].iter().map(|s| s.to_string()).collect();
        if fg { return hub(&args) }
        let log = match fs::OpenOptions::new().create(true).append(true).open(home.join("hub.log")) { Ok(f) => f, Err(e) => { eprintln!("cannot write {}: {e}", home.join("hub.log").display()); return 1 } };
        let mut c = Command::new(&me); c.arg("hub").args(&args).stdin(Stdio::null()).stdout(log.try_clone().map(Stdio::from).unwrap_or(Stdio::null())).stderr(Stdio::from(log));
        sys::detach(&mut c);
        c.spawn().map(|_| ()).map_err(|e| e.to_string())
    };
    if let Err(e) = started { eprintln!("{e}"); return 1 }
    // the guard writes its status once it has started the hub
    for _ in 0..100 { if guard_up(&d) { break } std::thread::sleep(Duration::from_millis(100)); }
    let code = wait_up(&d);
    if code == 0 { println!("Backplane's hub runs in the background for {} (port {}). Pair a phone: backplane pair. Stop it: backplane hub stop.", home.display(), d.json("status.json")["port"]); }
    code
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let code = match a.first().map(String::as_str) {
        Some("hub") => hub(&a[1..]),
        Some("--headless") => headless(&a),
        Some("pair") => pair::run(&a[1..]),
        Some("--version") => { println!("backplane {}", env!("CARGO_PKG_VERSION")); 0 }
        None => open_app(),
        _ => { eprint!("{USAGE}"); 2 }
    };
    std::process::exit(code)
}
