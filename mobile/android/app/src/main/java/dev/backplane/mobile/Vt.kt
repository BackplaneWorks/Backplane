package dev.backplane.mobile

// The terminal emulator: a port of the hub's native/host/src/term/vt.rs (VT100/xterm subset: UTF-8, C0, ESC, CSI, SGR, OSC titles, scroll regions,
// alternate screen, 2048-row scrollback, DSR/DA replies, keys and paste). Bytes in, styled rows out; the same rules as the desktop's, so a phone that
// feeds the hub's bytes draws what the hub's screen holds. Bounded: at most MAX_COLS x MAX_ROWS, a 2048-row scrollback, an OSC string of 4096 bytes,
// queued replies of MAX_REPLY bytes. (VtTest is vt/tests.rs line for line.)

const val VT_MAX_COLS = 500
const val VT_MAX_ROWS = 200
const val VT_SCROLLBACK = 2048
private const val OSC_MAX = 4096
private const val MAX_REPLY = 4096
const val VT_DEFAULT_FG = 14277092
const val VT_DEFAULT_BG = 855826

private val PAL16 = intArrayOf(1908774, 14707829, 10011513, 15057019, 6402031, 13007069, 5682882, 11252415, 6054768, 16743302, 11921552, 16766602, 8570111, 14392560, 8378080, 15790837)
private fun lvl(v: Int) = when (v) { 0 -> 0; 1 -> 95; 2 -> 135; 3 -> 175; 4 -> 215; else -> 255 }
/** Palette entry i as 0xRRGGBB (16 colors, the 6x6x6 cube, the gray ramp). */
fun vtPal(i: Int): Int = if (i < 16) PAL16[i] else if (i < 232) { val j = i - 16; (lvl(j / 36) shl 16) or (lvl((j / 6) % 6) shl 8) or lvl(j % 6) } else (8 + (i - 232) * 10) * 65793
private const val DEF = 256
private const val TRUE = 1 shl 24
private fun rgb(c: Int, d: Int) = if (c == DEF) d else if (c >= TRUE) c and 0xFFFFFF else vtPal(c and 255)
private fun blend(f: Int, b: Int) = ((f shr 1) and 8355711) + ((b shr 1) and 8355711)

private const val A_BOLD = 1; private const val A_UL = 2; private const val A_INV = 4; private const val A_DIM = 8; private const val A_HID = 16

/** ch 0 is the right half of a wide character. */
private data class Cell(val ch: Int, val fg: Int, val bg: Int, val at: Int) { companion object { fun blank(bg: Int) = Cell(32, DEF, bg, 0) } }
private data class Pen(var fg: Int = DEF, var bg: Int = DEF, var at: Int = 0)
private data class Cur(var x: Int = 0, var y: Int = 0, var wrap: Boolean = false, var pen: Pen = Pen(), var dec: Boolean = false) {
    fun copyDeep() = Cur(x, y, wrap, pen.copy(), dec)
}
private class Modes(var show: Boolean = true, var awm: Boolean = true, var appk: Boolean = false, var paste: Boolean = false, var org: Boolean = false, var ins: Boolean = false)

/** A run of cells with one style, colors resolved (0xRRGGBB). */
data class VRun(val text: String, val fg: Int, val bg: Int, val bold: Boolean, val underline: Boolean)
data class VLine(val runs: List<VRun> = emptyList())
data class VCur(val x: Int, val y: Int, val visible: Boolean)

private sealed class Par {
    object G : Par()
    class U(val cp: Int, val owe: Int) : Par()
    object E : Par()
    class I(val inter: Int) : Par()
    class C(val pv: Int, val ps: ArrayList<Int>, val cur: Int, val inter: Int) : Par()
    class O(val acc: java.io.ByteArrayOutputStream, val esc: Boolean) : Par()
    class S(val esc: Boolean) : Par()
}

private fun clampc(n: Int) = n.coerceIn(1, VT_MAX_COLS)
private fun clampr(n: Int) = n.coerceIn(1, VT_MAX_ROWS)

