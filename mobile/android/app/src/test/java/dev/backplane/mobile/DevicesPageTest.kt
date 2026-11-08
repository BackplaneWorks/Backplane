package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

// Settings, Devices on the phone: the opaque state it keeps for the hub, the codes it draws from a row, the presses it keeps to itself
class DevicesPageTest {
    @Test fun theLatestStateGoesBack() {
        val k = DevicesState()
        assertEquals("", k.to("a:3787"))
        assertTrue(k.took("a:3787", JSONObject().put("rows", JSONArray()).put("state", "s1")))
        assertEquals("s1", k.to("a:3787"))
        k.took("a:3787", JSONObject().put("state", "s2"))
        assertEquals("s2", k.to("a:3787"))
        // a reply without a state keeps the last one
        k.took("a:3787", JSONObject().put("rows", JSONArray()))
        assertEquals("s2", k.to("a:3787"))
    }

    @Test fun anotherHubStartsAfresh() {
        val k = DevicesState()
        k.to("a:3787"); k.took("a:3787", JSONObject().put("state", "s1"))
        assertEquals("", k.to("b:3787"))
        // the first hub's late reply is dropped, and its state never reaches the second
        assertFalse(k.took("a:3787", JSONObject().put("state", "late")))
        assertEquals("", k.to("b:3787"))
        // back on the first: it starts afresh too
        assertEquals("", k.to("a:3787"))
    }

    private fun row(qr: Any?) = JSONObject().put("label", "Pairing link").apply { if (qr != null) put("qr", qr) }

    @Test fun aRowDrawsASquareCode() {
        val rows = listOf("111", "101", "111")
        assertEquals(rows, PageRow.qr(row(JSONArray(rows))))
        assertEquals(emptyList<String>(), PageRow.qr(row(null)))
        assertEquals(emptyList<String>(), PageRow.qr(row(JSONArray())))
        // never half a code: a short row, a stray character, a number
        assertEquals(emptyList<String>(), PageRow.qr(row(JSONArray(listOf("111", "10", "111")))))
        assertEquals(emptyList<String>(), PageRow.qr(row(JSONArray(listOf("111", "1x1", "111")))))
        assertEquals(emptyList<String>(), PageRow.qr(row(JSONArray().put("111").put(101).put("111"))))
        assertEquals(emptyList<String>(), PageRow.qr(row("111\n101\n111")))
    }

    @Test fun thePhonesOwnPresses() {
        listOf("copy", "copy-text", "share-text", "open-url").forEach { assertTrue(it, PageRow.local(it)) }
        listOf("pair-new", "revoke", "address", "lan-on", "").forEach { assertFalse(it, PageRow.local(it)) }
        assertTrue(PageRow.webUrl("https://backplane.works/privacy"))
        assertTrue(PageRow.webUrl(" http://100.64.0.7:3787/pair "))
        assertFalse(PageRow.webUrl("https://"))
        assertFalse(PageRow.webUrl("javascript:alert(1)"))
        assertFalse(PageRow.webUrl("file:///sdcard/x"))
        assertFalse(PageRow.webUrl("intent://x#Intent;end"))
    }
}
