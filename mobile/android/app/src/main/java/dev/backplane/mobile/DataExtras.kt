package dev.backplane.mobile

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import kotlinx.coroutines.launch
import org.json.JSONObject

// The data/1 additions on the phone: thread and project actions, search, the model picker, mention popup, bots and rooms (see native/host/src/data/ext.rs).

/** A flat list of actions in a dialog (the desktop's row menu): square, mono, one tap each. */
@Composable
fun ActionSheet(title: String, actions: List<Pair<String, () -> Unit>>, close: () -> Unit) {
    Dialog(onDismissRequest = close) {
        Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).testTag("sheet")) {
            DMono(title.ifEmpty { Msg.thread_new }, dcs.onSurfaceVariant, 11, Modifier.padding(12.dp))
            DLine()
            actions.forEach { (label, run) ->
                Text(Tr.t(label), Modifier.fillMaxWidth().clickable { close(); run() }.padding(14.dp).testTag("sheet.$label"), color = if (label.startsWith("Delete") || label.startsWith("Remove")) dcs.error else dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp)
                DLine()
            }
        }
    }
}

@Composable
fun TextPrompt(title: String, initial: String, ok: String, close: () -> Unit, done: (String) -> Unit) {
    var t by remember { mutableStateOf(initial) }
    Dialog(onDismissRequest = close) {
        Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            DMono(title, dcs.onSurfaceVariant, 11)
            DField(t, { t = it }, "", Modifier.testTag("prompt.field"))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { DBtn(ok, true, t.isNotBlank(), Modifier.testTag("prompt.ok")) { close(); done(t.trim()) }; DBtn(Msg.common_cancel, false, onClick = close) }
        }
    }
}

/** The actions of a thread, as the desktop's header and row menu offer them. */
fun threadActions(t: DThread, rename: () -> Unit, confirmDelete: () -> Unit, search: (() -> Unit)? = null, buy: Pair<String, () -> Unit>? = null): List<Pair<String, () -> Unit>> = buildList {
    buy?.let { add(it) } // Buy it heads the thread's tools, as on the desktop
    search?.let { add((if (DataClient.has("find")) Msg.android_find_inthread else Msg.android_search_inthread) to it) }
    add((if (t.pinned) "Unpin" else "Pin") to { DataClient.threadAct(t.id, if (t.pinned) "unpin" else "pin"); Unit })
    add((if (t.settled) "Unsettle" else "Settle") to { DataClient.threadAct(t.id, if (t.settled) "unsettle" else "settle"); Unit })
    add((if (t.archived) "Unarchive" else "Archive") to { DataClient.threadAct(t.id, if (t.archived) "unarchive" else "archive"); Unit })
    if (t.parent.isNotEmpty()) add("Move to top level" to { DataClient.threadAct(t.id, "detach"); Unit })
    add("Rename" to rename)
    add("Delete…" to confirmDelete)
}

@Composable
fun ThreadMenus(t: DThread, shown: Boolean, close: () -> Unit, deleted: () -> Unit = {}, search: (() -> Unit)? = null, buy: Pair<String, () -> Unit>? = null) {
    var renaming by remember(t.id) { mutableStateOf(false) }
    var deleting by remember(t.id) { mutableStateOf(false) }
    if (shown) ActionSheet(t.title, threadActions(t, { renaming = true }, { deleting = true }, search, buy), close)
    if (renaming) TextPrompt(Msg.android_thread_rename, t.title, Msg.menu_rename, { renaming = false }) { DataClient.threadAct(t.id, "rename", it) }
    if (deleting) ActionSheet(Msg.android_thread_delete_ask(t.title), listOf("Delete thread" to { DataClient.threadAct(t.id, "delete"); deleted() }, "Keep it" to {}), { deleting = false })
}

