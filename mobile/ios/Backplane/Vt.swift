import Foundation

// The terminal emulator, a port of the hub's native/host/src/term/vt.rs (the desktop uses the same rules): VT100/xterm subset, UTF-8, C0, ESC, CSI, SGR,
// OSC titles, scroll regions, alternate screen, 2048-row scrollback, DSR/DA replies, keys and paste. Bytes in, styled rows out. Bounded like the original:
// the screen is at most MAX_COLS x MAX_ROWS, an OSC string 4096 bytes, queued replies 4096 bytes. Pure: no UIKit here (tests: Tests/VtTests.swift, the
// cases of vt/tests.rs). Keep it line for line with vt.rs when either changes.

let vtMaxCols = 500
let vtMaxRows = 200
let vtScrollback = 2048
private let oscMax = 4096
private let maxReply = 4096
let vtDefaultFg: UInt32 = 14277092
let vtDefaultBg: UInt32 = 855826

private let pal16: [UInt32] = [1908774, 14707829, 10011513, 15057019, 6402031, 13007069, 5682882, 11252415, 6054768, 16743302, 11921552, 16766602, 8570111, 14392560, 8378080, 15790837]
private func lvl(_ v: UInt32) -> UInt32 { switch v { case 0: return 0; case 1: return 95; case 2: return 135; case 3: return 175; case 4: return 215; default: return 255 } }
/// Palette entry i as 0xRRGGBB (16 colors, the 6x6x6 cube, the gray ramp).
func vtPal(_ i: UInt32) -> UInt32 {
    if i < 16 { return pal16[Int(i)] }
    if i < 232 { let j = i - 16; return (lvl(j / 36) << 16) | (lvl((j / 6) % 6) << 8) | lvl(j % 6) }
    return (8 + (i - 232) * 10) * 65793
}
private let DEF: UInt32 = 256
private let TRUE: UInt32 = 1 << 24
private func rgb(_ c: UInt32, _ d: UInt32) -> UInt32 { c == DEF ? d : c >= TRUE ? c & 0xFFFFFF : vtPal(c & 255) }
private func blend(_ f: UInt32, _ b: UInt32) -> UInt32 { ((f >> 1) & 8355711) + ((b >> 1) & 8355711) }

private let A_BOLD: UInt8 = 1, A_UL: UInt8 = 2, A_INV: UInt8 = 4, A_DIM: UInt8 = 8, A_HID: UInt8 = 16

/// ch 0 is the right half of a wide character.
private struct Cell: Equatable {
    var ch: UInt32, fg: UInt32, bg: UInt32, at: UInt8
    static func blank(_ bg: UInt32) -> Cell { Cell(ch: 32, fg: DEF, bg: bg, at: 0) }
}
private struct Pen: Equatable { var fg: UInt32, bg: UInt32, at: UInt8 }
private let PEN0 = Pen(fg: DEF, bg: DEF, at: 0)
private struct Cur: Equatable { var x: Int, y: Int, wrap: Bool, pen: Pen, dec: Bool }
private let CUR0 = Cur(x: 0, y: 0, wrap: false, pen: PEN0, dec: false)
private struct Modes { var show = true, awm = true, appk = false, paste = false, org = false, ins = false }

/// A run of cells with one style, colors resolved (0xRRGGBB).
struct VRun: Equatable { var text: String; var fg: UInt32; var bg: UInt32; var bold: Bool; var underline: Bool }
struct VLine: Equatable { var runs: [VRun] = [] }
struct VCur: Equatable { var x: Int; var y: Int; var visible: Bool }

private enum Par {
    case g
    case u(cp: UInt32, owe: UInt8)
    case e
    case i(UInt8)
    case c(pv: UInt8, ps: [UInt32], cur: UInt32, inter: UInt8)
    case o(acc: [UInt8], esc: Bool)
    case s(esc: Bool)
}

private func clampc(_ n: Int) -> Int { min(max(n, 1), vtMaxCols) }
private func clampr(_ n: Int) -> Int { min(max(n, 1), vtMaxRows) }
private func sat(_ a: Int, _ b: Int) -> Int { max(a - b, 0) }

