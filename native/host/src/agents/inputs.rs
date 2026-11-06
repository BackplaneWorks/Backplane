//! What a message and a project give an agent besides the prompt: the project's KiCad files,
//! skills and `$skill` mentions, and attached images.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

// ---- KiCad project files ----
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Files { pub pro: String, pub pcb: String, pub sch: String, pub all: String }

/// A folder's name that says it holds copies.
fn copies(c: &str) -> bool {
    let l = c.to_lowercase();
    l.starts_with("archive") || l.starts_with("build") || l.contains("backup") || l == "old" || l == "out" || l.starts_with("output") || l == "tmp" || l == "temp" || l == "staging" || l == "scratch" || l == "renders" || l.starts_with("export")
}
fn walk(root: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(root.join(rel)) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned(); let p = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        let Ok(t) = e.file_type() else { continue };
        if t.is_dir() { if depth < 4 && !name.starts_with('.') && name != "node_modules" && !name.ends_with("-backups") { walk(root, &p, depth + 1, out) } }
        else if name.ends_with(".kicad_pro") && depth >= 1 { out.push(p) }
    }
}
/// Files.found: every .kicad_pro four folders deep, paths from the root, shallowest first, folders of copies after the rest, at most 50.
pub fn found(root: &Path) -> Vec<String> {
    let mut v = vec![]; walk(root, "", 1, &mut v);
    let key = |p: &String| { let parts: Vec<&str> = p.split('/').collect(); let c = parts[..parts.len() - 1].iter().any(|d| copies(d)) as u8; (c, parts.len(), p.clone()) };
    v.sort_by_key(key); v.truncate(50); v
}
fn abs(root: &str, rel: &str) -> String { if rel.is_empty() { String::new() } else if rel.starts_with('/') { rel.into() } else { format!("{root}/{rel}") } }
fn dir_of(p: &str) -> &str { p.rsplit_once('/').map_or("", |x| x.0) }
fn last(p: &str) -> &str { p.rsplit('/').next().unwrap_or(p) }
fn named(ns: &[String], stem: &str, ext: &str) -> String { let want = format!("{stem}{ext}"); if ns.contains(&want) { want } else { ns.iter().find(|n| n.ends_with(ext)).cloned().unwrap_or_default() } }
/// Files.read: `.backplane.json`'s project/pcb/schematic win; else the .kicad_pro in the root, else the shallowest found.
pub fn read(root: &str) -> Files {
    let cfg: Value = std::fs::read_to_string(format!("{root}/.backplane.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    let g = |k: &str| cfg.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let names = |d: &str| -> Vec<String> { let mut v: Vec<String> = std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default(); v.sort(); v };
    let all = found(Path::new(root));
    let top = names(root).into_iter().find(|n| n.ends_with(".kicad_pro")).unwrap_or_default();
    let pro = if !g("project").is_empty() { g("project") } else if !top.is_empty() { top } else { all.first().cloned().unwrap_or_default() };
    let ns = names(&if dir_of(&pro).is_empty() { root.to_string() } else { abs(root, dir_of(&pro)) });
    let stem = last(&pro).strip_suffix(".kicad_pro").unwrap_or(last(&pro)).to_string();
    let inn = |d: &str, n: &str| if d.is_empty() || n.is_empty() { n.to_string() } else { format!("{d}/{n}") };
    let pick = |k: &str, ext: &str| abs(root, &if g(k).is_empty() { inn(dir_of(&pro), &named(&ns, &stem, ext)) } else { g(k) });
    Files { pro: abs(root, &pro), pcb: pick("pcb", ".kicad_pcb"), sch: pick("schematic", ".kicad_sch"), all: all.join("\n") }
}

// ---- skills ----
#[derive(Clone, Debug, PartialEq)]
pub struct USkill { pub name: String, pub description: String, pub path: String, pub model: bool, pub user: bool }

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
    if ls.first().map_or(false, |l| l.trim() == "---") {
        for l in fold(&ls[1..]) { if l.trim() == "---" { break }
            if let Some(v) = field(&l, "name") { n = v } if let Some(v) = field(&l, "description") { d = v }
            if let Some(v) = field(&l, "disable-model-invocation") { m = v } if let Some(v) = field(&l, "user-invocable") { u = v } }
    }
    let flag = |v: &str, dv: bool| match v.to_lowercase().as_str() { "true" => true, "false" => false, _ => dv };
    USkill { name: n, description: d, path: path.into(), model: !flag(&m, false), user: flag(&u, true) }
}
fn dir_skills(root: &Path) -> Vec<USkill> {
    let mut ns: Vec<String> = std::fs::read_dir(root).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default(); ns.sort();
    ns.into_iter().filter_map(|d| { let p = root.join(&d).join("SKILL.md"); let t = std::fs::read_to_string(&p).ok()?; if t.is_empty() { None } else { Some(parse_skill(&t, &d, &p.to_string_lossy())) } }).collect()
}
/// USkill.merge: the first list wins a name both use.
pub fn merge(us: Vec<USkill>, ps: Vec<USkill>) -> Vec<USkill> { let mut v = us; for p in ps { if !v.iter().any(|x| x.name == p.name) { v.push(p) } } v }
/// Skills.scan: ~/.claude/skills, then <cwd>/.claude/skills (the user's win a clash), then KiStack's.
pub fn scan(home: &str, cwd: &str, kistack: Option<&Path>) -> Vec<USkill> {
    let ks = kistack.map(|k| dir_skills(&k.join("skills"))).unwrap_or_default();
    merge(merge(dir_skills(&PathBuf::from(home).join(".claude/skills")), dir_skills(&PathBuf::from(cwd).join(".claude/skills"))), ks)
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

// ---- attachments ----
pub const INLINE_MAX: u64 = 3_932_160;
fn is_att(l: &str) -> bool { let t = l.trim(); t.starts_with("[attachment: ") && t.ends_with(']') }
fn att_path(l: &str) -> String { let t = l.trim(); t[13..t.len() - 1].trim().to_string() }
/// Att.paths: the attachment lines at the end of a message (blank lines between them allowed).
pub fn att_paths(text: &str) -> Vec<String> {
    let ls: Vec<&str> = text.split('\n').collect(); let mut out = vec![]; let mut i = ls.len();
    while i > 0 { let l = ls[i - 1]; if is_att(l) { out.push(att_path(l)); i -= 1 } else if l.trim().is_empty() { i -= 1 } else { break } }
    out.reverse(); out
}
pub fn mime(path: &str) -> &'static str { match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() { "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "webp" => "image/webp", _ => "" } }
pub fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"; let mut o = String::with_capacity(bytes.len() / 3 * 4 + 4);
    for c in bytes.chunks(3) { let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(A[(n >> 18) as usize & 63] as char); o.push(A[(n >> 12) as usize & 63] as char); o.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' }); o.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' }) }
    o
}
/// Media.img: an attached image small enough to send inline (1 byte up to INLINE_MAX); bigger ones Claude reads by path.
pub fn read_images(text: &str) -> Vec<(String, String)> {
    att_paths(text).into_iter().filter(|p| !mime(p).is_empty()).filter_map(|p| { let n = std::fs::metadata(&p).ok()?.len(); if n == 0 || n > INLINE_MAX { return None } Some((mime(&p).to_string(), b64(&std::fs::read(&p).ok()?))) }).collect()
}
/// Att.claude + Claude.tag: the stream-json user line, images first, then the text; the command's uuid first.
pub fn claude_line(text: &str, imgs: &[(String, String)], uuid: &str) -> String { claude_line_p(text, imgs, uuid, false) }
/// `now`: the CLI's `priority: "now"`: a person's message into a running turn interrupts it and is taken at once (a steer), instead of waiting for the turn's end.
pub fn claude_line_p(text: &str, imgs: &[(String, String)], uuid: &str, now: bool) -> String {
    let mut content: Vec<Value> = imgs.iter().map(|(m, d)| json!({"type":"image","source":{"type":"base64","media_type":m,"data":d}})).collect();
    content.push(json!({"type":"text","text":text}));
    let mut o = serde_json::Map::new(); if !uuid.is_empty() { o.insert("uuid".into(), json!(uuid)); }
    if now { o.insert("priority".into(), json!("now")); }
    o.insert("type".into(), json!("user")); o.insert("message".into(), json!({"role":"user","content":content})); Value::Object(o).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-in-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn touch(p: &Path) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, "x").unwrap() }
    #[test] fn a_steering_line_carries_priority_now_and_a_plain_one_does_not() {
        let l: Value = serde_json::from_str(&claude_line_p("x", &[], "U", true)).unwrap(); assert_eq!(l["priority"], "now");
        let l: Value = serde_json::from_str(&claude_line("x", &[], "U")).unwrap(); assert!(l.get("priority").is_none());
    }
    #[test] fn kicad_files_prefer_the_root_then_the_shallowest() {
        let r = tmp("k1"); touch(&r.join("hw/a/a.kicad_pro")); touch(&r.join("hw/a/a.kicad_pcb")); touch(&r.join("hw/a/a.kicad_sch")); touch(&r.join("archive/old/z.kicad_pro")); touch(&r.join(".hidden/h.kicad_pro")); touch(&r.join("node_modules/n.kicad_pro"));
        let f = read(r.to_str().unwrap()); let rs = r.to_str().unwrap();
        assert_eq!(f.pro, format!("{rs}/hw/a/a.kicad_pro")); assert_eq!(f.pcb, format!("{rs}/hw/a/a.kicad_pcb")); assert_eq!(f.sch, format!("{rs}/hw/a/a.kicad_sch")); assert_eq!(f.all, "hw/a/a.kicad_pro\narchive/old/z.kicad_pro");
        touch(&r.join("top.kicad_pro")); assert_eq!(read(rs).pro, format!("{rs}/top.kicad_pro"));
    }
    #[test] fn backplane_json_wins() {
        let r = tmp("k2"); touch(&r.join("a/a.kicad_pro")); touch(&r.join("b/b.kicad_pro")); std::fs::write(r.join(".backplane.json"), r#"{"project":"b/b.kicad_pro","pcb":"b/x.kicad_pcb"}"#).unwrap();
        let f = read(r.to_str().unwrap()); assert!(f.pro.ends_with("b/b.kicad_pro") && f.pcb.ends_with("b/x.kicad_pcb")); assert_eq!(f.sch, "");
    }
    #[test] fn none_found_is_empty() { assert_eq!(read(tmp("k3").to_str().unwrap()), Files { pro: "".into(), pcb: "".into(), sch: "".into(), all: "".into() }); }
    #[test] fn skills_read_their_frontmatter_including_folded_values() {
        let s = parse_skill("---\nname: pcb-review\ndescription: >\n  Review a board\n  with care.\nuser-invocable: false\n---\nbody", "dir", "/p");
        assert_eq!((s.name.as_str(), s.description.as_str(), s.user, s.model), ("pcb-review", "Review a board with care.", false, true));
        let t = parse_skill("no frontmatter", "dflt", "/p"); assert_eq!(t.name, "dflt"); assert!(t.user);
        let q = parse_skill("---\ndescription: \"Quoted: yes\"\ndisable-model-invocation: true\n---", "d", "/p"); assert_eq!(q.description, "Quoted: yes"); assert!(!q.model);
    }
    #[test] fn dollar_mentions_become_slash_commands_for_known_user_skills_only() {
        let ss = vec![USkill { name: "pcb-review".into(), description: "".into(), path: "".into(), model: true, user: true }, USkill { name: "secret".into(), description: "".into(), path: "".into(), model: true, user: false }];
        assert_eq!(expand_claude("run $pcb-review, then $secret and $other\nnext $pcb-review", &ss), "run /pcb-review, then $secret and $other\nnext /pcb-review");
        assert_eq!(expand_claude("cost is 5", &ss), "cost is 5"); assert_eq!(mention_name("$a:b-c_d."), "a:b-c_d"); assert_eq!(mention_name("a$b"), "");
    }
    #[test] fn user_skills_win_a_name_clash() { let u = |n: &str, p: &str| USkill { name: n.into(), description: "".into(), path: p.into(), model: true, user: true }; let m = merge(vec![u("a", "user")], vec![u("a", "proj"), u("b", "proj")]); assert_eq!(m.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(), ["user", "proj"]); }
    #[test] fn attachment_lines_at_the_end_are_the_attachments() {
        assert_eq!(att_paths("Look\n\n[attachment: /a/x.png]\n\n[attachment: /a/y.pdf]"), ["/a/x.png", "/a/y.pdf"]); assert!(att_paths("no\n[attachment: /a/x.png]\nmore").is_empty()); assert_eq!(mime("/a/X.JPEG"), "image/jpeg"); assert_eq!(mime("/a/x.pdf"), "");
    }
    #[test] fn base64_matches_the_standard() { assert_eq!(b64(b""), ""); assert_eq!(b64(b"f"), "Zg=="); assert_eq!(b64(b"fo"), "Zm8="); assert_eq!(b64(b"foo"), "Zm9v"); assert_eq!(b64(b"foobar"), "Zm9vYmFy"); }
    #[test] fn images_go_first_in_the_line_and_big_ones_stay_paths() {
        let r = tmp("img"); let ok = r.join("a.png"); std::fs::write(&ok, [1u8, 2, 3]).unwrap(); let big = r.join("b.png"); std::fs::write(&big, vec![0u8; INLINE_MAX as usize + 1]).unwrap(); let empty = r.join("c.png"); std::fs::write(&empty, b"").unwrap();
        let text = format!("hi\n\n[attachment: {}]\n[attachment: {}]\n[attachment: {}]", ok.display(), big.display(), empty.display());
        let imgs = read_images(&text); assert_eq!(imgs, vec![("image/png".to_string(), "AQID".to_string())]);
        let l: Value = serde_json::from_str(&claude_line(&text, &imgs, "U")).unwrap(); assert_eq!(l["uuid"], "U"); assert_eq!(l["message"]["content"][0]["type"], "image"); assert_eq!(l["message"]["content"][1]["text"], text.as_str());
    }
}

#[cfg(test)] mod others_tests {
    use super::*;
    fn sk(n: &str) -> USkill { USkill { name: n.into(), description: String::new(), path: format!("/s/{n}/SKILL.md"), model: true, user: true } }
    #[test] fn mention_others_names_each_skill_once_and_keeps_the_text() {
        let t = "use $pcb, then $pcb again and $nope\nplus $bom."; let o = mention_others(t, &[sk("pcb"), sk("bom")]);
        assert!(o.starts_with("<skill_invocations>\nThe user invoked the $pcb skill: read \"/s/pcb/SKILL.md\" completely before anything else and follow its instructions for this request.\nThe user invoked the $bom skill"));
        assert!(o.ends_with("</skill_invocations>\n\nuse $pcb, then $pcb again and $nope\nplus $bom.")); assert_eq!(o.matches("$pcb skill").count(), 1);
        assert_eq!(mention_others("no skills here", &[sk("pcb")]), "no skills here"); assert_eq!(mention_others("$unknown", &[sk("pcb")]), "$unknown");
    }
}
