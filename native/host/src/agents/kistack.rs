//! KiStack's KiCad skills, pinned by deploy/kistack.lock and compiled into the hub (build.rs), so every build on every
//! platform has them whatever ships beside it. At start the hub writes them into its home as `<home>/kistack`: Backplane's
//! own Claude plugin manifest (named `kistack`), REVISION, LICENSE and `skills/<name>/...`, the shape `scripts/kistack.py
//! check` accepts. A file is rewritten only when it differs and a file this build does not ship is removed, so a stale or
//! edited copy is put back to the pin. Claude loads it with `--plugin-dir` (`kistack:<name>`), Pi with `--skill`, and
//! every provider's prompt names each SKILL.md (`prompt::kistack_skills`) and says to use these copies, never an older
//! personal one. `BACKPLANE_KISTACK` (a folder of the same shape) replaces it for a developer trying another KiStack.
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/kistack_files.rs"));

/// Backplane's own manifest (scripts/kistack.py MANIFEST): never the repository's, whose hooks would run in every Claude.
pub const MANIFEST: &str = "{\"name\":\"kistack\",\"description\":\"KiStack KiCad skills by American Embedded\"}\n";

pub fn dir(home: &Path) -> PathBuf { home.join("kistack") }

/// What the prompt says of the copy the agents get.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Copy { pub dir: String, pub rev: String, pub pinned: bool, pub skills: Vec<(String, String, String)> }

/// Writes the pinned copy into `<home>/kistack`; the folder, or None when it could not be written.
pub fn install(home: &Path) -> Option<PathBuf> {
    let d = dir(home);
    let mut keep = vec![d.join(".claude-plugin/plugin.json"), d.join("REVISION")];
    let r: std::io::Result<()> = (|| {
        super::bundled::put(&keep[0], MANIFEST.as_bytes())?; super::bundled::put(&keep[1], format!("{REV}\n").as_bytes())?;
        for (rel, bytes, x) in FILES {
            let p = d.join(rel); super::bundled::put(&p, bytes)?;
            #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let m = if *x { 0o755 } else { 0o644 }; if std::fs::metadata(&p)?.permissions().mode() & 0o777 != m { std::fs::set_permissions(&p, std::fs::Permissions::from_mode(m))? } }
            #[cfg(not(unix))] let _ = x;
            keep.push(p);
        }
        Ok(())
    })();
    if let Err(e) = r { eprintln!("kistack: {}: {e}", d.display()); return None }
    super::bundled::sweep(&d, &d.join("skills"), &keep); super::bundled::sweep(&d, &d.join(".claude-plugin"), &keep);
    Some(d)
}

/// The copy agents get: `BACKPLANE_KISTACK` when it names a folder, else the pinned one written into the home.
pub fn resolve(home: &Path) -> Option<PathBuf> {
    std::env::var("BACKPLANE_KISTACK").ok().filter(|s| !s.is_empty()).map(PathBuf::from).filter(|p| p.join("skills").is_dir()).or_else(|| install(home))
}

/// Each skill of the copy at `d`, sorted by name, for the prompt.
pub fn describe(d: &Path) -> Copy {
    let rev = std::fs::read_to_string(d.join("REVISION")).map(|s| s.trim().to_string()).unwrap_or_default();
    let mut skills: Vec<(String, String, String)> = std::fs::read_dir(d.join("skills")).into_iter().flatten().flatten().filter_map(|e| {
        let p = e.path().join("SKILL.md"); let t = std::fs::read_to_string(&p).ok()?;
        let s = crate::agents::inputs::parse_skill(&t, &e.file_name().to_string_lossy(), &p.to_string_lossy()); Some((s.name, s.description, s.path))
    }).collect();
    skills.sort();
    Copy { dir: d.to_string_lossy().into_owned(), pinned: rev == REV, rev, skills }
}

