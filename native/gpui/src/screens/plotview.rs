//! The board / schematic canvas: the hub's plot chunks (screens::plot) painted as paths with pan (drag) and zoom (wheel).
//! One path per chunk (strokes as line subpaths of the chunk's stroke width, fills as closed polygons), coloured and faded
//! as the hub says, culled to the viewport. Which chunks, their order and colours are the hub's; the view is only a camera.
use super::plot::{Chunk, Frame, View};
use crate::{hub::Command, theme, Backplane};
use gpui::{prelude::*, *};

#[derive(Default)]
pub struct PlotView {
    pub view: Option<View>, pub fitted: f32, pub key: String, pub drag: Option<(f32, f32)>,
    /// The canvas size the camera was fitted to, and whether the person has moved it since: an untouched camera is fitted again when the canvas turns out bigger or smaller than first assumed.
    pub fit_wh: (f32, f32), pub touched: bool,
    /// The canvas's last painted bounds (window pixels), for the pointer.
    pub bounds: Option<Bounds<Pixels>>,
    /// The Layers menu is open; the focus key as the person reads it, for its tooltip.
    pub layers_open: bool, pub layers_key: String,
    /// The layers the plot on show has, a bit each, for the hub (viewers::layers::present_bits).
    pub present: u32,
    /// CPU ms of the last canvas paints (bounded), for the e2e `plot_paint` metric.
    pub paint_ms: Vec<f32>,
    /// Where those paints go (ms per paint): the segment/polygon loop (culling + path building), lyon's tessellation (`PathBuilder::build`), and `paint_path` (submission).
    pub split: Vec<[f32; 3]>,
    /// Timing is on only when a script drives the client (the e2e metrics); a production paint takes no timestamps.
    pub measure: bool,
    /// The Layers panel's scroll state and, for the e2e `layers` step, a smaller height cap (px) so a short list scrolls too.
    pub layers_scroll: ScrollHandle, pub layers_cap: Option<f32>,
    /// Where the left button went down (a click is a press and release within 4 px); the piece last tapped (drawn lit) and the card the hub's info made of it.
    /// The Sheets menu is open (a schematic with more than one sheet).
    pub sheets_open: bool,
    /// Compare / History: the menu is open (1 compare, 2 history); the comparison on show (screens::histbar).
    pub menu: u8, pub compare: Option<super::histbar::Cmp>, pub hist: super::histbar::Hist,
    /// Pieces a cross-probe lights (chunk index, piece info index), and the canvas's focus for the keys.
    pub lit: Vec<(usize, usize)>, pub focus: Option<FocusHandle>,
    pub down: Option<(f32, f32)>, pub picked: Option<super::plot::Pick>, pub card: Option<super::plot::Card>,
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

/// A tap at window position `at` on canvas `slot`: the piece under it is lit and its info asked of the hub (the answer is the card), or, on nothing, the card goes.
pub fn tap_at(app: &mut Backplane, slot: usize, key: &str, off: u32, at: (f32, f32), view: Option<View>, bounds: Option<Bounds<Pixels>>) {
    let (Some(v), Some(b)) = (view, bounds) else { return };
    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
    let (x, y) = v.to_plot(at.0 - f32::from(b.origin.x), at.1 - f32::from(b.origin.y), w, h);
    let found = app.screens.plot.frame_for(key).and_then(|f| super::plot::hit(f, x, y, 14. / v.scale, off));
    let pv = app.screens.pv(slot); pv.card = None;
    for q in [0usize, 1, 2] { app.screens.pv(q).lit.clear(); }
    let pv = app.screens.pv(slot);
    match found {
        Some(p) => { let req = serde_json::json!({"key": key, "at": p.chunk, "idx": p.info}); pv.picked = Some(p); app.screens.pick_slot = Some(slot);
            let _ = app.tx.try_send(Command::Rpc("kicad.info".into(), crate::screens::settings::Secret(req), "pick")); }
        None => { pv.picked = None; }
    }
}

/// The pieces a cross-probe lights (the same net or part as the tapped one, on this plot): each outlined in cyan.
fn paint_probe(window: &mut Window, chunks: &[std::sync::Arc<super::plot::Chunk>], lit: &[(usize, usize)], v: &View, o: Point<Pixels>, w: f32, h: f32) {
    let col: Hsla = rgb(0x4cc9f0).into();
    let mut by: std::collections::BTreeMap<usize, std::collections::HashSet<u32>> = Default::default();
    for (c, i) in lit { by.entry(*c).or_default().insert(*i as u32); }
    let mut pb = PathBuilder::stroke(px(2.0)); let mut any = false;
    for (ci, want) in by {
        let Some(c) = chunks.get(ci) else { continue };
        for p in c.pieces.iter().filter(|p| want.contains(&p.info)) {
            let a = v.to_px(p.bbox[0], p.bbox[1], w, h); let b = v.to_px(p.bbox[2], p.bbox[3], w, h);
            if b.0 < -20. || a.0 > w + 20. || b.1 < -20. || a.1 > h + 20. { continue }
            let (x0, y0, x1, y1) = (a.0.min(b.0) - 2., a.1.min(b.1) - 2., a.0.max(b.0) + 2., a.1.max(b.1) + 2.);
            for (k, (x, y)) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)].into_iter().enumerate() { let q = point(o.x + px(x), o.y + px(y)); if k == 0 { pb.move_to(q) } else { pb.line_to(q) } }
            any = true;
        }
    }
    if any { if let Ok(path) = pb.build() { window.paint_path(path, col); } }
}

