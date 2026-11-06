package dev.backplane.mobile

import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.SocketException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.concurrent.thread

// test/mobile/hub_cases.json holds this and iOS's HubRescue.swift to the same decisions (scripts/mobile/hub-check.sh runs the Swift side); the stub below is the hub's HTTP.
class HubRescueTest {
    private val cases: JSONObject by lazy {
        var f = File("").absoluteFile
        while (!File(f, "test/mobile/hub_cases.json").exists()) f = f.parentFile
        JSONObject(File(f, "test/mobile/hub_cases.json").readText())
    }
    private fun each(kind: String, check: (JSONObject) -> Unit) { val a: JSONArray = cases.getJSONArray(kind); assertTrue(a.length() > 0); for (i in 0 until a.length()) check(a.getJSONObject(i)) }
    private fun body(c: JSONObject) = c.get("body").toString()

    @Test fun panels() = each("panel") { c ->
        val n = c.getString("name")
        val f = if (c.getInt("code") == 0) HubFetch.Unreachable else HubRescue.fetch(c.getInt("code"), body(c))
        val p = HubRescue.panel(f)
        assertEquals(n, c.getString("text"), p.text)
        assertEquals(n, c.getJSONArray("actions").let { a -> (0 until a.length()).map { a.getString(it) } }, p.actions.map { it.id + ":" + it.label })
        assertEquals(n, c.getJSONArray("incidents").let { a -> (0 until a.length()).map { a.getString(it) } }, p.incidents.map { it.what })
        if (c.has("firstAtMs")) assertEquals(n, c.getLong("firstAtMs"), p.incidents[0].atMs)
        if (c.has("notice")) assertEquals(n, c.getString("notice"), (f as HubFetch.Got).status.notice?.text)
    }
    @Test fun replies() = each("reply") { c ->
        val r = if (c.getInt("code") == 0) HubReply.Unreachable else HubRescue.reply(c.getInt("code"), body(c))
        assertEquals(c.getString("name"), HubAfter(c.getString("text"), c.getBoolean("poll")), HubRescue.after(r))
    }
    @Test fun steps() = each("step") { c ->
        val h = if (c.getInt("code") == 0) null else HubRescue.health(body(c))
        assertEquals(c.getString("name"), HubStep(c.getString("text"), c.getBoolean("done"), c.getBoolean("movedAfter")), HubRescue.step(h, c.getLong("elapsed"), c.getBoolean("moved")))
    }
    @Test fun notices() = each("notice") { c ->
        val n = HubRescue.notice(c.optJSONObject("body"))
        assertEquals(c.getString("name"), c.getString("text"), n.text)
        assertEquals(c.getString("name"), c.getBoolean("warn"), n.warn)
    }
    @Test fun aWaitNeverOutlivesTheLimitWhateverTheAnswers() {
        for (h in listOf(null, HubHealth("starting", false), HubHealth("hung", false), HubHealth("ok", false), HubHealth("down", false)))
            assertTrue(HubRescue.step(h, HubRescue.LIMIT_MS, false).done)
    }

