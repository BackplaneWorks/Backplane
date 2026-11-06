//! The fallback chain: one ordered, opt-in list of `provider/model` per hub. Never wraps: an exhausted suffix blocks until a
//! person intervenes. Ported here: parsing, validity against the installed models, editing and picking; the outage classifier (`classify`: only a provider's own
//! error envelope, never arbitrary text); holds on a provider or model; and the planner over named accounts (core/account-fallback: which account is
//! eligible, the one with the most allowance left, when a hold ends, the thread's preference). What is NOT here is the driver: reading a failed turn,
//! writing the hold, switching the thread to the next account or provider and starting it again belongs to the agents' turn handling, which calls these.
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
#[derive(Clone, Debug, PartialEq)]
pub struct Limit { pub scope: String, pub reason: String, pub reset: u64 }
impl Limit {
    /// Limit.json: the record kept as the setting `fallback.outage.<scope>` (the reason cut to 400 characters).
    pub fn json(&self, now: u64) -> String { json!({"scope":self.scope,"reason":self.reason.chars().take(400).collect::<String>(),"reset":self.reset,"at":now}).to_string() }
    pub fn key(scope: &str) -> String { format!("fallback.outage.{scope}") }
}
/// Limit.active.o: a record holds from its start until its reset; an unknown reset (0) holds until cleared.
pub fn limit_active(o: &Value, now: u64) -> bool { !o["scope"].as_str().unwrap_or("").is_empty() && (o["reset"].as_u64().unwrap_or(0) == 0 || now < o["reset"].as_u64().unwrap_or(0)) }
/// Target.held: the provider or the `provider/model` is held (`get` reads a setting).
pub fn target_held(get: &dyn Fn(&str) -> String, t: &Target, now: u64) -> bool {
    let rec = |scope: &str| serde_json::from_str::<Value>(&get(&Limit::key(scope))).unwrap_or(Value::Null);
    limit_active(&rec(&t.provider), now) || limit_active(&rec(&t.key()), now)
}
/// Chain.pick: the first target that is offered and not held, or None (an exhausted chain holds work; it never wraps).
pub fn pick(xs: &[Target], get: &dyn Fn(&str) -> String, caps: &Value, now: u64) -> Option<Target> { xs.iter().find(|t| caps_valid(caps, t) && !target_held(get, t, now)).cloned() }

/// Error.code
fn quota_code(c: &str) -> bool { matches!(c, "rate_limit_error" | "rate_limit_exceeded" | "insufficient_quota" | "usage_limit_reached" | "budget_exceeded" | "quota_exceeded") }
/// Error.words (on the lower-cased reason)
fn quota_words(s: &str) -> bool { ["hit your usage limit", "you've hit your limit", "you have hit your limit", "weekly limit", "usage limit reached", "usage limit has been reached", "insufficient quota", "rate limit exceeded", "budget exceeded"].iter().any(|w| s.contains(w)) }
/// Error.no: sign-in and cancellation words are never a capacity signal.
fn not_quota(s: &str) -> bool { ["authentication", "unauthorized", "log in", "login required", "cancelled", "canceled", "invalid api key", "keyring"].iter().any(|w| s.contains(w)) }
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
/// Error.reset: the provider's reset time (0 unknown).
pub fn reset_of(o: &Value) -> u64 { o["error"]["resets_at"].as_u64().or(o["resets_at"].as_u64()).or(o["rate_limit_info"]["resetsAt"].as_u64()).unwrap_or(0) }
/// Error.scope: an account-wide limit holds the provider; a model limit holds `provider/model`. An explicit protocol scope wins over wording.
pub fn scope_of(p: &str, m: &str, o: &Value) -> String {
    let explicit = o["error"]["scope"].as_str().or(o["scope"].as_str()).unwrap_or("");
    let low = reason(o).to_lowercase();
    let account = explicit != "model" && (explicit == "account" || rejected(o) || assistant(o) || (!low.contains("hit your usage limit for ") && (low.contains("weekly limit") || low.contains("usage limit"))));
    if account { p.to_string() } else { format!("{p}/{m}") }
}
/// Error.classify: only a provider error envelope is read, never arbitrary text. Auth, cancellations and network failures are not capacity signals.
pub fn classify(p: &str, m: &str, o: &Value) -> Option<Limit> {
    let low = reason(o).to_lowercase();
    let code = o["error"]["code"].as_str().or(o["error"]["type"].as_str()).unwrap_or("");
    let signal = rejected(o) || assistant(o) || envelope(o);
    if signal && !not_quota(&low) && (rejected(o) || assistant(o) || quota_code(code) || quota_words(&low)) { Some(Limit { scope: scope_of(p, m, o), reason: reason(o), reset: reset_of(o) }) } else { None }
}

