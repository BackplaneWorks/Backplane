//! The fallback chain: one ordered, opt-in list of `provider/model` per hub. Never wraps: an exhausted suffix blocks until a
//! person intervenes. Ported here: parsing, validity against the installed models, editing and picking; the outage classifier (`classify`: only a provider's own
//! error envelope, never arbitrary text); holds on a provider or model; and the planner over named accounts (core/account-fallback: which account is
//! eligible, the one with the most allowance left, when a hold ends, the thread's preference). What is NOT here is the driver: reading a failed turn,
//! writing the hold, switching the thread to the next account or provider and starting it again is settings/driver.rs, which the agents' actor calls.
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Target { pub provider: String, pub model: String }
impl Target {
    pub fn key(&self) -> String { format!("{}/{}", self.provider, self.model) }
    /// Target.parse: the provider is the text before the first `/`, the model everything after it.
    pub fn parse(s: &str) -> Target { let mut p = s.split('/'); let provider = p.next().unwrap_or("").to_string(); Target { provider, model: p.collect::<Vec<_>>().join("/") } }
}

/// Chain.parse
pub fn parse(s: &str) -> Vec<Target> { if s.is_empty() { vec![] } else { s.split(',').map(Target::parse).collect() } }
/// Chain.show
pub fn show(xs: &[Target]) -> String { xs.iter().map(Target::key).collect::<Vec<_>>().join(",") }
pub fn has(xs: &[Target], key: &str) -> bool { xs.iter().any(|t| t.key() == key) }

/// Caps.valid: the provider lists models and the target names one of them (or none).
pub fn caps_valid(caps: &Value, t: &Target) -> bool {
    let xs = caps.get(&t.provider).and_then(Value::as_array).cloned().unwrap_or_default();
    !xs.is_empty() && (t.model.is_empty() || xs.iter().any(|m| m.as_str() == Some(t.model.as_str())))
}
/// Chain.valid: every target is offered, none twice.
pub fn valid(xs: &[Target], caps: &Value) -> bool { xs.iter().enumerate().all(|(i, t)| caps_valid(caps, t) && !has(&xs[i + 1..], &t.key())) }

/// Chain.remove / Chain.up (the Settings lists' edits).
pub fn remove(xs: &[Target], key: &str) -> Vec<Target> { xs.iter().filter(|t| t.key() != key).cloned().collect() }
pub fn up(xs: &[Target], key: &str) -> Vec<Target> { let mut v = xs.to_vec(); if let Some(i) = v.iter().position(|t| t.key() == key) { if i > 0 { v.swap(i, i - 1) } } v }
/// Chain.suffix: what follows `key` when it is in the chain, else the whole chain.
pub fn suffix(xs: &[Target], key: &str) -> Vec<Target> { match xs.iter().position(|t| t.key() == key) { Some(i) => xs[i + 1..].to_vec(), None => xs.to_vec() } }


// ---- outages: what counts as a quota signal, and a held scope ----
use serde_json::json;

/// Limit: a hold on a provider login (scope = the provider) or on a model (scope = `provider/model`). Unknown reset (0) stays held until cleared;
/// no retry time is invented. No credential or account identifier is stored.
/// `auth`: the login failed rather than ran out; it is held the same way, and only a fresh sign-in check or the person's own choice ends it.
#[derive(Clone, Debug, PartialEq)]
pub struct Limit { pub scope: String, pub reason: String, pub reset: u64, pub auth: bool }
impl Limit {
    /// Limit.json: the record kept as the setting `fallback.outage.<scope>` (the reason cut to 400 characters).
    pub fn json(&self, now: u64) -> String { let mut o = json!({"scope":self.scope,"reason":self.reason.chars().take(400).collect::<String>(),"reset":self.reset,"at":now}); if self.auth { o["auth"] = json!(true) } o.to_string() }
    pub fn key(scope: &str) -> String { format!("fallback.outage.{scope}") }
}
/// Limit.active.o: a record holds from its start until its reset; an unknown reset (0) holds until cleared.
pub fn limit_active(o: &Value, now: u64) -> bool { !o["scope"].as_str().unwrap_or("").is_empty() && (o["reset"].as_u64().unwrap_or(0) == 0 || now < o["reset"].as_u64().unwrap_or(0)) }
/// Target.held: the provider or the `provider/model` is held (`get` reads a setting).
pub fn target_held(get: &dyn Fn(&str) -> String, t: &Target, now: u64) -> bool {
    let rec = |scope: &str| serde_json::from_str::<Value>(&get(&Limit::key(scope))).unwrap_or(Value::Null);
    hold_active(&rec(&t.provider), now) || hold_active(&rec(&t.key()), now)
}
/// Chain.pick: the first target that is offered and not held, or None (an exhausted chain holds work; it never wraps).
pub fn pick(xs: &[Target], get: &dyn Fn(&str) -> String, caps: &Value, now: u64) -> Option<Target> { xs.iter().find(|t| caps_valid(caps, t) && !target_held(get, t, now)).cloned() }

