//! Time for routines: src/core/cron (`Cron.*`), tz (`Tz.*`), when (`When.*`) and sched (`Sched.*`, `Zone.*`).
//! Pure. Every number is a non-negative count of seconds and subtraction stops at 0, as `Nat.sub` does. test corpora
//! are repeated as the Rust tests below.
use std::collections::HashMap;

// ---- calendar (Cron.days, Cron.civil) ----
/// Cron.days: days since 1970-01-01 of a civil date
pub fn days(y: u64, m: u64, d: u64) -> u64 {
    let (y, mp) = if m <= 2 { (y.saturating_sub(1), m + 9) } else { (y, m.saturating_sub(3)) };
    let era = y / 400; let yoe = y - era * 400; let doy = (153 * mp + 2) / 5 + d.saturating_sub(1);
    (era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy).saturating_sub(719468)
}
/// Cron.civil: (year, month, day) of a day number
pub fn civil(days: u64) -> (u64, u64, u64) {
    let z = days + 719468; let era = z / 146097; let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153; let d = doy - (153 * mp + 2) / 5 + 1;
    let y = yoe + era * 400;
    if mp < 10 { (y, mp + 3, d) } else { (y + 1, mp - 9, d) }
}
/// Tz.wday: 0 = Sunday
pub fn wday(days: u64) -> u64 { (days + 4) % 7 }
fn leap(y: u64) -> bool { y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) }
/// Snooze.local / Snooze.utc
fn local(east: bool, t: u64, off: u64) -> u64 { if east { t + off } else { t.saturating_sub(off) } }
fn utc(east: bool, w: u64, off: u64) -> u64 { if east { w.saturating_sub(off) } else { w + off } }

// ---- cron ----
#[derive(Clone, Debug, PartialEq)]
pub struct Cron { pub mins: Vec<u64>, pub hours: Vec<u64>, pub doms: Vec<u64>, pub months: Vec<u64>, pub dows: Vec<u64>, pub dom_any: bool, pub dow_any: bool }

/// Cron.num: digits only, at least one
pub fn num(s: &str) -> Option<u64> { if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) { return None } Some(s.bytes().fold(0u64, |a, c| a.saturating_mul(10).saturating_add((c - b'0') as u64))) }
const MNAMES: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const DNAMES: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
fn val(s: &str, names: &[&str], base: u64) -> Option<u64> { num(s).or_else(|| names.iter().position(|n| *n == s).map(|i| base + i as u64)) }
/// (a, b, step) in lo..=hi
fn mk(a: u64, b: u64, s: u64, lo: u64, hi: u64) -> Option<(u64, u64, u64)> { if a >= lo && a <= b && b <= hi && s >= 1 { Some((a, b, s)) } else { None } }
fn rng(r: &str, stepped: bool, s: u64, lo: u64, hi: u64, names: &[&str], base: u64) -> Option<(u64, u64, u64)> {
    let ps: Vec<&str> = r.split('-').collect();
    match ps.as_slice() {
        [a] if *a == "*" => mk(lo, hi, s, lo, hi),
        [a] => { let x = val(a, names, base)?; if stepped { mk(x, hi, s, lo, hi) } else { mk(x, x, s, lo, hi) } }
        [a, b] => mk(val(a, names, base)?, val(b, names, base)?, s, lo, hi),
        _ => None,
    }
}
fn item(s: &str, lo: u64, hi: u64, names: &[&str], base: u64) -> Option<(u64, u64, u64)> {
    let ps: Vec<&str> = s.split('/').collect();
    match ps.as_slice() { [r] => rng(r, false, 1, lo, hi, names, base), [r, st] => rng(r, true, num(st)?, lo, hi, names, base), _ => None }
}
fn rngs(s: &str, lo: u64, hi: u64, names: &[&str], base: u64) -> Option<Vec<(u64, u64, u64)>> { s.split(',').map(|i| item(i, lo, hi, names, base)).collect() }
fn covers(r: &(u64, u64, u64), v: u64) -> bool { v >= r.0 && v <= r.1 && (v - r.0) % r.2 == 0 }
fn hit(rs: &[(u64, u64, u64)], v: u64) -> bool { rs.iter().any(|r| covers(r, v)) }
fn vals(rs: &[(u64, u64, u64)], lo: u64, hi: u64) -> Vec<u64> { (lo..=hi).filter(|v| hit(rs, *v)).collect() }
/// Cron.fields: lower case words; one alias word, or "every 15m", "daily 09:30", "weekdays 08:00"
pub fn fields(s: &str) -> Vec<String> {
    let words = |t: &str| -> Vec<String> { t.trim().split(' ').filter(|w| !w.is_empty()).map(str::to_string).collect() };
    let ws = words(&s.to_lowercase());
    match ws.len() {
        0 => vec![],
        1 => { let a = match ws[0].as_str() { "@hourly" => "0 * * * *", "@daily" | "@midnight" => "0 0 * * *", "@weekly" => "0 0 * * 0", "@monthly" => "0 0 1 * *", "@yearly" | "@annually" => "0 0 1 1 *", _ => "" }; words(a) }
        2 => words(&two(&ws[0], &ws[1])),
        _ => ws,
    }
}
fn within(n: Option<u64>, lo: u64, hi: u64) -> Option<u64> { n.filter(|x| *x >= lo && *x <= hi) }
fn two(a: &str, b: &str) -> String {
    let at = |dw: &str| -> String { let ps: Vec<&str> = b.split(':').collect(); match ps.as_slice() { [h, m] => match (within(num(h), 0, 23), within(num(m), 0, 59)) { (Some(h), Some(m)) => format!("{m} {h} * * {dw}"), _ => String::new() }, _ => String::new() } };
    match a {
        "every" => { let (n, u) = (b.get(..b.len().saturating_sub(1)).and_then(num), b.chars().last()); match (u, n) {
            (Some('m'), n) => within(n, 1, 59).map_or(String::new(), |n| format!("*/{n} * * * *")),
            (Some('h'), n) => within(n, 1, 23).map_or(String::new(), |n| format!("0 */{n} * * *")),
            _ => String::new() } }
        "daily" => at("*"), "weekdays" => at("1-5"), _ => String::new(),
    }
}
impl Cron {
    pub fn parse(s: &str) -> Option<Cron> {
        let f = fields(s); if f.len() != 5 { return None }
        let (mi, ho, dm, mo, dw) = (rngs(&f[0], 0, 59, &[], 0)?, rngs(&f[1], 0, 23, &[], 0)?, rngs(&f[2], 1, 31, &[], 0)?, rngs(&f[3], 1, 12, &MNAMES, 1)?, rngs(&f[4], 0, 7, &DNAMES, 0)?);
        Some(Cron { mins: vals(&mi, 0, 59), hours: vals(&ho, 0, 23), doms: vals(&dm, 1, 31), months: vals(&mo, 1, 12),
            dows: (0..7).filter(|v| hit(&dw, *v) || (*v == 0 && hit(&dw, 7))).collect(), dom_any: f[2] == "*", dow_any: f[4] == "*" })
    }
    /// Cron.text: the normalised line ("" when it does not parse)
    pub fn text(s: &str) -> String { if Cron::parse(s).is_some() { fields(s).join(" ") } else { String::new() } }
    fn day_ok(&self, dm: bool, dw: bool) -> bool { match (self.dom_any, self.dow_any) { (true, true) => true, (true, false) => dw, (false, true) => dm, (false, false) => dm || dw } }
    /// Cron.clock: the first minute-of-day at or after m0 the line has
    fn clock(&self, m0: u64) -> Option<u64> {
        let h0 = m0 / 60; let h = *self.hours.iter().find(|x| **x >= h0)?;
        let head = self.mins.first().copied().unwrap_or(0);
        if h == h0 {
            if let Some(x) = self.mins.iter().find(|x| **x >= m0 % 60) { return Some(h * 60 + x) }
            let h2 = *self.hours.iter().find(|x| **x >= h0 + 1)?; Some(h2 * 60 + head)
        } else { Some(h * 60 + head) }
    }
    /// Cron.raw: the next match strictly after the wall minute of `after`, scanning at most Cron.span days
    fn next_wall(&self, loc: u64) -> u64 {
        let end = loc / 86400 + 3300; let mut d = loc / 86400; let mut m0 = (loc % 86400) / 60;
        for _ in 0..4000 {
            if d > end { return 0 }
            let (y, m, dd) = civil(d);
            if !self.months.contains(&m) { d = match self.months.iter().find(|x| **x >= m + 1) { Some(x) => days(y, *x, 1), None => days(y + 1, self.months.first().copied().unwrap_or(0), 1) }; m0 = 0; continue }
            if self.day_ok(self.doms.contains(&dd), self.dows.contains(&wday(d))) { if let Some(t) = self.clock(m0) { return d * 86400 + t * 60 } }
            d += 1; m0 = 0;
        }
        0
    }
    /// Cron.next: the UTC instant of the next match after `after` (0: none), in the offset (off, east)
    pub fn next(&self, after: u64, off: u64, east: bool) -> u64 {
        let loc = local(east, after, off) / 60 * 60 + 60;
        let r = self.next_wall(loc); let x = if r == 0 { 0 } else { utc(east, r, off) };
        if x > after { x } else { 0 }
    }
    /// Cron.due: now is past `last` and at or past the next match after it
    pub fn due(&self, last: u64, now: u64, off: u64, east: bool) -> bool { now > last && { let x = self.next(last, off, east); x != 0 && now >= x } }
}

