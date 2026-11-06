//! The settings families the first fold left out: uber ops, wake subscriptions, named locks,
//! token totals and the last turn, a reminder's Act, and the task tools' todo edits. Each is a pure fold over
//! `State` with no state of its own: bounded by the settings it writes, which fold keeps too. Equal to State.replay for
//! these events by the differential oracle (test/parity/oracle, `--skip none`).
#[path = "json.rs"] pub mod json;
#[path = "uber.rs"] mod uber;
#[path = "todo.rs"] mod todo;
#[path = "liveness.rs"] pub mod liveness;
#[path = "watchdog.rs"] pub mod watchdog;
#[path = "turnend.rs"] pub mod turnend;
#[path = "summary.rs"] pub mod summary;
#[path = "star.rs"] pub mod star;
#[path = "ext.rs"] pub mod ext;
#[path = "policy.rs"] pub mod policy;
#[path = "autosettle.rs"] pub mod autosettle;
pub use ext::{on_post, Ext};
use json::*;
use super::State;
use serde_json::{json, Value};

fn s<'a>(e: &'a Value, k: &str) -> &'a str { gstr(e, k, "") }
fn n(e: &Value, k: &str) -> u64 { gnat(e, k, 0) }

/// Kinds this module folds; `None` never (the decoder is lenient: every field defaults). Other kinds are not ours: `Some(())`.
pub fn fold(st: &mut State, kind: &str, e: &Value) -> Option<()> {
    match kind {
        "MailPosted" => ext::mail_posted(st, s(e, "id"), s(e, "to"), s(e, "kind"), n(e, "at")),
        "TaskDelivered" => ext::delivered(st, s(e, "task")),
        "Reminded" => { let (id, th, at) = (s(e, "id"), s(e, "thread"), n(e, "at")); let rid = format!("remind:{id}:{at}"); st.post(th, &rid); on_post(st, th, &rid, at, None); }
        "UberOp" => st.uberop(s(e, "uid"), s(e, "origin"), n(e, "n"), s(e, "stamp"), s(e, "op"), n(e, "at")),
        "TodosChanged" => st.todos_changed(s(e, "thread"), s(e, "who"), s(e, "data")),
        "WakeSet" => st.wake_set(e),
        "WakeDropped" => st.wake_drop(s(e, "id")),
        "LockChanged" => st.lock_changed(s(e, "name"), gobj(e, "lock")),
        "TurnUsage" => st.turn_usage(e),
        // DeployAsked and DeployBuilt: the Bend hub's deploy_main, which nothing writes now. Old logs still hold them; they count in seq and set nothing.
        // A hub moves to a new build only through its updater and the guard (update/, docs/deploy.md).
        _ => {}
    }
    Some(())
}

/// Launch.add: a line added to a list of lines once ("" has none)
fn add_line(v: &str, id: &str) -> String {
    if v.split('\n').any(|l| l == id) && !v.is_empty() { v.into() } else if v.is_empty() { id.into() } else { format!("{v}\n{id}") }
}

impl State {
    // ---- wake subscriptions: indexes hold ids, records live under wake.<id> ----
    fn wake_index(&mut self, key: &str, id: &str) { let v = add_line(&self.get(key, ""), id); self.set(key, &v); }
    fn wake_set(&mut self, e: &Value) {
        let (id, by, kind, target) = (s(e, "id"), s(e, "by"), s(e, "kind"), s(e, "target"));
        let events: Vec<Value> = elems(gobj(e, "events")).iter().map(|x| Value::String(x.as_str().unwrap_or("").into())).collect();
        let rec = json!({"id": id, "by": by, "kind": kind, "target": target, "events": events, "once": gbool(e, "once", true), "note": s(e, "note"),
                         "created": serde_json::Number::from(n(e, "at"))});
        self.set(&format!("wake.{id}"), &show(&rec));
        self.wake_index(&format!("wakes.by.{by}"), id);
        self.wake_index(&format!("wakes.on.{kind}:{target}"), id);
        if kind == "pr" || kind == "file" { self.wake_index("wakes.poll", id); }
    }
    fn wake_unindex(&mut self, key: &str, id: &str) {
        let v = self.get(key, "");
        let kept: Vec<&str> = if v.is_empty() { vec![] } else { v.split('\n').filter(|x| *x != id).collect() };
        self.set(key, &kept.join("\n"));
    }
    fn wake_drop(&mut self, id: &str) {
        let o = parse_or_null(&self.get(&format!("wake.{id}"), ""));
        self.wake_unindex("wakes.poll", id);
        self.wake_unindex(&format!("wakes.by.{}", gstr(&o, "by", "")), id);
        self.wake_unindex(&format!("wakes.on.{}:{}", gstr(&o, "kind", ""), gstr(&o, "target", "")), id);
        self.set(&format!("wake.{id}"), "");
    }

