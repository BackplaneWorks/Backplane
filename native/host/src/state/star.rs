//! The optional GitHub star suggestion: one offer per hub home, after the fifth new human prompt, or after a few days of use
//! (`DAYS_S` since the first prompt, at a later prompt with at least two counted).
//! The ledger is the setting `star.nudge.v1`: at most five accepted message ids (bounded, however long the log), then `sent`;
//! `later` snoozes it for `SNOOZE_S` (status `snoozed`, `snoozeUntil`), after which it is out again under the same token;
//! opened/dismissed (the card's "Star on GitHub" / "Don't ask again") are terminal and only the offered token can decide. Pure: each function reads the setting's text
//! and returns the SettingSet events to commit (empty when nothing may be written).
use super::json::*;
use serde_json::{json, Value};

pub const KEY: &str = "star.nudge.v1";
pub const URL: &str = "https://github.com/BackplaneWorks/Backplane";
pub const BODY: &str = "Like Backplane? Give us a star on GitHub!";
/// the banner's link (it opens URL and decides `open`); its close decides `dismiss`
pub const LABEL: &str = "Star on GitHub";
/// the repository as the card names it, and the line on why a star helps
pub const REPO: &str = "BackplaneWorks/Backplane";
pub const WHY: &str = "Stars help other hardware designers find Backplane, and tell us what to keep building.";
/// "Later" puts it off for about a week
pub const SNOOZE_S: u64 = 7 * 86_400;
/// a few days of use: offered at a prompt this long after the first, with at least two prompts counted
pub const DAYS_S: u64 = 3 * 86_400;

/// Star.offer: what every client shows while the offer is out and undecided (`{token, text, url, label}`), else null. Clients show
/// exactly this: at the offer, on (re)connect (`star.offer`), and they hide it when a `{"t":"star","offer":null}` frame says it was decided.
/// A snoozed offer is out again from its `snoozeUntil`.
pub fn offer(raw: &str, now: u64) -> Value {
    let o = parse_or_null(raw); let token = gstr(&o, "token", "");
    if !out(&o, now) || token.is_empty() || ids(&o).is_empty() || ids(&o).len() > 5 { return Value::Null }
    json!({"token": token, "text": BODY, "url": URL, "label": LABEL, "repo": REPO, "why": WHY})
}
/// sent, or snoozed and past its snooze
fn out(o: &Value, now: u64) -> bool {
    match gstr(o, "status", "") { "sent" => true, "snoozed" => now >= o.get("snoozeUntil").and_then(Value::as_u64).unwrap_or(u64::MAX), _ => false }
}
/// whether a snoozed offer has come out again by now (the next prompt pushes it to the person's connection)
pub fn woke(raw: &str, now: u64) -> bool { let o = parse_or_null(raw); gstr(&o, "status", "") == "snoozed" && out(&o, now) }
/// Star.replay (Settings, Developer): the offer made again now, whatever was decided, under a fresh token so it is decided again like the first.
/// The ids are marked `dev-` (no person's prompt), so the ledger stays bounded and well formed.
pub fn replay(now: u64) -> Vec<Value> {
    let ids: Vec<String> = (1..=5).map(|i| format!("dev-{now}-{i}")).collect();
    let v = json!({"status": "sent", "ids": ids, "token": ids[4], "eligibleAt": now, "sentAt": now, "dev": true});
    vec![set(show(&v), now)]
}
/// Settings, Developer: where the suggestion stands, in a few words
pub fn standing(raw: &str) -> String {
    let o = parse_or_null(raw);
    match gstr(&o, "status", "") {
        "" if raw.is_empty() => "Not offered yet: 0 of 5 prompts".into(),
        "counting" => format!("Not offered yet: {} of 5 prompts", ids(&o).len()),
        "sent" => "Offered, not decided".into(),
        "snoozed" => "Snoozed (Later)".into(),
        "opened" => "Opened".into(),
        "dismissed" => "Dismissed".into(),
        _ => "Unreadable".into(),
    }
}
/// The frame that carries `offer` to clients (targeted at the fifth prompt; to every client when it is decided)
pub fn frame(offer: Value) -> Value { json!({"t": "star", "offer": offer}) }

fn ids(o: &Value) -> Vec<&str> { elems(gobj(o, "ids")).iter().filter_map(|x| x.as_str()).collect() }
fn valid_counting(o: &Value) -> bool {
    let ids = ids(o); let n = ids.len();
    gstr(o, "status", "") == "counting" && n > 0 && n < 5 && o.get("firstAt").map_or(true, |f| f.is_u64())
        && ids.iter().enumerate().all(|(i, x)| !x.is_empty() && !ids[i + 1..].contains(x))
        && n == elems(gobj(o, "ids")).len() && gstr(o, "token", "") == *ids.last().unwrap_or(&"")
}
/// Star.counting: an absent key is a pre-feature install; damaged state fails closed
fn counting(raw: &str) -> bool { raw.is_empty() || valid_counting(&parse_or_null(raw)) }
fn set(value: String, now: u64) -> Value { json!({"$": "SettingSet", "key": KEY, "value": value, "at": now}) }

