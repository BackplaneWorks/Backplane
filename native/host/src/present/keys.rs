//! The keys of `shown.thread` that follow the thread's own state (src/mobile/screen.bend `Mob.thread`): `state`, `tools` (core/tools.bend), `menu`,
//! `working` and `phase` (core/signal.bend), computed from `state::State` for a thread with no asks, task links, queue, background rows or
//! launch (`plain_signal` is `signal::own` for exactly that case). `None`: a case this does not compute (a failed or stopped turn, a stalled thread,
//! a forked thread whose fork record a lean State may have dropped): the caller refuses it, retryably.
use crate::shelf::{shelf_of, Shelf};
use crate::signal;
use crate::state::{Override, State, Turn};
use serde_json::{json, Map, Value};

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
        "omp" => &["off", "auto", "minimal", "low", "medium", "high", "xhigh", "max"], _ => &[] }
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
        let applies = provider == "codex" || scope == "shared" || scope == "five_hour" || scope == "seven_day" || (scope == "opus" && m.contains("opus")) || (scope == "sonnet" && m.contains("sonnet"));
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
    let mut a = p["allowance"].clone();
    if a.is_object() { a["provider"] = json!(provider); let ws = a["windows"].as_array().cloned().unwrap_or_default().into_iter().map(|mut w| {
        let sc = w["scope"].as_str().unwrap_or("").to_string(); w["scope"] = json!(match sc.as_str() { "five_hour" | "seven_day" => "shared", "seven_day_opus" => "opus", "seven_day_sonnet" => "sonnet", o => o }); w }).collect::<Vec<_>>(); a["windows"] = json!(ws) }
    a
}
/// Mob.picker.ctx.loc with no models info: the model chip, the model menu (the thread's own model only: the hub's model list is not here), the efforts the
/// provider takes and the providers. This is what the helper draws before it has heard the hub's models.
pub fn picker(provider: &str, model: &str, effort: &str, reading: &str) -> Value {
    let label = format!("{} · {}{}{}", provider, if model.is_empty() { "default" } else { model }, if effort.is_empty() { String::new() } else { format!(" · {}", effort_label(effort)) }, usage_tail(provider, reading));
    let choice = |l: String, v: &str, on: bool| json!({"label": l, "value": v, "on": on});
    let lv = levels(provider);
    let efforts: Vec<Value> = if lv.is_empty() { vec![] } else { std::iter::once("").chain(lv.iter().copied()).map(|e| choice(effort_label(e), e, e == effort || e == format!("@{provider}"))).collect() };
    let providers: Vec<Value> = ["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"].iter().map(|p| { let v = format!("@{p}"); choice((*p).into(), &v, v == effort || *p == provider) }).collect();
    json!({"label": label, "models": [choice("default".into(), "", model.is_empty())], "efforts": efforts, "providers": providers})
}

/// The model menu once the hub's `models` info is known (Models.menu): the provider's default first, its models, and the thread's own when the list lacks it.
pub fn model_choices(models: &Value, provider: &str, model: &str) -> Value {
    let choice = |l: &str, v: &str| json!({"label": if l.is_empty() { "default" } else { l }, "value": v, "on": v == model});
    let mut v = vec![choice("", "")];
    for id in models[provider].as_array().into_iter().flatten().filter_map(Value::as_str) { v.push(choice(id, id)); }
    if !model.is_empty() && !v.iter().any(|c| c["value"] == model) { v.push(choice(model, model)); }
    Value::Array(v)
}

/// The Context panel's sections for a thread (present/tokstats.rs), read from its settings.
pub fn stats(st: &State, id: &str) -> Value {
    let Some(t) = st.threads.get(id) else { return Value::Null };
    let get = |k: String| st.settings.get(&k).map(String::as_str).unwrap_or("");
    let tok = { let ep = format!("{}{}", get("tok.epoch".into()), get("tok.recount".into())); if ep.is_empty() { "tok.".to_string() } else { format!("tok.{ep}.") } };
    Value::Array(super::tokstats::sections(&t.provider, get(format!("usage.{id}")), get(format!("turn.last.{id}")), get(format!("{tok}t.{id}")), get(format!("turn.speed.{id}")), get(format!("turn.models.{id}"))))
}

/// `state`, `tools`, `menu`, `working`, `phase` for a thread at clock `now_s`, or None for a case not computed here.
pub fn keys(st: &State, id: &str, now_s: u64, why: &str, models: Option<&Value>) -> Option<Map<String, Value>> {
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
    let mut pk = picker(&t.provider, &t.model, get(format!("effort.{id}")), get(format!("usage.{id}")));
    if let Some(ms) = models { pk["models"] = model_choices(ms, &t.provider, &t.model); }
    m.insert("picker".into(), pk);
    m.insert("claude".into(), json!(t.provider == "claude"));
    m.insert("allowance".into(), json!(allowance_text(&allowance_of(st, id, &t.provider), &t.provider, &t.model, now_s)));
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
    fn fx() -> Value { serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/thread-variants.json"))).unwrap() }
    const T: &str = "t1700000003-8";
    /// every variant the helper was dumped for: a key not in a variant's diff is the base thread's (the helper's own dump of it)
    #[test] fn each_variants_state_keys_are_the_helpers() {
        let v = fx(); let log = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/thread-log.jsonl"));
        let pair: Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/thread-pair.json"))).unwrap();
        let base = &pair["out"]["shown"]["thread"];
        let mut checked = 0;
        for (name, x) in v.as_object().unwrap() {
            if x["thread"].is_null() && x.get("diff").is_none() { continue }
            let mut st = State::new(); for l in log.lines() { st.apply_line(l); }
            for e in x["extra"].as_array().unwrap() { st.apply(e); }
            let Some(got) = keys(&st, T, 1_700_100_000, "", None) else { panic!("{name} should be computed"); };
            let diff = x["diff"].as_object().unwrap();
            for k in ["state", "tools", "menu", "working", "phase", "picker", "claude", "permissions"] { assert_eq!(Some(&got[k]), diff.get(k).or(base.get(k)), "{name}: {k}"); }
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
        assert_eq!(p["label"], "claude · claude-opus-4 · high"); assert_eq!(p["efforts"][3]["on"], true); assert_eq!(p["models"][0]["on"], false);
    }
    #[test] fn the_model_menu_lists_the_providers_models_default_first_and_keeps_the_threads_own() {
        let ms = json!({"claude": ["fable", "opus"], "codex": []});
        let v = model_choices(&ms, "claude", "opus"); let l: Vec<(String, bool)> = v.as_array().unwrap().iter().map(|c| (c["value"].as_str().unwrap().to_string(), c["on"].as_bool().unwrap())).collect();
        assert_eq!(l, [("".to_string(), false), ("fable".to_string(), false), ("opus".to_string(), true)]);
        let v = model_choices(&ms, "codex", "gpt-x"); assert_eq!(v.as_array().unwrap().len(), 2); assert_eq!(v[1]["value"], "gpt-x");
        assert_eq!(model_choices(&ms, "grok", "")[0]["on"], true);
    }
}
