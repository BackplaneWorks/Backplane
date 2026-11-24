//! Skills the person has installed, found where each provider keeps them, for the composer's `$` popup, the chips under a sent message and the turn's text.
//! Claude: ~/.claude/skills, <project>/.claude/skills (the folder and its parents), the plugins `installed_plugins.json` lists (named `plugin:skill`; a plugin
//! `settings.json` turns off is left out; a directory marketplace of a missing install is read in place). Codex: ~/.codex/skills (or $CODEX_HOME/skills),
//! ~/.agents/skills, <project>/.agents/skills. Backplane's own: KiStack's skills, <hub home>/skills and the bundled ones (<hub home>/bundled/skills, bundled.rs).
//! A skill the thread's provider loads itself is "native": Claude gets `/name`, Codex its own `$name`. Any other is "read": the turn's text names the SKILL.md to read first.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

// ---- skills ----
#[derive(Clone, Debug, Default, PartialEq)]
pub struct USkill { pub name: String, pub description: String, pub path: String, pub model: bool, pub user: bool, pub provider: String, pub model_pattern: String }

fn unquote(s: &str) -> String { if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 { s[1..s.len() - 1].to_string() } else { s.to_string() } }
fn field(line: &str, key: &str) -> Option<String> { line.strip_prefix(&format!("{key}:")).map(|r| unquote(r.trim())) }
/// Front.join / Front.cont / Front.fold: a value carried on to indented lines is read whole on its key's line.
fn fold(ls: &[&str]) -> Vec<String> {
    let mut acc: Vec<String> = vec![]; let mut open = true;
    for l in ls {
        let cont = open && (l.starts_with(' ') || l.starts_with('\t')) && !l.trim().is_empty() && l.trim() != "---";
        if cont && !acc.is_empty() {
            let h = acc.pop().unwrap().trim_end().to_string(); let s = l.trim();
            let one = h.ends_with(": >") || h.ends_with(": |"); let two = [": >-", ": |-", ": >+", ": |+"].iter().any(|e| h.ends_with(e));
            acc.push(if one { format!("{}{s}", &h[..h.len() - 1]) } else if two { format!("{}{s}", &h[..h.len() - 2]) } else { format!("{h} {s}") });
        } else { acc.push(l.to_string()) }
        if l.trim() == "---" { open = false }
    }
    acc
}
/// USkill.parse: the leading --- block's name, description, disable-model-invocation, user-invocable.
pub fn parse_skill(text: &str, dflt: &str, path: &str) -> USkill {
    let ls: Vec<&str> = text.split('\n').collect();
    let (mut n, mut d, mut m, mut u) = (dflt.to_string(), String::new(), String::new(), String::new());
    let (mut provider, mut model_pattern) = (String::new(), String::new());
    if ls.first().map_or(false, |l| l.trim() == "---") {
        for l in fold(&ls[1..]) { if l.trim() == "---" { break }
            if let Some(v) = field(&l, "provider") { provider = v }
            if let Some(v) = field(&l, "model") { model_pattern = v }
            if let Some(v) = field(&l, "name") { n = v } if let Some(v) = field(&l, "description") { d = v }
            if let Some(v) = field(&l, "disable-model-invocation") { m = v } if let Some(v) = field(&l, "user-invocable") { u = v } }
    }
    let flag = |v: &str, dv: bool| match v.to_lowercase().as_str() { "true" => true, "false" => false, _ => dv };
    USkill { name: n, description: d, path: path.into(), model: !flag(&m, false), user: flag(&u, true), provider, model_pattern }
}
fn namec(c: char) -> bool { c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' }
/// Mention.name: "$pcb-review," -> "pcb-review"; "" for a word that is no mention.
pub fn mention_name(w: &str) -> String { match w.strip_prefix('$') { Some(r) => r.chars().take_while(|c| namec(*c)).collect(), None => String::new() } }
/// Mention.expand for Claude: `$name` of a skill the user may invoke becomes `/name`, any other `$word` stays.
pub fn expand_claude(text: &str, ss: &[USkill]) -> String {
    if !text.contains('$') { return text.to_string() }
    text.split('\n').map(|l| l.split(' ').map(|w| { let n = mention_name(w); if !n.is_empty() && ss.iter().any(|s| s.user && s.name == n) { format!("/{}", &w[1..]) } else { w.to_string() } }).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n")
}

/// Mention.note / Mention.others: for providers without skills of their own, the text as written after one instruction per named skill (once each) to read its SKILL.md.
pub fn mention_others(text: &str, ss: &[USkill]) -> String {
    if !text.contains('$') { return text.to_string() }
    let mut seen: Vec<String> = vec![]; let mut notes: Vec<String> = vec![];
    for w in text.split('\n').flat_map(|l| l.split(' ')) {
        let n = mention_name(w); if n.is_empty() || seen.contains(&n) { continue }
        if let Some(s) = ss.iter().find(|s| s.user && s.name == n) { seen.push(n.clone()); notes.push(format!("The user invoked the ${} skill: read \"{}\" completely before anything else and follow its instructions for this request.", s.name, s.path)) }
    }
    if notes.is_empty() { text.to_string() } else { format!("<skill_invocations>\n{}\n</skill_invocations>\n\n{}", notes.join("\n"), text) }
}

#[cfg(test)] mod others_tests {
    use super::*;
    fn sk(n: &str) -> USkill { USkill { name: n.into(), description: String::new(), path: format!("/s/{n}/SKILL.md"), model: true, user: true, ..Default::default() } }
    #[test] fn mention_others_names_each_skill_once_and_keeps_the_text() {
        let t = "use $pcb, then $pcb again and $nope\nplus $bom."; let o = mention_others(t, &[sk("pcb"), sk("bom")]);
        assert!(o.starts_with("<skill_invocations>\nThe user invoked the $pcb skill: read \"/s/pcb/SKILL.md\" completely before anything else and follow its instructions for this request.\nThe user invoked the $bom skill"));
        assert!(o.ends_with("</skill_invocations>\n\nuse $pcb, then $pcb again and $nope\nplus $bom.")); assert_eq!(o.matches("$pcb skill").count(), 1);
        assert_eq!(mention_others("no skills here", &[sk("pcb")]), "no skills here"); assert_eq!(mention_others("$unknown", &[sk("pcb")]), "$unknown");
    }
}

