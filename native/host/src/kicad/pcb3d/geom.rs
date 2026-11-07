//! 2D geometry for the board's layers: shapes to polygons, booleans (i_overlay), triangulation (i_triangle).
//!
//! Rings are `Vec<[f64; 2]>` in board coordinates. `norm` turns a ring to the orientation the overlay library wants for a filled outer
//! contour (found once at run time, so a library change cannot flip us); a set of such rings unions with the non-zero rule.
use super::{Drill, P2, Shape};
use i_triangle::i_overlay::core::fill_rule::FillRule;
use i_triangle::i_overlay::core::overlay_rule::OverlayRule;
use i_triangle::i_overlay::float::overlay::FloatOverlay;
use i_triangle::i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_triangle::i_overlay::mesh::float::style::{LineJoin, OutlineStyle};
use i_triangle::float::triangulatable::Triangulatable;
use rayon::prelude::*;
use std::f64::consts::PI;
use std::sync::OnceLock;

pub type Pt = [f64; 2];
pub type Ring = Vec<Pt>;
/// A shape: its outer ring, then its holes.
pub type Shapes = Vec<Vec<Ring>>;

/// Chord error of a curve, mm: the finest, what a board is meshed at unless that is too many triangles (mesh::layers).
pub const TOL: f64 = 0.005;
/// The chord error and the fewest segments of a full circle now: (TOL, 8) but while mesh::layers meshes a big board coarser, under its lock.
static DETAIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn detail() -> (f64, usize) { match DETAIL.load(std::sync::atomic::Ordering::Relaxed) { 0 => (TOL, 8), v => (f64::from_bits(v & !0xff), (v & 0xff) as usize) } }
/// Set the detail (`tol` mm, at least `min` segments a circle); (TOL, 8) is the finest. Only mesh::layers calls it, holding its lock.
pub fn set_detail(tol: f64, min: usize) { DETAIL.store(if tol == TOL && min == 8 { 0 } else { (tol.to_bits() & !0xff) | min.min(255) as u64 }, std::sync::atomic::Ordering::Relaxed) }

/// Twice the signed area of a ring, positive when it runs counter-clockwise in a y-up plane (a +z normal).
pub fn area2(r: &[Pt]) -> f64 {
    let mut s = 0.0;
    for i in 0..r.len() { let (a, b) = (r[i], r[(i + 1) % r.len()]); s += a[0] * b[1] - b[0] * a[1] }
    s
}

/// Which sign of `area2` the library gives a filled outer contour (+1 or -1).
fn lib_sign() -> f64 {
    static S: OnceLock<f64> = OnceLock::new();
    *S.get_or_init(|| {
        let sq: Vec<Ring> = vec![vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]];
        let r = FloatOverlay::<Pt>::with_subj_and_clip(&sq, &Vec::<Ring>::new()).overlay(OverlayRule::Subject, FillRule::NonZero);
        match r.first().and_then(|s| s.first()) { Some(o) if area2(o) < 0.0 => -1.0, _ => 1.0 }
    })
}

/// A ring in the library's orientation for a filled contour.
pub fn norm(mut r: Ring) -> Ring { if area2(&r) * lib_sign() < 0.0 { r.reverse() } r }
fn hole(mut r: Ring) -> Ring { if area2(&r) * lib_sign() > 0.0 { r.reverse() } r }

fn pt(p: P2) -> Pt { [p.x, p.y] }

/// Segments for a full circle of radius r at the chord error `TOL`: 8 at least, 128 at most.
pub fn full_steps(r: f64) -> usize {
    let (tol, min) = detail();
    if r <= tol { return min }
    ((PI / (1.0 - tol / r).acos()).ceil() as usize).clamp(min, 128)
}
fn steps_for(r: f64, sweep: f64) -> usize { (((full_steps(r) as f64) * sweep.abs() / (2.0 * PI)).ceil() as usize).max(1) }

pub fn circle(c: Pt, r: f64) -> Ring {
    let n = full_steps(r);
    (0..n).map(|i| { let a = 2.0 * PI * i as f64 / n as f64; [c[0] + r * a.cos(), c[1] + r * a.sin()] }).collect()
}

