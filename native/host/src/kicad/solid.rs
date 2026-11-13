//! core/plot's solids in Rust: a 3D model's triangles for the phones' 3D view, in board axes (x, y as on the board, z up) and 10
//! micrometre units: each triangle is the XOR of its colour with the one before, then its 9 coordinates as zigzag deltas, varints.
//! Grid.thin is the vertex clustering the phones and the web get a model through. The same bytes as `Line.solid`'s `m` line.
use super::{fl, glb::Tri3, plot::{Plot, Rect, Solid}, plotgen::um};

/// Solid.most: the biggest GLB the hub reads into a solid. reader took about 300 bytes of memory per byte; this reader does
/// not, but the limit is the product's rule and its words, so the same models show.
pub const MOST: u64 = 12582912;
pub fn fits(size: u64) -> bool { size <= MOST }
/// The biggest STEP mesh the hub reads (one it made: its triangles are then held to `STEP_TRIS` by `fit`, not by the file's size).
pub const MOST_STEP: u64 = 64 << 20;
/// Whether a model's GLB of `size` bytes is read: a STEP's mesh up to MOST_STEP, any other up to MOST.
pub fn fits_for(size: u64, step: bool) -> bool { size <= if step { MOST_STEP } else { MOST } }
/// The most triangles a STEP's model goes out with: a phone takes (64 << 20) / 84 for a model, and a frame of about 10 bytes a triangle
/// stays under the hub's 8 MB outbox. A model within it is sent exactly as meshed.
pub const STEP_TRIS: usize = 600_000;
/// Solid.big: what a refused model says (its size in whole megabytes, rounded up)
pub fn big(size: u64) -> String { format!("This 3D model is too big to show here ({} MB; the hub reads models up to {} MB)", (size + 1048575) >> 20, MOST >> 20) }

/// Mech.upright: CAD parts are Z up, the viewer's world is Y up: (x, y, z) goes to (x, z, -y)
pub fn upright(ts: &mut [Tri3]) { for t in ts { for v in [&mut t.a, &mut t.b, &mut t.c] { *v = [v[0], v[2], -v[1]] } } }
/// Mech.metres: a STEP's millimetres in glTF's metres
pub fn metres(ts: &mut [Tri3]) { for t in ts { for v in [&mut t.a, &mut t.b, &mut t.c] { *v = [v[0] * 0.001, v[1] * 0.001, v[2] * 0.001] } } }

/// Grid.at: a coordinate's cell (F32.to_u32: a negative or a fraction under 1 is 0)
fn cell_of(v: f32, o: f32, s: f32) -> u32 { fl::to_u32(((v - o) / s).floor()) }
/// Grid.snap: a coordinate at its cell's centre
fn snap(v: f32, o: f32, s: f32) -> f32 { o + (((v - o) / s).floor() + 0.5) * s }
/// Solid.thin: every corner moves across to the centre of its cell in a grid a 4000th of the model's size, a triangle two of whose
/// corners land in one cell is dropped. y, the model's up, keeps its value, so a board's thin layers stay apart. A thousandth moved a
/// 60 mm part's corners by up to 30 um: its small holes and fillets came out dented and shaded in blocks.
pub fn thin(ts: Vec<Tri3>) -> Vec<Tri3> { thin_by(ts, 4000.0) }
/// A STEP's triangles within `most`: as they are when they fit (thinning snaps corners to a grid, which made a small hole in a 2 m
/// assembly a jagged 16-gon), else thinned on the finest grid that fits.
pub fn fit(ts: Vec<Tri3>, most: usize) -> Vec<Tri3> {
    if ts.len() <= most { return ts }
    let mut last = vec![];
    for cells in [64000.0, 16000.0, 4000.0, 1000.0, 250.0] { last = thin_by(ts.clone(), cells); if last.len() <= most { break } }
    last
}
/// `thin` on a grid of `cells` a side of the model's size (fewer cells: fewer triangles, coarser).
pub fn thin_by(ts: Vec<Tri3>, cells: f32) -> Vec<Tri3> {
    let big = 1000000000.0f32;
    let (mut lo, mut hi) = ([big; 3], [-big; 3]);
    for t in &ts { for v in [t.a, t.b, t.c] { for k in 0..3 { lo[k] = fl::min(lo[k], v[k]); hi[k] = fl::max(hi[k], v[k]) } } }
    let d = |k: usize| fl::max(hi[k] - lo[k], 0.0);
    let (dx, dy, dz) = (d(0), d(1), d(2));
    let s = (dx * dx + (dy * dy + dz * dz)).sqrt() / cells;
    if !(s > 0.0) { return ts }
    let same = |p: [f32; 3], q: [f32; 3]| (0..3).all(|k| cell_of(p[k], lo[k], s) == cell_of(q[k], lo[k], s));
    let moved = |v: [f32; 3]| [snap(v[0], lo[0], s), v[1], snap(v[2], lo[2], s)];
    ts.into_iter().filter(|t| !(same(t.a, t.b) || same(t.b, t.c) || same(t.a, t.c))).map(|t| Tri3 { a: moved(t.a), b: moved(t.b), c: moved(t.c), color: t.color }).collect()
}

