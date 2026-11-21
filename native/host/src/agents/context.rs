//! Worktree context shared by launch and the thread inspector.
use serde::Serialize;
use std::{fs, io::Read, path::Path};

const FILE_LIMIT: usize = 8 * 1024;
const TOTAL_LIMIT: usize = 32 * 1024;
const NAMES: &[&str] = &[
    "AGENTS.md", "AGENTS.override.md", ".claude/AGENTS.md", "CLAUDE.md", ".claude/CLAUDE.md", "CLAUDE.local.md",
    "GEMINI.md", ".backplane/instructions.md", ".cursorrules",
    ".github/copilot-instructions.md",
];

#[derive(Serialize)]
pub struct Instruction {
    pub path: String,
    pub bytes: u64,
    pub estimated_tokens: u64,
    pub status: &'static str,
    pub preview: String,
    pub note: String,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum ClaudeFiles {
    #[default]
    ClaudeOnly,
    Fallback,
    Both,
    ManagedOnly,
}

impl ClaudeFiles {
    pub fn configured(version: &str, settings: &serde_json::Value) -> Self {
        if !crate::settings::agent_versions::parse(version).is_some_and(|v| v.as_slice() >= &[2, 1, 277]) {
            return Self::ClaudeOnly
        }
        for plugin in ["cc-plugin-agents-md@builtin", "agents-md@builtin"] {
            if settings["enabledPlugins"][plugin] == false { return Self::ClaudeOnly }
            match settings["pluginConfigs"][plugin]["options"]["instructionFiles"].as_str() {
                Some("claude-md") => return Self::ClaudeOnly,
                Some("managed-only") => return Self::ManagedOnly,
                Some("claude-md-and-agents-md") => return Self::Both,
                _ => {}
            };
        }
        Self::Fallback
    }
}

fn disposition(provider: &str, name: &str, dir: &Path, has_agents: bool, claude: ClaudeFiles, has_claude: bool) -> &'static str {
    let overridden = dir.join("AGENTS.override.md").is_file();
    let agents = dir.join("AGENTS.md").is_file();
    match (provider, name) {
        ("claude", "CLAUDE.md" | ".claude/CLAUDE.md" | "CLAUDE.local.md") => {
            if claude == ClaudeFiles::ManagedOnly { "Skipped" } else { "Native" }
        }
        ("claude", "AGENTS.md" | ".claude/AGENTS.md") if claude == ClaudeFiles::Both || claude == ClaudeFiles::Fallback && !has_claude => "Native",
        ("claude", _) if claude == ClaudeFiles::ManagedOnly => "Skipped",
        (_, "CLAUDE.local.md") => "Skipped",

        ("antigravity", "AGENTS.md" | "GEMINI.md") |
        ("grok" | "muse", "AGENTS.md") |
        ("codex" | "pi", "AGENTS.override.md") => "Native",
        ("omp", "AGENTS.md") | ("omp", "CLAUDE.md") if !dir.join(".claude/CLAUDE.md").is_file() => "Native",
        ("omp", ".cursorrules") => "Native",
        ("codex" | "pi", "AGENTS.md") if overridden => "Skipped",
        ("codex" | "pi" | "opencode", "AGENTS.md") => "Native",
        ("pi", "CLAUDE.md") if !overridden && !agents => "Native",
        ("opencode", "CLAUDE.md") if !has_agents && std::env::var_os("OPENCODE_DISABLE_CLAUDE_CODE_PROMPT").is_none() && std::env::var_os("OPENCODE_DISABLE_CLAUDE_CODE").is_none() => "Native",
        _ => "Injected",
    }
}

#[cfg(test)]
fn instructions(cwd: &str, provider: &str) -> (Vec<Instruction>, String) {
    instructions_for(cwd, provider, ClaudeFiles::default())
}

