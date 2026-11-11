//! Done is material (Nov 11, the person: "are you not receiving and bubbling up status updates from the mailroom?"). A launched thread that finished its turn
//! (the launcher's report "… completed") is work the person asked for coming in: it is not routine FYI for triage to keep quiet. Such a report skips the
//! triage run and joins the owner's done batch; finishes that come within `WINDOW` seconds of the first go out together. The batch is one private lane whose
//! brief asks for exactly one tell_person (one short line per item: what landed, whether it is in the running build, what to try). A lane that ends without
//! telling (it chose silence, failed, or a restart cut it) has the hub post the plain digest itself, so a finish always reaches the person once.
//! Progress (a thread's message mid-turn, a report while its work still runs) and asks keep their own paths. No message text or address is logged here.
use super::{escalate::private_told_key, route::{jshow, ref_uri, take_chars, Ctx, Step}, Body};
use crate::bots::rpc::{now_s, Bots};
use serde_json::{json, Value};
use std::sync::Arc;

/// Done.window: seconds a batch stays open after its first finish
pub const WINDOW: u64 = 60;
fn set(k: String, v: &str, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
pub fn batch_key(bot: &str) -> String { format!("mail.private.done.batch.{bot}") }
pub fn due_key(bot: &str) -> String { format!("mail.private.done.due.{bot}") }
fn item_key(input: &str) -> String { format!("mail.private.done.item.{input}") }
/// the plain digest a done lane owes, kept until it tells (`said`) or the hub posts it
pub fn lane_key(lane: &str) -> String { format!("mail.private.done.lane.{lane}") }
fn said_key(lane: &str) -> String { format!("mail.private.said.{lane}") }
fn handed_key(lane: &str) -> String { format!("mail.private.handed.{lane}") }

/// Done.report: the launcher's report of a launched thread whose turn completed (`launch::note` with `word` "completed"): (thread, title).
/// "waits for an answer", "failed", "was interrupted" and "needs the person" are not finishes.
pub fn done_report(text: &str) -> Option<(String, String)> {
    let first = text.lines().next()?;
    let rest = first.strip_prefix("[Backplane] Launched thread \"")?;
    let end = rest.rfind(") completed")?; if !matches!(&rest[end..], ") completed" | ") completed:" | ") completed.") { return None }
    let open = rest[..end].rfind(" (")?; let title = rest[..open].strip_suffix('"')?; let thread = &rest[open + 2..end];
    (!thread.is_empty()).then(|| (thread.to_string(), title.to_string()))
}
/// Done.summary: the reply's first line of substance, one line, at most 200 characters
fn summary(text: &str) -> String {
    let line = text.lines().skip(1).map(|l| l.trim().trim_start_matches(['#', '*', '-', '>', ' '])).find(|l| !l.is_empty()).unwrap_or("");
    let s: String = line.chars().map(|c| if c == '\t' { ' ' } else { c }).collect();
    if s.chars().count() > 200 { format!("{}…", take_chars(&s, 199)) } else { s }
}
/// Done.items: the owner's open batch
fn items(v: &str) -> Vec<Value> { serde_json::from_str::<Value>(v).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default() }
/// Done.line: one item as the plain digest has it
fn line(i: &Value) -> String {
    let (title, sum, th) = (i["title"].as_str().unwrap_or(""), i["summary"].as_str().unwrap_or(""), i["thread"].as_str().unwrap_or(""));
    let title = if title.is_empty() { th } else { title };
    format!("{title}{} ({})", if sum.is_empty() { " finished".to_string() } else { format!(": {sum}") }, ref_uri("thread", th))
}
/// Done.digest: the hub's own words when the lane said nothing: one line for one finish, a line each for several
pub fn digest(xs: &[Value]) -> String {
    match xs { [one] => format!("Done: {}", line(one)), _ => format!("Done:\n{}", xs.iter().map(|i| format!("- {}", line(i))).collect::<Vec<_>>().join("\n")) }
}
/// The build the person runs: the hub's own build folder (`<date>-<time>-<commit>`), "" when it is not one
pub fn running_build() -> String {
    std::env::current_exe().ok().and_then(|p| p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string())).filter(|n| n.matches('-').count() >= 2).unwrap_or_default()
}
const DONE_HEAD: &str = "[Private mailroom: finished work. Done is material: each item below is a launched thread that finished work the person asked for (or landed it on main). The person's rule (pref.report-completions) is to hear it right away, so you MUST call tell_person exactly once, with the key given below, before you end. Write ONE message, one short plain line per item: what landed or finished, whether it is live in their running build (or needs a hub restart, a rebuild or a merge first), and what to try. No preamble, no raw report text. Leave out an item only when its thread_read shows it is plainly intermediate progress and not a finish; if every item is, still call tell_person with a single line saying where they stand. Asks and approvals stay at their source. If you end without telling, the hub posts a plain digest of these items for you.]\n";