    // ---- named locks: lock.<name> is the record, locks.names the index ----
    fn lock_changed(&mut self, name: &str, l: &Value) {
        let party = |o: &Value| json!({"id": gstr(o, "id", ""), "kind": gstr(o, "kind", "thread"), "thread": gstr(o, "thread", ""), "note": gstr(o, "note", ""),
                                       "since": serde_json::Number::from(gnat(o, "since", 0)), "lease": serde_json::Number::from(gnat(o, "lease", 0))});
        let holder = match gobj(l, "holder") { Value::Null => Value::Null, o => party(o) };
        let waiters: Vec<Value> = elems(gobj(l, "waiters")).iter().map(party).collect();
        let mut m = serde_json::Map::new();
        m.insert("holder".into(), holder); m.insert("waiters".into(), Value::Array(waiters)); m.insert("ready".into(), Value::Bool(gbool(l, "ready", false)));
        if let o @ (Value::Object(_) | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_)) = gobj(l, "outside") {
            m.insert("outside".into(), json!({"pid": serde_json::Number::from(gnat(o, "pid", 0)), "cmd": gstr(o, "cmd", ""), "since": serde_json::Number::from(gnat(o, "since", 0))}));
        }
        self.set(&format!("lock.{name}"), &show(&Value::Object(m)));
        let v = add_line(&self.get("locks.names", ""), name); self.set("locks.names", &v);
    }

    // ---- token totals ----
    fn tok_sum(&mut self, key: &str, p: [u64; 6], ck: bool, kk: bool) {
        let old = self.get(key, "");
        let part = |i: usize| num(old.split('/').nth(i).unwrap_or("0"));
        let mut v = vec![part(0) + 1];
        for (i, x) in p.iter().enumerate() { v.push(part(i + 1) + x); }
        v.push(part(7) + ck as u64); v.push(part(8) + kk as u64);
        self.set(key, &v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("/"));
    }
    fn turn_usage(&mut self, e: &Value) {
        let (th, pv, mo, key, known, basis, at) = (s(e, "thread"), s(e, "provider"), s(e, "model"), s(e, "key"), s(e, "known"), s(e, "basis"), n(e, "at"));
        let p = [n(e, "input"), n(e, "cread"), n(e, "cwrite"), n(e, "output"), n(e, "reasoning"), n(e, "cost")];
        let x = { let ep = format!("{}{}", self.get("tok.epoch", ""), self.get("tok.recount", "")); if ep.is_empty() { "tok.".to_string() } else { format!("tok.{ep}.") } };
        let pj = self.threads.get(th).map(|t| t.project.clone()).unwrap_or_default();
        let pm = format!("{pv}|{}", if mo.is_empty() { "default" } else { mo });
        let (ck, kk) = (known.contains('c'), known.contains('o'));
        let (day, month) = (day_of(at), day_of(at).chars().take(7).collect::<String>());
        for k in [format!("{x}t.{th}"), format!("{x}d.{day}"), format!("{x}m.{month}"), format!("{x}mm.{month}|{pm}"), format!("{x}p.{pj}|{month}"),
                  format!("{x}dv.{day}|{}", pm.split('|').next().unwrap_or("")), format!("{x}dp.{day}|{pj}")] { self.tok_sum(&k, p, ck, kk); }
        let ds = format!("{x}daily.since"); if self.get(&ds, "").is_empty() { self.set(&ds, &at.to_string()); }
        let ml = format!("{x}ml.{month}"); let cur = self.get(&ml, "");
        let v = if cur.split('\n').any(|l| l == pm) { cur } else if cur.is_empty() { pm.clone() } else { format!("{cur}\n{pm}") }; self.set(&ml, &v);
        let sk = format!("{x}since"); if self.get(&sk, "").is_empty() { self.set(&sk, &at.to_string()); }
        self.set(&format!("tok.key.{th}"), key); self.set(&format!("tok.basis.{th}"), basis);
        self.set(&format!("turn.last.{th}"), &format!("{}/{}/{}/{}/{}/{}/{at}/{known}/{pm}", p[0], p[1], p[2], p[3], p[4], p[5]));
    }
}

