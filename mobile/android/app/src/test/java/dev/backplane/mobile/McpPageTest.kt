package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

// Settings > MCP servers on the phone (native/host/src/mcpx/page.rs draws it): which rows it draws by kind, the buttons a card keeps,
// the page state it keeps from the hub, and what is typed (sent, held back, emptied).
class McpPageTest {
    private fun o(s: String) = JSONObject(s)
    private fun btn(label: String, action: String, value: String = "", on: Boolean = false) = JSONObject().put("label", label).put("action", action).put("value", value).put("on", on)

    @Test fun kindsItDoesNotKnowArePlainRows() {
        assertEquals("card", McpPage.kind(o("""{"kind":"card","label":"fetch"}""")))
        assertEquals("", McpPage.kind(o("""{"label":"Agents","note":"Which providers get it."}""")))
        assertEquals("a kind from a newer hub is drawn as a plain row", "", McpPage.kind(o("""{"kind":"hologram","label":"x"}""")))
        for (k in listOf("head", "group", "card", "tool", "notice", "code", "empty", "panel", "paste", "actions")) assertEquals(k, McpPage.kind(JSONObject().put("kind", k)))
    }

    @Test fun anOpenedCardHoldsTheRowsInsideIt() {
        val rows = listOf(
            o("""{"kind":"head","label":"MCP servers"}"""), o("""{"kind":"group","label":"Servers"}"""),
            o("""{"kind":"card","label":"a","open":true}"""), o("""{"kind":"notice","label":"Answered","in":true}"""), o("""{"label":"Agents","in":true}"""), o("""{"kind":"actions","label":"","in":true}"""),
            o("""{"kind":"card","label":"b"}"""), o("""{"kind":"tool","label":"stray","in":true}"""), o("""{"kind":"group","label":"Built in"}"""), o("""{"kind":"notice","label":"alone","in":true}"""))
        val b = McpPage.blocks(rows)
        assertEquals(listOf("MCP servers", "Servers", "a", "b", "Built in", "alone"), b.map { it.head.optString("label") })
        assertEquals(listOf("Answered", "Agents", ""), b[2].inner.map { it.optString("label") })
        assertEquals("an inside row goes with the card above it", listOf("stray"), b[3].inner.map { it.optString("label") })
        assertTrue("one after a group stands alone", b[5].inner.isEmpty())
    }

    @Test fun aCardDrawsOnlyTheButtonsItsSwitchAndTapAreNot() {
        val card = JSONObject().put("kind", "card")
            .put("toggle", o("""{"on":true,"action":"mcp-toggle","value":"fetch"}""")).put("press", o("""{"action":"mcp-view","value":"open:fetch"}"""))
            .put("buttons", JSONArray().put(btn("on", "mcp-toggle", "fetch", true)).put(btn("Open", "mcp-view", "open:fetch")).put(btn("Docs", "open-url", "https://x")).put(btn("", "")))
        assertEquals(listOf("Docs"), McpPage.cardButtons(card).map { it.optString("label") })
        // a gallery card has no switch: its Add and Docs are drawn
        val g = JSONObject().put("kind", "card").put("buttons", JSONArray().put(btn("Add", "mcp-gallery", "github", true)).put(btn("Docs", "open-url", "https://x")))
        assertEquals(listOf("Add", "Docs"), McpPage.cardButtons(g).map { it.optString("label") })
        assertEquals(0, McpPage.primary(o("""{"kind":"head","primary":0}"""))); assertEquals(-1, McpPage.primary(o("""{"kind":"head"}""")))
    }

    @Test fun thePageStateIsTheOneTheHubGaveBack() {
        assertEquals(mapOf("view" to "open:fetch", "rm" to ""), McpPage.merge(o("""{"view":"open:fetch","rm":"","skip":null}""")))
        assertEquals(emptyMap<String, String>(), McpPage.merge(null))
        assertEquals(mapOf("n" to "3"), McpPage.merge(o("""{"n":3}""")))
    }

    @Test fun whatAPressEmpties() {
        for (a in listOf("mcp-add", "mcp-import", "mcp-gallery", "mcp-save", "mcp-remove", "mcp-dup")) assertNull(a, McpPage.clears(a, "x"))
        // the view moving to the list, Add or an Edit empties the form (as the hub's step does), not the paste
        assertEquals(McpPage.FORM - "mpaste", McpPage.clears("mcp-view", "edit:fetch"))
        assertEquals(McpPage.FORM - "mpaste", McpPage.clears("mcp-view", ""))
        assertEquals(emptySet<String>(), McpPage.clears("mcp-view", "open:fetch"))
        assertEquals(emptySet<String>(), McpPage.clears("mcp-toggle", "fetch"))
    }

    @Test fun typedFieldsAreSentButAVaultFieldNever() {
        val f = McpForm()
        assertEquals("the hub's value until it is typed over", "uvx x", f.shown("mcmd", "uvx x"))
        f.type("mpaste", "{\"mcpServers\":{}}"); f.typeVault("GitHub\u001fmsec", "ghp_secret_value")
        assertEquals("{\"mcpServers\":{}}", f.shown("mpaste", ""))
        assertEquals(setOf("mpaste"), f.sent().keySet())
        assertFalse(f.sent().toString().contains("ghp_secret_value"))
        assertEquals("ghp_secret_value", f.vault("GitHub\u001fmsec")); assertEquals("", f.vault("Other\u001fmsec"))
        // a view change empties the form fields typed, not the paste; an Add empties everything, the vault's text too
        f.type("mname", "docs"); assertTrue(f.after("mcp-view", "add")); assertFalse(f.has("mname")); assertTrue(f.has("mpaste"))
        assertFalse(f.after("mcp-view", "open:x"))
        assertTrue(f.after("mcp-add", "")); assertFalse(f.has("mpaste")); assertEquals("", f.vault("GitHub\u001fmsec"))
        assertFalse("nothing left to empty", f.after("mcp-add", ""))
    }

    @Test fun onlyTheFormsFieldsParseAsTheyAreTyped() {
        for (n in listOf("mpaste", "mcmd", "menv", "mcwd", "murl", "mhdr", "mname")) assertTrue(n, McpPage.live(n))
        assertFalse(McpPage.live("msec")); assertFalse(McpPage.live("mgarg"))
        assertTrue(McpPage.LIVE_MS in 150L..400L); assertTrue(McpPage.REFRESH_MS in 3000L..6000L)
    }

    @Test fun vaultSaveTilesAndColours() {
        assertEquals("mcp-github-authorization" to "MCP server github: Authorization", McpPage.unpack("mcp-github-authorization\u001fMCP server github: Authorization\u001f"))
        assertEquals("" to "", McpPage.unpack(""))
        assertEquals("GitHub\u001fmsec", McpPage.vaultKey(o("""{"label":"GitHub"}"""), "msec"))
        assertTrue(McpPage.isVault(o("""{"name":"msec","vault":true}"""))); assertFalse(McpPage.isVault(o("""{"name":"mname"}""")))
        assertEquals(0xFF24292FL, McpPage.hex("#24292f")); assertNull(McpPage.hex("24292f")); assertNull(McpPage.hex("#zz0000")); assertNull(McpPage.hex(""))
        assertTrue(McpPage.tileScale("GH") < McpPage.tileScale("B"))
    }
}
