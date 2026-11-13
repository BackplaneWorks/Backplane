//! Backplane's own Gerber viewer, hub side: a fab output set (an mfg/ folder, or one Gerber or drill file) read into the same board items
//! the board viewer draws (kicad/plotgen.rs `items_plot`), so the renderer, the layer list, picking, the viewers' probe and the phones'
//! data path are the board's. Each file is put on the board layer it is (its .gbrjob entry, its X2 TF.FileFunction, else its name),
//! Gerber's Y up turned to the board's Y down so a set lies over its board. An object's X2 attributes (net, part, pin, aperture function)
//! are its info card, as a board item's are. A file that does not read fully is kept as far as it read; what did not read is said,
//! file and line (`notes`).
pub mod compare;
pub mod excellon;
pub mod names;
pub mod rs274x;

use super::board::{self, Item, Prim, Pt};
use std::{path::Path, sync::Arc};

pub use names::*;

fn modified_ms(p: &Path) -> u64 { std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64) }
/// A span of time as a person says it: "40 s", "12 min", "3 h", "2 d".
pub fn span(ms: u64) -> String { let s = ms / 1000; if s < 90 { format!("{s} s") } else if s < 5400 { format!("{} min", s / 60) } else if s < 172800 { format!("{} h", s / 3600) } else { format!("{} d", s / 86400) } }
/// The bar's line for a set: where it is, how many files, and how old against its board (stale, a warning, when the board was saved
/// more than a minute after the newest file of the set).
pub fn set_line(label: &str, files: usize, newest_ms: u64, board_ms: u64) -> (String, bool) {
    let n = format!("{label} · {files} file{}", if files == 1 { "" } else { "s" });
    if board_ms == 0 || newest_ms == 0 { return (n, false) }
    if board_ms > newest_ms + 60_000 { (format!("{n} · made {} before the board's last change", span(board_ms - newest_ms)), true) } else { (format!("{n} · made after the board's last change"), false) }
}
fn label_of(set: &Path, board: Option<&Path>) -> String {
    let root = board.and_then(Path::parent);
    root.and_then(|r| set.strip_prefix(r).ok()).map(|r| r.to_string_lossy().replace('\\', "/")).filter(|r| !r.is_empty()).unwrap_or_else(|| set.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// What did not read in the sets read last (by path), for their viewers' note.
static NOTES: std::sync::Mutex<Vec<(String, Vec<String>)>> = std::sync::Mutex::new(Vec::new());
pub fn notes_of(path: &str) -> Vec<String> { NOTES.lock().unwrap().iter().find(|(p, _)| p == path).map(|x| x.1.clone()).unwrap_or_default() }
/// Differences found by comparisons not yet taken up by the Checks panel (kicad/checkrun.rs): (set, board, [(layer, difference)], how
/// long before the board's last change the set was made (ms; 0 not before)).
pub type Diffs = (String, String, Vec<(String, compare::Place)>, u64);
pub static DIFFS: std::sync::Mutex<Vec<Diffs>> = std::sync::Mutex::new(Vec::new());
fn cmp_board_is(path: &str) -> bool { path.starts_with(CMP) }

/// A set as a plot, as the board viewer draws one (kicad/plotread.rs `read` sends a Gerber path here), with its note: the set, its age
/// against the board, what did not read; compared with the board (a CMP path), the differences marked and counted.
pub fn plot(path: &str) -> super::plotgen::Read {
    let (set, cmp_board) = match path.strip_prefix(CMP) { Some(r) => { let (a, b) = r.split_once('\n').unwrap_or((r, "")); (a.to_string(), Some(b.to_string())) } None => (path.to_string(), None) };
    let sp = Path::new(&set);
    let r = read_set(sp);
    { let mut n = NOTES.lock().unwrap(); n.retain(|(p, _)| p != path); n.push((path.to_string(), r.notes.clone())); if n.len() > 32 { n.remove(0); } }
    let board = board_of(sp);
    let newest = set_files(sp).iter().map(|f| modified_ms(f)).max().unwrap_or(0);
    let board_ms = board.as_deref().map(modified_ms).unwrap_or(0);
    let (mut note, mut warn) = set_line(&label_of(sp, board.as_deref()), r.layers.len(), newest, board_ms);
    let older = if newest > 0 && board_ms > newest + 60_000 { board_ms - newest } else { 0 };
    if !r.notes.is_empty() { warn = true; note.push_str(&format!(" · {}{}", r.notes[0], if r.notes.len() > 1 { format!(" (and {} more)", r.notes.len() - 1) } else { String::new() })); }
    let (items, kind) = match cmp_board {
        None => (r.items, super::plotgen::GERBER),
        Some(b) => {
            let bytes = std::fs::read(&b).unwrap_or_default();
            let bi = super::sexpr::forms(&bytes).map(|fs| { let mut v = super::plotgen::board_items(&fs); v.extend(compare::board_copper_polys(&fs)); v }).unwrap_or_default();
            let t0 = std::time::Instant::now();
            let (marks, layers) = compare::compare(&r.items, &bi);
            eprintln!("gerber: {} compared with its board in {} ms ({} marks)", set, t0.elapsed().as_millis(), marks.len());
            // the bar: the total (each layer's count is in the layer list: names.rs COUNTS); each difference to the Checks panel (DIFFS)
            let total: usize = layers.iter().map(|l| l.places.len()).sum();
            let differ = layers.iter().filter(|l| l.removed + l.added > 0).count();
            if total == 0 { note.push_str(&format!(" · vs board: the same on {} layers", layers.len())) } else { warn = true; note.push_str(&format!(" · vs board: {total} difference{} on {differ} layer{}", if total == 1 { "" } else { "s" }, if differ == 1 { "" } else { "s" })) }
            set_counts(&set, layers.iter().map(|l| (l.name.clone(), l.removed, l.added)).collect());
            { let mut d = DIFFS.lock().unwrap(); d.retain(|x| x.0 != set); d.push((set.clone(), b.clone(), layers.iter().flat_map(|l| l.places.iter().map(|p| (l.name.clone(), p.clone()))).collect(), older)); }
            let mut items = r.items; items.extend(marks);
            (items, super::plotgen::GERBER | super::plotgen::DIFF)
        }
    };
    let (mut p, t) = super::plotgen::items_plot(items, kind);
    p.thick = 0;
    // a set (or a file) with no outline is fitted to what it draws
    if (p.edge.x0 as i32) > (p.edge.x1 as i32) { p.edge = p.b }
    p.note = note; p.warn = warn;
    // Fit fits the board's outline (not parts placed off it, nor marks of theirs)
    if (p.edge.x0 as i32) <= (p.edge.x1 as i32) && cmp_board_is(path) { p.b = p.edge }
    super::plotgen::Read::Plot(Box::new(p), t)
}

/// A layer function (X2 "Copper,L2,Inr", "Soldermask,Top", ...) as a board layer name; "" for one the board has no layer for.
pub fn layer_of_function(f: &str) -> String {
    let p: Vec<&str> = f.split(',').map(str::trim).collect();
    let side = |top: &str, bot: &str| -> String { if p.iter().any(|x| *x == "Bot") { bot.into() } else { top.into() } };
    match p.first().map(|x| x.to_ascii_lowercase()).unwrap_or_default().as_str() {
        "copper" => {
            let n: u32 = p.get(1).and_then(|l| l.strip_prefix('L')).and_then(|l| l.parse().ok()).unwrap_or(1);
            match p.get(2).copied().unwrap_or("") { "Top" => "F.Cu".into(), "Bot" => "B.Cu".into(), _ if (2..=7).contains(&n) => format!("In{}.Cu", n - 1), _ => "*.Cu".into() }
        }
        "soldermask" => side("F.Mask", "B.Mask"),
        "paste" | "solderpaste" => side("F.Paste", "B.Paste"),
        "legend" => side("F.SilkS", "B.SilkS"),
        "profile" => "Edge.Cuts".into(),
        "assemblydrawing" | "component" => side("F.Fab", "B.Fab"),
        "plated" | "nonplated" => "hole".into(),
        "" => String::new(),
        _ => "Dwgs.User".into(),
    }
}
/// A file's layer by its name: KiCad's "<board>-F_Cu.gbr" or a Protel extension.
pub fn layer_of_name(name: &str) -> String {
    let n = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let (stem, ext) = n.rsplit_once('.').unwrap_or((n, ""));
    let ext = ext.to_ascii_lowercase();
    if is_drill(n) { return "hole".into() }
    for (k, v) in [("F_Cu", "F.Cu"), ("B_Cu", "B.Cu"), ("F_Mask", "F.Mask"), ("B_Mask", "B.Mask"), ("F_Paste", "F.Paste"), ("B_Paste", "B.Paste"), ("F_Silkscreen", "F.SilkS"), ("B_Silkscreen", "B.SilkS"),
        ("F_SilkS", "F.SilkS"), ("B_SilkS", "B.SilkS"), ("Edge_Cuts", "Edge.Cuts"), ("F_Fab", "F.Fab"), ("B_Fab", "B.Fab"), ("F_Courtyard", "F.CrtYd"), ("B_Courtyard", "B.CrtYd")] {
        if stem.ends_with(&format!("-{k}")) || stem.ends_with(&format!("_{k}")) || stem == k { return v.into() }
    }
    if let Some(p) = stem.rfind("-In") { if let Some(k) = stem[p + 3..].strip_suffix("_Cu").and_then(|d| d.parse::<u32>().ok()) { return if k <= 6 { format!("In{k}.Cu") } else { "*.Cu".into() } } }
    match ext.as_str() {
        "gtl" => "F.Cu", "gbl" => "B.Cu", "gts" => "F.Mask", "gbs" => "B.Mask", "gto" => "F.SilkS", "gbo" => "B.SilkS", "gtp" => "F.Paste", "gbp" => "B.Paste",
        "gm1" | "gm2" | "gm" | "gko" | "gml" => "Edge.Cuts",
        e if e.starts_with('g') && e[1..].parse::<u32>().map_or(false, |k| (1..=6).contains(&k)) => return format!("In{}.Cu", &e[1..]),
        e if e.starts_with("gl") && e[2..].parse::<u32>().map_or(false, |k| (2..=7).contains(&k)) => return format!("In{}.Cu", e[2..].parse::<u32>().unwrap() - 1),
        _ => "Dwgs.User",
    }.into()
}
/// A job file's (file name -> layer function).
pub fn job_functions(text: &str) -> Vec<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    v["FilesAttributes"].as_array().into_iter().flatten().filter_map(|f| Some((f["Path"].as_str()?.to_string(), f["FileFunction"].as_str()?.to_string()))).collect()
}

/// Files read (content hash with name and job function -> items, layer, notes), so a set opened again, or with one file exported anew,
/// parses only what changed.
type Cached = (String, Vec<Item>, String, Vec<String>);
static CACHE: std::sync::Mutex<Vec<Cached>> = std::sync::Mutex::new(Vec::new());

/// One file of a set as read: its board layer, what it held, what did not read.
#[derive(Debug, Default)]
pub struct Read { pub items: Vec<Item>, pub layers: Vec<(String, String, usize)>, pub notes: Vec<String> }

fn pt((x, y): (f64, f64)) -> Pt { Pt { x: x as f32, y: -y as f32 } }
fn info(title: &str, fields: &[(&str, &str)]) -> board::Info {
    let mut s = title.replace([';', '\n'], ",");
    for (k, v) in fields { if !v.is_empty() { s.push_str(&format!(";{k}={}", v.replace([';', '\n'], ","))) } }
    Arc::from(s.as_str())
}
/// Clear (negative) objects are painted in the viewer's board colour over what they clear.
const CLEAR: u32 = 0x10141a;
fn disc_ring(c: (f64, f64), r: f64) -> Vec<Pt> { (0..32).map(|i| { let t = std::f64::consts::TAU * i as f64 / 32.0; pt((c.0 + r * t.cos(), c.1 + r * t.sin())) }).collect() }

/// A Gerber's objects as board items on `layer`.
pub fn gerber_items(file: &str, l: &rs274x::Layer, layer: u32, out: &mut Vec<Item>) {
    for (i, o) in l.objs.iter().enumerate() {
        let prim = match (&o.shape, o.dark) {
            (rs274x::Shape::Stroke { ps, w }, true) if ps.len() == 2 => Prim::Seg { a: pt(ps[0]), b: pt(ps[1]), w: *w as f32 },
            (rs274x::Shape::Stroke { ps, w }, true) => Prim::Line { ps: ps.iter().map(|p| pt(*p)).collect(), w: *w as f32 },
            (rs274x::Shape::Disc { c, r }, true) => Prim::Circ { c: pt(*c), r: *r as f32, w: 0.0, fill: true },
            (rs274x::Shape::Poly { ps }, true) => Prim::Poly { ps: ps.iter().map(|p| pt(*p)).collect(), w: 0.0, fill: true },
            (rs274x::Shape::Disc { c, r }, false) => board::tint_of(disc_ring(*c, *r), CLEAR, 255),
            (rs274x::Shape::Poly { ps }, false) => board::tint_of(ps.iter().map(|p| pt(*p)).collect(), CLEAR, 255),
            (rs274x::Shape::Stroke { .. }, false) => continue, // a clearing draw: rare in fab output, not drawn
        };
        let w = &o.owner;
        let (kind, _) = w.function.split_once(',').unwrap_or((&w.function, ""));
        let title = if !w.part.is_empty() && !w.pin.is_empty() { format!("pad {}.{}", w.part, w.pin) } else if kind.is_empty() { match &o.shape { rs274x::Shape::Stroke { .. } => "draw".into(), rs274x::Shape::Disc { .. } => "flash".into(), rs274x::Shape::Poly { .. } => "region".into() } } else { kind.to_lowercase() };
        let line = o.line.to_string(); let ap = if o.aperture > 0 { format!("D{}", o.aperture) } else { String::new() };
        let inf = info(&title, &[("ref", &w.part), ("pin", &w.pin), ("net", &w.net), ("function", &w.function), ("aperture", &ap), ("file", file), ("line", &line)]);
        let key = format!("{file}#{i}");
        out.push(Item { sig: board::hash(key.as_bytes(), board::FNV), key, layer, prim, info: inf });
    }
}
/// A drill file's holes as board items on the hole layer.
pub fn drill_items(file: &str, d: &excellon::Drills, plated: bool, out: &mut Vec<Item>) {
    let hole = board::layer_id("hole");
    for (i, (h, line)) in d.holes.iter().enumerate() {
        let (prim, size) = match h {
            excellon::Hole::Drill { c, d } => (Prim::Circ { c: pt(*c), r: (*d / 2.0) as f32, w: 0.0, fill: true }, *d),
            excellon::Hole::Slot { a, b, d } => (Prim::Seg { a: pt(*a), b: pt(*b), w: *d as f32 }, *d),
        };
        let key = format!("{file}#{i}");
        let (sz, ln) = (format!("{size:.3} mm"), line.to_string());
        let inf = info(if matches!(h, excellon::Hole::Slot { .. }) { "slot" } else { "hole" }, &[("size", &sz), ("plated", if plated { "yes" } else { "no" }), ("file", file), ("line", &ln)]);
        out.push(Item { sig: board::hash(key.as_bytes(), board::FNV), key, layer: hole, prim, info: inf });
    }
}

/// A set (folder, job file, or one file) read: its items, each file's (name, layer, objects), and what did not read.
pub fn read_set(path: &Path) -> Read {
    let files = set_files(path);
    let mut r = Read::default();
    let jobs: Vec<(String, String)> = files.iter().filter(|f| is_job(&f.to_string_lossy())).filter_map(|f| std::fs::read_to_string(f).ok()).flat_map(|t| job_functions(&t)).collect();
    for f in files.iter().filter(|f| !is_job(&f.to_string_lossy())) {
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let bytes = match std::fs::read(f) { Ok(b) => b, Err(e) => { r.notes.push(format!("{name}: {e}")); continue } };
        // a file read before (the same bytes, name and job) is not parsed again: its items, layer and notes are kept (CACHE)
        let job_fn = jobs.iter().find(|(p, _)| p.rsplit(['/', '\\']).next() == Some(&name)).map(|(_, f)| f.clone()).unwrap_or_default();
        let h = format!("{:016x}{name}\u{1}{job_fn}", bytes.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3)));
        if let Some((items, layer, notes)) = CACHE.lock().unwrap().iter().find(|e| e.0 == h).map(|e| (e.1.clone(), e.2.clone(), e.3.clone())) {
            r.layers.push((name, layer, items.len())); r.items.extend(items); r.notes.extend(notes);
            continue
        }
        let (n0, k0) = (r.items.len(), r.notes.len());
        let text = String::from_utf8_lossy(&bytes);
        let layer_name = if is_drill(&name) {
            let d = excellon::parse(&text);
            let plated = d.plated.unwrap_or_else(|| !name.to_ascii_uppercase().contains("NPTH"));
            drill_items(&name, &d, plated, &mut r.items);
            for (line, e) in d.errors.iter().take(20) { r.notes.push(format!("{name}:{line}: {e}")) }
            if d.errors.len() > 20 { r.notes.push(format!("{name}: {} more lines not read", d.errors.len() - 20)) }
            if plated { "hole".to_string() } else { "hole (non-plated)".to_string() }
        } else {
            let g = rs274x::parse(&text);
            let by_job = jobs.iter().find(|(p, _)| p.rsplit(['/', '\\']).next() == Some(&name)).map(|(_, func)| layer_of_function(func));
            let by_fn = by_job.filter(|l| !l.is_empty()).or_else(|| Some(layer_of_function(&g.function)).filter(|l| !l.is_empty()));
            // a generic function ("Other,User": KiCad's courtyards and user layers) says less than the name
            let ln = match by_fn { Some(l) if l != "Dwgs.User" => l, _ => layer_of_name(&name) };
            gerber_items(&name, &g, board::layer_id(&ln), &mut r.items);
            for (line, e) in g.errors.iter().take(20) { r.notes.push(format!("{name}:{line}: {e}")) }
            if g.errors.len() > 20 { r.notes.push(format!("{name}: {} more words not read", g.errors.len() - 20)) }
            if !g.ended && !g.objs.is_empty() { r.notes.push(format!("{name}: no end (M02): the file may be cut short")) }
            // a mask is "negative" by nature (its openings are drawn); a negative copper layer (a plane) is drawn as written, which is not what it makes
            if g.negative && ln.ends_with(".Cu") { r.notes.push(format!("{name}: a negative copper layer (a plane): drawn as written, which is the copper removed")) }
            ln
        };
        r.layers.push((name.clone(), layer_name.clone(), r.items.len() - n0));
        let mut c = CACHE.lock().unwrap();
        c.push((h, r.items[n0..].to_vec(), layer_name, r.notes[k0..].to_vec()));
        // about the files of a few sets
        while c.len() > 256 { c.remove(0); }
    }
    if files.is_empty() { r.notes.push("no Gerber or drill files here".into()) }
    r
}

