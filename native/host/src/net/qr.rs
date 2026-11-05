//! QR codes (core/qr.bend, ISO/IEC 18004): byte mode, level M, the smallest of versions 1-40 that holds the
//! text, the mask with the least penalty (Bend's own scoring, ties to the lowest mask). A code is its rows,
//! '1' dark and '0' light, joined by newlines, with no quiet zone. Laws `qr_*`; the tests carry the
//! reference matrices from test/qr_test.bend (python-qrcode, decoded with zxing).

/// Per version at level M: error correction codewords per block, blocks of d data codewords, blocks of d + 1.
const TABLE: [(usize, usize, usize, usize); 40] = [
    (10, 1, 16, 0), (16, 1, 28, 0), (26, 1, 44, 0), (18, 2, 32, 0), (24, 2, 43, 0), (16, 4, 27, 0), (18, 4, 31, 0), (22, 2, 38, 2), (22, 3, 36, 2), (26, 4, 43, 1),
    (30, 1, 50, 4), (22, 6, 36, 2), (22, 8, 37, 1), (24, 4, 40, 5), (24, 5, 41, 5), (28, 7, 45, 3), (28, 10, 46, 1), (26, 9, 43, 4), (26, 3, 44, 11), (26, 3, 41, 13),
    (26, 17, 42, 0), (28, 17, 46, 0), (28, 4, 47, 14), (28, 6, 45, 14), (28, 8, 47, 13), (28, 19, 46, 4), (28, 22, 45, 3), (28, 3, 45, 23), (28, 21, 45, 7), (28, 19, 47, 10),
    (28, 2, 46, 29), (28, 10, 46, 23), (28, 14, 46, 21), (28, 14, 46, 23), (28, 12, 47, 26), (28, 6, 47, 34), (28, 29, 46, 14), (28, 13, 46, 32), (28, 40, 47, 7), (28, 18, 47, 31),
];
fn cap(v: usize) -> usize { let (_, n1, d, n2) = TABLE[v - 1]; n1 * d + n2 * (d + 1) }
fn cc(v: usize) -> usize { if v <= 9 { 8 } else { 16 } }
/// The smallest version that holds `len` bytes, 0 for none.
pub fn version(len: usize) -> usize { (1..=40).find(|&v| 4 + cc(v) + len * 8 <= cap(v) * 8).unwrap_or(0) }
fn size(v: usize) -> usize { 17 + v * 4 }

fn push_bits(out: &mut Vec<bool>, x: usize, n: usize) { for i in (0..n).rev() { out.push((x >> i) & 1 == 1) } }
/// Mode 0100, the count, the bytes, up to four zero bits, then 236 and 17 in turn up to the capacity.
fn data(v: usize, bs: &[u8]) -> Vec<u8> {
    let mut bits = vec![];
    push_bits(&mut bits, 4, 4); push_bits(&mut bits, bs.len(), cc(v));
    for b in bs { push_bits(&mut bits, *b as usize, 8) }
    let room = cap(v) * 8 - bits.len();
    for _ in 0..room.min(4) { bits.push(false) }
    let mut cw: Vec<u8> = bits.chunks(8).map(|c| { let mut b = 0u8; for (i, x) in c.iter().enumerate() { if *x { b |= 0x80 >> i } } b }).collect();
    let mut odd = false;
    while cw.len() < cap(v) { cw.push(if odd { 17 } else { 236 }); odd = !odd }
    cw
}
fn gmul(mut a: u32, mut b: u32) -> u32 {
    let mut r = 0;
    for _ in 0..8 { let a2 = a << 1; if b & 1 == 1 { r ^= a } a = if a2 & 256 != 0 { a2 ^ 285 } else { a2 }; b >>= 1 }
    r
}
/// The generator of degree e, (x - 1)(x - 2)...(x - 2^(e-1)), highest first without the leading 1.
fn gen(e: usize) -> Vec<u32> {
    let (mut p, mut root) = (vec![1u32], 1u32);
    for _ in 0..e {
        let mut next = p.clone(); next.push(0);
        for (i, c) in p.iter().enumerate() { next[i + 1] ^= gmul(*c, root) }
        p = next; root = gmul(root, 2);
    }
    p[1..].to_vec()
}
fn rs(ds: &[u8], g: &[u32]) -> Vec<u8> {
    let mut rem = vec![0u32; g.len()];
    for d in ds {
        let f = *d as u32 ^ rem[0];
        rem.remove(0); rem.push(0);
        for (r, c) in rem.iter_mut().zip(g) { *r ^= gmul(*c, f) }
    }
    rem.into_iter().map(|x| x as u8).collect()
}
/// Every codeword in the order it is placed: data then error correction, each interleaved across the blocks.
fn codewords(v: usize, bs: &[u8]) -> Vec<u8> {
    let (ec, n1, d, n2) = TABLE[v - 1];
    let all = data(v, bs);
    let mut blocks: Vec<&[u8]> = vec![]; let mut at = 0;
    for i in 0..n1 + n2 { let len = if i < n1 { d } else { d + 1 }; blocks.push(&all[at..at + len]); at += len }
    let g = gen(ec);
    let eccs: Vec<Vec<u8>> = blocks.iter().map(|b| rs(b, &g)).collect();
    let mut out = vec![];
    for i in 0..d + 1 { for b in &blocks { if let Some(x) = b.get(i) { out.push(*x) } } }
    for i in 0..ec { for e in &eccs { out.push(e[i]) } }
    out
}

