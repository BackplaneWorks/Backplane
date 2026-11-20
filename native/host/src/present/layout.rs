//! Hub-owned docking/tiling layout model for the viewer pane (pure data & operations).
//!
//! Layout state (`@view.layout:<sel>`) is **per session (per client)**, matching today's `@view:<sel>`:
//! each connected window owns its own tile tree, while per-project memory (`layout:<root>`, PR 2b)
//! restores the last settled layout when a session opens the viewer for a thread of that project.
//! Whether a design tile hosts the embedded KiCad editor is a client-session decision (`design_mode`
//! + `holder`), never a property of `Tile`; [`kicad_tile`] picks the single eligible tile in a fixed,
//! deterministic order.

use crate::files::{kicad::{gerber, project}, rv};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;

pub const LAYOUT_VERSION: u32 = 1;
pub const MIN_TILE_PX: f32 = 180.0;
pub const MIN_KICAD_W_PX: f32 = 380.0;
pub const MIN_KICAD_H_PX: f32 = 300.0;

pub type TileId = String;
pub type SplitId = String;

fn default_version() -> u32 { LAYOUT_VERSION }
fn default_overlay_mode() -> OverlayMode { OverlayMode::Diff }
fn is_false(b: &bool) -> bool { !*b }

fn watch_key(kind: u32, root: &str, path: &str) -> String {
    format!("{kind}|{root}|{path}")
}

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

    /// Wire / rail identifier string for this page kind.
    pub fn as_str(self) -> &'static str {
        match self {
            PageKind::Board => "board",
            PageKind::Schematic => "schematic",
            PageKind::Solid3d => "3d",
            PageKind::Gerbers => "gerbers",
            PageKind::Mech => "mech",
            PageKind::Files => "files",
            PageKind::Diff => "diff",
            PageKind::Review => "review",
            PageKind::Browser => "browser",
            PageKind::Simulation => "simulation",
        }
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

    /// Map a KiCad embedded pane frame name (matching `native/gpui/src/viewers/kicad_pane.rs` `TABS`:
    /// `PCB`, `SCH`, `PCB_3D`, `SYMBOL_EDITOR`, `FOOTPRINT_EDITOR`, `GERBVIEW`, `SIMULATOR`)
    /// to its owning `PageKind`.
    pub fn from_kicad_frame(frame: &str) -> Option<Self> {
        match frame.to_ascii_uppercase().as_str() {
            "PCB" | "BOARD" | "FOOTPRINT_EDITOR" | "FOOTPRINTS" | "FP" => Some(PageKind::Board),
            "SCH" | "SCHEMATIC" | "SYMBOL_EDITOR" | "SYMBOLS" | "SYM" => Some(PageKind::Schematic),
            "PCB_3D" | "3D" | "3DVIEW" => Some(PageKind::Solid3d),
            "SIMULATOR" | "SIMULATION" | "SIM" => Some(PageKind::Simulation),
            "GERBVIEW" | "GERBERS" | "GERBER" => Some(PageKind::Gerbers),
            _ => None,
        }
    }
}

/// Authorised object store and repository top directory for one hub-issued `source_id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionSource {
    pub git_dir: String,
    pub repo_top: String,
}

/// Pure resolver mapping an opaque, hub-issued `source_id` to its authorised `(git_dir, repo_top)`.
/// Unknown or ambiguous ids return `None` and cause watch planning to fail explicitly.
pub trait VersionResolver {
    fn resolve(&self, source_id: &str) -> Option<&VersionSource>;
}

/// Deterministic opaque `source_id` for a `(git_dir, repo_top)` pair (never contains any path).
pub fn opaque_source_id(git_dir: &str, repo_top: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in git_dir.as_bytes().iter().chain([0xffu8].iter()).chain(repo_top.as_bytes()) {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("vsrc-{h:016x}")
}

/// Map-backed [`VersionResolver`]. Registering two different `(git_dir, repo_top)` values under
/// the same `source_id` marks that id ambiguous (`None`), so resolution fails explicitly.
#[derive(Clone, Debug, Default)]
pub struct VersionSourceMap {
    entries: BTreeMap<String, Option<VersionSource>>,
}

impl VersionSourceMap {
    pub fn new() -> Self { Self::default() }

    /// Insert a mapping for an explicit `source_id`. Conflicting registrations mark it ambiguous.
    pub fn insert(&mut self, source_id: impl Into<String>, src: VersionSource) {
        let key = source_id.into();
        match self.entries.get_mut(&key) {
            None => { self.entries.insert(key, Some(src)); }
            Some(Some(existing)) if *existing == src => {}
            Some(slot) => { *slot = None; }
        }
    }

    /// Register `(git_dir, repo_top)` under its deterministic opaque `source_id` and return that id.
    pub fn register(&mut self, git_dir: impl Into<String>, repo_top: impl Into<String>) -> String {
        let gd = git_dir.into();
        let top = repo_top.into();
        let id = opaque_source_id(&gd, &top);
        self.insert(id.clone(), VersionSource { git_dir: gd, repo_top: top });
        id
    }
}

impl VersionResolver for VersionSourceMap {
    fn resolve(&self, source_id: &str) -> Option<&VersionSource> {
        self.entries.get(source_id).and_then(|v| v.as_ref())
    }
}

/// Project/thread-scoped version reference for a historical tile or overlay.
///
/// - `source_id` is an opaque, hub-issued id mapped by [`VersionResolver`] to an authorised object
///   store and repository top (never a filesystem path).
/// - `id` is a full git object id (40-char SHA-1 or 64-char SHA-256 hex).
/// - `label` is a short display label (never a path or wire token).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileVersion {
    pub source_id: String,
    pub id: String,
    pub label: String,
}

impl TileVersion {
    /// True if both versions name the same commit in the same source repository (ignoring display label).
    pub fn same_commit(&self, other: &Self) -> bool {
        self.source_id == other.source_id && self.id.eq_ignore_ascii_case(&other.id)
    }
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
    /// Optional layer-diff bitmask preserved from `diff\n<old>\n<new>\n<bits>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_bits: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    pub id: TileId,
    pub kind: PageKind,
    #[serde(default)]
    pub source: String,
    /// True when `source` was explicitly selected (e.g. picked in the Boards menu or opened as a
    /// child sheet), so watch planning keys by the explicit path rather than collapsing to `""`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub explicit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<TileVersion>,
    /// True when a historical (`git\t` or `diff\n`) input was malformed or had a non-full object id:
    /// prevents falling back to the live file and forces watch planning to fail explicitly.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unresolved_version: bool,
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
            explicit: bool,
            #[serde(default)]
            version: Option<TileVersion>,
            #[serde(default)]
            unresolved_version: bool,
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
                explicit: w.explicit,
                version: w.version,
                unresolved_version: w.unresolved_version,
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

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Layout {
    pub version: u32,
    pub root: Option<Node>,
    pub focus: Option<TileId>,
    pub maximized: Option<TileId>,
}

/// Deserializing a [`Layout`] tolerates unknown fields and validates all structural invariants
/// (rejecting dangling focus/maximized, invalid sources, duplicate IDs, incompatible overlays, etc.).
impl<'de> Deserialize<'de> for Layout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct LayoutWire {
            #[serde(default = "default_version")]
            version: u32,
            #[serde(default)]
            root: Option<Node>,
            #[serde(default)]
            focus: Option<TileId>,
            #[serde(default)]
            maximized: Option<TileId>,
        }
        let w = LayoutWire::deserialize(deserializer)?;
        let layout = Layout {
            version: w.version,
            root: w.root,
            focus: w.focus,
            maximized: w.maximized,
        };
        check_invariants(&layout).map_err(serde::de::Error::custom)?;
        Ok(layout)
    }
}

impl Default for Layout {
    fn default() -> Self { Self::empty() }
}

/// Unit-square rectangle `[0, 1] × [0, 1]` occupied by a visible tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Refusal {
    /// Operation would hide, move, resize, close, swap, retarget, or cover a locked tile (e.g. KiCad modal open).
    Locked,
    /// Cannot open Simulation beside an existing KiCad tile (one KiCad host per process).
    OneKicad,
    /// Available bounds are too small to satisfy the subtree minimum sizes.
    TooSmall,
    /// Target tile or split ID does not exist in the layout.
    NotFound,
    /// Source and destination tile IDs are identical.
    SameTile,
    /// Target tile has no overlay to split out.
    NoOverlay,
    /// Overlay pair is not comparable (only Board+Gerbers, or same Board/Schematic source with distinct commits).
    IncompatibleOverlay,
    /// Source path is not relative to the project root or contains raw internal tokens.
    InvalidSource,
    /// Tile ID already exists in the layout.
    DuplicateId,
    /// Split ratio is not finite.
    InvalidRatio,
    /// Unknown KiCad frame name.
    UnknownFrame,
    /// Malformed action payload (invalid JSON, kind, side, mode, or missing required field).
    InvalidInput,
}

impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Refusal::Locked => "locked",
            Refusal::OneKicad => "oneKicad",
            Refusal::TooSmall => "tooSmall",
            Refusal::NotFound => "notFound",
            Refusal::SameTile => "sameTile",
            Refusal::NoOverlay => "noOverlay",
            Refusal::IncompatibleOverlay => "incompatibleOverlay",
            Refusal::InvalidSource => "invalidSource",
            Refusal::DuplicateId => "duplicateId",
            Refusal::InvalidRatio => "invalidRatio",
            Refusal::UnknownFrame => "unknownFrame",
            Refusal::InvalidInput => "invalidInput",
        }
    }

    /// Catalog key in `i18n/*.msg` for the header refusal banner.
    pub fn i18n_key(self) -> &'static str {
        match self {
            Refusal::Locked => "viewer.layout.refusal.locked",
            Refusal::OneKicad => "viewer.layout.refusal.onekicad",
            Refusal::TooSmall => "viewer.layout.refusal.toosmall",
            Refusal::NotFound => "viewer.layout.refusal.notfound",
            Refusal::SameTile => "viewer.layout.refusal.sametile",
            Refusal::NoOverlay => "viewer.layout.refusal.nooverlay",
            Refusal::IncompatibleOverlay => "viewer.layout.refusal.incompatibleoverlay",
            Refusal::InvalidSource => "viewer.layout.refusal.invalidsource",
            Refusal::DuplicateId => "viewer.layout.refusal.duplicateid",
            Refusal::InvalidRatio => "viewer.layout.refusal.invalidratio",
            Refusal::UnknownFrame => "viewer.layout.refusal.unknownframe",
            Refusal::InvalidInput => "viewer.layout.refusal.invalidinput",
        }
    }

    /// English default copy from `i18n/en.msg`.
    pub fn text(self) -> &'static str {
        self.text_for("en")
    }

    /// Catalog lookup from compile-time-embedded `i18n/{en,es,ja}.msg` for the session's locale (no runtime file reads, no duplicate table).
    pub fn text_for(self, locale: &str) -> &'static str {
        fn parse_msg(src: &'static str) -> HashMap<&'static str, &'static str> {
            src.lines()
                .filter_map(|l| {
                    let t = l.trim();
                    if t.is_empty() || t.starts_with('#') { return None; }
                    let (k, v) = t.split_once('=')?;
                    Some((k.trim(), v.trim()))
                })
                .collect()
        }
        static EN: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| parse_msg(include_str!("../../../../i18n/en.msg")));
        static ES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| parse_msg(include_str!("../../../../i18n/es.msg")));
        static JA: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| parse_msg(include_str!("../../../../i18n/ja.msg")));
        let key = self.i18n_key();
        let lang = locale.split(['-', '_']).next().unwrap_or("en");
        let table = match lang {
            "ja" => &*JA,
            "es" => &*ES,
            _ => &*EN,
        };
        table.get(key).copied().or_else(|| EN.get(key).copied()).unwrap_or(self.code())
    }
}

/// Result of [`split_overlay`], preserving `mode`, `layer_bits`, `prev_focus`, and `prev_maximized`
/// so [`unsplit_overlay`] / [`merge_overlay`] is an exact inverse.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitOverlayOut {
    pub layout: Layout,
    pub overlay_tile_id: TileId,
    pub mode: OverlayMode,
    pub layer_bits: Option<u32>,
    pub prev_focus: Option<TileId>,
    pub prev_maximized: Option<TileId>,
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

/// Only full git object ids (40-char SHA-1 or 64-char SHA-256 hex) are accepted.
pub fn is_full_object_id(c: &str) -> bool {
    matches!(c.len(), 40 | 64) && c.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_clean_source_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn is_clean_version(v: &TileVersion) -> bool {
    is_clean_source_id(&v.source_id)
        && is_full_object_id(&v.id)
        && !v.label.is_empty()
        && !v.label.contains('\t')
        && !v.label.contains('\n')
        && !v.label.contains('\r')
        && !v.label.contains('/')
        && !v.label.contains('\\')
        && !v.label.contains(':')
}

fn versions_same_commit(a: Option<&TileVersion>, b: Option<&TileVersion>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x.same_commit(y),
        (None, None) => true,
        _ => false,
    }
}

/// Whether `ov` is a valid comparable overlay on `tile`:
/// - Board + Gerbers (fabrication output compared against a board), OR
/// - Same design kind (`Board` or `Schematic`) and same `source` with distinct commits
///   (compared by `(source_id, id)`, ignoring display label; at least one side versioned).
pub fn overlay_compatible(tile: &Tile, ov: &Overlay) -> bool {
    if tile.kind == PageKind::Board && ov.kind == PageKind::Gerbers {
        return tile.version.is_none() && ov.version.is_none();
    }
    matches!(tile.kind, PageKind::Board | PageKind::Schematic)
        && ov.kind == tile.kind
        && ov.source == tile.source
        && !versions_same_commit(tile.version.as_ref(), ov.version.as_ref())
        && (tile.version.is_some() || ov.version.is_some())
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
    ///
    /// IDs of closed tiles are reusable: the lowest unused `tN` is returned.
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
    ///
    /// IDs of collapsed splits are reusable: the lowest unused `sN` is returned.
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
        if !overlay_compatible(tile, ov) {
            return Err(Refusal::IncompatibleOverlay);
        }
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

fn node_contains_tile(node: &Node, id: &str) -> bool {
    match node {
        Node::Tile(t) => t.id == id,
        Node::Split { a, b, .. } => node_contains_tile(a, id) || node_contains_tile(b, id),
    }
}

fn node_contains_split(node: &Node, id: &str) -> bool {
    match node {
        Node::Tile(_) => false,
        Node::Split { id: sid, a, b, .. } => sid == id || node_contains_split(a, id) || node_contains_split(b, id),
    }
}

fn split_is_ancestor_of(node: &Node, split_id: &str, tile_id: &str) -> bool {
    match node {
        Node::Tile(_) => false,
        Node::Split { id, a, b, .. } => {
            if id == split_id {
                node_contains_tile(a, tile_id) || node_contains_tile(b, tile_id)
            } else {
                split_is_ancestor_of(a, split_id, tile_id) || split_is_ancestor_of(b, split_id, tile_id)
            }
        }
    }
}

fn first_tile_id(node: &Node) -> TileId {
    match node {
        Node::Tile(t) => t.id.clone(),
        Node::Split { a, .. } => first_tile_id(a),
    }
}

/// Compute the visible unit-square rectangle `[0, 1] × [0, 1]` of `target` in `layout`.
/// Returns `None` if `target` is absent or covered by another maximized tile.
pub fn tile_rect(layout: &Layout, target: &str) -> Option<Rect> {
    if !layout.contains_tile(target) {
        return None;
    }
    if let Some(mid) = &layout.maximized {
        return if mid == target {
            Some(Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 })
        } else {
            None
        };
    }
    let root = layout.root.as_ref()?;
    node_tile_rect(root, target, Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 })
}

fn node_tile_rect(node: &Node, target: &str, r: Rect) -> Option<Rect> {
    match node {
        Node::Tile(t) => (t.id == target).then_some(r),
        Node::Split { axis, ratio, a, b, .. } => {
            let (ra, rb) = match axis {
                Axis::Row => (
                    Rect { x: r.x, y: r.y, w: r.w * ratio, h: r.h },
                    Rect { x: r.x + r.w * ratio, y: r.y, w: r.w * (1.0 - ratio), h: r.h },
                ),
                Axis::Col => (
                    Rect { x: r.x, y: r.y, w: r.w, h: r.h * ratio },
                    Rect { x: r.x, y: r.y + r.h * ratio, w: r.w, h: r.h * (1.0 - ratio) },
                ),
            };
            node_tile_rect(a, target, ra).or_else(|| node_tile_rect(b, target, rb))
        }
    }
}

