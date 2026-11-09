//! Where a link clicked in a message goes. Pure; the window does what the route says. A `backplane://` link is the hub's to resolve (`link-open`:
//! which project, which shelf, which room), a web address opens in the system browser, a path opens in the Files viewer, anything else says so.

#[derive(Clone, Debug, PartialEq)]
pub enum Route { Web(String), Hub(String), File(String), Note(&'static str) }

pub fn route(href: &str, file: bool) -> Route {
    let h = href.trim();
    if h.starts_with("backplane://") || crate::pairing::is_open_link(h) { return Route::Hub(h.to_string()) }
    if h.starts_with("https://") || h.starts_with("http://") || h.starts_with("mailto:") { return Route::Web(h.to_string()) }
    if file && !h.is_empty() { return Route::File(h.to_string()) }
    Route::Note("That link does not open here.")
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
    }
    #[test] fn what_opens_nowhere_is_a_note() {
        assert!(matches!(route("ftp://x", false), Route::Note(_))); assert!(matches!(route("", true), Route::Note(_))); assert!(matches!(route("javascript:alert(1)", false), Route::Note(_)));
    }
    #[test] fn the_hover_target_drops_a_message_anchor_only_on_backplane_links() {
        assert_eq!(target("backplane://thread/t1#m3"), "backplane://thread/t1"); assert_eq!(target("https://a.io/#x"), "https://a.io/#x");
    }
}
