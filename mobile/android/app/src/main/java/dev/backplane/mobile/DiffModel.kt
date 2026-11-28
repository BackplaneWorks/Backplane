package dev.backplane.mobile

/** The Diff tab's model: the hub's unified `git diff` text (viewer.diff) as files with their state, totals and numbered lines.
 *  Pure: the view only draws what this returns (DiffModelTest; the iOS twin is DiffModel.swift). */
object DiffModel {
    /** One shown line: kind '+', '-', ' ', '@' (a hunk head, its whole text) or '\\' (a note such as "No newline"); text without the
     *  leading marker except for '@' and '\\'; old/new line numbers, 0 when the side has none. */
    data class Line(val kind: Char, val text: String, val old: Int, val new: Int)
    /** A changed file: path (the new one), from (the old path when renamed, else ""), state "", "new", "deleted", "renamed" or "binary";
     *  cut: some of its lines were dropped by the parse's line cap (the view ends it with "the rest is not shown"). */
    data class File(val path: String, val from: String, val state: String, val added: Int, val removed: Int, val lines: List<Line>, val cut: Boolean = false)
    data class Parsed(val files: List<File>, val added: Int, val removed: Int)

    private val tooBig = Regex("""\+(\d+) -(\d+) lines""")

    private class Acc(var path: String) {
        var from = ""; var state = ""; var added = 0; var removed = 0; var big: Pair<Int, Int>? = null
        var hunk = false; var old = 0; var new = 0; var cut = false
        val lines = ArrayList<Line>()
        fun done() = File(path, from, state, big?.first ?: added, big?.second ?: removed, lines, cut)
    }

    /** git quotes odd paths as "a/x y": strip the quotes (escapes stay as git wrote them). */
    private fun unquote(s: String) = if (s.length >= 2 && s.startsWith("\"") && s.endsWith("\"")) s.substring(1, s.length - 1) else s

    /** The new path from `diff --git a/X b/Y`. */
    private fun gitPath(rest: String): String {
        val r = rest.trim()
        if (r.endsWith("\"")) {
            val i = r.lastIndexOf(" \"b/")
            if (i >= 0) return unquote(r.substring(i + 1)).removePrefix("b/")
        }
        val i = r.lastIndexOf(" b/")
        return if (i >= 0) unquote(r.substring(i + 1)).removePrefix("b/") else unquote(r)
    }

    private fun hunkStart(s: String, sign: Char): Int {
        val i = s.indexOf(sign, 2); if (i < 0) return 0
        var j = i + 1; while (j < s.length && s[j].isDigit()) j++
        return s.substring(i + 1, j).toIntOrNull() ?: 0
    }

    fun parse(text: String, maxLines: Int = 4000): Parsed {
        val files = ArrayList<File>()
        var cur: Acc? = null
        var kept = 0
        val all = text.split('\n')
        val n = if (all.isNotEmpty() && all.last().isEmpty()) all.size - 1 else all.size
        fun keep(a: Acc, l: Line) { if (kept < maxLines) { a.lines.add(l); kept++ } else a.cut = true }
        for (k in 0 until n) {
            val raw = all[k].removeSuffix("\r")
            if (raw.startsWith("diff --git ")) { cur?.let { files.add(it.done()) }; cur = Acc(gitPath(raw.removePrefix("diff --git "))); continue }
            val a = cur ?: continue   // text before the first file
            if (raw.startsWith("@@")) {
                a.hunk = true; a.old = hunkStart(raw, '-'); a.new = hunkStart(raw, '+')
                keep(a, Line('@', raw, 0, 0)); continue
            }
            if (!a.hunk) {
                when {
                    raw.startsWith("new file mode") -> a.state = "new"
                    raw.startsWith("deleted file mode") -> a.state = "deleted"
                    raw.startsWith("rename from ") -> { a.from = unquote(raw.removePrefix("rename from ")); a.state = "renamed" }
                    raw.startsWith("rename to ") -> { a.path = unquote(raw.removePrefix("rename to ")); a.state = "renamed" }
                    raw.startsWith("Binary files ") || raw.startsWith("GIT binary patch") -> a.state = "binary"
                }
                continue   // index, ---, +++, modes, similarity: header lines, not shown
            }
            when (raw.firstOrNull()) {
                '+' -> { a.added++; keep(a, Line('+', raw.substring(1), 0, a.new)); a.new++ }
                '-' -> { a.removed++; keep(a, Line('-', raw.substring(1), a.old, 0)); a.old++ }
                '\\' -> {
                    tooBig.find(raw)?.let { m -> a.big = Pair(m.groupValues[1].toInt(), m.groupValues[2].toInt()) }
                    keep(a, Line('\\', raw, 0, 0))
                }
                ' ', null -> { keep(a, Line(' ', if (raw.isEmpty()) "" else raw.substring(1), a.old, a.new)); a.old++; a.new++ }
                else -> if (raw.startsWith("Binary files ")) a.state = "binary"
            }
        }
        cur?.let { files.add(it.done()) }
        return Parsed(files, files.sumOf { it.added }, files.sumOf { it.removed })
    }
}
