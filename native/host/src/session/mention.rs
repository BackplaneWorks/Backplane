//! The composer's reference popup, natively: a word typed with a sigil names something, `@bot` a bot, `>thread` a thread,
//! `%project` a project, `#room` a group chat, `~machine` one of the owner's paired machines (after one, `%project` and `>thread` pick from that machine). The rows are computed from the hub's own state (no helper), ranked by how well the query matches, then the
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

/// A paired machine as the composer offers it: its name, whether it answered last, and what its mirror holds (ids are `<link>~<id>`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MachineRow { pub name: String, pub link: String, pub away: bool, pub projects: Vec<(String, String)>, pub threads: Vec<(String, String)> }
static MACHINES: std::sync::RwLock<Vec<MachineRow>> = std::sync::RwLock::new(Vec::new());
/// The paired machines, kept by the net lane (`bots::mesh::Bots::mesh_watch`): the popup and the chips read it, nothing else does.
pub fn set_machines(v: Vec<MachineRow>) { *MACHINES.write().unwrap() = v; }
pub fn machines() -> Vec<MachineRow> { MACHINES.read().unwrap().clone() }
/// The machine a sigil word `~name` names (a machine's name as a word)
fn machine_of<'a>(ms: &'a [MachineRow], w: &str) -> Option<&'a MachineRow> {
    let rest = w.strip_prefix('~')?; let r = slug(rest);
    if r.is_empty() { return None }
    ms.iter().find(|m| slug(&m.name) == r)
}
/// The machine named by a `~machine` word earlier in the draft (before the word being typed), the nearest one: what `%` and `>` then pick from.
fn machine_before<'a>(ms: &'a [MachineRow], draft: &str) -> Option<&'a MachineRow> {
    let t = token(draft); let head = &draft[..draft.len() - t.len()];
    head.split(|c: char| c.is_whitespace()).rev().find_map(|w| machine_of(ms, w.trim_end_matches(|c| matches!(c, ',' | '.' | ':' | ';' | '!' | '?'))))
}

/// The word being typed (the draft's last: after its last space, line feed, tab or carriage return).
pub fn token(draft: &str) -> &str { super::super::slash::token(draft) }

