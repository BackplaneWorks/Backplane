//! The project home's snapshot (src/server/effects/project-home.js `bp_project_home`, core/project-home.bend): the files that make a project's home
//! page, read within the project's folder only. `project.home {project, op?}`: no op is the snapshot; `document` reads a window of one of its files;
//! `intent` writes `.backplane/intent.md` (revision-checked, atomically) and answers the snapshot. Reads are bounded (windows, 256 scanned files, 32 rows).
//! Reads are whole-file with caps (8 MiB for the fab/config metadata, 16 MiB for a saved check report; a bigger file counts as unreadable): the JS effect streams them in constant memory.
//! Not ported: the `price` op's `price` field (the order server's estimate for the fab files; settings/fab.rs owns that client).
use crate::files::git;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn hash(b: &[u8]) -> String { hex::encode(Sha256::digest(b)) }
fn ms(t: std::time::SystemTime) -> f64 { t.duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64() * 1000.0).floor() }
fn num(v: &Value, k: &str) -> u64 { match v.get(k) { Some(Value::Number(n)) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)).unwrap_or(0), Some(Value::String(s)) => s.parse().unwrap_or(0), _ => 0 } }
fn st(v: &Value, k: &str) -> String { v.get(k).and_then(Value::as_str).unwrap_or("").to_string() }

struct Home { real: PathBuf }
struct F { path: PathBuf, mtime: f64, size: u64 }
impl Home {
    /// file(): a regular file inside the project (links resolved), else None.
    fn file(&self, rel: &str) -> Option<F> {
        if rel.is_empty() { return None }
        let p = std::fs::canonicalize(self.real.join(rel)).ok()?;
        if !(p == self.real || p.starts_with(&self.real)) { return None }
        let m = std::fs::metadata(&p).ok()?; if !m.is_file() { return None }
        Some(F { path: p, mtime: ms(m.modified().ok()?), size: m.len() })
    }
    /// text(): a window of a file as UTF-8 (a window may cut a character: up to three bytes are given back).
    fn text(&self, rel: &str, limit: u64, offset: u64) -> Result<Option<(F, String, u64, Option<u64>)>, String> {
        use std::io::{Read, Seek, SeekFrom};
        let Some(f) = self.file(rel) else { return Ok(None) };
        let mut h = std::fs::File::open(&f.path).map_err(|e| e.to_string())?;
        h.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
        let want = limit.min(f.size.saturating_sub(offset)) as usize;
        let mut b = vec![0u8; want]; let mut got = 0; while got < want { let n = h.read(&mut b[got..]).map_err(|e| e.to_string())?; if n == 0 { break } got += n }
        b.truncate(got);
        let mut n = got;
        loop {
            match std::str::from_utf8(&b[..n]) {
                Ok(s) => { let s = s.to_string(); let next = if offset + (n as u64) < f.size { Some(offset + n as u64) } else { None }; return Ok(Some((f, s, n as u64, next))) }
                Err(_) => { if offset + got as u64 == f.size || got - n >= 3 { return Err("This file is not valid UTF-8; open the original file to inspect it.".into()) } n -= 1 }
            }
        }
    }
    fn digest(&self, rel: &str) -> String { match self.file(rel) { Some(f) => std::fs::read(&f.path).map(|b| hash(&b)).unwrap_or_else(|_| hash(b"")), None => hash(b"") } }
}

/// bp_project_metadata: the approved top-level scalars of a JSON file, and the scalar entries of the named maps.
fn metadata(path: &Path, keys: &[&str], maps: &[&str]) -> Result<Map<String, Value>, String> {
    if std::fs::metadata(path).map_or(0, |m| m.len()) > 8 << 20 { return Err("Incomplete metadata".into()) }
    let v: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?).map_err(|_| "Malformed metadata".to_string())?;
    let Value::Object(o) = v else { return Err("Incomplete metadata".into()) };
    let mut out = Map::new();
    for (k, x) in o {
        let scalar = |x: &Value| match x { Value::String(s) => s.chars().count() <= 8192, Value::Number(_) | Value::Bool(_) | Value::Null => true, _ => false };
        if keys.contains(&k.as_str()) && scalar(&x) { out.insert(k, x); }
        else if maps.contains(&k.as_str()) { if let Value::Object(m) = x { out.insert(k, Value::Object(m.into_iter().filter(|(_, y)| scalar(y)).take(128).collect())); } }
    }
    Ok(out)
}

