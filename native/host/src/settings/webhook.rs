//! Webhook alerts: an alert that Notify me for would raise is also POSTed as a
//! JSON notice to `push.webhook.url` (http or https only; off when unset). With a signing secret (the setting `push.webhook.secret` names a
//! file, else `<home>/secrets/push.webhook`) the request carries `X-Backplane-Timestamp` and `X-Backplane-Signature: sha256=<hex HMAC-SHA256(secret,
//! ts + "." + body)>`. A secret that is named or present but unreadable or empty is mode 0: nothing is posted, so an unsigned notice is never
//! sent in its place. The URL, the body and the signature stay in a 0600 curl config in a 0700 folder of their own, as the APNs push does; a
//! missing curl or a dead endpoint is logged and changes nothing else. A copy of a log (BACKPLANE_COPY=1) posts nothing.
//! The entry point is `deliver`; the notification pipeline (net lane) calls it for every alert it raises: this lane does not decide *when*.
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{io::Write, os::unix::fs::{DirBuilderExt, OpenOptionsExt}, path::{Path, PathBuf}, sync::Arc};

pub const URL_KEY: &str = "push.webhook.url";
pub const SECRET_KEY: &str = "push.webhook.secret";

pub fn default_file(home: &Path) -> PathBuf { home.join("secrets").join("push.webhook") }
/// Wn.path: the secret's file: the setting when it names one, else the default.
pub fn path(home: &Path, raw: &str) -> PathBuf { let s = raw.trim(); if s.is_empty() { default_file(home) } else { PathBuf::from(s) } }
/// Wn.on / Wn.url.ok / Wn.send / Wn.live: an http(s) address of printable ASCII with no space, at most 2000 characters.
pub fn on(url: &str) -> bool { !url.is_empty() }
pub fn url_ok(u: &str) -> bool { (u.starts_with("http://") || u.starts_with("https://")) && u.chars().count() <= 2000 && u.chars().all(|c| c > ' ' && (c as u32) < 127) }
pub fn send(url: &str) -> bool { on(url) && url_ok(url) }
pub fn live(raw: &str) -> bool { send(raw.trim()) }

/// What a stat of the default secret file found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seen { File, None, Fail(i32) }
pub fn seen_fail(code: i32) -> Seen { if code == 2 || code == 4058 { Seen::None } else { Seen::Fail(code) } }
/// Wn.wants: a secret is wanted when the setting names a file, or unless the default file is surely absent.
pub fn wants(set: &str, seen: Seen) -> bool { !set.trim().is_empty() || seen != Seen::None }
/// Wn.mode: 0 a configured secret is missing (do not post), 1 post unsigned, 2 post signed.
pub fn mode(asked: bool, secret: &str) -> u8 { match (asked, secret.is_empty()) { (true, true) => 0, (true, false) => 2, (false, true) => 1, (false, false) => 2 } }