impl<'a> Ctx<'a> {
    /// Done.admit (Mail.route, the owner's inbound launch report of a finish): the report joins the batch instead of a triage run; its ticket is claimed so the
    /// outbox walk does not route it again
    pub fn done_admit(&mut self, body: &Body, bot: &str) -> Option<Step> {
        let m = &body.mail; if m.kind != "launch" { return None }
        if !self.setting(&item_key(&m.id), "").is_empty() { return Some(Step::none()) }
        let (thread, title) = done_report(&body.text).or_else(|| done_report(&m.head))?;
        let mut xs = items(&self.setting(&batch_key(bot), "[]"));
        let title = if title.is_empty() { self.th(&thread).title } else { title };
        xs.push(json!({"input":m.id,"conv":m.conv,"thread":thread,"title":title,"summary":summary(&body.text),"at":self.now}));
        let mut cs = self.owner_claim(&m.id);
        cs.push(set(item_key(&m.id), "batched", self.now)); cs.push(set(batch_key(bot), &jshow(json!(xs)), self.now));
        if self.setting(&due_key(bot), "").is_empty() { cs.push(set(due_key(bot), &(self.now + WINDOW).to_string(), self.now)) }
        self.replay(&cs); Some(Step { cs, outs: vec![] })
    }
    /// Done.flush: a batch whose window is over becomes one private lane (Private.spawn) that owes one tell_person
    pub fn done_flush(&mut self, bot: &str) -> Step {
        let due: u64 = self.setting(&due_key(bot), "").parse().unwrap_or(0);
        if due == 0 || self.now < due { return Step::none() }
        let xs = items(&self.setting(&batch_key(bot), "[]"));
        let mut cs = vec![set(batch_key(bot), "[]", self.now), set(due_key(bot), "", self.now)];
        if xs.is_empty() || self.w.triage_thread().is_empty() { self.replay(&cs); return Step { cs, outs: vec![] } }
        let id = self.id("done"); let key = format!("done:{id}");
        let build = running_build();
        let list: String = xs.iter().map(|i| format!("- {} · input {}\n", line(i), i["input"].as_str().unwrap_or(""))).collect();
        let note = format!("{DONE_HEAD}tell_person key: {key}\nRunning hub build: {}\n\nFinished ({}):\n{list}", if build.is_empty() { "unknown (check with the person's build folder)".to_string() } else { build }, xs.len());
        for i in &xs { cs.push(set(item_key(i["input"].as_str().unwrap_or("")), &key, self.now)) }
        self.replay(&cs);
        let spawn = self.private_spawn(bot, &format!("done-batch:{id}"), &note);
        let lane = spawn.cs.iter().find(|e| e["$"] == "ThreadCreated").and_then(|e| e["id"].as_str()).unwrap_or("").to_string();
        let mut own = vec![set(lane_key(&lane), &jshow(json!({"key":key,"text":digest(&xs)})), self.now)]; self.replay(&own);
        cs.append(&mut own);
        Step { cs, outs: vec![] }.cat(spawn)
    }
    /// Done.lane.end: a done lane that ended (any end) without telling has the hub post its digest, once; Private.lost's hand-over is not owed then
    pub fn done_lane_end(&mut self, lane: &str, state: &str) -> Option<Step> {
        let owed = self.setting(&lane_key(lane), ""); if owed.is_empty() { return None }
        if !["completed", "failed", "interrupted"].contains(&state) { return Some(Step::none()) }
        if !self.setting(&said_key(lane), "").is_empty() || !self.setting(&handed_key(lane), "").is_empty() { return Some(Step::none()) }
        let v: Value = serde_json::from_str(&owed).unwrap_or(Value::Null); let (key, text) = (v["key"].as_str().unwrap_or("").to_string(), v["text"].as_str().unwrap_or("").to_string());
        let cs = vec![set(handed_key(lane), &self.now.to_string(), self.now)]; self.replay(&cs);
        let bot = self.w.private_owner(lane);
        if key.is_empty() || text.is_empty() || !self.setting(&private_told_key(&bot, &key), "").is_empty() { return Some(Step { cs, outs: vec![] }) }
        let told = self.private_tell(lane, &text, &key); Some(Step { cs, outs: vec![] }.cat(told))
    }
    /// the owners with a done batch whose window is over
    pub fn done_due(&self) -> Vec<String> {
        let ub = self.w.uber_bot(); let due: u64 = self.setting(&due_key(&ub), "").parse().unwrap_or(0);
        if ub.is_empty() || due == 0 || self.now < due { vec![] } else { vec![ub] }
    }
}

