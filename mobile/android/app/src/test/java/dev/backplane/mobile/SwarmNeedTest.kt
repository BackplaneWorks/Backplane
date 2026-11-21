package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

// A swarm's row in Needs you (native/host/src/present/swarm_need.rs through present/sections.rs `swarms_of`): its `need` and what each button does.
class SwarmNeedTest {
    private val row = JSONObject("""{"id":"h|r1","kind":"swarm","title":"debounce","word":"needs","say":"Member gemini failed: Gemini sign-in expired","want":"sign-in","nav":"@room:h|r1",
      "need":{"line":"Member gemini failed: Gemini sign-in expired","why":"sign-in","actions":[{"label":"Sign in to Gemini","act":"signin","room":"h|r1","thread":"h|b"},{"label":"Move to Claude","act":"move","room":"h|r1","thread":"h|b"}]}}""")

    private fun map(o: JSONObject) = o.keys().asSequence().associateWith { o.get(it) }
    @Test fun aSwarmRowReadsItsNeed() {
        val r = DSecRow.of(row)
        val n = r.need!!
        assertEquals("Member gemini failed: Gemini sign-in expired", n.line); assertEquals("sign-in", n.why)
        assertEquals(listOf(DNeedAct("Sign in to Gemini", "signin", "h|r1", "h|b"), DNeedAct("Move to Claude", "move", "h|r1", "h|b")), n.actions)
        assertEquals("h|r1", r.swarmRoom)
        assertNull("a null need: nothing waits", DSecRow.of(JSONObject(row.toString()).put("need", JSONObject.NULL)).need)
        assertNull("an older hub sends none", DSecRow.of(JSONObject(row.toString()).apply { remove("need") }).need)
    }
    @Test fun aHubActionIsSwarmActWithTheThreadOnlyWhenThereIsOne() {
        val g = NeedUi.press(DNeedAct("Sign in to Gemini", "signin", "h|r1", "h|b")) as NeedGo.Call
        assertEquals("swarm.act", g.method); assertEquals(mapOf("room" to "h|r1", "action" to "signin", "thread" to "h|b"), map(g.params))
        for (a in listOf("resume", "raise", "retry", "move")) assertEquals(a, (NeedUi.press(DNeedAct(a, a, "r1", "")) as NeedGo.Call).params.getString("action"))
        assertEquals(mapOf("room" to "r1", "action" to "resume"), map((NeedUi.press(DNeedAct("Resume", "resume", "r1", "")) as NeedGo.Call).params))
    }
    @Test fun boardOpensTheSwarmAndOpenItsMember() {
        assertEquals(NeedGo.Board, NeedUi.press(DNeedAct("Read board", "board", "r1", "")))
        assertEquals(NeedGo.Thread("h|a"), NeedUi.press(DNeedAct("Answer", "open", "r1", "h|a")))
        assertNull(NeedUi.press(DNeedAct("Answer", "open", "r1", "")))
        assertNull("an act this app does not know draws a dead button", NeedUi.press(DNeedAct("?", "later", "r1", "")))
    }
    @Test fun theReplyOpensTheSignInItNames() {
        assertEquals(NeedGo.Account("p7"), NeedUi.after(JSONObject("""{"open":"account:p7"}""")))
        assertEquals(NeedGo.Thread("t9"), NeedUi.after(JSONObject("""{"open":"thread:t9"}""")))
        assertNull(NeedUi.after(JSONObject("""{"ok":true}"""))); assertNull(NeedUi.after(JSONObject("""{"open":"account:"}"""))); assertNull(NeedUi.after(null))
    }
}
