package dev.backplane.mobile

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException
import java.util.concurrent.TimeUnit

// Restarting the hub from the phone (docs/PHONE-DATA.md, "The Hub screen"): plain HTTP to the paired address, so it works with the data socket down, the hub's views broken, or only its guard answering.
// HubRescue decides what each answer shows, a pure function held to test/mobile/hub_cases.json with iOS's HubRescue.swift; HubHttp only carries the requests.
data class HubAction(val id: String, val label: String, val text: String = "")
data class HubIncident(val atMs: Long, val what: String, val why: String, val did: String)
data class HubNotice(val text: String = "", val tone: String = "") { val warn get() = tone == "warn" }
data class HubStatus(val hasGuard: Boolean, val text: String, val actions: List<HubAction>, val incidents: List<HubIncident>, val notice: HubNotice?)
data class HubHealth(val status: String, val safe: Boolean)

sealed interface HubFetch {
    object Loading : HubFetch
    data class Got(val status: HubStatus) : HubFetch
    data class Refused(val text: String) : HubFetch
    object Unreachable : HubFetch
}
sealed interface HubReply {
    data class Accepted(val text: String) : HubReply
    data class Refused(val text: String) : HubReply
    data class Credential(val text: String) : HubReply
    object Missing : HubReply
    object Unreachable : HubReply
}
data class HubPanel(val text: String, val incidents: List<HubIncident> = emptyList(), val actions: List<HubAction> = emptyList())
data class HubAfter(val text: String, val poll: Boolean)
data class HubStep(val text: String, val done: Boolean, val moved: Boolean)

object HubRescue {
    const val POLL_MS = 2_000L
    const val LIMIT_MS = 180_000L
    // The old hub answers for a moment after its 202, and rescue the same. An ok or a down counts once the restart has shown (a gap, starting, hung) or after this long: a restart can fall between two polls.
    const val SETTLE_MS = 10_000L
    private val RESTART = HubAction("restart", "Restart hub")
    private const val PAIRING = "The hub refused this phone's pairing. Pair it again."

    fun panel(f: HubFetch): HubPanel = when (f) {
        HubFetch.Loading -> HubPanel("Asking the hub…")
        is HubFetch.Got -> if (f.status.hasGuard) HubPanel(f.status.text, f.status.incidents, f.status.actions)
            else HubPanel("Restart is not available from here.", f.status.incidents, listOf(RESTART))
        is HubFetch.Refused -> HubPanel(f.text.ifEmpty { PAIRING })
        HubFetch.Unreachable -> HubPanel("The hub's status cannot be fetched.", actions = listOf(RESTART))
    }

    fun after(r: HubReply): HubAfter = when (r) {
        is HubReply.Accepted -> HubAfter(r.text.ifEmpty { "Sent." }, true)
        is HubReply.Refused -> HubAfter(r.text.ifEmpty { "The hub refused." }, false)
        is HubReply.Credential -> HubAfter(r.text.ifEmpty { PAIRING }, false)
        HubReply.Missing -> HubAfter("This hub cannot do that yet (update it).", false)
        HubReply.Unreachable -> HubAfter("The machine cannot be reached.", false)
    }

    /** One answer of /healthz (null: none) after the request was accepted. `moved`: the restart has shown itself. */
    fun step(h: HubHealth?, elapsedMs: Long, moved: Boolean): HubStep {
        val status = h?.status.orEmpty()
        val settled = status == "ok" || status == "degraded" || status == "down"
        val nowMoved = moved || !settled
        if (settled && (nowMoved || elapsedMs >= SETTLE_MS)) return HubStep(result(h!!), true, nowMoved)
        if (elapsedMs >= LIMIT_MS) return HubStep(if (status.isEmpty() || settled) "No answer from the hub after 3 minutes." else "Hub still $status after 3 minutes.", true, nowMoved)
        return HubStep(if (status.isEmpty() || settled) "Waiting for the hub…" else "Hub is $status…", false, nowMoved)
    }
    private fun result(h: HubHealth) = when (h.status) {
        "down" -> "Hub is down."
        "degraded" -> if (h.safe) "Hub is up in safe mode, degraded." else "Hub is up, degraded."
        else -> if (h.safe) "Hub is up in safe mode." else "Hub is up."
    }

