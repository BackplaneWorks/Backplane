//! The terminal emulator: a port of src/core/vt (VT100/xterm subset: UTF-8, C0, ESC, CSI, SGR, OSC titles,
//! scroll regions, alternate screen, 2048-row scrollback, DSR/DA replies, keys and paste). Bytes in, styled rows out.
//! Bounded: the screen is at most MAX_COLS x MAX_ROWS, the scrollback is 2048 rows, an OSC string 4096 bytes, queued
//! replies MAX_REPLY bytes. The corpus in `tests` is test/vt_test line for line.
use std::collections::VecDeque;

pub const MAX_COLS: usize = 500;
pub const MAX_ROWS: usize = 200;
pub const SCROLLBACK: usize = 2048;
const OSC_MAX: usize = 4096;
const MAX_REPLY: usize = 4096;
pub const DEFAULT_FG: u32 = 14277092;
pub const DEFAULT_BG: u32 = 855826;

const PAL16: [u32; 16] = [1908774, 14707829, 10011513, 15057019, 6402031, 13007069, 5682882, 11252415, 6054768, 16743302, 11921552, 16766602, 8570111, 14392560, 8378080, 15790837];
fn lvl(v: u32) -> u32 { match v { 0 => 0, 1 => 95, 2 => 135, 3 => 175, 4 => 215, _ => 255 } }
/// Palette entry i as 0xRRGGBB (16 colors, the 6x6x6 cube, the gray ramp).
pub fn pal(i: u32) -> u32 {
    if i < 16 { PAL16[i as usize] } else if i < 232 { let j = i - 16; (lvl(j / 36) << 16) | (lvl((j / 6) % 6) << 8) | lvl(j % 6) } else { (8 + (i - 232) * 10) * 65793 }
}
const DEF: u32 = 256;
const TRUE: u32 = 1 << 24;
fn rgb(c: u32, d: u32) -> u32 { if c == DEF { d } else if c >= TRUE { c & 0xFFFFFF } else { pal(c & 255) } }
fn blend(f: u32, b: u32) -> u32 { ((f >> 1) & 8355711) + ((b >> 1) & 8355711) }

const A_BOLD: u8 = 1; const A_UL: u8 = 2; const A_INV: u8 = 4; const A_DIM: u8 = 8; const A_HID: u8 = 16;

/// ch 0 is the right half of a wide character.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Cell { ch: u32, fg: u32, bg: u32, at: u8 }
impl Cell { fn blank(bg: u32) -> Cell { Cell { ch: 32, fg: DEF, bg, at: 0 } } }

#[derive(Clone, Copy, PartialEq, Debug)]
struct Pen { fg: u32, bg: u32, at: u8 }
const PEN0: Pen = Pen { fg: DEF, bg: DEF, at: 0 };

#[derive(Clone, Copy, PartialEq, Debug)]
struct Cur { x: usize, y: usize, wrap: bool, pen: Pen, dec: bool }
const CUR0: Cur = Cur { x: 0, y: 0, wrap: false, pen: PEN0, dec: false };

#[derive(Clone, Copy)]
struct Modes { show: bool, awm: bool, appk: bool, paste: bool, org: bool, ins: bool }
const MODES0: Modes = Modes { show: true, awm: true, appk: false, paste: false, org: false, ins: false };

/// A run of cells with one style, colors resolved (0xRRGGBB).
#[derive(Clone, PartialEq, Debug)]
pub struct VRun { pub text: String, pub fg: u32, pub bg: u32, pub bold: bool, pub underline: bool }
#[derive(Clone, PartialEq, Debug, Default)]
pub struct VLine { pub runs: Vec<VRun> }
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VCur { pub x: usize, pub y: usize, pub visible: bool }

enum Par { G, U { cp: u32, owe: u8 }, E, I(u8), C { pv: u8, ps: Vec<u32>, cur: u32, inter: u8 }, O { acc: Vec<u8>, esc: bool }, S { esc: bool } }

