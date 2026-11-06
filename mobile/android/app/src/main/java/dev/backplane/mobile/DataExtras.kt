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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import kotlinx.coroutines.launch

// The data/1 additions on the phone: thread and project actions, search, the model picker, mention popup, bots and rooms (see native/host/src/data/ext.rs).

/** A flat list of actions in a dialog (the desktop's row menu): square, mono, one tap each. */
@Composable
fun ActionSheet(title: String, actions: List<Pair<String, () -> Unit>>, close: () -> Unit) {
    Dialog(onDismissRequest = close) {
        Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).testTag("sheet")) {
            DMono(title.ifEmpty { "New thread" }, dcs.onSurfaceVariant, 11, Modifier.padding(12.dp))
            DLine()
            actions.forEach { (label, run) ->
                Text(label, Modifier.fillMaxWidth().clickable { close(); run() }.padding(14.dp).testTag("sheet.$label"), color = if (label.startsWith("Delete") || label.startsWith("Remove")) dcs.error else dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp)
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
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { DBtn(ok, true, t.isNotBlank(), Modifier.testTag("prompt.ok")) { close(); done(t.trim()) }; DBtn("Cancel", false, onClick = close) }
        }
    }
}

/** The actions of a thread, as the desktop's header and row menu offer them. */
fun threadActions(t: DThread, rename: () -> Unit, confirmDelete: () -> Unit): List<Pair<String, () -> Unit>> = buildList {
    add((if (t.pinned) "Unpin" else "Pin") to { DataClient.threadAct(t.id, if (t.pinned) "unpin" else "pin"); Unit })
    add((if (t.settled) "Unsettle" else "Settle") to { DataClient.threadAct(t.id, if (t.settled) "unsettle" else "settle"); Unit })
    add((if (t.archived) "Unarchive" else "Archive") to { DataClient.threadAct(t.id, if (t.archived) "unarchive" else "archive"); Unit })
    add("Rename" to rename)
    add("Delete…" to confirmDelete)
}

@Composable
fun ThreadMenus(t: DThread, shown: Boolean, close: () -> Unit, deleted: () -> Unit = {}) {
    var renaming by remember(t.id) { mutableStateOf(false) }
    var deleting by remember(t.id) { mutableStateOf(false) }
    if (shown) ActionSheet(t.title, threadActions(t, { renaming = true }, { deleting = true }), close)
    if (renaming) TextPrompt("Rename thread", t.title, "Rename", { renaming = false }) { DataClient.threadAct(t.id, "rename", it) }
    if (deleting) ActionSheet("Delete “${t.title}”? This cannot be undone.", listOf("Delete thread" to { DataClient.threadAct(t.id, "delete"); deleted() }, "Keep it" to {}), { deleting = false })
}

@Composable
fun ProjectMenus(p: DProject, shown: Boolean, close: () -> Unit) {
    var renaming by remember(p.id) { mutableStateOf(false) }
    var removing by remember(p.id) { mutableStateOf(false) }
    if (shown) ActionSheet(p.title, listOf("Rename" to { renaming = true }, "Remove project…" to { removing = true }), close)
    if (renaming) TextPrompt("Rename project", p.title, "Rename", { renaming = false }) { DataClient.projectAct(p.id, "rename", it) }
    if (removing) ActionSheet("Remove “${p.title}” and its threads from the list?", listOf("Remove project" to { DataClient.projectAct(p.id, "remove") }, "Keep it" to {}), { removing = false })
}

/** Search over threads and projects (the hub's `search`). */
@Composable
fun SearchResults(q: String, open: (String) -> Unit) {
    var res by remember { mutableStateOf<Pair<List<DThread>, List<DProject>>?>(null) }
    LaunchedEffect(q) { res = if (q.isBlank()) null else runCatching { DataClient.search(q) }.getOrNull() }
    val r = res
    if (r == null) { if (q.isNotBlank()) DMono("searching…", modifier = Modifier.padding(12.dp)); return }
    if (r.first.isEmpty() && r.second.isEmpty()) { DMono("nothing matches “$q”", modifier = Modifier.padding(12.dp).testTag("search.none")); return }
    LazyColumn(Modifier.fillMaxSize().testTag("search.list")) {
        items(r.second, key = { "sp:${it.id}" }) { p -> Column { DMono("project · ${p.title}", dcs.onSurface, 12, Modifier.fillMaxWidth().padding(12.dp)); DLine() } }
        items(r.first, key = { "st:${it.id}" }) { t -> ThreadRowPublic(t) { open(t.id) } }
    }
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
            DMono(picker?.chip ?: " ", dcs.onSurface, 12, Modifier.weight(1f).testTag("picker.chip"))
            DMono("Done", dcs.primary, 13, Modifier.clickable(onClick = close).padding(8.dp).testTag("picker.done"), bold = true)
        }
        DLine()
        if (err.isNotEmpty()) Text(err, Modifier.padding(horizontal = 14.dp, vertical = 8.dp).testTag("picker.error"), color = dcs.error, fontFamily = BpMono, fontSize = 12.sp)
        Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState()).padding(bottom = 16.dp).testTag("models")) {
            picker?.sections?.forEach { s ->
                DMono(s.title.uppercase(), dfaint, 11, Modifier.padding(start = 14.dp, top = 14.dp, bottom = 4.dp))
                s.choices.forEach { c ->
                    Row(Modifier.fillMaxWidth().background(if (c.on) dcs.surfaceContainerHigh else Color.Transparent).clickable(enabled = c.ok) {
                        scope.launch {
                            if (thread != null) { try { DataClient.pick(thread, s.action, c.value); load(choice) } catch (e: Exception) { err = e.message ?: "" } }
                            else { val n = choice.set(s.action, c.value); choose(n); load(n) }
                        }
                    }.heightIn(min = 44.dp).padding(horizontal = 14.dp, vertical = 6.dp).testTag("pick.${s.action}.${c.value}"), verticalAlignment = Alignment.CenterVertically) {
                        DMono(if (c.on) "✓" else "", dcs.primary, 13, Modifier.width(24.dp))
                        Column {
                            Text(c.label, color = if (c.ok) dcs.onSurface else dfaint, fontFamily = BpMono, fontSize = 13.sp)
                            if (!c.ok && c.why.isNotEmpty()) DMono(c.why, dfaint, 11)
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
            Row(Modifier.fillMaxWidth().clickable { pick(draft.dropLast(m.value.trimStart().length) + it.sigil + (if (it.sigil == "@" || it.sigil == "#" || it.sigil == "&") it.label else it.id) + " ") }.padding(horizontal = 12.dp, vertical = 8.dp)) {
                DMono(it.sigil + it.label, dcs.onSurface, 12, Modifier.weight(1f)); DMono(it.detail)
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
            items(posts.size) { i -> val p = posts[i]; Column(Modifier.padding(horizontal = 12.dp, vertical = 5.dp)) { DMono(p.from.ifEmpty { "you" }, dcs.primary, 11, bold = true); Text(p.text, color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp) } }
        }
        DLine()
        Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.Bottom) {
            Box(Modifier.weight(1f)) { DField(draft, { draft = it }, "Post to #${room.name}", Modifier.testTag("room.composer"), 4, dcs.primary) }
            Spacer(Modifier.width(8.dp))
            DBtn("Post", true, draft.isNotBlank(), Modifier.testTag("room.post")) { val t = draft; draft = ""; scope.launch { runCatching { DataClient.roomPost(room.id, t) }; load() } }
        }
    }
}
