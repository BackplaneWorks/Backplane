//! `--solid-demo TRIS[,STEPS]`: the 3D page on a synthetic sphere of about TRIS triangles, put through the real frame path (CBOR frame -> worker decode ->
//! render), then STEPS orbit-drag steps; prints render times (draft = half size while dragging, full at the end), the decode+first image time and the
//! process's resident memory before and after, as `GPUI_METRIC {"event":"solid_demo",...}`. Memory is the atlas too under a software Vulkan (llvmpipe).
//! A synthetic model: not a real board's. (The no-drop comparison, 137 -> 808 MB over 400 drag steps against 147 -> 156 MB with drop_image, was measured with a build
//! that had a switch for it; the switch is gone from the shipping render path.)
use super::solid::SolidPage;
use gpui::{prelude::*, *};
use std::time::{Duration, Instant};

fn put(out: &mut Vec<u8>, mut v: u32) { loop { let b = (v & 0x7f) as u8; v >>= 7; if v == 0 { out.push(b); return } out.push(b | 0x80); } }
fn zig(v: i32) -> u32 { ((v << 1) ^ (v >> 31)) as u32 }
/// A UV sphere (radius 50 mm) of about `tris` triangles as the hub's solid frame bytes.
pub fn sphere_frame(tris: usize) -> (Vec<u8>, usize) {
    let rings = ((tris as f32 / 2.0).sqrt() as usize).max(3); let segs = (tris / (2 * rings)).max(3);
    let v = |r: usize, s: usize| -> [i32; 3] {
        let (th, ph) = (std::f32::consts::PI * r as f32 / rings as f32, 2.0 * std::f32::consts::PI * (s % segs) as f32 / segs as f32);
        [(5000.0 * th.sin() * ph.cos()) as i32, (5000.0 * th.sin() * ph.sin()) as i32, (5000.0 * th.cos()) as i32]
    };
    let (mut mesh, mut color, mut p, mut n) = (Vec::new(), 0u32, [0i32; 3], 0usize);
    for r in 0..rings { for s in 0..segs {
        let c = 0x203040u32 + (((r * 255 / rings) as u32) << 16) + (((s * 255 / segs) as u32) << 8);
        for t in [[v(r, s), v(r + 1, s), v(r + 1, s + 1)], [v(r, s), v(r + 1, s + 1), v(r, s + 1)]] {
            put(&mut mesh, c ^ color); color = c;
            for vert in t { for a in 0..3 { put(&mut mesh, zig(vert[a].wrapping_sub(p[a]))); p[a] = vert[a]; } }
            n += 1;
        }
    } }
    let mut f = vec![0xa3];
    let text = |f: &mut Vec<u8>, s: &str| { f.push(0x60 | s.len() as u8); f.extend(s.as_bytes()) };
    text(&mut f, "key"); text(&mut f, "2|demo"); text(&mut f, "n"); f.push(0x1a); f.extend((n as u32).to_be_bytes());
    text(&mut f, "mesh"); f.push(0x5a); f.extend((mesh.len() as u32).to_be_bytes()); f.extend(&mesh);
    (f, n)
}
fn rss_mb() -> f64 { std::fs::read_to_string("/proc/self/statm").ok().and_then(|s| s.split_whitespace().nth(1).and_then(|x| x.parse::<f64>().ok())).map_or(0.0, |p| p * 4096.0 / 1048576.0) }
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); if v.is_empty() { 0.0 } else { v[((v.len() - 1) as f64 * p).round() as usize] } }

pub struct Demo { page: Entity<SolidPage> }
impl Demo {
    pub fn new(cx: &mut Context<Self>, tris: usize, steps: usize, started: Instant) -> Self {
        let page = cx.new(SolidPage::new);
        let (frame, n) = sphere_frame(tris);
        let p2 = page.clone();
        cx.spawn(async move |_, cx| {
            let t0 = Instant::now(); let rss0 = rss_mb();
            cx.background_executor().timer(Duration::from_millis(500)).await; // the window and its size
            let _ = p2.update(cx, |p, _| { let _ = p.accept(frame); });
            let wait = |cx: &mut AsyncApp, target: u64| { let p = p2.clone(); let cx = cx.clone(); async move {
                for _ in 0..5000 { if p.read_with(&cx, |p, _| p.stats.images).unwrap_or(target) >= target { return true } cx.background_executor().timer(Duration::from_millis(2)).await; } false } };
            let ok = wait(cx, 1).await;
            let first_ms = t0.elapsed().as_secs_f64() * 1000.0 - 500.0;
            let (size0, rss1) = (p2.read_with(cx, |p, _| p.stats.last_size).unwrap_or((0, 0)), rss_mb());
            let (mut draft, mut done) = (Vec::new(), 1u64);
            for i in 0..steps {
                let _ = p2.update(cx, |p, _| p.drag_by(5.0, if i % 2 == 0 { 3.0 } else { -3.0 }));
                done += 1; wait(cx, done).await;
                if let Ok(ms) = p2.read_with(cx, |p, _| p.stats.last_ms) { draft.push(ms); }
                cx.background_executor().timer(Duration::from_millis(4)).await;
            }
            let rss2 = rss_mb();
            let metric = serde_json::json!({"event": "solid_demo", "tris": n, "ok": ok, "steps": steps, "panel_image": [size0.0, size0.1],
                "first_image_ms": (first_ms * 10.0).round() / 10.0, "drag_render_ms": {"p50": (pct(&mut draft.clone(), 0.5) * 10.0).round() / 10.0, "p95": (pct(&mut draft.clone(), 0.95) * 10.0).round() / 10.0, "max": (pct(&mut draft, 1.0) * 10.0).round() / 10.0},
                "rss_mb": {"start": rss0.round(), "after_first_image": rss1.round(), "after_drag": rss2.round()}, "drop_image": true,
                "t_ms": started.elapsed().as_millis() as u64});
            eprintln!("GPUI_METRIC {metric}");
            let _ = cx.update(|cx| cx.quit());
        }).detach();
        Demo { page }
    }
}
impl Render for Demo { fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement { div().size_full().bg(rgb(0x0d0f12)).child(self.page.clone()) } }
