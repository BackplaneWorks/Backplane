//! The telemetry sender (docs/telemetry.md), ported from 0.14's `src/server/telemetry.bend` and `src/core/telemetry{,-usage}.bend`.
//! Wire v1 to `https://backplaneworks.com/api/v1/events`, HMAC-SHA256 over `ts + "." + body` with the release key baked at compile time
//! (`option_env!`: CI secrets, never in the repo). A build with no endpoint, key id or key sends nothing, and so does every kill switch.
//!
//! What is sent is an allowlist: `install`/`update`/`active` events (a random UUIDv4, version, channel, OS, architecture, UTC day, the
//! previous version of an update, the install's proof of work) and, in a separate request, `usage` snapshots of twelve fixed integer
//! counters (settings/telemetry_usage.rs). Nothing else is ever put in a body: no content, names, paths or keys.
//!
//! Durability: `<home>/telemetry/state.json` (0600, in a 0700 folder, written by atomic rename) is the outbox: the id, the queue of events
//! not yet acknowledged, the retry time. It survives restarts; a queued event leaves it only on HTTP 202 or a permanent 400/401/403/409/413,
//! anything else keeps it with an hour's backoff (or a longer Retry-After). Events are de-duplicated by kind (one install per id, one update per
//! version, one active per day), expire after seven days and cap at 200. `usage.json` holds the acknowledged usage snapshots, so an unchanged
//! day is not sent again. Off, or a disclosure not yet seen (settings/telemetry.rs `may_send`), deletes all of it.
use crate::{api::Observer, hub::{now_ms, Hub}};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::{Path, PathBuf}, sync::{atomic::{AtomicBool, AtomicU64, Ordering::Relaxed}, Arc, OnceLock}};

/// The one receiver a release may talk to.
pub const ENDPOINT: &str = "https://backplaneworks.com/api/v1/events";
const WEEK: u64 = 7 * 86400;
const MAX_QUEUE: usize = 200;
const MAX_BATCH: usize = 50;
/// 16 KiB less room for the envelope.
const MAX_BYTES: usize = 16256;
pub const POW_BITS: u32 = 18;

// ---- configuration ----

#[derive(Clone, Debug, PartialEq)]
pub struct Cfg { pub url: String, pub kid: String, pub key: String, pub channel: String }