// ---- time zones (POSIX TZ rules) ----
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Day { M(u64, u64, u64), J(u64), N(u64) }
/// Offsets are seconds east of UTC (negative: west)
#[derive(Clone, Debug, PartialEq)]
pub struct TzRule { pub sname: String, pub soff: i64, pub dst: bool, pub dname: String, pub doff: i64, pub start: Day, pub stime: i64, pub end: Day, pub etime: i64 }
pub struct TzOff { pub off: i64, pub dst: bool, pub abbr: String }

fn hms(t: &str) -> Option<u64> {
    let ps: Vec<&str> = t.split(':').collect();
    let n = |s: &str| num(s);
    match ps.as_slice() {
        [h] => { let h = n(h)?; (h <= 167).then(|| h * 3600) }
        [h, m] => { let (h, m) = (n(h)?, n(m)?); (h <= 167 && m <= 59).then(|| h * 3600 + m * 60) }
        [h, m, s] => { let (h, m, s) = (n(h)?, n(m)?, n(s)?); (h <= 167 && m <= 59 && s <= 59).then(|| h * 3600 + m * 60 + s) }
        _ => None,
    }
}
fn sign(s: &str) -> (bool, &str) { match s.chars().next() { Some('-') => (true, &s[1..]), Some('+') => (false, &s[1..]), _ => (false, s) } }
/// a POSIX offset: none or + is west, - is east (seconds east)
fn posix(t: &str) -> Option<i64> { let (neg, rest) = sign(t); let v = hms(rest)? as i64; Some(if neg { v } else { -v }) }
/// a transition time: the sign is literal
fn ttime(t: &str) -> Option<i64> { let (neg, rest) = sign(t); let v = hms(rest)? as i64; Some(if neg { -v } else { v }) }
fn day_of(s: &str) -> Option<Day> {
    let mut cs = s.chars(); let c = cs.next()?; let rest = &s[c.len_utf8()..];
    match c {
        'M' => { let ps: Vec<&str> = rest.split('.').collect(); if ps.len() != 3 { return None } let (m, w, d) = (num(ps[0])?, num(ps[1])?, num(ps[2])?); ((1..=12).contains(&m) && (1..=5).contains(&w) && d <= 6).then_some(Day::M(m, w, d)) }
        'J' => { let n = num(rest)?; (1..=365).contains(&n).then_some(Day::J(n)) }
        _ => { let n = num(s)?; (n <= 365).then_some(Day::N(n)) }
    }
}
fn trans(s: &str) -> Option<(Day, i64)> { let ps: Vec<&str> = s.split('/').collect(); match ps.as_slice() { [d] => Some((day_of(d)?, 7200)), [d, t] => Some((day_of(d)?, ttime(t)?)), _ => None } }
/// Tz.tokens: names (letters, or <...>) and offsets (sign, digits, colons)
fn tokens(s: &str) -> Option<Vec<String>> {
    let cs: Vec<char> = s.chars().collect(); let mut out = vec![]; let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '<' { let j = cs[i + 1..].iter().position(|x| *x == '>')? + i + 1; out.push(cs[i..=j].iter().collect()); i = j + 1 }
        else if c.is_ascii_alphabetic() { let j = cs[i..].iter().position(|x| !x.is_ascii_alphabetic()).map_or(cs.len(), |p| p + i); out.push(cs[i..j].iter().collect()); i = j }
        else if c.is_ascii_digit() || c == '+' || c == '-' || c == ':' { let j = cs[i + 1..].iter().position(|x| !(x.is_ascii_digit() || *x == ':')).map_or(cs.len(), |p| p + i + 1); out.push(cs[i..j].iter().collect()); i = j }
        else { return None }
    }
    Some(out)
}
fn name(t: &str) -> Option<String> {
    if let Some(r) = t.strip_prefix('<') { let r = r.strip_suffix('>').unwrap_or(r); return (!r.is_empty()).then(|| r.to_string()) }
    (t.chars().all(|c| c.is_ascii_alphabetic()) && t.chars().count() >= 3).then(|| t.to_string())
}
impl TzRule {
    fn std(sn: String, so: i64) -> TzRule { TzRule { sname: sn, soff: so, dst: false, dname: String::new(), doff: so, start: Day::M(3, 2, 0), stime: 7200, end: Day::M(11, 1, 0), etime: 7200 } }
    pub fn utc0() -> TzRule { TzRule { sname: "UTC".into(), ..TzRule::std(String::new(), 0) } }
    /// Tz.fixed
    pub fn fixed(east: bool, secs: u64) -> TzRule { TzRule::std(String::new(), if east { secs as i64 } else { -(secs as i64) }) }
    /// Tz.parse: a POSIX TZ string (the footer of a TZif file)
    pub fn parse(s: &str) -> Option<TzRule> {
        let ps: Vec<&str> = s.trim().split(',').collect();
        let ts = tokens(ps[0])?;
        let mut r = match ts.as_slice() {
            [n, o] => TzRule::std(name(n)?, posix(o)?),
            [n, o, d] => { let std = TzRule::std(name(n)?, posix(o)?); let so = std.soff; TzRule { dst: true, dname: name(d)?, doff: so + 3600, ..std } }
            [n, o, d, o2] => { let std = TzRule::std(name(n)?, posix(o)?); TzRule { dst: true, dname: name(d)?, doff: posix(o2)?, ..std } }
            _ => return None,
        };
        match &ps[1..] {
            [] => {}
            [a, b] => { if !r.dst { return None } let (da, ta) = trans(a)?; let (db, tb) = trans(b)?; r.start = da; r.stime = ta; r.end = db; r.etime = tb }
            _ => return None,
        }
        Some(r)
    }
    fn day_num(d: Day, y: u64) -> u64 {
        match d {
            Day::M(m, w, dd) => { let first = days(y, m, 1); let cand = first + (dd + 7 - wday(first)) % 7 + 7 * (w - 1); let next = if m == 12 { days(y + 1, 1, 1) } else { days(y, m + 1, 1) }; if cand >= next { cand - 7 } else { cand } }
            Day::J(n) => days(y, 1, 1) + n - 1 + if leap(y) && n >= 60 { 1 } else { 0 },
            Day::N(n) => days(y, 1, 1) + n,
        }
    }
    fn trans_wall(d: Day, t: i64, y: u64) -> u64 { ((TzRule::day_num(d, y) * 86400) as i64 + t).max(0) as u64 }
    fn sg_local(off: i64, t: u64) -> u64 { local(off >= 0, t, off.unsigned_abs()) }
    fn sg_utc(off: i64, w: u64) -> u64 { utc(off >= 0, w, off.unsigned_abs()) }
    fn begins(&self, y: u64) -> u64 { TzRule::sg_utc(self.soff, TzRule::trans_wall(self.start, self.stime, y)) }
    fn ends(&self, y: u64) -> u64 { TzRule::sg_utc(self.doff, TzRule::trans_wall(self.end, self.etime, y)) }
    /// Tz.is.dst: whether DST is in effect at the UTC instant t
    pub fn is_dst(&self, t: u64) -> bool {
        if !self.dst { return false }
        let y = civil(TzRule::sg_local(self.soff, t) / 86400).0; let (s, e) = (self.begins(y), self.ends(y));
        if s < e { t >= s && t < e } else { t >= s || t < e }
    }
    /// Tz.off
    pub fn off(&self, t: u64) -> TzOff { if self.is_dst(t) { TzOff { off: self.doff, dst: true, abbr: self.dname.clone() } } else { TzOff { off: self.soff, dst: false, abbr: self.sname.clone() } } }
    /// Tz.wall: the wall clock at the UTC instant t
    pub fn wall(&self, t: u64) -> u64 { TzRule::sg_local(self.off(t).off, t) }
    /// Tz.at: the instant a wall time happened; the earlier one when it happened twice, and in a gap the instant with the offset from before
    pub fn at(&self, w: u64) -> u64 { let ud = TzRule::sg_utc(self.doff, w); let us = TzRule::sg_utc(self.soff, w); if self.dst && self.is_dst(ud) { ud } else { us } }

