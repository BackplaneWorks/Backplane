package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OutboxTest {
    private val two = Outbox().push("h", "t1", "first\nmore", "c1").push("h", "t2", "other", "c2").push("h", "t1", "second", "c3")

    @Test fun pushKeepsOrderAndIgnoresARepeatedClientId() {
        assertEquals(listOf("c1", "c2", "c3"), two.items.map { it.clientId })
        assertEquals(two, two.push("h", "t1", "again", "c1"))
    }

    @Test fun removeTakesOneOut() {
        val o = two.remove("c2")
        assertEquals(listOf("c1", "c3"), o.items.map { it.clientId })
        assertEquals(o, o.remove("nope"))
        assertFalse(o.has("h", "t2")); assertTrue(o.has("h", "t1"))
    }

    @Test fun rowsForAThreadAreUnsentWithEditAndRemove() {
        assertEquals(listOf(
            QRow("c1", "Unsent", "first", listOf(QButton("Edit", "u-edit", "c1"), QButton("Remove", "u-remove", "c1"))),
            QRow("c3", "Unsent", "second", listOf(QButton("Edit", "u-edit", "c3"), QButton("Remove", "u-remove", "c3"))),
        ), two.rows("h", "t1"))
        assertEquals(emptyList<QRow>(), two.rows("other-hub", "t1"))
        assertTrue(Outbox.isOwn("u-edit") && Outbox.isOwn("u-remove") && !Outbox.isOwn("o-edit") && !Outbox.isOwn("q-edit"))
    }

    @Test fun nextIsTheOldestForThisHubAndSentMovesOn() {
        val o = two.push("g", "x", "elsewhere", "c0").let { Outbox(listOf(it.items.last()) + it.items.dropLast(1)) }
        assertEquals("c0", o.next("g")?.clientId)
        var h = o; val order = mutableListOf<String>()
        while (true) { val n = h.next("h") ?: break; order += n.clientId; h = h.sent(n.clientId) }
        assertEquals(listOf("c1", "c2", "c3"), order)
        assertEquals(listOf("c0"), h.items.map { it.clientId })
        assertNull(Outbox().next("h"))
    }

    @Test fun persistenceRoundTrips() {
        assertEquals(two, Outbox.decode(two.encode()))
        assertEquals("first\nmore", Outbox.decode(two.encode()).item("c1")?.text)
        assertEquals(Outbox(), Outbox.decode(null)); assertEquals(Outbox(), Outbox.decode("not json")); assertEquals(Outbox(), Outbox.decode(Outbox().encode()))
    }
}
