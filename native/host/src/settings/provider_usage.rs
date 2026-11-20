//! Remaining usage of the providers without named accounts: Antigravity, Grok, Muse, OpenCode, Pi and Oh My Pi (#1337). Each is read the way its own CLI offers,
//! with no model turn, and kept as the setting `allowance.<provider>`, the shape the Claude and Codex readings have (`provider`, `state`, `at`, `windows` of
//! `scope`, `label`, `used_percent`, `resets_at`), so Settings › Accounts, the model chip, `account_list` and failover read every provider the same way:
//! - Antigravity: `agy -p /quota` (agy 1.5): `<group>\t<Five Hour|Weekly> Limit Remaining\t<n>%\t<ISO reset>`, one line a window, a group a model family
//!   ("Gemini Models", "Claude and GPT models"); its windows name the models they bind (`models`). Only while turns run on the person's own agy: Backplane's
//!   pinned agent signs in on its own profile, which the CLI does not read.
//! - Grok: `_x.ai/billing` on `grok agent stdio` (ACP): `creditUsagePercent` of the current period (weekly), its end, the plan tier.
//! - Muse: `muse usage --json`: `allowance.window` (the 5-hour-class block) and `allowance.weekly`, used percent and reset in ms, the tier.
//! - Oh My Pi: `omp usage --json`: each signed-in provider account's limits (window, used fraction, reset).
//! - Pi and OpenCode report nothing: `state` "unreported", and the Accounts page says so beside what the hub saw itself (turns and tokens today, from
//!   the `tok.dv.<day>|<provider>` totals; the last rate limit a turn met, `allowance.seen.<provider>`, written by the failover driver).
//! Nothing here reads a token or a provider's files: only what the CLI prints, and no email or account id is kept from it.
use crate::{agents::proc::Cli, hub::Hub, proc::{self, Kind}};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

/// The providers this module reads, in the failover order's sense (Antigravity before Grok), then the rest.
pub const OTHERS: &[&str] = &["antigravity", "grok", "muse", "omp", "pi", "opencode"];
/// How often a reading is taken.
pub const EVERY: Duration = Duration::from_secs(300);
/// A reading the same as the last is written again only after this long: the log keeps the change, not every look.
pub const KEEP_S: u64 = 1800;
/// The CLI each provider runs.
pub fn cli_of(provider: &str) -> &str { if provider == "antigravity" { "agy" } else { provider } }
pub fn key(provider: &str) -> String { format!("allowance.{provider}") }
/// The last rate limit a turn of the provider met: `{scope, reason, reset, at}` (settings/driver.rs `on_limit`).
pub fn seen_key(provider: &str) -> String { format!("allowance.seen.{provider}") }
fn name(p: &str) -> String { super::fallback::provider_name(p) }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn pct(x: f64) -> f64 { (x.clamp(0.0, 100.0) * 10.0).round() / 10.0 }
/// Epoch seconds from a provider's reset: ISO text, or a number in seconds or milliseconds.
fn when_of(v: &Value) -> Option<u64> {
    if let Some(t) = v.as_str() { return super::accounts_io::parse_iso(t) }
    let n = v.as_f64()?; if n <= 0.0 { return None }
    Some(if n > 1e11 { (n / 1000.0) as u64 } else { n as u64 })
}

// ---- what each CLI prints, as windows ----

/// The words of a window's length: "5-hour", "Weekly", "Monthly", "Daily", else "<n>h".
fn span_label(minutes: u64) -> String { match minutes { 300 => "5-hour".into(), 10080 => "Weekly".into(), 1440 => "Daily".into(), m if (40_000..=45_000).contains(&m) => "Monthly".into(), m if m % 1440 == 0 && m > 0 => format!("{}-day", m / 1440), m if m % 60 == 0 && m > 0 => format!("{}-hour", m / 60), m => format!("{m}-minute") } }
fn span_scope(minutes: u64) -> String { match minutes { 300 => "five_hour".into(), 10080 => "seven_day".into(), 1440 => "day".into(), m if (40_000..=45_000).contains(&m) => "month".into(), m => format!("{m}m") } }

