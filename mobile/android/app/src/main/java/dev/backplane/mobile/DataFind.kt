package dev.backplane.mobile

// The find bar's stepping and the search snippet's marks. The matches and their order are the hub's (`find`: oldest first; `search`: newest first); this only says which
// one is current, where next and previous go, the "n of total" words and which letters of a snippet are the query's. Pure: DataFindTest.
object DataFind {
    /** The current match once results arrive: the one shown before when it is still found (the same words, a new message came), else the newest (the last). -1 for none. */
    fun pick(msgs: List<String>, before: String?): Int = when {
        msgs.isEmpty() -> -1
        before != null && before in msgs -> msgs.lastIndexOf(before)
        else -> msgs.size - 1
    }
    /** Next (+1, newer) or previous (-1, older), wrapping round at either end. */
    fun step(cur: Int, n: Int, dir: Int): Int = if (n <= 0) -1 else if (cur !in 0 until n) (if (dir < 0) n - 1 else 0) else ((cur + dir) % n + n) % n
    /** "3 of 12"; a hub that stopped at its cap says so with "+". Nothing found: "no match". The words come from the caller (the catalog's, on screen). */
    fun count(cur: Int, n: Int, total: Int, more: Boolean, none: String = "no match", of: (Int, Int) -> String = { a, b -> "$a of $b" }): String =
        if (n == 0) none else of(cur + 1, maxOf(total, n)) + if (more) "+" else ""
    /** Where each word of `q` is in `text`, any case, overlapping ones joined, in order. */
    fun marks(text: String, q: String): List<IntRange> {
        val hits = q.split(Regex("\\s+")).filter { it.isNotEmpty() }.flatMap { w ->
            generateSequence(text.indexOf(w, ignoreCase = true).takeIf { it >= 0 }) { i -> text.indexOf(w, i + 1, ignoreCase = true).takeIf { it >= 0 } }.map { it until it + w.length }.toList()
        }.sortedBy { it.first }
        val out = mutableListOf<IntRange>()
        for (r in hits) { val l = out.lastOrNull(); if (l != null && r.first <= l.last + 1) out[out.size - 1] = l.first..maxOf(l.last, r.last) else out.add(r) }
        return out
    }
    /** Who said it, as the hit's line says it. */
    fun who(role: String) = if (role == "user") "you said" else "agent said"
}
