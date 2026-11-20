//! Hub-owned docking/tiling layout model for the viewer pane (pure data & operations).
//!
//! The layout is per thread and drawn by every client (desktop GPUI, iOS, Android, linked machines).
//! Whether a design tile hosts the embedded KiCad editor is a client-session decision (`design_mode`
//! + `holder`), never a property of `Tile`; [`kicad_tile`] picks the single eligible tile in a fixed,
//! deterministic order.

use crate::files::kicad::{gerber, project};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

fn watch_key(kind: u32, root: &str, path: &str) -> String {
    format!("{kind}|{root}|{path}")
}

pub const LAYOUT_VERSION: u32 = 1;
pub const MIN_TILE_PX: f32 = 180.0;
pub const MIN_KICAD_W_PX: f32 = 380.0;
pub const MIN_KICAD_H_PX: f32 = 300.0;

pub type TileId = String;
pub type SplitId = String;

fn default_version() -> u32 { LAYOUT_VERSION }
fn default_overlay_mode() -> OverlayMode { OverlayMode::Diff }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    /// Left / right split (horizontal arrangement).
    Row,
    /// Top / bottom split (vertical arrangement).
    Col,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub fn axis(self) -> Axis {
        match self {
            Side::Left | Side::Right => Axis::Row,
            Side::Top | Side::Bottom => Axis::Col,
        }
    }
    pub fn is_first(self) -> bool {
        matches!(self, Side::Left | Side::Top)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageKind {
    Board,
    Schematic,
    #[serde(rename = "3d")]
    Solid3d,
    Gerbers,
    Mech,
    Files,
    Diff,
    Review,
    Browser,
    Simulation,
}

impl PageKind {
    /// Design pages that can host an embedded KiCad editor on a desktop client.
    pub fn is_design(self) -> bool {
        matches!(self, PageKind::Board | PageKind::Schematic)
    }

    pub fn from_page_str(s: &str) -> Option<Self> {
        match s {
            "board" | "pcb" => Some(PageKind::Board),
            "schematic" | "sch" => Some(PageKind::Schematic),
            "3d" => Some(PageKind::Solid3d),
            "gerbers" | "gerbview" => Some(PageKind::Gerbers),
            "mech" => Some(PageKind::Mech),
            "files" | "image" | "pdf" | "text" => Some(PageKind::Files),
            "diff" => Some(PageKind::Diff),
            "review" => Some(PageKind::Review),
            "browser" => Some(PageKind::Browser),
            "simulation" | "sim" => Some(PageKind::Simulation),
            _ => None,
        }
    }

    /// Map a KiCad embedded frame name (including KiCad-only editors) to its owning `PageKind`.
    pub fn from_kicad_frame(frame: &str) -> Option<Self> {
        match frame.to_ascii_lowercase().as_str() {
            "pcb" | "board" | "footprints" | "fp" | "footprint" => Some(PageKind::Board),
            "sch" | "schematic" | "symbols" | "sym" | "symbol" => Some(PageKind::Schematic),
            "3d" | "3dview" => Some(PageKind::Solid3d),
            "simulation" | "sim" | "simulator" => Some(PageKind::Simulation),
            "gerbview" | "gerbers" | "gerber" => Some(PageKind::Gerbers),
            _ => None,
        }
    }
}

/// Opaque version reference for a historical tile or overlay.
///
/// `id` is an opaque identifier the hub resolves (e.g. short commit sha `"a1b2c3d4"`),
/// never a raw `git\t<gitdir>…` wire string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileVersion {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayMode {
    Old,
    Diff,
    New,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overlay {
    pub kind: PageKind,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<TileVersion>,
    #[serde(default = "default_overlay_mode")]
    pub mode: OverlayMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    pub id: TileId,
    pub kind: PageKind,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<TileVersion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<Overlay>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Node {
    Tile(Tile),
    Split {
        id: SplitId,
        axis: Axis,
        ratio: f32,
        a: Box<Node>,
        b: Box<Node>,
    },
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NodeWire {
            #[serde(rename = "type")]
            node_type: String,
            id: String,
            kind: Option<PageKind>,
            #[serde(default)]
            source: String,
            #[serde(default)]
            version: Option<TileVersion>,
            #[serde(default)]
            overlay: Option<Overlay>,
            axis: Option<Axis>,
            ratio: Option<f32>,
            a: Option<Box<Node>>,
            b: Option<Box<Node>>,
        }
        let w = NodeWire::deserialize(deserializer)?;
        match w.node_type.as_str() {
            "tile" => Ok(Node::Tile(Tile {
                id: w.id,
                kind: w.kind.ok_or_else(|| serde::de::Error::missing_field("kind"))?,
                source: w.source,
                version: w.version,
                overlay: w.overlay,
            })),
            "split" => Ok(Node::Split {
                id: w.id,
                axis: w.axis.ok_or_else(|| serde::de::Error::missing_field("axis"))?,
                ratio: w.ratio.ok_or_else(|| serde::de::Error::missing_field("ratio"))?,
                a: w.a.ok_or_else(|| serde::de::Error::missing_field("a"))?,
                b: w.b.ok_or_else(|| serde::de::Error::missing_field("b"))?,
            }),
            other => Err(serde::de::Error::unknown_variant(other, &["tile", "split"])),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub root: Option<Node>,
    #[serde(default)]
    pub focus: Option<TileId>,
    #[serde(default)]
    pub maximized: Option<TileId>,
}

impl Default for Layout {
    fn default() -> Self { Self::empty() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Refusal {
    /// Operation would hide, move, close, swap, retarget, or cover a locked tile (e.g. KiCad modal open).
    Locked,
    /// Cannot open Simulation beside an existing KiCad tile (one KiCad host per process).
    OneKicad,
    /// Target tile or split ID does not exist in the layout.
    NotFound,
    /// Source and destination tile IDs are identical.
    SameTile,
    /// Target tile has no overlay to split out.
    NoOverlay,
    /// Source path is not relative to the project root or contains raw internal tokens.
    InvalidSource,
    /// Tile ID already exists in the layout.
    DuplicateId,
    /// Split ratio is not finite.
    InvalidRatio,
    /// Unknown KiCad frame name.
    UnknownFrame,
}

impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Refusal::Locked => "locked",
            Refusal::OneKicad => "oneKicad",
            Refusal::NotFound => "notFound",
            Refusal::SameTile => "sameTile",
            Refusal::NoOverlay => "noOverlay",
            Refusal::InvalidSource => "invalidSource",
            Refusal::DuplicateId => "duplicateId",
            Refusal::InvalidRatio => "invalidRatio",
            Refusal::UnknownFrame => "unknownFrame",
        }
    }
}

/// Returns true if `p` is empty (project default) or a clean relative path under the project root.
pub fn is_relative_source(p: &str) -> bool {
    if p.is_empty() {
        return true;
    }
    if p.starts_with('/') || p.starts_with('\\') {
        return false;
    }
    if p.as_bytes().get(1) == Some(&b':') && p.as_bytes()[0].is_ascii_alphabetic() {
        return false;
    }
    if p.contains('\n') || p.contains('\r') || p.contains('\t') {
        return false;
    }
    if p.starts_with("git\t") || p.starts_with("diff\n") || p.starts_with(gerber::CMP) || p.starts_with("noboard\n") {
        return false;
    }
    for seg in p.split(['/', '\\']) {
        if seg == ".." {
            return false;
        }
    }
    true
}

fn is_clean_version(v: &TileVersion) -> bool {
    !v.id.is_empty()
        && !v.id.contains('\t')
        && !v.id.contains('\n')
        && !v.label.contains('\t')
        && !v.label.contains('\n')
}

impl Layout {
    pub fn empty() -> Self {
        Self { version: LAYOUT_VERSION, root: None, focus: None, maximized: None }
    }

    pub fn single(tile: Tile) -> Result<Self, Refusal> {
        validate_tile(&tile)?;
        let id = tile.id.clone();
        Ok(Self {
            version: LAYOUT_VERSION,
            root: Some(Node::Tile(tile)),
            focus: Some(id),
            maximized: None,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Iterate all tiles in depth-first order (`a` before `b`).
    pub fn tiles(&self) -> Vec<&Tile> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            collect_tiles(root, &mut out);
        }
        out
    }

    pub fn tile(&self, id: &str) -> Option<&Tile> {
        self.tiles().into_iter().find(|t| t.id == id)
    }

    pub fn contains_tile(&self, id: &str) -> bool {
        self.tile(id).is_some()
    }

    pub fn contains_split(&self, id: &str) -> bool {
        self.root.as_ref().is_some_and(|n| node_contains_split(n, id))
    }

    /// Generate a fresh unique tile ID (`t1`, `t2`, ...).
    pub fn next_tile_id(&self) -> TileId {
        let mut n = 1usize;
        loop {
            let cand = format!("t{n}");
            if !self.contains_tile(&cand) {
                return cand;
            }
            n += 1;
        }
    }

    /// Generate a fresh unique split ID (`s1`, `s2`, ...).
    pub fn next_split_id(&self) -> SplitId {
        let mut n = 1usize;
        loop {
            let cand = format!("s{n}");
            if !self.contains_split(&cand) {
                return cand;
            }
            n += 1;
        }
    }
}

fn validate_tile(tile: &Tile) -> Result<(), Refusal> {
    if tile.id.is_empty() || !is_relative_source(&tile.source) {
        return Err(Refusal::InvalidSource);
    }
    if let Some(v) = &tile.version {
        if !is_clean_version(v) {
            return Err(Refusal::InvalidSource);
        }
    }
    if let Some(ov) = &tile.overlay {
        validate_overlay(ov)?;
    }
    Ok(())
}

fn validate_overlay(ov: &Overlay) -> Result<(), Refusal> {
    if !is_relative_source(&ov.source) {
        return Err(Refusal::InvalidSource);
    }
    if let Some(v) = &ov.version {
        if !is_clean_version(v) {
            return Err(Refusal::InvalidSource);
        }
    }
    Ok(())
}

fn collect_tiles<'a>(node: &'a Node, out: &mut Vec<&'a Tile>) {
    match node {
        Node::Tile(t) => out.push(t),
        Node::Split { a, b, .. } => {
            collect_tiles(a, out);
            collect_tiles(b, out);
        }
    }
}

fn node_contains_split(node: &Node, id: &str) -> bool {
    match node {
        Node::Tile(_) => false,
        Node::Split { id: sid, a, b, .. } => sid == id || node_contains_split(a, id) || node_contains_split(b, id),
    }
}

fn first_tile_id(node: &Node) -> TileId {
    match node {
        Node::Tile(t) => t.id.clone(),
        Node::Split { a, .. } => first_tile_id(a),
    }
}

/// Pick the single tile that should host the embedded KiCad editor for a client session.
///
/// Order: the focused tile first if it is a design page (`Board` or `Schematic`) and `eligible`
/// returns `true`; otherwise depth-first (`a` before `b`) for the first design tile with
/// `eligible(tile) == true`. Every other design tile renders as a viewer.
pub fn kicad_tile<F: Fn(&Tile) -> bool>(layout: &Layout, eligible: F) -> Option<TileId> {
    if let Some(fid) = &layout.focus {
        if let Some(t) = layout.tile(fid) {
            if t.kind.is_design() && eligible(t) {
                return Some(t.id.clone());
            }
        }
    }
    layout
        .tiles()
        .into_iter()
        .find(|t| t.kind.is_design() && eligible(t))
        .map(|t| t.id.clone())
}

/// Whether `kind` can be added as a new tile given whether this client already hosts an embedded
/// KiCad tile. Simulation cannot open beside an existing KiCad tile (`oneKicad`), but can open
/// beside a viewer-only Board (`client_has_kicad_tile == false`).
pub fn can_add(_layout: &Layout, kind: PageKind, client_has_kicad_tile: bool) -> Result<(), Refusal> {
    if kind == PageKind::Simulation && client_has_kicad_tile {
        return Err(Refusal::OneKicad);
    }
    Ok(())
}

/// Default per-tile minimum size callback: `380 × 300` for the KiCad tile, `180 × 180` for others.
pub fn default_min_for(kicad_tile_id: Option<&str>) -> impl Fn(&Tile) -> (f32, f32) + '_ {
    move |tile: &Tile| {
        if kicad_tile_id == Some(tile.id.as_str()) {
            (MIN_KICAD_W_PX, MIN_KICAD_H_PX)
        } else {
            (MIN_TILE_PX, MIN_TILE_PX)
        }
    }
}

/// Add a new tile beside `target` (or the focused/first tile if `target` is `None`, or as the root
/// if the layout is empty).
pub fn add(layout: &Layout, target: Option<&str>, side: Side, tile: Tile) -> Result<Layout, Refusal> {
    validate_tile(&tile)?;
    if layout.contains_tile(&tile.id) {
        return Err(Refusal::DuplicateId);
    }
    let Some(root) = &layout.root else {
        return Layout::single(tile);
    };
    let anchor = match target {
        Some(tid) => {
            if !layout.contains_tile(tid) {
                return Err(Refusal::NotFound);
            }
            tid.to_string()
        }
        None => layout.focus.clone().unwrap_or_else(|| first_tile_id(root)),
    };
    let split_id = layout.next_split_id();
    let new_id = tile.id.clone();
    let new_root = insert_beside(root, &anchor, side, tile, &split_id).ok_or(Refusal::NotFound)?;
    Ok(Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus: Some(new_id),
        maximized: None,
    })
}

