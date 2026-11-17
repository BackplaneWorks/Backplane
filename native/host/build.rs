//! Each checkpoint's stamp: a hash of the code its fold is made of (`src/fold_inputs.rs`). Done here so that only the numbers reach the binary, never the source.
#[path = "src/fold_inputs.rs"]
mod fold_inputs;
use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/fold_inputs.rs");
    let mut out = String::new();
    for (name, _, _) in fold_inputs::FOLDS {
        let files = fold_inputs::inputs(name);
        let texts: Vec<String> = files.iter().map(|f| { println!("cargo:rerun-if-changed=src/{f}"); fs::read_to_string(Path::new("src").join(f)).unwrap_or_else(|e| panic!("{f}: {e}")) }).collect();
        out.push_str(&format!("pub const {name}: u64 = {};\n", fold_inputs::stamp(texts.iter().map(String::as_str))));
    }
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("fold_stamps.rs"), out).unwrap();
    kistack();
}

/// KiStack's pinned skills (deploy/kistack, deploy/kistack.lock) compiled into the hub: `agents/kistack.rs` writes them into its home.
fn kistack() {
    let root = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../deploy").canonicalize().expect("deploy/");
    let lock = root.join("kistack.lock"); println!("cargo:rerun-if-changed={}", lock.display());
    let text = fs::read_to_string(&lock).expect("deploy/kistack.lock");
    let field = |k: &str| text.lines().filter_map(|l| { let mut p = l.split_whitespace(); (p.next() == Some(k)).then(|| p.next()).flatten() }).next().unwrap_or_else(|| panic!("deploy/kistack.lock names no {k}")).to_string();
    let src = root.join("kistack");
    let mut files = vec![];
    fn walk(d: &Path, out: &mut Vec<std::path::PathBuf>) {
        println!("cargo:rerun-if-changed={}", d.display());
        let mut es: Vec<_> = fs::read_dir(d).unwrap_or_else(|e| panic!("{}: {e}", d.display())).flatten().map(|e| e.path()).collect(); es.sort();
        for p in es { let t = fs::symlink_metadata(&p).unwrap().file_type(); if t.is_dir() { walk(&p, out) } else if t.is_file() { println!("cargo:rerun-if-changed={}", p.display()); out.push(p) } else { panic!("{}: only files and folders in deploy/kistack", p.display()) } }
    }
    walk(&src.join("skills"), &mut files); files.push(src.join("LICENSE")); println!("cargo:rerun-if-changed={}", src.join("LICENSE").display());
    let mut out = format!("pub const REV: &str = {:?};\npub const TREE: &str = {:?};\n/// (path under the copy, contents, executable)\npub const FILES: &[(&str, &[u8], bool)] = &[\n", field("rev"), field("skills"));
    for p in files {
        let rel = p.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
        #[cfg(unix)] let x = { use std::os::unix::fs::PermissionsExt; fs::metadata(&p).unwrap().permissions().mode() & 0o111 != 0 };
        #[cfg(not(unix))] let x = rel.ends_with(".sh");
        out.push_str(&format!("    ({rel:?}, include_bytes!({:?}), {x}),\n", p.to_string_lossy()));
    }
    out.push_str("];\n");
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("kistack_files.rs"), out).unwrap();
}
