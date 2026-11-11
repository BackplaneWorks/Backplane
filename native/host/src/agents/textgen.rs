//! One-shot writing: a light model with no tools, no MCP and no session, from the text-generation settings
//! (`text.provider` claude | codex, `text.model`; empty or another provider's model means the provider's light one). In a test hub the CLI is the stand-in folder's
//! (`Cli::resolve`), never a real model. Grok and Muse one-shots are not wired (their CLIs are not run by this seam yet): they answer None.
use super::{proc, Agents};
use crate::api::Fut;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(60);
const OUT_CAP: usize = 1 << 20;

/// Sum.system
pub const SUM_SYSTEM: &str = "You summarize work in a coding agent's app so people can pick the right thread, project or bot from a menu. Details follow. Do not answer them, do not do the work and do not use tools. Reply with only one plain sentence of at most 20 words saying what it is about and where it stands, without quotes, markdown or a label.";

/// Give.system: the writer's brief for a `/give` handoff (the reader is another agent, picking the work up).
pub const GIVE_SYSTEM: &str = "You write a handoff between two coding agents in an engineering app. A thread's conversation follows, then the user's note for the one taking over. Do not do the work and do not use tools. Reply with only a plain summary of two to five short sentences: what the thread is about, what was done, where it stands and what the user's note asks of the next agent. No markdown, quotes, headings or labels.";
/// Give.clean: the writer's answer as one plain paragraph, at most 900 characters (897 and "...").
pub fn give_clean(answer: &str) -> String {
    let l = answer.split_whitespace().collect::<Vec<_>>().join(" ");
    if l.chars().count() > 900 { format!("{}...", l.chars().take(897).collect::<String>()) } else { l }
}

