//! A bot's cat, as one pure description every client shares: the hub uses it, and the desktop client includes this very file
//! (native/gpui/src/main.rs `#[path]`), so a name gives the same cat on both. Phones get rigs already tinted from the hub (`cat.rig`).
//!
//! A look is one of 72: `coat + 12 * collar` (assets/cat-rigs/<look>-<mood>.json). The coat is the body pattern and markings (ginger tabby, black,
//! grey with a white chest, cream, white, brown, calico, tuxedo, points, silver tabby, brown tabby, ginger with white); the collar one of six colours.
//! A bot's look comes from its name (`look_of`) unless the person picked one (Different cat): the pick is stored, so a later change to the hash
//! never changes a chosen cat. The colour is the fur's: `tint` moves every fur, marking and outline colour to the hue picked, keeping each one's
//! lightness against the body, so stripes stay stripes and a white chest stays light. Eyes, nose, ear insides, collar and bell keep theirs.
//! Std only: this file compiles in both crates.

pub const COATS: u64 = 12;
pub const COLLARS: u64 = 6;
pub const LOOKS: u64 = COATS * COLLARS;
/// The collars' colours by `look / 12` (the rigs' own).
pub const COLLAR: [u32; 6] = [0xe5484d, 0x3e8ef7, 0x2bb5a0, 0x9b6bf2, 0xf06fa8, 0x5cc45c];
/// The same collars greyed, as the away mood's rigs have them.
pub const AWAY_COLLAR: [u32; 6] = [0x585860, 0x5c5c64, 0x5f5f67, 0x5f5f67, 0x66666e, 0x65656d];
/// A colour as the away mood greys it (fitted to the rigs' own: within 1 of every grey they have): its luma, dimmed, a little blue.
pub fn grey(c: u32) -> u32 {
    let l = 0.299 * ((c >> 16) & 255) as f64 + 0.587 * ((c >> 8) & 255) as f64 + 0.114 * (c & 255) as f64;
    let g = (0.3769 * l + 43.26).round().clamp(0., 247.) as u32; (g << 16) | (g << 8) | (g + 8)
}

/// The name's hash (the bots' seed since the first bots: a bot made before this file keeps the cat it had): lower case, then
/// `acc * 31 + char` from 7, mod the prime 4294967291. Stable across platforms (u64 arithmetic over Unicode scalar values).
pub fn seed(name: &str) -> u64 { name.to_lowercase().chars().fold(7u64, |acc, c| (acc * 31 + c as u64) % 4294967291) }
/// The look a name gives: leading and trailing spaces do not count, case does not either.
pub fn look_of(name: &str) -> u64 { seed(name.trim()) % LOOKS }
pub fn coat(look: u64) -> u64 { look % COATS }
pub fn collar(look: u64) -> u64 { (look / COATS) % COLLARS }
/// Different cat: the next coat, the same collar (twelve presses go round to the start).
pub fn shuffled(look: u64) -> u64 { (coat(look) + 1) % COATS + COATS * collar(look) }

/// The colours offered beside Custom: the coats' own five and five brighter ones (name, 0xRRGGBB).
pub const PRESETS: [(&str, u32); 10] = [
    ("Ginger", 0xf2a65a), ("Cream", 0xf6ddb5), ("Smoke", 0x8e9db5), ("Midnight", 0x2f2c3a), ("Cocoa", 0x7a5238),
    ("Sky", 0x6fb3f2), ("Lilac", 0xa98bf0), ("Rose", 0xf29ab8), ("Mint", 0x6fd3a0), ("Cherry", 0xe5484d),
];
/// "#rrggbb" or "rrggbb" (any case, spaces around) as 0xRRGGBB; None for anything else, "" included.
pub fn parse_color(s: &str) -> Option<u32> {
    let h = s.trim(); let h = h.strip_prefix('#').unwrap_or(h);
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) { return None }
    u32::from_str_radix(h, 16).ok()
}
/// 0xRRGGBB as "#rrggbb", the form a bot's colour is kept in.
pub fn color_hex(c: u32) -> String { format!("#{:06x}", c & 0xffffff) }

/// Fur and markings: their fill follows the colour.
const FUR: [&str; 17] = ["tail", "tail tip", "body", "chest", "stripes", "paws", "left ear", "right ear", "head", "head stripes", "patch", "spot", "patch 2", "mask", "bridge", "blaze", "muzzle"];
/// Outlines: their stroke becomes the colour's own outline.
const LINES: [&str; 8] = ["tail line", "body line", "paws", "toes", "left ear", "right ear", "head line", "mouth"];
pub fn is_fur(part: &str) -> bool { FUR.contains(&part) }
pub fn is_line(part: &str) -> bool { LINES.contains(&part) }

