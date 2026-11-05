//! `--swatches [--theme light|dark]`: every theme token as a flat square, and alpha blends over the page background,
//! for the colour check (test/gpui/visual/check_colors.py). Each square reports a GPUI_RECT line.
use crate::theme;
use gpui::{prelude::*, *};

pub fn view(light: bool) -> impl IntoElement {
    let tokens = theme::tokens(light);
    canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
        let scale = window.scale_factor();
        window.paint_quad(fill(b, theme::color(light, "bg")));
        let (w, h, gap) = (64.0_f32, 40.0_f32, 12.0_f32);
        for (i, (name, hex)) in tokens.iter().enumerate() {
            let (col, row) = ((i % 8) as f32, (i / 8) as f32);
            let r = Bounds { origin: point(b.origin.x + px(16.0 + col * (w + gap)), b.origin.y + px(16.0 + row * (h + gap))), size: size(px(w), px(h)) };
            window.paint_quad(fill(r, rgb(*hex)));
            crate::art::report(&format!("swatch:{name}"), r, scale);
        }
        // translucent white and black over the page background (what the web's rgba() does in sRGB space)
        let y0 = 16.0 + ((tokens.len() / 8 + 1) as f32) * (h + gap);
        for (j, (label, color)) in [("white", 0xffffff_u32), ("black", 0x000000_u32)].iter().enumerate() {
            for (k, alpha) in [64_u32, 128, 192].iter().enumerate() {
                let r = Bounds { origin: point(b.origin.x + px(16.0 + k as f32 * (w + gap)), b.origin.y + px(y0 + j as f32 * (h + gap))), size: size(px(w), px(h)) };
                window.paint_quad(fill(r, rgba((color << 8) | alpha)));
                crate::art::report(&format!("blend:{label}:{alpha}"), r, scale);
            }
        }
    }).size_full()
}

/// Every icon the web shows, large enough to measure, for the icon check: each reports `icon:<name>`.
pub const WEB_ICONS: [&str; 26] = ["chevron-down", "pin", "circle-check", "archive", "trash", "git-fork", "message-square", "terminal", "search", "circuit-board", "cpu", "box",
    "layout-panel-left", "layers", "folder", "diff", "globe", "chevron-right", "chevron-left", "settings", "plus", "user", "square-pen", "gauge", "paperclip", "mic"];
pub fn gallery(light: bool) -> impl IntoElement {
    let c = theme::color(light, "text");
    div().size_full().bg(theme::color(light, "bg")).p(px(16.)).flex().flex_wrap().gap(px(20.))
        .children(WEB_ICONS.iter().map(move |n| crate::art::icon(n, 36., c)))
}
