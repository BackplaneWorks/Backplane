//! The hub's layout fitted to ONE device's viewer area (data/1: phones, tablets, browsers). Pure: no I/O, no clock.
//!
//! The stored layout is the TREE (`layout::Layout`), the same for every connection of a device; its FIT is per connection and never stored:
//! each connection sends the bounds of its viewer area (CSS px) and gets back what to draw, so the clients hold no layout rule.
//!
//! - `stack` (short side < `STACK_BELOW_PX`): one tile at the full area (the maximized tile, else the focused one); the others are listed in
//!   `order` (focus recency) for a tab or swipe strip.
//! - `twoUp` (short side < `TWO_UP_BELOW_PX`): the focused tile and the one used before it, side by side in a wide area, one over the other in a tall
//!   one (`axis` says which). Where they are siblings of one split they keep its ratio, clamped so each keeps `MIN_TILE_PX`.
//! - `full` (wider): the whole tree, unless a tile would fall under `MIN_TILE_PX` there, which makes it `twoUp`.
//!
//! `bounds` is clamped before any arithmetic: a zero, negative, NaN, infinite or huge side is brought into `[MIN_BOUND_PX, MAX_BOUND_PX]`.

use super::layout::{Axis, Layout, Node, Rect, TileId, MIN_TILE_PX};

/// Short sides under this stack: a phone.
pub const STACK_BELOW_PX: f32 = 600.0;
/// Short sides under this (and over `STACK_BELOW_PX`) are two-up: a tablet.
pub const TWO_UP_BELOW_PX: f32 = 900.0;
/// A side is never taken to be smaller or larger than these, whatever a client sends.
pub const MIN_BOUND_PX: f32 = 100.0;
pub const MAX_BOUND_PX: f32 = 8192.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Stack, TwoUp, Full }

impl Mode {
    pub fn as_str(self) -> &'static str { match self { Mode::Stack => "stack", Mode::TwoUp => "twoUp", Mode::Full => "full" } }
}

/// A split drawn in the fit: its id, axis, ratio and the unit rectangle it divides (a client puts its handle on the seam).
#[derive(Clone, Debug, PartialEq)]
pub struct FitSplit { pub id: String, pub axis: Axis, pub ratio: f32, pub rect: Rect }

#[derive(Clone, Debug, PartialEq)]
pub struct Fitted {
    pub mode: Mode,
    /// The axis the visible tiles are arranged on (`Row`: side by side, `Col`: one over the other); None for one tile.
    pub axis: Option<Axis>,
    /// Every visible tile with its unit rectangle, in drawing order.
    pub visible: Vec<(TileId, Rect)>,
    /// The splits among the visible tiles the person can move (only real splits of the stored tree; never an invented one).
    pub splits: Vec<FitSplit>,
    /// Every tile id, the focused first, then by recency: the stack's order.
    pub order: Vec<TileId>,
    /// The bounds the fit used, after clamping.
    pub bounds: (f32, f32),
}

/// `bounds` as the fit takes them: finite, positive, within `[MIN_BOUND_PX, MAX_BOUND_PX]`.
pub fn clamp_bounds(w: f64, h: f64) -> (f32, f32) {
    let one = |v: f64| if v.is_finite() { (v as f32).clamp(MIN_BOUND_PX, MAX_BOUND_PX) } else if v == f64::INFINITY { MAX_BOUND_PX } else { MIN_BOUND_PX };
    (one(w), one(h))
}

pub fn mode_for(bounds: (f32, f32)) -> Mode {
    let short = bounds.0.min(bounds.1);
    if short < STACK_BELOW_PX { Mode::Stack } else if short < TWO_UP_BELOW_PX { Mode::TwoUp } else { Mode::Full }
}

const UNIT: Rect = Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };

/// Tile ids in the order a stack shows them: the focused first, then `recent` (most recent first) as far as they still exist, then the rest in tree order.
pub fn stack_order(layout: &Layout, recent: &[TileId]) -> Vec<TileId> {
    let all: Vec<TileId> = layout.tiles().into_iter().map(|t| t.id.clone()).collect();
    let mut out: Vec<TileId> = Vec::with_capacity(all.len());
    let mut push = |id: &TileId, out: &mut Vec<TileId>| if all.contains(id) && !out.contains(id) { out.push(id.clone()) };
    if let Some(m) = &layout.maximized { push(m, &mut out) }
    if let Some(f) = &layout.focus { push(f, &mut out) }
    for id in recent { push(id, &mut out) }
    for id in &all { push(id, &mut out) }
    out
}

