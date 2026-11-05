//! Time zones as POSIX TZ rules (src/core/tz.bend): parse the rule the hub stored for a zone (`tz.rule.<name>`, the footer of its TZif file),
//! and read the wall clock at a UTC instant. Same arithmetic as tz.bend, including its saturating subtraction (`M.Snooze.local`/`utc`).
//! Only what the time stamps need: no `Tz.at` (the instant of a wall time), no abbreviations.

#[derive(Clone, Copy, Debug, PartialEq)]
struct Sg { east: bool, n: u64 }
#[derive(Clone, Copy, Debug, PartialEq)]
enum Day { M(u64, u64, u64), J(u64), N(u64) }
#[derive(Clone, Debug, PartialEq)]
pub struct Rule { std: Sg, dst: Option<Sg>, start: (Day, Sg), end: (Day, Sg) }

impl Rule {
    pub fn utc() -> Rule { Rule { std: Sg { east: true, n: 0 }, dst: None, start: DEF_START, end: DEF_END } }
}
const DEF_START: (Day, Sg) = (Day::M(3, 2, 0), Sg { east: true, n: 7200 });
const DEF_END: (Day, Sg) = (Day::M(11, 1, 0), Sg { east: true, n: 7200 });

/// Tz.local / Tz.utc (M.Snooze.local/utc): saturating
fn local(s: Sg, t: u64) -> u64 { if s.east { t + s.n } else { t.saturating_sub(s.n) } }
fn to_utc(s: Sg, w: u64) -> u64 { if s.east { w.saturating_sub(s.n) } else { w + s.n } }
fn sg_add(a: Sg, b: Sg) -> Sg {
    if a.east == b.east { Sg { east: a.east, n: a.n + b.n } } else if a.n >= b.n { Sg { east: a.east, n: a.n - b.n } } else { Sg { east: b.east, n: b.n - a.n } }
}
fn sg_nat(s: Sg) -> u64 { if s.east { s.n } else { 0 } }

/// h[h[h]][:mm[:ss]] as seconds
fn hms(t: &str) -> Option<u64> {
    let p: Vec<&str> = t.split(':').collect();
    let num = |s: &str| -> Option<u64> { if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) { None } else { s.parse().ok() } };
    match p.as_slice() {
        [h] => { let h = num(h)?; (h <= 167).then(|| h * 3600) }
        [h, m] => { let (h, m) = (num(h)?, num(m)?); (h <= 167 && m <= 59).then(|| h * 3600 + m * 60) }
        [h, m, s] => { let (h, m, s) = (num(h)?, num(m)?, num(s)?); (h <= 167 && m <= 59 && s <= 59).then(|| h * 3600 + m * 60 + s) }
        _ => None,
    }
}
/// a POSIX offset: no sign or + is west, - is east
fn posix(t: &str) -> Option<Sg> {
    let (neg, rest) = match t.chars().next()? { '-' => (true, &t[1..]), '+' => (false, &t[1..]), _ => (false, t) };
    Some(Sg { east: neg, n: hms(rest)? })
}
/// a transition time: the sign is literal
fn ttime(t: &str) -> Option<Sg> {
    let (neg, rest) = match t.chars().next()? { '-' => (true, &t[1..]), '+' => (false, &t[1..]), _ => (false, t) };
    Some(Sg { east: !neg, n: hms(rest)? })
}
fn day(s: &str) -> Option<Day> {
    let mut c = s.chars(); let first = c.next()?; let rest = &s[first.len_utf8()..];
    let num = |x: &str| -> Option<u64> { if x.is_empty() || !x.bytes().all(|b| b.is_ascii_digit()) { None } else { x.parse().ok() } };
    match first {
        'M' => { let p: Vec<&str> = rest.split('.').collect(); if p.len() != 3 { return None } let (m, w, d) = (num(p[0])?, num(p[1])?, num(p[2])?);
            ((1..=12).contains(&m) && (1..=5).contains(&w) && d <= 6).then(|| Day::M(m, w, d)) }
        'J' => { let n = num(rest)?; (1..=365).contains(&n).then(|| Day::J(n)) }
        _ => { let n = num(s)?; (n <= 365).then(|| Day::N(n)) }
    }
}
fn trans(s: &str) -> Option<(Day, Sg)> {
    let p: Vec<&str> = s.split('/').collect();
    match p.as_slice() { [d] => Some((day(d)?, Sg { east: true, n: 7200 })), [d, t] => Some((day(d)?, ttime(t)?)), _ => None }
}
/// the name and offset tokens of a rule's first part: a run of letters, a run of digits/+/-/:, or <...>
fn tokens(s: &str) -> Option<Vec<String>> {
    let (mut out, mut cur, mut kind) = (Vec::<String>::new(), String::new(), 0u8); // kind: 0 none, 1 alpha, 2 number, 3 quoted
    for c in s.chars() {
        if kind == 3 { if c == '>' { out.push(format!("<{}", std::mem::take(&mut cur))); kind = 0 } else { cur.push(c) } continue }
        let k = if c == '<' { 4 } else if c.is_alphabetic() { 1 } else if c.is_ascii_digit() || matches!(c, '+' | '-' | ':') { 2 } else { return None };
        if k == 4 { if kind != 0 { out.push(std::mem::take(&mut cur)) } cur.clear(); kind = 3; continue }
        if k == kind { cur.push(c) } else { if kind != 0 { out.push(std::mem::take(&mut cur)) } cur.clear(); cur.push(c); kind = k }
    }
    if kind == 3 { return None }
    if kind != 0 { out.push(cur) }
    Some(out)
}
/// a name token: three or more letters, or <...> with something inside (quoted tokens keep their leading '<')
fn name(t: &str) -> bool { if let Some(r) = t.strip_prefix('<') { !r.is_empty() } else { t.chars().count() >= 3 && t.chars().all(char::is_alphabetic) } }

