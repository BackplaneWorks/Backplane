//! Keyed deltas of a view's body. The hub keeps the last three
//! bodies it sent a view; a client that holds one (by its token) is sent only the ops that turn it into the new body:
//!   ["s", path, value]   set the value at path ([] is the whole body)
//!   ["a", path, specs]   rebuild the array at path; a spec is [0, i] (the base array's i-th element) or [1, value]
//!   ["t", path]          set the request's own draft text at path
//! Objects with the same keys in the same order are diffed field by field, any other object is set whole. Arrays whose
//! elements keep their identities in place (a node's "key", else an entry's "id") are diffed element by element;
//! otherwise, when every new element has its own identity, the array is rebuilt from the old elements by identity and
//! each reused one is diffed in its new place; else from the common prefix and suffix, the middle new.
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)] pub enum Step { Key(String), At(usize) }
#[derive(Clone, Debug, PartialEq)] pub enum Spec { Old(usize), New(Value) }
#[derive(Clone, Debug, PartialEq)] pub enum Op { Set(Vec<Step>, Value), Rebuild(Vec<Step>, Vec<Spec>) }

const FUEL: usize = 4_000_000;

fn ident(v: &Value) -> String {
    match v.as_object() {
        Some(o) => {
            let key = o.get("key").and_then(Value::as_str).unwrap_or("");
            if !key.is_empty() { return format!("k:{key}") }
            let id = o.get("id").and_then(Value::as_str).unwrap_or("");
            if id.is_empty() { String::new() } else { format!("i:{id}") }
        }
        None => String::new(),
    }
}
fn same_keys(a: &serde_json::Map<String, Value>, b: &serde_json::Map<String, Value>) -> bool { a.len() == b.len() && a.keys().zip(b.keys()).all(|(x, y)| x == y) }

/// The ops from `old` to `new`. Some([]) exactly when they are equal; None: send `new` whole (out of fuel).
pub fn diff(old: &Value, new: &Value) -> Option<Vec<Op>> {
    if old == new { return Some(vec![]) }
    let mut ops: Vec<Op> = Vec::new();
    // a stack of pairs still to compare, the next one on top
    let mut work: Vec<(&Value, &Value, Vec<Step>)> = vec![(old, new, vec![])];
    let mut fuel = FUEL;
    while let Some((o, n, at)) = work.pop() {
        if fuel == 0 { return None } fuel -= 1;
        if o == n { continue }
        match (o, n) {
            (Value::Object(a), Value::Object(b)) => {
                if same_keys(a, b) {
                    let mut tasks: Vec<(&Value, &Value, Vec<Step>)> = Vec::with_capacity(a.len());
                    for ((k, x), (_, y)) in a.iter().zip(b.iter()) { let mut p = at.clone(); p.push(Step::Key(k.clone())); tasks.push((x, y, p)); }
                    work.extend(tasks.into_iter().rev());
                } else { ops.push(Op::Set(at, n.clone())) }
            }
            (Value::Array(a), Value::Array(b)) => {
                let (oi, ni): (Vec<String>, Vec<String>) = (a.iter().map(ident).collect(), b.iter().map(ident).collect());
                if oi == ni {
                    let tasks: Vec<_> = a.iter().zip(b.iter()).enumerate().map(|(i, (x, y))| { let mut p = at.clone(); p.push(Step::At(i)); (x, y, p) }).collect();
                    work.extend(tasks.into_iter().rev());
                } else if keyed(&ni) {
                    let mut specs = Vec::with_capacity(b.len()); let mut tasks = Vec::new();
                    for (j, x) in b.iter().enumerate() {
                        match oi.iter().position(|id| *id == ni[j]) {
                            Some(i) => { specs.push(Spec::Old(i)); let mut p = at.clone(); p.push(Step::At(j)); tasks.push((&a[i], x, p)); }
                            None => specs.push(Spec::New(x.clone())),
                        }
                    }
                    ops.push(Op::Rebuild(at, specs));
                    work.extend(tasks.into_iter().rev());
                } else { ops.push(Op::Rebuild(at, unkeyed(a, b))) }
            }
            _ => ops.push(Op::Set(at, n.clone())),
        }
    }
    if ops.is_empty() { None } else { Some(ops) }
}

fn keyed(ids: &[String]) -> bool { let mut seen = std::collections::HashSet::new(); ids.iter().all(|x| !x.is_empty() && seen.insert(x.as_str())) }

