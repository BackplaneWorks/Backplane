package dev.backplane.mobile

import com.whl.quickjs.android.QuickJSLoader
import com.whl.quickjs.wrapper.QuickJSContext
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine
import org.json.JSONObject

import java.util.concurrent.atomic.AtomicInteger

// A JavaScript runtime the Bend client runs in: evaluate an expression,
// answer its value as a string.
interface Js {
    fun evaluate(expr: String): String
}

// QuickJS, in this process: an interpreter, everywhere, and slow (a tap's
// screen took ~130 ms on an emulator, several times that on a phone)
class QuickJs(private val c: QuickJSContext) : Js {
    override fun evaluate(expr: String): String = c.evaluate(expr) as String
}

// V8 from the system WebView, in its sandbox process (androidx
// javascriptengine): the same client compiled, over ten times faster.
// Calls cross IPC, whose transactions are limited in size unless the
// WebView lifts the limit: then the source goes in and long answers come
// back in slices through globals.
class V8Js(private val iso: androidx.javascriptengine.JavaScriptIsolate, private val big: Boolean) : Js {
    private fun run(code: String): String = iso.evaluateJavaScriptAsync(code).get()

    // text into the global __s, slice by slice
    private fun push(text: String) {
        run("globalThis.__s = ''")
        var i = 0
        while (i < text.length) {
            run("__s += ${JSONObject.quote(text.substring(i, minOf(i + SLICE, text.length)))}")
            i += SLICE
        }
    }

    // Without the transaction limit an answer still comes back whole, and the
    // sandbox refuses one over 20 MB (EvaluationResultSizeLimitExceeded: the
    // kept state of a real log is 35 MB, and save() crashed the app every 30
    // s). An answer over BIG is left in __r and read in BIG slices; a script
    // over BIG goes in through __s.
    private fun evaluateBig(expr: String): String {
        val code = "(function(){var r = String($expr); if (r.length <= $BIG) return '-' + r; globalThis.__r = r; return '+' + r.length})()"
        val first = if (expr.length <= BIG) run(code)
        else { push(expr); run("(function(){var r = String((0, eval)(__s)); __s = ''; if (r.length <= $BIG) return '-' + r; globalThis.__r = r; return '+' + r.length})()") }
        if (first.startsWith("-")) return first.substring(1)
        val n = first.substring(1).toInt()
        val sb = StringBuilder(n)
        var i = 0
        while (i < n) { sb.append(run("__r.slice($i, ${i + BIG})")); i += BIG }
        run("__r = ''")
        return sb.toString()
    }

    override fun evaluate(expr: String): String {
        if (big) return evaluateBig(expr)
        val n = if (expr.length <= SLICE) run("globalThis.__r = String($expr); String(__r.length)").toInt()
        else { push(expr); run("globalThis.__r = String((0, eval)(__s)); __s = ''; String(__r.length)").toInt() }
        if (n <= SLICE) return run("__r")
        val sb = StringBuilder(n)
        var i = 0
        while (i < n) { sb.append(run("__r.slice($i, ${i + SLICE})")); i += SLICE }
        run("__r = ''")
        return sb.toString()
    }

    // a script too long for one transaction goes in as string slices
    fun load(source: String) {
        if (big && source.length <= BIG) { run(source); return }
        push(source)
        run("(0, eval)(__s); __s = ''")
    }

    companion object {
        const val SLICE = 200_000
        // characters per answer slice without the transaction limit: at most
        // 3 bytes each in UTF-8, so a slice stays well under the 20 MB cap
        const val BIG = 4_000_000

        // V8 when the device's WebView offers the sandbox, else null
        fun open(app: android.content.Context, source: String): V8Js? = runCatching {
            if (!androidx.javascriptengine.JavaScriptSandbox.isSupported()) return null
            val sb = androidx.javascriptengine.JavaScriptSandbox.createConnectedInstanceAsync(app).get()
            val params = androidx.javascriptengine.IsolateStartupParameters()
            if (sb.isFeatureSupported(androidx.javascriptengine.JavaScriptSandbox.JS_FEATURE_ISOLATE_MAX_HEAP_SIZE))
                params.maxHeapSizeBytes = 768L shl 20
            val iso = sb.createIsolate(params)
            val big = sb.isFeatureSupported(androidx.javascriptengine.JavaScriptSandbox.JS_FEATURE_EVALUATE_WITHOUT_TRANSACTION_LIMIT)
            V8Js(iso, big).also { it.load(source) }
        }.onFailure { android.util.Log.w("Backplane", "V8 sandbox unavailable: $it") }.getOrNull()
    }
}

