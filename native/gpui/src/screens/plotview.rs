//! The board / schematic canvas: the hub's plot chunks (screens::plot) painted as paths with pan (drag) and zoom (wheel).
//! One path per chunk (strokes as line subpaths of the chunk's stroke width, fills as closed polygons), coloured and faded
//! as the hub says, culled to the viewport. Which chunks, their order and colours are the hub's; the view is only a camera.
use super::plot::{Chunk, Frame, View};
use crate::{theme, Backplane};
use gpui::{prelude::*, *};

#[derive(Default)]
pub struct PlotView {
    pub view: Option<View>, pub fitted: f32, pub key: String, pub drag: Option<(f32, f32)>,
    /// The canvas's last painted bounds (window pixels), for the pointer.
    pub bounds: Option<Bounds<Pixels>>,
}

fn color(c: &Chunk) -> Hsla { let mut r = rgb(c.color); r.a = c.alpha; r.into() }

/// A chunk's stroke width in pixels: the hub's width is in micrometres on the capsule (radius = width / 2).
fn paint_chunk(window: &mut Window, c: &Chunk, v: &View, o: Point<Pixels>, w: f32, h: f32) {
    let on = |x: f32, y: f32| { let (px, py) = v.to_px(x, y, w, h); (px >= -50. && px <= w + 50. && py >= -50. && py <= h + 50.).then_some((px, py)) };
    if !c.caps.is_empty() {
        let width = (c.caps[4] * 2. * v.scale).max(1.0);
        let mut pb = PathBuilder::stroke(px(width)); let mut any = false;
        for s in c.caps.chunks_exact(5) {
            let (a, b) = (v.to_px(s[0], s[1], w, h), v.to_px(s[2], s[3], w, h));
            if on(s[0], s[1]).is_none() && on(s[2], s[3]).is_none() { continue; }
            pb.move_to(point(o.x + px(a.0), o.y + px(a.1)));
            if (a.0 - b.0).abs() + (a.1 - b.1).abs() < 0.5 { pb.line_to(point(o.x + px(b.0 + 0.5), o.y + px(b.1))); } else { pb.line_to(point(o.x + px(b.0), o.y + px(b.1))); }
            any = true;
        }
        if any { if let Ok(path) = pb.build() { window.paint_path(path, color(c)); } }
    }
    if !c.polys.is_empty() {
        let mut pb = PathBuilder::fill(); let mut any = false;
        for poly in &c.polys {
            let pts: Vec<(f32, f32)> = poly.chunks_exact(2).map(|p| v.to_px(p[0], p[1], w, h)).collect();
            let (minx, maxx) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
            let (miny, maxy) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
            if maxx < -50. || minx > w + 50. || maxy < -50. || miny > h + 50. { continue; }
            for (i, p) in pts.iter().enumerate() { let q = point(o.x + px(p.0), o.y + px(p.1)); if i == 0 { pb.move_to(q) } else { pb.line_to(q) } }
            pb.close(); any = true;
        }
        if any { if let Ok(path) = pb.build() { window.paint_path(path, color(c)); } }
    }
}

/// The canvas for a frame; its size comes from the layout, the camera from `Screens.plotview`.
pub fn canvas_el(cx: &mut Context<Backplane>, light: bool, frame: &Frame, view: Option<View>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let chunks = frame.chunks.clone();
    let this = cx.entity();
    let el = canvas(|b, _, _| b, move |_, b, window, cx| {
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        this.update(cx, |app, _| { app.screens.plotview.bounds = Some(b); });
        let Some(v) = view else { return };
        window.with_content_mask(Some(ContentMask { bounds: b }), |window| { for ch in &chunks { paint_chunk(window, ch, &v, b.origin, w, h); } });
    }).size_full();
    div().id("plot").flex_1().min_h_0().bg(c("view")).child(el)
        .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, cx| { this.screens.plotview.drag = Some((f32::from(e.position.x), f32::from(e.position.y))); cx.notify(); }))
        .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
            let pv = &mut this.screens.plotview;
            if let (Some((x, y)), Some(v), true) = (pv.drag, pv.view, e.dragging()) { let (nx, ny) = (f32::from(e.position.x), f32::from(e.position.y)); pv.view = Some(v.pan(nx - x, ny - y)); pv.drag = Some((nx, ny)); cx.notify(); }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| { this.screens.plotview.drag = None; cx.notify(); }))
        .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| { this.screens.plotview.drag = None; cx.notify(); }))
        .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
            let pv = &mut this.screens.plotview;
            let (Some(v), Some(b)) = (pv.view, pv.bounds) else { return };
            let d = f32::from(e.delta.pixel_delta(px(16.)).y);
            let factor = (1.0 + d * 0.0015).clamp(0.5, 2.0);
            pv.view = Some(v.zoom(factor, f32::from(e.position.x - b.origin.x), f32::from(e.position.y - b.origin.y), f32::from(b.size.width), f32::from(b.size.height), pv.fitted));
            cx.notify();
        })).into_any_element()
}

/// Keep the camera fitted to a new frame (a different file): called with the frame and the canvas size.
pub fn ensure_view(pv: &mut PlotView, frame: &Frame, w: f32, h: f32) -> Option<View> {
    if frame.none.is_empty() && (pv.key != frame.fam || pv.view.is_none()) {
        let v = View::fit(frame.bbox, w.max(10.), h.max(10.));
        pv.key = frame.fam.clone(); pv.fitted = v.scale; pv.view = Some(v);
    }
    pv.view
}
