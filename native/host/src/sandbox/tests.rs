use super::fake::Fake;
use super::nitro::{self, Config, Nitro, Pools};
use super::*;
use serde_json::json;
use std::collections::HashMap;

fn settings(kv: &[(&str, &str)]) -> HashMap<String, String> { kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
fn cfg(kv: &[(&str, &str)]) -> Config { let m = settings(kv); Config::from(&|k| m.get(k).cloned()) }
fn on() -> Config { cfg(&[("sandbox.nitro", "on"), ("sandbox.nitro.instance", "i-0123456789abcdef0"), ("sandbox.nitro.profile", "bp")]) }
fn small(name: &str) -> Spec { Spec { name: name.into(), vcpus: 2, mem_mib: 2048, disk_gib: 10, network: true, confidential: true, attest: false } }

#[test] fn nitro_is_off_unless_switched_on_and_given_a_parent() {
    let none = settings(&[]); let get = |k: &str| none.get(k).cloned();
    assert_eq!(order(&get, Use::Build, true), ["local", "boat"], "off by default");
    assert_eq!(order(&get, Use::Build, false), ["local"]);
    let m = settings(&[("sandbox.nitro", "on")]); assert_eq!(order(&|k| m.get(k).cloned(), Use::Build, false), ["local"], "on, but no parent: nothing to use");
    let m = settings(&[("sandbox.nitro", "on"), ("sandbox.nitro.instance", "i-0123456789abcdef0")]); let g = |k: &str| m.get(k).cloned();
    assert_eq!(order(&g, Use::Build, true), ["local", "boat", "nitro"]);
    assert_eq!(order(&g, Use::Thread, true), ["local"], "agent threads carry the person's work: their own switch");
    let m = settings(&[("sandbox.nitro", "on"), ("sandbox.nitro.ssh", "ec2-user@nitro.tail.ts.net"), ("sandbox.nitro.threads", "on"), ("sandbox.boat", "off")]);
    assert_eq!(order(&|k| m.get(k).cloned(), Use::Thread, true), ["local", "nitro"]);
    assert_eq!(order(&|k| m.get(k).cloned(), Use::Build, true), ["local", "nitro"], "sandbox.boat off leaves Boat out");
    let sh = Fake::new("running"); assert!(Nitro::new(cfg(&[]), &sh, "s").ready().unwrap_err().contains("off"));
    assert!(sh.calls().is_empty(), "off: AWS is never asked anything");
}

#[test] fn credentials_never_go_into_a_box() {
    for k in ["AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN", "GH_TOKEN", "ANTHROPIC_API_KEY", "CARGO_REGISTRY_TOKEN", "BP_TEST_PASSWORD", "MY_CREDS", "HTTP_AUTHORIZATION"] {
        assert!(!env_allowed(k, &[k]), "{k}");
    }
    for k in ["RUSTFLAGS", "CARGO_PROFILE_FAST_LTO", "BP_TEST_FOO", "PROFILE"] { assert!(env_allowed(k, &[]), "{k}") }
    assert!(!env_allowed("HOME", &[]) && env_allowed("FOO", &["FOO"]));
}