/// Error.code
fn quota_code(c: &str) -> bool { matches!(c, "rate_limit_error" | "rate_limit_exceeded" | "insufficient_quota" | "usage_limit_reached" | "budget_exceeded" | "quota_exceeded") }
/// Error.words (on the lower-cased reason)
fn quota_words(s: &str) -> bool { ["hit your usage limit", "you've hit your limit", "you have hit your limit", "weekly limit", "usage limit reached", "usage limit has been reached", "insufficient quota", "rate limit exceeded", "budget exceeded"].iter().any(|w| s.contains(w)) }
/// Error.no: sign-in and cancellation words are never a capacity signal.
fn not_quota(s: &str) -> bool { ["authentication", "authenticate", "unauthorized", "log in", "login required", "cancelled", "canceled", "invalid api key", "keyring"].iter().any(|w| s.contains(w)) }
fn s_of<'a>(o: &'a Value, k: &str) -> &'a str { o.get(k).and_then(Value::as_str).unwrap_or("") }
fn rejected(o: &Value) -> bool { s_of(o, "type") == "rate_limit_event" && s_of(&o["rate_limit_info"], "status") == "rejected" }
fn assistant(o: &Value) -> bool { s_of(o, "type") == "assistant" && s_of(o, "error") == "rate_limit" }
fn envelope(o: &Value) -> bool { s_of(o, "type") == "turn.failed" || s_of(o, "type") == "error" || (s_of(o, "type") == "result" && o["is_error"].as_bool().unwrap_or(false)) }
/// Error.reason: the provider's own words for it.
pub fn reason(o: &Value) -> String {
    if rejected(o) { return format!("Provider usage limit ({})", { let t = s_of(&o["rate_limit_info"], "rateLimitType"); if t.is_empty() { "account" } else { t } }) }
    for v in [&o["error"]["message"], &o["message"], &o["result"]] { if let Some(s) = v.as_str() { return s.to_string() } }
    let mut parts: Vec<String> = o["errors"].as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
    parts.extend(o["message"]["content"].as_array().map(|a| a.iter().map(|x| s_of(x, "text").to_string()).collect::<Vec<_>>()).unwrap_or_default());
    parts.join("; ")
}
/// Error.reset: the provider's reset time (0 unknown): its own field, else the time Claude Code puts after a bar ("Claude AI usage limit reached|1700000000").
pub fn reset_of(o: &Value) -> u64 {
    o["error"]["resets_at"].as_u64().or(o["resets_at"].as_u64()).or(o["rate_limit_info"]["resetsAt"].as_u64()).or_else(|| reason(o).rsplit_once('|').and_then(|(_, t)| t.trim().parse().ok()))
        .unwrap_or_else(|| reset_words(&reason(o), crate::hub::now_ms() / 1000, &|z| crate::bots::clock::TzRule::parse(&super::zone::rule(z))))
}
/// Reset.words: the time Claude Code writes after "resets" ("You've hit your weekly limit · resets Nov 12, 3am (America/New_York)", "resets 4:30pm (Europe/Paris)"), in
/// unix seconds; 0 when there is none or its zone is not known here (no zone is guessed). A date-less time is the next one to come; a date earlier than now is next year's.
pub fn reset_words(text: &str, now: u64, zone: &dyn Fn(&str) -> Option<crate::bots::clock::TzRule>) -> u64 {
    use crate::bots::clock::days;
    let low = text.to_lowercase();
    let Some(i) = low.rfind("resets ") else { return 0 };
    let rest = &low[i + 7..];
    let (z_name, body) = match (rest.find('('), rest.find(')')) { (Some(a), Some(b)) if b > a => (&text[i + 7 + a + 1..i + 7 + b], rest[..a].trim()), _ => return 0 };
    let Some(rule) = zone(z_name.trim()) else { return 0 };
    const MON: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let (date, clock) = match body.split_once(',') { Some((d, c)) => (Some(d.trim()), c.trim()), None => (None, body) };
    let pm = clock.ends_with("pm"); let am = clock.ends_with("am");
    let digits = clock.trim_end_matches("am").trim_end_matches("pm").trim();
    let (h, m): (u64, u64) = match digits.split_once(':') { Some((h, m)) => (h.parse().unwrap_or(99), m.parse().unwrap_or(99)), None => (digits.parse().unwrap_or(99), 0) };
    if m > 59 || (am || pm) && !(1..=12).contains(&h) || h > 23 { return 0 }
    let h = if pm && h != 12 { h + 12 } else if am && h == 12 { 0 } else { h };
    let wall_now = rule.wall(now); let (y, _, _) = crate::bots::clock::civil(wall_now / 86400);
    let at = |d: u64| rule.at(d * 86400 + h * 3600 + m * 60);
    match date {
        Some(d) => {
            let mut it = d.split_whitespace(); let (Some(mo), Some(day)) = (it.next(), it.next()) else { return 0 };
            let Some(mi) = MON.iter().position(|x| mo.starts_with(x)) else { return 0 };
            let Ok(day) = day.parse::<u64>() else { return 0 };
            let t = at(days(y, mi as u64 + 1, day)); if t + 86400 < now { at(days(y + 1, mi as u64 + 1, day)) } else { t }
        }
        None => { let today = wall_now / 86400; let t = at(today); if t > now { t } else { at(today + 1) } }
    }
}
/// Error.scope: an account-wide limit holds the provider; a model limit holds `provider/model`. An explicit protocol scope wins over wording.
pub fn scope_of(p: &str, m: &str, o: &Value) -> String {
    let explicit = o["error"]["scope"].as_str().or(o["scope"].as_str()).unwrap_or("");
    let low = reason(o).to_lowercase();
    let account = explicit != "model" && (explicit == "account" || rejected(o) || assistant(o) || (!low.contains("hit your usage limit for ") && (low.contains("weekly limit") || low.contains("usage limit") || low.contains("hit your limit"))));
    if account { p.to_string() } else { format!("{p}/{m}") }
}
/// Error.classify: only a provider error envelope is read, never arbitrary text. Auth, cancellations and network failures are not capacity signals.
pub fn classify(p: &str, m: &str, o: &Value) -> Option<Limit> {
    let low = reason(o).to_lowercase();
    let code = o["error"]["code"].as_str().or(o["error"]["type"].as_str()).unwrap_or("");
    let signal = rejected(o) || assistant(o) || envelope(o);
    if signal && !not_quota(&low) && (rejected(o) || assistant(o) || quota_code(code) || quota_words(&low)) { Some(Limit { scope: scope_of(p, m, o), reason: reason(o), reset: reset_of(o), auth: false }) } else { None }
}
/// Error.auth_code / Error.auth_words: what a provider's error envelope says when the login itself failed.
fn auth_code(c: &str) -> bool { matches!(c, "authentication_error" | "invalid_api_key" | "permission_error" | "unauthorized" | "token_expired") }
/// Claude Code 2.1.319 says "Failed to authenticate: OAuth session expired and could not be refreshed" when its refresh token is dead (OAuthRefreshDeadError: "OAuth refresh token is
/// no longer valid; run /login"), and "API Error: 401" for a token the API refused. Its refresh-lock timeout ("could not be refreshed: another Claude Code process is holding the
/// refresh lock") is a moment's contention, never a failed login: the next request refreshes.
fn auth_words(s: &str) -> bool {
    if s.contains("refresh lock") { return false }
    ["please run /login", "run /login", "invalid api key", "oauth token has expired", "authentication failed", "authentication_error", "not logged in", "login required", "unauthorized", "token has been revoked",
     "failed to authenticate", "oauth session expired", "refresh token is no longer valid", "api error: 401", "status code 401", "401 unauthorized"].iter().any(|w| s.contains(w))
}
/// Words a provider CLI's refusal (a control request's error, a status line) says when the login itself is dead: what [`auth_failure`] reads, on any text.
pub fn auth_text(s: &str) -> bool { let low = s.to_lowercase(); !low.contains("cancelled") && !low.contains("canceled") && auth_words(&low) }
/// Error.auth: a failed login on a provider's error envelope, never arbitrary text: an assistant message saying the words is not one, and a cancelled turn is not one.
/// The whole login is held (the provider's scope); nobody reports when it comes back, so the reset is unknown.
pub fn auth_failure(p: &str, o: &Value) -> Option<Limit> {
    if s_of(o, "type") == "assistant" && s_of(o, "error") == "authentication_failed" { return Some(Limit { scope: p.to_string(), reason: reason(o), reset: 0, auth: true }) }
    if !envelope(o) { return None }
    let low = reason(o).to_lowercase();
    let code = o["error"]["code"].as_str().or(o["error"]["type"].as_str()).unwrap_or("");
    if low.contains("cancelled") || low.contains("canceled") || !(auth_code(code) || auth_words(&low)) { return None }
    Some(Limit { scope: p.to_string(), reason: reason(o), reset: 0, auth: true })
}
/// A process that ended without a line saying why: what it left on stderr, read as the provider's own error envelope (its last 2000 characters; the limit or sign-in message is at the end).
pub fn from_stderr(p: &str, m: &str, stderr: &str) -> Option<Limit> {
    let n = stderr.chars().count(); let tail: String = stderr.chars().skip(n.saturating_sub(2000)).collect();
    let o = json!({"type":"error","message":tail.trim()});
    classify(p, m, &o).or_else(|| auth_failure(p, &o))
}