/// Antigravity's `/quota` lines. A group's words name the models it binds: "Gemini Models" binds gemini, "Claude and GPT models" claude and gpt.
pub fn agy_quota(out: &str) -> Vec<Value> {
    out.lines().filter_map(|l| {
        let f: Vec<&str> = l.split('\t').map(str::trim).collect();
        if f.len() < 4 { return None }
        let left: f64 = f[2].trim_end_matches('%').trim().parse().ok()?;
        let kind = f[1].to_lowercase();
        let minutes = if kind.contains("five hour") || kind.contains("5 hour") || kind.contains("5-hour") { 300 } else if kind.contains("week") { 10080 } else if kind.contains("day") || kind.contains("daily") { 1440 } else { return None };
        let group = f[0].trim_end_matches(" Models").trim_end_matches(" models").trim();
        let models: Vec<String> = group.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty() && !matches!(*w, "and" | "models" | "model")).map(str::to_string).collect();
        let slug = models.join("_");
        let mut w = json!({"scope":format!("{}_{slug}", span_scope(minutes)),"label":format!("{group} {}", span_label(minutes).to_lowercase()),"used_percent":pct(100.0 - left),"window_minutes":minutes,"models":models});
        if let Some(r) = super::accounts_io::parse_iso(f[3]) { w["resets_at"] = json!(r) }
        Some(w)
    }).collect()
}
/// Grok's `_x.ai/billing` result: the period's credit use, and the plan tier.
pub fn grok_billing(r: &Value) -> (Vec<Value>, String) {
    let c = &r["config"];
    let tier = s(r, "subscription_tier").to_string();
    let Some(used) = c["creditUsagePercent"].as_f64() else { return (vec![], tier) };
    let kind = s(&c["currentPeriod"], "type").to_uppercase();
    let minutes = if kind.contains("WEEK") { 10080 } else if kind.contains("MONTH") { 43_200 } else if kind.contains("DAY") || kind.contains("DAILY") { 1440 } else { 0 };
    let end = when_of(&c["currentPeriod"]["end"]).or_else(|| when_of(&c["billingPeriodEnd"]));
    let mut w = json!({"scope":if minutes == 0 { "period".to_string() } else { span_scope(minutes) },"label":if minutes == 0 { "Billing period".to_string() } else { span_label(minutes) },"used_percent":pct(used)});
    if minutes > 0 { w["window_minutes"] = json!(minutes) }
    if let Some(e) = end { w["resets_at"] = json!(e) }
    (vec![w], tier)
}
/// Muse's `usage --json`: Ok(windows, tier), or why it has none (it sends none before the first use in a new window).
pub fn muse_usage(v: &Value) -> Result<(Vec<Value>, String), String> {
    let a = &v["allowance"];
    if !a.is_object() { return Err(if s(v, "reason") == "server_sent_no_usage" { "Muse has sent no usage yet: it does after the first use in a window".into() } else { let h = s(v, "hint"); if h.is_empty() { "Muse reported no usage".into() } else { format!("Muse: {h}") } }) }
    let mut ws = vec![];
    for (k, default) in [("window", 300u64), ("weekly", 10080)] {
        let b = &a[k]; let Some(u) = b["usedPercent"].as_f64() else { continue };
        let minutes = b["windowDurationMins"].as_u64().filter(|m| *m > 0).unwrap_or(default);
        let mut w = json!({"scope":span_scope(minutes),"label":span_label(minutes),"used_percent":pct(u),"window_minutes":minutes});
        if let Some(r) = when_of(&b["resetsAtMs"]) { w["resets_at"] = json!(r) }
        ws.push(w);
    }
    Ok((ws, s(a, "tier").to_string()))
}
/// Oh My Pi's `usage --json`: every report's limits, named by the provider they are of ("anthropic 5-hour"). No account email or id is kept.
pub fn omp_usage(v: &Value) -> Vec<Value> {
    let mut out = vec![];
    for r in v["reports"].as_array().into_iter().flatten().take(20) {
        for l in r["limits"].as_array().into_iter().flatten().take(20) {
            let a = &l["amount"];
            let frac = a["usedFraction"].as_f64()
                .or_else(|| match (a["used"].as_f64(), a["limit"].as_f64()) { (Some(u), Some(m)) if m > 0.0 => Some(u / m), _ => None })
                .or_else(|| (s(a, "unit") == "percent").then(|| a["used"].as_f64().map(|u| u / 100.0)).flatten())
                .or_else(|| a["remainingFraction"].as_f64().map(|r| 1.0 - r));
            let Some(frac) = frac else { continue };
            let provider = [s(&l["scope"], "provider"), s(r, "provider")].into_iter().find(|p| !p.is_empty()).unwrap_or("omp");
            let win = &l["window"];
            let minutes = win["durationMs"].as_f64().map(|m| (m / 60_000.0).round() as u64).unwrap_or(0);
            let what = [s(win, "label"), s(l, "label")].into_iter().find(|x| !x.is_empty()).map(str::to_string).unwrap_or_else(|| span_label(minutes));
            let mut w = json!({"scope":format!("{provider}/{}", s(l, "id")),"label":format!("{provider} {what}"),"used_percent":pct(frac * 100.0)});
            if minutes > 0 { w["window_minutes"] = json!(minutes) }
            if let Some(m) = l["scope"]["modelId"].as_str().filter(|m| !m.is_empty()) { w["models"] = json!([m.to_lowercase()]) }
            if let Some(t) = when_of(&win["resetsAt"]) { w["resets_at"] = json!(t) }
            out.push(w);
        }
    }
    out
}

