//! The board / schematic canvas: the hub's plot chunks (screens::plot) painted as paths with pan (drag) and zoom (wheel).
//! One path per chunk (strokes as line subpaths of the chunk's stroke width, fills as closed polygons), coloured and faded
//! as the hub says, culled to the viewport. Which chunks, their order and colours are the hub's; the view is only a camera.
use super::plot::{Chunk, Frame, View};
use crate::{hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use crate::mouse::{Action, Scroll};

#[derive(Default)]
pub struct PlotView {
    pub view: Option<View>, pub fitted: f32, pub key: String, pub drag: Option<(f32, f32)>, /** what the drag now held does (the table's), and the button that holds it */ pub drag_act: Option<Action>, pub drag_btn: Option<MouseButton>,
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
    /// Its search, highlighted row and scroll (screens::sheets).
    pub sheets: super::sheets::SheetsUi,
    /// Compare / History: the menu is open (1 compare, 2 history); the comparison on show (screens::histbar).
    pub menu: u8, pub compare: Option<super::histbar::Cmp>, pub hist: super::histbar::Hist,
    /// Pieces a cross-probe lights (chunk index, piece info index), and the canvas's focus for the keys.
    pub lit: Vec<(usize, usize)>, pub focus: Option<FocusHandle>,
    pub down: Option<(f32, f32)>, pub picked: Option<super::plot::Pick>, pub card: Option<super::plot::Card>,
    /// The plot as last tessellated, and what it was made from: a paint of the same chunks at the same zoom and canvas size submits these
    /// again, moved to where the camera is now, instead of tessellating every chunk (tens of ms on a real board) for a frame that only moved
    /// the plot (a drag) or something else. `tess_ahead`: the plot is being tessellated around the camera off the window's thread.
    pub tess: Option<(TessKey, std::rc::Rc<Vec<(Path<Pixels>, Hsla)>>)>, pub tess_ahead: bool,
    /// What the probe lights, tessellated for that exact camera and those lit pieces (a hash of `lit`).
    pub lit_tess: Option<(TessKey, u64, std::rc::Rc<Vec<(Path<Pixels>, Hsla)>>)>,
    /// The Boards menu (viewers::boards), held on the Board/Schematic page's camera for every page that shows it.
    pub boards: crate::viewers::boards::BoardsUi,
    /// The hub's probe this canvas last lit, with the source it was lit on ("<key>\u{1}<info>"): another source or another probe lights it again.
    pub probe_at: String,
    /// Follow agent: how many times a person's hand moved this canvas (screens::follow), and the focus the camera takes once its plot is drawn.
    pub hands: u32, pub follow_to: Option<([f32; 4], serde_json::Value)>,
    /// Per-window loading elapsed-time clock (scoped to this Backplane entity, never thread-local).
    pub loading: super::panestate::LoadingClock,
    /// Last Retry press time on this window (enforces the 1 s client cooldown).
    pub last_retry: Option<std::time::Instant>,
}

/// What a tessellated plot depends on: the frame's chunks (by identity), the order and fade they are drawn in, the zoom and the canvas's
/// size; and where it was made: the camera's centre and the canvas's corner (`at`), and how far past the canvas its paths reach (`pad`, px).
#[derive(Clone, PartialEq, Debug)]
pub struct TessKey { chunks: Vec<usize>, order: Vec<(usize, u32)>, scale: u32, size: [u32; 2], at: [f32; 4], pad: f32 }
impl TessKey {
    pub fn of(chunks: &[std::sync::Arc<Chunk>], order: &[(usize, f32)], v: &View, b: Bounds<Pixels>, pad: f32) -> TessKey {
        TessKey { chunks: chunks.iter().map(|c| std::sync::Arc::as_ptr(c) as usize).collect(), order: order.iter().map(|&(i, f)| (i, f.to_bits())).collect(), scale: v.scale.to_bits(),
            size: [f32::from(b.size.width).to_bits(), f32::from(b.size.height).to_bits()], at: [v.cx, v.cy, f32::from(b.origin.x), f32::from(b.origin.y)], pad }
    }
    /// The same plot drawn the same way (chunks, order, fades, zoom, canvas size): only where the camera and the canvas are may differ.
    pub fn same_plot(&self, o: &TessKey) -> bool { self.chunks == o.chunks && self.order == o.order && self.scale == o.scale && self.size == o.size }
    /// How far (px) the paths made for `self` are moved to be the paint `now` asks for, or None when they cannot be: another plot, zoom or
    /// canvas size, or the camera has gone further than they reach past the canvas.
    pub fn moved(&self, now: &TessKey) -> Option<(f32, f32)> {
        if !self.same_plot(now) { return None }
        let s = f32::from_bits(self.scale);
        let d = ((self.at[0] - now.at[0]) * s + now.at[2] - self.at[2], (self.at[1] - now.at[1]) * s + now.at[3] - self.at[3]);
        (d.0.abs() <= self.pad && d.1.abs() <= self.pad).then_some(d)
    }
}
/// How far past the canvas (px, each side) the plot is tessellated off the window's thread: a drag moves those paths until the camera has
/// gone half of it, then the plot around the camera is made again, off the thread too.
pub fn tess_pad(w: f32, h: f32) -> f32 { (w.max(h) * 0.5).clamp(200., 1200.) }
/// Every chunk of the plot as paths, in the order and at the fades given, culled to the canvas and `pad` px around it.
/// Also whether that is the whole plot (nothing lay outside): paths that then serve the camera wherever it goes at this zoom.
fn tessellate(chunks: &[std::sync::Arc<Chunk>], order: &[(usize, f32)], v: &View, o: Point<Pixels>, w: f32, h: f32, pad: f32, mut sp: Option<&mut [f32; 3]>) -> (Vec<(Path<Pixels>, Hsla)>, bool) {
    let mut out = Vec::new(); let mut culled = false;
    for &(i, factor) in order { culled |= paint_chunk(&mut out, &chunks[i], factor, v, o, w, h, pad, sp.as_deref_mut()); }
    (out, !culled)
}
/// The plot tessellated around where the camera is now, off the window's thread (one at a time): the paints that follow move its paths
/// (`TessKey::moved`) instead of making them, so a drag costs the window's thread no tessellation.
fn tess_ahead(this: &Entity<Backplane>, cx: &mut App, slot: usize, chunks: &[std::sync::Arc<Chunk>], order: &[(usize, f32)], v: View, b: Bounds<Pixels>) {
    this.update(cx, |app, cx| {
        let pv = app.screens.pv(slot);
        if pv.tess_ahead { return }
        let source = pv.key.clone();
        pv.tess_ahead = true;
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        let pad = tess_pad(w, h); let mut key = TessKey::of(chunks, order, &v, b, pad);
        let (chunks, order) = (chunks.to_vec(), order.to_vec());
        let owner = app.screens.plots.owner.clone();
        cx.spawn(async move |weak, cx| {
            let (out, whole) = cx.background_executor().spawn(async move { tessellate(&chunks, &order, &v, b.origin, w, h, pad, None) }).await;
            if whole { key.pad = f32::INFINITY; }
            let _ = weak.update(cx, |app, cx| {
                if app.screens.plots.owner != owner || app.screens.plots[slot].key != source { return }
                let pv = app.screens.pv(slot); pv.tess_ahead = false;
                // (a plot, zoom or canvas that changed meanwhile: these paths are of the old one, and the paint on show keeps its own)
                if pv.tess.as_ref().map_or(true, |t| t.0.same_plot(&key)) { pv.tess = Some((key, std::rc::Rc::new(out))); cx.notify(); }
            });
        }).detach();
    });
}

fn color(c: &Chunk, factor: f32) -> Hsla { let mut r = rgb(c.color); r.a = c.alpha * factor; r.into() }

/// A chunk's stroke width in pixels: the hub's width is in micrometres on the capsule (radius = width / 2).
/// Answers whether anything of the chunk was left out for lying outside the canvas and its `pad`.
fn paint_chunk(out: &mut Vec<(Path<Pixels>, Hsla)>, c: &Chunk, factor: f32, v: &View, o: Point<Pixels>, w: f32, h: f32, pad: f32, mut sp: Option<&mut [f32; 3]>) -> bool {
    let mut culled = false;
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
        let margin = 50. + rpx + pad;
        let near = |x: f32, y: f32| x >= -margin && x <= w + margin && y >= -margin && y <= h + margin;
        // where the last piece ended (plot units) and the pixel last drawn to: a polyline's pieces follow one another (plot.rs `path`)
        let mut last: Option<((f32, f32), (f32, f32))> = None;
        for s in c.caps.chunks_exact(5) {
            let (a, b) = (v.to_px(s[0], s[1], w, h), v.to_px(s[2], s[3], w, h));
            if s[0] == s[2] && s[1] == s[3] { last = None; if near(a.0, a.1) { circle(&mut dots, a.0, a.1); any_dot = true; } else { culled = true; } continue; }
            // (a piece of a polyline that ends within `FINE` of the pixel last drawn to adds nothing at this zoom: an arc's or a letter's many short pieces)
            let Some(a) = step(last, (s[0], s[1]), a, b) else { last = last.map(|l| ((s[2], s[3]), l.1)); continue };
            last = Some(((s[2], s[3]), b));
            if !crosses(a, b, w, h, margin) { culled = true; continue; }
            pb.move_to(point(o.x + px(a.0), o.y + px(a.1)));
            pb.line_to(point(o.x + px(b.0), o.y + px(b.1)));
            any = true;
            if round_ends { circle(&mut dots, a.0, a.1); circle(&mut dots, b.0, b.1); any_dot = true; }
        }
        lap(0, &mut sp);
        if any { if let Ok(path) = pb.build() { lap(1, &mut sp); out.push((path, color(c, factor))); } }
        if any_dot { if let Ok(path) = dots.build() { lap(1, &mut sp); out.push((path, color(c, factor))); } }
    }
    if !c.polys.is_empty() {
        let mut pb = PathBuilder::fill(); let mut any = false;
        let mut pts: Vec<(f32, f32)> = Vec::new();
        for poly in &c.polys {
            let (mut minx, mut maxx, mut miny, mut maxy) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for p in poly.chunks_exact(2) { let q = v.to_px(p[0], p[1], w, h); minx = minx.min(q.0); maxx = maxx.max(q.0); miny = miny.min(q.1); maxy = maxy.max(q.1); }
            if maxx < -50. - pad || minx > w + 50. + pad || maxy < -50. - pad || miny > h + 50. + pad { culled = true; continue; }
            outline(poly, v, w, h, &mut pts);
            if pts.len() < 3 { continue; }
            for (i, p) in pts.iter().enumerate() { let q = point(o.x + px(p.0), o.y + px(p.1)); if i == 0 { pb.move_to(q) } else { pb.line_to(q) } }
            pb.close(); any = true;
        }
        lap(0, &mut sp);
        if any { if let Ok(path) = pb.build() { lap(1, &mut sp); out.push((path, color(c, factor))); } }
    }
    culled
}
/// Whether the segment a-b may show on a canvas of w x h with `margin` px around it: not when both ends lie beyond the same side. (Each end
/// alone was tested before: a long track across the canvas with both ends outside it was not drawn.)
pub fn crosses(a: (f32, f32), b: (f32, f32), w: f32, h: f32, margin: f32) -> bool {
    !(a.0.max(b.0) < -margin || a.0.min(b.0) > w + margin || a.1.max(b.1) < -margin || a.1.min(b.1) > h + margin)
}
/// Where a stroke's piece is drawn from, or None when it is left out. `last`: where the piece before ended (plot units) and the pixel last
/// drawn to; `start`: this piece's start (plot units); `a`, `b`: its ends in pixels. A piece that goes on from the one before is drawn from
/// the pixel last drawn to, and left out while its end is still within `FINE` of it; any other piece is drawn whole, however short.
pub fn step(last: Option<((f32, f32), (f32, f32))>, start: (f32, f32), a: (f32, f32), b: (f32, f32)) -> Option<(f32, f32)> {
    match last {
        Some((end, drawn)) if end == start => if (b.0 - drawn.0).abs() < FINE && (b.1 - drawn.1).abs() < FINE { None } else { Some(drawn) },
        _ => Some(a),
    }
}
/// A point of an outline closer than this (px, on both axes) to the last one kept is left out: a quarter of a pixel is under what four
/// samples a pixel can show, and a board fitted to the pane has most of its outline points (a zone's arcs around every pad) that close.
pub const FINE: f32 = 0.25;
/// A polygon's outline in canvas pixels, without the points that add nothing at this zoom (`FINE`), into `out` (emptied first). An outline
/// that comes to fewer than three points is smaller than can be seen.
pub fn outline(poly: &[f32], v: &View, w: f32, h: f32, out: &mut Vec<(f32, f32)>) {
    out.clear();
    for p in poly.chunks_exact(2) {
        let q = v.to_px(p[0], p[1], w, h);
        if let Some(l) = out.last() { if (q.0 - l.0).abs() < FINE && (q.1 - l.1).abs() < FINE { continue } }
        out.push(q);
    }
    // the last point kept may sit on the first: the outline closes there anyway
    while out.len() > 1 { let (f, l) = (out[0], out[out.len() - 1]); if (f.0 - l.0).abs() < FINE && (f.1 - l.1).abs() < FINE { out.pop(); } else { break } }
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
        None => { pv.picked = None; if app.data["thread"]["viewer"]["probe"].as_str().map_or(false, |p| !p.is_empty()) { keep_probe(app, None); } }
    }
}

