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

/// Words that say nothing happened the person can use. Lowercase, straight apostrophes. A second layer only: a tell first needs a verified `Ref` (below).
const NO_CHANGE: &[&str] = &[
    "nothing landed", "nothing has landed", "nothing new", "nothing to try", "nothing's been", "nothing has been released", "nothing is released", "nothing released",
    "nothing changed", "nothing has changed", "nothing was merged", "nothing merged", "nothing yet", "no change", "unchanged", "no new results", "no results yet",
    "no news", "no update", "still waiting", "still testing", "still running", "still nothing", "still isn't", "still hasn't", "still not ", "still in progress",
    "still being", "still on hold", "still going", "still an ", "not on main yet", "isn't on main yet", "not landed yet", "isn't landed", "not landed", "hasn't landed", "has not landed",
    "isn't merged", "not merged", "hasn't merged", "queued", "rebased", "rebasing", "waiting its turn", "waiting on its", "is landing now", "landing now",
    "in review", "being reviewed", "under review", "awaiting review", "prep step only", "groundwork only",
    // Nov 20 afternoon: the notes that reached the person after #1437 (gate.rs laws: the_notes_that_leaked_after_1437_are_refused)
    "nothing for you", "nothing here needs", "nothing needs you", "nothing needs your", "nothing to do", "nothing to decide", "no decision needed", "nothing for you to",
    "next in line", "in line to land", "waiting behind", "waiting for a free", "is waiting on", "are waiting on", "is waiting for", "are waiting for", "on hold", "paused",
    "correction", "corrected", "internal only", "only reviews", "no new release", "isn't released", "not released yet", "isn't published", "not published",
    "unpublished", "staged but", "being written", "being built", "being tested", "being rerun", "heads-up", "i'll tell you when", "i'll tell you once",
    "will tell you when", "once it lands", "when it lands", "once it's on main", "once it's live", "once it's released", "todo digest", "morning summary",
];
/// Words a landing or release tell must have
const LANDED: &[&str] = &["landed", "merged", "on main", "released", "release", "in your build", "in your running build", "shipped", "published", "is live", "is out"];
/// Words a tell that needs the person must have
const NEEDS: &[&str] = &["?", "your call", "decide", "decision", "approve", "approval", "need you", "needs you", "needs your", "need your", "blocked", "blocker",
    "your answer", "confirm", "choose", "pick ", "your ok", "sign in", "log in", "password", "permission", "waiting on you"];
/// Words that a turn-end report asks the person something (narrower than NEEDS: a lead's "passed my decision to the lanes" asks nothing)
const ASKS: &[&str] = &["?", "needs your", "need your", "needs you", "need you", "your answer", "your call", "your approval", "your ok", "waiting on you", "yours to make"];
/// Gate.recent: a landing or release older than this is not news (a lane citing an old merge in a status note)
pub const RECENT: u64 = 2 * 86400;

fn norm(t: &str) -> String { t.to_lowercase().replace(['\u{2019}', '\u{2018}'], "'") }
fn has(t: &str, ws: &[&str]) -> Option<String> { ws.iter().find(|w| t.contains(**w)).map(|w| w.trim().to_string()) }

/// Gate.kind: what a tell is: a landing on main, a release to try, or something that needs the person now
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind { Landed, Release, NeedsYou }
impl Kind {
    pub fn parse(s: &str) -> Option<Kind> { match s.trim() { "landed" => Some(Kind::Landed), "release" => Some(Kind::Release), "needs_you" => Some(Kind::NeedsYou), _ => None } }
    pub fn urgent(self) -> bool { self == Kind::NeedsYou }
}

