package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test

/** The marks, switcher chips and "new on" choices of docs/PHONE-DATA.md (Your machines), as the hub sends them. */
class DMachinesTest {
    private val mark = """{"name":"desktop.tail.ts.net","short":"desktop","away":true,"label":"on desktop","runs":"runs on desktop"}"""
    @Test fun a_thread_of_another_machine_carries_its_mark() {
        val t = parseThread(JSONObject("""{"id":"L~t1","project":"p1","title":"x","state":"idle","updatedAt":5,"mark":$mark}"""))
        assertEquals("desktop", t.mark?.short); assertTrue(t.mark!!.away); assertEquals("runs on desktop", t.mark!!.runs); assertEquals("desktop.tail.ts.net", t.mark!!.name)
        assertNull(parseThread(JSONObject("""{"id":"t1","project":"p1","title":"x"}""")).mark)
        assertNull(parseThread(JSONObject("""{"id":"t1","project":"p1","title":"x","mark":null}""")).mark)
    }
    @Test fun a_project_says_where_it_is_and_where_a_new_thread_can_start() {
        val p = parseProject(JSONObject("""{"id":"p1","title":"R","also":"also on desktop","newOn":[{"value":"p1","label":"New thread on laptop","away":false},{"value":"L~p1","label":"New thread on desktop (away)","away":true}]}"""))
        assertEquals("also on desktop", p.also); assertEquals(listOf("p1", "L~p1"), p.newOn.map { it.value }); assertTrue(p.newOn[1].away); assertNull(p.mark)
        assertEquals("on desktop", parseProject(JSONObject("""{"id":"L~p2","title":"S","mark":$mark}""")).mark?.label)
    }
    @Test fun the_switcher_chips_are_the_hubs() {
        val c = parseChips(org.json.JSONArray("""[{"value":"","label":"All","on":true,"away":false,"hint":"Every machine"},{"value":"desk","label":"desk","on":false,"away":true,"hint":"desk · away"}]"""))
        assertEquals(listOf("", "desk"), c.map { it.value }); assertTrue(c[0].on); assertTrue(c[1].away); assertEquals("desk · away", c[1].hint)
        assertTrue(parseChips(null).isEmpty())
    }
}
