//! Hub-side MCP tool `viewer_render`: render KiCad schematics, 2D PCBs, 3D PCBs, and STEP/GLB/STL
//! mechanical models to cached PNGs with optional hierarchical sheet selection and refdes/bbox zoom crops.
use super::{board, render, schematic, sexpr};
use crate::hub::Hub;
use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_PCB_LAYERS: &[&str] = &["F.Cu", "B.Cu", "F.Mask", "F.SilkS", "Edge.Cuts"];
const DEFAULT_PCB3D_VIEWS: &[&str] = &["top", "bottom", "iso"];
const DEFAULT_MECH_VIEWS: &[&str] = &["iso", "front", "top", "right"];
const DEFAULT_2D_WIDTH: u32 = 1600;
const DEFAULT_3D_WIDTH: u32 = 1200;
const DEFAULT_PCB_MARGIN_MM: f64 = 6.0;
const DEFAULT_SCH_MARGIN_MM: f64 = 10.0;
const MAX_INLINE_IMAGES: usize = 4;
const MAX_SINGLE_INLINE_BYTES: usize = 3_500_000;
const MAX_INLINE_BYTES: usize = 6 << 20;
const MAX_SHEETS_ALL: usize = 8;
const CLI_TIMEOUT: Duration = Duration::from_secs(120);

fn unique_part_path(target: &Path, suffix: &str) -> PathBuf {
    static PART_SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = PART_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("part");
    let name = format!("{stem}.{pid}.{seq}.{suffix}");
    match target.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(name),
        _ => PathBuf::from(name),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Schematic,
    Pcb,
    Pcb3d,
    Mech,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Schematic => "schematic",
            Kind::Pcb => "pcb",
            Kind::Pcb3d => "3d",
            Kind::Mech => "mech",
        }
    }

    pub fn from_ext(path: &str) -> Result<Self, String> {
        let low = path.to_ascii_lowercase();
        if low.ends_with(".kicad_sch") {
            Ok(Kind::Schematic)
        } else if low.ends_with(".kicad_pcb") {
            Ok(Kind::Pcb)
        } else if mech_ext(&low) {
            Ok(Kind::Mech)
        } else {
            Err(format!(
                "unsupported file extension for viewer_render: {path} (expected .kicad_sch, .kicad_pcb, .step, .stp, .glb, .stl, .sc, or .solidcraft)"
            ))
        }
    }

    pub fn parse(raw: &str, path: &str) -> Result<Self, String> {
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Self::from_ext(path);
        }
        let k = match trimmed.as_str() {
            "schematic" | "sch" => Kind::Schematic,
            "pcb" | "board" => Kind::Pcb,
            "3d" | "pcb3d" => Kind::Pcb3d,
            "mech" | "model" => Kind::Mech,
            _ => {
                return Err(format!(
                    "unknown kind {raw:?} (expected schematic, pcb, 3d, or mech)"
                ))
            }
        };
        let low = path.to_ascii_lowercase();
        let compatible = match k {
            Kind::Schematic => low.ends_with(".kicad_sch"),
            Kind::Pcb | Kind::Pcb3d => low.ends_with(".kicad_pcb"),
            Kind::Mech => mech_ext(&low),
        };
        if !compatible {
            return Err(format!(
                "kind {:?} does not match file extension of {path}",
                k.as_str()
            ));
        }
        Ok(k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bbox {
    pub x_mm: f64,
    pub y_mm: f64,
    pub w_mm: f64,
    pub h_mm: f64,
}

impl Bbox {
    pub fn expand(self, margin_mm: f64) -> Self {
        let m = margin_mm.max(0.0);
        Bbox {
            x_mm: self.x_mm - m,
            y_mm: self.y_mm - m,
            w_mm: (self.w_mm + 2.0 * m).max(0.5),
            h_mm: (self.h_mm + 2.0 * m).max(0.5),
        }
    }

    pub fn to_json(self) -> Value {
        json!({
            "x_mm": round4(self.x_mm),
            "y_mm": round4(self.y_mm),
            "w_mm": round4(self.w_mm),
            "h_mm": round4(self.h_mm),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Focus {
    Refdes(String),
    Box(Bbox),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub kind: Kind,
    pub sheet: Option<String>,
    pub layers: Vec<String>,
    pub views: Vec<String>,
    pub focus: Option<Focus>,
    pub margin_mm: Option<f64>,
    pub width: Option<u32>,
    pub dpi: Option<f64>,
    pub inline: bool,
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

fn num_f64(v: Option<&Value>) -> Option<f64> {
    let v = v?;
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn parse_u32(v: Option<&Value>, name: &str) -> Result<Option<u32>, String> {
    let Some(v) = v else { return Ok(None) };
    if v.is_null() {
        return Ok(None);
    }
    if let Some(n) = v.as_u64() {
        return Ok(Some(n.min(u32::MAX as u64) as u32));
    }
    if let Some(f) = v.as_f64() {
        if f.is_finite() && f >= 1.0 {
            return Ok(Some(f.round().min(u32::MAX as f64) as u32));
        }
    }
    Err(format!("{name} must be a positive number"))
}

fn parse_focus(v: Option<&Value>) -> Result<Option<Focus>, String> {
    let Some(v) = v else { return Ok(None) };
    if v.is_null() {
        return Ok(None);
    }
    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                Ok(None)
            } else {
                Ok(Some(Focus::Refdes(t.to_string())))
            }
        }
        Value::Object(o) => {
            let x = num_f64(o.get("x_mm"))
                .filter(|x| x.is_finite())
                .ok_or_else(|| "focus.x_mm must be a finite number".to_string())?;
            let y = num_f64(o.get("y_mm"))
                .filter(|y| y.is_finite())
                .ok_or_else(|| "focus.y_mm must be a finite number".to_string())?;
            let w = num_f64(o.get("w_mm"))
                .filter(|w| w.is_finite() && *w > 0.0)
                .ok_or_else(|| "focus.w_mm must be a positive number".to_string())?;
            let h = num_f64(o.get("h_mm"))
                .filter(|h| h.is_finite() && *h > 0.0)
                .ok_or_else(|| "focus.h_mm must be a positive number".to_string())?;
            Ok(Some(Focus::Box(Bbox {
                x_mm: x,
                y_mm: y,
                w_mm: w,
                h_mm: h,
            })))
        }
        _ => Err("focus must be a reference designator string (e.g. \"U2\") or {x_mm, y_mm, w_mm, h_mm}".into()),
    }
}

fn parse_str_list(v: Option<&Value>, name: &str) -> Result<Option<Vec<String>>, String> {
    let Some(v) = v else { return Ok(None) };
    if v.is_null() {
        return Ok(None);
    }
    let Some(arr) = v.as_array() else {
        return Err(format!("{name} must be an array of strings"));
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(s) = item.as_str() else {
            return Err(format!("{name} entries must be strings"));
        };
        let t = s.trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
    }
    if out.is_empty() {
        Ok(None)
    } else {
        Ok(Some(out))
    }
}

fn normalize_pcb3d_view(v: &str) -> Result<String, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "top" => Ok("top".into()),
        "bottom" => Ok("bottom".into()),
        "iso" => Ok("iso".into()),
        "iso_bottom" | "iso-bottom" => Ok("iso_bottom".into()),
        "front" => Ok("front".into()),
        "back" => Ok("back".into()),
        "left" => Ok("left".into()),
        "right" => Ok("right".into()),
        _ => Err(format!(
            "unsupported 3D PCB view {v:?} (expected top, bottom, iso, iso_bottom, front, back, left, or right)"
        )),
    }
}

fn normalize_mech_view(v: &str) -> Result<String, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "iso" => Ok("iso".into()),
        "front" => Ok("front".into()),
        "back" => Ok("back".into()),
        "top" => Ok("top".into()),
        "bottom" => Ok("bottom".into()),
        "right" => Ok("right".into()),
        "left" => Ok("left".into()),
        _ => Err(format!(
            "unsupported mech view {v:?} (expected iso, front, back, top, bottom, right, or left)"
        )),
    }
}

impl Params {
    pub fn parse(path: &str, a: &Value) -> Result<Self, String> {
        let raw_kind = a.get("kind").and_then(Value::as_str).unwrap_or("");
        let kind = Kind::parse(raw_kind, path)?;

        let sheet = match a.get("sheet") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => {
                let t = s.trim();
                (!t.is_empty()).then(|| t.to_string())
            }
            Some(Value::Number(n)) => match n.as_u64() {
                Some(p) if p >= 1 => Some(p.to_string()),
                _ => return Err("numeric sheet must be a positive integer page number".into()),
            },
            Some(_) => return Err("sheet must be a string or positive integer page number".into()),
        };

