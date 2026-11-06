package dev.backplane.mobile

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

// Screens for the data/1 app, in the desktop client's language (native/gpui): flat panels on the same tokens, square corners, mono labels, a state dot per thread.
internal val dcs @Composable get() = MaterialTheme.colorScheme
internal val damber = Color(0xFFE0A030)
@Composable internal fun dStateColor(s: String) = when (s) { "working" -> dcs.primary; "asking" -> damber; "failed" -> dcs.error; else -> dcs.outlineVariant }
internal fun dAgo(ms: Long): String {
    val s = (System.currentTimeMillis() - ms) / 1000
    return when { ms <= 0 -> ""; s < 60 -> "now"; s < 3600 -> "${s / 60}m"; s < 86400 -> "${s / 3600}h"; else -> "${s / 86400}d" }
}
@Composable internal fun DMono(t: String, c: Color = dcs.onSurfaceVariant, size: Int = 11, modifier: Modifier = Modifier, bold: Boolean = false) =
    Text(t, modifier, color = c, fontFamily = BpMono, fontSize = size.sp, fontWeight = if (bold) FontWeight.Bold else FontWeight.Normal, maxLines = 1, overflow = TextOverflow.Ellipsis)
@Composable internal fun DDot(c: Color) = Box(Modifier.size(8.dp).background(c))
@Composable internal fun DLine() = Box(Modifier.fillMaxWidth().height(1.dp).background(dcs.outlineVariant))

private sealed interface Page { object Improve : Page; data class Room(val room: DataClient.Room) : Page; data class Viewer(val id: String) : Page; object Home : Page; data class Thread(val id: String) : Page; object Settings : Page; object Folders : Page; data class New(val project: String) : Page }

@Composable
fun DataRoot() {
    var page by remember { mutableStateOf<Page>(DataClient.openFromIntent?.let { Page.Thread(it) } ?: Page.Home) }
    LaunchedEffect(DataClient.openFromIntent) { DataClient.openFromIntent?.let { page = Page.Thread(it); DataClient.openFromIntent = null } }
    Box(Modifier.fillMaxSize().background(dcs.background).windowInsetsPadding(WindowInsets.systemBars.union(WindowInsets.ime))) {
        if (DataClient.link == null) PairScreen()
        else when (val p = page) {
            Page.Home -> HomeScreen({ page = Page.Thread(it) }, { page = Page.Settings }, { page = Page.Folders }, { page = Page.New(it) }) { page = Page.Room(it) }
            is Page.Viewer -> { BackHandler { page = Page.Thread(p.id) }; DataClient.threads.firstOrNull { it.id == p.id }?.let { ViewerScreen(it) { page = Page.Thread(p.id) } } }
            is Page.Room -> { BackHandler { page = Page.Home }; RoomScreen(p.room) { page = Page.Home } }
            is Page.Thread -> { BackHandler { page = Page.Home }; ThreadScreen(p.id, { page = Page.Viewer(p.id) }) { page = Page.Home } }
            Page.Settings -> { BackHandler { page = Page.Home }; SettingsScreen({ page = Page.Home }) { page = Page.Improve } }
            Page.Improve -> { BackHandler { page = Page.Settings }; ImproveScreen { page = Page.Settings } }
            Page.Folders -> { BackHandler { page = Page.Home }; FolderScreen({ page = Page.Home }) { page = Page.New(it) } }
            is Page.New -> { BackHandler { page = Page.Home }; NewThreadScreen(p.project, { page = Page.Home }) { page = Page.Thread(it) } }
        }
    }
}

