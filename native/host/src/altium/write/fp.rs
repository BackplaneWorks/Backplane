//! Footprints from package dimensions, the way IPC-7351B sizes lands (nominal density): a land runs from the lead's
//! toe plus a toe fillet out to its heel less a heel fillet in, and is as wide as the lead plus side fillets. Each comes
//! with a silkscreen outline (clear of the copper), an assembly outline (Mechanical 13), a courtyard (Mechanical 15) and an
//! extruded 3D body (Mechanical 13), as Altium's IPC footprint wizard lays them out, and IPC-7351 names.
//! Dimensions are mm; pin 1 is at the top left (IPC's zero orientation).
use super::pcblib::{Arc, Body, Footprint, Line, Pad, PadShape, TOP_OVERLAY};

const SILK: f64 = 0.15;
const SILK_GAP: f64 = 0.2; // silk to copper
const COURTYARD: f64 = 0.25; // courtyard excess (nominal)
const ASSY: u8 = 57 + 12; // Mechanical 13
const CYARD: u8 = 57 + 14; // Mechanical 15

fn r2(v: f64) -> f64 { (v * 100.0).round() / 100.0 }
fn rect_lines(layer: u8, x0: f64, y0: f64, x1: f64, y1: f64, w: f64) -> Vec<Line> {
    vec![Line { layer, a: (x0, y0), b: (x1, y0), w }, Line { layer, a: (x1, y0), b: (x1, y1), w }, Line { layer, a: (x1, y1), b: (x0, y1), w }, Line { layer, a: (x0, y1), b: (x0, y0), w }]
}
/// The pads' extent (x0, y0, x1, y1).
fn pad_box(ps: &[Pad]) -> (f64, f64, f64, f64) {
    ps.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, p| {
        let (hw, hh) = if (p.rot / 90.0).round() as i64 % 2 != 0 { (p.h / 2.0, p.w / 2.0) } else { (p.w / 2.0, p.h / 2.0) };
        (b.0.min(p.x - hw), b.1.min(p.y - hh), b.2.max(p.x + hw), b.3.max(p.y + hh))
    })
}
/// Adds the courtyard (around body and pads), the assembly outline and the 3D body, and sets the height.
fn finish(f: &mut Footprint, body: (f64, f64), height: f64) {
    let (bw, bl) = body;
    let pb = pad_box(&f.pads);
    let (x0, y0, x1, y1) = (pb.0.min(-bw / 2.0) - COURTYARD, pb.1.min(-bl / 2.0) - COURTYARD, pb.2.max(bw / 2.0) + COURTYARD, pb.3.max(bl / 2.0) + COURTYARD);
    let g = |v: f64| (v / 0.05).round() * 0.05;
    f.lines.extend(rect_lines(CYARD, g(x0), g(y0), g(x1), g(y1), 0.05));
    f.lines.extend(rect_lines(ASSY, -bw / 2.0, -bl / 2.0, bw / 2.0, bl / 2.0, 0.1));
    if height > 0.0 { f.bodies.push(Body { outline: vec![(-bw / 2.0, -bl / 2.0), (bw / 2.0, -bl / 2.0), (bw / 2.0, bl / 2.0), (-bw / 2.0, bl / 2.0)], z0: 0.0, z1: height, layer: ASSY }) }
    f.height = height;
}
/// Silk segments along y = `y` from x0 to x1, cut where they would come within SILK_GAP of a pad.
fn silk_h(f: &mut Footprint, y: f64, x0: f64, x1: f64) {
    let mut spans = vec![(x0, x1)];
    for p in &f.pads {
        let (hw, hh) = (p.w / 2.0 + SILK_GAP + SILK / 2.0, p.h / 2.0 + SILK_GAP + SILK / 2.0);
        if (y - p.y).abs() >= hh { continue }
        spans = spans.into_iter().flat_map(|(a, b)| { let (c, d) = (p.x - hw, p.x + hw); if d <= a || c >= b { vec![(a, b)] } else { [(a, c.min(b)), (d.max(a), b)].into_iter().filter(|(u, v)| v - u > 0.1).collect() } }).collect();
    }
    for (a, b) in spans { f.lines.push(Line { layer: TOP_OVERLAY, a: (a, y), b: (b, y), w: SILK }) }
}
fn silk_v(f: &mut Footprint, x: f64, y0: f64, y1: f64) {
    let mut spans = vec![(y0, y1)];
    for p in &f.pads {
        let (hw, hh) = (p.w / 2.0 + SILK_GAP + SILK / 2.0, p.h / 2.0 + SILK_GAP + SILK / 2.0);
        if (x - p.x).abs() >= hw { continue }
        spans = spans.into_iter().flat_map(|(a, b)| { let (c, d) = (p.y - hh, p.y + hh); if d <= a || c >= b { vec![(a, b)] } else { [(a, c.min(b)), (d.max(a), b)].into_iter().filter(|(u, v)| v - u > 0.1).collect() } }).collect();
    }
    for (a, b) in spans { f.lines.push(Line { layer: TOP_OVERLAY, a: (x, a), b: (x, b), w: SILK }) }
}
fn dot(f: &mut Footprint, x: f64, y: f64) { f.arcs.push(Arc { layer: TOP_OVERLAY, c: (x, y), r: 0.1, a0: 0.0, a1: 360.0, w: 0.2 }) }