    // ---- over real HTTP, against a stub of the hub's routes (a plain socket: android.jar hides the JDK's HttpServer from the compiler)
    private data class Seen(val method: String, val uri: String, val auth: String?, val body: String)
    private class Route(val code: Int, val body: String, val location: String? = null, val holdMs: Long = 0)
    private val seen = CopyOnWriteArrayList<Seen>()
    private val routes = ConcurrentHashMap<String, Route>()
    private val server = ServerSocket(0, 50, InetAddress.getByName("127.0.0.1"))
    private val base get() = "http://127.0.0.1:${server.localPort}"
    private fun answer(path: String, code: Int, body: String, location: String? = null, holdMs: Long = 0) { routes[path] = Route(code, body, location, holdMs) }
    private fun serve(s: Socket) = s.use {
        val input = s.getInputStream().buffered()
        fun line() = generateSequence { input.read().takeIf { it >= 0 } }.takeWhile { it != 10 }.map { it.toChar() }.joinToString("").trimEnd('\r')
        val (method, uri) = line().split(" ")
        val heads = generateSequence { line().takeIf { it.isNotEmpty() } }.associate { it.substringBefore(":").lowercase() to it.substringAfter(":").trim() }
        val body = ByteArray(heads["content-length"]?.toInt() ?: 0).also { b -> var n = 0; while (n < b.size) n += input.read(b, n, b.size - n) }.decodeToString()
        seen += Seen(method, uri, heads["authorization"], body)
        val r = routes[uri] ?: Route(404, "")
        Thread.sleep(r.holdMs)
        val out = r.body.toByteArray()
        s.getOutputStream().write(("HTTP/1.1 ${r.code} X\r\nContent-Length: ${out.size}\r\nConnection: close\r\n" + (r.location?.let { "Location: $it\r\n" } ?: "") + "\r\n").toByteArray() + out)
    }
    private fun start() { thread(isDaemon = true) { try { while (true) { val s = server.accept(); thread(isDaemon = true) { runCatching { serve(s) } } } } catch (e: SocketException) { } } }
    @After fun stop() = server.close()

    @Test fun theCredentialGoesInTheHeaderAndHealthGoesWithout() = runBlocking {
        answer("/healthz", 503, """{"status":"down","rescue":1}""")
        answer("/hub/status", 200, """{"guard":{"state":"rescue","text":"Hub is down: x","actions":[{"id":"restart","label":"Restart hub"}]},"incidents":[],"rescue":1}""")
        answer("/hub/control", 202, """{"ok":true,"text":"restarting"}""")
        start()
        val hub = HubHttp(base, "abc123")
        assertEquals(HubHealth("down", false), hub.health())
        assertEquals("Hub is down: x", HubRescue.panel(hub.status()).text)
        assertEquals(HubReply.Accepted("restarting"), hub.control("restart"))
        assertEquals(listOf("GET /healthz", "GET /hub/status", "POST /hub/control"), seen.map { it.method + " " + it.uri })
        assertEquals(listOf(null, "Bearer abc123", "Bearer abc123"), seen.map { it.auth })
        assertEquals("restart", JSONObject(seen[2].body).getString("action"))
    }
    @Test fun aRefusalAndARefusedCredentialAreAnswers() = runBlocking {
        answer("/hub/control", 409, """{"ok":false,"text":"wait"}"""); answer("/hub/status", 401, """{"ok":false,"text":"needs its token"}"""); start()
        val hub = HubHttp(base, "abc123")
        assertEquals(HubReply.Refused("wait"), hub.control("restart"))
        assertEquals(HubFetch.Refused("needs its token"), hub.status())
    }
    @Test fun aRedirectIsNotFollowedWithTheCredential() = runBlocking {
        answer("/hub/control", 302, "", "$base/elsewhere"); answer("/elsewhere", 202, """{"ok":true}"""); start()
        assertEquals(HubReply.Unreachable, HubHttp(base, "abc123").control("restart"))
        assertEquals(listOf("/hub/control"), seen.map { it.uri })
    }
    @Test fun aHungHubDoesNotHoldARequest() = runBlocking {
        answer("/healthz", 200, """{"status":"ok"}""", holdMs = 3000); start()
        val t = System.currentTimeMillis()
        assertNull(HubHttp(base, null, healthTimeoutMs = 200).health())
        assertTrue(System.currentTimeMillis() - t < 2000)
    }
    @Test fun aMachineThatIsGoneIsUnreachable() = runBlocking {
        val hub = HubHttp("http://127.0.0.1:1", "abc123", 500, 500)
        assertEquals(HubFetch.Unreachable, hub.status())
        assertEquals(HubReply.Unreachable, hub.control("restart"))
        assertNull(hub.health())
        assertFalse(HubRescue.after(HubReply.Unreachable).poll)
    }
}