fn unkeyed(a: &[Value], b: &[Value]) -> Vec<Spec> {
    let (lo, ln) = (a.len(), b.len());
    let p = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let s = a.iter().rev().zip(b.iter().rev()).take_while(|(x, y)| x == y).count().min(lo - p).min(ln - p);
    let mut v: Vec<Spec> = (0..p).map(Spec::Old).collect();
    v.extend(b[p..ln - s].iter().cloned().map(Spec::New));
    v.extend((lo - s..lo).map(Spec::Old));
    v
}

fn path_json(p: &[Step]) -> Value { Value::Array(p.iter().map(|s| match s { Step::Key(k) => json!(k), Step::At(i) => json!(i) }).collect()) }

/// The ops as the wire carries them. A set of the very draft the request carried goes as ["t", path].
pub fn ops_json(ops: &[Op], draft: &str) -> Value {
    Value::Array(ops.iter().map(|op| match op {
        Op::Set(p, v) => if !draft.is_empty() && v.as_str() == Some(draft) { json!(["t", path_json(p)]) } else { json!(["s", path_json(p), v]) },
        Op::Rebuild(p, ss) => json!(["a", path_json(p), ss.iter().map(|s| match s { Spec::Old(i) => json!([0, i]), Spec::New(v) => json!([1, v]) }).collect::<Vec<_>>()]),
    }).collect())
}

/// Drafts this long go by reference (shorter ones cost less than finding them).
pub fn draft_is_long(draft: &str) -> bool { draft.chars().count() >= 1024 }

fn undraft(v: &Value, draft: &str) -> Value {
    match v {
        Value::String(s) if s == draft => json!(""),
        Value::Array(x) => Value::Array(x.iter().map(|y| undraft(y, draft)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, y)| (k.clone(), undraft(y, draft))).collect()),
        _ => v.clone(),
    }
}
fn drafted(v: &Value, at: &mut Vec<Step>, draft: &str, out: &mut Vec<Vec<Step>>) {
    match v {
        Value::String(s) if s == draft => out.push(at.clone()),
        Value::Array(x) => for (i, y) in x.iter().enumerate() { at.push(Step::At(i)); drafted(y, at, draft, out); at.pop(); },
        Value::Object(o) => for (k, y) in o { at.push(Step::Key(k.clone())); drafted(y, at, draft, out); at.pop(); },
        _ => {}
    }
}
/// A whole body as ops from nothing, every copy of the long draft by reference (so it never carries the text back).
pub fn whole_ops(body: &Value, draft: &str) -> Value {
    let mut refs = Vec::new(); drafted(body, &mut vec![], draft, &mut refs);
    let mut ops = vec![json!(["s", [], undraft(body, draft)])];
    ops.extend(refs.iter().map(|p| json!(["t", path_json(p)])));
    Value::Array(ops)
}

