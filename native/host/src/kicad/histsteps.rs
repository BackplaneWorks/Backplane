//! `history.steps {thread}`: the design-history steps of a thread (agents/hist.rs captures them onto `refs/backplane/checkpoints/<b64 thread>/steps`), oldest first, with where the
//! repository is, so a client can name each step's file as a version and watch it or compare two steps.
use crate::{files::git as fgit, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

/// `git log --format='%H %P %ct'` lines (newest first) as steps oldest first: {c: commit, p: parent ("" for the first), t: seconds}.
pub fn parse(out: &str) -> Vec<Value> {
    let mut v: Vec<Value> = out.lines().filter_map(|l| { let f: Vec<&str> = l.split(' ').collect(); let c = *f.first()?; if c.len() < 7 { return None }
        let (p, t) = if f.len() >= 3 { (f[1], f[f.len() - 1]) } else { ("", f.get(1).copied().unwrap_or("0")) };
        Some(json!({"c": c, "p": p, "t": t.parse::<u64>().unwrap_or(0)})) }).collect();
    v.reverse(); v
}

pub async fn steps(hub: &Arc<Hub>, thread: &str) -> Value {
    let Some(cwd) = crate::files::root_of(hub, thread).await else { return json!({"thread": thread, "steps": []}) };
    let cwd = cwd.to_string_lossy().into_owned();
    let out = fgit::refs(&cwd, thread).await; let mut ls = out.split('\n');
    let (gd, top) = (ls.next().unwrap_or("").to_string(), ls.next().unwrap_or("").to_string());
    let f = super::project::read(&cwd);
    if gd.is_empty() || top.is_empty() { return json!({"thread": thread, "steps": [], "git": gd, "top": top, "pcb": f.pcb, "sch": f.sch}) }
    let r = fgit::proc_ro("git", &["--git-dir", &gd, "log", "-n", "200", "--format=%H %P %ct", &format!("refs/backplane/checkpoints/{}/steps", fgit::b64url(thread))], "/", &[]).await;
    json!({"thread": thread, "git": gd, "top": top, "pcb": f.pcb, "sch": f.sch, "steps": if r.code == 0 { parse(&r.out) } else { vec![] }})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn steps_come_oldest_first_with_their_parent() {
        let v = parse("ccccccc bbbbbbb 30\nbbbbbbb aaaaaaa 20\naaaaaaa  10\n");
        assert_eq!(v.len(), 3); assert_eq!(v[0], json!({"c": "aaaaaaa", "p": "", "t": 10})); assert_eq!(v[2], json!({"c": "ccccccc", "p": "bbbbbbb", "t": 30}));
        assert!(parse("").is_empty());
    }
}
