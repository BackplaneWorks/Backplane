//! The tell gate (Nov 20, the person: "I'm getting 30 billion KiCad messages, please fix that"; pref.fewer-progress-notes). Busy lead threads end a turn every few
//! minutes, and each end reached the person as a tell_person saying "nothing landed, nothing to try, still waiting on …". The person hears only what is material:
//! a landing on main or a release they can try (`landed`, `release`), or a decision, approval or blocker that needs them (`needs_you`). This is decided here, by
//! the hub, not only by the lanes' briefs:
//! - `refusal`: a tell whose words amount to no change ("nothing landed", "nothing to try", "still testing", "queued", "rebased", …), or whose words do not
//!   back its kind, is refused; the lane is told why and nothing is posted.
//! - `quiet_report`: a launched thread's turn-end report that opens with no-change words never becomes a done lane at all.
//! - Batch: material tells are collected per owner and go out as ONE message at most every `BATCH` seconds; a `needs_you` tell goes at once (with whatever is
//!   collected), but never sooner than `GAP` seconds after the last one.
//! No message text or address is logged here.
use super::{escalate::private_told_key, route::{jshow, Ctx, Step}};
use serde_json::{json, Value};

/// Gate.batch: seconds material tells are collected before they go out, and the least time between two collected messages
pub const BATCH: u64 = 1800;
/// Gate.gap: the least time between any two tells, a `needs_you` one included
pub const GAP: u64 = 60;
fn set(k: String, v: &str, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
pub fn batch_key(bot: &str) -> String { format!("mail.private.tell.batch.{bot}") }
pub fn due_key(bot: &str) -> String { format!("mail.private.tell.due.{bot}") }
pub fn last_key(bot: &str) -> String { format!("mail.private.tell.last.{bot}") }
fn said_key(lane: &str) -> String { format!("mail.private.said.{lane}") }

/// Words that say nothing happened the person can use. Lowercase, straight apostrophes.
const NO_CHANGE: &[&str] = &[
    "nothing landed", "nothing has landed", "nothing new", "nothing to try", "nothing's been", "nothing has been released", "nothing is released", "nothing released",
    "nothing changed", "nothing has changed", "nothing was merged", "nothing merged", "nothing yet", "no change", "unchanged", "no new results", "no results yet",
    "no news", "no update", "still waiting", "still testing", "still running", "still nothing", "still isn't", "still hasn't", "still not ", "still in progress",
    "still being", "still on hold", "not on main yet", "isn't on main yet", "not landed yet", "isn't landed", "not landed", "hasn't landed", "has not landed",
    "isn't merged", "not merged", "hasn't merged", "queued", "rebased", "rebasing", "waiting its turn", "waiting on its", "is landing now", "landing now",
    "in review", "being reviewed", "under review", "awaiting review", "prep step only", "groundwork only",
];
/// Words a landing or release tell must have
const LANDED: &[&str] = &["landed", "merged", "on main", "released", "release", "in your build", "in your running build", "shipped", "published", "is live"];
/// Words a tell that needs the person must have
const NEEDS: &[&str] = &["?", "your call", "decide", "decision", "approve", "approval", "need you", "needs you", "needs your", "need your", "blocked", "blocker",
    "your answer", "confirm", "choose", "pick ", "your ok", "sign in", "log in", "password", "permission", "waiting on you"];
/// Words that a turn-end report asks the person something (narrower than NEEDS: a lead's "passed my decision to the lanes" asks nothing)
const ASKS: &[&str] = &["?", "needs your", "need your", "needs you", "need you", "your answer", "your call", "your approval", "your ok", "waiting on you", "yours to make"];

fn norm(t: &str) -> String { t.to_lowercase().replace(['\u{2019}', '\u{2018}'], "'") }
fn has(t: &str, ws: &[&str]) -> Option<String> { ws.iter().find(|w| t.contains(**w)).map(|w| w.trim().to_string()) }

/// Gate.kind: what a tell is: a landing on main, a release to try, or something that needs the person now
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind { Landed, Release, NeedsYou }
impl Kind {
    pub fn parse(s: &str) -> Option<Kind> { match s.trim() { "landed" => Some(Kind::Landed), "release" => Some(Kind::Release), "needs_you" => Some(Kind::NeedsYou), _ => None } }
    pub fn urgent(self) -> bool { self == Kind::NeedsYou }
}

/// Gate.refusal: why a tell is not material (None: it may go out)
pub fn refusal(kind: &str, text: &str) -> Option<String> {
    let Some(k) = Kind::parse(kind) else { return Some("kind must be landed (merged to main), release (a build or release the person can try now) or needs_you (a decision, approval or blocker only the person can resolve). Anything else is not told.".into()) };
    let t = norm(text);
    if let Some(w) = has(&t, NO_CHANGE) { return Some(format!("refused: \"{w}\" is a no-change status. The person hears only landings, releases they can try, and what needs them; progress, waiting, review, queues and rebases stay private. Drop the status words and tell only the material item, or end silently.")) }
    match k {
        Kind::Landed | Kind::Release if has(&t, LANDED).is_none() => Some("refused: a landed or release tell must say what landed on main or what was released for the person to try.".into()),
        Kind::NeedsYou if has(&t, NEEDS).is_none() => Some("refused: a needs_you tell must name the decision, approval or blocker and what the person should answer.".into()),
        _ => None,
    }
}
/// Gate.quiet: a launched thread's turn-end report whose first line says no change, and asks nothing of the person, is no finish to tell
pub fn quiet_report(summary: &str) -> bool { let t = norm(summary); has(&t, NO_CHANGE).is_some() && has(&t, ASKS).is_none() }

/// Gate.items: an owner's collected tells
fn items(v: &str) -> Vec<Value> { serde_json::from_str::<Value>(v).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default() }
fn num(s: String) -> u64 { s.parse().unwrap_or(0) }

impl<'a> Ctx<'a> {
    /// Gate.lines: the owner's reporting preferences, quoted back with a refusal so the lane sees the rule it broke
    fn report_prefs(&self, bot: &str) -> String {
        let mut ps: Vec<_> = self.w.mems.values().filter(|m| m.bot == bot && (m.kind == "pref" || m.key.starts_with("pref.")))
            .filter(|m| { let h = norm(&format!("{} {}", m.key, m.text)); ["notif", "progress", "report", "tell", "quiet", "fewer"].iter().any(|w| h.contains(w)) }).collect();
        ps.sort_by(|a, b| b.updated.cmp(&a.updated));
        ps.iter().take(3).map(|m| format!("\n- {}: {}", m.key, super::route::take_chars(&m.text, 400))).collect()
    }
    /// Gate.tell (tell_person, Done.lane.end): a material tell joins the owner's batch; Err is the refusal, nothing posted. Ok carries when the batch goes out (0: no
    /// change, a repeat of a told key).
    pub fn tell_gate(&mut self, lane: &str, text: &str, key: &str, kind: &str) -> Result<(Step, u64), String> {
        let bot = self.w.private_owner(lane);
        if let Some(why) = refusal(kind, text) { let p = self.report_prefs(&bot); return Err(if p.is_empty() { why } else { format!("{why}\nThe person's rules:{p}") }) }
        if !self.setting(&private_told_key(&bot, key), "").is_empty() { return Ok((Step::none(), 0)) }
        let urgent = Kind::parse(kind).is_some_and(Kind::urgent); let now = self.now;
        let (last, open) = (num(self.setting(&last_key(&bot), "")), num(self.setting(&due_key(&bot), "")));
        let due = if urgent { let at = now.max(last + GAP); if open > 0 { open.min(at) } else { at } } else if open > 0 { open } else { (now + BATCH).max(last + BATCH) };
        let mut xs = items(&self.setting(&batch_key(&bot), "[]"));
        xs.push(json!({"key":key,"text":text,"lane":lane,"kind":kind,"at":now}));
        let cs = vec![set(batch_key(&bot), &jshow(json!(xs)), now), set(due_key(&bot), &due.to_string(), now),
            set(private_told_key(&bot, key), &format!("queued:{due}"), now), set(said_key(lane), &format!("queued:{key}"), now)];
        self.replay(&cs);
        let st = Step { cs, outs: vec![] };
        if due <= now { let f = self.tell_flush(&bot); return Ok((st.cat(f), due)) }
        Ok((st, due))
    }
    /// Gate.flush: a batch whose time has come is ONE message in the person's chat
    pub fn tell_flush(&mut self, bot: &str) -> Step {
        let due = num(self.setting(&due_key(bot), "")); if due == 0 || self.now < due { return Step::none() }
        let xs = items(&self.setting(&batch_key(bot), "[]"));
        let mut cs = vec![set(batch_key(bot), "[]", self.now), set(due_key(bot), "", self.now)];
        let Some(lane) = xs.last().and_then(|i| i["lane"].as_str()).map(str::to_string) else { self.replay(&cs); return Step { cs, outs: vec![] } };
        cs.push(set(last_key(bot), &self.now.to_string(), self.now)); self.replay(&cs);
        let t = |i: &Value| i["text"].as_str().unwrap_or("").trim().to_string();
        // the urgent ones first
        let mut xs = xs; xs.sort_by_key(|i| i["kind"] != "needs_you");
        let text = match xs.as_slice() { [one] => t(one), _ => xs.iter().map(|i| format!("- {}", t(i).replace('\n', " "))).collect::<Vec<_>>().join("\n") };
        let key = format!("tell-batch:{}", self.id("tb"));
        let told = self.private_tell(&lane, &text, &key);
        Step { cs, outs: vec![] }.cat(told)
    }
    /// the owner with a tell batch whose time has come
    pub fn tell_due(&self) -> Vec<String> {
        let ub = self.w.uber_bot(); let due = num(self.setting(&due_key(&ub), ""));
        if ub.is_empty() || due == 0 || self.now < due { vec![] } else { vec![ub] }
    }
}

#[cfg(test)]
mod laws {
    use super::*;
    #[test] fn the_no_change_notes_from_the_flood_are_refused() {
        for t in ["Nothing landed, and your build (20261119-205245) is unchanged. The keyboard fix for PR #1361 is pushed.",
            "KiCad: nothing new. PR #1361 is still waiting for its one test rerun.",
            "KiCad: still nothing to try. The macOS first-click fix is written.",
            "Viewer rail: the docking layout model landed on main. It's hub-side groundwork only, so there's nothing to try in your build yet.",
            "The KiCad Follow-after-reconnect fix PR #1410 passed review and is landing now, but it isn't on main yet.",
            "KiCad: the Mac mini now has room for bp.13.3's macOS tests. That's a prep step only: nothing is released.",
            "PR #1400 is rebased and queued with the Integrator.", "Still testing on bp-win."] {
            for k in ["landed", "release", "needs_you"] { assert!(refusal(k, t).is_some(), "{k}: {t}") }
        }
    }
    #[test] fn landings_releases_and_asks_go_through_and_need_their_words() {
        assert_eq!(refusal("landed", "KiCad: PR #1361 has landed on main (`5f3811dc45`). Your running build doesn't have it yet; it'll be in the next one."), None);
        assert_eq!(refusal("release", "KiCad bp.13.3 is released: restart Backplane and open a board to try it."), None);
        assert_eq!(refusal("needs_you", "There is a macOS privacy prompt on the Mac mini that needs your answer: allow Documents access?"), None);
        assert!(refusal("landed", "PR #1418 needs one fix before a verdict.").is_some(), "no landing");
        assert!(refusal("needs_you", "Carbon measured the Windows focus pause.").is_some(), "no ask");
        assert!(refusal("", "PR #1 landed on main.").is_some() && refusal("progress", "PR #1 landed on main.").is_some(), "a kind is required");
    }
    #[test] fn a_quiet_turn_end_is_no_finish_but_an_ask_is() {
        assert!(quiet_report("No new results: Integrator has passed my decision to the lanes."));
        assert!(quiet_report("The Follow lane's fix build is written, compiled for Windows, and queued with Carbon. No results yet."));
        assert!(!quiet_report("Landed 1a2b3c4 on main: KiCad opens inside the Board tab."));
        assert!(!quiet_report("Nothing landed, but the bp.13.x licence decision needs your answer."));
    }
}
