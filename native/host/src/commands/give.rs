//! /give (`bots.give {thread, text}`): the work of a thread handed to every thread (>), project (%) and bot (@) the text names, the rest of the text riding along as the person's
//! note. What goes is a handoff (handoff.rs): a summary the writing model makes of the conversation (the thread's own last words when it cannot be run), the references of
//! what the work touched (its files and boards, the message that asked and the one that answered, the thread itself) and the note. A thread gets it as a message from the
//! source: a running turn is steered at once, an idle thread starts one (agents `thread.hand`); a project gets a new thread that way; a bot is woken with it. Each receiving
//! thread logs the card (summary, references, a link back); the source logs "Given to ...". Without the agents module (a test hub) the text waits in the thread ahead of its
//! next message, as a fork brought back. A thread never gives to itself, and a text that names nothing to give to is refused, in words the person can act on (Give.*, Hand.give).
use super::{entries_of, handoff, hand_text};
use crate::{api::Caller, hub::Hub, session::composer::mention};
use serde_json::{json, Value};
use std::sync::Arc;

pub const NOTHING_NAMED: &str = "Name a thread (>), a project (%) or a bot (@) to give this thread to.";
fn refuse(id: &Value, why: &str) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":false,"error":why})] }
fn said(src: &str, act: &str, to: &str, at: u64) -> Value { json!({"$":"ActivityLogged","thread":src,"act":act,"tone":"info","kind":"fork","summary":format!("Given to {to}"),"at":at}) }
/// Hand.give: what the writing model is told it reads.
fn why(title: &str, src: &str) -> String { format!("The user hands the work of the thread \"{title}\" ({src}) to another agent. Its conversation, oldest first:") }
fn inner() -> Caller { Caller { gen: "give".into(), client: 0 } }

/// The writing model's summary of the thread for the one taking over, or None (it cannot be run, or said nothing).
async fn write(hub: &Arc<Hub>, input: String) -> Option<String> {
    let h = hub.rpc_for("text.write")?;
    let r = h.call(hub, &inner(), &json!(0), "text.write", &json!({"system": crate::agents::textgen::GIVE_SYSTEM, "input": input})).await.into_iter().next()?;
    let t = crate::agents::textgen::give_clean(r["result"]["text"].as_str().unwrap_or(""));
    (!t.is_empty()).then_some(t)
}