fn hex64(s: &str) -> bool { s.len() == 64 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) }
/// `http://127.0.0.1:PORT/api/v1/events`, the only endpoint test mode accepts.
pub fn loopback(url: &str) -> bool {
    url.strip_prefix("http://127.0.0.1:").and_then(|r| r.strip_suffix("/api/v1/events")).and_then(|p| p.parse::<u16>().ok()).map_or(false, |p| p > 0)
}
/// The channel a version is: `-nightly.` is nightly, a plain x.y.z is release.
pub fn channel_of(version: &str) -> &'static str { if version.contains("-nightly.") { "nightly" } else { "release" } }
/// Tm.config.ok: a release key (`rel-…`, 64 hex) for the canonical receiver, or in explicit test mode a loopback receiver.
pub fn config_ok(test: bool, c: &Cfg) -> bool {
    hex64(&c.key) && (c.kid.starts_with("dev-") || c.kid.starts_with("rel-")) && c.kid.len() <= 32
        && if test { loopback(&c.url) && matches!(c.channel.as_str(), "release" | "nightly" | "dev") } else { c.url == ENDPOINT && c.kid.starts_with("rel-") && matches!(c.channel.as_str(), "release" | "nightly") }
}
/// Tm.gate: the environment's kill switches (any one closes it).
pub fn gate(env: &dyn Fn(&str) -> String, sandbox_arg: bool) -> bool {
    let sb = env("BACKPLANE_SANDBOX"); let tm = env("BACKPLANE_TELEMETRY");
    !sandbox_arg && (sb.is_empty() || sb == "0") && tm != "0" && tm != "off" && env("DO_NOT_TRACK") != "1" && env("CI").is_empty()
}
/// The configuration this process sends with, None when it sends nothing. Baked values only, unless `BACKPLANE_TELEMETRY_TEST=1` names a
/// loopback receiver (integration tests). Read once.
pub fn cfg() -> Option<&'static Cfg> {
    static C: OnceLock<Option<Cfg>> = OnceLock::new();
    C.get_or_init(|| {
        let env = |k: &str| std::env::var(k).unwrap_or_default();
        if !gate(&env, std::env::args().any(|a| a == "--sandbox")) { return None }
        let test = env("BACKPLANE_TELEMETRY_TEST") == "1";
        let baked = |b: Option<&'static str>| b.unwrap_or("").to_string();
        let version = env!("CARGO_PKG_VERSION");
        let c = if test { Cfg { url: env("BACKPLANE_TELEMETRY_URL"), kid: env("BACKPLANE_TELEMETRY_KID"), key: env("BACKPLANE_TELEMETRY_KEY"), channel: { let c = env("BACKPLANE_TELEMETRY_CHANNEL"); if c.is_empty() { "dev".into() } else { c } } } }
            else { Cfg { url: baked(option_env!("BACKPLANE_TELEMETRY_URL")), kid: baked(option_env!("BACKPLANE_TELEMETRY_KID")), key: baked(option_env!("BACKPLANE_TELEMETRY_KEY")), channel: channel_of(version).into() } };
        config_ok(test, &c).then_some(c)
    }).as_ref()
}
/// Whether this build can send at all (the notice is only offered when it can).
pub fn capable() -> bool { cfg().is_some() || TEST_CAPABLE.load(Relaxed) }
/// Tests stand in a loopback receiver for the baked one (never set outside `cargo test`).
pub static TEST_CAPABLE: AtomicBool = AtomicBool::new(false);

// ---- the wire (pure) ----

/// One queued event: 0.14's TmEv, field for field, so an old state file still reads. `share` is always true here (no anonymous events
/// are made), `made` and `held` are local bookkeeping and never sent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ev { pub e: String, pub day: String, pub share: bool, pub id: String, pub ver: String, pub prev: String, pub ch: String, pub os: String, pub arch: String, pub pow: String, pub made: u64, pub held: bool }

/// The outbox: 0.14's TmState. `reset` is the reset marker the id belongs to; `today` the last day an active event was queued.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tm { pub id: String, pub ver: String, pub today: String, pub share: bool, pub reset: String, pub queue: Vec<Ev>, pub next: u64, pub busy: u64, pub seen: bool }

/// What the planner is told of this hub now.
#[derive(Clone, Debug, PartialEq)]
pub struct View { pub reset: String, pub ver: String, pub ch: String, pub os: String, pub arch: String, pub day: String, pub now: u64 }

