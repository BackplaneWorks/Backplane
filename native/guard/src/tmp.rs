//! Test-only temporary folders (`crate::tmp`), removed on drop (panic unwind included) and redirected off `/tmp` onto
//! `BP_TMP` / `~/.cache/bp-tmp` on Unix when `TMPDIR` was unset or `/tmp`.
#![allow(dead_code)]

use std::{
    ffi::OsStr,
    fmt, fs,
    ops::Deref,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

static SEQ: AtomicU64 = AtomicU64::new(0);

pub fn base() -> PathBuf {
    let t = std::env::temp_dir();
    #[cfg(unix)]
    if t == Path::new("/tmp") || t.starts_with("/tmp/") {
        let alt = std::env::var_os("BP_TMP")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .filter(|p| p != Path::new("/tmp") && !p.starts_with("/tmp/"))
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|v| !v.is_empty())
                    .map(|h| PathBuf::from(h).join(".cache").join("bp-tmp"))
            });
        if let Some(d) = alt {
            let _ = fs::create_dir_all(&d);
            return d;
        }
    }
    t
}

fn rm_tree(p: &Path) {
    if p.as_os_str().is_empty() {
        return;
    }
    let _ = fs::remove_file(p);
    let _ = fs::remove_dir_all(p);
}

#[must_use = "dropping TempDir removes the folder at once; bind it to a named variable"]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn to_path_buf(&self) -> PathBuf { self.path.clone() }
}

impl Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path { &self.path }
}
impl AsRef<Path> for TempDir { fn as_ref(&self) -> &Path { &self.path } }
impl AsRef<OsStr> for TempDir { fn as_ref(&self) -> &OsStr { self.path.as_os_str() } }
impl fmt::Debug for TempDir { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.path.fmt(f) } }
impl fmt::Display for TempDir { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.path.display().fmt(f) } }

impl Drop for TempDir {
    fn drop(&mut self) {
        rm_tree(&self.path);
    }
}

pub fn dir(tag: &str) -> TempDir {
    let p = base().join(format!("bp-{tag}-{}-{}", std::process::id(), SEQ.fetch_add(1, Relaxed)));
    rm_tree(&p);
    fs::create_dir_all(&p).unwrap();
    TempDir { path: p }
}
