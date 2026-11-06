//! core/glb in Rust: a binary glTF file to world-space triangles, for the phones' 3D view and for renders. A GLB is a 12-byte
//! header, a JSON chunk and a BIN chunk; scenes lead to nodes (matrix or TRS, children), nodes to meshes, meshes to primitives (mode 4
//! only), primitives to POSITION and indices, a primitive's colour to its material's baseColorFactor. Every F32 step is one
//! in the same order, so the triangles are the same bits.
//!
//! reader's edges are kept, because a malformed file must read the same: JSON is the lenient json (commas and colons are
//! not checked, the first of a duplicate key wins), a table index wraps at the table's power of two (Tab.go reads only d bits), an
//! aligned accessor stops at the end of the data and an unaligned one reads zeros past it.
//!
//! Deliberate differences, both limits where reader runs out of memory instead: `MOST_TRIS` triangles and `MOST_DEPTH` levels
//! of JSON. Past either the file answers `TooBig` (child takes about 300 bytes of memory per byte of a GLB, so a file this
//! size never finished there).
use super::fl;

/// Colours are 0xTTRRGGBB: TT is transparency (0 opaque, 255 clear), so an opaque colour is the plain 0xRRGGBB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tri3 { pub a: [f32; 3], pub b: [f32; 3], pub c: [f32; 3], pub color: u32 }

/// More triangles than any model reader could hold: 40 bytes each, 320 MB at most.
pub const MOST_TRIS: usize = 8 << 20;
const MOST_DEPTH: usize = 4096;
const MOST_WALK: usize = 4 << 20;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooBig;

// JSON ----------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum J { Null, Flag(bool), Num(String), Str(String), Arr(Vec<J>), Obj(Vec<(String, J)>) }
static NULL: J = J::Null;

enum Tok { Open(bool), Close, Colon, Comma, Str(String), Num(String), Word(String), Bad }

fn lex(s: &str) -> Vec<Tok> {
    #[derive(PartialEq)]
    enum St { Idle, Str, Esc, Uni(u32, u32), Atom(bool), Bad }
    let mut out = vec![]; let mut st = St::Idle; let mut acc = String::new();
    fn atom(acc: &mut String, num: bool, out: &mut Vec<Tok>) { let t = std::mem::take(acc); out.push(if num { Tok::Num(t) } else { Tok::Word(t) }) }
    for c in s.chars() {
        // a finished atom's character is lexed from idle (Lex.atom.step)
        if let St::Atom(num) = st {
            if matches!(c, '[' | '{' | ']' | '}' | '"' | ':' | ',') || (c as u32) <= 32 { atom(&mut acc, num, &mut out); st = St::Idle } else { acc.push(c); continue }
        }
        match st {
            St::Bad => break,
            St::Idle => match c {
                '[' => out.push(Tok::Open(false)), '{' => out.push(Tok::Open(true)), ']' | '}' => out.push(Tok::Close),
                '"' => { acc.clear(); st = St::Str } ':' => out.push(Tok::Colon), ',' => out.push(Tok::Comma),
                c if (c as u32) <= 32 => {}
                c => { acc.clear(); acc.push(c); st = St::Atom(c == '-' || c.is_ascii_digit()) }
            },
            St::Str => match c { '"' => { out.push(Tok::Str(std::mem::take(&mut acc))); st = St::Idle } '\\' => st = St::Esc, c => acc.push(c) },
            St::Esc => { st = St::Str; match c { 'u' => st = St::Uni(4, 0), 'n' => acc.push('\n'), 't' => acc.push('\t'), 'r' => acc.push('\r'), 'b' => acc.push('\u{8}'), 'f' => acc.push('\u{c}'), c => acc.push(c) } }
            St::Uni(left, code) => match c.to_digit(16) {
                None => st = St::Bad,
                Some(d) => { let code = (code << 4) | d; if left == 1 { acc.push(char::from_u32(code).unwrap_or('\u{fffd}')); st = St::Str } else { st = St::Uni(left - 1, code) } }
            },
            St::Atom(_) => unreachable!(),
        }
    }
    match st { St::Idle => {} St::Atom(num) => atom(&mut acc, num, &mut out), _ => out.push(Tok::Bad) }
    out
}

