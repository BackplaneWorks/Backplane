//! RS-274X (Gerber X2/X3) to shapes: what fab output from KiCad and the common tools holds. Format and units, standard and macro
//! apertures (with their expressions), draws, arcs (multi and single quadrant), flashes, regions, polarity, step and repeat, and the X2
//! attributes that say a file's layer (TF.FileFunction) and an object's net, part and pin (TO.N, TO.C, TO.P). Coordinates stay as the
//! file has them (millimetres, Y up); kicad/gerber/mod.rs places them on the board. A word it cannot read is reported with its line,
//! and the rest of the file is still read.
use std::collections::HashMap;

pub type P = (f64, f64);

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// a stroke along points, round ends, this wide
    Stroke { ps: Vec<P>, w: f64 },
    Disc { c: P, r: f64 },
    /// a filled outline (closed)
    Poly { ps: Vec<P> },
}

/// Who an object belongs to (X2 object attributes) and what its aperture is for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Owner { pub net: String, pub part: String, pub pin: String, pub function: String }

#[derive(Clone, Debug, PartialEq)]
pub struct Obj { pub shape: Shape, pub dark: bool, pub owner: Owner, pub line: usize, pub aperture: i32 }

#[derive(Debug, Default)]
pub struct Layer {
    pub objs: Vec<Obj>,
    /// TF.FileFunction ("Copper,L1,Top", "Soldermask,Bot", "Profile,NP"...), as written
    pub function: String,
    /// TF.FilePolarity Negative: the file draws what is not there (a plane)
    pub negative: bool,
    /// (line, what) for each word not read
    pub errors: Vec<(usize, String)>,
    /// the file ended with M02
    pub ended: bool,
}

#[derive(Clone, Debug)]
enum Aperture {
    Circle { d: f64 },
    Rect { w: f64, h: f64 },
    Obround { w: f64, h: f64 },
    Polygon { d: f64, n: u32, rot: f64 },
    /// a macro's shapes about the flash point, each dark or clear
    Shapes(Vec<(Shape, bool)>),
}
impl Aperture {
    /// how wide a draw with it is (a circle's diameter; a rectangle draws as wide as its smaller side)
    fn width(&self) -> f64 {
        match self { Aperture::Circle { d } | Aperture::Polygon { d, .. } => *d, Aperture::Rect { w, h } | Aperture::Obround { w, h } => w.min(*h), Aperture::Shapes(_) => 0.0 }
    }
    fn flash(&self, at: P) -> Vec<(Shape, bool)> {
        let (x, y) = at;
        match self {
            Aperture::Circle { d } => vec![(Shape::Disc { c: at, r: d / 2.0 }, true)],
            Aperture::Rect { w, h } => vec![(Shape::Poly { ps: vec![(x - w / 2.0, y - h / 2.0), (x + w / 2.0, y - h / 2.0), (x + w / 2.0, y + h / 2.0), (x - w / 2.0, y + h / 2.0)] }, true)],
            Aperture::Obround { w, h } => {
                // a stroke between the two round ends' centres, as wide as the short side
                let r = w.min(*h) / 2.0;
                let (a, b) = if w >= h { ((x - w / 2.0 + r, y), (x + w / 2.0 - r, y)) } else { ((x, y - h / 2.0 + r), (x, y + h / 2.0 - r)) };
                vec![(Shape::Stroke { ps: vec![a, b], w: 2.0 * r }, true)]
            }
            Aperture::Polygon { d, n, rot } => vec![(Shape::Poly { ps: (0..*n).map(|i| { let t = (rot + 360.0 * i as f64 / *n as f64).to_radians(); (x + d / 2.0 * t.cos(), y + d / 2.0 * t.sin()) }).collect() }, true)],
            Aperture::Shapes(v) => v.iter().map(|(s, dark)| (moved(s, at), *dark)).collect(),
        }
    }
}
fn moved(s: &Shape, (dx, dy): P) -> Shape {
    let m = |ps: &[P]| ps.iter().map(|(x, y)| (x + dx, y + dy)).collect();
    match s { Shape::Stroke { ps, w } => Shape::Stroke { ps: m(ps), w: *w }, Shape::Disc { c, r } => Shape::Disc { c: (c.0 + dx, c.1 + dy), r: *r }, Shape::Poly { ps } => Shape::Poly { ps: m(ps) } }
}
fn rotated((x, y): P, deg: f64) -> P { if deg == 0.0 { return (x, y) } let (s, c) = deg.to_radians().sin_cos(); (x * c - y * s, x * s + y * c) }

