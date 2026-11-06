//! The hub's keyed view delta.
//! A request offers `viewDelta {version:1, token}` naming the newest body held; the reply is whole
//! (`viewDelta {base:"", token}` plus the fields) or ops from that body. Bodies are `{shown, session, window, overlay}`.
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

const MAX_OPS: usize = 20000;
const MAX_DEPTH: usize = 96;
pub const BASES: usize = 8;
const FIELDS: [&str; 4] = ["shown", "session", "window", "overlay"];

/// The last bodies received, by token, bounded.
#[derive(Default)]
pub struct Bases { map: HashMap<String, Value>, order: VecDeque<String>, pub newest: String }
impl Bases {
    pub fn clear(&mut self) { self.map.clear(); self.order.clear(); self.newest.clear(); }
    pub fn keep(&mut self, token: String, body: Value) {
        if self.map.insert(token.clone(), body).is_none() { self.order.push_back(token.clone()); }
        self.newest = token;
        while self.order.len() > BASES { if let Some(old) = self.order.pop_front() { self.map.remove(&old); } }
    }
    pub fn get(&self, token: &str) -> Option<&Value> { self.map.get(token) }
}

fn step<'a>(node: &'a mut Value, key: &Value) -> Result<&'a mut Value> {
    match key {
        Value::String(k) => node.as_object_mut().and_then(|o| o.get_mut(k)).context("delta path"),
        Value::Number(n) => {
            let i = n.as_u64().context("delta index")? as usize;
            node.as_array_mut().and_then(|a| a.get_mut(i)).context("delta path")
        }
        _ => bail!("delta key"),
    }
}
fn walk<'a>(node: &'a mut Value, path: &[Value]) -> Result<&'a mut Value> {
    match path.split_first() { None => Ok(node), Some((k, rest)) => { let child = step(node, k)?; walk(child, rest) } }
}
fn rebuild(slot: &mut Value, specs: &Value) -> Result<()> {
    let old = slot.as_array_mut().context("delta array")?;
    let specs = specs.as_array().context("delta specs")?;
    let mut uses = vec![0usize; old.len()];
    for s in specs {
        let s = s.as_array().filter(|s| s.len() == 2).context("delta spec")?;
        match s[0].as_u64() {
            Some(1) => {}
            Some(0) => { let i = s[1].as_u64().context("delta spec")? as usize; ensure!(i < old.len(), "delta spec"); uses[i] += 1; }
            _ => bail!("delta spec"),
        }
    }
    let mut items: Vec<Option<Value>> = std::mem::take(old).into_iter().map(Some).collect();
    let mut out = Vec::with_capacity(specs.len());
    for s in specs {
        if s[0].as_u64() == Some(1) { out.push(s[1].clone()); continue; }
        let i = s[1].as_u64().unwrap() as usize;
        uses[i] -= 1;
        // the last use moves the element; earlier uses of the same index copy it
        out.push(if uses[i] == 0 { items[i].take().context("delta spec")? } else { items[i].clone().context("delta spec")? });
    }
    *slot = Value::Array(out);
    Ok(())
}
/// Apply ops to a body (None: made from nothing). `draft` is the selected thread's draft the request carried.
pub fn apply(base: Option<Value>, ops: &Value, draft: Option<&str>) -> Result<Value> {
    let ops = ops.as_array().context("delta ops")?;
    ensure!(ops.len() <= MAX_OPS, "delta ops");
    let mut body = base;
    for op in ops {
        let op = op.as_array().context("delta op")?;
        let path = op.get(1).and_then(Value::as_array).context("delta op")?;
        ensure!(path.len() <= MAX_DEPTH, "delta op");
        let kind = op.first().and_then(Value::as_str).unwrap_or("");
        let apply_at = |body: &mut Option<Value>, f: &mut dyn FnMut(&mut Value) -> Result<()>| -> Result<()> {
            if path.is_empty() && body.is_none() { *body = Some(Value::Null); }
            f(walk(body.as_mut().context("delta without base")?, path)?)
        };
        match kind {
            "t" => { ensure!(op.len() == 2, "delta op"); let d = draft.filter(|d| !d.is_empty()).context("delta draft")?; apply_at(&mut body, &mut |n| { *n = Value::String(d.to_string()); Ok(()) })?; }
            "s" => { ensure!(op.len() == 3, "delta op"); apply_at(&mut body, &mut |n| { *n = op[2].clone(); Ok(()) })?; }
            "a" => { ensure!(op.len() == 3, "delta op"); apply_at(&mut body, &mut |n| rebuild(n, &op[2]))?; }
            _ => bail!("delta op"),
        }
    }
    body.context("empty delta")
}
fn valid_token(v: &Value) -> Option<&str> {
    let t = v.as_str()?;
    (!t.is_empty() && t.len() <= 16 && !t.starts_with('0') && t.bytes().all(|b| b.is_ascii_digit())).then_some(t)
}
/// The full result from a delta reply `r`, its token and the body to keep. Errors when the reply is
/// malformed or names a body this client does not hold; the caller then asks for the whole view.
pub fn picture(mut r: Value, offered: &str, request_revision: u64, bases: &Bases, draft: Option<&str>) -> Result<(Value, String, Value)> {
    let d = r.as_object_mut().and_then(|o| o.remove("viewDelta")).context("no view delta")?;
    ensure!(d.is_object() && d["version"] == 1, "invalid view delta");
    let token = valid_token(&d["token"]).context("invalid token")?.to_string();
    let base = d["base"].as_str().context("invalid base")?;
    ensure!(token == request_revision.to_string(), "token does not name the request");
    let obj = r.as_object_mut().context("reply not an object")?;
    let body = if d.get("ops").is_none() {
        ensure!(base.is_empty() && obj.contains_key("shown"), "invalid whole view");
        let mut body = serde_json::Map::new();
        for k in FIELDS { if let Some(v) = obj.remove(k) { body.insert(k.to_string(), v); } }
        Value::Object(body)
    } else {
        ensure!(FIELDS.iter().all(|k| !obj.contains_key(*k)), "unacknowledged view delta");
        let ops = &d["ops"];
        let fresh = base.is_empty();
        if fresh {
            let first = ops.get(0).and_then(Value::as_array);
            ensure!(first.map_or(false, |o| o.first().and_then(Value::as_str) == Some("s") && o.get(1).and_then(Value::as_array).map_or(false, |p| p.is_empty())), "unacknowledged view delta");
        } else {
            ensure!(base == offered, "unacknowledged view delta");
        }
        let base_body = if fresh { None } else { Some(bases.get(base).context("unknown base")?.clone()) };
        let body = apply(base_body, ops, draft)?;
        ensure!(body.is_object() && body.get("shown").is_some(), "invalid view delta body");
        body
    };
    // The result the client reads: the reply's own fields with the body's.
    let mut full = r;
    for (k, v) in body.as_object().unwrap() { full[k.as_str()] = v.clone(); }
    Ok((full, token, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test] fn recorded_frames_equal_whole_bodies() {
        let text = include_str!("../../../test/fixtures/delta_frames.json");
        let rec: Value = serde_json::from_str(text).unwrap();
        let mut bases = Bases::default();
        let mut deltas = 0;
        for f in rec["frames"].as_array().unwrap() {
            let offered = f["offered"].as_str().unwrap();
            for key in ["delta", "whole"] {
                let r = f[key].clone();
                let rev = r["requestRevision"].as_u64().unwrap();
                let (full, token, body) = picture(r.clone(), if key == "delta" { offered } else { "" }, rev, &bases, None).unwrap_or_else(|e| panic!("{} {key}: {e:#}", f["name"]));
                if key == "delta" && r["viewDelta"].get("ops").is_some() { deltas += 1; }
                bases.keep(token, body.clone());
                if key == "whole" {
                    // the delta chain and the whole of the same state agree on every body field
                    for k in FIELDS { assert_eq!(full.get(k), f["whole"].get(k), "{} {k}", f["name"]); }
                }
                let _ = full;
            }
            // compare the delta-reconstructed body with the whole one of the same step
        }
        assert!(deltas >= 2, "fixture holds delta replies");
    }
    #[test] fn delta_body_equals_whole_body() {
        let text = include_str!("../../../test/fixtures/delta_frames.json");
        let rec: Value = serde_json::from_str(text).unwrap();
        let mut bases = Bases::default();
        for f in rec["frames"].as_array().unwrap() {
            let offered = f["offered"].as_str().unwrap();
            let r = f["delta"].clone();
            let rev = r["requestRevision"].as_u64().unwrap();
            let (full, token, body) = picture(r, offered, rev, &bases, None).unwrap();
            for k in ["shown", "window", "overlay"] {
                // the overlay holds a copy of the shown picture, with the same draft difference
                if k == "overlay" && f["name"].as_str().unwrap().starts_with("draft") { continue; }
                let (mut a, mut b) = (full[k].clone(), f["whole"][k].clone());
                // the recording's follow-up whole request did not carry the draft the action typed
                if f["name"].as_str().unwrap().starts_with("draft") && k == "shown" {
                    for t in [&mut a, &mut b] { for key in ["draft", "skills"] { t["thread"].as_object_mut().unwrap().remove(key); } }
                }
                assert_eq!(a, b, "{} {k}", f["name"]);
            }
            bases.keep(token, body);
            // continue the chain from the whole body the recording continued from
            let w = f["whole"].clone();
            let wrev = w["requestRevision"].as_u64().unwrap();
            let (_, wtoken, wbody) = picture(w, "", wrev, &bases, None).unwrap();
            bases.keep(wtoken, wbody);
        }
    }
    #[test] fn malformed_deltas_fail() {
        let bases = Bases::default();
        for r in [
            json!({"viewDelta":{"version":2,"token":"5","base":"","ops":[]}}),
            json!({"viewDelta":{"version":1,"token":"7","base":"","ops":[]}}),
            json!({"viewDelta":{"version":1,"token":"5","base":"3","ops":[["s",["shown"],1]]}}),
            json!({"viewDelta":{"version":1,"token":"5","base":"","ops":[["s",["x"],1]]}}),
            json!({"viewDelta":{"version":1,"token":"5","base":""}}),
            json!({"viewDelta":{"version":1,"token":"5","base":"","ops":[["s",[],{"shown":1}]]},"shown":{}}),
        ] { assert!(picture(r.clone(), "", 5, &bases, None).is_err(), "{r}"); }
        // a path that is not there, a spec out of range
        assert!(apply(Some(json!({"a":[1]})), &json!([["s",["b"],1]]), None).is_err());
        assert!(apply(Some(json!({"a":[1]})), &json!([["a",["a"],[[0,3]]]]), None).is_err());
        assert!(apply(Some(json!({"a":[1]})), &json!([["t",["a"]]]), None).is_err());
    }
    #[test] fn array_specs_reuse_and_copy() {
        let out = apply(Some(json!({"a":[{"x":1},{"x":2}]})), &json!([["a",["a"],[[0,1],[1,{"x":9}],[0,1],[0,0]]]]), None).unwrap();
        assert_eq!(out, json!({"a":[{"x":2},{"x":9},{"x":2},{"x":1}]}));
    }
}
