package dev.backplane.mobile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Memory
import androidx.compose.material.icons.filled.NotificationsActive
import androidx.compose.material.icons.filled.PriorityHigh
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.QuestionMark
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material.icons.filled.Sync
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

// The sidebar's sections (docs/ALWAYS-ON.md section 8, the hub's present/sections.rs through data/1 `sidebar.sections`):
// Inbox (`needs`, `running`, `snoozed`) and Bench (`pinned`, `active`, `group:*`, `all`, `quiet`).
// The hub decides what is in each; the phone renders them with mobile-native touch ergonomics, explicit icon+word status pills, and swipe actions.

/** One snooze span offered by the hub (`row-snooze` action with `value`, e.g. `<id>|1h`). */
data class DSnoozeOpt(val label: String, val value: String) {
    companion object {
        fun list(a: JSONArray?): List<DSnoozeOpt> = (0 until (a?.length() ?: 0)).mapNotNull { i ->
            a!!.optJSONObject(i)?.let { o ->
                val l = o.optString("label"); val v = o.optString("value")
                if (l.isNotEmpty() && v.isNotEmpty()) DSnoozeOpt(l, v) else null
            }
        }
    }
}

/** A row of a section: a thread, or a project (its `key` set) with its visible threads under it. */
data class DSecRow(val id: String, val title: String, val word: String, val say: String, val want: String, val ago: String, val mark: DMark?, val project: String,
    val off: Boolean, val nav: String, val pinned: Boolean, val key: String, val running: Int, val threads: List<DSecRow>, val css: String, val dots: List<Pair<Boolean, Boolean>>, val kind: String = "", val hide: String = "", val icon: String = "", val menu: List<DMenuItem> = emptyList(),
    /** the fold the hub keeps for a project row (`sidebar.fold.<key>`): the action its chevron sends, folded now, the rows it hides or would hide */
    val fold: String = "", val folded: Boolean = false, val under: Int = 0, val needsCount: Int = 0,
    /** a project's threads with no row under it (finished ones, on its page): counted on its row so none seems lost */
    val rest: Int = 0,
    /** a swarm row's need (present/swarm_need.rs): the line saying what happened and the actions that resolve it; null when nothing waits on the person */
    val need: DNeed? = null,
    /** explicit status pill word & tone from the hub (`badge` in present/sections.rs: bad | run | wait | ok | "") */
    val label: String = "", val tone: String = "",
    /** snooze duration choices (`row-snooze`), wake value (`<id>|off` on snoozed rows), and one-swipe settle id */
    val snooze: List<DSnoozeOpt> = emptyList(), val wake: String = "", val settle: String = "",
    /** hardware indicators for Bench project rows (present/sections.rs `hasBoard`/`hasSchematic`) */
    val hasBoard: Boolean = false, val hasSchematic: Boolean = false) {
    companion object {
        fun of(o: JSONObject): DSecRow = DSecRow(o.optString("id"), o.optString("title"), o.optString("word"), o.optString("say"), o.optString("want"), o.optString("ago"),
            parseMark(o.optJSONObject("mark")), o.optString("project"), o.optBoolean("off"), o.optString("nav"), o.optBoolean("pinned"), o.optString("key"), o.optInt("running"),
            list(o.optJSONArray("threads")), o.optString("css"), (0 until (o.optJSONArray("dots")?.length() ?: 0)).map { i -> o.getJSONArray("dots").getJSONObject(i).let { it.optBoolean("here") to it.optBoolean("dim") } }, o.optString("kind"), o.optString("hide"), o.optString("icon"), DMenuItem.list(o.optJSONArray("menu")),
            o.optString("fold"), o.optBoolean("folded"), o.optInt("under"), o.optInt("needsCount"), o.optInt("rest"), DNeed.of(o.optJSONObject("need")),
            o.optString("label"), o.optString("tone"), DSnoozeOpt.list(o.optJSONArray("snooze")), o.optString("wake"), o.optString("settle"),
            o.optBoolean("hasBoard"), o.optBoolean("hasSchematic"))
        fun list(a: JSONArray?): List<DSecRow> = (0 until (a?.length() ?: 0)).map { of(a!!.getJSONObject(it)) }
    }
    /** a chevron on the row: something is under it, or it is folded (an older hub sends no fold) */
    val canFold: Boolean get() = fold.isNotEmpty() && (folded || under > 0)
    /** a live swarm's row (`kind` swarm): it opens the swarm's page, its room */
    val swarmRoom: String? get() = if (kind == "swarm") nav.removePrefix("@room:").takeIf { it.isNotEmpty() && nav.startsWith("@room:") } else null
}

/** Explicit, glanceable icon+word status pill resolved from hub `label`+`tone` (or legacy `word`+`want`). Pure, unit-tested. */
enum class PillKind { APPROVE, INPUT, SIGN_IN, FAILED, STOPPED, WORKING, WAITING, DONE, SNOOZED }
enum class PillTone { BAD, BAD_OUTLINE, RUN, WAIT, OK, DIM }
data class StatusPillModel(val kind: PillKind, val label: String, val tone: PillTone)

