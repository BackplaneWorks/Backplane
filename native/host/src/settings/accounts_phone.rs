//! The phones' account screens (data/1 `accounts.page`, `account.view`, `account.act`; docs/PHONE-DATA.md): the hub's accounts snapshot (accounts_io.rs) turned into the rows
//! and words a phone draws. The phone holds no rule: which state reads how, which buttons an account offers, what a sign-in step asks of the person, all come from here.
//! Every account the hub keeps is a named login of Claude or Codex; the other providers sign in on the hub's machine and have no accounts here.
use serde_json::{json, Value};

pub const PROVIDERS: &[&str] = &["claude", "codex"];
/// The groups of the page: Claude's CLI logins, Claude's sync tokens (accounts_sync.rs), Codex's. `claude-sync` is what a phone adds a sync account with.
pub const GROUPS: &[&str] = &["claude", "claude-sync", "codex"];
fn group_of(p: &Value) -> &'static str { if s(p, "kind") == super::accounts_io::SYNC { "claude-sync" } else if s(p, "provider") == "codex" { "codex" } else { "claude" } }
fn group_title(g: &str) -> &'static str { match g { "claude-sync" => "Claude (sync)", "codex" => "Codex", _ => "Claude" } }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
pub fn title(provider: &str) -> &'static str { super::accounts_hub::provider_title(provider) }