        let layers = match parse_str_list(a.get("layers"), "layers")? {
            Some(ls) => ls,
            None => DEFAULT_PCB_LAYERS.iter().map(|s| (*s).to_string()).collect(),
        };

        let raw_views = parse_str_list(a.get("views"), "views")?;
        let views = match kind {
            Kind::Pcb3d => {
                let vs = raw_views
                    .unwrap_or_else(|| DEFAULT_PCB3D_VIEWS.iter().map(|s| (*s).to_string()).collect());
                vs.iter()
                    .map(|v| normalize_pcb3d_view(v))
                    .collect::<Result<Vec<_>, _>>()?
            }
            Kind::Mech => {
                let vs = raw_views
                    .unwrap_or_else(|| DEFAULT_MECH_VIEWS.iter().map(|s| (*s).to_string()).collect());
                vs.iter()
                    .map(|v| normalize_mech_view(v))
                    .collect::<Result<Vec<_>, _>>()?
            }
            Kind::Schematic | Kind::Pcb => raw_views.unwrap_or_default(),
        };

        let focus = parse_focus(a.get("focus"))?;
        if focus.is_some() && !matches!(kind, Kind::Schematic | Kind::Pcb) {
            return Err(format!(
                "focus is only supported for schematic and 2D pcb renders (got kind {:?})",
                kind.as_str()
            ));
        }

        let margin_mm = match a.get("margin_mm") {
            None | Some(Value::Null) => None,
            Some(v) => {
                let m = num_f64(Some(v))
                    .filter(|m| m.is_finite() && *m >= 0.0)
                    .ok_or_else(|| "margin_mm must be a non-negative finite number".to_string())?;
                Some(m.clamp(0.0, 200.0))
            }
        };

        let width = parse_u32(a.get("width"), "width")?.map(|w| match kind {
            Kind::Schematic | Kind::Pcb => w.clamp(200, 3200),
            Kind::Pcb3d | Kind::Mech => w.clamp(200, 2400),
        });

        let dpi = match a.get("dpi") {
            None | Some(Value::Null) => None,
            Some(v) => {
                let d = num_f64(Some(v))
                    .filter(|d| d.is_finite() && *d > 0.0)
                    .ok_or_else(|| "dpi must be a positive finite number".to_string())?;
                Some(d.clamp(36.0, 1200.0))
            }
        };

        let inline = match a.get("inline") {
            None | Some(Value::Null) => true,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err("inline must be a boolean".into()),
        };

        Ok(Params {
            kind,
            sheet,
            layers,
            views,
            focus,
            margin_mm,
            width,
            dpi,
            inline,
        })
    }

    fn effective_margin_for_refdes(&self) -> f64 {
        self.margin_mm.unwrap_or(match self.kind {
            Kind::Pcb => DEFAULT_PCB_MARGIN_MM,
            Kind::Schematic => DEFAULT_SCH_MARGIN_MM,
            Kind::Pcb3d | Kind::Mech => 0.0,
        })
    }
}

/// Read `(width, height)` from a PNG's IHDR header.
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 {
        return None;
    }
    if &bytes[0..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    (w > 0 && h > 0).then_some((w, h))
}

/// Deterministic cache key for `(content_hash, cli_version, params)`.
pub fn cache_key(content_hash: &str, cli_version: &str, params: &Params) -> String {
    let mut h = Sha256::new();
    let mut feed = |k: &str, v: &str| {
        h.update(k.as_bytes());
        h.update(b"=");
        h.update((v.len() as u64).to_le_bytes());
        h.update(v.as_bytes());
        h.update(b"\n");
    };
    feed("v", "vrender-v1");
    feed("cli", cli_version);
    feed("hash", content_hash);
    feed("kind", params.kind.as_str());
    feed("sheet", params.sheet.as_deref().unwrap_or(""));
    if matches!(params.kind, Kind::Pcb) {
        feed("layers", &params.layers.join(","));
    }
    if matches!(params.kind, Kind::Pcb3d | Kind::Mech) {
        feed("views", &params.views.join(","));
    }
    match &params.focus {
        None => feed("focus", ""),
        Some(Focus::Refdes(r)) => feed("focus", &format!("ref:{r}")),
        Some(Focus::Box(b)) => feed(
            "focus",
            &format!(
                "box:{:.4},{:.4},{:.4},{:.4}",
                b.x_mm, b.y_mm, b.w_mm, b.h_mm
            ),
        ),
    }
    feed(
        "margin",
        &params
            .margin_mm
            .map(|m| format!("{m:.4}"))
            .unwrap_or_default(),
    );
    feed(
        "width",
        &params.width.map(|w| w.to_string()).unwrap_or_default(),
    );
    feed(
        "dpi",
        &params.dpi.map(|d| format!("{d:.4}")).unwrap_or_default(),
    );
    hex::encode(h.finalize())[..24].to_string()
}

fn sha256_file_into(h: &mut Sha256, path: &Path) -> Result<(), String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let mut buf = [0u8; 65536];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(())
}

/// Extract `(Sheetname, Sheetfile)` entries from a `.kicad_sch`.
pub fn sch_subsheets(sch_bytes: &[u8]) -> Vec<(String, String)> {
    let Some(forms) = sexpr::forms(sch_bytes) else {
        return vec![];
    };
    let mut out = vec![];
    for f in forms {
        if schematic::head(f) != b"sheet" {
            continue;
        }
        let x = sexpr::parse(f);
        let mut name = board::prop(&x, "Sheetname").to_string();
        if name.is_empty() {
            name = board::prop(&x, "Sheet name").to_string();
        }
        let mut file = board::prop(&x, "Sheetfile").to_string();
        if file.is_empty() {
            file = board::prop(&x, "Sheet file").to_string();
        }
        if !file.is_empty() {
            out.push((name, file));
        }
    }
    out
}

fn collect_sch_files(root: &Path, depth: u32, seen: &mut BTreeSet<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(canon) = crate::plat::fs::canonicalize(root).or_else(|_| Ok::<_, std::io::Error>(root.to_path_buf())) else {
        return;
    };
    if !seen.insert(canon) {
        return;
    }
    let Ok(bytes) = std::fs::read(root) else {
        return;
    };
    let Some(dir) = root.parent() else {
        return;
    };
    for (_name, rel) in sch_subsheets(&bytes) {
        let child = dir.join(&rel);
        if child.is_file() {
            collect_sch_files(&child, depth + 1, seen);
        }
    }
}

fn sibling_pro(path: &Path) -> Option<PathBuf> {
    let pro = super::ipc::pro_of(&path.to_string_lossy());
    if pro.is_empty() {
        None
    } else {
        let p = PathBuf::from(pro);
        p.is_file().then_some(p)
    }
}

/// If `path` is a hierarchical subsheet, resolve to `(root_schematic, Some(auto_sheet_name))`.
/// Otherwise returns `(path, None)`.
pub fn resolve_root_schematic(path: &Path) -> (PathBuf, Option<String>) {
    let Some(dir) = path.parent() else {
        return (path.to_path_buf(), None);
    };
    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
        return (path.to_path_buf(), None);
    };
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name)
        .to_string();

    if let Some(pro) = sibling_pro(path) {
        if let Some(pro_stem) = pro.file_stem().and_then(|s| s.to_str()) {
            let root_cand = pro.with_file_name(format!("{pro_stem}.kicad_sch"));
            if root_cand.is_file() && root_cand != path {
                let mut tree = BTreeSet::new();
                collect_sch_files(&root_cand, 0, &mut tree);
                let canon_self = crate::plat::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
                if tree.contains(&canon_self) {
                    if let Ok(bytes) = std::fs::read(&root_cand) {
                        let subs = sch_subsheets(&bytes);
                        if let Some((sname, _)) = subs.iter().find(|(_, f)| {
                            Path::new(f).file_name().and_then(|n| n.to_str()) == Some(file_name)
                        }) {
                            return (
                                root_cand,
                                Some(if sname.is_empty() { stem } else { sname.clone() }),
                            );
                        }
                    }
                    return (root_cand, Some(stem));
                }
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut cands: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p != path
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("kicad_sch"))
            })
            .collect();
        cands.sort();
        for cand in cands {
            if let Ok(bytes) = std::fs::read(&cand) {
                if let Some((sname, _)) = sch_subsheets(&bytes).into_iter().find(|(_, f)| {
                    Path::new(f).file_name().and_then(|n| n.to_str()) == Some(file_name)
                }) {
                    return (
                        cand,
                        Some(if sname.is_empty() { stem } else { sname }),
                    );
                }
            }
        }
    }

    (path.to_path_buf(), None)
}

