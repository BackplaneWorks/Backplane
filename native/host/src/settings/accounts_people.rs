//! Accounts by person (the `accounts` answer's `people`, the phones' `accounts.page` `people`): one entry per account, titled by its email, holding each login of it
//! (the provider CLI's own login, an added account, a sync token; Claude and Codex) with its state, its holds and a bar per limit window. The same email twice is
//! one entry; the CLI's login and an added account of the same identity (`sameAsDefault`) are one login. The desktop's Settings › Accounts and the phones'
//! Accounts & Usage draw these entries as they are: the hub decides who is who, what is held and each bar's share and tone (accounts_phone.rs `bars`).
use serde_json::{json, Value};
use super::accounts_phone::{bars, limit_lines, state_line, PROVIDERS};

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn provider_title(provider: &str, sync: bool) -> &'static str { if sync { "Claude (sync)" } else if provider == "codex" { "Codex" } else { "Claude" } }
/// Who a profile is: its verified email, else its label when that is an email (an account still signing in), else nobody yet.
fn email_of(p: &Value) -> String {
    let e = s(&p["identity"], "email"); if !e.is_empty() { return e.to_string() }
    let l = s(p, "label"); if l.contains('@') && !l.contains(' ') { l.to_string() } else { String::new() }
}
/// The default for new threads of a provider: `(profile id, epoch)`, ("", 0) for the CLI's own login.
fn default_of(snap: &Value, provider: &str) -> (String, u64) {
    let d: Value = serde_json::from_str(s(&snap["defaults"], provider)).unwrap_or(Value::Null);
    (s(&d, "id").to_string(), d["epoch"].as_u64().unwrap_or(0))
}
fn windows(al: &Value) -> usize { al["windows"].as_array().map_or(0, |w| w.iter().filter(|w| w["used_percent"].as_f64().is_some() || w["used_permille"].as_f64().is_some()).count()) }

