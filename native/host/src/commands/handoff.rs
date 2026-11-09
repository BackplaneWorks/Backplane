//! What `/give` hands over, as pure functions: the references a thread's work touched (its files and boards, the messages that asked and answered, the thread itself),
//! the summary that stands in when the writing model cannot be run, the text the receiving agent reads and the card every client draws. 0.14 handed over the
//! conversation alone; the summary and the references are what this adds (docs/PHONE-DATA.md, "Handoffs").
use super::plan::Line;
use serde_json::{json, Value};

/// The first words of the message a handoff is to the agent. The window and the phones leave that message out (`present::thread::agent_only`): they draw the card.
pub use crate::present::thread::{HANDOFF_HEAD as HEAD, HANDOFF_TONE as TONE, handoff_card as card_of};
/// The tone and kind of the activity that carries the card; its summary is the card's JSON (never cut to a row's length: model.rs).
/// At most this many files and boards ride on a card.
const FILES: usize = 8;

fn clip(s: &str, n: usize) -> String { let one = s.split_whitespace().collect::<Vec<_>>().join(" "); if one.chars().count() > n { format!("{}...", one.chars().take(n.saturating_sub(3)).collect::<String>()) } else { one } }

/// Whether a tool writes the file it names (the text of its row is that file's path): the agents' own edit tools, and Backplane's file and viewer tools.
fn writes(kind: &str) -> bool {
    let k = kind.rsplit("__").next().unwrap_or(kind).to_ascii_lowercase();
    matches!(k.as_str(), "edit" | "write" | "multiedit" | "notebookedit" | "apply_patch" | "str_replace_editor" | "files_write" | "viewer_show")
}
/// Whether a row's text is a path: one line, no spaces, a folder or an extension in it.
fn path_like(t: &str) -> bool { let t = t.trim(); !t.is_empty() && t.len() <= 400 && !t.contains(['\n', ' ', '\t']) && (t.contains('/') || t.rsplit_once('.').map_or(false, |(a, e)| !a.is_empty() && !e.is_empty() && e.len() <= 12)) }
/// "board" for a KiCad design file, "file" for the rest.
fn kind_of_path(p: &str) -> &'static str { let l = p.to_ascii_lowercase(); if [".kicad_pcb", ".kicad_sch", ".kicad_pro"].iter().any(|e| l.ends_with(e)) { "board" } else { "file" } }

/// The references of a thread's work, in the order a person wants them: the thread, the message that asked and the one that answered, the boards, the files.
/// `es` newest first; `root` the thread's folder (a path under it is given relative to it, as a link to a file in the project wants it).
pub fn references(es: &[Line], src: &str, title: &str, root: &str) -> Vec<Value> {
    let mut out = vec![json!({"kind":"thread","label":title,"url":format!("backplane://thread/{src}"),"thread":src})];
    let real = |l: &&Line| l.msg && !l.id.is_empty() && !l.text.trim().is_empty() && !l.text.starts_with(HEAD);
    for (role, name) in [("user", "Asked"), ("assistant", "Answered")] {
        if let Some(l) = es.iter().filter(real).find(|l| l.role == role) {
            out.push(json!({"kind":"message","role":role,"label":format!("{name}: {}", clip(&l.text, 80)),"msg":l.id,"url":format!("backplane://thread/{src}#{}", l.id),"thread":src}));
        }
    }
    let mut seen: Vec<String> = vec![]; let (mut boards, mut files) = (vec![], vec![]);
    for l in es.iter().filter(|l| !l.msg && l.role != "error" && writes(&l.kind) && path_like(&l.text)) {
        let p = l.text.trim().to_string(); if seen.contains(&p) { continue } seen.push(p.clone());
        let rel = if root.is_empty() { p.clone() } else { p.strip_prefix(&format!("{}/", root.trim_end_matches('/'))).unwrap_or(&p).to_string() };
        let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
        let kind = kind_of_path(&p);
        let r = json!({"kind":kind,"label":name,"path":rel,"url":rel,"thread":src});
        if kind == "board" { boards.push(r) } else { files.push(r) }
    }
    out.extend(boards.into_iter().chain(files).take(FILES));
    out
}

