//! The drawing pad's model and picture: pure, no window. Ported from the old client's board (src/web/draw.js on the tag `main-bend-final`):
//! boxes that are dragged out, moved, resized and given typed lengths, each new one taking the next colour. The pen and the eraser are new.
//! The screen shows `paint(true)` and the attachment is `paint(false)`: one rasterizer, so what is attached is what was drawn.

pub const COLORS: [u32; 7] = [0xe5484d, 0xf5a524, 0x3e9b4f, 0x2f80ed, 0x8e4ec6, 0x12a594, 0x1f2328];
pub const W: usize = 960;
pub const H: usize = 600; // the paper, in its own pixels
pub const UNIT: f32 = 4.; // paper pixels per unit of a typed length
const EDGE: f32 = 7.; // how near a side counts as on it
const MIN: f32 = 6.; // a smaller drag is a click, not a box
const LINE: f32 = 3.; // the width of a pen stroke and of a box's outline
const STEP: f32 = 3.; // the largest gap between a stroke's points, so the eraser can cut it anywhere
const ERASER: f32 = 12.; // the eraser's radius
const UNDO_MAX: usize = 100;
const INK: u32 = 0x1f2328; // selection marks

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool { Pen, Boxes, Eraser }
/// Which part of an item a point is on: a side, or its middle.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Part { L, R, T, B, In }
#[derive(Clone, PartialEq, Debug)]
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32, pub color: u32, pub dim_w: String, pub dim_h: String }
#[derive(Clone, PartialEq, Debug)]
pub struct Stroke { pub pts: Vec<(f32, f32)>, pub color: u32 }
#[derive(Clone, PartialEq, Debug)]
pub enum Item { Rect(Rect), Stroke(Stroke) }
/// A typed length: 40 and "mm".
#[derive(Clone, PartialEq, Debug)]
pub struct Length { pub n: f32, pub unit: String }

fn near(a: f32, b: f32, edge: f32) -> bool { (a - b).abs() <= edge }