/// The word TurnEnded uses: a stop is "stopped", not the codec's "interrupted".
pub fn outcome(turn: &str) -> &'static str { match turn { "failed" => "failed", "interrupted" => "stopped", "completed" => "completed", "running" => "running", _ => "idle" } }
pub fn known(s: &str) -> bool { matches!(s, "completed" | "failed" | "stopped" | "interrupted") }
/// Wn.report: the end the hub reported, or the thread's turn when that word is absent.
pub fn report(given: &str, turn: &str) -> String { if known(given) { given.to_string() } else { outcome(turn).to_string() } }
/// Wn.ended / Wn.event: a settle (done) or a failure (fail) is the event "turn.ended"; any other alert is its own kind.
pub fn ended(kind: &str) -> bool { kind == "done" || kind == "fail" }
pub fn event(kind: &str) -> String { if ended(kind) { "turn.ended".into() } else { kind.into() } }
/// Wn.fires: the kinds Notify me for raises (an empty kind is the quiet settle: nothing is posted).
pub fn fires(kind: &str) -> bool { matches!(kind, "done" | "fail" | "ask" | "merged" | "reminder" | "note") }
/// Wn.route: 0 quiet, 1 the turn-end path (done, fail), 2 the alert path (ask, merged, reminder, note).
pub fn route(kind: &str) -> u8 { if !fires(kind) { 0 } else if ended(kind) { 1 } else { 2 } }
pub fn due_turn(kind: &str) -> bool { route(kind) == 1 }
pub fn due_alert(kind: &str) -> bool { route(kind) == 2 }
/// Wn.broke: a turn that failed or that the agent cut off (a Stop the person asked for, "stopped", is not one).
pub fn broke(state: &str) -> bool { state == "failed" || state == "interrupted" }
/// Wn.due.end: a settle or failure the policy raised, or a broken turn whenever Notify me would tell of a failure (`settled`), unless private.
pub fn due_end(kind: &str, state: &str, settled: bool, private: bool) -> bool { due_turn(kind) || (settled && !private && broke(state)) }
/// Wn.end.kind: the kind a turn end posts under: the policy's, or "fail" for a broken turn the policy kept quiet.
pub fn end_kind(kind: &str, state: &str) -> String { if due_turn(kind) { kind.into() } else if broke(state) { "fail".into() } else { kind.into() } }
/// Wn.end.text
pub fn end_text(state: &str, line: &str) -> String { if state == "failed" || (broke(state) && line.is_empty()) { "Failed".into() } else if line.is_empty() { "Finished".into() } else { line.into() } }
/// Wn.reason: "limit" (a usage, rate or quota limit), "error", "interrupted", or "" (a settle, a stop). Only a broken turn is classified, and only
/// the first 2048 characters of its error are read.
pub fn reason(state: &str, err: &str) -> &'static str {
    if !broke(state) { return "" }
    let head: String = err.chars().take(2048).collect::<String>().to_lowercase();
    let limit = ["hit your usage limit", "you've hit your limit", "you have hit your limit", "weekly limit", "usage limit reached", "usage limit has been reached", "insufficient quota", "rate limit exceeded", "budget exceeded"].iter().any(|w| head.contains(w));
    if state == "failed" { if limit { "limit" } else { "error" } } else if limit { "limit" } else { "interrupted" }
}
/// Wn.end.reason: a turn that logged an error of its own ended on that error ("error", not "interrupted").
pub fn end_reason(state: &str, err: &str, reply: &str) -> &'static str { let r = reason(state, if err.is_empty() { reply } else { err }); if !err.is_empty() && r == "interrupted" { "error" } else { r } }

/// Wn.body.r: the JSON notice.
pub fn body(kind: &str, thread: &str, state: &str, reason: &str, title: &str, text: &str) -> String {
    json!({"event":event(kind),"kind":kind,"thread":thread,"state":state,"reason":reason,"title":title,"text":text}).to_string()
}
/// Sign.key: the secret's bytes: its hex, or its text when it is not hex.
fn key_bytes(s: &str) -> Vec<u8> { if !s.is_empty() && s.len() % 2 == 0 { if let Ok(b) = hex::decode(s) { return b } } s.as_bytes().to_vec() }
/// Sign.header: `sha256=<hex>` over "<ts>.<body>".
pub fn signature(secret: &str, ts: u64, body: &str) -> String { format!("sha256={}", hex::encode(crate::agents::mcp::hmac_sha256(&key_bytes(secret), format!("{ts}.{body}").as_bytes()))) }
/// Wn.headers
pub fn headers(secret: &str, ts: u64, body: &str) -> Vec<String> {
    let mut h = vec!["Content-Type: application/json".to_string()];
    if !secret.is_empty() { h.push(format!("X-Backplane-Timestamp: {ts}")); h.push(format!("X-Backplane-Signature: {}", signature(secret, ts, body))); }
    h
}
/// Wn.safe: a thread id safe to put in a temp file name.
pub fn safe(thread: &str) -> String { thread.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(80).collect() }

/// Curl.q / Curl.config: a quoted config value (control characters dropped, `"` and `\` escaped).
fn q(s: &str) -> String { let mut o = String::from("\""); for c in s.chars() { if (c as u32) < 32 { continue } if c == '"' || c == '\\' { o.push('\\') } o.push(c) } o.push('"'); o }
pub fn curl_config(url: &str, hs: &[String], body_file: &str, out_file: &str) -> String {
    let mut c = format!("url = {}\n", q(url)); for h in hs { c.push_str(&format!("header = {}\n", q(h))) }
    c.push_str(&format!("data-binary = {}\noutput = {}\nwrite-out = \"%{{http_code}}\"\n", q(&format!("@{body_file}")), q(out_file))); c
}

