//! A plot chunk's triangles in plot space (micrometres), made once and kept: fills when the chunk is first painted, strokes per
//! stroke look (the width a hairline is drawn at, round ends, circle sides: what changes with zoom). Triangles are grouped by area so a
//! paint walks only the groups in view; the canvas (screens::plotview) then only maps the kept triangles to pixels, so a pan or a zoom
//! does no tessellation. Pure, tested.
use super::plot::Chunk;
use std::{collections::HashMap, sync::Arc};

/// Triangles `tris[at..end]` lie in `bbox` (x0 y0 x1 y1, micrometres).
#[derive(Clone, Debug, PartialEq)]
pub struct Group { pub bbox: [f32; 4], pub at: usize, pub end: usize }
/// Triangles as (x0 y0 x1 y1 x2 y2), in groups.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geom { pub tris: Vec<[f32; 6]>, pub groups: Vec<Group>, pub bbox: [f32; 4] }

/// What a chunk's strokes look like at a scale (pixels per micrometre): the half width (micrometres; a stroke thinner than a pixel is drawn
/// one pixel wide), the sides of its dots and round ends, whether a stroke gets round ends (3 px or wider), as the canvas always drew them,
/// and `tol`: a polyline's points closer than that (0.4 px) to the last point kept are skipped, the shape stays within it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look { pub hw: f32, pub sides: usize, pub round: bool, pub thin: bool, pub tol: f32 }
pub fn look(c: &Chunk, scale: f32) -> Option<Look> {
    let r = *c.caps.get(4)?;
    let width = (r * 2. * scale).max(1.0);
    let thin = r * 2. * scale < 1.0;
    // a dot under 2 px across is a square, under 4 px a hexagon: more sides there only add triangles nobody sees
    let sides = if width < 2. { 4 } else if width < 4. { 6 } else { ((6. + width) as usize).clamp(8, 32) };
    Some(Look { hw: if thin { 0.5 / scale } else { r }, sides, round: width >= 3.0, thin, tol: 0.4 / scale })
}
/// The scale strokes are made at: the view's scale rounded to a quarter octave, so a zoom remakes them a few times, not on every frame,
/// and a hairline stays within 9% of a pixel.
pub fn bucket(scale: f32) -> f32 { ((scale.max(1e-9).log2() * 4.).round() / 4.).exp2() }
/// The octave fills are simplified for, and how far (micrometres) an outline may move there: a quarter pixel at the octave's top.
pub fn fill_level(scale: f32) -> i32 { scale.max(1e-9).log2().floor() as i32 }
pub fn fill_tol(level: i32) -> f32 { 0.25 / (level as f32 + 1.).exp2() }

fn bbox_of(tris: &[[f32; 6]]) -> [f32; 4] {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for t in tris { for k in 0..3 { b[0] = b[0].min(t[2 * k]); b[1] = b[1].min(t[2 * k + 1]); b[2] = b[2].max(t[2 * k]); b[3] = b[3].max(t[2 * k + 1]); } }
    b
}
/// Group triangles on a grid over their box (by centre), so the triangles of a big chunk off the screen are skipped a group at a time.
pub fn grouped(mut tris: Vec<[f32; 6]>) -> Geom {
    if tris.is_empty() { return Geom::default() }
    let bbox = bbox_of(&tris);
    let n = tris.len();
    let g = ((n as f32 / 1024.).sqrt().ceil() as usize).clamp(1, 16);
    if g == 1 { return Geom { groups: vec![Group { bbox, at: 0, end: n }], tris, bbox } }
    let (w, h) = ((bbox[2] - bbox[0]).max(1e-3), (bbox[3] - bbox[1]).max(1e-3));
    let cell = |t: &[f32; 6]| {
        let (x, y) = ((t[0] + t[2] + t[4]) / 3., (t[1] + t[3] + t[5]) / 3.);
        let (i, j) = ((((x - bbox[0]) / w) * g as f32) as usize, (((y - bbox[1]) / h) * g as f32) as usize);
        i.min(g - 1) + j.min(g - 1) * g
    };
    let mut keyed: Vec<(u16, [f32; 6])> = tris.drain(..).map(|t| (cell(&t) as u16, t)).collect();
    keyed.sort_by_key(|k| k.0);
    let mut groups = Vec::new(); let mut at = 0;
    while at < keyed.len() {
        let c = keyed[at].0; let mut end = at;
        while end < keyed.len() && keyed[end].0 == c { end += 1; }
        groups.push((at, end)); at = end;
    }
    let tris: Vec<[f32; 6]> = keyed.into_iter().map(|k| k.1).collect();
    let groups = groups.into_iter().map(|(at, end)| Group { bbox: bbox_of(&tris[at..end]), at, end }).collect();
    Geom { tris, groups, bbox }
}

