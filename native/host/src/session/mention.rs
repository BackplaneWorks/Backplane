//! The composer's reference popup, natively: a word typed with a sigil names something, `@bot` a bot, `>thread` a thread,
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

/// The hub's `models.all` rows as a JSON text (`{provider: [{value, label, ok, why}]}`), set once at start: the screen-audit bin has no settings, so it offers none.
static MODELS: std::sync::OnceLock<fn() -> String> = std::sync::OnceLock::new();
pub fn set_models(f: fn() -> String) { let _ = MODELS.set(f); }

/// The word being typed (the draft's last: after its last space, line feed, tab or carriage return).
pub fn token(draft: &str) -> &str { super::super::slash::token(draft) }

/// Refs.typing: a sigil and then only slug characters.
fn typing(w: &str) -> Option<(char, String)> {
    let mut cs = w.chars(); let sig = cs.next()?;
    if !matches!(sig, '@' | '>' | '%' | '#' | '&') { return None }
    let q: String = cs.collect();
    // a model id (`provider/model`) holds more than a slug does (settings/mentions.rs)
    let ok = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_' || (sig == '&' && matches!(c, '/' | '.' | ':' | '+'));
    q.chars().all(ok).then(|| (sig, q.to_ascii_lowercase()))
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
        '&' => {
            let all: Value = MODELS.get().map(|f| f()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
            for (provider, rows) in all.as_object().into_iter().flatten() {
                for r in rows.as_array().into_iter().flatten() {
                    let Some(v) = r["value"].as_str() else { continue };
                    let id = format!("{provider}/{v}"); let label = r["label"].as_str().unwrap_or(v);
                    let sc = score(&id.to_ascii_lowercase().replace('/', "-"), &format!("{id} {label}"), &q.replace('/', "-"));
                    if sc > 0 { c.push(Cand { word: format!("&{id}"), about: if r["ok"].as_bool().unwrap_or(false) { label.to_string() } else { format!("{label} (not usable now)") }, score: sc + if r["ok"].as_bool().unwrap_or(false) { 5 } else { 0 }, same: false, at: 0 }) }
                }
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

/// What a sent message's reference words point at, for the cards drawn under it: `{word, kind, id, label}` with `kind` bot, thread, project, room or model and `id` the hub-qualified id
/// (`<hub>|<id>`; a model has none). A word that names nothing here is left out (it stays plain text); each word once, in order.
pub fn refs_in(st: &State, hub: &str, text: &str) -> Vec<Value> {
    let words: Vec<String> = { let mut out: Vec<String> = vec![];
        for w in text.split(|c: char| c.is_whitespace()) {
            let w = w.trim_start_matches(|c| matches!(c, '(' | '[' | '"' | '\'' | '`')).trim_end_matches(|c| matches!(c, '.' | ',' | ':' | ';' | '!' | '?' | ')' | ']' | '"' | '\'' | '`'));
            if w.len() > 1 && matches!(w.chars().next(), Some('@' | '>' | '%' | '#' | '&')) && !out.iter().any(|x| x == w) { out.push(w.to_string()) }
        }
        out };
    if words.is_empty() { return vec![] }
    let all: Value = MODELS.get().map(|f| f()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    let mut slugs: std::collections::HashMap<String, usize> = Default::default();
    for t in st.threads.values().filter(|t| !t.deleted) { *slugs.entry(slug(&t.title)).or_default() += 1 }
    let q = |id: &str| format!("{hub}|{id}");
    let mut out = vec![];
    for w in words {
        let (sig, rest) = (w.chars().next().unwrap(), &w[1..]);
        let hit: Option<(&str, String, String)> = match sig {
            '@' => st.bots_nav.iter().find(|(_, b)| slug(&b.name) == rest.to_ascii_lowercase()).map(|(id, b)| ("bot", q(id), b.name.clone())),
            '>' => st.threads.iter().filter(|(_, t)| !t.deleted).find(|(id, t)| { let s = slug(&t.title); let n = slugs.get(&s).copied().unwrap_or(0);
                    !s.is_empty() && (rest == s || (n > 1 && rest == format!("{s}-{}", id.rsplit('-').next().unwrap_or("")))) }).map(|(id, t)| ("thread", q(id), t.title.clone())),
            '%' => st.projects.iter().filter(|(_, p)| !p.removed).find(|(_, p)| slug(&p.title) == rest.to_ascii_lowercase()).map(|(id, p)| ("project", q(id), p.title.clone())),
            '#' => st.rooms_nav.iter().find(|(_, r)| slug(&r.name) == rest.to_ascii_lowercase()).map(|(id, r)| ("room", q(id), r.name.clone())),
            _ => rest.split_once('/').and_then(|(pr, m)| all[pr].as_array().and_then(|rows| rows.iter().find(|r| r["value"].as_str() == Some(m))).map(|r| ("model", String::new(), r["label"].as_str().unwrap_or(m).to_string()))),
        };
        if let Some((kind, id, label)) = hit { out.push(json!({"word": w, "kind": kind, "id": id, "label": label})) }
    }
    out
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
    #[test] fn references_in_a_message_point_at_what_they_name() {
        let mut st = State::new();
        for l in [r#"{"$":"ProjectCreated","id":"p1","title":"Rocket Lab","root":"/r","at":10}"#,
                  r#"{"$":"ThreadCreated","id":"t1-1","project":"p1","title":"Fix perf lag","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":11}"#] { st.apply_line(l); }
        let r = refs_in(&st, "h", "see >fix-perf-lag, and %rocket-lab. also >nothing @ghost");
        let got: Vec<(String, String, String)> = r.iter().map(|x| (x["word"].as_str().unwrap().into(), x["kind"].as_str().unwrap().into(), x["id"].as_str().unwrap().into())).collect();
        assert_eq!(got, [(">fix-perf-lag".into(), "thread".into(), "h|t1-1".into()), ("%rocket-lab".into(), "project".into(), "h|p1".into())]);
        assert!(refs_in(&st, "h", "no references here").is_empty());
    }
    #[test] fn models_are_offered_from_the_hubs_rows() {
        fn rows_json() -> String { r#"{"claude":[{"value":"opus","label":"Opus 5.5","ok":true,"why":""},{"value":"sonnet","label":"Sonnet 5","ok":true,"why":""}],"codex":[{"value":"gpt","label":"GPT","ok":false,"why":"x"}]}"#.into() }
        set_models(rows_json);
        let st = State::new();
        let w: Vec<String> = rows(&st, "t", "use &claude/op").iter().map(|x| x["word"].as_str().unwrap().to_string()).collect();
        assert_eq!(w, ["&claude/opus"]);
        assert_eq!(rows(&st, "t", "&").len(), 3, "a bare & lists every model, usable ones first");
        assert!(rows(&st, "t", "&").last().unwrap()["desc"].as_str().unwrap().contains("not usable"));
    }
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