/// The oracle: ops applied to a base (what src/web/view-delta.js and the native clients do).
pub fn apply(ops: &[Op], base: &Value) -> Value {
    fn at<'a>(v: &'a mut Value, p: &[Step]) -> &'a mut Value {
        match p.split_first() { None => v, Some((Step::Key(k), rest)) => at(v.as_object_mut().unwrap().get_mut(k).unwrap(), rest), Some((Step::At(i), rest)) => at(&mut v.as_array_mut().unwrap()[*i], rest) }
    }
    let mut v = base.clone();
    for op in ops {
        match op {
            Op::Set(p, x) => *at(&mut v, p) = x.clone(),
            Op::Rebuild(p, ss) => { let old = at(&mut v, p).as_array().cloned().unwrap_or_default();
                *at(&mut v, p) = Value::Array(ss.iter().map(|s| match s { Spec::Old(i) => old[*i].clone(), Spec::New(x) => x.clone() }).collect()); }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Rng(u64);
    impl Rng { fn n(&mut self, m: u64) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (self.0 >> 33) % m } }
    fn tree(r: &mut Rng, depth: u32) -> Value {
        match if depth == 0 { r.n(3) } else { r.n(7) } {
            0 => json!(r.n(5)), 1 => json!(format!("s{}", r.n(4))), 2 => json!(r.n(2) == 0),
            3 | 4 => { let n = r.n(4); Value::Object((0..n).map(|i| (format!("k{}", if r.n(5) == 0 { 9 + i } else { i }), tree(r, depth - 1))).collect()) }
            5 => { let n = r.n(6); Value::Array((0..n).map(|_| { let mut o = serde_json::Map::new(); o.insert("id".into(), json!(format!("e{}", r.n(7)))); o.insert("v".into(), tree(r, depth - 1)); Value::Object(o) }).collect()) }
            _ => { let n = r.n(5); Value::Array((0..n).map(|_| tree(r, depth - 1)).collect()) }
        }
    }
    fn mutate(r: &mut Rng, v: &Value, depth: u32) -> Value {
        match v {
            Value::Object(o) if r.n(3) > 0 => Value::Object(o.iter().map(|(k, x)| (k.clone(), if r.n(3) == 0 { mutate(r, x, depth) } else { x.clone() })).collect()),
            Value::Array(a) if r.n(3) > 0 => { let mut a = a.clone(); match r.n(4) { 0 if !a.is_empty() => { a.remove(r.n(a.len() as u64) as usize); } 1 => a.insert(r.n(a.len() as u64 + 1) as usize, tree(r, 2)), 2 if a.len() > 1 => { let i = r.n(a.len() as u64) as usize; let x = a.remove(i); a.insert(r.n(a.len() as u64 + 1) as usize, x) } _ => { if let Some(x) = a.first().cloned() { a[0] = mutate(r, &x, depth) } } } Value::Array(a) }
            _ => if r.n(2) == 0 { tree(r, 2) } else { v.clone() },
        }
    }
    // laws view_delta_same / view_delta_empty_same / the oracle round trip
    #[test] fn view_delta_round_trip_on_random_bodies() {
        let mut r = Rng(7); let mut some = 0;
        for _ in 0..3000 {
            let old = tree(&mut r, 4); let new = if r.n(4) == 0 { tree(&mut r, 4) } else { mutate(&mut r, &old, 3) };
            match diff(&old, &new) { Some(ops) => { assert_eq!(apply(&ops, &old), new, "ops {ops:?}"); assert_eq!(ops.is_empty(), old == new); if !ops.is_empty() { some += 1 } } None => panic!("no fuel problem expected") }
        }
        assert!(some > 1000);
    }
    #[test] fn view_delta_same_is_empty() { let v = json!({"a":[{"id":"x","n":1}]}); assert_eq!(diff(&v, &v), Some(vec![])); }
    #[test] fn keyed_rows_move_and_only_the_changed_one_is_sent() {
        let old = json!({"projects":[{"id":"a","n":1},{"id":"b","n":2},{"id":"c","n":3}]});
        let new = json!({"projects":[{"id":"c","n":3},{"id":"a","n":9},{"id":"d","n":4}]});
        let ops = diff(&old, &new).unwrap();
        assert_eq!(ops[0], Op::Rebuild(vec![Step::Key("projects".into())], vec![Spec::Old(2), Spec::Old(0), Spec::New(json!({"id":"d","n":4}))]));
        assert_eq!(apply(&ops, &old), new);
        assert!(serde_json::to_string(&ops_json(&ops, "")).unwrap().len() < serde_json::to_string(&new).unwrap().len() + 40);
    }
    #[test] fn unkeyed_arrays_keep_prefix_and_suffix() {
        // same length and no identities: the identity lists are equal, so walk diffs pairwise
        assert_eq!(diff(&json!([1, 2, 3]), &json!([1, 9, 3])).unwrap(), vec![Op::Set(vec![Step::At(1)], json!(9))]);
        // a different length: common prefix and suffix kept, the middle new
        let old = json!([1, 2, 3, 4, 5]); let new = json!([1, 2, 9, 9, 9, 5]);
        assert_eq!(diff(&old, &new).unwrap(), vec![Op::Rebuild(vec![], vec![Spec::Old(0), Spec::Old(1), Spec::New(json!(9)), Spec::New(json!(9)), Spec::New(json!(9)), Spec::Old(4)])]);
    }
    #[test] fn objects_with_other_keys_are_set_whole_and_the_draft_goes_by_reference() {
        let old = json!({"s":{"a":1}}); let new = json!({"s":{"b":2}});
        assert_eq!(diff(&old, &new).unwrap(), vec![Op::Set(vec![Step::Key("s".into())], json!({"b":2}))]);
        let ops = diff(&json!({"d":""}), &json!({"d":"typed text"})).unwrap();
        assert_eq!(ops_json(&ops, "typed text"), json!([["t", ["d"]]])); assert_eq!(ops_json(&ops, ""), json!([["s", ["d"], "typed text"]]));
    }
    #[test] fn a_long_draft_is_never_carried_back() {
        let draft = "x".repeat(2000); let body = json!({"session":{"drafts":{"t1": draft.clone()}},"shown":{"composer": draft.clone()}});
        assert!(draft_is_long(&draft)); let w = whole_ops(&body, &draft);
        assert_eq!(w.as_array().unwrap().len(), 3); assert!(!serde_json::to_string(&w).unwrap().contains(&draft));
    }
}