fn content_hash_for(path: &Path, kind: Kind) -> Result<String, String> {
    let mut h = Sha256::new();
    match kind {
        Kind::Schematic => {
            let mut files = BTreeSet::new();
            collect_sch_files(path, 0, &mut files);
            if files.is_empty() {
                files.insert(path.to_path_buf());
            }
            for f in &files {
                h.update(f.to_string_lossy().as_bytes());
                h.update(b"\0");
                sha256_file_into(&mut h, f)?;
            }
            if let Some(pro) = sibling_pro(path) {
                h.update(b"pro\0");
                let _ = sha256_file_into(&mut h, &pro);
            }
        }
        Kind::Pcb | Kind::Pcb3d => {
            sha256_file_into(&mut h, path)?;
            if let Some(pro) = sibling_pro(path) {
                h.update(b"pro\0");
                let _ = sha256_file_into(&mut h, &pro);
            }
        }
        Kind::Mech => {
            sha256_file_into(&mut h, path)?;
        }
    }
    Ok(hex::encode(h.finalize()))
}

struct AccBox {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    any: bool,
}

impl AccBox {
    fn new() -> Self {
        AccBox {
            x0: f64::INFINITY,
            y0: f64::INFINITY,
            x1: f64::NEG_INFINITY,
            y1: f64::NEG_INFINITY,
            any: false,
        }
    }

    fn add_pt(&mut self, x: f64, y: f64, r: f64) {
        if x.is_finite() && y.is_finite() && r.is_finite() {
            self.x0 = self.x0.min(x - r);
            self.y0 = self.y0.min(y - r);
            self.x1 = self.x1.max(x + r);
            self.y1 = self.y1.max(y + r);
            self.any = true;
        }
    }

    fn add_prim(&mut self, prim: &board::Prim) {
        match prim {
            board::Prim::Seg { a, b, w } => {
                let r = (*w as f64) * 0.5;
                self.add_pt(a.x as f64, a.y as f64, r);
                self.add_pt(b.x as f64, b.y as f64, r);
            }
            board::Prim::Line { ps, w } | board::Prim::Poly { ps, w, .. } => {
                let r = (*w as f64) * 0.5;
                for p in ps {
                    self.add_pt(p.x as f64, p.y as f64, r);
                }
            }
            board::Prim::Circ { c, r, w, .. } => {
                self.add_pt(c.x as f64, c.y as f64, (*r as f64) + (*w as f64) * 0.5);
            }
            board::Prim::Tint { ps, .. } => {
                for p in ps {
                    self.add_pt(p.x as f64, p.y as f64, 0.0);
                }
            }
        }
    }

    fn finish(self) -> Option<Bbox> {
        self.any.then(|| Bbox {
            x_mm: self.x0,
            y_mm: self.y0,
            w_mm: (self.x1 - self.x0).max(0.5),
            h_mm: (self.y1 - self.y0).max(0.5),
        })
    }
}

fn footprint_ref<'a>(x: &'a sexpr::Sx<'a>) -> &'a str {
    let r = board::prop(x, "Reference");
    if !r.is_empty() {
        return r;
    }
    x.args()
        .iter()
        .find(|c| c.name() == "fp_text" && c.arg(0) == "reference")
        .map(|c| c.arg(1))
        .unwrap_or("")
}

/// Extract a footprint's bounding box in board millimetres by reference designator.
pub fn pcb_refdes_bbox(pcb_bytes: &[u8], refdes: &str) -> Option<Bbox> {
    let want = refdes.trim();
    if want.is_empty() {
        return None;
    }
    let forms = sexpr::forms(pcb_bytes)?;
    let mut found_ci = None;
    let mut matched = None;
    for form in forms {
        if schematic::head(form) != b"footprint" {
            continue;
        }
        let x = sexpr::parse(form);
        let r = footprint_ref(&x);
        if r == want {
            matched = Some(x);
            break;
        } else if found_ci.is_none() && r.eq_ignore_ascii_case(want) {
            found_ci = Some(x);
        }
    }
    let x = matched.or(found_ci)?;
    let mut items = Vec::new();
    board::form(&mut items, &x, 0, 0);

    let mut geom = AccBox::new();
    let mut all = AccBox::new();
    for it in &items {
        all.add_prim(&it.prim);
        if !it.info.starts_with("text") {
            geom.add_prim(&it.prim);
        }
    }
    if let Some(b) = geom.finish().or_else(|| all.finish()) {
        return Some(b);
    }
    let pl = board::fp_place(&x);
    Some(Bbox {
        x_mm: pl.x as f64 - 0.5,
        y_mm: pl.y as f64 - 0.5,
        w_mm: 1.0,
        h_mm: 1.0,
    })
}

fn info_ref_value(info: &str) -> Option<&str> {
    for part in info.split(';') {
        if let Some(r) = part.strip_prefix("ref=") {
            return Some(r);
        }
        if let Some(r) = part.strip_prefix("symbol=") {
            return Some(r);
        }
    }
    None
}

fn ref_matches(candidate: &str, want: &str, allow_unit_suffix: bool) -> bool {
    if candidate.eq_ignore_ascii_case(want) {
        return true;
    }
    if allow_unit_suffix
        && candidate.len() > want.len()
        && candidate[..want.len()].eq_ignore_ascii_case(want)
    {
        let suf = &candidate[want.len()..];
        return suf.len() <= 2 && suf.chars().all(|c| c.is_ascii_uppercase());
    }
    false
}

/// Extract a schematic symbol's bounding box in sheet millimetres by reference designator.
pub fn sch_refdes_bbox(sch_bytes: &[u8], refdes: &str) -> Option<Bbox> {
    let want = refdes.trim();
    if want.is_empty() {
        return None;
    }
    let forms = sexpr::forms(sch_bytes)?;
    let order: Vec<u32> = (0..forms.len() as u32).collect();
    let items = schematic::items(&forms, &|n, f| (0..n).map(f).collect(), &order);

    for allow_unit in [false, true] {
        let mut acc = AccBox::new();
        for it in &items {
            if let Some(r) = info_ref_value(&it.info) {
                if ref_matches(r, want, allow_unit) {
                    acc.add_prim(&it.prim);
                }
            }
        }
        if let Some(b) = acc.finish() {
            return Some(b);
        }
    }

    // Fallback: symbol form exists even without drawable library primitives.
    for form in &forms {
        if schematic::head(form) != b"symbol" {
            continue;
        }
        let x = sexpr::parse(form);
        let r = board::prop(&x, "Reference");
        if ref_matches(r, want, true) {
            let p = board::at(&x, "at");
            return Some(Bbox {
                x_mm: p.x as f64 - 2.5,
                y_mm: p.y as f64 - 2.5,
                w_mm: 5.0,
                h_mm: 5.0,
            });
        }
    }
    None
}

fn parse_viewbox(raw: &str) -> Result<Bbox, String> {
    let nums: Vec<f64> = raw
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("invalid SVG viewBox: {raw:?}"))?;
    if nums.len() != 4 || nums.iter().any(|v| !v.is_finite()) || nums[2] <= 0.0 || nums[3] <= 0.0 {
        return Err(format!("invalid SVG viewBox: {raw:?}"));
    }
    Ok(Bbox {
        x_mm: nums[0],
        y_mm: nums[1],
        w_mm: nums[2],
        h_mm: nums[3],
    })
}

fn find_svg_tag_bounds(svg: &str) -> Result<(usize, usize), String> {
    let mut idx = 0;
    while let Some(rel) = svg[idx..].find("<svg") {
        let start = idx + rel;
        let after = svg.as_bytes().get(start + 4).copied();
        if matches!(after, Some(b' ' | b'\t' | b'\r' | b'\n' | b'>')) {
            let bytes = svg.as_bytes();
            let mut i = start + 4;
            let mut quote: Option<u8> = None;
            while i < bytes.len() {
                let b = bytes[i];
                match quote {
                    Some(q) => {
                        if b == q {
                            quote = None;
                        }
                    }
                    None => {
                        if b == b'"' || b == b'\'' {
                            quote = Some(b);
                        } else if b == b'>' {
                            return Ok((start, i));
                        }
                    }
                }
                i += 1;
            }
            return Err("unterminated <svg> tag".into());
        }
        idx = start + 4;
    }
    Err("exported SVG contains no <svg> tag".into())
}

fn parse_svg_attrs(inner: &str) -> Vec<(String, String)> {
    let bytes = inner.as_bytes();
    let mut i = 0;
    let mut attrs = Vec::new();
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name_start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'=' {
            i += 1;
        }
        let name = inner[name_start..i].to_string();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let val = if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let q = bytes[i];
                i += 1;
                let vs = i;
                while i < bytes.len() && bytes[i] != q {
                    i += 1;
                }
                let s = inner[vs..i].to_string();
                if i < bytes.len() {
                    i += 1;
                }
                s
            } else {
                let vs = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                inner[vs..i].to_string()
            };
            attrs.push((name, val));
        } else if !name.is_empty() {
            attrs.push((name, String::new()));
        }
    }
    attrs
}