// Macro expressions -------------------------------------------------------------------------------------------------------------

/// A macro's arithmetic: numbers, $n, + - x X / and parentheses, unary minus. None when it does not read.
pub fn eval(e: &str, vars: &HashMap<u32, f64>) -> Option<f64> {
    let t: Vec<char> = e.chars().filter(|c| !c.is_whitespace()).collect();
    let mut i = 0;
    let v = sum(&t, &mut i, vars)?;
    (i == t.len()).then_some(v)
}
fn sum(t: &[char], i: &mut usize, vars: &HashMap<u32, f64>) -> Option<f64> {
    let mut v = prod(t, i, vars)?;
    while *i < t.len() && (t[*i] == '+' || t[*i] == '-') { let op = t[*i]; *i += 1; let r = prod(t, i, vars)?; v = if op == '+' { v + r } else { v - r } }
    Some(v)
}
fn prod(t: &[char], i: &mut usize, vars: &HashMap<u32, f64>) -> Option<f64> {
    let mut v = unary(t, i, vars)?;
    while *i < t.len() && matches!(t[*i], 'x' | 'X' | '/') { let op = t[*i]; *i += 1; let r = unary(t, i, vars)?; v = if op == '/' { if r == 0.0 { return None } v / r } else { v * r } }
    Some(v)
}
fn unary(t: &[char], i: &mut usize, vars: &HashMap<u32, f64>) -> Option<f64> {
    match t.get(*i)? {
        '-' => { *i += 1; Some(-unary(t, i, vars)?) }
        '+' => { *i += 1; unary(t, i, vars) }
        '(' => { *i += 1; let v = sum(t, i, vars)?; if t.get(*i) != Some(&')') { return None } *i += 1; Some(v) }
        '$' => { *i += 1; let s = *i; while *i < t.len() && t[*i].is_ascii_digit() { *i += 1 } let n: u32 = t[s..*i].iter().collect::<String>().parse().ok()?; Some(*vars.get(&n).unwrap_or(&0.0)) }
        _ => { let s = *i; while *i < t.len() && (t[*i].is_ascii_digit() || t[*i] == '.') { *i += 1 } if s == *i { return None } t[s..*i].iter().collect::<String>().parse().ok() }
    }
}

/// A macro's body (its statements after the name) instantiated with the aperture's parameters: its shapes about the origin.
pub fn macro_shapes(body: &[String], params: &[f64]) -> Result<Vec<(Shape, bool)>, String> {
    let mut vars: HashMap<u32, f64> = params.iter().enumerate().map(|(i, v)| (i as u32 + 1, *v)).collect();
    let mut out = vec![];
    for st in body {
        let st = st.trim();
        if st.is_empty() || st.starts_with('0') && (st.len() == 1 || st.as_bytes()[1] == b' ') { continue }
        if let Some(rest) = st.strip_prefix('$') {
            let (n, e) = rest.split_once('=').ok_or_else(|| format!("macro variable {st}"))?;
            let v = eval(e, &vars).ok_or_else(|| format!("macro expression {e}"))?;
            vars.insert(n.trim().parse().map_err(|_| format!("macro variable {st}"))?, v);
            continue
        }
        let parts: Vec<&str> = st.split(',').collect();
        let code: u32 = parts[0].trim().parse().map_err(|_| format!("macro primitive {st}"))?;
        let a: Vec<f64> = parts[1..].iter().map(|e| eval(e, &vars).ok_or_else(|| format!("macro expression {e}"))).collect::<Result<_, _>>()?;
        let g = |i: usize| a.get(i).copied().unwrap_or(0.0);
        let dark = g(0) != 0.0;
        match code {
            1 => { let c = rotated((g(2), g(3)), g(4)); out.push((Shape::Disc { c, r: g(1) / 2.0 }, dark)) }
            20 | 2 => {
                let (w, s, e, rot) = (g(1), (g(2), g(3)), (g(4), g(5)), g(6));
                let (dx, dy) = (e.0 - s.0, e.1 - s.1); let l = (dx * dx + dy * dy).sqrt().max(1e-12);
                let (nx, ny) = (-dy / l * w / 2.0, dx / l * w / 2.0);
                out.push((Shape::Poly { ps: [(s.0 + nx, s.1 + ny), (e.0 + nx, e.1 + ny), (e.0 - nx, e.1 - ny), (s.0 - nx, s.1 - ny)].iter().map(|p| rotated(*p, rot)).collect() }, dark))
            }
            21 => {
                let (w, h, c, rot) = (g(1), g(2), (g(3), g(4)), g(5));
                out.push((Shape::Poly { ps: [(c.0 - w / 2.0, c.1 - h / 2.0), (c.0 + w / 2.0, c.1 - h / 2.0), (c.0 + w / 2.0, c.1 + h / 2.0), (c.0 - w / 2.0, c.1 + h / 2.0)].iter().map(|p| rotated(*p, rot)).collect() }, dark))
            }
            4 => {
                let n = g(1) as usize; let rot = g(4 + 2 * n);
                let ps: Vec<P> = (0..=n).map(|k| rotated((g(2 + 2 * k), g(3 + 2 * k)), rot)).collect();
                out.push((Shape::Poly { ps }, dark))
            }
            5 => {
                let (n, c, d, rot) = (g(1).max(3.0) as u32, (g(2), g(3)), g(4), g(5));
                out.push((Shape::Poly { ps: (0..n).map(|k| { let t = (360.0 * k as f64 / n as f64).to_radians(); rotated((c.0 + d / 2.0 * t.cos(), c.1 + d / 2.0 * t.sin()), rot) }).collect() }, dark))
            }
            7 => {
                // a thermal: drawn as its ring (the gaps left out)
                let (c, od, id) = ((g(0), g(1)), g(2), g(3));
                let r = (od + id) / 4.0;
                out.push((Shape::Stroke { ps: (0..=36).map(|k| { let t = (10.0 * k as f64).to_radians(); rotated((c.0 + r * t.cos(), c.1 + r * t.sin()), g(5)) }).collect(), w: (od - id) / 2.0 }, true))
            }
            _ => return Err(format!("macro primitive {code}")),
        }
    }
    Ok(out)
}

