//! Browser task skills, pure: a task is steps aimed at elements by meaning (role, name, label...), never by
//! coordinates. The task and its JSON, the recorder's events as steps, parameters, the selectors to try, risk, the pixel scaling of the person's
//! clicks, who drives, and the run's state machine (a pause alerts once). A secret's value is never part of a task: the recorder sends only
//! {secret: true, label}, and `step_of_record` drops any "value" beside it. Laws task_*, run_*, drive_*, teach_scale_* map to the tests below.
use serde_json::{json, Map, Value as J};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Target { pub role: String, pub name: String, pub label: String, pub placeholder: String, pub text: String, pub testid: String, pub css: String, pub nth: u64, pub frame: String, pub near: String }
/// A step's typed value: a literal, a parameter's name, a secret the person gives each time (its label only), or none.
#[derive(Clone, Debug, PartialEq)]
pub enum Value { Lit(String), Param(String), Secret(String), None }
#[derive(Clone, Debug, PartialEq)]
pub struct Step { pub n: u64, pub act: String, pub target: Target, pub value: Value, pub wait_url: String, pub wait_text: String, pub url: String, pub risk: String, pub note: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Param { pub name: String, pub label: String, pub kind: String, pub example: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Task { pub slug: String, pub name: String, pub description: String, pub origin: String, pub start: String, pub params: Vec<Param>, pub steps: Vec<Step>, pub urls: Vec<String>, pub created: u64, pub updated: u64 }

fn gs(v: &J, k: &str) -> String { v.get(k).and_then(J::as_str).unwrap_or("").to_string() }
fn gn(v: &J, k: &str) -> u64 { v.get(k).and_then(J::as_u64).unwrap_or(0) }
pub fn first_name(t: &Task) -> String { first(&t.name, &t.slug) }
fn first(a: &str, b: &str) -> String { if a.is_empty() { b.to_string() } else { a.to_string() } }

/// Task.slug: lowercase, [a-z0-9-], dashes collapsed and trimmed, at most 48 chars, "task" if nothing is left.
pub fn slug(name: &str) -> String {
    let mut out = String::new(); let mut prev = true;
    for c in name.chars().take(200) {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() { out.push(c); prev = false } else if !prev { out.push('-'); prev = true }
    }
    let t = out.trim_end_matches('-'); let cut: String = t.chars().take(48).collect(); let cut = cut.trim_end_matches('-');
    if cut.is_empty() { "task".into() } else { cut.into() }
}

// ---- values ----
pub fn value_str(s: &str) -> Value {
    if s.starts_with("{{") && s.ends_with("}}") && s.chars().count() >= 5 { Value::Param(s.chars().skip(2).take(s.chars().count() - 4).collect()) } else if s.is_empty() { Value::None } else { Value::Lit(s.into()) }
}
pub fn value_of(j: Option<&J>) -> Value {
    match j { Some(J::String(s)) => value_str(s), Some(o @ J::Object(_)) => Value::Secret(gs(o, "secret")), _ => Value::None }
}
pub fn value_json(v: &Value) -> J {
    match v { Value::Lit(t) => json!(t), Value::Param(n) => json!(format!("{{{{{n}}}}}")), Value::Secret(l) => json!({"secret": l}), Value::None => json!("") }
}

// ---- JSON ----
pub fn target_of(t: &J, frame: &str) -> Target {
    Target { role: gs(t, "role"), name: gs(t, "name"), label: gs(t, "label"), placeholder: gs(t, "placeholder"), text: gs(t, "text"), testid: gs(t, "testid"), css: gs(t, "css"), nth: gn(t, "nth"), frame: first(frame, &gs(t, "frame")), near: gs(t, "near") }
}
pub fn target_json(t: &Target) -> J { json!({"role":t.role,"name":t.name,"label":t.label,"placeholder":t.placeholder,"text":t.text,"testid":t.testid,"css":t.css,"nth":t.nth,"frame":t.frame,"near":t.near}) }
pub fn step_of(j: &J) -> Step {
    let w = j.get("wait").cloned().unwrap_or(J::Null);
    Step { n: gn(j, "n"), act: gs(j, "do"), target: target_of(&j.get("target").cloned().unwrap_or(J::Null), ""), value: value_of(j.get("value")), wait_url: gs(&w, "url"), wait_text: gs(&w, "text"), url: gs(j, "url"),
        risk: j.get("risk").and_then(J::as_str).unwrap_or("none").to_string(), note: gs(j, "note") }
}
pub fn step_json(s: &Step) -> J { json!({"n":s.n,"do":s.act,"target":target_json(&s.target),"value":value_json(&s.value),"wait":{"url":s.wait_url,"text":s.wait_text},"url":s.url,"risk":s.risk,"note":s.note}) }
fn param_of(j: &J) -> Param { Param { name: gs(j, "name"), label: gs(j, "label"), kind: j.get("kind").and_then(J::as_str).unwrap_or("text").to_string(), example: gs(j, "example") } }
fn param_json(p: &Param) -> J { json!({"name":p.name,"label":p.label,"kind":p.kind,"example":p.example}) }
fn arr<'a>(j: &'a J, k: &str) -> &'a [J] { j.get(k).and_then(J::as_array).map(Vec::as_slice).unwrap_or(&[]) }
/// Task.parse: None unless it is an object with a slug.
pub fn parse(o: &J) -> Option<Task> {
    if !o.is_object() || gs(o, "slug").is_empty() { return None }
    Some(Task { slug: gs(o, "slug"), name: gs(o, "name"), description: gs(o, "description"), origin: gs(o, "origin"), start: gs(o, "start"), params: arr(o, "params").iter().map(param_of).collect(), steps: arr(o, "steps").iter().map(step_of).collect(),
        urls: arr(o, "urls").iter().filter_map(|u| u.as_str().map(str::to_string)).collect(), created: gn(o, "created"), updated: gn(o, "updated") })
}
pub fn read(text: &str) -> Option<Task> { serde_json::from_str::<J>(text).ok().and_then(|j| parse(&j)) }
pub fn json(t: &Task) -> J {
    json!({"v":1,"slug":t.slug,"name":t.name,"description":t.description,"origin":t.origin,"start":t.start,"params":t.params.iter().map(param_json).collect::<Vec<_>>(),"steps":t.steps.iter().map(step_json).collect::<Vec<_>>(),"urls":t.urls,"created":t.created,"updated":t.updated})
}

// ---- the recorder's events ----
/// A record-event step of the browser helper. A record with secret:true becomes a Secret(label) and its "value", if any, is ignored; a submit is a click
/// that sends something; a note's words are its text.
pub fn step_of_record(r: &J, n: u64) -> Step {
    let act = gs(r, "do"); let secret = r.get("secret").and_then(J::as_bool).unwrap_or(false); let v = gs(r, "value");
    let tg = r.get("target").cloned().unwrap_or(J::Null);
    let label = first(&gs(r, "label"), &first(&gs(&tg, "label"), &gs(&tg, "name")));
    let note = act == "note";
    let value = if note { Value::None } else if secret { Value::Secret(label) } else if v.is_empty() { Value::None } else { Value::Lit(v.clone()) };
    Step { n, act: if act == "submit" { "click".into() } else { act.clone() }, target: target_of(&tg, &gs(r, "frame")), value, wait_url: String::new(), wait_text: String::new(), url: gs(r, "url"), risk: if act == "submit" { "submit".into() } else { "none".into() }, note: if note { v } else { String::new() } }
}
/// task_secret_never_stored
pub fn value_secret(secret: bool, v: &str, label: &str) -> Value { if secret { Value::Secret(label.into()) } else if v.is_empty() { Value::None } else { Value::Lit(v.into()) } }

// ---- parameters ----
fn arg(ps: &Map<String, J>, name: &str) -> Option<String> { ps.get(name).map(|v| match v { J::String(s) => s.clone(), J::Number(n) => n.to_string(), _ => String::new() }) }
/// Task.missing: the parameters a task's steps use that `ps` does not give, each once, in order.
pub fn missing(t: &Task, ps: &Map<String, J>) -> Vec<String> {
    let mut need: Vec<String> = vec![];
    for s in &t.steps { if let Value::Param(n) = &s.value { if !need.contains(n) { need.push(n.clone()) } } }
    need.into_iter().filter(|n| arg(ps, n).is_none()).collect()
}
/// Task.fill: every step's value a literal, or the first parameter that is missing.
pub fn fill(t: &Task, ps: &Map<String, J>) -> Result<Task, String> {
    if let Some(n) = missing(t, ps).first() { return Err(format!("missing parameter: {n}")) }
    let mut o = t.clone();
    for s in &mut o.steps { if let Value::Param(n) = &s.value { if let Some(v) = arg(ps, n) { s.value = Value::Lit(v) } } }
    Ok(o)
}

// ---- selectors ----
/// Tk.quoted
pub fn quoted(s: &str) -> String { let mut o = String::from("\""); for c in s.chars() { if c == '"' || c == '\\' { o.push('\\') } o.push(c) } o.push('"'); o }
/// Task.locators: Playwright selectors for a target, best first: role and name (the nth match when it is not the first), aria-label, placeholder,
/// test id, text, then the css the recorder kept.
pub fn locators(t: &Target) -> Vec<String> {
    let mut v = vec![];
    if !t.role.is_empty() && !t.name.is_empty() { v.push(format!("role={}[name={}]{}", t.role, quoted(&t.name), if t.nth > 0 { format!(" >> nth={}", t.nth) } else { String::new() })) }
    if !t.label.is_empty() { v.push(format!("[aria-label={}]", quoted(&t.label))) }
    if !t.placeholder.is_empty() { v.push(format!("[placeholder={}]", quoted(&t.placeholder))) }
    if !t.testid.is_empty() { v.push(format!("[data-testid={}]", quoted(&t.testid))) }
    if !t.text.is_empty() { v.push(format!("text={}", quoted(&t.text))) }
    if !t.css.is_empty() { v.push(t.css.clone()) }
    v
}

// ---- risk ----
fn words(s: &str, ws: &[&str]) -> bool { ws.iter().any(|w| s.contains(w)) }
fn risk_pick(s: &str) -> &'static str {
    if words(s, &["delete", "remove"]) { "delete" } else if words(s, &["place order", "checkout", "purchase", "buy", "pay", "submit payment"]) { "purchase" } else if words(s, &["send", "transfer"]) { "send" } else if words(s, &["confirm"]) { "confirm" } else { "none" }
}
/// Task.risk: a step's own mark if it has one; else a click, press or submit on something named pay, buy, send, delete, confirm...
pub fn risk(s: &Step) -> String {
    if !s.risk.is_empty() && s.risk != "none" { return s.risk.clone() }
    if matches!(s.act.as_str(), "click" | "press" | "submit") {
        let t = &s.target; let text: String = format!("{} {} {}", t.name, t.label, t.text).chars().take(80).collect::<String>().to_lowercase();
        risk_pick(&text).into()
    } else { "none".into() }
}

