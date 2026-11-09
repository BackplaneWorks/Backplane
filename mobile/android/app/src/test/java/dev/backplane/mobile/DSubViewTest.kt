package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** A subagent's view (feature subview) as the hub sends it (present/panels.rs `sub_view`), and the rows that open it. */
class DSubViewTest {
    @Test fun aSubViewParsesItsHeadAndStepsWithTheirResults() {
        val v = parseSubView(JSONObject("""{"key":"c1","thread":"t1","title":"Survey","state":"working","live":true,"meta":"Claude Code · Explore","took":"<1m","where":"w","ask":"Find.","outcome":"","act":"task-stop","value":"a1","label":"Stop","more":2,
            "entries":[{"id":"c1/about","kind":"user","text":"Find.","at":1},{"id":"c1/s1","kind":"tool","text":"ls","at":2,"tool":{"name":"Bash","summary":"ls","state":"failed","result":"no such file"}}]}"""))
        assertEquals(listOf("Survey", "working", "Stop", "a1"), listOf(v.title, v.state, v.label, v.value)); assertTrue(v.live); assertEquals(2, v.more)
        assertEquals(listOf("user", "tool"), v.entries.map { it.kind }); assertEquals("no such file", v.entries[1].tool?.result); assertEquals("failed", v.entries[1].tool?.state)
    }
    @Test fun anAgentCallAndASubagentRowNameTheirSubagent() {
        assertEquals("c9", parseEntry(JSONObject("""{"id":"c9","kind":"tool","text":"Survey","at":1,"tool":{"name":"Agent","summary":"Survey","state":"done","sub":"c9"}}""")).tool?.sub)
        val w = parseWork(JSONObject("""{"word":"1 at work","live":1,"rows":[{"title":"Survey","kind":"subagent","state":"working","doing":"","key":"c9","meta":"m","detail":{}},{"title":"sleep","kind":"shell","state":"working","doing":"","key":"x"}]}"""))
        assertEquals(listOf("c9", ""), w.rows.map { it.key }); assertEquals("m", w.rows[0].meta)
    }
    @Test fun aLongResultShowsThreeLinesUntilOpened() {
        assertEquals("a\nb" to false, subResultLines("a\nb", false)); assertEquals("a\nb\nc" to true, subResultLines("a\nb\nc\nd", false)); assertEquals("a\nb\nc\nd" to false, subResultLines("a\nb\nc\nd", true))
    }
}