/// Rewrite an exported KiCad SVG's `viewBox`, `width`, `height` (in mm) and optionally inject a dark background `<rect>`.
pub fn rewrite_svg_viewport(svg: &str, focus: Option<Bbox>, dark_bg: bool) -> Result<(String, Bbox), String> {
    let (start, gt) = find_svg_tag_bounds(svg)?;
    let self_closing = gt > start && svg.as_bytes()[gt - 1] == b'/';
    let attr_end = if self_closing { gt - 1 } else { gt };
    let attrs = parse_svg_attrs(&svg[start + 4..attr_end]);

    let orig_vb = attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("viewBox"))
        .map(|(_, v)| parse_viewbox(v))
        .transpose()?
        .unwrap_or(Bbox {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 297.0,
            h_mm: 210.0,
        });

    let target = match focus {
        Some(f) => Bbox {
            x_mm: f.x_mm,
            y_mm: f.y_mm,
            w_mm: f.w_mm.max(0.5),
            h_mm: f.h_mm.max(0.5),
        },
        None => orig_vb,
    };

    let mut out = String::with_capacity(svg.len() + 256);
    out.push_str(&svg[..start]);
    out.push_str("<svg");
    for (k, v) in &attrs {
        if k.eq_ignore_ascii_case("width")
            || k.eq_ignore_ascii_case("height")
            || k.eq_ignore_ascii_case("viewBox")
        {
            continue;
        }
        out.push(' ');
        out.push_str(k);
        if !v.is_empty() {
            out.push_str("=\"");
            out.push_str(v);
            out.push('"');
        }
    }
    out.push_str(&format!(
        " width=\"{:.4}mm\" height=\"{:.4}mm\" viewBox=\"{:.4} {:.4} {:.4} {:.4}\"",
        target.w_mm, target.h_mm, target.x_mm, target.y_mm, target.w_mm, target.h_mm
    ));
    if self_closing {
        out.push_str("/>");
    } else {
        out.push('>');
        if dark_bg {
            out.push_str(&format!(
                "\n<rect x=\"{:.4}\" y=\"{:.4}\" width=\"{:.4}\" height=\"{:.4}\" fill=\"#14161a\"/>",
                target.x_mm, target.y_mm, target.w_mm, target.h_mm
            ));
        }
    }
    out.push_str(&svg[gt + 1..]);
    Ok((out, target))
}

/// Compute target `(width, height)` in pixels from `viewBox` dimensions and optional `width` or `dpi`.
pub fn target_pixel_size(vb: Bbox, width: Option<u32>, dpi: Option<f64>) -> (u32, u32) {
    let mut w = if let Some(w) = width {
        w.clamp(200, 3200)
    } else if let Some(d) = dpi {
        ((vb.w_mm / 25.4) * d.clamp(36.0, 1200.0))
            .round()
            .clamp(200.0, 3200.0) as u32
    } else {
        DEFAULT_2D_WIDTH
    };
    let aspect = vb.h_mm / vb.w_mm.max(0.01);
    let mut h = ((w as f64) * aspect).round().max(1.0) as u32;
    if h > 2400 {
        w = ((w as f64) * (2400.0 / h as f64)).round().max(200.0) as u32;
        h = 2400;
    }
    (w, h)
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let runnable = |p: &Path| {
        #[cfg(unix)]
        {
            use crate::plat::fs::PermissionsExt;
            std::fs::metadata(p)
                .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            p.is_file()
        }
    };
    let names = if std::env::consts::EXE_SUFFIX.is_empty() {
        vec![name.to_string()]
    } else {
        vec![
            format!("{name}{}", std::env::consts::EXE_SUFFIX),
            name.to_string(),
        ]
    };
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        for cand_name in &names {
            let p = dir.join(cand_name);
            if runnable(&p) {
                return Some(p);
            }
        }
    }
    None
}

async fn run_local(program: &Path, args: &[String], headless_display: bool) -> Result<std::process::Output, String> {
    let mut cmd = crate::proc::tokio_cmd(crate::proc::Kind::Local, program);
    cmd.args(args);
    cmd.kill_on_drop(true);
    if headless_display {
        cmd.env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY");
    }
    let out = tokio::time::timeout(CLI_TIMEOUT, crate::proc::output(&mut cmd))
        .await
        .map_err(|_| format!("{} timed out after {}s", program.display(), CLI_TIMEOUT.as_secs()))?
        .map_err(|e| format!("failed to run {}: {e}", program.display()))?;
    Ok(out)
}

async fn rasterize_svg_to_png(svg_path: &Path, png_path: &Path, px_w: u32, vb: Bbox) -> Result<(u32, u32), String> {
    let part_path = unique_part_path(png_path, "part.png");
    let svg_str = svg_path.to_string_lossy().into_owned();
    let part_str = part_path.to_string_lossy().into_owned();
    let w_str = px_w.to_string();

    let mut tried = Vec::new();
    let mut last_err = String::new();

    let mut attempt = |label: &str, out: Result<std::process::Output, String>| -> Option<(u32, u32)> {
        tried.push(label.to_string());
        match out {
            Ok(o) if o.status.success() => {
                if let Ok(bytes) = std::fs::read(&part_path) {
                    if let Some((w, h)) = png_dimensions(&bytes) {
                        if crate::plat::fs::rename(&part_path, png_path).is_ok() {
                            return Some((w, h));
                        }
                    }
                }
                let _ = std::fs::remove_file(&part_path);
                last_err = format!("{label} produced no valid PNG");
                None
            }
            Ok(o) => {
                let _ = std::fs::remove_file(&part_path);
                let msg = String::from_utf8_lossy(&o.stderr).trim().to_string();
                let msg = if msg.is_empty() {
                    String::from_utf8_lossy(&o.stdout).trim().to_string()
                } else {
                    msg
                };
                last_err = format!(
                    "{label} exited with {}: {}",
                    o.status.code().unwrap_or(-1),
                    msg.chars().take(300).collect::<String>()
                );
                None
            }
            Err(e) => {
                let _ = std::fs::remove_file(&part_path);
                last_err = format!("{label}: {e}");
                None
            }
        }
    };

    if let Some(bin) = find_on_path("rsvg-convert") {
        let args = vec!["-w".into(), w_str.clone(), "-o".into(), part_str.clone(), svg_str.clone()];
        if let Some(dims) = attempt("rsvg-convert", run_local(&bin, &args, false).await) {
            return Ok(dims);
        }
    }
    if let Some(bin) = find_on_path("resvg") {
        let args = vec!["--width".into(), w_str.clone(), svg_str.clone(), part_str.clone()];
        if let Some(dims) = attempt("resvg", run_local(&bin, &args, false).await) {
            return Ok(dims);
        }
    }
    for py_name in ["python3", "python"] {
        if let Some(bin) = find_on_path(py_name) {
            let code = "import cairosvg, sys; cairosvg.svg2png(url=sys.argv[1], write_to=sys.argv[2], output_width=int(sys.argv[3]))";
            let args = vec!["-c".into(), code.into(), svg_str.clone(), part_str.clone(), w_str.clone()];
            if let Some(dims) = attempt(&format!("{py_name}(cairosvg)"), run_local(&bin, &args, false).await) {
                return Ok(dims);
            }
        }
    }
    if let Some(bin) = find_on_path("inkscape") {
        let args = vec![
            "--export-type=png".into(),
            format!("--export-width={w_str}"),
            format!("--export-filename={part_str}"),
            svg_str.clone(),
        ];
        if let Some(dims) = attempt("inkscape", run_local(&bin, &args, true).await) {
            return Ok(dims);
        }
    }
    const IM_BINS: &[&str] = if cfg!(windows) { &["magick"] } else { &["magick", "convert"] };
    for &im_name in IM_BINS {
        if let Some(bin) = find_on_path(im_name) {
            let density = ((px_w as f64) / (vb.w_mm.max(1.0) / 25.4)).clamp(36.0, 1200.0);
            let args = vec![
                "-background".into(),
                "none".into(),
                "-density".into(),
                format!("{density:.1}"),
                svg_str.clone(),
                "-resize".into(),
                format!("{w_str}x"),
                part_str.clone(),
            ];
            if let Some(dims) = attempt(im_name, run_local(&bin, &args, false).await) {
                return Ok(dims);
            }
        }
    }

    if tried.is_empty() {
        Err("no SVG-to-PNG rasterizer found on PATH (tried rsvg-convert, resvg, python3 cairosvg, inkscape, magick)".into())
    } else {
        Err(format!("SVG-to-PNG rasterization failed (tried {}): {last_err}", tried.join(", ")))
    }
}