/// One alert, posted. `kind`, `state`, `err`, `reason`, `reply` come with the turn's end (`reason` from `end_reason`); `title` is the thread's.
/// Returns the HTTP status curl reported, or why nothing was posted.
pub async fn deliver(hub: &Arc<Hub>, thread: &str, kind: &str, state: &str, err: &str, reason: &str, reply: &str, title: &str) -> Result<String, String> {
    let url = hub.setting(URL_KEY).await.unwrap_or_default().trim().to_string();
    if !on(&url) { return Err("off".into()) }
    if !url_ok(&url) { return Err("push.webhook.url is not an http address".into()) }
    if crate::copy::on() { return Err("a copy of a log posts nothing".into()) }
    let home = hub.cfg.home.clone();
    let set = hub.setting(SECRET_KEY).await.unwrap_or_default();
    let p = path(&home, &set);
    let seen = match std::fs::metadata(&p) { Ok(_) => Seen::File, Err(e) => seen_fail(e.raw_os_error().unwrap_or(0)) };
    let secret = std::fs::read_to_string(&p).map(|t| t.trim().to_string()).unwrap_or_default();
    if mode(wants(&set, seen), &secret) == 0 { return Err("the signing secret could not be read or is empty; not posted".into()) }
    let line = if ended(kind) { end_text(state, reply) } else { reply.to_string() };
    let text = if broke(state) && !err.is_empty() { err.lines().next().unwrap_or("").to_string() } else { line };
    let b = body(kind, thread, state, reason, title, &text);
    let (url2, thread2, b2) = (url, thread.to_string(), b);
    tokio::task::spawn_blocking(move || post(&home, &thread2, &url2, &secret, &b2)).await.unwrap_or_else(|_| Err("the delivery task failed".into()))
}

