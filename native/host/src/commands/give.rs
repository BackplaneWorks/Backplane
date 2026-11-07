//! /give (`bots.give {thread, text}`): the conversation of a thread handed to every thread (>), project (%) and bot (@) the text names, the rest of the text riding along as the
//! person's note. A thread gets it queued ahead of its next message, like a fork brought back; a project gets a new thread with it queued, like a fork; a bot is woken with it. A thread
//! never gives to itself, and a text that names nothing to give to is refused, in words the person can act on (Give.*, Hand.give).
use super::{entries_of, hand_text};
use crate::{api::Caller, hub::Hub, session::composer::mention};
use serde_json::{json, Value};
use std::sync::Arc;

pub const NOTHING_NAMED: &str = "Name a thread (>), a project (%) or a bot (@) to give this thread to.";
fn refuse(id: &Value, why: &str) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":false,"error":why})] }
fn said(src: &str, act: &str, to: &str, at: u64) -> Value { json!({"$":"ActivityLogged","thread":src,"act":act,"tone":"info","kind":"fork","summary":format!("Given to {to}"),"at":at}) }
/// Hand.give: what was said and done in the thread given from, with the person's note.
fn why(title: &str, src: &str, note: &str) -> String {
    format!("The user hands you the conversation of the thread \"{title}\" ({src}).{} Its conversation, oldest first:", if note.is_empty() { String::new() } else { format!(" The user's note with it: {note}") })
}

/// `modes`: the source thread's runtime and interaction modes (the log's fold keeps them); a new thread in a project takes them, like a fork.
pub async fn give(hub: &Arc<Hub>, who: &Caller, id: &Value, p: &Value, modes: (String, String)) -> Vec<Value> {
    let (runtime, interaction) = modes;
    let (src, text) = (p["thread"].as_str().unwrap_or(""), p["text"].as_str().unwrap_or(""));
    let bots: Vec<(String, String)> = crate::bots::api::directory(hub).await["bots"].as_array().into_iter().flatten().map(|b| (b["id"].as_str().unwrap_or("").to_string(), b["name"].as_str().unwrap_or("").to_string())).collect();
    let (hits, title, provider, model) = {
        let m = hub.model().await;
        let Some(t) = m.threads.get(src).filter(|t| !t.deleted) else { return refuse(id, "no such thread") };
        let threads: Vec<(String, String)> = m.threads.iter().filter(|(_, t)| !t.deleted).map(|(i, t)| (i.clone(), t.title.clone())).collect();
        let projects: Vec<(String, String)> = m.projects.iter().filter(|(_, x)| !x.removed).map(|(i, x)| (i.clone(), x.title.clone())).collect();
        (mention::give_hits(text, &threads, &projects, &bots), t.title.clone(), t.provider.clone(), t.model.clone())
    };
    let hits: Vec<_> = hits.into_iter().filter(|h| !(h.0 == '>' && h.1 == src)).collect();
    if hits.is_empty() { return refuse(id, NOTHING_NAMED) }
    let note = text.trim().get(5..).unwrap_or("").trim();
    let hand = hand_text(&why(&title, src, note), &entries_of(hub, src).await.entries, src);
    let at = super::now_s(); let base = format!("{at}-{}", hub.store.len());
    let mut failed = String::new();
    for (i, (sigil, to, name)) in hits.iter().enumerate() {
        let key = format!("{base}.{i}"); let (msg, act) = (format!("handoff:give:{src}:{key}"), format!("k{key}"));
        let queued = |th: &str| json!({"$":"TurnQueued","thread":th,"msg":msg,"text":hand,"at":at});
        let events = match sigil {
            '>' => vec![queued(to), json!({"$":"ActivityLogged","thread":to,"act":format!("{act}.in"),"tone":"info","kind":"fork","summary":format!("\"{title}\" was handed over; it goes with the next message"),"at":at}), said(src, &act, &format!("\"{name}\""), at)],
            '%' => {
                let Some(h) = hub.rpc_for("thread.create") else { continue };
                let r = h.call(hub, who, &json!(0), "thread.create", &json!({"project": to, "title": title, "provider": provider, "model": model})).await.into_iter().next().unwrap_or(Value::Null);
                let Some(th) = r["thread"].as_str().filter(|_| r["ok"] == true).map(str::to_string) else { failed = r["error"].as_str().unwrap_or("the new thread could not be made").to_string(); continue };
                vec![json!({"$":"ModesSet","id":th,"runtime":if runtime.is_empty() { "full-access" } else { &runtime },"interaction":if interaction.is_empty() { "default" } else { &interaction },"model":model,"at":at}), queued(&th), said(src, &act, &format!("a new thread in {name}"), at)]
            }
            _ => {
                let Some(h) = hub.rpc_for("bots.say") else { continue };
                let r = h.call(hub, who, &json!(0), "bots.say", &json!({"bot": to, "text": hand, "msg": format!("m{key}")})).await.into_iter().next().unwrap_or(Value::Null);
                if r["ok"] != true { failed = r["error"].as_str().unwrap_or("the bot could not be woken").to_string(); continue }
                vec![said(src, &act, &format!("@{name}"), at)]
            }
        };
        if hub.commit_many(&events).await.is_err() { failed = "store write failed".into(); }
    }
    if failed.is_empty() { vec![json!({"t":"reply","id":id,"ok":true})] } else { refuse(id, &failed) }
}
