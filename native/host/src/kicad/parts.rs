//! Parts lookups for the BOM panel (docs/MANAGEMENT.md, "Stock"): each BOM line's part in the fab's stock, through the person's `pcbparts`
//! MCP server (its tools jlc_get_part, jlc_stock_check, jlc_find_alternatives; Settings > MCP servers can turn it off: `mcp.pcbparts`).
//! A line is looked up by its fab part number when it has one, else by its MPN. Answers are cached (6 h; a failure 15 min) and asked one
//! at a time, at most one call every GAP and PER_HOUR an hour, by one worker: never per keystroke, never on the caller's time. The BOM
//! shows "checking" meanwhile. A 429 pauses every lookup for the retry_after it names (doubling on each one in a row, to 30 min; the
//! key goes back to the queue's head, never retried in a loop) and the BOM says the check is paused. Answers outlive a restart: a part's
//! last history sample younger than FRESH is used as its answer. Each fresh answer is also a sample in the part's history (kicad/parthist.rs).
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet, VecDeque}, future::Future, pin::Pin, sync::{atomic::{AtomicU64, Ordering}, Arc, Mutex, OnceLock, Weak}, time::{Duration, Instant}};

pub const FRESH: Duration = Duration::from_secs(6 * 3600);
pub const RETRY: Duration = Duration::from_secs(15 * 60);
pub const GAP: Duration = Duration::from_secs(3);
pub const PER_HOUR: usize = 300;
/// A part's identity (its MPN's fab number) is kept this long: a refresh then asks for the fab number's stock only, one call.
pub const KNOWN: u64 = 7 * 86400;
/// A part not wanted by any shown BOM for this long is not refreshed any more.
const FORGET: Duration = Duration::from_secs(24 * 3600);

/// What the fab's stock says of one part.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Quote {
    /// the fab's part number and the maker's, as the fab lists them
    pub fab: String, pub model: String, pub mfr: String,
    /// units in stock (-1 unknown); basic | preferred | extended | ""
    pub stock: i64, pub class: String,
    /// (quantity from, unit price in USD), ascending
    pub breaks: Vec<(u64, f64)>,
    /// unix seconds of the answer; why it failed ("" when it did not); not found in the fab's stock
    pub at: u64, #[serde(default)] pub err: String, #[serde(default)] pub missing: bool,
}