fn instructions_for(cwd: &str, provider: &str, claude: ClaudeFiles) -> (Vec<Instruction>, String) {
    let dirs = super::skills::directories(Path::new(cwd));
    let root = dirs.first();
    let has_agents = dirs.iter().any(|p| p.join("AGENTS.md").is_file());
    let has_claude = dirs.last().is_some_and(|cwd| cwd.ancestors().any(|p| {
        ["CLAUDE.md", ".claude/CLAUDE.md", "CLAUDE.local.md"].iter().any(|name| p.join(name).is_file())
    }));
    let mut candidates = Vec::new();
    let mut native = std::collections::HashSet::new();
    for dir in dirs.iter().rev() {
        for name in NAMES.iter().rev() {
            let path = dir.join(name);
            let Ok(real) = crate::plat::fs::canonicalize(&path) else { continue };
            if !root.is_some_and(|r| real.starts_with(r)) { continue }
            let Ok(meta) = fs::metadata(&real) else { continue };
            if !meta.is_file() { continue }
            let status = disposition(provider, name, dir, has_agents, claude, has_claude);
            if status == "Native" { native.insert(real.clone()); }
            candidates.push((path, real, meta.len(), status));
        }
    }
    if provider == "claude" {
        // A standard Claude import (for example @AGENTS.md) is itself native context.
        let known: std::collections::HashSet<_> = candidates.iter().map(|(_, real, _, _)| real.clone()).collect();
        let mut pending: Vec<_> = native.iter().cloned().collect();
        while let Some(p) = pending.pop() {
            let mut bytes = Vec::new();
            if fs::File::open(&p).and_then(|f| f.take(FILE_LIMIT as u64).read_to_end(&mut bytes)).is_err() { continue }
            let Ok(text) = std::str::from_utf8(&bytes) else { continue };
            for line in text.lines() {
                let Some(import) = line.trim().strip_prefix('@') else { continue };
                if import.is_empty() || import.chars().any(char::is_whitespace) { continue }
                let Some(dir) = p.parent() else { continue };
                let Ok(target) = crate::plat::fs::canonicalize(dir.join(import)) else { continue };
                if known.contains(&target) && native.insert(target.clone()) { pending.push(target); }
            }
        }
    }
    let mut rows = Vec::new();
    let mut injected = String::new();
    // Reserve space for closer instructions before allocating the root's budget.
    let mut remaining = TOTAL_LIMIT;
    for (path, real, size, mut status) in candidates {
        let path_str = path.to_string_lossy().into_owned();
        let alias = status == "Injected" && native.contains(&real);
        if alias { status = "Skipped"; }
        let limit = if status == "Injected" { FILE_LIMIT.min(remaining) } else { FILE_LIMIT };
        let mut bytes = Vec::new();
        let read = fs::File::open(&real).and_then(|f| f.take(limit as u64 + 1).read_to_end(&mut bytes));
        let mut note = if read.is_err() {
            status = "Skipped";
            format!("Could not read {path_str}")
        } else if alias {
            "The provider already loads this file by another path".into()
        } else if limit == 0 {
            status = "Skipped";
            format!("Context limit reached before {path_str}")
        } else if status == "Skipped" {
            if claude == ClaudeFiles::ManagedOnly { "The provider is configured for managed instructions only".into() }
            else if path.ends_with("CLAUDE.local.md") { "Local Claude instructions".into() }
            else { "AGENTS.override.md takes precedence".into() }
        } else if size > limit as u64 || bytes.len() > limit {
            if status == "Native" {
                format!("Preview truncated at {limit} bytes. The provider decides its own limit.")
            } else { format!("Truncated {path_str} at {limit} bytes") }
        } else { String::new() };
        bytes.truncate(limit);
        let preview = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(e) if e.error_len().is_none() && size > bytes.len() as u64 => {
                std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or_default()
            }
            Err(_) => {
                status = "Skipped";
                note = format!("{path_str} is not UTF-8");
                ""
            }
        }.trim_start_matches('\u{feff}').to_string();
        if status == "Injected" { remaining -= preview.len(); }
        rows.push(Instruction { path: path_str, bytes: size, estimated_tokens: size.div_ceil(4), status, preview, note });
    }
    rows.reverse();
    for row in rows.iter().filter(|r| r.status == "Injected" || r.note.starts_with("Context limit reached")) {
        use std::fmt::Write;
        if row.status == "Injected" {
            let _ = writeln!(injected, "\n## {}\n\n{}", row.path, row.preview);
        }
        if !row.note.is_empty() { let _ = writeln!(injected, "\n[{}]", row.note); }
    }
    // Bound the extra markup as well as the file bodies.
    if injected.len() > TOTAL_LIMIT + FILE_LIMIT {
        let end = injected.floor_char_boundary(TOTAL_LIMIT + FILE_LIMIT);
        injected.truncate(end);
        injected.push_str("\n[Project instructions reached the context limit.]\n");
    }
    if !injected.is_empty() { injected = format!("<project_instructions>\n{injected}</project_instructions>"); }
    (rows, injected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_worktree_context_stops_at_its_own_root() {
        let main = crate::plat::tmp::dir("context-worktree");
        let work = main.join(".backplane/worktrees/thread");
        let cwd = work.join("src");
        fs::create_dir_all(&cwd).unwrap();
        fs::write(main.join(".git"), "").unwrap();
        fs::write(main.join("AGENTS.md"), "MAIN ONLY").unwrap();
        fs::write(work.join(".git"), "gitdir: ignored").unwrap();
        fs::write(work.join("AGENTS.md"), "WORKTREE").unwrap();
        fs::write(cwd.join("CLAUDE.md"), "LOCAL").unwrap();
        for (provider, agents_native, claude_native) in [
            ("claude", false, true), ("codex", true, false), ("antigravity", true, false),
            ("grok", true, false), ("opencode", true, false), ("pi", true, true),
            ("omp", true, true),
        ] {
            let (rows, prompt) = instructions(&cwd.to_string_lossy(), provider);
            assert_eq!(rows.len(), 2, "{provider}");
            assert_eq!(prompt.contains("WORKTREE"), !agents_native, "{provider}");
            assert_eq!(prompt.contains("LOCAL"), !claude_native, "{provider}");
            assert!(!prompt.contains("MAIN ONLY"));
        }
    }

    #[test]
    fn claude_native_fallback_respects_its_version_mode_and_imports() {
        use serde_json::{json, Value};
        let dir = crate::plat::tmp::dir("claude-instructions");
        fs::write(dir.join("AGENTS.md"), "TEAM GUIDANCE").unwrap();
        let cwd = dir.to_string_lossy();
        let mode = |v| ClaudeFiles::configured(v, &Value::Null);
        assert!(instructions_for(&cwd, "claude", mode("2.1.268")).1.contains("TEAM GUIDANCE"));
        assert!(instructions_for(&cwd, "claude", mode("2.1.328")).1.is_empty());
        fs::write(dir.join("CLAUDE.md"), "Additional memory").unwrap();
        assert!(instructions_for(&cwd, "claude", mode("2.1.328")).1.contains("TEAM GUIDANCE"));
        let both = ClaudeFiles::configured("2.1.328", &json!({
            "pluginConfigs":{"cc-plugin-agents-md@builtin":{"options":{"instructionFiles":"claude-md-and-agents-md"}}}
        }));
        assert!(instructions_for(&cwd, "claude", both).1.is_empty());
        fs::write(dir.join("CLAUDE.md"), "@AGENTS.md\n").unwrap();
        assert!(instructions_for(&cwd, "claude", mode("2.1.268")).1.is_empty());
    }

    #[test]
    fn truncation_preserves_utf8_and_reports_the_path() {
        let dir = crate::plat::tmp::dir("context-truncate");
        let path = dir.join("AGENTS.md");
        fs::write(&path, "€".repeat(FILE_LIMIT)).unwrap();
        let (rows, prompt) = instructions(&dir.to_string_lossy(), "claude");
        assert_eq!(rows[0].bytes, (3 * FILE_LIMIT) as u64);
        assert!(rows[0].preview.len() <= FILE_LIMIT);
        assert!(!prompt.contains('�'));
        assert!(prompt.contains(&format!("Truncated {}", path.display())));
    }
}