class Vt(cols: Int, rows: Int) {
    private var cols = clampc(cols); private var rows = clampr(rows)
    private var top = 0; private var bot = this.rows - 1
    private var scr: ArrayList<ArrayList<Cell>> = blankScreen(this.cols, this.rows)
    private var main: ArrayList<ArrayList<Cell>>? = null
    private var cur = Cur(); private var saved = Cur(); private var md = Modes()
    private var sb = ArrayDeque<List<Cell>>()
    private var titleText = ""; private var out = java.io.ByteArrayOutputStream(); private var par: Par = Par.G

    private fun blankScreen(c: Int, r: Int): ArrayList<ArrayList<Cell>> = ArrayList<ArrayList<Cell>>(r).also { s -> repeat(r) { s.add(ArrayList<Cell>(c).also { row -> repeat(c) { row.add(Cell.blank(DEF)) } }) } }

    fun cols() = cols
    fun rows() = rows
    /** Rows in the scrollback (the most `lines` can scroll). */
    fun history() = sb.size
    fun alt() = main != null
    fun title() = titleText
    fun appk() = md.appk
    fun bpaste() = md.paste
    fun cursor() = VCur(cur.x, cur.y, md.show)
    /** Replies the program asked for (DSR, DA): write them to the pty, then `drain`. */
    fun replies(): ByteArray = out.toByteArray()
    fun drain() { out.reset() }

    private fun full() = top == 0 && bot == rows - 1
    private fun bcell() = Cell.blank(cur.pen.bg)
    private fun brow(): ArrayList<Cell> { val c = bcell(); return ArrayList<Cell>(cols).also { r -> repeat(cols) { r.add(c) } } }
    private fun emit(b: ByteArray) { if (out.size() + b.size <= MAX_REPLY) out.write(b, 0, b.size) }
    private fun emit(s: String) = emit(s.toByteArray(Charsets.ISO_8859_1))
    private fun pushSb(row: List<Cell>) { if (main == null) { if (sb.size >= VT_SCROLLBACK) sb.removeFirst(); sb.addLast(row) } }

    // ---- cursor motion ----
    private fun goto(x: Int, y: Int) { cur.x = minOf(x, cols - 1); cur.y = minOf(y, rows - 1); cur.wrap = false }
    private fun cr() = goto(0, cur.y)
    private fun orow(r: Int) = if (md.org) minOf(r + top, bot) else r
    private fun home() { val y = orow(0); goto(0, y) }
    private fun up(n: Int) { val y = cur.y; val lo = if (y >= top) top else 0; goto(cur.x, maxOf(maxOf(y - n, 0), lo)) }
    private fun down(n: Int) { val y = cur.y; val hi = if (y <= bot) bot else rows - 1; goto(cur.x, minOf(y + n, hi)) }
    private fun tab(n: Int) = goto(((cur.x shr 3) + n) shl 3, cur.y)
    private fun btab(n: Int) = goto(maxOf(((cur.x + 7) shr 3) - n, 0) shl 3, cur.y)

    // ---- scrolling ----
    private fun rot1() { val blank = brow(); val gone = scr.removeAt(0); scr.add(blank); pushSb(gone) }
    private fun scrollUp(n: Int) {
        if (full()) { repeat(minOf(n, rows)) { rot1() } }
        else {
            val t = top; val b = bot; val k = minOf(n, b + 1 - t)
            val toSb = t == 0 && main == null
            val blank = brow()
            repeat(k) { val gone = scr.removeAt(t); scr.add(b, ArrayList(blank)); if (toSb) pushSb(gone) }
        }
    }
    private fun scrollDown(n: Int) { val t = top; val b = bot; val blank = brow(); repeat(minOf(n, b + 1 - t)) { scr.removeAt(b); scr.add(t, ArrayList(blank)) } }
    private fun nl() { val y = cur.y; if (y == bot) scrollUp(1) else if (y + 1 < rows) goto(cur.x, y + 1) }
    private fun ri() { val y = cur.y; if (y == top) scrollDown(1) else if (y > 0) goto(cur.x, y - 1) }
    /** IL (del false) / DL (del true) inside the region. */
    private fun ilDl(n: Int, del: Boolean) {
        val y = cur.y; if (y < top || y > bot) return
        val b = bot; val blank = brow()
        repeat(minOf(n, b + 1 - y)) { if (del) { scr.removeAt(y); scr.add(b, ArrayList(blank)) } else { scr.removeAt(b); scr.add(y, ArrayList(blank)) } }
        goto(0, y)
    }

