//! The AWS Nitro Sandboxes backend (docs/NITRO-SANDBOXES.md). One EC2 parent instance the person owns, launched with Nitro Enclaves on,
//! runs many sandboxes: each a Firecracker (1.18+) microVM that the Nitro Hypervisor isolates from the parent and from the others.
//! The hub reaches the parent over SSH and drives `bp-nitro` there (scripts/nitro/bp-nitro); AWS itself is only asked to describe, start and stop that
//! one instance. Nothing here creates an AWS resource: `plan` prints the commands that would, with their cost, for the person to run or approve.
//!
//! The rules below are the EC2 guide's (Considerations and limitations, Run Nitro Sandboxes with Firecracker, Attest a Nitro Sandbox), as of 2026-11:
//! up to 5 sandboxes per parent vCPU and 896 per parent; at least 256 MiB each, a multiple of 2 MiB; memory from a pool reserved at boot, each sandbox
//! taking its memory + 2 MiB, + 16 MiB per device in confidential mode; 4 slots, raised in steps of 8 (each step 128 MiB of the pool); without CPU
//! overcommit whole cores from the pool (an even count on two-thread cores); no snapshot, pause or resize; IMDS must be blocked or a sandbox reads the
//! parent's role credentials.
use super::{env_filter, Backend, Lease, Out, Shell, Spec};
use serde_json::{json, Value};
use sha2::{Digest, Sha384};
use std::path::Path;

/// The settings that drive it. Off by default; every key is `sandbox.nitro.*`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    pub on: bool,
    pub region: String,
    /// the person's AWS CLI profile (credentials never come through Backplane: the CLI reads its own files)
    pub profile: String,
    /// the parent instance, `i-...` (made by the person from `plan`)
    pub instance: String,
    /// how to reach it: `ec2-user@host` (a tailnet name, or empty: the instance's public DNS name)
    pub ssh: String,
    /// for `plan` and the cost: the instance type
    pub itype: String,
    pub disk_gib: u32,
    /// seconds with no sandbox before the parent is stopped
    pub idle: u64,
    /// the KMS key whose policy releases secrets only to an attested sandbox
    pub kms: String,
    /// sandboxes in direct mode (the parent reads their memory; no attestation; more throughput) instead of confidential
    pub direct: bool,
}
impl Config {
    pub fn from(get: &dyn Fn(&str) -> Option<String>) -> Config {
        let s = |k: &str, d: &str| get(&format!("sandbox.nitro.{k}")).filter(|v| !v.is_empty()).unwrap_or_else(|| d.to_string());
        Config {
            on: get("sandbox.nitro").as_deref() == Some("on"), region: s("region", "us-east-1"), profile: s("profile", ""), instance: s("instance", ""),
            ssh: s("ssh", ""), itype: s("type", "c7i.4xlarge"), disk_gib: s("disk", "200").parse().unwrap_or(200), idle: s("idle", "900").parse().unwrap_or(900),
            kms: s("kms", ""), direct: s("mode", "confidential") == "direct",
        }
    }
    /// there is a parent to use: an instance id or an SSH destination
    pub fn host_known(&self) -> bool { valid_instance(&self.instance) || !self.ssh.is_empty() }
    /// `aws ...` with the region, the profile and JSON out
    pub fn aws(&self, args: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = ["aws", "--output", "json", "--region", &self.region].map(String::from).to_vec();
        if !self.profile.is_empty() { v.extend(["--profile".to_string(), self.profile.clone()]) }
        v.extend(args.iter().map(|s| s.to_string())); v
    }
}
pub fn valid_instance(id: &str) -> bool { id.len() >= 10 && id.starts_with("i-") && id[2..].chars().all(|c| c.is_ascii_hexdigit()) }

// ---- the parent's pools and what fits ---------------------------------------------------------------------------------------------------------------

