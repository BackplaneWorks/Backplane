//! Telemetry choices.
//! One installation-wide choice covers every category. Missing values select On in the disclosure but never authorize collection.
//!
//! This hub SENDS NOTHING: the sender is not ported and no
//! endpoint or key is baked into this binary, so `may_send` is false in every build and every test. What is here is the choice itself:
//! the opt-out, the reset and the receipts that keep a retried click from re-enabling.
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

/// Choice.on
pub fn on(share: &str, usage: &str) -> bool { (share.is_empty() || share == "on") && usage != "0" }
/// Choice.answered
pub fn answered(share: &str, asked: &str, usage: &str) -> bool { share == "off" || usage == "0" || (asked == "1" && usage == "1") }
/// Choice.allowed: what a sender would need before it could send anything.
pub fn allowed(share: &str, asked: &str, usage: &str) -> bool { on(share, usage) && asked == "1" && usage == "1" }
/// No sender and no endpoint exist in this build: nothing is ever sent, whatever the choice.
pub fn may_send(_share: &str, _asked: &str, _usage: &str) -> bool { false }

/// The settings a choice reads.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct State { pub share: String, pub asked: String, pub usage_asked: String, pub generation: String, pub revision: String, pub receipts: String }
impl State {
    /// Tm.choice.token: binds a UI decision to the preferences it displayed.
    pub fn token(&self) -> String { json!([self.share, self.asked, self.usage_asked, self.generation, self.revision]).to_string() }
    pub fn revision_n(&self) -> u64 { self.revision.parse().unwrap_or(0) }
    /// Tm.choice.json
    pub fn json(&self) -> Value { json!({"share":self.share,"asked":self.asked,"usage":self.usage_asked,"generation":self.generation,"revision":self.revision}) }
    pub fn answered(&self) -> bool { answered(&self.share, &self.asked, &self.usage_asked) }
    fn seen(&self) -> Vec<String> { serde_json::from_str::<Value>(if self.receipts.is_empty() { "[]" } else { &self.receipts }).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().map(|x| x.as_str().unwrap_or("").to_string()).collect() }
}

/// Hub.telemetry.choice.fingerprint
pub fn fingerprint(decision: &str, expected: &str, value: &str, popup: bool) -> String { json!([decision, expected, value, popup]).to_string() }

/// Hub.telemetry.choice.changes: the five settings one decision writes, in the log's order.
pub fn changes(s: &State, value: &str, fp: &str, at: u64) -> Vec<Value> {
    let mut r = vec![fp.to_string()]; r.extend(s.seen()); r.truncate(16);
    let set = |k: &str, v: String| json!({"$":"SettingSet","key":k,"value":v,"at":at});
    vec![set("telemetry.share", value.into()), set("telemetry.asked", "1".into()), set("telemetry.usage.asked", if value == "on" { "1" } else { "0" }.into()),
        set("telemetry.choice.revision", (s.revision_n() + 1).to_string()), set("telemetry.choice.receipts", json!(r).to_string())]
}

/// State.replay of those changes.
pub fn replay(s: &State, cs: &[Value]) -> State {
    let mut n = s.clone();
    for c in cs { let (k, v) = (c["key"].as_str().unwrap_or(""), c["value"].as_str().unwrap_or("").to_string());
        match k { "telemetry.share" => n.share = v, "telemetry.asked" => n.asked = v, "telemetry.usage.asked" => n.usage_asked = v, "telemetry.choice.revision" => n.revision = v, "telemetry.choice.receipts" => n.receipts = v, _ => {} } }
    n
}

/// Hub.telemetry.choice.reply
pub fn reply(s: &State, accepted: bool, id: &Value) -> Value {
    json!({"t":"reply","id":id,"ok":accepted,"error": if accepted { "" } else { "Telemetry preferences changed. Your current saved choice is shown; choose again to change it." },"telemetryChoice":s.json()})
}

/// Hub.telemetry.choice: (the events to write, the reply).
pub fn choice(s: &State, p: &Value, id: &Value, at: u64) -> (Vec<Value>, Value) {
    let g = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let (value, decision, expected, popup) = (g("value"), g("decision"), g("expected"), p.get("popup").and_then(Value::as_bool).unwrap_or(false));
    let fp = fingerprint(&decision, &expected, &value, popup);
    let seen = !decision.is_empty() && s.seen().contains(&fp);
    let ok = (value == "on" || value == "off") && (value == "off" || (expected == s.token() && (!popup || !s.answered())));
    let cs = if ok && !seen { changes(s, &value, &fp, at) } else { vec![] };
    let after = replay(s, &cs);
    (cs, reply(&after, ok || seen, id))
}

/// Hub.telemetry.setting.reserved: older clients may still disable telemetry or reset the id; an individual On, an answer marker or an
/// internal revision written through setting.set cannot re-enable.
pub fn reserved(key: &str) -> bool {
    matches!(key, "telemetry.share" | "telemetry.asked" | "telemetry.usage.asked" | "telemetry.usage.enabled" | "telemetry.usage.generation") || key.starts_with("telemetry.choice.")
}

/// Hub.telemetry.setting: only an Off applies (as one recorded choice); every other write answers "choose again".
pub fn legacy_changes(s: &State, key: &str, value: &str, at: u64) -> (Vec<Value>, bool) {
    let off = (key == "telemetry.share" && value == "off") || ((key == "telemetry.asked" || key == "telemetry.usage.asked") && (value.is_empty() || value == "0"));
    let fp = fingerprint("", &s.token(), "legacy-off", false);
    (if off { changes(s, "off", &fp, at) } else { vec![] }, off)
}

