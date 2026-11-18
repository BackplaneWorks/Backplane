//! .SchDoc → schematic items (stand-in until the reader lands: draws nothing).
use crate::files::kicad::board::Item;

pub struct Part { pub refdes: String, pub value: String, pub footprint: String, pub lib_ref: String, pub description: String, pub fields: Vec<(String, String)> }
pub fn items(bytes: &[u8]) -> Option<Vec<Item>> { if super::is_cfb(bytes) { Some(vec![]) } else { None } }
pub fn sheets(_bytes: &[u8]) -> Vec<(String, String)> { vec![] }
pub fn sheet_boxes(_bytes: &[u8]) -> Vec<(f32, f32, f32, f32)> { vec![] }
pub fn parts(_bytes: &[u8]) -> Vec<Part> { vec![] }
pub fn nets(_bytes: &[u8]) -> Vec<String> { vec![] }