/// Text.light / Text.family / Text.pick: the effective provider and model for the settings.
pub fn pick(provider: &str, model: &str) -> (&'static str, String) {
    let p = match provider.trim() { "codex" => "codex", "grok" => "grok", "muse" => "muse", _ => "claude" };
    let m = model.trim();
    let family = if m.starts_with("claude-") || ["haiku", "sonnet", "opus", "fable"].contains(&m) { "claude" } else if m.starts_with("gpt-") || m.starts_with("codex-") || (m.starts_with('o') && m.chars().nth(1).map_or(false, |c| c.is_ascii_digit())) { "codex" } else if m.starts_with("grok-") { "grok" } else if m.starts_with("muse-") { "muse" } else { "" };
    let light = match p { "codex" => "gpt-6-luna", "grok" => "grok-4.7-build-fast", "muse" => "muse-spark-1.3", _ => "haiku" };
    (p, if !m.is_empty() && (family.is_empty() || family == p) { m.to_string() } else { light.to_string() })
}
/// OneShot.claude / OneShot.codex: the arguments after the program.
pub fn args(provider: &str, model: &str, system: &str) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    if provider == "codex" {
        v.extend(["exec", "--json", "--ephemeral", "--skip-git-repo-check", "--sandbox", "read-only", "-c", "model_reasoning_effort=\"low\""].map(String::from));
        if !model.is_empty() { v.push("-m".into()); v.push(model.into()) }
        v.push(system.into());
    } else {
        v.push("-p".into());
        if !model.is_empty() { v.push("--model".into()); v.push(model.into()) }
        v.extend(["--output-format", "json", "--no-session-persistence", "--tools", "", "--strict-mcp-config", "--effort", "low", "--system-prompt"].map(String::from)); v.push(system.into());
    }
    v
}
/// OneShot.answer: claude's last `result` line (not an error), codex's last agent_message.
pub fn answer(provider: &str, out: &str) -> String {
    let mut last = String::new();
    for l in out.lines() {
        let Ok(o) = serde_json::from_str::<Value>(l.trim()) else { continue };
        if provider == "codex" { if o["type"] == "item.completed" && o["item"]["type"] == "agent_message" { last = o["item"]["text"].as_str().unwrap_or("").to_string() } }
        else if o["type"] == "result" && o["is_error"] != true { last = o["result"].as_str().unwrap_or("").to_string() }
    }
    if provider != "codex" && last.is_empty() { // claude may print one JSON document
        if let Ok(o) = serde_json::from_str::<Value>(out.trim()) { if o["type"] == "result" && o["is_error"] != true { last = o["result"].as_str().unwrap_or("").to_string() } }
    }
    last.trim().to_string()
}
/// Why a one-shot's output holds no answer: claude's error result (a limit, a sign-in), else "no answer".
pub fn failure(out: &str) -> String {
    let mut why = String::new();
    for l in out.lines() { if let Ok(o) = serde_json::from_str::<Value>(l.trim()) { if o["type"] == "result" && o["is_error"] == true { why = o["result"].as_str().unwrap_or("an error result").to_string(); if o["api_error_status"].is_number() { why = format!("{why} ({})", o["api_error_status"]) } } } }
    if why.is_empty() { "no answer".into() } else { why.chars().take(300).collect() }
}
/// A failure that is a usage limit (the login is skipped for an hour), not a passing error.
pub fn is_limit(why: &str) -> bool { let l = why.to_lowercase(); l.contains("limit") || l.contains("(429)") }
fn login_key(id: &str, epoch: u64) -> String { format!("{id}:{epoch}") }
/// Logins a one-shot found out (key `id:epoch`, "" the default login) and until when; in memory only: a restart tries them again.
fn limited_logins() -> &'static std::sync::Mutex<std::collections::HashMap<String, u64>> { static M: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, u64>>> = std::sync::OnceLock::new(); M.get_or_init(Default::default) }
/// The logins a one-shot tries, in order: the default login (id "") unless held, then the named Claude accounts by allowance left; none a one-shot found out within the hour.
pub fn healthy_order(profiles: &[Value], get: &dyn Fn(&str) -> String, now: u64, model: &str, out: &std::collections::HashMap<String, u64>) -> Vec<(String, u64)> {
    let gone = |id: &str, epoch: u64| out.get(&login_key(id, epoch)).map_or(false, |until| *until > now);
    let mut v = vec![];
    if !crate::settings::fallback::hold_current(get, "claude", "", 0, model, now) && !gone("", 0) { v.push((String::new(), 0)) }
    let mut tried: Vec<String> = vec![];
    while let Some(p) = crate::settings::fallback::pick_untried(profiles, &tried, get, now, "claude", model) {
        let (id, epoch) = (p["id"].as_str().unwrap_or("").to_string(), p["epoch"].as_u64().unwrap_or(0));
        tried.push(crate::settings::fallback::tried_id(&id, epoch));
        if !gone(&id, epoch) { v.push((id, epoch)) }
    }
    v
}
/// Sum.clean: the first line, one plain line of single-spaced words, at most 200 characters (197 and "...").
pub fn sum_clean(answer: &str) -> String {
    let first = answer.trim().lines().next().unwrap_or("").trim().replace('\t', " ");
    let l = first.split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    if l.chars().count() > 200 { format!("{}...", l.chars().take(197).collect::<String>()) } else { l }
}

/// Title.system
pub const TITLE_SYSTEM: &str = "You title threads in a coding agent's sidebar. The user's first message to the agent follows. Do not answer it, do not do the work and do not use tools. Reply with only a title of 3 to 7 words (under 50 characters) naming the change or question the user wants, in sentence case, without quotes, markdown or a trailing period.";
/// The model that titles threads (the person's Claude login runs it); Codex's light model is the fallback.
pub const TITLE_MODEL: &str = "claude-haiku-5-5";

/// Title.input: what the title writer reads. The message is fenced and the request comes after it: handed bare, a short question ("what is this board")
/// reads as one put to the writer, and it answered it, refused it, or titled its own instructions ("Board title request: 3-7 words...").
pub fn title_input(text: &str) -> String {
    let t: String = text.chars().take(4000).collect();
    format!("<user_message>\n{}\n</user_message>\n\nWrite the title for the thread this starts. Reply with the title only.", t.trim())
}

