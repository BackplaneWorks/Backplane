//! The logo and the bots' cats, drawn as vector paths at the window's scale (no raster copies).
//! Logo: src/web/favicon.svg / favicon-light.svg (the web's own files, read at build time).
//! Cats: core/cat.bend's rig (Cat.rig.json), generated on Boat into cat-rigs/<look>-<mood>.json.
use gpui::{prelude::*, *};
use lyon::tessellation::{LineCap, LineJoin};
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

#[derive(Clone, Copy)] enum Seg { M(f32, f32), L(f32, f32), C(f32, f32, f32, f32, f32, f32), Q(f32, f32, f32, f32), Z }

fn parse_d(d: &str) -> Vec<Seg> {
    // absolute M L C Q Z (also comma separated, and "L" written with a leading letter glued to a number)
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() { if !cur.is_empty() { toks.push(std::mem::take(&mut cur)); } toks.push(ch.to_string()); }
        else if ch == ',' || ch.is_whitespace() { if !cur.is_empty() { toks.push(std::mem::take(&mut cur)); } }
        else if ch == '-' && !cur.is_empty() && !cur.ends_with('e') { toks.push(std::mem::take(&mut cur)); cur.push(ch); }
        else { cur.push(ch); }
    }
    if !cur.is_empty() { toks.push(cur); }
    let mut out = Vec::new();
    let (mut i, mut cmd) = (0, 'M');
    let num = |i: usize| toks.get(i).and_then(|t| t.parse::<f32>().ok()).unwrap_or(0.0);
    while i < toks.len() {
        if toks[i].len() == 1 && toks[i].chars().next().unwrap().is_ascii_alphabetic() { cmd = toks[i].chars().next().unwrap(); i += 1; if cmd == 'Z' || cmd == 'z' { out.push(Seg::Z); } continue; }
        match cmd {
            'M' => { out.push(Seg::M(num(i), num(i + 1))); i += 2; cmd = 'L'; }
            'L' => { out.push(Seg::L(num(i), num(i + 1))); i += 2; }
            'C' => { out.push(Seg::C(num(i), num(i + 1), num(i + 2), num(i + 3), num(i + 4), num(i + 5))); i += 6; }
            'Q' => { out.push(Seg::Q(num(i), num(i + 1), num(i + 2), num(i + 3))); i += 4; }
            _ => i += 1,
        }
    }
    out
}

fn rgb_alpha(c: u32, a: f32) -> Rgba { rgba((c << 8) | (a.clamp(0.0, 1.0) * 255.0).round() as u32) }

/// One `GPUI_RECT` line per distinct drawn region (physical px), for the visual check.
fn report(name: &str, b: Bounds<Pixels>, scale: f32) {
    static SEEN: std::sync::Mutex<Option<std::collections::HashSet<String>>> = std::sync::Mutex::new(None);
    let line = format!("{{\"name\":\"{}\",\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1},\"scale\":{}}}", name,
        f32::from(b.origin.x) * scale, f32::from(b.origin.y) * scale, f32::from(b.size.width) * scale, f32::from(b.size.height) * scale, scale);
    let mut g = SEEN.lock().unwrap();
    if g.get_or_insert_with(Default::default).insert(line.clone()) { eprintln!("GPUI_RECT {line}"); }
}

