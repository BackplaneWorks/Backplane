package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

// The Diff tab's parser: files, states, totals and line numbers from the hub's unified diff (the iOS twin: DiffModelTests.swift).
class DiffModelTest {
    private val two = """
        |diff --git a/src/main.c b/src/main.c
        |index 1111111..2222222 100644
        |--- a/src/main.c
        |+++ b/src/main.c
        |@@ -10,4 +10,5 @@ int main(void)
        | a
        |-b
        |+B
        |+C
        | d
        |@@ -40,2 +41,2 @@
        |-x
        |+y
        |diff --git a/notes.md b/notes.md
        |new file mode 100644
        |index 0000000..3333333
        |--- /dev/null
        |+++ b/notes.md
        |@@ -0,0 +1,2 @@
        |+one
        |+two
        |\ No newline at end of file
        |""".trimMargin()

    @Test fun two_files_with_hunks_and_a_new_one() {
        val p = DiffModel.parse(two)
        assertEquals(2, p.files.size)
        val (m, n) = p.files
        assertEquals("src/main.c", m.path); assertEquals("", m.state); assertEquals(3, m.added); assertEquals(2, m.removed)
        assertEquals(listOf('@', ' ', '-', '+', '+', ' ', '@', '-', '+'), m.lines.map { it.kind })
        assertEquals("@@ -10,4 +10,5 @@ int main(void)", m.lines[0].text)
        assertEquals("notes.md", n.path); assertEquals("new", n.state); assertEquals(2, n.added); assertEquals(0, n.removed)
        assertEquals('\\', n.lines.last().kind)
        assertEquals(5, p.added); assertEquals(2, p.removed)
    }

    @Test fun line_numbers_of_the_first_hunk() {
        val l = DiffModel.parse(two).files[0].lines
        assertEquals(DiffModel.Line(' ', "a", 10, 10), l[1])
        assertEquals(DiffModel.Line('-', "b", 11, 0), l[2])
        assertEquals(DiffModel.Line('+', "B", 0, 11), l[3])
        assertEquals(DiffModel.Line('+', "C", 0, 12), l[4])
        assertEquals(DiffModel.Line(' ', "d", 12, 13), l[5])
        assertEquals(DiffModel.Line('-', "x", 40, 0), l[7])
        assertEquals(DiffModel.Line('+', "y", 0, 41), l[8])
    }

    @Test fun a_deleted_file_too_big_to_show() {
        val p = DiffModel.parse("diff --git a/FrontEnd.kicad_sch b/FrontEnd.kicad_sch\ndeleted file mode 100644\nindex 7cbc5af..0000000\n--- a/FrontEnd.kicad_sch\n+++ /dev/null\n@@ -0,0 +0,0 @@\n\\ Too big to show here: +0 -36792 lines\n")
        val f = p.files.single()
        assertEquals("FrontEnd.kicad_sch", f.path); assertEquals("deleted", f.state)
        assertEquals(0, f.added); assertEquals(36792, f.removed)
        assertEquals(listOf('@', '\\'), f.lines.map { it.kind })
        assertEquals(36792, p.removed)
    }

    @Test fun a_rename() {
        val f = DiffModel.parse("diff --git a/old/a.txt b/new/b.txt\nsimilarity index 90%\nrename from old/a.txt\nrename to new/b.txt\n@@ -1 +1 @@\n-x\n+y\n").files.single()
        assertEquals("new/b.txt", f.path); assertEquals("old/a.txt", f.from); assertEquals("renamed", f.state)
        assertEquals(1, f.added); assertEquals(1, f.removed)
    }

    @Test fun a_binary_file_and_quoted_paths() {
        val p = DiffModel.parse("diff --git \"a/my board.png\" \"b/my board.png\"\nindex 1..2 100644\nBinary files \"a/my board.png\" and \"b/my board.png\" differ\n")
        val f = p.files.single()
        assertEquals("my board.png", f.path); assertEquals("binary", f.state); assertTrue(f.lines.isEmpty())
    }

    @Test fun empty_text_has_no_files() {
        assertTrue(DiffModel.parse("").files.isEmpty())
        assertTrue(DiffModel.parse("warning: something\n").files.isEmpty())
    }

    @Test fun lines_stop_at_the_cap_and_totals_go_on() {
        val p = DiffModel.parse(two, maxLines = 3)
        assertEquals(3, p.files.sumOf { it.lines.size })
        assertEquals(5, p.added); assertEquals(2, p.removed)
        assertEquals(listOf(true, true), p.files.map { it.cut })
        assertEquals(listOf(false, false), DiffModel.parse(two).files.map { it.cut })
    }
}
