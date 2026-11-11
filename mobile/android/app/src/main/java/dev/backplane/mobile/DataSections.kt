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
import androidx.compose.material3.Icon
import androidx.compose.runtime.*
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
    val off: Boolean, val nav: String, val pinned: Boolean, val key: String, val running: Int, val threads: List<DSecRow>, val css: String, val dots: List<Pair<Boolean, Boolean>>) {
    companion object {
        fun of(o: JSONObject): DSecRow = DSecRow(o.optString("id"), o.optString("title"), o.optString("word"), o.optString("say"), o.optString("want"), o.optString("ago"),
            parseMark(o.optJSONObject("mark")), o.optString("project"), o.optBoolean("off"), o.optString("nav"), o.optBoolean("pinned"), o.optString("key"), o.optInt("running"),
            list(o.optJSONArray("threads")), o.optString("css"), (0 until (o.optJSONArray("dots")?.length() ?: 0)).map { i -> o.getJSONArray("dots").getJSONObject(i).let { it.optBoolean("here") to it.optBoolean("dim") } })
        fun list(a: JSONArray?): List<DSecRow> = (0 until (a?.length() ?: 0)).map { of(a!!.getJSONObject(it)) }
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
@Composable
fun SectionThread(r: DSecRow, indent: Int = 12, showProject: Boolean = false, open: (String) -> Unit) {
    Row(Modifier.fillMaxWidth().height(36.dp).alpha(if (r.off) .55f else 1f).clickable(enabled = !r.off) { open(r.nav) }.padding(start = indent.dp, end = 12.dp).testTag("home.section.thread.${r.id}"), verticalAlignment = Alignment.CenterVertically) {
        DDot(toneOf(r.word), 7); Spacer(Modifier.width(8.dp))
        DMono(r.title, dcs.onSurface, 13, Modifier.weight(1f, fill = false))
        if (showProject) { Spacer(Modifier.width(8.dp)); DMono(r.project, dfaint, 11) }
        Spacer(Modifier.weight(1f))
        r.mark?.let { DMono(it.short + if (it.away) " · " + Msg.android_away else "", dfaint, 11); Spacer(Modifier.width(6.dp)) }
        if (r.want.isNotEmpty()) DMono(Tr.t(r.want), dcs.error, 11) else DMono(Tr.t(r.say.ifEmpty { r.ago }), dfaint, 11)
    }
}
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SectionProject(r: DSecRow, faded: Boolean, onClick: () -> Unit, onLong: () -> Unit) {
    Row(Modifier.fillMaxWidth().height(38.dp).alpha(if (faded) .55f else 1f).combinedClickable(onClick = onClick, onLongClick = onLong).padding(horizontal = 12.dp).testTag("home.section.project.${r.key}"), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(8.dp).background(runCatching { Color(android.graphics.Color.parseColor(r.css)) }.getOrDefault(dcs.onSurfaceVariant))); Spacer(Modifier.width(8.dp))
        DMono(r.title, dcs.onSurface, 13, Modifier.weight(1f, fill = false))
        if (r.dots.size > 1) { Spacer(Modifier.width(6.dp)); r.dots.forEach { (here, dim) -> DDot(if (dim) dfaint else if (here) dok else dcs.onSurfaceVariant, 5); Spacer(Modifier.width(3.dp)) } }
        Spacer(Modifier.weight(1f))
        if (r.running > 0) DMono(Msg.ios_sections_running(r.running), dok, 11)
    }
}

/** The sections as the list's items (Bub is drawn above them by the home screen). `page` opens a project's page; `menu` a project's long press; `rename` a group's. */
fun LazyListScope.sectionItems(open: (String) -> Unit, page: (DSecRow) -> Unit, menu: (DSecRow) -> Unit, rename: (DSection) -> Unit) {
    for (s in DataClient.sections) {
        if (s.kind == "bub" || (s.id == "active" && s.rows.isEmpty())) continue
        item("sec:${s.id}") { SectionHead(s, if (s.rename) ({ rename(s) }) else null) }
        if (s.kind == "threads") items(s.rows, key = { "sn:${it.id}" }) { r -> SectionThread(r, showProject = true, open = open) }
        else for (p in s.rows) {
            item("sp:${s.id}:${p.key}") { SectionProject(p, s.id == "quiet" || (p.off && p.key.isNotEmpty()), { page(p) }, { menu(p) }) }
            items(p.threads, key = { "st:${s.id}:${it.id}" }) { t -> SectionThread(t, 34, open = open) }
        }
    }
}

private const val PIN = "project-pin"
private const val MOVE = "group-move"
/** A project row's long press: pin or unpin it, move it to a group. */
@Composable
fun SectionProjectMenu(p: DSecRow, close: () -> Unit) {
    val pin: () -> Unit = { DataClient.sidebarAct(PIN, p.key) }
    val moves = DataClient.groupsTo.map { (gid, label) -> val go: () -> Unit = { DataClient.sidebarAct(MOVE, p.key + "\u001f" + gid) }; Msg.gpui_sections_move_to(label) to go }
    ActionSheet(p.title, listOf((if (p.pinned) Msg.sidebar_sections_unpin else Msg.sidebar_sections_pin) to pin) + moves, close)
}
@Composable
fun SectionRename(s: DSection, close: () -> Unit) { TextPrompt(Msg.sidebar_sections_rename, s.title, Msg.menu_rename, close) { DataClient.sidebarAct("group-rename", s.group + "\u001f" + it) } }

/** A project's page: running, then needs you, then recent, then Done (n) folded; every machine's threads. */
@Composable
fun ProjectPageScreen(p: DSecRow, open: (String) -> Unit, back: () -> Unit) {
    var done by remember { mutableStateOf(false) }
    var lists by remember { mutableStateOf<List<DSection>?>(null) }
    LaunchedEffect(p.key, done, DataClient.sections) { lists = DataClient.projectPage(p.key, done) }
    Column(Modifier.fillMaxSize().background(dcs.surfaceVariant)) {
        DBar(p.title, back)
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
                items(s.rows, key = { "pgt:${s.id}:${it.id}" }) { r -> SectionThread(r, open = open) }
            }
            if (ls.all { it.rows.isEmpty() && it.count == 0 }) item("pg:none") { DMono(Msg.android_threads_none, dfaint, 12, Modifier.padding(16.dp)) }
        }
    }
}
