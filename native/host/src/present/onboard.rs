//! The first-run setup on a computer: five steps (Welcome, Agents, Design tools, Where your uberbot lives, First project), then Done. Phones have none:
//! they pair and open the uberbot (docs/ALWAYS-ON.md, D9). Pure: `view` turns the state, the
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

/// Who is asking. `phone`: an app, which has no setup (it only pairs). The hub's own `os` decides the install words.
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
impl Mark { pub fn word(self) -> &'static str { match self { Mark::Done => "done", Mark::Skipped => "skipped", Mark::Todo => "todo" } } }

/// One step as drawn. `task`: it has something to finish, so Next reads Skip step until it is done. `accept`: Next takes what is preselected
/// (step 4's Stay on this computer), so it reads Next. `group` is drawn above the step in the rail when it changes; five steps need none.
pub struct Step { pub id: &'static str, pub group: &'static str, pub title: String, pub body: String, pub rows: Vec<Value>, pub choices: Vec<Value>, pub panel: String, pub done: bool, pub task: bool, pub accept: bool }

/// The steps' ids in order. Ids of the old setup (install, home, usage, claude, codex, others, kicad, freecad, machine, phone, remote, thread,
/// the tour's pages, uber) are ignored where they are kept (`onboard.step`, `onboard.skipped`, `onboard.seen`).
pub const STEPS: &[&str] = &["welcome", "agents", "tools", "alwayson", "project", "done"];

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn set<'a>(st: &'a State, k: &str) -> &'a str { st.settings.get(k).map(String::as_str).unwrap_or("") }
pub fn draft<'a>(session: &'a Value, k: &str) -> &'a str { s(&session["drafts"], k) }
fn list(v: &str) -> Vec<&str> { v.split(',').filter(|x| !x.is_empty()).collect() }
pub fn btn(label: &str, action: &str, value: &str, on: bool) -> Value { json!({"label": label, "action": action, "value": value, "on": on}) }
pub fn row(label: &str, note: &str, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "buttons": buttons}) }
pub fn copy(cmd: &str) -> Value { btn("Copy", "copy", cmd, false) }
pub fn link(label: &str, url: &str) -> Value { btn(label, "open-url", url, false) }
pub fn choice(label: &str, action: &str, value: &str) -> Value { json!({"label": label, "action": action, "value": value}) }

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
    Agent { id: "agy", name: "Antigravity", install: &[], guide: "https://antigravity.google", sign_in: "agy, then sign in" },
    Agent { id: "grok", name: "Grok Build", install: &[("linux", "curl -fsSL https://x.ai/cli/install.sh | bash"), ("darwin", "curl -fsSL https://x.ai/cli/install.sh | bash"), ("windows", "irm https://x.ai/cli/install.ps1 | iex")], guide: "https://x.ai/cli", sign_in: "grok login" },
    Agent { id: "muse", name: "Muse Code", install: &[], guide: "https://dev.meta.ai/docs/muse-code/auth", sign_in: "muse login" },
    Agent { id: "opencode", name: "OpenCode", install: &[], guide: "https://opencode.ai", sign_in: "opencode auth login" },
    Agent { id: "pi", name: "Pi", install: &[], guide: "https://pi.dev", sign_in: "pi, then /login" },
    Agent { id: "omp", name: "Oh My Pi", install: &[], guide: "https://github.com/can1357/oh-my-pi", sign_in: "omp, then /login" },
];
fn install_cmd<'a>(a: &'a Agent, os: &str) -> &'a str { a.install.iter().find(|(p, _)| *p == os).map_or("", |(_, c)| *c) }

/// The agents setup lists first; the rest are under More agents.
pub const MAIN_AGENTS: usize = 4;

/// What the checks found, and the state's own answers (step 4's rows read it too: present/onboard_alwayson.rs).
pub struct Facts<'a> { pub st: &'a State, pub info: &'a Value, pub os: String, pub home: String, pub listen: String, pub exe: String, pub on_path: String, pub checks: Value, pub checked: bool, pub tailscale: String, pub peers: usize, pub devices: usize, pub version: String, pub machines: Value, pub installs: Value, pub arch: String, pub update: String }
impl<'a> Facts<'a> {
    pub fn of(st: &'a State, info: &'a Value) -> Facts<'a> {
        let checks: Value = st.settings.get("onboard.checks").and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null);
        let tailscale = { let t = s(&checks, "tailscale"); if t.is_empty() { s(info, "tailscale").to_string() } else { t.to_string() } };
        let peers = st.settings.keys().filter(|k| k.starts_with("peer.own.")).count();
        Facts { st, info, os: { let o = s(info, "os"); if o.is_empty() { "linux".into() } else { o.to_string() } }, home: s(info, "home").into(), listen: s(info, "listen").into(), exe: s(&checks, "exe").into(), on_path: s(&checks, "onPath").into(),
            checked: checks.is_object(), tailscale, peers, devices: checks["devices"].as_u64().unwrap_or(0) as usize, version: super::settings::version().to_string(), machines: info["machines"].clone(), installs: serde_json::from_str(s(info, "onboard.installs")).unwrap_or(Value::Null), arch: s(info, "arch").into(), update: s(info, "update").into(), checks }
    }
    /// The rows for a tool that lives outside Backplane: its Install button (or page), how an install is going, and a retry. `tools_install::plan` decides.
    pub fn install_rows(&self, tool: &str, label: &str) -> Vec<Value> {
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
    pub fn agent(&self, id: &str) -> &Value { &self.checks["agents"][id] }
    pub fn found(&self, id: &str) -> bool { !s(self.agent(id), "path").is_empty() }
    pub fn accounts(&self, id: &str) -> Vec<String> { self.checks["accounts"][id].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect() }
    pub fn signed(&self, id: &str) -> bool { s(self.agent(id), "signed") == "yes" || !self.accounts(id).is_empty() }
    pub fn runs(&self, tool: &str) -> bool { self.checks[tool]["runs"].as_bool().unwrap_or(false) }
    pub fn projects(&self) -> Vec<&crate::state::Project> { self.st.projects.values().filter(|p| !p.removed).collect() }
    pub fn threads(&self) -> usize { let p: Vec<&str> = self.st.projects.iter().filter(|(_, p)| !p.removed).map(|(id, _)| id.as_str()).collect(); self.st.threads.values().filter(|t| !t.deleted && p.contains(&t.project.as_str())).count() }
}

fn looking(f: &Facts) -> &'static str { if f.checked { "" } else { "Checking this machine." } }
fn warn(mut r: Value) -> Value { r["tone"] = json!("warn"); r }

/// A press's requests to the hub, in order, each with the view's next request id.
pub struct Ask<'a> { pub hub: &'a str, pub rid: u64, pub cmds: Vec<Value> }
impl<'a> Ask<'a> {
    pub fn send(&mut self, m: &str, p: Value) { send(self.hub, &mut self.rid, &mut self.cmds, m, p) }
    pub fn put(&mut self, k: &str, v: &str) { put(self.hub, &mut self.rid, &mut self.cmds, k, v) }
}

// ---- the steps ----

fn welcome(f: &Facts, c: &Client) -> Step {
    let body = "Backplane runs coding agents on your boards, schematics and mechanical parts. Five short steps set it up, and each one can be skipped. Finish later closes setup until the next launch.".to_string();
    let mut folder = row("Data folder", &format!("{}. Your threads, settings, bots and keys are kept here, and the hub runs your agents from it.", f.home), vec![]);
    if c.launcher { folder["buttons"] = json!([btn("Choose another", "ob-home", "", false)]); }
    let mut rows = vec![folder, usage_row(f)];
    if let Some(v) = import_info(f.info) {
        let from = s(&v, "from");
        rows.push(row("Backplane 0.14", &if from.is_empty() { "Your 0.14 data was copied into this hub.".to_string() } else { format!("Your 0.14 data is in {from}. This hub can copy its projects, threads, bots and settings. The 0.14 folder is only read.") }, vec![]));
        rows.extend(import_rows(f.info));
    }
    Step { id: "welcome", group: "", title: "Welcome to Backplane".into(), body, rows, choices: vec![], panel: String::new(), done: !f.home.is_empty(), task: false, accept: false }
}