fn hexn(s: &str, n: usize) -> bool { s.len() == n && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) }
pub fn uuid_ok(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    s.len() == 36 && p.len() == 5 && hexn(p[0], 8) && hexn(p[1], 4) && hexn(p[2], 4) && p[2].starts_with('4') && hexn(p[3], 4) && p[3].starts_with(['8', '9', 'a', 'b']) && hexn(p[4], 12)
}
/// The receiver's version pattern: `\d{1,4}\.\d{1,4}\.\d{1,4}(-[0-9A-Za-z.]{1,32})?`.
pub fn version_ok(s: &str) -> bool {
    let (base, suf) = match s.split_once('-') { Some((b, x)) => (b, Some(x)), None => (s, None) };
    let parts: Vec<&str> = base.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| (1..=4).contains(&p.len()) && p.bytes().all(|c| c.is_ascii_digit()))
        && suf.map_or(true, |x| (1..=32).contains(&x.len()) && x.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.'))
}
pub fn day_ok(s: &str) -> bool { s.len() == 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-' && s.bytes().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) }
/// UTC day of unix seconds.
pub fn day_of(secs: u64) -> String {
    let z = (secs / 86400) as i64 + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153; let d = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 }; let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Tm.clean: only what the receiver's schema accepts is ever put in a body.
pub fn clean(ev: &Ev) -> bool {
    let fields = matches!(ev.e.as_str(), "install" | "update" | "active") && day_ok(&ev.day) && version_ok(&ev.ver) && matches!(ev.ch.as_str(), "release" | "nightly" | "dev")
        && matches!(ev.os.as_str(), "linux" | "darwin" | "windows") && matches!(ev.arch.as_str(), "x64" | "arm64");
    let prev = ev.prev.is_empty() || (ev.e == "update" && version_ok(&ev.prev));
    let pow = ev.pow.is_empty() || (ev.e == "install" && ev.pow.len() <= 16 && ev.pow.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)));
    !ev.held && ev.share && uuid_ok(&ev.id) && fields && prev && pow
}
/// Tm.ev.json: the wire object, in 0.14's key order. Only these keys exist.
pub fn ev_json(ev: &Ev) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("e".into(), json!(ev.e)); o.insert("day".into(), json!(ev.day)); o.insert("id".into(), json!(ev.id)); o.insert("share".into(), json!(true)); o.insert("ver".into(), json!(ev.ver));
    if ev.e == "update" && !ev.prev.is_empty() { o.insert("prev".into(), json!(ev.prev)); }
    o.insert("ch".into(), json!(ev.ch)); o.insert("os".into(), json!(ev.os)); o.insert("arch".into(), json!(ev.arch));
    if ev.e == "install" && !ev.pow.is_empty() { o.insert("pow".into(), json!(ev.pow)); }
    Value::Object(o)
}
/// The envelope: `{"v":1,"ts":…,"nonce":…,"events":[…]}`.
pub fn body(ts: u64, nonce: &str, events: &[Value]) -> String {
    let nonce = if hexn(nonce, 16) { nonce } else { "0000000000000000" };
    json!({"v": 1, "ts": ts, "nonce": nonce, "events": events}).to_string()
}
/// Tm.batch: the queue's first clean events that fit one request (50 events, 16 KiB).
pub fn batch(xs: &[Ev]) -> Vec<Ev> {
    let (mut out, mut bytes) = (vec![], 0usize);
    for x in xs.iter().filter(|x| clean(x)) {
        let n = ev_json(x).to_string().len() + 1;
        if out.len() >= MAX_BATCH || bytes + n > MAX_BYTES { break }
        bytes += n; out.push(x.clone());
    }
    out
}
/// The request's headers (the key itself never leaves this process: only its HMAC).
pub fn headers(kid: &str, key: &str, ts: u64, body: &str) -> Vec<String> {
    vec!["Content-Type: application/json".into(), format!("X-Backplane-Key: {kid}"), format!("X-Backplane-Timestamp: {ts}"), format!("X-Backplane-Signature: {}", crate::net::sign::header(key, ts, body))]
}
/// Tm.http.drop: only the acknowledgement and explicit permanent rejections consume a batch.
pub fn http_drop(status: u16) -> bool { matches!(status, 202 | 400 | 401 | 403 | 409 | 413) }