// ---- logo -------------------------------------------------------------------------------------
pub struct Logo { bg: u32, fg: u32, segs: Vec<Seg>, tx: f32, ty: f32, scale: f32, size: f32 }
fn attr<'a>(svg: &'a str, from: &str, name: &str) -> Option<&'a str> {
    let at = svg.find(from)?; let rest = &svg[at..]; let key = format!("{name}=\""); let s = rest.find(&key)? + key.len();
    Some(&rest[s..s + rest[s..].find('"')?])
}
fn hex(s: &str) -> u32 { u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0) }
impl Logo {
    fn parse(svg: &str) -> Logo {
        let tr = attr(svg, "<path", "transform").unwrap_or("");
        let nums: Vec<f32> = tr.replace("translate(", " ").replace("scale(", " ").replace(')', " ").split_whitespace().filter_map(|t| t.parse().ok()).collect();
        Logo { bg: hex(attr(svg, "<rect", "fill").unwrap_or("#000000")), fg: hex(attr(svg, "<path", "fill").unwrap_or("#ffffff")),
            segs: parse_d(attr(svg, "<path", "d").unwrap_or("")), tx: *nums.first().unwrap_or(&0.0), ty: *nums.get(1).unwrap_or(&0.0), scale: *nums.get(2).unwrap_or(&1.0),
            size: attr(svg, "<svg", "width").and_then(|w| w.parse().ok()).unwrap_or(256.0) }
    }
}
pub fn logo(light: bool, size: f32, name: &'static str) -> impl IntoElement {
    static DARK: std::sync::OnceLock<Logo> = std::sync::OnceLock::new();
    static LIGHT: std::sync::OnceLock<Logo> = std::sync::OnceLock::new();
    let l: &'static Logo = if light { LIGHT.get_or_init(|| Logo::parse(include_str!("../../../src/web/favicon-light.svg"))) }
        else { DARK.get_or_init(|| Logo::parse(include_str!("../../../src/web/favicon.svg"))) };
    canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
        report(name, b, window.scale_factor());
        let s = size / l.size;
        window.paint_quad(fill(b, rgb(l.bg)));
        let o = b.origin;
        let mut pb = PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::even_odd().with_tolerance(0.02)));
        trace(&mut pb, &l.segs, |x, y| point(o.x + px((l.tx + x * l.scale) * s), o.y + px((l.ty + y * l.scale) * s)));
        if let Ok(path) = pb.build() { window.paint_path(path, rgb(l.fg)); }
    }).size(px(size))
}

fn trace(pb: &mut PathBuilder, segs: &[Seg], f: impl Fn(f32, f32) -> Point<Pixels>) {
    for s in segs {
        match *s {
            Seg::M(x, y) => pb.move_to(f(x, y)),
            Seg::L(x, y) => pb.line_to(f(x, y)),
            Seg::C(a, b, c, d, x, y) => pb.cubic_bezier_to(f(x, y), f(a, b), f(c, d)),
            Seg::Q(a, b, x, y) => pb.curve_to(f(x, y), f(a, b)),
            Seg::Z => pb.close(),
        }
    }
}

// ---- cats -------------------------------------------------------------------------------------
struct Anim { kind: u8, from: f32, to: f32, px: f32, py: f32, per: f32, delay: f32, ease: u8 }
struct Part { segs: Vec<Seg>, fill: Option<u32>, stroke: Option<u32>, sw: f32, alpha: f32, anims: Vec<Anim> }
pub struct Rig { parts: Vec<Part>, pub moving: bool }

fn color(v: &Value) -> Option<u32> { v.as_u64().map(|n| n as u32) }
fn load_rig(dir: &PathBuf, look: u32, mood: &str, moving: bool) -> Option<Rig> {
    let text = std::fs::read_to_string(dir.join(format!("{look}-{mood}.json"))).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let parts = v.as_array()?.iter().map(|p| Part {
        segs: parse_d(p["d"].as_str().unwrap_or("")), fill: color(&p["fill"]), stroke: color(&p["stroke"]),
        sw: p["sw"].as_f64().unwrap_or(1.0) as f32, alpha: p["alpha"].as_f64().unwrap_or(255.0) as f32 / 255.0,
        anims: p["anims"].as_array().map(|a| a.iter().map(|x| Anim {
            kind: match x["k"].as_str().unwrap_or("") { "rot" => 1, "tx" => 2, "ty" => 3, "sx" => 4, "sy" => 5, _ => 6 },
            from: x["from"].as_f64().unwrap_or(0.0) as f32, to: x["to"].as_f64().unwrap_or(0.0) as f32,
            px: x["px"].as_f64().unwrap_or(0.0) as f32, py: x["py"].as_f64().unwrap_or(0.0) as f32,
            per: (x["per"].as_f64().unwrap_or(1000.0) as f32).max(1.0), delay: x["delay"].as_f64().unwrap_or(0.0) as f32,
            ease: match x["ease"].as_str().unwrap_or("io") { "blink" => 1, "saw" => 2, _ => 0 } }).collect()).unwrap_or_default(),
    }).collect();
    Some(Rig { parts, moving })
}