struct StageDir(PathBuf);
impl StageDir {
    fn new(base: &Path) -> Result<Self, String> {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = base.join(format!(".tmp-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("could not create render staging dir {}: {e}", dir.display()))?;
        Ok(StageDir(dir))
    }
}
impl Drop for StageDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sanitize_stem(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if out.is_empty() { "render".into() } else { out }
}

fn find_focus_in_schematic_tree(root: &Path, refdes: &str, sheet_hint: Option<&str>) -> Option<(PathBuf, Bbox)> {
    let mut files = BTreeSet::new();
    collect_sch_files(root, 0, &mut files);
    let mut ordered: Vec<PathBuf> = files.into_iter().collect();
    if let Some(hint) = sheet_hint {
        let low = hint.to_ascii_lowercase();
        ordered.sort_by_key(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            let m = stem == low || name == low || stem.contains(&low);
            (!m, p.clone())
        });
    } else {
        ordered.sort_by_key(|p| (p != root, p.clone()));
    }
    for f in ordered {
        if let Ok(bytes) = std::fs::read(&f) {
            if let Some(bb) = sch_refdes_bbox(&bytes, refdes) {
                return Some((f, bb));
            }
        }
    }
    None
}

fn select_exported_svgs(
    svgs: &[PathBuf],
    root_stem: &str,
    sheet: Option<&str>,
    auto_sheet: Option<&str>,
    pages_filtered: bool,
) -> Result<(Vec<PathBuf>, Vec<String>), String> {
    let available: Vec<String> = svgs
        .iter()
        .map(|p| {
            let s = p.file_stem().and_then(|x| x.to_str()).unwrap_or("").to_string();
            s.strip_prefix(&format!("{root_stem}-"))
                .unwrap_or(&s)
                .to_string()
        })
        .collect();

    if svgs.is_empty() {
        return Err("kicad-cli sch export svg produced no SVG files".into());
    }

    if pages_filtered {
        return Ok((vec![svgs[0].clone()], available));
    }

    let wanted = sheet.or(auto_sheet);
    let Some(w) = wanted else {
        let root_svg = format!("{root_stem}.svg");
        let picked = svgs
            .iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(root_svg.as_str()))
            .cloned()
            .unwrap_or_else(|| svgs[0].clone());
        return Ok((vec![picked], available));
    };

    if w.eq_ignore_ascii_case("all") {
        let picked: Vec<PathBuf> = svgs.iter().take(MAX_SHEETS_ALL).cloned().collect();
        return Ok((picked, available));
    }

    if let Ok(page_num) = w.parse::<usize>() {
        if page_num >= 1 && page_num <= svgs.len() {
            if svgs.len() == 1 {
                return Ok((vec![svgs[0].clone()], available));
            }
            return Ok((vec![svgs[page_num - 1].clone()], available));
        }
    }

    let w_low = w.trim_end_matches(".kicad_sch").to_ascii_lowercase();
    let exact: Vec<PathBuf> = svgs
        .iter()
        .filter(|p| {
            let s = p.file_stem().and_then(|x| x.to_str()).unwrap_or("");
            let sub = s.strip_prefix(&format!("{root_stem}-")).unwrap_or(s);
            sub.eq_ignore_ascii_case(&w_low) || s.eq_ignore_ascii_case(&w_low)
        })
        .cloned()
        .collect();
    if !exact.is_empty() {
        return Ok((exact, available));
    }

    let partial: Vec<PathBuf> = svgs
        .iter()
        .filter(|p| {
            let s = p.file_stem().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            s.contains(&w_low)
        })
        .cloned()
        .collect();
    if !partial.is_empty() {
        return Ok((partial, available));
    }

    Err(format!(
        "sheet {w:?} did not match any exported schematic sheet (available: {})",
        available.join(", ")
    ))
}

async fn run_kicad_cli(cli: &str, args: &[String]) -> Result<(), String> {
    let out = run_local(Path::new(cli), args, false).await?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if msg.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            msg
        };
        return Err(format!(
            "kicad-cli exited with {}: {}",
            out.status.code().unwrap_or(-1),
            msg.chars().take(400).collect::<String>()
        ));
    }
    Ok(())
}

async fn render_schematic(
    cli: &str,
    path: &Path,
    params: &Params,
    out_dir: &Path,
    stage: &Path,
) -> Result<Value, String> {
    let (root_sch, auto_sheet) = resolve_root_schematic(path);
    let root_stem = root_sch
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("schematic");

    let mut effective_sheet = params.sheet.clone();
    let focus_bbox = match &params.focus {
        None => None,
        Some(Focus::Box(b)) => Some(b.expand(params.margin_mm.unwrap_or(0.0))),
        Some(Focus::Refdes(r)) => {
            let hint = effective_sheet.as_deref().or(auto_sheet.as_deref());
            let (matched_file, bb) = find_focus_in_schematic_tree(&root_sch, r, hint)
                .ok_or_else(|| format!("reference {r:?} was not found in {}", path.display()))?;
            if effective_sheet.is_none() && matched_file != root_sch {
                if let Some(fname) = matched_file.file_name().and_then(|n| n.to_str()) {
                    if let Ok(root_bytes) = std::fs::read(&root_sch) {
                        if let Some((sname, _)) = sch_subsheets(&root_bytes).into_iter().find(|(_, f)| {
                            Path::new(f).file_name().and_then(|n| n.to_str()) == Some(fname)
                        }) {
                            effective_sheet = Some(sname);
                        }
                    }
                    if effective_sheet.is_none() {
                        effective_sheet = matched_file
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(str::to_string);
                    }
                }
            }
            Some(bb.expand(params.effective_margin_for_refdes()))
        }
    };

    let svg_dir = stage.join("sch_svg");
    std::fs::create_dir_all(&svg_dir)
        .map_err(|e| format!("could not create {}: {e}", svg_dir.display()))?;
    let mut args = vec![
        "sch".into(),
        "export".into(),
        "svg".into(),
        "-o".into(),
        svg_dir.to_string_lossy().into_owned(),
    ];
    let pages_filtered = effective_sheet
        .as_deref()
        .is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()));
    if pages_filtered {
        if let Some(s) = effective_sheet.as_deref() {
            args.push("--pages".into());
            args.push(s.to_string());
        }
    }
    args.push(root_sch.to_string_lossy().into_owned());
    run_kicad_cli(cli, &args).await?;

    let mut svgs: Vec<PathBuf> = std::fs::read_dir(&svg_dir)
        .map_err(|e| format!("could not read {}: {e}", svg_dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
        })
        .collect();
    svgs.sort();

    let (selected, available) = select_exported_svgs(
        &svgs,
        root_stem,
        effective_sheet.as_deref(),
        auto_sheet.as_deref(),
        pages_filtered,
    )?;

    let mut paths = Vec::with_capacity(selected.len());
    let mut first_dims = (0u32, 0u32);
    let mut final_vb = None;
    for (idx, svg_path) in selected.iter().enumerate() {
        let svg_text = std::fs::read_to_string(svg_path)
            .map_err(|e| format!("could not read {}: {e}", svg_path.display()))?;
        let (rewritten, vb) = rewrite_svg_viewport(&svg_text, focus_bbox, false)?;
        let cropped_svg = stage.join(format!("crop_{idx}.svg"));
        std::fs::write(&cropped_svg, rewritten.as_bytes())
            .map_err(|e| format!("could not write {}: {e}", cropped_svg.display()))?;
        let (px_w, _) = target_pixel_size(vb, params.width, params.dpi);
        let stem = sanitize_stem(
            svg_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("schematic"),
        );
        let png_path = out_dir.join(format!("{stem}.png"));
        let dims = rasterize_svg_to_png(&cropped_svg, &png_path, px_w, vb).await?;
        if idx == 0 {
            first_dims = dims;
            final_vb = Some(vb);
        }
        paths.push(png_path.to_string_lossy().into_owned());
    }

    let mut meta = json!({
        "paths": paths,
        "width": first_dims.0,
        "height": first_dims.1,
        "kind": params.kind.as_str(),
        "cached": false,
    });
    if let Some(s) = effective_sheet.or(auto_sheet) {
        meta["sheet"] = json!(s);
    }
    if available.len() > 1 {
        meta["available_sheets"] = json!(available);
    }
    if focus_bbox.is_some() {
        if let Some(vb) = final_vb {
            meta["focus_bbox"] = vb.to_json();
        }
    }
    Ok(meta)
}