/// The parent's capacity, as `bp-nitro status` reports it from /sys/class/misc/nitro_enclaves and lscpu.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pools {
    pub vcpus: u32,
    pub threads_per_core: u32,
    pub slots: u32,
    pub slots_total: u32,
    pub slots_in_use: u32,
    pub mem_free_mib: u64,
    pub cpu_free: u32,
    /// the vCPUs of the sandboxes running now (overcommitted or not)
    pub vcpus_in_use: u32,
}
impl Pools {
    pub fn parse(v: &Value) -> Option<Pools> {
        let n = |k: &str| v.get(k).and_then(Value::as_u64);
        Some(Pools { vcpus: n("vcpus")? as u32, threads_per_core: n("threads_per_core").unwrap_or(2) as u32, slots: n("slots")? as u32, slots_total: n("slots_total")? as u32,
            slots_in_use: n("slots_in_use")? as u32, mem_free_mib: n("mem_free_mib")?, cpu_free: n("cpu_free").unwrap_or(0) as u32, vcpus_in_use: n("vcpus_in_use").unwrap_or(0) as u32 })
    }
}
/// The devices a sandbox gets: vsock, the read-only system disk, the work disk, and a network interface unless it has none.
pub fn devices(s: &Spec) -> u32 { 3 + s.network as u32 }
/// The pool memory one sandbox takes.
pub fn pool_mib(s: &Spec) -> u64 { s.mem_mib as u64 + 2 + if s.confidential { 16 * devices(s) as u64 } else { 0 } }
/// The slot count that holds `n` running sandboxes: 4, then 12, 20, 28...
pub fn slots_for(n: u32) -> u32 { if n <= 4 { 4 } else { 4 + 8 * (n - 4).div_ceil(8) } }
/// The pool to reserve at boot (`nitro_enclaves.mempool=<G>G@0`) for these sandboxes at once, in GiB, with the slot steps they need.
pub fn mempool_gib(specs: &[Spec]) -> u64 {
    let mib: u64 = specs.iter().map(pool_mib).sum::<u64>() + 128 * ((slots_for(specs.len() as u32) - 4) / 8) as u64;
    mib.div_ceil(1024).max(1)
}
/// Can this sandbox start now? Ok(Some(n)) = yes after raising the slots to n; Ok(None) = yes; Err = why not.
pub fn fits(s: &Spec, p: &Pools) -> Result<Option<u32>, String> {
    if s.mem_mib < 256 || s.mem_mib % 2 != 0 { return Err(format!("{} MiB: a sandbox needs at least 256 MiB, in steps of 2", s.mem_mib)) }
    if s.vcpus == 0 || s.vcpus > p.vcpus { return Err(format!("{} vCPUs: the parent has {}", s.vcpus, p.vcpus)) }
    if p.slots_in_use + 1 > (5 * p.vcpus).min(896) { return Err(format!("the parent runs {} sandboxes, its most ({} vCPUs x 5, at most 896)", p.slots_in_use, p.vcpus)) }
    let overcommit = !s.attest;
    if overcommit { if p.vcpus_in_use + s.vcpus > 5 * p.vcpus { return Err(format!("{} + {} vCPUs is more than 5 x the parent's {}", p.vcpus_in_use, s.vcpus, p.vcpus)) } }
    else {
        if p.threads_per_core == 2 && s.vcpus % 2 != 0 { return Err(format!("{} vCPUs: dedicated vCPUs come in whole cores (an even count here)", s.vcpus)) }
        if s.vcpus > p.cpu_free { return Err(format!("{} dedicated vCPUs: {} are free", s.vcpus, p.cpu_free)) }
    }
    let raise = if p.slots_in_use >= p.slots { let n = p.slots + 8; if n > p.slots_total { return Err(format!("all {} sandbox slots are in use", p.slots_total)) } Some(n) } else { None };
    let need = pool_mib(s) + if raise.is_some() { 128 } else { 0 };
    if need > p.mem_free_mib { return Err(format!("{need} MiB of the sandbox pool needed, {} free", p.mem_free_mib)) }
    Ok(raise)
}

// ---- one sandbox on the parent ----------------------------------------------------------------------------------------------------------------------