/// The anonymous-usage switch, one row.
fn usage_row(f: &Facts) -> Value {
    let (share, asked, ua) = (set(f.st, "telemetry.share"), set(f.st, "telemetry.asked"), set(f.st, "telemetry.usage.asked"));
    let answered = crate::settings::telemetry::answered(share, asked, ua) || !set(f.st, "onboard.usage").is_empty();
    let on = crate::settings::telemetry::on(share, ua) && set(f.st, "onboard.usage") != "off";
    row("Share anonymous usage", "This build sends nothing. With On, a later build may send daily counts with a random id. Never your conversations, files, names or keys. Settings, Privacy changes it.",
        vec![btn("On", "ob-usage", "on", answered && on), btn("Off", "ob-usage", "off", answered && !on)])
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
/// How a tool outside Backplane gets onto this machine, for its row: a note when an install runs or failed, its buttons, and whether it warns.
/// `tools_install::plan` decides.
fn install_btns(f: &Facts, tool: &str, guide: &str) -> (String, Vec<Value>, bool) {
    use crate::settings::tools_install::{plan, Plan};
    let st = &f.installs[tool];
    let p = plan(tool, &f.os, &f.arch, f.checks["winget"] == true, f.checks["node"] == true);
    let page = |p: &Option<Plan>| match p { Some(Plan::Link(u)) => Some(link("Download", u)), _ => None };
    let fallback = || if guide.is_empty() { vec![] } else { vec![link("Install guide", guide)] };
    match (s(st, "state"), &p) {
        ("running", _) => (format!("Installing. {}", if s(st, "note").is_empty() { "Starting." } else { s(st, "note") }), vec![], false),
        ("failed", _) => { let mut b = vec![btn("Try again", "ob-install", tool, true)]; b.extend(page(&p)); (format!("It did not install: {}.", s(st, "note").trim_end_matches('.')), b, true) }
        (_, Some(Plan::Winget(_))) | (_, Some(Plan::Sh(..))) | (_, Some(Plan::Ps(_))) => (String::new(), vec![btn("Install", "ob-install", tool, true)], false),
        (_, Some(Plan::Link(u))) => (String::new(), vec![link("Download", u)], false),
        (_, None) => (String::new(), fallback(), false),
    }
}

/// One agent's row: Install, Connect, or who it is connected as.
fn agent_row(f: &Facts, a: &Agent) -> Value {
    let c = f.agent(a.id);
    if !f.checked { return row(a.name, looking(f), vec![]) }
    if !f.found(a.id) {
        let (note, b, bad) = install_btns(f, a.id, a.guide);
        let r = row(a.name, if note.is_empty() { "Not installed." } else { &note }, b);
        return if bad { warn(r) } else { r }
    }
    let ver = s(c, "version");
    let at = if ver.is_empty() { "Installed.".to_string() } else { format!("Version {ver}.") };
    let names = f.accounts(a.id);
    match a.id {
        "claude" | "codex" if f.signed(a.id) => {
            let who = if !s(c, "who").is_empty() { s(c, "who").to_string() } else { names.join(", ") };
            row(a.name, &if who.is_empty() { format!("{at} Connected.") } else { format!("{at} Connected as {who}.") }, vec![btn("Add another", "ob-connect", a.id, false)])
        }
        "claude" | "codex" => row(a.name, &format!("{at} Not connected."), vec![btn("Connect", "ob-connect", a.id, true)]),
        // the others sign in with their own command, which Backplane cannot check
        _ => row(a.name, &format!("{at} If it is not signed in yet, run {} in a terminal.", a.sign_in), vec![copy(a.sign_in.split(',').next().unwrap_or(a.sign_in))]),
    }
}

fn agents(f: &Facts, session: &Value) -> Step {
    let body = "Backplane runs these agents on this computer. You need one. Install it here and connect your account. Claude and Codex sign in from here. The others sign in with their own command.".to_string();
    let more = draft(session, "@ob.more") == "1";
    let mut rows: Vec<Value> = AGENTS[..MAIN_AGENTS].iter().map(|a| agent_row(f, a)).collect();
    let rest: Vec<&str> = AGENTS[MAIN_AGENTS..].iter().map(|a| a.name).collect();
    rows.push(row("More agents", &format!("{} and {}.", rest[..rest.len() - 1].join(", "), rest[rest.len() - 1]), vec![btn(if more { "Hide" } else { "Show" }, "ob-more", "", false)]));
    if more { rows.extend(AGENTS[MAIN_AGENTS..].iter().map(|a| agent_row(f, a))); }
    rows.push(row("Check again", "After you install or sign in somewhere else.", vec![btn("Check again", "ob-check", "", false)]));
    // Connect opens that provider's accounts panel under the list
    let panel = match draft(session, "@ob.panel") { p @ ("claude" | "codex") => p.to_string(), _ => String::new() };
    let done = f.signed("claude") || f.signed("codex") || AGENTS[2..].iter().any(|a| f.found(a.id));
    Step { id: "agents", group: "", title: "Agents".into(), body, rows, choices: vec![], panel, done, task: true, accept: false }
}

/// Where Backplane KiCad's releases are: the fallback when the install cannot run here.
pub const KICAD_RELEASES: &str = "https://github.com/i2cjak/Backplane_KiCad/releases";
pub const APK_RELEASES: &str = "https://github.com/BackplaneWorks/Backplane/releases/latest";
pub const IOS_GUIDE: &str = "https://github.com/BackplaneWorks/Backplane/blob/main/docs/install.md#phones";
pub const KICAD_STOCK: &str = "https://www.kicad.org/download/";

/// The KiCad row for a computer with this package (None: none is published for it). What happens is `install::decide`'s; the hub starts the
/// `Auto` install itself (`settings/onboard.rs`), this only says how it goes: progress from info `kicad.install`, status and version from the checks.
fn kicad_row(f: &Facts, pkg: Option<&crate::files::kicad::install::Package>) -> (Value, bool) {
    use crate::files::kicad::install::{decide, Action};
    let i = |k: &str| s(f.info, k);
    let (status, version) = (i("kicad.status"), i("kicad.version"));
    let prog: Value = serde_json::from_str(i("kicad.install")).unwrap_or_default();
    let (pstate, note) = (s(&prog, "state"), s(&prog, "note"));
    let action = decide(pkg, status, pstate);
    let done = f.runs("kicad") || matches!(status, "ok" | "stock" | "old");
    let v = |v: &str| if v.is_empty() { String::new() } else { format!(" {v}") };
    let found = if !i("kicad.cli").is_empty() { format!("{}{} at {}.", if status == "stock" { "Stock KiCad" } else { "KiCad" }, v(version), i("kicad.cli")) }
        else if f.runs("kicad") { format!("KiCad{} at {}.", v(s(&f.checks["kicad"], "version")), s(&f.checks["kicad"], "path")) }
        else if !s(&f.checks["kicad"], "path").is_empty() { format!("Found at {}, but it does not run: {}.", s(&f.checks["kicad"], "path"), s(&f.checks["kicad"], "why")) }
        else { String::new() };
    let rel = link("Releases", KICAD_RELEASES);
    let r = match action {
        Action::Nothing => row("KiCad", &format!("Backplane KiCad{} at {}. Agents can edit a board in a running KiCad.", v(version), i("kicad.cli")), vec![]),
        Action::Wait if pstate == "running" => row("KiCad", &format!("Installing Backplane KiCad{}. {}", v(s(&prog, "version")), if note.is_empty() { "Starting." } else { note }), vec![]),
        Action::Wait => row("KiCad", looking(f), vec![rel]),
        Action::Auto => row("KiCad", &format!("Not found. Downloading Backplane KiCad{} into Backplane's own folder. This takes a few minutes.", v(pkg.map_or("", |p| p.version))), vec![rel]),
        Action::Offer if pstate == "failed" => warn(row("KiCad", &format!("{}.", note.trim_end_matches('.')), vec![btn("Try again", "ob-kicad-install", "", true), rel])),
        Action::Offer => row("KiCad", &format!("{} {} Backplane KiCad adds editing by agents.", found, offer_lead(status)).trim().replace("  ", " "), vec![btn("Install Backplane KiCad", "ob-kicad-install", "", true), rel]),
        Action::NoPackage => row("KiCad", &if found.is_empty() { format!("{} No Backplane KiCad is published for this computer. Stock KiCad 9 or later works for viewing and export.", if f.checked { "Not found." } else { looking(f) }) } else { found.clone() }.trim().to_string(),
            vec![link("Download KiCad", KICAD_STOCK), rel]),
    };
    (r, done)
}
fn offer_lead(status: &str) -> &'static str {
    match status { "stock" => "It works for viewing and export.", "old" => "It is too old for editing.", _ => "It does not run." }
}
fn freecad_row(f: &Facts) -> (Value, bool) {
    let t = &f.checks["freecad"];
    if f.runs("freecad") { return (row("FreeCAD", &format!("FreeCAD {} at {}.", s(t, "version"), s(t, "path")), vec![]), true) }
    if !f.checked { return (row("FreeCAD", looking(f), vec![]), false) }
    let (note, b, bad) = install_btns(f, "freecad", "");
    let what = if !s(t, "path").is_empty() { format!("Found at {}, but it does not run: {}.", s(t, "path"), s(t, "why")) } else { "Not found. Agents use it to model enclosures and brackets.".to_string() };
    let r = row("FreeCAD", &if note.is_empty() { what } else { note }, b);
    (if bad || !s(t, "path").is_empty() { warn(r) } else { r }, false)
}
/// A Locate row: the full path of a tool found nowhere Backplane looks.
fn locate(st: &State, session: &Value, field: &str, label: &str, hint: &str, setting: &str, action: &str) -> Value {
    let path = if session["drafts"][format!("@f.{field}")].is_string() { draft(session, &format!("@f.{field}")).to_string() } else { set(st, setting).to_string() };
    let mut r = row(label, &format!("If it is installed somewhere else, give the full path to {hint}."), vec![btn("Use this path", action, "", false)]);
    r["fields"] = json!([{"name": field, "hint": format!("Path to {hint}"), "secret": false, "value": path}]);
    r
}
fn tools(f: &Facts, st: &State, session: &Value) -> Step { tools_with(f, st, session, crate::files::kicad::install::package()) }
fn tools_with(f: &Facts, st: &State, session: &Value, pkg: Option<&crate::files::kicad::install::Package>) -> Step {
    let body = "Agents use KiCad for boards and schematics, and FreeCAD for mechanical parts. Skip FreeCAD if you only design boards.".to_string();
    let ((k, kd), (fc, fd)) = (kicad_row(f, pkg), freecad_row(f));
    let mut rows = vec![k];
    if !kd && f.checked { rows.push(locate(st, session, "obkicad", "Locate KiCad", "kicad-cli", "kicad.path", "ob-kicad")); }
    rows.push(fc);
    if !fd && f.checked { rows.push(locate(st, session, "obfreecad", "Locate FreeCAD", "freecadcmd", "freecad.path", "ob-freecad")); }
    Step { id: "tools", group: "", title: "Design tools".into(), body, rows, choices: vec![], panel: String::new(), done: kd && fd, task: true, accept: false }
}