// ---- the planner over named accounts: holds, eligibility, the best account, the thread's preference ----

/// Hold.key: the hold on one profile at one epoch (`""` profile is the CLI's own login); the per-model hold adds `model:<model>`. Parts are length-prefixed.
pub fn part(s: &str) -> String { format!("{}:{s}", s.chars().count()) }
pub fn hold_key(provider: &str, id: &str, epoch: u64) -> String { format!("account.limit.{}{}{epoch}:", part(provider), part(id)) }
pub fn hold_model_key(provider: &str, id: &str, epoch: u64, model: &str) -> String { format!("{}model:{}", hold_key(provider, id, epoch), part(model)) }
fn rec(get: &dyn Fn(&str) -> String, key: &str) -> Value { serde_json::from_str(&get(key)).unwrap_or(Value::Null) }
/// How long a limit that named no reset holds before a turn is let try again (a turn is the test: still out, it is held again). A failed sign-in has no such end: the
/// account's own sign-in state ends it (accounts_io `after`).
pub const UNKNOWN_HOLD_S: u64 = 3600;
/// Hold.end: when a hold ends: its reset, else an hour after it was met; 0 for a failed sign-in (until a sign-in or a proof).
pub fn hold_end(o: &Value) -> u64 {
    let r = o["reset"].as_u64().unwrap_or(0);
    if r > 0 { r } else if o["auth"] == true { 0 } else { o["at"].as_u64().unwrap_or(0) + UNKNOWN_HOLD_S }
}
/// Hold.active: a record holds from when it was met until its end (`hold_end`).
pub fn hold_active(o: &Value, now: u64) -> bool { !o["scope"].as_str().unwrap_or("").is_empty() && { let e = hold_end(o); e == 0 || now < e } }
/// Hold.current: the profile (or this model on it) is held now.
pub fn hold_current(get: &dyn Fn(&str) -> String, provider: &str, id: &str, epoch: u64, model: &str, now: u64) -> bool {
    hold_active(&rec(get, &hold_key(provider, id, epoch)), now) || hold_active(&rec(get, &hold_model_key(provider, id, epoch, model)), now)
}
/// Hold.until: when the holds on this profile and model end (0: a failed sign-in, held until it is signed in again).
pub fn hold_until(get: &dyn Fn(&str) -> String, provider: &str, id: &str, epoch: u64, model: &str) -> u64 {
    let (a, b) = (rec(get, &hold_key(provider, id, epoch)), rec(get, &hold_model_key(provider, id, epoch, model)));
    let end = |o: &Value| if o["scope"].as_str().unwrap_or("").is_empty() { Some(0) } else { match hold_end(o) { 0 => None, e => Some(e) } };
    match (end(&a), end(&b)) { (Some(x), Some(y)) => x.max(y), _ => 0 }
}
/// Hold.mark: the setting a limit on this thread's login writes (the provider hold, or the model's).
pub fn hold_mark(provider: &str, id: &str, epoch: u64, lim: &Limit, now: u64, at: u64) -> Value {
    let model = Target::parse(&lim.scope).model;
    let key = if lim.scope == provider { hold_key(provider, id, epoch) } else { hold_model_key(provider, id, epoch, &model) };
    json!({"$":"SettingSet","key":key,"value":lim.json(now),"at":at})
}
/// Profile.model: the profile's catalog offers the model (an empty catalog or model does not exclude).
pub fn profile_model(profile: &Value, model: &str) -> bool { let xs = profile["catalog"].as_array().cloned().unwrap_or_default(); model.is_empty() || xs.is_empty() || xs.iter().any(|x| x["id"].as_str() == Some(model)) }
/// Usage.applies: a model-specific window counts only for that model.
fn applies(w: &Value, model: &str) -> bool {
    let scope = w["scope"].as_str().unwrap_or("");
    (!matches!(scope, "seven_day_opus" | "opus") || model.contains("opus")) && (!matches!(scope, "seven_day_sonnet" | "sonnet") || model.contains("sonnet"))
}
/// Usage.score: the remaining allowance in thousandths of a percent across the applicable live windows (the largest use counts); None when unknown.
/// Missing, stale and expired readings stay unknown, never zero.
pub fn usage_score(profile: &Value, now: u64, model: &str) -> Option<u32> {
    let a = &profile["allowance"];
    if a["state"] != "ok" || a["phase"].as_str().unwrap_or("complete") != "complete" || a["provider"].as_str().unwrap_or(profile["provider"].as_str().unwrap_or("")) != profile["provider"].as_str().unwrap_or("") { return None }
    // a reading older than five minutes, from the future, or undated says nothing about the account now
    let at = a["at"].as_u64().unwrap_or(0);
    if at == 0 || at > now || now - at > 300 { return None }
    let mut used: Option<u32> = None;
    for w in a["windows"].as_array().cloned().unwrap_or_default() {
        let reset = w["resets_at"].as_u64().unwrap_or(0);
        if !(applies(&w, model) && (reset == 0 || reset > now)) { continue }
        let n = w["used_percent"].as_f64().filter(|x| (0.0..=100.0).contains(x)).map(|x| (x * 1000.0) as u32).or_else(|| w["used_permille"].as_f64().filter(|x| (0.0..=1000.0).contains(x)).map(|x| (x * 100.0) as u32));
        used = match (used, n) { (Some(a), Some(b)) => Some(a.max(b)), (None, b) => b, (a, None) => a };
    }
    used.map(|u| 100_000u32.saturating_sub(u))
}
/// Profile.eligible: connected, the provider's, offers the model, has allowance left, and is not held.
pub fn eligible(get: &dyn Fn(&str) -> String, now: u64, provider: &str, model: &str, profile: &Value) -> bool {
    let id = profile["id"].as_str().unwrap_or("");
    profile["provider"] == provider && profile["state"] == "connected" && !id.is_empty() && profile_model(profile, model) && usage_score(profile, now, model) != Some(0) && !hold_current(get, provider, id, profile["epoch"].as_u64().unwrap_or(0), model, now)
}
/// Pick.account: the eligible profile with the most remaining allowance. A reading that is unknown ranks as zero, so it comes after every known one; a tie goes to the
/// earliest saved profile.
pub fn pick_account(profiles: &[Value], get: &dyn Fn(&str) -> String, now: u64, provider: &str, model: &str) -> Option<Value> {
    let xs: Vec<&Value> = profiles.iter().filter(|p| eligible(get, now, provider, model, p)).collect();
    let mut best = *xs.first()?; let mut score = usage_score(best, now, model).unwrap_or(0);
    for p in xs.iter().skip(1) { let v = usage_score(p, now, model).unwrap_or(0); if v > score { best = p; score = v } }
    Some(best.clone())
}
/// Pick.account without the accounts this request already ran out on: a request never goes back to one, whatever its hold says now.
pub fn pick_untried(profiles: &[Value], tried: &[String], get: &dyn Fn(&str) -> String, now: u64, provider: &str, model: &str) -> Option<Value> {
    let xs: Vec<Value> = profiles.iter().filter(|p| !tried.contains(&accounts_id(p))).cloned().collect();
    pick_account(&xs, get, now, provider, model)
}
fn accounts_id(p: &Value) -> String { format!("{}:{}", p["id"].as_str().unwrap_or(""), p["epoch"].as_u64().unwrap_or(0)) }
/// One entry of the list of logins a request has run out on.
pub fn tried_id(id: &str, epoch: u64) -> String { format!("{id}:{epoch}") }
/// Usage.reset: when a limit that came with no reset ends, from the login's own windows: the fullest window that applies and has a reset to come (0: none known).
/// A window's reading is used however old it is: a reset time does not go stale until it has passed.
pub fn window_reset(profile: &Value, model: &str, now: u64) -> u64 {
    let mut best = (-1.0f64, 0u64);
    for w in profile["allowance"]["windows"].as_array().cloned().unwrap_or_default() {
        let (reset, used) = (w["resets_at"].as_u64().unwrap_or(0), w["used_percent"].as_f64().or(w["used_permille"].as_f64().map(|x| x / 10.0)).unwrap_or(0.0));
        if applies(&w, model) && reset > now && used > best.0 { best = (used, reset) }
    }
    best.1
}
/// Hold.clearable: of `keys` (the hold records there are), the ones on this profile that are over or that a newer fact has ended: a hold past its end; a limit once a
/// reading taken after it shows allowance left in every window that applies (a weekly window still full keeps it); a failed login once the account's sign-in was checked
/// after it. The CLI's own login is the profile with id "" at epoch 0, its reading the default allowance (`allowance.claude`): its holds end the same ways.
pub fn clearable(profile: &Value, keys: &[String], get: &dyn Fn(&str) -> String, now: u64) -> Vec<String> {
    let (provider, id, epoch) = (profile["provider"].as_str().unwrap_or(""), profile["id"].as_str().unwrap_or(""), profile["epoch"].as_u64().unwrap_or(0));
    if profile["state"] != "connected" { return vec![] }
    let mine = hold_key(provider, id, epoch);
    keys.iter().filter(|k| k.starts_with(&mine)).filter(|k| {
        let o = rec(get, k);
        if o["scope"].as_str().unwrap_or("").is_empty() { return false }
        // one with no reset is gone once over; one past its known reset is left: it holds nothing, and the thread's return to its account reads when it ended (`preference_ready`)
        if !hold_active(&o, now) { return o["reset"].as_u64().unwrap_or(0) == 0 }
        let at = o["at"].as_u64().unwrap_or(0);
        let model = k.split_once("model:").map(|(_, m)| m.split_once(':').map_or(m, |(_, x)| x)).unwrap_or("");
        if o["auth"] == true { profile["checkedAt"].as_u64().unwrap_or(0) > at } else { profile["allowance"]["at"].as_u64().unwrap_or(0) > at && usage_score(profile, now, model).map_or(false, |s| s > 0) }
    }).cloned().collect()
}
/// The CLI's own login as a profile, for `clearable`: its reading is the default allowance the hub keeps (`allowance.claude`).
pub fn default_profile(provider: &str, allowance: &str) -> Value {
    json!({"provider":provider,"id":"","epoch":0,"state":"connected","allowance":serde_json::from_str::<Value>(allowance).unwrap_or(Value::Null)})
}
/// Hold.proven: a turn that completed on this login (and model) proves it is not out: every limit hold on it is over. A failed sign-in is left to the sign-in state.
pub fn proven(keys: &[String], get: &dyn Fn(&str) -> String, provider: &str, id: &str, epoch: u64, model: &str) -> Vec<String> {
    [hold_key(provider, id, epoch), hold_model_key(provider, id, epoch, model)].into_iter().filter(|k| keys.contains(k)).filter(|k| { let o = rec(get, k); !o["scope"].as_str().unwrap_or("").is_empty() && o["auth"] != true }).collect()
}
/// Pick.may_cross: a thread may leave its provider only when no account of it is eligible.
pub fn may_cross(profiles: &[Value], get: &dyn Fn(&str) -> String, now: u64, provider: &str, model: &str) -> bool { pick_account(profiles, get, now, provider, model).is_none() }
/// Reset.next: the earliest time any connected profile of the provider (or the thread's own login) is free again (0: none known).
pub fn next_reset(profiles: &[Value], get: &dyn Fn(&str) -> String, now: u64, provider: &str, model: &str, own_id: &str, own_epoch: u64) -> u64 {
    let earliest = |a: u64, b: u64| if a == 0 { b } else if b == 0 { a } else { a.min(b) };
    let mut best = hold_until(get, provider, own_id, own_epoch, model);
    for h in profiles {
        let r = hold_until(get, provider, h["id"].as_str().unwrap_or(""), h["epoch"].as_u64().unwrap_or(0), model);
        if h["provider"] == provider && h["state"] == "connected" && profile_model(h, model) && r > now { best = earliest(best, r) }
    }
    best
}
/// Preference: the thread's own provider, model, effort and binding before a failover, kept so it can return when the hold ends.
pub fn preference_key(thread: &str) -> String { format!("account.failover.preference.{thread}") }
/// Preference.save: the first failover of a run of them keeps what the thread was on; a later one (the record still describes where the thread is) only moves `active`.
pub fn preference_save(old: &str, provider: &str, model: &str, effort: &str, binding: &str, active: &str) -> String {
    let o: Value = serde_json::from_str(old).unwrap_or(Value::Null);
    let mut r = if o["activeProvider"] == provider && o["activeBinding"] == binding { o } else { json!({"provider":provider,"model":model,"effort":effort,"binding":binding}) };
    r["activeProvider"] = json!(provider); r["activeBinding"] = json!(active); r.to_string()
}
/// Preference.ready: the thread is still where the failover left it (`binding`), and the login it left has a reset that has come, is not held any more and can be used again. A hold
/// with no reset never reads as ready: it ends with a newer fact (`clearable`), and then `until` is 0 and nothing more is known, so the thread stays where it works.
pub fn preference_ready(rec_raw: &str, provider: &str, binding: &str, inventory: &[Value], get: &dyn Fn(&str) -> String, now: u64) -> Option<(String, u64)> {
    let o: Value = serde_json::from_str(rec_raw).ok()?;
    if o["activeProvider"] != provider || o["activeBinding"] != binding || o["provider"] != provider { return None }
    let b: Value = serde_json::from_str(o["binding"].as_str()?).ok()?;
    let (id, epoch, model) = (b["id"].as_str()?, b["epoch"].as_u64().unwrap_or(0), o["model"].as_str().unwrap_or(""));
    let until = hold_until(get, provider, id, epoch, model);
    if until == 0 || now < until || hold_current(get, provider, id, epoch, model, now) { return None }
    if !id.is_empty() && !inventory.iter().any(|p| p["id"] == id && p["epoch"] == epoch && p["provider"] == provider && p["state"] == "connected" && profile_model(p, model)) { return None }
    Some((id.to_string(), epoch))
}

