//! Done is material (Nov 11, the person: "are you not receiving and bubbling up status updates from the mailroom?"). A launched thread that finished its turn
//! (the launcher's report "… completed") is work the person asked for coming in: it is not routine FYI for triage to keep quiet. Such a report skips the
//! triage run and joins the owner's done batch; finishes that come within `WINDOW` seconds of the first go out together. The batch is one private lane whose
//! brief asks for at most one tell_person, and only for what is material (gate.rs: a landing, a release, a completed non-PR deliverable, or an ask; Nov 20, pref.fewer-progress-notes). A lane
//! that ends without telling has told nothing: the hub's plain digest names no verified ref, so it is recorded and never posted. A turn-end
//! report that opens with no-change words makes no lane at all.
//! Progress (a thread's message mid-turn, a report while its work still runs) and asks keep their own paths. No message text or address is logged here.
use super::{escalate::private_told_key, route::{jshow, ref_uri, take_chars, Ctx, Step}, Body};
use crate::bots::rpc::{now_s, Bots};
use serde_json::{json, Value};
use std::sync::Arc;

/// Done.window: seconds a batch stays open after its first finish
pub const WINDOW: u64 = 60;
/// Done.fallback: seconds after a finish is admitted (or a lane starts) before an untold completion gets a plain fallback tell
pub const FALLBACK_AFTER: u64 = 120;
/// Historical `triage-end:` mail older than this (from un-claimed backlog) is claimed and dropped without telling
pub const TRIAGE_END_STALE_SECS: u64 = 7200;
/// Cooldown between repeated `triage-end:` completion tells for the same thread (seconds).
pub const COOLDOWN: u64 = 1800;
fn set(k: String, v: &str, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
pub fn batch_key(bot: &str) -> String { format!("mail.private.done.batch.{bot}") }
pub fn due_key(bot: &str) -> String { format!("mail.private.done.due.{bot}") }
pub fn pending_key(bot: &str) -> String { format!("mail.private.done.pending.{bot}") }
pub fn told_at_key(bot: &str, thread: &str) -> String { format!("mail.private.done.told.{bot}.{thread}") }
pub fn fallback_at_key(bot: &str, thread: &str) -> String { format!("mail.private.done.fallback.{bot}.{thread}") }
fn item_key(input: &str) -> String { format!("mail.private.done.item.{input}") }
/// the plain digest a done lane owes, kept until it tells (`said`) or the hub posts it
pub fn lane_key(lane: &str) -> String { format!("mail.private.done.lane.{lane}") }
fn said_key(lane: &str) -> String { format!("mail.private.said.{lane}") }
fn handed_key(lane: &str) -> String { format!("mail.private.handed.{lane}") }

/// Extract `<thread>` from `triage-end:<thread>:<ts>` or an imported `owner:[uid,origin,"triage-end:<thread>:<ts>"]`.
pub fn triage_end_thread_of_id(mail_id: &str) -> Option<String> {
    let raw = if let Some(r) = mail_id.strip_prefix("triage-end:") {
        r.to_string()
    } else if let Some(arr) = mail_id.strip_prefix("owner:") {
        let v: Vec<String> = serde_json::from_str(arr).ok()?;
        v.get(2).and_then(|s| s.strip_prefix("triage-end:"))?.to_string()
    } else {
        return None;
    };
    let th = raw.rsplit_once(':').map(|(t, _)| t).unwrap_or(&raw);
    (!th.is_empty()).then(|| th.to_string())
}
/// Extract `<thread>` from a `triage-end:` mail id (local or owner-imported).
pub fn triage_end_thread(mail_id: &str) -> Option<String> { triage_end_thread_of_id(mail_id) }
/// Parse a `triage-end:` completed report: returns `(thread, title)` only when the status is `completed`.
fn triage_end_completed(m: &super::Hdr, text: &str) -> Option<(String, String)> {
    let extracted = triage_end_thread(&m.id)?;
    let thread = if m.id.starts_with("owner:") {
        m.from.strip_prefix("thread:").filter(|t| !t.is_empty()).map(str::to_string).unwrap_or(extracted)
    } else {
        extracted
    };
    let first = text.lines().next().filter(|l| !l.is_empty()).unwrap_or(&m.head);
    let rest = first.strip_prefix("Thread completed:")?.trim();
    let title = if rest.starts_with("backplane://") {
        String::new()
    } else if let Some(i) = rest.rfind(" (backplane://") {
        rest[..i].trim().to_string()
    } else {
        rest.to_string()
    };
    Some((thread, title))
}
fn contains_id(text: &str, id: &str) -> bool {
    if id.is_empty() { return false }
    let is_id_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '~');
    for (idx, _) in text.match_indices(id) {
        let before_ok = text[..idx].chars().next_back().map_or(true, |c| !is_id_char(c));
        let after_ok = text[idx + id.len()..].chars().next().map_or(true, |c| !is_id_char(c));
        if before_ok && after_ok { return true }
    }
    false
}
/// Extract up to `max_lines` non-empty cleaned lines, truncated to `max_chars`.
pub fn first_lines(text: &str, max_lines: usize, max_chars: usize) -> String {
    if max_lines == 0 || max_chars == 0 { return String::new() }
    let mut lines: Vec<String> = vec![];
    for raw in text.lines() {
        let cleaned = raw.trim().trim_start_matches(['#', '*', '-', '>', ' ']).trim().replace('\t', " ");
        if !cleaned.is_empty() {
            lines.push(cleaned);
            if lines.len() >= max_lines { break }
        }
    }
    let joined = lines.join("\n");
    if joined.chars().count() > max_chars {
        format!("{}…", take_chars(&joined, max_chars.saturating_sub(1)))
    } else {
        joined
    }
}
/// Plain fallback tell for a completion that went untold past `FALLBACK_AFTER`: title, link, and first lines of reply.
pub fn fallback_text(title: &str, reply: &str, thread: &str) -> String {
    let title = if title.trim().is_empty() { thread } else { title.trim() };
    let link = ref_uri("thread", thread);
    let preview = first_lines(reply, 3, 300);
    if preview.is_empty() {
        format!("Finished: {title} ({link})")
    } else {
        format!("Finished: {title} ({link})\n{preview}")
    }
}

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
const DONE_HEAD: &str = "[Private mailroom: threads ended a turn. Each item below is a thread (launched or user-requested) that ended a turn; most recurring-lead turn-ends are progress, not news. The person's rules in <owner_preferences> (pref.fewer-progress-notes among them) rule here: speak only when something landed on main, was released for them to try, a non-PR deliverable/user-requested thread actually finished (local firmware build, CAD/PDF render, hardware BOM, local analysis/task with no PR/commit/release ref), or something needs their decision, approval or answer. Never use completed for intermediate progress turns of recurring software leads. For each item, thread_read it if unsure. If one is material, call tell_person ONCE with the key given below, its kind (landed, release, completed or needs_you) and its ref (owner/repo#N merged, commit:owner/repo@sha, release:owner/repo@tag published, thread:<id> local or <link>~<id> completed, ask:<id> open: the hub verifies it and refuses a tell without one): one short plain line per material item, what landed and whether it is in their running build, what deliverable/task finished, or exactly what they must decide. If nothing is material, end silently with no tell: that is the right outcome. Never tell \"nothing landed\", \"nothing to try\", \"still waiting\", \"in review\", \"queued\" or \"rebased\": the hub refuses it. Landings, releases and completed deliverables are collected and reach the person at most every 30 minutes; needs_you goes at once.]\n";