fn insert_beside(node: &Node, anchor: &str, side: Side, tile: Tile, split_id: &str) -> Option<Node> {
    match node {
        Node::Tile(t) if t.id == anchor => {
            let new_leaf = Box::new(Node::Tile(tile));
            let old_leaf = Box::new(Node::Tile(t.clone()));
            let (a, b) = if side.is_first() { (new_leaf, old_leaf) } else { (old_leaf, new_leaf) };
            Some(Node::Split {
                id: split_id.to_string(),
                axis: side.axis(),
                ratio: 0.5,
                a,
                b,
            })
        }
        Node::Tile(_) => None,
        Node::Split { id, axis, ratio, a, b } => {
            if let Some(na) = insert_beside(a, anchor, side, tile.clone(), split_id) {
                Some(Node::Split { id: id.clone(), axis: *axis, ratio: *ratio, a: Box::new(na), b: b.clone() })
            } else {
                insert_beside(b, anchor, side, tile, split_id).map(|nb| Node::Split {
                    id: id.clone(),
                    axis: *axis,
                    ratio: *ratio,
                    a: a.clone(),
                    b: Box::new(nb),
                })
            }
        }
    }
}

/// Remove a tile from a subtree, returning `(removed_tile, remaining_subtree)`.
fn remove_tile(node: &Node, target: &str) -> Option<(Tile, Option<Node>)> {
    match node {
        Node::Tile(t) if t.id == target => Some((t.clone(), None)),
        Node::Tile(_) => None,
        Node::Split { id, axis, ratio, a, b } => {
            if let Some((removed, rem_a)) = remove_tile(a, target) {
                let next = match rem_a {
                    Some(na) => Node::Split { id: id.clone(), axis: *axis, ratio: *ratio, a: Box::new(na), b: b.clone() },
                    None => (**b).clone(),
                };
                Some((removed, Some(next)))
            } else if let Some((removed, rem_b)) = remove_tile(b, target) {
                let next = match rem_b {
                    Some(nb) => Node::Split { id: id.clone(), axis: *axis, ratio: *ratio, a: a.clone(), b: Box::new(nb) },
                    None => (**a).clone(),
                };
                Some((removed, Some(next)))
            } else {
                None
            }
        }
    }
}

/// Move `src` beside `dst` on `side`.
pub fn move_tile(layout: &Layout, src: &str, dst: &str, side: Side, locked: Option<&str>) -> Result<Layout, Refusal> {
    if locked == Some(src) || locked == Some(dst) {
        return Err(Refusal::Locked);
    }
    if src == dst {
        return Err(Refusal::SameTile);
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    if !layout.contains_tile(src) || !layout.contains_tile(dst) {
        return Err(Refusal::NotFound);
    }
    let (tile, rem) = remove_tile(root, src).ok_or(Refusal::NotFound)?;
    let rem_root = rem.ok_or(Refusal::NotFound)?;
    let intermediate = Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(rem_root),
        focus: Some(dst.to_string()),
        maximized: None,
    };
    add(&intermediate, Some(dst), side, tile)
}

/// Swap two tiles in place.
pub fn swap(layout: &Layout, a_id: &str, b_id: &str, locked: Option<&str>) -> Result<Layout, Refusal> {
    if locked == Some(a_id) || locked == Some(b_id) {
        return Err(Refusal::Locked);
    }
    if a_id == b_id {
        return Err(Refusal::SameTile);
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    let ta = layout.tile(a_id).ok_or(Refusal::NotFound)?.clone();
    let tb = layout.tile(b_id).ok_or(Refusal::NotFound)?.clone();
    let new_root = swap_nodes(root, a_id, &tb, b_id, &ta);
    Ok(Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus: layout.focus.clone(),
        maximized: layout.maximized.clone(),
    })
}