/// What the summary is when the writing model gave none (it cannot be run, or answered nothing): where the thread was, in its own words.
pub fn fallback_summary(title: &str, es: &[Line]) -> String {
    let real = |l: &&Line| l.msg && !l.text.trim().is_empty() && !l.text.starts_with(HEAD);
    let last = |role: &str| es.iter().filter(real).find(|l| l.role == role).map(|l| clip(&l.text, 220));
    let mut s = format!("The thread \"{title}\".");
    if let Some(a) = last("user") { s.push_str(&format!(" It was last asked: {a}")); if !a.ends_with(['.', '?', '!']) { s.push('.') } }
    if let Some(a) = last("assistant") { s.push_str(&format!(" Its last reply: {a}")); if !a.ends_with(['.', '?', '!']) { s.push('.') } }
    s
}

/// The note the person typed after the targets: the text without `/give` and without the words that named a target (`is_target` says which those are).
pub fn note_of(text: &str, is_target: impl Fn(&str) -> bool) -> String {
    let t = text.trim(); let t = t.get(5..).unwrap_or("");
    t.split_whitespace().filter(|w| !(w.starts_with(['>', '%', '@']) && is_target(w))).collect::<Vec<_>>().join(" ")
}

/// The card every client draws in the receiving thread: where it came from, the person's note, the summary, the references.
pub fn card(src: &str, title: &str, note: &str, summary: &str, refs: &[Value]) -> Value {
    json!({"from":src,"title":title,"note":note,"summary":summary,"refs":refs})
}
/// The activity that carries a card into a thread's log.
pub fn card_event(thread: &str, act: &str, at: u64, card: &Value) -> Value {
    json!({"$":"ActivityLogged","thread":thread,"act":act,"tone":TONE,"kind":TONE,"summary":card.to_string(),"at":at})
}