/// One reading as the setting keeps it. `got`: Ok(windows, plan) or why there are none; `source` names how it was read.
pub fn reading(provider: &str, source: &str, got: Result<(Vec<Value>, String), String>, now: u64) -> Value {
    match got {
        Ok((ws, plan)) => {
            let mut a = json!({"provider":provider,"state":"ok","at":now,"attempted_at":now,"source":source,"windows":ws});
            if !plan.is_empty() { a["plan"] = json!(super::accounts_io::label(&plan)) }
            if a["windows"].as_array().map_or(true, Vec::is_empty) { a["reason"] = json!(format!("{} reported no limit windows", name(provider))) }
            a
        }
        Err(why) => json!({"provider":provider,"state":"unavailable","at":0,"attempted_at":now,"source":source,"reason":why.chars().take(200).collect::<String>(),"windows":[]}),
    }
}
/// A provider whose CLI tells nothing of its limits.
pub fn unreported(provider: &str, now: u64) -> Value { json!({"provider":provider,"state":"unreported","at":0,"attempted_at":now,"source":"none","reason":format!("usage not reported by {}", name(provider)),"windows":[]}) }
/// Whether a new reading is written: what it says changed (the times of the look aside), or the kept one is older than [`KEEP_S`]. A failed look never replaces a good reading's
/// windows (they stand, as a last reading, until their resets).
pub fn worth(old: &str, new: &Value, now: u64) -> bool {
    let o: Value = serde_json::from_str(old).unwrap_or(Value::Null);
    if o.is_null() { return true }
    if new["state"] == "unavailable" && o["state"] == "ok" && o["windows"].as_array().map_or(false, |w| !w.is_empty()) { return false }
    let bare = |v: &Value| { let mut v = v.clone(); if let Some(m) = v.as_object_mut() { m.remove("at"); m.remove("attempted_at"); } v };
    bare(&o) != bare(new) || now.saturating_sub(o["attempted_at"].as_u64().unwrap_or(0)) >= KEEP_S
}

// ---- reading the CLIs ----

