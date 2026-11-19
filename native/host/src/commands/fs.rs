//! The project picker's folder requests: `fs.list`, `fs.mkdir`, `fs.find`, and the folder
//! half of `project.add` / `project.new`. Paths are arguments to the scripts, never shell text.
use crate::files::git::{proc, proc_ro, Ran};
use serde_json::{json, Value};

/// FP.Folder.path.sh: "" and ~ are home, ~/x and relative paths are under home.
const PATH: &str = "d=\"$1\"; if command -v cygpath >/dev/null 2>&1; then case \"$d\" in /[a-zA-Z]:[\\\\/]*) d=\"${d#/}\";; esac; case \"$d\" in [a-zA-Z]:[\\\\/]*|\\\\\\\\*|//*) d=$(cygpath -au -- \"$d\") || { echo \"cannot resolve folder: $1\"; exit 2; };; [a-zA-Z]:*) echo \"invalid drive path: $d (use an absolute folder)\"; exit 2;; esac; fi; case \"$d\" in ''|'~') d=\"$HOME\";; '~/'*) d=\"$HOME/${d#??}\";; /*) ;; [a-zA-Z]:*) echo \"cannot resolve Windows folder: $d\"; exit 2;; *) d=\"$HOME/$d\";; esac; ";
const NATIVE: &str = "if command -v cygpath >/dev/null 2>&1; then d=$(cygpath -am -- \"$d\") || { echo \"cannot resolve folder: $d\"; exit 2; }; fi; ";
/// `$n`: the folder as the person reads it, for a message (Windows: cygpath -am's `C:/x/y`, never Git's sh's `/tmp/...` or `/c/...`, which the
/// answers already were and the errors were not, Nov 19: /project said "/tmp/.../afile is there and is not a folder."); elsewhere `$d`.
const SHOWN: &str = "n=$d; if command -v cygpath >/dev/null 2>&1; then n=$(cygpath -am -- \"$d\" 2>/dev/null) || n=$d; fi; ";
const DIR: &str = "[ -e \"$d\" ] || { echo \"no such folder: $n\"; exit 2; }; [ -d \"$d\" ] || { echo \"not a folder: $n\"; exit 2; }; [ -r \"$d\" ] && [ -x \"$d\" ] && d=$(cd -P -- \"$d\" && pwd -P) || { echo \"cannot open: $n\"; exit 2; }; ";
const TMO: &str = "tmo() { t=$1; shift; if command -v timeout >/dev/null 2>&1; then timeout -k 1 \"$t\" \"$@\"; elif command -v gtimeout >/dev/null 2>&1; then gtimeout -k 1 \"$t\" \"$@\"; else \"$@\"; fi; }; ";

fn list_sh() -> String { format!("exec 2>&1; set -e; {PATH}{SHOWN}{DIR}{NATIVE}echo \"$d\"; find \"$d\" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sed 's|^.*/||' | LC_ALL=C sort | head -n 5000") }
fn mkdir_sh() -> String { format!("exec 2>&1; set -e; {PATH}{SHOWN}if [ -e \"$d\" ] || [ -L \"$d\" ]; then echo \"already exists: $n\"; exit 3; fi; mkdir -p -- \"$d\"; {DIR}{NATIVE}echo \"$d\"") }
fn resolve_sh() -> String { format!("exec 2>&1; set -e; {PATH}{SHOWN}{DIR}{NATIVE}echo \"$d\"") }
fn find_sh() -> String { format!("exec 2>/dev/null; {TMO}echo \"$HOME\"; tmo 5 find \"$HOME\" -xdev -mindepth 1 -maxdepth 5 \\( -type d \\( -name node_modules -o -path \"$HOME/Library\" -o \\( -name '.*' ! -name .backplane \\) \\) -prune \\) -o \\( -type d -name .backplane -print -prune \\) -o \\( -name .backplane.json -print \\) | sed 's|/[^/]*$||' | grep -vxF \"$HOME\" | LC_ALL=C sort -u | head -n 500; true") }

/// A Windows drive or UNC path with its backslashes as slashes (`C:\x\y` -> `C:/x/y`, `\\srv\s` -> `//srv/s`); any other path as it is.
/// Git for Windows' sh reads a `-c` script from a command line that MSYS2 parses again, so the script's `[\\/]` reaches it as `[\/]` and a
/// pasted `C:\...` was refused as "invalid drive path" (bp-win, 2026-11-15); the script's own forms `C:/...` and `//...` go through cygpath.
pub fn slashed(p: &str) -> String {
    let b = p.as_bytes();
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
    if drive || p.starts_with("\\\\") { p.replace('\\', "/") } else { p.to_string() }
}
/// A write (mkdir) is refused on a copy; `sh_ro` is the listed read.
async fn sh(script: &str, path: &str) -> Ran { let p = slashed(path); proc("sh", &["-c", script, "sh", &p], "/", &[]).await }
async fn sh_ro(script: &str, path: &str) -> Ran { let p = slashed(path); proc_ro("sh", &["-c", script, "sh", &p], "/", &[]).await }
fn lines(out: &str) -> Vec<String> { out.trim().lines().map(str::to_string).collect() }
fn error(out: &str) -> Value { json!({"error": out.trim()}) }

