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
    private val bridge by lazy { app.assets.open("bridge.js").bufferedReader().readText() }
    private val engine by lazy { Engine(app, { bridge }, cid(), prefs.getString("drafts", "{}") ?: "{}", app.filesDir) }
    private val hubs = mutableMapOf<String, Hub>()
    // the client's state, kept for the next launch (StateStore)
    private val kept by lazy { StateStore(app.filesDir, bridge) }
    private val recentScreen = ScreenCache(java.io.File(app.filesDir, "recent-screen-v1.json")) { android.util.Log.i("Backplane", "screen-cache $it") }
    private var cacheGeneration = 0L
    private var cacheJob: kotlinx.coroutines.Job? = null
    private var lastScreenJSON: String? = null
    private val presentationStore = PresentationStore(java.io.File(app.filesDir, "view-choices-v1.json"))
    private var presentationGeneration = 0L
    private var pendingNav: String? = null
    val startup = mutableStateMapOf<String, StartupPolicy>()
    // whether each hub's socket is up, as the native side knows it (a thin
    // screen keeps the hub's last word on that through an outage)
    val linkUp = mutableStateMapOf<String, Boolean>()
    // when the app last left the front (its sockets may have died since)
    private var awayAt = 0L
    private val thinReady = mutableStateMapOf<String, Pair<Long, String>>()
    private val thinRequests = mutableMapOf<String, ThinPresentationFence>()
    private val legacyReady = mutableStateMapOf<String, Long>()
    private var cachedPresentation = false
    private var engineReady = false
    private val epochs = mutableMapOf<String, Long>()
    private val connected = mutableSetOf<String>()
    private val presentationEpochs = mutableMapOf<String, Long>()
    private val requestedFrameAt = mutableMapOf<String, Long>()
    private val restoredAt = mutableMapOf<String, Long>()
    private var requestDeadline: Long? = null
    private var deadlineJob: kotlinx.coroutines.Job? = null
    private val earlyActions = mutableListOf<Pair<String, String>>()
    private val localDrafts = mutableMapOf<String, String>()
    val contentReady get(): Boolean {
        val current = screen ?: return false
        if (!engineReady || cachedPresentation) return false
        if (startup[current.hub] == StartupPolicy.COMPATIBILITY_CHOSEN) return current.hub in connected && legacyReady[current.hub] == epochs[current.hub]
        val mark = thinReady[current.hub] ?: return false
        return current.hub in connected && mark.first == epochs[current.hub] &&
            mark.second == (pendingNav ?: opening?.nav ?: nav(current))
    }
    // the page on show, presented, whose own action or update is on its way
    // (its ready mark is down until the hub answers): what the person types
    // or taps there still goes, and waits its turn in the bridge. Dropped, a
    // field typed faster than the hub answered kept only its first letter.
    private val settling = mutableMapOf<String, Pair<Long, String>>()
    private val acting get(): Boolean {
        if (contentReady) return true
        val current = screen ?: return false
        if (!engineReady || cachedPresentation || pendingNav != null || opening != null) return false
        if (startup[current.hub] != StartupPolicy.THIN || current.hub !in connected) return false
        val mark = settling[current.hub] ?: return false
        return mark.first == epochs[current.hub] && mark.second == nav(current)
    }
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
        pendingNav = nav
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
        apply(engine.local("mailroom-renderer", "android-v1"))
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
    fun cacheSize(): Long = recentScreen.size() + kept.size() + (if (settingsFile.exists()) settingsFile.length() else 0L)

    fun clearCache() {
        cleared = true
        retireScreenCache()
        presentationGeneration += 1
        presentationStore.clear(presentationGeneration)
        kept.clear()
        settingsFile.delete()
        hubSettings.clear()
    }

    private fun keepHubSettings() {
        if (cleared) return
        val o = JSONObject()
        for ((k, v) in hubSettings) o.put(k, settingsJson(v))
        val f = settingsFile
        scope.launch(Dispatchers.IO) { runCatching { f.writeText(o.toString()) } }
    }

    private val settingsFile get() = java.io.File(app.filesDir, "hub-settings.json")

    private fun loadHubSettings() {
        val f = settingsFile
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
        // a network that comes up (Wi-Fi back, a switch to cellular): sockets
        // are tried now, and one that went quiet on the old network replaced
        runCatching {
            val cm = app.getSystemService(android.net.ConnectivityManager::class.java)
            cm?.registerDefaultNetworkCallback(object : android.net.ConnectivityManager.NetworkCallback() {
                private var last: android.net.Network? = null
                override fun onAvailable(network: android.net.Network) {
                    val changed = last != null && last != network
                    last = network
                    android.os.Handler(android.os.Looper.getMainLooper()).post {
                        for (h in hubs.values) h.wake(if (changed) 3000 else 40_000)
                    }
                }
            })
        }
        val cached = recentScreen.load(links, cid())
        if (cached != null) runCatching { parseScreen(cached) }.onSuccess { saved ->
            screen = saved
            cachedPresentation = true
            composer = runCatching { JSONObject(prefs.getString("drafts", "{}") ?: "{}").optString(saved.sel) }.getOrDefault("")
            lastUber = saved.uber
            lastPlane = saved.plane
            val nav = nav(saved)
            if (nav.isNotEmpty()) {
                tab = destination(saved, nav)
                saved.thread?.let { remember(nav, it, seenThreads, threadOrder) }
                saved.bot?.let { remember(nav, it, seenBots, botOrder) }
                remember(nav, saved, seenPages, pageOrder)
            }
        }.onFailure { android.util.Log.i("Backplane", "screen-cache miss_native_decode") }
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
            loadHubSettings()
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
        scope.launch {
            val paired = links.toList()
            withContext(Dispatchers.IO) { presentationStore.load(paired, cid()) }?.takeIf { paired == links }?.let {
                apply(engine.loadPresentation(it))
            }
            engineReady = true
            connect()
        }
        engine.onReset = {
            scope.launch {
                for (h in hubs.values) h.stop()
                hubs.clear()
                connected.clear()
                thinReady.clear()
                presentationEpochs.clear()
                val paired = links.toList()
                withContext(Dispatchers.IO) { presentationStore.load(paired, cid()) }?.takeIf { paired == links }?.let { apply(engine.loadPresentation(it)) }
                connect()
            }
        }
    }

    // the state written down when it changed (the app may be stopped at any
    // time once in the background)
    private fun keep() {
        lastScreenJSON?.takeIf { !cleared }?.let { text ->
            val generation = cacheGeneration
            val paired = links.toList()
            val profile = cid()
            scope.launch(Dispatchers.IO) { recentScreen.save(text, paired, profile, generation) }
        }
        if (!dirty || cleared || !engineReady) return
        dirty = false
        scope.launch {
            presentationGeneration += 1
            val generation = presentationGeneration
            val paired = links.toList()
            val text = engine.savePresentation()
            if (generation == presentationGeneration && paired == links)
                withContext(Dispatchers.IO) { presentationStore.save(text, paired, cid(), generation) }
        }
    }

    private fun retireScreenCache() {
        cacheJob?.cancel()
        lastScreenJSON = null
        cacheGeneration += 1
        recentScreen.clear(cacheGeneration)
        presentationGeneration += 1
        presentationStore.clear(presentationGeneration)
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
        retireScreenCache()
        hubs.remove(k)?.stop()
        epochs[k] = (epochs[k] ?: 0L) + 1
        connected.remove(k)
        presentationEpochs.remove(k)
        thinReady.remove(k)
        legacyReady.remove(k)
        startup[k] = StartupPolicy.DISCOVERING
        links = links.filter { Pairing.key(it) != k } + l
        save()
        connect()
    }

    fun unpair(key: String) {
        retireScreenCache()
        startup.remove(key)
        thinReady.remove(key); settling.remove(key)
        legacyReady.remove(key)
        epochs[key] = (epochs[key] ?: 0L) + 1
        connected.remove(key)
        presentationEpochs.remove(key)
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

    fun retryView(key: String) {
        if (links.none { Pairing.key(it) == key } || startup[key] !in setOf(StartupPolicy.COMPATIBILITY_REQUIRED, StartupPolicy.PAIRING_REFUSED)) return
        hubs.remove(key)?.stop()
        thinReady.remove(key); settling.remove(key)
        presentationEpochs.remove(key)
        startup[key] = StartupPolicy.DISCOVERING
        connect()
    }

    private fun open(key: String, link: String) {
        val base = Pairing.socket(link) ?: return
        hubs[key] = Hub(scope,
            url = {
                val epoch = (epochs[key] ?: 0L) + 1
                epochs[key] = epoch
                connected.remove(key)
                presentationEpochs.remove(key)
                requestedFrameAt.remove(key)
                thinReady.remove(key); settling.remove(key)
                legacyReady.remove(key)
                run {
                    startup[key] = StartupPolicy.DISCOVERING
                    val found = ViewCapability.find(link)
                    if (link !in links || epochs[key] != epoch) Next.Stop
                    // a hub that is down or restarting is asked again after
                    // the backoff; only a hub that answers without views parks
                    else if (found == ViewCapability.Found.UNREACHABLE) Next.Later
                    else if (found == ViewCapability.Found.REFUSED) {
                        startup[key] = StartupPolicy.PAIRING_REFUSED
                        android.util.Log.i("Backplane", "view pairing_refused")
                        Next.Stop
                    }
                    else if (found != ViewCapability.Found.SUPPORTED) {
                        startup[key] = StartupPolicy.COMPATIBILITY_REQUIRED
                        android.util.Log.i("Backplane", "view capability_required")
                        Next.Stop
                    } else {
                        startup[key] = StartupPolicy.THIN
                        apply(engine.connection(key, epoch.toString(), "view-v1"))
                        val pending = earlyActions.lastOrNull { it.second.startsWith("$key|") || (it.second.isEmpty() && screen?.hub == key) }
                        if (pending != null) {
                            earlyActions.remove(pending)
                            apply(engine.act(pending.first, pending.second))
                        } else if (screen?.hub == key) apply(engine.act("select", screen?.sel.orEmpty()))
                        Pairing.viewSocket(link)?.let { Next.Go(it) } ?: Next.Stop
                    }
                }
            },
            onOpen = {
                if (shownHub == key) plots.reset()
                connected.add(key)
                linkUp[key] = true
                val epoch = epochs[key] ?: 0L
                scope.launch { apply(engine.online(key, true, epoch.toString())) }
                // where the person is: every hub keeps the phone's zone (routines follow it)
                act("tz-report", java.time.ZoneId.systemDefault().id)
            },
            // plots go straight to the viewer, never through the Bend
            // client, and only the hub in focus draws
            onMessage = { b ->
                if (PlotStore.isPlot(b)) { if (shownHub == key) plots.receive(b) }
                else {
                    val epoch = epochs[key] ?: 0L
                    scope.launch {
                        dirty = true
                        val out = engine.recv(key, Base64.encodeToString(b, Base64.NO_WRAP), epoch.toString())
                        if (epochs[key] != epoch || key !in connected || links.none { Pairing.key(it) == key }) return@launch
                        for (update in out.sync.filter { it.hub == key }) {
                            val thin = startup[key] == StartupPolicy.THIN
                            observeThinPresenting(update, epoch)
                            val presented = thin && currentThinReply(update, epoch)
                            if (!thin && update.phase == "live") legacyReady[key] = epoch
                            if (presented) {
                                val wanted = pendingNav ?: opening?.nav ?: earlyActions.lastOrNull { it.first == "select" }?.second ?: (if (cachedPresentation) screen?.let { nav(it) } else null) ?: out.screen?.let { nav(it) } ?: screen?.let { nav(it) }.orEmpty()
                                thinReady[key] = epoch to wanted
                            }
                            if (presented || (!thin && (update.phase == "presented" || update.phase == "live"))) presentationEpochs[key] = epoch
                            if (update.phase == "recent" || update.phase == "failed") presentationEpochs.remove(key)
                        }
                        apply(out)
                        if (startup[key] == StartupPolicy.COMPATIBILITY_CHOSEN && cachedPresentation && screen?.hub == key && restoredAt[key] != epoch &&
                            out.sync.any { it.hub == key && (it.phase == "recent" || it.phase == "live") }) {
                            restoredAt[key] = epoch
                            val selection = earlyActions.lastOrNull { it.first == "select" }?.second ?: screen?.sel.orEmpty()
                            val selected = engine.act("select", selection)
                            if (epochs[key] == epoch && key in connected) apply(selected)
                        }
                        if (cachedPresentation && presentationEpochs[key] == epoch && out.screen == null && requestedFrameAt[key] != epoch) {
                            requestedFrameAt[key] = epoch
                            val current = engine.screen()
                            if (epochs[key] == epoch && key in connected) apply(current)
                        }
                    }
                }
            },
            onClose = {
                connected.remove(key)
                linkUp[key] = false
                legacyReady.remove(key)
                thinReady.remove(key); settling.remove(key)
                presentationEpochs.remove(key)
                val epoch = epochs[key] ?: 0L
                scope.launch { apply(engine.online(key, false, epoch.toString())) }
            },
        ).also { it.start() }
    }

    private fun dispatch(action: String, value: String) {
        if (action == "select" && (value.isEmpty() || links.any { value.startsWith((Pairing.key(it) ?: "") + "|") })) pendingNav = value
        if (action == "select" || action in setOf("bot", "room", "remote", "uber", "uber-bot")) retireScreenCache()
        // (an action on the page on show still goes while its own last one is
        // answered: acting)
        if (!contentReady && (action in setOf("select", "bot", "room", "remote", "uber", "uber-bot") || !acting)) {
            if (action in setOf("select", "bot", "room", "remote", "uber", "uber-bot")) {
                earlyActions.removeAll { it.first == action }
                val targetHub = links.mapNotNull { Pairing.key(it) }.firstOrNull { value.startsWith("$it|") } ?: screen?.hub
                val configured = startup[targetHub] in setOf(StartupPolicy.THIN, StartupPolicy.COMPATIBILITY_CHOSEN)
                if (engineReady && configured) scope.launch {
                    val out = engine.act(action, value)
                    apply(out)
                    dirty = true
                    keep()
                } else earlyActions.add(action to value)
            }
            return
        }
        val navigation = action == "select" || action in setOf("bot", "room", "remote", "uber", "uber-bot")
        scope.launch {
            val out = engine.act(action, value)
            apply(out)
            if (navigation) { dirty = true; keep() }
        }
    }

    fun moreMessage(hub: String, thread: String, entry: String, partial: MessagePartial, live: Boolean = false) {
        if (!contentReady || startup[hub] != StartupPolicy.THIN || !partial.canRequest || entry.isEmpty()) return
        val current = screen ?: return
        val shown = current.thread ?: return
        if (current.hub != hub || shown.id != thread) return
        val actual = if (live) shown.livePartial else shown.entries.firstOrNull { it.id == entry }?.partial
        if (actual != partial || (live && actual?.id != entry)) return
        act("thin-message-more", JSONObject().put("entryId", entry).put("cursor", partial.cursor).toString())
    }

    // what was typed in the secret box's value field: held here only until
    // Save, then handed to the engine once (never a form field, never kept)
    private var vaultText = ""

    fun vault(text: String) { vaultText = text }

    fun act(action: String, value: String = "") {
        // the secret box's Save: the value joins the box's name, purpose and
        // thread and leaves in one secret.set request (Engine.secret)
        if (action == "secret-save") {
            val v = vaultText
            vaultText = ""
            if (!acting) return
            scope.launch { apply(engine.secret(value + "\u001f" + v)) }
            return
        }
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
        if (!acting) return
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
        if (!acting) return
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
        if (!contentReady) return
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
        if (!contentReady) {
            val sel = screen?.sel.orEmpty()
            if (sel.isNotEmpty()) { localDrafts[sel] = text; keep(sel, text) }
            return
        }
        typing += 1
        scope.launch {
            val out = engine.quiet("draft", text)
            typing -= 1
            apply(out)
        }
    }

    private fun observeThinPresenting(update: SyncUpdate, epoch: Long) {
        if (startup[update.hub] != StartupPolicy.THIN || !update.thinPresenting || epochs[update.hub] != epoch) return
        if (thinRequests.getOrPut(update.hub) { ThinPresentationFence() }.observe(update, epoch)) {
            thinReady.remove(update.hub)?.let { if (it.first == epoch) settling[update.hub] = it }
            presentationEpochs.remove(update.hub)
            requestedFrameAt.remove(update.hub)
        }
    }

    private fun currentThinReply(update: SyncUpdate, epoch: Long) = update.thinPresented &&
        thinRequests[update.hub]?.accepts(epoch, update.requestRevision) == true

    private fun apply(out: Reply) {
        for (update in out.sync) observeThinPresenting(update, epochs[update.hub] ?: 0L)
        // Expiry reaches apply through pulse, outside a received frame. It
        // can retire a marker but cannot establish a new connection's one.
        for (update in out.sync) if (update.phase == "recent" || update.phase == "failed") {
            presentationEpochs.remove(update.hub)
            requestedFrameAt.remove(update.hub)
            thinReady.remove(update.hub)
            settling.remove(update.hub)
            thinRequests[update.hub]?.invalidate()
            legacyReady.remove(update.hub)
        }
        if (out.deadline != requestDeadline) {
            requestDeadline = out.deadline
            deadlineJob?.cancel()
            out.deadline?.let { deadline -> deadlineJob = scope.launch {
                delay(maxOf(0L, deadline - System.currentTimeMillis()) + 10)
                if (requestDeadline == deadline) apply(engine.pulse())
            } }
        }
        val expected = pendingNav ?: opening?.nav ?: earlyActions.lastOrNull { it.first == "select" }?.second ?: screen?.let { nav(it) }.orEmpty()
        val fresh = out.screen?.let { next ->
            (next.hub == screen?.hub || opening != null || earlyActions.any { it.first == "select" }) &&
                nav(next) == expected && next.hub in connected &&
                presentationEpochs[next.hub] == epochs[next.hub]
        } == true
        out.screen?.takeIf { next ->
            val thinFresh = startup[next.hub] != StartupPolicy.THIN || thinReady[next.hub]?.let {
                // the screen the presented view was for; or, with no navigation of the
                // person's own pending, where the hub went by itself (a new thread
                // or fork opens once thread.create is answered, often in a later
                // frame than the one that said "presented": it was dropped, and
                // "New thread" waited for good)
                it.first == epochs[next.hub] && (it.second == nav(next) || (pendingNav == null && opening == null)) &&
                    (pendingNav == null || pendingNav == nav(next)) && (opening == null || opening?.nav == nav(next)) &&
                    out.sync.none { update -> update.hub == next.hub && update.phase in setOf("presented", "presenting") && !currentThinReply(update, it.first) }
            } == true
            (next.hub.isEmpty() || links.any { link -> Pairing.key(link) == next.hub }) && (!cachedPresentation || fresh) && thinFresh
        }?.let { next ->
            cachedPresentation = false
            // the screen went where the hub went: the ready mark follows it
            // (else contentReady stayed false and every action was dropped)
            if (startup[next.hub] == StartupPolicy.THIN && pendingNav == null && opening == null)
                thinReady[next.hub]?.let { if (it.first == epochs[next.hub] && it.second != nav(next)) thinReady[next.hub] = it.first to nav(next) }
            if (pendingNav == nav(next)) pendingNav = null
            out.screenJSON?.takeIf { !cleared && (opening == null || opening?.nav == nav(next)) }?.let { raw ->
                lastScreenJSON = raw
                cacheGeneration += 1
                val generation = cacheGeneration
                val text = lastScreenJSON!!
                val paired = links.toList()
                val profile = cid()
                cacheJob?.cancel()
                cacheJob = scope.launch {
                    delay(300)
                    dirty = true
                    keep()
                    withContext(Dispatchers.IO) { recentScreen.save(text, paired, profile, generation) }
                }
            }
            if (typing == 0) composer = localDrafts[next.sel] ?: next.thread?.draft ?: ""
            val was = screen?.sel
            val wasNav = screen?.let { nav(it) } ?: ""
            // Channel IDs and preferences persist; only their displayed names follow locale.
            if (screen?.locale != next.locale) Notes.channels(app, next.locale)
            screen = next
            val label = next.lang?.rows?.firstOrNull()?.label
            if (label != langLabel) { langLabel = label; refreshTexts() }
            val nav = nav(next)
            if (!started0) {
                started0 = true
                AppSettings.migrate(next.theme, next.view)
                if (nav.isEmpty()) tab = Tab.entries.firstOrNull { it.name == AppSettings.defaultTab && it != Tab.Settings } ?: if (next.uber == null) Tab.Threads else Tab.Home
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
            if (contentReady && foreground && next.sel.isNotEmpty() && next.sel != was) Notes.clear(app, next.sel)
            if (contentReady) LiveService.sync(app, next.island, foreground)
        }
        if (contentReady) screen?.sel?.let { sel -> localDrafts.remove(sel)?.let { text ->
            scope.launch { apply(engine.quiet("draft", text)) }
        } }
        if (contentReady && out.screen != null && earlyActions.isNotEmpty()) {
            val actions = earlyActions.toList()
            earlyActions.clear()
            for ((action, value) in actions) dispatch(action, value)
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
        if (contentReady) screen?.sel?.let { if (it.isNotEmpty()) Notes.clear(app, it) }
    }

    // being in front is when the live service may start
    override fun onActivityResumed(activity: Activity) {
        if (contentReady) screen?.let { LiveService.sync(app, it.island, true) }
        // back in front: a socket that may have died while away is replaced
        // at once, and a hub waiting out its backoff is tried now
        val away = if (awayAt > 0) android.os.SystemClock.elapsedRealtime() - awayAt else 0L
        awayAt = 0L
        for (h in hubs.values) h.wake(if (away > 20_000) 3000 else 40_000)
    }

    override fun onActivityStopped(activity: Activity) {
        started = maxOf(0, started - 1)
        if (started == 0) { keep(); awayAt = android.os.SystemClock.elapsedRealtime() }
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
