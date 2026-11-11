package dev.backplane.mobile

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

// The sidebar's sections (docs/ALWAYS-ON.md section 8, the hub's present/sections.rs through data/1 `sidebar.sections`): Needs you, Pinned,
// Active, the groups (by git owner, then folder) and Quiet. The hub decides what is in each; the app draws them as they come.

/** A row of a section: a thread, or a project (its `key` set) with its visible threads under it. */
data class DSecRow(val id: String, val title: String, val word: String, val say: String, val want: String, val ago: String, val mark: DMark?, val project: String,
    val off: Boolean, val nav: String, val pinned: Boolean, val key: String, val running: Int, val threads: List<DSecRow>, val css: String, val dots: List<Pair<Boolean, Boolean>>, val kind: String = "", val hide: String = "", val icon: String = "", val menu: List<DMenuItem> = emptyList(),
    /** the fold the hub keeps for a project row (`sidebar.fold.<key>`): the action its chevron sends, folded now, the rows it hides or would hide */
    val fold: String = "", val folded: Boolean = false, val under: Int = 0, val needsCount: Int = 0) {
    companion object {
        fun of(o: JSONObject): DSecRow = DSecRow(o.optString("id"), o.optString("title"), o.optString("word"), o.optString("say"), o.optString("want"), o.optString("ago"),
            parseMark(o.optJSONObject("mark")), o.optString("project"), o.optBoolean("off"), o.optString("nav"), o.optBoolean("pinned"), o.optString("key"), o.optInt("running"),
            list(o.optJSONArray("threads")), o.optString("css"), (0 until (o.optJSONArray("dots")?.length() ?: 0)).map { i -> o.getJSONArray("dots").getJSONObject(i).let { it.optBoolean("here") to it.optBoolean("dim") } }, o.optString("kind"), o.optString("hide"), o.optString("icon"), DMenuItem.list(o.optJSONArray("menu")),
            o.optString("fold"), o.optBoolean("folded"), o.optInt("under"), o.optInt("needsCount"))
        fun list(a: JSONArray?): List<DSecRow> = (0 until (a?.length() ?: 0)).map { of(a!!.getJSONObject(it)) }
    }
    /** a chevron on the row: something is under it, or it is folded (an older hub sends no fold) */
    val canFold: Boolean get() = fold.isNotEmpty() && (folded || under > 0)
    /** a live swarm's row (`kind` swarm): it opens the swarm's page, its room */
    val swarmRoom: String? get() = if (kind == "swarm") nav.removePrefix("@room:").takeIf { it.isNotEmpty() && nav.startsWith("@room:") } else null
}
/** An item of a project row's menu, the hub's (present/sections.rs `project_menu`): `id` says what it does here, `sub` is Move to group's list. */
data class DMenuItem(val id: String, val label: String, val action: String, val value: String, val confirm: Boolean, val sub: List<DMenuItem>) {
    companion object {
        fun list(a: JSONArray?): List<DMenuItem> = (0 until (a?.length() ?: 0)).map { i -> a!!.getJSONObject(i).let { o ->
            DMenuItem(o.optString("id"), o.optString("label"), o.optString("action"), o.optString("value"), o.optBoolean("confirm"), list(o.optJSONArray("sub"))) } }
        /** What a phone's long press lists: the hub's menu without Open folder (the folder is on the hub's machine) and New swarm when swarms are off. */
        fun phone(menu: List<DMenuItem>, swarms: Boolean): List<DMenuItem> = menu.filter { it.id != "open" && (swarms || it.id != "swarm") && it.label.isNotEmpty() }
    }
}
data class DSection(val id: String, val kind: String, val title: String, val count: Int, val fold: Boolean, val open: Boolean, val toggle: String, val group: String, val rename: Boolean, val rows: List<DSecRow>) {
    companion object {
        fun list(a: JSONArray?): List<DSection> = (0 until (a?.length() ?: 0)).map { i -> a!!.getJSONObject(i).let { o ->
            DSection(o.optString("id"), o.optString("kind"), o.optString("title"), o.optInt("count"), o.optBoolean("fold"), o.optBoolean("open"), o.optString("toggle"), o.optString("group"), o.optBoolean("rename"), DSecRow.list(o.optJSONArray("rows"))) } }
        fun groups(a: JSONArray?): List<Pair<String, String>> = (0 until (a?.length() ?: 0)).map { i -> a!!.getJSONObject(i).let { it.optString("value") to it.optString("label") } }
    }
}

@Composable private fun toneOf(word: String): Color = when (word) { "needs" -> dcs.error; "running", "monitoring", "limit" -> dok; "failed", "cut", "interrupted" -> dcs.error.copy(alpha = .6f); else -> dfaint }