/// Points along an arc about c from angle a0 sweeping by `sweep`, endpoints included.
fn arc_run(c: Pt, r: f64, a0: f64, sweep: f64, out: &mut Ring) {
    let n = steps_for(r, sweep);
    for i in 0..=n { let a = a0 + sweep * i as f64 / n as f64; out.push([c[0] + r * a.cos(), c[1] + r * a.sin()]) }
}

/// A stroke from a to b with round ends, width w.
pub fn capsule(a: Pt, b: Pt, w: f64) -> Ring {
    let r = w / 2.0;
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    if dx.hypot(dy) < 1e-9 { return circle(a, r) }
    let ang = dy.atan2(dx);
    let mut out = Ring::new();
    arc_run(b, r, ang - PI / 2.0, PI, &mut out);
    arc_run(a, r, ang + PI / 2.0, PI, &mut out);
    out
}

/// The centre, radius, start angle and signed sweep of the arc through a, m, b (a to b); None when they are on a line.
pub fn arc_of(a: Pt, m: Pt, b: Pt) -> Option<(Pt, f64, f64, f64)> {
    let d = 2.0 * (a[0] * (m[1] - b[1]) + m[0] * (b[1] - a[1]) + b[0] * (a[1] - m[1]));
    if d.abs() < 1e-12 { return None }
    let (a2, m2, b2) = (a[0] * a[0] + a[1] * a[1], m[0] * m[0] + m[1] * m[1], b[0] * b[0] + b[1] * b[1]);
    let ux = (a2 * (m[1] - b[1]) + m2 * (b[1] - a[1]) + b2 * (a[1] - m[1])) / d;
    let uy = (a2 * (b[0] - m[0]) + m2 * (a[0] - b[0]) + b2 * (m[0] - a[0])) / d;
    let c = [ux, uy];
    let r = (a[0] - ux).hypot(a[1] - uy);
    let ang = |p: Pt| (p[1] - uy).atan2(p[0] - ux);
    let (a0, am, a1) = (ang(a), ang(m), ang(b));
    let norm2pi = |x: f64| x.rem_euclid(2.0 * PI);
    // counter-clockwise (in math terms) from a through m to b?
    let ccw = norm2pi(am - a0) < norm2pi(a1 - a0);
    let sweep = if ccw { norm2pi(a1 - a0) } else { norm2pi(a1 - a0) - 2.0 * PI };
    Some((c, r, a0, sweep))
}

/// The points along the arc a, m, b (ends included); a line when they are collinear.
pub fn arc_points(a: Pt, m: Pt, b: Pt) -> Ring {
    match arc_of(a, m, b) {
        Some((c, r, a0, sw)) => { let mut v = Ring::new(); arc_run(c, r, a0, sw, &mut v); v[0] = a; let n = v.len(); v[n - 1] = b; v }
        None => vec![a, b],
    }
}

/// An arc stroked with width w and round ends: a ring.
fn arc_stroke(a: Pt, m: Pt, b: Pt, w: f64, out: &mut Vec<Ring>) {
    let Some((c, r, a0, sw)) = arc_of(a, m, b) else { out.push(norm(capsule(a, b, w))); return };
    let h = w / 2.0;
    if r <= h || sw.abs() >= 2.0 * PI - 1e-6 { // a stub or a whole circle: chords
        let pts = arc_points(a, m, b);
        for p in pts.windows(2) { out.push(norm(capsule(p[0], p[1], w))) }
        return;
    }
    let n = steps_for(r + h, sw);
    let at = |rad: f64, i: usize| { let t = a0 + sw * i as f64 / n as f64; [c[0] + rad * t.cos(), c[1] + rad * t.sin()] };
    let mut ring = Ring::new();
    for i in 0..=n { ring.push(at(r + h, i)) }
    // the end cap at b: a half circle about b's centreline point from the outer side to the inner
    let dir = if sw > 0.0 { 1.0 } else { -1.0 };
    let tb = a0 + sw;
    let eb = [c[0] + r * tb.cos(), c[1] + r * tb.sin()];
    let outward_b = tb.sin().atan2(tb.cos());
    let start_b = outward_b; // outer point direction from the centre of the cap is the radial direction
    let cap_sweep = dir * PI; // continues past the end, turning towards the inner side
    let mut cap = Ring::new();
    arc_run(eb, h, start_b, cap_sweep, &mut cap);
    ring.extend(cap.into_iter().skip(1));
    for i in (0..=n).rev() { ring.push(at(r - h, i)) }
    let ea = [c[0] + r * a0.cos(), c[1] + r * a0.sin()];
    let mut cap = Ring::new();
    arc_run(ea, h, a0 + PI, dir * PI, &mut cap); // from the inner side round the back to the outer
    ring.extend(cap.into_iter().skip(1));
    out.push(norm(ring));
}

