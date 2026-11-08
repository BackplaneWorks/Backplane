package dev.backplane.mobile

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloat
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
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

// Screens for the data/1 app, in the desktop client's language (native/gpui): flat panels on the same tokens, square corners, mono labels, a state dot per thread.
internal val dcs @Composable get() = MaterialTheme.colorScheme
private val dlight @Composable get() = dcs.background.luminance() > .5f
internal val dok @Composable get() = Color(if (dlight) 0xFF1C8550 else 0xFF58C28A)
internal val dwarn @Composable get() = Color(if (dlight) 0xFF9A6700 else 0xFFE3B341)
internal val dfaint @Composable get() = Color(if (dlight) 0xFF72777F else 0xFF5C6473)
internal val dview @Composable get() = Color(if (dlight) 0xFFF5F4EF else 0xFF08090B)
// a thread's activity, one colour each (native/gpui/src/ui.rs `side_thread`): green working; red wanting the person or failed; grey otherwise
@Composable internal fun dStateColor(s: String) = when (s) { "working" -> dok; "asking", "failed" -> dcs.error; else -> dfaint }
internal fun dStateWord(s: String) = when (s) { "working" -> "working"; "asking" -> "needs you"; "failed" -> "failed"; else -> "inactive" }
private val dClock = java.text.SimpleDateFormat("HH:mm", java.util.Locale.getDefault())
private val dDay = java.text.SimpleDateFormat("EEE d MMM HH:mm", java.util.Locale.getDefault())
/** A message's time: the clock today, the day too on another. */
internal fun dStamp(ms: Long): String {
    if (ms <= 0) return ""
    val d = java.util.Date(ms); return (if (android.text.format.DateUtils.isToday(ms)) dClock else dDay).format(d)
}
internal fun dAgo(ms: Long): String {
    val s = (System.currentTimeMillis() - ms) / 1000
    return when { ms <= 0 -> ""; s < 60 -> "now"; s < 3600 -> "${s / 60}m"; s < 86400 -> "${s / 3600}h"; else -> "${s / 86400}d" }
}
@Composable internal fun DMono(t: String, c: Color = dcs.onSurfaceVariant, size: Int = 11, modifier: Modifier = Modifier, bold: Boolean = false, lines: Int = 1) =
    Text(t, modifier, color = c, fontFamily = BpMono, fontSize = size.sp, fontWeight = if (bold) FontWeight.Bold else FontWeight.Normal, maxLines = lines, overflow = TextOverflow.Ellipsis)
@Composable internal fun DDot(c: Color, size: Int = 6) = Box(Modifier.size(size.dp).background(c))
@Composable internal fun DLine() = Box(Modifier.fillMaxWidth().height(1.dp).background(dcs.outlineVariant))

private sealed interface Page { object Improve : Page; data class Room(val room: DataClient.Room) : Page; data class Viewer(val id: String, val file: String = "", val line: Int = 0) : Page; object Home : Page; data class Hub(val from: Page) : Page; data class Thread(val id: String) : Page; object Settings : Page; object Folders : Page; data class New(val project: String) : Page; object Accounts : Page; data class Term(val key: String, val from: Page) : Page; data class Terms(val project: String?, val from: Page) : Page; data class Account(val id: String, val start: Boolean, val n: Long = System.nanoTime()) : Page }

@Composable
fun DataRoot() {
    var page by remember { mutableStateOf<Page>(DataClient.openFromIntent?.let { if (DataClient.openViewerTab != null) Page.Viewer(it) else Page.Thread(it) } ?: Page.Home) }
    // an expired sign-in's alert: that account, its sign-in started
    LaunchedEffect(DataClient.openAccount) { DataClient.openAccount?.let { page = Page.Account(it, true); DataClient.openAccount = null } }
    LaunchedEffect(DataClient.openFromIntent) { DataClient.openFromIntent?.let { page = if (DataClient.openViewerTab != null) Page.Viewer(it) else Page.Thread(it); DataClient.openFromIntent = null } }
    // a link the hub resolved (DataLinks.kt): its screen
    LaunchedEffect(DataClient.linkTarget) {
        val t = DataClient.linkTarget ?: return@LaunchedEffect
        DataClient.linkTarget = null
        when (t.kind) {
            LinkTarget.Kind.THREAD -> page = Page.Thread(t.id)
            LinkTarget.Kind.FILE -> page = Page.Viewer(t.id, t.path, t.line)
            LinkTarget.Kind.PROJECT -> { DataClient.focusProject = t.id; page = Page.Home }
            LinkTarget.Kind.ROOM -> {
                if (DataClient.rooms.none { it.id == t.id }) runCatching { DataClient.loadBots() }
                DataClient.rooms.firstOrNull { it.id == t.id }?.let { page = Page.Room(it) } ?: DataClient.say("That room is not on this hub.")
            }
        }
    }
    Box(Modifier.fillMaxSize().background(dcs.background).windowInsetsPadding(WindowInsets.systemBars.union(WindowInsets.ime))) {
        if (DataClient.link == null || DataClient.needsRepair || DataClient.adding) { if (DataClient.adding && DataClient.link != null) BackHandler { DataClient.adding = false }; PairScreen() }
        else CompositionLocalProvider(LocalTermOpen provides { k: String -> page = Page.Term(k, page.let { if (it is Page.Term) it.from else it }) }, LocalTermsOpen provides { pr: String? -> page = Page.Terms(pr, page) }) { when (val p = page) {
            Page.Home -> HomeScreen({ page = Page.Thread(it) }, { page = Page.Settings }, { page = Page.Hub(Page.Home) }, { page = Page.Folders }, { page = Page.New(it) }) { page = Page.Room(it) }
            is Page.Term -> { BackHandler { page = p.from }; TermFull(p.key, "Terminal") { page = p.from } }
            is Page.Terms -> { BackHandler { page = p.from }; TermsScreen(p.project, { page = p.from }) { page = Page.Term(it, p) } }
            is Page.Viewer -> { BackHandler { page = Page.Thread(p.id) }; DataClient.threads.firstOrNull { it.id == p.id }?.let { ViewerScreen(it, p.file, p.line) { page = Page.Thread(p.id) } }
                ?: Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() } } // never a blank page while the thread list comes
            is Page.Room -> { BackHandler { page = Page.Home }; RoomScreen(p.room) { page = Page.Home } }
            is Page.Thread -> { BackHandler { page = Page.Home }; ThreadScreen(p.id, { page = Page.Viewer(p.id) }, { page = Page.Hub(p) }) { page = Page.Home } }
            Page.Settings -> { BackHandler { page = Page.Home }; SettingsScreen({ page = Page.Home }, { page = Page.Hub(Page.Settings) }, { page = Page.Accounts }) { page = Page.Improve } }
            Page.Accounts -> { BackHandler { page = Page.Settings }; AccountsScreen({ page = Page.Settings }) { page = Page.Account(it, false) } }
            is Page.Account -> { BackHandler { page = Page.Accounts }; key(p) { AccountScreen(p.id, p.start) { page = Page.Accounts } } } // each opening its own screen: an alert's tap reads the account afresh and starts the sign-in
            is Page.Hub -> { BackHandler { page = p.from }; HubScreen { page = p.from } }
            Page.Improve -> { BackHandler { page = Page.Settings }; ImproveScreen { page = Page.Settings } }
            Page.Folders -> { BackHandler { page = Page.Home }; FolderScreen({ page = Page.Home }) { page = Page.New(it) } }
            is Page.New -> { BackHandler { page = Page.Home }; NewThreadScreen(p.project, { page = Page.Home }) { page = Page.Thread(it) } }
        } }
        DataClient.askLink?.let { AskPair(it) }
        LinkNote()
    }
}

