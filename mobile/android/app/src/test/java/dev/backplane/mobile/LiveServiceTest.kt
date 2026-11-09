package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

// What the ongoing notification says while threads work.
class LiveServiceTest {
    @Test fun oneThreadSaysItsTitleSeveralTheirCount() {
        assertEquals("Working" to "Lamp board", LiveService.text(listOf("Lamp board")))
        assertEquals("2 threads working" to "Lamp board · New thread", LiveService.text(listOf("Lamp board", "")))
    }
}