pub struct Vt {
    cols: usize, rows: usize, top: usize, bot: usize,
    scr: Vec<Vec<Cell>>, main: Option<Vec<Vec<Cell>>>,
    cur: Cur, saved: Cur, md: Modes, sb: VecDeque<Vec<Cell>>, title: String, out: Vec<u8>, par: Par,
}

fn clampc(n: usize) -> usize { n.clamp(1, MAX_COLS) }
fn clampr(n: usize) -> usize { n.clamp(1, MAX_ROWS) }

impl Vt {
    pub fn new(cols: usize, rows: usize) -> Vt { Vt::with_sb(clampc(cols), clampr(rows), VecDeque::new()) }
    fn with_sb(cols: usize, rows: usize, sb: VecDeque<Vec<Cell>>) -> Vt {
        Vt { cols, rows, top: 0, bot: rows - 1, scr: vec![vec![Cell::blank(DEF); cols]; rows], main: None, cur: CUR0, saved: CUR0, md: MODES0, sb, title: String::new(), out: vec![], par: Par::G }
    }
    pub fn cols(&self) -> usize { self.cols }
    pub fn rows(&self) -> usize { self.rows }
    /// Rows in the scrollback (the most `lines` can scroll).
    pub fn history(&self) -> usize { self.sb.len() }
    pub fn alt(&self) -> bool { self.main.is_some() }
    pub fn title(&self) -> &str { &self.title }
    pub fn appk(&self) -> bool { self.md.appk }
    pub fn bpaste(&self) -> bool { self.md.paste }
    pub fn cursor(&self) -> VCur { VCur { x: self.cur.x, y: self.cur.y, visible: self.md.show } }
    /// Replies the program asked for (DSR, DA): write them to the pty, then `drain`.
    pub fn replies(&self) -> &[u8] { &self.out }
    pub fn drain(&mut self) { self.out.clear() }

    fn full(&self) -> bool { self.top == 0 && self.bot == self.rows - 1 }
    fn bcell(&self) -> Cell { Cell::blank(self.cur.pen.bg) }
    fn brow(&self) -> Vec<Cell> { vec![self.bcell(); self.cols] }
    fn emit(&mut self, b: &[u8]) { if self.out.len() + b.len() <= MAX_REPLY { self.out.extend_from_slice(b) } }
    fn push_sb(&mut self, row: Vec<Cell>) { if self.main.is_none() { if self.sb.len() >= SCROLLBACK { self.sb.pop_front(); } self.sb.push_back(row) } }

    // ---- cursor motion ----
    fn goto(&mut self, x: usize, y: usize) { self.cur.x = x.min(self.cols - 1); self.cur.y = y.min(self.rows - 1); self.cur.wrap = false }
    fn cr(&mut self) { self.goto(0, self.cur.y) }
    fn orow(&self, r: usize) -> usize { if self.md.org { (r + self.top).min(self.bot) } else { r } }
    fn home(&mut self) { let y = self.orow(0); self.goto(0, y) }
    fn up(&mut self, n: usize) { let y = self.cur.y; let lo = if y >= self.top { self.top } else { 0 }; self.goto(self.cur.x, y.saturating_sub(n).max(lo)) }
    fn down(&mut self, n: usize) { let y = self.cur.y; let hi = if y <= self.bot { self.bot } else { self.rows - 1 }; self.goto(self.cur.x, (y + n).min(hi)) }
    fn tab(&mut self, n: usize) { self.goto(((self.cur.x >> 3) + n) << 3, self.cur.y) }
    fn btab(&mut self, n: usize) { self.goto(((self.cur.x + 7) >> 3).saturating_sub(n) << 3, self.cur.y) }

