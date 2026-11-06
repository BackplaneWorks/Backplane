//! core/png.bend in Rust: a PNG file from 8-bit RGB rows, byte for byte: each row filtered with Sub, deflated in one fixed-Huffman
//! block whose only matches are runs of the byte before (distance 1), so equal pixels give equal files.

/// Png.crc: CRC-32 as PNG chunks use it
pub fn crc(bs: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for b in bs { c ^= *b as u32; for _ in 0..8 { c = if c & 1 == 1 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 } } }
    c ^ 0xffff_ffff
}
/// Png.adler: Adler-32, zlib's trailer
pub fn adler(bs: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for x in bs { a = (a + *x as u32) % 65521; b = (b + a) % 65521 }
    (b << 16) | a
}
fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes()); out.extend(ty); out.extend(data);
    let mut c = ty.to_vec(); c.extend(data); out.extend(crc(&c).to_be_bytes());
}
/// Png.sub: a filter byte (1) before each row, then each byte less the one three before it in the row
fn sub(bs: &[u8], w: usize) -> Vec<u8> {
    let row = w * 3; let mut o = Vec::with_capacity(bs.len() + bs.len() / row.max(1) + 1);
    for r in bs.chunks(row.max(1)) { o.push(1); for (i, x) in r.iter().enumerate() { o.push(if i < 3 { *x } else { x.wrapping_sub(r[i - 3]) }) } }
    o
}
/// bits out, least significant first
struct Bw { acc: u32, n: u32, out: Vec<u8> }
impl Bw {
    fn put(&mut self, v: u32, len: u32) { self.acc |= v << self.n; self.n += len; while self.n >= 8 { self.out.push(self.acc as u8); self.acc >>= 8; self.n -= 8 } }
    /// a Huffman code goes out most significant bit first
    fn code(&mut self, c: u32, len: u32) { self.put(c.reverse_bits() >> (32 - len), len) }
    /// a literal/length symbol in the fixed code (RFC 1951 3.2.6)
    fn sym(&mut self, s: u32) { match s { 0..=143 => self.code(s + 48, 8), 144..=255 => self.code(s + 256, 9), 256..=279 => self.code(s - 256, 7), _ => self.code(s - 88, 8) } }
    /// a match of l bytes at distance 1 (distance code 0, five zero bits)
    fn run_match(&mut self, l: u32) {
        let m = l - 3;
        let (s, e, x) = if l == 258 { (285, 0, 0) } else if m < 8 { (257 + m, 0, 0) } else { let e = (32 - m.leading_zeros()) - 3; (257 + 4 * (e + 1) + ((m >> e) - 4), e, m & ((1 << e) - 1)) };
        self.sym(s); self.put(x, e); self.put(0, 5);
    }
    /// a run of r copies of b after b itself: a match, or literals when short
    fn run(&mut self, b: u32, r: u32) { if r < 3 { for _ in 0..r { self.sym(b) } } else { self.run_match(r) } }
}
/// Png.deflate: one final fixed-Huffman block (BFINAL 1, BTYPE 01), then end of block
fn deflate(bs: &[u8]) -> Vec<u8> {
    let mut w = Bw { acc: 0, n: 0, out: Vec::with_capacity(bs.len() / 4 + 16) };
    w.put(3, 3);
    if let Some((first, rest)) = bs.split_first() {
        let (mut b, mut r) = (*first as u32, 0u32); w.sym(b);
        for x in rest { let x = *x as u32; if x == b && r < 258 { r += 1 } else { w.run(b, r); b = x; r = 0; w.sym(x) } }
        w.run(b, r);
    }
    w.sym(256); w.put(0, 7);
    w.out
}
/// Png.rgb: w x h pixels of 8-bit RGB, row by row, as a PNG file's bytes
pub fn rgb(bs: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut o = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut head = vec![]; head.extend(w.to_be_bytes()); head.extend(h.to_be_bytes()); head.extend([8, 2, 0, 0, 0]);
    chunk(&mut o, b"IHDR", &head);
    let filtered = sub(bs, w as usize);
    let mut z = vec![120, 1]; z.extend(deflate(&filtered)); z.extend(adler(&filtered).to_be_bytes());
    chunk(&mut o, b"IDAT", &z); chunk(&mut o, b"IEND", &[]);
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    // law png_crc_iend: PNG chunks carry the standard CRC-32 (IEND's is AE426082)
    #[test] fn png_crc_iend() { assert_eq!(crc(b"IEND"), 2923585666); }
    #[test] fn adler_is_zlibs() { assert_eq!(adler(b"Wikipedia"), 0x11E60398); assert_eq!(adler(b""), 1); }
    /// a whole inflate of fixed-Huffman blocks, so the stream is checked against RFC 1951 and not against the encoder
    fn inflate(z: &[u8]) -> Vec<u8> {
        let (mut pos, mut out) = (0usize, vec![]);
        let mut bit = |n: u32| { let mut v = 0u32; for i in 0..n { v |= ((z[pos / 8] >> (pos % 8) & 1) as u32) << i; pos += 1 } v };
        assert_eq!(bit(3), 3);
        loop {
            let mut code = 0u32; let mut len = 0;
            let sym = loop {
                code = code << 1 | bit(1); len += 1;
                match (len, code) {
                    (7, 0..=23) => break code + 256,
                    (8, 48..=191) => break code - 48,
                    (8, 192..=199) => break code - 192 + 280,
                    (9, 400..=511) => break code - 400 + 144,
                    _ => assert!(len < 9),
                }
            };
            match sym {
                0..=255 => out.push(sym as u8),
                256 => return out,
                _ => {
                    let (base, e) = match sym { 257..=264 => (sym - 254, 0), 285 => (258, 0), _ => { let e = (sym - 261) / 4; (3 + ((4 + (sym - 261) % 4) << e), e) } };
                    let l = base + bit(e);
                    assert_eq!(bit(5), 0);
                    for _ in 0..l { out.push(*out.last().unwrap()) }
                }
            }
        }
    }
    #[test] fn deflate_round_trips_every_run_length() {
        let mut bs = vec![]; for r in 0..600usize { bs.extend(std::iter::repeat((r % 251) as u8).take(r)); bs.push(((r * 7) % 256) as u8) }
        assert_eq!(inflate(&deflate(&bs)), bs);
        assert_eq!(inflate(&deflate(&[])), Vec::<u8>::new()); assert_eq!(inflate(&deflate(&[7])), vec![7]); assert_eq!(inflate(&deflate(&[9; 2000])), vec![9; 2000]);
    }
    #[test] fn a_two_pixel_image_is_exact() {
        // the filtered rows of 2x1 pixels: 1, r g b, then the second pixel less the first
        assert_eq!(sub(&[10, 20, 30, 11, 19, 30], 2), vec![1, 10, 20, 30, 1, 255, 0]);
        let p = rgb(&[10, 20, 30, 11, 19, 30], 2, 1);
        assert_eq!(&p[..8], &[137, 80, 78, 71, 13, 10, 26, 10]); assert_eq!(&p[12..16], b"IHDR"); assert_eq!(&p[p.len() - 8..p.len() - 4], b"IEND");
        assert_eq!(&p[p.len() - 4..], &[0xAE, 0x42, 0x60, 0x82]);
    }
}
