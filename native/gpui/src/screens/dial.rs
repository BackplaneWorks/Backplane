//! The speed dial in the model picker (issue #1166): a small round knob with one detent per speed the cursor's model takes (Normal, Fast, UltraFast, ...,
//! as the hub sends them: agents/speed.rs). It turns by a drag, the scroll wheel, a click (the next detent, round), a press on a detent's label, or the arrow
//! keys in its column; it always rests on a detent. With Normal only it is drawn disabled and says why on hover.
//! The geometry is pure (tested below); `view` only draws it. Nothing animates: a turn is drawn once.
use gpui::{prelude::*, *};

/// The detents' angles in degrees, clockwise from twelve o'clock: 90° apart, the sweep kept within 240° and centred on twelve.
pub fn angles(n: usize) -> Vec<f32> {
    if n == 0 { return vec![] }
    if n == 1 { return vec![0.] }
    let step = (240. / (n - 1) as f32).min(90.);
    let from = -step * (n - 1) as f32 / 2.;
    (0..n).map(|i| from + step * i as f32).collect()
}
/// The angle of a point seen from the dial's centre, as `angles` measures it (twelve o'clock 0°, clockwise positive, -180..180).
pub fn angle_at(cx: f32, cy: f32, x: f32, y: f32) -> f32 { (x - cx).atan2(cy - y).to_degrees() }
/// The detent nearest an angle (a drag snaps to it).
pub fn nearest(n: usize, deg: f32) -> usize {
    angles(n).iter().enumerate().min_by(|a, b| (a.1 - deg).abs().total_cmp(&(b.1 - deg).abs())).map_or(0, |(i, _)| i)
}
/// One detent on from `i` (`up`: faster), stopping at the ends.
pub fn step(n: usize, i: usize, up: bool) -> usize { if n == 0 { 0 } else if up { (i + 1).min(n - 1) } else { i.saturating_sub(1) } }
/// A click on the knob: the next detent, back to Normal after the last.
pub fn next(n: usize, i: usize) -> usize { if n == 0 { 0 } else { (i + 1) % n } }
/// The scroll wheel, up faster (GPUI's scroll is positive upward): a mouse wheel's notch (any number of lines in one event) is one detent;
/// a trackpad's pixels turn one detent per `NOTCH` px, the rest kept for the next event.
pub fn notch(lines: f32) -> i32 { if lines > 0. { 1 } else if lines < 0. { -1 } else { 0 } }
pub const NOTCH: f32 = 40.;
pub fn wheel(acc: f32, dy: f32) -> (i32, f32) {
    let t = acc + dy;
    let k = (t / NOTCH).trunc();
    (k as i32, t - k * NOTCH)
}
/// A point on the circle of radius `r` around (cx, cy) at angle `deg`.
pub fn on_circle(cx: f32, cy: f32, r: f32, deg: f32) -> (f32, f32) { let a = deg.to_radians(); (cx + r * a.sin(), cy - r * a.cos()) }

/// The knob drawn in a `size` px box: its ring, a tick at each detent (the lit one in `accent`), the pointer to detent `at`. `on` false: dimmed, no pointer.
/// `seen` is told where it was drawn (a drag measures its angle from the centre).
pub fn knob(n: usize, at: usize, size: f32, ring: Rgba, tick: Rgba, accent: Rgba, on: bool, seen: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>) -> impl IntoElement {
    canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
        seen.set(b);
        let (ox, oy) = (f32::from(b.origin.x), f32::from(b.origin.y));
        let (c, r) = (size / 2., size / 2. - 4.);
        let p = |x: f32, y: f32| point(px(ox + x), px(oy + y));
        let stroke = |w: f32| PathBuilder::stroke(px(w));
        // the ring: a polygon fine enough to read as a circle at this size
        let mut pb = stroke(1.5);
        for i in 0..=48 { let (x, y) = on_circle(c, c, r, i as f32 * 7.5); if i == 0 { pb.move_to(p(x, y)) } else { pb.line_to(p(x, y)) } }
        if let Ok(path) = pb.build() { window.paint_path(path, ring); }
        // the detents: a short tick outside the ring each
        for (i, a) in angles(n.max(1)).into_iter().enumerate() {
            let (x0, y0) = on_circle(c, c, r + 1., a); let (x1, y1) = on_circle(c, c, r + 4., a);
            let mut t = stroke(2.); t.move_to(p(x0, y0)); t.line_to(p(x1, y1));
            if let Ok(path) = t.build() { window.paint_path(path, if on && i == at { accent } else { tick }); }
        }
        // the pointer, from near the centre to the ring, at the detent in force
        if on {
            let a = angles(n).get(at).copied().unwrap_or(0.);
            let (x0, y0) = on_circle(c, c, r * 0.2, a); let (x1, y1) = on_circle(c, c, r - 3., a);
            let mut t = stroke(3.); t.move_to(p(x0, y0)); t.line_to(p(x1, y1));
            if let Ok(path) = t.build() { window.paint_path(path, accent); }
        }
    }).size(px(size)).flex_shrink_0()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn detents_sit_on_an_arc_centred_on_twelve() {
        assert_eq!(angles(2), [-45., 45.]); assert_eq!(angles(3), [-90., 0., 90.]); assert_eq!(angles(1), [0.]);
        let a = angles(5); assert_eq!((a[0], a[4]), (-120., 120.), "never past 240° in all");
    }
    #[test] fn a_drag_snaps_to_the_nearest_detent() {
        assert_eq!(nearest(3, -70.), 0); assert_eq!(nearest(3, 20.), 1); assert_eq!(nearest(3, 170.), 2); assert_eq!(nearest(2, -1.), 0); assert_eq!(nearest(2, 1.), 1);
        // the angle a point makes: right of centre is 90°, below is 180°, left is -90°
        assert_eq!(angle_at(10., 10., 20., 10.).round(), 90.); assert_eq!(angle_at(10., 10., 10., 0.).round(), 0.); assert_eq!(angle_at(10., 10., 0., 10.).round(), -90.);
    }
    #[test] fn keys_clicks_and_the_wheel_turn_it_a_detent_at_a_time() {
        assert_eq!(step(3, 0, true), 1); assert_eq!(step(3, 2, true), 2, "stops at the fastest"); assert_eq!(step(3, 0, false), 0, "stops at Normal");
        assert_eq!(next(2, 1), 0, "a click past the last goes back to Normal"); assert_eq!(next(3, 1), 2);
        assert_eq!((notch(3.), notch(-3.), notch(0.)), (1, -1, 0), "a notch is one detent however many lines it scrolls");
        assert_eq!(wheel(0., 40.), (1, 0.)); assert_eq!(wheel(0., -95.), (-2, -15.)); assert_eq!(wheel(30., 15.), (1, 5.), "a trackpad's small steps add up");
    }
}
