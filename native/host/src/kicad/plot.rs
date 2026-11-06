//! core/plot's hub side in Rust: the child's output parsed (Scan.plot), a phone's delta against what it holds
//! (Plot.delta, Plot.quiet) and the frames it receives (Plot.msg.fam, Plot.msg.solid), byte for byte.
//! Decision (M6): the parser stays the existing child, `backplane --plot KIND FILE`; Rust only reads its lines.
//! Laws plot_delta_exact, plot_unchanged_sends_nothing are stated as Rust tests below.

/// `BACKPLANE_PLOT_TIMES=1`: each stage of a viewer's plot (read, parse, chunks, delta, frame) as a `PLOT_TIMES {json}` line on stderr.
pub fn times(line: impl FnOnce() -> String) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| std::env::var("BACKPLANE_PLOT_TIMES").map_or(false, |v| v == "1")) { eprintln!("PLOT_TIMES {}", line()) }
}
/// A file's bytes as a 128-bit name (two multiply-mix lanes over 8-byte words, the length folded in): plots are kept by what they are of,
/// so the same board in another worktree, or saved again unchanged, is not plotted again.
pub fn digest(b: &[u8]) -> (u64, u64) {
    const K1: u64 = 0x9E37_79B9_7F4A_7C15; const K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
    let mix = |h: u64, w: u64, k: u64| (h ^ w).wrapping_mul(k).rotate_left(29);
    let (mut a, mut c) = (K2 ^ b.len() as u64, K1.wrapping_add(b.len() as u64));
    let mut it = b.chunks_exact(8);
    for w in &mut it { let w = u64::from_le_bytes(w.try_into().unwrap()); a = mix(a, w, K1); c = mix(c, w.rotate_left(17) ^ K2, K2) }
    let mut t = [0u8; 8]; let r = it.remainder(); t[..r.len()].copy_from_slice(r); let w = u64::from_le_bytes(t) ^ ((r.len() as u64) << 56);
    a = mix(a, w, K1); c = mix(c, w ^ K1, K2);
    let fin = |mut h: u64| { h ^= h >> 33; h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD); h ^= h >> 33; h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53); h ^ (h >> 33) };
    (fin(a ^ c.rotate_left(7)), fin(c ^ a.rotate_left(41)))
}
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk { pub sig: u32, pub layer: u32, pub color: u32, pub alpha: u32, pub mode: u32, pub width: u32, pub d: Vec<u8>, pub info: String }
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect { pub x0: u32, pub y0: u32, pub x1: u32, pub y1: u32 }
#[derive(Clone, Debug, PartialEq)]
pub struct Solid { pub n: u32, pub b: [u32; 6], pub d: Vec<u8> }
#[derive(Clone, Debug, PartialEq)]
pub struct Plot { pub cs: Vec<Chunk>, pub b: Rect, pub thick: u32, pub solid: Option<Solid>, pub edge: Rect }
#[derive(Clone, Debug, PartialEq)]
pub enum Ref { Keep(usize), Fresh(usize) } // Fresh: index into the plot's chunks

