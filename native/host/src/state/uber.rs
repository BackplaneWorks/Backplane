//! UberOp: the uberbot's op log folded into settings under its uid (src/core/model.bend State.uberop*, ubop.bend).
//! An op is taken only as its origin's next (n = highest seen + 1); taken, it is kept as a line, moves the origin's count and the
//! clock, and writes its register only if it is later than the one there. A "fired" op keeps the highest slot a routine ran for.
use super::json::*;
use super::State;
use serde_json::{json, Value};

fn key_op(u: &str, o: &str, n: u64) -> String { format!("uber.op.{u}.{o}.{n}") }
fn key_reg(u: &str, k: &str) -> String { format!("uber.r.{u}.{k}") }
fn key_stamp(u: &str, k: &str) -> String { format!("uber.rs.{u}.{k}") }
fn key_rn(u: &str, k: &str) -> String { format!("uber.rn.{u}.{k}") }

/// Ub.line: one op as a line
fn line(u: &str, o: &str, n: u64, stamp: &str, op: &str, at: u64) -> String {
    show(&json!({"u": u, "o": o, "n": n, "s": stamp, "op": op, "at": at}))
}
fn line_nop(u: &str, o: &str, n: u64, stamp: &str) -> String { line(u, o, n, stamp, "{\"k\":\"nop\"}", 0) }

/// Ub.op.key: the register an op writes ("" for fired and the unknown ones)
fn op_key(kind: &str, o: &Value) -> String {
    match kind {
        "mem" | "forget" => format!("mem:{}", gstr(o, "key", "")),
        "routine" | "routine-del" => format!("routine:{}", gstr(o, "id", "")),
        "bind" => format!("bind:{}", gstr(o, "machine", "")),
        "space" | "page" | "who" | "lease" => kind.to_string(),
        _ => String::new(),
    }
}
/// Ub.newer: whether an op beats what its register holds (later stamp; the lease by term first). An empty old stamp is always beaten.
fn newer(key: &str, nop: &str, nst: &str, oop: &str, ost: &str) -> bool {
    if ost.is_empty() { return true }
    if key == "lease" {
        let (nt, ot) = (unat(&parse_or_null(nop), "term"), unat(&parse_or_null(oop), "term"));
        return if nt != ot { nt > ot } else { nst > ost };
    }
    nst > ost
}
/// Ub.origins.add: sorted, empty entries dropped, the new one added (not deduplicated)
fn origins_add(cur: &str, o: &str) -> String {
    let mut v: Vec<&str> = std::iter::once(o).chain(cur.split(' ').filter(|x| !x.is_empty())).collect();
    v.sort(); v.join(" ")
}

impl State {
    /// the op that held the register before is now a nop line (its "origin.n" must split in exactly two at '.')
    fn uber_nop(&mut self, uid: &str, rn: &str, ostamp: &str) {
        let ps: Vec<&str> = rn.split('.').collect();
        if ps.len() == 2 { let n = num(ps[1]); self.set(&key_op(uid, ps[0], n), &line_nop(uid, ps[0], n, ostamp)); }
    }
    fn uber_reg(&mut self, uid: &str, key: &str, stamp: &str, op: &str, origin: &str, n: u64) {
        let (oop, ost) = (self.get(&key_reg(uid, key), ""), self.get(&key_stamp(uid, key), ""));
        if !newer(key, op, stamp, &oop, &ost) { return }
        let rn = self.get(&key_rn(uid, key), "");
        self.uber_nop(uid, &rn, &ost);
        self.set(&key_reg(uid, key), op); self.set(&key_stamp(uid, key), stamp); self.set(&key_rn(uid, key), &format!("{origin}.{n}"));
    }
    fn uber_fired(&mut self, uid: &str, origin: &str, o: &Value) {
        let gid = gstr(o, "routine", "").to_string(); let slot = unat(o, "slot");
        if slot > num(&self.get(&format!("uber.fired.{uid}.{gid}"), "0")) {
            self.set(&format!("uber.fired.{uid}.{gid}"), &slot.to_string()); self.set(&format!("uber.firedby.{uid}.{gid}"), origin);
        }
    }
    pub(super) fn uberop(&mut self, uid: &str, origin: &str, n: u64, stamp: &str, op: &str, at: u64) {
        let vv = format!("uber.vv.{uid}.{origin}");
        if !(n == num(&self.get(&vv, "0")).wrapping_add(1) && !origin.is_empty()) { return }
        let body = parse_or_null(op); let kind = gstr(&body, "k", "").to_string(); let key = op_key(&kind, &body);
        let keep = kind == "fired" || key.is_empty() || newer(&key, op, stamp, &self.get(&key_reg(uid, &key), ""), &self.get(&key_stamp(uid, &key), ""));
        self.set(&vv, &n.to_string());
        self.set(&key_op(uid, origin, n), &if keep { line(uid, origin, n, stamp, op, at) } else { line_nop(uid, origin, n, stamp) });
        if n == 1 { let k = format!("uber.origins.{uid}"); let v = origins_add(&self.get(&k, ""), origin); self.set(&k, &v); }
        let hk = format!("uber.hlc.{uid}");
        if stamp > self.get(&hk, "").as_str() { self.set(&hk, stamp); }
        if kind == "fired" { self.uber_fired(uid, origin, &body); } else if !key.is_empty() { self.uber_reg(uid, &key, stamp, op, origin, n); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fold(lines: &[&str]) -> State { let mut s = State::new(); for l in lines { s.apply_line(l); } s }
    fn ev(origin: &str, n: u64, stamp: &str, op: &str) -> String { serde_json::to_string(&json!({"$":"UberOp","uid":"u","origin":origin,"n":n,"stamp":stamp,"op":op,"at":5})).unwrap() }
    #[test] fn uber_op_only_as_the_origins_next() {
        // rust uber_op_only_as_the_origins_next
        let a = ev("m1", 1, "0000000001.000000.m1", r#"{"k":"space","spec":"a"}"#); let c = ev("m1", 3, "0000000003.000000.m1", r#"{"k":"space","spec":"c"}"#);
        let s = fold(&[&a, &c]);
        assert_eq!(s.settings["uber.vv.u.m1"], "1"); assert!(!s.settings.contains_key("uber.op.u.m1.3")); assert_eq!(s.seq, 2);
    }
    #[test] fn uber_later_stamp_wins_and_the_loser_becomes_a_nop() {
        // rust uber_later_stamp_wins_and_the_loser_becomes_a_nop
        let a = ev("m1", 1, "0000000009.000000.m1", r#"{"k":"space","spec":"new"}"#); let b = ev("m2", 1, "0000000002.000000.m2", r#"{"k":"space","spec":"old"}"#);
        let s = fold(&[&a, &b]);
        assert_eq!(s.settings["uber.r.u.space"], r#"{"k":"space","spec":"new"}"#);
        assert_eq!(s.settings["uber.op.u.m2.1"], r#"{"u":"u","o":"m2","n":1,"s":"0000000002.000000.m2","op":"{\"k\":\"nop\"}","at":0}"#);
        assert_eq!(s.settings["uber.origins.u"], "m1 m2");
    }
}