/// The chunk's strokes at `look`: each segment a quad of the half width, each dot (a zero-length segment) a circle, and with round ends a
/// circle at each end (once where a polyline's segments meet). A polyline's points closer than `look.tol` to the last one drawn are
/// skipped (the next segment reaches from that point), and a polyline shorter than that is still drawn, as a short stroke.
pub fn strokes(c: &Chunk, look: Look) -> Geom {
    let n = look.sides;
    let unit: Vec<(f32, f32)> = (0..n).map(|k| { let t = std::f32::consts::TAU * k as f32 / n as f32; (t.cos() * look.hw, t.sin() * look.hw) }).collect();
    let mut out: Vec<[f32; 6]> = Vec::with_capacity(c.caps.len() / 5 * if look.round { 2 + (n - 2) } else { 2 });
    let circle = |out: &mut Vec<[f32; 6]>, (x, y): (f32, f32)| {
        let p0 = (x + unit[0].0, y + unit[0].1);
        for k in 1..n - 1 { out.push([p0.0, p0.1, x + unit[k].0, y + unit[k].1, x + unit[k + 1].0, y + unit[k + 1].1]); }
    };
    let emit = |out: &mut Vec<[f32; 6]>, (ax, ay): (f32, f32), (bx, by): (f32, f32), first: bool| {
        let (dx, dy) = (bx - ax, by - ay); let len = (dx * dx + dy * dy).sqrt();
        let (nx, ny) = (-dy / len * look.hw, dx / len * look.hw);
        out.push([ax + nx, ay + ny, bx + nx, by + ny, bx - nx, by - ny]);
        out.push([ax + nx, ay + ny, bx - nx, by - ny, ax - nx, ay - ny]);
        if look.round { if first { circle(out, (ax, ay)); } circle(out, (bx, by)); }
    };
    let tol2 = look.tol * look.tol;
    // the polyline being drawn: its last point drawn to, whether anything was drawn yet, and where it really ends so far
    let mut chain: Option<((f32, f32), bool, (f32, f32))> = None;
    let flush = |out: &mut Vec<[f32; 6]>, chain: &mut Option<((f32, f32), bool, (f32, f32))>| {
        if let Some((at, drawn, end)) = chain.take() { if end != at { emit(out, at, end, !drawn); } }
    };
    for s in c.caps.chunks_exact(5) {
        let (a, b) = ((s[0], s[1]), (s[2], s[3]));
        if a == b { flush(&mut out, &mut chain); circle(&mut out, a); continue }
        if !matches!(chain, Some((_, _, end)) if end == a) { flush(&mut out, &mut chain); chain = Some((a, false, a)); }
        let Some((at, drawn, end)) = chain.as_mut() else { continue };
        let (dx, dy) = (b.0 - at.0, b.1 - at.1);
        if dx * dx + dy * dy < tol2 { *end = b; continue }
        emit(&mut out, *at, b, !*drawn); *at = b; *drawn = true; *end = b;
    }
    flush(&mut out, &mut chain);
    grouped(out)
}

