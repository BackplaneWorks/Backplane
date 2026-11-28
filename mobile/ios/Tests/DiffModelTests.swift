import Foundation

// swiftc DiffModel.swift main.swift (scripts/mobile/ios-unit-tests.sh): the Diff tab's parser, files, states, totals and line numbers (the Android twin: DiffModelTest.kt)
var failed = 0
func check(_ ok: Bool, _ what: String) { if !ok { failed += 1; print("FAIL", what) } }

let two = [
    "diff --git a/src/main.c b/src/main.c",
    "index 1111111..2222222 100644",
    "--- a/src/main.c",
    "+++ b/src/main.c",
    "@@ -10,4 +10,5 @@ int main(void)",
    " a",
    "-b",
    "+B",
    "+C",
    " d",
    "@@ -40,2 +41,2 @@",
    "-x",
    "+y",
    "diff --git a/notes.md b/notes.md",
    "new file mode 100644",
    "index 0000000..3333333",
    "--- /dev/null",
    "+++ b/notes.md",
    "@@ -0,0 +1,2 @@",
    "+one",
    "+two",
    "\\ No newline at end of file",
    "",
].joined(separator: "\n")

// two files: one modified with two hunks, one new
let p = DiffModel.parse(two)
check(p.files.count == 2, "two files \(p.files.count)")
if p.files.count == 2 {
    let m = p.files[0], n = p.files[1]
    check(m.path == "src/main.c" && m.state == "" && m.added == 3 && m.removed == 2, "modified file \(m.path) \(m.state) +\(m.added) -\(m.removed)")
    check(String(m.lines.map { $0.kind }) == "@ -++ @-+", "kinds \(String(m.lines.map { $0.kind }))")
    check(m.lines.first?.text == "@@ -10,4 +10,5 @@ int main(void)", "hunk head kept whole")
    check(n.path == "notes.md" && n.state == "new" && n.added == 2 && n.removed == 0, "new file")
    check(n.lines.last?.kind == "\\", "no newline note")
    // line numbers of the first hunk
    let l = m.lines
    check(l[1] == DiffModel.Line(kind: " ", text: "a", old: 10, new: 10), "context 10/10")
    check(l[2] == DiffModel.Line(kind: "-", text: "b", old: 11, new: 0), "removed 11")
    check(l[3] == DiffModel.Line(kind: "+", text: "B", old: 0, new: 11), "added 11")
    check(l[4] == DiffModel.Line(kind: "+", text: "C", old: 0, new: 12), "added 12")
    check(l[5] == DiffModel.Line(kind: " ", text: "d", old: 12, new: 13), "context 12/13")
    check(l[7] == DiffModel.Line(kind: "-", text: "x", old: 40, new: 0), "second hunk removed 40")
    check(l[8] == DiffModel.Line(kind: "+", text: "y", old: 0, new: 41), "second hunk added 41")
}
check(p.added == 5 && p.removed == 2, "totals +\(p.added) -\(p.removed)")

// a deleted file too big to show
let big = DiffModel.parse("diff --git a/FrontEnd.kicad_sch b/FrontEnd.kicad_sch\ndeleted file mode 100644\nindex 7cbc5af..0000000\n--- a/FrontEnd.kicad_sch\n+++ /dev/null\n@@ -0,0 +0,0 @@\n\\ Too big to show here: +0 -36792 lines\n")
check(big.files.count == 1, "one big file")
if let f = big.files.first {
    check(f.path == "FrontEnd.kicad_sch" && f.state == "deleted", "deleted \(f.path) \(f.state)")
    check(f.added == 0 && f.removed == 36792, "too big counts +\(f.added) -\(f.removed)")
    check(String(f.lines.map { $0.kind }) == "@\\", "too big lines kept")
}
check(big.removed == 36792, "big total")

// a rename
let ren = DiffModel.parse("diff --git a/old/a.txt b/new/b.txt\nsimilarity index 90%\nrename from old/a.txt\nrename to new/b.txt\n@@ -1 +1 @@\n-x\n+y\n")
if let f = ren.files.first {
    check(f.path == "new/b.txt" && f.from == "old/a.txt" && f.state == "renamed", "rename \(f.path) \(f.from) \(f.state)")
    check(f.added == 1 && f.removed == 1, "rename counts")
} else { check(false, "rename parsed") }

// a binary file with quoted paths
let bin = DiffModel.parse("diff --git \"a/my board.png\" \"b/my board.png\"\nindex 1..2 100644\nBinary files \"a/my board.png\" and \"b/my board.png\" differ\n")
if let f = bin.files.first {
    check(f.path == "my board.png" && f.state == "binary" && f.lines.isEmpty, "binary \(f.path) \(f.state)")
} else { check(false, "binary parsed") }

// empty text: no files
check(DiffModel.parse("").files.isEmpty, "empty")
check(DiffModel.parse("warning: something\n").files.isEmpty, "text before the first file")

// the line cap: lines stop, totals go on
let cap = DiffModel.parse(two, maxLines: 3)
check(cap.files.reduce(0) { $0 + $1.lines.count } == 3, "cap keeps 3 lines")
check(cap.added == 5 && cap.removed == 2, "cap totals")
check(cap.files.map { $0.cut } == [true, true] && p.files.map { $0.cut } == [false, false], "cut marks")

if failed == 0 { print("diff model: ok") }
exit(failed == 0 ? 0 : 1)