async fn render_pcb_2d(
    cli: &str,
    path: &Path,
    params: &Params,
    out_dir: &Path,
    stage: &Path,
) -> Result<Value, String> {
    let focus_bbox = match &params.focus {
        None => None,
        Some(Focus::Box(b)) => Some(b.expand(params.margin_mm.unwrap_or(0.0))),
        Some(Focus::Refdes(r)) => {
            let pcb_bytes = std::fs::read(path)
                .map_err(|e| format!("could not read {}: {e}", path.display()))?;
            let bb = pcb_refdes_bbox(&pcb_bytes, r)
                .ok_or_else(|| format!("footprint {r:?} was not found in {}", path.display()))?;
            Some(bb.expand(params.effective_margin_for_refdes()))
        }
    };

    let svg_path = stage.join("pcb.svg");
    let layers_csv = params.layers.join(",");
    let args = vec![
        "pcb".into(),
        "export".into(),
        "svg".into(),
        "--mode-single".into(),
        "--page-size-mode".into(),
        "2".into(),
        "--exclude-drawing-sheet".into(),
        "-l".into(),
        layers_csv,
        "-o".into(),
        svg_path.to_string_lossy().into_owned(),
        path.to_string_lossy().into_owned(),
    ];
    run_kicad_cli(cli, &args).await?;

    let svg_text = std::fs::read_to_string(&svg_path)
        .map_err(|e| format!("could not read {}: {e}", svg_path.display()))?;
    let (rewritten, vb) = rewrite_svg_viewport(&svg_text, focus_bbox, true)?;
    let cropped_svg = stage.join("pcb_crop.svg");
    std::fs::write(&cropped_svg, rewritten.as_bytes())
        .map_err(|e| format!("could not write {}: {e}", cropped_svg.display()))?;

    let (px_w, _) = target_pixel_size(vb, params.width, params.dpi);
    let stem = sanitize_stem(path.file_stem().and_then(|s| s.to_str()).unwrap_or("pcb"));
    let png_path = out_dir.join(format!("{stem}.pcb.png"));
    let (w, h) = rasterize_svg_to_png(&cropped_svg, &png_path, px_w, vb).await?;

    let mut meta = json!({
        "paths": [png_path.to_string_lossy()],
        "width": w,
        "height": h,
        "kind": params.kind.as_str(),
        "layers": params.layers,
        "cached": false,
    });
    if focus_bbox.is_some() {
        meta["focus_bbox"] = vb.to_json();
    }
    Ok(meta)
}

async fn render_pcb_3d(
    cli: &str,
    path: &Path,
    params: &Params,
    out_dir: &Path,
) -> Result<Value, String> {
    let w = params.width.unwrap_or(DEFAULT_3D_WIDTH).clamp(200, 2400);
    let h = (w * 3 / 4).max(200);
    let stem = sanitize_stem(path.file_stem().and_then(|s| s.to_str()).unwrap_or("pcb"));

    let mut paths = Vec::with_capacity(params.views.len());
    let mut first_dims = (w, h);
    for (idx, view) in params.views.iter().enumerate() {
        let png_path = out_dir.join(format!("{stem}.3d.{view}.png"));
        let part_path = unique_part_path(&png_path, "part.png");
        let mut args = vec![
            "pcb".into(),
            "render".into(),
            "--width".into(),
            w.to_string(),
            "--height".into(),
            h.to_string(),
            "--background".into(),
            "opaque".into(),
            "--quality".into(),
            "basic".into(),
            "--use-board-stackup-colors".into(),
        ];
        match view.as_str() {
            "top" => args.extend(["--side".into(), "top".into()]),
            "bottom" => args.extend(["--side".into(), "bottom".into()]),
            "iso" => args.extend([
                "--side".into(),
                "top".into(),
                "--rotate".into(),
                "-45,0,45".into(),
                "--perspective".into(),
            ]),
            "iso_bottom" => args.extend([
                "--side".into(),
                "bottom".into(),
                "--rotate".into(),
                "-45,0,45".into(),
                "--perspective".into(),
            ]),
            "front" => args.extend(["--side".into(), "front".into()]),
            "back" => args.extend(["--side".into(), "back".into()]),
            "left" => args.extend(["--side".into(), "left".into()]),
            "right" => args.extend(["--side".into(), "right".into()]),
            _ => unreachable!(),
        }
        args.extend([
            "-o".into(),
            part_path.to_string_lossy().into_owned(),
            path.to_string_lossy().into_owned(),
        ]);
        if let Err(e) = run_kicad_cli(cli, &args).await {
            let _ = std::fs::remove_file(&part_path);
            return Err(e);
        }
        let bytes = std::fs::read(&part_path)
            .map_err(|e| format!("kicad-cli pcb render wrote no image: {e}"))?;
        let dims = png_dimensions(&bytes)
            .ok_or_else(|| "kicad-cli pcb render produced an invalid PNG".to_string())?;
        crate::plat::fs::rename(&part_path, &png_path)
            .map_err(|e| format!("could not publish {}: {e}", png_path.display()))?;
        if idx == 0 {
            first_dims = dims;
        }
        paths.push(png_path.to_string_lossy().into_owned());
    }

    Ok(json!({
        "paths": paths,
        "width": first_dims.0,
        "height": first_dims.1,
        "kind": params.kind.as_str(),
        "views": params.views,
        "cached": false,
    }))
}

/// The model files viewer_render draws: what the hub's own mesher reads (STEP, GLB, STL), and SolidCraft's designs, which only
/// `solidcraft-cli` draws.
fn mech_ext(low: &str) -> bool {
    [".step", ".stp", ".glb", ".stl"].iter().any(|e| low.ends_with(e)) || crate::solidcraft::is_design(low)
}

/// A model's views from `solidcraft-cli snapshot` (its kernel's own faces and edges), for the files it reads when it is on this
/// machine. None: not tried, or it failed (said on stderr) and the hub's own mesher draws the file, where it can.
async fn render_mech_cli(path: &Path, params: &Params, out_dir: &Path) -> Option<Value> {
    let path_str = path.to_string_lossy();
    if !crate::solidcraft::cli_reads(&path_str) { return None }
    let cli = crate::solidcraft::runtime::cli_path()?;
    let w = params.width.unwrap_or(DEFAULT_3D_WIDTH).clamp(200, 2400);
    let h = (w * 3 / 4).max(200);
    let stem = sanitize_stem(path.file_stem().and_then(|s| s.to_str()).unwrap_or("model"));
    match crate::solidcraft::render::views(&cli, path, out_dir, &stem, &params.views, w, h).await {
        Ok(paths) => Some(json!({"paths": paths, "width": w, "height": h, "kind": Kind::Mech.as_str(), "views": params.views, "cached": false, "renderer": "solidcraft"})),
        Err(why) => { eprintln!("viewer_render: solidcraft-cli could not draw {path_str}: {why}"); None }
    }
}

async fn render_mech(
    home: &Path,
    path: &Path,
    params: &Params,
    out_dir: &Path,
) -> Result<Value, String> {
    if let Some(done) = render_mech_cli(path, params, out_dir).await { return Ok(done) }
    if crate::solidcraft::is_design(&path.to_string_lossy()) {
        return Err(format!("{} is a SolidCraft design: drawing it needs SolidCraft on this computer (Settings, CAD installs it)", path.display()));
    }
    let home = home.to_path_buf();
    let path_str = path.to_string_lossy().into_owned();
    let out_dir = out_dir.to_path_buf();
    let views = params.views.clone();
    let w = params.width.unwrap_or(DEFAULT_3D_WIDTH).clamp(200, 2400);
    let h = (w * 3 / 4).max(200);

    tokio::task::spawn_blocking(move || {
        let tools = super::stepglb::machine(&home);
        let ts = render::load_tris(&path_str, &super::stepglb::cache_dir(), &tools)?;
        let (lo, hi) = super::mesh::bounds(&ts);
        let cam = super::mesh::fit(lo, hi);
        let stem = sanitize_stem(
            Path::new(&path_str)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("model"),
        );

        let mut paths = Vec::with_capacity(views.len());
        for view in &views {
            let (yaw, pitch) = match view.as_str() {
                "iso" => (0.785398f32, 0.61548f32),
                "front" => (0.0, 0.0),
                "back" => (std::f32::consts::PI, 0.0),
                "top" => (0.0, 1.570796),
                "bottom" => (0.0, -1.570796),
                "right" => (1.570796, 0.0),
                "left" => (-1.570796, 0.0),
                _ => unreachable!(),
            };
            let rgb = super::mesh::render(w, h, &ts, &cam.face(yaw, pitch), super::mesh::GROUND);
            let png_bytes = super::png::rgb(&rgb, w, h);
            let png_path = out_dir.join(format!("{stem}.{view}.png"));
            let part_path = unique_part_path(&png_path, "part");
            std::fs::write(&part_path, &png_bytes)
                .map_err(|e| format!("could not write {}: {e}", part_path.display()))?;
            if let Err(e) = crate::plat::fs::rename(&part_path, &png_path) {
                let _ = std::fs::remove_file(&part_path);
                return Err(format!("could not publish {}: {e}", png_path.display()));
            }
            paths.push(png_path.to_string_lossy().into_owned());
        }

        Ok(json!({
            "paths": paths,
            "width": w,
            "height": h,
            "kind": Kind::Mech.as_str(),
            "views": views,
            "cached": false,
        }))
    })
    .await
    .map_err(|e| format!("mech render task failed: {e}"))?
}

