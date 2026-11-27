//! Where a link clicked in a message goes. Pure; the window does what the route says. A `backplane://` link is the hub's to resolve (`link-open`:
//! which project, which shelf, which room), a web address opens in the system browser, a path or a file:// address is the hub's too (`link-file`:
//! its project's thread and the viewer tab that draws it, never the system's app for a design file), anything else says so.

#[derive(Clone, Debug, PartialEq)]
pub enum Route { Web(String), Hub(String), File(String), Settings(String), /** a part, pin or net of the thread's design (screens/xprobe.rs) */ Design(String), Note(&'static str),
    /** an agent's plan review (`backplane://plan/<board from the project folder>`, the hub's plan/mod.rs `review_link`): that board with its Plan panel */ Plan(String),
    /** a file an agent shared (`/s/<token>/<name>`, the hub's files/share.rs `share_route`): it downloads from the hub */ Share(String) }

/// Match `backplane://settings/<page>` (or app scheme alias) and return the normalized page name.
pub fn settings_page(href: &str) -> Option<String> {
    let h = href.trim();
    let rest = ["backplane://settings/", "bpgpui://settings/", "backplane-gpui://settings/"]
        .iter().find_map(|p| h.strip_prefix(p))?;
    let page = rest.split(['#', '?']).next().unwrap_or("").trim_end_matches('/');
    (!page.is_empty()).then(|| page.to_string())
}

/// `%XX` decoded (a link's path segments, as the hub's md/refs.rs `uri` writes them); a bad escape is kept as it is.
fn unpct(s: &str) -> String {
    let b = s.as_bytes(); let mut out = Vec::with_capacity(b.len()); let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    while i < b.len() { if b[i] == b'%' && i + 3 <= b.len() { if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) { out.push(h * 16 + l); i += 3; continue } } out.push(b[i]); i += 1 }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn route(href: &str, file: bool) -> Route {
    let h = href.trim();
    if let Some(rel) = h.strip_prefix("backplane://plan/").map(|r| r.split(['#', '?']).next().unwrap_or("")).filter(|r| !r.is_empty()) { return Route::Plan(rel.split('/').map(unpct).collect::<Vec<_>>().join("/")) }
    if let Some(page) = settings_page(h) { return Route::Settings(page) }
    if let Some(w) = h.strip_prefix(super::xprobe::HREF).filter(|w| !w.is_empty()) { return Route::Design(w.to_string()) }
    if h.starts_with("backplane://") || crate::pairing::is_open_link(h) { return Route::Hub(h.to_string()) }
    if h.starts_with("https://") || h.starts_with("http://") || h.starts_with("mailto:") { return Route::Web(h.to_string()) }
    if share_name(h).is_some() { return Route::Share(h.to_string()) }
    // a path, or a file:// address: inside Backplane (the hub's `link-file`)
    if (file || h.starts_with("file://")) && !h.is_empty() { return Route::File(h.to_string()) }
    /* i18n-ok: the note is drawn through trs */ Route::Note("That link does not open here.")
}

/// A share's file name, when `href` is a share's address on the hub (`/s/<token>/<name>`, the token hex as the hub makes it).
pub fn share_name(href: &str) -> Option<String> {
    let rest = href.trim().strip_prefix("/s/")?; let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (token, name) = rest.split_once('/')?;
    (token.len() >= 16 && token.bytes().all(|b| b.is_ascii_alphanumeric()) && !name.is_empty() && !name.contains('/')).then(|| crate::viewers::downloads::safe_name(&crate::viewers::downloads::unpct(name)))
}
/// A path as a message and the hub's `files` spell it, without what a link adds: `file://`, a `:line` (`:12`, `:L12`) or an anchor.
fn bare(p: &str) -> &str {
    let p = p.trim(); let p = p.strip_prefix("file://").unwrap_or(p); let p = p.split('#').next().unwrap_or(p);
    let p = p.split_once(":L").map_or(p, |(a, b)| if b.chars().next().map_or(false, |c| c.is_ascii_digit()) { a } else { p });
    match p.rsplit_once(':') { Some((a, b)) if !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()) => a, _ => p }
}
/// The entry of a message's `files` (the hub's `{path, name, size, open}`) that a file link names.
pub fn listed<'a>(href: &str, files: &'a serde_json::Value) -> Option<&'a serde_json::Value> {
    let want = bare(href);
    files.as_array()?.iter().find(|f| f["path"].as_str().map_or(false, |p| !want.is_empty() && bare(p) == want))
}
/// The hub address that downloads a file of the hub's machine (`/file`), and the name it is saved as. A thread of another machine
/// (`<link>~<id>`) names itself, so its hub reads the file there. A path the hub cannot take whole (relative, outside a far thread) has none.
pub fn file_download(path: &str, thread: &str) -> Option<(String, String)> {
    let p = bare(path);
    let abs = p.starts_with('/') || p.starts_with("~/") || p.as_bytes().get(1) == Some(&b':') || p.starts_with("\\\\");
    let far = thread.contains('~');
    if p.is_empty() || (!abs && !far) { return None }
    use crate::viewers::downloads::{pct, safe_name};
    let url = if far { format!("/file?thread={}&path={}", pct(thread), pct(p)) } else { format!("/file?path={}", pct(p)) };
    Some((url, safe_name(file_name(p))))
}
/// What a press on a file link does (docs/file-links.md): on this computer's hub it opens inside Backplane (`link-file`) as always. On another
/// computer's hub a file the hub lists for the message with `open: "download"` comes down to this computer; one Backplane's viewers draw
/// (`view`) and any path the hub did not list still open in the viewer, which reads through the hub.
#[derive(Clone, Debug, PartialEq)]
pub enum FileGo { Open(String), Download { url: String, name: String } }
pub fn file_go(href: &str, files: &serde_json::Value, remote: bool, thread: &str) -> FileGo {
    if remote { if let Some(f) = listed(href, files).filter(|f| f["open"] == "download") {
        if let Some((url, name)) = file_download(f["path"].as_str().unwrap_or(""), thread) {
            let name = f["name"].as_str().filter(|n| !n.is_empty()).map(crate::viewers::downloads::safe_name).unwrap_or(name);
            return FileGo::Download { url, name }
        }
    } }
    FileGo::Open(href.trim().to_string())
}
/// The Download a file link's menu offers: on another computer's hub, any path the hub could serve (it says no to one it does not share).
pub fn menu_download(href: &str, files: &serde_json::Value, remote: bool, thread: &str) -> Option<(String, String)> {
    if !remote { return None }
    let name = listed(href, files).and_then(|f| f["name"].as_str()).filter(|n| !n.is_empty()).map(crate::viewers::downloads::safe_name);
    file_download(href, thread).map(|(u, n)| (u, name.unwrap_or(n)))
}

