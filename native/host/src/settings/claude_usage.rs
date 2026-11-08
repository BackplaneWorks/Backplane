//! Claude's usage read with each login's own OAuth token: `GET https://api.anthropic.com/api/oauth/usage` (read-only), what Claude Code's `/usage` asks.
//! Claude Code 2.1.319 answers the hub's `get_usage` control request with `rate_limits_available: true` and `rate_limits: null` without asking the endpoint
//! (seen on a saved Max profile on Nov 8 2026, while the endpoint itself answered with that profile's token), so the hub asks the endpoint itself.
//! The token is read from the login's own store (`<store>/.credentials.json`, `claudeAiOauth`), passed to curl on its stdin (never an argument, a log line or an
//! error), and dropped. An expired token is refreshed by Claude Code itself (one start of it in that store; it takes its own refresh lock), never here.
//! The answer becomes the hub's windows (`used_percent`, `resets_at` in epoch seconds) and the extra-usage spend; nothing else of it is kept.
use serde_json::{json, Value};
use std::{future::Future, path::{Path, PathBuf}, pin::Pin, sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// A token this close to its end is refreshed first (ms).
const MARGIN_MS: u64 = 60_000;

/// One login's OAuth access token and when it ends (ms; 0: not said). Its Debug never shows the token.
#[derive(Clone, PartialEq)]
pub struct Login { pub token: String, pub expires_ms: u64 }
impl std::fmt::Debug for Login { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Login {{ token: <{} bytes>, expires_ms: {} }}", self.token.len(), self.expires_ms) } }
impl Login { pub fn fresh(&self, now_ms: u64) -> bool { self.expires_ms == 0 || self.expires_ms > now_ms + MARGIN_MS } }

/// The Claude.ai login kept in a Claude Code store, or None (no file, an API-key or console login, a keychain login on macOS).
pub fn read_login(dir: &Path) -> Option<Login> {
    let f = dir.join(".credentials.json");
    if std::fs::metadata(&f).ok()?.len() > 16 << 20 { return None }
    let v: Value = serde_json::from_slice(&std::fs::read(&f).ok()?).ok()?;
    let o = &v["claudeAiOauth"];
    let token = o["accessToken"].as_str().filter(|t| !t.is_empty() && t.len() < 8192 && t.bytes().all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\'))?.to_string();
    Some(Login { token, expires_ms: o["expiresAt"].as_u64().or_else(|| o["expiresAt"].as_f64().map(|x| x as u64)).unwrap_or(0) })
}
/// Where the CLI's own login keeps its store: `$CLAUDE_CONFIG_DIR` is not inherited by the default login's children (accounts_io `STORE_ENV`), so `~/.claude`.
pub fn default_dir() -> Option<PathBuf> { std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join(".claude")) }

/// The request: a token in, (HTTP status, body) out; Err only when no answer came (its words never hold the token).
pub type Get = Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<(u16, Value), String>> + Send>> + Send + Sync>;
/// The real request, through curl (Kind::Net, so a copy of a hub never makes it). The headers go in on stdin as curl's config.
pub fn curl() -> Get {
    Arc::new(|token: String| Box::pin(async move {
        let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Net, "curl");
        c.args(["-sS", "--proto", "=https", "--max-time", "15", "--max-filesize", "1048576", "-K", "-"]);
        c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
        let mut child = c.spawn().map_err(|_| "curl could not be started to read Claude's usage.".to_string())?;
        let cfg = format!("url = \"{URL}\"\nheader = \"Authorization: Bearer {token}\"\nheader = \"anthropic-beta: oauth-2025-04-20\"\nheader = \"Content-Type: application/json\"\nheader = \"User-Agent: backplane-hub\"\nwrite-out = \"\\n%{{http_code}}\"\n");
        drop(token);
        { let mut stdin = child.stdin.take().ok_or("curl could not be started to read Claude's usage.")?; stdin.write_all(cfg.as_bytes()).await.map_err(|_| "curl closed before the request.".to_string())?; }
        drop(cfg);
        let mut out = child.stdout.take().ok_or("curl could not be started to read Claude's usage.")?;
        let mut buf = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(20), async { let mut chunk = [0u8; 8192]; loop { let n = out.read(&mut chunk).await.map_err(|_| ())?; if n == 0 { break } buf.extend_from_slice(&chunk[..n]); if buf.len() > 1 << 20 { return Err(()) } } Ok::<(), ()>(()) }).await;
        let _ = child.wait().await;
        if !matches!(read, Ok(Ok(()))) { return Err("Claude's usage service did not answer within 20 seconds.".into()) }
        answer(&buf)
    }))
}
/// curl's output: the body, then a line with the status (`write-out`). No status: no answer.
pub fn answer(out: &[u8]) -> Result<(u16, Value), String> {
    let s = String::from_utf8_lossy(out);
    let (body, code) = s.rsplit_once('\n').unwrap_or(("", &s));
    let code: u16 = code.trim().parse().ok().filter(|c| *c > 0).ok_or("Claude's usage service could not be reached.")?;
    Ok((code, serde_json::from_str(body).unwrap_or(Value::Null)))
}
/// What a status other than 200 says, in the words the failover reads (a 401 is a dead login: fallback `auth_text`).
pub fn refusal(code: u16) -> String {
    match code {
        401 => "Claude refused the request: OAuth token rejected (status code 401).".into(),
        403 => "Claude refused the usage read for this account (403); it may not be a subscription plan.".into(),
        429 => "Claude is limiting usage reads right now (429); the last reading stays.".into(),
        c => format!("Claude's usage service answered {c}."),
    }
}

