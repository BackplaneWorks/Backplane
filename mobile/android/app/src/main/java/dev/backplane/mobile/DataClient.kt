package dev.backplane.mobile

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
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
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
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
    var link by mutableStateOf<String?>(null); private set
    var pairingError by mutableStateOf(""); var pairingBusy by mutableStateOf(false)
    /** a pairing link that came from outside the app: nothing happens until the person says so */
    var askLink by mutableStateOf<String?>(null)
    val hubName get() = link?.let { Pairing.key(it) } ?: ""

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
        link = prefs().getString("link", null)
        System.getenv("BACKPLANE_LINK")?.let { link = it }
        loadCache()
        if (link != null) connect()
    }
    private fun prefs() = app.getSharedPreferences("data", Context.MODE_PRIVATE)

    // ---- pairing
    fun pair(text: String) {
        val inv = Pairing.invitation(text)
        if (inv != null) {
            if (pairingBusy) return
            pairingBusy = true; pairingError = "Connecting…"
            scope.launch {
                try { val l = Pairing.exchange(inv); pairingError = ""; adopt(l) }
                catch (e: Exception) { pairingError = e.message ?: "Hub unreachable" }
                pairingBusy = false
            }
            return
        }
        if (Pairing.bareCode(text)) { pairingError = "Enter the hub's address with the short code."; return }
        if (Pairing.malformedInvitation(text)) { pairingError = "Enter the complete invitation link or a valid eight-character code."; return }
        adopt(text.trim())
    }
    private fun adopt(l: String) {
        if (Pairing.key(l) == null) { pairingError = "Enter the hub's address or pairing link."; return }
        val changed = Pairing.key(link ?: "") != Pairing.key(l)
        link = l; prefs().edit().putString("link", l).apply(); needsRepair = false
        if (changed) reset()
        connect()
    }
    fun unpair() { link = null; prefs().edit().remove("link").apply(); reset(); close(); conn = Conn.OFFLINE }
    private fun reset() { notice = HubNotice(); projects = emptyList(); threads = emptyList(); entries = emptyMap(); older = emptyMap(); listsLoaded = false; open = null; cacheFile().delete() }

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
                scope.launch { if (g == gen && (st == 401 || st == 403)) needsRepair = true; lost(g, if (st == 0) (t.message ?: "offline") else "HTTP $st") }
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
    fun foreground() { if (link != null && (conn != Conn.ONLINE || System.currentTimeMillis() - heard > 20_000)) connect() }
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
                notice = HubRescue.notice(o.optJSONObject("notice"))
                conn = Conn.ONLINE; failure = ""; backoff = 250; plots.reset()
                scope.launch { synchronise(g) }
            }
            "reply" -> {
                val d = calls.remove(o.optInt("id", -1)) ?: return
                if (o.optBoolean("ok")) d.complete(o.optJSONObject("result") ?: JSONObject())
                else { val e = o.opt("error"); d.completeExceptionally(Exception(if (e is JSONObject) e.optString("message", e.optString("code")) else e?.toString() ?: "The hub refused")) }
            }
            "d" -> push(o)
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
            coroutineScope { launch { loadProjects(g) }; launch { loadThreads(g) } }
            open?.let { subscribeThread(it, g); refreshThread(it) }
        } catch (e: Exception) { if (g == gen) failure = e.message ?: "" }
    }
    suspend fun refresh() { if (conn == Conn.ONLINE) { val g = gen; runCatching { loadProjects(g); loadThreads(g) } } }
    private suspend fun loadProjects(g: Int) {
        val r = call("projects.list", JSONObject().put("limit", 100)); if (g != gen) return
        projects = parseProjects(r.optJSONArray("projects")).sortedByDescending { it.updatedAt }; listsLoaded = true; save()
    }
    private suspend fun loadThreads(g: Int) {
        val r = call("threads.list", JSONObject().put("limit", 200)); if (g != gen) return
        threads = parseThreads(r.optJSONArray("threads")).sortedByDescending { it.updatedAt }; listsLoaded = true; save()
    }
    fun openThread(id: String) {
        open = id
        if (conn != Conn.ONLINE) return
        val g = gen
        scope.launch { launch { runCatching { subscribeThread(id, g) } }; refreshThread(id) }
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
            "thread" -> o.optJSONObject("thread")?.let { upsert(parseThread(it)) }
            "thread.removed" -> { val id = o.optString("id"); threads = threads.filter { it.id != id }; entries = entries - id; save() }
            "project.removed" -> { val id = o.optString("id"); projects = projects.filter { it.id != id }; save() }
            "project" -> o.optJSONObject("project")?.let { val p = parseProject(it); projects = (projects.filter { x -> x.id != p.id } + p).sortedByDescending { x -> x.updatedAt }; save() }
            "entry" -> o.optJSONObject("entry")?.let { put(parseEntry(it), o.optString("thread")) }
            "text" -> {
                val tid = o.optString("thread"); val eid = o.optString("entry"); val add = o.optString("append")
                val l = entries[tid] ?: return
                entries = entries + (tid to l.map { if (it.id == eid) it.copy(text = it.text + add, live = true) else it })
            }
            "notify" -> notify(o)
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
        setDraft(tid, "")
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
    fun setDraft(tid: String, t: String) { prefs().edit().apply { if (t.isEmpty()) remove("draft.$tid") else putString("draft.$tid", t) }.apply() }

    // ---- the Hub screen: plain HTTP from the pairing alone, whatever the socket is doing
    fun hubHttp(): HubHttp? { val l = link ?: return null; return HubHttp(Pairing.http(l, "", token = false) ?: return null, Pairing.token(l)) }
    fun heardNotice(n: HubNotice) { notice = n }

    // ---- the hub's additions to data/1 (features actions, search, models, mentions, bots, notify)
    fun has(f: String) = f in features
    var openFromIntent by mutableStateOf<String?>(null)
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
    data class Mention(val sigil: String, val id: String, val label: String, val detail: String)
    suspend fun mentions(q: String, kind: String): List<Mention> {
        val a = call("mentions.list", JSONObject().put("q", q).put("kind", kind).put("limit", 8)).optJSONArray("items") ?: return emptyList()
        return (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { Mention(it.optString("sigil"), it.optString("id"), it.optString("label"), it.optString("detail")) }
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

    // alerts: a turn that ended or asks, while the app is not on screen (the hub's `notify` push on the lists subscription)
    private fun notify(o: JSONObject) {
        if (foreground || !AppSettings.alertsOn()) return
        val kind = o.optString("kind"); val tid = o.optString("thread")
        val nm = app.getSystemService(Context.NOTIFICATION_SERVICE) as android.app.NotificationManager
        nm.createNotificationChannel(android.app.NotificationChannel("data-alerts", "Threads", android.app.NotificationManager.IMPORTANCE_DEFAULT))
        val open = android.app.PendingIntent.getActivity(app, tid.hashCode(), android.content.Intent(app, DataActivity::class.java).putExtra("thread", tid).addFlags(android.content.Intent.FLAG_ACTIVITY_SINGLE_TOP), android.app.PendingIntent.FLAG_IMMUTABLE or android.app.PendingIntent.FLAG_UPDATE_CURRENT)
        val n = android.app.Notification.Builder(app, "data-alerts").setSmallIcon(android.R.drawable.stat_notify_chat).setContentTitle(o.optString("title").ifEmpty { "Backplane" })
            .setContentText(when (kind) { "ask" -> "Waiting for your answer: " + o.optString("body"); "fail" -> "The turn failed"; else -> "Finished" }).setContentIntent(open).setAutoCancel(true).build()
        nm.notify(tid.hashCode(), n)
    }

    // ---- viewers, files, browser (features viewers, files, browser; native/host/src/data/ext.rs)
    data class ProjectFiles(val root: String, val pcb: String, val sch: String)
    suspend fun viewerFiles(tid: String): ProjectFiles { val r = call("viewer.files", JSONObject().put("thread", tid)); return ProjectFiles(r.optString("root"), r.optString("pcb"), r.optString("sch")) }
    fun stopWatch(tid: String) { scope.launch { runCatching { call("viewer.watch", JSONObject().put("thread", tid).put("kind", "")) } } }
    suspend fun watch(tid: String, kind: String, path: String = "") { call("viewer.watch", JSONObject().put("thread", tid).put("kind", kind).apply { if (path.isNotEmpty()) put("path", path) }) }
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
                    .put("projects", JSONArray(ps.map { JSONObject().put("id", it.id).put("title", it.title).put("root", it.root).put("threadCount", it.threadCount).put("updatedAt", it.updatedAt).put("waiting", it.waiting) }))
                    .put("threads", JSONArray(ts.map { JSONObject().put("id", it.id).put("project", it.project).put("title", it.title).put("state", it.state).put("updatedAt", it.updatedAt).put("pinned", it.pinned).put("settled", it.settled).put("archived", it.archived).put("provider", it.provider).put("model", it.model).put("effort", it.effort).put("runtime", it.runtime).put("chip", it.chip).put("preview", it.preview).put("waiting", it.waiting) }))
                    .put("entries", JSONObject().also { o -> es.forEach { (k, v) -> o.put(k, JSONArray(v.map(::J))) } })
                runCatching { cacheFile().writeText(j.toString()) }
            }
        }
    }
}