/// Title.clean: the answer's first line without markdown marks, quotes, a "Title:" label or a trailing period, spaces collapsed, 60 characters at most ("" for none).
pub fn title_clean(answer: &str) -> String {
    let first = answer.trim().lines().next().unwrap_or("").trim();
    let bare: String = first.chars().filter(|c| !matches!(c, '`' | '"' | '*')).collect();
    let mut l = bare.trim_start_matches(|c| c == '#' || c == ' ').to_string();
    if l.to_lowercase().starts_with("title:") { l = l[6..].trim().to_string() }
    if l.chars().count() >= 2 && l.starts_with('\'') && l.ends_with('\'') { l = l[1..l.len() - 1].trim().to_string() }
    let mut l = l.split_whitespace().collect::<Vec<_>>().join(" ");
    if l.ends_with('.') && !l.ends_with("..") { l.pop(); l = l.trim().to_string() }
    if l.chars().count() > 60 { format!("{}...", l.chars().take(57).collect::<String>()) } else { l }
}

/// Btw.system
const BTW_SYSTEM: &str = "You answer a quick side question about a coding agent's thread. The thread so far and the question follow. Answer from what the thread shows, in a few sentences, in plain text. Do not do the work, do not use tools and do not repeat the question. If the thread does not tell, say so.";

/// How a one-shot is run: the program and arguments to put ahead, and the environment (a named account's profile).
#[derive(Default, Clone)]
pub struct Launch { pub program: Option<std::path::PathBuf>, pub args: Vec<String>, pub env: Vec<(String, String)> }