fn swap_nodes(node: &Node, a_id: &str, replacement_a: &Tile, b_id: &str, replacement_b: &Tile) -> Node {
    match node {
        Node::Tile(t) if t.id == a_id => Node::Tile(replacement_a.clone()),
        Node::Tile(t) if t.id == b_id => Node::Tile(replacement_b.clone()),
        Node::Tile(t) => Node::Tile(t.clone()),
        Node::Split { id, axis, ratio, a, b } => Node::Split {
            id: id.clone(),
            axis: *axis,
            ratio: *ratio,
            a: Box::new(swap_nodes(a, a_id, replacement_a, b_id, replacement_b)),
            b: Box::new(swap_nodes(b, a_id, replacement_a, b_id, replacement_b)),
        },
    }
}

/// Close `target` and collapse its parent split.
pub fn close(layout: &Layout, target: &str, locked: Option<&str>) -> Result<Layout, Refusal> {
    if locked == Some(target) {
        return Err(Refusal::Locked);
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    let (_, rem) = remove_tile(root, target).ok_or(Refusal::NotFound)?;
    let Some(new_root) = rem else {
        return Ok(Layout::empty());
    };
    let focus = match &layout.focus {
        Some(f) if f != target => Some(f.clone()),
        _ => Some(first_tile_id(&new_root)),
    };
    let maximized = match &layout.maximized {
        Some(m) if m != target => Some(m.clone()),
        _ => None,
    };
    Ok(Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus,
        maximized,
    })
}

fn subtree_min<F: Fn(&Tile) -> (f32, f32)>(node: &Node, min_for: &F) -> (f32, f32) {
    match node {
        Node::Tile(t) => {
            let (w, h) = min_for(t);
            (w.max(1.0), h.max(1.0))
        }
        Node::Split { axis, a, b, .. } => {
            let (aw, ah) = subtree_min(a, min_for);
            let (bw, bh) = subtree_min(b, min_for);
            match axis {
                Axis::Row => (aw + bw, ah.max(bh)),
                Axis::Col => (aw.max(bw), ah + bh),
            }
        }
    }
}

/// Resize `split_id` to `desired_ratio`, clamped so neither subtree drops below the minimum sizes
/// returned by `min_for` inside `bounds = (width_px, height_px)`.
pub fn resize<F: Fn(&Tile) -> (f32, f32)>(
    layout: &Layout,
    split_id: &str,
    desired_ratio: f32,
    bounds: (f32, f32),
    min_for: F,
) -> Result<Layout, Refusal> {
    if !desired_ratio.is_finite() {
        return Err(Refusal::InvalidRatio);
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    let new_root = resize_node(root, split_id, desired_ratio, bounds, &min_for).ok_or(Refusal::NotFound)?;
    Ok(Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus: layout.focus.clone(),
        maximized: layout.maximized.clone(),
    })
}

fn resize_node<F: Fn(&Tile) -> (f32, f32)>(
    node: &Node,
    split_id: &str,
    desired_ratio: f32,
    bounds: (f32, f32),
    min_for: &F,
) -> Option<Node> {
    match node {
        Node::Tile(_) => None,
        Node::Split { id, axis, ratio, a, b } => {
            if id == split_id {
                let (min_aw, min_ah) = subtree_min(a, min_for);
                let (min_bw, min_bh) = subtree_min(b, min_for);
                let (total, min_a, min_b) = match axis {
                    Axis::Row => (bounds.0, min_aw, min_bw),
                    Axis::Col => (bounds.1, min_ah, min_bh),
                };
                let clamped = if !total.is_finite() || total <= 0.0 || min_a + min_b >= total {
                    (min_a / (min_a + min_b)).clamp(0.05, 0.95)
                } else {
                    let lo = (min_a / total).clamp(0.05, 0.95);
                    let hi = (1.0 - min_b / total).clamp(0.05, 0.95);
                    if lo <= hi {
                        desired_ratio.clamp(lo, hi)
                    } else {
                        ((lo + hi) * 0.5).clamp(0.05, 0.95)
                    }
                };
                return Some(Node::Split {
                    id: id.clone(),
                    axis: *axis,
                    ratio: clamped,
                    a: a.clone(),
                    b: b.clone(),
                });
            }
            let (a_bounds, b_bounds) = match axis {
                Axis::Row => ((bounds.0 * ratio, bounds.1), (bounds.0 * (1.0 - ratio), bounds.1)),
                Axis::Col => ((bounds.0, bounds.1 * ratio), (bounds.0, bounds.1 * (1.0 - ratio))),
            };
            if let Some(na) = resize_node(a, split_id, desired_ratio, a_bounds, min_for) {
                Some(Node::Split { id: id.clone(), axis: *axis, ratio: *ratio, a: Box::new(na), b: b.clone() })
            } else {
                resize_node(b, split_id, desired_ratio, b_bounds, min_for).map(|nb| Node::Split {
                    id: id.clone(),
                    axis: *axis,
                    ratio: *ratio,
                    a: a.clone(),
                    b: Box::new(nb),
                })
            }
        }
    }
}

/// Maximize `target`, covering all other tiles without closing them.
///
/// Covering is **not** leaving: the desktop layout host hides a covered KiCad tile while keeping
/// its embedded surface attached, whereas closing or swapping is the leave. If `locked` names
/// another tile currently in the tree (e.g. a KiCad tile with an open modal dialog), maximizing
/// a different tile is refused with [`Refusal::Locked`].
pub fn maximize(layout: &Layout, target: &str, locked: Option<&str>) -> Result<Layout, Refusal> {
    if !layout.contains_tile(target) {
        return Err(Refusal::NotFound);
    }
    if let Some(lid) = locked {
        if lid != target && layout.contains_tile(lid) {
            return Err(Refusal::Locked);
        }
    }
    Ok(Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: layout.root.clone(),
        focus: Some(target.to_string()),
        maximized: Some(target.to_string()),
    })
}

/// Restore from a maximized tile back to the full split tree.
pub fn restore(layout: &Layout) -> Layout {
    Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: layout.root.clone(),
        focus: layout.focus.clone(),
        maximized: None,
    }
}

fn map_tile(node: &Node, target: &str, f: &dyn Fn(&Tile) -> Tile) -> Option<Node> {
    match node {
        Node::Tile(t) if t.id == target => Some(Node::Tile(f(t))),
        Node::Tile(_) => None,
        Node::Split { id, axis, ratio, a, b } => {
            if let Some(na) = map_tile(a, target, f) {
                Some(Node::Split { id: id.clone(), axis: *axis, ratio: *ratio, a: Box::new(na), b: b.clone() })
            } else {
                map_tile(b, target, f).map(|nb| Node::Split {
                    id: id.clone(),
                    axis: *axis,
                    ratio: *ratio,
                    a: a.clone(),
                    b: Box::new(nb),
                })
            }
        }
    }
}

/// Set or clear an overlay on `target`.
pub fn overlay(layout: &Layout, target: &str, ov: Option<Overlay>, locked: Option<&str>) -> Result<Layout, Refusal> {
    if locked == Some(target) {
        return Err(Refusal::Locked);
    }
    if let Some(o) = &ov {
        validate_overlay(o)?;
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    let new_root = map_tile(root, target, &|t| {
        let mut nt = t.clone();
        nt.overlay = ov.clone();
        nt
    })
    .ok_or(Refusal::NotFound)?;
    Ok(Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus: layout.focus.clone(),
        maximized: layout.maximized.clone(),
    })
}

/// Detach `target`'s overlay into its own tile beside `target` on `side`.
pub fn split_overlay(
    layout: &Layout,
    target: &str,
    side: Side,
    new_tile_id: Option<TileId>,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    if locked == Some(target) {
        return Err(Refusal::Locked);
    }
    let t = layout.tile(target).ok_or(Refusal::NotFound)?;
    let ov = t.overlay.clone().ok_or(Refusal::NoOverlay)?;
    let id = new_tile_id.unwrap_or_else(|| layout.next_tile_id());
    let cleared = overlay(layout, target, None, locked)?;
    let new_tile = Tile {
        id,
        kind: ov.kind,
        source: ov.source,
        version: ov.version,
        overlay: None,
    };
    add(&cleared, Some(target), side, new_tile)
}

/// Retarget an existing tile to a new `kind` and `source` (e.g. when the embedded KiCad tile
/// switches frames on KiCad's strip: Board ↔ Schematic, Symbols, Footprints, 3D, Simulation, GerbView).
pub fn retarget(
    layout: &Layout,
    target: &str,
    kind: PageKind,
    source: String,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    if locked == Some(target) {
        return Err(Refusal::Locked);
    }
    if !is_relative_source(&source) {
        return Err(Refusal::InvalidSource);
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    let new_root = map_tile(root, target, &|t| Tile {
        id: t.id.clone(),
        kind,
        source: source.clone(),
        version: None,
        overlay: None,
    })
    .ok_or(Refusal::NotFound)?;
    Ok(Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus: layout.focus.clone(),
        maximized: layout.maximized.clone(),
    })
}

