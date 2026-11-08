package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

// native/host/src/term/stream.rs `frames_merge_by_offset`, and the phone's use of it (TermScreen): nothing lost, nothing fed twice.
class TermStreamTest {
    @Test fun framesMergeByOffset() {
        assertEquals(TMerge.Take(0), TermStream.merge(10, 10, 5)); assertEquals(TMerge.Take(2), TermStream.merge(10, 8, 5)); assertEquals(TMerge.Skip, TermStream.merge(10, 3, 5))
        assertEquals(TMerge.Gap, TermStream.merge(10, 12, 5)); assertEquals(TMerge.Skip, TermStream.merge(10, 10, 0))
    }
    private fun f(g: Long?, o: Long?, s: String, reset: Pair<Int, Int>? = null, from: Long? = null, state: String? = null) =
        TFrame(g, o, s.toByteArray(), from, reset, state, null, null)
    private fun text(sc: TermScreen) = sc.vt.lines(0).joinToString("|") { l -> l.runs.joinToString("") { it.text } }

    @Test fun aSnapshotThenLiveFramesThatOverlapOrRepeat() {
        val sc = TermScreen()
        sc.read(f(7, 5, "hello", reset = 20 to 2))
        assertEquals(5L, sc.held); assertEquals("hello|", text(sc))
        assertEquals(TNext.Nothing, sc.push(f(7, 5, " world"))); assertEquals(11L, sc.held)
        assertEquals(TNext.Nothing, sc.push(f(7, 5, " world"))) // repeated: held already
        assertEquals(TNext.Nothing, sc.push(f(7, 9, "ld!"))) // overlaps: only the new byte
        assertEquals("hello world!|", text(sc)); assertEquals(12L, sc.held)
    }
    @Test fun aGapAsksForWhatCameAfterTheOffsetHeld() {
        val sc = TermScreen(); sc.read(f(7, 3, "abc", reset = 20 to 2))
        assertEquals(TNext.Resume(7, 3), sc.push(f(7, 10, "xyz")))
        assertEquals("abc|", text(sc))
        sc.read(f(7, 13, "defghijxyz", from = 3)) // the read after `from`
        assertEquals("abcdefghijxyz|", text(sc)); assertEquals(13L, sc.held)
    }
    @Test fun anotherRunOrNoRunMeansReadAgain() {
        val sc = TermScreen(); sc.read(f(7, 3, "abc", reset = 20 to 2))
        assertEquals(TNext.Reread, sc.push(f(8, 3, "x"))); assertEquals(TNext.Reread, sc.push(f(null, null, "x")))
        assertEquals("abc|", text(sc))
        sc.read(f(8, 1, "z", reset = 10 to 3)); assertEquals("z||", text(sc)); assertEquals(8L, sc.g)
    }
    @Test fun aSnapshotPushReplacesTheScreenAtItsOffset() {
        val sc = TermScreen(); sc.read(f(7, 3, "abc", reset = 20 to 2))
        assertEquals(TNext.Nothing, sc.push(f(7, 40, "new", reset = 30 to 5))); assertEquals("new||||", text(sc)); assertEquals(40L, sc.held)
        assertEquals(30, sc.vt.cols()); assertEquals(5, sc.vt.rows())
    }
    @Test fun stateFramesCarryNoBytes() {
        val sc = TermScreen(); sc.read(f(7, 3, "abc", reset = 20 to 2, state = "running"))
        assertEquals(true, sc.running)
        sc.push(f(null, null, "", state = "done 0")); assertEquals("done 0", sc.state); assertEquals(true, sc.running)
        sc.push(f(null, null, "", state = "exited 0")); assertEquals(false, sc.running); assertEquals("abc|", text(sc))
        sc.push(f(null, null, "", state = "stopped")); assertEquals("stopped", sc.stateLine)
    }
    @Test fun aReconnectResumesFromTheHeldOffsetWithNothingLost() {
        val sc = TermScreen(); sc.read(f(7, 3, "abc", reset = 20 to 2))
        sc.push(f(7, 3, "def")) // held 6
        // away: the hub printed more; term.watch with g=7, from=6 answers only what came after
        sc.read(f(7, 12, "ghijkl", from = 6))
        assertEquals("abcdefghijkl|", text(sc)); assertEquals(12L, sc.held)
        // the same answer twice does not repeat it
        sc.read(f(7, 12, "ghijkl", from = 6)); assertEquals("abcdefghijkl|", text(sc))
    }
}