/** Resolve the status pill for a thread/swarm row; returns null when idle with nothing to say.
 *  Snoozed rows keep their real status (Failed/Working/Done…) since the section header already says Snoozed and line 2 shows "Wakes in …". */
fun statusPillOf(r: DSecRow): StatusPillModel? {
    val rawLabel = r.label.trim()
    val rawTone = r.tone.trim()
    if (rawLabel.isNotEmpty()) {
        val kind = when (rawLabel.lowercase()) {
            "approve" -> PillKind.APPROVE
            "sign in" -> PillKind.SIGN_IN
            "input" -> PillKind.INPUT
            "failed" -> PillKind.FAILED
            "stopped" -> PillKind.STOPPED
            "working" -> PillKind.WORKING
            "waiting" -> PillKind.WAITING
            "done" -> PillKind.DONE
            else -> when (rawTone) {
                "bad" -> PillKind.INPUT
                "run" -> PillKind.WORKING
                "wait" -> PillKind.WAITING
                "ok" -> PillKind.DONE
                else -> PillKind.SNOOZED
            }
        }
        val tone = when {
            kind == PillKind.FAILED || kind == PillKind.STOPPED -> PillTone.BAD_OUTLINE
            rawTone == "bad" -> PillTone.BAD
            rawTone == "run" -> PillTone.RUN
            rawTone == "wait" -> PillTone.WAIT
            rawTone == "ok" -> PillTone.OK
            else -> PillTone.DIM
        }
        return StatusPillModel(kind, rawLabel, tone)
    }
    val derived = when (r.word) {
        "needs" -> when (r.want) {
            "approval" -> StatusPillModel(PillKind.APPROVE, "Approve", PillTone.BAD)
            "sign-in" -> StatusPillModel(PillKind.SIGN_IN, "Sign in", PillTone.BAD)
            else -> StatusPillModel(PillKind.INPUT, "Input", PillTone.BAD)
        }
        "failed" -> StatusPillModel(PillKind.FAILED, "Failed", PillTone.BAD_OUTLINE)
        "cut", "interrupted" -> StatusPillModel(PillKind.STOPPED, "Stopped", PillTone.BAD_OUTLINE)
        "running" -> StatusPillModel(PillKind.WORKING, "Working", PillTone.RUN)
        "monitoring", "limit" -> StatusPillModel(PillKind.WAITING, "Waiting", PillTone.WAIT)
        "done" -> StatusPillModel(PillKind.DONE, "Done", PillTone.OK)
        else -> null
    }
    return derived ?: if (r.wake.isNotEmpty()) StatusPillModel(PillKind.SNOOZED, "Snoozed", PillTone.DIM) else null
}

/** Partition the hub's sections into the mobile-native Inbox and Bench tabs. Pure, unit-tested. */
data class MobilePartition(
    val inbox: List<DSection>,
    val bench: List<DSection>,
    val needsCount: Int,
    val runningCount: Int,
    val snoozedCount: Int,
) {
    val inboxEmpty: Boolean get() = needsCount == 0 && runningCount == 0
    /** Top projects (Pinned first, then Recent/active) for the calm "All clear" quick-jump strip. */
    fun quickJumpProjects(limit: Int = 3): List<DSecRow> {
        val seen = HashSet<String>()
        val out = ArrayList<DSecRow>(limit)
        for (id in listOf("pinned", "active")) {
            val sec = bench.firstOrNull { it.id == id } ?: continue
            for (r in sec.rows) {
                if (r.key.isNotEmpty() && seen.add(r.key)) {
                    out.add(r)
                    if (out.size >= limit) return out
                }
            }
        }
        return out
    }
}

private val INBOX_IDS = setOf("needs", "running", "snoozed")

fun partitionSections(sections: List<DSection>): MobilePartition {
    val inbox = sections.filter { it.id in INBOX_IDS }
    val bench = sections.filter { it.kind == "projects" && !(it.id == "active" && it.rows.isEmpty()) }
    val needs = sections.firstOrNull { it.id == "needs" }?.let { maxOf(it.count, it.rows.size) } ?: 0
    val running = sections.firstOrNull { it.id == "running" }?.let { maxOf(it.count, it.rows.size) } ?: 0
    val snoozed = sections.firstOrNull { it.id == "snoozed" }?.let { maxOf(it.count, it.rows.size) } ?: 0
    return MobilePartition(inbox, bench, needs, running, snoozed)
}

