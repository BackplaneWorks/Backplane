//! A thin scrollbar for the scrolling areas a person reads (the thread, documents, the long lists): where you are and how long it is, and
//! a thumb you can drag. 6 px wide, flush with the pane's right edge, no track, square; the theme's `line`, `dim` under the pointer, `accent`
//! while dragged. Shown only when there is something to scroll. It is laid over the pane's right gutter: nothing else moves.
//! Drawn again only when the list scrolls or the pointer enters, leaves or drags it (no animation, no repaint loop).
//! One drag at a time, held at window level from the press to the release (as the dividers do), so it keeps working off the bar.
use gpui::{canvas, div, point, px, AnyElement, Bounds, CursorStyle, DispatchPhase, HitboxBehavior, IntoElement, ListState, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, ScrollHandle, Styled};
use std::cell::RefCell;

/// The thumb's width, the strip the pointer finds it in, and the shortest thumb.
pub const WIDTH: f32 = 6.;
pub const STRIP: f32 = 10.;
pub const MIN_THUMB: f32 = 24.;

/// The thumb's top and height in a track `viewport_h` tall, for content `max_offset` taller than the track scrolled down by `offset`.
/// None when there is nothing to scroll.
pub fn thumb(viewport_h: f32, max_offset: f32, offset: f32) -> Option<(f32, f32)> {
    if !(viewport_h > 0.) || !(max_offset >= 1.) { return None }
    let content = viewport_h + max_offset;
    let h = (viewport_h * viewport_h / content).max(MIN_THUMB).min(viewport_h);
    let room = viewport_h - h;
    let top = (offset.clamp(0., max_offset) / max_offset) * room;
    Some((top, h))
}

/// The offset that puts the thumb's top at `top` (the inverse of `thumb`), inside 0..=max_offset.
pub fn offset_for_thumb_top(viewport_h: f32, max_offset: f32, top: f32) -> f32 {
    let Some((_, h)) = thumb(viewport_h, max_offset, 0.) else { return 0. };
    let room = viewport_h - h;
    if room <= 0. { return 0. }
    (top / room * max_offset).clamp(0., max_offset)
}

/// One viewport up (`dir` < 0) or down (`dir` > 0) from `offset`, as PgUp/PgDn.
pub fn page(offset: f32, viewport_h: f32, max_offset: f32, dir: i32) -> f32 {
    (offset + dir.signum() as f32 * viewport_h).clamp(0., max_offset.max(0.))
}

/// What scrolls: a list that draws only the rows on screen, or a plain scrolling element.
#[derive(Clone)]
pub enum Src { List(ListState), Handle(ScrollHandle) }

impl Src {
    /// How far down it is scrolled (px, positive).
    fn offset(&self) -> f32 {
        match self { Src::List(l) => -f32::from(l.scroll_px_offset_for_scrollbar().y), Src::Handle(h) => -f32::from(h.offset().y) }
    }
    fn max(&self) -> f32 {
        match self { Src::List(l) => f32::from(l.max_offset_for_scrollbar().height), Src::Handle(h) => f32::from(h.max_offset().height) }
    }
    /// Scroll to `o`. A bottom-aligned list given its end follows the newest rows again; anywhere above it stops following, as the wheel does.
    fn set(&self, o: f32) {
        match self {
            Src::List(l) => l.set_offset_from_scrollbar(point(px(0.), px(-o))),
            Src::Handle(h) => { let x = h.offset().x; h.set_offset(point(x, px(-o.clamp(0., f32::from(h.max_offset().height))))) }
        }
    }
    /// The list keeps the height it had when the drag began, so rows measured on the way do not make the thumb jump.
    fn dragging(&self, on: bool) {
        if let Src::List(l) = self { if on { l.scrollbar_drag_started() } else { l.scrollbar_drag_ended() } }
    }
}

/// The drag going on: which bar, where in the thumb it was taken, and what it scrolls.
struct Drag { key: &'static str, grab: f32, src: Src }
thread_local! { static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) }; }

fn dragging(key: &str) -> bool { DRAG.with(|d| d.borrow().as_ref().map_or(false, |d| d.key == key)) }
/// The drag's end: on any release of the left button, wherever it happens.
fn end_drag() -> bool {
    match DRAG.with(|d| d.borrow_mut().take()) { Some(d) => { d.src.dragging(false); true } None => false }
}