    // ---- scrolling ----
    fn rot1(&mut self) { let blank = self.brow(); let gone = self.scr.remove(0); self.scr.push(blank); self.push_sb(gone) }
    fn scroll_up(&mut self, n: usize) {
        if self.full() { for _ in 0..n.min(self.rows) { self.rot1() } }
        else {
            let (top, bot) = (self.top, self.bot); let k = n.min(bot + 1 - top);
            let to_sb = top == 0 && self.main.is_none();
            let blank = self.brow();
            for _ in 0..k { let gone = self.scr.remove(top); self.scr.insert(bot, blank.clone()); if to_sb { self.push_sb(gone) } }
        }
    }
    fn scroll_down(&mut self, n: usize) { let (top, bot) = (self.top, self.bot); let blank = self.brow(); for _ in 0..n.min(bot + 1 - top) { self.scr.remove(bot); self.scr.insert(top, blank.clone()) } }
    fn nl(&mut self) { let y = self.cur.y; if y == self.bot { self.scroll_up(1) } else if y + 1 < self.rows { self.goto(self.cur.x, y + 1) } }
    fn ri(&mut self) { let y = self.cur.y; if y == self.top { self.scroll_down(1) } else if y > 0 { self.goto(self.cur.x, y - 1) } }
    /// IL (del false) / DL (del true) inside the region.
    fn il_dl(&mut self, n: usize, del: bool) {
        let y = self.cur.y; if y < self.top || y > self.bot { return }
        let bot = self.bot; let blank = self.brow();
        for _ in 0..n.min(bot + 1 - y) { if del { self.scr.remove(y); self.scr.insert(bot, blank.clone()) } else { self.scr.remove(bot); self.scr.insert(y, blank.clone()) } }
        self.goto(0, y)
    }

    // ---- erasing ----
    fn fill(&mut self, a: usize, b: usize) { let c = self.bcell(); let y = self.cur.y; for x in a..b.min(self.cols) { self.scr[y][x] = c } }
    fn el(&mut self, mode: u32) { let x = self.cur.x; match mode { 0 => self.fill(x, self.cols), 1 => self.fill(0, x + 1), _ => self.fill(0, self.cols) } }
    fn clear_rows(&mut self, from: usize, n: usize) { let b = self.brow(); for y in from..(from + n).min(self.rows) { self.scr[y] = b.clone() } }
    fn ed(&mut self, mode: u32) {
        let (x, y) = (self.cur.x, self.cur.y);
        match mode {
            0 => { self.fill(x, self.cols); self.clear_rows(y + 1, self.rows - 1 - y) }
            1 => { self.clear_rows(0, y); self.fill(0, x + 1) }
            2 => { self.clear_rows(0, self.rows) }
            3 => { self.sb.clear() }
            _ => {}
        }
    }

    // ---- printing ----
    fn print(&mut self, cp: u32) {
        let cp = if self.cur.dec && (95..=126).contains(&cp) { gfx(cp) } else { cp };
        let w = width(cp); if w == 0 { return }
        if self.md.awm && (self.cur.wrap || (w == 2 && self.cur.x + 1 >= self.cols)) { self.cr(); self.nl() }
        let (x, y, p) = (self.cur.x, self.cur.y, self.cur.pen);
        if self.md.ins { let row = &mut self.scr[y]; for _ in 0..w { row.insert(x, Cell::blank(p.bg)); row.pop(); } }
        let cell = |ch| Cell { ch, fg: p.fg, bg: p.bg, at: p.at };
        self.scr[y][x] = cell(cp);
        if w == 2 && x + 1 < self.cols { self.scr[y][x + 1] = cell(0) }
        // Cur.adv
        if x + w as usize >= self.cols { self.cur.x = self.cols - 1; self.cur.wrap = self.md.awm } else { self.cur.x = x + w as usize; self.cur.wrap = false }
    }