/** One button of a swarm's need: `act` says what a press does (NeedUi.press), `room` and `thread` are the hub's ids (a far hub's with its prefix). */
data class DNeedAct(val label: String, val act: String, val room: String, val thread: String)
data class DNeed(val line: String, val why: String, val actions: List<DNeedAct>) {
    companion object {
        fun of(o: JSONObject?): DNeed? = o?.let { n -> DNeed(n.optString("line"), n.optString("why"), (0 until (n.optJSONArray("actions")?.length() ?: 0)).mapNotNull { n.getJSONArray("actions").optJSONObject(it) }
            .map { DNeedAct(it.optString("label"), it.optString("act"), it.optString("room"), it.optString("thread")) }).takeIf { it.line.isNotEmpty() || it.actions.isNotEmpty() } }
    }
}
/** What a press of a need's button does: open the swarm's page (as a tap on its row), open a thread or an account's sign-in, or ask the hub (`swarm.act`). */
sealed interface NeedGo {
    object Board : NeedGo
    data class Thread(val id: String) : NeedGo
    data class Account(val id: String) : NeedGo
    data class Call(val method: String, val params: JSONObject) : NeedGo
}
/** The need's presses, pure (SwarmNeedTest). */
object NeedUi {
    private val asks = setOf("resume", "raise", "retry", "move", "signin")
    fun press(a: DNeedAct): NeedGo? = when {
        a.act == "board" -> NeedGo.Board
        a.act == "open" -> a.thread.takeIf { it.isNotEmpty() }?.let { NeedGo.Thread(it) }
        a.act in asks -> NeedGo.Call("swarm.act", JSONObject().put("room", a.room).put("action", a.act).also { if (a.thread.isNotEmpty()) it.put("thread", a.thread) })
        else -> null
    }
    /** what `swarm.act`'s reply opens next: `open` "account:<id>" (that account's sign-in) or "thread:<id>" (the terminal of a provider's sign-in is on it) */
    fun after(reply: JSONObject?): NeedGo? { val o = reply?.optString("open").orEmpty()
        return when { o.startsWith("account:") -> NeedGo.Account(o.removePrefix("account:")); o.startsWith("thread:") -> NeedGo.Thread(o.removePrefix("thread:")); else -> null }.takeIf { o.substringAfter(':').isNotEmpty() } }
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
data class DSection(val id: String, val kind: String, val title: String, val count: Int, val fold: Boolean, val open: Boolean, val toggle: String, val group: String, val rename: Boolean, val rows: List<DSecRow>, val hint: String = "") {
    companion object {
        fun list(a: JSONArray?): List<DSection> = (0 until (a?.length() ?: 0)).map { i -> a!!.getJSONObject(i).let { o ->
            DSection(o.optString("id"), o.optString("kind"), o.optString("title"), o.optInt("count"), o.optBoolean("fold"), o.optBoolean("open"), o.optString("toggle"), o.optString("group"), o.optBoolean("rename"), DSecRow.list(o.optJSONArray("rows")), o.optString("hint")) } }
        fun groups(a: JSONArray?): List<Pair<String, String>> = (0 until (a?.length() ?: 0)).map { i -> a!!.getJSONObject(i).let { it.optString("value") to it.optString("label") } }
    }
}

private fun pillIcon(kind: PillKind): ImageVector = when (kind) {
    PillKind.APPROVE -> Icons.Default.PriorityHigh
    PillKind.INPUT -> Icons.Default.QuestionMark
    PillKind.SIGN_IN -> Icons.Default.Key
    PillKind.FAILED -> Icons.Default.Close
    PillKind.STOPPED -> Icons.Default.Stop
    PillKind.WORKING -> Icons.Default.Sync
    PillKind.WAITING -> Icons.Default.Schedule
    PillKind.DONE -> Icons.Default.Check
    PillKind.SNOOZED -> Icons.Default.Schedule
}

/** Square-cornered, glanceable icon+word status pill (min 24dp tall) replacing tiny colored dots. */
@Composable
fun StatusPill(pill: StatusPillModel, modifier: Modifier = Modifier) {
    val bg: Color
    val fg: Color
    val border: Color
    when (pill.tone) {
        PillTone.BAD -> { bg = dcs.error; fg = dcs.onError; border = dcs.error }
        PillTone.BAD_OUTLINE -> { bg = dcs.errorContainer; fg = dcs.error; border = dcs.error }
        PillTone.RUN -> { bg = dcs.surfaceContainerHigh; fg = dcs.primary; border = dcs.primary }
        PillTone.WAIT -> { bg = dcs.surfaceContainerHigh; fg = dwarn; border = dwarn }
        PillTone.OK -> { bg = dcs.surfaceContainerHigh; fg = dok; border = dok }
        PillTone.DIM -> { bg = dcs.surfaceContainerHigh; fg = dcs.onSurfaceVariant; border = dcs.outlineVariant }
    }
    Row(
        modifier
            .heightIn(min = 24.dp)
            .background(bg, RectangleShape)
            .border(BorderStroke(1.dp, border), RectangleShape)
            .padding(horizontal = 7.dp, vertical = 3.dp)
            .testTag("pill.${pill.kind.name.lowercase()}"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Icon(pillIcon(pill.kind), null, tint = fg, modifier = Modifier.size(12.dp))
        Text(Tr.t(pill.label), color = fg, fontFamily = BpMono, fontSize = 11.sp, fontWeight = FontWeight.Bold, maxLines = 1)
    }
}

/** Subtle hardware/PCB indicator for Bench project rows with a canonical KiCad board (`hasBoard`). */
@Composable
fun BoardBadge(modifier: Modifier = Modifier) {
    Row(
        modifier
            .heightIn(min = 20.dp)
            .background(dcs.surfaceContainerHigh, RectangleShape)
            .border(BorderStroke(1.dp, dok.copy(alpha = 0.55f)), RectangleShape)
            .padding(horizontal = 5.dp, vertical = 2.dp)
            .testTag("badge.pcb"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        Icon(Icons.Default.Memory, null, tint = dok, modifier = Modifier.size(11.dp))
        DMono(Msg.mobile_badge_pcb, dok, 10, bold = true)
    }
}

@Composable
fun SectionHead(s: DSection, onLong: (() -> Unit)?) {
    @OptIn(ExperimentalFoundationApi::class)
    Column(Modifier.fillMaxWidth().combinedClickable(onClick = { if (s.fold) DataClient.toggleSection(s.toggle) }, onLongClick = onLong).padding(start = 12.dp, end = 12.dp, top = 12.dp, bottom = 4.dp).testTag("home.section.${s.id}")) {
        Row(Modifier.fillMaxWidth().heightIn(min = 28.dp), verticalAlignment = Alignment.CenterVertically) {
            if (s.fold) { Icon(if (s.open) Icons.Default.ExpandMore else Icons.Default.ChevronRight, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(6.dp)) }
            DMono(Tr.t(s.title).uppercase(), if (s.id == "needs") dcs.error else dcs.onSurfaceVariant, 11, bold = s.id == "needs")
            if (s.fold || s.kind == "threads") { Spacer(Modifier.width(6.dp)); DMono("${s.count}", dfaint, 11) }
        }
        if (s.hint.isNotEmpty() && s.open) DMono(Tr.t(s.hint), dfaint, 10, Modifier.padding(top = 2.dp))
    }
}

/** A mobile-native thread row (min 56dp): two-line title + project/meta subtitle, explicit right-aligned StatusPill,
 *  and native Material 3 swipe actions (trailing swipe = Snooze sheet, leading swipe = Settle or Wake). */
@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
@Composable
fun SectionThread(r: DSecRow, indent: Int = 12, showProject: Boolean = false, openThread: (String) -> Unit = {}, open: (String) -> Unit) {
    var menu by remember { mutableStateOf(false) }
    var snoozeSheet by remember { mutableStateOf(false) }
    val acts = if (r.key.isEmpty() && r.swarmRoom == null) r.menu.filter { it.label.isNotEmpty() } else emptyList()
    val need = r.need?.takeIf { r.kind == "swarm" }
    val pill = remember(r.label, r.tone, r.word, r.want, r.wake) { statusPillOf(r) }
    val canSwipeStart = r.wake.isNotEmpty() || r.settle.isNotEmpty()
    val canSwipeEnd = r.snooze.isNotEmpty()
    val hasMenu = r.hide.isNotEmpty() || acts.isNotEmpty() || canSwipeEnd || r.wake.isNotEmpty()

    val meta = buildList {
        if (showProject && r.project.isNotEmpty()) add(r.project)
        r.mark?.let { add(it.short + if (it.away) " · " + Msg.android_away else "") }
        if (r.ago.isNotEmpty()) add(if (r.wake.isNotEmpty()) Msg.mobile_snooze_wakes(r.ago) else r.ago)
    }.joinToString(" · ")

    val rowContent: @Composable () -> Unit = {
        Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant)) {
            Box {
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 56.dp)
                        .alpha(if (r.off) .55f else 1f)
                        .combinedClickable(
                            onClick = { if (!r.off) open(r.nav) },
                            onLongClick = if (hasMenu) ({ menu = true }) else null,
                        )
                        .padding(start = indent.dp, end = 12.dp, top = 8.dp, bottom = 8.dp)
                        .testTag("home.section.thread.${r.id}"),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                r.title,
                                color = dcs.onSurface,
                                fontFamily = BpMono,
                                fontSize = 14.sp,
                                fontWeight = if (r.word == "needs") FontWeight.Bold else FontWeight.Normal,
                                maxLines = 1,
                                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                                modifier = Modifier.weight(1f, fill = false),
                            )
                            if (r.pinned) {
                                Spacer(Modifier.width(6.dp))
                                Icon(Icons.Default.PushPin, Msg.tools_pinned, tint = dcs.primary, modifier = Modifier.size(12.dp).testTag("home.section.pinned.${r.id}"))
                            }
                        }
                        if (meta.isNotEmpty()) DMono(meta, dcs.onSurfaceVariant, 11)
                    }
                    if (pill != null && need == null) {
                        Spacer(Modifier.width(10.dp))
                        StatusPill(pill)
                    }
                }
                DropdownMenu(menu, { menu = false }) {
                    if (r.wake.isNotEmpty()) {
                        DropdownMenuItem(text = { Text(Msg.menu_wake, fontFamily = BpMono) }, onClick = { menu = false; DataClient.snoozeRow(r.wake) }, modifier = Modifier.testTag("home.section.wake.${r.id}"))
                    }
                    if (r.snooze.isNotEmpty()) {
                        DropdownMenuItem(text = { Text(Msg.tools_snooze, fontFamily = BpMono) }, onClick = { menu = false; snoozeSheet = true }, modifier = Modifier.testTag("home.section.snooze.${r.id}"))
                    }
                    if (r.hide.isNotEmpty()) DropdownMenuItem(text = { Text(Msg.common_hide, fontFamily = BpMono) }, onClick = { menu = false; DataClient.sidebarAct("thread-hide", r.hide) }, modifier = Modifier.testTag("home.section.hide.${r.id}"))
                    acts.forEach { m -> DropdownMenuItem(text = { Text(Tr.t(m.label), fontFamily = BpMono) }, onClick = { menu = false; DataClient.sectionThreadAct(r.id, m.id) }, modifier = Modifier.testTag("home.section.${m.id}.${r.id}")) }
                }
            }
            if (need != null) SwarmNeedLine(r, need, indent, { open(r.nav) }, openThread)
            DLine()
        }
    }

    if (canSwipeStart || canSwipeEnd) {
        val swipeState = rememberSwipeToDismissBoxState(
            confirmValueChange = { v ->
                when (v) {
                    SwipeToDismissBoxValue.StartToEnd -> {
                        if (r.wake.isNotEmpty()) DataClient.snoozeRow(r.wake)
                        else if (r.settle.isNotEmpty()) DataClient.sectionThreadAct(r.id, "settle")
                        false
                    }
                    SwipeToDismissBoxValue.EndToStart -> {
                        if (r.snooze.isNotEmpty()) snoozeSheet = true
                        false
                    }
                    SwipeToDismissBoxValue.Settled -> false
                }
            }
        )
        SwipeToDismissBox(
            state = swipeState,
            enableDismissFromStartToEnd = canSwipeStart,
            enableDismissFromEndToStart = canSwipeEnd,
            backgroundContent = {
                val dir = swipeState.dismissDirection
                val isStart = dir == SwipeToDismissBoxValue.StartToEnd
                val bg = when {
                    isStart && r.wake.isNotEmpty() -> dcs.primary
                    isStart -> dok
                    dir == SwipeToDismissBoxValue.EndToStart -> dwarn
                    else -> dcs.surfaceContainerHigh
                }
                val light = dcs.background.luminance() > 0.5f
                val fg = when {
                    isStart && r.wake.isNotEmpty() -> dcs.onPrimary
                    light -> Color.White
                    else -> Color(0xFF06121F)
                }
                val align = if (isStart) Alignment.CenterStart else Alignment.CenterEnd
                val icon = when {
                    isStart && r.wake.isNotEmpty() -> Icons.Default.NotificationsActive
                    isStart -> Icons.Default.Check
                    else -> Icons.Default.Schedule
                }
                val text = when {
                    isStart && r.wake.isNotEmpty() -> Msg.menu_wake
                    isStart -> Msg.menu_settle
                    else -> Msg.tools_snooze
                }
                Box(Modifier.fillMaxSize().background(bg).padding(horizontal = 18.dp), contentAlignment = align) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        Icon(icon, text, tint = fg, modifier = Modifier.size(18.dp))
                        DMono(text.uppercase(), fg, 12, bold = true)
                    }
                }
            },
            content = { rowContent() },
        )
    } else {
        rowContent()
    }

    if (snoozeSheet && r.snooze.isNotEmpty()) {
        ActionSheet(
            Msg.mobile_snooze_for,
            r.snooze.map { opt -> Tr.t(opt.label) to { snoozeSheet = false; DataClient.snoozeRow(opt.value) } },
        ) { snoozeSheet = false }
    }
}