/// The parent's addresses for sandbox number `idx`: a /30 each in 10.250.0.0/16 (tap, guest).
pub fn addrs(idx: u32) -> (String, String) { let b = idx * 4; let o = |k: u32| format!("10.250.{}.{}", (b + k) >> 8, (b + k) & 255); (o(1), o(2)) }
pub fn tap(idx: u32) -> String { format!("bpn{idx}") }

/// The Firecracker configuration (`firecracker --config-file`) of one sandbox. `dir` is its folder on the parent, `img` the guest images' folder.
pub fn firecracker_config(s: &Spec, idx: u32, dir: &str, img: &str) -> Value {
    let (_, guest) = addrs(idx);
    let boot = if s.attest { json!({"kernel_image_path": format!("{img}/guest.eif")}) }
        else { json!({"kernel_image_path": format!("{img}/vmlinuz"), "initrd_path": format!("{img}/initrd.img"), "boot_args": format!("panic=1 initcall_blacklist=i8042_init bp.ip={guest} bp.net={}", s.network as u8)}) };
    let mut enclave = serde_json::Map::new();
    if !s.attest { enclave.insert("cpu_overcommit".into(), json!(true)); }
    if !s.confidential { enclave.insert("direct_mode".into(), json!(true)); }
    let net = if s.network { json!([{"iface_id": "eth0", "host_dev_name": tap(idx)}]) } else { json!([]) };
    json!({
        "boot-source": boot,
        "machine-config": {"vcpu_count": s.vcpus, "mem_size_mib": s.mem_mib},
        // vda: the system (shared by every sandbox, read-only); vdb: this sandbox's work disk (src/, target/, caches)
        "drives": [{"drive_id": "sys", "path_on_host": format!("{img}/rootfs.ext4"), "is_root_device": false, "is_read_only": true},
                   {"drive_id": "work", "path_on_host": format!("{dir}/work.img"), "is_root_device": false, "is_read_only": false}],
        "network-interfaces": net,
        "vsock": {"guest_cid": 3, "uds_path": format!("{dir}/v.sock")},
        "enclave": Value::Object(enclave),
    })
}

/// The parent's nftables for one sandbox: NAT out through the parent, never the parent's IMDS (its role credentials), never another sandbox,
/// and a cap on its connections so one sandbox cannot fill the conntrack table they share.
pub fn nft_rules(idx: u32, uplink: &str, max_conns: u32) -> String {
    let t = tap(idx);
    // one table per sandbox (`bp_nitro_<idx>`): `down` deletes it whole. Nothing on the parent is a service for a sandbox (input: all refused).
    format!("table ip bp_nitro_{idx} {{\n  set conns {{ type ipv4_addr; size 65535; flags dynamic; }}\n  chain forward {{ type filter hook forward priority filter; policy accept;\n    iifname \"{t}\" ip daddr 169.254.169.254 reject\n    iifname \"{t}\" oifname \"bpn*\" drop\n    iifname \"{t}\" ct state new add @conns {{ ip saddr ct count over {max_conns} }} reject\n  }}\n  chain input {{ type filter hook input priority filter; policy accept;\n    iifname \"{t}\" udp dport 67 accept\n    iifname \"{t}\" reject\n  }}\n  chain postrouting {{ type nat hook postrouting priority srcnat; policy accept;\n    iifname \"{t}\" oifname \"{uplink}\" masquerade\n  }}\n}}\n")
}

// ---- attestation ------------------------------------------------------------------------------------------------------------------------------------

