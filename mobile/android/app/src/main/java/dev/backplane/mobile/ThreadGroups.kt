package dev.backplane.mobile

/** Which threads sit under a project and which behind its "Settled (N)" row. Pure: the hub decides what is settled (`settled`, `settledAt`); this only splits and orders (ThreadGroupsTest). */
object ThreadGroups {
    /** `all`: every thread of the project the groups were made from, children included (what [tree] nests from). */
    data class Split(val active: List<DThread>, val settled: List<DThread>, val all: List<DThread> = active + settled)
    /** A project's threads, `hidden` (bots' threads) left out: active ones first (pinned in the hub's pin order, then newest activity), settled ones newest settle first. A subagent or
     *  subthread whose parent is here is in neither group: it sits under its parent, wherever that is (the desktop's Kids.set). */
    fun split(all: List<DThread>, project: String, hidden: Set<String> = emptySet()): Split {
        val mine = all.filter { it.project == project && !it.archived && it.id !in hidden }
        val ids = mine.associateBy { it.id }
        val roots = mine.filter { parentIn(it, ids) == null }
        val active = roots.filter { !it.settled }.sortedWith(compareByDescending<DThread> { it.pinned }.thenBy { if (it.pinned) it.pinOrder else 0 }.thenByDescending { it.updatedAt }.thenBy { it.id })
        val settled = roots.filter { it.settled }.sortedWith(compareByDescending<DThread> { it.settledAt }.thenBy { it.id })
        return Split(active, settled, mine.sortedWith(compareByDescending<DThread> { it.updatedAt }.thenBy { it.id }))
    }
    /** The parent a thread sits under: its `parent` when that is among `ids` and the ancestry does not loop (Tree.display_parent); else null, a root. */
    private fun parentIn(t: DThread, ids: Map<String, DThread>): String? {
        val p = t.parent; if (p.isEmpty() || p == t.id || p !in ids) return null
        var cur = p; var n = 0
        while (n++ < 64) { if (cur == t.id) return null; cur = ids[cur]?.parent?.takeIf { it.isNotEmpty() && it in ids } ?: return p }
        return null
    }
    /** The number on the row: the hub's count when it sends one (the list may hold only a page), else what is loaded. */
    fun count(hub: Int?, loaded: Int) = maxOf(hub ?: 0, loaded)
    fun title(n: Int) = Msg.android_settled_title(n)

    /** A row of the tree: the thread, how deep it sits, how many children it has. */
    data class Node(val t: DThread, val depth: Int, val kids: Int)
    /** `roots` with their subagents and subthreads (`parent`, the hub's thread.parent) from `all` under them, depth first, children newest first, as the
     *  desktop's sidebar nests them. The children of a parent show only while it is in `open`. */
    fun tree(roots: List<DThread>, all: List<DThread>, open: Set<String>): List<Node> {
        val ids = all.associateBy { it.id }
        val kids = all.groupBy { parentIn(it, ids) }
        val out = mutableListOf<Node>()
        fun walk(t: DThread, d: Int) { val k = kids[t.id].orEmpty(); out += Node(t, d, k.size); if (t.id in open && d < 16) k.forEach { walk(it, d + 1) } }
        roots.forEach { walk(it, 0) }
        return out
    }
}
