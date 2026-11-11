//! Where a link clicked in a message goes. Pure; the window does what the route says. A `backplane://` link is the hub's to resolve (`link-open`:
//! which project, which shelf, which room), a web address opens in the system browser, a path or a file:// address is the hub's too (`link-file`:
//! its project's thread and the viewer tab that draws it, never the system's app for a design file), anything else says so.

#[derive(Clone, Debug, PartialEq)]
pub enum Route { Web(String), Hub(String), File(String), /** a part, pin or net of the thread's design (screens/xprobe.rs) */ Design(String), Note(&'static str) }

pub fn route(href: &str, file: bool) -> Route {
    let h = href.trim();
    if let Some(w) = h.strip_prefix(super::xprobe::HREF).filter(|w| !w.is_empty()) { return Route::Design(w.to_string()) }
    if h.starts_with("backplane://") || crate::pairing::is_open_link(h) { return Route::Hub(h.to_string()) }
    if h.starts_with("https://") || h.starts_with("http://") || h.starts_with("mailto:") { return Route::Web(h.to_string()) }
    // a path, or a file:// address: inside Backplane (the hub's `link-file`)
    if (file || h.starts_with("file://")) && !h.is_empty() { return Route::File(h.to_string()) }
    /* i18n-ok: the note is drawn through trs */ Route::Note("That link does not open here.")
}

/// A file the viewer draws, by its name (the hub's present/filelink.rs `design_kind`): a KiCad board, schematic or project, a STEP, STP or
/// GLB model, Gerbers. Such a word is a link in a message even alone (`main.kicad_sch`), and opens inside Backplane.
pub fn design_file(p: &str) -> bool {
    let name = p.rsplit('/').next().unwrap_or(p); let l = name.to_ascii_lowercase();
    let Some((stem, e)) = l.rsplit_once('.') else { return false };
    let inner = e.strip_prefix('g').map_or(false, |n| !n.is_empty() && n.len() <= 2 && n.bytes().all(|b| b.is_ascii_digit()));
    !stem.is_empty() && (inner || matches!(e, "kicad_pcb" | "kicad_sch" | "kicad_pro" | "step" | "stp" | "glb" | "gbr" | "gbrjob" | "gtl" | "gbl" | "gts" | "gbs" | "gto" | "gbo" | "gtp" | "gbp" | "gko" | "gm1" | "gml" | "drl" | "xln"))
}
/// A file KiCad edits (a board, a schematic, a KiCad project): its link offers Open in KiCad (embedded).
pub fn kicad_file(p: &str) -> bool { let l = p.split(['#', '?']).next().unwrap_or(p).to_ascii_lowercase(); let l = l.trim_end_matches(|c: char| c.is_ascii_digit() || c == ':'); [".kicad_pcb", ".kicad_sch", ".kicad_pro"].iter().any(|x| l.ends_with(x)) }

/// A file link's name without its folder, extension or `:line` (`boards/main/main.kicad_pro:3` → `main`): what the viewer's source must be named.
pub fn stem_of(p: &str) -> String {
    let p = p.split('#').next().unwrap_or(p); let p = p.trim_end_matches(|c: char| c.is_ascii_digit() || c == ':');
    let name = p.rsplit('/').next().unwrap_or(p); name.rsplit_once('.').map_or(name, |(s, _)| s).to_string()
}
/// Whether the viewer shows the board or schematic of that name (its page and its source key, `0|<root>|<file>`).
pub fn shows_file(open: &str, key: &str, stem: &str) -> bool {
    matches!(open, "board" | "schematic") && !stem.is_empty() && key.rsplit('/').next().map_or(false, |f| f.rsplit_once('.').map_or(false, |(s, _)| s == stem))
}

/// What hovering a link shows: its target, as the person would read it.
pub fn target(href: &str) -> String {
    let h = href.trim();
    match h.strip_prefix("backplane://") { Some(r) => format!("backplane://{}", r.split('#').next().unwrap_or(r)), None => h.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn each_kind_of_link_has_its_route() {
        assert_eq!(route("backplane://thread/t1794241204-133", false), Route::Hub("backplane://thread/t1794241204-133".into()));
        assert_eq!(route("backplane://conv/room%3Aabc", false), Route::Hub("backplane://conv/room%3Aabc".into()));
        assert_eq!(route("https://example.com/a", false), Route::Web("https://example.com/a".into()));
        assert_eq!(route("http://x.io", false), Route::Web("http://x.io".into()));
        assert_eq!(route("~/Documents/a.rs", true), Route::File("~/Documents/a.rs".into()));
        assert_eq!(route("src/x.rs:12", true), Route::File("src/x.rs:12".into()));
        assert_eq!(route("kicad:U3", false), Route::Design("U3".into()));
    }
    #[test] fn a_file_address_is_a_file_and_design_files_are_known_by_name() {
        assert_eq!(route("file:///w/a.kicad_pcb", false), Route::File("file:///w/a.kicad_pcb".into()));
        for n in ["main.kicad_sch", "b/a.KICAD_PCB", "x.kicad_pro", "case.step", "m.glb", "a-F_Cu.gbr", "a.g2", "a.drl"] { assert!(design_file(n), "{n}"); }
        for n in ["x.rs", ".kicad_pcb", "notes.md", "g", "a.g123"] { assert!(!design_file(n), "{n}"); }
        assert!(kicad_file("a/b.kicad_sch:12") && kicad_file("file:///w/x.kicad_pro") && !kicad_file("a.step"));
    }
    #[test] fn the_embedded_editor_waits_for_the_viewer_to_show_the_file() {
        assert_eq!(stem_of("boards/main/main.kicad_pro:3"), "main"); assert_eq!(stem_of("file:///w/a.b.kicad_sch"), "a.b");
        assert!(shows_file("schematic", "1|/w|/w/boards/main/main.kicad_sch", "main")); assert!(shows_file("board", "0|/w|/w/main.kicad_pcb", "main"));
        assert!(!shows_file("board", "0|/w|", "main"), "the project's own board, before the file's"); assert!(!shows_file("mech", "2|/w|/w/main.step", "main"));
        assert!(!shows_file("board", "0|/w|/w/other.kicad_pcb", "main"));
    }
    #[test] fn what_opens_nowhere_is_a_note() {
        assert!(matches!(route("ftp://x", false), Route::Note(_))); assert!(matches!(route("", true), Route::Note(_))); assert!(matches!(route("javascript:alert(1)", false), Route::Note(_)));
    }
    #[test] fn the_hover_target_drops_a_message_anchor_only_on_backplane_links() {
        assert_eq!(target("backplane://thread/t1#m3"), "backplane://thread/t1"); assert_eq!(target("https://a.io/#x"), "https://a.io/#x");
    }
}