@Composable
internal fun DBar(title: String, back: (() -> Unit)?, trailing: @Composable RowScope.() -> Unit = {}) {
    Column(Modifier.background(dcs.surfaceVariant)) {
        Row(Modifier.fillMaxWidth().height(48.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            if (back != null) IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back", tint = dcs.onSurface) } else Spacer(Modifier.width(12.dp))
            Text(title.ifEmpty { "New thread" }, Modifier.weight(1f), color = dcs.onSurface, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            trailing()
        }
        DLine()
    }
}

@Composable
private fun ConnBanner() {
    if (DataClient.conn == DataClient.Conn.ONLINE) return
    Row(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        DDot(if (DataClient.conn == DataClient.Conn.CONNECTING) damber else dcs.error); Spacer(Modifier.width(8.dp))
        DMono(if (DataClient.conn == DataClient.Conn.CONNECTING) "connecting to ${DataClient.hubName}…" else "offline · showing what this phone has saved", dcs.onSurface, 12)
    }
}

@Composable
private fun PairScreen() {
    var text by remember { mutableStateOf("") }
    val ctx = androidx.compose.ui.platform.LocalContext.current
    Column(Modifier.fillMaxSize().padding(24.dp), verticalArrangement = Arrangement.Center) {
        DMono("BACKPLANE", dcs.primary, 13, bold = true)
        Spacer(Modifier.height(8.dp))
        Text("Pair with a hub", color = dcs.onSurface, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(4.dp))
        Text("Paste the hub's pairing link or its address.", color = dcs.onSurfaceVariant, fontSize = 14.sp)
        Spacer(Modifier.height(20.dp))
        DField(text, { text = it }, "http://host:3787/#invite=…", Modifier.testTag("pair.field"))
        if (DataClient.pairingError.isNotEmpty()) { Spacer(Modifier.height(8.dp)); Text(DataClient.pairingError, color = dcs.onSurfaceVariant, fontSize = 13.sp) }
        Spacer(Modifier.height(16.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DBtn("Pair", true, text.isNotBlank() && !DataClient.pairingBusy, Modifier.testTag("pair.go")) { DataClient.pair(text) }
            DBtn("Scan code", false, true) { scanPairing(ctx, "data") { DataClient.pair(it) } }
        }
    }
}

@Composable
internal fun DField(v: String, on: (String) -> Unit, hint: String, modifier: Modifier = Modifier, lines: Int = 1) {
    Box(modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(12.dp)) {
        if (v.isEmpty()) Text(hint, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 13.sp)
        BasicTextField(v, on, textStyle = TextStyle(color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp), cursorBrush = SolidColor(dcs.primary), maxLines = lines, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
internal fun DBtn(label: String, primary: Boolean, enabled: Boolean = true, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val bg = if (primary && enabled) dcs.primary else dcs.surfaceContainerHigh
    val fg = if (primary && enabled) dcs.onPrimary else if (enabled) dcs.onSurface else dcs.onSurfaceVariant
    Box(modifier.background(bg).border(BorderStroke(1.dp, dcs.outlineVariant)).clickable(enabled = enabled, onClick = onClick).padding(horizontal = 16.dp, vertical = 10.dp)) {
        Text(label, color = fg, fontFamily = BpMono, fontSize = 13.sp, fontWeight = FontWeight.Bold)
    }
}

// ---- home: the sidebar, projects with their threads
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HomeScreen(open: (String) -> Unit, settings: () -> Unit, addProject: () -> Unit, newThread: (String) -> Unit, openRoom: (DataClient.Room) -> Unit) {
    var searching by remember { mutableStateOf(false) }
    var q by remember { mutableStateOf("") }
    var projMenu by remember { mutableStateOf<DProject?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(DataClient.conn) { DataClient.refresh(); if (DataClient.conn == DataClient.Conn.ONLINE && DataClient.has("bots")) runCatching { DataClient.loadBots() } }
    Column(Modifier.fillMaxSize()) {
        DBar("Backplane", null) {
            if (DataClient.has("search")) IconButton(onClick = { searching = !searching; q = "" }, modifier = Modifier.testTag("home.search")) { Icon(Icons.Default.Search, "Search", tint = dcs.onSurface) }
            IconButton(onClick = addProject, modifier = Modifier.testTag("home.add")) { Icon(Icons.Default.CreateNewFolder, "Add project", tint = dcs.onSurface) }
            IconButton(onClick = settings, modifier = Modifier.testTag("home.settings")) { Icon(Icons.Default.Settings, "Settings", tint = dcs.onSurface) }
        }
        ConnBanner()
        if (searching) { Box(Modifier.padding(8.dp)) { DField(q, { q = it }, "Search projects and threads", Modifier.testTag("home.searchfield")) }; SearchResults(q, open); return@Column }
        val botThreads = DataClient.bots.map { it.thread }.toSet()
        val projects = DataClient.projects; val threads = DataClient.threads.filter { !it.archived && it.id !in botThreads }
        if (projects.isEmpty() && threads.isEmpty()) {
            Column(Modifier.fillMaxSize().padding(32.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
                Text(if (DataClient.listsLoaded) "No projects yet" else "Loading…", color = dcs.onSurface, fontSize = 16.sp)
                if (DataClient.listsLoaded) { Spacer(Modifier.height(12.dp)); DBtn("Add a project", true, true, onClick = addProject) }
            }
        } else LazyColumn(Modifier.fillMaxSize().testTag("home.list")) {
            for (p in projects) {
                val ts = threads.filter { it.project == p.id }
                item(key = "p:${p.id}") {
                    Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).combinedClickable(onClick = {}, onLongClick = { projMenu = p }).padding(start = 12.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        DMono(p.title, dcs.onSurface, 12, Modifier.weight(1f), bold = true)
                        if (p.waiting > 0) { DMono("${p.waiting}", damber, 11, bold = true); Spacer(Modifier.width(6.dp)) }
                        IconButton(onClick = { newThread(p.id) }, modifier = Modifier.size(32.dp)) { Icon(Icons.Default.Add, "New thread in ${p.title}", tint = dcs.primary, modifier = Modifier.size(18.dp)) }
                    }
                    DLine()
                }
                items(ts, key = { "t:${it.id}" }) { t -> ThreadRow(t) { open(t.id) } }
            }
            val known = projects.map { it.id }.toSet()
            items(threads.filter { it.project !in known }, key = { "t:${it.id}" }) { t -> ThreadRow(t) { open(t.id) } }
            if (DataClient.bots.isNotEmpty()) { item("bots") { DMono("BOTS", dcs.onSurfaceVariant, 11, Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(12.dp)); DLine() }
                items(DataClient.bots, key = { "b:${it.id}" }) { b -> SidebarRow("@", b.name, if (b.uber) "uberbot" else "bot") { if (b.thread.isNotEmpty()) open(b.thread) } } }
            if (DataClient.rooms.isNotEmpty()) { item("rooms") { DMono("ROOMS", dcs.onSurfaceVariant, 11, Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(12.dp)); DLine() }
                items(DataClient.rooms, key = { "r:${it.id}" }) { r -> SidebarRow("#", r.name, "room") { openRoom(r) } } }
        }
        projMenu?.let { ProjectMenus(it, true) { projMenu = null } }
    }
}

@Composable
internal fun ThreadRowPublic(t: DThread, onClick: () -> Unit) = ThreadRow(t, onClick)

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ThreadRow(t: DThread, onClick: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    ThreadMenus(t, menu, { menu = false })
    Column(Modifier.fillMaxWidth().combinedClickable(onClick = onClick, onLongClick = { menu = true })) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.Top) {
            Box(Modifier.padding(top = 5.dp)) { DDot(dStateColor(t.state)) }
            if (t.pinned) { Spacer(Modifier.width(4.dp)); DMono("^", dcs.primary, 11, bold = true) }
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(t.title.ifEmpty { "New thread" }, color = if (t.settled) dcs.onSurfaceVariant else dcs.onSurface, fontSize = 13.sp, fontFamily = BpMono, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (t.preview.isNotEmpty()) Text(t.preview.replace("**", "").replace("`", ""), color = dcs.onSurfaceVariant, fontSize = 11.sp, fontFamily = BpMono, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            Spacer(Modifier.width(8.dp))
            DMono(dAgo(t.updatedAt))
        }
        DLine()
    }
}

// ---- thread
@Composable
private fun ThreadScreen(id: String, viewer: () -> Unit, back: () -> Unit) {
    val scope = rememberCoroutineScope()
    DisposableEffect(id) { DataClient.openThread(id); onDispose { DataClient.closeThread(id) } }
    val thread = DataClient.threads.firstOrNull { it.id == id }
    val list = DataClient.entries[id] ?: emptyList()
    val state = rememberLazyListState()
    LaunchedEffect(list.size, list.lastOrNull()?.text?.length) { if (list.isNotEmpty()) state.scrollToItem(list.size - 1) }
    var draft by remember(id) { mutableStateOf(DataClient.draft(id)) }
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val pending = remember(id) { mutableStateListOf<ByteArray>() }
    var notice by remember(id) { mutableStateOf("") }
    val pick = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) scope.launch { val b = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { DataClient.prepared(ctx, uri) }; if (b != null) pending.add(b) else notice = "That picture could not be read." }
    }
    var menu by remember { mutableStateOf(false) }; var models by remember { mutableStateOf(false) }
    thread?.let { t -> ThreadMenus(t, menu, { menu = false }) { back() }; if (models) ModelPicker(t) { models = false } }
    Column(Modifier.fillMaxSize()) {
        DBar(thread?.title ?: "", back) {
            thread?.let { t ->
                if (DataClient.has("viewers")) IconButton(onClick = viewer, modifier = Modifier.testTag("thread.viewer")) { Icon(Icons.Default.Dashboard, "Viewer", tint = dcs.onSurface) }
                if (DataClient.has("actions")) IconButton(onClick = { menu = true }, modifier = Modifier.testTag("thread.more")) { Icon(Icons.Default.MoreVert, "More", tint = dcs.onSurface) }
            }
            thread?.let { DMono("STATE"); Spacer(Modifier.width(6.dp)); DDot(dStateColor(it.state)); Spacer(Modifier.width(6.dp)); DMono(if (it.state == "idle") "finished" else it.state, dcs.onSurface); Spacer(Modifier.width(12.dp)) }
        }
        ConnBanner()
        if (thread != null && DataClient.has("models") && thread.provider.isNotEmpty()) DMono("${thread.provider} / ${thread.model.ifEmpty { "default" }}", dcs.primary, 11, Modifier.clickable { models = true }.padding(horizontal = 12.dp, vertical = 6.dp).testTag("thread.model"))
        LazyColumn(Modifier.weight(1f).fillMaxWidth().testTag("thread.list"), state = state, contentPadding = PaddingValues(vertical = 8.dp)) {
            if (DataClient.older[id] != null) item("older") { Box(Modifier.fillMaxWidth().clickable { DataClient.loadOlder(id) }.padding(10.dp), contentAlignment = Alignment.Center) { DMono("load earlier") } }
            items(list, key = { it.id }) { EntryRow(id, it) }
        }
        val working = thread?.state == "working"
        DLine()
        MentionPopup(draft) { draft = it; DataClient.setDraft(id, it) }
        if (notice.isNotEmpty()) DMono(notice, damber, 12, Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 12.dp, vertical = 4.dp))
        if (pending.isNotEmpty()) Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(8.dp).horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            pending.toList().forEach { b -> Box(Modifier.testTag("thread.pending").clickable { pending.remove(b) }) { Pic(b, 64) }; }
        }
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(8.dp), verticalAlignment = Alignment.Bottom) {
            IconButton(onClick = { if (DataClient.canSendImages) pick.launch("image/*") else notice = "This hub does not take pictures yet (update it)." }, modifier = Modifier.testTag("thread.attach")) { Icon(Icons.Default.AttachFile, "Attach", tint = dcs.onSurfaceVariant) }
            Box(Modifier.weight(1f)) { DField(draft, { draft = it; DataClient.setDraft(id, it) }, "Ask anything: @bot, >thread, %project, \$skill, /btw", Modifier.testTag("thread.composer"), 6) }
            Spacer(Modifier.width(8.dp))
            if (working && draft.isBlank()) IconButton(onClick = { DataClient.interrupt(id) }, modifier = Modifier.testTag("thread.stop")) { Icon(Icons.Default.Stop, "Stop", tint = dcs.error) }
            else DBtn("Send", true, draft.isNotBlank() || pending.isNotEmpty(), Modifier.testTag("thread.send")) { DataClient.send(id, draft, pending.toList()); pending.clear(); draft = ""; notice = "" }
        }
    }
}

@Composable
private fun EntryRow(tid: String, e: DEntry) {
    when (e.kind) {
        "user" -> Column(Modifier.fillMaxWidth().padding(start = 48.dp, end = 12.dp, top = 6.dp, bottom = 6.dp), horizontalAlignment = Alignment.End) {
            e.localImages.forEach { Pic(it, 160); Spacer(Modifier.height(4.dp)) }
            e.attachments.filter { e.localImages.isEmpty() }.forEach { PicById(it.id, 160); Spacer(Modifier.height(4.dp)) }
            if (e.text.isNotEmpty()) Box(Modifier.background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.primary.copy(alpha = .5f))).padding(10.dp)) { Text(e.text, color = dcs.onSurface, fontSize = 13.sp, fontFamily = BpMono) }
            if (e.sending) DMono("sending…", modifier = Modifier.padding(top = 2.dp))
            if (e.failed) DMono("not sent · retry", dcs.error, modifier = Modifier.clickable { DataClient.retry(tid, e) }.padding(top = 2.dp))
        }
        "assistant" -> Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp)) {
            MdText(e.text)
            if (e.live) DMono("▍", dcs.primary)
        }
        "tool" -> Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
            DDot(when (e.tool?.state) { "running" -> dcs.primary; "failed" -> dcs.error; else -> dcs.outlineVariant }); Spacer(Modifier.width(8.dp))
            DMono(e.tool?.name ?: "tool", dcs.onSurface, 12, bold = true); Spacer(Modifier.width(8.dp))
            DMono(e.tool?.summary ?: e.text, modifier = Modifier.weight(1f))
        }
        "ask" -> AskCard(tid, e)
        else -> Text(e.text, Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), color = dcs.onSurfaceVariant, fontSize = 12.sp, fontFamily = BpMono)
    }
}

