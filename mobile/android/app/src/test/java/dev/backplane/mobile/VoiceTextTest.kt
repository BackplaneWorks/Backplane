package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File

// the same cases as the desktop's screens/voice.rs and the hub's place::apply: positions are characters, not UTF-16 units
class VoiceTextTest {
    @Test fun editReplacesAfterTheMarkAndMovesIt() {
        assertEquals(VoiceText.Applied("hello brave world", 12, 12), VoiceText.edit("hello world", 6, 0, "brave ", 6, 6))
        assertEquals(VoiceText.Applied("hi there", 8, 8), VoiceText.edit("hi thx", 3, 3, "there", 5, 5))
    }
    @Test fun aMarkPastTheEndIsTheEnd() {
        assertEquals(VoiceText.Applied("abcdef", 6, 6), VoiceText.edit("abc", 99, 0, "def", 3, 3))
    }
    @Test fun charactersAreCodePoints() {
        val t = "a😀b"
        assertEquals(3, VoiceText.length(t))
        assertEquals(VoiceText.Applied("a😀Xb", 3, 3), VoiceText.edit(t, 2, 0, "X", 1, 1))
        assertEquals(3, VoiceText.index(t, 2))
    }
    @Test fun typingBeforeTheMarkMovesIt() {
        assertEquals(7, VoiceText.shift("hello world", "XXhello world", 5))
        assertEquals(5, VoiceText.shift("hello world", "hello worldXX", 5))
        assertEquals(4, VoiceText.shift("hello world", "ello world", 5))
    }

    // test/fixtures/voice-edits.json: the hub's own edits, applied the way the phone does, must reach its `want` (GPUI and iOS check the same file)
    private fun fixture(): JSONObject {
        var f = File("").absoluteFile
        while (!File(f, "test/fixtures/voice-edits.json").exists()) f = f.parentFile
        return JSONObject(File(f, "test/fixtures/voice-edits.json").readText())
    }
    @Test fun theSharedEdits() {
        val cases = fixture().getJSONArray("edits")
        for (i in 0 until cases.length()) {
            val c = cases.getJSONObject(i); val want = c.getJSONObject("want"); val edits = want.getJSONArray("edits")
            var text = c.getString("draft"); var mark = c.getInt("caret"); var caret = mark
            for (k in 0 until edits.length()) {
                val e = edits.getJSONObject(k)
                val r = VoiceText.edit(text, mark, e.getInt("del"), e.getString("ins"), e.getInt("adv"), e.getInt("to"))
                text = r.text; mark = r.mark; caret = r.caret
            }
            val name = c.getString("name")
            assertEquals(name, want.getString("text"), text); assertEquals(name, want.getInt("mark"), mark); assertEquals(name, want.getInt("caret"), caret)
        }
    }
    @Test fun theSharedApplyAndShift() {
        val f = fixture()
        val apply = f.getJSONArray("apply")
        for (i in 0 until apply.length()) {
            val c = apply.getJSONObject(i); val e = c.getJSONObject("edit"); val want = c.getJSONObject("want")
            val r = VoiceText.edit(c.getString("text"), c.getInt("mark"), e.getInt("del"), e.getString("ins"), e.getInt("adv"), e.getInt("to"))
            val name = c.getString("name")
            assertEquals(name, want.getString("text"), r.text); assertEquals(name, want.getInt("mark"), r.mark); assertEquals(name, want.getInt("caret"), r.caret)
        }
        val shift = f.getJSONArray("shift")
        for (i in 0 until shift.length()) {
            val c = shift.getJSONObject(i)
            assertEquals(c.getString("name"), c.getInt("want"), VoiceText.shift(c.getString("old"), c.getString("new"), c.getInt("mark")))
        }
    }
}