final class Vt {
    private(set) var cols: Int
    private(set) var rows: Int
    private var top = 0, bot: Int
    private var scr: [[Cell]]
    private var main: [[Cell]]?
    private var cur = CUR0, saved = CUR0, md = Modes()
    private var sb: [[Cell]] = []    // oldest first; trimmed from the front
    private(set) var title = ""
    private var out: [UInt8] = []
    private var par: Par = .g

    init(cols: Int, rows: Int) {
        let c = clampc(cols), r = clampr(rows)
        self.cols = c; self.rows = r; bot = r - 1
        scr = Array(repeating: Array(repeating: Cell.blank(DEF), count: c), count: r)
    }
    /// A blank screen of this size that keeps `from`'s scrollback (a snapshot at a new size: the history stays).
    convenience init(cols: Int, rows: Int, keepingHistoryOf from: Vt) { self.init(cols: cols, rows: rows); sb = from.sb }

    /// Rows in the scrollback (the most `lines` can scroll).
    var history: Int { sb.count }
    var alt: Bool { main != nil }
    var appk: Bool { md.appk }
    var bpaste: Bool { md.paste }
    var cursor: VCur { VCur(x: cur.x, y: cur.y, visible: md.show) }
    /// Replies the program asked for (DSR, DA): write them to the pty, then `drain`. (The hub's own emulator answers them; a phone never writes these.)
    var replies: [UInt8] { out }
    func drain() { out.removeAll() }

    private var full: Bool { top == 0 && bot == rows - 1 }
    private func bcell() -> Cell { Cell.blank(cur.pen.bg) }
    private func brow() -> [Cell] { Array(repeating: bcell(), count: cols) }
    private func emit(_ b: [UInt8]) { if out.count + b.count <= maxReply { out.append(contentsOf: b) } }
    private func emit(_ s: String) { emit(Array(s.utf8)) }
    private func pushSb(_ row: [Cell]) { if main == nil { if sb.count >= vtScrollback { sb.removeFirst() }; sb.append(row) } }

    // ---- cursor motion ----
    private func goto(_ x: Int, _ y: Int) { cur.x = min(x, cols - 1); cur.y = min(y, rows - 1); cur.wrap = false }
    private func cr() { goto(0, cur.y) }
    private func orow(_ r: Int) -> Int { md.org ? min(r + top, bot) : r }
    private func home() { goto(0, orow(0)) }
    private func up(_ n: Int) { let y = cur.y; let lo = y >= top ? top : 0; goto(cur.x, max(sat(y, n), lo)) }
    private func down(_ n: Int) { let y = cur.y; let hi = y <= bot ? bot : rows - 1; goto(cur.x, min(y + n, hi)) }
    private func tab(_ n: Int) { goto(((cur.x >> 3) + n) << 3, cur.y) }
    private func btab(_ n: Int) { goto(sat((cur.x + 7) >> 3, n) << 3, cur.y) }

    // ---- scrolling ----
    private func rot1() { let blank = brow(); let gone = scr.remove(at: 0); scr.append(blank); pushSb(gone) }
    private func scrollUp(_ n: Int) {
        if full { for _ in 0..<min(n, rows) { rot1() } }
        else {
            let (t, b) = (top, bot); let k = min(n, b + 1 - t)
            let toSb = t == 0 && main == nil
            let blank = brow()
            for _ in 0..<k { let gone = scr.remove(at: t); scr.insert(blank, at: b); if toSb { pushSb(gone) } }
        }
    }
    private func scrollDown(_ n: Int) { let (t, b) = (top, bot); let blank = brow(); for _ in 0..<min(n, b + 1 - t) { scr.remove(at: b); scr.insert(blank, at: t) } }
    private func nl() { let y = cur.y; if y == bot { scrollUp(1) } else if y + 1 < rows { goto(cur.x, y + 1) } }
    private func ri() { let y = cur.y; if y == top { scrollDown(1) } else if y > 0 { goto(cur.x, y - 1) } }
    /// IL (del false) / DL (del true) inside the region.
    private func ilDl(_ n: Int, _ del: Bool) {
        let y = cur.y; if y < top || y > bot { return }
        let b = bot; let blank = brow()
        for _ in 0..<min(n, b + 1 - y) { if del { scr.remove(at: y); scr.insert(blank, at: b) } else { scr.remove(at: b); scr.insert(blank, at: y) } }
        goto(0, y)
    }