/// bp_project_report: a saved DRC/ERC report's source, date and counts.
fn report(path: &Path) -> Option<Value> {
    if std::fs::metadata(path).ok()?.len() > 16 << 20 { return None }
    let v: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    fn walk(v: &Value, key: &str, r: &mut (u64, u64, u64)) {
        match v {
            Value::Array(a) => for x in a {
                if let Value::Object(o) = x {
                    if key == "unconnected_items" { r.2 += 1 }
                    if key == "violations" { match o.get("severity").and_then(Value::as_str) { Some("error") => r.0 += 1, Some("warning") => r.1 += 1, _ => {} } }
                }
                walk(x, key, r)
            },
            Value::Object(o) => for (k, x) in o { walk(x, k, r) },
            _ => {}
        }
    }
    let mut r = (0, 0, 0); walk(&v, "", &mut r);
    let source = v.get("source").and_then(Value::as_str).unwrap_or("").to_string();
    if source.is_empty() { return None }
    Some(json!({"source": source, "date": v.get("date").and_then(Value::as_str).unwrap_or(""), "errors": r.0, "warnings": r.1, "unconnected": r.2}))
}

fn fab_row(h: &Home, art: &mut Vec<Value>, issues: &mut Vec<String>, rel: &str) {
    let Some(raw) = h.file(rel) else { return };
    match metadata(&raw.path, &["name", "source", "kind", "qty", "order", "engineering_status"], &["options", "features"]) {
        Ok(v) => {
            let src = v.get("source").and_then(Value::as_str).map(|s| raw.path.parent().unwrap_or(Path::new("/")).join(s));
            if let Some(sf) = src.and_then(|s| h.file(&s.to_string_lossy())) { let _ = sf;
                let summary = json!({"name": v.get("name"), "source": v.get("source"), "kind": v.get("kind"), "order": v.get("order") != Some(&Value::Bool(false)), "qty": v.get("qty"), "options": v.get("options"), "features": v.get("features"), "engineering_status": v.get("engineering_status")});
                art.push(json!({"kind": "fab", "path": raw.path.to_string_lossy(), "mtime": raw.mtime, "size": raw.size, "title": "Fab readiness", "summary": summary.to_string()}));
            }
        }
        Err(_) => issues.push("Fab metadata is invalid.".into()),
    }
}