// characters per slice when the kept state moves between its file and the engine
private const val KEEP = 1_000_000

// A single engine thread, with FIFO order in each lane. Person work can
// overtake queued hub frames; a running JavaScript call finishes first.
private class Worker : Thread(null, null, "bend", 64L shl 20) {
    private val monitor = java.lang.Object()
    private val people = java.util.ArrayDeque<() -> Unit>()
    private val background = java.util.ArrayDeque<() -> Unit>()
    val personWaiting: Boolean get() = synchronized(monitor) { people.isNotEmpty() }
    init { start() }
    fun enqueue(person: Boolean, job: () -> Unit) = synchronized(monitor) {
        (if (person) people else background).addLast(job)
        monitor.notify()
    }
    override fun run() {
        while (true) {
            val job = synchronized(monitor) {
                while (people.isEmpty() && background.isEmpty()) monitor.wait()
                (if (people.isNotEmpty()) people else background).removeFirst()
            }
            job()
        }
    }
}

// The Bend client (bridge.js) on one thread of its own: in V8 where the
// WebView offers it, else in QuickJS.
// Every call answers {"screen": ..., "cmds": [...]} as a string, except
// resume(), which answers {"since": "<n>", "origin": "<o>"}. The client is
// started with this install's id and its kept drafts before anything
// else runs.
// The bridge is compiled to QuickJS bytecode once per build and kept in
// dir: parsing and compiling 1.3 MB of JavaScript took half a second at
// every launch.
class Engine(private val app: android.content.Context, private val source: String, private val cid: String, private val drafts: String, private val dir: java.io.File) {
    private val worker = Worker()

    private suspend fun <T> queued(person: Boolean, f: () -> T): T = suspendCoroutine { k ->
        worker.enqueue(person) {
            try { k.resume(f()) } catch (t: Throwable) { k.resumeWithException(t) }
        }
    }
    private var ctx: Js? = null
    // V8's sandbox died once: QuickJS for the rest of this process
    private var noV8 = false
    // called (on the engine's thread) when the client started over in a
    // new engine: its state is gone, so the app loads the kept state and
    // connects again
    var onReset: (() -> Unit)? = null

    private fun context(): Js =
        ctx ?: run {
            val t0 = android.os.SystemClock.elapsedRealtime()
            val js: Js = (if (noV8) null else V8Js.open(app, source)) ?: run {
                QuickJSLoader.init()
                QuickJSContext.create().let {
                    it.setMaxStackSize(48 shl 20)
                    load(it)
                    QuickJs(it)
                }
            }
            android.util.Log.i("Backplane", "engine ${js.javaClass.simpleName} in ${android.os.SystemClock.elapsedRealtime() - t0} ms")
            js.evaluate("Backplane.start(${q(cid)}, ${q(drafts)})")
            ctx = js
            js
        }

    // the bridge from its kept bytecode, else compiled now and kept (a
    // bytecode file of another build, or one that fails, is dropped)
    private fun load(c: QuickJSContext) {
        val name = "bridge-%08x-%d.qjs".format(source.hashCode(), source.length)
        dir.listFiles { f -> f.name.startsWith("bridge-") && f.name != name }?.forEach { it.delete() }
        val kept = java.io.File(dir, name)
        if (kept.exists() && runCatching { c.execute(kept.readBytes()) }.isSuccess) return
        kept.delete()
        val code = runCatching { c.compile(source, "bridge.js") }.getOrNull()
        if (code == null) { c.evaluate(source, "bridge.js"); return }
        c.execute(code)
        runCatching { java.io.File(dir, "$name.tmp").also { it.writeBytes(code) }.renameTo(kept) }
    }

