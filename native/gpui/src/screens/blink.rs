//! The text cursor's blink, one rule for every text box. The caret is solid while typing (a moment after the last edit), then follows
//! the platform's blink period (on for the first half, off for the second); a period of 0 means the platform turned blinking off.
//! A repaint is asked only at the next phase change while a caret is shown: nothing repaints when no box has focus.
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The default full period (on + off), what GTK and most desktops use.
pub const DEFAULT_MS: u64 = 1200;

/// Whether the caret shows `elapsed_ms` after the last edit/focus, for a full period; 0 = never blinks (always shown).
pub fn visible(elapsed_ms: u64, period_ms: u64) -> bool { period_ms == 0 || (elapsed_ms % period_ms) < period_ms / 2 }
/// Milliseconds until the phase changes next (at least 1); None when the caret never changes.
pub fn next_change(elapsed_ms: u64, period_ms: u64) -> Option<u64> { if period_ms == 0 { None } else { let half = (period_ms / 2).max(1); Some(half - (elapsed_ms % half)) } }

/// The platform's period: `BACKPLANE_GPUI_BLINK_MS` first (tests), then GNOME's `cursor-blink`/`cursor-blink-time`, else the default. Read once.
pub fn period() -> u64 {
    static P: OnceLock<u64> = OnceLock::new();
    *P.get_or_init(|| {
        if let Some(v) = std::env::var("BACKPLANE_GPUI_BLINK_MS").ok().and_then(|v| v.trim().parse::<u64>().ok()) { return v; }
        let get = |key: &str| std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.interface", key]).output().ok()
            .filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        if get("cursor-blink").as_deref() == Some("false") { return 0; }
        get("cursor-blink-time").and_then(|t| t.trim_start_matches("uint32 ").parse::<u64>().ok()).filter(|t| (200..=5000).contains(t)).unwrap_or(DEFAULT_MS)
    })
}

/// Per text box: when it last changed (an edit, a caret move, focus), which restarts the solid phase.
#[derive(Clone, Debug)]
pub struct Blink { since: Instant, sig: u64, pub pending: bool }
impl Default for Blink { fn default() -> Self { Self { since: Instant::now(), sig: 0, pending: false } } }
impl Blink {
    /// Called each paint with a signature of the box's state (text and selection); a change makes the caret solid again.
    pub fn seen(&mut self, sig: u64, focused: bool) { if sig != self.sig || !focused { self.sig = sig; self.since = Instant::now(); } }
    pub fn elapsed_ms(&self) -> u64 { self.since.elapsed().as_millis() as u64 }
    pub fn shown(&self) -> bool { visible(self.elapsed_ms(), period()) }
    pub fn wait(&self) -> Option<Duration> { next_change(self.elapsed_ms(), period()).map(Duration::from_millis) }
}
/// A cheap signature of a box's text and selection.
pub fn sig(text: &str, head: usize, anchor: usize) -> u64 { let mut h: u64 = 1469598103934665603; for b in text.bytes().rev().take(64) { h = (h ^ b as u64).wrapping_mul(1099511628211); } h ^ ((text.len() as u64) << 20) ^ ((head as u64) << 40) ^ ((anchor as u64) << 8) }

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    #[test] fn on_for_half_off_for_half() { assert!(visible(0, 1200) && visible(599, 1200) && !visible(600, 1200) && !visible(1199, 1200) && visible(1200, 1200)); }
    #[test] fn zero_period_is_always_shown_and_never_wakes() { assert!(visible(777, 0) && next_change(5, 0).is_none()); }
    #[test] fn the_wake_is_the_next_phase_change() { assert_eq!(next_change(0, 1200), Some(600)); assert_eq!(next_change(599, 1200), Some(1)); assert_eq!(next_change(600, 1200), Some(600)); }
    #[test] fn an_edit_restarts_the_solid_phase() {
        let mut b = Blink::default(); b.seen(sig("a", 1, 1), true); std::thread::sleep(Duration::from_millis(30));
        let before = b.elapsed_ms(); b.seen(sig("ab", 2, 2), true); assert!(b.elapsed_ms() < before.max(1) || b.elapsed_ms() == 0);
        assert_ne!(sig("a", 1, 1), sig("ab", 2, 2));
    }
}
