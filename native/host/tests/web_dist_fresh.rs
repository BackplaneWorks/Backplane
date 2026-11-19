//! Gate test: fails when committed `web/dist` is stale against `web/src`, `assets/web/style.css`,
//! or `i18n/*.msg`. Runs in pure Rust without requiring `bun` on build/release machines.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("native/host sits two levels below repo root")
        .to_path_buf()
}

fn sha256_hex(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let digest = Sha256::digest(&bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            walk_files(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

#[test]
fn web_dist_is_fresh_and_meets_csp_and_size_budget() {
    let root = repo_root();
    let dist = root.join("web/dist");
    let manifest_path = dist.join("manifest.json");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .expect("web/dist/manifest.json missing — run scripts/build-web.sh");
    let manifest: Value =
        serde_json::from_str(&manifest_raw).expect("web/dist/manifest.json must be valid JSON");

    // 1. Every recorded input must match its SHA-256 on disk.
    let inputs = manifest["inputs"]
        .as_object()
        .expect("manifest.inputs must be an object");
    assert!(!inputs.is_empty(), "manifest.inputs must not be empty");
    for (rel, expected) in inputs {
        let want = expected.as_str().expect("input hash must be a string");
        let full = root.join(rel);
        assert!(
            full.is_file(),
            "web/dist is stale: recorded input {rel} no longer exists (run scripts/build-web.sh)"
        );
        let got = sha256_hex(&full);
        assert_eq!(
            got, want,
            "web/dist is stale: {rel} changed since last build (run scripts/build-web.sh)"
        );
    }

    // 2. Every file under web/src/** must be recorded in manifest.inputs.
    let mut src_files = Vec::new();
    walk_files(&root.join("web/src"), &mut src_files);
    for full in src_files {
        let rel = full
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            inputs.contains_key(&rel),
            "web/dist is stale: new source file {rel} is not in web/dist/manifest.json (run scripts/build-web.sh)"
        );
    }

    // 3. Every recorded output must exist in web/dist and match its SHA-256, with no extra files.
    let outputs = manifest["outputs"]
        .as_object()
        .expect("manifest.outputs must be an object");
    let mut dist_files = Vec::new();
    walk_files(&dist, &mut dist_files);
    let mut actual_outputs = BTreeSet::new();
    for full in dist_files {
        let rel = full
            .strip_prefix(&dist)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "manifest.json" {
            continue;
        }
        actual_outputs.insert(rel);
    }
    for (rel, info) in outputs {
        let want = info["sha256"]
            .as_str()
            .expect("output sha256 must be a string");
        let full = dist.join(rel);
        assert!(
            full.is_file(),
            "web/dist missing recorded output {rel} (run scripts/build-web.sh)"
        );
        let got = sha256_hex(&full);
        assert_eq!(
            got, want,
            "web/dist output {rel} does not match manifest.json (run scripts/build-web.sh)"
        );
        assert!(actual_outputs.remove(rel));
    }
    assert!(
        actual_outputs.is_empty(),
        "unexpected unrecorded files in web/dist: {actual_outputs:?}"
    );

    // 4. P1 size budget: JS + CSS <= 300 KB gzipped.
    let gz = manifest["totalAssetGzipBytes"]
        .as_u64()
        .expect("totalAssetGzipBytes must be a number");
    assert!(
        gz <= 300 * 1024,
        "web bundle exceeds P1 300 KB gz budget: {gz} bytes"
    );

    // 5. S8 CSP guard: HTML shells must contain no inline scripts or inline style attributes.
    for html_name in ["index.html", "pair.html"] {
        let html = fs::read_to_string(dist.join(html_name))
            .unwrap_or_else(|e| panic!("missing {html_name}: {e}"));
        let lower = html.to_ascii_lowercase();
        assert!(
            !lower.contains("style="),
            "{html_name} must not contain inline style= attributes"
        );
        for chunk in lower.split("<script").skip(1) {
            let tag_end = chunk.find('>').expect("unclosed <script tag");
            let open_tag = &chunk[..tag_end];
            assert!(
                open_tag.contains("src="),
                "{html_name} must not contain inline <script> blocks"
            );
            let rest = &chunk[tag_end + 1..];
            let close = rest.find("</script>").expect("unclosed </script>");
            assert!(
                rest[..close].trim().is_empty(),
                "{html_name} <script> tag must have empty body"
            );
        }
    }
}
