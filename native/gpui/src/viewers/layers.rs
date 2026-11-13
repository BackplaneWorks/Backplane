//! Board and schematic layers as the hub decides them (viewer.off / dim / lift bit masks, viewer.layerList rows): which chunks are drawn, which
//! dimmed to a quarter, which one last. The phones and the web draw exactly this from the same fields (core/layers `Lyr.masks`, Plot.kt
//! `shown`/`dimmed`/`lift`). The client never decides a mask: a row press or Ctrl+H is a view action and the hub's next picture carries the new masks.
use crate::{hub::Command, i18n::trs, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::BTreeMap;

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
/// The layers a comparison on this page keeps its marks to (0 every layer): the hub's `viewer.layerBits`.
pub fn bits(viewer: &Value) -> u32 { viewer["layerBits"].as_u64().unwrap_or(0) as u32 }
/// Whether Ctrl+H has a plot to act on: Board, Schematic or Split on show (a 3D mesh has no layers, and an open Diff covers the page).
pub fn cycles(thread: &Value) -> bool { !thread["diff"].is_object() && matches!(thread["viewer"]["open"].as_str(), Some("board" | "schematic" | "split")) }
/// The layers a plot has, a bit each (a layer outside 0..31 has none): the hub starts the focus on the first of them with art.
pub fn present_bits(layers: impl IntoIterator<Item = u32>) -> u32 { layers.into_iter().filter(|l| *l < 32).fold(0, |a, l| a | 1 << l) }
/// Ctrl+H's value: the pane it is for ("schematic" on Split; empty otherwise), then the plot's layers.
/* i18n-ok: an action value */ pub fn cycle_value(schematic: bool, present: u32) -> String { format!("{} {present}", if schematic { "schematic" } else { "" }) }
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
pub struct Row { pub layer: u32, pub name: String, pub on: bool, pub active: bool, pub color: u32 }
#[derive(Clone, Debug, PartialEq)]
pub struct Strip { pub head: String, pub mode: u32, pub modes: bool, pub reset: bool, pub present: u32, pub rows: Vec<Row> }
/// The Layers menu: only the layers this plot has (as the phones list them), in the hub's (KiCad's) order, a comparison's marks after, each in the colour the plot draws it (`present`: layer to colour).
pub fn strip(viewer: &Value, present: &BTreeMap<u32, u32>) -> Strip {
    let rows = viewer["layerList"].as_array().into_iter().flatten().chain(viewer["layerMarks"].as_array().into_iter().flatten()).filter_map(|r| {
        let layer = r["layer"].as_u64()? as u32;
        present.get(&layer).map(|color| Row { layer, name: r["name"].as_str().unwrap_or("").to_string(), on: r["on"].as_bool().unwrap_or(true), active: r["active"].as_bool().unwrap_or(false), color: *color })
    }).collect();
    /* i18n-ok: drawn through trs (strip_el) */ let head = viewer["layerHead"].as_str().filter(|h| !h.is_empty()).unwrap_or("Layers").to_string();
    Strip { head, mode: viewer["layerMode"].as_u64().unwrap_or(0) as u32, modes: viewer["layerModes"].as_bool().unwrap_or(false), reset: viewer["layerReset"].as_bool().unwrap_or(false), present: present_bits(present.keys().copied()), rows }
}

/// The Layers button and, while open, its menu: All/Dim/Only (a board), a swatch per layer in its colour (show or hide it), its name (make it the active layer,
/// again: all back), Reset layers while anything differs. Stays open so several layers can be turned on and off in a row. The button is lit while a layer stands out; `key` is the focus key, for its tooltip.
pub fn strip_el(cx: &mut Context<Backplane>, light: bool, s: &Strip, key: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    if s.rows.is_empty() { return div().into_any_element(); }
    let head = crate::screens::viewer::tall(div().id("layers-btn").px_2().text_size(crate::scale::tpx(12.))).cursor_pointer().border_1().border_color(c(if s.mode > 0 { "accent" } else { "line" })).bg(c("raise")).when(s.mode > 0, |d| d.text_color(c("accent"))).child(trs(&s.head))
        .tooltip(crate::keys::tip(&trs("Layers: others as they are, dimmed, hidden"), key, light))
        .on_click(cx.listener(|this, _, _, cx| { let pv = &mut this.screens.plotview; pv.layers_open = !pv.layers_open; cx.notify(); }));
    head.into_any_element()
}

/// The open Layers panel: absolute over the canvas (the button row keeps its height, so the canvas never jumps), its own scroll past `max_h`.
pub fn panel_el(cx: &mut Context<Backplane>, light: bool, s: &Strip, open: bool, cap: f32, scroll: &ScrollHandle) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    if !open || s.rows.is_empty() { return None; }
    // a cap of 0: as tall as the pane lets it be before it scrolls
    let mh: Length = if cap > 0. { px(cap).into() } else { relative(0.96).into() };
    let mut panel = div().id("layers-panel").absolute().top(px(4.)).left(px(12.)).max_h(mh).overflow_y_scroll().track_scroll(scroll).flex().flex_col().gap_1().p_2().border_1().border_color(c("line")).bg(c("panel")).occlude();
    if s.modes {
        let present = s.present;
        /* i18n-ok: drawn through trs */ panel = panel.child(div().flex().gap_1().children(["All", "Dim", "Only"].iter().enumerate().map(|(i, label)| {
            let on = s.mode == i as u32; let d = div().id(SharedString::from(format!("layer-mode-{i}"))).px_2().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1();
            (if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.border_color(c("line")).hover(|s| s.bg(c("raise"))) }).child(trs(label))
                /* i18n-ok: an action value */ .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-layer-mode".into(), format!("{i} {present}"))); cx.notify(); }))
        })));
    }
    for r in &s.rows {
        let (l, on, active, name) = (r.layer, r.on, r.active, r.name.clone());
        panel = panel.child(div().flex().items_center().gap_2().text_size(crate::scale::tpx(12.))
            .when(active, |d| d.bg(c("raise")))
            .child(div().id(SharedString::from(format!("layer-box-{l}"))).size(px(12.)).border_1().border_color(c("line")).cursor_pointer().bg(swatch(r.color, on))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-layer".into(), l.to_string())); cx.notify(); })))
            .child(div().id(SharedString::from(format!("layer-name-{l}"))).cursor_pointer().text_color(if active { c("accent") } else if on { c("text") } else { c("faint") }).child(name)
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-layer-pick".into(), l.to_string())); cx.notify(); }))));
    }
    if s.reset {
        panel = panel.child(div().id("layers-reset").text_size(crate::scale::tpx(12.)).cursor_pointer().text_color(c("accent")).child(trs("Reset layers"))
            .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-layers-reset".into(), String::new())); cx.notify(); })));
    }
    Some(panel.into_any_element())
}
/// A layer's swatch: its colour, faint while the layer is off (Side.layer's 50 of 255).
fn swatch(color: u32, on: bool) -> Rgba { rgba((color & 0xffffff) << 8 | if on { 0xff } else { 50 }) }

#[cfg(test)]
mod tests {
    use super::{bits, cycle_value, cycles, masks, plan, present_bits, strip, swatch, Masks, Row, DIM};
    use serde_json::json;
    use std::collections::BTreeMap;
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
        let s = strip(&v, &BTreeMap::from([(0, 0xff0000), (1, 0x00ff00)]));
        assert_eq!(s.rows, vec![Row { layer: 0, name: "F.Cu".into(), on: true, active: false, color: 0xff0000 }, Row { layer: 1, name: "B.Cu".into(), on: false, active: true, color: 0x00ff00 }]);
        assert!(s.modes && s.mode == 1 && !s.reset);
        assert!(strip(&json!({"layerReset": true}), &BTreeMap::new()).reset);
        let marks = json!({"layerList": [{"layer": 0, "name": "F.Cu"}], "layerMarks": [{"layer": 26, "name": "Removed"}, {"layer": 28, "name": "Added"}]});
        assert_eq!(strip(&marks, &BTreeMap::from([(0, 1), (26, 2)])).rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["F.Cu", "Removed"], "a comparison's marks last, those the plot has");
    }
    #[test] fn a_swatch_is_the_layers_colour_and_faint_while_it_is_off() {
        assert_eq!(swatch(0x336699, true), gpui::rgba(0x336699ff)); assert_eq!(swatch(0x336699, false), gpui::rgba(0x33669932));
    }
    #[test] fn ctrl_h_acts_on_a_plot_on_show_only() {
        for open in ["board", "schematic", "split"] { assert!(cycles(&json!({"viewer": {"open": open}})), "{open}"); }
        for open in ["3d", "mech", "files", "browser", "review"] { assert!(!cycles(&json!({"viewer": {"open": open}})), "{open}"); }
        assert!(!cycles(&json!({"viewer": {"open": "board"}, "diff": {}})), "an open Diff covers the board"); assert!(!cycles(&json!(null)));
    }
    #[test] fn the_hub_is_told_the_layers_the_plot_has() {
        assert_eq!(present_bits([0, 5, 5, 40]), 0b100001, "a layer outside 0..31 has no bit");
        assert_eq!(strip(&json!({}), &BTreeMap::from([(5, 1), (13, 2)])).present, 1 << 5 | 1 << 13);
        assert_eq!(cycle_value(false, 32), " 32"); assert_eq!(cycle_value(true, 32), "schematic 32");
    }
    #[test] fn a_comparison_keeps_to_the_hubs_bits() { assert_eq!(bits(&json!({"layerBits": 4096})), 4096); assert_eq!(bits(&json!({})), 0); }
}
