//! The Context panel's sections (core/usage.bend `Ts.sections`, laws in usage tests): what the thread holds in its context now, its last turn and the thread's totals,
//! read from the thread's settings the agents lane writes (`usage.<t>`, `turn.last.<t>`, `tok.<epoch>.t.<t>`), with Claude's speed rows (`turn.speed.<t>`),
//! "This session by model" (`turn.models.<t>`) and the speed note.
use serde_json::{json, Value};

fn part(v: &str, i: usize) -> u64 { v.split('/').nth(i).and_then(|s| s.parse().ok()).unwrap_or(0) }
/// Ts.num: 1234567 as "1,234,567".
pub fn num(n: u64) -> String { let s = n.to_string(); let mut o = String::new(); for (i, c) in s.chars().enumerate() { if i > 0 && (s.len() - i) % 3 == 0 { o.push(',') } o.push(c) } o }
fn pct(part: u64, whole: u64) -> String { format!("{}%", part * 100 / whole.max(1)) }
/// Tk.usd: nano-dollars, to the cent from $1, else to a hundredth of a cent.
pub fn usd(c: u64) -> String { if c >= 1_000_000_000 { format!("${}.{:02}", c / 1_000_000_000, (c % 1_000_000_000) / 10_000_000) } else { format!("$0.{:04}", c / 100_000) } }
fn held(provider: &str, used: u64, window: u64) -> String {
    if used == 0 { if provider == "claude" { "no reading yet".into() } else { format!("not reported by {provider}") } }
    else if window == 0 { format!("{} tokens", num(used)) } else { format!("{} of {} ({})", num(used), num(window), pct(used, window)) }
}
fn tenths(x10: u64) -> String { format!("{}.{}", x10 / 10, x10 % 10) }
/// Ts.secs: milliseconds as "1.4 s" or "2m 05s"
fn secs(ms: u64) -> String { if ms < 60000 { format!("{} s", tenths(ms / 100)) } else { format!("{}m {:02}s", ms / 60000, (ms % 60000) / 1000) } }
/// Ts.rate: output tokens a second, to a tenth
fn rate(out: u64, ms: u64) -> String { format!("{} tok/s", tenths(out * 10000 / ms.max(1))) }
/// Use.tokens: 950, 5.9k, 62k, 1.2M
fn short(n: u64) -> String {
    let t = |x10: u64| if x10 % 10 == 0 { (x10 / 10).to_string() } else { format!("{}.{}", x10 / 10, x10 % 10) };
    if n < 1000 { n.to_string() } else if n < 9950 { t((n + 50) / 100) + "k" } else if n < 999500 { ((n + 500) / 1000).to_string() + "k" } else if n < 9950000 { t((n + 50000) / 100000) + "M" } else { ((n + 500000) / 1000000).to_string() + "M" }
}
fn known(k: &str, p: &str, v: String) -> String { if k.contains(p) { v } else { "—".into() } }
fn sec(title: &str, rows: Vec<(String, String)>) -> Value { json!({"title": title, "rows": rows.into_iter().map(|(l, v)| json!({"label": l, "value": v})).collect::<Vec<_>>()}) }
fn r(l: &str, v: String) -> (String, String) { (l.into(), v) }

/// `usage` is "used/window", `last` the turn's "i/r/w/o/t/c/at/known/provider|model", `tv` the thread's "turns/in/cread/cwrite/out/reasoning/cost/costTurns/knownTurns".
/// `speed` is "turn.speed.<t>" ("/"-parts: 2 model ms, 3 output tokens, 4 turn ms, 5 average ms, 6 average output, 7 steps), `models` "turn.models.<t>" (a line per model:
/// "name|in|out|cacheRead|cacheWrite|cost|webSearches").
pub fn sections(provider: &str, usage: &str, last: &str, tv: &str, speed: &str, models: &str) -> Vec<Value> {
    let (used, window) = (part(usage, 0), part(usage, 1));
    let mut ctx = vec![r("Held now", held(provider, used, window))];
    if window > 0 && window >= used { ctx.push(r("Room left", num(window - used))) }
    let l = if last.is_empty() { vec![r("", "no turn counted yet".into())] } else {
        let kn = last.split('/').nth(7).unwrap_or("");
        let model = last.split('/').skip(8).collect::<Vec<_>>().join("/").split('|').collect::<Vec<_>>().join(" · ");
        let mut rows = vec![r("Model", model.clone())];
        if kn.is_empty() { rows.push(r("Tokens", "not told (the session ran before Backplane counted it)".into())) } else {
            rows.push(r("Input", known(kn, "i", num(part(last, 0))))); rows.push(r("Cache read", known(kn, "r", num(part(last, 1))))); rows.push(r("Cache write", known(kn, "w", num(part(last, 2)))));
            rows.push(r("Output", known(kn, "o", num(part(last, 3))) + &if kn.contains('t') { format!(" ({} reasoning)", num(part(last, 4))) } else { String::new() }));
            rows.push(r("Cost", if kn.contains('c') { usd(part(last, 5)) } else { "not reported".into() }));
        }
        if model.starts_with("claude ·") {
            if part(speed, 2) > 0 { rows.push(r("Output speed", rate(part(speed, 3), part(speed, 2)))); rows.push(r("Model time", secs(part(speed, 2)) + &if part(speed, 7) > 0 { format!(" over {} step{}", part(speed, 7), if part(speed, 7) == 1 { "" } else { "s" }) } else { String::new() })); }
            if part(speed, 4) > 0 { rows.push(r("Turn time", secs(part(speed, 4)))) }
        }
        rows
    };
    let t = if part(tv, 0) == 0 { vec![r("", "no turns counted yet".into())] } else {
        let ins = part(tv, 1) + part(tv, 2) + part(tv, 3);
        let unknown = if part(tv, 8) >= part(tv, 0) { String::new() } else { format!(", tokens unknown for {}", part(tv, 0) - part(tv, 8)) };
        let mut rows = vec![r("Turns", num(part(tv, 0)) + &unknown), r("Input", num(part(tv, 1))), r("Cache read", num(part(tv, 2))), r("Cache write", num(part(tv, 3))),
            r("Output", num(part(tv, 4)) + &if part(tv, 5) == 0 { String::new() } else { format!(" ({} reasoning)", num(part(tv, 5))) })];
        if ins > 0 { rows.push(r("Cache hits", format!("{} of input", pct(part(tv, 2), ins)))) }
        rows.push(r("Cost", if part(tv, 7) == 0 { "cost not reported".into() } else { usd(part(tv, 6)) + &if part(tv, 7) == part(tv, 0) { String::new() } else { format!(" for {} of them", part(tv, 7)) } }));
        if part(speed, 5) > 0 { rows.push(r("Average speed", rate(part(speed, 6), part(speed, 5)))) }
        rows
    };
    let mut out = vec![sec("Context", ctx), sec("Last turn", l), sec("This thread", t)];
    if !models.is_empty() {
        let mut rows = vec![];
        for line in models.split('\n').filter(|l| !l.is_empty()) {
            let ps: Vec<&str> = line.split('|').collect(); let p = |i: usize| ps.get(i).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
            rows.push(r("", format!("{} in · {} out · {} cache read · {} cache write{}", short(p(1)), short(p(2)), short(p(3)), short(p(4)), if p(6) == 0 { String::new() } else { format!(" · {} web searches", p(6)) })));
            rows.push(r(ps.first().copied().unwrap_or(""), usd(p(5))));
        }
        out.push(sec("This session by model", rows));
    }
    if provider == "claude" && part(speed, 5) > 0 { out.push(sec("", vec![r("", "Speed is output tokens over the time Claude's model calls took, as Claude reports it: the wait for the first token is in it (so short replies read slow), tool runs are not.".into())])) }
    out
}