impl Agents {
    /// Write one piece of text with the settings' light model; None when it cannot be run or says nothing. Bounded: 60 s, 1 MB.
    pub async fn one_shot(&self, system: &str, input: &str) -> Option<String> {
        let (tp, tm) = { let w = self.world().await; (w.setting("text.provider").to_string(), w.setting("text.model").to_string()) };
        let (provider, model) = pick(&tp, &tm);
        self.shot(provider, &model, system, input, &Launch::default()).await
    }
    /// One run of `provider`'s CLI with `model`, the text on stdin, tools off.
    pub async fn shot(&self, provider: &str, model: &str, system: &str, input: &str, via: &Launch) -> Option<String> { self.shot_r(provider, model, system, input, via).await.ok() }
    /// `shot`, with why it gave nothing: the CLI's own error result ("You've hit your weekly limit · resets 3am"), a timeout, or no answer.
    pub async fn shot_r(&self, provider: &str, model: &str, system: &str, input: &str, via: &Launch) -> Result<String, String> {
        if provider != "claude" && provider != "codex" { return Err(format!("{provider} one-shots are not wired")) }
        let prog = match &via.program { Some(p) => p.clone(), None => self.cli.resolve(provider)? };
        let mut a = via.args.clone(); a.extend(args(provider, model, system));
        let p = proc::spawn(&prog, &a, &self.hub.cfg.home, self.cli.dir(), &via.env).map_err(|e| format!("could not start {provider}: {e}"))?;
        let (pid, mut lines, input_tx, exit) = (p.pid, p.lines, p.input, p.exit);
        let _ = input_tx.try_send(input.to_string()); drop(input_tx); // the text on stdin, then it closes
        let read = async { let mut out = String::new(); while let Some(l) = lines.recv().await { if out.len() + l.len() > OUT_CAP { break } out.push_str(&l); out.push('\n') } out };
        let out = match tokio::time::timeout(TIMEOUT, read).await { Ok(o) => o, Err(_) => { proc::terminate(pid, Duration::from_secs(1)); return Err("no answer in 60 s".into()) } };
        let _ = tokio::time::timeout(Duration::from_secs(5), exit).await; proc::terminate(pid, Duration::from_millis(100));
        let a = answer(provider, &out); if a.is_empty() { Err(failure(&out)) } else { Ok(a) }
    }
    /// A one-shot on a Claude login that can take it, in the failover's order: the CLI's own login unless it is held (a limit a turn recorded, or one
    /// a one-shot met within the hour), then the named accounts, the one with the most allowance left first (settings/fallback.rs `pick_untried`).
    /// A login that answers with a limit is skipped by the next one-shots for an hour. `who` names the job in the hub's log. None when every login failed.
    pub async fn shot_healthy(&self, model: &str, system: &str, input: &str, who: &str) -> Option<String> {
        let now = super::now_s();
        let (profiles, holds) = {
            let profiles = self.accounts.get().map(|m| m.snapshot()["profiles"].as_array().cloned().unwrap_or_default()).unwrap_or_default();
            let keys: Vec<String> = { let m = self.hub.model().await; m.settings_with_prefix("account.limit.").map(|(k, _)| k.to_string()).collect() };
            let mut h = std::collections::HashMap::new(); for k in keys { if let Some(v) = self.hub.setting(&k).await { h.insert(k, v); } }
            (profiles, h)
        };
        let out = limited_logins().lock().unwrap().clone();
        let order = healthy_order(&profiles, &|k: &str| holds.get(k).cloned().unwrap_or_default(), now, model, &out);
        if order.is_empty() { eprintln!("{who}: every Claude login is out; asked again later"); return None }
        for (id, epoch) in order {
            let name = if id.is_empty() { "the default login".to_string() } else { profiles.iter().find(|p| p["id"] == id.as_str()).and_then(|p| p["label"].as_str()).unwrap_or("an account").to_string() };
            let via = if id.is_empty() { Launch::default() } else {
                let Some(mgr) = self.accounts.get().cloned() else { continue };
                match mgr.launch_plan(&id, &epoch.to_string(), "claude").await { Ok(pl) => Launch { program: Some(pl.program), args: pl.args, env: pl.env }, Err(e) => { eprintln!("{who}: {name} cannot run it ({e})"); continue } }
            };
            match self.shot_r("claude", model, system, input, &via).await {
                Ok(a) => return Some(a),
                Err(e) => { eprintln!("{who}: {name} gave nothing: {e}"); if is_limit(&e) { limited_logins().lock().unwrap().insert(login_key(&id, epoch), now + 3600); } }
            }
        }
        None
    }
    /// A side question (/btw): the writing model reads the thread so far and the question; the thread, its turn and the log never see either. The answer, or "" (the client says where the model is chosen).
    /// Under the thread's own account when it has one for that provider, else the default login (TextScope).
    pub async fn side_answer(&self, thread: &str, q: &str) -> String {
        let (title, tp, tm) = { let w = self.world().await; (w.threads.get(thread).map(|t| t.title.clone()).unwrap_or_default(), w.setting("text.provider").to_string(), w.setting("text.model").to_string()) };
        let cx = crate::commands::recent(&self.hub, thread).await;
        let why = format!("The user asks a side question about the thread \"{title}\" ({thread}) while its agent works. Its conversation so far, oldest first:");
        let input = format!("{}\n\n{q}", crate::commands::hand_text(&why, &cx.entries, thread));
        let (provider, model) = pick(&tp, &tm);
        let raw = self.hub.setting(&crate::settings::accounts::binding_key(thread, provider)).await.unwrap_or_default();
        let b: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        let id = b["id"].as_str().unwrap_or("").to_string();
        if !id.is_empty() { if let Some(mgr) = self.accounts.get().cloned() {
            if let Ok(pl) = mgr.launch_plan(&id, &b["epoch"].as_u64().unwrap_or(0).to_string(), provider).await {
                if let Some(a) = self.shot(provider, &model, BTW_SYSTEM, &input, &Launch { program: Some(pl.program), args: pl.args, env: pl.env }).await { return a }
            }
        } }
        self.shot(provider, &model, BTW_SYSTEM, &input, &Launch::default()).await.unwrap_or_default()
    }
    /// A thread's title from `text`: the Writing setting's provider and model when the person chose one, else Haiku under the thread's own account (else the default login), then Haiku on the default login, then Codex's light model.
    /// The cleaned title, or None (each failed attempt is logged).
    pub async fn write_title(&self, thread: &str, text: &str) -> Option<String> {
        if text.trim().is_empty() { return None }
        let input = title_input(text);
        let mut tries: Vec<(&str, String, Launch, &str)> = vec![];
        // Settings > Writing: a provider or model the person chose is what names threads (the page says so); with neither set, Haiku as before
        let (tp, tm) = { let w = self.world().await; (w.setting("text.provider").to_string(), w.setting("text.model").to_string()) };
        if !tp.trim().is_empty() || !tm.trim().is_empty() { let (p, m) = pick(&tp, &tm); if p == "claude" || p == "codex" { tries.push((p, m, Launch::default(), "the Writing setting")) } }
        let raw = self.hub.setting(&crate::settings::accounts::binding_key(thread, "claude")).await.unwrap_or_default();
        let b: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        let id = b["id"].as_str().unwrap_or("").to_string();
        if !id.is_empty() {
            if let Some(mgr) = self.accounts.get().cloned() {
                match mgr.launch_plan(&id, &b["epoch"].as_u64().unwrap_or(0).to_string(), "claude").await {
                    Ok(pl) => tries.push(("claude", TITLE_MODEL.into(), Launch { program: Some(pl.program), args: pl.args, env: pl.env }, "the thread's account")),
                    Err(e) => eprintln!("titles: {thread}: the thread's account cannot run a title ({e}); using the default login") }
            }
        }
        for (prov, model, via, what) in tries {
            match self.shot(prov, &model, TITLE_SYSTEM, &input, &via).await {
                Some(a) => { let t = title_clean(&a); if !t.is_empty() { return Some(t) } eprintln!("titles: {thread}: {what} answered nothing usable") }
                None => eprintln!("titles: {thread}: {what} ({prov} {model}) gave no title"),
            }
        }
        // then Haiku on whichever Claude login can take it (the default login, else the named accounts: the failover's order), then Codex's light model
        if let Some(a) = self.shot_healthy(TITLE_MODEL, TITLE_SYSTEM, &input, &format!("titles: {thread}")).await { let t = title_clean(&a); if !t.is_empty() { return Some(t) } }
        match self.shot("codex", &pick("codex", "").1, TITLE_SYSTEM, &input, &Launch::default()).await { Some(a) => { let t = title_clean(&a); if !t.is_empty() { return Some(t) } } None => eprintln!("titles: {thread}: codex gave no title") }
        None
    }
    /// The text a title is written from: the first message; for a regeneration also the newest few, so a thread that moved on is titled for where it is.
    async fn title_source(&self, thread: &str, regenerate: bool) -> String {
        let Some(c) = crate::commands::entries(&self.hub, thread).await else { return String::new() };
        let mine: Vec<&crate::commands::Line> = c.entries.iter().filter(|l| l.msg && l.role == "user").collect(); // newest first
        let Some(first) = mine.last() else { return String::new() };
        let clip = |t: &str, n: usize| t.chars().take(n).collect::<String>();
        if !regenerate || mine.len() == 1 { return clip(&first.text, 4000) }
        let recent: Vec<String> = mine.iter().take(3).rev().map(|l| clip(&l.text, 600)).collect();
        format!("{}\n\nLater messages:\n{}", clip(&first.text, 1800), recent.join("\n---\n"))
    }
    /// Retitle a thread with the written title. `auto` (after a thread's first message): only while the thread still has the title it was given then
    /// (`placeholder`) and the person has not typed one. A regeneration replaces any title unless the person typed it and `force` is not set.
    /// Bots and the uberbot are left alone. Ok(title) once the event is logged; Err says why not (and is logged).
    pub async fn retitle(&self, thread: &str, auto: Option<&str>, force: bool) -> Result<String, String> {
        let (title, bot) = { let w = self.world().await; (w.threads.get(thread).map(|t| t.title.clone()), !w.bot(thread).is_empty()) };
        let Some(title) = title else { return Err("unknown thread".into()) };
        if bot { return Err("a bot's chat keeps its title".into()) }
        let typed = |ag: &Agents| { let (h, k) = (ag.hub.clone(), format!("title.person.{thread}")); async move { !h.setting(&k).await.unwrap_or_default().is_empty() } };
        if auto.is_none() && !force && typed(self).await { return Err("the person titled this thread; give force or rename it yourself".into()) }
        let text = match auto { Some(_) => self.title_source(thread, false).await, None => self.title_source(thread, true).await };
        let _ = title;
        let r = match self.write_title(thread, &text).await { Some(t) => t, None => { let e = "no title could be written (the model gave none)".to_string(); eprintln!("titles: {thread}: {e}"); return Err(e) } };
        let (now, cur) = (super::now_s(), { let w = self.world().await; w.threads.get(thread).map(|t| t.title.clone()).unwrap_or_default() });
        if let Some(ph) = auto { if cur != ph || typed(self).await { return Err("the title changed meanwhile; kept".into()) } }
        let mut ev = vec![json!({"$":"ThreadRenamed","id":thread,"title":r,"at":now})];
        if auto.is_none() { ev.push(json!({"$":"SettingSet","key":format!("title.person.{thread}"),"value":"","at":now})) }
        self.commit_events(ev).await; Ok(r)
    }
}