/// bp_project_home, blocking part: everything but the git and gh calls.
#[cfg(test)] pub fn read_files_pub(root: &str, req: &Value) -> (Value, bool, PathBuf) { read_files(root, req) }
fn read_files(root: &str, req: &Value) -> (Value, bool, PathBuf) {
    let real = match std::fs::canonicalize(root) { Ok(r) => r, Err(_) => return (json!({"project_home": req.get("project"), "root": root, "artifacts": [], "error": "Project folder is unavailable."}), false, PathBuf::new()) };
    let h = Home { real: real.clone() };
    let project = req.get("project").cloned().unwrap_or(Value::Null);
    let (mut art, mut issues): (Vec<Value>, Vec<String>) = (vec![], vec![]);
    let op = st(req, "op");
    if op == "intent" {
        // .backplane/intent.md lies in the project folder, outside a copy's home
        if crate::copy::on() { return (json!({"error": crate::copy::HOME_ONLY, "project_home": project}), false, real) }
        let dest = real.join(".backplane/intent.md"); let parent = dest.parent().unwrap().to_path_buf();
        let fail = |m: &str| (json!({"error": m, "project_home": project}), false, real.clone());
        if std::fs::create_dir_all(&parent).is_err() { return fail("Intent directory leaves this project.") }
        match std::fs::canonicalize(&parent) { Ok(p) if p.starts_with(&real) => {}, _ => return fail("Intent directory leaves this project.") }
        if h.digest(".backplane/intent.md") != st(req, "revision") { return fail("Goal and notes changed. Reload before saving; your edit is retained.") }
        let prior = h.file(".backplane/intent.md"); let size = prior.as_ref().map_or(0, |f| f.size);
        let (offset, remove) = (num(req, "editor"), num(req, "removeBytes"));
        if offset > size || offset + remove > size { return fail("This edit no longer matches the saved section.") }
        let old = prior.as_ref().and_then(|f| std::fs::read(&f.path).ok()).unwrap_or_default();
        let mut new = old[..offset as usize].to_vec(); new.extend_from_slice(st(req, "text").as_bytes()); new.extend_from_slice(&old[(offset + remove) as usize..]);
        let tmp = parent.join(format!(".intent-{:x}.tmp", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
        let wrote = (|| -> std::io::Result<()> { use std::io::Write; use std::os::unix::fs::OpenOptionsExt; let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?; f.write_all(&new)?; f.sync_all()?; Ok(()) })();
        if wrote.is_err() || h.digest(".backplane/intent.md") != st(req, "revision") { let _ = std::fs::remove_file(&tmp); return fail("Goal and notes changed while saving. Your edit is retained.") }
        if std::fs::rename(&tmp, &dest).is_err() { let _ = std::fs::remove_file(&tmp); return fail("Goal and notes changed while saving. Your edit is retained.") }
    }
    if op == "document" {
        let r = h.text(&st(req, "path"), 8192, num(req, "offset"));
        // a file that is not UTF-8 fails the whole request, as the JS effect's throw does
        let doc = match r { Ok(Some((f, t, n, next))) => json!({"path": f.path.to_string_lossy(), "mtime": f.mtime, "size": f.size, "text": t, "bytes": n, "next": next}), Ok(None) => json!({"error": "File is unavailable."}), Err(m) => return (json!({"project_home": project, "error": m}), false, real) };
        return (json!({"project_home": project, "document": doc}), false, real);
    }
    let mut config = Map::new(); let mut invalid = false;
    if let Some(c) = h.file(".backplane.json") { match metadata(&c.path, &["project", "pcb", "schematic", "drc", "erc"], &[]) { Ok(m) => config = m, Err(_) => { invalid = true; issues.push("Project configuration could not be read.".into()) } } }
    let cfg = |k: &str| config.get(k).and_then(Value::as_str).map(str::to_string);
    let mut stem: Option<String> = cfg("project").map(|p| p.strip_suffix(".kicad_pro").unwrap_or(&p).to_string());
    let skip = [".git", ".backplane", "node_modules", "build", "dist", "mfg", "extra", "target", ".easyeda_cache"];
    let (mut scanned, mut scan_next): (u64, Option<u64>) = (0, None);
    if !invalid && stem.is_none() && cfg("pcb").is_none() && cfg("schematic").is_none() {
        let start = num(req, "scan"); let mut ordinal = 0u64;
        fn scan(dir: &Path, depth: u32, skip: &[&str], start: u64, ord: &mut u64, scanned: &mut u64, next: &mut Option<u64>, stem: &mut Option<String>) {
            if depth > 4 || *scanned >= 256 { return }
            let Ok(it) = std::fs::read_dir(dir) else { return };
            for e in it.flatten() {
                let name = e.file_name().to_string_lossy().into_owned(); let Ok(ft) = e.file_type() else { continue };
                if ft.is_symlink() || skip.contains(&name.as_str()) { continue }
                let p = e.path();
                if ft.is_dir() { scan(&p, depth + 1, skip, start, ord, scanned, next, stem) } else {
                    *ord += 1; if *ord <= start { continue }
                    *scanned += 1;
                    if name.ends_with(".kicad_pro") && stem.is_none() { let s = p.to_string_lossy(); *stem = Some(s[..s.len() - 10].to_string()) }
                    if *scanned >= 256 { *next = Some(*ord); break }
                }
                if stem.is_some() || *scanned >= 256 { break }
            }
        }
        scan(&real, 0, &skip, start, &mut ordinal, &mut scanned, &mut scan_next, &mut stem);
    }
    let add = |art: &mut Vec<Value>, kind: &str, rel: &str, title: &str, summary: &str| -> Option<(PathBuf, f64)> {
        let f = h.file(rel)?;
        let mut row = json!({"kind": kind, "path": f.path.to_string_lossy(), "mtime": f.mtime, "size": f.size, "title": title, "summary": summary});
        if kind == "mech" { let d = f.path.parent().unwrap_or(Path::new("/")).join("renders").join(format!("{}.iso.png", f.path.file_stem().unwrap_or_default().to_string_lossy())); if let Some(t) = h.file(&d.to_string_lossy()) { row["thumbnail"] = json!(t.path.to_string_lossy()) } }
        art.push(row); Some((f.path, f.mtime))
    };
    let pcb = add(&mut art, "board", &cfg("pcb").unwrap_or_else(|| stem.as_ref().map(|s| format!("{s}.kicad_pcb")).unwrap_or_default()), "Board", "");
    let sch = add(&mut art, "schematic", &cfg("schematic").unwrap_or_else(|| stem.as_ref().map(|s| format!("{s}.kicad_sch")).unwrap_or_default()), "Schematic", "");
    let editor = num(req, "editor");
    let fails = |m: String| (json!({"project_home": project, "error": m}), false, real.clone());
    let saved = match h.text(".backplane/intent.md", 8192, editor) { Ok(s) => s, Err(m) => return fails(m) };
    let mut intent_rev = hash(b"");
    if let Some((_, t, _, _)) = &saved { add(&mut art, "intent", ".backplane/intent.md", "Goal & notes", t); intent_rev = h.digest(".backplane/intent.md") }
    match h.text("docs/SPEC.md", 4096, 0) { Ok(Some((_, t, _, _))) => { add(&mut art, "spec", "docs/SPEC.md", "Specification", &t); } Err(m) => return fails(m), _ => {} }
    match h.text("README.md", 4096, 0) { Ok(Some((_, t, _, _))) => { add(&mut art, "readme", "README.md", "README", &t); } Err(m) => return fails(m), _ => {} }
    if let Some((p, _)) = &pcb { let s = p.to_string_lossy(); if let Some(b) = s.strip_suffix(".kicad_pcb") { fab_row(&h, &mut art, &mut issues, &format!("{b}.fab.json")) } }
    let (mut next, mut ordinal, after, mut seen) = (None::<u64>, 0u64, num(req, "after"), 0u64);
    if let Ok(it) = std::fs::read_dir(real.join("mech")) {
        for e in it.flatten() {
            ordinal += 1; if ordinal <= after { continue }
            seen += 1;
            let name = e.file_name().to_string_lossy().into_owned(); let lower = name.to_lowercase();
            if e.file_type().map_or(false, |t| t.is_file()) && (lower.ends_with(".step") || lower.ends_with(".stp") || lower.ends_with(".glb")) {
                if let Some((p, _)) = add(&mut art, "mech", &format!("mech/{name}"), &name, "") { let s = p.to_string_lossy().into_owned(); if let Some(i) = s.rfind('.') { fab_row(&h, &mut art, &mut issues, &format!("{}.fab.json", &s[..i])) } }
            }
            if art.len() >= 26 || seen >= 256 { next = Some(ordinal); break }
        }
    }
    if pcb.is_some() || sch.is_some() {
        let mut cands: [Vec<F>; 2] = [vec![], vec![]];
        let cand = |rel: &str, k: usize, c: &mut [Vec<F>; 2]| { if let Some(f) = h.file(rel) { c[k].push(f); c[k].sort_by(|a, b| b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal)); c[k].truncate(4) } };
        if let Ok(it) = std::fs::read_dir(real.join("build/reports")) { for e in it.flatten() {
            let n = e.file_name().to_string_lossy().into_owned(); let l = n.to_lowercase();
            if !e.file_type().map_or(false, |t| t.is_file()) || !l.ends_with(".json") || !(l.contains("drc") || l.contains("erc")) { continue }
            cand(&format!("build/reports/{n}"), if l.contains("erc") { 1 } else { 0 }, &mut cands);
        } }
        for (k, kind) in ["drc", "erc"].iter().enumerate() {
            if let Some(c) = cfg(kind) { cand(&c, k, &mut cands) }
            for raw in &cands[k] {
                let Some(r) = report(&raw.path) else { continue };
                let base = r["source"].as_str().and_then(|s| Path::new(s).file_name()).map(|b| b.to_os_string());
                if ![pcb.as_ref(), sch.as_ref()].into_iter().flatten().any(|(p, _)| p.file_name().map(|b| b.to_os_string()) == base) { continue }
                let newest = pcb.as_ref().map_or(0.0, |x| x.1).max(sch.as_ref().map_or(0.0, |x| x.1));
                let mut s = r.clone(); s["stale"] = json!(raw.mtime < newest);
                art.push(json!({"kind": kind, "path": raw.path.to_string_lossy(), "mtime": raw.mtime, "size": raw.size, "title": kind.to_uppercase(), "summary": s.to_string()}));
                break;
            }
        }
    }
    for name in ["mfg/bom.csv", "mfg/BOM.csv", "bom.csv", "BOM.csv"] { match h.text(name, 4096, 0) { Ok(Some((_, t, _, _))) => { add(&mut art, "bom", name, "BOM", &t); break } Err(m) => return fails(m), _ => {} } }
    let git = real.join(".git").exists();
    let revision = hash(serde_json::to_string(&art.iter().map(|a| json!([a["path"], a["mtime"], a["size"]])).collect::<Vec<_>>()).unwrap_or_default().as_bytes());
    let _ = (&mut issues, scanned);
    let snapshot = json!({"project_home": project, "root": real.to_string_lossy(), "artifacts": art, "intentRevision": intent_rev,
        "intentComplete": saved.as_ref().map_or(true, |s| s.3.is_none()), "revision": revision, "next": next, "after": after, "scanNext": scan_next, "issues": issues,
        "intentSaved": op == "intent", "intentOffset": editor, "intentNext": saved.as_ref().and_then(|s| s.3), "intentBytes": saved.as_ref().map_or(0, |s| s.2),
        "intent": saved.map_or("# Goal\n\n# Parts\n\n# Notes\n".to_string(), |s| s.1)});
    (snapshot, git, real)
}

/// The request's answer: the snapshot (with the git rows: recent changes, open pull requests), or the document window.
pub async fn read(root: &str, req: &Value) -> Value {
    let (r, rq) = (root.to_string(), req.clone());
    let Ok((mut v, git_dir, real)) = tokio::task::spawn_blocking(move || read_files(&r, &rq)).await else { return json!({"error": "the project home could not be read"}) };
    if !git_dir || v.get("artifacts").is_none() || st(req, "op") == "document" { return v }
    let real_s = real.to_string_lossy().into_owned();
    let run = |args: Vec<&'static str>| { let c = real_s.clone(); async move {
        let a: Vec<&str> = args.clone();
        match tokio::time::timeout(std::time::Duration::from_millis(1500), git::read(a[0], &a[1..], &c, &[("GIT_TERMINAL_PROMPT", "0"), ("GIT_OPTIONAL_LOCKS", "0")])).await { Ok(r) if r.code == 0 => Some(r.out), _ => None } } };
    let now = ms(std::time::SystemTime::now());
    let mut extra: Vec<Value> = vec![];
    if let Some(changes) = run(vec!["git", "-c", "core.fsmonitor=false", "log", "-6", "--format=%h %s"]).await {
        extra.push(json!({"kind": "changes", "path": format!("{real_s}/.git"), "title": "Recent changes", "summary": changes.trim(), "mtime": now, "size": 0}));
    }
    if let Some(remote) = run(vec!["git", "-c", "core.fsmonitor=false", "config", "--get", "remote.origin.url"]).await {
        let r = remote.trim(); let tail = r.strip_prefix("https://github.com/").or_else(|| r.strip_prefix("git@github.com:")).map(|t| t.strip_suffix(".git").unwrap_or(t));
        if let Some(repo) = tail.filter(|t| t.split('/').count() == 2 && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '/'))) {
            let c = real_s.clone(); let repo = repo.to_string();
            let raw = match tokio::time::timeout(std::time::Duration::from_millis(1500), git::read("gh", &["pr", "list", "--repo", &repo, "--state", "open", "--limit", "12", "--json", "number,title,url"], &c, &[("GIT_TERMINAL_PROMPT", "0")])).await { Ok(r) if r.code == 0 => Some(r.out), _ => None };
            let mut summary = "Pull requests could not be refreshed. Open the repository to inspect them.".to_string();
            if let Some(Ok(Value::Array(rows))) = raw.map(|r| serde_json::from_str::<Value>(&r)) {
                summary = if rows.is_empty() { "No open pull requests in the saved snapshot.".into() } else { rows.iter().map(|r| format!("[{} (#{})]({})", st(r, "title").replace(['[', ']', '\n'], " "), r["number"], st(r, "url"))).collect::<Vec<_>>().join("\n\n") };
            }
            extra.push(json!({"kind": "prs", "path": format!("{real_s}/.git"), "title": "Open pull requests", "summary": format!("{summary}\n\n[Open repository](https://github.com/{repo})"), "mtime": now, "size": 0}));
        }
    }
    if let Some(Value::Array(a)) = v.get_mut("artifacts") { a.extend(extra); a.truncate(32); }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    fn proj(name: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-home-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
    #[test] fn a_kicad_project_lists_its_board_schematic_goal_and_checks() {
        let d = proj("snap"); std::fs::create_dir_all(d.join(".backplane")).unwrap(); std::fs::create_dir_all(d.join("build/reports")).unwrap();
        std::fs::write(d.join("a.kicad_pro"), "{}").unwrap(); std::fs::write(d.join("a.kicad_pcb"), "(kicad_pcb)").unwrap(); std::fs::write(d.join("a.kicad_sch"), "(kicad_sch)").unwrap();
        std::fs::write(d.join(".backplane/intent.md"), "# Goal\n\nA board\n").unwrap(); std::fs::write(d.join("README.md"), "readme").unwrap();
        std::fs::write(d.join("build/reports/drc.json"), r#"{"source":"a.kicad_pcb","date":"d","violations":[{"severity":"error"},{"severity":"warning"},{"severity":"error"}],"unconnected_items":[{},{}]}"#).unwrap();
        let (v, git, _) = read_files(d.to_str().unwrap(), &json!({"project":"p1"}));
        let kinds: Vec<&str> = v["artifacts"].as_array().unwrap().iter().map(|a| a["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["board", "schematic", "intent", "readme", "drc"]); assert!(!git);
        assert_eq!(v["intent"], "# Goal\n\nA board\n"); assert_eq!(v["intentComplete"], true); assert_eq!(v["project_home"], "p1");
        let s: Value = serde_json::from_str(v["artifacts"][4]["summary"].as_str().unwrap()).unwrap(); assert_eq!((s["errors"].clone(), s["warnings"].clone(), s["unconnected"].clone()), (json!(2), json!(1), json!(2)));
        assert_eq!(v["intentRevision"], hash(&std::fs::read(d.join(".backplane/intent.md")).unwrap()));
    }
    #[test] fn the_goal_is_written_only_against_the_revision_the_page_holds() {
        let d = proj("intent"); std::fs::create_dir_all(d.join(".backplane")).unwrap(); std::fs::write(d.join(".backplane/intent.md"), "# Goal\n\nold\n").unwrap();
        let rev = hash(b"# Goal\n\nold\n");
        let (v, _, _) = read_files(d.to_str().unwrap(), &json!({"project":"p","op":"intent","revision":"stale","editor":0,"removeBytes":0,"text":"x"}));
        assert_eq!(v["error"], "Goal and notes changed. Reload before saving; your edit is retained."); assert_eq!(std::fs::read_to_string(d.join(".backplane/intent.md")).unwrap(), "# Goal\n\nold\n");
        let (v, _, _) = read_files(d.to_str().unwrap(), &json!({"project":"p","op":"intent","revision":rev,"editor":8,"removeBytes":3,"text":"new"}));
        assert_eq!(std::fs::read_to_string(d.join(".backplane/intent.md")).unwrap(), "# Goal\n\nnew\n"); assert_eq!(v["intentSaved"], true);
        let (v, _, _) = read_files(d.to_str().unwrap(), &json!({"project":"p","op":"intent","revision":hash(b"# Goal\n\nnew\n"),"editor":999,"removeBytes":0,"text":"x"})); assert_eq!(v["error"], "This edit no longer matches the saved section.");
    }
    #[test] fn a_document_window_stays_inside_the_project_and_is_utf8() {
        let d = proj("doc"); std::fs::write(d.join("README.md"), "héllo").unwrap(); std::fs::write(d.join("bad.bin"), [0xff, 0xfe, 0xfd, 0xfc, 0xfb]).unwrap();
        let (v, _, _) = read_files(d.to_str().unwrap(), &json!({"project":"p","op":"document","path":"README.md"})); assert_eq!(v["document"]["text"], "héllo"); assert_eq!(v["document"]["next"], Value::Null);
        let (v, _, _) = read_files(d.to_str().unwrap(), &json!({"project":"p","op":"document","path":"../x"})); assert_eq!(v["document"]["error"], "File is unavailable.");
        let (v, _, _) = read_files(d.to_str().unwrap(), &json!({"project":"p","op":"document","path":"bad.bin"})); assert_eq!(v["error"], "This file is not valid UTF-8; open the original file to inspect it.");
        let (v, _, _) = read_files("/definitely/not/here", &json!({"project":"p"})); assert_eq!(v["error"], "Project folder is unavailable.");
    }
}