/// A land for a lead whose toe is at `toe` and heel at `heel` from the centre line (toe > heel): (centre, length).
fn land(toe: f64, heel: f64, jt: f64, jh: f64) -> (f64, f64) { let (z, g) = (toe + jt, heel - jh); ((z + g) / 2.0, z - g) }

/// Two-terminal chip (resistor, capacitor, inductor): body length `l`, width `w`, terminal length `t`, height `h`.
pub fn chip(prefix: &str, l: f64, w: f64, t: f64, h: f64) -> Footprint {
    let small = l < 1.5;
    let (jt, jh, js) = if small { (0.2, 0.0, 0.0) } else { (0.35, 0.0, 0.0) };
    let (c, len) = land(l / 2.0, l / 2.0 - t, jt, jh);
    let pw = r2(w + 2.0 * js);
    let mut f = Footprint { name: format!("{prefix}C{:02}{:02}X{:.0}N", (l * 10.0).round() as i64, (w * 10.0).round() as i64, h * 100.0), description: format!("Chip, {l} x {w} mm body"), ..Default::default() };
    for (n, s) in [("1", -1.0), ("2", 1.0)] { f.pads.push(Pad::smd(n, r2(s * c), 0.0, r2(len), pw, PadShape::RoundRect(25))) }
    // silk above and below the body, between the pads
    let y = w / 2.0 + SILK_GAP + SILK;
    if !small { silk_h(&mut f, y, -l / 2.0, l / 2.0); silk_h(&mut f, -y, -l / 2.0, l / 2.0); }
    finish(&mut f, (l, w), h);
    f
}
/// Chip sizes by their (inch) code: body length, width, terminal length.
pub fn chip_size(code: &str) -> Option<(f64, f64, f64)> {
    Some(match code.trim().trim_start_matches(|c: char| c.is_ascii_alphabetic()) {
        "01005" => (0.4, 0.2, 0.1), "0201" => (0.6, 0.3, 0.15), "0402" => (1.0, 0.5, 0.25), "0603" => (1.6, 0.8, 0.35), "0805" => (2.0, 1.25, 0.4),
        "1206" => (3.2, 1.6, 0.5), "1210" => (3.2, 2.5, 0.5), "1812" => (4.5, 3.2, 0.5), "2010" => (5.0, 2.5, 0.6), "2512" => (6.3, 3.2, 0.6),
        _ => return None,
    })
}

/// Gull-wing leads on two sides (SOIC, SOP, TSSOP, MSOP, SOT-23): `left`/`right` pin counts (pins numbered down the left,
/// then up the right), `pitch`, lead span tip to tip `span`, foot length `foot`, lead width `lead`, body (x, y), height.
pub fn gullwing(name: &str, left: usize, right: usize, pitch: f64, span: f64, foot: f64, lead: f64, body: (f64, f64), h: f64) -> Footprint {
    let fine = pitch <= 0.625;
    let (jt, jh, js) = if fine { (0.35, 0.35, -0.02) } else { (0.35, 0.35, 0.03) };
    let (c, len) = land(span / 2.0, span / 2.0 - foot, jt, jh);
    let pw = r2((lead + 2.0 * js).min(pitch - 0.2));
    let mut f = Footprint { name: name.to_string(), description: format!("{} pins, {pitch} mm pitch, {span} mm span", left + right), ..Default::default() };
    let row = |i: usize, n: usize| ((n as f64 - 1.0) / 2.0 - i as f64) * pitch;
    // the right side counts up from the bottom; a lone pin sits mid-way, and one fewer than an odd left side skips the middle
    let right_y: Vec<f64> = if right == 1 { vec![0.0] }
        else if right + 1 == left && left % 2 == 1 { (0..left).rev().filter(|i| *i != left / 2).map(|i| row(i, left)).collect() }
        else { (0..right).map(|i| -row(i, right)).collect() };
    let mut num = 1;
    // SOT-23-3: the two left pins are the outer two of three rows
    let left_y: Vec<f64> = if left == 2 && right == 1 { vec![pitch, -pitch] } else { (0..left).map(|i| row(i, left)).collect() };
    for y in left_y { f.pads.push(Pad::smd(&num.to_string(), r2(-c), r2(y), r2(len), pw, PadShape::RoundRect(25))); num += 1 }
    for y in right_y { f.pads.push(Pad::smd(&num.to_string(), r2(c), r2(y), r2(len), pw, PadShape::RoundRect(25))); num += 1 }
    // silk: the body's top and bottom edges, clear of the lands, and a pin-1 dot beyond the first land
    let (bx, by) = body;
    let y = by / 2.0 + SILK / 2.0;
    silk_h(&mut f, y, -bx / 2.0, bx / 2.0); silk_h(&mut f, -y, -bx / 2.0, bx / 2.0);
    let p1 = f.pads[0].clone();
    dot(&mut f, r2(p1.x - p1.w / 2.0 - 0.3), r2(p1.y + p1.h / 2.0 + 0.3).max(by / 2.0 + 0.3));
    finish(&mut f, body, h);
    f
}
/// SOIC/SOP style name: SOIC127P600X175-8N.
pub fn gullwing_name(kind: &str, pitch: f64, span: f64, h: f64, n: usize) -> String { format!("{kind}{:.0}P{:.0}X{:.0}-{n}N", pitch * 100.0, span * 100.0, h * 100.0) }