/** A swarm's need under its row: the hub's line and a small button per action; a refusal's words under them. */
@Composable
private fun SwarmNeedLine(r: DSecRow, n: DNeed, indent: Int, board: () -> Unit, openThread: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var busy by remember(r.id) { mutableStateOf(false) }
    var err by remember(r.id) { mutableStateOf("") }
    fun go(g: NeedGo?) { when (g) {
        NeedGo.Board -> board()
        is NeedGo.Thread -> openThread(g.id)
        is NeedGo.Account -> DataClient.openAccount = g.id
        is NeedGo.Call -> { busy = true; err = ""
            scope.launch { try { go(NeedUi.after(DataClient.call(g.method, g.params))); Swarms.load() } catch (e: Exception) { err = e.message ?: "" } finally { busy = false } } }
        null -> {}
    } }
    Column(Modifier.fillMaxWidth().alpha(if (r.off) .55f else 1f).padding(start = indent.dp, end = 12.dp, bottom = 10.dp).testTag("home.section.need.${r.id}")) {
        DMono(Tr.t(n.line), dcs.error, 11, Modifier.testTag("home.section.need.line.${r.id}"), lines = 2)
        if (n.actions.isNotEmpty()) {
            Spacer(Modifier.height(6.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                n.actions.forEach { a -> DBtn(Tr.t(a.label), false, !busy && NeedUi.press(a) != null, Modifier.testTag("home.section.need.${a.act}.${r.id}"), small = true) { go(NeedUi.press(a)) } }
            }
        }
        if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 11, Modifier.padding(top = 4.dp), lines = 3)
    }
}