/// Star.accept: an accepted message id (never an RPC id or a text hash), at most five remembered
pub fn accept(raw: &str, id: &str, now: u64) -> Vec<Value> {
    let o = parse_or_null(raw); let have = ids(&o);
    if !(counting(raw) && !id.is_empty() && have.len() < 5 && !have.contains(&id)) { return vec![] }
    // the first prompt's time (a ledger from before it was kept starts its days at this prompt)
    let first = if have.is_empty() { now } else { o.get("firstAt").and_then(Value::as_u64).unwrap_or(now) };
    let mut all: Vec<&str> = have; all.push(id); all.truncate(5);
    let sent = all.len() >= 5 || (all.len() >= 2 && now >= first.saturating_add(DAYS_S)); let t = if sent { now } else { 0 };
    let v = json!({"status": if sent { "sent" } else { "counting" }, "ids": all, "token": id, "eligibleAt": t, "sentAt": t, "firstAt": first});
    vec![set(show(&v), now)]
}
/// Star.decide: open and dismiss are terminal, later snoozes it for SNOOZE_S; only the offered token decides, and only while it is out
pub fn decide(raw: &str, token: &str, action: &str, now: u64) -> Vec<Value> {
    let o = parse_or_null(raw);
    if !(out(&o, now) && gstr(&o, "token", "") == token && !token.is_empty() && matches!(action, "dismiss" | "open" | "later")) { return vec![] }
    let status = match action { "open" => "opened", "later" => "snoozed", _ => "dismissed" };
    let mut v = json!({"status": status, "ids": gobj(&o, "ids"), "token": token,
                   "eligibleAt": gobj(&o, "eligibleAt"), "sentAt": gobj(&o, "sentAt"), "decidedAt": now});
    if action == "later" { v["snoozeUntil"] = json!(now + SNOOZE_S); }
    vec![set(show(&v), now)]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apply(raw: &str, evs: Vec<Value>) -> String { evs.first().map(|e| e["value"].as_str().unwrap().to_string()).unwrap_or_else(|| raw.into()) }
    #[test] fn star_nudge_rejected_accept() {
        assert!(accept("", "", 1).is_empty()); assert!(accept("damaged", "m1", 1).is_empty());
        let a = apply("", accept("", "m1", 1)); assert!(accept(&a, "m1", 2).is_empty());
    }
    #[test] fn star_nudge_rejected_decision() {
        let mut raw = String::new(); for i in 1..=4 { raw = apply(&raw, accept(&raw, &format!("m{i}"), i)); }
        assert!(decide(&raw, "m4", "open", 9).is_empty()); // still counting
        raw = apply(&raw, accept(&raw, "m5", 5)); assert!(raw.contains("\"status\":\"sent\""));
        assert!(decide(&raw, "stale", "open", 9).is_empty() && decide(&raw, "", "open", 9).is_empty() && decide(&raw, "m5", "bogus", 9).is_empty());
        let d = apply(&raw, decide(&raw, "m5", "open", 9)); assert!(d.contains("\"status\":\"opened\"") && d.contains("\"decidedAt\":9"));
        assert!(decide(&d, "m5", "dismiss", 10).is_empty() && accept(&d, "m6", 11).is_empty());
    }
    #[test] fn star_nudge_official_repository() { assert_eq!(URL, "https://github.com/BackplaneWorks/Backplane"); }
    #[test] fn star_nudge_words() { assert_eq!(BODY, "Like Backplane? Give us a star on GitHub!"); assert_eq!(LABEL, "Star on GitHub"); assert_eq!(REPO, "BackplaneWorks/Backplane"); }
    fn sent5() -> String { let mut raw = String::new(); for i in 1..=5 { raw = apply(&raw, accept(&raw, &format!("m{i}"), i)); } raw }
    #[test] fn later_snoozes_it_for_a_week_then_it_is_out_again() {
        let raw = sent5();
        let s = apply(&raw, decide(&raw, "m5", "later", 100)); assert_eq!(standing(&s), "Snoozed (Later)");
        assert_eq!(offer(&s, 101), Value::Null, "hidden at once"); assert_eq!(offer(&s, 100 + SNOOZE_S - 1), Value::Null, "for the week");
        assert!(!woke(&s, 101) && woke(&s, 100 + SNOOZE_S));
        assert!(decide(&s, "m5", "open", 101).is_empty(), "nothing decides it while it sleeps");
        let o = offer(&s, 100 + SNOOZE_S); assert_eq!(o["token"], "m5", "out again under its token");
        let again = apply(&s, decide(&s, "m5", "later", 100 + SNOOZE_S)); assert_eq!(offer(&again, 100 + SNOOZE_S + 1), Value::Null, "later again");
        let d = apply(&s, decide(&s, "m5", "dismiss", 100 + SNOOZE_S)); assert_eq!(standing(&d), "Dismissed");
        assert_eq!(offer(&d, u64::MAX), Value::Null, "don't ask again: never again"); assert!(accept(&d, "m9", 1 << 40).is_empty());
        let op = apply(&raw, decide(&raw, "m5", "open", 9)); assert_eq!(offer(&op, u64::MAX), Value::Null, "starred: never again");
    }
    #[test] fn a_few_days_of_use_offer_it_before_five_prompts() {
        let a = apply("", accept("", "m1", 1000));
        let b = apply(&a, accept(&a, "m2", 1000 + DAYS_S - 1)); assert_eq!(offer(&b, 1000 + DAYS_S), Value::Null, "two prompts, not yet three days");
        let c = apply(&b, accept(&b, "m3", 1000 + DAYS_S)); assert_eq!(offer(&c, 1000 + DAYS_S)["token"], "m3", "three days after the first");
        let one = apply("", accept("", "m1", 1)); assert_eq!(standing(&one), "Not offered yet: 1 of 5 prompts");
        let old = r#"{"status":"counting","ids":["m1"],"token":"m1","eligibleAt":0,"sentAt":0}"#;
        let o2 = apply(old, accept(old, "m2", 5000)); assert!(o2.contains(r#""firstAt":5000"#), "an old ledger starts its days now: {o2}");
    }
    #[test] fn star_offer_is_shown_only_while_sent_and_undecided() {
        let mut raw = String::new(); assert_eq!(offer(&raw, 9), Value::Null, "a fresh home");
        for i in 1..=4 { raw = apply(&raw, accept(&raw, &format!("m{i}"), i)); assert_eq!(offer(&raw, 9), Value::Null, "after {i} prompts"); }
        raw = apply(&raw, accept(&raw, "m5", 5));
        assert_eq!(offer(&raw, 9), json!({"token":"m5","text":BODY,"url":URL,"label":LABEL,"repo":REPO,"why":WHY}), "the fifth: shown, and again on every reconnect");
        assert_eq!(offer(&raw, 9), offer(&raw, 9));
        for a in ["open", "dismiss"] { let d = apply(&raw, decide(&raw, "m5", a, 9)); assert_eq!(offer(&d, u64::MAX), Value::Null, "{a} is final"); }
        assert_eq!(offer("damaged", 9), Value::Null); assert_eq!(offer(r#"{"status":"sent","ids":[],"token":"a"}"#, 9), Value::Null, "an empty ledger is not an offer");
        assert_eq!(frame(Value::Null), json!({"t":"star","offer":null}));
    }
    #[test] fn star_replay_offers_again_under_a_fresh_token() {
        let mut raw = String::new(); for i in 1..=5 { raw = apply(&raw, accept(&raw, &format!("m{i}"), i)); }
        raw = apply(&raw, decide(&raw, "m5", "dismiss", 6)); assert_eq!(standing(&raw), "Dismissed");
        let r = apply(&raw, replay(100)); let o = offer(&r, 100);
        assert_eq!(o["token"], "dev-100-5"); assert_eq!(o["text"], BODY); assert_eq!(standing(&r), "Offered, not decided");
        assert!(decide(&r, "m5", "open", 101).is_empty(), "the old token decides nothing");
        let d = apply(&r, decide(&r, "dev-100-5", "open", 101)); assert_eq!(offer(&d, 101), Value::Null); assert_eq!(standing(&d), "Opened");
        assert!(accept(&r, "m9", 102).is_empty(), "a replay counts nothing more");
        assert_eq!(standing(""), "Not offered yet: 0 of 5 prompts"); assert_eq!(standing(&apply("", accept("", "m1", 1))), "Not offered yet: 1 of 5 prompts");
    }
    #[test] fn star_ledger_is_bounded_at_five_ids() {
        let mut raw = String::new(); for i in 1..=9 { raw = apply(&raw, accept(&raw, &format!("m{i}"), i)); }
        assert_eq!(raw, r#"{"status":"sent","ids":["m1","m2","m3","m4","m5"],"token":"m5","eligibleAt":5,"sentAt":5,"firstAt":1}"#);
    }
}
