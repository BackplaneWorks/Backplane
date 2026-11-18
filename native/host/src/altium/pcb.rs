//! .PcbDoc → board items (stand-in until the reader lands: draws nothing).
use crate::files::kicad::board::Item;

pub struct Comp { pub refdes: String, pub footprint: String, pub x_mm: f32, pub y_mm: f32, pub rot: f32, pub bottom: bool, pub value: String }
pub fn items(bytes: &[u8]) -> Option<Vec<Item>> { if super::is_cfb(bytes) { Some(vec![]) } else { None } }
pub fn thickness_um(_bytes: &[u8]) -> u32 { 1600 }
pub fn components(_bytes: &[u8]) -> Vec<Comp> { vec![] }
pub fn nets(_bytes: &[u8]) -> Vec<String> { vec![] }
pub fn layers(_bytes: &[u8]) -> Vec<(u32, String)> { vec![] }
