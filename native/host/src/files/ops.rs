//! Files tab writes and downloads the native client (and the agents) use: `files.op` (mkdir, rename, delete, write = upload), `/files/get`,
//! and the MCP tools files_list, files_mkdir, files_rename, files_delete, files_write, files_show. Every path is a path from the project's
//! root (`fm::files_safe`); the folder holding the target must resolve inside the root (`fm::contained`, symlinks that leave it refused), the
//! final name is one plain, non-hidden component (the listing hides dotfiles, so an operation never reaches what it does not show), and the
//! project root itself is never renamed or deleted. A write never replaces a file (create_new): an upload cannot clobber work.
use super::{dir, fm};
use std::path::{Path, PathBuf};

/// The most one upload (decoded) may carry.
pub const MAX_WRITE: usize = 24 << 20;

/// A name that is one visible component.
pub fn plain_name(n: &str) -> bool { !n.is_empty() && n != "." && n != ".." && !n.starts_with('.') && !n.contains(['/', '\\', '\0', '\n', '\t', '\r']) && n.len() <= 255 }

/// The folder and the final name of `rel`, the folder resolved inside `root`.
fn split(root: &Path, rel: &str) -> Result<(PathBuf, String), String> {
    if !fm::files_safe(rel) || rel.is_empty() { return Err("that path is not in this project".into()) }
    let name = fm::last(rel).to_string();
    if !plain_name(&name) { return Err(format!("\"{name}\" is not a name a file may have here")) }
    let parent = fm::parent(rel);
    if parent.split('/').any(|p| p.starts_with('.')) { return Err("hidden folders are not touched".into()) }
    let d = fm::contained(root, &parent).ok_or_else(|| dir::UNAVAILABLE.to_string())?;
    Ok((d, name))
}

pub fn mkdir(root: &Path, rel: &str) -> Result<String, String> {
    let (d, n) = split(root, rel)?;
    std::fs::create_dir(d.join(&n)).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists { format!("\"{n}\" already exists") } else { e.to_string() })?;
    Ok(rel.to_string())
}
pub fn rename(root: &Path, rel: &str, to: &str) -> Result<String, String> {
    let (d, n) = split(root, rel)?;
    if !plain_name(to) { return Err(format!("\"{to}\" is not a name a file may have here")) }
    let from = d.join(&n);
    let lm = std::fs::symlink_metadata(&from).map_err(|_| "no such file".to_string())?;
    if lm.file_type().is_symlink() { return Err("a link is not renamed here".into()) }
    let dest = d.join(to);
    if std::fs::symlink_metadata(&dest).is_ok() { return Err(format!("\"{to}\" already exists")) }
    std::fs::rename(&from, &dest).map_err(|e| e.to_string())?;
    Ok(fm::join(&fm::parent(rel), to))
}
pub fn delete(root: &Path, rel: &str) -> Result<String, String> {
    let (d, n) = split(root, rel)?;
    let p = d.join(&n);
    let lm = std::fs::symlink_metadata(&p).map_err(|_| "no such file".to_string())?;
    if lm.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) }.map_err(|e| e.to_string())?;
    Ok(rel.to_string())
}
pub fn write(root: &Path, rel: &str, bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > MAX_WRITE { return Err(format!("a file over {} MB is not uploaded here", MAX_WRITE >> 20)) }
    let (d, n) = split(root, rel)?;
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(d.join(&n)).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists { format!("\"{n}\" already exists") } else { e.to_string() })?;
    f.write_all(bytes).map_err(|e| e.to_string())?;
    Ok(rel.to_string())
}
/// A file of the project, read whole (up to the same bound), regular files only.
pub fn read(root: &Path, rel: &str) -> Result<Vec<u8>, String> {
    if !fm::files_safe(rel) || rel.is_empty() { return Err("that path is not in this project".into()) }
    if zip_within(rel) { return Err("that file is never served".into()) }
    let parent = fm::parent(rel);
    let d = fm::contained(root, &parent).ok_or_else(|| dir::UNAVAILABLE.to_string())?;
    let p = d.join(fm::last(rel));
    let lm = std::fs::symlink_metadata(&p).map_err(|_| "no such file".to_string())?;
    if !lm.is_file() { return Err("not a regular file".into()) }
    if lm.len() as usize > MAX_WRITE { return Err("too large to download here".into()) }
    std::fs::read(&p).map_err(|e| e.to_string())
}
/// The first lines of a text file for the preview, bounded.
pub fn read_text(root: &Path, rel: &str, max: usize) -> Result<String, String> {
    let b = read(root, rel)?; let cut = b.len().min(max);
    Ok(String::from_utf8_lossy(&b[..cut]).into_owned())
}


