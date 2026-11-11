//! A Board's layers as triangles (pcb3d::Layer), each layer on its own core (rayon), the big face sets cut into chunks that triangulate
//! in parallel (geom::triangulate).
//!
//! Heights (mm, z up from the laminate's bottom face): laminate 0..T with T = thickness - 2 copper; copper's outer faces at T + cu and
//! -cu; the finish one wire unit (10 um) above the copper, the mask two, the silk three (the bottom side mirrored). Every height is a whole
//! number of wire units (`heights`): the clients get positions in 10 um, so layers closer than that, or on a half unit, would round onto
//! one plane and fight (a 0.8 mm board's copper at 76.5 units and its mask at 77.5 drew as stripes of both).
use super::geom::{self, Ring, Shapes};
use super::{Board, Drill, Finish, Layer, Rgba, Side};
use rayon::prelude::*;

/// Where the board's outline came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineFrom { /** closed Edge.Cuts loops */ Joined, /** the box of the (unclosed) edge shapes */ EdgeBox, /** the box of all copper */ CopperBox, /** nothing to go on */ Nothing }

const KICAD_MASK: Rgba = Rgba { r: 20.0 / 255.0, g: 51.0 / 255.0, b: 36.0 / 255.0, a: 0.83 };
const KICAD_SILK: Rgba = Rgba { r: 245.0 / 255.0, g: 245.0 / 255.0, b: 245.0 / 255.0, a: 1.0 };
const KICAD_FR4: Rgba = Rgba { r: 109.0 / 255.0, g: 116.0 / 255.0, b: 75.0 / 255.0, a: 0.83 };
const COPPER: Rgba = Rgba { r: 184.0 / 255.0, g: 115.0 / 255.0, b: 50.0 / 255.0, a: 1.0 };
const GOLD: Rgba = Rgba { r: 178.0 / 255.0, g: 156.0 / 255.0, b: 0.0, a: 1.0 };
const SILVER: Rgba = Rgba { r: 213.0 / 255.0, g: 213.0 / 255.0, b: 213.0 / 255.0, a: 1.0 };
const TIN: Rgba = Rgba { r: 160.0 / 255.0, g: 160.0 / 255.0, b: 160.0 / 255.0, a: 1.0 };

/// A mask this dark (display numbers) is drawn opaque and only marked a coat by transparency 1/255 (glb.rs `board_color`).
const DARK_MASK: f32 = 0.12;

fn linear(c: f32) -> f32 { if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) } }
fn chan(f: f32) -> u32 { (f.clamp(0.0, 1.0) * 255.0 + 0.5) as u32 }
/// The wire colour of a display colour: linear light 0xRRGGBB, (255 - alpha) on top.
pub fn wire(c: Rgba, mask: bool) -> u32 {
    let rgb = chan(linear(c.r)) << 16 | chan(linear(c.g)) << 8 | chan(linear(c.b));
    if mask && c.r.max(c.g).max(c.b) < DARK_MASK { return rgb | 1 << 24 }
    rgb | (255 - chan(c.a)) << 24
}

/// The finish's colour; None for bare copper (nothing is drawn over it).
fn finish_color(f: Finish) -> Option<Rgba> {
    match f {
        Finish::Enig | Finish::Enepig | Finish::ImmersionGold | Finish::HardGold => Some(GOLD),
        Finish::Hasl | Finish::HaslLeadFree | Finish::ImmersionTin => Some(TIN),
        Finish::ImmersionSilver => Some(SILVER),
        Finish::None | Finish::Osp => None,
    }
}