    // ---- erasing ----
    private fun fill(a: Int, b: Int) { val c = bcell(); val y = cur.y; for (x in a until minOf(b, cols)) scr[y][x] = c }
    private fun el(mode: Int) { val x = cur.x; when (mode) { 0 -> fill(x, cols); 1 -> fill(0, x + 1); else -> fill(0, cols) } }
    private fun clearRows(from: Int, n: Int) { val b = brow(); for (y in from until minOf(from + n, rows)) scr[y] = ArrayList(b) }
    private fun ed(mode: Int) {
        val x = cur.x; val y = cur.y
        when (mode) {
            0 -> { fill(x, cols); clearRows(y + 1, rows - 1 - y) }
            1 -> { clearRows(0, y); fill(0, x + 1) }
            2 -> clearRows(0, rows)
            3 -> sb.clear()
        }
    }

    // ---- printing ----
    private fun print(cp0: Int) {
        val cp = if (cur.dec && cp0 in 95..126) gfx(cp0) else cp0
        val w = vtWidth(cp); if (w == 0) return
        if (md.awm && (cur.wrap || (w == 2 && cur.x + 1 >= cols))) { cr(); nl() }
        val x = cur.x; val y = cur.y; val p = cur.pen
        if (md.ins) { val row = scr[y]; repeat(w) { row.add(x, Cell.blank(p.bg)); row.removeAt(row.size - 1) } }
        fun cell(ch: Int) = Cell(ch, p.fg, p.bg, p.at)
        scr[y][x] = cell(cp)
        if (w == 2 && x + 1 < cols) scr[y][x + 1] = cell(0)
        if (x + w >= cols) { cur.x = cols - 1; cur.wrap = md.awm } else { cur.x = x + w; cur.wrap = false }
    }