/// A window's reset, to the nearest minute: the service says "07:59:59.98" one moment and "08:00:00.24" the next for the same reset.
fn reset(w: &Value) -> Option<u64> { w["resets_at"].as_str().and_then(super::accounts_io::parse_iso).or_else(|| w["resets_at"].as_f64().map(|x| x as u64)).filter(|r| *r > 0).map(|r| (r + 30) / 60 * 60) }
fn percent(v: &Value) -> Option<f64> { v.as_f64().filter(|u| u.is_finite() && *u >= 0.0).map(|u| u.min(100.0)) }
/// The fixed windows the endpoint names: (key, minutes, label, shared).
const FIXED: &[(&str, u64, &str, bool)] = &[("five_hour", 300, "5-hour", true), ("seven_day", 10080, "Weekly", true), ("seven_day_opus", 10080, "Opus weekly", false), ("seven_day_sonnet", 10080, "Sonnet weekly", false)];
/// The windows of a usage answer (the endpoint's body, or what Claude Code's `get_usage` passes on of it as `rate_limits`): the 5-hour and weekly pools, and each
/// model's own weekly limit (a fixed key, or a `limits[]` entry of kind `weekly_scoped`, which names the model; `model` is then set, so it applies to that model only).
/// Over 100 reads as 100 (out); a window with no utilization is left out.
pub fn windows(body: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for (key, minutes, label, shared) in FIXED {
        let w = &body[*key]; let Some(u) = percent(&w["utilization"]) else { continue };
        let mut o = json!({"name":key,"scope":key,"label":label,"shared":shared,"used_percent":u,"window_minutes":minutes});
        if let Some(r) = reset(w) { o["resets_at"] = json!(r) }
        out.push(o);
    }
    for l in body["limits"].as_array().into_iter().flatten().take(20) {
        if l["kind"] != "weekly_scoped" { continue }
        let name = super::accounts_io::label(l["scope"]["model"]["display_name"].as_str().unwrap_or(""));
        let Some(u) = percent(&l["percent"]) else { continue };
        if name.is_empty() { continue }
        let model = name.to_lowercase(); let scope = format!("seven_day_{}", model.replace(' ', "_"));
        if out.iter().any(|w| w["scope"] == scope.as_str()) { continue }
        let mut o = json!({"name":scope,"scope":scope,"label":format!("{name} weekly"),"model":model,"shared":false,"used_percent":u,"window_minutes":10080,"active":l["is_active"].as_bool().unwrap_or(true)});
        if let Some(r) = reset(l) { o["resets_at"] = json!(r) }
        out.push(o);
    }
    out
}
/// Money in minor units: "$12.34" for dollars, else "12.34 EUR".
fn money(minor: f64, exponent: u32, currency: &str) -> String {
    let v = minor / 10f64.powi(exponent as i32);
    let n = format!("{v:.prec$}", prec = exponent.min(4) as usize);
    if currency.is_empty() || currency == "USD" { format!("${n}") } else { format!("{n} {currency}") }
}
/// The extra usage (paid beyond the plan): {enabled, used, limit, percent, reason}, from `spend` when the answer has it, else `extra_usage`; None when neither.
pub fn extra(body: &Value) -> Option<Value> {
    let sp = &body["spend"];
    if sp.is_object() && sp["used"].is_object() {
        let e = |m: &Value| m["exponent"].as_u64().unwrap_or(2) as u32;
        let cur = sp["used"]["currency"].as_str().or(sp["limit"]["currency"].as_str()).unwrap_or("USD");
        let used = money(sp["used"]["amount_minor"].as_f64().unwrap_or(0.0), e(&sp["used"]), cur);
        let limit = sp["limit"]["amount_minor"].as_f64().map(|m| money(m, e(&sp["limit"]), cur));
        return Some(json!({"enabled":sp["enabled"].as_bool().unwrap_or(false),"used":used,"limit":limit,"percent":sp["percent"].as_f64(),"reason":super::accounts_io::label(sp["disabled_reason"].as_str().unwrap_or(""))}));
    }
    let x = &body["extra_usage"];
    if !x.is_object() { return None }
    let (exp, cur) = (x["decimal_places"].as_u64().unwrap_or(2) as u32, x["currency"].as_str().unwrap_or("USD"));
    Some(json!({"enabled":x["is_enabled"].as_bool().unwrap_or(false),"used":money(x["used_credits"].as_f64().unwrap_or(0.0), exp, cur),"limit":x["monthly_limit"].as_f64().map(|m| money(m, exp, cur)),
        "percent":x["utilization"].as_f64(),"reason":super::accounts_io::label(x["disabled_reason"].as_str().unwrap_or(""))}))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    /// The endpoint's answer for a Max account on Nov 8 2026, as recorded (no token in it; the codenamed keys it also sends were null and are kept so the reader is
    /// shown ignoring them).
    pub const RECORDED: &str = r#"{"five_hour":null,"seven_day":{"utilization":64.0,"resets_at":"2026-11-11T17:00:00.229000+00:00","limit_dollars":null,"used_dollars":null,"remaining_dollars":null,"locked_reason":null},
        "seven_day_oauth_apps":null,"seven_day_opus":null,"seven_day_sonnet":null,"seven_day_cowork":null,"tangelo":null,"promo_credits":null,"pace":null,"indicator":null,
        "extra_usage":{"is_enabled":false,"monthly_limit":5000,"used_credits":0.0,"utilization":0.0,"currency":"USD","decimal_places":2,"disabled_reason":"out_of_credits","user_disabled":false,"spend_limit_reached":false,"credits_ever_enabled":true,"daily":null,"weekly":null},
        "limits":[{"kind":"weekly_all","group":"weekly","percent":64,"severity":"normal","resets_at":"2026-11-11T17:00:00.229000+00:00","scope":null,"is_active":true},
                  {"kind":"weekly_scoped","group":"weekly","percent":0,"severity":"normal","resets_at":"2026-11-11T17:00:00+00:00","scope":{"model":{"id":null,"display_name":"Fable"},"surface":null},"is_active":false}],
        "spend":{"used":{"amount_minor":0,"currency":"USD","exponent":2},"limit":{"amount_minor":5000,"currency":"USD","exponent":2},"percent":0,"severity":"normal","enabled":false,"disabled_reason":"out_of_credits","cap":{"money":null,"credits":{"amount_minor":5000,"exponent":2}},"balance":null,"auto_reload":null,"can_purchase_credits":false,"can_toggle":false},
        "member_dashboard_available":false,
        "seven_day_breakdown":{"as_of":"2026-11-08T22:09:54.277028+00:00","window_started_at":"2026-11-04T17:00:00.229000+00:00","rows":[{"key":"claude_code","display_name":"Claude Code","percent":100},{"key":"chat","display_name":"Chats","percent":0}]},
        "weekly_scoped_shares":[{"limit_index":1,"used_percent_of_weekly":0,"allowance_percent_of_weekly":50}]}"#;
    /// An answer in the middle of a 5-hour window with Opus and Sonnet pools and spend on, in the shape of earlier answers (numbers made up).
    pub const BUSY: &str = r#"{"five_hour":{"utilization":81.5,"resets_at":"2026-11-08T19:30:00+00:00"},"seven_day":{"utilization":40,"resets_at":"2026-11-11T17:00:00Z"},
        "seven_day_opus":{"utilization":130,"resets_at":"2026-11-11T17:00:00Z"},"seven_day_sonnet":{"utilization":2.5,"resets_at":null},
        "limits":[{"kind":"weekly_scoped","percent":100,"resets_at":"2026-11-11T17:00:00Z","scope":{"model":{"display_name":"Opus"}},"is_active":true}],
        "extra_usage":{"is_enabled":true,"monthly_limit":10000,"used_credits":1234.0,"utilization":12.34,"currency":"EUR","decimal_places":2,"disabled_reason":null}}"#;

    #[test] fn the_recorded_answer_reads_as_its_weekly_window_and_the_models_own() {
        let b: Value = serde_json::from_str(RECORDED).unwrap();
        let w = windows(&b);
        assert_eq!(w.len(), 2, "{w:?}");
        assert_eq!((w[0]["scope"].clone(), w[0]["label"].clone(), w[0]["used_percent"].clone(), w[0]["shared"].clone(), w[0]["window_minutes"].clone()), (json!("seven_day"), json!("Weekly"), json!(64.0), json!(true), json!(10080)));
        assert_eq!(w[0]["resets_at"], 1_794_416_400u64, "2026-11-11T17:00:00Z");
        assert_eq!((w[1]["scope"].clone(), w[1]["label"].clone(), w[1]["model"].clone(), w[1]["shared"].clone(), w[1]["used_percent"].clone(), w[1]["active"].clone()), (json!("seven_day_fable"), json!("Fable weekly"), json!("fable"), json!(false), json!(0.0), json!(false)));
        assert!(!w.iter().any(|x| x["scope"] == "five_hour"), "no 5-hour window is open: none is made up");
        assert_eq!(extra(&b).unwrap(), json!({"enabled":false,"used":"$0.00","limit":"$50.00","percent":0.0,"reason":"out_of_credits"}));
    }
    #[test] fn a_busy_answer_has_every_window_once_and_over_a_hundred_is_out() {
        let b: Value = serde_json::from_str(BUSY).unwrap();
        let w = windows(&b);
        assert_eq!(w.iter().map(|x| x["scope"].as_str().unwrap()).collect::<Vec<_>>(), ["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet"], "the scoped Opus limit is the fixed key's");
        assert_eq!((w[0]["used_percent"].clone(), w[0]["resets_at"].clone()), (json!(81.5), json!(1_794_166_200u64)));
        assert_eq!(w[2]["used_percent"], 100.0); assert!(w[3].get("resets_at").is_none());
        assert_eq!(extra(&b).unwrap(), json!({"enabled":true,"used":"12.34 EUR","limit":"100.00 EUR","percent":12.34,"reason":""}));
        assert!(windows(&Value::Null).is_empty() && extra(&json!({})).is_none());
        let jitter = windows(&json!({"seven_day":{"utilization":1,"resets_at":"2026-11-11T16:59:59.981000+00:00"}})); assert_eq!(jitter[0]["resets_at"], 1_794_416_400u64, "a reset a hair early is the minute it means");
        // Claude Code's own `get_usage` passes the same body on as `rate_limits`, with its projection added
        let cli = json!({"rate_limits_available":true,"rate_limits":b.clone()});
        assert_eq!(super::super::accounts_io::Accounts::claude_windows(&cli), w);
    }
    #[test] fn a_login_is_read_from_its_own_store_and_never_shown() {
        let d = std::env::temp_dir().join(format!("bp-usage-login-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
        assert_eq!(read_login(&d), None);
        std::fs::write(d.join(".credentials.json"), r#"{"claudeAiOauth":{"accessToken":"tok-A","refreshToken":"r","expiresAt":2000000,"scopes":["user:inference"]},"mcpOAuth":{}}"#).unwrap();
        let l = read_login(&d).unwrap(); assert_eq!((l.token.as_str(), l.expires_ms), ("tok-A", 2_000_000));
        assert!(!format!("{l:?}").contains("tok-A"), "Debug hides the token");
        assert!(l.fresh(1_000_000) && !l.fresh(1_950_000) && !l.fresh(3_000_000), "a token within a minute of its end is not fresh");
        std::fs::write(d.join(".credentials.json"), r#"{"claudeAiOauth":{"accessToken":""}}"#).unwrap(); assert_eq!(read_login(&d), None);
        std::fs::write(d.join(".credentials.json"), r#"{"primaryApiKey":"sk"}"#).unwrap(); assert_eq!(read_login(&d), None, "an API-key login has no usage to read");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn curls_output_is_the_body_then_the_status() {
        assert_eq!(answer(b"{\"a\":1}\n200").unwrap(), (200, json!({"a":1})));
        assert_eq!(answer(b"\n429").unwrap(), (429, Value::Null));
        assert!(answer(b"").is_err() && answer(b"\n000").is_err(), "no answer");
        assert!(super::super::fallback::auth_text(&refusal(401)), "a 401 is a dead login to the failover");
        assert!(!super::super::fallback::auth_text(&refusal(429)) && !super::super::fallback::auth_text(&refusal(500)));
    }
}
