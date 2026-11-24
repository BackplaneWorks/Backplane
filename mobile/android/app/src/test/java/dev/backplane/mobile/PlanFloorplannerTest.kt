package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PlanFloorplannerTest {
    @Test
    fun parseThreadPlanModeAndCycling() {
        val tObj = JSONObject(
            """{"id":"t1","project":"p","title":"T","state":"idle","updatedAt":1,"provider":"claude","model":"opus","planMode":{"phase":"armed","interaction":"default","once":true,"scope":"once","label":"Plan (1 turn)","pending":false,"executing":false}}""",
        )
        val t = parseThread(tObj)
        assertNotNull(t.planMode)
        assertEquals("armed", t.planMode?.phase)
        assertEquals("once", t.planMode?.scope)
        assertEquals(PlanModeTarget.ALWAYS, nextPlanModeCycle(t.planMode!!))
        assertEquals(PlanModeTarget.ONCE, nextPlanModeCycle(DPlanMode()))
        assertEquals(PlanModeTarget.OFF, nextPlanModeCycle(DPlanMode(phase = "pending")))

        val omitted = parseThread(JSONObject("""{"id":"t2","project":"p","title":"T","state":"idle","updatedAt":1,"provider":"claude","model":"opus"}"""))
        assertNull(omitted.planMode)
        assertEquals("off", effectivePlanMode(omitted)?.phase)

        val ask = parseAsk(JSONObject("""{"id":"a1","state":"open","kind":"plan","head":"Proposed plan","question":"Body","options":[],"revise":true}"""))
        assertEquals("Proposed plan", ask.head)
        assertTrue(ask.revise)
    }

    @Test
    fun unwrapLayersTitlesAndDefaultActiveLayer() {
        val wrapped = JSONObject("""{"result":{"panel":{"revision":7,"board_trimmed":true,"board":{"bbox":[0,0,80,50],"outline":[],"parts":[]}}}}""")
        val panel = PlanFloorplannerModel.unwrapPanel(wrapped)
        assertNotNull(panel)
        assertEquals(7, panel?.revision)
        assertTrue(panel?.boardTrimmed == true)

        val planned = listOf(PlanLayer(layer = "In5.Cu", role = "signal", items = listOf(PlanItem(id = "c1", kind = "corridor"))))
        val copper = listOf(
            PlanCopperLayer("F.Cu", 1, "signal", "L1 · TOP"),
            PlanCopperLayer("In1.Cu", 2, "power", "L2 · GND1"),
            PlanCopperLayer("In5.Cu", 6, "signal", "L6 · SIG1"),
        )
        val all = PlanFloorplannerModel.allCanvasLayers(planned, copper)
        assertEquals(3, all.size)
        assertEquals("In5.Cu", PlanFloorplannerModel.defaultActiveLayer(all))
        assertEquals("L6 · SIG1", PlanFloorplannerModel.layerDisplayTitle("In5.Cu", copper, all))
        assertEquals("In9.Cu", PlanFloorplannerModel.layerDisplayTitle("In9.Cu", copper, all))
    }

    @Test
    fun hitTestVertexMidpointBandRegionAndCollisionFreePills() {
        val cam = PlanCamera.fit(listOf(0.0, 0.0, 100.0, 100.0), 200.0, 200.0, 0.0)
        val bands = listOf(PlanBand(id = "c1", layer = "In5.Cu", name = "SPI", width = 4.0, nets = 4, path = listOf(10.0 to 50.0, 90.0 to 50.0)))
        val regions = listOf(
            PlanRegion(
                id = "p1", layer = "In5.Cu", net = "+3V3",
                outline = listOf(5.0 to 5.0, 95.0 to 5.0, 95.0 to 95.0, 5.0 to 95.0),
                holes = listOf(listOf(30.0 to 70.0, 45.0 to 70.0, 45.0 to 85.0, 30.0 to 85.0)),
            ),
        )
        assertEquals(PlanHit.Vertex("c1", false, 0), PlanFloorplannerModel.hitTest(bands, regions, "In5.Cu", "c1", cam, 200.0, 200.0, 21.0, 101.0))
        assertEquals(PlanHit.Midpoint("c1", false, 0), PlanFloorplannerModel.hitTest(bands, regions, "In5.Cu", "c1", cam, 200.0, 200.0, 100.0, 100.0))
        assertEquals(PlanHit.Band("c1"), PlanFloorplannerModel.hitTest(bands, regions, "In5.Cu", "", cam, 200.0, 200.0, 120.0, 100.0))
        assertEquals(PlanHit.Region("p1"), PlanFloorplannerModel.hitTest(bands, regions, "In5.Cu", "", cam, 200.0, 200.0, 30.0, 40.0))
        assertNull(PlanFloorplannerModel.hitTest(bands, regions, "In5.Cu", "", cam, 200.0, 200.0, 76.0, 156.0))

        val open = listOf(0.0 to 0.0, 10.0 to 0.0)
        assertNull(PlanFloorplannerModel.removeVertex(open, 0, false))
        val ins = PlanFloorplannerModel.insertVertex(open, 0, 5.0 to 4.0, false)
        assertEquals(3, ins.size)
        assertEquals(open, PlanFloorplannerModel.removeVertex(ins, 1, false))

        val cam400 = PlanCamera.fit(listOf(0.0, 0.0, 100.0, 100.0), 400.0, 400.0, 0.0)
        val bA = PlanBand(id = "b1", layer = "F.Cu", name = "SPI", nets = 4, path = listOf(10.0 to 50.0, 90.0 to 50.0))
        val bB = PlanBand(id = "b2", layer = "F.Cu", name = "I2C", nets = 2, path = listOf(10.0 to 50.0, 90.0 to 50.0))
        val pills = PlanFloorplannerModel.placeCorridorPills(listOf(bA, bB), cam400, 400.0, 400.0, "b1")
        assertEquals(2, pills.size)
        assertEquals("SPI · 4", pills[0].text)
        assertFalse(pills[0].rect.overlaps(pills[1].rect))

        val blocked = PlanFloorplannerModel.layoutLabels(
            listOf(PlanPartBox("U_BLK", "F", listOf(40.0, 40.0, 60.0, 60.0), false)),
            cam400, 400.0, 400.0, emptyList(), listOf(pills[0].rect),
        )
        assertTrue(blocked.isEmpty())

        val pourPt = PlanFloorplannerModel.pourLabelPoint(
            listOf(0.0 to 0.0, 100.0 to 0.0, 100.0 to 100.0, 0.0 to 100.0),
            listOf(listOf(35.0 to 35.0, 65.0 to 35.0, 65.0 to 65.0, 35.0 to 65.0)),
        )
        assertNotNull(pourPt)
        assertFalse(pourPt!!.first in 35.0..65.0 && pourPt.second in 35.0..65.0)
        assertNull(
            PlanFloorplannerModel.pourLabelPoint(
                listOf(0.0 to 0.0, 100.0 to 0.0, 100.0 to 100.0, 0.0 to 100.0),
                listOf(listOf(2.0 to 2.0, 98.0 to 2.0, 98.0 to 98.0, 2.0 to 98.0)),
            ),
        )

        val crowded = (0 until 8).map { i ->
            PlanBand(id = "k$i", layer = "F.Cu", name = "BUS$i", nets = i + 1, path = listOf(50.0 to 50.0, 51.0 to 50.0))
        }
        val crowdedPills = PlanFloorplannerModel.placeCorridorPills(crowded, cam400, 400.0, 400.0, "")
        assertTrue(crowdedPills.size < 8)
        for (i in crowdedPills.indices) {
            for (j in i + 1 until crowdedPills.size) {
                assertFalse(crowdedPills[i].rect.overlaps(crowdedPills[j].rect))
            }
        }

        val projOnly = PlanFloorplannerModel.unwrapPanel(JSONObject("""{"project":"/p/boards/b/b.kicad_pro","revision":5,"layers":[]}"""))
        assertEquals("/p/boards/b/b.kicad_pro", projOnly?.pro)
        val outerPro = PlanFloorplannerModel.unwrapPanel(JSONObject("""{"pro":"/p/main.kicad_pro","panel":{"revision":6,"layers":[]}}"""))
        assertEquals("/p/main.kicad_pro", outerPro?.pro)

        val boardRefs = listOf(
            PlanBoardRef("boards/main/main.kicad_pcb", "", "", "/proj/boards/main/main.kicad_pro"),
            PlanBoardRef("boards/sub/sub.kicad_pcb", "/proj/boards/sub/sub.kicad_pcb", "/proj/boards/sub/sub.kicad_sch", "/proj/boards/sub/sub.kicad_pro"),
        )
        assertEquals(
            "/proj/boards/sub/sub.kicad_pro",
            PlanFloorplannerModel.resolveMgmtProject("/proj/boards/sub/sub.kicad_pcb", boardRefs[0], boardRefs, "/proj"),
        )
        assertEquals(
            "/proj/boards/sub/sub.kicad_pro",
            PlanFloorplannerModel.resolveMgmtProject("boards/sub/power.kicad_sch", boardRefs[0], boardRefs, "/proj"),
        )
        assertEquals(
            "/proj/extra/test_jig.kicad_pcb",
            PlanFloorplannerModel.resolveMgmtProject("extra/test_jig.kicad_pcb", boardRefs[0], boardRefs, "/proj"),
        )
        assertEquals(
            "/proj/boards/main/main.kicad_pro",
            PlanFloorplannerModel.resolveMgmtProject(null, boardRefs[0], boardRefs, "/proj"),
        )

        val stack = listOf(
            PlanUndoEntry("/p/a", "c1", PlanOptimisticPatch(path = listOf(0.0 to 0.0, 10.0 to 0.0)), "path"),
            PlanUndoEntry("/p/a", "c2", PlanOptimisticPatch(width = 1.5)),
        )
        assertFalse(PlanFloorplannerModel.hasUndo(stack, "/p/a", "c3"))
        val (popped, rest) = PlanFloorplannerModel.popUndo(stack, "/p/a", "c1")
        assertEquals("c1", popped?.id)
        assertFalse(PlanFloorplannerModel.hasUndo(rest, "/p/a", "c1"))
        assertTrue(PlanFloorplannerModel.hasUndo(rest, "/p/a", "c2"))

        val folded = PlanFloorplannerModel.foldPatches(
            listOf("c1" to PlanOptimisticPatch(width = 2.0), "c2" to PlanOptimisticPatch(width = 3.0)),
            preview = "c1" to PlanOptimisticPatch(path = listOf(1.0 to 1.0, 2.0 to 2.0)),
        )
        assertEquals(2.0, folded["c1"]?.width)
        assertEquals(listOf(1.0 to 1.0, 2.0 to 2.0), folded["c1"]?.path)
        val noPreview = PlanFloorplannerModel.foldPatches(listOf("c1" to PlanOptimisticPatch(width = 2.0)))
        assertEquals(2.0, noPreview["c1"]?.width)
        assertNull(noPreview["c1"]?.path)
    }
}