/// Step 4. The frame is this file's; its rows and presses are present/onboard_alwayson.rs's.
fn alwayson(f: &Facts, session: &Value, c: &Client) -> Step {
    let body = "Always-on keeps your uberbot, reminders, bots and phone alerts running while this computer sleeps. Hardware and code still run here.".to_string();
    let rows = super::onboard_alwayson::rows(f, session, c);
    Step { id: "alwayson", group: "", title: "Where your uberbot lives".into(), body, rows, choices: vec![], panel: String::new(), done: super::onboard_alwayson::done(f), task: true, accept: true }
}

/// The warning for a code that names only this computer (the Machines page's code row, `code.address`).
pub const LOCAL_CODE: &str = "This code names only this computer (127.0.0.1): another computer cannot reach it. Start the hub with Tailscale on, or set BACKPLANE_URL to an address the other machine can reach, then make a new code.";

/// Pair a phone: optional, one row (two once the code shows). The phone itself has no setup.
fn phone_rows(session: &Value) -> Vec<Value> {
    let inv: Value = serde_json::from_str(draft(session, "@invite")).unwrap_or(Value::Null);
    let apps = || vec![link("Android app", APK_RELEASES), link("iPhone app", IOS_GUIDE)];
    if let Some(l) = inv["link"].as_str() {
        let mut r = row("Pair a phone", "Scan this with the Backplane app, or type the short code and this address. It works once, for ten minutes. Treat it as a password.", [vec![copy(l), btn("Make another", "device-invite", "", false)], apps()].concat());
        r["qr"] = json!(crate::net::qr::text(l).lines().map(str::to_string).collect::<Vec<_>>());
        r["qrnote"] = json!(format!("Short code {}, for ten minutes", s(&inv, "code")));
        let mut v = vec![r];
        if inv["localOnly"] == true { v.push(warn(row("Only this computer", super::settings::LOCAL_DEVICE, vec![]))); }
        v
    } else if let Some(e) = inv["error"].as_str() { vec![warn(row("Pair a phone", &format!("Backplane could not make a code: {e}."), vec![btn("Try again", "device-invite", "", false)]))] }
    else { vec![row("Pair a phone", "Optional. Get the app, then show the code here and scan it. The phone opens your uberbot.", [vec![btn("Show the code", "device-invite", "", false)], apps()].concat())] }
}

fn project(f: &Facts, session: &Value, user_home: &str) -> Step {
    let ps = f.projects();
    let done = !ps.is_empty();
    let body = "A project is a folder, usually with a KiCad project in it. Give a folder that exists to add it, or a new path to make it. Then tell an agent what you want.".to_string();
    let mut rows = vec![];
    if let Some(p) = ps.iter().max_by_key(|p| p.created) {
        rows.push(row(&p.title, &p.root, vec![]));
        let id = f.st.projects.iter().find(|(_, x)| x.root == p.root).map(|(i, _)| i.as_str()).unwrap_or("");
        let n = f.threads();
        let mut r = row("First message", &if n == 0 { "Starts a thread in this project with it. You can also leave it empty and write after setup.".to_string() } else { format!("{n} {} so far. Another message starts another one.", if n == 1 { "thread" } else { "threads" }) }, vec![btn("Start", "ob-thread", id, n == 0)]);
        r["fields"] = json!([{"name": "obfirst", "hint": "What should the agent do?", "secret": false, "value": draft(session, "@f.obfirst")}]);
        rows.push(r);
    } else {
        let path = if session["drafts"]["@f.obproject"].is_string() { draft(session, "@f.obproject").to_string() } else { format!("{}/Backplane/first-board", user_home.trim_end_matches('/')) };
        let e = draft(session, "@ob.project.err");
        let mut add = row("Folder", if e.is_empty() { "The full path of a folder on this computer." } else { e }, vec![btn("Create", "ob-project", "", true)]);
        add["fields"] = json!([{"name": "obproject", "hint": "Folder", "secret": false, "value": path}]);
        rows.push(add);
    }
    rows.extend(phone_rows(session));
    Step { id: "project", group: "", title: "First project".into(), body, rows, choices: vec![], panel: String::new(), done, task: true, accept: false }
}

/// The uberbot of this home, if it has one.
fn uber_of(st: &State) -> Option<String> { let u = set(st, "uber.bot"); st.bots_nav.contains_key(u).then(|| u.to_string()) }

fn finish(marks: &[(String, String, Mark)]) -> Step {
    let open: Vec<&(String, String, Mark)> = marks.iter().filter(|(id, _, m)| *m != Mark::Done && id != "done").collect();
    let rows = open.iter().map(|(id, title, m)| row(title, if *m == Mark::Skipped { "Skipped." } else { "Not done." }, vec![btn("Go", "ob-go", id, false)])).collect();
    Step { id: "done", group: "", title: "Done".into(), body: "Press ? any time for keys.".into(), rows, choices: vec![choice("Open the uberbot", "ob-uber-open", "")], panel: String::new(), done: true, task: false, accept: false }
}

fn all(f: &Facts, st: &State, session: &Value, c: &Client, user_home: &str) -> Vec<Step> {
    vec![welcome(f, c), agents(f, session), tools(f, st, session), alwayson(f, session, c), project(f, session, user_home)]
}

fn mark(st: &State, x: &Step) -> Mark {
    if x.task || !x.done { if x.done { Mark::Done } else if list(set(st, "onboard.skipped")).contains(&x.id) { Mark::Skipped } else { Mark::Todo } }
    else if list(set(st, "onboard.seen")).contains(&x.id) { Mark::Done } else { Mark::Todo }
}

/// Whether setup is on screen for this view. Never on a phone (D9). It comes up by itself on a home with nothing in it, or one where it already began;
/// a home with projects that never saw it (one that came from before setup existed) is left alone and Settings offers it.
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
/// The steps with Done after them, their marks, and where the view is: the step it names, or (an old setup's id, or none) the first not done.
fn walk(f: &Facts, st: &State, session: &Value, c: &Client, user_home: &str) -> (Vec<Step>, Vec<(String, String, Mark)>, usize) {
    let mut steps = all(f, st, session, c, user_home);
    let mut marks: Vec<(String, String, Mark)> = steps.iter().map(|x| (x.id.to_string(), x.title.clone(), mark(st, x))).collect();
    steps.push(finish(&marks)); marks.push(("done".into(), "Done".into(), Mark::Todo));
    let id = step_id(st, session);
    let i = steps.iter().position(|x| x.id == id).unwrap_or_else(|| if id.is_empty() { 0 } else { marks.iter().position(|m| m.2 != Mark::Done).unwrap_or(0) });
    (steps, marks, i)
}

/// What a computer draws while setup is up, None when it is not (and always None on a phone).
pub fn view(st: &State, session: &Value, info: &Value, c: &Client, user_home: &str) -> Option<Value> {
    if c.phone || !due(st, session) { return None }
    Some(page(st, session, info, c, user_home))
}