/// Json.parse: the value of a JSON text, None when malformed (Parse.fin), Err past `MOST_DEPTH`.
pub fn parse(s: &str) -> Result<Option<J>, TooBig> {
    enum Frame { Arr(Vec<J>), Obj(Vec<(String, J)>, Option<String>) }
    fn put(stack: &mut Vec<Frame>, top: &mut Option<J>, v: J) -> bool {
        match stack.last_mut() {
            None => *top = Some(v),
            Some(Frame::Arr(a)) => a.push(v),
            Some(Frame::Obj(a, k)) => match (k.take(), v) { (Some(k), v) => a.push((k, v)), (None, J::Str(s)) => *k = Some(s), _ => return false },
        }
        true
    }
    let (mut stack, mut top): (Vec<Frame>, Option<J>) = (vec![], None);
    for t in lex(s) {
        top = None; // a token after a finished value leaves none (Parse.tok)
        let ok = match t {
            Tok::Open(o) => { stack.push(if o { Frame::Obj(vec![], None) } else { Frame::Arr(vec![]) }); if stack.len() > MOST_DEPTH { return Err(TooBig) } true }
            Tok::Close => match stack.pop() {
                None => false,
                Some(Frame::Arr(a)) => put(&mut stack, &mut top, J::Arr(a)),
                Some(Frame::Obj(a, _)) => put(&mut stack, &mut top, J::Obj(a)),
            },
            Tok::Colon | Tok::Comma => true,
            Tok::Str(s) => put(&mut stack, &mut top, J::Str(s)),
            Tok::Num(r) => put(&mut stack, &mut top, J::Num(r)),
            Tok::Word(w) => put(&mut stack, &mut top, match w.as_str() { "true" => J::Flag(true), "false" => J::Flag(false), _ => J::Null }),
            Tok::Bad => false,
        };
        if !ok { return Ok(None) }
    }
    Ok(if stack.is_empty() { top } else { None })
}

/// U32.read: digits only, None past 32 bits (the runtime's check: n / 10 is the number before).
pub fn u32_read(s: &str) -> Option<u32> {
    if s.is_empty() { return None }
    let mut acc = 0u32;
    for c in s.chars() { let n = acc.wrapping_mul(10).wrapping_add((c as u32).wrapping_sub(48)); if n / 10 != acc { return None } acc = n }
    Some(acc)
}
impl J {
    /// Json.get: the first field of that name
    fn get(&self, key: &str) -> Option<&J> { if let J::Obj(fs) = self { fs.iter().find(|f| f.0 == key).map(|f| &f.1) } else { None } }
    fn get_obj(&self, key: &str) -> &J { self.get(key).unwrap_or(&NULL) }
    fn get_u32(&self, key: &str, d: u32) -> u32 { match self.get(key) { Some(J::Num(r)) | Some(J::Str(r)) => u32_read(r).unwrap_or(d), _ => d } }
    fn get_str<'a>(&'a self, key: &str, d: &'a str) -> &'a str { if let Some(J::Str(s)) = self.get(key) { s } else { d } }
    fn elems(&self) -> &[J] { if let J::Arr(a) = self { a } else { &[] } }
    /// Glb.nums: the numbers of an array field (0 for what is no number)
    fn nums(&self, key: &str) -> Vec<f32> { self.get_obj(key).elems().iter().map(|j| if let J::Num(r) = j { fl::read(r).unwrap_or(0.0) } else { 0.0 }).collect() }
    /// Glb.u32s: an array field's numbers as U32 (0 for one that does not read), what is no number left out
    fn u32s(&self, key: &str) -> Vec<u32> { self.get_obj(key).elems().iter().filter_map(|j| if let J::Num(r) = j { Some(u32_read(r).unwrap_or(0)) } else { None }).collect() }
}
fn nth(xs: &[f32], n: usize, d: f32) -> f32 { xs.get(n).copied().unwrap_or(d) }

// Tables ------------------------------------------------------------------------------------------------------------------------