#[cfg(test)] mod tests { use super::*;
    fn flat(v: &[Value]) -> Vec<String> { v.iter().flat_map(|s| s["rows"].as_array().unwrap().iter().map(|r| format!("{}={}", r["label"].as_str().unwrap(), r["value"].as_str().unwrap()))).collect() }
    #[test] fn formats_as_bends_panel_does() {
        assert_eq!((num(7), num(1234), num(1234567), usd(421_000_000), usd(1_200_000_000)), ("7".into(), "1,234".into(), "1,234,567".into(), "$0.4210".into(), "$1.20".into()));
        let s = flat(&sections("claude", "45000/200000", "1/2/3/4/5/6/1793900000/co/claude|default", "2/2/4/6/8/10/12/2/2", "", ""));
        let want = ["Held now=45,000 of 200,000 (22%)", "Room left=155,000", "Model=claude · default", "Input=—", "Cache read=—", "Cache write=—", "Output=4", "Cost=$0.0000",
            "Turns=2", "Input=2", "Cache read=4", "Cache write=6", "Output=8 (10 reasoning)", "Cache hits=33% of input", "Cost=$0.0000"];
        assert_eq!(s, want.iter().map(|x| x.to_string()).collect::<Vec<_>>());
    }
    #[test] fn claudes_speed_the_models_of_a_session_and_the_note_are_drawn() {
        // speed: 1.4 s of model time over 2 steps for 70 output tokens, a 9.5 s turn; average 70 tokens over 1.4 s per model call
        let s = flat(&sections("claude", "", "1/2/3/4/5/6/1793900000/co/claude|default", "2/2/4/6/8/10/12/2/2", "0/0/1400/70/9500/1400/70/2", "claude-opus|1200|3400|56000|2100|421000000|2\nclaude-haiku|10|20|0|0|0|0"));
        for want in ["Output speed=50.0 tok/s", "Model time=1.4 s over 2 steps", "Turn time=9.5 s", "Average speed=50.0 tok/s", "=1.2k in · 3.4k out · 56k cache read · 2.1k cache write · 2 web searches", "claude-opus=$0.4210", "claude-haiku=$0.0000"] { assert!(s.contains(&want.to_string()), "{want} in {s:?}"); }
        assert!(s.last().unwrap().starts_with("=Speed is output tokens"), "{s:?}");
        let v = sections("claude", "", "1/2/3/4/5/6/1793900000/co/claude|default", "", "0/0/1400/70/9500/1400/70/2", "claude-opus|1|2|3|4|5|0"); assert_eq!(v.iter().map(|x| x["title"].as_str().unwrap()).collect::<Vec<_>>(), ["Context", "Last turn", "This thread", "This session by model", ""]);
        // not Claude: no speed rows, no note
        let c = flat(&sections("codex", "", "1/2/3/4/5/6/1793900000/co/codex|gpt", "", "0/0/1400/70/9500/1400/70/2", "")); assert!(!c.iter().any(|x| x.starts_with("Output speed") || x.starts_with("Turn time")), "{c:?}");
    }
    #[test] fn nothing_yet_says_so() {
        let s = flat(&sections("codex", "", "", "", "", ""));
        assert_eq!(s, ["Held now=not reported by codex", "=no turn counted yet", "=no turns counted yet"]);
        assert_eq!(flat(&sections("claude", "", "", "", "", ""))[0], "Held now=no reading yet");
    }
}
