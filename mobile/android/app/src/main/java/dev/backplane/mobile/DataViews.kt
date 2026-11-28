package dev.backplane.mobile

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloat
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

// Screens for the data/1 app, in the desktop client's language (native/gpui): flat panels on the same tokens, a state dot per thread; on a phone the controls are rounded (dShape, dShapeCard) and interface text is the system face (DText), mono stays for code, paths and small labels (DMono).
internal val dcs @Composable get() = MaterialTheme.colorScheme
private val dlight @Composable get() = dcs.background.luminance() > .5f
internal val dok @Composable get() = Color(if (dlight) 0xFF1C8550 else 0xFF58C28A)
internal val dwarn @Composable get() = Color(if (dlight) 0xFF9A6700 else 0xFFE3B341)
internal val dfaint @Composable get() = Color(if (dlight) 0xFF72777F else 0xFF5C6473)
internal val dview @Composable get() = Color(if (dlight) 0xFFF5F4EF else 0xFF08090B)
internal val dinput @Composable get() = Color(if (dlight) 0xFF7446D0 else 0xFFB48CFF)
// a thread's activity, one colour each (native/gpui/src/ui.rs `side_thread`): green working; red wanting the person or failed; grey otherwise
@Composable internal fun dStateColor(s: String) = when (s) { "working" -> dok; "asking", "failed" -> dcs.error; else -> dfaint }
/** A row's dot as the desktop's sidebar draws it (gpui siderow.rs `dot_of`): the hub's word for the thread (`status`) picks the colour and the shape; finished is the
 *  green dot (a turn done, not yet settled: ready for you to look at). An older hub sends no word: the turn's state decides. */
internal data class DotLook(val color: Color, val round: Boolean, val hollow: Boolean)
@Composable internal fun dotOf(t: DThread): DotLook = when (t.status) {
    "blocked" -> DotLook(dcs.error, false, false)
    "failed" -> DotLook(dcs.error, false, true)
    "stalled" -> DotLook(dwarn, false, false)
    "active", "working" -> DotLook(dcs.primary, true, false)
    "waiting", "monitoring" -> DotLook(dcs.primary.copy(alpha = .55f), true, true)
    "finished" -> DotLook(dok, true, false)
    "" -> when (t.state) { "working" -> DotLook(dcs.primary, true, false); "asking" -> DotLook(dcs.error, false, false); "failed" -> DotLook(dcs.error, false, true); else -> DotLook(dfaint, true, false) }
    else -> DotLook(dfaint, true, false)
}
@Composable internal fun StatusDot(t: DThread, size: Int = 7) {
    val d = dotOf(t); val shape = if (d.round) androidx.compose.foundation.shape.CircleShape else RectangleShape
    Box(Modifier.size(size.dp).then(if (d.hollow) Modifier.border(BorderStroke(1.5.dp, d.color), shape) else Modifier.background(d.color, shape)).testTag("dot.${t.status.ifEmpty { t.state }}"))
}
/** The protocol word (English), drawn through [statusLabel]. */
internal fun dStateWord(s: String) = when (s) { "working" -> "working"; "asking" -> "needs you"; "failed" -> "failed"; else -> "inactive" }
/** A thread state word in the app's language. */
internal fun statusLabel(w: String) = when (w) { "working" -> Msg.android_state_working; "needs you" -> Msg.state_needsyou; "failed" -> Msg.state_failed; "inactive" -> Msg.android_state_inactive; "finished" -> Msg.android_state_finished; "settled" -> Msg.android_state_settled; "stalled" -> Msg.android_state_stalled; "waiting" -> Msg.state_waiting; "monitoring" -> Msg.android_state_monitoring; else -> Tr.t(w) }
/** The STATE strip's word: the hub's (`status`) when it sends one, as the desktop's header says it; else the turn's. */
internal fun dStatusWord(t: DThread) = when (t.status) { "" -> dStateWord(t.state); "active" -> "working"; "blocked" -> "needs you"; else -> t.status }
private val dClock = java.text.SimpleDateFormat("HH:mm", java.util.Locale.getDefault())
private val dDay = java.text.SimpleDateFormat("EEE d MMM HH:mm", java.util.Locale.ENGLISH)
private val dDayJa = java.text.SimpleDateFormat("M月d日(E) HH:mm", java.util.Locale.JAPANESE)
private val dDayEs = java.text.SimpleDateFormat("EEE d MMM HH:mm", java.util.Locale("es", "ES"))
/** A message's time: the clock today, the day too on another. */
internal fun dStamp(ms: Long): String {
    if (ms <= 0) return ""
    val d = java.util.Date(ms); return (if (android.text.format.DateUtils.isToday(ms)) dClock else if (Tr.lang == "ja") dDayJa else if (Tr.lang == "es") dDayEs else dDay).format(d)
}
internal fun dAgo(ms: Long): String {
    val s = (System.currentTimeMillis() - ms) / 1000
    return when { ms <= 0 -> ""; s < 60 -> Msg.android_ago_now; s < 3600 -> Msg.android_ago_m(s / 60); s < 86400 -> Msg.android_ago_h(s / 3600); else -> Msg.android_ago_d(s / 86400) }
}
@Composable internal fun DMono(t: String, c: Color = dcs.onSurfaceVariant, size: Int = 11, modifier: Modifier = Modifier, bold: Boolean = false, lines: Int = 1) =
    Text(t, modifier, color = c, fontFamily = BpMono, fontSize = size.sp, fontWeight = if (bold) FontWeight.Bold else FontWeight.Normal, maxLines = lines, overflow = TextOverflow.Ellipsis)
/** Interface text (titles, sentences, labels of controls) in the system face; [DMono] stays for code, paths, ids, meta lines and the small uppercase labels. */
@Composable internal fun DText(t: String, c: Color = dcs.onSurface, size: Int = 14, modifier: Modifier = Modifier, bold: Boolean = false, lines: Int = Int.MAX_VALUE) =
    // one line keeps the theme's line height (rows stay as tall as they were); a paragraph reads closer, as the settings' notes do
    Text(t, modifier, color = c, fontFamily = BpUi, fontSize = size.sp, fontWeight = if (bold) FontWeight.SemiBold else FontWeight.Normal, maxLines = lines, overflow = TextOverflow.Ellipsis,
        lineHeight = if (lines == 1) androidx.compose.ui.unit.TextUnit.Unspecified else (size + 5).sp)
@Composable internal fun DDot(c: Color, size: Int = 6) = Box(Modifier.size(size.dp).background(c))
@Composable internal fun DLine() = Box(Modifier.fillMaxWidth().height(1.dp).background(dcs.outlineVariant))

private sealed interface Page { object Improve : Page; data class Room(val room: DataClient.Room) : Page; data class Viewer(val id: String, val file: String = "", val line: Int = 0, val from: Page? = null) : Page; object Home : Page; data class Hub(val from: Page) : Page; data class Thread(val id: String, val from: Page? = null) : Page; data class Swarm(val room: String) : Page; data class Sub(val id: String, val key: String) : Page; object Settings : Page; data class Folders(val clone: CloneAsk? = null) : Page; data class New(val project: String) : Page; object Accounts : Page; object Mcp : Page; data class Term(val key: String, val from: Page) : Page; data class Terms(val project: String?, val from: Page) : Page; data class Account(val id: String, val start: Boolean, val n: Long = System.nanoTime()) : Page }

@Composable
fun DataRoot() {
    var page by remember { mutableStateOf<Page>(DataClient.openFromIntent?.let { if (DataClient.openViewerTab != null) Page.Viewer(it) else Page.Thread(it) } ?: Page.Home) }
    // an expired sign-in's alert: that account, its sign-in started
    LaunchedEffect(DataClient.openAccount) { DataClient.openAccount?.let { page = Page.Account(it, true); DataClient.openAccount = null } }
    LaunchedEffect(DataClient.openAccounts) { if (DataClient.openAccounts) { DataClient.openAccounts = false; page = Page.Accounts } }
    LaunchedEffect(DataClient.openSettings) { if (DataClient.openSettings) { DataClient.openSettings = false; page = Page.Settings } }
    LaunchedEffect(DataClient.openFromIntent) { DataClient.openFromIntent?.let { page = if (DataClient.openViewerTab != null) Page.Viewer(it) else Page.Thread(it); DataClient.openFromIntent = null } }
    // a link the hub resolved (DataLinks.kt): its screen
    LaunchedEffect(DataClient.openAddProject) { if (DataClient.openAddProject) { DataClient.openAddProject = false; page = Page.Folders() } }
    // /project <address>: Add project on Clone from git with it
    LaunchedEffect(Clones.ask) { Clones.ask?.let { page = Page.Folders(it); Clones.ask = null } }
    LaunchedEffect(DataClient.linkTarget) {
        val t = DataClient.linkTarget ?: return@LaunchedEffect
        DataClient.linkTarget = null
        when (t.kind) {
            LinkTarget.Kind.THREAD -> page = Page.Thread(t.id)
            LinkTarget.Kind.FILE -> page = Page.Viewer(t.id, t.path, t.line, from = page)
            LinkTarget.Kind.PROJECT -> { DataClient.focusProject = t.id; page = Page.Home }
            LinkTarget.Kind.SETTINGS -> page = if (t.id == "accounts") Page.Accounts else Page.Settings
            LinkTarget.Kind.ROOM -> {
                if (Swarms.on() && !Swarms.isSwarm(t.id)) Swarms.load()
                if (Swarms.isSwarm(t.id)) { page = Page.Swarm(t.id); return@LaunchedEffect }   // a swarm's page is its room's
                if (DataClient.rooms.none { it.id == t.id }) runCatching { DataClient.loadBots() }
                DataClient.rooms.firstOrNull { it.id == t.id }?.let { page = Page.Room(it) } ?: DataClient.say(Msg.android_room_gone)
            }
        }
    }
    Box(Modifier.fillMaxSize().background(dcs.background).windowInsetsPadding(WindowInsets.safeDrawing)) {
        if (DataClient.link == null || DataClient.needsRepair || DataClient.adding) { if (DataClient.adding && DataClient.link != null) BackHandler { DataClient.adding = false }; PairScreen() }
        else CompositionLocalProvider(LocalTermOpen provides { k: String -> page = Page.Term(k, page.let { if (it is Page.Term) it.from else it }) }, LocalTermsOpen provides { pr: String? -> page = Page.Terms(pr, page) }) { when (val p = page) {
            Page.Home -> HomeScreen({ page = Page.Thread(it) }, { page = Page.Settings }, { page = Page.Hub(Page.Home) }, { page = Page.Folders() }, { page = Page.New(it) }, { page = Page.Swarm(it) }, { id, file -> page = Page.Viewer(id, file) }) { page = Page.Room(it) }
            is Page.Term -> { BackHandler { page = p.from }; TermFull(p.key, Msg.palette_terminal) { page = p.from } }
            is Page.Terms -> { BackHandler { page = p.from }; TermsScreen(p.project, { page = p.from }) { page = Page.Term(it, p) } }
            is Page.Viewer -> {
                val backPage = p.from ?: if (p.id.startsWith("project-home-")) Page.Home else Page.Thread(p.id)
                BackHandler { page = backPage }
                val viewerThread = DataClient.threads.firstOrNull { it.id == p.id }
                    // a project's own viewer (the project page's Board / Schematic / 3D / Files): no thread, the project's folder
                    ?: p.id.removePrefix("project-home-").takeIf { p.id.startsWith("project-home-") && it.isNotEmpty() }?.let { pid -> DThread(id = p.id, project = pid, title = DataClient.projects.firstOrNull { it.id == pid }?.title ?: pid, state = "idle", updatedAt = 0L, pinned = false, settled = false, archived = false, provider = "", model = "", preview = "", waiting = 0) }
                    ?: if (p.file.isNotEmpty()) DThread(id = p.id, project = "", title = p.file.substringAfterLast('/'), state = "idle", updatedAt = 0L, pinned = false, settled = false, archived = false, provider = "", model = "", preview = "", waiting = 0) else null
                viewerThread?.let { ViewerScreen(it, p.file, p.line) { page = backPage } }
                ?: Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() } } // never a blank page while the thread list comes
            is Page.Room -> { BackHandler { page = Page.Home }; RoomScreen(p.room) { page = Page.Home } }
            is Page.Thread -> { BackHandler { page = p.from ?: Page.Home }; CompositionLocalProvider(LocalOpenSub provides { k -> page = Page.Sub(p.id, k) }) { ThreadScreen(p.id, { page = Page.Viewer(p.id) }, { page = Page.Hub(p) }, { page = Page.Thread(it) }, { page = Page.Settings }) { page = p.from ?: Page.Home } } }
            is Page.Swarm -> { BackHandler { page = Page.Home }; SwarmScreen(p.room, { page = Page.Home }) { page = Page.Thread(it, p) } }
            is Page.Sub -> { BackHandler { page = Page.Thread(p.id) }; SubScreen(p.id, p.key) { page = Page.Thread(p.id) } }
            Page.Settings -> { BackHandler { page = Page.Home }; SettingsScreen({ page = Page.Home }, { page = Page.Hub(Page.Settings) }, { page = Page.Accounts }, { page = Page.Mcp }) { page = Page.Improve } }
            Page.Mcp -> { BackHandler { page = Page.Settings }; McpScreen { page = Page.Settings } }
            Page.Accounts -> { BackHandler { page = Page.Settings }; AccountsScreen({ page = Page.Settings }) { page = Page.Account(it, false) } }
            is Page.Account -> { BackHandler { page = Page.Accounts }; key(p) { AccountScreen(p.id, p.start, { page = Page.Accounts }) { page = Page.Account(it, false) } } } // each opening its own screen: an alert's tap reads the account afresh and starts the sign-in
            is Page.Hub -> { BackHandler { page = p.from }; HubScreen { page = p.from } }
            Page.Improve -> { BackHandler { page = Page.Settings }; ImproveScreen { page = Page.Settings } }
            is Page.Folders -> { BackHandler { page = Page.Home }; key(p) { FolderScreen(p.clone, { page = Page.Home }, { page = Page.New(it) }) { t -> page = if (t != null) Page.Thread(t) else Page.Home } } }
            is Page.New -> { BackHandler { page = Page.Home }; NewThreadScreen(p.project, { page = Page.Home }) { page = Page.Thread(it) } }
        } }
        DataClient.askLink?.let { AskPair(it) }
        DownloadStrip()
        LinkNote()
    }
}