/// The unit rectangle of every node of `node` laid out in `rect`: tiles, and splits with the rectangle they divide.
fn walk(node: &Node, rect: Rect, tiles: &mut Vec<(TileId, Rect)>, splits: &mut Vec<FitSplit>) {
    match node {
        Node::Tile(t) => tiles.push((t.id.clone(), rect)),
        Node::Split { id, axis, ratio, a, b } => {
            let r = if ratio.is_finite() { ratio.clamp(0.0, 1.0) } else { 0.5 };
            let (ra, rb) = match axis {
                Axis::Row => (Rect { x: rect.x, y: rect.y, w: rect.w * r, h: rect.h }, Rect { x: rect.x + rect.w * r, y: rect.y, w: rect.w * (1.0 - r), h: rect.h }),
                Axis::Col => (Rect { x: rect.x, y: rect.y, w: rect.w, h: rect.h * r }, Rect { x: rect.x, y: rect.y + rect.h * r, w: rect.w, h: rect.h * (1.0 - r) }),
            };
            splits.push(FitSplit { id: id.clone(), axis: *axis, ratio: r, rect });
            walk(a, ra, tiles, splits);
            walk(b, rb, tiles, splits);
        }
    }
}

/// Whether every tile keeps at least `MIN_TILE_PX` in CSS px along both axes at `bounds`.
fn fits_whole(bounds: (f32, f32), tiles: &[(TileId, Rect)]) -> bool {
    tiles.iter().all(|(_, r)| r.w * bounds.0 >= MIN_TILE_PX - 0.5 && r.h * bounds.1 >= MIN_TILE_PX - 0.5)
}

/// The split directly above both tiles (the lowest common ancestor), when it is a split of the stored tree.
fn common_split<'a>(node: &'a Node, a: &str, b: &str) -> Option<(&'a str, Axis, f32)> {
    fn has(node: &Node, id: &str) -> bool { match node { Node::Tile(t) => t.id == id, Node::Split { a, b, .. } => has(a, id) || has(b, id) } }
    match node {
        Node::Tile(_) => None,
        Node::Split { id, axis, ratio, a: na, b: nb } => {
            if (has(na, a) && has(nb, b)) || (has(na, b) && has(nb, a)) { return Some((id, *axis, *ratio)) }
            common_split(na, a, b).or_else(|| common_split(nb, a, b))
        }
    }
}

