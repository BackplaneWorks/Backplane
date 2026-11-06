package dev.backplane.mobile

import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File

class DataFoldTest {
    private fun tool(id: String, name: String = "Bash", text: String = "ls") = DEntry(id, "tool", text, 1, tool = DTool(name, text, "done"))

    @Test fun labelsAreTheHubs() {
        var f = File("").absoluteFile
        while (!File(f, "test/mobile/fold_labels.json").exists()) f = f.parentFile
        val cases = JSONArray(File(f, "test/mobile/fold_labels.json").readText())
        for (i in 0 until cases.length()) {
            val c = cases.getJSONObject(i); val rows = c.getJSONArray("rows")
            val run = (0 until rows.length()).map { n ->
                val r = rows.getJSONObject(n)
                if (r.optBoolean("step")) DEntry("a$n", "step", r.getString("text"), 1) else tool("a$n", r.getString("name"), r.getString("text"))
            }
            assertEquals(c.getString("label"), DataFold.label(run, c.getBoolean("live")))
        }
    }

    @Test fun aRunEndsAtAMessageAndOnlyTheLastOfARunningTurnIsLive() {
        val rows = DataFold.rows(listOf(tool("1"), tool("2"), DEntry("u", "user", "x", 1), tool("3")), true)
        assertEquals(listOf("fold:1", "u", "fold:3"), rows.map { it.id })
        assertEquals(false, (rows[0] as DRow.Fold).fold.live)
        assertEquals(true, (rows[2] as DRow.Fold).fold.live)
        assertEquals(false, (DataFold.rows(listOf(tool("1")), false)[0] as DRow.Fold).fold.live)
    }
}