fn bch(m: u32, top: u32, poly: u32, steps: u32) -> u32 {
    let mut rem = m;
    for _ in 0..steps { rem = (rem << 1) ^ if (rem >> top) & 1 == 1 { poly } else { 0 } }
    rem
}
fn format_bits(m: u32) -> u32 { (((m << 10) | bch(m, 9, 1335, 10)) ^ 21522) & 0x7fff }
fn version_bits(v: u32) -> u32 { (v << 12) | bch(v, 11, 7973, 12) }
fn aligns(v: usize) -> Vec<usize> {
    if v == 1 { return vec![] }
    let n = v / 7 + 2;
    let step = if v == 32 { 26 } else { (v * 4 + n * 2 + 1) / (n * 2 - 2) * 2 };
    let mut out = vec![6]; let mut ps: Vec<usize> = (0..n - 1).map(|i| size(v) - 7 - i * step).collect(); ps.reverse(); out.extend(ps); out
}

struct Grid { s: usize, dark: Vec<bool>, func: Vec<bool> }
impl Grid {
    fn set(&mut self, x: usize, y: usize, d: bool) { self.dark[y * self.s + x] = d; self.func[y * self.s + x] = true }
    fn is_func(&self, x: usize, y: usize) -> bool { self.func[y * self.s + x] }
    fn finder(&mut self, cx: i32, cy: i32) {
        for dy in -4i32..=4 { for dx in -4i32..=4 {
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 || x >= self.s as i32 || y >= self.s as i32 { continue }
            let d = dx.abs().max(dy.abs());
            self.set(x as usize, y as usize, d != 2 && d != 4);
        } }
    }
    fn format(&mut self, m: u32) {
        let (b, s) = (format_bits(m), self.s);
        let bit = |i: usize| (b >> i) & 1 == 1;
        for i in 0..6 { self.set(8, i, bit(i)) }
        self.set(8, 7, bit(6)); self.set(8, 8, bit(7)); self.set(7, 8, bit(8));
        for i in 9..15 { self.set(14 - i, 8, bit(i)) }
        for i in 0..8 { self.set(s - 1 - i, 8, bit(i)) }
        for i in 8..15 { self.set(8, s - 15 + i, bit(i)) }
        self.set(8, s - 8, true);
    }
}
/// The finished module grid with data placed and `mask` applied.
fn build(v: usize, cw: &[u8], mask: u32) -> Grid {
    let s = size(v);
    let mut g = Grid { s, dark: vec![false; s * s], func: vec![false; s * s] };
    for i in 0..s { g.set(6, i, i % 2 == 0); g.set(i, 6, i % 2 == 0) }
    g.finder(3, 3); g.finder(s as i32 - 4, 3); g.finder(3, s as i32 - 4);
    let al = aligns(v);
    for (i, &cy) in al.iter().enumerate() { for (j, &cx) in al.iter().enumerate() {
        if (i == 0 && j == 0) || (i == 0 && j == al.len() - 1) || (i == al.len() - 1 && j == 0) { continue }
        for dy in -2i32..=2 { for dx in -2i32..=2 { g.set((cx as i32 + dx) as usize, (cy as i32 + dy) as usize, dx.abs().max(dy.abs()) != 1) } }
    } }
    g.format(mask);
    if v >= 7 { let b = version_bits(v as u32); for i in 0..18 { let (a, c) = (s - 11 + i % 3, i / 3); g.set(a, c, (b >> i) & 1 == 1); g.set(c, a, (b >> i) & 1 == 1) } }
    let bits: Vec<bool> = cw.iter().flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1 == 1)).collect();
    let (mut k, mut right) = (0, s as i32 - 1);
    while right >= 1 {
        if right == 6 { right = 5 }
        for vert in 0..s { for j in 0..2 {
            let x = (right - j) as usize;
            let up = ((right + 1) & 2) == 0;
            let y = if up { s - 1 - vert } else { vert };
            if !g.is_func(x, y) {
                let b = k < bits.len() && bits[k]; k += 1;
                g.dark[y * s + x] = b != masked(mask, x, y);
            }
        } }
        right -= 2;
    }
    g
}
fn masked(m: u32, x: usize, y: usize) -> bool {
    let (r, c) = (y, x); let p = r * c; let a = (r + c) % 2;
    match m { 0 => a == 0, 1 => r % 2 == 0, 2 => c % 3 == 0, 3 => (r + c) % 3 == 0, 4 => (r / 2 + c / 3) % 2 == 0, 5 => p % 2 + p % 3 == 0, 6 => (p % 2 + p % 3) % 2 == 0, _ => (a + p % 3) % 2 == 0 }
}

