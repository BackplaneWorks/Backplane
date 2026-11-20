package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

// The hub's restart in the connection strip (feature "restarting", docs/PHONE-DATA.md, Restarts).
class RestartStripTest {
    private val going = RestartNote("Restarting Backplane… (new build 2c25023cc9)", 1_000)
    private val back = RestartNote("Backplane is back after it stopped unexpectedly", 1_000)

    @Test fun the_restart_shows_in_place_of_the_reconnecting_look_while_the_hub_is_away() {
        assertEquals(going.text, RestartStrip.text(going, null, online = false, now = 1_000))
        assertEquals(going.text, RestartStrip.text(going, null, online = false, now = 1_000 + RestartStrip.AWAY_MS - 1))
    }
    @Test fun after_five_minutes_without_a_hub_the_usual_look_comes_back() {
        assertNull(RestartStrip.text(going, null, online = false, now = 1_000 + RestartStrip.AWAY_MS))
        assertNull(RestartStrip.next(going, null, online = false, now = 1_000 + RestartStrip.AWAY_MS))
        assertEquals(1_000 + RestartStrip.AWAY_MS, RestartStrip.next(going, null, online = false, now = 2_000))
    }
    @Test fun a_drop_with_no_frame_keeps_the_usual_look() {
        assertNull(RestartStrip.text(null, null, online = false, now = 5_000))
        assertNull(RestartStrip.next(null, null, online = false, now = 5_000))
    }
    @Test fun connected_again_the_restart_is_not_shown() {
        assertNull(RestartStrip.text(going, null, online = true, now = 2_000))
        assertNull(RestartStrip.next(going, null, online = true, now = 2_000))
    }
    @Test fun the_back_note_shows_for_six_seconds_as_the_app_joins() {
        assertEquals(back.text, RestartStrip.text(null, back, online = true, now = 1_000))
        assertEquals(back.text, RestartStrip.text(null, back, online = true, now = 1_000 + RestartStrip.BACK_MS - 1))
        assertEquals(1_000 + RestartStrip.BACK_MS, RestartStrip.next(null, back, online = true, now = 3_000))
        assertNull(RestartStrip.text(null, back, online = true, now = 1_000 + RestartStrip.BACK_MS))
        assertNull(RestartStrip.next(null, back, online = true, now = 1_000 + RestartStrip.BACK_MS))
        assertNull("offline is the usual look", RestartStrip.text(null, back, online = false, now = 1_000))
    }
    @Test fun the_hubs_words_go_through_the_lookup() {
        try {
            Tr.fixed = "ja"
            assertEquals(Msg.hub_restarting_build("2c25023cc9"), Tr.t("Restarting Backplane… (new build 2c25023cc9)"))
            assertEquals(Msg.hub_restarting, Tr.t("Restarting Backplane…")); assertEquals(Msg.hub_stopped, Tr.t("Backplane stopped"))
            assertEquals(Msg.hub_back_crash, Tr.t("Backplane is back after it stopped unexpectedly"))
            assertNotEquals("Backplane stopped", Msg.hub_stopped)
        } finally { Tr.fixed = null }
    }
    @Test fun hub_back_is_empty_or_a_json_string() {
        assertEquals(5L to "Backplane is back after it stopped unexpectedly", RestartStrip.back("""{"reason":"crash","text":"Backplane is back after it stopped unexpectedly","at":5}"""))
        for (n in listOf(null, "", " ", "{", """{"reason":"crash","text":"","at":5}""")) assertNull(n, RestartStrip.back(n))
    }
}