/// The bar, over the right edge of its parent (which must be `relative` and as tall as what scrolls). `key` names the bar (one drag at a time).
pub fn bar(key: &'static str, src: Src, light: bool) -> AnyElement {
    let colour = |name: &str| crate::theme::color(light, name);
    let (idle, hot, held) = (colour("line"), colour("dim"), colour("accent"));
    let probe = src.clone();
    div().absolute().top_0().bottom_0().right_0().w(px(STRIP)).child(canvas(
        move |b: Bounds<Pixels>, window, _| {
            let shown = thumb(f32::from(b.size.height), probe.max(), probe.offset()).is_some() || dragging(key);
            shown.then(|| window.insert_hitbox(b, HitboxBehavior::BlockMouseExceptScroll))
        },
        move |b, hitbox, window, _| {
            let Some(hitbox) = hitbox else { return };
            let track_h = f32::from(b.size.height);
            let (max, off) = (src.max(), src.offset());
            let Some((top, h)) = thumb(track_h, max, off) else { return };
            let held_now = dragging(key);
            let hovered = hitbox.is_hovered(window);
            let fill = if held_now { held } else if hovered { hot } else { idle };
            let x = b.origin.x + b.size.width - px(WIDTH);
            window.paint_quad(gpui::fill(Bounds::new(point(x, b.origin.y + px(top)), gpui::size(px(WIDTH), px(h))), fill));
            window.set_cursor_style(CursorStyle::Arrow, &hitbox);
            let y0 = f32::from(b.origin.y);
            // press: on the thumb it is taken; above or below it, a page that way
            let (s, hb) = (src.clone(), hitbox.clone());
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || e.button != MouseButton::Left || !hb.is_hovered(window) { return }
                cx.stop_propagation();
                let y = f32::from(e.position.y) - y0;
                if y >= top && y <= top + h {
                    end_drag();
                    s.dragging(true);
                    DRAG.with(|d| *d.borrow_mut() = Some(Drag { key, grab: y - top, src: s.clone() }));
                } else {
                    s.set(page(off, track_h, max, if y < top { -1 } else { 1 }));
                }
                window.refresh();
            });
            // the pointer over the strip or not: drawn again only when that changes
            let hb = hitbox.clone();
            window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, _| {
                if phase == DispatchPhase::Bubble && hb.is_hovered(window) != hovered { window.refresh(); }
            });
            window.on_mouse_event(move |e: &MouseUpEvent, phase, window, _| {
                if phase == DispatchPhase::Capture && e.button == MouseButton::Left && end_drag() { window.refresh(); }
            });
            if held_now {
                let s = src.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture { return }
                    let Some(grab) = DRAG.with(|d| d.borrow().as_ref().filter(|d| d.key == key).map(|d| d.grab)) else { return };
                    cx.stop_propagation();
                    let o = offset_for_thumb_top(track_h, max, f32::from(e.position.y) - y0 - grab);
                    if (o - s.offset()).abs() >= 0.5 { s.set(o); window.refresh(); }
                });
            }
        },
    ).size_full()).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn nothing_to_scroll_no_thumb() {
        assert_eq!(thumb(500., 0., 0.), None); assert_eq!(thumb(500., 0.4, 0.), None); assert_eq!(thumb(0., 100., 0.), None);
    }
    #[test] fn thumb_is_the_share_on_screen_and_follows_the_offset() {
        assert_eq!(thumb(500., 500., 0.), Some((0., 250.)));
        assert_eq!(thumb(500., 500., 500.), Some((250., 250.)));
        assert_eq!(thumb(500., 500., 250.), Some((125., 250.)));
    }
    #[test] fn long_content_keeps_a_thumb_you_can_grab_and_offsets_clamp() {
        let (top, h) = thumb(400., 100_000., 100_000.).unwrap();
        assert_eq!(h, MIN_THUMB); assert_eq!(top, 400. - MIN_THUMB);
        assert_eq!(thumb(400., 1000., 5000.).unwrap().0, thumb(400., 1000., 1000.).unwrap().0);
        assert_eq!(thumb(400., 1000., -30.).unwrap().0, 0.);
    }
    #[test] fn thumb_top_back_to_the_offset() {
        for off in [0., 10., 333., 999., 1000.] {
            let (top, _) = thumb(400., 1000., off).unwrap();
            assert!((offset_for_thumb_top(400., 1000., top) - off).abs() < 0.01, "{off}");
        }
        assert_eq!(offset_for_thumb_top(400., 1000., -50.), 0.);
        assert_eq!(offset_for_thumb_top(400., 1000., 9000.), 1000.);
        assert_eq!(offset_for_thumb_top(400., 0., 10.), 0.);
    }
    #[test] fn a_page_is_one_viewport_inside_the_ends() {
        assert_eq!(page(100., 400., 1000., 1), 500.); assert_eq!(page(500., 400., 1000., -1), 100.);
        assert_eq!(page(900., 400., 1000., 1), 1000.); assert_eq!(page(100., 400., 1000., -1), 0.);
        assert_eq!(page(0., 400., 0., 1), 0.);
    }
}
