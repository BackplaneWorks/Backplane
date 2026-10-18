package dev.backplane.mobile

import android.app.Activity
import android.app.Application
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.util.Base64
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.security.SecureRandom

// The app's one Backplane client, alive as long as the process: the Bend
// engine, a socket per paired hub (keyed by host:port, Pairing.key), the
// current screen and the composer. Activities
// come and go and only observe it; the live service keeps the process
// (and so this) running while agents work.
class Core(private val app: Application) : Application.ActivityLifecycleCallbacks {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val prefs = app.getSharedPreferences("backplane", Context.MODE_PRIVATE)
    private val engine = Engine(app, app.assets.open("bridge.js").bufferedReader().readText(), cid(), prefs.getString("drafts", "{}") ?: "{}", app.filesDir)
    private val hubs = mutableMapOf<String, Hub>()
    // the client's state, kept for the next launch (StateStore)
    private val kept = StateStore(app.filesDir, app.assets.open("bridge.js").bufferedReader().readText())
    // something changed since the state was last kept
    private var dirty = false
    // the hub whose plots are drawn: frames from any other are dropped
    private var shownHub = ""

    // the pairing links, one per hub, in the order they were paired
    var links by mutableStateOf(loadLinks())
        private set
    var screen by mutableStateOf<Screen?>(null)
    // the last status plane the hub sent, drawn while a screen carries none (the
    // hub builds the plane only for the list and the Status tab, so going back
    // from a thread shows this one at once)
    var lastUber by mutableStateOf<UberCard?>(null)
    var lastPlane by mutableStateOf<PlaneModel?>(null)
        private set
    data class Opening(val nav: String, val title: String, val cat: String, val kind: String)
    var opening by mutableStateOf<Opening?>(null)
        private set
    var leaving by mutableStateOf(false)
        private set
    // The tab bar. A page (a thread, a bot) opens over the bar from the tab it
    // belongs to, and Back returns to that tab's list; each tab keeps its own
    // scroll position (Tabs.kt). A tap opens in the tab it is on, except from
    // Home, where a thread goes to Threads and a bot, room or routine to Bots
    var tab by mutableStateOf(Tab.Threads)
        private set
    private var started0 = false
    // the tab a tapped page was opened in, until its screen comes
    private var openedIn: Pair<String, Tab>? = null
    // the hub settings page's last rows, by hub, on disk beside the kept state
    val hubSettings = mutableStateMapOf<String, Settings>()
    private var cleared = false
    private var pushed = Long.MIN_VALUE
    private var tapAt = 0L
    private var firstMs = 0L
    val seenThreads = mutableStateMapOf<String, ThreadView>()
    val seenBots = mutableStateMapOf<String, BotView>()
    val seenPages = mutableStateMapOf<String, Screen>()
    private val threadOrder = java.util.ArrayDeque<String>()
    private val botOrder = java.util.ArrayDeque<String>()
    private val pageOrder = java.util.ArrayDeque<String>()

    companion object {
        fun nav(s: Screen): String = s.bot?.let { if (it.kind != "bot") "@${it.kind}:${it.id}" else s.sel } ?: s.sel
    }

    fun open(nav: String, action: String, value: String, title: String, cat: String = "", kind: String = "thread") {
        val now = android.os.SystemClock.elapsedRealtime()
        if (nav.isEmpty() || opening?.nav == nav || (!leaving && screen?.let { Core.nav(it) } == nav) ||
            (pushed != Long.MIN_VALUE && now - pushed < 350)) return
        pushed = now
        tapAt = now
        leaving = false
        val dest = destination(kind, action)
        openedIn = nav to dest
        tab = dest
        val rig = cat.ifEmpty { screen?.bots?.firstOrNull { it.nav == nav }?.cat ?: screen?.uber?.takeIf { it.nav == nav }?.cat ?: "" }
        opening = Opening(nav, title, rig, kind)
        firstMs = android.os.SystemClock.elapsedRealtime() - tapAt
        dispatch(action, value)
    }