async fn run(cmd: &Path, args: &[&str], limit: Duration) -> Option<(i32, String, String)> { run_unset(cmd, args, limit, &[]).await }
/// [`run`] with `unset` taken out of the CLI's environment (agy's sign-in store: agy_cli.rs `storage_unset`).
async fn run_unset(cmd: &Path, args: &[&str], limit: Duration, unset: &[&str]) -> Option<(i32, String, String)> {
    let mut c = proc::tokio_cmd(Kind::Agent, cmd);
    for k in unset { c.env_remove(k); }
    if let Some(p) = Cli::from_env().child_path() { c.env("PATH", p); }
    // a read in its own scratch folder, so nothing the CLI writes beside its working folder lands in a project
    let tmp = std::env::temp_dir(); c.current_dir(&tmp);
    c.args(args).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let out = tokio::time::timeout(limit, c.output()).await.ok()?.ok()?;
    Some((out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned()))
}
fn last_line(e: &str) -> String { e.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").chars().take(160).collect() }
/// JSON-RPC over stdio: each request sent once the answer to the one before it came (an ACP agent reads its `initialize` first); the last answer's `result`.
async fn rpc(cmd: &Path, args: &[&str], reqs: &[Value], limit: Duration) -> Result<Value, String> {
    let mut c = proc::tokio_cmd(Kind::Agent, cmd);
    if let Some(p) = Cli::from_env().child_path() { c.env("PATH", p); }
    c.current_dir(std::env::temp_dir());
    c.args(args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
    let mut child = c.spawn().map_err(|_| "it did not start".to_string())?;
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else { return Err("it did not start".into()) };
    let r = tokio::time::timeout(limit, async {
        let mut lines = tokio::io::BufReader::new(stdout).lines(); let mut last = Value::Null;
        for q in reqs {
            stdin.write_all(format!("{q}\n").as_bytes()).await.map_err(|_| "it closed its input".to_string())?; let _ = stdin.flush().await;
            loop {
                let Ok(Some(l)) = lines.next_line().await else { return Err("it ended before it answered".to_string()) };
                let Ok(v) = serde_json::from_str::<Value>(&l) else { continue };
                if v["id"] != q["id"] { continue }
                if v["error"].is_object() { return Err(s(&v["error"], "message").chars().take(160).collect()) }
                last = v["result"].clone(); break;
            }
        }
        Ok(last)
    }).await.unwrap_or_else(|_| Err("it did not answer in time".into()));
    let _ = child.start_kill(); let _ = child.wait().await;
    r
}
/// One provider's reading now (None: its CLI is not on this machine). `home`: the Backplane home (Antigravity's pinned agent lives there).
pub async fn read(cli: &Cli, home: &str, provider: &str, now: u64) -> Option<Value> {
    let prog = cli.resolve(cli_of(provider)).ok()?;
    Some(match provider {
        "antigravity" if crate::agents::providers::agy::pinned(home) => json!({"provider":provider,"state":"unreported","at":0,"attempted_at":now,"source":"none","reason":"usage not reported by Backplane's pinned Antigravity agent","windows":[]}),
        "antigravity" => reading(provider, "agy-quota", match run_unset(&prog, &["-p", "/quota"], Duration::from_secs(45), &crate::agents::providers::agy_cli::storage_unset_here(&std::path::PathBuf::from(crate::plat::dirs::home_or_empty()))).await {
            Some((0, o, _)) => Ok((agy_quota(&o), String::new())),
            Some((2, o, e)) => Err(format!("Antigravity has no account quota to show ({})", { let l = last_line(&format!("{o}\n{e}")); if l.is_empty() { "an API key is in use".into() } else { l } })),
            Some((c, o, e)) => Err(format!("agy /quota failed: {}", { let l = last_line(&format!("{e}\n{o}")); if l.is_empty() { format!("exit {c}") } else { l } })),
            None => Err("agy /quota did not answer in time".into()),
        }, now),
        "grok" => reading(provider, "grok-acp-billing", rpc(&prog, &["agent", "stdio"], &[json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{}}}),
            json!({"jsonrpc":"2.0","id":2,"method":"_x.ai/billing","params":{}})], Duration::from_secs(30)).await.map(|r| grok_billing(&r)).map_err(|e| format!("Grok did not report its usage: {e}")), now),
        "muse" => reading(provider, "muse-usage", match run(&prog, &["usage", "--json"], Duration::from_secs(30)).await {
            Some((0, o, _)) => serde_json::from_str::<Value>(o.trim()).map_err(|_| "muse usage printed no JSON".to_string()).and_then(|v| muse_usage(&v)),
            Some((c, _, e)) => Err(format!("muse usage failed: {}", { let l = last_line(&e); if l.is_empty() { format!("exit {c}") } else { l } })),
            None => Err("muse usage did not answer in time".into()),
        }, now),
        "omp" => reading(provider, "omp-usage", match run(&prog, &["usage", "--json"], Duration::from_secs(60)).await {
            Some((0, o, _)) => serde_json::from_str::<Value>(o.trim()).map(|v| (omp_usage(&v), String::new())).map_err(|_| "omp usage printed no JSON".to_string()),
            Some((c, _, e)) => Err(format!("omp usage failed: {}", { let l = last_line(&e); if l.is_empty() { format!("exit {c}") } else { l } })),
            None => Err("omp usage did not answer in time".into()),
        }, now),
        _ => unreported(provider, now),
    })
}
/// Every provider's reading, written as `allowance.<provider>` where it is worth it (`worth`).
pub async fn refresh(hub: &Arc<Hub>, cli: &Cli) -> usize {
    let home = hub.cfg.home.to_string_lossy().into_owned(); let mut n = 0;
    for p in OTHERS {
        let now = crate::hub::now_ms() / 1000;
        let Some(v) = read(cli, &home, p, now).await else { continue };
        let old = hub.setting(&key(p)).await.unwrap_or_default();
        if worth(&old, &v, now) { let _ = hub.commit(&json!({"$":"SettingSet","key":key(p),"value":v.to_string(),"at":now})).await; n += 1 }
    }
    n
}
/// At start, after a moment, and every [`EVERY`]. A copy of a real log with no stand-ins reads nothing (it would ask the person's own CLIs).
pub fn start(hub: Arc<Hub>) {
    if crate::copy::on() && std::env::var("BACKPLANE_AGENTS").map_or(true, |d| d.is_empty()) { return }
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop { let n = refresh(&hub, &Cli::from_env()).await; if n > 0 { eprintln!("provider usage: {n} reading(s) kept") } tokio::time::sleep(EVERY).await; }
    });
}

// ---- what the hub saw itself, and the Accounts page's entries ----