    // ---- escape sequences ----
    fn save(&mut self) { self.saved = self.cur }
    fn restore(&mut self) { let mut c = self.saved; c.x = c.x.min(self.cols - 1); c.y = c.y.min(self.rows - 1); self.cur = c }
    fn alt_on(&mut self) { if self.main.is_none() { let blank = vec![vec![Cell::blank(DEF); self.cols]; self.rows]; self.main = Some(std::mem::replace(&mut self.scr, blank)) } }
    fn alt_off(&mut self) { if let Some(m) = self.main.take() { self.scr = m } }
    fn dec(&mut self, n: u32, on: bool) {
        match n {
            1 => self.md.appk = on, 6 => { self.md.org = on; self.home() } 7 => self.md.awm = on, 25 => self.md.show = on,
            47 | 1047 => if on { self.alt_on() } else { self.alt_off() },
            1048 => if on { self.save() } else { self.restore() },
            1049 => if on { self.save(); self.alt_on() } else { self.alt_off(); self.restore() },
            2004 => self.md.paste = on,
            _ => {}
        }
    }
    fn sgr(&mut self, ps: &[u32]) {
        // a little machine, as vt's Sg: 0 plain, 1 after 38/48, 2 palette index, 3/4/5 r/g/b
        let (mut m, mut fg, mut r, mut g) = (0u8, true, 0u32, 0u32); let mut p = self.cur.pen;
        let color = |p: &mut Pen, fg: bool, c: u32| if fg { p.fg = c } else { p.bg = c };
        for &n in ps {
            match m {
                0 => match n {
                    0 => p = PEN0, 1 => p.at |= A_BOLD, 2 => p.at |= A_DIM, 4 | 21 => p.at |= A_UL, 7 => p.at |= A_INV, 8 => p.at |= A_HID,
                    22 => p.at &= !(A_BOLD | A_DIM), 24 => p.at &= !A_UL, 27 => p.at &= !A_INV, 28 => p.at &= !A_HID,
                    38 => { m = 1; fg = true } 48 => { m = 1; fg = false }
                    39 => p.fg = DEF, 49 => p.bg = DEF,
                    30..=37 => p.fg = n - 30, 40..=47 => p.bg = n - 40, 90..=97 => p.fg = n - 90 + 8, 100..=107 => p.bg = n - 100 + 8,
                    _ => {}
                },
                1 => m = if n == 5 { 2 } else if n == 2 { 3 } else { 0 },
                2 => { color(&mut p, fg, n & 255); m = 0 }
                3 => { r = n; m = 4 }
                4 => { g = n; m = 5 }
                _ => { color(&mut p, fg, TRUE | ((r & 255) << 16) | ((g & 255) << 8) | (n & 255)); m = 0 }
            }
        }
        self.cur.pen = p;
    }
    fn dsr(&mut self, k: u32) { match k { 5 => self.emit(b"\x1b[0n"), 6 => { let s = format!("\x1b[{};{}R", self.cur.y + 1, self.cur.x + 1); self.emit(s.as_bytes()) } _ => {} } }
    fn stbm(&mut self, ps: &[u32]) {
        let arg = |i: usize, d: u32| ps.get(i).copied().filter(|&v| v != 0).unwrap_or(d) as usize;
        let top = arg(0, 1) - 1; let bot = arg(1, self.rows as u32).min(self.rows) - 1;
        if top < bot { self.top = top; self.bot = bot; self.home() }
    }
    fn csi(&mut self, pv: u8, ps: &[u32], inter: u8, fin: u8) {
        if inter != 0 { return }
        let a0 = ps.first().copied().unwrap_or(0);
        let n = if a0 == 0 { 1 } else { a0 as usize };
        match pv {
            0 => match fin {
                b'@' => { let c = self.bcell(); let (x, y) = (self.cur.x, self.cur.y); for _ in 0..n.min(self.cols) { self.scr[y].insert(x, c); self.scr[y].pop(); } }
                b'A' => self.up(n), b'B' => self.down(n), b'C' | b'a' => self.goto(self.cur.x + n, self.cur.y), b'D' => self.goto(self.cur.x.saturating_sub(n), self.cur.y),
                b'E' => { self.down(n); self.cr() } b'F' => { self.up(n); self.cr() }
                b'G' | b'`' => self.goto(n - 1, self.cur.y),
                b'H' | b'f' => { let c = ps.get(1).copied().filter(|&v| v != 0).unwrap_or(1) as usize; let y = self.orow(n - 1); self.goto(c - 1, y) }
                b'I' => self.tab(n), b'J' => self.ed(a0), b'K' => self.el(a0),
                b'L' => self.il_dl(n, false), b'M' => self.il_dl(n, true),
                b'P' => { let c = self.bcell(); let (x, y) = (self.cur.x, self.cur.y); for _ in 0..n.min(self.cols) { if x < self.scr[y].len() { self.scr[y].remove(x); self.scr[y].push(c); } } }
                b'S' => self.scroll_up(n), b'T' => self.scroll_down(n),
                b'X' => { let x = self.cur.x; self.fill(x, x + n.min(self.cols)) }
                b'Z' => self.btab(n),
                b'c' => self.emit(b"\x1b[?1;2c"),
                b'd' => { let y = self.orow(n - 1); self.goto(self.cur.x, y) }
                b'e' => self.goto(self.cur.x, self.cur.y + n),
                b'h' | b'l' => for &v in ps { if v == 4 { self.md.ins = fin == b'h' } },
                b'm' => self.sgr(ps),
                b'n' => self.dsr(a0),
                b'r' => self.stbm(ps),
                b's' => self.save(), b'u' => self.restore(),
                _ => {}
            },
            b'?' => match fin { b'J' => self.ed(a0), b'K' => self.el(a0), b'h' => for &v in ps { self.dec(v, true) }, b'l' => for &v in ps { self.dec(v, false) }, _ => {} },
            b'>' => if fin == b'c' { self.emit(b"\x1b[>1;10;0c") },
            _ => {}
        }
    }
    fn esc(&mut self, inter: u8, fin: u8) {
        match inter {
            0 => match fin { b'7' => self.save(), b'8' => self.restore(), b'D' => self.nl(), b'E' => { self.cr(); self.nl() } b'M' => self.ri(), b'c' => self.reset(), _ => {} },
            b'(' => self.cur.dec = fin == b'0',
            _ => {}
        }
    }
    /// RIS: the scrollback stays.
    fn reset(&mut self) { let sb = std::mem::take(&mut self.sb); let (c, r) = (self.cols, self.rows); *self = Vt::with_sb(c, r, sb) }
    fn osc(&mut self, bs: &[u8]) {
        let s = bs.iter().position(|&b| b == b';'); let Some(s) = s else { return };
        if bs[..s].iter().all(u8::is_ascii_digit) && !bs[..s].is_empty() {
            let n: u32 = std::str::from_utf8(&bs[..s]).ok().and_then(|x| x.parse().ok()).unwrap_or(u32::MAX);
            if n == 0 || n == 2 { self.title = String::from_utf8_lossy(&bs[s + 1..]).into_owned() }
        }
    }
    fn ctl(&mut self, b: u8) { match b { 8 => self.goto(self.cur.x.saturating_sub(1), self.cur.y), 9 => self.tab(1), 10 | 11 | 12 => self.nl(), 13 => self.cr(), _ => {} } }