@Composable
fun SectionHead(s: DSection, onLong: (() -> Unit)?) {
    @OptIn(ExperimentalFoundationApi::class)
    Row(Modifier.fillMaxWidth().height(32.dp).combinedClickable(onClick = { if (s.fold) DataClient.toggleSection(s.toggle) }, onLongClick = onLong).padding(start = 10.dp, top = 6.dp).testTag("home.section.${s.id}"), verticalAlignment = Alignment.CenterVertically) {
        if (s.fold) { Icon(if (s.open) Icons.Default.ExpandMore else Icons.Default.ChevronRight, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(14.dp)); Spacer(Modifier.width(6.dp)) }
        DMono(Tr.t(s.title).uppercase(), if (s.id == "needs") dcs.error else dcs.onSurfaceVariant, 11, bold = s.id == "needs")
        if (s.fold || s.kind == "threads") { Spacer(Modifier.width(6.dp)); DMono("${s.count}", dfaint, 11) }
    }
}
/** Another machine's thread (the hub's `hide`): a long press offers Hide, here only, until something new happens there (`thread-hide`). */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SectionThread(r: DSecRow, indent: Int = 12, showProject: Boolean = false, open: (String) -> Unit) {
    var menu by remember { mutableStateOf(false) }
    Box {
        Row(Modifier.fillMaxWidth().height(36.dp).alpha(if (r.off) .55f else 1f).combinedClickable(onClick = { if (!r.off) open(r.nav) }, onLongClick = if (r.hide.isNotEmpty()) ({ menu = true }) else null).padding(start = indent.dp, end = 12.dp).testTag("home.section.thread.${r.id}"), verticalAlignment = Alignment.CenterVertically) {
            DDot(toneOf(r.word), 7); Spacer(Modifier.width(8.dp))
            DMono(r.title, dcs.onSurface, 13, Modifier.weight(1f, fill = false))
            if (showProject) { Spacer(Modifier.width(8.dp)); DMono(r.project, dfaint, 11) }
            Spacer(Modifier.weight(1f))
            r.mark?.let { DMono(it.short + if (it.away) " · " + Msg.android_away else "", dfaint, 11); Spacer(Modifier.width(6.dp)) }
            if (r.want.isNotEmpty()) DMono(Tr.t(r.want), dcs.error, 11) else DMono(Tr.t(r.say.ifEmpty { r.ago }), dfaint, 11)
        }
        DropdownMenu(menu, { menu = false }) { DropdownMenuItem(text = { Text(Msg.common_hide) }, onClick = { menu = false; DataClient.sidebarAct("thread-hide", r.hide) }, modifier = Modifier.testTag("home.section.hide.${r.id}")) }
    }
}
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SectionProject(r: DSecRow, faded: Boolean, onClick: () -> Unit, onLong: () -> Unit) {
    Row(Modifier.fillMaxWidth().height(38.dp).alpha(if (faded) .55f else 1f).combinedClickable(onClick = onClick, onLongClick = onLong).padding(end = 12.dp).testTag("home.section.project.${r.key}"), verticalAlignment = Alignment.CenterVertically) {
        // the fold: a tap on the chevron folds its threads away or back (the hub keeps it), a tap elsewhere opens the page
        Box(Modifier.size(width = 30.dp, height = 38.dp).then(if (r.canFold) Modifier.clickable { DataClient.sidebarAct(r.fold, r.key) } else Modifier).testTag("home.section.fold.${r.key}"), contentAlignment = Alignment.Center) {
            if (r.canFold) Icon(if (r.folded) Icons.Default.ChevronRight else Icons.Default.ExpandMore, if (r.folded) Msg.common_show else Msg.common_hide, tint = dfaint, modifier = Modifier.size(14.dp))
        }
        // the project's icon in its colour (the hub's; an unknown name is the folder), or the logo for a project named Backplane
        if (r.icon == "backplane") androidx.compose.foundation.Image(androidx.compose.ui.res.painterResource(if (dcs.background.luminance() > 0.5f) R.drawable.brand_light else R.drawable.brand_dark), null, Modifier.size(15.dp).testTag("home.section.logo.${r.key}"))
        else androidx.compose.material3.Icon(badgeGlyph(r.icon), null, tint = runCatching { Color(android.graphics.Color.parseColor(r.css)) }.getOrDefault(dcs.onSurfaceVariant), modifier = Modifier.size(15.dp).testTag("home.section.icon.${r.key}"))
        Spacer(Modifier.width(8.dp))
        DMono(r.title, dcs.onSurface, 13, Modifier.weight(1f, fill = false))
        if (r.dots.size > 1) { Spacer(Modifier.width(6.dp)); r.dots.forEach { (here, dim) -> DDot(if (dim) dfaint else if (here) dok else dcs.onSurfaceVariant, 5); Spacer(Modifier.width(3.dp)) } }
        Spacer(Modifier.weight(1f))
        if (r.running > 0) DMono(Msg.ios_sections_running(r.running), dok, 11)
        // what of it waits on the person (its rows are in Needs you, folded or not)
        if (r.needsCount > 0) { Spacer(Modifier.width(6.dp)); Box(Modifier.background(dcs.error).padding(horizontal = 5.dp).testTag("home.section.needs.${r.key}")) { DMono("${r.needsCount}", dcs.onError, 10) } }
    }
}