/// Which part of `r` a point is on, if any.
pub fn hit(r: &Rect, x: f32, y: f32) -> Option<Part> {
    if x < r.x - EDGE || x > r.x + r.w + EDGE || y < r.y - EDGE || y > r.y + r.h + EDGE { return None; }
    let (in_x, in_y) = (x >= r.x && x <= r.x + r.w, y >= r.y && y <= r.y + r.h);
    let sides = [(Part::L, in_y, (x - r.x).abs()), (Part::R, in_y, (x - r.x - r.w).abs()), (Part::T, in_x, (y - r.y).abs()), (Part::B, in_x, (y - r.y - r.h).abs())];
    let best = sides.iter().filter(|s| s.1).min_by(|a, b| a.2.total_cmp(&b.2));
    if let Some(s) = best { if s.2 <= EDGE { return Some(s.0); } }
    (x > r.x && x < r.x + r.w && y > r.y && y < r.y + r.h).then_some(Part::In)
}
fn on_stroke(s: &Stroke, x: f32, y: f32) -> bool { s.pts.iter().any(|p| near(p.0, x, EDGE) && near(p.1, y, EDGE)) }
/// The topmost item under a point, and the part of it.
pub fn pick(items: &[Item], x: f32, y: f32) -> Option<(usize, Part)> {
    items.iter().enumerate().rev().find_map(|(i, it)| match it { Item::Rect(r) => hit(r, x, y).map(|p| (i, p)), Item::Stroke(s) => on_stroke(s, x, y).then_some((i, Part::In)) })
}
/// "40", "40 mm", "12.5cm"; anything else is none.
pub fn parse_length(text: &str) -> Option<Length> {
    let t = text.trim();
    let end = t.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(t.len());
    let (num, rest) = t.split_at(end);
    let unit = rest.trim_start();
    if num.starts_with('.') || num.ends_with('.') || unit.chars().count() > 4 || !unit.chars().all(|c| c.is_ascii_alphabetic() || matches!(c, 'µ' | '"' | '\'')) { return None; }
    let n: f32 = num.parse().ok()?;
    (n > 0.).then(|| Length { n, unit: unit.to_string() })
}
/// A box with one side's length set: the side's axis takes n units, the opposite side stays, and the label is kept for the picture.
pub fn with_length(r: &Rect, side: Part, len: &Length) -> Rect {
    let px = (len.n * UNIT).round().max(MIN);
    let label = if len.unit.is_empty() { len.n.to_string() } else { format!("{} {}", len.n, len.unit) };
    let mut o = r.clone();
    match side {
        Part::L => { o.x = r.x + r.w - px; o.w = px; o.dim_w = label; }
        Part::R => { o.w = px; o.dim_w = label; }
        Part::T => { o.y = r.y + r.h - px; o.h = px; o.dim_h = label; }
        Part::B => { o.h = px; o.dim_h = label; }
        Part::In => {}
    }
    o
}
/// A side dragged to (x, y): the box with that side there, never flipped inside out; a typed length on that axis no longer holds.
pub fn resize(r: &Rect, side: Part, x: f32, y: f32) -> Rect {
    let mut o = r.clone();
    match side {
        Part::L => { let right = r.x + r.w; o.x = x.min(right - MIN); o.w = right - o.x; o.dim_w.clear(); }
        Part::R => { o.w = (x - r.x).max(MIN); o.dim_w.clear(); }
        Part::T => { let bottom = r.y + r.h; o.y = y.min(bottom - MIN); o.h = bottom - o.y; o.dim_h.clear(); }
        Part::B => { o.h = (y - r.y).max(MIN); o.dim_h.clear(); }
        Part::In => {}
    }
    o
}
fn clamp(x: f32, y: f32) -> (f32, f32) { (x.clamp(0., W as f32), y.clamp(0., H as f32)) }
fn bounds(it: &Item) -> (f32, f32, f32, f32) {
    match it {
        Item::Rect(r) => (r.x, r.y, r.w, r.h),
        Item::Stroke(s) => {
            let (mut a, mut b, mut c, mut d) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for p in &s.pts { a = a.min(p.0); b = b.min(p.1); c = c.max(p.0); d = d.max(p.1); }
            (a, b, c - a, d - b)
        }
    }
}
/// The points from `a` to `b`, `b` included, no further apart than STEP.
fn walk(a: (f32, f32), b: (f32, f32)) -> Vec<(f32, f32)> {
    let n = ((b.0 - a.0).hypot(b.1 - a.1) / STEP).ceil().max(1.) as usize;
    (1..=n).map(|i| { let t = i as f32 / n as f32; (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t) }).collect()
}
/// What is left of `items` after the eraser touched (x, y): a box it touches goes, a stroke is cut around it. None when nothing changed.
fn erased(items: &[Item], x: f32, y: f32) -> Option<Vec<Item>> {
    let touched = |p: &(f32, f32)| (p.0 - x).hypot(p.1 - y) <= ERASER;
    let (mut out, mut changed) = (Vec::with_capacity(items.len()), false);
    for it in items {
        match it {
            Item::Rect(r) if x >= r.x - ERASER && x <= r.x + r.w + ERASER && y >= r.y - ERASER && y <= r.y + r.h + ERASER => changed = true,
            Item::Stroke(s) if s.pts.iter().any(touched) => {
                changed = true;
                // what stays is the runs of points the eraser missed; a lone point is a crumb, not a stroke
                for run in s.pts.split(touched).filter(|r| r.len() > 1) { out.push(Item::Stroke(Stroke { pts: run.to_vec(), color: s.color })); }
            }
            _ => out.push(it.clone()),
        }
    }
    changed.then_some(out)
}

#[derive(Clone, PartialEq, Debug)]
enum Drag { New { x0: f32, y0: f32 }, Pen, Erase { kept: bool, at: (f32, f32) }, Item { i: usize, part: Part, x0: f32, y0: f32, from: Item, moved: bool } }

pub struct Pad {
    pub items: Vec<Item>,
    pub tool: Tool,
    /// The colour the next stroke or box takes, an index into COLORS that may run past its end.
    pub next: usize,
    /// A colour was chosen by hand: new boxes keep it instead of taking the next.
    pub pinned: bool,
    pub picked: Option<usize>,
    /// The side being given a length.
    pub editing: Option<(usize, Part)>,
    /// The paper changed since the last `paint`.
    pub dirty: bool,
    undo: Vec<(Vec<Item>, usize)>,
    hover: Option<(usize, Part)>,
    drag: Option<Drag>,
    draft: Option<Rect>,
    ink: Option<Stroke>,
    pointer: Option<(f32, f32)>,
}
impl Default for Pad {
    fn default() -> Self { Self { items: vec![], tool: Tool::Pen, next: 0, pinned: false, picked: None, editing: None, dirty: true, undo: vec![], hover: None, drag: None, draft: None, ink: None, pointer: None } }
}
impl Pad {
    pub fn color(&self) -> u32 { COLORS[self.next % COLORS.len()] }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
    pub fn dragging(&self) -> bool { self.drag.is_some() }
    fn keep(&mut self) { self.undo.push((self.items.clone(), self.next)); if self.undo.len() > UNDO_MAX { self.undo.remove(0); } }
    pub fn set_tool(&mut self, tool: Tool) { self.tool = tool; self.picked = None; self.hover = None; self.pointer = None; self.dirty = true; }
    pub fn set_color(&mut self, i: usize) { self.next = i % COLORS.len(); self.pinned = true; self.dirty = true; }