@Composable
private fun AskCard(tid: String, e: DEntry) {
    val a = e.ask
    Column(Modifier.fillMaxWidth().padding(12.dp).background(dcs.surfaceVariant).border(BorderStroke(1.dp, damber)).padding(12.dp).testTag("ask.card")) {
        DMono("ASKS", damber, 11, bold = true)
        Text(a?.question ?: e.text, color = dcs.onSurface, fontSize = 14.sp, modifier = Modifier.padding(vertical = 6.dp))
        if (a != null && a.open) Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.horizontalScroll(rememberScrollState())) {
            a.options.forEach { o -> DBtn(o.label, o.id == "accept", true, Modifier.testTag("ask.opt.${o.id}")) { DataClient.answer(tid, a.id, o.id) } }
        } else if (a?.answered != null) DMono("answered: ${a.answered}")
    }
}

// ---- settings, folders, new thread
@Composable
private fun SettingsScreen(back: () -> Unit, improve: () -> Unit) {
    Column(Modifier.fillMaxSize()) {
        DBar("Settings", back)
        Column(Modifier.padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            DMono("HUB"); Text(DataClient.hubName, color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp)
            DMono(if (DataClient.conn == DataClient.Conn.ONLINE) "online" else "offline ${DataClient.failure}", dcs.onSurfaceVariant, 12)
            DLine()
            DBtn("Improvements", false, modifier = Modifier.testTag("settings.improve"), onClick = improve)
            DLine()
            DMono("ALERTS")
            var alerts by remember { mutableStateOf(AppSettings.alertsOn()) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { listOf(true to "on", false to "off").forEach { (v, l) -> DBtn(l, alerts == v, modifier = Modifier.testTag("settings.alerts.$l")) { alerts = v; AppSettings.putAlerts(v) } } }
            DMono("a turn that ends or asks, while the app is not on screen", dcs.onSurfaceVariant, 11)
            DLine()
            DMono("APPEARANCE")
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("system", "light", "dark").forEach { m -> DBtn(m, AppSettings.appearance == m) { AppSettings.putAppearance(m) } }
            }
            DLine()
            DBtn("Unpair this phone", false, modifier = Modifier.testTag("settings.unpair")) { DataClient.unpair(); back() }
        }
    }
}

