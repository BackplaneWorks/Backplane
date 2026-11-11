//! The first-run setup: one list of steps for every client, from what is installed to a tour of the features. Pure: `view` turns the state, the
//! session and the hub's info into what a client draws, `act` turns a press into a session patch and hub requests. Nothing here runs a program;
//! `settings/onboard.rs` runs the checks and keeps them in the setting `onboard.checks`, so a result redraws every client and outlives a restart.
//!
//! What lasts is in settings, so a step skipped on one client is skipped on all: `onboard.step` (the step reached), `onboard.skipped` and
//! `onboard.seen` (comma lists of step ids), `onboard.done` ("1": finished or ended). A view keeps `@ob.step` (the step on screen), `@ob.closed`
//! ("1": Finish later, until the next launch) and `@ob.open` ("1": asked for from Settings) in its drafts.
//!
//! Every word a person reads is in this file, and the test `every_word_follows_unslop` checks them against the rules of the unslop skill
//! (docs/onboarding.md says where it comes from).
use super::settings::Out;
use crate::state::State;
use serde_json::{json, Value};

/// Who is asking. `phone`: an app with no CAD or install steps of its own. `os` decides the words for keys; the hub's own `os` decides the install words.
#[derive(Clone, Debug, PartialEq)]
pub struct Client { pub phone: bool, pub os: String, pub launcher: bool }
impl Client {
    pub fn of(p: &Value) -> Client {
        let c = &p["client"];
        Client { phone: c["kind"].as_str() != Some("desktop"), os: c["os"].as_str().unwrap_or("").to_string(), launcher: c["launcher"].as_bool().unwrap_or(false) }
    }
}

/// The person's own folder on the hub's machine: where a first project goes.
pub fn home_dir() -> String { std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default() }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark { Done, Skipped, Todo }
impl Mark { fn word(self) -> &'static str { match self { Mark::Done => "done", Mark::Skipped => "skipped", Mark::Todo => "todo" } } }

/// One step as drawn. `task`: it has something to finish, so Next reads Skip step until it is done.
pub struct Step { pub id: &'static str, pub group: &'static str, pub title: String, pub body: String, pub rows: Vec<Value>, pub choices: Vec<Value>, pub panel: &'static str, pub done: bool, pub task: bool }

pub const GROUPS: &[&str] = &["Install", "Agents", "CAD tools", "Devices", "First project", "Tour"];

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn set<'a>(st: &'a State, k: &str) -> &'a str { st.settings.get(k).map(String::as_str).unwrap_or("") }
fn draft<'a>(session: &'a Value, k: &str) -> &'a str { s(&session["drafts"], k) }
fn list(v: &str) -> Vec<&str> { v.split(',').filter(|x| !x.is_empty()).collect() }
fn btn(label: &str, action: &str, value: &str, on: bool) -> Value { json!({"label": label, "action": action, "value": value, "on": on}) }
fn row(label: &str, note: &str, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "buttons": buttons}) }
fn copy(cmd: &str) -> Value { btn("Copy", "copy", cmd, false) }
fn link(label: &str, url: &str) -> Value { btn(label, "open-url", url, false) }
fn choice(label: &str, action: &str, value: &str) -> Value { json!({"label": label, "action": action, "value": value}) }

fn modkey(c: &Client, hub_os: &str) -> &'static str { if (if c.os.is_empty() { hub_os } else { &c.os }) == "darwin" { "Cmd" } else { "Ctrl" } }

/// Everything setup says or looks for that differs by system. A new system is one more entry in `PLATFORMS`: the steps read only this, and the tests walk the list.
pub struct Platform {
    /// The hub's `os`.
    pub id: &'static str, pub name: &'static str,
    /// The Installed step's words, and what to do when the terminal command is not on PATH.
    pub installed: &'static str, pub path_fix: &'static str,
    /// The command that opens Backplane from a terminal.
    pub launcher: &'static str,
    /// How to install each CAD tool: where it applies, the command.
    pub kicad: &'static [(&'static str, &'static str)], pub freecad: &'static [(&'static str, &'static str)],
    /// Where the checks look, after PATH: kicad-cli's names and folders, FreeCAD's console names.
    pub kicad_names: &'static [&'static str], pub kicad_places: &'static [&'static str], pub freecad_names: &'static [&'static str],
}
pub const PLATFORMS: &[Platform] = &[
    Platform { id: "linux", name: "Linux",
        installed: "The installer puts Backplane in ~/.local/share/backplane, links the backplane command into ~/.local/bin and adds Backplane to your applications menu. Closing the window leaves the hub running, so agents finish their work.",
        path_fix: "Add ~/.local/bin to your PATH to run backplane from a terminal.", launcher: "backplane",
        kicad: &[("Debian and Ubuntu", "sudo apt install kicad"), ("Fedora", "sudo dnf install kicad"), ("Arch", "sudo pacman -S kicad")],
        freecad: &[("Debian and Ubuntu", "sudo apt install freecad"), ("Fedora", "sudo dnf install freecad"), ("Arch", "sudo pacman -S freecad")],
        kicad_names: &["kicad-cli"], kicad_places: &["/usr/bin/kicad-cli", "/usr/local/bin/kicad-cli"], freecad_names: &["freecadcmd", "FreeCADCmd"] },
    Platform { id: "darwin", name: "macOS",
        installed: "The app is in Applications, or in the folder where you unpacked it. macOS builds are not notarized yet, so macOS asks you to approve the app once. If it refused to open, use Open Anyway in System Settings, Privacy and Security. Closing the window leaves the hub running, so agents finish their work.",
        path_fix: "Add ~/.local/bin to your PATH to run backplane from a terminal.", launcher: "backplane",
        kicad: &[("Homebrew", "brew install --cask kicad")], freecad: &[("Homebrew", "brew install --cask freecad")],
        kicad_names: &["kicad-cli"], kicad_places: &["/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli"], freecad_names: &["freecadcmd", "FreeCADCmd"] },
    Platform { id: "windows", name: "Windows",
        installed: "Backplane is installed in your user folder, under AppData, Local, Backplane. Its guard keeps it updated and brings the hub back if it stops. The Start menu opens it. Closing the window leaves the hub running, so agents finish their work. Windows may ask for permission once when a tool's own installer starts.",
        path_fix: "Backplane is not on your PATH. Open it from its folder or the Start menu.", launcher: "backplane.exe",
        kicad: &[("winget", "winget install --id KiCad.KiCad -e")], freecad: &[("winget", "winget install --id FreeCAD.FreeCAD -e")],
        kicad_names: &["kicad-cli.exe"], kicad_places: &["C:\\Program Files\\KiCad\\10.0\\bin\\kicad-cli.exe", "C:\\Program Files\\KiCad\\9.0\\bin\\kicad-cli.exe"], freecad_names: &["FreeCADCmd.exe", "freecadcmd.exe"] },
];
/// The entry for this hub's system; one with no entry yet reads the first.
pub fn platform(os: &str) -> &'static Platform { PLATFORMS.iter().find(|p| p.id == os).unwrap_or(&PLATFORMS[0]) }

/// An agent the setup looks for: how to install it per system (a system with no line follows the guide), where to read more, and how it signs in.
pub struct Agent { pub id: &'static str, pub name: &'static str, pub install: &'static [(&'static str, &'static str)], pub guide: &'static str, pub sign_in: &'static str }
pub const AGENTS: &[Agent] = &[
    Agent { id: "claude", name: "Claude Code", install: &[("linux", "curl -fsSL https://claude.ai/install.sh | bash"), ("darwin", "curl -fsSL https://claude.ai/install.sh | bash"), ("windows", "irm https://claude.ai/install.ps1 | iex")], guide: "https://code.claude.com/docs/en/setup", sign_in: "" },
    Agent { id: "codex", name: "Codex", install: &[], guide: "https://developers.openai.com/codex/cli", sign_in: "" },
    Agent { id: "grok", name: "Grok Build", install: &[("linux", "curl -fsSL https://x.ai/cli/install.sh | bash"), ("darwin", "curl -fsSL https://x.ai/cli/install.sh | bash"), ("windows", "irm https://x.ai/cli/install.ps1 | iex")], guide: "https://x.ai/cli", sign_in: "grok login" },
    Agent { id: "muse", name: "Muse Code", install: &[], guide: "https://dev.meta.ai/docs/muse-code/auth", sign_in: "muse login" },
    Agent { id: "opencode", name: "OpenCode", install: &[], guide: "https://opencode.ai", sign_in: "opencode auth login" },
    Agent { id: "pi", name: "Pi", install: &[], guide: "https://pi.dev", sign_in: "pi, then /login" },
    Agent { id: "omp", name: "Oh My Pi", install: &[], guide: "https://github.com/can1357/oh-my-pi", sign_in: "omp, then /login" },
];
fn install_cmd<'a>(a: &'a Agent, os: &str) -> &'a str { a.install.iter().find(|(p, _)| *p == os).map_or("", |(_, c)| *c) }

/// What the checks found, and the state's own answers.
struct Facts<'a> { st: &'a State, info: &'a Value, os: String, home: String, listen: String, exe: String, on_path: String, checks: Value, checked: bool, tailscale: String, peers: usize, devices: usize, version: String, machines: Value, installs: Value, arch: String, update: String }
impl<'a> Facts<'a> {
    fn of(st: &'a State, info: &'a Value) -> Facts<'a> {
        let checks: Value = st.settings.get("onboard.checks").and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null);
        let tailscale = { let t = s(&checks, "tailscale"); if t.is_empty() { s(info, "tailscale").to_string() } else { t.to_string() } };
        let peers = st.settings.keys().filter(|k| k.starts_with("peer.own.")).count();
        Facts { st, info, os: { let o = s(info, "os"); if o.is_empty() { "linux".into() } else { o.to_string() } }, home: s(info, "home").into(), listen: s(info, "listen").into(), exe: s(&checks, "exe").into(), on_path: s(&checks, "onPath").into(),
            checked: checks.is_object(), tailscale, peers, devices: checks["devices"].as_u64().unwrap_or(0) as usize, version: super::settings::version().to_string(), machines: info["machines"].clone(), installs: serde_json::from_str(s(info, "onboard.installs")).unwrap_or(Value::Null), arch: s(info, "arch").into(), update: s(info, "update").into(), checks }
    }
    /// The rows for a tool that lives outside Backplane: its Install button (or page), how an install is going, and a retry. `tools_install::plan` decides.
    fn install_rows(&self, tool: &str, label: &str) -> Vec<Value> {
        use crate::settings::tools_install::{plan, Plan};
        let st = &self.installs[tool];
        let page = |p: &Option<Plan>| match p { Some(Plan::Link(u)) => Some(link("Download", u)), _ => None };
        let p = plan(tool, &self.os, &self.arch, self.checks["winget"] == true, self.checks["node"] == true);
        match (s(st, "state"), &p) {
            ("running", _) => vec![row(&format!("Installing {label}"), if s(st, "note").is_empty() { "Starting." } else { s(st, "note") }, vec![])],
            ("failed", _) => { let mut b = vec![btn("Try again", "ob-install", tool, true)]; b.extend(page(&p)); let mut r = row(&format!("{label} did not install"), &format!("{}.", s(st, "note").trim_end_matches('.')), b); r["tone"] = json!("warn"); vec![r] }
            (_, Some(Plan::Winget(_))) => vec![row(&format!("Install {label}"), "Installs with winget. Windows may ask for permission once.", vec![btn("Install", "ob-install", tool, true)])],
            (_, Some(Plan::Sh(..))) | (_, Some(Plan::Ps(_))) => vec![row(&format!("Install {label}"), &format!("Downloads and runs {}.", if tool == "codex" { "Codex's own release, checked against its published checksum" } else { "its vendor's own installer" }), vec![btn("Install", "ob-install", tool, true)])],
            (_, Some(Plan::Link(u))) => vec![row(&format!("Get {label}"), "Opens its download page.", vec![link("Download", u)])],
            (_, None) => vec![],
        }
    }
    fn agent(&self, id: &str) -> &Value { &self.checks["agents"][id] }
    fn found(&self, id: &str) -> bool { !s(self.agent(id), "path").is_empty() }
    fn accounts(&self, id: &str) -> Vec<String> { self.checks["accounts"][id].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect() }
    fn signed(&self, id: &str) -> bool { s(self.agent(id), "signed") == "yes" || !self.accounts(id).is_empty() }
    fn runs(&self, tool: &str) -> bool { self.checks[tool]["runs"].as_bool().unwrap_or(false) }
    fn projects(&self) -> Vec<&crate::state::Project> { self.st.projects.values().filter(|p| !p.removed).collect() }
    fn threads(&self) -> usize { let p: Vec<&str> = self.st.projects.iter().filter(|(_, p)| !p.removed).map(|(id, _)| id.as_str()).collect(); self.st.threads.values().filter(|t| !t.deleted && p.contains(&t.project.as_str())).count() }
}

fn looking(f: &Facts) -> &'static str { if f.checked { "" } else { "Checking this machine." } }

// ---- the steps ----

fn welcome(f: &Facts, c: &Client) -> Step {
    let body = if c.phone { "Backplane runs coding agents on your boards, schematics and mechanical parts. This setup shows how to use it from this phone. Next moves on. Finish later closes it, and it comes back at the same step. Settings, Setup shows it again.".to_string() }
        else { "Backplane runs coding agents on your boards, schematics and mechanical parts, and draws the results in its own viewers. This setup checks what you have, helps you add what is missing and ends with a tour of each feature. Every step can be skipped. Enter moves on. Esc is Finish later, which brings you back to this step at the next launch. End setup stops it for good. Press a step in the list to go straight to it. Settings, Setup runs all of this again.".to_string() };
    let _ = f;
    Step { id: "welcome", group: "Install", title: "Welcome to Backplane".into(), body, rows: vec![], choices: vec![], panel: "", done: true, task: false }
}