/** The sections as the list's items (Bub is drawn above them by the home screen). `page` opens a project's page; `menu` a project's long press; `rename` a group's. */
fun LazyListScope.sectionItems(open: (String) -> Unit, page: (DSecRow) -> Unit, menu: (DSecRow) -> Unit, rename: (DSection) -> Unit, openSwarm: (String) -> Unit = {}) {
    // a swarm in Needs you is not repeated under its project
    val asking = DataClient.sections.firstOrNull { it.id == "needs" }?.rows?.mapNotNull { it.swarmRoom }?.toSet() ?: emptySet()
    for (s in DataClient.sections) {
        if (s.kind == "bub" || (s.id == "active" && s.rows.isEmpty())) continue
        item("sec:${s.id}") { SectionHead(s, if (s.rename) ({ rename(s) }) else null) }
        // Needs you: the title has the row (no project name), and a stop says so by its dot; an ask keeps its red word
        if (s.kind == "threads") items(s.rows, key = { "sn:${it.id}" }) { r -> val room = r.swarmRoom
            if (room != null) SectionThread(r) { openSwarm(room) } else SectionThread(if (r.want == "failed" || r.want == "stopped") r.copy(want = "", say = "") else r, open = open) }
        else for (p in s.rows) {
            item("sp:${s.id}:${p.key}") { SectionProject(p, s.id == "quiet" || (p.off && p.key.isNotEmpty()), { page(p) }, { menu(p) }) }
            // its live swarms (the client's own rows, feature swarm), above its threads; their members have no rows
            if (Swarms.on() && !p.folded) items(SwarmUi.live(Swarms.all, p.id).filter { it.id !in asking }, key = { "ss:${s.id}:${it.id}" }) { x -> SwarmRow(x) { openSwarm(x.id) } }
            items(p.threads, key = { "st:${s.id}:${it.id}" }) { t -> SectionThread(t, 34, open = open) }
        }
    }
}

private const val PIN = "project-pin"
private const val MOVE = "group-move"
private const val ARCHIVE = "project-archive"
private const val NEW_GROUP = "new-group"
/** A project row's long press: the hub's menu (Pin, New swarm, Rename, Badge, Move to group, Archive, Remove project); Move to group lists the
 *  other groups and New group…; Remove asks first and only takes the project off Backplane's list. An older hub sends no menu: pin, badge, moves. */
