//! Per-layer interactive PCB routing floorplanner canvas rendered in GPUI.
//! Mirrors `web/src/plan-canvas.ts` so desktop and browser show identical board geometry,
//! signal corridors with glowing multi-pass highways, diamond vias, net-count pills,
//! translucent power pours, collision-free refdes labels, and direct vertex/layer/width steering.
use crate::{art::mono_font, hub::Command, i18n::trs, screens::mgmt_plan, theme, Backplane};
use gpui::{prelude::*, *};
use lyon::tessellation::{LineCap, LineJoin};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub const BOARD_TAG: u64 = 9042;
pub const COMPACT_PANEL_W: u64 = 400;
pub const EXPANDED_PANEL_W: u64 = 880;
const SIDE_BY_SIDE_MIN_W: f32 = 540.0;

// PCB darkroom palette (matches reference floorplanner visual language regardless of chrome theme).
const SUBSTRATE_BG: u32 = 0x111917;
const BOARD_FILL: u32 = 0x16201d;
const EDGE_STROKE: u32 = 0xd4e0db;
const PART_STROKE: u32 = 0x3d5049;
const PART_OVERHANG: u32 = 0x526b61;
const REFDES_COLOR: u32 = 0x8fa39b;
const BADGE_BG: u32 = 0x111917;
const BADGE_FG: u32 = 0xeef5f2;
const HANDLE_BG: u32 = 0xeef5f2;
const HANDLE_ACTIVE: u32 = 0xf59e0b;
const BAD_RED: u32 = 0xef4444;

const RAIL_PALETTE: &[u32] = &[
    0xf59e0b, 0x38bdf8, 0xc084fc, 0x2dd4bf, 0xef4444, 0xa3e635, 0xf97316, 0xec4899,
];
const BUS_PALETTE: &[u32] = &[
    0x14b8a6, 0xf59e0b, 0xc084fc, 0x22c55e, 0x38bdf8, 0xef4444, 0xe879f9, 0x84cc16,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Canvas,
    Split,
    List,
}
impl Default for ViewMode {
    fn default() -> Self { Self::Split }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridMode {
    Pair,
    All,
    Single,
}
impl Default for GridMode {
    fn default() -> Self { Self::Pair }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub cx: f32,
    pub cy: f32,
    pub scale: f32,
    pub fitted: f32,
    pub touched: bool,
}

impl Camera {
    pub fn fit(bbox: [f32; 4], w: f32, h: f32, pad: f32) -> Self {
        let bw = (bbox[2] - bbox[0]).abs().max(1.0);
        let bh = (bbox[3] - bbox[1]).abs().max(1.0);
        let uw = (w - pad * 2.0).max(20.0);
        let uh = (h - pad * 2.0).max(20.0);
        let scale = (uw / bw).min(uh / bh).clamp(0.05, 200.0);
        Self {
            cx: (bbox[0] + bbox[2]) * 0.5,
            cy: (bbox[1] + bbox[3]) * 0.5,
            scale,
            fitted: scale,
            touched: false,
        }
    }

    pub fn to_px(&self, x: f32, y: f32, w: f32, h: f32) -> (f32, f32) {
        (w * 0.5 + (x - self.cx) * self.scale, h * 0.5 + (y - self.cy) * self.scale)
    }

    pub fn to_world(&self, px_x: f32, px_y: f32, w: f32, h: f32) -> (f32, f32) {
        (
            self.cx + (px_x - w * 0.5) / self.scale.max(1e-4),
            self.cy + (px_y - h * 0.5) / self.scale.max(1e-4),
        )
    }

    pub fn pan(&self, dx_px: f32, dy_px: f32) -> Self {
        Self {
            cx: self.cx - dx_px / self.scale.max(1e-4),
            cy: self.cy - dy_px / self.scale.max(1e-4),
            touched: true,
            ..*self
        }
    }