/// A file the viewer draws, by its name (the hub's present/filelink.rs `design_kind`): a KiCad board, schematic or project, a SolidCraft CAD/3D
/// model, Gerbers. Such a word is a link in a message even alone (`main.kicad_sch`, `case.sc`), and opens inside Backplane.
pub fn design_file(p: &str) -> bool {
    let l = file_name(p).to_ascii_lowercase();
    let Some((stem, e)) = l.rsplit_once('.') else { return false };
    let inner = e.strip_prefix('g').map_or(false, |n| !n.is_empty() && n.len() <= 2 && n.bytes().all(|b| b.is_ascii_digit()));
    !stem.is_empty() && (inner || matches!(e, "kicad_pcb" | "kicad_sch" | "kicad_pro" | "pcbdoc" | "schdoc" | "prjpcb" | "step" | "stp" | "sc" | "solidcraft" | "iges" | "igs" | "stl" | "3mf" | "obj" | "glb" | "gbr" | "gbrjob" | "gtl" | "gbl" | "gts" | "gbs" | "gto" | "gbo" | "gtp" | "gbp" | "gko" | "gm1" | "gml" | "drl" | "xln"))
}
/// A CAD or 3D model file opened in the embedded SolidCraft CAD pane.
pub fn cad_file(p: &str) -> bool {
    let l = p.split(['#', '?']).next().unwrap_or(p).to_ascii_lowercase();
    let l = l.trim_end_matches(|c: char| c.is_ascii_digit() || c == ':');
    [".step", ".stp", ".sc", ".solidcraft", ".iges", ".igs", ".stl", ".3mf", ".obj", ".glb"].iter().any(|x| l.ends_with(x))
}
/// A file KiCad edits (a board, a schematic, a KiCad project): its link offers Open in KiCad (embedded).
pub fn kicad_file(p: &str) -> bool { let l = p.split(['#', '?']).next().unwrap_or(p).to_ascii_lowercase(); let l = l.trim_end_matches(|c: char| c.is_ascii_digit() || c == ':'); [".kicad_pcb", ".kicad_sch", ".kicad_pro"].iter().any(|x| l.ends_with(x)) }

/// A path's last part: after its last `/` or `\` (a Windows path, `C:\w\a.kicad_pcb` or `\\host\share\a.kicad_pcb`, may mix both).
fn file_name(p: &str) -> &str { p.rsplit(['/', '\\']).next().unwrap_or(p) }
/// Whether two file stems name the same file: case-blind where the file system is (Windows, macOS), exact elsewhere.
fn same_stem(a: &str, b: &str, fold: bool) -> bool { if fold { a.eq_ignore_ascii_case(b) } else { a == b } }
const FOLD_CASE: bool = cfg!(any(windows, target_os = "macos"));