impl Rule {
    /// Tz.parse: None when the text is not a POSIX rule
    pub fn parse(s: &str) -> Option<Rule> {
        let parts: Vec<&str> = s.trim().split(',').collect();
        let ts = tokens(parts[0])?;
        let std = |n: &str, o: &str| -> Option<Sg> { if !name(n) { return None } posix(o) };
        let mut r = match ts.as_slice() {
            [n, o] => Rule { std: std(n, o)?, dst: None, start: DEF_START, end: DEF_END },
            [n, o, d] => { let so = std(n, o)?; if !name(d) { return None } Rule { std: so, dst: Some(sg_add(so, Sg { east: true, n: 3600 })), start: DEF_START, end: DEF_END } }
            [n, o, d, o2] => { let so = std(n, o)?; if !name(d) { return None } Rule { std: so, dst: Some(posix(o2)?), start: DEF_START, end: DEF_END } }
            _ => return None,
        };
        match parts.as_slice() {
            [_] => {}
            [_, a, b] => { if r.dst.is_none() { return None } r.start = trans(a)?; r.end = trans(b)?; }
            _ => return None,
        }
        Some(r)
    }
    /// Tz.wall: the clock on the wall at the UTC instant t, in seconds of the wall's own epoch
    pub fn wall(&self, t: u64) -> u64 {
        let off = if self.is_dst(t) { self.dst.unwrap_or(self.std) } else { self.std };
        local(off, t)
    }
    fn is_dst(&self, t: u64) -> bool {
        let Some(dof) = self.dst else { return false };
        let y = year(local(self.std, t));
        let s = to_utc(self.std, trans_wall(self.start, y));
        let e = to_utc(dof, trans_wall(self.end, y));
        if s < e { t >= s && t < e } else { t >= s || t < e }
    }
}

