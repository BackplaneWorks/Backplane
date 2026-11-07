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
}