@Composable
internal fun DBar(title: String, back: (() -> Unit)?, titleMenu: (@Composable ColumnScope.(dismiss: () -> Unit) -> Unit)? = null, trailing: @Composable RowScope.() -> Unit = {}) {
    Column(Modifier.background(dcs.surfaceVariant)) {
        Row(Modifier.fillMaxWidth().height(48.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            if (back != null) IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back", tint = dcs.onSurfaceVariant) } else { Spacer(Modifier.width(10.dp)); BrandMark(DataClient.conn == DataClient.Conn.ONLINE); Spacer(Modifier.width(8.dp)) }
            if (titleMenu == null) Text(title.ifEmpty { "New thread" }, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            else {
                var open by remember { mutableStateOf(false) }
                Box(Modifier.weight(1f)) {
                    Row(Modifier.clickable { open = true }.padding(vertical = 12.dp).testTag("home.hub"), verticalAlignment = Alignment.CenterVertically) {
                        Text(title, color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                        Spacer(Modifier.width(4.dp)); Icon(Icons.Default.ArrowDropDown, null, tint = dcs.onSurfaceVariant)
                    }
                    DropdownMenu(open, { open = false }) { titleMenu { open = false } }
                }
            }
            trailing()
        }
        DLine()
    }
}

@Composable
private fun ConnBanner(hub: () -> Unit) {
    MigrateBanner()
    HubBanner(hub)
    StarBanner()
    if (DataClient.conn == DataClient.Conn.ONLINE) return
    Row(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        DDot(if (DataClient.conn == DataClient.Conn.CONNECTING) dwarn else dcs.error, 8); Spacer(Modifier.width(8.dp))
        DMono(if (DataClient.conn == DataClient.Conn.CONNECTING) "connecting to ${DataClient.hubName}…" else "offline · showing what this phone has saved", dcs.onSurface, 12, Modifier.weight(1f))
        Spacer(Modifier.width(8.dp))
        DMono("Hub", dcs.primary, 12, Modifier.testTag("conn.hub").clickable(onClick = hub).padding(horizontal = 4.dp, vertical = 4.dp), bold = true)
    }
}

@Composable
private fun PairScreen() {
    var text by remember(DataClient.pairEpoch) { mutableStateOf("") }
    var address by remember(DataClient.pairEpoch) { mutableStateOf("") }
    var code by remember(DataClient.pairEpoch) { mutableStateOf("") }
    val ctx = androidx.compose.ui.platform.LocalContext.current
    // centred when it fits; scrolls when it does not (a phone in landscape, a large font, the keyboard up): the short code and its button stay reachable
    Box(Modifier.fillMaxSize().imePadding(), contentAlignment = Alignment.Center) {
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(24.dp)) {
        DMono("BACKPLANE", dcs.primary, 13, bold = true)
        Spacer(Modifier.height(8.dp))
        Text(if (DataClient.needsRepair) "Re-pair this device" else if (DataClient.adding) "Add a hub" else "Pair with a hub", color = dcs.onSurface, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(4.dp))
        Text(if (DataClient.needsRepair) "This device was revoked. Make a new link in Settings → Devices on the hub." else "Paste the hub's pairing link or its address.", color = dcs.onSurfaceVariant, fontSize = 14.sp)
        Spacer(Modifier.height(20.dp))
        DField(text, { text = it }, "http://host:3787/pair#invite=…", Modifier.testTag("pair.field"))
        if (DataClient.pairingError.isNotEmpty()) { Spacer(Modifier.height(8.dp)); Text(DataClient.pairingError, color = dcs.onSurfaceVariant, fontSize = 13.sp) }
        Spacer(Modifier.height(16.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DBtn("Pair", true, text.isNotBlank() && !DataClient.pairingBusy, Modifier.testTag("pair.go")) { DataClient.pair(text) }
            DBtn("Scan code", false, true) { scanPairing(ctx, "data") { DataClient.pair(it) } }
            if (DataClient.adding && DataClient.link != null) DBtn("Cancel", false, true, Modifier.testTag("pair.cancel")) { DataClient.adding = false; DataClient.pairingError = "" }
        }
        // revoked: the other paired hubs are one tap away
        val others = DataClient.links.drop(1).mapNotNull { Pairing.key(it) }
        if (DataClient.needsRepair && others.isNotEmpty()) {
            Spacer(Modifier.height(16.dp)); Text("Or use another paired hub", color = dcs.onSurfaceVariant, fontSize = 13.sp); Spacer(Modifier.height(8.dp))
            others.forEach { k -> DBtn(k, false, modifier = Modifier.testTag("pair.use.$k")) { DataClient.useHub(k) } }
        }
        Spacer(Modifier.height(24.dp))
        Text("Or the hub's address and short code", color = dcs.onSurfaceVariant, fontSize = 13.sp)
        Spacer(Modifier.height(8.dp))
        DField(address, { address = it }, "host:port", Modifier.testTag("pair.address"))
        Spacer(Modifier.height(8.dp))
        DField(code, { code = it }, "ABCD-2345", Modifier.testTag("pair.code"))
        Spacer(Modifier.height(16.dp))
        DBtn("Pair with code", false, address.isNotBlank() && code.isNotBlank() && !DataClient.pairingBusy, Modifier.testTag("pair.code.go")) {
            val l = Pairing.codeLink(address.trim(), code)
            if (l == null) DataClient.pairingError = "Check the hub address and the eight-character code." else DataClient.pair(l)
        }
    }
    }
}

/** A link from outside the app names the hub it would pair with; Cancel comes first. */
@Composable
private fun AskPair(link: String) {
    val hub = Pairing.invitation(link)?.origin ?: Pairing.key(link)
    if (hub == null) { LaunchedEffect(link) { DataClient.askLink = null; DataClient.pairingError = "That is not a pairing link." }; return }
    Column(Modifier.fillMaxSize().background(dcs.background).padding(24.dp), verticalArrangement = Arrangement.Center) {
        Text("Pair with this hub?", color = dcs.onSurface, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(8.dp))
        DMono(hub, dcs.onSurface, 14)
        Spacer(Modifier.height(8.dp))
        Text("A link opened this. Pair only with a hub you know.", color = dcs.onSurfaceVariant, fontSize = 14.sp)
        Spacer(Modifier.height(16.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DBtn("Cancel", true, true, Modifier.testTag("pair.ask.cancel")) { DataClient.askLink = null }
            DBtn("Pair", false, true, Modifier.testTag("pair.ask.yes")) { DataClient.askLink = null; DataClient.pair(link) }
        }
    }
}

@Composable
internal fun DField(v: String, on: (String) -> Unit, hint: String, modifier: Modifier = Modifier, lines: Int = 1, line: Color = dcs.outlineVariant) {
    Box(modifier.fillMaxWidth().background(dview).border(BorderStroke(1.dp, line)).padding(horizontal = 10.dp, vertical = 10.dp)) {
        if (v.isEmpty()) Text(hint, color = dfaint, fontFamily = BpMono, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        BasicTextField(v, on, textStyle = TextStyle(color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp), cursorBrush = SolidColor(dcs.primary), maxLines = lines, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
internal fun DField(v: TextFieldValue, on: (TextFieldValue) -> Unit, hint: String, modifier: Modifier = Modifier, lines: Int = 1, line: Color = dcs.outlineVariant) {
    Box(modifier.fillMaxWidth().background(dview).border(BorderStroke(1.dp, line)).padding(horizontal = 10.dp, vertical = 10.dp)) {
        if (v.text.isEmpty()) Text(hint, color = dfaint, fontFamily = BpMono, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        BasicTextField(v, on, textStyle = TextStyle(color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp), cursorBrush = SolidColor(dcs.primary), maxLines = lines, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
internal fun DBtn(label: String, primary: Boolean, enabled: Boolean = true, modifier: Modifier = Modifier, danger: Boolean = false, onClick: () -> Unit) {
    val bg = if (primary && enabled) dcs.primary else dcs.surfaceContainerHigh
    val fg = if (primary && enabled) dcs.onPrimary else if (!enabled) dfaint else if (danger) dcs.error else dcs.onSurface
    Box(modifier.background(bg).border(BorderStroke(1.dp, if (primary && enabled) dcs.primary else dcs.outlineVariant)).clickable(enabled = enabled, onClick = onClick).heightIn(min = 40.dp).padding(horizontal = 14.dp), contentAlignment = Alignment.CenterStart) {
        Text(label, color = fg, fontFamily = BpMono, fontSize = 13.sp, fontWeight = FontWeight.Bold, maxLines = 1)
    }
}

/** The composer's model chip: the hub's "claude / opus / high" in a line box; a tap opens the picker. */
@Composable
internal fun ModelChip(text: String, tag: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Box(modifier.height(40.dp).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).clickable(onClick = onClick).padding(horizontal = 10.dp).testTag(tag), contentAlignment = Alignment.CenterStart) {
        DMono(text, dcs.onSurface, 12)
    }
}

// ---- home: the sidebar, projects with their threads
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HomeScreen(open: (String) -> Unit, settings: () -> Unit, hub: () -> Unit, addProject: () -> Unit, newThread: (String) -> Unit, openRoom: (DataClient.Room) -> Unit) {
    var q by remember { mutableStateOf("") }
    var projMenu by remember { mutableStateOf<DProject?>(null) }
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val prefs = remember { ctx.getSharedPreferences("data.ui", android.content.Context.MODE_PRIVATE) }
    var shut by remember { mutableStateOf(prefs.getStringSet("projects.shut", emptySet())!!.toSet()) }
    val termsOpen = LocalTermsOpen.current
    LaunchedEffect(DataClient.conn) { DataClient.refresh(); if (DataClient.conn == DataClient.Conn.ONLINE && DataClient.has("bots")) runCatching { DataClient.loadBots() } }
    val listState = androidx.compose.foundation.lazy.rememberLazyListState()
    var flash by remember { mutableStateOf("") }
    val rowOf = remember { mutableMapOf<String, Int>() }   // a project's header row in the list
    // a project link: its group opens, the list scrolls to it and its row is marked for a moment
    LaunchedEffect(DataClient.focusProject, DataClient.projects.size) {
        val id = DataClient.focusProject; if (id.isEmpty() || DataClient.projects.none { it.id == id }) return@LaunchedEffect
        shut = shut - id; prefs.edit().putStringSet("projects.shut", shut).apply(); flash = id; DataClient.focusProject = ""
        kotlinx.coroutines.delay(150)
        rowOf[id]?.let { listState.animateScrollToItem(it) }
        kotlinx.coroutines.delay(2500); if (flash == id) flash = ""
    }
    Column(Modifier.fillMaxSize().background(dcs.surfaceVariant)) {
        DBar(DataClient.hubName.ifEmpty { "Backplane" }, null, { dismiss ->
            DataClient.links.drop(1).mapNotNull { Pairing.key(it) }.forEach { k -> DropdownMenuItem({ Text(k, fontFamily = BpMono) }, { dismiss(); DataClient.useHub(k) }, Modifier.testTag("home.hub.use.$k")) }
            DropdownMenuItem({ Text("Add a hub", fontFamily = BpMono) }, { dismiss(); DataClient.pairingError = ""; DataClient.adding = true }, Modifier.testTag("home.hub.add"))
            if (termsOn()) DropdownMenuItem({ Text("Terminals", fontFamily = BpMono) }, { dismiss(); termsOpen(null) }, Modifier.testTag("home.hub.terms"))
        }) {
            IconButton(onClick = settings, modifier = Modifier.testTag("home.settings")) { Icon(Icons.Default.Settings, "Settings", tint = dcs.onSurfaceVariant) }
        }
        ConnBanner(hub)
        Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 4.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (DataClient.has("search")) Box(Modifier.weight(1f)) { DField(q, { q = it }, "Search projects and threads", Modifier.testTag("home.searchfield")) } else Spacer(Modifier.weight(1f))
            IconButton(onClick = addProject, modifier = Modifier.testTag("home.add")) { Icon(Icons.Default.Add, "Add project", tint = dcs.onSurfaceVariant) }
        }
        DLine()
        if (DataClient.locks.isNotEmpty()) LocksStrip(open)
        if (q.isNotBlank() && DataClient.has("search")) { SearchResults(q, open); return@Column }
        val botThreads = DataClient.bots.map { it.thread }.toSet()
        val projects = DataClient.projects; val threads = DataClient.threads.filter { !it.archived && it.id !in botThreads && !it.history }
        val hist = DataClient.history
        if (projects.isEmpty() && threads.isEmpty() && hist.count == 0) {
            Column(Modifier.fillMaxSize().padding(32.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
                Text(if (DataClient.listsLoaded) "No projects yet" else "Loading…", color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp)
                if (DataClient.listsLoaded) { Spacer(Modifier.height(12.dp)); DBtn("Add a project", true, true, onClick = addProject) }
            }
        } else LazyColumn(Modifier.fillMaxSize().testTag("home.list"), state = listState) {
            val uber = DataClient.bots.filter { it.uber && it.thread.isNotEmpty() }
            items(uber, key = { "u:${it.id}" }) { b -> SidebarRow("@", b.name, "uberbot") { open(b.thread) } }
            var at = uber.size; rowOf.clear()
            for (p in projects) {
                val closed = p.id in shut
                rowOf[p.id] = at; at += 1
                if (!closed) { val g0 = ThreadGroups.split(threads, p.id); val n0 = ThreadGroups.count(p.settledCount, g0.settled.size); at += g0.active.size + (if (g0.active.isEmpty() && n0 == 0) 1 else 0) + (if (n0 > 0) 1 else 0)
                    if (n0 > 0 && p.id in DataClient.settledOpen) at += g0.settled.size + (if (p.id in DataClient.settledMore) 1 else 0) }
                item(key = "p:${p.id}") {
                    Row(Modifier.fillMaxWidth().height(40.dp).background(if (flash == p.id) dcs.primary.copy(alpha = .18f) else Color.Transparent).combinedClickable(onClick = {
                        shut = if (closed) shut - p.id else shut + p.id; prefs.edit().putStringSet("projects.shut", shut).apply()
                    }, onLongClick = { projMenu = p }).padding(start = 10.dp, end = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(if (closed) Icons.Default.ChevronRight else Icons.Default.ExpandMore, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(6.dp))
                        Icon(Icons.Default.FolderOpen, null, tint = dok, modifier = Modifier.size(15.dp)); Spacer(Modifier.width(8.dp))
                        Box(Modifier.size(8.dp).background(runCatching { androidx.compose.ui.graphics.Color(android.graphics.Color.parseColor(p.css)) }.getOrDefault(dcs.onSurfaceVariant)).testTag("proj.badge.${p.id}"))
                        Spacer(Modifier.width(6.dp))
                        DMono(p.title.uppercase(), dcs.onSurfaceVariant, 12, Modifier.weight(1f))
                        if (p.waiting > 0) { DMono("${p.waiting}", dcs.error, 11, bold = true); Spacer(Modifier.width(6.dp)) }
                        IconButton(onClick = { newThread(p.id) }, modifier = Modifier.size(40.dp)) { Icon(Icons.Default.Add, "New thread in ${p.title}", tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)) }
                    }
                }
                if (!closed) {
                    val g = ThreadGroups.split(threads, p.id)
                    val n = ThreadGroups.count(p.settledCount, g.settled.size)
                    items(g.active, key = { "t:${it.id}" }) { t -> ThreadRow(t) { open(t.id) } }
                    if (g.active.isEmpty() && n == 0) item("none:${p.id}") { DMono("No threads", dfaint, 12, Modifier.padding(start = 34.dp, top = 6.dp, bottom = 8.dp)) }
                    if (n > 0) {
                        val on = p.id in DataClient.settledOpen
                        item("settled:${p.id}") {
                            Row(Modifier.fillMaxWidth().height(32.dp).clickable { DataClient.setSettledOpen(p.id, !on) }.padding(start = 28.dp).testTag("home.settled.${p.id}"), verticalAlignment = Alignment.CenterVertically) {
                                Icon(if (on) Icons.Default.ExpandMore else Icons.Default.ChevronRight, null, tint = dfaint, modifier = Modifier.size(14.dp)); Spacer(Modifier.width(6.dp)); DMono(ThreadGroups.title(n), dfaint, 11)
                            }
                        }
                        if (on) {
                            items(g.settled, key = { "t:${it.id}" }) { t -> ThreadRow(t) { open(t.id) } }
                            if (p.id in DataClient.settledMore) item("settled.more:${p.id}") {
                                val more = rememberCoroutineScope()
                                Row(Modifier.fillMaxWidth().height(32.dp).clickable { more.launch { DataClient.loadSettled(p.id, more = true) } }.padding(start = 34.dp).testTag("home.settled.more.${p.id}"), verticalAlignment = Alignment.CenterVertically) { DMono("More", dfaint, 11) }
                            }
                        }
                    }
                }
            }
            val known = projects.map { it.id }.toSet()
            items(threads.filter { it.project !in known }, key = { "t:${it.id}" }) { t -> ThreadRow(t) { open(t.id) } }
            // Conversation history: the old Scratch chats. Its row opens or shuts the list; a chat that wants attention always has its row; nothing starts here
            if (hist.count > 0) {
                item("history") {
                    Row(Modifier.fillMaxWidth().height(40.dp).clickable { DataClient.showHistory(!DataClient.historyOpen) }.padding(start = 10.dp).testTag("home.history"), verticalAlignment = Alignment.CenterVertically) {
                        Icon(if (DataClient.historyOpen) Icons.Default.ExpandMore else Icons.Default.ChevronRight, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(6.dp))
                        DMono(hist.title.uppercase(), dcs.onSurfaceVariant, 12, Modifier.weight(1f))
                    }
                }
                items(hist.shown(DataClient.historyOpen), key = { "h:${it.id}" }) { t -> ThreadRow(t) { open(t.id) } }
                if (DataClient.historyOpen && hist.next != null) item("history.more") {
                    val more = rememberCoroutineScope()
                    Row(Modifier.fillMaxWidth().height(32.dp).clickable { more.launch { DataClient.loadHistory(more = true) } }.padding(start = 34.dp).testTag("home.history.more"), verticalAlignment = Alignment.CenterVertically) { DMono("More", dfaint, 11) }
                }
            }
            val bots = DataClient.bots.filter { !it.uber }
            if (bots.isNotEmpty()) { item("bots") { DMono("BOTS", dfaint, 11, Modifier.fillMaxWidth().padding(start = 12.dp, top = 12.dp, bottom = 4.dp)) }
                items(bots, key = { "b:${it.id}" }) { b -> SidebarRow("@", b.name, "bot") { if (b.thread.isNotEmpty()) open(b.thread) } } }
            if (DataClient.rooms.isNotEmpty()) { item("rooms") { DMono("ROOMS", dfaint, 11, Modifier.fillMaxWidth().padding(start = 12.dp, top = 12.dp, bottom = 4.dp)) }
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
    Row(Modifier.fillMaxWidth().combinedClickable(onClick = onClick, onLongClick = { menu = true }).padding(start = 30.dp, end = 12.dp, top = 8.dp, bottom = 8.dp).alpha(if (t.settled) .55f else 1f), verticalAlignment = Alignment.Top) {
        Box(Modifier.padding(top = 6.dp)) { DDot(dStateColor(t.state)) }
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (t.pinned) { Icon(Icons.Default.PushPin, "Pinned", tint = dcs.primary, modifier = Modifier.size(11.dp)); Spacer(Modifier.width(5.dp)) }
                Text(t.title.ifEmpty { "New thread" }, color = dcs.onSurface, fontSize = 13.sp, fontFamily = BpMono, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            if (t.preview.isNotEmpty()) Text(t.preview.replace("**", "").replace("`", ""), color = dfaint, fontSize = 11.sp, fontFamily = BpMono, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Spacer(Modifier.width(8.dp))
        if (t.waiting > 0) { DMono("${t.waiting}", dcs.error, 11, bold = true); Spacer(Modifier.width(6.dp)) }
        DMono(dAgo(t.updatedAt), dfaint)
    }
}

// ---- thread
// The thread's background work: a head that folds, its rows, and the Release, Cancel or Stop a row offers. Open while any is at work.
@Composable
private fun WorkPanel(id: String, w: DWork) {
    var chosen by remember(id) { mutableStateOf<Boolean?>(null) }
    val open = chosen ?: (w.live > 0)
    Column(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp).testTag("thread.work")) {
        DMono("BACKGROUND WORK · ${w.word.uppercase()}", dcs.onSurfaceVariant, 11, Modifier.fillMaxWidth().clickable { chosen = !open }.testTag("thread.work.head"))
        if (open) w.rows.forEach { r ->
            Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) { DMono(r.title, dcs.onSurface, 12); DMono(r.doing.ifEmpty { "${r.kind} · ${r.state}" }, dfaint, 11) }
                if (r.label.isNotEmpty()) DBtn(r.label, false, modifier = Modifier.testTag("thread.work.${r.act}.${r.value}")) { DataClient.workAct(id, r) }
            }
        }
    }
}

// Who holds each lock, on the home list; a tap opens the holder's thread.
@Composable
private fun LocksStrip(open: (String) -> Unit) {
    Column(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp).testTag("home.locks")) {
        DMono("LOCKS", dcs.onSurfaceVariant, 11)
        DataClient.locks.forEach { l ->
            Row(Modifier.fillMaxWidth().clickable(enabled = l.thread.isNotEmpty()) { open(l.thread) }.padding(top = 3.dp).testTag("home.lock.${l.name}"), verticalAlignment = Alignment.CenterVertically) {
                DMono(l.name, dcs.onSurface, 12); Spacer(Modifier.width(8.dp)); DMono(l.line, dfaint, 11, Modifier.weight(1f)); if (l.waiting > 0) DMono("${l.waiting} waiting", dfaint, 11)
            }
        }
    }
}

@Composable
private fun StateStrip(t: DThread) {
    Column(Modifier.background(dcs.surfaceVariant)) {
        Row(Modifier.fillMaxWidth().height(32.dp).padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            DMono("STATE", dfaint); Spacer(Modifier.width(8.dp)); DDot(dStateColor(t.state)); Spacer(Modifier.width(6.dp)); DMono(dStateWord(t.state), dcs.onSurface, modifier = Modifier.testTag("thread.state"))
            Spacer(Modifier.weight(1f))
            if (DataClient.has("actions")) {
                StripCell(if (t.pinned) "Unpin" else "Pin", t.pinned) { DataClient.threadAct(t.id, if (t.pinned) "unpin" else "pin") }
                StripCell(if (t.settled) "Unsettle" else "Settle", t.settled) { DataClient.threadAct(t.id, if (t.settled) "unsettle" else "settle") }
            }
        }
        DLine()
    }
}
@Composable private fun StripCell(label: String, on: Boolean, run: () -> Unit) {
    Box(Modifier.width(1.dp).fillMaxHeight().background(dcs.outlineVariant))
    Box(Modifier.fillMaxHeight().clickable(onClick = run).padding(horizontal = 10.dp), contentAlignment = Alignment.Center) { DMono(label, if (on) dcs.primary else dcs.onSurfaceVariant) }
}

@Composable
private fun ThreadScreen(id: String, viewer: () -> Unit, hub: () -> Unit, back: () -> Unit) {
    val scope = rememberCoroutineScope()
    DisposableEffect(id) { DataClient.openThread(id); onDispose { DataClient.closeThread(id) } }
    val thread = DataClient.threads.firstOrNull { it.id == id }
    val list = DataClient.entries[id] ?: emptyList()
    val running = thread?.state == "working" || thread?.state == "asking"
    val rows = remember(list, running) { DataFold.rows(list, running) }
    val state = rememberLazyListState()
    // the reader's place: the list follows the newest row only while they are at the end of it
    var stick by remember(id) { mutableStateOf(true) }
    LaunchedEffect(state) { snapshotFlow { state.isScrollInProgress to state.canScrollForward }.collect { (moving, more) -> if (moving) stick = !more } }
    LaunchedEffect(rows.size, list.lastOrNull()?.text?.length, thread?.state) { if (stick && state.layoutInfo.totalItemsCount > 0) state.scrollToItem(state.layoutInfo.totalItemsCount - 1, Int.MAX_VALUE) }
    // a tapped notification lands on its approval, once it is loaded
    LaunchedEffect(rows.size, DataClient.pendingAsk) {
        val at = rows.indexOfFirst { DataClient.pendingAsk.isNotEmpty() && it.id == "ask:" + DataClient.pendingAsk }
        if (at >= 0) { stick = false; state.scrollToItem(at + if (DataClient.older[id] != null) 1 else 0); DataClient.pendingAsk = "" }
    }
    val field = DataClient.field(id); val draft = field.text
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val pending = remember(id) { mutableStateListOf<ByteArray>() }
    var notice by remember(id) { mutableStateOf("") }
    val micAsk = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.RequestPermission()) { ok ->
        if (ok) { notice = ""; DataVoice.press(id) } else notice = "Microphone access is off for Backplane. Turn it on in Settings, then press the mic again."
    }
    val dictate = DataVoice.available
    LaunchedEffect(id, DataClient.conn, dictate) { if (dictate && DataClient.conn == DataClient.Conn.ONLINE) DataVoice.refresh(id) }
    // the masked box: a name ("" for a new one) while open; the value stays in this box until Save
    var box by remember(id) { mutableStateOf<String?>(null) }
    LaunchedEffect(id) { DataClient.refreshSecretAsks(id) }
    val word = DataClient.commandOf(draft)
    fun put(t: String) = DataClient.setField(id, TextFieldValue(t, TextRange(t.length)), typed = false)
    val pick = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) scope.launch { val b = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { DataClient.prepared(ctx, uri) }; if (b != null) pending.add(b) else notice = "That picture could not be read." }
    }
    var menu by remember { mutableStateOf(false) }; var models by remember { mutableStateOf(false) }
    var order by remember(id) { mutableStateOf<DataClient.Order?>(null) }
    LaunchedEffect(id, list.size, thread?.state, DataClient.conn) { if (DataClient.has("order")) runCatching { DataClient.order(id) }.onSuccess { order = it } }
    var drawing by remember(id) { mutableStateOf(false) }
    if (drawing) DrawPadSheet({ drawing = false }) { pending.add(it); notice = "" }
    var openRuns by remember(id) { mutableStateOf(emptySet<String>()) }
    thread?.let { t -> ThreadMenus(t, menu, { menu = false }) { back() }; if (models) PickerSheet(t.id, DNewThreadChoice(), {}) { models = false } }
    Column(Modifier.fillMaxSize()) {
        DBar(thread?.title ?: "", back) {
            if (thread != null && DataClient.has("viewers")) IconButton(onClick = viewer, modifier = Modifier.testTag("thread.viewer")) { Icon(Icons.Default.Dashboard, "Viewer", tint = dcs.onSurfaceVariant) }
            if (thread != null && DataClient.has("actions")) IconButton(onClick = { menu = true }, modifier = Modifier.testTag("thread.more")) { Icon(Icons.Default.MoreVert, "More", tint = dcs.onSurfaceVariant) }
        }
        ConnBanner(hub)
        thread?.let { StateStrip(it) }
        DataClient.work[id]?.takeIf { it.rows.isNotEmpty() }?.let { WorkPanel(id, it) }
        LazyColumn(Modifier.weight(1f).fillMaxWidth().testTag("thread.list"), state = state, contentPadding = PaddingValues(vertical = 8.dp)) {
            if (DataClient.older[id] != null) item("older") { Box(Modifier.fillMaxWidth().clickable { DataClient.loadOlder(id) }.padding(10.dp), contentAlignment = Alignment.Center) { DMono("load earlier", dfaint) } }
            items(rows, key = { it.id }) { r ->
                when (r) {
                    is DRow.Entry -> EntryRow(id, r.entry)
                    is DRow.Fold -> FoldRow(r.fold, r.fold.id in openRuns) { openRuns = if (r.fold.id in openRuns) openRuns - r.fold.id else openRuns + r.fold.id }
                }
            }
            if (list.isEmpty() && DataClient.listsLoaded) item("empty") { DMono("Start a conversation.", dcs.onSurfaceVariant, 13, Modifier.padding(14.dp)) }
            if (thread?.state == "working") item("working") { WorkingRow() }
        }
        DLine()
        order?.let { OrderCard(it) }
        MentionPopup(draft) { put(it) }
        DataClient.btw[id]?.let { b -> BtwCard(b) { DataClient.closeBtw(id) } }
        val asked = DataClient.secretAsks[id]?.firstOrNull()
        if (box != null || asked != null) SecretBox(id, box ?: asked?.name ?: "", asked?.takeIf { box == null || it.name == box }, { box = null })
        Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (notice.isNotEmpty()) DMono(notice, dwarn, 12)
            if (pending.isNotEmpty()) Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                pending.toList().forEach { b -> Box(Modifier.testTag("thread.pending").clickable { pending.remove(b) }) { Pic(b, 64) }; }
            }
            if (dictate) VoiceStrip(id)
            DField(field, { DataClient.setField(id, it) }, "Ask anything: @bot, >thread, %project, #room, &model, \$skill, /btw", Modifier.testTag("thread.composer"), 6, dcs.primary)
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (thread != null && (DataClient.has("picker") || thread.chip.isNotEmpty())) ModelChip(thread.chip.ifEmpty { "${thread.provider} / ${thread.model.ifEmpty { "default" }}" }, "thread.model", Modifier.weight(1f)) { models = true } else Spacer(Modifier.weight(1f))
                Box(Modifier.size(40.dp).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).clickable { if (DataClient.canSendImages) pick.launch("image/*") else notice = "This hub does not take pictures yet (update it)." }.testTag("thread.attach"), contentAlignment = Alignment.Center) { Icon(Icons.Default.AttachFile, "Attach", tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)) }
                Box(Modifier.size(40.dp).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).clickable { if (DataClient.canSendImages) drawing = true else notice = "This hub does not take pictures yet (update it)." }.testTag("thread.draw"), contentAlignment = Alignment.Center) { Icon(Icons.Default.Draw, "Draw", tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)) }
                if (dictate) MicChip(DataVoice.on(id)) {
                    if (DataVoice.on(id) || androidx.core.content.ContextCompat.checkSelfPermission(ctx, android.Manifest.permission.RECORD_AUDIO) == android.content.pm.PackageManager.PERMISSION_GRANTED) DataVoice.press(id)
                    else micAsk.launch(android.Manifest.permission.RECORD_AUDIO)
                }
                val blank = draft.isBlank() && pending.isEmpty()
                if (running && blank) DBtn("Stop", false, modifier = Modifier.testTag("thread.stop"), danger = true) { DataClient.interrupt(id) }
                else DBtn(word?.label ?: "Send", true, !blank, Modifier.testTag("thread.send")) {
                    notice = ""
                    if (word == null) { if (dictate) DataVoice.sent(id); DataClient.send(id, draft, pending.toList()); pending.clear(); stick = true }
                    // a command of the composer: the hub runs it; a refusal leaves the draft and says why
                    else { val text = draft.trim(); scope.launch { try {
                        val r = DataClient.runCommand(id, text)
                        when (r.optString("kind")) {
                            "edit" -> put(r.optString("text"))
                            "secret" -> { box = r.optString("name"); put("") }
                            "intent" -> { notice = "Intent saved"; put("") }
                            else -> put("")
                        }
                    } catch (e: Exception) { notice = e.message ?: "That could not be done." } } }
                }
            }
        }
    }
}

/** A run of tool calls as one line (the desktop's fold row): a tap opens it, each call a line of its own. */
@Composable
private fun FoldRow(f: DFold, open: Boolean, toggle: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 3.dp)) {
        Row(Modifier.fillMaxWidth().heightIn(min = 36.dp).border(BorderStroke(1.dp, dcs.outlineVariant)).clickable(onClick = toggle).padding(horizontal = 8.dp).testTag("fold.${f.id}"), verticalAlignment = Alignment.CenterVertically) {
            Icon(if (open) Icons.Default.ExpandMore else Icons.Default.ChevronRight, if (open) "Shut" else "Open", tint = dcs.onSurfaceVariant, modifier = Modifier.size(15.dp)); Spacer(Modifier.width(8.dp))
            DMono(f.label, dcs.onSurfaceVariant, 12)
        }
        if (open) f.items.forEach { ToolRow(it) }
    }
}