    pub fn zoom(&self, factor: f32, mx: f32, my: f32, w: f32, h: f32) -> Self {
        let min_s = (self.fitted * 0.35).max(0.02);
        let max_s = (self.fitted * 24.0).max(40.0);
        let ns = (self.scale * factor).clamp(min_s, max_s);
        let (wx, wy) = self.to_world(mx, my, w, h);
        Self {
            cx: wx - (mx - w * 0.5) / ns,
            cy: wy - (my - h * 0.5) / ns,
            scale: ns,
            touched: true,
            ..*self
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    Vertex { id: String, is_plane: bool, idx: usize },
    Midpoint { id: String, is_plane: bool, seg_idx: usize, pos: [f64; 2] },
    Band { id: String },
    Region { id: String },
}

impl Hit {
    pub fn item_id(&self) -> &str {
        match self {
            Hit::Vertex { id, .. } | Hit::Midpoint { id, .. } | Hit::Band { id } | Hit::Region { id } => id,
        }
    }
}

#[derive(Clone, Debug)]
pub enum DragAction {
    Pan { last_x: f32, last_y: f32 },
    Vertex { id: String, is_plane: bool, idx: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedLabel {
    pub text: String,
    pub cx: f32,
    pub cy: f32,
}

pub struct State {
    pub view_mode: ViewMode,
    pub grid_mode: GridMode,
    pub sync_zoom: bool,
    pub expanded: bool,
    pub shown_layers: Vec<String>,
    pub shared_cam: Option<Camera>,
    pub layer_cams: HashMap<String, Camera>,
    pub card_bounds: HashMap<String, Bounds<Pixels>>,
    pub hover: Option<(String, Hit)>,
    pub hover_px: Option<(f32, f32)>,
    pub selected_vertex: Option<(String, usize)>,
    pub drag: Option<(String, DragAction)>,
    pub down_pos: Option<(f32, f32)>,
    pub optimistic: HashMap<String, Value>,
    pub optimistic_at: Option<Instant>,
    pub last_rev: Option<u64>,
    pub hydrated_key: String,
    pub hydrated_board: Option<Value>,
    pub hydrated_draw: Option<Value>,
    pub note_open: bool,
    pub note_input: Option<Entity<super::composer::Composer>>,
    pub focus: Option<FocusHandle>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            view_mode: ViewMode::Split,
            grid_mode: GridMode::Pair,
            sync_zoom: true,
            expanded: false,
            shown_layers: Vec::new(),
            shared_cam: None,
            layer_cams: HashMap::new(),
            card_bounds: HashMap::new(),
            hover: None,
            hover_px: None,
            selected_vertex: None,
            drag: None,
            down_pos: None,
            optimistic: HashMap::new(),
            optimistic_at: None,
            last_rev: None,
            hydrated_key: String::new(),
            hydrated_board: None,
            hydrated_draw: None,
            note_open: false,
            note_input: None,
            focus: None,
        }
    }
}

impl State {
    pub fn reset(&mut self) {
        self.shown_layers.clear();
        self.shared_cam = None;
        self.layer_cams.clear();
        self.card_bounds.clear();
        self.hover = None;
        self.hover_px = None;
        self.selected_vertex = None;
        self.drag = None;
        self.down_pos = None;
        self.optimistic.clear();
        self.optimistic_at = None;
        self.last_rev = None;
        self.hydrated_key.clear();
        self.hydrated_board = None;
        self.hydrated_draw = None;
        self.note_open = false;
        self.note_input = None;
    }

    pub fn fit_all(&mut self) {
        self.shared_cam = None;
        self.layer_cams.clear();
    }
}

pub fn parse_hex_rgb(c: &str, fallback: u32) -> u32 {
    let trimmed = c.trim().trim_start_matches('#');
    if trimmed.is_empty() {
        return fallback;
    }
    u32::from_str_radix(trimmed, 16).unwrap_or(fallback)
}

fn stable_hash(s: &str) -> usize {
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h as usize
}

/// Distinct semantic rail colors for power planes (keeps explicit non-default hex when set).
pub fn rail_color(net: &str, fallback: &str) -> u32 {
    let lower = fallback.trim().to_ascii_lowercase();
    if !lower.is_empty() && lower != "#5a7a9a" && lower != "#8a8a8a" {
        return parse_hex_rgb(fallback, 0x5a7a9a);
    }
    let up = net.to_ascii_uppercase();
    if up.contains("3V3A") || up.contains("AVDD") || up.contains("VDDA") {
        return 0x38bdf8;
    }
    if up.contains("1V8") || up.contains("1.8V") {
        return 0xc084fc;
    }
    if up.contains("2V5") || up.contains("2.5V") {
        return 0xa3e635;
    }
    if up.contains("3V3") || up.contains("3.3V") || up.contains("VCC") {
        return 0xf59e0b;
    }
    if up.contains("5V") || up.starts_with("+5") {
        return 0xef4444;
    }
    if up.contains("12V") || up.contains("24V") {
        return 0xf97316;
    }
    if up.contains("VBAT") || up.contains("VIN") {
        return 0xec4899;
    }
    if up.contains("GND") {
        return 0x2dd4bf;
    }
    RAIL_PALETTE[stable_hash(&up) % RAIL_PALETTE.len()]
}

/// High-contrast signal corridor colors (keeps explicit non-default hex when set).
pub fn bus_color(name: &str, fallback: &str) -> u32 {
    let lower = fallback.trim().to_ascii_lowercase();
    if !lower.is_empty() && lower != "#8a8a8a" {
        return parse_hex_rgb(fallback, 0x14b8a6);
    }
    BUS_PALETTE[stable_hash(name) % BUS_PALETTE.len()]
}

pub fn pts_from_val(v: &Value) -> Vec<[f64; 2]> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some([p[0].as_f64()?, p[1].as_f64()?]))
        .collect()
}

pub fn pts_to_val(pts: &[[f64; 2]]) -> Value {
    let arr: Vec<Value> = pts
        .iter()
        .map(|[x, y]| json!([(x * 1000.0).round() / 1000.0, (y * 1000.0).round() / 1000.0]))
        .collect();
    Value::Array(arr)
}

/// Exact arc-length 50% midpoint of a multi-segment polyline (for net-count badge placement).
pub fn polyline_midpoint(pts: &[[f64; 2]]) -> Option<[f64; 2]> {
    if pts.is_empty() {
        return None;
    }
    if pts.len() == 1 {
        return Some(pts[0]);
    }
    let mut total = 0.0f64;
    for w in pts.windows(2) {
        total += (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
    }
    if total < 1e-6 {
        return Some(pts[0]);
    }
    let target = total * 0.5;
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let len = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        if acc + len >= target && len > 1e-9 {
            let t = (target - acc) / len;
            return Some([
                w[0][0] + (w[1][0] - w[0][0]) * t,
                w[0][1] + (w[1][1] - w[0][1]) * t,
            ]);
        }
        acc += len;
    }
    pts.last().copied()
}

pub fn insert_vertex(pts: &[[f64; 2]], seg_idx: usize, pos: [f64; 2], closed: bool) -> Vec<[f64; 2]> {
    let mut out = pts.to_vec();
    if out.is_empty() {
        return vec![pos];
    }
    let at = if closed && seg_idx + 1 >= out.len() {
        out.len()
    } else {
        (seg_idx + 1).min(out.len())
    };
    out.insert(at, pos);
    out
}

pub fn remove_vertex(pts: &[[f64; 2]], idx: usize, closed: bool) -> Option<Vec<[f64; 2]>> {
    let min_pts = if closed { 3 } else { 2 };
    if pts.len() <= min_pts || idx >= pts.len() {
        return None;
    }
    let mut out = pts.to_vec();
    out.remove(idx);
    Some(out)
}

fn dist_to_segment_sq(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-6 {
        let ex = px - ax;
        let ey = py - ay;
        return ex * ex + ey * ey;
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0);
    let cx = ax + t * dx;
    let cy = ay + t * dy;
    let ex = px - cx;
    let ey = py - cy;
    ex * ex + ey * ey
}

fn point_in_ring(x: f64, y: f64, ring: &[[f64; 2]]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (xi, yi) = (ring[i][0], ring[i][1]);
        let (xj, yj) = (ring[j][0], ring[j][1]);
        let intersect = ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / ((yj - yi) + 1e-12) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn band_stroke_px(width_mm: f64, scale: f32) -> f32 {
    ((width_mm as f32) * scale).clamp(5.0, 32.0)
}

/// Priority hit-test on a layer canvas: Vertex > Midpoint > Band > Region (respecting polygon holes).
pub fn hit_test(
    bands: &[Value],
    regions: &[Value],
    layer: &str,
    selected_id: Option<&str>,
    cam: &Camera,
    w: f32,
    h: f32,
    mx: f32,
    my: f32,
) -> Option<Hit> {
    // 1 & 2: Handles for currently selected unlocked item on this layer
    if let Some(sel) = selected_id {
        for b in bands.iter().filter(|b| b["layer"] == layer && b["id"] == sel) {
            if b["locked"] != true {
                let pts = pts_from_val(&b["path"]);
                for (i, p) in pts.iter().enumerate() {
                    let (vx, vy) = cam.to_px(p[0] as f32, p[1] as f32, w, h);
                    if (mx - vx).hypot(my - vy) <= 9.5 {
                        return Some(Hit::Vertex { id: sel.to_string(), is_plane: false, idx: i });
                    }
                }
                for (i, seg) in pts.windows(2).enumerate() {
                    let mid = [(seg[0][0] + seg[1][0]) * 0.5, (seg[0][1] + seg[1][1]) * 0.5];
                    let (hx, hy) = cam.to_px(mid[0] as f32, mid[1] as f32, w, h);
                    if (mx - hx).hypot(my - hy) <= 8.5 {
                        return Some(Hit::Midpoint { id: sel.to_string(), is_plane: false, seg_idx: i, pos: mid });
                    }
                }
            }
        }
        for r in regions.iter().filter(|r| r["layer"] == layer && r["id"] == sel) {
            if r["locked"] != true {
                let pts = pts_from_val(&r["outline"]);
                for (i, p) in pts.iter().enumerate() {
                    let (vx, vy) = cam.to_px(p[0] as f32, p[1] as f32, w, h);
                    if (mx - vx).hypot(my - vy) <= 9.5 {
                        return Some(Hit::Vertex { id: sel.to_string(), is_plane: true, idx: i });
                    }
                }
                let n = pts.len();
                if n >= 3 {
                    for i in 0..n {
                        let a = pts[i];
                        let b = pts[(i + 1) % n];
                        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
                        let (hx, hy) = cam.to_px(mid[0] as f32, mid[1] as f32, w, h);
                        if (mx - hx).hypot(my - hy) <= 8.5 {
                            return Some(Hit::Midpoint { id: sel.to_string(), is_plane: true, seg_idx: i, pos: mid });
                        }
                    }
                }
            }
        }
    }

    // 3: Bands on this layer (reverse order so top-painted wins)
    let mut best_band: Option<(f32, String)> = None;
    for b in bands.iter().filter(|b| b["layer"] == layer) {
        let pts = pts_from_val(&b["path"]);
        if pts.len() < 2 {
            continue;
        }
        let thresh = (band_stroke_px(b["width"].as_f64().unwrap_or(1.5), cam.scale) * 0.5 + 3.0).max(7.0);
        let mut min_d2 = f32::MAX;
        for wseg in pts.windows(2) {
            let (ax, ay) = cam.to_px(wseg[0][0] as f32, wseg[0][1] as f32, w, h);
            let (bx, by) = cam.to_px(wseg[1][0] as f32, wseg[1][1] as f32, w, h);
            min_d2 = min_d2.min(dist_to_segment_sq(mx, my, ax, ay, bx, by));
        }
        let d = min_d2.sqrt();
        if d <= thresh && best_band.as_ref().map_or(true, |(bd, _)| d <= *bd) {
            if let Some(id) = b["id"].as_str() {
                best_band = Some((d, id.to_string()));
            }
        }
    }
    if let Some((_, id)) = best_band {
        return Some(Hit::Band { id });
    }

    // 4: Regions on this layer (reverse order; exclude holes)
    let (wx, wy) = cam.to_world(mx, my, w, h);
    let (wx, wy) = (wx as f64, wy as f64);
    for r in regions.iter().filter(|r| r["layer"] == layer).rev() {
        let outer = pts_from_val(&r["outline"]);
        if !point_in_ring(wx, wy, &outer) {
            continue;
        }
        let in_hole = r["holes"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|h| point_in_ring(wx, wy, &pts_from_val(h)));
        if !in_hole {
            if let Some(id) = r["id"].as_str() {
                return Some(Hit::Region { id: id.to_string() });
            }
        }
    }
    None
}

/// Default 2-layer pair: primary signal layer (most corridors) + primary power layer (most planes).
pub fn default_layer_pair(layers: &[Value]) -> Vec<String> {
    let mut best_sig: Option<(usize, String)> = None;
    let mut best_pwr: Option<(usize, String)> = None;
    let mut all_names: Vec<String> = Vec::new();
    for l in layers {
        let name = l["layer"].as_str().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        all_names.push(name.clone());
        let items = l["items"].as_array().cloned().unwrap_or_default();
        let cor_n = items.iter().filter(|i| i["kind"] == "corridor").count();
        let pln_n = items.iter().filter(|i| i["kind"] == "plane").count();
        let role = l["role"].as_str().unwrap_or("");
        if (cor_n > 0 || role == "signal") && best_sig.as_ref().map_or(true, |(c, _)| cor_n > *c) {
            best_sig = Some((cor_n, name.clone()));
        }
        if (pln_n > 0 || role == "power" || role == "plane") && best_pwr.as_ref().map_or(true, |(p, _)| pln_n > *p) {
            best_pwr = Some((pln_n, name));
        }
    }
    let mut out = Vec::new();
    if let Some((_, s)) = best_sig {
        out.push(s);
    }
    if let Some((_, p)) = best_pwr {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    for n in all_names {
        if out.len() >= 2 {
            break;
        }
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// Greedy screen-space collision-free placement of component refdes labels.
pub fn layout_labels(parts: &[Value], cam: &Camera, w: f32, h: f32, pinned: &[&str]) -> Vec<PlacedLabel> {
    let pin_set: HashSet<&str> = pinned.iter().copied().collect();
    struct Cand<'a> {
        r: &'a str,
        cx: f32,
        cy: f32,
        box_w: f32,
        box_h: f32,
        score: f32,
    }
    let mut cands: Vec<Cand<'_>> = Vec::new();
    for p in parts {
        let r = p["r"].as_str().unwrap_or("");
        let b = match p["b"].as_array() {
            Some(a) if a.len() == 4 => [
                a[0].as_f64().unwrap_or(0.0) as f32,
                a[1].as_f64().unwrap_or(0.0) as f32,
                a[2].as_f64().unwrap_or(0.0) as f32,
                a[3].as_f64().unwrap_or(0.0) as f32,
            ],
            _ => continue,
        };
        if r.is_empty() {
            continue;
        }
        let (x0, y0) = cam.to_px(b[0], b[1], w, h);
        let (x1, y1) = cam.to_px(b[2], b[3], w, h);
        let bw = (x1 - x0).abs();
        let bh = (y1 - y0).abs();
        if bw < 13.0 || bh < 9.0 {
            continue;
        }
        let cx = (x0 + x1) * 0.5;
        let cy = (y0 + y1) * 0.5;
        if cx < 8.0 || cx > w - 8.0 || cy < 8.0 || cy > h - 8.0 {
            continue;
        }
        let is_pin = pin_set.contains(r);
        let area = bw * bh;
        let score = area + if is_pin { 10_000.0 } else { 0.0 };
        let tw = (r.len() as f32) * 5.8 + 4.0;
        let th = 11.0;
        cands.push(Cand { r, cx, cy, box_w: tw, box_h: th, score });
    }
    cands.sort_by(|a, b| b.score.total_cmp(&a.score));

    let mut placed_boxes: Vec<[f32; 4]> = Vec::new();
    let mut out = Vec::new();
    for c in cands {
        let rect = [
            c.cx - c.box_w * 0.5,
            c.cy - c.box_h * 0.5,
            c.cx + c.box_w * 0.5,
            c.cy + c.box_h * 0.5,
        ];
        let overlaps = placed_boxes
            .iter()
            .any(|o| !(rect[2] <= o[0] || rect[0] >= o[2] || rect[3] <= o[1] || rect[1] >= o[3]));
        if !overlaps {
            placed_boxes.push(rect);
            out.push(PlacedLabel { text: c.r.to_string(), cx: c.cx, cy: c.cy });
        }
    }
    out
}

pub fn merge_optimistic_into_draw(draw: &Value, optimistic: &HashMap<String, Value>) -> Value {
    if optimistic.is_empty() {
        return draw.clone();
    }
    let mut out = draw.clone();
    for key in ["bands", "regions"] {
        if let Some(arr) = out[key].as_array_mut() {
            for item in arr.iter_mut() {
                if let Some(id) = item["id"].as_str() {
                    if let Some(patch) = optimistic.get(id) {
                        if let Some(obj) = patch.as_object() {
                            for (k, v) in obj {
                                item[k] = v.clone();
                            }
                            if item["state"] == "proposed" {
                                item["state"] = json!("edited");
                            }
                            item["edited"] = json!(true);
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn merge_optimistic_into_layers(layers: &Value, optimistic: &HashMap<String, Value>) -> Value {
    if optimistic.is_empty() {
        return layers.clone();
    }
    let mut arr = layers.as_array().cloned().unwrap_or_default();
    let mut moved: Vec<(String, Value)> = Vec::new();
    for l in arr.iter_mut() {
        let lname = l["layer"].as_str().unwrap_or("").to_string();
        if let Some(items) = l["items"].as_array_mut() {
            items.retain_mut(|item| {
                let id = item["id"].as_str().unwrap_or("");
                if let Some(patch) = optimistic.get(id) {
                    if let Some(w) = patch.get("width") {
                        item["width"] = w.clone();
                    }
                    if let Some(lk) = patch.get("locked") {
                        item["locked"] = lk.clone();
                    }
                    if item["state"] == "proposed" {
                        item["state"] = json!("edited");
                    }
                    item["edited"] = json!(true);
                    if let Some(nl) = patch["layer"].as_str() {
                        if nl != lname {
                            moved.push((nl.to_string(), item.clone()));
                            return false;
                        }
                    }
                }
                true
            });
        }
    }
    for (target, item) in moved {
        if let Some(l) = arr.iter_mut().find(|l| l["layer"] == target) {
            if let Some(items) = l["items"].as_array_mut() {
                items.push(item);
            }
        }
    }
    Value::Array(arr)
}

pub fn resolved_board_and_draw(app: &Backplane, d: &Value) -> (Value, Value) {
    let ui = &app.screens.mgmt.plan_ui;
    let pro = &app.screens.mgmt.project;
    let rev = d["revision"].as_u64().unwrap_or(0);
    let key = format!("{pro}:{rev}");
    let use_hyd = ui.hydrated_key == key;
    let board = if use_hyd {
        ui.hydrated_board.clone().unwrap_or_else(|| d["board"].clone())
    } else {
        d["board"].clone()
    };
    let raw_draw = if use_hyd {
        ui.hydrated_draw.clone().unwrap_or_else(|| d["draw"].clone())
    } else {
        d["draw"].clone()
    };
    (board, merge_optimistic_into_draw(&raw_draw, &ui.optimistic))
}

pub fn board_answered(app: &mut Backplane, reply: &Value, cx: &mut Context<Backplane>) {
    let ui = &mut app.screens.mgmt.plan_ui;
    let panel = &reply["panel"];
    if panel.is_null() {
        return;
    }
    let rev = panel["revision"].as_u64().unwrap_or(0);
    ui.hydrated_key = format!("{}:{rev}", app.screens.mgmt.project);
    ui.hydrated_board = Some(panel["board"].clone());
    ui.hydrated_draw = Some(panel["draw"].clone());
    cx.notify();
}

pub fn sync_revision_and_hydrate(app: &mut Backplane, d: &Value) {
    let rev = d["revision"].as_u64();
    let pro = app.screens.mgmt.project.clone();
    let ui = &mut app.screens.mgmt.plan_ui;
    if ui.last_rev != rev {
        ui.last_rev = rev;
        ui.optimistic.clear();
        ui.optimistic_at = None;
    } else if let Some(at) = ui.optimistic_at {
        if at.elapsed() > Duration::from_secs(3) {
            ui.optimistic.clear();
            ui.optimistic_at = None;
        }
    }
    let key = format!("{pro}:{}", rev.unwrap_or(0));
    if d["board_trimmed"] == true && ui.hydrated_key != key && !pro.is_empty() {
        ui.hydrated_key = key;
        let _ = app.tx.try_send(Command::Ask("plan.get".into(), json!({"pro": pro, "board": true}), BOARD_TAG));
    }
}

pub fn apply_optimistic(app: &mut Backplane, id: &str, patch: Value) {
    let ui = &mut app.screens.mgmt.plan_ui;
    let entry = ui.optimistic.entry(id.to_string()).or_insert_with(|| json!({}));
    if let (Some(dst), Some(src)) = (entry.as_object_mut(), patch.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    ui.optimistic_at = Some(Instant::now());
}

pub fn commit_item_fields(app: &mut Backplane, id: &str, fields: Value, cx: &mut Context<Backplane>) {
    apply_optimistic(app, id, fields.clone());
    let mut wire_fields = fields;
    if let Some(outer) = wire_fields.get("outline").cloned() {
        if outer.as_array().and_then(|a| a.first()).and_then(|p| p.as_array()).and_then(|p| p.first()).and_then(Value::as_f64).is_some() {
            let d = app.screens.mgmt.data("plan");
            let (_, draw) = resolved_board_and_draw(app, &d);
            if let Some(holes) = draw["regions"].as_array().into_iter().flatten().find(|r| r["id"] == id).and_then(|r| r["holes"].as_array()).filter(|h| !h.is_empty()) {
                let mut rings = Vec::with_capacity(1 + holes.len());
                rings.push(outer);
                rings.extend(holes.iter().cloned());
                if let Some(obj) = wire_fields.as_object_mut() {
                    obj.insert("outline".into(), Value::Array(rings));
                }
            }
        }
    }
    mgmt_plan::send(app, "plan.update", json!({"ops": [{"op": "set", "id": id, "fields": wire_fields}]}), cx);
}

fn board_bbox_f32(board: &Value) -> [f32; 4] {
    let bb = &board["bbox"];
    let x0 = bb[0].as_f64().unwrap_or(0.0) as f32;
    let y0 = bb[1].as_f64().unwrap_or(0.0) as f32;
    let x1 = bb[2].as_f64().unwrap_or(100.0) as f32;
    let y1 = bb[3].as_f64().unwrap_or(70.0) as f32;
    if (x1 - x0).abs() < 0.5 || (y1 - y0).abs() < 0.5 {
        [0.0, 0.0, 100.0, 70.0]
    } else {
        [x0, y0, x1, y1]
    }
}

fn alpha_col(rgb_hex: u32, alpha: u32) -> Hsla {
    rgba((rgb_hex << 8) | (alpha & 0xff)).into()
}

fn band_alphas(state: &str, selected: bool, hovered: bool) -> (u32, u32, u32, bool) {
    let (mut glow, mut body, mut core, bad) = match state {
        "approved" | "routing" => (0x2a, 0x82, 0xe8, false),
        "edited" => (0x34, 0x90, 0xf2, false),
        "routed" | "verified" => (0x16, 0x42, 0x96, false),
        "rejected" | "failed" => (0x0c, 0x20, 0x44, false),
        "blocked" => (0x2e, 0x7a, 0xe0, true),
        _ => (0x24, 0x72, 0xd8, false),
    };
    if selected {
        glow = (glow + 0x18).min(0x58);
        body = (body + 0x1e).min(0xb4);
        core = 0xff;
    } else if hovered {
        glow = (glow + 0x0e).min(0x48);
        body = (body + 0x12).min(0xa0);
    }
    (glow, body, core, bad)
}

fn region_alphas(state: &str, selected: bool, hovered: bool) -> (u32, u32, bool) {
    let (mut fill_a, mut edge_a, bad) = match state {
        "approved" | "routing" => (0x5c, 0xe8, false),
        "edited" => (0x62, 0xf2, false),
        "routed" | "verified" => (0x28, 0x8a, false),
        "rejected" | "failed" => (0x14, 0x45, false),
        "blocked" => (0x50, 0xf0, true),
        _ => (0x46, 0xd4, false),
    };
    if selected {
        fill_a = (fill_a + 0x1e).min(0x88);
        edge_a = 0xff;
    } else if hovered {
        fill_a = (fill_a + 0x10).min(0x74);
        edge_a = (edge_a + 0x18).min(0xff);
    }
    (fill_a, edge_a, bad)
}

fn paint_shaped_text(
    window: &mut Window,
    cx: &mut App,
    text: &str,
    size_px: f32,
    color: Hsla,
    center_x: f32,
    center_y: f32,
    bold: bool,
) {
    if text.is_empty() {
        return;
    }
    let mut font = mono_font();
    if bold {
        font.weight = FontWeight::BOLD;
    }
    let run = TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = window
        .text_system()
        .shape_line(SharedString::from(text.to_string()), px(size_px), &[run], None);
    let x = center_x - f32::from(shaped.width) * 0.5;
    let y = center_y - size_px * 0.56;
    let _ = shaped.paint(point(px(x), px(y)), px(size_px * 1.25), window, cx);
}

#[allow(clippy::too_many_arguments)]
fn paint_layer_scene(
    window: &mut Window,
    cx: &mut App,
    bounds: Bounds<Pixels>,
    layer: &str,
    board: &Value,
    draw: &Value,
    cam: &Camera,
    selected_id: Option<&str>,
    selected_vertex: Option<(&str, usize)>,
    hover: Option<&Hit>,
) {
    let w = f32::from(bounds.size.width).max(1.0);
    let h = f32::from(bounds.size.height).max(1.0);
    let ox = f32::from(bounds.origin.x);
    let oy = f32::from(bounds.origin.y);
    let pt = |wx: f32, wy: f32| {
        let (x, y) = cam.to_px(wx, wy, w, h);
        point(px(ox + x), px(oy + y))
    };

    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        // 1. Dark PCB substrate background
        window.paint_quad(fill(bounds, rgb(SUBSTRATE_BG)));

        // 2. Board interior fill (#16201d) from Edge.Cuts outline (or bbox fallback)
        let outline_polys: Vec<Vec<[f64; 2]>> = board["outline"]
            .as_array()
            .into_iter()
            .flatten()
            .map(pts_from_val)
            .filter(|p| p.len() >= 3)
            .collect();
        if !outline_polys.is_empty() {
            let mut pb = PathBuilder::fill().with_style(PathStyle::Fill(gpui::FillOptions::even_odd().with_tolerance(0.05)));
            for poly in &outline_polys {
                for (i, p) in poly.iter().enumerate() {
                    let q = pt(p[0] as f32, p[1] as f32);
                    if i == 0 { pb.move_to(q); } else { pb.line_to(q); }
                }
                pb.close();
            }
            if let Ok(path) = pb.build() {
                window.paint_path(path, rgb(BOARD_FILL));
            }
        } else {
            let bb = board_bbox_f32(board);
            let p0 = cam.to_px(bb[0], bb[1], w, h);
            let p1 = cam.to_px(bb[2], bb[3], w, h);
            let rect = Bounds::new(point(px(ox + p0.0), px(oy + p0.1)), size(px((p1.0 - p0.0).max(1.0)), px((p1.1 - p0.1).max(1.0))));
            window.paint_quad(fill(rect, rgb(BOARD_FILL)));
        }

        // 3. Power plane pours (on this layer, under parts & Edge.Cuts)
        let regions: Vec<&Value> = draw["regions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| r["layer"] == layer)
            .collect();
        for r in &regions {
            let id = r["id"].as_str().unwrap_or("");
            let outer = pts_from_val(&r["outline"]);
            if outer.len() < 3 {
                continue;
            }
            let sel = selected_id == Some(id);
            let hov = hover.map_or(false, |x| x.item_id() == id);
            let (fa, oa, bad) = region_alphas(r["state"].as_str().unwrap_or("proposed"), sel, hov);
            let col = rail_color(r["net"].as_str().unwrap_or(""), r["color"].as_str().unwrap_or(""));

            let mut fpb = PathBuilder::fill().with_style(PathStyle::Fill(gpui::FillOptions::even_odd().with_tolerance(0.05)));
            for (i, p) in outer.iter().enumerate() {
                let q = pt(p[0] as f32, p[1] as f32);
                if i == 0 { fpb.move_to(q); } else { fpb.line_to(q); }
            }
            fpb.close();
            for hole_v in r["holes"].as_array().into_iter().flatten() {
                let hole = pts_from_val(hole_v);
                if hole.len() >= 3 {
                    for (i, p) in hole.iter().enumerate() {
                        let q = pt(p[0] as f32, p[1] as f32);
                        if i == 0 { fpb.move_to(q); } else { fpb.line_to(q); }
                    }
                    fpb.close();
                }
            }
            if let Ok(path) = fpb.build() {
                window.paint_path(path, alpha_col(col, fa));
            }

            let sw = if sel || bad { 2.0 } else { 1.25 };
            let mut spb = PathBuilder::stroke(px(sw)).with_style(PathStyle::Stroke(
                gpui::StrokeOptions::default()
                    .with_line_width(sw)
                    .with_line_join(LineJoin::Round)
                    .with_tolerance(0.05),
            ));
            for (i, p) in outer.iter().chain(outer.first()).enumerate() {
                let q = pt(p[0] as f32, p[1] as f32);
                if i == 0 { spb.move_to(q); } else { spb.line_to(q); }
            }
            if let Ok(path) = spb.build() {
                let scol = if bad { alpha_col(BAD_RED, 0xf2) } else if sel { alpha_col(0xffffff, 0xe8) } else { alpha_col(col, oa) };
                window.paint_path(path, scol);
            }
        }

        // 4. Crisp Edge.Cuts outline (#d4e0db)
        if !outline_polys.is_empty() {
            let mut epb = PathBuilder::stroke(px(1.25)).with_style(PathStyle::Stroke(
                gpui::StrokeOptions::default()
                    .with_line_width(1.25)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Round)
                    .with_tolerance(0.05),
            ));
            for poly in &outline_polys {
                for (i, p) in poly.iter().chain(poly.first()).enumerate() {
                    let q = pt(p[0] as f32, p[1] as f32);
                    if i == 0 { epb.move_to(q); } else { epb.line_to(q); }
                }
            }
            if let Ok(path) = epb.build() {
                window.paint_path(path, alpha_col(EDGE_STROKE, 0xe8));
            }
        } else {
            let bb = board_bbox_f32(board);
            let mut epb = PathBuilder::stroke(px(1.25));
            let corners = [(bb[0], bb[1]), (bb[2], bb[1]), (bb[2], bb[3]), (bb[0], bb[3]), (bb[0], bb[1])];
            for (i, (x, y)) in corners.into_iter().enumerate() {
                let q = pt(x, y);
                if i == 0 { epb.move_to(q); } else { epb.line_to(q); }
            }
            if let Ok(path) = epb.build() {
                window.paint_path(path, alpha_col(EDGE_STROKE, 0xe8));
            }
        }

        // 5. Component courtyard/body outlines (solid top, dashed bottom/overhang)
        let parts = board["parts"].as_array().cloned().unwrap_or_default();
        let mut solid_pb = PathBuilder::stroke(px(0.85));
        let mut dash_pb = PathBuilder::stroke(px(0.95)).dash_array(&[px(4.0), px(3.0)]);
        let (mut has_solid, mut has_dash) = (false, false);
        for p in &parts {
            let Some(arr) = p["b"].as_array().filter(|a| a.len() == 4) else { continue };
            let (x0, y0) = (arr[0].as_f64().unwrap_or(0.0) as f32, arr[1].as_f64().unwrap_or(0.0) as f32);
            let (x1, y1) = (arr[2].as_f64().unwrap_or(0.0) as f32, arr[3].as_f64().unwrap_or(0.0) as f32);
            let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)];
            let target = if p["o"] == true { has_dash = true; &mut dash_pb } else { has_solid = true; &mut solid_pb };
            for (i, (cx_w, cy_w)) in corners.into_iter().enumerate() {
                let q = pt(cx_w, cy_w);
                if i == 0 { target.move_to(q); } else { target.line_to(q); }
            }
        }
        if has_solid {
            if let Ok(path) = solid_pb.build() { window.paint_path(path, alpha_col(PART_STROKE, 0xb8)); }
        }
        if has_dash {
            if let Ok(path) = dash_pb.build() { window.paint_path(path, alpha_col(PART_OVERHANG, 0xd8)); }
        }

        // 6. Collision-free component refdes labels
        let bands: Vec<&Value> = draw["bands"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["layer"] == layer)
            .collect();
        let mut pinned_refs: Vec<&str> = Vec::new();
        for b in &bands {
            for k in ["from", "to"] {
                for r in b[k].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    pinned_refs.push(r);
                }
            }
        }
        for lbl in layout_labels(&parts, cam, w, h, &pinned_refs) {
            paint_shaped_text(window, cx, &lbl.text, 9.5, alpha_col(REFDES_COLOR, 0xdd), ox + lbl.cx, oy + lbl.cy, false);
        }

        // 7. Faint color-tinted signal ratsnest guides
        for b in &bands {
            let guides = b["guides"].as_array().into_iter().flatten().collect::<Vec<_>>();
            if guides.is_empty() {
                continue;
            }
            let id = b["id"].as_str().unwrap_or("");
            let active = selected_id == Some(id) || hover.map_or(false, |x| x.item_id() == id);
            let col = bus_color(b["name"].as_str().unwrap_or(id), b["color"].as_str().unwrap_or(""));
            let mut gpb = PathBuilder::stroke(px(if active { 1.0 } else { 0.7 }));
            let mut any = false;
            for seg in guides {
                if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (seg[0].as_f64(), seg[1].as_f64(), seg[2].as_f64(), seg[3].as_f64()) {
                    gpb.move_to(pt(x1 as f32, y1 as f32));
                    gpb.line_to(pt(x2 as f32, y2 as f32));
                    any = true;
                }
            }
            if any {
                if let Ok(path) = gpb.build() {
                    window.paint_path(path, alpha_col(col, if active { 0x62 } else { 0x2c }));
                }
            }
        }

        // 8. Multi-pass rounded-cap signal bus corridors + diamond vias + net-count pills
        for b in &bands {
            let pts = pts_from_val(&b["path"]);
            if pts.len() < 2 {
                continue;
            }
            let id = b["id"].as_str().unwrap_or("");
            let sel = selected_id == Some(id);
            let hov = hover.map_or(false, |x| x.item_id() == id);
            let stt = b["state"].as_str().unwrap_or("proposed");
            let (glow_a, body_a, core_a, bad) = band_alphas(stt, sel, hov);
            let col = bus_color(b["name"].as_str().unwrap_or(id), b["color"].as_str().unwrap_or(""));
            let wpx = band_stroke_px(b["width"].as_f64().unwrap_or(1.5), cam.scale);

            let stroke_poly = |sw: f32| {
                let mut pb = PathBuilder::stroke(px(sw)).with_style(PathStyle::Stroke(
                    gpui::StrokeOptions::default()
                        .with_line_width(sw)
                        .with_line_cap(LineCap::Round)
                        .with_line_join(LineJoin::Round)
                        .with_tolerance(0.05),
                ));
                for (i, p) in pts.iter().enumerate() {
                    let q = pt(p[0] as f32, p[1] as f32);
                    if i == 0 { pb.move_to(q); } else { pb.line_to(q); }
                }
                pb.build().ok()
            };

            // Pass 1: outer halo glow
            if let Some(p) = stroke_poly(wpx + 6.0) {
                window.paint_path(p, alpha_col(col, glow_a));
            }
            // Optional blocked / selected outer border
            if bad || sel {
                if let Some(p) = stroke_poly(wpx + 2.5) {
                    window.paint_path(p, alpha_col(if bad { BAD_RED } else { 0xffffff }, if bad { 0xe8 } else { 0xb8 }));
                }
            }
            // Pass 2: translucent corridor envelope
            if let Some(p) = stroke_poly(wpx) {
                window.paint_path(p, alpha_col(col, body_a));
            }
            // Pass 3: crisp bright spine
            if let Some(p) = stroke_poly((wpx * 0.34).clamp(1.8, 5.0)) {
                window.paint_path(p, alpha_col(col, core_a));
            }

            // 9. Diamond via/escape markers
            for via in b["vias"].as_array().into_iter().flatten() {
                if let (Some(vx), Some(vy)) = (via[0].as_f64(), via[1].as_f64()) {
                    let (px_x, px_y) = cam.to_px(vx as f32, vy as f32, w, h);
                    let r = 4.2f32;
                    let dpts = [
                        point(px(ox + px_x), px(oy + px_y - r)),
                        point(px(ox + px_x + r), px(oy + px_y)),
                        point(px(ox + px_x), px(oy + px_y + r)),
                        point(px(ox + px_x - r), px(oy + px_y)),
                    ];
                    let mut fpb = PathBuilder::fill();
                    for (i, q) in dpts.iter().enumerate() {
                        if i == 0 { fpb.move_to(*q); } else { fpb.line_to(*q); }
                    }
                    fpb.close();
                    if let Ok(path) = fpb.build() {
                        window.paint_path(path, alpha_col(SUBSTRATE_BG, 0xf2));
                    }
                    let mut spb = PathBuilder::stroke(px(1.5));
                    for (i, q) in dpts.iter().chain(dpts.first()).enumerate() {
                        if i == 0 { spb.move_to(*q); } else { spb.line_to(*q); }
                    }
                    if let Ok(path) = spb.build() {
                        window.paint_path(path, alpha_col(if sel { 0xffffff } else { col }, 0xff));
                    }
                }
            }

            // 10. Centered net-count pill at arc-length midpoint
            let nets = b["nets"].as_u64().unwrap_or(0);
            if nets > 0 {
                if let Some(mid) = polyline_midpoint(&pts) {
                    let txt = nets.to_string();
                    let (mx_px, my_px) = cam.to_px(mid[0] as f32, mid[1] as f32, w, h);
                    let pw = (txt.len() as f32 * 6.5 + 8.0).max(16.0);
                    let ph = 15.0f32;
                    let badge_bounds = Bounds::new(
                        point(px(ox + mx_px - pw * 0.5), px(oy + my_px - ph * 0.5)),
                        size(px(pw), px(ph)),
                    );
                    window.paint_quad(gpui::quad(
                        badge_bounds,
                        px(0.),
                        alpha_col(BADGE_BG, 0xf2),
                        px(1.2),
                        alpha_col(if sel { 0xffffff } else { col }, 0xee),
                        BorderStyle::Solid,
                    ));
                    paint_shaped_text(window, cx, &txt, 10.0, alpha_col(BADGE_FG, 0xff), ox + mx_px, oy + my_px, true);
                }
            }
        }

        // 11. Interactive vertex & midpoint handles on selected item (if unlocked)
        if let Some(sel) = selected_id {
            let paint_handles = |pts: &[[f64; 2]], closed: bool, window: &mut Window| {
                let segs = if closed { pts.len() } else { pts.len().saturating_sub(1) };
                for i in 0..segs {
                    let a = pts[i];
                    let b = pts[(i + 1) % pts.len()];
                    let (mx_w, my_w) = ((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5);
                    let (hx, hy) = cam.to_px(mx_w as f32, my_w as f32, w, h);
                    let is_hov = matches!(hover, Some(Hit::Midpoint { id, seg_idx, .. }) if id == sel && *seg_idx == i);
                    let s = if is_hov { 9.0f32 } else { 7.5f32 };
                    let hb = Bounds::new(point(px(ox + hx - s * 0.5), px(oy + hy - s * 0.5)), size(px(s), px(s)));
                    window.paint_quad(gpui::quad(
                        hb,
                        px(0.),
                        alpha_col(BOARD_FILL, 0xe8),
                        px(1.0),
                        alpha_col(if is_hov { HANDLE_ACTIVE } else { 0x94a3b8 }, 0xee),
                        BorderStyle::Solid,
                    ));
                }
                for (i, p) in pts.iter().enumerate() {
                    let (vx, vy) = cam.to_px(p[0] as f32, p[1] as f32, w, h);
                    let is_sel_v = selected_vertex == Some((sel, i));
                    let is_hov_v = matches!(hover, Some(Hit::Vertex { id, idx, .. }) if id == sel && *idx == i);
                    let s = if is_sel_v || is_hov_v { 9.5f32 } else { 7.5f32 };
                    let vb = Bounds::new(point(px(ox + vx - s * 0.5), px(oy + vy - s * 0.5)), size(px(s), px(s)));
                    window.paint_quad(gpui::quad(
                        vb,
                        px(0.),
                        alpha_col(if is_sel_v || is_hov_v { HANDLE_ACTIVE } else { HANDLE_BG }, 0xff),
                        px(1.2),
                        alpha_col(SUBSTRATE_BG, 0xff),
                        BorderStyle::Solid,
                    ));
                }
            };
            for b in &bands {
                if b["id"] == sel && b["locked"] != true {
                    paint_handles(&pts_from_val(&b["path"]), false, window);
                }
            }
            for r in &regions {
                if r["id"] == sel && r["locked"] != true {
                    paint_handles(&pts_from_val(&r["outline"]), true, window);
                }
            }
        }

        // 12. Subtle top-right hover HUD pill
        if let Some(h_item) = hover {
            let hid = h_item.item_id();
            let hud_txt = bands
                .iter()
                .find(|b| b["id"] == hid)
                .map(|b| {
                    let name = b["name"].as_str().unwrap_or(hid);
                    let n = b["nets"].as_u64().unwrap_or(0);
                    let wd = b["width"].as_f64().unwrap_or(0.0);
                    format!("{name} · {n}n · {wd:.2}mm") // i18n-ok: compact metric hud
                })
                .or_else(|| {
                    regions.iter().find(|r| r["id"] == hid).map(|r| {
                        let net = r["net"].as_str().unwrap_or(hid);
                        let refr = r["reference"].as_str().unwrap_or("");
                        if refr.is_empty() { net.to_string() } else { format!("{net} · {refr}") } // i18n-ok: compact metric hud
                    })
                });
            if let Some(txt) = hud_txt {
                let pw = (txt.len() as f32 * 6.0 + 12.0).max(48.0);
                let hb = Bounds::new(point(px(ox + w - pw - 8.0), px(oy + 8.0)), size(px(pw), px(18.0)));
                window.paint_quad(gpui::quad(
                    hb,
                    px(0.),
                    alpha_col(SUBSTRATE_BG, 0xee),
                    px(1.0),
                    alpha_col(PART_OVERHANG, 0xee),
                    BorderStyle::Solid,
                ));
                paint_shaped_text(window, cx, &txt, 10.0, alpha_col(BADGE_FG, 0xff), ox + w - pw * 0.5 - 8.0, oy + 17.0, false);
            }
        }
    });
}

fn active_cam(ui: &State, layer: &str, bbox: [f32; 4], w: f32, h: f32) -> Camera {
    let raw = if ui.sync_zoom {
        ui.shared_cam
    } else {
        ui.layer_cams.get(layer).copied()
    };
    match raw {
        Some(c) if c.touched => c,
        _ => Camera::fit(bbox, w, h, 14.0),
    }
}

fn set_cam(ui: &mut State, layer: &str, cam: Camera) {
    if ui.sync_zoom {
        ui.shared_cam = Some(cam);
    } else {
        ui.layer_cams.insert(layer.to_string(), cam);
    }
}

pub fn visible_layers(ui: &State, layers: &[Value]) -> Vec<Value> {
    if layers.is_empty() {
        return Vec::new();
    }
    match ui.grid_mode {
        GridMode::All => layers.to_vec(),
        GridMode::Single => {
            let target = ui
                .shown_layers
                .first()
                .filter(|n| layers.iter().any(|l| l["layer"].as_str() == Some(n.as_str())))
                .cloned()
                .or_else(|| default_layer_pair(layers).into_iter().next())
                .or_else(|| layers.first().and_then(|l| l["layer"].as_str().map(str::to_string)));
            target
                .and_then(|t| layers.iter().find(|l| l["layer"].as_str() == Some(t.as_str())).cloned())
                .into_iter()
                .collect()
        }
        GridMode::Pair => {
            let names: Vec<String> = if !ui.shown_layers.is_empty()
                && ui.shown_layers.iter().all(|n| layers.iter().any(|l| l["layer"].as_str() == Some(n.as_str())))
            {
                ui.shown_layers.iter().take(2).cloned().collect()
            } else {
                default_layer_pair(layers)
            };
            let mut out = Vec::new();
            for n in names {
                if let Some(l) = layers.iter().find(|l| l["layer"].as_str() == Some(n.as_str())) {
                    out.push(l.clone());
                }
            }
            if out.is_empty() {
                layers.iter().take(2).cloned().collect()
            } else {
                out
            }
        }
    }
}

pub fn pick_layer_in_grid(ui: &mut State, layer_name: String) {
    if ui.grid_mode == GridMode::Single {
        ui.shown_layers = vec![layer_name];
    } else if ui.grid_mode == GridMode::Pair {
        if !ui.shown_layers.contains(&layer_name) {
            if ui.shown_layers.len() >= 2 {
                ui.shown_layers[1] = layer_name;
            } else {
                ui.shown_layers.push(layer_name);
            }
        }
    } else {
        ui.grid_mode = GridMode::Single;
        ui.shown_layers = vec![layer_name];
    }
}

fn layer_status_badge(l: &Value) -> (String, &'static str) {
    let items = l["items"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let any_blocked = items.iter().any(|i| matches!(i["state"].as_str(), Some("blocked" | "failed")));
    let any_routing = items.iter().any(|i| i["state"] == "routing");
    let any_awaiting = items.iter().any(|i| matches!(i["state"].as_str(), Some("proposed" | "edited")));
    let all_done = !items.is_empty() && items.iter().all(|i| matches!(i["state"].as_str(), Some("routed" | "verified")));
    let all_approved = !items.is_empty() && items.iter().all(|i| matches!(i["state"].as_str(), Some("approved" | "routed" | "verified")));
    if any_blocked { (trs("Blocked"), "bad") }
    else if any_routing { (trs("Routing"), "accent") }
    else if any_awaiting { (trs("Awaiting review"), "warn") }
    else if all_done { (trs("Routed"), "ok") }
    else if all_approved { (trs("Approved"), "accent") }
    else { (trs("Empty"), "dim") }
}

/// Interactive per-layer PCB floorplanner canvas grid view.
pub fn view(app: &mut Backplane, d: &Value, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (board, draw) = resolved_board_and_draw(app, d);
    let layers_val = merge_optimistic_into_layers(&d["layers"], &app.screens.mgmt.plan_ui.optimistic);
    let all_layers = layers_val.as_array().cloned().unwrap_or_default();
    let shown = visible_layers(&app.screens.mgmt.plan_ui, &all_layers);

    if app.screens.mgmt.plan_ui.focus.is_none() {
        app.screens.mgmt.plan_ui.focus = Some(cx.focus_handle());
    }
    let focus = app.screens.mgmt.plan_ui.focus.clone().unwrap();

    let two_cols = app.screens.mgmt.panel_w >= SIDE_BY_SIDE_MIN_W && shown.len() > 1;
    let bb = board_bbox_f32(&board);
    let aspect = ((bb[3] - bb[1]).abs() / (bb[2] - bb[0]).abs().max(1.0)).clamp(0.45, 1.35);
    let avail_card_w = if two_cols {
        (app.screens.mgmt.panel_w - 28.0) * 0.5
    } else {
        (app.screens.mgmt.panel_w - 18.0).max(240.0)
    };
    let canvas_h = if app.screens.mgmt.plan_ui.view_mode == ViewMode::Canvas && shown.len() == 1 {
        (avail_card_w * aspect).clamp(260.0, 480.0)
    } else {
        (avail_card_w * aspect).clamp(190.0, 340.0)
    };

    let selected_id = app.screens.mgmt.plan_sel.clone();
    let entity = cx.entity();

    let mut cards: Vec<AnyElement> = Vec::with_capacity(shown.len());
    for l in &shown {
        let layer_name = l["layer"].as_str().unwrap_or("").to_string();
        let title = l["title"].as_str().filter(|s| !s.is_empty()).unwrap_or(&layer_name).to_string();
        let subtitle = l["subtitle"].as_str().filter(|s| !s.is_empty()).or_else(|| l["stackup"].as_str()).unwrap_or("").to_string();
        let (st_label, st_tone) = layer_status_badge(l);
        let items = l["items"].as_array().cloned().unwrap_or_default();
        let has_awaiting = items.iter().any(|i| matches!(i["state"].as_str(), Some("proposed" | "edited")));
        let is_power = l["role"] == "power" || (!items.is_empty() && items.iter().all(|i| i["kind"] == "plane"));

        let ln_paint = layer_name.clone();
        let ln_down_l = layer_name.clone();
        let ln_down_r = layer_name.clone();
        let ln_move = layer_name.clone();
        let ln_up = layer_name.clone();
        let ln_wheel = layer_name.clone();
        let focus_down = focus.clone();
        let entity_paint = entity.clone();
        let board_c = board.clone();
        let draw_c = draw.clone();
        let bb_c = bb;

        let el = canvas(|b, _, _| b, move |_, b, window, cx| {
            let snap = entity_paint.update(cx, |app, _| {
                app.screens.mgmt.plan_ui.card_bounds.insert(ln_paint.clone(), b);
                let ui = &app.screens.mgmt.plan_ui;
                let w = f32::from(b.size.width).max(1.0);
                let h = f32::from(b.size.height).max(1.0);
                let cam = active_cam(ui, &ln_paint, bb_c, w, h);
                let hov = ui.hover.as_ref().filter(|(l, _)| l == &ln_paint).map(|(_, h)| h.clone());
                let sv = ui.selected_vertex.clone();
                (cam, app.screens.mgmt.plan_sel.clone(), sv, hov)
            });
            paint_layer_scene(
                window,
                cx,
                b,
                &ln_paint,
                &board_c,
                &draw_c,
                &snap.0,
                snap.1.as_deref(),
                snap.2.as_ref().map(|(i, idx)| (i.as_str(), *idx)),
                snap.3.as_ref(),
            );
        }).size_full();

        let ln_cursor = layer_name.clone();
        let canvas_box = div()
            .id(SharedString::from(format!("plan-card-canvas-{layer_name}")))
            .w_full()
            .h(px(canvas_h))
            .bg(rgb(SUBSTRATE_BG))
            .cursor(if app.screens.mgmt.plan_ui.hover.as_ref().map_or(false, |(l, h)| l == &ln_cursor && matches!(h, Hit::Vertex { .. } | Hit::Midpoint { .. })) {
                CursorStyle::PointingHand
            } else {
                CursorStyle::Crosshair
            })
            .child(el)
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                window.focus(&focus_down);
                handle_left_down(this, &ln_down_l, e, cx);
            }))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                handle_right_down(this, &ln_down_r, e, cx);
            }))
            .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                handle_mouse_move(this, &ln_move, e, cx);
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(move |this, e: &MouseUpEvent, _, cx| {
                handle_left_up(this, &ln_up, e, cx);
            }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if let Some((_l, DragAction::Vertex { id, is_plane, .. })) = this.screens.mgmt.plan_ui.drag.take() {
                    commit_optimistic_geometry(this, &id, is_plane, cx);
                }
                this.screens.mgmt.plan_ui.drag = None;
                this.screens.mgmt.plan_ui.down_pos = None;
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(move |this, e: &ScrollWheelEvent, _, cx| {
                handle_wheel(this, &ln_wheel, e, cx);
            }));

        let ln_ok = layer_name.clone();
        let ln_no = layer_name.clone();
        let mut head = div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .px_2()
            .py(px(5.))
            .bg(c("panel"))
            .border_b_1()
            .border_color(c("line"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(div().w(px(3.)).h(px(14.)).bg(c(if is_power { "warn" } else { "accent" })))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(12.)).text_color(c("text")).child(title))
                    .child(div().px_1().border_1().border_color(c(st_tone)).bg(c("raise")).text_size(crate::scale::tpx(10.)).text_color(c(st_tone)).child(st_label))
                    .when(has_awaiting, |hdr| {
                        let lo = ln_ok.clone();
                        let ln = ln_no.clone();
                        hdr.child(small_btn(cx, light, format!("mgmt:plan:card-ok:{ln_ok}"), "✓".into(), false, move |a, cx| mgmt_plan::approve(a, &lo, &[], true, cx))) // i18n-ok: a mark
                            .child(small_btn(cx, light, format!("mgmt:plan:card-no:{ln_no}"), "✗".into(), false, move |a, cx| mgmt_plan::approve(a, &ln, &[], false, cx))) // i18n-ok: a mark
                    }),
            );
        if !subtitle.is_empty() {
            head = head.child(div().text_size(crate::scale::tpx(10.)).text_color(c("dim")).overflow_hidden().whitespace_nowrap().text_ellipsis().child(subtitle));
        }

        // Power rail legend footer for power layers
        let mut card = div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_1()
            .border_color(c("line"))
            .bg(c("panel"))
            .child(head)
            .child(canvas_box);

        let regions_on_layer: Vec<&Value> = draw["regions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| r["layer"] == layer_name)
            .collect();
        if !regions_on_layer.is_empty() {
            let mut legend = div()
                .flex_shrink_0()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(5.))
                .px_2()
                .py(px(5.))
                .bg(c("panel"))
                .border_t_1()
                .border_color(c("line"));
            for r in regions_on_layer {
                let rid = r["id"].as_str().unwrap_or("").to_string();
                let net = r["net"].as_str().unwrap_or(&rid).to_string();
                let col = rail_color(&net, r["color"].as_str().unwrap_or(""));
                let is_sel = selected_id.as_deref() == Some(rid.as_str());
                let rid_c = rid.clone();
                legend = legend.child(
                    div()
                        .id(SharedString::from(format!("plan-legend-{layer_name}-{rid}")))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .px(px(5.))
                        .py(px(2.))
                        .cursor_pointer()
                        .border_1()
                        .border_color(if is_sel { c("accent") } else { c("line") })
                        .bg(if is_sel { c("raise") } else { c("bg") })
                        .hover(|s| s.bg(c("raise")))
                        .child(div().size(px(8.)).bg(rgb(col)))
                        .child(div().text_size(crate::scale::tpx(10.)).font(mono_font()).text_color(c(if is_sel { "text" } else { "dim" })).child(net))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.screens.mgmt.plan_sel = Some(rid_c.clone());
                            mgmt_plan::send(this, "plan.focus", json!({"id": rid_c}), cx);
                        })),
                );
            }
            card = card.child(legend);
        }