@Composable
private fun FolderScreen(back: () -> Unit, added: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var path by remember { mutableStateOf<String?>(null) }
    var cur by remember { mutableStateOf("") }
    var parent by remember { mutableStateOf<String?>(null) }
    var dirs by remember { mutableStateOf<List<String>>(emptyList()) }
    var err by remember { mutableStateOf("") }
    LaunchedEffect(path) { try { val (p, par, d) = DataClient.dirs(path); cur = p; parent = par; dirs = d; err = "" } catch (e: Exception) { err = e.message ?: "" } }
    Column(Modifier.fillMaxSize()) {
        DBar("Add a project", back)
        DMono(cur, dcs.onSurface, 12, Modifier.padding(12.dp))
        if (err.isNotEmpty()) Text(err, color = dcs.error, modifier = Modifier.padding(horizontal = 12.dp))
        LazyColumn(Modifier.weight(1f)) {
            if (parent != null) item { FolderRow("..") { path = parent } }
            items(dirs) { d -> FolderRow(d) { path = cur.trimEnd('/') + "/" + d } }
        }
        DLine()
        Box(Modifier.padding(12.dp)) { DBtn("Use this folder", true, cur.isNotEmpty(), Modifier.testTag("folders.use")) { scope.launch { try { added(DataClient.addProject(cur).id) } catch (e: Exception) { err = e.message ?: "" } } } }
    }
}
@Composable private fun FolderRow(name: String, onClick: () -> Unit) {
    Column(Modifier.clickable(onClick = onClick)) {
        Row(Modifier.fillMaxWidth().padding(12.dp), verticalAlignment = Alignment.CenterVertically) { Icon(Icons.Default.Folder, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)); Spacer(Modifier.width(10.dp)); Text(name, color = dcs.onSurface, fontSize = 14.sp) }
        DLine()
    }
}