/** A project's badge: the hub's tints and icons (`badge.list`); a press is `project.act color|icon|pick`. */
@Composable
fun BadgeSheet(p: DProject, close: () -> Unit) {
    var list by remember { mutableStateOf<JSONObject?>(null) }
    LaunchedEffect(p.id) { list = runCatching { DataClient.badgeList() }.getOrNull() }
    Dialog(onDismissRequest = close) {
        Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(12.dp).testTag("badge.sheet"), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            DMono(p.title, dcs.onSurfaceVariant, 11)
            val l = list
            if (l == null) DMono(Msg.viewer_loading, dcs.onSurfaceVariant, 11)
            else {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    val cs = l.optJSONArray("colors"); (0 until (cs?.length() ?: 0)).map { cs!!.getJSONObject(it).let { c -> c.optString("name") to c.optString("css") } }.forEach { (name, css) ->
                        val on = css.equals(p.css, ignoreCase = true)
                        Box(Modifier.size(24.dp).background(runCatching { Color(android.graphics.Color.parseColor(css)) }.getOrDefault(dcs.onSurfaceVariant))
                            .border(BorderStroke(if (on) 2.dp else 0.dp, dcs.onSurface)).clickable { DataClient.projectAct(p.id, "color", value = name) }.testTag("badge.color.$name"))
                    }
                }
                val ia = l.optJSONArray("icons"); val icons = (0 until (ia?.length() ?: 0)).map { ia!!.optString(it) }
                icons.chunked(3).forEach { row ->
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        row.forEach { n -> DMono((if (n == p.icon) "● " else "○ ") + n, dcs.onSurface, 12, Modifier.weight(1f).clickable { DataClient.projectAct(p.id, "icon", value = n) }.padding(vertical = 4.dp).testTag("badge.icon.$n")) }
                    }
                }
            }
            // Pick again: the hub's model chooses the icon anew (the person's choice is cleared)
            if (l != null) DBtn(Tr.t("Pick again"), false, modifier = Modifier.testTag("badge.again")) { DataClient.projectAct(p.id, "pick") }
            DBtn(Msg.common_close, false, onClick = close)
        }
    }
}

@Composable
fun ProjectMenus(p: DProject, shown: Boolean, close: () -> Unit) {
    var renaming by remember(p.id) { mutableStateOf(false) }
    var removing by remember(p.id) { mutableStateOf(false) }
    var badge by remember(p.id) { mutableStateOf(false) }
    val terms = LocalTermsOpen.current
    if (shown) ActionSheet(p.title, listOfNotNull(("Terminals" to { terms(p.id) }).takeIf { termsOn() }, "Rename" to { renaming = true }, "Badge…" to { badge = true }, "Remove project…" to { removing = true }), close)
    if (badge) BadgeSheet(p) { badge = false }
    if (renaming) TextPrompt(Msg.android_project_rename, p.title, Msg.menu_rename, { renaming = false }) { DataClient.projectAct(p.id, "rename", it) }
    if (removing) ActionSheet(Msg.android_project_remove_ask(p.title), listOf("Remove project" to { DataClient.projectAct(p.id, "remove") }, "Keep it" to {}), { removing = false })
}