/// How far the rest of the plot dims under a probe (0.14's plot2d.js DIM: the rest at 0.28), and the fills small enough to get the light edge (25 mm²).
pub const PROBE_DIM: f32 = 0.72;
const EDGED: f32 = 25e6;
/// What a probe lights, as the pieces themselves (0.14's plot2d.js): for each drawn chunk with lit pieces, a chunk of only those pieces in the
/// chunk's own colour at full strength, its strokes and dots never thinner than `least` micrometres across, and the outlines the light edge goes
/// round (fills under 25 mm² and dots, as x y pairs; a dot as its centre and radius). A comparison's marks (`own` chunks) are not here: they are boxed.
pub fn lit_chunks(chunks: &[std::sync::Arc<Chunk>], order: &[(usize, f32)], lit: &[(usize, usize)], least: f32) -> (Vec<Chunk>, Vec<Vec<f32>>, Vec<[f32; 3]>) {
    let mut by: std::collections::HashMap<usize, std::collections::HashSet<u32>> = Default::default();
    for (c, i) in lit { by.entry(*c).or_default().insert(*i as u32); }
    let (mut out, mut edges, mut dots) = (vec![], vec![], vec![]);
    for &(ci, _) in order {
        let (Some(c), Some(want)) = (chunks.get(ci), by.get(&ci)) else { continue };
        if c.own { continue }
        let r = c.r.max(least / 2.);
        let mut l = Chunk { layer: c.layer, color: c.color, alpha: c.alpha, r, ..Default::default() };
        for p in c.pieces.iter().filter(|p| want.contains(&p.info)) {
            let pts = &c.pts[p.at as usize..p.at as usize + p.n as usize * 2];
            match p.kind {
                2 => { l.caps.extend([pts[0], pts[1], pts[0], pts[1], r]); dots.push([pts[0], pts[1], r]); }
                1 => { let mut k = 2; while k + 1 < pts.len() { l.caps.extend([pts[k - 2], pts[k - 1], pts[k], pts[k + 1], r]); k += 2; } }
                _ => { if pts.len() >= 6 { l.polys.push(pts.to_vec()); if (p.bbox[2] - p.bbox[0]) * (p.bbox[3] - p.bbox[1]) < EDGED { edges.push(pts.to_vec()); } } }
            }
        }
        if !l.caps.is_empty() || !l.polys.is_empty() { out.push(l); }
    }
    (out, edges, dots)
}
/// The probe's light as paths: the lit pieces over the dimmed plot, then the light edge (`edge`) round the small fills and the dots, so a part or a
/// net reads at any zoom.
fn lit_paths(chunks: &[std::sync::Arc<Chunk>], order: &[(usize, f32)], lit: &[(usize, usize)], v: &View, o: Point<Pixels>, w: f32, h: f32, edge: Hsla) -> Vec<(Path<Pixels>, Hsla)> {
    let (ls, edges, dots) = lit_chunks(chunks, order, lit, 2.5 / v.scale);
    let mut out = Vec::new();
    for l in &ls { paint_chunk(&mut out, l, 1.0, v, o, w, h, 0.0, None); }
    let mut pb = PathBuilder::stroke(px(1.5)); let mut any = false;
    let seen = |x: f32, y: f32, m: f32| x >= -m && x <= w + m && y >= -m && y <= h + m;
    for e in &edges {
        let pts: Vec<(f32, f32)> = e.chunks_exact(2).map(|q| v.to_px(q[0], q[1], w, h)).collect();
        if !pts.iter().any(|p| seen(p.0, p.1, 20.)) { continue }
        for (i, q) in pts.iter().chain(pts.first()).enumerate() { let q = point(o.x + px(q.0), o.y + px(q.1)); if i == 0 { pb.move_to(q) } else { pb.line_to(q) } }
        any = true;
    }
    for d in &dots {
        let (c, r) = (v.to_px(d[0], d[1], w, h), d[2] * v.scale);
        if !seen(c.0, c.1, r + 20.) { continue }
        let n = ((6. + r * 2.) as usize).clamp(8, 32);
        for k in 0..=n { let t = std::f32::consts::TAU * k as f32 / n as f32; let q = point(o.x + px(c.0 + r * t.cos()), o.y + px(c.1 + r * t.sin())); if k == 0 { pb.move_to(q) } else { pb.line_to(q) } }
        any = true;
    }
    if any { if let Ok(path) = pb.build() { out.push((path, edge)); } }
    out
}
/// A comparison's marks a probe lights (tints: many cells of one difference): one cyan box round all of each.
fn paint_marks(window: &mut Window, chunks: &[std::sync::Arc<super::plot::Chunk>], lit: &[(usize, usize)], v: &View, o: Point<Pixels>, w: f32, h: f32) {
    let col: Hsla = rgb(0x4cc9f0).into();
    let mut by: std::collections::BTreeMap<usize, std::collections::HashSet<u32>> = Default::default();
    for (c, i) in lit { by.entry(*c).or_default().insert(*i as u32); }
    let mut pb = PathBuilder::stroke(px(2.0)); let mut any = false;
    let mut marks: std::collections::BTreeMap<u32, [f32; 4]> = Default::default();
    for (ci, want) in by {
        let Some(c) = chunks.get(ci).filter(|c| c.own) else { continue };
        for p in c.pieces.iter().filter(|p| want.contains(&p.info)) {
            let m = marks.entry(p.info).or_insert([f32::MAX, f32::MAX, f32::MIN, f32::MIN]); *m = [m[0].min(p.bbox[0]), m[1].min(p.bbox[1]), m[2].max(p.bbox[2]), m[3].max(p.bbox[3])];
        }
    }
    for bb in marks.into_values() {
        let a = v.to_px(bb[0], bb[1], w, h); let b = v.to_px(bb[2], bb[3], w, h);
        if b.0.max(a.0) < -20. || a.0.min(b.0) > w + 20. || b.1.max(a.1) < -20. || a.1.min(b.1) > h + 20. { continue }
        // a mark hugged (3 px out), never smaller than can be seen
        let (cx, cy) = ((a.0 + b.0) / 2., (a.1 + b.1) / 2.);
        let (hw, hh) = (((a.0 - b.0).abs() / 2. + 3.).max(6.), ((a.1 - b.1).abs() / 2. + 3.).max(6.));
        let (x0, y0, x1, y1) = (cx - hw, cy - hh, cx + hw, cy + hh);
        for (k, (x, y)) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)].into_iter().enumerate() { let q = point(o.x + px(x), o.y + px(y)); if k == 0 { pb.move_to(q) } else { pb.line_to(q) } }
        any = true;
    }
    if any { if let Ok(path) = pb.build() { window.paint_path(path, col); } }
}