#[test] fn the_limits_are_the_ec2_guides() {
    let p = Pools { vcpus: 16, threads_per_core: 2, slots: 4, slots_total: 52, slots_in_use: 0, mem_free_mib: 32768, cpu_free: 14, vcpus_in_use: 0 };
    assert_eq!(nitro::fits(&small("a"), &p), Ok(None));
    let mut s = small("a"); s.mem_mib = 255; assert!(nitro::fits(&s, &p).unwrap_err().contains("256"));
    s.mem_mib = 257; assert!(nitro::fits(&s, &p).is_err(), "a multiple of 2 MiB");
    s = small("a"); s.vcpus = 17; assert!(nitro::fits(&s, &p).is_err(), "no more vCPUs than the parent");
    // overcommit: 5 x the parent's vCPUs in all
    let full = Pools { vcpus_in_use: 79, ..p.clone() }; assert!(nitro::fits(&small("a"), &full).unwrap_err().contains("5 x"));
    // attested sandboxes take whole cores: even on 2-thread cores, and only what is free
    let mut a = small("a"); a.attest = true; a.vcpus = 3; assert!(nitro::fits(&a, &p).unwrap_err().contains("even"));
    a.vcpus = 16; assert!(nitro::fits(&a, &p).unwrap_err().contains("free"));
    // slots: full at 4 → raise to 12, which takes 128 MiB of the pool; no more than the total
    let busy = Pools { slots_in_use: 4, ..p.clone() }; assert_eq!(nitro::fits(&small("a"), &busy), Ok(Some(12)));
    let top = Pools { slots: 52, slots_in_use: 52, ..p.clone() }; assert!(nitro::fits(&small("a"), &top).unwrap_err().contains("slots"));
    let tight = Pools { slots_in_use: 4, mem_free_mib: 2048 + 2 + 64 + 127, ..p.clone() }; assert!(nitro::fits(&small("a"), &tight).unwrap_err().contains("pool"));
    // 5 per vCPU, 896 per parent
    let many = Pools { vcpus: 2, slots_in_use: 10, slots: 12, ..p.clone() }; assert!(nitro::fits(&Spec { vcpus: 1, ..small("a") }, &many).is_err());
    assert_eq!([1, 4, 5, 12, 13, 20, 21].map(nitro::slots_for), [4, 4, 12, 12, 20, 20, 28]);
    // the memory a sandbox takes: its own + 2 MiB, + 16 MiB per device when confidential (4 devices with a network, 3 without)
    assert_eq!(nitro::pool_mib(&small("a")), 2048 + 2 + 64);
    assert_eq!(nitro::pool_mib(&Spec { confidential: false, ..small("a") }), 2050);
    assert_eq!(nitro::pool_mib(&Spec { network: false, ..small("a") }), 2048 + 2 + 48);
    // four builds of 16 GiB → a 65 GiB pool (4 x (16384 + 2 + 64)), five need the first slot step too
    let b: Vec<Spec> = (0..4).map(Spec::build).map(|s| Spec { confidential: true, ..s }).collect(); assert_eq!(nitro::mempool_gib(&b), 65);
}

#[test] fn the_firecracker_config_is_a_nitro_sandbox() {
    let c = nitro::firecracker_config(&small("bp-build-0"), 3, "/var/lib/bp-nitro/bp-build-0", "/var/lib/bp-nitro/img");
    assert_eq!(c["enclave"], json!({"cpu_overcommit": true}), "confidential is the default: an enclave object without direct_mode");
    assert_eq!(c["machine-config"], json!({"vcpu_count": 2, "mem_size_mib": 2048}));
    assert!(c["drives"].is_array() && c["drives"][0]["is_read_only"] == true && c["drives"][1]["path_on_host"] == "/var/lib/bp-nitro/bp-build-0/work.img");
    assert_eq!(c["network-interfaces"][0]["host_dev_name"], "bpn3");
    assert_eq!(c["vsock"], json!({"guest_cid": 3, "uds_path": "/var/lib/bp-nitro/bp-build-0/v.sock"}));
    assert!(c["boot-source"]["boot_args"].as_str().unwrap().contains("bp.ip=10.250.0.14"));
    assert!(c["enclave"].get("debug_mode").is_none(), "never debug mode: it zeroes the image measurements");
    let a = nitro::firecracker_config(&Spec { attest: true, network: false, ..small("t") }, 0, "/d", "/i");
    assert_eq!(a["boot-source"], json!({"kernel_image_path": "/i/guest.eif"}), "the measured image, built ahead");
    assert_eq!(a["enclave"], json!({}), "attested: no overcommit (the image's flags must match)");
    assert_eq!(a["network-interfaces"], json!([]), "no network: no device at all");
    let d = nitro::firecracker_config(&Spec { confidential: false, ..small("d") }, 0, "/d", "/i"); assert_eq!(d["enclave"]["direct_mode"], true);
    assert_eq!(nitro::addrs(0), ("10.250.0.1".to_string(), "10.250.0.2".to_string()));
    assert_eq!(nitro::addrs(64), ("10.250.1.1".to_string(), "10.250.1.2".to_string()));
}

#[test] fn the_parents_imds_and_the_other_sandboxes_are_out_of_reach() {
    let r = nitro::nft_rules(5, "ens5", 4096);
    assert!(r.contains("iifname \"bpn5\" ip daddr 169.254.169.254 reject"), "the role credentials of the parent");
    assert!(r.contains("oifname \"bpn*\" drop"), "sandbox to sandbox");
    assert!(r.contains("ct count over 4096"), "one sandbox cannot fill the shared conntrack table");
    assert!(r.contains("table ip bp_nitro_5") && r.contains("oifname \"ens5\" masquerade") && r.contains("iifname \"bpn5\" reject\n  }"), "nothing on the parent answers a sandbox");
}

