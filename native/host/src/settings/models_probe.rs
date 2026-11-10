//! Which agents are installed and what models each offers (0.14's Models.detect, src/server/server.bend): the `models` info the composer's picker,
//! Settings > Agents and the `&` mentions read. Each CLI is asked the way 0.14 asked it, read-only and with no model turn:
//!   claude: an `initialize` control request (`claude -p --input-format stream-json`), answered before any model is called;
//!   codex: its own cache (`~/.codex/models_cache.json`); grok: `grok models`; muse: `muse serve`'s model/list; opencode: `opencode models` and
//!   `opencode auth list`; pi: `pi --list-models` (offline); omp: `omp models --json`; antigravity: what is installed under the home.
//! The window opens before any of this: the hub serves the list it found last time (`<home>/models.cache`) and probes after it, then every
//! 15 minutes; a list that changed is cached, pushed and every view is drawn again (`Hub::bump_views`). A copy of a real log probes nothing.
//! The parsers are pure and tested; `detect` is the only part that runs programs (through proc.rs, as agents).
use crate::agents::providers::{agy, muse, opencode};
use crate::hub::Hub;
use crate::proc::{self, Kind};
use serde_json::{json, Map, Value};
use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

/// 0.14's Models.every(): the full detection's period.
const EVERY: Duration = Duration::from_secs(900);

// ---- the parsers (pure) ----

