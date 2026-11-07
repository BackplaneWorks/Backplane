//! The optional GitHub star suggestion: one offer per hub home, after the fifth new human prompt.
//! The ledger is the setting `star.nudge.v1`: at most five accepted message ids (bounded, however long the log), then `sent`;
//! a decision (opened/dismissed) is terminal and only the offered token can make it. Pure: each function reads the setting's text
//! and returns the SettingSet events to commit (empty when nothing may be written).
use super::json::*;
use serde_json::{json, Value};

pub const KEY: &str = "star.nudge.v1";
pub const URL: &str = "https://github.com/BackplaneWorks/Backplane";
pub const BODY: &str = "Like Backplane? Give us a star on GitHub!";
/// the banner's link (it opens URL and decides `open`); its close decides `dismiss`
pub const LABEL: &str = "Star on GitHub";

/// Star.offer: what every client shows while the offer is out and undecided (`{token, text, url, label}`), else null. Clients show
/// exactly this: at the offer, on (re)connect (`star.offer`), and they hide it when a `{"t":"star","offer":null}` frame says it was decided.
pub fn offer(raw: &str) -> Value {
    let o = parse_or_null(raw); let token = gstr(&o, "token", "");
    if gstr(&o, "status", "") != "sent" || token.is_empty() || ids(&o).len() != 5 { return Value::Null }
    json!({"token": token, "text": BODY, "url": URL, "label": LABEL})
}
/// The frame that carries `offer` to clients (targeted at the fifth prompt; to every client when it is decided)
pub fn frame(offer: Value) -> Value { json!({"t": "star", "offer": offer}) }

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
    #[test] fn star_nudge_words() { assert_eq!(BODY, "Like Backplane? Give us a star on GitHub!"); assert_eq!(LABEL, "Star on GitHub"); }
    #[test] fn star_offer_is_shown_only_while_sent_and_undecided() {
        let mut raw = String::new(); assert_eq!(offer(&raw), Value::Null, "a fresh home");
        for i in 1..=4 { raw = apply(&raw, accept(&raw, &format!("m{i}"), i)); assert_eq!(offer(&raw), Value::Null, "after {i} prompts"); }
        raw = apply(&raw, accept(&raw, "m5", 5));
        assert_eq!(offer(&raw), json!({"token":"m5","text":BODY,"url":URL,"label":LABEL}), "the fifth: shown, and again on every reconnect");
        assert_eq!(offer(&raw), offer(&raw));
        for a in ["open", "dismiss"] { let d = apply(&raw, decide(&raw, "m5", a, 9)); assert_eq!(offer(&d), Value::Null, "{a} is final"); }
        assert_eq!(offer("damaged"), Value::Null); assert_eq!(offer(r#"{"status":"sent","ids":["a"],"token":"a"}"#), Value::Null, "a short ledger is not an offer");
        assert_eq!(frame(Value::Null), json!({"t":"star","offer":null}));
    }
    #[test] fn star_ledger_is_bounded_at_five_ids() {
        let mut raw = String::new(); for i in 1..=9 { raw = apply(&raw, accept(&raw, &format!("m{i}"), i)); }
        assert_eq!(raw, r#"{"status":"sent","ids":["m1","m2","m3","m4","m5"],"token":"m5","eligibleAt":5,"sentAt":5}"#);
    }
}