/// Tab.depth: the smallest d >= 1 with 2^d >= n
fn depth(n: usize) -> u32 { let mut d = 1; while (1u64 << d) < n as u64 { d += 1 } d }
/// Tab.at: an index reads only the table's d bits, and a slot past the end is the default
fn mask(n: usize) -> u64 { (1u64 << depth(n)) - 1 }
struct Tbl<'a> { xs: &'a [J] }
impl<'a> Tbl<'a> { fn at(&self, i: u32) -> &'a J { self.xs.get((i as u64 & mask(self.xs.len())) as usize).unwrap_or(&NULL) } }

// Transforms --------------------------------------------------------------------------------------------------------------------

/// an affine transform, row-major: [a b c d; e f g h; i j k l]
#[derive(Clone, Copy, Debug, PartialEq)]
struct M4([f32; 12]);
const ID: M4 = M4([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
fn dot(a: f32, b: f32, c: f32, x: f32, y: f32, z: f32) -> f32 { a * x + b * y + c * z }
impl M4 {
    /// M4.mul: q first, then self
    fn mul(&self, q: &M4) -> M4 {
        let [a, b, c, d, e, f, g, h, i, j, k, l] = self.0; let [qa, qb, qc, qd, qe, qf, qg, qh, qi, qj, qk, ql] = q.0;
        M4([dot(a, b, c, qa, qe, qi), dot(a, b, c, qb, qf, qj), dot(a, b, c, qc, qg, qk), dot(a, b, c, qd, qh, ql) + d,
            dot(e, f, g, qa, qe, qi), dot(e, f, g, qb, qf, qj), dot(e, f, g, qc, qg, qk), dot(e, f, g, qd, qh, ql) + h,
            dot(i, j, k, qa, qe, qi), dot(i, j, k, qb, qf, qj), dot(i, j, k, qc, qg, qk), dot(i, j, k, qd, qh, ql) + l])
    }
    fn app(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let [a, b, c, d, e, f, g, h, i, j, k, l] = self.0;
        [dot(a, b, c, x, y, z) + d, dot(e, f, g, x, y, z) + h, dot(i, j, k, x, y, z) + l]
    }
    /// M4.cols: a glTF matrix is column-major
    fn cols(m: &[f32]) -> M4 {
        M4([nth(m, 0, 1.0), nth(m, 4, 0.0), nth(m, 8, 0.0), nth(m, 12, 0.0), nth(m, 1, 0.0), nth(m, 5, 1.0), nth(m, 9, 0.0), nth(m, 13, 0.0), nth(m, 2, 0.0), nth(m, 6, 0.0), nth(m, 10, 1.0), nth(m, 14, 0.0)])
    }
    /// M4.trs: translation * rotation (unit quaternion x y z w) * scale
    fn trs(t: &[f32], r: &[f32], s: &[f32]) -> M4 {
        let (tx, ty, tz) = (nth(t, 0, 0.0), nth(t, 1, 0.0), nth(t, 2, 0.0));
        let (x, y, z, w) = (nth(r, 0, 0.0), nth(r, 1, 0.0), nth(r, 2, 0.0), nth(r, 3, 1.0));
        let (sx, sy, sz) = (nth(s, 0, 1.0), nth(s, 1, 1.0), nth(s, 2, 1.0));
        M4([(1.0 - 2.0 * (y * y + z * z)) * sx, (2.0 * (x * y - z * w)) * sy, (2.0 * (x * z + y * w)) * sz, tx,
            (2.0 * (x * y + z * w)) * sx, (1.0 - 2.0 * (x * x + z * z)) * sy, (2.0 * (y * z - x * w)) * sz, ty,
            (2.0 * (x * z - y * w)) * sx, (2.0 * (y * z + x * w)) * sy, (1.0 - 2.0 * (x * x + y * y)) * sz, tz])
    }
    /// M4.of_node: a node's local transform
    fn of_node(n: &J) -> M4 { if n.get("matrix").is_some() { M4::cols(&n.nums("matrix")) } else { M4::trs(&n.nums("translation"), &n.nums("rotation"), &n.nums("scale")) } }
}

// Accessors ---------------------------------------------------------------------------------------------------------------------

/// where an accessor's elements are: byte start, element count, stride, component type and the byte length that covers them all
#[derive(Clone, Copy, Debug)]
struct Acc { start: u32, count: u32, stride: u32, ct: u32, len: u32 }
fn csize(ct: u32) -> u32 { if ct <= 5121 { 1 } else if ct <= 5123 { 2 } else { 4 } }
fn ncomp(t: &str) -> u32 { match t { "SCALAR" => 1, "VEC2" => 2, "VEC3" => 3, "VEC4" => 4, _ => 16 } }
impl Acc {
    fn of(a: &J, views: &Tbl) -> Acc {
        let v = views.at(a.get_u32("bufferView", 0)); let ct = a.get_u32("componentType", 5126);
        let (start, count, vstride) = (v.get_u32("byteOffset", 0).wrapping_add(a.get_u32("byteOffset", 0)), a.get_u32("count", 0), v.get_u32("byteStride", 0));
        let esize = csize(ct).wrapping_mul(ncomp(a.get_str("type", "SCALAR")));
        let stride = if vstride == 0 { esize } else { vstride };
        Acc { start, count, stride, ct, len: if count == 0 { 0 } else { stride.wrapping_mul(count - 1).wrapping_add(esize) } }
    }
}
/// The BIN chunk as words (a partial last word is not read).
struct Bin { w: Vec<u32> }
impl Bin {
    /// Bin.words: the words covering len bytes from byte start
    fn words(&self, start: u32, len: u32) -> &[u32] {
        let (lo, hi) = ((start / 4) as usize, (start.wrapping_add(len).wrapping_add(3) / 4) as usize);
        let hi = hi.min(self.w.len());
        if lo < hi { &self.w[lo..hi] } else { &[] }
    }
    /// Bin.slice: len bytes from byte start
    fn slice(&self, start: u32, len: u32) -> Vec<u8> {
        let mut o: Vec<u8> = self.words(start, len).iter().flat_map(|w| w.to_le_bytes()).collect();
        let skip = ((start % 4) as usize).min(o.len()); o.drain(..skip); o.truncate(len as usize); o
    }
}
fn word_at(bs: &[u8], at: u64) -> u32 { // Bytes.u32 of what is left: 0 when under four bytes are
    match usize::try_from(at).ok().and_then(|a| bs.get(a..a.checked_add(4)?)) { Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]), None => 0 }
}
/// Pos.of: an accessor's positions in world space
fn positions(a: &Acc, bin: &Bin, m: &M4) -> Result<Vec<[f32; 3]>, TooBig> {
    let f = f32::from_bits;
    if a.start % 4 == 0 && a.stride % 4 == 0 {
        let ws = bin.words(a.start, a.len); let step = 3 + (a.stride.wrapping_sub(12) / 4) as usize;
        let mut o = Vec::with_capacity((a.count as usize).min(ws.len() / 3 + 1));
        let mut at = 0usize;
        for _ in 0..a.count {
            let Some(p) = ws.get(at..at.saturating_add(3)) else { break };
            o.push(m.app(f(p[0]), f(p[1]), f(p[2]))); at = at.saturating_add(step);
        }
        return Ok(o)
    }
    if a.count as usize > 3 * MOST_TRIS { return Err(TooBig) }
    let bs = bin.slice(a.start, a.len); let step = 12 + a.stride.wrapping_sub(12) as u64;
    Ok((0..a.count as u64).map(|i| { let at = i * step; m.app(f(word_at(&bs, at)), f(word_at(&bs, at + 4)), f(word_at(&bs, at + 8))) }).collect())
}
/// Idx.of: an accessor's indices
fn indices(a: &Acc, bin: &Bin) -> Result<Vec<u32>, TooBig> {
    let kind = if a.start % 4 == 0 && a.stride == csize(a.ct) { a.stride } else { 0 };
    match kind {
        2 => Ok(bin.words(a.start, a.len).iter().flat_map(|w| [w & 65535, w >> 16]).take(a.count as usize).collect()),
        4 => Ok(bin.words(a.start, a.len).iter().copied().take(a.count as usize).collect()),
        _ => {
            if a.count as usize > 3 * MOST_TRIS { return Err(TooBig) }
            let (bs, size, stride) = (bin.slice(a.start, a.len), csize(a.ct), a.stride as u64);
            Ok((0..a.count as u64).map(|i| { let at = i * stride; let byte = |k: u64| usize::try_from(at + k).ok().and_then(|x| bs.get(x)).copied();
                match size {
                    1 => byte(0).unwrap_or(0) as u32,
                    2 => match (byte(0), byte(1)) { (Some(a), Some(b)) => a as u32 | (b as u32) << 8, _ => 0 },
                    _ => word_at(&bs, at),
                } }).collect())
        }
    }
}

// Materials ---------------------------------------------------------------------------------------------------------------------

fn clamp01(f: f32) -> f32 { fl::min(fl::max(f, 0.0), 1.0) }
fn chan(f: f32) -> u32 { fl::to_u32(clamp01(f) * 255.0 + 0.5) }
/// Glb.color: a material's colour (untextured: the base colour factor)
fn color(m: &J) -> u32 {
    let f = m.get_obj("pbrMetallicRoughness").nums("baseColorFactor");
    let rgb = chan(nth(&f, 0, 1.0)) << 16 | chan(nth(&f, 1, 1.0)) << 8 | chan(nth(&f, 2, 1.0));
    rgb | if m.get_str("alphaMode", "OPAQUE") == "BLEND" { 255u32.wrapping_sub(chan(nth(&f, 3, 1.0))) << 24 } else { 0 }
}

// Triangles ---------------------------------------------------------------------------------------------------------------------

struct Gltf<'a> { accs: Tbl<'a>, views: Tbl<'a>, mats: Tbl<'a>, bin: Bin }
/// Prim.tris: a primitive's triangles onto `out` (only mode 4 draws)
fn prim(p: &J, g: &Gltf, m: &M4, out: &mut Vec<Tri3>) -> Result<(), TooBig> {
    if p.get_u32("mode", 4) != 4 { return Ok(()) }
    let vs = positions(&Acc::of(g.accs.at(p.get_obj("attributes").get_u32("POSITION", 0)), &g.views), &g.bin, m)?;
    let col = if p.get("material").is_some() { color(g.mats.at(p.get_u32("material", 0))) } else { 13421772 };
    let mut push = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| { if out.len() >= MOST_TRIS { return Err(TooBig) } out.push(Tri3 { a, b, c, color: col }); Ok(()) };
    if let Some(J::Num(r)) = p.get("indices") {
        let ix = indices(&Acc::of(g.accs.at(u32_read(r).unwrap_or(0)), &g.views), &g.bin)?;
        let m = mask(vs.len());
        let at = |i: u32| vs.get((i as u64 & m) as usize).copied().unwrap_or([0.0; 3]);
        for t in ix.chunks_exact(3) { push(at(t[0]), at(t[1]), at(t[2]))? }
    } else { for t in vs.chunks_exact(3) { push(t[0], t[1], t[2])? } }
    Ok(())
}