fn rot(p: Pt, deg: f64) -> Pt {
    let (s, c) = deg.to_radians().sin_cos();
    [p[0] * c + p[1] * s, -p[0] * s + p[1] * c]
}

/// A rectangle w x h about c turned by `deg` (KiCad's sense), corners rounded by r and the `chamfer_corners` cut by `chamfer`.
pub fn rect(c: Pt, w: f64, h: f64, deg: f64, r: f64, chamfer: f64, corners: u8) -> Ring {
    let (hw, hh) = (w / 2.0, h / 2.0);
    let r = r.clamp(0.0, hw.min(hh));
    let ch = chamfer.clamp(0.0, hw.min(hh) * 2.0);
    // corners in order around the rectangle (y down): top-left, top-right, bottom-right, bottom-left, as (sign x, sign y, bit)
    let cs = [(-1.0, -1.0, 1u8), (1.0, -1.0, 2), (1.0, 1.0, 8), (-1.0, 1.0, 4)];
    let mut pts = Ring::new();
    for (k, &(sx, sy, bit)) in cs.iter().enumerate() {
        let (px, py) = (sx * hw, sy * hh);
        if corners & bit != 0 && ch > 0.0 {
            // cut: the two points on the edges adjacent to the corner, walking the order
            let prev = cs[(k + 3) % 4]; let next = cs[(k + 1) % 4];
            let from_prev = if prev.0 != sx { (px - sx * ch, py) } else { (px, py - sy * ch) };
            let to_next = if next.0 != sx { (px - sx * ch, py) } else { (px, py - sy * ch) };
            pts.push([from_prev.0, from_prev.1]); pts.push([to_next.0, to_next.1]);
        } else if r > 1e-9 {
            let (cx, cy) = (px - sx * r, py - sy * r);
            // walking top-left, top-right, bottom-right, bottom-left (counter-clockwise in a y-up plane): each corner's arc start
            let base = match k { 0 => PI, 1 => 1.5 * PI, 2 => 0.0, _ => 0.5 * PI };
            arc_run([cx, cy], r, base, PI / 2.0, &mut pts);
        } else { pts.push([px, py]) }
    }
    pts.into_iter().map(|p| { let q = rot(p, deg); [c[0] + q[0], c[1] + q[1]] }).collect()
}

/// A drill's outline: a circle, or a slot (an oval w x h turned by rot).
pub fn drill_ring(d: &Drill, shrink: f64) -> Ring {
    let (w, h) = ((d.w - 2.0 * shrink).max(1e-4), (d.h - 2.0 * shrink).max(1e-4));
    let c = pt(d.c);
    if (w - h).abs() < 1e-6 { return circle(c, w / 2.0) }
    rect(c, w, h, d.rot, w.min(h) / 2.0, 0.0, 0)
}