/// Scan.plot: the lines `Line.plot` prints. Chars, not bytes: a chunk's bytes are chars 0..255 (UTF-8 on the wire),
/// counted in chars. A plot with a malformed tail yields what was read, as scan does.
pub fn parse_child(out: &str) -> Plot {
    let cs: Vec<char> = out.chars().collect();
    let (mut p, mut at) = (Plot { cs: vec![], b: Rect::default(), thick: 1600, solid: None, edge: Rect::default() }, 0usize);
    while at < cs.len() {
        let tag = cs[at]; at += 1;
        let mut nums: Vec<u32> = vec![]; let mut cur: u32 = 0;
        let want = 8usize;
        // numbers: digits, separated by spaces (a space ends a number); 'b' ends at the newline, 'c' after 8 numbers, 'm' after its length
        loop {
            if at >= cs.len() { return p }
            let c = cs[at]; at += 1;
            if c == ' ' { nums.push(cur); cur = 0;
                if tag == 'c' && nums.len() == want { break }
                if tag == 'm' && nums.len() == 8 { break } }
            else if c == '\n' { if tag == 'b' { nums.push(cur) } break }
            else if c.is_ascii_digit() { cur = cur.wrapping_mul(10).wrapping_add(c as u32 - 48) }
        }
        match tag {
            'b' if nums.len() >= 9 => { p.b = Rect { x0: nums[0], y0: nums[1], x1: nums[2], y1: nums[3] }; p.thick = nums[4]; p.edge = Rect { x0: nums[5], y0: nums[6], x1: nums[7], y1: nums[8] } }
            'c' if nums.len() == 8 => {
                // Line.chunks: the 7th number counts the bytes, the 8th the info's chars (Sc.lens: all = both)
                let (dl, il) = (nums[6] as usize, nums[7] as usize); let all = dl + il;
                if at + all > cs.len() { return p }
                let d: Vec<u8> = cs[at..at + dl].iter().map(|c| *c as u32 as u8).collect();
                let info: String = cs[at + dl..at + dl + il].iter().collect();
                at += all;
                if cs.get(at) == Some(&'\n') { at += 1 }
                p.cs.push(Chunk { sig: nums[0], layer: nums[1], color: nums[2], alpha: nums[3], mode: nums[4], width: nums[5], d, info });
            }
            'm' if nums.len() == 8 => {
                let dl = nums[7] as usize;
                if at + dl > cs.len() { return p }
                let d: Vec<u8> = cs[at..at + dl].iter().map(|c| *c as u32 as u8).collect(); at += dl;
                if cs.get(at) == Some(&'\n') { at += 1 }
                p.solid = Some(Solid { n: nums[0], b: [nums[1], nums[2], nums[3], nums[4], nums[5], nums[6]], d });
            }
            _ => {}
        }
    }
    p
}

fn same(a: &Chunk, b: &Chunk) -> bool { a.sig == b.sig && a.layer == b.layer && a.color == b.color && a.alpha == b.alpha && a.mode == b.mode && a.width == b.width && a.d == b.d && a.info == b.info }

/// Plot.delta: each new chunk as the index of the first equal chunk the phone holds, else itself. The held chunks are looked up by
/// their signature (a hash of the chunk), so a big board against a big board is linear, not every pair compared.
pub fn delta(ns: &[Chunk], held: &[Chunk]) -> Vec<Ref> {
    if held.len() <= 8 { return ns.iter().enumerate().map(|(i, n)| match held.iter().position(|o| same(n, o)) { Some(k) => Ref::Keep(k), None => Ref::Fresh(i) }).collect() }
    let mut by: std::collections::HashMap<u32, Vec<usize>> = std::collections::HashMap::with_capacity(held.len());
    for (k, o) in held.iter().enumerate() { by.entry(o.sig).or_default().push(k) }
    ns.iter().enumerate().map(|(i, n)| match by.get(&n.sig).and_then(|ks| ks.iter().copied().find(|k| same(n, &held[*k]))) { Some(k) => Ref::Keep(k), None => Ref::Fresh(i) }).collect()
}
/// Plot.quiet: the delta changes nothing for a phone holding `held` chunks (each kept where it was).
pub fn quiet(rs: &[Ref], held: usize) -> bool { rs.len() == held && rs.iter().enumerate().all(|(i, r)| *r == Ref::Keep(i)) }
/// Plot.apply: what the phone rebuilds.
pub fn apply(rs: &[Ref], ns: &[Chunk], held: &[Chunk]) -> Vec<Chunk> {
    rs.iter().map(|r| match r { Ref::Keep(k) => held[*k].clone(), Ref::Fresh(i) => ns[*i].clone() }).collect()
}

// --- CBOR, in the hub's dialect, written straight onto the frame
fn head(o: &mut Vec<u8>, major: u8, n: u32) {
    let mb = major << 5;
    if n < 24 { o.push(mb | n as u8) } else if n < 256 { o.extend([mb | 24, n as u8]) } else if n < 65536 { o.push(mb | 25); o.extend((n as u16).to_be_bytes()) } else { o.push(mb | 26); o.extend(n.to_be_bytes()) }
}
fn text(o: &mut Vec<u8>, s: &str) { head(o, 3, s.len() as u32); o.extend(s.as_bytes()) }
fn uint(o: &mut Vec<u8>, n: u32) { head(o, 0, n) }
/// Cb.int: a two's complement U32 as major 0 or 1.
fn int(o: &mut Vec<u8>, v: u32) { if v >= 2147483648 { head(o, 1, !v) } else { head(o, 0, v) } }
fn rect(o: &mut Vec<u8>, r: &Rect) { head(o, 4, 4); int(o, r.x0); int(o, r.y0); int(o, r.x1); int(o, r.y1) }
fn bytes(o: &mut Vec<u8>, d: &[u8]) { head(o, 2, d.len() as u32); o.extend(d) }