    // ---- erasing ----
    private func fill(_ a: Int, _ b: Int) { let c = bcell(); let y = cur.y; var x = a; while x < min(b, cols) { scr[y][x] = c; x += 1 } }
    private func el(_ mode: UInt32) { let x = cur.x; switch mode { case 0: fill(x, cols); case 1: fill(0, x + 1); default: fill(0, cols) } }
    private func clearRows(_ from: Int, _ n: Int) { let b = brow(); var y = from; while y < min(from + n, rows) { scr[y] = b; y += 1 } }
    private func ed(_ mode: UInt32) {
        let (x, y) = (cur.x, cur.y)
        switch mode {
        case 0: fill(x, cols); clearRows(y + 1, rows - 1 - y)
        case 1: clearRows(0, y); fill(0, x + 1)
        case 2: clearRows(0, rows)
        case 3: sb.removeAll()
        default: break
        }
    }

    // ---- printing ----
    private func print(_ cp0: UInt32) {
        let cp = cur.dec && (95...126).contains(cp0) ? gfx(cp0) : cp0
        let w = vtWidth(cp); if w == 0 { return }
        if md.awm && (cur.wrap || (w == 2 && cur.x + 1 >= cols)) { cr(); nl() }
        let (x, y, p) = (cur.x, cur.y, cur.pen)
        if md.ins { for _ in 0..<w { scr[y].insert(Cell.blank(p.bg), at: x); scr[y].removeLast() } }
        let cell = { (ch: UInt32) in Cell(ch: ch, fg: p.fg, bg: p.bg, at: p.at) }
        scr[y][x] = cell(cp)
        if w == 2 && x + 1 < cols { scr[y][x + 1] = cell(0) }
        if x + w >= cols { cur.x = cols - 1; cur.wrap = md.awm } else { cur.x = x + w; cur.wrap = false }
    }