@Composable
private fun NewThreadScreen(project: String, back: () -> Unit, created: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var text by remember { mutableStateOf("") }
    var err by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val title = DataClient.projects.firstOrNull { it.id == project }?.title ?: ""
    Column(Modifier.fillMaxSize()) {
        DBar("New thread · $title", back)
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            DField(text, { text = it }, "What should it do?", Modifier.testTag("new.text"), 8)
            if (err.isNotEmpty()) Text(err, color = dcs.error, fontSize = 13.sp)
            DBtn(if (busy) "Starting…" else "Start", true, !busy, Modifier.testTag("new.start")) {
                busy = true; scope.launch { try { created(DataClient.createThread(project, text).id) } catch (e: Exception) { err = e.message ?: ""; busy = false } }
            }
        }
    }
}

@Composable private fun Pic(b: ByteArray, size: Int) {
    val bmp = remember(b) { android.graphics.BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap() }
    if (bmp != null) androidx.compose.foundation.Image(bmp, null, Modifier.heightIn(max = size.dp).widthIn(max = (size * 1.5).toInt().dp).border(BorderStroke(1.dp, dcs.outlineVariant)), contentScale = androidx.compose.ui.layout.ContentScale.Fit)
}
@Composable private fun PicById(id: String, size: Int) {
    var b by remember(id) { mutableStateOf<ByteArray?>(null) }
    LaunchedEffect(id) { b = DataClient.picture(id) }
    b?.let { Pic(it, size) } ?: Box(Modifier.size(size.dp, (size / 2).dp).background(dcs.surfaceVariant))
}

