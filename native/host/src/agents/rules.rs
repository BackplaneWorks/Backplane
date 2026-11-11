//! Small pure rules the turn machine uses, each named after def it follows (src/core/hub,
//! steer, prompt). Property tests sit beside them.
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

/// Claude.args then Hub.agent_args: effort, thinking, plugins and the MCP config follow.
pub struct ArgsIn<'a> { pub model: &'a str, pub session: &'a str, pub runtime: &'a str, pub interaction: &'a str, pub prompt: &'a str, pub dirs: &'a [String], pub effort: &'a str, pub think: bool, pub plugins: &'a [String], pub mcp_url: &'a str, /** the --mcp-config JSON (mcpx gen::claude); empty: Backplane's own and pcbparts */ pub mcp_json: &'a str, /** tools Claude is told not to use */ pub disallowed: &'a [String] }
pub fn claude_args(a: &ArgsIn) -> Vec<String> {
    let s = |x: &str| x.to_string();
    let mut v: Vec<String> = ["-p", "--verbose", "--input-format", "stream-json", "--output-format", "stream-json", "--include-partial-messages", "--permission-mode"].iter().map(|x| s(x)).collect();
    v.push(s(permission(a.runtime, a.interaction)));
    v.push(s("--disallowedTools")); v.push(std::iter::once("Agent,Task".to_string()).chain(a.disallowed.iter().cloned()).collect::<Vec<_>>().join(","));
    v.push(s("--append-system-prompt")); v.push(s(a.prompt));
    if !(a.runtime == "full-access" && a.interaction == "default") && !a.runtime.is_empty() { v.extend(["--permission-prompt-tool", "mcp__backplane__permission_prompt"].iter().map(|x| s(x))); }
    if !a.model.is_empty() { v.push(s("--model")); v.push(s(a.model)); }
    if !a.session.is_empty() { v.push(s("--resume")); v.push(s(a.session)); }
    for d in a.dirs { v.push(s("--add-dir")); v.push(s(d)); }
    if effort_ok(a.effort) { v.push(s("--effort")); v.push(s(a.effort)); }
    if a.think { v.push(s("--thinking-display")); v.push(s("summarized")); }
    for d in a.plugins { v.push(s("--plugin-dir")); v.push(s(d)); }
    v.push(s("--mcp-config")); v.push(if a.mcp_json.is_empty() { mcp_config(a.mcp_url) } else { a.mcp_json.to_string() });
    v
}
/// Windows: the system prompt goes to Claude in a file (`--append-system-prompt-file`). A command line there holds 32767 characters,
/// npm's `claude.cmd` (cmd.exe) 8191 and Git's sh keeps 8186 of a quoted argument, while the prompt is 13 to 20 thousand; and a batch
/// file's arguments may not hold the prompt's newlines at all.
pub fn prompt_to_file(args: &mut [String], file: &std::path::Path) -> std::io::Result<()> {
    let Some(i) = args.iter().position(|a| a == "--append-system-prompt") else { return Ok(()) };
    if i + 1 >= args.len() { return Ok(()) }
    if let Some(d) = file.parent() { std::fs::create_dir_all(d)?; }
    let tmp = file.with_extension("tmp"); std::fs::write(&tmp, &args[i + 1])?; std::fs::rename(&tmp, file)?;
    args[i] = "--append-system-prompt-file".into(); args[i + 1] = file.to_string_lossy().into_owned();
    Ok(())
}
/// AG.Effort.of("claude", e): the levels Claude takes.
pub fn effort_ok(e: &str) -> bool { ["low", "medium", "high", "xhigh", "max"].contains(&e) }
/// The MCP servers Claude is given: Backplane's own, and pcbparts (a free parts-search MCP).
pub fn mcp_config(url: &str) -> String { format!("{{\"mcpServers\":{{\"backplane\":{{\"type\":\"http\",\"url\":{}}},\"pcbparts\":{{\"type\":\"http\",\"url\":\"https://pcbparts.dev/mcp\"}}}}}}", serde_json::to_string(url).unwrap_or_default()) }
/// Agent.key: what a process was started with; a turn asking for another restarts it.
pub fn agent_key(model: &str, runtime: &str, interaction: &str, think: bool) -> String { format!("{model}|{runtime}|{interaction}{}", if think { "|think" } else { "" }) }

