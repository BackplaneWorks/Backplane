//! The keys of `shown.thread` that follow the thread's own state: `state`, `tools`, `menu`,
//! `working` and `phase`, computed from `state::State` for a thread with no asks, task links, queue, background rows or
//! launch (`plain_signal` is `signal::own` for exactly that case). `None`: a case this does not compute (a failed or stopped turn, a stalled thread,
//! a forked thread whose fork record a lean State may have dropped): the caller refuses it, retryably.
use crate::shelf::{shelf_of, Shelf};
use crate::signal;
use crate::state::{Override, State, Turn};
use serde_json::{json, Map, Value};
use std::sync::{Arc, Mutex};

/// Mob.state
pub fn state_word(t: Turn) -> &'static str { match t { Turn::Running => "run", Turn::Failed => "fail", Turn::Interrupted => "stop", _ => "idle" } }

fn tool(icon: &str, label: &str, tip: &str, action: &str, value: &str, on: bool, danger: bool) -> Value {
    json!({"icon": icon, "label": label, "tip": tip, "action": action, "value": value, "on": on, "danger": danger})
}

/// Tools.settled: settled by hand, or by itself (an answer the person has seen)
pub fn settled(st: &State, id: &str) -> bool {
    let Some(t) = st.threads.get(id) else { return false };
    match t.override_ { Override::ForcedSettled => true, Override::None => t.settled_at > t.unsettled_at, Override::ForcedActive => false }
}

/// Mob.head.loc: the turn's own button, pin, settle, archive, fork, and "back" for a forked thread (English)
pub fn tools(turn: Turn, pinned: bool, settled: bool, archived: bool, forked: bool) -> Vec<Value> {
    let mut v = Vec::new();
    match turn {
        Turn::Running => v.push(tool("stop", "Stop", "Stop the running turn · Ctrl+.", "interrupt", "", false, true)),
        Turn::Failed => v.push(tool("repeat", "Retry", "The turn failed: send your last message again", "retry", "", false, false)),
        _ => {}
    }
    v.push(if pinned { tool("pin", "Pinned", "Unpin: it sorts with the rest again", "unpin", "", true, false) } else { tool("pin", "Pin", "Keep it at the top of the list", "pin", "", false, false) });
    v.push(if settled { tool("circle-check", "Settled", "Bring it back to the active threads", "unsettle", "", true, false) } else { tool("circle-check", "Settle", "Done for now: it leaves the active threads", "settle", "", false, false) });
    v.push(if archived { tool("archive", "Archived", "Bring it back from the archive", "unarchive", "", true, false) } else { tool("archive", "Archive", "Put it away in the archive", "archive", "", false, false) });
    v.push(tool("git-fork", "Fork thread", "A new thread that goes on from here", "fork", "", false, false));
    if forked { v.push(tool("undo", "Back to the original", "Go back to the thread this one forked from", "back", "", false, false)) }
    v
}

fn item(icon: &str, label: &str, action: &str, value: &str, on: bool, danger: bool, options: Value) -> Value {
    json!({"icon": icon, "label": label, "action": action, "value": value, "on": on, "danger": danger, "options": options})
}
/// Mob.menu.loc without the project card's "mini" items: diff, terminal, find, snooze (or wake), delete
pub fn menu(id: &str, snoozed: bool) -> Vec<Value> {
    vec![
        item("diff", "Diff", "diff", "", false, false, json!([])),
        item("terminal", "Terminal", "term-toggle", "", false, false, json!([])),
        item("search", "Find file", "find-open", "files", false, false, json!([])),
        if snoozed { item("clock", "Snoozed", "snooze", "off", true, false, json!([])) }
        else { item("clock", "Snooze", "snooze", "", false, false, json!([{"label": "1 hour", "value": "1h"}, {"label": "Tomorrow", "value": "tomorrow"}, {"label": "1 week", "value": "1w"}])) },
        item("trash", "Delete", "row-delete", id, false, true, json!([])),
    ]
}

/// The thread's signal: its own turn and shelf alone when it has no asks, task links, queue, background rows, launch or pending, else the hub's
/// whole index (signal::index flows bits from children; O(threads), so only for the threads that need it)
fn plain_signal(st: &State, id: &str) -> Option<u8> {
    let t = st.threads.get(id)?;
    if t.deleted { return None }
    let linked = t.pending > 0 || st.queue.get(id).map_or(false, |q| !q.is_empty())
        || ["bg", "watch", "auth.wait", "order.wait", "launch"].iter().any(|k| st.settings.get(&format!("{k}.{id}")).map_or(false, |v| !v.is_empty()))
        || st.asks.values().any(|(th, _)| th == id) || st.tasks.values().any(|(p, c, _, _)| p == id || c == id);
    if linked { return signal::index(st).get(id).copied() }
    let blocked = t.turn == Turn::Failed;
    let active = t.turn == Turn::Running && !blocked;
    let settled = shelf_of(t) == Shelf::Settled;
    Some(signal::done(t.turn == Turn::Completed, signal::compose(blocked, active, settled)))
}

/// the sentence of Signal.label for a name (English); failed/stopped are not computed here
fn label(name: &str) -> Option<&'static str> {
    Some(match name {
        "active" => "Working: agent or owned work active", "inactive" => "Inactive: no confirmed active work", "finished" => "Finished: ready for you to look at",
        "settled" => "Settled", "waiting" => "Waiting: on subagents or work it started", "blocked" => "Blocked: needs interaction", _ => return None })
}


/// PM.Permissions.json: the permission mode chips (English); `rt` is the mode's name, absent for full access
pub fn permissions(rt: &str) -> Value {
    let modes = [("approval-required", "Approval required"), ("auto-accept-edits", "Auto-accept edits"), ("auto", "Auto"), ("full-access", "Full access")];
    let cur = if modes.iter().any(|(v, _)| *v == rt) { rt } else { "full-access" };
    let name = modes.iter().find(|(v, _)| *v == cur).map(|(_, n)| *n).unwrap_or("Full access");
    json!({"label": "Permissions", "current": format!("Permissions: {name}"),
        "choices": modes.iter().map(|(v, n)| json!({"label": n, "value": v, "on": *v == cur})).collect::<Vec<_>>()})
}

