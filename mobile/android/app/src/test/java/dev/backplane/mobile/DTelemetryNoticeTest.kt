package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

// The one-time telemetry notice (feature "telemetryNotice", docs/PHONE-DATA.md): the app shows the hub's words and buttons as they are.
class DTelemetryNoticeTest {
    @Test fun a_notice_carries_its_token_words_and_labels() {
        val n = DTelemetryNotice.of(JSONObject("""{"token":"t1","text":"Backplane sends anonymous usage counts.","off":"Turn off","settings":"Settings"}"""))!!
        assertEquals(DTelemetryNotice("t1", "Backplane sends anonymous usage counts.", "Turn off", "Settings"), n)
    }
    @Test fun null_or_empty_is_no_notice() {
        assertNull(DTelemetryNotice.of(null))
        assertNull("telemetry.notice's null", JSONObject("""{"notice":null}""").optJSONObject("notice").let(DTelemetryNotice::of))
        assertNull("a frame's null", JSONObject("""{"t":"telemetry.notice","notice":null}""").optJSONObject("notice").let(DTelemetryNotice::of))
        assertNull("no token", DTelemetryNotice.of(JSONObject("""{"token":"","text":"x","off":"Turn off","settings":"Settings"}""")))
        assertNull("no text", DTelemetryNotice.of(JSONObject("""{"token":"t","text":"","off":"Turn off","settings":"Settings"}""")))
    }
    @Test fun the_act_carries_token_and_action() {
        for (a in listOf("shown", "close", "off")) {
            val p = DTelemetryNotice.act("t1", a)
            assertEquals("t1", p.getString("token")); assertEquals(a, p.getString("action")); assertEquals(2, p.length())
        }
    }
}
