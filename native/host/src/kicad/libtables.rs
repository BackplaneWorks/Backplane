//! The libraries a runtime registers by default reach a person who already has KiCad tables. A Backplane runtime's default global
//! tables (`<root>/share/kicad/fp-lib-table`, `sym-lib-table`) are copied only when the person has none (its launcher, at first launch);
//! a table the person already has keeps what is in it. So when the hub selects a runtime (`ipc::publish_cad`: at start, after an
//! install, when the KiCad path changes) it adds to the person's table, for that runtime's settings version, each entry of the runtime's
//! default table that the person's lacks by name: Backplane's own libraries arrive on update without a setup step.
//!
//! - An entry the person's table already names, whatever its path, is theirs and is left alone (no duplicate name, no rewrite).
//! - An entry the hub added once is never added again (`<hub home>/kicad-libtables.json`), so a library the person removed stays removed.
//! - KiCad's own nested table (`KiCad`) is left to KiCad: a person's table always has one, pointing where they chose.
//! - A table the person does not have yet is not made here: the runtime's launcher copies the whole default at first launch.
//! - The table is replaced whole (written beside it, then renamed) after a copy of it goes to `<hub home>/backups/kicad-libtables/`.
//! KiCad reads its global tables when it starts and writes them only when the person saves a library dialog, so a running KiCad sees
//! the new entries at its next start and does not undo them unless they save that dialog before restarting.
use super::sexpr;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The tables merged, with the s-expression head each must have.
pub const TABLES: &[(&str, &str)] = &[("fp-lib-table", "fp_lib_table"), ("sym-lib-table", "sym_lib_table")];

/// One `(lib ...)` entry: its name and its text as the default table writes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry { pub name: String, pub text: String }

/// The entries of a table's text with their names (None when it is not a whole table with that head).
pub fn entries(text: &str, head: &str) -> Option<Vec<Entry>> {
    let b = text.as_bytes();
    let first = b.iter().position(|c| *c == b'(')?;
    if !text[first + 1..].trim_start().starts_with(head) { return None }
    let forms = sexpr::forms(b)?;
    Some(forms.into_iter().filter_map(|f| {
        let x = sexpr::parse(f);
        (x.name() == "lib").then(|| Entry { name: x.attr("name").to_string(), text: String::from_utf8_lossy(f).into_owned() })
    }).filter(|e| !e.name.is_empty()).collect())
}

/// The table's text with `add` appended before its closing parenthesis, one tab-indented line each, as KiCad writes them.
pub fn appended(text: &str, add: &[Entry]) -> String {
    let end = text.trim_end().len().saturating_sub(1);
    let mut out = text[..end].trim_end_matches(['\n', '\r', ' ', '\t']).to_string();
    out.push('\n');
    for e in add { out.push('\t'); out.push_str(&e.text); out.push('\n') }
    out.push_str(")\n");
    out
}

/// What one table needs: the default's entries the person's lacks and the hub has not added before. None: nothing to do (no person's
/// table, a table that is not whole, or nothing new).
pub fn plan(default: &str, mine: &str, head: &str, offered: &[String]) -> Option<Vec<Entry>> {
    let want = entries(default, head)?;
    let have: Vec<String> = entries(mine, head)?.into_iter().map(|e| e.name).collect();
    let add: Vec<Entry> = want.into_iter()
        .filter(|e| e.name != "KiCad" && !have.contains(&e.name) && !offered.contains(&e.name)).collect();
    (!add.is_empty()).then_some(add)
}

/// KiCad's settings folder name for a runtime version: "10.99.0-bp.13" -> "10.99" (None for one that is not major.minor).
pub fn settings_version(version: &str) -> Option<String> {
    let mut it = version.split(['-', '+']).next()?.split('.');
    let (a, b) = (it.next()?, it.next()?);
    (!a.is_empty() && !b.is_empty() && a.chars().chain(b.chars()).all(|c| c.is_ascii_digit())).then(|| format!("{a}.{b}"))
}

/// The settings root the runtime's launcher writes: `KICAD_CONFIG_HOME`, else `$XDG_CONFIG_HOME/kicad`, else the system's (dirs.rs).
pub fn config_root() -> Option<PathBuf> { crate::plat::dirs::kicad_config_roots().into_iter().next().map(PathBuf::from) }