    // ---- escape sequences ----
    private func save() { saved = cur }
    private func restore() { var c = saved; c.x = min(c.x, cols - 1); c.y = min(c.y, rows - 1); cur = c }
    private func altOn() { if main == nil { let blank = Array(repeating: Array(repeating: Cell.blank(DEF), count: cols), count: rows); main = scr; scr = blank } }
    private func altOff() { if let m = main { main = nil; scr = m } }
    private func dec(_ n: UInt32, _ on: Bool) {
        switch n {
        case 1: md.appk = on
        case 6: md.org = on; home()
        case 7: md.awm = on
        case 25: md.show = on
        case 47, 1047: if on { altOn() } else { altOff() }
        case 1048: if on { save() } else { restore() }
        case 1049: if on { save(); altOn() } else { altOff(); restore() }
        case 2004: md.paste = on
        default: break
        }
    }
    private func sgr(_ ps: [UInt32]) {
        // a little machine, as vt's Sg: 0 plain, 1 after 38/48, 2 palette index, 3/4/5 r/g/b
        var (m, fg, r, g) = (0, true, UInt32(0), UInt32(0)); var p = cur.pen
        func color(_ fg: Bool, _ c: UInt32) { if fg { p.fg = c } else { p.bg = c } }
        for n in ps {
            switch m {
            case 0:
                switch n {
                case 0: p = PEN0
                case 1: p.at |= A_BOLD
                case 2: p.at |= A_DIM
                case 4, 21: p.at |= A_UL
                case 7: p.at |= A_INV
                case 8: p.at |= A_HID
                case 22: p.at &= ~(A_BOLD | A_DIM)
                case 24: p.at &= ~A_UL
                case 27: p.at &= ~A_INV
                case 28: p.at &= ~A_HID
                case 38: m = 1; fg = true
                case 48: m = 1; fg = false
                case 39: p.fg = DEF
                case 49: p.bg = DEF
                case 30...37: p.fg = n - 30
                case 40...47: p.bg = n - 40
                case 90...97: p.fg = n - 90 + 8
                case 100...107: p.bg = n - 100 + 8
                default: break
                }
            case 1: m = n == 5 ? 2 : n == 2 ? 3 : 0
            case 2: color(fg, n & 255); m = 0
            case 3: r = n; m = 4
            case 4: g = n; m = 5
            default: color(fg, TRUE | ((r & 255) << 16) | ((g & 255) << 8) | (n & 255)); m = 0
            }
        }
        cur.pen = p
    }
    private func dsr(_ k: UInt32) { switch k { case 5: emit("\u{1b}[0n"); case 6: emit("\u{1b}[\(cur.y + 1);\(cur.x + 1)R"); default: break } }
    private func stbm(_ ps: [UInt32]) {
        func arg(_ i: Int, _ d: Int) -> Int { if i < ps.count, ps[i] != 0 { return Int(ps[i]) }; return d }
        let t = arg(0, 1) - 1; let b = min(arg(1, rows), rows) - 1
        if t < b { top = t; bot = b; home() }
    }
    private func csi(_ pv: UInt8, _ ps: [UInt32], _ inter: UInt8, _ fin: UInt8) {
        if inter != 0 { return }
        let a0 = ps.first ?? 0
        let n = a0 == 0 ? 1 : Int(a0)
        let f = Character(Unicode.Scalar(fin))
        switch pv {
        case 0:
            switch f {
            case "@": let c = bcell(); let (x, y) = (cur.x, cur.y); for _ in 0..<min(n, cols) { scr[y].insert(c, at: x); scr[y].removeLast() }
            case "A": up(n)
            case "B": down(n)
            case "C", "a": goto(cur.x + n, cur.y)
            case "D": goto(sat(cur.x, n), cur.y)
            case "E": down(n); cr()
            case "F": up(n); cr()
            case "G", "`": goto(n - 1, cur.y)
            case "H", "f": let c = ps.count > 1 && ps[1] != 0 ? Int(ps[1]) : 1; let y = orow(n - 1); goto(c - 1, y)
            case "I": tab(n)
            case "J": ed(a0)
            case "K": el(a0)
            case "L": ilDl(n, false)
            case "M": ilDl(n, true)
            case "P": let c = bcell(); let (x, y) = (cur.x, cur.y); for _ in 0..<min(n, cols) { if x < scr[y].count { scr[y].remove(at: x); scr[y].append(c) } }
            case "S": scrollUp(n)
            case "T": scrollDown(n)
            case "X": let x = cur.x; fill(x, x + min(n, cols))
            case "Z": btab(n)
            case "c": emit("\u{1b}[?1;2c")
            case "d": let y = orow(n - 1); goto(cur.x, y)
            case "e": goto(cur.x, cur.y + n)
            case "h", "l": for v in ps where v == 4 { md.ins = f == "h" }
            case "m": sgr(ps)
            case "n": dsr(a0)
            case "r": stbm(ps)
            case "s": save()
            case "u": restore()
            default: break
            }
        case UInt8(ascii: "?"):
            switch f {
            case "J": ed(a0)
            case "K": el(a0)
            case "h": for v in ps { dec(v, true) }
            case "l": for v in ps { dec(v, false) }
            default: break
            }
        case UInt8(ascii: ">"): if f == "c" { emit("\u{1b}[>1;10;0c") }
        default: break
        }
    }
    private func esc(_ inter: UInt8, _ fin: UInt8) {
        switch inter {
        case 0:
            switch fin {
            case UInt8(ascii: "7"): save()
            case UInt8(ascii: "8"): restore()
            case UInt8(ascii: "D"): nl()
            case UInt8(ascii: "E"): cr(); nl()
            case UInt8(ascii: "M"): ri()
            case UInt8(ascii: "c"): reset()
            default: break
            }
        case UInt8(ascii: "("): cur.dec = fin == UInt8(ascii: "0")
        default: break
        }
    }
    /// RIS: the scrollback stays.
    private func reset() {
        top = 0; bot = rows - 1
        scr = Array(repeating: Array(repeating: Cell.blank(DEF), count: cols), count: rows); main = nil
        cur = CUR0; saved = CUR0; md = Modes(); title = ""; out = []; par = .g
    }
    private func osc(_ bs: [UInt8]) {
        guard let s = bs.firstIndex(of: UInt8(ascii: ";")) else { return }
        if s > 0 && bs[..<s].allSatisfy({ $0 >= 48 && $0 <= 57 }) {
            let n = UInt32(String(decoding: bs[..<s], as: UTF8.self)) ?? UInt32.max
            if n == 0 || n == 2 { title = String(decoding: bs[(s + 1)...], as: UTF8.self) }
        }
    }
    private func ctl(_ b: UInt8) { switch b { case 8: goto(sat(cur.x, 1), cur.y); case 9: tab(1); case 10, 11, 12: nl(); case 13: cr(); default: break } }