/// Core/qr.bend's penalty: runs of five or more (3, and 1 beyond), 1011101 with four light either side (40),
/// 2x2 blocks (3), and the dark share's distance from half in steps of five (10 each).
fn penalty(g: &Grid) -> usize {
    let s = g.s;
    let at = |x: usize, y: usize| g.dark[y * s + x];
    let rows: Vec<Vec<bool>> = (0..s).map(|y| (0..s).map(|x| at(x, y)).collect()).collect();
    let cols: Vec<Vec<bool>> = (0..s).map(|x| (0..s).map(|y| at(x, y)).collect()).collect();
    let line = |l: &Vec<bool>| -> usize {
        let (mut acc, mut cur, mut n) = (0usize, l[0], 1usize);
        let end = |n: usize, acc: usize| if n >= 5 { acc + n - 2 } else { acc };
        for &x in &l[1..] { if x == cur { n += 1 } else { acc = end(n, acc); cur = x; n = 1 } }
        acc = end(n, acc);
        let mut h = 0u32;
        for (i, &x) in l.iter().enumerate() { h = ((h << 1) | x as u32) & 2047; if i >= 10 && (h == 1488 || h == 93) { acc += 40 } }
        acc
    };
    let mut total: usize = rows.iter().chain(cols.iter()).map(line).sum();
    for y in 1..s { for x in 1..s { let c = at(x, y); if c == at(x - 1, y) && c == at(x, y - 1) && c == at(x - 1, y - 1) { total += 3 } } }
    let darks = rows.iter().flatten().filter(|d| **d).count();
    let pct = darks * 100 / (s * s);
    total + pct.abs_diff(50) / 5 * 10
}

