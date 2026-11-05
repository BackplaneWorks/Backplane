// The CBOR dictionaries are generated from src/core/cbor.bend, the one source of truth
// (append-only forever there). Same method as native/gpui/build.rs.
use std::{env, fs, path::PathBuf};
fn main() {
    let source = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../src/core/cbor.bend");
    println!("cargo:rerun-if-changed={}", source.display());
    let text = fs::read_to_string(source).unwrap();
    let mut output = String::new();
    for (name, constant) in [("keys", "KEYS"), ("words", "WORDS")] {
        let tail = text.split(&format!("def Cbor.{name}()")).nth(1).unwrap();
        let array = &tail[tail.find('[').unwrap()..=tail.find(']').unwrap()];
        output.push_str(&format!("pub const {constant}: &[&str] = &{array};\n"));
    }
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("wire_dict.rs"), output).unwrap();
}
