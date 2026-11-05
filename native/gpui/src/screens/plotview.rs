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
    /// The Layers menu is open.
    pub layers_open: bool,
    /// CPU ms of the last canvas paints (bounded), for the e2e `plot_paint` metric.
    pub paint_ms: Vec<f32>,
    /// Where those paints go (ms per paint): the segment/polygon loop (culling + path building), lyon's tessellation (`PathBuilder::build`), and `paint_path` (submission).
    pub split: Vec<[f32; 3]>,
    /// Timing is on only when a script drives the client (the e2e metrics); a production paint takes no timestamps.
    pub measure: bool,
    /// The Layers panel's scroll state and, for the e2e `layers` step, a smaller height cap (px) so a short list scrolls too.
    pub layers_scroll: ScrollHandle, pub layers_cap: Option<f32>,
}

fn color(c: &Chunk, factor: f32) -> Hsla { let mut r = rgb(c.color); r.a = c.alpha * factor; r.into() }

/// A chunk's stroke width in pixels: the hub's width is in micrometres on the capsule (radius = width / 2).
fn paint_chunk(window: &mut Window, c: &Chunk, factor: f32, v: &View, o: Point<Pixels>, w: f32, h: f32, mut sp: Option<&mut [f32; 3]>) {
    // (script mode only) ms since the last lap, added to the culling/tessellation/submission split
    let mut t = sp.as_ref().map(|_| std::time::Instant::now());
    let mut lap = |k: usize, sp: &mut Option<&mut [f32; 3]>| { if let (Some(t), Some(sp)) = (t.as_mut(), sp.as_mut()) { let n = std::time::Instant::now(); sp[k] += n.duration_since(*t).as_secs_f32() * 1000.0; *t = n; } };
    if !c.caps.is_empty() {
        let width = (c.caps[4] * 2. * v.scale).max(1.0);
        let rpx = width / 2.;
        // a dot (a round pad, a via, a hole) is a zero-length capsule: a butt-capped stroke of no length draws nothing, so it is a filled circle;
        // a stroke wide enough to see gets round ends the same way
        let round_ends = width >= 3.0;
        let n = ((6. + rpx * 2.) as usize).clamp(8, 32);
        let circle = |pb: &mut PathBuilder, x: f32, y: f32| {
            for k in 0..n { let t = std::f32::consts::TAU * k as f32 / n as f32; let q = point(o.x + px(x + rpx * t.cos()), o.y + px(y + rpx * t.sin())); if k == 0 { pb.move_to(q) } else { pb.line_to(q) } }
            pb.close();
        };
        let mut pb = PathBuilder::stroke(px(width)); let mut any = false;
        let mut dots = PathBuilder::fill(); let mut any_dot = false;
        let margin = 50. + rpx;
        let near = |x: f32, y: f32| x >= -margin && x <= w + margin && y >= -margin && y <= h + margin;
        for s in c.caps.chunks_exact(5) {
            let (a, b) = (v.to_px(s[0], s[1], w, h), v.to_px(s[2], s[3], w, h));
            if s[0] == s[2] && s[1] == s[3] { if near(a.0, a.1) { circle(&mut dots, a.0, a.1); any_dot = true; } continue; }
            if !near(a.0, a.1) && !near(b.0, b.1) { continue; }
            pb.move_to(point(o.x + px(a.0), o.y + px(a.1)));
            pb.line_to(point(o.x + px(b.0), o.y + px(b.1)));
            any = true;
            if round_ends { circle(&mut dots, a.0, a.1); circle(&mut dots, b.0, b.1); any_dot = true; }
        }
        lap(0, &mut sp);
        if any { if let Ok(path) = pb.build() { lap(1, &mut sp); window.paint_path(path, color(c, factor)); lap(2, &mut sp); } }
        if any_dot { if let Ok(path) = dots.build() { lap(1, &mut sp); window.paint_path(path, color(c, factor)); lap(2, &mut sp); } }
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
        lap(0, &mut sp);
        if any { if let Ok(path) = pb.build() { lap(1, &mut sp); window.paint_path(path, color(c, factor)); lap(2, &mut sp); } }
    }
}

/// The canvas for a frame; its size comes from the layout, the camera from `Screens.plotview`.
pub fn canvas_el(cx: &mut Context<Backplane>, light: bool, frame: &Frame, view: Option<View>, masks: crate::viewers::layers::Masks) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let chunks = frame.chunks.clone();
    // the hub's masks decide which chunks are drawn, dimmed, and which one is painted last
    let order = crate::viewers::layers::plan(&chunks.iter().map(|c| c.layer).collect::<Vec<_>>(), &masks);
    let this = cx.entity();
    let el = canvas(|b, _, _| b, move |_, b, window, cx| {
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        this.update(cx, |app, _| { app.screens.plotview.bounds = Some(b); });
        let Some(v) = view else { return };
        let measure = this.update(cx, |app, _| app.screens.plotview.measure);
        let t0 = measure.then(std::time::Instant::now); let mut sp = [0f32; 3];
        window.with_content_mask(Some(ContentMask { bounds: b }), |window| { for &(i, factor) in &order { paint_chunk(window, &chunks[i], factor, &v, b.origin, w, h, measure.then_some(&mut sp)); } });
        if let Some(t0) = t0 {
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            this.update(cx, |app, _| { let pv = &mut app.screens.plotview; if pv.paint_ms.len() >= 4096 { pv.paint_ms.remove(0); } pv.paint_ms.push(ms); if pv.split.len() >= 4096 { pv.split.remove(0); } pv.split.push(sp); });
        }
        // click to the first painted plot: what the person feels (the metric the gate asks for)
        this.update(cx, |app, _| { if let Some(t) = app.screens.plot_click.take() { app.m("plot_painted", serde_json::json!({"ms": (t.elapsed().as_secs_f64() * 1e6).round() / 1e3, "chunks": chunks.len()})); } });
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
