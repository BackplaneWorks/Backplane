package dev.backplane.mobile

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.runtime.setValue
import kotlinx.coroutines.*
import okhttp3.*
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit

// The phone's whole link to a hub, docs/PHONE-DATA.md (data/1): one WebSocket of plain JSON, typed values, a disk cache that paints at launch,
// deltas for the lists and the open thread only. No engine, no bridge, no server-drawn views. (Same protocol as iOS's DataClient.swift.)
object DataClient {
    enum class Conn { OFFLINE, CONNECTING, ONLINE }
    private lateinit var app: Context
    internal val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val http = OkHttpClient.Builder().pingInterval(0, TimeUnit.SECONDS).readTimeout(0, TimeUnit.SECONDS).build()

    var conn by mutableStateOf(Conn.OFFLINE); private set
    var failure by mutableStateOf(""); private set
    var needsRepair by mutableStateOf(false); private set
    var projects by mutableStateOf<List<DProject>>(emptyList()); private set
    var threads by mutableStateOf<List<DThread>>(emptyList()); private set
    var entries by mutableStateOf<Map<String, List<DEntry>>>(emptyMap()); private set
    var older by mutableStateOf<Map<String, String>>(emptyMap()); private set
    var listsLoaded by mutableStateOf(false); private set
    var features by mutableStateOf<Set<String>>(emptySet()); private set
    var notice by mutableStateOf(HubNotice()); private set   // hello `notice`, newer from /hub/status: the banner
    var star by mutableStateOf<DStarOffer?>(null); private set
    var migrating by mutableStateOf<DMigrateCard?>(null); private set   // a 0.14 home coming across: hello `migrating`, then each `migrating` frame (DataMigrate.kt)
    var migrateOpen by mutableStateOf(false)   // the GitHub star suggestion (feature "star"): shown while the hub says so
    // the composer's own commands (hello.commands): the hub's table; none of them is ever a message
    data class Word(val word: String, val label: String, val desc: String)
    var commands by mutableStateOf<List<Word>>(emptyList()); private set
    data class Btw(val q: String, val a: String)
    var btw by mutableStateOf<Map<String, Btw>>(emptyMap()); private set
    data class SecretAsk(val name: String, val purpose: String)
    var secretAsks by mutableStateOf<Map<String, List<SecretAsk>>>(emptyMap()); private set
    /** every hub this phone is paired with (HubList); the first is the one in use */
    var links by mutableStateOf<List<String>>(emptyList()); private set
    val link: String? get() = links.firstOrNull()
    /** the pairing screen is open over a paired hub, to add another (Settings → Add a hub) */
    var adding by mutableStateOf(false)
    var pairingError by mutableStateOf(""); var pairingBusy by mutableStateOf(false)
    /** a pairing link that came from outside the app: nothing happens until the person says so */
    var askLink by mutableStateOf<String?>(null)
    val hubName get() = link?.let { Pairing.key(it) } ?: ""
    /** The hub's machine name from its hello, for titles ("" until heard, or from an older hub); `hubName` stays the key. */
    var hubTitle by mutableStateOf(""); private set

    private var ws: WebSocket? = null
    private var gen = 0
    private var nextId = 1
    private val calls = ConcurrentHashMap<Int, CompletableDeferred<JSONObject>>()
    private var backoff = 250L
    private var listSub = 0; private var threadSub = 0
    private var open: String? = null
    private var heard = 0L
    private var watchdog: Job? = null
    private var reconnect: Job? = null
    private var saveJob: Job? = null

    fun init(ctx: Context) {
        if (this::app.isInitialized) return
        app = ctx.applicationContext
        MeshDisk.dir = java.io.File(app.cacheDir, "models")
        links = HubList.decode(prefs().getString("links", null), prefs().getString("link", null))
        settledOpen = prefs().getStringSet("settled.open", emptySet())!!.toSet(); historyOpen = prefs().getBoolean("history.open", false)
        System.getenv("BACKPLANE_LINK")?.let { links = it.split(' ').filter(String::isNotBlank) }
        loadCache()
        if (link != null) connect()
        // while threads work the process stays (LiveService): their alerts reach this phone with the app off screen
        scope.launch { androidx.compose.runtime.snapshotFlow { threads.filter { it.state == "working" && !it.archived && !it.history }.map { it.title } to foreground }.collect { (w, front) -> LiveService.sync(app, w, front) } }
    }
    private fun prefs() = app.getSharedPreferences("data", Context.MODE_PRIVATE)

    // ---- pairing
    fun pair(text: String) {
        val inv = Pairing.invitation(text)
        if (inv != null) {
            if (pairingBusy) return
            pairingBusy = true; pairingError = "Connecting…"
            scope.launch {
                try {
                    val tries = inv.routes.ifEmpty { listOf(inv.origin) }
                    if (HubList.local(tries)) { pairingError = HubList.LOCAL; pairingBusy = false; return@launch }
                    val l = Pairing.exchange(inv, tries); pairingError = ""; adopt(l)
                }
                catch (e: Exception) { pairingError = e.message ?: "Hub unreachable" }
                pairingBusy = false
            }
            return
        }
        if (Pairing.bareCode(text)) { pairingError = "Enter the hub's address with the short code."; return }
        if (Pairing.malformedInvitation(text)) { pairingError = "Enter the complete invitation link or a valid eight-character code."; return }
        adopt(text.trim())
    }
    private fun save(next: List<String>) { links = next; prefs().edit().putString("links", HubList.encode(next)).remove("link").apply() }
    private fun adopt(l: String) {
        if (Pairing.key(l) == null) { pairingError = "Enter the hub's address or pairing link."; return }
        val changed = Pairing.key(link ?: "") != Pairing.key(l)
        save(HubList.adopt(links, l) { Pairing.key(it) }); needsRepair = false; adding = false
        if (changed) reset()
        connect()
    }
    /** Switch to another paired hub: it goes first, this phone's copy of the last one's threads is dropped, and the socket moves. */
    fun useHub(key: String) {
        if (Pairing.key(link ?: "") == key) return
        val next = HubList.use(links, key) { Pairing.key(it) }; if (next == links) return
        close(); save(next); reset(); needsRepair = false; pairingError = ""; failure = ""; connect()
    }
    /** Forget one hub; when it was the one in use, the next paired hub is used (none left: the pairing screen). */
    fun forgetHub(key: String) {
        val current = Pairing.key(link ?: "") == key
        save(HubList.forget(links, key) { Pairing.key(it) })
        if (!current) return
        close(); reset(); needsRepair = false; pairingError = ""
        if (link != null) connect() else { conn = Conn.OFFLINE; askLink = null; pairEpoch++ }
    }
    /** bumped by unpair(): the pairing screen starts again with empty fields */
    var pairEpoch by mutableStateOf(0); private set
    // forgets the hub and everything a pairing attempt left: a revoked mark, the last message, a pending question
    fun unpair() { links = emptyList(); adding = false; prefs().edit().remove("link").remove("links").apply(); reset(); close(); conn = Conn.OFFLINE; needsRepair = false; pairingError = ""; askLink = null; pairEpoch++ }
    private fun reset() { hubTitle = ""; notice = HubNotice(); star = null; migrating = null; migrateOpen = false; projects = emptyList(); threads = emptyList(); entries = emptyMap(); older = emptyMap(); listsLoaded = false; open = null; cacheFile().delete() }