@Composable
private fun ToolRow(e: DEntry) {
    var whole by remember(e.id) { mutableStateOf(false) }
    val step = e.kind == "step"
    val name = if (step) "" else e.tool?.name ?: "tool"
    val what = if (step) e.label.ifEmpty { "Design change" } else if (e.tool?.summary.isNullOrEmpty()) "" else e.text
    Row(Modifier.fillMaxWidth().clickable { whole = !whole }.padding(start = 8.dp, top = 3.dp, bottom = 3.dp)) {
        if (name.isNotEmpty()) { Text(name, color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, fontWeight = FontWeight.Bold); Spacer(Modifier.width(8.dp)) }
        Text(what, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, maxLines = if (whole) Int.MAX_VALUE else 3, overflow = TextOverflow.Ellipsis)
    }
}

/** The working square: breathes while the turn runs (the one animation on screen, and only while this row is). */
@Composable
private fun WorkingRow() {
    val t = androidx.compose.animation.core.rememberInfiniteTransition(label = "working")
    val a by t.animateFloat(0f, 1f, androidx.compose.animation.core.infiniteRepeatable(androidx.compose.animation.core.tween(550), androidx.compose.animation.core.RepeatMode.Reverse), label = "breath")
    Row(Modifier.padding(horizontal = 14.dp, vertical = 8.dp).testTag("thread.working"), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(12.dp), contentAlignment = Alignment.Center) { Box(Modifier.size((5 + 7 * a).dp).alpha(.45f + .55f * a).background(dok)) }
        Spacer(Modifier.width(8.dp)); DMono("Working…", dcs.onSurfaceVariant, 13)
    }
}

