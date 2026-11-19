//! #1228: every Windows executable carries the logo (icon group 1) and a VERSIONINFO; the Start-menu shortcut names an exe that has it.
use super::winres::{rc_text, version_numbers};

fn read(rel: &str) -> String { std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")) }

#[test]
fn versioninfo_carries_the_crate_version() {
    assert_eq!(version_numbers("0.16.2"), [0, 16, 2, 0]);
    assert_eq!(version_numbers("0.16.2-nightly-20261119.1"), [0, 16, 2, 0]);
    let rc = rc_text("C:/x/backplane.ico", "backplane-hub.exe", "Backplane hub", "0.16.2");
    assert!(rc.starts_with("1 ICON \"C:/x/backplane.ico\"\n"), "the icon is group 1");
    for want in ["FILEVERSION 0,16,2,0", "VALUE \"ProductName\", \"Backplane\"", "VALUE \"CompanyName\", \"Backplane\"", "VALUE \"FileVersion\", \"0.16.2\"",
                 "VALUE \"OriginalFilename\", \"backplane-hub.exe\"", "VALUE \"FileDescription\", \"Backplane hub\""] {
        assert!(rc.contains(want), "{want}\n{rc}");
    }
}

#[test]
fn every_windows_exe_embeds_icon_and_versioninfo() {
    for (script, exe) in [("build.rs", "backplane-hub.exe"), ("../gpui/build.rs", "backplane-gpui.exe"), ("../guard/build.rs", "backplane.exe")] {
        let s = read(script);
        assert!(s.contains("mod winres;") && s.contains(&format!("winres::embed(\"{exe}\"")), "{script} must embed the icon and VERSIONINFO of {exe}");
    }
    for manifest in ["Cargo.toml", "../gpui/Cargo.toml", "../guard/Cargo.toml"] {
        assert!(read(manifest).contains("embed-resource"), "{manifest}: build-dependency embed-resource");
    }
    assert!(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon/backplane.ico").is_file());
}

#[test]
fn the_start_menu_shortcut_takes_its_icon_from_the_guard() {
    let s = read("../gpui/src/install.rs");
    assert!(s.contains("$s.IconLocation = '{},0'") && s.contains("q(&prefix.join(\"backplane.exe\")), q(prefix), q(&prefix.join(\"backplane.exe\"))"),
        "IconLocation must be backplane.exe,0: it survives upgrades and has the icon");
}