    /// Feed bytes, any chunking.
    func feed(_ bytes: [UInt8]) { for b in bytes { byte(b) } }
    func feed(_ s: String) { feed(Array(s.utf8)) }
    private func byte(_ b: UInt8) {
        let p = par; par = .g
        switch p {
        case .g: ground(b)
        case .u(let cp, let owe):
            if (0x80..<0xC0).contains(b) { let cp = (cp << 6) | (UInt32(b) & 63); if owe == 1 { print(cp) } else { par = .u(cp: cp, owe: owe - 1) } }
            else { print(0xFFFD); ground(b) }
        case .e: afterEsc(b)
        case .i(let i):
            switch b {
            case 0x00...0x1f: c0(b, .i(i))
            case 0x20...0x2f: par = .i(b)
            case 0x30...0x7e: esc(i, b)
            default: break
            }
        case .c(let pv, var ps, let cur, let inter):
            switch b {
            case 0x00...0x1f: c0(b, .c(pv: pv, ps: ps, cur: cur, inter: inter))
            case 0x20...0x2f: par = .c(pv: pv, ps: ps, cur: cur, inter: b)
            case 0x30...0x39: par = .c(pv: pv, ps: ps, cur: min(cur * 10 + UInt32(b - 0x30), 65535), inter: inter)
            case 0x3a, 0x3b: if ps.count < 32 { ps.append(cur) }; par = .c(pv: pv, ps: ps, cur: 0, inter: inter)
            case 0x3c...0x3f: par = .c(pv: b, ps: ps, cur: cur, inter: inter)
            case 0x40...0x7e: if ps.count < 32 { ps.append(cur) }; csi(pv, ps, inter, b)
            default: par = .c(pv: pv, ps: ps, cur: cur, inter: inter)
            }
        case .o(var acc, let esc):
            if esc { if b == UInt8(ascii: "\\") { osc(acc) } else { afterEsc(b) } }
            else {
                switch b {
                case 7: osc(acc)
                case 27: par = .o(acc: acc, esc: true)
                case 24, 26: break
                default: if acc.count < oscMax { acc.append(b) }; par = .o(acc: acc, esc: false)
                }
            }
        case .s(let esc):
            if (esc && b == UInt8(ascii: "\\")) || b == 24 || b == 26 { }
            else if b == 27 { par = .s(esc: true) } else { par = .s(esc: false) }
        }
    }
    private func c0(_ b: UInt8, _ stay: Par) { switch b { case 27: par = .e; case 24, 26: break; default: par = stay; ctl(b) } }
    private func ground(_ b: UInt8) {
        switch b {
        case 0x00...0x1f: c0(b, .g)
        case 0x7f: break
        case 0x20...0x7e: print(UInt32(b))
        case 0x80...0xBF: print(0xFFFD)
        case 0xC0...0xDF: par = .u(cp: UInt32(b & 31), owe: 1)
        case 0xE0...0xEF: par = .u(cp: UInt32(b & 15), owe: 2)
        case 0xF0...0xF7: par = .u(cp: UInt32(b & 7), owe: 3)
        default: print(0xFFFD)
        }
    }
    private func afterEsc(_ b: UInt8) {
        switch b {
        case 0x00...0x1f: c0(b, .e)
        case 0x20...0x2f: par = .i(b)
        case UInt8(ascii: "["): par = .c(pv: 0, ps: [], cur: 0, inter: 0)
        case UInt8(ascii: "]"): par = .o(acc: [], esc: false)
        case UInt8(ascii: "P"), UInt8(ascii: "X"), UInt8(ascii: "^"), UInt8(ascii: "_"): par = .s(esc: false)
        case 0x30...0x7e: esc(0, b)
        default: break
        }
    }