/// The text model behind thread and project summaries (H.Sum / SumJob). `text` is the writer's whole input (Sum.thread.input / Sum.project.input, built by the caller);
/// the answer is cleaned (Sum.clean). Async, may take seconds, None on any failure; the caller writes `summary.<id>` and `summary.at.<id>`.
pub trait Summarizer: Send + Sync {
    fn summarize<'a>(&'a self, kind: &'a str, id: &'a str, text: &'a str) -> Fut<'a, Option<String>>;
}
impl Summarizer for Agents {
    fn summarize<'a>(&'a self, _kind: &'a str, _id: &'a str, text: &'a str) -> Fut<'a, Option<String>> {
        Box::pin(async move { self.one_shot(SUM_SYSTEM, text).await.map(|a| sum_clean(&a)).filter(|a| !a.is_empty()) })
    }
}
/// `agents::summarizer()`: the hub's summarizer, or None before the agents start.
pub fn summarizer() -> Option<Arc<dyn Summarizer>> { super::handle().map(|a| a as Arc<dyn Summarizer>) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_one_shot_tries_the_default_login_then_the_accounts_with_most_allowance_and_skips_one_found_out() {
        let now = 1_000_000u64;
        let prof = |id: &str, used: f64| json!({"id":id,"epoch":2,"provider":"claude","state":"connected","allowance":{"state":"ok","at":now - 10,"windows":[{"label":"Weekly","used_percent":used,"resets_at":now + 9999}]}});
        let ps = vec![prof("work", 60.0), prof("home", 5.0), json!({"id":"codex1","epoch":1,"provider":"codex","state":"connected"}), json!({"id":"gone","epoch":1,"provider":"claude","state":"expired"})];
        let none = std::collections::HashMap::new();
        assert_eq!(healthy_order(&ps, &|_| String::new(), now, TITLE_MODEL, &none), vec![(String::new(), 0), ("home".into(), 2), ("work".into(), 2)], "default, then most allowance left; no codex, no expired login");
        // the default login out for the week (a one-shot met its limit): the accounts take it, as the person's live hub needed on Nov 11
        let mut out = std::collections::HashMap::new(); out.insert(":0".to_string(), now + 3600);
        assert_eq!(healthy_order(&ps, &|_| String::new(), now, TITLE_MODEL, &out), vec![("home".to_string(), 2), ("work".into(), 2)]);
        out.insert("home:2".into(), now - 1); assert_eq!(healthy_order(&ps, &|_| String::new(), now, TITLE_MODEL, &out).len(), 2, "an hour later it is tried again");
        // a hold a turn recorded on the default login counts too
        let held = crate::settings::fallback::hold_key("claude", "", 0); let rec = json!({"scope":"claude","reset":now + 600,"at":now - 5}).to_string();
        let got = healthy_order(&ps, &|k: &str| if k == held { rec.clone() } else { String::new() }, now, TITLE_MODEL, &none);
        assert_eq!(got, vec![("home".to_string(), 2), ("work".into(), 2)], "a held default login is skipped");
    }
    #[test] fn a_one_shots_failure_says_why_and_a_limit_is_told_apart() {
        let out = r#"{"type":"result","subtype":"success","is_error":true,"result":"You've hit your weekly limit · resets 3am (America/New_York)","api_error_status":429}"#;
        assert_eq!(answer("claude", out), ""); let w = failure(out); assert!(w.starts_with("You've hit your weekly limit") && w.ends_with("(429)"), "{w}"); assert!(is_limit(&w));
        assert_eq!(failure(""), "no answer"); assert!(!is_limit("no answer in 60 s"));
    }
    #[test] fn the_light_model_is_the_providers_unless_the_setting_fits_it() {
        assert_eq!(pick("", ""), ("claude", "haiku".into())); assert_eq!(pick("codex", "haiku"), ("codex", "gpt-6-luna".into())); assert_eq!(pick("codex", "gpt-5.5"), ("codex", "gpt-5.5".into())); assert_eq!(pick("claude", "sonnet"), ("claude", "sonnet".into())); assert_eq!(pick("nope", "x-unknown"), ("claude", "x-unknown".into()));
    }
    #[test] fn the_arguments_keep_the_model_tool_free_and_sessionless() {
        let c = args("claude", "haiku", "SYS"); assert_eq!(c[0], "-p"); assert!(c.windows(2).any(|w| w == ["--tools", ""]) && c.contains(&"--no-session-persistence".to_string()) && c.windows(2).any(|w| w == ["--system-prompt", "SYS"]));
        let x = args("codex", "", "SYS"); assert_eq!(x.last().unwrap(), "SYS"); assert!(x.contains(&"--ephemeral".to_string()) && !x.contains(&"-m".to_string()));
    }
    #[test] fn a_written_title_is_one_plain_line() {
        assert_eq!(title_clean("  \"Fix the Haiku titler.\"\nmore"), "Fix the Haiku titler"); assert_eq!(title_clean("## Title: **Auto-settle merged threads**"), "Auto-settle merged threads");
        assert_eq!(title_clean("'quoted'"), "quoted"); assert_eq!(title_clean("Wait..."), "Wait..."); assert_eq!(title_clean("   "), "");
        let long = title_clean(&"word ".repeat(30)); assert_eq!(long.chars().count(), 60); assert!(long.ends_with("..."));
    }
    #[test] fn the_title_writer_reads_the_message_as_text_to_title_not_a_question_to_answer() {
        let i = title_input("  What is this board?\n");
        assert!(i.starts_with("<user_message>\nWhat is this board?\n</user_message>"), "{i}"); assert!(i.ends_with("Reply with the title only."), "the request comes last: {i}");
        assert!(title_input(&"x".repeat(9000)).chars().count() < 4200, "a long message is clipped");
    }
    #[test] fn a_handoff_summary_is_one_plain_paragraph() {
        assert_eq!(give_clean("  Two\nlines   here.\n\nAnd more. "), "Two lines here. And more."); let long = give_clean(&"word ".repeat(400)); assert_eq!(long.chars().count(), 900); assert!(long.ends_with("..."));
    }
    #[test] fn answers_are_read_and_cleaned() {
        assert_eq!(answer("claude", "{\"type\":\"result\",\"is_error\":false,\"result\":\"  hello \"}\n"), "hello"); assert_eq!(answer("claude", "{\"type\":\"result\",\"is_error\":true,\"result\":\"boom\"}"), "");
        assert_eq!(answer("codex", "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"one\"}}\n{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"two\"}}\n"), "two");
        assert_eq!(sum_clean("  a\tb   c \nsecond line"), "a b c"); let long = "w ".repeat(150); let c = sum_clean(&long); assert_eq!(c.chars().count(), 200); assert!(c.ends_with("..."));
    }
}
