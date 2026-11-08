package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

// native/host/src/term/vt/tests.rs, case for case (same names, same expected strings): the phone's emulator is the hub's.
class VtTest {
    private val E = "\u001b"
    private fun feed(v: Vt, s: String): Vt { v.feed(s); return v }
    private fun run(c: Int, r: Int, s: String) = feed(Vt(c, r), s)
    private fun text(l: VLine) = l.runs.joinToString("") { it.text }
    private fun screen(v: Vt) = v.lines(0).joinToString("|") { text(it) }
    private fun scrolled(v: Vt, n: Int) = v.lines(n).joinToString("|") { text(it) }
    private fun row0(v: Vt) = v.lines(0)[0].runs.joinToString("") { "[${it.text} ${it.fg} ${it.bg} ${if (it.bold) 1 else 0}${if (it.underline) 1 else 0}]" }
    private fun cur(v: Vt) = v.cursor().let { "${it.x},${it.y}${if (it.visible) 1 else 0}" }
    private fun bytes(b: ByteArray) = b.joinToString("") { val x = it.toInt() and 255; if (x == 27) "^[" else if (x < 32 || x >= 127) "<$x>" else x.toChar().toString() }
    private fun eq(name: String, got: String, want: String) = assertEquals(name, want, got)

    @Test fun vt_text_and_wrap() {
        val d40 = "0123456789012345678901234567890123456789"; val d100 = "$d40${d40}01234567890123456789"
        eq("vt text", screen(run(10, 3, "hi")), "hi||")
        val wrap = run(10, 3, "hello world!")
        eq("vt wrap", screen(wrap), "hello worl|d!|"); eq("vt wrap cursor", cur(wrap), "2,11")
        eq("vt pending wrap", cur(run(5, 2, "abcde")), "4,01")
        eq("vt pending wrap cr", screen(run(5, 2, "abcde\r\nf")), "abcde|f")
        eq("vt long wrap", screen(run(40, 3, d100)), "$d40|$d40|01234567890123456789")
        eq("vt long wrap cursor", cur(run(40, 3, d100)), "20,21")
        eq("vt long scroll", screen(run(40, 2, d100)), "$d40|01234567890123456789")
        eq("vt long after wrap", screen(feed(run(40, 2, d40), d40)), "$d40|$d40")
        eq("vt long nowrap", screen(run(40, 2, "$E[?7l${d40}abcdefghij")), "012345678901234567890123456789012345678j|")
        eq("vt long insert", screen(run(40, 1, "abc\r$E[4h$d40")), d40)
        eq("vt long insert keeps", screen(run(40, 1, "abc\r$E[4h0123456789012345678901234567890123")), "0123456789012345678901234567890123abc")
        eq("vt long mid row", screen(run(40, 2, "ab$E[31m$d40")), "ab01234567890123456789012345678901234567|89")
        eq("vt crlf", screen(run(10, 3, "ab\r\ncd")), "ab|cd|"); eq("vt lf keeps column", screen(run(10, 3, "ab\ncd")), "ab|  cd|")
        eq("vt backspace", screen(run(10, 1, "abc\u0008\u0008X")), "aXc"); eq("vt tab", screen(run(20, 1, "a\tb")), "a       b")
    }
    @Test fun vt_cursor_and_erase() {
        eq("vt cup", screen(run(5, 3, "$E[2;3HX")), "|  X|")
        eq("vt cursor moves", screen(run(6, 3, "$E[3;3H$E[AU$E[2DL$E[BD$E[CR")), "| LU|  D R")
        eq("vt cha vpa", screen(run(6, 3, "$E[4GA$E[3dB")), "   A||    B")
        eq("vt cursor clamps", cur(run(5, 3, "$E[99;99H")), "4,21")
        eq("vt el 0", screen(run(10, 1, "abcdef$E[1;3H$E[K")), "ab")
        eq("vt el 1", screen(run(10, 1, "abcdef$E[1;3H$E[1K")), "   def")
        eq("vt el 2", screen(run(10, 1, "abcdef$E[2K")), "")
        eq("vt ed 0", screen(run(5, 3, "aaa\r\nbbb\r\nccc$E[2;2H$E[J")), "aaa|b|")
        eq("vt ed 1", screen(run(5, 3, "aaa\r\nbbb\r\nccc$E[2;2H$E[1J")), "|  b|ccc")
        eq("vt ed 2", screen(run(5, 3, "aaa\r\nbbb$E[2J")), "||")
        eq("vt ich", screen(run(10, 1, "abcdef\r$E[2@")), "  abcdef")
        eq("vt dch", screen(run(10, 1, "abcdef\r$E[2P")), "cdef")
        eq("vt ech", screen(run(10, 1, "abcdef\r$E[2X")), "  cdef")
        eq("vt il", screen(run(5, 3, "a\r\nb\r\nc$E[2;1H$E[L")), "a||b")
        eq("vt dl", screen(run(5, 3, "a\r\nb\r\nc$E[1;1H$E[M")), "b|c|")
        eq("vt scroll", screen(run(5, 3, "1\r\n2\r\n3\r\n4")), "2|3|4")
        eq("vt ri", screen(run(5, 3, "1\r\n2$E[H${E}M0")), "0|1|2")
    }
    @Test fun vt_colors() {
        eq("vt sgr red", row0(run(10, 1, "$E[31mR$E[0mn")), "[R 14707829 855826 00][n 14277092 855826 00]")
        eq("vt sgr bold underline", row0(run(10, 1, "$E[1;4mB")), "[B 14277092 855826 11]")
        eq("vt sgr bright bg", row0(run(10, 1, "$E[92;104mG")), "[G 11921552 8570111 00]")
        eq("vt sgr 256", row0(run(10, 1, "$E[38;5;196;48;5;244mX")), "[X 16711680 8421504 00]")
        eq("vt sgr truecolor", row0(run(10, 1, "$E[38;2;1;2;3;48;2;255;128;0mT")), "[T 66051 16744448 00]")
        eq("vt sgr inverse", row0(run(10, 1, "$E[7mI$E[27mJ")), "[I 855826 14277092 00][J 14277092 855826 00]")
        eq("vt sgr default", row0(run(10, 1, "$E[31;42mA$E[39mB$E[49mC")), "[A 14707829 10011513 00][B 14277092 10011513 00][C 14277092 855826 00]")
        eq("vt erase uses bg", row0(run(4, 1, "$E[41m$E[2K")), "[     14277092 14707829 00]")
    }
    @Test fun vt_regions_and_screens() {
        val rs = run(10, 5, "a\r\nb\r\nc\r\nd\r\ne$E[2;4r$E[4;1H\n\nX"); eq("vt scroll region", screen(rs), "a|d||X|e")
        eq("vt su", screen(run(5, 3, "1\r\n2\r\n3$E[S")), "2|3|"); eq("vt sd", screen(run(5, 3, "1\r\n2\r\n3$E[T")), "|1|2")
        val top = run(5, 4, "$E[1;3ra\r\nb\r\nc\r\nd"); eq("vt region at top", screen(top), "b|c|d|"); eq("vt region at top history", top.history().toString(), "1")
        eq("vt origin mode", screen(run(5, 4, "$E[2;3r$E[?6h$E[1;1HX")), "|X||")
        eq("vt cnl", screen(run(5, 2, "ab$E[Ec")), "ab|c")
        eq("vt alt 47", screen(run(5, 1, "m$E[?47hx$E[?47l")), "m")
        eq("vt da2", bytes(run(10, 1, "$E[>c").replies()), "^[[>1;10;0c")
        val alt1 = run(10, 3, "main$E[?1049h$E[2;2Halt"); val alt2 = feed(run(10, 3, "main$E[?1049h$E[2;2Halt"), "$E[?1049l")
        eq("vt alt screen", screen(alt1), "| alt|"); eq("vt alt restore", screen(alt2), "main||"); eq("vt alt cursor", cur(alt2), "4,01"); assertTrue(alt1.alt())
    }
    @Test fun vt_text_encodings_and_replies() {
        val split = Vt(10, 2); split.feed(byteArrayOf(97, 226.toByte(), 156.toByte())); split.feed(byteArrayOf(147.toByte(), 98)); eq("vt utf8 split", screen(split), "a✓b|")
        val bad = Vt(5, 1); bad.feed(byteArrayOf(97, 255.toByte(), 98)); eq("vt utf8 bad", screen(bad), "a�b")
        eq("vt wide", cur(run(10, 1, "中a")), "3,01"); eq("vt wide text", screen(run(10, 1, "中a")), "中a")
        eq("vt dec graphics", screen(run(10, 1, "$E(0lqk$E(Bq")), "┌─┐q")
        val dsr = run(10, 5, "$E[3;4H$E[6n"); eq("vt dsr", bytes(dsr.replies()), "^[[3;4R"); dsr.drain(); eq("vt drain", bytes(dsr.replies()), "")
        eq("vt da", bytes(run(10, 1, "$E[c").replies()), "^[[?1;2c")
        eq("vt title bel", run(10, 1, "$E]0;hello\u0007x").title(), "hello")
        eq("vt title st", run(10, 1, "$E]2;wörld$E\\x").title(), "wörld")
        eq("vt title text", screen(run(10, 1, "$E]2;t$E\\x")), "x")
        eq("vt osc split", feed(run(10, 1, "$E]0;ab"), "c\u0007").title(), "abc")
        eq("vt unknown csi", screen(run(10, 1, "a$E[?1000;1006h$E[>4;1m$E[5 qb")), "ab")
        eq("vt dcs skipped", screen(run(10, 1, "a${E}P+q544e$E\\b")), "ab")
        eq("vt cursor hide", cur(run(10, 1, "$E[?25l")), "0,00")
        eq("vt save restore", cur(run(10, 3, "$E[2;5H${E}7$E[H${E}8")), "4,11")
        eq("vt reset", screen(run(10, 2, "abc${E}c")), "|")
    }
    @Test fun vt_scrollback_and_resize() {
        val sb = run(6, 3, "1\r\n2\r\n3\r\n4\r\n5")
        eq("vt scrollback", scrolled(sb, 1), "2|3|4"); eq("vt scrollback all", scrolled(sb, 9), "1|2|3"); eq("vt history", sb.history().toString(), "2")
        var v = run(10, 2, "abcdefgh"); v.resize(4, 2); eq("vt resize narrow", screen(v), "abcd|")
        v = run(4, 3, "a\r\nb\r\nc"); v.resize(4, 2); eq("vt resize short", screen(v), "b|c"); eq("vt resize short history", scrolled(v, 1), "a|b")
        v = run(4, 2, "ab"); v.resize(6, 3); eq("vt resize grow", screen(v), "ab||")
    }
    @Test fun vt_keys_and_paste() {
        val plain = Vt(10, 2); val app = run(10, 2, "$E[?1h$E[?2004h")
        fun k(v: Vt, ks: Int, t: Int, m: Int) = bytes(v.key(ks, t, m))
        eq("vt key kp enter", k(plain, 65421, 13, 0), "<13>"); eq("vt key return", k(plain, 65293, 13, 0), "<13>"); eq("vt key backspace", k(plain, 65288, 8, 0), "<127>")
        eq("vt key up", k(plain, 65362, 0, 0), "^[[A"); eq("vt key up app", k(app, 65362, 0, 0), "^[OA"); eq("vt key shift up", k(plain, 65362, 0, 1), "^[[1;2A")
        eq("vt key ctrl right", k(plain, 65363, 0, 4), "^[[1;5C"); eq("vt key ctrl c", k(plain, 99, 99, 4), "<3>"); eq("vt key ctrl c text", k(plain, 99, 3, 4), "<3>")
        eq("vt key alt x", k(plain, 120, 120, 8), "^[x"); eq("vt key text", k(plain, 233, 233, 0), "<195><169>")
        eq("vt key f1", k(plain, 65470, 0, 0), "^[OP"); eq("vt key f5", k(plain, 65474, 0, 0), "^[[15~"); eq("vt key delete", k(plain, 65535, 0, 0), "^[[3~")
        eq("vt key pgup ctrl", k(plain, 65365, 0, 4), "^[[5;5~"); eq("vt key backtab", k(plain, 65056, 0, 1), "^[[Z"); eq("vt key tab", k(plain, 65289, 9, 0), "<9>")
        eq("vt key escape", k(plain, 65307, 27, 0), "^["); eq("vt key shift", k(plain, 65505, 0, 1), "")
        eq("vt paste", bytes(plain.paste("a\nb\r\nc")), "a<13>b<13>c"); eq("vt paste bracketed", bytes(app.paste("x\u001by")), "^[[200~xy^[[201~")
    }
    @Test fun bounded() {
        val v = Vt(100000, 100000); assertTrue(v.cols() <= VT_MAX_COLS && v.rows() <= VT_MAX_ROWS)
        repeat(5000) { v.feed("line\r\n") }; assertTrue(v.history() <= VT_SCROLLBACK)
        v.feed("$E]0;${"x".repeat(100000)}\u0007"); assertTrue(v.title().length <= 4096)
        repeat(5000) { v.feed("$E[6n") }; assertTrue(v.replies().size <= 4096)
    }
}
