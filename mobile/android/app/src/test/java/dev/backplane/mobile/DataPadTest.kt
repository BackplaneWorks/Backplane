package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

// The desktop pad's tests (native/gpui/src/screens/pad.rs), on the phone's model.
class DataPadTest {
    private val r = PadItem.Box(100f, 100f, 200f, 100f, 0)
    private fun drag(p: DataPad, a: Pair<Float, Float>, b: Pair<Float, Float>): Boolean {
        p.down(a.first, a.second)
        for (i in 1..6) { val t = i / 6f; p.moved(a.first + (b.first - a.first) * t, a.second + (b.second - a.second) * t) }
        return p.up()
    }
    private fun pad(t: PadTool) = DataPad().also { it.setTool(t) }
    private fun box(p: DataPad, i: Int) = p.items[i] as PadItem.Box

    @Test fun aPointOnEachSideIsThatSide() {
        assertEquals(listOf(PadPart.L, PadPart.R, PadPart.T, PadPart.B), listOf(100f to 150f, 300f to 150f, 200f to 100f, 200f to 200f).map { PadMath.hit(r, it.first, it.second, 7f) })
        assertEquals(PadPart.In, PadMath.hit(r, 200f, 150f, 7f)); assertNull(PadMath.hit(r, 50f, 50f, 7f))
    }
    @Test fun theTopmostItemIsPicked() { assertEquals(1, PadMath.pick(listOf(r, r.copy(x = 150f)), 200f, 150f, 7f)!!.first) }
    @Test fun lengthsParse() {
        assertEquals(PadLength(40f, "mm"), PadMath.parseLength("40 mm"))
        assertEquals(12.5f, PadMath.parseLength("12.5cm")!!.n); assertEquals("", PadMath.parseLength(" 7 ")!!.unit)
        for (bad in listOf("x", "0", "", "1.", ".5", "40 metres", "4 0")) assertNull(bad, PadMath.parseLength(bad))
    }
    @Test fun aTypedLengthKeepsTheOppositeSide() {
        val l = PadLength(30f, "mm")
        val right = PadMath.withLength(r, PadPart.R, l); assertEquals(Triple(100f, 120f, "30 mm"), Triple(right.x, right.w, right.dimW))
        val left = PadMath.withLength(r, PadPart.L, PadLength(30f, "")); assertEquals(Triple(300f, 120f, "30"), Triple(left.x + left.w, left.w, left.dimW))
        val top = PadMath.withLength(r, PadPart.T, l); assertEquals(Triple(200f, 120f, "30 mm"), Triple(top.y + top.h, top.h, top.dimH))
    }
    @Test fun aSideDraggedPastTheOtherNeverFlipsAndDropsTheTypedWidth() {
        val s = PadMath.resize(PadMath.withLength(r, PadPart.R, PadLength(30f, "mm")), PadPart.R, 50f, 0f)
        assertTrue(s.w >= 6f && s.x == 100f && s.dimW.isEmpty())
    }
    @Test fun eachNewBoxTakesTheNextColourUnlessOneWasChosen() {
        val p = pad(PadTool.Boxes)
        drag(p, 100f to 100f, 300f to 250f); drag(p, 400f to 100f, 600f to 300f)
        assertEquals(listOf(PAD_COLORS[0], PAD_COLORS[1]), p.items.map { (it as PadItem.Box).color })
        p.setColor(3); drag(p, 700f to 100f, 800f to 300f); drag(p, 700f to 350f, 800f to 500f)
        assertEquals(PAD_COLORS[3], box(p, 2).color); assertEquals(PAD_COLORS[3], box(p, 3).color)
    }
    @Test fun aSmallDragIsATapAndMakesNoBox() { val p = pad(PadTool.Boxes); drag(p, 100f to 100f, 102f to 101f); assertTrue(p.isEmpty); assertEquals(0, p.next) }
    @Test fun aBoxsMiddleMovesItASideResizesItAndATapOnASideAsksItsLength() {
        val p = pad(PadTool.Boxes); drag(p, 100f to 100f, 300f to 250f)
        assertFalse(drag(p, 200f to 175f, 250f to 215f)); assertEquals(150f to 140f, box(p, 0).x to box(p, 0).y)
        drag(p, 250f to 290f, 250f to 340f); assertEquals(340f, box(p, 0).y + box(p, 0).h)
        p.down(350f, 200f); assertTrue(p.up()); assertEquals(0 to PadPart.R, p.editing); assertEquals("50", p.lengthText())
        p.commitLength("30 mm"); assertEquals(120f, box(p, 0).w); assertEquals("30 mm", box(p, 0).dimW); assertNull(p.editing)
    }
    @Test fun theLengthBoxStartsWithTheAxisItSets() {
        val p = pad(PadTool.Boxes); drag(p, 100f to 100f, 300f to 200f)
        p.down(200f, 200f); p.up(); assertEquals(0 to PadPart.B, p.editing); assertEquals("25", p.lengthText())
        p.commitLength("not a length"); assertEquals(100f, box(p, 0).h)
    }
    @Test fun aWiderEdgeCatchesASideAFingerMissed() {
        val p = pad(PadTool.Boxes); drag(p, 100f to 100f, 300f to 200f)
        p.down(312f, 150f); assertFalse("7 px off the side is not on it", p.up()); assertNull(p.editing)
        p.edge = 16f; p.down(312f, 150f); assertTrue(p.up()); assertEquals(0 to PadPart.R, p.editing)
    }
    @Test fun undoTakesBackOneStrokeAtATime() {
        val p = DataPad()
        drag(p, 100f to 100f, 300f to 100f); drag(p, 100f to 200f, 300f to 200f); assertEquals(2, p.items.size)
        p.undo(); assertEquals(1, p.items.size); assertEquals(100f, (p.items[0] as PadItem.Stroke).pts[0].y)
        p.undo(); assertTrue(p.isEmpty); p.undo(); assertTrue(p.isEmpty)
    }
    @Test fun aTapWithThePenIsADot() { val p = DataPad(); p.down(50f, 50f); p.up(); assertEquals(1, (p.items[0] as PadItem.Stroke).pts.size) }
    @Test fun aStrokeLeavesNoGapWiderThanTheStepHoweverFastTheFinger() {
        val p = DataPad(); p.down(0f, 0f); p.moved(900f, 0f); p.up()
        assertTrue((p.items[0] as PadItem.Stroke).pts.zipWithNext().all { (a, b) -> abs(b.x - a.x) <= 3.01f })
    }
    @Test fun deleteRemovesTheSelectionAndWithNoneClearsThePaperAndUndoBringsEitherBack() {
        val p = pad(PadTool.Boxes); drag(p, 100f to 100f, 300f to 250f); drag(p, 400f to 100f, 600f to 300f)
        assertEquals("a new box is picked", 1, p.picked)
        p.delete(); assertEquals(1, p.items.size); assertNull(p.picked)
        p.undo(); assertEquals(2, p.items.size)
        p.delete(); assertTrue("nothing picked: the paper clears", p.isEmpty)
        p.undo(); assertEquals(2, p.items.size)
    }
    @Test fun aStrokeCanBePickedMovedAndDeleted() {
        val p = DataPad(); drag(p, 100f to 100f, 200f to 100f); p.setTool(PadTool.Boxes)
        drag(p, 150f to 100f, 150f to 150f); assertEquals(PadPt(100f, 150f), (p.items[0] as PadItem.Stroke).pts[0])
        assertEquals(0, p.picked); p.delete(); assertTrue(p.isEmpty)
    }
    @Test fun theEraserCutsAStrokeAndTakesABoxAndOneSweepIsOneUndo() {
        val p = DataPad(); drag(p, 100f to 100f, 500f to 100f)
        p.setTool(PadTool.Eraser); drag(p, 300f to 50f, 300f to 150f)
        assertEquals("cut in two", 2, p.items.size)
        val xs = p.items.flatMap { (it as PadItem.Stroke).pts.map { q -> q.x } }
        assertTrue(xs.none { abs(it - 300f) <= 12f } && xs.any { it < 290f } && xs.any { it > 310f })
        p.undo(); assertEquals(1, p.items.size)
        p.setTool(PadTool.Boxes); drag(p, 600f to 300f, 800f to 500f); p.setTool(PadTool.Eraser); drag(p, 700f to 400f, 710f to 400f)
        assertEquals("the box is gone", 1, p.items.size)
    }
    @Test fun anEraserSweepOverNothingLeavesNoUndo() {
        val p = DataPad(); drag(p, 100f to 100f, 200f to 100f); p.setTool(PadTool.Eraser); drag(p, 500f to 500f, 600f to 500f)
        p.undo(); assertTrue("the undo took the stroke, not an empty sweep", p.isEmpty)
    }
    @Test fun nothingHappensMidDrag() {
        val p = DataPad(); drag(p, 10f to 10f, 50f to 10f); p.down(100f, 100f); p.moved(200f, 100f)
        p.undo(); p.delete(); assertEquals(1, p.items.size); p.up(); assertEquals(2, p.items.size)
    }
    @Test fun aTouchTakenAwayDrawsNothing() {
        val p = DataPad(); p.down(100f, 100f); p.moved(200f, 100f); p.cancel(); assertFalse(p.up()); assertTrue(p.isEmpty); assertNull(p.ink)
    }
    @Test fun theWorkInProgressIsNotAnItem() {
        val p = pad(PadTool.Boxes); p.down(500f, 100f); p.moved(600f, 200f)
        assertTrue(p.isEmpty); assertEquals(100f, p.draft!!.w)
    }
}
