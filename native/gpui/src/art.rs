//! The logo and the bots' cats, drawn as vector paths at the window's scale (no raster copies).
//! Logo: src/web/favicon.svg / favicon-light.svg (the web's own files, read at build time).
//! Cats: core/cat's rig (Cat.rig.json), generated on Boat into cat-rigs/<look>-<mood>.json.
use gpui::{prelude::*, *};
use lyon::tessellation::{LineCap, LineJoin};
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

#[derive(Clone, Copy)] pub enum Seg { M(f32, f32), L(f32, f32), C(f32, f32, f32, f32, f32, f32), Q(f32, f32, f32, f32), A(f32, f32, f32, bool, bool, f32, f32), Z }

pub(crate) fn parse_d(d: &str) -> Vec<Seg> {
    // M L H V C S Q A Z, absolute or relative (also comma separated, numbers glued by a sign or a second dot)
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() && ch != 'e' && ch != 'E' { if !cur.is_empty() { toks.push(std::mem::take(&mut cur)); } toks.push(ch.to_string()); }
        else if ch == ',' || ch.is_whitespace() { if !cur.is_empty() { toks.push(std::mem::take(&mut cur)); } }
        else if ch == '-' && !cur.is_empty() && !cur.ends_with('e') { toks.push(std::mem::take(&mut cur)); cur.push(ch); }
        else if ch == '.' && cur.contains('.') { toks.push(std::mem::take(&mut cur)); cur.push(ch); }
        else { cur.push(ch); }
    }
    if !cur.is_empty() { toks.push(cur); }
    let mut out = Vec::new();
    let (mut i, mut cmd) = (0, 'M');
    let (mut x, mut y, mut sx, mut sy) = (0f32, 0f32, 0f32, 0f32);
    let mut last_c: Option<(f32, f32)> = None; // the last cubic's second control point, for S
    let num = |i: usize| toks.get(i).and_then(|t| t.parse::<f32>().ok()).unwrap_or(0.0);
    while i < toks.len() {
        if toks[i].len() == 1 && toks[i].chars().next().unwrap().is_ascii_alphabetic() { cmd = toks[i].chars().next().unwrap(); i += 1; if cmd == 'Z' || cmd == 'z' { out.push(Seg::Z); x = sx; y = sy; } continue; }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        let mut next_c = None;
        match cmd.to_ascii_uppercase() {
            'M' => { x = ox + num(i); y = oy + num(i + 1); sx = x; sy = y; out.push(Seg::M(x, y)); i += 2; cmd = if rel { 'l' } else { 'L' }; }
            'L' => { x = ox + num(i); y = oy + num(i + 1); out.push(Seg::L(x, y)); i += 2; }
            'H' => { x = ox + num(i); out.push(Seg::L(x, y)); i += 1; }
            'V' => { y = oy + num(i); out.push(Seg::L(x, y)); i += 1; }
            'C' => { let (a, b, c2, d2) = (ox + num(i), oy + num(i + 1), ox + num(i + 2), oy + num(i + 3)); x = ox + num(i + 4); y = oy + num(i + 5); out.push(Seg::C(a, b, c2, d2, x, y)); next_c = Some((c2, d2)); i += 6; }
            'S' => { let (a, b) = last_c.map_or((x, y), |(cx, cy)| (2.0 * x - cx, 2.0 * y - cy)); let (c2, d2) = (ox + num(i), oy + num(i + 1)); x = ox + num(i + 2); y = oy + num(i + 3); out.push(Seg::C(a, b, c2, d2, x, y)); next_c = Some((c2, d2)); i += 4; }
            'Q' => { let (a, b) = (ox + num(i), oy + num(i + 1)); x = ox + num(i + 2); y = oy + num(i + 3); out.push(Seg::Q(a, b, x, y)); i += 4; }
            'A' => { let (ex, ey) = (ox + num(i + 5), oy + num(i + 6)); out.push(Seg::A(num(i), num(i + 1), num(i + 2), num(i + 3) != 0.0, num(i + 4) != 0.0, ex, ey)); x = ex; y = ey; i += 7; }
            _ => i += 1,
        }
        last_c = next_c;
    }
    out
}

