//! The guard's Windows icon and VERSIONINFO: the Start-menu shortcut targets `backplane.exe`, so this is the icon people see (#1228).
#[path = "../winres.rs"]
mod winres;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    winres::embed("backplane.exe", "Backplane");
}