impl<'a> Ctx<'a> {
    /// Done.admit (Mail.route, the owner's inbound launch or `triage-end:` completion): the report joins the batch instead of a triage run;
    /// its ticket is claimed so the outbox walk does not route it again, and repeated completions of the same thread are deduplicated.
    pub fn done_admit(&mut self, body: &Body, bot: &str) -> Option<Step> {
        let m = &body.mail;
        let (thread, title, source) = if m.kind == "launch" {
            let (th, ti) = done_report(&body.text).or_else(|| done_report(&m.head))?;
            (th, ti, "launch")
        } else if triage_end_thread(&m.id).is_some() {
            let Some((th, ti)) = triage_end_completed(m, &body.text) else {
                let mut cs = self.owner_claim(&m.id);
                if self.setting(&item_key(&m.id), "").is_empty() { cs.push(set(item_key(&m.id), "ignored", self.now)); }
                self.replay(&cs);
                return Some(Step { cs, outs: vec![] });
            };
            if m.at >= 1_700_000_000 && self.now.saturating_sub(m.at) > TRIAGE_END_STALE_SECS {
                let mut cs = self.owner_claim(&m.id);
                if self.setting(&item_key(&m.id), "").is_empty() { cs.push(set(item_key(&m.id), "stale", self.now)); }
                self.replay(&cs);
                return Some(Step { cs, outs: vec![] });
            }
            (th, ti, "triage-end")
        } else {
            return None;
        };
        if !self.setting(&item_key(&m.id), "").is_empty() {
            let cs = self.owner_claim(&m.id);
            self.replay(&cs);
            return Some(Step { cs, outs: vec![] });
        }
        // Gate.quiet: a launched thread's turn-end that opens with no-change words and asks nothing is no finish: no lane, nothing told
        if source == "launch" && super::gate::quiet_report(&summary(&body.text)) {
            let mut cs = self.owner_claim(&m.id); cs.push(set(item_key(&m.id), "quiet", self.now)); self.replay(&cs); return Some(Step { cs, outs: vec![] })
        }
        let title = if title.is_empty() { self.th(&thread).title } else { title };
        let body_reply = body.text.split_once('\n').map(|(_, r)| r).unwrap_or("").trim();
        let reply = if !body_reply.is_empty() {
            body_reply
        } else {
            self.f.last.get(&thread).map(|l| l.0.as_str()).unwrap_or("")
        };
        let sum = first_lines(reply, 1, 200);
        let preview = first_lines(reply, 3, 300);

        let mut xs = items(&self.setting(&batch_key(bot), "[]"));
        let mut pend = items(&self.setting(&pending_key(bot), "[]"));
        if let Some(existing) = xs.iter_mut().find(|i| i["thread"] == thread.as_str()) {
            if let Some(o) = existing.as_object_mut() {
                if !title.is_empty() { o.insert("title".into(), json!(title)); }
                if !sum.is_empty() { o.insert("summary".into(), json!(sum)); }
                o.insert("at".into(), json!(self.now));
            }
            let mut pend_changed = false;
            for p in &mut pend {
                if p["thread"] == thread.as_str() {
                    if let Some(o) = p.as_object_mut() {
                        if !title.is_empty() { o.insert("title".into(), json!(title)); }
                        if !preview.is_empty() { o.insert("summary".into(), json!(preview)); }
                        pend_changed = true;
                    }
                }
            }
            let mut cs = self.owner_claim(&m.id);
            cs.push(set(item_key(&m.id), "dedup", self.now));
            cs.push(set(batch_key(bot), &jshow(json!(xs)), self.now));
            if pend_changed { cs.push(set(pending_key(bot), &jshow(json!(pend)), self.now)); }
            self.replay(&cs);
            return Some(Step { cs, outs: vec![] });
        }
        if let Some(existing_p) = pend.iter_mut().find(|i| i["thread"] == thread.as_str()) {
            let mut pend_changed = false;
            if let Some(o) = existing_p.as_object_mut() {
                if !title.is_empty() { o.insert("title".into(), json!(title)); pend_changed = true; }
                if !preview.is_empty() { o.insert("summary".into(), json!(preview)); pend_changed = true; }
            }
            let mut cs = self.owner_claim(&m.id);
            cs.push(set(item_key(&m.id), "dedup", self.now));
            if pend_changed { cs.push(set(pending_key(bot), &jshow(json!(pend)), self.now)); }
            self.replay(&cs);
            return Some(Step { cs, outs: vec![] });
        }
        if source == "triage-end" {
            let told_at: u64 = self.setting(&told_at_key(bot, &thread), "").parse().unwrap_or(0);
            if told_at > 0 && self.now.saturating_sub(told_at) < COOLDOWN {
                let mut cs = self.owner_claim(&m.id);
                cs.push(set(item_key(&m.id), "dedup", self.now));
                self.replay(&cs);
                return Some(Step { cs, outs: vec![] });
            }
        }
        xs.push(json!({"input":m.id,"conv":m.conv,"thread":thread,"title":title,"summary":sum,"source":source,"at":self.now}));
        pend.push(json!({"input":m.id,"thread":thread,"title":title,"summary":preview,"source":source,"at":self.now,"due_at":self.now + FALLBACK_AFTER,"lane":""}));
        let mut cs = self.owner_claim(&m.id);
        cs.push(set(item_key(&m.id), "batched", self.now));
        cs.push(set(batch_key(bot), &jshow(json!(xs)), self.now));
        cs.push(set(pending_key(bot), &jshow(json!(pend)), self.now));
        if self.setting(&due_key(bot), "").is_empty() { cs.push(set(due_key(bot), &(self.now + WINDOW).to_string(), self.now)) }
        self.replay(&cs); Some(Step { cs, outs: vec![] })
    }
    /// Whether `bot` currently has a live done lane still running inside its grace window.
    pub(super) fn has_live_done_lane(&self, bot: &str) -> bool {
        !bot.is_empty() && self.w.mail.runs.values().any(|r| {
            r.live && r.bot == bot && self.w.private_is(&r.thread)
                && !self.setting(&lane_key(&r.thread), "").is_empty()
                && self.th(&r.thread).turn == "running"
                && self.now.saturating_sub(r.at) < FALLBACK_AFTER
        })
    }
    /// Done.flush: a batch whose window is over becomes one private lane (Private.spawn) that owes one tell_person
    pub fn done_flush(&mut self, bot: &str) -> Step { self.done_flush_inner(bot, false) }
    /// Flush any queued finishes immediately (used when a running mailcoord lane ends while completions arrived).
    pub fn done_flush_now(&mut self, bot: &str) -> Step { self.done_flush_inner(bot, true) }
    fn done_flush_inner(&mut self, bot: &str, force: bool) -> Step {
        if bot.is_empty() { return Step::none() }
        if !force {
            let due: u64 = self.setting(&due_key(bot), "").parse().unwrap_or(0);
            if due == 0 || self.now < due { return Step::none() }
        }
        let mut xs = items(&self.setting(&batch_key(bot), "[]"));
        if force && xs.is_empty() { return Step::none() }
        let mut cs = vec![set(batch_key(bot), "[]", self.now), set(due_key(bot), "", self.now)];
        if xs.is_empty() || self.w.triage_thread().is_empty() { self.replay(&cs); return Step { cs, outs: vec![] } }
        for i in &mut xs {
            if let Some(th) = i["thread"].as_str().filter(|t| !t.is_empty()).map(str::to_string) {
                let th_title = self.th(&th).title;
                if !th_title.trim().is_empty() {
                    if let Some(o) = i.as_object_mut() { o.insert("title".into(), json!(th_title)); }
                }
                if let Some(last_r) = self.f.last.get(&th).map(|l| l.0.as_str()).filter(|s| !s.trim().is_empty()) {
                    let s = first_lines(last_r, 1, 200);
                    if !s.is_empty() {
                        if let Some(o) = i.as_object_mut() { o.insert("summary".into(), json!(s)); }
                    }
                }
            }
        }
        let id = self.id("done"); let key = format!("done:{id}");
        let build = running_build();
        let list: String = xs.iter().map(|i| format!("- {} · input {}\n", line(i), i["input"].as_str().unwrap_or(""))).collect();
        let note = format!("{DONE_HEAD}tell_person key: {key}\nRunning hub build: {}\n\nFinished ({}):\n{list}", if build.is_empty() { "unknown (check with the person's build folder)".to_string() } else { build }, xs.len());
        for i in &xs { cs.push(set(item_key(i["input"].as_str().unwrap_or("")), &key, self.now)) }
        self.replay(&cs);
        let spawn = self.private_spawn(bot, &format!("done-batch:{id}"), &note);
        let lane = spawn.cs.iter().find(|e| e["$"] == "ThreadCreated").and_then(|e| e["id"].as_str()).unwrap_or("").to_string();
        let mut own = vec![set(lane_key(&lane), &jshow(json!({"key":key,"text":digest(&xs)})), self.now)];
        let mut pend = items(&self.setting(&pending_key(bot), "[]"));
        if !pend.is_empty() && !lane.is_empty() {
            let mut changed = false;
            for p in &mut pend {
                let inp = p["input"].as_str().unwrap_or("");
                let th = p["thread"].as_str().unwrap_or("");
                if xs.iter().any(|i| i["input"].as_str() == Some(inp) || (!th.is_empty() && i["thread"].as_str() == Some(th))) {
                    if let Some(o) = p.as_object_mut() {
                        o.insert("lane".into(), json!(lane));
                        let old_due = o.get("due_at").and_then(Value::as_u64).unwrap_or(0);
                        o.insert("due_at".into(), json!(old_due.max(self.now + FALLBACK_AFTER)));
                        changed = true;
                    }
                }
            }
            if changed { own.push(set(pending_key(bot), &jshow(json!(pend)), self.now)); }
        }
        self.replay(&own);
        cs.append(&mut own);
        Step { cs, outs: vec![] }.cat(spawn)
    }
    /// Clear pending fallback entries when `tell_person` handles them.
    pub fn done_pending_clear(&mut self, bot: &str, lane: &str, r: Option<&super::gate::Ref>, text: &str) -> Vec<Value> {
        let mut pend = items(&self.setting(&pending_key(bot), "[]"));
        if pend.is_empty() { return vec![] }
        let mut cleared_threads: Vec<String> = vec![];
        let mut keep = vec![];
        let any_named = r.is_none() && pend.iter().any(|i| contains_id(text, i["thread"].as_str().unwrap_or("")));
        for i in pend.drain(..) {
            let th = i["thread"].as_str().unwrap_or("").to_string();
            let remove = match r {
                Some(super::gate::Ref::Thread { id }) => th == id.as_str(),
                _ if any_named => !th.is_empty() && contains_id(text, &th),
                _ => !lane.is_empty() && i["lane"].as_str() == Some(lane),
            };
            if remove {
                if !th.is_empty() && !cleared_threads.contains(&th) { cleared_threads.push(th); }
            } else {
                keep.push(i);
            }
        }
        if cleared_threads.is_empty() && keep.len() == items(&self.setting(&pending_key(bot), "[]")).len() {
            return vec![];
        }
        let mut cs = vec![set(pending_key(bot), &jshow(json!(keep)), self.now)];
        for th in cleared_threads {
            cs.push(set(told_at_key(bot, &th), &self.now.to_string(), self.now));
        }
        cs
    }
    /// Done.lane.end: a done lane that ended (any end) without telling has the hub settle pending state:
    /// a cleanly completed `launch` lane that chose silence removes its `launch` entries (no Nov 20 digest noise);
    /// `triage-end` entries (or any failed/interrupted lane's entries) restore their `at + FALLBACK_AFTER` deadline.
    pub fn done_lane_end(&mut self, lane: &str, state: &str) -> Option<Step> {
        let owed = self.setting(&lane_key(lane), ""); if owed.is_empty() { return None }
        if !["completed", "failed", "interrupted"].contains(&state) { return Some(Step::none()) }
        if !self.setting(&said_key(lane), "").is_empty() || !self.setting(&handed_key(lane), "").is_empty() { return Some(Step::none()) }
        let v: Value = serde_json::from_str(&owed).unwrap_or(Value::Null); let (key, text) = (v["key"].as_str().unwrap_or("").to_string(), v["text"].as_str().unwrap_or("").to_string());
        let mut cs = vec![set(handed_key(lane), &self.now.to_string(), self.now)];
        let bot = self.w.private_owner(lane);
        let mut pend = items(&self.setting(&pending_key(&bot), "[]"));
        if !pend.is_empty() {
            let mut changed = false;
            let mut next_pend = vec![];
            for mut p in pend.drain(..) {
                if p["lane"].as_str() == Some(lane) {
                    if state == "completed" && p["source"].as_str() == Some("launch") {
                        changed = true;
                        continue;
                    }
                    let base = p["at"].as_u64().unwrap_or(self.now);
                    let target_due = base + FALLBACK_AFTER;
                    if p["due_at"].as_u64() != Some(target_due) {
                        if let Some(o) = p.as_object_mut() { o.insert("due_at".into(), json!(target_due)); changed = true; }
                    }
                }
                next_pend.push(p);
            }
            if changed { cs.push(set(pending_key(&bot), &jshow(json!(next_pend)), self.now)); }
        }
        self.replay(&cs);
        if key.is_empty() || text.is_empty() || !self.setting(&private_told_key(&bot, &key), "").is_empty() { return Some(Step { cs, outs: vec![] }) }
        // Gate.tell: the hub's own digest names no verified landing, release or ask, so it is never told (Nov 20: 56 "Done: …" digests reached the person): a lane
        // that has something material tells it with tell_person and its ref; silence is the right end
        let _ = text; Some(Step { cs, outs: vec![] })
    }
    /// the owners with a done batch whose window is over (waiting if another done lane is still running inside its grace window)
    pub fn done_due(&self) -> Vec<String> {
        let ub = self.w.uber_bot(); let due: u64 = self.setting(&due_key(&ub), "").parse().unwrap_or(0);
        if ub.is_empty() || due == 0 || self.now < due || self.has_live_done_lane(&ub) { vec![] } else { vec![ub] }
    }
    /// the owners with an untold completion whose fallback deadline has expired
    pub fn done_fallback_due(&self) -> Vec<String> {
        let ub = self.w.uber_bot();
        if ub.is_empty() { return vec![] }
        let due = items(&self.setting(&pending_key(&ub), "[]")).iter().any(|i| {
            let d = i["due_at"].as_u64().unwrap_or(0);
            d > 0 && self.now >= d
        });
        if due { vec![ub] } else { vec![] }
    }
    /// Post a plain fallback tell for any completion still untold past `FALLBACK_AFTER`.
    pub fn done_fallback_flush(&mut self, bot: &str) -> Step {
        if bot.is_empty() { return Step::none() }
        let pend = items(&self.setting(&pending_key(bot), "[]"));
        if pend.is_empty() { return Step::none() }
        let (mut expired, mut keep) = (vec![], vec![]);
        for i in pend {
            let d = i["due_at"].as_u64().unwrap_or(0);
            if d > 0 && self.now >= d { expired.push(i); } else { keep.push(i); }
        }
        if expired.is_empty() { return Step::none() }
        let mut cs = vec![set(pending_key(bot), &jshow(json!(keep)), self.now)];
        let mut batch = items(&self.setting(&batch_key(bot), "[]"));
        let blen = batch.len();
        batch.retain(|b| !expired.iter().any(|e| !e["thread"].as_str().unwrap_or("").is_empty() && e["thread"] == b["thread"]));
        if batch.len() != blen {
            cs.push(set(batch_key(bot), &jshow(json!(batch)), self.now));
            if batch.is_empty() { cs.push(set(due_key(bot), "", self.now)); }
        }
        self.replay(&cs);
        let mut st = Step { cs, outs: vec![] };
        for i in expired {
            let thread = i["thread"].as_str().unwrap_or("");
            let lane = i["lane"].as_str().unwrap_or("");
            let source = i["source"].as_str().unwrap_or("");
            let input = i["input"].as_str().unwrap_or("");
            if thread.is_empty() { continue }
            if source == "triage-end" {
                let told_at: u64 = self.setting(&told_at_key(bot, thread), "").parse().unwrap_or(0);
                if told_at > 0 && self.now.saturating_sub(told_at) < COOLDOWN { continue }
            }
            let th_title = self.th(thread).title;
            let title = if !th_title.trim().is_empty() { th_title } else { i["title"].as_str().unwrap_or("").to_string() };
            let reply = self.f.last.get(thread).map(|l| l.0.as_str()).filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| i["summary"].as_str().unwrap_or(""));
            let preview = first_lines(reply, 3, 300);
            if super::gate::quiet_report(&preview) { continue }
            let text = fallback_text(&title, reply, thread);
            let fb_key = if input.is_empty() { format!("done-fallback:{thread}:{}", self.now) } else { format!("done-fallback:{input}") };
            let told = self.private_tell_as(bot, lane, &text, &fb_key);
            if !told.cs.is_empty() {
                let mark = vec![
                    set(told_at_key(bot, thread), &self.now.to_string(), self.now),
                    set(fallback_at_key(bot, thread), &self.now.to_string(), self.now),
                ];
                self.replay(&mark);
                st = st.cat(Step { cs: mark, outs: vec![] }).cat(told);
            }
        }
        st
    }
}