    // ---- the hub's answers (a JSON null reads as "", which org.json's optString would not)
    private fun JSONObject.str(k: String) = if (isNull(k)) "" else opt(k).toString()
    private fun objects(a: JSONArray?) = if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }
    private fun json(body: String) = runCatching { JSONObject(body) }.getOrNull()

    fun notice(o: JSONObject?) = if (o == null) HubNotice() else HubNotice(o.str("text"), o.str("tone"))
    fun fetch(code: Int, body: String): HubFetch {
        val o = json(body)
        if (code == 401) return HubFetch.Refused(o?.str("text").orEmpty())
        if (code !in 200..299 || o == null || !o.has("guard")) return HubFetch.Unreachable
        val g = o.optJSONObject("guard")
        val actions = objects(g?.optJSONArray("actions")).map { a -> HubAction(a.str("id"), a.str("label").ifEmpty { a.str("id") }, a.str("text")) }
        // oldest first on the wire, `at` in seconds; newest first here
        val incidents = objects(o.optJSONArray("incidents")).map { HubIncident(it.optLong("at") * 1000, it.str("what"), it.str("why"), it.str("did")) }.reversed()
        return HubFetch.Got(HubStatus(g != null, g?.let { it.str("text").ifEmpty { it.str("state") } }.orEmpty(), actions, incidents, o.optJSONObject("notice")?.let(::notice)))
    }
    fun reply(code: Int, body: String): HubReply {
        val o = json(body); val text = o?.str("text").orEmpty()
        return when {
            code in 200..299 -> if (o?.optBoolean("ok", true) == false) HubReply.Refused(text) else HubReply.Accepted(text)
            code == 401 -> HubReply.Credential(text)
            code == 409 || text.isNotEmpty() -> HubReply.Refused(text)
            code == 404 -> HubReply.Missing
            else -> HubReply.Unreachable
        }
    }
    // a 503 with a body is an answer like a 200: only the body counts
    fun health(body: String) = json(body)?.let { o -> o.str("status").takeIf { it.isNotEmpty() }?.let { HubHealth(it, o.optBoolean("safe")) } }
}

class HubHttp(private val base: String, private val token: String?, healthTimeoutMs: Long = 5_000, callTimeoutMs: Long = 12_000) {
    // a client of its own: DataClient's has no timeouts, and a hung hub must not hold a request. The credential goes to the paired address and nowhere a redirect points.
    private val client = OkHttpClient.Builder().followRedirects(false).followSslRedirects(false).callTimeout(callTimeoutMs, TimeUnit.MILLISECONDS).build()
    private val quick = client.newBuilder().callTimeout(healthTimeoutMs, TimeUnit.MILLISECONDS).build()

    private fun request(path: String) = Request.Builder().url(base + path).apply { if (!token.isNullOrEmpty()) header("Authorization", "Bearer $token") }
    private suspend fun send(c: OkHttpClient, r: Request): Pair<Int, String> = withContext(Dispatchers.IO) {
        try { c.newCall(r).execute().use { it.code to (it.body?.string() ?: "") } } catch (e: IOException) { 0 to "" }
    }
    suspend fun status(): HubFetch { val (code, body) = send(client, request("/hub/status").build()); return HubRescue.fetch(code, body) }
    suspend fun control(action: String): HubReply {
        val body = JSONObject().put("action", action).toString().toRequestBody("application/json".toMediaType())
        val (code, text) = send(client, request("/hub/control").post(body).build())
        return HubRescue.reply(code, text)
    }
    suspend fun health(): HubHealth? = HubRescue.health(send(quick, Request.Builder().url("$base/healthz").build()).second)
}
