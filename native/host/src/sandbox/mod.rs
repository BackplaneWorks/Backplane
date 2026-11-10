//! Sandbox backends: where a job (a build, a risky command, later an agent thread) runs when it does not run here.
//! `local` is this machine; `boat` is a Boat sandbox (the build offload of scripts/lib-boat.sh, the same steps through the `boat` CLI);
//! `nitro` is an AWS Nitro Sandbox, a Firecracker microVM on one shared EC2 parent instance the person owns (nitro.rs, docs/NITRO-SANDBOXES.md).
//! Every backend does the same five steps: `up` a box, `sync_in` the exact worktree, `run` a command with its exit code, `pull` artifacts, `down`.
//! Nitro is off unless the setting `sandbox.nitro` is `on`; nothing here ever creates an AWS resource (the host is planned, the person makes it).
pub mod boat;
pub mod nitro;
pub mod rpc;

use std::path::Path;

/// What a sandbox is for: the order of backends and the defaults differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use { Build, Thread, Hub }

/// The size and shape of one sandbox.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    /// a stable name (`bp-build-0`, `bp-thread-t17...`): the same name finds the same box again
    pub name: String,
    pub vcpus: u32,
    pub mem_mib: u32,
    /// the work disk, GiB (sparse on the host)
    pub disk_gib: u32,
    /// a network device at all (no device = no network, like EKS's network-isolated pods)
    pub network: bool,
    /// Nitro: confidential (memory taken from the parent, NSM attestation) or direct (the parent can read it; no attestation)
    pub confidential: bool,
    /// Nitro: boot the prebuilt, measured image so KMS can release secrets to it (no CPU overcommit: the documented attested path)
    pub attest: bool,
}
impl Spec {
    pub fn build(i: u32) -> Spec { Spec { name: format!("bp-build-{i}"), vcpus: 8, mem_mib: 16384, disk_gib: 60, network: true, confidential: false, attest: false } }
    pub fn thread(id: &str) -> Spec { Spec { name: format!("bp-thread-{}", slug(id)), vcpus: 2, mem_mib: 4096, disk_gib: 20, network: true, confidential: true, attest: true } }
}
/// A name a host path and a tap device can carry: lowercase letters, digits and `-`, at most 40.
pub fn slug(s: &str) -> String { s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).take(40).collect() }

/// A running box a backend handed out.
#[derive(Clone, Debug, PartialEq)]
pub struct Lease { pub backend: &'static str, pub name: String, pub id: String }

/// What a command printed and how it ended.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Out { pub code: i32, pub stdout: String, pub stderr: String }

/// The commands a backend runs here (aws, ssh, rsync, boat). The real one goes through proc.rs; tests give a fake that answers like AWS.
pub trait Shell: Send + Sync {
    /// run to the end, output captured
    fn run(&self, argv: &[String], env: &[(String, String)]) -> Out;
    /// run with this process's stdout and stderr (a build's output), the exit code back
    fn stream(&self, argv: &[String], env: &[(String, String)]) -> Result<i32, String>;
    /// wait between polls (a fake does not)
    fn pause(&self, _secs: u64) {}
}

/// The real shell: every program is a network command (it reaches AWS, the host, a Boat).
pub struct Real;
impl Shell for Real {
    fn run(&self, argv: &[String], env: &[(String, String)]) -> Out {
        let Some((p, a)) = argv.split_first() else { return Out { code: 127, ..Default::default() } };
        let mut c = crate::proc::std_cmd(crate::proc::Kind::Net, p); c.args(a).envs(env.iter().map(|(k, v)| (k, v))).stdin(std::process::Stdio::null());
        match c.output() {
            Ok(o) => Out { code: o.status.code().unwrap_or(128), stdout: String::from_utf8_lossy(&o.stdout).into(), stderr: String::from_utf8_lossy(&o.stderr).into() },
            Err(e) => Out { code: 127, stdout: String::new(), stderr: format!("{p}: {e}") },
        }
    }
    fn stream(&self, argv: &[String], env: &[(String, String)]) -> Result<i32, String> {
        let Some((p, a)) = argv.split_first() else { return Err("no command".into()) };
        let mut c = crate::proc::std_cmd(crate::proc::Kind::Net, p); c.args(a).envs(env.iter().map(|(k, v)| (k, v))).stdin(std::process::Stdio::null());
        c.status().map(|s| s.code().unwrap_or(128)).map_err(|e| format!("{p}: {e}"))
    }
    fn pause(&self, secs: u64) { std::thread::sleep(std::time::Duration::from_secs(secs)) }
}

/// The five steps every backend does.
pub trait Backend {
    fn name(&self) -> &'static str;
    /// usable now (configured, tools present), or why not
    fn ready(&self) -> Result<(), String>;
    fn up(&self, spec: &Spec) -> Result<Lease, String>;
    /// `files`: the worktree's file list (`git ls-files -z --cached --others --exclude-standard`), relative to `root`; lands in the box's `src/`
    fn sync_in(&self, lease: &Lease, root: &Path, files: &Path) -> Result<(), String>;
    /// run ARGV in `src/<cwd>` with only the allowed environment; the command's own exit code
    fn run(&self, lease: &Lease, cwd: &str, argv: &[String], env: &[(String, String)]) -> Result<i32, String>;
    /// copy the files listed in `files` (relative to `remote`) into `local`
    fn pull(&self, lease: &Lease, remote: &str, files: &Path, local: &Path) -> Result<(), String>;
    fn down(&self, lease: &Lease) -> Result<(), String>;
}

/// A variable that may go into a box: never one that looks like a credential, whatever else allows it (the rule of bo_env in scripts/lib-boat.sh).
pub fn env_allowed(name: &str, extra: &[&str]) -> bool {
    const NEVER: &[&str] = &["TOKEN", "KEY", "SECRET", "PASS", "CRED", "AUTH", "COOKIE", "SESSION"];
    if NEVER.iter().any(|w| name.contains(w)) { return false }
    const KNOBS: &[&str] = &["RUST_BACKTRACE", "RUST_LOG", "RUST_TEST_THREADS", "RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_INCREMENTAL", "CARGO_BUILD_JOBS", "BACKPLANE_COMMIT", "PROFILE", "GPUI_FEATURES", "NO_SCCACHE", "JOBS", "BPT_KEEP"];
    KNOBS.contains(&name) || ["CARGO_TERM_", "CARGO_PROFILE_", "BP_TEST_"].iter().any(|p| name.starts_with(p)) || extra.contains(&name)
}
pub fn env_filter(env: &[(String, String)], extra: &[&str]) -> Vec<(String, String)> { env.iter().filter(|(k, _)| env_allowed(k, extra)).cloned().collect() }

/// The backends to try for a use, in order, from the settings. `local` is always first; Nitro only when switched on and given a host
/// (and, for agent threads and the hub, only with its own switch: those carry the person's work, a build carries only sources).
pub fn order(get: &dyn Fn(&str) -> Option<String>, u: Use, boat_cli: bool) -> Vec<&'static str> {
    let on = |k: &str| get(k).as_deref() == Some("on");
    let mut v = vec!["local"];
    if u == Use::Build && boat_cli && get("sandbox.boat").as_deref() != Some("off") { v.push("boat") }
    let nitro = on("sandbox.nitro") && nitro::Config::from(get).host_known();
    let nitro_use = match u { Use::Build => true, Use::Thread => on("sandbox.nitro.threads"), Use::Hub => on("sandbox.nitro.hub") };
    if nitro && nitro_use { v.push("nitro") }
    v
}

#[cfg(test)]
pub mod fake;
#[cfg(test)]
mod tests;