/// Plot.msg.fam: `{1: "plot", 31: key, fam, base, box, thick, edge, cs}`.
pub fn frame(key: &str, fam: &str, base: &str, p: &Plot, rs: &[Ref]) -> Vec<u8> {
    let mut o = Vec::with_capacity(256 + p.cs.iter().map(|c| c.d.len() + 32).sum::<usize>());
    head(&mut o, 5, 8); uint(&mut o, 1); text(&mut o, "plot"); uint(&mut o, 31); text(&mut o, key);
    text(&mut o, "fam"); text(&mut o, fam); text(&mut o, "base"); text(&mut o, base);
    text(&mut o, "box"); rect(&mut o, &p.b); text(&mut o, "thick"); uint(&mut o, p.thick); text(&mut o, "edge"); rect(&mut o, &p.edge);
    text(&mut o, "cs"); head(&mut o, 4, rs.len() as u32);
    for r in rs { match r {
        Ref::Keep(k) => uint(&mut o, *k as u32),
        Ref::Fresh(i) => { let c = &p.cs[*i]; head(&mut o, 5, 7);
            text(&mut o, "s"); uint(&mut o, c.sig); text(&mut o, "l"); uint(&mut o, c.layer); text(&mut o, "c"); uint(&mut o, c.color); text(&mut o, "a"); uint(&mut o, c.alpha);
            text(&mut o, "m"); uint(&mut o, c.mode); text(&mut o, "w"); uint(&mut o, c.width); text(&mut o, "d"); bytes(&mut o, &c.d) }
    } }
    o
}
/// Plot.msg.solid: `{1: "plot", 31: key, box3, n, mesh}`.
pub fn solid_frame(key: &str, so: &Solid) -> Vec<u8> {
    let mut o = Vec::with_capacity(64 + so.d.len());
    head(&mut o, 5, 5); uint(&mut o, 1); text(&mut o, "plot"); uint(&mut o, 31); text(&mut o, key);
    text(&mut o, "box3"); head(&mut o, 4, 6); for v in so.b { int(&mut o, v) }
    text(&mut o, "n"); uint(&mut o, so.n); text(&mut o, "mesh"); bytes(&mut o, &so.d);
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ch(sig: u32, d: &[u8], info: &str) -> Chunk { Chunk { sig, layer: 5, color: 0xff00, alpha: 200, mode: 1, width: 500, d: d.to_vec(), info: info.into() } }
    fn line(c: &Chunk) -> String { format!("c{} {} {} {} {} {} {} {} {}{}\n", c.sig, c.layer, c.color, c.alpha, c.mode, c.width, c.d.len(), c.info.chars().count(), c.d.iter().map(|b| *b as char).collect::<String>(), c.info) }
    fn child(p: &Plot) -> String {
        let mut s = format!("b{} {} {} {} {} {} {} {} {}\n", p.b.x0, p.b.y0, p.b.x1, p.b.y1, p.thick, p.edge.x0, p.edge.y0, p.edge.x1, p.edge.y1);
        for c in &p.cs { s += &line(c) } s
    }
    #[test] fn scan_reads_what_line_plot_writes() {
        let p = Plot { cs: vec![ch(1, &[0, 200, 255, 10, 32], "R1\0pin\0"), ch(2, &[], ""), ch(3, b"abc", "日本\0")], b: Rect { x0: 0, y0: 0, x1: 40000, y1: 30000 }, thick: 1600, solid: None, edge: Rect { x0: 1, y0: 2, x1: 3, y1: 4 } };
        assert_eq!(parse_child(&child(&p)), p);
        assert_eq!(parse_child("").cs.len(), 0); assert_eq!(parse_child("b1 2").cs.len(), 0);
        let cut = child(&p); let half: String = cut.chars().take(cut.chars().count() - 6).collect(); let half = half.as_str(); assert!(parse_child(half).cs.len() < 3);
    }
    #[test] fn scan_reads_a_solid() {
        let t = "m3 1 2 3 4 5 6 4 \u{0}\u{80}\u{ff}\u{41}\n";
        let p = parse_child(t); assert_eq!(p.solid, Some(Solid { n: 3, b: [1, 2, 3, 4, 5, 6], d: vec![0, 128, 255, 65] }));
    }
    // law plot_delta_exact: what the phone rebuilds from the delta is exactly the new plot, whatever it held
    #[test] fn plot_delta_exact() {
        let all: Vec<Chunk> = (0..6).map(|i| ch(i, &[i as u8; 3], "i")).collect();
        let sets: Vec<Vec<usize>> = vec![vec![], vec![0, 1, 2], vec![2, 1, 0], vec![0, 0, 3], vec![5], vec![4, 3, 2, 1, 0, 5]];
        for old in &sets { for new in &sets {
            let (o, n): (Vec<Chunk>, Vec<Chunk>) = (old.iter().map(|i| all[*i].clone()).collect(), new.iter().map(|i| all[*i].clone()).collect());
            assert_eq!(apply(&delta(&n, &o), &n, &o), n);
        } }
    }
    // the signature index answers what the scan over every held chunk answers, collisions and repeats included
    #[test] fn plot_delta_by_signature_is_the_scan() {
        let all: Vec<Chunk> = (0..40).map(|i| ch(i % 7, &[i as u8 % 5; 2], if i % 3 == 0 { "a" } else { "" })).collect();
        let held: Vec<Chunk> = all.iter().rev().cloned().chain(all[..10].iter().cloned()).collect();
        let scan = |ns: &[Chunk], h: &[Chunk]| -> Vec<Ref> { ns.iter().enumerate().map(|(i, n)| match h.iter().position(|o| same(n, o)) { Some(k) => Ref::Keep(k), None => Ref::Fresh(i) }).collect() };
        let fresh: Vec<Chunk> = (0..12).map(|i| ch(i, &[99, i as u8], "z")).collect();
        for ns in [all.clone(), fresh.clone(), [fresh.clone(), all.clone()].concat()] { assert_eq!(delta(&ns, &held), scan(&ns, &held)); assert_eq!(apply(&delta(&ns, &held), &ns, &held), ns) }
    }
    #[test] fn digest_tells_bytes_apart() {
        let mut bs = std::collections::BTreeSet::new();
        for n in 0..64usize { for v in [0u8, 1, 255] { let mut b = vec![7u8; n]; if n > 0 { b[n / 2] = v } bs.insert(b); } }
        let ds: std::collections::HashSet<(u64, u64)> = bs.iter().map(|b| digest(b)).collect();
        assert_eq!(ds.len(), bs.len()); assert_eq!(digest(b"abc"), digest(b"abc")); assert_ne!(digest(b"abc"), digest(b"abc\0")); assert_ne!(digest(&[0; 8]), digest(&[0; 9]));
    }
    // law plot_unchanged_sends_nothing
    #[test] fn plot_unchanged_sends_nothing() {
        let o: Vec<Chunk> = (0..4).map(|i| ch(i, &[i as u8], "")).collect();
        assert!(quiet(&delta(&o, &o), 4)); assert!(!quiet(&delta(&o[..3], &o), 4)); assert!(!quiet(&delta(&[o[1].clone(), o[0].clone()], &o[..2]), 2)); assert!(quiet(&delta(&[], &[]), 0));
        let mut changed = o.clone(); changed[2].color += 1; assert!(!quiet(&delta(&changed, &o), 4));
        let mut info = o.clone(); info[1].info = "x".into(); assert!(!quiet(&delta(&info, &o), 4)); // equality includes the info table
    }
    #[test] fn frame_bytes() {
        let p = Plot { cs: vec![ch(7, &[1, 2], "")], b: Rect { x0: 0, y0: 0, x1: 10, y1: u32::MAX - 1 }, thick: 1600, solid: None, edge: Rect::default() };
        let f = frame("0|/p|", "0|/p|", "", &p, &[Ref::Keep(3), Ref::Fresh(0)]);
        assert_eq!(&f[..9], &[0xa8, 0x01, 0x64, b'p', b'l', b'o', b't', 0x18, 0x1f]); // map(8), 1:"plot", 31:
        let s = String::from_utf8_lossy(&f).into_owned(); assert!(s.contains("fam") && s.contains("base") && s.contains("thick"));
        assert!(f.windows(3).any(|w| w == [0x41, 0x01, 0x02] || w == [0x42, 0x01, 0x02])); // d as a byte string
        let m = solid_frame("2|/p|", &Solid { n: 1, b: [1, 2, 3, 4, 5, 6], d: vec![9] }); assert_eq!(m[0], 0xa5);
    }
}