/// The key a line is looked up by: `fab:<number>` or `mpn:<number>`; None for a line with neither.
pub fn key_of(fab: &str, mpn: &str) -> Option<String> {
    let (f, m) = (fab.trim(), mpn.trim());
    if !f.is_empty() { Some(format!("fab:{}", f.to_ascii_uppercase())) } else if !m.is_empty() { Some(format!("mpn:{m}")) } else { None }
}
/// "1000+" or 1000 as a number.
fn qty_of(v: &Value) -> Option<u64> { v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().trim_end_matches('+').replace(',', "").parse().ok())) }
/// A part's price breaks: `prices` [{qty: "1+", price}] or `price_breaks` [{qty, price}], else its one `price` at 1.
pub fn breaks_of(it: &Value) -> Vec<(u64, f64)> {
    let mut v: Vec<(u64, f64)> = it["prices"].as_array().or(it["price_breaks"].as_array()).into_iter().flatten()
        .filter_map(|b| Some((qty_of(&b["qty"])?, b["price"].as_f64()?))).collect();
    if v.is_empty() { if let Some(p) = it["price"].as_f64() { v.push((1, p)) } }
    v.sort_by_key(|b| b.0); v.dedup_by_key(|b| b.0); v
}
/// The unit price when buying `need`: the break for the largest quantity not above it (the first break when `need` is below them all).
pub fn unit_price(breaks: &[(u64, f64)], need: u64) -> Option<f64> { breaks.iter().rev().find(|b| b.0 <= need.max(1)).or(breaks.first()).map(|b| b.1) }
/// in | low | out | unknown for `need` units: out when the fab has fewer than needed, low under five builds' worth.
pub fn status(stock: i64, need: u64) -> &'static str {
    if stock < 0 { return "unknown" }
    let (s, n) = (stock as u64, need.max(1));
    if s < n { "out" } else if s < n.saturating_mul(5) { "low" } else { "in" }
}
fn class_of(it: &Value) -> String {
    match it["library_type"].as_str().unwrap_or("") { "basic" => "basic".into(), _ if it["preferred"] == true => "preferred".into(), "" => String::new(), t => t.to_string() }
}
fn quote_of(it: &Value, at: u64) -> Quote {
    Quote { fab: it["lcsc"].as_str().unwrap_or("").into(), model: it["model"].as_str().unwrap_or("").into(), mfr: it["manufacturer"].as_str().unwrap_or("").into(),
        stock: it["stock"].as_i64().unwrap_or(-1), class: class_of(it), breaks: breaks_of(it), at, err: String::new(), missing: false }
}
/// Two part numbers that are the same part ("RC0603FR-0710KL" and "rc0603fr 0710kl").
pub fn same_mpn(a: &str, b: &str) -> bool { let n = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase(); !a.is_empty() && n(a) == n(b) }
/// The part an answer names: an object with `lcsc`, or the `results` entry whose model is `mpn` (any results' first for a fab number).
pub fn pick(v: &Value, mpn: &str, at: u64) -> Option<Quote> {
    if v["lcsc"].is_string() { return Some(quote_of(v, at)) }
    let rs = v["results"].as_array()?;
    let it = if mpn.is_empty() { rs.first() } else { rs.iter().find(|r| same_mpn(r["model"].as_str().unwrap_or(""), mpn)) }?;
    Some(quote_of(it, at))
}
/// A line's stock as the BOM publishes it: {status, stock, class, price (at `need`), need, fab, at}, or {status: "checking"}.
pub fn line_stock(q: Option<&Quote>, need: u64) -> Value {
    match q {
        None => json!({"status": "checking"}),
        Some(q) if !q.err.is_empty() => json!({"status": "unknown", "why": q.err, "at": q.at}),
        Some(q) if q.missing => json!({"status": "unknown", "why": "not in the fab's stock", "at": q.at}),
        Some(q) => json!({"status": status(q.stock, need), "stock": q.stock, "class": q.class, "price": unit_price(&q.breaks, need), "need": need, "fab": q.fab, "at": q.at}),
    }
}

/// One call at a time, at most one every `gap` and `per_hour` an hour.
pub struct Limiter { pub gap: Duration, pub per_hour: usize, calls: VecDeque<Instant> }
impl Limiter {
    pub fn new(gap: Duration, per_hour: usize) -> Self { Limiter { gap, per_hour, calls: VecDeque::new() } }
    /// How long to wait before the next call at `now` (zero: go).
    pub fn wait(&mut self, now: Instant) -> Duration {
        while self.calls.front().map_or(false, |t| now.duration_since(*t) >= Duration::from_secs(3600)) { self.calls.pop_front(); }
        let gap = self.calls.back().map_or(Duration::ZERO, |t| self.gap.saturating_sub(now.duration_since(*t)));
        let hour = if self.calls.len() >= self.per_hour { Duration::from_secs(3600).saturating_sub(now.duration_since(self.calls[0])) } else { Duration::ZERO };
        gap.max(hour)
    }
    pub fn took(&mut self, now: Instant) { self.calls.push_back(now) }
}

/// "2026-11-03" as the unix second its UTC day starts (Howard Hinnant's days_from_civil).
pub fn day_at(d: &str) -> Option<u64> {
    let mut it = d.split('-'); let (y, m, dd): (i64, i64, i64) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    let y = if m <= 2 { y - 1 } else { y }; let era = y.div_euclid(400); let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + dd - 1; let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    u64::try_from((era * 146097 + doe - 719468) * 86400).ok()
}
/// The fab's recorded stock changes (jlc_stock_history's events) as history samples newer than `after`: each change, new or reappear with
/// the stock it left; a "gone" (missing from a scrape, not a stockout) is not a sample. `now` marks when they were read (`hist_at`).
pub fn stock_history(v: &Value, key: &str, fab: &str, after: u64, now: u64) -> Vec<Value> {
    let mut out: Vec<Value> = v["events"].as_array().into_iter().flatten().filter_map(|e| {
        let at = day_at(e["date"].as_str()?)?; let stock = e["stock_after"].as_i64()?;
        (at > after && e["type"].as_str() != Some("gone")).then(|| json!({"at": at, "key": key, "stock": stock, "fab": fab, "source": "fab_history", "hist_at": now}))
    }).collect();
    out.sort_by_key(|s| s["at"].as_u64().unwrap_or(0));
    out
}
/// Whether a part's fab history is due a read (none in the last week), and the newest day it already holds.
pub fn history_due(samples: &[Value], now: u64) -> (bool, u64) {
    let read = samples.iter().filter_map(|s| s["hist_at"].as_u64()).max();
    let newest = samples.iter().filter(|s| s["source"] == "fab_history" && s["stock"].is_i64()).filter_map(|s| s["at"].as_u64()).max().unwrap_or(0);
    (read.map_or(true, |r| now.saturating_sub(r) >= 7 * 86400), newest)
}

