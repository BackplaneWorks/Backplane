//! Settings → Ring (pure; docs/ring.md): the rows every client draws, the desktop through the presenter (`ring.setup` and `ring.notes` as info) and the phones
//! through `ring.page`, from the same values. `setup`: the ring's bot and webhook as the hub has them (`bot`, `hook`, `url`, `token`, `test`), and `home` when
//! another hub owns the ring (the setting `ring.home`, `{url, machine}`, written at a cutover). `notes`: `{q, total, found, notes}`. Buttons: `ring-op`
//! (setup, hear:<id>:<engine>, send:<id>) and `ring-search` go to the hub; `copy`, `share-text`, `open-url` and `link-open` are the client's own.
use serde_json::{json, Value};

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn btn(label: &str, action: &str, value: &str, on: bool) -> Value { json!({"label": label, "action": action, "value": value, "on": on}) }
fn row(label: &str, note: &str, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "buttons": buttons}) }

/// Where the ring's webhook lives, as the Pebble app needs it: (the URL, the machine that owns it when it is another, the token).
pub fn target(setup: &Value) -> (String, String, String) {
    let home = &setup["home"];
    if !s(home, "url").is_empty() { return (s(home, "url").to_string(), s(home, "machine").to_string(), s(setup, "token").to_string()) }
    (s(setup, "url").to_string(), String::new(), s(setup, "token").to_string())
}
/// The two things the Pebble app's webhook needs, in one text to copy or share.
pub fn both(url: &str, token: &str) -> String { if token.is_empty() { format!("URL: {url}") } else { format!("URL: {url}\nHeader: Authorization\nValue: Bearer {token}") } }

