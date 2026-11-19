//! The Windows resources of every Backplane executable (#1228): the logo as icon group 1 (the first icon, `exe,0`, which a
//! shortcut's IconLocation names) and a VERSIONINFO (ProductName, CompanyName, FileDescription, FileVersion from the crate's
//! version). Included by the build scripts of `native/host`, `native/gpui` and `native/guard` (`#[path = "../winres.rs"] mod winres;`)
//! and by the hub's tests, which run `rc_text` below. Windows targets only; other targets get nothing.
#![allow(dead_code)]

#[cfg(not(test))]
use std::{env, fs, path::PathBuf};

pub const COMPANY: &str = "Backplane";
pub const PRODUCT: &str = "Backplane";

/// `1.2.3` (or `1.2.3-nightly`) as the four numbers of a VERSIONINFO.
pub fn version_numbers(version: &str) -> [u32; 4] {
    let mut out = [0; 4];
    for (slot, part) in out.iter_mut().zip(version.split(['.', '-', '+'])) {
        *slot = part.parse().unwrap_or(0);
        if part.parse::<u32>().is_err() { break; }
    }
    out
}

/// The `.rc` source: `ico` is a forward-slash path.
pub fn rc_text(ico: &str, file_name: &str, description: &str, version: &str) -> String {
    let [a, b, c, d] = version_numbers(version);
    let quote = |s: &str| s.replace('"', "\"\"");
    format!(
        "1 ICON \"{ico}\"\n\
         1 VERSIONINFO\n FILEVERSION {a},{b},{c},{d}\n PRODUCTVERSION {a},{b},{c},{d}\n FILEOS 0x40004\n FILETYPE 1\n\
         BEGIN\n BLOCK \"StringFileInfo\"\n BEGIN\n  BLOCK \"040904b0\"\n  BEGIN\n\
         \x20  VALUE \"CompanyName\", \"{}\"\n   VALUE \"FileDescription\", \"{}\"\n   VALUE \"FileVersion\", \"{}\"\n\
         \x20  VALUE \"InternalName\", \"{}\"\n   VALUE \"OriginalFilename\", \"{}\"\n   VALUE \"ProductName\", \"{}\"\n   VALUE \"ProductVersion\", \"{}\"\n\
         \x20 END\n END\n BLOCK \"VarFileInfo\"\n BEGIN\n  VALUE \"Translation\", 0x409, 1200\n END\nEND\n",
        quote(COMPANY), quote(description), quote(version), quote(file_name.trim_end_matches(".exe")), quote(file_name), quote(PRODUCT), quote(version),
    )
}

/// Build-script step: embed the icon and VERSIONINFO into this crate's Windows binaries. `file_name` is the exe's name
/// (`backplane-hub.exe`), `description` what Explorer shows as its description.
#[cfg(not(test))] // (the hub's tests include this file without the build dependency)
pub fn embed(file_name: &str, description: &str) {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") { return; }
    let ico = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../assets/icon/backplane.ico");
    println!("cargo:rerun-if-changed={}", ico.display());
    let ico = fs::canonicalize(&ico).unwrap();
    let rc = PathBuf::from(env::var("OUT_DIR").unwrap()).join("backplane-winres.rc");
    // forward slashes: rc.exe and llvm-rc both take them, and a quoted path in .rc then needs no escaping
    let text = rc_text(ico.display().to_string().replace('\\', "/").trim_start_matches("//?/"), file_name, description, &env::var("CARGO_PKG_VERSION").unwrap());
    fs::write(&rc, text).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().unwrap();
}