// Mesh building -----------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct M { pos: Vec<[f32; 3]>, idx: Vec<u32> }
impl M {
    /// A flat face set at z: facing up (+z) or down.
    fn face(&mut self, s: &Shapes, z: f64, up: bool) {
        if s.is_empty() { return }
        let (pts, idx) = geom::triangulate(s);
        let base = self.pos.len() as u32;
        self.pos.extend(pts.iter().map(|p| [p[0] as f32, p[1] as f32, z as f32]));
        if up { self.idx.extend(idx.iter().map(|i| i + base)) }
        else { for t in idx.chunks_exact(3) { self.idx.extend([t[0] + base, t[2] + base, t[1] + base]) } }
    }
    /// Walls from z0 up to z1 along a ring whose material side is on its left (counter-clockwise outer; clockwise holes).
    fn wall(&mut self, ring: &Ring, ccw: bool, z0: f64, z1: f64) {
        let n = ring.len();
        if n < 3 { return }
        let flip = (geom::area2(ring) > 0.0) != ccw;
        let (z0, z1) = (z0 as f32, z1 as f32);
        for i in 0..n {
            let (p, q) = if flip { (ring[(n - i) % n], ring[(n - i - 1) % n]) } else { (ring[i], ring[(i + 1) % n]) };
            let base = self.pos.len() as u32;
            self.pos.extend([[p[0] as f32, p[1] as f32, z0], [q[0] as f32, q[1] as f32, z0], [q[0] as f32, q[1] as f32, z1], [p[0] as f32, p[1] as f32, z1]]);
            self.idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    fn walls(&mut self, s: &Shapes, z0: f64, z1: f64) {
        for sh in s { for (k, r) in sh.iter().enumerate() { self.wall(r, k == 0, z0, z1) } }
    }
    fn into_layer(self, name: &'static str, color: u32) -> Layer { Layer { name, color, pos: self.pos, idx: self.idx } }
}

// The board's pieces ------------------------------------------------------------------------------------------------------------

/// The outline as shapes (outer loops and their cutouts nested even-odd) and where it came from.
pub fn outline(b: &Board) -> (Shapes, OutlineFrom) {
    let (loops, _open) = geom::join_outline(&b.outline);
    if !loops.is_empty() { return (geom::even_odd(&loops), OutlineFrom::Joined) }
    let boxed = |rs: &[Ring], from| geom::bounds(rs).map(|(x0, y0, x1, y1)| (vec![vec![geom::norm(vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]])]], from));
    // the box of the edge shapes' own geometry (their stroke width does not count)
    let rs: Vec<Ring> = b.outline.iter().map(geom::centerline).collect();
    if let Some(r) = boxed(&rs, OutlineFrom::EdgeBox) { return r }
    let mut rs = vec![];
    for p in &b.pads { if p.layers.cu_top || p.layers.cu_bottom { for s in &p.shapes { geom::shape_rings(s, 0.0, &mut rs) } } }
    for v in &b.vias { rs.push(geom::circle([v.c.x, v.c.y], v.d / 2.0)) }
    for (_, s) in &b.copper { geom::shape_rings(s, 0.0, &mut rs) }
    for (_, z) in &b.zones { rs.push(z.iter().map(|p| [p.x, p.y]).collect()) }
    if let Some(r) = boxed(&rs, OutlineFrom::CopperBox) { return r }
    (vec![], OutlineFrom::Nothing)
}

/// Every hole in the board: pads', vias' and the loose ones.
fn drills(b: &Board) -> Vec<Drill> {
    let mut v: Vec<Drill> = b.pads.iter().filter_map(|p| p.drill).collect();
    v.extend(b.vias.iter().map(|x| Drill { c: x.c, w: x.drill, h: x.drill, rot: 0.0, plated: true }));
    v.extend(b.holes.iter().copied());
    v.retain(|d| d.w > 1e-4 && d.h > 1e-4);
    v
}

fn pt_ring(z: &[super::P2]) -> Ring { z.iter().map(|p| [p.x, p.y]).collect() }

/// Copper rings on a side (before the drills are taken out).
fn copper_rings(b: &Board, side: Side) -> Vec<Ring> {
    let mut rs: Vec<Ring> = vec![];
    for p in &b.pads {
        if (side == Side::Top && p.layers.cu_top) || (side == Side::Bottom && p.layers.cu_bottom) { for s in &p.shapes { geom::shape_rings(s, 0.0, &mut rs) } }
    }
    for v in &b.vias { rs.push(geom::norm(geom::circle([v.c.x, v.c.y], v.d / 2.0))) }
    for (s, sh) in &b.copper { if *s == side { geom::shape_rings(sh, 0.0, &mut rs) } }
    for (s, z) in &b.zones { if *s == side && z.len() >= 3 { rs.push(geom::norm(pt_ring(z))) } }
    rs
}

/// The mask's openings on a side.
fn openings(b: &Board, side: Side) -> Vec<Ring> {
    let mut rs: Vec<Ring> = vec![];
    for p in &b.pads {
        if (side == Side::Top && p.layers.mask_top) || (side == Side::Bottom && p.layers.mask_bottom) { for s in &p.shapes { geom::shape_rings(s, p.mask_margin, &mut rs) } }
    }
    for v in &b.vias {
        let tented = if side == Side::Top { v.tented_top } else { v.tented_bottom };
        if !tented { let r = v.d / 2.0 + b.stack.mask_margin; if r > 1e-6 { rs.push(geom::norm(geom::circle([v.c.x, v.c.y], r))) } }
    }
    for (s, sh) in &b.mask { if *s == side { geom::shape_rings(sh, 0.0, &mut rs) } }
    rs
}

fn silk_rings(b: &Board, side: Side) -> Vec<Ring> {
    let mut rs = vec![];
    for (s, sh) in &b.silk { if *s == side { geom::shape_rings(sh, 0.0, &mut rs) } }
    rs
}

/// The copper of a side as shapes, drills taken out.
pub fn copper_shapes(b: &Board, side: Side, drills: &[Ring]) -> Shapes {
    let rs = copper_rings(b, side);
    geom::difference(&rs, drills)
}

/// The mask of a side as shapes: the board minus the openings.
pub fn mask_shapes(b: &Board, side: Side, board_rings: &[Ring]) -> Shapes { geom::difference(board_rings, &openings(b, side)) }

/// The most triangles a board's own model may have: the phones take (64 << 20) / 84 for a whole model, and its parts need room.
pub const MOST_TRIS: usize = 600_000;
/// The details a board is meshed at, finest first (chord error mm, fewest segments a circle): the first that fits MOST_TRIS is kept.
const DETAILS: [(f64, usize); 4] = [(geom::TOL, 8), (0.015, 6), (0.04, 6), (0.1, 4)];

/// Every layer of the board, as fine as fits `MOST_TRIS` (the coarser ones run only for a board that needs them). Meshing takes turns
/// (a lock): the detail is the process's while one board is meshed.
pub fn layers(b: &Board) -> Vec<Layer> { layers_within(b, MOST_TRIS) }
/// A board with many parts leaves them more room: its own model as fine as fits this many triangles (`stream::board_budget`).
pub const MOST_TRIS_WITH_PARTS: usize = 380_000;
/// `layers`, as fine as fits `most` triangles.
pub fn layers_within(b: &Board, most: usize) -> Vec<Layer> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut best = vec![];
    for (tol, min) in DETAILS {
        geom::set_detail(tol, min);
        let ls = layers_now(b);
        let n: usize = ls.iter().map(Layer::tris).sum();
        best = ls;
        if n <= most { break }
    }
    geom::set_detail(geom::TOL, 8);
    best
}