@Composable
internal fun DBar(title: String, back: (() -> Unit)?, titleMenu: (@Composable ColumnScope.(dismiss: () -> Unit) -> Unit)? = null, trailing: @Composable RowScope.() -> Unit = {}) {
    Column(Modifier.background(dcs.surfaceVariant)) {
        Row(Modifier.fillMaxWidth().height(48.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            if (back != null) IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, Msg.tour_back, tint = dcs.onSurfaceVariant) } else { Spacer(Modifier.width(10.dp)); BrandMark(DataClient.conn == DataClient.Conn.ONLINE); Spacer(Modifier.width(8.dp)) }
            if (titleMenu == null) Text(title.ifEmpty { Msg.thread_new }, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpUi, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            else {
                var open by remember { mutableStateOf(false) }
                Box(Modifier.weight(1f)) {
                    Row(Modifier.clickable { open = true }.padding(vertical = 12.dp).testTag("home.hub"), verticalAlignment = Alignment.CenterVertically) {
                        Text(title, color = dcs.onSurface, fontFamily = BpUi, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
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
private fun BoxScope.ReconnectingPill() {
    if (DataClient.conn == DataClient.Conn.CONNECTING && DataClient.restartText == null && (DataClient.listsLoaded || DataClient.sections.isNotEmpty() || DataClient.projects.isNotEmpty())) {
        Row(
            Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 10.dp)
                .background(dcs.surfaceContainerHigh, CircleShape)
                .border(BorderStroke(1.dp, dcs.outlineVariant), CircleShape)
                .padding(horizontal = 10.dp, vertical = 5.dp)
                .testTag("conn.reconnecting"),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            DDot(dwarn, 6); Spacer(Modifier.width(6.dp))
            DMono(Msg.mobile_conn_reconnecting, dcs.onSurfaceVariant, 11)
        }
    }
}

@Composable
private fun ConnBanner(hub: () -> Unit, settings: () -> Unit) {
    MigrateBanner()
    HubBanner(hub)
    DataClient.home?.awayLine()?.let { (line, isErr) ->
        Row(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp).testTag("home.away"), verticalAlignment = Alignment.CenterVertically) {
            DMono(line, if (isErr) dcs.error else dcs.onSurfaceVariant, 12, Modifier.weight(1f))
        }
    }
    TelemetryBanner(settings)
    StarBanner()
    AlertsBanner()
    // the hub's restart (RestartStrip.kt) says itself here in place of the usual look: while it is away, and its crash note as we join
    val online = DataClient.conn == DataClient.Conn.ONLINE
    val restart = DataClient.restartText
    val hasCached = DataClient.listsLoaded || DataClient.sections.isNotEmpty() || DataClient.projects.isNotEmpty()
    if (online && restart == null) return
    if (!online && restart == null && DataClient.conn == DataClient.Conn.CONNECTING && hasCached) return
    Row(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp).testTag("conn.strip"), verticalAlignment = Alignment.CenterVertically) {
        DDot(if (online) dcs.primary else if (restart != null || DataClient.conn == DataClient.Conn.CONNECTING) dwarn else dcs.error, 8); Spacer(Modifier.width(8.dp))
        DMono(restart?.let { Tr.t(it) } ?: if (DataClient.conn == DataClient.Conn.CONNECTING) Msg.android_conn_connecting(DataClient.hubName) else Msg.android_conn_offline, dcs.onSurface, 12, Modifier.weight(1f))
        if (!online) {
            Spacer(Modifier.width(8.dp))
            DMono(Msg.android_hub, dcs.primary, 12, Modifier.testTag("conn.hub").clickable(onClick = hub).padding(horizontal = 4.dp, vertical = 4.dp), bold = true)
        }
    }
}

@Composable
private fun PairScreen() {
    var text by remember(DataClient.pairEpoch) { mutableStateOf("") }
    var address by remember(DataClient.pairEpoch) { mutableStateOf("") }
    var code by remember(DataClient.pairEpoch) { mutableStateOf("") }
    val ctx = androidx.compose.ui.platform.LocalContext.current
    // the in-app scanner (Scanner.kt): over this screen while open; a camera refused or missing leaves a note here, beside the photo and the paste
    val scan = pairScanner()
    val photo = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) pairingFromImage(ctx, uri)?.let { DataClient.pair(it) } ?: run { DataClient.pairingError = Msg.native_pair_scanner_invalid }
    }
    if (scan.open) { ScannerView(scan.close, scan.failed) { scan.close(); DataClient.pair(it) }; return }
    // centred when it fits; scrolls when it does not (a phone in landscape, a large font, the keyboard up): the short code and its button stay reachable
    Box(Modifier.fillMaxSize().imePadding(), contentAlignment = Alignment.Center) {
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(24.dp)) {
        DMono("BACKPLANE", dcs.primary, 13, bold = true)
        Spacer(Modifier.height(8.dp))
        Text(if (DataClient.needsRepair) Msg.android_pair_repair else if (DataClient.adding) Msg.android_hub_add else Msg.native_pair_title, color = dcs.onSurface, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(4.dp))
        Text(if (DataClient.needsRepair) Msg.android_pair_revoked else Msg.android_pair_paste, color = dcs.onSurfaceVariant, fontSize = 14.sp)
        Spacer(Modifier.height(20.dp))
        // the keyboard's Done presses Pair, Next (address) moves to the code, Go (code) presses Pair with code: each only when its button is on
        val pairGo = { if (text.isNotBlank() && !DataClient.pairingBusy) DataClient.pair(text) }
        val codeGo = {
            if (address.isNotBlank() && code.isNotBlank() && !DataClient.pairingBusy) { val l = Pairing.codeLink(address.trim(), code); if (l == null) DataClient.pairingError = Msg.android_pair_code_bad else DataClient.pair(l) }
        }
        val codeFocus = remember { FocusRequester() }
        DField(text, { text = it }, "http://host:3787/pair#invite=…", Modifier.testTag("pair.field"), single = true, keyboard = KeyboardOptions(imeAction = ImeAction.Done), actions = KeyboardActions(onDone = { pairGo() }), mono = true)
        if (DataClient.pairingError.isNotEmpty()) { Spacer(Modifier.height(8.dp)); Text(Tr.t(DataClient.pairingError), color = dcs.onSurfaceVariant, fontSize = 13.sp) }
        Spacer(Modifier.height(16.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DBtn(Msg.native_pair_action, true, text.isNotBlank() && !DataClient.pairingBusy, Modifier.testTag("pair.go")) { pairGo() }
            DBtn(Msg.native_pair_scanner_title, false, !DataClient.pairingBusy, Modifier.testTag("pair.scan"), onClick = scan.start)
            if (DataClient.adding && DataClient.link != null) DBtn(Msg.common_cancel, false, true, Modifier.testTag("pair.cancel")) { DataClient.adding = false; DataClient.pairingError = "" }
        }
        Spacer(Modifier.height(8.dp))
        DBtn(Msg.android_pair_photo, false, !DataClient.pairingBusy, Modifier.testTag("pair.photo")) { photo.launch("image/*") }
        if (scan.note.isNotEmpty()) { Spacer(Modifier.height(8.dp)); ScanNote(scan) }
        // revoked: the other paired hubs are one tap away
        val others = DataClient.links.drop(1).mapNotNull { Pairing.key(it) }
        if (DataClient.needsRepair && others.isNotEmpty()) {
            Spacer(Modifier.height(16.dp)); Text(Msg.android_pair_other, color = dcs.onSurfaceVariant, fontSize = 13.sp); Spacer(Modifier.height(8.dp))
            others.forEach { k -> DBtn(k, false, modifier = Modifier.testTag("pair.use.$k")) { DataClient.useHub(k) } }
        }
        Spacer(Modifier.height(24.dp))
        Text(Msg.android_pair_code_lead, color = dcs.onSurfaceVariant, fontSize = 13.sp)
        Spacer(Modifier.height(8.dp))
        DField(address, { address = it }, "host:port", Modifier.testTag("pair.address"), single = true, keyboard = KeyboardOptions(imeAction = ImeAction.Next), actions = KeyboardActions(onNext = { codeFocus.requestFocus() }), mono = true)
        Spacer(Modifier.height(8.dp))
        DField(code, { code = it }, "ABCD-2345", Modifier.testTag("pair.code"), single = true, keyboard = KeyboardOptions(imeAction = ImeAction.Go), actions = KeyboardActions(onGo = { codeGo() }), focus = codeFocus, mono = true)
        Spacer(Modifier.height(16.dp))
        DBtn(Msg.android_pair_code, false, address.isNotBlank() && code.isNotBlank() && !DataClient.pairingBusy, Modifier.testTag("pair.code.go")) { codeGo() }
    }
    }
}

/** A link from outside the app names the hub it would pair with; Cancel comes first. */
@Composable
private fun AskPair(link: String) {
    val hub = Pairing.invitation(link)?.origin ?: Pairing.key(link)
    if (hub == null) { LaunchedEffect(link) { DataClient.askLink = null; DataClient.pairingError = Msg.android_pair_link_bad }; return }
    Column(Modifier.fillMaxSize().background(dcs.background).padding(24.dp), verticalArrangement = Arrangement.Center) {
        Text(Msg.android_pair_ask, color = dcs.onSurface, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(8.dp))
        DMono(hub, dcs.onSurface, 14)
        Spacer(Modifier.height(8.dp))
        Text(Msg.android_pair_ask_note, color = dcs.onSurfaceVariant, fontSize = 14.sp)
        Spacer(Modifier.height(16.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DBtn(Msg.common_cancel, true, true, Modifier.testTag("pair.ask.cancel")) { DataClient.askLink = null }
            DBtn(Msg.native_pair_action, false, true, Modifier.testTag("pair.ask.yes")) { DataClient.askLink = null; DataClient.pair(link) }
        }
    }
}

@Composable
internal fun DField(v: String, on: (String) -> Unit, hint: String, modifier: Modifier = Modifier, lines: Int = 1, line: Color = dcs.outlineVariant, single: Boolean = false, keyboard: KeyboardOptions = KeyboardOptions.Default, actions: KeyboardActions = KeyboardActions.Default, focus: FocusRequester? = null, mono: Boolean = false) {
    val face = if (mono) BpMono else BpUi; val size = if (mono) 13.sp else 15.sp
    Box(modifier.fillMaxWidth().clip(dShape).background(dview).border(BorderStroke(1.dp, line), dShape).padding(horizontal = 10.dp, vertical = 10.dp)) {
        if (v.isEmpty()) Text(hint, color = dfaint, fontFamily = face, fontSize = size, maxLines = 1, overflow = TextOverflow.Ellipsis)
        BasicTextField(v, on, textStyle = TextStyle(color = dcs.onSurface, fontFamily = face, fontSize = size), cursorBrush = SolidColor(dcs.primary), singleLine = single, maxLines = if (single) 1 else lines, keyboardOptions = keyboard, keyboardActions = actions, modifier = Modifier.fillMaxWidth().let { m -> focus?.let { m.focusRequester(it) } ?: m })
    }
}

@Composable
internal fun DField(v: TextFieldValue, on: (TextFieldValue) -> Unit, hint: String, modifier: Modifier = Modifier, lines: Int = 1, line: Color = dcs.outlineVariant, mono: Boolean = false) {
    val face = if (mono) BpMono else BpUi; val size = if (mono) 13.sp else 15.sp
    Box(modifier.fillMaxWidth().clip(dShape).background(dview).border(BorderStroke(1.dp, line), dShape).padding(horizontal = 10.dp, vertical = 10.dp)) {
        if (v.text.isEmpty()) Text(hint, color = dfaint, fontFamily = face, fontSize = size, maxLines = 1, overflow = TextOverflow.Ellipsis)
        BasicTextField(v, on, textStyle = TextStyle(color = dcs.onSurface, fontFamily = face, fontSize = size), cursorBrush = SolidColor(dcs.primary), maxLines = lines, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
internal fun DBtn(label: String, primary: Boolean, enabled: Boolean = true, modifier: Modifier = Modifier, danger: Boolean = false, small: Boolean = false, onClick: () -> Unit) {
    val bg = if (primary && enabled) dcs.primary else dcs.surfaceContainerHigh
    val fg = if (primary && enabled) dcs.onPrimary else if (!enabled) dfaint else if (danger) dcs.error else dcs.onSurface
    Box(modifier.clip(dShape).background(bg).border(BorderStroke(1.dp, if (primary && enabled) dcs.primary else dcs.outlineVariant), dShape).clickable(enabled = enabled, onClick = onClick).heightIn(min = if (small) 30.dp else 44.dp).padding(horizontal = if (small) 8.dp else 14.dp), contentAlignment = Alignment.Center) {
        Text(label, color = fg, fontFamily = BpUi, fontSize = if (small) 12.sp else 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
    }
}

/** The composer's picture tools, the same in a thread and on New thread: Attach (Choose a photo, Take a photo, and what `more` adds) and Draw;
 *  each picture made ready (DataClient.prepared) goes into `pending`; `say` gets the line for the composer's notice ("" clears it). */
@Composable
internal fun PhotoTools(pending: androidx.compose.runtime.snapshots.SnapshotStateList<ByteArray>, say: (String) -> Unit, tag: String, more: @Composable (close: () -> Unit) -> Unit = {}) {
    val ctx = androidx.compose.ui.platform.LocalContext.current; val scope = rememberCoroutineScope()
    val pick = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) scope.launch { val b = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { DataClient.prepared(ctx, uri) }; if (b != null) pending.add(b) else say(Msg.android_picture_bad) }
    }
    // the camera's photo (Take a photo, by Attach): into the app's cache through its FileProvider, then made ready like a chosen one
    var shotUri by remember { mutableStateOf<android.net.Uri?>(null) }
    val shoot = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.TakePicture()) { ok ->
        val u = shotUri; if (ok && u != null) scope.launch { val b = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { DataClient.prepared(ctx, u) }; if (b != null) pending.add(b) else say(Msg.android_photo_bad) }
    }
    fun takePhoto() = runCatching {
        val dir = java.io.File(ctx.cacheDir, "camera").apply { mkdirs() }
        dir.listFiles()?.filter { System.currentTimeMillis() - it.lastModified() > 3_600_000 }?.forEach { it.delete() }
        val u = androidx.core.content.FileProvider.getUriForFile(ctx, ctx.packageName + ".files", java.io.File(dir, "photo-${System.currentTimeMillis()}.jpg"))
        shotUri = u; shoot.launch(u)
    }.onFailure { say(Msg.android_camera_failed) }
    val camAsk = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.RequestPermission()) { ok ->
        if (ok) { say(""); takePhoto() } else say(Msg.android_camera_off)
    }
    var attachMenu by remember { mutableStateOf(false) }
    var drawing by remember { mutableStateOf(false) }
    if (drawing) DrawPadSheet({ drawing = false }) { pending.add(it); say("") }
    Box(Modifier.size(40.dp).clip(dShape).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant), dShape).clickable { if (DataClient.canSendImages) attachMenu = true else say(Msg.android_pictures_old) }.testTag("$tag.attach"), contentAlignment = Alignment.Center) {
        Icon(Icons.Default.AttachFile, Msg.composer_attach, tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp))
        DropdownMenu(attachMenu, { attachMenu = false }) {
            DropdownMenuItem({ Text(Msg.android_photo_choose, fontFamily = BpUi) }, { attachMenu = false; pick.launch("image/*") }, Modifier.testTag("$tag.attach.pick"), leadingIcon = { Icon(Icons.Default.Image, null) })
            DropdownMenuItem({ Text(Msg.android_photo_take, fontFamily = BpUi) }, { attachMenu = false
                if (androidx.core.content.ContextCompat.checkSelfPermission(ctx, android.Manifest.permission.CAMERA) == android.content.pm.PackageManager.PERMISSION_GRANTED) takePhoto() else camAsk.launch(android.Manifest.permission.CAMERA)
            }, Modifier.testTag("$tag.attach.camera"), leadingIcon = { Icon(Icons.Default.PhotoCamera, null) })
            more { attachMenu = false }
        }
    }
    Box(Modifier.size(40.dp).clip(dShape).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant), dShape).clickable { if (DataClient.canSendImages) drawing = true else say(Msg.android_pictures_old) }.testTag("$tag.draw"), contentAlignment = Alignment.Center) { Icon(Icons.Default.Draw, Msg.draw_title, tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)) }
}

/** The pictures waiting to go with a message: a tap on one drops it; the × in its corner says so (square, as iOS's xmark on the same thumbnails). */
@Composable
internal fun PendingPics(pending: androidx.compose.runtime.snapshots.SnapshotStateList<ByteArray>, tag: String) {
    if (pending.isNotEmpty()) Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        pending.toList().forEach { b -> Box(Modifier.testTag("$tag.pending").clickable { pending.remove(b) }) {
            Pic(b, 64)
            Box(Modifier.align(Alignment.TopEnd).padding(3.dp).size(18.dp).background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant)).clickable { pending.remove(b) }.testTag("$tag.pending.remove"), contentAlignment = Alignment.Center) {
                Icon(Icons.Default.Close, Msg.ios_picture_remove, tint = dcs.onSurface, modifier = Modifier.size(12.dp))
            }
        } }
    }
}

/** A file sent with a message, or waiting in the composer (with its × to drop it): a document mark and its name. */
@Composable
internal fun FileChip(name: String, modifier: Modifier = Modifier, remove: (() -> Unit)? = null) {
    Row(modifier.clip(dShape).background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant), dShape).padding(start = 8.dp, end = if (remove == null) 10.dp else 2.dp).heightIn(min = 36.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.Default.Description, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(6.dp))
        DMono(name, dcs.onSurface, 12, Modifier.widthIn(max = 220.dp))
        if (remove != null) Box(Modifier.size(32.dp).clickable(onClick = remove).testTag("thread.pending.file.remove"), contentAlignment = Alignment.Center) { Icon(Icons.Default.Close, Msg.common_remove, tint = dcs.onSurfaceVariant, modifier = Modifier.size(14.dp)) }
    }
}

/** The composer's model chip: the hub's "claude / opus / high" in a line box; a tap opens the picker. */
@Composable
internal fun ModelChip(text: String, tag: String, modifier: Modifier = Modifier, provider: String = "", onClick: () -> Unit) {
    val pv = remember(provider, text) { provider.ifEmpty { ProviderMarks.providerOfChip(text) } }
    Box(modifier.height(40.dp).clip(dShape).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant), dShape).clickable(onClick = onClick).padding(horizontal = 10.dp).testTag(tag), contentAlignment = Alignment.CenterStart) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (pv.isNotEmpty()) ProviderMark(pv, 14.dp)
            DMono(Tr.t(text), dcs.onSurface, 12)
        }
    }
}

private enum class HomeTab { INBOX, CREW, BENCH }

// ---- home: mobile-native Inbox | Crew | Bench tabs (with persistent top-bar Bub + global Search)
/** The project page open over the home list, with the hub it belongs to (DataClient.usingKey): it outlives HomeScreen while a thread opened from it
 *  is in front, so Back lands on the page again; a page of another hub is not shown. The row only gives the page its key and title. */