#[test] fn attestation_needs_the_image_measurement() {
    let role = "arn:aws:iam::111122223333:role/bp-nitro-host";
    // the EC2 guide's PCR3 rule: sha384(48 zero bytes ‖ the ARN)
    assert_eq!(nitro::pcr_of_text(role).len(), 96);
    assert_eq!(nitro::pcr_of_text("i-1234567890abcdef0"), { use sha2::Digest; let mut h = sha2::Sha384::new(); h.update([0u8; 48]); h.update(b"i-1234567890abcdef0"); hex::encode(h.finalize()) });
    assert!(nitro::kms_statement(role, "").is_err(), "PCR3 alone matches a debug-mode sandbox");
    assert!(nitro::kms_statement(role, &"0".repeat(96)).is_err(), "zero is what debug mode reads");
    let pcr0 = "294af39e266a15be97de5d99b210f81fb9147044d9eb456ad808eeb8b2c2498cfebd4cf57e669228a2a8cde356722f25";
    let s = nitro::kms_statement(role, &pcr0.to_uppercase()).unwrap();
    assert_eq!(s["Condition"]["StringEqualsIgnoreCase"]["kms:RecipientAttestation:PCR0"], pcr0);
    assert_eq!(s["Condition"]["StringEqualsIgnoreCase"]["kms:RecipientAttestation:PCR3"], nitro::pcr_of_text(role));
    assert_eq!(s["Principal"]["AWS"], role);
    assert!(nitro::kms_statement("bp-nitro-host", pcr0).is_err());
}

#[test] fn the_plan_and_the_cost_create_nothing() {
    let c = on(); let p = nitro::plan(&c); let line = p[0].join(" ");
    assert!(line.starts_with("aws --output json --region us-east-1 --profile bp ec2 run-instances"));
    assert!(line.contains("--enclave-options Enabled=true") && line.contains("HttpPutResponseHopLimit=1") && line.contains("al2023-ami-kernel-6.18-x86_64") && line.contains("--instance-type c7i.4xlarge"));
    assert!(nitro::plan(&cfg(&[("sandbox.nitro.type", "c7g.2xlarge")]))[0].join(" ").contains("kernel-6.18-arm64"));
    assert!(nitro::plan(&cfg(&[("sandbox.nitro.type", "m8gd.xlarge")]))[0].join(" ").contains("arm64"));
    let k = nitro::cost("c7i.4xlarge", 200, 8.0, 4).unwrap();
    assert_eq!(k["hourly"], 0.714); assert_eq!(k["disk_monthly"], 16.0); assert_eq!(k["monthly"], 187.36); assert_eq!(k["per_sandbox_hour"], 0.179);
    assert!(nitro::cost("x9.huge", 10, 1.0, 1).is_none());
    // the status RPC: the plan as text, the order, nothing run
    let m = settings(&[("sandbox.nitro", "on"), ("sandbox.nitro.instance", "i-0123456789abcdef0")]);
    let st = rpc::status(&|k| m.get(k).cloned(), false);
    assert_eq!(st["order"]["build"], json!(["local", "nitro"])); assert_eq!(st["nitro"]["mode"], "confidential");
    assert!(st["nitro"]["plan"][0].as_str().unwrap().contains("run-instances"));
    // the probe of a hub with Nitro off asks nothing
    assert_eq!(rpc::probe(cfg(&[]))["error"], "Nitro sandboxes are off (setting sandbox.nitro)");
}