/// PCR3 / PCR4 of a sandbox: SHA-384 of 48 zero bytes and the text (the parent's role ARN, or its instance id).
pub fn pcr_of_text(s: &str) -> String { let mut h = Sha384::new(); h.update([0u8; 48]); h.update(s.as_bytes()); hex::encode(h.finalize()) }
fn is_pcr(s: &str) -> bool { s.len() == 96 && s.chars().all(|c| c.is_ascii_hexdigit()) && s.chars().any(|c| c != '0') }
/// The KMS key policy statement that lets the parent's role decrypt only for a sandbox running the expected image (PCR0) on the expected parent (PCR3).
/// PCR0 is required: PCR3 and PCR4 alone also match a sandbox in debug mode, whose image is not measured.
pub fn kms_statement(role_arn: &str, pcr0: &str) -> Result<Value, String> {
    if !is_pcr(pcr0) { return Err("PCR0 must be the image's SHA-384 (96 hex digits, not zero): without it any image on this parent gets the secrets".into()) }
    if !role_arn.starts_with("arn:aws:iam::") || !role_arn.contains(":role/") { return Err(format!("{role_arn}: not an IAM role ARN")) }
    Ok(json!({"Sid": "BackplaneAttestedNitroSandbox", "Effect": "Allow", "Principal": {"AWS": role_arn}, "Action": ["kms:Decrypt", "kms:GenerateDataKey"], "Resource": "*",
        "Condition": {"StringEqualsIgnoreCase": {"kms:RecipientAttestation:PCR0": pcr0.to_lowercase(), "kms:RecipientAttestation:PCR3": pcr_of_text(role_arn)}}}))
}

// ---- cost -------------------------------------------------------------------------------------------------------------------------------------------

/// On-demand Linux prices, us-east-1, USD per hour (the published rates when this was written: check the EC2 pricing page before asking the person).
pub const PRICES: &[(&str, f64, u32, u32)] = &[
    // type, $/h, vCPUs, GiB
    ("c7i.xlarge", 0.1785, 4, 8), ("c7i.2xlarge", 0.357, 8, 16), ("c7i.4xlarge", 0.714, 16, 32), ("c7i.8xlarge", 1.428, 32, 64),
    ("m7i.xlarge", 0.2016, 4, 16), ("m7i.2xlarge", 0.4032, 8, 32), ("m7i.4xlarge", 0.8064, 16, 64),
    ("c7g.xlarge", 0.145, 4, 8), ("c7g.2xlarge", 0.29, 8, 16), ("c7g.4xlarge", 0.58, 16, 32),
    ("m7g.xlarge", 0.1632, 4, 16), ("m7g.2xlarge", 0.3264, 8, 32), ("m7g.4xlarge", 0.6528, 16, 64),
];
/// gp3, USD per GiB-month (us-east-1)
pub const GP3: f64 = 0.08;
/// What a parent costs: per hour running, per month at `hours_a_day`, and per sandbox-hour when `sandboxes` share it. None for a type not in the table.
pub fn cost(itype: &str, disk_gib: u32, hours_a_day: f64, sandboxes: u32) -> Option<Value> {
    let &(_, h, vcpus, gib) = PRICES.iter().find(|p| p.0 == itype)?;
    let disk = disk_gib as f64 * GP3;
    let month = h * hours_a_day * 30.0 + disk;
    let r = |x: f64| (x * 1000.0).round() / 1000.0;
    Some(json!({"type": itype, "vcpus": vcpus, "gib": gib, "hourly": h, "disk_monthly": r(disk), "monthly": r(month), "hours_a_day": hours_a_day,
        "per_sandbox_hour": r(h / sandboxes.max(1) as f64), "sandboxes": sandboxes, "region": "us-east-1", "note": "on-demand list price; stopped: only the disk"}))
}

// ---- the plan (never run here) ----------------------------------------------------------------------------------------------------------------------

