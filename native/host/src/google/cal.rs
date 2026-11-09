//! Google Calendar (v3), pure, as 0.14's core/google.bend: the primary calendar's paths, an event list an agent reads, the bodies of insert and patch
//! (alerts, attendees, all-day ends), the arguments as they will run (`resolve`: a start alone keeps the event's length; whether guests are told), and the
//! words the person approves for a create, a change or a delete.
use super::oauth::enc;
use serde_json::{json, Map, Value};

const BASE: &str = "/calendar/v3/calendars/primary/events";

// ---- dates (UTC, proleptic Gregorian; H. Hinnant's days from civil) ----
pub fn days(y: i64, m: i64, d: i64) -> i64 { let y = if m <= 2 { y - 1 } else { y }; let era = y.div_euclid(400); let yoe = y - era * 400; let mp = (m + 9) % 12; let doy = (153 * mp + 2) / 5 + d - 1; let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; era * 146097 + doe - 719468 }
pub fn civil(z: i64) -> (i64, i64, i64) { let z = z + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097; let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let y = yoe + era * 400; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153; let d = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 }; (if m <= 2 { y + 1 } else { y }, m, d) }
fn num(s: &str, at: usize, n: usize) -> i64 { s.get(at..at + n).and_then(|x| x.parse().ok()).unwrap_or(0) }
fn ymd((y, m, d): (i64, i64, i64)) -> String { format!("{y:04}-{m:02}-{d:02}") }
/// Date.iso: epoch seconds as RFC 3339 UTC
pub fn iso(t: u64) -> String { let t = t as i64; let s = t.rem_euclid(86400); format!("{}T{:02}:{:02}:{:02}Z", ymd(civil(t.div_euclid(86400))), s / 3600, s % 3600 / 60, s % 60) }
pub fn plus_days(s: &str, n: i64) -> String { ymd(civil(days(num(s, 0, 4), num(s, 5, 2), num(s, 8, 2)) + n)) }
/// Date.time: a bare date as the start of that day (UTC); anything else as given
pub fn time(s: &str) -> String { if s.len() == 10 { format!("{s}T00:00:00Z") } else { s.into() } }

pub fn list_path(from: &str, to: &str, max: u64, q: &str) -> String {
    format!("{BASE}?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime&maxResults={max}{}", enc(&time(from)), enc(&time(to)), if q.is_empty() { String::new() } else { format!("&q={}", enc(q)) })
}
pub fn insert_path(any: bool) -> String { if any { format!("{BASE}?sendUpdates=all") } else { BASE.into() } }
pub fn event_path(id: &str) -> String { format!("{BASE}/{}", enc(id)) }
pub fn patch_path(id: &str, any: bool) -> String { format!("{}?sendUpdates={}", event_path(id), if any { "all" } else { "none" }) }
/// Cal.id_ok: not empty, no '/' and no whitespace
pub fn id_ok(id: &str) -> bool { !id.is_empty() && !id.contains('/') && !id.chars().any(char::is_whitespace) }
pub fn gone(code: &str) -> bool { code == "404" || code == "410" }
pub fn nosuch(id: &str) -> String { format!("no event with id {id}") }
/// Gt.until: the end of a window: given, else a week after from (or now)
pub fn until(from: &str, to: &str, now: u64) -> String { if !to.is_empty() { to.into() } else if from.is_empty() { iso(now + 604800) } else { time(&plus_days(from, 7)) } }

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn when(o: &Value) -> String { let d = s(o, "dateTime"); if d.is_empty() { s(o, "date").into() } else { d.into() } }
fn n_att(e: &Value) -> usize { e["attendees"].as_array().map_or(0, |a| a.len()) }
/// Google.event_line
pub fn event_line(e: &Value) -> String {
    let title = if s(e, "summary").is_empty() { "(no title)" } else { s(e, "summary") };
    format!("- {} -> {}  {title}{}\n  id {}{}", when(&e["start"]), when(&e["end"]), if s(e, "location").is_empty() { String::new() } else { format!("\n  at {}", s(e, "location")) }, s(e, "id"),
        if n_att(e) == 0 { String::new() } else { format!(", {} attendees", n_att(e)) })
}
pub fn events(j: &Value) -> String {
    let es = j["items"].as_array().cloned().unwrap_or_default(); let tz = s(j, "timeZone");
    if es.is_empty() { return "No events.".into() }
    format!("{} events{}:\n{}", es.len(), if tz.is_empty() { String::new() } else { format!(" (calendar time zone {tz})") }, es.iter().map(event_line).collect::<Vec<_>>().join("\n"))
}
pub fn created(e: &Value) -> String { format!("Created:\n{}{}", event_line(e), if s(e, "htmlLink").is_empty() { String::new() } else { format!("\n  {}", s(e, "htmlLink")) }) }
pub fn updated(e: &Value) -> String { format!("Updated:\n{}", event_line(e)) }