/// Gate.ref: the one thing a tell is about, which the hub checks before anything is told: a pull request merged to its repository's default branch, a commit
/// on it, a published release, or an ask open in the hub. A tell without one is not told.
#[derive(Clone, PartialEq, Debug)]
pub enum Ref { Pr { repo: String, n: u64 }, Commit { repo: String, sha: String }, Release { repo: String, tag: String }, Ask { id: String } }
fn repo_ok(r: &str) -> bool { let mut p = r.split('/'); matches!((p.next(), p.next(), p.next()), (Some(o), Some(n), None) if !o.is_empty() && !n.is_empty() && r.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))) }
fn tag_ok(t: &str) -> bool { !t.is_empty() && t.len() <= 120 && t.chars().all(|c| c.is_ascii_alphanumeric() || "-_.+".contains(c)) }
impl Ref {
    /// `owner/repo#N` or a pull URL; `commit:owner/repo@sha`; `release:owner/repo@tag` or a release-tag URL; `ask:<id>`
    pub fn parse(s: &str) -> Result<Ref, String> {
        let s = s.trim(); let bad = || Err(format!("ref must be owner/repo#N (a merged pull request), commit:owner/repo@sha (a commit on main), release:owner/repo@tag (a published release) or ask:<id> (an open ask), not \"{}\"", take(s, 80)));
        let s2 = s.strip_prefix("https://github.com/").or_else(|| s.strip_prefix("pr:")).unwrap_or(s);
        if let Some(id) = s.strip_prefix("ask:") { let id = id.trim(); return if !id.is_empty() && id.len() <= 120 && id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c)) { Ok(Ref::Ask { id: id.into() }) } else { bad() } }
        if let Some(r) = s.strip_prefix("commit:") { let Some((repo, sha)) = r.split_once('@') else { return bad() }; let sha = sha.to_lowercase();
            return if repo_ok(repo) && (7..=40).contains(&sha.len()) && sha.chars().all(|c| c.is_ascii_hexdigit()) { Ok(Ref::Commit { repo: repo.into(), sha }) } else { bad() } }
        if let Some(r) = s.strip_prefix("release:") { let Some((repo, tag)) = r.split_once('@') else { return bad() };
            return if repo_ok(repo) && tag_ok(tag) { Ok(Ref::Release { repo: repo.into(), tag: tag.into() }) } else { bad() } }
        if let Some((repo, tag)) = s2.split_once("/releases/tag/") { return if repo_ok(repo) && tag_ok(tag) { Ok(Ref::Release { repo: repo.into(), tag: tag.into() }) } else { bad() } }
        let (repo, n) = if let Some((r, n)) = s2.split_once("/pull/") { (r, n) } else if let Some((r, n)) = s2.split_once('#') { (r, n) } else { return bad() };
        match n.trim_end_matches('/').parse::<u64>() { Ok(n) if n > 0 && repo_ok(repo) => Ok(Ref::Pr { repo: repo.into(), n }), _ => bad() }
    }
    /// the told key: one tell per landed PR, commit, release or ask, whatever key the lane gives
    pub fn key(&self) -> String { match self { Ref::Pr { repo, n } => format!("pr:{repo}#{n}"), Ref::Commit { repo, sha } => format!("commit:{repo}@{}", take(sha, 10)), Ref::Release { repo, tag } => format!("release:{repo}@{tag}"), Ref::Ask { id } => format!("ask:{id}") } }
    fn fits(&self, k: Kind) -> bool { matches!((self, k), (Ref::Pr { .. } | Ref::Commit { .. }, Kind::Landed) | (Ref::Release { .. }, Kind::Release) | (Ref::Ask { .. }, Kind::NeedsYou)) }
    /// the text names what it is about: the PR's number, the commit's short sha, the release's tag (an ask is linked by the hub, so no words are asked of it)
    fn named(&self, t: &str) -> bool {
        match self {
            Ref::Pr { n, .. } => { let n = n.to_string(); [format!("#{n}"), format!("/pull/{n}")].iter().any(|w| t.match_indices(w.as_str()).any(|(i, _)| !t[i + w.len()..].starts_with(|c: char| c.is_ascii_digit()))) }
            Ref::Commit { sha, .. } => t.contains(&sha[..7]), Ref::Release { tag, .. } => t.contains(&tag.to_lowercase()), Ref::Ask { .. } => true,
        }
    }
}
fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }

/// Gate.proof: what the hub found about a tell's ref before the gate ran (`verify`): None, no ref given; Some(Err), the check refused it
pub type Proof = Option<(Ref, Result<(), String>)>;