/// A ring simplified to `tol` (Lang's method over at most 32 points at a time): a point is dropped when every point it stands for lies
/// within `tol` of the line that replaces them, so straight runs and arcs far below a pixel keep few points. A ring that would keep
/// fewer than three keeps four of its points spread around it (a small pad or hole stays, as a pixel's worth of quad).
fn decimate(ring: &[f32], tol: f32, out: &mut Vec<f32>) {
    let n = ring.len() / 2; let at = out.len();
    if n < 4 { out.extend_from_slice(ring); return }
    let p = |i: usize| (ring[2 * i], ring[2 * i + 1]);
    let tol2 = tol * tol;
    // the squared distance of q from the segment a-b
    let off = |q: (f32, f32), a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1); let l2 = dx * dx + dy * dy;
        let t = if l2 > 0. { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l2).clamp(0., 1.) } else { 0. };
        let (x, y) = (a.0 + t * dx - q.0, a.1 + t * dy - q.1); x * x + y * y
    };
    let mut a = 0; out.extend([ring[0], ring[1]]);
    let mut i = 2;
    while i < n {
        let far = i - a > 32 || (a + 1..i).any(|k| off(p(k), p(a), p(i)) > tol2);
        if far { a = i - 1; out.extend([ring[2 * a], ring[2 * a + 1]]); i = a + 2; } else { i += 1; }
    }
    out.extend([ring[2 * (n - 1)], ring[2 * (n - 1) + 1]]);
    if out.len() - at < 6 || (out.len() - at == 6 && { let q = &out[at..]; ((q[2] - q[0]) * (q[5] - q[1]) - (q[4] - q[0]) * (q[3] - q[1])).abs() < tol2 }) {
        out.truncate(at);
        for k in 0..4 { let i = k * n / 4; out.extend([ring[2 * i], ring[2 * i + 1]]); }
    }
}
/// The chunk's polygons filled together (even-odd, as the canvas always filled them: a hole drawn inside an outline stays open), their
/// outlines simplified to `tol` micrometres (0: as they are).
pub fn fills(c: &Chunk, tol: f32) -> Geom {
    use lyon::{math::{point, Point}, path::Path, tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers}};
    if c.polys.is_empty() { return Geom::default() }
    let mut b = Path::builder(); let mut ring: Vec<f32> = Vec::new();
    for poly in &c.polys {
        ring.clear(); if tol > 0. { decimate(poly, tol, &mut ring); } else { ring.extend_from_slice(poly); }
        let mut it = ring.chunks_exact(2);
        let Some(p) = it.next() else { continue };
        b.begin(point(p[0], p[1])); for q in it { b.line_to(point(q[0], q[1])); } b.end(true);
    }
    let path = b.build();
    let mut buf: VertexBuffers<Point, u32> = VertexBuffers::new();
    let ok = FillTessellator::new().tessellate_path(&path, &FillOptions::default(), &mut BuffersBuilder::new(&mut buf, |v: FillVertex| v.position())).is_ok();
    if !ok { return Geom::default() }
    let v = &buf.vertices;
    grouped(buf.indices.chunks_exact(3).map(|i| { let (a, b, c) = (v[i[0] as usize], v[i[1] as usize], v[i[2] as usize]); [a.x, a.y, b.x, b.y, c.x, c.y] }).collect())
}
/// How many outline points the chunk's polygons keep at `tol`.
fn kept(c: &Chunk, tol: f32) -> usize { let mut v = Vec::new(); let mut n = 0; for p in &c.polys { v.clear(); decimate(p, tol, &mut v); n += v.len() / 2; } n }

/// One chunk's kept triangles: its fills per octave they were simplified for (an octave whose outlines keep nearly every point shares the
/// exact fill), the fill on show, the strokes and the look they were made at.
pub struct Entry { chunk: Arc<Chunk>, fills: Vec<(i32, Arc<Geom>)>, pub fill: Arc<Geom>, pub stroke: Geom, look: Option<Look>, level: Option<i32>, pub bbox: [f32; 4] }
fn union(a: [f32; 4], b: [f32; 4]) -> [f32; 4] { [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])] }
impl Entry {
    /// The box of what is drawn: the fill on show and the strokes.
    fn span(&self) -> [f32; 4] { match (self.fill.tris.is_empty(), self.stroke.tris.is_empty()) { (false, false) => union(self.fill.bbox, self.stroke.bbox), (true, false) => self.stroke.bbox, _ => self.fill.bbox } }
}