@Composable
private fun BtwCard(b: DataClient.Btw, close: () -> Unit) {
    Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 12.dp, vertical = 8.dp).testTag("thread.btw")) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            DMono("btw · ${b.q}", modifier = Modifier.weight(1f))
            IconButton(onClick = close, modifier = Modifier.testTag("thread.btw.close")) { Icon(Icons.Default.Close, "Close", tint = dcs.onSurfaceVariant) }
        }
        Text(b.a, color = dcs.onSurface, fontSize = 13.sp, modifier = Modifier.heightIn(max = 220.dp).verticalScroll(rememberScrollState()))
    }
}

/** The masked box of a chat: `/secret`, or an agent's request. The value is held here and leaves once, as `secret.set`; it is never a draft. */
@Composable
private fun SecretBox(tid: String, name0: String, asked: DataClient.SecretAsk?, shut: () -> Unit) {
    val scope = rememberCoroutineScope()
    var name by remember(tid, name0) { mutableStateOf(name0) }; var value by remember(tid, name0) { mutableStateOf("") }; var err by remember(tid, name0) { mutableStateOf("") }
    Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.primary)).padding(12.dp).testTag("thread.secret"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(if (name0.isEmpty()) "A secret for this chat. Agents use it by name and never see the value." else "Secret: $name0. Agents use it by name and never see the value.", color = dcs.onSurface, fontSize = 13.sp)
        asked?.purpose?.takeIf { it.isNotEmpty() }?.let { DMono(it) }
        if (name0.isEmpty()) DField(name, { name = it }, "Name", Modifier.testTag("thread.secret.name"))
        Box(Modifier.fillMaxWidth().background(dcs.surface).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(12.dp)) {
            if (value.isEmpty()) Text("Paste the value", color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 13.sp)
            BasicTextField(value, { value = it }, textStyle = TextStyle(color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp), cursorBrush = SolidColor(dcs.primary), maxLines = 1,
                visualTransformation = PasswordVisualTransformation(), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false), modifier = Modifier.fillMaxWidth().testTag("thread.secret.value"))
        }
        if (err.isNotEmpty()) DMono(err, dcs.error, 12)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DBtn("Save", true, name.isNotBlank() && value.isNotBlank(), Modifier.testTag("thread.secret.save")) { scope.launch { try { DataClient.saveSecret(tid, name.trim(), value, asked?.purpose ?: ""); value = ""; shut() } catch (e: Exception) { err = e.message ?: "The hub refused the secret" } } }
            DBtn("Cancel", false, true, Modifier.testTag("thread.secret.cancel")) { value = ""; shut() }
            if (asked != null) DBtn("Not needed", false, true) { scope.launch { runCatching { DataClient.dismissSecret(tid, asked.name) }; value = ""; shut() } }
        }
    }
}