/// Gate.refusal: why a tell is not material (None: it may go out). Default deny: a kind, a ref the hub verified (merged, on main, published or open), and words
/// that name it and say no no-change status.
pub fn refusal(kind: &str, text: &str, proof: &Proof) -> Option<String> {
    let Some(k) = Kind::parse(kind) else { return Some("kind must be landed (merged to main), release (a build or release the person can try now) or needs_you (a decision, approval or blocker only the person can resolve). Anything else is not told.".into()) };
    let t = norm(text);
    if let Some(w) = has(&t, NO_CHANGE) { return Some(format!("refused: \"{w}\" is a no-change status. The person hears only landings, releases they can try, and what needs them; progress, waiting, review, queues, corrections and rebases stay private. Drop the status words and tell only the material item, or end silently.")) }
    let Some((r, checked)) = proof else { return Some("refused: a tell must give ref: the merged pull request (owner/repo#N), the commit on main (commit:owner/repo@sha), the published release (release:owner/repo@tag) or the open ask (ask:<id>) it is about. Without one nothing is told; end silently.".into()) };
    if !r.fits(k) { return Some(format!("refused: a {kind} tell needs a {} ref, not {}.", match k { Kind::Landed => "merged pull request or commit", Kind::Release => "release", Kind::NeedsYou => "open ask" }, r.key())) }
    if let Err(why) = checked { return Some(format!("refused: {} is not verified: {why}. Only what is merged, published or open is told.", r.key())) }
    if !r.named(&t) { return Some(format!("refused: the text must name {} (its number, short sha or tag).", r.key())) }
    match k {
        Kind::Landed | Kind::Release if has(&t, LANDED).is_none() => Some("refused: a landed or release tell must say what landed on main or what was released for the person to try.".into()),
        Kind::NeedsYou if has(&t, NEEDS).is_none() => Some("refused: a needs_you tell must name the decision, approval or blocker and what the person should answer.".into()),
        _ => None,
    }
}
/// Gate.verify.pr: `gh api repos/R/pulls/N --jq '[(.merged_at // "" | if . == "" then 0 else fromdateiso8601 end), .base.ref, .base.repo.default_branch] | map(tostring) | join(" ")'`
pub fn pr_landed(out: &str, now: u64) -> Result<(), String> {
    let f: Vec<&str> = out.split_whitespace().collect();
    let (Some(at), Some(base), Some(main)) = (f.first().and_then(|a| a.parse::<u64>().ok()), f.get(1), f.get(2)) else { return Err("GitHub did not say".into()) };
    if at == 0 { return Err("it is not merged".into()) } if base != main { return Err(format!("it merged into {base}, not {main}")) }
    if now.saturating_sub(at) > RECENT { return Err("it merged more than two days ago: not news".into()) } Ok(())
}
/// Gate.verify.commit: `gh api repos/R/compare/<default>...<sha> --jq .status` (behind or identical: the commit is on the default branch)
pub fn commit_landed(status: &str) -> Result<(), String> { match status.trim() { "behind" | "identical" => Ok(()), "" => Err("GitHub did not say".into()), s => Err(format!("it is not on the default branch ({s})")) } }
/// Gate.verify.release: `gh api repos/R/releases/tags/T --jq '[.draft, (.published_at // "" | if . == "" then 0 else fromdateiso8601 end)] | map(tostring) | join(" ")'`
pub fn release_out(out: &str, now: u64) -> Result<(), String> {
    let f: Vec<&str> = out.split_whitespace().collect();
    match (f.first().copied(), f.get(1).and_then(|a| a.parse::<u64>().ok())) {
        (Some("false"), Some(at)) if at > 0 => if now.saturating_sub(at) > RECENT { Err("it was published more than two days ago: not news".into()) } else { Ok(()) },
        (Some("true"), _) => Err("it is a draft, not published".into()), _ => Err("GitHub has no such release".into()),
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
    /// Gate.tell (tell_person): a material tell, its ref verified (`proof`), joins the owner's batch; Err is the refusal, nothing posted. Ok carries when the
    /// batch goes out (0: no change, the ref was told already). The told key is the ref's, so one landing, release or ask is told once whatever key a lane gives.
    pub fn tell_gate(&mut self, lane: &str, text: &str, key: &str, kind: &str, proof: &Proof) -> Result<(Step, u64), String> {
        let bot = self.w.private_owner(lane);
        if let Some(why) = refusal(kind, text, proof) { let p = self.report_prefs(&bot); return Err(if p.is_empty() { why } else { format!("{why}\nThe person's rules:{p}") }) }
        let rkey = proof.as_ref().map(|(r, _)| r.key()).unwrap_or_default();
        if !self.setting(&private_told_key(&bot, &rkey), "").is_empty() || !self.setting(&private_told_key(&bot, key), "").is_empty() { return Ok((Step::none(), 0)) }
        if items(&self.setting(&batch_key(&bot), "[]")).iter().any(|i| i["ref"] == rkey.as_str()) { return Ok((Step::none(), 0)) }
        let urgent = Kind::parse(kind).is_some_and(Kind::urgent); let now = self.now;
        let (last, open) = (num(self.setting(&last_key(&bot), "")), num(self.setting(&due_key(&bot), "")));
        let due = if urgent { let at = now.max(last + GAP); if open > 0 { open.min(at) } else { at } } else if open > 0 { open } else { (now + BATCH).max(last + BATCH) };
        let mut xs = items(&self.setting(&batch_key(&bot), "[]"));
        xs.push(json!({"key":key,"ref":rkey,"text":text,"lane":lane,"kind":kind,"at":now}));
        let cs = vec![set(batch_key(&bot), &jshow(json!(xs)), now), set(due_key(&bot), &due.to_string(), now),
            set(private_told_key(&bot, key), &format!("queued:{due}"), now), set(private_told_key(&bot, &rkey), &format!("queued:{due}"), now), set(said_key(lane), &format!("queued:{key}"), now)];
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

/// Tests stand in for GitHub: the ref keys that count as merged or published
#[cfg(test)] pub fn test_verified() -> &'static std::sync::Mutex<std::collections::HashSet<String>> { static V: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::OnceLock::new(); V.get_or_init(Default::default) }
/// Gate.verify: the hub's own check of a tell's ref, before the gate: GitHub for a pull request, commit or release (bounded, a read), the hub's asks for an ask.
/// Anything that cannot be checked is refused.
pub async fn verify(hub: &crate::hub::Hub, r: &Ref, now: u64) -> Result<(), String> {
    use crate::files::git::read_within;
    let gh = |args: Vec<String>| async move { let a: Vec<&str> = args.iter().map(String::as_str).collect(); read_within(std::time::Duration::from_secs(20), "gh", &a, "/", &[]).await };
    let said = |r: crate::files::git::Ran| if r.code == 0 { Ok(r.out.trim().to_string()) } else { Err(format!("GitHub could not be asked (gh exit {})", r.code)) };
    #[cfg(test)] if !matches!(r, Ref::Ask { .. }) { let _ = (&gh, &said, now); return if test_verified().lock().unwrap().contains(&r.key()) { Ok(()) } else { Err("not merged or published (test)".into()) } }
    match r {
        Ref::Ask { id } => crate::data::ask_thread(hub, id).map(|_| ()).ok_or_else(|| "no such open ask in the hub".to_string()),
        Ref::Pr { repo, n } => pr_landed(&said(gh(vec!["api".into(), format!("repos/{repo}/pulls/{n}"), "--jq".into(),
            "[(.merged_at // \"\" | if . == \"\" then 0 else fromdateiso8601 end), .base.ref, .base.repo.default_branch] | map(tostring) | join(\" \")".into()]).await)?, now),
        Ref::Commit { repo, sha } => {
            let main = said(gh(vec!["api".into(), format!("repos/{repo}"), "--jq".into(), ".default_branch".into()]).await)?;
            commit_landed(&said(gh(vec!["api".into(), format!("repos/{repo}/compare/{main}...{sha}"), "--jq".into(), ".status".into()]).await)?)
        }
        Ref::Release { repo, tag } => release_out(&said(gh(vec!["api".into(), format!("repos/{repo}/releases/tags/{tag}"), "--jq".into(),
            "[.draft, (.published_at // \"\" | if . == \"\" then 0 else fromdateiso8601 end)] | map(tostring) | join(\" \")".into()]).await).unwrap_or_default(), now),
    }
}

#[cfg(test)]
mod laws {
    use super::*;
    const BP: &str = "BackplaneWorks/Backplane";
    fn pr(n: u64) -> Ref { Ref::Pr { repo: BP.into(), n } }
    fn ok(r: Ref) -> Proof { Some((r, Ok(()))) }
    fn no(r: Ref) -> Proof { Some((r, Err("it is not merged".into()))) }
    /// every tell_person that reached Bub's chat on Nov 20 between #1437's merge and this fix (events.jsonl from 1795163572, thread t1790292962-8017): 127 notes
    fn leaked() -> Vec<String> { serde_json::from_str(include_str!("gate_leaked_20261120.json")).unwrap() }
    /// the first pull request a note names, as a lane would have given it for ref
    fn first_pr(t: &str) -> Option<u64> { t.split("/pull/").nth(1).map(|r| r.chars().take_while(char::is_ascii_digit).collect::<String>()).and_then(|n| n.parse().ok()) }

    #[test] fn the_notes_that_leaked_after_1437_are_refused() {
        let xs = leaked(); assert_eq!(xs.len(), 127);
        // the person's five examples are among them
        for w in ["Nothing for you here: ", "Nothing new to try yet", "is next in line to land", "No new results: the Integrator", "Nothing needs you: ", "Correction on "] {
            assert!(xs.iter().any(|t| t.contains(w)), "{w}") }
        for t in &xs { for k in ["landed", "release", "needs_you"] {
            // as sent: no ref the hub could check
            assert!(refusal(k, t, &None).is_some(), "{k} without a ref: {t}");
            // with the PR it names, as GitHub had it then for all but the genuine landings: not merged
            if let Some(n) = first_pr(t) { assert!(refusal(k, t, &no(pr(n))).is_some(), "{k} #{n} unmerged: {t}") }
        } }
        // the no-change ones (the five kinds the person named, and the hub's digests) stay refused even if the PR they cite had merged
        let noise: Vec<&String> = xs.iter().filter(|t| { let l = norm(t); ["nothing ", "done:", "correction", "a correction", "kicad: nothing", "internal only", "only reviews", "your desktop"].iter().any(|p| l.starts_with(p)) || l.contains("next in line") }).collect();
        assert!(noise.len() >= 80, "{}", noise.len());
        for t in noise { if t.starts_with("Done:") { continue } // the digest is never told at all (done.rs Done.lane.end)
            let n = first_pr(t).unwrap_or(1);
            for k in ["landed", "release", "needs_you"] { assert!(refusal(k, t, &ok(pr(n))).is_some(), "{k} #{n} merged, still no-change: {t}") }
        }
    }
    #[test] fn a_tell_needs_a_verified_ref_that_fits_its_kind_and_is_named() {
        let land = "KiCad: PR #1361 has landed on main (`5f3811dc45`). Your running build doesn't have it yet; it'll be in the next one.";
        assert_eq!(refusal("landed", land, &ok(pr(1361))), None);
        assert!(refusal("landed", land, &None).unwrap().contains("must give ref"));
        assert!(refusal("landed", land, &no(pr(1361))).unwrap().contains("not verified"), "GitHub says it is not merged");
        assert!(refusal("landed", land, &ok(pr(1362))).unwrap().contains("must name"), "a ref the text does not name");
        assert!(refusal("landed", "PR #13610 landed on main.", &ok(pr(1361))).is_some(), "#13610 is not #1361");
        assert_eq!(refusal("landed", land, &ok(Ref::Commit { repo: BP.into(), sha: "5f3811dc45".into() })), None, "a commit on main");
        assert!(refusal("release", land, &ok(pr(1361))).unwrap().contains("needs a release ref"));
        let rel = "Backplane KiCad bp.13.3 is released: restart Backplane and open a board to try it (backplane-kicad-v10.99.0-bp.13.3).";
        assert_eq!(refusal("release", rel, &ok(Ref::Release { repo: "BackplaneWorks/Backplane_KiCad".into(), tag: "backplane-kicad-v10.99.0-bp.13.3".into() })), None);
        let ask = "There is a macOS privacy prompt on the Mac mini that needs your answer: allow Documents access?";
        assert_eq!(refusal("needs_you", ask, &ok(Ref::Ask { id: "q1".into() })), None);
        assert!(refusal("needs_you", ask, &Some((Ref::Ask { id: "q1".into() }, Err("no such open ask in the hub".into())))).is_some(), "an ask that is not open");
        assert!(refusal("needs_you", ask, &ok(pr(1))).is_some(), "needs_you is about an open ask");
        assert!(refusal("needs_you", "Carbon measured the Windows focus pause.", &ok(Ref::Ask { id: "q1".into() })).is_some(), "no ask in its words");
        assert!(refusal("", land, &ok(pr(1361))).is_some() && refusal("progress", land, &ok(pr(1361))).is_some(), "a kind is required");
    }
    #[test] fn refs_parse_strictly_and_key_one_tell_each() {
        assert_eq!(Ref::parse("BackplaneWorks/Backplane#1437"), Ok(pr(1437)));
        assert_eq!(Ref::parse("https://github.com/BackplaneWorks/Backplane/pull/1437"), Ok(pr(1437)));
        assert_eq!(Ref::parse("commit:BackplaneWorks/Backplane@09D43D3BD1"), Ok(Ref::Commit { repo: BP.into(), sha: "09d43d3bd1".into() }));
        assert_eq!(Ref::parse("https://github.com/BackplaneWorks/Backplane_KiCad/releases/tag/backplane-kicad-v10.99.0-bp.13.3"), Ok(Ref::Release { repo: "BackplaneWorks/Backplane_KiCad".into(), tag: "backplane-kicad-v10.99.0-bp.13.3".into() }));
        assert_eq!(Ref::parse("ask:q1795-3"), Ok(Ref::Ask { id: "q1795-3".into() }));
        for b in ["", "#1437", "1437", "pr:1437", "commit:abc", "commit:BackplaneWorks/Backplane@xyz1234", "release:x@", "ask:", "https://github.com/BackplaneWorks/Backplane/issues/1403", "a/b/c#1", "done:done1795"] {
            assert!(Ref::parse(b).is_err(), "{b}") }
        assert_eq!(pr(1437).key(), "pr:BackplaneWorks/Backplane#1437"); assert_eq!(Ref::parse(&pr(1437).key()), Ok(pr(1437)), "a told key reads back");
    }
    #[test] fn githubs_answers_are_read_strictly() {
        let now = 1_795_170_000;
        assert_eq!(pr_landed(&format!("{} main main", now - 60), now), Ok(()));
        assert!(pr_landed("0 main main", now).is_err(), "open"); assert!(pr_landed(&format!("{} rail main", now), now).is_err(), "into another branch");
        assert!(pr_landed(&format!("{} main main", now - RECENT - 1), now).is_err(), "old news"); assert!(pr_landed("", now).is_err());
        assert_eq!(commit_landed("behind"), Ok(())); assert_eq!(commit_landed("identical"), Ok(())); assert!(commit_landed("ahead").is_err() && commit_landed("diverged").is_err() && commit_landed("").is_err());
        assert_eq!(release_out(&format!("false {}", now - 5), now), Ok(())); assert!(release_out(&format!("true {now}"), now).is_err(), "a draft");
        assert!(release_out("false 0", now).is_err() && release_out("", now).is_err());
    }
    #[test] fn a_quiet_turn_end_is_no_finish_but_an_ask_is() {
        assert!(quiet_report("No new results: Integrator has passed my decision to the lanes."));
        assert!(quiet_report("The Follow lane's fix build is written, compiled for Windows, and queued with Carbon. No results yet."));
        assert!(!quiet_report("Landed 1a2b3c4 on main: KiCad opens inside the Board tab."));
        assert!(!quiet_report("Nothing landed, but the bp.13.x licence decision needs your answer."));
    }
}
