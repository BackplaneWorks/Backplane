package dev.backplane.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Forum
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.json.JSONArray
import org.json.JSONObject

// The status plane (docs/uber.md): the uberbot's card, and what waits on
// the person, what runs, the bots, rooms and routines under it, on the home
// list and as the uberbot's Status tab. All of it is the screen's
// (src/mobile/plane.bend, merged over hubs by hubs.bend); these composables
// only draw it and send each row's act and value back.

// the uberbot's card: a bot's row with the plane's counts
data class UberCard(
    val id: String, val name: String, val mood: String, val cat: String, val counts: String, val sel: Boolean, val nav: String = "",
)

// kind: thread, bot, room, routine; act and value are what a tap sends
data class PlaneRow(
    val kind: String, val id: String, val act: String, val value: String, val title: String, val sub: String,
    val state: String, val ago: String, val machine: String, val away: Boolean, val nav: String = "",
)

data class PlaneMachine(val name: String, val state: String)

// where the uberbot's conversation is held; Bend says whether to offer
// "Make this the home" (button) and what a tap sends (act, value)
data class PlaneHome(val name: String, val line: String, val label: String, val act: String, val value: String, val here: Boolean, val button: Boolean)

data class PlaneModel(
    val need: List<PlaneRow>, val run: List<PlaneRow>, val idle: List<PlaneRow>, val bots: List<PlaneRow>,
    val rooms: List<PlaneRow>, val routines: List<PlaneRow>, val idleN: Int, val working: Int,
    val machines: List<PlaneMachine>,
    val home: PlaneHome?,
    val locks: List<PlaneRow> = emptyList(),
)

private fun planeRows(a: JSONArray?): List<PlaneRow> =
    if (a == null) emptyList() else (0 until a.length()).map {
        val o = a.getJSONObject(it)
        PlaneRow(o.optString("kind"), o.optString("id"), o.optString("act"), o.optString("value"), o.optString("title"),
            o.optString("sub"), o.optString("state"), o.optString("ago"), o.optString("machine"), o.optBoolean("away"), o.optString("nav"))
    }

fun parseUber(o: JSONObject) = UberCard(
    o.optString("id"), o.optString("name"), o.optString("mood"), o.optString("cat"), o.optString("counts"), o.optBoolean("sel"), o.optString("nav"),
)

fun parsePlane(o: JSONObject) = PlaneModel(
    planeRows(o.optJSONArray("need")), planeRows(o.optJSONArray("run")), planeRows(o.optJSONArray("idle")),
    planeRows(o.optJSONArray("bots")), planeRows(o.optJSONArray("rooms")), planeRows(o.optJSONArray("routines")),
    o.optInt("idleN"), o.optInt("working"),
    o.optJSONArray("machines").let { a ->
        if (a == null) emptyList() else (0 until a.length()).map { PlaneMachine(a.getJSONObject(it).optString("name"), a.getJSONObject(it).optString("state")) }
    },
    o.optJSONObject("home")?.let {
        PlaneHome(it.optString("name"), it.optString("line"), it.optString("label"), it.optString("act"), it.optString("value"), it.optBoolean("here"), it.optBoolean("button"))
    },
    planeRows(o.optJSONArray("locks")),
)

// a thread row's dot, from the plane's states
private fun planeStatus(s: String) = when (s) {
    "question" -> "input"
    "away" -> "idle"
    else -> s
}

private fun line(r: PlaneRow) = listOf(r.sub, r.machine).filter { it.isNotEmpty() }.joinToString(" · ")

// The home list
// -------------

