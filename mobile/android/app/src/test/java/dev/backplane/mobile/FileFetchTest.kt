package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

// Files of the hub's machine (docs/file-links.md): which link is a file to download, its address, and how a stopped download goes on
class FileFetchTest {
    private val tok = "0123456789abcdef0123456789abcdef0123456789abcdef"
    private val files = listOf(DFileLink("/home/user/p/mfg/gerbers.zip", "gerbers.zip", 123, "download"), DFileLink("~/p/board.kicad_pcb", "board.kicad_pcb", 9, "view"), DFileLink("/home/user/p/docs/My Doc.pdf", "My Doc.pdf", 456, "view"))

    @Test fun sharesAreLinks() {
        assertEquals("/s/$tok/gerbers.zip", FileFetch.shareRoute("/s/$tok/gerbers.zip"))
        assertEquals("/s/$tok/POWER_TEST.md", FileFetch.shareRoute("/s/$tok/POWER_TEST.md?inline=1#top"))
        assertEquals("/s/$tok/my%20case.step", FileFetch.shareRoute("/s/$tok/my%20case.step"))
        assertNull(FileFetch.shareRoute("/s/short/gerbers.zip"))
        assertNull(FileFetch.shareRoute("/s/$tok/a/b.zip"))
        assertNull(FileFetch.shareRoute("/s/$tok/"))
        // only what the hub's share_route writes: nothing that takes the address to another route (a review found `..\..\file?path=` did, on OkHttp)
        listOf("..\\..\\file?path=%2Fx", "..", ".", "a\\b", "%zz", "a b", "?b", "#b").forEach { assertNull(it, FileFetch.shareRoute("/s/$tok/$it")) }
        // a query or a fragment after the name is dropped, never sent: the route is the name alone
        listOf("a?b", "a#b", "a?path=%2Fx#L3").forEach { assertEquals(it, "/s/$tok/a", FileFetch.shareRoute("/s/$tok/$it")) }
        assertTrue(FileFetch.isShare("/s/$tok/a.zip")); assertFalse(FileFetch.isShare("/file"))
        assertEquals("download", FileFetch.safeName("..")); assertEquals("download", FileFetch.safeName(" . ")); assertEquals("a_b_c", FileFetch.safeName("a/b\\c"))
        // in running text, a share is a link as a path is (a name without an extension too), and what follows it in a sentence stays out
        assertEquals(listOf("bppath:/s/$tok/my%20case.step|/s/$tok/my%20case.step"), linkRuns("get /s/$tok/my%20case.step.").mapNotNull { (t, u) -> u?.let { "$it|$t" } })
        assertEquals(listOf("bppath:/s/$tok/README|/s/$tok/README"), linkRuns("(/s/$tok/README)").mapNotNull { (t, u) -> u?.let { "$it|$t" } })
        assertEquals("my case.step", FileFetch.shareName("/s/$tok/my%20case.step"))
        assertEquals("a+b.zip", FileFetch.shareName("/s/$tok/a+b.zip"))
    }

    @Test fun aTappedPathFindsItsFile() {
        assertEquals("gerbers.zip", FileFetch.find("/home/user/p/mfg/gerbers.zip", files)?.name)
        assertEquals("gerbers.zip", FileFetch.find("file:///home/user/p/mfg/gerbers.zip", files)?.name)
        assertEquals("My Doc.pdf", FileFetch.find("file:///home/user/p/docs/My%20Doc.pdf", files)?.name)
        assertEquals("My Doc.pdf", FileFetch.find("file://localhost/home/user/p/docs/My%20Doc.pdf#page=2", files)?.name)
        // a link carries the line it points at; the file is the same
        assertEquals("board.kicad_pcb", FileFetch.find("~/p/board.kicad_pcb:12", files)?.name)
        assertEquals("board.kicad_pcb", FileFetch.find("~/p/board.kicad_pcb:3:4", files)?.name)
        assertEquals("gerbers.zip", FileFetch.find("/home/user/p/mfg/gerbers.zip#L4-L9", files)?.name)
        assertNull(FileFetch.find("/home/user/p/mfg/other.zip", files))
        assertNull(FileFetch.find("mfg/gerbers.zip", files))
    }

    @Test fun theAddress() {
        val enc = { s: String -> java.net.URLEncoder.encode(s, "UTF-8").replace("+", "%20") }
        assertEquals("path=%2Fhome%2Fuser%2Fa%20b.zip", FileFetch.fileQuery("/home/user/a b.zip", "t12", enc))
        assertEquals("path=%7E%2Fp%2Fx.zip&thread=desk%7Et9", FileFetch.fileQuery("~/p/x.zip", "desk~t9", enc))
        assertFalse(FileFetch.far("t9")); assertTrue(FileFetch.far("desk~t9"))
    }