// The file ---------------------------------------------------------------------------------------------------------------------

/// Arcs are drawn as chords no longer than this many degrees.
const ARC_STEP: f64 = 6.0;
pub fn arc_points(s: P, e: P, c: P, cw: bool) -> Vec<P> {
    let r = ((s.0 - c.0).powi(2) + (s.1 - c.1).powi(2)).sqrt();
    let a0 = (s.1 - c.1).atan2(s.0 - c.0);
    let mut a1 = (e.1 - c.1).atan2(e.0 - c.0);
    if cw { while a1 >= a0 - 1e-9 { a1 -= std::f64::consts::TAU } } else { while a1 <= a0 + 1e-9 { a1 += std::f64::consts::TAU } }
    let n = (((a1 - a0).abs().to_degrees() / ARC_STEP).ceil() as usize).clamp(2, 720);
    let mut v: Vec<P> = (0..=n).map(|k| { let t = a0 + (a1 - a0) * k as f64 / n as f64; (c.0 + r * t.cos(), c.1 + r * t.sin()) }).collect();
    if let Some(l) = v.last_mut() { *l = e }
    v
}

struct St {
    int_d: (u32, u32), dec_d: (u32, u32), trailing: bool, mm: bool,
    aps: HashMap<i32, (Aperture, String)>, macros: HashMap<String, Vec<String>>, cur_ap: i32,
    x: f64, y: f64, mode: u8, multi: bool, region: bool, contour: Vec<P>, dark: bool,
    path: Vec<P>, owner: Owner, ap_attrs: HashMap<String, String>, sr: Option<(u32, u32, f64, f64, usize)>,
}

pub fn parse(text: &str) -> Layer {
    let mut l = Layer::default();
    let mut st = St { int_d: (3, 3), dec_d: (6, 6), trailing: false, mm: true, aps: HashMap::new(), macros: HashMap::new(), cur_ap: 0, x: 0.0, y: 0.0, mode: 1, multi: true, region: false, contour: vec![], dark: true, path: vec![], owner: Owner::default(), ap_attrs: HashMap::new(), sr: None };
    let b = text.as_bytes();
    let (mut i, mut line) = (0usize, 1usize);
    while i < b.len() {
        match b[i] {
            b'\n' => { line += 1; i += 1 }
            b'\r' | b' ' | b'\t' => i += 1,
            b'%' => {
                let start_line = line;
                let Some(end) = b[i + 1..].iter().position(|c| *c == b'%').map(|p| i + 1 + p) else { l.errors.push((line, "an extended command with no closing %".into())); break };
                let block = &text[i + 1..end];
                line += block.matches('\n').count();
                extended(&mut st, &mut l, block, start_line);
                i = end + 1;
            }
            _ => {
                let start_line = line;
                let Some(end) = b[i..].iter().position(|c| *c == b'*').map(|p| i + p) else { if !text[i..].trim().is_empty() { l.errors.push((line, "a word with no closing *".into())) } break };
                let word = &text[i..end];
                line += word.matches('\n').count();
                let w: String = word.chars().filter(|c| !c.is_whitespace()).collect();
                if let Err(e) = command(&mut st, &mut l, &w, start_line) { l.errors.push((start_line, e)) }
                i = end + 1;
                if l.ended { break }
            }
        }
    }
    flush(&mut st, &mut l, line);
    l
}

