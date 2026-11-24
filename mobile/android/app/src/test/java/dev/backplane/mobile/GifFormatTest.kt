package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class GifFormatTest {
    @Test fun detectsGif87aAndGif89a() {
        assertTrue(GifFormat.isGif("GIF89a\u0002\u0000".toByteArray()))
        assertTrue(GifFormat.isGif("GIF87a\u0002\u0000".toByteArray()))
        assertFalse(GifFormat.isGif(byteArrayOf(0x89.toByte(), 'P'.code.toByte(), 'N'.code.toByte(), 'G'.code.toByte(), 0x0d, 0x0a)))
        assertFalse(GifFormat.isGif("GIF8".toByteArray()))
        assertEquals("image/gif", GifFormat.mimeOf("GIF89a\u0001\u0000".toByteArray()))
        assertEquals("gif", GifFormat.extOf("GIF89a\u0001\u0000".toByteArray()))
        assertEquals("image/jpeg", GifFormat.mimeOf(byteArrayOf(0xff.toByte(), 0xd8.toByte(), 0xff.toByte(), 0xe0.toByte(), 0, 0)))
        assertEquals("jpg", GifFormat.extOf(byteArrayOf(0xff.toByte(), 0xd8.toByte(), 0xff.toByte(), 0xe0.toByte(), 0, 0)))
    }

    @Test fun parseEntryReadsReplyImages() {
        val o = JSONObject()
            .put("id", "e1")
            .put("kind", "assistant")
            .put("text", "look /tmp/demo.gif")
            .put("at", 123L)
            .put("images", JSONArray().put("/tmp/demo.gif").put("/tmp/other.png"))
        val e = parseEntry(o)
        assertEquals(listOf("/tmp/demo.gif", "/tmp/other.png"), e.images)
    }
}
