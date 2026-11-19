//! Issue references as links: in a project whose origin is on GitHub, `#1163` in a message's text links to that repository's issue (GitHub
//! sends an issue URL that is a pull request on to the pull request), and `OWNER/REPO#123` to that repository's. Applied to the nodes a
//! message renders to (`nodes_json`), so every client draws them as it draws any web link. Code (spans and blocks) and the text of a link
//! are left as they are; so is a `#` inside a word (`page#2`, `a#1`) or before a number that starts with 0 or is over seven digits (a colour).
use serde_json::{json, Value};

/// `github.com/OWNER/REPO` (`mirror::normalize_origin`'s form) as `OWNER/REPO`; None for another host or another shape.
pub fn repo_of(origin: &str) -> Option<String> {
    let rest = origin.strip_prefix("github.com/")?;
    let (o, r) = rest.split_once('/')?;
    (owner_ok(o) && repo_ok(r)).then(|| format!("{o}/{r}"))
}
fn owner_ok(s: &str) -> bool { !s.is_empty() && s.len() <= 39 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !s.starts_with('-') }
fn repo_ok(s: &str) -> bool { !s.is_empty() && s.len() <= 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) && s != "." && s != ".." }

/// What may stand right before a reference: the start, a space or punctuation that is not part of a word, a path or a URL.
fn opens(prev: Option<char>) -> bool { prev.map_or(true, |c| !(c.is_alphanumeric() || matches!(c, '_' | '/' | '.' | '-' | '#' | '&' | '@' | '='))) }
/// What may follow its number: the end, a space or punctuation that is not part of a word.
fn closes(next: Option<char>) -> bool { next.map_or(true, |c| !(c.is_alphanumeric() || c == '_')) }

/// The repository a project with no GitHub remote (and a bot's conversation) links to.
pub const DEFAULT_REPO: &str = "BackplaneWorks/Backplane";

/// One piece of a text: plain, an issue reference (its text, the repository, the number) or a commit (its text, the SHA).
#[derive(Debug, PartialEq)]
pub enum Piece { Text(String), Issue(String, String, u64), Commit(String) }

/// A short commit SHA: 7 to 12 lower-case hex digits, at least two of them digits and one a letter (a plain number, a word such as
/// `defaced` or `decade` is not one; `facade12` is, the one false link this allows).
fn is_sha(w: &str) -> bool {
    (7..=12).contains(&w.len()) && w.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        && w.chars().filter(|c| c.is_ascii_digit()).count() >= 2 && w.chars().any(|c| c.is_ascii_lowercase())
}

/// A text's references in `repo` (`#N`) and in any repository named (`OWNER/REPO#N`), and its short commit SHAs, the rest as text.
pub fn pieces(s: &str, repo: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    for p in issue_pieces(s, repo) { match p { Piece::Text(t) => out.extend(commit_pieces(&t)), p => out.push(p) } }
    out
}
/// A text's SHAs: a run of hex digits that no word, path, URL or number runs into on either side.
fn commit_pieces(s: &str) -> Vec<Piece> {
    let cs: Vec<char> = s.chars().collect();
    let (mut out, mut from, mut i) = (Vec::new(), 0usize, 0usize);
    while i < cs.len() {
        if cs[i].is_ascii_hexdigit() && opens(if i == 0 { None } else { Some(cs[i - 1]) }) {
            let mut j = i; while j < cs.len() && cs[j].is_ascii_alphanumeric() { j += 1 }
            let w: String = cs[i..j].iter().collect();
            // a sentence's full stop or a colon before a space may follow; any other path or URL character may not
            let after = match cs.get(j) { Some('.' | ':') if cs.get(j + 1).map_or(true, |c| c.is_whitespace()) => None, c => c.copied() };
            if is_sha(&w) && closes(after) && !matches!(after, Some('-' | '/' | '.' | ':' | '@' | '=' | '#' | '&')) {
                if i > from { out.push(Piece::Text(cs[from..i].iter().collect())) }
                out.push(Piece::Commit(w)); from = j; i = j; continue
            }
            i = j; continue
        }
        i += 1;
    }
    if from < cs.len() { out.push(Piece::Text(cs[from..].iter().collect())) }
    out
}
fn issue_pieces(s: &str, repo: &str) -> Vec<Piece> {
    let cs: Vec<char> = s.chars().collect();
    let (mut out, mut from, mut i) = (Vec::new(), 0usize, 0usize);
    while i < cs.len() {
        if cs[i] == '#' {
            // the number after the #
            let mut j = i + 1; while j < cs.len() && cs[j].is_ascii_digit() { j += 1 }
            let digits = j - i - 1;
            if digits >= 1 && digits <= 7 && cs[i + 1] != '0' && closes(cs.get(j).copied()) {
                let n: u64 = cs[i + 1..j].iter().collect::<String>().parse().unwrap_or(0);
                // OWNER/REPO just before it, or nothing (a word before the # is not a reference)
                let mut k = i; while k > from && (cs[k - 1].is_ascii_alphanumeric() || matches!(cs[k - 1], '-' | '_' | '.' | '/')) { k -= 1 }
                let named: String = cs[k..i].iter().collect();
                // a list of references (`#1198/#1172`): the `/` after a reference's number does not make the next a path's
                let listed = named == "/" && k == from && from > 0 && cs[from - 1].is_ascii_digit(); // right after the previous reference
                let start = if named.is_empty() || listed { (listed || opens(if i == 0 { None } else { Some(cs[i - 1]) })).then_some((i, repo.to_string())) }
                    else { named.split_once('/').filter(|(o, r)| owner_ok(o) && repo_ok(r) && opens(if k == 0 { None } else { Some(cs[k - 1]) })).map(|(o, r)| (k, format!("{o}/{r}"))) };
                if let Some((at, r)) = start.filter(|(_, r)| !r.is_empty()) {
                    if at > from { out.push(Piece::Text(cs[from..at].iter().collect())) }
                    out.push(Piece::Issue(cs[at..j].iter().collect(), r, n));
                    from = j; i = j; continue
                }
            }
        }
        i += 1;
    }
    if from < cs.len() { out.push(Piece::Text(cs[from..].iter().collect())) }
    out
}