/// Retarget a tile from a KiCad frame name (`pcb`, `sch`, `symbols`, `footprints`, `3d`, `simulation`, `gerbview`).
pub fn retarget_kicad_frame(
    layout: &Layout,
    target: &str,
    kicad_frame: &str,
    source: String,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    let kind = PageKind::from_kicad_frame(kicad_frame).ok_or(Refusal::UnknownFrame)?;
    retarget(layout, target, kind, source, locked)
}

/// Pure rail press rule:
/// - Focused tile has `kind`: close that tile.
/// - Another tile has `kind`: focus it (and restore if maximized on another tile).
/// - Absent `kind`: replace the focused tile, or open a 1-tile layout from idle.
pub fn rail_press(
    layout: &Layout,
    kind: PageKind,
    default_source: String,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    if !is_relative_source(&default_source) {
        return Err(Refusal::InvalidSource);
    }
    if let Some(fid) = layout.focus.as_deref() {
        if let Some(ft) = layout.tile(fid) {
            if ft.kind == kind {
                return close(layout, fid, locked);
            }
        }
    }
    if let Some(existing) = layout.tiles().into_iter().find(|t| t.kind == kind) {
        let mut next = layout.clone();
        next.focus = Some(existing.id.clone());
        if next.maximized.as_deref() != Some(existing.id.as_str()) {
            next.maximized = None;
        }
        return Ok(next);
    }
    if let Some(fid) = layout.focus.as_deref() {
        return retarget(layout, fid, kind, default_source, locked);
    }
    Layout::single(Tile {
        id: layout.next_tile_id(),
        kind,
        source: default_source,
        version: None,
        overlay: None,
    })
}

/// Pure `viewer_show` rule:
/// - A tile of `kind` exists: update its source (if non-empty) and focus it.
/// - Otherwise replace the focused tile, or open a 1-tile layout from idle.
pub fn viewer_show(
    layout: &Layout,
    kind: PageKind,
    source: String,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    if !is_relative_source(&source) {
        return Err(Refusal::InvalidSource);
    }
    if let Some(existing) = layout.tiles().into_iter().find(|t| t.kind == kind) {
        let id = existing.id.clone();
        let updated = if !source.is_empty() && source != existing.source {
            retarget(layout, &id, kind, source, locked)?
        } else {
            layout.clone()
        };
        let mut next = updated;
        next.focus = Some(id.clone());
        if next.maximized.as_deref() != Some(id.as_str()) {
            next.maximized = None;
        }
        return Ok(next);
    }
    if let Some(fid) = layout.focus.as_deref() {
        return retarget(layout, fid, kind, source, locked);
    }
    Layout::single(Tile {
        id: layout.next_tile_id(),
        kind,
        source,
        version: None,
        overlay: None,
    })
}

/// Verify all structural invariants of a layout.
pub fn check_invariants(layout: &Layout) -> Result<(), String> {
    if layout.version == 0 {
        return Err("version must be >= 1".into());
    }
    let Some(root) = &layout.root else {
        if layout.focus.is_some() {
            return Err("empty layout must not have focus".into());
        }
        if layout.maximized.is_some() {
            return Err("empty layout must not have maximized".into());
        }
        return Ok(());
    };
    let mut tile_ids = HashSet::new();
    let mut split_ids = HashSet::new();
    check_node(root, &mut tile_ids, &mut split_ids)?;
    let Some(fid) = &layout.focus else {
        return Err("non-empty layout must have focus".into());
    };
    if !tile_ids.contains(fid.as_str()) {
        return Err(format!("focus {fid:?} not in tree"));
    }
    if let Some(mid) = &layout.maximized {
        if !tile_ids.contains(mid.as_str()) {
            return Err(format!("maximized {mid:?} not in tree"));
        }
    }
    Ok(())
}