#[derive(Default)]
pub struct Prompt {
    pub base: String,
    pub project: String,
    pub index: String,
    pub instructions: Vec<Instruction>,
    pub skills: Vec<serde_json::Value>,
}

impl Prompt {
    pub fn build(base: String, env: &super::skills::Env, cwd: &str, provider: &str, model: &str, isolated: bool, claude: ClaudeFiles) -> Self {
        use super::skills;
        use serde_json::json;
        let (instructions, project) = instructions_for(cwd, provider, claude);
        let discovered = skills::index(env, cwd);
        let active = skills::for_model(&discovered, provider, model);
        let index = skills::prompt_index(&active, provider, isolated);
        let skills = discovered.iter().map(|s| {
            let status = if !active.contains(s) { "Skipped" }
                else if skills::native(s, provider, isolated) { "Native" }
                else if !s.model { "Explicit invocation" }
                else { "Cross-provider" };
            json!({"name":s.name, "description":s.description, "path":s.path, "scope":s.scope,
                "source":s.group, "status":status, "provider":super::skills::provider_scope(s), "model":s.model_pattern})
        }).collect();
        Self { base, project, index, instructions, skills }
    }

    pub fn text(&self) -> String {
        [&self.base, &self.project, &self.index].into_iter().filter(|s| !s.is_empty()).map(String::as_str).collect::<Vec<_>>().join("\n\n")
    }
}

