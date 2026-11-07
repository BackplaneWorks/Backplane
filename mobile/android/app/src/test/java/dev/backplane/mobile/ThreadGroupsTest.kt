package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

// The grouping of a project's threads under its "Settled (N)" row (ios: Tests/ThreadGroupsTests.swift, the same cases).
class ThreadGroupsTest {
    private fun t(id: String, project: String = "p", settled: Boolean = false, pinned: Boolean = false, archived: Boolean = false, updated: Long = 0, settledAt: Long = 0) =
        DThread(id, project, id, "idle", updated, pinned, settled, archived, "claude", "", "", 0, settledAt = settledAt)
    private val rows = listOf(
        t("a", updated = 10), t("b", updated = 30), t("c", pinned = true, updated = 5),
        t("s1", settled = true, updated = 99, settledAt = 100), t("s2", settled = true, updated = 1, settledAt = 200),
        t("x", project = "q", updated = 50), t("z", archived = true, updated = 60), t("bot", updated = 70),
    )
    private fun ids(l: List<DThread>) = l.map { it.id }

    @Test fun activeFirstPinnedThenNewestSettledByTheirSettleTime() {
        val g = ThreadGroups.split(rows, "p", setOf("bot"))
        assertEquals(listOf("c", "b", "a"), ids(g.active))
        assertEquals(listOf("s2", "s1"), ids(g.settled))
    }
    @Test fun aThreadSettledLiveMovesDownAndAnUnsettledOneMovesBackUp() {
        var live = rows.map { if (it.id == "b") it.copy(settled = true, settledAt = 300) else it }
        var g = ThreadGroups.split(live, "p", setOf("bot"))
        assertEquals(listOf("c", "a"), ids(g.active)); assertEquals(listOf("b", "s2", "s1"), ids(g.settled))
        live = live.map { if (it.id == "s1") it.copy(settled = false, settledAt = 0) else it }
        g = ThreadGroups.split(live, "p", setOf("bot"))
        assertEquals(listOf("c", "s1", "a"), ids(g.active)); assertEquals(listOf("b", "s2"), ids(g.settled))
    }
    @Test fun tiesAreByIdAndAnEmptyProjectIsEmpty() {
        assertEquals(listOf("k", "m"), ids(ThreadGroups.split(listOf(t("m", updated = 1), t("k", updated = 1)), "p").active))
        assertTrue(ThreadGroups.split(rows, "none").active.isEmpty())
    }
    @Test fun theRowShowsTheHubsCountWhenItSendsOne() {
        assertEquals(40, ThreadGroups.count(40, 3)); assertEquals(3, ThreadGroups.count(null, 3)); assertEquals(2, ThreadGroups.count(0, 2))
        assertEquals("Settled (7)", ThreadGroups.title(7))
    }
}