/// The step on screen, with the rail and the rows the desktop draws beside it.
pub fn page(st: &State, session: &Value, info: &Value, c: &Client, user_home: &str) -> Value {
    let f = Facts::of(st, info);
    let (steps, marks, i) = walk(&f, st, session, c, user_home);
    let x = &steps[i]; let last = i + 1 == steps.len();
    let next = if last { "Done" } else if x.task && !x.done && !x.accept { "Skip step" } else { "Next" };
    let rail: Vec<Value> = steps.iter().zip(marks.iter()).map(|(x, m)| json!({"id": x.id, "title": x.title, "group": x.group, "state": m.2.word()})).collect();
    let done_n = marks.iter().filter(|m| m.2 == Mark::Done).count();
    json!({"id": x.id, "step": i, "count": steps.len(), "counter": format!("{} of {}", i + 1, steps.len()), "title": x.title, "body": x.body, "next": next, "back": "Back", "skip": "Finish later", "first": i == 0, "off": "",
        "end": "End setup", "group": x.group, "panel": x.panel, "rows": x.rows, "choices": x.choices, "steps": rail, "state": mark(st, x).word(), "progress": format!("{done_n} of {} done", marks.len() - 1)})
}

/// The rows of Settings, Setup: where it stands, and the way back in. A phone only says where setup is.
pub fn settings_rows(st: &State, session: &Value, info: &Value, c: &Client, user_home: &str) -> Vec<Value> {
    if c.phone { return vec![row("Setup", "Setup runs on a computer, in Settings, Setup there. This phone is paired with it.", vec![])] }
    let p = page(st, session, info, c, user_home);
    let done = set(st, "onboard.done") == "1";
    let mut v = vec![if done { row("Setup is finished", &format!("{}. Run it again to check what changed.", s(&p, "progress")), vec![btn("Run setup again", "ob-open", "start", true)]) }
        else if !set(st, "onboard.step").is_empty() { row("Setup is not finished", &s(&p, "progress").to_string(), vec![btn("Resume setup", "ob-open", "resume", true), btn("Start again", "ob-open", "start", false)]) }
        else { row("Setup", "Five short steps: your data folder, agents, design tools, where your uberbot lives and a first project.", vec![btn("Run setup", "ob-open", "start", true)]) }];
    for x in p["steps"].as_array().into_iter().flatten().filter(|x| s(x, "id") != "done") {
        let word = match s(x, "state") { "done" => "Done", "skipped" => "Skipped", _ => "Not done" };
        v.push(row(s(x, "title"), word, vec![btn("Go", "ob-go", s(x, "id"), false)]));
    }
    v
}

fn send(hub: &str, rid: &mut u64, cmds: &mut Vec<Value>, m: &str, p: Value) { cmds.push(json!({"type": "send", "hub": hub, "text": json!({"id": *rid, "m": m, "p": p}).to_string()})); *rid += 1; }
fn put(hub: &str, rid: &mut u64, cmds: &mut Vec<Value>, k: &str, v: &str) { send(hub, rid, cmds, "setting.set", json!({"key": k, "value": v})); }
fn with(v: &str, id: &str) -> String { let mut l = list(v); if !l.contains(&id) { l.push(id); } l.join(",") }

/// Whether this press ended setup for good (End setup, Done on the last step, Open the uberbot): its patch empties the step.
pub fn ended(action: &str, patch: &Value) -> bool { matches!(action, "ob-next" | "ob-end" | "ob-uber-open") && patch["drafts"]["@ob.step"] == "" && patch["drafts"]["@ob.open"] == "" }
/// Where the person lands when setup ends with nothing open: the newest thread of a project that is still there (never a bot's chat, an archived or a deleted one).
pub fn land(st: &State) -> Option<String> {
    st.threads.iter().filter(|(_, t)| !t.deleted && !t.archived && st.projects.get(&t.project).map_or(false, |p| !p.removed))
        .max_by_key(|(id, t)| (t.activity.max(t.created), (*id).clone())).map(|(id, _)| id.clone())
}
/// The bot Open the uberbot opens once setup has ended: this home's uberbot, when it has one yet.
pub fn land_uber(st: &State) -> Option<String> { uber_of(st) }