/// Leadless parts with pads on four sides (QFN, DFN when `ny` is 0): `nx` pins on each left/right side, `ny` on top/bottom,
/// numbered counter-clockwise from the top left; body (x, y), terminal length `term`, width `lead`, an optional exposed pad.
pub fn qfn(name: &str, nx: usize, ny: usize, pitch: f64, body: (f64, f64), term: f64, lead: f64, ep: Option<(f64, f64)>, h: f64) -> Footprint {
    let (jt, jh, js) = (0.3, 0.0, -0.04);
    let mut f = Footprint { name: name.to_string(), description: format!("{} pins, {pitch} mm pitch, {} x {} mm body", 2 * (nx + ny), body.0, body.1), ..Default::default() };
    let (cx, lx) = land(body.0 / 2.0, body.0 / 2.0 - term, jt, jh);
    let (cy, ly) = land(body.1 / 2.0, body.1 / 2.0 - term, jt, jh);
    let pw = r2((lead + 2.0 * js).max(0.15));
    let off = |i: usize, n: usize| ((n as f64 - 1.0) / 2.0 - i as f64) * pitch;
    let mut num = 1;
    let mut add = |f: &mut Footprint, x: f64, y: f64, w: f64, hh: f64| { f.pads.push(Pad::smd(&num.to_string(), r2(x), r2(y), r2(w), r2(hh), PadShape::RoundRect(25))); num += 1 };
    for i in 0..nx { add(&mut f, -cx, off(i, nx), lx, pw) }
    for i in 0..ny { add(&mut f, -off(i, ny), -cy, pw, ly) }
    for i in 0..nx { add(&mut f, cx, -off(i, nx), lx, pw) }
    for i in 0..ny { add(&mut f, off(i, ny), cy, pw, ly) }
    if let Some((ew, eh)) = ep { f.pads.push(Pad::smd(&num.to_string(), 0.0, 0.0, ew, eh, PadShape::Rect)) }
    // silk: corner marks outside the pad rows, and a pin-1 dot
    let (bx, by) = (body.0 / 2.0 + SILK, body.1 / 2.0 + SILK);
    let reach_x = if ny > 0 { off(0, ny).abs() + pw / 2.0 + SILK_GAP + SILK } else { bx - 0.4 };
    let reach_y = if nx > 0 { off(0, nx).abs() + pw / 2.0 + SILK_GAP + SILK } else { by - 0.4 };
    for (sx, sy) in [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        if bx > reach_x { f.lines.push(Line { layer: TOP_OVERLAY, a: (sx * bx, sy * by), b: (sx * reach_x, sy * by), w: SILK }) }
        if by > reach_y { f.lines.push(Line { layer: TOP_OVERLAY, a: (sx * bx, sy * by), b: (sx * bx, sy * reach_y), w: SILK }) }
    }
    dot(&mut f, r2(-bx - 0.3), r2(off(0, nx.max(1)) + 0.3).max(by * 0.5));
    finish(&mut f, body, h);
    f
}