/// The stamp of a set: every file's (name, size, modified), so a new export (files rewritten in place or replaced) shows at once.
pub fn set_stamp(path: &Path) -> String {
    let mut s = String::new();
    for f in set_files(path) { if let Ok(m) = std::fs::metadata(&f) { s.push_str(&format!("{}:{}:{:?};", f.display(), m.len(), m.modified().ok())) } }
    format!("{:016x}", s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_sets_line() {
        assert_eq!(set_line("mfg/b/gerber", 37, 1_000_000, 1_000_000 + 2 * 3_600_000), ("mfg/b/gerber · 37 files · made 2 h before the board's last change".into(), true));
        assert_eq!(set_line("mfg/b/gerber", 1, 1_000_000, 1_030_000), ("mfg/b/gerber · 1 file · made after the board's last change".into(), false));
        assert_eq!(set_line("x.gtl", 1, 5, 0), ("x.gtl · 1 file".into(), false));
        assert_eq!((span(40_000), span(720_000), span(3 * 86_400_000)), ("40 s".into(), "12 min".into(), "3 d".into()));
    }
    #[test] fn layers_by_function_name_and_extension() {
        assert_eq!(layer_of_function("Copper,L1,Top"), "F.Cu"); assert_eq!(layer_of_function("Copper,L8,Bot"), "B.Cu");
        assert_eq!(layer_of_function("Copper,L3,Inr"), "In2.Cu"); assert_eq!(layer_of_function("Copper,L9,Inr"), "*.Cu");
        assert_eq!(layer_of_function("Soldermask,Bot"), "B.Mask"); assert_eq!(layer_of_function("SolderMask,Top"), "F.Mask"); assert_eq!(layer_of_function("SolderPaste,Bot"), "B.Paste");
        assert_eq!(layer_of_name("b-F_Courtyard.gbr"), "F.CrtYd"); assert_eq!(layer_of_function("Legend,Top"), "F.SilkS"); assert_eq!(layer_of_function("Profile,NP"), "Edge.Cuts");
        assert_eq!(layer_of_name("Hubble_DK-F_Cu.gbr"), "F.Cu"); assert_eq!(layer_of_name("Hubble_DK-In3_Cu.gbr"), "In3.Cu"); assert_eq!(layer_of_name("Hubble_DK-Edge_Cuts.gm1"), "Edge.Cuts");
        assert_eq!(layer_of_name("board.GBO"), "B.SilkS"); assert_eq!(layer_of_name("board.g2"), "In2.Cu"); assert_eq!(layer_of_name("board.gl3"), "In2.Cu"); assert_eq!(layer_of_name("x-NPTH.drl"), "hole");
        assert!(is_gerber("a.GTL") && is_gerber("a-F_Cu.gbr") && is_gerber("a.g4") && !is_gerber("a.gbrjob") && is_drill("a-PTH.drl") && is_job("a-job.gbrjob") && !is_fab_file("a.kicad_pcb"));
        let j = job_functions(r#"{"FilesAttributes":[{"Path":"b-F_Cu.gbr","FileFunction":"Copper,L1,Top","FilePolarity":"Positive"},{"Path":"b-In1_Cu.gbr","FileFunction":"Copper,L2,Inr"}]}"#);
        assert_eq!(j[1], ("b-In1_Cu.gbr".to_string(), "Copper,L2,Inr".to_string()));
    }
    /// A real set (BACKPLANE_TEST_GERBERS=<folder>): every file read with no note, timed.
    #[test] fn a_real_set_reads_cleanly() {
        let Ok(dir) = std::env::var("BACKPLANE_TEST_GERBERS") else { eprintln!("skipped: no BACKPLANE_TEST_GERBERS"); return };
        let t = std::time::Instant::now();
        let r = read_set(Path::new(&dir));
        let ms = t.elapsed().as_millis();
        let t2 = std::time::Instant::now();
        let (p, _) = super::super::plotgen::items_plot(r.items, 0);
        eprintln!("read {ms} ms, plot {} ms, {} chunks", t2.elapsed().as_millis(), p.cs.len());
        for (n, l, k) in &r.layers { eprintln!("  {n} -> {l}: {k}") }
        assert!(r.notes.is_empty(), "{:?}", r.notes);
    }
    fn centre(p: &Prim) -> (f32, f32) {
        // the box's centre (a point average counts a closing point twice, and arcs' points unevenly)
        let avg = |ps: &[Pt]| { let (mut a, mut b) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN)); for q in ps { a = (a.0.min(q.x), a.1.min(q.y)); b = (b.0.max(q.x), b.1.max(q.y)) } ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0) };
        match p { Prim::Circ { c, .. } => (c.x, c.y), Prim::Seg { a, b, .. } => ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0), Prim::Line { ps, .. } | Prim::Poly { ps, .. } | Prim::Tint { ps, .. } => avg(ps) }
    }
    fn field<'a>(info: &'a str, k: &str) -> &'a str { info.split(';').find_map(|f| f.strip_prefix(&format!("{k}="))).unwrap_or("") }
    /// The oracle (BACKPLANE_TEST_GERBERS=<the set> BACKPLANE_TEST_BOARD=<the board it was exported from>): every pad the top copper
    /// Gerber flashes for a part's pin lies where the board has that pad (within 2 µm), and every such board pad is in the Gerber.
    #[test] fn the_set_matches_its_board() {
        let (Ok(dir), Ok(pcb)) = (std::env::var("BACKPLANE_TEST_GERBERS"), std::env::var("BACKPLANE_TEST_BOARD")) else { eprintln!("skipped: no BACKPLANE_TEST_GERBERS / BACKPLANE_TEST_BOARD"); return };
        let r = read_set(Path::new(&dir));
        let fcu = board::layer_id("F.Cu");
        let mut g: std::collections::HashMap<String, Vec<(f32, f32)>> = Default::default();
        for i in r.items.iter().filter(|i| i.layer == fcu && i.info.starts_with("pad ")) {
            g.entry(format!("{}.{}", field(&i.info, "ref"), field(&i.info, "pin"))).or_default().push(centre(&i.prim));
        }
        let b = std::fs::read(&pcb).unwrap();
        let fs = super::super::sexpr::forms(&b).unwrap();
        let items = super::super::plotgen::board_items(&fs);
        let (mut matched, mut far, mut missing) = (0, vec![], vec![]);
        for i in items.iter().filter(|i| i.info.starts_with("pad ") && (i.layer == fcu || i.layer == board::layer_id("*.Cu"))) {
            let pin = i.info[4..].split(';').next().unwrap_or("");
            let rf = field(&i.info, "ref");
            if rf.is_empty() || pin.is_empty() { continue }
            let k = format!("{rf}.{pin}");
            let c = centre(&i.prim);
            match g.get(&k) {
                None => missing.push(k),
                Some(v) => { let d = v.iter().map(|q| ((q.0 - c.0).powi(2) + (q.1 - c.1).powi(2)).sqrt()).fold(f32::MAX, f32::min); if d <= 0.002 { matched += 1 } else { far.push((k, d)) } }
            }
        }
        eprintln!("pads matched {matched}, far {}, missing {}", far.len(), missing.len());
        assert!(matched > 0);
        assert!(far.is_empty(), "{:?}", &far[..far.len().min(5)]);
        assert!(missing.len() * 20 <= matched, "pads on the board but not in the Gerber: {:?}", &missing[..missing.len().min(8)]);
    }
    /// The comparison on a real set (BACKPLANE_TEST_GERBERS, BACKPLANE_TEST_BOARD): what each layer came to, and how long it took.
    #[test] fn a_real_set_compared_with_its_board() {
        let (Ok(dir), Ok(pcb)) = (std::env::var("BACKPLANE_TEST_GERBERS"), std::env::var("BACKPLANE_TEST_BOARD")) else { eprintln!("skipped"); return };
        let r = read_set(Path::new(&dir));
        let b = std::fs::read(&pcb).unwrap(); let fs = super::super::sexpr::forms(&b).unwrap(); let mut bi = super::super::plotgen::board_items(&fs); bi.extend(compare::board_copper_polys(&fs));
        let t = std::time::Instant::now();
        let (marks, layers) = compare::compare(&r.items, &bi);
        eprintln!("compared in {} ms", t.elapsed().as_millis());
        eprintln!("{} marks", marks.len());
        for l in &layers { eprintln!("  {}: {} differences", l.name, l.places.len()); for p in l.places.iter().take(12) { eprintln!("    {}: {} ({:.2} x {:.2} mm at {})", p.side(), p.what, p.b[2] - p.b[0], p.b[3] - p.b[1], compare::at_text(p.b)) } }
        if let Ok(at) = std::env::var("BACKPLANE_TEST_AT") {
            let (x, y) = at.split_once(',').map(|(a, b)| (a.parse::<f32>().unwrap(), b.parse::<f32>().unwrap())).unwrap();
            for i in bi.iter().filter(|i| { let c = centre(&i.prim); (c.0 - x).abs() < 1.5 && (c.1 - y).abs() < 1.5 }) { eprintln!("  board item layer {} {}: {:?}", i.layer, i.info, i.prim) }
            let bx = |p: &Prim| { let ps: Vec<Pt> = match p { Prim::Poly { ps, .. } | Prim::Line { ps, .. } | Prim::Tint { ps, .. } => ps.clone(), Prim::Seg { a, b, .. } => vec![*a, *b], Prim::Circ { c, .. } => vec![*c] }; ps.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, q| [a[0].min(q.x), a[1].min(q.y), a[2].max(q.x), a[3].max(q.y)]) };
            for i in r.items.iter().filter(|i| { let b = bx(&i.prim); b[0] <= x && x <= b[2] && b[1] <= y && y <= b[3] && (5..=12).contains(&i.layer) }) { let b = bx(&i.prim); eprintln!("  gerber item layer {} {} box {:?} pts {}", i.layer, i.info, b, match &i.prim { Prim::Poly { ps, .. } => ps.len(), _ => 0 }) }
            for i in bi.iter().filter(|i| { let b = bx(&i.prim); b[0] <= x && x <= b[2] && b[1] <= y && y <= b[3] && i.info.starts_with("zone") }) { eprintln!("  board zone layer {} {} box {:?} pts {}", i.layer, i.info, bx(&i.prim), match &i.prim { Prim::Poly { ps, .. } => ps.len(), _ => 0 }) }
        }
        if std::env::var("BACKPLANE_TEST_SAME").is_ok() { assert!(layers.iter().all(|l| l.places.is_empty()), "a fresh export differs from its board") }
    }
    #[test] fn a_set_reads_into_board_items() {
        let d = crate::plat::tmp::dir("gerber-set");
        std::fs::create_dir_all(d.join("gerbers")).unwrap();
        std::fs::write(d.join("gerbers/b-F_Cu.gbr"), "%TF.FileFunction,Copper,L1,Top*%\n%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.25*%\n%TO.N,GND*%\nD10*\nX0Y0D02*\nX1000000Y-2000000D01*\n%TD*%\nM02*\n").unwrap();
        std::fs::write(d.join("gerbers/b-Edge_Cuts.gm1"), "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.1*%\nD10*\nX0Y0D02*\nX10000000Y0D01*\nX10000000Y-10000000D01*\nX0Y-10000000D01*\nX0Y0D01*\nM02*\n").unwrap();
        std::fs::write(d.join("gerbers/b-PTH.drl"), "M48\nMETRIC\nT1C0.300\n%\nT1\nX1.0Y-2.0\nM30\n").unwrap();
        std::fs::write(d.join("gerbers/b-B_Cu.gbr"), "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.25*%\nD10*\nX0Y0D03*\nX1Q1D03*\n").unwrap();
        std::fs::write(d.join("notes.txt"), "x").unwrap();
        let r = read_set(&d);
        let names: Vec<(&str, &str, usize)> = r.layers.iter().map(|(a, b, n)| (a.as_str(), b.as_str(), *n)).collect();
        assert_eq!(names, [("b-B_Cu.gbr", "B.Cu", 1), ("b-Edge_Cuts.gm1", "Edge.Cuts", 1), ("b-F_Cu.gbr", "F.Cu", 1), ("b-PTH.drl", "hole", 1)]);
        let track = r.items.iter().find(|i| i.layer == board::layer_id("F.Cu")).unwrap();
        assert_eq!(track.prim, Prim::Seg { a: Pt { x: 0.0, y: 0.0 }, b: Pt { x: 1.0, y: 2.0 }, w: 0.25 }, "Y turned down, as the board's");
        assert!(track.info.contains("net=GND") && track.info.contains("file=b-F_Cu.gbr"));
        let hole = r.items.iter().find(|i| i.layer == board::layer_id("hole")).unwrap();
        assert_eq!(hole.prim, Prim::Circ { c: Pt { x: 1.0, y: 2.0 }, r: 0.15, w: 0.0, fill: true });
        assert_eq!(r.notes, ["b-B_Cu.gbr:6: Q in X1Q1D03", "b-B_Cu.gbr: no end (M02): the file may be cut short"]);
        // the plot is the board's kind: the edge box from Edge.Cuts
        let (p, _) = super::super::plotgen::items_plot(r.items, 0);
        assert_eq!((p.edge.x1 as i32, p.edge.y1 as i32), (10000, 10000));
        // one file alone
        assert_eq!(read_set(&d.join("gerbers/b-PTH.drl")).layers.len(), 1);
        assert_eq!(default_set(&d), Some(d.join("gerbers")), "a gerbers/ folder is a set too");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir_all(d.join("mfg/b/gerber")).unwrap(); std::fs::write(d.join("mfg/b/gerber/b-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap(); std::fs::write(d.join("mfg/b/b.zip"), "x").unwrap();
        assert_eq!(default_set(&d), Some(d.join("mfg/b/gerber")), "the set written last");
        assert!(is_set_path(&d.join("gerbers")) && is_set_path(&d.join("gerbers/b-PTH.drl")) && !is_set_path(&d.join("notes.txt")) && !is_set_path(&d.join("mfg/b")));
        // read again: the same items (from the cache); a file exported anew is read anew
        let again = read_set(&d.join("gerbers"));
        assert_eq!(again.items, read_set(&d.join("gerbers")).items);
        std::fs::write(d.join("gerbers/b-PTH.drl"), "M48\nMETRIC\nT1C0.500\n%\nT1\nX1.0Y-2.0\nM30\n").unwrap();
        let hole = |r: &Read| r.items.iter().find(|i| i.layer == board::layer_id("hole")).map(|i| i.prim.clone());
        assert_eq!(hole(&read_set(&d.join("gerbers"))), Some(Prim::Circ { c: Pt { x: 1.0, y: 2.0 }, r: 0.25, w: 0.0, fill: true }));
        let s0 = set_stamp(&d); std::fs::write(d.join("gerbers/b-PTH.drl"), "M48\nMETRIC\nT1C0.400\n%\nT1\nX1.0Y-2.0\nM30\n").unwrap(); assert_ne!(set_stamp(&d), s0);
        let _ = std::fs::remove_dir_all(&d);
    }
}
