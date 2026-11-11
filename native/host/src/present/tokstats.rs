//! The composer's token chip and the Context panel behind it (`usage`): what the thread holds in its context now, what its turns cost and, for a bot's chat,
//! what feeds each turn and what its side lanes ran. Read from the settings the agents lane and the fold write (`usage.<t>`, `turn.last.<t>`, `turn.live.<t>`,
//! `tok.<epoch>.t.<t>`, `.td.` the thread's day, `.tr.` its last turns, `.bl.` a mailroom lane's day; `turn.speed.<t>`, `turn.models.<t>`).
use serde_json::{json, Value};

fn part(v: &str, i: usize) -> u64 { v.split('/').nth(i).and_then(|s| s.parse().ok()).unwrap_or(0) }
/// Ts.num: 1234567 as "1,234,567".
pub fn num(n: u64) -> String { let s = n.to_string(); let mut o = String::new(); for (i, c) in s.chars().enumerate() { if i > 0 && (s.len() - i) % 3 == 0 { o.push(',') } o.push(c) } o }
fn pct(part: u64, whole: u64) -> String { format!("{}%", part * 100 / whole.max(1)) }
/// Tk.usd: nano-dollars, to the cent from $1, else to a hundredth of a cent.
pub fn usd(c: u64) -> String { if c >= 1_000_000_000 { format!("${}.{:02}", c / 1_000_000_000, (c % 1_000_000_000) / 10_000_000) } else { format!("$0.{:04}", c / 100_000) } }
fn tenths(x10: u64) -> String { format!("{}.{}", x10 / 10, x10 % 10) }
/// Ts.secs: milliseconds as "1.4 s" or "2m 05s"
fn secs(ms: u64) -> String { if ms < 60000 { format!("{} s", tenths(ms / 100)) } else { format!("{}m {:02}s", ms / 60000, (ms % 60000) / 1000) } }
/// Ts.rate: output tokens a second, to a tenth
fn rate(out: u64, ms: u64) -> String { format!("{} tok/s", tenths(out * 10000 / ms.max(1))) }
/// Use.tokens: 950, 5.9k, 62k, 1.2M
pub fn short(n: u64) -> String {
    let t = |x10: u64| if x10 % 10 == 0 { (x10 / 10).to_string() } else { format!("{}.{}", x10 / 10, x10 % 10) };
    if n < 1000 { n.to_string() } else if n < 9950 { t((n + 50) / 100) + "k" } else if n < 999500 { ((n + 500) / 1000).to_string() + "k" } else if n < 9950000 { t((n + 50000) / 100000) + "M" } else { ((n + 500000) / 1000000).to_string() + "M" }
}

/// The composer's token button (0.14's Ts.chip): the thread's tokens, its cost and the context held, each only once known ("18k tok · $0.0123 · ctx 9%"), "" for none.
/// `tv` and `usage` as in `sections`; `live` the running turn's "in/cread/cwrite/out/cost" so far (agents/lines.rs, "" when none), counted in at once:
/// its cost is an estimate ("~$0.0150"), or "+" after the counted cost while no result has priced the turn's model yet.
pub fn chip(usage: &str, tv: &str, live: &str) -> String {
    let (used, window) = (part(usage, 0), part(usage, 1));
    let lt = part(live, 0) + part(live, 1) + part(live, 2) + part(live, 3);
    let lc = live.split('/').nth(4).filter(|c| !c.is_empty()).map(|_| part(live, 4));
    let mut v = vec![];
    if part(tv, 0) > 0 || lt > 0 { v.push(format!("{} tok", short(part(tv, 1) + part(tv, 2) + part(tv, 3) + part(tv, 4) + lt))) }
    if part(tv, 7) > 0 || lc.is_some() { v.push(match (lt > 0, lc) { (true, Some(c)) => format!("~{}", money(part(tv, 6) + c)), (true, None) => format!("{}+", money(part(tv, 6))), _ => money(part(tv, 6)) }) }
    if used > 0 { v.push(if window == 0 { format!("{} ctx", short(used)) } else { format!("ctx {}", pct(used, window)) }) }
    v.join(" · ")
}