    @Test fun anEntryCarriesItsFiles() {
        val e = parseEntry(JSONObject().put("id", "a1").put("kind", "assistant").put("text", "see /x/y.zip")
            .put("files", JSONArray().put(JSONObject().put("path", "/x/y.zip").put("name", "y.zip").put("size", 104857600L).put("open", "download").put("project", "pa").put("absPath", "/real/x/y.zip")).put(JSONObject().put("name", "no path"))))
        assertEquals(listOf(DFileLink("/x/y.zip", "y.zip", 104857600L, "download", "pa", "/real/x/y.zip")), e.files)
        assertEquals("pa", FileFetch.find("/real/x/y.zip", e.files)?.project)
        assertEquals(emptyList<DFileLink>(), parseEntry(JSONObject().put("id", "a2").put("kind", "assistant")).files)
    }

    @Test fun resumeAsksForTheRest() {
        assertEquals(emptyMap<String, String>(), FileFetch.ask(0, "\"1-2\""))
        // without the first answer's tag the rest cannot be joined to it: the start again
        assertEquals(emptyMap<String, String>(), FileFetch.ask(500, ""))
        assertEquals(mapOf("Range" to "bytes=500-", "If-Range" to "\"64-abc\""), FileFetch.ask(500, "\"64-abc\""))
    }

    @Test fun answers() {
        // 206 that starts where the part ends: append, and the whole file's length from Content-Range
        assertEquals(FileFetch.Step.Write(500, 104857600), FileFetch.step(206, 500, "bytes 500-104857599/104857600", 104857100))
        // a range that does not join the part: drop it and start again
        assertEquals(FileFetch.Step.Again, FileFetch.step(206, 500, "bytes 0-99/100", 100))
        assertEquals(FileFetch.Step.Again, FileFetch.step(206, 500, null, 100))
        // 200: the file changed (a stale If-Range) or a first ask: start over
        assertEquals(FileFetch.Step.Write(0, 2048), FileFetch.step(200, 500, null, 2048))
        assertEquals(FileFetch.Step.Write(0, 2048), FileFetch.step(200, 0, null, 2048))
        // past the end: the part is already the file, or it is no good
        assertEquals(FileFetch.Step.Done, FileFetch.step(416, 2048, "bytes */2048", 0))
        assertEquals(FileFetch.Step.Again, FileFetch.step(416, 4096, "bytes */2048", 0))
        // the hub's own words for a refusal
        assertEquals(FileFetch.Step.Fail("that file is not shared"), FileFetch.step(403, 0, null, 23, "that file is not shared\n"))
        assertEquals(FileFetch.Step.Fail("no such file"), FileFetch.step(404, 0, null, 12, "no such file"))
        assertEquals(FileFetch.Step.Fail("HTTP 502"), FileFetch.step(502, 0, null, 0, ""))
    }

    @Test fun anAddressStaysOnItsRoute() {
        val u = { s: String -> okhttp3.HttpUrl.Companion.run { s.toHttpUrlOrNull()!! } }
        assertTrue(FileFetch.sameRoute(u("http://h:3787/s/$tok/my%20case.step").encodedPath, "/s/$tok/my%20case.step"))
        assertTrue(FileFetch.sameRoute(u("http://h:3787/file?path=%2Fa").encodedPath, "/file"))
        // OkHttp resolves `..` and `\`: such an address is another route, and is never asked
        assertFalse(FileFetch.sameRoute(u("http://h:3787/s/$tok/..\\..\\file?path=%2Fx").encodedPath, "/s/$tok/..\\..\\file"))
    }

    @Test fun aShareRowsRevokeGoesToTheHub() {
        // Settings → Devices draws the hub's "Shared file · <name>" rows; their Revoke is a row button the hub answers (devices.page.act), never the phone's own
        assertFalse(PageRow.local("device-share-revoke"))
    }

    @Test fun wholeAndBackoff() {
        assertTrue(FileFetch.complete(100, 100)); assertFalse(FileFetch.complete(99, 100)); assertFalse(FileFetch.complete(100, -1))
        assertEquals(1000L, FileFetch.backoffMs(1)); assertEquals(2000L, FileFetch.backoffMs(2)); assertEquals(30_000L, FileFetch.backoffMs(9))
    }
}