use crate::plat::time::local_tm;
/// Rec.cut: "Backplane restarted at 06:04 during this turn." in the person's zone.
pub fn clock(zone: &str, at: u64) -> String { let tm = local_tm(zone, at); format!("{:02}:{:02}", tm.tm_hour, tm.tm_min) }
/// A coming time for the timeline: "15:40" today, "Nov 8 15:40" on another day.
pub fn when(zone: &str, at: u64, now: u64) -> String {
    let (a, n) = (local_tm(zone, at), local_tm(zone, now));
    if (a.tm_year, a.tm_yday) == (n.tm_year, n.tm_yday) { return format!("{:02}:{:02}", a.tm_hour, a.tm_min) }
    format!("{} {} {:02}:{:02}", ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][a.tm_mon.clamp(0, 11) as usize], a.tm_mday, a.tm_hour, a.tm_min)
}
pub fn restart_cut(zone: &str, at: u64) -> String { format!("Backplane restarted at {} during this turn.", clock(zone, at)) }
/// Carry.cut
pub const CARRY_CUT: &str = "If a tool call was running when it stopped, its result was lost, and what it started may still be running or have partly run: check its effect before running it again.";
/// Rec.quote / Rec.unanswered: what went unanswered when the hub restarted.
pub fn rec_quote(x: &str) -> String { let l = x.split('\n').next().unwrap_or("").trim(); if l.is_empty() { String::new() } else { format!(": \u{201C}{l}\u{201D}") } }
pub fn unanswered(kind: &str, tool: &str, detail: &str) -> String {
    match kind { "input" => format!(" Its question was not answered{}", rec_quote(detail)), "approval" => format!(" Its request to use {tool} was not answered{}", rec_quote(detail)), "plan" => " Its plan was not answered.".into(), _ => " Its request was not answered.".into() }
}
/// Exit.why: a SIGKILL (137) is most likely the kernel taking the process for memory.
pub fn exit_why(code: i32) -> String { if code == 137 { "agent was killed (signal 9, likely out of memory)".into() } else { format!("agent exited with code {code}") } }

/// Err.scrub: the text with its secrets dropped (token=/key=/secret=/password= values, Bearer tokens, key-shaped words).
pub fn err_scrub(s: &str) -> String {
    const KEYS: [&str; 5] = ["token=", "key=", "secret=", "password=", "Bearer "];
    const STARTS: [&str; 10] = ["sk-", "ghp_", "gho_", "ghs_", "ghu_", "github_pat_", "xoxb-", "xoxp-", "AKIA", "AIza"];
    let cs: Vec<char> = s.chars().collect(); let at = |i: usize, k: &str| k.chars().enumerate().all(|(j, c)| cs.get(i + j) == Some(&c));
    let sec = |c: char| c.is_ascii_alphanumeric() || "_-.+/".contains(c);
    let (mut out, mut i, mut edge) = (String::new(), 0, true);
    while i < cs.len() {
        let hit = KEYS.iter().find(|k| at(i, k)).or_else(|| if edge { STARTS.iter().find(|k| at(i, k)) } else { None });
        match hit { Some(k) => { out.push_str(k); out.push('…'); i += k.chars().count(); while i < cs.len() && sec(cs[i]) { i += 1 } edge = false }
                    None => { out.push(cs[i]); edge = !cs[i].is_ascii_alphanumeric(); i += 1 } }
    }
    out
}
/// Exit.say: `<Exit.why's words, else dflt> (exit N | signal N)` and the scrubbed tail of stderr (the last 2048 bytes are kept by the caller).
pub fn exit_say(dflt: &str, code: i32, tail: &str) -> String {
    let why = if code == 137 { "agent was killed (signal 9, likely out of memory)" } else { "" };
    let c = if code >= 128 { format!("signal {}", code - 128) } else { format!("exit {code}") };
    let tl = err_scrub(tail); let tl = tl.trim();
    format!("{} ({c}){}", if why.is_empty() { dflt } else { why }, if tail.trim().is_empty() { String::new() } else { format!(": {tl}") })
}