/** Search over threads and projects (the hub's `search`), and under them what was said (its `messages`): a tap opens the thread at the message, marked. */
@Composable
fun SearchResults(q: String, open: (String) -> Unit) {
    var res by remember { mutableStateOf<DataClient.Found?>(null) }
    LaunchedEffect(q) { res = if (q.isBlank()) null else runCatching { DataClient.search(q) }.getOrNull() }
    val r = res
    if (r == null) { if (q.isNotBlank()) DMono(Msg.android_searching, modifier = Modifier.padding(12.dp)); return }
    if (r.threads.isEmpty() && r.projects.isEmpty() && r.messages.isEmpty()) { DMono(Msg.android_search_none(q), modifier = Modifier.padding(12.dp).testTag("search.none")); return }
    LazyColumn(Modifier.fillMaxSize().testTag("search.list")) {
        items(r.projects, key = { "sp:${it.id}" }) { p -> Column { DMono(Msg.android_search_project(p.title), dcs.onSurface, 12, Modifier.fillMaxWidth().padding(12.dp)); DLine() } }
        items(r.threads.filter { !it.history }, key = { "st:${it.id}" }) { t -> ThreadRowPublic(t) { open(t.id) } }
        // the old Scratch chats the search found, under their group's name (feature history)
        val old = r.threads.filter { it.history }
        if (old.isNotEmpty()) item("sh") { DMono(Msg.android_history_head, dfaint, 11, Modifier.fillMaxWidth().padding(start = 12.dp, top = 12.dp, bottom = 4.dp).testTag("search.history")) }
        items(old, key = { "sh:${it.id}" }) { t -> ThreadRowPublic(t) { open(t.id) } }
        if (r.messages.isNotEmpty()) item("sm") { DMono(Msg.android_search_messages, dfaint, 11, Modifier.fillMaxWidth().padding(start = 12.dp, top = 12.dp, bottom = 4.dp).testTag("search.messages")) }
        items(r.messages, key = { "sm:${it.thread}:${it.msg}" }) { m ->
            Column(Modifier.fillMaxWidth().clickable { DataClient.pendingMsg = m.msg; open(m.thread) }.padding(horizontal = 12.dp, vertical = 10.dp).testTag("search.msg")) {
                Text(marked(m.text, q, dcs.primary), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp, maxLines = 3)
                DMono(Tr.t(DataFind.who(m.role)) + " · " + m.title + (if (m.project.isNotEmpty()) " · " + m.project else "") + " · " + dAgo(m.at), dfaint, 11)
            }
            DLine()
        }
    }
}

/** A snippet with the words of `q` in bold (DataFind.marks). */
internal fun marked(text: String, q: String, c: Color) = androidx.compose.ui.text.buildAnnotatedString {
    append(text); DataFind.marks(text, q).forEach { addStyle(androidx.compose.ui.text.SpanStyle(fontWeight = androidx.compose.ui.text.font.FontWeight.Bold, color = c), it.first, it.last + 1) }
}