    // The sandbox is another process: when it dies (a native crash in it,
    // such as a stack overflow its V8 does not catch) every call on it
    // throws, and the app died with it. Now the client starts again in
    // QuickJS, in this process, and the call is made there.
    private fun dead(t: Throwable) = generateSequence(t) { it.cause }.any { it is androidx.javascriptengine.IsolateTerminatedException }

    private fun eval(e: String): String = try {
        context().evaluate(e)
    } catch (t: Throwable) {
        if (ctx !is V8Js || !dead(t)) throw t
        android.util.Log.w("Backplane", "V8 sandbox died ($t): the client starts again in QuickJS")
        ctx = null
        noV8 = true
        val r = context().evaluate(e)
        onReset?.invoke()
        r
    }

    // calls queued whose answer carries a screen: while one is, the screen
    // before it is out of date before it could be drawn, so it is skipped
    private val personAhead = AtomicInteger(0)
    private val backgroundAhead = AtomicInteger(0)
    // a screen was dropped that way: the next recv builds one even for a
    // frame that changes nothing on screen (bridge.js's owed)
    private val owe = java.util.concurrent.atomic.AtomicBoolean(false)
    // Only the bend thread owns these. Timers enqueue background work;
    // they never evaluate JavaScript themselves or delay a person call.
    private val timer = android.os.Handler(android.os.Looper.getMainLooper())
    private val window = 250L
    private var built = 0L
    private var flushPending = false
    var onFlush: ((Reply) -> Unit)? = null

    private fun scheduleFlush() {
        if (flushPending) return
        flushPending = true
        val delay = maxOf(0L, window - (android.os.SystemClock.elapsedRealtime() - built))
        timer.postDelayed({ worker.enqueue(false) { flushReady() } }, delay)
    }

    private fun flushReady() {
        flushPending = false
        if (worker.personWaiting || android.os.SystemClock.elapsedRealtime() - built < window) {
            scheduleFlush()
            return
        }
        val began = android.os.SystemClock.elapsedRealtime()
        val o = JSONObject(eval("Backplane.flush(${owe.get()})"))
        val raw = o.optJSONObject("screen")
        val stale = raw != null && worker.personWaiting
        if (raw != null) { built = began; owe.set(stale) }
        if (stale) scheduleFlush()
        val s = if (stale) null else runCatching { raw?.let(::parseScreen) }
            .onFailure { android.util.Log.w("Backplane", "screen unreadable: $it") }.getOrNull()
        onFlush?.invoke(Reply(s, parseCmds(o)))
    }

    private suspend fun call(expr: String, person: Boolean = false): String =
        queued(person) { eval(expr) }

    // Parse off the main thread; commands survive every discarded screen.
    private suspend fun out(expr: String, screen: Boolean = true, person: Boolean = false, late: ((Boolean, Boolean) -> String)? = null): Reply {
        val ahead = if (person) personAhead else backgroundAhead
        if (screen) ahead.incrementAndGet()
        return queued(person) {
            val t0 = android.os.SystemClock.elapsedRealtime()
            // a streaming frame builds at most one screen per window; a burst
            // (a cold join, catching up) one a second, as before, while
            // more frames wait behind this one
            val withheld = late != null && (worker.personWaiting || t0 - built < window ||
                (backgroundAhead.get() > 1 && t0 - built < 1000))
            val e = late?.invoke(withheld, owe.getAndSet(false)) ?: expr
            val text = try { eval(e) } catch (t: Throwable) {
                if (screen) ahead.decrementAndGet()
                throw t
            }
            val ms = android.os.SystemClock.elapsedRealtime() - t0
            if (ms > 100) android.util.Log.i("Backplane", "slow ${e.substringBefore('(')}: $ms ms")
            val remaining = if (screen) ahead.decrementAndGet() else 0
            val o = JSONObject(text)
            val raw = o.optJSONObject("screen")
            val stale = raw != null && (if (person) remaining > 0 else worker.personWaiting)
            if (raw != null) {
                if (!person) built = t0
                owe.set(stale)
            }
            if (withheld || stale) scheduleFlush()
            val s = if (stale) null else runCatching { raw?.let(::parseScreen) }
                .onFailure { android.util.Log.w("Backplane", "screen unreadable: $it") }.getOrNull()
            Reply(s, parseCmds(o))
        }
    }