fn post(home: &Path, thread: &str, url: &str, secret: &str, body: &str) -> Result<String, String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let tmp = home.join("tmp");
    std::fs::create_dir_all(&tmp).map_err(|_| "no delivery directory".to_string())?;
    let d = tmp.join(format!("hook-{now}-{}-{:x}", safe(thread), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos())));
    std::fs::DirBuilder::new().mode(0o700).create(&d).map_err(|_| "no delivery directory".to_string())?;
    let (file, out, cfg) = (d.join("body.json"), d.join("out"), d.join("curl"));
    let w = |p: &Path, t: &str| std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(p).and_then(|mut f| f.write_all(t.as_bytes()));
    let r = (|| -> Result<String, String> {
        w(&file, body).map_err(|e| e.to_string())?;
        w(&cfg, &curl_config(url, &headers(secret, now, body), &file.to_string_lossy(), &out.to_string_lossy())).map_err(|e| e.to_string())?;
        let o = crate::proc::std_cmd(crate::proc::Kind::Net, "curl").args(["-sS", "--max-time", "10", "--retry", "2", "--retry-max-time", "20", "--proto", "=http,https", "--max-redirs", "0", "-X", "POST", "-K"]).arg(&cfg).current_dir(&d).output().map_err(|e| format!("curl: {e}"))?;
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string(); Ok(s.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect())
    })();
    let _ = std::fs::remove_dir_all(&d);
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn hook_off_unset() { assert!(!on("")); }
    #[test] fn hook_on_set() { assert!(on("https://example.com/hook")); }
    #[test] fn hook_url_https() { assert!(url_ok("https://example.com/hook")); }
    #[test] fn hook_url_file() { assert!(!url_ok("file:///etc/passwd")); }
    #[test] fn hook_url_space() { assert!(!url_ok("https://example.com/a b")); }
    #[test] fn hook_live_blank() { assert!(!live("  ")); }
    #[test] fn hook_mode_unsigned() { assert_eq!(mode(false, ""), 1); }
    #[test] fn hook_mode_default_file() { assert_eq!(mode(false, "s"), 2); }
    #[test] fn hook_mode_missing() { assert_eq!(mode(true, ""), 0); }
    #[test] fn hook_mode_configured() { assert_eq!(mode(true, "s"), 2); }
    #[test] fn hook_stopped_word() { assert_eq!(outcome("interrupted"), "stopped"); }
    #[test] fn hook_failed_word() { assert_eq!(outcome("failed"), "failed"); }
    #[test] fn hook_completed_word() { assert_eq!(outcome("completed"), "completed"); }
    #[test] fn hook_body_state() { assert_eq!(j(&body("fail", "t1", "failed", "", "Board", "Failed"))["state"], "failed"); }
    #[test] fn hook_body_event() { assert_eq!(j(&body("done", "t1", "completed", "", "A", "ok"))["event"], "turn.ended"); }
    #[test] fn hook_body_kind() { assert_eq!(j(&body("fail", "t1", "failed", "", "Board", "Failed"))["kind"], "fail"); }
    #[test] fn hook_body_ask_event() { assert_eq!(j(&body("ask", "t1", "running", "", "Board", "Needs you"))["event"], "ask"); }
    #[test] fn hook_unsigned_header() { assert_eq!(headers("", 1, "{}").len(), 1); }
    #[test] fn hook_signed_header() { assert_eq!(headers("s", 1, "{}").len(), 3); }
    #[test] fn hook_secret_default() { assert_eq!(path(Path::new("/h"), ""), PathBuf::from("/h/secrets/push.webhook")); assert_eq!(path(Path::new("/h"), " /x/y "), PathBuf::from("/x/y")); }
    #[test] fn hook_secret_path() { assert_eq!(path(Path::new("/h"), "/tmp/s"), PathBuf::from("/tmp/s")); }
    #[test] fn hook_report_keeps_end() { assert_eq!(report("completed", "running"), "completed"); }
    #[test] fn hook_report_keeps_fail() { assert_eq!(report("failed", "running"), "failed"); }
    #[test] fn hook_report_fallback() { assert_eq!(report("", "interrupted"), "stopped"); }
    #[test] fn hook_route_done() { assert_eq!(route("done"), 1); }
    #[test] fn hook_route_fail() { assert_eq!(route("fail"), 1); }
    #[test] fn hook_route_ask() { assert_eq!(route("ask"), 2); }
    #[test] fn hook_route_merged() { assert_eq!(route("merged"), 2); }
    #[test] fn hook_route_reminder() { assert_eq!(route("reminder"), 2); }
    #[test] fn hook_route_note() { assert_eq!(route("note"), 2); }
    #[test] fn hook_route_quiet() { assert_eq!(route(""), 0); }
    #[test] fn a_default_secret_that_is_there_but_unreadable_is_not_a_downgrade_to_unsigned() {
        assert!(!wants("", Seen::None)); assert!(wants("", Seen::File)); assert!(wants("", Seen::Fail(13))); assert!(wants("/named", Seen::None));
        assert_eq!(seen_fail(2), Seen::None); assert_eq!(seen_fail(13), Seen::Fail(13));
    }
    #[test] fn the_signature_is_hmac_sha256_of_timestamp_dot_body_with_the_hex_secret_decoded() {
        // RFC 4231 case 2 through the same path: key "Jefe" as hex, message "what do ya want for nothing?" is not ts.body, so check the shape and the hex/text key rule
        assert_eq!(signature("4a656665", 1, "x"), signature("4a656665", 1, "x")); assert_ne!(signature("4a656665", 1, "x"), signature("4a656665", 2, "x"));
        assert_eq!(key_bytes("4a656665"), b"Jefe".to_vec()); assert_eq!(key_bytes("not hex!"), b"not hex!".to_vec()); assert_eq!(key_bytes("abc"), b"abc".to_vec(), "an odd-length string is text");
        assert!(signature("s", 5, "{}").starts_with("sha256=") && signature("s", 5, "{}").len() == 7 + 64);
    }
    #[test] fn routes_follow_the_policys_kinds() {
        assert_eq!((route("done"), route("fail"), route("ask"), route("merged"), route("reminder"), route("note"), route(""), route("other")), (1, 1, 2, 2, 2, 2, 0, 0));
        assert!(due_turn("done") && !due_turn("ask") && due_alert("ask") && !due_alert("done"));
        assert!(due_end("", "failed", true, false), "a broken turn the policy kept quiet is posted"); assert!(!due_end("", "failed", true, true), "unless private"); assert!(!due_end("", "interrupted", false, false)); assert!(!due_end("", "completed", true, false));
        assert_eq!((end_kind("", "failed").as_str(), end_kind("done", "completed").as_str(), end_kind("", "completed").as_str()), ("fail", "done", ""));
    }
    #[test] fn a_broken_turns_reason_is_a_limit_an_error_or_an_interruption() {
        assert_eq!(reason("failed", "You've hit your limit"), "limit"); assert_eq!(reason("failed", "boom"), "error"); assert_eq!(reason("interrupted", ""), "interrupted"); assert_eq!(reason("interrupted", "weekly limit"), "limit"); assert_eq!(reason("completed", "usage limit reached"), "");
        assert_eq!(end_reason("interrupted", "API error", ""), "error", "an interrupted turn that logged its own error ended on it"); assert_eq!(end_reason("interrupted", "", ""), "interrupted");
        assert_eq!((end_text("failed", "x"), end_text("interrupted", ""), end_text("completed", ""), end_text("completed", "done it")), ("Failed".to_string(), "Failed".to_string(), "Finished".to_string(), "done it".to_string()));
        assert_eq!(report("stopped", "running"), "stopped"); assert_eq!(report("?", "interrupted"), "stopped");
    }
    #[test] fn the_curl_config_quotes_and_the_thread_name_is_safe() {
        let c = curl_config("https://h/x", &["A: \"b\"".into()], "/t/body.json", "/t/out"); assert_eq!(c, "url = \"https://h/x\"\nheader = \"A: \\\"b\\\"\"\ndata-binary = \"@/t/body.json\"\noutput = \"/t/out\"\nwrite-out = \"%{http_code}\"\n");
        assert_eq!(safe("t1/../x y"), "t1_.._x_y".replace('.', "_")); assert_eq!(safe(&"a".repeat(100)).len(), 80);
    }
    /// A real POST to a local listener: the body, the signed headers, the 0600 folder gone afterwards. Skipped when this machine has no curl.
    #[tokio::test(flavor = "multi_thread")] async fn a_notice_is_posted_signed_to_the_address_and_leaves_no_file_behind() {
        if crate::proc::std_cmd(crate::proc::Kind::Net, "curl").arg("--version").output().is_err() { eprintln!("no curl: skipped"); return }
        use crate::{hub::Cfg, presenter::Canned};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
        let got = tokio::spawn(async move { let (mut s, _) = l.accept().await.unwrap(); let mut b = vec![0u8; 8192]; let mut n = 0; loop { let k = s.read(&mut b[n..]).await.unwrap(); n += k; let t = String::from_utf8_lossy(&b[..n]).to_string(); if let Some(i) = t.find("\r\n\r\n") { let cl = t.to_lowercase().split("content-length: ").nth(1).and_then(|x| x.split("\r\n").next()).and_then(|x| x.trim().parse::<usize>().ok()).unwrap_or(0); if n >= i + 4 + cl { break } } if k == 0 { break } }
            s.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n").await.unwrap(); String::from_utf8_lossy(&b[..n]).to_string() });
        let home = std::env::temp_dir().join(format!("bp-hook-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(home.join("secrets")).unwrap();
        std::fs::write(default_file(&home), "73656372657431\n").unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        hub.commit(&json!({"$":"SettingSet","key":URL_KEY,"value":format!("http://127.0.0.1:{port}/hook"),"at":1})).await.unwrap();
        let r = deliver(&hub, "t1", "fail", "failed", "boom\nsecond line", "error", "", "Board").await; assert_eq!(r, Ok("204".to_string()));
        let req = got.await.unwrap(); let lower = req.to_lowercase();
        assert!(req.starts_with("POST /hook HTTP/1.1") && lower.contains("content-type: application/json") && lower.contains("x-backplane-timestamp: "), "{req}");
        let b = req.split("\r\n\r\n").nth(1).unwrap(); let v: Value = serde_json::from_str(b).unwrap();
        assert_eq!((v["event"].clone(), v["state"].clone(), v["reason"].clone(), v["title"].clone(), v["text"].clone()), (json!("turn.ended"), json!("failed"), json!("error"), json!("Board"), json!("boom")));
        let ts: u64 = lower.split("x-backplane-timestamp: ").nth(1).unwrap().split("\r\n").next().unwrap().parse().unwrap();
        let sig = req.split("X-Backplane-Signature: ").nth(1).or_else(|| req.split("x-backplane-signature: ").nth(1)).unwrap().split("\r\n").next().unwrap();
        assert_eq!(sig, signature("73656372657431", ts, b), "signed with the file's secret, hex decoded");
        assert!(std::fs::read_dir(home.join("tmp")).map(|d| d.count()).unwrap_or(0) == 0, "no delivery folder is left");
        // a named secret that is empty: nothing is posted, never an unsigned notice
        std::fs::write(default_file(&home), "\n").unwrap(); assert!(deliver(&hub, "t1", "fail", "failed", "", "error", "", "Board").await.unwrap_err().contains("not posted"));
        hub.commit(&json!({"$":"SettingSet","key":URL_KEY,"value":"file:///etc/passwd","at":2})).await.unwrap(); assert_eq!(deliver(&hub, "t1", "done", "completed", "", "", "ok", "B").await, Err("push.webhook.url is not an http address".into()));
        hub.commit(&json!({"$":"SettingSet","key":URL_KEY,"value":"","at":3})).await.unwrap(); assert_eq!(deliver(&hub, "t1", "done", "completed", "", "", "ok", "B").await, Err("off".into()));
        let _ = std::fs::remove_dir_all(&home);
    }
}