@Composable
private fun EntryRow(tid: String, e: DEntry) {
    when (e.kind) {
        "user" -> Column(Modifier.fillMaxWidth().padding(start = 32.dp, end = 12.dp, top = 6.dp, bottom = 6.dp), horizontalAlignment = Alignment.End) {
            e.localImages.forEach { Pic(it, 160); Spacer(Modifier.height(4.dp)) }
            e.attachments.filter { e.localImages.isEmpty() }.forEach { PicById(it.id, 160); Spacer(Modifier.height(4.dp)) }
            if (e.text.isNotEmpty()) Box(Modifier.background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 10.dp, vertical = 8.dp)) { LinkText(e.text, tid, color = dcs.onSurface, size = 13, lineHeight = 19) }
            if (e.sending) DMono("sending…", dfaint, modifier = Modifier.padding(top = 3.dp))
            else if (e.failed) DMono("not sent · retry", dcs.error, modifier = Modifier.clickable { DataClient.retry(tid, e) }.padding(top = 3.dp))
            else DMono(dStamp(e.at), dfaint, modifier = Modifier.padding(top = 3.dp))
        }
        "assistant" -> Column(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 6.dp)) {
            MdText(e.text, thread = tid)
            if (!e.live) DMono(dStamp(e.at), dfaint, modifier = Modifier.padding(top = 4.dp))
        }
        "tool", "step" -> Box(Modifier.padding(horizontal = 12.dp)) { ToolRow(e) }
        "ask" -> AskCard(tid, e)
        "term" -> if (e.term != null && termsOn()) TermCard(e)
        else -> Row(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 4.dp)) {
            val c = if (e.tone == "error") dcs.error else dcs.onSurfaceVariant
            if (e.label.isNotEmpty()) { Text(e.label, color = c, fontFamily = BpMono, fontSize = 12.sp, fontWeight = FontWeight.Bold); Spacer(Modifier.width(8.dp)) }
            LinkText(e.text, tid, color = c, size = 12)
        }
    }
}