/// The wire's unit (mm): positions reach the clients in 10 um (solid.rs `unit`).
pub const WIRE: f64 = 0.01;
/// The laminate's top T and the copper's thickness cu, each a whole number of wire units, cu at least one and the whole board as thick
/// as the file says to the unit: (T, cu).
pub fn heights(thickness: f64, copper: f64) -> (f64, f64) {
    let cu = (copper.max(0.0) / WIRE).round().max(1.0);
    let t = ((thickness / WIRE).round() - 2.0 * cu).max((0.05 / WIRE as f64).round());
    (t * WIRE, cu * WIRE)
}

fn layers_now(b: &Board) -> Vec<Layer> {
    let st = &b.stack;
    let (t, cu) = heights(st.thickness, st.copper);
    let (outline, _) = outline(b);
    let ds = drills(b);
    let drill_rings: Vec<Ring> = ds.iter().map(|d| geom::norm(geom::drill_ring(d, 0.0))).collect();
    let outline_rings = geom::flat(&outline);
    let board = geom::difference(&outline_rings, &drill_rings);
    let board_rings = geom::flat(&board);
    let fin = finish_color(st.finish);
    let copper_rgba = COPPER;
    let barrel_rgba = fin.unwrap_or(copper_rgba);

    #[derive(Clone, Copy)]
    enum K { Laminate, Holes, Copper(Side), Mask(Side), Silk(Side) }
    let tasks = [K::Copper(Side::Top), K::Copper(Side::Bottom), K::Laminate, K::Mask(Side::Top), K::Mask(Side::Bottom), K::Silk(Side::Top), K::Silk(Side::Bottom), K::Holes];
    let mut out: Vec<Layer> = tasks.par_iter().flat_map_iter(|k| -> Vec<Layer> {
        match *k {
            K::Laminate => {
                let mut m = M::default();
                m.face(&board, t, true);
                m.face(&board, 0.0, false);
                m.walls(&board, 0.0, t);
                vec![m.into_layer("laminate", wire(st.substrate.unwrap_or(KICAD_FR4), false))]
            }
            K::Holes => {
                let mut m = M::default();
                for d in ds.iter().filter(|d| d.plated) {
                    let r = geom::drill_ring(d, 0.001);
                    m.wall(&r, false, -cu, t + cu);
                }
                vec![m.into_layer("hole_walls", wire(barrel_rgba, false))]
            }
            K::Copper(side) => {
                let top = side == Side::Top;
                // clipped to the board, as KiCad's 3D view and kicad-cli are: a pad off the board's edge is not drawn
                let cu_s = geom::intersect(&geom::flat(&copper_shapes(b, side, &drill_rings)), &board_rings);
                let op = openings(b, side);
                let (zc, zf) = if top { (t + cu, t + cu + WIRE) } else { (-cu, -cu - WIRE) };
                let (a, f) = rayon::join(
                    || { let mut m = M::default(); m.face(&cu_s, zc, top); m.into_layer(if top { "copper_top" } else { "copper_bottom" }, wire(copper_rgba, false)) },
                    || fin.map(|fc| { let fs = geom::intersect(&geom::flat(&cu_s), &op); let mut m = M::default(); m.face(&fs, zf, top); m.into_layer(if top { "finish_top" } else { "finish_bottom" }, wire(fc, false)) }),
                );
                let mut v = vec![a]; v.extend(f); v
            }
            K::Mask(side) => {
                let top = side == Side::Top;
                let ms = mask_shapes(b, side, &board_rings);
                let z = if top { t + cu + 2.0 * WIRE } else { -cu - 2.0 * WIRE };
                let mut m = M::default(); m.face(&ms, z, top);
                let c = if top { st.mask_top } else { st.mask_bottom }.unwrap_or(KICAD_MASK);
                vec![m.into_layer(if top { "mask_top" } else { "mask_bottom" }, wire(c, true))]
            }
            K::Silk(side) => {
                let top = side == Side::Top;
                // the board's shape, not the mask's openings: kicad-cli draws silk over a pad as the file has it (it has no option to subtract the mask)
                let s = geom::intersect(&silk_rings(b, side), &board_rings);
                let z = if top { t + cu + 3.0 * WIRE } else { -cu - 3.0 * WIRE };
                let mut m = M::default(); m.face(&s, z, top);
                let c = if top { st.silk_top } else { st.silk_bottom }.unwrap_or(KICAD_SILK);
                vec![m.into_layer(if top { "silk_top" } else { "silk_bottom" }, wire(c, false))]
            }
        }
    }).collect();
    let rank = |n: &str| ["laminate", "hole_walls", "copper_top", "copper_bottom", "finish_top", "finish_bottom", "mask_top", "mask_bottom", "silk_top", "silk_bottom"].iter().position(|x| *x == n).unwrap_or(99);
    out.sort_by_key(|l| rank(l.name));
    out
}