fn key(ev: &Ev) -> String { match ev.e.as_str() { "active" => format!("active:{}:{}", ev.id, ev.day), "update" => format!("update:{}:{}", ev.id, ev.ver), e => format!("{e}:{}", ev.id) } }
/// The queue kept: identified, released events of this id within the week, de-duplicated, at most 200 (the oldest activity goes first).
fn tidy(q: Vec<Ev>, id: &str, now: u64) -> Vec<Ev> {
    let mut seen = std::collections::HashSet::new();
    let mut q: Vec<Ev> = q.into_iter().filter(|e| e.share && !e.held && e.id == id && now.saturating_sub(e.made) <= WEEK).filter(|e| seen.insert(key(e))).collect();
    while q.len() > MAX_QUEUE { let i = q.iter().position(|e| e.e == "active").unwrap_or(0); q.remove(i); }
    q
}
fn ev(e: &str, prev: &str, v: &View, id: &str, day: &str) -> Ev {
    Ev { e: e.into(), day: day.into(), share: true, id: id.into(), ver: v.ver.clone(), prev: prev.into(), ch: v.ch.clone(), os: v.os.clone(), arch: v.arch.clone(), pow: String::new(), made: v.now, held: false }
}
/// Tm.plan (sending allowed): a new id when there is none or the reset marker moved (Reset id rotates identity only: the old id's queue
/// goes, a fresh install registers the new one), an update when the version changed, and one active event for a day with a UI action.
pub fn plan(st: &Tm, v: &View, fresh_id: &str, action_day: &str) -> Tm {
    let rotate = !uuid_ok(&st.id) || st.reset != v.reset;
    let id = if rotate { fresh_id.to_string() } else { st.id.clone() };
    let mut q = if rotate { vec![] } else { st.queue.clone() };
    if rotate { q.push(ev("install", "", v, &id, &v.day)) }
    else if !st.ver.is_empty() && st.ver != v.ver { q.push(ev("update", if version_ok(&st.ver) { &st.ver } else { "" }, v, &id, &v.day)) }
    let today = if rotate { String::new() } else { st.today.clone() };
    let fresh = day_ok(action_day) && action_day != today;
    if fresh { q.push(ev("active", "", v, &id, action_day)) }
    Tm { queue: tidy(q, &id, v.now), id, ver: v.ver.clone(), today: if fresh { action_day.into() } else { today }, share: true, reset: v.reset.clone(), next: st.next, busy: 0, seen: true }
}
/// The events that may go now: within the week, an install only with its proof of work.
pub fn ready(q: &[Ev], now: u64) -> Vec<Ev> { q.iter().filter(|e| now.saturating_sub(e.made) <= WEEK && !e.held && (e.e != "install" || !e.pow.is_empty())).cloned().collect() }
pub fn needs_pow(q: &[Ev]) -> bool { q.iter().any(|e| e.e == "install" && e.pow.is_empty()) }
/// Tm.sent: a consumed batch leaves the queue; anything else waits an hour, or longer when the receiver says so.
pub fn sent(st: &Tm, evs: &[Ev], status: u16, retry_after: u64, now: u64) -> Tm {
    let mut s = st.clone();
    if http_drop(status) { s.queue.retain(|e| !evs.contains(e)) } else { s.next = now + retry_after.max(3600) }
    s
}
/// Tm.pow.ok: SHA-256 of `bp-pow1:<id>:<pow>` starts with `bits` zero bits.
pub fn pow_ok(id: &str, pow: &str, bits: u32) -> bool {
    let h = Sha256::digest(format!("bp-pow1:{id}:{pow}").as_bytes());
    let lead = h.iter().take_while(|b| **b == 0).count() as u32 * 8 + h.iter().find(|b| **b != 0).map_or(0, |b| b.leading_zeros());
    lead >= bits
}
/// Tf.pow: the first hex counter that proves `bits` of work for this id ("" if none in 2^26 tries).
pub fn pow(id: &str, bits: u32) -> String { if !uuid_ok(id) { return String::new() } (0u64..1 << 26).map(|i| format!("{i:x}")).find(|p| pow_ok(id, p, bits)).unwrap_or_default() }
/// The UUIDv4 of 16 random bytes.
pub fn uuid(b: &[u8; 16]) -> String {
    let mut b = *b; b[6] = (b[6] & 0x0f) | 0x40; b[8] = (b[8] & 0x3f) | 0x80; let h = hex::encode(b);
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}
/// A Retry-After in seconds from dumped response headers (0 when none).
pub fn retry_after(headers: &str) -> u64 { headers.lines().find_map(|l| { let (k, v) = l.split_once(':')?; k.trim().eq_ignore_ascii_case("retry-after").then(|| v.trim().parse().ok()).flatten() }).unwrap_or(0) }

