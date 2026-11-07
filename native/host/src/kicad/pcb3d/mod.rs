//! A board's 3D model straight from its .kicad_pcb, no kicad-cli: the file is parsed (parse.rs) into the board's shapes in board
//! coordinates, each layer is meshed on its own core (mesh.rs, polygon booleans by i_overlay, rayon across layers), and the result is
//! both a GLB (glbw.rs: one mesh per layer, 16-bit indices where they fit, cached) and the `solid` frame the clients already draw. The
//! parts come after, lazily (models.rs resolves and converts each unique 3D model once, cached by content; stream.rs sends each model
//! as it is ready with the transforms of its instances, and the clients fade them in).
//!
//! Units and axes, everywhere in this module: millimetres, board axes as the file has them (x right, y DOWN), z up from the laminate's
//! bottom face. Angles in degrees as KiCad writes them (counter-clockwise on screen, so with y down a positive angle turns x towards -y).
pub mod parse;
pub mod geom;
pub mod mesh;
pub mod glbw;
pub mod models;
pub mod stream;

/// A point on the board, mm, x right, y down.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct P2 { pub x: f64, pub y: f64 }
impl P2 { pub const fn new(x: f64, y: f64) -> P2 { P2 { x, y } } }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side { Top, Bottom }

/// A drawn or copper shape in board coordinates (a footprint's own shapes are already placed: rotated, flipped and moved).
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// a stroke from a to b, round ends, width w
    Seg { a: P2, b: P2, w: f64 },
    /// a stroke along the circle through a, m, b from a to b, round ends, width w
    Arc { a: P2, m: P2, b: P2, w: f64 },
    /// a circle: filled (and its edge stroked by w), or only its edge stroked by w
    Circle { c: P2, r: f64, w: f64, fill: bool },
    /// a closed polygon: filled (and its edges stroked by w), or only its edges stroked by w
    Poly { pts: Vec<P2>, w: f64, fill: bool },
    /// a rectangle of size w x h centred at c, turned by `rot` degrees, corners rounded by radius `r` (0: sharp; min(w,h)/2: an oval)
    /// and the corners in `chamfer_corners` (bits: 1 top-left, 2 top-right, 4 bottom-left, 8 bottom-right, before turning) cut by `chamfer` mm
    Rect { c: P2, w: f64, h: f64, rot: f64, r: f64, chamfer: f64, chamfer_corners: u8 },
}

/// A hole through the board: round when w == h, else a slot (an oval w x h turned by rot degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drill { pub c: P2, pub w: f64, pub h: f64, pub rot: f64, pub plated: bool }

/// Which outer layers a pad is on.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PadLayers { pub cu_top: bool, pub cu_bottom: bool, pub mask_top: bool, pub mask_bottom: bool }

