//! Layers to a GLB: one mesh per layer (`pcb3d_<layer>`), glTF 2.0 binary, metres, y up.
//!
//! Board (x, y, z) goes to glTF (x, z, y) / 1000, which glb::tris and solid::of read back as board coordinates. That map swaps two
//! axes, which mirrors, so each triangle's last two corners swap too and its front face stays outward.
//! A primitive holds at most 65535 vertices (16-bit indices): a bigger layer is several primitives of one mesh.
use super::Layer;
use super::super::glb::Tri3;
use std::fmt::Write;

const MAX_VERT: usize = 65535;

fn gl(p: [f32; 3]) -> [f32; 3] { [p[0] / 1000.0, p[2] / 1000.0, p[1] / 1000.0] }

/// The layers' triangles in glTF convention (metres, y up), outward-facing, coloured by their layer.
pub fn tris(layers: &[Layer]) -> Vec<Tri3> {
    let mut out = Vec::with_capacity(layers.iter().map(|l| l.tris()).sum());
    for l in layers {
        for t in l.idx.chunks_exact(3) {
            out.push(Tri3 { a: gl(l.pos[t[0] as usize]), b: gl(l.pos[t[2] as usize]), c: gl(l.pos[t[1] as usize]), color: l.color });
        }
    }
    out
}

/// One primitive: its vertices (glTF convention) and 16-bit indices.
struct Prim { pos: Vec<[f32; 3]>, idx: Vec<u16> }

fn split(l: &Layer) -> Vec<Prim> {
    let mut prims = vec![];
    let mut cur = Prim { pos: vec![], idx: vec![] };
    // local[v] is v's index in the current primitive, valid when stamp[v] == the primitive's number
    let mut local = vec![0u16; l.pos.len()];
    let mut stamp = vec![u32::MAX; l.pos.len()];
    let mut n = 0u32;
    for t in l.idx.chunks_exact(3) {
        let fresh = t.iter().filter(|&&v| stamp[v as usize] != n).count();
        if cur.pos.len() + fresh > MAX_VERT {
            prims.push(std::mem::replace(&mut cur, Prim { pos: vec![], idx: vec![] }));
            n += 1;
        }
        // the corners swap (b and c) for the mirrored axes
        for &v in &[t[0], t[2], t[1]] {
            let vi = v as usize;
            if stamp[vi] != n { stamp[vi] = n; local[vi] = cur.pos.len() as u16; cur.pos.push(gl(l.pos[vi])) }
            cur.idx.push(local[vi]);
        }
    }
    if !cur.idx.is_empty() { prims.push(cur) }
    prims
}

fn pad4(b: &mut Vec<u8>) { while b.len() % 4 != 0 { b.push(0) } }

/// The GLB of the layers (empty layers are left out). `name` goes in the asset's extras.
pub fn glb(layers: &[Layer], name: &str) -> Vec<u8> {
    let mut bin: Vec<u8> = vec![];
    let (mut views, mut accs, mut meshes, mut mats, mut nodes) = (String::new(), String::new(), String::new(), String::new(), String::new());
    let (mut nv, mut na, mut nm) = (0usize, 0usize, 0usize);
    for l in layers.iter().filter(|l| !l.idx.is_empty()) {
        let (r, g, b, a) = ((l.color >> 16 & 255) as f32 / 255.0, (l.color >> 8 & 255) as f32 / 255.0, (l.color & 255) as f32 / 255.0, (255 - (l.color >> 24)) as f32 / 255.0);
        let blend = if a < 1.0 { ",\"alphaMode\":\"BLEND\"" } else { "" };
        if nm > 0 { mats.push(',') }
        let _ = write!(mats, "{{\"name\":\"{}\",\"pbrMetallicRoughness\":{{\"baseColorFactor\":[{},{},{},{}],\"metallicFactor\":0,\"roughnessFactor\":0.8}}{},\"doubleSided\":false}}", l.name, r, g, b, a, blend);
        let mut prims = String::new();
        for (k, p) in split(l).iter().enumerate() {
            pad4(&mut bin);
            let (po, pl) = (bin.len(), p.pos.len() * 12);
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for v in &p.pos { for i in 0..3 { lo[i] = lo[i].min(v[i]); hi[i] = hi[i].max(v[i]); bin.extend(v[i].to_le_bytes()) } }
            pad4(&mut bin);
            let (io, il) = (bin.len(), p.idx.len() * 2);
            for i in &p.idx { bin.extend(i.to_le_bytes()) }
            let _ = write!(views, "{}{{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{},\"target\":34962}},{{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{},\"target\":34963}}", if nv > 0 { "," } else { "" }, po, pl, io, il);
            let _ = write!(accs, "{}{{\"bufferView\":{},\"componentType\":5126,\"count\":{},\"type\":\"VEC3\",\"min\":[{},{},{}],\"max\":[{},{},{}]}},{{\"bufferView\":{},\"componentType\":5123,\"count\":{},\"type\":\"SCALAR\"}}",
                if na > 0 { "," } else { "" }, nv, p.pos.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], nv + 1, p.idx.len());
            let _ = write!(prims, "{}{{\"attributes\":{{\"POSITION\":{}}},\"indices\":{},\"material\":{},\"mode\":4}}", if k > 0 { "," } else { "" }, na, na + 1, nm);
            nv += 2; na += 2;
        }
        if nm > 0 { meshes.push(','); nodes.push(',') }
        let _ = write!(meshes, "{{\"name\":\"pcb3d_{}\",\"primitives\":[{}]}}", l.name, prims);
        let _ = write!(nodes, "{{\"name\":\"pcb3d_{}\",\"mesh\":{}}}", l.name, nm);
        nm += 1;
    }
    pad4(&mut bin);
    let scene: Vec<String> = (0..nm).map(|i| i.to_string()).collect();
    let esc: String = name.chars().flat_map(|c| match c { '"' | '\\' => vec!['\\', c], c if (c as u32) < 32 => vec![' '], c => vec![c] }).collect();
    let mut json = format!("{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"Backplane pcb3d\",\"extras\":{{\"name\":\"{}\"}}}},\"scene\":0,\"scenes\":[{{\"nodes\":[{}]}}],\"nodes\":[{}],\"meshes\":[{}],\"materials\":[{}],\"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"byteLength\":{}}}]}}",
        esc, scene.join(","), nodes, meshes, mats, accs, views, bin.len());
    while json.len() % 4 != 0 { json.push(' ') }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend(0x46546C67u32.to_le_bytes());
    out.extend(2u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((json.len() as u32).to_le_bytes());
    out.extend(0x4E4F534Au32.to_le_bytes());
    out.extend(json.as_bytes());
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(0x004E4942u32.to_le_bytes());
    out.extend(&bin);
    out
}