// ---- the planner over named accounts: holds, eligibility, the best account, the thread's preference ----

/// Hold.key: the hold on one profile at one epoch (`""` profile is the CLI's own login); the per-model hold adds `model:<model>`. Parts are length-prefixed.
pub fn part(s: &str) -> String { format!("{}:{s}", s.chars().count()) }
pub fn hold_key(provider: &str, id: &str, epoch: u64) -> String { format!("account.limit.{}{}{epoch}:", part(provider), part(id)) }
pub fn hold_model_key(provider: &str, id: &str, epoch: u64, model: &str) -> String { format!("{}model:{}", hold_key(provider, id, epoch), part(model)) }
fn rec(get: &dyn Fn(&str) -> String, key: &str) -> Value { serde_json::from_str(&get(key)).unwrap_or(Value::Null) }
/// Hold.current: the profile (or this model on it) is held now.
pub fn hold_current(get: &dyn Fn(&str) -> String, provider: &str, id: &str, epoch: u64, model: &str, now: u64) -> bool {
    limit_active(&rec(get, &hold_key(provider, id, epoch)), now) || limit_active(&rec(get, &hold_model_key(provider, id, epoch, model)), now)
}
/// Hold.until: when the holds on this profile and model end (0: unknown, held until cleared).
pub fn hold_until(get: &dyn Fn(&str) -> String, provider: &str, id: &str, epoch: u64, model: &str) -> u64 {
    let (a, b) = (rec(get, &hold_key(provider, id, epoch)), rec(get, &hold_model_key(provider, id, epoch, model)));
    let unknown = |o: &Value| !o["scope"].as_str().unwrap_or("").is_empty() && o["reset"].as_u64().unwrap_or(0) == 0;
    if unknown(&a) || unknown(&b) { 0 } else { a["reset"].as_u64().unwrap_or(0).max(b["reset"].as_u64().unwrap_or(0)) }
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

#[cfg(test)]
mod outage_tests {
    use super::*;
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
    #[test] fn a_stale_future_or_undated_allowance_scores_nothing() {
        let mk = |at: Value| json!({"provider":"claude","allowance":{"state":"ok","phase":"complete","provider":"claude","at":at,"windows":[{"scope":"five_hour","used_percent":10.0}]}});
        assert!(usage_score(&mk(json!(1000)), 1100, "m").is_some(), "fresh");
        assert_eq!(usage_score(&mk(json!(1000)), 1301, "m"), None, "older than five minutes");
        assert_eq!(usage_score(&mk(json!(2000)), 1100, "m"), None, "from the future");
        assert_eq!(usage_score(&mk(json!(0)), 1100, "m"), None, "undated");
    }
    #[test] fn the_limit_record_keeps_four_fields_and_a_cut_reason() { let l = Limit { scope: "claude".into(), reason: "x".repeat(500), reset: 7 }; let v: Value = serde_json::from_str(&l.json(3)).unwrap(); assert_eq!((v["reason"].as_str().unwrap().len(), v["reset"].clone(), v["at"].clone()), (400, json!(7), json!(3))); assert_eq!(Limit::key("codex/m"), "fallback.outage.codex/m"); }
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
        let unknown = |k: &str| if k == hold_key("claude", "a", 1) { json!({"scope":"claude","reset":0}).to_string() } else { String::new() }; assert_eq!(hold_until(&unknown, "claude", "a", 1, "opus"), 0, "an unknown reset stays unknown");
    }
    #[test] fn a_hold_is_marked_on_the_provider_or_the_model_with_length_prefixed_keys() {
        assert_eq!(hold_key("claude", "w", 3), "account.limit.6:claude1:w3:"); assert_eq!(hold_model_key("claude", "w", 3, "opus"), "account.limit.6:claude1:w3:model:4:opus");
        let l = Limit { scope: "claude/opus".into(), reason: "r".into(), reset: 9 }; assert_eq!(hold_mark("claude", "w", 3, &l, 5, 5)["key"], "account.limit.6:claude1:w3:model:4:opus");
        let p = Limit { scope: "claude".into(), reason: "r".into(), reset: 9 }; assert_eq!(hold_mark("claude", "w", 3, &p, 5, 5)["key"], "account.limit.6:claude1:w3:"); assert_eq!(preference_key("t1"), "account.failover.preference.t1");
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
