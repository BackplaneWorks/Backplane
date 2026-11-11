//! Telemetry choices (docs/telemetry.md). One installation-wide choice covers every category.
//!
//! Opt-out: telemetry is on unless the person turned it off, but nothing is sent before the person could have seen what is sent.
//! The setting `telemetry.notice` records that: "setup" (a new install finished setup with the telemetry step on screen), "shown" (the
//! one-time notice of an existing install was drawn on a client), "closed" (it was closed, or answered with Turn off), "chosen" (On pressed
//! in Settings, Privacy, beside the full list). Off (an explicit Off, or any earlier decline) is final against everything but the person's
//! own later On: upgrades and the notice never flip it. The sender (settings/telemetry_send.rs) asks `may_send` before every request.
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

/// Choice.on
pub fn on(share: &str, usage: &str) -> bool { (share.is_empty() || share == "on") && usage != "0" }
/// Choice.answered
pub fn answered(share: &str, asked: &str, usage: &str) -> bool { share == "off" || usage == "0" || (asked == "1" && usage == "1") }
/// Choice.allowed: what a sender would need before it could send anything.
pub fn allowed(share: &str, asked: &str, usage: &str) -> bool { on(share, usage) && asked == "1" && usage == "1" }
/// The setting that records the disclosure was seen.
pub const NOTICE: &str = "telemetry.notice";
/// Off: an explicit Off (`telemetry.share` off, or the expanded answer 0) or an earlier decline in setup (`onboard.usage` off).
pub fn off(share: &str, usage: &str, onboard_usage: &str) -> bool { share == "off" || usage == "0" || onboard_usage == "off" }
/// Whether telemetry may be sent (a build with an endpoint and key and no kill switch is the sender's own check): not Off, and the
/// person could have seen what is sent.
pub fn may_send(share: &str, usage: &str, onboard_usage: &str, notice: &str) -> bool { !off(share, usage, onboard_usage) && !notice.is_empty() }
/// The token of the one notice there is (one per hub home).
pub const NOTICE_TOKEN: &str = "telemetry-notice-1";
/// What the notice says; the client draws exactly this.
pub const NOTICE_TEXT: &str = "Backplane now sends anonymous usage counts: installs, updates, daily activity, and daily totals of threads, messages, model tokens, bot traffic, boards made and boards ordered, with a random install id, the version, the system and the day. Never your conversations, projects, files, names, paths or keys. It is on. Turn it off here or in Settings, Privacy.";
/// Tm.notice.offer: the one-time notice an existing install gets (`existing`: setup is not what shows the disclosure on this home), while
/// it is not Off and was not closed, in a build that can send. `{token, text, off, settings}` or null.
pub fn notice_offer(s: &State, existing: bool, capable: bool) -> Value {
    if !capable || !existing || off(&s.share, &s.usage_asked, &s.onboard_usage) || !matches!(s.notice.as_str(), "" | "shown") { return Value::Null }
    json!({"token": NOTICE_TOKEN, "text": NOTICE_TEXT, "off": "Turn off", "settings": "Settings"})
}
/// Tm.notice.act: what a press on (or the drawing of) the notice writes. `setup` comes from setup's end, which shows the same words in its
/// telemetry step; the others need the notice to be out under this token. Off is the same recorded choice as Settings' Off.
pub fn notice_act(s: &State, token: &str, action: &str, offered: bool, at: u64) -> Vec<Value> {
    let set = |v: &str| json!({"$":"SettingSet","key":NOTICE,"value":v,"at":at});
    match action {
        "setup" if s.notice.is_empty() => vec![set("setup")],
        _ if !offered || token != NOTICE_TOKEN => vec![],
        "shown" if s.notice.is_empty() => vec![set("shown")],
        "close" => vec![set("closed")],
        "off" => { let mut v = changes(s, "off", &fingerprint("", &s.token(), "notice-off", false), at); v.push(set("closed")); v }
        _ => vec![],
    }
}

/// The settings a choice reads.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct State { pub share: String, pub asked: String, pub usage_asked: String, pub generation: String, pub revision: String, pub receipts: String, pub notice: String, pub onboard_usage: String }
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