// ---- frame pixels to viewport pixels ----
fn scale1(v: u64, f: u64, w: u64) -> u64 { (if f == 0 { v } else { v * w / f }).min(w.saturating_sub(1)) }
/// Task.scale: a click at (x, y) in a frame of fw by fh, as a point of a viewport of vw by vh.
pub fn scale(x: u64, y: u64, fw: u64, fh: u64, vw: u64, vh: u64) -> (u64, u64) { (scale1(x, fw, vw), scale1(y, fh, vh)) }

// ---- who drives, and a run ----
/// Drive.can_bot: the bot drives unless the person has the page ("teach" or "takeover").
pub fn can_bot(drive: &str) -> bool { drive.is_empty() }

#[derive(Clone, Debug, PartialEq)]
pub enum Run { Running(u64), Waiting { n: u64, kind: String, risky: bool }, Person(u64), Ended(String) }
#[derive(Clone, Debug, PartialEq)]
pub enum In { Pause { kind: String, risky: bool }, Answer(String), HandBack, Advance, Finish(String) }
fn choice(c: &str) -> u8 { match c { "confirm" => 1, "continue" => 2, "skip" => 3, "takeover" => 4, "stop" => 5, _ => 0 } }
/// Run.step: the run after one input (docs/teach.md). Continue goes on only when the step is not risky.
pub fn step(st: &Run, i: &In) -> Run {
    match st {
        Run::Running(n) => match i {
            In::Advance => Run::Running(n + 1), In::Finish(s) => Run::Ended(s.clone()), In::Pause { kind, risky } => Run::Waiting { n: *n, kind: kind.clone(), risky: *risky },
            In::Answer(c) if choice(c) == 5 => Run::Ended("stopped".into()), _ => Run::Running(*n),
        },
        Run::Waiting { n, kind, risky } => match i {
            In::Answer(c) => match choice(c) {
                1 => Run::Running(*n), 2 => if *risky { st.clone() } else { Run::Running(*n) }, 3 => Run::Running(n + 1), 4 => Run::Person(*n), 5 => Run::Ended("stopped".into()), _ => Run::Waiting { n: *n, kind: kind.clone(), risky: *risky },
            },
            _ => st.clone(),
        },
        Run::Person(n) => match i { In::HandBack => Run::Running(*n), In::Answer(c) if choice(c) == 5 => Run::Ended("stopped".into()), _ => st.clone() },
        Run::Ended(s) => Run::Ended(s.clone()),
    }
}
pub fn alert_key(run: &str, n: u64) -> String { format!("task-{run}-{n}") }
/// Run.alerts: only on entering a pause, so a run that keeps waiting alerts once.
pub fn alerts(before: &Run, after: &Run) -> bool { !matches!(before, Run::Waiting { .. }) && matches!(after, Run::Waiting { .. }) }

