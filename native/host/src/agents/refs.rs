//! Refs.agent: the references a sent message makes, read by the agent as a `<references>` block after the text.
//! `@bot`, `>thread`, `%project`, `#room`: each word starting with a sigil is resolved against this hub's own bots, threads, projects and
//! rooms (slugs); a bot on a linked machine (`@name@machine`) and a thread elsewhere (`>machine:slug`) are not resolved here.
use super::world::World;

/// Slug.of: lower case letters and digits, any other run one '-', no leading or trailing '-', at most 40 characters.
pub fn slug_of(s: &str) -> String {
    let mut out = String::new(); let mut dash = true;
    for c in s.chars() { if c.is_ascii_alphanumeric() { out.push(c.to_ascii_lowercase()); dash = false } else if !dash { out.push('-'); dash = true } }
    if out.ends_with('-') { out.pop(); }
    let mut cut: String = out.chars().take(40).collect(); if cut.ends_with('-') { cut.pop(); } cut
}
/// Slug.take: the leading slug characters (letters, digits, '-', '_') of what follows a sigil.
pub fn slug_take(s: &str) -> &str { let n = s.char_indices().find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_')).map_or(s.len(), |(i, _)| i); &s[..n] }
fn kind_of(c: char) -> &'static str { match c { '@' => "bot", '>' => "thread", '%' => "project", '#' => "room", _ => "" } }
/// A thread's slug, its id's own slug when the title has none.
fn thread_slug(title: &str, id: &str) -> String { let s = slug_of(title); if s.is_empty() { slug_of(id) } else { s } }
fn seq(id: &str) -> &str { id.rsplit('-').next().unwrap_or("") }

#[derive(Debug, Clone, PartialEq)]
pub struct Hit { pub kind: &'static str, pub id: String, pub name: String }

/// Refs.find: what one sigil word names (nothing, or one hit). `sel`: the thread the message is said in.
pub fn find(w: &World, sel: &str, word: &str) -> Option<(Hit, String)> {
    let first = word.chars().next()?; let kind = kind_of(first); if kind.is_empty() { return None }
    let rest = &word[first.len_utf8()..]; let taken = slug_take(rest); if taken.is_empty() { return None }
    let after = &rest[taken.len()..];
    if kind == "thread" && after.starts_with(':') && !slug_take(&after[1..]).is_empty() { return None } // a thread elsewhere: the mirror's, not here
    if kind == "bot" && after.starts_with('@') && !slug_take(&after[1..]).is_empty() { return None } // a bot on a linked machine
    let slug = slug_of(taken);
    match kind {
        "bot" => { let sel_bot = w.bot(sel).to_string(); let mut bs: Vec<(&String, &(String, String, bool))> = w.bots.iter().filter(|(_, b)| !b.2 && !b.0.is_empty()).collect(); bs.sort();
            bs.into_iter().find(|(id, b)| slug_of(&b.0) == slug && !(**id == sel_bot || b.1 == sel)).map(|(id, b)| (Hit { kind: "bot", id: id.clone(), name: b.0.clone() }, format!("@{}", b.0))) }
        "thread" => { let here = w.threads.get(sel).map(|t| t.project.clone()).unwrap_or_default();
            let mut ts: Vec<(&String, &super::world::T)> = w.threads.iter().filter(|(id, t)| !t.project.is_empty() && !t.archived && !t.deleted && { let s = thread_slug(&t.title, id); s == slug || format!("{s}-{}", seq(id)) == slug }).collect();
            ts.sort_by(|a, b| a.0.cmp(b.0)); // Map.values: by id, then the stable sort by nearness and recency
            ts.sort_by(|a, b| ((b.1.project == here) as u8, b.1.act).cmp(&((a.1.project == here) as u8, a.1.act)));
            ts.first().map(|(id, t)| (Hit { kind: "thread", id: (*id).clone(), name: t.title.clone() }, format!(">{slug}"))) }
        "project" => { let mut ps: Vec<(&String, &(String, bool))> = w.projects.iter().filter(|(_, p)| !p.1).collect(); ps.sort();
            ps.into_iter().find(|(id, p)| thread_slug(&p.0, id) == slug).map(|(id, p)| (Hit { kind: "project", id: id.clone(), name: p.0.clone() }, format!("%{slug}"))) }
        _ => { let mut rs: Vec<(&String, &(String, bool))> = w.rooms.iter().filter(|(_, r)| !r.1).collect(); rs.sort();
            rs.into_iter().find(|(id, r)| thread_slug(&r.0, id) == slug).map(|(id, r)| (Hit { kind: "room", id: id.clone(), name: r.0.clone() }, format!("#{slug}"))) }
    }
}
/// The sigil words of a text, in order (split on space, tab, line break), the same hit once.
pub fn found(w: &World, sel: &str, text: &str) -> Vec<(Hit, String)> {
    let mut out: Vec<(Hit, String)> = vec![];
    for word in text.split(|c: char| c == ' ' || c == '\n' || c == '\t' || c == '\r').filter(|x| !x.is_empty()) {
        if let Some(h) = find(w, sel, word) { if !out.iter().any(|(o, _)| o.id == h.0.id) { out.push(h) } }
    }
    out
}
/// Refs.line.go: what the agent is told about each reference.
pub fn line(w: &World, hit: &Hit, word: &str) -> String {
    let sum = |id: &str| w.setting(&format!("summary.{id}")).to_string();
    match hit.kind {
        "bot" => format!("{word} is the bot \"{}\"; it has been sent this message and may answer in this thread.", hit.name),
        "thread" => { let t = w.threads.get(&hit.id); let proj = t.and_then(|t| w.projects.get(&t.project)).map(|p| p.0.clone()).unwrap_or_default(); let s = sum(&hit.id);
            format!("{word} is the thread \"{}\" (id {}, project \"{proj}\"){}. Read it with thread_read.", hit.name, hit.id, if s.is_empty() { String::new() } else { format!(": {s}") }) }
        "project" => { let s = sum(&hit.id); format!("{word} is the project \"{}\" (id {}, folder {}){}.", hit.name, hit.id, w.roots.get(&hit.id).cloned().unwrap_or_default(), if s.is_empty() { String::new() } else { format!(": {s}") }) }
        _ => format!("{word} is the group chat \"{}\"; this message was posted there.", hit.name),
    }
}
/// The `<references>` block: the sigil references' lines, then the `&model` lines, in one block ("" when there are none).
pub fn block(w: &World, sel: &str, text: &str, model_lines: &[String]) -> String {
    let mut ls: Vec<String> = found(w, sel, text).iter().map(|(h, word)| line(w, h, word)).collect(); ls.extend(model_lines.iter().cloned());
    if ls.is_empty() { String::new() } else { format!("\n\n<references>\n{}\n</references>", ls.join("\n")) }
}

/// The lines for `~machine` references in a message, and for the `%project` and `>thread` words after one (they mean that machine's): what the agent is
/// told about each. A word that names nothing is left out.
pub fn machine_lines(ms: &[crate::session::composer::mention::MachineRow], text: &str) -> Vec<String> {
    use crate::session::composer::mention::slug;
    let local = |id: &str| id.split_once('~').map_or(id.to_string(), |x| x.1.to_string());
    let (mut out, mut on): (Vec<String>, Option<&crate::session::composer::mention::MachineRow>) = (vec![], None);
    for w in text.split(char::is_whitespace).filter(|x| !x.is_empty()) {
        let w = w.trim_end_matches(|c| matches!(c, ',' | '.' | ':' | ';' | '!' | '?' | ')'));
        let (Some(sig), true) = (w.chars().next(), w.len() > 1) else { continue };
        let rest = slug(&w[sig.len_utf8()..]);
        match sig {
            '~' => if let Some(m) = ms.iter().find(|m| slug(&m.name) == rest) {
                on = Some(m);
                let ps: Vec<String> = m.projects.iter().take(12).map(|(i, t)| format!("\"{t}\" (id {})", local(i))).collect();
                let line = format!("{w} is the machine \"{}\" (link {}), one of the person's paired machines, {}. {}Work asked \"on {w}\" is a thread launched there: thread_launch with machine \"{}\"{}; its threads are <link>~<id>.",
                    m.name, m.link, if m.away { "away now (it may not answer)" } else { "here" },
                    if ps.is_empty() { String::new() } else { format!("Its projects: {}. ", ps.join(", ")) }, m.name, if m.projects.is_empty() { "" } else { " and the project id" });
                if !out.contains(&line) { out.push(line) }
            },
            '%' | '>' => if let Some(m) = on {
                let l = if sig == '%' { &m.projects } else { &m.threads };
                if let Some((id, t)) = l.iter().find(|(_, t)| slug(t) == rest) {
                    out.push(if sig == '%' { format!("{w} is the project \"{t}\" on ~{} (id {}): to work in it, thread_launch with machine \"{}\" and project \"{}\".", slug(&m.name), local(id), m.name, local(id)) }
                        else { format!("{w} is the thread \"{t}\" on ~{} (id {id}). Read it with thread_read, message it with thread_send.", slug(&m.name)) })
                }
            },
            _ => {}
        }
    }
    out
}