fn ease(a: &Anim, ms: f32) -> f32 {
    let t = ((ms - a.delay) % a.per + a.per) % a.per / a.per;
    let w = |x: f32| (1.0 - (std::f32::consts::TAU * x).cos()) / 2.0;
    let e = match a.ease { 1 => if t < 0.94 { 0.0 } else { w((t - 0.94) / 0.06) }, 2 => t, _ => w(t) };
    a.from + (a.to - a.from) * e
}
/// A point of a part drawn at A1(A2(...An(p))): the last animation applies first.
fn pose(anims: &[Anim], ms: f32, mut x: f32, mut y: f32) -> (f32, f32) {
    for a in anims.iter().rev() {
        let v = ease(a, ms);
        match a.kind {
            1 => { let (s, c) = v.to_radians().sin_cos(); let (dx, dy) = (x - a.px, y - a.py); x = a.px + dx * c - dy * s; y = a.py + dx * s + dy * c; }
            2 => x += v, 3 => y += v,
            4 => x = a.px + (x - a.px) * v, 5 => y = a.py + (y - a.py) * v,
            _ => {}
        }
    }
    (x, y)
}
fn opacity(anims: &[Anim], ms: f32) -> f32 { anims.iter().filter(|a| a.kind == 6).map(|a| ease(a, ms)).product() }

pub fn paint_cat(window: &mut Window, b: Bounds<Pixels>, rig: &Rig, ms: f32, size: f32, gain: f32) {
    let s = size / 64.0; let o = b.origin;
    for p in &rig.parts {
        let al = p.alpha * opacity(&p.anims, ms) * gain;
        let f = |x: f32, y: f32| { let (x, y) = pose(&p.anims, ms, x, y); point(o.x + px(x * s), o.y + px(y * s)) };
        if let Some(c) = p.fill {
            let mut pb = PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::even_odd().with_tolerance(0.02)));
            trace(&mut pb, &p.segs, f);
            if let Ok(path) = pb.build() { window.paint_path(path, rgb_alpha(c, al)); }
        }
        if let Some(c) = p.stroke {
            let w = p.sw * s;
            let mut pb = PathBuilder::stroke(px(w)).with_style(PathStyle::Stroke(StrokeOptions::default().with_line_width(w).with_line_cap(LineCap::Round).with_line_join(LineJoin::Round).with_tolerance(0.02)));
            trace(&mut pb, &p.segs, f);
            if let Ok(path) = pb.build() { window.paint_path(path, rgb_alpha(c, al)); }
        }
    }
}

/// Rigs by (look, mood): loaded once from the generated folder, at most 64 kept.
pub struct Cats { dir: PathBuf, cache: HashMap<(u32, String), Option<Arc<Rig>>> }
impl Cats {
    pub fn new() -> Self {
        // BACKPLANE_GPUI_CATS, else cat-rigs/ beside the binary, else the repo's build folder (a copied binary, as try.sh makes)
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(d) = std::env::var_os("BACKPLANE_GPUI_CATS") { candidates.push(PathBuf::from(d)); }
        if let Some(p) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("cat-rigs"))) { candidates.push(p); }
        if let Some(h) = std::env::var_os("HOME") { candidates.push(PathBuf::from(h).join("Documents/i2cjak/backplane-gpui/build/gpui/linux/cat-rigs")); }
        let dir = candidates.iter().find(|d| d.is_dir()).cloned().unwrap_or_default();
        Cats { dir, cache: HashMap::new() }
    }
    pub fn rig(&mut self, seed: u64, mood: &str) -> Option<Arc<Rig>> {
        let look = (seed % 72) as u32;
        if self.cache.len() > 64 { self.cache.clear(); }
        let moving = matches!(mood, "work" | "think" | "wait" | "talk");
        let dir = self.dir.clone();
        self.cache.entry((look, mood.to_string())).or_insert_with(|| load_rig(&dir, look, mood, moving).map(Arc::new)).clone()
    }
}
/// A cat, `size` px square. Only a moving mood asks for another frame, so an idle window never repaints.
pub fn cat(rig: Option<Arc<Rig>>, mood: &str, size: f32, started: std::time::Instant, name: String) -> AnyElement {
    let away = mood == "away";
    match rig {
        Some(rig) => canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
            report(&name, b, window.scale_factor());
            let ms = if rig.moving { started.elapsed().as_secs_f32() * 1000.0 } else { 0.0 };
            paint_cat(window, b, &rig, ms, size, if away { 0.5 } else { 1.0 });
            if rig.moving { window.request_animation_frame(); }
        }).size(px(size)).into_any_element(),
        None => div().size(px(size)).into_any_element(),
    }
}