    // ---- connection
    fun connect() {
        val l = link ?: return
        val base = Pairing.socket(l) ?: return
        close()
        val g = ++gen
        conn = Conn.CONNECTING
        heard = System.currentTimeMillis()
        val url = base + (if (base.contains("?")) "&" else "?") + "data=1&enc=json"
        ws = http.newWebSocket(Request.Builder().url(url).build(), object : WebSocketListener() {
            override fun onMessage(webSocket: WebSocket, text: String) { scope.launch { if (g == gen) { heard = System.currentTimeMillis(); handle(text, g) } } }
            override fun onMessage(webSocket: WebSocket, bytes: okio.ByteString) { val b = bytes.toByteArray(); if (PlotStore.isPlot(b)) scope.launch { if (g == gen) plots.receive(b) } }
            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) { scope.launch { lost(g, "closed") } }
            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                val st = response?.code ?: 0
                // the words a person reads in Settings: not the socket's ("failed to connect to /192.168.77.2 (port 3871) from /10.0.2.16")
                val why = when {
                    st != 0 -> "The hub answered HTTP $st"
                    t is java.net.ConnectException || t is java.net.NoRouteToHostException -> "Can't reach $hubName: is it running, and is this phone on its network (or tailnet)?"
                    t is java.net.SocketTimeoutException -> "$hubName did not answer in time."
                    t is java.net.UnknownHostException -> "$hubName: no such address on this network."
                    else -> t.message ?: "offline"
                }
                scope.launch { if (g == gen && (st == 401 || st == 403)) needsRepair = true; lost(g, why) }
            }
        })
        watchdog = scope.launch { while (isActive) { delay(10_000); if (g != gen) return@launch; if (System.currentTimeMillis() - heard > 40_000) { lost(g, "silent"); return@launch } } }
    }
    private fun close() {
        gen++
        watchdog?.cancel(); reconnect?.cancel()
        ws?.cancel(); ws = null
        calls.values.forEach { it.completeExceptionally(Exception("Offline")) }; calls.clear()
        listSub = 0; threadSub = 0
    }
    fun foreground() { if (link != null && (conn != Conn.ONLINE || System.currentTimeMillis() - heard > 20_000)) connect() else if (link != null) scope.launch { syncPush() } }
    private fun lost(g: Int, why: String) {
        if (g != gen) return
        failure = why; close(); conn = Conn.OFFLINE
        if (link == null || needsRepair) return
        val w = backoff; backoff = minOf(backoff * 2, 4000)
        reconnect = scope.launch { delay(w); connect() }
    }

    private fun handle(text: String, g: Int) {
        val o = runCatching { JSONObject(text) }.getOrNull() ?: return
        when (o.optString("t")) {
            "hello" -> {
                if (!o.optString("proto").startsWith("data/")) { failure = "This hub does not speak the phone data protocol (update it)."; lost(g, failure); return }
                features = o.optJSONArray("features")?.let { a -> (0 until a.length()).map { a.getString(it) }.toSet() } ?: emptySet()
                hubTitle = o.optString("name") // the machine's name (an older hub sends none: the address shows)
                notice = HubRescue.notice(o.optJSONObject("notice"))
                heardMigrating(o.optJSONObject("migrating"))
                commands = o.optJSONArray("commands")?.let { a -> (0 until a.length()).map { val w = a.getJSONObject(it); Word(w.optString("word"), w.optString("label"), w.optString("desc")) } } ?: emptyList()
                conn = Conn.ONLINE; failure = ""; backoff = 250; plots.reset()
                scope.launch { synchronise(g) }
            }
            "reply" -> {
                val d = calls.remove(o.optInt("id", -1)) ?: return
                if (o.optBoolean("ok")) d.complete(o.optJSONObject("result") ?: JSONObject())
                else { val e = o.opt("error"); d.completeExceptionally(Exception(if (e is JSONObject) e.optString("message", e.optString("code")) else e?.toString() ?: "The hub refused")) }
            }
            "d" -> push(o)
            "voice" -> DataVoice.frame(o)
            "plot" -> scope.launch(Dispatchers.Main) { plots.receiveJSON(o) } // why a view has nothing to show (plots themselves are binary)
            "star" -> { star = DStarOffer.of(o.optJSONObject("offer")) }
            "migrating" -> heardMigrating(o)
            "shot" -> { shotBot = o.optString("bot"); shot = o.optLong("n") }
        }
    }

    suspend fun call(m: String, p: JSONObject = JSONObject()): JSONObject {
        val w = ws
        if (conn == Conn.OFFLINE || w == null) throw Exception("Offline")
        val id = nextId++
        val d = CompletableDeferred<JSONObject>(); calls[id] = d
        w.send(JSONObject().put("id", id).put("m", m).put("p", p).toString())
        return try { withTimeout(25_000) { d.await() } } catch (e: TimeoutCancellationException) { calls.remove(id); throw Exception("The hub did not answer") }
    }

    // ---- sync
    private suspend fun synchronise(g: Int) {
        try {
            listSub = call("subscribe", JSONObject().put("topic", "lists")).optInt("sub")
            if (g != gen) return
            coroutineScope { launch { loadProjects(g) }; launch { loadThreads(g) }; launch { runCatching { loadLocks() } }; launch { runCatching { loadStar() } }; launch { loadHistory() } }
            syncPush()
            open?.let { subscribeThread(it, g); refreshThread(it) }
        } catch (e: Exception) { if (g == gen) failure = e.message ?: "" }
    }
    suspend fun refresh() { if (conn == Conn.ONLINE) { val g = gen; runCatching { loadProjects(g); loadThreads(g); loadLocks(); loadHistory() } } }
    // ---- Conversation history (feature history): the old Scratch chats, found and opened, never a place to start one
    var history by mutableStateOf(DHistory()); private set
    var historyOpen by mutableStateOf(false); private set
    fun showHistory(on: Boolean) { historyOpen = on; prefs().edit().putBoolean("history.open", on).apply() }
    suspend fun loadHistory(more: Boolean = false) {
        if (!has("history")) return
        val cursor = history.next; if (more && cursor == null) return
        val r = runCatching { call("history.list", JSONObject().put("limit", 50).also { if (more) it.put("before", cursor) }) }.getOrNull() ?: return
        history = DHistory.parse(r, if (more) history else null)
    }
    private var historyDue = false
    /** A chat of the group moved (a push): ask the hub for the group again, once for a burst. */
    private fun historyMoved() { if (historyDue) return; historyDue = true; scope.launch { kotlinx.coroutines.delay(300); historyDue = false; loadHistory() } }
    var work by mutableStateOf<Map<String, DWork>>(emptyMap()); private set
    var locks by mutableStateOf<List<DLock>>(emptyList()); private set
    private suspend fun loadStar() { if (has("star")) star = DStarOffer.of(call("star.offer").optJSONObject("offer")) }
    /** open (the banner also opens the page) or dismiss: final, the banner goes at once */
    fun starDecide(action: String) {
        val s = star ?: return; star = null
        scope.launch { runCatching { call("star.nudge", JSONObject().put("token", s.token).put("action", action)) } }
    }
    private suspend fun loadLocks() { if (has("work")) locks = parseLocks(call("locks.list").optJSONArray("locks")) }
    private suspend fun loadWork(id: String) { if (has("work")) setWork(id, call("thread.work", JSONObject().put("thread", id)).optJSONObject("subs")) }
    private fun setWork(id: String, o: JSONObject?) { work = if (o == null) work - id else work + (id to parseWork(o)) }
    /** A row's button; what it changes comes back as a push. */
    fun workAct(id: String, r: DWorkRow) { scope.launch { runCatching { call("work.act", JSONObject().put("thread", id).put("act", r.act).put("value", r.value)) } } }
    private suspend fun loadProjects(g: Int) {
        val r = call("projects.list", JSONObject().put("limit", 100)); if (g != gen) return
        projects = parseProjects(r.optJSONArray("projects")).sortedByDescending { it.updatedAt }; listsLoaded = true; save()
    }
    private suspend fun loadThreads(g: Int) {
        // a hub with `settled` sends the live threads alone, so settled ones never push them off the page; the settled ones come per project when its row opens
        val split = has("settled")
        val r = call("threads.list", JSONObject().put("limit", 200).also { if (split) it.put("settled", false) }); if (g != gen) return
        val live = parseThreads(r.optJSONArray("threads"))
        threads = live.sortedByDescending { it.updatedAt } + (if (split) threads.filter { old -> old.settled && live.none { it.id == old.id } } else emptyList())
        listsLoaded = true; save()
        if (split) for (p in settledOpen) loadSettled(p, g = g)
        parentsOf(g)
    }
    /** The parents of subagents and subthreads that are not in the list (a settled one the page left out): read one by one, so a child sits under its parent, as on the
     *  desktop, and not on its own among the active threads. A few at most. */
    private suspend fun parentsOf(g: Int) {
        val have = threads.map { it.id }.toSet()
        for (pid in threads.map { it.parent }.filter { it.isNotEmpty() && it !in have }.distinct().take(20)) {
            val t = runCatching { call("thread.entries", JSONObject().put("thread", pid).put("limit", 1)).optJSONObject("thread") }.getOrNull() ?: continue
            if (g != gen) return
            val p = parseThread(t); if (threads.none { it.id == p.id }) threads = threads + p
        }
        save()
    }
    /** The projects whose "Settled (N)" row is open, kept on the device. */
    var settledOpen by mutableStateOf(emptySet<String>()); private set
    var settledMore by mutableStateOf<Map<String, String>>(emptyMap()); private set   // a project's next page of settled threads
    private val settledBusy = HashSet<String>()
    fun setSettledOpen(project: String, on: Boolean) {
        settledOpen = if (on) settledOpen + project else settledOpen - project
        prefs().edit().putStringSet("settled.open", settledOpen).apply()
        if (on && has("settled") && conn == Conn.ONLINE) { val g = gen; scope.launch { loadSettled(project, g = g) } }
    }
    /** The first page of a project's settled threads, or the next one. */
    suspend fun loadSettled(project: String, more: Boolean = false, g: Int = gen) {
        if (!has("settled") || !settledBusy.add(project)) return
        try {
            val cursor = settledMore[project]; if (more && cursor == null) return
            val q = JSONObject().put("project", project).put("settled", true).put("limit", 50).also { if (more) it.put("before", cursor) }
            val r = runCatching { call("threads.list", q) }.getOrNull() ?: return; if (g != gen) return
            val got = parseThreads(r.optJSONArray("threads"))
            threads = threads.filter { t -> got.none { it.id == t.id } } + got
            settledMore = if (r.isNull("next") || !r.has("next")) settledMore - project else settledMore + (project to r.optString("next"))
            save()
        } finally { settledBusy.remove(project) }
    }
    fun openThread(id: String) {
        open = id
        if (conn != Conn.ONLINE) return
        val g = gen
        if (has("push")) scope.launch { runCatching { call("push.seen", JSONObject().put("thread", id)) }; syncPush() }
        scope.launch { launch { runCatching { subscribeThread(id, g) } }; refreshThread(id) }
    }
    // cache-first, as the iOS app: a thread is read ahead when its row shows and again when a finger goes down on it, so opening it paints from memory
    private val prefetched = HashMap<String, Long>()
    private var prefetchBusy = 0
    fun prefetch(id: String, touch: Boolean = false) {
        if (conn != Conn.ONLINE || id == open || prefetchBusy >= 4) return
        val now = System.currentTimeMillis(); val last = prefetched[id]
        if (last != null && now - last < (if (touch) 10_000 else 60_000)) return
        prefetched[id] = now; prefetchBusy++
        val g = gen
        scope.launch {
            try {
                val r = runCatching { call("thread.entries", JSONObject().put("thread", id).put("limit", 40)) }.getOrNull() ?: return@launch
                if (g != gen || id == open) return@launch
                val fresh = parseEntries(r.optJSONArray("entries"))
                val pending = (entries[id] ?: emptyList()).filter { (it.sending || it.failed) && fresh.none { f -> f.clientId != null && f.clientId == it.clientId } }
                entries = entries + (id to fresh + pending)
                older = if (r.isNull("next")) older - id else older + (id to r.optString("next"))
            } finally { prefetchBusy-- }
        }
    }
    fun closeThread(id: String) {
        if (open == id) open = null
        val s = threadSub; threadSub = 0
        if (s != 0 && conn == Conn.ONLINE) scope.launch { runCatching { call("unsubscribe", JSONObject().put("sub", s)) } }
    }
    private suspend fun subscribeThread(id: String, g: Int) {
        val s = call("subscribe", JSONObject().put("topic", "thread").put("thread", id)); if (g == gen) threadSub = s.optInt("sub")
    }
    private suspend fun refreshThread(id: String) {
        val g = gen
        val r = runCatching { call("thread.entries", JSONObject().put("thread", id).put("limit", 40)) }.getOrNull() ?: return
        if (g != gen) return
        val fresh = parseEntries(r.optJSONArray("entries"))
        val pending = (entries[id] ?: emptyList()).filter { (it.sending || it.failed) && fresh.none { f -> f.clientId != null && f.clientId == it.clientId } }
        entries = entries + (id to fresh + pending)
        older = if (r.isNull("next")) older - id else older + (id to r.optString("next"))
        runCatching { loadWork(id) }
        r.optJSONObject("thread")?.let { upsert(parseThread(it)) }
        save()
    }
    fun loadOlder(id: String) {
        val cursor = older[id] ?: return
        if (conn != Conn.ONLINE) return
        older = older - id
        val g = gen
        scope.launch {
            val r = runCatching { call("thread.entries", JSONObject().put("thread", id).put("limit", 40).put("before", cursor)) }.getOrNull()
            if (r == null || g != gen) { older = older + (id to cursor); return@launch }
            val have = (entries[id] ?: emptyList()).map { it.id }.toSet()
            entries = entries + (id to parseEntries(r.optJSONArray("entries")).filter { it.id !in have } + (entries[id] ?: emptyList()))
            if (!r.isNull("next")) older = older + (id to r.optString("next"))
        }
    }

    // ---- pushes
    private fun push(o: JSONObject) {
        when (o.optString("k")) {
            "thread" -> o.optJSONObject("thread")?.let { val t = parseThread(it); upsert(t); if (t.history) historyMoved() }
            "btw" -> { val tid = o.optString("thread"); if (btw[tid]?.q == o.optString("q")) btw = btw + (tid to Btw(o.optString("q"), o.optString("a"))) }
            "secret" -> scope.launch { refreshSecretAsks(o.optString("thread")) }
            "thread.removed" -> { val id = o.optString("id"); threads = threads.filter { it.id != id }; entries = entries - id; save() }
            "project.removed" -> { val id = o.optString("id"); projects = projects.filter { it.id != id }; save() }
            "project" -> o.optJSONObject("project")?.let { val p = parseProject(it); projects = (projects.filter { x -> x.id != p.id } + p).sortedByDescending { x -> x.updatedAt }; save() }
            "entry" -> o.optJSONObject("entry")?.let { put(parseEntry(it), o.optString("thread")) }
            "work" -> setWork(o.optString("thread"), o.optJSONObject("subs"))
            "term" -> Terms.push(o)
            "locks" -> locks = parseLocks(o.optJSONArray("locks"))
            "text" -> {
                val tid = o.optString("thread"); val eid = o.optString("entry"); val add = o.optString("append")
                val l = entries[tid] ?: return
                entries = entries + (tid to l.map { if (it.id == eid) it.copy(text = it.text + add, live = true) else it })
            }
            "notify" -> notify(o)
            "clear" -> cleared(o.optJSONArray("keys")?.let { a -> (0 until a.length()).map { a.getString(it) } } ?: emptyList())
            "turn" -> { val tid = o.optString("thread"); val s = o.optString("state"); threads = threads.map { if (it.id == tid) it.copy(state = s) else it } }
        }
    }
    private fun upsert(t: DThread) {
        threads = (threads.filter { it.id != t.id } + t).sortedByDescending { it.updatedAt }; save()
    }
    private fun put(e: DEntry, tid: String) {
        var l = entries[tid] ?: emptyList()
        val mine = l.indexOfFirst { it.sending && ((it.clientId == e.clientId && e.clientId != null) || it.clientId == e.id) }
        val rep = e.replaces?.let { r -> l.indexOfFirst { it.id == r } } ?: -1
        val same = l.indexOfFirst { it.id == e.id }
        l = when {
            mine >= 0 -> l.toMutableList().also { it[mine] = e }
            rep >= 0 -> l.toMutableList().also { it[rep] = e }.let { m -> var seen = false; m.filter { x -> if (x.id == e.id) { if (seen) false else { seen = true; true } } else true } }
            same >= 0 -> l.toMutableList().also { it[same] = e }
            else -> l + e
        }
        entries = entries + (tid to l); save()
    }

    // ---- actions
    val canSendImages get() = "images" in features
    fun send(tid: String, text: String, images: List<ByteArray> = emptyList()) {
        val t = text.trim(); if (t.isEmpty() && images.isEmpty()) return
        val cid = UUID.randomUUID().toString()
        val local = DEntry("local:$cid", "user", t, System.currentTimeMillis(), clientId = cid, sending = true, localImages = images)
        entries = entries + (tid to (entries[tid] ?: emptyList()) + local)
        setDraft(tid, ""); fields[tid] = TextFieldValue("")
        scope.launch { deliver(tid, t, cid, images) }
    }
    fun retry(tid: String, e: DEntry) {
        val cid = e.clientId ?: return
        entries = entries + (tid to (entries[tid] ?: emptyList()).map { if (it.id == e.id) it.copy(sending = true, failed = false) else it })
        scope.launch { deliver(tid, e.text, cid, e.localImages) }
    }
    private suspend fun deliver(tid: String, text: String, cid: String, images: List<ByteArray>) {
        try {
            val p = JSONObject().put("thread", tid).put("text", text).put("clientId", cid)
            if (images.isNotEmpty()) p.put("attachments", JSONArray(images.mapIndexed { i, b ->
                JSONObject().put("name", "photo-${i + 1}.jpg").put("mime", "image/jpeg").put("data", android.util.Base64.encodeToString(b, android.util.Base64.NO_WRAP)) }))
            val r = call("message.send", p)
            val e = r.optJSONObject("entry")?.let(::parseEntry)
            // the pictures sent stay on this phone under the hub's ids: no fetch for what we just sent
            e?.attachments?.forEachIndexed { i, a -> if (i < images.size) pictures.put(a.id, images[i]) }
            e?.let { put(it, tid) }
            if (has("queue")) runCatching { loadQueue(tid) } // sent while a turn runs, it waits in the queue
        } catch (e: Exception) {
            entries = entries + (tid to (entries[tid] ?: emptyList()).map { if (it.clientId == cid && it.sending) it.copy(sending = false, failed = true) else it })
        }
    }
    fun answer(tid: String, ask: String, option: String) = scope.launch {
        try {
            call("ask.answer", JSONObject().put("thread", tid).put("ask", ask).put("answer", option))
            entries = entries + (tid to (entries[tid] ?: emptyList()).map { if (it.ask?.id == ask) it.copy(ask = it.ask.copy(state = "answered", answered = option)) else it })
        } catch (e: Exception) { failure = e.message ?: "" }
    }
    fun interrupt(tid: String) { scope.launch { runCatching { call("turn.interrupt", JSONObject().put("thread", tid)) } } }
    suspend fun addProject(path: String): DProject {
        val p = parseProject(call("project.add", JSONObject().put("path", path)).getJSONObject("project"))
        projects = (projects.filter { it.id != p.id } + p).sortedByDescending { it.updatedAt }
        return p
    }
    suspend fun createThread(project: String, text: String, choice: DNewThreadChoice = DNewThreadChoice()): DThread {
        val p = choice.into(JSONObject().put("project", project).put("clientId", UUID.randomUUID().toString()).put("permission", "ask"), skipEmpty = true)
        if (text.isNotBlank()) p.put("text", text.trim())
        val t = parseThread(call("thread.create", p).getJSONObject("thread")); upsert(t); return t
    }
    suspend fun dirs(path: String?): Triple<String, String?, List<String>> {
        val r = call("dirs.list", JSONObject().apply { if (path != null) put("path", path) })
        return Triple(r.optString("path"), if (r.isNull("parent")) null else r.optString("parent"), r.optJSONArray("dirs")?.let { a -> (0 until a.length()).map { a.getString(it) } } ?: emptyList())
    }
    fun draft(tid: String) = prefs().getString("draft.$tid", "") ?: ""
    // the composer's text with its caret, kept per thread: dictation edits it from the hub's frames whether or not the thread is on screen
    private val fields = mutableStateMapOf<String, TextFieldValue>()
    fun field(tid: String): TextFieldValue = fields[tid] ?: TextFieldValue(draft(tid))
    fun setField(tid: String, v: TextFieldValue, typed: Boolean = true) {
        val old = field(tid).text; fields[tid] = v; setDraft(tid, v.text)
        if (typed && old != v.text) DataVoice.typed(tid, old, v.text)
    }
    fun setDraft(tid: String, t: String) { prefs().edit().apply { if (t.isEmpty()) remove("draft.$tid") else putString("draft.$tid", t) }.apply() }

    // ---- the Hub screen: plain HTTP from the pairing alone, whatever the socket is doing
    fun hubHttp(): HubHttp? { val l = link ?: return null; return HubHttp(Pairing.http(l, "", token = false) ?: return null, Pairing.token(l)) }
    fun heardNotice(n: HubNotice) { notice = n }
    // ---- the composer's own commands (feature commands): the hub runs them, the draft stays until it said yes
    /** The command this draft is, by the hub's own rule (session/composer.rs command_is): the word alone, or followed by a space or a new line. */
    fun commandOf(draft: String): Word? {
        val lead = draft.trimStart()
        return commands.firstOrNull { w -> lead.startsWith(w.word) && lead.substring(w.word.length).let { r -> r.isBlank() || r[0] == ' ' || r[0] == '\n' } }
    }
    /** Runs it; throws the hub's plain words when it cannot. The answer's `kind` is what became of it. */
    suspend fun runCommand(tid: String, text: String): JSONObject {
        val r = call("composer.run", JSONObject().put("thread", tid).put("text", text))
        if (r.optString("kind") == "btw") btw = btw + (tid to Btw(r.optString("q"), "Thinking…"))
        return r
    }
    fun closeBtw(tid: String) { btw = btw - tid }
    suspend fun saveSecret(tid: String, name: String, value: String, purpose: String) { call("secret.set", JSONObject().put("name", name).put("value", value).put("purpose", purpose).put("thread", tid)); refreshSecretAsks(tid) }
    suspend fun dismissSecret(tid: String, name: String) { call("secret.dismiss", JSONObject().put("name", name)); refreshSecretAsks(tid) }
    suspend fun refreshSecretAsks(tid: String) {
        if (!has("commands") || tid.isEmpty()) return
        val a = runCatching { call("secret.asks", JSONObject().put("thread", tid)).optJSONArray("asks") }.getOrNull() ?: return
        secretAsks = secretAsks + (tid to (0 until a.length()).map { val x = a.getJSONObject(it); SecretAsk(x.optString("name"), x.optString("purpose")) })
    }

    // ---- the hub's additions to data/1 (features actions, search, models, mentions, bots, notify)
    fun has(f: String) = f in features
    var openFromIntent by mutableStateOf<String?>(null)
    /** A tapped link the hub resolved (DataLinks.kt): the root screen opens it. A link that opens nothing leaves a short note; a project link scrolls the list to it. */
    var linkTarget by mutableStateOf<LinkTarget?>(null)
    var linkNote by mutableStateOf("")
    var focusProject by mutableStateOf("")
    /** An account whose sign-in an alert's tap (or backplane://account/<id>) opens. */
    var openAccount by mutableStateOf<String?>(null)
    // a tapped notification's approval: the thread scrolls to it once it is loaded
    var pendingAsk by mutableStateOf("")
    /** Debug builds: the viewer tab the opened thread's viewer starts on (the intent's `vtab`). */
    var openViewerTab: String? = null
    val plots = PlotStore()
    var shot by mutableStateOf(0L); private set   // the browser page's newest frame announced by the hub (any thread: the tab compares the bot it shows)
    var shotBot by mutableStateOf(""); private set
    var foreground by mutableStateOf(true)
    fun threadAct(tid: String, action: String, title: String = "") = scope.launch {
        try {
            val p = JSONObject().put("thread", tid).put("action", action); if (title.isNotEmpty()) p.put("title", title)
            val r = call("thread.act", p)
            if (action == "delete") { threads = threads.filter { it.id != tid }; entries = entries - tid }
            else r.optJSONObject("thread")?.let { upsert(parseThread(it)) }
        } catch (e: Exception) { failure = e.message ?: "" }
    }
    suspend fun badgeList(): JSONObject = call("badge.list", JSONObject())
    fun projectAct(id: String, action: String, title: String = "", value: String = "") = scope.launch {
        try {
            call("project.act", JSONObject().put("project", id).put("action", action).apply { if (title.isNotEmpty()) put("title", title); if (value.isNotEmpty()) put("value", value) })
            if (action == "remove") { projects = projects.filter { it.id != id }; threads = threads.filter { it.project != id } } else refresh()
        } catch (e: Exception) { failure = e.message ?: "" }
    }
    suspend fun search(q: String): Pair<List<DThread>, List<DProject>> {
        val r = call("search", JSONObject().put("q", q))
        return parseThreads(r.optJSONArray("threads")) to parseProjects(r.optJSONArray("projects"))
    }
    // "Search in thread" (feature search): what was said in one thread with every word of `q`, newest first; `msg` is the entry it is in
    data class Hit(val msg: String, val role: String, val text: String, val at: Long)
    suspend fun searchIn(tid: String, q: String): List<Hit> {
        val a = call("search", JSONObject().put("q", q).put("thread", tid).put("limit", 60)).optJSONArray("messages") ?: return emptyList()
        return (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Hit(it.optString("msg"), it.optString("role"), it.optString("text"), it.optLong("at").let { t -> if (t in 1 until 100_000_000_000) t * 1000 else t }) } // an older hub said seconds
    }
    // Buy it (feature buy): the desktop header's button for this thread (the hub's words), null when there is nothing to buy; a press makes the buy thread, which the app opens
    data class Buy(val label: String, val tip: String)
    suspend fun buyOf(tid: String): Buy? {
        if (!has("buy")) return null
        val o = call("thread.buy", JSONObject().put("thread", tid)).optJSONObject("buy") ?: return null
        return Buy(o.optString("label").ifEmpty { "Buy it" }, o.optString("tip"))
    }
    suspend fun buy(tid: String): String = call("order.buy", JSONObject().put("thread", tid)).optString("thread")
    // what waits for the agent (feature queue): the desktop's strip over the composer, rows and buttons as the hub has them; read again on the thread's pushes and after a send
    data class QButton(val label: String, val action: String, val value: String)
    data class QRow(val msg: String, val tag: String, val text: String, val buttons: List<QButton>)
    var queues by mutableStateOf<Map<String, List<QRow>>>(emptyMap()); private set
    suspend fun loadQueue(tid: String) {
        if (!has("queue")) return
        val a = call("thread.queue", JSONObject().put("thread", tid)).optJSONArray("rows")
        val rows = if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { r ->
            val b = r.optJSONArray("buttons"); QRow(r.optString("msg"), r.optString("tag"), r.optString("text"), if (b == null) emptyList() else (0 until b.length()).mapNotNull { b.optJSONObject(it) }.map { QButton(it.optString("label"), it.optString("action"), it.optString("value")) })
        }
        queues = if (rows.isEmpty()) queues - tid else queues + (tid to rows)
    }
    /** A queue row's button; q-edit answers the message's text (it left the queue) for the draft. */
    suspend fun queueAct(tid: String, b: QButton): String {
        val r = call("queue.act", JSONObject().put("thread", tid).put("action", b.action).put("value", b.value))
        runCatching { loadQueue(tid) }
        return r.optString("text")
    }
    // a buy thread's Order card (feature order): the hub's, drawn as it says; null for any other thread
    data class OrderStep(val label: String, val mark: String)
    data class Order(val name: String, val steps: List<OrderStep>, val line: String, val waiting: Boolean, val total: String, val customs: String, val pay: String, val label: String)
    suspend fun order(tid: String): Order? {
        val o = call("thread.order", JSONObject().put("thread", tid)).optJSONObject("order") ?: return null
        val st = o.optJSONArray("steps")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { OrderStep(it.optString("label"), it.optString("mark")) } } ?: emptyList()
        return Order(o.optString("name"), st, o.optString("line"), o.optBoolean("waiting"), o.optString("total"), o.optString("customs"), o.optString("pay"), o.optString("label"))
    }
    suspend fun picker(thread: String?, choice: DNewThreadChoice = DNewThreadChoice()): DPicker {
        val p = choice.into(JSONObject()); if (thread != null) p.put("thread", thread)
        return parsePicker(call("thread.picker", p).getJSONObject("picker"))
    }
    suspend fun pick(tid: String, action: String, value: String) { call("thread.pick", JSONObject().put("thread", tid).put("action", action).put("value", value)).optJSONObject("thread")?.let { upsert(parseThread(it)) } }
    /** `word`: what goes in the draft, the desktop's word (`>bom-review`, `%demo`), which /give and the reference cards read; "" from an older hub. */
    data class Mention(val sigil: String, val id: String, val label: String, val detail: String, val word: String = "")
    suspend fun mentions(q: String, kind: String): List<Mention> {
        val a = call("mentions.list", JSONObject().put("q", q).put("kind", kind).put("limit", 8)).optJSONArray("items") ?: return emptyList()
        return (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Mention(it.optString("sigil"), it.optString("id"), it.optString("label"), it.optString("detail"), it.optString("word")) }
    }
    data class Bot(val id: String, val name: String, val thread: String, val uber: Boolean)
    data class Room(val id: String, val name: String)
    var bots by mutableStateOf<List<Bot>>(emptyList()); private set
    var rooms by mutableStateOf<List<Room>>(emptyList()); private set
    suspend fun loadBots() {
        val r = call("bots.list")
        bots = r.optJSONArray("bots")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Bot(it.optString("id"), it.optString("name"), it.optString("thread"), it.optBoolean("uber")) } } ?: emptyList()
        rooms = r.optJSONArray("rooms")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Room(it.optString("id"), it.optString("name")) } } ?: emptyList()
    }
    data class Post(val from: String, val text: String, val at: Long)
    suspend fun roomPosts(room: String): List<Post> = call("room.posts", JSONObject().put("room", room).put("limit", 60)).optJSONArray("posts")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Post(it.optString("from"), it.optString("text"), it.optLong("at")) } } ?: emptyList()
    suspend fun roomPost(room: String, text: String) { call("room.post", JSONObject().put("room", room).put("text", text).put("clientId", UUID.randomUUID().toString())) }
    suspend fun botSay(bot: String, text: String) { call("bot.say", JSONObject().put("bot", bot).put("text", text).put("clientId", UUID.randomUUID().toString())) }

    // alerts: only what the hub's gate lets through reaches here (native/host/src/push): this phone has no push service, so it hears them on its socket while the app lives
    private val alertTag = "push"
    private fun heardMigrating(m: JSONObject?) { migrating = DMigrateCard.of(m); if (migrating == null) migrateOpen = false }
    /** A press on the card's button: the hub's own onboard.act (setup's buttons). Once the hub has taken a press on a finished card (Continue, Not now, Retry)
     *  that card is over; a running import's frames bring the next one. */
    suspend fun migrateAct(card: DMigrateCard, b: DMigrateButton) {
        call("onboard.act", JSONObject().put("action", b.action).put("value", b.value))
        if (!card.running && migrating == card) { migrating = null; migrateOpen = false }
    }
    private fun notify(o: JSONObject) {
        if (foreground) return
        if (!AppSettings.alertsOn() && !o.optBoolean("through")) return
        val tid = o.optString("thread"); val key = o.optString("key"); val ask = o.optString("ask")
        val nm = app.getSystemService(Context.NOTIFICATION_SERVICE) as android.app.NotificationManager
        nm.createNotificationChannel(android.app.NotificationChannel("data-alerts", "Alerts", android.app.NotificationManager.IMPORTANCE_DEFAULT))
        // a notice of the hub's own (kind import) has no thread: its tap opens the lists
        val intent = android.content.Intent(app, DataActivity::class.java).apply { if (tid.isNotEmpty()) putExtra("thread", tid) }.putExtra("ask", ask).putExtra("link", o.optString("link")).addFlags(android.content.Intent.FLAG_ACTIVITY_SINGLE_TOP or android.content.Intent.FLAG_ACTIVITY_CLEAR_TOP)
        val open = android.app.PendingIntent.getActivity(app, key.hashCode(), intent, android.app.PendingIntent.FLAG_IMMUTABLE or android.app.PendingIntent.FLAG_UPDATE_CURRENT)
        val n = android.app.Notification.Builder(app, "data-alerts").setSmallIcon(android.R.drawable.stat_notify_chat).setContentTitle(o.optString("title").ifEmpty { "Backplane" })
            .setContentText(o.optString("body")).apply { if (tid.isNotEmpty()) setGroup(tid) }.setNumber(o.optInt("badge")).setContentIntent(open).setAutoCancel(true).build()
        nm.notify(alertTag, key.hashCode(), n)
    }
    // what was dealt with leaves this phone, by its key
    private fun cleared(keys: List<String>) { val nm = app.getSystemService(Context.NOTIFICATION_SERVICE) as android.app.NotificationManager; keys.forEach { nm.cancel(alertTag, it.hashCode()) } }
    var quiet by mutableStateOf(""); private set
    var quietUrgent by mutableStateOf(false); private set
    // what is waiting, on connect and on coming to the front: whatever was dealt with while this phone slept leaves it
    private suspend fun syncPush() {
        if (!has("push")) return
        val r = runCatching { call("push.state") }.getOrNull() ?: return
        quiet = r.optString("quiet"); quietUrgent = r.optBoolean("urgent")
        val keys = r.optJSONArray("keys")?.let { a -> (0 until a.length()).map { a.getString(it).hashCode() }.toSet() } ?: emptySet()
        val nm = app.getSystemService(Context.NOTIFICATION_SERVICE) as android.app.NotificationManager
        nm.activeNotifications.filter { it.tag == alertTag && it.id !in keys }.forEach { nm.cancel(alertTag, it.id) }
    }
    fun putQuiet(spec: String) { quiet = spec; scope.launch { runCatching { call("push.prefs", JSONObject().put("notify.quiet", spec)) } } }
    fun putQuietUrgent(on: Boolean) { quietUrgent = on; scope.launch { runCatching { call("push.prefs", JSONObject().put("notify.quiet.urgent", if (on) "on" else "off")) } } }

    // ---- viewers, files, browser (features viewers, files, browser; native/host/src/data/ext.rs)
    data class ProjectFiles(val root: String, val pcb: String, val sch: String)
    suspend fun viewerFiles(tid: String): ProjectFiles { val r = call("viewer.files", JSONObject().put("thread", tid)); return ProjectFiles(r.optString("root"), r.optString("pcb"), r.optString("sch")) }
    fun stopWatch(tid: String) { scope.launch { runCatching { call("viewer.watch", JSONObject().put("thread", tid).put("kind", "")) } } }
    /** A viewer (tab, file, thread) opens: the last one's plots go at once and late ones for it are dropped (PlotStore.begin/shown);
     *  a 3D view shows its own cached model once the hub names that model's file among the view's. A stop changes nothing here. */
    suspend fun watch(tid: String, kind: String, path: String = "") {
        val slot = if (kind.startsWith("3d")) listOf(hubName, tid, kind, path).joinToString("|") else ""
        val begun = if (kind.isNotEmpty()) withContext(Dispatchers.Main) { plots.begin(slot) } else null
        val kept = begun?.kept.orEmpty(); val serial = begun?.serial ?: 0
        val r = try { call("viewer.watch", JSONObject().put("thread", tid).put("kind", kind).apply { if (path.isNotEmpty()) put("path", path); if (kept.isNotEmpty()) put("kept", kept) }) }
            catch (e: Exception) { if (kind.isNotEmpty()) withContext(Dispatchers.Main) { plots.failed(serial) }; throw e }
        if (kind.isEmpty()) return
        val keys = r.optJSONArray("keys")?.let { a -> (0 until a.length()).map { a.getString(it) } }.orEmpty()
        withContext(Dispatchers.Main) { plots.shown(keys.ifEmpty { PlotStore.keys(kind, r.optString("root"), path) }, serial) }
    }
    /** `viewer.layers`: what a press does, and what to draw; the rules are the hub's (present/viewer.rs `layer_reply`). `rows`: the layers, then a comparison's marks. */
    data class LayerRow(val layer: Int, val name: String, val on: Boolean, val active: Boolean)
    data class Layers(val own: Int, val focus: String, val off: Int, val dim: Int, val lift: Int, val bits: Int, val head: String, val mode: Int, val modes: Boolean, val reset: Boolean, val rows: List<LayerRow>)
    suspend fun layers(kind: String, present: Int, own: Int, focus: String, action: String = "", value: String = ""): Layers {
        val r = call("viewer.layers", JSONObject().put("kind", kind).put("present", present).put("off", own).put("focus", focus).put("action", action).put("value", value))
        fun rows(k: String) = r.optJSONArray(k)?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { LayerRow(it.optInt("layer"), it.optString("name"), it.optBoolean("on"), it.optBoolean("active")) } }.orEmpty()
        return Layers(r.optInt("own"), r.optString("focus"), r.optInt("off"), r.optInt("dim"), r.optInt("lift", 255), r.optInt("layerBits"), r.optString("layerHead", "Layers"), r.optInt("layerMode"), r.optBoolean("layerModes"), r.optBoolean("layerReset"), rows("layerList") + rows("layerMarks"))
    }
    data class Ref(val kind: String, val label: String, val commit: String)
    suspend fun refs(tid: String): List<Ref> = call("viewer.refs", JSONObject().put("thread", tid)).optJSONArray("refs")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Ref(it.optString("kind"), it.optString("label"), it.optString("commit")) } }.orEmpty()
    suspend fun compare(tid: String, kind: String, commit: String, bits: Int, path: String) { call("viewer.compare", JSONObject().put("thread", tid).put("kind", kind).put("commit", commit).put("bits", bits).put("path", path)) }
    suspend fun info(key: String, at: Int, idx: Int): String = call("viewer.info", JSONObject().put("key", key).put("at", at).put("idx", idx)).optString("info")
    /** The cross-probe's light: the pieces ("chunk,piece") of the plot held for `key` whose info has `name=value` (a part's `ref`, a `net`). */
    suspend fun probe(key: String, name: String, value: String): List<String> {
        val a = call("viewer.probe", JSONObject().put("key", key).put("name", name).put("value", value)).optJSONArray("matches") ?: return emptyList()
        return (0 until a.length()).mapNotNull { a.optJSONArray(it) }.map { "${it.optInt(0)},${it.optInt(1)}" }
    }
    data class Entry(val dir: Boolean, val size: Long, val name: String)
    suspend fun entries(tid: String, path: String): List<Entry> = call("files.entries", JSONObject().put("thread", tid).put("path", path)).optJSONArray("entries")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Entry(it.optBoolean("dir"), it.optLong("size"), it.optString("name")) } } ?: emptyList()
    data class Mech(val parts: List<String>, val renders: List<String>)
    private fun strings(a: JSONArray?): List<String> = if (a == null) emptyList() else (0 until a.length()).map { a.getString(it) }
    suspend fun mech(tid: String): Mech { val r = call("mech.list", JSONObject().put("thread", tid)); return Mech(strings(r.optJSONArray("parts")), strings(r.optJSONArray("renders"))) }
    suspend fun browser(tid: String, op: String, extra: JSONObject.() -> Unit = {}) { call("browser", JSONObject().put("thread", tid).put("op", op).apply(extra)) }
    private fun httpGet(path: String, query: String): ByteArray? = runCatching {
        val u = Pairing.http(link ?: return null, path, query) ?: return null
        http.newCall(Request.Builder().url(u).build()).execute().use { r -> if (r.isSuccessful) r.body?.bytes() else null }
    }.getOrNull()
    suspend fun fileBytes(tid: String, path: String): ByteArray? = withContext(Dispatchers.IO) { httpGet("/files/get", "thread=" + android.net.Uri.encode(tid) + "&path=" + android.net.Uri.encode(path)) }
    suspend fun screenBytes(tid: String): ByteArray? = withContext(Dispatchers.IO) { httpGet("/bots/" + android.net.Uri.encode(tid) + "/screen.jpg", "") }

    // ---- diff and review (features diff, review; the hub's git.diff and review.* requests)
    suspend fun diff(tid: String): Pair<String, String> { val r = call("viewer.diff", JSONObject().put("thread", tid)); return r.optString("diff") to r.optString("nogit") }
    data class Task(val text: String, val done: Boolean, val group: String)
    data class Report(val md: String, val err: String, val gate: String, val line: String, val tasks: List<Task>)
    suspend fun reviewPage(tid: String, page: String): Report {
        val r = call("review.page", JSONObject().put("thread", tid).put("page", page)).optJSONObject("report") ?: JSONObject()
        val ts = r.optJSONArray("tasks")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Task(it.optString("text"), it.optBoolean("done"), it.optString("group")) } } ?: emptyList()
        return Report(r.optString("md"), r.optString("err"), r.optString("gate"), r.optString("line"), ts)
    }
    suspend fun tick(tid: String, index: Int): List<Task> {
        val a = call("review.tick", JSONObject().put("thread", tid).put("index", index)).optJSONObject("ticked")?.optJSONArray("tasks") ?: return emptyList()
        return (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Task(it.optString("text"), it.optBoolean("done"), it.optString("group")) }
    }
    data class Comment(val id: String, val text: String, val by: String, val parent: String, val resolved: Boolean, val file: String)
    suspend fun comments(tid: String): List<Comment> = call("review.list", JSONObject().put("thread", tid)).optJSONArray("comments")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Comment(it.optString("id"), it.optString("text"), it.optString("by"), it.optString("parent"), it.optBoolean("resolved"), it.optString("file")) } } ?: emptyList()
    suspend fun comment(tid: String, op: String, extra: JSONObject.() -> Unit) { call("review.comment", JSONObject().put("thread", tid).put("op", op).apply(extra)) }

    // ---- pictures
    private val pictures = object : android.util.LruCache<String, ByteArray>(16 shl 20) { override fun sizeOf(k: String, v: ByteArray) = v.size }
    suspend fun picture(id: String): ByteArray? {
        pictures.get(id)?.let { return it }
        val r = runCatching { call("attachment.get", JSONObject().put("id", id)) }.getOrNull() ?: return null
        val b = runCatching { android.util.Base64.decode(r.optString("data"), android.util.Base64.DEFAULT) }.getOrNull() ?: return null
        pictures.put(id, b); return b
    }
    // a picture chosen: the longest side at most 2048 px, JPEG at 80 (about a megabyte at most; the hub takes 8 MB)
    fun prepared(ctx: Context, uri: android.net.Uri): ByteArray? = runCatching {
        val bytes = ctx.contentResolver.openInputStream(uri)!!.use { it.readBytes() }
        val o = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
        android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size, o)
        var sample = 1; while (maxOf(o.outWidth, o.outHeight) / sample > 4096) sample *= 2
        var bmp = android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size, android.graphics.BitmapFactory.Options().apply { inSampleSize = sample })!!
        val longest = maxOf(bmp.width, bmp.height)
        if (longest > 2048) { val f = 2048f / longest; bmp = android.graphics.Bitmap.createScaledBitmap(bmp, (bmp.width * f).toInt(), (bmp.height * f).toInt(), true) }
        java.io.ByteArrayOutputStream().also { bmp.compress(android.graphics.Bitmap.CompressFormat.JPEG, 80, it) }.toByteArray()
    }.getOrNull()

    // ---- cache (paints at launch, replaced as soon as the hub answers)
    private fun cacheFile() = java.io.File(app.filesDir, "data-cache-v1.json")
    private fun J(o: DEntry) = JSONObject().put("id", o.id).put("kind", o.kind).put("text", o.text).put("at", o.at).apply {
        if (o.agentOnly) put("agentOnly", true)
        o.term?.let { t -> put("term", JSONObject().put("key", t.key).put("id", t.id).put("title", t.title).put("cols", t.cols).put("rows", t.rows).put("cmd", t.cmd)) }
        o.tool?.let { put("tool", JSONObject().put("name", it.name).put("summary", it.summary).put("state", it.state)) }
        o.ask?.let { a -> put("ask", JSONObject().put("id", a.id).put("state", a.state).put("question", a.question).put("answered", a.answered ?: JSONObject.NULL)
            .put("options", JSONArray(a.options.map { JSONObject().put("id", it.id).put("label", it.label) }))) }
    }
    private fun loadCache() {
        val hub = link?.let { Pairing.key(it) } ?: return
        runCatching {
            val c = JSONObject(cacheFile().readText()); if (c.optString("hub") != hub) return
            projects = parseProjects(c.optJSONArray("projects")); threads = parseThreads(c.optJSONArray("threads"))
            val e = c.optJSONObject("entries"); entries = e?.keys()?.asSequence()?.associateWith { parseEntries(e.optJSONArray(it)) } ?: emptyMap()
            listsLoaded = threads.isNotEmpty() || projects.isNotEmpty()
        }
    }
    private fun save() {
        saveJob?.cancel()
        saveJob = scope.launch {
            delay(1000)
            val hub = link?.let { Pairing.key(it) } ?: return@launch
            val ps = projects; val ts = threads
            val recent = ts.take(10).map { it.id }.toSet()
            val es = entries.filterKeys { it in recent }.mapValues { (_, v) -> v.filter { !it.sending && !it.failed }.takeLast(40) }
            withContext(Dispatchers.IO) {
                val j = JSONObject().put("hub", hub)
                    .put("projects", JSONArray(ps.map { JSONObject().put("id", it.id).put("title", it.title).put("root", it.root).put("threadCount", it.threadCount).apply { it.settledCount?.let { n -> put("settledCount", n) } }.put("updatedAt", it.updatedAt).put("waiting", it.waiting) }))
                    .put("threads", JSONArray(ts.map { JSONObject().put("id", it.id).put("project", it.project).put("title", it.title).put("state", it.state).put("updatedAt", it.updatedAt).put("pinned", it.pinned).put("settled", it.settled).put("settledAt", it.settledAt).put("archived", it.archived).put("provider", it.provider).put("model", it.model).put("effort", it.effort).put("runtime", it.runtime).put("chip", it.chip).put("preview", it.preview).put("waiting", it.waiting) }))
                    .put("entries", JSONObject().also { o -> es.forEach { (k, v) -> o.put(k, JSONArray(v.map(::J))) } })
                runCatching { cacheFile().writeText(j.toString()) }
            }
        }
    }
}

/** The GitHub star suggestion while the hub says it is out and undecided (feature "star", native/host/src/state/star.rs): from star.offer at every
 *  (re)connect and from a `{"t":"star","offer"}` frame (null hides it). The hub decides when; the app shows exactly this. */
data class DStarOffer(val token: String, val text: String, val url: String, val label: String) {
    companion object {
        fun of(o: JSONObject?): DStarOffer? {
            if (o == null) return null
            val s = DStarOffer(o.optString("token"), o.optString("text"), o.optString("url"), o.optString("label"))
            return if (s.token.isNotEmpty() && s.text.isNotEmpty() && s.url.startsWith("https://")) s else null
        }
    }
}
