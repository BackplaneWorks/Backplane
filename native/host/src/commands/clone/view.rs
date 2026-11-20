//! The desktop's Add project dialog in Clone from git (present/picker.rs draws the rest): the tabs, the repository step (the field takes an
//! address or `owner/repo`; the person's GitHub repositories are suggested under it), and the clone's own step (its progress, Cancel,
//! Try again, Sign in to GitHub, and Open when it is done). The session holds `@proj.mode`, `@proj.text`, `@proj.clone`, `@proj.job`,
//! `@proj.nothread`, `@proj.signin`; every row names its action, as the folder rows do.
use serde_json::{json, Value};

fn d<'a>(drafts: &'a Value, k: &str) -> &'a str { drafts[k].as_str().unwrap_or("") }

/// The two ways to add a project, the one on show marked.
pub fn tabs(clone: bool) -> Value {
    json!([{"label": "Open folder", "action": "proj-mode", "value": "folder", "on": !clone}, {"label": "Clone from git", "action": "proj-mode", "value": "clone", "on": clone}])
}
/// GitHub's sign-in box, while one was asked for here (`@proj.signin`) and is under way or just ended.
pub fn signin(drafts: &Value) -> Value {
    if d(drafts, "@proj.signin").is_empty() { return Value::Null }
    let s = super::gh::signin_now();
    if s["state"].as_str().unwrap_or("").is_empty() || s["state"] == "cancelled" { return Value::Null }
    s
}

/// The repository step: what the field holds as an address (its Clone row), then the person's repositories that match.
pub fn repo_step(drafts: &Value, home: &str) -> Value {
    let _ = home;
    let text = d(drafts, "@proj.text").to_string();
    let mut items: Vec<Value> = vec![];
    if let Some(r) = super::parse(&text) {
        items.push(json!({"label": format!("Clone {}", super::shown(&r)), "action": "proj-clone", "value": text.trim(), "kind": "add"}));
    }
    let (rows, more, state, loading) = super::gh::repos_now(&text, 12);
    for r in &rows {
        let mut it = json!({"label": r.name, "action": "proj-clone", "value": r.value(), "kind": "repo", "sub": r.description});
        if r.private { it["tag"] = json!("private"); }
        items.push(it);
    }
    if rows.is_empty() {
        if loading { items.push(json!({"label": "Loading your GitHub repositories…", "action": "", "value": "", "kind": "note"})); }
        else if state == "signedout" { items.push(json!({"label": "Sign in to GitHub to see your repositories here", "action": "proj-gh-signin", "value": "", "kind": "repo", "icon": "github"})); }
        else if !super::gh::state_hint(state).is_empty() && text.trim().is_empty() { items.push(json!({"label": super::gh::state_hint(state), "action": "", "value": "", "kind": "note"})); }
    }
    let t = text.trim();
    let error = if !t.is_empty() && items.iter().all(|i| i["kind"] == "note") && super::parse(t).is_none() && (t.contains("://") || t.contains('@') || t.contains(':')) { "That isn't a repository address: paste an https or ssh URL, or owner/repo." } else { "" };
    json!({"mode": "clone", "tabs": tabs(true), "icon": "git", "text": text, "hint": "Repository: https or ssh URL, or owner/repo", "error": error, "items": items, "more": more.saturating_sub(rows.len()), "cut": false, "signin": signin(drafts)})
}

/// The clone's step: its progress and what can be done with it now.
pub fn job_step(drafts: &Value, home: &str) -> Value {
    let id = d(drafts, "@proj.job");
    let Some(j) = super::job(id) else {
        return json!({"mode": "clone", "tabs": tabs(true), "icon": "git", "text": "", "hint": "", "error": "That clone is gone (the hub restarted). Start it again.", "items": [{"label": "Back", "action": "proj-clone-back", "value": "", "kind": "add"}], "more": 0, "cut": false});
    };
    let mut job = j.json(home);
    let act = |label: &str, action: &str, value: &str, kind: &str| json!({"label": label, "action": action, "value": value, "kind": kind});
    let open = format!("{}\u{1f}{}", j.thread, j.project);
    let actions: Vec<Value> = match j.state {
        super::State::Running => vec![act("Cancel", "proj-clone-cancel", id, "secondary")],
        super::State::Failed => { let mut v = vec![act("Try again", "proj-clone-retry", id, "primary")]; if j.auth && j.github { v.push(act("Sign in to GitHub", "proj-gh-signin", "", "secondary")); } v.push(act("Back", "proj-clone-back", id, "secondary")); v }
        super::State::Cancelled => vec![act("Try again", "proj-clone-retry", id, "primary"), act("Back", "proj-clone-back", id, "secondary")],
        super::State::Done => vec![act(if j.thread.is_empty() { "Open the project" } else { "Open the thread" }, "proj-clone-open", &open, "primary")],
    };
    job["actions"] = json!(actions);
    job["title"] = json!(match j.state { super::State::Done => format!("Cloned {} into {}", j.repo, super::tilde(&j.dest, home)), super::State::Cancelled => format!("Stopped cloning {}", j.repo), _ => format!("Cloning {} into {}", j.repo, super::tilde(&j.dest, home)) });
    // done with nothing the person must read: the window goes to it by itself (`open`); with notes, they are read first and Open goes
    if j.state == super::State::Done && !j.hold { job["open"] = json!({"thread": j.thread, "project": j.project}); }
    json!({"mode": "clone", "tabs": tabs(true), "icon": "git", "text": "", "hint": "", "error": "", "items": [], "more": 0, "cut": false, "job": job, "signin": signin(drafts)})
}

/// The destination step's own rows (the folder rows follow, from present/picker.rs): the repository being cloned (Change goes back to the
/// repository step) and whether its first thread is made.
pub fn dest_header(drafts: &Value) -> (Value, Value) {
    let clone = d(drafts, "@proj.clone");
    let shown = super::parse(clone).map(|r| super::shown(&r)).unwrap_or_else(|| clone.to_string());
    let repo = json!({"label": shown, "action": "proj-clone-back", "value": "", "change": "Change"});
    let on = d(drafts, "@proj.nothread").is_empty();
    (repo, json!({"label": "Start a thread in it", "action": "proj-clone-thread", "value": if on { "off" } else { "on" }, "kind": "toggle", "on": on}))
}