fn read_cached_meta(meta_path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(meta_path).ok()?;
    let mut v: Value = serde_json::from_str(&text).ok()?;
    let arr = v.get("paths")?.as_array()?;
    if arr.is_empty() {
        return None;
    }
    for p in arr {
        let s = p.as_str()?;
        let meta = std::fs::metadata(s).ok()?;
        if !meta.is_file() || meta.len() == 0 {
            return None;
        }
    }
    v["cached"] = json!(true);
    Some(v)
}

pub fn build_mcp_content(mut meta: Value, inline: bool) -> (bool, Value) {
    let paths: Vec<String> = meta
        .get("paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_str().map(str::to_string))
        .collect();

    let mut image_blocks = Vec::new();
    let mut inline_count = 0usize;
    let mut omitted = 0usize;
    let mut total_bytes = 0usize;

    if inline {
        for p in &paths {
            if image_blocks.len() >= MAX_INLINE_IMAGES {
                omitted += 1;
                continue;
            }
            match std::fs::read(p) {
                Ok(bytes) => {
                    if bytes.len() > MAX_SINGLE_INLINE_BYTES
                        || total_bytes + bytes.len() > MAX_INLINE_BYTES
                    {
                        omitted += 1;
                        continue;
                    }
                    total_bytes += bytes.len();
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    image_blocks.push(json!({
                        "type": "image",
                        "data": b64,
                        "mimeType": "image/png",
                    }));
                    inline_count += 1;
                }
                Err(_) => {
                    omitted += 1;
                }
            }
        }
    } else {
        omitted = paths.len();
    }

    if let Some(o) = meta.as_object_mut() {
        o.insert("images".into(), json!(inline_count));
        if omitted > 0 && inline {
            o.insert("inline_omitted".into(), json!(omitted));
        }
    }

    let mut blocks = vec![json!({
        "type": "text",
        "text": meta.to_string(),
    })];
    blocks.extend(image_blocks);
    (true, json!({ "$content": blocks }))
}

/// Execute `viewer_render` for the given resolved path and JSON arguments.
pub async fn call(hub: &Arc<Hub>, path: &str, a: &Value) -> (bool, Value) {
    match render_inner(hub, path, a).await {
        Ok((meta, inline)) => build_mcp_content(meta, inline),
        Err(e) => (false, json!({ "error": e })),
    }
}

/// Core render helper returning `(metadata_json, inline_flag)`.
pub async fn render_inner(hub: &Arc<Hub>, path: &str, a: &Value) -> Result<(Value, bool), String> {
    if path.trim().is_empty() {
        return Err("path is required".into());
    }
    if !crate::plat::fs::is_abs(path) {
        return Err(format!("path must be absolute: {path}"));
    }
    let src = Path::new(path);
    if !src.is_file() {
        return Err(format!("file does not exist: {path}"));
    }

    let mut params = Params::parse(path, a)?;
    let cli_env = if matches!(params.kind, Kind::Schematic | Kind::Pcb | Kind::Pcb3d) {
        let rt = super::ipc::runtime(hub).await;
        if rt.cli.is_empty() {
            return Err("KiCad CLI (kicad-cli) is not installed on this machine".into());
        }
        Some(rt)
    } else {
        None
    };

    let hash_src = if params.kind == Kind::Schematic {
        let (root_sch, auto_sheet) = resolve_root_schematic(src);
        if params.sheet.is_none() {
            params.sheet = auto_sheet;
        }
        root_sch
    } else {
        src.to_path_buf()
    };
    let chash = content_hash_for(&hash_src, params.kind)?;
    let cli_ver = cli_env.as_ref().map(|r| r.version.as_str()).unwrap_or("");
    let key = cache_key(&chash, cli_ver, &params);

    let cache_root = hub.cfg.home.join("cache").join("viewer-render");
    let out_dir = cache_root.join(&key);
    let meta_path = out_dir.join("meta.json");

    if let Some(cached) = read_cached_meta(&meta_path) {
        return Ok((cached, params.inline));
    }

    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("could not create cache dir {}: {e}", out_dir.display()))?;

    let meta = match params.kind {
        Kind::Schematic => {
            let rt = cli_env.as_ref().unwrap();
            let stage = StageDir::new(&cache_root)?;
            render_schematic(&rt.cli, &hash_src, &params, &out_dir, &stage.0).await?
        }
        Kind::Pcb => {
            let rt = cli_env.as_ref().unwrap();
            let stage = StageDir::new(&cache_root)?;
            render_pcb_2d(&rt.cli, src, &params, &out_dir, &stage.0).await?
        }
        Kind::Pcb3d => {
            let rt = cli_env.as_ref().unwrap();
            render_pcb_3d(&rt.cli, src, &params, &out_dir).await?
        }
        Kind::Mech => render_mech(&hub.cfg.home, src, &params, &out_dir).await?,
    };

    let meta_part = unique_part_path(&meta_path, "part");
    if std::fs::write(&meta_part, meta.to_string()).is_ok() {
        let _ = crate::plat::fs::rename(&meta_part, &meta_path);
    }

    Ok((meta, params.inline))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_infer_kind_and_validate_options() {
        let p = Params::parse("/a/b.kicad_sch", &json!({})).unwrap();
        assert_eq!(p.kind, Kind::Schematic);
        assert!(p.inline);

        let p = Params::parse("/a/b.kicad_pcb", &json!({})).unwrap();
        assert_eq!(p.kind, Kind::Pcb);
        assert_eq!(p.layers, ["F.Cu", "B.Cu", "F.Mask", "F.SilkS", "Edge.Cuts"]);

        let p = Params::parse("/a/b.kicad_pcb", &json!({"kind": "board", "layers": ["F.Cu", "Edge.Cuts"]})).unwrap();
        assert_eq!(p.kind, Kind::Pcb);
        assert_eq!(p.layers, ["F.Cu", "Edge.Cuts"]);

        let p = Params::parse("/a/b.kicad_pcb", &json!({"kind": "3d"})).unwrap();
        assert_eq!(p.kind, Kind::Pcb3d);
        assert_eq!(p.views, ["top", "bottom", "iso"]);

        let p = Params::parse("/a/part.STEP", &json!({"views": ["top", "iso"]})).unwrap();
        assert_eq!(p.kind, Kind::Mech);
        assert_eq!(p.views, ["top", "iso"]);

        let p = Params::parse("/a/part.stl", &json!({"width": 9999})).unwrap();
        assert_eq!(p.kind, Kind::Mech);
        assert_eq!(p.width, Some(2400));

        let p = Params::parse(
            "/a/b.kicad_pcb",
            &json!({"focus": {"x_mm": 10, "y_mm": 20.5, "w_mm": 8, "h_mm": 6}, "margin_mm": 2.5, "dpi": 600, "inline": false}),
        )
        .unwrap();
        assert_eq!(
            p.focus,
            Some(Focus::Box(Bbox {
                x_mm: 10.0,
                y_mm: 20.5,
                w_mm: 8.0,
                h_mm: 6.0
            }))
        );
        assert_eq!(p.margin_mm, Some(2.5));
        assert_eq!(p.dpi, Some(600.0));
        assert!(!p.inline);

        assert!(Params::parse("/a/b.txt", &json!({})).is_err());
        assert!(Params::parse("/a/b.kicad_sch", &json!({"kind": "3d"})).is_err());
        assert!(Params::parse("/a/b.kicad_pcb", &json!({"kind": "3d", "focus": "U1"})).is_err());
        assert!(Params::parse("/a/b.kicad_pcb", &json!({"kind": "3d", "views": ["diagonal"]})).is_err());
        assert!(Params::parse("/a/b.kicad_pcb", &json!({"focus": {"x_mm": 1, "y_mm": 2, "w_mm": 0, "h_mm": 3}})).is_err());
        assert_eq!(Params::parse("/a/b.kicad_sch", &json!({"sheet": 2})).unwrap().sheet.as_deref(), Some("2"));
        assert!(Params::parse("/a/b.kicad_sch", &json!({"sheet": true})).is_err());
        assert!(Params::parse("/a/b.kicad_sch", &json!({"inline": "yes"})).is_err());
    }

    #[test]
    fn cache_key_changes_with_content_or_params() {
        let base = Params::parse("/a/b.kicad_pcb", &json!({})).unwrap();
        let k0 = cache_key("abc123", "10.99.0", &base);
        assert_eq!(k0.len(), 24);
        assert_eq!(k0, cache_key("abc123", "10.99.0", &base));
        assert_ne!(k0, cache_key("def456", "10.99.0", &base));
        assert_ne!(k0, cache_key("abc123", "10.99.1", &base));

        let p_layers = Params::parse("/a/b.kicad_pcb", &json!({"layers": ["F.Cu"]})).unwrap();
        assert_ne!(k0, cache_key("abc123", "10.99.0", &p_layers));

        let p_focus = Params::parse("/a/b.kicad_pcb", &json!({"focus": "U1"})).unwrap();
        assert_ne!(k0, cache_key("abc123", "10.99.0", &p_focus));

        let p_margin = Params::parse("/a/b.kicad_pcb", &json!({"focus": "U1", "margin_mm": 12.0})).unwrap();
        assert_ne!(cache_key("abc123", "10.99.0", &p_focus), cache_key("abc123", "10.99.0", &p_margin));

        let p_width = Params::parse("/a/b.kicad_pcb", &json!({"width": 800})).unwrap();
        assert_ne!(k0, cache_key("abc123", "10.99.0", &p_width));
    }

    #[test]
    fn pcb_and_sch_refdes_bbox_extract_from_real_and_10_99_fixtures() {
        let pcb = include_bytes!("../../../../test/files/fixtures/plot/ecc83-pp.kicad_pcb");
        let u1 = pcb_refdes_bbox(pcb, "U1").expect("U1 in ecc83-pp.kicad_pcb");
        assert!(u1.w_mm > 5.0 && u1.h_mm > 5.0, "{u1:?}");
        assert!(pcb_refdes_bbox(pcb, "u1").is_some());
        assert!(pcb_refdes_bbox(pcb, "NONEXISTENT_REF").is_none());

        // KiCad 10.99 transform syntax
        let k10 = br#"(kicad_pcb (footprint "R_0603" (layer "F.Cu") (transform (translate 50 30) (rotate 0)) (property "Reference" "R42") (pad "1" smd rect (at -0.8 0) (size 0.8 0.9) (layers "F.Cu")) (pad "2" smd rect (at 0.8 0) (size 0.8 0.9) (layers "F.Cu"))))"#;
        let r42 = pcb_refdes_bbox(k10, "R42").expect("R42 bbox");
        assert!((r42.x_mm - 48.8).abs() < 0.05 && (r42.w_mm - 2.4).abs() < 0.05, "{r42:?}");

        let sch = include_bytes!("../../../../test/files/fixtures/plot/ecc83-pp.kicad_sch");
        let r1 = sch_refdes_bbox(sch, "R1").expect("R1 in ecc83-pp.kicad_sch");
        assert!(r1.w_mm > 1.0 && r1.h_mm > 1.0, "{r1:?}");
        // Multi-unit symbol U1 (units U1A, U1B, U1C)
        let u1_sch = sch_refdes_bbox(sch, "U1").expect("U1 multi-unit in ecc83-pp.kicad_sch");
        let u1a_sch = sch_refdes_bbox(sch, "U1A").expect("U1A unit in ecc83-pp.kicad_sch");
        assert!(u1_sch.w_mm >= u1a_sch.w_mm, "{u1_sch:?} vs {u1a_sch:?}");

        let hier = include_bytes!("../../../../test/files/fixtures/plot/complex_hierarchy.kicad_sch");
        let subs = sch_subsheets(hier);
        assert!(subs.iter().any(|(n, f)| n == "ampli_ht_vertical" && f == "ampli_ht.kicad_sch"), "{subs:?}");
    }

    #[test]
    fn rewrite_svg_viewport_updates_viewbox_and_injects_dark_bg_for_pcb() {
        let svg = r#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="297mm" height="210mm" viewBox="0 0 297 210"><path d="M 10 10 L 20 20"/></svg>"#;
        let (out, vb) = rewrite_svg_viewport(
            svg,
            Some(Bbox {
                x_mm: 100.0,
                y_mm: 50.0,
                w_mm: 20.0,
                h_mm: 15.0,
            }),
            true,
        )
        .unwrap();
        assert_eq!(vb.w_mm, 20.0);
        assert!(out.contains(r#"width="20.0000mm" height="15.0000mm" viewBox="100.0000 50.0000 20.0000 15.0000""#), "{out}");
        assert!(out.contains(r##"<rect x="100.0000" y="50.0000" width="20.0000" height="15.0000" fill="#14161a"/>"##), "{out}");

        let (sch_out, sch_vb) = rewrite_svg_viewport(svg, None, false).unwrap();
        assert_eq!(sch_vb.w_mm, 297.0);
        assert!(!sch_out.contains("#14161a"));
    }

    #[test]
    fn stl_tris_parses_binary_and_ascii_and_png_dimensions_reads_ihdr() {
        // ASCII STL
        let ascii = b"solid box\n facet normal 0 0 1\n outer loop\n vertex 0 0 0\n vertex 10 0 0\n vertex 0 10 0\n endloop\n endfacet\nendsolid box\n";
        let ts = render::stl_tris(ascii).unwrap();
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0].b, [10.0, 0.0, 0.0]);

        // Binary STL (1 triangle)
        let mut bin = vec![0u8; 80];
        bin.extend_from_slice(&1u32.to_le_bytes());
        bin.extend_from_slice(&[0u8; 12]); // normal
        for f in [0.0f32, 0.0, 0.0, 5.0, 0.0, 0.0, 0.0, 5.0, 0.0] {
            bin.extend_from_slice(&f.to_le_bytes());
        }
        bin.extend_from_slice(&[0u8; 2]);
        let bts = render::stl_tris(&bin).unwrap();
        assert_eq!(bts.len(), 1);
        assert_eq!(bts[0].c, [0.0, 5.0, 0.0]);

        let p = super::super::png::rgb(&[0u8; 4 * 3 * 3], 4, 3);
        assert_eq!(png_dimensions(&p), Some((4, 3)));
        assert_eq!(png_dimensions(b"not a png"), None);

        // Non-ASCII UTF-8 comment line should not panic; malformed vertex should error.
        let utf8_stl = b"solid \xc3\xa9\xc3\xa9\n facet normal 0 0 1\n outer loop\n vertex 0 0 0\n vertex 1 0 0\n vertex 0 1 0\n endloop\n endfacet\nendsolid\n";
        assert_eq!(render::stl_tris(utf8_stl).unwrap().len(), 1);
        let bad_vertex = b"solid x\n vertex 0 0 bad\n vertex 1 0 0\n vertex 0 1 0\nendsolid\n";
        assert!(render::stl_tris(bad_vertex).is_err());
    }

    #[test]
    fn select_exported_svgs_handles_pages_filtered_and_standalone_sch() {
        let svgs = vec![PathBuf::from("/tmp/root-sub.svg")];
        let (picked, _) = select_exported_svgs(&svgs, "root", Some("2"), None, true).unwrap();
        assert_eq!(picked, svgs);

        let d = crate::plat::tmp::dir("vr-standalone");
        std::fs::write(d.join("board.kicad_pro"), "{}").unwrap();
        std::fs::write(d.join("board.kicad_sch"), "(kicad_sch)").unwrap();
        std::fs::write(d.join("alt.kicad_sch"), "(kicad_sch)").unwrap();
        let (resolved, auto_s) = resolve_root_schematic(&d.join("alt.kicad_sch"));
        assert_eq!(resolved, d.join("alt.kicad_sch"));
        assert!(auto_s.is_none());
    }

    #[test]
    fn target_pixel_size_respects_width_dpi_and_height_cap() {
        let a4 = Bbox { x_mm: 0.0, y_mm: 0.0, w_mm: 297.0, h_mm: 210.0 };
        assert_eq!(target_pixel_size(a4, None, None), (1600, 1131));
        assert_eq!(target_pixel_size(a4, Some(1000), None), (1000, 707));
        let (dw, dh) = target_pixel_size(a4, None, Some(150.0));
        assert!((1750..=1760).contains(&dw) && dh > 1200, "{dw}x{dh}");
        let tall = Bbox { x_mm: 0.0, y_mm: 0.0, w_mm: 10.0, h_mm: 100.0 };
        let (tw, th) = target_pixel_size(tall, None, None);
        assert_eq!(th, 2400);
        assert_eq!(tw, 240);
    }
}