/// Earliest pending timer deadline across done batch, fallback guarantees, and tell batch (0 = none).
pub fn next_deadline(w: &crate::bots::world::World, bot: &str) -> u64 {
    if bot.is_empty() { return 0 }
    let mut best = 0u64;
    let mut consider = |v: u64| { if v > 0 && (best == 0 || v < best) { best = v; } };
    consider(w.setting(&due_key(bot), "").parse().unwrap_or(0));
    consider(w.setting(&super::gate::due_key(bot), "").parse().unwrap_or(0));
    for i in items(&w.setting(&pending_key(bot), "[]")) {
        consider(i["due_at"].as_u64().unwrap_or(0));
    }
    best
}

impl Bots {
    /// Done.tick: the minute tick, and the timer a batch's first finish sets, flush what is due
    pub async fn done_tick(self: &Arc<Self>) { self.done_tick_at(now_s()).await }
    pub async fn done_tick_at(self: &Arc<Self>, now: u64) {
        if crate::copy::on() { return }
        let report: Vec<String> = {
            let w = self.world().await;
            let ub = w.uber_bot();
            if ub.is_empty() { vec![] } else {
                let mut threads: Vec<String> = vec![];
                for key in [pending_key(&ub), batch_key(&ub)] {
                    for i in items(&w.setting(&key, "[]")) {
                        if let Some(th) = i["thread"].as_str().filter(|t| !t.is_empty()) {
                            let s = th.to_string();
                            if !threads.contains(&s) { threads.push(s); }
                        }
                    }
                }
                threads
            }
        };
        let outs = self.decide_at(&report, now, |c| {
            let mut st = Step::none();
            for b in c.done_due() { let s = c.done_flush(&b); st = st.cat(s); }
            for b in c.done_fallback_due() { let s = c.done_fallback_flush(&b); st = st.cat(s); }
            for b in c.tell_due() { let s = c.tell_flush(&b); st = st.cat(s); }
            st
        }).await;
        self.perform(outs).await;
        self.arm_done_timer().await;
    }
    /// Arm `done_timer` for the earliest pending batch, fallback, or tell deadline.
    pub(super) async fn arm_done_timer(self: &Arc<Self>) {
        if crate::copy::on() { return }
        let due = { let w = self.world().await; next_deadline(&w, &w.uber_bot()) };
        self.done_timer(due);
    }
    /// Done.timer: once a batch opens, a flush when its window is over (the minute tick covers a restart in between)
    pub(super) fn done_timer(self: &Arc<Self>, due: u64) {
        if crate::copy::on() || due == 0 { return }
        loop {
            let cur = self.done_timer_at.load(std::sync::atomic::Ordering::Acquire);
            if cur != 0 && cur <= due { return }
            if self.done_timer_at.compare_exchange(cur, due, std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Acquire).is_ok() { break }
        }
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            let now = now_s();
            let wait_s = if due > now { due - now + 1 } else { 30 };
            tokio::time::sleep(std::time::Duration::from_secs(wait_s)).await;
            let Some(b) = me.upgrade() else { return };
            let _ = b.done_timer_at.compare_exchange(due, 0, std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Acquire);
            b.done_tick().await
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