    private fun q(s: String) = JSONObject.quote(s)

    // the paired hubs' keys, in order
    suspend fun hubs(keys: List<String>) = out("Backplane.hubs(${org.json.JSONArray(keys)})")
    // a choice of this device that Bend draws by ("view": "projects" or "active")
    suspend fun local(key: String, value: String) = out("Backplane.local(${q(key)}, ${q(value)})", person = true)
    suspend fun resume(key: String) = call("Backplane.resume(${q(key)})")
    suspend fun screen() = out("Backplane.screen()", person = true)
    // a binary frame from hub key, as base64
    suspend fun recv(key: String, data: String) =
        out("", late = { quiet, owed -> "Backplane.recv(${q(key)}, ${q(data)}, $quiet, $owed)" })
    // the whole client state as text (StateStore), and the state loaded back
    suspend fun save() = call("Backplane.save()")
    suspend fun load(text: String) = out("Backplane.load(${q(text)})")

    // The kept state straight to and from its file, a slice at a time: a real
    // log's state is ~35 MB, and as one Java string (read whole, then quoted
    // into the call) it ran a phone out of heap at launch, as one V8 answer it
    // was over the sandbox's 20 MB. A slice never ends inside a surrogate
    // pair (the sandbox carries text as UTF-8).
    suspend fun saveTo(file: java.io.File): Boolean = queued(false) {
        val n = eval("(globalThis.__k = String(Backplane.save()), String(__k.length))").toInt()
        if (n == 0) return@queued false
        file.bufferedWriter().use { w ->
            var i = 0
            while (i < n) {
                val r = eval("(function(){var e = Math.min($i + $KEEP, __k.length); var c = __k.charCodeAt(e - 1); if (e < __k.length && c >= 0xD800 && c <= 0xDBFF) e--; return e + '|' + __k.slice($i, e)})()")
                val bar = r.indexOf('|')
                w.write(r, bar + 1, r.length - bar - 1)
                i = r.substring(0, bar).toInt()
            }
        }
        eval("(globalThis.__k = '', '')")
        true
    }

    suspend fun loadFrom(file: java.io.File): Reply {
        queued(false) {
            eval("(globalThis.__k = '', '')")
            file.bufferedReader().use { r ->
                val buf = CharArray(KEEP)
                var carry = ""
                while (true) {
                    val k = r.read(buf)
                    if (k < 0) break
                    var piece = carry + String(buf, 0, k)
                    carry = ""
                    if (Character.isHighSurrogate(piece.last())) { carry = piece.takeLast(1); piece = piece.dropLast(1) }
                    eval("(__k += ${q(piece)}, '')")
                }
                if (carry.isNotEmpty()) eval("(__k += ${q(carry)}, '')")
            }
        }
        return out("(function(){var t = __k; __k = ''; return Backplane.load(t)})()")
    }
    suspend fun act(action: String, value: String) = out("Backplane.act(${q(action)}, ${q(value)})", person = true)
    suspend fun quiet(action: String, value: String) = out("Backplane.quiet(${q(action)}, ${q(value)})", screen = false, person = true)
    suspend fun online(key: String, b: Boolean) = out("Backplane.online(${q(key)}, $b)")
    // a hub marked offline with no screen of its own (after load())
    suspend fun offline(key: String) = out("Backplane.online(${q(key)}, false, true)", screen = false)
    suspend fun tick(now: Long) = out("Backplane.tick($now)")
    // a catalog text by key, in the device's language
    suspend fun t(key: String) = call("Backplane.t(${q(key)})", person = true)
    // a cat's rig (JSON text) for its key ("look:mood")
    suspend fun cat(key: String) = call("Backplane.cat(${q(key)})", person = true)
}