/// Use.human: 4, 950, 4.9k, 95.3k, 312k, 2.24M, 12.3M (the panel's figures; the exact count is in the tip)
pub fn human(n: u64) -> String {
    let d = |x: f64, p: usize| { let s = format!("{x:.p$}"); if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s } };
    let f = n as f64;
    if n < 1000 { n.to_string() } else if n < 99_950 { d(f / 1e3, 1) + "k" } else if n < 999_500 { d(f / 1e3, 0) + "k" } else if n < 9_995_000 { d(f / 1e6, 2) + "M" } else { d(f / 1e6, 1) + "M" }
}
/// Model.nice: "claude-fable-5-2[1m]" as ("Fable 5.2", "1M"): the family and version, and the window a `[..]` suffix names
pub fn model_nice(model: &str) -> (String, String) {
    let m = model.split('|').last().unwrap_or(model);
    let (base, tag) = match m.find('[') { Some(i) => (&m[..i], m[i + 1..].trim_end_matches(']')), None => (m, "") };
    (if base.is_empty() || base == "default" { "Default model".into() } else { super::keys::model_short(base) }, tag.to_uppercase())
}
fn ago(now: u64, at: u64) -> String { let s = now.saturating_sub(at); match s { 0..=89 => "just now".into(), 90..=5399 => format!("{} min ago", (s + 30) / 60), 5400..=129_599 => format!("{} h ago", (s + 1800) / 3600), _ => { let d = (s + 43_200) / 86_400; format!("{d} day{} ago", if d == 1 { "" } else { "s" }) } } }
/// Use.money: the panel's cost, to the cent from 10 cents (the exact figure is in its tip), else as `usd`
pub fn money(c: u64) -> String { if c >= 100_000_000 { let cents = (c + 5_000_000) / 10_000_000; format!("${}.{:02}", cents / 100, cents % 100) } else { usd(c) } }