#[derive(Clone, Debug, PartialEq)]
pub struct Pad {
    /// the copper's outline: the union of these (a custom pad is its anchor and its primitives)
    pub shapes: Vec<Shape>,
    pub layers: PadLayers,
    pub drill: Option<Drill>,
    /// how far the mask opening reaches beyond the copper, mm (the pad's own margin, else its footprint's, else the board's; may be negative)
    pub mask_margin: f64,
    /// the footprint's reference ("" for none), for a picked pad
    pub reference: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Via {
    pub c: P2,
    /// pad diameter and drill, mm
    pub d: f64,
    pub drill: f64,
    /// covered by the mask on that side (KiCad's default for a via; `(tenting ...)` and the board setting can open it)
    pub tented_top: bool,
    pub tented_bottom: bool,
}

/// One 3D model reference of a footprint, as the file writes it (the path unexpanded: `${KICAD10_3DMODEL_DIR}/...`, `${KIPRJMOD}/...`, relative).
#[derive(Clone, Debug, PartialEq)]
pub struct ModelRef {
    pub path: String,
    /// mm (a legacy `(at (xyz ...))` in inches is converted)
    pub offset: [f64; 3],
    pub scale: [f64; 3],
    /// degrees about x, y, z
    pub rotate: [f64; 3],
    pub hide: bool,
    /// 0..1
    pub opacity: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Footprint {
    pub reference: String,
    pub value: String,
    /// the library name (`Resistor_SMD:R_0402_1005Metric`)
    pub name: String,
    pub at: P2,
    /// degrees, as the file has it
    pub rot: f64,
    /// Bottom: the footprint is flipped onto the back
    pub side: Side,
    pub models: Vec<ModelRef>,
    /// courtyard shapes (F.CrtYd / B.CrtYd), board coordinates: a part with no model is a box over these
    pub courtyard: Vec<Shape>,
    /// fab outline shapes (F.Fab / B.Fab), board coordinates: the box's fallback when there is no courtyard
    pub fab: Vec<Shape>,
    /// attributes: (attr ... dnp), (attr ... exclude_from_bom), board_only
    pub dnp: bool,
}

/// The board's surface finish, from the stackup's `copper_finish`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Finish { #[default] None, Enig, Enepig, Hasl, HaslLeadFree, ImmersionGold, ImmersionSilver, ImmersionTin, Osp, HardGold }

/// A colour as KiCad's 3D view shows it: display (sRGB) numbers 0..1 and an opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba { pub r: f32, pub g: f32, pub b: f32, pub a: f32 }

#[derive(Clone, Debug, PartialEq)]
pub struct Stackup {
    /// the whole board, mm (setup's stackup total, else general (thickness), else 1.6)
    pub thickness: f64,
    /// an outer copper layer's thickness, mm (0.035 when not said)
    pub copper: f64,
    /// None when the stackup says nothing: mesh.rs then uses KiCad's defaults
    pub mask_top: Option<Rgba>,
    pub mask_bottom: Option<Rgba>,
    pub silk_top: Option<Rgba>,
    pub silk_bottom: Option<Rgba>,
    pub substrate: Option<Rgba>,
    pub finish: Finish,
    /// the board's solder mask expansion (setup's pad_to_mask_clearance), mm
    pub mask_margin: f64,
}
impl Default for Stackup {
    fn default() -> Self { Stackup { thickness: 1.6, copper: 0.035, mask_top: None, mask_bottom: None, silk_top: None, silk_bottom: None, substrate: None, finish: Finish::None, mask_margin: 0.0 } }
}

/// Everything of a board its 3D model is made of, board coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    /// Edge.Cuts: the board's and every footprint's edge shapes. Strokes of width 0 or w; their joined loops are the outline and its cutouts.
    pub outline: Vec<Shape>,
    pub stack: Stackup,
    /// copper on the outer layers that is not a pad or a via: tracks, arcs, graphics and text strokes on F.Cu / B.Cu
    pub copper: Vec<(Side, Shape)>,
    /// zones' filled polygons on F.Cu / B.Cu, each as KiCad stores it (one ring, holes joined to it by zero-width cuts)
    pub zones: Vec<(Side, Vec<P2>)>,
    pub pads: Vec<Pad>,
    pub vias: Vec<Via>,
    /// holes that are not a pad's: none today (KiCad keeps every hole in a pad); kept for NPTH-only footprints' drills if a format has them
    pub holes: Vec<Drill>,
    /// F.SilkS / B.SilkS: graphics and text strokes
    pub silk: Vec<(Side, Shape)>,
    /// shapes drawn on F.Mask / B.Mask: extra openings in the mask
    pub mask: Vec<(Side, Shape)>,
    pub footprints: Vec<Footprint>,
}

/// One layer of the board's model: a material's triangles, mm, board axes (x right, y down, z up).
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// the layer's role: "laminate", "hole_walls", "copper_top", "copper_bottom", "finish_top", "finish_bottom", "mask_top",
    /// "mask_bottom", "silk_top", "silk_bottom"
    pub name: &'static str,
    /// as the wire has it: linear light 0xRRGGBB, and (255 - opacity) in the top byte (0 is opaque)
    pub color: u32,
    pub pos: Vec<[f32; 3]>,
    pub idx: Vec<u32>,
}
impl Layer { pub fn tris(&self) -> usize { self.idx.len() / 3 } }