    // ---- escape sequences ----
    private fun save() { saved = cur.copyDeep() }
    private fun restore() { val c = saved.copyDeep(); c.x = minOf(c.x, cols - 1); c.y = minOf(c.y, rows - 1); cur = c }
    private fun altOn() { if (main == null) { main = scr; scr = blankScreen(cols, rows) } }
    private fun altOff() { main?.let { scr = it; main = null } }
    private fun dec(n: Int, on: Boolean) {
        when (n) {
            1 -> md.appk = on
            6 -> { md.org = on; home() }
            7 -> md.awm = on
            25 -> md.show = on
            47, 1047 -> if (on) altOn() else altOff()
            1048 -> if (on) save() else restore()
            1049 -> if (on) { save(); altOn() } else { altOff(); restore() }
            2004 -> md.paste = on
        }
    }
    private fun sgr(ps: List<Int>) {
        // a little machine, as vt's Sg: 0 plain, 1 after 38/48, 2 palette index, 3/4/5 r/g/b
        var m = 0; var fg = true; var r = 0; var g = 0; val p = cur.pen.copy()
        fun color(fgc: Boolean, c: Int) { if (fgc) p.fg = c else p.bg = c }
        for (n in ps) {
            when (m) {
                0 -> when (n) {
                    0 -> { p.fg = DEF; p.bg = DEF; p.at = 0 }
                    1 -> p.at = p.at or A_BOLD; 2 -> p.at = p.at or A_DIM; 4, 21 -> p.at = p.at or A_UL; 7 -> p.at = p.at or A_INV; 8 -> p.at = p.at or A_HID
                    22 -> p.at = p.at and (A_BOLD or A_DIM).inv(); 24 -> p.at = p.at and A_UL.inv(); 27 -> p.at = p.at and A_INV.inv(); 28 -> p.at = p.at and A_HID.inv()
                    38 -> { m = 1; fg = true }
                    48 -> { m = 1; fg = false }
                    39 -> p.fg = DEF; 49 -> p.bg = DEF
                    in 30..37 -> p.fg = n - 30; in 40..47 -> p.bg = n - 40; in 90..97 -> p.fg = n - 90 + 8; in 100..107 -> p.bg = n - 100 + 8
                }
                1 -> m = if (n == 5) 2 else if (n == 2) 3 else 0
                2 -> { color(fg, n and 255); m = 0 }
                3 -> { r = n; m = 4 }
                4 -> { g = n; m = 5 }
                else -> { color(fg, TRUE or ((r and 255) shl 16) or ((g and 255) shl 8) or (n and 255)); m = 0 }
            }
        }
        cur.pen = p
    }
    private fun dsr(k: Int) { when (k) { 5 -> emit("\u001b[0n"); 6 -> emit("\u001b[${cur.y + 1};${cur.x + 1}R") } }
    private fun stbm(ps: List<Int>) {
        fun arg(i: Int, d: Int) = ps.getOrNull(i)?.takeIf { it != 0 } ?: d
        val t = arg(0, 1) - 1; val b = minOf(arg(1, rows), rows) - 1
        if (t < b) { top = t; bot = b; home() }
    }
    private fun csi(pv: Int, ps: List<Int>, inter: Int, fin: Int) {
        if (inter != 0) return
        val a0 = ps.firstOrNull() ?: 0
        val n = if (a0 == 0) 1 else a0
        val f = fin.toChar()
        when (pv) {
            0 -> when (f) {
                '@' -> { val c = bcell(); val x = cur.x; val y = cur.y; repeat(minOf(n, cols)) { scr[y].add(x, c); scr[y].removeAt(scr[y].size - 1) } }
                'A' -> up(n); 'B' -> down(n); 'C', 'a' -> goto(cur.x + n, cur.y); 'D' -> goto(maxOf(cur.x - n, 0), cur.y)
                'E' -> { down(n); cr() }; 'F' -> { up(n); cr() }
                'G', '`' -> goto(n - 1, cur.y)
                'H', 'f' -> { val c = ps.getOrNull(1)?.takeIf { it != 0 } ?: 1; val y = orow(n - 1); goto(c - 1, y) }
                'I' -> tab(n); 'J' -> ed(a0); 'K' -> el(a0)
                'L' -> ilDl(n, false); 'M' -> ilDl(n, true)
                'P' -> { val c = bcell(); val x = cur.x; val y = cur.y; repeat(minOf(n, cols)) { if (x < scr[y].size) { scr[y].removeAt(x); scr[y].add(c) } } }
                'S' -> scrollUp(n); 'T' -> scrollDown(n)
                'X' -> { val x = cur.x; fill(x, x + minOf(n, cols)) }
                'Z' -> btab(n)
                'c' -> emit("\u001b[?1;2c")
                'd' -> { val y = orow(n - 1); goto(cur.x, y) }
                'e' -> goto(cur.x, cur.y + n)
                'h', 'l' -> for (v in ps) { if (v == 4) md.ins = f == 'h' }
                'm' -> sgr(ps)
                'n' -> dsr(a0)
                'r' -> stbm(ps)
                's' -> save(); 'u' -> restore()
            }
            '?'.code -> when (f) { 'J' -> ed(a0); 'K' -> el(a0); 'h' -> for (v in ps) dec(v, true); 'l' -> for (v in ps) dec(v, false) }
            '>'.code -> if (f == 'c') emit("\u001b[>1;10;0c")
        }
    }
    private fun esc(inter: Int, fin: Int) {
        when (inter) {
            0 -> when (fin.toChar()) { '7' -> save(); '8' -> restore(); 'D' -> nl(); 'E' -> { cr(); nl() }; 'M' -> ri(); 'c' -> reset() }
            '('.code -> cur.dec = fin == '0'.code
        }
    }
    /** RIS: the scrollback stays. */
    private fun reset() {
        val keep = sb; val c = cols; val r = rows
        top = 0; bot = r - 1; scr = blankScreen(c, r); main = null; cur = Cur(); saved = Cur(); md = Modes(); sb = keep; titleText = ""; out = java.io.ByteArrayOutputStream(); par = Par.G
    }
    private fun osc(bs: ByteArray) {
        val s = bs.indexOf(';'.code.toByte()); if (s < 0) return
        if (s > 0 && (0 until s).all { bs[it] in '0'.code.toByte()..'9'.code.toByte() }) {
            val n = String(bs, 0, s, Charsets.ISO_8859_1).toLongOrNull() ?: Long.MAX_VALUE
            if (n == 0L || n == 2L) titleText = String(bs, s + 1, bs.size - s - 1, Charsets.UTF_8)
        }
    }
    private fun ctl(b: Int) { when (b) { 8 -> goto(maxOf(cur.x - 1, 0), cur.y); 9 -> tab(1); 10, 11, 12 -> nl(); 13 -> cr() } }