        cards.push(card.into_any_element());
    }

    // Build 1-col or 2-col layout
    let mut grid = div()
        .id("plan-canvas-grid")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .p(px(6.))
        .flex()
        .flex_col()
        .gap(px(8.));

    if two_cols {
        let mut iter = cards.into_iter();
        while let Some(a) = iter.next() {
            let mut row = div().w_full().flex().items_start().gap(px(8.)).child(div().flex_1().min_w_0().child(a));
            if let Some(b) = iter.next() {
                row = row.child(div().flex_1().min_w_0().child(b));
            } else {
                row = row.child(div().flex_1().min_w_0());
            }
            grid = grid.child(row);
        }
    } else {
        for card in cards {
            grid = grid.child(card);
        }
    }

    div()
        .id("plan-canvas-root")
        .track_focus(&focus)
        .key_context("PlanCanvas") // i18n-ok: key context id
        .size_full()
        .flex()
        .flex_col()
        .min_h_0()
        .bg(c("bg"))
        .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
            handle_key_down(this, e, cx);
        }))
        .child(grid)
        .into_any_element()
}

fn small_btn(cx: &mut Context<Backplane>, light: bool, id: String, label: String, primary: bool, on: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div()
        .id(SharedString::from(id))
        .h(crate::scale::tpx(18.))
        .px(px(6.))
        .flex()
        .items_center()
        .cursor_pointer()
        .border_1()
        .text_size(crate::scale::tpx(10.))
        .when(primary, |d| d.bg(c("accent")).border_color(c("accent")).text_color(c("on-accent")))
        .when(!primary, |d| d.bg(c("raise")).border_color(c("line")).text_color(c("text")).hover(|s| s.bg(c("line"))))
        .child(label)
        .on_click(cx.listener(move |this, _, _, cx| { on(this, cx); cx.notify(); }))
        .into_any_element()
}

