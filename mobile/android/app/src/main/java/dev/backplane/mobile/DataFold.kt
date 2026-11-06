package dev.backplane.mobile

// A thread's rows as the desktop draws them: a run of tool calls and design steps is one row, opened with a tap (the hub's present/thread.rs `entries`, whose
// words test/mobile/fold_labels.json holds this file to: DataFoldTest). The same as the iOS app's DataFold.swift. Pure: entries in, rows out.
data class DFold(val id: String, val items: List<DEntry>, val label: String, val live: Boolean)

sealed interface DRow {
    val id: String
    data class Entry(val entry: DEntry) : DRow { override val id get() = entry.id }
    data class Fold(val fold: DFold) : DRow { override val id get() = "fold:" + fold.id }
}

object DataFold {
    fun folds(e: DEntry) = e.kind == "tool" || e.kind == "step"
    private fun calls(n: Int) = "$n tool ${if (n == 1) "call" else "calls"}"
    private fun steps(n: Int) = "$n design ${if (n == 1) "change" else "changes"}"

    fun label(run: List<DEntry>, live: Boolean): String {
        val tools = run.filter { it.kind == "tool" }
        val d = run.size - tools.size
        val last = tools.lastOrNull() ?: return steps(d)
        if (live) {
            val name = last.tool?.name ?: ""
            val first = last.text.split('\n').first().take(160).trim()
            val what = if (name == "To-do" || first.isEmpty()) name else first
            return calls(tools.size) + (if (d == 0) "" else " · " + steps(d)) + " · running: " + what
        }
        val names = tools.mapNotNull { it.tool?.name }.filter { it != "To-do" }.distinct()
        var out = calls(tools.size)
        if (names.isNotEmpty()) out += " · " + names.take(3).joinToString(", ") + if (names.size > 3) ", …" else ""
        if (d > 0) out += " · " + steps(d)
        return out
    }

    /** `running`: the thread's turn runs, so a run at the very end of the thread is still growing. */
    fun rows(entries: List<DEntry>, running: Boolean): List<DRow> {
        val out = ArrayList<DRow>(); var i = 0
        while (i < entries.size) {
            if (!folds(entries[i])) { out += DRow.Entry(entries[i]); i++; continue }
            var j = i
            while (j < entries.size && folds(entries[j])) j++
            val run = entries.subList(i, j); val live = running && j == entries.size
            out += DRow.Fold(DFold(run[0].id, run.toList(), label(run, live), live))
            i = j
        }
        return out
    }
}