    /// Feed bytes, any chunking.
    pub fn feed(&mut self, bytes: &[u8]) { for &b in bytes { self.byte(b) } }
    fn byte(&mut self, b: u8) {
        let par = std::mem::replace(&mut self.par, Par::G);
        match par {
            Par::G => self.ground(b),
            Par::U { cp, owe } => {
                if (0x80..0xC0).contains(&b) { let cp = (cp << 6) | (b as u32 & 63); if owe == 1 { self.print(cp) } else { self.par = Par::U { cp, owe: owe - 1 } } }
                else { self.print(0xFFFD); self.ground(b) }
            }
            Par::E => self.after_esc(b),
            Par::I(i) => match b {
                0x00..=0x1f => self.c0(b, Par::I(i)),
                0x20..=0x2f => self.par = Par::I(b),
                0x30..=0x7e => self.esc(i, b),
                _ => {}
            },
            Par::C { pv, mut ps, cur, inter } => match b {
                0x00..=0x1f => self.c0(b, Par::C { pv, ps, cur, inter }),
                0x20..=0x2f => self.par = Par::C { pv, ps, cur, inter: b },
                b'0'..=b'9' => self.par = Par::C { pv, ps, cur: ((cur * 10) + (b - b'0') as u32).min(65535), inter },
                b':' | b';' => { if ps.len() < 32 { ps.push(cur) } self.par = Par::C { pv, ps, cur: 0, inter } }
                b'<'..=b'?' => self.par = Par::C { pv: b, ps, cur, inter },
                0x40..=0x7e => { if ps.len() < 32 { ps.push(cur) } self.csi(pv, &ps, inter, b) }
                _ => self.par = Par::C { pv, ps, cur, inter },
            },
            Par::O { mut acc, esc } => {
                if esc { if b == b'\\' { self.osc(&acc) } else { self.after_esc(b) } }
                else { match b { 7 => self.osc(&acc), 27 => self.par = Par::O { acc, esc: true }, 24 | 26 => {} _ => { if acc.len() < OSC_MAX { acc.push(b) } self.par = Par::O { acc, esc: false } } } }
            }
            Par::S { esc } => match (esc, b) { (true, b'\\') | (_, 24) | (_, 26) => {} (_, 27) => self.par = Par::S { esc: true }, _ => self.par = Par::S { esc: false } },
        }
    }
    fn c0(&mut self, b: u8, stay: Par) { match b { 27 => self.par = Par::E, 24 | 26 => {} _ => { self.par = stay; self.ctl(b) } } }
    fn ground(&mut self, b: u8) {
        match b {
            0x00..=0x1f => self.c0(b, Par::G),
            0x7f => {}
            0x20..=0x7e => self.print(b as u32),
            0x80..=0xBF => self.print(0xFFFD),
            0xC0..=0xDF => self.par = Par::U { cp: (b & 31) as u32, owe: 1 },
            0xE0..=0xEF => self.par = Par::U { cp: (b & 15) as u32, owe: 2 },
            0xF0..=0xF7 => self.par = Par::U { cp: (b & 7) as u32, owe: 3 },
            _ => self.print(0xFFFD),
        }
    }
    fn after_esc(&mut self, b: u8) {
        match b {
            0x00..=0x1f => self.c0(b, Par::E),
            0x20..=0x2f => self.par = Par::I(b),
            b'[' => self.par = Par::C { pv: 0, ps: vec![], cur: 0, inter: 0 },
            b']' => self.par = Par::O { acc: vec![], esc: false },
            b'P' | b'X' | b'^' | b'_' => self.par = Par::S { esc: false },
            0x30..=0x7e => self.esc(0, b),
            _ => {}
        }
    }