/// The commands that make the parent, for the person to read, price and run (or approve). Backplane does not run them.
pub fn plan(c: &Config) -> Vec<Vec<String>> {
    let tag = "ResourceType=instance,Tags=[{Key=Name,Value=bp-nitro-host},{Key=backplane,Value=nitro-host}]";
    let fam = c.itype.split('.').next().unwrap_or("");
    let arch = if fam.trim_start_matches(|ch: char| ch.is_ascii_alphabetic()).trim_start_matches(|ch: char| ch.is_ascii_digit()).contains('g') { "arm64" } else { "x86_64" }; // c7g, m8gd: Graviton
    vec![c.aws(&["ec2", "run-instances", "--image-id", &format!("resolve:ssm:/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-6.18-{arch}"),
        "--instance-type", &c.itype, "--enclave-options", "Enabled=true", "--metadata-options", "HttpTokens=required,HttpPutResponseHopLimit=1",
        "--block-device-mappings", &format!("DeviceName=/dev/xvda,Ebs={{VolumeSize={},VolumeType=gp3,DeleteOnTermination=true}}", c.disk_gib),
        "--instance-initiated-shutdown-behavior", "stop", "--iam-instance-profile", "Name=bp-nitro-host", "--key-name", "<your key pair>", "--security-group-ids", "<a group allowing 22 from you>",
        "--user-data", "file://scripts/nitro/host-setup.sh", "--tag-specifications", tag])]
}

/// An argument for the parent's command line: base64, and `-` for nothing (ssh joins the words into one line, so an empty one would vanish and shift the rest).
pub fn b(t: String) -> String { use base64::Engine; if t.is_empty() { "-".into() } else { base64::engine::general_purpose::STANDARD.encode(t) } }

// ---- the parent's lifecycle ---------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Host { pub state: String, pub dns: String, pub ip: String, pub itype: String }
fn parse_host(o: &Out) -> Result<Host, String> {
    if o.code != 0 { return Err(format!("aws ec2 describe-instances: {}", o.stderr.trim())) }
    let v: Value = serde_json::from_str(&o.stdout).map_err(|e| format!("aws answered no JSON: {e}"))?;
    let i = v.pointer("/Reservations/0/Instances/0").ok_or("no such instance")?;
    let s = |p: &str| i.pointer(p).and_then(Value::as_str).unwrap_or("").to_string();
    Ok(Host { state: s("/State/Name"), dns: s("/PublicDnsName"), ip: s("/PrivateIpAddress"), itype: s("/InstanceType") })
}