/// The probe's `initialize` answer: its models ([] when it failed or the CLI is too old to answer).
pub fn claude_listed(out: &str) -> Vec<Value> {
    out.lines().filter(|l| l.contains("\"control_response\"")).find_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .and_then(|v| v["response"]["response"]["models"].as_array().cloned()).unwrap_or_default()
}
/// A Claude id's family and version: "claude-fable-5-2" -> ("fable", [5, 2]); None for a dated id, an alias or another shape.
fn claude_version(id: &str) -> Option<(&str, Vec<u32>)> {
    let mut p = id.strip_prefix("claude-")?.split('-'); let fam = p.next()?;
    if !["opus", "sonnet", "haiku", "fable"].contains(&fam) { return None }
    let v = p.map(|x| if x.len() < 4 { x.parse().ok() } else { None }).collect::<Option<Vec<u32>>>()?;
    if v.is_empty() { None } else { Some((fam, v)) }
}
/// The CLI's own model rows plus the newest model of each family that the API lists (`GET /v1/models`, `api`) and the CLI's list lacks: a CLI release lags a model
/// the account already has (Fable 5.2 was usable on Claude Code 2.1.321, which listed only 5.1). The new row follows its family's newest row and takes its
/// efforts, description and a `[1m]` suffix; the CLI's rows are untouched. A family the CLI does not list is not added (no efforts are known for it).
pub fn claude_extend(mut xs: Vec<Value>, api: &Value) -> Vec<Value> {
    let Some(ids) = api["data"].as_array() else { return xs };
    let mut add: Vec<(usize, Value)> = vec![];
    for fam in ["fable", "opus", "sonnet", "haiku"] {
        let best = xs.iter().enumerate().filter_map(|(i, o)| claude_version(gs(o, "resolvedModel")).filter(|(f, _)| *f == fam).map(|(_, v)| (v, i))).max_by(|a, b| a.0.cmp(&b.0));
        let Some((have, at)) = best else { continue };
        let newer = ids.iter().filter_map(|o| { let id = gs(o, "id"); claude_version(id).filter(|(f, v)| *f == fam && *v > have).map(|(_, v)| (v, o.clone())) }).max_by(|a, b| a.0.cmp(&b.0));
        let Some((_, o)) = newer else { continue };
        let (id, row) = (gs(&o, "id").to_string(), &xs[at]);
        let name = { let d = gs(&o, "display_name"); d.strip_prefix("Claude ").unwrap_or(d).to_string() };
        let suffix = if gs(row, "value").ends_with("[1m]") { "[1m]" } else { "" };
        let mut n = row.clone();
        n["value"] = json!(format!("{id}{suffix}")); n["resolvedModel"] = json!(id); n["displayName"] = json!(if name.is_empty() { id.clone() } else { name });
        add.push((at + 1, n));
    }
    for (at, n) in add.into_iter().rev() { xs.insert(at.min(xs.len()), n) }
    xs
}
fn gs<'a>(o: &'a Value, k: &str) -> &'a str { o.get(k).and_then(Value::as_str).unwrap_or("") }
/// Claude.values: what a thread may pick ("default" is the picker's own first row).
pub fn claude_models(xs: &[Value]) -> Vec<String> {
    if xs.is_empty() { return super::models::CLAUDE.iter().map(|s| s.to_string()).collect() }
    xs.iter().map(|o| gs(o, "value")).filter(|v| !v.is_empty() && *v != "default").map(str::to_string).collect()
}
/// Claude.about: "Opus 5.5 · claude-opus-5-5" a model (its description when the value is the full name), and the efforts each takes (none for a model without).
pub fn claude_about(cli: &str, xs: &[Value]) -> Value {
    if xs.is_empty() { let mut a = super::models::about()["claude"].clone(); a["cli"] = json!(cli); return a }
    let (mut labels, mut efforts) = (Map::new(), Map::new());
    for o in xs {
        let v = gs(o, "value"); if v.is_empty() { continue }
        let tail = if v == gs(o, "resolvedModel") { gs(o, "description") } else { gs(o, "resolvedModel") };
        let name = { let d = gs(o, "displayName"); if d.is_empty() { v } else { d } };
        labels.insert(v.into(), json!(if tail.is_empty() { name.to_string() } else { format!("{name} · {tail}") }));
        efforts.insert(v.into(), o.get("supportedEffortLevels").cloned().filter(Value::is_array).unwrap_or(json!([])));
    }
    json!({"cli": cli, "labels": labels, "efforts": efforts})
}
/// Codex's cached list ({"models": [{"slug", "visibility", "display_name", "description", "supported_reasoning_levels"}]}): the listed slugs.
pub fn codex_models(text: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    v["models"].as_array().into_iter().flatten().filter(|o| o.get("visibility").and_then(Value::as_str).unwrap_or("list") == "list").map(|o| gs(o, "slug")).filter(|s| !s.is_empty()).map(str::to_string).collect()
}
/// About.codex: "GPT-6.1-Sol · Latest workhorse model..." a model, and the reasoning levels each takes.
pub fn codex_about(cli: &str, text: &str) -> Value {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let (mut labels, mut efforts) = (Map::new(), Map::new()); labels.insert("default".into(), json!("Codex's configured default"));
    for o in v["models"].as_array().into_iter().flatten() {
        let s = gs(o, "slug"); if s.is_empty() { continue }
        labels.insert(s.into(), json!([gs(o, "display_name"), gs(o, "description")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")));
        if let Some(ls) = o.get("supported_reasoning_levels").and_then(Value::as_array) { efforts.insert(s.into(), Value::Array(ls.iter().filter_map(|l| l.get("effort").cloned()).collect())); }
    }
    json!({"cli": cli, "labels": labels, "efforts": efforts})
}
fn bullet(l: &str) -> Option<&str> { let t = l.trim(); t.strip_prefix("* ").or_else(|| t.strip_prefix("- ")).and_then(|r| r.split(' ').next()).filter(|s| !s.is_empty()) }
/// `grok models`: "  * grok-4.6 (default)" / "  - grok-4.5" lines.
pub fn grok_models(out: &str) -> Vec<String> { out.lines().filter_map(bullet).map(str::to_string).collect() }
pub fn grok_about(cli: &str, out: &str) -> Value {
    let mut labels = Map::new();
    if !grok_models(out).is_empty() { labels.insert("default".into(), json!("Grok's own default")); }
    for l in out.lines() { if let Some(s) = bullet(l) { labels.insert(s.into(), json!(if l.contains("(default)") { "Grok's default model" } else { "Grok" })); } }
    json!({"cli": cli, "labels": labels})
}
/// `opencode auth list`'s rows ("Name  method  stored") as the listing script's "!auth\t<name>\t<method>" lines.
pub fn opencode_auth(out: &str) -> String {
    out.lines().filter_map(|l| { let l = l.trim_end(); let f: Vec<&str> = l.split("  ").map(str::trim).filter(|x| !x.is_empty()).collect();
        (f.len() >= 3 && f[f.len() - 1] == "stored").then(|| format!("!auth\t{}\t{}\n", f[0], f[1])) }).collect()
}
/// `opencode models`' stdout as the listing script keeps it: "provider/model" lines only.
pub fn opencode_lines(out: &str) -> String { out.lines().map(str::trim_end).filter(|l| !l.is_empty() && !l.contains(' ') && l.contains('/') && l.chars().next().map_or(false, |c| c.is_ascii_alphanumeric())).map(|l| format!("{l}\n")).collect() }
/// `pi --list-models`: "provider model context max-out thinking images" rows after a header, here (id, thinks).
pub fn pi_rows(out: &str) -> Vec<(String, bool)> {
    out.lines().skip(1).filter_map(|l| { let f: Vec<&str> = l.split_whitespace().collect(); (f.len() >= 6).then(|| (format!("{}/{}", f[0], f[1]), f[4] == "yes")) }).collect()
}
const PI_THINKS: &[&str] = &["off", "minimal", "low", "medium", "high"];
pub fn pi_about(cli: &str, rows: &[(String, bool)]) -> Value {
    let mut efforts = Map::new(); efforts.insert("default".into(), json!(PI_THINKS));
    for (id, thinks) in rows { efforts.insert(id.clone(), if *thinks { json!(PI_THINKS) } else { json!([]) }); }
    json!({"cli": cli, "labels": {"default": "Pi's configured default"}, "efforts": efforts})
}
/// `omp models --json`: each model's selector "provider/id", its name and the thinking levels it takes (null: none); its sessions also take off and auto.
pub fn omp_list(out: &str) -> Vec<Value> { serde_json::from_str::<Value>(out).ok().and_then(|v| v["models"].as_array().cloned()).unwrap_or_default() }
pub fn omp_about(cli: &str, xs: &[Value]) -> Value {
    let (mut labels, mut efforts) = (Map::new(), Map::new());
    labels.insert("default".into(), json!("Oh My Pi's configured default")); efforts.insert("default".into(), json!(["off", "auto", "minimal", "low", "medium", "high"]));
    for o in xs {
        let s = gs(o, "selector"); if s.is_empty() { continue }
        labels.insert(s.into(), json!(format!("{} · {s}", gs(o, "name"))));
        let th = o.get("thinking").and_then(Value::as_array).map(|a| { let mut v = vec![json!("off"), json!("auto")]; v.extend(a.iter().cloned()); Value::Array(v) }).unwrap_or(json!([]));
        efforts.insert(s.into(), th);
    }
    json!({"cli": cli, "labels": labels, "efforts": efforts})
}
/// The first line of `--version`.
pub fn version_line(out: &str) -> String { out.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(80).collect() }

/// Everything one detection found, before it is turned into the info.
#[derive(Default)]
pub struct Found {
    pub claude: Option<(String, Vec<Value>)>, pub codex: Option<(String, String)>, pub grok: Option<(String, String)>, pub muse: Option<(String, Vec<Value>)>,
    pub opencode: Option<(String, String)>, pub pi: Option<(String, Vec<(String, bool)>)>, pub omp: Option<(String, Vec<Value>)>, pub agy: agy::Info,
}
/// Models.found: the `models` info (a JSON text) from what was found: None is an agent that is not installed.
pub fn info_of(f: &Found) -> String {
    let base = super::models::about();
    let none = |p: &str| { let mut a = base[p].clone(); a["cli"] = json!(""); a };
    let (cl, cla) = match &f.claude { Some((v, xs)) => (claude_models(xs), claude_about(v, xs)), None => (super::models::CLAUDE.iter().map(|s| s.to_string()).collect(), none("claude")) };
    let (cx, cxa) = match &f.codex { Some((v, t)) => (codex_models(t), codex_about(v, t)), None => (vec![], none("codex")) };
    let (gk, gka) = match &f.grok { Some((v, o)) => (grok_models(o), grok_about(v, o)), None => (vec![], none("grok")) };
    let (ms, msa) = match &f.muse { Some((v, xs)) => (muse::models(xs), json!({"cli": v, "labels": muse::about(xs)})), None => (vec![], none("muse")) };
    let (oc, oca) = match &f.opencode { Some((v, o)) => (opencode::models(o), opencode::about(v, o)), None => (vec![], opencode::about("", "")) };
    let (pi, pia) = match &f.pi { Some((v, rows)) => (rows.iter().map(|r| r.0.clone()).collect(), pi_about(v, rows)), None => (vec![], none("pi")) };
    let (om, oma) = match &f.omp { Some((v, xs)) => (xs.iter().map(|o| gs(o, "selector").to_string()).filter(|s| !s.is_empty()).collect(), omp_about(v, xs)), None => (vec![], none("omp")) };
    json!({"claude": cl, "codex": cx, "grok": gk, "muse": ms, "antigravity": agy::info_models(&f.agy), "opencode": oc, "pi": pi, "omp": om, "agy": agy::info_json(&f.agy), "probed": true,
        "about": {"claude": cla, "codex": cxa, "grok": gka, "muse": msa, "antigravity": agy::info_about(&f.agy), "opencode": oca, "pi": pia, "omp": oma}}).to_string()
}

/// The models cache's text (0.14's Cache.of: the list's JSON on the first line; the prompt's text after it, unused here) and back.
pub fn cache_text(info: &str) -> String { format!("{}\n", info.replace('\n', "")) }
pub fn cache_read(text: &str) -> Option<String> {
    let first = text.lines().next()?.trim();
    serde_json::from_str::<Value>(first).ok().filter(|v| v["claude"].is_array() && v["about"].is_object()).map(|_| first.to_string())
}

// ---- running the CLIs ----

/// A program's exit code and stdout, None when it could not start or ran out of time (it is killed then).
async fn run(cmd: &Path, args: &[&str], env: &[(&str, &str)], limit: Duration) -> Option<(i32, String, String)> {
    let mut c = proc::tokio_cmd(Kind::Agent, cmd);
    c.args(args).envs(env.iter().copied()).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let out = tokio::time::timeout(limit, c.output()).await.ok()?.ok()?;
    Some((out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned()))
}
async fn version(cmd: &Path) -> String { run(cmd, &["--version"], &[], Duration::from_secs(10)).await.filter(|r| r.0 == 0).map_or(String::new(), |r| version_line(&r.1)) }
/// A program fed `lines` on stdin, read until a line `done` says is the answer (or its output ends): the lines read.
async fn ask(cmd: &Path, args: &[&str], lines: &[&str], done: fn(&str) -> bool, limit: Duration) -> String {
    let mut c = proc::tokio_cmd(Kind::Agent, cmd);
    c.args(args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
    let Ok(mut child) = c.spawn() else { return String::new() };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else { return String::new() };
    let mut out = String::new();
    let _ = tokio::time::timeout(limit, async {
        for l in lines { if stdin.write_all(format!("{l}\n").as_bytes()).await.is_err() { return } }
        let _ = stdin.flush().await; drop(stdin);
        let mut r = tokio::io::BufReader::new(stdout).lines();
        while let Ok(Some(l)) = r.next_line().await { let hit = done(&l); out.push_str(&l); out.push('\n'); if hit { return } }
    }).await;
    let _ = child.start_kill(); let _ = child.wait().await;
    out
}

fn codex_cache_path() -> PathBuf {
    std::env::var_os("CODEX_HOME").filter(|v| !v.is_empty()).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".codex")).join("models_cache.json")
}

/// Models.detect: every agent at once; one that is not installed (or blocked: a stand-in folder without it) is None.
pub async fn detect(home: &Path) -> Found {
    let cli = crate::agents::proc::Cli::from_env();
    let at = |n: &str| cli.resolve(n).ok();
    let claude = async { let p = at("claude")?; let v = version(&p).await;
        let out = ask(&p, &["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"], &[r#"{"type":"control_request","request_id":"models","request":{"subtype":"initialize"}}"#],
            |l| l.contains("\"control_response\""), Duration::from_secs(30)).await;
        let api = match super::claude_usage::default_dir() { Some(d) => super::claude_usage::api_models(&d, &super::claude_usage::curl_models()).await, None => Value::Null };
        Some((v, claude_extend(claude_listed(&out), &api))) };
    let codex = async { let p = at("codex")?; let v = version(&p).await; Some((v, std::fs::read_to_string(codex_cache_path()).unwrap_or_default())) };
    let grok = async { let p = at("grok")?; let v = version(&p).await; let o = run(&p, &["models"], &[], Duration::from_secs(15)).await.filter(|r| r.0 == 0).map_or(String::new(), |r| r.1); Some((v, o)) };
    let muse = async { let p = at("muse")?; let v = version(&p).await;
        let out = ask(&p, &["serve", "--no-session-log"], &[r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"backplane","version":"1"}}}"#,
            r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#, r#"{"jsonrpc":"2.0","id":2,"method":"model/list","params":{}}"#],
            |l| l.contains("\"id\":2"), Duration::from_secs(15)).await;
        Some((v, muse::catalog(&out))) };
    let oc = async { let p = at("opencode")?; let v = version(&p).await;
        let mut out = match run(&p, &["models"], &[], Duration::from_secs(30)).await {
            Some((0, o, _)) => opencode_lines(&o),
            Some((c, o, e)) => format!("{}!exit {c}\t{}\n", opencode_lines(&o), e.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").replace('\t', "").chars().take(200).collect::<String>()),
            None => "!exit 124\ttimed out\n".to_string(),
        };
        if let Some((_, a, _)) = run(&p, &["auth", "list"], &[], Duration::from_secs(15)).await { out.push_str(&opencode_auth(&a)); }
        Some((v, out)) };
    let pi = async { let p = at("pi")?; let v = version(&p).await; let o = run(&p, &["--list-models"], &[("PI_OFFLINE", "1")], Duration::from_secs(30)).await.map_or(String::new(), |r| r.1); Some((v, pi_rows(&o))) };
    let omp = async { let p = at("omp")?; let v = version(&p).await; let o = run(&p, &["models", "--json"], &[], Duration::from_secs(30)).await.map_or(String::new(), |r| r.1); Some((v, omp_list(&o))) };
    let ag = async {
        let root = agy::root(&home.to_string_lossy());
        let mut c = proc::tokio_cmd(Kind::Local, "sh"); c.arg("-c").arg(agy::probe_sh()).arg("sh").arg(&root).stdin(std::process::Stdio::null()).kill_on_drop(true);
        let out = tokio::time::timeout(Duration::from_secs(10), c.output()).await.ok().and_then(Result::ok).map_or(String::new(), |o| String::from_utf8_lossy(&o.stdout).into_owned());
        let mut i = agy::info(&out);
        // no pinned agent: turns run the person's agy CLI (providers/agy_cli.rs), so the picker lists it and the models it offers this sign-in
        if i.have.is_empty() { if let Some(p) = at("agy") { i.cli = version(&p).await; if i.cli.is_empty() { i.cli = "?".into() }
            i.cli_models = run(&p, &["models"], &[], Duration::from_secs(30)).await.filter(|r| r.0 == 0).map_or(vec![], |r| agy::cli_models(&r.1)); } }
        i };
    let (claude, codex, grok, muse, opencode, pi, omp, agy) = tokio::join!(claude, codex, grok, muse, oc, pi, omp, ag);
    Found { claude, codex, grok, muse, opencode, pi, omp, agy }
}

/// One detection: a list that differs from the one in force is cached, pushed to every client and every view is drawn again. Answers whether it changed.
pub async fn refresh(hub: &Arc<Hub>) -> bool {
    let info = info_of(&detect(&hub.cfg.home).await);
    if !super::models::set_live(&info) { return false }
    let _ = std::fs::write(hub.cfg.home.join("models.cache.part"), cache_text(&info)).and_then(|_| std::fs::rename(hub.cfg.home.join("models.cache.part"), hub.cfg.home.join("models.cache")));
    hub.push_all(&json!({"t":"info","info":{"models":super::models::info(),"models.all":super::models::all()}}));
    hub.bump_views();
    true
}

/// At start: the list found last time at once, then a detection (after the window is up) and one every 15 minutes. A copy of a real log with no stand-ins probes nothing.
pub fn start(hub: Arc<Hub>) {
    if let Some(i) = std::fs::read_to_string(hub.cfg.home.join("models.cache")).ok().as_deref().and_then(cache_read) { super::models::set_live(&i); }
    if crate::copy::on() && std::env::var("BACKPLANE_AGENTS").map_or(true, |d| d.is_empty()) { return }
    super::models::set_detects();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        loop { let t = std::time::Instant::now(); let changed = refresh(&hub).await; eprintln!("models: detected in {} ms{}", t.elapsed().as_millis(), if changed { " (changed)" } else { "" }); tokio::time::sleep(EVERY).await; }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    const CLAUDE: &str = r#"{"type":"system"}
{"type":"control_response","response":{"subtype":"success","request_id":"models","response":{"models":[{"value":"default","resolvedModel":"claude-opus-5-5","displayName":"Default (recommended)","description":"Opus 5.5","supportedEffortLevels":["low","medium","high","xhigh","max"]},{"value":"opus","resolvedModel":"claude-opus-5-5","displayName":"Opus 5.5","description":"For complex work","supportedEffortLevels":["low","medium","high","xhigh","max"]},{"value":"claude-fable-5-1","resolvedModel":"claude-fable-5-1","displayName":"Fable 5.1","description":"For your toughest challenges","supportedEffortLevels":["low","high"]},{"value":"haiku","resolvedModel":"claude-haiku-4-5","displayName":"Haiku 4.5","description":"Fastest"}]}}}"#;
    #[test] fn claude_lists_its_models_with_labels_and_efforts_and_falls_back_to_the_aliases() {
        let xs = claude_listed(CLAUDE);
        assert_eq!(claude_models(&xs), ["opus", "claude-fable-5-1", "haiku"], "default is the picker's own first row");
        let a = claude_about("2.1.0 (Claude Code)", &xs);
        assert_eq!(a["labels"]["opus"], "Opus 5.5 · claude-opus-5-5"); assert_eq!(a["labels"]["claude-fable-5-1"], "Fable 5.1 · For your toughest challenges", "a full name gets its description");
        assert_eq!(a["efforts"]["default"], json!(["low","medium","high","xhigh","max"])); assert_eq!(a["efforts"]["haiku"], json!([]), "a model without effort takes none");
        assert_eq!(claude_models(&claude_listed("")), ["fable", "opus", "sonnet", "haiku"], "a CLI that cannot say: the known aliases");
        assert_eq!(claude_about("x", &[])["labels"]["opus"], "Opus 5.5 · claude-opus-5-5");
    }
    #[test] fn a_model_the_api_lists_and_the_cli_lacks_is_added_after_its_family() {
        let xs = claude_listed(CLAUDE);
        let api = json!({"data":[{"id":"claude-fable-5-2","display_name":"Claude Fable 5.2"},{"id":"claude-fable-5-1"},{"id":"claude-fable-5"},{"id":"claude-opus-5-5"},{"id":"claude-opus-4-8"},{"id":"claude-haiku-4-5-20251001"},{"id":"claude-sonnet-5-6","display_name":"Claude Sonnet 5.6"}]});
        let ys = claude_extend(xs.clone(), &api);
        assert_eq!(claude_models(&ys), ["opus", "claude-fable-5-1", "claude-fable-5-2", "haiku"], "sonnet is not in the CLI list: not added");
        let a = claude_about("/x/claude", &ys);
        assert_eq!(a["labels"]["claude-fable-5-2"], "Fable 5.2 · For your toughest challenges"); assert_eq!(a["efforts"]["claude-fable-5-2"], json!(["low","high"]));
        assert_eq!(claude_extend(xs.clone(), &Value::Null), xs, "no API answer: the CLI's list as it is");
        let one = claude_extend(claude_listed(&CLAUDE.replace("\"value\":\"claude-fable-5-1\"", "\"value\":\"claude-fable-5-1[1m]\"")), &api);
        assert_eq!(claude_models(&one), ["opus", "claude-fable-5-1[1m]", "claude-fable-5-2[1m]", "haiku"], "the [1m] of the family's row carries over");
    }
    #[test] fn codex_reads_its_cache_and_hides_what_it_hides() {
        let t = r#"{"models":[{"slug":"gpt-6.1-sol","visibility":"list","display_name":"GPT-6.1-Sol","description":"Latest workhorse","supported_reasoning_levels":[{"effort":"low"},{"effort":"high"}]},{"slug":"gpt-reserve","visibility":"hide","display_name":"R"}]}"#;
        assert_eq!(codex_models(t), ["gpt-6.1-sol"]); let a = codex_about("codex 1.2", t);
        assert_eq!(a["labels"]["gpt-6.1-sol"], "GPT-6.1-Sol · Latest workhorse"); assert_eq!(a["labels"]["default"], "Codex's configured default"); assert_eq!(a["efforts"]["gpt-6.1-sol"], json!(["low", "high"]));
        assert!(codex_models("not json").is_empty());
    }
    #[test] fn grok_reads_its_bullets() {
        let o = "You are not authenticated.\n\nDefault model: grok-4.6\n\nAvailable models:\n  - grok-4.7\n  * grok-4.6 (default)\n  - grok-4.5\n";
        assert_eq!(grok_models(o), ["grok-4.7", "grok-4.6", "grok-4.5"]); let a = grok_about("1.0", o);
        assert_eq!(a["labels"]["grok-4.6"], "Grok's default model"); assert_eq!(a["labels"]["grok-4.7"], "Grok"); assert_eq!(a["labels"]["default"], "Grok's own default");
    }
    #[test] fn opencode_keeps_model_lines_and_stored_sign_ins() {
        assert_eq!(opencode_lines("opencode/big-pickle\nnot a model\nanthropic/claude-x\n\n"), "opencode/big-pickle\nanthropic/claude-x\n");
        assert_eq!(opencode_auth("OpenAI      oauth      stored\nAnthropic   api   missing\n"), "!auth\tOpenAI\toauth\n");
        let out = format!("{}{}", opencode_lines("opencode/a\n"), opencode_auth("OpenAI  oauth  stored\n")); assert_eq!(opencode::models(&out), ["opencode/a"]);
    }
    #[test] fn pi_and_omp_list_their_models_and_thinking() {
        let rows = pi_rows("provider model context max-out thinking images\nanthropic claude-x 1M 128K yes yes\nopenai small 1M 128K no no\nshort row\n");
        assert_eq!(rows, [("anthropic/claude-x".to_string(), true), ("openai/small".to_string(), false)]);
        let a = pi_about("pi 1.4", &rows); assert_eq!(a["efforts"]["openai/small"], json!([])); assert_eq!(a["efforts"]["anthropic/claude-x"][0], "off");
        let xs = omp_list(r#"{"models":[{"selector":"openai-codex/gpt-5","name":"GPT-5","thinking":["low","high"]},{"selector":"x/y","name":"Y","thinking":null}]}"#);
        let a = omp_about("omp 2", &xs); assert_eq!(a["labels"]["openai-codex/gpt-5"], "GPT-5 · openai-codex/gpt-5"); assert_eq!(a["efforts"]["openai-codex/gpt-5"], json!(["off", "auto", "low", "high"])); assert_eq!(a["efforts"]["x/y"], json!([]));
    }
    #[test] fn the_info_says_what_is_installed_and_the_cache_reads_back() {
        let none = info_of(&Found::default()); let v: Value = serde_json::from_str(&none).unwrap();
        assert_eq!(v["claude"], json!(["fable", "opus", "sonnet", "haiku"])); assert_eq!(v["codex"], json!([])); assert_eq!(v["about"]["codex"]["cli"], ""); assert_eq!(v["about"]["opencode"]["cli"], ""); assert_eq!(v["probed"], true, "the agents were asked: an empty cli is one not installed");
        let f = Found { codex: Some(("codex 1".into(), r#"{"models":[{"slug":"g","visibility":"list"}]}"#.into())), claude: Some(("c 2".into(), claude_listed(CLAUDE))), ..Default::default() };
        let v: Value = serde_json::from_str(&info_of(&f)).unwrap(); assert_eq!(v["codex"], json!(["g"])); assert_eq!(v["about"]["codex"]["cli"], "codex 1"); assert_eq!(v["claude"][0], "opus");
        let i = info_of(&f); assert_eq!(cache_read(&cache_text(&i)).as_deref(), Some(i.as_str())); assert_eq!(cache_read("{}\n"), None); assert_eq!(cache_read(""), None);
    }
}