#[cfg(test)]
mod outage_tests {
    use super::*;
    fn ny(z: &str) -> Option<crate::bots::clock::TzRule> { (z == "America/New_York").then(|| crate::bots::clock::TzRule::parse("EST5EDT,M3.2.0,M11.1.0")).flatten() }
    #[test] fn a_reset_in_claude_codes_words_is_read_in_its_zone() {
        let now = 1794092090; // 2026-11-07 17:54:50 EST
        assert_eq!(reset_words("You've hit your weekly limit · resets Nov 12, 3am (America/New_York)", now, &ny), 1794470400, "Nov 12 03:00 EST is 08:00 UTC");
        assert_eq!(reset_words("You've hit your limit · resets 8pm (America/New_York)", now, &ny), 1794099600, "today, still to come");
        assert_eq!(reset_words("You've hit your limit · resets 4:30pm (America/New_York)", now, &ny), 1794087000 + 86400, "passed today: tomorrow");
        assert_eq!(reset_words("You've hit your limit · resets Jan 2, 12am (America/New_York)", now, &ny), 1798866000, "a date already past is next year's");
        for t in ["You've hit your limit", "resets soon (America/New_York)", "resets 3am (Mars/Base)", "resets 13pm (America/New_York)", "resets 3am"] { assert_eq!(reset_words(t, now, &ny), 0, "{t}: no guess") }
    }
    #[test] fn a_hold_ends_at_its_reset_and_one_with_none_an_hour_after_it_was_met() {
        let get = |k: &str| match k { k if k == hold_key("claude", "w", 2) => json!({"scope":"claude","reset":0,"at":1000}).to_string(), k if k == hold_key("claude", "r", 1) => json!({"scope":"claude","reset":5000,"at":1000}).to_string(),
            k if k == hold_key("claude", "a", 1) => json!({"scope":"claude","reset":0,"at":1000,"auth":true}).to_string(), _ => String::new() };
        assert!(hold_current(&get, "claude", "w", 2, "", 1000 + UNKNOWN_HOLD_S - 1)); assert!(!hold_current(&get, "claude", "w", 2, "", 1000 + UNKNOWN_HOLD_S), "a turn may try again");
        assert_eq!(hold_until(&get, "claude", "w", 2, ""), 1000 + UNKNOWN_HOLD_S, "a waiting thread knows when");
        assert!(hold_current(&get, "claude", "r", 1, "", 4999) && !hold_current(&get, "claude", "r", 1, "", 5000), "its own reset"); assert_eq!(hold_until(&get, "claude", "r", 1, ""), 5000);
        assert!(hold_current(&get, "claude", "a", 1, "", 9_999_999), "a failed sign-in waits for the sign-in"); assert_eq!(hold_until(&get, "claude", "a", 1, ""), 0);
    }
    #[test] fn a_reading_with_allowance_or_a_completed_turn_ends_a_hold_on_any_login_the_default_too() {
        let now = 100_000;
        let ok = |used: f64| json!({"provider":"claude","state":"ok","at":now - 10,"windows":[{"scope":"five_hour","used_percent":used},{"scope":"seven_day","used_percent":40.0}]});
        let (named, dflt) = (hold_key("claude", "w", 2), hold_key("claude", "", 0));
        let rec = |reset: u64| json!({"scope":"claude","reset":reset,"at":now - 600}).to_string();
        let keys = vec![named.clone(), dflt.clone()];
        let get = |k: &str| if k == named || k == dflt { rec(0) } else { String::new() };
        let w = json!({"id":"w","provider":"claude","epoch":2,"state":"connected","allowance":ok(10.0)});
        assert_eq!(clearable(&w, &keys, &get, now), [named.clone()]);
        assert_eq!(clearable(&default_profile("claude", &ok(28.0).to_string()), &keys, &get, now), [dflt.clone()], "the CLI's own login: on Nov 7 it had 72% left and was held all the same");
        assert!(clearable(&default_profile("claude", &ok(100.0).to_string()), &keys, &get, now).is_empty(), "a window still full keeps it");
        assert!(clearable(&json!({"id":"w","provider":"claude","epoch":2,"state":"unchecked","allowance":ok(10.0)}), &keys, &get, now).is_empty(), "an unchecked login proves nothing yet");
        let far = |k: &str| if k == named { rec(now + 86400 * 4) } else { String::new() };
        assert_eq!(clearable(&w, &keys, &far, now), [named.clone()], "a weekly reset days away ends early when every window shows allowance");
        let gone = |k: &str| if k == named { json!({"scope":"claude","reset":0,"at":now - UNKNOWN_HOLD_S - 1}).to_string() } else { String::new() };
        assert_eq!(clearable(&json!({"id":"w","provider":"claude","epoch":2,"state":"connected"}), &keys, &gone, now), [named.clone()], "past its end, with no reading at all");
        let reset_past = |k: &str| if k == named { json!({"scope":"claude","reset":now - 1,"at":now - 9000}).to_string() } else { String::new() };
        assert!(clearable(&json!({"id":"w","provider":"claude","epoch":2,"state":"connected"}), &keys, &reset_past, now).is_empty() && !hold_current(&reset_past, "claude", "w", 2, "", now), "past a known reset it holds nothing, and its record is left for the thread's return");
        let model = hold_model_key("claude", "w", 2, "opus"); let mk = vec![named.clone(), model.clone(), hold_key("claude", "a", 1)];
        let mixed = |k: &str| if k == named || k == model { rec(0) } else if k == hold_key("claude", "a", 1) { json!({"scope":"claude","reset":0,"at":1,"auth":true}).to_string() } else { String::new() };
        assert_eq!(proven(&mk, &mixed, "claude", "w", 2, "opus"), [named.clone(), model.clone()], "a completed turn ends the login's and its model's holds");
        assert!(proven(&mk, &mixed, "claude", "a", 1, "").is_empty(), "never a failed sign-in");
    }
    fn none(_: &str) -> String { String::new() }
    #[test] fn fallback_empty_exhausted() { assert_eq!(pick(&[], &none, &json!({}), 5), None); }
    #[test] fn fallback_auth_is_not_quota() { assert_eq!(classify("codex", "gpt-6.1-sol", &json!({"type":"turn.failed","message":"login keyring is locked"})), None); }
    #[test] fn fallback_unknown_reset_held() { assert!(limit_active(&json!({"scope":"claude","reset":0}), 1000)); }
    #[test] fn fallback_reset_boundary() { assert!(!limit_active(&json!({"scope":"claude","reset":1000}), 1000)); assert!(limit_active(&json!({"scope":"claude","reset":1001}), 1000)); assert!(!limit_active(&json!({"reset":5}), 1)); }
    #[test] fn a_provider_error_envelope_with_a_quota_signal_is_a_limit_and_the_scope_follows_the_words() {
        let l = classify("codex", "gpt-6.1-sol", &json!({"type":"turn.failed","error":{"message":"You've hit your usage limit. Try again later.","resets_at":2000}})).unwrap();
        assert_eq!((l.scope.as_str(), l.reset), ("codex", 2000), "a bare usage limit is account capacity");
        let m = classify("codex", "gpt-6.1-sol", &json!({"type":"turn.failed","message":"You've hit your usage limit for gpt-6.1-sol"})).unwrap(); assert_eq!(m.scope, "codex/gpt-6.1-sol", "the 'for <model>' form is the model's");
        assert_eq!(classify("claude", "opus", &json!({"type":"result","is_error":true,"result":"Claude AI usage limit reached|1700000000"})).unwrap().scope, "claude");
        assert_eq!(classify("claude", "opus", &json!({"type":"result","is_error":true,"result":"You've hit your limit"})).unwrap().scope, "claude", "Claude Code's own words are the account's");
        let r = classify("claude", "opus", &json!({"type":"rate_limit_event","rate_limit_info":{"status":"rejected","rateLimitType":"seven_day","resetsAt":3000}})).unwrap(); assert_eq!((r.reason.as_str(), r.reset), ("Provider usage limit (seven_day)", 3000));
        assert_eq!(classify("claude", "opus", &json!({"type":"assistant","error":"rate_limit","message":{"content":[{"text":"limit"}]}})).unwrap().reason, "limit");
        assert_eq!(classify("codex", "m", &json!({"type":"turn.failed","error":{"message":"x","scope":"model"},"scope":"account"})).map(|l| l.scope), None, "no quota words: not a limit");
        assert_eq!(classify("codex", "m", &json!({"type":"turn.failed","error":{"code":"rate_limit_exceeded","message":"slow down","scope":"model"}})).unwrap().scope, "codex/m", "an explicit protocol scope wins");
    }
    #[test] fn ordinary_text_and_non_quota_failures_are_never_a_limit() {
        assert_eq!(classify("claude", "o", &json!({"type":"assistant","message":{"content":[{"text":"the weekly limit of my patience"}]}})), None, "an assistant message is not an error envelope");
        assert_eq!(classify("claude", "o", &json!({"type":"result","is_error":true,"result":"authentication failed: usage limit"})), None);
        assert_eq!(classify("claude", "o", &json!({"type":"error","error":{"message":"turn cancelled: weekly limit"}})), None); assert_eq!(classify("claude", "o", &json!({"type":"result","is_error":false,"result":"usage limit reached"})), None);
    }
    #[test] fn a_failed_login_is_read_from_an_error_envelope_only_and_is_not_a_limit() {
        for o in [json!({"type":"result","is_error":true,"result":"Invalid API key · Please run /login"}), json!({"type":"error","error":{"type":"authentication_error","message":"nope"}}), json!({"type":"turn.failed","error":{"message":"OAuth token has expired"}})] {
            let l = auth_failure("claude", &o).unwrap(); assert!(l.auth && l.scope == "claude" && l.reset == 0, "{o}"); assert_eq!(classify("claude", "o", &o), None, "{o}: never a capacity signal"); }
        assert_eq!(auth_failure("claude", &json!({"type":"assistant","message":{"content":[{"text":"please run /login"}]}})), None, "an assistant saying it is not a failure");
        assert_eq!(auth_failure("claude", &json!({"type":"result","is_error":false,"result":"please run /login"})), None, "nor a good result");
        assert_eq!(auth_failure("claude", &json!({"type":"error","message":"turn cancelled: not logged in"})), None);
        assert_eq!(auth_failure("claude", &json!({"type":"result","is_error":true,"result":"Overloaded"})), None, "a provider's bad day is neither");
        // Nov 7: the words Claude Code 2.1.319 ended every turn with once its refresh token was dead; the lock timeout beside them is not one
        for t in ["Failed to authenticate: OAuth session expired and could not be refreshed", "OAuth refresh token is no longer valid; run /login to re-authenticate", "API Error: 401 {\"type\":\"error\",\"error\":{\"type\":\"authentication_error\"}}"] {
            let o = json!({"type":"result","is_error":true,"result":t}); assert!(auth_failure("claude", &o).unwrap().auth, "{t}"); assert_eq!(classify("claude", "o", &o), None, "{t}"); assert!(auth_text(t), "{t}"); }
        assert!(auth_failure("claude", &json!({"type":"assistant","error":"authentication_failed","message":{"content":[{"type":"text","text":"Failed to authenticate: OAuth session expired and could not be refreshed"}]}})).unwrap().auth);
        let lock = json!({"type":"result","is_error":true,"result":"OAuth access token could not be refreshed: another Claude Code process is holding the refresh lock"});
        assert_eq!(auth_failure("claude", &lock), None, "contention for the refresh lock is not a dead login"); assert!(!auth_text("OAuth access token could not be refreshed: another Claude Code process is holding the refresh lock"));
        assert!(!Value::from(serde_json::from_str::<Value>(&Limit { scope: "c".into(), reason: "r".into(), reset: 0, auth: false }.json(1)).unwrap()).as_object().unwrap().contains_key("auth"), "the record of a plain limit is as it always was");
    }
    #[test] fn what_a_process_left_on_stderr_is_read_like_an_envelope() {
        assert!(from_stderr("codex", "m", &format!("{}\nYou've hit your usage limit. Try again later.\n", "x".repeat(5000))).is_some());
        assert!(from_stderr("claude", "m", "error: Please run /login").unwrap().auth); assert_eq!(from_stderr("claude", "m", "segmentation fault"), None);
    }
    #[test] fn the_reset_claude_code_puts_after_a_bar_is_read() {
        assert_eq!(classify("claude", "opus", &json!({"type":"result","is_error":true,"result":"Claude AI usage limit reached|1700000000"})).unwrap().reset, 1700000000);
        assert_eq!(classify("claude", "opus", &json!({"type":"result","is_error":true,"result":"Claude AI usage limit reached"})).unwrap().reset, 0, "no bar: unknown, never invented");
    }
    #[test] fn a_limit_with_no_reset_takes_it_from_the_fullest_window_that_applies() {
        let p = json!({"allowance":{"state":"stale","windows":[{"scope":"five_hour","used_percent":100.0,"resets_at":500},{"scope":"seven_day","used_percent":60.0,"resets_at":9000},{"scope":"seven_day_opus","used_percent":100.0,"resets_at":7000},{"scope":"x","used_percent":100.0}]}});
        assert_eq!(window_reset(&p, "sonnet", 100), 500, "a stale reading still knows its reset"); assert_eq!(window_reset(&p, "sonnet", 600), 9000, "a window that has passed says nothing");
        assert_eq!(window_reset(&json!({}), "m", 1), 0);
    }
    #[test] fn a_hold_with_no_reset_ends_with_a_newer_fact_and_only_then() {
        let key = hold_key("claude", "w", 2); let now = 1000;
        let hold = |o: Value| { let (k, v) = (key.clone(), o.to_string()); move |x: &str| if x == k { v.clone() } else { String::new() } };
        let p = |checked: u64, at: u64, used: f64| json!({"id":"w","epoch":2,"provider":"claude","state":"connected","checkedAt":checked,"allowance":{"state":"ok","phase":"complete","provider":"claude","at":at,"windows":[{"scope":"five_hour","used_percent":used}]}});
        let lim = hold(json!({"scope":"claude","reset":0,"at":900}));
        let ks = vec![key.clone()];
        assert_eq!(clearable(&p(0, 950, 10.0), &ks, &lim, now), ks, "a reading after the limit shows allowance left");
        assert!(clearable(&p(0, 950, 100.0), &ks, &lim, now).is_empty(), "still full"); assert!(clearable(&p(0, 850, 10.0), &ks, &lim, now).is_empty(), "the reading is older than the limit");
        let auth = hold(json!({"scope":"claude","reset":0,"at":900,"auth":true}));
        assert_eq!(clearable(&p(950, 950, 10.0), &ks, &auth, now), ks, "the sign-in was checked after it failed"); assert!(clearable(&p(850, 950, 10.0), &ks, &auth, now).is_empty(), "allowance says nothing of a login");
        assert_eq!(clearable(&p(950, 950, 10.0), &ks, &hold(json!({"scope":"claude","reset":2000,"at":900})), now), ks, "a known reset days away ends early too when a newer reading shows allowance in every window");
        assert!(clearable(&p(950, 950, 10.0), &[hold_key("claude", "other", 2)], &auth, now).is_empty(), "another account's hold is not this one's");
    }
    #[test] fn a_request_never_goes_back_to_an_account_it_ran_out_on() {
        let xs = vec![json!({"id":"a","epoch":1,"provider":"claude","state":"connected"}), json!({"id":"b","epoch":1,"provider":"claude","state":"connected"})];
        assert_eq!(pick_untried(&xs, &[tried_id("a", 1)], &none, 5, "claude", "m").unwrap()["id"], "b");
        assert_eq!(pick_untried(&xs, &[tried_id("a", 1), tried_id("b", 1)], &none, 5, "claude", "m"), None);
    }
    #[test] fn the_preference_is_the_first_login_of_a_run_and_ready_when_its_reset_has_come() {
        let (a, b, c) = (r#"{"id":"a","epoch":1}"#, r#"{"id":"b","epoch":1}"#, r#"{"id":"c","epoch":1}"#);
        let first = preference_save("", "claude", "opus", "high", a, b); let second = preference_save(&first, "claude", "opus", "high", b, c); let o: Value = serde_json::from_str(&second).unwrap();
        assert_eq!((o["binding"].as_str().unwrap(), o["activeBinding"].as_str().unwrap()), (a, c), "where it started stays, where it is moves");
        let inv = vec![json!({"id":"a","epoch":1,"provider":"claude","state":"connected"})]; let k = hold_key("claude", "a", 1);
        let held = |reset: u64| { let (k, v) = (k.clone(), json!({"scope":"claude","reset":reset}).to_string()); move |x: &str| if x == k { v.clone() } else { String::new() } };
        assert_eq!(preference_ready(&second, "claude", c, &inv, &held(500), 600), Some(("a".to_string(), 1)), "its reset has come");
        assert_eq!(preference_ready(&second, "claude", c, &inv, &held(500), 400), None, "not yet"); assert_eq!(preference_ready(&second, "claude", c, &inv, &held(0), 600), None, "no reset: it stays");
        assert_eq!(preference_ready(&second, "claude", b, &inv, &held(500), 600), None, "the thread is no longer where the failover left it"); assert_eq!(preference_ready(&second, "claude", c, &[], &held(500), 600), None, "the account is gone");
    }
    #[test] fn a_stale_future_or_undated_allowance_scores_nothing() {
        let mk = |at: Value| json!({"provider":"claude","allowance":{"state":"ok","phase":"complete","provider":"claude","at":at,"windows":[{"scope":"five_hour","used_percent":10.0}]}});
        assert!(usage_score(&mk(json!(1000)), 1100, "m").is_some(), "fresh");
        assert_eq!(usage_score(&mk(json!(1000)), 1301, "m"), None, "older than five minutes");
        assert_eq!(usage_score(&mk(json!(2000)), 1100, "m"), None, "from the future");
        assert_eq!(usage_score(&mk(json!(0)), 1100, "m"), None, "undated");
    }
    #[test] fn the_limit_record_keeps_four_fields_and_a_cut_reason() { let l = Limit { scope: "claude".into(), reason: "x".repeat(500), reset: 7, auth: false }; let v: Value = serde_json::from_str(&l.json(3)).unwrap(); assert_eq!((v["reason"].as_str().unwrap().len(), v["reset"].clone(), v["at"].clone()), (400, json!(7), json!(3))); assert_eq!(Limit::key("codex/m"), "fallback.outage.codex/m"); }
    #[test] fn a_chain_target_is_held_by_its_provider_or_its_model_and_the_pick_skips_it() {
        let get = |k: &str| match k { "fallback.outage.claude" => json!({"scope":"claude","reset":0}).to_string(), "fallback.outage.codex/gpt-6" => json!({"scope":"codex/gpt-6","reset":9999}).to_string(), _ => String::new() };
        let caps = json!({"claude":["opus"],"codex":["gpt-6","gpt-5"],"grok":["g"]});
        let c = parse("claude/opus,codex/gpt-6,codex/gpt-5,grok/g");
        assert_eq!(pick(&c, &get, &caps, 100).unwrap().key(), "codex/gpt-5"); assert!(target_held(&get, &c[1], 100)); assert!(!target_held(&get, &c[1], 10_000), "the model hold ended");
        assert_eq!(pick(&parse("claude/opus"), &get, &caps, 100), None, "an exhausted chain holds the work, it does not wrap");
    }
    fn prof(id: &str, epoch: u64, left: Option<f64>) -> Value {
        let allow = left.map(|u| json!({"state":"ok","provider":"claude","at":10,"windows":[{"scope":"five_hour","used_percent":u}]})).unwrap_or(Value::Null);
        json!({"id":id,"provider":"claude","state":"connected","epoch":epoch,"catalog":[{"id":"opus"}],"allowance":allow})
    }
    #[test] fn the_account_with_the_most_allowance_left_is_picked_and_unknown_comes_after_known() {
        let ps = vec![prof("a", 1, Some(80.0)), prof("b", 1, Some(20.0)), prof("c", 1, None)];
        assert_eq!(pick_account(&ps, &none, 10, "claude", "opus").unwrap()["id"], "b", "80 used leaves 20, 20 used leaves 80; the unknown one ranks last but is eligible");
        assert_eq!(pick_account(&[prof("c", 1, None), prof("d", 1, None)], &none, 10, "claude", "opus").unwrap()["id"], "c", "with nothing known the saved order stands");
        let known = vec![prof("a", 1, Some(80.0)), prof("b", 1, Some(20.0))]; assert_eq!(pick_account(&known, &none, 10, "claude", "opus").unwrap()["id"], "b");
        let spent = vec![prof("a", 1, Some(100.0)), prof("b", 1, Some(50.0))]; assert_eq!(pick_account(&spent, &none, 10, "claude", "opus").unwrap()["id"], "b", "an exhausted account is not eligible");
        assert!(pick_account(&[prof("a", 1, Some(100.0))], &none, 10, "claude", "opus").is_none()); assert!(may_cross(&[prof("a", 1, Some(100.0))], &none, 10, "claude", "opus"), "none eligible: the thread may leave the provider");
        assert!(!may_cross(&known, &none, 10, "claude", "opus"));
    }
    #[test] fn a_held_or_wrong_model_or_disconnected_account_is_not_eligible_and_its_reset_is_the_next_free_time() {
        let get = |k: &str| if k == hold_key("claude", "a", 1) { json!({"scope":"claude","reset":500}).to_string() } else { String::new() };
        let mut off = prof("c", 1, Some(10.0)); off["state"] = json!("disconnected"); let mut other = prof("d", 1, Some(10.0)); other["catalog"] = json!([{"id":"sonnet"}]);
        let ps = vec![prof("a", 1, Some(10.0)), off, other, prof("e", 2, Some(10.0))];
        let ids: Vec<&str> = ps.iter().filter(|p| eligible(&get, 100, "claude", "opus", p)).map(|p| p["id"].as_str().unwrap()).collect(); assert_eq!(ids, ["e"]);
        assert_eq!(next_reset(&ps, &get, 100, "claude", "opus", "", 0), 500); assert_eq!(hold_until(&get, "claude", "a", 1, "opus"), 500);
        let unknown = |k: &str| if k == hold_key("claude", "a", 1) { json!({"scope":"claude","reset":0,"at":500}).to_string() } else { String::new() }; assert_eq!(hold_until(&unknown, "claude", "a", 1, "opus"), 500 + UNKNOWN_HOLD_S, "an unknown reset is tried again an hour after it was met");
    }
    #[test] fn a_hold_is_marked_on_the_provider_or_the_model_with_length_prefixed_keys() {
        assert_eq!(hold_key("claude", "w", 3), "account.limit.6:claude1:w3:"); assert_eq!(hold_model_key("claude", "w", 3, "opus"), "account.limit.6:claude1:w3:model:4:opus");
        let l = Limit { scope: "claude/opus".into(), reason: "r".into(), reset: 9, auth: false }; assert_eq!(hold_mark("claude", "w", 3, &l, 5, 5)["key"], "account.limit.6:claude1:w3:model:4:opus");
        let p = Limit { scope: "claude".into(), reason: "r".into(), reset: 9, auth: false }; assert_eq!(hold_mark("claude", "w", 3, &p, 5, 5)["key"], "account.limit.6:claude1:w3:"); assert_eq!(preference_key("t1"), "account.failover.preference.t1");
    }
}

