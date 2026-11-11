//! Where a download goes, and what it is called. Kept out of the embedded browser (feature `cef`) so the default test run, which CI and the gate build without the Chromium distribution, covers it.
#![cfg_attr(not(feature = "cef"), allow(dead_code))]
use std::path::{Path, PathBuf};

/// The person's Downloads folder: XDG_DOWNLOAD_DIR, else ~/Downloads.
pub fn downloads_folder() -> PathBuf { std::env::var_os("XDG_DOWNLOAD_DIR").map(PathBuf::from).or_else(|| crate::plat::user_home().map(|h| h.join("Downloads"))).unwrap_or_else(std::env::temp_dir) } // Windows: %USERPROFILE%\Downloads (no HOME there)
/// A finished download at `file` (named by the engine, e.g. a guid in a temp folder) moved into the Downloads folder as `suggested`; the file already there, or a failure, leaves `file` as it was.
pub fn bring_to_downloads(file: &Path, suggested: &str) -> PathBuf { bring_to(&downloads_folder(), file, suggested) }
fn bring_to(dir: &Path, file: &Path, suggested: &str) -> PathBuf {
    if file.parent() == Some(dir) || suggested.trim().is_empty() || !file.is_file() { return file.to_path_buf() }
    let _ = std::fs::create_dir_all(dir);
    let to = unique_path(dir, &safe_name(suggested));
    if std::fs::rename(file, &to).is_ok() || std::fs::copy(file, &to).is_ok() { to } else { file.to_path_buf() }
}
pub fn safe_name(n: &str) -> String { let n: String = n.chars().map(|c| if c == '/' || c == '\\' || c.is_control() { '_' } else { c }).collect(); let n = n.trim_matches('.').trim().to_string(); if n.is_empty() { "download".into() } else { n } }
/// `dir/name`, or `dir/name (2)`... when that file exists
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name); if !first.exists() { return first }
    let (stem, ext) = match name.rsplit_once('.') { Some((a, b)) if !a.is_empty() => (a.to_string(), format!(".{b}")), _ => (name.to_string(), String::new()) };
    /* i18n-ok: a file name pattern */ (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_finished_download_named_by_the_engine_is_brought_to_downloads_under_its_own_name() {
        let d = crate::tmp::uncreated("dl-bring");
        let (tmp, dl) = (d.join("artifacts"), d.join("Downloads")); std::fs::create_dir_all(&tmp).unwrap();
        let f = tmp.join("461feba6-guid"); std::fs::write(&f, "bytes").unwrap();
        let to = bring_to(&dl, &f, "report.txt"); assert_eq!(to, dl.join("report.txt")); assert_eq!(std::fs::read_to_string(&to).unwrap(), "bytes"); assert!(!f.exists());
        let g = tmp.join("second-guid"); std::fs::write(&g, "again").unwrap(); let to2 = bring_to(&dl, &g, "report.txt"); assert_eq!(to2, dl.join("report (2).txt"), "never over a file");
        assert_eq!(bring_to(&dl, &to, "report.txt"), to, "a file already in Downloads stays");
        assert_eq!(bring_to(&dl, &tmp.join("nothing"), "x.txt"), tmp.join("nothing"), "a missing file is left alone");
        let _ = std::fs::remove_dir_all(&d);
    }
}