private var homeSecPage by mutableStateOf<Pair<String, DSecRow>?>(null)
/** The home tab on show, kept the same way: Back from a thread or a project page lands on the tab it was opened from. */
private val homeTab = mutableStateOf(HomeTab.INBOX)

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HomeScreen(open: (String) -> Unit, settings: () -> Unit, hub: () -> Unit, addProject: () -> Unit, newThread: (String) -> Unit, openSwarm: (String) -> Unit, openViewer: (String, String) -> Unit = { _, _ -> }, openRoom: (DataClient.Room) -> Unit) {
    var q by remember { mutableStateOf("") }
    var searchOpen by remember { mutableStateOf(false) }
    var tab by homeTab
    var projMenu by remember { mutableStateOf<DProject?>(null) }
    var botDelete by remember { mutableStateOf<DataClient.Bot?>(null) }
    var roomDelete by remember { mutableStateOf<DataClient.Room?>(null) }
    // the project the long press was on stays after its sheet closes: Rename, Badge and Remove open their own box after it (the sheet alone closes)
    var projSheet by remember { mutableStateOf(false) }
    var pickNew by remember { mutableStateOf<DProject?>(null) }
    var newSwarm by remember { mutableStateOf<DProject?>(null) }   // New swarm's sheet, from a project's menu   // a project on more than one machine: where the new thread starts
    // a project's page (sections), over the list until Back; kept outside the screen (homeSecPage) so Back from a thread it opened comes back to it
    val secPage = homeSecPage?.takeIf { it.first == DataClient.usingKey }?.second
    fun showSec(r: DSecRow?) { homeSecPage = r?.let { DataClient.usingKey to it } }
    var secMenu by remember { mutableStateOf<DSecRow?>(null) }
    var secRename by remember { mutableStateOf<DSection?>(null) }
    var secNewSwarm by remember { mutableStateOf<String?>(null) }   // New swarm from a section project's menu (a project id)
    secPage?.let { p -> androidx.activity.compose.BackHandler { showSec(null) }; ProjectPageScreen(p, open, { id -> showSec(null); newThread(id) }, { room -> showSec(null); openSwarm(room) }, openViewer) { showSec(null) }; return }
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val prefs = remember { ctx.getSharedPreferences("data.ui", android.content.Context.MODE_PRIVATE) }
    var shut by remember { mutableStateOf(prefs.getStringSet("projects.shut", emptySet())!!.toSet()) }
    var treeOpen by remember { mutableStateOf(prefs.getStringSet("tree.open", emptySet())!!.toSet()) } // parents whose subthreads show
    val termsOpen = LocalTermsOpen.current
    val listState = androidx.compose.foundation.lazy.rememberLazyListState()
    var flash by remember { mutableStateOf("") }
    val rowOf = remember { mutableMapOf<String, Int>() }   // a project's header row in the list
    val sectioned = DataClient.has("sections") && DataClient.sections.isNotEmpty()
    val partition = remember(DataClient.sections) { partitionSections(DataClient.sections) }
    val uberBot = DataClient.bots.firstOrNull { it.uber && it.thread.isNotEmpty() }
    // a project link: switch to Bench (or open its group on older hubs)
    LaunchedEffect(DataClient.focusProject, DataClient.projects.size) {
        val id = DataClient.focusProject; if (id.isEmpty() || DataClient.projects.none { it.id == id }) return@LaunchedEffect
        if (sectioned) { tab = HomeTab.BENCH }
        shut = shut - id; prefs.edit().putStringSet("projects.shut", shut).apply(); flash = id; DataClient.focusProject = ""
        kotlinx.coroutines.delay(150)
        rowOf[id]?.let { listState.animateScrollToItem(it) }
        kotlinx.coroutines.delay(2500); if (flash == id) flash = ""
    }
    Column(Modifier.fillMaxSize().background(dcs.surfaceVariant)) {
        DBar(DataClient.hubName.ifEmpty { "Backplane" }, null, { dismiss ->
            DataClient.links.drop(1).mapNotNull { Pairing.key(it) }.forEach { k -> DropdownMenuItem({ Text(k, fontFamily = BpMono) }, { dismiss(); DataClient.useHub(k) }, Modifier.testTag("home.hub.use.$k")) }
            DropdownMenuItem({ Text(Msg.android_hub_add, fontFamily = BpUi) }, { dismiss(); DataClient.pairingError = ""; DataClient.adding = true }, Modifier.testTag("home.hub.add"))
            if (termsOn()) DropdownMenuItem({ Text(Msg.android_terminals, fontFamily = BpUi) }, { dismiss(); termsOpen(null) }, Modifier.testTag("home.hub.terms"))
        }) {
            // Persistent 1-tap Bub quick-jump in the top bar across every tab
            if (sectioned && uberBot != null) {
                IconButton(onClick = { open(uberBot.thread) }, modifier = Modifier.testTag("home.bub.quick")) {
                    if (DataClient.has("cats")) BotCat(uberBot.look, uberBot.color, 26.dp)
                    else DMono("@", dcs.primary, 15, bold = true)
                }
            }
            if (sectioned && DataClient.has("search")) {
                IconButton(onClick = { searchOpen = !searchOpen; if (!searchOpen) q = "" }, modifier = Modifier.testTag("home.search.toggle")) {
                    Icon(if (searchOpen) Icons.Default.Close else Icons.Default.Search, if (searchOpen) Msg.common_close else Msg.common_search, tint = if (searchOpen) dcs.primary else dcs.onSurfaceVariant)
                }
            }
            // New chat: a thread in no project, from every tab (the hub keeps it under Chats until it is settled or moved to a project)
            IconButton(onClick = { newThread(DataClient.CHATS) }, modifier = Modifier.testTag("home.chat.new")) { Icon(Icons.Default.Edit, Msg.chats_new, tint = dcs.onSurfaceVariant) }
            IconButton(onClick = settings, modifier = Modifier.testTag("home.settings")) { Icon(Icons.Default.Settings, Msg.palette_settings, tint = dcs.onSurfaceVariant) }
        }
        ConnBanner(hub, settings)
        NoProviderBanner { DataClient.openAccounts = true }
        if (sectioned) {
            val searchFocus = remember { androidx.compose.ui.focus.FocusRequester() }
            LaunchedEffect(searchOpen) { if (searchOpen) runCatching { searchFocus.requestFocus() } }
            if (searchOpen && DataClient.has("search")) {
                Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 4.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.weight(1f)) { DField(q, { q = it }, Msg.android_search_hint, Modifier.focusRequester(searchFocus).testTag("home.searchfield")) }
                    IconButton(onClick = { q = ""; searchOpen = false }, modifier = Modifier.testTag("home.search.close")) { Icon(Icons.Default.Close, Msg.common_close, tint = dcs.onSurfaceVariant) }
                }
                DLine()
                if (q.isNotBlank()) {
                    Box(Modifier.weight(1f)) { SearchResults(q, open) }
                }
            }
            if (!searchOpen || q.isBlank()) {
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    when (tab) {
                        HomeTab.INBOX -> {
                            LazyColumn(Modifier.fillMaxSize().testTag("home.list")) {
                                item("inbox.machines") { MachineChips() }
                                if (partition.inboxEmpty) {
                                    item("inbox.clear") {
                                        InboxAllClearCard(
                                            bub = uberBot,
                                            quickProjects = partition.quickJumpProjects(3),
                                            openBub = open,
                                            openProject = { showSec(it) },
                                            openBench = { tab = HomeTab.BENCH },
                                        )
                                    }
                                    // keep Snoozed reachable underneath All clear when items are snoozed
                                    inboxItems(quietInbox(partition.inbox), open, openSwarm)
                                } else {
                                    inboxItems(partition.inbox, open, openSwarm)
                                }
                            }
                        }
                        HomeTab.CREW -> {
                            val bots = DataClient.bots.filter { !it.uber && it.thread.isNotEmpty() }
                            val rooms = DataClient.rooms
                            LazyColumn(Modifier.fillMaxSize().testTag("home.crew")) {
                                uberBot?.let { b ->
                                    item("crew.bub") {
                                        Column(Modifier.fillMaxWidth().padding(12.dp)) {
                                            Row(
                                                Modifier
                                                    .fillMaxWidth()
                                                    .clip(dShapeCard)
                                                    .background(dcs.surface)
                                                    .border(BorderStroke(1.dp, dcs.primary.copy(alpha = 0.45f)), dShapeCard)
                                                    .clickable { open(b.thread) }
                                                    .padding(horizontal = 14.dp, vertical = 12.dp)
                                                    .testTag("crew.bub"),
                                                verticalAlignment = Alignment.CenterVertically,
                                            ) {
                                                if (DataClient.has("cats")) BotCat(b.look, b.color, 52.dp)
                                                else Box(Modifier.size(44.dp).background(dcs.primary.copy(alpha = 0.14f), dShape), contentAlignment = Alignment.Center) { DMono("@", dcs.primary, 18, bold = true) }
                                                Spacer(Modifier.width(12.dp))
                                                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                                    DMono(b.name, dcs.onSurface, 16, bold = true)
                                                    DMono(onMachine(b.note.ifEmpty { Msg.android_kind_uberbot }, b.mark), dcs.onSurfaceVariant, 11)
                                                }
                                                Icon(Icons.Default.ChevronRight, null, tint = dcs.primary, modifier = Modifier.size(18.dp))
                                            }
                                        }
                                    }
                                }
                                if (bots.isNotEmpty()) {
                                    item("crew.bots.head") { DMono(Msg.plane_heading_bots.uppercase(), dcs.onSurfaceVariant, 11, Modifier.fillMaxWidth().padding(start = 12.dp, top = 8.dp, bottom = 4.dp), bold = true) }
                                    items(bots, key = { "cb:${it.id}" }) { b ->
                                        Column(Modifier.fillMaxWidth()) {
                                            Row(
                                                Modifier
                                                    .fillMaxWidth()
                                                    .heightIn(min = 56.dp)
                                                    .combinedClickable(onClick = { open(b.thread) }, onLongClick = if (b.delete) ({ botDelete = b }) else null)
                                                    .padding(horizontal = 12.dp, vertical = 8.dp)
                                                    .testTag("side.@${b.name}"),
                                                verticalAlignment = Alignment.CenterVertically,
                                            ) {
                                                if (DataClient.has("cats")) BotCat(b.look, b.color, 34.dp)
                                                else DMono("@", dcs.primary, 14, bold = true)
                                                Spacer(Modifier.width(12.dp))
                                                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                                    DMono(b.name, dcs.onSurface, 14)
                                                    DMono(onMachine(b.note.ifEmpty { Msg.android_kind_bot }, b.mark), dcs.onSurfaceVariant, 11)
                                                }
                                                Icon(Icons.Default.ChevronRight, null, tint = dfaint, modifier = Modifier.size(16.dp))
                                            }
                                            DLine()
                                        }
                                    }
                                }
                                if (rooms.isNotEmpty()) {
                                    item("crew.rooms.head") { DMono(Msg.plane_heading_rooms.uppercase(), dcs.onSurfaceVariant, 11, Modifier.fillMaxWidth().padding(start = 12.dp, top = 14.dp, bottom = 4.dp), bold = true) }
                                    items(rooms, key = { "cr:${it.id}" }) { r ->
                                        Column(Modifier.fillMaxWidth()) {
                                            Row(
                                                Modifier
                                                    .fillMaxWidth()
                                                    .heightIn(min = 56.dp)
                                                    .combinedClickable(onClick = { openRoom(r) }, onLongClick = if (r.delete) ({ roomDelete = r }) else null)
                                                    .padding(horizontal = 12.dp, vertical = 8.dp)
                                                    .testTag("side.#${r.name}"),
                                                verticalAlignment = Alignment.CenterVertically,
                                            ) {
                                                Box(Modifier.size(32.dp).background(dcs.surfaceContainerHigh, dShape).border(BorderStroke(1.dp, dcs.outlineVariant), dShape), contentAlignment = Alignment.Center) {
                                                    DMono("#", dcs.primary, 14, bold = true)
                                                }
                                                Spacer(Modifier.width(12.dp))
                                                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                                    DMono(r.name, dcs.onSurface, 14)
                                                    DMono(onMachine(Msg.android_kind_room, r.mark), dcs.onSurfaceVariant, 11)
                                                }
                                                Icon(Icons.Default.ChevronRight, null, tint = dfaint, modifier = Modifier.size(16.dp))
                                            }
                                            DLine()
                                        }
                                    }
                                }
                                if (uberBot == null && bots.isEmpty() && rooms.isEmpty()) {
                                    item("crew.empty") {
                                        Column(Modifier.fillMaxWidth().padding(32.dp).testTag("crew.empty"), horizontalAlignment = Alignment.CenterHorizontally) {
                                            Text(if (DataClient.listsLoaded) Msg.mobile_crew_empty else Msg.viewer_loading, color = dcs.onSurfaceVariant, fontFamily = BpUi, fontSize = 15.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
                                        }
                                    }
                                }
                            }
                        }
                        HomeTab.BENCH -> {
                            val hist = DataClient.history
                            Column(Modifier.fillMaxSize()) {
                                Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 4.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                                    if (DataClient.has("search") && !searchOpen) {
                                        Row(
                                            Modifier
                                                .weight(1f)
                                                .heightIn(min = 40.dp)
                                                .clip(dShape)
                                                .background(dview)
                                                .border(BorderStroke(1.dp, dcs.outlineVariant), dShape)
                                                .clickable { searchOpen = true }
                                                .padding(horizontal = 10.dp)
                                                .testTag("home.bench.search"),
                                            verticalAlignment = Alignment.CenterVertically,
                                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                                        ) {
                                            Icon(Icons.Default.Search, null, tint = dfaint, modifier = Modifier.size(16.dp))
                                            DText(Msg.android_search_hint, dfaint, 15, lines = 1)
                                        }
                                    } else Spacer(Modifier.weight(1f))
                                    IconButton(onClick = addProject, modifier = Modifier.testTag("home.add")) { Icon(Icons.Default.Add, Msg.sidebar_add_project, tint = dcs.onSurfaceVariant) }
                                }
                                MachineChips()
                                DLine()
                                LazyColumn(Modifier.fillMaxSize().testTag("home.bench"), state = listState) {
                                    if (partition.bench.isEmpty() && DataClient.projects.isEmpty()) {
                                        item("bench.empty") {
                                            Column(Modifier.fillMaxWidth().padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                                                Text(if (DataClient.listsLoaded) Msg.android_projects_none else Msg.viewer_loading, color = dcs.onSurface, fontFamily = BpUi, fontSize = 15.sp)
                                                if (DataClient.listsLoaded) { Spacer(Modifier.height(12.dp)); DBtn(Msg.android_project_add, true, true, onClick = addProject) }
                                            }
                                        }
                                    } else {
                                        benchItems(partition.bench, open, { showSec(it) }, { secMenu = it }, { secRename = it }, openSwarm)
                                    }
                                    if (hist.count > 0) {
                                        item("history") {
                                            Row(Modifier.fillMaxWidth().height(44.dp).clickable { DataClient.showHistory(!DataClient.historyOpen) }.padding(start = 12.dp).testTag("home.history"), verticalAlignment = Alignment.CenterVertically) {
                                                Icon(if (DataClient.historyOpen) Icons.Default.ExpandMore else Icons.Default.ChevronRight, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(6.dp))
                                                DMono(hist.title.uppercase(), dcs.onSurfaceVariant, 11, Modifier.weight(1f))
                                            }
                                        }
                                        items(hist.shown(DataClient.historyOpen), key = { "h:${it.id}" }) { t -> ThreadRow(t, { open(t.id) }) }
                                        if (DataClient.historyOpen && hist.next != null) item("history.more") {
                                            val more = rememberCoroutineScope()
                                            Row(Modifier.fillMaxWidth().height(36.dp).clickable { more.launch { DataClient.loadHistory(more = true) } }.padding(start = 34.dp).testTag("home.history.more"), verticalAlignment = Alignment.CenterVertically) { DMono(Msg.common_more, dfaint, 11) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ReconnectingPill()
                }
            }
            // the keyboard up (search): the tabs give their room to the list
            @OptIn(ExperimentalLayoutApi::class)
            val imeUp = WindowInsets.isImeVisible
            if (!imeUp) DLine()
            if (!imeUp) NavigationBar(containerColor = dcs.surfaceVariant, tonalElevation = 0.dp, modifier = Modifier.testTag("home.tabs")) {
                NavigationBarItem(
                    selected = tab == HomeTab.INBOX,
                    onClick = { tab = HomeTab.INBOX; searchOpen = false; q = "" },
                    icon = {
                        BadgedBox(badge = { if (partition.needsCount > 0) Badge(containerColor = dcs.error, contentColor = dcs.onError) { DMono("${partition.needsCount}", dcs.onError, 10, bold = true) } }) {
                            Icon(Icons.Default.Inbox, Msg.mobile_tab_inbox)
                        }
                    },
                    label = { Text(Msg.mobile_tab_inbox, color = if (tab == HomeTab.INBOX) dcs.primary else dcs.onSurfaceVariant, fontFamily = BpUi, fontSize = 12.sp, fontWeight = if (tab == HomeTab.INBOX) FontWeight.Bold else FontWeight.Medium, maxLines = 1) },
                    colors = NavigationBarItemDefaults.colors(selectedIconColor = dcs.primary, unselectedIconColor = dcs.onSurfaceVariant, indicatorColor = dcs.primary.copy(alpha = 0.14f)),
                    modifier = Modifier.testTag("home.tab.inbox"),
                )
                NavigationBarItem(
                    selected = tab == HomeTab.CREW,
                    onClick = { tab = HomeTab.CREW; searchOpen = false; q = "" },
                    icon = { Icon(Icons.Default.Groups, Msg.mobile_tab_crew) },
                    label = { Text(Msg.mobile_tab_crew, color = if (tab == HomeTab.CREW) dcs.primary else dcs.onSurfaceVariant, fontFamily = BpUi, fontSize = 12.sp, fontWeight = if (tab == HomeTab.CREW) FontWeight.Bold else FontWeight.Medium, maxLines = 1) },
                    colors = NavigationBarItemDefaults.colors(selectedIconColor = dcs.primary, unselectedIconColor = dcs.onSurfaceVariant, indicatorColor = dcs.primary.copy(alpha = 0.14f)),
                    modifier = Modifier.testTag("home.tab.crew"),
                )
                NavigationBarItem(
                    selected = tab == HomeTab.BENCH,
                    onClick = { tab = HomeTab.BENCH; searchOpen = false; q = "" },
                    icon = { Icon(Icons.Default.DeveloperBoard, Msg.mobile_tab_bench) },
                    label = { Text(Msg.mobile_tab_bench, color = if (tab == HomeTab.BENCH) dcs.primary else dcs.onSurfaceVariant, fontFamily = BpUi, fontSize = 12.sp, fontWeight = if (tab == HomeTab.BENCH) FontWeight.Bold else FontWeight.Medium, maxLines = 1) },
                    colors = NavigationBarItemDefaults.colors(selectedIconColor = dcs.primary, unselectedIconColor = dcs.onSurfaceVariant, indicatorColor = dcs.primary.copy(alpha = 0.14f)),
                    modifier = Modifier.testTag("home.tab.bench"),
                )
            }
        } else {
        Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 4.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (DataClient.has("search")) Box(Modifier.weight(1f)) { DField(q, { q = it }, Msg.android_search_hint, Modifier.testTag("home.searchfield")) } else Spacer(Modifier.weight(1f))
            IconButton(onClick = addProject, modifier = Modifier.testTag("home.add")) { Icon(Icons.Default.Add, Msg.sidebar_add_project, tint = dcs.onSurfaceVariant) }
        }
        MachineChips()
        DLine()
        if (DataClient.locks.isNotEmpty()) LocksStrip(open)
        if (q.isNotBlank() && DataClient.has("search")) { SearchResults(q, open); return@Column }
        val botThreads = DataClient.bots.map { it.thread }.toSet()
        val projects = DataClient.projects; val swarmsOn = Swarms.on()
        // a swarm's members are left out of their project's list: the project shows the swarm once (feature swarm)
        val threads = DataClient.threads.filter { !it.archived && it.id !in botThreads && !it.history && !(swarmsOn && it.swarm.isNotEmpty()) }
        val hist = DataClient.history
        Box(Modifier.weight(1f).fillMaxWidth()) {
        if (projects.isEmpty() && threads.isEmpty() && hist.count == 0) {
            // no project yet: Bub still leads the list; the empty state and Add a project come under it
            LazyColumn(Modifier.fillMaxSize().testTag("home.list")) {
                items(DataClient.bots.filter { it.uber && it.thread.isNotEmpty() }, key = { "u:${it.id}" }) { b -> SidebarRow("@", b.name, onMachine(Msg.android_kind_uberbot, b.mark), lead = botCat(b)) { open(b.thread) } }
                item("empty") {
                    Column(Modifier.fillMaxWidth().padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        Text(if (DataClient.listsLoaded) Msg.android_projects_none else Msg.viewer_loading, color = dcs.onSurface, fontFamily = BpUi, fontSize = 15.sp)
                        if (DataClient.listsLoaded) { Spacer(Modifier.height(12.dp)); DBtn(Msg.android_project_add, true, true, onClick = addProject) }
                    }
                }
            }
        } else LazyColumn(Modifier.fillMaxSize().testTag("home.list"), state = listState) {
            val uber = DataClient.bots.filter { it.uber && it.thread.isNotEmpty() }
            items(uber, key = { "u:${it.id}" }) { b -> SidebarRow("@", b.name, onMachine(b.note.ifEmpty { Msg.android_kind_uberbot }, b.mark), lead = botCat(b)) { open(b.thread) } }
            var at = uber.size; rowOf.clear()
            for (p in projects) {
                val closed = p.id in shut
                rowOf[p.id] = at; at += 1
                val sw = if (swarmsOn) SwarmUi.live(Swarms.all, p.id) else emptyList()
                if (!closed) { at += sw.size }
                if (!closed) { val g0 = ThreadGroups.split(threads, p.id); val n0 = ThreadGroups.count(p.settledCount, g0.settled.size); at += ThreadGroups.tree(g0.active, g0.all, treeOpen).size + (if (g0.active.isEmpty() && n0 == 0 && sw.isEmpty()) 1 else 0) + (if (n0 > 0) 1 else 0)
                    if (n0 > 0 && p.id in DataClient.settledOpen) at += ThreadGroups.tree(g0.settled, g0.all, treeOpen).size + (if (p.id in DataClient.settledMore) 1 else 0) }
                item(key = "p:${p.id}") {
                    Row(Modifier.fillMaxWidth().height(40.dp).background(if (flash == p.id) dcs.primary.copy(alpha = .18f) else Color.Transparent).combinedClickable(onClick = {
                        shut = if (closed) shut - p.id else shut + p.id; prefs.edit().putStringSet("projects.shut", shut).apply()
                    }, onLongClick = { projMenu = p; projSheet = true }).padding(start = 10.dp, end = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(if (closed) Icons.Default.ChevronRight else Icons.Default.ExpandMore, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(6.dp))
                        // a project named Backplane: the app's own logo (the hub's `backplane`, which no badge sheet offers)
                        if (p.icon == "backplane") androidx.compose.foundation.Image(androidx.compose.ui.res.painterResource(if (dcs.background.luminance() > 0.5f) R.drawable.brand_light else R.drawable.brand_dark), null, Modifier.size(15.dp).testTag("proj.logo.${p.id}"))
                        // the project's icon in its colour (the hub's; an unknown name is the folder)
                        else Icon(badgeGlyph(p.icon), null, tint = runCatching { androidx.compose.ui.graphics.Color(android.graphics.Color.parseColor(p.css)) }.getOrDefault(dok), modifier = Modifier.size(15.dp).testTag("proj.badge.${p.id}"))
                        Spacer(Modifier.width(8.dp))
                        DMono(p.title.uppercase(), dcs.onSurfaceVariant, 12)
                        // another machine's project says where it is ("on desktop"); one that is also on a linked machine says so ("also on desktop")
                        val where = p.mark?.let { Msg.android_machine_on(it.short) + if (it.away) " · " + Msg.android_away else "" } ?: Tr.t(p.also)
                        if (where.isNotEmpty()) { Spacer(Modifier.width(8.dp)); DMono(where, dfaint, 11, Modifier.testTag("proj.where.${p.id}")) }
                        Spacer(Modifier.weight(1f))
                        if (p.waiting > 0) { DMono("${p.waiting}", dcs.error, 11, bold = true); Spacer(Modifier.width(6.dp)) }
                        IconButton(onClick = { if (p.newOn.size > 1) pickNew = p else newThread(p.id) }, modifier = Modifier.size(40.dp).testTag("proj.new.${p.id}")) { Icon(Icons.Default.Add, Msg.android_thread_new_in(p.title), tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)) }
                    }
                }
                if (!closed) {
                    items(sw, key = { "s:${it.id}" }) { x -> SwarmRow(x) { openSwarm(x.id) } }
                    val g = ThreadGroups.split(threads, p.id)
                    val n = ThreadGroups.count(p.settledCount, g.settled.size)
                    items(ThreadGroups.tree(g.active, g.all, treeOpen), key = { "t:${it.t.id}" }) { r -> TreeRow(r, treeOpen, { treeOpen = it; prefs.edit().putStringSet("tree.open", it).apply() }, open) }
                    if (g.active.isEmpty() && n == 0 && sw.isEmpty()) item("none:${p.id}") { DMono(Msg.android_threads_none, dfaint, 12, Modifier.padding(start = 34.dp, top = 6.dp, bottom = 8.dp)) }
                    if (n > 0) {
                        val on = p.id in DataClient.settledOpen
                        item("settled:${p.id}") {
                            Row(Modifier.fillMaxWidth().height(32.dp).clickable { DataClient.setSettledOpen(p.id, !on) }.padding(start = 28.dp).testTag("home.settled.${p.id}"), verticalAlignment = Alignment.CenterVertically) {
                                Icon(if (on) Icons.Default.ExpandMore else Icons.Default.ChevronRight, null, tint = dfaint, modifier = Modifier.size(14.dp)); Spacer(Modifier.width(6.dp)); DMono(ThreadGroups.title(n), dfaint, 11)
                            }
                        }
                        if (on) {
                            items(ThreadGroups.tree(g.settled, g.all, treeOpen), key = { "t:${it.t.id}" }) { r -> TreeRow(r, treeOpen, { treeOpen = it; prefs.edit().putStringSet("tree.open", it).apply() }, open) }
                            if (p.id in DataClient.settledMore) item("settled.more:${p.id}") {
                                val more = rememberCoroutineScope()
                                Row(Modifier.fillMaxWidth().height(32.dp).clickable { more.launch { DataClient.loadSettled(p.id, more = true) } }.padding(start = 34.dp).testTag("home.settled.more.${p.id}"), verticalAlignment = Alignment.CenterVertically) { DMono(Msg.common_more, dfaint, 11) }
                            }
                        }
                    }
                }
            }
            val known = projects.map { it.id }.toSet()
            items(threads.filter { it.project !in known }, key = { "t:${it.id}" }) { t -> ThreadRow(t, { open(t.id) }) }
            // Conversation history: the old Scratch chats. Its row opens or shuts the list; a chat that wants attention always has its row; nothing starts here
            if (hist.count > 0) {
                item("history") {
                    Row(Modifier.fillMaxWidth().height(40.dp).clickable { DataClient.showHistory(!DataClient.historyOpen) }.padding(start = 10.dp).testTag("home.history"), verticalAlignment = Alignment.CenterVertically) {
                        Icon(if (DataClient.historyOpen) Icons.Default.ExpandMore else Icons.Default.ChevronRight, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(6.dp))
                        DMono(hist.title.uppercase(), dcs.onSurfaceVariant, 12, Modifier.weight(1f))
                    }
                }
                items(hist.shown(DataClient.historyOpen), key = { "h:${it.id}" }) { t -> ThreadRow(t, { open(t.id) }) }
                if (DataClient.historyOpen && hist.next != null) item("history.more") {
                    val more = rememberCoroutineScope()
                    Row(Modifier.fillMaxWidth().height(32.dp).clickable { more.launch { DataClient.loadHistory(more = true) } }.padding(start = 34.dp).testTag("home.history.more"), verticalAlignment = Alignment.CenterVertically) { DMono(Msg.common_more, dfaint, 11) }
                }
            }
            val bots = DataClient.bots.filter { !it.uber }
            if (bots.isNotEmpty()) { item("bots") { DMono(Msg.plane_heading_bots, dfaint, 11, Modifier.fillMaxWidth().padding(start = 12.dp, top = 12.dp, bottom = 4.dp)) }
                items(bots, key = { "b:${it.id}" }) { b -> SidebarRow("@", b.name, onMachine(Msg.android_kind_bot, b.mark), onLong = if (b.delete) ({ botDelete = b }) else null, lead = botCat(b)) { if (b.thread.isNotEmpty()) open(b.thread) } } }
            if (DataClient.rooms.isNotEmpty()) { item("rooms") { DMono(Msg.plane_heading_rooms, dfaint, 11, Modifier.fillMaxWidth().padding(start = 12.dp, top = 12.dp, bottom = 4.dp)) }
                items(DataClient.rooms, key = { "r:${it.id}" }) { r -> SidebarRow("#", r.name, onMachine(Msg.android_kind_room, r.mark), onLong = if (r.delete) ({ roomDelete = r }) else null) { openRoom(r) } } }
        }
        ReconnectingPill()
        }
        } // end !sectioned
        // Delete bot (a long press on a bot the hub offers it for): asked first, then `bot.delete`
        botDelete?.let { b -> BotDeleteDialog(b) { botDelete = null } }
        // Delete room (a long press on a room the hub offers it for): asked first, then `room.delete`
        roomDelete?.let { r ->
            val del = rememberCoroutineScope()
            AlertDialog(shape = dShapeDialog, onDismissRequest = { roomDelete = null }, title = { Text(Msg.native_bot_delete_ask(r.name)) },
                confirmButton = { TextButton({ roomDelete = null; del.launch { runCatching { DataClient.roomDelete(r.id) } } }, Modifier.testTag("room.delete.yes")) { Text(Msg.rooms_delete, color = dcs.error) } },
                dismissButton = { TextButton({ roomDelete = null }) { Text(Msg.common_cancel) } })
        }
        projMenu?.let { ProjectMenus(it, projSheet, { newSwarm = it }) { projSheet = false } }
        newSwarm?.let { pr -> NewSwarmSheet(pr.id, { newSwarm = null }) { openSwarm(it) } }
        secMenu?.let { SectionProjectMenu(it, { pid -> secNewSwarm = pid }) { secMenu = null } }
        secNewSwarm?.let { pid -> NewSwarmSheet(pid, { secNewSwarm = null }) { openSwarm(it) } }
        secRename?.let { SectionRename(it) { secRename = null } }
        pickNew?.let { p -> ActionSheet(p.title, p.newOn.map { n -> (n.label + if (n.away) " · " + Msg.android_away else "") to { pickNew = null; newThread(n.value) } }) { pickNew = null } }
    }
}