    pub fn offset_text(o: &TzOff) -> String { let s = o.off.unsigned_abs(); format!("{}{:02}:{:02}", if o.off >= 0 { "+" } else { "-" }, s / 3600, s % 3600 / 60) }
    pub fn abbr(o: &TzOff) -> String { if o.abbr.is_empty() { TzRule::offset_text(o) } else { o.abbr.clone() } }
    /// Tz.show: "Tue 14 Oct 09:30 EEST"
    pub fn show(&self, t: u64) -> String {
        const W: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]; const M: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        let w = self.wall(t); let (_, m, d) = civil(w / 86400);
        format!("{} {} {} {:02}:{:02} {}", W[wday(w / 86400) as usize], d, M[(m - 1) as usize], w % 86400 / 3600, w % 3600 / 60, TzRule::abbr(&self.off(t)))
    }
    /// Tz.iso: "2026-10-14T09:30:00+03:00"
    pub fn iso(&self, t: u64) -> String { let w = self.wall(t); let (y, m, d) = civil(w / 86400); format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}{}", w % 86400 / 3600, w % 3600 / 60, w % 60, TzRule::offset_text(&self.off(t))) }
}

// ---- when (times people write) ----
const DEFAULT_TIME: u64 = 32400;
pub const WHEN_TEXT: &str = "I can read: 2026-10-14T09:00+03:00, 2026-10-14 09:00, in 20m, in 1h30m, in 3d, today 17:30, tomorrow 9am, monday 09:00, or just 9am.";
fn n_exact(s: &str, k: usize) -> Option<u64> { if s.chars().count() == k { num(s) } else { None } }
fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }
fn drop(s: &str, n: usize) -> String { s.chars().skip(n).collect() }
fn char_at(s: &str, i: usize, c: char) -> bool { s.chars().nth(i) == Some(c) }
/// When.date: YYYY-MM-DD as the wall seconds of its midnight
fn w_date(j: &str) -> Option<u64> {
    let (y, m, d) = (n_exact(&take(j, 4), 4)?, n_exact(&take(&drop(j, 5), 2), 2)?, n_exact(&take(&drop(j, 8), 2), 2)?);
    let dashes = char_at(j, 4, '-') && char_at(j, 7, '-') && j.chars().count() == 10;
    if !(dashes && (1..=12).contains(&m) && (1..=31).contains(&d)) { return None }
    let dn = days(y, m, d); (civil(dn) == (y, m, d)).then(|| dn * 86400)
}
struct Wt { wall: u64, has: bool, east: bool, off: u64 }
fn w_off(tail: &str) -> Option<(bool, bool, u64)> {
    let mut cs = tail.chars();
    let Some(c) = cs.next() else { return Some((false, true, 0)) };
    let t: String = cs.collect();
    let hm = |neg: bool| -> Option<(bool, bool, u64)> {
        let d: String = t.chars().filter(|x| *x != ':').collect(); let len = d.chars().count();
        let (h, m) = (n_exact(&take(&d, 2), 2), n_exact(&take(&drop(&d, 2), 2), 2));
        let off = match (h, m) { (Some(h), Some(m)) if len == 4 && h <= 23 && m <= 59 => h * 3600 + m * 60, (Some(h), None) if len == 2 && h <= 23 => h * 3600, _ => return None };
        Some((true, !neg, off))
    };
    match c { 'z' => if t.is_empty() { Some((true, true, 0)) } else { None }, '+' => hm(false), '-' => hm(true), _ => None }
}
fn w_clock(day: u64, t: &str) -> Option<Wt> {
    let (h, m) = (n_exact(&take(t, 2), 2)?, n_exact(&take(&drop(t, 3), 2), 2)?);
    if !(char_at(t, 2, ':') && h <= 23 && m <= 59) { return None }
    let mut secs = h * 3600 + m * 60; let tail = drop(t, 5);
    let rest = if let Some(r) = tail.strip_prefix(':') { let s = n_exact(&take(r, 2), 2)?; if s > 59 { return None } secs += s; drop(r, 2) } else { tail };
    let (has, east, off) = w_off(&rest)?; Some(Wt { wall: day + secs, has, east, off })
}
fn w_iso(j: &str) -> Option<Wt> {
    let day = w_date(&take(j, 10))?; let rest = drop(j, 10);
    match rest.chars().next() { None => Some(Wt { wall: day + DEFAULT_TIME, has: false, east: true, off: 0 }), Some('t') => w_clock(day, &drop(&rest, 1)), _ => None }
}
fn unit(u: &str) -> Option<u64> { Some(match u { "s" | "sec" | "secs" | "second" | "seconds" => 1, "m" | "min" | "mins" | "minute" | "minutes" => 60, "h" | "hr" | "hrs" | "hour" | "hours" => 3600, "d" | "day" | "days" => 86400, "w" | "week" | "weeks" => 604800, _ => return None }) }
fn day_word(w: &str) -> Option<u64> { Some(match w { "sun" | "sunday" => 0, "mon" | "monday" => 1, "tue" | "tues" | "tuesday" => 2, "wed" | "wednesday" => 3, "thu" | "thur" | "thurs" | "thursday" => 4, "fri" | "friday" => 5, "sat" | "saturday" => 6, "today" => 100, "tomorrow" => 101, _ => return None }) }
/// When.runs: runs of digits and of letters; anything else spoils it
fn runs(s: &str) -> Option<Vec<String>> {
    let mut out: Vec<String> = vec![]; let mut kind = 0;
    for c in s.chars() { let k = if c.is_ascii_digit() { 1 } else if c.is_alphabetic() && c.is_ascii() { 2 } else { return None }; if k == kind { out.last_mut().unwrap().push(c) } else { out.push(c.to_string()); kind = k } }
    Some(out)
}
fn w_rel(ws: &[String]) -> Option<u64> {
    let ts = runs(&ws.concat())?; let mut acc = Some(0u64); let mut i = 0;
    while i < ts.len() { if i + 1 >= ts.len() { return None } acc = match (acc, num(&ts[i]), unit(&ts[i + 1])) { (Some(a), Some(n), Some(u)) => Some(a.saturating_add(n.saturating_mul(u))), _ => None }; i += 2 }
    acc
}
/// When.hm: "9", "9:30", "9am", "9:30pm", "21:00" as seconds into the day
fn w_hm(w: &str) -> Option<u64> {
    let (am, pm) = (w.ends_with("am"), w.ends_with("pm")); let ampm = am || pm;
    let body = if ampm { take(w, w.chars().count().saturating_sub(2)) } else { w.to_string() };
    let ps: Vec<&str> = body.split(':').collect();
    let (h, m) = match ps.as_slice() { [h] => (*h, ""), [h, m] => (*h, *m), _ => return None };
    let (h, m) = (num(h)?, if m.is_empty() { 0 } else { n_exact(m, 2)? });
    let okh = if ampm { (1..=12).contains(&h) } else { h <= 23 };
    if !(okh && m <= 59) { return None }
    let hh = if ampm { h % 12 + if pm { 12 } else { 0 } } else { h };
    Some(hh * 3600 + m * 60)
}
fn w_hm_explicit(w: &str) -> Option<u64> { if w.contains(':') || w.ends_with("am") || w.ends_with("pm") { w_hm(w) } else { None } }
fn w_at(rule: &TzRule, code: u64, secs: u64, now: u64) -> Option<u64> {
    let d0 = rule.wall(now) / 86400;
    let delta = if code >= 100 { if code == 101 { 1 } else { 0 } } else { (code + 7).saturating_sub(wday(d0)) % 7 };
    let day = d0 + delta; let t = rule.at(day * 86400 + secs);
    if t > now { return Some(t) }
    if code == 102 { Some(rule.at((day + 1) * 86400 + secs)) } else if code < 7 { Some(rule.at((day + 7) * 86400 + secs)) } else { None }
}
fn w_day(ws: &[String], rule: &TzRule, now: u64) -> Option<u64> {
    match ws {
        [w] => match day_word(w) { Some(c) => w_at(rule, c, DEFAULT_TIME, now), None => w_at(rule, 102, w_hm_explicit(w)?, now) },
        [d, t] => w_at(rule, day_word(d)?, w_hm(t)?, now),
        _ => None,
    }
}
/// When.parse: a moment strictly after `now` from a phrase, else None
pub fn when_parse(text: &str, now: u64, rule: &TzRule) -> Option<u64> {
    if text.chars().count() > 80 { return None }
    let ws: Vec<String> = text.to_lowercase().trim().split(' ').filter(|w| !w.is_empty()).map(str::to_string).collect();
    let h = ws.first()?; let t = &ws[1..];
    let r = if h == "in" { let s = w_rel(t)?; (s > 0).then(|| now + s) }
        else if h.chars().count() >= 10 && char_at(h, 4, '-') && char_at(h, 7, '-') { let w = w_iso(&ws.join("t"))?; Some(if w.has { utc(w.east, w.wall, w.off) } else { rule.at(w.wall) }) }
        else { let ws2: Vec<String> = ws.iter().filter(|w| *w != "at").cloned().collect(); w_day(&ws2, rule, now) };
    r.filter(|t| *t > now)
}
/// When.independent: a phrase that needs no zone to read
pub fn when_independent(text: &str) -> bool { let s = take(text, 256).trim().to_lowercase(); let clock = drop(&s, 10); s.starts_with("in ") || clock.ends_with('z') || clock.contains('+') || clock.contains('-') }