/// A provider's vendor mark as path segments in its 24x24 viewBox (assets/providers/<provider>.svg), None for a provider without one.
pub fn mark_path(provider: &str) -> Option<&'static [Seg]> {
    static CLAUDE: std::sync::OnceLock<Vec<Seg>> = std::sync::OnceLock::new();
    static OPENCODE: std::sync::OnceLock<Vec<Seg>> = std::sync::OnceLock::new();
    let (cell, svg): (&'static std::sync::OnceLock<Vec<Seg>>, &'static str) = match provider {
        "claude" => (&CLAUDE, include_str!("../../../assets/providers/claude.svg")),
        "opencode" => (&OPENCODE, include_str!("../../../assets/providers/opencode.svg")),
        _ => return None,
    };
    Some(cell.get_or_init(|| parse_d(attr(svg, "<path", "d").unwrap_or(""))).as_slice())
}
/// The mark drawn plain in `color` in a `size` px box (one paint, no repaint loop).
pub fn mark(segs: &'static [Seg], size: f32, color: u32) -> impl IntoElement {
    canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
        let (o, s) = (b.origin, size / 24.0);
        let mut pb = PathBuilder::fill();
        trace(&mut pb, segs, 1.0, |x, y| point(o.x + px(x * s), o.y + px(y * s)));
        if let Ok(path) = pb.build() { window.paint_path(path, rgb(color)); }
    }).size(px(size)).flex_shrink_0()
}

fn rgb_alpha(c: u32, a: f32) -> Rgba { rgba((c << 8) | (a.clamp(0.0, 1.0) * 255.0).round() as u32) }

static RECTS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// GPUI_RECT lines are for the visual checks only: on with --gallery/--swatches/--fonts or BACKPLANE_GPUI_RECTS=1, off in the normal app
/// (no formatting, no lock, no stderr from the render path).
pub fn enable_rects() { RECTS.store(true, std::sync::atomic::Ordering::Relaxed); }
pub fn rects_on() -> bool { RECTS.load(std::sync::atomic::Ordering::Relaxed) }
/// One `GPUI_RECT` line per distinct drawn region (physical px), for the visual check.
pub fn report(name: &str, b: Bounds<Pixels>, scale: f32) {
    if !rects_on() { return; }
    static SEEN: std::sync::Mutex<Option<std::collections::HashSet<String>>> = std::sync::Mutex::new(None);
    let line = format!("{{\"name\":\"{}\",\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1},\"scale\":{}}}", name,
        f32::from(b.origin.x) * scale, f32::from(b.origin.y) * scale, f32::from(b.size.width) * scale, f32::from(b.size.height) * scale, scale);
    let mut g = SEEN.lock().unwrap();
    let set = g.get_or_insert_with(Default::default);
    if set.len() > 4096 { set.clear(); }
    if set.insert(line.clone()) { eprintln!("GPUI_RECT {line}"); }
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
    let l: &'static Logo = if light { LIGHT.get_or_init(|| Logo::parse(include_str!("../../../assets/web/favicon-light.svg"))) }
        else { DARK.get_or_init(|| Logo::parse(include_str!("../../../assets/web/favicon.svg"))) };
    canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
        report(name, b, window.scale_factor());
        let s = size / l.size;
        window.paint_quad(fill(b, rgb(l.bg)));
        let o = b.origin;
        let mut pb = PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::even_odd().with_tolerance(0.02)));
        trace(&mut pb, &l.segs, 1.0, |x, y| point(o.x + px((l.tx + x * l.scale) * s), o.y + px((l.ty + y * l.scale) * s)));
        if let Ok(path) = pb.build() { window.paint_path(path, rgb(l.fg)); }
    }).size(px(size))
}