/** The machine switcher over the lists: one chip each for every machine, this one and each linked one, in the hub's words (none with no linked machine). */
@Composable
private fun MachineChips() {
    val chips = DataClient.chips; if (chips.isEmpty()) return
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(start = 10.dp, end = 10.dp, bottom = 8.dp).testTag("home.machines"), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        chips.forEach { c ->
            Box(Modifier.clip(dShape).border(BorderStroke(1.dp, if (c.on) dcs.primary else dcs.outlineVariant), dShape).clickable { DataClient.pickMachine(c.value) }.padding(horizontal = 10.dp, vertical = 5.dp).alpha(if (c.away && !c.on) .55f else 1f).testTag("home.machine.${c.value.ifEmpty { "all" }}")) {
                DMono(c.label + if (c.away) " · " + Msg.android_away else "", if (c.on) dcs.primary else dcs.onSurfaceVariant, 11)
            }
        }
    }
}
/** The machine mark of a row: its short name, quiet; dimmer with "away" when that machine did not answer. */
@Composable
private fun MarkText(m: DMark?, tag: String = "") { if (m != null) { DMono(m.short + if (m.away) " · " + Msg.android_away else "", dfaint, 11, Modifier.testTag(tag)); Spacer(Modifier.width(6.dp)) } }

@Composable
internal fun ThreadRowPublic(t: DThread, onClick: () -> Unit) = ThreadRow(t, onClick)
@Composable
private fun TreeRow(r: ThreadGroups.Node, openSet: Set<String>, setOpen: (Set<String>) -> Unit, open: (String) -> Unit) {
    val on = r.t.id in openSet
    ThreadRow(r.t, { open(r.t.id) }, r.depth, r.kids, on) { setOpen(if (on) openSet - r.t.id else openSet + r.t.id) }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ThreadRow(t: DThread, onClick: () -> Unit, depth: Int = 0, kids: Int = 0, open: Boolean = false, toggle: () -> Unit = {}) {
    var menu by remember { mutableStateOf(false) }
    ThreadMenus(t, menu, { menu = false })
    // a subagent or subthread sits one step in under its parent; a parent's chevron (left of its dot, where the project's is) shows or hides its children
    LaunchedEffect(t.id) { DataClient.prefetch(t.id) } // read ahead while it shows (the iOS app's onAppear)
    Row(Modifier.fillMaxWidth()
        // the finger going down reads it again: by the tap's end the thread is usually here (the person: the phone must be fast, 09-26, 11-05)
        .pointerInput(t.id) { awaitEachGesture { awaitFirstDown(requireUnconsumed = false); DataClient.prefetch(t.id, touch = true) } }
        .combinedClickable(onClick = onClick, onLongClick = { menu = true }).padding(start = (8 + 16 * depth.coerceAtMost(4)).dp, end = 12.dp).alpha(if (t.mark?.away == true) .5f else if (t.settled) .55f else 1f).testTag("row.${t.id}"), verticalAlignment = Alignment.Top) {
        Box(Modifier.width(22.dp).height(36.dp).then(if (kids > 0) Modifier.clickable(onClick = toggle).testTag("row.kids.${t.id}") else Modifier), contentAlignment = Alignment.Center) {
            if (kids > 0) Icon(if (open) Icons.Default.ExpandMore else Icons.Default.ChevronRight, if (open) Msg.android_kids_hide(kids) else Msg.android_kids_show(kids), tint = dfaint, modifier = Modifier.size(16.dp))
        }
        Box(Modifier.padding(top = 14.dp)) { StatusDot(t) }
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (t.pinned) { Icon(Icons.Default.PushPin, Msg.tools_pinned, tint = dcs.primary, modifier = Modifier.size(11.dp)); Spacer(Modifier.width(5.dp)) }
                if (t.provider.isNotEmpty()) { ProviderMark(t.provider, 12.dp); Spacer(Modifier.width(6.dp)) }
                Text(t.title.ifEmpty { Msg.thread_new }, color = dcs.onSurface, fontSize = 15.sp, fontFamily = BpUi, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            if (t.preview.isNotEmpty()) Text(t.preview.replace("**", "").replace("`", ""), color = dfaint, fontSize = 13.sp, fontFamily = BpUi, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Spacer(Modifier.width(8.dp))
        Row(Modifier.padding(top = 9.dp), verticalAlignment = Alignment.CenterVertically) {
            if (kids > 0 && !open) { DMono("$kids", dfaint, 11, Modifier.testTag("row.kidcount.${t.id}")); Spacer(Modifier.width(6.dp)) }
            if (t.waiting > 0) { DMono("${t.waiting}", dcs.error, 11, bold = true); Spacer(Modifier.width(6.dp)) }
            MarkText(t.mark, "row.mark.${t.id}")
            DMono(dAgo(t.updatedAt), dfaint)
        }
    }
}

// ---- thread
// The thread's background work: a head that folds, its rows, and the Release, Cancel or Stop a row offers. Open while any is at work.
@Composable
private fun WorkPanel(id: String, w: DWork) {
    var chosen by remember(id) { mutableStateOf<Boolean?>(null) }
    val open = chosen ?: (w.live > 0)
    Column(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp).testTag("thread.work")) {
        DMono(Msg.android_work_head(Tr.t(w.word)).uppercase(), dcs.onSurfaceVariant, 11, Modifier.fillMaxWidth().clickable { chosen = !open }.testTag("thread.work.head"))
        val openSub = LocalOpenSub.current
        if (open) w.rows.forEach { r ->
            // a subagent's row opens its own view (feature subview)
            val sub = r.key.isNotEmpty() && DataClient.has("subview")
            Row(Modifier.fillMaxWidth().clickable(enabled = sub) { openSub(r.key) }.padding(top = 4.dp).testTag(if (sub) "thread.work.sub.${r.key}" else "thread.work.row"), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) { DMono(r.title, dcs.onSurface, 12); if (r.meta.isNotEmpty()) DMono(Tr.t(r.meta), dfaint, 11); DMono(Tr.t(r.doing.ifEmpty { "${r.kind} · ${r.state}" }), dfaint, 11) }
                if (sub) { Icon(Icons.Default.ChevronRight, Msg.android_open, tint = dcs.primary, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(4.dp)) }
                if (r.label.isNotEmpty()) DBtn(Tr.t(r.label), false, modifier = Modifier.testTag("thread.work.${r.act}.${r.value}")) { if (r.act == "open-thread") DataClient.openLink("backplane://thread/${r.value}", id) else DataClient.workAct(id, r) }
            }
        }
    }
}

// Who holds each lock, on the home list; a tap opens the holder's thread.
@Composable
private fun LocksStrip(open: (String) -> Unit) {
    Column(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp).testTag("home.locks")) {
        DMono(Msg.plane_heading_locks, dcs.onSurfaceVariant, 11)
        DataClient.locks.forEach { l ->
            Row(Modifier.fillMaxWidth().clickable(enabled = l.thread.isNotEmpty()) { open(l.thread) }.padding(top = 3.dp).testTag("home.lock.${l.name}"), verticalAlignment = Alignment.CenterVertically) {
                DMono(l.name, dcs.onSurface, 12); Spacer(Modifier.width(8.dp)); DMono(Tr.t(l.line), dfaint, 11, Modifier.weight(1f)); if (l.waiting > 0) DMono(Msg.android_lock_waiting(l.waiting), dfaint, 11)
            }
        }
    }
}

private fun planPhaseLabel(pm: DPlanMode): String = when (pm.phase) {
    "armed" -> if (pm.scope == "once") Msg.gpui_planmode_chip_once else Msg.gpui_mgmt_plan_tab
    "planning" -> Msg.gpui_planmode_top_planning
    "pending" -> Msg.gpui_planmode_chip_review
    "executing" -> Msg.gpui_planmode_chip_building
    else -> Msg.gpui_mgmt_plan_tab
}

private fun planBannerText(pm: DPlanMode): String = when (pm.phase) {
    "armed" -> if (pm.scope == "once") Msg.gpui_planmode_bar_once else Msg.gpui_planmode_bar_always
    "planning" -> Msg.gpui_planmode_bar_planning
    "pending" -> Msg.gpui_planmode_bar_pending
    "executing" -> Msg.gpui_planmode_bar_executing
    else -> ""
}

@Composable
private fun planModeTint(pm: DPlanMode?): Color = when (pm?.phase) {
    "armed", "planning", "pending" -> dinput
    "executing" -> dok
    else -> dcs.primary
}