    /// The mouse went down on the paper at (x, y).
    pub fn down(&mut self, x: f32, y: f32) {
        let (x, y) = clamp(x, y);
        self.picked = None;
        self.drag = Some(match self.tool {
            Tool::Pen => { self.ink = Some(Stroke { pts: vec![(x, y)], color: self.color() }); Drag::Pen }
            Tool::Eraser => { let mut d = Drag::Erase { kept: false, at: (x, y) }; self.erase_to(&mut d, x, y); d }
            Tool::Boxes => match pick(&self.items, x, y) {
                Some((i, part)) => { self.picked = Some(i); Drag::Item { i, part, x0: x, y0: y, from: self.items[i].clone(), moved: false } }
                None => Drag::New { x0: x, y0: y },
            },
        });
        self.dirty = true;
    }
    fn erase_to(&mut self, d: &mut Drag, x: f32, y: f32) {
        let Drag::Erase { kept, at } = d else { return };
        for p in std::iter::once(*at).chain(walk(*at, (x, y))) {
            if let Some(left) = erased(&self.items, p.0, p.1) {
                if !*kept { *kept = true; self.keep(); } // one sweep is one undo
                self.items = left;
            }
        }
        *at = (x, y);
    }
    /// The mouse moved to (x, y), on the paper or (while dragging) anywhere.
    pub fn moved(&mut self, x: f32, y: f32) {
        let (cx, cy) = clamp(x, y);
        match self.drag.take() {
            None => {
                if self.tool == Tool::Eraser { self.pointer = Some((cx, cy)); self.dirty = true; }
                let hover = if self.tool == Tool::Boxes { pick(&self.items, x, y).filter(|h| h.1 != Part::In) } else { None };
                if hover != self.hover { self.hover = hover; self.dirty = true; }
            }
            Some(Drag::Pen) => {
                if let Some(s) = &mut self.ink { let last = *s.pts.last().unwrap_or(&(cx, cy)); if last != (cx, cy) { s.pts.extend(walk(last, (cx, cy))); self.dirty = true; } }
                self.drag = Some(Drag::Pen);
            }
            Some(mut d @ Drag::Erase { .. }) => { self.pointer = Some((cx, cy)); self.erase_to(&mut d, cx, cy); self.drag = Some(d); self.dirty = true; }
            Some(Drag::New { x0, y0 }) => {
                if (cx - x0).hypot(cy - y0) >= 3. || self.draft.is_some() {
                    self.draft = Some(Rect { x: x0.min(cx), y: y0.min(cy), w: (cx - x0).abs(), h: (cy - y0).abs(), color: self.color(), dim_w: String::new(), dim_h: String::new() });
                    self.dirty = true;
                }
                self.drag = Some(Drag::New { x0, y0 });
            }
            Some(Drag::Item { i, part, x0, y0, from, mut moved }) => {
                if moved || (x - x0).hypot(y - y0) >= 3. {
                    if !moved { moved = true; self.keep(); }
                    self.items[i] = match (&from, part) {
                        (Item::Rect(r), Part::In) => { let mut o = r.clone(); o.x = (r.x + x - x0).clamp(0., W as f32 - r.w).round(); o.y = (r.y + y - y0).clamp(0., H as f32 - r.h).round(); Item::Rect(o) }
                        (Item::Rect(r), side) => Item::Rect(resize(r, side, cx.round(), cy.round())),
                        (Item::Stroke(s), _) => {
                            let (bx, by, bw, bh) = bounds(&from);
                            let (dx, dy) = ((x - x0).clamp(-bx, W as f32 - bw - bx), (y - y0).clamp(-by, H as f32 - bh - by));
                            Item::Stroke(Stroke { pts: s.pts.iter().map(|p| (p.0 + dx, p.1 + dy)).collect(), color: s.color })
                        }
                    };
                    self.dirty = true;
                }
                self.drag = Some(Drag::Item { i, part, x0, y0, from, moved });
            }
        }
    }
    /// The mouse left the paper with no button down.
    pub fn left(&mut self) { if self.hover.is_some() || self.pointer.is_some() { self.hover = None; self.pointer = None; self.dirty = true; } }
    /// The button came up. A click on a box's side (no drag) asks for its length: the caller opens the box and `editing` says which side.
    pub fn up(&mut self) -> bool {
        let Some(d) = self.drag.take() else { return false };
        self.dirty = true;
        match d {
            Drag::Pen => { if let Some(s) = self.ink.take() { self.keep(); self.items.push(Item::Stroke(s)); } false }
            Drag::Erase { .. } => false,
            Drag::New { .. } => {
                if let Some(r) = self.draft.take().filter(|r| r.w >= MIN && r.h >= MIN) {
                    self.keep();
                    self.items.push(Item::Rect(Rect { x: r.x.round(), y: r.y.round(), w: r.w.round(), h: r.h.round(), ..r }));
                    self.picked = Some(self.items.len() - 1);
                    if !self.pinned { self.next += 1; }
                }
                false
            }
            Drag::Item { i, part, moved, from, .. } => {
                let side = !moved && part != Part::In && matches!(from, Item::Rect(_));
                if side { self.editing = Some((i, part)); }
                side
            }
        }
    }
    /// What the length box starts with: the side's typed length, else its length in units.
    pub fn length_text(&self) -> String {
        let Some((i, side)) = self.editing else { return String::new() };
        let Some(Item::Rect(r)) = self.items.get(i) else { return String::new() };
        let (label, px) = if matches!(side, Part::L | Part::R) { (&r.dim_w, r.w) } else { (&r.dim_h, r.h) };
        if label.is_empty() { format!("{}", (px / UNIT).round()) } else { label.clone() }
    }
    /// Where the length box goes: the middle of the side, in paper pixels.
    pub fn length_at(&self) -> Option<(f32, f32)> {
        let (i, side) = self.editing?;
        let Some(Item::Rect(r)) = self.items.get(i) else { return None };
        Some(match side { Part::L => (r.x, r.y + r.h / 2.), Part::R | Part::In => (r.x + r.w, r.y + r.h / 2.), Part::T => (r.x + r.w / 2., r.y), Part::B => (r.x + r.w / 2., r.y + r.h) })
    }
    /// The length box closed with `text`; text that is no length changes nothing.
    pub fn commit_length(&mut self, text: &str) {
        let Some((i, side)) = self.editing.take() else { return };
        self.dirty = true;
        let (Some(len), Some(Item::Rect(r))) = (parse_length(text), self.items.get(i)) else { return };
        let next = with_length(r, side, &len);
        self.keep();
        self.items[i] = Item::Rect(next);
    }
    pub fn cancel_length(&mut self) { self.editing = None; self.dirty = true; }
    /// Take back the last change.
    pub fn undo(&mut self) {
        if self.dragging() { return; }
        if let Some((items, next)) = self.undo.pop() { self.items = items; self.next = next; self.picked = None; self.hover = None; self.dirty = true; }
    }
    /// Delete the selection; with none, clear the paper. Either is one undo.
    pub fn delete(&mut self) {
        if self.dragging() || self.items.is_empty() { return; }
        self.keep();
        match self.picked.take() { Some(i) if i < self.items.len() => { self.items.remove(i); } _ => self.items.clear() }
        self.hover = None; self.dirty = true;
    }
    /// The cursor's shape at the pointer's last place: the part hovered.
    pub fn over(&self) -> Option<Part> { self.hover.map(|h| h.1) }