/// Rings for a shape grown by `m` mm (negative shrinks), appended to `out`; strokes are included. Orientation is `norm`ed.
pub fn shape_rings(s: &Shape, m: f64, out: &mut Vec<Ring>) {
    match s {
        Shape::Seg { a, b, w } => { let w = w + 2.0 * m; if w > 1e-6 { out.push(norm(capsule(pt(*a), pt(*b), w))) } }
        Shape::Arc { a, m: mid, b, w } => { let w = w + 2.0 * m; if w > 1e-6 { arc_stroke(pt(*a), pt(*mid), pt(*b), w, out) } }
        Shape::Circle { c, r, w, fill } => {
            let outer = r + w / 2.0 + m;
            if *fill { if outer > 1e-6 { out.push(norm(circle(pt(*c), outer))) } }
            else {
                let inner = r - w / 2.0 - m;
                if outer > 1e-6 && outer > inner {
                    out.push(norm(circle(pt(*c), outer)));
                    if inner > 1e-6 { out.push(hole(circle(pt(*c), inner))) }
                }
            }
        }
        Shape::Poly { pts, w, fill } => {
            let ring: Ring = pts.iter().map(|p| pt(*p)).collect();
            if *fill && ring.len() >= 3 {
                if m.abs() < 1e-9 { out.push(norm(ring.clone())) } else { for sh in offset_ring(&ring, m) { out.extend(sh) } }
            }
            let sw = w + 2.0 * m;
            if sw > 1e-6 && (*w > 0.0 || !*fill) && pts.len() >= 2 {
                let n = pts.len();
                for i in 0..n { let (a, b) = (pts[i], pts[(i + 1) % n]); if n > 2 || i == 0 { out.push(norm(capsule(pt(a), pt(b), sw))) } }
            }
        }
        Shape::Rect { c, w, h, rot, r, chamfer, chamfer_corners } => {
            let (w2, h2) = (w + 2.0 * m, h + 2.0 * m);
            if w2 > 1e-6 && h2 > 1e-6 {
                let r2 = if m > 0.0 && *r >= 0.0 && *chamfer_corners == 0 { r + m } else { (r + m).max(0.0) };
                out.push(norm(rect(pt(*c), w2, h2, *rot, r2, *chamfer, *chamfer_corners)));
            }
        }
    }
}

/// A ring grown by m (the polygon offset), as shapes.
pub fn offset_ring(r: &Ring, m: f64) -> Shapes {
    let style = OutlineStyle::new(m).line_join(LineJoin::Round(0.2));
    let ring = norm(r.clone());
    let out = vec![ring].outline(&style);
    out
}

/// Boolean of two ring sets (non-zero fill): subj op clip.
pub fn bool_op(subj: &[Ring], clip: &[Ring], rule: OverlayRule) -> Shapes {
    if subj.is_empty() { return vec![] }
    FloatOverlay::<Pt>::with_subj_and_clip(&subj.to_vec(), &clip.to_vec()).overlay(rule, FillRule::NonZero)
}
pub fn union(rings: &[Ring]) -> Shapes { bool_op(rings, &[], OverlayRule::Subject) }
pub fn difference(subj: &[Ring], clip: &[Ring]) -> Shapes { if clip.is_empty() { union(subj) } else { bool_op(subj, clip, OverlayRule::Difference) } }
pub fn intersect(subj: &[Ring], clip: &[Ring]) -> Shapes { if clip.is_empty() { vec![] } else { bool_op(subj, clip, OverlayRule::Intersect) } }

/// A shape set's rings, flat (outers and holes), for use as the next operation's input.
pub fn flat(s: &Shapes) -> Vec<Ring> { s.iter().flat_map(|sh| sh.iter().cloned()).collect() }

/// Even-odd union of nested loops (outline loops, cutouts).
pub fn even_odd(loops: &[Ring]) -> Shapes {
    if loops.is_empty() { return vec![] }
    FloatOverlay::<Pt>::with_subj(&loops.to_vec()).overlay(OverlayRule::Subject, FillRule::EvenOdd)
}

/// Area of a shape set, mm2.
pub fn area(s: &Shapes) -> f64 {
    s.iter().map(|sh| sh.iter().enumerate().map(|(i, r)| { let a = area2(r).abs() / 2.0; if i == 0 { a } else { -a } }).sum::<f64>()).sum()
}

/// Triangles of a shape set: points and u32 indices, every triangle counter-clockwise (+z normal). Big sets are cut into chunks that
/// triangulate on their own cores.
pub fn triangulate(s: &Shapes) -> (Vec<Pt>, Vec<u32>) {
    const CHUNK: usize = 400;
    let parts: Vec<(Vec<Pt>, Vec<u32>)> = s.par_chunks(CHUNK).map(|ch| {
        let t = ch.triangulate().to_triangulation::<u32>();
        (t.points, t.indices)
    }).collect();
    let (mut pts, mut idx) = (Vec::with_capacity(parts.iter().map(|p| p.0.len()).sum()), Vec::with_capacity(parts.iter().map(|p| p.1.len()).sum()));
    for (p, i) in parts {
        let base = pts.len() as u32;
        pts.extend(p);
        idx.extend(i.into_iter().map(|x| x + base));
    }
    for t in idx.chunks_exact_mut(3) {
        let (a, b, c) = (pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]);
        if (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]) < 0.0 { t.swap(1, 2) }
    }
    (pts, idx)
}

