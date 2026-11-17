//! The 0.14 telemetry choice across the import (docs/IMPORT-0.14.md, docs/telemetry.md). An explicit Off stays Off: no notice, nothing sent.
//!
//! 0.14 (the Bend hub, `<user folder>/.backplane-bend`) kept the choice in its log as settings: an Off is `telemetry.share` "off",
//! `telemetry.asked` "1", `telemetry.usage.asked` "0" with `telemetry.choice.revision` and `telemetry.choice.receipts` (src/core/hub.bend
//! `Hub.telemetry.choice.changes`); an older Off is `telemetry.share` "off" or `telemetry.usage.asked` "0" alone. 0.15 reads the same keys
//! (settings/telemetry.rs), so the import brings them as they are (plan.rs), first and as one, unless this home made its own choice.
//!
//! 0.15.x and 0.16.0 imported without them: a 0.14 Off came across as the default On, with the one-time notice. `settle` mends such a home once
//! at start, before anything is sent or the notice is offered: when it was imported (`import.v014.done`), the 0.14 home it came from is still
//! there and its log says Off, and nothing here is a choice of the person's since (an On in Settings, the notice's or setup's), it records the
//! same Off as Settings' Off. When the old home is gone or unreadable it cannot know, and the home is left as it is.
use crate::hub::{now_ms, Hub};
use crate::settings::telemetry as tc;
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, sync::Arc};

/// The settings that are the telemetry choice, in the order 0.14 wrote them (and `onboard.usage`, setup's answer, for a home that has one).
pub const CHOICE: &[&str] = &["telemetry.share", "telemetry.asked", "telemetry.usage.asked", "telemetry.choice.revision", "telemetry.choice.receipts", "onboard.usage"];
/// This home has a choice of its own (keys with a value): then none of 0.14's comes across.
pub fn chose_here(have: &HashSet<String>) -> bool { ["telemetry.share", "telemetry.usage.asked", "onboard.usage"].iter().any(|k| have.contains(*k)) }
/// Records that the mend was decided for this home: "off" (0.14's Off restored) or "kept" (0.14 was not Off). Never written when it could not know.
pub const MENDED: &str = "import.v014.telemetry";

/// The last value of each choice setting in a 0.14 log (whole lines only).
pub fn old_choice(log: &Path) -> std::io::Result<HashMap<String, String>> {
    let upto = super::plan::whole_size(log)?; let mut m = HashMap::new();
    super::plan::each_line(log, upto, |l| {
        if crate::session::event_type(l) != Some("SettingSet") || !l.contains("\"key\":\"telemetry.") && !l.contains("\"key\":\"onboard.usage\"") { return }
        if let Ok(v) = serde_json::from_str::<Value>(l) { let k = v["key"].as_str().unwrap_or(""); if CHOICE.contains(&k) { m.insert(k.to_string(), v["value"].as_str().unwrap_or("").to_string()); } }
    })?;
    Ok(m)
}
/// That choice is Off.
pub fn old_off(c: &HashMap<String, String>) -> bool { let g = |k: &str| c.get(k).map(String::as_str).unwrap_or(""); tc::off(g("telemetry.share"), g("telemetry.usage.asked"), g("onboard.usage")) }
/// The person turned it on here since: Settings' or the notice's On ("chosen" marks one made before the notice), or setup's.
pub fn chose_on_here(s: &tc::State) -> bool { s.share == "on" || s.onboard_usage == "on" || s.notice == "chosen" }
/// Whether this home may need the mend: imported, not yet decided, not Off already, no On of the person's own.
pub fn may_need(done: &str, mended: &str, s: &tc::State) -> bool { !done.is_empty() && mended.is_empty() && !tc::off(&s.share, &s.usage_asked, &s.onboard_usage) && !chose_on_here(s) }
/// The 0.14 home an import came from (its recorded result), when it is still a 0.14 home.
fn source_of(done: &str, home: &Path) -> Option<PathBuf> {
    let d: Value = serde_json::from_str(done).ok()?; let f = PathBuf::from(d["from"].as_str().filter(|f| !f.is_empty())?);
    super::is_old_home(&f, home).then_some(f)
}

/// The mend, once: what it decided ("off", "kept"), or "" (nothing to do, or it cannot know).
pub async fn mend(hub: &Arc<Hub>) -> &'static str {
    let (done, mended) = (hub.setting(super::DONE).await.unwrap_or_default(), hub.setting(MENDED).await.unwrap_or_default());
    if !may_need(&done, &mended, &tc::state(hub).await) { return "" }
    let Some(old) = source_of(&done, &hub.cfg.home) else { return "" };
    let Ok(Ok(c)) = tokio::task::spawn_blocking(move || old_choice(&old.join("events.jsonl"))).await else { return "" };
    // read again: a choice made while the old log was read wins
    let s = tc::state(hub).await;
    if !may_need(&done, &hub.setting(MENDED).await.unwrap_or_default(), &s) { return "" }
    let at = now_ms() / 1000;
    let what = if old_off(&c) { "off" } else { "kept" };
    let mut cs = if what == "off" { tc::changes(&s, "off", &tc::fingerprint("", &s.token(), "import014-off", false), at) } else { vec![] };
    cs.push(json!({"$":"SettingSet","key":MENDED,"value":what,"at":at}));
    if let Err(e) = hub.commit_many(&cs).await { eprintln!("import: the 0.14 telemetry choice could not be restored: {e:#}"); return "" }
    if what == "off" { eprintln!("import: Backplane 0.14 had anonymous usage Off; it is Off here again"); // (not tc::changed: its offer waits for this settling) Off has no notice
        hub.push_all(&json!({"t":"telemetry.notice","notice":Value::Null})); crate::settings::telemetry_send::wake(); }
    what
}

/// One settling per home: the sender and the notice wait for it.
static ONCE: std::sync::Mutex<Vec<(PathBuf, Arc<tokio::sync::OnceCell<()>>)>> = std::sync::Mutex::new(Vec::new());
/// Waits until this home's telemetry choice is settled (the mend has run once in this process).
pub async fn settle(hub: &Arc<Hub>) {
    let cell = { let mut l = ONCE.lock().unwrap();
        match l.iter().find(|(h, _)| h == &hub.cfg.home) { Some((_, c)) => c.clone(), None => { let c = Arc::new(tokio::sync::OnceCell::new()); l.push((hub.cfg.home.clone(), c.clone())); c } } };
    cell.get_or_init(|| async { mend(hub).await; }).await;
}