fn effort_label(e: &str) -> String { if e.is_empty() { "auto".into() } else { e.to_string() } }
fn levels(provider: &str) -> &'static [&'static str] {
    match provider { "claude" => &["low", "medium", "high", "xhigh", "max"], "codex" => &["low", "medium", "high", "xhigh"], "grok" => &["low", "medium", "high"],
        "muse" => &["minimal", "low", "medium", "high", "xhigh", "max"], "pi" => &["off", "minimal", "low", "medium", "high", "xhigh", "max"],
        "omp" => &["off", "auto", "minimal", "low", "medium", "high", "xhigh", "max"],
        "antigravity" => &["minimal", "low", "medium", "high", "xhigh", "max"], _ => &[] } // antigravity: each model shows only the variants agy has (about.efforts)
}
fn tokens(n: u64) -> String { if n >= 1_000_000 { format!("{}M", n / 1_000_000) } else if n >= 1000 { format!("{}k", n / 1000) } else { n.to_string() } }
/// Use.tail / Usage.text: the model chip's context tail from the thread's `usage.<id>` reading ("used/window")
fn usage_tail(provider: &str, reading: &str) -> String {
    let mut p = reading.split('/').map(|x| x.parse::<u64>().unwrap_or(0));
    let (used, window) = (p.next().unwrap_or(0), p.next().unwrap_or(0));
    if used > 0 { if window == 0 { format!(" · ctx {}", tokens(used)) } else { format!(" · ctx {}% of {}", used * 100 / window, tokens(window)) } }
    else if provider == "claude" { String::new() } else { " · ctx n/a".into() }
}
/// Allowance.chip, simplified: the least remaining percent across the provider's reported windows that apply to the model ("72% left"), or "" when no
/// window has a current reading (the client says so, never a made-up percentage). A window whose reset has passed is not a current reading.
/// `snapshot` is the thread's `allowance.<provider>` setting (permille used), or the bound profile's allowance (percent used) from `accounts.inventory`.
pub fn allowance_text(snapshot: &Value, provider: &str, model: &str, now_s: u64) -> String {
    if !matches!(provider, "claude" | "codex") || snapshot["provider"].as_str().is_some_and(|p| p != provider) { return String::new() }
    let m = model.to_lowercase();
    let mut left: Option<u64> = None; let mut shared = true;
    for w in snapshot["windows"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let scope = w["scope"].as_str().unwrap_or("");
        let applies = match w["model"].as_str().filter(|x| !x.is_empty()) { Some(x) => m.contains(x), None => provider == "codex" || scope == "shared" || scope == "five_hour" || scope == "seven_day" || (scope == "opus" && m.contains("opus")) || (scope == "sonnet" && m.contains("sonnet")) };
        if !applies { continue }
        let used = w["used_permille"].as_f64().or_else(|| w["used_percent"].as_f64().map(|p| p * 10.0));
        let Some(used) = used.filter(|u| (0.0..=1000.0).contains(u)) else { return String::new() };
        if w["resets_at"].as_f64().is_some_and(|r| r > 0.0 && (r as u64) <= now_s) { return String::new() }
        let l = ((1000.0 - used) / 10.0).floor() as u64;
        left = Some(left.map_or(l, |x| x.min(l)));
        shared &= w["shared"].as_bool().unwrap_or(true);
    }
    left.map(|l| format!("{l}% left{}", if shared { " · shared" } else { "" })).unwrap_or_default()
}
/// The allowance snapshot a thread reads: its bound named profile's, else the default login's `allowance.<provider>` setting.
fn allowance_of(st: &State, id: &str, provider: &str) -> Value {
    let get = |k: &str| st.settings.get(k).map(String::as_str).unwrap_or("");
    let bound = serde_json::from_str::<Value>(get(&format!("account.thread.{id}.{provider}"))).ok().and_then(|b| b["id"].as_str().map(str::to_string)).unwrap_or_default();
    if bound.is_empty() { return serde_json::from_str(get(&format!("allowance.{provider}"))).unwrap_or(Value::Null) }
    let p = serde_json::from_str::<Value>(get("accounts.inventory")).ok().and_then(|v| v["profiles"].as_array().cloned()).unwrap_or_default().into_iter().find(|p| p["id"].as_str() == Some(bound.as_str())).unwrap_or(Value::Null);
    profile_allowance(&p, provider)
}
/// A saved account's allowance as a thread reads it: its windows' scopes in the thread's words (five_hour and seven_day are the shared pool).
fn profile_allowance(p: &Value, provider: &str) -> Value {
    let mut a = p["allowance"].clone();
    if a.is_object() { a["provider"] = json!(provider); let ws = a["windows"].as_array().cloned().unwrap_or_default().into_iter().map(|mut w| {
        let sc = w["scope"].as_str().unwrap_or("").to_string(); w["scope"] = json!(match sc.as_str() { "five_hour" | "seven_day" => "shared", "seven_day_opus" => "opus", "seven_day_sonnet" => "sonnet", o => o }); w }).collect::<Vec<_>>(); a["windows"] = json!(ws) }
    a
}
/// The account a thread runs on, as the combined chip names it: (short name, email, saved). A saved account is its label (an email label: its name part); the CLI's
/// own login is its email's name part when its reading says the email, else "default".
fn account_of(st: &State, id: &str, provider: &str, snapshot: &Value) -> (String, String, bool) {
    let get = |k: &str| st.settings.get(k).map(String::as_str).unwrap_or("");
    let short = |s: &str| s.split('@').next().unwrap_or("").to_string();
    let bound = serde_json::from_str::<Value>(get(&format!("account.thread.{id}.{provider}"))).ok().and_then(|b| b["id"].as_str().map(str::to_string)).unwrap_or_default();
    if bound.is_empty() { let e = snapshot["email"].as_str().unwrap_or(""); return (if e.is_empty() { "default".into() } else { short(e) }, e.to_string(), false) }
    let p = serde_json::from_str::<Value>(get("accounts.inventory")).ok().and_then(|v| v["profiles"].as_array().cloned()).unwrap_or_default().into_iter().find(|p| p["id"].as_str() == Some(bound.as_str())).unwrap_or(Value::Null);
    let (label, email) = (p["label"].as_str().unwrap_or(""), p["identity"]["email"].as_str().unwrap_or(""));
    let name = if label.is_empty() { short(email) } else if label.contains('@') { short(label) } else { label.to_string() };
    (if name.is_empty() { "account".into() } else { name }, email.to_string(), true)
}
/// Model.short: the chip's name of a model. Claude's ids read as their family and version ("claude-opus-5-5" → "Opus 5.5", "sonnet" → "Sonnet", a dated id loses its
/// date); another provider's id is as it is; none is "default".
pub fn model_short(model: &str) -> String {
    if model.is_empty() { return "default".into() }
    // a window tag ("[1m]") says the window after the name: "claude-fable-5-2[1m]" is "Fable 5.2 (1M)"
    if let Some(i) = model.find('[').filter(|i| *i > 0) { let n = model_short(&model[..i]); return if n == model[..i] { model.to_string() } else { format!("{n} ({})", model[i..].trim_matches(['[', ']']).to_uppercase()) } }
    let m = model.strip_prefix("claude-").unwrap_or(model);
    let fam = ["opus", "sonnet", "haiku", "fable"];
    let mut parts = m.split('-');
    let first = parts.next().unwrap_or("");
    let base = first.split('[').next().unwrap_or(first);
    if !fam.contains(&base) { return model.to_string() }
    let ver: Vec<&str> = parts.filter(|x| !x.is_empty() && x.len() < 8 && x.chars().all(|c| c.is_ascii_digit())).collect();
    let mut name = format!("{}{}", base[..1].to_uppercase(), &base[1..]);
    if !ver.is_empty() { name = format!("{name} {}", ver.join(".")) }
    format!("{name}{}", &first[base.len()..])
}
/// Limit.tone: the colour a remaining percentage reads in ("" plain, "warn" under a quarter, "bad" under a tenth).
pub fn tone(left: Option<u64>) -> &'static str { match left { Some(l) if l < 10 => "bad", Some(l) if l < 25 => "warn", _ => "" } }
/// The combined chip of the composer (model, effort, account, the binding limit) and what its menu's Usage part lists, from the thread's allowance snapshot
/// (`allowance_of`). `full`/`mid`/`short` are the chip's words at each width; `left` the least remaining percent of the windows that apply (None: no current
/// reading); `windows` every window of the account with its reset (`zone` words it); `extra` the extra-usage line; `tip` all of it for the hover.
pub fn combo(snapshot: &Value, provider: &str, model: &str, effort: &str, account: (&str, &str, bool), zone: &super::tz::Rule, now_s: u64) -> Value {
    let accounts = matches!(provider, "claude" | "codex");
    let left = allowance_text(snapshot, provider, model, now_s).split('%').next().and_then(|x| x.parse::<u64>().ok());
    let (m, e) = (model_short(model), if effort.is_empty() { "auto".to_string() } else { effort.to_string() });
    let mut full = vec![m.clone(), e.clone()]; let mut mid = vec![m.clone()]; let mut short = vec![m.clone()];
    if accounts { full.push(account.0.to_string()); mid.push(account.0.to_string()); }
    if let Some(l) = left { full.push(format!("{l}% left")); mid.push(format!("{l}%")); short.push(format!("{l}%")); }
    let ml = model.to_lowercase();
    let windows: Vec<Value> = if !accounts || snapshot["provider"].as_str().is_some_and(|p| p != provider) { vec![] } else { snapshot["windows"].as_array().into_iter().flatten().filter_map(|w| {
        let used = w["used_permille"].as_f64().map(|p| p / 10.0).or_else(|| w["used_percent"].as_f64()).filter(|u| (0.0..=100.0).contains(u))?;
        let scope = w["scope"].as_str().unwrap_or("");
        let label = w["label"].as_str().map(str::to_string).unwrap_or_else(|| match scope { "five_hour" => "5-hour".into(), "seven_day" | "shared" => "Weekly".into(), "seven_day_opus" | "opus" => "Opus weekly".into(), "seven_day_sonnet" | "sonnet" => "Sonnet weekly".into(), o => w["name"].as_str().unwrap_or(o).to_string() });
        let applies = match w["model"].as_str().filter(|x| !x.is_empty()) { Some(x) => ml.contains(x), None => !matches!(scope, "opus" | "seven_day_opus") || ml.contains("opus") } && (!matches!(scope, "sonnet" | "seven_day_sonnet") || ml.contains("sonnet"));
        let reset = w["resets_at"].as_f64().map(|r| r as u64).filter(|r| *r > now_s);
        let l = ((100.0 - used).floor()).max(0.0) as u64;
        Some(json!({"label":label,"left":l,"tone":tone(Some(l)),"applies":applies,"resets_at":reset,"resets":reset.map(|r| super::thread::clock(r, now_s, zone)).unwrap_or_default()}))
    }).collect() };
    let x = &snapshot["extra"];
    let extra = if x.is_object() { format!("Extra usage {}{}{}", x["used"].as_str().unwrap_or("$0"), x["limit"].as_str().map(|l| format!(" of {l}")).unwrap_or_default(),
        if x["enabled"] == true { String::new() } else { let r = x["reason"].as_str().unwrap_or("").replace('_', " "); if r.is_empty() { " · off".into() } else { format!(" · off ({r})") } }) } else { String::new() };
    let note = if !accounts { String::new() } else if windows.is_empty() { snapshot["reason"].as_str().filter(|r| !r.is_empty()).unwrap_or("No limit reading yet.").to_string() } else { String::new() };
    let mut tip = vec![];
    if accounts { let who = if account.1.is_empty() { account.0.to_string() } else { account.1.to_string() }; tip.push(if account.2 { format!("{who} (saved account)") } else { format!("{who} (the CLI's own login)") }) }
    for w in &windows { tip.push(format!("{}  {}% left{}", w["label"].as_str().unwrap_or(""), w["left"], w["resets"].as_str().filter(|r| !r.is_empty()).map(|r| format!(" · resets {r}")).unwrap_or_default())) }
    if !extra.is_empty() { tip.push(extra.clone()) }
    if !note.is_empty() { tip.push(note.clone()) }
    json!({"model":m,"effort":e,"account":if accounts { account.0 } else { "" },"email":account.1,"saved":account.2,"left":left,"tone":tone(left),
        "full":full.join(" · "),"mid":mid.join(" · "),"short":short.join(" · "),"windows":windows,"extra":extra,"note":note,"tip":tip.join("\n")})
}
/// The combined chip of a thread (`combo`), from the state.
pub fn combo_of(st: &State, id: &str, now_s: u64) -> Value {
    let Some(t) = st.threads.get(id) else { return Value::Null };
    let snap = allowance_of(st, id, &t.provider);
    let acct = account_of(st, id, &t.provider, &snap);
    let effort = st.settings.get(&format!("effort.{id}")).map(String::as_str).unwrap_or("");
    combo(&snap, &t.provider, &t.model, effort, (&acct.0, &acct.1, acct.2), &super::thread::person_zone(st).unwrap_or_else(super::tz::Rule::utc), now_s)
}
/// The phones' picker for a thread on an account provider: an Account section (the CLI's own login, then each saved account of the provider, with what it has
/// left; a press is `thread.pick {action: account, value: "<profile>\u{1f}<epoch>"}`, "" the CLI's own) and a Usage section (the thread's account's windows with
/// their resets, its extra usage; rows to read, not to press). Empty for other providers.
pub fn account_sections(st: &State, id: &str, now_s: u64) -> Vec<Value> {
    let Some(t) = st.threads.get(id) else { return vec![] };
    let provider = t.provider.as_str();
    if !matches!(provider, "claude" | "codex") { return vec![] }
    let get = |k: &str| st.settings.get(k).map(String::as_str).unwrap_or("");
    let bound = serde_json::from_str::<Value>(get(&format!("account.thread.{id}.{provider}"))).ok().and_then(|b| b["id"].as_str().map(str::to_string)).unwrap_or_default();
    let left = |a: &Value| allowance_text(a, provider, &t.model, now_s).split('%').next().and_then(|x| x.parse::<u64>().ok()).map(|l| format!(" · {l}% left")).unwrap_or_default();
    let dflt: Value = serde_json::from_str(get(&format!("allowance.{provider}"))).unwrap_or(Value::Null);
    let mut choices = vec![json!({"label":format!("Default (CLI login){}", if provider == "claude" { left(&dflt) } else { String::new() }),"value":"","on":bound.is_empty(),"ok":true,"why":""})];
    for p in serde_json::from_str::<Value>(get("accounts.inventory")).ok().and_then(|v| v["profiles"].as_array().cloned()).unwrap_or_default().into_iter().filter(|p| p["provider"] == provider) {
        let ok = p["state"] == "connected";
        let why = match p["state"].as_str().unwrap_or("") { "connected" => "", "expired" => "Sign-in expired. Sign in again in Settings, Accounts.", "unchecked" => "Not checked yet.", "connecting" => "Signing in.", _ => "Not signed in." };
        choices.push(json!({"label":format!("{}{}", p["label"].as_str().unwrap_or(""), left(&profile_allowance(&p, provider))),"value":format!("{}\u{1f}{}", p["id"].as_str().unwrap_or(""), p["epoch"].as_u64().unwrap_or(0)),
            "on":p["id"].as_str() == Some(bound.as_str()),"ok":ok,"why":why}));
    }
    let c = combo_of(st, id, now_s);
    let mut usage: Vec<Value> = c["windows"].as_array().into_iter().flatten().map(|w| json!({"label":format!("{}  {}% left", w["label"].as_str().unwrap_or(""), w["left"]),"value":"","on":false,"ok":false,
        "why":w["resets"].as_str().filter(|r| !r.is_empty()).map(|r| format!("resets {r}")).unwrap_or_default()})).collect();
    for k in ["extra", "note"] { if let Some(x) = c[k].as_str().filter(|x| !x.is_empty()) { usage.push(json!({"label":x,"value":"","on":false,"ok":false,"why":""})) } }
    let mut out = vec![json!({"title":"Account","action":"account","choices":choices})];
    if !usage.is_empty() { out.push(json!({"title":"Usage","action":"usage","choices":usage})) }
    out
}
/// The models info as the thread's own account has it: a Claude or Codex thread bound to a saved account (else the provider's default account) lists that account's catalog
/// (`accounts.inventory`, read again each hour), each row with the catalog's label and efforts, so a model the account has shows on the desktop and the phones alike even
/// before the CLI's own list (the picker's detection) names it. No account, or an empty catalog: the info as it is.
pub fn info_for_thread(st: &State, id: &str, info: &Value) -> Value {
    let Some(t) = st.threads.get(id) else { return info.clone() };
    let provider = t.provider.as_str();
    if !matches!(provider, "claude" | "codex") { return info.clone() }
    let get = |k: &str| st.settings.get(k).map(String::as_str).unwrap_or("");
    let bind = |k: String| serde_json::from_str::<Value>(get(&k)).ok().and_then(|b| b["id"].as_str().map(str::to_string)).filter(|b| !b.is_empty());
    let Some(bound) = bind(format!("account.thread.{id}.{provider}")).or_else(|| bind(format!("account.default.{provider}"))) else { return info.clone() };
    let cat = serde_json::from_str::<Value>(get("accounts.inventory")).ok().and_then(|v| v["profiles"].as_array().cloned()).unwrap_or_default().into_iter()
        .find(|p| p["id"].as_str() == Some(bound.as_str()) && p["provider"] == provider).and_then(|p| p["catalog"].as_array().cloned()).unwrap_or_default();
    with_catalog(info, provider, &cat)
}
/// `info` with `provider`'s models, labels and efforts replaced by an account's catalog rows (`{id, label, efforts}`); "default" stays the picker's own first row.
pub fn with_catalog(info: &Value, provider: &str, cat: &[Value]) -> Value {
    let mut out = info.clone();
    let rows: Vec<&Value> = cat.iter().filter(|c| c["id"].as_str().map_or(false, |i| !i.is_empty() && i != "default")).collect();
    if rows.is_empty() { return out }
    out[provider] = Value::Array(rows.iter().map(|c| c["id"].clone()).collect());
    let (mut labels, mut efforts) = (serde_json::Map::new(), serde_json::Map::new());
    for (k, v) in out["about"][provider]["labels"].as_object().into_iter().flatten() { if k == "default" { labels.insert(k.clone(), v.clone()); } }
    for (k, v) in out["about"][provider]["efforts"].as_object().into_iter().flatten() { if k == "default" { efforts.insert(k.clone(), v.clone()); } }
    for c in rows {
        let id = c["id"].as_str().unwrap_or("");
        labels.insert(id.into(), json!(c["label"].as_str().filter(|l| !l.is_empty()).unwrap_or(id)));
        if let Some(e) = c["efforts"].as_array() { efforts.insert(id.into(), json!(e)); }
    }
    if !out["about"][provider].is_object() { out["about"][provider] = json!({"cli":"","labels":{}}) }
    out["about"][provider]["labels"] = Value::Object(labels);
    if provider == "claude" || !efforts.is_empty() { out["about"][provider]["efforts"] = Value::Object(efforts) }
    out
}
/// The agents the picker offers, in its order, with what each is (dim beside its name: 0.14's ModelPick.product).
pub const AGENTS: &[(&str, &str)] = &[("claude", "Claude Code"), ("codex", "Codex CLI"), ("grok", "Grok CLI"), ("muse", "Muse Code"), ("antigravity", "Google"), ("opencode", "OpenCode"), ("pi", "Pi"), ("omp", "Oh My Pi")];
/// Effort.chips.about: the levels `model` of `provider` takes, its default ("") first: those its CLI says that model takes (Claude, Codex, Pi and Oh My Pi
/// report them per model), kept to the levels Backplane passes on for the provider; none when it takes none.
pub fn model_efforts(models: Option<&Value>, provider: &str, model: &str) -> Vec<&'static str> {
    let lv = levels(provider);
    let known = models.and_then(|m| m["about"][provider]["efforts"].get(if model.is_empty() { "default" } else { model })).and_then(Value::as_array);
    let take: Vec<&'static str> = match known { Some(xs) => lv.iter().copied().filter(|l| xs.iter().any(|x| x == l)).collect(), None => lv.to_vec() };
    if take.is_empty() { vec![] } else { std::iter::once("").chain(take).collect() }
}
/// Whether an agent can be used here, and why not: its CLI found (Claude's aliases are built in), the reason it lists nothing.
pub fn agent_ok(models: Option<&Value>, provider: &str) -> (bool, String) {
    let Some(m) = models.filter(|m| m["probed"] == true) else { return (true, String::new()) }; // not asked yet (the built-in list): nothing is known to be missing
    let about = &m["about"][provider];
    let cli = about["cli"].as_str().unwrap_or("");
    if cli.is_empty() && provider != "claude" { return (false, if provider == "antigravity" { "not installed: install Google's Antigravity CLI (agy), or Settings > Agents installs it".into() } else { "not installed on this machine".into() }) }
    (true, about["error"].as_str().unwrap_or("").to_string())
}
/// The chip under the composer: "claude / opus / high", the effort left out for an agent whose model takes none (0.14's View.mp.text).
pub fn chip(provider: &str, model: &str, effort: &str, efforts: bool) -> String {
    format!("{provider} / {}{}", if model.is_empty() { "default" } else { model }, if efforts { format!(" / {}", effort_label(effort)) } else { String::new() })
}
/// Mob.picker.ctx.loc: the model chip and its label, the model menu (the thread's provider's models, each with what its CLI calls it), the efforts its model
/// takes and the agents (with what each is and whether it can be used). Without the hub's models info: the provider's default only.
pub fn picker_with(provider: &str, model: &str, effort: &str, reading: &str, models: Option<&Value>) -> Value {
    let label = format!("{} · {}{}{}", provider, if model.is_empty() { "default" } else { model }, if effort.is_empty() { String::new() } else { format!(" · {}", effort_label(effort)) }, usage_tail(provider, reading));
    let choice = |l: String, v: &str, on: bool| json!({"label": l, "value": v, "on": on});
    let es = model_efforts(models, provider, model);
    let efforts: Vec<Value> = es.iter().map(|e| choice(effort_label(e), e, *e == effort || *e == format!("@{provider}"))).collect();
    let providers: Vec<Value> = AGENTS.iter().map(|(p, what)| { let v = format!("@{p}"); let (ok, why) = agent_ok(models, p);
        json!({"label": p, "value": v, "on": v == effort || *p == provider, "note": if ok { *what } else if why.starts_with("not installed") { "not installed" } else { why.as_str() }, "ok": ok}) }).collect();
    let menu = match models { Some(ms) => model_choices(ms, provider, model), None => json!([{"label": "default", "value": "", "on": model.is_empty()}]) };
    json!({"label": label, "chip": chip(provider, model, effort, !es.is_empty()), "provider": provider, "models": menu, "efforts": efforts, "providers": providers})
}
pub fn picker(provider: &str, model: &str, effort: &str, reading: &str) -> Value { picker_with(provider, model, effort, reading, None) }

