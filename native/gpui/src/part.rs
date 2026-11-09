//! Regions of the window drawn by their own cached views, so a frame that another region asked for reuses their last layout and paint.
//!
//! GPUI draws a frame from the root view down, and the root (`Backplane`) builds the whole window. A hover that changes a style, the
//! composer's caret blinking, a scroll, a cat's animation: each asks for a frame, and before this every such frame rebuilt and laid out
//! the sidebar and the thread's visible messages too (25 ms of CPU a frame on the person's real data, 2 such frames a second at idle).
//! A part is a view whose render is one of `Backplane`'s methods. GPUI reuses a cached view's previous frame unless that view was notified
//! (or the window refreshed), and a press, hover or animation inside a part notifies only that part (and re-renders the root's shell).
//! Every notify of `Backplane` (any change of what is shown) re-renders every part, exactly as before, so nothing can show stale state.
//! Check runs (`BACKPLANE_GPUI_RECTS`) draw parts uncached: their probes and control rectangles are recorded at paint, which a reused frame skips.
use crate::{script::CtlEntry, Backplane};
use gpui::{prelude::*, *};
use std::time::Duration;

pub type Draw = fn(&mut Backplane, &mut Window, &mut Context<Backplane>) -> AnyElement;

pub struct Part { root: WeakEntity<Backplane>, draw: Draw }
impl Render for Part {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let draw = self.draw;
        self.root.update(cx, |b, cx| draw(b, window, cx)).unwrap_or_else(|_| Empty.into_any_element())
    }
}

/// The window's parts, made on first use, and the controls each drew (the e2e inventory; the root's own are `Backplane::controls`).
#[derive(Default)]
pub struct Parts { views: Vec<(Draw, Entity<Part>)>, pub side_ctl: Vec<CtlEntry>, pub rows_ctl: Vec<CtlEntry>, ticking: Vec<usize> }
impl Parts {
    /// The part drawing `draw`, as an element whose root is laid out with `style` (the part's own root element fills it).
    pub fn el(&mut self, draw: Draw, style: StyleRefinement, cx: &mut Context<Backplane>) -> AnyElement {
        let view = match self.views.iter().find(|(d, _)| *d as usize == draw as usize) {
            Some((_, v)) => v.clone(),
            None => {
                let root = cx.entity();
                let v = cx.new(|cx: &mut Context<Part>| { cx.observe(&root, |_, _, cx| cx.notify()).detach(); Part { root: root.downgrade(), draw } });
                self.views.push((draw, v.clone())); v
            }
        };
        let any = AnyView::from(view);
        if crate::art::rects_on() { let mut d = div(); d.style().refine(&style); d.child(any).into_any_element() } else { any.cached(style).into_any_element() }
    }
}

impl Parts {
    /// Draw `draw`'s part again in `after` (a low-rate animation: that part and the root's shell, not the window 60 times a second). One wait at a time.
    pub fn tick(&mut self, draw: Draw, after: Duration, cx: &mut Context<Backplane>) {
        let key = draw as usize;
        let Some(view) = self.views.iter().find(|(d, _)| *d as usize == key).map(|(_, v)| v.clone()) else { return };
        if self.ticking.contains(&key) { return }
        self.ticking.push(key);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(after).await;
            let _ = this.update(cx, |b, cx| { b.screens.parts.ticking.retain(|k| *k != key); view.update(cx, |_, cx| cx.notify()); });
        }).detach();
    }
}

/// The "Working..." row's rate: 12 steps a second.
pub const PULSE: Duration = Duration::from_millis(83);
/// The working square's breath (0..1, 1.1 s there and back, eased) and the dots' count (1..3, 1.8 s round) at `t`.
pub fn pulse(t: Duration) -> (f32, usize) {
    let ms = t.as_millis() as u64;
    let d = bounce(ease_in_out)((ms % 1100) as f32 / 1100.0);
    (d, 1 + (((ms % 1800) as f32 / 1800.0 * 3.0) as usize).min(2))
}

impl Backplane {
    /// Every control drawn (the e2e inventory, G4): the sidebar's, the root's, then the rows'.
    pub fn all_controls(&self) -> impl Iterator<Item = &CtlEntry> { self.screens.parts.side_ctl.iter().chain(&self.controls).chain(&self.screens.parts.rows_ctl) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn the_pulse_breathes_and_the_dots_count_up() {
        let at = |ms| pulse(Duration::from_millis(ms));
        assert_eq!(at(0), (0.0, 1)); assert!((at(550).0 - 1.0).abs() < 1e-3, "full at half the breath"); assert_eq!(at(1100).0, 0.0);
        assert_eq!((at(600).1, at(1200).1, at(1799).1, at(1800).1), (2, 3, 3, 1));
    }
}
