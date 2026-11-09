//! What the hub says about an order and draws of it, with no dependency on the agents: the quote's words, the Order card, the Buy now link.
//! present/ uses it (and the screen-audit bin, which builds present/ without the agents); settings/order.rs re-exports all of it.
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{LazyLock, RwLock}, time::Duration};

fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
pub const STATUS_SENT: &str = "The files are sent; the quote is being made (usually 1-2 minutes). Backplane posts it in this thread and tells you. Nothing is wrong and there is nothing for the person to press: say it is on its way and end your turn.";
pub const LAST_MARK: &str = " · last time";
pub fn ask_value(choice: &str) -> &str { choice.strip_suffix(LAST_MARK).map_or(choice, str::trim) }

// ---- what a quote says ----

/// Order.payable: worked out and nothing to fix.
pub fn payable(q: &Value) -> bool { sv(q, "status") == "ready" && q["quote"]["problems"].as_array().map_or(true, Vec::is_empty) }
/// Order.pending: the order server is still working it out.
pub fn pending(q: &Value) -> bool { matches!(sv(q, "status"), "received" | "analysing") }
/// Order.ddp
pub fn ddp(q: &Value) -> bool { q["quote"]["shipping"]["ddp"].as_bool().unwrap_or(false) }
/// Order.customs: what the price covers about customs, said where the total is.
pub fn customs(ddp: bool) -> &'static str { if ddp { "Includes import VAT and duties: nothing to pay on delivery." } else { "Excludes customs: the courier will contact you to pay any duties and taxes before delivery." } }
/// Order.state: the order's state once there is one (a checkout that expired unpaid is no order); on a pilot server paid is "pilot": nothing was charged or ordered.
pub fn state(q: &Value) -> String {
    let order = sv(&q["order"], "status"); let st = if order.is_empty() || order == "cancelled" { sv(q, "status") } else { order };
    if q["pilot"].as_bool().unwrap_or(false) && st == "paid" { "pilot".into() } else { st.to_string() }
}
/// Order.done: nothing more will happen to it.
pub fn done(st: &str) -> bool { matches!(st, "delivered" | "refunded" | "pilot" | "expired" | "failed" | "gone") }
/// Order.every: while unpaid it is checked each minute, after that every ten.
pub fn every(st: &str) -> Duration { Duration::from_secs(if matches!(st, "ready" | "quoted" | "") { 60 } else { 600 }) }
/// Order.status.text: an order's progress, said once each in the buy thread.
pub fn status_text(st: &str, tracking: &str) -> String {
    match st {
        "paid" => "**Paid.** Thank you. We're placing the order now; this thread will say when it ships.".into(),
        "placed" => "**Ordered.** It's in production.".into(),
        "shipped" => if tracking.is_empty() { "**Shipped.**".into() } else { format!("**Shipped.** Tracking: {tracking}") },
        "delivered" => "**Delivered.**".into(),
        "refunded" => "**Refunded.** The money is on its way back to you.".into(),
        "expired" => "This quote has expired. Ask here for a new one when you're ready.".into(),
        "pilot" => "**Test payment received.** Ordering through Backplane is a pilot: your card was not charged and nothing is ordered. To get the parts made now, order them from any fabricator with the files exported to `mfg/` here.".into(),
        _ => String::new(),
    }
}
/// Order.status.note: the alert it raises on the person's devices, "" for none (not Expired, not a pilot's: it alerts no one).
pub fn status_note(st: &str, tracking: &str) -> String {
    match st {
        "paid" => "Paid: your order is being placed.".into(), "placed" => "Ordered: your order is in production.".into(),
        "shipped" => if tracking.is_empty() { "Shipped.".into() } else { format!("Shipped. Tracking: {tracking}") },
        "delivered" => "Delivered.".into(), "refunded" => "Refunded: the money is on its way back to you.".into(), _ => String::new(),
    }
}
/// Order.said: what the agent hears back: never the token or the Buy now link.
pub fn said(q: &Value) -> Value {
    let failed = sv(q, "status") == "failed";
    let why = format!("The quote could not be worked out: {}. Nothing was posted with a Buy now link; say so in a line and offer to make a new quote.", sv(q, "error").trim_end_matches('.'));
    let note = if failed { why.as_str() } else if sv(q, "status") == "expired" { "The quote has expired (the server keeps a quote for a few days). Make a new quote if the person still wants it." } else if pending(q) { STATUS_SENT } else if payable(q) { "Backplane posted the quote with its Buy now link in this thread; do not repeat its table." }
        else { "Backplane posted the quote in this thread with NO Buy now link: it cannot be paid until its problems are fixed. Do not say a link is there. Tell the person each problem in plain words and what fixes it (for a destination that cannot be priced, ask for another country), then make a new quote." };
    let or = |v: &Value, d: Value| if v.is_null() { d } else { v.clone() };
    json!({"status":sv(q, "status"),"total":q["quote"]["total"],"lead_days":q["quote"]["lead_days"],"problems":or(&q["quote"]["problems"], json!([])),"warnings":or(&q["quote"]["warnings"], json!([])),"order":q["order"],"note":note})
}
/// Order.quote.md after Order.quote.customs: the order server's text, what the price covers, and Buy now only when it can be paid. The link names the quote, never its token.
pub fn quote_md(q: &Value) -> String {
    let md = sv(&q["quote"], "markdown");
    let md = if md.trim().is_empty() { md.to_string() } else { format!("{md}\n\n{}", customs(ddp(q))) };
    if payable(q) { let l = sv(&q["quote"], "pay_label"); format!("{md}\n\n[{}]({LINK}{})", if l.is_empty() { "Buy now" } else { l }, sv(q, "id")) } else { md }
}
pub fn total_of(label: &str, raw: &Value) -> String {
    if let Some(t) = label.strip_prefix("Buy now · ") { return t.to_string() }
    match raw { Value::Null => String::new(), Value::String(s) if s.is_empty() => String::new(), Value::String(s) => format!("${s}"), v => format!("${v}") }
}
/// Order.sum: what the hub keeps of a quote for the card (the setting `order.quote.<thread>`), without its link: `pay_link` finds that by the id.
pub fn sum(q: &Value) -> String {
    let label = sv(&q["quote"], "pay_label");
    json!({"id":sv(q, "id"),"state":state(q),"total":total_of(label, &q["quote"]["total"]),"ddp":ddp(q),"payable":payable(q),"label":if label.is_empty() { "Buy now" } else { label },"tracking":sv(&q["order"], "tracking")}).to_string()
}
/// Order.sum.keeps: a newer quote always takes the card; a quote's own progress only while it is the card's.
pub fn sum_keeps(fresh: bool, cur: &str, id: &str) -> bool { fresh || cur.is_empty() || serde_json::from_str::<Value>(cur).map_or(false, |v| sv(&v, "id") == id) }