    // where a tap on a row opens its page (see tab)
    private fun destination(kind: String, action: String): Tab {
        if (tab != Tab.Home) return tab
        return when (kind) {
            "thread" -> Tab.Threads
            "bot" -> if (action == "uber") Tab.Home else Tab.Bots
            "room", "remote", "routine" -> Tab.Bots
            else -> tab
        }
    }

    // where a page selected from elsewhere (a notification, a link, a link in
    // a thread) opens: a bot's thread, a room or a remote bot in Bots, the
    // uberbot's on Home, any other thread in Threads (Bend says which: the
    // screen's bot, and the uber card's sel)
    private fun destination(s: Screen, nav: String): Tab {
        openedIn?.let { if (it.first == nav) return it.second }
        return if (s.bot != null) (if (s.uber?.sel == true) Tab.Home else Tab.Bots) else Tab.Threads
    }

    fun selectTab(t: Tab) {
        tab = t
    }

    // the thread list's view, for Bend to draw (the device's own choice)
    private suspend fun languageLocal() {
        apply(engine.local("lang-preview", if (AppSettings.langPreview) "1" else ""))
        apply(engine.local("lang", AppSettings.lang))
        apply(engine.local("lang-os", android.os.LocaleList.getDefault()[0].toLanguageTag()))
    }

    fun language(v: String) {
        AppSettings.putLang(v)
        scope.launch { languageLocal() }
    }

    fun forceLanguage(v: String) {
        AppSettings.forceLang(v)
        scope.launch { languageLocal() }
    }

    fun threadList(v: String) {
        AppSettings.putThreadList(v)
        scope.launch { apply(engine.local("view", v)) }
    }

    // App settings, Cache: the kept state's size, and forgetting it (the next
    // launch fetches each hub's log again; nothing is kept again before)
    fun cacheSize(): Long = kept.size() + (if (kept.settingsFile.exists()) kept.settingsFile.length() else 0L)

    fun clearCache() {
        cleared = true
        kept.clear()
        kept.settingsFile.delete()
        hubSettings.clear()
    }

    private fun keepHubSettings() {
        if (cleared) return
        val o = JSONObject()
        for ((k, v) in hubSettings) o.put(k, settingsJson(v))
        val f = kept.settingsFile
        scope.launch(Dispatchers.IO) { runCatching { f.writeText(o.toString()) } }
    }

    private fun loadHubSettings() {
        val f = kept.settingsFile
        runCatching {
            val o = JSONObject(f.readText())
            for (k in o.keys()) hubSettings[k] = parseSettings(o.getJSONObject(k))
        }
    }

    fun back() {
        opening = null
        leaving = true
        tapAt = 0L
        act("select", "")
    }

    private fun <T> remember(nav: String, value: T, map: MutableMap<String, T>, order: java.util.ArrayDeque<String>) {
        map[nav] = value
        order.remove(nav)
        order.addLast(nav)
        if (order.size > 24) map.remove(order.removeFirst())
    }

    // the composer's text, owned here so typing never waits on Bend
    var composer by mutableStateOf("")
        private set
    var scrolls by mutableIntStateOf(0)
        private set
    // an entry of the open thread to show (a "jump"); n counts the jumps, so
    // the same entry shown twice scrolls twice
    var jump by mutableStateOf<Jump?>(null)
        private set
    // a jump waiting for a screen that holds its entry (the jump opens folds
    // and pages first), and how many more screens it waits
    private var jumping: String? = null
    private var jumpLeft = 0
    // the board viewer's plots, which come straight from the socket
    val plots = PlotStore()
    // drafts sent to Bend and not yet answered: until then a screen may
    // carry an older draft than the one on screen
    private var typing = 0
    // activities started: above zero, the app is in the foreground
    private var started = 0
    val foreground get() = started > 0

