package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test

// Settings' section index (chips in page order, normal-case labels) and the Advanced fold's split
class SettingsNavTest {
    @Test fun chipsReadInNormalCase() {
        assertEquals("Voice", SettingsNav.chip("VOICE"))
        assertEquals("Notifications", SettingsNav.chip("NOTIFICATIONS"))
        assertEquals("Home machine", SettingsNav.chip("Home machine"))
        assertEquals("3D background", SettingsNav.chip("3D background"))
        assertEquals("音声", SettingsNav.chip("音声"))
    }

    @Test fun chipsFollowThePage() {
        val titles = linkedMapOf("voice" to "VOICE", "hubs" to "Hubs", "devices" to "DEVICES", "late" to "LATE")
        val ys = mapOf("voice" to 900, "hubs" to 10, "devices" to 2400)
        // a section not measured yet waits; the rest go top to bottom, whatever order they registered in
        assertEquals(listOf("hubs", "voice", "devices"), SettingsNav.order(titles, ys).map { it.first })
        assertEquals(emptyList<Pair<String, String>>(), SettingsNav.order(emptyMap(), emptyMap()))
    }

    private fun row(label: String, adv: Boolean = false) = JSONObject().put("label", label).apply { if (adv) put("adv", true) }

    @Test fun markedRowsFoldInPageOrder() {
        val rows = listOf(row("Speech to text"), row("How eager", true), row("Microphone"), row("Voice commands", true))
        val (plain, adv) = SettingsNav.split(rows)
        assertEquals(listOf("Speech to text", "Microphone"), plain.map { it.optString("label") })
        assertEquals(listOf("How eager", "Voice commands"), adv.map { it.optString("label") })
        // no mark: every row is plain, as before
        assertEquals(0, SettingsNav.split(listOf(row("A"), row("B"))).second.size)
        // the debug override marks by label
        assertEquals(listOf("Microphone"), SettingsNav.split(listOf(row("A"), row("Microphone")), setOf("Microphone")).second.map { it.optString("label") })
    }
}