/// Fit `layout` to `bounds` (already clamped by `clamp_bounds`); `recent` is the device's focus recency, most recent first.
pub fn fit(layout: &Layout, recent: &[TileId], bounds: (f32, f32)) -> Fitted {
    let order = stack_order(layout, recent);
    let mut out = Fitted { mode: mode_for(bounds), axis: None, visible: vec![], splits: vec![], order: order.clone(), bounds };
    let Some(root) = &layout.root else { return out };
    // a maximized tile is the whole area in every mode
    if let Some(m) = layout.maximized.as_ref().filter(|m| layout.contains_tile(m)) {
        out.visible.push((m.clone(), UNIT));
        return out;
    }
    if out.mode == Mode::Full {
        let (mut tiles, mut splits) = (vec![], vec![]);
        walk(root, UNIT, &mut tiles, &mut splits);
        if fits_whole(bounds, &tiles) {
            out.axis = splits.first().map(|s| s.axis);
            out.visible = tiles;
            out.splits = splits;
            return out;
        }
        out.mode = Mode::TwoUp; // a tile would be too small here: two of them, as in a tablet
    }
    let focus = order.first().cloned();
    let Some(first) = focus else { return out };
    if out.mode == Mode::TwoUp {
        if let Some(second) = order.get(1).cloned() {
            // the axis comes from the aspect of the area: a wide one puts the two side by side, a tall one stacks them
            let axis = if bounds.0 >= bounds.1 { Axis::Row } else { Axis::Col };
            let len = if axis == Axis::Row { bounds.0 } else { bounds.1 };
            if len >= 2.0 * MIN_TILE_PX {
                let mut ratio = 0.5;
                let mut split = None;
                if let Some((sid, saxis, r)) = common_split(root, &first, &second) {
                    if saxis == axis && r.is_finite() {
                        ratio = r;
                        split = Some(sid.to_string());
                        // the stored order of the pair on the split decides which is first
                    }
                }
                let lo = (MIN_TILE_PX / len).min(0.5);
                let ratio = ratio.clamp(lo, 1.0 - lo);
                // which of the pair is first on the axis: the one that comes first in the stored tree when they share a split, else the focused one
                let tree_first = layout.tiles().iter().map(|t| t.id.clone()).find(|id| *id == first || *id == second);
                let (a_id, b_id) = if split.is_some() && tree_first.as_deref() == Some(second.as_str()) { (second.clone(), first.clone()) } else { (first.clone(), second.clone()) };
                let (ra, rb) = if axis == Axis::Row {
                    (Rect { x: 0.0, y: 0.0, w: ratio, h: 1.0 }, Rect { x: ratio, y: 0.0, w: 1.0 - ratio, h: 1.0 })
                } else {
                    (Rect { x: 0.0, y: 0.0, w: 1.0, h: ratio }, Rect { x: 0.0, y: ratio, w: 1.0, h: 1.0 - ratio })
                };
                out.axis = Some(axis);
                out.visible = vec![(a_id, ra), (b_id, rb)];
                if let Some(sid) = split { out.splits.push(FitSplit { id: sid, axis, ratio, rect: UNIT }); }
                return out;
            }
        }
        out.mode = Mode::Stack; // not room for two
    }
    out.mode = Mode::Stack;
    out.visible = vec![(first, UNIT)];
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::present::layout::{self, PageKind, Side, Tile};

    fn tile(id: &str, kind: PageKind) -> Tile { Tile { id: id.into(), kind, source: String::new(), explicit: false, version: None, unresolved_version: false, overlay: None } }
    fn two() -> Layout {
        let l = Layout::single(tile("t1", PageKind::Board)).unwrap();
        layout::add(&l, None, Side::Right, tile("t2", PageKind::Schematic), false, None).unwrap()
    }
    fn three() -> Layout { layout::add(&two(), Some("t2"), Side::Bottom, tile("t3", PageKind::Solid3d), false, None).unwrap() }
    fn ids(f: &Fitted) -> Vec<&str> { f.visible.iter().map(|(i, _)| i.as_str()).collect() }

    #[test]
    fn bounds_are_clamped_before_any_arithmetic() {
        assert_eq!(clamp_bounds(0.0, -5.0), (MIN_BOUND_PX, MIN_BOUND_PX));
        assert_eq!(clamp_bounds(f64::NAN, f64::NEG_INFINITY), (MIN_BOUND_PX, MIN_BOUND_PX));
        assert_eq!(clamp_bounds(f64::INFINITY, 1e30), (MAX_BOUND_PX, MAX_BOUND_PX));
        assert_eq!(clamp_bounds(390.0, 844.0), (390.0, 844.0));
        // a fit at any clamped bounds never divides by zero or yields a non-finite rectangle
        for (w, h) in [(0.0, 0.0), (f64::NAN, 5.0), (1e30, 1e30), (-1.0, 800.0)] {
            let f = fit(&three(), &[], clamp_bounds(w, h));
            assert!(f.visible.iter().all(|(_, r)| r.x.is_finite() && r.y.is_finite() && r.w.is_finite() && r.h.is_finite() && r.w > 0.0 && r.h > 0.0));
        }
    }

    #[test]
    fn the_mode_follows_the_short_side() {
        assert_eq!(mode_for((390.0, 844.0)), Mode::Stack);
        assert_eq!(mode_for((844.0, 390.0)), Mode::Stack);
        assert_eq!(mode_for((599.9, 2000.0)), Mode::Stack);
        assert_eq!(mode_for((820.0, 1180.0)), Mode::TwoUp);
        assert_eq!(mode_for((600.0, 600.0)), Mode::TwoUp);
        assert_eq!(mode_for((899.0, 5000.0)), Mode::TwoUp);
        assert_eq!(mode_for((1280.0, 900.0)), Mode::Full);
    }

    #[test]
    fn a_phone_stacks_one_tile_and_lists_the_rest_by_recency() {
        let first = layout::focus_tile(&three(), "t1", None).unwrap(); // `add` focuses the new tile: focus the first for this one
        let f = fit(&first, &[], (390.0, 844.0));
        assert_eq!((f.mode, ids(&f), f.axis), (Mode::Stack, vec!["t1"], None));
        assert_eq!(f.visible[0].1, UNIT);
        assert_eq!(f.order, vec!["t1", "t2", "t3"]);
        assert!(f.splits.is_empty());
        // the new tile is the focused one, so a stack shows it first
        assert_eq!(fit(&three(), &[], (390.0, 844.0)).order[0], "t3");
        // focus recency decides the order; unknown ids are dropped, missing ones appended
        let l = layout::focus_tile(&three(), "t3", None).unwrap();
        let f = fit(&l, &["t2".into(), "gone".into(), "t1".into()], (390.0, 844.0));
        assert_eq!((ids(&f), f.order.clone()), (vec!["t3"], vec!["t3".to_string(), "t2".into(), "t1".into()]));
    }

    #[test]
    fn a_tablet_is_two_up_and_the_hub_picks_the_axis_from_the_aspect() {
        let wide = fit(&three(), &[], (880.0, 700.0));
        assert_eq!(wide.mode, Mode::TwoUp);
        assert_eq!(wide.axis, Some(Axis::Row));
        assert_eq!(ids(&wide).len(), 2);
        let tall = fit(&three(), &[], (700.0, 1000.0));
        assert_eq!((tall.mode, tall.axis), (Mode::TwoUp, Some(Axis::Col)), "a tall narrow area stacks the two");
        let (a, b) = (tall.visible[0].1, tall.visible[1].1);
        assert!((a.h + b.h - 1.0).abs() < 1e-5 && a.w == 1.0 && b.w == 1.0 && (b.y - a.h).abs() < 1e-5);
        // the pair is the focused tile and the one used before it
        let l = layout::focus_tile(&three(), "t3", None).unwrap();
        let f = fit(&l, &["t2".into(), "t1".into()], (820.0, 1180.0));
        // t3 and t2 share a split: they keep the tree's order along the axis (t2 is the first child), the focused one is not moved ahead of it
        assert_eq!(ids(&f), vec!["t2", "t3"]);
    }

    #[test]
    fn siblings_keep_their_ratio_clamped_to_the_minimum_size() {
        let mut l = two(); // t1 | t2 in one split `s1`
        if let Some(Node::Split { ratio, .. }) = &mut l.root { *ratio = 0.7 }
        let f = fit(&l, &[], (800.0, 650.0));
        assert_eq!(f.mode, Mode::TwoUp);
        assert!((f.visible[0].1.w - 0.7).abs() < 1e-5);
        assert_eq!(f.splits.len(), 1);
        assert_eq!(f.splits[0].id, "s1");
        // a ratio that would leave a tile under 180 px is clamped
        if let Some(Node::Split { ratio, .. }) = &mut l.root { *ratio = 0.99 }
        let f = fit(&l, &[], (800.0, 650.0));
        assert!((f.visible[1].1.w * 800.0 - MIN_TILE_PX).abs() < 0.5, "{:?}", f.visible);
        // a pair that does not share a split of this axis gets an even split and no handle
        let f = fit(&three(), &["t3".into()], (800.0, 650.0));
        assert!(f.visible.len() == 2 && f.splits.len() <= 1);
    }

    #[test]
    fn a_wide_area_shows_the_whole_tree_and_a_narrow_pane_falls_back_to_two_up() {
        let f = fit(&three(), &[], (1400.0, 900.0));
        assert_eq!(f.mode, Mode::Full);
        assert_eq!(f.visible.len(), 3);
        assert_eq!(f.splits.len(), 2);
        let area: f32 = f.visible.iter().map(|(_, r)| r.w * r.h).sum();
        assert!((area - 1.0).abs() < 1e-4, "the tiles tile the area");
        // 900 wide, 900 tall: three tiles but a 450x450 corner still holds, so full; 900 x 900 with four deep splits does not
        let mut l = three();
        for (i, k) in [PageKind::Gerbers, PageKind::Mech, PageKind::Files].into_iter().enumerate() {
            l = layout::add(&l, Some("t3"), Side::Bottom, tile(&format!("t{}", i + 4), k), false, None).unwrap();
        }
        let f = fit(&l, &[], (900.0, 900.0));
        assert_eq!(f.mode, Mode::TwoUp, "a tile would be under {MIN_TILE_PX} px: two of them");
    }

    #[test]
    fn a_maximized_tile_is_the_whole_area_in_every_mode_and_empty_layouts_fit_to_nothing() {
        let l = layout::maximize(&three(), "t2", None).unwrap();
        for b in [(390.0, 844.0), (820.0, 1180.0), (1400.0, 900.0)] {
            let f = fit(&l, &[], b);
            assert_eq!((ids(&f), f.visible[0].1), (vec!["t2"], UNIT));
            assert!(f.splits.is_empty());
        }
        let f = fit(&Layout::empty(), &[], (1400.0, 900.0));
        assert!(f.visible.is_empty() && f.order.is_empty());
    }

    #[test]
    fn a_one_tile_layout_is_one_tile_everywhere() {
        let l = Layout::single(tile("t1", PageKind::Board)).unwrap();
        for b in [(390.0, 844.0), (820.0, 1180.0), (1400.0, 900.0)] {
            let f = fit(&l, &[], b);
            assert_eq!((ids(&f), f.splits.len()), (vec!["t1"], 0));
        }
    }
}