pub struct Nitro<'a> { pub cfg: Config, pub sh: &'a dyn Shell, pub scripts: String }
impl<'a> Nitro<'a> {
    pub fn new(cfg: Config, sh: &'a dyn Shell, scripts: &str) -> Self { Nitro { cfg, sh, scripts: scripts.into() } }
    pub fn describe(&self) -> Result<Host, String> { parse_host(&self.sh.run(&self.cfg.aws(&["ec2", "describe-instances", "--instance-ids", &self.cfg.instance]), &[])) }
    /// The parent, running: started if it was stopped (its disk keeps the images and the warm targets), waited for while it starts.
    pub fn ensure_running(&self) -> Result<Host, String> {
        if !valid_instance(&self.cfg.instance) { return Ok(Host { state: "running".into(), dns: String::new(), ip: String::new(), itype: String::new() }) } // SSH only: the person keeps it on
        for n in 0..60 {
            let h = self.describe()?;
            match h.state.as_str() {
                "running" => return Ok(h),
                "stopped" => { let o = self.sh.run(&self.cfg.aws(&["ec2", "start-instances", "--instance-ids", &self.cfg.instance]), &[]); if o.code != 0 { return Err(format!("could not start {}: {}", self.cfg.instance, o.stderr.trim())) } }
                "pending" | "stopping" => {}
                other => return Err(format!("{} is {other}", self.cfg.instance)),
            }
            if n < 59 { self.sh.pause(5) }
        }
        Err(format!("{} did not come up", self.cfg.instance))
    }
    /// Stop the parent when no sandbox has run on it for `cfg.idle` seconds. Returns whether it asked AWS to stop it.
    pub fn stop_if_idle(&self, running: u32, idle_for: u64) -> Result<bool, String> {
        if running > 0 || idle_for < self.cfg.idle || !valid_instance(&self.cfg.instance) { return Ok(false) }
        let o = self.sh.run(&self.cfg.aws(&["ec2", "stop-instances", "--instance-ids", &self.cfg.instance]), &[]);
        if o.code != 0 { return Err(format!("could not stop {}: {}", self.cfg.instance, o.stderr.trim())) }
        Ok(true)
    }
    fn dest(&self, h: &Host) -> Result<String, String> {
        if !self.cfg.ssh.is_empty() { return Ok(self.cfg.ssh.clone()) }
        if !h.dns.is_empty() { return Ok(format!("ec2-user@{}", h.dns)) }
        Err("no way to reach the parent: set sandbox.nitro.ssh (it has no public name)".into())
    }
    fn ssh(&self, dest: &str, args: &[String]) -> Vec<String> {
        let mut v: Vec<String> = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", "-o", "ServerAliveInterval=15", dest, "sudo", "/usr/local/bin/bp-nitro"].map(String::from).to_vec();
        v.extend(args.iter().cloned()); v
    }
    fn host_json(&self, dest: &str, args: &[String]) -> Result<Value, String> {
        let o = self.sh.run(&self.ssh(dest, args), &[]);
        if o.code != 0 { return Err(format!("bp-nitro {}: {}", args.first().map(String::as_str).unwrap_or(""), o.stderr.trim())) }
        serde_json::from_str(o.stdout.lines().last().unwrap_or("")).map_err(|e| format!("bp-nitro answered no JSON: {e}"))
    }
    fn rsync_env(&self, dest: &str) -> Vec<(String, String)> { vec![("BP_NITRO_SSH".into(), dest.into())] }
}

impl Nitro<'_> {
    /// Give an attested sandbox its secret: `blob` is a KMS ciphertext already on the parent (made with `aws kms encrypt` where the person is; not secret).
    /// The parent passes the sandbox's attestation document to KMS and the answer back; KMS encrypts it to a key only the sandbox holds, and answers at all
    /// only if the document's PCR0 is the image the key policy names. No vault value is copied: the hub never sees the secret either.
    pub fn release_secret(&self, l: &Lease, blob: &str) -> Result<(), String> {
        if self.cfg.kms.is_empty() { return Err("no KMS key: set sandbox.nitro.kms".into()) }
        if blob.is_empty() || blob.contains(char::is_whitespace) || !blob.starts_with('/') { return Err(format!("{blob}: a path on the parent")) }
        let d = l.id.split('#').next().unwrap_or("");
        let o = self.sh.run(&self.ssh(d, &["secret".into(), l.name.clone(), blob.into(), self.cfg.kms.clone()]), &[]);
        if o.code != 0 { return Err(format!("the secret for {}: {}", l.name, o.stderr.trim())) }
        let o = self.sh.run(&self.ssh(d, &["wait".into(), l.name.clone()]), &[]);
        if o.code != 0 { return Err(format!("{} after its secret: {}", l.name, o.stderr.trim())) } Ok(())
    }
}