fn trace(pb: &mut PathBuilder, segs: &[Seg], rs: f32, f: impl Fn(f32, f32) -> Point<Pixels>) {
    for s in segs {
        match *s {
            Seg::M(x, y) => pb.move_to(f(x, y)),
            Seg::L(x, y) => pb.line_to(f(x, y)),
            Seg::C(a, b, c, d, x, y) => pb.cubic_bezier_to(f(x, y), f(a, b), f(c, d)),
            Seg::Q(a, b, x, y) => pb.curve_to(f(x, y), f(a, b)),
            Seg::A(rx, ry, rot, large, sweep, x, y) => pb.arc_to(point(px(rx * rs), px(ry * rs)), px(rot), large, sweep, f(x, y)),
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
            trace(&mut pb, &p.segs, 1.0, f);
            if let Ok(path) = pb.build() { window.paint_path(path, rgb_alpha(c, al)); }
        }
        if let Some(c) = p.stroke {
            let w = p.sw * s;
            let mut pb = PathBuilder::stroke(px(w)).with_style(PathStyle::Stroke(StrokeOptions::default().with_line_width(w).with_line_cap(LineCap::Round).with_line_join(LineJoin::Round).with_tolerance(0.02)));
            trace(&mut pb, &p.segs, 1.0, f);
            if let Ok(path) = pb.build() { window.paint_path(path, rgb_alpha(c, al)); }
        }
    }
}

// ---- fonts: JetBrains Mono (SIL OFL, assets/fonts/jetbrains-mono), bundled for code and mono labels, ligatures off ----
pub const MONO: &str = "JetBrains Mono";
static MONO_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf");
static MONO_BOLD: &[u8] = include_bytes!("../../../assets/fonts/jetbrains-mono/JetBrainsMono-Bold.ttf");
/// Registers the bundled faces; logs GPUI_LOG {"font":...,"loaded":bool} (loudly false when the family did not appear).
pub fn load_fonts(cx: &mut App) {
    let added = cx.text_system().add_fonts(vec![std::borrow::Cow::Borrowed(MONO_REGULAR), std::borrow::Cow::Borrowed(MONO_BOLD)]);
    let loaded = added.is_ok() && cx.text_system().all_font_names().iter().any(|n| n == MONO);
    match added { Ok(()) => eprintln!("GPUI_LOG {{\"font\":\"{MONO}\",\"loaded\":{loaded}}}"), Err(e) => eprintln!("GPUI_LOG {{\"font\":\"{MONO}\",\"loaded\":false,\"error\":{}}}", Value::String(format!("{e:#}"))) }
}
/// JetBrains Mono with ligatures (calt) off; CJK and anything it lacks falls back to the system font.
pub fn mono_font() -> Font {
    Font { family: MONO.into(), features: FontFeatures::disable_ligatures(), fallbacks: None, weight: FontWeight::NORMAL, style: FontStyle::Normal }
}

// ---- generated assets (cat rigs, icons): found beside the binary, loud when missing --------------------
/// BACKPLANE_GPUI_ASSETS, else `assets/` beside the binary (try.sh copies it); debug builds also try the repo's build folder.
pub fn assets_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let mut c: Vec<PathBuf> = Vec::new();
        if let Some(d) = std::env::var_os("BACKPLANE_GPUI_ASSETS") { c.push(PathBuf::from(d)); }
        if let Some(p) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("assets"))) { c.push(p); }
        #[cfg(debug_assertions)] // a debug build run from the repo; a release build never looks outside its own folder
        c.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/gpui/linux/assets"));
        c.iter().find(|d| d.join("icons.json").is_file()).cloned().unwrap_or_else(|| c.into_iter().next().unwrap_or_default())
    }).clone()
}
/// A message when the generated assets are not there; logged as GPUI_LOG and shown in the window.
pub fn assets_missing() -> Option<String> {
    let d = assets_dir();
    let mut missing = Vec::new();
    if !d.join("icons.json").is_file() { missing.push("icons.json"); }
    if !d.join("cat-rigs").is_dir() { missing.push("cat-rigs/"); }
    if missing.is_empty() { return None; }
    let m = format!("assets missing in {}: {}", d.display(), missing.join(", "));
    eprintln!("GPUI_LOG {{\"asset_missing\":{}}}", Value::String(m.clone()));
    Some(m)
}
fn log_once(what: String) {
    static SEEN: std::sync::Mutex<Option<std::collections::HashSet<String>>> = std::sync::Mutex::new(None);
    if SEEN.lock().unwrap().get_or_insert_with(Default::default).insert(what.clone()) { eprintln!("GPUI_LOG {{\"asset_missing\":{}}}", Value::String(what)); }
}