/// A plan item's fill and outline alpha (0..255) by its state, in the floorplanner's language: proposed translucent, approved solid,
/// routed fading so the copper shows, rejected faint; blocked outlined red (`true`).
pub fn plan_alpha(state: &str) -> (u32, u32, bool) {
    match state { "approved" | "routing" => (0x70, 0xe0, false), "edited" => (0x55, 0xff, false), "routed" | "verified" => (0x22, 0x80, false), "rejected" => (0x14, 0x40, false), "blocked" => (0x55, 0xff, true), _ => (0x38, 0xa0, false) }
}
/// Whether the plan is drawn over the board: only while the Plan panel is the one open beside it (closing the panel takes it away).
pub fn plan_shown(mgmt_state: &serde_json::Value) -> bool { mgmt_state["open"] == "plan" }
/// The plan over the board: plane regions as filled outlines, corridors as bands along their path at their width (millimetres, board
/// coordinates: the plot's micrometres), each in its group's colour, its look by state; vias as small diamonds.
fn paint_plan(window: &mut Window, d: &serde_json::Value, v: &View, o: Point<Pixels>, w: f32, h: f32) {
    let at = |p: &serde_json::Value| { let (x, y) = (p[0].as_f64().unwrap_or(0.) as f32 * 1000., p[1].as_f64().unwrap_or(0.) as f32 * 1000.); let q = v.to_px(x, y, w, h); point(o.x + px(q.0), o.y + px(q.1)) };
    let colour = |c: &serde_json::Value, a: u32| -> Hsla { let n = u32::from_str_radix(c.as_str().unwrap_or("#8a8a8a").trim_start_matches('#'), 16).unwrap_or(0x8a8a8a); rgba((n << 8) | a).into() };
    for r in d["regions"].as_array().into_iter().flatten() {
        let pts: Vec<&serde_json::Value> = r["outline"].as_array().into_iter().flatten().collect(); if pts.len() < 3 { continue }
        let (fa, oa, bad) = plan_alpha(r["state"].as_str().unwrap_or(""));
        let mut f = PathBuilder::fill(); for (i, p) in pts.iter().enumerate() { if i == 0 { f.move_to(at(p)) } else { f.line_to(at(p)) } } f.close();
        if let Ok(p) = f.build() { window.paint_path(p, colour(&r["color"], fa)) }
        let mut s = PathBuilder::stroke(px(if bad { 2.0 } else { 1.0 })); for (i, p) in pts.iter().chain(pts.first()).enumerate() { if i == 0 { s.move_to(at(p)) } else { s.line_to(at(p)) } }
        if let Ok(p) = s.build() { window.paint_path(p, if bad { rgb(0xe05252).into() } else { colour(&r["color"], oa) }) }
    }
    for b in d["bands"].as_array().into_iter().flatten() {
        let pts: Vec<&serde_json::Value> = b["path"].as_array().into_iter().flatten().collect(); if pts.len() < 2 { continue }
        let (fa, oa, bad) = plan_alpha(b["state"].as_str().unwrap_or(""));
        let wpx = (b["width"].as_f64().unwrap_or(1.0) as f32 * 1000. * v.scale).max(2.0);
        let line = |width: f32| { let mut s = PathBuilder::stroke(px(width)); for (i, p) in pts.iter().enumerate() { if i == 0 { s.move_to(at(p)) } else { s.line_to(at(p)) } } s.build().ok() };
        // the band's edge first (a little wider), then its body over it: a band with an outline, as the editor draws it
        if let Some(p) = line(wpx + 2.0) { window.paint_path(p, if bad { rgb(0xe05252).into() } else { colour(&b["color"], oa) }) }
        if let Some(p) = line(wpx) { window.paint_path(p, colour(&b["color"], fa)) }
        for via in b["vias"].as_array().into_iter().flatten() {
            let c = at(via); let r = 4.0;
            let mut s = PathBuilder::fill(); s.move_to(point(c.x, c.y - px(r))); s.line_to(point(c.x + px(r), c.y)); s.line_to(point(c.x, c.y + px(r))); s.line_to(point(c.x - px(r), c.y)); s.close();
            if let Ok(p) = s.build() { window.paint_path(p, colour(&b["color"], 0xff)) }
        }
    }
}

