package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

// Conversation history (`history.list`, feature history): what the home list shows under the group's row (ios: Tests/HistoryGroupTests.swift, the same cases).
class DHistoryTest {
    private fun t(id: String) = DThread(id, "scratch", id, "idle", 0, false, false, false, "claude", "", "", 0, history = true)
    private val h = DHistory(count = 3, live = listOf(t("live")), rows = listOf(t("old1"), t("live"), t("old2")), next = "9:old2")
    private fun ids(l: List<DThread>) = l.map { it.id }

    @Test fun aLiveChatAlwaysHasItsRowAndTheRestOnlyWhileOpen() {
        assertEquals(listOf("live"), ids(h.shown(open = false)))
        assertEquals(listOf("live", "old1", "old2"), ids(h.shown(open = true)))
    }
    @Test fun theRowCountsEveryChat() { assertEquals("Conversation history (3)", h.title); assertEquals(emptyList<DThread>(), DHistory().shown(true)) }
}