fn install(f: &Facts) -> Step {
    let pf = platform(&f.os);
    let (text, path_fix) = (pf.installed, pf.path_fix);
    let body = format!("You are running Backplane {} on {}. {text}", f.version, pf.name);
    let mut rows = vec![];
    if !f.exe.is_empty() { rows.push(row("Program", &f.exe, vec![])); }
    rows.push(if !f.on_path.is_empty() { row("Terminal command", &format!("Found at {}.", f.on_path), vec![]) } else if f.checked { row("Terminal command", path_fix, vec![]) } else { row("Terminal command", looking(f), vec![]) });
    rows.push(row("Updates", "Settings, Network and updates picks the stable or the nightly channel.", vec![]));
    Step { id: "install", group: "Install", title: "Installed".into(), body, rows, choices: vec![], panel: "", done: true, task: false }
}

fn home(f: &Facts, c: &Client) -> Step {
    let body = "Backplane keeps your threads, settings, bots and keys in one folder on this computer. A background program, the hub, runs your agents from that folder. It keeps running when you close the window, so agents finish their work. Each computer usually has one.".to_string();
    let mut rows = vec![row("Data folder", &f.home, vec![]), row("Address", &format!("The hub listens on {}.", f.listen), vec![])];
    if c.launcher { rows.push(row("Use another folder", "Backplane restarts, asks for the folder and starts a hub there. This folder stays as it is, and its hub keeps running until this computer restarts.", vec![btn("Choose another folder", "ob-home", "", false)])); }
    Step { id: "home", group: "Install", title: "Where your data is kept".into(), body, rows, choices: vec![], panel: "", done: !f.home.is_empty(), task: false }
}

// ---- the 0.14 import (import014/mod.rs does it; these are its words) ----