/// FsJob.listed: `{dir, entries}` or `{error}`.
fn listed(r: &Ran) -> Value {
    if r.code != 0 { return error(&r.out) }
    let mut ls = lines(&r.out);
    if ls.is_empty() { return json!({"error":"not a folder"}) }
    let d = ls.remove(0); json!({"dir": d, "entries": ls})
}
/// A folder macOS's privacy settings refuse lists as empty or fails as "cannot open": then the words for it (plat::privacy_block) instead.
fn private(path: &str, v: Value) -> Value {
    let empty = v["entries"].as_array().map_or(false, |e| e.is_empty());
    if v.get("error").is_none() && !empty { return v }
    match crate::plat::privacy_block(v["dir"].as_str().unwrap_or(path)) { Some(w) => error(&w), None => v }
}
pub async fn list(path: &str) -> Value { if no_sh() { return native::list(path, &crate::plat::dirs::home_or_empty()) } private(path, listed(&sh_ro(&list_sh(), path).await)) }
/// FsJob.made: a new folder, then its listing.
pub async fn mkdir(path: &str) -> Value { if no_sh() { return match make(path).await { Ok(d) => list(&d).await, Err(e) => e } } let r = sh(&mkdir_sh(), path).await; if r.code == 0 { list(lines(&r.out).last().map(String::as_str).unwrap_or("")).await } else { error(&r.out) } }
pub async fn find() -> Value {
    let r = sh_ro(&find_sh(), "").await; let mut ls = lines(&r.out);
    if ls.is_empty() { json!({"found": []}) } else { let h = ls.remove(0); json!({"home": h, "found": ls}) }
}
/// The folder a project would have: the resolved path or `{error}`.
pub async fn resolve(path: &str) -> Result<String, Value> { if no_sh() { return native::resolve(path, &crate::plat::dirs::home_or_empty()).map_err(|e| error(&e)) } let r = sh_ro(&resolve_sh(), path).await; if r.code == 0 { Ok(lines(&r.out).last().cloned().unwrap_or_default()) } else { Err(private(path, error(&r.out))) } }
/// FsJob.create's folder half for project.new: make it, then answer its path.
pub async fn make(path: &str) -> Result<String, Value> {
    if no_sh() { if crate::copy::on() { return Err(error("a hub on a copy makes no folders")) } return native::make(path, &crate::plat::dirs::home_or_empty()).map_err(|e| error(&e)) }
    let r = sh(&mkdir_sh(), path).await; if r.code == 0 { Ok(lines(&r.out).last().cloned().unwrap_or_default()) } else { Err(private(path, error(&r.out))) } }
/// FsJob.register: a resolved path must be a folder.
pub fn is_dir(path: &str) -> bool { std::fs::metadata(path).map_or(false, |m| m.is_dir()) }
/// A folder with nothing in it (hidden files count): /project makes its project in one, never in one that holds something.
pub fn is_empty(path: &str) -> bool { std::fs::read_dir(path).map_or(false, |mut r| r.next().is_none()) }
/// /project's refusal of a folder that holds something: the folder, and what to do instead.
pub fn not_empty(dir: &str) -> String { format!("{dir} is there and is not empty. Add it as it is (Add existing), or give /project a new name.") }
/// A folder request's error in the person's words for /project: no permission said plainly, the script's own words otherwise.
pub fn plain(e: &str) -> String {
    let low = e.to_lowercase();
    if low.contains("permission denied") || low.contains("access is denied") || low.contains("read-only file system") { return format!("No permission to make that folder there ({}).", e.trim()) }
    if let Some(p) = e.trim().strip_prefix("cannot open: ") { return format!("No permission to open {p}.") }
    if let Some(p) = e.trim().strip_prefix("already exists: ") { return format!("{p} is there and is not a folder.") }
    e.trim().to_string()
}