/// The tapped piece over everything: a stroke in a bright colour, a fill or a dot outlined in it.
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
    if p.kind == 1 {
        // a stroke (a track, a wire) is lit as itself, at its own width (never thinner than 2.5 px), round ended: no line down its middle
        let c = Chunk { color: 0xffe066, alpha: 1.0, r: p.r.max(1.25 / v.scale), caps: p.pts.windows(4).step_by(2).flat_map(|q| [q[0], q[1], q[2], q[3], p.r.max(1.25 / v.scale)]).collect(), ..Default::default() };
        let mut out = Vec::new(); paint_chunk(&mut out, &c, 1.0, v, o, w, h, 0.0, None);
        for (path, color) in out { window.paint_path(path, color); }
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
    let chunks = frame.chunks.clone(); let fkey = frame.key.clone();
    // the hub's masks decide which chunks are drawn, dimmed, and which one is painted last
    let order = crate::viewers::layers::plan(&chunks.iter().map(|c| c.layer).collect::<Vec<_>>(), &masks);
    let this = cx.entity();
    let (key, off) = (frame.key.clone(), masks.off); let (key2, key3) = (key.clone(), key.clone());
    // a probe: the rest dims towards the canvas's ground, and what it lights gets an edge in the text's colour (light on the dark ground, dark on the light)
    let (dim, edge): (Hsla, Hsla) = ({ let mut d = c("view"); d.a = PROBE_DIM; d.into() }, { let mut e = c("text"); e.a = 0.77; e.into() });
    let el = canvas(|b, _, _| b, move |_, b, window, cx| {
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        this.update(cx, |app, _| { app.screens.pv(slot).bounds = Some(b); });
        let Some(v) = view else { return };
        let measure = this.update(cx, |app, _| app.screens.pv(slot).measure);
        let t0 = measure.then(std::time::Instant::now); let mut sp = [0f32; 3];
        let (lit, probe) = this.update(cx, |app, _| { let pv = app.screens.pv(slot); (pv.picked.clone(), pv.lit.clone()) });
        // the paths kept from an earlier paint are drawn again, moved by what the camera moved since (a drag tessellates nothing here);
        // none that fit (the first paint, a zoom, another plot or size): the canvas alone is tessellated now, as little as shows it, and
        // the plot around it off this thread, for the paints that follow
        let mut key = TessKey::of(&chunks, &order, &v, b, 0.0);
        let exact = key.clone(); // this very camera: what the probe's light is kept for
        let (paths, moved) = match this.update(cx, |app, _| app.screens.pv(slot).tess.as_ref().and_then(|t| t.0.moved(&key).map(|d| (t.1.clone(), d, t.0.pad)))) {
            Some((p, d, pad)) => { if d.0.abs().max(d.1.abs()) > pad * 0.5 || pad == 0.0 { tess_ahead(&this, cx, slot, &chunks, &order, v, b); } (p, d) }
            None => {
                let (out, whole) = tessellate(&chunks, &order, &v, b.origin, w, h, 0.0, measure.then_some(&mut sp));
                // (the whole plot is on the canvas: these paths serve wherever the camera goes, and none are made ahead)
                if whole { key.pad = f32::INFINITY; }
                let p = std::rc::Rc::new(out);
                this.update(cx, |app, _| app.screens.pv(slot).tess = Some((key, p.clone())));
                if !whole { tess_ahead(&this, cx, slot, &chunks, &order, v, b); }
                (p, (0.0, 0.0))
            }
        };
        let moved = point(px(moved.0), px(moved.1));
        window.with_content_mask(Some(ContentMask { bounds: b }), |window| {
            let t2 = measure.then(std::time::Instant::now);
            for (path, color) in paths.iter() { window.paint_path_at(path, moved, *color); }
            if let Some(t2) = t2 { sp[2] += t2.elapsed().as_secs_f32() * 1000.0; }
            if !probe.is_empty() {
                // the probed net or part itself, over the rest dimmed (tessellated once a camera, as the plot is)
                let stamp = { use std::hash::{Hash, Hasher}; let mut hs = std::collections::hash_map::DefaultHasher::new(); probe.hash(&mut hs); hs.finish() };
                let lp = match this.update(cx, |app, _| app.screens.pv(slot).lit_tess.as_ref().filter(|t| t.0 == exact && t.1 == stamp).map(|t| t.2.clone())) {
                    Some(p) => p,
                    None => { let p = std::rc::Rc::new(lit_paths(&chunks, &order, &probe, &v, b.origin, w, h, edge)); this.update(cx, |app, _| app.screens.pv(slot).lit_tess = Some((exact.clone(), stamp, p.clone()))); p }
                };
                if !lp.is_empty() { window.paint_quad(fill(b, dim)); for (path, color) in lp.iter() { window.paint_path(path.clone(), *color); } }
                paint_marks(window, &chunks, &probe, &v, b.origin, w, h);
            }
            if let Some(p) = &lit { paint_lit(window, p, &v, b.origin, w, h); }
            // the routing plan's bands and regions over the board, read-only (the hub's mgmt.plan data; steering is in KiCad), only while the Plan panel is
            // open: drawn always, a proposed plan's grey corridors covered the board with nothing saying what they were or how to hide them (Nov 16)
            if fkey.starts_with("0|") { let draw = this.update(cx, |app, _| if plan_shown(&app.screens.mgmt.peek()) { app.screens.mgmt.data("plan")["draw"].clone() } else { serde_json::Value::Null }); if !draw.is_null() { paint_plan(window, &draw, &v, b.origin, w, h); } }
        });
        if let Some(t0) = t0 {
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            this.update(cx, |app, _| { let pv = &mut app.screens.pv(slot); if pv.paint_ms.len() >= 4096 { pv.paint_ms.remove(0); } pv.paint_ms.push(ms); if pv.split.len() >= 4096 { pv.split.remove(0); } pv.split.push(sp); });
        }
        // click to the first painted plot: what the person feels (the metric the gate asks for)
        // (a paint of the plot that was on show at the press is not it: the press asked for another)
        this.update(cx, |app, _| { if app.screens.plot_click.as_ref().map_or(false, |c| c.1 != fkey) { let (t, _) = app.screens.plot_click.take().unwrap(); app.m("plot_painted", serde_json::json!({"ms": (t.elapsed().as_secs_f64() * 1e6).round() / 1e3, "key": fkey.clone(), "chunks": chunks.len()})); } });
    }).size_full();
    div().id(SharedString::from(format!("plot-{slot}"))).flex_1().min_h_0().bg(c("view")).child(el)
        .when_some(focus, |d, f| d.track_focus(&f).key_context("Viewer Plot").on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
            // keys on the plot: arrows move the view, + and - zoom at the centre, 0 or Home fits again, Escape puts the tapped piece away
            let pv = this.screens.pv(slot);
            let (Some(v), Some(b)) = (pv.view, pv.bounds) else { return };
            let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
            let nv = match e.keystroke.key.as_str() {
                "left" => Some(v.pan(60., 0.)), "right" => Some(v.pan(-60., 0.)), "up" => Some(v.pan(0., 60.)), "down" => Some(v.pan(0., -60.)),
                "+" | "=" | "plus" => Some(v.zoom(1.25, w / 2., h / 2., w, h, pv.fitted)), "-" | "minus" => Some(v.zoom(0.8, w / 2., h / 2., w, h, pv.fitted)),
                "0" | "home" => { pv.view = None; cx.notify(); return }
                // (a Split's probe is lit in both its panes: Escape clears it in both)
                "escape" => { let all: &[usize] = if slot == 0 { &[0] } else { &[1, 2] }; for &i in all { let p = this.screens.pv(i); p.picked = None; p.card = None; p.lit.clear(); } keep_probe(this, None); cx.notify(); return }
                _ => None,
            };
            if let Some(nv) = nv { pv.touched = true; pv.view = Some(nv); super::follow::person_input(this, slot); cx.notify(); }
        })))
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, window, cx| { plot_down(this, slot, e, window, cx) }))
        .on_mouse_down(crate::mousecfg::BUTTONS[1], cx.listener(move |this, e: &MouseDownEvent, window, cx| { plot_down(this, slot, e, window, cx) }))
        .on_mouse_down(crate::mousecfg::BUTTONS[2], cx.listener(move |this, e: &MouseDownEvent, window, cx| { plot_down(this, slot, e, window, cx) }))
        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
            let pv = this.screens.pv(slot);
            if let (Some((x, y)), Some(v), Some(act), Some(b), true) = (pv.drag, pv.view, pv.drag_act, pv.bounds, e.dragging()) {
                let (nx, ny) = (f32::from(e.position.x), f32::from(e.position.y));
                let moved = match act {
                    Action::Pan => { pv.view = Some(v.pan(nx - x, ny - y)); true }
                    Action::Zoom => { pv.view = Some(v.zoom(crate::mousecfg::drag_zoom_factor(ny - y, crate::mousecfg::invert()), f32::from(px(x) - b.origin.x), f32::from(px(y) - b.origin.y), f32::from(b.size.width), f32::from(b.size.height), pv.fitted)); true }
                    _ => false,
                };
                pv.drag = Some((nx, ny)); if moved { person_moved(this, slot); } cx.notify();
            }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(move |this, e: &MouseUpEvent, _, cx| { plot_up(this, slot, &key, off, e, cx) }))
        .on_mouse_up(crate::mousecfg::BUTTONS[1], cx.listener({ let key = key2; move |this, e: &MouseUpEvent, _, cx| { plot_up(this, slot, &key, off, e, cx) } }))
        .on_mouse_up(crate::mousecfg::BUTTONS[2], cx.listener({ let key = key3; move |this, e: &MouseUpEvent, _, cx| { plot_up(this, slot, &key, off, e, cx) } }))
        .on_mouse_up_out(MouseButton::Left, cx.listener(move |this, _, _, cx| { let pv = this.screens.pv(slot); pv.drag = None; pv.drag_act = None; cx.notify(); }))
        .on_mouse_up_out(crate::mousecfg::BUTTONS[1], cx.listener(move |this, _, _, cx| { let pv = this.screens.pv(slot); pv.drag = None; pv.drag_act = None; cx.notify(); }))
        .on_mouse_up_out(crate::mousecfg::BUTTONS[2], cx.listener(move |this, _, _, cx| { let pv = this.screens.pv(slot); pv.drag = None; pv.drag_act = None; cx.notify(); }))
        .on_scroll_wheel(cx.listener(move |this, e: &ScrollWheelEvent, _, cx| {
            let pv = this.screens.pv(slot);
            let (Some(v), Some(b)) = (pv.view, pv.bounds) else { return };
            // what a wheel or two-finger scroll means is the table's (mouse.rs): a mouse's lines and a trackpad's pixels are told apart there
            let (lines, x, y) = crate::mousecfg::delta(&e.delta);
            match crate::mousecfg::plot().scroll(crate::mousecfg::mods(&e.modifiers), lines, x, y, crate::mousecfg::invert()) {
                Scroll::Pan { dx, dy } => { pv.view = Some(v.pan(dx, dy)); person_moved(this, slot); cx.notify(); }
                Scroll::Zoom { lines, y, .. } => {
                    let d = if lines { y * 16. } else { y };
                    let factor = (1.0 + d * 0.0015).clamp(0.5, 2.0);
                    pv.view = Some(v.zoom(factor, f32::from(e.position.x - b.origin.x), f32::from(e.position.y - b.origin.y), f32::from(b.size.width), f32::from(b.size.height), pv.fitted));
                    person_moved(this, slot); cx.notify();
                }
                Scroll::None => {}
            }
        }))
        // a trackpad pinch: macOS sends it as a magnify gesture (gpui's PinchEvent), never as a wheel, so it zooms here at the pointer
        .on_pinch(cx.listener(move |this, e: &PinchEvent, _, cx| {
            let pv = this.screens.pv(slot);
            let (Some(v), Some(b)) = (pv.view, pv.bounds) else { return };
            let factor = crate::mousecfg::plot().pinch_factor(crate::viewers::solid::pinch_zoom(e.delta), crate::mousecfg::invert()); if factor == 1.0 { return }
            pv.view = Some(v.zoom(factor, f32::from(e.position.x - b.origin.x), f32::from(e.position.y - b.origin.y), f32::from(b.size.width), f32::from(b.size.height), pv.fitted));
            person_moved(this, slot); cx.notify();
        })).into_any_element()
}