/// A bot's chat, for its panel: what feeds each of its turns and what ran beside it. Only what is true of this bot is filled in.
#[derive(Default)]
pub struct Bot {
    pub name: String,
    /// the uberbot: mail, routines and reports are triaged in lanes of their own
    pub uber: bool,
    /// the mailroom files its routines (`mail.on`, or the uberbot): they do not run in this chat
    pub routines_filed: bool,
    /// its memories: (kind, characters of its <memory> line)
    pub mems: Vec<(String, u64)>,
    /// handovers waiting for its next turn: (why, text)
    pub waiting: Vec<(String, String)>,
    /// its lanes' day ("<day>|<totals>|<provider>|<account id>" as the fold keeps it, `tok.bl.`) with the account's name: (lane, value, account)
    pub lanes: Vec<(String, String, String)>,
}
/// What the panel reads for one thread (each a setting's text as the agents lane or the fold wrote it).
#[derive(Default)]
pub struct In<'a> {
    pub provider: &'a str, pub model: &'a str, pub usage: &'a str, pub last: &'a str, pub tv: &'a str, pub speed: &'a str, pub models: &'a str,
    /// the running turn's numbers so far ("" when no turn runs)
    pub live: &'a str,
    /// the thread's last turns ("tok/cost/ctx ..." `tok.tr.`) and its day ("<day>|<totals>|" `tok.td.`), and today's day
    pub ring: &'a str, pub today: &'a str, pub day: &'a str,
    /// when its session last began afresh, and when it was last handed the conversation and why (0: never)
    pub began: u64, pub handed: (u64, &'a str), pub now: u64, pub running: bool,
    pub bot: Option<Bot>,
}
fn totals(tv: &str) -> (u64, u64) { (part(tv, 1) + part(tv, 2) + part(tv, 3) + part(tv, 4), part(tv, 6)) }
/// Tone.ctx: how a context reading reads as it fills ("" fine, "warn" from 60%, "bad" from 85%)
pub fn ctx_tone(pct: u64) -> &'static str { if pct >= 85 { "bad" } else if pct >= 60 { "warn" } else { "" } }
/// Ctx.runway: whole turns left at the context's recent growth (the last eight readings after a turn), None when it does not grow
fn runway(ring: &str, used: u64, window: u64) -> Option<u64> {
    let xs: Vec<u64> = ring.split(' ').filter(|x| !x.is_empty()).map(|x| part(x, 2)).filter(|x| *x > 0).collect();
    let xs = &xs[xs.len().saturating_sub(8)..];
    // the context shrank since (a new session, a compaction): the past rate says nothing about it
    if xs.len() < 3 || window <= used || xs.last().map_or(false, |l| used < *l) { return None }
    let (a, b) = (xs[0], xs[xs.len() - 1]);
    if b <= a { return None }
    let grow = (b - a) / (xs.len() as u64 - 1);
    (grow > 0).then(|| (window - used) / grow)
}
fn parts_of(i: u64, r: u64, w: u64, o: u64, kn: &str) -> Value {
    let all = (i + r + w + o).max(1) as f64;
    let p = |key: &str, label: &str, n: u64, k: char| json!({"key": key, "label": label, "short": if kn.is_empty() || kn.contains(k) { human(n) } else { "—".into() }, "exact": format!("{} tokens", num(n)), "frac": n as f64 / all});
    json!([p("input", "Input", i, 'i'), p("cread", "Cache read", r, 'r'), p("cwrite", "Cache write", w, 'w'), p("output", "Output", o, 'o')])
}
/// The Context panel (the composer's token chip opens it): a picture of the thread's numbers for the client to draw as it is. `kind` "thread" or "bot";
/// `ctx` the context meter (null when the provider has not said); `turns` this turn so far and the last one, each split four ways; `totals` the thread's
/// (and its day's) tokens and cost with a sparkline of its last turns; for a bot, `session` and `feeds` (what each turn starts with) and `lanes` (what ran beside
/// the chat today, each with its account); `actions` what the panel offers. Words with an exact figure carry it in `tip`.
pub fn usage(x: &In) -> Value {
    let (used, window) = (part(x.usage, 0), part(x.usage, 1));
    let last_model = x.last.split('/').skip(8).collect::<Vec<_>>().join("/");
    let (name, tag) = model_nice(if x.model.is_empty() && !last_model.is_empty() { &last_model } else { x.model });
    let win_words = if window > 0 { format!("{} context", human(window)) } else if !tag.is_empty() { format!("{tag} context") } else { String::new() };
    let ctx = if used == 0 { Value::Null } else if window == 0 {
        json!({"pct": null, "frac": 0.0, "tone": "", "big": human(used), "line": "tokens held", "tip": format!("{} tokens held; {} does not say its window", num(used), x.provider), "hint": ""})
    } else {
        let pc = used * 100 / window;
        let hint = match runway(x.ring, used, window) { Some(0) => "nearly full at this rate".to_string(), Some(1) => "about 1 more turn at this rate".into(), Some(n) if n < 50 => format!("about {n} more turns at this rate"), Some(_) => "50+ more turns at this rate".into(), None => String::new() };
        json!({"pct": pc, "frac": (used as f64 / window as f64).min(1.0), "tone": ctx_tone(pc), "big": format!("{pc}%"), "line": format!("{} of {}", human(used), human(window)),
            "tip": format!("{} of {} tokens held · {} left", num(used), num(window), num(window - used)), "hint": hint, "room": format!("{} left", human(window.saturating_sub(used)))})
    };
    let ctx_note = if used > 0 { String::new() } else if x.provider == "claude" { "No reading yet: the first turn's result reports it.".into() } else { format!("{} does not report its context.", x.provider) };
    // this turn so far, then the last one
    let mut turns = vec![];
    let lv = [part(x.live, 0), part(x.live, 1), part(x.live, 2), part(x.live, 3)];
    if x.running && lv.iter().sum::<u64>() > 0 {
        let cost = match x.live.split('/').nth(4).filter(|c| !c.is_empty()) { Some(_) => format!("~{}", money(part(x.live, 4))), None => String::new() };
        turns.push(json!({"title": "This turn so far", "live": true, "parts": parts_of(lv[0], lv[1], lv[2], lv[3], ""), "total": human(lv.iter().sum()), "totalTip": format!("{} tokens", num(lv.iter().sum())),
            "cost": cost, "costTip": if cost.is_empty() { "Priced when the turn ends".to_string() } else { "An estimate at this session's rate for the model, until the turn's result prices it".into() }, "meta": ""}));
    }
    if !x.last.is_empty() {
        let kn = x.last.split('/').nth(7).unwrap_or("");
        let (i, r, w, o) = (part(x.last, 0), part(x.last, 1), part(x.last, 2), part(x.last, 3));
        let mut meta = vec![model_nice(&last_model).0];
        if last_model.starts_with("claude|") && part(x.speed, 2) > 0 { meta.push(rate(part(x.speed, 3), part(x.speed, 2))); }
        if part(x.speed, 4) > 0 { meta.push(secs(part(x.speed, 4))) }
        meta.push(ago(x.now, part(x.last, 6)));
        let cost = if kn.contains('c') { money(part(x.last, 5)) } else { String::new() };
        if kn.is_empty() { turns.push(json!({"title": "Last turn", "live": false, "parts": [], "total": "", "cost": "", "meta": "Not counted: the session ran before Backplane counted turns"})) }
        else { turns.push(json!({"title": "Last turn", "live": false, "parts": parts_of(i, r, w, o, kn), "total": human(i + r + w + o), "totalTip": format!("{} tokens{}", num(i + r + w + o), if part(x.last, 4) > 0 { format!(" ({} reasoning)", num(part(x.last, 4))) } else { String::new() }),
            "cost": cost, "costTip": format!("{}: what the agent reported, at the model's API rates", usd(part(x.last, 5))), "meta": meta.join(" · ")})) }
    }
    // the thread's totals, with its day and its last turns
    let mut tots = vec![];
    if part(x.tv, 0) > 0 {
        let (tok, cost) = totals(x.tv); let ins = part(x.tv, 1) + part(x.tv, 2) + part(x.tv, 3);
        let mut sub = vec![format!("{} turn{}", num(part(x.tv, 0)), if part(x.tv, 0) == 1 { "" } else { "s" })];
        if ins > 0 && part(x.tv, 2) > 0 { sub.push(format!("{} cache hits", pct(part(x.tv, 2), ins))) }
        let ring: Vec<u64> = x.ring.split(' ').filter(|s| !s.is_empty()).map(|s| part(s, 0)).collect();
        let top = ring.iter().copied().max().unwrap_or(0).max(1) as f64;
        let spark: Vec<f64> = if ring.len() >= 3 { ring.iter().map(|v| *v as f64 / top).collect() } else { vec![] };
        tots.push(json!({"title": if x.bot.is_some() { "This chat" } else { "This thread" }, "tokens": human(tok), "cost": if part(x.tv, 7) == 0 { String::new() } else { money(cost) }, "sub": sub.join(" · "),
            "tip": format!("{} tokens: {} input, {} cache read, {} cache write, {} output{}", num(tok), num(part(x.tv, 1)), num(part(x.tv, 2)), num(part(x.tv, 3)), num(part(x.tv, 4)), if part(x.tv, 8) < part(x.tv, 0) { format!("; tokens unknown for {} turns", part(x.tv, 0) - part(x.tv, 8)) } else { String::new() }),
            "spark": spark, "sparkTip": format!("Tokens of the last {} turns", ring.len())}));
    }
    let day_of = |v: &str| -> Option<String> { let mut it = v.splitn(3, '|'); (it.next() == Some(x.day)).then(|| it.next().unwrap_or("").to_string()) };
    let today_main = day_of(x.today).unwrap_or_default();
    // a bot: its session, what each turn starts with, and its lanes
    let (mut session, mut feeds, mut lanes, mut lanes_note) = (vec![], vec![], vec![], String::new());
    let mut today_all = (totals(&today_main), part(&today_main, 0));
    if let Some(b) = &x.bot {
        if x.began > 0 { session.push(json!({"label": "Session", "value": format!("since {}", ago(x.now, x.began)), "tip": "When this chat's agent session last changed: a new one, or another account's or agent's taken up"})) }
        if x.handed.0 > 0 { session.push(json!({"label": "Last handover", "value": format!("{} · {}", x.handed.1, ago(x.now, x.handed.0)), "tip": "The conversation was handed to the session with the next message (at most 128 items)"})) }
        let always: Vec<&(String, u64)> = b.mems.iter().filter(|m| m.0 == "user" || m.0 == "pref").collect();
        let ach: u64 = always.iter().map(|m| m.1 + 1).sum::<u64>().min(2400);
        if !b.mems.is_empty() {
            feeds.push(json!({"label": "Memory", "value": format!("{} always + 6 matched", always.len()), "frac": ach as f64 / 2400.0, "meter": format!("{} / 2.4k", human(ach)), "sub": "",
                "tip": format!("Each turn starts with a <memory> block: every user and pref memory ({} characters here), then the six of the other {} saved that best match the message, as many as fit 2,400 characters", num(ach), b.mems.len() - always.len())}));
        }
        let prefs: Vec<&(String, u64)> = b.mems.iter().filter(|m| m.0 == "pref").collect();
        if b.uber && !prefs.is_empty() {
            let pc: u64 = prefs.iter().map(|m| m.1 + 1).sum::<u64>().min(6000);
            feeds.push(json!({"label": "Preferences", "value": format!("{} rule{}", prefs.len(), if prefs.len() == 1 { "" } else { "s" }), "frac": pc as f64 / 6000.0, "meter": format!("{} / 6k", human(pc)), "sub": "back office",
                "tip": format!("The person's standing rules (pref memories, {} characters) go to each back-office turn as <owner_preferences>, up to 6,000 characters", num(pc))}));
        }
        if b.waiting.is_empty() { feeds.push(json!({"label": "Handover", "value": "none waiting", "frac": null, "meter": "", "sub": "", "tip": "A switch of account or agent hands the conversation the session has not seen to its next turn"})) }
        else {
            let n: usize = b.waiting.iter().map(|w| w.1.split("\n\n").count().saturating_sub(2)).sum(); let ch: u64 = b.waiting.iter().map(|w| w.1.chars().count() as u64).sum();
            feeds.push(json!({"label": "Handover", "value": format!("{n} item{} waiting", if n == 1 { "" } else { "s" }), "frac": null, "meter": format!("~{} tok", human(ch / 4)), "sub": b.waiting[0].0, "tip": "Goes with the next message, at most 128 items"}));
        }
        for (lane, v, acct) in &b.lanes {
            let Some(t) = day_of(v) else { continue };
            let (tok, cost) = totals(&t);
            if part(&t, 0) == 0 { continue }
            let label = if lane == "office" { "Back office" } else if b.uber { "Mailroom triage" } else { "Mail runs" };
            let tip = if lane == "office" { "Private coordination turns: they settle mail and reports and tell you what matters" } else { "A run of its own for each piece of mail, routine or report, triaged in its own session" };
            lanes.push(json!({"name": label, "tokens": human(tok), "cost": if part(&t, 7) == 0 { String::new() } else { money(cost) }, "sub": format!("{} turn{}", part(&t, 0), if part(&t, 0) == 1 { "" } else { "s" }), "account": acct, "tip": format!("{tip}. {} tokens, {}", num(tok), usd(cost))}));
            today_all = ((today_all.0 .0 + tok, today_all.0 .1 + cost), today_all.1 + part(&t, 0));
        }
        lanes_note = if b.routines_filed { "Mail, routines and reports run in sessions of their own: they never fill this chat's context.".into() } else { "Routines run as turns of this chat.".into() };
    }
    // a thread's day says something only when the thread is older than it
    if today_all.1 > 0 && (x.bot.is_some() || today_all.1 < part(x.tv, 0)) {
        let ((tok, cost), n) = today_all;
        tots.push(json!({"title": "Today", "tokens": human(tok), "cost": money(cost), "sub": format!("{n} turn{}{}", if n == 1 { "" } else { "s" }, if x.bot.is_some() && !lanes.is_empty() { " · chat and lanes" } else { "" }),
            "tip": format!("{} tokens since 00:00 UTC", num(tok)), "spark": [], "sparkTip": ""}));
    }
    // more than one model this session: each one's share
    let mut models = vec![];
    for line in x.models.split('\n').filter(|l| !l.is_empty()) {
        let ps: Vec<&str> = line.split('|').collect(); let p = |i: usize| ps.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        models.push(json!({"name": model_nice(ps.first().copied().unwrap_or("")).0, "tokens": human(p(1) + p(2) + p(3) + p(4)), "cost": money(p(5)),
            "tip": format!("{} in · {} out · {} cache read · {} cache write{}", num(p(1)), num(p(2)), num(p(3)), num(p(4)), if p(6) == 0 { String::new() } else { format!(" · {} web searches", p(6)) })}));
    }
    if models.len() < 2 { models.clear() }
    let mut actions = vec![];
    if x.provider == "claude" && used > 0 && !x.running { actions.push(json!({"id": "compact", "label": "Compact", "tip": "Summarize the conversation to free context (/compact)"})) }
    actions.push(json!({"id": "model", "label": "Switch model", "tip": "Pick another model or agent for this thread"}));
    json!({"kind": if x.bot.is_some() { "bot" } else { "thread" }, "model": name, "window": win_words, "provider": x.provider, "ctx": ctx, "ctxNote": ctx_note,
        "session": session, "feeds": feeds, "turns": turns, "totals": tots, "lanes": lanes, "lanesNote": lanes_note,
        "models": models, "note": "Costs are estimates at the model's API rates.", "actions": actions})
}