/** A Bench project row (min 56dp): icon, title, optional PCB hardware badge, machine dots, and running/needs indicators. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SectionProject(r: DSecRow, faded: Boolean, onClick: () -> Unit, onLong: () -> Unit) {
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 56.dp)
                .alpha(if (faded) .55f else 1f)
                .combinedClickable(onClick = onClick, onLongClick = onLong)
                .padding(start = if (r.canFold) 4.dp else 12.dp, end = 12.dp, top = 8.dp, bottom = 8.dp)
                .testTag("home.section.project.${r.key}"),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (r.canFold) {
                Box(Modifier.size(width = 36.dp, height = 40.dp).clickable { DataClient.sidebarAct(r.fold, r.key) }.testTag("home.section.fold.${r.key}"), contentAlignment = Alignment.Center) {
                    Icon(if (r.folded) Icons.Default.ChevronRight else Icons.Default.ExpandMore, if (r.folded) Msg.common_show else Msg.common_hide, tint = dfaint, modifier = Modifier.size(16.dp))
                }
            }
            if (r.icon == "backplane") androidx.compose.foundation.Image(androidx.compose.ui.res.painterResource(if (dcs.background.luminance() > 0.5f) R.drawable.brand_light else R.drawable.brand_dark), null, Modifier.size(18.dp).testTag("home.section.logo.${r.key}"))
            else Icon(badgeGlyph(r.icon), null, tint = runCatching { Color(android.graphics.Color.parseColor(r.css)) }.getOrDefault(dcs.onSurfaceVariant), modifier = Modifier.size(18.dp).testTag("home.section.icon.${r.key}"))
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    DMono(r.title, dcs.onSurface, 14, Modifier.weight(1f, fill = false), bold = r.pinned || r.needsCount > 0)
                    if (r.hasBoard) BoardBadge()
                    if (r.pinned) Icon(Icons.Default.PushPin, Msg.tools_pinned, tint = dcs.primary, modifier = Modifier.size(12.dp))
                }
                if (r.dots.size > 1 || r.running > 0) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        if (r.running > 0) DMono(Msg.ios_sections_running(r.running), dcs.primary, 11)
                        if (r.dots.size > 1) {
                            Row(horizontalArrangement = Arrangement.spacedBy(3.dp), verticalAlignment = Alignment.CenterVertically) {
                                r.dots.forEach { (here, dim) -> DDot(if (dim) dfaint else if (here) dok else dcs.onSurfaceVariant, 5) }
                            }
                        }
                    }
                }
            }
            if (r.needsCount > 0) {
                Spacer(Modifier.width(8.dp))
                Box(Modifier.background(dcs.error, RectangleShape).padding(horizontal = 7.dp, vertical = 3.dp).testTag("home.section.needs.${r.key}")) {
                    DMono("${r.needsCount}", dcs.onError, 11, bold = true)
                }
            }
            Spacer(Modifier.width(4.dp))
            Icon(Icons.Default.ChevronRight, null, tint = dfaint, modifier = Modifier.size(16.dp))
        }
        DLine()
    }
}

/** Inbox tab items: Needs you → Running → Snoozed (collapsible), each row naming its project. */
fun LazyListScope.inboxItems(inbox: List<DSection>, open: (String) -> Unit, openSwarm: (String) -> Unit = {}) {
    for (s in inbox) {
        if (s.rows.isEmpty() && !(s.fold && s.count > 0)) continue
        item("sec:${s.id}") { SectionHead(s, null) }
        items(s.rows, key = { "sn:${s.id}:${it.id}" }) { r ->
            val room = r.swarmRoom
            if (room != null) SectionThread(r, showProject = true, openThread = open) { openSwarm(room) }
            else SectionThread(r, showProject = true, open = open)
        }
    }
}

