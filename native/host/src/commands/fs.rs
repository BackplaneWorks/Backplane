//! The project picker's folder requests: `fs.list`, `fs.mkdir`, `fs.find`, and the folder
//! half of `project.add` / `project.new`. Paths are arguments to the scripts, never shell text.
use crate::files::git::{proc, proc_ro, Ran};
use serde_json::{json, Value};

/// FP.Folder.path.sh: "" and ~ are home, ~/x and relative paths are under home.
const PATH: &str = "d=\"$1\"; if command -v cygpath >/dev/null 2>&1; then case \"$d\" in /[a-zA-Z]:[\\\\/]*) d=\"${d#/}\";; esac; case \"$d\" in [a-zA-Z]:[\\\\/]*|\\\\\\\\*|//*) d=$(cygpath -au -- \"$d\") || { echo \"cannot resolve folder: $1\"; exit 2; };; [a-zA-Z]:*) echo \"invalid drive path: $d (use an absolute folder)\"; exit 2;; esac; fi; case \"$d\" in ''|'~') d=\"$HOME\";; '~/'*) d=\"$HOME/${d#??}\";; /*) ;; [a-zA-Z]:*) echo \"cannot resolve Windows folder: $d\"; exit 2;; *) d=\"$HOME/$d\";; esac; ";
const NATIVE: &str = "if command -v cygpath >/dev/null 2>&1; then d=$(cygpath -am -- \"$d\") || { echo \"cannot resolve folder: $d\"; exit 2; }; fi; ";
const DIR: &str = "[ -e \"$d\" ] || { echo \"no such folder: $d\"; exit 2; }; [ -d \"$d\" ] || { echo \"not a folder: $d\"; exit 2; }; [ -r \"$d\" ] && [ -x \"$d\" ] && d=$(cd -P -- \"$d\" && pwd -P) || { echo \"cannot open: $d\"; exit 2; }; ";
const TMO: &str = "tmo() { t=$1; shift; if command -v timeout >/dev/null 2>&1; then timeout -k 1 \"$t\" \"$@\"; elif command -v gtimeout >/dev/null 2>&1; then gtimeout -k 1 \"$t\" \"$@\"; else \"$@\"; fi; }; ";

fn list_sh() -> String { format!("exec 2>&1; set -e; {PATH}{DIR}{NATIVE}echo \"$d\"; find \"$d\" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sed 's|^.*/||' | LC_ALL=C sort | head -n 5000") }
fn mkdir_sh() -> String { format!("exec 2>&1; set -e; {PATH}if [ -e \"$d\" ] || [ -L \"$d\" ]; then echo \"already exists: $d\"; exit 3; fi; mkdir -p -- \"$d\"; {DIR}{NATIVE}echo \"$d\"") }
fn resolve_sh() -> String { format!("exec 2>&1; set -e; {PATH}{DIR}{NATIVE}echo \"$d\"") }
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
pub async fn list(path: &str) -> Value { private(path, listed(&sh_ro(&list_sh(), path).await)) }
/// FsJob.made: a new folder, then its listing.
pub async fn mkdir(path: &str) -> Value { let r = sh(&mkdir_sh(), path).await; if r.code == 0 { list(lines(&r.out).last().map(String::as_str).unwrap_or("")).await } else { error(&r.out) } }
pub async fn find() -> Value {
    let r = sh_ro(&find_sh(), "").await; let mut ls = lines(&r.out);
    if ls.is_empty() { json!({"found": []}) } else { let h = ls.remove(0); json!({"home": h, "found": ls}) }
}
/// The folder a project would have: the resolved path or `{error}`.
pub async fn resolve(path: &str) -> Result<String, Value> { let r = sh_ro(&resolve_sh(), path).await; if r.code == 0 { Ok(lines(&r.out).last().cloned().unwrap_or_default()) } else { Err(private(path, error(&r.out))) } }
/// FsJob.create's folder half for project.new: make it, then answer its path.
pub async fn make(path: &str) -> Result<String, Value> { let r = sh(&mkdir_sh(), path).await; if r.code == 0 { Ok(lines(&r.out).last().cloned().unwrap_or_default()) } else { Err(private(path, error(&r.out))) } }
/// FsJob.register: a resolved path must be a folder.
pub fn is_dir(path: &str) -> bool { std::fs::metadata(path).map_or(false, |m| m.is_dir()) }

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
