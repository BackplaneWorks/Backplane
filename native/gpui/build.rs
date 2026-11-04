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
    let css_path=PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../src/web/style.css");
    println!("cargo:rerun-if-changed={}",css_path.display());
    let css=fs::read_to_string(css_path).unwrap();
    let mut theme=String::new();
    for (selector, name) in [(":root {","DARK"),(".app.light {","LIGHT")] {
        let block=css.split(selector).nth(1).unwrap().split('}').next().unwrap();
        theme.push_str(&format!("pub const {name}: &[(&str,u32)] = &[\n"));
        for line in block.lines() {
            if let Some((key,value))=line.trim().strip_prefix("--").and_then(|l|l.split_once(": #")) {
                let value=value.trim_end_matches(';');
                if value.len()==6 && value.chars().all(|c|c.is_ascii_hexdigit()) {theme.push_str(&format!("(\"{key}\",0x{value}),\n"));}
            }
        }
        theme.push_str("];\n");
    }
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("theme.rs"),theme).unwrap();
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("wire_dict.rs"), output).unwrap();
}