#[test] fn a_build_on_a_stopped_parent_starts_it_runs_and_comes_back() {
    let sh = Fake::new("stopped"); let n = Nitro::new(on(), &sh, "/w/scripts");
    let l = n.up(&small("bp-build-0")).unwrap();
    assert_eq!(l, Lease { backend: "nitro", name: "bp-build-0".into(), id: "ec2-user@ec2-3-1-2-3.compute-1.amazonaws.com#0".into() });
    let lines = sh.lines();
    assert!(lines[0].starts_with("aws --output json --region us-east-1 --profile bp ec2 describe-instances --instance-ids i-0123456789abcdef0"));
    assert!(lines.iter().any(|l| l.contains("ec2 start-instances")), "{lines:#?}");
    assert!(lines.iter().all(|l| !l.contains("run-instances") && !l.contains("terminate") && !l.contains("create")), "nothing is made or destroyed: {lines:#?}");
    let up = lines.iter().find(|l| l.contains("bp-nitro up")).unwrap();
    assert!(up.starts_with("ssh -o BatchMode=yes") && up.contains("bp-nitro up bp-build-0 0 10 "), "{up}");
    assert!(sh.st.lock().unwrap().nft[0].contains("table ip bp_nitro_0") && sh.st.lock().unwrap().nft[0].contains("169.254.169.254 reject"), "the rules went with it");
    // the same name again is the same box
    assert_eq!(n.up(&small("bp-build-0")).unwrap().id, l.id); assert_eq!(sh.lines().iter().filter(|l| l.contains("bp-nitro up")).count(), 1);
    // files in, the run's exit code, files out
    n.sync_in(&l, Path::new("/w"), Path::new("/tmp/list")).unwrap();
    let c = sh.calls(); let k = c.len();
    assert_eq!(c[k - 3][0], "rsync"); assert!(c[k - 3].iter().any(|a| a == "bp-build-0:/work/.bp/list"));
    assert!(c[k - 2].contains(&"--from0".to_string()) && c[k - 2].contains(&"/w/scripts/nitro-ssh.sh".to_string()) && c[k - 2].last().unwrap() == "bp-build-0:/work/src/");
    assert_eq!(sh.env_of(k - 2), vec![("BP_NITRO_SSH".to_string(), "ec2-user@ec2-3-1-2-3.compute-1.amazonaws.com".to_string())]);
    assert!(c[k - 1].join(" ").ends_with("bp-nitro prune bp-build-0"));
    sh.st.lock().unwrap().run_rc = 101;
    let env = vec![("RUSTFLAGS".to_string(), "-g".to_string()), ("GH_TOKEN".to_string(), "ghp_x".to_string())];
    assert_eq!(n.run(&l, "native/host", &["cargo".into(), "test".into()], &env), Ok(101), "a failed test is the command's exit code");
    let run = sh.lines().last().unwrap().clone();
    use base64::Engine; let enc = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
    assert!(run.ends_with(&format!("bp-nitro run bp-build-0 {} {} {}", enc("native/host"), enc("cargo\0test"), enc("RUSTFLAGS=-g"))), "the token stays here: {run}");
    n.run(&l, "", &["true".into()], &[]).unwrap(); assert!(sh.lines().last().unwrap().ends_with(&format!("bp-nitro run bp-build-0 - {} -", enc("true"))), "no empty word: ssh would drop it");
    sh.st.lock().unwrap().run_rc = 255; assert!(n.run(&l, "", &["true".into()], &[]).unwrap_err().contains("connection"), "ssh's own 255 is the transport, not the test");
    n.pull(&l, "/work/target/x", Path::new("/tmp/art"), Path::new("/w/target")).unwrap();
    assert!(sh.calls().last().unwrap().ends_with(&["bp-build-0:/work/target/x/".to_string(), "/w/target/".to_string()]));
    n.down(&l).unwrap(); assert!(sh.st.lock().unwrap().boxes.is_empty());
    // idle: the parent stops once nothing ran on it for sandbox.nitro.idle
    assert_eq!(n.stop_if_idle(1, 10_000), Ok(false)); assert_eq!(n.stop_if_idle(0, 899), Ok(false));
    assert_eq!(n.stop_if_idle(0, 900), Ok(true)); assert_eq!(sh.st.lock().unwrap().ec2, "stopping");
}

#[test] fn a_full_parent_raises_its_slots_or_says_why_not() {
    let sh = Fake::new("running"); let n = Nitro::new(on(), &sh, "s");
    for i in 0..4 { n.up(&small(&format!("b{i}"))).unwrap(); }
    n.up(&small("b4")).unwrap();
    assert!(sh.lines().last().unwrap().ends_with(" --slots=12"), "the fifth sandbox raises the slots to 12 first");
    n.down(&Lease { backend: "nitro", name: "b1".into(), id: "x#1".into() }).unwrap(); n.up(&small("b6")).unwrap();
    assert!(sh.lines().last().unwrap().contains("bp-nitro up b6 1 "), "a freed number is used again");
    assert_eq!(sh.st.lock().unwrap().slots, 12);
    sh.st.lock().unwrap().mem_free = 100;
    let e = n.up(&small("b5")).unwrap_err(); assert!(e.contains("pool"), "{e}");
    assert!(!sh.lines().last().unwrap().contains("bp-nitro up b5"), "nothing is tried that cannot fit");
}