/// Windows before Git for Windows is installed (#1163): no `sh` runs the scripts above, so the picker and project.add/new do the same in
/// Rust (setup's Design tools step, or a thread's Install Git ask, installs it). Answers and errors are the scripts' own: `C:/x/y` as
/// cygpath -am writes it, "no such folder: ...". `fs.find` finds nothing then.
fn no_sh() -> bool { cfg!(windows) && crate::plat::hub::program(std::ffi::OsStr::new("sh")) == "sh.exe" }
mod native {
    use serde_json::{json, Value};
    use std::path::{Path, PathBuf};
    /// FP.Folder.path.sh: "" and ~ are home, ~/x and relative paths are under home; a drive with no slash after it is refused.
    pub fn path(p: &str, home: &str) -> Result<PathBuf, String> {
        let b = p.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && !(b.len() >= 3 && (b[2] == b'\\' || b[2] == b'/')) { return Err(format!("invalid drive path: {p} (use an absolute folder)")) }
        let rooted = b.len() >= 3 && b[1] == b':' || p.starts_with('/') || p.starts_with('\\');
        Ok(match p {
            "" | "~" => PathBuf::from(home),
            _ if p.starts_with("~/") || p.starts_with("~\\") => Path::new(home).join(&p[2..]),
            _ if rooted => PathBuf::from(p),
            _ => Path::new(home).join(p),
        })
    }
    fn shown(d: &Path) -> String { d.to_string_lossy().replace('\\', "/") }
    pub fn resolve(p: &str, home: &str) -> Result<String, String> {
        let d = path(p, home)?; let s = shown(&d);
        if !d.exists() { return Err(format!("no such folder: {s}")) }
        if !d.is_dir() { return Err(format!("not a folder: {s}")) }
        std::fs::read_dir(&d).map_err(|_| format!("cannot open: {s}"))?;
        crate::plat::fs::canonicalize(&d).map(|c| shown(&c)).map_err(|_| format!("cannot open: {s}"))
    }
    pub fn make(p: &str, home: &str) -> Result<String, String> {
        let d = path(p, home)?;
        if d.exists() || d.symlink_metadata().is_ok() { return Err(format!("already exists: {}", shown(&d))) }
        std::fs::create_dir_all(&d).map_err(|e| format!("cannot make {}: {e}", shown(&d)))?;
        resolve(&shown(&d), home)
    }
    /// FsJob.listed: `{dir, entries}` (sub-folders by name, byte order, at most 5000) or `{error}`.
    pub fn list(p: &str, home: &str) -> Value {
        let d = match resolve(p, home) { Ok(d) => d, Err(e) => return json!({"error": e}) };
        let mut v: Vec<String> = std::fs::read_dir(&d).map(|r| r.flatten().filter(|e| e.file_type().map_or(false, |t| t.is_dir())).map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
        v.sort(); v.truncate(5000);
        json!({"dir": d, "entries": v})
    }
}

#[cfg(test)]
mod project_tests {
    #[test] fn slash_project_folders_say_why_in_plain_words() {
        let t = crate::plat::tmp::dir("fsp"); let d = t.to_string_lossy().into_owned();
        assert!(super::is_empty(&d), "a new folder is empty");
        std::fs::write(t.join(".x"), "").unwrap(); assert!(!super::is_empty(&d), "a hidden file counts");
        assert!(super::not_empty(&d).contains("Add existing"));
        assert!(super::plain("mkdir: cannot create directory '/root/x': Permission denied").starts_with("No permission"));
        assert!(super::plain("cannot make C:/x: Access is denied. (os error 5)").starts_with("No permission"));
        assert_eq!(super::plain("already exists: /h/f"), "/h/f is there and is not a folder.");
        assert_eq!(super::plain("cannot open: /root"), "No permission to open /root.");
        assert_eq!(super::plain("no such folder: /x"), "no such folder: /x");
        let _ = std::fs::remove_dir_all(&t);
    }
}

#[cfg(test)]
mod slash_tests {
    use super::slashed;
    #[test] fn windows_paths_reach_sh_with_slashes() {
        assert_eq!(slashed(r"C:\bp\psmoke\out\proj"), "C:/bp/psmoke/out/proj");
        assert_eq!(slashed(r"d:\"), "d:/");
        assert_eq!(slashed(r"\\srv\share\x"), "//srv/share/x");
        assert_eq!(slashed("C:/already/fine"), "C:/already/fine");
        assert_eq!(slashed("/home/h/a\\b"), "/home/h/a\\b", "a POSIX name with a backslash is left alone");
        assert_eq!(slashed("~/x"), "~/x"); assert_eq!(slashed(""), "");
    }
}

#[cfg(test)]
mod native_tests {
    use super::native;
    #[test] fn folders_without_sh_answer_as_the_scripts_do() {
        let tmp = crate::plat::tmp::dir("fsn"); let base = tmp.to_path_buf(); std::fs::create_dir_all(base.join("b")).unwrap(); std::fs::create_dir_all(base.join("a")).unwrap(); std::fs::write(base.join("f"), "x").unwrap();
        let home = base.to_string_lossy().into_owned();
        assert_eq!(native::path("", &home).unwrap(), base); assert_eq!(native::path("~", &home).unwrap(), base); assert_eq!(native::path("~/a", &home).unwrap(), base.join("a")); assert_eq!(native::path("a", &home).unwrap(), base.join("a"));
        assert_eq!(native::path("C:\\x", &home).unwrap(), std::path::PathBuf::from("C:\\x")); assert!(native::path("C:x", &home).unwrap_err().starts_with("invalid drive path"));
        let l = native::list("", &home); assert_eq!(l["entries"], serde_json::json!(["a", "b"]), "{l}");
        assert!(native::resolve("f", &home).unwrap_err().starts_with("not a folder")); assert!(native::resolve("nope", &home).unwrap_err().starts_with("no such folder"));
        let m = native::make("new/proj", &home).unwrap(); assert!(m.ends_with("new/proj") && !m.contains('\\'), "{m}"); assert!(native::make("new/proj", &home).unwrap_err().starts_with("already exists"));
    }
}