#[cfg(test)] mod tests { use super::*;
    #[test] fn the_token_button_says_tokens_cost_and_context_once_known() {
        assert_eq!(chip("45000/200000", "2/2000/14000/1000/1000/10/12300000/2/2", ""), "18k tok · $0.0123 · ctx 22%");
        assert_eq!(chip("", "", ""), ""); assert_eq!(chip("1200/0", "", ""), "1.2k ctx"); assert_eq!(chip("", "1/10/0/0/5/0/0/0/1", ""), "15 tok", "no cost told: none shown");
    }
    #[test] fn a_running_turn_counts_in_at_once_its_cost_estimated_or_marked_open() {
        let tv = "2/2000/14000/1000/1000/10/12300000/2/2";
        assert_eq!(chip("45000/200000", tv, "10/20000/3000/2000/2700000"), "43k tok · ~$0.0150 · ctx 22%");
        assert_eq!(chip("45000/200000", tv, "10/20000/3000/2000/"), "43k tok · $0.0123+ · ctx 22%", "no rate for the model yet: the counted cost and more to come");
        assert_eq!(chip("", "", "10/0/0/5/"), "15 tok", "a first turn shows its tokens before its result");
    }
    #[test] fn the_chip_and_the_panel_say_a_cost_alike() { assert_eq!(chip("", "1/10/0/0/5/0/880600000/1/1", ""), "15 tok · $0.88"); assert_eq!((money(96_600_000), money(1_555_000_000)), ("$0.0966".to_string(), "$1.56".to_string())); }
    #[test] fn formats() {
        assert_eq!((num(7), num(1234), num(1234567), usd(421_000_000), usd(1_200_000_000)), ("7".into(), "1,234".into(), "1,234,567".into(), "$0.4210".into(), "$1.20".into()));
    }
    #[test] fn figures_read_short_and_models_by_name() {
        assert_eq!([4, 950, 4854, 95292, 312_400, 2_240_000, 12_340_000].map(human), ["4", "950", "4.9k", "95.3k", "312k", "2.24M", "12.3M"]);
        assert_eq!(human(100_157), "100k"); assert_eq!(human(1_000_000), "1M");
        assert_eq!(model_nice("claude-fable-5-2[1m]"), ("Fable 5.2".to_string(), "1M".to_string())); assert_eq!(model_nice("claude|claude-opus-5-5").0, "Opus 5.5"); assert_eq!(model_nice("").0, "Default model");
    }
    fn thread<'a>(usage: &'a str, ring: &'a str) -> In<'a> {
        In { provider: "claude", model: "claude-fable-5-2[1m]", usage, last: "4/95292/4854/7/0/96600000/1793900000/iorwco/claude|claude-fable-5-2[1m]", tv: "143/1000/2000000/150000/50000/0/2380000000/143/143",
            ring, today: "2026-11-05|3/10/20/30/40/0/5000000/3/3|", day: "2026-11-05", now: 1_793_900_100, ..Default::default() }
    }
    #[test] fn a_threads_panel_has_one_meter_its_turns_and_totals() {
        let v = usage(&thread("100157/1000000", "10/1/90000 20/1/95000 30/1/100157"));
        assert_eq!((v["kind"].as_str(), v["model"].as_str(), v["window"].as_str()), (Some("thread"), Some("Fable 5.2"), Some("1M context")));
        assert_eq!((v["ctx"]["big"].as_str(), v["ctx"]["line"].as_str(), v["ctx"]["tone"].as_str()), (Some("10%"), Some("100k of 1M"), Some("")));
        assert_eq!(v["ctx"]["tip"], "100,157 of 1,000,000 tokens held · 899,843 left");
        assert_eq!(v["ctx"]["hint"], "50+ more turns at this rate", "growing 5k a turn: 177 turns of room");
        let t = &v["turns"][0]; assert_eq!(t["title"], "Last turn"); assert_eq!(t["parts"].as_array().unwrap().iter().map(|p| p["short"].as_str().unwrap()).collect::<Vec<_>>(), ["4", "95.3k", "4.9k", "7"]);
        assert_eq!(t["parts"][1]["exact"], "95,292 tokens"); assert_eq!(t["cost"], "$0.0966");
        assert_eq!(v["totals"][0]["title"], "This thread"); assert_eq!((v["totals"][0]["tokens"].as_str(), v["totals"][0]["cost"].as_str()), (Some("2.2M"), Some("$2.38")));
        assert_eq!(v["totals"][0]["spark"].as_array().unwrap().len(), 3); assert_eq!(v["totals"][1]["title"], "Today");
        assert!(v["session"].as_array().unwrap().is_empty() && v["feeds"].as_array().unwrap().is_empty() && v["lanes"].as_array().unwrap().is_empty(), "a thread has no bot parts");
        assert_eq!(v["actions"][0]["id"], "compact");
        // near full: red, with the turns it has left
        let v = usage(&thread("930000/1000000", "1/1/870000 1/1/900000 1/1/930000")); assert_eq!((v["ctx"]["tone"].as_str(), v["ctx"]["hint"].as_str()), (Some("bad"), Some("about 2 more turns at this rate")));
        let v = usage(&thread("620000/1000000", "")); assert_eq!(v["ctx"]["tone"], "warn"); assert_eq!(v["ctx"]["hint"], "", "no growth known: no guess");
        let v = usage(&In { provider: "codex", ..Default::default() }); assert!(v["ctx"].is_null()); assert_eq!(v["ctxNote"], "codex does not report its context."); assert!(v["turns"].as_array().unwrap().is_empty());
    }
    #[test] fn a_running_turn_is_an_estimate() {
        let mut x = thread("100157/1000000", ""); x.running = true; x.live = "10/20000/3000/2000/2700000";
        let v = usage(&x); assert_eq!(v["turns"][0]["title"], "This turn so far"); assert_eq!(v["turns"][0]["cost"], "~$0.0027"); assert_eq!(v["turns"][1]["title"], "Last turn");
        assert!(v["actions"].as_array().unwrap().iter().all(|a| a["id"] != "compact"), "no compacting while a turn runs");
    }
    #[test] fn a_bots_panel_says_what_feeds_its_turns_and_its_lanes() {
        let mut x = thread("100157/1000000", ""); x.began = 1_793_890_000; x.handed = (1_793_800_000, "account switch");
        x.bot = Some(Bot { name: "Bub".into(), uber: true, routines_filed: true, mems: vec![("user".into(), 100), ("pref".into(), 300), ("fact".into(), 900)], waiting: vec![],
            lanes: vec![("runs".into(), "2026-11-05|12/100000/200000/0/5000/0/840000000/12/12|claude|".into(), "default login".into()), ("office".into(), "2026-11-04|1/1/1/1/1/0/1/1/1|claude|a1".into(), "kelly".into())] });
        let v = usage(&x);
        assert_eq!(v["kind"], "bot"); assert_eq!(v["totals"][0]["title"], "This chat");
        assert_eq!(v["session"][0]["value"], "since 3 h ago"); assert_eq!(v["session"][1]["value"], "account switch · 28 h ago");
        let f: Vec<String> = v["feeds"].as_array().unwrap().iter().map(|f| format!("{}={}", f["label"].as_str().unwrap(), f["value"].as_str().unwrap())).collect();
        assert_eq!(f, ["Memory=2 always + 6 matched", "Preferences=1 rule", "Handover=none waiting"]);
        assert_eq!(v["feeds"][0]["meter"], "402 / 2.4k"); assert!(v["feeds"][0]["tip"].as_str().unwrap().contains("the other 1 saved"));
        let l = v["lanes"].as_array().unwrap(); assert_eq!(l.len(), 1, "yesterday's back office is not today's: {l:?}"); assert_eq!((l[0]["name"].as_str(), l[0]["tokens"].as_str(), l[0]["account"].as_str()), (Some("Mailroom triage"), Some("305k"), Some("default login")));
        assert_eq!(v["totals"][1]["sub"], "15 turns · chat and lanes"); assert!(v["lanesNote"].as_str().unwrap().contains("never fill this chat's context"));
    }
}