    /** Feed bytes, any chunking. */
    fun feed(bytes: ByteArray) { for (b in bytes) byte(b.toInt() and 255) }
    fun feed(s: String) = feed(s.toByteArray(Charsets.UTF_8))
    private fun byte(b: Int) {
        val p = par; par = Par.G
        when (p) {
            is Par.G -> ground(b)
            is Par.U -> {
                if (b in 0x80..0xBF) { val cp = (p.cp shl 6) or (b and 63); if (p.owe == 1) print(cp) else par = Par.U(cp, p.owe - 1) }
                else { print(0xFFFD); ground(b) }
            }
            is Par.E -> afterEsc(b)
            is Par.I -> when (b) {
                in 0x00..0x1f -> c0(b, p)
                in 0x20..0x2f -> par = Par.I(b)
                in 0x30..0x7e -> esc(p.inter, b)
            }
            is Par.C -> when (b) {
                in 0x00..0x1f -> c0(b, p)
                in 0x20..0x2f -> par = Par.C(p.pv, p.ps, p.cur, b)
                in '0'.code..'9'.code -> par = Par.C(p.pv, p.ps, minOf(p.cur * 10 + (b - '0'.code), 65535), p.inter)
                ':'.code, ';'.code -> { if (p.ps.size < 32) p.ps.add(p.cur); par = Par.C(p.pv, p.ps, 0, p.inter) }
                in '<'.code..'?'.code -> par = Par.C(b, p.ps, p.cur, p.inter)
                in 0x40..0x7e -> { if (p.ps.size < 32) p.ps.add(p.cur); csi(p.pv, p.ps, p.inter, b) }
                else -> par = p
            }
            is Par.O -> {
                if (p.esc) { if (b == '\\'.code) osc(p.acc.toByteArray()) else afterEsc(b) }
                else when (b) {
                    7 -> osc(p.acc.toByteArray())
                    27 -> par = Par.O(p.acc, true)
                    24, 26 -> {}
                    else -> { if (p.acc.size() < OSC_MAX) p.acc.write(b); par = Par.O(p.acc, false) }
                }
            }
            is Par.S -> if ((p.esc && b == '\\'.code) || b == 24 || b == 26) {} else if (b == 27) par = Par.S(true) else par = Par.S(false)
        }
    }
    private fun c0(b: Int, stay: Par) { when (b) { 27 -> par = Par.E; 24, 26 -> {}; else -> { par = stay; ctl(b) } } }
    private fun ground(b: Int) {
        when (b) {
            in 0x00..0x1f -> c0(b, Par.G)
            0x7f -> {}
            in 0x20..0x7e -> print(b)
            in 0x80..0xBF -> print(0xFFFD)
            in 0xC0..0xDF -> par = Par.U(b and 31, 1)
            in 0xE0..0xEF -> par = Par.U(b and 15, 2)
            in 0xF0..0xF7 -> par = Par.U(b and 7, 3)
            else -> print(0xFFFD)
        }
    }
    private fun afterEsc(b: Int) {
        when (b) {
            in 0x00..0x1f -> c0(b, Par.E)
            in 0x20..0x2f -> par = Par.I(b)
            '['.code -> par = Par.C(0, ArrayList(), 0, 0)
            ']'.code -> par = Par.O(java.io.ByteArrayOutputStream(), false)
            'P'.code, 'X'.code, '^'.code, '_'.code -> par = Par.S(false)
            in 0x30..0x7e -> esc(0, b)
        }
    }