/// The lock's tree hash (scripts/kistack.py `tree`) of the files compiled in.
pub fn tree_of(files: &[(&str, &[u8], bool)]) -> String {
    use sha2::{Digest, Sha256};
    let mut fs: Vec<&(&str, &[u8], bool)> = files.iter().filter(|(r, _, _)| r.starts_with("skills/")).collect(); fs.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let t: String = fs.iter().map(|(r, b, _)| format!("{:x}  {r}\n", Sha256::digest(b))).collect();
    format!("{:x}", Sha256::digest(t.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("kistack-{n}")) }

    #[test] fn the_compiled_copy_is_the_pinned_tree() {
        assert_eq!(tree_of(FILES), TREE, "deploy/kistack is not the tree deploy/kistack.lock pins (scripts/kistack.py verify)");
        assert!(FILES.iter().any(|(r, _, _)| *r == "skills/schematic/SKILL.md") && FILES.iter().any(|(r, _, _)| *r == "LICENSE"));
    }

    #[test] fn the_home_gets_the_pinned_copy_and_a_stale_one_is_put_back() {
        let h = tmp("install");
        let stale = h.join("kistack/skills/old-skill/SKILL.md"); std::fs::create_dir_all(stale.parent().unwrap()).unwrap(); std::fs::write(&stale, "old").unwrap();
        let edited = h.join("kistack/skills/schematic/SKILL.md"); std::fs::create_dir_all(edited.parent().unwrap()).unwrap(); std::fs::write(&edited, "edited").unwrap();
        let d = install(&h).unwrap(); assert_eq!(d, h.join("kistack"));
        assert!(!stale.exists() && !h.join("kistack/skills/old-skill").exists(), "a skill this build does not ship is removed");
        assert!(std::fs::read_to_string(&edited).unwrap().starts_with("---"), "an edited file is the pinned one again");
        assert_eq!(std::fs::read_to_string(d.join("REVISION")).unwrap().trim(), REV);
        assert_eq!(std::fs::read_to_string(d.join(".claude-plugin/plugin.json")).unwrap(), MANIFEST);
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; assert!(std::fs::metadata(d.join("skills/layout-placement/scripts/run.sh")).unwrap().permissions().mode() & 0o100 != 0, "scripts stay executable") }
        let c = describe(&d); assert!(c.pinned && c.rev == REV, "{c:?}");
        assert_eq!(c.skills.len(), FILES.iter().filter(|(r, _, _)| r.ends_with("/SKILL.md") && r.matches('/').count() == 2).count());
        let s = c.skills.iter().find(|s| s.0 == "kicad-schematic").expect("kicad-schematic"); assert!(!s.1.is_empty() && s.2.replace('\\', "/").ends_with("kistack/skills/schematic/SKILL.md"));
        // scripts/kistack.py's own check of a shipped copy: the pinned commit, the pinned tree, nothing else
        let py = ["python3", "/usr/bin/python3"].into_iter().find(|p| std::process::Command::new(p).arg("-V").output().map_or(false, |o| o.status.success()));
        if let Some(py) = py {
            let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/kistack.py");
            let o = std::process::Command::new(py).arg("-I").arg(&script).arg("check").arg(&d).output().unwrap();
            assert!(o.status.success(), "kistack.py check: {}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        }
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test] fn the_popup_lists_the_home_copy_under_backplane() {
        let h = tmp("scan"); let home = h.join("person"); std::fs::create_dir_all(&home).unwrap();
        let k = install(&h.join("hub")).unwrap();
        let env = crate::agents::skills::Env { home: home.to_string_lossy().into(), hub_home: h.join("hub"), kistack: Some(k), codex_home: None };
        // the person's older copy, as found on a real machine (~/.agents/skills)
        let stale = home.join(".agents/skills/schematic/SKILL.md"); std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, "---\nname: kicad-schematic\ndescription: an older copy\n---\nold\n").unwrap();
        let ss = crate::agents::skills::scan_all(&env, &home.to_string_lossy());
        let f = crate::agents::skills::lookup(&ss, "kicad-schematic").expect("listed");
        assert_eq!((f.group.as_str(), f.kind), ("Backplane", crate::agents::skills::Kind::Hub), "the pinned copy wins the name");
        assert_eq!(ss.iter().filter(|f| f.name == "kicad-schematic" && f.kind == crate::agents::skills::Kind::Hub).count(), 1, "one entry, not one per root");
        assert!(crate::agents::skills::prepare("$kicad-schematic draw it", &ss, "codex", false).replace('\\', "/").contains("hub/kistack/skills/schematic/SKILL.md"), "Codex is told the file");
        let _ = std::fs::remove_dir_all(&h);
    }
}