/// The tapped piece, outlined in a bright line over everything.
fn paint_lit(window: &mut Window, p: &super::plot::Pick, v: &View, o: Point<Pixels>, w: f32, h: f32) {
    let pts: Vec<(f32, f32)> = p.pts.chunks_exact(2).map(|q| v.to_px(q[0], q[1], w, h)).collect();
    if pts.is_empty() { return }
    let col: Hsla = rgb(0xffe066).into();
    if pts.len() == 1 || p.kind == 2 {
        let r = (p.r * v.scale).max(4.0) + 3.0; let mut pb = PathBuilder::stroke(px(2.0)); let n = 24;
        for k in 0..=n { let t = std::f32::consts::TAU * k as f32 / n as f32; let q = point(o.x + px(pts[0].0 + r * t.cos()), o.y + px(pts[0].1 + r * t.sin())); if k == 0 { pb.move_to(q) } else { pb.line_to(q) } }
        if let Ok(path) = pb.build() { window.paint_path(path, col); }
        return
    }
    let mut pb = PathBuilder::stroke(px(2.5));
    for (i, q) in pts.iter().enumerate() { let q = point(o.x + px(q.0), o.y + px(q.1)); if i == 0 { pb.move_to(q) } else { pb.line_to(q) } }
    if p.kind == 0 { if let Some(f) = pts.first() { pb.line_to(point(o.x + px(f.0), o.y + px(f.1))); } }
    if let Ok(path) = pb.build() { window.paint_path(path, col); }
}