/// The person moved or zoomed the view: the one place every plot input ends, so a camera that follows something lets go here (follow pauses).
fn person_moved(app: &mut Backplane, slot: usize) { app.screens.pv(slot).touched = true; super::follow::person_input(app, slot); }

/// A button went down on the plot: the table says whether it drags (and what the drag does) and whether a click of it selects.
fn plot_down(this: &mut Backplane, slot: usize, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Backplane>) {
    if let Some(f) = this.screens.pv(slot).focus.clone() { window.focus(&f); }
    let Some(b) = crate::mousecfg::button(e.button) else { return };
    let (scheme, m) = (crate::mousecfg::plot(), crate::mousecfg::mods(&e.modifiers));
    let at = (f32::from(e.position.x), f32::from(e.position.y));
    let pv = this.screens.pv(slot);
    let act = scheme.drag(b, m);
    pv.drag = Some(at); pv.drag_act = Some(act); pv.drag_btn = Some(e.button);
    pv.down = scheme.selects(b, m).then_some(at);
    super::follow::person_input(this, slot); cx.notify();
}
/// A button came up: a press and release within 4 px of a selecting button is a tap, a double-click of it fits the plot again.
fn plot_up(this: &mut Backplane, slot: usize, key: &str, off: u32, e: &MouseUpEvent, cx: &mut Context<Backplane>) {
    if this.screens.pv(slot).drag_btn != Some(e.button) { return }
    let up = (f32::from(e.position.x), f32::from(e.position.y));
    let selecting = crate::mousecfg::plot().selects(crate::mousecfg::button(e.button).unwrap_or(crate::mouse::Button::Left), crate::mouse::Mods::NONE);
    if selecting && e.click_count >= 2 { let pv = this.screens.pv(slot); pv.down = None; pv.drag = None; pv.drag_act = None; pv.view = None; cx.notify(); return }
    let (tap, view, bounds) = { let pv = this.screens.pv(slot); pv.drag = None; pv.drag_act = None; pv.drag_btn = None; (pv.down.take().filter(|d| (d.0 - up.0).abs() < 4. && (d.1 - up.1).abs() < 4.), pv.view, pv.bounds) };
    if tap.is_some() { tap_at(this, slot, key, off, up, view, bounds); }
    cx.notify();
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
    // Follow agent: an edit's box, once the plot of its page and sheet is the one drawn
    if frame.none.is_empty() && pv.follow_to.is_some() { super::follow::take(pv, &frame.key, w, h); }
    pv.view
}