impl Bots {
    /// Done.tick: the minute tick, and the timer a batch's first finish sets, flush what is due
    pub async fn done_tick(self: &Arc<Self>) {
        if crate::copy::on() { return }
        let outs = self.decide(&[], |c| { let mut st = Step::none(); for b in c.done_due() { let s = c.done_flush(&b); st = st.cat(s) } st }).await;
        self.perform(outs).await
    }
    /// Done.timer: once a batch opens, a flush when its window is over (the minute tick covers a restart in between)
    pub(super) fn done_timer(self: &Arc<Self>, due: u64) {
        if crate::copy::on() || due == 0 { return }
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(due.saturating_sub(now_s()) + 1)).await;
            if let Some(b) = me.upgrade() { b.done_tick().await }
        });
    }
}

#[cfg(test)]
mod laws {
    use super::*;
    #[test] fn a_completed_report_is_a_finish_and_the_others_are_not() {
        let r = crate::agents::launch::note("t1-2", "Fix scroll (v2)", "completed", "Landed abc123 on main.\nMore.");
        assert_eq!(done_report(&r), Some(("t1-2".into(), "Fix scroll (v2)".into())));
        assert_eq!(summary(&r), "Landed abc123 on main.");
        assert_eq!(done_report(&crate::agents::launch::note("t1", "x", "completed", "")), Some(("t1".into(), "x".into())));
        for w in ["failed", "was interrupted", "finished its turn and waits for an answer"] { assert_eq!(done_report(&crate::agents::launch::note("t1", "x", w, "r")), None, "{w}") }
        assert_eq!(done_report(&crate::agents::launch::ask_note("t1", "x", "Bash: rm")), None);
        assert_eq!(done_report("progress: still building"), None);
    }
    #[test] fn a_tell_always_shows_the_old_ids_too() {
        use crate::present::thread::{backend_replies, told_row};
        assert!(told_row("tell:1794-1") && told_row("tell1794403844-694027") && !told_row("teller") && !told_row("a12"));
        let hid = backend_replies([("n1", true, "user", "[Backplane] note"), ("tell1794403844-694027", true, "assistant", "Done: x"), ("a1", true, "assistant", "private")], true);
        assert!(!hid.contains("tell1794403844-694027") && hid.contains("a1"), "{hid:?}");
    }
    #[test] fn the_digest_is_one_line_for_one_and_a_line_each_for_more() {
        let a = json!({"thread":"t1","title":"A","summary":"landed"}); let b = json!({"thread":"t2","title":"B","summary":""});
        assert_eq!(digest(&[a.clone()]), "Done: A: landed (backplane://thread/t1)");
        assert_eq!(digest(&[a, b]), "Done:\n- A: landed (backplane://thread/t1)\n- B finished (backplane://thread/t2)");
    }
}
