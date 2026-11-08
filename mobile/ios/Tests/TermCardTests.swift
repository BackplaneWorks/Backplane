// TermCard's rows and states. Compiled with Backplane/Vt.swift and TermCard.swift outside the app: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
func texts(_ ls: [VLine]) -> String { ls.map { $0.runs.map(\.text).joined() }.joined(separator: "|") }

let v = Vt(cols: 10, rows: 6); v.feed("one\r\ntwo\r\nthree")
var r = TermCard.rows(v.lines(0), cursor: v.cursor, n: 8)
check(texts(r.lines) == "one|two|three" && r.cursorRow == 2, "blank rows below the cursor are left out", (texts(r.lines), r.cursorRow as Any))
r = TermCard.rows(v.lines(0), cursor: v.cursor, n: 2)
check(texts(r.lines) == "two|three" && r.cursorRow == 1, "only the last n rows", (texts(r.lines), r.cursorRow as Any))
v.feed("\r\n")   // the cursor on a blank row: it is shown
r = TermCard.rows(v.lines(0), cursor: v.cursor, n: 8)
check(texts(r.lines) == "one|two|three|" && r.cursorRow == 3, "a cursor on a blank row keeps it", (texts(r.lines), r.cursorRow as Any))
v.feed("\u{1b}[?25l")
r = TermCard.rows(v.lines(0), cursor: v.cursor, n: 8)
check(texts(r.lines) == "one|two|three" && r.cursorRow == nil, "a hidden cursor adds nothing", (texts(r.lines), r.cursorRow as Any))
r = TermCard.rows(Vt(cols: 4, rows: 3).lines(0), cursor: VCur(x: 0, y: 0, visible: false), n: 8)
check(r.lines.isEmpty && r.cursorRow == nil, "an empty screen has no rows", r.lines)
check(TermCard.live("running") && TermCard.live("done 0") && !TermCard.live("exited 1") && !TermCard.live("stopped") && !TermCard.live("failed to start"), "which states can be stopped", "")
print("term card: \(passed) passed, \(failed) failed"); exit(failed == 0 ? 0 : 1)