/// The import as the hub info `import.v014` tells it. None: no 0.14 home on this computer and nothing imported.
pub fn import_info(info: &Value) -> Option<Value> { info["import.v014"].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()).filter(|v| v["state"].is_string() && v["state"] != "none") }
fn num(v: &Value, k: &str) -> u64 { v[k].as_u64().unwrap_or(0) }
fn many(n: u64, one: &str, more: &str) -> String { format!("{} {}", thousands(n), if n == 1 { one } else { more }) }
/// "12 projects, 1045 threads with 170677 messages, 6 bots with 1293 memories, ..." for what is not zero.
pub fn import_counted(c: &Value) -> String {
    let mut parts = vec![];
    if num(c, "projects") > 0 { parts.push(many(num(c, "projects"), "project", "projects")) }
    if num(c, "threads") > 0 { parts.push(format!("{} with {}", many(num(c, "threads"), "thread", "threads"), many(num(c, "messages"), "message", "messages"))) }
    else if num(c, "messages") > 0 { parts.push(format!("{} in threads already here", many(num(c, "messages"), "new message", "new messages"))) }
    if num(c, "bots") > 0 || num(c, "memories") > 0 { parts.push(format!("{} with {}", many(num(c, "bots"), "bot", "bots"), many(num(c, "memories"), "memory", "memories"))) }
    if num(c, "rooms") > 0 || num(c, "posts") > 0 { parts.push(format!("{} with {}", many(num(c, "rooms"), "room", "rooms"), many(num(c, "posts"), "post", "posts"))) }
    if num(c, "routines") > 0 { parts.push(many(num(c, "routines"), "routine", "routines")) }
    if num(c, "settings") > 0 { parts.push(many(num(c, "settings"), "setting", "settings")) }
    if num(c, "devices") > 0 { parts.push(many(num(c, "devices"), "paired device", "paired devices")) }
    // on its own (Import again on a home imported before 0.14's subagents came under their threads): said, so it is not "Nothing new"
    if num(c, "threads") == 0 && num(c, "parents") > 0 { parts.push(format!("{} under the threads that started them", many(num(c, "parents"), "subagent thread", "subagent threads"))) }
    match parts.len() { 0 => "Nothing new.".into(), 1 => format!("{}.", parts[0]), n => format!("{} and {}.", parts[..n - 1].join(", "), parts[n - 1]) }
}
/// The label and words for one note on what stays behind or needs doing.
pub fn import_note(n: &Value) -> (String, String) {
    let (k, c) = (s(n, "k"), num(n, "n"));
    let (l, t) = match k {
        "runs" => ("Mail runs", format!("{} in 0.14. They were the mailroom's own working threads, and its mail stays with them.", if c == 1 { "1 mail run stays".to_string() } else { format!("{c} mail runs stay") })),
        "routines-off" => ("Routines", format!("{} switched off, so none runs twice while 0.14 still runs. Turn each on in its bot's Routines tab.", if c == 1 { "1 routine arrives".to_string() } else { format!("{c} routines arrive") })),
        "machines" => ("Other machines", "Links to your other machines stay in 0.14. Link them again in Settings, Devices.".into()),
        "keys-stay" => ("Keys", "Keys, webhook secrets and sign-ins stay in 0.14 unless you import with keys.".into()),
        "google" => ("Google", "The Google connection is not part of this version and stays in 0.14.".into()),
        "clashes" => ("Name clashes", format!("{} in 0.14 because this hub already uses {} for something else.", if c == 1 { "1 item stays".to_string() } else { format!("{c} items stay") }, if c == 1 { "its name" } else { "their names" })),
        "fresh" => ("Starts fresh", "Turn state, queues, wakes, locks and usage meters start fresh here.".into()),
        "running" => ("0.14 is running", "0.14 is still running. Import again later to bring what it adds after now.".into()),
        "bot-folder" => ("Bot folder", format!("The folder of {} holds {} GB and stays at {}. Its threads and memory came across.", s(n, "name"), num(n, "gb"), s(n, "path"))),
        "copy-failed" => ("Not copied", format!("{} could not be copied: {}.", s(n, "path"), s(n, "why").trim_end_matches('.'))),
        "sign-in" => ("Sign in again", format!("Sign in again to {}. Their history came across.", n["names"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default())),
        "accounts-later" => ("Accounts", "The account profiles were copied but not added yet. Import again once the hub has finished starting.".into()),
        "vault-failed" => ("Keys", format!("The 0.14 keys did not come: {}. Add them again in Settings, API keys.", s(n, "why").trim_end_matches('.'))),
        _ => ("", String::new()),
    };
    (l.to_string(), t)
}
/// The rows for the import, in setup and in Settings, Import.
pub fn import_rows(info: &Value) -> Vec<Value> {
    let Some(v) = import_info(info) else { return vec![row("Nothing to import", "No Backplane 0.14 data was found on this computer. If it is in another folder, start the hub with BACKPLANE_IMPORT_FROM set to that folder.", vec![])] };
    let notes = |v: &Value| v["notes"].as_array().into_iter().flatten().map(import_note).filter(|(_, t)| !t.is_empty()).map(|(l, t)| row(&l, &t, vec![])).collect::<Vec<_>>();
    let mut rows = vec![];
    match s(&v, "state") {
        "found" => rows.push(row("What is there", "Not counted yet. Counting reads the 0.14 log and changes nothing.", vec![btn("Count", "ob-import", "scan", true)])),
        "scanning" => rows.push(row("Counting", "Reading the 0.14 log.", vec![])),
        "ready" => {
            let c = &v["counts"];
            if num(c, "lines") == 0 { rows.push(row("Nothing new", "Everything in the 0.14 log is here already.", vec![btn("Count again", "ob-import", "scan", false)])) }
            else {
                rows.push(row("What comes across", &import_counted(c), vec![]));
                rows.push(row("Import", "Copies them into this hub now. Keys and sign-ins stay in 0.14.", vec![btn("Import", "ob-import", "run", true)]));
                rows.push(row("Import with keys", "Also copies the vault, the voice key, webhook secrets and your Claude and Codex sign-ins. If 0.14 keeps running, one of them may ask you to sign in again.", vec![btn("Import with keys", "ob-import", "run-secrets", false)]));
            }
            rows.extend(notes(&v));
        }
        "running" => rows.push(row("Importing", &if num(&v, "of") == 0 { "Reading the 0.14 log.".to_string() } else { format!("{} of {} events written.", num(&v, "done"), num(&v, "of")) }, vec![])),
        "done" => {
            let c = &v["counts"];
            let what = if num(c, "lines") == 0 { "Nothing new came across: everything was here already.".to_string() } else { import_counted(c) };
            rows.push(row("Imported", &what, vec![]));
            rows.push(row("Import again", "Counts what 0.14 added since. Nothing already here comes twice.", vec![btn("Count again", "ob-import", "scan", false)]));
            rows.extend(notes(&v));
        }
        _ => { let mut r = row("The import stopped", s(&v, "why"), vec![btn("Try again", "ob-import", "scan", true)]); r["tone"] = json!("warn"); rows.push(r) }
    }
    rows
}
fn thousands(n: u64) -> String { let s = n.to_string(); let mut o = String::new(); for (i, c) in s.chars().enumerate() { if i > 0 && (s.len() - i) % 3 == 0 { o.push(',') } o.push(c) } o }
/// The window over everything while a 0.14 home comes across, and after it until the person has seen what came (`import014::ACK`); a failure until they
/// retry or put it off for this launch (`@import.later`). The desktop draws it as it is: a title, a body, the bar (`pct`), lines and buttons. It has no close
/// while the import runs: the hub serves meanwhile, and the window comes back on every client until the import ends.
pub fn import_overlay(st: &State, session: &Value, info: &Value) -> Option<Value> {
    let v = import_info(info)?;
    import_card(&v, set(st, super::super::import014::ACK) == v["at"].to_string(), draft(session, "@import.later") == "1")
}
/// The card for one import state (the info key's value, or a data/1 `migrating` frame): what the desktop's overlay and the phones' banner show. `seen`: the
/// person has seen this result; `later`: they put off a failure for now.
pub fn import_card(v: &Value, seen: bool, later: bool) -> Option<Value> {
    let v = v.clone();
    let btns = |b: Vec<Value>| Value::Array(b);
    match s(&v, "state") {
        "running" => {
            let (p, t) = (&v["projects"], &v["threads"]);
            let mut lines = vec![];
            if v["phase"] == "writing" { lines.push(format!("{} of {} projects \u{b7} {} of {} threads \u{b7} {} of {} events", thousands(p[0].as_u64().unwrap_or(0)), thousands(p[1].as_u64().unwrap_or(0)), thousands(t[0].as_u64().unwrap_or(0)), thousands(t[1].as_u64().unwrap_or(0)), thousands(num(&v, "done")), thousands(num(&v, "total")))); }
            else if v["phase"] == "reading" && num(&v, "total") > 0 { lines.push(format!("{} of {} MB read", num(&v, "done") >> 20, num(&v, "total") >> 20)); }
            else if v["phase"] == "copying" { lines.push(format!("{} files copied", thousands(num(&v, "done")))); }
            if !s(&v, "current").is_empty() { lines.push(format!("Now: {}", s(&v, "current"))); }
            lines.push(match v["eta_s"].as_u64() { Some(e) if e >= 60 => format!("About {} min left in this step.", (e + 59) / 60), Some(e) => format!("About {e} s left in this step."), None => "Measuring how long this step takes.".into() });
            Some(json!({"state": "running", "title": "Bringing your 0.14 work across\u{2026}", "body": format!("{}. Backplane works meanwhile, and your 0.14 folder is only read.", s(&v, "what")), "pct": num(&v, "pct"), "lines": lines, "buttons": btns(vec![])}))
        }
        "done" => {
            if seen { return None }
            let c = &v["counts"];
            let what = if num(c, "lines") == 0 { "Nothing new came across: everything was here already.".to_string() } else { import_counted(c) };
            let mut lines: Vec<String> = v["notes"].as_array().into_iter().flatten().map(import_note).filter(|(_, t)| !t.is_empty()).map(|(_, t)| t).collect();
            if let Some(e) = v["elapsed_s"].as_u64() { lines.insert(0, format!("It took {} s.", e.max(1))); }
            let mut b = vec![btn("Continue", "ob-import", &format!("ack:{}", v["at"]), true)];
            if v["secrets"] != true { b.push(btn("Import with keys", "ob-import", "run-secrets", false)); }
            Some(json!({"state": "done", "title": "Your 0.14 work is here", "body": what, "pct": 100, "lines": lines, "buttons": btns(b)}))
        }
        "failed" => {
            if later { return None }
            let from = s(&v, "from");
            Some(json!({"state": "failed", "title": "The import stopped", "body": format!("{}. Your 0.14 data{} is untouched. Retry picks up where it stopped: nothing comes twice.", s(&v, "why").trim_end_matches('.'), if from.is_empty() { String::new() } else { format!(" in {from}") }),
                "pct": Value::Null, "lines": [], "buttons": btns(vec![btn("Retry", "ob-import", "retry", true), btn("Not now", "ob-import", "later", false)])}))
        }
        _ => None,
    }
}
/// Setup's import step: only on a desktop, and only when this computer has 0.14 data or an import was made.
fn import(f: &Facts, c: &Client) -> Option<Step> {
    if c.phone { return None }
    let v = import_info(f.info)?;
    let from = s(&v, "from");
    let body = if from.is_empty() { "Your Backplane 0.14 data was copied into this hub.".to_string() } else { format!("Backplane 0.14 kept its data in {from}. This hub can copy your projects, threads, bots, rooms and settings from it. The 0.14 folder is only read: nothing in it changes or moves, and 0.14 keeps working.") };
    Some(Step { id: "import", group: "Install", title: "Bring your 0.14 projects across?".into(), body, rows: import_rows(f.info), choices: vec![], panel: "", done: v["state"] == "done", task: true })
}

/// The anonymous-usage step: On is preselected, and finishing setup with it on is the choice (docs/telemetry.md).
fn usage(f: &Facts) -> Step {
    let off = crate::settings::telemetry::off(set(f.st, "telemetry.share"), set(f.st, "telemetry.usage.asked"), set(f.st, "onboard.usage"));
    let rows = vec![row("Share anonymous usage", "", vec![btn("On", "ob-usage", "on", !off), btn("Off", "ob-usage", "off", off)])];
    Step { id: "usage", group: "Install", title: "Anonymous usage".into(), body: USAGE_NOTE.to_string(), rows, choices: vec![], panel: "", done: true, task: true }
}
/// What the telemetry step says is sent, all of it.
pub const USAGE_NOTE: &str = "On unless you turn it off. Backplane sends anonymous counts: installs, updates, daily activity, and daily totals of threads, messages, model tokens, bot traffic, boards made and boards ordered, with a random install id, the version, the system and the day. Never your conversations, projects, files, names, paths or keys. Settings, Privacy changes it.";

fn agent_note(f: &Facts, a: &Agent) -> String {
    let c = f.agent(a.id);
    if f.found(a.id) {
        let ver = s(c, "version");
        let at = if ver.is_empty() { format!("Found at {}.", s(c, "path")) } else { format!("Version {ver} at {}.", s(c, "path")) };
        match s(c, "signed") { "yes" => { let who = s(c, "who"); if who.is_empty() { format!("{at} Signed in.") } else { format!("{at} Signed in as {who}.") } }, "no" => format!("{at} Not signed in."), _ => if a.sign_in.is_empty() { at } else { format!("{at} To sign in, run {}.", a.sign_in) } }
    } else if !f.checked { looking(f).to_string() } else { "Not installed.".to_string() }
}
fn agent_rows(f: &Facts, a: &Agent) -> Vec<Value> {
    let mut r = vec![row(a.name, &agent_note(f, a), vec![link("Install guide", a.guide)])];
    if !f.found(a.id) && f.checked {
        let rows = f.install_rows(a.id, a.name);
        if !rows.is_empty() { r.extend(rows); let cmd = install_cmd(a, &f.os); if !cmd.is_empty() { r.push(row("Or in a terminal", cmd, vec![copy(cmd)])); } } else { r.push(row("Install", &format!("No install button yet. Follow the guide, then make sure {} is on your PATH.", a.id), vec![])) }
    }
    r
}

fn claude(f: &Facts, c: &Client) -> Step {
    let a = &AGENTS[0];
    let names = f.accounts("claude");
    let done = f.signed("claude");
    if c.phone { return Step { id: "claude", group: "Agents", title: "Sign in to Claude".into(), body: "Sign in on the computer that runs the hub, in Setup or in Settings, Accounts. Agents run there, not on this phone.".into(), rows: vec![], choices: vec![], panel: "", done, task: false } }
    let body = "Backplane runs Claude Code, Anthropic's command-line agent, on this computer. Install it, then add an account below. Name it, for example Work or Personal, and press Connect. Open the link, sign in and paste the code Claude shows. Add as many accounts as you have. When one reaches its usage limit, Backplane moves the thread to another. Each thread keeps its own account, and the chip beside the model picker changes it. Use for new threads picks where new threads start. If a sign-in fails or the code expires, press Connect again. Backplane never copies your keys and never runs a paid request to check them.".to_string();
    let mut rows = agent_rows(f, a);
    if !names.is_empty() { rows.push(row("Accounts", &names.join(", "), vec![])); }
    rows.push(row("Check again", "Runs the checks now, after you install or sign in.", vec![btn("Check again", "ob-check", "", false)]));
    Step { id: "claude", group: "Agents", title: "Sign in to Claude".into(), body, rows, choices: vec![], panel: "claude", done, task: true }
}

fn codex(f: &Facts, c: &Client) -> Step {
    let a = &AGENTS[1];
    let names = f.accounts("codex");
    let done = f.signed("codex");
    if c.phone { return Step { id: "codex", group: "Agents", title: "Sign in to Codex".into(), body: "Sign in on the computer that runs the hub, in Setup or in Settings, Accounts.".into(), rows: vec![], choices: vec![], panel: "", done, task: false } }
    let body = "Codex is OpenAI's command-line agent. Install it, add an account below and press Connect. Backplane shows a link and a code to enter there, and the step turns done when Codex reports the sign-in. You need only one agent, so skip this step if you use Claude.".to_string();
    let mut rows = agent_rows(f, a);
    if !names.is_empty() { rows.push(row("Accounts", &names.join(", "), vec![])); }
    rows.push(row("Check again", "Runs the checks now, after you install or sign in.", vec![btn("Check again", "ob-check", "", false)]));
    Step { id: "codex", group: "Agents", title: "Sign in to Codex".into(), body, rows, choices: vec![], panel: "codex", done, task: true }
}

fn others(f: &Facts, c: &Client) -> Step {
    let done = AGENTS[2..].iter().any(|a| f.found(a.id));
    if c.phone { return Step { id: "others", group: "Agents", title: "Other agents".into(), body: "Backplane also runs Grok Build, Muse Code, OpenCode, Pi and Oh My Pi. Install them on the computer that runs the hub.".into(), rows: vec![], choices: vec![], panel: "", done, task: false } }
    let body = "Backplane also runs these agents. Each one signs in with its own command, in a terminal on this computer, and Backplane cannot check that sign-in. An agent that is not signed in says so in its first reply. Skip this step if you use Claude or Codex. Antigravity is listed in the model picker, but this build cannot install it yet.".to_string();
    let mut rows: Vec<Value> = AGENTS[2..].iter().flat_map(|a| agent_rows(f, a)).collect();
    rows.push(row("Check again", "Runs the checks now, after you install one.", vec![btn("Check again", "ob-check", "", false)]));
    Step { id: "others", group: "Agents", title: "Other agents".into(), body, rows, choices: vec![], panel: "", done, task: true }
}

fn tool_note(f: &Facts, tool: &str, name: &str) -> String {
    let t = &f.checks[tool];
    if f.runs(tool) { format!("{name} {} runs from {}.", s(t, "version"), s(t, "path")) }
    else if !s(t, "path").is_empty() { format!("Found at {}, but it does not run: {}.", s(t, "path"), s(t, "why")) }
    else if f.checked { format!("{name} was not found.") } else { looking(f).to_string() }
}
/// Where Backplane KiCad's releases are: the fallback when the install cannot run here.
pub const KICAD_RELEASES: &str = "https://github.com/i2cjak/Backplane_KiCad/releases";
pub const APK_RELEASES: &str = "https://github.com/BackplaneWorks/Backplane/releases/latest";
pub const IOS_GUIDE: &str = "https://github.com/BackplaneWorks/Backplane/blob/main/docs/install.md#phones";
pub const TS_IOS: &str = "https://apps.apple.com/app/tailscale/id1470499037";
pub const TS_ANDROID: &str = "https://play.google.com/store/apps/details?id=com.tailscale.ipn";
pub const KICAD_STOCK: &str = "https://www.kicad.org/download/";
fn kicad(f: &Facts, st: &State, session: &Value, c: &Client) -> Step { kicad_with(f, st, session, c, crate::files::kicad::install::package()) }
/// The KiCad step for a computer with this package (None: none is published for it). What happens is `install::decide`'s; the hub starts the
/// `Auto` install itself (`settings/onboard.rs`), this only says how it goes: progress from info `kicad.install`, status and version from the checks.
fn kicad_with(f: &Facts, st: &State, session: &Value, c: &Client, pkg: Option<&crate::files::kicad::install::Package>) -> Step {
    use crate::files::kicad::install::{decide, Action};
    let i = |k: &str| s(f.info, k);
    let (status, version) = (i("kicad.status"), i("kicad.version"));
    let prog: Value = serde_json::from_str(i("kicad.install")).unwrap_or_default();
    let (pstate, note) = (s(&prog, "state"), s(&prog, "note"));
    let action = decide(pkg, status, pstate);
    let done = f.runs("kicad") || matches!(status, "ok" | "stock" | "old");
    if c.phone { return Step { id: "kicad", group: "CAD tools", title: "KiCad".into(), body: "KiCad runs on the computer that runs the hub, and the hub installs Backplane KiCad there by itself. This phone shows boards and schematics the hub draws.".into(), rows: vec![], choices: vec![], panel: "", done, task: false } }
    let body = "Agents use KiCad to read, change and check boards and schematics. Backplane draws boards itself, but it needs kicad-cli to plot and export them. Backplane KiCad is our build of KiCad with an editing API, so agents can edit a board in a running KiCad. Setup downloads it for you when no KiCad is found: it checks the download against its checksum and installs it in Backplane's own folder, with no administrator password. Stock KiCad 9 or later works for viewing and export.".to_string();
    let path = if session["drafts"]["@f.obkicad"].is_string() { draft(session, "@f.obkicad").to_string() } else { set(st, "kicad.path").to_string() };
    let mut rows = vec![];
    let version_of = |v: &str| if v.is_empty() { String::new() } else { format!(" {v}") };
    let fork = match action {
        Action::Nothing => row("Backplane KiCad", &format!("Status ok. Backplane KiCad{} runs from {}. Agents can edit a board in a running KiCad.", version_of(version), i("kicad.cli")), vec![]),
        Action::Wait if pstate == "running" => row(&format!("Installing Backplane KiCad{}", version_of(s(&prog, "version"))), if note.is_empty() { "Starting." } else { note }, vec![]),
        Action::Wait => row("Backplane KiCad", "Checking this machine.", vec![link("Releases", KICAD_RELEASES)]),
        Action::Auto => row("Backplane KiCad", &format!("Starting the download of Backplane KiCad {}. This takes a few minutes.", pkg.map_or("", |p| p.version)), vec![link("Releases", KICAD_RELEASES)]),
        Action::Offer if pstate == "failed" => { let mut r = row("Backplane KiCad", &format!("{note}."), vec![btn("Try again", "ob-kicad-install", "", true), link("Releases", KICAD_RELEASES)]); r["tone"] = json!("warn"); r }
        Action::Offer => row("Backplane KiCad", &format!("{} Backplane KiCad adds editing: agents change a board in a running KiCad. Install it beside this one in one press, or keep what you have.", offer_lead(status, version)),
            vec![btn("Install Backplane KiCad", "ob-kicad-install", "", true), link("Releases", KICAD_RELEASES)]),
        Action::NoPackage => row("Backplane KiCad", &format!("No Backplane KiCad is published for this computer ({}). Install stock KiCad below, or set the path of a Backplane KiCad you have.", crate::files::kicad::install::this_computer()), vec![link("Releases", KICAD_RELEASES)]),
    };
    rows.push(fork);
    let found = if i("kicad.cli").is_empty() { tool_note(f, "kicad", "KiCad") } else { format!("{} {} at {}.", if status == "stock" { "Stock KiCad" } else { "KiCad" }, version, i("kicad.cli")).replace("  ", " ") };
    rows.push(row("KiCad found", &found, vec![]));
    if matches!(action, Action::NoPackage) && !done && f.checked { for (name, cmd) in platform(&f.os).kicad { rows.push(row(name, cmd, vec![copy(cmd)])); } }
    if matches!(action, Action::NoPackage) { rows.push(row("Stock KiCad", "Works for viewing and export. Agents then edit the files.", vec![link("Download KiCad", KICAD_STOCK)])); }
    let mut located = row("Locate it", "If KiCad is somewhere else, give the full path to kicad-cli.", vec![btn("Use this path", "ob-kicad", "", false), btn("Check again", "ob-check", "", false)]);
    located["fields"] = json!([{"name": "obkicad", "hint": "Path to kicad-cli", "secret": false, "value": path}]);
    rows.push(located);
    Step { id: "kicad", group: "CAD tools", title: "KiCad".into(), body, rows, choices: vec![], panel: "", done, task: true }
}
fn offer_lead(status: &str, version: &str) -> String {
    let v = if version.is_empty() { String::new() } else { format!(" {version}") };
    match status {
        "stock" => format!("Stock KiCad{v} works for viewing and export."),
        "old" => format!("The Backplane KiCad{v} found is too old for editing."),
        _ => format!("The KiCad found{v} does not run."),
    }
}
fn freecad(f: &Facts, st: &State, session: &Value, c: &Client) -> Step {
    let done = f.runs("freecad");
    if c.phone { return Step { id: "freecad", group: "CAD tools", title: "FreeCAD".into(), body: "FreeCAD runs on the computer that runs the hub. This phone shows the parts it makes.".into(), rows: vec![], choices: vec![], panel: "", done, task: false } }
    let body = "Agents use FreeCAD to model enclosures, brackets and other parts. Skip this step if you only design boards. To check it, Backplane runs FreeCAD's console program and asks for its version.".to_string();
    let path = if session["drafts"]["@f.obfreecad"].is_string() { draft(session, "@f.obfreecad").to_string() } else { set(st, "freecad.path").to_string() };
    let mut rows = vec![row("FreeCAD", &tool_note(f, "freecad", "FreeCAD"), vec![])];
    if !done && f.checked { rows.extend(f.install_rows("freecad", "FreeCAD")); }
    if !done && f.checked { for (name, cmd) in platform(&f.os).freecad { rows.push(row(name, cmd, vec![copy(cmd)])); } }
    let mut located = row("Locate it", "If FreeCAD is somewhere else, give the full path to freecadcmd.", vec![btn("Use this path", "ob-freecad", "", false), btn("Check again", "ob-check", "", false)]);
    located["fields"] = json!([{"name": "obfreecad", "hint": "Path to freecadcmd", "secret": false, "value": path}]);
    rows.push(located);
    Step { id: "freecad", group: "CAD tools", title: "FreeCAD".into(), body, rows, choices: vec![], panel: "", done, task: true }
}

/// The warning for a code that names only this computer (the Machines page's code row, `code.address`).
pub const LOCAL_CODE: &str = "This code names only this computer (127.0.0.1): another computer cannot reach it. Start the hub with Tailscale on, or set BACKPLANE_URL to an address the other machine can reach, then make a new code.";

/// The phones' links to their own Tailscale install: the store of their system, or both when the app did not say.
fn phone_ts(c: &Client) -> Vec<Value> {
    let (ios, android) = (choice("Tailscale on the App Store", "open-url", TS_IOS), choice("Tailscale on Google Play", "open-url", TS_ANDROID));
    match c.os.as_str() { "ios" => vec![ios], "android" => vec![android], _ => vec![ios, android] }
}
fn machine(f: &Facts, session: &Value, c: &Client) -> Step {
    let paired = f.machines["linked"].as_array().into_iter().flatten().filter(|l| l["state"] == "paired").count();
    let done = paired > 0 || f.peers > 0;
    if c.phone { return Step { id: "machine", group: "Devices", title: "Another machine".into(), body: "Each computer runs its own hub. Hubs link only after you pair them with a code and approve the request, in Settings, Machines on a computer. Linked hubs show each other's projects and threads, and a linked machine appears in the list on this phone too. The phone's Settings, Hubs lists the hubs it is paired with.".into(), rows: vec![], choices: phone_ts(c), panel: "", done, task: false } }
    let on = matches!(f.tailscale.as_str(), "on" | "reachable");
    let body = "Each computer you use runs its own hub. Linked hubs show each other's projects and threads in one sidebar, and work stays on the computer that owns it. Hubs never link by themselves, even on one Tailscale account. To link two: make a code here, enter it with this address on the other computer, then approve its request here. You can also run backplane pair --machine over ssh. Both computers need a network route to each other, usually Tailscale. A code works once, for ten minutes. To look at another hub without linking, use the hub list at the top of the sidebar or Settings, Hubs.".to_string();
    let ts_note = if on { "On. This hub listens on your tailnet." } else if f.tailscale == "off" { "Off. This hub started with --no-tailscale." } else { "Not connected. Sign in to Tailscale, then restart Backplane." };
    let mut rows = vec![row("Tailscale", ts_note, vec![])];
    if !on { rows.extend(f.install_rows("tailscale", "Tailscale")); }
    if f.machines.is_object() {
        let draft_of = |k: &str| draft(session, &format!("@f.{k}")).to_string();
        rows.extend(crate::net::machinespage::rows(&f.machines, &draft_of));
        if f.machines["code"]["address"].as_str().map_or(false, super::settings::loopback) && f.machines["code"]["addresses"].as_array().map_or(true, |a| a.is_empty()) { let mut w = row("Only this computer", LOCAL_CODE, vec![]); w["tone"] = json!("warn"); rows.push(w); }
    } else { rows.push(row("Linked machines", &if f.peers == 0 { "None yet.".to_string() } else { format!("{} linked.", f.peers) }, vec![])); }
    Step { id: "machine", group: "Devices", title: "Another machine".into(), body, rows, choices: vec![], panel: "", done, task: true }
}

fn phone(f: &Facts, session: &Value, c: &Client) -> Step {
    let done = f.devices > 0;
    if c.phone { return Step { id: "phone", group: "Devices", title: "This phone".into(), body: "This phone is paired. To add another device, open Settings, Devices, Add a device on the computer that runs the hub.".into(), rows: vec![], choices: [phone_ts(c), vec![choice("Android app on the release page", "open-url", APK_RELEASES), choice("iPhone app: install guide", "open-url", IOS_GUIDE)]].concat(), panel: "", done: true, task: false } }
    let body = "Install the Backplane app: iOS through TestFlight, Android from the signed file on the release page. Then scan the QR code with the app, or type the short code and this hub's address. Each invitation works once, for ten minutes. The phone needs a network route to this hub, usually Tailscale on the same tailnet. Treat the QR code and the link as passwords.".to_string();
    let inv: Value = serde_json::from_str(draft(session, "@invite")).unwrap_or(Value::Null);
    let mut rows = vec![];
    if let Some(l) = inv["link"].as_str() {
        let mut r = row("Pairing link", l, vec![copy(l), btn("Make another", "device-invite", "", false)]);
        r["qr"] = json!(crate::net::qr::text(l).lines().map(str::to_string).collect::<Vec<_>>());
        r["qrnote"] = json!(format!("Short code {}, for ten minutes", s(&inv, "code")));
        rows.push(r);
        if inv["localOnly"] == true { let mut w = row("Only this computer", super::settings::LOCAL_DEVICE, vec![]); w["tone"] = json!("warn"); rows.push(w); }
    } else if let Some(e) = inv["error"].as_str() { rows.push(row("Add a device", &format!("Backplane could not make an invitation: {e}."), vec![btn("Try again", "device-invite", "", false)])); }
    else { rows.push(row("Add a device", "Makes a QR code, a link and a short code for one device.", vec![btn("Show the QR code", "device-invite", "", true)])); }
    rows.push(row("Android app", "The signed file on the release page. Open it on the phone and allow the install.", vec![link("Download the APK", APK_RELEASES)]));
    rows.push(row("iPhone app", "Installed through a TestFlight invitation. The install guide says how to get one.", vec![link("Install guide", IOS_GUIDE)]));
    rows.push(row("Tailscale on the phone", "The phone and this computer need to reach each other, usually on one tailnet.", vec![link("App Store", TS_IOS), link("Google Play", TS_ANDROID)]));
    rows.push(row("Paired devices", &if f.devices == 0 { "None yet.".to_string() } else { format!("{} paired. Settings, Devices lists and revokes them.", f.devices) }, vec![btn("Check again", "ob-check", "", false)]));
    Step { id: "phone", group: "Devices", title: "A phone".into(), body, rows, choices: vec![], panel: "", done, task: true }
}

fn remote(f: &Facts, c: &Client) -> Step {
    let restart = set(f.st, "restart.continue") != "off";
    let cont = if restart { "On. Turns that a restart cuts short go on by themselves." } else { "Off. A restart stops running turns, and you start them again." };
    let upd = f.update.as_str();
    let how = if upd == "package" || upd.is_empty() { format!("This hub is updated by the installer or package it came from. To update, run the install command again{}.", if c.phone { " on the hub's computer" } else { "" }) }
        else { format!("This hub checks for a new build every hour on the {} channel and installs it once no turn is running. If the new build fails its health check, the hub goes back to the one it had. Paired phones show the same line in their Settings.", { let ch = set(f.st, "update.channel"); if ch.is_empty() { "stable" } else { ch } }) };
    let body = format!("The hub keeps running when you close the window, so agents finish their work. {how} Settings, Network and updates shows the version and the update channel. Settings, Threads sets what happens to running turns when the hub restarts.");
    let mut rows = vec![row("Continue after a restart", cont, vec![])];
    if !c.phone { rows.push(row("Open the page", "Closes setup and opens Settings, Network and updates. Finish setup in the sidebar brings you back.", vec![btn("Network and updates", "ob-try", "settings:network", false)])); }
    Step { id: "remote", group: "Devices", title: "Restarts and updates".into(), body, rows, choices: vec![], panel: "", done: true, task: false }
}

fn project(f: &Facts, session: &Value, c: &Client, user_home: &str) -> Step {
    let ps = f.projects();
    let done = !ps.is_empty();
    if c.phone { return Step { id: "project", group: "First project", title: "Your first project".into(), body: "A project is a folder on the hub's machine, usually a KiCad project. Add one with the folder button on the project list.".into(), rows: vec![], choices: vec![], panel: "", done, task: false } }
    let body = "A project is a folder, usually with a KiCad project in it. Give a folder that exists to add it, or a new path to make it. In a git repository, each thread works in its own worktree, so threads do not touch each other's files.".to_string();
    let mut rows: Vec<Value> = ps.iter().take(6).map(|p| row(&p.title, &p.root, vec![])).collect();
    let suggested = format!("{}/Backplane/first-board", user_home.trim_end_matches('/'));
    let path = if session["drafts"]["@f.obproject"].is_string() { draft(session, "@f.obproject").to_string() } else if done { String::new() } else { suggested };
    let note = { let e = draft(session, "@ob.project.err"); if e.is_empty() { if done { "Add another project." } else { "The path to a folder on this machine." }.to_string() } else { e.to_string() } };
    let mut add = row(if done { "Add a project" } else { "Create the project" }, &note, vec![btn(if done { "Add" } else { "Create" }, "ob-project", "", !done)]);
    add["fields"] = json!([{"name": "obproject", "hint": "Folder", "secret": false, "value": path}]);
    rows.push(add);
    Step { id: "project", group: "First project", title: "Your first project".into(), body, rows, choices: vec![], panel: "", done, task: true }
}

fn thread(f: &Facts, session: &Value, c: &Client) -> Step {
    let n = f.threads(); let done = n > 0;
    if c.phone { return Step { id: "thread", group: "First project", title: "Your first thread".into(), body: "A thread is one conversation with an agent in a project. Open a project and press the new thread button, then type what you want.".into(), rows: vec![], choices: vec![], panel: "", done, task: false } }
    let body = "A thread is one conversation with an agent, working in a project. Type what you want in the box at the bottom and press Enter. Pick the agent, model and effort with the picker beside the box. Agents ask before they run tools, unless you choose a looser mode in the same place.".to_string();
    let projects = f.projects();
    let mut rows = vec![];
    if let Some(p) = projects.iter().max_by_key(|p| p.created) {
        let id = f.st.projects.iter().find(|(_, x)| x.root == p.root).map(|(i, _)| i.as_str()).unwrap_or("");
        let mut r = row(&format!("New thread in {}", p.title), "You can start it empty and write in it after setup, or give it a first message.", vec![btn("Start the thread", "ob-thread", id, !done)]);
        r["fields"] = json!([{"name": "obfirst", "hint": "First message", "secret": false, "value": draft(session, "@f.obfirst")}]);
        rows.push(r);
    } else { rows.push(row("New thread", "Add a project first.", vec![btn("Go to the project step", "ob-go", "project", false)])); }
    if done { rows.push(row("Threads", &format!("{n} so far."), vec![])); }
    Step { id: "thread", group: "First project", title: "Your first thread".into(), body, rows, choices: vec![], panel: "", done, task: true }
}

fn tourstep(id: &'static str, title: &str, body: String, rows: Vec<Value>) -> Step { Step { id, group: "Tour", title: title.into(), body, rows, choices: vec![], panel: "", done: true, task: false } }
fn key(label: &str, keys: &str) -> Value { row(label, keys, vec![]) }

fn tour(f: &Facts, c: &Client) -> Vec<Step> {
    let m = modkey(c, &f.os);
    if c.phone {
        return vec![
            tourstep("viewers", "Viewers", "Open a thread and use its viewer menu. It shows the board, schematic, 3D model, mechanical parts and files of that project. Drag to turn a 3D model and pinch to zoom.".into(), vec![]),
            tourstep("threadtour", "Working in a thread", "Settle takes a thread out of the active list and Snooze hides it until later. Fork starts a new thread from this one. Pin, archive and delete are in the same menu.".into(), vec![]),
            tourstep("bots", "Bots", "A bot is an agent that stays between chats, with a name, a cat and a memory. Add one under Bots, then mention it in any message: @scout look at the BOM.".into(), vec![]),
            tourstep("space", "A bot's space", "Each bot has a Space tab, a page the bot writes for you. It can hold a status, a table, buttons and fields to fill in. Ask a bot to make one.".into(), vec![]),
            tourstep("rooms", "Rooms", "A room is a shared chat for you and your bots. Make one under Bots, pick who joins and post in it. Each bot reads the room and answers there.".into(), vec![]),
            tourstep("routines", "Routines", "A routine runs a bot on a schedule. Open a bot, then Routines, and write what it should do and when, for example tomorrow 9am or monday 09:00.".into(), vec![]),
            tourstep("settings", "Settings", "Settings holds this phone's choices, your notifications and Hubs, the hubs it is paired with. The hub's own settings are on a computer.".into(), vec![]),
        ]
    }
    vec![
        tourstep("viewers", "Boards and schematics", "The rail beside a thread opens its project in a viewer. Backplane draws boards and schematics itself, so they open without KiCad running, and the view updates when an agent changes a file. Press a letter while the keyboard is outside the message box. The history bar under a view steps through each saved version.".into(),
            vec![key("Board", "B"), key("Schematic", "S"), key("Schematic and board side by side", "V"), key("Files", "F"), key("Diff of the thread's changes", "D"), key("Review", "R")]),
        tourstep("model3d", "3D view", "Press 3 for the board in 3D, with its parts. Drag to turn it in any direction and use the wheel to zoom at the pointer. A right drag, a middle drag or Shift with a drag moves it. The view redraws only while you move it, so an open 3D view uses no processor time at rest.".into(),
            vec![key("3D model", "3")]),
        tourstep("mech", "Mechanical parts", "Press M for the Mech view. It lists every STEP file in the project's mech folder, such as an enclosure, a bracket or the board in a fit check, and draws the one you pick in 3D with the same controls. Agents write each part as a FreeCAD script beside its STEP, so FreeCAD has to be installed for them to change one.".into(),
            vec![key("Mechanical parts", "M")]),
        tourstep("browser", "Browser", "Agents have a web browser on the hub's machine for datasheets, part pages and web tools. Press W to watch it. A bot's browser is also its Browser tab. This build shows the page the agent has open, and you cannot click in it yet.".into(),
            vec![key("Browser", "W")]),
        tourstep("threadtour", "Working in a thread", format!("Settle takes a thread out of the active list and Snooze hides it until later. Fork starts a new thread from this one. Runs of tool calls fold into one row, and delegated work shows under Subagents. {m}+Shift+S settles and Ctrl+. stops a running turn."), vec![]),
        tourstep("bots", "Bots", "A bot is an agent that stays between chats, with a name, a cat and a memory. Bots work across all your projects. Mention one in any message: @scout look at the BOM. Each bot has tabs for its chat, its space, its browser, its tasks, its memory, its routines and its webhooks.".into(),
            vec![row("Make a bot", "Opens the form in the sidebar.", vec![btn("New bot", "ob-try", "bot", false)])]),
        tourstep("space", "A bot's space", "Each bot has a Space tab, a page the bot writes for you with its space_set tool. It can hold a status, a table, buttons and fields to fill in, for example a parts list with an Order button. Ask the bot to build one: make me a space that tracks the open BOM questions. A new bot has an empty space.".into(), vec![]),
        tourstep("rooms", "Rooms", "A room is a shared chat for you and your bots. Pick who joins, then post. Each bot reads the room and answers there, so two bots can review each other's work while you watch. Rooms work across linked machines too.".into(),
            vec![row("Make a room", "Opens the form in the sidebar.", vec![btn("New room", "ob-try", "room", false)])]),
        tourstep("routines", "Routines", "A routine runs a bot on a schedule. Open a bot, then its Routines tab, and write what it should do and when. Backplane reads times like tomorrow 9am, monday 09:00, in 20m or 2026-10-14 09:00. Routines follow your time zone, which Settings, Time zone shows. Reminders use the same list.".into(), vec![]),
        tourstep("palette", "Palette and shortcuts", format!("{m}+K opens the palette. Type part of a thread's name or a command, then press Enter. Settings, This device lists every shortcut and lets you change them."),
            vec![key("Palette", &format!("{m}+K")), key("New thread", &format!("{m}+N")), key("Sidebar", &format!("{m}+B")), key("Search threads", &format!("{m}+Shift+F")), key("Agent, model and effort", "Ctrl+M"), key("Previous and next thread", &format!("{m}+Shift+[ and {m}+Shift+]")), key("Uberbot chat", &format!("{m}+Shift+U")),
                row("Try it", "Closes setup and opens the palette. Finish setup in the sidebar brings you back.", vec![btn("Open the palette", "ob-try", "palette", false)])]),
        tourstep("settings", "Settings", format!("Settings holds the choices for the whole hub. This device holds the choices for this window only. The pages are {}. Accounts is where you add more Claude and Codex accounts, and Hubs switches this window to another hub.", super::settings::PAGES.iter().map(|(_, l)| *l).collect::<Vec<_>>().join(", ")),
            vec![row("Try it", "Closes setup and opens Settings. Finish setup in the sidebar brings you back.", vec![btn("Open Settings", "ob-try", "settings", false)])]),
    ]
}

fn uber(f: &Facts, c: &Client) -> Step {
    let id = set(f.st, "uber.bot");
    let name = f.st.bots_nav.get(id).map(|b| b.name.clone()).unwrap_or_default();
    let done = !name.is_empty();
    let mine: Vec<(&String, &crate::state::BotNav)> = f.st.bots_nav.iter().filter(|(i, _)| !i.contains('~')).collect();
    let where_ = if c.phone { "the list" } else { "the sidebar" };
    let (body, mut choices) = if done { (format!("{name} is your uberbot. Open it at the top of {where_} for the status of everything: what runs, what waits for you and what failed."), vec![]) }
        else if let Some((bid, b)) = mine.iter().min_by_key(|(_, b)| b.created) { (format!("Your uberbot is the bot you talk to first. It sees each project, thread, bot and machine, and its Status tab shows what runs and what waits for you. Make one of your bots the uberbot. You can change it later."), vec![choice(&format!("Make {} my uberbot", b.name), "ob-uber", bid)]) }
        else { ("Your uberbot is the bot you talk to first. It sees each project, thread, bot and machine, and its Status tab shows what runs and what waits for you. Create it now and rename it later.".to_string(), vec![choice("Create my uberbot", "ob-uber", "")]) };
    if done { choices.clear(); }
    Step { id: "uber", group: "Tour", title: "Your uberbot".into(), body, rows: vec![], choices, panel: "", done, task: true }
}

fn finish(marks: &[(String, String, Mark)]) -> Step {
    let open: Vec<&(String, String, Mark)> = marks.iter().filter(|(id, _, m)| *m != Mark::Done && id != "done").collect();
    let body = if open.is_empty() { "Everything is set up. Settings, Setup runs this again at any time.".to_string() } else { "These steps are still open. Go to one now, or press Done and return to them in Settings, Setup.".to_string() };
    let rows = open.iter().map(|(id, title, m)| row(title, if *m == Mark::Skipped { "Skipped." } else { "Not done." }, vec![btn("Go", "ob-go", id, false)])).collect();
    Step { id: "done", group: "Tour", title: "Done".into(), body, rows, choices: vec![], panel: "", done: true, task: false }
}

fn all(f: &Facts, st: &State, session: &Value, c: &Client, user_home: &str) -> Vec<Step> {
    let mut v = vec![welcome(f, c)];
    if !c.phone { v.push(install(f)); v.push(home(f, c)); }
    v.extend(import(f, c));
    v.push(usage(f));
    v.extend([claude(f, c), codex(f, c), others(f, c), kicad(f, st, session, c), freecad(f, st, session, c), machine(f, session, c), phone(f, session, c), remote(f, c), project(f, session, c, user_home), thread(f, session, c)]);
    v.extend(tour(f, c));
    v.push(uber(f, c));
    v
}

fn mark(st: &State, x: &Step) -> Mark {
    if x.task || !x.done { if x.done { Mark::Done } else if list(set(st, "onboard.skipped")).contains(&x.id) { Mark::Skipped } else { Mark::Todo } }
    else if list(set(st, "onboard.seen")).contains(&x.id) { Mark::Done } else { Mark::Todo }
}

/// Whether setup is on screen for this view. It comes up by itself on a home with nothing in it, or one where it already began; a home with projects
/// that never saw it (one that came from before setup existed) is left alone and Settings offers it.
pub fn due(st: &State, session: &Value) -> bool {
    if draft(session, "@ob.closed") == "1" { return false }
    if draft(session, "@ob.open") == "1" { return true }
    if set(st, "onboard.done") == "1" { return false }
    !set(st, "onboard.step").is_empty() || !st.projects.values().any(|p| !p.removed)
}

/// The sidebar's Finish setup row, while setup waits: closed for now, begun, not finished.
pub fn resume(st: &State, session: &Value) -> Value {
    if due(st, session) || set(st, "onboard.done") == "1" { return Value::Null }
    if draft(session, "@ob.closed") != "1" && set(st, "onboard.step").is_empty() { return Value::Null }
    json!({"label": "Finish setup", "action": "ob-open", "value": "resume"})
}

fn step_id<'a>(st: &'a State, session: &'a Value) -> &'a str { let d = draft(session, "@ob.step"); if d.is_empty() { set(st, "onboard.step") } else { d } }

/// What a client draws while setup is up, None when it is not.
pub fn view(st: &State, session: &Value, info: &Value, c: &Client, user_home: &str) -> Option<Value> {
    if !due(st, session) { return None }
    Some(page(st, session, info, c, user_home))
}

/// The step on screen as the phones decode it (`Tour`), with the rail and the rows the desktop draws beside that.
pub fn page(st: &State, session: &Value, info: &Value, c: &Client, user_home: &str) -> Value {
    let f = Facts::of(st, info);
    let mut steps = all(&f, st, session, c, user_home);
    let mut marks: Vec<(String, String, Mark)> = steps.iter().map(|x| (x.id.to_string(), x.title.clone(), mark(st, x))).collect();
    steps.push(finish(&marks)); marks.push(("done".into(), "Done".into(), Mark::Todo));
    let id = step_id(st, session);
    let i = steps.iter().position(|x| x.id == id).unwrap_or(0);
    let x = &steps[i]; let last = i + 1 == steps.len();
    let next = if last { "Done" } else if x.task && !x.done { "Skip step" } else { "Next" };
    let rail: Vec<Value> = steps.iter().zip(marks.iter()).map(|(x, m)| json!({"id": x.id, "title": x.title, "group": x.group, "state": m.2.word()})).collect();
    let done_n = marks.iter().filter(|m| m.2 == Mark::Done).count();
    let mut body = x.body.clone();
    if c.phone { for r in &x.rows { let (l, n) = (s(r, "label"), s(r, "note")); if !n.is_empty() && r["buttons"].as_array().map_or(true, Vec::is_empty) { body.push_str(&format!("\n{l}: {n}")); } } }
    json!({"id": x.id, "step": i, "count": steps.len(), "counter": format!("{} of {}", i + 1, steps.len()), "title": x.title, "body": body, "next": next, "back": "Back", "skip": "Finish later", "first": i == 0, "off": "",
        "end": "End setup", "group": x.group, "panel": x.panel, "rows": x.rows, "choices": x.choices, "steps": rail, "state": mark(st, x).word(), "progress": format!("{done_n} of {} done", marks.len() - 1)})
}

/// The rows of Settings, Setup: where it stands, and the way back in.
pub fn settings_rows(st: &State, session: &Value, info: &Value, c: &Client, user_home: &str) -> Vec<Value> {
    let p = page(st, session, info, c, user_home);
    let done = set(st, "onboard.done") == "1";
    let mut v = vec![if done { row("Setup is finished", &format!("{}. Run it again to check what changed.", s(&p, "progress")), vec![btn("Run setup again", "ob-open", "start", true)]) }
        else if !set(st, "onboard.step").is_empty() { row("Setup is not finished", &s(&p, "progress").to_string(), vec![btn("Resume setup", "ob-open", "resume", true), btn("Start again", "ob-open", "start", false)]) }
        else { row("Setup", "Checks what you have, helps you add what is missing and tours each feature.", vec![btn("Run setup", "ob-open", "start", true)]) }];
    for x in p["steps"].as_array().into_iter().flatten().filter(|x| s(x, "id") != "done") {
        let word = match s(x, "state") { "done" => "Done", "skipped" => "Skipped", _ => "Not done" };
        v.push(row(s(x, "title"), word, vec![btn("Go", "ob-go", s(x, "id"), false)]));
    }
    v
}

fn send(hub: &str, rid: &mut u64, cmds: &mut Vec<Value>, m: &str, p: Value) { cmds.push(json!({"type": "send", "hub": hub, "text": json!({"id": *rid, "m": m, "p": p}).to_string()})); *rid += 1; }
fn put(hub: &str, rid: &mut u64, cmds: &mut Vec<Value>, k: &str, v: &str) { send(hub, rid, cmds, "setting.set", json!({"key": k, "value": v})); }
fn with(v: &str, id: &str) -> String { let mut l = list(v); if !l.contains(&id) { l.push(id); } l.join(",") }

/// A press in setup. None for what is not setup's.
/// Whether this press ended setup for good (End setup, or Done on the last step): its patch empties the step.
pub fn ended(action: &str, patch: &Value) -> bool { matches!(action, "ob-next" | "ob-end") && patch["drafts"]["@ob.step"] == "" && patch["drafts"]["@ob.open"] == "" }
/// Where the person lands when setup ends with nothing open: the newest thread of a project that is still there (never a bot's chat, an archived or a deleted one).
pub fn land(st: &State) -> Option<String> {
    st.threads.iter().filter(|(_, t)| !t.deleted && !t.archived && st.projects.get(&t.project).map_or(false, |p| !p.removed))
        .max_by_key(|(id, t)| (t.activity.max(t.created), (*id).clone())).map(|(id, _)| id.clone())
}

pub fn act(st: &State, hub: &str, session: &Value, info: &Value, c: &Client, user_home: &str, action: &str, value: &str) -> Option<Out> {
    if !action.starts_with("ob-") && action != "bfield" { return None }
    if action == "bfield" {
        let (name, text) = value.split_once('\u{1f}')?;
        if !matches!(name, "obkicad" | "obfreecad" | "obproject" | "objoin" | "obfirst") { return None }
        return Some(Out { patch: json!({"drafts": {format!("@f.{name}"): text}}), cmds: vec![] })
    }
    let mut rid = session["rid"].as_u64().unwrap_or(1);
    let mut cmds = vec![];
    let f = Facts::of(st, info);
    let mut steps = all(&f, st, session, c, user_home);
    let marks: Vec<(String, String, Mark)> = steps.iter().map(|x| (x.id.to_string(), x.title.clone(), mark(st, x))).collect();
    steps.push(finish(&marks));
    let cur = step_id(st, session).to_string();
    let i = steps.iter().position(|x| x.id == cur).unwrap_or(0);
    let go = |id: &str, hub: &str, rid: &mut u64, cmds: &mut Vec<Value>| -> Value {
        put(hub, rid, cmds, "onboard.step", id); send(hub, rid, cmds, "onboard.check", json!({}));
        json!({"drafts": {"@ob.step": id, "@ob.project.err": "", "@invite": Value::Null}})
    };
    let mut patch = match action {
        "ob-next" => {
            let x = &steps[i];
            if i + 1 == steps.len() {
                // finishing setup with the telemetry step behind it is the disclosure: recorded before setup is marked done (settings/telemetry.rs)
                send(hub, &mut rid, &mut cmds, "telemetry.notice.act", json!({"action": "setup"})); put(hub, &mut rid, &mut cmds, "onboard.done", "1"); put(hub, &mut rid, &mut cmds, "onboard.step", "");
                json!({"drafts": {"@ob.step": "", "@ob.open": "", "@ob.closed": ""}})
            } else {
                if x.task && !x.done { put(hub, &mut rid, &mut cmds, "onboard.skipped", &with(set(st, "onboard.skipped"), x.id)); }
                else if !x.task { put(hub, &mut rid, &mut cmds, "onboard.seen", &with(set(st, "onboard.seen"), x.id)); }
                go(steps[i + 1].id, hub, &mut rid, &mut cmds)
            }
        }
        "ob-back" => go(steps[i.saturating_sub(1)].id, hub, &mut rid, &mut cmds),
        "ob-go" => { if !steps.iter().any(|x| x.id == value) { return None } let mut p = go(value, hub, &mut rid, &mut cmds); p["drafts"]["@ob.open"] = json!("1"); p["drafts"]["@ob.closed"] = json!(""); p["open"] = json!({"settings": false}); p }
        // Finish later (the phones' dismissal too): closed here until the next launch, the step kept
        "ob-skip" => { put(hub, &mut rid, &mut cmds, "onboard.step", steps[i].id); json!({"drafts": {"@ob.closed": "1", "@ob.open": ""}}) }
        "ob-end" => { send(hub, &mut rid, &mut cmds, "telemetry.notice.act", json!({"action": "setup"})); put(hub, &mut rid, &mut cmds, "onboard.done", "1"); put(hub, &mut rid, &mut cmds, "onboard.step", ""); json!({"drafts": {"@ob.closed": "", "@ob.open": "", "@ob.step": ""}}) }
        // from Settings or the sidebar: "start" at the first step, anything else where it was left
        "ob-open" => {
            let first = if value == "start" { steps[0].id.to_string() } else { cur.clone() };
            if value == "start" { put(hub, &mut rid, &mut cmds, "onboard.step", &first); }
            send(hub, &mut rid, &mut cmds, "onboard.check", json!({}));
            json!({"drafts": {"@ob.open": "1", "@ob.closed": "", "@ob.step": first}, "open": {"settings": false}})
        }
        // Settings, Developer: setup's progress forgotten, opened at the first step as on a new hub
        "ob-replay" => {
            let first = steps[0].id;
            for (k, v) in [("onboard.done", ""), ("onboard.skipped", ""), ("onboard.seen", ""), ("onboard.kicad.auto", ""), ("onboard.step", first)] { put(hub, &mut rid, &mut cmds, k, v); }
            send(hub, &mut rid, &mut cmds, "onboard.check", json!({}));
            json!({"drafts": {"@ob.open": "1", "@ob.closed": "", "@ob.step": first}, "open": {"settings": false}})
        }
        "ob-install" => { if value.is_empty() { return None } send(hub, &mut rid, &mut cmds, "onboard.install", json!({"tool": value})); json!({}) }
        "ob-kicad-install" => { send(hub, &mut rid, &mut cmds, "kicad.install", json!({"consent": true})); json!({}) }
        "ob-check" => { send(hub, &mut rid, &mut cmds, "onboard.check", json!({})); json!({}) }
        "ob-import" => {
            if let Some(at) = value.strip_prefix("ack:") { put(hub, &mut rid, &mut cmds, super::super::import014::ACK, at); return Some(Out { patch: json!({}), cmds }) }
            if value == "later" { return Some(Out { patch: json!({"drafts": {"@import.later": "1"}}), cmds }) }
            let (op, secrets) = match value { "scan" => ("scan", false), "run" => ("run", false), "run-secrets" => ("run", true), "retry" => ("run", import_info(info).map_or(false, |v| v["secrets"] == true)), _ => return None };
            send(hub, &mut rid, &mut cmds, "import.v014", json!({"op": op, "secrets": secrets}));
            json!({"drafts": {"@import.later": ""}})
        }
        "ob-usage" => {
            if value != "on" && value != "off" { return None }
            put(hub, &mut rid, &mut cmds, "onboard.usage", value);
            if value == "off" { put(hub, &mut rid, &mut cmds, "telemetry.share", "off"); }
            // On after an Off is the person's own recorded choice, bound to the preferences on screen (setting.set cannot re-enable)
            else if crate::settings::telemetry::off(set(st, "telemetry.share"), set(st, "telemetry.usage.asked"), "") {
                let t = crate::settings::telemetry::of(st); let d = format!("ob-{}-{}", t.revision_n(), rid);
                send(hub, &mut rid, &mut cmds, "telemetry.choice", json!({"value": "on", "expected": t.token(), "decision": d, "popup": false}));
            }
            json!({})
        }
        "ob-kicad" => { let raw = draft(session, "@f.obkicad").trim().to_string(); put(hub, &mut rid, &mut cmds, "kicad.path", &raw); send(hub, &mut rid, &mut cmds, "onboard.check", json!({"kicad": raw})); json!({}) }
        "ob-freecad" => { let raw = draft(session, "@f.obfreecad").trim().to_string(); put(hub, &mut rid, &mut cmds, "freecad.path", &raw); send(hub, &mut rid, &mut cmds, "onboard.check", json!({"freecad": raw})); json!({}) }
        "ob-project" => {
            let raw = draft(session, "@f.obproject").trim().to_string();
            let path = if raw.is_empty() { format!("{}/Backplane/first-board", user_home.trim_end_matches('/')) } else { raw };
            if !(path.starts_with('/') || path.starts_with('~') || path.chars().nth(1) == Some(':')) { return Some(Out { patch: json!({"drafts": {"@ob.project.err": "Give the full path, starting at the top of the disk."}}), cmds }) }
            send(hub, &mut rid, &mut cmds, "project.new", json!({"path": path, "orAdd": true}));
            json!({"drafts": {"@ob.project.err": "", "@f.obproject": Value::Null}})
        }
        "ob-thread" => {
            let text = draft(session, "@f.obfirst").trim().to_string();
            if !st.projects.contains_key(value) { return None }
            let mut p = json!({"project": value});
            if !text.is_empty() { p["text"] = json!(text); }
            send(hub, &mut rid, &mut cmds, "thread.create", p);
            json!({"drafts": {"@f.obfirst": ""}})
        }
        "ob-uber" => {
            if value.is_empty() { send(hub, &mut rid, &mut cmds, "onboard.uber", json!({})); } else { send(hub, &mut rid, &mut cmds, "bots.uber", json!({"bot": value})); }
            json!({})
        }
        // leave setup to try something; Finish setup in the sidebar comes back here
        "ob-try" => {
            put(hub, &mut rid, &mut cmds, "onboard.step", steps[i].id);
            let mut p = json!({"drafts": {"@ob.closed": "1", "@ob.open": ""}});
            match value { "bot" => { p["drafts"]["@bnew"] = json!("1"); } "room" => { p["drafts"]["@rnew"] = json!("1"); } "settings" => { p["open"] = json!({"settings": true}); } "settings:network" => { p["open"] = json!({"settings": true}); p["drafts"]["@settings.page"] = json!("network"); } _ => {} }
            p
        }
        _ => return None,
    };
    if rid != session["rid"].as_u64().unwrap_or(1) { patch["rid"] = json!(rid); }
    Some(Out { patch, cmds })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn st(kv: &[(&str, &str)]) -> State { let mut s = State::new(); for (k, v) in kv { s.settings.insert(k.to_string(), v.to_string()); } s }
    fn desk() -> Client { Client { phone: false, os: "linux".into(), launcher: true } }
    fn phone_c() -> Client { Client { phone: true, os: String::new(), launcher: false } }
    fn sess(d: Value) -> Value { json!({"rid": 1, "drafts": d, "open": {}}) }
    fn info(os: &str) -> Value { json!({"os": os, "home": "/h/.backplane/hub", "listen": "127.0.0.1:3800", "tailscale": "off"}) }
    /// A state whose checks found `checks`, with these settings.
    fn cst(checks: Value, kv: &[(&str, &str)]) -> State { let mut x = st(kv); x.settings.insert("onboard.checks".into(), checks.to_string()); x }
    fn blank() -> Value { json!({"exe": "/opt/bp/backplane-hub", "onPath": "", "agents": {}, "accounts": {}, "kicad": {}, "freecad": {}, "devices": 0, "tailscale": "off"}) }
    fn s_(v: &Value, k: &str) -> String { s(v, k).to_string() }
    fn ids(v: &Value) -> Vec<String> { v["steps"].as_array().unwrap().iter().map(|x| s(x, "id").to_string()).collect() }
    fn at(st_: &State, id: &str, i: &Value, c: &Client) -> Value { page(st_, &sess(json!({"@ob.step": id})), i, c, "/h") }
    fn setting_of(o: &Out, key: &str) -> Option<String> { o.cmds.iter().filter_map(|c| serde_json::from_str::<Value>(c["text"].as_str()?).ok()).find(|v| v["m"] == "setting.set" && v["p"]["key"] == key).map(|v| s(&v["p"], "value").to_string()) }
    fn sent(o: &Out, m: &str) -> Vec<Value> { o.cmds.iter().filter_map(|c| serde_json::from_str::<Value>(c["text"].as_str()?).ok()).filter(|v| v["m"] == m).map(|v| v["p"].clone()).collect() }

    /// A hub on 127.0.0.1 with Tailscale off makes codes and links that name only this computer: the step says so beside them.
    #[test] fn import_again_says_the_subagent_threads_it_put_under_their_threads() {
        assert_eq!(import_counted(&json!({"lines": 86, "parents": 86})), "86 subagent threads under the threads that started them.");
        assert_eq!(import_counted(&json!({"lines": 101, "parents": 86, "settings": 15})), "15 settings and 86 subagent threads under the threads that started them.");
        assert_eq!(import_counted(&json!({"projects": 1, "threads": 3, "messages": 9, "parents": 1})), "1 project and 3 threads with 9 messages.", "a first import counts the threads alone");
    }
    #[test] fn codes_and_links_that_name_only_this_computer_carry_a_warning() {
        let warned = |v: &Step| v.rows.iter().any(|r| r["label"] == "Only this computer" && r["tone"] == "warn");
        let code = |addr: &str| json!({"linked": [], "found": [], "requests": [], "outgoing": [], "code": {"code": "123456", "address": addr, "left": 590, "peer": "p"}});
        let mut i = info("linux");
        i["machines"] = code("http://127.0.0.1:3800");
        assert!(warned(&machine(&Facts::of(&st(&[]), &i), &sess(json!({})), &desk())), "a loopback machine code");
        i["machines"] = code("http://desktop.tail.ts.net:3800");
        assert!(!warned(&machine(&Facts::of(&st(&[]), &i), &sess(json!({})), &desk())), "a tailnet one");
        let link = |local: bool| sess(json!({"@invite": json!({"link": "http://127.0.0.1:3800/pair#invite=x", "code": "ABCD2345", "qr": "", "localOnly": local}).to_string()}));
        assert!(warned(&phone(&Facts::of(&st(&[]), &info("linux")), &link(true), &desk())), "a loopback device link");
        assert!(!warned(&phone(&Facts::of(&st(&[]), &info("linux")), &link(false), &desk())), "a reachable one");
    }
    /// The machine step is the Machines page: pair by code and approval, never an invite that links by itself.
    #[test] fn the_machine_step_pairs_with_codes_and_approval_like_settings_machines() {
        let (s0, mut i) = (cst(blank(), &[]), info("linux"));
        i["machines"] = json!({"linked": [{"id": "a", "name": "desk", "url": "http://d:3800", "state": "paired"}], "found": [], "requests": [{"id": "r", "name": "lab", "url": "http://l:3800", "check": "482913", "left": 100}], "outgoing": [], "code": null});
        let p = at(&s0, "machine", &i, &desk());
        assert_eq!(p["state"], "done", "a paired machine finishes the step");
        let rows = p["rows"].as_array().unwrap();
        let actions: Vec<&str> = rows.iter().flat_map(|r| r["buttons"].as_array().into_iter().flatten()).map(|b| s(b, "action")).collect();
        for a in ["machine-approve", "machine-deny", "machine-pair", "machine-code"] { assert!(actions.contains(&a), "{a} in {actions:?}"); }
        assert!(!actions.iter().any(|a| a.starts_with("ob-machine")), "no invite that links without approval");
        let body = p["body"].as_str().unwrap();
        assert!(body.contains("approve") && body.contains("never link by themselves") && body.contains("Settings, Hubs"), "{body}");
        let shown = format!("{} {}", p["body"], p["rows"]);
        for stale in ["Bots", "Mine", "invite", "bp1"] { assert!(!shown.contains(stale), "{stale} is stale in the machine step: {shown}"); }
        let blank_i = info("linux");
        assert_eq!(at(&s0, "machine", &blank_i, &desk())["state"], "todo", "no pairing data yet: not done, not a dead end");
        assert!(at(&s0, "machine", &blank_i, &phone_c())["body"].as_str().unwrap().contains("Settings, Machines"));
    }
    /// What the KiCad step does for each computer and each KiCad found: the same decision `install::decide` is tested on, as the person sees it.
    #[test] fn the_kicad_step_installs_offers_or_explains_by_what_was_found() {
        use crate::files::kicad::install::{pick, LINUX};
        let step = |os: &str, arch: &str, status: &str, ins: Option<&str>| {
            let mut i = info(os); i["kicad.status"] = json!(status);
            if status != "missing" { i["kicad.cli"] = json!("/k/bin/kicad-cli"); i["kicad.version"] = json!(if status == "ok" { "10.99.0-bp.4" } else { "10.0.6" }); }
            if let Some(s) = ins { i["kicad.install"] = json!(s); }
            let s0 = cst(blank(), &[]);
            kicad_with(&Facts::of(&s0, &i), &s0, &sess(json!({})), &desk(), pick(os, arch))
        };
        let row = |s: &Step, l: &str| s.rows.iter().find(|r| r["label"].as_str().unwrap().starts_with(l)).cloned().unwrap_or_else(|| panic!("no {l} row in {:?}", s.rows));
        let actions = |s: &Step| -> Vec<String> { s.rows.iter().flat_map(|r| r["buttons"].as_array().into_iter().flatten()).map(|b| s_(b, "action")).collect() };
        // nothing found, a package: the hub starts it, the step shows it start, then progress, then ok with the version
        let a = step("linux", "x86_64", "missing", None); assert!(row(&a, "Backplane KiCad")["note"].as_str().unwrap().contains("Starting the download of Backplane KiCad 10.99.0-bp.4"));
        assert!(!actions(&a).contains(&"ob-kicad-install".to_string()), "no button to press: it starts by itself");
        let r = step("windows", "x86_64", "missing", Some(r#"{"state":"running","version":"10.99.0-bp.6","note":"Unpacking"}"#)); assert_eq!(row(&r, "Installing Backplane KiCad 10.99.0-bp.6")["note"], "Unpacking");
        let ok = step("linux", "x86_64", "ok", Some(r#"{"state":"done","version":"10.99.0-bp.4","note":"x"}"#)); assert!(ok.done); let n = row(&ok, "Backplane KiCad")["note"].as_str().unwrap().to_string(); assert!(n.contains("Status ok") && n.contains("10.99.0-bp.4"), "{n}");
        // failed: a reason and one retry, never an automatic loop
        let bad = step("linux", "x86_64", "missing", Some(r#"{"state":"failed","note":"KiCad install failed: KiCad archive checksum mismatch"}"#)); assert!(actions(&bad).contains(&"ob-kicad-install".to_string()) && row(&bad, "Backplane KiCad")["note"].as_str().unwrap().contains("checksum mismatch"));
        // stock KiCad: works, and one press installs the fork beside it
        let stock = step("macos", "aarch64", "stock", None); assert!(stock.done); let n = row(&stock, "Backplane KiCad")["note"].as_str().unwrap().to_string();
        assert!(n.contains("works for viewing and export") && n.contains("adds editing"), "{n}"); assert!(actions(&stock).contains(&"ob-kicad-install".to_string()));
        // no package: say so, the path field, the stock download
        let none = step("linux", "aarch64", "missing", None); assert!(row(&none, "Backplane KiCad")["note"].as_str().unwrap().contains("No Backplane KiCad is published for this computer"));
        assert!(row(&none, "Locate it")["fields"][0]["name"] == "obkicad" && row(&none, "Stock KiCad").to_string().contains("kicad.org"));
        // the release page is always one press away when an install is not
        for x in [&a, &bad, &stock, &none] { assert!(x.rows.iter().any(|r| r["buttons"].as_array().into_iter().flatten().any(|b| s_(b, "value") == KICAD_RELEASES)), "release page link"); }
        assert!(LINUX.version.contains("bp."));
    }
    /// Every URL a step can show, on each system and for desktop and phone.
    fn all_urls() -> Vec<String> {
        fn walk(v: &Value, out: &mut Vec<String>) { match v { Value::String(t) if t.starts_with("https://") && !t.contains(' ') => out.push(t.clone()), Value::Array(a) => a.iter().for_each(|x| walk(x, out)), Value::Object(o) => o.values().for_each(|x| walk(x, out)), _ => {} } }
        let mut out = vec![];
        for os in ["linux", "darwin", "windows"] { for c in [desk(), phone_c(), Client { phone: true, os: "ios".into(), launcher: false }] { for winget in [false, true] {
            let mut ch = blank(); ch["winget"] = json!(winget);
            let (s0, mut i) = (cst(ch, &[]), info(os)); i["arch"] = json!("x64");
            for id in ids(&at(&s0, "welcome", &i, &c)) { walk(&at(&s0, &id, &i, &c), &mut out); }
            i["kicad.status"] = json!("stock"); walk(&at(&s0, "kicad", &i, &c), &mut out);
        } } }
        out.sort(); out.dedup(); out
    }
    /// Every state the import shows, with every note it can carry.
    fn import_samples() -> Vec<Value> {
        let notes = json!([{"k":"runs","n":12644},{"k":"routines-off","n":3},{"k":"machines"},{"k":"keys-stay"},{"k":"google"},{"k":"clashes","n":1},{"k":"fresh"},{"k":"running"},
            {"k":"bot-folder","name":"Bub","gb":46,"path":"/home/a/.backplane-bend/bots/b1"},{"k":"copy-failed","path":"/x","why":"no space left"},{"k":"sign-in","names":["Work"]},{"k":"accounts-later"},{"k":"vault-failed","why":"its key file is missing"}]);
        let c = json!({"projects":12,"threads":1045,"messages":170677,"bots":6,"memories":1293,"rooms":3,"posts":170,"routines":40,"settings":20,"devices":2,"lines":400000});
        [json!({"state":"found","from":"/home/a/.backplane-bend"}), json!({"state":"scanning","from":"/f"}), json!({"state":"ready","from":"/f","counts":c,"notes":notes}), json!({"state":"ready","from":"/f","counts":{"lines":0},"notes":[]}),
         json!({"state":"running","from":"/f","done":4000,"of":9000}), json!({"state":"running","from":"/f","done":0,"of":0}), json!({"state":"done","from":"/f","counts":c,"notes":notes}), json!({"state":"done","counts":{"lines":0}}),
         json!({"state":"failed","from":"/f","why":"The 0.14 log could not be read: denied."})].into_iter().map(|v| json!({"import.v014": v.to_string()})).collect()
    }
    #[test] fn the_import_card_runs_without_a_close_and_ends_with_the_summary_until_seen() {
        let run = json!({"state":"running","phase":"writing","what":"Writing projects and threads","done":4000,"total":9000,"pct":60,"eta_s":75,"projects":[3,12],"threads":[40,300],"current":"Nordic \u{b7} Pick a regulator"});
        let c = import_card(&run, false, false).unwrap(); assert_eq!(c["buttons"], json!([]), "no way out while it runs"); assert_eq!(c["pct"], 60);
        let lines = c["lines"].to_string(); assert!(lines.contains("3 of 12 projects") && lines.contains("40 of 300 threads") && lines.contains("Now: Nordic") && lines.contains("About 2 min left"), "{lines}");
        let done = json!({"state":"done","at":77,"counts":{"projects":2,"threads":3,"messages":9,"lines":20},"notes":[],"elapsed_s":12,"secrets":false});
        let d = import_card(&done, false, false).unwrap(); assert_eq!(d["body"], "2 projects and 3 threads with 9 messages."); assert_eq!(d["buttons"][0]["value"], "ack:77");
        assert!(import_card(&done, true, false).is_none(), "seen: gone");
        let failed = json!({"state":"failed","from":"/h/.backplane-bend","why":"The 0.14 log could not be read: denied"});
        let f = import_card(&failed, false, false).unwrap(); assert!(f["body"].as_str().unwrap().contains("Your 0.14 data in /h/.backplane-bend is untouched")); assert_eq!(f["buttons"][0]["value"], "retry");
        assert!(import_card(&failed, false, true).is_none(), "put off for now");
        let st = cst(blank(), &[]); let i = json!({"import.v014": done.to_string()});
        let o = act(&st, "h", &sess(json!({})), &i, &desk(), "/h", "ob-import", "ack:77").unwrap();
        let sent: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap(); assert_eq!((sent["p"]["key"].as_str(), sent["p"]["value"].as_str()), (Some("import.v014.ack"), Some("77")));
    }
    #[test] fn the_import_step_shows_only_with_0_14_data_and_each_press_asks_the_hub() {
        let st = cst(blank(), &[]);
        assert!(ids(&at(&st, "welcome", &info("linux"), &desk())).iter().all(|i| i != "import"), "no 0.14 home: no step");
        let mut i = info("linux"); i["import.v014"] = json!(json!({"state":"found","from":"/h/.backplane-bend"}).to_string());
        assert!(ids(&at(&st, "welcome", &i, &desk())).iter().any(|x| x == "import"));
        assert!(ids(&at(&st, "welcome", &i, &phone_c())).iter().all(|x| x != "import"), "a phone does not import");
        for (v, op, secrets) in [("scan", "scan", false), ("run", "run", false), ("run-secrets", "run", true)] {
            let o = act(&st, "h", &sess(json!({})), &i, &desk(), "/h", "ob-import", v).unwrap();
            let sent: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap();
            assert_eq!((sent["m"].as_str(), sent["p"]["op"].as_str(), sent["p"]["secrets"].as_bool()), (Some("import.v014"), Some(op), Some(secrets)));
        }
        assert!(act(&st, "h", &sess(json!({})), &i, &desk(), "/h", "ob-import", "rm").is_none());
        let done = json!({"import.v014": json!({"state":"done","counts":{"projects":2,"threads":3,"messages":9,"lines":20},"notes":[{"k":"runs","n":4}]}).to_string()});
        let r = import_rows(&done); assert_eq!(r[0]["note"], "2 projects and 3 threads with 9 messages."); assert_eq!(r[1]["label"], "Import again"); assert!(r[2]["note"].as_str().unwrap().starts_with("4 mail runs stay in 0.14"));
        assert!(import_note(&json!({"k":"runs","n":1})).1.starts_with("1 mail run stays")); assert!(import_note(&json!({"k":"routines-off","n":1})).1.starts_with("1 routine arrives"));
    }
    /// Everything that lives outside Backplane has a button or a page, on each system, and the phones' links are choices they decode.
    #[test] fn every_step_that_needs_something_outside_backplane_links_or_installs_it() {
        for os in ["linux", "darwin", "windows"] {
            let (s0, mut i) = (cst(blank(), &[]), info(os)); i["arch"] = json!("x64");
            let has = |id: &str, what: &dyn Fn(&Value) -> bool| { let p = at(&s0, id, &i, &desk()); assert!(p["rows"].as_array().unwrap().iter().flat_map(|r| r["buttons"].as_array().into_iter().flatten()).any(|b| what(b)), "{os}: {id} has no link or install button: {p}"); };
            let any = |b: &Value| matches!(s(b, "action"), "open-url" | "ob-install" | "ob-kicad-install");
            for id in ["claude", "codex", "freecad", "machine", "phone"] { has(id, &any); }
            let o = at(&s0, "others", &i, &desk()); assert!(o.to_string().contains("ob-install") == (os != "windows") || o.to_string().contains("ob-install"), "grok installs on {os}");
            has("kicad", &|b| s(b, "value") == KICAD_RELEASES);
        }
        let ob = |os: &str, id: &str, tool: &str| { let mut i = info(os); i["arch"] = json!("x64"); at(&cst(blank(), &[]), id, &i, &desk())["rows"].to_string().contains(&format!("\"ob-install\",\"value\":\"{tool}\"")) };
        for os in ["linux", "darwin", "windows"] { assert!(ob(os, "claude", "claude") && ob(os, "codex", "codex") && ob(os, "others", "grok"), "{os}"); }
        // Windows: winget buttons only when the checks found winget, the download page otherwise
        let w = |winget: bool| { let mut ch = blank(); ch["winget"] = json!(winget); let mut i = info("windows"); i["arch"] = json!("x64"); at(&cst(ch, &[]), "freecad", &i, &desk())["rows"].to_string() };
        assert!(w(true).contains("\"ob-install\",\"value\":\"freecad\"") && w(true).contains("winget")); assert!(!w(false).contains("ob-install") && w(false).contains("freecad.org"));
        // the phones decode choices: Tailscale for their own system, the APK and the iPhone guide
        let phone_ios = Client { phone: true, os: "ios".into(), launcher: false };
        let m = at(&cst(blank(), &[]), "machine", &info("linux"), &phone_ios); let c = m["choices"].as_array().unwrap(); assert_eq!(c.len(), 1); assert_eq!((s(&c[0], "action"), s(&c[0], "value")), ("open-url", TS_IOS));
        let m = at(&cst(blank(), &[]), "machine", &info("linux"), &phone_c()); assert_eq!(m["choices"].as_array().unwrap().len(), 2, "both stores when the app did not say");
        let p = at(&cst(blank(), &[]), "phone", &info("linux"), &phone_c()); assert!(p["choices"].to_string().contains(APK_RELEASES) && p["choices"].to_string().contains(IOS_GUIDE));
        assert!(all_urls().iter().all(|u| u.starts_with("https://")));
    }
    /// Run by hand or in CI with network: `cargo test links_are_live -- --ignored`. Every URL a step shows answers.
    #[test] #[ignore] fn links_are_live() {
        let mut bad = vec![];
        for u in all_urls() {
            let o = std::process::Command::new("curl").args(["-s", "-o", "/dev/null", "-L", "--max-time", "30", "-A", "Mozilla/5.0 (X11; Linux x86_64) Chrome/120", "-w", "%{http_code}", &u]).output().unwrap();
            let code = String::from_utf8_lossy(&o.stdout).to_string(); if !code.starts_with('2') { bad.push(format!("{code} {u}")); }
        }
        assert!(bad.is_empty(), "{bad:?}");
    }
    /// No internal word reaches a new person, and the Settings tour names every Settings page.
    #[test] fn no_internal_words_and_the_settings_step_lists_every_page() {
        let mut every = String::new();
        for c in [desk(), phone_c()] { for os in ["linux", "darwin"] {
            let (s0, i) = (cst(blank(), &[]), info(os));
            for id in ids(&at(&s0, "welcome", &i, &c)) { every.push_str(&at(&s0, &id, &i, &c).to_string()); }
        } }
        for bad in ["Bend", "hub home", "hub's home", "guard", "mirror", "PeerTable", "far thread", "Bots page", "Skip all of setup"] { assert!(!every.contains(bad), "{bad:?} in the steps"); }
        let p = at(&cst(blank(), &[]), "settings", &info("linux"), &desk());
        for (_, label) in super::super::settings::PAGES { assert!(p["body"].as_str().unwrap().contains(label), "{label} missing from the Settings step"); }
        assert_eq!(at(&cst(blank(), &[]), "welcome", &info("linux"), &desk())["choices"].as_array().unwrap().len(), 0, "End setup is the footer's, not a second button");
    }
    #[test] fn it_comes_up_on_a_blank_home_and_leaves_an_old_home_alone() {
        assert!(due(&st(&[]), &sess(json!({}))));
        let mut old = st(&[]); old.apply(&json!({"$":"ProjectCreated","id":"p1","title":"x","root":"/x","at":1}));
        assert!(!due(&old, &sess(json!({}))), "a home with projects that never saw setup is not interrupted");
        let mut begun = old; begun.settings.insert("onboard.step".into(), "kicad".into());
        assert!(due(&begun, &sess(json!({}))), "one where it began resumes at launch");
        assert!(!due(&st(&[("onboard.done", "1")]), &sess(json!({}))));
        assert!(due(&st(&[("onboard.done", "1")]), &sess(json!({"@ob.open": "1"}))), "Settings reopens a finished setup");
        assert!(!due(&st(&[]), &sess(json!({"@ob.closed": "1"}))));
    }
    #[test] fn the_desktop_covers_every_step_in_order_and_the_phone_has_no_install_steps() {
        let d = at(&cst(blank(), &[]), "welcome", &info("linux"), &desk());
        assert_eq!(ids(&d), ["welcome", "install", "home", "usage", "claude", "codex", "others", "kicad", "freecad", "machine", "phone", "remote", "project", "thread", "viewers", "model3d", "mech", "browser", "threadtour", "bots", "space", "rooms", "routines", "palette", "settings", "uber", "done"]);
        let p = at(&cst(blank(), &[]), "welcome", &info("linux"), &phone_c());
        assert!(!ids(&p).contains(&"install".to_string()) && !ids(&p).contains(&"home".to_string()) && !ids(&p).contains(&"palette".to_string()));
        for k in ["step", "count", "counter", "title", "body", "next", "back", "skip", "first", "choices"] { assert!(p.get(k).is_some(), "the phones decode {k}"); }
    }
    #[test] fn next_on_an_unfinished_task_says_skip_step_and_records_it() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let p = at(&s0, "kicad", &i, &desk()); assert_eq!(p["next"], "Skip step"); assert_eq!(p["state"], "todo");
        let o = act(&s0, "h", &sess(json!({"@ob.step": "kicad"})), &i, &desk(), "/h", "ob-next", "").unwrap();
        assert_eq!(setting_of(&o, "onboard.skipped").as_deref(), Some("kicad")); assert_eq!(setting_of(&o, "onboard.step").as_deref(), Some("freecad")); assert_eq!(o.patch["drafts"]["@ob.step"], "freecad");
        let skipped = cst(blank(), &[("onboard.skipped", "kicad")]); assert_eq!(at(&skipped, "kicad", &i, &desk())["state"], "skipped");
    }
    #[test] fn a_step_the_checks_find_done_is_done_and_says_next() {
        let mut c = blank(); c["kicad"] = json!({"path": "/usr/bin/kicad-cli", "version": "10.0.7", "runs": true}); c["agents"]["claude"] = json!({"path": "/x/claude", "version": "2.1", "signed": "yes", "who": "a@b.c"});
        let (i, s0) = (info("linux"), cst(c, &[]));
        let p = at(&s0, "kicad", &i, &desk()); assert_eq!(p["next"], "Next"); assert_eq!(p["state"], "done"); assert!(p["rows"].as_array().unwrap().iter().any(|r| r["label"] == "KiCad found" && r["note"].as_str().unwrap().contains("10.0.7")));
        assert_eq!(at(&s0, "claude", &i, &desk())["state"], "done");
        let mut named = blank(); named["accounts"]["claude"] = json!(["Work"]); assert_eq!(at(&cst(named, &[]), "claude", &i, &desk())["state"], "done", "a connected named account counts");
    }
    #[test] fn a_tool_that_is_found_but_broken_is_not_done_and_says_why() {
        let mut c = blank(); c["freecad"] = json!({"path": "/x/freecadcmd", "runs": false, "why": "exit code 1"});
        let p = at(&cst(c, &[]), "freecad", &info("linux"), &desk());
        assert_eq!(p["state"], "todo"); assert!(p["rows"][0]["note"].as_str().unwrap().contains("exit code 1"));
    }
    #[test] fn install_words_follow_the_hubs_system() {
        // a computer with no published package: stock KiCad's own commands, per system
        for (os, want) in [("linux", "sudo apt install kicad"), ("darwin", "brew install --cask kicad"), ("windows", "winget install --id KiCad.KiCad")] {
            let (s0, mut i) = (cst(blank(), &[]), info(os)); i["kicad.status"] = json!("missing");
            let p = kicad_with(&Facts::of(&s0, &i), &s0, &sess(json!({})), &desk(), None); assert!(Value::Array(p.rows).to_string().contains(want), "{os}");
        }
        assert!(at(&cst(blank(), &[]), "install", &info("darwin"), &desk())["body"].as_str().unwrap().contains("notarized"));
        assert!(at(&cst(blank(), &[]), "install", &info("windows"), &desk())["body"].as_str().unwrap().contains("Windows"));
    }
    #[test] fn every_platform_says_how_to_install_each_tool_and_the_agents_with_a_command() {
        for pf in PLATFORMS {
            assert!(!pf.name.is_empty() && !pf.installed.is_empty() && !pf.path_fix.is_empty() && !pf.kicad.is_empty() && !pf.freecad.is_empty() && !pf.kicad_names.is_empty() && !pf.freecad_names.is_empty(), "{}", pf.id);
            for a in ["claude", "codex", "grok"] { assert!(crate::settings::tools_install::plan(a, pf.id, "x64", false, false).is_some(), "{a} on {}", pf.id); }
        }
        assert_eq!(platform("beos").id, PLATFORMS[0].id, "a system with no entry yet reads the first");
    }
    #[test] fn keys_follow_the_clients_system() {
        let mac = Client { phone: false, os: "darwin".into(), launcher: false };
        assert!(at(&st(&[]), "palette", &info("linux"), &mac).to_string().contains("Cmd+K"));
        assert!(at(&st(&[]), "palette", &info("darwin"), &desk()).to_string().contains("Ctrl+K"));
    }
    #[test] fn finish_later_keeps_the_step_and_end_setup_is_final() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let o = act(&s0, "h", &sess(json!({"@ob.step": "machine"})), &i, &desk(), "/h", "ob-skip", "").unwrap();
        assert_eq!(setting_of(&o, "onboard.step").as_deref(), Some("machine")); assert_eq!(o.patch["drafts"]["@ob.closed"], "1"); assert_eq!(setting_of(&o, "onboard.done"), None);
        let begun = cst(blank(), &[("onboard.step", "machine")]);
        assert_eq!(at(&begun, "", &i, &desk())["id"], "machine", "the next launch opens where it stopped");
        assert_eq!(resume(&begun, &sess(json!({"@ob.closed": "1"})))["action"], "ob-open");
        let e = act(&s0, "h", &sess(json!({})), &i, &desk(), "/h", "ob-end", "").unwrap(); assert_eq!(setting_of(&e, "onboard.done").as_deref(), Some("1"));
    }
    #[test] fn done_ends_it_and_settings_runs_it_again_from_the_start() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let o = act(&s0, "h", &sess(json!({"@ob.step": "done"})), &i, &desk(), "/h", "ob-next", "").unwrap(); assert_eq!(setting_of(&o, "onboard.done").as_deref(), Some("1"));
        let fin = cst(blank(), &[("onboard.done", "1")]);
        assert_eq!(settings_rows(&fin, &sess(json!({})), &i, &desk(), "/h")[0]["buttons"][0]["label"], "Run setup again");
        let r = act(&fin, "h", &sess(json!({})), &i, &desk(), "/h", "ob-open", "start").unwrap();
        assert_eq!(r.patch["drafts"]["@ob.open"], "1"); assert_eq!(r.patch["drafts"]["@ob.step"], "welcome"); assert_eq!(r.patch["open"]["settings"], false);
        assert_eq!(sent(&r, "onboard.check").len(), 1, "a rerun detects what is done again");
    }
    #[test] fn replay_forgets_the_progress_and_opens_the_first_step() {
        let i = info("linux");
        let fin = cst(blank(), &[("onboard.done", "1"), ("onboard.skipped", "kicad"), ("onboard.seen", "palette"), ("onboard.step", "")]);
        let r = act(&fin, "h", &sess(json!({"@ob.closed": "1"})), &i, &desk(), "/h", "ob-replay", "").unwrap();
        for k in ["onboard.done", "onboard.skipped", "onboard.seen"] { assert_eq!(setting_of(&r, k).as_deref(), Some(""), "{k}"); }
        assert_eq!(setting_of(&r, "onboard.step").as_deref(), Some("welcome"));
        assert_eq!((r.patch["drafts"]["@ob.open"].as_str(), r.patch["drafts"]["@ob.closed"].as_str(), r.patch["drafts"]["@ob.step"].as_str()), (Some("1"), Some(""), Some("welcome")));
        assert_eq!(r.patch["open"]["settings"], false); assert_eq!(sent(&r, "onboard.check").len(), 1);
        let fresh = cst(blank(), &[("onboard.done", ""), ("onboard.skipped", ""), ("onboard.seen", ""), ("onboard.step", "welcome")]);
        assert!(due(&fresh, &sess(json!({}))), "every client opens it, as on a new hub");
    }
    #[test] fn ending_setup_names_the_thread_to_land_in() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let o = act(&s0, "h", &sess(json!({"@ob.step": "done"})), &i, &desk(), "/h", "ob-next", "").unwrap(); assert!(ended("ob-next", &o.patch), "Done on the last step");
        let e = act(&s0, "h", &sess(json!({})), &i, &desk(), "/h", "ob-end", "").unwrap(); assert!(ended("ob-end", &e.patch));
        let n = act(&s0, "h", &sess(json!({"@ob.step": "kicad"})), &i, &desk(), "/h", "ob-next", "").unwrap(); assert!(!ended("ob-next", &n.patch), "a step on is not the end");
        let k = act(&s0, "h", &sess(json!({"@ob.step": "kicad"})), &i, &desk(), "/h", "ob-skip", "").unwrap(); assert!(!ended("ob-skip", &k.patch), "Finish later keeps the step");
        assert_eq!(land(&cst(blank(), &[])), None, "no thread, nowhere to land");
    }
    #[test] fn the_last_step_lists_what_is_still_open_with_a_way_there() {
        let p = at(&cst(blank(), &[("onboard.skipped", "kicad")]), "done", &info("linux"), &desk());
        let rows = p["rows"].as_array().unwrap();
        let k = rows.iter().find(|r| r["label"] == "KiCad").unwrap(); assert_eq!(k["note"], "Skipped."); assert_eq!(k["buttons"][0]["action"], "ob-go"); assert_eq!(k["buttons"][0]["value"], "kicad");
    }
    #[test] fn pressing_make_project_makes_the_folder_on_the_hub() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let o = act(&s0, "h", &sess(json!({"@ob.step": "project"})), &i, &desk(), "/home/x", "ob-project", "").unwrap();
        assert_eq!(sent(&o, "project.new")[0]["path"], "/home/x/Backplane/first-board"); assert_eq!(sent(&o, "project.new")[0]["orAdd"], true, "a folder that is already there is added, as the step says");
        let bad = act(&s0, "h", &sess(json!({"@f.obproject": "here"})), &i, &desk(), "/home/x", "ob-project", "").unwrap(); assert!(bad.cmds.is_empty()); assert!(s(&bad.patch["drafts"], "@ob.project.err").contains("full path"));
    }
    #[test] fn a_typed_path_is_used_and_checked() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let o = act(&s0, "h", &sess(json!({"@f.obfreecad": " /opt/fc/freecadcmd "})), &i, &desk(), "/h", "ob-freecad", "").unwrap();
        assert_eq!(setting_of(&o, "freecad.path").as_deref(), Some("/opt/fc/freecadcmd")); assert_eq!(sent(&o, "onboard.check")[0]["freecad"], "/opt/fc/freecadcmd");
    }
    #[test] fn usage_off_writes_the_real_choice_and_nothing_counts_as_answered_before_a_press() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        assert_eq!(at(&s0, "usage", &i, &desk())["state"], "todo");
        let o = act(&s0, "h", &sess(json!({})), &i, &desk(), "/h", "ob-usage", "off").unwrap(); assert_eq!(setting_of(&o, "telemetry.share").as_deref(), Some("off"));
        assert_eq!(at(&cst(blank(), &[("onboard.usage", "on")]), "usage", &i, &desk())["state"], "done");
    }
    #[test] fn the_phone_gets_row_facts_in_its_body_and_the_desktop_keeps_them_as_rows() {
        let p = at(&cst(blank(), &[("onboard.step", "welcome")]), "welcome", &info("linux"), &phone_c()); assert_eq!(p["first"], true);
        let d = at(&cst(blank(), &[]), "home", &info("linux"), &desk()); assert_eq!(d["rows"][0]["note"], "/h/.backplane/hub");
    }

    /// Every word setup can show, on each system and client, against the rules of the unslop skill.
    fn all_text() -> Vec<String> {
        fn walk(v: &Value, out: &mut Vec<String>) { match v { Value::String(t) => out.push(t.clone()), Value::Array(a) => a.iter().for_each(|x| walk(x, out)), Value::Object(o) => for (k, x) in o { if !matches!(k.as_str(), "action" | "value" | "id" | "state" | "group" | "panel" | "qr" | "name") { walk(x, out) } }, _ => {} } }
        let mut out = vec![];
        let mut full = blank(); full["agents"]["claude"] = json!({"path": "/x", "version": "1", "signed": "no"}); full["kicad"] = json!({"path": "/x", "runs": false, "why": "exit code 1"}); full["freecad"] = json!({"path": "/x", "version": "1", "runs": true});
        for os in PLATFORMS.iter().map(|p| p.id) { for c in [desk(), phone_c(), Client { phone: false, os: "darwin".into(), launcher: false }] { for checks in [blank(), full.clone()] {
            let i = info(os); let s0 = cst(checks, &[]);
            for id in ids(&at(&s0, "welcome", &i, &c)) { walk(&at(&s0, &id, &i, &c), &mut out); }
            for r in settings_rows(&s0, &sess(json!({})), &i, &c, "/h") { walk(&r, &mut out); }
        } } }
        for st in import_samples() { walk(&Value::Array(import_rows(&st)), &mut out); if let Some(x) = import(&Facts::of(&cst(blank(), &[]), &st), &desk()) { out.push(x.title); out.push(x.body) } }
        walk(&Value::Array(import_rows(&json!({}))), &mut out);
        let running = json!({"state":"running","phase":"writing","what":"Writing projects and threads","done":4000,"total":9000,"pct":60,"eta_s":75,"projects":[3,12],"threads":[40,300],"current":"Nordic \u{b7} Pick a regulator","from":"/f"});
        for v in [running.clone(), json!({"state":"running","phase":"reading","what":"Reading the 0.14 log","done":1<<20,"total":9<<20,"pct":3,"eta_s":null}), json!({"state":"running","phase":"copying","what":"Copying","done":25,"total":0,"eta_s":5})] { walk(&import_card(&v, false, false).unwrap(), &mut out); }
        for st in import_samples() { let v: Value = serde_json::from_str(st["import.v014"].as_str().unwrap()).unwrap(); if let Some(c) = import_card(&v, false, false) { walk(&c, &mut out); } }
        out.sort(); out.dedup(); out
    }
    #[test] fn the_update_words_follow_how_the_hub_was_installed() {
        let mut i = info("linux"); i["update"] = json!("package");
        let b = at(&cst(blank(), &[]), "remote", &i, &desk())["body"].as_str().unwrap().to_string();
        assert!(b.contains("run the install command again"), "{b}");
        i["update"] = json!("managed");
        let b = at(&cst(blank(), &[("update.channel", "beta")]), "remote", &i, &desk())["body"].as_str().unwrap().to_string();
        assert!(b.contains("every hour on the beta channel") && b.contains("health check"), "{b}");
    }
    #[test] fn every_word_follows_unslop() {
        const BANNED: &[&str] = &["additionally", "crucial", "delve", "enduring", "enhance", "fostering", "garner", "interplay", "intricate", "landscape", "pivotal", "showcase", "tapestry", "testament", "underscore", "vibrant", "serves as", "stands as", "boasts", "not just", "not only",
            "in order to", "due to the fact", "it is important to note", "utilize", "leverage", "facilitate", "numerous", "in the event that", "seamless", "robust", "powerful", "simply", "easily", "effortless", "great question", "i hope", "let me know", "of course", "certainly", "substrate", "north star", "flywheel", "paradigm", "unlock", "supercharge"];
        for t in all_text() {
            let l = t.to_lowercase();
            for b in BANNED { assert!(!l.contains(b), "{b:?} in {t:?}"); }
            assert!(!t.contains(['\u{2014}', '\u{2013}', '\u{201c}', '\u{201d}', '\u{2018}', '\u{2019}']), "a dash or curly quote in {t:?}");
            assert!(!t.contains(" - ") && !t.contains("->"), "a hyphen used as a dash, or an arrow, in {t:?}");
            if !t.contains("://") && !t.contains("--") { assert!(!(t.contains(" (") && t.contains(')')), "parentheses in {t:?}"); }
            assert!(!t.contains('!'), "an exclamation in {t:?}");
        }
    }
    #[test] fn sentences_stay_short() {
        for t in all_text() { for sent in t.split(". ") { assert!(sent.split_whitespace().count() <= 40, "{sent:?}"); } }
    }
}
