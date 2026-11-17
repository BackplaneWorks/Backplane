// Evidence for docs/kicad-pins.md: what an old updater does with the kicad-pins prerelease and a signed pin document.
// Rerun against a tag: git worktree add --detach .backplane/worktrees/kp-ev <tag>; append this file to native/host/src/update/run.rs there;
// scripts/cargo-q test --offline --bin backplane-hub -- kicad_pins_evidence --nocapture (v0.15.3 has no plat::tmp: give it std::env::temp_dir() dirs).
// Passed on v0.15.3 and v0.16.0 (2026-11-17): find says Err("the manifest lists no assets") for a pin document; the prerelease is never offered or fetched.

#[cfg(test)]
mod kicad_pins_evidence {
    //! Added only in a scratch worktree of an old tag: what this release's updater does with the `kicad-pins` prerelease and a signed pin document.
    use super::*;
    use ed25519_compact::{KeyPair, Seed};
    #[test] fn the_kicad_pins_prerelease_is_never_an_update_nor_half_published() {
        let listed = listing(br#"[{"tag_name":"kicad-pins","prerelease":true,"draft":false,"assets":[{"name":"kicad-pins.json","browser_download_url":"u"}]},
                                  {"tag_name":"v0.0.1","prerelease":false,"draft":false,"assets":[]}]"#).unwrap();
        assert_eq!(listed[0].rel.tag, "kicad-pins"); assert!(!listed[0].rel.signed, "no manifest.json*: not signed");
        assert!(!rel::tag_ok("kicad-pins"), "not an updater's tag shape");
        let rels: Vec<rel::Rel> = listed.iter().map(|l| l.rel.clone()).collect();
        for ch in ["stable", "beta", "nightly"] {
            assert!(!rel::wants(ch, &listed[0].rel), "{ch}");
            assert!(!rel::wants(ch, &rel::Rel { tag: "kicad-pins".into(), prerelease: true, draft: false, signed: true }), "{ch}: even with all three signed files");
            assert_eq!(rel::offer("0.0.1", ch, &rels), None, "{ch}");
            let d = crate::plat::tmp::dir("kp-ev"); let k: Vec<(String, [u8; 32])> = vec![];
            let asked = std::cell::RefCell::new(0); let get = |_: &str, _: &std::path::Path, _: u64| -> Result<(), String> { *asked.borrow_mut() += 1; Err("no".into()) };
            assert_eq!(find(&Ask { current: "0.0.1", channel: ch, target: "linux-x64", keys: &k, bad: &[] }, &listed, &d, &get), Ok(None), "{ch}");
            assert_eq!(*asked.borrow(), 0, "{ch}: nothing of it is fetched");
        }
    }
    #[test] fn a_signed_pin_document_is_no_release_manifest() {
        let kp = KeyPair::from_seed(Seed::new([3; 32])); let keys = vec![("k1".to_string(), *kp.pk)];
        let body = br#"{"schema":1,"kind":"backplane-kicad-pins","sequence":4,"expires":2000000000,"pins":{"linux-x86_64":{"version":"10.99.0-bp.13","release":"backplane-kicad-v10.99.0-bp.13","asset":"backplane-kicad-linux-x86_64-10.99.0-bp.13.tar.gz","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","size":1100000000,"min_hub":"0.16.1"}}}"#.to_vec();
        use base64::Engine; let b64 = base64::engine::general_purpose::STANDARD;
        let envelope = format!(r#"{{"schema":1,"keyid":"k1","sig":"{}","doc":"{}"}}"#, b64.encode(kp.sk.sign(&body, None).as_ref()), b64.encode(&body)).into_bytes();
        for (what, m) in [("the signed body", body.clone()), ("the envelope", envelope)] {
            assert!(rel::parse(&m).is_err(), "{what}: rel::parse");
            // the whole updater: a release v0.0.2 whose signed manifest.json is that document, under a trusted key
            let sig = kp.sk.sign(&m, None);
            let files: std::collections::HashMap<String, Vec<u8>> = [("manifest.json", m.clone()), ("manifest.json.sig", sig.as_ref().to_vec()), ("manifest.json.keyid", b"k1".to_vec())].into_iter().map(|(n, b)| (format!("mem://{n}"), b)).collect();
            let get = |u: &str, out: &std::path::Path, _: u64| -> Result<(), String> { std::fs::write(out, files.get(u).ok_or("404")?).map_err(|e| e.to_string()) };
            let listed = listing(br#"[{"tag_name":"v0.0.2","prerelease":false,"draft":false,"assets":[{"name":"manifest.json","browser_download_url":"mem://manifest.json"},{"name":"manifest.json.sig","browser_download_url":"mem://manifest.json.sig"},{"name":"manifest.json.keyid","browser_download_url":"mem://manifest.json.keyid"}]}]"#).unwrap();
            let d = crate::plat::tmp::dir("kp-ev2");
            let r = find(&Ask { current: "0.0.1", channel: "beta", target: "linux-x64", keys: &keys, bad: &[] }, &listed, &d, &get);
            eprintln!("{what}: find says {r:?}");
            assert!(r.is_err(), "{what}: refused");
        }
    }
}