    /// The picture as RGBA. `marks`: with the selection, the hover and what is being drawn, for the screen; without, for the attachment.
    pub fn paint(&self, marks: bool) -> Vec<u8> {
        let mut c = Canvas::new();
        for (i, it) in self.items.iter().enumerate() {
            match it { Item::Rect(r) => c.rect(r), Item::Stroke(s) => c.stroke(s) }
            if marks && self.picked == Some(i) { let (x, y, w, h) = bounds(it); c.dashed(x - 4., y - 4., w + 8., h + 8., INK); }
        }
        if marks {
            if let Some((i, side)) = self.hover.filter(|_| !self.dragging()) { if let Some(Item::Rect(r)) = self.items.get(i) { c.side(r, side); } }
            if let Some(d) = &self.draft { c.dashed(d.x, d.y, d.w, d.h, d.color); }
            if let Some(s) = &self.ink { c.stroke(s); }
            if let (Tool::Eraser, Some(p)) = (self.tool, self.pointer) { c.ring(p, ERASER); }
        }
        c.px
    }
}

/// The picture as a PNG.
pub fn png(rgba: &[u8]) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect(); // the paper is opaque
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out).write_image(&rgb, W as u32, H as u32, image::ExtendedColorType::Rgb8).ok()?;
    Some(out)
}
/// Now as it is named on a drawing: local time, 20261021-025300.
pub fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as libc::time_t;
    // SAFETY: both pointers are to live locals; localtime_r writes only to `tm`.
    let tm = unsafe { let mut tm: libc::tm = std::mem::zeroed(); libc::localtime_r(&secs, &mut tm); tm };
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
}