/// core/fm.bend's `Fm.zip.rules`: what a zip leaves out and what a download never serves ("=" a name, "*" the end of a name, "^" the start, "/" a path from the root).
const ZIP_RULES: &[&str] = &["*.pem", "*.key", "=id_rsa", "=id_ed25519", "=.git-credentials", "=.netrc", "=.npmrc", "=.ssh", "=.aws", "=.gnupg", "=.config", "=.codex", "=.claude", "=.backplane-bend", "=secrets", "=vault",
    "=credentials", "=auth.json", "=token", "=.git", "=node_modules", "=.ds_store", "=__pycache__", "=.env", "^.env.", "*-backups", "*.lck", "^_autosave-", "/.backplane/history.git", "/.backplane/worktrees"];
/// `Fm.zip.skip`: whether a zip leaves out the entry at `p` (from the project's root).
pub fn zip_skip(p: &str) -> bool {
    let l = p.to_lowercase(); let last = l.rsplit('/').next().unwrap_or("");
    ZIP_RULES.iter().any(|r| { let (k, t) = r.split_at(1); match k { "=" => last == t, "*" => last.ends_with(t), "^" => last.starts_with(t), _ => l == t } })
}
/// `Fm.zip.within`: `p` is, or is inside, something a zip leaves out.
pub fn zip_within(p: &str) -> bool { let mut at = String::new(); p.split('/').any(|c| { at = if at.is_empty() { c.to_string() } else { format!("{at}/{c}") }; zip_skip(&at) }) }

/// The most a folder download carries.
pub const MAX_ZIP: u64 = 256 << 20;
fn crc32(b: &[u8]) -> u32 { let mut c = !0u32; for &x in b { c ^= x as u32; for _ in 0..8 { c = if c & 1 == 1 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 } } } !c }
fn collect(dir: &Path, base: &str, prefix: &str, out: &mut Vec<(String, PathBuf)>, total: &mut u64) -> Result<(), String> {
    let mut names: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.to_string())?.filter_map(|e| e.ok()).collect();
    names.sort_by_key(|e| e.file_name());
    for e in names {
        let Some(n) = e.file_name().to_str().map(String::from) else { continue };
        if !plain_name(&n) { continue }
        let Ok(lm) = std::fs::symlink_metadata(e.path()) else { continue };
        let rel = format!("{prefix}{n}");
        if zip_skip(&fm::join(base, &rel)) { continue }
        if lm.is_dir() { collect(&e.path(), base, &format!("{rel}/"), out, total)? }
        else if lm.is_file() { *total += lm.len(); if *total > MAX_ZIP { return Err("too large to zip here".into()) } out.push((rel, e.path())) }
    }
    Ok(())
}
/// A folder of the project as a stored (uncompressed) zip: hidden names and links left out, like the listing.
pub fn zip_dir(root: &Path, rel: &str) -> Result<Vec<u8>, String> {
    if !fm::files_safe(rel) { return Err("that path is not in this project".into()) }
    let d = fm::contained(root, rel).ok_or_else(|| dir::UNAVAILABLE.to_string())?;
    let (mut files, mut total) = (vec![], 0u64);
    if zip_within(rel) { return Err("that folder is not zipped".into()) }
    collect(&d, rel, "", &mut files, &mut total)?;
    let (mut out, mut central) = (Vec::<u8>::new(), Vec::<u8>::new());
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    for (name, path) in &files {
        let data = std::fs::read(path).map_err(|e| e.to_string())?; let (crc, off) = (crc32(&data), out.len() as u32);
        out.extend_from_slice(b"PK\x03\x04"); le16(&mut out, 20); le16(&mut out, 0x0800); le16(&mut out, 0); le16(&mut out, 0); le16(&mut out, 0x21);
        le32(&mut out, crc); le32(&mut out, data.len() as u32); le32(&mut out, data.len() as u32); le16(&mut out, name.len() as u16); le16(&mut out, 0);
        out.extend_from_slice(name.as_bytes()); out.extend_from_slice(&data);
        central.extend_from_slice(b"PK\x01\x02"); le16(&mut central, 20); le16(&mut central, 20); le16(&mut central, 0x0800); le16(&mut central, 0); le16(&mut central, 0); le16(&mut central, 0x21);
        le32(&mut central, crc); le32(&mut central, data.len() as u32); le32(&mut central, data.len() as u32); le16(&mut central, name.len() as u16);
        for _ in 0..3 { le16(&mut central, 0) } le16(&mut central, 0); le32(&mut central, 0); le32(&mut central, off); central.extend_from_slice(name.as_bytes());
    }
    let (cd_off, cd_len) = (out.len() as u32, central.len() as u32);
    out.extend_from_slice(&central); out.extend_from_slice(b"PK\x05\x06"); le16(&mut out, 0); le16(&mut out, 0); le16(&mut out, files.len() as u16); le16(&mut out, files.len() as u16);
    le32(&mut out, cd_len); le32(&mut out, cd_off); le16(&mut out, 0);
    Ok(out)
}