    /// Resize: rows cut or padded on the right (no reflow); a screen shorter than the cursor's row gives its top rows to the scrollback.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let (cols, rows) = (clampc(cols), clampr(rows));
        let k = (self.cur.y + 1).saturating_sub(rows);
        let refit = |s: Vec<Vec<Cell>>, k: usize| -> Vec<Vec<Cell>> {
            let mut v: Vec<Vec<Cell>> = s.into_iter().skip(k).map(|mut r| { r.resize(cols, Cell::blank(DEF)); r }).collect(); v.resize(rows, vec![Cell::blank(DEF); cols]); v
        };
        let old = std::mem::take(&mut self.scr);
        let gone: Vec<Vec<Cell>> = old.iter().take(k).cloned().collect();
        if self.main.is_none() { for r in gone { self.push_sb(r) } }
        self.scr = refit(old, k);
        if let Some(m) = self.main.take() { self.main = Some(refit(m, 0)) }
        self.cols = cols; self.rows = rows; self.top = 0; self.bot = rows - 1;
        self.cur.y -= k.min(self.cur.y); self.cur.x = self.cur.x.min(cols - 1); self.cur.y = self.cur.y.min(rows - 1);
        self.saved.x = self.saved.x.min(cols - 1); self.saved.y = self.saved.y.min(rows - 1);
    }

    /// Exactly `rows` lines; with `scroll > 0` the top ones come from the scrollback. Trailing blank cells are left out.
    pub fn lines(&self, scroll: usize) -> Vec<VLine> {
        let scroll = scroll.min(self.sb.len());
        let total = self.sb.len() + self.rows; let end = total - scroll; let start = end - self.rows;
        (start..end).map(|i| { let row = if i < self.sb.len() { &self.sb[i] } else { &self.scr[i - self.sb.len()] }; line_of(row) }).collect()
    }

    // ---- keys ----
    /// Bytes for a key press: an X11 keysym, the text it typed (a code point, 0 for none), mods (bit 0 shift, 2 ctrl, 3 alt).
    pub fn key(&self, keysym: u32, text: u32, mods: u32) -> Vec<u8> {
        let (ctrl, alt, shift) = (mods & 4 != 0, mods & 8 != 0, mods & 1 != 0);
        let meta = |mut v: Vec<u8>| { if alt { v.insert(0, 27) } v };
        if keysym >= 65280 {
            let m = 1 + (mods & 1) + ((mods & 8) >> 2) + (mods & 4);
            let csik = |l: u8, app: bool| -> Vec<u8> { if m == 1 { vec![27, if app { b'O' } else { b'[' }, l] } else { format!("\x1b[1;{}{}", m, l as char).into_bytes() } };
            let tilde = |n: u32| -> Vec<u8> { if m == 1 { format!("\x1b[{n}~") } else { format!("\x1b[{n};{m}~") }.into_bytes() };
            let a = self.md.appk;
            match keysym - 65280 {
                8 => return meta(vec![if ctrl { 8 } else { 127 }]),
                9 => return if shift { vec![27, 91, 90] } else { meta(vec![9]) },
                13 => return meta(vec![13]), 27 => return vec![27],
                80 | 149 => return csik(b'H', a), 81 | 150 => return csik(b'D', a), 82 | 151 => return csik(b'A', a), 83 | 152 => return csik(b'C', a), 84 | 153 => return csik(b'B', a),
                85 | 154 => return tilde(5), 86 | 155 => return tilde(6), 87 | 156 => return csik(b'F', a), 99 | 158 => return tilde(2), 159 | 255 => return tilde(3),
                141 => return vec![13],
                190 => return csik(b'P', true), 191 => return csik(b'Q', true), 192 => return csik(b'R', true), 193 => return csik(b'S', true),
                194 => return tilde(15), 195 => return tilde(17), 196 => return tilde(18), 197 => return tilde(19), 198 => return tilde(20), 199 => return tilde(21), 200 => return tilde(23), 201 => return tilde(24),
                _ => {}
            }
        } else if keysym == 65056 { return vec![27, 91, 90] }
        let cp = if text != 0 { text } else if (32..=255).contains(&keysym) { keysym } else if keysym >= 16777216 { keysym - 16777216 } else { 0 };
        if cp == 0 { return vec![] }
        let cp = if ctrl { ctrl_cp(cp) } else { cp };
        meta(utf8(cp))
    }
    /// Bytes for pasted text: LF and CR LF become CR; bracketed when the program asked (?2004), ESC dropped then.
    pub fn paste(&self, text: &str) -> Vec<u8> {
        let br = self.md.paste; let b = text.as_bytes(); let mut o = vec![];
        for (i, &c) in b.iter().enumerate() {
            if c == 13 && b.get(i + 1) == Some(&10) { continue }
            if c == 10 { o.push(13) } else if c == 27 && br { } else { o.push(c) }
        }
        if br { let mut w = b"\x1b[200~".to_vec(); w.extend(o); w.extend_from_slice(b"\x1b[201~"); w } else { o }
    }
}