/// The pause a rate limit asks for: its retry_after (at least a minute), doubled for each limit in a row, at most 30 minutes.
#[derive(Default, Debug)]
pub struct Backoff { pub until: Option<Instant>, strikes: u32 }
impl Backoff {
    pub fn limited(&mut self, retry_after: u64, now: Instant) -> Duration {
        let d = Duration::from_secs(retry_after.max(60).saturating_mul(1 << self.strikes.min(5)).min(1800));
        self.strikes += 1; self.until = Some(now + d); d
    }
    pub fn ok(&mut self) { self.strikes = 0; self.until = None; }
    pub fn wait(&self, now: Instant) -> Duration { self.until.map_or(Duration::ZERO, |u| u.saturating_duration_since(now)) }
}
/// A lookup's error that is a rate limit: the seconds to wait (the server's retry_after, 60 when it names none).
pub fn limit_secs(err: &str) -> Option<u64> {
    if let Some(n) = crate::mcpx::health::retry_after(err) { return Some(n) }
    let l = err.to_ascii_lowercase();
    if !(l.contains("rate limit") || l.contains("429") || l.contains("too many requests")) { return None }
    Some(l.split("retry_after").nth(1).and_then(|t| t.trim_start_matches(|c: char| !c.is_ascii_digit()).split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()).unwrap_or(60))
}

/// The cache and the queue (pure: the worker drives it).
#[derive(Default)]
pub struct Book { pub quotes: HashMap<String, (Quote, Instant)>, queue: VecDeque<String>, queued: HashSet<String>, wanted: HashMap<String, Instant>, asking: HashSet<String> }
impl Book {
    /// A BOM shows these keys: each one not cached fresh is queued once.
    pub fn want(&mut self, keys: &[String], now: Instant) {
        for k in keys {
            self.wanted.insert(k.clone(), now);
            let stale = match self.quotes.get(k) { None => true, Some((q, at)) => now.duration_since(*at) >= if q.err.is_empty() { FRESH } else { RETRY } };
            if stale && !self.asking.contains(k) && self.queued.insert(k.clone()) { self.queue.push_back(k.clone()) }
        }
    }
    /// The schedule: every wanted key whose answer went stale is queued again; keys nobody showed for a day are dropped.
    pub fn sweep(&mut self, now: Instant) {
        self.wanted.retain(|_, t| now.duration_since(*t) < FORGET);
        let ks: Vec<String> = self.wanted.keys().cloned().collect();
        let held: Vec<(String, Instant)> = self.wanted.iter().map(|(k, t)| (k.clone(), *t)).collect();
        self.want(&ks, now);
        for (k, t) in held { self.wanted.insert(k, t); } // a sweep is not someone showing it
    }
    /// The next key to ask (it is being asked until `put`: a BOM shown meanwhile does not queue it again).
    pub fn next(&mut self) -> Option<String> { let k = self.queue.pop_front()?; self.queued.remove(&k); self.asking.insert(k.clone()); Some(k) }
    pub fn put(&mut self, k: &str, q: Quote, now: Instant) { self.asking.remove(k); self.quotes.insert(k.to_string(), (q, now)); }
    /// A key whose lookup was cut short (a rate limit): first in line again, its old answer kept.
    pub fn again(&mut self, k: &str) { self.asking.remove(k); if self.queued.insert(k.to_string()) { self.queue.push_front(k.to_string()) } }
    /// A key's answer from before (the history's last sample), taken when the book has none.
    pub fn seed(&mut self, k: &str, q: Quote, age: Duration, now: Instant) { if !self.quotes.contains_key(k) { self.quotes.insert(k.to_string(), (q, now.checked_sub(age).unwrap_or(now))); } }
    pub fn get(&self, k: &str) -> Option<&Quote> { self.quotes.get(k).map(|(q, _)| q) }
    pub fn pending(&self) -> usize { self.queue.len() }
}