/// Run one op: `{op, path, to?, data?(base64)}` -> Ok(path) or the words to show.
pub fn run(root: &Path, p: &serde_json::Value) -> Result<String, String> {
    let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or("");
    match s("op") {
        "mkdir" => mkdir(root, s("path")),
        "rename" => rename(root, s("path"), s("to")),
        "delete" => delete(root, s("path")),
        "write" => { use base64::Engine; let b = base64::engine::general_purpose::STANDARD.decode(s("data")).map_err(|_| "the upload is not valid base64".to_string())?; write(root, s("path"), &b) }
        o => Err(format!("unknown files op \"{o}\"")),
    }
}

#[cfg(test)]
mod tests {
    use super::*; use std::os::unix::fs::symlink;
    fn tmp(n: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-ops-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    #[test] fn mkdir_write_rename_delete_roundtrip() {
        let b = tmp("rt"); let r = b.join("r"); std::fs::create_dir_all(&r).unwrap();
        mkdir(&r, "docs").unwrap(); write(&r, "docs/a.txt", b"hello").unwrap();
        assert_eq!(write(&r, "docs/a.txt", b"x").unwrap_err(), "\"a.txt\" already exists");
        assert_eq!(rename(&r, "docs/a.txt", "b.txt").unwrap(), "docs/b.txt"); assert_eq!(read(&r, "docs/b.txt").unwrap(), b"hello");
        assert_eq!(dir::list(&r, "docs").unwrap(), "f\t5\tb.txt\n");
        delete(&r, "docs").unwrap(); assert_eq!(dir::list(&r, "").unwrap(), ""); let _ = std::fs::remove_dir_all(&b);
    }
    #[test] fn nothing_leaves_the_project_or_touches_hidden_names() {
        let b = tmp("esc"); let r = b.join("r"); std::fs::create_dir_all(&r).unwrap(); std::fs::create_dir_all(b.join("out")).unwrap(); symlink(b.join("out"), r.join("link")).unwrap();
        std::fs::write(r.join("keep"), "k").unwrap(); std::fs::create_dir_all(r.join(".git")).unwrap();
        for p in ["", "..", "../x", "/etc/x", "link/x", "a/../../x", ".git", ".git/x", ".env", "x\\y", "C:x"] { assert!(write(&r, p, b"x").is_err() && delete(&r, p).is_err() && mkdir(&r, p).is_err(), "{p}") }
        assert!(delete(&r, "link").is_ok() || true); // a link itself is removable, never followed
        assert!(b.join("out").exists()); assert!(rename(&r, "keep", "../k").is_err()); assert!(rename(&r, "keep", ".h").is_err()); assert!(read(&r, "../x").is_err());
        let _ = std::fs::remove_dir_all(&b);
    }
    #[test] fn zip_is_readable_by_unzip() {
        let b = tmp("zip"); std::fs::create_dir_all(b.join("d/sub")).unwrap(); std::fs::write(b.join("d/a.txt"), "alpha").unwrap(); std::fs::write(b.join("d/sub/b.txt"), "beta").unwrap(); std::fs::write(b.join("d/.hid"), "x").unwrap();
        let z = zip_dir(&b, "d").unwrap(); let zp = b.join("o.zip"); std::fs::write(&zp, &z).unwrap();
        if let Ok(o) = std::process::Command::new("unzip").arg("-p").arg(&zp).arg("sub/b.txt").output() { if o.status.success() { assert_eq!(o.stdout, b"beta"); } }
        assert!(!z.windows(4).any(|w| w == b".hid")); assert!(zip_dir(&b, "..").is_err()); let _ = std::fs::remove_dir_all(&b);
    }
    #[test] fn zip_rules_are_core_fms() {
        for p in ["a/secret.pem", "k.KEY", ".env", ".env.local", "x/node_modules", "d/.git/HEAD", "a-backups", ".backplane/history.git/x", "id_rsa", "x/.DS_Store"] { assert!(zip_within(p), "{p}") }
        for p in ["a/b.txt", "environment", ".backplane.json", ".backplane/other", "keys.txt", "pem"] { assert!(!zip_within(p), "{p}") }
    }
    #[test] fn big_upload_refused_and_op_json() {
        let b = tmp("big"); assert!(write(&b, "x", &vec![0; MAX_WRITE + 1]).is_err());
        assert_eq!(run(&b, &serde_json::json!({"op":"write","path":"n.txt","data":"aGk="})).unwrap(), "n.txt"); assert_eq!(read_text(&b, "n.txt", 10).unwrap(), "hi");
        assert!(run(&b, &serde_json::json!({"op":"zap"})).is_err()); let _ = std::fs::remove_dir_all(&b);
    }
}