fn canvas_hit_at(app: &Backplane, layer: &str, abs_x: f32, abs_y: f32) -> Option<(Bounds<Pixels>, Camera, Option<Hit>, f32, f32)> {
    let ui = &app.screens.mgmt.plan_ui;
    let b = ui.card_bounds.get(layer).copied()?;
    let w = f32::from(b.size.width).max(1.0);
    let h = f32::from(b.size.height).max(1.0);
    let lx = abs_x - f32::from(b.origin.x);
    let ly = abs_y - f32::from(b.origin.y);
    let d = app.screens.mgmt.data("plan");
    let (board, draw) = resolved_board_and_draw(app, &d);
    let bb = board_bbox_f32(&board);
    let cam = active_cam(ui, layer, bb, w, h);
    let bands = draw["bands"].as_array().cloned().unwrap_or_default();
    let regions = draw["regions"].as_array().cloned().unwrap_or_default();
    let hit = hit_test(&bands, &regions, layer, app.screens.mgmt.plan_sel.as_deref(), &cam, w, h, lx, ly);
    Some((b, cam, hit, lx, ly))
}

fn handle_left_down(app: &mut Backplane, layer: &str, e: &MouseDownEvent, cx: &mut Context<Backplane>) {
    let (ax, ay) = (f32::from(e.position.x), f32::from(e.position.y));
    let Some((b, cam, hit, lx, ly)) = canvas_hit_at(app, layer, ax, ay) else { return };
    app.screens.mgmt.plan_ui.down_pos = Some((ax, ay));
    let w = f32::from(b.size.width).max(1.0);
    let h = f32::from(b.size.height).max(1.0);

    match hit {
        Some(Hit::Vertex { id, is_plane, idx }) => {
            app.screens.mgmt.plan_sel = Some(id.clone());
            app.screens.mgmt.plan_ui.selected_vertex = Some((id.clone(), idx));
            app.screens.mgmt.plan_ui.drag = Some((layer.to_string(), DragAction::Vertex { id, is_plane, idx }));
        }
        Some(Hit::Midpoint { id, is_plane, seg_idx, pos }) => {
            let d = app.screens.mgmt.data("plan");
            let (_, draw) = resolved_board_and_draw(app, &d);
            let pts = if is_plane {
                draw["regions"].as_array().into_iter().flatten().find(|r| r["id"] == id).map(|r| pts_from_val(&r["outline"])).unwrap_or_default()
            } else {
                draw["bands"].as_array().into_iter().flatten().find(|b| b["id"] == id).map(|b| pts_from_val(&b["path"])).unwrap_or_default()
            };
            let new_pts = insert_vertex(&pts, seg_idx, pos, is_plane);
            let inserted_idx = if is_plane && seg_idx + 1 >= pts.len() { new_pts.len() - 1 } else { (seg_idx + 1).min(new_pts.len() - 1) };
            let field_key = if is_plane { "outline" } else { "path" };
            apply_optimistic(app, &id, json!({ field_key: pts_to_val(&new_pts) }));
            app.screens.mgmt.plan_sel = Some(id.clone());
            app.screens.mgmt.plan_ui.selected_vertex = Some((id.clone(), inserted_idx));
            app.screens.mgmt.plan_ui.drag = Some((layer.to_string(), DragAction::Vertex { id, is_plane, idx: inserted_idx }));
        }
        Some(Hit::Band { id } | Hit::Region { id }) => {
            app.screens.mgmt.plan_sel = Some(id.clone());
            app.screens.mgmt.plan_ui.selected_vertex = None;
            app.screens.mgmt.plan_ui.drag = Some((layer.to_string(), DragAction::Pan { last_x: lx, last_y: ly }));
            let _ = cam; let _ = w; let _ = h;
        }
        None => {
            app.screens.mgmt.plan_ui.selected_vertex = None;
            app.screens.mgmt.plan_ui.drag = Some((layer.to_string(), DragAction::Pan { last_x: lx, last_y: ly }));
        }
    }
    cx.notify();
}