@Composable
private fun StateStrip(t: DThread) {
    val pm = effectivePlanMode(t)
    Column(Modifier.background(dcs.surfaceVariant)) {
        Row(Modifier.fillMaxWidth().height(32.dp).padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            DMono(Msg.android_state_head, dfaint); Spacer(Modifier.width(8.dp)); StatusDot(t, 6); Spacer(Modifier.width(6.dp)); DMono(statusLabel(dStatusWord(t)), dcs.onSurface, modifier = Modifier.testTag("thread.state"))
            if (pm != null && pm.isActive) {
                val ptint = planModeTint(pm)
                Spacer(Modifier.width(8.dp))
                DMono(
                    planPhaseLabel(pm),
                    ptint,
                    10,
                    Modifier.border(BorderStroke(1.dp, ptint), CircleShape).padding(horizontal = 7.dp, vertical = 1.dp).testTag("thread.planmode"),
                    bold = true,
                )
            }
            Spacer(Modifier.weight(1f))
            if (DataClient.has("actions")) {
                StripCell(if (t.pinned) Msg.menu_unpin else Msg.menu_pin, t.pinned) { DataClient.threadAct(t.id, if (t.pinned) "unpin" else "pin") }
                StripCell(if (t.settled) Msg.android_unsettle else Msg.menu_settle, t.settled) { DataClient.threadAct(t.id, if (t.settled) "unsettle" else "settle") }
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
private fun ThreadScreen(id: String, viewer: () -> Unit, hub: () -> Unit, go: (String) -> Unit, settings: () -> Unit, back: () -> Unit) {
    // the chat's own terminal (feature termChat): a project thread's in its folder, a bot's chat (the uberbot's too) in the home folder
    val ct = rememberChatTerm(RunInTerminal.threadKey(id))
    CompositionLocalProvider(LocalChatTerm provides ct) { ThreadBody(id, viewer, hub, go, settings, back, ct) }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ThreadBody(id: String, viewer: () -> Unit, hub: () -> Unit, go: (String) -> Unit, settings: () -> Unit, back: () -> Unit, chatTerm: ChatTerm?) {
    val scope = rememberCoroutineScope()
    DisposableEffect(id) { DataClient.openThread(id); onDispose { DataClient.closeThread(id) } }
    val thread = DataClient.threads.firstOrNull { it.id == id }
    // what cannot be delivered right away waits in the strip over the composer, not in the conversation: the hub's rows (a far thread's
    // held messages, a queued one), then this phone's own outbox (written with the socket down). A turn running is no reason to wait.
    val queue = (DataClient.queues[id] ?: emptyList()) + DataClient.outboxRows(id)
    val all = DataClient.entries[id] ?: emptyList()
    val list = remember(all, queue) { if (queue.isEmpty()) all else { val q = queue.map { it.msg }.toSet(); all.filter { it.id !in q && (it.clientId == null || it.clientId !in q) } } }
    val running = thread?.state == "working" || thread?.state == "asking"
    val rows = remember(list, running) { DataFold.rows(list, running) }
    val state = rememberLazyListState()
    // the reader's place: the list follows the newest row only while they are at the end of it
    var stick by remember(id) { mutableStateOf(true) }
    LaunchedEffect(state) { snapshotFlow { state.isScrollInProgress to state.canScrollForward }.collect { (moving, more) -> if (moving) stick = !more } }
    suspend fun toEnd() { if (state.layoutInfo.totalItemsCount > 0) state.scrollToItem(state.layoutInfo.totalItemsCount - 1, Int.MAX_VALUE) }
    LaunchedEffect(rows.size, list.lastOrNull()?.text?.length, thread?.state) { if (stick) toEnd() }
    // the keyboard opening: a reader at the end (or within ~80 dp of it) keeps the newest row above the composer while the list shrinks; one who
    // had scrolled up stays where they are
    @OptIn(ExperimentalLayoutApi::class)
    val imeUp = WindowInsets.isImeVisible
    val nearPx = with(androidx.compose.ui.platform.LocalDensity.current) { 80.dp.roundToPx() }
    LaunchedEffect(imeUp) {
        if (!imeUp) return@LaunchedEffect
        val li = state.layoutInfo; val last = li.visibleItemsInfo.lastOrNull()
        if (!stick && (last == null || last.index != li.totalItemsCount - 1 || last.offset + last.size - li.viewportEndOffset > nearPx)) return@LaunchedEffect
        stick = true
        snapshotFlow { state.layoutInfo.viewportSize }.collect { if (stick) toEnd() }
    }
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
        if (ok) { notice = ""; DataVoice.press(id) } else notice = Msg.android_mic_off
    }
    val dictate = DataVoice.available
    LaunchedEffect(id, DataClient.conn, dictate) { if (dictate && DataClient.conn == DataClient.Conn.ONLINE) DataVoice.refresh(id) }
    DisposableEffect(id) { DataVoice.open(id); onDispose { DataVoice.open("") } }
    // the masked box: a name ("" for a new one) while open; the value stays in this box until Save
    var box by remember(id) { mutableStateOf<String?>(null) }
    // /project's folder that holds something: Add existing adds it as it is
    var existing by remember(id) { mutableStateOf<String?>(null) }
    LaunchedEffect(id) { DataClient.refreshSecretAsks(id) }
    val word = DataClient.commandOf(draft)
    fun put(t: String) = DataClient.setField(id, TextFieldValue(t, TextRange(t.length)), typed = false)
    var menu by remember { mutableStateOf(false) }; var models by remember { mutableStateOf(false) }; var finding by remember(id) { mutableStateOf(false) }
    val rowsNow by rememberUpdatedState(rows); var found by remember(id) { mutableStateOf("") } // the row find or a search hit went to, marked
    var order by remember(id) { mutableStateOf<DataClient.Order?>(null) }
    LaunchedEffect(id, list.size, thread?.state, DataClient.conn) { if (DataClient.has("order")) runCatching { DataClient.order(id) }.onSuccess { order = it } }
    // the review bar (#1305): asked again when the thread's entries or turn change (a review starts with a turn; a press anywhere logs a line)
    var review by remember(id) { mutableStateOf<DReviewBar?>(null) }; var reviewing by remember(id) { mutableStateOf(false) }
    LaunchedEffect(id, list.size, thread?.state, DataClient.conn) { if (DataClient.has("reviewmode")) runCatching { DataClient.review(id) }.onSuccess { review = it } }
    LaunchedEffect(id, all.size, thread?.state, DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE) runCatching { DataClient.loadQueue(id) } }
    // Buy it: the hub's button for this thread (a KiCad project or fab files ready to order), asked again when the turn ends
    var buy by remember(id) { mutableStateOf<DataClient.Buy?>(null) }; var buying by remember(id) { mutableStateOf(false) }
    LaunchedEffect(id, thread?.state, DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE && thread?.state != "working") runCatching { DataClient.buyOf(id) }.onSuccess { buy = it } }
    val startBuy: () -> Unit = { if (!buying) { buying = true; scope.launch { try { val t = DataClient.buy(id); if (t.isNotEmpty()) go(t) } catch (e: Exception) { notice = e.message ?: Msg.android_buy_failed } finally { buying = false } } } }
    val focus = androidx.compose.ui.platform.LocalFocusManager.current
    // Choose a file (a datasheet, a CSV): sent to the hub at once, as the desktop's paperclip does; a chip with its name until the message goes
    val pendingFiles = remember(id) { mutableStateListOf<DAttachedFile>() }
    var uploading by remember(id) { mutableStateOf("") }
    val pickFile = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) scope.launch { uploading = uri.lastPathSegment?.substringAfterLast('/').orEmpty().ifEmpty { "…" }; notice = ""
            try { pendingFiles.add(DataClient.attachFile(ctx, id, uri)) } catch (e: Exception) { notice = e.message ?: Msg.upload_failed } finally { uploading = "" } }
    }
    var openRuns by remember(id) { mutableStateOf(emptySet<String>()) }
    thread?.let { t -> ThreadMenus(t, menu, { menu = false }, { back() }, if (DataClient.has("search")) ({ finding = true }) else null, buy?.label?.let { it to startBuy }, opened = { go(it) }); if (models) PickerSheet(t.id, DNewThreadChoice(), {}) { models = false } }
    // go to a message (a find match, a home search hit): its row, loading earlier pages until it is there; marked until the next one or the find bar closes
    var jumping by remember(id) { mutableStateOf<kotlinx.coroutines.Job?>(null) }
    fun jump(msg: String, fade: Boolean) { jumping?.cancel(); jumping = scope.launch {
        var n = 0
        while (n++ < 30) {
            val now = rowsNow; val at = now.indexOfFirst { r -> r is DRow.Entry && (r.entry.id == msg) || r is DRow.Fold && r.fold.items.any { it.id == msg } }
            if (at >= 0) { stick = false; state.scrollToItem(at + if (DataClient.older[id] != null) 1 else 0); found = now[at].id; if (fade) { kotlinx.coroutines.delay(2500); if (found == now[at].id) found = "" }; break }
            if (DataClient.older[id] == null) { notice = Msg.android_message_gone; break }
            DataClient.loadOlder(id); kotlinx.coroutines.delay(400)
        }
    } }
    // a message hit tapped on the home search: once the thread's first page is here
    LaunchedEffect(id, DataClient.pendingMsg, all.isNotEmpty()) { val m = DataClient.pendingMsg; if (m.isNotEmpty() && all.isNotEmpty()) { DataClient.pendingMsg = ""; jump(m, false) } }
    if (finding && !DataClient.has("find")) FindInThread(id, { finding = false }) { msg -> jump(msg, true) }
    Column(Modifier.fillMaxSize()) {
        DBar(thread?.title ?: "", back) {
            // Buy it when there is something to buy; the thread's menu; the viewers in the corner (the person's order, 11-04)
            ChatTermButton(chatTerm)
            buy?.let { b -> IconButton(onClick = startBuy, enabled = !buying, modifier = Modifier.testTag("thread.buy")) { Icon(Icons.Default.ShoppingCart, Tr.t(b.label), tint = dcs.primary) } }
            if (thread != null && DataClient.has("actions")) IconButton(onClick = { menu = true }, modifier = Modifier.testTag("thread.more")) { Icon(Icons.Default.MoreVert, Msg.native_thread_menu, tint = dcs.onSurfaceVariant) }
            if (thread != null && DataClient.has("viewers")) IconButton(onClick = viewer, modifier = Modifier.testTag("thread.viewer")) { Icon(Icons.Default.Dashboard, Msg.thread_viewer, tint = dcs.onSurfaceVariant) }
        }
        ConnBanner(hub, settings)
        thread?.let { StateStrip(it) }
        thread?.mark?.let { m -> DMono(Msg.android_machine_on(m.name) + if (m.away) " · " + Msg.android_away else "", dfaint, 11, Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 3.dp).testTag("thread.machine")) }
        if (finding && DataClient.has("find")) FindBar(id, { finding = false; jumping?.cancel(); found = "" }) { if (it.isEmpty()) { jumping?.cancel(); found = "" } else jump(it, false) }
        DataClient.work[id]?.takeIf { it.rows.isNotEmpty() }?.let { WorkPanel(id, it) }
        LazyColumn(Modifier.weight(1f).fillMaxWidth().testTag("thread.list"), state = state, contentPadding = PaddingValues(vertical = 8.dp)) {
            if (DataClient.older[id] != null) item("older") { Box(Modifier.fillMaxWidth().clickable { DataClient.loadOlder(id) }.padding(10.dp), contentAlignment = Alignment.Center) { DMono(Msg.android_load_earlier, dfaint) } }
            items(rows, key = { it.id }) { r ->
                Box(Modifier.background(if (found == r.id) dcs.primary.copy(alpha = .16f) else Color.Transparent)) {
                when (r) {
                    // a thought: folded to its first line, opened with a tap, kept open or shut as a run of tool calls is
                    is DRow.Entry -> if (r.entry.folds) ThoughtRow(id, r.entry, r.entry.id in openRuns) { openRuns = if (r.entry.id in openRuns) openRuns - r.entry.id else openRuns + r.entry.id } else EntryRow(id, r.entry, goTo = { jump(it, true) }, reply = { DataClient.startReply(id, it) })
                    is DRow.Fold -> FoldRow(r.fold, r.fold.id in openRuns) { openRuns = if (r.fold.id in openRuns) openRuns - r.fold.id else openRuns + r.fold.id }
                }
                }
            }
            if (list.isEmpty() && DataClient.listsLoaded) item("empty") { DMono(Msg.android_thread_empty, dcs.onSurfaceVariant, 13, Modifier.padding(14.dp)) }
            if (thread?.state == "working") item("working") { WorkingRow() }
        }
        DLine()
        if (!id.contains('~')) NoProviderBanner { DataClient.openAccounts = true }
        review?.let { r -> ReviewBar(r, !reviewing) { if (!reviewing) { reviewing = true; scope.launch { try { review = DataClient.reviewSet(id, r.value) } catch (e: Exception) { notice = e.message ?: "" } finally { reviewing = false } } } } }
        order?.let { OrderCard(it) }
        MentionPopup(draft) { put(it) }
        DataClient.btw[id]?.let { b -> BtwCard(b) { DataClient.closeBtw(id) } }
        if (queue.isNotEmpty()) QueueStrip(id, queue) { put(if (draft.isBlank()) it else draft.trimEnd() + "\n\n" + it) }
        val asked = DataClient.secretAsks[id]?.firstOrNull()
        if (box != null || asked != null) SecretBox(id, box ?: asked?.name ?: "", asked?.takeIf { box == null || it.name == box }, { box = null })
        Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (notice.isNotEmpty()) DMono(Tr.t(notice), dwarn, 12)
            existing?.let { dir -> DBtn(Msg.android_hub_cmd_project_existing, false, true, Modifier.testTag("thread.project.existing")) { scope.launch { try { DataClient.addProject(dir); existing = null; notice = ""; put("") } catch (e: Exception) { notice = e.message ?: Msg.android_failed_generic } } } }
            thread?.mark?.let { m -> DMono(Tr.t(m.runs) + if (m.away) " · " + Msg.android_away else "", dfaint, 11, Modifier.testTag("thread.runs")) }
            DataClient.sendError[id]?.let {
                DMono(Tr.t(it), dcs.error, 12, Modifier.testTag("thread.refused"))
                if (DNoProvider.isSendError(it)) DBtn(Msg.provider_notice_button, false, true, Modifier.testTag("thread.refused.accounts")) { DataClient.openAccounts = true }
            }
            PendingPics(pending, "thread")
            if (pendingFiles.isNotEmpty() || uploading.isNotEmpty()) Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                pendingFiles.toList().forEach { f -> FileChip(f.name, Modifier.testTag("thread.pending.file")) { pendingFiles.remove(f) } }
                if (uploading.isNotEmpty()) DMono(Msg.gpui_attach_uploading(uploading), dfaint, 12, Modifier.testTag("thread.pending.uploading"))
            }
            if (dictate) VoiceStrip(id)
            // Reply on a message: the strip over the box (who wrote it, its first line; a tap goes to it, × drops it), and the box takes the keyboard
            val replyBox = remember { androidx.compose.ui.focus.FocusRequester() }
            val replyAt = DataClient.replyFocus; LaunchedEffect(replyAt) { if (replyAt > 0 && id in DataClient.replying) runCatching { replyBox.requestFocus() } }
            DataClient.replying[id]?.let { r -> ReplyStrip(r, go = { jump(r.id, true) }) { DataClient.dropReply(id) } }
            val planMode = effectivePlanMode(thread)
            val planActive = planMode?.isActive == true
            val planTint = planModeTint(planMode)
            val planPrimary = planMode?.phase in setOf("armed", "planning", "pending")
            var planMenu by remember { mutableStateOf(false) }
            if (planMode != null && planActive) {
                Row(
                    Modifier.fillMaxWidth().clip(dShape).background(dcs.surfaceVariant).border(BorderStroke(1.dp, planTint.copy(alpha = 0.45f)), dShape).padding(horizontal = 8.dp, vertical = 5.dp).testTag("thread.plan.banner"),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Box(Modifier.width(2.dp).height(14.dp).background(planTint))
                    DMono(planBannerText(planMode), planTint, 10, Modifier.weight(1f), lines = 2)
                    if (planMode.phase == "armed" || planMode.phase == "planning") {
                        val isOnce = planMode.scope == "once"
                        DMono(
                            if (isOnce) Msg.gpui_planmode_scope_always else Msg.gpui_planmode_scope_once,
                            planTint,
                            10,
                            Modifier.clip(dShape).border(BorderStroke(1.dp, planTint.copy(alpha = 0.6f)), dShape).clickable {
                                scope.launch { runCatching { DataClient.setPlanMode(id, if (isOnce) PlanModeTarget.ALWAYS else PlanModeTarget.ONCE) } }
                            }.padding(horizontal = 6.dp, vertical = 2.dp),
                            bold = true,
                        )
                        DMono(
                            Msg.settings_lang_off,
                            dcs.onSurfaceVariant,
                            10,
                            Modifier.clip(dShape).border(BorderStroke(1.dp, dcs.outlineVariant), dShape).clickable {
                                scope.launch { runCatching { DataClient.setPlanMode(id, PlanModeTarget.OFF) } }
                            }.padding(horizontal = 6.dp, vertical = 2.dp),
                        )
                    }
                }
            }
            DField(field, { DataClient.setField(id, it) }, Msg.android_composer_hint, Modifier.focusRequester(replyBox).testTag("thread.composer"), 6, if (planActive) planTint else dcs.primary)
            thread?.memory?.let { MemoryBar(it) }
            // the context meter, as the desktop composer's "ctx N%", once it is worth a look (from half full; amber from 80%), in the memory bar's form so
            // the tool row keeps its room: a tap shows Context & Prompt
            thread?.ctxPct?.takeIf { it >= 50 }?.let { pct -> var ctxOpen by remember(id) { mutableStateOf(false) }
                val tone = if (pct >= 80) dwarn else dcs.primary
                Row(Modifier.fillMaxWidth().clickable(enabled = DataClient.has("threadContext")) { ctxOpen = true }.testTag("thread.ctx"), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.width(48.dp).height(4.dp).background(dcs.outlineVariant)) { Box(Modifier.fillMaxHeight().fillMaxWidth(pct / 100f).background(tone)) }
                    DMono(Msg.android_ctx_pct(pct), if (pct >= 80) dwarn else dcs.onSurfaceVariant, 11)
                }
                if (ctxOpen) ThreadContextSheet(id) { ctxOpen = false } }
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (thread != null && (DataClient.has("picker") || thread.chip.isNotEmpty())) ModelChip(thread.chip.ifEmpty { "${thread.provider} / ${thread.model.ifEmpty { Msg.common_default }}" }, "thread.model", Modifier.weight(1f), provider = thread.provider) { models = true } else Spacer(Modifier.weight(1f))
                if (DataClient.has("plan") && planMode != null) {
                    val pcol = if (planActive) planTint else dcs.onSurfaceVariant
                    Box(
                        Modifier.height(40.dp).clip(dShape).background(dcs.surfaceVariant).border(BorderStroke(1.dp, if (planActive) planTint else dcs.outlineVariant), dShape)
                            .combinedClickable(
                                onClick = {
                                    val nxt = nextPlanModeCycle(planMode)
                                    scope.launch { runCatching { DataClient.setPlanMode(id, nxt) } }
                                },
                                onLongClick = { planMenu = true },
                            )
                            .padding(horizontal = 8.dp).testTag("thread.plan.toggle"),
                        contentAlignment = Alignment.Center,
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            Icon(Icons.Default.Checklist, Msg.gpui_planmode_tip, tint = pcol, modifier = Modifier.size(16.dp))
                            DMono(if (planActive) planPhaseLabel(planMode) else Msg.gpui_mgmt_plan_tab, pcol, 10, bold = planActive)
                        }
                        DropdownMenu(planMenu, { planMenu = false }) {
                            DropdownMenuItem({ Text(Msg.settings_lang_off, fontFamily = BpUi) }, { planMenu = false; scope.launch { runCatching { DataClient.setPlanMode(id, PlanModeTarget.OFF) } } })
                            DropdownMenuItem({ Text(Msg.gpui_planmode_scope_once, fontFamily = BpUi) }, { planMenu = false; scope.launch { runCatching { DataClient.setPlanMode(id, PlanModeTarget.ONCE) } } })
                            DropdownMenuItem({ Text(Msg.gpui_planmode_scope_always, fontFamily = BpUi) }, { planMenu = false; scope.launch { runCatching { DataClient.setPlanMode(id, PlanModeTarget.ALWAYS) } } })
                        }
                    }
                }
                PhotoTools(pending, { notice = it }, "thread") { close -> DropdownMenuItem({ Text(Msg.android_attach_file, fontFamily = BpUi) }, { close(); pickFile.launch(arrayOf("*/*")) }, Modifier.testTag("thread.attach.file"), leadingIcon = { Icon(Icons.Default.Description, null) }) }
                if (dictate && (DataVoice.always || DataVoice.alwaysWanted)) AlwaysPill()
                if (dictate) MicChip(DataVoice.on(id), held = { begins -> DataVoice.held(id, begins) { if (androidx.core.content.ContextCompat.checkSelfPermission(ctx, android.Manifest.permission.RECORD_AUDIO) == android.content.pm.PackageManager.PERMISSION_GRANTED) DataVoice.press(id) else micAsk.launch(android.Manifest.permission.RECORD_AUDIO) } }) {
                    if (DataVoice.on(id) || androidx.core.content.ContextCompat.checkSelfPermission(ctx, android.Manifest.permission.RECORD_AUDIO) == android.content.pm.PackageManager.PERMISSION_GRANTED) DataVoice.press(id)
                    else micAsk.launch(android.Manifest.permission.RECORD_AUDIO)
                }
                val blank = draft.isBlank() && pending.isEmpty() && pendingFiles.isEmpty()
                if (running && blank) DBtn(Msg.composer_stop, false, modifier = Modifier.height(40.dp).testTag("thread.stop"), danger = true) { DataClient.interrupt(id) }
                else DBtn(word?.label?.let(Tr::t) ?: (if (planPrimary) Msg.gpui_mgmt_plan_tab else Msg.composer_send), true, !blank && uploading.isEmpty(), Modifier.height(40.dp).testTag("thread.send")) {
                    notice = ""; existing = null
                    // sent: the keyboard goes away, so the reply has the screen (a tap on the box brings it back)
                    // (focus first: the keyboard's word in progress is committed before the box empties, never after it)
                    if (word == null) { val text = draft; focus.clearFocus(); val spoken = if (dictate && pending.isEmpty()) DataVoice.dictated(id) else null; if (dictate) DataVoice.sent(id); DataClient.send(id, text, pending.toList(), voice = spoken, reply = DataClient.replying[id], files = pendingFiles.toList()); pending.clear(); pendingFiles.clear(); stick = true }
                    // a command of the composer: the hub runs it; a refusal leaves the draft and says why
                    else { val text = draft.trim(); scope.launch { try {
                        val r = DataClient.runCommand(id, text)
                        when (r.optString("kind")) {
                            "edit" -> put(r.optString("text"))
                            "secret" -> { box = r.optString("name"); put("") }
                            "intent" -> { notice = Msg.intent_saved; put("") }
                            // /project: alone, the Add project screen; made, its first thread opens; a folder that holds something, Add existing under the reason
                            "project" -> { put(""); DataClient.openAddProject = true }
                            // /project <repository address> [message]: Add project's Clone from git on it, the message kept for its first thread
                            "clone" -> { put(""); Clones.ask = CloneAsk(r.optString("repo"), r.optString("text")) }
                            "made" -> { put(""); notice = r.optString("note"); r.optString("thread").takeIf { it.isNotEmpty() }?.let { DataClient.linkTarget = LinkTarget(LinkTarget.Kind.THREAD, it) } }
                            "exists" -> { notice = r.optString("error"); existing = r.optString("path") }
                            else -> put("")
                        }
                    } catch (e: Exception) { notice = e.message ?: Msg.android_failed_generic } } }
                }
            }
        }
    }
}