/// Merge every table of the runtime at `root` (version `version`) into the person's under `config_root`; `hub_home` keeps the record of
/// what was added and the backups. Returns one line per table changed ("fp-lib-table: Backplane_Logo, ..."); errors are said, not raised.
pub fn sync(root: &Path, version: &str, config_root: &Path, hub_home: &Path) -> Vec<String> {
    let Some(ver) = settings_version(version) else { return vec![] };
    let dir = config_root.join(&ver);
    let record_path = hub_home.join("kicad-libtables.json");
    let mut record: Value = std::fs::read_to_string(&record_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
    let mut said = vec![];
    for (table, head) in TABLES {
        let (default_path, mine_path) = (root.join("share/kicad").join(table), dir.join(table));
        let (Ok(default), Ok(mine)) = (std::fs::read_to_string(&default_path), std::fs::read_to_string(&mine_path)) else { continue };
        let key = mine_path.to_string_lossy().to_string();
        let offered: Vec<String> = record[&key].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
        let Some(add) = plan(&default, &mine, head, &offered) else { continue };
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let backup = hub_home.join("backups/kicad-libtables").join(format!("{stamp}-{ver}-{table}"));
        if let Err(e) = std::fs::create_dir_all(backup.parent().unwrap_or(hub_home)).and_then(|_| std::fs::write(&backup, &mine)) {
            said.push(format!("{table}: not changed, its backup failed: {e}")); continue
        }
        // read again just before replacing: a table the person saved meanwhile is left for the next time
        if std::fs::read_to_string(&mine_path).ok().as_deref() != Some(mine.as_str()) { continue }
        if let Err(e) = crate::ckpt::write_atomic(&mine_path, &[appended(&mine, &add).as_bytes()]) { said.push(format!("{table}: could not write {}: {e}", mine_path.display())); continue }
        let names: Vec<String> = add.iter().map(|e| e.name.clone()).collect();
        let mut all = offered; all.extend(names.iter().cloned());
        record[&key] = json!(all);
        said.push(format!("{table} {ver}: added {} (backup {})", names.join(", "), backup.display()));
    }
    if !said.is_empty() { let _ = crate::ckpt::write_atomic(&record_path, &[serde_json::to_string_pretty(&record).unwrap_or_default().as_bytes()]); }
    said
}

#[cfg(test)]
mod tests {
    use super::*;
    const DEFAULT: &str = "(fp_lib_table\n  (version 7)\n  (lib (name \"KiCad\")(type \"Table\")(uri \"${KICAD10_TEMPLATE_DIR}/fp-lib-table\")(options \"\")(descr \"Official\"))\n  (lib (name \"Backplane_Logo\")(type \"KiCad\")(uri \"${BACKPLANE_KICAD_LIBRARY}/libraries/Backplane_Logo.pretty\")(options \"\")(descr \"Backplane\"))\n  (lib (name \"Backplane_Shoggoth\")(type \"KiCad\")(uri \"${BACKPLANE_KICAD_LIBRARY}/libraries/Backplane_Shoggoth.pretty\")(options \"\")(descr \"Backplane\"))\n)\n";
    const MINE: &str = "(fp_lib_table\n\t(version 7)\n\t(lib (name \"KiCad\") (type \"Table\") (uri \"/usr/local/share/kicad/template/fp-lib-table\") (options \"\") (descr \"KiCad Default Libraries\"))\n\t(lib (name \"amemb\") (type \"Table\") (uri \"/home/h/lib/fp-lib-table\") (options \"\") (descr \"\"))\n\t(lib (name \"Backplane_Shoggoth\") (type \"KiCad\") (uri \"/mine/Shoggoth.pretty\") (options \"\") (descr \"\"))\n)\n";

    #[test] fn adds_only_what_the_person_lacks_and_keeps_their_own() {
        let add = plan(DEFAULT, MINE, "fp_lib_table", &[]).unwrap();
        assert_eq!(add.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Backplane_Logo"], "KiCad and their own Shoggoth stay theirs");
        let out = appended(MINE, &add);
        assert!(out.starts_with(&MINE[..MINE.len() - 2]), "every line they had is kept as it was: {out}");
        assert!(out.ends_with("\t(lib (name \"Backplane_Logo\")(type \"KiCad\")(uri \"${BACKPLANE_KICAD_LIBRARY}/libraries/Backplane_Logo.pretty\")(options \"\")(descr \"Backplane\"))\n)\n"));
        let names: Vec<String> = entries(&out, "fp_lib_table").unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["KiCad", "amemb", "Backplane_Shoggoth", "Backplane_Logo"]);
        assert_eq!(plan(DEFAULT, &out, "fp_lib_table", &[]), None, "a second pass adds nothing");
    }
    #[test] fn a_name_added_once_is_not_added_again_after_the_person_removes_it() {
        assert_eq!(plan(DEFAULT, MINE, "fp_lib_table", &["Backplane_Logo".into()]), None);
    }
    #[test] fn a_table_that_is_not_whole_or_not_that_kind_is_left_alone() {
        assert_eq!(plan(DEFAULT, "(fp_lib_table\n\t(version 7)\n\t(lib (name \"x\")", "fp_lib_table", &[]), None, "cut short (being written)");
        assert_eq!(plan(DEFAULT, "(sym_lib_table\n\t(version 7)\n)\n", "fp_lib_table", &[]), None, "another table's head");
        assert_eq!(plan("(fp_lib_table (version 7))", MINE, "fp_lib_table", &[]), None, "a default with nothing to add");
    }
    #[test] fn settings_versions() {
        assert_eq!(settings_version("10.99.0-bp.13").as_deref(), Some("10.99"));
        assert_eq!(settings_version("10.0.7").as_deref(), Some("10.0"));
        assert_eq!(settings_version(""), None); assert_eq!(settings_version("bp"), None);
    }
    #[test] fn sync_merges_once_records_it_and_backs_the_table_up() {
        let t = crate::plat::tmp::dir("libtables-sync");
        let (root, cfg, home) = (t.path().join("rt"), t.path().join("cfg"), t.path().join("hub"));
        std::fs::create_dir_all(root.join("share/kicad")).unwrap(); std::fs::create_dir_all(cfg.join("10.99")).unwrap();
        std::fs::write(root.join("share/kicad/fp-lib-table"), DEFAULT).unwrap();
        std::fs::write(root.join("share/kicad/sym-lib-table"), "(sym_lib_table\n  (version 7)\n  (lib (name \"KiCad\")(type \"Table\")(uri \"x\")(options \"\")(descr \"\"))\n)\n").unwrap();
        std::fs::write(cfg.join("10.99/fp-lib-table"), MINE).unwrap();
        // the 10.0 folder (stock KiCad) has no table of this runtime's version: untouched
        std::fs::create_dir_all(cfg.join("10.0")).unwrap(); std::fs::write(cfg.join("10.0/fp-lib-table"), MINE).unwrap();
        let said = sync(&root, "10.99.0-bp.13", &cfg, &home);
        assert_eq!(said.len(), 1, "{said:?}"); assert!(said[0].starts_with("fp-lib-table 10.99: added Backplane_Logo"), "{said:?}");
        let now = std::fs::read_to_string(cfg.join("10.99/fp-lib-table")).unwrap();
        assert!(now.contains("(name \"Backplane_Logo\")") && now.contains("(name \"amemb\")"));
        assert_eq!(std::fs::read_to_string(cfg.join("10.0/fp-lib-table")).unwrap(), MINE);
        let backups: Vec<_> = std::fs::read_dir(home.join("backups/kicad-libtables")).unwrap().flatten().collect();
        assert_eq!(backups.len(), 1); assert_eq!(std::fs::read_to_string(backups[0].path()).unwrap(), MINE);
        // the person removes it: it is not put back
        std::fs::write(cfg.join("10.99/fp-lib-table"), MINE).unwrap();
        assert!(sync(&root, "10.99.0-bp.13", &cfg, &home).is_empty());
        assert_eq!(std::fs::read_to_string(cfg.join("10.99/fp-lib-table")).unwrap(), MINE);
        // no table of their own yet: the launcher makes it at first launch, not the hub
        std::fs::remove_file(cfg.join("10.99/fp-lib-table")).unwrap();
        assert!(sync(&root, "10.99.0-bp.13", &cfg, &home).is_empty()); assert!(!cfg.join("10.99/fp-lib-table").exists());
    }
}