// ---- improvements (the same as iOS's): a GitHub issue draft opens in the browser, or the text starts a thread on the hub
@Composable
private fun ImproveScreen(back: () -> Unit) {
    val scope = rememberCoroutineScope(); val ctx = androidx.compose.ui.platform.LocalContext.current
    var text by remember { mutableStateOf("") }; var sent by remember { mutableStateOf("") }
    var project by remember { mutableStateOf(DataClient.projects.firstOrNull()?.id ?: "") }
    Column(Modifier.fillMaxSize()) {
        DBar("Improvements", back)
        Column(Modifier.padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            if (sent.isNotEmpty()) DMono(sent, dcs.primary, 12, Modifier.testTag("improve.sent"))
            DMono("WHAT WOULD YOU LIKE TO IMPROVE?")
            DField(text, { text = it }, "", Modifier.testTag("improve.text"), 8)
            Text("A public GitHub issue draft on BackplaneWorks/Backplane opens in your browser, filled with this text. You review it and press Submit there; nothing is sent from here. Do not include private information.", color = dcs.onSurfaceVariant, fontSize = 12.sp)
            DBtn("Open a GitHub issue draft", false, text.isNotBlank(), Modifier.testTag("improve.github")) {
                val title = text.lineSequence().firstOrNull().orEmpty().take(80)
                val u = android.net.Uri.parse("https://github.com/BackplaneWorks/Backplane/issues/new").buildUpon().appendQueryParameter("title", title)
                    .appendQueryParameter("body", text + "\n\n---\nBackplane Android ${BuildConfig.VERSION_NAME}, Android ${android.os.Build.VERSION.RELEASE}").build()
                ctx.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, u))
            }
            if (DataClient.projects.isNotEmpty()) {
                DLine(); DMono("PROJECT")
                DataClient.projects.forEach { p -> DMono((if (p.id == project) "● " else "○ ") + p.title, dcs.onSurface, 13, Modifier.clickable { project = p.id }.padding(vertical = 4.dp)) }
                DBtn("Send to an agent thread", true, text.isNotBlank() && DataClient.conn == DataClient.Conn.ONLINE && project.isNotEmpty(), Modifier.testTag("improve.thread")) {
                    scope.launch { runCatching { DataClient.createThread(project, "Improvement idea for this app:\n\n$text") }.onSuccess { sent = "Thread started in Threads."; text = "" }.onFailure { sent = it.message ?: "" } }
                }
            }
        }
    }
}