    /// Resize: rows cut or padded on the right (no reflow); a screen shorter than the cursor's row gives its top rows to the scrollback.
    func resize(cols c0: Int, rows r0: Int) {
        let (c, r) = (clampc(c0), clampr(r0))
        let k = sat(cur.y + 1, r)
        func refit(_ s: [[Cell]], _ k: Int) -> [[Cell]] {
            var v: [[Cell]] = s.dropFirst(k).map { row in
                var row = row
                if row.count > c { row.removeLast(row.count - c) } else if row.count < c { row.append(contentsOf: Array(repeating: Cell.blank(DEF), count: c - row.count)) }
                return row
            }
            if v.count > r { v.removeLast(v.count - r) }
            while v.count < r { v.append(Array(repeating: Cell.blank(DEF), count: c)) }
            return v
        }
        let old = scr
        if main == nil { for row in old.prefix(k) { pushSb(row) } }
        scr = refit(old, k)
        if let m = main { main = refit(m, 0) }
        cols = c; rows = r; top = 0; bot = r - 1
        cur.y -= min(k, cur.y); cur.x = min(cur.x, c - 1); cur.y = min(cur.y, r - 1)
        saved.x = min(saved.x, c - 1); saved.y = min(saved.y, r - 1)
    }

    /// Exactly `rows` lines; with `scroll > 0` the top ones come from the scrollback. Trailing blank cells are left out.
    func lines(_ scroll0: Int = 0) -> [VLine] {
        let scroll = min(max(scroll0, 0), sb.count)
        let total = sb.count + rows; let end = total - scroll; let start = end - rows
        return (start..<end).map { i in lineOf(i < sb.count ? sb[i] : scr[i - sb.count]) }
    }

    // ---- keys ----
    /// Bytes for a key press: an X11 keysym, the text it typed (a code point, 0 for none), mods (bit 0 shift, 2 ctrl, 3 alt).
    func key(_ keysym: UInt32, _ text: UInt32, _ mods: UInt32) -> [UInt8] {
        let (ctrl, alt, shift) = (mods & 4 != 0, mods & 8 != 0, mods & 1 != 0)
        func meta(_ v: [UInt8]) -> [UInt8] { alt ? [27] + v : v }
        if keysym >= 65280 {
            let m = 1 + (mods & 1) + ((mods & 8) >> 2) + (mods & 4)
            func csik(_ l: Character, _ app: Bool) -> [UInt8] { m == 1 ? [27, app ? UInt8(ascii: "O") : UInt8(ascii: "["), l.asciiValue!] : Array("\u{1b}[1;\(m)\(l)".utf8) }
            func tilde(_ n: Int) -> [UInt8] { Array((m == 1 ? "\u{1b}[\(n)~" : "\u{1b}[\(n);\(m)~").utf8) }
            let a = md.appk
            switch keysym - 65280 {
            case 8: return meta([ctrl ? 8 : 127])
            case 9: return shift ? [27, 91, 90] : meta([9])
            case 13: return meta([13])
            case 27: return [27]
            case 80, 149: return csik("H", a)
            case 81, 150: return csik("D", a)
            case 82, 151: return csik("A", a)
            case 83, 152: return csik("C", a)
            case 84, 153: return csik("B", a)
            case 85, 154: return tilde(5)
            case 86, 155: return tilde(6)
            case 87, 156: return csik("F", a)
            case 99, 158: return tilde(2)
            case 159, 255: return tilde(3)
            case 141: return [13]
            case 190: return csik("P", true)
            case 191: return csik("Q", true)
            case 192: return csik("R", true)
            case 193: return csik("S", true)
            case 194: return tilde(15)
            case 195: return tilde(17)
            case 196: return tilde(18)
            case 197: return tilde(19)
            case 198: return tilde(20)
            case 199: return tilde(21)
            case 200: return tilde(23)
            case 201: return tilde(24)
            default: break
            }
        } else if keysym == 65056 { return [27, 91, 90] }
        let cp0: UInt32 = text != 0 ? text : (32...255).contains(keysym) ? keysym : keysym >= 16777216 ? keysym - 16777216 : 0
        if cp0 == 0 { return [] }
        return meta(vtUtf8(ctrl ? ctrlCp(cp0) : cp0))
    }
    /// Bytes for pasted text: LF and CR LF become CR; bracketed when the program asked (?2004), ESC dropped then.
    func paste(_ text: String) -> [UInt8] {
        let br = md.paste; let b = Array(text.utf8); var o: [UInt8] = []
        for (i, c) in b.enumerated() {
            if c == 13 && i + 1 < b.count && b[i + 1] == 10 { continue }
            if c == 10 { o.append(13) } else if c == 27 && br { } else { o.append(c) }
        }
        return br ? Array("\u{1b}[200~".utf8) + o + Array("\u{1b}[201~".utf8) : o
    }
}