#[cfg(test)]
mod parsing_tests {
    use super::*;
    #[test] fn skills_read_their_frontmatter_including_folded_values() {
        let s = parse_skill("---\nname: pcb-review\ndescription: >\n  Review a board\n  with care.\nuser-invocable: false\n---\nbody", "dir", "/p");
        assert_eq!((s.name.as_str(), s.description.as_str(), s.user, s.model), ("pcb-review", "Review a board with care.", false, true));
        let t = parse_skill("no frontmatter", "dflt", "/p"); assert_eq!(t.name, "dflt"); assert!(t.user);
        let q = parse_skill("---\ndescription: \"Quoted: yes\"\ndisable-model-invocation: true\n---", "d", "/p"); assert_eq!(q.description, "Quoted: yes"); assert!(!q.model);
    }
    #[test] fn dollar_mentions_become_slash_commands_for_known_user_skills_only() {
        let ss = vec![USkill { name: "pcb-review".into(), description: "".into(), path: "".into(), model: true, user: true, ..Default::default() }, USkill { name: "secret".into(), description: "".into(), path: "".into(), model: true, user: false, ..Default::default() }];
        assert_eq!(expand_claude("run $pcb-review, then $secret and $other\nnext $pcb-review", &ss), "run /pcb-review, then $secret and $other\nnext /pcb-review");
        assert_eq!(expand_claude("cost is 5", &ss), "cost is 5"); assert_eq!(mention_name("$a:b-c_d."), "a:b-c_d"); assert_eq!(mention_name("a$b"), "");
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { ClaudeUser, ClaudeProject, ClaudePlugin, Codex, Agents, Gemini, Cursor, Universal, Pi, Omp, Grok, Hub, HubLocal }

