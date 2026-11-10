//! The Boat backend: the five steps of scripts/lib-boat.sh (bo_box, bo_sync, bo_run, bo_pull, bo_finish) through the `boat` CLI, behind the same
//! trait as Nitro. The build offload itself stays in the scripts; this is how the hub would use a Boat box for the same jobs.
use super::{env_filter, Backend, Lease, Shell, Spec};
use serde_json::Value;
use std::path::Path;

pub struct Boat<'a> { pub sh: &'a dyn Shell, pub scripts: String, pub base: String, pub size: String }
impl<'a> Boat<'a> {
    pub fn new(sh: &'a dyn Shell, scripts: &str) -> Self { Boat { sh, scripts: scripts.into(), base: "bp-build-base".into(), size: "large".into() } }
    fn b(&self, a: &[&str]) -> Vec<String> { let mut v = vec!["boat".to_string(), "--no-update".into()]; v.extend(a.iter().map(|s| s.to_string())); v }
    /// the box of this name: (id, state), a running one before a stopped one
    fn find(&self, name: &str) -> Result<Option<(String, String)>, String> {
        let o = self.sh.run(&self.b(&["list", "--all", "--json"]), &[]);
        if o.code != 0 { return Err("cannot list sandboxes".into()) }
        let v: Value = serde_json::from_str(&o.stdout).map_err(|_| "boat list: no JSON")?;
        let mut s: Vec<(String, String)> = v["sandboxes"].as_array().into_iter().flatten()
            .filter(|x| x["name"] == name && !matches!(x["state"].as_str(), Some("deleted" | "deleting")))
            .map(|x| (x["id"].as_str().unwrap_or("").to_string(), x["state"].as_str().unwrap_or("").to_string())).collect();
        s.sort_by_key(|(_, st)| matches!(st.as_str(), "stopped" | "archived"));
        Ok(s.into_iter().next())
    }
}
impl Backend for Boat<'_> {
    fn name(&self) -> &'static str { "boat" }
    fn ready(&self) -> Result<(), String> { if self.sh.run(&self.b(&["--version"]), &[]).code == 0 { Ok(()) } else { Err("no boat CLI".into()) } }
    fn up(&self, s: &Spec) -> Result<Lease, String> {
        for _ in 0..40 {
            match self.find(&s.name)? {
                Some((id, st)) if matches!(st.as_str(), "ready" | "idle" | "running" | "provisioned") => return Ok(Lease { backend: "boat", name: s.name.clone(), id }),
                Some((id, st)) if matches!(st.as_str(), "stopped" | "archived") => { if self.sh.run(&self.b(&["resume", &id]), &[]).code != 0 { return Err(format!("resume of {id} failed")) } }
                Some(_) => self.sh.pause(3),
                None => {
                    // --no-env: the box gets none of the person's logins or tokens
                    let o = self.sh.run(&self.b(&["new", "--from", &self.base, "--name", &s.name, "--environment", "bp-build", "--no-env", "--type", &self.size, "--ttl", "1800", "--json"]), &[]);
                    if !o.stdout.contains("\"ready\"") { return Err(format!("cannot create {}: {}", s.name, o.stderr.trim())) }
                }
            }
        }
        Err(format!("{} never became ready", s.name))
    }
    fn sync_in(&self, l: &Lease, root: &Path, files: &Path) -> Result<(), String> {
        let a = vec!["rsync".into(), "-rlp".into(), "--checksum".into(), "--no-times".into(), "--ignore-missing-args".into(), "--from0".into(), format!("--files-from={}", files.display()),
            "-e".into(), format!("{}/boat-ssh.sh", self.scripts), format!("{}/", root.display()), format!("{}:/home/user/ws/src/", l.id)];
        let o = self.sh.run(&a, &[]); if o.code != 0 { return Err(format!("rsync to {}: {}", l.id, o.stderr.trim())) }
        let a = vec!["rsync".into(), "-q".into(), "-e".into(), format!("{}/boat-ssh.sh", self.scripts), files.display().to_string(), format!("{}:/home/user/.bpjobs/list", l.id)];
        let o = self.sh.run(&a, &[]); if o.code != 0 { return Err(format!("rsync to {}: {}", l.id, o.stderr.trim())) }
        let o = self.sh.run(&self.b(&["exec", &l.id, "--timeout", "120", "python3 -I ~/ws/src/scripts/boat-prune.py ~/ws/src ~/.bpjobs/list"]), &[]);
        if o.code != 0 { return Err(format!("prune on {}: {}", l.id, o.stderr.trim())) } Ok(())
    }
    fn run(&self, l: &Lease, cwd: &str, argv: &[String], env: &[(String, String)]) -> Result<i32, String> {
        let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        let mut line = String::from("export BP_NO_BOAT=1 BP_ON_BOAT=1;");
        for (k, v) in env_filter(env, &[]) { line += &format!(" export {k}={};", q(&v)) }
        line += &format!(" cd ~/ws/src/{} || exit 96; exec", q(cwd));
        for a in argv { line += " "; line += &q(a) }
        match self.sh.stream(&["python3".into(), "-I".into(), format!("{}/boat-run.py", self.scripts), l.id.clone(), "bash".into(), "-c".into(), q(&line)], &[])? { 97 => Err(format!("lost the connection to {}", l.id)), rc => Ok(rc) }
    }
    fn pull(&self, l: &Lease, remote: &str, files: &Path, local: &Path) -> Result<(), String> {
        let a = vec!["rsync".into(), "-rlp".into(), "--no-times".into(), format!("--files-from={}", files.display()), "-e".into(), format!("{}/boat-ssh.sh", self.scripts), format!("{}:{remote}/", l.id), format!("{}/", local.display())];
        let o = self.sh.run(&a, &[]); if o.code != 0 { return Err(format!("rsync from {}: {}", l.id, o.stderr.trim())) } Ok(())
    }
    fn down(&self, l: &Lease) -> Result<(), String> { let o = self.sh.run(&self.b(&["stop", &l.id]), &[]); if o.code != 0 { return Err(format!("stop {}: {}", l.id, o.stderr.trim())) } Ok(()) }
}
