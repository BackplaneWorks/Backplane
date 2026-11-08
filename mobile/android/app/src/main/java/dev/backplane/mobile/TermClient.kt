package dev.backplane.mobile

import androidx.compose.runtime.mutableStateMapOf
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import org.json.JSONObject

// The phone's terminals (feature `terms`, docs/PHONE-DATA.md Terminals): the screens held per key, watched only while a view shows them, fed by
// `term.watch` replies and `{t:"d",k:"term"}` pushes through TermScreen's offset rule. No timers, no polling: a view redraws when bytes arrive.
object Terms {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val screens = HashMap<String, TermScreen>()
    private val revs = mutableStateMapOf<String, Int>()
    private val refs = HashMap<String, Int>()
    private val reading = HashSet<String>()
    private val pending = HashMap<String, MutableList<TFrame>>()
    private val outq = HashMap<String, Channel<ByteArray>>()

    /** The screen held for a terminal (a blank one until the hub answered). Read `rev` first so a view redraws when it changes. */
    fun screen(key: String): TermScreen = screens.getOrPut(key) { TermScreen() }
    fun rev(key: String): Int = revs[key] ?: 0
    private fun bump(key: String) { revs[key] = (revs[key] ?: 0) + 1 }

    private fun frame(o: JSONObject): TFrame {
        val b = if (o.has("b") && !o.isNull("b")) runCatching { android.util.Base64.decode(o.optString("b"), android.util.Base64.DEFAULT) }.getOrNull() ?: ByteArray(0) else ByteArray(0)
        fun long(k: String): Long? = if (o.has(k) && !o.isNull(k)) o.optLong(k) else null
        return TFrame(long("g"), long("o"), b, long("from"), o.optJSONArray("reset")?.let { it.optInt(0, 80) to it.optInt(1, 24) },
            if (o.has("state") && !o.isNull("state")) o.optString("state") else null, if (o.has("title")) o.optString("title") else null,
            if (o.has("running")) o.optBoolean("running") else null)
    }

    /** A view shows this terminal: ask the hub for what this phone lacks (the g and from held) and for pushes from now on. */
    fun watch(key: String) {
        refs[key] = (refs[key] ?: 0) + 1
        scope.launch { fetch(key, "term.watch", true) }
    }
    /** The view left the screen (or the app went to the background): no more pushes; the last screen stays. */
    fun unwatch(key: String) {
        val n = (refs[key] ?: 1) - 1
        if (n > 0) { refs[key] = n; return }
        refs.remove(key); pending.remove(key)
        scope.launch { runCatching { DataClient.call("term.unwatch", JSONObject().put("key", key)) } }
    }

    private suspend fun fetch(key: String, method: String, resume: Boolean) {
        if (!reading.add(key)) return
        val sc = screen(key)
        var got: TFrame? = null
        try {
            val p = JSONObject().put("key", key)
            if (resume && sc.g != 0L && sc.held >= 0) p.put("g", sc.g).put("from", sc.held)
            got = frame(DataClient.call(method, p))
        } catch (e: CancellationException) { throw e } catch (e: Exception) { /* offline or gone: the screen held stays */ }
        finally { reading.remove(key) }
        if (got != null) { sc.read(got); bump(key) }
        pending.remove(key)?.forEach { apply(key, sc, it) }
    }

    /** `{t:"d", k:"term", key, b, g?, o?, reset?, state?}` */
    fun push(o: JSONObject) {
        val key = o.optString("key"); val sc = screens[key] ?: return
        val f = frame(o)
        if (key in reading) { pending.getOrPut(key) { ArrayList() }.add(f); return }
        apply(key, sc, f)
    }
    private fun apply(key: String, sc: TermScreen, f: TFrame) {
        val next = sc.push(f); bump(key)
        when (next) {
            is TNext.Resume -> scope.launch { fetch(key, "term.read", true) }
            TNext.Reread -> scope.launch { fetch(key, "term.read", false) }
            TNext.Nothing -> {}
        }
    }

    // ---- input, in order: one writer per terminal, whatever has queued goes in one write
    fun send(key: String, bytes: ByteArray) {
        if (bytes.isEmpty()) return
        val ch = outq.getOrPut(key) {
            Channel<ByteArray>(Channel.UNLIMITED).also { c ->
                scope.launch {
                    for (first in c) {
                        var all = first
                        while (true) { val more = c.tryReceive().getOrNull() ?: break; all += more }
                        runCatching { DataClient.call("term.write", JSONObject().put("key", key).put("b", android.util.Base64.encodeToString(all, android.util.Base64.NO_WRAP))) }
                    }
                }
            }
        }
        ch.trySend(bytes)
    }
    fun resize(key: String, cols: Int, rows: Int) { scope.launch { runCatching { DataClient.call("term.resize", JSONObject().put("key", key).put("cols", cols).put("rows", rows)) } } }
    fun stop(key: String) { scope.launch { runCatching { DataClient.call("term.stop", JSONObject().put("key", key)) } } }

    // ---- the Terminals list
    data class Row(val key: String, val kind: String, val title: String, val state: String, val running: Boolean, val cwd: String, val at: Long)
    suspend fun list(project: String?): List<Row> {
        val a = DataClient.call("term.list", JSONObject().apply { if (!project.isNullOrEmpty()) put("project", project) }).optJSONArray("terms") ?: return emptyList()
        return (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Row(it.optString("key"), it.optString("kind"), it.optString("title"), it.optString("state"), it.optBoolean("running"), it.optString("cwd"), it.optLong("at")) }
    }
    /** A shell of its own in the project's folder (the hub's home folder without one); its key. */
    suspend fun newShell(project: String?, cols: Int, rows: Int): String {
        val r = DataClient.call("term.new", JSONObject().apply { if (!project.isNullOrEmpty()) put("project", project) }.put("cols", cols).put("rows", rows))
        val key = r.optString("key"); val sc = screen(key); sc.read(frame(r)); bump(key); return key
    }
}