/// The model menu once the hub's `models` info is known (Models.menu): the provider's default first, its models (each with its CLI's label as `note`, and the efforts it
/// takes when they differ from the default's), and the thread's own when the list lacks it.
pub fn model_choices(models: &Value, provider: &str, model: &str) -> Value {
    let labels = &models["about"][provider]["labels"];
    let base = model_efforts(Some(models), provider, "");
    let choice = |v: &str| { let mut c = json!({"label": if v.is_empty() { "default" } else { v }, "value": v, "on": v == model, "note": labels[if v.is_empty() { "default" } else { v }].as_str().unwrap_or("")});
        let es = model_efforts(Some(models), provider, v); if es != base { c["efforts"] = json!(es) } c };
    let mut v = vec![choice("")];
    for id in models[provider].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|m| !m.is_empty() && *m != "default") { v.push(choice(id)); }
    if !model.is_empty() && !v.iter().any(|c| c["value"] == model) { v.push(choice(model)); }
    Value::Array(v)
}

/// Every agent's models and efforts, by name: the desktop picker's cursor walks another agent's lists on the client, a key at a time, with no request (the thread's own lists stay in `picker`).
pub fn picker_agents(models: Option<&Value>) -> Value {
    Value::Object(AGENTS.iter().map(|(p, _)| {
        let k = picker_with(p, "", "", "", models);
        let (ok, why) = agent_ok(models, p);
        (p.to_string(), json!({"models": if ok { k["models"].clone() } else { json!([]) }, "efforts": k["efforts"], "ok": ok, "why": why}))
    }).collect())
}
/// The picker as the helper recorded it (the test fixtures): the chip, the agent and each row's note, state and efforts are Backplane's own.
#[cfg(test)]
pub fn as_helper(p: &Value) -> Value {
    let rows = |v: &Value| Value::Array(v.as_array().into_iter().flatten().map(|c| json!({"label":c["label"],"value":c["value"],"on":c["on"]})).collect());
    json!({"label":p["label"],"models":rows(&p["models"]),"efforts":rows(&p["efforts"]),"providers":rows(&p["providers"])})
}
/// `picker_agents` for the models info text in force, made once per text: every thread's picture carries it, and the lists run to hundreds of models.
pub fn picker_agents_of(text: Option<&str>) -> (Option<Arc<Value>>, Value) {
    static LAST: Mutex<Option<(String, Arc<Value>, Value)>> = Mutex::new(None);
    let Some(t) = text else { return (None, picker_agents(None)) };
    let mut g = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((k, m, a)) = g.as_ref() { if k == t { return (Some(m.clone()), a.clone()) } }
    let Ok(m) = serde_json::from_str::<Value>(t) else { return (None, picker_agents(None)) };
    let a = picker_agents(Some(&m)); let m = Arc::new(m);
    *g = Some((t.to_string(), m.clone(), a.clone()));
    (Some(m), a)
}

