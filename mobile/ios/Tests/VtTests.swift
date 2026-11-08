// Vt.swift against the cases of the hub's native/host/src/term/vt/tests.rs (same names, same expected strings). Compiled with Backplane/Vt.swift alone, outside the app:
// scripts/mobile/ios-unit-tests.sh.
import Foundation

let E = "\u{1b}"
var failed = 0, passed = 0
func eq(_ name: String, _ got: String, _ want: String) { if got == want { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got) want \(want)") } }
func ok(_ name: String, _ c: Bool) { eq(name, "\(c)", "true") }
func feed(_ v: Vt, _ s: String) -> Vt { v.feed(s); return v }
func run(_ c: Int, _ r: Int, _ s: String) -> Vt { feed(Vt(cols: c, rows: r), s) }
func text(_ l: VLine) -> String { l.runs.map(\.text).joined() }
func screen(_ v: Vt) -> String { v.lines(0).map(text).joined(separator: "|") }
func scrolled(_ v: Vt, _ n: Int) -> String { v.lines(n).map(text).joined(separator: "|") }
func row0(_ v: Vt) -> String { v.lines(0)[0].runs.map { "[\($0.text) \($0.fg) \($0.bg) \($0.bold ? 1 : 0)\($0.underline ? 1 : 0)]" }.joined() }
func cur(_ v: Vt) -> String { let c = v.cursor; return "\(c.x),\(c.y)\(c.visible ? 1 : 0)" }
func bytes(_ b: [UInt8]) -> String { b.map { $0 == 27 ? "^[" : ($0 < 32 || $0 >= 127) ? "<\($0)>" : String(UnicodeScalar($0)) }.joined() }

func textAndWrap() {
    let d40 = "0123456789012345678901234567890123456789"; let d100 = d40 + d40 + "01234567890123456789"
    eq("vt text", screen(run(10, 3, "hi")), "hi||")
    let wrap = run(10, 3, "hello world!")
    eq("vt wrap", screen(wrap), "hello worl|d!|"); eq("vt wrap cursor", cur(wrap), "2,11")
    eq("vt pending wrap", cur(run(5, 2, "abcde")), "4,01")
    eq("vt pending wrap cr", screen(run(5, 2, "abcde\r\nf")), "abcde|f")
    eq("vt long wrap", screen(run(40, 3, d100)), "\(d40)|\(d40)|01234567890123456789")
    eq("vt long wrap cursor", cur(run(40, 3, d100)), "20,21")
    eq("vt long scroll", screen(run(40, 2, d100)), "\(d40)|01234567890123456789")
    eq("vt long after wrap", screen(feed(run(40, 2, d40), d40)), "\(d40)|\(d40)")
    eq("vt long nowrap", screen(run(40, 2, "\(E)[?7l\(d40)abcdefghij")), "012345678901234567890123456789012345678j|")
    eq("vt long insert", screen(run(40, 1, "abc\r\(E)[4h\(d40)")), d40)
    eq("vt long insert keeps", screen(run(40, 1, "abc\r\(E)[4h0123456789012345678901234567890123")), "0123456789012345678901234567890123abc")
    eq("vt long mid row", screen(run(40, 2, "ab\(E)[31m\(d40)")), "ab01234567890123456789012345678901234567|89")
    eq("vt crlf", screen(run(10, 3, "ab\r\ncd")), "ab|cd|"); eq("vt lf keeps column", screen(run(10, 3, "ab\ncd")), "ab|  cd|")
    eq("vt backspace", screen(run(10, 1, "abc\u{8}\u{8}X")), "aXc"); eq("vt tab", screen(run(20, 1, "a\tb")), "a       b")
}
func cursorAndErase() {
    eq("vt cup", screen(run(5, 3, "\(E)[2;3HX")), "|  X|")
    eq("vt cursor moves", screen(run(6, 3, "\(E)[3;3H\(E)[AU\(E)[2DL\(E)[BD\(E)[CR")), "| LU|  D R")
    eq("vt cha vpa", screen(run(6, 3, "\(E)[4GA\(E)[3dB")), "   A||    B")
    eq("vt cursor clamps", cur(run(5, 3, "\(E)[99;99H")), "4,21")
    eq("vt el 0", screen(run(10, 1, "abcdef\(E)[1;3H\(E)[K")), "ab")
    eq("vt el 1", screen(run(10, 1, "abcdef\(E)[1;3H\(E)[1K")), "   def")
    eq("vt el 2", screen(run(10, 1, "abcdef\(E)[2K")), "")
    eq("vt ed 0", screen(run(5, 3, "aaa\r\nbbb\r\nccc\(E)[2;2H\(E)[J")), "aaa|b|")
    eq("vt ed 1", screen(run(5, 3, "aaa\r\nbbb\r\nccc\(E)[2;2H\(E)[1J")), "|  b|ccc")
    eq("vt ed 2", screen(run(5, 3, "aaa\r\nbbb\(E)[2J")), "||")
    eq("vt ich", screen(run(10, 1, "abcdef\r\(E)[2@")), "  abcdef")
    eq("vt dch", screen(run(10, 1, "abcdef\r\(E)[2P")), "cdef")
    eq("vt ech", screen(run(10, 1, "abcdef\r\(E)[2X")), "  cdef")
    eq("vt il", screen(run(5, 3, "a\r\nb\r\nc\(E)[2;1H\(E)[L")), "a||b")
    eq("vt dl", screen(run(5, 3, "a\r\nb\r\nc\(E)[1;1H\(E)[M")), "b|c|")
    eq("vt scroll", screen(run(5, 3, "1\r\n2\r\n3\r\n4")), "2|3|4")
    eq("vt ri", screen(run(5, 3, "1\r\n2\(E)[H\(E)M0")), "0|1|2")
}
func colors() {
    eq("vt sgr red", row0(run(10, 1, "\(E)[31mR\(E)[0mn")), "[R 14707829 855826 00][n 14277092 855826 00]")
    eq("vt sgr bold underline", row0(run(10, 1, "\(E)[1;4mB")), "[B 14277092 855826 11]")
    eq("vt sgr bright bg", row0(run(10, 1, "\(E)[92;104mG")), "[G 11921552 8570111 00]")
    eq("vt sgr 256", row0(run(10, 1, "\(E)[38;5;196;48;5;244mX")), "[X 16711680 8421504 00]")
    eq("vt sgr truecolor", row0(run(10, 1, "\(E)[38;2;1;2;3;48;2;255;128;0mT")), "[T 66051 16744448 00]")
    eq("vt sgr inverse", row0(run(10, 1, "\(E)[7mI\(E)[27mJ")), "[I 855826 14277092 00][J 14277092 855826 00]")
    eq("vt sgr default", row0(run(10, 1, "\(E)[31;42mA\(E)[39mB\(E)[49mC")), "[A 14707829 10011513 00][B 14277092 10011513 00][C 14277092 855826 00]")
    eq("vt erase uses bg", row0(run(4, 1, "\(E)[41m\(E)[2K")), "[     14277092 14707829 00]")
}
func regionsAndScreens() {
    let rs = run(10, 5, "a\r\nb\r\nc\r\nd\r\ne\(E)[2;4r\(E)[4;1H\n\nX"); eq("vt scroll region", screen(rs), "a|d||X|e")
    eq("vt su", screen(run(5, 3, "1\r\n2\r\n3\(E)[S")), "2|3|"); eq("vt sd", screen(run(5, 3, "1\r\n2\r\n3\(E)[T")), "|1|2")
    let top = run(5, 4, "\(E)[1;3ra\r\nb\r\nc\r\nd"); eq("vt region at top", screen(top), "b|c|d|"); eq("vt region at top history", "\(top.history)", "1")
    eq("vt origin mode", screen(run(5, 4, "\(E)[2;3r\(E)[?6h\(E)[1;1HX")), "|X||")
    eq("vt cnl", screen(run(5, 2, "ab\(E)[Ec")), "ab|c")
    eq("vt alt 47", screen(run(5, 1, "m\(E)[?47hx\(E)[?47l")), "m")
    eq("vt da2", bytes(run(10, 1, "\(E)[>c").replies), "^[[>1;10;0c")
    let alt1 = run(10, 3, "main\(E)[?1049h\(E)[2;2Halt"); let alt2 = feed(run(10, 3, "main\(E)[?1049h\(E)[2;2Halt"), "\(E)[?1049l")
    eq("vt alt screen", screen(alt1), "| alt|"); eq("vt alt restore", screen(alt2), "main||"); eq("vt alt cursor", cur(alt2), "4,01"); ok("vt alt", alt1.alt)
}
func encodingsAndReplies() {
    let split = Vt(cols: 10, rows: 2); split.feed([97, 226, 156]); split.feed([147, 98]); eq("vt utf8 split", screen(split), "a✓b|")
    let bad = Vt(cols: 5, rows: 1); bad.feed([97, 255, 98]); eq("vt utf8 bad", screen(bad), "a\u{FFFD}b")
    eq("vt wide", cur(run(10, 1, "中a")), "3,01"); eq("vt wide text", screen(run(10, 1, "中a")), "中a")
    eq("vt dec graphics", screen(run(10, 1, "\(E)(0lqk\(E)(Bq")), "┌─┐q")
    let dsr = run(10, 5, "\(E)[3;4H\(E)[6n"); eq("vt dsr", bytes(dsr.replies), "^[[3;4R"); dsr.drain(); eq("vt drain", bytes(dsr.replies), "")
    eq("vt da", bytes(run(10, 1, "\(E)[c").replies), "^[[?1;2c")
    eq("vt title bel", run(10, 1, "\(E)]0;hello\u{7}x").title, "hello")
    eq("vt title st", run(10, 1, "\(E)]2;wörld\(E)\\x").title, "wörld")
    eq("vt title text", screen(run(10, 1, "\(E)]2;t\(E)\\x")), "x")
    eq("vt osc split", feed(run(10, 1, "\(E)]0;ab"), "c\u{7}").title, "abc")
    eq("vt unknown csi", screen(run(10, 1, "a\(E)[?1000;1006h\(E)[>4;1m\(E)[5 qb")), "ab")
    eq("vt dcs skipped", screen(run(10, 1, "a\(E)P+q544e\(E)\\b")), "ab")
    eq("vt cursor hide", cur(run(10, 1, "\(E)[?25l")), "0,00")
    eq("vt save restore", cur(run(10, 3, "\(E)[2;5H\(E)7\(E)[H\(E)8")), "4,11")
    eq("vt reset", screen(run(10, 2, "abc\(E)c")), "|")
}
func scrollbackAndResize() {
    let sb = run(6, 3, "1\r\n2\r\n3\r\n4\r\n5")
    eq("vt scrollback", scrolled(sb, 1), "2|3|4"); eq("vt scrollback all", scrolled(sb, 9), "1|2|3"); eq("vt history", "\(sb.history)", "2")
    var v = run(10, 2, "abcdefgh"); v.resize(cols: 4, rows: 2); eq("vt resize narrow", screen(v), "abcd|")
    v = run(4, 3, "a\r\nb\r\nc"); v.resize(cols: 4, rows: 2); eq("vt resize short", screen(v), "b|c"); eq("vt resize short history", scrolled(v, 1), "a|b")
    v = run(4, 2, "ab"); v.resize(cols: 6, rows: 3); eq("vt resize grow", screen(v), "ab||")
    // a snapshot at a new size keeps the history
    let k = Vt(cols: 3, rows: 2, keepingHistoryOf: sb); eq("vt keeps history", "\(k.history) \(screen(k))", "2 |")
}
func keysAndPaste() {
    let plain = Vt(cols: 10, rows: 2); let app = run(10, 2, "\(E)[?1h\(E)[?2004h")
    func k(_ v: Vt, _ ks: UInt32, _ t: UInt32, _ m: UInt32) -> String { bytes(v.key(ks, t, m)) }
    eq("vt key kp enter", k(plain, 65421, 13, 0), "<13>"); eq("vt key return", k(plain, 65293, 13, 0), "<13>"); eq("vt key backspace", k(plain, 65288, 8, 0), "<127>")
    eq("vt key up", k(plain, 65362, 0, 0), "^[[A"); eq("vt key up app", k(app, 65362, 0, 0), "^[OA"); eq("vt key shift up", k(plain, 65362, 0, 1), "^[[1;2A")
    eq("vt key ctrl right", k(plain, 65363, 0, 4), "^[[1;5C"); eq("vt key ctrl c", k(plain, 99, 99, 4), "<3>"); eq("vt key ctrl c text", k(plain, 99, 3, 4), "<3>")
    eq("vt key alt x", k(plain, 120, 120, 8), "^[x"); eq("vt key text", k(plain, 233, 233, 0), "<195><169>")
    eq("vt key f1", k(plain, 65470, 0, 0), "^[OP"); eq("vt key f5", k(plain, 65474, 0, 0), "^[[15~"); eq("vt key delete", k(plain, 65535, 0, 0), "^[[3~")
    eq("vt key pgup ctrl", k(plain, 65365, 0, 4), "^[[5;5~"); eq("vt key backtab", k(plain, 65056, 0, 1), "^[[Z"); eq("vt key tab", k(plain, 65289, 9, 0), "<9>")
    eq("vt key escape", k(plain, 65307, 27, 0), "^["); eq("vt key shift", k(plain, 65505, 0, 1), "")
    eq("vt paste", bytes(plain.paste("a\nb\r\nc")), "a<13>b<13>c"); eq("vt paste bracketed", bytes(app.paste("x\u{1b}y")), "^[[200~xy^[[201~")
}
func bounded() {
    let v = Vt(cols: 100000, rows: 100000); ok("bounded size", v.cols <= vtMaxCols && v.rows <= vtMaxRows)
    for _ in 0..<5000 { v.feed("line\r\n") }; ok("bounded history", v.history <= vtScrollback)
    v.feed("\(E)]0;\(String(repeating: "x", count: 100000))\u{7}"); ok("bounded title", v.title.utf8.count <= 4096)
    for _ in 0..<5000 { v.feed("\(E)[6n") }; ok("bounded replies", v.replies.count <= 4096)
}
textAndWrap(); cursorAndErase(); colors(); regionsAndScreens(); encodingsAndReplies(); scrollbackAndResize(); keysAndPaste(); bounded()
print("vt: \(passed) passed, \(failed) failed"); exit(failed == 0 ? 0 : 1)