/** The picker the composer's chip opens: the hub's sections, the one in force marked. On a thread a press is the desktop's own request; before a thread exists it is
 *  remembered and sent with `thread.create`. It stays open, so a model and an effort are one visit. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PickerSheet(thread: String?, choice: DNewThreadChoice, choose: (DNewThreadChoice) -> Unit, close: () -> Unit) {
    val scope = rememberCoroutineScope()
    var picker by remember { mutableStateOf<DPicker?>(null) }
    var err by remember { mutableStateOf("") }
    suspend fun load(c: DNewThreadChoice) { try { picker = DataClient.picker(thread, c); err = "" } catch (e: Exception) { err = e.message ?: "" } }
    LaunchedEffect(Unit) { load(choice) }
    ModalBottomSheet(onDismissRequest = close, shape = RectangleShape, containerColor = dcs.surfaceVariant, dragHandle = null, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
      // one height from the first frame: a sheet that opens empty and grows when the list arrives stays where it opened; short of the whole screen, it clears the status bar
      Column(Modifier.fillMaxWidth().fillMaxHeight(.85f)) {
        Row(Modifier.fillMaxWidth().height(48.dp).padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            DMono(picker?.chip?.let(Tr::t) ?: " ", dcs.onSurface, 12, Modifier.weight(1f).testTag("picker.chip"))
            DMono(Msg.android_done, dcs.primary, 13, Modifier.clickable(onClick = close).padding(8.dp).testTag("picker.done"), bold = true)
        }
        DLine()
        if (err.isNotEmpty()) Text(Tr.t(err), Modifier.padding(horizontal = 14.dp, vertical = 8.dp).testTag("picker.error"), color = dcs.error, fontFamily = BpMono, fontSize = 12.sp)
        Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState()).padding(bottom = 16.dp).testTag("models")) {
            picker?.sections?.forEach { s ->
                DMono(Tr.t(s.title).uppercase(), dfaint, 11, Modifier.padding(start = 14.dp, top = 14.dp, bottom = 4.dp))
                s.choices.forEach { c ->
                    Row(Modifier.fillMaxWidth().background(if (c.on) dcs.surfaceContainerHigh else Color.Transparent).clickable(enabled = c.ok) {
                        scope.launch {
                            if (thread != null) { try { DataClient.pick(thread, s.action, c.value); load(choice) } catch (e: Exception) { err = e.message ?: "" } }
                            else { val n = choice.set(s.action, c.value); choose(n); load(n) }
                        }
                    }.heightIn(min = 44.dp).padding(horizontal = 14.dp, vertical = 6.dp).testTag("pick.${s.action}.${c.value}"), verticalAlignment = Alignment.CenterVertically) {
                        DMono(if (c.on) "✓" else "", dcs.primary, 13, Modifier.width(24.dp))
                        Column {
                            Text(Tr.t(c.label), color = if (c.ok) dcs.onSurface else dfaint, fontFamily = BpMono, fontSize = 13.sp)
                            if (!c.ok && c.why.isNotEmpty()) DMono(Tr.t(c.why), dfaint, 11)
                        }
                    }
                }
            }
        }
      }
    }
}

private val mentionTail = Regex("(?:^|\\s)([@>%#&])(\\S*)$")
/** The popup above the composer: the word being typed begins with @ > % # & and the hub offers matches. */
@Composable
fun MentionPopup(draft: String, pick: (String) -> Unit) {
    if (!DataClient.has("mentions")) return
    val m = mentionTail.find(draft) ?: return
    val sigil = m.groupValues[1]; val q = m.groupValues[2]
    var items by remember { mutableStateOf<List<DataClient.Mention>>(emptyList()) }
    LaunchedEffect(sigil, q) { items = runCatching { DataClient.mentions(q, sigil) }.getOrDefault(emptyList()) }
    if (items.isEmpty()) return
    Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).testTag("mentions")) {
        items.take(6).forEach { it ->
            Row(Modifier.fillMaxWidth().clickable { pick(draft.dropLast(m.value.trimStart().length) + it.word.ifEmpty { it.sigil + (if (it.sigil == "@" || it.sigil == "#" || it.sigil == "&") it.label else it.id) } + " ") }.padding(horizontal = 12.dp, vertical = 8.dp)) {
                DMono(it.sigil + it.label, dcs.onSurface, 12, Modifier.weight(1f)); DMono(Tr.t(it.detail))
            }
        }
    }
}

/** A bot's or room's name row in the sidebar. */
@Composable
fun SidebarRow(sigil: String, name: String, detail: String, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick).testTag("side.$sigil$name")) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            DMono(sigil, dcs.primary, 13, bold = true); Spacer(Modifier.width(10.dp)); Text(name, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp); DMono(detail, dfaint)
        }
    }
}

@Composable
fun RoomScreen(room: DataClient.Room, back: () -> Unit) {
    val scope = rememberCoroutineScope()
    var posts by remember { mutableStateOf<List<DataClient.Post>>(emptyList()) }
    var draft by remember { mutableStateOf("") }
    suspend fun load() { posts = runCatching { DataClient.roomPosts(room.id) }.getOrDefault(posts) }
    LaunchedEffect(room.id) { while (true) { load(); kotlinx.coroutines.delay(2500) } }
    Column(Modifier.fillMaxSize()) {
        DBar("#${room.name}", back)
        LazyColumn(Modifier.weight(1f).fillMaxWidth().testTag("room.list"), contentPadding = PaddingValues(vertical = 8.dp)) {
            items(posts.size) { i -> val p = posts[i]; Column(Modifier.padding(horizontal = 12.dp, vertical = 5.dp)) { DMono(p.from.ifEmpty { Msg.android_you }, dcs.primary, 11, bold = true); LinkText(p.text, "", color = dcs.onSurface, size = 13) } }
        }
        DLine()
        Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.Bottom) {
            Box(Modifier.weight(1f)) { DField(draft, { draft = it }, Msg.android_room_post_to(room.name), Modifier.testTag("room.composer"), 4, dcs.primary) }
            Spacer(Modifier.width(8.dp))
            DBtn(Msg.android_post, true, draft.isNotBlank(), Modifier.testTag("room.post")) { val t = draft; draft = ""; scope.launch { runCatching { DataClient.roomPost(room.id, t) }; load() } }
        }
    }
}