/// Hub.telemetry.choice.changes: the five settings one decision writes, in the log's order (an On chosen before the notice was seen also
/// records that it was: Settings shows the full list beside the button).
pub fn changes(s: &State, value: &str, fp: &str, at: u64) -> Vec<Value> {
    let mut r = vec![fp.to_string()]; r.extend(s.seen()); r.truncate(16);
    let set = |k: &str, v: String| json!({"$":"SettingSet","key":k,"value":v,"at":at});
    vec![set("telemetry.share", value.into()), set("telemetry.asked", "1".into()), set("telemetry.usage.asked", if value == "on" { "1" } else { "0" }.into()),
        set("telemetry.choice.revision", (s.revision_n() + 1).to_string()), set("telemetry.choice.receipts", json!(r).to_string())]
        .into_iter().chain((value == "on" && s.notice.is_empty()).then(|| set(NOTICE, "chosen".into()))).collect()
}

/// State.replay of those changes.
pub fn replay(s: &State, cs: &[Value]) -> State {
    let mut n = s.clone();
    for c in cs { let (k, v) = (c["key"].as_str().unwrap_or(""), c["value"].as_str().unwrap_or("").to_string());
        match k { "telemetry.share" => n.share = v, "telemetry.asked" => n.asked = v, "telemetry.usage.asked" => n.usage_asked = v, "telemetry.choice.revision" => n.revision = v, "telemetry.choice.receipts" => n.receipts = v, NOTICE => n.notice = v, _ => {} } }
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
    matches!(key, "telemetry.share" | "telemetry.asked" | "telemetry.usage.asked" | "telemetry.usage.enabled" | "telemetry.usage.generation" | NOTICE) || key.starts_with("telemetry.choice.")
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
        revision: g("telemetry.choice.revision").await, receipts: g("telemetry.choice.receipts").await, notice: g(NOTICE).await, onboard_usage: g("onboard.usage").await }
}
/// The same, from the model's settings.
pub fn of(st: &crate::state::State) -> State {
    let g = |k: &str| st.settings.get(k).cloned().unwrap_or_default();
    State { share: g("telemetry.share"), asked: g("telemetry.asked"), usage_asked: g("telemetry.usage.asked"), generation: g("telemetry.usage.generation"),
        revision: g("telemetry.choice.revision"), receipts: g("telemetry.choice.receipts"), notice: g(NOTICE), onboard_usage: g("onboard.usage") }
}
/// Whether the notice is this home's way to the disclosure: setup is not (present/onboard.rs `due` with no view open: it finished, or the
/// home has projects from before setup and setup never began).
pub fn existing(st: &crate::state::State) -> bool { !crate::present::onboard::due(st, &Value::Null) }
/// The same, from the hub.
pub async fn existing_hub(hub: &Arc<Hub>) -> bool {
    if hub.setting("onboard.done").await.unwrap_or_default() == "1" { return true }
    if !hub.setting("onboard.step").await.unwrap_or_default().is_empty() { return false }
    hub.model().await.projects.values().any(|p| !p.removed)
}
/// The notice as the hub offers it now.
pub async fn offer(hub: &Arc<Hub>) -> Value { let s = state(hub).await; notice_offer(&s, existing_hub(hub).await, crate::settings::telemetry_send::capable()) }
/// After a change: every client hears the notice as it now stands, and the sender looks again.
pub async fn changed(hub: &Arc<Hub>) { let o = offer(hub).await; hub.push_all(&json!({"t":"telemetry.notice","notice":o})); crate::settings::telemetry_send::wake(); }