fn ctrl_cp(cp: u32) -> u32 {
    if (97..=122).contains(&cp) { cp - 96 } else if (65..=90).contains(&cp) { cp - 64 } else if cp < 128 {
        match cp { 32 | 64 | 50 => 0, 91 | 51 => 27, 92 | 52 => 28, 93 | 53 => 29, 94 | 54 => 30, 95 | 55 | 47 => 31, 63 | 56 => 127, _ => cp }
    } else { cp }
}
pub fn utf8(cp: u32) -> Vec<u8> { match char::from_u32(cp) { Some(c) => { let mut b = [0u8; 4]; c.encode_utf8(&mut b).as_bytes().to_vec() } None => if cp == 0 { vec![0] } else { "\u{FFFD}".as_bytes().to_vec() } } }

fn zero(cp: u32) -> bool { let i = |a, b| (a..=b).contains(&cp); i(768, 879) || i(8203, 8207) || i(8400, 8447) || i(65024, 65039) || i(65056, 65071) || i(6832, 6911) || i(7616, 7679) }
fn wide(cp: u32) -> bool {
    let i = |a, b| (a..=b).contains(&cp);
    i(4352, 4447) || i(11904, 12350) || i(12353, 13311) || i(13312, 19903) || i(19968, 40959) || i(40960, 42191) || i(44032, 55203) || i(63744, 64255) || i(65072, 65103) || i(65280, 65376) || i(65504, 65510) || i(127744, 128591) || i(128640, 128767) || i(129280, 129535) || i(131072, 262141)
}
/// The cells a code point takes: 0 (combining), 1, or 2 (wide).
pub fn width(cp: u32) -> u32 { if cp < 768 { 1 } else if zero(cp) { 0 } else if wide(cp) { 2 } else { 1 } }
/// The DEC special graphics set (ESC ( 0).
fn gfx(cp: u32) -> u32 {
    match cp { 95 => 32, 96 => 9670, 97 => 9618, 102 => 176, 103 => 177, 106 => 9496, 107 => 9488, 108 => 9484, 109 => 9492, 110 => 9532, 111 => 9146, 112 => 9147, 113 => 9472, 114 => 9148, 115 => 9149, 116 => 9500, 117 => 9508, 118 => 9524, 119 => 9516, 120 => 9474, 121 => 8804, 122 => 8805, 123 => 960, 124 => 8800, 125 => 163, 126 => 183, _ => cp }
}

