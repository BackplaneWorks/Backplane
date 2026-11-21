package dev.backplane.mobile

import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.Request
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.TimeUnit

// The app's own download loop (FileFetchRun) against a live hub, through a relay that can cut every connection (scripts/mobile/tcp-relay.py).
// Skipped unless the rig sets BP_LIVE_URL (the big file's /file address through the relay), BP_LIVE_TOKEN, BP_LIVE_SHA256 (the file's),
// BP_LIVE_CUT (the relay's control address) and BP_LIVE_DENY (a /file address the hub refuses with 403). Rig: test/mobile/file-links-live.sh.
class FileFetchLiveTest {
    private fun env(k: String) = System.getenv(k).orEmpty()
    private val asked = mutableListOf<String>()
    private val http = OkHttpClient.Builder().readTimeout(30, TimeUnit.SECONDS)
        .addNetworkInterceptor { ch -> val r = ch.proceed(ch.request()); synchronized(asked) { asked += "${ch.request().header("Range") ?: "-"} ${r.code}" }; r }.build()
    private fun get(u: String) = OkHttpClient().newCall(Request.Builder().url(u).build()).execute().close()
    private fun sha(f: File): String { val d = MessageDigest.getInstance("SHA-256"); f.inputStream().use { i -> val b = ByteArray(1 shl 20); while (true) { val n = i.read(b); if (n < 0) break; d.update(b, 0, n) } }; return d.digest().joinToString("") { "%02x".format(it) } }

    @Test fun aBigFileResumesAfterTheLinkDrops() {
        assumeTrue("no live hub", env("BP_LIVE_URL").isNotEmpty())
        val dir = File(System.getProperty("java.io.tmpdir"), "bp-live-${System.nanoTime()}").apply { mkdirs() }
        try {
            val part = File(dir, "big.bin.part"); val tag = File(dir, "etag"); var cut = false
            val out = runBlocking {
                FileFetchRun.fetch(http, { FileFetchRun.Where(env("BP_LIVE_URL"), env("BP_LIVE_TOKEN")) }, part, tag, -1, "offline", "failed", { have, _ ->
                    // past 40 MB the relay drops the connection (and refuses new ones for a moment)
                    if (!cut && have > (40 shl 20)) { cut = true; get(env("BP_LIVE_CUT") + "/cut"); Thread { Thread.sleep(1500); get(env("BP_LIVE_CUT") + "/heal") }.start() }
                })
            }
            println("live requests: $asked")
            assertTrue("whole: $out", out is FileFetchRun.Out.Whole)
            assertTrue("it was cut", cut)
            assertEquals("the first ask is the whole file", "- 200", asked.first())
            assertTrue("a later ask went on from where it stopped with a 206: $asked", asked.drop(1).any { it.startsWith("bytes=") && it.endsWith(" 206") })
            assertEquals(env("BP_LIVE_SHA256"), sha(part))
        } finally { dir.deleteRecursively() }
    }

    @Test fun aRefusalShowsTheHubsWords() {
        assumeTrue("no live hub", env("BP_LIVE_DENY").isNotEmpty())
        val dir = File(System.getProperty("java.io.tmpdir"), "bp-live-${System.nanoTime()}").apply { mkdirs() }
        try {
            val out = runBlocking { FileFetchRun.fetch(http, { FileFetchRun.Where(env("BP_LIVE_DENY"), env("BP_LIVE_TOKEN")) }, File(dir, "x.part"), File(dir, "etag"), -1, "offline", "failed") }
            println("live refusal: $out $asked")
            assertEquals(FileFetchRun.Out.Fail("that file is not shared"), out)
            assertTrue(asked.single().endsWith(" 403"))
        } finally { dir.deleteRecursively() }
    }
}