@Composable
private fun AskCard(tid: String, e: DEntry) {
    val a = e.ask
    var answer by remember(e.id) { mutableStateOf("") }
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp).background(dcs.surfaceVariant).border(BorderStroke(1.dp, if (a?.open == true) dcs.error else dcs.outlineVariant)).padding(12.dp).testTag("ask.card")) {
        DMono(if (a?.open == true) "ASKS" else "ASKED", dfaint, 11)
        Text(a?.question ?: e.text, color = dcs.onSurface, fontSize = 13.sp, fontFamily = BpMono, modifier = Modifier.padding(vertical = 6.dp))
        if (a != null && a.open) {
            if (a.options.isEmpty()) Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) { DField(answer, { answer = it }, "Your answer", Modifier.testTag("ask.answer")) }
                DBtn("Send", true, answer.isNotBlank()) { DataClient.answer(tid, a.id, answer) }
            } else Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                a.options.forEach { o -> DBtn(o.label, o.id == "accept", true, Modifier.fillMaxWidth().testTag("ask.opt.${o.id}"), danger = o.id == "deny" || o.id == "decline") { DataClient.answer(tid, a.id, o.id) } }
            }
        } else if (a?.answered != null) DMono("answered: ${a.answered}")
    }
}

// ---- settings, folders, new thread
@Composable
private fun SettingsScreen(back: () -> Unit, hub: () -> Unit, accounts: () -> Unit, improve: () -> Unit) {
    Column(Modifier.fillMaxSize()) {
        DBar("Settings", back)
        Column(Modifier.padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            DMono("HUBS", dfaint)
            var forget by remember { mutableStateOf<String?>(null) }
            DataClient.links.mapNotNull { Pairing.key(it) }.forEachIndexed { i, k ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (i == 0) DDot(if (DataClient.conn == DataClient.Conn.ONLINE) dcs.primary else if (DataClient.conn == DataClient.Conn.CONNECTING) dwarn else dcs.error, 8)
                    Text(k, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (i > 0) DBtn("Use", false, modifier = Modifier.testTag("settings.hub.use.$k")) { DataClient.useHub(k); back() }
                    DBtn("Forget", false, modifier = Modifier.testTag("settings.hub.forget.$k"), danger = true) { forget = k }
                }
            }
            DMono(if (DataClient.conn == DataClient.Conn.ONLINE) "online" else "offline ${DataClient.failure}", dcs.onSurfaceVariant, 12)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                DBtn("Hub", false, modifier = Modifier.testTag("settings.hub"), onClick = hub)
                DBtn("Add a hub", false, modifier = Modifier.testTag("settings.hub.add")) { DataClient.pairingError = ""; DataClient.adding = true }
            }
            if (DataClient.has("accounts")) DBtn("Accounts", false, modifier = Modifier.testTag("settings.accounts"), onClick = accounts)
            forget?.let { k ->
                AlertDialog(onDismissRequest = { forget = null }, title = { Text("Forget $k?") },
                    text = { Text("This phone's saved copy of its threads is removed. Pair again to come back.") },
                    confirmButton = { TextButton({ forget = null; DataClient.forgetHub(k); if (DataClient.link == null) back() }) { Text("Forget") } },
                    dismissButton = { TextButton({ forget = null }) { Text("Cancel") } })
            }
            DLine()
            UpdateSection()
            DLine()
            DBtn("Improvements", false, modifier = Modifier.testTag("settings.improve"), onClick = improve)
            DLine()
            if (DataVoice.available) VoiceSettings()
            if (DataMachines.available) { MachinesSettings(); DLine() }
            DMono("ALERTS", dfaint)
            var alerts by remember { mutableStateOf(AppSettings.alertsOn()) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { listOf(true to "on", false to "off").forEach { (v, l) -> DBtn(l, alerts == v, modifier = Modifier.testTag("settings.alerts.$l")) { alerts = v; AppSettings.putAlerts(v) } } }
            DMono("off: only what you let through reaches this phone. alerts come from your uberbot", dcs.onSurfaceVariant, 11)
            if (DataClient.has("push")) {
                val quietOn = DataClient.quiet.isNotEmpty() && DataClient.quiet != "off"
                DMono("QUIET HOURS")
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    DBtn("on", quietOn, modifier = Modifier.testTag("settings.quiet.on")) { if (!quietOn) DataClient.putQuiet("22:00-07:00") }
                    DBtn("off", !quietOn, modifier = Modifier.testTag("settings.quiet.off")) { DataClient.putQuiet("off") }
                }
                if (quietOn) DMono(DataClient.quiet.replace("-", " to "), dcs.onSurfaceVariant, 12)
                DMono("URGENT ASKS COME THROUGH")
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    DBtn("on", DataClient.quietUrgent, modifier = Modifier.testTag("settings.urgent.on")) { DataClient.putQuietUrgent(true) }
                    DBtn("off", !DataClient.quietUrgent, modifier = Modifier.testTag("settings.urgent.off")) { DataClient.putQuietUrgent(false) }
                }
                DMono("in your time zone, on every phone", dcs.onSurfaceVariant, 11)
            }
            DLine()
            DMono("APPEARANCE", dfaint)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("system", "light", "dark").forEach { m -> DBtn(m, AppSettings.appearance == m) { AppSettings.putAppearance(m) } }
            }
            DLine()
            DBtn("Unpair this phone", false, modifier = Modifier.testTag("settings.unpair")) { DataClient.unpair(); back() }
        }
    }
}