// ---- the order card ----

pub const STEPS: [&str; 6] = ["Details", "Files", "Quote", "Pay", "Made", "Shipped"];
pub fn card_at(st: &str) -> usize { match st { "delivered" => 6, "shipped" => 5, "paid" | "placed" => 4, _ => 3 } }
pub fn card_said(st: &str, pay: bool, tracking: &str) -> String {
    match st {
        "expired" => "This quote expired. Ask here for a new one.".into(), "failed" => "The quote could not be worked out. Ask here to try again.".into(),
        "pilot" => "Test payment: nothing was charged or ordered.".into(), "refunded" => "Refunded: the money is on its way back to you.".into(),
        _ if card_at(st) >= 4 => status_note(st, tracking),
        _ => if pay { "Quoted. Buy now to place the order.".into() } else { "The quote has something to fix first: see it above.".into() },
    }
}
/// Order.card with Mob.order.card: a buy thread's card from its open asks, the quote on its way and the quote the hub keeps. The hub decides it all; every client draws it.
pub fn card(name: &str, asked: bool, files: bool, waiting: bool, running: bool, sum: &str) -> Value {
    let q: Value = serde_json::from_str(sum).unwrap_or(Value::Null);
    let (st, pay, tracking) = (sv(&q, "state"), q["payable"].as_bool().unwrap_or(false), sv(&q, "tracking"));
    let i = if files { 1 } else if waiting { 2 } else if !sum.is_empty() { card_at(st) } else { 0 };
    let line = if asked { "Backplane needs your answer below.".to_string() } else { match i {
        0 => if running { "Getting the order ready…".into() } else { "Reply below to go on.".into() }, 1 => "Check the files below and send them for a quote.".into(), 2 => "Getting your quote… usually 1-2 min".into(), _ => card_said(st, pay, tracking) } };
    let buys = i == 3 && pay && matches!(st, "ready" | "quoted"); let (shown, total) = (i >= 3, sv(&q, "total"));
    json!({"name":name,"steps":STEPS.iter().enumerate().map(|(k, l)| json!({"label":l,"mark":if k < i { "done" } else if k == i { "on" } else { "" }})).collect::<Vec<_>>(),
        "line":line,"waiting":i == 2,"total":if shown { total } else { "" },"customs":if shown && !total.is_empty() { customs(q["ddp"].as_bool().unwrap_or(false)) } else { "" },
        "pay":if buys { pay_link(sv(&q, "id")).unwrap_or_default() } else { String::new() },"label":if buys { let l = sv(&q, "label"); if l.is_empty() { "Buy now" } else { l } } else { "" }})
}

// ---- the link and the record ----

/// How the log writes a Buy now link: it names the quote, and the hub alone can turn that into the link with its token.
pub const LINK: &str = "order:";
pub static PAY: LazyLock<RwLock<HashMap<String, String>>> = LazyLock::new(Default::default);
pub fn pay_link(qid: &str) -> Option<String> { PAY.read().unwrap().get(qid).cloned() }
#[cfg(test)] pub fn test_link(qid: &str, link: &str) { PAY.write().unwrap().insert(qid.into(), link.into()); }
/// What a client is shown of a message: its Buy now link, real. Only the hub's drawing calls this; thread_read and the log keep `order:<quote>`.
pub fn pay_shown(text: &str) -> String {
    let needle = format!("]({LINK}");
    if !text.contains(&needle) { return text.to_string() }
    let (mut out, mut rest) = (String::new(), text);
    while let Some(i) = rest.find(&needle) {
        let from = i + needle.len(); let Some(end) = rest[from..].find(')') else { break }; let id = &rest[from..from + end];
        out.push_str(&rest[..i + 2]); match pay_link(id) { Some(l) => out.push_str(&l), None => { out.push_str(LINK); out.push_str(id) } } out.push(')'); rest = &rest[from + end + 1..];
    }
    out.push_str(rest); out
}