/// The Context panel for a thread (present/tokstats.rs `usage`), read from its settings; a bot's chat with what feeds its turns and its lanes.
pub fn usage(st: &State, id: &str, now: u64) -> Value {
    use super::tokstats as ts;
    let Some(t) = st.threads.get(id) else { return Value::Null };
    let get = |k: String| st.settings.get(&k).map(String::as_str).unwrap_or("");
    let tok = { let ep = format!("{}{}", get("tok.epoch".into()), get("tok.recount".into())); if ep.is_empty() { "tok.".to_string() } else { format!("tok.{ep}.") } };
    let bot = st.bot_of_thread(id).map(|b| {
        let uber = get("uber.bot".into()) == b;
        let pre = format!("{b}\u{1f}");
        let mems = st.ext.mems.iter().filter(|(k, _)| k.starts_with(&pre)).map(|(_, m)| (m.0.clone(), m.1 as u64)).collect();
        let waiting = st.queue.get(id).into_iter().flatten().filter(|(m, _)| m.starts_with("handoff:")).map(|(m, x)| (crate::state::fam::ext::handoff_why(m).to_string(), x.clone())).collect();
        let lanes = ["runs", "office"].iter().map(|l| { let v = get(format!("{tok}bl.{b}|{l}")).to_string();
            let mut it = v.split('|').skip(2); let (pv, acct) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
            (l.to_string(), v.clone(), account_name(st, pv, acct)) }).collect();
        ts::Bot { name: st.bots_nav.get(&b).map(|n| n.name.clone()).unwrap_or_default(), uber, routines_filed: uber || get("mail.on".into()) == "on", mems, waiting, lanes }
    });
    let lv = live(st, id); let sess = st.ext.sess.get(id).cloned().unwrap_or_default();
    let x = ts::In { provider: &t.provider, model: &t.model, usage: get(format!("usage.{id}")), last: get(format!("turn.last.{id}")), tv: get(format!("{tok}t.{id}")), speed: get(format!("turn.speed.{id}")),
        models: get(format!("turn.models.{id}")), live: &lv, ring: get(format!("{tok}tr.{id}")), today: get(format!("{tok}td.{id}")), day: &crate::state::fam::day_of(now),
        began: sess.began, handed: (sess.handed, &sess.why), now, running: t.turn == crate::state::Turn::Running, bot };
    ts::usage(&x)
}
/// An account's name for the panel: a saved account's label, else the CLI's own login
fn account_name(st: &State, provider: &str, id: &str) -> String {
    if provider.is_empty() { return String::new() }
    if id.is_empty() { let a = serde_json::from_str::<Value>(st.settings.get(&format!("allowance.{provider}")).map(String::as_str).unwrap_or("")).unwrap_or(Value::Null); return a["email"].as_str().and_then(|e| e.split('@').next()).filter(|e| !e.is_empty()).unwrap_or("default login").to_string() }
    let inv = st.settings.get("accounts.inventory").map(String::as_str).unwrap_or("");
    let p = serde_json::from_str::<Value>(inv).ok().and_then(|v| v["profiles"].as_array().cloned()).unwrap_or_default().into_iter().find(|p| p["id"].as_str() == Some(id)).unwrap_or(Value::Null);
    let l = p["label"].as_str().filter(|l| !l.is_empty()).or_else(|| p["identity"]["email"].as_str()).unwrap_or("saved account");
    l.split('@').next().unwrap_or(l).to_string()
}

