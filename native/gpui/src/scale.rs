//! The text size this device chose (Settings, This device, `Local::text_pct`). Every text size and line height in the client is
//! written in its size at 100% and goes through `tpx`, so a size and its line grow together and keep the proportions they have at 100%
//! (a root line height that grew while an 11 px label kept its size spread every label and note apart and over its neighbours). A box
//! sized to hold one line of text (a bar, a chip, a list row) is written through `tpx` too, so the line it holds still fits it.
//! The terminal keeps its own grid size. The window's root sets it from `Local` at the start of every frame (`set`).
use gpui::{px, Pixels};
use std::sync::atomic::{AtomicU32, Ordering};

static PCT: AtomicU32 = AtomicU32::new(100);

/// The size in force, from the device's setting (clamped to the setting's range).
pub fn set(pct: u32) { PCT.store(pct.clamp(crate::local::TEXT_PCT.0, crate::local::TEXT_PCT.1), Ordering::Relaxed) }
/// The factor: 1.0 at 100%.
pub fn s() -> f32 { at(1., PCT.load(Ordering::Relaxed)) }
/// `n` px at 100%, at the size in force.
pub fn tpx(n: f32) -> Pixels { px(n * s()) }
/// `n` px at 100%, at the size in force but never over `max`: a label in a box whose width is fixed by the layout (the rail's 72 px).
pub fn tpx_max(n: f32, max: f32) -> Pixels { px((n * s()).min(max)) }
/// `n` at `pct` percent (whole pixels' fractions kept: the layout rounds).
pub fn at(n: f32, pct: u32) -> f32 { n * pct as f32 / 100. }

#[cfg(test)]
mod tests {
    #[test] fn a_size_scales_with_the_setting() {
        assert_eq!(super::at(13., 100), 13.); assert_eq!(super::at(20., 160), 32.); assert_eq!(super::at(10., 80), 8.);
    }
}