fn extended(st: &mut St, l: &mut Layer, block: &str, line: usize) {
    let words: Vec<&str> = block.split('*').map(str::trim).filter(|w| !w.is_empty()).collect();
    let Some(first) = words.first() else { return };
    let f: String = first.chars().filter(|c| !c.is_whitespace()).collect();
    let err = |l: &mut Layer, m: String| l.errors.push((line, m));
    if let Some(r) = f.strip_prefix("FS") {
        // FSLAX36Y36: leading (L) or trailing (T) zeros omitted, absolute; X and Y digits
        st.trailing = r.starts_with('T');
        let digits = |k: char| r.find(k).and_then(|p| { let d: Vec<u32> = r[p + 1..].chars().take(2).filter_map(|c| c.to_digit(10)).collect(); (d.len() == 2).then(|| (d[0], d[1])) });
        match (digits('X'), digits('Y')) { (Some(x), Some(y)) => { st.int_d = (x.0, y.0); st.dec_d = (x.1, y.1) } _ => err(l, format!("format {f}")) }
    } else if let Some(r) = f.strip_prefix("MO") { st.mm = r.starts_with("MM") }
    else if let Some(r) = f.strip_prefix("LP") { flush(st, l, line); st.dark = !r.starts_with('C') }
    else if let Some(r) = f.strip_prefix("AM") { st.macros.insert(r.to_string(), words[1..].iter().map(|w| w.to_string()).collect()); }
    else if let Some(r) = f.strip_prefix("AD") { if let Err(e) = define(st, r) { err(l, e) } }
    else if let Some(r) = f.strip_prefix("TF") {
        let (k, v) = r.split_once(',').unwrap_or((r, ""));
        match k { ".FileFunction" => l.function = v.to_string(), ".FilePolarity" => l.negative = v.starts_with("Neg"), _ => {} }
    } else if let Some(r) = f.strip_prefix("TA") { let (k, v) = r.split_once(',').unwrap_or((r, "")); st.ap_attrs.insert(k.to_string(), v.to_string()); }
    else if let Some(r) = f.strip_prefix("TO") {
        flush(st, l, line);
        let (k, v) = r.split_once(',').unwrap_or((r, ""));
        let mut it = v.split(',');
        match k { ".N" => st.owner.net = it.next().unwrap_or("").to_string(), ".C" => st.owner.part = it.next().unwrap_or("").to_string(), ".P" => { st.owner.part = it.next().unwrap_or("").to_string(); st.owner.pin = it.next().unwrap_or("").to_string() } _ => {} }
    } else if let Some(r) = f.strip_prefix("TD") {
        flush(st, l, line);
        match r { "" => { st.owner = Owner::default(); st.ap_attrs.clear() } ".N" => st.owner.net.clear(), ".C" => st.owner.part.clear(), ".P" => { st.owner.pin.clear() } k => { st.ap_attrs.remove(k); } }
    } else if let Some(r) = f.strip_prefix("SR") {
        flush(st, l, line);
        // close the block open (its objects repeated), then open a new one if this one has counts
        if let Some((nx, ny, dx, dy, from)) = st.sr.take() {
            let base: Vec<Obj> = l.objs[from..].to_vec();
            for ix in 0..nx { for iy in 0..ny { if ix == 0 && iy == 0 { continue } for o in &base { l.objs.push(Obj { shape: moved(&o.shape, (dx * ix as f64, dy * iy as f64)), ..o.clone() }) } } }
        }
        let num = |k: char| r.find(k).map(|p| r[p + 1..].chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect::<String>());
        if let (Some(x), Some(y)) = (num('X'), num('Y')) {
            let (nx, ny) = (x.parse().unwrap_or(1u32).max(1), y.parse().unwrap_or(1u32).max(1));
            let (dx, dy) = (num('I').and_then(|s| s.parse().ok()).unwrap_or(0.0), num('J').and_then(|s| s.parse().ok()).unwrap_or(0.0));
            let k = if st.mm { 1.0 } else { 25.4 };
            if nx * ny > 1 { st.sr = Some((nx, ny, dx * k, dy * k, l.objs.len())) }
        }
    } else if f.starts_with("IP") || f.starts_with("OF") || f.starts_with("IN") || f.starts_with("LN") || f.starts_with("AS") || f.starts_with("MI") || f.starts_with("SF") || f.starts_with("IR") {
        // legacy image parameters: read as the default (KiCad writes none)
    } else if f.starts_with("LM") || f.starts_with("LR") || f.starts_with("LS") {
        if !f.ends_with("LMN") && f != "LR0" && f != "LS1" { err(l, format!("{f} (mirrored, rotated or scaled apertures) is not drawn as such")) }
    } else if f.starts_with("G04") {
    } else { err(l, format!("unknown command %{f}%")) }
}