fn handle_right_down(app: &mut Backplane, layer: &str, e: &MouseDownEvent, cx: &mut Context<Backplane>) {
    let (ax, ay) = (f32::from(e.position.x), f32::from(e.position.y));
    let Some((_b, _cam, Some(Hit::Vertex { id, is_plane, idx }), _, _)) = canvas_hit_at(app, layer, ax, ay) else { return };
    let d = app.screens.mgmt.data("plan");
    let (_, draw) = resolved_board_and_draw(app, &d);
    let pts = if is_plane {
        draw["regions"].as_array().into_iter().flatten().find(|r| r["id"] == id).map(|r| pts_from_val(&r["outline"])).unwrap_or_default()
    } else {
        draw["bands"].as_array().into_iter().flatten().find(|b| b["id"] == id).map(|b| pts_from_val(&b["path"])).unwrap_or_default()
    };
    if let Some(new_pts) = remove_vertex(&pts, idx, is_plane) {
        let key = if is_plane { "outline" } else { "path" };
        app.screens.mgmt.plan_ui.selected_vertex = None;
        commit_item_fields(app, &id, json!({ key: pts_to_val(&new_pts) }), cx);
    }
}

fn handle_mouse_move(app: &mut Backplane, layer: &str, e: &MouseMoveEvent, cx: &mut Context<Backplane>) {
    let (ax, ay) = (f32::from(e.position.x), f32::from(e.position.y));
    let drag_copy = app.screens.mgmt.plan_ui.drag.clone();
    if let Some((drag_layer, action)) = drag_copy {
        if drag_layer == layer && e.dragging() {
            let Some(b) = app.screens.mgmt.plan_ui.card_bounds.get(layer).copied() else { return };
            let w = f32::from(b.size.width).max(1.0);
            let h = f32::from(b.size.height).max(1.0);
            let lx = ax - f32::from(b.origin.x);
            let ly = ay - f32::from(b.origin.y);
            match action {
                DragAction::Pan { last_x, last_y } => {
                    let d = app.screens.mgmt.data("plan");
                    let (board, _) = resolved_board_and_draw(app, &d);
                    let bb = board_bbox_f32(&board);
                    let cur = active_cam(&app.screens.mgmt.plan_ui, layer, bb, w, h);
                    let next = cur.pan(lx - last_x, ly - last_y);
                    set_cam(&mut app.screens.mgmt.plan_ui, layer, next);
                    app.screens.mgmt.plan_ui.drag = Some((drag_layer, DragAction::Pan { last_x: lx, last_y: ly }));
                    cx.notify();
                    return;
                }
                DragAction::Vertex { id, is_plane, idx } => {
                    let d = app.screens.mgmt.data("plan");
                    let (board, draw) = resolved_board_and_draw(app, &d);
                    let bb = board_bbox_f32(&board);
                    let cam = active_cam(&app.screens.mgmt.plan_ui, layer, bb, w, h);
                    let (wx, wy) = cam.to_world(lx, ly, w, h);
                    let mut pts = if is_plane {
                        draw["regions"].as_array().into_iter().flatten().find(|r| r["id"] == id).map(|r| pts_from_val(&r["outline"])).unwrap_or_default()
                    } else {
                        draw["bands"].as_array().into_iter().flatten().find(|b| b["id"] == id).map(|b| pts_from_val(&b["path"])).unwrap_or_default()
                    };
                    if let Some(slot) = pts.get_mut(idx) {
                        *slot = [wx as f64, wy as f64];
                        let key = if is_plane { "outline" } else { "path" };
                        apply_optimistic(app, &id, json!({ key: pts_to_val(&pts) }));
                        cx.notify();
                    }
                    return;
                }
            }
        }
    }

    if let Some((_b, _cam, hit, lx, ly)) = canvas_hit_at(app, layer, ax, ay) {
        let next = hit.map(|h| (layer.to_string(), h));
        if app.screens.mgmt.plan_ui.hover != next {
            app.screens.mgmt.plan_ui.hover = next;
            app.screens.mgmt.plan_ui.hover_px = Some((lx, ly));
            cx.notify();
        }
    }
}