/// The glyphs of the bundled bitmap font, 8 wide and 16 tall, for the typed lengths on the picture.
fn glyph(ch: char) -> Option<&'static [u8; 16]> {
    static FONT: std::sync::OnceLock<std::collections::HashMap<char, [u8; 16]>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        for block in include_str!("../../../../assets/fonts/spleen-8x16.bdf").split("STARTCHAR").skip(1) {
            let Some(code) = block.lines().find_map(|l| l.strip_prefix("ENCODING ")).and_then(|n| n.trim().parse::<u32>().ok()).and_then(char::from_u32) else { continue };
            let Some((_, rows)) = block.split_once("BITMAP") else { continue };
            let mut bits = [0u8; 16];
            for (i, row) in rows.lines().filter(|l| !l.is_empty()).take_while(|l| !l.starts_with("ENDCHAR")).take(16).enumerate() { bits[i] = u8::from_str_radix(row.trim(), 16).unwrap_or(0); }
            map.insert(code, bits);
        }
        map
    }).get(&ch)
}

struct Canvas { px: Vec<u8> }
impl Canvas {
    fn new() -> Self { Self { px: vec![255; W * H * 4] } }
    /// `color` at `a` (0 to 1) over the pixel; off the paper nothing happens.
    fn dot(&mut self, x: i32, y: i32, color: u32, a: f32) {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 || a <= 0. { return; }
        let at = (y as usize * W + x as usize) * 4;
        for (k, sh) in [16, 8, 0].iter().enumerate() {
            let s = ((color >> sh) & 255) as f32; let d = self.px[at + k] as f32;
            self.px[at + k] = (d + (s - d) * a.min(1.)).round() as u8;
        }
    }
    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32, a: f32) {
        for j in y.round() as i32..(y + h).round() as i32 { for i in x.round() as i32..(x + w).round() as i32 { self.dot(i, j, color, a); } }
    }
    fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, color: u32) {
        self.fill(x, y, w, t, color, 1.); self.fill(x, y + h - t, w, t, color, 1.);
        self.fill(x, y + t, t, h - 2. * t, color, 1.); self.fill(x + w - t, y + t, t, h - 2. * t, color, 1.);
    }
    fn rect(&mut self, r: &Rect) {
        self.fill(r.x, r.y, r.w, r.h, r.color, 0.2);
        self.outline(r.x, r.y, r.w, r.h, LINE, r.color);
        if !r.dim_w.is_empty() { self.text(&r.dim_w, r.x + r.w / 2., r.y - 6., false, r.color); }
        if !r.dim_h.is_empty() { self.text(&r.dim_h, r.x - 8., r.y + r.h / 2., true, r.color); }
    }
    /// A round-ended line, its edge smoothed.
    fn capsule(&mut self, a: (f32, f32), b: (f32, f32), radius: f32, color: u32) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1); let len2 = (dx * dx + dy * dy).max(1e-6); let m = radius + 1.;
        for j in (a.1.min(b.1) - m).floor() as i32..=(a.1.max(b.1) + m).ceil() as i32 {
            for i in (a.0.min(b.0) - m).floor() as i32..=(a.0.max(b.0) + m).ceil() as i32 {
                let (px, py) = (i as f32 + 0.5, j as f32 + 0.5);
                let t = (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0., 1.);
                let d = (px - a.0 - dx * t).hypot(py - a.1 - dy * t);
                self.dot(i, j, color, radius + 0.5 - d);
            }
        }
    }
    fn stroke(&mut self, s: &Stroke) {
        let r = LINE / 2.;
        if let [p] = s.pts[..] { self.capsule(p, p, r, s.color); }
        for w in s.pts.windows(2) { self.capsule(w[0], w[1], r, s.color); }
    }
    fn dashed(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32) {
        let (x0, y0, x1, y1) = (x.round() as i32, y.round() as i32, (x + w).round() as i32, (y + h).round() as i32);
        for i in (x0..x1).filter(|i| (i - x0) % 10 < 6) { self.dot(i, y0, color, 1.); self.dot(i, y1, color, 1.); }
        for j in (y0..y1).filter(|j| (j - y0) % 10 < 6) { self.dot(x0, j, color, 1.); self.dot(x1, j, color, 1.); }
    }
    fn side(&mut self, r: &Rect, side: Part) {
        let t = 5.;
        match side {
            Part::L => self.fill(r.x - 2., r.y, t, r.h, INK, 1.), Part::R => self.fill(r.x + r.w - 2., r.y, t, r.h, INK, 1.),
            Part::T => self.fill(r.x, r.y - 2., r.w, t, INK, 1.), Part::B => self.fill(r.x, r.y + r.h - 2., r.w, t, INK, 1.), Part::In => {}
        }
    }
    fn ring(&mut self, p: (f32, f32), radius: f32) {
        for k in 0..64 { let t = k as f32 / 64. * std::f32::consts::TAU; self.dot((p.0 + radius * t.cos()).round() as i32, (p.1 + radius * t.sin()).round() as i32, INK, 1.); }
    }
    /// `text` centred on `x` with its foot on `y`, or running upward beside `x` centred on `y` when `up`.
    fn text(&mut self, text: &str, x: f32, y: f32, up: bool, color: u32) {
        let width = text.chars().count() as i32 * 8;
        for (n, ch) in text.chars().enumerate() {
            let Some(rows) = glyph(ch) else { continue };
            for (j, bits) in rows.iter().enumerate() {
                for i in 0..8 {
                    if bits & (0x80 >> i) == 0 { continue; }
                    let (u, v) = (n as i32 * 8 + i - width / 2, j as i32 - 16); // along the text from its middle; above its foot
                    let (px, py) = if up { (x as i32 + v, y as i32 - u) } else { (x as i32 + u, y as i32 + v) };
                    self.dot(px, py, color, 1.);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn r() -> Rect { Rect { x: 100., y: 100., w: 200., h: 100., color: 0, dim_w: String::new(), dim_h: String::new() } }
    fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] { let i = (y * W + x) * 4; [px[i], px[i + 1], px[i + 2]] }
    const WHITE: [u8; 3] = [255, 255, 255];
    fn drag(p: &mut Pad, a: (f32, f32), b: (f32, f32)) -> bool { p.down(a.0, a.1); for i in 1..=6 { let t = i as f32 / 6.; p.moved(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t); } p.up() }
    fn with_tool(tool: Tool) -> Pad { let mut p = Pad::default(); p.set_tool(tool); p }

    #[test] fn a_point_on_each_side_is_that_side() {
        let b = r();
        assert_eq!((hit(&b, 100., 150.), hit(&b, 300., 150.), hit(&b, 200., 100.), hit(&b, 200., 200.)), (Some(Part::L), Some(Part::R), Some(Part::T), Some(Part::B)));
        assert_eq!((hit(&b, 200., 150.), hit(&b, 50., 50.)), (Some(Part::In), None));
    }
    #[test] fn the_topmost_item_is_picked() {
        let items = vec![Item::Rect(r()), Item::Rect(Rect { x: 150., ..r() })];
        assert_eq!(pick(&items, 200., 150.).unwrap().0, 1);
    }
    #[test] fn lengths_parse() {
        assert_eq!(parse_length("40 mm"), Some(Length { n: 40., unit: "mm".into() }));
        assert_eq!(parse_length("12.5cm").unwrap().n, 12.5); assert_eq!(parse_length(" 7 ").unwrap().unit, "");
        for bad in ["x", "0", "", "1.", ".5", "40 metres", "4 0"] { assert_eq!(parse_length(bad), None, "{bad}"); }
    }
    #[test] fn a_typed_length_keeps_the_opposite_side() {
        let l = Length { n: 30., unit: "mm".into() };
        let right = with_length(&r(), Part::R, &l); assert_eq!((right.x, right.w, right.dim_w.as_str()), (100., 120., "30 mm"));
        let left = with_length(&r(), Part::L, &Length { n: 30., unit: String::new() }); assert_eq!((left.x + left.w, left.w, left.dim_w.as_str()), (300., 120., "30"));
        let top = with_length(&r(), Part::T, &l); assert_eq!((top.y + top.h, top.h, top.dim_h.as_str()), (200., 120., "30 mm"));
    }
    #[test] fn a_side_dragged_past_the_other_never_flips_and_drops_the_typed_width() {
        let typed = with_length(&r(), Part::R, &Length { n: 30., unit: "mm".into() });
        let s = resize(&typed, Part::R, 50., 0.); assert!(s.w >= MIN && s.x == 100. && s.dim_w.is_empty());
    }
    #[test] fn each_new_box_takes_the_next_colour_unless_one_was_chosen() {
        let mut p = with_tool(Tool::Boxes);
        drag(&mut p, (100., 100.), (300., 250.)); drag(&mut p, (400., 100.), (600., 300.));
        let colors: Vec<u32> = p.items.iter().map(|i| if let Item::Rect(r) = i { r.color } else { 0 }).collect();
        assert_eq!(colors, [COLORS[0], COLORS[1]]);
        p.set_color(3); drag(&mut p, (700., 100.), (800., 300.)); drag(&mut p, (700., 350.), (800., 500.));
        assert!(matches!((&p.items[2], &p.items[3]), (Item::Rect(a), Item::Rect(b)) if a.color == COLORS[3] && b.color == COLORS[3]));
    }
    #[test] fn a_small_drag_is_a_click_and_makes_no_box() {
        let mut p = with_tool(Tool::Boxes); drag(&mut p, (100., 100.), (102., 101.)); assert!(p.is_empty()); assert_eq!(p.next, 0);
    }
    #[test] fn a_boxs_middle_moves_it_and_a_side_resizes_it_and_a_click_on_a_side_asks_its_length() {
        let mut p = with_tool(Tool::Boxes); drag(&mut p, (100., 100.), (300., 250.));
        assert!(!drag(&mut p, (200., 175.), (250., 215.)));
        assert!(matches!(&p.items[0], Item::Rect(b) if (b.x, b.y) == (150., 140.)));
        drag(&mut p, (250., 290.), (250., 340.));
        assert!(matches!(&p.items[0], Item::Rect(b) if b.y + b.h == 340.));
        p.down(350., 200.); assert!(p.up()); assert_eq!(p.editing, Some((0, Part::R))); assert_eq!(p.length_text(), "50");
        p.commit_length("30 mm"); assert!(matches!(&p.items[0], Item::Rect(b) if b.w == 120. && b.dim_w == "30 mm")); assert_eq!(p.editing, None);
    }
    /// The old client showed a box's width when a click on its top or bottom was about to set the height.
    #[test] fn the_length_box_starts_with_the_axis_it_sets() {
        let mut p = with_tool(Tool::Boxes); drag(&mut p, (100., 100.), (300., 200.));
        p.down(200., 200.); p.up(); assert_eq!(p.editing, Some((0, Part::B))); assert_eq!(p.length_text(), "25");
        p.commit_length("not a length"); assert!(matches!(&p.items[0], Item::Rect(b) if b.h == 100.));
    }
    #[test] fn ctrl_z_takes_back_one_stroke_at_a_time() {
        let mut p = Pad::default();
        drag(&mut p, (100., 100.), (300., 100.)); drag(&mut p, (100., 200.), (300., 200.)); assert_eq!(p.items.len(), 2);
        p.undo(); assert_eq!(p.items.len(), 1); assert!(matches!(&p.items[0], Item::Stroke(s) if s.pts[0].1 == 100.));
        p.undo(); assert!(p.is_empty()); p.undo(); assert!(p.is_empty());
    }
    #[test] fn a_click_with_the_pen_is_a_dot() {
        let mut p = Pad::default(); p.down(50., 50.); p.up();
        assert_ne!(at(&p.paint(false), 50, 50), WHITE);
    }
    #[test] fn a_stroke_leaves_no_gap_wider_than_the_step_however_fast_the_mouse() {
        let mut p = Pad::default(); p.down(0., 0.); p.moved(900., 0.); p.up();
        assert!(matches!(&p.items[0], Item::Stroke(s) if s.pts.windows(2).all(|w| (w[1].0 - w[0].0).abs() <= STEP + 0.01)));
    }
    #[test] fn delete_removes_the_selection_and_with_none_clears_the_paper_and_undo_brings_either_back() {
        let mut p = with_tool(Tool::Boxes); drag(&mut p, (100., 100.), (300., 250.)); drag(&mut p, (400., 100.), (600., 300.));
        assert_eq!(p.picked, Some(1), "a new box is picked");
        p.delete(); assert_eq!(p.items.len(), 1); assert_eq!(p.picked, None);
        p.undo(); assert_eq!(p.items.len(), 2);
        p.delete(); assert!(p.is_empty(), "nothing picked: the paper clears");
        p.undo(); assert_eq!(p.items.len(), 2);
    }
    #[test] fn a_stroke_can_be_picked_moved_and_deleted() {
        let mut p = Pad::default(); drag(&mut p, (100., 100.), (200., 100.)); p.set_tool(Tool::Boxes);
        drag(&mut p, (150., 100.), (150., 150.)); assert!(matches!(&p.items[0], Item::Stroke(s) if s.pts[0] == (100., 150.)));
        assert_eq!(p.picked, Some(0)); p.delete(); assert!(p.is_empty());
    }
    #[test] fn the_eraser_cuts_a_stroke_and_takes_a_box_and_one_sweep_is_one_undo() {
        let mut p = Pad::default(); drag(&mut p, (100., 100.), (500., 100.));
        p.set_tool(Tool::Eraser); drag(&mut p, (300., 50.), (300., 150.));
        assert_eq!(p.items.len(), 2, "cut in two"); let px = p.paint(false);
        assert_eq!(at(&px, 300, 100), WHITE); assert_ne!(at(&px, 200, 100), WHITE); assert_ne!(at(&px, 400, 100), WHITE);
        p.undo(); assert_eq!(p.items.len(), 1); assert_ne!(at(&p.paint(false), 300, 100), WHITE);
        p.set_tool(Tool::Boxes); drag(&mut p, (600., 300.), (800., 500.)); p.set_tool(Tool::Eraser); drag(&mut p, (700., 400.), (710., 400.));
        assert_eq!(p.items.len(), 1, "the box is gone");
    }
    #[test] fn an_eraser_sweep_over_nothing_leaves_no_undo() {
        let mut p = Pad::default(); drag(&mut p, (100., 100.), (200., 100.)); p.set_tool(Tool::Eraser); drag(&mut p, (500., 500.), (600., 500.));
        p.undo(); assert!(p.is_empty(), "the undo took the stroke, not an empty sweep");
    }
    #[test] fn nothing_happens_mid_drag() {
        let mut p = Pad::default(); drag(&mut p, (10., 10.), (50., 10.)); p.down(100., 100.); p.moved(200., 100.);
        p.undo(); p.delete(); assert_eq!(p.items.len(), 1); p.up(); assert_eq!(p.items.len(), 2);
    }
    #[test] fn the_attachment_has_no_selection_or_work_in_progress_and_the_screen_does() {
        let mut p = with_tool(Tool::Boxes); drag(&mut p, (100., 100.), (300., 250.));
        assert_eq!(at(&p.paint(false), 96, 96), WHITE); assert_ne!(at(&p.paint(true), 96, 96), WHITE, "the dashes round the picked box");
        p.down(500., 100.); p.moved(600., 200.);
        assert_eq!(at(&p.paint(false), 500, 100), WHITE); assert_ne!(at(&p.paint(true), 500, 100), WHITE, "the draft");
    }
    #[test] fn a_box_is_a_light_fill_and_a_solid_outline() {
        let mut p = with_tool(Tool::Boxes); drag(&mut p, (100., 100.), (300., 250.)); let px = p.paint(false);
        assert_eq!(at(&px, 101, 150), [0xe5, 0x48, 0x4d]);
        let inside = at(&px, 200, 175); assert!(inside != WHITE && inside[0] > 0xe5 && inside[1] > 0x48, "{inside:?}");
    }
    #[test] fn typed_lengths_are_written_on_the_picture_beside_their_sides() {
        let mut p = with_tool(Tool::Boxes); drag(&mut p, (200., 200.), (400., 300.));
        let clean = p.paint(false);
        p.down(400., 250.); p.up(); p.commit_length("30 mm");
        p.down(260., 300.); p.up(); p.commit_length("25");
        let px = p.paint(false);
        let inked = |x0: usize, y0: usize, x1: usize, y1: usize| (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| at(&px, x, y) != WHITE).count();
        assert!(inked(230, 178, 290, 194) > 30, "the width over the top side"); assert!(inked(176, 230, 192, 270) > 15, "the height beside the left side");
        assert_ne!(clean, px);
    }
    #[test] fn the_font_has_what_a_length_can_contain() { for ch in "0123456789. abcmnxyzAZµ\"'".chars() { assert!(glyph(ch).is_some(), "{ch}"); } assert!(glyph('0').unwrap().iter().any(|b| *b != 0)); }
    #[test] fn the_picture_is_a_png_of_the_paper() {
        let mut p = Pad::default(); drag(&mut p, (100., 100.), (300., 100.));
        let bytes = png(&p.paint(false)).unwrap(); assert_eq!(&bytes[..4], b"\x89PNG");
        let img = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (W as u32, H as u32)); assert_eq!(img.get_pixel(200, 100).0, [0xe5, 0x48, 0x4d]); assert_eq!(img.get_pixel(5, 5).0, WHITE);
    }
    #[test] fn the_stamp_reads_as_the_old_name() { let s = stamp(); assert_eq!(s.len(), 15); assert_eq!(&s[8..9], "-"); assert!(s.chars().filter(|c| *c != '-').all(|c| c.is_ascii_digit())); }
}