/// The link of a reference, and its hover title.
pub fn url(repo: &str, n: u64) -> String { format!("https://github.com/{repo}/issues/{n}") }

/// The link of a commit.
pub fn commit_url(repo: &str, sha: &str) -> String { format!("https://github.com/{repo}/commit/{sha}") }

/// The references in rendered nodes (`nodes_json`'s JSON) as links, in place. `repo` is the project's `OWNER/REPO`; "" (a project with no
/// GitHub remote, a bot's conversation) is `DEFAULT_REPO`.
pub fn link_nodes(nodes: &mut Vec<Value>, repo: &str) {
    let repo = if repo.is_empty() { DEFAULT_REPO } else { repo };
    let mut out = Vec::with_capacity(nodes.len());
    for mut n in std::mem::take(nodes) {
        if let Some(t) = n.get("text").and_then(Value::as_str).filter(|_| n.get("tag").is_none()) {
            if !t.contains('#') && !t.chars().any(|c| c.is_ascii_hexdigit()) { out.push(n); continue }
            for p in pieces(t, repo) {
                out.push(match p {
                    Piece::Text(t) => json!({"text": t}),
                    Piece::Commit(sha) => json!({"tag": "a", "kids": [{"text": sha.clone()}], "href": commit_url(repo, &sha), "file": false, "title": format!("{repo}@{sha}")}),
                    Piece::Issue(label, r, num) => json!({"tag": "a", "kids": [{"text": label}], "href": url(&r, num), "file": false, "title": format!("{r}#{num}")}),
                });
            }
            continue
        }
        let skip = matches!(n["tag"].as_str(), Some("code" | "pre" | "a")) || n.get("href").is_some() || n.get("ref").is_some();
        if !skip { if let Some(Value::Array(kids)) = n.get_mut("kids") { link_nodes(kids, repo) } }
        out.push(n);
    }
    *nodes = out;
}

#[cfg(test)]
mod tests {
    use super::*;
    const R: &str = "BackplaneWorks/Backplane";
    fn refs(s: &str) -> Vec<(String, String, u64)> { pieces(s, R).into_iter().filter_map(|p| match p { Piece::Issue(l, r, n) => Some((l, r, n)), _ => None }).collect() }
    fn one(l: &str, r: &str, n: u64) -> (String, String, u64) { (l.into(), r.into(), n) }

