//! The room guards and the wake-up batching (docs/SWARM.md): pure functions of a room's recent posts, so every room (a swarm, the person's,
//! a bots' exchange) is bounded the same way. They replace the old fixed hop cap; the hop count is still carried and kept only as a backstop.
use std::collections::HashSet;

/// What the gate knows of one post (folded from RoomPosted in `world`): the newest `KEEP` of a room are kept.
#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct PostMeta {
    pub id: String,
    /// who posted: a member key (`thread:<id>`, a bot's id or name), "" for the person, `hub` for a guard's own post
    pub member: String,
    pub tags: Vec<String>,
    /// the start of the text (for the circular guard)
    pub text: String,
    pub at: u64,
    /// nobody was woken by it
    pub held: bool,
}
impl PostMeta {
    pub fn person(&self) -> bool { self.member.is_empty() }
    pub fn hub(&self) -> bool { self.member == "hub" }
    pub fn has(&self, t: &str) -> bool { self.tags.iter().any(|x| x == t) }
    /// a post that shows work moving: a claim, a done, a synthesis
    pub fn progress(&self) -> bool { self.has("claim") || self.has("done") || self.has("synthesis") }
    /// a post that starts the counters again: the person's, or the hub's resume
    pub fn fresh(&self) -> bool { self.person() || (self.hub() && self.has("resume")) }
}
pub const KEEP: usize = 64;
pub const TEXT: usize = 400;

pub const TAGS: [&str; 6] = ["idea", "critique", "claim", "done", "question", "synthesis"];
/// the tags given, known ones only, each once, in the order given
pub fn tags_of(given: &[String]) -> Vec<String> { let mut v: Vec<String> = vec![]; for t in given { let t = t.trim().to_lowercase(); if TAGS.contains(&t.as_str()) && !v.contains(&t) { v.push(t) } } v }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// a member posts at most `rate_n` times in `rate_win` seconds
    pub rate_n: usize, pub rate_win: u64,
    /// member posts in a row with no progress and no person
    pub chatter: usize,
    /// the circular guard looks at this many member posts and fires at this mean similarity
    pub circ_n: usize, pub circ_sim: f64,
    /// every member idle this long (swarms)
    pub idle: u64,
    /// a hop this deep is held whatever the guards say (the old cap's safety net)
    pub backstop: u64,
    /// a member hears at most once in this many seconds (thread members)
    pub batch: u64,
}
pub const LIMITS: Limits = Limits { rate_n: 6, rate_win: 300, chatter: 12, circ_n: 8, circ_sim: 0.5, idle: 600, backstop: 30, batch: 20 };

/// A swarm's budget (`RoomSet.budget`); 0 is no limit.
#[derive(Clone, Copy, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Budget { #[serde(default)] pub posts: u64, #[serde(default)] pub turns: u64, #[serde(default)] pub tokens: u64 }
pub const SWARM_BUDGET: Budget = Budget { posts: 300, turns: 120, tokens: 4_000_000 };
impl Budget {
    /// which part of the budget `used` has reached, if any
    pub fn hit(&self, used: &Budget) -> Option<&'static str> {
        if self.posts > 0 && used.posts >= self.posts { return Some("posts") }
        if self.turns > 0 && used.turns >= self.turns { return Some("turns") }
        if self.tokens > 0 && used.tokens >= self.tokens { return Some("tokens") }
        None
    }
}