fn show(g: &Grid) -> String {
    (0..g.s).map(|y| (0..g.s).map(|x| if g.dark[y * g.s + x] { '1' } else { '0' }).collect::<String>()).collect::<Vec<_>>().join("\n")
}
/// Qr.rows.with: the code with mask `m` (None: the best, ties to the lowest), "" for empty text or too long.
pub fn rows_with(text: &str, m: Option<u32>) -> String {
    let bs = text.as_bytes();
    if bs.is_empty() { return String::new() }
    let v = version(bs.len());
    if v == 0 { return String::new() }
    let cw = codewords(v, bs);
    match m {
        Some(k) => show(&build(v, &cw, k)),
        None => {
            let mut best: Option<(usize, Grid)> = None;
            for k in 0..8 { let g = build(v, &cw, k); let p = penalty(&g); if best.as_ref().map_or(true, |(bp, _)| p < *bp) { best = Some((p, g)) } }
            show(&best.unwrap().1)
        }
    }
}
/// Qr.text: the best code as rows joined by newlines.
pub fn text(link: &str) -> String { rows_with(link, None) }

/// Qr.link: the app's own link, carrying the hub's pairing link, so a phone's camera opens the app.
pub fn link(l: &str) -> String {
    if l.is_empty() { return String::new() }
    let esc: String = l.bytes().map(|c| if c.is_ascii_alphanumeric() || (47..=58).contains(&c) || matches!(c, b'-' | b'.' | b'=' | b'_' | b'~') { (c as char).to_string() } else { format!("%{c:02X}") }).collect();
    format!("backplane://pair?url={esc}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn qr_codewords() {
        assert_eq!(codewords(1, &[65]), [64, 20, 16, 236, 17, 236, 17, 236, 17, 236, 17, 236, 17, 236, 17, 236, 107, 112, 244, 24, 163, 122, 17, 95, 52, 252]);
    }
    #[test] fn qr_hello_mask_3() { assert_eq!(rows_with("hello", Some(3)), "111111101000001111111\n100000101000001000001\n101110100101001011101\n101110101010101011101\n101110100111001011101\n100000100111101000001\n111111101010101111111\n000000001111100000000\n101101110101101001011\n111010010011111001101\n110111101101000000011\n110101000111001111010\n010100111100100100001\n000000001011001000100\n111111101011100100000\n100000101010000111110\n101110100000111111111\n101110101011001011110\n101110101000101100100\n100000100000010110001\n111111101110010100100"); }
    #[test] fn qr_link_mask_5() {
        assert_eq!(rows_with(&link("http://100.101.102.103:3787/#token=0123456789abcdef0123456789abcdef"), Some(5)), "11111110010000111100101001001100101111111\n10000010111101000010011010000000101000001\n10111010110000111001000000110001001011101\n10111010100010001010000000110100101011101\n10111010000001011001010110100011001011101\n10000010001000000000110000000000101000001\n11111110101010101010101010101010101111111\n00000000101101010010100110111000000000000\n10000010101001100100101001100110011001110\n00010000001110101001001100110101010010110\n01011010000011101110001000010100010110110\n11100000100000110010101000100011110101001\n00111011110100010011101100111010101101001\n00001101110111011000101011101001101011101\n00011011101110011111011000000000010011100\n00010000100110011001000010010010001010110\n11010010011011101111101011101110100101101\n00000001111101001101000101010001001110101\n01001011101011101001011100010001100000011\n00001000010001000110001111011011001001011\n11100011011000011000010000110010110000101\n11111100000001011011110111111111110111100\n00011111110010011010111010010110010111100\n10010001000110000011100110110010000000001\n00000011100010100010001000010011111000010\n11110000010101110001111100111011111010011\n01101011111001110110010000011010110001010\n01011000110011101000101100110000100101111\n00011110010110000101100111001110000100100\n11001101001010100000001000100001101010001\n11101111000010000101000011110101000110101\n10000101000010001010000100100001000010000\n10000011110100000110100111111110111111100\n00000000101101010001110100111101100011100\n11111110000100110010001010010101101011010\n10000010011100101000010110111110100011001\n10111010001101010111110101110101111111010\n10111010010011010001010110110111010000110\n10111010001000100110111001010100011110000\n10000010000011000001000110101001001100101\n11111110111111000110001001010110000011100");
    }
    #[test] fn qr_version_7_mask_2() { assert_eq!(rows_with(&"x".repeat(120), Some(2)), "111111100000100001010111010101100100101111111\n100000100100000100000101111111001101001000001\n101110101110101110101000101010011101001011101\n101110101100000101111010000000110001101011101\n101110101000101111011111110101100011101011101\n100000101101100001001000111111001000001000001\n111111101010101010101010101010101010101111111\n000000001110100000001000101010011110000000000\n101111100000010101001111100000110101001111100\n000000000001111110101111010101100001110100011\n010000110000000000000101111111001010111110110\n111001000111000000100000101010011110001011100\n110000100111110111100010000000110101000001001\n001110000011011111001111010101100001110100011\n101010110100111100011101111111001010111110110\n000001011011000110110000101010011110001011100\n110110110101111011100010000000110101000001001\n110101010011011101001111010101100001110100011\n011100100100100000011101111111001010111110110\n111101001011001000110000101010011110001011100\n011111111100111111101111100000110101111111001\n101110001011001111001000110101100001100010011\n010110101011010000101010111111001011101010110\n010110001010011011111000101010011110100011100\n111111111101000110111111100000110100111111001\n001100010001110111111010010101100000100000011\n010110111010110000000111011111001011010100110\n010110000100011010100101101010011111011111100\n111101101110000010011000100000110100101011001\n001000001101110111011010010101100000100000011\n000010110010000001100111011111001011010100110\n101110001100101000100101101010011111011111100\n010111101110000000111000100000110100101011001\n011000001111110001011010010101100000100000011\n000010110010001111100111011111001011010100110\n011110001100110110100101101010011111011111100\n100110111110011000111111100000110100111111001\n000000001110000101011000110101100000100010011\n111111100001001100111010111111001010101010110\n100000101101000010011000101010011110100011100\n101110101011001001001111100000110101111111001\n101110101001000100100000110101100001001010011\n101110101000101100001010011111001011100000110\n100000100111100010011111001010011110110101100\n111111101010101001010101100000110100011111010"); }
    #[test] fn qr_format_m() { assert_eq!(format_bits(0), 21522); }
    #[test] fn qr_format_m1() { assert_eq!(format_bits(1), 20773); }
    #[test] fn qr_version7_bits() { assert_eq!(version_bits(7), 31892); }
    #[test] fn qr_gmul_wraps() { assert_eq!(gmul(128, 2), 29); }
    #[test] fn qr_versions() { assert_eq!([version(84), version(85), version(2331), version(2332)], [5, 6, 40, 0]); }
    #[test] fn qr_link_escapes() {
        assert_eq!(link("http://100.101.102.103:3787/#token=ab"), "backplane://pair?url=http://100.101.102.103:3787/%23token=ab");
        assert_eq!(link("http://h:1/#token=a b&c"), "backplane://pair?url=http://h:1/%23token=a%20b%26c");
        assert_eq!(link(""), "");
    }
    #[test] fn qr_empty_and_too_long() { assert_eq!(text(""), ""); assert_eq!(text(&"y".repeat(2332)), ""); assert_eq!(text("hello").lines().count(), 21); }
    #[test] fn qr_best_mask_is_the_lowest_penalty() {
        // every mask yields a valid grid of the same size; the chosen one scores no worse than any other
        let cw = codewords(1, b"hello"); let best = text("hello");
        let scores: Vec<usize> = (0..8).map(|k| penalty(&build(1, &cw, k))).collect();
        let pick = (0..8).min_by_key(|k| scores[*k as usize]).unwrap();
        assert_eq!(best, show(&build(1, &cw, pick)));
    }
}