private func ctrlCp(_ cp: UInt32) -> UInt32 {
    if (97...122).contains(cp) { return cp - 96 }
    if (65...90).contains(cp) { return cp - 64 }
    if cp < 128 {
        switch cp { case 32, 64, 50: return 0; case 91, 51: return 27; case 92, 52: return 28; case 93, 53: return 29; case 94, 54: return 30; case 95, 55, 47: return 31; case 63, 56: return 127; default: return cp }
    }
    return cp
}
func vtUtf8(_ cp: UInt32) -> [UInt8] { if let s = Unicode.Scalar(cp) { return Array(String(Character(s)).utf8) }; return Array("\u{FFFD}".utf8) }

private func zero(_ cp: UInt32) -> Bool { func i(_ a: UInt32, _ b: UInt32) -> Bool { (a...b).contains(cp) }; return i(768, 879) || i(8203, 8207) || i(8400, 8447) || i(65024, 65039) || i(65056, 65071) || i(6832, 6911) || i(7616, 7679) }
private func wide(_ cp: UInt32) -> Bool {
    func i(_ a: UInt32, _ b: UInt32) -> Bool { (a...b).contains(cp) }
    return i(4352, 4447) || i(11904, 12350) || i(12353, 13311) || i(13312, 19903) || i(19968, 40959) || i(40960, 42191) || i(44032, 55203) || i(63744, 64255) || i(65072, 65103) || i(65280, 65376) || i(65504, 65510) || i(127744, 128591) || i(128640, 128767) || i(129280, 129535) || i(131072, 262141)
}
/// The cells a code point takes: 0 (combining), 1, or 2 (wide).
func vtWidth(_ cp: UInt32) -> Int { cp < 768 ? 1 : zero(cp) ? 0 : wide(cp) ? 2 : 1 }
/// The DEC special graphics set (ESC ( 0).
private func gfx(_ cp: UInt32) -> UInt32 {
    switch cp { case 95: return 32; case 96: return 9670; case 97: return 9618; case 102: return 176; case 103: return 177; case 106: return 9496; case 107: return 9488; case 108: return 9484; case 109: return 9492; case 110: return 9532; case 111: return 9146; case 112: return 9147; case 113: return 9472; case 114: return 9148; case 115: return 9149; case 116: return 9500; case 117: return 9508; case 118: return 9524; case 119: return 9516; case 120: return 9474; case 121: return 8804; case 122: return 8805; case 123: return 960; case 124: return 8800; case 125: return 163; case 126: return 183; default: return cp }
}

private func resolve(_ c: Cell) -> (UInt32, UInt32, Bool, Bool) {
    let (f0, b0) = (rgb(c.fg, vtDefaultFg), rgb(c.bg, vtDefaultBg))
    let inv = c.at & A_INV != 0
    let (f1, b1) = inv ? (b0, f0) : (f0, b0)
    let f2 = c.at & A_DIM != 0 ? blend(f1, b1) : f1
    return (c.at & A_HID != 0 ? b1 : f2, b1, c.at & A_BOLD != 0, c.at & A_UL != 0)
}
private func lineOf(_ row: [Cell]) -> VLine {
    guard let last = row.lastIndex(where: { !($0.ch == 32 && $0.bg == DEF && $0.at & (A_UL | A_INV) == 0) }) else { return VLine() }
    var runs: [VRun] = []
    for c in row[...last] {
        if c.ch == 0 { continue }
        let (fg, bg, bold, ul) = resolve(c)
        let ch = Unicode.Scalar(c.ch) ?? "\u{FFFD}"
        if let l = runs.last, l.fg == fg, l.bg == bg, l.bold == bold, l.underline == ul { runs[runs.count - 1].text.unicodeScalars.append(ch) }
        else { var t = ""; t.unicodeScalars.append(ch); runs.append(VRun(text: t, fg: fg, bg: bg, bold: bold, underline: ul)) }
    }
    return VLine(runs: runs)
}
