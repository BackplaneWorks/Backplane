//! The composer's reference popup, natively (src/core/refs.bend `Refs.offer`): a word typed with a sigil names something, `@bot` a bot, `>thread` a thread,
//! `%project` a project, `#room` a group chat. The rows are computed from the hub's own state (no helper), ranked by how well the query matches, then the
//! same project, then recent work; at most 6. Choosing a row (action `skill`) replaces the word being typed with the row's word and a space.
use crate::state::State;
use serde_json::{json, Value};

pub const MAX: usize = 6;

/// Slug.of: lower case, every other run of characters one '-', no edge dashes, at most 40 long.
pub fn slug(s: &str) -> String {
    let mut out = String::new(); let mut dash = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() { out.push(c.to_ascii_lowercase()); dash = false } else if !dash { out.push('-'); dash = true }
    }
    let mut out: String = out.chars().take(40).collect();
    while out.ends_with('-') { out.pop(); }
    out
}

/// The word being typed (the draft's last: after its last space, line feed, tab or carriage return).
pub fn token(draft: &str) -> &str { super::super::slash::token(draft) }

/// Refs.typing: a sigil and then only slug characters.
fn typing(w: &str) -> Option<(char, String)> {
    let mut cs = w.chars(); let sig = cs.next()?;
    if !matches!(sig, '@' | '>' | '%' | '#') { return None }
    let q: String = cs.collect();
    q.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_').then(|| (sig, q.to_ascii_lowercase()))
}

/// 0: no match; a prefix beats a word start beats a substring beats letters in order. An empty query matches everything a little.
fn score(slug: &str, name: &str, q: &str) -> u32 {
    if q.is_empty() { return 1 }
    let (n, name) = (slug, name.to_ascii_lowercase());
    if n.starts_with(q) { return 100 }
    if n.split('-').any(|p| p.starts_with(q)) || name.split(|c: char| !c.is_alphanumeric()).any(|p| p.starts_with(q)) { return 60 }
    if n.contains(q) || name.contains(q) { return 40 }
    let mut it = n.chars(); if q.chars().all(|c| it.any(|x| x == c)) { 15 } else { 0 }
}

struct Cand { word: String, about: String, score: u32, same: bool, at: u64 }

/// The popup rows for the draft in thread `here`: `{word, label, desc}` each; empty unless the last word is a sigil word with something to offer.
pub fn rows(st: &State, here: &str, draft: &str) -> Vec<Value> {
    let Some((sig, q)) = typing(token(draft)) else { return vec![] };
    let here_project = st.threads.get(here).map(|t| t.project.clone()).unwrap_or_default();
    let mut c: Vec<Cand> = vec![];
    match sig {
        '@' => for b in st.bots_nav.values() {
            let s = slug(&b.name); let sc = score(&s, &b.name, &q);
            if sc > 0 && !s.is_empty() { c.push(Cand { word: format!("@{s}"), about: "bot".into(), score: sc, same: false, at: b.talked.max(b.created) }) }
        },
        '>' => {
            let live: Vec<(&String, &crate::state::Thread)> = st.threads.iter().filter(|(i, t)| !t.deleted && i.as_str() != here && !t.title.trim().is_empty()).collect();
            let mut seen: std::collections::HashMap<String, usize> = Default::default();
            for (_, t) in &live { *seen.entry(slug(&t.title)).or_default() += 1 }
            for (id, t) in live {
                let mut s = slug(&t.title); if s.is_empty() { continue }
                let sc = score(&s, &t.title, &q);
                if seen.get(&s).copied().unwrap_or(0) > 1 { s = format!("{s}-{}", id.rsplit('-').next().unwrap_or("")) } // Refs.seq: the number the id ends with
                let proj = st.projects.get(&t.project).map(|p| p.title.clone()).unwrap_or_default();
                if sc > 0 { c.push(Cand { word: format!(">{s}"), about: if proj.is_empty() { "thread".into() } else { format!("thread · {proj}") }, score: sc, same: t.project == here_project, at: t.activity }) }
            }
        }
        '%' => for (_, p) in st.projects.iter().filter(|(_, p)| !p.removed) {
            let s = slug(&p.title); let sc = score(&s, &p.title, &q);
            if sc > 0 && !s.is_empty() { c.push(Cand { word: format!("%{s}"), about: "project".into(), score: sc, same: false, at: p.created }) }
        },
        _ => for r in st.rooms_nav.values() {
            let s = slug(&r.name); let sc = score(&s, &r.name, &q);
            if sc > 0 && !s.is_empty() { c.push(Cand { word: format!("#{s}"), about: if r.members == 1 { "1 member".into() } else { format!("{} members", r.members) }, score: sc, same: false, at: 0 }) }
        },
    }
    c.sort_by(|a, b| b.score.cmp(&a.score).then(b.same.cmp(&a.same)).then(b.at.cmp(&a.at)).then(a.word.cmp(&b.word)));
    c.truncate(MAX);
    c.into_iter().map(|x| json!({"word": x.word, "label": x.word, "desc": x.about})).collect()
}

/// The draft with its last word replaced by `word` and a space (the popup row chosen; also a `/command` or `$skill`).
pub fn complete(draft: &str, word: &str) -> String {
    let t = token(draft);
    format!("{}{} ", &draft[..draft.len() - t.len()], word)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_title_is_a_slug() { assert_eq!(slug("Fix the perf lag!"), "fix-the-perf-lag"); assert_eq!(slug("--A  b--"), "a-b"); assert_eq!(slug(&"x".repeat(50)).len(), 40); }
    #[test] fn only_a_sigil_word_is_offered() { assert!(typing("hello").is_none() && typing("@").is_some() && typing(">fo").is_some() && typing("@a.b").is_none()); }
    #[test] fn the_chosen_row_replaces_the_word_being_typed() {
        assert_eq!(complete("see @bo", "@bob"), "see @bob "); assert_eq!(complete("@b", "@bob"), "@bob "); assert_eq!(complete("hi\n>th", ">thread-a"), "hi\n>thread-a ");
    }
    #[test] fn a_prefix_outranks_a_substring() { assert!(score("bob", "Bob", "bo") > score("sbob", "sbob", "bo")); assert_eq!(score("bob", "Bob", "zz"), 0); }
    #[test] fn threads_and_projects_are_offered_best_match_first() {
        let mut st = State::new();
        for l in [r#"{"$":"ProjectCreated","id":"p1","title":"Rocket Lab","root":"/r","at":10}"#,
                  r#"{"$":"ThreadCreated","id":"t1-1","project":"p1","title":"Fix perf lag","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":11}"#,
                  r#"{"$":"ThreadCreated","id":"t2-2","project":"p1","title":"Write docs","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":12}"#,
                  r#"{"$":"ThreadCreated","id":"t3-3","project":"p1","title":"Perf budget","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":13}"#] { st.apply_line(l); }
        let r = rows(&st, "t2-2", "look at >pe");
        let words: Vec<&str> = r.iter().map(|x| x["word"].as_str().unwrap()).collect();
        assert_eq!(words, [">perf-budget", ">fix-perf-lag"], "prefix first, then a word inside; the thread itself is never offered");
        assert_eq!(rows(&st, "t2-2", "%ro")[0]["word"], "%rocket-lab");
        assert!(rows(&st, "t2-2", "plain words").is_empty());
        assert_eq!(rows(&st, "t2-2", ">").len(), 2, "a bare sigil lists what there is");
    }
}