/// The canvas for a frame; its size comes from the layout, the camera from `Screens.plotview`.
/// `slot` is the camera this canvas drives: 0 the Board/Schematic page, 1 and 2 a Split's schematic and board panes (`Screens::pv`).
pub fn canvas_el(cx: &mut Context<Backplane>, slot: usize, focus: Option<FocusHandle>, light: bool, frame: &Frame, view: Option<View>, masks: crate::viewers::layers::Masks) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let chunks = frame.chunks.clone();
    // the hub's masks decide which chunks are drawn, dimmed, and which one is painted last
    let order = crate::viewers::layers::plan(&chunks.iter().map(|c| c.layer).collect::<Vec<_>>(), &masks);
    let this = cx.entity();
    let (key, off) = (frame.key.clone(), masks.off);
    let el = canvas(|b, _, _| b, move |_, b, window, cx| {
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        this.update(cx, |app, _| { app.screens.pv(slot).bounds = Some(b); });
        let Some(v) = view else { return };
        let measure = this.update(cx, |app, _| app.screens.pv(slot).measure);
        let t0 = measure.then(std::time::Instant::now); let mut sp = [0f32; 3];
        let (lit, probe) = this.update(cx, |app, _| { let pv = app.screens.pv(slot); (pv.picked.clone(), pv.lit.clone()) });
        window.with_content_mask(Some(ContentMask { bounds: b }), |window| {
            for &(i, factor) in &order { paint_chunk(window, &chunks[i], factor, &v, b.origin, w, h, measure.then_some(&mut sp)); }
            if !probe.is_empty() { paint_probe(window, &chunks, &probe, &v, b.origin, w, h); }
            if let Some(p) = &lit { paint_lit(window, p, &v, b.origin, w, h); }
        });
        if let Some(t0) = t0 {
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            this.update(cx, |app, _| { let pv = &mut app.screens.pv(slot); if pv.paint_ms.len() >= 4096 { pv.paint_ms.remove(0); } pv.paint_ms.push(ms); if pv.split.len() >= 4096 { pv.split.remove(0); } pv.split.push(sp); });
        }
        // click to the first painted plot: what the person feels (the metric the gate asks for)
        this.update(cx, |app, _| { if let Some(t) = app.screens.plot_click.take() { app.m("plot_painted", serde_json::json!({"ms": (t.elapsed().as_secs_f64() * 1e6).round() / 1e3, "chunks": chunks.len()})); } });
    }).size_full();
    div().id(SharedString::from(format!("plot-{slot}"))).flex_1().min_h_0().bg(c("view")).child(el)
        .when_some(focus, |d, f| d.track_focus(&f).key_context("Viewer").on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
            // keys on the plot: arrows move the view, + and - zoom at the centre, 0 or Home fits again, Escape puts the tapped piece away
            let pv = this.screens.pv(slot);
            let (Some(v), Some(b)) = (pv.view, pv.bounds) else { return };
            let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
            let nv = match e.keystroke.key.as_str() {
                "left" => Some(v.pan(60., 0.)), "right" => Some(v.pan(-60., 0.)), "up" => Some(v.pan(0., 60.)), "down" => Some(v.pan(0., -60.)),
                "+" | "=" | "plus" => Some(v.zoom(1.25, w / 2., h / 2., w, h, pv.fitted)), "-" | "minus" => Some(v.zoom(0.8, w / 2., h / 2., w, h, pv.fitted)),
                "0" | "home" => { pv.view = None; cx.notify(); return }
                "escape" => { pv.picked = None; pv.card = None; pv.lit.clear(); cx.notify(); return }
                _ => None,
            };
            if let Some(nv) = nv { pv.touched = true; pv.view = Some(nv); cx.notify(); }
        })))
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, window, cx| { if let Some(f) = this.screens.pv(slot).focus.clone() { window.focus(&f); } let at = (f32::from(e.position.x), f32::from(e.position.y)); let pv = this.screens.pv(slot); pv.drag = Some(at); pv.down = Some(at); cx.notify(); }))
        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
            let pv = this.screens.pv(slot);
            if let (Some((x, y)), Some(v), true) = (pv.drag, pv.view, e.dragging()) { let (nx, ny) = (f32::from(e.position.x), f32::from(e.position.y)); pv.touched = true; pv.view = Some(v.pan(nx - x, ny - y)); pv.drag = Some((nx, ny)); cx.notify(); }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(move |this, e: &MouseUpEvent, _, cx| {
            // a press and release within 4 px is a tap: inspect what lies under it (View.pick: the finger's reach is 14 px)
            let up = (f32::from(e.position.x), f32::from(e.position.y));
            // a double-click fits the plot again
            if e.click_count >= 2 { let pv = this.screens.pv(slot); pv.down = None; pv.drag = None; pv.view = None; cx.notify(); return }
            let (tap, view, bounds) = { let pv = this.screens.pv(slot); pv.drag = None; (pv.down.take().filter(|d| (d.0 - up.0).abs() < 4. && (d.1 - up.1).abs() < 4.), pv.view, pv.bounds) };
            if tap.is_some() { tap_at(this, slot, &key, off, up, view, bounds); }
            cx.notify(); }))
        .on_mouse_up_out(MouseButton::Left, cx.listener(move |this, _, _, cx| { this.screens.pv(slot).drag = None; cx.notify(); }))
        .on_scroll_wheel(cx.listener(move |this, e: &ScrollWheelEvent, _, cx| {
            let pv = this.screens.pv(slot);
            let (Some(v), Some(b)) = (pv.view, pv.bounds) else { return };
            // wheelKind (src/web/): a touchpad's two-finger scroll (pixel deltas) moves the plot, a mouse wheel (lines) and a pinch (ctrl + wheel) zoom
            let pad = matches!(e.delta, ScrollDelta::Pixels(_)) && !e.modifiers.control;
            if pad { let d = e.delta.pixel_delta(px(16.)); pv.view = Some(v.pan(-f32::from(d.x), -f32::from(d.y))); cx.notify(); return }
            let d = f32::from(e.delta.pixel_delta(px(16.)).y);
            let factor = (1.0 + d * 0.0015).clamp(0.5, 2.0); pv.touched = true;
            pv.view = Some(v.zoom(factor, f32::from(e.position.x - b.origin.x), f32::from(e.position.y - b.origin.y), f32::from(b.size.width), f32::from(b.size.height), pv.fitted));
            cx.notify();
        })).into_any_element()
}

/// Whether an untouched camera must be fitted again: the canvas is not the size it was fitted to (the first paint's size was a guess).
pub fn refit_needed(touched: bool, fit_wh: (f32, f32), w: f32, h: f32) -> bool { !touched && ((fit_wh.0 - w).abs() > 2. || (fit_wh.1 - h).abs() > 2.) }