/** A run of tool calls as one line (the desktop's fold row): a tap opens it, each call a line of its own. */
@Composable
private fun FoldRow(f: DFold, open: Boolean, toggle: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 3.dp)) {
        Row(Modifier.fillMaxWidth().heightIn(min = 36.dp).clip(dShapeCard).border(BorderStroke(1.dp, dcs.outlineVariant), dShapeCard).clickable(onClick = toggle).padding(horizontal = 8.dp).testTag("fold.${f.id}"), verticalAlignment = Alignment.CenterVertically) {
            Icon(if (open) Icons.Default.ExpandMore else Icons.Default.ChevronRight, if (open) Msg.common_hide else Msg.common_show, tint = dcs.onSurfaceVariant, modifier = Modifier.size(15.dp)); Spacer(Modifier.width(8.dp))
            DMono(Tr.t(f.label), dcs.onSurfaceVariant, 12)
        }
        if (open) f.items.forEach { ToolRow(it) }
    }
}

/** A thought the agent sent (data/1 `folds`, #1325): shut, its first line beside a chevron, as Settings' Show thinking says ("folded"); a tap opens the
 *  whole of it as Markdown and shuts it again. The desktop's screens/thought.rs. */
@Composable
private fun ThoughtRow(tid: String, e: DEntry, open: Boolean, toggle: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 2.dp).testTag("thought.${e.id}")) {
        Row(Modifier.fillMaxWidth().heightIn(min = 32.dp).clickable(onClick = toggle).testTag("thought.head.${e.id}"), verticalAlignment = Alignment.CenterVertically) {
            Icon(if (open) Icons.Default.ExpandMore else Icons.Default.ChevronRight, if (open) Msg.common_hide else Msg.common_show, tint = dfaint, modifier = Modifier.size(14.dp)); Spacer(Modifier.width(6.dp))
            Text(Tr.t(e.label.ifEmpty { "thinking" }), color = dfaint, fontFamily = BpMono, fontSize = 12.sp, fontWeight = FontWeight.Bold)
            if (!open) { Spacer(Modifier.width(8.dp)); Text(DataFold.thoughtLine(e.text), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f)) }
        }
        if (open) Row(Modifier.padding(start = 6.dp, top = 2.dp, bottom = 4.dp).height(IntrinsicSize.Min)) {
            Box(Modifier.width(1.dp).fillMaxHeight().background(dcs.outlineVariant)); Spacer(Modifier.width(10.dp))
            Box(Modifier.alpha(.85f).testTag("thought.body.${e.id}")) { MdText(e.text, thread = tid, copy = CopyLookup.of(e.copy, DataClient.has("copy"), false)) }
        }
    }
}

@Composable
private fun ToolRow(e: DEntry) {
    var whole by remember(e.id) { mutableStateOf(false) }
    val step = e.kind == "step"
    val name = if (step) "" else e.tool?.name ?: "tool"
    val what = if (step) Tr.t(e.label.ifEmpty { "Design change" }) else if (e.tool?.summary.isNullOrEmpty()) "" else e.text
    // an Agent call opens its subagent's own view (feature subview); any other call shows all of itself
    val sub = e.tool?.sub.orEmpty().takeIf { it.isNotEmpty() && DataClient.has("subview") }; val openSub = LocalOpenSub.current
    Row(Modifier.fillMaxWidth().clickable { if (sub != null) openSub(sub) else whole = !whole }.padding(start = 8.dp, top = 3.dp, bottom = 3.dp).testTag(if (sub != null) "tool.sub.$sub" else "tool.${e.id}"), verticalAlignment = Alignment.CenterVertically) {
        if (name.isNotEmpty()) { Text(Tr.t(name), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, fontWeight = FontWeight.Bold); Spacer(Modifier.width(8.dp)) }
        Text(what, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, maxLines = if (whole) Int.MAX_VALUE else 3, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
        if (sub != null) { Spacer(Modifier.width(8.dp)); DMono(Msg.android_open, dcs.primary, 12); Icon(Icons.Default.ChevronRight, null, tint = dcs.primary, modifier = Modifier.size(14.dp)) }
    }
}

/** The working square: breathes while the turn runs (the one animation on screen, and only while this row is). */
@Composable
private fun WorkingRow() {
    val t = androidx.compose.animation.core.rememberInfiniteTransition(label = "working")
    val a by t.animateFloat(0f, 1f, androidx.compose.animation.core.infiniteRepeatable(androidx.compose.animation.core.tween(550), androidx.compose.animation.core.RepeatMode.Reverse), label = "breath")
    Row(Modifier.padding(horizontal = 14.dp, vertical = 8.dp).testTag("thread.working"), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(12.dp), contentAlignment = Alignment.Center) { Box(Modifier.size((5 + 7 * a).dp).alpha(.45f + .55f * a).background(dok)) }
        Spacer(Modifier.width(8.dp)); DText(Msg.phase_working, dcs.onSurfaceVariant, 14, lines = 1)
    }
}

@Composable
private fun BtwCard(b: DataClient.Btw, close: () -> Unit) {
    Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 12.dp, vertical = 8.dp).testTag("thread.btw")) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            DMono(Msg.native_composer_btw(b.q), modifier = Modifier.weight(1f))
            IconButton(onClick = close, modifier = Modifier.testTag("thread.btw.close")) { Icon(Icons.Default.Close, Msg.common_close, tint = dcs.onSurfaceVariant) }
        }
        Text(b.a, color = dcs.onSurface, fontSize = 13.sp, modifier = Modifier.heightIn(max = 220.dp).verticalScroll(rememberScrollState()))
    }
}

/** The masked box of a chat: `/secret`, or an agent's request. The value is held here and leaves once, as `secret.set`; it is never a draft. */
@Composable
private fun SecretBox(tid: String, name0: String, asked: DataClient.SecretAsk?, shut: () -> Unit) {
    val scope = rememberCoroutineScope()
    var name by remember(tid, name0) { mutableStateOf(name0) }; var value by remember(tid, name0) { mutableStateOf("") }; var err by remember(tid, name0) { mutableStateOf("") }
    Column(Modifier.fillMaxWidth().clip(dShapeCard).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.primary), dShapeCard).padding(12.dp).testTag("thread.secret"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(if (name0.isEmpty()) Msg.android_secret_new else Msg.android_secret_named(name0), color = dcs.onSurface, fontSize = 13.sp)
        asked?.purpose?.takeIf { it.isNotEmpty() }?.let { DMono(it) }
        if (name0.isEmpty()) DField(name, { name = it }, Msg.bots_name, Modifier.testTag("thread.secret.name"), mono = true)
        Box(Modifier.fillMaxWidth().clip(dShape).background(dcs.surface).border(BorderStroke(1.dp, dcs.outlineVariant), dShape).padding(12.dp)) {
            if (value.isEmpty()) Text(Msg.android_secret_paste, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 13.sp)
            BasicTextField(value, { value = it }, textStyle = TextStyle(color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp), cursorBrush = SolidColor(dcs.primary), maxLines = 1,
                visualTransformation = PasswordVisualTransformation(), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false), modifier = Modifier.fillMaxWidth().testTag("thread.secret.value"))
        }
        if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 12)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DBtn(Msg.common_save, true, name.isNotBlank() && value.isNotBlank(), Modifier.testTag("thread.secret.save")) { scope.launch { try { DataClient.saveSecret(tid, name.trim(), value, asked?.purpose ?: ""); value = ""; shut() } catch (e: Exception) { err = e.message ?: Msg.android_secret_refused } } }
            DBtn(Msg.common_cancel, false, true, Modifier.testTag("thread.secret.cancel")) { value = ""; shut() }
            if (asked != null) DBtn(Msg.android_secret_notneeded, false, true) { scope.launch { runCatching { DataClient.dismissSecret(tid, asked.name) }; value = ""; shut() } }
        }
    }
}

/** Reply's strip over the composer (the desktop's `reply-strip`): who wrote the message and its first line; a tap goes to it, × drops the reply. */
@Composable
private fun ReplyStrip(r: DReplyRef, go: () -> Unit, drop: () -> Unit) {
    Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min).clip(dShape).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant), dShape).testTag("thread.reply.strip"), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.width(2.dp).fillMaxHeight().background(dcs.primary))
        Column(Modifier.weight(1f).clickable(onClick = go).padding(horizontal = 8.dp, vertical = 4.dp)) {
            DMono(if (ReplyDraft.mine(r)) Msg.secret_you else Msg.model_col_agent, dcs.primary, 11, bold = true)
            DMono(Msg.gpui_b_replying_to(ReplyDraft.firstLine(r.text)), dcs.onSurfaceVariant, 11, lines = 1)
        }
        IconButton(onClick = drop, modifier = Modifier.size(36.dp).testTag("thread.reply.drop")) { Icon(Icons.Default.Close, Msg.reply_cancel, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)) }
    }
}

/** Reply by a swipe right, as messaging apps do: the message follows the finger, the arrow shows; let go past it (ReplyDraft.swipe), Reply. The list still scrolls. */
@Composable
private fun Modifier.replySwipe(r: DReplyRef?, reply: (DReplyRef) -> Unit): Modifier {
    if (r == null) return this
    val density = androidx.compose.ui.platform.LocalDensity.current
    val haptic = androidx.compose.ui.platform.LocalHapticFeedback.current
    var pull by remember(r.id) { mutableStateOf(0f) }
    return this.pointerInput(r.id) {
        detectHorizontalDragGestures(onDragEnd = { if (ReplyDraft.swipe(with(density) { pull.toDp().value }, 0f)) { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress); reply(r) }; pull = 0f },
            onDragCancel = { pull = 0f }) { ch, dx -> val n = (pull + dx).coerceIn(0f, with(density) { 80.dp.toPx() }); if (n != pull || n > 0f) ch.consume(); pull = n }
    }.offset { androidx.compose.ui.unit.IntOffset(pull.toInt(), 0) }
}

/** Long-press on a message: Reply and Copy (the desktop's two under a message), then whatever else it has. */
@Composable
private fun ReplyMenuItems(e: DEntry, r: DReplyRef?, close: () -> Unit, reply: (DReplyRef) -> Unit) {
    val clip = androidx.compose.ui.platform.LocalClipboardManager.current
    r?.let { DropdownMenuItem({ Text(Msg.reply_action, fontFamily = BpUi) }, { close(); reply(it) }, Modifier.testTag("thread.entry.reply.menu"), leadingIcon = { Icon(Icons.AutoMirrored.Filled.Reply, null) }) }
    if (e.text.isNotEmpty()) DropdownMenuItem({ Text(Msg.common_copy, fontFamily = BpUi) }, { close(); clip.setText(androidx.compose.ui.text.AnnotatedString(e.text)) }, Modifier.testTag("thread.entry.copy"), leadingIcon = { Icon(Icons.Default.ContentCopy, null) })
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun EntryRow(tid: String, e: DEntry, goTo: (String) -> Unit = {}, reply: (DReplyRef) -> Unit = {}) {
    val rr = if (DataClient.has("reply")) ReplyDraft.of(e.id, e.kind, e.text, e.live) else null
    val replyLink: @Composable () -> Unit = { rr?.let { DMono(Msg.reply_action, dfaint, 11, Modifier.clickable { reply(it) }.testTag("thread.entry.reply")) } }
    when (e.kind) {
        "user" -> Column(Modifier.fillMaxWidth().replySwipe(rr, reply).padding(start = 32.dp, end = 12.dp, top = 6.dp, bottom = 6.dp), horizontalAlignment = Alignment.End) {
            // a reply: what it answers, a tap goes there
            e.replyTo?.let { r -> DMono(Msg.gpui_b_in_reply_to(r.quote), dfaint, modifier = Modifier.clickable { goTo(r.id) }.padding(bottom = 3.dp).testTag("thread.entry.replyto")) }
            e.localImages.forEach { Pic(it, 160); Spacer(Modifier.height(4.dp)) }
            e.attachments.filter { e.localImages.isEmpty() }.forEach { PicById(it.id, 160); Spacer(Modifier.height(4.dp)) }
            e.attached.forEach { FileChip(it.name, Modifier.padding(bottom = 4.dp).testTag("thread.entry.file")) }
            // long-press: Reply, Copy, and Restore to before this when the hub has the snapshot before the turn this message started (asked first, with the snapshot's info)
            var menu by remember { mutableStateOf(false) }; var asking by remember { mutableStateOf(false) }
            val press = if (e.restore != null || rr != null) Modifier.combinedClickable(onClick = {}, onLongClick = { menu = true }) else Modifier
            if (e.text.isNotEmpty()) Box(Modifier.clip(dShapeCard).then(press).background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant), dShapeCard).padding(horizontal = 10.dp, vertical = 8.dp)) {
                LinkText(e.text, tid, color = dcs.onSurface, size = 14, lineHeight = 20, mono = false)
                DropdownMenu(menu, { menu = false }) {
                    ReplyMenuItems(e, rr, { menu = false }, reply)
                    if (e.restore != null) DropdownMenuItem({ Text(Msg.thread_restore_action, fontFamily = BpUi) }, { menu = false; asking = true }, Modifier.testTag("thread.entry.restore"))
                }
            }
            e.restore?.takeIf { asking }?.let { r ->
                AlertDialog(shape = dShapeDialog, onDismissRequest = { asking = false }, title = { Text(Msg.thread_restore_ask) },
                    text = { Column { Text(r.info, fontFamily = BpMono, fontSize = 12.sp); Spacer(Modifier.height(6.dp)); Text(Msg.thread_restore_note, fontSize = 13.sp) } },
                    confirmButton = { TextButton({ asking = false; DataClient.restore(tid, r) }, Modifier.testTag("thread.restore.yes")) { Text(Msg.thread_restore_go, color = dcs.error) } },
                    dismissButton = { TextButton({ asking = false }) { Text(Msg.common_cancel) } })
            }
            Row(Modifier.padding(top = 3.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                // spoken: the mic, and how sure the engine was (edited when typed in); just sent by voice: Undo, for a moment
                e.voice?.let { v ->
                    Icon(Icons.Default.Mic, Msg.voice_mark_tip(v.stt, ((v.conf ?: 0.0) * 100).toInt().toString()), tint = dfaint, modifier = Modifier.size(12.dp).testTag("thread.entry.voice"))
                    v.conf?.let { c -> DMono("${(c * 100).toInt()}%" + if (v.edited) " · " + Msg.voice_mark_edited else "", dfaint) }
                }
                DataVoice.recent?.takeIf { it.tid == tid && (e.clientId == it.cid || e.id == it.cid || e.id == "local:" + it.cid) }?.let {
                    DMono(Msg.voice_cmd_undo, dcs.primary, modifier = Modifier.clickable { DataVoice.unsend() }.testTag("thread.entry.undo"))
                }
                if (e.sending) DMono(Msg.thread_sending, dfaint)
                else if (e.failed) DMono(Msg.android_send_retry, dcs.error, modifier = Modifier.clickable { DataClient.retry(tid, e) })
                else { if (e.text.isNotBlank()) CopyButton(e.text, word = true, tag = "thread.entry.copy", size = 12); replyLink(); DMono(dStamp(e.at), dfaint) }
            }
        }
        "assistant" -> Column(Modifier.fillMaxWidth().replySwipe(rr, reply).padding(horizontal = 14.dp, vertical = 6.dp)) {
            MdText(e.text, thread = tid, copy = CopyLookup.of(e.copy, DataClient.has("copy"), e.live))
            // the pictures the reply names (the hub lists them as attachments or `images`), tap for the full size
            e.attachments.forEach { Spacer(Modifier.height(6.dp)); PicById(it.id, 240, zoom = true) }
            e.images.forEach { Spacer(Modifier.height(6.dp)); PicByReply(e.id, it, 240) }
            // the footer: the time, Reply; a long-press on it (the text keeps its own selection) is Reply and Copy
            if (!e.live) { var menu by remember { mutableStateOf(false) }
                Row(Modifier.padding(top = 4.dp).combinedClickable(onClick = {}, onLongClick = { if (rr != null) menu = true }), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    // Copy the whole message (its Markdown, as the desktop's Copy), beside Reply
                    DMono(dStamp(e.at), dfaint); replyLink(); if (e.text.isNotBlank()) CopyButton(e.text, word = true, tag = "thread.entry.copy", size = 12)
                    DropdownMenu(menu, { menu = false }) { ReplyMenuItems(e, rr, { menu = false }, reply) }
                } }
        }
        "tool", "step" -> Box(Modifier.padding(horizontal = 12.dp)) { ToolRow(e) }
        "ask" -> AskCard(tid, e)
        "handoff" -> HandoffCard(tid, e)
        "term" -> if (e.term != null && termsOn()) TermCard(e)
        else -> Row(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 4.dp)) {
            val c = if (e.tone == "error") dcs.error else dcs.onSurfaceVariant
            if (e.label.isNotEmpty()) { Text(Tr.t(e.label), color = c, fontFamily = BpMono, fontSize = 12.sp, fontWeight = FontWeight.Bold); Spacer(Modifier.width(8.dp)) }
            Column(Modifier.weight(1f)) {
                if (e.text.contains('`') || !e.copy.isNullOrEmpty()) MdText(e.text, thread = tid, copy = CopyLookup.of(e.copy, DataClient.has("copy")), color = c, fontSize = 12, mono = true)
                else LinkText(e.text, tid, color = c, size = 12)
            }
        }
    }
}

