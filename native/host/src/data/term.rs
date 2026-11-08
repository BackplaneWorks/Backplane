//! Terminals on a phone (feature `terms`; docs/PHONE-DATA.md, Terminals). The hub's own terminals (term/mod.rs): the chat terminals a thread's
//! entries show (`kind:"term"`), the threads' floating shells, and shells a phone opens itself. A data/1 socket holds the hub's token or a paired
//! device's, and every pairing is full access (a linked machine never holds one), so it may type into them as the desktop does.
//! Output comes as pushes on the socket only while the phone watches a terminal (`term.watch`), each saying where its bytes start, so a phone
//! that was away asks for what came after the offset it holds (`term.read`) and gets only that, or a snapshot when that is gone.
use crate::term::{self, emb};
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{collections::HashSet, sync::Arc};

/// Terminals one socket watches at once.
pub const MAX_WATCH: usize = 16;
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn err(m: &str) -> Value { json!(m) }
fn size(p: &Value, k: &str, d: u64) -> u16 { p.get(k).and_then(Value::as_u64).unwrap_or(d).clamp(1, 1000) as u16 }
fn b64(b: &[u8]) -> String { use base64::Engine; base64::engine::general_purpose::STANDARD.encode(b) }
fn unb64(t: &str) -> Option<Vec<u8>> { use base64::Engine; base64::engine::general_purpose::STANDARD.decode(t).ok() }
/// A key a phone may name: a chat terminal's, a shell's, or a thread's own (its id).
fn key_ok(k: &str) -> bool { !k.is_empty() && k.len() <= 256 && k.bytes().all(|b| b.is_ascii_alphanumeric() || b":._-|".contains(&b)) }

/// The thread of a project's terminals: those of its threads and its own shells.
async fn in_project(hub: &Hub, project: &str, t: &Value) -> bool {
    let k = s(t, "key");
    if let Some(r) = k.strip_prefix("s:") { return r.split(':').next() == Some(project) }
    let th = term::thread_of(k); hub.model().await.threads.get(th).map_or(false, |x| x.project == project)
}

/// The requests; None: not one of these. `watch` is the socket's set of watched keys.
pub async fn call(hub: &Arc<Hub>, watch: &mut HashSet<String>, m: &str, p: &Value) -> Option<Result<Value, Value>> {
    if !matches!(m, "term.list" | "term.read" | "term.watch" | "term.unwatch" | "term.write" | "term.resize" | "term.new" | "term.stop") { return None }
    let Some(t) = term::get(hub) else { return Some(Err(err("terminals are not available"))) };
    let key = s(p, "key");
    if !matches!(m, "term.list" | "term.new") && !key_ok(key) { return Some(Err(err("no such terminal"))) }
    Some(match m {
        "term.list" => {
            let (project, thread) = (s(p, "project"), s(p, "thread"));
            let mut out = vec![];
            for x in t.list("").await {
                if !thread.is_empty() && term::thread_of(s(&x, "key")) != thread { continue }
                if !project.is_empty() && !in_project(hub, project, &x).await { continue }
                out.push(x);
            }
            Ok(json!({"terms":out}))
        }
        // a phone that holds run `g` up to `from` asks for what it lacks; `watch` also streams it from then on
        "term.read" | "term.watch" => {
            let (g, from) = (p.get("g").and_then(Value::as_u64).unwrap_or(0), p.get("from").and_then(Value::as_u64));
            match t.stream_read(key, g, from).await {
                None => Err(err("no such terminal")),
                Some(r) => {
                    if m == "term.watch" { if !watch.contains(key) && watch.len() >= MAX_WATCH { return Some(Err(err("watching too many terminals"))) } watch.insert(key.to_string()); }
                    Ok(r)
                }
            }
        }
        "term.unwatch" => { watch.remove(key); Ok(json!({})) }
        "term.write" => {
            let bs = match p.get("b").and_then(Value::as_str) { Some(b) => match unb64(b) { Some(x) => x, None => return Some(Err(err("b is not base64"))) }, None => s(p, "text").as_bytes().to_vec() };
            if bs.len() > term::MAX_INPUT { return Some(Err(err("too much at once"))) }
            if bs.is_empty() { return Some(Ok(json!({"sent":0}))) }
            let n = bs.len();
            if t.input(key, bs).await { Ok(json!({"sent":n})) } else { Err(err("the terminal is not running")) }
        }
        "term.resize" => {
            if !t.known(key).await { return Some(Err(err("no such terminal"))) }
            t.resize(key, size(p, "cols", 80), size(p, "rows", 24)).await; Ok(json!({}))
        }
        "term.stop" => { if !t.known(key).await { return Some(Err(err("no such terminal"))) } t.kill(key).await; Ok(json!({})) }
        "term.new" => {
            let (project, thread) = (s(p, "project"), s(p, "thread"));
            let (pk, cwd) = if !thread.is_empty() {
                let pk = hub.model().await.threads.get(thread).map(|x| x.project.clone());
                let Some(pk) = pk else { return Some(Err(err("unknown thread"))) };
                (pk, t.cwd(thread).await)
            } else if !project.is_empty() {
                let root = hub.model().await.projects.get(project).filter(|x| !x.removed).map(|x| x.root.clone());
                let Some(root) = root else { return Some(Err(err("unknown project"))) };
                (project.to_string(), root)
            } else { (String::new(), std::env::var("HOME").unwrap_or_default()) };
            match t.new_shell(&pk, &cwd, size(p, "cols", 80), size(p, "rows", 24)).await {
                Ok(k) => Ok(t.stream_read(&k, 0, None).await.unwrap_or_else(|| json!({"key":k}))),
                Err(e) => Err(json!(e)),
            }
        }
        _ => Err(err("unknown method")),
    })
}

/// A hub `term` frame as the push a watching socket gets: `{t:"d", k:"term", key, g?, o?, b (base64), reset?, state?}` (None: not watched).
pub fn push_of(watch: &HashSet<String>, f: &Value) -> Option<Value> {
    if s(f, "t") != "term" { return None }
    let key = s(f, "thread"); if !watch.contains(key) { return None }
    let b: Vec<u8> = f["b"].as_array().map(|a| a.iter().map(|x| x.as_u64().unwrap_or(0) as u8).collect()).unwrap_or_default();
    let mut o = json!({"t":"d","k":"term","key":key,"b":b64(&b)});
    for k in ["g", "o", "reset", "state"] { if let Some(x) = f.get(k) { o[k] = x.clone() } }
    Some(o)
}

/// The entry a chat terminal's ActivityLogged is, for a phone: `{id, kind:"term", at, term:{key, id, title, cols, rows, cmd}}`.
pub fn entry(thread: &str, id: &str, summary: &str, at: u64) -> Value {
    let sm = emb::parse_summary(summary);
    json!({"id":id,"kind":"term","text":sm.title,"at":at,"term":{"key":emb::key(thread, id),"id":id,"title":sm.title,"cols":sm.cols,"rows":sm.rows,"cmd":sm.cmd}})
}