/// Cal.emails: attendees as a JSON array of emails, or one comma-separated string
pub fn emails(v: Option<&Value>) -> Vec<String> {
    let raw: Vec<String> = match v { Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(), Some(Value::String(t)) => t.split(',').map(str::to_string).collect(), _ => vec![] };
    raw.into_iter().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}
fn at(all_day: bool, v: &str, tz: &str) -> Value { if all_day { json!({"date": v.chars().take(10).collect::<String>()}) } else if tz.is_empty() { json!({"dateTime": v}) } else { json!({"dateTime": v, "timeZone": tz}) } }
/// Cal.end: an all-day event ends (exclusively) after its start: the next day when the end given is not later
fn end_of(all_day: bool, start: &str, end: &str) -> String { let (s10, e10): (String, String) = (start.chars().take(10).collect(), end.chars().take(10).collect()); if all_day && e10 <= s10 { plus_days(&s10, 1) } else { end.into() } }
fn who(xs: &[String]) -> Value { Value::Array(xs.iter().map(|a| json!({"email": a})).collect()) }
/// Cal.rem1: a bare number is a popup that many minutes ahead; minutes at most 28 days
fn rem1(r: &Value) -> Value {
    let (m, n) = match r { Value::Number(x) => ("popup", x.as_u64().unwrap_or(0)), o => (if s(o, "method") == "email" { "email" } else { "popup" }, o["minutes"].as_u64().unwrap_or(10)) };
    json!({"method": m, "minutes": n.min(40320)})
}
/// Cal.reminders: given → the overrides (none for []); omitted → nothing. default_reminders: true gives the calendar's own back.
fn reminders(a: &Value) -> Option<Value> {
    if a.get("default_reminders").and_then(Value::as_bool).unwrap_or(false) { return Some(json!({"useDefault": true})) }
    let xs = a.get("reminders")?.as_array()?; let os: Vec<Value> = xs.iter().take(5).map(rem1).collect();
    Some(if os.is_empty() { json!({"useDefault": false}) } else { json!({"useDefault": false, "overrides": os}) })
}
/// Cal.check: alerts Google would refuse, before the person is asked anything
pub fn check(a: &Value) -> Option<String> {
    let one = |r: &Value| -> Option<String> {
        let mins = |m: Option<&Value>| match m { Some(Value::Number(n)) => match n.as_u64() { Some(x) if x <= 40320 => None, Some(_) => Some("an alert's minutes are 0 to 40320".to_string()), None => Some("an alert's minutes are a whole number".to_string()) }, Some(_) => Some("an alert's minutes are a whole number".to_string()), None => Some("each alert needs its minutes".to_string()) };
        match r { Value::Number(_) => mins(Some(r)), Value::Object(_) => { let m = r.get("method").and_then(Value::as_str).unwrap_or("popup"); if m != "popup" && m != "email" { Some("an alert's method is popup or email".into()) } else { mins(r.get("minutes")) } } _ => Some("each alert is {method, minutes} or a number of minutes".into()) }
    };
    match a.get("reminders") { None | Some(Value::Null) => None, Some(Value::Array(xs)) => if xs.len() > 5 { Some("at most 5 alerts".into()) } else { xs.iter().find_map(one) }, Some(_) => Some("reminders is a list".into()) }
}
/// calendar_create's arguments as events.insert's body
pub fn create_body(a: &Value) -> Value {
    let all = a.get("all_day").and_then(Value::as_bool).unwrap_or(false); let tz = s(a, "timeZone").trim().to_string();
    let (st, en) = (s(a, "start").trim().to_string(), s(a, "end").trim().to_string());
    let mut o = Map::new();
    o.insert("summary".into(), json!(if s(a, "summary").is_empty() { "(no title)" } else { s(a, "summary") }));
    for k in ["description", "location"] { if !s(a, k).is_empty() { o.insert(k.into(), json!(s(a, k))); } }
    o.insert("start".into(), at(all, &st, &tz)); o.insert("end".into(), at(all, &end_of(all, &st, &en), &tz));
    let ws = emails(a.get("attendees")); if !ws.is_empty() { o.insert("attendees".into(), who(&ws)); }
    if let Some(r) = reminders(a) { o.insert("reminders".into(), r); }
    Value::Object(o)
}
/// events.patch's body: only what was given
pub fn patch_body(a: &Value) -> Value {
    let mut o = Map::new();
    for k in ["summary", "description", "location"] { if let Some(Value::String(v)) = a.get(k) { o.insert(k.into(), json!(v)); } }
    let all = a.get("all_day").and_then(Value::as_bool).unwrap_or(false); let tz = s(a, "timeZone").trim().to_string();
    let (st, en) = (s(a, "start").trim().to_string(), s(a, "end").trim().to_string());
    if !st.is_empty() { o.insert("start".into(), at(all, &st, &tz)); }
    if !en.is_empty() || (all && !st.is_empty()) { o.insert("end".into(), at(all, &if st.is_empty() { en.clone() } else { end_of(all, &st, &en) }, &tz)); }
    if a.get("attendees").is_some() { o.insert("attendees".into(), who(&emails(a.get("attendees")))); }
    if let Some(r) = reminders(a) { o.insert("reminders".into(), r); }
    Value::Object(o)
}

