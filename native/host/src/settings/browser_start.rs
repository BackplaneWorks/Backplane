//! Settings › Browser › Start page: what a new or empty Browser tab opens, on the desktop, on the phones and for an agent's `preview_open` with no
//! address. One hub setting, `browser.start`: "" is the default (https://www.google.com), "blank" an empty page, else an http(s) address.
//! The hub owns the rule (`normalize`, `url`) and the rows; the desktop draws them through the presenter (`present/settings.rs` page `browser`), the
//! phones ask `browser.page` and press with `browser.page.act {action, value, fields}` (data/1 feature `browserstart`).
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

pub const KEY: &str = "browser.start";
pub const DEFAULT: &str = "https://www.google.com";
pub const BLANK: &str = "blank";
/// The field's name on the page (`@f.bstart` on the desktop, `fields.bstart` from a phone).
pub const FIELD: &str = "bstart";
pub const ACTION: &str = "browser-start";
pub const BAD: &str = "The start page must be an http or https address.";

fn url_ok(u: &str) -> bool {
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or("");
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty() && u.len() <= 2000 && !u.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// What a typed start page is stored as: an http(s) address as typed, a bare host (`example.com`, `localhost:3000/x`) with https:// in front,
/// "blank" for a blank page (`blank` or `about:blank`). Anything else (another scheme, spaces, nothing) is refused with the words to show.
pub fn normalize(typed: &str) -> Result<String, &'static str> {
    let t = typed.trim();
    if t.eq_ignore_ascii_case(BLANK) || t.eq_ignore_ascii_case("about:blank") { return Ok(BLANK.into()) }
    let lower = t.to_ascii_lowercase();
    let u = if let Some(scheme) = ["http://", "https://"].into_iter().find(|s| lower.starts_with(s)) { format!("{scheme}{}", &t[scheme.len()..]) }
        else if t.contains("://") { return Err(BAD) }
        else {
            // a bare host, perhaps with a port: a colon anywhere else is another scheme (javascript:, mailto:, data:)
            let host = t.split(['/', '?', '#']).next().unwrap_or("");
            let port_ok = host.split_once(':').map_or(true, |(h, p)| !h.is_empty() && !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
            if !port_ok { return Err(BAD) }
            format!("https://{t}")
        };
    if url_ok(&u) { Ok(u) } else { Err(BAD) }
}
/// Whether a value may be stored as it is: "", "blank" or an address `normalize` keeps as it is.
pub fn storable(v: &str) -> bool { v.is_empty() || v == BLANK || normalize(v).as_deref() == Ok(v) }

/// The address a new tab opens for the stored value: the default for "" (or a value that is no longer good), about:blank for "blank".
pub fn url(stored: &str) -> String {
    match stored.trim() { "" => DEFAULT.into(), BLANK => "about:blank".into(), v if storable(v) => v.into(), _ => DEFAULT.into() }
}
/// The address to hand the browser helper as its `homeUrl`, None for a blank start page (the page stays empty).
pub fn home(stored: &str) -> Option<String> { let u = url(stored); (u != "about:blank").then_some(u) }

/// The page's rows: the address field with Save, Reset to default and Blank (one row: a kept press clears what was typed on every client), and what a new tab opens now. `typed` is the field as the person
/// left it (None: the stored address), `err` a refusal to say.
pub fn rows(stored: &str, typed: Option<&str>, err: &str) -> Vec<Value> {
    let stored = if storable(stored) { stored } else { "" };
    let value = typed.map(str::to_string).unwrap_or_else(|| if stored == BLANK { String::new() } else { url(stored) });
    let btn = |label: &str, value: &str, on: bool| json!({"label": label, "action": ACTION, "value": value, "on": on});
    let now = match stored { "" => format!("New tabs open {DEFAULT}, the default."), BLANK => "New tabs open blank.".to_string(), u => format!("New tabs open {u}.") };
    vec![
        json!({"label": "Start page", "note": if err.is_empty() { "What a new Browser tab opens, yours and an agent's. An http or https address; a bare name like example.com gets https://." } else { err },
            "fields": [{"name": FIELD, "hint": DEFAULT, "secret": false, "value": value}], "buttons": [btn("Save", "save", false), btn("Reset to default", "reset", stored.is_empty()), btn("Blank", BLANK, stored == BLANK)], "tone": if err.is_empty() { "" } else { "bad" }}),
        json!({"label": "Opens", "note": now, "buttons": []}),
    ]
}

/// A press: the value to store, or the refusal to show. `save` reads the typed field, `reset` goes back to the default, `blank` an empty page.
pub fn act(value: &str, typed: &str) -> Result<String, &'static str> {
    match value { "save" => normalize(typed), "reset" => Ok(String::new()), BLANK => Ok(BLANK.into()), _ => Err("browser-start: save, reset or blank") }
}