/// The verdict on a new post by a member (not the person, who is never refused or held).
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// posted, and heard
    Live,
    /// not posted: the member posts too fast
    Refuse(String),
    /// posted, heard by nobody (`why`: paused, backstop)
    Hold(&'static str),
    /// posted and heard, then the room pauses (`why`: circular, chatter, budget)
    Pause(&'static str),
}

/// the posts since the counters last started again (the person's newest post or a resume), oldest first
pub fn since_fresh(posts: &[PostMeta]) -> &[PostMeta] { match posts.iter().rposition(PostMeta::fresh) { Some(i) => &posts[i + 1..], None => posts } }

/// rate: how many posts `member` made in the window ending `now`
pub fn rate_count(posts: &[PostMeta], member: &str, now: u64, lim: &Limits) -> usize { posts.iter().filter(|p| p.member == member && now.saturating_sub(p.at) < lim.rate_win).count() }

/// chatter: member posts in a row (since fresh) with no progress
pub fn chatter_run(posts: &[PostMeta]) -> usize { since_fresh(posts).iter().rev().filter(|p| !p.hub()).take_while(|p| !p.progress()).count() }

/// Word shingles (pairs of words, lower case, letters and digits) of a text, hashed.
pub fn shingles(t: &str) -> HashSet<u64> {
    let w: Vec<String> = t.split(|c: char| !c.is_alphanumeric()).filter(|x| x.chars().count() >= 2).map(str::to_lowercase).collect();
    let h = |s: &str| crate::ckpt::fnv(s.as_bytes());
    if w.len() < 2 { return w.iter().map(|x| h(x)).collect() }
    w.windows(2).map(|p| h(&format!("{} {}", p[0], p[1]))).collect()
}
pub fn jaccard(a: &HashSet<u64>, b: &HashSet<u64>) -> f64 { if a.is_empty() && b.is_empty() { return 1.0 } let i = a.intersection(b).count() as f64; let u = a.union(b).count() as f64; if u == 0.0 { 0.0 } else { i / u } }

/// circular: the last `circ_n` member posts (since fresh) hold no progress, and each repeats an earlier post of the room (its best match among
/// all posts kept before it) by `circ_sim` on average
pub fn circular(posts: &[PostMeta], lim: &Limits) -> bool {
    let all: Vec<&PostMeta> = posts.iter().filter(|p| !p.hub()).collect();
    let fresh = since_fresh(posts).iter().filter(|p| !p.hub()).count();
    if fresh < lim.circ_n || all.len() < lim.circ_n + 1 { return false }
    let tail = &all[all.len() - lim.circ_n..];
    if tail.iter().any(|p| p.progress() || p.person()) { return false }
    let sh: Vec<HashSet<u64>> = all.iter().map(|p| shingles(&p.text)).collect();
    let start = all.len() - lim.circ_n;
    let mean = (start..all.len()).map(|i| (0..i).map(|j| jaccard(&sh[i], &sh[j])).fold(0.0, f64::max)).sum::<f64>() / lim.circ_n as f64;
    mean >= lim.circ_sim
}

/// The verdict on a member's post `next` (its text and tags) to a room in `paused` state or not, with the hop it would go out at.
/// `posts` are the room's kept posts before it. Order: rate refuses first; a paused room or a hop past the backstop holds; then the guards
/// that pause the room look at the posts with this one added.
pub fn verdict(posts: &[PostMeta], next: &PostMeta, paused: bool, hop: u64, budget: &Budget, used: &Budget, lim: &Limits) -> Verdict {
    if !next.person() && !next.hub() && rate_count(posts, &next.member, next.at, lim) >= lim.rate_n {
        return Verdict::Refuse(format!("you have posted {} times in {} minutes here: slow down. Do the work you claimed, read the board (room_read) and post when you have something new.", lim.rate_n, lim.rate_win / 60))
    }
    if next.person() { return Verdict::Live }
    if paused { return Verdict::Hold("paused") }
    if hop > lim.backstop { return Verdict::Hold("backstop") }
    let mut with: Vec<PostMeta> = posts.to_vec(); with.push(next.clone());
    let used = Budget { posts: used.posts + 1, ..*used };
    if budget.hit(&used).is_some() { return Verdict::Pause("budget") }
    if circular(&with, lim) { return Verdict::Pause("circular") }
    if chatter_run(&with) >= lim.chatter { return Verdict::Pause("chatter") }
    Verdict::Live
}

/// What a guard's own post says when it pauses a room.
pub fn pause_text(why: &str, lim: &Limits) -> String {
    match why {
        "circular" => "Paused: the last posts go round in circles (they repeat earlier ones, and nobody claimed or finished anything). Nobody is woken until the person posts or resumes.".into(),
        "chatter" => format!("Paused: {} posts in a row with no claim and no done. Nobody is woken until the person posts or resumes.", lim.chatter),
        "budget" => "Paused: the room's budget is used up. Nobody is woken until the person raises it or resumes.".into(),
        "idle" => format!("Paused: every member has been idle for {} minutes.", lim.idle / 60),
        "person" => "Paused by the person.".into(),
        _ => format!("Paused ({why})."),
    }
}

/// Batching: when a member that last heard the room at `last` (0: never) is next given what waits, from `now`. `urgent` (the person's post, a
/// mention of the member) is at once.
pub fn due(last: u64, now: u64, urgent: bool, lim: &Limits) -> u64 { if urgent || last == 0 { now } else { now.max(last + lim.batch) } }

/// Mentions: the members a text names with `@name` (any case, the name followed by a non-name character), from `names` (member, name).
pub fn mentions(text: &str, names: &[(String, String)]) -> Vec<String> {
    let l = text.to_lowercase(); let mut out: Vec<String> = vec![];
    for (m, n) in names {
        let pat = format!("@{}", n.to_lowercase()); let mut from = 0;
        while let Some(i) = l[from..].find(&pat) {
            let at = from + i; let end = at + pat.len();
            let before_ok = at == 0 || !l[..at].chars().next_back().map_or(false, |c| c.is_alphanumeric());
            let after_ok = l[end..].chars().next().map_or(true, |c| !(c.is_alphanumeric() || c == '-' || c == '_'));
            if before_ok && after_ok { if !out.contains(m) { out.push(m.clone()) } break }
            from = end;
        }
    }
    out
}

/// A member's short name from a role hint or its model and provider (`opus`, `gemini`, `codex`), unique among `taken` (`-2`, `-3`... added).
pub fn short_name(role: &str, provider: &str, model: &str, taken: &[String]) -> String {
    let clean = |s: &str| s.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect::<String>().to_lowercase();
    let r = clean(role.split_whitespace().next().unwrap_or(""));
    let base = if !r.is_empty() { r } else {
        let m = model.to_lowercase();
        let known = ["opus", "sonnet", "haiku", "fable", "gemini", "grok", "codex", "gpt", "muse", "pi"];
        known.iter().find(|k| m.contains(*k)).map(|k| k.to_string()).unwrap_or_else(|| match provider { "antigravity" => "gemini".into(), "" => "peer".into(), p => clean(p) })
    };
    let base: String = base.chars().take(16).collect();
    if !taken.iter().any(|t| t.eq_ignore_ascii_case(&base)) { return base }
    (2..).map(|i| format!("{base}-{i}")).find(|c| !taken.iter().any(|t| t.eq_ignore_ascii_case(c))).unwrap()
}
/// The member colours, in order of joining (distinct in light and dark; the clients draw them as given).
pub const COLORS: [&str; 8] = ["#3b82f6", "#f97316", "#10b981", "#a855f7", "#ef4444", "#eab308", "#06b6d4", "#ec4899"];
pub fn color(i: usize) -> &'static str { COLORS[i % COLORS.len()] }

#[cfg(test)]
mod tests {
    use super::*;
    fn p(id: &str, m: &str, tags: &[&str], text: &str, at: u64) -> PostMeta { PostMeta { id: id.into(), member: m.into(), tags: tags.iter().map(|t| t.to_string()).collect(), text: text.into(), at, held: false } }
    const L: Limits = LIMITS;
    const NONE: Budget = Budget { posts: 0, turns: 0, tokens: 0 };

    #[test] fn the_rate_guard_refuses_a_seventh_post_in_five_minutes_and_lets_it_through_after() {
        let posts: Vec<PostMeta> = (0..6).map(|i| p(&format!("p{i}"), "thread:a", &["idea"], &format!("thought number {i} about pumps"), 100 + i)).collect();
        assert!(matches!(verdict(&posts, &p("x", "thread:a", &[], "one more", 110), false, 1, &NONE, &NONE, &L), Verdict::Refuse(_)));
        assert_eq!(verdict(&posts, &p("x", "thread:b", &[], "a different member", 110), false, 1, &NONE, &NONE, &L), Verdict::Live, "the rate is per member");
        assert_eq!(verdict(&posts, &p("x", "thread:a", &[], "later on", 100 + 300 + 1), false, 1, &NONE, &NONE, &L), Verdict::Live, "the window moves on");
    }
    #[test] fn the_person_is_never_refused_or_held() {
        let posts: Vec<PostMeta> = (0..20).map(|i| p(&format!("p{i}"), "", &[], "me again", 100)).collect();
        assert_eq!(verdict(&posts, &p("x", "", &[], "steer", 100), true, 99, &NONE, &NONE, &L), Verdict::Live);
    }
    #[test] fn a_paused_room_and_the_backstop_hold() {
        assert_eq!(verdict(&[], &p("x", "thread:a", &[], "hi", 1), true, 1, &NONE, &NONE, &L), Verdict::Hold("paused"));
        assert_eq!(verdict(&[], &p("x", "thread:a", &[], "hi", 1), false, L.backstop + 1, &NONE, &NONE, &L), Verdict::Hold("backstop"));
        assert_eq!(verdict(&[], &p("x", "thread:a", &[], "hi", 1), false, L.backstop, &NONE, &NONE, &L), Verdict::Live);
    }
    #[test] fn chatter_without_claims_pauses_and_a_claim_or_the_person_starts_it_again() {
        let distinct = |i: usize| format!("{} {} {}", ["pumps", "valves", "gears", "belts", "chains", "springs", "levers", "cams", "rods", "pins", "nuts", "bolts", "keys", "hubs"][i % 14], i, ["red", "blue", "green", "gold"][i % 4]);
        let mut posts: Vec<PostMeta> = (0..11).map(|i| p(&format!("p{i}"), if i % 2 == 0 { "thread:a" } else { "thread:b" }, &["idea"], &distinct(i), i as u64 * 100)).collect();
        assert_eq!(verdict(&posts, &p("x", "thread:c", &[], &distinct(11), 2000), false, 1, &NONE, &NONE, &L), Verdict::Pause("chatter"), "the twelfth in a row");
        posts[5].tags = vec!["claim".into()];
        assert_eq!(verdict(&posts, &p("x", "thread:c", &[], &distinct(11), 2000), false, 1, &NONE, &NONE, &L), Verdict::Live, "a claim is progress");
        posts[5].tags.clear(); posts[6].member = String::new();
        assert_eq!(verdict(&posts, &p("x", "thread:c", &[], &distinct(11), 2000), false, 1, &NONE, &NONE, &L), Verdict::Live, "the person's post starts the count again");
    }
    #[test] fn posts_that_repeat_each_other_are_circular_and_new_ideas_are_not() {
        let same = ["I think we should use a hall sensor for the position", "we should use a hall sensor for the position I think", "a hall sensor for the position is what we should use", "I still think we should use a hall sensor for the position"];
        let mut posts: Vec<PostMeta> = vec![]; for i in 0..9 { posts.push(p(&format!("p{i}"), if i % 2 == 0 { "thread:a" } else { "thread:b" }, &["critique"], same[i % 4], i as u64 * 60)) }
        assert!(circular(&posts, &L));
        let fresh: Vec<PostMeta> = (0..9).map(|i| p(&format!("q{i}"), "thread:a", &["idea"], &format!("idea {i}: {}", ["hall sensor", "optical encoder", "a stepper count", "magnetic strip", "linear pot", "capacitive", "ultrasonic", "lidar", "limit switch"][i]), i as u64)).collect();
        assert!(!circular(&fresh, &L), "different ideas are not a loop");
        let mut claimed = posts.clone(); claimed[7].tags = vec!["claim".into()];
        assert!(!circular(&claimed, &L), "a claim in the tail is progress");
        assert!(!circular(&posts[..5], &L), "too few posts to tell");
    }
    #[test] fn a_budget_pauses_at_its_limit() {
        let b = Budget { posts: 3, turns: 0, tokens: 0 };
        assert_eq!(verdict(&[], &p("x", "thread:a", &[], "hi", 1), false, 1, &b, &Budget { posts: 1, ..NONE }, &L), Verdict::Live);
        assert_eq!(verdict(&[], &p("x", "thread:a", &[], "hi", 1), false, 1, &b, &Budget { posts: 2, ..NONE }, &L), Verdict::Pause("budget"));
        assert_eq!(Budget { tokens: 10, ..NONE }.hit(&Budget { tokens: 10, ..NONE }), Some("tokens"));
        assert_eq!(NONE.hit(&Budget { posts: 1_000_000, turns: 9, tokens: 9 }), None, "no budget, no limit");
    }
    #[test] fn batching_gives_a_member_at_most_once_a_window_unless_urgent() {
        assert_eq!(due(0, 100, false, &L), 100, "never heard: at once");
        assert_eq!(due(95, 100, false, &L), 115, "heard 5 s ago: at the window's end");
        assert_eq!(due(95, 100, true, &L), 100, "a mention or the person: at once");
        assert_eq!(due(50, 100, false, &L), 100);
    }
    #[test] fn mentions_are_whole_names_in_any_case() {
        let names = vec![("thread:1".to_string(), "opus".to_string()), ("thread:2".to_string(), "codex".to_string()), ("thread:3".to_string(), "codex-2".to_string())];
        assert_eq!(mentions("@Opus what do you think? cc @codex-2.", &names), vec!["thread:1", "thread:3"]);
        assert_eq!(mentions("mail me at x@opusmail.com", &names), Vec::<String>::new());
        assert_eq!(mentions("@codex, @codex", &names), vec!["thread:2"]);
    }
    #[test] fn short_names_come_from_the_role_or_the_model_and_stay_unique() {
        assert_eq!(short_name("", "claude", "claude-opus-5-5", &[]), "opus");
        assert_eq!(short_name("", "antigravity", "gemini-carbon-4", &[]), "gemini");
        assert_eq!(short_name("", "codex", "", &[]), "codex");
        assert_eq!(short_name("critic and tester", "claude", "x", &[]), "critic");
        assert_eq!(short_name("", "claude", "claude-opus-5-5", &["opus".into(), "opus-2".into()]), "opus-3");
    }
    #[test] fn tags_keep_only_known_ones_once() { assert_eq!(tags_of(&["Claim".into(), "x".into(), "claim".into(), "done".into()]), vec!["claim", "done"]) }
}