/// Fit the camera to the frame at this canvas size, and forget any pan or zoom.
pub fn fit_now(pv: &mut PlotView, frame: &Frame, w: f32, h: f32) {
    let v = View::fit(frame.bbox, w.max(10.), h.max(10.));
    pv.fitted = v.scale; pv.view = Some(v); pv.fit_wh = (w, h); pv.touched = false;
}

/// Keep the camera fitted to a new frame (a different file): called with the frame and the canvas size.
pub fn ensure_view(pv: &mut PlotView, frame: &Frame, w: f32, h: f32) -> Option<View> {
    if frame.none.is_empty() && (pv.key != frame.fam || pv.view.is_none()) {
        pv.key = frame.fam.clone(); fit_now(pv, frame, w, h); pv.picked = None; pv.card = None; pv.lit.clear(); // another file: what was tapped in the last is not in this
    } else if frame.none.is_empty() && refit_needed(pv.touched, pv.fit_wh, w, h) { fit_now(pv, frame, w, h); }
    pv.view
}

/// A tapped piece's net and part, lit on every plot the page shows (the web's cross-probe): a part by its `ref` on the schematic and the board, a net by its name on the board
/// (the schematic's info has no nets). The hub answers with the pieces of the chunks this client holds (`kicad.probe`); they arrive as `Event::Probe` per canvas.
pub fn cross_probe(app: &mut Backplane, slot: usize, card: &super::plot::Card) {
    let get = |k: &str| card.rows.iter().find(|r| r.0 == k).map(|r| r.1.clone()).filter(|v| !v.is_empty());
    let (refd, net) = (get("ref"), get("net"));
    let panes = app.data["thread"]["viewer"]["panes"].as_array().cloned().unwrap_or_default();
    let targets: Vec<(usize, String)> = if panes.len() >= 3 { vec![(1, panes[0]["key"].as_str().unwrap_or("").to_string()), (2, panes[1]["key"].as_str().unwrap_or("").to_string())] }
        else { vec![(slot, app.screens.plot.frame.as_ref().map(|f| f.key.clone()).unwrap_or_default())] };
    for (s, key) in targets {
        if key.is_empty() { continue }
        let tag: &'static str = ["probe0", "probe1", "probe2"][s.min(2)];
        for (name, value) in [("ref", refd.clone()), ("net", net.clone())] {
            let Some(value) = value else { continue };
            if name == "net" && !key.starts_with("0|") { continue }
            let _ = app.tx.try_send(Command::Rpc("kicad.probe".into(), crate::screens::settings::Secret(serde_json::json!({"key": key, "name": name, "value": value})), tag));
        }
    }
}

/// What the viewer shows now, for the hub's `viewer.state` (an agent's viewer_show answers with it): the page, its source, the part (Mech), the sheet (schematic), the active layer and the tapped piece. Sent only on change.
pub fn report_state(app: &mut Backplane) {
    let v = app.data["thread"]["viewer"].clone();
    let open = v["open"].as_str().unwrap_or("").to_string();
    let key = v["key"].as_str().unwrap_or("").to_string();
    let card = [0usize, 1, 2].iter().find_map(|i| app.screens.pv(*i).card.clone());
    let layer = v["layerHead"].as_str().filter(|h| h.starts_with("Layer")).map(|h| h.trim_start_matches("Layer: ").to_string());
    let lit: usize = (0..3).map(|i| app.screens.pv(i).lit.len()).sum();
    let state = serde_json::json!({"open": open, "key": key,
        "part": if open == "mech" { v["mech"]["rel"].clone() } else { serde_json::Value::Null },
        "sheet": if open == "schematic" { serde_json::json!(key.splitn(3, '|').nth(2).unwrap_or("")) } else { serde_json::Value::Null },
        "layer": layer, "layerMode": v["layerMode"],
        "selection": card.map(|c| serde_json::json!({"title": c.title, "rows": c.rows.iter().map(|r| serde_json::json!([r.0, r.1])).collect::<Vec<_>>()})),
        "lit": lit});
    let text = state.to_string();
    if text == app.screens.last_state || app.selected.is_empty() { return }
    app.screens.last_state = text;
    let _ = app.tx.try_send(Command::Rpc("viewer.state".into(), crate::screens::settings::Secret(serde_json::json!({"thread": app.selected, "state": state})), "vstate"));
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn an_untouched_camera_is_fitted_again_when_the_canvas_size_changes() {
        assert!(refit_needed(false, (480., 600.), 520., 930.));
        assert!(!refit_needed(false, (520., 930.), 521., 929.), "a pixel of jitter is not a change");
        assert!(!refit_needed(true, (480., 600.), 520., 930.), "a camera the person moved stays where they left it");
    }
}