/// Through-hole pin header: `rows` x `cols`, `pitch`, numbered along each row (1, 2 across a two-row header, as most are);
/// pin 1 square.
pub fn header(name: &str, rows: usize, cols: usize, pitch: f64, drill: f64, pad: f64, h: f64) -> Footprint {
    let mut f = Footprint { name: name.to_string(), description: format!("{rows} x {cols} header, {pitch} mm pitch"), ..Default::default() };
    // rows go down (y), columns across (x); pin numbers run across a row first
    let mut num = 1;
    for r in 0..rows { for c in 0..cols {
        let shape = if num == 1 { PadShape::Rect } else { PadShape::Round };
        f.pads.push(Pad::thru(&num.to_string(), r2((c as f64 - (cols as f64 - 1.0) / 2.0) * pitch), r2(((rows as f64 - 1.0) / 2.0 - r as f64) * pitch), pad, drill, shape));
        num += 1;
    } }
    let (bw, bh) = (cols as f64 * pitch, rows as f64 * pitch);
    f.lines.extend(rect_lines(TOP_OVERLAY, -bw / 2.0, -bh / 2.0, bw / 2.0, bh / 2.0, SILK));
    finish(&mut f, (bw, bh), h);
    f
}

/// Lands placed by hand (the spec's `pads`), with a silk box around the body when it is given.
pub fn custom(name: &str, pads: Vec<Pad>, body: Option<(f64, f64)>, h: f64) -> Footprint {
    let mut f = Footprint { name: name.to_string(), pads, ..Default::default() };
    match body {
        Some((bw, bl)) => {
            let (x, y) = (bw / 2.0 + SILK / 2.0, bl / 2.0 + SILK / 2.0);
            silk_h(&mut f, y, -x, x); silk_h(&mut f, -y, -x, x); silk_v(&mut f, -x, -y, y); silk_v(&mut f, x, -y, y);
            finish(&mut f, (bw, bl), h);
        }
        None => { let b = pad_box(&f.pads); finish(&mut f, ((b.2.abs().max(b.0.abs()) * 2.0).max(0.1), (b.3.abs().max(b.1.abs()) * 2.0).max(0.1)), h) }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn chip_0603() {
        let (l, w, t) = chip_size("0603").unwrap();
        let f = chip("RES", l, w, t, 0.55);
        assert_eq!(f.name, "RESC1608X55N");
        let p = &f.pads;
        assert_eq!(p.len(), 2);
        // lands: toe at 0.8 + 0.35, heel at 0.45: centre 0.8, length 0.7; width 0.8
        assert!((p[1].x - 0.8).abs() < 1e-9 && (p[1].w - 0.7).abs() < 1e-9 && (p[1].h - 0.8).abs() < 1e-9, "{p:?}");
        assert!(f.lines.iter().any(|l| l.layer == CYARD) && f.bodies.len() == 1);
    }
    #[test] fn soic8_and_sot23() {
        let f = gullwing(&gullwing_name("SOIC", 1.27, 6.0, 1.75, 8), 4, 4, 1.27, 6.0, 0.835, 0.42, (3.9, 4.9), 1.75);
        assert_eq!(f.name, "SOIC127P600X175-8N");
        assert_eq!(f.pads[0].name, "1"); assert!(f.pads[0].x < 0.0 && f.pads[0].y > 0.0);
        assert!(f.pads[4].x > 0.0 && f.pads[4].y < 0.0, "pin 5 at the bottom right");
        // silk never touches copper
        for l in f.lines.iter().filter(|l| l.layer == TOP_OVERLAY) { for p in &f.pads { assert!((l.a.1 - p.y).abs() > p.h / 2.0 || l.a.0.max(l.b.0) < p.x - p.w / 2.0 || l.a.0.min(l.b.0) > p.x + p.w / 2.0) } }
        let s = gullwing("SOT23-3", 2, 1, 0.95, 2.4, 0.4, 0.4, (1.3, 2.9), 1.1);
        assert_eq!(s.pads.iter().map(|p| (p.name.as_str(), p.y)).collect::<Vec<_>>(), vec![("1", 0.95), ("2", -0.95), ("3", 0.0)]);
        let s5 = gullwing("SOT23-5", 3, 2, 0.95, 2.8, 0.45, 0.4, (1.6, 2.9), 1.45);
        assert_eq!(s5.pads.iter().map(|p| (p.name.as_str(), p.x > 0.0, p.y)).collect::<Vec<_>>(), vec![("1", false, 0.95), ("2", false, 0.0), ("3", false, -0.95), ("4", true, -0.95), ("5", true, 0.95)]);
    }
    #[test] fn qfn_and_header() {
        let q = qfn("QFN", 4, 4, 0.5, (3.0, 3.0), 0.4, 0.25, Some((1.7, 1.7)), 0.9);
        assert_eq!(q.pads.len(), 17);
        assert!(q.pads[4].y < 0.0 && q.pads[8].x > 0.0 && q.pads[12].y > 0.0, "counter-clockwise");
        let h = header("HDR", 2, 3, 2.54, 1.0, 1.7, 8.5);
        assert_eq!(h.pads.len(), 6); assert_eq!(h.pads[0].shape, PadShape::Rect); assert!(h.pads.iter().all(|p| p.hole == 1.0));
    }
}
