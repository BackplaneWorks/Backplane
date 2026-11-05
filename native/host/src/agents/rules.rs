//! Small pure rules the turn machine uses, each named after the Bend def it follows (src/core/hub.bend,
//! steer.bend, prompt.bend). Property tests sit beside them.
use serde_json::{json, Value};

/// Steer.head / Steer.tail: a message sent into a streaming reply posts the text up to its last line break first
/// ("" when it has none: an unfinished first line is not cut) and the rest keeps streaming.
pub fn steer_head(text: &str) -> String { match text.rfind('\n') { Some(i) => text[..=i].to_string(), None => String::new() } }
pub fn steer_tail(text: &str) -> String { match text.rfind('\n') { Some(i) => text[i + 1..].to_string(), None => text.to_string() } }

/// Hub.hash: acc = (acc * m + c) mod 2^32 over the code points.
pub fn hub_hash(s: &str, m: u64, seed: u64) -> u64 { s.chars().fold(seed, |acc, c| (acc * m + c as u64) % (1u64 << 32)) }
fn hex(w: usize, n: u64) -> String { let s = format!("{n:x}"); let s = if s.len() < w { format!("{}{s}", "0".repeat(w - s.len())) } else { s }; s[s.len() - w..].to_string() }
/// Hub.uuid: a v4-shaped command id from the time and a hash of what makes it unique (the thread and the message).
pub fn hub_uuid(now: u64, key: &str) -> String {
    let (a, b) = (hub_hash(key, 31, 7), hub_hash(key, 131, 3));
    format!("{}-{}-4{}-8{}-{}{}", hex(8, now), hex(4, a), hex(3, a / 65536), hex(3, b), hex(4, b / 4096), hex(8, a + b))
}

/// Claude.user.u: the user line with the command's uuid first (Claude.tag).
pub fn user_line(text: &str, uuid: &str) -> String {
    let mut o = serde_json::Map::new();
    if !uuid.is_empty() { o.insert("uuid".into(), json!(uuid)); }
    o.insert("type".into(), json!("user")); o.insert("message".into(), json!({"role":"user","content":[{"type":"text","text":text}]}));
    Value::Object(o).to_string()
}
/// Claude.interrupt
pub fn interrupt_line(rid: &str) -> String { json!({"type":"control_request","request_id":rid,"request":{"subtype":"interrupt"}}).to_string() }

/// Hub.title.of: a thread still called "New thread" takes its first line, 60 characters at most.
pub fn title_of(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim().to_string();
    if line.chars().count() > 60 { format!("{}...", line.chars().take(57).collect::<String>()) } else { line }
}

/// Claude.permission
pub fn permission(runtime: &str, interaction: &str) -> &'static str {
    if interaction == "plan" { return "plan" }
    match runtime { "approval-required" => "default", "auto-accept-edits" => "acceptEdits", "auto" => "auto", _ => "bypassPermissions" }
}

/// Claude.args (prompt.bend). The runtime prompt, the Backplane MCP server and plugins are not ported yet: `prompt` is what the caller gives.
pub fn claude_args(model: &str, session: &str, runtime: &str, interaction: &str, prompt: &str) -> Vec<String> {
    let mut a: Vec<String> = ["-p", "--verbose", "--input-format", "stream-json", "--output-format", "stream-json", "--include-partial-messages", "--permission-mode"].iter().map(|s| s.to_string()).collect();
    a.push(permission(runtime, interaction).into());
    a.extend(["--disallowedTools", "Agent,Task", "--append-system-prompt"].iter().map(|s| s.to_string())); a.push(prompt.into());
    if !(runtime == "full-access" && interaction == "default") && !runtime.is_empty() { a.extend(["--permission-prompt-tool", "mcp__backplane__permission_prompt"].iter().map(|s| s.to_string())); }
    if !model.is_empty() { a.push("--model".into()); a.push(model.into()); }
    if !session.is_empty() { a.push("--resume".into()); a.push(session.into()); }
    a
}
/// Agent.key: what a process was started with; a turn asking for another restarts it.
pub fn agent_key(model: &str, runtime: &str, interaction: &str, think: bool) -> String { format!("{model}|{runtime}|{interaction}{}", if think { "|think" } else { "" }) }

