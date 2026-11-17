package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The speed dial (issue #1166): the hub's Speed section is a dial, its detents carry their cost, and the geometry is the desktop's. */
class SpeedDialTest {
    @Test fun theSpeedSectionIsADialWithItsCosts() {
        val p = parsePicker(JSONObject("""{"chip":"codex / gpt-6.1-sol / high / Fast","sections":[{"title":"Effort","action":"effort","choices":[{"label":"high","value":"high","on":true}]},
            {"title":"Speed","action":"speed","dial":true,"choices":[{"label":"Normal","value":"","on":false,"cost":""},{"label":"Fast","value":"fast","on":true,"cost":"×2.5 usage"},{"label":"UltraFast","value":"ultrafast","cost":"×8 usage"}]}]}"""))
        val s = p.sections.last(); assertTrue(s.dial); assertFalse(p.sections.first().dial)
        assertEquals(listOf("", "×2.5 usage", "×8 usage"), s.choices.map { it.cost }); assertEquals(1, s.choices.indexOfFirst { it.on })
    }
    @Test fun aNewThreadKeepsItsSpeedAndAnotherAgentStartsAtNormal() {
        val c = DNewThreadChoice(provider = "claude").set("speed", "fast"); assertEquals("fast", c.speed); assertEquals(null, c.runtime)
        assertEquals("fast", c.into(JSONObject()).getString("speed")); assertEquals("", c.set("provider", "codex").speed)
    }
    @Test fun detentsSitOnTheDesktopsArcAndADragSnaps() {
        assertEquals(listOf(-45.0, 45.0), DialGeometry.angles(2)); assertEquals(listOf(-90.0, 0.0, 90.0), DialGeometry.angles(3))
        assertEquals(0, DialGeometry.nearest(3, -70.0)); assertEquals(2, DialGeometry.nearest(3, 170.0))
        assertEquals(90.0, DialGeometry.angle(10f, 10f, 20f, 10f), 0.01); assertEquals(-90.0, DialGeometry.angle(10f, 10f, 0f, 10f), 0.01)
    }
}