/** A `/give` received: the way back, the summary, the note, and what the work touched, each reference opening what it names (the hub's `link.resolve`). */
@Composable
private fun HandoffCard(tid: String, e: DEntry) {
    val h = e.handoff ?: return
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp).clip(dShapeCard).background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant), dShapeCard).padding(10.dp).testTag("handoff.card")) {
        val from = "backplane://thread/${h.from}"
        DMono(Msg.android_handoff_from(h.title), dcs.primary, 11, Modifier.clickable { DataClient.openLink(from, tid) }.testTag("handoff.back"))
        Text(h.summary, color = dcs.onSurface, fontSize = 14.sp, fontFamily = BpUi, modifier = Modifier.padding(top = 6.dp).testTag("handoff.summary"))
        if (h.note.isNotEmpty()) DMono(Msg.android_handoff_note(h.note), dcs.onSurfaceVariant, 12, Modifier.padding(top = 4.dp), lines = 4)
        h.refs.forEachIndexed { i, r ->
            val mark = when (r.kind) { "thread" -> "▸"; "message" -> "“"; "board" -> "▦"; else -> "·" }
            Row(Modifier.fillMaxWidth().padding(top = 6.dp).clip(dShape).border(BorderStroke(1.dp, dcs.outlineVariant), dShape).clickable { DataClient.openLink(if (r.kind == "file" || r.kind == "board") PATH_SCHEME + r.url else r.url, r.thread) }.padding(horizontal = 8.dp, vertical = 6.dp).testTag("handoff.ref.$i")) {
                DMono(mark, dcs.primary, 12, bold = true); Spacer(Modifier.width(8.dp)); DMono(r.label, dcs.onSurface, 12, Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun AskCard(tid: String, e: DEntry) {
    val a = e.ask
    var answer by remember(e.id) { mutableStateOf("") }
    var revising by remember(e.id) { mutableStateOf(false) }
    var revision by remember(e.id) { mutableStateOf("") }
    var expanded by remember(e.id) { mutableStateOf(false) }
    var copied by remember(e.id) { mutableStateOf(false) }
    val clip = androidx.compose.ui.platform.LocalClipboardManager.current
    val scope = rememberCoroutineScope()
    val isPlan = a?.kind == "plan"
    val border = if (a?.open == true) (if (isPlan) dinput else dcs.error) else dcs.outlineVariant

    if (a != null && isPlan) {
        val content = a.question.ifEmpty { e.text }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp)
                .clip(dShapeCard).background(dcs.surfaceVariant).border(BorderStroke(1.dp, border), dShapeCard)
                .height(IntrinsicSize.Min).testTag("ask.card"),
        ) {
            if (a.open) Box(Modifier.width(3.dp).fillMaxHeight().background(dinput))
            Column(Modifier.weight(1f).padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    DMono(if (a.head.isNotEmpty()) Tr.t(a.head) else Msg.ask_plan, dcs.onSurface, 12, bold = true)
                    if (a.open) {
                        DMono(
                            Msg.gpui_ask_plan_ready,
                            dinput,
                            10,
                            Modifier.border(BorderStroke(1.dp, dinput.copy(alpha = 0.65f)), CircleShape).padding(horizontal = 7.dp, vertical = 1.dp),
                            bold = true,
                        )
                    }
                    Spacer(Modifier.weight(1f))
                    DBtn(if (copied) Msg.mobile_plan_copied else Msg.common_copy, false, small = true, modifier = Modifier.testTag("ask.copy")) {
                        clip.setText(androidx.compose.ui.text.AnnotatedString(content))
                        copied = true
                        scope.launch { kotlinx.coroutines.delay(1500); copied = false }
                    }
                    DBtn(if (expanded) Msg.gpui_mgmt_plan_compact else Msg.gpui_mgmt_plan_expand, false, small = true, modifier = Modifier.testTag("ask.expand")) {
                        expanded = !expanded
                    }
                }
                Box(
                    Modifier.fillMaxWidth()
                        .then(if (expanded) Modifier else Modifier.heightIn(max = 260.dp).verticalScroll(rememberScrollState()))
                        .clip(dShape)
                        .background(dcs.background)
                        .border(BorderStroke(1.dp, dcs.outlineVariant), dShape)
                        .padding(10.dp),
                ) {
                    MdText(content, thread = tid, copy = CopyLookup(null))
                }
                if (a.open) {
                    Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        a.options.forEach { o ->
                            val primary = o.id == "accept" || o.id == "implement"
                            DBtn(Tr.t(o.label), primary, true, Modifier.testTag("ask.opt.${o.id}"), small = false) {
                                DataClient.answer(tid, a.id, o.id)
                            }
                        }
                        if (a.revise) {
                            DBtn(Msg.gpui_ask_plan_revise, revising, true, Modifier.testTag("ask.opt.revise"), small = false) {
                                revising = !revising
                            }
                        }
                    }
                    if (a.revise && revising) {
                        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            DField(revision, { revision = it }, Msg.gpui_ask_plan_reviseph, Modifier.testTag("ask.revise.input"), lines = 3)
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                DBtn(Msg.gpui_ask_plan_sendrev, true, revision.isNotBlank(), Modifier.testTag("ask.revise.send"), small = true) {
                                    val fb = revision.trim()
                                    if (fb.isNotEmpty()) {
                                        revising = false
                                        revision = ""
                                        DataClient.answer(tid, a.id, "revise:$fb")
                                    }
                                }
                                DBtn(Msg.common_cancel, false, true, small = true) {
                                    revising = false
                                    revision = ""
                                }
                            }
                        }
                    }
                } else if (a.answered != null) {
                    DMono(Msg.android_ask_answered(a.options.firstOrNull { it.id == a.answered }?.label?.let(Tr::t) ?: a.answered))
                }
            }
        }
        return
    }

    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp).clip(dShapeCard).background(dcs.surfaceVariant).border(BorderStroke(1.dp, border), dShapeCard).padding(12.dp).testTag("ask.card")) {
        DMono(if (a?.open == true) Msg.android_ask_open else Msg.android_ask_done, dfaint, 11)
        Text(a?.question ?: e.text, color = dcs.onSurface, fontSize = 13.sp, fontFamily = BpMono, modifier = Modifier.padding(vertical = 6.dp))
        if (a != null && a.open) {
            if (a.options.isEmpty()) Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) { DField(answer, { answer = it }, Msg.android_ask_answer, Modifier.testTag("ask.answer")) }
                DBtn(Msg.composer_send, true, answer.isNotBlank()) { DataClient.answer(tid, a.id, answer) }
            } else Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                a.options.forEach { o -> DBtn(Tr.t(o.label), o.id == "accept" || o.id == "implement", true, Modifier.fillMaxWidth().testTag("ask.opt.${o.id}"), danger = o.id == "deny" || o.id == "decline") { DataClient.answer(tid, a.id, o.id) } }
            }
        } else if (a?.answered != null) DMono(Msg.android_ask_answered(a.options.firstOrNull { it.id == a.answered }?.label?.let(Tr::t) ?: a.answered))
    }
}

/** A bot row's cat (feature cats; the uberbot's larger, as the desktop's sidebar has it), or null: the row keeps its @. */
private fun botCat(b: DataClient.Bot): (@Composable () -> Unit)? =
    if (!DataClient.has("cats")) null else ({ BotCat(b.look, b.color, if (b.uber) 56.dp else 28.dp) })

/** Delete bot's confirm (a long press on a bot row, or Settings' Bots): the hub's words (`ask`: the bot by name, what goes with it), then `bot.delete`. */
@Composable
internal fun BotDeleteDialog(b: DataClient.Bot, done: () -> Unit) {
    val del = rememberCoroutineScope()
    AlertDialog(shape = dShapeDialog, onDismissRequest = done, title = { Text(b.ask.title.ifEmpty { "" }.let { if (it.isEmpty()) Msg.native_bot_delete_ask(b.name) else Tr.t(it) }) },
        text = if (b.ask.body.isEmpty()) null else ({ Text(Tr.t(b.ask.body), Modifier.testTag("bot.delete.body")) }),
        confirmButton = { TextButton({ done(); del.launch { runCatching { DataClient.botDelete(b.id) } } }, Modifier.testTag("bot.delete.yes")) { Text(if (b.ask.yes.isEmpty()) Msg.bots_delete else Tr.t(b.ask.yes), color = dcs.error) } },
        dismissButton = { TextButton(done) { Text(Msg.common_cancel) } })
}

/** Settings' Bots: this hub's bots the hub offers Delete bot for (`bots.list` rows with `delete`; never the uberbot), each with Delete bot. */
@Composable
private fun BotsSettings() {
    LaunchedEffect(Unit) { runCatching { DataClient.loadBots() } }
    val bots = DataClient.bots.filter { it.delete }
    if (bots.isEmpty()) return
    var ask by remember { mutableStateOf<DataClient.Bot?>(null) }
    DMono(Msg.bots_settings_head.uppercase(), dfaint)
    bots.forEach { b ->
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(b.name, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            DBtn(Msg.bots_delete, false, modifier = Modifier.testTag("settings.bot.delete.${b.id}"), danger = true) { ask = b }
        }
    }
    ask?.let { b -> BotDeleteDialog(b) { ask = null } }
    DLine()
}

// ---- settings, folders, new thread
@Composable
private fun SettingsScreen(back: () -> Unit, hub: () -> Unit, accounts: () -> Unit, mcp: () -> Unit, improve: () -> Unit) {
    Column(Modifier.fillMaxSize()) {
        DBar(Msg.palette_settings, back)
        Column(Modifier.padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            DMono(Msg.native_settings_hubs.uppercase(), dfaint)
            var forget by remember { mutableStateOf<String?>(null) }
            DataClient.links.mapNotNull { Pairing.key(it) }.forEachIndexed { i, k ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    // the hub connected to now: the one in use, or the home hub while that one cannot be reached
                    val on = k == DataClient.usingKey
                    if (on) DDot(if (DataClient.conn == DataClient.Conn.ONLINE) dcs.primary else if (DataClient.conn == DataClient.Conn.CONNECTING) dwarn else dcs.error, 8)
                    Text(k, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (k == DataClient.homeKey) DMono(Msg.native_hub_home, dcs.onSurfaceVariant, 12, Modifier.testTag("settings.hub.home.$k"))
                    if (i == 0 && DataClient.onHome) DMono(Msg.native_hub_unreachable, dcs.onSurfaceVariant, 12)
                    if (i > 0 && !on) DBtn(Msg.android_hub_use, false, modifier = Modifier.testTag("settings.hub.use.$k")) { DataClient.useHub(k); back() }
                    DBtn(Msg.bots_memory_forget, false, modifier = Modifier.testTag("settings.hub.forget.$k"), danger = true) { forget = k }
                }
            }
            DMono(if (DataClient.conn == DataClient.Conn.ONLINE) Msg.android_online else Msg.android_offline(DataClient.failure), dcs.onSurfaceVariant, 12)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                DBtn(Msg.android_hub, false, modifier = Modifier.testTag("settings.hub"), onClick = hub)
                DBtn(Msg.android_hub_add, false, modifier = Modifier.testTag("settings.hub.add")) { DataClient.pairingError = ""; DataClient.adding = true }
            }
            // the uberbot's name and cat (feature cats), beside the agents' settings
            if (DataClient.has("cats")) UberSettings()
            BotsSettings()
            // the agents' accounts, and the MCP servers every agent gets (feature mcpsettings)
            if (DataClient.has("accounts") || DataMcp.available) Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (DataClient.has("accounts")) DBtn(Msg.android_accounts, false, modifier = Modifier.testTag("settings.accounts"), onClick = accounts)
                if (DataMcp.available) McpEntry(mcp)
            }
            forget?.let { k ->
                AlertDialog(shape = dShapeDialog, onDismissRequest = { forget = null }, title = { Text(Msg.android_hub_forget_ask(k)) },
                    text = { Text(Msg.android_hub_forget_note) },
                    confirmButton = { TextButton({ forget = null; DataClient.forgetHub(k); if (DataClient.link == null) back() }) { Text(Msg.bots_memory_forget) } },
                    dismissButton = { TextButton({ forget = null }) { Text(Msg.common_cancel) } })
            }
            DLine()
            UpdateSection()
            DLine()
            DBtn(Msg.android_improve, false, modifier = Modifier.testTag("settings.improve"), onClick = improve)
            DLine()
            if (DataVoice.available) VoiceSettings()
            if (DataDevices.available) { DevicesSettings(); DLine() }
            DataClient.home?.let { h ->
                DMono(Msg.home_machine, dfaint)
                val whereText = when (h.kind) {
                    "local" -> "${h.name.ifEmpty { "This machine" }} (this machine)"
                    "boat" -> "${h.name.ifEmpty { "Boat" }} (Boat)"
                    else -> h.name.ifEmpty { "Linked machine" }
                }
                DMono(whereText, dcs.onSurface, 13)
                h.awayLine()?.let { (line, isErr) -> DMono(line, if (isErr) dcs.error else dcs.onSurfaceVariant, 12) }
                if (h.canMoveHere) {
                    DBtn(Msg.home_move_here, true, modifier = Modifier.testTag("settings.home.move")) { DataClient.moveHomeHere() }
                }
                DLine()
            }
            if (DataMachines.available) { MachinesSettings(); DLine() }
            if (DataNotify.available) { NotifySettings(); DLine() }
            if (DataBrowserStart.available) { BrowserStartSettings(); DLine() }
            if (DataRing.available) { RingSettings(); DLine() }
            DMono(Msg.android_alerts_head, dfaint)
            var alerts by remember { mutableStateOf(AppSettings.alertsOn()) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { listOf(true to "on", false to "off").forEach { (v, l) -> DBtn(if (v) Msg.option_on else Msg.option_off, alerts == v, modifier = Modifier.testTag("settings.alerts.$l")) { alerts = v; AppSettings.putAlerts(v) } } }
            DText(Msg.android_alerts_note, dcs.onSurfaceVariant, 13, lines = 3)
            // an older hub has no Notifications page: its quiet hours and urgent switch are drawn here
            if (DataClient.has("push") && !DataNotify.available) {
                val quietOn = DataClient.quiet.isNotEmpty() && DataClient.quiet != "off"
                DText(Msg.android_quiet_head, size = 15, lines = 1)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    DBtn(Msg.option_on, quietOn, modifier = Modifier.testTag("settings.quiet.on")) { if (!quietOn) DataClient.putQuiet("22:00-07:00") }
                    DBtn(Msg.option_off, !quietOn, modifier = Modifier.testTag("settings.quiet.off")) { DataClient.putQuiet("off") }
                }
                if (quietOn) DMono(DataClient.quiet.split("-", limit = 2).let { if (it.size == 2) Msg.android_quiet_range(it[0], it[1]) else DataClient.quiet }, dcs.onSurfaceVariant, 12)
                DText(Msg.android_urgent_head, size = 15, lines = 1)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    DBtn(Msg.option_on, DataClient.quietUrgent, modifier = Modifier.testTag("settings.urgent.on")) { DataClient.putQuietUrgent(true) }
                    DBtn(Msg.option_off, !DataClient.quietUrgent, modifier = Modifier.testTag("settings.urgent.off")) { DataClient.putQuietUrgent(false) }
                }
                DText(Msg.android_quiet_note, dcs.onSurfaceVariant, 13, lines = 2)
            }
            DLine()
            DMono(Msg.settings_lang.uppercase(), dfaint)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("auto" to Msg.settings_lang_auto, "en" to Msg.settings_lang_en, "ja" to Msg.settings_lang_ja, "es" to Msg.settings_lang_es).forEach { (v, l) ->
                    DBtn(l, AppSettings.lang == v, modifier = Modifier.testTag("settings.lang.$v")) { AppSettings.putLang(v); DataClient.channels() }
                }
            }
            DText(Msg.settings_lang_note, dcs.onSurfaceVariant, 13, lines = 3)
            DLine()
            DMono(Msg.settings_appearance.uppercase(), dfaint)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("system" to Msg.native_settings_system, "light" to Msg.settings_theme_light, "dark" to Msg.settings_theme_dark).forEach { (m, l) -> DBtn(l, AppSettings.appearance == m, modifier = Modifier.testTag("settings.appearance.$m")) { AppSettings.putAppearance(m) } }
            }
            DLine()
            ViewerBgSettings()
            DLine()
            DBtn(Msg.android_unpair, false, modifier = Modifier.testTag("settings.unpair")) { DataClient.unpair(); back() }
        }
    }
}