/// Glb.parse then Mesh.tris: every triangle of a GLB file in scene order; none when it is not a GLB.
pub fn tris(bs: &[u8]) -> Result<Vec<Tri3>, TooBig> {
    let u32_at = |at: u64| word_at(bs, at);
    if u32_at(0) != 1179937895 { return Ok(vec![]) }
    let jl = u32_at(12) as usize;
    let json = bs.get(20..).map(|r| &r[..jl.min(r.len())]).unwrap_or(&[]);
    let Some(j) = parse(&String::from_utf8_lossy(json))? else { return Ok(vec![]) };
    let rest = 20u64 + jl as u64;
    let n = (u32_at(rest) / 4) as usize; // BIN's words, as many as the file holds
    let body = usize::try_from(rest + 8).ok().and_then(|at| bs.get(at..)).unwrap_or(&[]);
    let bin = Bin { w: body.chunks_exact(4).take(n).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect() };
    let (nodes, meshes) = (Tbl { xs: j.get_obj("nodes").elems() }, Tbl { xs: j.get_obj("meshes").elems() });
    let g = Gltf { accs: Tbl { xs: j.get_obj("accessors").elems() }, views: Tbl { xs: j.get_obj("bufferViews").elems() }, mats: Tbl { xs: j.get_obj("materials").elems() }, bin };
    let scenes = Tbl { xs: j.get_obj("scenes").elems() };
    let roots = scenes.at(j.get_u32("scene", 0)).u32s("nodes");
    // Walk.go: depth first over the node forest, the first child next; fuel bounds the visits
    let mut work: Vec<(u32, M4)> = roots.iter().rev().map(|r| (*r, ID)).collect();
    let (mut fuel, mut out) = (nodes.xs.len().saturating_mul(2).saturating_add(16), vec![]);
    while let Some((idx, m)) = work.pop() {
        if fuel == 0 { break }
        fuel -= 1;
        let n = nodes.at(idx); let w = m.mul(&M4::of_node(n));
        work.extend(n.u32s("children").iter().rev().map(|k| (*k, w)));
        if work.len() > MOST_WALK { return Err(TooBig) }
        if n.get("mesh").is_some() { for p in meshes.at(n.get_u32("mesh", 0)).get_obj("primitives").elems() { prim(p, &g, &w, &mut out)? } }
    }
    Ok(out)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    /// A GLB of these JSON text and BIN bytes.
    pub fn glb(json: &str, bin: &[u8]) -> Vec<u8> {
        let mut j = json.as_bytes().to_vec(); while j.len() % 4 != 0 { j.push(b' ') }
        let mut b = bin.to_vec(); while b.len() % 4 != 0 { b.push(0) }
        let mut o = vec![]; o.extend(b"glTF"); o.extend(2u32.to_le_bytes()); o.extend(((28 + j.len() + b.len()) as u32).to_le_bytes());
        o.extend((j.len() as u32).to_le_bytes()); o.extend(b"JSON"); o.extend(&j); o.extend((b.len() as u32).to_le_bytes()); o.extend(b"BIN\0"); o.extend(&b); o
    }
    pub fn floats(xs: &[f32]) -> Vec<u8> { xs.iter().flat_map(|x| x.to_le_bytes()).collect() }
    const TRI: &str = r#"{"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"material":0}]}],"materials":[{"pbrMetallicRoughness":{"baseColorFactor":[1,0,0.5,1]}}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}],"bufferViews":[{"byteOffset":0}]}"#;
    fn one() -> Vec<u8> { glb(TRI, &floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0])) }

    #[test] fn a_triangle_reads_with_its_material() {
        let t = tris(&one()).unwrap();
        assert_eq!(t, vec![Tri3 { a: [0.0; 3], b: [1.0, 0.0, 0.0], c: [0.0, 1.0, 0.0], color: 0xff0080 }]);
    }
    #[test] fn not_a_glb_is_no_triangles() {
        assert!(tris(b"").unwrap().is_empty()); assert!(tris(b"glTX0000000000000000000000000000").unwrap().is_empty());
        let mut bad = one(); bad[20] = b'['; bad[21] = b'['; assert!(tris(&bad).unwrap().is_empty());
    }
    #[test] fn nodes_compose_translation_and_children_first_child_first() {
        let j = TRI.replace(r#""nodes":[{"mesh":0}]"#, r#""nodes":[{"translation":[1,2,3],"children":[1,2]},{"mesh":0,"scale":[2,2,2]},{"mesh":0,"matrix":[1,0,0,0,0,1,0,0,0,0,1,0,5,0,0,1]}]"#);
        let t = tris(&glb(&j, &floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]))).unwrap();
        assert_eq!(t.len(), 2); assert_eq!(t[0].b, [3.0, 2.0, 3.0]); assert_eq!(t[1].b, [7.0, 2.0, 3.0]);
    }
    #[test] fn a_cycle_of_nodes_ends_on_fuel() {
        let j = TRI.replace(r#""nodes":[{"mesh":0}]"#, r#""nodes":[{"mesh":0,"children":[0]}]"#);
        assert_eq!(tris(&glb(&j, &floats(&[0.0; 9]))).unwrap().len(), 18); // 1 node: 2 * 1 + 16 visits
    }
    #[test] fn indices_in_every_width_and_a_table_index_wraps() {
        let pts = floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        for (ct, ixs) in [(5121u32, vec![0u8, 1, 2]), (5123, [0u16, 1, 2].iter().flat_map(|x| x.to_le_bytes()).collect()), (5125, [0u32, 1, 2].iter().flat_map(|x| x.to_le_bytes()).collect())] {
            let j = format!(r#"{{"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1}}]}}],"accessors":[{{"bufferView":0,"count":3,"type":"VEC3"}},{{"bufferView":1,"componentType":{ct},"count":3}}],"bufferViews":[{{"byteOffset":0}},{{"byteOffset":36}}]}}"#);
            let mut bin = pts.clone(); bin.extend(&ixs);
            let t = tris(&glb(&j, &bin)).unwrap(); assert_eq!(t.len(), 1, "{ct}"); assert_eq!((t[0].b, t[0].color), ([1.0, 0.0, 0.0], 13421772));
        }
        // 3 vertices: a table of 4; index 4 wraps to 0, index 3 is past the end and reads as the origin
        let at = |i: u32| { let vs = [[1.0f32; 3], [2.0; 3], [3.0; 3]]; vs.get((i as u64 & mask(3)) as usize).copied().unwrap_or([0.0; 3]) };
        assert_eq!((at(4), at(3), at(6)), ([1.0; 3], [0.0; 3], [3.0; 3]));
        assert_eq!((depth(0), depth(1), depth(2), depth(3), depth(4), depth(5), depth(1024), depth(1025)), (1, 1, 1, 2, 2, 3, 10, 11));
    }
    #[test] fn an_aligned_accessor_stops_at_the_data_and_an_unaligned_one_reads_zeros() {
        let bin = Bin { w: vec![0, 0, 0, 1065353216, 1065353216] };
        let a = Acc { start: 0, count: 3, stride: 12, ct: 5126, len: 36 };
        assert_eq!(positions(&a, &bin, &ID).unwrap().len(), 1);
        let u = Acc { start: 2, count: 3, stride: 12, ct: 5126, len: 36 };
        assert_eq!(positions(&u, &bin, &ID).unwrap().len(), 3);
    }
    #[test] fn json_is_lenient() {
        let v = |s: &str| parse(s).unwrap();
        assert_eq!(v(r#"{"a" 1 "a":2}"#), Some(J::Obj(vec![("a".into(), J::Num("1".into())), ("a".into(), J::Num("2".into()))]))); // commas and colons unchecked, the first key wins on read
        assert_eq!(v(r#"{"a":1}"#).unwrap().get("a"), Some(&J::Num("1".into())));
        assert_eq!(v("[1,nan,true,false,\"x\\u0041\\n\"]"), Some(J::Arr(vec![J::Num("1".into()), J::Null, J::Flag(true), J::Flag(false), J::Str("xA\n".into())])));
        assert_eq!(v("[1"), None); assert_eq!(v("]"), None); assert_eq!(v(r#"{1:2}"#), None); assert_eq!(v(r#""\uZZ""#), None); assert_eq!(v("1 ,"), None); assert_eq!(v("  "), None);
        assert_eq!(v("1e-5"), Some(J::Num("1e-5".into())));
        assert_eq!(parse(&"[".repeat(MOST_DEPTH + 1)), Err(TooBig));
    }
    #[test] fn u32_reads_digits_only() {
        for (s, v) in [("0", Some(0)), ("007", Some(7)), ("4294967295", Some(u32::MAX)), ("4294967296", None), ("", None), ("1.0", None), ("-1", None), ("1e3", None)] { assert_eq!(u32_read(s), v, "{s}") }
    }
    #[test] fn too_many_triangles_are_refused_not_held() {
        let j = TRI.replace(r#""count":3"#, r#""count":4294967295"#).replace(r#""byteOffset":0"#, r#""byteOffset":2"#);
        assert_eq!(tris(&glb(&j, &floats(&[0.0; 9]))), Err(TooBig));
    }
    #[test] fn colours_blend_and_clamp() {
        let m = |s: &str| color(&parse(s).unwrap().unwrap());
        assert_eq!(m(r#"{"pbrMetallicRoughness":{"baseColorFactor":[1,1,1,1]}}"#), 0xffffff);
        assert_eq!(m(r#"{"pbrMetallicRoughness":{"baseColorFactor":[2,-1,0.5,0.5]},"alphaMode":"BLEND"}"#), 0x7fff0080);
        assert_eq!(m("{}"), 0xffffff);
    }
}