/// Refs.typing: a sigil and then only slug characters.
fn typing(w: &str) -> Option<(char, String)> {
    let mut cs = w.chars(); let sig = cs.next()?;
    if !matches!(sig, '@' | '>' | '%' | '#' | '&' | '~') { return None }
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

/// What the `$` popup and the chips need of a thread: the folder it works in, its provider, and whether its Claude runs under an account profile.
pub fn thread_skill_facts(st: &State, thread: &str) -> (String, String, bool) {
    let Some(t) = st.threads.get(thread) else { return (String::new(), String::new(), false) };
    let cwd = if !t.worktree.is_empty() { t.worktree.clone() } else { st.projects.get(&t.project).map(|p| p.root.clone()).unwrap_or_default() };
    let bound = st.settings.get(&format!("account.thread.{thread}.{}", t.provider)).map_or(false, |r| serde_json::from_str::<Value>(r).ok().map_or(true, |j| !j["id"].as_str().unwrap_or("").is_empty()));
    (cwd, t.provider.clone(), bound)
}

/// The popup rows for the draft in thread `here`: `{word, label, desc}` each; empty unless the last word is a sigil word with something to offer.
pub fn rows(st: &State, here: &str, draft: &str) -> Vec<Value> {
    if token(draft).starts_with('$') {
        let (cwd, provider, isolated) = thread_skill_facts(st, here);
        return crate::agents::skills::rows(&crate::agents::skills::popup_index(&cwd), token(draft), &provider, isolated);
    }
    let Some((sig, q)) = typing(token(draft)) else { return vec![] };
    let ms = machines();
    let on = if matches!(sig, '%' | '>') { machine_before(&ms, draft) } else { None };
    let here_project = st.threads.get(here).map(|t| t.project.clone()).unwrap_or_default();
    let mut c: Vec<Cand> = vec![];
    match sig {
        '@' => for b in st.bots_nav.values() {
            let s = slug(&b.name); let sc = score(&s, &b.name, &q);
            if sc > 0 && !s.is_empty() { c.push(Cand { word: format!("@{s}"), about: "bot".into(), score: sc, same: false, at: b.talked.max(b.created) }) }
        },
        '~' => for m in &ms {
            let s = slug(&m.name); let sc = score(&s, &m.name, &q);
            if sc > 0 && !s.is_empty() { c.push(Cand { word: format!("~{s}"), about: format!("machine · {}", if m.away { "away" } else { "here" }), score: sc + if m.away { 0 } else { 5 }, same: false, at: 0 }) }
        },
        '>' if on.is_some() => for (id, t) in &on.unwrap().threads {
            let s = slug(t); let sc = score(&s, t, &q);
            if sc > 0 && !s.is_empty() { c.push(Cand { word: format!(">{s}"), about: format!("thread · {}", on.unwrap().name), score: sc, same: false, at: id.len() as u64 }) }
        },
        '%' if on.is_some() => for (_, t) in &on.unwrap().projects {
            let s = slug(t); let sc = score(&s, t, &q);
            if sc > 0 && !s.is_empty() { c.push(Cand { word: format!("%{s}"), about: format!("project · {}", on.unwrap().name), score: sc, same: false, at: 0 }) }
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

/// The words of a text that start with a sigil, its punctuation off, each once, in order.
fn sigil_words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for w in text.split(|c: char| c.is_whitespace()) {
        let w = w.trim_start_matches(|c| matches!(c, '(' | '[' | '"' | '\'' | '`')).trim_end_matches(|c| matches!(c, '.' | ',' | ':' | ';' | '!' | '?' | ')' | ']' | '"' | '\'' | '`'));
        if w.len() > 1 && matches!(w.chars().next(), Some('@' | '>' | '%' | '#' | '&' | '~')) && !out.iter().any(|x| x == w) { out.push(w.to_string()) }
    }
    out
}

/// What a `/give` names: `(sigil, id, name)` for each `>thread`, `%project` and `@bot` the text points at, each once, in order. Lists are `(id, title)`; a slug two threads share takes the id's tail, as the popup writes it.
pub fn give_hits(text: &str, threads: &[(String, String)], projects: &[(String, String)], bots: &[(String, String)]) -> Vec<(char, String, String)> {
    let mut out: Vec<(char, String, String)> = vec![];
    for w in sigil_words(text) {
        let (sig, rest) = (w.chars().next().unwrap(), w[1..].to_ascii_lowercase());
        let named = |l: &[(String, String)]| l.iter().find(|(_, n)| slug(n) == rest).cloned();
        let hit = match sig {
            '>' => threads.iter().find(|(id, t)| { let s = slug(t); let n = threads.iter().filter(|(_, o)| slug(o) == s).count();
                !s.is_empty() && (rest == s || (n > 1 && rest == format!("{s}-{}", id.rsplit('-').next().unwrap_or("")))) }).cloned(),
            '%' => named(projects), '@' => named(bots), _ => None };
        if let Some((id, name)) = hit { if !out.iter().any(|h| h.0 == sig && h.1 == id) { out.push((sig, id, name)) } }
    }
    out
}

/// What a sent message's reference words point at, for the cards drawn under it: `{word, kind, id, label}` with `kind` bot, thread, project, room or model and `id` the hub-qualified id
/// (`<hub>|<id>`; a model has none). A word that names nothing here is left out (it stays plain text); each word once, in order.
pub fn refs_in(st: &State, hub: &str, text: &str) -> Vec<Value> {
    let words = sigil_words(text);
    if words.is_empty() { return vec![] }
    let all: Value = MODELS.get().map(|f| f()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    let mut slugs: std::collections::HashMap<String, usize> = Default::default();
    for t in st.threads.values().filter(|t| !t.deleted) { *slugs.entry(slug(&t.title)).or_default() += 1 }
    let q = |id: &str| format!("{hub}|{id}");
    let ms = machines();
    let mut out = vec![];
    let mut on: Option<&MachineRow> = None; // the machine a `~name` before this word names: %project and >thread then mean that machine's
    for w in words {
        let (sig, rest) = (w.chars().next().unwrap(), &w[1..]);
        if sig == '~' { on = machine_of(&ms, &w).or(on) }
        let there = if matches!(sig, '%' | '>') { on } else { None };
        let hit: Option<(&str, String, String)> = if let Some(m) = there {
            let l = if sig == '%' { &m.projects } else { &m.threads };
            l.iter().find(|(_, t)| slug(t) == rest.to_ascii_lowercase()).map(|(id, t)| (if sig == '%' { "project" } else { "thread" }, q(id), t.clone()))
        } else { match sig {
            '~' => machine_of(&ms, &w).map(|m| ("machine", String::new(), m.name.clone())),
            '@' => st.bots_nav.iter().find(|(_, b)| slug(&b.name) == rest.to_ascii_lowercase()).map(|(id, b)| ("bot", q(id), b.name.clone())),
            '>' => st.threads.iter().filter(|(_, t)| !t.deleted).find(|(id, t)| { let s = slug(&t.title); let n = slugs.get(&s).copied().unwrap_or(0);
                    !s.is_empty() && (rest == s || (n > 1 && rest == format!("{s}-{}", id.rsplit('-').next().unwrap_or("")))) }).map(|(id, t)| ("thread", q(id), t.title.clone())),
            '%' => st.projects.iter().filter(|(_, p)| !p.removed).find(|(_, p)| slug(&p.title) == rest.to_ascii_lowercase()).map(|(id, p)| ("project", q(id), p.title.clone())),
            '#' => st.rooms_nav.iter().find(|(_, r)| slug(&r.name) == rest.to_ascii_lowercase()).map(|(id, r)| ("room", q(id), r.name.clone())),
            _ => rest.split_once('/').and_then(|(pr, m)| all[pr].as_array().and_then(|rows| rows.iter().find(|r| r["value"].as_str() == Some(m))).map(|r| ("model", String::new(), r["label"].as_str().unwrap_or(m).to_string()))),
        } };
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
    #[test] fn a_give_finds_what_it_names() {
        let l = |v: &[(&str, &str)]| v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
        let (t, p, b) = (l(&[("t1", "Fix the lamp"), ("t2-7", "Same"), ("t3-9", "Same")]), l(&[("p1", "Boards")]), l(&[("b1", "Bub")]));
        assert_eq!(give_hits("/give >fix-the-lamp, %boards and @Bub! see this", &t, &p, &b), vec![('>', "t1".into(), "Fix the lamp".into()), ('%', "p1".into(), "Boards".into()), ('@', "b1".into(), "Bub".into())]);
        assert_eq!(give_hits("/give >same-9 >same-9", &t, &p, &b), vec![('>', "t3-9".into(), "Same".into())], "a shared slug takes the tail; once");
        assert!(give_hits("/give >nothing here", &t, &p, &b).is_empty());
    }
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
    #[test] fn a_machine_word_is_offered_and_picks_what_follows_from_that_machine() {
        set_machines(vec![MachineRow { name: "Desk Top".into(), link: "L1".into(), away: false, projects: vec![("L1~p1".into(), "Boards".into())], threads: vec![("L1~t1-4".into(), "Fix lamp".into())] },
                          MachineRow { name: "laptop".into(), link: "L2".into(), away: true, projects: vec![], threads: vec![] }]);
        let st = State::new();
        let w = |d: &str| rows(&st, "t", d).iter().map(|x| (x["word"].as_str().unwrap().to_string(), x["desc"].as_str().unwrap().to_string())).collect::<Vec<_>>();
        assert_eq!(w("on ~"), [("~desk-top".into(), "machine · here".into()), ("~laptop".into(), "machine · away".into())]);
        assert_eq!(w("~la").len(), 1);
        assert_eq!(w("~desk-top %b"), [("%boards".into(), "project · Desk Top".into())], "after a machine, % is that machine's");
        assert_eq!(w("~desk-top >f")[0].0, ">fix-lamp");
        assert!(w("%b").is_empty(), "without a machine, % is this machine's (none here)");
        let r = refs_in(&st, "h", "run on ~desk-top %boards and >fix-lamp, not ~nowhere");
        let got: Vec<(String, String, String)> = r.iter().map(|x| (x["word"].as_str().unwrap().into(), x["kind"].as_str().unwrap().into(), x["id"].as_str().unwrap().into())).collect();
        assert_eq!(got, [("~desk-top".into(), "machine".into(), "".into()), ("%boards".into(), "project".into(), "h|L1~p1".into()), (">fix-lamp".into(), "thread".into(), "h|L1~t1-4".into())]);
        let l = crate::agents::refs::machine_lines(&machines(), "do it on ~desk-top %boards");
        assert!(l[0].contains("thread_launch") && l[0].contains("p1") && l[1].contains("project \"p1\""), "{l:?}");
        set_machines(vec![]);
    }
}