/// Rules.block: the standing rules ("rule.<id>" settings, oldest first then by id) that apply to a thread: on, for every bot or this thread's bot, for every project or
/// this thread's project. "" when none.
pub fn rules_for(st: &std::collections::HashMap<String, String>, bot: &str, project: &str) -> Vec<Value> {
    let mut rs: Vec<(u64, String, Value)> = st.iter().filter(|(k, _)| k.starts_with("rule.")).filter_map(|(k, v)| {
        let o: Value = serde_json::from_str(v).ok()?; if !o.is_object() { return None }
        let id = o.get("id").and_then(Value::as_str).filter(|i| !i.is_empty()).map(str::to_string).unwrap_or_else(|| k[5..].to_string());
        Some((o.get("at").and_then(Value::as_u64).unwrap_or(0), id, o)) }).collect();
    rs.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    let g = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    rs.into_iter().filter_map(|(_, id, mut o)| {
        let on = o.get("on").and_then(Value::as_bool).unwrap_or(true); let (rb, rp) = (g(&o, "bot"), g(&o, "project"));
        if !(on && (rb.is_empty() || rb == bot) && (rp.is_empty() || rp == project)) { return None }
        o["id"] = json!(id); Some(o) }).collect()
}
pub fn rules_block(st: &std::collections::HashMap<String, String>, bot: &str, project: &str) -> String {
    let g = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut lines = String::new();
    for o in &rules_for(st, bot, project) {
        let (tool, word, enforce) = (g(o, "tool"), g(o, "word"), o.get("enforce").and_then(Value::as_bool).unwrap_or(false));
        let tail = if tool.is_empty() { String::new() } else if enforce { if word.is_empty() { format!(" (enforced: {tool} is refused)") } else { format!(" (enforced: {tool} is refused unless the person's latest message says \"{word}\")") } } else { format!(" (when using {tool})") };
        lines += &format!("- {}{tail}\n", g(o, "text"));
    }
    if lines.is_empty() { String::new() } else { format!("<rules>\nStanding rules from the person. Follow them; the enforced ones are checked by Backplane.\n{lines}</rules>") }
}
/// Rules.strip: a tool name without its `mcp__<server>__` prefix: the characters since the last pair of underscores.
pub fn strip_mcp(s: &str) -> String {
    let (mut cur, mut under) = (String::new(), false);
    for c in s.chars() { if under { if c == '_' { cur.clear(); under = false } else { cur.push(c); under = false } } else { cur.push(c); under = c == '_' } }
    cur
}
/// Rules.tool.match: the rule's patterns (split on spaces, commas, tabs, newlines) name the tool: exact, or a prefix before a final `*`, as given or without the MCP prefix.
pub fn tool_matches(pattern: &str, name: &str) -> bool {
    let bare = strip_mcp(name);
    let one = |p: &str, n: &str| p == n || (p.ends_with('*') && n.starts_with(&p[..p.len() - 1]));
    pattern.split(|c: char| c == ' ' || c == ',' || c == '\t' || c == '\n').filter(|p| !p.is_empty()).any(|p| one(p, name) || one(p, &bare))
}
/// Rules.word.in: the word is in the text as a whole word (no letter or digit on either side), in any case.
pub fn word_in(word: &str, text: &str) -> bool {
    if word.is_empty() { return false }
    let (w, t) = (word.to_lowercase(), text.to_lowercase()); let tc: Vec<char> = t.chars().collect(); let wc: Vec<char> = w.chars().collect();
    (0..tc.len()).any(|i| i + wc.len() <= tc.len() && tc[i..i + wc.len()] == wc[..] && (i == 0 || !tc[i - 1].is_ascii_alphanumeric()) && (i + wc.len() == tc.len() || !tc[i + wc.len()].is_ascii_alphanumeric()))
}
/// Rules.refusal: "" when the call is allowed; else why not. An enforced rule that applies, names the tool, and is not unlocked by the person's latest message (a message of theirs, not one
/// delivered from a room, bot, routine, webhook or space: those start with "[") refuses it.
pub fn refusal(rules: &[Value], tool: &str, last: &str) -> String {
    if tool.is_empty() { return String::new() }
    let g = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    for o in rules {
        let (rt, word) = (g(o, "tool"), g(o, "word"));
        if !(o.get("enforce").and_then(Value::as_bool).unwrap_or(false) && !rt.is_empty() && tool_matches(&rt, tool)) { continue }
        if !word.is_empty() && !last.trim().starts_with('[') && word_in(&word, last) { continue }
        let tail = if word.is_empty() { format!("{tool} is not allowed here.") } else { format!("{tool} needs the person's word \"{word}\" in their latest message.") };
        return format!("Refused by a standing rule ({}): \"{}\". {tail}", g(o, "id"), g(o, "text"));
    }
    String::new()
}
/// Rules.agent: a turn's text with the thread's rules before it.
pub fn rules_agent(block: &str, text: &str) -> String { if block.is_empty() { text.to_string() } else { format!("{block}\n\n{text}") } }