@Composable
fun SectionProjectMenu(p: DSecRow, newSwarm: (String) -> Unit = {}, close: () -> Unit) {
    var shown by remember(p.key) { mutableStateOf(true) }
    var step by remember(p.key) { mutableStateOf("") }   // "" | move | rename | new-group | remove | badge
    var moveShown by remember(p.key) { mutableStateOf(true) }
    val proj = DataClient.projects.firstOrNull { it.id == p.id }
    val items = DMenuItem.phone(p.menu, Swarms.on())
    // the sheet shut and nothing after it (a press that opens a second box sets `step` in the same click)
    if (!shown && step.isEmpty()) LaunchedEffect(Unit) { close() }
    val done: () -> Unit = { step = ""; shown = false }
    if (items.isEmpty()) {
        if (step == "badge" && proj != null) { BadgeSheet(proj) { done(); close() }; return }
        val moves = DataClient.groupsTo.map { (gid, label) -> val go: () -> Unit = { DataClient.sidebarAct(MOVE, p.key + "\u001f" + gid) }; Msg.gpui_sections_move_to(label) to go }
        if (shown) ActionSheet(p.title, listOf((if (p.pinned) Msg.sidebar_sections_unpin else Msg.sidebar_sections_pin) to { DataClient.sidebarAct(PIN, p.key) }) + (if (proj != null) listOf(Msg.gpui_badge to { step = "badge" }) else emptyList()) + (if (Swarms.on()) listOf(Msg.android_swarm_new to { newSwarm(p.id) }) else emptyList()) + moves) { shown = false }
        return
    }
    fun value(id: String) = items.firstOrNull { it.id == id }?.value ?: ""
    if (shown) ActionSheet(p.title, items.map { it -> it.label to {
        when (it.id) {
            "pin" -> DataClient.sidebarAct(PIN, it.value)
            "swarm" -> newSwarm(p.id)
            "archive" -> DataClient.sidebarAct(ARCHIVE, it.value)
            "rename", "badge", "move", "remove" -> step = it.id
            else -> {}
        }
    } }) { shown = false }
    when (step) {
        "badge" -> if (proj != null) BadgeSheet(proj) { done() } else done()
        "rename" -> TextPrompt(Msg.android_project_rename, p.title, Msg.menu_rename, done) { DataClient.projectAct(value("rename"), "rename", it) }
        NEW_GROUP -> TextPrompt(Msg.sidebar_sections_new_group_title, "", Msg.common_create, done) { DataClient.sidebarAct(MOVE, p.key + "\u001f+" + it) }
        "remove" -> ActionSheet(Msg.sidebar_sections_remove_ask(p.title), listOf("Remove project" to { DataClient.projectAct(value("remove"), "remove") }, "Keep it" to {}), done)
        // the other groups, then New group…; a pick shuts the sheet (`moveShown`), New group… asks its name next
        "move" -> { val sub = items.firstOrNull { it.id == "move" }?.sub.orEmpty()
            if (moveShown) ActionSheet(Msg.sidebar_sections_move, sub.map { g -> g.label to { if (g.id == NEW_GROUP) step = NEW_GROUP else DataClient.sidebarAct(MOVE, g.value) } }) { moveShown = false }
            else LaunchedEffect(Unit) { done() } }
    }
}
@Composable
fun SectionRename(s: DSection, close: () -> Unit) { TextPrompt(Msg.sidebar_sections_rename, s.title, Msg.menu_rename, close) { DataClient.sidebarAct("group-rename", s.group + "\u001f" + it) } }

/** A project's page: running, then needs you, then recent, then Done (n) folded; every machine's threads. */
@Composable
fun ProjectPageScreen(p: DSecRow, open: (String) -> Unit, newThread: (String) -> Unit, openSwarm: (String) -> Unit = {}, back: () -> Unit) {
    var done by remember { mutableStateOf(false) }
    var lists by remember { mutableStateOf<List<DSection>?>(null) }
    LaunchedEffect(p.key, done, DataClient.sections) { lists = DataClient.projectPage(p.key, done) }
    Column(Modifier.fillMaxSize().background(dcs.surfaceVariant)) {
        DBar(p.title, back)
        // New thread in this project: its New thread screen says where it runs and has the machine picker (projects.list's row of the same key)
        DataClient.projects.firstOrNull { it.key.isNotEmpty() && it.key == p.key }?.let { pr ->
            DMono("+ " + Msg.android_thread_new_in(pr.title), dcs.primary, 12, Modifier.fillMaxWidth().clickable { newThread(pr.id) }.padding(horizontal = 12.dp, vertical = 10.dp).testTag("project.page.new"))
        }
        val ls = lists
        if (ls == null) { DMono(Msg.viewer_loading, dfaint, 12, Modifier.padding(16.dp)); return@Column }
        LazyColumn(Modifier.fillMaxSize().testTag("project.page")) {
            for (s in ls) {
                if (s.rows.isEmpty() && !(s.fold && s.count > 0)) continue
                item("pg:${s.id}") {
                    Row(Modifier.fillMaxWidth().height(32.dp).clickable(enabled = s.fold) { done = !done }.padding(start = 10.dp, top = 6.dp).testTag("project.page.${s.id}"), verticalAlignment = Alignment.CenterVertically) {
                        if (s.fold) { Icon(if (s.open) Icons.Default.ExpandMore else Icons.Default.ChevronRight, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(14.dp)); Spacer(Modifier.width(6.dp)) }
                        DMono(Tr.t(s.title).uppercase(), dcs.onSurfaceVariant, 11)
                    }
                }
                items(s.rows, key = { "pgt:${s.id}:${it.id}" }) { r -> val room = r.swarmRoom; SectionThread(r) { if (room != null) openSwarm(room) else open(it) } }
            }
            if (ls.all { it.rows.isEmpty() && it.count == 0 }) item("pg:none") { DMono(Msg.android_threads_none, dfaint, 12, Modifier.padding(16.dp)) }
        }
    }
}