fn commit_optimistic_geometry(app: &mut Backplane, id: &str, is_plane: bool, cx: &mut Context<Backplane>) {
    let key = if is_plane { "outline" } else { "path" };
    if let Some(pts) = app.screens.mgmt.plan_ui.optimistic.get(id).and_then(|p| p.get(key)).cloned() {
        commit_item_fields(app, id, json!({ key: pts }), cx);
    }
}

fn handle_left_up(app: &mut Backplane, layer: &str, e: &MouseUpEvent, cx: &mut Context<Backplane>) {
    let (ax, ay) = (f32::from(e.position.x), f32::from(e.position.y));
    let down = app.screens.mgmt.plan_ui.down_pos.take();
    let drag = app.screens.mgmt.plan_ui.drag.take();
    if let Some((_, DragAction::Vertex { id, is_plane, .. })) = drag {
        commit_optimistic_geometry(app, &id, is_plane, cx);
        return;
    }
    if e.click_count >= 2 {
        app.screens.mgmt.plan_ui.fit_all();
        cx.notify();
        return;
    }
    if let Some((dx, dy)) = down {
        if (dx - ax).hypot(dy - ay) < 4.5 {
            if let Some((_b, _cam, hit, _, _)) = canvas_hit_at(app, layer, ax, ay) {
                match hit {
                    Some(Hit::Band { id } | Hit::Region { id }) => {
                        app.screens.mgmt.plan_sel = Some(id.clone());
                        mgmt_plan::send(app, "plan.focus", json!({"id": id}), cx);
                    }
                    None => {
                        app.screens.mgmt.plan_sel = None;
                        cx.notify();
                    }
                    _ => {}
                }
            }
        }
    }
}