extern "C" { fn tzset(); }
/// Rec.cut: "Backplane restarted at 06:04 during this turn." in the person's zone.
pub fn clock(zone: &str, at: u64) -> String {
    let t = at as libc::time_t; let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    static TZ: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = TZ.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        if zone.is_empty() { libc::gmtime_r(&t, &mut tm); }
        else { let old = std::env::var_os("TZ"); std::env::set_var("TZ", zone); tzset(); libc::localtime_r(&t, &mut tm); match old { Some(o) => std::env::set_var("TZ", o), None => std::env::remove_var("TZ") } tzset(); }
    }
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}
pub fn restart_cut(zone: &str, at: u64) -> String { format!("Backplane restarted at {} during this turn.", clock(zone, at)) }
/// Carry.cut
pub const CARRY_CUT: &str = "If a tool call was running when it stopped, its result was lost, and what it started may still be running or have partly run: check its effect before running it again.";
/// Exit.why: a SIGKILL (137) is most likely the kernel taking the process for memory.
pub fn exit_why(code: i32) -> String { if code == 137 { "agent was killed (signal 9, likely out of memory)".into() } else { format!("agent exited with code {code}") } }

#[cfg(test)]
mod tests {
    use super::*;
    // law steer_head_tail (steer.bend Steer.head/Steer.tail): head ++ tail is the text, head ends at a line break
    #[test] fn steer_head_and_tail_split_the_text_at_its_last_line_break() {
        for t in ["", "abc", "abc\n", "a\nb", "a\nb\nc", "\n\n", "x\ny\n"] {
            assert_eq!(format!("{}{}", steer_head(t), steer_tail(t)), t);
            assert!(steer_head(t).is_empty() || steer_head(t).ends_with('\n')); assert!(!steer_tail(t).contains('\n'));
        }
        assert_eq!(steer_head("one line"), ""); // an unfinished first line is not cut
    }
    #[test] fn uuids_are_v4_shaped_and_deterministic() {
        let u = hub_uuid(1793842119, "t1/m1"); assert_eq!(u, hub_uuid(1793842119, "t1/m1")); assert_ne!(u, hub_uuid(1793842119, "t1/m2"));
        let p: Vec<&str> = u.split('-').collect(); assert_eq!(p.iter().map(|x| x.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12]);
        assert!(p[2].starts_with('4') && p[3].starts_with('8')); assert!(u.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    }
    #[test] fn the_user_line_carries_the_uuid_first() {
        let l = user_line("hi \"x\"", "U"); assert!(l.starts_with("{\"uuid\":\"U\",\"type\":\"user\"")); let v: Value = serde_json::from_str(&l).unwrap(); assert_eq!(v["message"]["content"][0]["text"], "hi \"x\"");
        assert!(user_line("a", "").starts_with("{\"type\""));
    }
    #[test] fn titles_cut_at_sixty_characters() { assert_eq!(title_of("  hello\nworld"), "hello"); let t = title_of(&"é".repeat(80)); assert_eq!(t.chars().count(), 60); assert!(t.ends_with("...")); assert_eq!(title_of(&"a".repeat(60)).len(), 60); }
    #[test] fn modes_map_to_claudes_permission_flags() {
        assert_eq!(permission("full-access", "default"), "bypassPermissions"); assert_eq!(permission("approval-required", "default"), "default"); assert_eq!(permission("full-access", "plan"), "plan");
        let a = claude_args("opus", "S", "full-access", "default", "P"); assert!(!a.contains(&"--permission-prompt-tool".to_string())); assert_eq!(&a[a.len() - 4..], ["--model", "opus", "--resume", "S"]);
        assert!(claude_args("", "", "auto", "default", "P").contains(&"--permission-prompt-tool".to_string()));
    }
    #[test] fn the_restart_note_reads_the_person_zone() {
        assert_eq!(clock("", 0), "00:00"); assert_eq!(clock("UTC", 3600 * 5 + 60 * 7), "05:07");
        assert_eq!(restart_cut("", 3600 * 6 + 240), "Backplane restarted at 06:04 during this turn.");
    }
    #[test] fn a_sigkill_is_named() { assert!(exit_why(137).contains("signal 9")); assert_eq!(exit_why(3), "agent exited with code 3"); }
}