/// The running turn's numbers so far (`turn.live.<t>`, agents/lines.rs), "" unless the thread's turn is running.
fn live(st: &State, id: &str) -> String {
    if st.threads.get(id).map_or(true, |t| t.turn != crate::state::Turn::Running) { return String::new() }
    st.settings.get(&format!("turn.live.{id}")).cloned().unwrap_or_default()
}

/// The composer's token button for a thread (present/tokstats.rs `chip`), from the same settings as its panel.
pub fn tok_chip(st: &State, id: &str) -> String {
    if !st.threads.contains_key(id) { return String::new() }
    let get = |k: String| st.settings.get(&k).map(String::as_str).unwrap_or("");
    let tok = { let ep = format!("{}{}", get("tok.epoch".into()), get("tok.recount".into())); if ep.is_empty() { "tok.".to_string() } else { format!("tok.{ep}.") } };
    super::tokstats::chip(get(format!("usage.{id}")), get(format!("{tok}t.{id}")), &live(st, id))
}

/// `state`, `tools`, `menu`, `working`, `phase` for a thread at clock `now_s`, or None for a case not computed here.
pub fn keys(st: &State, id: &str, now_s: u64, why: &str, models_text: Option<&str>) -> Option<Map<String, Value>> {
    let t = st.threads.get(id)?;
    // a lean State may have dropped the fork record: then "forked" cannot be told
    let fork_key = format!("fork.{id}");
    if st.lean && !crate::state::fam::policy::lean_keeps(&fork_key) { return None }
    let forked = st.settings.get(&fork_key).map_or(false, |v| !v.is_empty());
    let n = plain_signal(st, id)?;
    let phase = signal::display_name(st, n, id);
    // Signal.label.shown: how the last turn ended says more than the signal when nothing else is going on
    let shown = signal::shown_k(signal::name(n), t.turn, t.pending);
    // Signal.display.note: a stalled thread says why, in the liveness record's own words
    let stalled_reason = st.settings.get(&format!("liveness.{id}")).and_then(|v| serde_json::from_str::<Value>(v).ok()).and_then(|v| v["reason"].as_str().map(str::to_string)).unwrap_or_default();
    let working = if phase == "stalled" { format!("Stalled: {stalled_reason}") } else { match shown {
        "stopped" => "Stopped: nothing is running".to_string(),
        "failed" => format!("Failed{}{}", if why.is_empty() { String::new() } else { format!(": {why}") }, if signal::active(n) { "; work also active" } else { "" }),
        _ => label(phase)?.to_string(),
    } };
    let until = st.snooze_until.get(id).copied().unwrap_or(0);
    let mut m = Map::new();
    m.insert("state".into(), json!(state_word(t.turn)));
    m.insert("tools".into(), Value::Array(tools(t.turn, t.pinned, settled(st, id), t.archived, forked)));
    m.insert("menu".into(), Value::Array(menu(id, now_s < until)));
    m.insert("working".into(), json!(working)); m.insert("phase".into(), json!(phase));
    m.insert("permissions".into(), permissions(st.runtime.get(id).map(String::as_str).unwrap_or("")));
    let get = |k: String| st.settings.get(&k).map(String::as_str).unwrap_or("");
    let (models, agents) = picker_agents_of(models_text);
    let mine = models.as_deref().map(|i| info_for_thread(st, id, i));
    m.insert("picker".into(), picker_with(&t.provider, &t.model, get(format!("effort.{id}")), get(format!("usage.{id}")), mine.as_ref()));
    m.insert("pickerAgents".into(), agents);
    m.insert("claude".into(), json!(t.provider == "claude"));
    m.insert("allowance".into(), json!(allowance_text(&allowance_of(st, id, &t.provider), &t.provider, &t.model, now_s)));
    m.insert("combo".into(), combo_of(st, id, now_s));
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn allowance_is_the_least_remaining_window_and_never_invented() {
        let snap = json!({"provider":"claude","windows":[{"scope":"shared","shared":true,"used_permille":280},{"scope":"shared","shared":true,"used_permille":420},{"scope":"opus","shared":true,"used_permille":900}]});
        assert_eq!(allowance_text(&snap, "claude", "claude-sonnet-5-5", 100), "58% left · shared");
        assert_eq!(allowance_text(&snap, "claude", "claude-opus-5-5", 100), "10% left · shared");
        assert_eq!(allowance_text(&Value::Null, "claude", "", 100), "");
        assert_eq!(allowance_text(&json!({"windows":[{"scope":"shared","used_permille":null}]}), "claude", "", 100), "");
        assert_eq!(allowance_text(&json!({"windows":[{"scope":"shared","used_permille":100,"resets_at":50}]}), "claude", "", 100), "", "a passed reset is no current reading");
        assert_eq!(allowance_text(&snap, "grok", "", 100), "");
    }
    #[test] fn a_models_short_name_is_its_family_and_version() {
        for (id, want) in [("", "default"), ("claude-opus-5-5", "Opus 5.5"), ("opus", "Opus"), ("sonnet", "Sonnet"), ("claude-sonnet-4-5-20250929", "Sonnet 4.5"), ("claude-fable-5-1", "Fable 5.1"), ("opus[1m]", "Opus (1M)"), ("claude-fable-5-2[1m]", "Fable 5.2 (1M)"), ("gpt-5.1-codex", "gpt-5.1-codex"), ("claude-mystery-1", "claude-mystery-1")] {
            assert_eq!(model_short(id), want, "{id}")
        }
    }
    /// The recorded reading of a saved Max account (64% of the week used, nothing in the 5-hour window, a Fable weekly limit, extra usage off).
    fn reading() -> Value { json!({"provider":"claude","state":"ok","source":"claude-oauth-usage","windows":[
        {"scope":"shared","label":"Weekly","used_percent":64.0,"resets_at":1_794_416_400u64},
        {"scope":"seven_day_fable","label":"Fable weekly","model":"fable","used_percent":0.0,"resets_at":1_794_416_400u64}],
        "extra":{"enabled":false,"used":"$0.00","limit":"$50.00","percent":0.0,"reason":"out_of_credits"}}) }
    #[test] fn the_one_chip_says_model_effort_account_and_the_binding_limit_at_every_width() {
        let now = 1_794_175_561; let utc = super::super::tz::Rule::utc();
        let c = combo(&reading(), "claude", "claude-opus-5-5", "", ("kellyhlord", "kellyhlord@gmail.com", true), &utc, now);
        assert_eq!((c["full"].as_str().unwrap(), c["mid"].as_str().unwrap(), c["short"].as_str().unwrap()), ("Opus 5.5 · auto · kellyhlord · 36% left", "Opus 5.5 · kellyhlord · 36%", "Opus 5.5 · 36%"));
        assert_eq!((c["left"].clone(), c["tone"].clone()), (json!(36), json!("")));
        assert_eq!(c["windows"][0], json!({"label":"Weekly","left":36,"tone":"","applies":true,"resets_at":1_794_416_400u64,"resets":"Wed 11 Nov 17:00"}));
        assert_eq!((c["windows"][1]["label"].clone(), c["windows"][1]["applies"].clone()), (json!("Fable weekly"), json!(false)), "Fable's own limit does not bind Opus");
        assert_eq!(c["extra"], "Extra usage $0.00 of $50.00 · off (out of credits)");
        assert_eq!(c["tip"], "kellyhlord@gmail.com (saved account)\nWeekly  36% left · resets Wed 11 Nov 17:00\nFable weekly  100% left · resets Wed 11 Nov 17:00\nExtra usage $0.00 of $50.00 · off (out of credits)");
        // a Fable thread is bound by the fuller of the two; low and out read in their colours
        let mut r = reading(); r["windows"][1]["used_percent"] = json!(95.5);
        let f = combo(&r, "claude", "claude-fable-5-1", "high", ("default", "", false), &utc, now);
        assert_eq!((f["full"].as_str().unwrap(), f["tone"].as_str().unwrap()), ("Fable 5.1 · high · default · 4% left", "bad"));
        assert_eq!((tone(Some(24)), tone(Some(25)), tone(Some(0)), tone(None)), ("warn", "", "bad", ""));
        // no reading: no number is made up, and the tip says why
        let n = combo(&json!({"provider":"claude","state":"unavailable","windows":[],"reason":"Claude is limiting usage reads right now (429); the last reading stays."}), "claude", "", "", ("work", "w@x.com", true), &utc, now);
        assert_eq!((n["full"].as_str().unwrap(), n["short"].as_str().unwrap(), n["left"].clone()), ("default · auto · work", "default", Value::Null));
        assert!(n["tip"].as_str().unwrap().ends_with("(429); the last reading stays."));
        // an agent with no accounts: model and effort only
        let g = combo(&Value::Null, "grok", "grok-4", "low", ("default", "", false), &utc, now);
        assert_eq!((g["full"].as_str().unwrap(), g["account"].as_str().unwrap(), g["tip"].as_str().unwrap()), ("grok-4 · low", "", ""));
    }
    #[test] fn a_thread_reads_its_bound_accounts_numbers_or_the_cli_logins() {
        let mut st = State::new();
        st.apply(&json!({"$":"ThreadCreated","id":"t1","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"claude-opus-5-5","at":1}));
        let inv = json!({"version":1,"revision":1,"profiles":[{"id":"k","provider":"claude","label":"kellyhlord@gmail.com","epoch":2,"state":"connected","identity":{"email":"kellyhlord@gmail.com"},"allowance":{"state":"ok","windows":[{"scope":"seven_day","label":"Weekly","used_percent":64.0}]}},
            {"id":"w","provider":"claude","label":"Work","epoch":1,"state":"expired","identity":{"email":"w@x.com"},"allowance":null}]});
        for (k, v) in [("accounts.inventory", inv.to_string()), ("allowance.claude", json!({"provider":"claude","state":"ok","email":"me@home.org","windows":[{"scope":"five_hour","used_percent":10.0},{"scope":"seven_day","used_percent":37.0}]}).to_string())] {
            st.apply(&json!({"$":"SettingSet","key":k,"value":v,"at":2}));
        }
        let c = combo_of(&st, "t1", 100); assert_eq!(c["full"], "Opus 5.5 · auto · me · 63% left", "the CLI's own login: its email's name");
        st.apply(&json!({"$":"SettingSet","key":"account.thread.t1.claude","value":"{\"id\":\"k\",\"epoch\":2}","at":3}));
        let c = combo_of(&st, "t1", 100); assert_eq!(c["full"], "Opus 5.5 · auto · kellyhlord · 36% left"); assert_eq!(c["email"], "kellyhlord@gmail.com");
        let a = account_sections(&st, "t1", 100);
        assert_eq!(a[0]["choices"], json!([{"label":"Default (CLI login) · 63% left","value":"","on":false,"ok":true,"why":""},{"label":"kellyhlord@gmail.com · 36% left","value":"k\u{1f}2","on":true,"ok":true,"why":""},
            {"label":"Work","value":"w\u{1f}1","on":false,"ok":false,"why":"Sign-in expired. Sign in again in Settings, Accounts."}]));
        assert_eq!((a[1]["title"].clone(), a[1]["choices"][0]["label"].clone(), a[1]["choices"][0]["ok"].clone()), (json!("Usage"), json!("Weekly  36% left"), json!(false)));
        st.apply(&json!({"$":"ThreadCreated","id":"t2","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"grok","model":"","at":1})); assert!(account_sections(&st, "t2", 100).is_empty());
    }
    /// a choice list without the fields the helper did not have (`note`, `efforts`)
    fn strip(v: &Value) -> Value { Value::Array(v.as_array().unwrap().iter().map(|c| json!({"label":c["label"],"value":c["value"],"on":c["on"]})).collect()) }
    fn fx() -> Value { serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-variants.json"))).unwrap() }
    const T: &str = "t1700000003-8";
    /// every variant the helper was dumped for: a key not in a variant's diff is the base thread's (the helper's own dump of it)
    #[test] fn each_variants_state_keys_are_the_helpers() {
        let v = fx(); let log = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl"));
        let pair: Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-pair.json"))).unwrap();
        let base = &pair["out"]["shown"]["thread"];
        let mut checked = 0;
        for (name, x) in v.as_object().unwrap() {
            if x["thread"].is_null() && x.get("diff").is_none() { continue }
            let mut st = State::new(); for l in log.lines() { st.apply_line(l); }
            for e in x["extra"].as_array().unwrap() { st.apply(e); }
            let Some(got) = keys(&st, T, 1_700_100_000, "", None) else { panic!("{name} should be computed"); };
            let diff = x["diff"].as_object().unwrap();
            for k in ["state", "tools", "menu", "working", "phase", "claude", "permissions"] { assert_eq!(Some(&got[k]), diff.get(k).or(base.get(k)), "{name}: {k}"); }
            // the picker is the helper's, plus what 0.14's web picker drew beside it (the chip, each agent's product and state)
            let want = diff.get("picker").or(base.get("picker")).unwrap();
            assert_eq!(got["picker"]["label"], want["label"], "{name}: picker.label");
            for k in ["models", "efforts"] { assert_eq!(strip(&got["picker"][k]), want[k], "{name}: picker.{k}"); }
            assert_eq!(got["picker"]["providers"].as_array().unwrap().iter().map(|p| json!({"label":p["label"],"value":p["value"],"on":p["on"]})).collect::<Vec<_>>(), want["providers"].as_array().unwrap().clone(), "{name}: picker.providers");
            checked += 1;
        }
        assert!(checked >= 14, "{checked}");
    }
    #[test] fn permission_modes_and_the_context_tail() {
        assert_eq!(permissions("approval-required")["current"], "Permissions: Approval required");
        assert_eq!(permissions("")["choices"][3]["on"], true);
        assert_eq!(usage_tail("claude", ""), ""); assert_eq!(usage_tail("codex", ""), " · ctx n/a");
        assert_eq!(usage_tail("claude", "45000/200000"), " · ctx 22% of 200k"); assert_eq!(usage_tail("claude", "900/0"), " · ctx 900");
        let p = picker("claude", "claude-opus-4", "high", "");
        let a = picker_agents(Some(&json!({"probed": true, "codex": ["gpt-6"], "about": {"codex": {"cli": "codex 1"}}})));
        assert_eq!(a["codex"]["models"].as_array().unwrap().iter().map(|m| m["value"].as_str().unwrap()).collect::<Vec<_>>(), ["", "gpt-6"], "the agent's default, then its models");
        assert_eq!(a["claude"]["efforts"][0]["value"], "", "auto first"); assert_eq!(a["grok"]["models"], json!([]), "an agent that is not installed lists nothing"); assert_eq!(a["grok"]["ok"], false);
        assert_eq!(picker_agents(None)["codex"]["models"].as_array().unwrap().len(), 1, "only the default before the hub has heard its models");
        assert_eq!(p["label"], "claude · claude-opus-4 · high"); assert_eq!(p["efforts"][3]["on"], true); assert_eq!(p["models"][0]["on"], false);
    }
    /// what a machine's CLIs reported (settings/models_probe.rs's shape): Claude with per-model efforts, Codex installed with labels, Grok not installed
    fn info() -> Value {
        json!({"probed": true, "claude": ["opus", "haiku"], "codex": ["gpt-6.1-sol", "gpt-6-luna"], "grok": [], "muse": [], "antigravity": [], "opencode": [], "pi": [], "omp": [],
            "about": {"claude": {"cli": "2.1", "labels": {"default": "Claude Code's own default", "opus": "Opus 5.5 · claude-opus-5-5", "haiku": "Haiku 4.5 · claude-haiku-4-5"}, "efforts": {"default": ["low", "high"], "opus": ["low", "high"], "haiku": []}},
                "codex": {"cli": "codex 1.2", "labels": {"default": "Codex's configured default", "gpt-6.1-sol": "GPT-6.1-Sol · Latest workhorse"}, "efforts": {"gpt-6.1-sol": ["low", "medium", "high", "xhigh", "max", "ultra"]}},
                "grok": {"cli": "", "labels": {}}, "opencode": {"cli": "1.0 · lists no models", "error": "lists no models: sign in with `opencode auth login`", "labels": {}}}})
    }
    #[test] fn every_agent_lists_its_models_in_a_bot_chat_with_the_info_and_default_only_without() {
        let mut m = info();
        for p in ["claude", "codex", "grok", "opencode", "muse", "antigravity", "pi", "omp"] { if !m[p].is_array() { m[p] = json!(["m1", "m2"]) } }
        for (p, _) in AGENTS {
            let with = picker_with(p, "", "", "", Some(&m));
            let n = m[*p].as_array().map_or(0, |a| a.iter().filter(|x| x.as_str().map_or(false, |s| !s.is_empty() && s != "default")).count());
            assert_eq!(with["models"].as_array().unwrap().len(), 1 + n, "{p}: default and its real models");
            assert_eq!(picker_with(p, "", "", "", None)["models"].as_array().unwrap().len(), 1, "{p}: no info is the bare default (the bug)");
            let a = picker_agents(Some(&m)); assert_eq!(a[*p]["models"].as_array().map_or(0, Vec::len) > 1 || n == 0 || a[*p]["ok"] == false, true, "{p}: agents list");
        }
    }
    #[test] fn an_accounts_catalog_lays_over_the_detected_list() {
        let info = json!({"claude":["fable","opus"],"about":{"claude":{"cli":"2","labels":{"default":"d","fable":"Fable 5.1"},"efforts":{"fable":["low"]}}}});
        let cat = vec![json!({"id":"default","label":"Default"}), json!({"id":"claude-fable-5-1[1m]","label":"Fable 5.1","efforts":["low","high"]}), json!({"id":"claude-fable-5-2[1m]","label":"Fable 5.2","efforts":["low","high"]})];
        let m = with_catalog(&info, "claude", &cat);
        assert_eq!(m["claude"], json!(["claude-fable-5-1[1m]","claude-fable-5-2[1m]"]));
        let p = picker_with("claude", "claude-fable-5-2[1m]", "high", "", Some(&m));
        assert!(p["models"].as_array().unwrap().iter().any(|c| c["value"] == "claude-fable-5-2[1m]" && c["note"] == "Fable 5.2"));
        assert_eq!(model_efforts(Some(&m), "claude", "claude-fable-5-2[1m]"), ["", "low", "high"]);
        assert_eq!(with_catalog(&info, "claude", &[]), info, "no catalog: the detected list");
    }
    #[test] fn the_picker_lists_each_agents_models_with_labels_and_the_efforts_its_model_takes() {
        let m = info();
        let p = picker_with("claude", "opus", "high", "", Some(&m));
        assert_eq!(p["models"].as_array().unwrap().iter().map(|c| (c["label"].as_str().unwrap(), c["note"].as_str().unwrap(), c["on"].as_bool().unwrap())).collect::<Vec<_>>(),
            [("default", "Claude Code's own default", false), ("opus", "Opus 5.5 · claude-opus-5-5", true), ("haiku", "Haiku 4.5 · claude-haiku-4-5", false)]);
        assert_eq!(p["efforts"].as_array().unwrap().iter().map(|c| c["value"].as_str().unwrap()).collect::<Vec<_>>(), ["", "low", "high"], "the model's own levels, auto first");
        assert_eq!(p["models"][2]["efforts"], json!([]), "a model that takes no effort says so"); assert!(p["models"][1].get("efforts").is_none(), "the same as the default's: not repeated");
        assert_eq!(p["chip"], "claude / opus / high"); assert_eq!(p["provider"], "claude");
        assert_eq!(picker_with("claude", "haiku", "", "", Some(&m))["chip"], "claude / haiku", "no effort for a model that takes none"); assert_eq!(picker_with("claude", "haiku", "", "", Some(&m))["efforts"], json!([]));
    }
    #[test] fn each_agent_state_shows_in_the_agent_column() {
        let m = info(); let p = picker_with("codex", "gpt-6.1-sol", "xhigh", "", Some(&m));
        let row = |n: &str| p["providers"].as_array().unwrap().iter().find(|r| r["label"] == n).unwrap().clone();
        assert_eq!((row("codex")["on"].clone(), row("codex")["ok"].clone(), row("codex")["note"].clone()), (json!(true), json!(true), json!("Codex CLI")), "installed: what it is");
        assert_eq!((row("grok")["ok"].clone(), row("grok")["note"].clone()), (json!(false), json!("not installed")), "short beside its name; the model column says the rest");
        assert_eq!(row("antigravity")["note"], "not installed"); assert_eq!(picker_agents(Some(&m))["antigravity"]["why"], "not installed: install Google's Antigravity CLI (agy), or Settings > Agents installs it"); assert_eq!(row("claude")["ok"], true);
        assert_eq!(row("opencode")["ok"], true, "installed though it lists nothing: Settings says why");
        let mut unasked = m.clone(); unasked["probed"] = json!(false); assert_eq!(agent_ok(Some(&unasked), "grok"), (true, String::new()), "before the CLIs are asked nothing is missing");
        assert_eq!(p["efforts"].as_array().unwrap().iter().map(|c| c["value"].as_str().unwrap()).collect::<Vec<_>>(), ["", "low", "medium", "high", "xhigh"], "kept to the levels Backplane passes to Codex");
        assert_eq!(p["chip"], "codex / gpt-6.1-sol / xhigh"); assert_eq!(p["models"][1]["note"], "GPT-6.1-Sol · Latest workhorse");
        assert_eq!(picker_with("opencode", "", "", "", Some(&m))["chip"], "opencode / default", "an agent with no efforts");
        let a = picker_agents(Some(&m)); assert_eq!(a["opencode"]["why"], "lists no models: sign in with `opencode auth login`"); assert_eq!(a["codex"]["models"].as_array().unwrap().len(), 3);
        let (parsed, cached) = picker_agents_of(Some(&m.to_string())); assert_eq!(cached, a); assert!(parsed.is_some()); assert_eq!(picker_agents_of(Some(&m.to_string())).1, a, "made once per text");
    }
    #[test] fn the_model_menu_lists_the_providers_models_default_first_and_keeps_the_threads_own() {
        let ms = json!({"claude": ["fable", "opus"], "codex": []});
        let v = model_choices(&ms, "claude", "opus"); let l: Vec<(String, bool)> = v.as_array().unwrap().iter().map(|c| (c["value"].as_str().unwrap().to_string(), c["on"].as_bool().unwrap())).collect();
        assert_eq!(l, [("".to_string(), false), ("fable".to_string(), false), ("opus".to_string(), true)]);
        let v = model_choices(&ms, "codex", "gpt-x"); assert_eq!(v.as_array().unwrap().len(), 2); assert_eq!(v[1]["value"], "gpt-x");
        assert_eq!(model_choices(&ms, "grok", "")[0]["on"], true);
    }
}
