//! M6 KiCad side: a project's design files, the plot actor behind `kicad.watch` / `kicad.info`, and (next) KiCad IPC.
//! `files::start` registers it (the one line main.rs calls); no signal is sent to any process here.
pub mod bj;
pub mod bom;
pub mod cad;
pub mod checks;
pub mod ipc;
pub mod kexport;
pub mod kipc;
pub mod kserve;
pub mod nng;
pub mod proto;
pub mod boards;
pub mod board;
pub mod fl;
pub mod glb;
pub mod mesh;
pub mod pcb3d;
pub mod plot;
pub mod plotgen;
pub mod plotread;
pub mod png;
pub mod render;
pub mod schematic;
pub mod sexpr;
pub mod solid;
pub mod solidread;
pub mod stepglb;
pub mod strokefont;
pub mod strokefont_data;
pub mod project;
pub mod release;
pub mod review;
pub mod watch;
pub mod histsteps;
pub mod open;

use crate::hub::Hub;
use std::sync::Arc;

pub fn start(hub: &Arc<Hub>) {
    let plots = watch::Plots::new(&hub.cfg.home);
    hub.register_rpc(Arc::new(watch::KicadRpc(plots)));
    ipc::start(hub);
}

/// `kicad.watch` names a folder and maybe a file: both must lie under a folder the hub knows (a project's root or a
/// thread's worktree). The JS hub trusts the client's `root`; this does not (a token holder could otherwise have the
/// hub read any file as a board). Versions (`git\t<gitdir>...`, `diff\n...`) must name only known folders too.
pub async fn known_root(hub: &Arc<Hub>, root: &str, path: &str) -> bool {
    let roots: Vec<String> = { let m = hub.model().await; m.projects.values().filter(|p| !p.removed).map(|p| p.root.clone()).chain(m.threads.values().filter(|t| !t.worktree.is_empty()).map(|t| t.worktree.clone())).collect() };
    allowed(&roots, root, path)
}
pub fn allowed(roots: &[String], root: &str, path: &str) -> bool {
    let under = |p: &str, r: &str| !r.is_empty() && (p == r || (p.starts_with(r) && p.as_bytes().get(r.len()) == Some(&b'/'))) && !p.split('/').any(|x| x == "..") && !p.contains('\0');
    let known = |p: &str| roots.iter().any(|r| under(p, r));
    if !known(root) || root.is_empty() { return false }
    if path.is_empty() { return true }
    // every absolute path a version names (git dir, files) must be known; the commit is an opaque word
    let parts: Vec<&str> = path.split(|c| c == '\t' || c == '\n').collect();
    if parts.len() == 1 { return known(path) }
    parts.iter().filter(|p| p.starts_with('/')).all(|p| known(p)) && parts.iter().any(|p| p.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn allowed_roots_and_paths() {
        let roots = vec!["/home/p".to_string(), "/home/p/.backplane/worktrees/w".to_string()];
        assert!(allowed(&roots, "/home/p", "")); assert!(allowed(&roots, "/home/p/sub", "/home/p/sub/b.kicad_pcb")); assert!(allowed(&roots, "/home/p", "git\t/home/p/.backplane/history.git\tabc\t7\t/home/p/b.kicad_pcb"));
        assert!(allowed(&roots, "/home/p", "diff\ngit\t/home/p/.git\ta\t7\t/home/p/b\ngit\t/home/p/.git\tb\t7\t/home/p/b"));
        for (r, p) in [("/etc", ""), ("/home/pp", ""), ("", ""), ("/home/p", "/etc/passwd"), ("/home/p", "/home/p/../../etc/x"), ("/home/p", "/home/pq/b.kicad_pcb"), ("/home/p", "git\t/tmp/evil.git\tabc\t7\t/home/p/b"), ("/home/p/..", ""), ("/home/p", "git\tabc\t7")] { assert!(!allowed(&roots, r, p), "{r} {p}") }
    }
}