struct IconLayer { segs: Vec<Seg>, fill: bool }
fn icons() -> &'static HashMap<String, Vec<IconLayer>> {
    static ICONS: std::sync::OnceLock<HashMap<String, Vec<IconLayer>>> = std::sync::OnceLock::new();
    ICONS.get_or_init(|| {
        let path = assets_dir().join("icons.json");
        let v: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        v.as_object().into_iter().flatten().map(|(k, layers)| (k.clone(), layers.as_array().into_iter().flatten()
            .map(|l| IconLayer { segs: parse_d(l["d"].as_str().unwrap_or("")), fill: l["fill"] == true }).collect())).collect()
    })
}
/// A Lucide icon as vector paths on its 24 x 24 grid: a 2-unit round stroke, or a filled layer. `size` px square.
pub fn icon(name: &'static str, size: f32, color: Rgba) -> AnyElement {
    let Some(layers) = icons().get(name) else {
        log_once(format!("icon {name} not in icons.json"));
        return div().size(px(size)).into_any_element();
    };
    canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
        if rects_on() { report(&format!("icon:{name}"), b, window.scale_factor()); }
        let s = size / 24.0; let o = b.origin;
        for l in layers {
            let f = |x: f32, y: f32| point(o.x + px(x * s), o.y + px(y * s));
            if l.fill {
                let mut pb = PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::even_odd().with_tolerance(0.02)));
                trace(&mut pb, &l.segs, s, f);
                if let Ok(p) = pb.build() { window.paint_path(p, color); }
            } else {
                let w = 2.0 * s;
                let mut pb = PathBuilder::stroke(px(w)).with_style(PathStyle::Stroke(StrokeOptions::default().with_line_width(w).with_line_cap(LineCap::Round).with_line_join(LineJoin::Round).with_tolerance(0.02)));
                trace(&mut pb, &l.segs, s, f);
                if let Ok(p) = pb.build() { window.paint_path(p, color); }
            }
        }
    }).size(px(size)).into_any_element()
}

/// Rigs by (look, mood): loaded once from the generated folder, at most 64 kept.
pub struct Cats { dir: PathBuf, cache: HashMap<(u32, String), Option<Arc<Rig>>> }
impl Cats {
    pub fn new() -> Self { Cats { dir: assets_dir().join("cat-rigs"), cache: HashMap::new() } }
    pub fn rig(&mut self, seed: u64, mood: &str) -> Option<Arc<Rig>> {
        let look = (seed % 72) as u32;
        if self.cache.len() > 64 { self.cache.clear(); }
        let moving = matches!(mood, "work" | "think" | "wait" | "talk");
        let dir = self.dir.clone();
        let r = self.cache.entry((look, mood.to_string())).or_insert_with(|| load_rig(&dir, look, mood, moving).map(Arc::new)).clone();
        if r.is_none() { log_once(format!("cat rig {look}-{mood} missing in {}", dir.display())); }
        r
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

// ---- layout probes (G3): the rectangle of a stable region, in check mode only -------------------
static LAYOUT: std::sync::Mutex<Option<std::collections::BTreeMap<String, [f32; 4]>>> = std::sync::Mutex::new(None);
/// An invisible overlay that records its parent's rectangle when painted (physical px); does nothing outside check mode.
pub fn probe(name: &'static str) -> AnyElement {
    canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
        if !rects_on() { return; }
        let s = window.scale_factor();
        LAYOUT.lock().unwrap().get_or_insert_with(Default::default).insert(name.to_string(), [f32::from(b.origin.x) * s, f32::from(b.origin.y) * s, f32::from(b.size.width) * s, f32::from(b.size.height) * s]);
    }).absolute().size_full().into_any_element()
}
/// A probe's region is gone (an overlay shut): its rectangle leaves the snapshot.
pub fn forget(name: &str) { if rects_on() { if let Some(m) = LAYOUT.lock().unwrap().as_mut() { m.remove(name); } } }
/// The recorded rectangles as a JSON object {name:{x,y,w,h}}.
pub fn layout_snapshot() -> String {
    let g = LAYOUT.lock().unwrap();
    let m: serde_json::Map<String, Value> = g.iter().flatten().map(|(k, r)| (k.clone(), serde_json::json!({"x": r[0], "y": r[1], "w": r[2], "h": r[3]}))).collect();
    Value::Object(m).to_string()
}