fn zig(d: u32) -> u32 { (d << 1) ^ 0u32.wrapping_sub(d >> 31) }
fn num(o: &mut Vec<u8>, mut z: u32) { while z >= 128 { o.push((z & 127) as u8 | 128); z >>= 7 } o.push(z as u8) }
/// SE.u: a glTF coordinate (metres, y up) in 10 micrometre units
fn unit(m: f32) -> u32 { um(m * 100.0) }
/// Each triangle's corners in the wire's integers (board axes, 10 micrometre units), as `of` writes them, and its colour: a part frame
/// of these with the identity placement draws exactly where `of`'s model does (a STEP's streamed parts, then its whole model).
pub fn ints(ts: &[Tri3]) -> impl Iterator<Item = ([i32; 9], u32)> + '_ {
    ts.iter().map(|t| { let mut o = [0i32; 9]; for (k, v) in [t.a, t.b, t.c].into_iter().enumerate() { o[3 * k] = unit(v[0]) as i32; o[3 * k + 1] = unit(v[2]) as i32; o[3 * k + 2] = unit(v[1]) as i32 } (o, t.color) })
}
/// Each triangle's face as runs (Solid::faces): a varint a face, how many triangles in a row are of it. Empty when `faces` is not one a
/// triangle (a model thinned, or one whose mesher said none).
pub fn face_runs(faces: &[u32], n: usize) -> Vec<u8> {
    if faces.len() != n || n == 0 { return vec![] }
    let mut o = vec![]; let mut run = 1u32;
    for w in faces.windows(2) { if w[1] == w[0] { run += 1 } else { num(&mut o, run); run = 1 } }
    num(&mut o, run); o
}
/// A part frame's `n` and `mesh` (plot::part_frame) of these triangles, placed by the identity: the same bytes as `of`'s mesh.
pub fn part_mesh(ts: &[Tri3]) -> (u32, Vec<u8>) {
    let (mut d, mut c, mut p) = (Vec::with_capacity(ts.len() * 16), 0u32, [0u32; 3]);
    for (t, color) in ints(ts) { num(&mut d, color ^ c); c = color; for k in 0..3 { for a in 0..3 { let v = t[3 * k + a] as u32; num(&mut d, zig(v.wrapping_sub(p[a]))); p[a] = v } } }
    (ts.len() as u32, d)
}
/// Solid.of: a model's triangles as a solid; None for no triangles (Line.solid prints nothing for one)
pub fn of(ts: &[Tri3]) -> Option<Solid> {
    if ts.is_empty() { return None }
    let mut d = Vec::with_capacity(ts.len() * 16);
    let (mut px, mut py, mut pz, mut c) = (0u32, 0u32, 0u32, 0u32);
    let mut b: Option<[i32; 6]> = None; // SE.box compares as two's complement (Box.lt)
    for t in ts {
        num(&mut d, t.color ^ c); c = t.color;
        for v in [t.a, t.b, t.c] {
            let (x, y, z) = (unit(v[0]), unit(v[2]), unit(v[1]));
            num(&mut d, zig(x.wrapping_sub(px))); num(&mut d, zig(y.wrapping_sub(py))); num(&mut d, zig(z.wrapping_sub(pz)));
            (px, py, pz) = (x, y, z);
            let (i, j, k) = (x as i32, y as i32, z as i32);
            b = Some(match b { None => [i, j, k, i, j, k], Some([x0, y0, z0, x1, y1, z1]) => [x0.min(i), y0.min(j), z0.min(k), x1.max(i), y1.max(j), z1.max(k)] });
        }
    }
    Some(Solid { n: ts.len() as u32, b: b.unwrap().map(|v| v as u32), d, faces: vec![] })
}
/// Plot.of.solid: the plot of a model: no chunks, Box.empty, thickness 0 (None: Line.solid prints nothing and no mesh goes out)
pub fn plot(so: Option<Solid>) -> Plot {
    let e = Rect { x0: 2147483647, y0: 2147483647, x1: 2147483648, y1: 2147483648 };
    Plot { cs: vec![], b: e, thick: 0, solid: so, edge: e, note: String::new(), warn: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tri(a: [f32; 3], b: [f32; 3], c: [f32; 3], color: u32) -> Tri3 { Tri3 { a, b, c, color } }
    fn read(d: &[u8]) -> Vec<u32> { let (mut o, mut z, mut s) = (vec![], 0u32, 0); for b in d { z |= ((*b & 127) as u32) << s; s += 7; if b & 128 == 0 { o.push(z); z = 0; s = 0 } } o }
    fn unzig(z: u32) -> i32 { ((z >> 1) ^ 0u32.wrapping_sub(z & 1)) as i32 }
    #[test] fn a_step_within_its_budget_is_kept_exact_and_one_over_it_thinned_as_little_as_fits() {
        let fine: Vec<Tri3> = (0..200).map(|i| { let a = i as f32 * 0.0314; tri([a.cos() * 0.003, a.sin() * 0.003, 0.0], [(a + 0.0314).cos() * 0.003, (a + 0.0314).sin() * 0.003, 0.0], [0.0, 0.0, 0.0], 1) }).collect();
        let mut model = fine.clone(); model.push(tri([2.0, 0.0, 0.0], [2.0, 0.001, 0.0], [2.0, 0.0, 0.001], 2)); // a 6 mm hole's rim in a 2 m model
        assert_eq!(fit(model.clone(), 1000), model, "within the budget: not one corner moved");
        assert!(thin(model.clone()).len() < 120, "the old grid (a 4000th of 2 m: 0.5 mm) kept {} of the 6 mm rim's 200", thin(model.clone()).len());
        let f = fit(model.clone(), 199); assert!(f.len() <= 199 && f.len() > 150, "over it: the finest grid that fits ({})", f.len());
        assert!(fits_for(MOST_STEP, true) && !fits_for(MOST + 1, false) && fits_for(MOST, false));
    }
    #[test] fn faces_go_as_runs_and_none_when_they_are_not_one_a_triangle() {
        assert_eq!(face_runs(&[0, 0, 0, 1, 2, 2], 6), vec![3, 1, 2]);
        assert_eq!(face_runs(&vec![7; 200], 200), vec![200 & 127 | 128, 1], "a long run is a varint");
        assert!(face_runs(&[0, 1], 3).is_empty() && face_runs(&[], 0).is_empty(), "thinned (fewer triangles): none");
    }
    #[test] fn a_part_of_ints_is_the_same_bytes_as_the_solid() {
        let t = [tri([0.001, 0.002, 0.003], [0.002, 0.002, -0.003], [0.001, -0.002, 0.003], 0xff0000), tri([0.5, 0.0, 1.0], [0.0; 3], [-0.25, 0.1, 0.0], 0x00ff01)];
        assert_eq!(part_mesh(&t), (2, of(&t).unwrap().d));
    }
    #[test] fn a_solid_codes_colours_by_xor_and_points_by_zigzag_deltas() {
        let t = [tri([0.001, 0.002, 0.003], [0.002, 0.002, 0.003], [0.001, -0.002, 0.003], 0xff0000), tri([0.0; 3], [0.0; 3], [0.0; 3], 0xff0001)];
        let s = of(&t).unwrap(); let n = read(&s.d);
        assert_eq!(s.n, 2); assert_eq!(n.len(), 20);
        assert_eq!(n[0], 0xff0000); assert_eq!(n[10], 1); // the colour, then the XOR with the one before
        assert_eq!((unzig(n[1]), unzig(n[2]), unzig(n[3])), (100, 300, 200)); // board x, y = glTF x, z; z up = glTF y; 10 um units
        assert_eq!((unzig(n[4]), unzig(n[5]), unzig(n[6])), (100, 0, 0));
        assert_eq!((unzig(n[7]), unzig(n[8]), unzig(n[9])), (-100, 0, -400));
        assert_eq!(s.b.map(|v| v as i32), [0, 0, -200, 200, 300, 200]);
    }
    #[test] fn no_triangles_is_no_solid_and_the_plot_has_the_empty_box() {
        assert_eq!(of(&[]), None);
        let p = plot(None); assert_eq!((p.b.x0, p.b.x1, p.thick, p.edge.y1), (2147483647, 2147483648, 0, 2147483648)); assert!(p.cs.is_empty());
    }
    #[test] fn thinning_drops_a_collapsed_triangle_and_keeps_heights() {
        // a box 1 m across: cells of about 0.4 mm
        let big = tri([0.0, 0.0, 0.0], [1.0, 0.25, 0.0], [0.0, 0.5, 1.0], 7);
        let speck = tri([0.5, 0.0, 0.5], [0.5001, 0.0, 0.5], [0.5, 0.0, 0.5001], 7);
        let out = thin(vec![big, speck]);
        assert_eq!(out.len(), 1); assert_eq!(out[0].a[1], 0.0); assert_eq!(out[0].b[1], 0.25); assert_eq!(out[0].c[1], 0.5);
        assert_ne!(out[0].b[0], 1.0); // moved across, to a cell's centre
    }
    #[test] fn a_coarser_grid_keeps_fewer_triangles() {
        // a fan of thin slivers around a 1 m disc: a coarse grid merges their outer corners
        let ts: Vec<Tri3> = (0..400).map(|i| { let (a, b) = (i as f32 * 0.0157, (i + 1) as f32 * 0.0157); tri([0.0, 0.0, 0.0], [a.cos(), 0.0, a.sin()], [b.cos(), 0.0, b.sin()], 1) }).collect();
        let (fine, coarse) = (thin_by(ts.clone(), 1000.0).len(), thin_by(ts, 15.0).len());
        assert!(fine > 300 && coarse < fine / 4, "{fine} {coarse}");
    }
    #[test] fn a_model_of_no_extent_is_not_thinned() { let t = vec![tri([1.0; 3], [1.0; 3], [1.0; 3], 0)]; assert_eq!(thin(t.clone()), t); }
    #[test] fn upright_and_metres() {
        let mut t = [tri([1.0, 2.0, 3.0], [0.0; 3], [0.0; 3], 0)];
        upright(&mut t); assert_eq!(t[0].a, [1.0, 3.0, -2.0]);
        metres(&mut t); assert_eq!(t[0].a, [1.0f32 * 0.001, 3.0f32 * 0.001, -2.0f32 * 0.001]);
    }
    #[test] fn the_size_limit_says_so() {
        assert!(fits(MOST) && !fits(MOST + 1));
        assert_eq!(big(MOST + 1), "This 3D model is too big to show here (13 MB; the hub reads models up to 12 MB)");
    }
}