/** Calm "All clear" empty Inbox card: 1-tap Talk to Bub + top 3 Pinned/Recent projects + Open Bench. */
@Composable
fun InboxAllClearCard(
    bub: DataClient.Bot?,
    quickProjects: List<DSecRow>,
    openBub: (String) -> Unit,
    openProject: (DSecRow) -> Unit,
    openBench: () -> Unit,
) {
    Column(Modifier.fillMaxWidth().padding(16.dp).testTag("inbox.all_clear"), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Column(
            Modifier
                .fillMaxWidth()
                .background(dcs.surface, RectangleShape)
                .border(BorderStroke(1.dp, dcs.outlineVariant), RectangleShape)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.size(28.dp).background(dok.copy(alpha = 0.16f), RectangleShape).border(BorderStroke(1.dp, dok), RectangleShape), contentAlignment = Alignment.Center) {
                    Icon(Icons.Default.Check, null, tint = dok, modifier = Modifier.size(16.dp))
                }
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    DMono(Msg.mobile_inbox_clear, dcs.onSurface, 15, bold = true)
                    DMono(Msg.mobile_inbox_clear_note, dcs.onSurfaceVariant, 11, lines = 2)
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (bub != null && bub.thread.isNotEmpty()) {
                    DBtn(Msg.mobile_inbox_talk_bub, true, modifier = Modifier.testTag("inbox.clear.bub")) { openBub(bub.thread) }
                }
                DBtn(Msg.mobile_inbox_open_bench, false, modifier = Modifier.testTag("inbox.clear.bench"), onClick = openBench)
            }
        }
        if (quickProjects.isNotEmpty()) {
            DMono(Msg.mobile_inbox_jump_in.uppercase(), dcs.onSurfaceVariant, 11, bold = true)
            Column(Modifier.fillMaxWidth().background(dcs.surface, RectangleShape).border(BorderStroke(1.dp, dcs.outlineVariant), RectangleShape)) {
                quickProjects.forEach { p ->
                    SectionProject(p, faded = false, onClick = { openProject(p) }, onLong = {})
                }
            }
        }
    }
}