/// Enforce the tile lock against the resulting layout with **exact** (zero-epsilon) rect equality:
/// if `locked` names a tile present in `before`, it must remain present with identical contents
/// and identical visible unit-square rect in `after`.
fn check_lock_preserved(before: &Layout, after: &Layout, locked: Option<&str>) -> Result<(), Refusal> {
    let Some(lid) = locked else {
        return Ok(());
    };
    let Some(before_tile) = before.tile(lid) else {
        return Ok(());
    };
    let Some(after_tile) = after.tile(lid) else {
        return Err(Refusal::Locked);
    };
    if before_tile != after_tile {
        return Err(Refusal::Locked);
    }
    match (tile_rect(before, lid), tile_rect(after, lid)) {
        (Some(r1), Some(r2)) if r1 == r2 => Ok(()),
        _ => Err(Refusal::Locked),
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

/// Whether a rail JSON array (from `present::rail::rail`) allows pressing `kind`
/// (entry is present and not `"disabled"`).
pub fn rail_allows(rail_json: &Value, kind: PageKind) -> bool {
    let want = kind.as_str();
    rail_json
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["id"].as_str() == Some(want)))
        .is_some_and(|r| r["state"].as_str() != Some("disabled"))
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
/// if the layout is empty). Enforces `can_add` (`oneKicad`) and `locked` (refuses if adding would
/// move/resize/cover the locked tile).
pub fn add(
    layout: &Layout,
    target: Option<&str>,
    side: Side,
    tile: Tile,
    client_has_kicad_tile: bool,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    can_add(layout, tile.kind, client_has_kicad_tile)?;
    validate_tile(&tile)?;
    if layout.contains_tile(&tile.id) {
        return Err(Refusal::DuplicateId);
    }
    let next = match &layout.root {
        None => Layout::single(tile)?,
        Some(root) => {
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
            Layout {
                version: layout.version.max(LAYOUT_VERSION),
                root: Some(new_root),
                focus: Some(new_id),
                maximized: None,
            }
        }
    };
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
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

/// Move `src` beside `dst` on `side`. Enforces `locked` against the resulting tree placement.
pub fn move_tile(layout: &Layout, src: &str, dst: &str, side: Side, locked: Option<&str>) -> Result<Layout, Refusal> {
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
    let next = add(&intermediate, Some(dst), side, tile, false, None)?;
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
}

/// Swap two tiles in place. Enforces `locked` against the resulting tree placement.
pub fn swap(layout: &Layout, a_id: &str, b_id: &str, locked: Option<&str>) -> Result<Layout, Refusal> {
    if a_id == b_id {
        return Err(Refusal::SameTile);
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    let ta = layout.tile(a_id).ok_or(Refusal::NotFound)?.clone();
    let tb = layout.tile(b_id).ok_or(Refusal::NotFound)?.clone();
    let new_root = swap_nodes(root, a_id, &tb, b_id, &ta);
    let next = Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus: layout.focus.clone(),
        maximized: layout.maximized.clone(),
    };
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
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

/// Close `target` and collapse its parent split. Enforces `locked` against the resulting tree placement.
pub fn close(layout: &Layout, target: &str, locked: Option<&str>) -> Result<Layout, Refusal> {
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    let (_, rem) = remove_tile(root, target).ok_or(Refusal::NotFound)?;
    let next = match rem {
        None => Layout::empty(),
        Some(new_root) => {
            let focus = match &layout.focus {
                Some(f) if f != target => Some(f.clone()),
                _ => Some(first_tile_id(&new_root)),
            };
            let maximized = match &layout.maximized {
                Some(m) if m != target => Some(m.clone()),
                _ => None,
            };
            Layout {
                version: layout.version.max(LAYOUT_VERSION),
                root: Some(new_root),
                focus,
                maximized,
            }
        }
    };
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
}

/// Minimum `(width_px, height_px)` required for `node` so every descendant leaf meets `min_for`
/// at the subtree's exact stored split ratios (e.g. an 80/20 row of two 180 px leaves requires 900 px).
pub fn subtree_min<F: Fn(&Tile) -> (f32, f32)>(node: &Node, min_for: &F) -> (f32, f32) {
    match node {
        Node::Tile(t) => {
            let (w, h) = min_for(t);
            (w.max(1.0), h.max(1.0))
        }
        Node::Split { axis, ratio, a, b, .. } => {
            let (aw, ah) = subtree_min(a, min_for);
            let (bw, bh) = subtree_min(b, min_for);
            let r = *ratio;
            match axis {
                Axis::Row => ((aw / r).max(bw / (1.0 - r)), ah.max(bh)),
                Axis::Col => (aw.max(bw), (ah / r).max(bh / (1.0 - r))),
            }
        }
    }
}

/// Resize `split_id` to `desired_ratio`, clamped so every descendant leaf stays at or above the
/// minimum size returned by `min_for` inside `bounds = (width_px, height_px)` (accounting for
/// descendant split ratios).
///
/// - Refuses with [`Refusal::Locked`] if `split_id` is on the ancestor path to `locked` (zero drift).
/// - Refuses with [`Refusal::TooSmall`] if `bounds` cannot satisfy both subtrees' minimums.
pub fn resize<F: Fn(&Tile) -> (f32, f32)>(
    layout: &Layout,
    split_id: &str,
    desired_ratio: f32,
    bounds: (f32, f32),
    min_for: F,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    if !desired_ratio.is_finite() {
        return Err(Refusal::InvalidRatio);
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    if let Some(lid) = locked {
        if split_is_ancestor_of(root, split_id, lid) {
            return Err(Refusal::Locked);
        }
    }
    let new_root = resize_node(root, split_id, desired_ratio, bounds, &min_for)?;
    let next = Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus: layout.focus.clone(),
        maximized: layout.maximized.clone(),
    };
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
}

fn resize_node<F: Fn(&Tile) -> (f32, f32)>(
    node: &Node,
    split_id: &str,
    desired_ratio: f32,
    bounds: (f32, f32),
    min_for: &F,
) -> Result<Node, Refusal> {
    match node {
        Node::Tile(_) => Err(Refusal::NotFound),
        Node::Split { id, axis, ratio, a, b } => {
            if id == split_id {
                if !bounds.0.is_finite() || !bounds.1.is_finite() || bounds.0 <= 0.0 || bounds.1 <= 0.0 {
                    return Err(Refusal::TooSmall);
                }
                let (min_aw, min_ah) = subtree_min(a, min_for);
                let (min_bw, min_bh) = subtree_min(b, min_for);
                let (total, min_a, min_b, cross_total, cross_min) = match axis {
                    Axis::Row => (bounds.0, min_aw, min_bw, bounds.1, min_ah.max(min_bh)),
                    Axis::Col => (bounds.1, min_ah, min_bh, bounds.0, min_aw.max(min_bw)),
                };
                if cross_min > cross_total || min_a + min_b > total {
                    return Err(Refusal::TooSmall);
                }
                let lo = min_a / total;
                let hi = 1.0 - min_b / total;
                if lo > hi || lo <= 0.0 || hi >= 1.0 {
                    return Err(Refusal::TooSmall);
                }
                let clamped = desired_ratio.clamp(lo, hi);
                return Ok(Node::Split {
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
            match resize_node(a, split_id, desired_ratio, a_bounds, min_for) {
                Ok(na) => Ok(Node::Split { id: id.clone(), axis: *axis, ratio: *ratio, a: Box::new(na), b: b.clone() }),
                Err(Refusal::NotFound) => {
                    let nb = resize_node(b, split_id, desired_ratio, b_bounds, min_for)?;
                    Ok(Node::Split { id: id.clone(), axis: *axis, ratio: *ratio, a: a.clone(), b: Box::new(nb) })
                }
                Err(e) => Err(e),
            }
        }
    }
}

/// Focus `target` (restoring if another tile was maximized, respecting `locked`).
pub fn focus_tile(layout: &Layout, target: &str, locked: Option<&str>) -> Result<Layout, Refusal> {
    if !layout.contains_tile(target) {
        return Err(Refusal::NotFound);
    }
    let mut next = layout.clone();
    next.focus = Some(target.to_string());
    if next.maximized.as_deref() != Some(target) {
        next.maximized = None;
    }
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
}

/// Hub-resolved KiCad guard for one client session on a project.
///
/// - `is_holder`: true iff `client_id` matches the hub's KiCad `holder` for this project.
/// - `kicad_tile`: the tile hosting KiCad in this holder's session (`None` for any non-holder).
/// - `client_has_kicad`: derived by the hub (`is_holder && kicad_tile.is_some()`), never trusted
///   from a client payload, so Simulation beside an occupied KiCad pane is refused whatever the
///   client claims.
/// - `locked`: non-empty only for the holder (when a KiCad dialog is open in hub/session info,
///   or when the holder itself reports a lock); ignored for every non-holder session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KicadGuard {
    pub is_holder: bool,
    pub kicad_tile: Option<TileId>,
    pub client_has_kicad: bool,
    pub locked: Option<TileId>,
}

/// Resolve holder, open dialogs, KiCad tile, `client_has_kicad`, and `locked` ONLY from the
/// hub's own runtime snapshot (`hub_runtime`, `hub_dialog_open`) for the selected thread's project.
///
/// Never reads request-session `info`. No holder → no lock. A dialog of another project does not
/// lock this one. A session that is NOT the holder cannot set `locked`, `kicadTile`, or
/// `clientHasKicad` to any effect.
pub fn resolve_kicad_guard(
    layout: &Layout,
    hub_runtime: Option<&Value>,
    hub_dialog_open: bool,
    client_id: Option<u64>,
    client_locked: Option<&str>,
    client_kicad_tile: Option<&str>,
) -> KicadGuard {
    let holder: Option<u64> = hub_runtime
        .and_then(|rt| rt["holder"].as_u64())
        .filter(|n| *n > 0);
    let is_holder = client_id.is_some() && holder == client_id;
    if !is_holder {
        return KicadGuard::default();
    }
    let kt = client_kicad_tile
        .filter(|s| !s.is_empty())
        .and_then(|s| layout.tile(s))
        .filter(|t| t.kind.is_design())
        .map(|t| t.id.clone())
        .or_else(|| kicad_tile(layout, |_| true));
    let client_has_kicad = kt.is_some();
    let locked = if hub_dialog_open { kt.clone() } else { None };
    let _ = client_locked;
    KicadGuard {
        is_holder: true,
        kicad_tile: kt,
        client_has_kicad,
        locked,
    }
}

/// Maximize `target`, covering all other tiles without closing them.
///
/// Covering is **not** leaving: the desktop layout host hides a covered KiCad tile while keeping
/// its embedded surface attached, whereas closing or swapping is the leave. Enforces `locked`
/// against the resulting placement (if `locked` is present and would be covered or resized, returns
/// [`Refusal::Locked`]).
pub fn maximize(layout: &Layout, target: &str, locked: Option<&str>) -> Result<Layout, Refusal> {
    if !layout.contains_tile(target) {
        return Err(Refusal::NotFound);
    }
    let next = Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: layout.root.clone(),
        focus: Some(target.to_string()),
        maximized: Some(target.to_string()),
    };
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
}

/// Restore from a maximized tile back to the full split tree.
pub fn restore(layout: &Layout, locked: Option<&str>) -> Result<Layout, Refusal> {
    let next = Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: layout.root.clone(),
        focus: layout.focus.clone(),
        maximized: None,
    };
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
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
///
/// Enforces [`overlay_compatible`] (only Board+Gerbers, or same Board/Schematic source with
/// distinct commits) and `locked`.
pub fn overlay(layout: &Layout, target: &str, ov: Option<Overlay>, locked: Option<&str>) -> Result<Layout, Refusal> {
    let existing = layout.tile(target).ok_or(Refusal::NotFound)?;
    if let Some(o) = &ov {
        validate_overlay(o)?;
        if !overlay_compatible(existing, o) {
            return Err(Refusal::IncompatibleOverlay);
        }
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    let new_root = map_tile(root, target, &|t| {
        let mut nt = t.clone();
        nt.overlay = ov.clone();
        nt
    })
    .ok_or(Refusal::NotFound)?;
    let next = Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus: layout.focus.clone(),
        maximized: layout.maximized.clone(),
    };
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
}

/// Detach `target`'s overlay into its own tile beside `target` on `side`, preserving `mode`,
/// `layer_bits`, `prev_focus`, and `prev_maximized` in [`SplitOverlayOut`] so [`unsplit_overlay`]
/// is an exact inverse.
pub fn split_overlay(
    layout: &Layout,
    target: &str,
    side: Side,
    new_tile_id: Option<TileId>,
    locked: Option<&str>,
) -> Result<SplitOverlayOut, Refusal> {
    let t = layout.tile(target).ok_or(Refusal::NotFound)?;
    let ov = t.overlay.clone().ok_or(Refusal::NoOverlay)?;
    let id = new_tile_id.unwrap_or_else(|| layout.next_tile_id());
    let prev_focus = layout.focus.clone();
    let prev_maximized = layout.maximized.clone();
    let cleared = overlay(layout, target, None, locked)?;
    let new_tile = Tile {
        id: id.clone(),
        kind: ov.kind,
        source: ov.source,
        explicit: t.explicit,
        version: ov.version,
        unresolved_version: false,
        overlay: None,
    };
    let next = add(&cleared, Some(target), side, new_tile, false, locked)?;
    check_lock_preserved(layout, &next, locked)?;
    Ok(SplitOverlayOut {
        layout: next,
        overlay_tile_id: id,
        mode: ov.mode,
        layer_bits: ov.layer_bits,
        prev_focus,
        prev_maximized,
    })
}

/// Inverse of [`split_overlay`]: close `overlay_tile_id` and attach it back onto `base_id` as an
/// [`Overlay`] with `mode` and `layer_bits`, restoring `restore_focus` and `restore_maximized`
/// when provided and present in the resulting tree.
pub fn merge_overlay(
    layout: &Layout,
    base_id: &str,
    overlay_tile_id: &str,
    mode: OverlayMode,
    layer_bits: Option<u32>,
    restore_focus: Option<&str>,
    restore_maximized: Option<&str>,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    if base_id == overlay_tile_id {
        return Err(Refusal::SameTile);
    }
    let base = layout.tile(base_id).ok_or(Refusal::NotFound)?;
    let ov_tile = layout.tile(overlay_tile_id).ok_or(Refusal::NotFound)?.clone();
    let ov = Overlay {
        kind: ov_tile.kind,
        source: ov_tile.source,
        version: ov_tile.version,
        mode,
        layer_bits,
    };
    validate_overlay(&ov)?;
    if !overlay_compatible(base, &ov) {
        return Err(Refusal::IncompatibleOverlay);
    }
    let without = close(layout, overlay_tile_id, None)?;
    let mut merged = overlay(&without, base_id, Some(ov), None)?;
    merged.focus = match restore_focus {
        Some(f) if merged.contains_tile(f) => Some(f.to_string()),
        _ => Some(base_id.to_string()),
    };
    merged.maximized = match restore_maximized {
        Some(m) if merged.contains_tile(m) => Some(m.to_string()),
        _ => None,
    };
    check_lock_preserved(layout, &merged, locked)?;
    Ok(merged)
}

/// Exact inverse of [`split_overlay`], restoring mode, layer bits, focus, and maximized state.
pub fn unsplit_overlay(out: &SplitOverlayOut, base_id: &str, locked: Option<&str>) -> Result<Layout, Refusal> {
    merge_overlay(
        &out.layout,
        base_id,
        &out.overlay_tile_id,
        out.mode,
        out.layer_bits,
        out.prev_focus.as_deref(),
        out.prev_maximized.as_deref(),
        locked,
    )
}

/// Retarget an existing tile to a new `kind` and `source` (e.g. when the embedded KiCad tile
/// switches frames on KiCad's strip).
pub fn retarget(
    layout: &Layout,
    target: &str,
    kind: PageKind,
    source: String,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    if !is_relative_source(&source) {
        return Err(Refusal::InvalidSource);
    }
    let root = layout.root.as_ref().ok_or(Refusal::NotFound)?;
    let new_root = map_tile(root, target, &|t| Tile {
        id: t.id.clone(),
        kind,
        source: source.clone(),
        explicit: false,
        version: None,
        unresolved_version: false,
        overlay: None,
    })
    .ok_or(Refusal::NotFound)?;
    let next = Layout {
        version: layout.version.max(LAYOUT_VERSION),
        root: Some(new_root),
        focus: layout.focus.clone(),
        maximized: layout.maximized.clone(),
    };
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
}

/// Retarget a tile from a KiCad pane frame name (`PCB`, `SCH`, `PCB_3D`, `SYMBOL_EDITOR`,
/// `FOOTPRINT_EDITOR`, `GERBVIEW`, `SIMULATOR`).
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
/// - If `available(kind)` is `false` (disabled or omitted in the rail): no-op, returns `layout` unchanged.
/// - Focused tile has `kind`: close that tile.
/// - Another tile has `kind`: focus it (restoring if maximized on another tile).
/// - Absent `kind`: replace the focused tile, or open a 1-tile layout from idle.
pub fn rail_press<F: Fn(PageKind) -> bool>(
    layout: &Layout,
    kind: PageKind,
    default_source: String,
    available: F,
    locked: Option<&str>,
) -> Result<Layout, Refusal> {
    if !available(kind) {
        return Ok(layout.clone());
    }
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
        check_lock_preserved(layout, &next, locked)?;
        return Ok(next);
    }
    if let Some(fid) = layout.focus.as_deref() {
        return retarget(layout, fid, kind, default_source, locked);
    }
    Layout::single(Tile {
        id: layout.next_tile_id(),
        kind,
        source: default_source,
        explicit: false,
        version: None,
        unresolved_version: false,
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
    // a show that names a source makes its tile explicit: the tile draws the file named, not the project's default
    let named = !source.is_empty();
    let mark = |l: Layout, id: &str| -> Layout {
        if !named { return l }
        let root = l.root.as_ref().and_then(|r| map_tile(r, id, &|t| Tile { explicit: true, ..t.clone() }));
        Layout { root: root.or(l.root.clone()), ..l }
    };
    if let Some(existing) = layout.tiles().into_iter().find(|t| t.kind == kind) {
        let id = existing.id.clone();
        let updated = if named && (source != existing.source || !existing.explicit) {
            mark(retarget(layout, &id, kind, source, locked)?, &id)
        } else {
            layout.clone()
        };
        let mut next = updated;
        next.focus = Some(id.clone());
        if next.maximized.as_deref() != Some(id.as_str()) {
            next.maximized = None;
        }
        check_lock_preserved(layout, &next, locked)?;
        return Ok(next);
    }
    if let Some(fid) = layout.focus.as_deref() {
        let fid = fid.to_string();
        return Ok(mark(retarget(layout, &fid, kind, source, locked)?, &fid));
    }
    Layout::single(Tile {
        id: layout.next_tile_id(),
        kind,
        source,
        explicit: named,
        version: None,
        unresolved_version: false,
        overlay: None,
    })
}

/// An agent's Split show over a stored layout: never discards the person's tiles. The layout already holding a Board and a Schematic of the
/// source (a source of "" is any) is satisfied by focusing the Board; otherwise the missing one is added beside the focused tile (through
/// `can_add` and the lock); a refusal says why. `board` and `sch` are root-relative sources ("" for the project's own).
pub fn show_split(layout: &Layout, board: &str, sch: &str, client_has_kicad_tile: bool, locked: Option<&str>) -> Result<Layout, Refusal> {
    if !is_relative_source(board) || !is_relative_source(sch) {
        return Err(Refusal::InvalidSource);
    }
    let holds = |l: &Layout, kind: PageKind, src: &str| l.tiles().into_iter().find(|t| t.kind == kind && t.version.is_none() && t.overlay.is_none() && (src.is_empty() || t.source == src)).map(|t| t.id.clone());
    let mut next = layout.clone();
    for (kind, src) in [(PageKind::Board, board), (PageKind::Schematic, sch)] {
        if holds(&next, kind, src).is_some() { continue }
        can_add(&next, kind, client_has_kicad_tile)?;
        let tile = Tile { id: next.next_tile_id(), kind, source: src.to_string(), explicit: !src.is_empty(), version: None, unresolved_version: false, overlay: None };
        let anchor = next.focus.clone();
        next = add(&next, anchor.as_deref(), Side::Right, tile, client_has_kicad_tile, locked)?;
    }
    let id = holds(&next, PageKind::Board, board).ok_or(Refusal::NotFound)?;
    next.focus = Some(id.clone());
    // the pair is shown: a maximized tile (the Board, or any) is restored so the Schematic is visible beside it
    next.maximized = None;
    check_lock_preserved(layout, &next, locked)?;
    Ok(next)
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
                if !overlay_compatible(t, ov) {
                    return Err(format!("tile {:?} has incompatible overlay {:?}", t.id, ov));
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

/// Parse a hub `git\t<gitdir>\t<commit>\t<top_len>\t<file>` version string into
/// `(rel_source, TileVersion)` with an opaque `source_id` derived from `(gitdir, top)` and a
/// full 40- or 64-char hex object id.
pub fn parse_git_source(root: &str, raw: &str) -> Option<(String, TileVersion)> {
    let rest = raw.strip_prefix("git\t")?;
    let mut parts = rest.splitn(4, '\t');
    let gitdir = parts.next()?;
    let commit = parts.next()?;
    let top_len_str = parts.next()?;
    let file = parts.next()?;
    if !is_full_object_id(commit) || gitdir.is_empty() {
        return None;
    }
    let top_len = top_len_str.parse::<usize>().ok()?;
    let split_byte = file.char_indices().nth(top_len).map(|(i, _)| i)?;
    let repo_top = file.get(..split_byte)?;
    let tail = file.get(split_byte..)?;
    let rel_raw = tail.strip_prefix('/').or_else(|| tail.strip_prefix('\\'))?;
    let rel_norm = rel_raw.replace('\\', "/");
    let rel = if is_relative_source(&rel_norm) { rel_norm } else { sanitize_rel(root, file) };
    let source_id = opaque_source_id(gitdir, repo_top);
    let label: String = commit.chars().take(8).collect();
    Some((rel, TileVersion { source_id, id: commit.to_string(), label }))
}

/// Parse a hub `diff\n<old>\n<new>[\n<bits>]` comparison string into
/// `(new_rel, new_ver, old_rel, old_ver, layer_bits)`.
pub fn parse_diff_source(
    root: &str,
    raw: &str,
) -> Option<(String, Option<TileVersion>, String, Option<TileVersion>, Option<u32>)> {
    let rest = raw.strip_prefix("diff\n")?;
    let mut lines = rest.split('\n');
    let old_raw = lines.next()?;
    let new_raw = lines.next()?;
    let layer_bits = lines.next().and_then(|b| b.trim().parse::<u32>().ok());
    let parse_side = |side: &str| -> Option<(String, Option<TileVersion>)> {
        if side.starts_with("git\t") {
            let (r, v) = parse_git_source(root, side)?;
            Some((r, Some(v)))
        } else {
            Some((sanitize_rel(root, side), None))
        }
    };
    let (old_rel, old_ver) = parse_side(old_raw)?;
    let (new_rel, new_ver) = parse_side(new_raw)?;
    Some((new_rel, new_ver, old_rel, old_ver, layer_bits))
}

/// Inputs for deriving today's diagnostic `viewer.layout` from the current single-page viewer state.
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
    /// Resolved sibling schematic for `picked_board` (empty if none or unconfirmed).
    pub picked_sch: &'a str,
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
            explicit: false,
            version: None,
            unresolved_version: false,
            overlay: None,
        })
        .unwrap_or_else(|_| Layout::empty());
    }
    if inp.open.is_empty() {
        return Layout::empty();
    }
    if inp.open == "split" {
        let picked = !inp.picked_board.is_empty();
        let board_rel = if picked {
            sanitize_rel(inp.root, inp.picked_board)
        } else {
            sanitize_rel(inp.root, inp.default_pcb)
        };
        let sch_explicit = !inp.sheet.is_empty() || picked;
        let sch_rel = if !inp.sheet.is_empty() {
            sanitize_rel(inp.root, inp.sheet)
        } else if picked {
            sanitize_rel(inp.root, inp.picked_sch)
        } else {
            sanitize_rel(inp.root, inp.default_sch)
        };
        let include_sch = !sch_rel.is_empty() || (!picked && inp.default_sch.is_empty());
        let right = Node::Split {
            id: if include_sch { "s2".into() } else { "s1".into() },
            axis: Axis::Col,
            ratio: 0.5,
            a: Box::new(Node::Tile(Tile {
                id: "pcb".into(),
                kind: PageKind::Board,
                source: board_rel.clone(),
                explicit: picked,
                version: None,
                unresolved_version: false,
                overlay: None,
            })),
            b: Box::new(Node::Tile(Tile {
                id: "3d".into(),
                kind: PageKind::Solid3d,
                source: board_rel,
                explicit: picked,
                version: None,
                unresolved_version: false,
                overlay: None,
            })),
        };
        let root = if include_sch {
            Node::Split {
                id: "s1".into(),
                axis: Axis::Row,
                ratio: 0.5,
                a: Box::new(Node::Tile(Tile {
                    id: "sch".into(),
                    kind: PageKind::Schematic,
                    source: sch_rel,
                    explicit: sch_explicit,
                    version: None,
                    unresolved_version: false,
                    overlay: None,
                })),
                b: Box::new(right),
            }
        } else {
            right
        };
        return Layout {
            version: LAYOUT_VERSION,
            root: Some(root),
            focus: Some("pcb".into()),
            maximized: None,
        };
    }

    if inp.gerbers {
        let no_board = inp.file.starts_with("noboard\n");
        let raw_file = inp.file.strip_prefix("noboard\n").unwrap_or(inp.file);
        let set_raw = gerber::set_of(raw_file);
        let gerber_rel = if !set_raw.is_empty() {
            sanitize_rel(inp.root, set_raw)
        } else {
            sanitize_rel(inp.root, inp.default_gerber_set)
        };
        if inp.gerbers_vs_board {
            let board_rel = if no_board {
                String::new()
            } else {
                let board_raw = raw_file
                    .strip_prefix(gerber::CMP)
                    .and_then(|r| r.split_once('\n'))
                    .map(|(_, b)| b)
                    .unwrap_or("");
                if !board_raw.is_empty() {
                    sanitize_rel(inp.root, board_raw)
                } else if !inp.picked_board.is_empty() {
                    sanitize_rel(inp.root, inp.picked_board)
                } else {
                    sanitize_rel(inp.root, inp.default_pcb)
                }
            };
            return Layout::single(Tile {
                id: "t1".into(),
                kind: PageKind::Board,
                source: board_rel,
                explicit: false,
                version: None,
                unresolved_version: false,
                overlay: Some(Overlay {
                    kind: PageKind::Gerbers,
                    source: gerber_rel,
                    version: None,
                    mode: OverlayMode::Diff,
                    layer_bits: None,
                }),
            })
            .unwrap_or_else(|_| Layout::empty());
        }
        return Layout::single(Tile {
            id: "t1".into(),
            kind: PageKind::Gerbers,
            source: gerber_rel,
            explicit: false,
            version: None,
            unresolved_version: false,
            overlay: None,
        })
        .unwrap_or_else(|_| Layout::empty());
    }

    let Some(kind) = PageKind::from_page_str(inp.open) else {
        return Layout::empty();
    };

    // Historical inputs (`diff\n` or `git\t`) NEVER fall back to live content: malformed or
    // abbreviated-id inputs yield an `unresolved_version` tile instead of the live default PCB/SCH.
    if inp.file.starts_with("diff\n") {
        if let Some((new_rel, new_ver, old_rel, old_ver, layer_bits)) = parse_diff_source(inp.root, inp.file) {
            let ov = Overlay {
                kind,
                source: old_rel,
                version: old_ver,
                mode: OverlayMode::Diff,
                layer_bits,
            };
            let t = Tile {
                id: "t1".into(),
                kind,
                source: new_rel,
                explicit: false,
                version: new_ver,
                unresolved_version: false,
                overlay: Some(ov),
            };
            if let Ok(l) = Layout::single(t) {
                return l;
            }
        }
        return Layout::single(Tile {
            id: "t1".into(),
            kind,
            source: String::new(),
            explicit: false,
            version: None,
            unresolved_version: true,
            overlay: None,
        })
        .unwrap_or_else(|_| Layout::empty());
    }

    if inp.file.starts_with("git\t") {
        if let Some((rel, ver)) = parse_git_source(inp.root, inp.file) {
            if let Ok(l) = Layout::single(Tile {
                id: "t1".into(),
                kind,
                source: rel,
                explicit: false,
                version: Some(ver),
                unresolved_version: false,
                overlay: None,
            }) {
                return l;
            }
        }
        return Layout::single(Tile {
            id: "t1".into(),
            kind,
            source: String::new(),
            explicit: false,
            version: None,
            unresolved_version: true,
            overlay: None,
        })
        .unwrap_or_else(|_| Layout::empty());
    }

    let (source, explicit) = match kind {
        PageKind::Board | PageKind::Solid3d => {
            if !inp.file.is_empty() {
                (sanitize_rel(inp.root, inp.file), true)
            } else if !inp.picked_board.is_empty() {
                (sanitize_rel(inp.root, inp.picked_board), true)
            } else {
                (sanitize_rel(inp.root, inp.default_pcb), false)
            }
        }
        PageKind::Schematic => {
            if !inp.sheet.is_empty() {
                (sanitize_rel(inp.root, inp.sheet), true)
            } else if !inp.file.is_empty() {
                (sanitize_rel(inp.root, inp.file), true)
            } else if !inp.picked_board.is_empty() {
                (sanitize_rel(inp.root, inp.picked_sch), true)
            } else {
                (sanitize_rel(inp.root, inp.default_sch), false)
            }
        }
        PageKind::Mech => (sanitize_rel(inp.root, inp.mech_rel), false),
        PageKind::Files => {
            if !inp.file.is_empty() {
                (sanitize_rel(inp.root, inp.file), true)
            } else {
                (sanitize_rel(inp.root, inp.files_dir), false)
            }
        }
        PageKind::Gerbers | PageKind::Diff | PageKind::Review | PageKind::Browser | PageKind::Simulation => {
            (sanitize_rel(inp.root, inp.file), false)
        }
    };

    Layout::single(Tile {
        id: "t1".into(),
        kind,
        source,
        explicit,
        version: None,
        unresolved_version: false,
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
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchError {
    /// A versioned tile or overlay could not be resolved against the [`VersionResolver`];
    /// never falls back silently to watching the live working file or returning empty `Ok`.
    UnresolvedVersion {
        tile_id: TileId,
        source_id: String,
        id: String,
    },
    /// A versioned tile requires a non-empty project root to form its watch key.
    MissingRoot {
        tile_id: TileId,
    },
}

/// One `kicad.watch` subscription planned from a layout.
///
/// `path` is the exact request path passed to `kicad.watch` (`""` for the project default file,
/// or an absolute / encoded wire path otherwise), so `watch::key(spec.kind, &spec.root, &spec.path) == spec.key`.
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

/// Whether a tile's file (`join_root`: `C:\p/main.kicad_pcb` on Windows, as the live view names it) is the project's default one
/// (`kicad::project`: `C:\p\main.kicad_pcb`): the same file whichever separator each one uses.
fn is_default(abs: &str, default: &str) -> bool { !default.is_empty() && abs.replace('\\', "/") == default.replace('\\', "/") }

fn resolve_version(
    tile_id: &str,
    ver: &TileVersion,
    rel: &str,
    resolver: &dyn VersionResolver,
) -> Result<String, WatchError> {
    if rel.is_empty() || !is_full_object_id(&ver.id) {
        return Err(WatchError::UnresolvedVersion {
            tile_id: tile_id.to_string(),
            source_id: ver.source_id.clone(),
            id: ver.id.clone(),
        });
    }
    let Some(src) = resolver.resolve(&ver.source_id) else {
        return Err(WatchError::UnresolvedVersion {
            tile_id: tile_id.to_string(),
            source_id: ver.source_id.clone(),
            id: ver.id.clone(),
        });
    };
    if src.git_dir.is_empty() || src.repo_top.is_empty() {
        return Err(WatchError::UnresolvedVersion {
            tile_id: tile_id.to_string(),
            source_id: ver.source_id.clone(),
            id: ver.id.clone(),
        });
    }
    Ok(rv::hist_git(&src.git_dir, &ver.id, src.repo_top.trim_end_matches('/'), rel))
}

/// Plan the deduplicated list of `kicad.watch` subscriptions needed to draw all tiles in `layout`.
///
/// Pure function over `layout`, `ctx`, and `resolver`: does not touch `kicad.watch` runtime state.
/// Always resolves versioned tiles/overlays first (even if `ctx.root` is empty) and returns
/// `Err(WatchError::UnresolvedVersion)` on any unknown or ambiguous source id, never falling back
/// to the live working file.
pub fn plan_watches(
    layout: &Layout,
    ctx: &WatchContext,
    resolver: &dyn VersionResolver,
) -> Result<Vec<WatchSpec>, WatchError> {
    let mut specs = Vec::new();
    let mut seen = HashSet::new();
    for tile in layout.tiles() {
        for spec in plan_tile_watches(tile, ctx, resolver)? {
            if seen.insert(spec.key.clone()) {
                specs.push(spec);
            }
        }
    }
    Ok(specs)
}

fn plan_tile_watches(
    tile: &Tile,
    ctx: &WatchContext,
    resolver: &dyn VersionResolver,
) -> Result<Vec<WatchSpec>, WatchError> {
    if tile.unresolved_version {
        return Err(WatchError::UnresolvedVersion {
            tile_id: tile.id.clone(),
            source_id: String::new(),
            id: String::new(),
        });
    }
    let root = &ctx.root;

    // 1. Handle Board + Gerbers overlay (all modes: Diff, Old, New)
    if tile.kind == PageKind::Board {
        if let Some(ov) = &tile.overlay {
            if ov.kind == PageKind::Gerbers {
                if root.is_empty() {
                    return Ok(Vec::new());
                }
                return match ov.mode {
                    OverlayMode::Diff => {
                        if tile.source.is_empty() || ov.source.is_empty() {
                            return Ok(Vec::new());
                        }
                        let abs_set = join_root(root, &ov.source);
                        let abs_board = join_root(root, &tile.source);
                        let cmp = gerber::cmp_path(&abs_set, &abs_board);
                        Ok(vec![WatchSpec { kind: 0, key: watch_key(0, root, &cmp), root: root.clone(), path: cmp }])
                    }
                    OverlayMode::Old => {
                        if ov.source.is_empty() {
                            return Ok(Vec::new());
                        }
                        let abs_set = join_root(root, &ov.source);
                        Ok(vec![WatchSpec { kind: 0, key: watch_key(0, root, &abs_set), root: root.clone(), path: abs_set }])
                    }
                    OverlayMode::New => {
                        if tile.source.is_empty() {
                            return Ok(Vec::new());
                        }
                        let abs_board = join_root(root, &tile.source);
                        let req_path = if !tile.explicit && is_default(&abs_board, &ctx.default_pcb) { String::new() } else { abs_board };
                        Ok(vec![WatchSpec { kind: 0, key: watch_key(0, root, &req_path), root: root.clone(), path: req_path }])
                    }
                };
            }
        }
    }

    // 2. Handle versioned overlay on Board or Schematic (all modes: Diff, Old, New)
    if matches!(tile.kind, PageKind::Board | PageKind::Schematic) {
        if let Some(ov) = &tile.overlay {
            if ov.kind == tile.kind && (tile.version.is_some() || ov.version.is_some()) {
                let k = if tile.kind == PageKind::Board { 0 } else { 1 };
                let default_abs = if k == 0 { &ctx.default_pcb } else { &ctx.default_sch };
                let new_rel = if tile.source.is_empty() { sanitize_rel(root, default_abs) } else { tile.source.clone() };
                let old_rel = if ov.source.is_empty() { new_rel.clone() } else { ov.source.clone() };
                // Check root before selecting the overlay side so a versioned base with a live Old
                // overlay never returns Ok([]) on an empty root.
                if root.is_empty() {
                    if let Some(v) = &ov.version {
                        let _ = resolve_version(&tile.id, v, &old_rel, resolver)?;
                    }
                    if let Some(v) = &tile.version {
                        let _ = resolve_version(&tile.id, v, &new_rel, resolver)?;
                    }
                    return Err(WatchError::MissingRoot { tile_id: tile.id.clone() });
                }
                return match ov.mode {
                    OverlayMode::Diff => {
                        let old_spec = match &ov.version {
                            Some(v) => resolve_version(&tile.id, v, &old_rel, resolver)?,
                            None => join_root(root, &old_rel),
                        };
                        let new_spec = match &tile.version {
                            Some(v) => resolve_version(&tile.id, v, &new_rel, resolver)?,
                            None => join_root(root, &new_rel),
                        };
                        let mut diff_path = format!("diff\n{old_spec}\n{new_spec}");
                        if let Some(bits) = ov.layer_bits {
                            diff_path.push('\n');
                            diff_path.push_str(&bits.to_string());
                        }
                        Ok(vec![WatchSpec { kind: k, key: watch_key(k, root, &diff_path), root: root.clone(), path: diff_path }])
                    }
                    OverlayMode::Old => {
                        match &ov.version {
                            Some(v) => {
                                let p = resolve_version(&tile.id, v, &old_rel, resolver)?;
                                Ok(vec![WatchSpec { kind: k, key: watch_key(k, root, &p), root: root.clone(), path: p }])
                            }
                            None => {
                                let abs = join_root(root, &old_rel);
                                if abs.is_empty() { return Ok(Vec::new()); }
                                let req_path = if !tile.explicit && abs == *default_abs { String::new() } else { abs };
                                Ok(vec![WatchSpec { kind: k, key: watch_key(k, root, &req_path), root: root.clone(), path: req_path }])
                            }
                        }
                    }
                    OverlayMode::New => {
                        match &tile.version {
                            Some(v) => {
                                let p = resolve_version(&tile.id, v, &new_rel, resolver)?;
                                Ok(vec![WatchSpec { kind: k, key: watch_key(k, root, &p), root: root.clone(), path: p }])
                            }
                            None => {
                                let abs = join_root(root, &new_rel);
                                if abs.is_empty() { return Ok(Vec::new()); }
                                let req_path = if !tile.explicit && abs == *default_abs { String::new() } else { abs };
                                Ok(vec![WatchSpec { kind: k, key: watch_key(k, root, &req_path), root: root.clone(), path: req_path }])
                            }
                        }
                    }
                };
            }
        }
    }

    // 3. Single-source tiles (resolve version FIRST even when root is empty)
    if let Some(ver) = &tile.version {
        let default_abs = match tile.kind {
            PageKind::Schematic => &ctx.default_sch,
            _ => &ctx.default_pcb,
        };
        let rel = if tile.source.is_empty() { sanitize_rel(root, default_abs) } else { tile.source.clone() };
        let p = resolve_version(&tile.id, ver, &rel, resolver)?;
        if root.is_empty() {
            return Err(WatchError::MissingRoot { tile_id: tile.id.clone() });
        }
        return match tile.kind {
            PageKind::Board => Ok(vec![WatchSpec { kind: 0, key: watch_key(0, root, &p), root: root.clone(), path: p }]),
            PageKind::Schematic => Ok(vec![WatchSpec { kind: 1, key: watch_key(1, root, &p), root: root.clone(), path: p }]),
            PageKind::Solid3d => {
                // Versioned 3D tile resolves BOTH kind 2 and kind 0 against the versioned spec — NEVER the live board.
                let mut out = vec![WatchSpec { kind: 2, key: watch_key(2, root, &p), root: root.clone(), path: p.clone() }];
                if rel.ends_with(".kicad_pcb") {
                    out.push(WatchSpec { kind: 0, key: watch_key(0, root, &p), root: root.clone(), path: p });
                }
                Ok(out)
            }
            _ => Ok(Vec::new()),
        };
    }

    if root.is_empty() {
        return Ok(Vec::new());
    }

    match tile.kind {
        PageKind::Board => {
            let abs = if tile.source.is_empty() { ctx.default_pcb.clone() } else { join_root(root, &tile.source) };
            if abs.is_empty() {
                return Ok(Vec::new());
            }
            let req_path = if !tile.explicit && is_default(&abs, &ctx.default_pcb) { String::new() } else { abs };
            Ok(vec![WatchSpec { kind: 0, key: watch_key(0, root, &req_path), root: root.clone(), path: req_path }])
        }
        PageKind::Schematic => {
            let abs = if tile.source.is_empty() { ctx.default_sch.clone() } else { join_root(root, &tile.source) };
            if abs.is_empty() {
                return Ok(Vec::new());
            }
            let req_path = if !tile.explicit && is_default(&abs, &ctx.default_sch) { String::new() } else { abs };
            Ok(vec![WatchSpec { kind: 1, key: watch_key(1, root, &req_path), root: root.clone(), path: req_path }])
        }
        PageKind::Solid3d => {
            let abs = if tile.source.is_empty() { ctx.default_pcb.clone() } else { join_root(root, &tile.source) };
            if abs.is_empty() {
                return Ok(Vec::new());
            }
            let req_path = if !tile.explicit && is_default(&abs, &ctx.default_pcb) { String::new() } else { abs.clone() };
            let mut out = vec![WatchSpec { kind: 2, key: watch_key(2, root, &req_path), root: root.clone(), path: req_path.clone() }];
            if abs.ends_with(".kicad_pcb") {
                out.push(WatchSpec { kind: 0, key: watch_key(0, root, &req_path), root: root.clone(), path: req_path });
            }
            Ok(out)
        }
        PageKind::Gerbers => {
            let abs = join_root(root, &tile.source);
            if abs.is_empty() {
                return Ok(Vec::new());
            }
            Ok(vec![WatchSpec { kind: 0, key: watch_key(0, root, &abs), root: root.clone(), path: abs }])
        }
        PageKind::Mech => {
            let abs = join_root(root, &tile.source);
            if abs.is_empty() {
                return Ok(Vec::new());
            }
            Ok(vec![WatchSpec { kind: 2, key: watch_key(2, root, &abs), root: root.clone(), path: abs }])
        }
        PageKind::Files | PageKind::Diff | PageKind::Review | PageKind::Browser | PageKind::Simulation => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_A: &str = "1111aaaa99998888777766665555444433332222";
    const SHA_B: &str = "2222bbbb88889999000011112222333344445555";

    fn tile(id: &str, kind: PageKind, source: &str) -> Tile {
        Tile { id: id.into(), kind, source: source.into(), explicit: false, version: None, unresolved_version: false, overlay: None }
    }

    fn ver(source_id: &str, id: &str) -> TileVersion {
        TileVersion { source_id: source_id.into(), id: id.into(), label: id.chars().take(8).collect() }
    }

    #[test]
    fn kicad_tile_picks_focused_first_then_depth_first_and_at_most_one() {
        let mut l = Layout::single(tile("t1", PageKind::Schematic, "a.kicad_sch")).unwrap();
        l = add(&l, Some("t1"), Side::Right, tile("t2", PageKind::Board, "a.kicad_pcb"), false, None).unwrap();
        l = add(&l, Some("t2"), Side::Bottom, tile("t3", PageKind::Solid3d, "a.kicad_pcb"), false, None).unwrap();
        assert_eq!(kicad_tile(&l, |_| false), None);
        assert_eq!(kicad_tile(&l, |_| true).as_deref(), Some("t1"));
        l.focus = Some("t2".into());
        assert_eq!(kicad_tile(&l, |_| true).as_deref(), Some("t2"));
        assert_eq!(kicad_tile(&l, |t| t.id == "t1").as_deref(), Some("t1"));
        assert_eq!(kicad_tile(&l, |t| t.kind == PageKind::Solid3d), None);
    }

    #[test]
    fn retarget_maps_all_real_kicad_pane_tab_names() {
        let l = Layout::single(tile("k1", PageKind::Board, "main.kicad_pcb")).unwrap();
        for (frame, expected) in [
            ("PCB", PageKind::Board),
            ("SCH", PageKind::Schematic),
            ("PCB_3D", PageKind::Solid3d),
            ("SYMBOL_EDITOR", PageKind::Schematic),
            ("FOOTPRINT_EDITOR", PageKind::Board),
            ("GERBVIEW", PageKind::Gerbers),
            ("SIMULATOR", PageKind::Simulation),
        ] {
            let next = retarget_kicad_frame(&l, "k1", frame, "main.kicad_pcb".into(), None)
                .unwrap_or_else(|e| panic!("frame {frame} failed: {e:?}"));
            assert_eq!(next.tile("k1").unwrap().kind, expected, "frame={frame}");
        }
        assert_eq!(retarget_kicad_frame(&l, "k1", "UNKNOWN_TAB", "main.kicad_pcb".into(), None), Err(Refusal::UnknownFrame));
        assert_eq!(retarget(&l, "k1", PageKind::Schematic, "main.kicad_sch".into(), Some("k1")), Err(Refusal::Locked));
        assert_eq!(retarget(&l, "k1", PageKind::Schematic, "/proj/main.kicad_sch".into(), None), Err(Refusal::InvalidSource));
    }

    #[test]
    fn add_enforces_one_kicad_for_simulation_and_allows_beside_viewer_board() {
        let l = Layout::single(tile("t1", PageKind::Board, "main.kicad_pcb")).unwrap();
        assert_eq!(
            add(&l, Some("t1"), Side::Right, tile("t2", PageKind::Simulation, "main.kicad_sch"), true, None),
            Err(Refusal::OneKicad)
        );
        assert_eq!(Refusal::OneKicad.code(), "oneKicad");
        let ok = add(&l, Some("t1"), Side::Right, tile("t2", PageKind::Simulation, "main.kicad_sch"), false, None).unwrap();
        assert_eq!(ok.tile("t2").unwrap().kind, PageKind::Simulation);
    }

    #[test]
    fn resize_refuses_too_small_and_prevents_ancestor_drift_on_locked_tile() {
        let l = Layout::single(tile("k", PageKind::Board, "main.kicad_pcb")).unwrap();
        let l = add(&l, Some("k"), Side::Right, tile("v", PageKind::Schematic, "main.kicad_sch"), false, None).unwrap();

        // Counterexample: 500 px row with 380 px (KiCad) and 180 px minimums cannot fit -> Refusal::TooSmall
        assert_eq!(
            resize(&l, "s1", 0.5, (500.0, 800.0), default_min_for(Some("k")), None),
            Err(Refusal::TooSmall)
        );
        assert_eq!(Refusal::TooSmall.code(), "tooSmall");

        // Counterexample (c): 1000×100 row with a 380×300 KiCad tile must fail cross-axis check -> TooSmall
        assert_eq!(
            resize(&l, "s1", 0.5, (1000.0, 100.0), default_min_for(Some("k")), None),
            Err(Refusal::TooSmall)
        );

        // Counterexample (c): ratio below 0.05 stored by resize_node must be used verbatim by subtree_min
        let wide = resize(&l, "s1", 0.02, (10000.0, 800.0), default_min_for(None), None).unwrap();
        let Node::Split { ratio: stored_r, .. } = wide.root.as_ref().unwrap() else { panic!() };
        assert!((*stored_r - 0.02).abs() < 1e-5, "expected stored 0.02, got {stored_r}");
        let (min_w, _) = subtree_min(wide.root.as_ref().unwrap(), &default_min_for(None));
        assert!((min_w - 9000.0).abs() < 1.0, "expected 180/0.02 = 9000px, got {min_w}");

        // Counterexample: repeated 1e-6 ancestor resizes must NEVER drift a locked tile
        let mut cur = l.clone();
        for i in 1..=100 {
            let tiny = 0.5 + (i as f32) * 1e-6;
            assert_eq!(
                resize(&cur, "s1", tiny, (1000.0, 800.0), default_min_for(Some("k")), Some("k")),
                Err(Refusal::Locked)
            );
            cur = cur;
        }
        assert_eq!(tile_rect(&cur, "k"), tile_rect(&l, "k"));

        // Descendant ratio test: 80/20 row of two 180px leaves requires 900px; inside 1200px -> min ratio 0.75
        let nested = Layout {
            version: LAYOUT_VERSION,
            root: Some(Node::Split {
                id: "parent".into(),
                axis: Axis::Row,
                ratio: 0.8,
                a: Box::new(Node::Split {
                    id: "child".into(),
                    axis: Axis::Row,
                    ratio: 0.8,
                    a: Box::new(Node::Tile(tile("a", PageKind::Board, "m.kicad_pcb"))),
                    b: Box::new(Node::Tile(tile("b", PageKind::Schematic, "m.kicad_sch"))),
                }),
                b: Box::new(Node::Tile(tile("c", PageKind::Solid3d, "m.kicad_pcb"))),
            }),
            focus: Some("a".into()),
            maximized: None,
        };
        let resized = resize(&nested, "parent", 0.4, (1200.0, 800.0), default_min_for(None), None).unwrap();
        let Node::Split { ratio, .. } = resized.root.as_ref().unwrap() else { panic!() };
        assert!((*ratio - 0.75).abs() < 1e-4, "expected 0.75, got {ratio}");
    }

    #[test]
    fn overlay_same_commit_different_label_rejected_and_unsplit_restores_focus_and_maximized() {
        let mut map = VersionSourceMap::new();
        let sid = map.register("/proj/.git", "/proj");

        // Same (source_id, id) with different label must be rejected as IncompatibleOverlay
        let base = Layout::single(Tile {
            id: "t1".into(),
            kind: PageKind::Board,
            source: "main.kicad_pcb".into(),
            explicit: false,
            version: Some(TileVersion { source_id: sid.clone(), id: SHA_A.into(), label: "v1".into() }),
            unresolved_version: false,
            overlay: None,
        })
        .unwrap();
        let same_commit_diff_label = Overlay {
            kind: PageKind::Board,
            source: "main.kicad_pcb".into(),
            version: Some(TileVersion { source_id: sid.clone(), id: SHA_A.into(), label: "rev-one".into() }),
            mode: OverlayMode::Diff,
            layer_bits: None,
        };
        assert_eq!(overlay(&base, "t1", Some(same_commit_diff_label), None), Err(Refusal::IncompatibleOverlay));

        // Add a second tile, attach valid overlay to t1, focus t2, maximize t1 -> split -> unsplit restores exact Layout
        let l = add(&base, Some("t1"), Side::Right, tile("t2", PageKind::Files, "README.md"), false, None).unwrap();
        let ov = Overlay {
            kind: PageKind::Board,
            source: "main.kicad_pcb".into(),
            version: Some(ver(&sid, SHA_B)),
            mode: OverlayMode::Old,
            layer_bits: Some(12),
        };
        let mut with_ov = overlay(&l, "t1", Some(ov), None).unwrap();
        with_ov = maximize(&with_ov, "t1", None).unwrap();
        with_ov.focus = Some("t2".into()); // distinct focus + maximized
        let split = split_overlay(&with_ov, "t1", Side::Bottom, Some("t3".into()), None).unwrap();
        let back = unsplit_overlay(&split, "t1", None).unwrap();
        assert_eq!(back, with_ov);
    }

    #[test]
    fn versions_strict_full_id_resolver_old_overlay_versioned_3d_and_empty_root() {
        let root = "/proj";
        let mut resolver = VersionSourceMap::new();
        let sid_main = resolver.register("/proj/.git", "/proj");
        let sid_sub = resolver.register("/proj/sub/.git", "/proj/sub");
        assert_ne!(sid_main, sid_sub, "distinct repos/tops get distinct opaque source_ids");

        // Short prefix (8 hex) must be rejected everywhere
        assert!(!is_full_object_id("1111aaaa"));
        let short_json = format!(
            r#"{{"version":1,"focus":"t1","maximized":null,"root":{{"type":"tile","id":"t1","kind":"board","source":"m.kicad_pcb","version":{{"source_id":"{sid_main}","id":"1111aaaa","label":"1111aaaa"}}}}}}"#
        );
        assert!(serde_json::from_str::<Layout>(&short_json).is_err());

        // Ambiguous source_id -> unresolved error
        let mut ambig = VersionSourceMap::new();
        ambig.insert("vsrc-dup", VersionSource { git_dir: "/a/.git".into(), repo_top: "/a".into() });
        ambig.insert("vsrc-dup", VersionSource { git_dir: "/b/.git".into(), repo_top: "/b".into() });
        assert!(ambig.resolve("vsrc-dup").is_none());

        // Empty context root MUST NOT return Ok(vec![]) for a versioned tile
        let v_board = Layout::single(Tile {
            id: "t1".into(),
            kind: PageKind::Board,
            source: "main.kicad_pcb".into(),
            explicit: false,
            version: Some(ver(&sid_main, SHA_A)),
            unresolved_version: false,
            overlay: None,
        })
        .unwrap();
        assert!(plan_watches(&v_board, &WatchContext::default(), &VersionSourceMap::new()).is_err());
        assert_eq!(
            plan_watches(&v_board, &WatchContext::default(), &resolver),
            Err(WatchError::MissingRoot { tile_id: "t1".into() })
        );

        // A Windows root: the default board, named by kicad::project with '\\', is still the default when a tile names it by its path
        // under the root (no explicit path is watched), as on Unix
        let win = derive_layout(&DeriveInput { root: "C:\\p", open: "board", file: "", default_pcb: "C:\\p\\main.kicad_pcb", ..Default::default() });
        let wctx = WatchContext { root: "C:\\p".into(), default_pcb: "C:\\p\\main.kicad_pcb".into(), default_sch: String::new() };
        assert_eq!(wctx.default_pcb, "C:\\p\\main.kicad_pcb");
        let ws = plan_watches(&win, &wctx, &VersionSourceMap::new()).unwrap();
        assert_eq!(ws.iter().map(|w| (w.kind, w.path.as_str())).collect::<Vec<_>>(), vec![(0, "")], "{ws:?}");
        assert!(is_default("C:\\p/main.kicad_pcb", "C:\\p\\main.kicad_pcb") && !is_default("C:\\p/a.kicad_pcb", "C:\\p\\main.kicad_pcb") && !is_default("", ""));

        // Counterexample (a): historical input with abbreviated id NEVER falls back to live default PCB
        let abbrev = derive_layout(&DeriveInput {
            root,
            open: "board",
            file: "git\t/proj/.git\t1111aaaa\t5\t/proj/main.kicad_pcb",
            default_pcb: "/proj/main.kicad_pcb",
            ..Default::default()
        });
        let abbrev_t = abbrev.tile("t1").unwrap();
        assert!(abbrev_t.unresolved_version, "abbreviated git input must yield unresolved_version tile");
        assert!(abbrev_t.source.is_empty(), "must not fall back to live default PCB");
        assert!(matches!(plan_watches(&abbrev, &WatchContext { root: root.into(), default_pcb: "/proj/main.kicad_pcb".into(), default_sch: String::new() }, &resolver), Err(WatchError::UnresolvedVersion { .. })));

        // Counterexample (b): versioned base + live Old overlay on empty root MUST error, NEVER Ok([])
        let ver_base_live_old = Layout::single(Tile {
            id: "t1".into(),
            kind: PageKind::Board,
            source: "main.kicad_pcb".into(),
            explicit: false,
            version: Some(ver(&sid_main, SHA_B)),
            unresolved_version: false,
            overlay: Some(Overlay {
                kind: PageKind::Board,
                source: "main.kicad_pcb".into(),
                version: None,
                mode: OverlayMode::Old,
                layer_bits: None,
            }),
        })
        .unwrap();
        assert_eq!(
            plan_watches(&ver_base_live_old, &WatchContext::default(), &resolver),
            Err(WatchError::MissingRoot { tile_id: "t1".into() })
        );

        // Versioned 3D tile MUST NOT plan the live board
        let v_3d = Layout::single(Tile {
            id: "t1".into(),
            kind: PageKind::Solid3d,
            source: "main.kicad_pcb".into(),
            explicit: false,
            version: Some(ver(&sid_main, SHA_A)),
            unresolved_version: false,
            overlay: None,
        })
        .unwrap();
        let ctx = WatchContext { root: root.into(), default_pcb: "/proj/main.kicad_pcb".into(), default_sch: String::new() };
        assert!(plan_watches(&v_3d, &ctx, &VersionSourceMap::new()).is_err());
        let specs_3d = plan_watches(&v_3d, &ctx, &resolver).unwrap();
        let expected_wire = rv::hist_git("/proj/.git", SHA_A, "/proj", "main.kicad_pcb");
        assert_eq!(specs_3d, vec![
            WatchSpec { kind: 2, key: watch_key(2, root, &expected_wire), root: root.into(), path: expected_wire.clone() },
            WatchSpec { kind: 0, key: watch_key(0, root, &expected_wire), root: root.into(), path: expected_wire.clone() },
        ]);

        // `Old` overlay MUST watch the overlay's version (SHA_A), NEVER the base tile's version (SHA_B) or live file
        let old_ov = Layout::single(Tile {
            id: "t1".into(),
            kind: PageKind::Board,
            source: "main.kicad_pcb".into(),
            explicit: false,
            version: Some(ver(&sid_main, SHA_B)),
            unresolved_version: false,
            overlay: Some(Overlay {
                kind: PageKind::Board,
                source: "main.kicad_pcb".into(),
                version: Some(ver(&sid_main, SHA_A)),
                mode: OverlayMode::Old,
                layer_bits: None,
            }),
        })
        .unwrap();
        let specs_old = plan_watches(&old_ov, &ctx, &resolver).unwrap();
        assert_eq!(specs_old.len(), 1);
        assert_eq!(specs_old[0].path, expected_wire);
    }

    #[test]
    fn random_sequence_checks_invariants_overlays_locks_and_watch_planning() {
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
        let ctx = WatchContext {
            root: "/proj".into(),
            default_pcb: "/proj/dir/file.kicad_pcb".into(),
            default_sch: "/proj/dir/file.kicad_sch".into(),
        };
        let mut resolver = VersionSourceMap::new();
        resolver.register("/proj/.git", "/proj");
        let mut layout = Layout::empty();
        for step in 0..600 {
            let op = next_u64() % 12;
            let tiles: Vec<TileId> = layout.tiles().iter().map(|t| t.id.clone()).collect();
            let locked = if !tiles.is_empty() && next_u64() % 4 == 0 {
                Some(tiles[(next_u64() as usize) % tiles.len()].clone())
            } else {
                None
            };
            let locked_ref = locked.as_deref();
            let before = layout.clone();
            let candidate = match op {
                0 | 1 => {
                    let k = kinds[(next_u64() as usize) % kinds.len()];
                    let s = sides[(next_u64() as usize) % sides.len()];
                    let target = if tiles.is_empty() { None } else { Some(tiles[(next_u64() as usize) % tiles.len()].as_str()) };
                    let id = layout.next_tile_id();
                    let src = match k { PageKind::Schematic => "dir/file.kicad_sch", PageKind::Gerbers => "mfg/r1", _ => "dir/file.kicad_pcb" };
                    add(&layout, target, s, tile(&id, k, src), false, locked_ref).ok()
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
                    resize(&layout, &sid, r, (2400.0, 1600.0), default_min_for(kt.as_deref()), locked_ref).ok()
                }
                6 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    maximize(&layout, t, locked_ref).ok()
                }
                7 => restore(&layout, locked_ref).ok(),
                8 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    let ov = if next_u64() % 2 == 0 {
                        Some(Overlay { kind: PageKind::Gerbers, source: "mfg/r1".into(), version: None, mode: OverlayMode::Diff, layer_bits: None })
                    } else {
                        None
                    };
                    overlay(&layout, t, ov, locked_ref).ok()
                }
                9 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    let s = sides[(next_u64() as usize) % sides.len()];
                    split_overlay(&layout, t, s, None, locked_ref).ok().map(|o| o.layout)
                }
                10 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    let k = kinds[(next_u64() as usize) % kinds.len()];
                    retarget(&layout, t, k, "dir/file.kicad_sch".into(), locked_ref).ok()
                }
                11 if !tiles.is_empty() => {
                    let t = &tiles[(next_u64() as usize) % tiles.len()];
                    let bad = Overlay { kind: PageKind::Browser, source: "".into(), version: None, mode: OverlayMode::Diff, layer_bits: None };
                    assert_eq!(overlay(&layout, t, Some(bad), locked_ref), Err(Refusal::IncompatibleOverlay));
                    None
                }
                _ => None,
            };
            if let Some(next) = candidate {
                check_lock_preserved(&before, &next, locked_ref)
                    .unwrap_or_else(|e| panic!("step {step} broke lock {locked_ref:?}: {e:?}"));
                layout = next;
            }
            check_invariants(&layout).unwrap_or_else(|e| panic!("step {step} violated invariant: {e}\n{layout:#?}"));
            let specs = plan_watches(&layout, &ctx, &resolver).unwrap_or_else(|e| panic!("step {step} watch plan failed: {e:?}"));
            let unique: HashSet<&str> = specs.iter().map(|s| s.key.as_str()).collect();
            assert_eq!(unique.len(), specs.len(), "step {step} duplicate watch keys");
            for sp in &specs {
                assert_eq!(sp.key, watch_key(sp.kind, &sp.root, &sp.path));
            }
        }
    }

    #[test]
    fn refusal_catalog_text_matches_i18n_files() {
        let all = [
            Refusal::Locked,
            Refusal::OneKicad,
            Refusal::TooSmall,
            Refusal::NotFound,
            Refusal::SameTile,
            Refusal::NoOverlay,
            Refusal::IncompatibleOverlay,
            Refusal::InvalidSource,
            Refusal::DuplicateId,
            Refusal::InvalidRatio,
            Refusal::UnknownFrame,
            Refusal::InvalidInput,
        ];
        for (lang, text) in [
            ("en", include_str!("../../../../i18n/en.msg")),
            ("es", include_str!("../../../../i18n/es.msg")),
            ("ja", include_str!("../../../../i18n/ja.msg")),
        ] {
            for r in all {
                let want = format!("{} = {}", r.i18n_key(), r.text_for(lang));
                assert!(text.lines().any(|l| l.trim() == want), "missing in {lang}.msg: {want}");
            }
        }
    }
}