pub async fn legacy(hub: &Arc<Hub>, id: &Value, key: &str, value: &str) -> Value {
    let s = state(hub).await;
    let (cs, off) = legacy_changes(&s, key, value, now_ms() / 1000);
    if !cs.is_empty() { if let Err(e) = hub.commit_many(&cs).await { return json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")}) } changed(hub).await; }
    reply(&replay(&s, &cs), off, id)
}

pub struct TelemetryRpc;
impl RpcHandler for TelemetryRpc {
    fn methods(&self) -> &'static [&'static str] { &["telemetry.choice", "telemetry.notice", "telemetry.notice.act"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let fail = |e: anyhow::Error| vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})];
            if m == "telemetry.notice" { return vec![json!({"t":"reply","id":id,"ok":true,"result":{"notice":offer(hub).await}})] }
            let s = state(hub).await;
            if m == "telemetry.notice.act" {
                let g = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("");
                let cs = notice_act(&s, g("token"), g("action"), !offer(hub).await.is_null(), now_ms() / 1000);
                if !cs.is_empty() { if let Err(e) = hub.commit_many(&cs).await { return fail(e) } changed(hub).await; }
                return vec![json!({"t":"reply","id":id,"ok":true,"result":{}})]
            }
            let (cs, r) = choice(&s, p, id, now_ms() / 1000);
            if !cs.is_empty() { if let Err(e) = hub.commit_many(&cs).await { return fail(e) } changed(hub).await; }
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
    #[test] fn nothing_is_sent_before_the_disclosure_was_seen_and_never_after_off() {
        for n in ["", "setup", "shown", "closed", "chosen"] { assert!(!may_send("off", "1", "", n) && !may_send("on", "0", "", n) && !may_send("", "", "off", n), "Off stays Off ({n})") }
        assert!(!may_send("", "", "", ""), "unanswered and unseen: nothing");
        for n in ["setup", "shown", "closed", "chosen"] { assert!(may_send("", "", "", n) && may_send("on", "1", "1", n) && may_send("", "", "on", n)) }
    }
    fn st_n(notice: &str) -> State { State { notice: notice.into(), ..Default::default() } }
    #[test] fn the_notice_is_offered_once_to_an_existing_install_that_is_not_off() {
        assert!(notice_offer(&st_n(""), true, true)["token"] == NOTICE_TOKEN && notice_offer(&st_n("shown"), true, true).is_object());
        for n in ["setup", "closed", "chosen"] { assert!(notice_offer(&st_n(n), true, true).is_null(), "{n}") }
        assert!(notice_offer(&st_n(""), false, true).is_null(), "a new install: setup shows it");
        assert!(notice_offer(&st_n(""), true, false).is_null(), "a build that cannot send shows nothing");
        let off = State { share: "off".into(), ..Default::default() }; assert!(notice_offer(&off, true, true).is_null());
        let declined = State { onboard_usage: "off".into(), ..Default::default() }; assert!(notice_offer(&declined, true, true).is_null());
    }
    #[test] fn the_notice_records_shown_close_and_off() {
        let k = |cs: &[Value]| cs.iter().map(|c| format!("{}={}", c["key"].as_str().unwrap(), c["value"].as_str().unwrap())).collect::<Vec<_>>();
        assert_eq!(k(&notice_act(&st_n(""), NOTICE_TOKEN, "shown", true, 1)), ["telemetry.notice=shown"]);
        assert!(notice_act(&st_n("shown"), NOTICE_TOKEN, "shown", true, 1).is_empty(), "once");
        assert!(notice_act(&st_n(""), "other", "shown", true, 1).is_empty() && notice_act(&st_n(""), NOTICE_TOKEN, "shown", false, 1).is_empty(), "only the notice that is out");
        assert_eq!(k(&notice_act(&st_n("shown"), NOTICE_TOKEN, "close", true, 1)), ["telemetry.notice=closed"]);
        let off = k(&notice_act(&st_n(""), NOTICE_TOKEN, "off", true, 1)); assert_eq!(off[0], "telemetry.share=off"); assert_eq!(off.last().unwrap(), "telemetry.notice=closed");
        assert_eq!(k(&notice_act(&st_n(""), "", "setup", false, 1)), ["telemetry.notice=setup"]); assert!(notice_act(&st_n("shown"), "", "setup", false, 1).is_empty());
    }
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
        let (cs, r) = choice(&s, &p("on", &s.token(), "d1", false), &json!(1), 9); assert_eq!(cs.len(), 6); assert_eq!(cs[5]["key"], NOTICE); assert_eq!(cs[5]["value"], "chosen"); assert_eq!(r["ok"], true); assert_eq!(r["telemetryChoice"]["usage"], "1");
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
