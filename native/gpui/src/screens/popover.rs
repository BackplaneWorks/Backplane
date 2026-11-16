//! A menu that drops from a button (the Boards menu, a schematic's Sheets): a press outside shuts it, and that press may be on its own button, whose
//! click then came after and opened it again (Nov 16: "clicking it again does not close it, it reopens"). `shut` notes when a menu was shut by a press
//! outside; `just_shut` tells the button's click that this press already shut it, so the click leaves it shut.
use std::{sync::Mutex, time::{Duration, Instant}};

static SHUT: Mutex<Option<(&'static str, Instant)>> = Mutex::new(None);
/// A press and its click come within this of each other.
const SAME_PRESS: Duration = Duration::from_millis(400);

/// The menu `id` was shut by a press outside it, now.
pub fn shut(id: &'static str) { *SHUT.lock().unwrap() = Some((id, Instant::now())); }
/// Whether the menu `id` was shut by the press this click ends (taken once).
pub fn just_shut(id: &'static str) -> bool { let mut s = SHUT.lock().unwrap(); let hit = within(*s, id, Instant::now()); if hit { *s = None; } hit }
pub fn within(shut: Option<(&'static str, Instant)>, id: &'static str, now: Instant) -> bool { shut.map_or(false, |(i, at)| i == id && now.saturating_duration_since(at) < SAME_PRESS) }

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_click_on_the_button_that_shut_its_menu_leaves_it_shut() {
        let t = Instant::now();
        assert!(within(Some(("boards", t)), "boards", t + Duration::from_millis(120)), "the same press");
        assert!(!within(Some(("boards", t)), "boards", t + Duration::from_secs(2)), "a later click opens it");
        assert!(!within(Some(("sheets", t)), "boards", t), "another menu");
        assert!(!within(None, "boards", t));
    }
}