fn check_node(node: &Node, tile_ids: &mut HashSet<String>, split_ids: &mut HashSet<String>) -> Result<(), String> {
    match node {
        Node::Tile(t) => {
            if t.id.is_empty() {
                return Err("empty tile id".into());
            }
            if !tile_ids.insert(t.id.clone()) {
                return Err(format!("duplicate tile id {:?}", t.id));
            }
            if !is_relative_source(&t.source) {
                return Err(format!("tile {:?} has non-relative source {:?}", t.id, t.source));
            }
            if let Some(v) = &t.version {
                if !is_clean_version(v) {
                    return Err(format!("tile {:?} has invalid version {:?}", t.id, v));
                }
            }
            if let Some(ov) = &t.overlay {
                if !is_relative_source(&ov.source) {
                    return Err(format!("tile {:?} overlay has non-relative source {:?}", t.id, ov.source));
                }
                if let Some(v) = &ov.version {
                    if !is_clean_version(v) {
                        return Err(format!("tile {:?} overlay has invalid version {:?}", t.id, v));
                    }
                }
            }
            Ok(())
        }
        Node::Split { id, ratio, a, b, .. } => {
            if id.is_empty() {
                return Err("empty split id".into());
            }
            if !split_ids.insert(id.clone()) {
                return Err(format!("duplicate split id {:?}", id));
            }
            if !ratio.is_finite() || *ratio <= 0.0 || *ratio >= 1.0 {
                return Err(format!("split {:?} has invalid ratio {:?}", id, ratio));
            }
            check_node(a, tile_ids, split_ids)?;
            check_node(b, tile_ids, split_ids)?;
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Derivation of `viewer.layout` from today's single-page / Split / Gerbers / history state
// ---------------------------------------------------------------------------

/// Sanitize a path (absolute under `root` or already relative) into a clean project-relative path.
/// Returns `""` if the path cannot be represented safely relative to `root`.
pub fn sanitize_rel(root: &str, raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let rel = super::viewer::rel_in(root, raw).replace('\\', "/");
    let rel = rel.trim_start_matches("./");
    if is_relative_source(rel) {
        rel.to_string()
    } else {
        String::new()
    }
}

/// Parse a hub `git\t<gitdir>\t<commit>\t<len>\t<file>` version string into `(rel_source, TileVersion)`.
pub fn parse_git_source(root: &str, raw: &str) -> Option<(String, TileVersion)> {
    let rest = raw.strip_prefix("git\t")?;
    let mut parts = rest.splitn(4, '\t');
    let _gitdir = parts.next()?;
    let commit = parts.next()?;
    let _len = parts.next()?;
    let file = parts.next()?;
    let short: String = commit.chars().take(8).collect();
    if short.is_empty() {
        return None;
    }
    let rel = sanitize_rel(root, file);
    Some((rel, TileVersion { id: short.clone(), label: short }))
}

/// Parse a hub `diff\n<old>\n<new>[\n<bits>]` comparison string into
/// `(new_rel, new_ver, old_rel, old_ver)`.
pub fn parse_diff_source(root: &str, raw: &str) -> Option<(String, Option<TileVersion>, String, Option<TileVersion>)> {
    let rest = raw.strip_prefix("diff\n")?;
    let mut lines = rest.split('\n');
    let old_raw = lines.next()?;
    let new_raw = lines.next()?;
    let parse_side = |side: &str| -> (String, Option<TileVersion>) {
        if let Some((r, v)) = parse_git_source(root, side) {
            (r, Some(v))
        } else {
            (sanitize_rel(root, side), None)
        }
    };
    let (old_rel, old_ver) = parse_side(old_raw);
    let (new_rel, new_ver) = parse_side(new_raw);
    Some((new_rel, new_ver, old_rel, old_ver))
}

/// Inputs for deriving today's inert `viewer.layout` from the current single-page viewer state.
#[derive(Clone, Debug, Default)]
pub struct DeriveInput<'a> {
    pub root: &'a str,
    pub open: &'a str,
    pub diff_on: bool,
    pub gerbers: bool,
    pub gerbers_vs_board: bool,
    pub file: &'a str,
    pub sheet: &'a str,
    pub picked_board: &'a str,
    pub mech_rel: &'a str,
    pub files_dir: &'a str,
    pub default_pcb: &'a str,
    pub default_sch: &'a str,
    pub default_gerber_set: &'a str,
}

/// Derive a canonical [`Layout`] from today's single open page / Split / Gerbers / history comparison.
pub fn derive_layout(inp: &DeriveInput<'_>) -> Layout {
    if inp.diff_on && inp.open.is_empty() {
        return Layout::single(Tile {
            id: "t1".into(),
            kind: PageKind::Diff,
            source: String::new(),
            version: None,
            overlay: None,
        })
        .unwrap_or_else(|_| Layout::empty());
    }
    if inp.open.is_empty() {
        return Layout::empty();
    }
    if inp.open == "split" {
        let board_rel = if !inp.picked_board.is_empty() {
            sanitize_rel(inp.root, inp.picked_board)
        } else {
            sanitize_rel(inp.root, inp.default_pcb)
        };
        let sch_rel = if !inp.sheet.is_empty() {
            sanitize_rel(inp.root, inp.sheet)
        } else if !inp.picked_board.is_empty() {
            let derived = inp
                .picked_board
                .strip_suffix(".kicad_pcb")
                .map(|b| format!("{b}.kicad_sch"))
                .unwrap_or_default();
            if !derived.is_empty() {
                sanitize_rel(inp.root, &derived)
            } else {
                sanitize_rel(inp.root, inp.default_sch)
            }
        } else {
            sanitize_rel(inp.root, inp.default_sch)
        };
        return Layout {
            version: LAYOUT_VERSION,
            root: Some(Node::Split {
                id: "s1".into(),
                axis: Axis::Row,
                ratio: 0.5,
                a: Box::new(Node::Tile(Tile {
                    id: "sch".into(),
                    kind: PageKind::Schematic,
                    source: sch_rel,
                    version: None,
                    overlay: None,
                })),
                b: Box::new(Node::Split {
                    id: "s2".into(),
                    axis: Axis::Col,
                    ratio: 0.5,
                    a: Box::new(Node::Tile(Tile {
                        id: "pcb".into(),
                        kind: PageKind::Board,
                        source: board_rel.clone(),
                        version: None,
                        overlay: None,
                    })),
                    b: Box::new(Node::Tile(Tile {
                        id: "3d".into(),
                        kind: PageKind::Solid3d,
                        source: board_rel,
                        version: None,
                        overlay: None,
                    })),
                }),
            }),
            focus: Some("pcb".into()),
            maximized: None,
        };
    }

    if inp.gerbers {
        let raw_file = inp.file.strip_prefix("noboard\n").unwrap_or(inp.file);
        let set_raw = gerber::set_of(raw_file);
        let gerber_rel = if !set_raw.is_empty() {
            sanitize_rel(inp.root, set_raw)
        } else {
            sanitize_rel(inp.root, inp.default_gerber_set)
        };
        if inp.gerbers_vs_board {
            let board_raw = raw_file
                .strip_prefix(gerber::CMP)
                .and_then(|r| r.split_once('\n'))
                .map(|(_, b)| b)
                .unwrap_or("");
            let board_rel = if !board_raw.is_empty() {
                sanitize_rel(inp.root, board_raw)
            } else if !inp.picked_board.is_empty() {
                sanitize_rel(inp.root, inp.picked_board)
            } else {
                sanitize_rel(inp.root, inp.default_pcb)
            };
            return Layout::single(Tile {
                id: "t1".into(),
                kind: PageKind::Board,
                source: board_rel,
                version: None,
                overlay: Some(Overlay {
                    kind: PageKind::Gerbers,
                    source: gerber_rel,
                    version: None,
                    mode: OverlayMode::Diff,
                }),
            })
            .unwrap_or_else(|_| Layout::empty());
        }
        return Layout::single(Tile {
            id: "t1".into(),
            kind: PageKind::Gerbers,
            source: gerber_rel,
            version: None,
            overlay: None,
        })
        .unwrap_or_else(|_| Layout::empty());
    }

    let Some(kind) = PageKind::from_page_str(inp.open) else {
        return Layout::empty();
    };

    // History comparison (`diff\n<old>\n<new>[\n<bits>]`)
    if let Some((new_rel, new_ver, old_rel, old_ver)) = parse_diff_source(inp.root, inp.file) {
        return Layout::single(Tile {
            id: "t1".into(),
            kind,
            source: new_rel,
            version: new_ver,
            overlay: Some(Overlay {
                kind,
                source: old_rel,
                version: old_ver,
                mode: OverlayMode::Diff,
            }),
        })
        .unwrap_or_else(|_| Layout::empty());
    }

    // Single historical version (`git\t<gitdir>\t<commit>\t<len>\t<file>`)
    if let Some((rel, ver)) = parse_git_source(inp.root, inp.file) {
        return Layout::single(Tile {
            id: "t1".into(),
            kind,
            source: rel,
            version: Some(ver),
            overlay: None,
        })
        .unwrap_or_else(|_| Layout::empty());
    }

    let source = match kind {
        PageKind::Board | PageKind::Solid3d => {
            if !inp.file.is_empty() {
                sanitize_rel(inp.root, inp.file)
            } else if !inp.picked_board.is_empty() {
                sanitize_rel(inp.root, inp.picked_board)
            } else {
                sanitize_rel(inp.root, inp.default_pcb)
            }
        }
        PageKind::Schematic => {
            if !inp.sheet.is_empty() {
                sanitize_rel(inp.root, inp.sheet)
            } else if !inp.file.is_empty() {
                sanitize_rel(inp.root, inp.file)
            } else if !inp.picked_board.is_empty() {
                let derived = inp
                    .picked_board
                    .strip_suffix(".kicad_pcb")
                    .map(|b| format!("{b}.kicad_sch"))
                    .unwrap_or_default();
                if !derived.is_empty() {
                    sanitize_rel(inp.root, &derived)
                } else {
                    sanitize_rel(inp.root, inp.default_sch)
                }
            } else {
                sanitize_rel(inp.root, inp.default_sch)
            }
        }
        PageKind::Mech => sanitize_rel(inp.root, inp.mech_rel),
        PageKind::Files => {
            if !inp.file.is_empty() {
                sanitize_rel(inp.root, inp.file)
            } else {
                sanitize_rel(inp.root, inp.files_dir)
            }
        }
        PageKind::Gerbers | PageKind::Diff | PageKind::Review | PageKind::Browser | PageKind::Simulation => {
            sanitize_rel(inp.root, inp.file)
        }
    };

    Layout::single(Tile {
        id: "t1".into(),
        kind,
        source,
        version: None,
        overlay: None,
    })
    .unwrap_or_else(|_| Layout::empty())
}

// ---------------------------------------------------------------------------
// Pure N-source watch planner (`plan_watches`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct WatchContext {
    pub root: String,
    /// Project default PCB absolute path (from `kicad::project::read`).
    pub default_pcb: String,
    /// Project default schematic absolute path (from `kicad::project::read`).
    pub default_sch: String,
    /// Optional git directory for resolving `TileVersion.id` into `git\t…` specs.
    pub git_dir: String,
}

impl WatchContext {
    pub fn from_root(root: &str) -> Self {
        if root.is_empty() {
            return Self::default();
        }
        let files = project::read(root);
        Self {
            root: root.to_string(),
            default_pcb: files.pcb,
            default_sch: files.sch,
            git_dir: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchSpec {
    pub kind: u32,
    pub key: String,
    pub root: String,
    pub path: String,
}

fn join_root(root: &str, rel: &str) -> String {
    if rel.is_empty() {
        String::new()
    } else {
        format!("{}/{}", root.trim_end_matches('/'), rel)
    }
}

fn git_spec(git_dir: &str, ver: &TileVersion, abs_file: &str) -> String {
    format!("git\t{git_dir}\t{}\t{}\t{abs_file}", ver.id, abs_file.len())
}

/// Plan the deduplicated list of `kicad.watch` subscriptions needed to draw all tiles in `layout`.
///
/// Pure function over `layout` and `ctx`: does not touch `kicad.watch` runtime state.
/// On the Split preset, yields the exact three subscriptions and keys produced by
/// `kicad::watch::Watch::watch_split` (`1|<root>|<sch_at>`, `0|<root>|<pcb_at>`, `2|<root>|<pcb_at>`).
pub fn plan_watches(layout: &Layout, ctx: &WatchContext) -> Vec<WatchSpec> {
    if ctx.root.is_empty() {
        return Vec::new();
    }
    let mut specs = Vec::new();
    let mut seen = HashSet::new();
    for tile in layout.tiles() {
        let Some(spec) = plan_tile_watch(tile, ctx) else {
            continue;
        };
        if seen.insert(spec.key.clone()) {
            specs.push(spec);
        }
    }
    specs
}

fn plan_tile_watch(tile: &Tile, ctx: &WatchContext) -> Option<WatchSpec> {
    let root = &ctx.root;
    // Board + Gerbers diff overlay -> `gcmp\n<abs_set>\n<abs_board>`
    if tile.kind == PageKind::Board {
        if let Some(ov) = &tile.overlay {
            if ov.kind == PageKind::Gerbers && ov.mode == OverlayMode::Diff {
                let abs_set = join_root(root, &ov.source);
                let abs_board = if tile.source.is_empty() {
                    ctx.default_pcb.clone()
                } else {
                    join_root(root, &tile.source)
                };
                if abs_set.is_empty() || abs_board.is_empty() {
                    return None;
                }
                let cmp = gerber::cmp_path(&abs_set, &abs_board);
                let key = watch_key(0, root, &cmp);
                return Some(WatchSpec { kind: 0, key, root: root.clone(), path: cmp });
            }
        }
    }

    // History diff overlay on Board or Schematic -> `diff\n<old>\n<new>`
    if matches!(tile.kind, PageKind::Board | PageKind::Schematic) {
        if let Some(ov) = &tile.overlay {
            if ov.kind == tile.kind && ov.mode == OverlayMode::Diff && (tile.version.is_some() || ov.version.is_some()) {
                let k = if tile.kind == PageKind::Board { 0 } else { 1 };
                let default_abs = if k == 0 { &ctx.default_pcb } else { &ctx.default_sch };
                let abs_new = if tile.source.is_empty() { default_abs.clone() } else { join_root(root, &tile.source) };
                let abs_old = if ov.source.is_empty() { abs_new.clone() } else { join_root(root, &ov.source) };
                let side_spec = |ver: Option<&TileVersion>, abs: &str| match ver {
                    Some(v) if !ctx.git_dir.is_empty() => git_spec(&ctx.git_dir, v, abs),
                    _ => abs.to_string(),
                };
                let diff_path = format!("diff\n{}\n{}", side_spec(ov.version.as_ref(), &abs_old), side_spec(tile.version.as_ref(), &abs_new));
                let key = watch_key(k, root, &diff_path);
                return Some(WatchSpec { kind: k, key, root: root.clone(), path: diff_path });
            }
        }
    }

    match tile.kind {
        PageKind::Board => {
            let abs = if tile.source.is_empty() { ctx.default_pcb.clone() } else { join_root(root, &tile.source) };
            if abs.is_empty() {
                return None;
            }
            if let Some(ver) = &tile.version {
                if !ctx.git_dir.is_empty() {
                    let p = git_spec(&ctx.git_dir, ver, &abs);
                    return Some(WatchSpec { kind: 0, key: watch_key(0, root, &p), root: root.clone(), path: p });
                }
            }
            let at = if abs == ctx.default_pcb { "" } else { abs.as_str() };
            Some(WatchSpec { kind: 0, key: watch_key(0, root, at), root: root.clone(), path: abs })
        }
        PageKind::Schematic => {
            let abs = if tile.source.is_empty() { ctx.default_sch.clone() } else { join_root(root, &tile.source) };
            if abs.is_empty() {
                return None;
            }
            if let Some(ver) = &tile.version {
                if !ctx.git_dir.is_empty() {
                    let p = git_spec(&ctx.git_dir, ver, &abs);
                    return Some(WatchSpec { kind: 1, key: watch_key(1, root, &p), root: root.clone(), path: p });
                }
            }
            let at = if abs == ctx.default_sch { "" } else { abs.as_str() };
            Some(WatchSpec { kind: 1, key: watch_key(1, root, at), root: root.clone(), path: abs })
        }
        PageKind::Solid3d => {
            let abs = if tile.source.is_empty() { ctx.default_pcb.clone() } else { join_root(root, &tile.source) };
            if abs.is_empty() {
                return None;
            }
            let at = if abs == ctx.default_pcb { "" } else { abs.as_str() };
            Some(WatchSpec { kind: 2, key: watch_key(2, root, at), root: root.clone(), path: abs })
        }
        PageKind::Gerbers => {
            let abs = join_root(root, &tile.source);
            if abs.is_empty() {
                return None;
            }
            Some(WatchSpec { kind: 0, key: watch_key(0, root, &abs), root: root.clone(), path: abs })
        }
        PageKind::Mech => {
            let abs = join_root(root, &tile.source);
            if abs.is_empty() {
                return None;
            }
            Some(WatchSpec { kind: 2, key: watch_key(2, root, &abs), root: root.clone(), path: abs })
        }
        PageKind::Files | PageKind::Diff | PageKind::Review | PageKind::Browser | PageKind::Simulation => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(id: &str, kind: PageKind, source: &str) -> Tile {
        Tile { id: id.into(), kind, source: source.into(), version: None, overlay: None }
    }

    #[test]
    fn kicad_tile_picks_focused_first_then_depth_first_and_at_most_one() {
        let mut l = Layout::single(tile("t1", PageKind::Schematic, "a.kicad_sch")).unwrap();
        l = add(&l, Some("t1"), Side::Right, tile("t2", PageKind::Board, "a.kicad_pcb")).unwrap();
        l = add(&l, Some("t2"), Side::Bottom, tile("t3", PageKind::Solid3d, "a.kicad_pcb")).unwrap();
        // None eligible -> None
        assert_eq!(kicad_tile(&l, |_| false), None);
        // Focused tile ("t3") is 3D (not design); depth-first picks "t1" (a before b)
        assert_eq!(kicad_tile(&l, |_| true).as_deref(), Some("t1"));
        // Focus "t2" (Board, eligible) -> picks "t2" over "t1"
        l.focus = Some("t2".into());
        assert_eq!(kicad_tile(&l, |_| true).as_deref(), Some("t2"));
        // Two design tiles, only "t1" eligible -> picks "t1"
        assert_eq!(kicad_tile(&l, |t| t.id == "t1").as_deref(), Some("t1"));
        // Only non-design tile eligible -> None
        assert_eq!(kicad_tile(&l, |t| t.kind == PageKind::Solid3d), None);
    }

    #[test]
    fn retarget_updates_kind_and_source_and_maps_kicad_frames() {
        let l = Layout::single(tile("k1", PageKind::Board, "main.kicad_pcb")).unwrap();
        let l2 = retarget_kicad_frame(&l, "k1", "symbols", "main.kicad_sch".into(), None).unwrap();
        assert_eq!(l2.tile("k1").unwrap().kind, PageKind::Schematic);
        assert_eq!(l2.tile("k1").unwrap().source, "main.kicad_sch");

        let l3 = retarget_kicad_frame(&l2, "k1", "footprints", "main.kicad_pcb".into(), None).unwrap();
        assert_eq!(l3.tile("k1").unwrap().kind, PageKind::Board);

        let l4 = retarget_kicad_frame(&l3, "k1", "gerbview", "mfg/rev1".into(), None).unwrap();
        assert_eq!(l4.tile("k1").unwrap().kind, PageKind::Gerbers);

        let l5 = retarget_kicad_frame(&l4, "k1", "simulation", "main.kicad_sch".into(), None).unwrap();
        assert_eq!(l5.tile("k1").unwrap().kind, PageKind::Simulation);

        // Locked tile cannot be retargeted
        assert_eq!(
            retarget(&l, "k1", PageKind::Schematic, "main.kicad_sch".into(), Some("k1")),
            Err(Refusal::Locked)
        );
        // Absolute path refused
        assert_eq!(
            retarget(&l, "k1", PageKind::Schematic, "/proj/main.kicad_sch".into(), None),
            Err(Refusal::InvalidSource)
        );
    }

    #[test]
    fn can_add_refuses_simulation_beside_kicad_tile_but_allows_beside_viewer_board() {
        let l = Layout::single(tile("t1", PageKind::Board, "main.kicad_pcb")).unwrap();
        assert_eq!(can_add(&l, PageKind::Simulation, true), Err(Refusal::OneKicad));
        assert_eq!(Refusal::OneKicad.code(), "oneKicad");
        assert_eq!(can_add(&l, PageKind::Simulation, false), Ok(()));
        assert_eq!(can_add(&l, PageKind::Schematic, true), Ok(()));
    }

    #[test]
    fn resize_enforces_kicad_380x300_and_nested_subtree_minimums() {
        let l = Layout::single(tile("k", PageKind::Board, "main.kicad_pcb")).unwrap();
        let l = add(&l, Some("k"), Side::Right, tile("v", PageKind::Schematic, "main.kicad_sch")).unwrap();
        // Split s1 is Row: left="k" (KiCad min 380w), right="v" (min 180w), total width=1000.
        // Desired 0.1 -> clamped to 380 / 1000 = 0.38
        let r1 = resize(&l, "s1", 0.1, (1000.0, 800.0), default_min_for(Some("k"))).unwrap();
        let Node::Split { ratio, .. } = r1.root.as_ref().unwrap() else { panic!() };
        assert!((*ratio - 0.38).abs() < 1e-4, "ratio={ratio}");

        // Desired 0.95 -> clamped to 1.0 - 180/1000 = 0.82
        let r2 = resize(&l, "s1", 0.95, (1000.0, 800.0), default_min_for(Some("k"))).unwrap();
        let Node::Split { ratio, .. } = r2.root.as_ref().unwrap() else { panic!() };
        assert!((*ratio - 0.82).abs() < 1e-4, "ratio={ratio}");

        // Vertical split with KiCad min height 300 on top inside 600px height -> min ratio 0.5
        let lv = Layout::single(tile("k", PageKind::Board, "main.kicad_pcb")).unwrap();
        let lv = add(&lv, Some("k"), Side::Bottom, tile("v", PageKind::Solid3d, "main.kicad_pcb")).unwrap();
        let rv = resize(&lv, "s1", 0.2, (1000.0, 600.0), default_min_for(Some("k"))).unwrap();
        let Node::Split { ratio, .. } = rv.root.as_ref().unwrap() else { panic!() };
        assert!((*ratio - 0.5).abs() < 1e-4, "ratio={ratio}");

        // Non-finite ratio refused
        assert_eq!(resize(&l, "s1", f32::NAN, (1000.0, 800.0), default_min_for(None)), Err(Refusal::InvalidRatio));
    }

    #[test]
    fn locked_tile_refuses_move_swap_close_overlay_retarget_and_covering_maximize() {
        let l = Layout::single(tile("k", PageKind::Board, "a.kicad_pcb")).unwrap();
        let l = add(&l, Some("k"), Side::Right, tile("v", PageKind::Schematic, "a.kicad_sch")).unwrap();
        assert_eq!(move_tile(&l, "k", "v", Side::Top, Some("k")), Err(Refusal::Locked));
        assert_eq!(move_tile(&l, "v", "k", Side::Top, Some("k")), Err(Refusal::Locked));
        assert_eq!(swap(&l, "k", "v", Some("k")), Err(Refusal::Locked));
        assert_eq!(close(&l, "k", Some("k")), Err(Refusal::Locked));
        assert_eq!(maximize(&l, "v", Some("k")), Err(Refusal::Locked));
        // Maximizing the locked tile itself does NOT cover it, so it is allowed.
        let m = maximize(&l, "k", Some("k")).unwrap();
        assert_eq!(m.maximized.as_deref(), Some("k"));
        assert_eq!(restore(&m).maximized, None);
        assert_eq!(
            overlay(&l, "k", Some(Overlay { kind: PageKind::Gerbers, source: "mfg/r1".into(), version: None, mode: OverlayMode::Diff }), Some("k")),
            Err(Refusal::Locked)
        );
        assert_eq!(retarget(&l, "k", PageKind::Schematic, "a.kicad_sch".into(), Some("k")), Err(Refusal::Locked));
        // Closing the unlocked tile succeeds
        let c = close(&l, "v", Some("k")).unwrap();
        assert_eq!(c.tiles().len(), 1);
        assert_eq!(c.focus.as_deref(), Some("k"));
    }

    #[test]
    fn overlay_and_split_overlay_round_trip() {
        let l = Layout::single(tile("t1", PageKind::Board, "main.kicad_pcb")).unwrap();
        assert_eq!(split_overlay(&l, "t1", Side::Right, None, None), Err(Refusal::NoOverlay));
        let ov = Overlay { kind: PageKind::Gerbers, source: "mfg/rev1".into(), version: None, mode: OverlayMode::Diff };
        let l = overlay(&l, "t1", Some(ov), None).unwrap();
        let l = split_overlay(&l, "t1", Side::Right, Some("t2".into()), None).unwrap();
        check_invariants(&l).unwrap();
        assert!(l.tile("t1").unwrap().overlay.is_none());
        let t2 = l.tile("t2").unwrap();
        assert_eq!(t2.kind, PageKind::Gerbers);
        assert_eq!(t2.source, "mfg/rev1");
    }

    #[test]
    fn rail_press_and_viewer_show_follow_contract() {
        // Idle -> rail press opens 1-tile layout
        let l = rail_press(&Layout::empty(), PageKind::Board, "main.kicad_pcb".into(), None).unwrap();
        assert_eq!(l.tiles().len(), 1);
        assert_eq!(l.focus.as_deref(), Some("t1"));
        // Press same focused kind -> closes that tile (back to idle)
        let l = rail_press(&l, PageKind::Board, "main.kicad_pcb".into(), None).unwrap();
        assert!(l.is_empty());

        // Two tiles (Board + Schematic focused): pressing Board focuses Board; pressing Board again closes it
        let l = Layout::single(tile("t1", PageKind::Board, "main.kicad_pcb")).unwrap();
        let l = add(&l, Some("t1"), Side::Right, tile("t2", PageKind::Schematic, "main.kicad_sch")).unwrap();
        assert_eq!(l.focus.as_deref(), Some("t2"));
        let l = rail_press(&l, PageKind::Board, "main.kicad_pcb".into(), None).unwrap();
        assert_eq!(l.focus.as_deref(), Some("t1"));
        // Press absent kind (3D) -> replaces focused tile ("t1")
        let l = rail_press(&l, PageKind::Solid3d, "main.kicad_pcb".into(), None).unwrap();
        assert_eq!(l.tile("t1").unwrap().kind, PageKind::Solid3d);
        assert_eq!(l.tiles().len(), 2);

        // viewer_show updates & focuses existing tile of that kind, else replaces focused
        let l = viewer_show(&l, PageKind::Schematic, "sub/sheet.kicad_sch".into(), None).unwrap();
        assert_eq!(l.focus.as_deref(), Some("t2"));
        assert_eq!(l.tile("t2").unwrap().source, "sub/sheet.kicad_sch");
        let l = viewer_show(&l, PageKind::Mech, "mech/case.step".into(), None).unwrap();
        assert_eq!(l.focus.as_deref(), Some("t2"));
        assert_eq!(l.tile("t2").unwrap().kind, PageKind::Mech);
        assert_eq!(l.tile("t2").unwrap().source, "mech/case.step");
    }

    #[test]
    fn serde_round_trip_and_unknown_fields_tolerated() {
        let l = derive_layout(&DeriveInput {
            root: "/proj",
            open: "split",
            default_pcb: "/proj/main.kicad_pcb",
            default_sch: "/proj/main.kicad_sch",
            ..Default::default()
        });
        let raw = serde_json::to_string(&l).unwrap();
        let back: Layout = serde_json::from_str(&raw).unwrap();
        assert_eq!(l, back);

        // Unknown fields on Layout, Split, Tile, Overlay, TileVersion are tolerated
        let with_extra = r#"{
            "version": 1,
            "futureField": 42,
            "focus": "t1",
            "maximized": null,
            "root": {
                "type": "tile",
                "id": "t1",
                "kind": "board",
                "source": "main.kicad_pcb",
                "kicad": true,
                "version": {"id": "a1b2c3d4", "label": "a1b2c3d4", "extra": "ok"},
                "overlay": {"kind": "gerbers", "source": "mfg/rev1", "mode": "diff", "opacity": 0.5}
            }
        }"#;
        let parsed: Layout = serde_json::from_str(with_extra).unwrap();
        check_invariants(&parsed).unwrap();
        assert_eq!(parsed.tile("t1").unwrap().source, "main.kicad_pcb");
        assert_eq!(parsed.tile("t1").unwrap().version.as_ref().unwrap().id, "a1b2c3d4");
        assert_eq!(parsed.tile("t1").unwrap().overlay.as_ref().unwrap().kind, PageKind::Gerbers);
    }

    #[test]
    fn random_sequence_of_operations_preserves_invariants() {
        let mut seed: u64 = 0x5EED_1317_CAFE_BABEu64;
        let mut next_u64 = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            seed
        };
        let kinds = [
            PageKind::Board,
            PageKind::Schematic,
            PageKind::Solid3d,
            PageKind::Gerbers,
            PageKind::Mech,
            PageKind::Files,
            PageKind::Review,
            PageKind::Browser,
        ];
        let sides = [Side::Left, Side::Right, Side::Top, Side::Bottom];
        let mut layout = Layout::empty();
        for step in 0..600 {
            let op = next_u64() % 11;
            let tiles: Vec<TileId> = layout.tiles().iter().map(|t| t.id.clone()).collect();
            let locked = if !tiles.is_empty() && next_u64() % 5 == 0 {
                Some(tiles[(next_u64() as usize) % tiles.len()].clone())
            } else {
                None
            };
            let locked_ref = locked.as_deref();
            let candidate = match op {
                0 | 1 => {
                    // add
                    let k = kinds[(next_u64() as usize) % kinds.len()];
                    let s = sides[(next_u64() as usize) % sides.len()];
                    let target = if tiles.is_empty() { None } else { Some(tiles[(next_u64() as usize) % tiles.len()].as_str()) };
                    let id = layout.next_tile_id();
                    add(&layout, target, s, tile(&id, k, "dir/file.kicad_pcb")).ok()
                }
                2 if tiles.len() >= 2 => {
                    let a = &tiles[(next_u64() as usize) % tiles.len()];
                    let b = &tiles[(next_u64() as usize) % tiles.len()];
                    let s = sides[(next_u64() as usize) % sides.len()];
                    move_tile(&layout, a, b, s, locked_ref).ok()
                }
                3 if tiles.len() >= 2 => {
                    let a = &tiles[(next_u64() as usize) % tiles.len()];
                    let b = &tiles[(next_u64() as usize) % tiles.len()];
                    swap(&layout, a, b, locked_ref).ok()
                }
                4 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    close(&layout, t, locked_ref).ok()
                }
                5 => {
                    let sid = format!("s{}", (next_u64() % 6) + 1);
                    let r = ((next_u64() % 100) as f32) / 100.0;
                    let kt = kicad_tile(&layout, |_| true);
                    resize(&layout, &sid, r, (1400.0, 900.0), default_min_for(kt.as_deref())).ok()
                }
                6 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    maximize(&layout, t, locked_ref).ok()
                }
                7 => Some(restore(&layout)),
                8 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    let ov = if next_u64() % 2 == 0 {
                        Some(Overlay { kind: PageKind::Gerbers, source: "mfg/r1".into(), version: None, mode: OverlayMode::Diff })
                    } else {
                        None
                    };
                    overlay(&layout, t, ov, locked_ref).ok()
                }
                9 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    let s = sides[(next_u64() as usize) % sides.len()];
                    split_overlay(&layout, t, s, None, locked_ref).ok()
                }
                10 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    let k = kinds[(next_u64() as usize) % kinds.len()];
                    retarget(&layout, t, k, "other/file.kicad_sch".into(), locked_ref).ok()
                }
                _ => None,
            };
            if let Some(next) = candidate {
                layout = next;
            }
            check_invariants(&layout).unwrap_or_else(|e| panic!("step {step} violated invariant: {e}\n{layout:#?}"));
        }
    }

    #[test]
    fn plan_watches_on_split_preset_matches_watch_split_exactly() {
        let root = "/proj";
        let ctx = WatchContext {
            root: root.into(),
            default_pcb: "/proj/main.kicad_pcb".into(),
            default_sch: "/proj/main.kicad_sch".into(),
            git_dir: "/proj/.git".into(),
        };

        // Case 1: default project board + root schematic
        let split_default = derive_layout(&DeriveInput {
            root,
            open: "split",
            default_pcb: &ctx.default_pcb,
            default_sch: &ctx.default_sch,
            ..Default::default()
        });
        check_invariants(&split_default).unwrap();
        let specs = plan_watches(&split_default, &ctx);
        assert_eq!(
            specs,
            vec![
                WatchSpec { kind: 1, key: watch_key(1, root, ""), root: root.into(), path: "/proj/main.kicad_sch".into() },
                WatchSpec { kind: 0, key: watch_key(0, root, ""), root: root.into(), path: "/proj/main.kicad_pcb".into() },
                WatchSpec { kind: 2, key: watch_key(2, root, ""), root: root.into(), path: "/proj/main.kicad_pcb".into() },
            ]
        );

        // Case 2: picked board from Boards menu (`sub/alt.kicad_pcb`)
        let split_picked = derive_layout(&DeriveInput {
            root,
            open: "split",
            picked_board: "sub/alt.kicad_pcb",
            default_pcb: &ctx.default_pcb,
            default_sch: &ctx.default_sch,
            ..Default::default()
        });
        check_invariants(&split_picked).unwrap();
        let specs_picked = plan_watches(&split_picked, &ctx);
        assert_eq!(
            specs_picked,
            vec![
                WatchSpec {
                    kind: 1,
                    key: watch_key(1, root, "/proj/sub/alt.kicad_sch"),
                    root: root.into(),
                    path: "/proj/sub/alt.kicad_sch".into(),
                },
                WatchSpec {
                    kind: 0,
                    key: watch_key(0, root, "/proj/sub/alt.kicad_pcb"),
                    root: root.into(),
                    path: "/proj/sub/alt.kicad_pcb".into(),
                },
                WatchSpec {
                    kind: 2,
                    key: watch_key(2, root, "/proj/sub/alt.kicad_pcb"),
                    root: root.into(),
                    path: "/proj/sub/alt.kicad_pcb".into(),
                },
            ]
        );

        // Case 3: default board + explicit schematic sub-sheet
        let split_sheet = derive_layout(&DeriveInput {
            root,
            open: "split",
            sheet: "/proj/power.kicad_sch",
            default_pcb: &ctx.default_pcb,
            default_sch: &ctx.default_sch,
            ..Default::default()
        });
        let specs_sheet = plan_watches(&split_sheet, &ctx);
        assert_eq!(
            specs_sheet,
            vec![
                WatchSpec {
                    kind: 1,
                    key: watch_key(1, root, "/proj/power.kicad_sch"),
                    root: root.into(),
                    path: "/proj/power.kicad_sch".into(),
                },
                WatchSpec { kind: 0, key: watch_key(0, root, ""), root: root.into(), path: "/proj/main.kicad_pcb".into() },
                WatchSpec { kind: 2, key: watch_key(2, root, ""), root: root.into(), path: "/proj/main.kicad_pcb".into() },
            ]
        );
    }

    #[test]
    fn derive_layout_four_canonical_shapes_have_no_absolute_paths_or_raw_git() {
        let root = "/proj";
        // 1. One page
        let one = derive_layout(&DeriveInput {
            root,
            open: "board",
            default_pcb: "/proj/main.kicad_pcb",
            ..Default::default()
        });
        check_invariants(&one).unwrap();
        assert_eq!(one.tile("t1").unwrap().kind, PageKind::Board);
        assert_eq!(one.tile("t1").unwrap().source, "main.kicad_pcb");

        // 2. Split
        let split = derive_layout(&DeriveInput {
            root,
            open: "split",
            default_pcb: "/proj/main.kicad_pcb",
            default_sch: "/proj/main.kicad_sch",
            ..Default::default()
        });
        check_invariants(&split).unwrap();
        assert_eq!(split.tiles().len(), 3);

        // 3. Gerbers vs board
        let gcmp = gerber::cmp_path("/proj/mfg/rev1", "/proj/main.kicad_pcb");
        let gvb = derive_layout(&DeriveInput {
            root,
            open: "board",
            gerbers: true,
            gerbers_vs_board: true,
            file: &gcmp,
            default_pcb: "/proj/main.kicad_pcb",
            ..Default::default()
        });
        check_invariants(&gvb).unwrap();
        let gvb_tile = gvb.tile("t1").unwrap();
        assert_eq!(gvb_tile.kind, PageKind::Board);
        assert_eq!(gvb_tile.source, "main.kicad_pcb");
        assert_eq!(gvb_tile.overlay.as_ref().unwrap().kind, PageKind::Gerbers);
        assert_eq!(gvb_tile.overlay.as_ref().unwrap().source, "mfg/rev1");
        assert_eq!(gvb_tile.overlay.as_ref().unwrap().mode, OverlayMode::Diff);

        // 4. History comparison
        let hist = derive_layout(&DeriveInput {
            root,
            open: "board",
            file: "diff\ngit\t/proj/.git\t1111aaaa9999\t20\t/proj/main.kicad_pcb\ngit\t/proj/.git\t2222bbbb8888\t20\t/proj/main.kicad_pcb\n0",
            default_pcb: "/proj/main.kicad_pcb",
            ..Default::default()
        });
        check_invariants(&hist).unwrap();
        let ht = hist.tile("t1").unwrap();
        assert_eq!(ht.source, "main.kicad_pcb");
        assert_eq!(ht.version.as_ref().unwrap().id, "2222bbbb");
        let hov = ht.overlay.as_ref().unwrap();
        assert_eq!(hov.source, "main.kicad_pcb");
        assert_eq!(hov.version.as_ref().unwrap().id, "1111aaaa");
        assert_eq!(hov.mode, OverlayMode::Diff);

        // None of the serialized JSON strings contain `/proj` or `git\t` or `gcmp` or `diff\n`
        for l in [&one, &split, &gvb, &hist] {
            let s = serde_json::to_string(l).unwrap();
            assert!(!s.contains("/proj"), "leaked abs path: {s}");
            assert!(!s.contains("git\\t"), "leaked git wire: {s}");
            assert!(!s.contains("gerbercmp"), "leaked gerbercmp wire: {s}");
            assert!(!s.contains("\"kicad\""), "unexpected kicad flag in tile JSON: {s}");
        }
    }
}
