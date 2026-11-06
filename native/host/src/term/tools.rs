//! The terminal MCP tools: terminal_open, terminal_read, terminal_send,
//! terminal_close. The agents lane's MCP server calls `call` for any name in `NAMES` (and lists them from the JS hub's tool definitions).
use super::{emb, find_act, get, now_s, Terms};
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{sync::{atomic::{AtomicU64, Ordering}, Arc}, time::Duration};

pub const TERMINAL: &[&str] = &["terminal_open", "terminal_read", "terminal_send", "terminal_close"];
/// Every tool this lane serves (the agents lane lists them from the JS hub's definitions and routes them to `call`).
pub const NAMES: &[&str] = &["terminal_open", "terminal_read", "terminal_send", "terminal_close", "preview_open", "preview_navigate", "preview_snapshot", "preview_click", "preview_type", "preview_press", "preview_scroll", "preview_evaluate", "preview_wait_for", "preview_resize", "preview_status", "task_list", "task_get", "task_save", "task_delete", "task_run", "task_step", "task_pause", "task_end"];
static SEQ: AtomicU64 = AtomicU64::new(0);

fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }
fn n(v: &Value, k: &str) -> u64 { v.get(k).and_then(|x| x.as_u64().or_else(|| x.as_f64().map(|f| f.max(0.0) as u64))).unwrap_or(0) }

/// None: not a terminal tool. Some((ok, body)): the tool's answer. A `Value::String` body (terminal_read) is the result's text as it is, not JSON
/// (the JS hub's TermRead.answer); an object `{"$content": [blocks]}` (preview_*) is the result's content array as it is (text and image blocks);
/// anything else is the JSON text.
pub async fn call(hub: &Arc<Hub>, thread: &str, name: &str, a: &Value) -> Option<(bool, Value)> {
    if !NAMES.contains(&name) { return None }
    if name.starts_with("task_") { return Some(match super::driver::get(hub) { Some(d) => d.tool(thread, name, a).await, None => fail("tasks are not available") }) }
    if name.starts_with("preview_") { return Some(super::browser::preview(hub, thread, name, a).await) }
    let Some(t) = get(hub) else { return Some(fail("terminals are not available")) };
    Some(match name {
        "terminal_open" => open(&t, hub, thread, a).await,
        "terminal_read" => read(&t, thread, a).await,
        "terminal_send" => send(&t, hub, thread, a).await,
        _ => close(&t, hub, thread, a).await,
    })
}

async fn open(t: &Arc<Terms>, hub: &Arc<Hub>, thread: &str, a: &Value) -> (bool, Value) {
    let cmd = sv(a, "command");
    if cmd.trim().is_empty() { return fail("command is empty") }
    if cmd.len() > 65536 { return fail("command is too long") }
    if t.running_of(thread).await.len() >= super::MAX_PER_THREAD { return fail("too many terminals are open in this chat: close one first") }
    let (cols, rows) = (emb::cols(n(a, "cols").min(100000) as u32), emb::rows(n(a, "rows").min(100000) as u32));
    let title = emb::title_of(sv(a, "title"), cmd);
    let now = now_s(); let id = format!("x{now}-{}", hub.store.len() as u64 + SEQ.fetch_add(1, Ordering::Relaxed));
    let ev = json!({"$":"ActivityLogged","thread":thread,"act":id,"tone":emb::TONE,"kind":emb::KIND,"summary":emb::summary(&title, cols, rows, cmd),"at":now});
    if hub.commit(&ev).await.is_err() { return fail("could not write the terminal's entry") }
    let cwd = t.cwd(thread).await;
    t.run(&emb::key(thread, &id), &title, &cwd, cmd, cols, rows, false).await;
    (true, json!({"id":id,"cols":cols,"rows":rows,"state":"running"}))
}

/// The actor's answer again every 250 ms while the program runs and waitMs lasts (at most 120 s).
async fn read(t: &Arc<Terms>, thread: &str, a: &Value) -> (bool, Value) {
    let key = emb::key(thread, sv(a, "id"));
    let mut left = n(a, "waitMs").min(120_000) / 250;
    loop {
        let Some((state, report)) = t.read(&key).await else { return (false, Value::String("There is no such terminal in this thread.".into())) };
        if state != "running" || left == 0 { return (true, Value::String(report)) }
        left -= 1; tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn exists(t: &Arc<Terms>, hub: &Arc<Hub>, thread: &str, id: &str) -> bool { t.read_known(&emb::key(thread, id)).await || find_act(hub, thread, id).await.is_some() }

async fn send(t: &Arc<Terms>, hub: &Arc<Hub>, thread: &str, a: &Value) -> (bool, Value) {
    let id = sv(a, "id");
    if id.is_empty() || !exists(t, hub, thread, id).await { return fail(&format!("no terminal {id} in this thread")) }
    let text = sv(a, "text");
    if text.len() > super::MAX_INPUT { return fail("text is too long") }
    t.input(&emb::key(thread, id), text.as_bytes().to_vec()).await;
    (true, json!({"sent": text.chars().count()}))
}

async fn close(t: &Arc<Terms>, hub: &Arc<Hub>, thread: &str, a: &Value) -> (bool, Value) {
    let id = sv(a, "id");
    if id.is_empty() || !exists(t, hub, thread, id).await { return fail(&format!("no terminal {id} in this thread")) }
    t.kill(&emb::key(thread, id)).await;
    (true, json!({"state":"stopped"}))
}

/// For the agents lane, when the person sends a message: what the thread's terminals show now that the agent has not seen (the
/// `<terminals>` block, "" when none), after which the terminals not pinned are stopped.
pub async fn on_person_message(hub: &Arc<Hub>, thread: &str) -> String {
    let Some(t) = get(hub) else { return String::new() };
    let note = t.note(thread).await;
    for key in t.running_of(thread).await { if hub.setting(&emb::pin_key(&key)).await.as_deref() != Some("1") { t.kill(&key).await } }
    note
}