/// State.line: the word for a profile's state, and its tone (ok, warn, bad, dim).
pub fn state_line(state: &str) -> (&'static str, &'static str) {
    match state {
        "connected" => ("connected", "ok"),
        "expired" => ("sign-in expired", "bad"),
        "connecting" => ("signing in", "warn"),
        "unchecked" => ("not checked yet", "dim"),
        _ => ("not signed in", "dim"),
    }
}
/// Window.name: "5h", "7d", "7d opus", or the provider's own name.
fn window_name(w: &Value) -> String {
    match s(w, "scope") {
        "five_hour" => "5h".into(), "seven_day" => "7d".into(), "seven_day_opus" => "7d opus".into(), "seven_day_sonnet" => "7d sonnet".into(),
        _ => match w["window_minutes"].as_f64().map(|m| m as u64) { Some(300) => "5h".into(), Some(10080) => "7d".into(), Some(m) if m > 0 && m % 1440 == 0 => format!("{}d", m / 1440), Some(m) if m > 0 && m % 60 == 0 => format!("{}h", m / 60), _ => s(w, "name").to_string() },
    }
}
/// Allowance.lines: one line a window ("5h  42% used, resets 15:40"), or why there are none.
pub fn limit_lines(p: &Value, zone: &str, now: u64) -> Vec<String> {
    let a = &p["allowance"];
    if a.is_null() { return vec![] }
    let ws = a["windows"].as_array().cloned().unwrap_or_default();
    // a reading's refusal says something only of a working login: an expired or signing-in one has its state line
    if ws.is_empty() { let r = s(a, "reason"); return if r.is_empty() || s(p, "state") != "connected" { vec![] } else { vec![r.to_string()] } }
    let old = if s(a, "state") == "stale" { " (last reading)" } else { "" };
    ws.iter().filter_map(|w| {
        let used = w["used_percent"].as_f64()?;
        let reset = w["resets_at"].as_f64().map(|r| r as u64).filter(|r| *r > now).map(|r| format!(", resets {}", crate::agents::rules::when(zone, r, now))).unwrap_or_default();
        Some(format!("{}  {}% used{reset}{old}", window_name(w), used.round() as u64))
    }).collect()
}
/// One account in the list.
pub fn row(p: &Value, holds: &Value, zone: &str, now: u64) -> Value {
    let id = s(p, "id"); let (line, tone) = state_line(s(p, "state"));
    let line = if s(p, "kind") == super::accounts_io::SYNC && s(p, "state") == "connected" { "connected, sync token" } else { line };
    json!({"id":id,"provider":s(p, "provider"),"label":s(p, "label"),"email":s(&p["identity"], "email"),"state":s(p, "state"),"line":line,"tone":tone,
        "limits":limit_lines(p, zone, now).join(" · "),"hold":s(&holds[id], "text")})
}
/// Page: every provider with accounts here, in order, with its accounts as they were saved.
pub fn page(snap: &Value, zone: &str, now: u64) -> Value {
    let profiles = snap["profiles"].as_array().cloned().unwrap_or_default();
    let providers: Vec<Value> = GROUPS.iter().map(|g| json!({"provider":g,"title":group_title(g),"add":if *g == "claude-sync" { "Add a Claude sync token".to_string() } else { format!("Add a {} account", group_title(g)) },
        "accounts":profiles.iter().filter(|p| group_of(p) == *g).map(|p| row(p, &snap["holds"], zone, now)).collect::<Vec<_>>()})).collect();
    json!({"providers":providers,"error":s(snap, "error"),"note":"Accounts sign in on the hub. Other agents use the hub machine's own sign-in."})
}
/// The newest sign-in attempt of a profile.
fn attempt<'a>(snap: &'a Value, id: &str) -> Option<&'a Value> { snap["attempts"].as_array()?.iter().filter(|a| s(a, "profile") == id).last() }
/// Attempt.step: what a sign-in in progress (or just over) shows and asks. `paste`: Claude's sign-in page shows a code the person pastes back here; Codex's device sign-in
/// takes the code shown here on its own page and the hub hears when it is done. `busy`: the phone asks again until it is not.
pub fn signin(a: &Value, provider: &str) -> Value {
    let st = s(a, "state"); let claude = provider != "codex";
    let text = match st {
        "starting" => "Starting the sign-in on the hub.".to_string(),
        "waiting" if claude => "Open the link and sign in. The page then shows a code: copy it and paste it here.".to_string(),
        "waiting" => "Open the link, sign in, and enter this code there.".to_string(),
        "done" => "Signed in.".to_string(),
        "expired" => "The sign-in took too long. Start it again.".to_string(),
        "cancelled" => "Sign-in cancelled.".to_string(),
        _ => { let m = s(a, "message"); if m.is_empty() { "The sign-in did not finish. Start it again.".to_string() } else { m.to_string() } }
    };
    let waiting = st == "waiting";
    json!({"attempt":s(a, "id"),"state":st,"url":if waiting { s(a, "url") } else { "" },"code":if waiting && !claude { s(a, "userCode") } else { "" },"paste":waiting && claude,
        "text":text,"error":if waiting { s(a, "message") } else { "" },"busy":matches!(st, "starting" | "waiting"),"done":st == "done"})
}
/// View: one account, its sign-in if one was started, and the buttons it offers now.
pub fn view(snap: &Value, id: &str, zone: &str, now: u64) -> Option<Value> {
    let p = snap["profiles"].as_array()?.iter().find(|p| s(p, "id") == id)?;
    let (state, provider) = (s(p, "state"), s(p, "provider"));
    let (line, tone) = state_line(state);
    let sg = attempt(snap, id).map(|a| signin(a, provider));
    let busy = sg.as_ref().map_or(false, |x| x["busy"] == true);
    let act = |label: &str, action: &str, danger: bool| json!({"label":label,"action":action,"danger":danger});
    let actions: Vec<Value> = if busy { vec![act("Cancel sign-in", "cancel", false)] } else {
        let sync = s(p, "kind") == super::accounts_io::SYNC;
        let mut v = vec![if sync { act(if state == "disconnected" { "Make the sync token" } else { "Renew the sync token" }, "signin", false) } else { match state { "connected" => act("Sign in again", "signin", false), "expired" => act("Sign in again", "signin", false), _ => act("Sign in", "signin", false) } }];
        if matches!(state, "connected" | "expired" | "unchecked") { v.push(act("Check now", "check", false)) }
        v.push(act("Remove", "remove", true)); v
    };
    let msg = s(p, "message");
    let ends = p["tokenExpiresAt"].as_u64().unwrap_or(0);
    let mut limits = limit_lines(p, zone, now);
    if s(p, "kind") == super::accounts_io::SYNC && ends > 0 { limits.insert(0, if ends > now { format!("Sync token ends {}", crate::agents::rules::when(zone, ends, now)) } else { "Sync token ended".to_string() }) }
    Some(json!({"id":id,"provider":provider,"title":if s(p, "kind") == super::accounts_io::SYNC { "Claude (sync)" } else { title(provider) },"label":s(p, "label"),"email":s(&p["identity"], "email"),"plan":s(&p["identity"], "plan"),"state":state,"line":line,"tone":tone,
        "message":if msg == super::accounts_io::EXPIRED_SHORT { "" } else { msg },"limits":limits,"hold":s(&snap["holds"][id], "text"),"signin":sg,"actions":actions,
        "remove":format!("Remove {}? Threads on it stop and must choose another account.", s(p, "label"))}))
}
/// The label a new sync account gets when the person gives none: "Claude sync 1" for the first.
pub fn new_sync_label(snap: &Value, given: &str) -> String {
    let g = super::accounts_io::label(given); if !g.is_empty() { return g }
    let n = snap["profiles"].as_array().map_or(0, |a| a.iter().filter(|p| s(p, "kind") == super::accounts_io::SYNC).count());
    format!("Claude sync {}", n + 1)
}
/// The label a new account gets when the person gives none: "Claude 2" for the second.
pub fn new_label(snap: &Value, provider: &str, given: &str) -> String {
    let g = super::accounts_io::label(given); if !g.is_empty() { return g }
    let n = snap["profiles"].as_array().map_or(0, |a| a.iter().filter(|p| s(p, "provider") == provider).count());
    format!("{} {}", title(provider), n + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snap() -> Value {
        json!({"profiles":[
            {"id":"a","provider":"claude","label":"Work","state":"expired","identity":{"email":"w@x.com","plan":"max"},"message":"Sign-in expired. Sign in again.","allowance":{"state":"ok","windows":[{"scope":"five_hour","used_percent":42.4,"resets_at":2000},{"scope":"seven_day","used_percent":10.0}]}},
            {"id":"b","provider":"codex","label":"Home","state":"connected","identity":{"email":"h@x.com"},"message":"","allowance":{"state":"ok","windows":[{"name":"codex/primary","window_minutes":300.0,"used_percent":5.0}]}},
            {"id":"c","provider":"claude","label":"Spare","state":"disconnected","identity":{},"message":""}],
            "attempts":[{"id":"t1","profile":"b","state":"failed","message":"old"},{"id":"t2","profile":"b","state":"waiting","url":"https://auth.openai.com/codex/device","userCode":"ABCD-1234","message":""}],
            "holds":{"a":{"until":0,"text":"limited"}},"error":""})
    }
    #[test] fn every_state_reads_one_way() {
        for (st, want) in [("connected", ("connected", "ok")), ("expired", ("sign-in expired", "bad")), ("connecting", ("signing in", "warn")), ("unchecked", ("not checked yet", "dim")), ("disconnected", ("not signed in", "dim")), ("", ("not signed in", "dim"))] { assert_eq!(state_line(st), want, "{st}") }
    }
    #[test] fn the_page_lists_each_provider_with_its_accounts_state_and_limits() {
        let p = page(&snap(), "UTC", 1000);
        let pv: Vec<&str> = p["providers"].as_array().unwrap().iter().map(|x| x["provider"].as_str().unwrap()).collect(); assert_eq!(pv, ["claude", "claude-sync", "codex"]);
        let claude = &p["providers"][0]["accounts"]; assert_eq!(claude.as_array().unwrap().len(), 2);
        assert_eq!((claude[0]["line"].as_str().unwrap(), claude[0]["tone"].as_str().unwrap(), claude[0]["hold"].as_str().unwrap()), ("sign-in expired", "bad", "limited"));
        assert!(claude[0]["limits"].as_str().unwrap().starts_with("5h  42% used, resets ") && claude[0]["limits"].as_str().unwrap().ends_with(" · 7d  10% used"), "{}", claude[0]["limits"]);
        assert_eq!(p["providers"][2]["accounts"][0]["limits"], "5h  5% used", "a Codex window by its length");
        assert_eq!(p["providers"][0]["add"], "Add a Claude account");
    }
    #[test] fn an_expired_account_offers_to_sign_in_again_and_says_nothing_twice() {
        let v = view(&snap(), "a", "UTC", 1000).unwrap();
        assert_eq!(v["actions"][0], json!({"label":"Sign in again","action":"signin","danger":false})); assert_eq!(v["actions"].as_array().unwrap().last().unwrap()["action"], "remove");
        assert_eq!(v["message"], "", "the state line already says it"); assert!(v["signin"].is_null());
        assert_eq!(view(&snap(), "c", "UTC", 1000).unwrap()["actions"][0]["label"], "Sign in"); assert!(view(&snap(), "zz", "UTC", 1000).is_none());
    }
    #[test] fn a_codex_device_sign_in_shows_its_link_and_code_and_only_cancel() {
        let v = view(&snap(), "b", "UTC", 1000).unwrap(); let sg = &v["signin"];
        assert_eq!((sg["attempt"].as_str().unwrap(), sg["url"].as_str().unwrap(), sg["code"].as_str().unwrap(), sg["paste"].as_bool().unwrap(), sg["busy"].as_bool().unwrap()), ("t2", "https://auth.openai.com/codex/device", "ABCD-1234", false, true), "the newest attempt");
        assert_eq!(v["actions"], json!([{"label":"Cancel sign-in","action":"cancel","danger":false}]));
    }
    #[test] fn a_claude_sign_in_asks_for_the_code_back_and_ends_in_words() {
        let w = signin(&json!({"id":"t","state":"waiting","url":"https://claude.com/cai/oauth/authorize?code=true&state=S","userCode":"","message":"That code belongs to a different sign-in attempt."}), "claude");
        assert_eq!((w["paste"].as_bool().unwrap(), w["code"].as_str().unwrap(), w["error"].as_str().unwrap()), (true, "", "That code belongs to a different sign-in attempt."));
        let d = signin(&json!({"id":"t","state":"done","url":"https://claude.com/x"}), "claude"); assert_eq!((d["busy"].as_bool().unwrap(), d["done"].as_bool().unwrap(), d["url"].as_str().unwrap()), (false, true, ""), "no link once it is over");
        assert_eq!(signin(&json!({"state":"failed","message":"Claude could not reach its sign-in service."}), "claude")["text"], "Claude could not reach its sign-in service.");
        assert_eq!(signin(&json!({"state":"expired"}), "claude")["text"], "The sign-in took too long. Start it again.");
    }
    #[test] fn an_expired_account_does_not_repeat_its_refusal_as_its_limits() {
        let p = json!({"state":"expired","allowance":{"state":"unavailable","windows":[],"reason":"Claude refused the request: OAuth refresh token is no longer valid"}});
        assert!(limit_lines(&p, "UTC", 1).is_empty()); assert_eq!(limit_lines(&json!({"state":"connected","allowance":p["allowance"]}), "UTC", 1).len(), 1);
    }
    #[test] fn a_new_account_is_named_for_the_person_when_they_give_no_name() { assert_eq!(new_label(&snap(), "claude", "  "), "Claude 3"); assert_eq!(new_label(&snap(), "codex", "Laptop"), "Laptop"); }
}