    init {
        AppSettings.init(app)
        loadHubSettings()
        engine.onFlush = { out -> scope.launch { apply(out) } }
        app.registerActivityLifecycleCallbacks(this)
        // a changed zone is reported at once (hubs keep it for routines and reminders)
        androidx.core.content.ContextCompat.registerReceiver(app, object : android.content.BroadcastReceiver() {
            override fun onReceive(c: Context, i: android.content.Intent) { act("tz-report", java.time.ZoneId.systemDefault().id) }
        }, android.content.IntentFilter(android.content.Intent.ACTION_TIMEZONE_CHANGED), androidx.core.content.ContextCompat.RECEIVER_NOT_EXPORTED)
        androidx.core.content.ContextCompat.registerReceiver(app, object : android.content.BroadcastReceiver() {
            override fun onReceive(c: Context, i: android.content.Intent) { scope.launch { languageLocal() } }
        }, android.content.IntentFilter(android.content.Intent.ACTION_LOCALE_CHANGED), androidx.core.content.ContextCompat.RECEIVER_NOT_EXPORTED)
        val t0 = android.os.SystemClock.elapsedRealtime()
        fun since() = android.os.SystemClock.elapsedRealtime() - t0
        scope.launch {
            apply(engine.screen())
            apply(engine.local("view", AppSettings.threadList))
            languageLocal()
            android.util.Log.i("Backplane", "engine ready ${since()} ms")
        }
        scope.launch {
            while (true) {
                delay(30_000)
                if (foreground) apply(engine.tick(System.currentTimeMillis() / 1000))
                keep()
            }
        }
        // the state kept at the last launch first: the screen shows at once,
        // and each hub then sends only what came since
        scope.launch {
            if (kept.has()) {
                android.util.Log.i("Backplane", "state ${kept.path.length()} bytes")
                apply(engine.loadFrom(kept.path))
                apply(engine.local("view", AppSettings.threadList))
                languageLocal()
                android.util.Log.i("Backplane", "state loaded ${since()} ms")
                for (l in links) Pairing.key(l)?.let { apply(engine.offline(it)) }
            }
            connect()
        }
        // the engine started over (Engine.onReset): every socket anew, from
        // the kept state as at launch
        engine.onReset = {
            scope.launch {
                for (h in hubs.values) h.stop()
                hubs.clear()
                if (kept.has()) {
                    apply(engine.loadFrom(kept.path))
                    for (l in links) Pairing.key(l)?.let { apply(engine.offline(it)) }
                }
                connect()
            }
        }
    }

    // the state written down when it changed (the app may be stopped at any
    // time once in the background)
    private fun keep() {
        if (!dirty || cleared) return
        dirty = false
        scope.launch {
            if (engine.saveTo(kept.tmp)) kept.commit()
        }
    }

    // this install's client id: 8 hex digits, made once
    private fun cid(): String =
        prefs.getString("cid", null) ?: "%08x".format(SecureRandom().nextInt()).also {
            prefs.edit().putString("cid", it).apply()
        }

    private fun loadLinks(): List<String> {
        prefs.getString("links", null)?.let { j ->
            val a = org.json.JSONArray(j)
            return (0 until a.length()).map { a.optString(it) }
        }
        return listOfNotNull(prefs.getString("link", null)?.takeIf { it.isNotBlank() })
    }

    private fun save() {
        prefs.edit().putString("links", org.json.JSONArray(links).toString()).remove("link").apply()
    }

    // a new hub, or a new link to one already paired (it replaces the old)
    fun pair(text: String) {
        val l = text.trim()
        val k = Pairing.key(l) ?: return
        links = links.filter { Pairing.key(it) != k } + l
        save()
        connect()
    }

    fun unpair(key: String) {
        links = links.filter { Pairing.key(it) != key }
        save()
        connect()
    }

    // one socket per paired hub: new hubs connect, unpaired ones hang up
    private fun connect() {
        val keyed = LinkedHashMap<String, String>()
        for (l in links) Pairing.key(l)?.let { k -> keyed.putIfAbsent(k, l) }
        for (k in hubs.keys.toList()) if (k !in keyed) hubs.remove(k)?.stop()
        scope.launch {
            apply(engine.hubs(keyed.keys.toList()))
            apply(engine.local("view", AppSettings.threadList))
            languageLocal()
            for ((k, l) in keyed) if (k !in hubs) open(k, l)
        }
    }