@Composable
fun UberCardRow(m: AppModel, c: UberCard) {
    val lang = LocalBackplaneLanguage.current
    ListItem(
        headlineContent = { Text(c.name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                MoodDot(c.mood)
                Text(c.counts.ifEmpty { NativeMessages.native_plane_waiting(lang) }, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        },
        leadingContent = { BotCat(m, c.cat, 48.dp) },
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.primary.copy(alpha = if (c.sel) 0.22f else 0.10f)),
        modifier = Modifier.clickable { m.open(c.nav, "uber", c.id, c.name, c.cat, "bot") },
    )
}

@Composable
private fun PlaneLabel(text: String, count: Int?, modifier: Modifier = Modifier) {
    Row(modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 14.dp, bottom = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(text.uppercase(), style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold), color = MaterialTheme.colorScheme.outline)
        if (count != null) Text("$count", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
    }
}

@Composable
private fun PlaneItem(m: AppModel, r: PlaneRow, mood: Boolean = false) {
    val go: () -> Unit = { if (r.nav.isNotEmpty()) m.open(r.nav, r.act, r.value, r.title, kind = r.kind) else m.act(r.act, r.value) }
    when (r.kind) {
        "room" -> ListItem(
            headlineContent = { Text(r.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            leadingContent = { Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) { Icon(Icons.Filled.Forum, null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.outline) } },
            trailingContent = { Text(r.sub, style = MaterialTheme.typography.labelMedium, color = PhaseColor.accent) },
            modifier = Modifier.clickable(onClick = go),
        )
        "routine" -> ListItem(
            headlineContent = { Text(r.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            supportingContent = { if (line(r).isNotEmpty()) Text(line(r), maxLines = if (r.state == "stalled") 2 else 1, overflow = TextOverflow.Ellipsis) },
            leadingContent = {
                Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                    if (r.state == "working") CircularProgressIndicator(Modifier.size(16.dp), color = PhaseColor.accent, strokeWidth = 2.dp)
                    else Icon(if (r.state == "reminder") Icons.Filled.Notifications else Icons.Filled.Schedule, null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.outline)
                }
            },
            modifier = Modifier.alpha(if (r.away) 0.5f else 1f).clickable(onClick = go),
        )
        "bot" -> if (!mood) ListItem(
            headlineContent = { Text(r.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            supportingContent = { if (line(r).isNotEmpty()) Text(line(r), maxLines = if (r.state == "stalled") 2 else 1, overflow = TextOverflow.Ellipsis) },
            leadingContent = { StatusDot("", planeStatus(r.state)) },
            trailingContent = { Text(r.ago, style = MaterialTheme.typography.labelMedium) },
            modifier = Modifier.alpha(if (r.away) 0.5f else 1f).clickable(onClick = go),
        ) else ListItem(
            headlineContent = { Text(r.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            supportingContent = { if (line(r).isNotEmpty()) Text(line(r), maxLines = if (r.state == "stalled") 2 else 1, overflow = TextOverflow.Ellipsis) },
            leadingContent = { Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) { MoodDot(r.state) } },
            trailingContent = { if (r.ago.isNotEmpty()) Text(r.ago, style = MaterialTheme.typography.labelMedium) },
            modifier = Modifier.alpha(if (r.away) 0.5f else 1f).clickable(onClick = go),
        )
        else -> ListItem(
            headlineContent = { Text(r.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            supportingContent = { if (line(r).isNotEmpty()) Text(line(r), maxLines = if (r.state == "stalled") 2 else 1, overflow = TextOverflow.Ellipsis) },
            leadingContent = { StatusDot("", planeStatus(r.state)) },
            trailingContent = { Text(r.ago, style = MaterialTheme.typography.labelMedium) },
            modifier = Modifier.alpha(if (r.away) 0.5f else 1f).clickable(onClick = go),
        )
    }
}

private fun LazyListScope.planeSection(m: AppModel, key: String, name: String, rows: List<PlaneRow>, mood: Boolean = false) {
    if (rows.isEmpty()) return
    item(key = "pl:$key") { PlaneLabel(name, rows.size) }
    items(rows, key = { "pl:$key:${it.kind}:${it.id}" }) { PlaneItem(m, it, mood) }
}

// the plane as list items. On the home list (full false) it is what waits
// and what runs, then the bots, rooms and routines; on the Status tab
// (full true) the idle threads follow too, behind their fold.
fun LazyListScope.planeSections(m: AppModel, p: PlaneModel, full: Boolean, lang: String, idleOpen: Boolean = true, onIdle: () -> Unit = {}) {
    planeSection(m, "need", NativeMessages.plane_needs(lang), p.need)
    planeSection(m, "run", NativeMessages.plane_running(lang), p.run)
    if (full && p.idleN > 0) {
        item(key = "pl:idle") {
            Row(Modifier.fillMaxWidth().clickable(onClick = onIdle), verticalAlignment = Alignment.CenterVertically) {
                PlaneLabel(NativeMessages.web_plane_idle(lang), p.idleN, Modifier.weight(1f))
                Icon(if (idleOpen) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore, null, Modifier.padding(end = 16.dp).size(18.dp),
                    tint = MaterialTheme.colorScheme.outline)
            }
        }
        if (idleOpen) {
            items(p.idle, key = { "pl:idle:${it.kind}:${it.id}" }) { PlaneItem(m, it) }
            if (p.idleN > p.idle.size) item(key = "pl:idle:more") {
                Text(NativeMessages.native_plane_more(lang, (p.idleN - p.idle.size).toString()), Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
            }
        }
    }
    // the home list has its own Bots section (the cats)
    if (full) planeSection(m, "bots", NativeMessages.tour_title_bots(lang), p.bots, true)
    planeSection(m, "rooms", NativeMessages.bots_rooms(lang), p.rooms)
    planeSection(m, "locks", NativeMessages.plane_locks(lang), p.locks)
    planeSection(m, "routines", NativeMessages.bots_tab_routines(lang), p.routines)
    if (full && p.need.isEmpty() && p.run.isEmpty() && p.idleN == 0 && p.bots.isEmpty() && p.rooms.isEmpty() && p.routines.isEmpty()) {
        item(key = "pl:none") {
            Text(NativeMessages.sidebar_nothing(lang), Modifier.padding(16.dp), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.outline)
        }
    }
}

// The Status tab
// --------------

@Composable
private fun Counter(n: Int, label: String, tint: Color, modifier: Modifier) {
    Column(modifier) {
        Text("$n", style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.SemiBold),
            color = if (n > 0) tint else MaterialTheme.colorScheme.outline)
        Text(label.uppercase(), style = MaterialTheme.typography.labelSmall.copy(fontSize = 10.sp), color = MaterialTheme.colorScheme.outline, maxLines = 1)
    }
}

@Composable
private fun PlaneStrip(m: AppModel, p: PlaneModel) {
    val lang = LocalBackplaneLanguage.current
    Column(Modifier.fillMaxWidth().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Counter(p.need.size, NativeMessages.plane_need(lang), PhaseColor.bad, Modifier.weight(1f))
            Counter(p.run.size, NativeMessages.plane_running(lang), PhaseColor.accent, Modifier.weight(1f))
            Counter(p.working, NativeMessages.plane_bots_working(lang), PhaseColor.accent, Modifier.weight(1f))
            Counter(p.rooms.size, NativeMessages.native_plane_unread(lang), PhaseColor.input, Modifier.weight(1f))
        }
        if (p.machines.size > 1) {
            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                for (mc in p.machines) {
                    val tint = when (mc.state) { "here" -> PhaseColor.ok; "syncing" -> PhaseColor.accent; else -> PhaseColor.faint }
                    Row(Modifier.alpha(if (mc.state == "away") 0.55f else 1f)
                        .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(2.dp))
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                        Box(Modifier.size(6.dp).clip(CircleShape).background(tint))
                        Text(mc.name, style = MaterialTheme.typography.labelMedium)
                    }
                }
            }
        }
        p.home?.let { h ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(h.line, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (h.button) OutlinedButton(onClick = { m.act(h.act, h.value) }) { Text(h.label) }
            }
        }
    }
    HorizontalDivider()
}

@Composable
fun StatusTab(m: AppModel, p: PlaneModel?) {
    val lang = LocalBackplaneLanguage.current
    var idleOpen by remember { mutableStateOf(true) }
    if (p == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        return
    }
    LazyColumn(Modifier.fillMaxSize()) {
        item(key = "pl:strip") { PlaneStrip(m, p) }
        planeSections(m, p, true, lang, idleOpen) { idleOpen = !idleOpen }
    }
}

// The Home tab: the uberbot's card, then the plane in full (what needs you,
// what runs, the bots, rooms and routines, the idle threads). With no
// uberbot there is no plane: the tab offers to make one.
@Composable
fun HomeTab(m: AppModel, s: Screen, state: androidx.compose.foundation.lazy.LazyListState) {
    val lang = LocalBackplaneLanguage.current
    var idleOpen by remember { mutableStateOf(true) }
    val card = m.uber(s)
    val plane = m.plane(s)
    if (card == null) {
        Column(Modifier.fillMaxSize().padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            if (s.projects.isEmpty() && s.bots.isEmpty()) CircularProgressIndicator()
            else {
                Text(NativeMessages.native_home_none(lang), style = MaterialTheme.typography.titleMedium)
                Text(NativeMessages.native_home_hint(lang),
                    Modifier.padding(vertical = 12.dp), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.outline)
                androidx.compose.material3.Button(onClick = { m.act("uber-new") }) { Text(NativeMessages.native_home_make(lang)) }
            }
        }
        return
    }
    LazyColumn(Modifier.fillMaxSize(), state = state) {
        item(key = "uber") { UberCardRow(m, card) }
        if (plane == null) item(key = "pl:wait") { Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator() } }
        else {
            item(key = "pl:strip") { PlaneStrip(m, plane) }
            planeSections(m, plane, true, lang, idleOpen) { idleOpen = !idleOpen }
        }
    }
}