/** Bench tab items: Pinned, Recent (`active`, ≤8), Groups, All projects, Quiet. */
fun LazyListScope.benchItems(bench: List<DSection>, open: (String) -> Unit, page: (DSecRow) -> Unit, menu: (DSecRow) -> Unit, rename: (DSection) -> Unit, openSwarm: (String) -> Unit = {}) {
    val asking = DataClient.sections.firstOrNull { it.id == "needs" }?.rows?.mapNotNull { it.swarmRoom }?.toSet() ?: emptySet()
    for (s in bench) {
        if (s.rows.isEmpty() && !(s.fold && s.count > 0)) continue
        item("sec:${s.id}") { SectionHead(s, if (s.rename) ({ rename(s) }) else null) }
        for (p in s.rows) {
            item("sp:${s.id}:${p.key}") { SectionProject(p, s.id == "quiet" || s.id == "archived" || (p.off && p.key.isNotEmpty()), { page(p) }, { menu(p) }) }
            if (Swarms.on() && !p.folded) items(SwarmUi.live(Swarms.all, p.id).filter { it.id !in asking }, key = { "ss:${s.id}:${it.id}" }) { x -> SwarmRow(x) { openSwarm(x.id) } }
            items(p.threads, key = { "st:${s.id}:${it.id}" }) { t -> SectionThread(t, indent = 28, open = open) }
        }
    }
}

