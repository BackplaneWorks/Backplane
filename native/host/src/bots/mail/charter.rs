//! A mailroom run's charter: which risky tools a run may use. The grammar is deliberately small: one `may: tool:NAME` or `may: bash:COMMAND PREFIX` per line.
use serde_json::Value;

fn any(xs: &[&str], s: &str) -> bool { xs.iter().any(|h| s.contains(h)) }
fn prefix(s: &str, p: &str) -> bool { s == p || s.starts_with(&format!("{p} ")) }
/// Charter.composed: shell composition is risky and a prefix never authorizes a tail
fn composed(s: &str) -> bool { any(&[";", "&", "||", "|", "\n", "`", "$", "(", ")", "<", ">", "\\"], s) }
fn gs<'a>(v: &'a Value, k: &str) -> Option<&'a str> { v.get(k).and_then(Value::as_str) }
/// Charter.command: a single leading `cd DIR &&` is stripped
fn command(input: &Value) -> String {
    let s = gs(input, "command").or_else(|| gs(input, "cmd")).unwrap_or("").trim().to_string();
    if !s.starts_with("cd ") { return s }
    let ps: Vec<&str> = s.split('&').collect();
    if ps.len() >= 2 && ps[1].is_empty() && !composed(ps[0]) { ps[2..].join("&").trim().to_string() } else { s }
}
fn bash_risky(s: &str) -> bool {
    composed(s) || any(&["git push", "gh pr merge", "scripts/dev.sh install", "rm -", "git branch -D", " delete", "gh pr comment", "gh issue create", "thread delete", "curl ", "wget ", "boat new", "boat create", "order", "purchase"], s)
        || any(&["bash ", "sh ", "zsh ", "eval ", "exec ", "sudo ", "env ", "python", "node ", "bun ", "ruby ", "perl "], s)
}
fn bash(tool: &str) -> bool { any(&["Bash", "bash", "exec_command", "shell", "work_run", "terminal_open", "terminal_send", "secret_run"], tool) }
/// Charter.risky
pub fn risky(tool: &str, input: &Value) -> bool {
    any(&["gmail_send", "gmail_reply", "send_email", "reply_email", "deploy_restart", "deploy_main", "purchase", "order", "store_", "sandbox_create", "boat_new", "boat_create", "delete", "webhook"], tool)
        || (bash(tool) && bash_risky(&command(input)))
        || (any(&["room_post", "mail", "thread_send"], tool) && any(&["~", "peer:", ":room:", "https://", "http://"], gs(input, "to").or_else(|| gs(input, "room")).unwrap_or("")))
}
fn grant(line: &str, tool: &str, input: &Value) -> bool {
    if line == format!("tool:{tool}") { return true }
    let Some(p) = line.strip_prefix("bash:").map(str::trim) else { return false };
    bash(tool) && !composed(&command(input)) && !p.is_empty() && prefix(&command(input), p)
}
/// Charter.line: the first line that grants this call, "" for none
pub fn line(text: &str, tool: &str, input: &Value) -> String {
    text.split('\n').map(str::trim).find(|h| h.strip_prefix("may:").map_or(false, |g| grant(g.trim(), tool, input))).unwrap_or("").to_string()
}
/// Charter.allows
pub fn allows(text: &str, tool: &str, input: &Value) -> bool { !line(text, tool, input).is_empty() }

#[cfg(test)]
mod tests {
    use super::*; use serde_json::json;
    #[test] fn a_charter_grants_only_what_it_names() {
        let c = "may: tool:deploy_main\nmay: bash:git status\nescalate: the rest";
        assert!(allows(c, "deploy_main", &json!({}))); assert!(!allows(c, "deploy_restart", &json!({})));
        assert!(allows(c, "Bash", &json!({"command":"cd /x && git status -s"})), "one leading cd is stripped");
        assert!(!allows(c, "Bash", &json!({"command":"git status; rm -rf x"})), "a prefix never authorizes a tail");
        assert_eq!(line(c, "Bash", &json!({"command":"git status"})), "may: bash:git status");
    }
    #[test] fn risk_follows_the_tool_and_its_input() {
        assert!(risky("deploy_main", &json!({}))); assert!(!risky("memory_save", &json!({})));
        assert!(risky("Bash", &json!({"command":"git push origin"}))); assert!(risky("Bash", &json!({"command":"python x.py"}))); assert!(!risky("Bash", &json!({"command":"git status"})));
        assert!(risky("thread_send", &json!({"to":"l1~t2"}))); assert!(!risky("thread_send", &json!({"to":"t2"})));
    }
}