// ---- state on disk ----

pub fn dir(home: &Path) -> PathBuf { home.join("telemetry") }
fn private_dir(home: &Path) -> std::io::Result<PathBuf> {
    let d = dir(home); std::fs::create_dir_all(&d)?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700))?; }
    Ok(d)
}
/// An atomic, private write (0600, then rename).
pub fn write_private(home: &Path, name: &str, text: &str) -> std::io::Result<()> {
    let d = private_dir(home)?; let tmp = d.join(format!(".{name}.tmp")); let _ = std::fs::remove_file(&tmp);
    let mut o = std::fs::OpenOptions::new(); o.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    use std::io::Write; o.open(&tmp)?.write_all(text.as_bytes())?;
    crate::plat::fs::rename(&tmp, &d.join(name))
}
pub fn read_state(home: &Path) -> Tm { std::fs::read_to_string(dir(home).join("state.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default() }
/// Off or not yet allowed: the id, the outbox, the acknowledgements and the counters go (no backfill later).
pub fn forget(home: &Path) { for f in ["state.json", "usage.json", "counts.json", "request.curl", "request.body", "response.headers"] { let _ = std::fs::remove_file(dir(home).join(f)); } }

// ---- the worker ----

/// Whether counting and sending are allowed now, as the worker last found it (the usage observer reads it; a privacy change clears it at once).
pub static ALLOWED: AtomicBool = AtomicBool::new(false);
/// The last successful authenticated allowlisted UI action (unix seconds), while allowed; only its UTC day is ever used.
static HUMAN: AtomicU64 = AtomicU64::new(0);
fn notify() -> &'static tokio::sync::Notify { static N: OnceLock<tokio::sync::Notify> = OnceLock::new(); N.get_or_init(tokio::sync::Notify::new) }
/// Ask the worker to look again now.
pub fn wake() { notify().notify_one() }
/// Methods that are a person's own UI action (never a query, a timer, an agent or provider method).
pub fn human_method(m: &str) -> bool { matches!(m, "turn.start" | "thread.create" | "bots.say" | "bots.post" | "project.new" | "project.add" | "thread.fork") }
/// Hub: a successful authenticated allowlisted UI action happened.
pub fn human() { if ALLOWED.load(Relaxed) { let first = HUMAN.swap(now_ms() / 1000, Relaxed) == 0; if first { wake() } } }
/// A privacy setting changed: no evidence or counting carries across it, until the worker has looked again. An Off (or a reset) also
/// forgets the id and the outbox at once, so an Off and an On between two passes still start a new identity with fresh counters.
pub fn privacy_changed(home: &Path, off_or_reset: bool) {
    ALLOWED.store(false, Relaxed); HUMAN.store(0, Relaxed);
    if off_or_reset { crate::settings::telemetry_usage::clear(); forget(home) }
    wake()
}
/// Whether a committed setting is an Off or a reset.
pub fn off_or_reset(e: &Value) -> bool {
    let (k, v) = (e["key"].as_str().unwrap_or(""), e["value"].as_str().unwrap_or(""));
    (k == "telemetry.share" && v == "off") || (k == "telemetry.usage.asked" && v == "0") || (k == "onboard.usage" && v == "off") || k == "telemetry.reset" || k == "telemetry.usage.generation"
}

/// The privacy settings: any change to one invalidates what the worker holds.
pub fn privacy_key(k: &str) -> bool { matches!(k, "telemetry.share" | "telemetry.asked" | "telemetry.usage.asked" | "telemetry.usage.generation" | "telemetry.reset" | "telemetry.notice" | "onboard.usage") }

/// The hub's settings as the worker reads them: whether it may send, and the reset marker.
async fn snapshot(hub: &Arc<Hub>) -> (bool, String) {
    let t = crate::settings::telemetry::state(hub).await;
    let r = hub.setting("telemetry.reset").await.unwrap_or_default();
    (crate::settings::telemetry::may_send(&t.share, &t.usage_asked, &t.onboard_usage, &t.notice), format!("{}:{}", if r.is_empty() { "0" } else { &r }, t.generation))
}

/// One HTTP POST through curl: the URL, headers and body go in a private config and body file, never on the command line; a user's
/// .curlrc is ignored (`--disable` first). Answers (status, Retry-After); 0 when there was no answer.
async fn post(home: &Path, c: &Cfg, body: &str, ts: u64) -> (u16, u64) {
    let d = dir(home); let (cf, bf, hf) = (d.join("request.curl"), d.join("request.body"), d.join("response.headers"));
    let q = |p: &Path| p.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
    let mut conf = format!("url = \"{}\"\nrequest = \"POST\"\n", c.url);
    for h in headers(&c.kid, &c.key, ts, body) { conf.push_str(&format!("header = \"{h}\"\n")); }
    conf.push_str(&format!("data-binary = \"@{}\"\noutput = \"{}\"\ndump-header = \"{}\"\nwrite-out = \"%{{http_code}}\"\n", q(&bf), if cfg!(windows) { "NUL" } else { "/dev/null" }, q(&hf)));
    if write_private(home, "request.body", body).is_err() || write_private(home, "request.curl", &conf).is_err() { forget_request(&d); return (0, 0) }
    let out = crate::proc::tokio_cmd(crate::proc::Kind::Net, "curl").args(["--disable", "--silent", "--max-time", "10", "--max-filesize", "16384", "--proto", if c.url.starts_with("https://") { "=https" } else { "=http" }, "-K"]).arg(&cf)
        .stdin(std::process::Stdio::null()).kill_on_drop(true).output().await;
    let status = out.ok().and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u16>().ok()).unwrap_or(0);
    let ra = retry_after(&std::fs::read_to_string(&hf).unwrap_or_default());
    forget_request(&d);
    (status, ra)
}
fn forget_request(d: &Path) { for f in ["request.curl", "request.body", "response.headers"] { let _ = std::fs::remove_file(d.join(f)); } }
fn random_hex(n: usize) -> String { let mut b = vec![0u8; n]; let _ = crate::plat::fs::random(&mut b); hex::encode(b) }

/// One pass: plan, prove, save, send what is due, then the usage snapshots. `c` is the configuration (tests pass a loopback one).
pub async fn job(hub: &Arc<Hub>, c: &Cfg) {
    let home = hub.cfg.home.clone();
    let (allowed, reset) = snapshot(hub).await;
    ALLOWED.store(allowed, Relaxed);
    if !allowed { HUMAN.store(0, Relaxed); crate::settings::telemetry_usage::clear(); forget(&home); hub.set_info("telemetry.id", "").await; return }
    let now = now_ms() / 1000;
    let h = HUMAN.swap(0, Relaxed); let action_day = if h > 0 { day_of(h) } else { String::new() };
    let v = View { reset: reset.clone(), ver: hub.cfg.version.clone(), ch: c.channel.clone(), os: hub.cfg.os.clone(), arch: hub.cfg.arch.clone(), day: day_of(now), now };
    let mut b = [0u8; 16]; let _ = crate::plat::fs::random(&mut b);
    let mut st = plan(&read_state(&home), &v, &uuid(&b), &action_day);
    if needs_pow(&st.queue) {
        let id = st.id.clone(); let p = tokio::task::spawn_blocking(move || pow(&id, POW_BITS)).await.unwrap_or_default();
        if pow_ok(&st.id, &p, POW_BITS) { for e in st.queue.iter_mut().filter(|e| e.e == "install" && e.pow.is_empty()) { e.pow = p.clone() } }
    }
    // the counters belong to this id: a reset (or the first id) starts them again
    crate::settings::telemetry_usage::scope(&home, &reset);
    let _ = write_private(&home, "state.json", &serde_json::to_string(&st).unwrap_or_default());
    hub.set_info("telemetry.id", &st.id).await; hub.set_info("telemetry.channel", &c.channel).await;
    // a choice made meanwhile (Off, Reset) wins: asked again right before any request
    let still = |hub: Arc<Hub>, reset: String| async move { let (a, r) = snapshot(&hub).await; a && r == reset };
    let evs = batch(&ready(&st.queue, now));
    if !evs.is_empty() && now >= st.next && still(hub.clone(), reset.clone()).await {
        let wire: Vec<Value> = evs.iter().map(ev_json).collect();
        let (status, ra) = post(&home, c, &body(now, &random_hex(8), &wire), now).await;
        st = sent(&st, &evs, status, ra, now);
        let _ = write_private(&home, "state.json", &serde_json::to_string(&st).unwrap_or_default());
    }
    // usage: its own request, after registration, so an older receiver's usage rejection never costs the events above
    let all = crate::settings::telemetry_usage::events(&st.id, &v);
    let acked: Vec<Value> = std::fs::read_to_string(dir(&home).join("usage.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let pending: Vec<Value> = all.iter().filter(|e| !acked.contains(e)).cloned().collect();
    let now = now_ms() / 1000;
    if !pending.is_empty() && uuid_ok(&st.id) && now >= st.next && !st.queue.iter().any(|e| e.e == "install") && still(hub.clone(), reset.clone()).await {
        let (status, ra) = post(&home, c, &body(now, &random_hex(8), &pending), now).await;
        if http_drop(status) { let _ = write_private(&home, "usage.json", &Value::Array(all).to_string()); }
        else { st = sent(&st, &[], status, ra, now); let _ = write_private(&home, "state.json", &serde_json::to_string(&st).unwrap_or_default()); }
    }
}

/// Counts committed events (settings/telemetry_usage.rs) and notices privacy changes.
struct Watch;
impl Observer for Watch {
    fn committed(&self, hub: &Arc<Hub>, events: &[Value], _first: u32) {
        let privacy: Vec<&Value> = events.iter().filter(|e| e["$"] == "SettingSet" && privacy_key(e["key"].as_str().unwrap_or(""))).collect();
        if !privacy.is_empty() { privacy_changed(&hub.cfg.home, privacy.iter().any(|e| off_or_reset(e))) }
        if ALLOWED.load(Relaxed) { crate::settings::telemetry_usage::count(events, now_ms() / 1000) }
    }
}

/// Counting and privacy changes follow the log from here on.
pub fn observe(hub: &Arc<Hub>) { hub.register_observer(Arc::new(Watch)) }
/// Starts the worker (inside the runtime): a pass a minute, and at once when woken. Nothing at all runs in a build that cannot send, or on a copy.
pub fn start(hub: &Arc<Hub>) {
    let Some(c) = cfg() else { return };
    if crate::copy::on() { return } // a hub on a copy of a log sends nothing and writes no telemetry state
    observe(hub);
    let hub = hub.clone();
    tokio::spawn(async move {
        let every = std::time::Duration::from_millis(std::env::var("BACKPLANE_TELEMETRY_TICK_MS").ok().filter(|_| std::env::var("BACKPLANE_TELEMETRY_TEST").as_deref() == Ok("1")).and_then(|v| v.parse().ok()).unwrap_or(60_000));
        loop {
            let _ = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(job(&hub, c))).await;
            let _ = tokio::time::timeout(every, notify().notified()).await;
        }
    });
}

#[cfg(test)]
#[path = "telemetry_send_tests.rs"]
mod tests;