#[derive(Clone, Debug, PartialEq)]
pub struct Found { pub name: String, pub bare: String, pub description: String, pub path: String, pub group: String, pub kind: Kind, pub provider: String, pub model_pattern: String, pub model: bool, pub user: bool, pub scope: &'static str }

/// Where to look: the person's home, the hub's home, KiStack, and Codex's own home when set.
#[derive(Clone, Debug, Default)]
pub struct Env { pub home: String, pub hub_home: PathBuf, pub kistack: Option<PathBuf>, pub codex_home: Option<String> }

static ENV: OnceLock<Env> = OnceLock::new();
/// Set once at start (HostInfo::probe) for the popup, which has no host in reach.
pub fn set_env(e: Env) { let _ = ENV.set(e); }
pub fn env_from(home: &str, hub_home: &Path, kistack: Option<&Path>) -> Env { Env { home: home.into(), hub_home: hub_home.into(), kistack: kistack.map(Path::to_path_buf), codex_home: std::env::var("CODEX_HOME").ok().filter(|s| !s.is_empty()) } }

/// A linked worktree has a .git file. Its parent checkout is outside its scope.
pub fn directories(cwd: &Path) -> Vec<PathBuf> {
    if cwd.as_os_str().is_empty() || !cwd.is_absolute() { return vec![] }
    let cwd = crate::plat::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let mut dirs = vec![cwd.clone()];
    if let Some(root) = cwd.ancestors().find(|p| p.join(".git").exists()) {
        dirs = cwd.ancestors().take_while(|p| *p != root).map(Path::to_path_buf).collect();
        dirs.push(root.to_path_buf());
        dirs.reverse();
    }
    dirs
}

struct Root { dir: PathBuf, kind: Kind, group: String, plugin: String, scope: &'static str }

fn read_json(p: &Path) -> Value { std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null) }

/// Every folder skills may sit in, for a thread working in `cwd`.
fn roots(env: &Env, cwd: &str) -> Vec<Root> {
    let home = PathBuf::from(&env.home); let mut v: Vec<Root> = vec![];
    let mut add = |dir: PathBuf, kind: Kind, group: &str, plugin: &str, scope| if !v.iter().any(|r| r.dir == dir) { v.push(Root { dir, kind, group: group.into(), plugin: plugin.into(), scope }) };
    for dir in directories(Path::new(cwd)).into_iter().rev().filter(|d| *d != home) {
        for (rel, kind, group) in locations(true) { add(dir.join(rel), kind, group, "", "Project"); }
    }
    for (rel, kind, group) in locations(false) { add(home.join(rel), kind, group, "", "User"); }
    let enabled = read_json(&home.join(".claude/settings.json"));
    let installed = read_json(&home.join(".claude/plugins/installed_plugins.json"));
    for (key, list) in installed["plugins"].as_object().into_iter().flatten() {
        if enabled["enabledPlugins"][key.as_str()] == json!(false) { continue }
        let (plugin, market) = key.split_once('@').unwrap_or((key, ""));
        let group = format!("Claude · plugin {plugin}");
        let mut any = false;
        for i in list.as_array().into_iter().flatten() {
            let p = PathBuf::from(i["installPath"].as_str().unwrap_or("")); if !p.is_dir() { continue }
            any = true; add(p.join("skills"), Kind::ClaudePlugin, &group, plugin, "User");
        }
        if !any { // the install is gone: a directory marketplace is read where it is
            if let Some(m) = enabled["extraKnownMarketplaces"][market]["source"]["path"].as_str() { add(PathBuf::from(m).join("skills"), Kind::ClaudePlugin, &group, plugin, "User"); add(PathBuf::from(m), Kind::ClaudePlugin, &group, plugin, "User"); }
        }
    }
    let codex = env.codex_home.as_ref().map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
    add(codex.join("skills"), Kind::Codex, "Codex · yours", "", "User");
    if let Some(k) = &env.kistack { add(k.join("skills"), Kind::Hub, "Backplane", "", "Bundled"); }
    add(env.hub_home.join("kistack/skills"), Kind::Hub, "Backplane", "", "Bundled");
    add(env.hub_home.join("skills"), Kind::HubLocal, "Backplane", "", "Bundled"); add(env.hub_home.join("bundled/skills"), Kind::Hub, "Backplane", "", "Bundled"); // bundled.rs dir()
    v
}

fn locations(project: bool) -> [(&'static str, Kind, &'static str); 11] {
    let (claude, group, agents, pi, omp) = if project {
        (Kind::ClaudeProject, "Claude · project", "Agents · project", ".pi/skills", ".omp/skills")
    } else { (Kind::ClaudeUser, "Claude · yours", "Agents · yours", ".pi/agent/skills", ".omp/agent/skills") };
    [
        (".backplane/skills", Kind::Universal, "Backplane"),
        (".agents/skills", Kind::Agents, agents),
        (".claude/skills", claude, group),
        (".claude/commands", claude, group),
        (".codex/skills", Kind::Codex, if project { "Codex · project" } else { "Codex · yours" }),
        (".cursor/skills", Kind::Cursor, "Cursor"),
        (".gemini/skills", Kind::Gemini, "Gemini"),
        (".gemini/config/skills", Kind::Gemini, "Gemini"),
        (pi, Kind::Pi, "Pi"), (omp, Kind::Omp, "Oh My Pi"), (".grok/skills", Kind::Grok, "Grok"),
    ]
}

fn skill_path(root: &Path, entry: &str) -> PathBuf {
    let path = root.join(entry);
    if root.ends_with(".claude/commands") && entry.ends_with(".md") { path }
    else { path.join("SKILL.md") }
}

fn child_dirs(dir: &Path) -> Vec<String> { let mut ns: Vec<String> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.')).collect()).unwrap_or_default(); ns.sort(); ns }

/// Every skill a thread in `cwd` may be given, the first of a name in a group order winning nothing: a name two places hold is listed in both (the group tells them apart).
fn read_skill(path: &Path) -> Option<String> {
    use std::io::Read;
    if !path.metadata().ok()?.is_file() { return None }
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(16 * 1024).read_to_end(&mut bytes).ok()?;
    if bytes.is_empty() { return None }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn matches_scope(spec: &str, value: &str) -> bool {
    spec.is_empty() || spec.trim_matches(['[', ']']).split(',').any(|s| {
        let s = s.trim().trim_matches(['"', '\'']);
        s == "*" || s == value || s.strip_suffix('*').is_some_and(|prefix| value.starts_with(prefix))
    })
}

pub fn provider_scope(f: &Found) -> &str {
    let implied = match f.kind {
        Kind::Codex => "codex", Kind::Gemini => "antigravity", Kind::Cursor => "cursor",
        Kind::Pi => "pi", Kind::Omp => "omp", Kind::Grok => "grok",
        Kind::ClaudeUser | Kind::ClaudeProject | Kind::ClaudePlugin => "claude",
        _ => "",
    };
    if f.provider.is_empty() { implied } else { &f.provider }
}

fn applicable(f: &Found, provider: &str, model: &str) -> bool {
    let scope = provider_scope(f);
    (matches_scope(scope, provider) || provider == "antigravity" && matches_scope(scope, "gemini"))
        && matches_scope(&f.model_pattern, model)
}

pub fn for_model(ss: &[Found], provider: &str, model: &str) -> Vec<Found> {
    let mut candidates: Vec<_> = ss.iter().filter(|f| applicable(f, provider, model)).collect();
    // Stable sort keeps the closest directory ahead of its ancestors within the same scope.
    candidates.sort_by_key(|f| (match f.scope { "Project" => 0, "User" => 1, _ => 2 },
        f.provider.is_empty() && matches!(f.kind, Kind::Universal | Kind::Agents | Kind::Hub | Kind::HubLocal), f.model_pattern.is_empty()));
    let mut seen = std::collections::HashSet::new();
    candidates.into_iter().filter(|f| seen.insert(&f.name)).cloned().collect()
}

pub fn prompt_index(ss: &[Found], provider: &str, isolated: bool) -> String {
    let mut lines = String::new();
    for f in ss.iter().filter(|f| f.model && !native(f, provider, isolated)) {
        let description: String = f.description.chars().take(300).collect();
        let entry = json!({"name":f.name, "description":description, "path":f.path}).to_string()
            .replace('<', "\\u003c").replace('>', "\\u003e");
        if lines.len() + entry.len() > 16 * 1024 { break }
        lines.push_str(&entry);
        lines.push('\n');
    }
    if lines.is_empty() { lines } else {
        format!("<available_skills>\nRead the matching skill file completely before applying it.\n{lines}</available_skills>")
    }
}

pub fn scan_all(env: &Env, cwd: &str) -> Vec<Found> {
    let mut out: Vec<Found> = vec![];
    for r in roots(env, cwd) {
        for d in child_dirs(&r.dir) {
            let p = skill_path(&r.dir, &d);
            let Some(t) = read_skill(&p) else { continue };
            let s = parse_skill(&t, d.strip_suffix(".md").unwrap_or(&d), &p.to_string_lossy());
            let name = if r.plugin.is_empty() { s.name.clone() } else { format!("{}:{}", r.plugin, s.name) };
            if out.iter().any(|f| f.path == s.path) { continue }
            out.push(Found { name, bare: s.name, description: s.description, path: s.path, group: r.group.clone(), kind: r.kind, provider: s.provider, model_pattern: s.model_pattern, model: s.model, user: s.user, scope: r.scope });
        }
    }
    out
}

/// What the folders look like now (modified times of each folder, skill folder and SKILL.md): changes when a skill is added, edited or removed.
fn fingerprint(env: &Env, cwd: &str) -> u64 {
    use std::hash::{Hash, Hasher}; let mut h = std::collections::hash_map::DefaultHasher::new();
    let mt = |p: &Path| crate::plat::fs::change_stamp(p);
    for f in ["settings.json", "plugins/installed_plugins.json"] { mt(&Path::new(&env.home).join(".claude").join(f)).hash(&mut h) }
    for r in roots(env, cwd) {
        r.dir.hash(&mut h); mt(&r.dir).hash(&mut h);
        for d in child_dirs(&r.dir) { mt(&r.dir.join(&d)).hash(&mut h); mt(&skill_path(&r.dir, &d)).hash(&mut h) }
    }
    h.finish()
}

static CACHE: OnceLock<Mutex<HashMap<String, (u64, Arc<Vec<Found>>)>>> = OnceLock::new();
/// The skills for `cwd`, rescanned only when a folder changed.
pub fn index(env: &Env, cwd: &str) -> Arc<Vec<Found>> {
    let fp = fingerprint(env, cwd); let key = format!("{}|{}|{cwd}", env.home, env.hub_home.display());
    let m = CACHE.get_or_init(Default::default);
    if let Some((f, v)) = m.lock().unwrap().get(&key) { if *f == fp { return v.clone() } }
    let v = Arc::new(scan_all(env, cwd)); let mut g = m.lock().unwrap(); if g.len() > 64 { g.clear() } g.insert(key, (fp, v.clone())); v
}
/// The popup's index, over the environment set at start.
pub fn popup_index(cwd: &str) -> Arc<Vec<Found>> { match ENV.get() { Some(e) => index(e, cwd), None => Arc::new(vec![]) } }

/// Whether `provider` loads this skill itself. `isolated`: the turn runs under an account profile or the person mode, whose Claude reads neither the person's skills nor their plugins.
pub fn native(f: &Found, provider: &str, isolated: bool) -> bool {
    match (provider, f.kind) {
        ("claude", Kind::Hub | Kind::ClaudeProject) => true,
        ("claude", Kind::ClaudeUser | Kind::ClaudePlugin) => !isolated,
        ("codex", Kind::Codex) => !isolated && f.scope != "Project",
        ("codex" | "pi" | "omp" | "opencode", Kind::Agents) => true,
        ("antigravity", Kind::Agents) => f.scope == "Project",
        ("antigravity", Kind::Gemini) => f.scope == "User",
        ("pi", Kind::Pi) | ("omp", Kind::Omp | Kind::ClaudeUser | Kind::ClaudeProject | Kind::ClaudePlugin | Kind::Codex) |
        ("grok", Kind::Grok) | ("opencode", Kind::ClaudeUser | Kind::ClaudeProject) => true,
        ("pi", Kind::Hub) => f.path.replace('\\', "/").contains("/kistack/skills/"),
        _ => false,
    }
}
/// A skill word (`$name`, or the bare name of a plugin's skill when only one has it) to its skill.
pub fn lookup<'a>(ss: &'a [Found], name: &str) -> Option<&'a Found> {
    ss.iter().find(|f| f.name == name).or_else(|| { let m: Vec<&Found> = ss.iter().filter(|f| f.bare == name).collect(); if m.len() == 1 { Some(m[0]) } else { None } })
}
fn as_u(f: &Found) -> USkill { USkill { name: f.name.clone(), description: f.description.clone(), path: f.path.clone(), model: f.model, user: f.user, ..Default::default() } }
fn canon(text: &str, ss: &[Found]) -> String { // a bare plugin skill word becomes its full name
    if !text.contains('$') { return text.to_string() }
    text.split('\n').map(|l| l.split(' ').map(|w| { let n = mention_name(w); match lookup(ss, &n) { Some(f) if !n.is_empty() && f.name != n => format!("${}{}", f.name, &w[1 + n.len()..]), _ => w.to_string() } }).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n")
}

/// The text a turn gets: `$name` of a skill the provider loads itself is made real (Claude `/name`; Codex keeps its own `$name`), and any other found skill is named in a leading
/// note that has the agent read its SKILL.md first. Words that name no skill stay as written.
pub fn prepare(text: &str, ss: &[Found], provider: &str, isolated: bool) -> String {
    if !text.contains('$') || ss.is_empty() { return text.to_string() }
    let text = canon(text, ss);
    let (nat, rd): (Vec<&Found>, Vec<&Found>) = ss.iter().partition(|f| native(f, provider, isolated));
    let reads: Vec<USkill> = rd.into_iter().map(as_u).collect();
    let noted = if provider == "claude" { mention_others(&text, &reads) } else { mention_others(&text, &ss.iter().map(as_u).collect::<Vec<_>>()) }; // Codex and the rest are told the file as well: it is the same instruction
    if provider == "claude" { let n: Vec<USkill> = nat.into_iter().map(as_u).collect(); expand_claude(&noted, &n) } else { noted }
}

/// How a mentioned skill stands for the thread's provider: "" loaded by the provider, "read from file" named to the agent, and None for a word that is no skill.
pub fn status(ss: &[Found], word: &str, provider: &str, isolated: bool) -> Option<(Found, &'static str)> {
    let f = lookup(ss, word).filter(|f| f.user)?; Some((f.clone(), if native(f, provider, isolated) { "" } else { "read from file" }))
}

/// Chips for the `$skill` words of a sent message: `{word, kind:"skill", id:"", label, note}`. A lower-case word with a dash or colon that names nothing says so (`$HOME`, `$5` are left alone).
pub fn chips(ss: &[Found], text: &str, provider: &str, isolated: bool) -> Vec<Value> {
    let mut out: Vec<Value> = vec![]; let mut seen: Vec<String> = vec![];
    for w in text.split(|c: char| c.is_whitespace()) {
        let w = w.trim_start_matches(|c| matches!(c, '(' | '[' | '"' | '\'' | '`')); let n = mention_name(w);
        if n.is_empty() || seen.contains(&n) { continue } seen.push(n.clone());
        let (label, note) = match status(ss, &n, provider, isolated) {
            Some((f, note)) => (f.name, note.to_string()),
            None if n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == ':') && (n.contains('-') || n.contains(':')) => (n.clone(), "no such skill here".to_string()),
            None => continue,
        };
        out.push(json!({"word": format!("${n}"), "kind": "skill", "id": "", "label": if note.is_empty() { label } else { format!("{label} · {note}") }, "note": note}));
    }
    out
}

fn score(f: &Found, q: &str) -> u32 {
    if q.is_empty() { return 1 }
    let n = f.name.to_ascii_lowercase().replace(':', "-");
    if f.bare.to_ascii_lowercase().starts_with(q) { return 100 } if n.starts_with(q) { return 90 }
    if n.split('-').any(|p| p.starts_with(q)) { return 60 } if n.contains(q) { return 40 }
    if f.description.to_ascii_lowercase().contains(q) { return 20 }
    let mut it = n.chars(); if q.chars().all(|c| it.any(|x| x == c)) { 10 } else { 0 }
}
pub const MAX: usize = 12;

/// The `$` popup rows for the word being typed (`$` and then a slug), best matches first, a source's skills together: `{word, label, desc, group, note}`.
/// A pasted address (`$https://…/SKILL.md`) is one row that offers to install it.
pub fn rows(ss: &[Found], word: &str, provider: &str, isolated: bool) -> Vec<Value> {
    let Some(q) = word.strip_prefix('$') else { return vec![] };
    if let Some(url) = q.strip_prefix("install?") {
        let name = install_name(url); let dir = if provider == "codex" { "~/.agents/skills" } else { "~/.claude/skills" };
        let have = ENV.get().map_or(false, |e| install_dir(&e.home, provider).join(&name).join("SKILL.md").exists()); // the skill is there already: the confirm says it replaces it
        if have { return vec![json!({"word": format!("$replace!{url}"), "label": format!("confirm: replace {name}"), "desc": format!("{dir}/{name} is installed already; this overwrites its SKILL.md with {url}"), "group": "Install from a link", "note": "replaces"})] }
        return vec![json!({"word": format!("$install!{url}"), "label": format!("confirm: install {name}"), "desc": format!("writes {dir}/{name}/SKILL.md from {url}"), "group": "Install from a link", "note": ""})];
    }
    if q.starts_with("http://") || q.starts_with("https://") {
        let name = install_name(q);
        return vec![json!({"word": format!("$install:{q}"), "label": format!("install {name}"), "desc": format!("from {q}"), "group": "Install from a link", "note": ""})];
    }
    if !q.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':')) { return vec![] }
    let q = q.to_ascii_lowercase().replace(':', "-");
    let mut c: Vec<(u32, bool, usize, &Found)> = ss.iter().filter(|f| f.user).map(|f| (score(f, &q), native(f, provider, isolated), 0, f)).filter(|x| x.0 > 0).collect();
    let rank = |g: &str| -> usize { let pre = match provider { "claude" => "Claude", "codex" => "Codex", _ => "" }; if g == "Backplane" { 2 } else if g.starts_with("Agents") && provider == "codex" { 1 } else if !pre.is_empty() && g.starts_with(pre) { 0 } else { 3 } };
    for x in c.iter_mut() { x.2 = rank(&x.3.group) }
    c.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.3.name.cmp(&b.3.name))); c.truncate(MAX);
    c.sort_by(|a, b| a.2.cmp(&b.2).then(a.3.group.cmp(&b.3.group))); // stable: score order stays within a source
    c.into_iter().map(|(_, nat, _, f)| { let note = if nat { "" } else { "read from file" };
        json!({"word": format!("${}", f.name), "label": format!("${}", f.name), "desc": f.description, "group": f.group, "note": note}) }).collect()
}