#[cfg(test)]
mod tests {
    #[test] fn exit_say_words_the_exit_and_scrubs_the_tail() {
        use super::*;
        assert_eq!(exit_say("grok exited before the turn ended", 1, ""), "grok exited before the turn ended (exit 1)");
        assert_eq!(exit_say("x", 137, ""), "agent was killed (signal 9, likely out of memory) (signal 9)");
        assert_eq!(exit_say("x", 2, " boom token=abc123 and sk-LIVEKEY99 Bearer zzz \n"), "x (exit 2): boom token=… and sk-… Bearer …");
    }
    use super::*;
    // law steer_head_tail: head ++ tail is the text, head ends at a line break
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
        let mk = |m: &'static str, s: &'static str, r: &'static str| ArgsIn { model: m, session: s, runtime: r, interaction: "default", prompt: "P", dirs: &[], effort: "", think: false, plugins: &[], mcp_url: "http://u", mcp_json: "", disallowed: &[] };
        let a = claude_args(&mk("opus", "S", "full-access")); assert!(!a.contains(&"--permission-prompt-tool".to_string())); assert_eq!(&a[a.len() - 6..a.len() - 2], ["--model", "opus", "--resume", "S"]);
        assert!(claude_args(&mk("", "", "auto")).contains(&"--permission-prompt-tool".to_string()));
        let mut x = mk("", "", "full-access"); let dirs = vec!["/k".to_string(), "/h/attachments".to_string()]; x.dirs = &dirs; x.effort = "high"; x.think = true; x.plugins = &dirs[..1];
        let a = claude_args(&x); assert_eq!(&a[a.len() - 12..a.len() - 2], ["--add-dir", "/k", "--add-dir", "/h/attachments", "--effort", "high", "--thinking-display", "summarized", "--plugin-dir", "/k"]); assert_eq!(a[a.len() - 2], "--mcp-config");
    }
    #[test] fn the_restart_note_reads_the_person_zone() {
        assert_eq!(clock("", 0), "00:00"); assert_eq!(clock("UTC", 3600 * 5 + 60 * 7), "05:07");
        assert_eq!(restart_cut("", 3600 * 6 + 240), "Backplane restarted at 06:04 during this turn.");
    }
    #[test] fn unanswered_asks_are_named() { assert_eq!(unanswered("approval", "Bash", "ls -la\nx"), " Its request to use Bash was not answered: \u{201C}ls -la\u{201D}"); assert_eq!(unanswered("plan", "", ""), " Its plan was not answered."); assert_eq!(unanswered("input", "", " "), " Its question was not answered"); assert_eq!(unanswered("x", "", ""), " Its request was not answered."); }
    #[test] fn a_sigkill_is_named() { assert!(exit_why(137).contains("signal 9")); assert_eq!(exit_why(3), "agent exited with code 3"); }
    #[test] fn the_rules_block_is_text_for_the_rules_that_apply() {
        let mut st = std::collections::HashMap::new();
        st.insert("rule.r1".to_string(), r#"{"id":"r1","text":"Never push to main","at":5}"#.to_string());
        st.insert("rule.r2".to_string(), r#"{"text":"Ask before deleting","tool":"Bash","at":3}"#.to_string());
        st.insert("rule.r3".to_string(), r#"{"id":"r3","text":"No deploys","tool":"deploy_main","enforce":true,"word":"ship it","at":7}"#.to_string());
        st.insert("rule.r4".to_string(), r#"{"id":"r4","text":"Never edit","tool":"Write","enforce":true,"at":8}"#.to_string());
        st.insert("rule.r5".to_string(), r#"{"id":"r5","text":"for bot b9","bot":"b9","at":9}"#.to_string());
        st.insert("rule.r6".to_string(), r#"{"id":"r6","text":"off","on":false,"at":1}"#.to_string());
        st.insert("rule.r7".to_string(), r#"{"id":"r7","text":"project p2 only","project":"p2","at":10}"#.to_string());
        assert_eq!(rules_block(&st, "", "p1"), "<rules>\nStanding rules from the person. Follow them; the enforced ones are checked by Backplane.\n- Ask before deleting (when using Bash)\n- Never push to main\n- No deploys (enforced: deploy_main is refused unless the person's latest message says \"ship it\")\n- Never edit (enforced: Write is refused)\n</rules>");
        assert!(rules_block(&st, "b9", "p2").contains("- for bot b9\n- project p2 only\n"));
        assert_eq!(rules_block(&std::collections::HashMap::new(), "", ""), ""); assert_eq!(rules_agent("", "hi"), "hi"); assert_eq!(rules_agent("<rules>x</rules>", "hi"), "<rules>x</rules>\n\nhi");
    }
    #[test] fn an_enforced_rule_refuses_the_tool_unless_the_persons_word_is_in_their_latest_message() {
        assert_eq!(strip_mcp("mcp__backplane__thread_send"), "thread_send"); assert_eq!(strip_mcp("Bash"), "Bash");
        assert!(tool_matches("Bash", "Bash") && tool_matches("thread_*", "mcp__backplane__thread_send") && tool_matches("Write, Bash", "Bash") && tool_matches("deploy_main\tthread_send", "mcp__backplane__thread_send") && !tool_matches("Bash", "Bashful") && !tool_matches("thread_*", "work_run"));
        assert!(word_in("ship it", "OK, Ship It now") && !word_in("ship", "shipping") && !word_in("", "x") && word_in("go", "go"));
        let mut st = std::collections::HashMap::new();
        st.insert("rule.r3".to_string(), r#"{"id":"r3","text":"No deploys","tool":"deploy_main","enforce":true,"word":"ship it","at":7}"#.to_string());
        st.insert("rule.r4".to_string(), r#"{"id":"r4","text":"Never edit","tool":"Write","enforce":true,"at":8}"#.to_string());
        st.insert("rule.r5".to_string(), r#"{"id":"r5","text":"just words","tool":"Bash","at":9}"#.to_string());
        let rs = rules_for(&st, "", "p1");
        assert_eq!(refusal(&rs, "mcp__backplane__deploy_main", "do it"), "Refused by a standing rule (r3): \"No deploys\". mcp__backplane__deploy_main needs the person's word \"ship it\" in their latest message.");
        assert_eq!(refusal(&rs, "deploy_main", "please SHIP IT"), ""); assert!(!refusal(&rs, "deploy_main", "[from a bot] ship it").is_empty(), "a delivered message cannot unlock");
        assert_eq!(refusal(&rs, "Write", "ship it"), "Refused by a standing rule (r4): \"Never edit\". Write is not allowed here."); assert_eq!(refusal(&rs, "Bash", ""), "", "a rule without enforce is words only"); assert_eq!(refusal(&rs, "", "x"), "");
    }
    #[test] fn a_prompt_too_long_for_a_windows_command_line_goes_in_a_file() {
        let d = crate::plat::tmp::uncreated("pf");
        let long = "line one\n".repeat(3000);
        let mut a: Vec<String> = ["-p", "--append-system-prompt", long.as_str(), "--model", "m"].iter().map(|x| x.to_string()).collect();
        let f = d.join("prompts/t1.md");
        prompt_to_file(&mut a, &f).unwrap();
        assert_eq!(a, vec!["-p".to_string(), "--append-system-prompt-file".into(), f.to_string_lossy().into_owned(), "--model".into(), "m".into()]);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), long, "the whole prompt, newlines and all");
        let mut b: Vec<String> = vec!["-p".into()]; prompt_to_file(&mut b, &f).unwrap(); assert_eq!(b, vec!["-p".to_string()], "no prompt, nothing changes");
    }
}

/// The row a steer leaves in the thread: the message that cut in, so the row never stands without it.
pub fn steer_summary(text: &str) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.is_empty() { return "Steered: the turn starts again".into() }
    let cut: String = one.chars().take(300).collect();
    format!("Steered: {cut}{}", if one.chars().count() > 300 { "..." } else { "" })
}
#[cfg(test)]
mod steer_summary_tests {
    #[test] fn names_the_message() {
        assert_eq!(super::steer_summary("  fix\nthe  legs "), "Steered: fix the legs");
        assert_eq!(super::steer_summary(" "), "Steered: the turn starts again");
        assert!(super::steer_summary(&"x".repeat(400)).ends_with("..."));
    }
}