/// A card as the hub's info text again ("title;k=v;..."), what the hub keeps as the probe.
pub fn card_text(card: &super::plot::Card) -> String {
    std::iter::once(card.title.replace(';', ",")).chain(card.rows.iter().map(|(k, v)| if v.is_empty() { k.replace(';', ",") } else { format!("{}={}", k.replace(';', ","), v.replace(';', ",")) }))
        .chain((!card.src.is_empty()).then(|| format!("g={}\u{1}{}", card.src.replace(';', ","), card.g))).collect::<Vec<_>>().join(";")
}
/// The probe the person made or cleared, kept by the hub for this project's board (0.14's probe:<root>|<board> setting): every page and every opening shows it again.
pub fn keep_probe(app: &mut Backplane, card: Option<&super::plot::Card>) {
    let text = card.map(card_text).unwrap_or_default();
    *PROBE_SENT.lock().unwrap() = Some((text.clone(), std::time::Instant::now()));
    let _ = app.tx.try_send(Command::Action("view-probe".into(), text));
}
/// A part clicked on the 3D model (`r` its reference; "" the board or nothing, which clears): the board's probe as the hub writes a part's
/// (kicad/xprobe.rs `probe_text`), so the schematic and the board light it wherever they are on show, now or later, and the open KiCad selects it.
pub fn probe_part(app: &mut Backplane, r: &str) {
    let r = r.replace([';', '\n', '\u{1}'], ",");
    if r.is_empty() { if app.data["thread"]["viewer"]["probe"].as_str().map_or(false, |p| !p.is_empty()) { keep_probe(app, None); } return }
    let Some(card) = super::plot::card(&format!("footprint {r};ref={r}")) else { return }; // i18n-ok: a probe's text, not drawn
    // (what a canvas had tapped is no longer the probe: its own mark goes, the hub's copy lights the part there)
    for q in [0usize, 1, 2] { let pv = app.screens.pv(q); pv.picked = None; }
    keep_probe(app, Some(&card));
    super::xprobe::tapped(app, &card);
}
/// The part the probe names, for the 3D model to light ("" none): the one this window just told the hub while the hub's copy is on its way, else the hub's.
pub fn probed_part(app: &Backplane) -> String {
    let sent = PROBE_SENT.lock().unwrap().clone().filter(|s| s.1.elapsed() < std::time::Duration::from_secs(3)).map(|s| s.0);
    let text = sent.unwrap_or_else(|| app.data["thread"]["viewer"]["probe"].as_str().unwrap_or("").to_string());
    super::plot::card(&text).and_then(|c| c.rows.iter().find(|x| x.0 == "ref" || x.0 == "symbol").map(|x| x.1.clone())).unwrap_or_default()
}
/// The probe this window just told the hub (and when): until the hub's copy says the same (or 3 s pass), the hub's older probe is not drawn over it.
static PROBE_SENT: std::sync::Mutex<Option<(String, std::time::Instant)>> = std::sync::Mutex::new(None);
/// The pieces of the probed part or connection on the plot `key` on canvas `slot`: a part by its `ref` on the schematic and the board (every unit of it);
/// a board net by its name on the board, and on a schematic the groups its pads' pins are in; a schematic wire, label or junction by its group on its own
/// sheet, and on the board the nets of the pins in it (the hub finds those through the other drawing, `card.src`). The hub answers with the pieces of the
/// chunks this client holds (`kicad.probe`); they arrive as `Event::Probe`.
pub fn probe_one(app: &mut Backplane, slot: usize, key: &str, card: &super::plot::Card) {
    if key.is_empty() { return }
    let get = |k: &str| card.rows.iter().find(|r| r.0 == k).map(|r| r.1.clone()).filter(|v| !v.is_empty());
    let tag: &'static str = ["probe0", "probe1", "probe2"][slot.min(2)];
    let board = |k: &str| k.starts_with("0|");
    // (a schematic pin names its part as `symbol`: the part is probed)
    let net = get("net").and_then(|n| if board(key) { Some(n) } else if board(&card.src) { Some(format!("{}\u{1}{n}", card.src)) } else { None });
    // (a pin or a symbol is its part; a wire, label or junction is its connection)
    let group = (!card.g.is_empty() && !card.src.is_empty() && !card.title.starts_with("pin") && !card.title.starts_with("symbol")).then(|| format!("{}\u{1}{}", card.src, card.g));
    // (a difference of Gerbers vs board is lit by its place: kicad/gerber/compare.rs)
    for (name, value) in [("ref", get("ref").or_else(|| get("symbol"))), ("net", net), ("g", group), ("at", get("at"))] {
        let Some(value) = value else { continue };
        let _ = app.tx.try_send(Command::Rpc("kicad.probe".into(), crate::screens::settings::Secret(serde_json::json!({"key": key, "name": name, "value": value})), tag));
    }
}
/// A tapped piece's net and part, lit on every plot the page shows (the web's cross-probe), and kept by the hub.
pub fn cross_probe(app: &mut Backplane, slot: usize, card: &super::plot::Card) {
    let panes = app.data["thread"]["viewer"]["panes"].as_array().cloned().unwrap_or_default();
    let targets: Vec<(usize, String)> = if panes.len() >= 3 { vec![(1, panes[0]["key"].as_str().unwrap_or("").to_string()), (2, panes[1]["key"].as_str().unwrap_or("").to_string())] }
        else { vec![(slot, app.screens.plot.frame.as_ref().map(|f| f.key.clone()).unwrap_or_default())] };
    // the source it was picked on goes with it: a connection is found on the other drawing through it
    let mut card = card.clone();
    if card.src.is_empty() { card.src = if slot == 0 { app.screens.plot.frame.as_ref().map(|f| f.key.clone()) } else { panes.get(slot - 1).and_then(|p| p["key"].as_str()).map(str::to_string) }.unwrap_or_default(); }
    let (card, text) = (&card, card_text(&card));
    // (lit here at once: the hub's copy of this probe, when it comes back, finds it already lit)
    for (s, key) in targets {
        let pv = app.screens.pv(s); pv.probe_at = format!("{key}\u{1}{text}");
        // (a Split's other pane: what it had tapped or was told before is not the probe any more; its bar names this one)
        if s != slot { pv.picked = None; pv.card = Some(card.clone()); }
        probe_one(app, s, &key, card);
    }
    keep_probe(app, Some(card));
    super::xprobe::tapped(app, card);
}
/// The hub's probe on a canvas showing source `key`: lit again (and its card set) when the source or the probe changed since it was last lit; cleared when the
/// hub has none. Called while the page is drawn; the requests go out right after.
pub fn probe_sync(cx: &mut Context<Backplane>, slot: usize, pv: &mut PlotView, key: &str, probe: &str) {
    let tag = format!("{key}\u{1}{probe}");
    if pv.probe_at == tag || key.is_empty() { return }
    // a tap (or a clear) on its way to the hub: what this canvas shows is already that; the hub's older copy waits
    if let Some((sent, at)) = PROBE_SENT.lock().unwrap().clone() {
        if at.elapsed() < std::time::Duration::from_secs(3) {
            if sent != probe { return }
            if pv.probe_at.ends_with(&format!("\u{1}{sent}")) && pv.probe_at.starts_with(&format!("{key}\u{1}")) { pv.probe_at = tag; return }
        }
    }
    let first = pv.probe_at.is_empty();
    pv.probe_at = tag;
    if probe.is_empty() { if !first { pv.card = None; pv.picked = None; pv.lit.clear(); } return }
    let Some(card) = super::plot::card(probe) else { return };
    eprintln!("GPUI_LOG {}", serde_json::json!({"event": "probe_sync", "slot": slot, "key": key, "probe": probe}));
    // the same probe on the same source is already lit (the tap that made it); otherwise light it here
    pv.card = Some(card.clone()); pv.lit.clear();
    let key = key.to_string();
    cx.spawn(async move |this, cx| { let _ = this.update(cx, |app, _| probe_one(app, slot, &key, &card)); }).detach();
}