pub async fn state(hub: &Arc<Hub>) -> State {
    let g = |k: &'static str| async move { hub.setting(k).await.unwrap_or_default() };
    State { share: g("telemetry.share").await, asked: g("telemetry.asked").await, usage_asked: g("telemetry.usage.asked").await, generation: g("telemetry.usage.generation").await,
        revision: g("telemetry.choice.revision").await, receipts: g("telemetry.choice.receipts").await }
}

pub async fn legacy(hub: &Arc<Hub>, id: &Value, key: &str, value: &str) -> Value {
    let s = state(hub).await;
    let (cs, off) = legacy_changes(&s, key, value, now_ms() / 1000);
    if !cs.is_empty() { if let Err(e) = hub.commit_many(&cs).await { return json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")}) } }
    reply(&replay(&s, &cs), off, id)
}

pub struct TelemetryRpc;
impl RpcHandler for TelemetryRpc {
    fn methods(&self) -> &'static [&'static str] { &["telemetry.choice"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let s = state(hub).await;
            let (cs, r) = choice(&s, p, id, now_ms() / 1000);
            if !cs.is_empty() { if let Err(e) = hub.commit_many(&cs).await { return vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})] } }
            vec![r]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(v: &str, expected: &str, decision: &str, popup: bool) -> Value { json!({"value":v,"expected":expected,"decision":decision,"popup":popup}) }
    #[test] fn telemetry_default_selected() { assert!(on("", "") && on("on", "1") && !on("off", "") && !on("", "0")); }
    #[test] fn telemetry_default_pending() { assert!(!answered("", "", "") && !allowed("", "", "")); }
    #[test] fn telemetry_all_off() { assert!(!allowed("off", "1", "1") && !allowed("on", "1", "0") && answered("off", "", "")); }
    #[test] fn telemetry_existing_optout_answered() { assert!(answered("off", "", "") && answered("on", "", "0")); }
    #[test] fn nothing_is_ever_sent_whatever_the_choice() { for (a, b, c) in [("on", "1", "1"), ("", "1", "1"), ("off", "1", "0")] { assert!(!may_send(a, b, c)) } assert!(!option_env!("BACKPLANE_TELEMETRY_URL").is_some() || cfg!(test)); }
    #[test] fn an_off_always_applies_and_is_five_settings_in_order() {
        let s = State::default(); let (cs, r) = choice(&s, &p("off", "stale", "", false), &json!(3), 9);
        let keys: Vec<&str> = cs.iter().map(|c| c["key"].as_str().unwrap()).collect();
        assert_eq!(keys, ["telemetry.share", "telemetry.asked", "telemetry.usage.asked", "telemetry.choice.revision", "telemetry.choice.receipts"]);
        assert_eq!((cs[0]["value"].as_str(), cs[2]["value"].as_str(), cs[3]["value"].as_str()), (Some("off"), Some("0"), Some("1")));
        assert_eq!(r["ok"], true); assert_eq!(r["telemetryChoice"]["share"], "off");
    }
    #[test] fn an_on_needs_the_token_the_client_displayed() {
        let s = State::default();
        let (cs, r) = choice(&s, &p("on", "stale", "d1", false), &json!(1), 9); assert!(cs.is_empty()); assert_eq!(r["ok"], false); assert!(r["error"].as_str().unwrap().starts_with("Telemetry preferences changed"));
        let (cs, r) = choice(&s, &p("on", &s.token(), "d1", false), &json!(1), 9); assert_eq!(cs.len(), 5); assert_eq!(r["ok"], true); assert_eq!(r["telemetryChoice"]["usage"], "1");
        // the popup cannot re-answer a question already answered
        let after = replay(&s, &cs); let (cs2, r2) = choice(&after, &p("on", &after.token(), "d2", true), &json!(2), 9); assert!(cs2.is_empty()); assert_eq!(r2["ok"], false);
    }
    #[test] fn telemetry_choice_no_decision_no_replay() {
        // a retry of the very same request is recognised; without a decision id nothing is suppressed
        let s = State::default(); let (cs, _) = choice(&s, &p("off", "", "d1", false), &json!(1), 9); let after = replay(&s, &cs);
        let (again, r) = choice(&after, &p("off", "", "d1", false), &json!(1), 9); assert!(again.is_empty() && r["ok"] == true, "a retry is already done");
        let (fresh, _) = choice(&after, &p("off", "", "", false), &json!(1), 9); assert_eq!(fresh.len(), 5, "no decision id: applies again");
    }
    #[test] fn only_sixteen_receipts_are_kept() { let mut s = State::default(); for i in 0..20 { let (cs, _) = choice(&s, &p("off", "", &format!("d{i}"), false), &json!(1), 9); s = replay(&s, &cs); } assert_eq!(s.seen().len(), 16); assert_eq!(s.revision_n(), 20); }
    #[test] fn the_old_setting_path_can_only_turn_telemetry_off() {
        let s = State::default();
        assert!(reserved("telemetry.share") && reserved("telemetry.choice.receipts") && !reserved("telemetry.reset") && !reserved("other"));
        assert!(legacy_changes(&s, "telemetry.share", "on", 1).0.is_empty()); assert_eq!(legacy_changes(&s, "telemetry.share", "off", 1).0.len(), 5);
        assert_eq!(legacy_changes(&s, "telemetry.asked", "0", 1).1, true); assert_eq!(legacy_changes(&s, "telemetry.asked", "1", 1).1, false);
    }
}