fn define(st: &mut St, r: &str) -> Result<(), String> {
    // D10C,0.5X0.2 / D11R,1X2 / D12RoundRect,0.25X...
    let r = r.strip_prefix('D').ok_or_else(|| format!("aperture definition AD{r}"))?;
    let n: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
    let code: i32 = n.parse().map_err(|_| format!("aperture number in AD{r}"))?;
    let rest = &r[n.len()..];
    let (name, args) = rest.split_once(',').unwrap_or((rest, ""));
    let k = if st.mm { 1.0 } else { 25.4 };
    let a: Vec<f64> = if args.is_empty() { vec![] } else { args.split('X').map(|x| x.trim().parse::<f64>().map_err(|_| format!("aperture parameter {x} in AD{r}"))).collect::<Result<_, _>>()? };
    let g = |i: usize| a.get(i).copied().unwrap_or(0.0);
    let ap = match name {
        "C" => Aperture::Circle { d: g(0) * k },
        "R" => Aperture::Rect { w: g(0) * k, h: g(1) * k },
        "O" => Aperture::Obround { w: g(0) * k, h: g(1) * k },
        "P" => Aperture::Polygon { d: g(0) * k, n: (g(1) as u32).max(3), rot: g(2) },
        m => {
            let body = st.macros.get(m).ok_or_else(|| format!("aperture D{code} uses macro {m}, which is not defined"))?;
            // a macro's numbers are in the file's units: scale its shapes
            let shapes = macro_shapes(body, &a)?;
            Aperture::Shapes(shapes.into_iter().map(|(s, d)| (scaled(&s, k), d)).collect())
        }
    };
    let function = st.ap_attrs.get(".AperFunction").cloned().unwrap_or_default();
    st.aps.insert(code, (ap, function));
    Ok(())
}
fn scaled(s: &Shape, k: f64) -> Shape {
    if k == 1.0 { return s.clone() }
    let m = |ps: &[P]| ps.iter().map(|(x, y)| (x * k, y * k)).collect();
    match s { Shape::Stroke { ps, w } => Shape::Stroke { ps: m(ps), w: w * k }, Shape::Disc { c, r } => Shape::Disc { c: (c.0 * k, c.1 * k), r: r * k }, Shape::Poly { ps } => Shape::Poly { ps: m(ps) } }
}

fn coord(st: &St, s: &str, axis: usize) -> Result<f64, String> {
    let (neg, digits) = match s.strip_prefix('-') { Some(d) => (true, d), None => (false, s.strip_prefix('+').unwrap_or(s)) };
    let k = if st.mm { 1.0 } else { 25.4 };
    let v = if digits.contains('.') { digits.parse::<f64>().map_err(|_| format!("coordinate {s}"))? } else {
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) { return Err(format!("coordinate {s}")) }
        let (id, dd) = (if axis == 0 { st.int_d.0 } else { st.int_d.1 }, if axis == 0 { st.dec_d.0 } else { st.dec_d.1 });
        let d = if st.trailing { format!("{digits:0<w$}", w = (id + dd) as usize) } else { digits.to_string() };
        d.parse::<f64>().map_err(|_| format!("coordinate {s}"))? / 10f64.powi(dd as i32)
    };
    Ok(if neg { -v } else { v } * k)
}

fn flush(st: &mut St, l: &mut Layer, line: usize) {
    if st.path.len() >= 2 {
        let (w, function) = st.aps.get(&st.cur_ap).map(|(a, f)| (a.width(), f.clone())).unwrap_or((0.0, String::new()));
        l.objs.push(Obj { shape: Shape::Stroke { ps: std::mem::take(&mut st.path), w }, dark: st.dark, owner: Owner { function, ..st.owner.clone() }, line, aperture: st.cur_ap });
    }
    st.path.clear();
}
fn close_contour(st: &mut St, l: &mut Layer, line: usize) {
    if st.contour.len() >= 3 { l.objs.push(Obj { shape: Shape::Poly { ps: std::mem::take(&mut st.contour) }, dark: st.dark, owner: Owner { function: "Region".into(), ..st.owner.clone() }, line, aperture: 0 }) }
    st.contour.clear();
}