/// The observed facts of a provider for one view: today's turns and tokens (UTC day, `tok.dv`), and the last rate limit a turn met with its end.
/// `get` reads a setting.
pub fn observed(get: &dyn Fn(&str) -> String, provider: &str, now: u64) -> Value {
    let ep = format!("{}{}", get("tok.epoch"), get("tok.recount"));
    let x = if ep.is_empty() { "tok.".to_string() } else { format!("tok.{ep}.") };
    let day = crate::state::fam::day_of(now);
    let raw = get(&format!("{x}dv.{day}|{provider}"));
    let part = |i: usize| raw.split('/').nth(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let mut o = json!({"turns":part(0),"tokens":part(1) + part(2) + part(3) + part(4)});
    // the last limit: what the driver saw (`seen`), else the failover hold on the provider (or a model of it)
    let rec = |k: &str| serde_json::from_str::<Value>(&get(k)).unwrap_or(Value::Null);
    let seen = rec(&seen_key(provider));
    let hold = rec(&super::fallback::Limit::key(provider));
    let l = if seen["at"].as_u64().unwrap_or(0) >= hold["at"].as_u64().unwrap_or(0) { seen } else { hold };
    if !s(&l, "scope").is_empty() || !s(&l, "reason").is_empty() {
        let end = super::fallback::hold_end(&l);
        o["limit"] = json!({"reason":s(&l, "reason").chars().take(200).collect::<String>(),"at":l["at"],"until":end,"active":end == 0 || end > now});
    }
    o
}
fn tokens(n: u64) -> String { if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1e6) } else if n >= 1000 { format!("{}k", n / 1000) } else { n.to_string() } }
/// The Accounts page's entries for these providers (accounts_people.rs `people` adds them after the accounts): one a provider whose CLI is here or that has a reading,
/// titled by the provider, its one login `kind: provider` (no account page, no default to choose). `others`: `[{provider, allowance, observed}]` (AccountsRpc).
pub fn people(others: &Value, zone: &str, now: u64) -> Vec<Value> {
    others.as_array().into_iter().flatten().filter_map(|o| {
        let (p, al, ob) = (s(o, "provider"), &o["allowance"], &o["observed"]);
        if p.is_empty() { return None }
        let lim = &ob["limit"];
        let out = lim["active"] == true;
        let hold = if !out { String::new() } else { let u = lim["until"].as_u64().unwrap_or(0); if u > now { format!("rate-limited until {}", crate::agents::rules::when(zone, u, now)) } else { "rate-limited".into() } };
        let bars = super::accounts_phone::bars(al, out, zone, now);
        let (state, line, tone) = match s(al, "state") {
            "ok" | "stale" => ("connected", "connected", "ok"),
            "unavailable" => ("unchecked", "usage not read", "dim"),
            "unreported" => ("unchecked", "usage not reported", "dim"),
            _ => ("unchecked", "no usage reading yet", "dim"),
        };
        let mut note: Vec<String> = vec![];
        if bars.is_empty() { let r = s(al, "reason"); note.push(if r.is_empty() { format!("usage not reported by {}", name(p)) } else { r.to_string() }) }
        let (turns, toks) = (ob["turns"].as_u64().unwrap_or(0), ob["tokens"].as_u64().unwrap_or(0));
        if turns > 0 { note.push(format!("Seen here today: {turns} turn{}{}", if turns == 1 { "" } else { "s" }, if toks > 0 { format!(", {} tokens", tokens(toks)) } else { String::new() })) }
        if !out && lim.is_object() { let at = lim["at"].as_u64().unwrap_or(0); if at > 0 { note.push(format!("Last rate limit {}", crate::agents::rules::when(zone, at, now))) } }
        let note = note.into_iter().map(|n| { let mut c = n.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }).collect::<Vec<_>>().join(". ");
        let login = json!({"provider":p,"title":name(p),"kind":"provider","profile":"","epoch":0,"plan":s(al, "plan"),"state":state,"line":line,"tone":tone,"hold":hold,"out":out,
            "default":false,"bars":bars,"note":note,"also":[],"label":""});
        Some(json!({"key":format!("provider:{p}"),"title":name(p),"logins":[login]}))
    }).collect()
}
/// `[{provider, allowance, observed}]` for every provider of [`OTHERS`] whose CLI was found by the last detection or that has a reading. `installed`: the provider ids found.
pub fn others(get: &dyn Fn(&str) -> String, installed: &[String], now: u64) -> Value {
    json!(OTHERS.iter().filter_map(|p| {
        let al: Value = serde_json::from_str(&get(&key(p))).unwrap_or(Value::Null);
        if al.is_null() && !installed.iter().any(|i| i == p) { return None }
        Some(json!({"provider":p,"allowance":al,"observed":observed(get, p, now)}))
    }).collect::<Vec<_>>())
}
/// What is left of a reading for `model` ("" every window): the least of its windows that bind the model and have not reset, in percent; None with no window.
/// A reading's age does not matter: a window only fills further until its reset.
pub fn remaining(al: &Value, model: &str, now: u64) -> Option<f64> {
    if !matches!(s(al, "state"), "ok" | "stale") { return None }
    let m = model.to_lowercase();
    al["windows"].as_array().into_iter().flatten().filter(|w| m.is_empty() || super::fallback::binds(w, &m).unwrap_or(true))
        .filter(|w| w["resets_at"].as_u64().map_or(true, |r| r > now)).filter_map(|w| w["used_percent"].as_f64()).map(|u| 100.0 - u.clamp(0.0, 100.0)).reduce(f64::min)
}
/// `account_list`'s row of a provider without accounts (`o`: one of [`others`]): its own login, the windows of its reading, what is left for `model`, and when it does not report
/// usage, that and what the hub saw (`seenToday`, `lastLimit`). `current`: the thread asked about runs on it.
pub fn account_row(o: &Value, model: &str, current: bool, now: u64) -> Value {
    let (p, al, ob) = (s(o, "provider"), &o["allowance"], &o["observed"]);
    let windows: Vec<Value> = al["windows"].as_array().into_iter().flatten().map(|w| json!({"scope":w["scope"],"label":w["label"],"usedPercent":w["used_percent"],"resetsAt":w["resets_at"],"models":w["models"]})).collect();
    let lim = &ob["limit"]; let held = lim["active"] == true;
    let reported = s(al, "state") == "ok" && !windows.is_empty();
    let mut row = json!({"id":"","label":"Default account","provider":p,"email":null,"plan":if s(al, "plan").is_empty() { Value::Null } else { json!(s(al, "plan")) },"state":"connected",
        "remainingPercent":remaining(al, model, now),"held":held,"heldUntil":if held { lim["until"].as_u64().unwrap_or(0) } else { 0 },"windows":windows,"current":current,"default":true,"sameAsDefault":false,
        "usageReported":reported,"seenToday":{"turns":ob["turns"],"tokens":ob["tokens"]}});
    if !reported { row["note"] = json!(if s(al, "reason").is_empty() { format!("usage not reported by {}", name(p)) } else { s(al, "reason").to_string() }) }
    if lim.is_object() { row["lastLimit"] = json!({"reason":lim["reason"],"at":lim["at"],"until":lim["until"]}) }
    row
}
/// The providers of [`OTHERS`] whose CLI the last detection found (`models::info`'s `about.<p>.cli`).
pub fn installed() -> Vec<String> {
    let m: Value = serde_json::from_str(&super::models::info()).unwrap_or(Value::Null);
    OTHERS.iter().filter(|p| m["about"][**p]["cli"].as_str().map_or(false, |c| !c.is_empty())).map(|p| p.to_string()).collect()
}
/// The settings `others` and `observed` read, from the hub.
pub async fn others_of(hub: &Arc<Hub>, now: u64) -> Value {
    let day = crate::state::fam::day_of(now);
    let mut keys: Vec<String> = vec![];
    for p in OTHERS { keys.push(key(p)); keys.push(seen_key(p)); keys.push(super::fallback::Limit::key(p)); }
    let mut settings = std::collections::HashMap::new();
    for k in &keys { if let Some(v) = hub.setting(k).await { settings.insert(k.clone(), v); } }
    // the token totals are the views' fold's (state/fam.rs `turn_usage`), not the log's settings: its epoch names their keys
    let tok = crate::statehost::with_state_of(hub, |st| {
        let g = |k: &str| st.settings.get(k).cloned().unwrap_or_default();
        let ep = format!("{}{}", g("tok.epoch"), g("tok.recount"));
        let x = if ep.is_empty() { "tok.".to_string() } else { format!("tok.{ep}.") };
        let mut v: Vec<(String, String)> = vec![("tok.epoch".into(), g("tok.epoch")), ("tok.recount".into(), g("tok.recount"))];
        for p in OTHERS { let k = format!("{x}dv.{day}|{p}"); v.push((k.clone(), g(&k))) }
        v
    }).unwrap_or_default();
    settings.extend(tok);
    others(&|k: &str| settings.get(k).cloned().unwrap_or_default(), &installed(), now)
}