/// A file link's name without its folder, extension or `:line` (`boards/main/main.kicad_pro:3` → `main`): what the viewer's source must be named.
pub fn stem_of(p: &str) -> String {
    let p = p.split('#').next().unwrap_or(p); let p = p.trim_end_matches(|c: char| c.is_ascii_digit() || c == ':');
    let name = file_name(p); name.rsplit_once('.').map_or(name, |(s, _)| s).to_string()
}
/// Whether the viewer shows the board or schematic of that name (its page and its source key, `0|<root>|<file>`).
pub fn shows_file(open: &str, key: &str, stem: &str) -> bool { shows_file_as(open, key, stem, FOLD_CASE) }
fn shows_file_as(open: &str, key: &str, stem: &str, fold: bool) -> bool {
    matches!(open, "board" | "schematic") && !stem.is_empty() && file_name(key.rsplit('|').next().unwrap_or(key)).rsplit_once('.').map_or(false, |(s, _)| same_stem(s, stem, fold))
}

/// What hovering a link shows: its target, as the person would read it.
pub fn target(href: &str) -> String {
    let h = href.trim();
    match h.strip_prefix("backplane://") { Some(r) => format!("backplane://{}", r.split('#').next().unwrap_or(r)), None => h.to_string() }
}

#[cfg(test)]
mod tests {
    #[test] fn a_plan_review_link_opens_its_board_with_the_plan() {
        assert_eq!(super::route("backplane://plan/Nodes/A%20b/A.kicad_pcb", false), super::Route::Plan("Nodes/A b/A.kicad_pcb".into()));
        assert_eq!(super::route("backplane://plan/a.kicad_pcb#x", false), super::Route::Plan("a.kicad_pcb".into()));
        assert_eq!(super::route("backplane://plan/", false), super::Route::Hub("backplane://plan/".into()), "no board: the hub's");
    }
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
        for n in ["main.kicad_sch", "b/a.KICAD_PCB", "x.kicad_pro", "Top.SchDoc", "hw/Board.PcbDoc", "x.PrjPcb", "case.step", "case.sc", "case.solidcraft", "part.iges", "part.stl", "part.3mf", "part.obj", "m.glb", "a-F_Cu.gbr", "a.g2", "a.drl"] { assert!(design_file(n), "{n}"); }
        for n in ["x.rs", ".kicad_pcb", "notes.md", "g", "a.g123"] { assert!(!design_file(n), "{n}"); }
        assert!(kicad_file("a/b.kicad_sch:12") && kicad_file("file:///w/x.kicad_pro") && !kicad_file("a.step"));
        assert!(cad_file("case.sc") && cad_file("file:///w/case.step:3") && cad_file("part.STL") && !cad_file("main.kicad_pcb"));
    }
    #[test] fn the_embedded_editor_waits_for_the_viewer_to_show_the_file() {
        assert_eq!(stem_of("boards/main/main.kicad_pro:3"), "main"); assert_eq!(stem_of("file:///w/a.b.kicad_sch"), "a.b");
        assert!(shows_file("schematic", "1|/w|/w/boards/main/main.kicad_sch", "main")); assert!(shows_file("board", "0|/w|/w/main.kicad_pcb", "main"));
        assert!(!shows_file("board", "0|/w|", "main"), "the project's own board, before the file's"); assert!(!shows_file("mech", "2|/w|/w/main.step", "main"));
        assert!(!shows_file("board", "0|/w|/w/other.kicad_pcb", "main"));
    }
    #[test] fn windows_paths_name_the_same_file() {
        // a link to a Windows absolute path, a UNC path, mixed separators, a file:// address and a project-relative link
        assert_eq!(stem_of(r"C:\Users\h\proj\ecc83-pp.kicad_sch"), "ecc83-pp"); assert_eq!(stem_of(r"\\host\share\p\main.kicad_pcb"), "main");
        assert_eq!(stem_of(r"C:/Users/h\proj/boards\main.kicad_pcb:12"), "main"); assert_eq!(stem_of("file:///C:/w/a.b.kicad_sch"), "a.b");
        assert_eq!(stem_of(r"boards\main\main.kicad_pro"), "main"); assert_eq!(stem_of("ecc83-pp.kicad_sch"), "ecc83-pp");
        // the viewer's source key on Windows (#1369)
        let key = r"1|C:\bp\runs\g1353\proj|C:\bp\runs\g1353\proj\ecc83-pp.kicad_sch";
        assert!(shows_file("schematic", key, "ecc83-pp")); assert!(!shows_file("schematic", key, "proj")); assert!(!shows_file("schematic", r"1|C:\w|", "w"), "no file yet");
        assert!(shows_file("board", r"0|C:\w|C:\w/boards\main.kicad_pcb", "main"), "mixed separators");
        assert!(shows_file("board", r"0|\\host\share\w|\\host\share\w\main.kicad_pcb", "main"), "UNC");
        // the same file in another case: one file where the file system ignores case, two where it does not
        assert!(shows_file_as("board", r"0|C:\w|C:\w\Main.kicad_pcb", "main", true)); assert!(!shows_file_as("board", "0|/w|/w/Main.kicad_pcb", "main", false));
    }
    #[test] fn on_another_computers_hub_a_listed_download_comes_down_and_the_rest_open() {
        let files = serde_json::json!([{"path": "/w/p/mfg/gerbers.zip", "name": "gerbers.zip", "size": 9, "open": "download"}, {"path": "~/w/p/main.kicad_pcb", "name": "main.kicad_pcb", "size": 9, "open": "view"}]);
        assert_eq!(file_go("/w/p/mfg/gerbers.zip", &files, true, "t1"), FileGo::Download { url: "/file?path=%2Fw%2Fp%2Fmfg%2Fgerbers.zip".into(), name: "gerbers.zip".into() });
        assert_eq!(file_go("file:///w/p/mfg/gerbers.zip:3", &files, true, "t1"), FileGo::Download { url: "/file?path=%2Fw%2Fp%2Fmfg%2Fgerbers.zip".into(), name: "gerbers.zip".into() }, "the link's own spelling");
        assert_eq!(file_go("/w/p/mfg/gerbers.zip", &files, false, "t1"), FileGo::Open("/w/p/mfg/gerbers.zip".into()), "this computer's hub: as always");
        assert_eq!(file_go("~/w/p/main.kicad_pcb", &files, true, "t1"), FileGo::Open("~/w/p/main.kicad_pcb".into()), "the viewer draws it");
        assert_eq!(file_go("/w/p/other.bin", &files, true, "t1"), FileGo::Open("/w/p/other.bin".into()), "not listed: the hub did not allow it");
        assert_eq!(file_go("/w/p/mfg/gerbers.zip", &files, true, "desk~t9"), FileGo::Download { url: "/file?thread=desk~t9&path=%2Fw%2Fp%2Fmfg%2Fgerbers.zip".into(), name: "gerbers.zip".into() }, "another machine's thread");
        assert_eq!(menu_download("/w/p/x.bin:4", &serde_json::Value::Null, true, "t1"), Some(("/file?path=%2Fw%2Fp%2Fx.bin".into(), "x.bin".into())), "the menu offers any absolute path");
        assert_eq!(menu_download("src/x.rs", &serde_json::Value::Null, true, "t1"), None, "a relative path has no file on the hub to ask for");
        assert_eq!(menu_download("src/x.rs", &serde_json::Value::Null, true, "desk~t9"), Some(("/file?thread=desk~t9&path=src%2Fx.rs".into(), "x.rs".into())), "a far thread reads its own folder");
        assert_eq!(menu_download("/w/x.bin", &files, false, "t1"), None, "this computer's hub: no Download");
    }
    #[test] fn a_share_link_is_a_download_from_the_hub() {
        let tok = "0123456789abcdef0123456789abcdef";
        assert_eq!(route(&format!("/s/{tok}/a%20b.zip"), false), Route::Share(format!("/s/{tok}/a%20b.zip")));
        assert_eq!(route(&format!("/s/{tok}/a%20b.zip"), true), Route::Share(format!("/s/{tok}/a%20b.zip")), "drawn as a path too");
        assert_eq!(share_name(&format!("/s/{tok}/a%20b.zip")).as_deref(), Some("a b.zip"));
        assert_eq!(share_name("/s/x/a.zip"), None); assert_eq!(share_name(&format!("/s/{tok}/")), None); assert_eq!(share_name(&format!("/s/{tok}/a/b")), None);
        assert_eq!(route("/srv/s/a.txt", true), Route::File("/srv/s/a.txt".into()));
    }
    #[test] fn what_opens_nowhere_is_a_note() {
        assert!(matches!(route("ftp://x", false), Route::Note(_))); assert!(matches!(route("", true), Route::Note(_))); assert!(matches!(route("javascript:alert(1)", false), Route::Note(_)));
    }
    #[test] fn the_hover_target_drops_a_message_anchor_only_on_backplane_links() {
        assert_eq!(target("backplane://thread/t1#m3"), "backplane://thread/t1"); assert_eq!(target("https://a.io/#x"), "https://a.io/#x");
    }
    #[test] fn settings_links_route_to_settings_page() {
        assert_eq!(route("backplane://settings/accounts", false), Route::Settings("accounts".into()));
        assert_eq!(route("bpgpui://settings/accounts/", false), Route::Settings("accounts".into()));
        assert_eq!(route("backplane://settings/agents?x=1", false), Route::Settings("agents".into()));
        assert_eq!(settings_page("https://example.com/settings/accounts"), None);
    }
}
