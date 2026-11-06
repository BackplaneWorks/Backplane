//! What a routine says of itself in words, in English: the agents' tools keep the
//! English text whatever the person's language. Pure.
use super::clock::{Cron, Plan, TzRule, Zones};
use super::routines::machine_off;
use super::world::Routine;

/// Sched.dur: "3h 5m", "2d 4h", "under a minute"
pub fn dur(d: u64) -> String {
    let pair = |a: u64, ua: &str, b: u64, ub: &str| format!("{a}{ua}{}", if b == 0 { String::new() } else { format!(" {b}{ub}") });
    if d < 60 { "under a minute".into() } else if d < 3600 { format!("{}m", d / 60) } else if d < 86400 { pair(d / 3600, "h", d % 3600 / 60, "m") } else { pair(d / 86400, "d", d % 86400 / 3600, "h") }
}
/// Sched.rel: "in 3h 5m", "due now"; "" for a time of 0
pub fn rel(now: u64, t: u64) -> String { if t == 0 { String::new() } else if t <= now { "due now".into() } else { format!("in {}", dur(t - now)) } }
fn gap(xs: &[u64]) -> u64 { match xs { [a, b, ..] => if *a == 0 { *b } else { 1 }, _ => 1 } }
fn hm(h: u64, m: u64) -> String { format!("{h:02}:{m:02}") }
fn vals_step(n: u64, hi: u64) -> Vec<u64> { (0..=hi).filter(|v| v % n == 0).collect() }
/// Cron.describe: the words for a cron line, else None
fn describe(c: &Cron) -> Option<String> {
    let all = c.dom_any && c.dow_any && c.months.len() == 12; let allh = c.hours.len() == 24;
    let (n, k) = (gap(&c.mins), gap(&c.hours)); let (m, h) = (c.mins.first().copied().unwrap_or(0), c.hours.first().copied().unwrap_or(0));
    if all && allh && n > 0 && 60 % n == 0 && c.mins == vals_step(n, 59) { return Some(if n == 1 { "every minute".into() } else { format!("every {n} minutes") }) }
    if all && allh && c.mins.len() == 1 { return Some(if m == 0 { "hourly".into() } else { format!("hourly at :{m:02}") }) }
    if all && c.mins == [0] && k > 0 && 24 % k == 0 && c.hours == vals_step(k, 23) { return Some(if k == 1 { "every hour".into() } else { format!("every {k} hours") }) }
    if all && c.mins.len() == 1 && c.hours.len() == 1 { return Some(format!("daily at {}", hm(h, m))) }
    if c.dom_any && !c.dow_any && c.months.len() == 12 && c.dows == [1, 2, 3, 4, 5] && c.mins.len() == 1 && c.hours.len() == 1 { return Some(format!("weekdays at {}", hm(h, m))) }
    None
}
/// Sched.when.cron: the words for a cron line, else the line itself
pub fn when_cron(cron: &str) -> String { match Cron::parse(cron) { Some(c) => describe(&c).unwrap_or_else(|| Cron::text(cron)), None => cron.to_string() } }
/// Sched.kind
pub fn kind(kind: &str, once: u64) -> &'static str { if kind == "remind" { "reminder" } else if once > 0 { "once" } else { "routine" } }

/// Sched.person: the person's zone as a rule, else the machine's offset
pub fn person_rule(z: &Zones, now: u64) -> TzRule { z.rule(&z.person()).unwrap_or_else(|| { let (o, east) = machine_off(z, now); TzRule::fixed(east, o) }) }
/// Zone.source: where the person's zone comes from
pub fn zone_source(z: &Zones) -> &'static str {
    let g = |k: &str| z.settings.get(k).map_or(true, |v| v.is_empty());
    if !g("person.tz") { "set" } else if !g("person.tz.device") { "phone" } else if !g("tz.machine") { "machine" } else { "utc" }
}
fn plan_of(r: &Routine) -> Plan<'_> { Plan { cron: &r.cron, on: r.on, deleted: r.deleted, done: r.done, last: r.last, once: r.once, tz: &r.tz, slot: r.slot } }

/// the parts of Sched.row the tools use, for a routine as it stands at `now`
pub struct Row { pub kind: &'static str, pub when: String, pub next_text: String, pub rel: String, pub done: bool, pub next: u64 }
pub fn row(z: &Zones, r: &Routine, now: u64) -> Row {
    let (mo, me) = machine_off(z, now); let nx = plan_of(r).next(z, now, mo, me); let rule = person_rule(z, now);
    let k = kind(&r.kind, r.once); let done = r.done;
    let when = if r.once > 0 { rule.show(r.once) } else { when_cron(&r.cron) } + &if done || r.once == 0 || nx == 0 { String::new() } else { format!(" · {}", rel(now, nx)) };
    Row { kind: k, when, next_text: if done || r.once > 0 || nx == 0 { String::new() } else { rule.show(nx) }, rel: if done { String::new() } else { rel(now, nx) }, done, next: nx }
}
fn iso(rule: &TzRule, t: u64) -> String { if t == 0 { String::new() } else { rule.iso(t) } }
/// Bots.routine.json: the routine as the tools tell it
pub fn routine_json(z: &Zones, r: &Routine, now: u64) -> serde_json::Value {
    let w = row(z, r, now); let rule = person_rule(z, now);
    serde_json::json!({"id":r.id,"name":r.name,"kind":w.kind,"schedule":r.cron,"when":w.when,"prompt":r.prompt,"on":r.on,"last":r.last,"tz":if r.tz.is_empty() { "machine" } else { r.tz.as_str() },
        "once":iso(&rule, r.once),"next":iso(&rule, w.next),"in":w.rel,"done":w.done})
}
/// Sched.saved: the sentence a tool says after saving
pub fn saved(z: &Zones, r: &Routine, now: u64) -> String {
    let w = row(z, r, now); let rule = person_rule(z, now);
    let zone = if r.tz.is_empty() { "this machine's time".to_string() } else if r.tz == "person" { format!("your time ({})", z.person()) } else { r.tz.clone() };
    if r.once > 0 { format!("once, {}{}", rule.show(r.once), if w.rel.is_empty() { String::new() } else { format!(" ({})", w.rel) }) }
    else { format!("{}, {zone}{}", w.when, if w.next_text.is_empty() { String::new() } else { format!("; next {}", w.next_text) }) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn sched_words() {
        assert_eq!(dur(5), "under a minute"); assert_eq!(dur(1200), "20m"); assert_eq!(dur(3600 * 3 + 300), "3h 5m"); assert_eq!(dur(86400 * 2 + 4 * 3600), "2d 4h"); assert_eq!(dur(86400), "1d");
        assert_eq!(rel(10, 0), ""); assert_eq!(rel(10, 10), "due now"); assert_eq!(rel(0, 70000), "in 19h 26m");
        for (c, w) in [("* * * * *", "every minute"), ("*/15 * * * *", "every 15 minutes"), ("0 * * * *", "hourly"), ("30 * * * *", "hourly at :30"), ("0 */2 * * *", "every 2 hours"), ("0 * * * *", "hourly"),
            ("30 9 * * *", "daily at 09:30"), ("0 8 * * 1-5", "weekdays at 08:00"), ("0 8 1 * *", "0 8 1 * *"), ("whatever", "whatever")] { assert_eq!(when_cron(c), w, "{c}") }
        assert_eq!(kind("remind", 5), "reminder"); assert_eq!(kind("", 5), "once"); assert_eq!(kind("", 0), "routine");
    }
}
