//! Pure `.kicad_pcb` geometry & stackup extractor for the Plan floorplanner canvas.
//! Parses Edge.Cuts outline (lines/arcs/rects/polys/circles, chained into continuous contours),
//! component courtyard/fab bounding boxes + refdes labels, copper layer stackup metadata,
//! and pad-to-pad ratsnest guides per net/corridor, cached by `(path, mtime, len)`.
use crate::files::kicad::{board, sexpr::{self, Sx}};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex, OnceLock},
    time::UNIX_EPOCH,
};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PartBox {
    /// Component reference designator (e.g. "U1", "J1").
    pub r: String,
    /// Board side: "F" or "B".
    pub s: &'static str,
    /// World bounding box [x0, y0, x1, y1] in mm.
    pub b: [f64; 4],
    /// True if on bottom side or extending > 1.0 mm past Edge.Cuts (drawn dashed).
    pub o: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CopperLayer {
    pub layer: String,
    pub index: usize,
    pub role: String,
    pub alias: String,
    pub title: String,
    pub subtitle: String,
    #[serde(skip)]
    pub above: Option<(String, f64, bool)>,
    #[serde(skip)]
    pub below: Option<(String, f64, bool)>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BoardGeom {
    pub bbox: [f64; 4],
    pub outline: Vec<Vec<[f64; 2]>>,
    pub parts: Vec<PartBox>,
    pub copper_layers: Vec<CopperLayer>,
}

#[derive(Clone, Debug)]
pub struct CachedBoard {
    pub geom: BoardGeom,
    /// Net name -> list of (refdes, x_mm, y_mm) pad coordinates.
    pub pad_nets: HashMap<String, Vec<(String, f64, f64)>>,
}

#[derive(Clone)]
struct CacheEntry {
    mtime_ns: u128,
    len: u64,
    board: Arc<CachedBoard>,
}

static CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();
fn cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    CACHE.get_or_init(Default::default)
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn pt2(x: f64, y: f64) -> [f64; 2] {
    [r2(x), r2(y)]
}

fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    (dx * dx + dy * dy).sqrt()
}

fn is_closed(pts: &[[f64; 2]]) -> bool {
    pts.len() >= 3 && dist2(pts[0], pts[pts.len() - 1]) < 0.05
}

fn chain_segments(mut segs: Vec<Vec<[f64; 2]>>) -> Vec<Vec<[f64; 2]>> {
    let mut closed = Vec::new();
    let mut open: Vec<Vec<[f64; 2]>> = Vec::new();
    for s in segs.drain(..) {
        if s.len() < 2 {
            continue;
        }
        if is_closed(&s) {
            closed.push(s);
        } else {
            open.push(s);
        }
    }
    let mut out = closed;
    while !open.is_empty() {
        let mut cur = open.remove(0);
        let mut grew = true;
        while grew && !is_closed(&cur) {
            grew = false;
            let (head, tail) = (cur[0], cur[cur.len() - 1]);
            let mut best: Option<(usize, bool, bool, f64)> = None;
            for (i, cand) in open.iter().enumerate() {
                let (ch, ct) = (cand[0], cand[cand.len() - 1]);
                for (append_end, rev_cand, d) in [
                    (true, false, dist2(tail, ch)),
                    (true, true, dist2(tail, ct)),
                    (false, false, dist2(head, ct)),
                    (false, true, dist2(head, ch)),
                ] {
                    if d <= 0.05 && best.map_or(true, |b| d < b.3) {
                        best = Some((i, append_end, rev_cand, d));
                    }
                }
            }
            if let Some((idx, append_end, rev_cand, _)) = best {
                let mut cand = open.remove(idx);
                if rev_cand {
                    cand.reverse();
                }
                if append_end {
                    cur.extend_from_slice(&cand[1..]);
                } else {
                    cand.pop();
                    cand.extend(cur);
                    cur = cand;
                }
                grew = true;
            }
        }
        out.push(cur);
    }
    out
}

fn sample_arc(a: board::Pt, b: board::Pt, c: board::Pt) -> Vec<[f64; 2]> {
    let pts = board::arc_points(a, b, c);
    if pts.len() <= 17 {
        return pts.into_iter().map(|p| pt2(p.x as f64, p.y as f64)).collect();
    }
    let mut out = Vec::with_capacity(17);
    let last = pts.len() - 1;
    for k in 0..=16 {
        let idx = (k * last) / 16;
        out.push(pt2(pts[idx].x as f64, pts[idx].y as f64));
    }
    out
}

fn sample_circle(c: board::Pt, r: f64) -> Vec<[f64; 2]> {
    (0..=32)
        .map(|i| {
            let a = std::f64::consts::TAU * (i as f64) / 32.0;
            pt2(c.x as f64 + r * a.cos(), c.y as f64 + r * a.sin())
        })
        .collect()
}

fn edge_graphic_pts(x: &Sx, pl: &board::Place) -> Option<Vec<[f64; 2]>> {
    match x.name() {
        "gr_line" | "fp_line" => {
            let a = pl.at(board::at(x, "start"));
            let b = pl.at(board::at(x, "end"));
            Some(vec![pt2(a.x as f64, a.y as f64), pt2(b.x as f64, b.y as f64)])
        }
        "gr_arc" | "fp_arc" => {
            let a = pl.at(board::at(x, "start"));
            let b = pl.at(board::at(x, "mid"));
            let c = pl.at(board::at(x, "end"));
            Some(sample_arc(a, b, c))
        }
        "gr_rect" | "fp_rect" => {
            let a = board::at(x, "start");
            let b = board::at(x, "end");
            let corners = [
                pl.at(a),
                pl.at(board::Pt { x: b.x, y: a.y }),
                pl.at(b),
                pl.at(board::Pt { x: a.x, y: b.y }),
                pl.at(a),
            ];
            Some(corners.into_iter().map(|p| pt2(p.x as f64, p.y as f64)).collect())
        }
        "gr_poly" | "fp_poly" => {
            let mut ps: Vec<[f64; 2]> = x
                .get("pts")
                .args()
                .iter()
                .filter(|c| c.name() == "xy")
                .map(|c| {
                    let p = pl.at(board::pt(c));
                    pt2(p.x as f64, p.y as f64)
                })
                .collect();
            if ps.len() >= 3 && !is_closed(&ps) {
                ps.push(ps[0]);
            }
            (ps.len() >= 2).then_some(ps)
        }
        "gr_circle" | "fp_circle" => {
            let center = board::at(x, "center");
            let end = board::at(x, "end");
            let r = ((end.x - center.x).powi(2) + (end.y - center.y).powi(2)).sqrt() as f64;
            if r > 0.01 {
                Some(sample_circle(pl.at(center), r))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn expand_bbox(bb: &mut [f64; 4], x: f64, y: f64) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    bb[0] = bb[0].min(x);
    bb[1] = bb[1].min(y);
    bb[2] = bb[2].max(x);
    bb[3] = bb[3].max(y);
}

fn valid_bbox(bb: [f64; 4]) -> bool {
    bb[2].is_finite() && bb[0].is_finite() && bb[2] > bb[0] && bb[3] > bb[1]
}

fn layer_rank(name: &str) -> usize {
    if name == "F.Cu" {
        0
    } else if name == "B.Cu" {
        10_000
    } else if let Some(inner) = name.strip_prefix("In").and_then(|s| s.strip_suffix(".Cu")) {
        inner.parse::<usize>().unwrap_or(500)
    } else {
        5_000
    }
}

fn fmt_um(h_mm: f64) -> String {
    let um = (h_mm * 1000.0).round() as i64;
    format!("{um} µm")
}

/// Build the concise stackup subtitle for a copper layer given an optional plan role override.
pub fn subtitle_for_layer(c: &CopperLayer, role_override: &str) -> String {
    let role = if !role_override.is_empty() {
        role_override
    } else {
        c.role.as_str()
    };
    let is_power = matches!(role, "power" | "plane");
    if is_power {
        let nearest = match (&c.above, &c.below) {
            (Some(a), Some(b)) => Some(if a.1 <= b.1 { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        if let Some((adj, h, _)) = nearest {
            return format!("Faces {adj} across {}", fmt_um(*h));
        }
        return String::new();
    }
    match (&c.above, &c.below) {
        (Some((an, ah, ac)), Some((bn, bh, bc))) => {
            let part = |h: f64, core: bool| {
                if core {
                    format!("{} core", fmt_um(h))
                } else {
                    fmt_um(h)
                }
            };
            format!("Stripline between {an} ({}) and {bn} ({})", part(*ah, *ac), part(*bh, *bc))
        }
        (Some((an, ah, _)), None) => format!("Microstrip over {an} ({})", fmt_um(*ah)),
        (None, Some((bn, bh, _))) => format!("Microstrip over {bn} ({})", fmt_um(*bh)),
        (None, None) => String::new(),
    }
}

fn parse_copper_layers(fs: &[&[u8]]) -> Vec<CopperLayer> {
    struct RawCu {
        layer: String,
        role: String,
        alias: String,
    }
    let mut raw: Vec<RawCu> = Vec::new();
    for f in fs.iter().filter(|f| f.starts_with(b"(layers")) {
        let sx = sexpr::parse(f);
        for c in sx.args() {
            let name = c.arg(0);
            if !name.ends_with(".Cu") || name == "*.Cu" {
                continue;
            }
            let typ = c.arg(1);
            let alias = c.arg(2).trim().to_string();
            let role = if typ == "power"
                || alias.to_ascii_uppercase().starts_with("PWR")
                || alias.to_ascii_uppercase().starts_with("GND")
            {
                "power".to_string()
            } else {
                "signal".to_string()
            };
            raw.push(RawCu {
                layer: name.to_string(),
                role,
                alias,
            });
        }
    }
    raw.sort_by_key(|r| layer_rank(&r.layer));

    struct Ply {
        name: String,
        is_copper: bool,
        is_core: bool,
        h_mm: f64,
    }
    let mut plies: Vec<Ply> = Vec::new();
    for f in fs.iter().filter(|f| f.starts_with(b"(setup")) {
        let sx = sexpr::parse(f);
        let stack = sx.get("stackup");
        for l in stack.args().iter().filter(|x| x.name() == "layer") {
            let lname = l.arg(0).to_string();
            let typ = l.attr("type").to_ascii_lowercase();
            let h_mm = l.get("thickness").f(0) as f64;
            let is_copper = typ == "copper" || lname.ends_with(".Cu");
            let is_dielectric = typ.contains("core") || typ.contains("prepreg") || typ.contains("dielectric") || lname.starts_with("dielectric");
            if (is_copper || is_dielectric) && (is_copper || h_mm > 0.0) {
                plies.push(Ply {
                    name: lname,
                    is_copper,
                    is_core: typ.contains("core"),
                    h_mm,
                });
            }
        }
    }

    let ref_name = |r: &RawCu| -> String {
        if !r.alias.is_empty() && r.alias != "top_cu" && r.alias != "bottom_cu" {
            r.alias.clone()
        } else {
            r.layer.clone()
        }
    };
    let cu_ply_idx: HashMap<String, usize> = plies
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_copper)
        .map(|(i, p)| (p.name.clone(), i))
        .collect();

    let total = raw.len();
    let (mut sig_n, mut pwr_n) = (0usize, 0usize);
    let mut out = Vec::with_capacity(total);
    for (i, r) in raw.iter().enumerate() {
        let tag = if !r.alias.is_empty() && r.alias != "top_cu" && r.alias != "bottom_cu" && r.alias != r.layer {
            r.alias.clone()
        } else if r.role == "power" {
            pwr_n += 1;
            format!("PWR{pwr_n}")
        } else if total >= 4 {
            sig_n += 1;
            format!("SIG{sig_n}")
        } else {
            r.layer.clone()
        };
        let index = i + 1;
        let title = format!("L{index} · {tag}");
        let mut above: Option<(String, f64, bool)> = None;
        let mut below: Option<(String, f64, bool)> = None;
        if let Some(&pi) = cu_ply_idx.get(&r.layer) {
            if i > 0 {
                let prev = &raw[i - 1];
                if let Some(&ai) = cu_ply_idx.get(&prev.layer) {
                    if ai < pi {
                        let h: f64 = plies[ai + 1..pi].iter().filter(|p| !p.is_copper).map(|p| p.h_mm).sum();
                        let core = plies[ai + 1..pi].iter().any(|p| !p.is_copper && p.is_core);
                        if h > 0.0 {
                            above = Some((ref_name(prev), h, core));
                        }
                    }
                }
            }
            if i + 1 < total {
                let next = &raw[i + 1];
                if let Some(&bi) = cu_ply_idx.get(&next.layer) {
                    if pi < bi {
                        let h: f64 = plies[pi + 1..bi].iter().filter(|p| !p.is_copper).map(|p| p.h_mm).sum();
                        let core = plies[pi + 1..bi].iter().any(|p| !p.is_copper && p.is_core);
                        if h > 0.0 {
                            below = Some((ref_name(next), h, core));
                        }
                    }
                }
            }
        }
        let mut cl = CopperLayer {
            layer: r.layer.clone(),
            index,
            role: r.role.clone(),
            alias: r.alias.clone(),
            title,
            subtitle: String::new(),
            above,
            below,
        };
        cl.subtitle = subtitle_for_layer(&cl, "");
        out.push(cl);
    }
    out
}

fn footprint_ref<'a>(x: &'a Sx<'a>) -> &'a str {
    let r = board::prop(x, "Reference");
    if !r.is_empty() {
        return r;
    }
    x.args()
        .iter()
        .find(|c| c.name() == "fp_text" && c.arg(0) == "reference")
        .map(|c| c.arg(1))
        .unwrap_or("")
}

fn part_priority(r: &str) -> u8 {
    let up = r.to_ascii_uppercase();
    if up.starts_with('U') || up.starts_with("IC") || up.starts_with("MCU") || up.starts_with("FPGA") {
        0
    } else if up.starts_with('J') || up.starts_with("CON") || up.starts_with('P') {
        1
    } else if up.starts_with('Q') || up.starts_with('Y') || up.starts_with('M') || up.starts_with("SW") {
        2
    } else {
        3
    }
}

/// Parse a `.kicad_pcb` file's bytes into cached board floorplan geometry + pad nets.
pub fn parse_board(bytes: &[u8]) -> Option<CachedBoard> {
    let fs = sexpr::forms(bytes)?;
    let nets = board::nets(&fs);
    let copper_layers = parse_copper_layers(&fs);

    let mut raw_outline: Vec<Vec<[f64; 2]>> = Vec::new();
    let home = board::Place::new(0.0, 0.0, 0.0);

    struct RawPart {
        r: String,
        s: &'static str,
        b: [f64; 4],
    }
    let mut raw_parts: Vec<RawPart> = Vec::new();
    let mut pad_nets: HashMap<String, Vec<(String, f64, f64)>> = HashMap::new();

    for f in &fs {
        let sx = sexpr::parse(f);
        match sx.name() {
            "gr_line" | "gr_arc" | "gr_rect" | "gr_poly" | "gr_circle" => {
                if sx.attr("layer") == "Edge.Cuts" {
                    if let Some(pts) = edge_graphic_pts(&sx, &home) {
                        raw_outline.push(pts);
                    }
                }
            }
            "footprint" => {
                let pl = board::fp_place(&sx);
                let refdes = footprint_ref(&sx);
                let side = if sx.attr("layer").starts_with("B.") { "B" } else { "F" };
                let mut crtyd = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
                let mut fab = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
                let mut pads = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];

                for c in sx.args() {
                    let cname = c.name();
                    if cname == "pad" {
                        let p = pl.at(board::at(c, "at"));
                        let sz = c.get("size");
                        let hw = (sz.f(0).abs().max(0.2) as f64) * 0.5;
                        let hh = (sz.f(1).abs().max(0.2) as f64) * 0.5;
                        let rad = hw.max(hh);
                        expand_bbox(&mut pads, p.x as f64 - rad, p.y as f64 - rad);
                        expand_bbox(&mut pads, p.x as f64 + rad, p.y as f64 + rad);

                        if !refdes.is_empty() && !refdes.starts_with('#') {
                            let n = c.get("net");
                            let net_name = if !n.arg(1).is_empty() {
                                n.arg(1).to_string()
                            } else {
                                nets.get(n.arg(0)).cloned().unwrap_or_default()
                            };
                            if !net_name.is_empty() {
                                pad_nets
                                    .entry(net_name)
                                    .or_default()
                                    .push((refdes.to_string(), r2(p.x as f64), r2(p.y as f64)));
                            }
                        }
                        continue;
                    }
                    let layer = c.attr("layer");
                    if layer == "Edge.Cuts" {
                        if let Some(pts) = edge_graphic_pts(c, &pl) {
                            raw_outline.push(pts);
                        }
                    } else if layer == "F.CrtYd" || layer == "B.CrtYd" {
                        if let Some(pts) = edge_graphic_pts(c, &pl) {
                            for p in pts {
                                expand_bbox(&mut crtyd, p[0], p[1]);
                            }
                        }
                    } else if layer == "F.Fab" || layer == "B.Fab" {
                        if let Some(pts) = edge_graphic_pts(c, &pl) {
                            for p in pts {
                                expand_bbox(&mut fab, p[0], p[1]);
                            }
                        }
                    }
                }

                if !refdes.is_empty() && !refdes.starts_with('#') {
                    let chosen = if valid_bbox(crtyd) {
                        crtyd
                    } else if valid_bbox(fab) {
                        fab
                    } else {
                        pads
                    };
                    if valid_bbox(chosen) {
                        raw_parts.push(RawPart {
                            r: refdes.to_string(),
                            s: side,
                            b: [r2(chosen[0]), r2(chosen[1]), r2(chosen[2]), r2(chosen[3])],
                        });
                    }
                }
            }
            _ => {}
        }
    }

    let outline = chain_segments(raw_outline);
    let mut edge_bb = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for poly in &outline {
        for p in poly {
            expand_bbox(&mut edge_bb, p[0], p[1]);
        }
    }
    let has_edge = valid_bbox(edge_bb);
    let mut bbox = edge_bb;
    if !has_edge {
        for p in &raw_parts {
            expand_bbox(&mut bbox, p.b[0], p.b[1]);
            expand_bbox(&mut bbox, p.b[2], p.b[3]);
        }
    }
    if !valid_bbox(bbox) {
        bbox = [0.0, 0.0, 100.0, 80.0];
    } else {
        bbox = [r2(bbox[0]), r2(bbox[1]), r2(bbox[2]), r2(bbox[3])];
    }

    let mut parts: Vec<PartBox> = raw_parts
        .into_iter()
        .map(|p| {
            let overhang = p.s == "B"
                || (has_edge
                    && (p.b[0] < edge_bb[0] - 1.0
                        || p.b[1] < edge_bb[1] - 1.0
                        || p.b[2] > edge_bb[2] + 1.0
                        || p.b[3] > edge_bb[3] + 1.0));
            PartBox {
                r: p.r,
                s: p.s,
                b: p.b,
                o: overhang,
            }
        })
        .collect();

    parts.sort_by(|a, b| {
        let pa = part_priority(&a.r);
        let pb = part_priority(&b.r);
        let aa = (a.b[2] - a.b[0]) * (a.b[3] - a.b[1]);
        let ab = (b.b[2] - b.b[0]) * (b.b[3] - b.b[1]);
        pa.cmp(&pb)
            .then_with(|| ab.partial_cmp(&aa).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.r.cmp(&b.r))
    });

    Some(CachedBoard {
        geom: BoardGeom {
            bbox,
            outline,
            parts,
            copper_layers,
        },
        pad_nets,
    })
}

/// Read and cache board geometry for `pcb_path` by `(mtime, len)`.
pub fn load(pcb_path: &str) -> Option<Arc<CachedBoard>> {
    if pcb_path.is_empty() {
        return None;
    }
    let meta = std::fs::metadata(Path::new(pcb_path)).ok()?;
    let mtime_ns = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let len = meta.len();

    {
        let map = cache().lock().unwrap();
        if let Some(hit) = map.get(pcb_path).filter(|e| e.mtime_ns == mtime_ns && e.len == len) {
            return Some(hit.board.clone());
        }
    }

    let bytes = std::fs::read(pcb_path).ok()?;
    let parsed = Arc::new(parse_board(&bytes)?);
    let mut map = cache().lock().unwrap();
    if map.len() >= 32 && !map.contains_key(pcb_path) {
        map.clear();
    }
    map.insert(
        pcb_path.to_string(),
        CacheEntry {
            mtime_ns,
            len,
            board: parsed.clone(),
        },
    );
    Some(parsed)
}

/// Select up to `max_parts` parts, ensuring any component referenced in the plan (`from`/`to`) stays included.
pub fn parts_for_plan(geom: &BoardGeom, plan: &Value, max_parts: usize) -> Vec<PartBox> {
    if geom.parts.len() <= max_parts {
        return geom.parts.clone();
    }
    let mut pinned: HashSet<&str> = HashSet::new();
    for c in plan["corridors"].as_array().into_iter().flatten() {
        for key in ["from", "to"] {
            for r in c[key].as_array().into_iter().flatten().filter_map(Value::as_str) {
                pinned.insert(r);
            }
        }
    }
    let mut out = Vec::with_capacity(max_parts);
    for p in &geom.parts {
        if pinned.contains(p.r.as_str()) {
            out.push(p.clone());
            if out.len() >= max_parts {
                return out;
            }
        }
    }
    for p in &geom.parts {
        if !pinned.contains(p.r.as_str()) {
            out.push(p.clone());
            if out.len() >= max_parts {
                break;
            }
        }
    }
    out
}

/// Compute faint ratsnest guide segments `[x1, y1, x2, y2]` per corridor id.
pub fn guides_for_plan(board: &CachedBoard, plan: &Value) -> HashMap<String, Vec<[f64; 4]>> {
    let mut out: HashMap<String, Vec<[f64; 4]>> = HashMap::new();
    let groups = plan["groups"].as_array().cloned().unwrap_or_default();
    let mut total_segs = 0usize;
    const MAX_TOTAL_GUIDES: usize = 180;
    const MAX_CORRIDOR_GUIDES: usize = 18;

    let part_center: HashMap<&str, [f64; 2]> = board
        .geom
        .parts
        .iter()
        .map(|p| (p.r.as_str(), [r2((p.b[0] + p.b[2]) * 0.5), r2((p.b[1] + p.b[3]) * 0.5)]))
        .collect();

    for c in plan["corridors"].as_array().into_iter().flatten() {
        if total_segs >= MAX_TOTAL_GUIDES {
            break;
        }
        let Some(cid) = c["id"].as_str() else { continue };
        let gid = c["group"].as_str().unwrap_or("");
        let nets: Vec<&str> = groups
            .iter()
            .find(|g| g["id"].as_str() == Some(gid))
            .and_then(|g| g["nets"].as_array())
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let from_refs: HashSet<&str> = c["from"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let to_refs: HashSet<&str> = c["to"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();

        let mut segs: Vec<[f64; 4]> = Vec::new();
        for net in &nets {
            if segs.len() >= MAX_CORRIDOR_GUIDES || total_segs + segs.len() >= MAX_TOTAL_GUIDES {
                break;
            }
            let Some(pads) = board.pad_nets.get(*net) else { continue };
            if pads.len() < 2 {
                continue;
            }
            // Prefer direct from -> to pad pair when both endpoint refs exist.
            let from_pads: Vec<&(String, f64, f64)> = if from_refs.is_empty() {
                vec![]
            } else {
                pads.iter().filter(|(r, _, _)| from_refs.contains(r.as_str())).collect()
            };
            let to_pads: Vec<&(String, f64, f64)> = if to_refs.is_empty() {
                vec![]
            } else {
                pads.iter().filter(|(r, _, _)| to_refs.contains(r.as_str())).collect()
            };

            if !from_pads.is_empty() && !to_pads.is_empty() {
                let mut best: Option<(f64, [f64; 4])> = None;
                for a in from_pads.iter().take(32) {
                    for b in to_pads.iter().take(32) {
                        let d = (a.1 - b.1).hypot(a.2 - b.2);
                        if d > 0.05 && best.map_or(true, |x| d < x.0) {
                            best = Some((d, [r2(a.1), r2(a.2), r2(b.1), r2(b.2)]));
                        }
                    }
                }
                if let Some((_, seg)) = best {
                    segs.push(seg);
                    continue;
                }
            }
            // Nearest-neighbor MST over the net's pads (capped to avoid quadratic blowup on huge power nets).
            let mut rem: Vec<(f64, f64)> = pads.iter().take(16).map(|p| (p.1, p.2)).collect();
            let mut tree = vec![rem.remove(0)];
            while !rem.is_empty() && segs.len() < MAX_CORRIDOR_GUIDES && total_segs + segs.len() < MAX_TOTAL_GUIDES {
                let mut best: Option<(usize, [f64; 4], f64)> = None;
                for a in &tree {
                    for (bi, b) in rem.iter().enumerate() {
                        let d = (a.0 - b.0).hypot(a.1 - b.1);
                        if best.map_or(true, |x| d < x.2) {
                            best = Some((bi, [r2(a.0), r2(a.1), r2(b.0), r2(b.1)], d));
                        }
                    }
                }
                if let Some((bi, seg, d)) = best {
                    let next = rem.remove(bi);
                    tree.push(next);
                    if d > 0.05 {
                        segs.push(seg);
                    }
                } else {
                    break;
                }
            }
        }

        // Graceful fallback for early/synthetic plans with from/to refs but unrouted/unassigned pads.
        if segs.is_empty() && !from_refs.is_empty() && !to_refs.is_empty() {
            let mut f_sorted: Vec<&str> = from_refs.iter().copied().collect();
            let mut t_sorted: Vec<&str> = to_refs.iter().copied().collect();
            f_sorted.sort_unstable();
            t_sorted.sort_unstable();
            'fallback: for fr in f_sorted {
                for tr in &t_sorted {
                    if segs.len() >= MAX_CORRIDOR_GUIDES || total_segs + segs.len() >= MAX_TOTAL_GUIDES {
                        break 'fallback;
                    }
                    if let (Some(a), Some(b)) = (part_center.get(fr), part_center.get(tr)) {
                        if (a[0] - b[0]).hypot(a[1] - b[1]) > 0.5 {
                            segs.push([a[0], a[1], b[0], b[1]]);
                        }
                    }
                }
            }
        }

        if !segs.is_empty() {
            total_segs += segs.len();
            out.insert(cid.to_string(), segs);
        }
    }
    out
}

/// Compute a fallback bbox from plan geometry when no board file is supplied.
pub fn fallback_bbox(plan: &Value) -> [f64; 4] {
    let mut bb = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for c in plan["corridors"].as_array().into_iter().flatten() {
        for p in c["path"].as_array().into_iter().flatten() {
            if let (Some(x), Some(y)) = (p[0].as_f64(), p[1].as_f64()) {
                expand_bbox(&mut bb, x, y);
            }
        }
    }
    for pl in plan["planes"].as_array().into_iter().flatten() {
        for p in pl["outline"][0].as_array().into_iter().flatten() {
            if let (Some(x), Some(y)) = (p[0].as_f64(), p[1].as_f64()) {
                expand_bbox(&mut bb, x, y);
            }
        }
    }
    if valid_bbox(bb) {
        [r2(bb[0] - 5.0), r2(bb[1] - 5.0), r2(bb[2] + 5.0), r2(bb[3] + 5.0)]
    } else {
        [0.0, 0.0, 100.0, 80.0]
    }
}

pub fn board_json(geom: &BoardGeom, plan: &Value, max_parts: usize) -> Value {
    let parts = parts_for_plan(geom, plan, max_parts);
    json!({
        "bbox": geom.bbox,
        "outline": geom.outline,
        "parts": parts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_ecc83_fixture_outline_parts_and_layers() {
        let pcb = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../test/files/fixtures/plot/ecc83-pp.kicad_pcb");
        let bytes = std::fs::read(&pcb).expect("read ecc83-pp.kicad_pcb");
        let cached = parse_board(&bytes).expect("parse ecc83-pp.kicad_pcb");
        assert!(!cached.geom.outline.is_empty(), "Edge.Cuts outline present");
        assert!(cached.geom.bbox[2] > cached.geom.bbox[0] && cached.geom.bbox[3] > cached.geom.bbox[1]);
        assert!(cached.geom.parts.iter().any(|p| p.r == "U1"), "U1 extracted: {:?}", cached.geom.parts);
        assert_eq!(cached.geom.copper_layers.len(), 2);
        assert_eq!(cached.geom.copper_layers[0].layer, "F.Cu");
        assert_eq!(cached.geom.copper_layers[1].layer, "B.Cu");
        assert!(cached.geom.copper_layers[0].subtitle.contains("1510 µm"));
    }

    #[test]
    fn parses_multilayer_stackup_arcs_overhangs_and_ratsnest_guides() {
        let pcb = br#"(kicad_pcb
            (layers
                (0 "F.Cu" signal "TOP")
                (4 "In1.Cu" power "GND1")
                (6 "In2.Cu" power "PWR1")
                (8 "In3.Cu" power "GND2")
                (10 "In4.Cu" power "PWR2")
                (12 "In5.Cu" signal "SIG1")
                (14 "In6.Cu" power "GND3")
                (2 "B.Cu" signal "BOT")
            )
            (setup
                (stackup
                    (layer "F.Cu" (type "copper") (thickness 0.035))
                    (layer "dielectric 1" (type "prepreg") (thickness 0.04))
                    (layer "In1.Cu" (type "copper") (thickness 0.035))
                    (layer "dielectric 2" (type "core") (thickness 0.1))
                    (layer "In2.Cu" (type "copper") (thickness 0.035))
                    (layer "dielectric 3" (type "prepreg") (thickness 0.05))
                    (layer "In3.Cu" (type "copper") (thickness 0.035))
                    (layer "dielectric 4" (type "core") (thickness 0.03))
                    (layer "In4.Cu" (type "copper") (thickness 0.035))
                    (layer "dielectric 5" (type "prepreg") (thickness 0.06))
                    (layer "In5.Cu" (type "copper") (thickness 0.035))
                    (layer "dielectric 6" (type "core") (thickness 0.05))
                    (layer "In6.Cu" (type "copper") (thickness 0.035))
                    (layer "dielectric 7" (type "prepreg") (thickness 0.04))
                    (layer "B.Cu" (type "copper") (thickness 0.035))
                )
            )
            (net 1 "DDR_D0")
            (net 2 "GND")
            (gr_line (start 10 10) (end 90 10) (layer "Edge.Cuts") (width 0.1))
            (gr_arc (start 90 10) (mid 95 15) (end 100 20) (layer "Edge.Cuts") (width 0.1))
            (gr_line (start 100 20) (end 100 70) (layer "Edge.Cuts") (width 0.1))
            (gr_line (start 100 70) (end 10 70) (layer "Edge.Cuts") (width 0.1))
            (gr_line (start 10 70) (end 10 10) (layer "Edge.Cuts") (width 0.1))
            (footprint "Pkg:QFN" (layer "F.Cu") (at 30 40)
                (property "Reference" "U1")
                (fp_rect (start -5 -5) (end 5 5) (layer "F.CrtYd") (width 0.05))
                (pad "1" smd rect (at 4 0) (size 0.5 0.5) (layers "F.Cu") (net 1 "DDR_D0"))
            )
            (footprint "Pkg:BGA" (layer "F.Cu") (at 75 40 90)
                (property "Reference" "U2")
                (fp_rect (start -8 -8) (end 8 8) (layer "F.CrtYd") (width 0.05))
                (pad "2" smd rect (at -6 0) (size 0.5 0.5) (layers "F.Cu") (net 1 "DDR_D0"))
            )
            (footprint "Conn:Edge" (layer "F.Cu") (at 50 8)
                (property "Reference" "J1")
                (fp_rect (start -10 -4) (end 10 4) (layer "F.CrtYd") (width 0.05))
            )
        )"#;
        let cached = parse_board(pcb).expect("parse synthetic board");
        assert_eq!(cached.geom.outline.len(), 1, "5 Edge.Cuts segments chain into 1 closed contour");
        assert!(is_closed(&cached.geom.outline[0]));
        assert_eq!(cached.geom.bbox, [10.0, 10.0, 100.0, 70.0]);

        let j1 = cached.geom.parts.iter().find(|p| p.r == "J1").unwrap();
        assert!(j1.o, "J1 extends past top Edge.Cuts and is flagged overhanging");
        let u1 = cached.geom.parts.iter().find(|p| p.r == "U1").unwrap();
        assert!(!u1.o && u1.b == [25.0, 35.0, 35.0, 45.0]);

        let sig1 = cached.geom.copper_layers.iter().find(|l| l.layer == "In5.Cu").unwrap();
        assert_eq!(sig1.title, "L6 · SIG1");
        assert_eq!(sig1.subtitle, "Stripline between PWR2 (60 µm) and GND3 (50 µm core)");
        let pwr2 = cached.geom.copper_layers.iter().find(|l| l.layer == "In4.Cu").unwrap();
        assert_eq!(pwr2.title, "L5 · PWR2");
        assert_eq!(pwr2.subtitle, "Faces GND2 across 30 µm");

        let plan = json!({
            "groups": [{"id": "g1", "nets": ["DDR_D0"]}],
            "corridors": [{"id": "c1", "group": "g1", "from": ["U1"], "to": ["U2"]}]
        });
        let guides = guides_for_plan(&cached, &plan);
        assert_eq!(guides["c1"], vec![[34.0, 40.0, 75.0, 46.0]]);

        // Fallback component-center guides are deterministically sorted and capped at 18
        let parts_many: Vec<PartBox> = (0..10).map(|i| PartBox { r: format!("A{i}"), s: "F".into(), b: [10.0, (i * 5) as f64, 12.0, (i * 5 + 2) as f64], o: false }).chain(
            (0..10).map(|i| PartBox { r: format!("B{i}"), s: "F".into(), b: [80.0, (i * 5) as f64, 82.0, (i * 5 + 2) as f64], o: false })
        ).collect();
        let dummy = CachedBoard {
            geom: BoardGeom { bbox: [0.0, 0.0, 100.0, 100.0], outline: vec![], parts: parts_many, copper_layers: vec![] },
            pad_nets: HashMap::new(),
        };
        let p_many = json!({"corridors": [{"id": "cm", "from": (0..10).map(|i| format!("A{i}")).collect::<Vec<_>>(), "to": (0..10).map(|i| format!("B{i}")).collect::<Vec<_>>()}]});
        let g1 = guides_for_plan(&dummy, &p_many);
        let g2 = guides_for_plan(&dummy, &p_many);
        assert_eq!(g1["cm"].len(), 18);
        assert_eq!(g1, g2);
    }
}