fn resolve(c: &Cell) -> (u32, u32, bool, bool) {
    let (f0, b0) = (rgb(c.fg, DEFAULT_FG), rgb(c.bg, DEFAULT_BG));
    let inv = c.at & A_INV != 0;
    let (f1, b1) = if inv { (b0, f0) } else { (f0, b0) };
    let f2 = if c.at & A_DIM != 0 { blend(f1, b1) } else { f1 };
    (if c.at & A_HID != 0 { b1 } else { f2 }, b1, c.at & A_BOLD != 0, c.at & A_UL != 0)
}
fn line_of(row: &[Cell]) -> VLine {
    let last = row.iter().rposition(|c| !(c.ch == 32 && c.bg == DEF && c.at & (A_UL | A_INV) == 0));
    let mut runs: Vec<VRun> = vec![];
    if let Some(last) = last {
        for c in &row[..=last] {
            if c.ch == 0 { continue }
            let (fg, bg, bold, underline) = resolve(c);
            let ch = char::from_u32(c.ch).unwrap_or('\u{FFFD}');
            match runs.last_mut() { Some(r) if r.fg == fg && r.bg == bg && r.bold == bold && r.underline == underline => r.text.push(ch), _ => runs.push(VRun { text: ch.to_string(), fg, bg, bold, underline }) }
        }
    }
    VLine { runs }
}

#[cfg(test)]
mod tests;