// wall-clock arithmetic ("2026-10-14T09:00:00-07:00")
fn local(t: &str) -> i64 { days(num(t, 0, 4), num(t, 5, 2), num(t, 8, 2)) * 86400 + num(t, 11, 2) * 3600 + num(t, 14, 2) * 60 + num(t, 17, 2) }
fn suf(t: &str) -> String { if t.ends_with('Z') { return "Z".into() } let n = t.len(); if n > 19 { let tail = &t[n - 6..]; if tail.starts_with('+') || tail.starts_with('-') { return tail.into() } } String::new() }
fn utc(t: &str) -> i64 { let f = suf(t); let off = num(&f, 1, 2) * 3600 + num(&f, 4, 2) * 60; if f.starts_with('-') { local(t) + off } else { local(t) - off } }
/// Cal.shifted: the end of an event of old_s..old_e's length that starts at new_s
pub fn shifted(old_s: &str, old_e: &str, new_s: &str) -> String { let t = local(new_s) + (utc(old_e) - utc(old_s)); let i = iso(t.max(0) as u64); format!("{}{}", &i[..i.len() - 1], suf(new_s)) }
/// Cal.resolve: the arguments as they will run (a start without an end keeps the fetched event's length; `_notify` whether guests are told)
pub fn resolve(old: &Value, a: &Value) -> Value {
    let (st, en) = (s(a, "start").trim(), s(a, "end").trim());
    let need = !st.is_empty() && en.is_empty() && !a.get("all_day").and_then(Value::as_bool).unwrap_or(false) && !s(&old["start"], "dateTime").is_empty();
    let told = n_att(old) > 0 || !emails(a.get("attendees")).is_empty();
    let mut o = a.as_object().cloned().unwrap_or_default(); o.insert("_notify".into(), json!(told));
    if need { o.insert("end".into(), json!(shifted(s(&old["start"], "dateTime"), s(&old["end"], "dateTime"), st))); }
    Value::Object(o)
}
pub fn told(a: &Value) -> bool { a.get("_notify").and_then(Value::as_bool).unwrap_or(false) }
pub fn nothing(a: &Value) -> bool { patch_body(a).as_object().map_or(true, |o| o.is_empty()) }