impl Backend for Nitro<'_> {
    fn name(&self) -> &'static str { "nitro" }
    fn ready(&self) -> Result<(), String> {
        if !self.cfg.on { return Err("Nitro sandboxes are off (setting sandbox.nitro)".into()) }
        if !self.cfg.host_known() { return Err("no parent instance: set sandbox.nitro.instance or sandbox.nitro.ssh".into()) }
        Ok(())
    }
    fn up(&self, s: &Spec) -> Result<Lease, String> {
        self.ready()?;
        let h = self.ensure_running()?; let d = self.dest(&h)?;
        let have = self.host_json(&d, &["list".into()])?;
        if let Some(b) = have.get("sandboxes").and_then(Value::as_array).and_then(|a| a.iter().find(|b| b["name"] == s.name.as_str())) {
            return Ok(Lease { backend: "nitro", name: s.name.clone(), id: format!("{d}#{}", b["idx"]) })
        }
        let p = Pools::parse(&self.host_json(&d, &["status".into()])?).ok_or("bp-nitro status: not the pools")?;
        let raise = fits(s, &p)?;
        // the lowest free number: its tap, its /30, its nft table. The config and the rules are made here, the parent only writes them.
        let taken: Vec<u64> = have.get("sandboxes").and_then(Value::as_array).into_iter().flatten().filter_map(|b| b["idx"].as_u64()).collect();
        let idx = (0u32..).find(|i| !taken.contains(&(*i as u64))).unwrap_or(0);
        let dir = format!("/var/lib/bp-nitro/{}", s.name);
        let nft = if s.network { nft_rules(idx, "@UPLINK@", 4096) } else { String::new() };
        let mut a: Vec<String> = vec!["up".into(), s.name.clone(), idx.to_string(), s.disk_gib.to_string(),
            b(firecracker_config(s, idx, &dir, "/var/lib/bp-nitro/img").to_string()), b(nft)];
        if let Some(n) = raise { a.push(format!("--slots={n}")) }
        if s.attest { a.push("--no-wait".into()) } // it waits for its secret first: release_secret, then it answers
        let r = self.host_json(&d, &a)?;
        Ok(Lease { backend: "nitro", name: s.name.clone(), id: format!("{d}#{}", r["idx"]) })
    }
    fn sync_in(&self, l: &Lease, root: &Path, files: &Path) -> Result<(), String> {
        let d = l.id.split('#').next().unwrap_or("");
        let e = format!("{}/nitro-ssh.sh", self.scripts);
        // the list first (the box prunes by it), then the files it names, by checksum (mtimes are not kept: cargo sees a changed file as new)
        let list = vec!["rsync".into(), "-q".into(), "-e".into(), e.clone(), files.display().to_string(), format!("{}:/work/.bp/list", l.name)];
        let argv = vec!["rsync".into(), "-rlp".into(), "--checksum".into(), "--no-times".into(), "--ignore-missing-args".into(), "--from0".into(), format!("--files-from={}", files.display()),
            "-e".into(), e, format!("{}/", root.display()), format!("{}:/work/src/", l.name)];
        for a in [list, argv] { let o = self.sh.run(&a, &self.rsync_env(d)); if o.code != 0 { return Err(format!("rsync to {}: {}", l.name, o.stderr.trim())) } }
        // what the worktree no longer has goes on the box too (the list is the truth, like scripts/boat-prune.py)
        let o = self.sh.run(&self.ssh(d, &["prune".into(), l.name.clone()]), &[]);
        if o.code != 0 { return Err(format!("prune on {}: {}", l.name, o.stderr.trim())) } Ok(())
    }
    fn run(&self, l: &Lease, cwd: &str, argv: &[String], env: &[(String, String)]) -> Result<i32, String> {
        let d = l.id.split('#').next().unwrap_or("");
        let env = env_filter(env, &[]);
        let a = vec!["run".into(), l.name.clone(), b(cwd.into()), b(argv.join("\0")), b(env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("\0"))];
        match self.sh.stream(&self.ssh(d, &a), &[])? { 255 => Err(format!("lost the connection to the parent of {}", l.name)), rc => Ok(rc) }
    }
    fn pull(&self, l: &Lease, remote: &str, files: &Path, local: &Path) -> Result<(), String> {
        let d = l.id.split('#').next().unwrap_or("");
        let argv = vec!["rsync".into(), "-rlp".into(), "--no-times".into(), format!("--files-from={}", files.display()), "-e".into(), format!("{}/nitro-ssh.sh", self.scripts),
            format!("{}:{remote}/", l.name), format!("{}/", local.display())];
        let o = self.sh.run(&argv, &self.rsync_env(d));
        if o.code != 0 { return Err(format!("rsync from {}: {}", l.name, o.stderr.trim())) } Ok(())
    }
    fn down(&self, l: &Lease) -> Result<(), String> {
        let d = l.id.split('#').next().unwrap_or("");
        let o = self.sh.run(&self.ssh(d, &["down".into(), l.name.clone()]), &[]);
        if o.code != 0 { return Err(format!("bp-nitro down {}: {}", l.name, o.stderr.trim())) } Ok(())
    }
}