// the 3D viewers' background (docs/VIEWER-BACKGROUND.md): Auto, Light, Dark or Custom, a swatch or a typed #rrggbb; kept on this device, live
@Composable
private fun ViewerBgSettings() {
    DMono(Msg.android_viewerbg.uppercase(), dfaint)
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        listOf("auto" to Msg.android_viewerbg_auto, "light" to Msg.android_viewerbg_light, "dark" to Msg.android_viewerbg_dark, "custom" to Msg.android_viewerbg_custom)
            .forEach { (m, l) -> DBtn(l, AppSettings.viewerBg == m, modifier = Modifier.testTag("settings.viewerbg.$m")) { AppSettings.putViewerBg(m) } }
    }
    val custom = ViewerBg.parse(AppSettings.viewerBgCustom)
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        ViewerBg.SWATCHES.forEach { c ->
            val on = AppSettings.viewerBg == "custom" && custom == c
            Box(Modifier.size(30.dp).background(Color(0xFF000000.toInt() or c)).border(BorderStroke(if (on) 2.dp else 1.dp, if (on) dcs.primary else dcs.outlineVariant))
                .clickable { AppSettings.putViewerBgCustom(ViewerBg.hex(c)) }.testTag("settings.viewerbg.swatch.${ViewerBg.hex(c).drop(1)}"))
        }
    }
    var typed by remember { mutableStateOf(AppSettings.viewerBgCustom) }
    LaunchedEffect(AppSettings.viewerBgCustom) { if (ViewerBg.parse(typed) != custom) typed = AppSettings.viewerBgCustom }
    val (top, bottom) = viewerBackdrop()
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        // what the viewers draw now
        Box(Modifier.size(56.dp, 30.dp).background(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(Color(0xFF000000.toInt() or top), Color(0xFF000000.toInt() or bottom))))
            .border(BorderStroke(1.dp, dcs.outlineVariant)).testTag("settings.viewerbg.preview"))
        DField(typed, { v -> typed = v.take(7); if (v.trim().removePrefix("#").length == 6 && ViewerBg.parse(v) != null) AppSettings.putViewerBgCustom(v) },
            Msg.android_viewerbg_hex, Modifier.width(120.dp).testTag("settings.viewerbg.hex"), mono = true)
    }
    DText(Msg.android_viewerbg_note, dcs.onSurfaceVariant, 13, lines = 2)
}

// the hub's updates: the hub says what to show (update.status); this draws it
@Composable
private fun UpdateSection() {
    val scope = rememberCoroutineScope()
    var u by remember { mutableStateOf<org.json.JSONObject?>(null) }
    suspend fun load(m: String, p: org.json.JSONObject = org.json.JSONObject()) { runCatching { DataClient.call(m, p) }.onSuccess { u = it } }
    LaunchedEffect(DataClient.conn) { load("update.status") }
    DMono(Msg.android_updates_head, dfaint)
    val s = u
    if (s == null) DMono(Msg.settings_tailscale_notconnected, dcs.onSurfaceVariant, 11) else {
        Text("Backplane ${s.optString("current")}", color = dcs.onSurface, fontFamily = BpUi, fontSize = 15.sp, fontWeight = FontWeight.Medium)
        DText(Tr.t(s.optString("line")), dcs.onSurfaceVariant, 13, Modifier.testTag("settings.update.line"), lines = 2)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("stable", "beta").forEach { c -> DBtn(if (c == "stable") Msg.option_stable else Msg.android_option_beta, s.optString("channel") == c, modifier = Modifier.testTag("settings.update.$c")) { scope.launch { load("update.channel", org.json.JSONObject().put("channel", c)) } } }
        }
        if (s.optBoolean("canCheck")) DBtn(Msg.settings_update_check, false, modifier = Modifier.testTag("settings.update.check")) { scope.launch { load("update.check"); kotlinx.coroutines.delay(3000); load("update.status") } }
    }
}

/** Add project: Open folder (a folder on the hub's machine) or, when the hub has it (feature "clone"), Clone from git. */
@Composable
private fun FolderScreen(ask: CloneAsk?, back: () -> Unit, added: (String) -> Unit, cloned: (String?) -> Unit) {
    val scope = rememberCoroutineScope()
    val can = DataClient.has("clone")
    // on Clone from git when /project named a repository, or while a clone this phone started runs
    var clone by remember { mutableStateOf(can && (ask != null || Clones.jobs[Clones.following]?.running == true)) }
    var err by remember { mutableStateOf("") }
    Column(Modifier.fillMaxSize()) {
        DBar(Msg.android_project_add, back)
        if (can) ModeSwitch(clone) { clone = it }
        if (can && clone) Box(Modifier.weight(1f)) { CloneBody(ask, cloned) }
        else Box(Modifier.weight(1f)) {
            FolderBrowser(null, Msg.android_folder_use, "folders.use", err) { cur -> scope.launch { try { added(DataClient.addProject(cur).id) } catch (e: Exception) { err = e.message ?: "" } } }
        }
    }
}
/** The hub machine's folders (dirs.list) from [start] (null: the hub's own start), and a button that takes the one shown. */
@Composable
internal fun FolderBrowser(start: String?, label: String, tag: String, refusal: String = "", use: (String) -> Unit) {
    var path by remember { mutableStateOf(start) }
    var cur by remember { mutableStateOf("") }
    var parent by remember { mutableStateOf<String?>(null) }
    var dirs by remember { mutableStateOf<List<String>>(emptyList()) }
    var err by remember { mutableStateOf("") }
    LaunchedEffect(path) { try { val (p, par, d) = DataClient.dirs(path); cur = p; parent = par; dirs = d; err = "" } catch (e: Exception) { err = e.message ?: "" } }
    Column(Modifier.fillMaxSize()) {
        DMono(cur, dcs.onSurface, 12, Modifier.padding(12.dp))
        err.ifEmpty { refusal }.takeIf { it.isNotEmpty() }?.let { Text(Tr.t(it), color = dcs.error, modifier = Modifier.padding(horizontal = 12.dp)) }
        LazyColumn(Modifier.weight(1f)) {
            if (parent != null) item { FolderRow("..") { path = parent } }
            items(dirs) { d -> FolderRow(d) { path = cur.trimEnd('/') + "/" + d } }
        }
        DLine()
        Box(Modifier.padding(12.dp)) { DBtn(label, true, cur.isNotEmpty(), Modifier.testTag(tag)) { use(cur) } }
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
    // a thread can start with pictures, as a reply carries them: the same Attach and Draw as a thread's composer, sent with the first message
    val pending = remember { mutableStateListOf<ByteArray>() }
    val owner = DataClient.projects.firstOrNull { it.id == project || it.newOn.any { n -> n.value == project } }
    val title = owner?.title ?: ""
    // where it runs (D6): the hub's choice first ("runs on desktop"); the line opens the machine picker when the project is on more than one
    var target by remember { mutableStateOf(project) }
    var pickRun by remember { mutableStateOf(false) }
    // the box is ready to type in as the screen opens (the keyboard comes up with it), as the desktop's composer is
    val focus = remember { androidx.compose.ui.focus.FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    val chosen = owner?.newOn?.firstOrNull { it.value == target }
    val where = if (owner != null && owner.runsOn.isNotEmpty() && (chosen == null || chosen == owner.newOn.firstOrNull())) owner.runsOn
        else chosen?.takeIf { owner!!.newOn.size > 1 || owner.mark != null }?.label ?: ""   // "New thread on desktop", "Wait for desktop"
    if (pickRun && owner != null) ActionSheet(title, owner.newOn.map { n -> (n.label + if (n.away) " · " + Msg.android_away else "") to { pickRun = false; target = n.value } }) { pickRun = false }
    LaunchedEffect(choice) { if (DataClient.has("picker")) runCatching { DataClient.picker(null, choice) }.onSuccess { chip = it.chip } }
    if (picking) PickerSheet(null, choice, { choice = it }) { picking = false }
    Column(Modifier.fillMaxSize()) {
        DBar(if (project == DataClient.CHATS) Msg.chats_new else Msg.android_thread_new_title(title), back)
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (where.isNotEmpty()) DMono(Tr.t(where) + if ((owner?.newOn?.size ?: 0) > 1) " ▾" else "", dfaint, 11, Modifier.testTag("new.where").clickable(enabled = (owner?.newOn?.size ?: 0) > 1) { pickRun = true })
            DField(text, { text = it }, Msg.android_thread_new_hint, Modifier.heightIn(min = 120.dp).focusRequester(focus).testTag("new.text"), 8, dcs.primary)
            if (err.isNotEmpty()) Text(Tr.t(err), color = dcs.error, fontFamily = BpMono, fontSize = 12.sp)
            PendingPics(pending, "new")
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (DataClient.has("picker")) ModelChip(chip.ifEmpty { " " }, "new.model", Modifier.weight(1f), provider = choice.provider ?: "") { picking = true } else Spacer(Modifier.weight(1f))
                PhotoTools(pending, { err = it }, "new")
                DBtn(if (busy) Msg.state_starting else Msg.android_start, true, !busy, Modifier.testTag("new.start")) {
                    busy = true; scope.launch { try { created(DataClient.createThread(target, text, choice, pending.toList()).id) } catch (e: Exception) { err = e.message ?: ""; busy = false } }
                }
            }
        }
    }
}

@Composable private fun Pic(b: ByteArray, size: Int) {
    AnimatedPic(b, Modifier.heightIn(max = size.dp).widthIn(max = (size * 1.5).toInt().dp).clip(RoundedCornerShape(8.dp)).border(BorderStroke(1.dp, dcs.outlineVariant), RoundedCornerShape(8.dp)), maxSidePx = 1400)
}
@Composable private fun PicZoomable(bytes: ByteArray, size: Int, zoom: Boolean) {
    var full by remember(bytes) { mutableStateOf(false) }
    Box(if (zoom) Modifier.clickable { full = true }.testTag("thread.image") else Modifier) { Pic(bytes, size) }
    if (full) androidx.compose.ui.window.Dialog(onDismissRequest = { full = false }, properties = androidx.compose.ui.window.DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(androidx.compose.ui.graphics.Color.Black).clickable { full = false }, contentAlignment = Alignment.Center) {
            AnimatedPic(bytes, Modifier.fillMaxSize())
        }
    }
}
@Composable private fun PicById(id: String, size: Int, zoom: Boolean = false) {
    var b by remember(id) { mutableStateOf<ByteArray?>(null) }
    LaunchedEffect(id) { b = DataClient.picture(id) }
    val bytes = b
    if (bytes == null) { Box(Modifier.size(size.dp, (size / 2).dp).background(dcs.surfaceVariant)); return }
    PicZoomable(bytes, size, zoom)
}
@Composable internal fun PicByReply(entry: String, path: String, size: Int) {
    var b by remember(entry, path) { mutableStateOf<ByteArray?>(null) }
    LaunchedEffect(entry, path) { b = DataClient.replyImage(entry, path) }
    val bytes = b
    if (bytes == null) { Box(Modifier.size(size.dp, (size / 2).dp).background(dcs.surfaceVariant)); return }
    PicZoomable(bytes, size, zoom = true)
}

// ---- improvements (the same as iOS's): a GitHub issue draft opens in the browser, or the text starts a thread on the hub
@Composable
private fun ImproveScreen(back: () -> Unit) {
    val scope = rememberCoroutineScope(); val ctx = androidx.compose.ui.platform.LocalContext.current
    var text by remember { mutableStateOf("") }; var sent by remember { mutableStateOf("") }
    // where it goes: the hub searches every machine's projects and says the last one chosen (improvements.projects); a pick is remembered there
    var project by remember { mutableStateOf("") }; var q by remember { mutableStateOf("") }; var rows by remember { mutableStateOf(listOf<org.json.JSONObject>()) }
    LaunchedEffect(q, DataClient.conn) {
        if (DataClient.conn != DataClient.Conn.ONLINE) return@LaunchedEffect
        kotlinx.coroutines.delay(if (q.isEmpty()) 0 else 250)
        runCatching { DataClient.call("improvements.projects", org.json.JSONObject().put("q", q)) }.onSuccess { r ->
            val a = r.optJSONArray("projects"); rows = (0 until (a?.length() ?: 0)).map { a!!.getJSONObject(it) }
            if (project.isEmpty()) project = r.optString("project").ifEmpty { rows.firstOrNull()?.optString("id") ?: "" }
        }
    }
    Column(Modifier.fillMaxSize()) {
        DBar(Msg.android_improve, back)
        Column(Modifier.padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            if (sent.isNotEmpty()) DMono(Tr.t(sent), dcs.primary, 12, Modifier.testTag("improve.sent"))
            DMono(Msg.android_improve_ask)
            DField(text, { text = it }, "", Modifier.testTag("improve.text"), 8)
            Text(Msg.android_improve_note, color = dcs.onSurfaceVariant, fontSize = 12.sp)
            DBtn(Msg.android_improve_github, false, text.isNotBlank(), Modifier.testTag("improve.github")) {
                val title = text.lineSequence().firstOrNull().orEmpty().take(80)
                val u = android.net.Uri.parse("https://github.com/BackplaneWorks/Backplane/issues/new").buildUpon().appendQueryParameter("title", title)
                    .appendQueryParameter("body", text + "\n\n---\nBackplane Android ${BuildConfig.VERSION_NAME}, Android ${android.os.Build.VERSION.RELEASE}").build()
                ctx.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, u))
            }
            if (DataClient.projects.isNotEmpty() || rows.isNotEmpty()) {
                DLine(); DMono(Msg.android_project_head)
                DField(q, { q = it }, Msg.android_improve_search, Modifier.testTag("improve.search"), 1)
                if (rows.isEmpty()) DMono(Msg.android_improve_nomatch, dcs.onSurfaceVariant, 12)
                rows.take(12).forEach { p -> val id = p.optString("id"); val short = p.optString("short")
                    DMono((if (id == project) "● " else "○ ") + p.optString("title") + (if (short.isEmpty()) "" else " · $short"), dcs.onSurface, 13,
                        Modifier.testTag("improve.project.$id").clickable { project = id; scope.launch { runCatching { DataClient.call("improvements.project", org.json.JSONObject().put("project", id)) } } }.padding(vertical = 4.dp)) }
                DBtn(Msg.android_improve_thread, true, text.isNotBlank() && DataClient.conn == DataClient.Conn.ONLINE && project.isNotEmpty(), Modifier.testTag("improve.thread")) {
                    scope.launch { runCatching { DataClient.createThread(project, "Improvement idea for this app:\n\n$text") }.onSuccess { sent = Msg.android_improve_sent; text = "" }.onFailure { sent = it.message ?: "" } }
                }
            }
        }
    }
}

/** The review bar (#1305): while the thread is read-only, why, and Allow edits; just after the person switched it off, Read-only again. The hub's words. */
@Composable
internal fun ReviewBar(r: DReviewBar, enabled: Boolean, press: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp).clip(dShapeCard).border(BorderStroke(1.dp, if (r.on) dwarn else dcs.outlineVariant), dShapeCard).background(dcs.surfaceVariant).padding(horizontal = 10.dp, vertical = 6.dp).testTag("thread.review"),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DMono(Tr.t(r.text), if (r.on) dcs.onSurface else dcs.onSurfaceVariant, 12, Modifier.weight(1f).testTag("thread.review.text"), lines = 3)
        DBtn(Tr.t(r.button), false, enabled, Modifier.testTag("thread.review.button")) { press() }
    }
}

/** Proactive banner shown when the hub has zero AI providers configured or signed in. */
@Composable
internal fun NoProviderBanner(onOpenAccounts: () -> Unit) {
    val n = DataClient.noProvider ?: return
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp).clip(dShapeCard).border(BorderStroke(1.dp, dwarn), dShapeCard).background(dcs.surfaceVariant).padding(horizontal = 10.dp, vertical = 8.dp).testTag("provider.notice"),
        verticalArrangement = Arrangement.spacedBy(6.dp)) {
        DText(Tr.t(n.head), dcs.onSurface, 14, bold = true, lines = 1)
        if (n.text.isNotEmpty()) DText(Tr.t(n.text), dcs.onSurfaceVariant, 13, lines = 3)
        DBtn(if (n.button.isNotEmpty()) Tr.t(n.button) else Msg.provider_notice_button, true, modifier = Modifier.testTag("provider.notice.button")) { onOpenAccounts() }
    }
}

/** A buy thread's Order card: the steps, the hub's line, the total and what it covers about customs, and Buy now (the hub's link, opened in the browser). */
@Composable
private fun OrderCard(o: DataClient.Order) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp).clip(dShapeCard).border(BorderStroke(1.dp, dcs.outlineVariant), dShapeCard).background(dcs.surfaceVariant).padding(10.dp).testTag("thread.order"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        DMono(if (o.name.isEmpty()) Msg.android_order_head else Msg.android_order_named(o.name), dcs.onSurface, 12, bold = true)
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            o.steps.forEachIndexed { i, s ->
                if (i > 0) DMono("›", dcs.outlineVariant, 11)
                val sl = Tr.t(s.label); when (s.mark) { "done" -> DMono("✓ $sl", dcs.onSurfaceVariant, 11); "on" -> DMono(sl, dcs.primary, 11, bold = true); else -> DMono(sl, dcs.outlineVariant, 11) }
            }
        }
        DMono(Tr.t(o.line), if (o.waiting) dcs.onSurfaceVariant else dcs.onSurface, 12, lines = 4) // the card's words wrap, as on iOS and in the window
        if (o.total.isNotEmpty()) DMono(o.total, dcs.onSurface, 14, bold = true)
        if (o.customs.isNotEmpty()) DMono(Tr.t(o.customs), dcs.onSurfaceVariant, 11, Modifier.testTag("thread.order.customs"), lines = 4)
        if (o.leadDays > 0) DMono(Msg.order_lead_days(o.leadDays), dcs.onSurfaceVariant, 11, Modifier.testTag("thread.order.lead"))
        if (o.estimateWarnings.isNotEmpty()) DMono(Msg.order_estimate_warnings(o.estimateWarnings), dwarn, 11, Modifier.testTag("thread.order.warnings"), lines = 4)
        if (o.pay.startsWith("https://") || o.pay.startsWith("http://")) DBtn(Tr.t(o.label.ifEmpty { "Buy now" }), true, modifier = Modifier.testTag("thread.order.pay")) {
            runCatching { ctx.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(o.pay))) }
        }
    }
}

/** A row's kind, with the linked machine it is on when it is another machine's (the hub's `mark.short`) */
fun onMachine(kind: String, mark: String): String = if (mark.isEmpty()) kind else "$kind · $mark"

/** Infinite context: the thread's memory bar over the composer's chips, in the hub's words (`thread.memory`). */
@Composable fun MemoryBar(m: DMemory) {
    val tone = when (m.state) { "held" -> dwarn; "caught" -> dok; else -> dcs.primary }
    Row(Modifier.fillMaxWidth().testTag("thread.memory"), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.width(48.dp).height(4.dp).background(dcs.outlineVariant)) { Box(Modifier.fillMaxHeight().fillMaxWidth(m.fill).background(tone)) }
        DMono(Tr.t(m.line), dcs.onSurfaceVariant, 11, Modifier.weight(1f, fill = false))
        DMono(Tr.t(m.status), if (m.state == "held") dwarn else dfaint, 11)
    }
}
