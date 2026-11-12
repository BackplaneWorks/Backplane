//! Where profiles live: `<home>/fab/profiles/<fab>.json` (the current one, as gathered) and `<home>/fab/history/<fab>/<rev>.json` (every
//! stored revision, so a change can be shown against what a project last saw). A fresh install has the bundled baseline (this folder's
//! baseline/), the profile as read when this Backplane was built: the newer of it and the stored one is the profile.
use super::model::Profile;
use std::path::{Path, PathBuf};

/// The bundled baselines: (fab, profile JSON).
pub const BASELINE: &[(&str, &str)] = &[("jlcpcb", include_str!("baseline/jlcpcb.json")), ("pcbway", include_str!("baseline/pcbway.json")), ("oshpark", include_str!("baseline/oshpark.json"))];

pub fn dir(home: &Path) -> PathBuf { home.join("fab") }
fn ok_id(fab: &str) -> bool { !fab.is_empty() && fab.len() <= 40 && fab.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-') }
pub fn current_path(home: &Path, fab: &str) -> PathBuf { dir(home).join("profiles").join(format!("{fab}.json")) }
pub fn history_path(home: &Path, fab: &str, rev: u64) -> PathBuf { dir(home).join("history").join(fab).join(format!("{rev:06}.json")) }

pub fn baseline(fab: &str) -> Option<Profile> { BASELINE.iter().find(|(f, _)| *f == fab).and_then(|(_, t)| Profile::parse(t).ok()) }
/// The fabs there are profiles of: bundled first, then any gathered only here.
pub fn fabs(home: &Path) -> Vec<String> {
    let mut v: Vec<String> = BASELINE.iter().map(|(f, _)| f.to_string()).collect();
    if let Ok(rd) = std::fs::read_dir(dir(home).join("profiles")) {
        let mut more: Vec<String> = rd.flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).map(str::to_string)).filter(|f| ok_id(f) && !v.contains(f)).collect();
        more.sort(); v.extend(more);
    }
    v
}
/// The fab Backplane orders from.
pub fn ordering_fab() -> String { BASELINE.iter().find(|(_, t)| Profile::parse(t).map_or(false, |p| p.orders)).map_or("jlcpcb", |(f, _)| f).to_string() }

/// The profile of a fab: the stored one unless the bundled baseline is newer (a Backplane built after it was last gathered).
pub fn load(home: &Path, fab: &str) -> Option<Profile> {
    if !ok_id(fab) { return None }
    let stored = std::fs::read_to_string(current_path(home, fab)).ok().and_then(|t| Profile::parse(&t).ok());
    match (stored, baseline(fab)) {
        (Some(s), Some(b)) => Some(if (b.updated.as_str(), b.rev) > (s.updated.as_str(), s.rev) { b } else { s }),
        (s, b) => s.or(b),
    }
}
/// A revision as it was stored (none for the bundled one unless it was stored once).
pub fn revision(home: &Path, fab: &str, rev: u64) -> Option<Profile> {
    if !ok_id(fab) { return None }
    std::fs::read_to_string(history_path(home, fab, rev)).ok().and_then(|t| Profile::parse(&t).ok()).or_else(|| baseline(fab).filter(|b| b.rev == rev))
}

fn write_atomic(p: &Path, text: &str) -> std::io::Result<()> {
    if let Some(d) = p.parent() { std::fs::create_dir_all(d)? }
    let tmp = p.with_extension("json.tmp"); std::fs::write(&tmp, text)?; std::fs::rename(&tmp, p)
}
/// Keep a profile as the current one and in the history.
pub fn save(home: &Path, p: &Profile) -> Result<(), String> {
    if !ok_id(&p.fab) { return Err(format!("not a fab id: {}", p.fab)) }
    if crate::copy::on() { return Err(crate::copy::HOME_ONLY.into()) }
    let text = serde_json::to_string_pretty(p).map_err(|e| e.to_string())? + "\n";
    write_atomic(&history_path(home, &p.fab, p.rev), &text).and_then(|_| write_atomic(&current_path(home, &p.fab), &text)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn fab_store_a_fresh_home_has_the_bundled_profile_and_a_newer_one_wins() {
        let h = crate::plat::tmp::dir("fab-store"); let home: &Path = &h;
        let b = load(home, "jlcpcb").unwrap(); assert_eq!((b.rev, fabs(home)), (1, vec!["jlcpcb".to_string(), "pcbway".to_string(), "oshpark".to_string()]));
        assert_eq!(ordering_fab(), "jlcpcb");
        let mut n = b.clone(); n.rev = 2; n.updated = "2099-01-01".into(); save(home, &n).unwrap();
        assert_eq!(load(home, "jlcpcb").unwrap().rev, 2); assert_eq!(revision(home, "jlcpcb", 2).unwrap().updated, "2099-01-01"); assert_eq!(revision(home, "jlcpcb", 1).unwrap().rev, 1, "the bundled revision");
        let mut old = b.clone(); old.rev = 3; old.updated = "2000-01-01".into(); save(home, &old).unwrap();
        assert_eq!(load(home, "jlcpcb").unwrap().updated, b.updated, "a stored profile older than the bundled one loses to it");
        assert!(load(home, "../x").is_none()); assert!(load(home, "nofab").is_none());
        let mut other = b.clone(); other.fab = "otherfab".into(); save(home, &other).unwrap(); assert_eq!(fabs(home), ["jlcpcb", "pcbway", "oshpark", "otherfab"]);
    }
}
