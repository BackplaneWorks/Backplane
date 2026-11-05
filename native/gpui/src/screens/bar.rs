//! The composer's controls row: every button the same height (the one-row law). One constant and one wrapper, so no control sizes itself.
use gpui::{prelude::*, px, Styled};

/// The row's button height in px.
pub const H: f32 = 32.;
/// A button of the row: the row's height, its content centred vertically. (Width is the caller's: the model chip grows, the rest fit.)
pub fn fit<E: Styled>(e: E) -> E { e.h(px(H)).flex().items_center() }

#[cfg(test)]
mod tests { use super::*; use core::prelude::v1::test; #[test] fn one_height() { assert_eq!(H, 32.); } }