#[cfg(test)]
mod launch_tests {
    use super::*;
    use crate::agents::{providers::{self, TurnIn}, rules};
    use serde_json::{json, Value};

    #[test]
    fn project_and_model_scope_select_the_metadata_only() {
        let dir = crate::plat::tmp::dir("context-skill-scope");
        let home = dir.join("home");
        let repo = dir.join("repo");
        let cwd = repo.join("sub");
        let bundle = dir.join("hub");
        let put = |base: &Path, rel: &str, body: &str| {
            let path = base.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, body).unwrap();
        };
        put(&repo, ".git", "");
        put(&repo, ".backplane/skills/a/SKILL.md", "---\nname: a\ndescription: repo skill\n---\nBODY");
        put(&cwd, ".backplane/skills/a/SKILL.md", "---\nname: a\ndescription: closest skill\n---\nBODY");
        put(&home, ".backplane/skills/a/SKILL.md", "---\nname: a\ndescription: home skill\n---\nBODY");
        put(&bundle, "bundled/skills/a/SKILL.md", "---\nname: a\ndescription: bundled skill\n---\nBODY");
        put(&repo, ".backplane/skills/be/SKILL.md", "---\nname: be\nprovider: codex\nmodel: gpt-*\ndescription: model scoped\n---\nBODY");
        put(&repo, ".claude/commands/deploy.md", "---\ndescription: command marker\n---\nBODY");
        let env = crate::agents::skills::env_from(home.to_str().unwrap(), &bundle, None);
        let prompt = |provider, model| Prompt::build("BASE".into(), &env, cwd.to_str().unwrap(), provider, model, false, ClaudeFiles::default());
        let codex = prompt("codex", "gpt-6");
        assert!(codex.index.contains("closest skill") && codex.index.contains("model scoped"));
        for absent in ["repo skill", "home skill", "bundled skill", "command marker", "BODY"] {
            assert!(!codex.index.contains(absent), "{absent}");
        }
        assert!(!prompt("codex", "other").index.contains("model scoped"));
        let claude = prompt("claude", "sonnet");
        assert!(claude.skills.iter().any(|row| row["name"] == "deploy" && row["status"] == "Native"));
        assert!(!claude.index.contains("command marker"));
    }

    #[test]
    fn provider_payloads_carry_the_assembled_prompt() {
        let dir = crate::plat::tmp::dir("context-launch-payload");
        fs::write(dir.join("AGENTS.md"), "NATIVE AGENT TEXT").unwrap();
        fs::create_dir(dir.join(".backplane")).unwrap();
        fs::write(dir.join(".backplane/instructions.md"), "BACKPLANE PROJECT TEXT").unwrap();
        let cwd = dir.to_string_lossy().into_owned();
        let home = dir.join("home");
        let env = crate::agents::skills::env_from(dir.to_str().unwrap(), &home, None);
        let prompt = |provider| Prompt::build("BASE".into(), &env, &cwd, provider, "", false, ClaudeFiles::default()).text();
        let a = |provider| TurnIn { prompt: prompt(provider), text: "MESSAGE".into(), cwd: cwd.clone(),
            home: home.to_string_lossy().into_owned(), thread: "context".into(), ..Default::default() };
        let expected = "BACKPLANE PROJECT TEXT";
        let p = prompt("claude");
        let claude = rules::claude_args(&rules::ArgsIn { model:"", session:"", runtime:"", interaction:"",
            prompt:&p, dirs:&[], effort:"", think:false, plugins:&[], mcp_url:"", mcp_json:"", disallowed:&[], speed:"" });
        assert_eq!(claude.windows(2).find(|a| a[0] == "--append-system-prompt").unwrap()[1], p);
        assert!(p.contains(expected) && p.contains("NATIVE AGENT TEXT"));
        assert!(claude.iter().any(|arg| arg == "--strict-mcp-config"));
        for id in ["codex", "pi", "omp"] {
            let mut driver = providers::driver(id).unwrap().driver;
            let a = a(id);
            driver.prepare(&a).unwrap();
            let argv = driver.argv(&a);
            let payload = if id == "omp" {
                let p = argv.windows(2).find(|a| a[0] == "--append-system-prompt").unwrap();
                fs::read_to_string(&p[1]).unwrap()
            } else if id == "pi" {
                let p = &argv.windows(2).find(|a| a[0] == "--append-system-prompt").unwrap()[1];
                fs::read_to_string(p).unwrap_or_else(|_| p.clone())
            } else if cfg!(windows) {
                let profile = argv.windows(2).find(|a| a[0] == "-p" || a[0] == "--profile").unwrap();
                fs::read_to_string(crate::agents::proc::run_dir(&home).join(format!("{}.config.toml", profile[1]))).unwrap()
            } else { argv.join("\n") };
            assert!(payload.contains(expected), "{id}");
            assert!(!payload.contains("NATIVE AGENT TEXT"), "{id}");
            drop(driver.take_files());
        }
        let print_input = providers::agy_cli::input(&a("antigravity"));
        assert!(print_input.contains(expected) && !print_input.contains("NATIVE AGENT TEXT"));
        for id in ["grok", "opencode", "antigravity", "muse"] {
            let mut driver = providers::driver(id).unwrap().driver;
            let a = a(id);
            driver.start(&a);
            let init = driver.on_line(&json!({"id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}));
            let open = if id == "antigravity" {
                assert!(init.send.iter().any(|r| r.contains("authenticate")));
                driver.on_line(&json!({"id":4,"result":{}}))
            } else { init };
            assert!(open.send.iter().any(|r| r.contains("session/new") || r.contains("session/start")), "{id}");
            let replies = driver.on_line(&json!({"id":2,"result":{"sessionId":"session"}})).send;
            let next: Vec<_> = replies.iter().filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|r| r["method"].as_str().is_some_and(|m| m != "session/prompt")).collect();
            let followups: Vec<_> = next.iter().flat_map(|r| driver.on_line(&json!({"id":r["id"],"result":{}})).send).collect();
            let payload = [replies, followups].concat().join("\n");
            assert!(payload.contains(expected), "{id}: {payload}");
            assert!(!payload.contains("NATIVE AGENT TEXT"), "{id}");
        }
    }
}