fn leap(y: u64) -> bool { y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) }
/// days since 1970-01-01 of y-m-d (CR.Cron.days)
fn days(y: u64, m: u64, d: u64) -> u64 {
    let (y, m) = (y as i64, m as i64);
    let y2 = if m <= 2 { y - 1 } else { y }; let era = y2.div_euclid(400); let yoe = y2 - era * 400;
    let mp = (m + 9) % 12; let doy = (153 * mp + 2) / 5 + d as i64 - 1; let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe - 719468) as u64
}
fn year(t: u64) -> u64 {
    let z = (t / 86400) as i64 + 719468; let era = z.div_euclid(146097); let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153; let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }) as u64
}
fn wday(d: u64) -> u64 { (d + 4) % 7 } // 0 = Sunday
fn month_next(y: u64, m: u64) -> u64 { if m == 12 { days(y + 1, 1, 1) } else { days(y, m + 1, 1) } }
fn m_day(y: u64, m: u64, w: u64, d: u64) -> u64 {
    let first = days(y, m, 1);
    let cand = first + (d + (7 - wday(first))) % 7 + 7 * (w - 1);
    if cand >= month_next(y, m) { cand - 7 } else { cand }
}
fn day_num(d: Day, y: u64) -> u64 {
    match d {
        Day::M(m, w, dd) => m_day(y, m, w, dd),
        Day::J(n) => days(y, 1, 1) + (n - 1) + if leap(y) && n >= 60 { 1 } else { 0 },
        Day::N(n) => days(y, 1, 1) + n,
    }
}
fn trans_wall(t: (Day, Sg), y: u64) -> u64 { sg_nat(sg_add(Sg { east: true, n: day_num(t.0, y) * 86400 }, t.1)) }

#[cfg(test)]
mod tests {
    use super::*;
    const NOV5_2026: u64 = 1_793_880_000; // 2026-11-05 12:00:00 UTC
    const JUL1_2026: u64 = 1_782_907_200; // 2026-07-01 12:00:00 UTC
    #[test] fn utc_and_fixed_zones() {
        assert_eq!(Rule::utc().wall(NOV5_2026), NOV5_2026);
        assert_eq!(Rule::parse("<+03>-3").unwrap().wall(NOV5_2026), NOV5_2026 + 3 * 3600);
        assert_eq!(Rule::parse("IST-5:30").unwrap().wall(NOV5_2026), NOV5_2026 + 19800);
    }
    #[test] fn eastern_time_follows_its_dst_rule() {
        let ny = Rule::parse("EST5EDT,M3.2.0,M11.1.0").unwrap();
        assert_eq!(ny.wall(NOV5_2026), NOV5_2026 - 5 * 3600, "November 5 2026 is after the first Sunday of November: EST");
        assert_eq!(ny.wall(JUL1_2026), JUL1_2026 - 4 * 3600, "July: EDT");
        // 2026-03-08 06:59:59 UTC is 01:59:59 EST, 07:00:00 UTC is 03:00:00 EDT
        let t = days(2026, 3, 8) * 86400 + 7 * 3600;
        assert_eq!(ny.wall(t - 1), t - 1 - 5 * 3600); assert_eq!(ny.wall(t), t - 4 * 3600);
        // 2026-11-01 05:59:59 UTC is 01:59:59 EDT, 06:00:00 UTC is 01:00:00 EST
        let u = days(2026, 11, 1) * 86400 + 6 * 3600;
        assert_eq!(ny.wall(u - 1), u - 1 - 4 * 3600); assert_eq!(ny.wall(u), u - 5 * 3600);
    }
    #[test] fn europe_and_the_southern_year() {
        let hel = Rule::parse("EET-2EEST,M3.5.0/3,M10.5.0/4").unwrap();
        assert_eq!(hel.wall(NOV5_2026), NOV5_2026 + 2 * 3600); assert_eq!(hel.wall(JUL1_2026), JUL1_2026 + 3 * 3600);
        let syd = Rule::parse("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap();
        assert_eq!(syd.wall(NOV5_2026), NOV5_2026 + 11 * 3600, "southern summer"); assert_eq!(syd.wall(JUL1_2026), JUL1_2026 + 10 * 3600);
    }
    #[test] fn what_is_not_a_rule_is_none() {
        for s in ["", "Europe/Helsinki", "EST5EDT,M13.1.0,M11.1.0", "EST5,M3.2.0,M11.1.0", "EST", "A5"] { assert!(Rule::parse(s).is_none(), "{s}"); }
    }
    #[test] fn calendar_helpers() { assert_eq!(days(1970, 1, 1), 0); assert_eq!(days(2023, 11, 14), 19675); assert_eq!(year(days(2026, 12, 31) * 86400), 2026); assert_eq!(wday(days(2026, 11, 1)), 0); }
}
