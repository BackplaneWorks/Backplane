//! Layout audit (Backplane patch): what a frame painted, for the size-matrix e2e. Off unless `enable()` is called.
//! Text runs, named `div`s and opaque quads are recorded in logical pixels in paint order; an opaque quad drops the earlier text it covers,
//! so a modal over the timeline does not read as overlapping text. `take()` returns the last complete frame as JSON.
use crate::{Bounds, ContentMask, Pixels};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(false);
static ST: Mutex<St> = Mutex::new(St { cur: Vec::new(), last: String::new(), win: [0.0; 3] });
struct St { cur: Vec<Rec>, last: String, win: [f32; 3] }
enum Rec {
    Text { text: String, b: [f32; 4], el: [f32; 4], mask: [f32; 4], occ: Vec<[f32; 4]> },
    Div { id: String, b: [f32; 4], mask: [f32; 4] },
}
fn r(b: &Bounds<Pixels>) -> [f32; 4] { [f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height)] }
fn m(c: &ContentMask<Pixels>) -> [f32; 4] { r(&c.bounds) }
fn inter(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let w = (a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0]);
    let h = (a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1]);
    if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
}
/// Turn recording on (the client does it for BACKPLANE_GPUI_AUDIT=1).
pub fn enable() { ON.store(true, Ordering::Relaxed); }
/// Whether recording is on.
pub fn on() -> bool { ON.load(Ordering::Relaxed) }
pub(crate) fn begin() { if on() { ST.lock().unwrap().cur.clear(); } }
pub(crate) fn end(w: f32, h: f32, scale: f32) {
    if !on() { return; }
    let mut g = ST.lock().unwrap();
    let recs = std::mem::take(&mut g.cur);
    let f = |x: f32| (x * 10.0).round() / 10.0;
    let list: Vec<serde_json::Value> = recs.iter().map(|rec| match rec {
        Rec::Text { text, b, el, mask, occ } => serde_json::json!({"k": "t", "s": text, "b": b.map(f), "el": el.map(f), "mask": mask.map(f), "occ": occ.iter().map(|o| o.map(f)).collect::<Vec<_>>()}),
        Rec::Div { id, b, mask } => serde_json::json!({"k": "d", "id": id, "b": b.map(f), "mask": mask.map(f)}),
    }).collect();
    g.win = [w, h, scale];
    g.last = serde_json::json!({"win": [w, h], "scale": scale, "recs": list}).to_string();
}
pub(crate) fn text(text: &str, b: Bounds<Pixels>, width: f32, height: f32, mask: &ContentMask<Pixels>) {
    if !on() || text.trim().is_empty() { return; }
    let t: String = text.chars().take(60).collect();
    let tb = [f32::from(b.origin.x), f32::from(b.origin.y), width, height];
    ST.lock().unwrap().cur.push(Rec::Text { text: t, b: tb, el: r(&b), mask: m(mask), occ: Vec::new() });
}
pub(crate) fn div(id: &str, b: Bounds<Pixels>, mask: &ContentMask<Pixels>) {
    if !on() { return; }
    ST.lock().unwrap().cur.push(Rec::Div { id: id.to_string(), b: r(&b), mask: m(mask) });
}
/// An opaque quad covers the text painted before it.
pub(crate) fn quad(b: Bounds<Pixels>, mask: &ContentMask<Pixels>, opaque: bool) {
    if !on() || !opaque { return; }
    let (q, mk) = (r(&b), m(mask));
    let w = (q[0] + q[2]).min(mk[0] + mk[2]) - q[0].max(mk[0]);
    let h = (q[1] + q[3]).min(mk[1] + mk[3]) - q[1].max(mk[1]);
    if w <= 0.0 || h <= 0.0 { return; }
    let q = [q[0].max(mk[0]), q[1].max(mk[1]), w, h];
    let mut g = ST.lock().unwrap();
    g.cur.retain(|rec| match rec { Rec::Text { b, .. } => { let a = (b[2] * b[3]).max(0.01); inter(b, &q) / a < 0.85 } _ => true });
    // a text only partly under the box keeps the box: an overlap inside it is hidden, not drawn
    for rec in g.cur.iter_mut() { if let Rec::Text { b, occ, .. } = rec { if inter(b, &q) > 0.0 && occ.len() < 8 { occ.push(q); } } }
}
/// The last complete frame as JSON {"win":[w,h],"scale":s,"recs":[{"k":"t","s":text,"b":[x,y,w,h],"el":[..],"mask":[..],"occ":[[..]]} | {"k":"d","id":..,"b":..,"mask":..}]}.
pub fn take() -> String { ST.lock().unwrap().last.clone() }