/** The find bar over a thread (feature `find`): the words typed (2 or more letters, after a short pause) are the hub's `find`; it lands on the newest match, the arrows
 *  step older and newer through the hub's matches (DataFind.step) and `go` scrolls to each and marks it ("": none, the mark goes); X closes it and clears the mark. */
@Composable
fun FindBar(tid: String, close: () -> Unit, go: (String) -> Unit) {
    var q by remember(tid) { mutableStateOf("") }
    var res by remember(tid) { mutableStateOf<DataClient.FindRes?>(null) }
    var cur by remember(tid) { mutableStateOf(-1) }
    var err by remember(tid) { mutableStateOf("") }
    val focus = remember { androidx.compose.ui.focus.FocusRequester() }
    LaunchedEffect(q) {
        val w = q.trim(); if (w.length < 2) { if (res != null) go(""); res = null; cur = -1; return@LaunchedEffect }
        kotlinx.coroutines.delay(250)
        try {
            val r = DataClient.find(tid, w); val ms = r.matches.map { it.msg }
            val before = res?.takeIf { it.q == r.q }?.matches?.getOrNull(cur)?.msg
            res = r; err = ""; cur = DataFind.pick(ms, before); go(ms.getOrNull(cur) ?: "")
        } catch (e: Exception) { err = e.message ?: "" }
    }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    fun move(dir: Int) { val r = res ?: return; cur = DataFind.step(cur, r.matches.size, dir); r.matches.getOrNull(cur)?.msg?.let(go) }
    Column(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).testTag("find")) {
        Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 2.dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.weight(1f)) { DField(q, { q = it }, Msg.android_find_inthread, Modifier.focusRequester(focus).testTag("find.field")) }
            val r = res
            DMono(when { err.isNotEmpty() -> Msg.android_find_error; r == null -> ""; else -> DataFind.count(cur, r.matches.size, r.total, r.more, Msg.android_find_nomatch) { a, b -> Msg.android_find_count(a, b) } }, if (err.isNotEmpty()) dcs.error else dfaint, 11, Modifier.padding(horizontal = 8.dp).testTag("find.count"))
            val some = (r?.matches?.size ?: 0) > 0
            androidx.compose.material3.IconButton(onClick = { move(-1) }, enabled = some, modifier = Modifier.size(40.dp).testTag("find.prev")) { androidx.compose.material3.Icon(androidx.compose.material.icons.Icons.Default.KeyboardArrowUp, Msg.android_find_prev, tint = if (some) dcs.onSurfaceVariant else dfaint) }
            androidx.compose.material3.IconButton(onClick = { move(1) }, enabled = some, modifier = Modifier.size(40.dp).testTag("find.next")) { androidx.compose.material3.Icon(androidx.compose.material.icons.Icons.Default.KeyboardArrowDown, Msg.android_find_next, tint = if (some) dcs.onSurfaceVariant else dfaint) }
            androidx.compose.material3.IconButton(onClick = close, modifier = Modifier.size(40.dp).testTag("find.close")) { androidx.compose.material3.Icon(androidx.compose.material.icons.Icons.Default.Close, Msg.common_close, tint = dcs.onSurfaceVariant) }
        }
        if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 11, Modifier.padding(start = 12.dp, bottom = 4.dp))
        DLine()
    }
}