#[cfg(test)]
mod tests {
    use super::super::{glbw, parse, Pad, PadLayers, Shape, Via, P2};
    use super::*;
    use std::time::Instant;

    fn seg(a: (f64, f64), b: (f64, f64)) -> Shape { Shape::Seg { a: P2::new(a.0, a.1), b: P2::new(b.0, b.1), w: 0.1 } }
    fn square(w: f64, h: f64) -> Vec<Shape> { vec![seg((0.0, 0.0), (w, 0.0)), seg((w, 0.0), (w, h)), seg((w, h), (0.0, h)), seg((0.0, h), (0.0, 0.0))] }
    fn board(w: f64, h: f64) -> Board { Board { outline: square(w, h), ..Default::default() } }
    fn layer<'a>(ls: &'a [Layer], n: &str) -> &'a Layer { ls.iter().find(|l| l.name == n).unwrap_or_else(|| panic!("no layer {n}")) }
    /// Area of the horizontal triangles at height z facing up (or down when `up` is false).
    fn flat_area(l: &Layer, z: f32, up: bool) -> f64 {
        l.idx.chunks_exact(3).map(|t| {
            let (a, b, c) = (l.pos[t[0] as usize], l.pos[t[1] as usize], l.pos[t[2] as usize]);
            if (a[2] - z).abs() > 1e-4 || (b[2] - z).abs() > 1e-4 || (c[2] - z).abs() > 1e-4 { return 0.0 }
            let s = ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])) as f64 / 2.0;
            if (s > 0.0) == up { s.abs() } else { 0.0 }
        }).sum()
    }
    fn wall_tris(l: &Layer) -> usize {
        l.idx.chunks_exact(3).filter(|t| { let z = |i: u32| l.pos[i as usize][2]; z(t[0]) != z(t[1]) || z(t[1]) != z(t[2]) }).count()
    }
    fn round_pad(x: f64, y: f64, d: f64, drill: Option<f64>) -> Pad {
        Pad { shapes: vec![Shape::Circle { c: P2::new(x, y), r: d / 2.0, w: 0.0, fill: true }], layers: PadLayers { cu_top: true, cu_bottom: true, mask_top: true, mask_bottom: true },
            drill: drill.map(|w| Drill { c: P2::new(x, y), w, h: w, rot: 0.0, plated: true }), mask_margin: 0.0, reference: String::new() }
    }
    fn poly_area(n: usize, r: f64) -> f64 { n as f64 * r * r * (2.0 * std::f64::consts::PI / n as f64).sin() / 2.0 }

    #[test]
    fn every_layer_lies_on_its_own_wire_plane() {
        // the clients get positions in 10 um: a 0.8 mm board's copper (76.5 units) and mask (77.5) rounded onto one plane and fought
        for (th, cu) in [(0.8, 0.035), (1.6, 0.035), (1.6, 0.07), (1.0, 0.0175), (0.4, 0.012), (1.57, 0.0)] {
            let mut b = board(10.0, 10.0);
            b.stack.thickness = th; b.stack.copper = cu; b.stack.finish = Finish::Enig;
            b.copper.push((Side::Top, seg((2.0, 5.0), (8.0, 5.0)))); b.copper.push((Side::Bottom, seg((2.0, 5.0), (8.0, 5.0))));
            b.pads.push(round_pad(5.0, 2.0, 1.0, None));
            b.silk.push((Side::Top, seg((2.0, 8.0), (8.0, 8.0)))); b.silk.push((Side::Bottom, seg((2.0, 8.0), (8.0, 8.0))));
            let ls = layers(&b);
            let plane = |n: &str, top: bool| {
                let zs: Vec<f64> = layer(&ls, n).pos.iter().map(|p| p[2] as f64 / WIRE).collect();
                for z in &zs { assert!((z - z.round()).abs() < 0.01, "{th}/{cu}: {n} at {z} units, not on the grid"); }
                zs.into_iter().map(|z| z.round() as i64).fold(None, |a: Option<i64>, z| Some(a.map_or(z, |a| if top { a.max(z) } else { a.min(z) })))
            };
            let up = ["laminate", "copper_top", "finish_top", "mask_top", "silk_top"].map(|n| plane(n, true).unwrap_or_else(|| panic!("{th}/{cu}: no {n}")));
            let down = ["laminate", "copper_bottom", "finish_bottom", "mask_bottom", "silk_bottom"].map(|n| plane(n, false).unwrap());
            assert!(up.windows(2).all(|w| w[0] < w[1]) && down.windows(2).all(|w| w[0] > w[1]), "{th}/{cu}: planes up {up:?} down {down:?}");
            assert_eq!(up[1] - down[1], (th / WIRE).round() as i64, "{th}/{cu}: copper to copper is the board's thickness");
        }
    }
    #[test]
    fn square_with_hole_and_slot() {
        let mut b = board(20.0, 10.0);
        b.pads.push(round_pad(5.0, 5.0, 3.0, Some(2.0)));
        b.holes.push(Drill { c: P2::new(14.0, 5.0), w: 4.0, h: 1.0, rot: 0.0, plated: false });
        let ls = layers(&b);
        let t = heights(1.6, 0.035).0;
        let lam = layer(&ls, "laminate");
        let hole = poly_area(geom::full_steps(1.0), 1.0);
        let slot = 3.0 * 1.0 + std::f64::consts::PI * 0.25; // chords of the caps: a little under
        let top = flat_area(lam, t as f32, true);
        assert!((200.0 - top - hole - slot).abs() < 0.05, "top {top}");
        assert!((flat_area(lam, 0.0, false) - top).abs() < 1e-3);
        assert!(wall_tris(lam) > 2 * (8 + 8), "hole walls in the laminate");
        let hw = layer(&ls, "hole_walls");
        assert!(hw.tris() >= 2 * 8, "barrel of the plated hole");
        assert_eq!(wall_tris(hw), hw.tris());
        // walls face the hole axis: for the first barrel quad, the normal points to (5,5)
        let t0 = &hw.idx[0..3];
        let (p, q, r) = (hw.pos[t0[0] as usize], hw.pos[t0[1] as usize], hw.pos[t0[2] as usize]);
        let n = [(q[1] - p[1]) * (r[2] - p[2]) - (q[2] - p[2]) * (r[1] - p[1]), (q[2] - p[2]) * (r[0] - p[0]) - (q[0] - p[0]) * (r[2] - p[2])];
        let to_axis = [5.0 - p[0], 5.0 - p[1]];
        assert!(n[0] * to_axis[0] + n[1] * to_axis[1] > 0.0, "barrel normal points inward");
        // the outer wall faces away
        let outer: Vec<_> = lam.idx.chunks_exact(3).filter(|t| (0..3).all(|k| lam.pos[t[k] as usize][1] == 0.0)).collect();
        assert!(!outer.is_empty());
        let t0 = outer[0];
        let (p, q, r) = (lam.pos[t0[0] as usize], lam.pos[t0[1] as usize], lam.pos[t0[2] as usize]);
        let ny = (q[2] - p[2]) * (r[0] - p[0]) - (q[0] - p[0]) * (r[2] - p[2]);
        assert!(ny < 0.0, "the wall on y=0 faces -y");
    }

    #[test]
    fn copper_and_silk_off_the_board_are_not_drawn() {
        // a 10 x 10 board; one pad on it, one beyond its edge; silk crossing the edge is cut at it (KiCad's 3D view and kicad-cli clip to the board)
        let mut b = board(10.0, 10.0);
        b.pads = vec![round_pad(5.0, 5.0, 1.0, None), round_pad(15.0, 5.0, 1.0, None)];
        b.silk.push((Side::Top, Shape::Seg { a: P2::new(8.0, 2.0), b: P2::new(12.0, 2.0), w: 0.2 }));
        let ls = layers(&b);
        let xs = |n: &str| layer(&ls, n).pos.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        assert!(xs("copper_top") <= 10.0 + 1e-4 && xs("copper_top") > 5.0, "the far pad is gone: {}", xs("copper_top"));
        assert!((xs("silk_top") - 10.0).abs() < 1e-4, "silk stops at the edge: {}", xs("silk_top"));
        // silk over a pad's opening stays, as kicad-cli draws it
        b.silk.push((Side::Top, Shape::Seg { a: P2::new(4.0, 5.0), b: P2::new(6.0, 5.0), w: 0.2 }));
        let ls = layers(&b);
        let on_pad = layer(&ls, "silk_top").pos.iter().any(|p| p[0] > 5.9 && (p[1] - 5.0).abs() < 0.2);
        assert!(on_pad);
    }
    #[test]
    fn track_minus_via_drill() {
        let mut b = board(20.0, 10.0);
        b.copper.push((Side::Top, Shape::Seg { a: P2::new(2.0, 5.0), b: P2::new(12.0, 5.0), w: 0.4 }));
        let no_via = copper_shapes(&b, Side::Top, &[]);
        let full = geom::area(&no_via);
        assert!((full - (10.0 * 0.4 + poly_area(geom::full_steps(0.2), 0.2))).abs() < 0.01, "{full}");
        b.vias.push(Via { c: P2::new(7.0, 5.0), d: 0.2, drill: 0.1, tented_top: true, tented_bottom: true });
        // a pad-less via inside the track: copper is track ∪ via pad − drill
        let ds = vec![geom::norm(geom::circle([7.0, 5.0], 0.05))];
        let cut = geom::area(&copper_shapes(&b, Side::Top, &ds));
        assert!((full - cut - poly_area(geom::full_steps(0.05), 0.05)).abs() < 1e-4, "{full} {cut}");
        let ls = layers(&b);
        let cu = layer(&ls, "copper_top");
        let (t, c) = heights(1.6, 0.035);
        assert!((flat_area(cu, (t + c) as f32, true) - cut).abs() < 0.01, "{}", flat_area(cu, (t + c) as f32, true));
    }

    #[test]
    fn mask_openings_grow_and_shrink() {
        let mut b = board(20.0, 10.0);
        let rect = Shape::Rect { c: P2::new(10.0, 5.0), w: 1.0, h: 1.0, rot: 0.0, r: 0.0, chamfer: 0.0, chamfer_corners: 0 };
        let mk = |m: f64| Pad { shapes: vec![rect.clone()], layers: PadLayers { cu_top: true, cu_bottom: false, mask_top: true, mask_bottom: false }, drill: None, mask_margin: m, reference: String::new() };
        let board_rings = geom::flat(&geom::difference(&geom::flat(&outline(&b).0), &[]));
        let base = 200.0;
        for (m, open) in [(0.0, 1.0), (0.1, 1.44 - 4.0 * 0.01 * (1.0 - std::f64::consts::FRAC_PI_4)), (-0.1, 0.64)] {
            b.pads = vec![mk(m)];
            let a = geom::area(&mask_shapes(&b, Side::Top, &board_rings));
            assert!((base - a - open).abs() < 0.01, "margin {m}: opening {} want {open}", base - a);
        }
        // the bottom has no opening
        assert!((geom::area(&mask_shapes(&b, Side::Bottom, &board_rings)) - base).abs() < 1e-6);
        // a via opens when not tented, with the board's margin
        b.pads.clear();
        b.stack.mask_margin = 0.05;
        b.vias.push(Via { c: P2::new(3.0, 3.0), d: 0.6, drill: 0.3, tented_top: false, tented_bottom: true });
        let a = geom::area(&mask_shapes(&b, Side::Top, &board_rings));
        assert!((base - a - poly_area(geom::full_steps(0.35), 0.35)).abs() < 0.01);
        assert!((geom::area(&mask_shapes(&b, Side::Bottom, &board_rings)) - base).abs() < 1e-6);
    }

    #[test]
    fn outline_joins_out_of_order_with_an_arc() {
        // 10 x 10 with the top-right corner a quarter circle of radius 3, strokes shuffled, one reversed
        let r = 3.0;
        let c = (7.0, 3.0);
        let k = std::f64::consts::FRAC_1_SQRT_2;
        let shapes = vec![
            seg((0.0, 10.0), (0.0, 0.0)),
            Shape::Arc { a: P2::new(10.0, 3.0), m: P2::new(c.0 + r * k, c.1 - r * k), b: P2::new(7.0, 0.0), w: 0.05 },
            seg((10.0, 10.0), (0.0, 10.0)),
            seg((0.0, 0.0), (7.0, 0.0)),
            seg((10.0, 3.0), (10.0, 10.0)),
        ];
        let (loops, open) = geom::join_outline(&shapes);
        assert_eq!(loops.len(), 1);
        assert!(!open);
        let b = Board { outline: shapes, ..Default::default() };
        let (s, from) = outline(&b);
        assert_eq!(from, OutlineFrom::Joined);
        let want = 100.0 - (9.0 - std::f64::consts::PI * 9.0 / 4.0);
        assert!((geom::area(&s) - want).abs() < 0.02, "{} vs {want}", geom::area(&s));
    }

    #[test]
    fn outline_nests_cutouts_and_falls_back() {
        let mut o = square(20.0, 20.0);
        o.push(Shape::Circle { c: P2::new(10.0, 10.0), r: 3.0, w: 0.05, fill: false });
        let (s, from) = outline(&Board { outline: o, ..Default::default() });
        assert_eq!(from, OutlineFrom::Joined);
        assert!((geom::area(&s) - (400.0 - poly_area(geom::full_steps(3.0), 3.0))).abs() < 0.01);
        // unclosed edge: the box of the shapes
        let (s, from) = outline(&Board { outline: vec![seg((0.0, 0.0), (10.0, 0.0)), seg((10.0, 0.0), (10.0, 5.0))], ..Default::default() });
        assert_eq!(from, OutlineFrom::EdgeBox);
        assert!((geom::area(&s) - 50.0 - 0.0).abs() < 0.5, "{}", geom::area(&s));
        // no edge: the copper's box
        let mut b = Board::default();
        b.copper.push((Side::Top, Shape::Seg { a: P2::new(0.0, 0.0), b: P2::new(10.0, 0.0), w: 1.0 }));
        let (_, from) = outline(&b);
        assert_eq!(from, OutlineFrom::CopperBox);
        assert_eq!(outline(&Board::default()).1, OutlineFrom::Nothing);
    }

    #[test]
    fn colors() {
        let w = wire(Rgba { r: 1.0, g: 0.0, b: 0.5, a: 1.0 }, false);
        assert_eq!(w >> 24, 0);
        assert_eq!(w >> 16 & 255, 255);
        assert_eq!(w >> 8 & 255, 0);
        assert_eq!(w & 255, 55); // 0.5 sRGB is 0.2140 linear
        assert_eq!(wire(KICAD_MASK, true) >> 24, 255 - 212); // 0.83 * 255 = 211.65
        let black = Rgba { r: 0.05, g: 0.05, b: 0.05, a: 0.5 };
        assert_eq!(wire(black, true) >> 24, 1, "a dark mask is opaque with a coat mark");
        assert_eq!(wire(black, false) >> 24, 255 - 128, "other layers keep their alpha");
        let ls = layers(&board(5.0, 5.0));
        assert_eq!(layer(&ls, "mask_top").color >> 24, 255 - 212);
        let mut b = board(5.0, 5.0);
        b.stack.mask_top = Some(black);
        assert_eq!(layer(&layers(&b), "mask_top").color >> 24, 1);
        b.stack.finish = Finish::Enig;
        assert!(layers(&b).iter().any(|l| l.name == "hole_walls" && l.color == wire(GOLD, false)));
    }

    fn assert_round_trip(ls: &[Layer]) -> usize {
        let want = glbw::tris(ls);
        let bytes = glbw::glb(ls, "test");
        let got = super::super::super::glb::tris(&bytes).expect("reads");
        assert_eq!(got.len(), want.len(), "triangle count");
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(g.color, w.color);
            for (p, q) in [(g.a, w.a), (g.b, w.b), (g.c, w.c)] { for k in 0..3 { assert!((p[k] - q[k]).abs() < 1e-6, "{p:?} vs {q:?}") } }
        }
        got.len()
    }

    #[test]
    fn glb_round_trip_and_board_coordinates() {
        let mut b = board(20.0, 10.0);
        b.pads.push(round_pad(5.0, 5.0, 3.0, Some(2.0)));
        b.silk.push((Side::Top, seg((1.0, 1.0), (4.0, 2.0))));
        let ls = layers(&b);
        let n = assert_round_trip(&ls);
        assert_eq!(n, ls.iter().map(|l| l.tris()).sum::<usize>());
        let ts = glbw::tris(&ls);
        let s = super::super::super::solid::of(&ts).expect("a solid");
        let _ = s;
        // board coordinates come back: x in 0..20, y in 0..10, z in -0.035..1.6 + a little
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for t in &ts { for v in [t.a, t.b, t.c] { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]) } } }
        assert!(lo[0].abs() < 1e-6 && (hi[0] - 0.020).abs() < 1e-6, "x {lo:?} {hi:?}");
        assert!(lo[2].abs() < 1e-6 && (hi[2] - 0.010).abs() < 1e-6, "board y is glTF z");
        assert!(lo[1] < 0.0 && hi[1] > 0.0015, "board z is glTF y");
        // the top face points up in glTF (+y)
        let lam = layer(&ls, "laminate");
        let up = glbw::tris(std::slice::from_ref(lam)).iter().filter(|t| t.a[1] > 0.0015 && t.b[1] > 0.0015 && t.c[1] > 0.0015).all(|t| {
            let (u, v) = ([t.b[0] - t.a[0], t.b[1] - t.a[1], t.b[2] - t.a[2]], [t.c[0] - t.a[0], t.c[1] - t.a[1], t.c[2] - t.a[2]]);
            u[2] * v[0] - u[0] * v[2] > 0.0
        });
        assert!(up);
    }

    #[test]
    fn indices_split_past_65535_vertices() {
        let n = 30000; // 90000 vertices, no sharing
        let mut pos = vec![]; let mut idx = vec![];
        for i in 0..n { let x = (i % 200) as f32; let y = (i / 200) as f32; let b = pos.len() as u32; pos.extend([[x, y, 0.0], [x + 0.5, y, 0.0], [x, y + 0.5, 0.0]]); idx.extend([b, b + 1, b + 2]) }
        let big = Layer { name: "copper_top", color: 0x00AABBCC, pos, idx };
        assert_eq!(assert_round_trip(std::slice::from_ref(&big)), n);
        let bytes = glbw::glb(std::slice::from_ref(&big), "t");
        let jl = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json = String::from_utf8_lossy(&bytes[20..20 + jl]).to_string();
        assert_eq!(json.matches("\"POSITION\"").count(), 2, "two primitives");
        assert!(json.contains("\"componentType\":5123") && !json.contains("\"componentType\":5125"));
        assert!(json.contains("pcb3d_copper_top"));
        for (i, _) in json.match_indices("\"type\":\"VEC3\"") { let at = json[..i].rfind("\"count\":").unwrap(); let c: usize = json[at + 8..].split(',').next().unwrap().parse().unwrap(); assert!(c <= 65535) }
    }

    /// BACKPLANE_TEST_BOARD=a.kicad_pcb:b.kicad_pcb cargo test --release pcb3d::mesh::tests::bench -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench() {
        let Ok(paths) = std::env::var("BACKPLANE_TEST_BOARD") else { return };
        for p in paths.split(':').filter(|p| !p.is_empty()) {
            let bytes = std::fs::read(p).expect("board file");
            let b = parse::board(&bytes).expect("parse");
            let _ = layers(&b); // warm the thread pool
            let (mut best, mut ls) = (u128::MAX, vec![]);
            for _ in 0..5 { let t = Instant::now(); ls = layers(&b); best = best.min(t.elapsed().as_micros()) }
            let t = Instant::now();
            let bytes = glbw::glb(&ls, "bench");
            let glb_ms = t.elapsed().as_secs_f64() * 1000.0;
            let total: usize = ls.iter().map(|l| l.tris()).sum();
            println!("{p}\n  mesh {:.1} ms (best of 5), glb {:.1} ms, {} bytes, {} triangles", best as f64 / 1000.0, glb_ms, bytes.len(), total);
            let (o, from) = outline(&b);
            println!("  outline {:?}, area {:.1} mm2; pads {} vias {} zones {} copper {} silk {} thickness {}", from, geom::area(&o), b.pads.len(), b.vias.len(), b.zones.len(), b.copper.len(), b.silk.len(), b.stack.thickness);
            for l in &ls { println!("    {:12} {:7} tris", l.name, l.tris()) }
            assert!(total < 798_000);
        }
    }
}