// ---- zones and schedules ----
/// Zone.*: the person's zone and the rules the hub resolved (settings `tz.rule.<name>`)
pub struct Zones<'a> { pub settings: &'a HashMap<String, String> }
impl Zones<'_> {
    fn get(&self, k: &str) -> &str { self.settings.get(k).map(String::as_str).unwrap_or("") }
    /// Zone.person: the zone they or a bot set, else the phone's, else the hub machine's, else UTC
    pub fn person(&self) -> String { [self.get("person.tz"), self.get("person.tz.device"), self.get("tz.machine")].into_iter().find(|s| !s.is_empty()).unwrap_or("UTC").to_string() }
    /// Zone.rule (UTC is built in)
    pub fn rule(&self, name: &str) -> Option<TzRule> { if ["UTC", "Etc/UTC", "Etc/UCT", "UCT"].contains(&name) { Some(TzRule::utc0()) } else { TzRule::parse(self.get(&format!("tz.rule.{name}"))) } }
    /// Zone.rule.of: a routine's tz field ("person" is the person's zone)
    pub fn rule_of(&self, tz: &str) -> Option<TzRule> { self.rule(&if tz == "person" { self.person() } else { tz.to_string() }) }
    /// Zone.valid: a name made of letters, digits and _ + - / segments
    pub fn valid(name: &str) -> bool { (1..=64).contains(&name.chars().count()) && name.split('/').all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '-')) }
}
/// the fields of a routine the schedule reads
pub struct Plan<'a> { pub cron: &'a str, pub on: bool, pub deleted: bool, pub done: bool, pub last: u64, pub once: u64, pub tz: &'a str, pub slot: u64 }
fn base(rule: &TzRule, last: u64, slot: u64) -> u64 { if slot > 0 { slot } else { rule.wall(last) } }
impl Plan<'_> {
    fn live(&self) -> bool { !self.done && self.on && !self.deleted }
    /// Sched.due: whether a routine runs at `now` (mo, me: the machine's offset, for legacy routines)
    pub fn due(&self, z: &Zones, now: u64, mo: u64, me: bool) -> bool {
        if !self.live() { return false }
        if self.once > 0 { return now >= self.once }
        let Some(c) = Cron::parse(self.cron) else { return false };
        if self.tz.is_empty() { return c.due(self.last, now, mo, me) }
        match z.rule_of(self.tz) { Some(rule) => c.due(base(&rule, self.last, self.slot), rule.wall(now), 0, true), None => false }
    }
    /// Sched.slot: the time next scheduled after the last run (a UTC instant for legacy and once, a wall clock when zoned; 0: never)
    pub fn slot_next(&self, z: &Zones, mo: u64, me: bool) -> u64 {
        if self.once > 0 { return self.once }
        let Some(c) = Cron::parse(self.cron) else { return 0 };
        if self.tz.is_empty() { return c.next(self.last, mo, me) }
        match z.rule_of(self.tz) { Some(rule) => c.next(base(&rule, self.last, self.slot), 0, true), None => 0 }
    }
    /// Sched.wall: the wall clock of a zoned routine's zone now (the slot to record), 0 for legacy, once or a zone with no rule yet
    pub fn wall(&self, z: &Zones, now: u64) -> u64 { if self.tz.is_empty() || self.once > 0 { 0 } else { z.rule_of(self.tz).map_or(0, |r| r.wall(now)) } }
    /// Sched.next: the UTC instant of the next fire, 0 when off, done or never
    pub fn next(&self, z: &Zones, now: u64, mo: u64, me: bool) -> u64 {
        if !self.live() { return 0 }
        if self.once > 0 { return self.once }
        let Some(c) = Cron::parse(self.cron) else { return 0 };
        let zoned = |rule: &TzRule| -> u64 { let w = c.next(base(rule, self.last, self.slot).max(rule.wall(now)), 0, true); if w == 0 { 0 } else { rule.at(w) } };
        if self.tz.is_empty() { match z.rule(z.get("tz.machine")) { Some(rule) => zoned(&rule), None => c.next(self.last.max(now), mo, me) } }
        else { z.rule_of(self.tz).map_or(0, |r| zoned(&r)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const HEL: &str = "EET-2EEST,M3.5.0/3,M10.5.0/4"; const NY: &str = "EST5EDT,M3.2.0,M11.1.0"; const SYD: &str = "AEST-10AEDT,M10.1.0,M4.1.0/3";
    fn sh(r: &str, t: u64) -> String { TzRule::parse(r).map_or("none".into(), |r| r.show(t)) }
    fn iso(r: &str, t: u64) -> String { TzRule::parse(r).map_or("none".into(), |r| r.iso(t)) }
    fn wall(y: u64, m: u64, d: u64, h: u64, mi: u64) -> u64 { days(y, m, d) * 86400 + h * 3600 + mi * 60 }
    fn at(r: &str, y: u64, m: u64, d: u64, h: u64, mi: u64) -> String { TzRule::parse(r).map_or("none".into(), |r| TzRule::utc0().iso(r.at(wall(y, m, d, h, mi)))) }
    fn ok(r: &str) -> &'static str { if TzRule::parse(r).is_some() { "some" } else { "none" } }
    fn off(r: &str, t: u64) -> String { TzRule::parse(r).map_or("none".into(), |r| { let o = r.off(t); format!("{} {}", TzRule::offset_text(&o), if o.dst { "True" } else { "False" }) }) }
    #[test] fn tz_corpus_test_tz_test() {
        // Europe/Helsinki 1774745999
        assert_eq!(sh("EET-2EEST,M3.5.0/3,M10.5.0/4", 1774745999), "Sun 29 Mar 02:59 EET");
        // Europe/Helsinki 1774746000
        assert_eq!(sh("EET-2EEST,M3.5.0/3,M10.5.0/4", 1774746000), "Sun 29 Mar 04:00 EEST");
        // Europe/Helsinki 1792889999
        assert_eq!(sh("EET-2EEST,M3.5.0/3,M10.5.0/4", 1792889999), "Sun 25 Oct 03:59 EEST");
        // Europe/Helsinki 1792890000
        assert_eq!(sh("EET-2EEST,M3.5.0/3,M10.5.0/4", 1792890000), "Sun 25 Oct 03:00 EET");
        // America/New_York 1772953199
        assert_eq!(sh("EST5EDT,M3.2.0,M11.1.0", 1772953199), "Sun 8 Mar 01:59 EST");
        // America/New_York 1772953200
        assert_eq!(sh("EST5EDT,M3.2.0,M11.1.0", 1772953200), "Sun 8 Mar 03:00 EDT");
        // America/New_York 1793512799
        assert_eq!(sh("EST5EDT,M3.2.0,M11.1.0", 1793512799), "Sun 1 Nov 01:59 EDT");
        // America/New_York 1793512800
        assert_eq!(sh("EST5EDT,M3.2.0,M11.1.0", 1793512800), "Sun 1 Nov 01:00 EST");
        // Australia/Sydney 1791043199
        assert_eq!(sh("AEST-10AEDT,M10.1.0,M4.1.0/3", 1791043199), "Sun 4 Oct 01:59 AEST");
        // Australia/Sydney 1791043200
        assert_eq!(sh("AEST-10AEDT,M10.1.0,M4.1.0/3", 1791043200), "Sun 4 Oct 03:00 AEDT");
        // Australia/Sydney 1775318399
        assert_eq!(sh("AEST-10AEDT,M10.1.0,M4.1.0/3", 1775318399), "Sun 5 Apr 02:59 AEDT");
        // Australia/Sydney 1775318400
        assert_eq!(sh("AEST-10AEDT,M10.1.0,M4.1.0/3", 1775318400), "Sun 5 Apr 02:00 AEST");
        // India
        assert_eq!(sh("IST-5:30", 1791957600), "Wed 14 Oct 11:30 IST");
        // quoted name
        assert_eq!(sh("<+0330>-3:30", 1791957600), "Wed 14 Oct 09:30 +0330");
        // negative transition before
        assert_eq!(sh("<-02>2<-01>,M3.5.0/-1,M10.5.0/0", 1774745999), "Sat 28 Mar 22:59 -02");
        // negative transition after
        assert_eq!(sh("<-02>2<-01>,M3.5.0/-1,M10.5.0/0", 1774746000), "Sun 29 Mar 00:00 -01");
        // iso helsinki summer
        assert_eq!(iso(HEL, 1791957600), "2026-10-14T09:00:00+03:00");
        // iso helsinki winter
        assert_eq!(iso(HEL, 1798761600), "2027-01-01T02:00:00+02:00");
        // iso new york
        assert_eq!(iso(NY, 1791957600), "2026-10-14T02:00:00-04:00");
        // offset text india
        assert_eq!(off("IST-5:30", 1791957600), "+05:30 False");
        // offset text helsinki dst
        assert_eq!(off(HEL, 1791957600), "+03:00 True");
        // at normal
        assert_eq!(at(HEL, 2026, 10, 14, 9, 30), "2026-10-14T06:30:00+00:00");
        // at winter
        assert_eq!(at(HEL, 2026, 12, 14, 9, 30), "2026-12-14T07:30:00+00:00");
        // at helsinki gap
        assert_eq!(at(HEL, 2026, 3, 29, 3, 30), "2026-03-29T01:30:00+00:00");
        // at helsinki overlap is the earlier
        assert_eq!(at(HEL, 2026, 10, 25, 3, 30), "2026-10-25T00:30:00+00:00");
        // at new york gap
        assert_eq!(at(NY, 2026, 3, 8, 2, 30), "2026-03-08T07:30:00+00:00");
        // at new york overlap
        assert_eq!(at(NY, 2026, 11, 1, 1, 30), "2026-11-01T05:30:00+00:00");
        // at sydney gap
        assert_eq!(at(SYD, 2026, 10, 4, 2, 30), "2026-10-03T16:30:00+00:00");
        // at sydney overlap
        assert_eq!(at(SYD, 2026, 4, 5, 2, 30), "2026-04-04T15:30:00+00:00");
        // at india
        assert_eq!(at("IST-5:30", 2026, 10, 14, 9, 30), "2026-10-14T04:00:00+00:00");
        // wall at inverse
        // dst default rule
        assert_eq!(off("EST5EDT", 1791957600), "-04:00 True");
        // dst default rule winter
        assert_eq!(off("EST5EDT", 1798761600), "-05:00 False");
        // J rule
        assert_eq!(off("AAA-1BBB,J60,J300", 1773000000), "+02:00 True");
        // N rule
        assert_eq!(off("AAA-1BBB,59,299", 1773000000), "+02:00 True");
        // hours over 24
        assert_eq!(off("AAA-1BBB,M3.5.0/25,M10.5.0/3", 1773000000), "+01:00 False");
        // explicit dst offset
        assert_eq!(off("AAA-1BBB-3,M3.5.0,M10.5.0", 1791957600), "+03:00 True");
        // reject empty
        assert_eq!(ok(""), "none");
        // reject junk
        assert_eq!(ok("garbage"), "none");
        // reject no offset
        assert_eq!(ok("EET"), "none");
        // reject short name
        assert_eq!(ok("E-2"), "none");
        // reject one rule
        assert_eq!(ok("EET-2EEST,M3.5.0"), "none");
        // reject month 13
        assert_eq!(ok("EET-2EEST,M13.1.0,M10.5.0"), "none");
        // reject rules without dst
        assert_eq!(ok("EET-2,M3.5.0,M10.5.0"), "none");
        // reject open quote
        assert_eq!(ok("<+03-3"), "none");
        // reject empty quote
        assert_eq!(ok("<>-3"), "none");
        // accept simple
        assert_eq!(ok("UTC0"), "some");
        // accept quoted
        assert_eq!(ok("<+03>-3"), "some");
        assert_eq!(TzRule::fixed(true, 7200).show(1791957600), "Wed 14 Oct 08:00 +02:00");
        assert_eq!(TzRule::fixed(false, 18000).show(1791957600), "Wed 14 Oct 01:00 -05:00");
        assert_eq!(TzRule::utc0().show(1791957600), "Wed 14 Oct 06:00 UTC");
        let h = TzRule::parse(HEL).unwrap(); assert_eq!(sh(HEL, h.at(wall(2026, 7, 1, 12, 0))), "Wed 1 Jul 12:00 EEST");
    }
    #[test] fn when_corpus_test_when_test() {
        let r = TzRule::parse(HEL).unwrap();
        assert_eq!(when_parse("2026-10-14T12:00:00+03:00", 1791957600, &r), Some(1791968400), "parse 2026-10-14T12:00:00+03:00 @1791957600");
        assert_eq!(when_parse("2026-10-14T09:00Z", 1791957600, &r), Some(1791968400), "parse 2026-10-14T09:00Z @1791957600");
        assert_eq!(when_parse("2026-10-14 15:00+0300", 1791957600, &r), Some(1791979200), "parse 2026-10-14 15:00+0300 @1791957600");
        assert_eq!(when_parse("2026-10-14T15:00:30-05:00", 1791957600, &r), Some(1792008030), "parse 2026-10-14T15:00:30-05:00 @1791957600");
        assert_eq!(when_parse("2026-10-14T18:00", 1791957600, &r), Some(1791990000), "parse 2026-10-14T18:00 @1791957600");
        assert_eq!(when_parse("2026-10-14 18:00", 1791957600, &r), Some(1791990000), "parse 2026-10-14 18:00 @1791957600");
        assert_eq!(when_parse("2026-10-15", 1791957600, &r), Some(1792044000), "parse 2026-10-15 @1791957600");
        assert_eq!(when_parse("2026-10-13T09:00", 1791957600, &r), None, "parse 2026-10-13T09:00 @1791957600");
        assert_eq!(when_parse("2026-10-14T05:00Z", 1791957600, &r), None, "parse 2026-10-14T05:00Z @1791957600");
        assert_eq!(when_parse("2026-02-30", 1791957600, &r), None, "parse 2026-02-30 @1791957600");
        assert_eq!(when_parse("in 20m", 1791957600, &r), Some(1791958800), "parse in 20m @1791957600");
        assert_eq!(when_parse("in 2h", 1791957600, &r), Some(1791964800), "parse in 2h @1791957600");
        assert_eq!(when_parse("in 1h30m", 1791957600, &r), Some(1791963000), "parse in 1h30m @1791957600");
        assert_eq!(when_parse("in 3d", 1791957600, &r), Some(1792216800), "parse in 3d @1791957600");
        assert_eq!(when_parse("in 90 minutes", 1791957600, &r), Some(1791963000), "parse in 90 minutes @1791957600");
        assert_eq!(when_parse("in 2 hours", 1791957600, &r), Some(1791964800), "parse in 2 hours @1791957600");
        assert_eq!(when_parse("in 1 day", 1791957600, &r), Some(1792044000), "parse in 1 day @1791957600");
        assert_eq!(when_parse("in 0m", 1791957600, &r), None, "parse in 0m @1791957600");
        assert_eq!(when_parse("in", 1791957600, &r), None, "parse in @1791957600");
        assert_eq!(when_parse("in xyz", 1791957600, &r), None, "parse in xyz @1791957600");
        assert_eq!(when_parse("in 5", 1791957600, &r), None, "parse in 5 @1791957600");
        assert_eq!(when_parse("today 17:30", 1791957600, &r), Some(1791988200), "parse today 17:30 @1791957600");
        assert_eq!(when_parse("today 08:00", 1791957600, &r), None, "parse today 08:00 @1791957600");
        assert_eq!(when_parse("tomorrow 9am", 1791957600, &r), Some(1792044000), "parse tomorrow 9am @1791957600");
        assert_eq!(when_parse("tomorrow 09:00", 1791957600, &r), Some(1792044000), "parse tomorrow 09:00 @1791957600");
        assert_eq!(when_parse("tomorrow", 1791957600, &r), Some(1792044000), "parse tomorrow @1791957600");
        assert_eq!(when_parse("monday 9am", 1791957600, &r), Some(1792389600), "parse monday 9am @1791957600");
        assert_eq!(when_parse("wed 18:00", 1791957600, &r), Some(1791990000), "parse wed 18:00 @1791957600");
        assert_eq!(when_parse("wed 8am", 1791957600, &r), Some(1792558800), "parse wed 8am @1791957600");
        assert_eq!(when_parse("mon 09:30", 1791957600, &r), Some(1792391400), "parse mon 09:30 @1791957600");
        assert_eq!(when_parse("Friday", 1791957600, &r), Some(1792130400), "parse Friday @1791957600");
        assert_eq!(when_parse("9am", 1791957600, &r), Some(1792044000), "parse 9am @1791957600");
        assert_eq!(when_parse("21:00", 1791957600, &r), Some(1792000800), "parse 21:00 @1791957600");
        assert_eq!(when_parse("9:30pm", 1791957600, &r), Some(1792002600), "parse 9:30pm @1791957600");
        assert_eq!(when_parse("12am", 1791957600, &r), Some(1792011600), "parse 12am @1791957600");
        assert_eq!(when_parse("12pm", 1791957600, &r), Some(1791968400), "parse 12pm @1791957600");
        assert_eq!(when_parse("  Tomorrow   9AM ", 1791957600, &r), Some(1792044000), "parse   Tomorrow   9AM  @1791957600");
        assert_eq!(when_parse("tomorrow at 9am", 1791957600, &r), Some(1792044000), "parse tomorrow at 9am @1791957600");
        assert_eq!(when_parse("tomorrow 9am", 1792843200, &r), Some(1792911600), "parse tomorrow 9am @1792843200");
        assert_eq!(when_parse("tomorrow 03:30", 1792843200, &r), Some(1792888200), "parse tomorrow 03:30 @1792843200");
        assert_eq!(when_parse("tomorrow 03:30", 1774699200, &r), Some(1774747800), "parse tomorrow 03:30 @1774699200");
        assert_eq!(when_parse("", 1791957600, &r), None, "parse  @1791957600");
        assert_eq!(when_parse("banana", 1791957600, &r), None, "parse banana @1791957600");
        assert_eq!(when_parse("tomorrow banana", 1791957600, &r), None, "parse tomorrow banana @1791957600");
        assert_eq!(when_parse("25:00", 1791957600, &r), None, "parse 25:00 @1791957600");
        assert_eq!(when_parse("9", 1791957600, &r), None, "parse 9 @1791957600");
        assert_eq!(when_parse("13pm", 1791957600, &r), None, "parse 13pm @1791957600");
        assert_eq!(when_parse("today 9am tomorrow", 1791957600, &r), None, "parse today 9am tomorrow @1791957600");
    }
    fn show_ns(v: &[u64]) -> String { v.iter().map(u64::to_string).collect::<Vec<_>>().join(",") }
    fn show_cron(s: &str) -> String { Cron::parse(s).map_or("none".into(), |c| format!("{} | {} | {} | {} | {} | {} {}", show_ns(&c.mins), show_ns(&c.hours), show_ns(&c.doms), show_ns(&c.months), show_ns(&c.dows), if c.dom_any { "True" } else { "False" }, if c.dow_any { "True" } else { "False" })) }
    fn nx(s: &str, after: u64, off: u64, east: bool) -> String { Cron::parse(s).map_or("invalid".into(), |c| c.next(after, off, east).to_string()) }
    fn due_str(s: &str, last: u64, now: u64) -> &'static str { match Cron::parse(s) { None => "invalid", Some(c) => if c.due(last, now, 0, true) { "True" } else { "False" } } }
    #[test] fn cron_corpus_test_cron_test() {
        assert_eq!(show_cron("* * * * *"), "0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 | 1,2,3,4,5,6,7,8,9,10,11,12 | 0,1,2,3,4,5,6 | True True", "parse star");
        assert_eq!(show_cron("*/15 9-17/4 1,15 */3 1-5"), "0,15,30,45 | 9,13,17 | 1,15 | 1,4,7,10 | 1,2,3,4,5 | False False", "parse lists ranges steps");
        assert_eq!(show_cron("30,5,5,10-12 0 * * *"), "5,10,11,12,30 | 0 | 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 | 1,2,3,4,5,6,7,8,9,10,11,12 | 0,1,2,3,4,5,6 | True True", "parse sorted once");
        assert_eq!(show_cron("0 0 * JAN,mar-may Mon-Fri"), "0 | 0 | 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 | 1,3,4,5 | 1,2,3,4,5 | True False", "parse names");
        assert_eq!(show_cron("0 0 * * 7"), "0 | 0 | 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 | 1,2,3,4,5,6,7,8,9,10,11,12 | 0 | True False", "parse dow 7 is sunday");
        assert_eq!(show_cron("50/5 0 1 1 *"), "50,55 | 0 | 1 | 1 | 0,1,2,3,4,5,6 | False True", "parse A/S");
        assert_eq!(Cron::parse("60 * * * *").is_some(), false, "reject minute 60");
        assert_eq!(Cron::parse("0 24 * * *").is_some(), false, "reject hour 24");
        assert_eq!(Cron::parse("0 0 0 * *").is_some(), false, "reject dom 0");
        assert_eq!(Cron::parse("0 0 32 * *").is_some(), false, "reject dom 32");
        assert_eq!(Cron::parse("0 0 * 13 *").is_some(), false, "reject month 13");
        assert_eq!(Cron::parse("0 0 * * 8").is_some(), false, "reject dow 8");
        assert_eq!(Cron::parse("0 0 * *").is_some(), false, "reject four fields");
        assert_eq!(Cron::parse("0 0 * * * *").is_some(), false, "reject six fields");
        assert_eq!(Cron::parse("*/0 * * * *").is_some(), false, "reject step 0");
        assert_eq!(Cron::parse("5-1 * * * *").is_some(), false, "reject backwards range");
        assert_eq!(Cron::parse("a b c d e").is_some(), false, "reject junk");
        assert_eq!(Cron::parse("1,,2 * * * *").is_some(), false, "reject empty item");
        assert_eq!(Cron::parse("0 0 * foo *").is_some(), false, "reject bad name");
        assert_eq!(Cron::parse("").is_some(), false, "reject empty");
        assert_eq!(Cron::parse("@often").is_some(), false, "reject unknown alias");
        assert_eq!(Cron::parse("every 0m").is_some(), false, "reject every 0m");
        assert_eq!(Cron::parse("every 60m").is_some(), false, "reject every 60m");
        assert_eq!(Cron::parse("every 24h").is_some(), false, "reject every 24h");
        assert_eq!(Cron::parse("daily 25:00").is_some(), false, "reject daily 25:00");
        assert_eq!(Cron::parse("daily 9").is_some(), false, "reject daily 9");
        assert_eq!(Cron::parse("0 0 31 2 *").is_some(), true, "accept impossible date");
        assert_eq!(Cron::text("@hourly"), "0 * * * *", "text @hourly");
        assert_eq!(Cron::text("@daily"), "0 0 * * *", "text @daily");
        assert_eq!(Cron::text("@Midnight"), "0 0 * * *", "text @midnight");
        assert_eq!(Cron::text("@weekly"), "0 0 * * 0", "text @weekly");
        assert_eq!(Cron::text("@monthly"), "0 0 1 * *", "text @monthly");
        assert_eq!(Cron::text("@yearly"), "0 0 1 1 *", "text @yearly");
        assert_eq!(Cron::text("@annually"), "0 0 1 1 *", "text @annually");
        assert_eq!(Cron::text("every 15m"), "*/15 * * * *", "text every 15m");
        assert_eq!(Cron::text("Every 2h"), "0 */2 * * *", "text every 2h");
        assert_eq!(Cron::text("daily 09:30"), "30 9 * * *", "text daily");
        assert_eq!(Cron::text("weekdays 08:00"), "0 8 * * 1-5", "text weekdays");
        assert_eq!(Cron::text("  0   5 * *  *  "), "0 5 * * *", "text spaces");
        assert_eq!(Cron::text("61 * * * *"), "", "text invalid");
        assert_eq!(nx("every 15m", 1790244420, 0, true), "1790244900", "next every 15m");
        assert_eq!(nx("every 15m", 1790244900, 0, true), "1790245800", "next every 15m on the boundary");
        assert_eq!(nx("* * * * *", 1790244420, 0, true), "1790244480", "next every minute");
        assert_eq!(nx("* * * * *", 1790244450, 0, true), "1790244480", "next mid-minute");
        assert_eq!(nx("@hourly", 1790244420, 0, true), "1790247600", "next hourly");
        assert_eq!(nx("every 2h", 1790244420, 0, true), "1790251200", "next every 2h");
        assert_eq!(nx("daily 09:30", 1790244420, 0, true), "1790328600", "next daily UTC");
        assert_eq!(nx("daily 09:30", 1790244420, 19800, true), "1790308800", "next daily +05:30");
        assert_eq!(nx("daily 09:30", 1790244420, 25200, false), "1790267400", "next daily -07:00");
        assert_eq!(nx("weekdays 08:00", 1790328600, 0, true), "1790582400", "next weekdays skips weekend");
        assert_eq!(nx("weekdays 08:00", 1790424000, 0, true), "1790582400", "next weekdays from saturday");
        assert_eq!(nx("0 0 31 * *", 1790244420, 0, true), "1793404800", "next monthly 31st skips september");
        assert_eq!(nx("0 0 31 * *", 1793404800, 0, true), "1798675200", "next monthly 31st skips november");
        assert_eq!(nx("0 0 29 2 *", 1790244420, 0, true), "1835395200", "next leap day");
        assert_eq!(nx("@yearly", 1790244420, 0, true), "1798761600", "next @yearly");
        assert_eq!(nx("0 0 * * 7", 1790244420, 0, true), "1790467200", "next sunday as 7");
        assert_eq!(nx("0 0 * * SUN", 1790244420, 0, true), "1790467200", "next sunday by name");
        assert_eq!(nx("0 0 13 * 5", 1790244420, 0, true), "1790294400", "next dom or dow: friday first");
        assert_eq!(nx("0 0 13 * 5", 1791590400, 0, true), "1791849600", "next dom or dow: 13th first");
        assert_eq!(nx("0 0 13 * *", 1790244420, 0, true), "1791849600", "next dom only");
        assert_eq!(nx("0 0 * * fri", 1791590400, 0, true), "1792108800", "next dow only");
        assert_eq!(nx("0 9 * jan mon", 1790244420, 0, true), "1799053200", "next months and dow");
        assert_eq!(nx("0 0 31 2 *", 1790244420, 0, true), "0", "next impossible");
        assert_eq!(nx("0 0 31 4 *", 1790244420, 0, true), "0", "next impossible april 31");
        assert_eq!(due_str("every 15m", 1790244420, 1790244600), "False", "due not yet");
        assert_eq!(due_str("every 15m", 1790244420, 1790244900), "True", "due at the time");
        assert_eq!(due_str("every 15m", 1790244420, 1790254800), "True", "due once after downtime");
        assert_eq!(due_str("every 15m", 1790254800, 1790254800), "False", "not due again at once");
        assert_eq!(due_str("every 15m", 1790254800, 1790255000), "False", "not due before the next");
        assert_eq!(due_str("0 0 31 2 *", 1790244420, 1835395200), "False", "due never when impossible");
    }

    // ---- sched ----
    use super::super::world::World;
    use serde_json::json;
    fn set(k: &str, v: &str) -> String { json!({"$":"SettingSet","key":k,"value":v,"at":1}).to_string() }
    fn base() -> Vec<String> { vec![set("tz.rule.Europe/Helsinki", HEL), set("tz.rule.America/New_York", "EST5EDT,M3.2.0,M11.1.0"), set("tz.rule.Asia/Tokyo", "JST-9"), set("person.tz", "Europe/Helsinki")] }
    fn mk(id: &str, cron: &str, tz: &str, once: u64, kind: &str, created: u64) -> Vec<String> {
        vec![json!({"$":"RoutineSet","id":id,"bot":"b1","name":"R","cron":cron,"prompt":"p","on":true,"at":created}).to_string(), json!({"$":"RoutinePlan","id":id,"once":once,"tz":tz,"kind":kind,"at":created}).to_string()]
    }
    fn world(cs: &[String]) -> World { let mut w = World::default(); for l in base().iter().chain(cs) { w.apply_line(l) } w }
    fn plan<'a>(w: &'a World, id: &str) -> Plan<'a> { let r = &w.routines[id]; Plan { cron: &r.cron, on: r.on, deleted: r.deleted, done: r.done, last: r.last, once: r.once, tz: &r.tz, slot: r.slot } }
    fn due(cs: &[String], id: &str, now: u64) -> bool { let w = world(cs); let z = Zones { settings: &w.settings }; plan(&w, id).due(&z, now, 0, true) }
    /// the changes the tick logs when a routine fires at `now`
    fn fire(cs: &[String], id: &str, now: u64) -> Vec<String> {
        let w = world(cs); let z = Zones { settings: &w.settings }; let wall = plan(&w, id).wall(&z, now);
        let mut v = cs.to_vec(); v.push(json!({"$":"RoutineFired","id":id,"at":now}).to_string()); v.push(json!({"$":"RoutineSlot","id":id,"slot":wall,"at":now}).to_string()); v
    }
    fn cat(a: Vec<String>, b: Vec<String>) -> Vec<String> { a.into_iter().chain(b).collect() }
    fn next(cs: &[String], id: &str, now: u64) -> u64 { let w = world(cs); let z = Zones { settings: &w.settings }; plan(&w, id).next(&z, now, 0, true) }
    fn zone_str(cs: &[String]) -> String { let mut w = World::default(); for l in cs { w.apply_line(l) } let z = Zones { settings: &w.settings };
        let src = if !w.setting("person.tz", "").is_empty() { "set" } else if !w.setting("person.tz.device", "").is_empty() { "phone" } else if !w.setting("tz.machine", "").is_empty() { "machine" } else { "utc" }; format!("{}/{src}", z.person()) }

    #[test] fn sched_zone_person_and_rules() {
        assert_eq!(zone_str(&[]), "UTC/utc");
        assert_eq!(zone_str(&[set("tz.machine", "Europe/London")]), "Europe/London/machine");
        assert_eq!(zone_str(&[set("tz.machine", "Europe/London"), set("person.tz.device", "Asia/Tokyo")]), "Asia/Tokyo/phone");
        assert_eq!(zone_str(&[set("person.tz.device", "Asia/Tokyo"), set("person.tz", "Europe/Helsinki")]), "Europe/Helsinki/set");
        assert_eq!(zone_str(&[set("person.tz.device", "Asia/Tokyo"), set("person.tz", "Europe/Helsinki"), set("person.tz", "")]), "Asia/Tokyo/phone");
        let w = world(&[]); let z = Zones { settings: &w.settings };
        assert_eq!(z.person(), "Europe/Helsinki"); assert!(z.rule_of("Asia/Tokyo").is_some() && z.rule("UTC").is_some() && z.rule("Mars/Base").is_none() && z.rule("Europe/Helsinki").is_some());
        assert!(Zones::valid("Europe/Helsinki") && !Zones::valid("a b") && !Zones::valid("") && !Zones::valid("a//b"));
    }
    #[test] fn sched_zoned_due_at_the_zones_wall_clock() {
        let r1 = |created| mk("r1", "30 9 * * *", "person", 0, "", created);
        assert!(!due(&r1(1791892800), "r1", 1791959399)); assert!(due(&r1(1791892800), "r1", 1791959400), "summer 09:30");
        assert!(!due(&r1(1797163200), "r1", 1797233399)); assert!(due(&r1(1797163200), "r1", 1797233400), "winter 09:30");
        assert!(!due(&mk("r1", "30 9 * * *", "Asia/Tokyo", 0, "", 1791939600), "r1", 1791959400), "named zone, not the person's");
        assert!(due(&mk("r1", "30 9 * * *", "Asia/Tokyo", 0, "", 1791939600), "r1", 1792024200));
        assert!(!due(&mk("r1", "30 9 * * *", "Mars/Base", 0, "", 1791892800), "r1", 1791959400), "a zone with no rule waits");
    }
    #[test] fn sched_zoned_once_per_slot_fired() {
        // law sched_zoned_once_per_slot: a routine that fired is not due again in the same slot
        let f = fire(&mk("r1", "30 9 * * *", "person", 0, "", 1791892800), "r1", 1791959400);
        assert!(!due(&f, "r1", 1791959460)); assert!(!due(&f, "r1", 1791980000)); assert!(due(&f, "r1", 1792045800), "next day");
        let west = cat(f.clone(), vec![set("person.tz", "America/New_York")]);
        assert!(!due(&west, "r1", 1791959460)); assert!(!due(&west, "r1", 1791984600)); assert!(due(&west, "r1", 1792071000));
        let hourly = cat(fire(&mk("r1", "30 * * * *", "person", 0, "", 1791892800), "r1", 1791959400), vec![set("person.tz", "Asia/Tokyo")]);
        assert!(due(&hourly, "r1", 1791959460), "east: one catch-up");
    }
    #[test] fn sched_dst_fall_back_and_spring_forward() {
        let c = || mk("r1", "30 3 * * *", "person", 0, "", 1792843200);
        assert!(due(&c(), "r1", 1792888200)); assert!(!due(&c(), "r1", 1792888199));
        let f = fire(&c(), "r1", 1792888200);
        assert!(!due(&f, "r1", 1792891800)); assert!(!due(&f, "r1", 1792893540)); assert!(due(&f, "r1", 1792978200));
        let s = || mk("r1", "30 3 * * *", "person", 0, "", 1774699200);
        assert!(!due(&s(), "r1", 1774745999)); assert!(due(&s(), "r1", 1774746000));
        assert!(!due(&fire(&s(), "r1", 1774746000), "r1", 1774747800));
    }
    #[test] fn sched_once_due_and_done_never() {
        // laws sched_once_due, sched_done_never
        let o = || mk("r1", "", "person", 1791979200, "", 1791892800);
        assert!(!due(&o(), "r1", 1791979199)); assert!(due(&o(), "r1", 1791979200)); assert!(due(&o(), "r1", 1792500000));
        let done = cat(o(), vec![json!({"$":"RoutineFired","id":"r1","at":1791979200}).to_string(), json!({"$":"RoutineDone","id":"r1","at":1791979200}).to_string()]);
        assert!(!due(&done, "r1", 1792500000));
        let again = cat(o(), vec![json!({"$":"RoutineDone","id":"r1","at":1791979200}).to_string(), json!({"$":"RoutinePlan","id":"r1","once":1792500000,"tz":"person","kind":"","at":1791979300}).to_string()]);
        assert!(due(&again, "r1", 1792500000), "set again with a new time is pending");
        let off = cat(o(), vec![json!({"$":"RoutineSet","id":"r1","bot":"b1","name":"R","cron":"","prompt":"p","on":false,"at":1791900000}).to_string()]);
        assert!(!due(&off, "r1", 1791979200));
        let edit = cat(o(), vec![json!({"$":"RoutineSet","id":"r1","bot":"b1","name":"R2","cron":"","prompt":"p","on":true,"at":1791900000}).to_string()]);
        assert!(due(&edit, "r1", 1791979200), "an edit without a plan keeps the plan");
    }
    #[test] fn sched_legacy_same_offset_cron_cases() {
        // law sched_legacy_same: a routine with no zone is a cron line in the machine's offset
        let cs = mk("r1", "30 9 * * *", "", 0, "", 1790244420);
        let w = world(&cs); let z = Zones { settings: &w.settings }; let p = plan(&w, "r1"); let c = Cron::parse("30 9 * * *").unwrap();
        for now in [1790244421u64, 1790280000, 1790284200, 1790330600] { assert_eq!(p.due(&z, now, 0, true), c.due(1790244420, now, 0, true)); }
        assert_eq!(p.slot_next(&z, 0, true), c.next(1790244420, 0, true));
    }
    #[test] fn sched_next_is_the_utc_instant_of_the_next_fire() {
        assert_eq!(next(&mk("r1", "30 9 * * *", "person", 0, "", 1791892800), "r1", 1791892800), 1791959400);
        assert_eq!(next(&mk("r1", "", "person", 1791979200, "", 1791892800), "r1", 1791892800), 1791979200);
        assert_eq!(next(&mk("r1", "30 9 * * *", "Mars/Base", 0, "", 1791892800), "r1", 1791892800), 0);
    }

    // ---- the property laws, over a grid of samples ----
    const CRONS: [&str; 7] = ["* * * * *", "*/15 * * * *", "30 9 * * *", "0 0 1 * *", "0 8 * * 1-5", "59 23 31 12 *", "5 4 29 2 *"];
    const TIMES: [u64; 9] = [0, 59, 60, 1000, 1790244420, 1791957600, 1774745999, 1792888200, 1798761600];
    #[test] fn cron_next_after() {
        // law cron_next_after: a routine's next time is after the time asked about, or 0 (never)
        for c in CRONS { let c = Cron::parse(c).unwrap(); for after in TIMES { for (off, east) in [(0, true), (7200, true), (18000, false)] { let x = c.next(after, off, east); assert!(x == 0 || x > after, "{x} {after}") } } }
    }
    #[test] fn cron_due_fires_once() {
        // law cron_due_fires_once: once a routine fires at now (last := now) it is not due again at now
        for c in CRONS { let c = Cron::parse(c).unwrap(); for now in TIMES { for (off, east) in [(0, true), (7200, true), (18000, false)] { assert!(!c.due(now, now, off, east)) } } }
    }
    #[test] fn tz_fixed_wall() {
        // law tz_fixed_wall: a fixed zone's wall clock is the snooze offset
        for (east, secs) in [(true, 0), (true, 7200), (false, 18000), (false, 99999999)] { for t in TIMES { assert_eq!(TzRule::fixed(east, secs).wall(t), local(east, t, secs)) } }
    }
    #[test] fn sched_done_never() {
        // law sched_done_never: a done routine is never due
        let w = World::default(); let z = Zones { settings: &w.settings };
        for (cron, once, tz) in [("* * * * *", 0, ""), ("* * * * *", 5, ""), ("* * * * *", 0, "UTC")] { for now in TIMES { assert!(!Plan { cron, on: true, deleted: false, done: true, last: 0, once, tz, slot: 0 }.due(&z, now, 0, true)) } }
    }
    #[test] fn sched_once_due() {
        // law sched_once_due: a pending one-time routine is due exactly when its time has come
        let w = World::default(); let z = Zones { settings: &w.settings };
        for once in [1u64, 1000, 1791979200] { for now in [0, 999, 1000, 1001, 1791979200, 1791979201] { for tz in ["", "person", "UTC"] {
            assert_eq!(Plan { cron: "", on: true, deleted: false, done: false, last: 0, once, tz, slot: 0 }.due(&z, now, 0, true), now >= once) } } }
    }
    #[test] fn sched_legacy_same() {
        // law sched_legacy_same: a routine with no plan (the machine's offset, repeating) is due exactly when its cron line is
        let w = World::default(); let z = Zones { settings: &w.settings };
        for c in CRONS { for last in TIMES { for now in TIMES { for (mo, me) in [(0, true), (7200, true), (18000, false)] {
            let p = Plan { cron: c, on: true, deleted: false, done: false, last, once: 0, tz: "", slot: 0 };
            assert_eq!(p.due(&z, now, mo, me), Cron::parse(c).unwrap().due(last, now, mo, me)) } } } }
    }
    #[test] fn sched_zoned_once_per_slot() {
        // law sched_zoned_once_per_slot: a zoned routine that fired at wall clock `slot` is not due at any later instant whose wall clock is no later
        let rule = TzRule::parse(HEL).unwrap();
        for c in CRONS { let c = Cron::parse(c).unwrap(); for slot in [1u64, 1791959400, 1791892800] { for last in TIMES { for now2 in TIMES {
            if rule.wall(now2) <= slot { assert!(!c.due(super::base(&rule, last, slot), rule.wall(now2), 0, true)) } } } } }
    }
    #[test] fn when_in_minutes() { assert_eq!(when_parse("in 20m", 1000, &TzRule::utc0()), Some(2200)); }
    #[test] fn when_in_hours() { assert_eq!(when_parse("in 2h", 1000, &TzRule::utc0()), when_parse("in 120m", 1000, &TzRule::utc0())); }
}
