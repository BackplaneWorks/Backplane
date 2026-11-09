use std::{env, fs, path::PathBuf};
fn main() {
    // the cef feature links libcef.so dynamically; the cef crate copies it beside the binary, so find it there (docs/browser-cef.md)
    // (Linux only: Windows finds libcef.dll beside the .exe by itself; macOS loads the framework at run time, viewers/cef.rs)
    if env::var_os("CARGO_FEATURE_CEF").is_some() && env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") { println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN"); }
    // Windows (MSVC): libcef.dll is delay-loaded, so a client whose runtime is missing still starts and its Browser tab says why and shows the hub's
    // frames; viewers/cef.rs checks the runtime and argv before any CEF call (a gnullvm cross build, scripts/windows-check.sh, builds without cef)
    if env::var_os("CARGO_FEATURE_CEF").is_some() && env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bin=backplane-gpui=/DELAYLOAD:libcef.dll");
    }
    let css_path=PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../assets/web/style.css");
    println!("cargo:rerun-if-changed={}",css_path.display());
    let css=fs::read_to_string(css_path).unwrap();
    let mut theme=String::new();
    for (selector, name) in [(":root {","DARK"),("html.light, .app.light, .bub-move.light {","LIGHT")] {
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
}