fn command(st: &mut St, l: &mut Layer, w: &str, line: usize) -> Result<(), String> {
    if w.is_empty() { return Ok(()) }
    if w.starts_with("G04") || w.starts_with("G4 ") { return Ok(()) }
    if w == "M02" || w == "M00" || w == "M2" { l.ended = true; return Ok(()) }
    if w == "M01" { return Ok(()) }
    let cs: Vec<char> = w.chars().collect();
    let mut i = 0;
    let (mut nx, mut ny, mut ni, mut nj): (Option<f64>, Option<f64>, Option<f64>, Option<f64>) = (None, None, None, None);
    let mut d: Option<i32> = None;
    let read_num = |i: &mut usize| -> String { let s = *i; while *i < cs.len() && (cs[*i].is_ascii_digit() || matches!(cs[*i], '-' | '+' | '.')) { *i += 1 } cs[s..*i].iter().collect() };
    while i < cs.len() {
        let c = cs[i]; i += 1;
        match c {
            'G' => {
                let n = read_num(&mut i);
                match n.parse::<u32>().map_err(|_| format!("G code in {w}"))? {
                    1 => st.mode = 1, 2 => st.mode = 2, 3 => st.mode = 3,
                    36 => { flush(st, l, line); st.region = true; st.contour.clear() }
                    37 => { close_contour(st, l, line); st.region = false }
                    74 => st.multi = false, 75 => st.multi = true,
                    70 => st.mm = false, 71 => st.mm = true,
                    90 | 91 | 54 | 55 => {}
                    4 => return Ok(()),
                    g => return Err(format!("G{g:02} in {w}")),
                }
            }
            'X' => nx = Some(coord(st, &read_num(&mut i), 0)?),
            'Y' => ny = Some(coord(st, &read_num(&mut i), 1)?),
            'I' => ni = Some(coord(st, &read_num(&mut i), 0)?),
            'J' => nj = Some(coord(st, &read_num(&mut i), 1)?),
            'D' => d = Some(read_num(&mut i).parse().map_err(|_| format!("D code in {w}"))?),
            'M' => { let _ = read_num(&mut i); }
            '*' => {}
            _ => return Err(format!("{c} in {w}")),
        }
    }
    let target = (nx.unwrap_or(st.x), ny.unwrap_or(st.y));
    let moved_somewhere = nx.is_some() || ny.is_some();
    match d {
        Some(n) if n >= 10 => { if !st.region { flush(st, l, line) } if !st.aps.contains_key(&n) { return Err(format!("D{n} is not defined")) } st.cur_ap = n; }
        Some(2) => {
            if st.region { close_contour(st, l, line); st.contour.push(target) } else { flush(st, l, line) }
            st.x = target.0; st.y = target.1;
        }
        Some(1) | None if moved_somewhere || d == Some(1) => {
            let from = (st.x, st.y);
            let pts = if st.mode == 1 { vec![target] } else {
                let (i0, j0) = (ni.unwrap_or(0.0), nj.unwrap_or(0.0));
                let c = if st.multi { (from.0 + i0, from.1 + j0) } else { quadrant_centre(from, target, i0.abs(), j0.abs(), st.mode == 2) };
                if st.multi && (from.0 - target.0).abs() < 1e-9 && (from.1 - target.1).abs() < 1e-9 {
                    // a whole circle
                    let r = (i0 * i0 + j0 * j0).sqrt();
                    let a0 = (from.1 - c.1).atan2(from.0 - c.0);
                    let sgn = if st.mode == 2 { -1.0 } else { 1.0 };
                    (1..=60).map(|k| { let t = a0 + sgn * std::f64::consts::TAU * k as f64 / 60.0; (c.0 + r * t.cos(), c.1 + r * t.sin()) }).collect()
                } else { arc_points(from, target, c, st.mode == 2)[1..].to_vec() }
            };
            if st.region {
                if st.contour.is_empty() { st.contour.push(from) }
                st.contour.extend(pts);
            } else {
                if st.path.is_empty() { st.path.push(from) }
                st.path.extend(pts);
            }
            st.x = target.0; st.y = target.1;
        }
        Some(3) => {
            flush(st, l, line);
            st.x = target.0; st.y = target.1;
            let Some((ap, function)) = st.aps.get(&st.cur_ap) else { return Err(format!("a flash with no aperture selected (D{})", st.cur_ap)) };
            for (shape, dark) in ap.flash(target) { l.objs.push(Obj { shape, dark: if dark { st.dark } else { !st.dark }, owner: Owner { function: function.clone(), ..st.owner.clone() }, line, aperture: st.cur_ap }) }
        }
        None => { st.x = target.0; st.y = target.1; }
        Some(n) => return Err(format!("D{n:02} in {w}")),
    }
    Ok(())
}
/// G74's centre: the one of the four signed offsets that gives an arc of at most 90 degrees with the same radius at both ends.
fn quadrant_centre(s: P, e: P, i: f64, j: f64, cw: bool) -> P {
    let mut best = (s.0 + i, s.1 + j); let mut err = f64::MAX;
    for (si, sj) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        let c = (s.0 + si * i, s.1 + sj * j);
        let (r0, r1) = (((s.0 - c.0).powi(2) + (s.1 - c.1).powi(2)).sqrt(), ((e.0 - c.0).powi(2) + (e.1 - c.1).powi(2)).sqrt());
        let a0 = (s.1 - c.1).atan2(s.0 - c.0); let a1 = (e.1 - c.1).atan2(e.0 - c.0);
        let mut sweep = if cw { a0 - a1 } else { a1 - a0 };
        while sweep < 0.0 { sweep += std::f64::consts::TAU }
        if sweep <= std::f64::consts::FRAC_PI_2 + 1e-6 && (r0 - r1).abs() < err { err = (r0 - r1).abs(); best = c }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    const KICAD: &str = "%TF.GenerationSoftware,KiCad,Pcbnew,10.99*%\n%TF.FileFunction,Copper,L1,Top*%\n%TF.FilePolarity,Positive*%\n%FSLAX46Y46*%\nG04 Gerber Fmt 4.6*\n%MOMM*%\n%LPD*%\nG01*\n\
%TA.AperFunction,SMDPad,CuDef*%\n%ADD10R,1.000000X0.500000*%\n%TA.AperFunction,Conductor*%\n%ADD11C,0.250000*%\n\
%AMRoundRect*\n0 Rectangle with rounded corners*\n0 $1 Rounding radius*\n0 $2 $3 $4 $5 $6 $7 $8 $9 X,Y pos of 4 corners*\n0 Add a 4 corners polygon primitive as pad body*\n4,1,4,$2,$3,$4,$5,$6,$7,$8,$9,$2,$3,0*\n0 Add four circle primitives for the rounded corners*\n1,1,$1+$1,$2,$3*\n1,1,$1+$1,$4,$5*\n1,1,$1+$1,$6,$7*\n1,1,$1+$1,$8,$9*\n0 Add four rect primitives between the rounded corners*\n20,1,$1+$1,$2,$3,$4,$5,0*\n20,1,$1+$1,$4,$5,$6,$7,0*\n20,1,$1+$1,$6,$7,$8,$9,0*\n20,1,$1+$1,$8,$9,$2,$3,0*%\n\
%ADD12RoundRect,0.250000X-0.450000X-0.350000X0.450000X-0.350000X0.450000X0.350000X-0.450000X0.350000X0*%\n\
%TO.P,R1,1*%\n%TO.N,GND*%\nD10*\nX100000000Y-50000000D03*\n%TD*%\n%TO.N,/SDA*%\nD11*\nX100000000Y-50000000D02*\nX110000000Y-50000000D01*\nX110000000Y-60000000D01*\n%TD*%\nD12*\nX120000000Y-50000000D03*\n\
G36*\nX0Y0D02*\nX10000000Y0D01*\nX10000000Y10000000D01*\nX0Y10000000D01*\nX0Y0D01*\nG37*\nM02*\n";
    #[test] fn reads_kicad_output() {
        let l = parse(KICAD);
        assert!(l.errors.is_empty(), "{:?}", l.errors); assert!(l.ended);
        assert_eq!(l.function, "Copper,L1,Top");
        // the pad, the track (one stroke of two segments), the round rect (polygon, 4 discs, 4 bars), the region
        assert_eq!(l.objs.len(), 1 + 1 + 9 + 1);
        let pad = &l.objs[0];
        assert_eq!(pad.owner, Owner { net: "GND".into(), part: "R1".into(), pin: "1".into(), function: "SMDPad,CuDef".into() });
        assert_eq!(pad.shape, Shape::Poly { ps: vec![(99.5, -50.25), (100.5, -50.25), (100.5, -49.75), (99.5, -49.75)] });
        assert_eq!(l.objs[1].shape, Shape::Stroke { ps: vec![(100.0, -50.0), (110.0, -50.0), (110.0, -60.0)], w: 0.25 });
        assert_eq!(l.objs[1].owner.net, "/SDA"); assert_eq!(l.objs[1].owner.part, "", "TD cleared the part");
        assert!(matches!(l.objs[3].shape, Shape::Disc { r, .. } if (r - 0.25).abs() < 1e-9), "the corner circles: $1+$1 is the diameter");
        assert!(matches!(&l.objs[11].shape, Shape::Poly { ps } if ps.len() == 5), "the region's contour");
    }
    #[test] fn expressions() {
        let v: HashMap<u32, f64> = [(1, 2.0), (2, 3.0)].into();
        assert_eq!(eval("$1+$1", &v), Some(4.0)); assert_eq!(eval("$1x$2-1", &v), Some(5.0)); assert_eq!(eval("-($1+$2)/2", &v), Some(-2.5));
        assert_eq!(eval("2X3", &v), Some(6.0)); assert_eq!(eval("$1+", &v), None); assert_eq!(eval("1/0", &v), None);
    }
    #[test] fn arcs_quadrants_and_circles() {
        // a quarter circle counterclockwise (G75), centre at the origin
        let l = parse("%FSLAX24Y24*%%MOIN*%%ADD10C,0.01*%D10*G75*X10000Y0D02*G03X0Y10000I-10000J0D01*M02*");
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        let Shape::Stroke { ps, w } = &l.objs[0].shape else { panic!() };
        assert!((w - 0.254).abs() < 1e-9, "inches to mm");
        let (a, b) = (ps[0], *ps.last().unwrap()); assert!((a.0 - 25.4).abs() < 1e-9 && (b.1 - 25.4).abs() < 1e-9);
        assert!(ps.iter().all(|(x, y)| ((x * x + y * y).sqrt() - 25.4).abs() < 1e-6), "on the circle");
        // single quadrant (G74): unsigned offsets, the centre chosen
        let l = parse("%FSLAX24Y24*%%MOMM*%%ADD10C,0.1*%D10*G74*X10000Y0D02*G02X0Y-10000I10000J0D01*M02*");
        let Shape::Stroke { ps, .. } = &l.objs[0].shape else { panic!() };
        assert!(ps.iter().all(|(x, y)| ((x * x + y * y).sqrt() - 1.0).abs() < 1e-6), "{ps:?}");
        // a whole circle
        let l = parse("%FSLAX24Y24*%%MOMM*%%ADD10C,0.1*%D10*G75*X10000Y0D02*G02X10000Y0I-10000J0D01*M02*");
        let Shape::Stroke { ps, .. } = &l.objs[0].shape else { panic!() }; assert!(ps.len() > 40);
    }
    #[test] fn polarity_step_repeat_and_omitted_coordinates() {
        let l = parse("%FSLAX24Y24*%%MOMM*%%ADD10C,1*%%SRX2Y3I5.0J2.0*%D10*X0Y0D03*%SR*%%LPC*%X10000Y20000D03*Y30000D03*M02*");
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        assert_eq!(l.objs.len(), 6 + 2);
        assert!(l.objs[..6].iter().all(|o| o.dark)); assert!(l.objs[6..].iter().all(|o| !o.dark));
        assert!(l.objs[..6].iter().any(|o| o.shape == Shape::Disc { c: (5.0, 4.0), r: 0.5 }));
        assert_eq!(l.objs[7].shape, Shape::Disc { c: (1.0, 3.0), r: 0.5 }, "X kept from the word before");
        // trailing zeros omitted (T)
        let l = parse("%FSTAX24Y24*%%MOMM*%%ADD10C,1*%D10*X1Y25D03*M02*");
        assert_eq!(l.objs[0].shape, Shape::Disc { c: (10.0, 25.0), r: 0.5 });
    }
    #[test] fn a_bad_file_says_where_and_keeps_the_rest() {
        let l = parse("%FSLAX24Y24*%\n%MOMM*%\n%ADD10C,1*%\nD10*\nX0Y0D03*\nX1Q2D03*\nD99*\nX20000Y0D03*\n%ADD11FOO,1*%\nX30000Y0D03*\n");
        assert_eq!(l.objs.len(), 3, "the good flashes are kept");
        let lines: Vec<usize> = l.errors.iter().map(|e| e.0).collect();
        assert_eq!(lines, [6, 7, 9], "{:?}", l.errors);
        assert!(l.errors[1].1.contains("D99") && l.errors[2].1.contains("FOO"));
        assert!(!l.ended);
    }
}