/// Tok.day: the UTC day of a moment, "2026-10-15" (H. Hinnant's civil_from_days)
fn day_of(at: u64) -> String {
    let z = (at / 86400) as i64 + 719468;
    let era = z.div_euclid(146097); let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fold_all(lines: &[String]) -> State { let mut st = State::new(); for l in lines { st.apply_line(l); } st }
    fn ev(v: Value) -> String { serde_json::to_string(&v).unwrap() }
    #[test] fn tok_day_matches_civil_dates() {
        // rust tok_day_matches_civil_dates
        assert_eq!(day_of(0), "1970-01-01"); assert_eq!(day_of(1_793_900_000), "2026-11-05"); assert_eq!(day_of(951_782_400), "2000-02-29"); assert_eq!(day_of(86399), "1970-01-01");
    }
    #[test] fn tok_totals_add_per_dimension_and_the_last_turn_is_kept() {
        // rust tok_totals_add_per_dimension_and_the_last_turn_is_kept
        let t = |k: &str| ev(json!({"$":"TurnUsage","thread":"t1","provider":"claude","model":"","key":k,"input":1,"cread":2,"cwrite":3,"output":4,"reasoning":5,"cost":6,"known":"co","basis":"b","at":1_793_900_000}));
        let st = fold_all(&[t("a"), t("b")]);
        assert_eq!(st.settings["tok.t.t1"], "2/2/4/6/8/10/12/2/2"); assert_eq!(st.settings["tok.d.2026-11-05"], "2/2/4/6/8/10/12/2/2");
        assert_eq!(st.settings["turn.last.t1"], "1/2/3/4/5/6/1793900000/co/claude|default"); assert_eq!(st.settings["tok.ml.2026-11"], "claude|default");
        assert_eq!(st.settings["tok.key.t1"], "b");
    }
    #[test] fn wake_drop_unindexes_and_blanks_the_record() {
        // rust wake_drop_unindexes_and_blanks_the_record
        let st = fold_all(&[ev(json!({"$":"WakeSet","id":"w1","by":"t","kind":"pr","target":"5","events":["merged"],"once":true,"note":"n","at":9})),
                            ev(json!({"$":"WakeSet","id":"w2","by":"t","kind":"pr","target":"6","events":[],"at":9})), ev(json!({"$":"WakeDropped","id":"w1","at":10}))]);
        assert_eq!(st.settings["wakes.by.t"], "w2"); assert_eq!(st.settings["wakes.poll"], "w2"); assert_eq!(st.settings["wake.w1"], ""); assert_eq!(st.settings["wakes.on.pr:5"], "");
        assert!(st.settings["wake.w2"].contains("\"once\":true"));
    }
    #[test] fn lock_record_is_reprinted_in_shape() {
        // rust lock_record_is_reprinted_in_shape
        let st = fold_all(&[ev(json!({"$":"LockChanged","name":"merge","lock":{"holder":{"id":"a"},"waiters":[],"ready":true},"at":1}))]);
        assert_eq!(st.settings["lock.merge"], r#"{"holder":{"id":"a","kind":"thread","thread":"","note":"","since":0,"lease":0},"waiters":[],"ready":true}"#);
        assert_eq!(st.settings["locks.names"], "merge");
    }
    #[test] fn old_deploy_events_count_and_set_nothing() {
        let st = fold_all(&[ev(json!({"$":"DeployAsked","by":"a","note":"","commit":"","cover":"","at":1})), ev(json!({"$":"DeployBuilt","commit":"c1","state":"staged","version":"","detail":"","at":2}))]);
        assert_eq!(st.seq, 2); assert!(!st.settings.contains_key("deploy.ship"), "the updater and the guard move builds now");
    }
    #[test] fn reminded_files_an_act_under_its_thread() {
        // rust reminded_files_an_act_under_its_thread
        let st = fold_all(&[ev(json!({"$":"Reminded","id":"r","bot":"b","thread":"t1","text":"x","at":7}))]);
        assert_eq!(st.entries["t1"], vec!["remind:r:7".to_string()]); assert_eq!(st.seq, 1);
    }
}