/// `browser.page` → `{rows}`; `browser.page.act {action, value, fields?}` → `{}` (the phones; the desktop's presses go through `setting.set`).
pub struct BrowserPage;
impl RpcHandler for BrowserPage {
    fn methods(&self) -> &'static [&'static str] { &["browser.page", "browser.page.act"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let stored = hub.setting(KEY).await.unwrap_or_default();
            if method == "browser.page" { return vec![json!({"t":"reply","id":id,"ok":true,"rows":rows(&stored, None, "")})] }
            let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            if s("action") != ACTION { return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown action"})] }
            let v = match act(&s("value"), p["fields"][FIELD].as_str().unwrap_or("")) { Ok(v) => v, Err(e) => return vec![json!({"t":"reply","id":id,"ok":false,"error":e})] };
            match hub.commit(&json!({"$": "SettingSet", "key": KEY, "value": v, "at": now_ms() / 1000})).await {
                Ok(_) => vec![json!({"t":"reply","id":id,"ok":true})],
                Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})],
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_typed_start_page_is_an_http_address_a_bare_host_gets_https_and_the_rest_is_refused() {
        for (typed, want) in [("https://example.com/a?b=1", "https://example.com/a?b=1"), (" http://127.0.0.1:3000/ ", "http://127.0.0.1:3000/"), ("example.com", "https://example.com"),
            ("docs.example.com/path", "https://docs.example.com/path"), ("localhost:3000", "https://localhost:3000"), ("HTTPS://Example.com", "https://Example.com"), ("blank", "blank"), ("about:blank", "blank")] {
            assert_eq!(normalize(typed).as_deref(), Ok(want), "{typed}");
        }
        for typed in ["", "  ", "ftp://example.com", "javascript:alert(1)", "file:///etc/passwd", "a b.com", "https://", "data:text/html,x", "mailto:a@b.c", "chrome://settings"] {
            assert_eq!(normalize(typed), Err(BAD), "{typed}");
        }
    }
    #[test] fn the_stored_value_says_what_a_new_tab_opens() {
        assert_eq!(url(""), DEFAULT); assert_eq!(url("blank"), "about:blank"); assert_eq!(url("https://example.com"), "https://example.com");
        assert_eq!(url("ftp://x"), DEFAULT, "a value that is not good any more opens the default");
        assert_eq!(home("blank"), None); assert_eq!(home("").as_deref(), Some(DEFAULT));
        assert!(storable("") && storable("blank") && storable("https://a.example") && !storable("a.example") && !storable("javascript:x"));
    }
    #[test] fn the_rows_hold_the_field_save_reset_and_blank_lit_as_stored() {
        let r = rows("", None, "");
        assert_eq!(r[0]["fields"][0]["value"], DEFAULT); assert_eq!(r[0]["fields"][0]["name"], FIELD);
        let lit = |r: &[Value]| r[0]["buttons"].as_array().unwrap().iter().filter(|b| b["on"] == true).map(|b| b["value"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(lit(&r), ["reset"]); assert!(r[1]["note"].as_str().unwrap().contains("the default"));
        let b = rows("blank", None, ""); assert_eq!(lit(&b), ["blank"]); assert_eq!(b[0]["fields"][0]["value"], "");
        let u = rows("https://example.com", Some("exa"), ""); assert!(lit(&u).is_empty()); assert_eq!(u[0]["fields"][0]["value"], "exa", "what is typed stays");
        let e = rows("", Some("ftp://x"), BAD); assert_eq!((e[0]["note"].as_str(), e[0]["tone"].as_str()), (Some(BAD), Some("bad")));
        assert_eq!((act("save", "example.com"), act("reset", "x"), act("blank", ""), act("save", "ftp://x")), (Ok("https://example.com".into()), Ok(String::new()), Ok("blank".into()), Err(BAD)));
    }
}
