//! Board and schematic layers as the hub decides them (viewer.off / dim / lift bit masks, viewer.layerList rows): which chunks are drawn, which
//! dimmed to a quarter, which one last. The phones and the web draw exactly this from the same fields (core/layers.bend `Lyr.masks`, Plot.kt
//! `shown`/`dimmed`/`lift`). The client never decides a mask: a row press is a view action and the hub's next picture carries the new masks.
use crate::{hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::BTreeSet;

pub const DIM: f32 = 0.25;
pub const NO_LIFT: u32 = 255;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Masks { pub off: u32, pub dim: u32, pub lift: u32 }
impl Default for Masks { fn default() -> Self { Masks { off: 0, dim: 0, lift: NO_LIFT } } }
/// The masks of `thread.viewer`.
pub fn masks(viewer: &Value) -> Masks {
    let g = |k: &str, d: u64| viewer[k].as_u64().unwrap_or(d) as u32;
    Masks { off: g("off", 0), dim: g("dim", 0), lift: g("lift", NO_LIFT as u64) }
}
fn bit(mask: u32, layer: u32) -> bool { layer < 32 && (mask >> layer) & 1 == 1 }

/// What to draw and in which order, for chunks whose layers are `layers` (paint order): (index, opacity factor). Hidden layers are left out
/// (a layer outside 0..31 is always shown), dimmed ones drawn at a quarter, the lifted layer after all the others.
pub fn plan(layers: &[u32], m: &Masks) -> Vec<(usize, f32)> {
    let shown = |l: u32| !bit(m.off, l);
    let f = |l: u32| if bit(m.dim, l) { DIM } else { 1.0 };
    let mut out: Vec<(usize, f32)> = Vec::with_capacity(layers.len());
    for lifted in [false, true] {
        for (i, l) in layers.iter().enumerate() { let l = *l; if shown(l) && (l == m.lift) == lifted { out.push((i, f(l))); } }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row { pub layer: u32, pub name: String, pub on: bool, pub active: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Strip { pub head: String, pub mode: u32, pub modes: bool, pub rows: Vec<Row> }
/// The Layers menu: only the layers this plot has (as the phones list them), in the hub's (KiCad's) order.
pub fn strip(viewer: &Value, present: &BTreeSet<u32>) -> Strip {
    let rows = viewer["layerList"].as_array().into_iter().flatten().filter_map(|r| {
        let layer = r["layer"].as_u64()? as u32;
        present.contains(&layer).then(|| Row { layer, name: r["name"].as_str().unwrap_or("").to_string(), on: r["on"].as_bool().unwrap_or(true), active: r["active"].as_bool().unwrap_or(false) })
    }).collect();
    let head = viewer["layerHead"].as_str().filter(|h| !h.is_empty()).unwrap_or("Layers").to_string();
    Strip { head, mode: viewer["layerMode"].as_u64().unwrap_or(0) as u32, modes: viewer["layerModes"].as_bool().unwrap_or(false), rows }
}

/// The Layers button and, while open, its menu: All/Dim/Only (a board), a box per layer (show or hide it), its name (make it the active layer,
/// again: all back). Stays open so several layers can be turned on and off in a row.
pub fn strip_el(cx: &mut Context<Backplane>, light: bool, s: &Strip, open: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    if s.rows.is_empty() { return div().into_any_element(); }
    let head = div().id("layers-btn").px_2().py_1().text_size(px(12.)).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).child(s.head.clone())
        .on_click(cx.listener(|this, _, _, cx| { let pv = &mut this.screens.plotview; pv.layers_open = !pv.layers_open; cx.notify(); }));
    div().flex().px_3().py_1().child(head).into_any_element()
}

/// The open Layers panel: absolute over the canvas (the button row keeps its height, so the canvas never jumps), its own scroll past `max_h`.
pub fn panel_el(cx: &mut Context<Backplane>, light: bool, s: &Strip, open: bool, cap: f32, scroll: &ScrollHandle) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    if !open || s.rows.is_empty() { return None; }
    let mut panel = div().id("layers-panel").absolute().top(px(4.)).left(px(12.)).max_h(px(cap)).overflow_y_scroll().track_scroll(scroll).flex().flex_col().gap_1().p_2().border_1().border_color(c("line")).bg(c("panel")).occlude();
    if s.modes {
        panel = panel.child(div().flex().gap_1().children(["All", "Dim", "Only"].iter().enumerate().map(|(i, label)| {
            let on = s.mode == i as u32; let d = div().id(SharedString::from(format!("layer-mode-{i}"))).px_2().text_size(px(12.)).cursor_pointer().border_1();
            (if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.border_color(c("line")).hover(|s| s.bg(c("raise"))) }).child(*label)
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-layer-mode".into(), i.to_string())); cx.notify(); }))
        })));
    }
    for r in &s.rows {
        let (l, on, active, name) = (r.layer, r.on, r.active, r.name.clone());
        panel = panel.child(div().flex().items_center().gap_2().text_size(px(12.))
            .child(div().id(SharedString::from(format!("layer-box-{l}"))).size(px(12.)).border_1().border_color(c("line")).cursor_pointer().when(on, |d| d.bg(c("accent")))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-layer".into(), l.to_string())); cx.notify(); })))
            .child(div().id(SharedString::from(format!("layer-name-{l}"))).cursor_pointer().text_color(if active { c("accent") } else { c("text") }).child(name)
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-layer-pick".into(), l.to_string())); cx.notify(); }))));
    }
    Some(panel.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::{masks, plan, strip, Masks, Row, DIM};
    use serde_json::json;
    use std::collections::BTreeSet;
    #[test] fn hidden_layers_are_left_out_and_outside_32_is_always_shown() {
        let m = Masks { off: 0b0110, dim: 0, lift: 255 };
        assert_eq!(plan(&[0, 1, 2, 3, 40], &m), vec![(0, 1.0), (3, 1.0), (4, 1.0)]);
    }
    #[test] fn dimmed_layers_take_a_quarter_and_the_lifted_one_is_last() {
        let m = Masks { off: 0, dim: 0b100, lift: 0 };
        assert_eq!(plan(&[0, 1, 2, 0], &m), vec![(1, 1.0), (2, DIM), (0, 1.0), (3, 1.0)], "layer 0 (lifted) after the rest, in its own order");
    }
    #[test] fn a_lifted_layer_that_is_hidden_stays_hidden() { assert!(plan(&[3, 3], &Masks { off: 0b1000, dim: 0, lift: 3 }).is_empty()); }
    #[test] fn masks_default_to_everything_shown_nothing_lifted() {
        assert_eq!(masks(&json!({})), Masks::default());
        assert_eq!(masks(&json!({"off": 5, "dim": 2, "lift": 7})), Masks { off: 5, dim: 2, lift: 7 });
    }
    #[test] fn the_menu_lists_only_the_layers_the_plot_has() {
        let v = json!({"layerHead": "Layers", "layerMode": 1, "layerModes": true, "layerList": [{"layer": 0, "name": "F.Cu", "on": true, "active": false}, {"layer": 1, "name": "B.Cu", "on": false, "active": true}, {"layer": 9, "name": "F.Mask", "on": true, "active": false}]});
        let s = strip(&v, &BTreeSet::from([0, 1]));
        assert_eq!(s.rows, vec![Row { layer: 0, name: "F.Cu".into(), on: true, active: false }, Row { layer: 1, name: "B.Cu".into(), on: false, active: true }]);
        assert!(s.modes && s.mode == 1);
    }
}