#[cfg(test)]
mod tests {
    use super::*;
    fn t() -> Task { parse(&json!({"slug":"s","name":"N","steps":[{"n":1,"do":"type","value":"{{order}}","target":{"role":"textbox","name":"Order"}},{"n":2,"do":"type","value":"{{order}}"},{"n":3,"do":"click","target":{"name":"Place order"}}]})).unwrap() }
    #[test] fn task_slug_rules() { assert_eq!(slug(""), "task"); assert_eq!(slug("  Check JLCPCB order status! "), "check-jlcpcb-order-status"); assert_eq!(slug("---"), "task"); assert!(slug(&"a b ".repeat(40)).len() <= 48 && !slug(&"a b ".repeat(40)).ends_with('-')); assert!(!slug("é").is_empty()) }
    #[test] fn task_secret_never_stored() {
        assert_eq!(value_secret(true, "hunter2", "Password"), Value::Secret("Password".into()));
        let s = step_of_record(&json!({"do":"type","secret":true,"value":"hunter2","target":{"name":"Password"}}), 3);
        assert_eq!(s.value, Value::Secret("Password".into())); assert!(!step_json(&s).to_string().contains("hunter2"));
        let n = step_of_record(&json!({"do":"note","value":"remember this"}), 1); assert_eq!((n.value, n.note.as_str()), (Value::None, "remember this"));
        let sub = step_of_record(&json!({"do":"submit","target":{"name":"Go"},"url":"u"}), 2); assert_eq!((sub.act.as_str(), sub.risk.as_str()), ("click", "submit"));
    }
    #[test] fn task_json_round_trips_with_fields_in_order() {
        let j = json(&t()); assert_eq!(serde_json::to_string(&j).unwrap().find("\"v\":1").unwrap(), 1); assert_eq!(parse(&j).unwrap(), t()); assert!(parse(&json!({"name":"x"})).is_none() && parse(&json!([1])).is_none());
        assert_eq!(value_str("{{a}}"), Value::Param("a".into())); assert_eq!(value_str("{{}}"), Value::Lit("{{}}".into())); assert_eq!(value_str(""), Value::None); assert_eq!(value_json(&Value::Param("a".into())), json!("{{a}}"));
        assert_eq!(read(&j.to_string()).unwrap().steps.len(), 3);
    }
    #[test] fn task_parameters_are_listed_once_and_filled() {
        let mut ps = Map::new(); assert_eq!(missing(&t(), &ps), vec!["order"]); assert_eq!(fill(&t(), &ps).unwrap_err(), "missing parameter: order");
        ps.insert("order".into(), json!(1042)); let f = fill(&t(), &ps).unwrap(); assert_eq!(f.steps[0].value, Value::Lit("1042".into())); assert_eq!(f.steps[1].value, Value::Lit("1042".into()));
    }
    #[test] fn task_locators_best_first() {
        let tg = Target { role: "button".into(), name: "Go \"now\"".into(), nth: 2, label: "L".into(), css: "#go".into(), ..Default::default() };
        assert_eq!(locators(&tg), vec!["role=button[name=\"Go \\\"now\\\"\"] >> nth=2", "[aria-label=\"L\"]", "#go"]); assert!(locators(&Target::default()).is_empty());
    }
    #[test] fn task_risky_steps_are_found_by_name() {
        let mk = |act: &str, name: &str, risk: &str| Step { n: 1, act: act.into(), target: Target { name: name.into(), ..Default::default() }, value: Value::None, wait_url: "".into(), wait_text: "".into(), url: "".into(), risk: risk.into(), note: "".into() };
        assert_eq!(risk(&mk("click", "Place order", "none")), "purchase"); assert_eq!(risk(&mk("click", "Delete account", "")), "delete"); assert_eq!(risk(&mk("click", "Send message", "none")), "send");
        assert_eq!(risk(&mk("click", "Confirm", "none")), "confirm"); assert_eq!(risk(&mk("type", "Place order", "none")), "none"); assert_eq!(risk(&mk("click", "Next", "submit")), "submit"); assert_eq!(risk(&mk("click", "Next", "none")), "none");
    }
    #[test] fn teach_scale() { assert_eq!(scale(1000, 1000, 1000, 1000, 1280, 800), (1279, 799)); assert_eq!(scale(500, 500, 1000, 1000, 1280, 800), (640, 400)); assert_eq!(scale(5, 5, 0, 0, 100, 100), (5, 5)) }
    #[test] fn drive_person_locks_bot() { assert!(can_bot("")); assert!(!can_bot("teach") && !can_bot("takeover")) }
    #[test] fn run_laws() {
        let w = |risky| Run::Waiting { n: 4, kind: "login".into(), risky };
        assert_eq!(step(&w(true), &In::Answer("continue".into())), w(true), "task_risky_needs_confirm"); assert_eq!(step(&w(false), &In::Answer("continue".into())), Run::Running(4));
        assert_eq!(step(&w(true), &In::Answer("confirm".into())), Run::Running(4), "run_confirm_runs"); assert_eq!(step(&w(true), &In::Answer("stop".into())), Run::Ended("stopped".into()), "run_stop_ends");
        assert_eq!(step(&w(false), &In::Answer("skip".into())), Run::Running(5)); assert_eq!(step(&w(false), &In::Answer("takeover".into())), Run::Person(4)); assert_eq!(step(&w(false), &In::Answer("what".into())), w(false));
        assert_eq!(step(&Run::Person(4), &In::HandBack), Run::Running(4)); assert_eq!(step(&Run::Person(4), &In::Advance), Run::Person(4)); assert_eq!(step(&Run::Ended("ok".into()), &In::Advance), Run::Ended("ok".into()));
        assert_eq!(step(&Run::Running(1), &In::Advance), Run::Running(2)); assert_eq!(step(&Run::Running(1), &In::Finish("ok".into())), Run::Ended("ok".into())); assert_eq!(step(&Run::Running(1), &In::Pause { kind: "k".into(), risky: true }), Run::Waiting { n: 1, kind: "k".into(), risky: true });
        // run_pause_alerts / run_stuck_alerts_once
        assert!(alerts(&Run::Running(4), &w(false))); for s in [Run::Running(1), w(true), Run::Person(1), Run::Ended("x".into())] { assert!(!alerts(&w(false), &s)) }
        assert_eq!(alert_key("r-1", 4), "task-r-1-4");
    }
}