    private fun open(key: String, link: String) {
        val base = Pairing.socket(link) ?: return
        hubs[key] = Hub(scope,
            url = { Pairing.resume(base, engine.resume(key)) },
            onOpen = {
                if (shownHub == key) plots.reset()
                scope.launch { apply(engine.online(key, true)) }
                // where the person is: every hub keeps the phone's zone (routines follow it)
                act("tz-report", java.time.ZoneId.systemDefault().id)
            },
            // plots go straight to the viewer, never through the Bend
            // client, and only the hub in focus draws
            onMessage = { b ->
                if (PlotStore.isPlot(b)) { if (shownHub == key) plots.receive(b) }
                else scope.launch {
                    dirty = true
                    apply(engine.recv(key, Base64.encodeToString(b, Base64.NO_WRAP)))
                }
            },
            onClose = { scope.launch { apply(engine.online(key, false)) } },
        ).also { it.start() }
    }

    private fun dispatch(action: String, value: String) {
        scope.launch { apply(engine.act(action, value)) }
    }

    fun act(action: String, value: String = "") {
        // a page on its way takes no other action (a cached page is inert);
        // given up after 3 s, so a page that never comes never freezes the app
        if (opening != null) {
            if (android.os.SystemClock.elapsedRealtime() - pushed < 3000) return
            opening = null
            tapAt = 0L
        }
        // a web address a settings row offers (Antigravity's sign-in):
        // opened in the browser here, never sent to the engine
        if (action == "open-url") {
            val u = android.net.Uri.parse(value)
            if (u.scheme == "https" || u.scheme == "http")
                runCatching { app.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, u).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)) }
            return
        }
        dispatch(action, value)
    }

    // an action whose effect shows only with what the hub answers (a key
    // typed into the terminal, which the shell echoes): its commands go
    // out, and the screen waits for the answer, so fast typing never
    // queues a screen per key
    fun quiet(action: String, value: String) {
        scope.launch { apply(engine.quiet(action, value)) }
    }

    // the cats' rigs by key ("look:mood"), each asked of the bridge once
    val cats = mutableStateMapOf<String, List<CatPart>>()
    private val asked = mutableSetOf<String>()

    // catalog texts for native parts, by key (spec 4.1); read again when the
    // language changes, which shows in the screen's language row
    val texts = mutableStateMapOf<String, String>()
    private val wanted = mutableSetOf<String>()
    private var langLabel: String? = null

    // the text for a key; its English (fallback) until the bridge answers
    fun t(key: String, fallback: String): String = texts[key] ?: fallback

    fun want(key: String) {
        wanted.add(key)
        scope.launch { texts[key] = engine.t(key) }
    }

    private fun refreshTexts() {
        val keys = wanted.toList()
        scope.launch { for (k in keys) texts[k] = engine.t(k) }
    }

    fun cat(key: String) {
        if (key.isEmpty() || !asked.add(key)) return
        scope.launch {
            val text = engine.cat(key)
            cats[key] = try { catRig(org.json.JSONArray(text)) } catch (e: Exception) { emptyList() }
        }
    }

    // an alert; a browser task's pause carries its screenshot's hub path,
    // fetched with the pairing token for the banner (none if it cannot be)
    private fun notify(c: Cmd) {
        val url = if (c.image.isEmpty()) null else web(c.image)
        if (url == null) { Notes.turn(app, c, null); return }
        scope.launch {
            val pic = withContext(kotlinx.coroutines.Dispatchers.IO) {
                runCatching {
                    val h = java.net.URL(url).openConnection() as java.net.HttpURLConnection
                    h.connectTimeout = 5000
                    h.readTimeout = 10000
                    try {
                        if (h.responseCode == 200) h.inputStream.use { android.graphics.BitmapFactory.decodeStream(it) } else null
                    } finally {
                        h.disconnect()
                    }
                }.getOrNull()
            }
            Notes.turn(app, c, pic)
        }
    }

    // a bot form's field ("bfield"), typed: Bend keeps it without a new
    // screen, the field on screen already shows it
    fun field(name: String, text: String) {
        scope.launch { apply(engine.quiet("bfield", name + "\u001f" + text)) }
    }

    // an address on the hub in focus (a bot's browser frame, a webhook)
    fun hubUrl(path: String, query: String = "", token: Boolean = true): String? {
        val s = screen ?: return null
        val l = links.firstOrNull { Pairing.key(it) == s.hub } ?: return null
        return Pairing.http(l, path, query, token)
    }

    // where the hub in focus serves a path ("/img?path=…"), with its token
    fun web(path: String): String? {
        val s = screen ?: return null
        val l = links.firstOrNull { Pairing.key(it) == s.hub } ?: links.firstOrNull() ?: return null
        val i = path.indexOf('?')
        return if (i < 0) Pairing.http(l, path) else Pairing.http(l, path.substring(0, i), path.substring(i + 1))
    }

    // a file for the next message: one POST /attach to the thread's hub with
    // the file as the body (no base64, no pieces through the engine); the
    // hub's answer goes back as "attach-end", as an attach.put's would
    fun attach(data: ByteArray, name: String) {
        if (data.isEmpty()) return
        val s = screen ?: return
        // the hub's own id: over several hubs the screen names it "<hub>|<id>"
        val thread = s.thread?.id?.substringAfterLast('|') ?: return
        val l = links.firstOrNull { Pairing.key(it) == s.hub } ?: links.firstOrNull() ?: return
        val key = "%08x".format(SecureRandom().nextInt())
        fun q(v: String) = java.net.URLEncoder.encode(v, "UTF-8").replace("+", "%20")
        val url = Pairing.http(l, "/attach", "thread=${q(thread)}&key=$key&name=${q(name)}") ?: return
        // the engine decides: one over the cap it refuses, and nothing goes
        act("attach-up", JSONObject().put("name", name).put("size", data.size).toString())
        if (data.size > (s.thread?.cap ?: 10_485_760)) return
        scope.launch {
            val answer = withContext(Dispatchers.IO) {
                try {
                    val c = java.net.URL(url).openConnection() as java.net.HttpURLConnection
                    c.requestMethod = "POST"
                    c.doOutput = true
                    c.connectTimeout = 15_000
                    c.readTimeout = 600_000
                    c.setFixedLengthStreamingMode(data.size)
                    c.setRequestProperty("Content-Type", "application/octet-stream")
                    c.outputStream.use { it.write(data) }
                    val code = c.responseCode
                    val body = (if (code < 400) c.inputStream else c.errorStream)?.bufferedReader()?.use { it.readText() } ?: ""
                    c.disconnect()
                    body
                } catch (e: Exception) {
                    ""
                }
            }
            val ok = try { JSONObject(answer); true } catch (e: Exception) { false }
            act("attach-end", if (ok) answer else JSONObject().put("text", "the upload did not reach the hub").put("thread", thread).put("upload", true).toString())
        }
    }

    fun draft(text: String) {
        composer = text
        typing += 1
        scope.launch {
            val out = engine.quiet("draft", text)
            typing -= 1
            apply(out)
        }
    }

    private fun apply(out: Reply) {
        out.screen?.let { next ->
            if (typing == 0) composer = next.thread?.draft ?: ""
            val was = screen?.sel
            val wasNav = screen?.let { nav(it) } ?: ""
            screen = next
            val label = next.lang?.rows?.firstOrNull()?.label
            if (label != langLabel) { langLabel = label; refreshTexts() }
            val nav = nav(next)
            if (!started0) {
                started0 = true
                AppSettings.migrate(next.theme, next.view)
                tab = Tab.entries.firstOrNull { it.name == AppSettings.defaultTab && it != Tab.Settings } ?: if (next.uber == null) Tab.Threads else Tab.Home
                scope.launch { apply(engine.local("view", AppSettings.threadList)); languageLocal() }
            }
            next.settings?.let { st -> if (next.hub.isNotEmpty() && hubSettings[next.hub] != st) { hubSettings[next.hub] = st; keepHubSettings() } }
            if (nav.isNotEmpty() && nav != wasNav) { tab = destination(next, nav); if (openedIn?.first == nav) openedIn = null }
            if (nav.isNotEmpty()) {
                next.thread?.let { remember(nav, it, seenThreads, threadOrder) }
                next.bot?.let { remember(nav, it, seenBots, botOrder) }
                remember(nav, next.copy(projects = emptyList(), bots = emptyList(), rooms = emptyList()), seenPages, pageOrder)
            }
            if (opening?.nav == nav) {
                android.util.Log.i("Backplane", "tap $nav first $firstMs ms full ${android.os.SystemClock.elapsedRealtime() - tapAt} ms")
                opening = null
                tapAt = 0L
            }
            if (leaving && nav.isEmpty()) leaving = false
            if (next.uber == null) { lastUber = null; lastPlane = null }
            else if (next.plane != null) { lastUber = next.uber; lastPlane = next.plane }
            if (next.hub != shownHub) {
                shownHub = next.hub
                plots.reset()
            }
            if (foreground && next.sel.isNotEmpty() && next.sel != was) Notes.clear(app, next.sel)
            LiveService.sync(app, next.island, foreground)
        }
        for (c in out.cmds) when (c.type) {
            "send" -> hubs[c.hub]?.send(Base64.decode(c.data, Base64.DEFAULT))
            "copy" -> {
                val cm = app.getSystemService(ClipboardManager::class.java)
                cm.setPrimaryClip(ClipData.newPlainText("Backplane", c.text))
            }
            "scroll" -> scrolls += 1
            // an action come due (the design history playing), sent as a tap would
            "later" -> if (c.action.isNotEmpty()) {
                val action = c.action
                val value = c.value
                scope.launch {
                    delay(maxOf(0L, c.ms))
                    act(action, value)
                }
            }
            "jump" -> if (c.id.isNotEmpty()) { jumping = c.id; jumpLeft = 4 }
            "keep" -> keep(c.thread, c.text)
            "notify" -> if (!(foreground && screen?.sel == c.thread)) notify(c)
        }
        jumping?.let { id ->
            val t = screen?.thread ?: return@let
            val e = t.entries.firstOrNull { it.id == id || it.id.endsWith("|$id") }
            if (e != null) {
                jumping = null
                jump = Jump(e.id, (jump?.n ?: 0) + 1)
            } else if (out.screen != null) {
                jumpLeft -= 1
                if (jumpLeft <= 0) jumping = null
            }
        }
    }

    data class Jump(val id: String, val n: Int)

    // a thread's draft, written at once (a crash loses nothing typed); "" forgets it
    private fun keep(thread: String, text: String) {
        val o = try { JSONObject(prefs.getString("drafts", "{}") ?: "{}") } catch (e: Exception) { JSONObject() }
        if (text.isEmpty()) o.remove(thread) else o.put(thread, text)
        prefs.edit().putString("drafts", o.toString()).commit()
    }

    override fun onActivityStarted(activity: Activity) {
        started += 1
        screen?.sel?.let { if (it.isNotEmpty()) Notes.clear(app, it) }
    }

    // being in front is when the live service may start
    override fun onActivityResumed(activity: Activity) {
        screen?.let { LiveService.sync(app, it.island, true) }
    }

    override fun onActivityStopped(activity: Activity) {
        started = maxOf(0, started - 1)
        if (started == 0) keep()
    }

    override fun onActivityCreated(activity: Activity, state: Bundle?) {}
    override fun onActivityPaused(activity: Activity) {}
    override fun onActivitySaveInstanceState(activity: Activity, state: Bundle) {}
    override fun onActivityDestroyed(activity: Activity) {}
}

class BackplaneApp : Application() {
    val core by lazy { Core(this) }

    override fun onCreate() {
        super.onCreate()
        Notes.channels(this)
        core
    }
}

val Context.core: Core get() = (applicationContext as BackplaneApp).core