/// A press in setup. None for what is not setup's.
pub fn act(st: &State, hub: &str, session: &Value, info: &Value, c: &Client, user_home: &str, action: &str, value: &str) -> Option<Out> {
    if !action.starts_with("ob-") && action != "bfield" { return None }
    if action == "bfield" {
        let (name, text) = value.split_once('\u{1f}')?;
        if !(matches!(name, "obkicad" | "obfreecad" | "obproject" | "obfirst") || name.starts_with("obaon")) { return None }
        return Some(Out { patch: json!({"drafts": {format!("@f.{name}"): text}}), cmds: vec![] })
    }
    let mut ask = Ask { hub, rid: session["rid"].as_u64().unwrap_or(1), cmds: vec![] };
    let f = Facts::of(st, info);
    let (steps, _, i) = walk(&f, st, session, c, user_home);
    let go = |id: &str, ask: &mut Ask| -> Value {
        ask.put("onboard.step", id); ask.send("onboard.check", json!({}));
        // the uberbot is made on the way to Done, so Open the uberbot finds it
        if id == "done" && uber_of(st).is_none() { ask.send("onboard.uber", json!({})); }
        json!({"drafts": {"@ob.step": id, "@ob.project.err": "", "@invite": Value::Null, "@ob.panel": ""}})
    };
    let end = |ask: &mut Ask| -> Value { ask.put("onboard.done", "1"); ask.put("onboard.step", ""); json!({"drafts": {"@ob.step": "", "@ob.open": "", "@ob.closed": ""}}) };
    let mut patch = if let Some(p) = super::onboard_alwayson::act(&f, session, c, action, value, &mut ask) { p } else { match action {
        "ob-next" => {
            let x = &steps[i];
            if i + 1 == steps.len() { end(&mut ask) } else {
                if x.id == "alwayson" { super::onboard_alwayson::next(&f, session, &mut ask); }
                else if x.task && !x.done { ask.put("onboard.skipped", &with(set(st, "onboard.skipped"), x.id)); }
                else if !x.task { ask.put("onboard.seen", &with(set(st, "onboard.seen"), x.id)); }
                go(steps[i + 1].id, &mut ask)
            }
        }
        "ob-back" => go(steps[i.saturating_sub(1)].id, &mut ask),
        "ob-go" => { if !steps.iter().any(|x| x.id == value) { return None } let mut p = go(value, &mut ask); p["drafts"]["@ob.open"] = json!("1"); p["drafts"]["@ob.closed"] = json!(""); p["open"] = json!({"settings": false}); p }
        // Finish later: closed here until the next launch, the step kept
        "ob-skip" => { ask.put("onboard.step", steps[i].id); json!({"drafts": {"@ob.closed": "1", "@ob.open": ""}}) }
        "ob-end" => { let mut p = end(&mut ask); p["drafts"]["@ob.closed"] = json!(""); p }
        "ob-uber-open" => { if uber_of(st).is_none() { ask.send("onboard.uber", json!({})); } end(&mut ask) }
        // from Settings or the sidebar: "start" at the first step, anything else where it was left
        "ob-open" => {
            let first = if value == "start" { steps[0].id.to_string() } else { steps[i].id.to_string() };
            ask.put("onboard.step", &first);
            ask.send("onboard.check", json!({}));
            json!({"drafts": {"@ob.open": "1", "@ob.closed": "", "@ob.step": first}, "open": {"settings": false}})
        }
        // Settings, Developer: setup's progress forgotten, opened at the first step as on a new hub
        "ob-replay" => {
            let first = steps[0].id;
            for (k, v) in [("onboard.done", ""), ("onboard.skipped", ""), ("onboard.seen", ""), ("onboard.kicad.auto", ""), ("onboard.step", first)] { ask.put(k, v); }
            ask.send("onboard.check", json!({}));
            json!({"drafts": {"@ob.open": "1", "@ob.closed": "", "@ob.step": first}, "open": {"settings": false}})
        }
        "ob-install" => { if value.is_empty() { return None } ask.send("onboard.install", json!({"tool": value})); json!({}) }
        "ob-kicad-install" => { ask.send("kicad.install", json!({"consent": true})); json!({}) }
        "ob-check" => { ask.send("onboard.check", json!({})); json!({}) }
        "ob-more" => json!({"drafts": {"@ob.more": if draft(session, "@ob.more") == "1" { "" } else { "1" }}}),
        "ob-connect" => { if !matches!(value, "claude" | "codex") { return None } json!({"drafts": {"@ob.panel": value}}) }
        "ob-import" => {
            if let Some(at) = value.strip_prefix("ack:") { ask.put(super::super::import014::ACK, at); return Some(Out { patch: json!({}), cmds: ask.cmds }) }
            if value == "later" { return Some(Out { patch: json!({"drafts": {"@import.later": "1"}}), cmds: ask.cmds }) }
            let (op, secrets) = match value { "scan" => ("scan", false), "run" => ("run", false), "run-secrets" => ("run", true), "retry" => ("run", import_info(info).map_or(false, |v| v["secrets"] == true)), _ => return None };
            ask.send("import.v014", json!({"op": op, "secrets": secrets}));
            json!({"drafts": {"@import.later": ""}})
        }
        "ob-usage" => {
            if value != "on" && value != "off" { return None }
            ask.put("onboard.usage", value);
            if value == "off" { ask.put("telemetry.share", "off"); }
            json!({})
        }
        "ob-kicad" => { let raw = draft(session, "@f.obkicad").trim().to_string(); ask.put("kicad.path", &raw); ask.send("onboard.check", json!({"kicad": raw})); json!({}) }
        "ob-freecad" => { let raw = draft(session, "@f.obfreecad").trim().to_string(); ask.put("freecad.path", &raw); ask.send("onboard.check", json!({"freecad": raw})); json!({}) }
        "ob-project" => {
            let raw = draft(session, "@f.obproject").trim().to_string();
            let path = if raw.is_empty() { format!("{}/Backplane/first-board", user_home.trim_end_matches('/')) } else { raw };
            if !(path.starts_with('/') || path.starts_with('~') || path.chars().nth(1) == Some(':')) { return Some(Out { patch: json!({"drafts": {"@ob.project.err": "Give the full path, starting at the top of the disk."}}), cmds: ask.cmds }) }
            ask.send("project.new", json!({"path": path, "orAdd": true}));
            json!({"drafts": {"@ob.project.err": "", "@f.obproject": Value::Null}})
        }
        "ob-thread" => {
            let text = draft(session, "@f.obfirst").trim().to_string();
            if !st.projects.contains_key(value) { return None }
            let mut p = json!({"project": value});
            if !text.is_empty() { p["text"] = json!(text); }
            ask.send("thread.create", p);
            json!({"drafts": {"@f.obfirst": ""}})
        }
        _ => return None,
    } };
    if ask.rid != session["rid"].as_u64().unwrap_or(1) { patch["rid"] = json!(ask.rid); }
    Some(Out { patch, cmds: ask.cmds })
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
    fn at_with(st_: &State, d: Value, i: &Value) -> Value { page(st_, &sess(d), i, &desk(), "/h") }
    fn setting_of(o: &Out, key: &str) -> Option<String> { o.cmds.iter().filter_map(|c| serde_json::from_str::<Value>(c["text"].as_str()?).ok()).find(|v| v["m"] == "setting.set" && v["p"]["key"] == key).map(|v| s(&v["p"], "value").to_string()) }
    fn sent(o: &Out, m: &str) -> Vec<Value> { o.cmds.iter().filter_map(|c| serde_json::from_str::<Value>(c["text"].as_str()?).ok()).filter(|v| v["m"] == m).map(|v| v["p"].clone()).collect() }
    fn row_of(p: &Value, label: &str) -> Value { p["rows"].as_array().unwrap().iter().find(|r| r["label"] == label).cloned().unwrap_or_else(|| panic!("no {label} row in {}", p["rows"])) }
    fn actions(p: &Value) -> Vec<String> { p["rows"].as_array().unwrap().iter().flat_map(|r| r["buttons"].as_array().into_iter().flatten()).map(|b| s_(b, "action")).collect() }

    #[test] fn five_steps_then_done_and_nothing_of_the_tour() {
        let d = at(&cst(blank(), &[]), "welcome", &info("linux"), &desk());
        assert_eq!(ids(&d), STEPS);
        assert_eq!(d["count"], 6); assert_eq!(d["counter"], "1 of 6");
        let mut every = String::new();
        for id in STEPS { every.push_str(&at(&cst(blank(), &[]), id, &info("linux"), &desk()).to_string()); }
        for gone in ["Ctrl+K", "Palette", "Routines", "Rooms", "Restarts and updates", "tour"] { assert!(!every.contains(gone), "{gone:?} left in setup"); }
    }
    #[test] fn a_phone_has_no_setup() {
        assert!(view(&st(&[]), &sess(json!({})), &info("linux"), &phone_c(), "/h").is_none(), "a blank home: a computer shows setup, a phone none");
        assert!(view(&st(&[]), &sess(json!({})), &info("linux"), &desk(), "/h").is_some());
        assert!(view(&st(&[]), &sess(json!({"@ob.open": "1"})), &info("linux"), &phone_c(), "/h").is_none(), "not even asked for");
        let r = settings_rows(&st(&[]), &sess(json!({})), &info("linux"), &phone_c(), "/h"); assert_eq!(r.len(), 1); assert!(r[0]["buttons"].as_array().unwrap().is_empty());
    }
    #[test] fn welcome_holds_the_folder_the_usage_switch_and_the_import_only_with_a_0_14_home() {
        let s0 = cst(blank(), &[]);
        let p = at(&s0, "welcome", &info("linux"), &desk());
        assert!(row_of(&p, "Data folder")["note"].as_str().unwrap().starts_with("/h/.backplane/hub")); assert_eq!(row_of(&p, "Data folder")["buttons"][0]["action"], "ob-home");
        assert_eq!(actions(&p).iter().filter(|a| *a == "ob-usage").count(), 2);
        assert_eq!(p["state"], "todo", "seen only after Next"); assert_eq!(p["next"], "Next");
        assert!(!p.to_string().contains("ob-import"), "no 0.14 home: no import");
        let mut i = info("linux"); i["import.v014"] = json!(json!({"state":"found","from":"/h/.backplane-bend"}).to_string());
        let p = at(&s0, "welcome", &i, &desk()); assert!(actions(&p).contains(&"ob-import".to_string())); assert!(row_of(&p, "Backplane 0.14")["note"].as_str().unwrap().contains("/h/.backplane-bend"));
        let nl = page(&s0, &sess(json!({"@ob.step": "welcome"})), &info("linux"), &Client { phone: false, os: "linux".into(), launcher: false }, "/h");
        assert!(row_of(&nl, "Data folder")["buttons"].as_array().unwrap().is_empty(), "no launcher: no Choose another");
    }
    #[test] fn agents_is_one_list_with_more_agents_folded() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let p = at(&s0, "agents", &i, &desk());
        let labels: Vec<String> = p["rows"].as_array().unwrap().iter().map(|r| s_(r, "label")).collect();
        assert_eq!(&labels[..5], ["Claude Code", "Codex", "Antigravity", "Grok Build", "More agents"]);
        assert!(!labels.contains(&"Muse Code".to_string()), "folded");
        assert_eq!(row_of(&p, "Claude Code")["buttons"][0]["action"], "ob-install"); assert_eq!(p["next"], "Skip step");
        let o = act(&s0, "h", &sess(json!({"@ob.step": "agents"})), &i, &desk(), "/h", "ob-more", "").unwrap(); assert_eq!(o.patch["drafts"]["@ob.more"], "1");
        let open = at_with(&s0, json!({"@ob.step": "agents", "@ob.more": "1"}), &i);
        for n in ["Muse Code", "OpenCode", "Pi", "Oh My Pi"] { row_of(&open, n); }
        assert_eq!(row_of(&open, "More agents")["buttons"][0]["label"], "Hide");
    }
    #[test] fn an_agent_reads_install_connect_or_connected_as() {
        let i = info("linux");
        let mut c = blank(); c["agents"]["claude"] = json!({"path": "/x/claude", "version": "2.1", "signed": "no"}); c["agents"]["codex"] = json!({"path": "/x/codex", "version": "0.9", "signed": "yes"}); c["accounts"]["codex"] = json!(["Work"]);
        c["agents"]["grok"] = json!({"path": "/x/grok", "version": "1.0", "signed": "unknown"});
        let s0 = cst(c.clone(), &[]);
        let p = at(&s0, "agents", &i, &desk());
        let cl = row_of(&p, "Claude Code"); assert_eq!(cl["buttons"][0]["action"], "ob-connect"); assert!(cl["note"].as_str().unwrap().contains("Not connected"));
        assert!(row_of(&p, "Codex")["note"].as_str().unwrap().contains("Connected as Work"));
        assert!(row_of(&p, "Grok Build")["note"].as_str().unwrap().contains("grok login")); assert_eq!(row_of(&p, "Grok Build")["buttons"][0]["value"], "grok login");
        assert_eq!(p["state"], "done", "one agent signed in");
        c["agents"]["claude"]["signed"] = json!("yes"); c["agents"]["claude"]["who"] = json!("a@b.c");
        assert!(row_of(&at(&cst(c, &[]), "agents", &i, &desk()), "Claude Code")["note"].as_str().unwrap().contains("Connected as a@b.c"));
        // Connect opens that provider's accounts panel under the list, and a step change closes it
        let o = act(&s0, "h", &sess(json!({"@ob.step": "agents"})), &i, &desk(), "/h", "ob-connect", "claude").unwrap(); assert_eq!(o.patch["drafts"]["@ob.panel"], "claude");
        assert_eq!(at_with(&s0, json!({"@ob.step": "agents", "@ob.panel": "claude"}), &i)["panel"], "claude");
        assert!(act(&s0, "h", &sess(json!({})), &i, &desk(), "/h", "ob-connect", "grok").is_none());
        let n = act(&s0, "h", &sess(json!({"@ob.step": "agents", "@ob.panel": "claude"})), &i, &desk(), "/h", "ob-next", "").unwrap(); assert_eq!(n.patch["drafts"]["@ob.panel"], "");
    }
    /// What the KiCad row does for each computer and each KiCad found: the same decision `install::decide` is tested on, as the person sees it.
    #[test] fn the_kicad_row_installs_offers_or_explains_by_what_was_found() {
        use crate::files::kicad::install::{pick, LINUX};
        let step = |os: &str, arch: &str, status: &str, ins: Option<&str>| {
            let mut i = info(os); i["kicad.status"] = json!(status);
            if status != "missing" { i["kicad.cli"] = json!("/k/bin/kicad-cli"); i["kicad.version"] = json!(if status == "ok" { "10.99.0-bp.4" } else { "10.0.6" }); }
            if let Some(s) = ins { i["kicad.install"] = json!(s); }
            let s0 = cst(blank(), &[]);
            let x = tools_with(&Facts::of(&s0, &i), &s0, &sess(json!({})), pick(os, arch));
            (Value::Array(x.rows.clone()), x.rows.iter().find(|r| r["label"] == "KiCad").cloned().unwrap())
        };
        let acts = |rows: &Value| -> Vec<String> { rows.as_array().unwrap().iter().flat_map(|r| r["buttons"].as_array().into_iter().flatten()).map(|b| s_(b, "action")).collect() };
        let (rows, k) = step("linux", "x86_64", "missing", None); assert!(k["note"].as_str().unwrap().contains("Downloading Backplane KiCad 10.99.0-bp.4"), "{k}");
        assert!(!acts(&rows).contains(&"ob-kicad-install".to_string()), "no button to press: it starts by itself");
        let (_, r) = step("windows", "x86_64", "missing", Some(r#"{"state":"running","version":"10.99.0-bp.6","note":"Unpacking"}"#)); assert!(r["note"].as_str().unwrap().contains("Installing Backplane KiCad 10.99.0-bp.6. Unpacking"));
        let (rows, ok) = step("linux", "x86_64", "ok", Some(r#"{"state":"done","version":"10.99.0-bp.4","note":"x"}"#)); assert!(ok["note"].as_str().unwrap().contains("10.99.0-bp.4"));
        assert!(!rows.to_string().contains("Locate KiCad"), "found: nothing to locate");
        let (rows, bad) = step("linux", "x86_64", "missing", Some(r#"{"state":"failed","note":"KiCad install failed: KiCad archive checksum mismatch"}"#)); assert!(acts(&rows).contains(&"ob-kicad-install".to_string()) && bad["note"].as_str().unwrap().contains("checksum mismatch") && bad["tone"] == "warn");
        let (rows, stock) = step("macos", "aarch64", "stock", None); let n = stock["note"].as_str().unwrap(); assert!(n.contains("Stock KiCad 10.0.6") && n.contains("works for viewing and export") && n.contains("adds editing"), "{n}"); assert!(acts(&rows).contains(&"ob-kicad-install".to_string()));
        let (rows, none) = step("linux", "aarch64", "missing", None); assert!(none["note"].as_str().unwrap().contains("No Backplane KiCad is published for this computer"));
        assert!(rows.to_string().contains("kicad.org") && rows.to_string().contains("\"obkicad\""), "stock download and the path field");
        assert!(LINUX.version.contains("bp."));
    }
    #[test] fn design_tools_is_done_when_both_are_found() {
        let i = info("linux");
        let mut c = blank(); c["kicad"] = json!({"path": "/usr/bin/kicad-cli", "version": "10.0.7", "runs": true});
        let p = at(&cst(c.clone(), &[]), "tools", &i, &desk()); assert_eq!(p["state"], "todo"); assert!(p["rows"].to_string().contains("Locate FreeCAD")); assert!(!p["rows"].to_string().contains("Locate KiCad"));
        c["freecad"] = json!({"path": "/x/freecadcmd", "version": "1.0.2", "runs": true});
        let p = at(&cst(c, &[]), "tools", &i, &desk()); assert_eq!(p["state"], "done"); assert_eq!(p["next"], "Next"); assert!(row_of(&p, "FreeCAD")["note"].as_str().unwrap().contains("1.0.2"));
        let mut b = blank(); b["freecad"] = json!({"path": "/x/freecadcmd", "runs": false, "why": "exit code 1"});
        let f = row_of(&at(&cst(b, &[]), "tools", &i, &desk()), "FreeCAD"); assert!(f["note"].as_str().unwrap().contains("exit code 1")); assert_eq!(f["tone"], "warn");
        // Windows: winget buttons only when the checks found winget, the download page otherwise
        let w = |winget: bool| { let mut ch = blank(); ch["winget"] = json!(winget); let mut i = info("windows"); i["arch"] = json!("x64"); at(&cst(ch, &[]), "tools", &i, &desk())["rows"].to_string() };
        assert!(w(true).contains("\"ob-install\",\"value\":\"freecad\"")); assert!(!w(false).contains("ob-install") && w(false).contains("freecad.org"));
    }
    #[test] fn where_your_uberbot_lives_preselects_this_computer_and_next_takes_it() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let p = at(&s0, "alwayson", &i, &desk());
        assert_eq!(p["title"], "Where your uberbot lives"); assert_eq!(p["next"], "Next", "the preselected choice is an answer");
        assert!(p["body"].as_str().unwrap().contains("while this computer sleeps"));
        let labels: Vec<String> = p["rows"].as_array().unwrap().iter().map(|r| s_(r, "label")).collect();
        for l in ["Stay on this computer", "An always-on Boat", "Another machine of yours"] { assert!(labels.contains(&l.to_string()), "{l}"); }
        assert_eq!(row_of(&p, "Stay on this computer")["buttons"][0]["on"], true);
        let o = act(&s0, "h", &sess(json!({"@ob.step": "alwayson"})), &i, &desk(), "/h", "ob-next", "").unwrap();
        assert_eq!(setting_of(&o, super::super::onboard_alwayson::CHOICE).as_deref(), Some("here")); assert_eq!(setting_of(&o, "onboard.step").as_deref(), Some("project"));
        assert_eq!(at(&cst(blank(), &[("alwayson.choice", "here")]), "alwayson", &i, &desk())["state"], "done");
        let b = act(&s0, "h", &sess(json!({"@ob.step": "alwayson"})), &i, &desk(), "/h", "ob-aon-pick", "boat").unwrap(); assert_eq!(b.patch["drafts"]["@ob.aon"], "boat");
    }
    #[test] fn first_project_makes_the_folder_then_takes_the_first_message_and_offers_a_phone() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let p = at(&s0, "project", &i, &desk()); assert_eq!(row_of(&p, "Folder")["fields"][0]["value"], "/h/Backplane/first-board");
        assert_eq!(row_of(&p, "Pair a phone")["buttons"][0]["action"], "device-invite");
        let o = act(&s0, "h", &sess(json!({"@ob.step": "project"})), &i, &desk(), "/home/x", "ob-project", "").unwrap();
        assert_eq!(sent(&o, "project.new")[0]["path"], "/home/x/Backplane/first-board"); assert_eq!(sent(&o, "project.new")[0]["orAdd"], true);
        let bad = act(&s0, "h", &sess(json!({"@f.obproject": "here"})), &i, &desk(), "/home/x", "ob-project", "").unwrap(); assert!(bad.cmds.is_empty()); assert!(s(&bad.patch["drafts"], "@ob.project.err").contains("full path"));
        let mut s1 = cst(blank(), &[]); s1.apply(&json!({"$":"ProjectCreated","id":"p1","title":"first-board","root":"/h/Backplane/first-board","at":1}));
        let p = at(&s1, "project", &i, &desk()); assert_eq!(p["state"], "done"); assert_eq!(row_of(&p, "First message")["buttons"][0]["value"], "p1");
        let t = act(&s1, "h", &sess(json!({"@ob.step": "project", "@f.obfirst": "make a blinky"})), &i, &desk(), "/h", "ob-thread", "p1").unwrap();
        assert_eq!(sent(&t, "thread.create")[0], json!({"project": "p1", "text": "make a blinky"}));
        let link = sess(json!({"@ob.step": "project", "@invite": json!({"link": "http://127.0.0.1:3800/pair#invite=x", "code": "ABCD2345", "localOnly": true}).to_string()}));
        let q = page(&s0, &link, &i, &desk(), "/h"); assert!(row_of(&q, "Pair a phone")["qr"].is_array()); assert_eq!(row_of(&q, "Only this computer")["tone"], "warn");
    }
    #[test] fn done_says_the_help_key_and_opens_the_uberbot() {
        let (s0, i) = (cst(blank(), &[("onboard.skipped", "tools")]), info("linux"));
        let p = at(&s0, "done", &i, &desk());
        assert_eq!(p["body"], "Press ? any time for keys."); assert_eq!(p["choices"][0]["action"], "ob-uber-open");
        let k = row_of(&p, "Design tools"); assert_eq!(k["note"], "Skipped."); assert_eq!(k["buttons"][0]["value"], "tools");
        let o = act(&s0, "h", &sess(json!({"@ob.step": "done"})), &i, &desk(), "/h", "ob-uber-open", "").unwrap();
        assert_eq!(setting_of(&o, "onboard.done").as_deref(), Some("1")); assert!(ended("ob-uber-open", &o.patch)); assert_eq!(sent(&o, "onboard.uber").len(), 1, "no uberbot yet: made");
        // the way to Done makes it, so Open finds it
        let g = act(&s0, "h", &sess(json!({"@ob.step": "project"})), &i, &desk(), "/h", "ob-next", "").unwrap(); assert_eq!(sent(&g, "onboard.uber").len(), 1);
        let mut u = cst(blank(), &[("uber.bot", "b1")]); u.apply(&json!({"$":"BotSet","id":"b1","name":"Bub","thread":"tb","look":0,"persona":"","at":1}));
        assert_eq!(land_uber(&u).as_deref(), Some("b1"));
        let o = act(&u, "h", &sess(json!({"@ob.step": "done"})), &i, &desk(), "/h", "ob-uber-open", "").unwrap(); assert!(sent(&o, "onboard.uber").is_empty());
    }
    #[test] fn an_old_setup_stays_done_and_a_half_done_one_resumes_at_the_first_new_step_not_done() {
        let i = info("linux");
        assert!(!due(&st(&[("onboard.done", "1")]), &sess(json!({}))), "finished the old one: done");
        let mut c = blank(); c["agents"]["claude"] = json!({"path": "/x", "version": "2", "signed": "yes"});
        let half = cst(c.clone(), &[("onboard.step", "machine"), ("onboard.seen", "welcome,install,home,viewers"), ("onboard.skipped", "codex,kicad")]);
        assert!(due(&half, &sess(json!({}))));
        assert_eq!(page(&half, &sess(json!({})), &i, &desk(), "/h")["id"], "tools", "welcome seen, agents done: tools is next");
        let fresh_ids = cst(c, &[("onboard.step", "palette")]);
        assert_eq!(page(&fresh_ids, &sess(json!({})), &i, &desk(), "/h")["id"], "welcome", "welcome not seen yet");
        let o = act(&half, "h", &sess(json!({})), &i, &desk(), "/h", "ob-next", "").unwrap(); assert_eq!(setting_of(&o, "onboard.step").as_deref(), Some("alwayson"), "Next from where it resumed");
        let r = act(&half, "h", &sess(json!({"@ob.closed": "1"})), &i, &desk(), "/h", "ob-open", "resume").unwrap(); assert_eq!(r.patch["drafts"]["@ob.step"], "tools");
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
        // the phones' card presses work, from a phone too
        assert!(act(&st, "h", &sess(json!({})), &i, &phone_c(), "/h", "ob-import", "later").is_some());
    }
    #[test] fn each_import_press_asks_the_hub() {
        let st = cst(blank(), &[]);
        let mut i = info("linux"); i["import.v014"] = json!(json!({"state":"found","from":"/h/.backplane-bend"}).to_string());
        for (v, op, secrets) in [("scan", "scan", false), ("run", "run", false), ("run-secrets", "run", true)] {
            let o = act(&st, "h", &sess(json!({})), &i, &desk(), "/h", "ob-import", v).unwrap();
            let sent: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap();
            assert_eq!((sent["m"].as_str(), sent["p"]["op"].as_str(), sent["p"]["secrets"].as_bool()), (Some("import.v014"), Some(op), Some(secrets)));
        }
        assert!(act(&st, "h", &sess(json!({})), &i, &desk(), "/h", "ob-import", "rm").is_none());
        let done = json!({"import.v014": json!({"state":"done","counts":{"projects":2,"threads":3,"messages":9,"lines":20},"notes":[{"k":"runs","n":4}]}).to_string()});
        let r = import_rows(&done); assert_eq!(r[0]["note"], "2 projects and 3 threads with 9 messages."); assert_eq!(r[1]["label"], "Import again"); assert!(r[2]["note"].as_str().unwrap().starts_with("4 mail runs stay in 0.14"));
        assert!(import_note(&json!({"k":"runs","n":1})).1.starts_with("1 mail run stays")); assert!(import_note(&json!({"k":"routines-off","n":1})).1.starts_with("1 routine arrives"));
        assert_eq!(import_counted(&json!({"lines": 86, "parents": 86})), "86 subagent threads under the threads that started them.");
        assert_eq!(import_counted(&json!({"projects": 1, "threads": 3, "messages": 9, "parents": 1})), "1 project and 3 threads with 9 messages.");
    }
    /// Every URL a step can show, on each system.
    fn all_urls() -> Vec<String> {
        fn walk(v: &Value, out: &mut Vec<String>) { match v { Value::String(t) if t.starts_with("https://") && !t.contains(' ') => out.push(t.clone()), Value::Array(a) => a.iter().for_each(|x| walk(x, out)), Value::Object(o) => o.values().for_each(|x| walk(x, out)), _ => {} } }
        let mut out = vec![];
        for os in ["linux", "darwin", "windows"] { for winget in [false, true] {
            let mut ch = blank(); ch["winget"] = json!(winget);
            let (s0, mut i) = (cst(ch, &[]), info(os)); i["arch"] = json!("x64");
            for id in STEPS { walk(&at_with(&s0, json!({"@ob.step": id, "@ob.more": "1"}), &i), &mut out); }
            i["kicad.status"] = json!("stock"); walk(&at(&s0, "tools", &i, &desk()), &mut out);
        } }
        out.sort(); out.dedup(); out
    }
    #[test] fn every_agent_and_tool_has_a_button_or_a_page_on_each_system() {
        for os in ["linux", "darwin", "windows"] {
            let (s0, mut i) = (cst(blank(), &[]), info(os)); i["arch"] = json!("x64");
            let p = at_with(&s0, json!({"@ob.step": "agents", "@ob.more": "1"}), &i);
            for r in p["rows"].as_array().unwrap().iter().filter(|r| r["label"] != "More agents" && r["label"] != "Check again") { assert!(!r["buttons"].as_array().unwrap().is_empty(), "{os}: {r}"); }
            let t = at(&s0, "tools", &i, &desk()); for l in ["KiCad", "FreeCAD"] { assert!(!row_of(&t, l)["buttons"].as_array().unwrap().is_empty(), "{os}: {l}"); }
        }
        let ob = |os: &str, tool: &str| { let mut i = info(os); i["arch"] = json!("x64"); at(&cst(blank(), &[]), "agents", &i, &desk())["rows"].to_string().contains(&format!("\"ob-install\",\"value\":\"{tool}\"")) };
        for os in ["linux", "darwin", "windows"] { assert!(ob(os, "claude") && ob(os, "codex") && ob(os, "grok"), "{os}"); }
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
    #[test] fn no_internal_words() {
        let mut every = String::new();
        for os in ["linux", "darwin"] { let (s0, i) = (cst(blank(), &[]), info(os)); for id in STEPS { every.push_str(&at(&s0, id, &i, &desk()).to_string()); } }
        for bad in ["Bend", "hub home", "hub's home", "guard", "mirror", "PeerTable", "far thread", "Bots page", "Skip all of setup"] { assert!(!every.contains(bad), "{bad:?} in the steps"); }
        assert_eq!(at(&cst(blank(), &[]), "welcome", &info("linux"), &desk())["choices"].as_array().unwrap().len(), 0, "End setup is the footer's, not a second button");
    }
    #[test] fn it_comes_up_on_a_blank_home_and_leaves_an_old_home_alone() {
        assert!(due(&st(&[]), &sess(json!({}))));
        let mut old = st(&[]); old.apply(&json!({"$":"ProjectCreated","id":"p1","title":"x","root":"/x","at":1}));
        assert!(!due(&old, &sess(json!({}))), "a home with projects that never saw setup is not interrupted");
        let mut begun = old; begun.settings.insert("onboard.step".into(), "tools".into());
        assert!(due(&begun, &sess(json!({}))), "one where it began resumes at launch");
        assert!(!due(&st(&[("onboard.done", "1")]), &sess(json!({}))));
        assert!(due(&st(&[("onboard.done", "1")]), &sess(json!({"@ob.open": "1"}))), "Settings reopens a finished setup");
        assert!(!due(&st(&[]), &sess(json!({"@ob.closed": "1"}))));
    }
    #[test] fn next_on_an_unfinished_task_says_skip_step_and_records_it() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let p = at(&s0, "tools", &i, &desk()); assert_eq!(p["next"], "Skip step"); assert_eq!(p["state"], "todo");
        let o = act(&s0, "h", &sess(json!({"@ob.step": "tools"})), &i, &desk(), "/h", "ob-next", "").unwrap();
        assert_eq!(setting_of(&o, "onboard.skipped").as_deref(), Some("tools")); assert_eq!(setting_of(&o, "onboard.step").as_deref(), Some("alwayson")); assert_eq!(o.patch["drafts"]["@ob.step"], "alwayson");
        assert_eq!(at(&cst(blank(), &[("onboard.skipped", "tools")]), "tools", &i, &desk())["state"], "skipped");
    }
    #[test] fn finish_later_keeps_the_step_and_end_setup_is_final() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let o = act(&s0, "h", &sess(json!({"@ob.step": "alwayson"})), &i, &desk(), "/h", "ob-skip", "").unwrap();
        assert_eq!(setting_of(&o, "onboard.step").as_deref(), Some("alwayson")); assert_eq!(o.patch["drafts"]["@ob.closed"], "1"); assert_eq!(setting_of(&o, "onboard.done"), None);
        let begun = cst(blank(), &[("onboard.step", "alwayson")]);
        assert_eq!(at(&begun, "", &i, &desk())["id"], "alwayson", "the next launch opens where it stopped");
        assert_eq!(resume(&begun, &sess(json!({"@ob.closed": "1"})))["action"], "ob-open");
        let e = act(&s0, "h", &sess(json!({})), &i, &desk(), "/h", "ob-end", "").unwrap(); assert_eq!(setting_of(&e, "onboard.done").as_deref(), Some("1")); assert!(ended("ob-end", &e.patch));
        let k = act(&s0, "h", &sess(json!({"@ob.step": "tools"})), &i, &desk(), "/h", "ob-skip", "").unwrap(); assert!(!ended("ob-skip", &k.patch), "Finish later keeps the step");
    }
    #[test] fn done_ends_it_and_settings_runs_it_again_from_the_start() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let o = act(&s0, "h", &sess(json!({"@ob.step": "done"})), &i, &desk(), "/h", "ob-next", "").unwrap(); assert_eq!(setting_of(&o, "onboard.done").as_deref(), Some("1")); assert!(ended("ob-next", &o.patch));
        let n = act(&s0, "h", &sess(json!({"@ob.step": "tools"})), &i, &desk(), "/h", "ob-next", "").unwrap(); assert!(!ended("ob-next", &n.patch), "a step on is not the end");
        let fin = cst(blank(), &[("onboard.done", "1")]);
        assert_eq!(settings_rows(&fin, &sess(json!({})), &i, &desk(), "/h")[0]["buttons"][0]["label"], "Run setup again");
        let r = act(&fin, "h", &sess(json!({})), &i, &desk(), "/h", "ob-open", "start").unwrap();
        assert_eq!(r.patch["drafts"]["@ob.open"], "1"); assert_eq!(r.patch["drafts"]["@ob.step"], "welcome"); assert_eq!(r.patch["open"]["settings"], false);
        assert_eq!(sent(&r, "onboard.check").len(), 1, "a rerun detects what is done again");
        assert_eq!(land(&cst(blank(), &[])), None, "no thread, nowhere to land");
    }
    #[test] fn replay_forgets_the_progress_and_opens_the_first_step() {
        let i = info("linux");
        let fin = cst(blank(), &[("onboard.done", "1"), ("onboard.skipped", "tools"), ("onboard.seen", "welcome"), ("onboard.step", "")]);
        let r = act(&fin, "h", &sess(json!({"@ob.closed": "1"})), &i, &desk(), "/h", "ob-replay", "").unwrap();
        for k in ["onboard.done", "onboard.skipped", "onboard.seen"] { assert_eq!(setting_of(&r, k).as_deref(), Some(""), "{k}"); }
        assert_eq!(setting_of(&r, "onboard.step").as_deref(), Some("welcome"));
        assert_eq!((r.patch["drafts"]["@ob.open"].as_str(), r.patch["drafts"]["@ob.closed"].as_str(), r.patch["drafts"]["@ob.step"].as_str()), (Some("1"), Some(""), Some("welcome")));
    }
    #[test] fn a_typed_path_is_used_and_checked() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let o = act(&s0, "h", &sess(json!({"@f.obfreecad": " /opt/fc/freecadcmd "})), &i, &desk(), "/h", "ob-freecad", "").unwrap();
        assert_eq!(setting_of(&o, "freecad.path").as_deref(), Some("/opt/fc/freecadcmd")); assert_eq!(sent(&o, "onboard.check")[0]["freecad"], "/opt/fc/freecadcmd");
        let f = act(&s0, "h", &sess(json!({})), &i, &desk(), "/h", "bfield", "obaonkey\u{1f}x").unwrap(); assert_eq!(f.patch["drafts"]["@f.obaonkey"], "x", "step 4's fields are kept");
    }
    #[test] fn usage_off_writes_the_real_choice() {
        let (s0, i) = (cst(blank(), &[]), info("linux"));
        let o = act(&s0, "h", &sess(json!({})), &i, &desk(), "/h", "ob-usage", "off").unwrap(); assert_eq!(setting_of(&o, "telemetry.share").as_deref(), Some("off"));
        assert_eq!(row_of(&at(&cst(blank(), &[("onboard.usage", "on")]), "welcome", &i, &desk()), "Share anonymous usage")["buttons"][0]["on"], true);
    }

    /// Every word setup can show, on each system, against the rules of the unslop skill.
    fn all_text() -> Vec<String> {
        fn walk(v: &Value, out: &mut Vec<String>) { match v { Value::String(t) => out.push(t.clone()), Value::Array(a) => a.iter().for_each(|x| walk(x, out)), Value::Object(o) => for (k, x) in o { if !matches!(k.as_str(), "action" | "value" | "id" | "state" | "group" | "panel" | "qr" | "name") { walk(x, out) } }, _ => {} } }
        let mut out = vec![];
        let mut full = blank(); full["agents"]["claude"] = json!({"path": "/x", "version": "1", "signed": "no"}); full["agents"]["grok"] = json!({"path": "/x", "version": "1"}); full["kicad"] = json!({"path": "/x", "runs": false, "why": "exit code 1"}); full["freecad"] = json!({"path": "/x", "version": "1", "runs": true});
        for os in PLATFORMS.iter().map(|p| p.id) { for checks in [blank(), full.clone()] { for aon in ["here", "boat", "machine"] {
            let i = info(os); let s0 = cst(checks.clone(), &[]);
            for id in STEPS { walk(&at_with(&s0, json!({"@ob.step": id, "@ob.more": "1", "@ob.aon": aon}), &i), &mut out); }
            for r in settings_rows(&s0, &sess(json!({})), &i, &desk(), "/h") { walk(&r, &mut out); }
        } } }
        walk(&Value::Array(settings_rows(&st(&[]), &sess(json!({})), &info("linux"), &phone_c(), "/h")), &mut out);
        for st in import_samples() { walk(&Value::Array(import_rows(&st)), &mut out); walk(&at(&cst(blank(), &[]), "welcome", &st, &desk()), &mut out); }
        walk(&Value::Array(import_rows(&json!({}))), &mut out);
        for v in [json!({"state":"running","phase":"writing","what":"Writing projects and threads","done":4000,"total":9000,"pct":60,"eta_s":75,"projects":[3,12],"threads":[40,300],"current":"Nordic","from":"/f"}), json!({"state":"running","phase":"reading","what":"Reading the 0.14 log","done":1<<20,"total":9<<20,"pct":3,"eta_s":null}), json!({"state":"running","phase":"copying","what":"Copying","done":25,"total":0,"eta_s":5})] { walk(&import_card(&v, false, false).unwrap(), &mut out); }
        for st in import_samples() { let v: Value = serde_json::from_str(st["import.v014"].as_str().unwrap()).unwrap(); if let Some(c) = import_card(&v, false, false) { walk(&c, &mut out); } }
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
    #[test] fn every_platform_says_how_to_install_each_tool_and_the_agents_with_a_command() {
        for pf in PLATFORMS {
            assert!(!pf.name.is_empty() && !pf.installed.is_empty() && !pf.path_fix.is_empty() && !pf.kicad.is_empty() && !pf.freecad.is_empty() && !pf.kicad_names.is_empty() && !pf.freecad_names.is_empty(), "{}", pf.id);
            for a in ["claude", "codex", "grok"] { assert!(crate::settings::tools_install::plan(a, pf.id, "x64", false, false).is_some(), "{a} on {}", pf.id); }
        }
        assert_eq!(platform("beos").id, PLATFORMS[0].id, "a system with no entry yet reads the first");
    }
}