/// The message the receiving agent reads.
pub fn agent_text(card: &Value) -> String {
    let s = |k: &str| card[k].as_str().unwrap_or("").to_string();
    let (src, title) = (s("from"), s("title"));
    let mut t = format!("{HEAD}\"{title}\" ({src}).");
    if !s("summary").is_empty() { t.push_str(&format!("\n\nSummary: {}", s("summary"))) }
    if !s("note").is_empty() { t.push_str(&format!("\n\nThe user's note with it: {}", s("note"))) }
    let lines: Vec<String> = card["refs"].as_array().into_iter().flatten().filter(|r| r["kind"] != "thread").map(|r| match r["kind"].as_str().unwrap_or("") {
        "message" => format!("- message {} ({}): {}", r["msg"].as_str().unwrap_or(""), r["role"].as_str().unwrap_or(""), r["label"].as_str().unwrap_or("")),
        k => format!("- {k}: {}", r["path"].as_str().unwrap_or("")),
    }).collect();
    if !lines.is_empty() { t.push_str(&format!("\n\nWhat the work touched:\n{}", lines.join("\n"))) }
    t.push_str(&format!("\n\nRead the whole conversation with thread_read, threadId {src}, when you need more than this."));
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    fn l(msg: bool, role: &str, kind: &str, id: &str, text: &str) -> Line { Line { id: id.into(), msg, role: role.into(), kind: kind.into(), text: text.into() } }
    fn es() -> Vec<Line> { vec![ // newest first
        l(true, "assistant", "", "m4", "Routed the 3V3 rail and ran DRC."), l(false, "tool", "Edit", "a3", "/w/p/boards/main/main.kicad_pcb"), l(false, "tool", "Bash", "a2", "kicad-cli pcb drc x"),
        l(false, "tool", "Write", "a1", "/w/p/notes/plan.md"), l(false, "tool", "Edit", "a0", "/w/p/boards/main/main.kicad_pcb"), l(false, "tool", "Read", "r0", "/w/p/other.txt"),
        l(true, "user", "", "m1", "Route the 3V3 rail"), ] }
    #[test] fn a_threads_work_is_its_thread_its_messages_its_boards_and_its_files() {
        let r = references(&es(), "t1", "Power rail", "/w/p");
        let kinds: Vec<_> = r.iter().map(|x| (x["kind"].as_str().unwrap(), x["label"].as_str().unwrap().to_string())).collect();
        assert_eq!(kinds, vec![("thread", "Power rail".to_string()), ("message", "Asked: Route the 3V3 rail".into()), ("message", "Answered: Routed the 3V3 rail and ran DRC.".into()), ("board", "main.kicad_pcb".into()), ("file", "plan.md".into())],
            "a board touched twice once; a read and a shell command are not work on a file");
        assert_eq!((r[0]["url"].as_str(), r[1]["url"].as_str(), r[1]["msg"].as_str()), (Some("backplane://thread/t1"), Some("backplane://thread/t1#m1"), Some("m1")));
        assert_eq!((r[3]["path"].as_str(), r[3]["url"].as_str(), r[3]["thread"].as_str()), (Some("boards/main/main.kicad_pcb"), Some("boards/main/main.kicad_pcb"), Some("t1")), "relative to the thread's folder");
        assert_eq!(references(&es(), "t1", "P", "")[3]["path"], "/w/p/boards/main/main.kicad_pcb", "no folder known: the path as it was");
    }
    #[test] fn no_more_than_eight_files_and_no_handoff_message_as_a_message() {
        let mut v: Vec<Line> = (0..12).map(|i| l(false, "tool", "Write", &format!("a{i}"), &format!("src/f{i}.rs"))).collect();
        v.insert(0, l(true, "user", "", "mh", &format!("{HEAD}\"X\" (t0).")));
        let r = references(&v, "t1", "T", ""); assert_eq!(r.len(), 1 + 8); assert!(r.iter().all(|x| x["kind"] != "message"));
    }
    #[test] fn words_that_are_not_paths_are_not_files() { assert!(path_like("a/b.kicad_pcb") && path_like("main.rs") && !path_like("ls -la") && !path_like("kicad-cli") && !path_like("two\nlines") && !path_like("")) }
    #[test] fn without_a_model_the_summary_is_where_the_thread_was() {
        assert_eq!(fallback_summary("Power rail", &es()), "The thread \"Power rail\". It was last asked: Route the 3V3 rail. Its last reply: Routed the 3V3 rail and ran DRC.");
        assert_eq!(fallback_summary("Empty", &[]), "The thread \"Empty\".");
    }
    #[test] fn the_note_is_what_the_person_wrote_besides_the_targets() {
        let t = |w: &str| w == ">two" || w == "@ada";
        assert_eq!(note_of("/give >two see  this @ada and %not-a-target", t), "see this and %not-a-target"); assert_eq!(note_of("/give >two", t), ""); assert_eq!(note_of("/give", t), "");
    }
    #[test] fn the_agent_reads_the_summary_the_note_and_what_was_touched() {
        let c = card("t1", "Power rail", "check the DRC", "Routing the rail.", &references(&es(), "t1", "Power rail", "/w/p"));
        let t = agent_text(&c);
        assert!(t.starts_with(&format!("{HEAD}\"Power rail\" (t1).")) && t.contains("Summary: Routing the rail.") && t.contains("The user's note with it: check the DRC"), "{t}");
        assert!(t.contains("- board: boards/main/main.kicad_pcb") && t.contains("- file: notes/plan.md") && t.contains("- message m1 (user): Asked: Route the 3V3 rail") && t.contains("thread_read, threadId t1"), "{t}");
        assert!(!t.contains("- thread:"), "the thread is named in the head");
    }
    #[test] fn a_card_survives_the_log() {
        let c = card("t1", "T", "n", "s", &[json!({"kind":"thread","label":"T","url":"backplane://thread/t1","thread":"t1"})]);
        let e = card_event("t2", "k1", 5, &c); assert_eq!((e["tone"].as_str(), e["kind"].as_str(), e["thread"].as_str()), (Some("handoff"), Some("handoff"), Some("t2")));
        assert_eq!(card_of(e["summary"].as_str().unwrap()), Some(c)); assert_eq!(card_of("Given to \"Two\""), None); assert_eq!(card_of("{}"), None);
    }
}