// the hub's updates: the hub says what to show (update.status); this draws it
@Composable
private fun UpdateSection() {
    val scope = rememberCoroutineScope()
    var u by remember { mutableStateOf<org.json.JSONObject?>(null) }
    suspend fun load(m: String, p: org.json.JSONObject = org.json.JSONObject()) { runCatching { DataClient.call(m, p) }.onSuccess { u = it } }
    LaunchedEffect(DataClient.conn) { load("update.status") }
    DMono("UPDATES", dfaint)
    val s = u
    if (s == null) DMono("not connected", dcs.onSurfaceVariant, 11) else {
        Text("Backplane ${s.optString("current")}", color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp)
        DMono(s.optString("line"), dcs.onSurfaceVariant, 11, Modifier.testTag("settings.update.line"))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("stable", "beta").forEach { c -> DBtn(c, s.optString("channel") == c, modifier = Modifier.testTag("settings.update.$c")) { scope.launch { load("update.channel", org.json.JSONObject().put("channel", c)) } } }
        }
        if (s.optBoolean("canCheck")) DBtn("Check now", false, modifier = Modifier.testTag("settings.update.check")) { scope.launch { load("update.check"); kotlinx.coroutines.delay(3000); load("update.status") } }
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
    var choice by remember { mutableStateOf(DNewThreadChoice()) }
    var chip by remember { mutableStateOf("") }
    var picking by remember { mutableStateOf(false) }
    val title = DataClient.projects.firstOrNull { it.id == project }?.title ?: ""
    LaunchedEffect(choice) { if (DataClient.has("picker")) runCatching { DataClient.picker(null, choice) }.onSuccess { chip = it.chip } }
    if (picking) PickerSheet(null, choice, { choice = it }) { picking = false }
    Column(Modifier.fillMaxSize()) {
        DBar("New thread · $title", back)
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            DField(text, { text = it }, "What should it do?", Modifier.heightIn(min = 120.dp).testTag("new.text"), 8, dcs.primary)
            if (err.isNotEmpty()) Text(err, color = dcs.error, fontFamily = BpMono, fontSize = 12.sp)
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (DataClient.has("picker")) ModelChip(chip.ifEmpty { " " }, "new.model", Modifier.weight(1f)) { picking = true } else Spacer(Modifier.weight(1f))
                DBtn(if (busy) "Starting…" else "Start", true, !busy, Modifier.testTag("new.start")) {
                    busy = true; scope.launch { try { created(DataClient.createThread(project, text, choice).id) } catch (e: Exception) { err = e.message ?: ""; busy = false } }
                }
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

/** A buy thread's Order card: the steps, the hub's line, the total and what it covers about customs, and Buy now (the hub's link, opened in the browser). */
@Composable
private fun OrderCard(o: DataClient.Order) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp).border(BorderStroke(1.dp, dcs.outlineVariant)).background(dcs.surfaceVariant).padding(10.dp).testTag("thread.order"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        DMono(if (o.name.isEmpty()) "ORDER" else "ORDER · ${o.name}", dcs.onSurface, 12, bold = true)
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            o.steps.forEachIndexed { i, s ->
                if (i > 0) DMono("›", dcs.outlineVariant, 11)
                when (s.mark) { "done" -> DMono("✓ ${s.label}", dcs.onSurfaceVariant, 11); "on" -> DMono(s.label, dcs.primary, 11, bold = true); else -> DMono(s.label, dcs.outlineVariant, 11) }
            }
        }
        DMono(o.line, if (o.waiting) dcs.onSurfaceVariant else dcs.onSurface, 12, lines = 4) // the card's words wrap, as on iOS and in the window
        if (o.total.isNotEmpty()) DMono(o.total, dcs.onSurface, 14, bold = true)
        if (o.customs.isNotEmpty()) DMono(o.customs, dcs.onSurfaceVariant, 11, Modifier.testTag("thread.order.customs"), lines = 4)
        if (o.pay.startsWith("https://") || o.pay.startsWith("http://")) DBtn(o.label.ifEmpty { "Buy now" }, true, modifier = Modifier.testTag("thread.order.pay")) {
            runCatching { ctx.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(o.pay))) }
        }
    }
}