    /** Resize: rows cut or padded on the right (no reflow); a screen shorter than the cursor's row gives its top rows to the scrollback. */
    fun resize(c0: Int, r0: Int) {
        val c = clampc(c0); val r = clampr(r0)
        val k = maxOf(cur.y + 1 - r, 0)
        fun refit(s: List<ArrayList<Cell>>, k: Int): ArrayList<ArrayList<Cell>> {
            val v = ArrayList<ArrayList<Cell>>(r)
            for (row in s.drop(k)) { val nr = ArrayList(row); while (nr.size > c) nr.removeAt(nr.size - 1); while (nr.size < c) nr.add(Cell.blank(DEF)); v.add(nr) }
            while (v.size > r) v.removeAt(v.size - 1)
            while (v.size < r) v.add(ArrayList<Cell>(c).also { row -> repeat(c) { row.add(Cell.blank(DEF)) } })
            return v
        }
        val old = scr
        if (main == null) for (row in old.take(k)) pushSb(row)
        scr = refit(old, k)
        main?.let { main = refit(it, 0) }
        cols = c; rows = r; top = 0; bot = r - 1
        cur.y -= minOf(k, cur.y); cur.x = minOf(cur.x, c - 1); cur.y = minOf(cur.y, r - 1)
        saved.x = minOf(saved.x, c - 1); saved.y = minOf(saved.y, r - 1)
    }

    /** Exactly `rows` lines; with `scroll > 0` the top ones come from the scrollback. Trailing blank cells are left out. */
    fun lines(scroll0: Int): List<VLine> {
        val scroll = minOf(scroll0, sb.size)
        val total = sb.size + rows; val end = total - scroll; val start = end - rows
        return (start until end).map { i -> lineOf(if (i < sb.size) sb[i] else scr[i - sb.size]) }
    }

    // ---- keys ----
    /** Bytes for a key press: an X11 keysym, the text it typed (a code point, 0 for none), mods (bit 0 shift, 2 ctrl, 3 alt). */
    fun key(keysym: Int, text: Int, mods: Int): ByteArray {
        val ctrl = (mods and 4) != 0; val alt = (mods and 8) != 0; val shift = (mods and 1) != 0
        fun meta(v: ByteArray) = if (alt) byteArrayOf(27) + v else v
        if (keysym >= 65280) {
            val m = 1 + (mods and 1) + ((mods and 8) shr 2) + (mods and 4)
            fun csik(l: Char, app: Boolean): ByteArray = if (m == 1) byteArrayOf(27, (if (app) 'O' else '[').code.toByte(), l.code.toByte()) else "\u001b[1;$m$l".toByteArray()
            fun tilde(n: Int): ByteArray = (if (m == 1) "\u001b[$n~" else "\u001b[$n;$m~").toByteArray()
            val a = md.appk
            when (keysym - 65280) {
                8 -> return meta(byteArrayOf(if (ctrl) 8 else 127))
                9 -> return if (shift) byteArrayOf(27, 91, 90) else meta(byteArrayOf(9))
                13 -> return meta(byteArrayOf(13)); 27 -> return byteArrayOf(27)
                80, 149 -> return csik('H', a); 81, 150 -> return csik('D', a); 82, 151 -> return csik('A', a); 83, 152 -> return csik('C', a); 84, 153 -> return csik('B', a)
                85, 154 -> return tilde(5); 86, 155 -> return tilde(6); 87, 156 -> return csik('F', a); 99, 158 -> return tilde(2); 159, 255 -> return tilde(3)
                141 -> return byteArrayOf(13)
                190 -> return csik('P', true); 191 -> return csik('Q', true); 192 -> return csik('R', true); 193 -> return csik('S', true)
                194 -> return tilde(15); 195 -> return tilde(17); 196 -> return tilde(18); 197 -> return tilde(19); 198 -> return tilde(20); 199 -> return tilde(21); 200 -> return tilde(23); 201 -> return tilde(24)
            }
        } else if (keysym == 65056) return byteArrayOf(27, 91, 90)
        var cp = if (text != 0) text else if (keysym in 32..255) keysym else if (keysym >= 16777216) keysym - 16777216 else 0
        if (cp == 0) return ByteArray(0)
        if (ctrl) cp = ctrlCp(cp)
        return meta(vtUtf8(cp))
    }
    /** Bytes for pasted text: LF and CR LF become CR; bracketed when the program asked (?2004), ESC dropped then. */
    fun paste(text: String): ByteArray {
        val br = md.paste; val b = text.toByteArray(Charsets.UTF_8); val o = java.io.ByteArrayOutputStream()
        for ((i, c) in b.withIndex()) {
            val ci = c.toInt() and 255
            if (ci == 13 && b.getOrNull(i + 1)?.toInt() == 10) continue
            if (ci == 10) o.write(13) else if (ci == 27 && br) {} else o.write(ci)
        }
        return if (br) "\u001b[200~".toByteArray() + o.toByteArray() + "\u001b[201~".toByteArray() else o.toByteArray()
    }
}