/// People: the entries in order (the CLI's own logins first, then the added accounts as they were saved), each `{key, title, logins:[…]}`.
/// A login: `{provider, title, kind: cli|account|sync, profile, epoch, plan, state, line, tone, hold, out, default, bars, note, also}`; `profile` "" is the CLI's own
/// login with no added account of it (no account page, no sign-in buttons), `also` the labels of added accounts folded into it.
pub fn people(snap: &Value, zone: &str, now: u64) -> Vec<Value> {
    let holds = &snap["holds"];
    let mut entries: Vec<(String, String, Vec<Value>)> = vec![];
    let mut put = |key: String, title: String, login: Value| {
        let i = match entries.iter().position(|e| e.0 == key) { Some(i) => i, None => { entries.push((key, title, vec![])); entries.len() - 1 } };
        let logins = &mut entries[i].2;
        // one login per provider and kind: the same account added twice is counted once, the one with a reading kept
        if let Some(j) = logins.iter().position(|l| l["provider"] == login["provider"] && l["kind"] == login["kind"]) {
            let keep_new = login["bars"].as_array().map_or(0, Vec::len) > logins[j]["bars"].as_array().map_or(0, Vec::len)
                || (login["state"] == "connected" && logins[j]["state"] != "connected");
            let mut also: Vec<Value> = logins[j]["also"].as_array().cloned().unwrap_or_default();
            let (gone, mut kept) = if keep_new { (logins[j].clone(), login) } else { (login, logins[j].clone()) };
            let name = s(&gone, "label").to_string(); if !name.is_empty() && !also.contains(&json!(name)) { also.push(json!(name)) }
            kept["default"] = json!(kept["default"] == true || gone["default"] == true);
            if kept["hold"] == "" && gone["hold"] != "" { kept["hold"] = gone["hold"].clone(); kept["out"] = json!(true) }
            kept["also"] = json!(also); logins[j] = kept;
        } else { logins.push(login) }
    };
    let profiles = snap["profiles"].as_array().cloned().unwrap_or_default();
    // the CLI's own logins, with the added account of the same identity folded in (its id carries the buttons, its reading counts when the CLI's has none)
    for pv in PROVIDERS {
        let d = &snap["default_login"][*pv];
        if d.is_null() { continue }
        let twin = profiles.iter().find(|p| s(p, "provider") == *pv && p["sameAsDefault"] == true);
        let own = if *pv == "claude" { &snap["default_allowance"] } else { &snap["default_allowance_codex"] };
        let al = match twin { Some(t) if windows(&t["allowance"]) > windows(own) => &t["allowance"], _ => own };
        let email = s(d, "email").to_string();
        let (state, line, tone) = match s(d, "state") { "known" => ("connected", "connected", "ok"), "signed-out" => ("disconnected", "signed out", "dim"), _ => ("unchecked", s(d, "reason"), "dim") };
        if email.is_empty() && twin.is_none() && windows(al) == 0 && s(d, "state") != "known" { continue }
        let tid = twin.map(|t| s(t, "id")).unwrap_or("");
        // a hold on the login (Claude's CLI login is "") or on the added account of it: one account, out once
        let keys: Vec<&str> = if *pv == "claude" { vec!["", tid] } else { vec![tid] };
        let hold = keys.iter().filter(|k| !k.is_empty() || *pv == "claude").find_map(|k| holds[*k]["text"].as_str()).unwrap_or("").to_string();
        let (did, de) = default_of(snap, pv);
        let default = did.is_empty() || (!tid.is_empty() && did == tid && twin.map_or(false, |t| t["epoch"].as_u64().unwrap_or(0) == de));
        let note = if windows(al) > 0 { String::new() } else { let l = limit_lines(&json!({"allowance":al,"state":state}), zone, now); l.first().cloned().unwrap_or_else(|| if state == "connected" { "no usage reading yet".into() } else { String::new() }) };
        let also: Vec<Value> = twin.map(|t| s(t, "label")).filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case(&email)).map(|l| vec![json!(l)]).unwrap_or_default();
        let login = json!({"provider":pv,"title":provider_title(pv, false),"kind":"cli","profile":tid,"epoch":twin.map_or(0, |t| t["epoch"].as_u64().unwrap_or(0)),"plan":s(d, "plan"),
            "state":state,"line":line,"tone":tone,"hold":hold,"out":!hold.is_empty(),"default":default,"bars":bars(al, !hold.is_empty(), zone, now),"note":note,"also":also,"label":""});
        let key = if email.is_empty() { format!("cli:{pv}") } else { email.to_lowercase() };
        put(key, if email.is_empty() { format!("{} CLI login", provider_title(pv, false)) } else { email }, login);
    }
    for p in profiles.iter().filter(|p| p["sameAsDefault"] != true) {
        let (id, pv, state) = (s(p, "id"), s(p, "provider"), s(p, "state"));
        let sync = s(p, "kind") == super::accounts_io::SYNC;
        let (line, tone) = state_line(state);
        let hold = s(&holds[id], "text").to_string();
        let (did, de) = default_of(snap, pv);
        let al = &p["allowance"];
        let note = if windows(al) > 0 { String::new() } else { limit_lines(p, zone, now).first().cloned().unwrap_or_else(|| if state == "connected" { "no usage reading yet".into() } else { String::new() }) };
        let login = json!({"provider":pv,"title":provider_title(pv, sync),"kind":if sync { "sync" } else { "account" },"profile":id,"epoch":p["epoch"].as_u64().unwrap_or(0),"plan":s(&p["identity"], "plan"),
            "state":state,"line":line,"tone":tone,"hold":hold,"out":!hold.is_empty(),"default":did == id && de == p["epoch"].as_u64().unwrap_or(0),"bars":bars(al, !hold.is_empty(), zone, now),"note":note,"also":[],"label":s(p, "label")});
        let email = email_of(p);
        let (key, title) = if email.is_empty() { (format!("id:{id}"), s(p, "label").to_string()) } else { (email.to_lowercase(), email) };
        put(key, title, login);
    }
    entries.into_iter().map(|(key, title, mut logins)| {
        // Claude before Codex, the CLI's login before an added one, a sync token last
        let rank = |l: &Value| (if l["provider"] == "codex" { 1 } else { 0 }, match s(l, "kind") { "cli" => 0, "account" => 1, _ => 2 });
        logins.sort_by_key(|l| rank(l));
        json!({"key":key,"title":title,"logins":logins})
    }).chain(super::provider_usage::people(&snap["others"], zone, now)).collect() // then each provider without accounts, with its own reading or what was seen (#1337)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The live hub's accounts on 2026-11-15, as `accounts` list answers them (emails as they were): the Claude CLI login is person@example.com and an added
    /// account is that same login; the Codex CLI login is alex@example.net, who also has an added Claude account still signing in and held.
    fn live() -> Value {
        json!({"profiles":[
            {"id":"k","provider":"claude","label":"alex@example.net","state":"connecting","epoch":2,"identity":{},"message":""},
            {"id":"h","provider":"claude","label":"sam@example.org","state":"connected","epoch":1,"identity":{"email":"sam@example.org","plan":"max"},
             "allowance":{"state":"ok","windows":[{"scope":"five_hour","label":"5-hour","used_percent":0.0},{"scope":"seven_day","label":"Weekly","used_percent":80.0,"resets_at":5000},{"scope":"seven_day_fable","label":"Fable weekly","used_percent":37.0,"resets_at":5000}]}},
            {"id":"b","provider":"claude","label":"person@example.com","state":"connected","epoch":3,"sameAsDefault":true,"identity":{"email":"person@example.com","plan":"max"},
             "allowance":{"state":"ok","windows":[{"scope":"seven_day","label":"Weekly","used_percent":95.0,"resets_at":9000},{"scope":"seven_day_fable","label":"Fable weekly","used_percent":100.0,"resets_at":9000}]}}],
            "default_login":{"claude":{"state":"known","email":"person@example.com","plan":"max"},"codex":{"state":"known","email":"alex@example.net","plan":"pro"}},
            "default_allowance":{"state":"ok","windows":[{"scope":"shared","used_percent":null,"resets_at":9000}]},
            "defaults":{"claude":"{\"id\":\"k\",\"epoch\":2}","codex":""},
            "holds":{"k":{"until":4000,"text":"limited until Nov 18 17:00"}}})
    }
    #[test] fn the_live_hub_is_three_people() {
        let p = people(&live(), "", 1000);
        assert_eq!(p.iter().map(|e| e["title"].as_str().unwrap()).collect::<Vec<_>>(), ["person@example.com", "alex@example.net", "sam@example.org"]);
        // the CLI login and the added account of it: one login, the added one's reading and id (for its buttons)
        let b = &p[0]["logins"]; assert_eq!(b.as_array().unwrap().len(), 1);
        assert_eq!((b[0]["kind"].as_str().unwrap(), b[0]["profile"].as_str().unwrap(), b[0]["bars"].as_array().unwrap().len(), b[0]["default"].as_bool().unwrap()), ("cli", "b", 2, false));
        assert_eq!((b[0]["bars"][0]["name"].as_str().unwrap(), b[0]["bars"][0]["tone"].as_str().unwrap(), b[0]["bars"][1]["tone"].as_str().unwrap()), ("Weekly", "bad", "bad"));
        assert_eq!(b[0]["bars"][0]["resets_at"], 9000);
        // alex: the Claude account (held, signing in, used for new Claude threads) and the Codex CLI login (used for new Codex threads, no reading yet)
        let k = p[1]["logins"].as_array().unwrap();
        assert_eq!(k.iter().map(|l| (l["provider"].as_str().unwrap(), l["kind"].as_str().unwrap())).collect::<Vec<_>>(), [("claude", "account"), ("codex", "cli")]);
        assert_eq!((k[0]["out"].as_bool().unwrap(), k[0]["hold"].as_str().unwrap(), k[0]["default"].as_bool().unwrap(), k[0]["line"].as_str().unwrap()), (true, "limited until Nov 18 17:00", true, "signing in"));
        assert_eq!((k[1]["default"].as_bool().unwrap(), k[1]["plan"].as_str().unwrap(), k[1]["note"].as_str().unwrap(), k[1]["profile"].as_str().unwrap()), (true, "pro", "no usage reading yet", ""));
        let h = &p[2]["logins"][0];
        assert_eq!(h["bars"].as_array().unwrap().iter().map(|b| (b["name"].as_str().unwrap(), b["tone"].as_str().unwrap())).collect::<Vec<_>>(), [("5-hour", "ok"), ("Weekly", "warn"), ("Fable weekly", "ok")]);
    }
    #[test] fn the_same_account_added_twice_is_one_login() {
        let mut s = live();
        s["profiles"].as_array_mut().unwrap().push(json!({"id":"h2","provider":"claude","label":"Work","state":"expired","epoch":1,"identity":{"email":"Sam@Example.org"}}));
        let p = people(&s, "", 1000);
        assert_eq!(p.len(), 3, "an email in another case is the same person");
        let h = p[2]["logins"].as_array().unwrap();
        assert_eq!((h.len(), h[0]["profile"].as_str().unwrap(), h[0]["also"].clone()), (1, "h", json!(["Work"])), "the connected one with a reading is kept, the other named");
    }
    #[test] fn an_account_with_no_one_yet_stands_alone_and_a_signed_out_cli_is_said() {
        let s = json!({"profiles":[{"id":"n","provider":"claude","label":"Spare","state":"disconnected","epoch":1,"identity":{}}],
            "default_login":{"claude":{"state":"signed-out"}},"defaults":{},"holds":{}});
        let p = people(&s, "", 0);
        assert_eq!(p.len(), 1, "a signed-out CLI login with nothing known is no entry");
        assert_eq!((p[0]["key"].as_str().unwrap(), p[0]["title"].as_str().unwrap(), p[0]["logins"][0]["line"].as_str().unwrap()), ("id:n", "Spare", "not signed in"));
    }
}