type Call = Arc<dyn Fn(String, Value) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>> + Send + Sync>;
static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();
pub fn book() -> &'static Mutex<Book> { BOOK.get_or_init(Default::default) }
/// Bumped at every answer, so the BOM panel republishes.
static GEN: AtomicU64 = AtomicU64::new(0);
pub fn generation() -> u64 { GEN.load(Ordering::Relaxed) }
/// Lookups are paused by a rate limit until this unix second (0: not paused).
static PAUSED: AtomicU64 = AtomicU64::new(0);
pub fn paused() -> bool { PAUSED.load(Ordering::Relaxed) > super::ipc::now_s() }
/// Something the panels show from the parts history changed (a lifecycle reading).
pub fn bump() { GEN.fetch_add(1, Ordering::Relaxed); }
static WAKE: OnceLock<tokio::sync::Notify> = OnceLock::new();
fn wake() -> &'static tokio::sync::Notify { WAKE.get_or_init(tokio::sync::Notify::new) }

/// A key's last history sample as an answer (Quote, its age), None without one.
pub fn from_history(home: &std::path::Path, k: &str, now_s: u64) -> Option<(Quote, Duration)> {
    let s = super::parthist::read(home, k).into_iter().rev().find(|s| s["stock"].is_i64())?;
    let at = s["at"].as_u64()?;
    let breaks = s["breaks"].as_array().into_iter().flatten().filter_map(|b| Some((b[0].as_u64()?, b[1].as_f64()?))).collect();
    Some((Quote { fab: s["fab"].as_str().unwrap_or("").into(), model: s["model"].as_str().unwrap_or("").into(), mfr: String::new(), stock: s["stock"].as_i64().unwrap_or(-1), class: s["class"].as_str().unwrap_or("").into(), breaks, at, err: String::new(), missing: false }, Duration::from_secs(now_s.saturating_sub(at))))
}
/// Queue lookups for a BOM's keys (cheap; answers come later). A key the book has no answer for takes its history's last one first.
pub fn want(keys: &[String]) {
    if keys.is_empty() { return }
    let home = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade).map(|h| h.cfg.home.clone());
    let fresh: Vec<String> = { let b = book().lock().unwrap(); keys.iter().filter(|k| b.get(k).is_none()).cloned().collect() };
    let seeds: Vec<(String, Quote, Duration)> = match &home { Some(h) => fresh.iter().filter_map(|k| from_history(h, k, super::ipc::now_s()).map(|(q, a)| (k.clone(), q, a))).collect(), None => vec![] };
    let mut b = book().lock().unwrap(); let now = Instant::now();
    let seeded = !seeds.is_empty();
    for (k, q, a) in seeds { b.seed(&k, q, a, now) }
    b.want(keys, now); drop(b); wake().notify_one();
    // answers taken from the history are answers like any other: the BOM shows them (it was drawn as "checking" a moment ago)
    if seeded { GEN.fetch_add(1, Ordering::Relaxed); }
}
pub fn quote(k: &str) -> Option<Quote> { book().lock().unwrap().get(k).cloned() }

/// The pcbparts server's URL: `BACKPLANE_PCBPARTS_URL` (tests' stand-in), else the built-in one.
pub fn url() -> String { std::env::var("BACKPLANE_PCBPARTS_URL").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| crate::mcpx::gen::PCBPARTS.to_string()) }

