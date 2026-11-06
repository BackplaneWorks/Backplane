//! One-shot writing (agent.bend OneShot.*, server.bend OneShot.run): a light model with no tools, no MCP and no session, from the text-generation settings
//! (`text.provider` claude | codex, `text.model`; empty or another provider's model means the provider's light one). In a test hub the CLI is the stand-in folder's
//! (`Cli::resolve`), never a real model. Grok and Muse one-shots are not wired (their CLIs are not run by this seam yet): they answer None.
use super::{proc, Agents};
use crate::api::Fut;
use serde_json::Value;
use std::{sync::Arc, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(60);
const OUT_CAP: usize = 1 << 20;

/// Sum.system
pub const SUM_SYSTEM: &str = "You summarize work in a coding agent's app so people can pick the right thread, project or bot from a menu. Details follow. Do not answer them, do not do the work and do not use tools. Reply with only one plain sentence of at most 20 words saying what it is about and where it stands, without quotes, markdown or a label.";

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
/// Sum.clean: the first line, one plain line of single-spaced words, at most 200 characters (197 and "...").
pub fn sum_clean(answer: &str) -> String {
    let first = answer.trim().lines().next().unwrap_or("").trim().replace('\t', " ");
    let l = first.split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    if l.chars().count() > 200 { format!("{}...", l.chars().take(197).collect::<String>()) } else { l }
}

impl Agents {
    /// Write one piece of text with the settings' light model; None when it cannot be run or says nothing. Bounded: 60 s, 1 MB.
    pub async fn one_shot(&self, system: &str, input: &str) -> Option<String> {
        let (tp, tm) = { let w = self.world().await; (w.setting("text.provider").to_string(), w.setting("text.model").to_string()) };
        let (provider, model) = pick(&tp, &tm);
        if provider != "claude" && provider != "codex" { return None }
        let prog = self.cli.resolve(provider).ok()?;
        let p = proc::spawn(&prog, &args(provider, &model, system), &self.hub.cfg.home, self.cli.dir(), &[]).ok()?;
        let (pid, mut lines, input_tx, exit) = (p.pid, p.lines, p.input, p.exit);
        let _ = input_tx.try_send(input.to_string()); drop(input_tx); // the text on stdin, then it closes
        let read = async { let mut out = String::new(); while let Some(l) = lines.recv().await { if out.len() + l.len() > OUT_CAP { break } out.push_str(&l); out.push('\n') } out };
        let out = match tokio::time::timeout(TIMEOUT, read).await { Ok(o) => o, Err(_) => { proc::terminate(pid, Duration::from_secs(1)); return None } };
        let _ = tokio::time::timeout(Duration::from_secs(5), exit).await; proc::terminate(pid, Duration::from_millis(100));
        let a = answer(provider, &out); if a.is_empty() { None } else { Some(a) }
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
    #[test] fn the_light_model_is_the_providers_unless_the_setting_fits_it() {
        assert_eq!(pick("", ""), ("claude", "haiku".into())); assert_eq!(pick("codex", "haiku"), ("codex", "gpt-6-luna".into())); assert_eq!(pick("codex", "gpt-5.5"), ("codex", "gpt-5.5".into())); assert_eq!(pick("claude", "sonnet"), ("claude", "sonnet".into())); assert_eq!(pick("nope", "x-unknown"), ("claude", "x-unknown".into()));
    }
    #[test] fn the_arguments_keep_the_model_tool_free_and_sessionless() {
        let c = args("claude", "haiku", "SYS"); assert_eq!(c[0], "-p"); assert!(c.windows(2).any(|w| w == ["--tools", ""]) && c.contains(&"--no-session-persistence".to_string()) && c.windows(2).any(|w| w == ["--system-prompt", "SYS"]));
        let x = args("codex", "", "SYS"); assert_eq!(x.last().unwrap(), "SYS"); assert!(x.contains(&"--ephemeral".to_string()) && !x.contains(&"-m".to_string()));
    }
    #[test] fn answers_are_read_and_cleaned() {
        assert_eq!(answer("claude", "{\"type\":\"result\",\"is_error\":false,\"result\":\"  hello \"}\n"), "hello"); assert_eq!(answer("claude", "{\"type\":\"result\",\"is_error\":true,\"result\":\"boom\"}"), "");
        assert_eq!(answer("codex", "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"one\"}}\n{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"two\"}}\n"), "two");
        assert_eq!(sum_clean("  a\tb   c \nsecond line"), "a b c"); let long = "w ".repeat(150); let c = sum_clean(&long); assert_eq!(c.chars().count(), 200); assert!(c.ends_with("..."));
    }
}
