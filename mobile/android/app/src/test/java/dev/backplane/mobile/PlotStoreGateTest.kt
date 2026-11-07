package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

// Switching viewers: what the last viewer showed never reaches the next one (the person saw the previous viewer's files after a switch).
class PlotStoreGateTest {
    private val a = "/p/GNSS"; private val b = "/p/ecc83"

    @Test fun twoBoardsSwitchedQuicklyShowOnlyTheSecond() {
        val g = ViewGate<String>()
        g.begin()                                              // board A opens
        assertNull(g.wanted("0|$a|"))                          // its plot waits for the answer
        g.park("0|$a|", "A board")
        assertEquals(listOf("A board"), g.shown(PlotStore.keys("board", a, "")))
        g.begin()                                              // board B, at once
        g.park("0|$a|", "A board, late")                       // A's reply comes after the switch
        g.park("0|$b|", "B board")
        assertEquals("only B's", listOf("B board"), g.shown(PlotStore.keys("board", b, "")))
        assertEquals("A's late plot is dropped", false, g.wanted("0|$a|"))
    }

    @Test fun viewerKindsSwitchedQuicklyShowOnlyTheLast() {
        val g = ViewGate<String> { x, y -> x[0] == y[0] }
        repeat(4) { g.begin() }  // tabs tapped fast: only the last watch is answered
        g.park("0|$a|", "board"); g.park("2|$a|", "model"); g.park("1|$a|", "sheet")
        assertEquals(listOf("sheet"), g.shown(PlotStore.keys("schematic", a, "")))
        g.begin(); g.park("1|$a|", "sheet, late")
        assertEquals("the 3D view takes its model and the board under it", emptyList<String>(), g.shown(PlotStore.keys("3d", a, "")))
        assertEquals(true, g.wanted("2|$a|")); assertEquals(true, g.wanted("0|$a|")); assertEquals(false, g.wanted("1|$a|"))
    }

    @Test fun aNewerParkedPlotOfAKeyReplacesTheOlder() {
        val g = ViewGate<String> { _, _ -> true }
        g.begin(); g.park("0|$a|", "old"); g.park("0|$a|", "new")
        assertEquals(listOf("new"), g.shown(listOf("0|$a|")))
    }

    @Test fun aModelsPartsWaitBehindItAndAllReachTheView() {
        // the native board mesher's parts come after the model under its key; parked before the answer, none may replace the model
        val g = ViewGate<String> { x, y -> x == "model" && y == "model" }
        g.begin(); g.park("2|$a|", "model"); g.park("2|$a|", "parts"); g.park("2|$a|", "part 1"); g.park("2|$b|", "parts of B")
        assertEquals(listOf("model", "parts", "part 1"), g.shown(PlotStore.keys("3d", a, "")))
        g.begin(); g.park("2|$a|", "parts")   // a `parts` alone (the kept model was the board)
        assertEquals(listOf("parts"), g.shown(PlotStore.keys("3d", a, "")))
    }

    @Test fun aPartWithItsOwnMeshIsNotTheModel() {
        // the hub's frames as they are: a `part` carries a mesh too (it was taken for the board's model, which then went blank)
        val model = mapOf<String, Any?>("key" to "2|$a|", "n" to 12L, "mesh" to ByteArray(120))
        val part = mapOf<String, Any?>("key" to "2|$a|", "part" to "0123456789abcdef", "n" to 2L, "mesh" to ByteArray(20), "at" to ByteArray(48))
        val start = mapOf<String, Any?>("key" to "2|$a|", "parts" to mapOf("of" to 3L, "rev" to "r"))
        assertEquals(true, PlotStore.isModel(model)); assertEquals(false, PlotStore.isModel(part)); assertEquals(false, PlotStore.isModel(start))
        assertEquals(true, PlotStore.isPart(part)); assertEquals(true, PlotStore.isPart(start))
        val g = ViewGate<Map<String, Any?>> { x, y -> PlotStore.sameKind(x, y) }
        g.begin(); g.park("2|$a|", model); g.park("2|$a|", start); g.park("2|$a|", part)
        assertEquals("the part did not replace the parked model", listOf(model, start, part), g.shown(PlotStore.keys("3d", a, "")))
        val newer = model + ("n" to 13L)
        g.begin(); g.park("2|$a|", model); g.park("2|$a|", newer)
        assertEquals("a newer model still replaces the older", listOf(newer), g.shown(PlotStore.keys("3d", a, "")))
    }

    @Test fun theKeysAreTheHubs() {
        assertEquals(listOf("0|/r|"), PlotStore.keys("board", "/r", ""))
        assertEquals(listOf("3|/r|/r/mech/a.step", "0|/r|/r/mech/a.step"), PlotStore.keys("3d-bare", "/r", "mech/a.step"))
        assertEquals(listOf("1|/r|/r/s.kicad_sch", "0|/r|", "2|/r|"), PlotStore.keys("split", "/r", "/r/s.kicad_sch"))
    }
}