/// The box (plot micrometres) around the lit pieces on these chunks, padded so the part sits in some of its surroundings; None with nothing lit.
pub fn lit_box(chunks: &[std::sync::Arc<super::plot::Chunk>], lit: &[(usize, usize)]) -> Option<[f32; 4]> {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for (ci, i) in lit { let Some(c) = chunks.get(*ci) else { continue }; for p in c.pieces.iter().filter(|p| p.info as usize == *i) { b = [b[0].min(p.bbox[0]), b[1].min(p.bbox[1]), b[2].max(p.bbox[2]), b[3].max(p.bbox[3])]; } }
    if b[0] > b[2] { return None }
    // the piece and its neighbourhood: at least 3 mm round it (a difference of a tenth of a mm is seen among the copper it is in)
    let pad = (b[2] - b[0]).max(b[3] - b[1]) * 0.6 + 3000.;
    Some([b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad])
}
/// "Bring it into view" (0.14's probe chip): canvas `slot`'s camera on the probed part (its pieces asked of the hub by its ref; the answer comes as
/// `Event::Probe(3 + slot, ..)`, `frame_on`), or on the lit net or piece when there is no part.
pub fn bring_into_view(app: &mut Backplane, slot: usize) {
    let key = if slot == 0 { app.screens.plot.frame.as_ref().map(|f| f.key.clone()) } else { app.data["thread"]["viewer"]["panes"][slot - 1]["key"].as_str().map(str::to_string) }.unwrap_or_default();
    let card = app.screens.pv(slot).card.clone().or_else(|| app.data["thread"]["viewer"]["probe"].as_str().and_then(super::plot::card));
    // a part by its reference; a place (Gerbers vs board's difference) by its `at`: asked again, framed when the answer comes
    let r = card.and_then(|c| c.rows.iter().find(|x| x.0 == "ref" || x.0 == "symbol").map(|x| ("ref", x.1.clone())).or_else(|| c.rows.iter().find(|x| x.0 == "at").map(|x| ("at", x.1.clone())))).filter(|r| !r.1.is_empty());
    if let (Some((name, r)), false) = (r, key.is_empty()) {
        let tag: &'static str = ["probe3", "probe4", "probe5"][slot.min(2)];
        let _ = app.tx.try_send(Command::Rpc("kicad.probe".into(), crate::screens::settings::Secret(serde_json::json!({"key": key, "name": name, "value": r})), tag));
        return
    }
    let lit = app.screens.pv(slot).lit.clone();
    frame_on(app, slot, &lit);
}
/// The camera of canvas `slot` on these pieces, no closer than 12 times the whole drawing's fit.
pub fn frame_on(app: &mut Backplane, slot: usize, pieces: &[(usize, usize)]) {
    let key = if slot == 0 { app.screens.plot.frame.as_ref().map(|f| f.key.clone()) } else { app.data["thread"]["viewer"]["panes"][slot - 1]["key"].as_str().map(str::to_string) }.unwrap_or_default();
    let Some(f) = app.screens.plot.frame_for(&key) else { return };
    let chunks = f.chunks.clone();
    let pv = app.screens.pv(slot);
    let (Some(b), Some(bounds)) = (lit_box(&chunks, pieces), pv.bounds) else { return };
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let mut v = View::fit(b, w.max(10.), h.max(10.));
    if pv.fitted > 0. { v.scale = v.scale.min(pv.fitted * 12.); }
    pv.view = Some(v); pv.touched = true;
}
/// What the viewer shows now, for the hub's `viewer.state` (an agent's viewer_show answers with it): the page, its source, the part (Mech), the sheet (schematic), the active layer and the tapped piece. Sent only on change.
/// Pure viewer state snapshot reported to the hub for `viewer_show` read-back (includes `gerbers` and any `empty` reason).
pub fn state_json(v: &serde_json::Value, card: Option<&super::plot::Card>, lit: usize) -> serde_json::Value {
    let open = v["open"].as_str().unwrap_or("");
    let key = v["key"].as_str().unwrap_or("");
    let layer = v["layerHead"].as_str().filter(|h| h.starts_with("Layer")).map(|h| h.trim_start_matches("Layer: ").to_string());
    let empty = v["empty"].as_str().filter(|s| !s.is_empty());
    let mut state = serde_json::json!({"open": open, "key": key,
        "part": if open == "mech" { v["mech"]["rel"].clone() } else { serde_json::Value::Null },
        "sheet": if open == "schematic" { serde_json::json!(key.splitn(3, '|').nth(2).unwrap_or("")) } else { serde_json::Value::Null },
        "layer": layer, "layerMode": v["layerMode"],
        "selection": card.map(|c| serde_json::json!({"title": c.title, "rows": c.rows.iter().map(|r| serde_json::json!([r.0, r.1])).collect::<Vec<_>>()})),
        "lit": lit});
    if v["gerbers"] == true { state["gerbers"] = serde_json::json!(true); }
    if let Some(e) = empty { state["empty"] = serde_json::json!(e); }
    state
}
pub fn report_state(app: &mut Backplane) {
    let v = app.data["thread"]["viewer"].clone();
    let card = [0usize, 1, 2].iter().find_map(|i| app.screens.pv(*i).card.clone());
    let lit: usize = (0..3).map(|i| app.screens.pv(i).lit.len()).sum();
    let mut state = state_json(&v, card.as_ref(), lit);
    // the embedded KiCad in this window's pane (Follow agent follows into it: host kicad/follow.rs); a window that cannot hold KiCad says false
    state["kicad"] = serde_json::json!(app.screens.kicad_shown.0); state["kicadFrame"] = serde_json::json!(app.screens.kicad_shown.1);
    // the thread too: the same page of another thread is news to the hub (whose page holds KiCad)
    let text = format!("{}\t{state}", app.selected);
    if text == app.screens.last_state || app.selected.is_empty() { return }
    app.screens.last_state = text;
    let _ = app.tx.try_send(Command::Rpc("viewer.state".into(), crate::screens::settings::Secret(serde_json::json!({"thread": app.selected, "state": state})), "vstate"));
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn the_plan_is_drawn_on_the_board_only_while_its_panel_is_open() {
        assert!(plan_shown(&serde_json::json!({"open": "plan"})));
        for st in [serde_json::json!({"open": ""}), serde_json::json!({"open": "bom"}), serde_json::Value::Null] { assert!(!plan_shown(&st), "{st}"); }
    }
    #[test] fn a_tessellated_plot_is_reused_only_for_the_same_chunks_order_zoom_and_canvas_size() {
        let a = std::sync::Arc::new(Chunk::default()); let b = std::sync::Arc::new(Chunk::default());
        let v = View { cx: 1., cy: 2., scale: 0.5 }; let bx = Bounds::new(point(px(10.), px(20.)), size(px(300.), px(200.)));
        let k = TessKey::of(&[a.clone(), b.clone()], &[(0, 1.0), (1, 0.4)], &v, bx, 0.0);
        let of = |cs: &[std::sync::Arc<Chunk>], order: &[(usize, f32)], v: &View, bx: Bounds<Pixels>| k.moved(&TessKey::of(cs, order, v, bx, 0.0));
        assert_eq!(of(&[a.clone(), b.clone()], &[(0, 1.0), (1, 0.4)], &v, bx), Some((0.0, 0.0)), "the same paint");
        assert_eq!(of(&[a.clone(), std::sync::Arc::new(Chunk::default())], &[(0, 1.0), (1, 0.4)], &v, bx), None, "a chunk replaced");
        assert_eq!(of(&[a.clone(), b.clone()], &[(1, 1.0), (0, 0.4)], &v, bx), None, "another order");
        assert_eq!(of(&[a.clone(), b.clone()], &[(0, 1.0), (1, 1.0)], &v, bx), None, "another fade");
        assert_eq!(of(&[a.clone(), b.clone()], &[(0, 1.0), (1, 0.4)], &View { scale: 0.6, ..v }, bx), None, "zoomed");
        assert_eq!(of(&[a.clone(), b.clone()], &[(0, 1.0), (1, 0.4)], &v, Bounds::new(point(px(10.), px(20.)), size(px(301.), px(200.)))), None, "resized");
        assert_eq!(of(&[a, b], &[(0, 1.0), (1, 0.4)], &View { cx: 3., ..v }, bx), None, "moved, and these paths reach no further than the canvas");
    }
    #[test] fn a_drag_moves_the_kept_paths_until_the_camera_is_past_what_they_cover() {
        let a = std::sync::Arc::new(Chunk::default());
        let v = View { cx: 100., cy: 200., scale: 0.5 }; let bx = Bounds::new(point(px(10.), px(20.)), size(px(800.), px(600.)));
        assert_eq!(tess_pad(800., 600.), 400.0); assert_eq!(tess_pad(100., 80.), 200.0, "a small canvas still covers a short drag"); assert_eq!(tess_pad(5000., 900.), 1200.0);
        let k = TessKey::of(&[a.clone()], &[(0, 1.0)], &v, bx, 400.0);
        let at = |v: &View, bx: Bounds<Pixels>| k.moved(&TessKey::of(&[a.clone()], &[(0, 1.0)], v, bx, 0.0));
        // the plot dragged 30 px right and 10 px down: the camera's centre goes the other way, 60 and 20 plot units at this zoom
        let dragged = v.pan(30., 10.);
        assert_eq!(at(&dragged, bx), Some((30.0, 10.0)), "the paths follow the pointer");
        assert_eq!(at(&v.pan(-400., 0.), bx), Some((-400.0, 0.0)), "as far as they were made");
        assert_eq!(at(&v.pan(-401., 0.), bx), None, "past it they are made again");
        assert_eq!(at(&v.pan(0., 500.), bx), None);
        // paths of the whole plot serve the camera wherever it goes
        let mut whole = k.clone(); whole.pad = f32::INFINITY;
        assert_eq!(whole.moved(&TessKey::of(&[a.clone()], &[(0, 1.0)], &v.pan(0., 5000.), bx, 0.0)), Some((0.0, 5000.0)));
        // the canvas itself moved in the window (a pane beside it changed width by the same amount on both sides): the paths go with it
        assert_eq!(at(&v, Bounds::new(point(px(25.), px(20.)), size(px(800.), px(600.)))), Some((15.0, 0.0)));
        // and a moved plot is where a tessellation at the new camera would put it: a plot point's pixel is the same either way
        let (w, h) = (800., 600.); let p = (140., 260.);
        let (was, now) = (v.to_px(p.0, p.1, w, h), dragged.to_px(p.0, p.1, w, h));
        assert!((was.0 + 30.0 - now.0).abs() < 1e-3 && (was.1 + 10.0 - now.1).abs() < 1e-3);
    }
    #[test] fn a_track_across_the_canvas_with_both_ends_outside_is_drawn() {
        let (w, h, m) = (800., 600., 50.);
        assert!(crosses((-500., 300.), (1500., 300.), w, h, m), "across, both ends out");
        assert!(crosses((-500., -400.), (900., 900.), w, h, m), "a diagonal across");
        assert!(crosses((10., 10.), (20., 20.), w, h, m) && crosses((-30., 10.), (-20., 20.), w, h, m), "inside, and inside the margin");
        assert!(!crosses((-500., 300.), (-60., 300.), w, h, m) && !crosses((900., 0.), (2000., 600.), w, h, m), "both ends beyond one side");
        assert!(!crosses((0., -300.), (800., -51.), w, h, m) && !crosses((0., 651.), (800., 900.), w, h, m));
    }
    #[test] fn a_polylines_pieces_too_short_to_show_are_left_out_and_the_line_stays_whole() {
        // an arc of 40 pieces a tenth of a pixel long, then a long piece: drawn as the pieces that reach a quarter pixel, each from where the last drawn one ended
        let pt = |k: usize| (k as f32 * 0.1, 0.0f32); let mut last = None; let mut drawn: Vec<((f32, f32), (f32, f32))> = Vec::new();
        let mut pieces: Vec<((f32, f32), (f32, f32))> = (0..40).map(|k| (pt(k), pt(k + 1))).collect(); pieces.push((pt(40), (50.0, 30.0)));
        for (a, b) in &pieces {
            let from = step(last, *a, *a, *b); // (plot units are pixels here)
            last = Some((*b, match from { Some(_) => *b, None => last.map_or(*b, |l: ((f32, f32), (f32, f32))| l.1) }));
            if let Some(f) = from { drawn.push((f, *b)); }
        }
        assert!(drawn.len() >= 12 && drawn.len() <= 16, "{} pieces drawn of 41", drawn.len());
        assert_eq!(drawn[0], (pt(0), pt(1)), "the first piece is drawn whole, however short");
        for w in drawn.windows(2) { assert_eq!(w[0].1, w[1].0, "no gap: each piece starts where the last drawn one ended"); }
        assert_eq!(drawn.last().unwrap().1, (50.0, 30.0)); assert!((drawn.last().unwrap().0 .0 - 4.0).abs() < FINE, "the long piece starts within a quarter pixel of the arc's end");
        // a piece that does not go on from the one before is drawn whole
        assert_eq!(step(Some(((9.0, 9.0), (9.0, 9.0))), (0.0, 0.0), (0.0, 0.0), (0.1, 0.0)), Some((0.0, 0.0)));
        assert_eq!(step(None, (0.0, 0.0), (0.0, 0.0), (0.1, 0.0)), Some((0.0, 0.0)));
    }
    #[test] fn an_outline_loses_only_the_points_too_close_to_show() {
        let v = View { cx: 0., cy: 0., scale: 1.0 }; let (w, h) = (0., 0.); let mut out = Vec::new();
        // a square of 10 px with a run of points a tenth of a pixel apart along one side, and its last point on its first
        let mut p = vec![0., 0.]; for k in 1..=100 { p.extend([k as f32 * 0.1, 0.]); } p.extend([10., 10., 0., 10., 0.05, 0.05]);
        outline(&p, &v, w, h, &mut out);
        assert_eq!(out.first(), Some(&(0., 0.))); assert_eq!(out.last(), Some(&(0., 10.)), "the point on the first is left out");
        assert!(out.len() >= 34 && out.len() <= 38, "{} points: about three a pixel along the fine side", out.len());
        for q in out.windows(2) { assert!((q[1].0 - q[0].0).abs() >= FINE || (q[1].1 - q[0].1).abs() >= FINE); }
        assert!(out.iter().any(|q| (q.0 - 10.).abs() < FINE && q.1 == 0.) && out.contains(&(10., 10.)), "the corners stay within a quarter pixel");
        // zoomed in ten times, every point shows
        outline(&p, &View { scale: 10.0, ..v }, w, h, &mut out); assert_eq!(out.len(), 104);
        // a pad a fifth of a pixel wide is smaller than can be seen
        outline(&[0., 0., 0.2, 0., 0.2, 0.2, 0., 0.2], &v, w, h, &mut out); assert!(out.len() < 3);
        // one of half a pixel still is
        outline(&[0., 0., 0.5, 0., 0.5, 0.5, 0., 0.5], &v, w, h, &mut out); assert_eq!(out.len(), 4);
    }
    #[test] fn the_lit_box_is_the_lit_pieces_padded() {
        let c = super::super::plot::Chunk { pieces: vec![super::super::plot::Piece { info: 7, bbox: [0., 0., 1000., 500.], at: 0, n: 0, kind: 0 }, super::super::plot::Piece { info: 8, bbox: [9e5, 9e5, 9.1e5, 9.1e5], at: 0, n: 0, kind: 0 }], ..Default::default() };
        let b = lit_box(&[std::sync::Arc::new(c)], &[(0, 7)]).unwrap();
        assert_eq!(b, [-3600., -3600., 4600., 4100.], "3 mm round it at least, for its neighbourhood");
        assert!(lit_box(&[], &[(0, 7)]).is_none());
    }
    #[test] fn a_probe_lights_the_pieces_themselves_not_their_boxes() {
        use super::super::plot::Piece;
        // a copper chunk: a two-segment track (info 3), another net's track (info 4), a via dot (info 3); a zone fill and a pad of info 3 on another chunk
        let track = Chunk { layer: 0, color: 0xc83434, alpha: 1.0, r: 100., pts: vec![0., 0., 1000., 0., 1000., 1000.,  5000., 0., 6000., 0.,  2000., 2000.],
            pieces: vec![Piece { info: 3, bbox: [-100., -100., 1100., 1100.], at: 0, n: 3, kind: 1 }, Piece { info: 4, bbox: [4900., -100., 6100., 100.], at: 6, n: 2, kind: 1 }, Piece { info: 3, bbox: [1900., 1900., 2100., 2100.], at: 10, n: 1, kind: 2 }], ..Default::default() };
        let fills = Chunk { layer: 1, color: 0x4d7fc4, alpha: 0.6, pts: vec![0., 0., 9000., 0., 9000., 9000.,  0., 0., 300., 0., 300., 300.],
            pieces: vec![Piece { info: 3, bbox: [0., 0., 9000., 9000.], at: 0, n: 3, kind: 0 }, Piece { info: 3, bbox: [0., 0., 300., 300.], at: 6, n: 3, kind: 0 }], ..Default::default() };
        let mark = Chunk { layer: 40, own: true, pts: vec![0., 0., 10., 0., 10., 10.], pieces: vec![Piece { info: 3, bbox: [0., 0., 10., 10.], at: 0, n: 3, kind: 0 }], ..Default::default() };
        let chunks = [std::sync::Arc::new(track), std::sync::Arc::new(fills), std::sync::Arc::new(mark)];
        let lit = [(0usize, 3usize), (1, 3), (2, 3)];
        let (ls, edges, dots) = lit_chunks(&chunks, &[(0, 0.25), (1, 1.0), (2, 1.0)], &lit, 50.);
        assert_eq!(ls.len(), 2, "the comparison's mark is boxed, not lit here");
        assert_eq!(ls[0].caps, vec![0., 0., 1000., 0., 100.,  1000., 0., 1000., 1000., 100.,  2000., 2000., 2000., 2000., 100.], "the net's track segments and its via; the other net's track is left out");
        assert_eq!((ls[0].color, ls[0].alpha), (0xc83434, 1.0), "in its own colour");
        assert_eq!(ls[1].polys.len(), 2, "its zone and its pad, as fills");
        assert_eq!(edges, vec![vec![0., 0., 300., 0., 300., 300.]], "the edge goes round the small fill (a pad), not a zone");
        assert_eq!(dots, vec![[2000., 2000., 100.]]);
        // zoomed far out, a lit stroke is never thinner than asked
        let (thin, _, d) = lit_chunks(&chunks, &[(0, 1.0)], &lit, 900.);
        assert!(thin[0].caps.chunks_exact(5).all(|s| s[4] == 450.) && d[0][2] == 450., "{:?}", thin[0].caps);
        // a hidden layer (not in the order) lights nothing
        assert!(lit_chunks(&chunks, &[(1, 1.0)], &[(0, 3)], 50.).0.is_empty());
    }
    #[test] fn an_untouched_camera_is_fitted_again_when_the_canvas_size_changes() {
        assert!(refit_needed(false, (480., 600.), 520., 930.));
        assert!(!refit_needed(false, (520., 930.), 521., 929.), "a pixel of jitter is not a change");
        assert!(!refit_needed(true, (480., 600.), 520., 930.), "a camera the person moved stays where they left it");
    }
    #[test] fn state_json_reports_gerbers_flag_and_empty_reason() {
        let v = serde_json::json!({"open": "board", "key": "", "gerbers": true, "empty": "No Gerber files in this project yet"});
        let s = state_json(&v, None, 0);
        assert_eq!((s["open"].as_str(), s["gerbers"].as_bool(), s["empty"].as_str()), (Some("board"), Some(true), Some("No Gerber files in this project yet")));
        let plain = state_json(&serde_json::json!({"open": "board", "key": "0|/p|"}), None, 0);
        assert!(plain.get("gerbers").is_none() && plain.get("empty").is_none());
    }
}