// ---- installing from a link ----
/// A GitHub page address to the file's raw address; any other address as it is.
pub fn raw_url(url: &str) -> String {
    let u = url.trim();
    if let Some(r) = u.strip_prefix("https://github.com/") { let p: Vec<&str> = r.split('/').collect(); if p.len() >= 5 && p[2] == "blob" { return format!("https://raw.githubusercontent.com/{}/{}/{}", p[0], p[1], p[3..].join("/")) } }
    u.to_string()
}
/// The skill's folder name from its address: `…/skills/unslop/SKILL.md` is "unslop".
pub fn install_name(url: &str) -> String {
    let parts: Vec<&str> = url.split(['?', '#']).next().unwrap_or("").trim_end_matches('/').split('/').collect();
    let mut i = parts.len(); if parts.last().map_or(false, |l| l.eq_ignore_ascii_case("skill.md")) { i -= 1 }
    let n = parts.get(i.wrapping_sub(1)).copied().unwrap_or("skill");
    let s: String = n.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.trim_matches('-').to_string(); if s.is_empty() || s.starts_with("http") { "skill".into() } else { s }
}
/// Where a skill for `provider` is installed: Codex reads ~/.agents/skills, every other provider's skills go to ~/.claude/skills.
pub fn install_dir(home: &str, provider: &str) -> PathBuf { PathBuf::from(home).join(if provider == "codex" { ".agents/skills" } else { ".claude/skills" }) }
/// The checks before a downloaded file is written: a SKILL.md with a front block, not large, and a name that is a plain folder name.
pub fn check_download(body: &str, url: &str) -> Result<(String, String), String> {
    if body.len() > 256 * 1024 { return Err("the file is over 256 KB".into()) }
    if !body.trim_start().starts_with("---") { return Err("not a SKILL.md: no front block".into()) }
    let s = parse_skill(body, &install_name(url), url);
    let name: String = s.name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).collect();
    if name.is_empty() || name != s.name { return Err("the skill's name is not a plain folder name".into()) }
    Ok((name, s.description))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(d: &Path, rel: &str, body: &str) { let p = d.join(rel); std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, body).unwrap(); }
    fn sk(n: &str, d: &str) -> String { format!("---\nname: {n}\ndescription: {d}\n---\nbody") }
    fn tmp(tag: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("skills-{tag}")) }

    #[test] fn every_provider_place_is_read_with_its_source() {
        let t = tmp("places"); let (home, proj) = (t.join("h"), t.join("h/work/proj"));
        put(&home, ".claude/skills/unslop/SKILL.md", &sk("unslop", "cut the filler")); put(&proj, ".claude/skills/local/SKILL.md", &sk("local", "project one"));
        put(&home, ".codex/skills/cdx/SKILL.md", &sk("cdx", "codex one")); put(&home, ".agents/skills/shared/SKILL.md", &sk("shared", "agents one"));
        put(&t, "plug/skills/review/SKILL.md", &sk("review", "plugin one"));
        put(&home, ".claude/plugins/installed_plugins.json", &serde_json::json!({"plugins":{"pk@mk":[{"installPath": t.join("plug")}]}}).to_string());
        put(&home, ".claude/skills/hidden/SKILL.md", "---\nname: hidden\nuser-invocable: false\n---\n");
        let env = Env { home: home.to_string_lossy().into(), hub_home: t.join("hub"), kistack: None, codex_home: None };
        put(&t, "hub/skills/own/SKILL.md", &sk("own", "backplane one"));
        let ss = scan_all(&env, &proj.to_string_lossy()); let get = |n: &str| ss.iter().find(|f| f.name == n).unwrap_or_else(|| panic!("{n} in {ss:?}"));
        assert_eq!((get("unslop").group.as_str(), get("unslop").kind), ("Claude · yours", Kind::ClaudeUser)); assert_eq!(get("local").group, "Claude · project");
        assert_eq!(get("cdx").group, "Codex · yours"); assert_eq!(get("shared").group, "Agents · yours"); assert_eq!(get("pk:review").group, "Claude · plugin pk"); assert_eq!(get("pk:review").bare, "review");
        assert_eq!(get("own").group, "Backplane"); assert!(!get("hidden").user);
        assert!(lookup(&ss, "review").is_some(), "a plugin's skill by its bare name when it is the only one");
        std::fs::remove_dir_all(&t).ok();
    }
    #[test] fn a_plugin_settings_turn_off_is_left_out_and_a_new_skill_shows_at_once() {
        let t = tmp("fresh"); let home = t.join("h");
        put(&t, "plug/skills/review/SKILL.md", &sk("review", "x"));
        put(&home, ".claude/plugins/installed_plugins.json", &serde_json::json!({"plugins":{"pk@mk":[{"installPath": t.join("plug")}]}}).to_string());
        put(&home, ".claude/settings.json", "{\"enabledPlugins\":{\"pk@mk\":false}}");
        let env = Env { home: home.to_string_lossy().into(), hub_home: t.join("hub"), kistack: None, codex_home: None };
        assert!(index(&env, "").is_empty(), "disabled plugin");
        put(&home, ".claude/skills/new/SKILL.md", &sk("new", "d")); assert_eq!(index(&env, "").len(), 1, "the index follows the files");
        put(&home, ".claude/skills/new/SKILL.md", &sk("new", "changed text")); assert_eq!(index(&env, "")[0].description, "changed text");
        std::fs::remove_dir_all(&t).ok();
    }
    fn f(n: &str, kind: Kind, group: &str) -> Found { Found { name: n.into(), bare: n.rsplit(':').next().unwrap().into(), description: format!("{n} does things"), path: format!("/s/{n}/SKILL.md"), group: group.into(), kind, provider: String::new(), model_pattern: String::new(), model: true, user: true, scope: "User" } }
    #[test] fn claude_gets_slash_for_what_it_loads_and_a_file_note_for_the_rest() {
        let ss = vec![f("unslop", Kind::ClaudeUser, "Claude · yours"), f("cdx", Kind::Codex, "Codex · yours"), f("pk:review", Kind::ClaudePlugin, "p")];
        let o = prepare("run $unslop and $cdx then $review and $nope", &ss, "claude", false);
        assert!(o.contains("run /unslop and $cdx then /pk:review and $nope"), "{o}"); assert!(o.contains("read \"/s/cdx/SKILL.md\"") && !o.contains("/s/unslop"), "{o}");
        let iso = prepare("run $unslop", &ss, "claude", true); assert!(iso.contains("read \"/s/unslop/SKILL.md\"") && iso.ends_with("run $unslop"), "an isolated Claude does not see the person's skills: {iso}");
        let cx = prepare("use $cdx", &ss, "codex", false); assert!(cx.ends_with("use $cdx") && cx.contains("/s/cdx/SKILL.md"), "{cx}");
        assert_eq!(prepare("cost $5", &ss, "claude", false), "cost $5");
    }
    #[test] fn chips_say_how_the_provider_gets_each_skill() {
        let ss = vec![f("unslop", Kind::ClaudeUser, "g"), f("cdx", Kind::Codex, "g")];
        let c = chips(&ss, "a $unslop b $cdx c $made-up d $HOME e $5 f $unslop", "claude", false);
        assert_eq!(c.len(), 3); assert_eq!((c[0]["label"].as_str(), c[0]["note"].as_str()), (Some("unslop"), Some("")));
        assert_eq!(c[1]["note"], "read from file"); assert_eq!(c[2]["note"], "no such skill here"); assert!(c[2]["label"].as_str().unwrap().contains("made-up"));
    }
    #[test] fn the_popup_searches_groups_and_offers_an_install_row() {
        let ss = vec![f("unslop", Kind::ClaudeUser, "Claude · yours"), f("zeta", Kind::Codex, "Codex · yours"), f("pk:slopless", Kind::ClaudePlugin, "Claude · plugin pk"), f("own", Kind::Hub, "Backplane")];
        let r = rows(&ss, "$slop", "claude", false); let w: Vec<&str> = r.iter().map(|x| x["word"].as_str().unwrap()).collect();
        assert_eq!(w, vec!["$pk:slopless", "$unslop"], "{w:?}"); // Claude's sources first, a source's rows together
        assert_eq!(rows(&ss, "$", "claude", false).len(), 4); assert_eq!(rows(&ss, "$", "claude", false)[3]["group"], "Codex · yours");
        assert_eq!(rows(&ss, "$", "claude", false)[3]["note"], "read from file"); assert!(rows(&ss, "$zz", "claude", false).is_empty());
        let i = rows(&ss, "$https://github.com/cursor/plugins/blob/main/pstack/skills/unslop/SKILL.md", "claude", false); assert_eq!(i.len(), 1); assert_eq!(i[0]["label"], "install unslop");
    }
    #[test] fn a_link_is_read_into_a_raw_address_and_a_folder_name() {
        assert_eq!(raw_url("https://github.com/cursor/plugins/blob/main/pstack/skills/unslop/SKILL.md"), "https://raw.githubusercontent.com/cursor/plugins/main/pstack/skills/unslop/SKILL.md");
        assert_eq!(install_name("https://github.com/cursor/plugins/blob/main/pstack/skills/unslop/SKILL.md"), "unslop"); assert_eq!(install_name("https://x.org/a/b/"), "b");
        assert!(check_download("---\nname: unslop\ndescription: d\n---\n", "u").is_ok()); assert!(check_download("hello", "u").is_err()); assert!(check_download("---\nname: ../x\n---\n", "u").is_err());
        assert_eq!(install_dir("/h", "codex"), PathBuf::from("/h/.agents/skills")); assert_eq!(install_dir("/h", "claude"), PathBuf::from("/h/.claude/skills"));
    }
}
