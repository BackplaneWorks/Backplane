//! A fake AWS, parent host and Boat for the tests: it answers the commands the backends send the way the real ones do, keeps the state they change
//! (an instance that starts and stops, the sandboxes on the parent, its pools) and records every command so a test can say what was and was not sent.
use super::{Out, Shell};
use serde_json::json;
use std::sync::Mutex;

pub struct Fake { pub st: Mutex<St> }
#[derive(Default)]
pub struct St {
    pub calls: Vec<(Vec<String>, Vec<(String, String)>)>,
    /// the EC2 instance: stopped, pending, running, terminated...
    pub ec2: String,
    /// polls left before a pending instance is running
    pub pending_polls: u32,
    /// the parent's sandboxes: (name, idx, vcpus)
    pub boxes: Vec<(String, u32, u32)>,
    pub slots: u32, pub slots_total: u32, pub mem_free: u64, pub vcpus: u32, pub cpu_free: u32,
    /// the exit code `bp-nitro run` gives
    pub run_rc: i32,
    /// every AWS call fails with this (no credentials, no permission)
    pub aws_err: Option<String>,
    /// the Boat sandboxes: (name, id, state)
    pub boat: Vec<(String, String, String)>,
    /// the nft rules each `up` brought
    pub nft: Vec<String>,
}
impl Fake {
    pub fn new(ec2: &str) -> Fake { Fake { st: Mutex::new(St { ec2: ec2.into(), pending_polls: 2, slots: 4, slots_total: 52, mem_free: 60 * 1024, vcpus: 16, cpu_free: 14, ..Default::default() }) } }
    pub fn calls(&self) -> Vec<Vec<String>> { self.st.lock().unwrap().calls.iter().map(|c| c.0.clone()).collect() }
    pub fn lines(&self) -> Vec<String> { self.calls().iter().map(|c| c.join(" ")).collect() }
    pub fn env_of(&self, i: usize) -> Vec<(String, String)> { self.st.lock().unwrap().calls[i].1.clone() }
    fn ok(s: String) -> Out { Out { code: 0, stdout: s, stderr: String::new() } }
    fn err(code: i32, e: &str) -> Out { Out { code, stdout: String::new(), stderr: e.into() } }
    fn aws(st: &mut St, a: &[String]) -> Out {
        if let Some(e) = &st.aws_err { return Fake::err(254, e) }
        let op = a.iter().position(|x| x == "ec2").and_then(|i| a.get(i + 1)).cloned().unwrap_or_default();
        match op.as_str() {
            "describe-instances" => {
                if st.ec2 == "pending" { if st.pending_polls == 0 { st.ec2 = "running".into() } else { st.pending_polls -= 1 } }
                let dns = if st.ec2 == "running" { "ec2-3-1-2-3.compute-1.amazonaws.com" } else { "" };
                Fake::ok(json!({"Reservations":[{"Instances":[{"InstanceId":"i-0123456789abcdef0","InstanceType":"c7i.4xlarge","State":{"Name":st.ec2},"PublicDnsName":dns,"PrivateIpAddress":"172.31.5.6"}]}]}).to_string())
            }
            "start-instances" => { st.ec2 = "pending".into(); Fake::ok("{}".into()) }
            "stop-instances" => { st.ec2 = "stopping".into(); Fake::ok("{}".into()) }
            // the backends never make or delete anything; if they did, the test sees it in the calls
            _ => Fake::err(252, &format!("fake aws: {op} is not something Backplane may call")),
        }
    }
    fn host(st: &mut St, a: &[String]) -> Out {
        let i = a.iter().position(|x| x == "/usr/local/bin/bp-nitro").map(|i| i + 1).unwrap_or(a.len());
        let (cmd, rest) = (a.get(i).cloned().unwrap_or_default(), &a[(i + 1).min(a.len())..]);
        match cmd.as_str() {
            "list" => Fake::ok(json!({"sandboxes": st.boxes.iter().map(|(n, i, v)| json!({"name":n,"idx":i,"vcpus":v})).collect::<Vec<_>>()}).to_string()),
            "status" => Fake::ok(json!({"vcpus":st.vcpus,"threads_per_core":2,"slots":st.slots,"slots_total":st.slots_total,"slots_in_use":st.boxes.len(),
                "mem_free_mib":st.mem_free,"cpu_free":st.cpu_free,"vcpus_in_use":st.boxes.iter().map(|b| b.2).sum::<u32>()}).to_string()),
            "up" => {
                if st.boxes.len() as u32 >= st.slots && !rest.iter().any(|x| x.starts_with("--slots=")) { return Fake::err(1, "No space left on device") }
                if let Some(n) = rest.iter().find_map(|x| x.strip_prefix("--slots=")) { st.slots = n.parse().unwrap(); st.mem_free -= 128 }
                use base64::Engine; let cfg: serde_json::Value = serde_json::from_slice(&base64::engine::general_purpose::STANDARD.decode(&rest[3]).unwrap()).unwrap();
                let idx: u32 = rest[1].parse().unwrap(); assert!(!st.boxes.iter().any(|b| b.1 == idx), "idx {idx} is taken");
                let (v, m) = (cfg["machine-config"]["vcpu_count"].as_u64().unwrap() as u32, cfg["machine-config"]["mem_size_mib"].as_u64().unwrap());
                st.nft.push(if rest[4] == "-" { String::new() } else { String::from_utf8(base64::engine::general_purpose::STANDARD.decode(&rest[4]).unwrap()).unwrap() });
                st.boxes.push((rest[0].clone(), idx, v)); st.mem_free -= m + 2;
                Fake::ok(format!("{{\"name\":\"{}\",\"idx\":{idx}}}", rest[0]))
            }
            "down" => { st.boxes.retain(|b| b.0 != rest[0]); Fake::ok("{}".into()) }
            "prune" => Fake::ok("{}".into()),
            "secret" | "wait" => Fake::ok("{}".into()),
            "run" => Out { code: st.run_rc, ..Default::default() },
            _ => Fake::err(2, "bp-nitro: unknown command"),
        }
    }
    fn boat(st: &mut St, a: &[String]) -> Out {
        match a.get(2).map(String::as_str) {
            Some("--version") => Fake::ok("boat 1.0".into()),
            Some("list") => Fake::ok(json!({"sandboxes": st.boat.iter().map(|(n, i, s)| json!({"name":n,"id":i,"state":s})).collect::<Vec<_>>()}).to_string()),
            Some("new") => { let n = a[a.iter().position(|x| x == "--name").unwrap() + 1].clone(); st.boat.push((n, "bx_new".into(), "ready".into())); Fake::ok("{\"state\":\"ready\"}".into()) }
            Some("resume") => { for b in st.boat.iter_mut() { if b.1 == a[3] { b.2 = "ready".into() } } Fake::ok(String::new()) }
            Some("stop") => { for b in st.boat.iter_mut() { if b.1 == a[3] { b.2 = "stopped".into() } } Fake::ok(String::new()) }
            Some("exec") => Fake::ok(String::new()),
            _ => Fake::err(2, "boat: unknown"),
        }
    }
}
impl Shell for Fake {
    fn run(&self, argv: &[String], env: &[(String, String)]) -> Out {
        let mut st = self.st.lock().unwrap(); st.calls.push((argv.to_vec(), env.to_vec()));
        match argv.first().map(String::as_str) {
            Some("aws") => Fake::aws(&mut st, argv),
            Some("ssh") => Fake::host(&mut st, argv),
            Some("boat") => Fake::boat(&mut st, argv),
            Some("rsync") => Fake::ok(String::new()),
            _ => Fake::err(127, "not found"),
        }
    }
    fn stream(&self, argv: &[String], env: &[(String, String)]) -> Result<i32, String> {
        if argv.first().map(String::as_str) == Some("python3") { self.st.lock().unwrap().calls.push((argv.to_vec(), env.to_vec())); return Ok(self.st.lock().unwrap().run_rc) }
        Ok(self.run(argv, env).code)
    }
}