/// The page. `q`: the search field as typed; `when`: a time as the person reads it; `phone`: the phones also get Share.
pub fn rows(setup: &Value, notes: &Value, busy: &str, q: &str, when: &dyn Fn(u64) -> String, phone: bool) -> Vec<Value> {
    let mut v = vec![];
    let (url, elsewhere, tok) = target(setup);
    if !elsewhere.is_empty() || !s(&setup["home"], "url").is_empty() {
        let m = if elsewhere.is_empty() { "another hub".to_string() } else { elsewhere.clone() };
        let mut b = vec![btn("Copy URL", "copy", &url, false)];
        if !tok.is_empty() { b.push(btn("Copy both", "copy", &both(&url, &tok), true)); if phone { b.push(btn("Share", "share-text", &both(&url, &tok), false)) } }
        v.push(row("Pebble Index ring", &format!("Ring is on {m}. Its recordings, Pebble and the test status are there. In the Pebble app the webhook's URL is {url}; the Authorization header stays as it was."), b));
        return v
    }
    if s(setup, "hook").is_empty() {
        v.push(row("Pebble Index ring", "Not set up. Set up makes the Pebble bot (it sorts each note into a request, a reminder, a calendar ask, an errand, a todo or a note), its webhook and the #ring-requests room.", vec![btn("Set up", "ring-op", "setup", true)]));
    } else {
        let https = if url.starts_with("https://") { "" } else { " This address is plain HTTP: the Pebble app wants HTTPS (share the hub with tailscale serve, docs/ring.md)." };
        v.push(row("Pebble Index ring", &format!("In the Pebble app: Index 01 Settings › Webhook › Hold & talk (and Double click & hold if you like). URL: {url}. Header: Authorization, with the value Copy header gives. What to send: Both. Save, then Send test event. The phone must be on your tailnet.{https}"),
            { let mut b = vec![btn("Copy URL", "copy", &url, false), btn("Copy header", "copy", &format!("Bearer {tok}"), false), btn("Copy both", "copy", &both(&url, &tok), true)];
              if phone { b.push(btn("Share", "share-text", &both(&url, &tok), false)) } b.push(btn("Set up again", "ring-op", "setup", false)); b }));
        let test = setup["test"].as_u64().unwrap_or(0);
        let mut t = row("Test event", &if test == 0 { "None yet. In the Pebble app, Send test event: it shows here when it arrives.".to_string() } else { format!("Arrived {}. Backplane hears this webhook.", when(test)) }, vec![btn("Check again", "ring-search", "keep", false)]);
        if test > 0 { t["tone"] = json!("ok") }
        v.push(t);
    }
    let mut sb = vec![btn("Search", "ring-search", "", true)]; if !s(notes, "q").is_empty() { sb.push(btn("Show all", "ring-search", "clear", false)) }
    let total = notes["total"].as_u64().unwrap_or(0);
    let count = if total == 0 { "No recordings yet.".to_string() } else if s(notes, "q").is_empty() { format!("{total} recording{}; the newest 40 are below.", if total == 1 { "" } else { "s" }) } else { format!("{} of {total} have “{}”.", notes["found"].as_u64().unwrap_or(0), s(notes, "q")) };
    v.push(json!({"label": "Recordings", "note": count, "fields": [{"name": "ringq", "hint": "Search what you said", "secret": false, "value": q}], "buttons": sb}));
    for n in notes["notes"].as_array().into_iter().flatten() {
        let id = s(n, "id"); let g = s(n, "gesture"); let said = when(n["said"].as_u64().unwrap_or(0));
        let label = if g.is_empty() { said } else { format!("{said} · {g}") };
        let heard = match (s(n, "model"), n["conf"].as_i64()) { ("", _) => "Not heard".to_string(), ("ring", _) => "The Pebble app's words".to_string(), (m, Some(c)) => format!("Heard by {m}, {c}% sure"), (m, None) => format!("Heard by {m}") };
        let state = match s(n, "state") { "sent" => "given to Pebble", "new" => "waiting to be heard", "heard" => "waiting to go to Pebble", "failed" => "not given to Pebble", x => x };
        let state = if busy == id { "being heard now" } else { state };
        let mut note = format!("{}\n{heard} · {state}", if s(n, "words").is_empty() { "(no words)" } else { s(n, "words") });
        if !s(n, "unsure").is_empty() { note.push_str(&format!("\n{}", s(n, "unsure"))) }
        if !s(n, "error").is_empty() { note.push_str(&format!("\n{}", s(n, "error"))) }
        let mut b = vec![];
        if !s(n, "play").is_empty() { b.push(btn("Play", "open-url", s(n, "play"), false)); b.push(btn("Hear again (local)", "ring-op", &format!("hear:{id}:local"), false)); b.push(btn("Hear again (OpenAI)", "ring-op", &format!("hear:{id}:openai"), false)); }
        if !s(n, "thread").is_empty() { b.push(btn("What happened", "link-open", &format!("backplane://thread/{}", s(n, "thread")), false)) }
        if s(n, "state") != "sent" || n["heard"].as_u64().unwrap_or(0) > 1 { b.push(btn("Send again", "ring-op", &format!("send:{id}"), false)) }
        let mut r = row(&label, &note, b); if s(n, "state") == "failed" { r["tone"] = json!("bad") } v.push(r);
    }
    v
}
/// What a hub does for a press (`ring.page.act` and the desktop's): the request and its params, or None for one that is not the hub's.
pub fn request(action: &str, value: &str, typed_q: &str) -> Option<(&'static str, Value)> {
    match action {
        "ring-op" => { let (op, rest) = value.split_once(':').unwrap_or((value, ""));
            match op { "setup" => Some(("ring.setup", json!({}))),
                "hear" => { let (id, engine) = rest.split_once(':').unwrap_or((rest, "")); Some(("ring.hear", json!({"note": id, "engine": engine}))) }
                "send" => Some(("ring.send", json!({"note": rest}))),
                _ => None } }
        // "keep": the list again as it is (a search with no q keeps the one there is)
        "ring-search" => Some(("ring.search", match value { "keep" => json!({}), "clear" => json!({"q": ""}), _ => json!({"q": typed_q.trim()}) })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn when(t: u64) -> String { format!("@{t}") }
    fn acts(r: &Value) -> Vec<(String, String)> { r["buttons"].as_array().unwrap().iter().map(|b| (b["action"].as_str().unwrap().to_string(), b["value"].as_str().unwrap().to_string())).collect() }
    fn note(id: &str, state: &str, play: &str, thread: &str) -> Value { json!({"id": id, "said": 5, "gesture": "Hold & talk", "words": "Call Jeff", "model": "parakeet-v2", "conf": 91, "unsure": "", "state": state, "error": "", "thread": thread, "play": play, "heard": 1}) }
    #[test] fn not_set_up_offers_set_up() {
        let r = rows(&json!({}), &json!({}), "", "", &when, true);
        assert_eq!(acts(&r[0]), [("ring-op".to_string(), "setup".to_string())]); assert_eq!(r[1]["note"], "No recordings yet.");
    }
    #[test] fn set_up_copies_the_url_and_header_and_says_when_the_test_came() {
        let setup = json!({"bot": "b1", "hook": "h1", "url": "https://desk.ts.net:8451/hook/h1", "token": "t0k"});
        let r = rows(&setup, &json!({}), "", "", &when, true); let a = acts(&r[0]);
        assert_eq!(a[..3], [("copy".to_string(), "https://desk.ts.net:8451/hook/h1".to_string()), ("copy".to_string(), "Bearer t0k".to_string()), ("copy".to_string(), "URL: https://desk.ts.net:8451/hook/h1\nHeader: Authorization\nValue: Bearer t0k".to_string())]);
        assert_eq!(a[3].0, "share-text"); assert_eq!(a[4], ("ring-op".to_string(), "setup".to_string()));
        assert!(!acts(&rows(&setup, &json!({}), "", "", &when, false)[0]).iter().any(|x| x.0 == "share-text"), "the desktop has no share sheet");
        assert!(r[1]["note"].as_str().unwrap().starts_with("None yet")); let mut t = setup.clone(); t["test"] = json!(77);
        assert_eq!(rows(&t, &json!({}), "", "", &when, true)[1]["note"], "Arrived @77. Backplane hears this webhook.");
        let mut h = setup.clone(); h["url"] = json!("http://127.0.0.1:3797/hook/h1"); assert!(rows(&h, &json!({}), "", "", &when, true)[0]["note"].as_str().unwrap().contains("plain HTTP"));
    }
    #[test] fn a_ring_owned_by_another_hub_shows_that_hubs_url_and_nothing_to_set_up() {
        let setup = json!({"bot": "b1", "hook": "", "url": "https://desk.ts.net:8451/hook/h1", "token": "t0k", "home": {"url": "https://boat-hub.tail9e0cf2.ts.net/hook/h1", "machine": "boat-hub"}});
        assert_eq!(target(&setup), ("https://boat-hub.tail9e0cf2.ts.net/hook/h1".to_string(), "boat-hub".to_string(), "t0k".to_string()));
        let r = rows(&setup, &json!({"total": 3, "notes": [note("n1", "sent", "x", "t")]}), "", "", &when, true);
        assert_eq!(r.len(), 1, "no test status, no list: they are on the owning hub"); assert!(r[0]["note"].as_str().unwrap().starts_with("Ring is on boat-hub."));
        let a = acts(&r[0]); assert_eq!(a[0], ("copy".to_string(), "https://boat-hub.tail9e0cf2.ts.net/hook/h1".to_string())); assert!(a[1].1.contains("boat-hub.tail9e0cf2.ts.net") && a[1].1.ends_with("Bearer t0k"));
        assert!(!a.iter().any(|x| x.1 == "setup"));
        assert_eq!(target(&json!({"url": "u", "token": "t"})), ("u".to_string(), String::new(), "t".to_string()));
    }
    #[test] fn notes_play_hear_again_and_open_where_they_went() {
        let notes = json!({"q": "", "total": 2, "found": 2, "notes": [note("n1", "sent", "https://h/ring/audio/n1?k=c", "t9"), note("n2", "failed", "", "")]});
        let r = rows(&json!({}), &notes, "n1", "jeff", &when, true);
        assert_eq!(r[1]["fields"][0]["value"], "jeff"); assert_eq!(r[2]["label"], "@5 · Hold & talk"); assert_eq!(r[2]["note"], "Call Jeff\nHeard by parakeet-v2, 91% sure · being heard now");
        assert_eq!(acts(&r[2]), [("open-url".to_string(), "https://h/ring/audio/n1?k=c".to_string()), ("ring-op".to_string(), "hear:n1:local".to_string()), ("ring-op".to_string(), "hear:n1:openai".to_string()), ("link-open".to_string(), "backplane://thread/t9".to_string())]);
        assert_eq!(acts(&r[3]), [("ring-op".to_string(), "send:n2".to_string())]); assert_eq!(r[3]["tone"], "bad");
    }
    #[test] fn presses_the_hub_does() {
        assert_eq!(request("ring-op", "hear:n1:openai", ""), Some(("ring.hear", json!({"note": "n1", "engine": "openai"}))));
        assert_eq!(request("ring-op", "setup", ""), Some(("ring.setup", json!({})))); assert_eq!(request("ring-op", "send:n2", "").unwrap().1, json!({"note": "n2"}));
        assert_eq!(request("ring-search", "", " jeff ").unwrap().1, json!({"q": "jeff"})); assert_eq!(request("ring-search", "clear", "x").unwrap().1, json!({"q": ""})); assert_eq!(request("ring-search", "keep", "x").unwrap().1, json!({}));
        assert!(request("ring-op", "rm:n1", "").is_none()); assert!(request("copy", "x", "").is_none());
    }
}
