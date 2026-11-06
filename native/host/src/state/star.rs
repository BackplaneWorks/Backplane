//! The optional GitHub star suggestion: one offer per hub home, after the fifth new human prompt.
//! The ledger is the setting `star.nudge.v1`: at most five accepted message ids (bounded, however long the log), then `sent`;
//! a decision (opened/dismissed) is terminal and only the offered token can make it. Pure: each function reads the setting's text
//! and returns the SettingSet events to commit (empty when nothing may be written).
use super::json::*;
use serde_json::{json, Value};

pub const KEY: &str = "star.nudge.v1";
pub const URL: &str = "https://github.com/BackplaneWorks/Backplane";
pub const BODY: &str = "Enjoying Backplane? A star on GitHub helps others find us.";

fn ids(o: &Value) -> Vec<&str> { elems(gobj(o, "ids")).iter().filter_map(|x| x.as_str()).collect() }
fn valid_counting(o: &Value) -> bool {
    let ids = ids(o); let n = ids.len();
    gstr(o, "status", "") == "counting" && n > 0 && n < 5
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
    let mut all: Vec<&str> = have; all.push(id); all.truncate(5);
    let sent = all.len() >= 5; let t = if sent { now } else { 0 };
    let v = json!({"status": if sent { "sent" } else { "counting" }, "ids": all, "token": id, "eligibleAt": t, "sentAt": t});
    vec![set(show(&v), now)]
}
/// Star.decide: both actions are terminal; only the offered token decides
pub fn decide(raw: &str, token: &str, action: &str, now: u64) -> Vec<Value> {
    let o = parse_or_null(raw);
    if !(gstr(&o, "status", "") == "sent" && gstr(&o, "token", "") == token && !token.is_empty() && (action == "dismiss" || action == "open")) { return vec![] }
    let v = json!({"status": if action == "open" { "opened" } else { "dismissed" }, "ids": gobj(&o, "ids"), "token": token,
                   "eligibleAt": gobj(&o, "eligibleAt"), "sentAt": gobj(&o, "sentAt"), "decidedAt": now});
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
    #[test] fn star_ledger_is_bounded_at_five_ids() {
        let mut raw = String::new(); for i in 1..=9 { raw = apply(&raw, accept(&raw, &format!("m{i}"), i)); }
        assert_eq!(raw, r#"{"status":"sent","ids":["m1","m2","m3","m4","m5"],"token":"m5","eligibleAt":5,"sentAt":5}"#);
    }
}
