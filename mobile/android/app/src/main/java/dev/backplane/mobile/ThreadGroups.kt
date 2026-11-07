package dev.backplane.mobile

/** Which threads sit under a project and which behind its "Settled (N)" row. Pure: the hub decides what is settled (`settled`, `settledAt`); this only splits and orders (ThreadGroupsTest). */
object ThreadGroups {
    data class Split(val active: List<DThread>, val settled: List<DThread>)
    /** A project's threads, `hidden` (bots' threads) left out: active ones first (pinned, then newest activity), settled ones newest settle first. */
    fun split(all: List<DThread>, project: String, hidden: Set<String> = emptySet()): Split {
        val mine = all.filter { it.project == project && !it.archived && it.id !in hidden }
        val active = mine.filter { !it.settled }.sortedWith(compareByDescending<DThread> { it.pinned }.thenByDescending { it.updatedAt }.thenBy { it.id })
        val settled = mine.filter { it.settled }.sortedWith(compareByDescending<DThread> { it.settledAt }.thenBy { it.id })
        return Split(active, settled)
    }
    /** The number on the row: the hub's count when it sends one (the list may hold only a page), else what is loaded. */
    fun count(hub: Int?, loaded: Int) = maxOf(hub ?: 0, loaded)
    fun title(n: Int) = "Settled ($n)"
}