fn handle_wheel(app: &mut Backplane, layer: &str, e: &ScrollWheelEvent, cx: &mut Context<Backplane>) {
    let Some(b) = app.screens.mgmt.plan_ui.card_bounds.get(layer).copied() else { return };
    let w = f32::from(b.size.width).max(1.0);
    let h = f32::from(b.size.height).max(1.0);
    let mx = f32::from(e.position.x - b.origin.x);
    let my = f32::from(e.position.y - b.origin.y);
    let d = app.screens.mgmt.data("plan");
    let (board, _) = resolved_board_and_draw(app, &d);
    let bb = board_bbox_f32(&board);
    let cur = active_cam(&app.screens.mgmt.plan_ui, layer, bb, w, h);
    let (lines, _x, y) = crate::mousecfg::delta(&e.delta);
    let dy = if lines { y * 16.0 } else { y };
    let factor = (1.0 + dy * 0.002).clamp(0.65, 1.55);
    let next = cur.zoom(factor, mx, my, w, h);
    set_cam(&mut app.screens.mgmt.plan_ui, layer, next);
    cx.notify();
}

fn handle_key_down(app: &mut Backplane, e: &KeyDownEvent, cx: &mut Context<Backplane>) {
    let key = e.keystroke.key.as_str();
    match key {
        "0" | "home" => {
            app.screens.mgmt.plan_ui.fit_all();
            cx.notify();
            return;
        }
        "escape" => {
            app.screens.mgmt.plan_sel = None;
            app.screens.mgmt.plan_ui.selected_vertex = None;
            cx.notify();
            return;
        }
        _ => {}
    }
    let Some(sel_id) = app.screens.mgmt.plan_sel.clone() else { return };
    let d = app.screens.mgmt.data("plan");
    let layers = merge_optimistic_into_layers(&d["layers"], &app.screens.mgmt.plan_ui.optimistic);
    let mut locked = false;
    let mut width = 1.5f64;
    let mut is_corridor = false;
    for l in layers.as_array().into_iter().flatten() {
        for it in l["items"].as_array().into_iter().flatten() {
            if it["id"] == sel_id {
                locked = it["locked"] == true;
                is_corridor = it["kind"] == "corridor";
                if let Some(w) = it["width"].as_f64() {
                    width = w;
                }
            }
        }
    }
    match key {
        "a" => mgmt_plan::approve(app, "", &[sel_id], true, cx),
        "r" => mgmt_plan::approve(app, "", &[sel_id], false, cx),
        "l" => commit_item_fields(app, &sel_id, json!({"locked": !locked}), cx),
        "w" if is_corridor && !locked => {
            let delta = if e.keystroke.modifiers.shift { -0.25 } else { 0.25 };
            let nw = ((width + delta).clamp(0.15, 25.0) * 100.0).round() / 100.0;
            commit_item_fields(app, &sel_id, json!({"width": nw}), cx);
        }
        "delete" | "backspace" if !locked => {
            if let Some((vid, idx)) = app.screens.mgmt.plan_ui.selected_vertex.clone() {
                if vid == sel_id {
                    let (_, draw) = resolved_board_and_draw(app, &d);
                    let is_plane = !is_corridor;
                    let pts = if is_plane {
                        draw["regions"].as_array().into_iter().flatten().find(|r| r["id"] == sel_id).map(|r| pts_from_val(&r["outline"])).unwrap_or_default()
                    } else {
                        draw["bands"].as_array().into_iter().flatten().find(|b| b["id"] == sel_id).map(|b| pts_from_val(&b["path"])).unwrap_or_default()
                    };
                    if let Some(new_pts) = remove_vertex(&pts, idx, is_plane) {
                        let fk = if is_plane { "outline" } else { "path" };
                        app.screens.mgmt.plan_ui.selected_vertex = None;
                        commit_item_fields(app, &sel_id, json!({ fk: pts_to_val(&new_pts) }), cx);
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn polyline_midpoint_handles_variable_segments() {
        assert_eq!(polyline_midpoint(&[]), None);
        assert_eq!(polyline_midpoint(&[[5.0, 6.0]]), Some([5.0, 6.0]));
        // 10mm horizontal + 30mm vertical => midpoint at total=20mm => (10, 10)
        let m = polyline_midpoint(&[[0.0, 0.0], [10.0, 0.0], [10.0, 30.0]]).unwrap();
        assert!((m[0] - 10.0).abs() < 1e-6 && (m[1] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn hit_test_prioritizes_vertices_midpoints_bands_then_regions_with_holes() {
        let cam = Camera::fit([0.0, 0.0, 100.0, 100.0], 200.0, 200.0, 0.0);
        // scale is 2.0 px/mm, center=(50,50) -> (100,100)
        let bands = vec![json!({
            "id": "c1", "layer": "In1.Cu", "width": 4.0, "locked": false,
            "path": [[10.0, 50.0], [90.0, 50.0]]
        })];
        let regions = vec![json!({
            "id": "p1", "layer": "In1.Cu", "locked": false,
            "outline": [[5.0, 5.0], [95.0, 5.0], [95.0, 95.0], [5.0, 95.0]],
            "holes": [[[30.0, 70.0], [45.0, 70.0], [45.0, 85.0], [30.0, 85.0]]]
        })];

        // Hit vertex of selected band c1 at (10,50) -> px(20,100)
        let h_v = hit_test(&bands, &regions, "In1.Cu", Some("c1"), &cam, 200.0, 200.0, 22.0, 101.0);
        assert_eq!(h_v, Some(Hit::Vertex { id: "c1".into(), is_plane: false, idx: 0 }));

        // Hit midpoint handle at (50,50) -> px(100,100)
        let h_m = hit_test(&bands, &regions, "In1.Cu", Some("c1"), &cam, 200.0, 200.0, 100.0, 100.0);
        assert!(matches!(h_m, Some(Hit::Midpoint { ref id, seg_idx: 0, .. }) if id == "c1"));

        // Without c1 selected, hitting (60,50) -> px(120,100) picks Band before Region under it
        let h_b = hit_test(&bands, &regions, "In1.Cu", None, &cam, 200.0, 200.0, 120.0, 100.0);
        assert_eq!(h_b, Some(Hit::Band { id: "c1".into() }));

        // Hit region at (15,20) -> px(30,40)
        let h_r = hit_test(&bands, &regions, "In1.Cu", None, &cam, 200.0, 200.0, 30.0, 40.0);
        assert_eq!(h_r, Some(Hit::Region { id: "p1".into() }));

        // Hole inside region at (38,78) -> px(76,156) yields None
        let h_hole = hit_test(&bands, &regions, "In1.Cu", None, &cam, 200.0, 200.0, 76.0, 156.0);
        assert_eq!(h_hole, None);
    }

    #[test]
    fn vertex_insertion_and_removal_enforces_minimum_counts() {
        let open = vec![[0.0, 0.0], [10.0, 0.0]];
        assert_eq!(remove_vertex(&open, 0, false), None);
        let open3 = insert_vertex(&open, 0, [5.0, 4.0], false);
        assert_eq!(open3, vec![[0.0, 0.0], [5.0, 4.0], [10.0, 0.0]]);
        assert_eq!(remove_vertex(&open3, 1, false), Some(open));

        let tri = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]];
        assert_eq!(remove_vertex(&tri, 0, true), None);
        let quad = insert_vertex(&tri, 2, [0.0, 10.0], true);
        assert_eq!(quad.len(), 4);
        assert_eq!(quad[3], [0.0, 10.0]);
    }

    #[test]
    fn default_pair_picks_busiest_signal_and_power_layers() {
        let layers = vec![
            json!({"layer": "F.Cu", "role": "signal", "items": [{"kind": "corridor"}]}),
            json!({"layer": "In1.Cu", "role": "power", "items": []}),
            json!({"layer": "In3.Cu", "role": "power", "items": [{"kind": "plane"}, {"kind": "plane"}]}),
            json!({"layer": "In5.Cu", "role": "signal", "items": [{"kind": "corridor"}, {"kind": "corridor"}, {"kind": "corridor"}]}),
        ];
        assert_eq!(default_layer_pair(&layers), vec!["In5.Cu".to_string(), "In3.Cu".to_string()]);
    }

    #[test]
    fn layout_labels_avoids_overlapping_boxes_and_keeps_pinned_first() {
        let cam = Camera::fit([0.0, 0.0, 100.0, 100.0], 400.0, 400.0, 0.0);
        let parts = vec![
            json!({"r": "U_BIG", "b": [20.0, 20.0, 40.0, 40.0], "o": false}),
            json!({"r": "R_OVERLAP", "b": [26.0, 26.0, 34.0, 34.0], "o": false}),
            json!({"r": "TINY", "b": [60.0, 60.0, 61.0, 61.0], "o": false}),
            json!({"r": "U2", "b": [70.0, 70.0, 85.0, 85.0], "o": false}),
        ];
        let placed = layout_labels(&parts, &cam, 400.0, 400.0, &[]);
        let names: Vec<&str> = placed.iter().map(|p| p.text.as_str()).collect();
        assert!(names.contains(&"U_BIG"));
        assert!(names.contains(&"U2"));
        assert!(!names.contains(&"R_OVERLAP"));
        assert!(!names.contains(&"TINY"));
    }

    #[test]
    fn rail_and_bus_palette_assignment() {
        assert_eq!(rail_color("+3V3", "#5a7a9a"), 0xf59e0b);
        assert_eq!(rail_color("+1V8", ""), 0xc084fc);
        assert_eq!(rail_color("+5V", "#123456"), 0x123456);
        assert_eq!(bus_color("USB_DP", "#14b8a6"), 0x14b8a6);
    }
}
