//! M6 KiCad side: a project's design files, the plot actor behind `kicad.watch` / `kicad.info`, and (next) KiCad IPC.
//! `files::start` registers it (the one line main.rs calls); no signal is sent to any process here.
pub mod bj;
pub mod bom;
pub mod cad;
pub mod canonical;
pub mod checks;
pub mod embed;
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
pub mod xprobe;
pub mod reload;
pub mod comment;
pub mod progress;
pub mod findings;
pub mod checkrun;
pub mod histsteps;
pub mod open;
pub mod install;
pub mod mgmt;
pub mod bompanel;

use crate::hub::Hub;
use std::sync::Arc;

pub fn start(hub: &Arc<Hub>) {
    let plots = watch::Plots::new(&hub.cfg.home);
    hub.register_rpc(Arc::new(watch::KicadRpc(plots)));
    ipc::start(hub);
    embed::start(hub);
    xprobe::start(hub);
    reload::start(hub);
    comment::start(hub);
    progress::start(hub);
    checkrun::start(hub);
    mgmt::start(hub);
    bompanel::start(hub);
}

/// `kicad.watch` names a folder and maybe a file: both must lie under a folder the hub knows (a project's root or a
/// thread's worktree). The JS hub trusts the client's `root`; this does not (a token holder could otherwise have the
/// hub read any file as a board). Versions (`git\t<gitdir>...`, `diff\n...`) must name only known folders too.
pub async fn known_root(hub: &Arc<Hub>, root: &str, path: &str) -> bool {
    let roots: Vec<String> = { let m = hub.model().await; m.projects.values().filter(|p| !p.removed).map(|p| p.root.clone()).chain(m.threads.values().filter(|t| !t.worktree.is_empty()).map(|t| t.worktree.clone())).collect() };
    // (a design file in no project the person opened by a link is drawn from the thread on show: present/filelink.rs `allow`)
    allowed(&roots, root, path) || (allowed(&roots, root, "") && crate::present::filelink::allowed(path))
}
pub fn allowed(roots: &[String], root: &str, path: &str) -> bool {
    let is_sep = |b: u8| b == b'/' || b == b'\\';
    let any_abs = |p: &str| {
        let b = p.as_bytes();
        p.starts_with(['/', '\\']) || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && is_sep(b[2]))
    };
    let under = |p: &str, r: &str| {
        let r = r.trim_end_matches(['/', '\\']);
        let p_trim = p.trim_end_matches(['/', '\\']);
        if r.is_empty() || p_trim.len() < r.len() || p.split(['/', '\\']).any(|x| x == "..") || p.contains('\0') { return false }
        let prefix_ok = r.bytes().enumerate().all(|(i, a)| {
            let b = p_trim.as_bytes()[i];
            a == b || (is_sep(a) && is_sep(b)) || (i == 0 && r.as_bytes().get(1) == Some(&b':') && a.eq_ignore_ascii_case(&b))
        });
        prefix_ok && (p_trim.len() == r.len() || is_sep(p_trim.as_bytes()[r.len()]))
    };
    let known = |p: &str| roots.iter().any(|r| under(p, r));
    if !known(root) || root.is_empty() { return false }
    if path.is_empty() { return true }
    // every absolute path a version names (git dir, files) must be known; the commit is an opaque word
    let parts: Vec<&str> = path.split(|c| c == '\t' || c == '\n').collect();
    if parts.len() == 1 { return known(path) }
    parts.iter().filter(|p| any_abs(p)).all(|p| known(p)) && parts.iter().any(|p| any_abs(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn allowed_roots_and_paths() {
        let roots = vec!["/home/p".to_string(), "/home/p/.backplane/worktrees/w".to_string(), r"C:\bp\proj".to_string()];
        assert!(allowed(&roots, "/home/p", "")); assert!(allowed(&roots, "/home/p/sub", "/home/p/sub/b.kicad_pcb")); assert!(allowed(&roots, "/home/p", "git\t/home/p/.backplane/history.git\tabc\t7\t/home/p/b.kicad_pcb"));
        assert!(allowed(&roots, "/home/p", "diff\ngit\t/home/p/.git\ta\t7\t/home/p/b\ngit\t/home/p/.git\tb\t7\t/home/p/b"));
        assert!(allowed(&roots, r"C:\bp\proj\boards\amp", r"C:\bp\proj\boards\amp\amp.kicad_pcb"));
        assert!(allowed(&roots, r"c:\bp\proj\", r"git	C:\bp\proj\.git	abc	7	c:\bp\proj\amp.kicad_pcb"));
        for (r, p) in [("/etc", ""), ("/home/pp", ""), ("", ""), ("/home/p", "/etc/passwd"), ("/home/p", "/home/p/../../etc/x"), ("/home/p", "/home/pq/b.kicad_pcb"), ("/home/p", "git\t/tmp/evil.git\tabc\t7\t/home/p/b"), ("/home/p/..", ""), ("/home/p", "git\tabc\t7"), (r"C:\bp\proj2", ""), (r"C:\bp\proj", r"C:\bp\proj\..\evil.kicad_pcb")] { assert!(!allowed(&roots, r, p), "{r} {p}") }
    }
}