    #[test] fn a_bare_number_is_this_projects_issue_and_owner_repo_names_another() {
        assert_eq!(refs("fixes #1163."), vec![one("#1163", R, 1163)]);
        assert_eq!(refs("#7 and (#12), see other/thing#3"), vec![one("#7", R, 7), one("#12", R, 12), one("other/thing#3", "other/thing", 3)]);
        assert_eq!(refs("BackplaneWorks/Backplane#1124: merged"), vec![one("BackplaneWorks/Backplane#1124", R, 1124)]);
        assert_eq!(refs("#1198/#1172 and #1228/#1233."), vec![one("#1198", R, 1198), one("#1172", R, 1172), one("#1228", R, 1228), one("#1233", R, 1233)]);
        assert_eq!(url(R, 1163), "https://github.com/BackplaneWorks/Backplane/issues/1163");
        let p = pieces("see #5 now", R);
        assert_eq!(p, vec![Piece::Text("see ".into()), Piece::Issue("#5".into(), R.into(), 5), Piece::Text(" now".into())]);
    }
    #[test] fn words_paths_urls_colours_and_headings_stay_text() {
        for s in ["page#2", "a#1", "x_#3", "https://e.x/a#12", "src/a.rs#L10", "#0", "#0123", "#12345678", "#12ab", "# Heading", "#", "##", "&#123;", "C#", "issue#", "1#2"] {
            assert!(refs(s).is_empty(), "{s}: {:?}", refs(s));
        }
        assert!(refs("-o/r#1").is_empty() || refs("-o/r#1")[0].1 == "o/r", "a leading dash is not an owner's");
        assert!(refs("a/b/c#4").is_empty(), "a path's tail is not OWNER/REPO");
    }
    #[test] fn rendered_nodes_link_their_text_but_not_code_or_links_and_nothing_without_a_repo() {
        let md = "fixes #12 and `#13`, [see #14](https://e.x) **bold #15**\n\n```\n#16\n```";
        let mut n = crate::md::nodes_json(&crate::md::render(md));
        let mut d = n.clone();
        link_nodes(&mut d, "");
        assert_eq!(d, { let mut e = n.clone(); link_nodes(&mut e, DEFAULT_REPO); e }, "no GitHub remote: the default repository");
        link_nodes(&mut n, R);
        let s = serde_json::to_string(&n).unwrap();
        for k in [12, 15] { assert!(s.contains(&format!(r#""href":"https://github.com/{R}/issues/{k}""#)), "#{k} linked: {s}"); }
        assert!(s.contains(r#""title":"BackplaneWorks/Backplane#12""#), "{s}");
        for k in [13, 14, 16] { assert!(!s.contains(&format!("/issues/{k}")), "#{k} is code or a link's text: {s}"); }
        assert!(s.contains(r##"{"tag":"code","kids":[{"text":"#13"}]}"##), "{s}");
    }
    fn commits(s: &str) -> Vec<String> { pieces(s, R).into_iter().filter_map(|p| match p { Piece::Commit(c) => Some(c), _ => None }).collect() }
    #[test] fn short_shas_link_and_words_numbers_paths_and_urls_do_not() {
        assert_eq!(commits("landed 1b41a005a."), vec!["1b41a005a"]);
        assert_eq!(commits("(350f18407), then 64846c0d4: ok"), vec!["350f18407", "64846c0d4"]);
        assert_eq!(commits("fixes #12 in 1b41a00"), vec!["1b41a00"]);
        for s in ["decade", "defaced1", "deadbeef", "1234567", "20261119", "1b41a00z", "x1b41a005a", "a/1b41a005a", "https://e.x/1b41a005a", "1b41a005a.rs", "1b41a005a-x", "file_1b41a005a", "1B41A005A", "1b41a005a1b41a005a", "1b41a0", "v0.16.2-1b41a005a", "1b41a005a/b", "a=1b41a005a"] {
            assert!(commits(s).is_empty(), "{s}: {:?}", commits(s));
        }
        let mut n = crate::md::nodes_json(&crate::md::render("at 1b41a005a and `1b41a005a`\n\n```\n350f18407\n```"));
        link_nodes(&mut n, R);
        let j = serde_json::to_string(&n).unwrap();
        assert!(j.contains(&format!(r#""href":"https://github.com/{R}/commit/1b41a005a""#)), "{j}");
        assert_eq!(j.matches("/commit/").count(), 1, "code is never linked: {j}");
    }
    #[test] fn only_a_github_origin_names_a_repository() {
        assert_eq!(repo_of("github.com/BackplaneWorks/Backplane").as_deref(), Some(R));
        for o in ["", "gitlab.com/a/b", "github.com/a", "github.com/a/b/c", "github.com/a b/c"] { assert_eq!(repo_of(o), None, "{o}"); }
    }
}
