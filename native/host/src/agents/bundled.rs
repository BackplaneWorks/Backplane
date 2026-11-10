//! Backplane's own skills (deploy/skills), compiled into the hub and written into each hub home at start, as
//! `<home>/bundled`: a Claude plugin (`.claude-plugin/plugin.json`, `skills/<name>/...`) that Claude loads with
//! `--plugin-dir` like KiStack, that the `$` popup lists under Backplane, and that every provider's prompt names
//! (`prompt::bundled_skills`), so Codex and the rest read its SKILL.md too. Nothing is fetched: a skill that needs
//! live data fetches it itself when it runs (sendcutsend's scripts/scs.py). A file of an older build that this one
//! no longer ships is removed; files are rewritten only when they differ.
use std::path::{Path, PathBuf};

const MANIFEST: &str = "{\"name\":\"backplane\",\"description\":\"Backplane's own skills\"}\n";

/// (path under `<home>/bundled`, contents)
pub const FILES: &[(&str, &str)] = &[
    ("skills/sendcutsend/SKILL.md", include_str!("../../../../deploy/skills/sendcutsend/SKILL.md")),
    ("skills/sendcutsend/scripts/scs.py", include_str!("../../../../deploy/skills/sendcutsend/scripts/scs.py")),
    ("skills/sendcutsend/scripts/scs_step.py", include_str!("../../../../deploy/skills/sendcutsend/scripts/scs_step.py")),
    ("skills/sendcutsend/scripts/scs_sheet.py", include_str!("../../../../deploy/skills/sendcutsend/scripts/scs_sheet.py")),
    ("skills/sendcutsend/examples/bracket.py", include_str!("../../../../deploy/skills/sendcutsend/examples/bracket.py")),
];

pub fn dir(home: &Path) -> PathBuf { home.join("bundled") }

fn put(p: &Path, text: &str) -> std::io::Result<()> {
    if std::fs::read(p).ok().as_deref() == Some(text.as_bytes()) { return Ok(()) }
    if let Some(d) = p.parent() { std::fs::create_dir_all(d)? }
    let tmp = p.with_extension("bp-new"); std::fs::write(&tmp, text)?; std::fs::rename(&tmp, p)
}

fn sweep(root: &Path, at: &Path, keep: &[PathBuf]) {
    let Ok(rd) = std::fs::read_dir(at) else { return };
    for e in rd.flatten() {
        let p = e.path(); let Ok(t) = e.file_type() else { continue };
        if t.is_dir() { sweep(root, &p, keep); let _ = std::fs::remove_dir(&p); } // only when empty
        else if !keep.contains(&p) && p.strip_prefix(root).is_ok() { let _ = std::fs::remove_file(&p); }
    }
}

/// Writes the skills into `<home>/bundled`; the folder, or None when it could not be written (the hub runs on without).
pub fn install(home: &Path) -> Option<PathBuf> {
    let d = dir(home);
    let mut keep = vec![d.join(".claude-plugin/plugin.json")];
    let r: std::io::Result<()> = (|| {
        put(&keep[0], MANIFEST)?;
        for (rel, text) in FILES { let p = d.join(rel); put(&p, text)?; keep.push(p); }
        Ok(())
    })();
    if let Err(e) = r { eprintln!("bundled skills: {}: {e}", d.display()); return None }
    sweep(&d, &d.join("skills"), &keep);
    Some(d)
}

/// Each bundled skill: (name, description, SKILL.md path), for the prompt.
pub fn list(d: &Path) -> Vec<(String, String, String)> {
    FILES.iter().filter(|(rel, _)| rel.ends_with("/SKILL.md")).map(|(rel, text)| {
        let p = d.join(rel); let folder = rel.trim_end_matches("/SKILL.md").rsplit('/').next().unwrap_or("");
        let s = crate::agents::inputs::parse_skill(text, folder, &p.to_string_lossy()); (s.name, s.description, s.path)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-bundled-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }

    #[test] fn the_skills_are_written_into_the_home_as_a_plugin_and_old_files_go() {
        let h = tmp("install");
        let stale = h.join("bundled/skills/gone/SKILL.md"); std::fs::create_dir_all(stale.parent().unwrap()).unwrap(); std::fs::write(&stale, "old").unwrap();
        let d = install(&h).unwrap();
        assert_eq!(d, h.join("bundled"));
        assert!(std::fs::read_to_string(d.join(".claude-plugin/plugin.json")).unwrap().contains("\"backplane\""));
        let md = std::fs::read_to_string(d.join("skills/sendcutsend/SKILL.md")).unwrap();
        assert!(md.starts_with("---\nname: sendcutsend\n"));
        assert!(d.join("skills/sendcutsend/scripts/scs.py").is_file() && d.join("skills/sendcutsend/scripts/scs_step.py").is_file());
        assert!(!stale.exists() && !h.join("bundled/skills/gone").exists(), "a skill an older build shipped is removed");
        let t0 = std::fs::metadata(d.join("skills/sendcutsend/SKILL.md")).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20)); install(&h).unwrap();
        assert_eq!(std::fs::metadata(d.join("skills/sendcutsend/SKILL.md")).unwrap().modified().unwrap(), t0, "an unchanged file is not rewritten");
        let l = list(&d); assert_eq!(l.len(), 1); assert_eq!(l[0].0, "sendcutsend"); assert!(l[0].1.contains("SendCutSend")); assert!(l[0].2.replace('\\', "/").ends_with("bundled/skills/sendcutsend/SKILL.md"));
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test] fn the_popup_and_the_turn_find_the_skill_under_backplane() {
        let h = tmp("scan"); let home = h.join("person"); std::fs::create_dir_all(&home).unwrap();
        install(&h.join("hub")).unwrap();
        let env = crate::agents::skills::Env { home: home.to_string_lossy().into(), hub_home: h.join("hub"), kistack: None, codex_home: None };
        let ss = crate::agents::skills::scan_all(&env, &h.to_string_lossy());
        let f = ss.iter().find(|f| f.name == "sendcutsend").expect("listed");
        assert_eq!((f.group.as_str(), f.kind), ("Backplane", crate::agents::skills::Kind::Hub));
        assert!(crate::agents::skills::prepare("$sendcutsend check the bracket", &ss, "codex", false).replace('\\', "/").contains("bundled/skills/sendcutsend/SKILL.md"), "Codex is told the file");
        assert!(crate::agents::skills::prepare("$sendcutsend check the bracket", &ss, "claude", false).starts_with("/sendcutsend"), "Claude loads it itself");
        let _ = std::fs::remove_dir_all(&h);
    }

    /// The skill's own tests (test/skills/sendcutsend: the helper on recorded copies of SendCutSend's files, no network;
    /// the DFM rules on the sample bracket and MDF panel), so the gate runs them. Skipped where there is no python3.
    #[test] fn the_sendcutsend_helper_passes_its_tests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."); let dir = root.join("test/skills/sendcutsend");
        let py = ["python3", "/usr/bin/python3"].into_iter().find(|p| std::process::Command::new(p).arg("-V").output().map_or(false, |o| o.status.success()));
        let Some(py) = py else { eprintln!("skip: no python3"); return };
        let tmp = tmp("py");
        let o = std::process::Command::new(py).args(["-I", "-W", "ignore", "-m", "unittest", "discover", "-s"]).arg(&dir).current_dir(&tmp)
            .env("TMPDIR", &tmp).env("SCS_CACHE", tmp.join("never-used")).env_remove("SCS_LIVE").env_remove("SCS_OFFLINE").output().unwrap();
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(o.status.success(), "the skill's tests failed:\n{err}");
        assert!(err.contains("OK"), "{err}");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