#[test] fn aws_errors_are_said_and_nothing_else_is_tried() {
    let sh = Fake::new("stopped"); sh.st.lock().unwrap().aws_err = Some("Unable to locate credentials".into());
    let e = Nitro::new(on(), &sh, "s").up(&small("a")).unwrap_err();
    assert!(e.contains("Unable to locate credentials"), "{e}"); assert_eq!(sh.calls().len(), 1);
    let sh = Fake::new("terminated"); assert!(Nitro::new(on(), &sh, "s").up(&small("a")).unwrap_err().contains("terminated"));
    // SSH-only parent (the person keeps it on, e.g. on the tailnet): AWS is never asked
    let sh = Fake::new("running"); let c = cfg(&[("sandbox.nitro", "on"), ("sandbox.nitro.ssh", "ec2-user@nitro.tail.ts.net")]);
    Nitro::new(c, &sh, "s").up(&small("a")).unwrap();
    assert!(sh.lines().iter().all(|l| !l.starts_with("aws")) && sh.lines()[0].contains("ec2-user@nitro.tail.ts.net"));
}

#[test] fn the_boat_backend_does_the_same_five_steps() {
    let sh = Fake::new("running"); sh.st.lock().unwrap().boat = vec![("bp-build-0".into(), "bx_1".into(), "stopped".into())];
    let b = boat::Boat::new(&sh, "/w/scripts"); b.ready().unwrap();
    let l = b.up(&Spec::build(0)).unwrap(); assert_eq!(l.id, "bx_1");
    assert!(sh.lines().iter().any(|l| l == "boat --no-update resume bx_1"));
    let l1 = b.up(&Spec::build(1)).unwrap(); assert_eq!(l1.id, "bx_new");
    assert!(sh.lines().iter().any(|l| l.contains("new --from bp-build-base --name bp-build-1 --environment bp-build --no-env")));
    b.sync_in(&l, Path::new("/w"), Path::new("/tmp/list")).unwrap();
    assert!(sh.lines().iter().any(|l| l.contains("boat-prune.py")));
    let rc = b.run(&l, "native/host", &["cargo".into(), "test".into()], &[("GH_TOKEN".into(), "x".into()), ("RUSTFLAGS".into(), "-g".into())]).unwrap(); assert_eq!(rc, 0);
    let run = sh.lines().last().unwrap().clone(); assert!(run.contains("boat-run.py bx_1 bash -c") && run.contains("RUSTFLAGS") && !run.contains("GH_TOKEN"), "{run}");
    b.down(&l).unwrap(); assert!(sh.lines().last().unwrap().ends_with("stop bx_1"));
}

#[test] fn an_attested_sandbox_gets_its_secret_through_kms_not_from_the_vault() {
    let sh = Fake::new("running"); let mut c = on(); let n = Nitro::new(c.clone(), &sh, "s");
    let t = Spec::thread("t1794346115-14"); assert_eq!(t.name, "bp-thread-t1794346115-14"); assert!(t.attest && t.confidential);
    let l = n.up(&t).unwrap();
    assert!(sh.lines().last().unwrap().ends_with(" --no-wait"), "it waits for its secret before its agent answers");
    assert!(n.release_secret(&l, "/var/lib/bp-nitro/blobs/claude.enc").unwrap_err().contains("sandbox.nitro.kms"));
    c.kms = "arn:aws:kms:us-east-1:111122223333:key/1234abcd-12ab-34cd-56ef-1234567890ab".into(); let n = Nitro::new(c, &sh, "s");
    assert!(n.release_secret(&l, "relative.enc").is_err() && n.release_secret(&l, "/a b").is_err());
    n.release_secret(&l, "/var/lib/bp-nitro/blobs/claude.enc").unwrap();
    let k = sh.lines(); let k = &k[k.len() - 2..];
    assert!(k[0].ends_with("bp-nitro secret bp-thread-t1794346115-14 /var/lib/bp-nitro/blobs/claude.enc arn:aws:kms:us-east-1:111122223333:key/1234abcd-12ab-34cd-56ef-1234567890ab"));
    assert!(k[1].ends_with("bp-nitro wait bp-thread-t1794346115-14"));
    assert!(sh.lines().iter().all(|l| !l.contains("kms decrypt")), "the hub never calls KMS itself: the parent relays for the sandbox");
}