#[cfg(test)]
mod tests {
    use super::*;
    // what agy 1.5.12 printed for `agy -p /quota` (Nov 19)
    const AGY: &str = "Gemini Models\tWeekly Limit Remaining\t94%\t2026-11-25T21:50:30Z\nGemini Models\tFive Hour Limit Remaining\t90%\t2026-11-20T00:58:05Z\nClaude and GPT models\tWeekly Limit Remaining\t100%\t2026-11-26T21:20:45Z\nClaude and GPT models\tFive Hour Limit Remaining\t0%\t2026-11-20T02:20:45Z\n";
    #[test] fn antigravity_quota_lines_are_windows_per_model_family() {
        let w = agy_quota(AGY);
        assert_eq!(w.len(), 4);
        assert_eq!((w[0]["scope"].as_str(), w[0]["label"].as_str(), w[0]["used_percent"].as_f64(), w[0]["resets_at"].as_u64()), (Some("seven_day_gemini"), Some("Gemini weekly"), Some(6.0), Some(1795643430)));
        assert_eq!((w[1]["scope"].as_str(), w[1]["window_minutes"].as_u64()), (Some("five_hour_gemini"), Some(300)));
        assert_eq!(w[3]["models"], json!(["claude", "gpt"])); assert_eq!(w[3]["label"], "Claude and GPT 5-hour"); assert_eq!(w[3]["used_percent"], 100.0);
        assert!(agy_quota("Remaining credits\t0\n").is_empty(), "credits are not a window");
    }
    #[test] fn grok_billing_is_the_periods_credit_use() {
        let r = json!({"config":{"creditUsagePercent":19.0,"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","start":"2026-11-18T08:10:26.011+00:00","end":"2026-11-25T08:10:26.011+00:00"},"prepaidBalance":{"val":0}},"subscription_tier":"SuperGrok"});
        let (w, tier) = grok_billing(&r);
        assert_eq!(tier, "SuperGrok");
        assert_eq!((w[0]["scope"].as_str(), w[0]["label"].as_str(), w[0]["used_percent"].as_f64(), w[0]["resets_at"].as_u64()), (Some("seven_day"), Some("Weekly"), Some(19.0), Some(1795594226)));
        assert!(grok_billing(&json!({"config":{}})).0.is_empty(), "no percent, no window");
    }
    #[test] fn muse_usage_has_its_window_and_week_or_says_why_not() {
        let v = json!({"allowance":{"observedAtMs":1,"tier":"pro","weekly":{"resetsAtMs":1795643430000u64,"usedPercent":130},"window":{"resetsAtMs":1795600000000u64,"usedPercent":42,"windowDurationMins":300}},"schemaVersion":1});
        let (w, tier) = muse_usage(&v).unwrap();
        assert_eq!(tier, "pro"); assert_eq!((w[0]["scope"].as_str(), w[0]["used_percent"].as_f64(), w[0]["resets_at"].as_u64()), (Some("five_hour"), Some(42.0), Some(1795600000)));
        assert_eq!((w[1]["label"].as_str(), w[1]["used_percent"].as_f64()), (Some("Weekly"), Some(100.0)), "over the budget reads full");
        let none = json!({"allowance":null,"hint":"the server sent no usage data","reason":"server_sent_no_usage","schemaVersion":1});
        assert!(muse_usage(&none).unwrap_err().contains("first use"));
    }
    #[test] fn omp_reports_every_accounts_limits_without_who() {
        let v = json!({"generatedAt":1,"reports":[{"provider":"anthropic","metadata":{"email":"person@example.com"},"limits":[
            {"id":"5h","label":"5 hour","scope":{"provider":"anthropic","accountId":"acc-1"},"window":{"id":"5h","label":"5-hour","durationMs":18_000_000u64,"resetsAt":1795600000000u64},"amount":{"usedFraction":0.25,"unit":"percent"}},
            {"id":"7d-opus","label":"Opus weekly","scope":{"provider":"anthropic","modelId":"Claude-Opus"},"window":{"id":"7d","label":"weekly","durationMs":604_800_000u64},"amount":{"used":50,"limit":200,"unit":"requests"}},
            {"id":"x","label":"nothing","scope":{"provider":"anthropic"},"amount":{"unit":"unknown"}}]}]});
        let w = omp_usage(&v);
        assert_eq!(w.len(), 2, "a limit with no amount is left out");
        assert_eq!((w[0]["label"].as_str(), w[0]["used_percent"].as_f64(), w[0]["resets_at"].as_u64(), w[0]["window_minutes"].as_u64()), (Some("anthropic 5-hour"), Some(25.0), Some(1795600000), Some(300)));
        assert_eq!((w[1]["used_percent"].as_f64(), &w[1]["models"]), (Some(25.0), &json!(["claude-opus"])));
        assert!(!w.iter().any(|x| x.to_string().contains("person@example.com") || x.to_string().contains("acc-1")), "no email or account id is kept");
    }
    #[test] fn a_reading_is_kept_when_it_changed_or_is_old_and_a_failed_look_keeps_the_last_good_one() {
        let ok = reading("grok", "grok-acp-billing", Ok((vec![json!({"scope":"seven_day","used_percent":19.0})], "SuperGrok".into())), 1000);
        assert_eq!((ok["state"].as_str(), ok["plan"].as_str()), (Some("ok"), Some("SuperGrok")));
        assert!(worth("", &ok, 1000));
        let same = reading("grok", "grok-acp-billing", Ok((vec![json!({"scope":"seven_day","used_percent":19.0})], "SuperGrok".into())), 1300);
        assert!(!worth(&ok.to_string(), &same, 1300), "the same reading five minutes on is not written again");
        assert!(worth(&ok.to_string(), &same, 1000 + KEEP_S), "but is after half an hour");
        let more = reading("grok", "grok-acp-billing", Ok((vec![json!({"scope":"seven_day","used_percent":20.0})], "SuperGrok".into())), 1300);
        assert!(worth(&ok.to_string(), &more, 1300));
        let bad = reading("grok", "grok-acp-billing", Err("it did not answer in time".into()), 1300);
        assert_eq!(bad["state"], "unavailable"); assert!(!worth(&ok.to_string(), &bad, 1300), "a failed look does not hide the windows");
        assert_eq!(reading("muse", "muse-usage", Ok((vec![], String::new())), 5)["reason"], "Muse reported no limit windows");
        assert_eq!(unreported("pi", 5)["reason"], "usage not reported by Pi");
    }
    #[test] fn observed_counts_todays_turns_and_tokens_and_the_last_limit() {
        let now = 1_795_100_000; let day = crate::state::fam::day_of(now);
        let set = std::collections::HashMap::from([
            (format!("tok.dv.{day}|pi"), "3/1000/200/0/500/0/0/0/3".to_string()),
            (seen_key("pi"), json!({"scope":"pi","reason":"429 Too Many Requests","reset":now + 600,"at":now - 60}).to_string()),
        ]);
        let get = |k: &str| set.get(k).cloned().unwrap_or_default();
        let o = observed(&get, "pi", now);
        assert_eq!((o["turns"].as_u64(), o["tokens"].as_u64()), (Some(3), Some(1700)));
        assert_eq!((o["limit"]["until"].as_u64(), o["limit"]["active"].as_bool()), (Some(now + 600), Some(true)));
        assert!(observed(&|_| String::new(), "pi", now)["limit"].is_null(), "no limit seen, none said");
    }
    #[test] fn account_list_has_each_providers_remaining_and_says_when_none_is_reported() {
        let now = 1_795_100_000;
        let o = json!({"provider":"antigravity","allowance":reading("antigravity", "agy-quota", Ok((agy_quota(AGY), String::new())), now - 3000),"observed":{"turns":2,"tokens":10}});
        let r = account_row(&o, "gemini-4-neon", true, now);
        assert_eq!((r["remainingPercent"].as_f64(), r["usageReported"].as_bool(), r["current"].as_bool(), r["windows"].as_array().unwrap().len()), (Some(90.0), Some(true), Some(true), 4), "an hour-old reading still counts: {r}");
        assert_eq!(account_row(&o, "claude-opus-5-5-high", false, now)["remainingPercent"], 0.0, "its Claude and GPT window is full");
        assert_eq!(account_row(&o, "", false, now)["remainingPercent"], 0.0, "no model: the fullest window");
        let pi = account_row(&json!({"provider":"pi","allowance":unreported("pi", now),"observed":{"turns":0,"tokens":0,"limit":{"reason":"429","at":now - 5,"until":now + 60,"active":true}}}), "", false, now);
        assert_eq!((pi["usageReported"].as_bool(), pi["note"].as_str(), pi["held"].as_bool(), pi["heldUntil"].as_u64()), (Some(false), Some("usage not reported by Pi"), Some(true), Some(now + 60)));
        assert!(pi["remainingPercent"].is_null());
    }
    #[test] fn the_accounts_page_says_what_each_provider_reports_and_what_was_seen() {
        let now = 1_795_100_000;
        let others = json!([
            {"provider":"antigravity","allowance":reading("antigravity", "agy-quota", Ok((agy_quota(AGY), String::new())), now),"observed":{"turns":0,"tokens":0}},
            {"provider":"pi","allowance":unreported("pi", now),"observed":{"turns":3,"tokens":1700,"limit":{"reason":"429","at":now - 60,"until":now + 600,"active":true}}},
            {"provider":"opencode","allowance":null,"observed":{"turns":0,"tokens":0}}]);
        let p = people(&others, "", now);
        assert_eq!(p.iter().map(|e| e["title"].as_str().unwrap()).collect::<Vec<_>>(), ["Antigravity", "Pi", "OpenCode"]);
        let agy = &p[0]["logins"][0];
        assert_eq!((agy["kind"].as_str(), agy["line"].as_str(), agy["bars"].as_array().unwrap().len()), (Some("provider"), Some("connected"), 4));
        assert_eq!(agy["bars"][3]["tone"], "bad", "a full window reads bad"); assert_eq!(agy["bars"][0]["name"], "Gemini weekly");
        let pi = &p[1]["logins"][0];
        assert_eq!((pi["out"].as_bool(), pi["line"].as_str()), (Some(true), Some("usage not reported")));
        assert!(pi["hold"].as_str().unwrap().starts_with("rate-limited until "), "{pi}");
        assert_eq!(pi["note"], "Usage not reported by Pi. Seen here today: 3 turns, 1k tokens");
        assert_eq!(p[2]["logins"][0]["note"], "Usage not reported by OpenCode");
    }
}