// ---- what the person reads ----
fn day(t: &str) -> String {
    const DOW: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]; const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let z = days(num(t, 0, 4), num(t, 5, 2), num(t, 8, 2));
    format!("{} {} {}", DOW[((z + 4).rem_euclid(7)) as usize], num(t, 8, 2), MON[((num(t, 5, 2) - 1).clamp(0, 11)) as usize])
}
fn hm(t: &str) -> String { t.get(11..16).unwrap_or("").into() }
/// Cal.nice: "Tue 14 Oct 09:00 -> 10:00"; a date alone is an all-day event
pub fn nice(st: &str, en: &str) -> String {
    if st.len() <= 10 { let e10: String = en.chars().take(10).collect(); return format!("{}{} (all day)", day(st), if e10 == plus_days(st, 1) { String::new() } else { format!(" -> {}", day(&plus_days(&e10, -1))) }) }
    format!("{} {} -> {}{}", day(st), hm(st), if st.get(..10) == en.get(..10) { String::new() } else { format!("{} ", day(en)) }, hm(en))
}
fn event_when(e: &Value) -> String { nice(&when(&e["start"]), &when(&e["end"])) }
fn unit(n: u64, u: &str) -> String { format!("{n} {u}{}", if n == 1 { "" } else { "s" }) }
fn mins(n: u64) -> String { if n > 0 && n % 1440 == 0 { unit(n / 1440, "day") } else if n > 0 && n % 60 == 0 { unit(n / 60, "hour") } else { format!("{n} min") } }
fn alerts(rm: &Value) -> String {
    if rm.get("useDefault").and_then(Value::as_bool).unwrap_or(true) { return "default".into() }
    let xs = rm["overrides"].as_array().cloned().unwrap_or_default();
    if xs.is_empty() { "none".into() } else { xs.iter().map(|r| format!("{} {}", if s(r, "method").is_empty() { "popup" } else { s(r, "method") }, mins(r["minutes"].as_u64().unwrap_or(0)))).collect::<Vec<_>>().join(", ") }
}
fn alerts_new(a: &Value) -> String { reminders(a).map(|r| alerts(&r)).unwrap_or_default() }
fn diff(given: bool, label: &str, old: &str, new: &str) -> String { if given && old != new { format!("{label}: {old}  =>  {new}") } else { String::new() } }
fn title(e: &Value) -> String { if s(e, "summary").is_empty() { "(no title)".into() } else { s(e, "summary").into() } }
fn or_none(x: String) -> String { if x.is_empty() { "(none)".into() } else { x } }
fn cap200(t: &str) -> String { super::gmail::cap(t, 200) }
fn join(xs: Vec<String>) -> String { xs.into_iter().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>().join("\n") }
fn guests(e: &Value) -> String { e["attendees"].as_array().map(|a| a.iter().map(|x| s(x, "email").to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ")).unwrap_or_default() }
/// Cal.change.detail: what the person approves for calendar_update (`a` is resolve's)
pub fn change_detail(old: &Value, a: &Value) -> String {
    let all = a.get("all_day").and_then(Value::as_bool).unwrap_or(false); let (st, en) = (s(a, "start").trim(), s(a, "end").trim());
    let ns = if st.is_empty() { when(&old["start"]) } else if all { st.chars().take(10).collect() } else { st.into() };
    let ne = if all && !st.is_empty() { end_of(true, &st.chars().take(10).collect::<String>(), en) } else if en.is_empty() { when(&old["end"]) } else { en.into() };
    let has = |k: &str| a.get(k).is_some();
    join(vec![format!("Change \"{}\" ({})", title(old), event_when(old)),
        diff(has("start") || has("end"), "Time", &event_when(old), &nice(&ns, &ne)),
        diff(has("summary"), "Title", &title(old), &if s(a, "summary").is_empty() { "(no title)".into() } else { s(a, "summary").to_string() }),
        diff(has("location"), "Where", &or_none(s(old, "location").into()), &or_none(s(a, "location").into())),
        diff(has("description"), "Details", &or_none(cap200(s(old, "description"))), &or_none(cap200(s(a, "description")))),
        diff(!alerts_new(a).is_empty(), "Alerts", &alerts(&old["reminders"]), &alerts_new(a)),
        diff(has("attendees"), "Attendees", &or_none(guests(old)), &or_none(emails(a.get("attendees")).join(", "))),
        if told(a) { "Guests will be told.".into() } else { String::new() }])
}
pub fn deleted(old: &Value) -> String { format!("Deleted: {} ({})", title(old), event_when(old)) }
/// Cal.delete.detail
pub fn delete_detail(old: &Value) -> String { join(vec![format!("Delete \"{}\" ({})", title(old), event_when(old)), if n_att(old) == 0 { String::new() } else { format!("{} will be told.", unit(n_att(old) as u64, "guest")) }]) }
/// Cal.create.detail: what the person approves for calendar_create
pub fn create_detail(a: &Value) -> String {
    let ws = emails(a.get("attendees")); let al = alerts_new(a);
    format!("{}\n{} to {}{}{}{}", s(a, "summary"), s(a, "start"), s(a, "end"), if ws.is_empty() { String::new() } else { format!("\nInvites: {}", ws.join(", ")) },
        if s(a, "location").is_empty() { String::new() } else { format!("\nWhere: {}", s(a, "location")) }, if al.is_empty() { String::new() } else { format!("\nAlerts: {al}") })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn dates_walk_the_calendar() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z"); assert_eq!(iso(1790000000), "2026-09-21T14:13:20Z"); assert_eq!(plus_days("2026-02-28", 1), "2026-03-01"); assert_eq!(plus_days("2024-03-01", -1), "2024-02-29");
        assert_eq!(time("2026-10-14"), "2026-10-14T00:00:00Z"); assert_eq!(until("2026-10-14", "", 0), "2026-10-21T00:00:00Z"); assert_eq!(until("", "", 0), "1970-01-08T00:00:00Z");
    }
    #[test] fn a_list_shows_when_what_where_and_the_id() {
        let j = json!({"timeZone":"Europe/Berlin","items":[{"id":"e1","summary":"Standup","location":"Room 1","start":{"dateTime":"2026-10-14T09:00:00+02:00"},"end":{"dateTime":"2026-10-14T09:15:00+02:00"},"attendees":[{"email":"a@x"}]}]});
        assert_eq!(events(&j), "1 events (calendar time zone Europe/Berlin):\n- 2026-10-14T09:00:00+02:00 -> 2026-10-14T09:15:00+02:00  Standup\n  at Room 1\n  id e1, 1 attendees"); assert_eq!(events(&json!({})), "No events.");
        assert_eq!(list_path("2026-10-14", "2026-10-21T00:00:00Z", 20, "x y"), "/calendar/v3/calendars/primary/events?timeMin=2026-10-14T00%3A00%3A00Z&timeMax=2026-10-21T00%3A00%3A00Z&singleEvents=true&orderBy=startTime&maxResults=20&q=x%20y");
    }
    #[test] fn a_create_body_has_alerts_attendees_and_an_all_day_end() {
        let b = create_body(&json!({"summary":"Trip","start":"2026-10-14","end":"2026-10-14","all_day":true,"attendees":"a@x, b@x","reminders":[30,{"method":"email","minutes":99999}]}));
        assert_eq!(b, json!({"summary":"Trip","start":{"date":"2026-10-14"},"end":{"date":"2026-10-15"},"attendees":[{"email":"a@x"},{"email":"b@x"}],"reminders":{"useDefault":false,"overrides":[{"method":"popup","minutes":30},{"method":"email","minutes":40320}]}}));
        assert_eq!(check(&json!({"reminders":[{"method":"sms","minutes":1}]})).unwrap(), "an alert's method is popup or email"); assert_eq!(check(&json!({"reminders":[1.5]})).unwrap(), "an alert's minutes are a whole number");
        assert_eq!(check(&json!({"reminders":[1,2,3,4,5,6]})).unwrap(), "at most 5 alerts"); assert!(check(&json!({"reminders":[10]})).is_none() && check(&json!({})).is_none());
    }
    #[test] fn a_change_keeps_the_length_and_says_old_and_new() {
        let old = json!({"summary":"Standup","start":{"dateTime":"2026-10-14T09:00:00-07:00"},"end":{"dateTime":"2026-10-14T09:30:00-07:00"},"attendees":[{"email":"a@x"}],"reminders":{"useDefault":true}});
        let a = resolve(&old, &json!({"id":"e1","start":"2026-10-15T10:00:00-07:00","reminders":[15]}));
        assert_eq!(a["end"], "2026-10-15T10:30:00-07:00"); assert!(told(&a));
        assert_eq!(change_detail(&old, &a), "Change \"Standup\" (Wed 14 Oct 09:00 -> 09:30)\nTime: Wed 14 Oct 09:00 -> 09:30  =>  Thu 15 Oct 10:00 -> 10:30\nAlerts: default  =>  popup 15 min\nGuests will be told.");
        assert_eq!(patch_body(&a), json!({"start":{"dateTime":"2026-10-15T10:00:00-07:00"},"end":{"dateTime":"2026-10-15T10:30:00-07:00"},"reminders":{"useDefault":false,"overrides":[{"method":"popup","minutes":15}]}}));
        assert!(nothing(&json!({"id":"e1"}))); assert_eq!(delete_detail(&old), "Delete \"Standup\" (Wed 14 Oct 09:00 -> 09:30)\n1 guest will be told."); assert_eq!(nice("2026-10-14", "2026-10-17"), "Wed 14 Oct -> Fri 16 Oct (all day)");
        assert!(!id_ok("a/b") && !id_ok("a b") && id_ok("abc_20261014T090000Z"));
    }
}