/// One lookup: a fab number by jlc_get_part; an MPN by jlc_get_part (the in-stock catalogue), else jlc_stock_check (what is out of stock
/// too), then the part's price breaks by its fab number when the first answer had one price only.
pub async fn fetch(call: &Call, key: &str, at: u64, known: Option<&Quote>) -> Quote {
    let fail = |e: String| Quote { stock: -1, at, err: e, ..Default::default() };
    if let Some(f) = key.strip_prefix("fab:") {
        return match call("jlc_get_part".into(), json!({"lcsc": f})).await { Ok(v) => pick(&v, "", at).unwrap_or(Quote { stock: -1, at, missing: true, ..Default::default() }), Err(e) => fail(e) }
    }
    let mpn = key.strip_prefix("mpn:").unwrap_or(key);
    // a part found before (within KNOWN): its fab number's stock and prices, one call
    if let Some(k) = known.filter(|k| !k.fab.is_empty() && k.err.is_empty() && at.saturating_sub(k.at) < KNOWN) {
        match call("jlc_get_part".into(), json!({"lcsc": k.fab})).await { Ok(v) => if let Some(q) = pick(&v, "", at) { return q }, Err(e) => return fail(e) }
    }
    let mut q = match call("jlc_get_part".into(), json!({"mpn": mpn})).await { Ok(v) => pick(&v, mpn, at), Err(e) => return fail(e) };
    if q.is_none() {
        q = match call("jlc_stock_check".into(), json!({"query": mpn, "min_stock": 0, "limit": 10})).await { Ok(v) => pick(&v, mpn, at), Err(e) => return fail(e) };
    }
    let Some(mut q) = q else { return Quote { stock: -1, at, missing: true, ..Default::default() } };
    if q.breaks.len() <= 1 && !q.fab.is_empty() {
        if let Ok(v) = call("jlc_get_part".into(), json!({"lcsc": q.fab})).await { if let Some(full) = pick(&v, "", at) { if full.breaks.len() > q.breaks.len() { q.breaks = full.breaks } if full.stock >= 0 { q.stock = full.stock } } }
    }
    q
}

/// `mgmt.bom.alts {project, fab}`: parts of the fab's stock that can stand in for a line's part (jlc_find_alternatives), asked now.
pub async fn alternatives(hub: &Arc<Hub>, fab: &str, need: u64) -> Result<Value, String> {
    if !enabled(hub).await { return Err("the parts search (pcbparts) is turned off in Settings > MCP servers".into()) }
    let v = crate::mcpx::health::call_tool(&url(), "jlc_find_alternatives", &json!({"lcsc": fab, "limit": 8, "board_quantity": need.max(1)})).await?;
    let list = v["alternatives"].as_array().or(v["results"].as_array()).or(v["verified_alternatives"].as_array()).cloned().unwrap_or_default();
    Ok(json!({"fab": fab, "alternatives": list.iter().take(8).map(|a| { let q = quote_of(a, 0); json!({"fab": q.fab, "model": q.model, "mfr": q.mfr, "stock": q.stock, "class": q.class, "price": unit_price(&q.breaks, need), "package": a["package"]}) }).collect::<Vec<_>>()}))
}

async fn enabled(hub: &Arc<Hub>) -> bool { hub.setting(crate::mcpx::svc::PCBPARTS_KEY).await.as_deref() != Some("off") }