/** Legacy single-list sections helper kept for empty-projects fallback. */
fun LazyListScope.sectionItems(open: (String) -> Unit, page: (DSecRow) -> Unit, menu: (DSecRow) -> Unit, rename: (DSection) -> Unit, openSwarm: (String) -> Unit = {}) {
    val part = partitionSections(DataClient.sections)
    inboxItems(part.inbox, open, openSwarm)
    benchItems(part.bench, open, page, menu, rename, openSwarm)
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
            "archive", "quiet" -> DataClient.sidebarAct(ARCHIVE, it.value) // Make quiet / Back to Active (an older hub: Archive)
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
    // a long Done list: its newest rows, the rest behind the hub's "Older (m)" (`page-older`)
    var older by remember { mutableStateOf(false) }
    var lists by remember { mutableStateOf<List<DSection>?>(null) }
    // record this visit on the hub so the Recent bench order follows this phone too
    LaunchedEffect(p.key) { DataClient.visitProject(p.key) }
    LaunchedEffect(p.key, done, older, DataClient.sections) { lists = DataClient.projectPage(p.key, done, older) }
    Column(Modifier.fillMaxSize().background(dcs.surfaceVariant)) {
        DBar(p.title, back)
        // New thread in this project: its New thread screen says where it runs and has the machine picker (projects.list's row of the same key)
        DataClient.projects.firstOrNull { it.key.isNotEmpty() && it.key == p.key }?.let { pr ->
            // Buy it beside New thread, as a thread's header has it: the hub's button for this project. It sits at the end of the New thread row, where a
            // thumb lands, and a buy thread starts an agent's turn at once: a press asks first, and only its answer makes the buy thread and opens it
            var buy by remember(pr.id) { mutableStateOf<DataClient.Buy?>(null) }; var buying by remember(pr.id) { mutableStateOf(false) }; var failed by remember(pr.id) { mutableStateOf("") }
            var asking by remember(pr.id) { mutableStateOf(false) }
            val scope = rememberCoroutineScope()
            LaunchedEffect(pr.id, DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE) runCatching { DataClient.buyOfProject(pr.id) }.onSuccess { buy = it } }
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                DMono("+ " + Msg.android_thread_new_in(pr.title), dcs.primary, 12, Modifier.weight(1f).clickable { newThread(pr.id) }.padding(horizontal = 12.dp, vertical = 12.dp).testTag("project.page.new"))
                buy?.let { b ->
                    DMono(Tr.t(b.label), dcs.primary, 12, Modifier.clickable(enabled = !buying) { asking = true }.padding(horizontal = 12.dp, vertical = 12.dp).testTag("project.page.buy"), bold = true)
                    if (asking) AlertDialog(onDismissRequest = { asking = false }, shape = RectangleShape,
                        title = { Text(Msg.android_buy_ask(pr.title), fontFamily = BpMono, fontSize = 15.sp) },
                        text = if (b.tip.isEmpty()) null else { { Text(Tr.t(b.tip), fontFamily = BpMono, fontSize = 13.sp) } },
                        confirmButton = { DBtn(Msg.android_buy_it, true, modifier = Modifier.testTag("project.page.buy.confirm")) { asking = false; buying = true; failed = ""
                            scope.launch { try { val t = DataClient.buy(b.thread); if (t.isNotEmpty()) open(t) } catch (e: Exception) { failed = e.message ?: Msg.android_buy_failed } finally { buying = false } } } },
                        dismissButton = { DBtn(Msg.common_cancel, false, modifier = Modifier.testTag("project.page.buy.cancel")) { asking = false } })
                }
            }
            if (failed.isNotEmpty()) DMono(failed, dcs.error, 11, Modifier.padding(horizontal = 12.dp, vertical = 4.dp), lines = 3)
            // what the project is, as the desktop's project page opens: its goal (a tap shows it whole) and its boards
            var card by remember(pr.id) { mutableStateOf<DataClient.ProjectCard?>(null) }
            var whole by remember(pr.id) { mutableStateOf(false) }
            LaunchedEffect(pr.id, DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE) runCatching { DataClient.projectCard(pr.id) }.onSuccess { card = it } }
            card?.takeIf { it.goal.isNotEmpty() || it.boards.isNotEmpty() }?.let { c ->
                Column(Modifier.fillMaxWidth().background(dcs.surface).clickable(enabled = c.goal.isNotEmpty()) { whole = !whole }.padding(horizontal = 12.dp, vertical = 8.dp).testTag("project.page.card")) {
                    if (c.goal.isNotEmpty()) DMono(c.goal, dcs.onSurface, 12, lines = if (whole) 20 else 2)
                    if (c.boards.isNotEmpty()) DMono(Msg.android_viewer_boards.uppercase() + "  " + c.boards.joinToString(" · "), dcs.onSurfaceVariant, 10, Modifier.padding(top = if (c.goal.isNotEmpty()) 4.dp else 0.dp), lines = 2)
                }
                DLine()
            }
        }
        val ls = lists
        if (ls == null) { DMono(Msg.viewer_loading, dfaint, 12, Modifier.padding(16.dp)); return@Column }
        LazyColumn(Modifier.fillMaxSize().testTag("project.page")) {
            for (s in ls) {
                if (s.rows.isEmpty() && !(s.fold && s.count > 0)) continue
                item("pg:${s.id}") {
                    Row(Modifier.fillMaxWidth().height(36.dp).clickable(enabled = s.fold) { if (s.toggle == "page-older") older = !older else done = !done }.padding(start = 12.dp, top = 6.dp).testTag("project.page.${s.id}"), verticalAlignment = Alignment.CenterVertically) {
                        if (s.fold) { Icon(if (s.open) Icons.Default.ExpandMore else Icons.Default.ChevronRight, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(6.dp)) }
                        DMono(Tr.t(s.title).uppercase(), dcs.onSurfaceVariant, 11)
                    }
                }
                items(s.rows, key = { "pgt:${s.id}:${it.id}" }) { r -> val room = r.swarmRoom; SectionThread(r, openThread = open) { if (room != null) openSwarm(room) else open(it) } }
            }
            if (ls.all { it.rows.isEmpty() && it.count == 0 }) item("pg:none") { DMono(Msg.android_threads_none, dfaint, 12, Modifier.padding(16.dp)) }
        }
    }
}
