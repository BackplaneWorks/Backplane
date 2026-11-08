package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

// The card a 0.14 import sends (present/onboard.rs import_card): the app takes its words and buttons as they are.
class DMigrateTest {
    private val running = JSONObject("""{"t":"migrating","state":"running","phase":"writing","pct":60,"card":{"state":"running","title":"Bringing your 0.14 work across…",
        "body":"Writing projects and threads. Backplane works meanwhile, and your 0.14 folder is only read.","pct":60,
        "lines":["3 of 12 projects · 40 of 300 threads · 4,000 of 9,000 events","Now: Nordic · Pick a regulator","About 2 min left in this step."],"buttons":[]}}""")

    @Test fun a_running_card_has_its_bar_two_lines_on_the_banner_and_no_way_out() {
        val c = DMigrateCard.of(running)!!
        assertTrue(c.running); assertEquals(60, c.pct); assertEquals(emptyList<DMigrateButton>(), c.buttons)
        assertEquals(listOf("3 of 12 projects · 40 of 300 threads · 4,000 of 9,000 events", "Now: Nordic · Pick a regulator"), c.head)
        assertEquals(3, c.lines.size)
    }
    @Test fun the_done_and_failed_cards_carry_the_hubs_buttons() {
        val d = DMigrateCard.of(JSONObject("""{"state":"done","card":{"state":"done","title":"Your 0.14 work is here","body":"2 projects and 3 threads with 9 messages.","pct":100,"lines":["It took 12 s."],
            "buttons":[{"label":"Continue","action":"ob-import","value":"ack:77","on":true},{"label":"Import with keys","action":"ob-import","value":"run-secrets","on":false}]}}"""))!!
        assertFalse(d.running); assertEquals(DMigrateButton("Continue", "ob-import", "ack:77", true), d.buttons[0]); assertEquals(2, d.buttons.size)
        val f = DMigrateCard.of(JSONObject("""{"state":"failed","card":{"state":"failed","title":"The import stopped","body":"x","pct":null,"lines":[],
            "buttons":[{"label":"Retry","action":"ob-import","value":"retry","on":true},{"label":"Not now","action":"ob-import","value":"later","on":false}]}}"""))!!
        assertNull("a failure has no bar", f.pct); assertEquals(listOf("retry", "later"), f.buttons.map { it.value })
    }
    @Test fun nothing_to_show_is_no_card() {
        assertNull(DMigrateCard.of(null))
        assertNull("hello migrating null", JSONObject("""{"migrating":null}""").optJSONObject("migrating").let(DMigrateCard::of))
        assertNull("a frame whose card is null (seen)", DMigrateCard.of(JSONObject("""{"t":"migrating","state":"done","card":null}""")))
    }
}