private fun ctrlCp(cp: Int): Int =
    if (cp in 97..122) cp - 96 else if (cp in 65..90) cp - 64 else if (cp < 128) {
        when (cp) { 32, 64, 50 -> 0; 91, 51 -> 27; 92, 52 -> 28; 93, 53 -> 29; 94, 54 -> 30; 95, 55, 47 -> 31; 63, 56 -> 127; else -> cp }
    } else cp
fun vtUtf8(cp: Int): ByteArray = if (cp == 0) byteArrayOf(0) else if (cp < 0 || cp > 0x10FFFF || cp in 0xD800..0xDFFF) "�".toByteArray() else String(Character.toChars(cp)).toByteArray(Charsets.UTF_8)

private fun zero(cp: Int) = cp in 768..879 || cp in 8203..8207 || cp in 8400..8447 || cp in 65024..65039 || cp in 65056..65071 || cp in 6832..6911 || cp in 7616..7679
private fun wide(cp: Int) = cp in 4352..4447 || cp in 11904..12350 || cp in 12353..13311 || cp in 13312..19903 || cp in 19968..40959 || cp in 40960..42191 || cp in 44032..55203 ||
    cp in 63744..64255 || cp in 65072..65103 || cp in 65280..65376 || cp in 65504..65510 || cp in 127744..128591 || cp in 128640..128767 || cp in 129280..129535 || cp in 131072..262141
/** The cells a code point takes: 0 (combining), 1, or 2 (wide). */
fun vtWidth(cp: Int) = if (cp < 768) 1 else if (zero(cp)) 0 else if (wide(cp)) 2 else 1
/** The DEC special graphics set (ESC ( 0). */
private fun gfx(cp: Int) = when (cp) {
    95 -> 32; 96 -> 9670; 97 -> 9618; 102 -> 176; 103 -> 177; 106 -> 9496; 107 -> 9488; 108 -> 9484; 109 -> 9492; 110 -> 9532; 111 -> 9146; 112 -> 9147; 113 -> 9472; 114 -> 9148; 115 -> 9149
    116 -> 9500; 117 -> 9508; 118 -> 9524; 119 -> 9516; 120 -> 9474; 121 -> 8804; 122 -> 8805; 123 -> 960; 124 -> 8800; 125 -> 163; 126 -> 183; else -> cp
}

private fun lineOf(row: List<Cell>): VLine {
    val last = row.indexOfLast { !(it.ch == 32 && it.bg == DEF && (it.at and (A_UL or A_INV)) == 0) }
    if (last < 0) return VLine()
    val runs = ArrayList<VRun>(); var sb = StringBuilder(); var cf = 0; var cb = 0; var cbold = false; var cul = false
    for (i in 0..last) {
        val c = row[i]; if (c.ch == 0) continue
        val (f0, b0) = rgb(c.fg, VT_DEFAULT_FG) to rgb(c.bg, VT_DEFAULT_BG)
        val inv = (c.at and A_INV) != 0
        val f1 = if (inv) b0 else f0; val b1 = if (inv) f0 else b0
        val f2 = if ((c.at and A_DIM) != 0) blend(f1, b1) else f1
        val fg = if ((c.at and A_HID) != 0) b1 else f2; val bg = b1; val bold = (c.at and A_BOLD) != 0; val ul = (c.at and A_UL) != 0
        if (sb.isNotEmpty() && (fg != cf || bg != cb || bold != cbold || ul != cul)) { runs.add(VRun(sb.toString(), cf, cb, cbold, cul)); sb = StringBuilder() }
        if (sb.isEmpty()) { cf = fg; cb = bg; cbold = bold; cul = ul }
        if (c.ch in 0xD800..0xDFFF || c.ch > 0x10FFFF) sb.append('�') else sb.appendCodePoint(c.ch)
    }
    if (sb.isNotEmpty()) runs.add(VRun(sb.toString(), cf, cb, cbold, cul))
    return VLine(runs)
}