/// The bounding box of some rings: (min x, min y, max x, max y).
pub fn bounds(rs: &[Ring]) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    for p in rs.iter().flatten() {
        b = Some(match b { None => (p[0], p[1], p[0], p[1]), Some((a, c, d, e)) => (a.min(p[0]), c.min(p[1]), d.max(p[0]), e.max(p[1])) });
    }
    b
}

/// A shape's own geometry as points, stroke width left out (a circle by its extent).
pub fn centerline(s: &Shape) -> Ring {
    match s {
        Shape::Seg { a, b, .. } => vec![pt(*a), pt(*b)],
        Shape::Arc { a, m, b, .. } => arc_points(pt(*a), pt(*m), pt(*b)),
        Shape::Circle { c, r, .. } => vec![[c.x - r, c.y - r], [c.x + r, c.y + r]],
        Shape::Poly { pts, .. } => pts.iter().map(|p| pt(*p)).collect(),
        Shape::Rect { c, w, h, rot, r, chamfer, chamfer_corners } => rect(pt(*c), *w, *h, *rot, *r, *chamfer, *chamfer_corners),
    }
}

// Outline joining ---------------------------------------------------------------------------------------------------------------

/// Joins strokes (segments, arcs, circles, rects, polygons) into closed loops by matching endpoints within 1 um. Closed shapes
/// are loops by themselves. Returns the loops and whether anything was left open.
pub fn join_outline(shapes: &[Shape]) -> (Vec<Ring>, bool) {
    let mut loops = Vec::new();
    let mut open: Vec<Ring> = Vec::new();
    for s in shapes {
        match s {
            Shape::Seg { a, b, .. } => open.push(vec![pt(*a), pt(*b)]),
            Shape::Arc { a, m, b, .. } => open.push(arc_points(pt(*a), pt(*m), pt(*b))),
            Shape::Circle { c, r, .. } => if *r > 1e-6 { loops.push(circle(pt(*c), *r)) },
            Shape::Rect { c, w, h, rot, r, chamfer, chamfer_corners } => loops.push(rect(pt(*c), *w, *h, *rot, *r, *chamfer, *chamfer_corners)),
            Shape::Poly { pts, .. } => if pts.len() >= 3 { loops.push(pts.iter().map(|p| pt(*p)).collect()) } else if pts.len() == 2 { open.push(pts.iter().map(|p| pt(*p)).collect()) },
        }
    }
    const E: f64 = 1e-3;
    let near = |a: Pt, b: Pt| (a[0] - b[0]).abs() <= E && (a[1] - b[1]).abs() <= E;
    let mut leftover = false;
    while let Some(mut cur) = open.pop() {
        loop {
            if cur.len() > 2 && near(cur[0], *cur.last().unwrap()) { cur.pop(); loops.push(cur); break }
            let end = *cur.last().unwrap();
            let mut found = None;
            for (i, o) in open.iter().enumerate() {
                if near(o[0], end) { found = Some((i, false)); break }
                if near(*o.last().unwrap(), end) { found = Some((i, true)); break }
            }
            match found {
                Some((i, rev)) => { let mut o = open.swap_remove(i); if rev { o.reverse() } cur.pop(); cur.extend(o) }
                None => {
                    // try the other end
                    let start = cur[0];
                    let mut f2 = None;
                    for (i, o) in open.iter().enumerate() {
                        if near(*o.last().unwrap(), start) { f2 = Some((i, false)); break }
                        if near(o[0], start) { f2 = Some((i, true)); break }
                    }
                    match f2 {
                        Some((i, rev)) => { let mut o = open.swap_remove(i); if rev { o.reverse() } o.pop(); o.extend(cur); cur = o }
                        None => { leftover = true; break }
                    }
                }
            }
        }
    }
    (loops, leftover)
}
