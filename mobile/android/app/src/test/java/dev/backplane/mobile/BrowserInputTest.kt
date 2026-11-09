package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

// test/mobile/browser_input_cases.json holds this and iOS's BrowserInput.swift to the same answers (scripts/mobile/ios-unit-tests.sh runs the Swift side).
class BrowserInputTest {
    private val cases: JSONObject by lazy {
        var f = File("").absoluteFile
        while (!File(f, "test/mobile/browser_input_cases.json").exists()) f = f.parentFile
        JSONObject(File(f, "test/mobile/browser_input_cases.json").readText())
    }
    private fun each(kind: String, check: (JSONObject) -> Unit) { val a = cases.getJSONArray(kind); assertTrue(a.length() > 0); for (i in 0 until a.length()) check(a.getJSONObject(i)) }

    @Test fun keys() = each("keys") { c ->
        val w = c.getJSONArray("want"); val want = (0 until w.length()).map { w.getJSONArray(it).let { p -> p.getString(0) to p.getString(1) } }
        val text = c.getString("text")
        assertEquals(c.getString("name"), want, BrowserInput.keys(c.optString("old", BrowserInput.SENTINEL), text))
        assertEquals(c.getString("name") + " (next)", c.getString("next"), BrowserInput.next(text))
    }
    @Test fun scroll() = each("scroll") { c ->
        val w = c.getJSONArray("want")
        assertEquals(c.getString("name"), w.getInt(0) to w.getInt(1), BrowserInput.scroll(c.getDouble("dx"), c.getDouble("dy"), c.getDouble("image"), c.getDouble("shown")))
    }
}