/// `modes`: the source thread's runtime and interaction modes (the log's fold keeps them); a new thread in a project takes them, like a fork.
pub async fn give(hub: &Arc<Hub>, who: &Caller, id: &Value, p: &Value, modes: (String, String)) -> Vec<Value> {
    let (runtime, interaction) = modes;
    let (src, text) = (p["thread"].as_str().unwrap_or(""), p["text"].as_str().unwrap_or(""));
    let dir = crate::bots::api::directory(hub).await;
    let bots: Vec<(String, String)> = dir["bots"].as_array().into_iter().flatten().map(|b| (b["id"].as_str().unwrap_or("").to_string(), b["name"].as_str().unwrap_or("").to_string())).collect();
    let (threads, projects, title, provider, model) = {
        let m = hub.model().await;
        let Some(t) = m.threads.get(src).filter(|t| !t.deleted) else { return refuse(id, "no such thread") };
        let threads: Vec<(String, String)> = m.threads.iter().filter(|(_, t)| !t.deleted).map(|(i, t)| (i.clone(), t.title.clone())).collect();
        let projects: Vec<(String, String)> = m.projects.iter().filter(|(_, x)| !x.removed).map(|(i, x)| (i.clone(), x.title.clone())).collect();
        (threads, projects, t.title.clone(), t.provider.clone(), t.model.clone())
    };
    let hits: Vec<_> = mention::give_hits(text, &threads, &projects, &bots).into_iter().filter(|h| !(h.0 == '>' && h.1 == src)).collect();
    if hits.is_empty() { return refuse(id, NOTHING_NAMED) }
    let note = handoff::note_of(text, |w| mention::give_hits(w, &threads, &projects, &bots).iter().any(|h| !(h.0 == '>' && h.1 == src)));
    let es = entries_of(hub, src).await.entries;
    let root = crate::files::root_of(hub, src).await.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let refs = handoff::references(&es, src, &title, &root);
    let input = format!("{}\n\nThe user's note for the next agent: {}", hand_text(&why(&title, src), &es, src), if note.is_empty() { "(none)" } else { &note });
    let summary = match write(hub, input).await { Some(s) => s, None => handoff::fallback_summary(&title, &es) };
    let card = handoff::card(src, &title, &note, &summary, &refs);
    let hand = handoff::agent_text(&card);
    let at = super::now_s(); let base = format!("{at}-{}", hub.store.len());
    let mut failed = String::new();
    for (i, (sigil, to, name)) in hits.iter().enumerate() {
        let key = format!("{base}.{i}"); let (msg, act) = (format!("handoff:give:{src}:{key}"), format!("k{key}"));
        let queued = |th: &str| json!({"$":"TurnQueued","thread":th,"msg":msg,"text":hand,"at":at});
        // the receiving thread: delivered now (a running turn is steered, an idle thread starts one), or, with no agents module, left to wait for its next message
        let send = |th: String, extra: Vec<Value>| { let (hub, hand, msg, src, queued) = (hub.clone(), hand.clone(), msg.clone(), src.to_string(), queued(&th)); async move {
            match hub.rpc_for("thread.hand") {
                Some(h) => { let r = h.call(&hub, &inner(), &json!(0), "thread.hand", &json!({"thread": th, "msg": msg, "text": hand, "from": format!("thread:{src}"), "extra": extra})).await.into_iter().next().unwrap_or(Value::Null);
                    if r["ok"] == true { Ok(()) } else { Err(r["error"].as_str().unwrap_or("the thread could not be written to").to_string()) } }
                None => { let mut ev = vec![queued]; ev.extend(extra); hub.commit_many(&ev).await.map(|_| ()).map_err(|_| "store write failed".to_string()) }
            } } };
        let result: Result<(), String> = match sigil {
            '>' => send(to.clone(), vec![handoff::card_event(to, &format!("{act}.in"), at, &card), said(src, &act, &format!("\"{name}\""), at)]).await,
            '%' => {
                let Some(h) = hub.rpc_for("thread.create") else { continue };
                let r = h.call(hub, who, &json!(0), "thread.create", &json!({"project": to, "title": title, "provider": provider, "model": model})).await.into_iter().next().unwrap_or(Value::Null);
                let Some(th) = r["thread"].as_str().filter(|_| r["ok"] == true).map(str::to_string) else { failed = r["error"].as_str().unwrap_or("the new thread could not be made").to_string(); continue };
                let modes = json!({"$":"ModesSet","id":th,"runtime":if runtime.is_empty() { "full-access" } else { &runtime },"interaction":if interaction.is_empty() { "default" } else { &interaction },"model":model,"at":at});
                match hub.commit_many(&[modes]).await { Ok(_) => send(th.clone(), vec![handoff::card_event(&th, &format!("{act}.in"), at, &card), said(src, &act, &format!("a new thread in {name}"), at)]).await, Err(_) => Err("store write failed".into()) }
            }
            _ => {
                let Some(h) = hub.rpc_for("bots.say") else { continue };
                let r = h.call(hub, who, &json!(0), "bots.say", &json!({"bot": to, "text": hand, "msg": format!("m{key}")})).await.into_iter().next().unwrap_or(Value::Null);
                if r["ok"] != true { Err(r["error"].as_str().unwrap_or("the bot could not be woken").to_string()) } else {
                    let thread = dir["bots"].as_array().into_iter().flatten().find(|b| b["id"] == to.as_str()).and_then(|b| b["thread"].as_str()).unwrap_or("").to_string();
                    let mut ev = vec![said(src, &act, &format!("@{name}"), at)]; if !thread.is_empty() { ev.push(handoff::card_event(&thread, &format!("{act}.in"), at, &card)) }
                    hub.commit_many(&ev).await.map(|_| ()).map_err(|_| "store write failed".to_string()) }
            }
        };
        if let Err(e) = result { failed = e }
    }
    if failed.is_empty() { vec![json!({"t":"reply","id":id,"ok":true})] } else { refuse(id, &failed) }
}
