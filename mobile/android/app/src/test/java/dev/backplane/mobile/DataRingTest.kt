package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DataRingTest {
    @Test
    fun showAllClearsSearchField() {
        assertEquals("ringq", RingLogic.clearedField("ring-search", "clear"))
        assertNull(RingLogic.clearedField("ring-search", ""))
        assertNull(RingLogic.clearedField("ring-search", "keep"))
        assertNull(RingLogic.clearedField("ring-op", "setup"))
    }

    @Test
    fun detectsPendingRingTranscriptionOrDelivery() {
        val idle = listOf(JSONObject().put("note", "Call Alex\nHeard by parakeet-v2, 91% sure · given to Pebble"))
        assertFalse(RingLogic.hasPendingWork(idle))
        val hearing = listOf(JSONObject().put("note", "Call Alex\nNot heard · being heard now"))
        assertTrue(RingLogic.hasPendingWork(hearing))
        val queued = listOf(JSONObject().put("note", "(no words)\nNot heard · waiting to be heard"))
        assertTrue(RingLogic.hasPendingWork(queued))
        val sending = listOf(JSONObject().put("note", "Call Alex\nHeard by parakeet-v2 · waiting to go to Pebble"))
        assertTrue(RingLogic.hasPendingWork(sending))
    }

    @Test
    fun localActionsStayOnPhone() {
        assertTrue(PageRow.local("copy"))
        assertTrue(PageRow.local("share-text"))
        assertTrue(PageRow.local("open-url"))
        assertFalse(PageRow.local("ring-op"))
        assertFalse(PageRow.local("link-open"))
    }
}