#[cfg(test)]
mod tests {
    use super::*; use serde_json::json;
    fn caps() -> Value { json!({"claude":["opus","sonnet"],"codex":["gpt-6"],"grok":[]}) }
    #[test] fn a_target_is_a_provider_and_everything_after_the_first_slash() { assert_eq!(Target::parse("openai/gpt/5"), Target { provider: "openai".into(), model: "gpt/5".into() }); assert_eq!(Target::parse("claude").key(), "claude/"); }
    #[test] fn chains_parse_and_show() { let c = parse("claude/opus,codex/gpt-6"); assert_eq!(c.len(), 2); assert_eq!(show(&c), "claude/opus,codex/gpt-6"); assert!(parse("").is_empty()); }
    #[test] fn a_chain_is_valid_against_what_is_installed_and_names_each_target_once() {
        assert!(valid(&parse("claude/opus,codex/gpt-6"), &caps())); assert!(valid(&parse("claude/"), &caps()));
        assert!(!valid(&parse("claude/nope"), &caps()), "unknown model"); assert!(!valid(&parse("grok/x"), &caps()), "a provider with no models"); assert!(!valid(&parse("muse/x"), &caps()), "unknown provider");
        assert!(!valid(&parse("claude/opus,claude/opus"), &caps()), "twice");
    }
    #[test] fn editing_moves_and_removes() {
        let c = parse("a/1,b/2,c/3"); assert_eq!(show(&up(&c, "c/3")), "a/1,c/3,b/2"); assert_eq!(show(&up(&c, "a/1")), "a/1,b/2,c/3"); assert_eq!(show(&remove(&c, "b/2")), "a/1,c/3");
        assert_eq!(show(&suffix(&c, "a/1")), "b/2,c/3"); assert_eq!(show(&suffix(&c, "x/9")), "a/1,b/2,c/3");
    }

    #[test] fn fallback_never_wraps() { assert_eq!(show(&suffix(&parse("claude/sonnet,codex/gpt-6.1-sol"), "codex/gpt-6.1-sol")), ""); }
}