fn hls(c: u32) -> (f64, f64, f64) {
    let (r, g, b) = (((c >> 16) & 255) as f64 / 255., ((c >> 8) & 255) as f64 / 255., (c & 255) as f64 / 255.);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.;
    if mx == mn { return (0., l, 0.) }
    let d = mx - mn;
    let s = if l <= 0.5 { d / (mx + mn) } else { d / (2. - mx - mn) };
    let (rc, gc, bc) = ((mx - r) / d, (mx - g) / d, (mx - b) / d);
    let h = if r == mx { bc - gc } else if g == mx { 2. + rc - bc } else { 4. + gc - rc };
    ((h / 6.).rem_euclid(1.), l, s)
}
fn from_hls(h: f64, l: f64, s: f64) -> u32 {
    let ch = |x: f64| ((x.clamp(0., 1.) * 255.).round() as u32) & 255;
    if s == 0. { let v = ch(l); return (v << 16) | (v << 8) | v }
    let m2 = if l <= 0.5 { l * (1. + s) } else { l + s - l * s };
    let m1 = 2. * l - m2;
    let v = |hue: f64| { let hue = hue.rem_euclid(1.); if hue < 1. / 6. { m1 + (m2 - m1) * hue * 6. } else if hue < 0.5 { m2 } else if hue < 2. / 3. { m1 + (m2 - m1) * (2. / 3. - hue) * 6. } else { m1 } };
    (ch(v(h + 1. / 3.)) << 16) | (ch(v(h)) << 8) | ch(v(h - 1. / 3.))
}
/// A fur colour `x` of a coat whose body is `body`, moved to the colour `c`: the body's own colour becomes `c` exactly; another keeps its
/// lightness against the body (a darker stripe stays darker) in `c`'s hue; a near-grey one (white chest, black spot) takes a little of it.
pub fn fur(x: u32, body: u32, c: u32) -> u32 {
    if x == body { return c & 0xffffff }
    let (_, lx, sx) = hls(x); let (_, lb, _) = hls(body); let (hc, lc, sc) = hls(c);
    from_hls(hc, (lc + (lx - lb)).clamp(0.08, 0.96), if sx < 0.12 { sc * 0.25 } else { sc })
}
/// The outline that goes with the colour `c`: darker on a light colour, lighter on a dark one.
pub fn line(c: u32) -> u32 { let (h, l, s) = hls(c); from_hls(h, if l > 0.5 { l - 0.42 } else { l + 0.32 }, s * 0.6) }
/// One part of a rig, tinted: (fill, stroke) after the colour `c` on a coat whose body is `body`. `None` for `c`: unchanged.
pub fn tint(part: &str, fill: Option<u32>, stroke: Option<u32>, body: u32, c: Option<u32>) -> (Option<u32>, Option<u32>) {
    let Some(c) = c else { return (fill, stroke) };
    (if is_fur(part) { fill.map(|f| fur(f, body, c)) } else { fill }, if is_line(part) { stroke.map(|_| line(c)) } else { stroke })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_name_gives_one_look_whatever_its_case_and_spaces() {
        assert_eq!(look_of("Bub"), look_of(" bub ")); assert_eq!(look_of("Bub"), seed("bub") % 72);
        // pinned: a change here changes every unpicked cat; the clients' copies and the phones' answers rest on these
        assert_eq!([look_of("Bub"), look_of("Pebble"), look_of("Scout"), look_of("Ada"), look_of("猫")], [8, 39, 7, 31, 36]);
        assert!(look_of("") < LOOKS);
    }
    #[test] fn looks_split_into_coat_and_collar_and_different_cat_keeps_the_collar() {
        assert_eq!((coat(8), collar(8)), (8, 0)); assert_eq!((coat(39), collar(39)), (3, 3));
        let mut l = 39; let mut seen = vec![];
        for _ in 0..12 { l = shuffled(l); assert_eq!(collar(l), 3); seen.push(coat(l)); }
        seen.sort(); assert_eq!(seen, (0..12).collect::<Vec<_>>(), "twelve presses show every coat once"); assert_eq!(l, 39);
    }
    #[test] fn colours_parse_and_print() {
        assert_eq!(parse_color("#6FB3F2"), Some(0x6fb3f2)); assert_eq!(parse_color(" 6fb3f2 "), Some(0x6fb3f2));
        for bad in ["", "#fff", "#12345g", "1234567"] { assert_eq!(parse_color(bad), None, "{bad}"); }
        assert_eq!(color_hex(0x0a0b0c), "#0a0b0c");
    }
    #[test] fn the_tint_keeps_the_pattern_in_the_picked_colour() {
        let (body, stripe, chest) = (0xf2a65a, 0xd2712a, 0xffe7c7); // the ginger tabby
        assert_eq!(fur(body, body, 0x6fb3f2), 0x6fb3f2, "the body is the colour picked");
        let (_, ls, _) = hls(fur(stripe, body, 0x6fb3f2)); let (_, lc, _) = hls(fur(chest, body, 0x6fb3f2)); let (_, lb, _) = hls(0x6fb3f2);
        assert!(ls < lb && lc > lb, "stripes darker, the chest lighter");
        assert!(hls(line(0xf6ddb5)).1 < 0.5 && hls(line(0x2f2c3a)).1 > 0.4, "outlines contrast with the fur");
        assert_eq!(tint("left eye", Some(0x8fd16a), None, body, Some(0x6fb3f2)), (Some(0x8fd16a), None), "eyes keep theirs");
        assert_eq!(tint("collar", Some(0xe5484d), None, body, Some(0x6fb3f2)).0, Some(0xe5484d));
        assert_eq!(tint("body", Some(body), None, body, None), (Some(body), None), "no colour, no change");
        // pinned values: the desktop client draws with this same file, and phones get the hub's answer
        assert_eq!([fur(stripe, body, 0x6fb3f2), fur(chest, body, 0x6fb3f2), line(0x6fb3f2)], [0x268ceb, 0xdfeefc, 0x234768]);
        assert!([(0xf2a65a, 0x6f6f77), (0xe5484d, 0x585860), (0xffe7c7, 0x84848c)].iter().all(|&(i, a)| (grey(i) >> 16).abs_diff(a >> 16) <= 1 && grey(i) & 255 == (grey(i) >> 16) + 8), "away's grey");
        for x in [0u32, 0xffffff, 0x123456, 0xf2a65a] { let (h, l, s) = hls(x); assert_eq!(from_hls(h, l, s), x, "{x:06x} round trips"); }
    }
}
