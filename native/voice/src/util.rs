//! Finding programs on PATH.

use std::env;
use std::path::{Path, PathBuf};

fn runnable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// `which`: the first NAME on PATH that can run.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for d in env::split_paths(&path) {
        let p = d.join(name);
        if runnable(&p) {
            return Some(p);
        }
        #[cfg(windows)]
        {
            let p = d.join(format!("{name}.exe"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}