/** "Search in thread" on a hub without `find`: what was said here with every word typed, newest first (the hub's `search` with `thread`); a tap goes to the message. The box
 *  takes the keyboard at once: the person asked to search. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FindInThread(tid: String, close: () -> Unit, go: (String) -> Unit) {
    var q by remember { mutableStateOf("") }
    var hits by remember { mutableStateOf<List<DataClient.Hit>?>(null) }
    var err by remember { mutableStateOf("") }
    LaunchedEffect(q) { if (q.isBlank()) { hits = null; return@LaunchedEffect }; kotlinx.coroutines.delay(250); try { hits = DataClient.searchIn(tid, q.trim()); err = "" } catch (e: Exception) { err = e.message ?: "" } }
    val focus = remember { androidx.compose.ui.focus.FocusRequester() }
    ModalBottomSheet(onDismissRequest = close, shape = RectangleShape, containerColor = dcs.surfaceVariant, dragHandle = null, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().fillMaxHeight(.85f).testTag("find")) {
            Row(Modifier.fillMaxWidth().height(48.dp).padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                DMono(Msg.android_find_head, dcs.onSurfaceVariant, 11, Modifier.weight(1f))
                DMono(Msg.android_done, dcs.primary, 13, Modifier.clickable(onClick = close).padding(8.dp).testTag("find.done"), bold = true)
            }
            Box(Modifier.padding(horizontal = 12.dp)) { DField(q, { q = it }, Msg.android_find_hint, Modifier.focusRequester(focus).testTag("find.field")) }
            LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
            Spacer(Modifier.height(8.dp)); DLine()
            val h = hits
            when {
                err.isNotEmpty() -> DMono(Tr.t(err), dcs.error, 12, Modifier.padding(14.dp))
                h == null -> {}
                h.isEmpty() -> DMono(Msg.android_find_none(q.trim()), dfaint, 12, Modifier.padding(14.dp).testTag("find.none"))
                else -> LazyColumn(Modifier.fillMaxWidth().weight(1f)) {
                    items(h, key = { it.msg + it.at }) { x ->
                        Column(Modifier.fillMaxWidth().clickable { close(); go(x.msg) }.padding(horizontal = 14.dp, vertical = 10.dp).testTag("find.hit")) {
                            DMono((if (x.role == "user") Msg.android_you else Msg.android_agent) + " · " + dAgo(x.at), dfaint, 11)
                            Text(x.text, color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp, maxLines = 3)
                        }
                        DLine()
                    }
                }
            }
        }
    }
}

/** What waits for the agent (the desktop's queue strip over the composer): each message's first line and the buttons the hub gives it (Steer sends it into the running turn now,
 *  Edit takes it back into the draft, Remove drops it). */
@Composable
fun QueueStrip(tid: String, rows: List<QRow>, edit: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var err by remember { mutableStateOf("") }
    Column(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp).testTag("thread.queue"), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        rows.forEach { r ->
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                DMono(Tr.t(r.tag).uppercase(), dfaint, 10); Spacer(Modifier.width(8.dp))
                DMono(r.text, dcs.onSurface, 12, Modifier.weight(1f))
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End)) {
                r.buttons.filter { it.action != "q-up" }.forEach { b ->
                    // q-* the hub's queue, o-* its far outbox, u-* this phone's own outbox (Outbox.kt; never sent to the hub)
                    DMono(Tr.t(b.label), if (b.action == "q-steer") dcs.primary else if (b.action.endsWith("-remove")) dcs.error else dcs.onSurfaceVariant, 12,
                        Modifier.border(BorderStroke(1.dp, dcs.outlineVariant)).clickable { scope.launch { try {
                            val t = if (Outbox.isOwn(b.action)) DataClient.outboxAct(b) else DataClient.queueAct(tid, b)
                            if (b.action.endsWith("-edit") && t.isNotEmpty()) edit(t); err = ""
                        } catch (e: Exception) { err = e.message ?: "" } } }
                            .padding(horizontal = 10.dp, vertical = 5.dp).testTag("queue.${b.action}"), bold = b.action == "q-steer")
                }
            }
        }
        if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 11)
    }
}