static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);
pub fn start(hub: &Arc<Hub>) {
    *HUB.lock().unwrap() = Some(Arc::downgrade(hub));
    if crate::copy::on() { return }
    let call: Call = Arc::new(|tool: String, args: Value| Box::pin(async move { crate::mcpx::health::call_tool(&url(), &tool, &args).await }));
    tokio::spawn(async move {
        // BACKPLANE_PARTS_GAP_MS: the tests' stand-in server takes calls closer together
        let gap = std::env::var("BACKPLANE_PARTS_GAP_MS").ok().and_then(|s| s.parse().ok()).map(Duration::from_millis).unwrap_or(GAP);
        let mut lim = Limiter::new(gap, PER_HOUR);
        let mut back = Backoff::default();
        let mut swept = Instant::now();
        loop {
            if swept.elapsed() > Duration::from_secs(600) { book().lock().unwrap().sweep(Instant::now()); swept = Instant::now(); }
            // a rate limit: nothing is asked until its pause is over
            let w = back.wait(Instant::now()); if !w.is_zero() { tokio::time::sleep(w).await; }
            if PAUSED.load(Ordering::Relaxed) != 0 { PAUSED.store(0, Ordering::Relaxed); GEN.fetch_add(1, Ordering::Relaxed); }
            let next = book().lock().unwrap().next();
            let Some(k) = next else { let _ = tokio::time::timeout(Duration::from_secs(60), wake().notified()).await; continue };
            let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) else { return };
            if !enabled(&hub).await { book().lock().unwrap().put(&k, Quote { stock: -1, at: super::ipc::now_s(), err: "the parts search is turned off".into(), ..Default::default() }, Instant::now()); GEN.fetch_add(1, Ordering::Relaxed); continue }
            let w = lim.wait(Instant::now()); if !w.is_zero() { tokio::time::sleep(w).await; }
            lim.took(Instant::now());
            let known = quote(&k);
            let q = fetch(&call, &k, super::ipc::now_s(), known.as_ref()).await;
            if let Some(secs) = limit_secs(&q.err) {
                let d = back.limited(secs, Instant::now());
                PAUSED.store(super::ipc::now_s() + d.as_secs(), Ordering::Relaxed);
                book().lock().unwrap().again(&k);
                GEN.fetch_add(1, Ordering::Relaxed);
                continue
            }
            back.ok();
            if q.err.is_empty() { super::parthist::record(&hub.cfg.home, &k, &q); }
            let fab = q.fab.clone();
            book().lock().unwrap().put(&k, q, Instant::now());
            GEN.fetch_add(1, Ordering::Relaxed);
            // the fab's own recorded stock changes, once a week a part, through the same limits: the graphs have a past from the first day
            if !fab.is_empty() {
                let now = super::ipc::now_s();
                let (due, newest) = history_due(&super::parthist::read(&hub.cfg.home, &k), now);
                if due {
                    let w = lim.wait(Instant::now()); if !w.is_zero() { tokio::time::sleep(w).await; }
                    lim.took(Instant::now());
                    match call("jlc_stock_history".into(), json!({"lcsc": fab, "days": 366})).await {
                        Ok(v) => {
                            let ss = stock_history(&v, &k, &fab, newest, now);
                            // read, even with nothing new: a marker so the week is counted
                            if ss.is_empty() { super::parthist::append(&hub.cfg.home, &k, &json!({"at": now, "key": k, "source": "fab_history", "hist_at": now})); }
                            for s in &ss { super::parthist::append(&hub.cfg.home, &k, s); }
                            GEN.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => if let Some(secs) = limit_secs(&e) { let d = back.limited(secs, Instant::now()); PAUSED.store(super::ipc::now_s() + d.as_secs(), Ordering::Relaxed); },
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn breaks_prices_and_status() {
        let it = json!({"lcsc": "C71629", "model": "GRM155R71C104KA88D", "stock": 1060770, "price": 0.0034, "library_type": "extended", "preferred": false,
            "prices": [{"qty": "1+", "price": 0.0034}, {"qty": "1000+", "price": 0.0029}, {"qty": "3000+", "price": 0.0026}]});
        let q = pick(&it, "", 9).unwrap();
        assert_eq!((q.fab.as_str(), q.class.as_str(), q.stock, q.breaks.len(), q.at), ("C71629", "extended", 1060770, 3, 9));
        assert_eq!(unit_price(&q.breaks, 5), Some(0.0034)); assert_eq!(unit_price(&q.breaks, 1000), Some(0.0029)); assert_eq!(unit_price(&q.breaks, 99999), Some(0.0026)); assert_eq!(unit_price(&[], 5), None);
        assert_eq!(breaks_of(&json!({"price_breaks": [{"qty": 10, "price": 0.3}, {"qty": 1, "price": 0.45}]})), vec![(1, 0.45), (10, 0.3)]);
        assert_eq!(breaks_of(&json!({"price": 4.12})), vec![(1, 4.12)]);
        assert_eq!(status(0, 5), "out"); assert_eq!(status(4, 5), "out"); assert_eq!(status(20, 5), "low"); assert_eq!(status(25, 5), "in"); assert_eq!(status(-1, 5), "unknown");
        assert_eq!(class_of(&json!({"library_type": "basic"})), "basic"); assert_eq!(class_of(&json!({"library_type": "extended", "preferred": true})), "preferred");
    }
    #[test] fn the_answer_names_the_part_asked_for() {
        let v = json!({"results": [{"lcsc": "C9900285923", "model": "NRF54L15-QFAA-R7", "stock": 0}, {"lcsc": "C42458750", "model": "NRF54L15-QFAA-R", "stock": 0, "price": 4.1267}]});
        assert_eq!(pick(&v, "nRF54L15-QFAA-R", 0).unwrap().fab, "C42458750");
        assert!(pick(&v, "nRF52840", 0).is_none()); assert!(pick(&json!({"results": []}), "", 0).is_none());
        assert!(same_mpn("RC0603FR-0710KL", "rc0603fr 0710kl")); assert!(!same_mpn("", ""));
        assert_eq!(key_of(" c123 ", "X"), Some("fab:C123".into())); assert_eq!(key_of("", "LM358"), Some("mpn:LM358".into())); assert_eq!(key_of("", " "), None);
    }
    #[test] fn a_line_says_what_its_stock_is() {
        assert_eq!(line_stock(None, 4)["status"], "checking");
        let q = Quote { fab: "C1".into(), stock: 12, class: "basic".into(), breaks: vec![(1, 0.5), (10, 0.4)], at: 7, ..Default::default() };
        assert_eq!(line_stock(Some(&q), 4), json!({"status": "low", "stock": 12, "class": "basic", "price": 0.5, "need": 4, "fab": "C1", "at": 7}));
        assert_eq!(line_stock(Some(&Quote { err: "quota".into(), ..Default::default() }), 1)["status"], "unknown");
        assert_eq!(line_stock(Some(&Quote { missing: true, ..Default::default() }), 1)["why"], "not in the fab's stock");
    }
    #[test] fn a_rate_limit_pauses_the_lookups_and_backs_off() {
        let t0 = Instant::now(); let mut b = Backoff::default();
        assert_eq!(b.wait(t0), Duration::ZERO);
        assert_eq!(b.limited(60, t0), Duration::from_secs(60), "the server's retry_after");
        assert_eq!(b.wait(t0 + Duration::from_secs(20)), Duration::from_secs(40));
        assert_eq!(b.limited(60, t0), Duration::from_secs(120), "a second limit in a row waits twice as long");
        assert_eq!(b.limited(5, t0), Duration::from_secs(240), "never under a minute, doubled");
        for _ in 0..6 { b.limited(60, t0); } assert_eq!(b.limited(60, t0), Duration::from_secs(1800), "at most 30 minutes");
        b.ok(); assert_eq!(b.wait(t0), Duration::ZERO); assert_eq!(b.limited(60, t0), Duration::from_secs(60), "an answer starts it over");
        assert_eq!(limit_secs("429 retry_after=90"), Some(90)); assert_eq!(limit_secs("Rate limit exceeded, retry_after 60"), Some(60));
        assert_eq!(limit_secs("Too Many Requests"), Some(60)); assert_eq!(limit_secs("it answered HTTP 500"), None); assert_eq!(limit_secs(""), None);
        // the key whose lookup was cut short goes first again, its old answer kept
        let mut k = Book::default(); k.want(&["mpn:A".into(), "mpn:B".into()], t0); let a = k.next().unwrap(); k.again(&a); assert_eq!(k.next().as_deref(), Some("mpn:A"));
    }
    #[test] fn answers_outlive_a_restart_through_the_history() {
        let d = crate::plat::tmp::dir("partseed");
        super::super::parthist::record(&d, "mpn:A", &Quote { fab: "C9".into(), stock: 40, class: "basic".into(), breaks: vec![(1, 0.1)], at: 1000, ..Default::default() });
        let (q, age) = from_history(&d, "mpn:A", 1000 + 3600).unwrap(); assert_eq!((q.fab.as_str(), q.stock, q.breaks.len(), age), ("C9", 40, 1, Duration::from_secs(3600)));
        let t0 = Instant::now() + Duration::from_secs(7200); let mut b = Book::default();
        b.seed("mpn:A", q.clone(), age, t0); b.want(&["mpn:A".into()], t0); assert_eq!(b.pending(), 0, "an hour-old answer is fresh: not asked");
        let mut b = Book::default(); b.seed("mpn:A", q, FRESH + age, t0); b.want(&["mpn:A".into()], t0); assert_eq!(b.pending(), 1, "a stale one is asked again");
        assert!(from_history(&d, "mpn:Z", 0).is_none());
    }
    #[test] fn the_fabs_recorded_stock_changes_become_samples() {
        assert_eq!(day_at("1970-01-01"), Some(0)); assert_eq!(day_at("2026-11-03").unwrap() - day_at("2026-11-02").unwrap(), 86400); assert_eq!(day_at("2026-03-01").unwrap() - day_at("2026-02-28").unwrap(), 86400); assert_eq!(day_at("2000-02-29"), Some(951782400)); assert_eq!(day_at("x"), None);
        let v = json!({"events": [{"date": "2026-11-04", "type": "gone", "stock_after": null}, {"date": "2026-11-03", "type": "change", "stock_after": 164}, {"date": "2026-11-02", "type": "change", "stock_after": 210}, {"date": "2026-08-03", "type": "new", "stock_after": 28}]});
        let ss = stock_history(&v, "fab:C1", "C1", 0, 9);
        assert_eq!(ss.iter().map(|s| s["stock"].as_i64().unwrap()).collect::<Vec<_>>(), [28, 210, 164], "oldest first, the gone left out");
        assert_eq!(ss[0]["source"], "fab_history"); assert_eq!(ss[0]["hist_at"], 9);
        assert_eq!(stock_history(&v, "fab:C1", "C1", day_at("2026-11-02").unwrap(), 9).len(), 1, "only what is newer than what is held");
        assert_eq!(history_due(&[], 1000), (true, 0));
        assert_eq!(history_due(&ss, 9 + 86400), (false, day_at("2026-11-03").unwrap()), "read this week");
        assert!(history_due(&ss, 9 + 7 * 86400).0, "a week later");
    }
    #[test] fn calls_are_spaced_and_bounded_an_hour() {
        let t0 = Instant::now(); let mut l = Limiter::new(Duration::from_millis(1000), 3);
        assert_eq!(l.wait(t0), Duration::ZERO); l.took(t0);
        assert_eq!(l.wait(t0 + Duration::from_millis(400)), Duration::from_millis(600));
        l.took(t0 + Duration::from_secs(1)); l.took(t0 + Duration::from_secs(2));
        assert_eq!(l.wait(t0 + Duration::from_secs(3)), Duration::from_secs(3597), "the hour's calls are used up");
        assert_eq!(l.wait(t0 + Duration::from_secs(3600)), Duration::ZERO);
    }
    #[test] fn lookups_are_cached_queued_once_and_refreshed_when_stale() {
        let t0 = Instant::now(); let mut b = Book::default(); let k = |s: &str| s.to_string();
        b.want(&[k("mpn:A"), k("mpn:B"), k("mpn:A")], t0); b.want(&[k("mpn:A")], t0);
        assert_eq!(b.pending(), 2, "a key typed again is not asked twice");
        assert_eq!(b.next(), Some(k("mpn:A"))); b.want(&[k("mpn:A")], t0); assert_eq!(b.pending(), 1, "a key being asked is not queued again");
        b.put("mpn:A", Quote { stock: 5, ..Default::default() }, t0);
        b.want(&[k("mpn:A")], t0 + Duration::from_secs(60)); assert_eq!(b.pending(), 1, "a fresh answer is used");
        assert_eq!(b.next(), Some(k("mpn:B"))); b.put("mpn:B", Quote { err: "down".into(), ..Default::default() }, t0);
        b.sweep(t0 + RETRY); assert_eq!(b.pending(), 1, "a failure is asked again after RETRY"); assert_eq!(b.next(), Some(k("mpn:B"))); b.put("mpn:B", Quote { err: "down".into(), ..Default::default() }, t0 + RETRY);
        b.sweep(t0 + FRESH); let mut q = vec![b.next().unwrap(), b.next().unwrap()]; q.sort(); assert_eq!(q, [k("mpn:A"), k("mpn:B")], "an answer goes stale after FRESH");
        b.sweep(t0 + FRESH + Duration::from_secs(25 * 3600)); assert_eq!(b.wanted.len(), 0, "parts nobody shows are forgotten");
    }
}