/// The triangles a canvas keeps, by chunk. A chunk the plot store no longer holds (only this cache does) is dropped.
#[derive(Default)]
pub struct Cache { map: HashMap<usize, Entry>, pub made: usize }
impl Cache {
    /// The chunk's triangles at `scale`; made when missing, or remade when a zoom changed how its strokes look or the octave its fills are
    /// simplified for (each octave's fill is kept). Returns it and whether anything was made.
    pub fn get(&mut self, c: &Arc<Chunk>, scale: f32) -> (&Entry, bool) {
        let key = Arc::as_ptr(c) as usize;
        let want = look(c, bucket(scale));
        let level = (!c.polys.is_empty()).then(|| fill_level(scale));
        let mut made = false;
        let e = self.map.entry(key).or_insert_with(|| Entry { chunk: c.clone(), fills: Vec::new(), fill: Arc::new(Geom::default()), stroke: Geom::default(), look: None, level: None, bbox: [0.; 4] });
        if e.look != want {
            if let Some(l) = want { e.stroke = strokes(c, l); } else { e.stroke = Geom::default(); }
            e.look = want; made = true;
        }
        if e.level != level {
            if let Some(lv) = level {
                let g = match e.fills.iter().find(|f| f.0 == lv) {
                    Some(f) => f.1.clone(),
                    None => {
                        let total: usize = c.polys.iter().map(|p| p.len() / 2).sum();
                        let tol = fill_tol(lv);
                        let g = if kept(c, tol) * 100 >= total * 85 {
                            match e.fills.iter().find(|f| f.0 == i32::MAX) { Some(f) => f.1.clone(), None => { let g = Arc::new(fills(c, 0.)); e.fills.push((i32::MAX, g.clone())); g } }
                        } else { Arc::new(fills(c, tol)) };
                        e.fills.push((lv, g.clone())); g
                    }
                };
                e.fill = g;
            }
            e.level = level; made = true;
        }
        if made { self.made += 1; e.bbox = e.span(); }
        (e, made)
    }
    /// Make what `get` would make for these chunks at `scale`, on several cores (a first paint, a zoom past a quarter octave); `get` then
    /// only finds it.
    pub fn make_all(&mut self, cs: &[&Arc<Chunk>], scale: f32) {
        enum Fill { No, Level(i32, bool) } // the octave to make, and whether the exact fill is kept already
        let mut jobs: Vec<(&Arc<Chunk>, Option<Look>, Fill)> = Vec::new();
        for c in cs {
            let want = look(c, bucket(scale));
            let level = (!c.polys.is_empty()).then(|| fill_level(scale));
            let e = self.map.get(&(Arc::as_ptr(c) as usize));
            let stroke = want.filter(|_| e.is_none_or(|e| e.look != want));
            let fill = match (level, e) {
                (Some(lv), Some(e)) if e.level == level || e.fills.iter().any(|f| f.0 == lv) => Fill::No,
                (Some(lv), e) => Fill::Level(lv, e.is_some_and(|e| e.fills.iter().any(|f| f.0 == i32::MAX))),
                (None, _) => Fill::No,
            };
            if stroke.is_some() || matches!(fill, Fill::Level(..)) { jobs.push((c, stroke, fill)); }
        }
        if jobs.len() < 16 { return } // get makes the few itself
        let work = |(c, stroke, fill): &(&Arc<Chunk>, Option<Look>, Fill)| -> (Option<Geom>, Option<(i32, bool, Geom)>) {
            let st = stroke.map(|l| strokes(c, l));
            let fi = match fill { Fill::No => None, Fill::Level(lv, has_full) => {
                let total: usize = c.polys.iter().map(|p| p.len() / 2).sum(); let tol = fill_tol(*lv);
                if kept(c, tol) * 100 >= total * 85 { if *has_full { None } else { Some((*lv, true, fills(c, 0.))) } } else { Some((*lv, false, fills(c, tol))) }
            } };
            (st, fi)
        };
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(6);
        let per = jobs.len().div_ceil(threads);
        let out: Vec<(Option<Geom>, Option<(i32, bool, Geom)>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = jobs.chunks(per).map(|part| sc.spawn(move || part.iter().map(work).collect::<Vec<_>>())).collect();
            hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        for ((c, stroke, _), (st, fi)) in jobs.iter().zip(out) {
            let e = self.map.entry(Arc::as_ptr(c) as usize).or_insert_with(|| Entry { chunk: (*c).clone(), fills: Vec::new(), fill: Arc::new(Geom::default()), stroke: Geom::default(), look: None, level: None, bbox: [0.; 4] });
            if let Some(g) = st { e.stroke = g; e.look = *stroke; e.bbox = e.span(); self.made += 1; } // (a new fill octave is taken up, and the box made again, by get)
            match fi {
                Some((lv, true, g)) => { let g = Arc::new(g); e.fills.push((i32::MAX, g.clone())); e.fills.push((lv, g)); }
                Some((lv, false, g)) => e.fills.push((lv, Arc::new(g))),
                None => { if let Some(full) = e.fills.iter().find(|f| f.0 == i32::MAX).map(|f| f.1.clone()) { if let Some(lv) = (!c.polys.is_empty()).then(|| fill_level(scale)) { if !e.fills.iter().any(|f| f.0 == lv) { e.fills.push((lv, full)); } } } }
            }
        }
    }
    /// The chunk's triangles as last made, if any.
    pub fn peek(&self, c: &Arc<Chunk>) -> Option<&Entry> { self.map.get(&(Arc::as_ptr(c) as usize)) }
    /// Forget the chunks nobody else holds (the store let them go).
    pub fn prune(&mut self) { self.map.retain(|_, e| Arc::strong_count(&e.chunk) > 1); }
    pub fn len(&self) -> usize { self.map.len() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn chunk(caps: Vec<f32>, polys: Vec<Vec<f32>>) -> Arc<Chunk> { Arc::new(Chunk { caps, polys, alpha: 1., ..Default::default() }) }
    fn area(t: &[f32; 6]) -> f32 { ((t[2] - t[0]) * (t[5] - t[1]) - (t[4] - t[0]) * (t[3] - t[1])).abs() / 2. }
    #[test] fn a_segment_is_a_quad_of_its_width_and_a_dot_a_circle() {
        let c = chunk(vec![0., 0., 1000., 0., 100.], vec![]); // 1 mm long, 0.2 mm wide
        let g = strokes(&c, look(&c, 0.001).unwrap()); // 0.2 px wide: a hairline, one pixel (1000 um at this scale)
        assert_eq!(g.tris.len(), 2); let a: f32 = g.tris.iter().map(area).sum(); assert!((a - 1000. * 1000.).abs() < 1., "{a}");
        let l = look(&c, 0.1).unwrap(); assert!(!l.thin && l.round && l.hw == 100.); // 20 px wide: its true width, round ends
        let g = strokes(&c, l); let a: f32 = g.tris.iter().map(area).sum();
        assert!(a > 1000. * 200. && a < 1000. * 200. + std::f32::consts::PI * 100. * 100. * 2.01, "quad and two end circles: {a}");
        let d = chunk(vec![5., 5., 5., 5., 50.], vec![]); let g = strokes(&d, look(&d, 0.1).unwrap());
        let a: f32 = g.tris.iter().map(area).sum(); assert!((a - std::f32::consts::PI * 2500.).abs() < 0.05 * std::f32::consts::PI * 2500., "{a}");
    }
    #[test] fn a_polyline_gets_one_circle_where_its_segments_meet() {
        let c = chunk(vec![0., 0., 1000., 0., 100., 1000., 0., 1000., 1000., 100.], vec![]);
        let l = look(&c, 0.1).unwrap(); let g = strokes(&c, l);
        assert_eq!(g.tris.len(), 2 * 2 + 3 * (l.sides - 2), "two quads and three circles");
    }
    #[test] fn fills_are_even_odd_so_a_hole_stays_open() {
        let outer = vec![0., 0., 100., 0., 100., 100., 0., 100.]; let hole = vec![25., 25., 75., 25., 75., 75., 25., 75.];
        let g = fills(&chunk(vec![], vec![outer.clone(), hole]), 0.);
        let a: f32 = g.tris.iter().map(area).sum(); assert!((a - (10000. - 2500.)).abs() < 1., "{a}");
        let g = fills(&chunk(vec![], vec![outer]), 0.); let a: f32 = g.tris.iter().map(area).sum(); assert!((a - 10000.).abs() < 1.);
    }
    #[test] fn zoomed_out_outlines_and_polylines_keep_fewer_points_and_small_pieces_stay() {
        let ring: Vec<f32> = (0..2000).flat_map(|k| { let t = std::f32::consts::TAU * k as f32 / 2000.; [5000. * t.cos(), 5000. * t.sin()] }).collect();
        let pad = vec![9000., 0., 9030., 0., 9030., 30., 9000., 30.]; // outside the ring (inside it would be a hole)
        let c = chunk(vec![], vec![ring, pad]);
        let fine = fills(&c, 0.); let coarse = fills(&c, fill_tol(fill_level(0.002)));
        assert!(coarse.tris.len() * 4 < fine.tris.len(), "{} vs {}", coarse.tris.len(), fine.tris.len());
        let (fa, ca): (f32, f32) = (fine.tris.iter().map(area).sum(), coarse.tris.iter().map(area).sum());
        assert!((fa - ca).abs() / fa < 0.02, "the same area within the outline's tolerance: {fa} {ca}");
        assert!(coarse.tris.iter().any(|t| t.iter().step_by(2).all(|x| *x >= 9000. && *x <= 9030.)), "the small pad is kept");
        // a wiggly polyline of 1 um steps at a scale where 0.4 px is 400 um: one stroke from end to end
        let mut caps = Vec::new(); for i in 0..1000 { let x = i as f32; caps.extend([x, (i % 2) as f32, x + 1., ((i + 1) % 2) as f32, 50.]); }
        let p = chunk(caps, vec![]); let g = strokes(&p, look(&p, 0.001).unwrap());
        assert!(g.tris.len() <= 8, "{}", g.tris.len());
        let xs: Vec<f32> = g.tris.iter().flat_map(|t| [t[0], t[2], t[4]]).collect();
        assert!(xs.iter().cloned().fold(f32::MAX, f32::min) <= 0.5 && xs.iter().cloned().fold(f32::MIN, f32::max) >= 999.5);
        let iso = chunk(vec![0., 0., 1., 0., 50.], vec![]); assert_eq!(strokes(&iso, look(&iso, 0.001).unwrap()).tris.len(), 2, "a piece shorter than the tolerance still shows");
    }
    #[test] fn making_on_several_cores_gives_what_get_makes() {
        let cs: Vec<Arc<Chunk>> = (0..40).map(|i| { let x = i as f32 * 1000.; chunk(vec![x, 0., x + 500., 0., 40.], vec![vec![x, 100., x + 300., 100., x + 300., 400., x, 400.]]) }).collect();
        let (mut a, mut b) = (Cache::default(), Cache::default());
        let refs: Vec<&Arc<Chunk>> = cs.iter().collect();
        for scale in [0.01, 0.013, 0.2] {
            b.make_all(&refs, scale);
            for c in &cs { let (ea, _) = a.get(c, scale); let (fa, sa, ba) = ((*ea.fill).clone(), ea.stroke.clone(), ea.bbox); let (eb, _) = b.get(c, scale); assert_eq!((fa, sa, ba), ((*eb.fill).clone(), eb.stroke.clone(), eb.bbox)); }
        }
    }
    #[test] fn a_big_chunk_is_grouped_by_area_and_keeps_every_triangle() {
        let mut caps = Vec::new(); for i in 0..5000 { let x = (i % 100) as f32 * 100.; let y = (i / 100) as f32 * 100.; caps.extend([x, y, x + 50., y, 10.]); }
        let c = chunk(caps, vec![]); let g = strokes(&c, look(&c, 0.001).unwrap());
        assert_eq!(g.tris.len(), 10000); assert!(g.groups.len() > 4);
        assert_eq!(g.groups.iter().map(|q| q.end - q.at).sum::<usize>(), 10000);
        for q in &g.groups { for t in &g.tris[q.at..q.end] { for k in 0..3 { assert!(t[2 * k] >= q.bbox[0] && t[2 * k] <= q.bbox[2] && t[2 * k + 1] >= q.bbox[1] && t[2 * k + 1] <= q.bbox[3]); } } }
    }
    #[test] fn the_cache_remakes_strokes_only_when_their_look_changes_and_drops_what_the_store_let_go() {
        let c = chunk(vec![0., 0., 1000., 0., 100.], vec![vec![0., 0., 10., 0., 10., 10.]]);
        let mut k = Cache::default();
        assert!(k.get(&c, 0.1).1, "made"); assert!(!k.get(&c, 0.1).1, "kept"); assert!(!k.get(&c, 0.104).1, "the same quarter octave");
        let thin = chunk(vec![0., 0., 1000., 0., 1.], vec![]);
        assert!(k.get(&thin, 0.01).1); assert!(k.get(&thin, 0.02).1, "a hairline is remade a pixel wide at the new zoom");
        assert_eq!(k.len(), 2); drop(thin); k.prune(); assert_eq!(k.len(), 1);
        assert!((bucket(0.1) / 0.1 - 1.).abs() < 0.1 && bucket(0.1) == bucket(0.104));
    }
}