// ---- control rectangles (G6): the window rectangle of each registered control, in check mode only ------------
static CTL_RECTS: std::sync::Mutex<Option<std::collections::HashMap<String, [f32; 4]>>> = std::sync::Mutex::new(None);
/// An invisible overlay recording its parent control's rectangle (physical window px) under the control's id.
pub fn probe_rect(id: String) -> AnyElement {
    canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
        let s = window.scale_factor();
        let mut g = CTL_RECTS.lock().unwrap();
        let m = g.get_or_insert_with(Default::default);
        if m.len() > 2048 { m.clear(); }
        m.insert(id, [f32::from(b.origin.x) * s, f32::from(b.origin.y) * s, f32::from(b.size.width) * s, f32::from(b.size.height) * s]);
    }).absolute().top_0().left_0().size_full().into_any_element()
}
pub fn ctl_rects() -> std::collections::HashMap<String, [f32; 4]> { CTL_RECTS.lock().unwrap().clone().unwrap_or_default() }

#[cfg(test)]
mod generated_assets_tests {
    use std::path::Path;
    /// Every icon the client names in its code is in the repo's assets/icons.json, and every cat rig is there (assets/GENERATED.md): a release
    /// archive carries these files as they are, and without them the client draws empty boxes ("icon X not in icons.json").
    #[test] fn the_generated_assets_cover_what_the_client_draws() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let icons: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root.join("icons.json")).expect("assets/icons.json")).expect("icons.json is JSON");
        let mut used = std::collections::BTreeSet::new(); let mut todo = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(d) = todo.pop() { for e in std::fs::read_dir(&d).unwrap().flatten() { let p = e.path();
            if p.is_dir() { todo.push(p); continue } if p.extension().map_or(true, |x| x != "rs") { continue }
            let text = std::fs::read_to_string(&p).unwrap();
            for (at, w) in text.match_indices("ic(").chain(text.match_indices("icon(")) {
                // the call itself, not a word ending in it (metric(, topic( ...)
                if text[..at].chars().last().map_or(false, |c| c.is_alphanumeric() || c == '_') { continue }
                let rest = &text[at + w.len()..text.len().min(at + 48)]; let Some(q) = rest.find('"') else { continue };
                // the first argument (icon("x") or ic(light, "x")), not a colour further on (c("accent"))
                let pre = rest[..q].trim(); if !(pre.is_empty() || (pre.ends_with(',') && pre[..pre.len() - 1].chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.'))) { continue }
                let name: String = rest[q + 1..].chars().take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-').collect();
                if !name.is_empty() && rest[q + 1 + name.len()..].starts_with('"') { used.insert(name); }
            } } }
        assert!(used.len() >= 10, "the scan found the icon names: {used:?}");
        let missing: Vec<_> = used.iter().filter(|n| icons.get(n.as_str()).is_none()).collect();
        assert!(missing.is_empty(), "icons the client names but assets/icons.json lacks: {missing:?}");
        let rigs = std::fs::read_dir(root.join("cat-rigs")).expect("assets/cat-rigs").count(); assert_eq!(rigs, 72 * 8, "a rig for every look and mood");
    }
}
