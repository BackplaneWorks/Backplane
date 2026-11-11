package dev.backplane.mobile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

// Swarms (feature `swarm`, docs/SWARM.md "Client data"): a permanent room whose members are threads, peers with no lead. The project list shows each
// live swarm once (its member threads are left out); its screen is the board as one feed, the members' chips, the state with Pause/Resume and Stop.

data class DSwarm(val id: String, val name: String, val project: String, val goal: String, val state: String, val reason: String, val members: Int, val last: Long)
data class DSwarmRoom(val id: String, val name: String, val project: String, val goal: String, val state: String, val reason: String, val by: String)
data class DSwarmMember(val member: String, val thread: String, val name: String, val color: String, val provider: String, val model: String, val role: String, val state: String, val title: String)
data class DSwarmPost(val id: String, val from: String, val thread: String, val color: String, val text: String, val tags: List<String>, val reply: String, val mentions: List<String>,
    val files: List<String>, val guard: Boolean, val held: String, val at: Long)
data class DSwarmClaim(val post: String, val thread: String, val files: List<String>, val state: String)
data class DSwarmBoard(val room: DSwarmRoom, val members: List<DSwarmMember>, val posts: List<DSwarmPost>, val claims: List<DSwarmClaim>)
data class DModel(val value: String, val label: String, val ok: Boolean, val why: String)
/** One member row of New swarm and Add member: what `swarm.create` and `swarm.act add` send. */
data class DSwarmPick(val provider: String, val model: String = "", val role: String = "") {
    fun json(): JSONObject = JSONObject().put("provider", provider).also { if (model.isNotEmpty()) it.put("model", model); if (role.isNotBlank()) it.put("role", role.trim()) }
}

private fun JSONObject.s(k: String): String = if (isNull(k)) "" else optString(k, "")
private fun JSONArray?.strings(): List<String> = if (this == null) emptyList() else (0 until length()).map { optString(it) }.filter { it.isNotEmpty() }
/** the hub writes a room post's time in seconds; the phone's clock reads ms */
internal fun swarmMs(t: Long) = if (t in 1 until 100_000_000_000L) t * 1000 else t
private fun <T> JSONArray?.objs(f: (JSONObject) -> T): List<T> = if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it) }.map(f)

object SwarmParse {
    fun swarm(o: JSONObject) = DSwarm(o.s("id"), o.s("name"), o.s("project"), o.s("goal"), o.s("state"), o.s("reason"), o.optInt("members", 0), swarmMs(o.optLong("last", 0)))
    fun list(o: JSONObject) = o.optJSONArray("swarms").objs(::swarm)
    fun board(o: JSONObject): DSwarmBoard {
        val r = o.optJSONObject("room") ?: JSONObject()
        return DSwarmBoard(DSwarmRoom(r.s("id"), r.s("name"), r.s("project"), r.s("goal"), r.s("state"), r.s("reason"), r.s("by")),
            o.optJSONArray("members").objs { DSwarmMember(it.s("member"), it.s("thread"), it.s("name"), it.s("color"), it.s("provider"), it.s("model"), it.s("role"), it.s("state"), it.s("title")) },
            o.optJSONArray("posts").objs { DSwarmPost(it.s("id"), it.s("from"), it.s("thread"), it.s("color"), it.s("text"), it.optJSONArray("tags").strings(), it.s("reply"), it.optJSONArray("mentions").strings(),
                it.optJSONArray("files").strings(), it.optBoolean("guard"), if (it.opt("held") is String) it.s("held") else if (it.optBoolean("held")) "held" else "", swarmMs(it.optLong("at", 0))) },
            o.optJSONArray("claims").objs { DSwarmClaim(it.s("post"), it.s("thread"), it.optJSONArray("files").strings(), it.s("state")) })
    }
    /** `models.list`: provider -> its models, in the hub's order. */
    fun models(o: JSONObject): List<Pair<String, List<DModel>>> {
        val m = o.optJSONObject("models") ?: return emptyList()
        return m.keys().asSequence().toList().mapNotNull { k -> m.optJSONArray(k)?.let { a -> k to a.objs { DModel(it.s("value"), it.s("label").ifEmpty { it.s("value") }, it.optBoolean("ok", true), it.s("why")) } } }
    }
}

/** What the swarm UI decides, pure (tested in SwarmUiTest). */
object SwarmUi {
    const val MIN = 2; const val MAX = 8
    /** the swarms a project's list shows: active or paused, as the hub ordered them (newest first) */
    fun live(all: List<DSwarm>, project: String) = all.filter { it.project == project && (it.state == "active" || it.state == "paused") }
    fun ended(state: String) = state == "stopped" || state == "done"
    /** the state line's buttons: the swarm.act action of each */
    fun controls(state: String): List<String> = when (state) { "active" -> listOf("pause", "stop"); "paused" -> listOf("resume", "stop"); else -> emptyList() }
    /** a re-read with `since` appended to what is shown: newer posts after, one per id (a post read again replaces its copy) */
    fun merge(old: DSwarmBoard?, fresh: DSwarmBoard, since: String): DSwarmBoard {
        if (old == null || since.isEmpty()) return fresh
        val ids = fresh.posts.map { it.id }.toSet()
        return fresh.copy(posts = old.posts.filter { it.id !in ids } + fresh.posts)
    }
    /** the id to read after: the newest post shown, never a guard's local one */
    fun since(b: DSwarmBoard?) = b?.posts?.lastOrNull()?.id ?: ""
    /** a claim post's mark: "waiting" while a peer holds a file it names, "held" while it is open, "" once a done answered it */
    fun claim(p: DSwarmPost, claims: List<DSwarmClaim>) = if ("claim" !in p.tags) "" else claims.firstOrNull { it.post == p.id }?.state ?: ""
    fun synthesis(p: DSwarmPost) = "synthesis" in p.tags
    /** a held post is dimmed, with why; a guard's own post is held by nature (it wakes nobody) and stays plain grey */
    fun dimmed(p: DSwarmPost) = p.held.isNotEmpty() && !p.guard
    /** the tags drawn as labels: synthesis is drawn as the box it sits in, not as a label */
    fun labels(p: DSwarmPost) = if (p.guard) emptyList() else p.tags.filter { it != "synthesis" }
    fun canCreate(topic: String, members: List<DSwarmPick>) = topic.isNotBlank() && members.size in MIN..MAX && members.all { it.provider.isNotEmpty() }
    /** a member's chip opens its thread; a removed member, or a bot, has none to open */
    fun opens(m: DSwarmMember) = m.thread.isNotEmpty() && m.state != "removed"
    /** a new member row: the first provider whose model can run here */
    fun firstPick(models: List<Pair<String, List<DModel>>>): DSwarmPick = DSwarmPick(models.firstOrNull { (_, l) -> l.any { it.ok } }?.first ?: models.firstOrNull()?.first ?: "claude")
}

/** The swarms this hub has (swarm.list), read again on a `swarm` push; and each push's room counted, so an open board re-reads. */
object Swarms {
    var all by mutableStateOf<List<DSwarm>>(emptyList()); private set
    var ticks by mutableStateOf<Map<String, Int>>(emptyMap()); private set
    private var due = false
    fun on() = DataClient.has("swarm")
    fun reset() { all = emptyList(); ticks = emptyMap() }
    suspend fun load() { if (on()) runCatching { DataClient.call("swarm.list", JSONObject()) }.onSuccess { all = SwarmParse.list(it) } }
    fun pushed(room: String) {
        ticks = ticks + (room to (ticks[room] ?: 0) + 1)
        if (due) return; due = true
        DataClient.scope.launch { kotlinx.coroutines.delay(400); due = false; load() }
    }
    fun isSwarm(room: String) = all.any { it.id == room }
    suspend fun get(room: String, since: String): DSwarmBoard = SwarmParse.board(DataClient.call("swarm.get", JSONObject().put("room", room).also { if (since.isNotEmpty()) it.put("since", since) }))
    suspend fun post(room: String, text: String) { DataClient.call("swarm.post", JSONObject().put("room", room).put("text", text).put("clientId", UUID.randomUUID().toString())) }
    suspend fun act(room: String, action: String, thread: String = "", pick: DSwarmPick? = null) {
        val p = JSONObject().put("room", room).put("action", action)
        if (thread.isNotEmpty()) p.put("thread", thread)
        pick?.json()?.let { j -> j.keys().forEach { k -> p.put(k, j.get(k)) } }
        DataClient.call("swarm.act", p); load()
    }
    suspend fun create(project: String, topic: String, goal: String, seed: String, members: List<DSwarmPick>): String {
        val p = JSONObject().put("project", project).put("topic", topic.trim()).put("members", JSONArray(members.map { it.json() }))
        if (goal.isNotBlank()) p.put("goal", goal.trim()); if (seed.isNotBlank()) p.put("seed", seed.trim())
        val r = DataClient.call("swarm.create", p); load(); return r.optString("room")
    }
    suspend fun models() = SwarmParse.models(DataClient.call("models.list"))
}

private fun hex(s: String, or: Color): Color = runCatching { Color(android.graphics.Color.parseColor(s)) }.getOrDefault(or)
internal fun swarmState(state: String) = when (state) { "active" -> Msg.android_swarm_state_active; "paused" -> Msg.android_swarm_state_paused; "stopped" -> Msg.android_swarm_state_stopped; "done" -> Msg.android_swarm_state_done; else -> state }
private fun swarmReason(r: String) = when (r) { "circular" -> Msg.android_swarm_reason_circular; "idle" -> Msg.android_swarm_reason_idle; "budget" -> Msg.android_swarm_reason_budget; "person" -> Msg.android_swarm_reason_person; "chatter" -> Msg.android_swarm_reason_chatter; else -> r }
private fun swarmTag(t: String) = when (t) { "idea" -> Msg.android_swarm_tag_idea; "critique" -> Msg.android_swarm_tag_critique; "claim" -> Msg.android_swarm_tag_claim; "done" -> Msg.android_swarm_tag_done; "question" -> Msg.android_swarm_tag_question; "synthesis" -> Msg.android_swarm_tag_synthesis; else -> t }
private fun swarmAct(a: String) = when (a) { "pause" -> Msg.android_swarm_pause; "resume" -> Msg.android_swarm_resume; else -> Msg.android_swarm_stop }
@Composable private fun memberDot(state: String) = when (state) { "working" -> dok; "waiting" -> dwarn; "failed" -> dcs.error; else -> dfaint }
@Composable private fun stateDot(state: String) = when (state) { "active" -> dok; "paused" -> dwarn; else -> dfaint }

/** A project's swarm in its list: one row, its name, state and member count. */
@Composable
fun SwarmRow(s: DSwarm, open: () -> Unit) {
    Row(Modifier.fillMaxWidth().height(40.dp).clickable(onClick = open).padding(start = 34.dp, end = 12.dp).testTag("swarm.row.${s.id}"), verticalAlignment = Alignment.CenterVertically) {
        DDot(stateDot(s.state), 7); Spacer(Modifier.width(10.dp))
        Text(s.name, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp, maxLines = 1)
        DMono(swarmState(s.state) + " · " + Msg.android_swarm_members(s.members), dfaint, 11)
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SwarmScreen(room: String, back: () -> Unit, openThread: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var board by remember(room) { mutableStateOf<DSwarmBoard?>(null) }
    var err by remember { mutableStateOf("") }
    var gone by remember { mutableStateOf(false) }
    var draft by remember { mutableStateOf("") }
    var stopping by remember { mutableStateOf(false) }
    var adding by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf<DSwarmMember?>(null) }
    suspend fun read(full: Boolean) {
        val since = if (full) "" else SwarmUi.since(board)
        try { board = SwarmUi.merge(board, Swarms.get(room, since), since); err = "" }
        catch (e: DataClient.Refused) { if (board == null) gone = true else err = e.message ?: "" }
        catch (e: Exception) { err = e.message ?: "" }
    }
    // the first read whole, again whole on each reconnect; a `swarm` push for this room reads what is newer than the newest post shown
    LaunchedEffect(room, DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE) read(true) }
    val tick = Swarms.ticks[room] ?: 0
    LaunchedEffect(room, tick) { if (tick > 0 && board != null) read(false) }
    val b = board
    val name = b?.room?.name ?: Swarms.all.firstOrNull { it.id == room }?.name ?: ""
    Column(Modifier.fillMaxSize().testTag("swarm.screen")) {
        DBar(name, back) {
            if (b != null && !SwarmUi.ended(b.room.state) && b.members.count { it.state != "removed" } < SwarmUi.MAX)
                IconButton(onClick = { adding = true }, modifier = Modifier.testTag("swarm.add")) { Icon(Icons.Default.PersonAdd, Msg.android_swarm_member_add, tint = dcs.onSurfaceVariant) }
        }
        if (gone) { DMono(Msg.android_swarm_gone, dfaint, 12, Modifier.padding(14.dp)); return@Column }
        if (b == null) { DMono(Msg.viewer_loading, dfaint, 12, Modifier.padding(14.dp)); return@Column }
        // the members: colour, name, state; a tap opens the member's thread, a long press offers Remove
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 8.dp).testTag("swarm.members"), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            b.members.forEach { m ->
                val on = SwarmUi.opens(m)
                Row(Modifier.border(BorderStroke(1.dp, dcs.outlineVariant)).alpha(if (m.state == "removed") .45f else 1f)
                    .combinedClickable(enabled = on, onClick = { openThread(m.thread) }, onLongClick = { if (!SwarmUi.ended(b.room.state)) removing = m })
                    .height(32.dp).padding(horizontal = 8.dp).testTag("swarm.member.${m.name}"), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(width = 3.dp, height = 16.dp).background(hex(m.color, dcs.primary))); Spacer(Modifier.width(6.dp))
                    DMono(m.name, dcs.onSurface, 12); Spacer(Modifier.width(6.dp)); DDot(memberDot(m.state), 6)
                }
            }
        }
        // the goal and the state, with what the person can do to it
        Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp).padding(bottom = 8.dp)) {
            if (b.room.goal.isNotEmpty()) Text(b.room.goal, color = dcs.onSurface, fontSize = 13.sp, modifier = Modifier.testTag("swarm.goal"))
            Row(Modifier.fillMaxWidth().padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                DDot(stateDot(b.room.state), 7); Spacer(Modifier.width(8.dp))
                DMono(swarmState(b.room.state) + (if (b.room.state == "paused" && b.room.reason.isNotEmpty()) " · " + swarmReason(b.room.reason) else ""), dcs.onSurfaceVariant, 12, Modifier.weight(1f).testTag("swarm.state"))
                SwarmUi.controls(b.room.state).forEach { a ->
                    Spacer(Modifier.width(6.dp))
                    DBtn(swarmAct(a), false, true, Modifier.testTag("swarm.act.$a"), danger = a == "stop") {
                        if (a == "stop") stopping = true else scope.launch { try { Swarms.act(room, a); read(false) } catch (e: Exception) { err = e.message ?: "" } }
                    }
                }
            }
            if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 11, Modifier.padding(top = 4.dp), lines = 3)
        }
        DLine()
        // the board, one feed, oldest first
        val list = rememberLazyListState()
        LaunchedEffect(b.posts.size) { if (b.posts.isNotEmpty()) list.scrollToItem(b.posts.size - 1) }
        LazyColumn(Modifier.weight(1f).fillMaxWidth().testTag("swarm.board"), state = list, contentPadding = PaddingValues(vertical = 6.dp)) {
            if (b.posts.isEmpty()) item("none") { DMono(Msg.android_swarm_none, dfaint, 12, Modifier.padding(14.dp)) }
            items(b.posts, key = { it.id }) { p -> SwarmPost(p, b.claims) }
        }
        if (!SwarmUi.ended(b.room.state)) {
            DLine()
            Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.Bottom) {
                Box(Modifier.weight(1f)) { DField(draft, { draft = it }, Msg.android_swarm_post_hint, Modifier.testTag("swarm.composer"), 4, dcs.primary) }
                Spacer(Modifier.width(8.dp))
                DBtn(Msg.android_post, true, draft.isNotBlank(), Modifier.testTag("swarm.post")) {
                    val t = draft.trim(); draft = ""
                    scope.launch { try { Swarms.post(room, t); read(false) } catch (e: Exception) { draft = t; err = e.message ?: "" } }
                }
            }
        }
    }
    if (stopping) ActionSheet(Msg.android_swarm_stop_ask(name), listOf(Msg.android_swarm_stop_yes to { scope.launch { try { Swarms.act(room, "stop"); read(false) } catch (e: Exception) { err = e.message ?: "" } }; Unit }, Msg.android_swarm_keep to {})) { stopping = false }
    removing?.let { m -> ActionSheet(m.name, listOf(Msg.android_swarm_remove_member(m.name) to { scope.launch { try { Swarms.act(room, "remove", m.thread); read(false) } catch (e: Exception) { err = e.message ?: "" } }; Unit })) { removing = null } }
    if (adding) AddMemberDialog({ adding = false }) { pick -> adding = false; scope.launch { try { Swarms.act(room, "add", pick = pick); read(false) } catch (e: Exception) { err = e.message ?: "" } } }
}

/** One post: its member's colour, its tags, a claim's files (a waiting one marked), a guard's post grey, a held one dimmed, a synthesis in a box of its own. */
@Composable
private fun SwarmPost(p: DSwarmPost, claims: List<DSwarmClaim>) {
    val syn = SwarmUi.synthesis(p)
    val bar = if (p.guard) Color.Transparent else hex(p.color, dcs.primary)
    val outer = if (syn) Modifier.padding(horizontal = 10.dp, vertical = 10.dp).border(BorderStroke(1.dp, dcs.primary)).background(dcs.surfaceContainerHigh).padding(10.dp) else Modifier.padding(horizontal = 12.dp, vertical = 5.dp)
    Column(Modifier.fillMaxWidth().then(outer).alpha(if (SwarmUi.dimmed(p)) .5f else 1f).testTag(if (syn) "swarm.synthesis" else if (p.guard) "swarm.guard" else "swarm.post.${p.id}")) {
        if (syn) DMono(Msg.android_swarm_tag_synthesis.uppercase(), dcs.primary, 11, Modifier.padding(bottom = 6.dp), bold = true)
        Row(Modifier.height(IntrinsicSize.Min)) {
            if (!syn) { Box(Modifier.width(3.dp).fillMaxHeight().background(bar)); Spacer(Modifier.width(8.dp)) }
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    DMono(p.from.ifEmpty { Msg.android_you }, if (p.guard) dfaint else bar, 11, bold = !p.guard)
                    SwarmUi.labels(p).forEach { t -> Spacer(Modifier.width(6.dp)); DMono(swarmTag(t).uppercase(), dcs.onSurfaceVariant, 9, Modifier.border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 4.dp, vertical = 1.dp)) }
                    Spacer(Modifier.weight(1f)); DMono(dAgo(p.at), dfaint, 10)
                }
                if (p.guard) Text(p.text, color = dfaint, fontFamily = BpMono, fontSize = 12.sp)
                else LinkText(p.text, p.thread, color = dcs.onSurface, size = 13)
                if (p.files.isNotEmpty()) {
                    val mark = SwarmUi.claim(p, claims)
                    Column(Modifier.padding(top = 4.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        p.files.forEach { f ->
                            Row(Modifier.border(BorderStroke(1.dp, if (mark == "waiting") dwarn else dcs.outlineVariant)).padding(horizontal = 6.dp, vertical = 2.dp).testTag("swarm.file"), verticalAlignment = Alignment.CenterVertically) {
                                DMono(f, if (mark.isEmpty()) dfaint else dcs.onSurface, 11)
                                if (mark == "waiting") { Spacer(Modifier.width(6.dp)); DMono(Msg.android_swarm_waiting, dwarn, 10, Modifier.testTag("swarm.waiting"), bold = true) }
                            }
                        }
                    }
                }
                if (SwarmUi.dimmed(p)) DMono(Msg.android_swarm_held(Tr.t(p.held)), dfaint, 10, Modifier.padding(top = 2.dp))
            }
        }
    }
}

/** A provider and a model from `models.list`, and a role: one member of a swarm. */
@Composable
private fun MemberPickRow(n: Int, pick: DSwarmPick, models: List<Pair<String, List<DModel>>>, change: (DSwarmPick) -> Unit, remove: (() -> Unit)?) {
    var prov by remember { mutableStateOf(false) }
    var mod by remember { mutableStateOf(false) }
    val choices = models.firstOrNull { it.first == pick.provider }?.second ?: emptyList()
    Column(Modifier.fillMaxWidth().border(BorderStroke(1.dp, dcs.outlineVariant)).padding(8.dp).testTag("swarm.new.member.$n"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            DMono(Msg.android_swarm_member_n(n), dfaint, 11, Modifier.weight(1f))
            if (remove != null) Box(Modifier.size(28.dp).clickable(onClick = remove).testTag("swarm.new.remove.$n"), contentAlignment = Alignment.Center) { Icon(Icons.Default.Close, Msg.common_remove, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Box(Modifier.weight(1f)) {
                ModelChip(pick.provider.ifEmpty { Msg.android_swarm_provider }, "swarm.new.provider.$n", Modifier.fillMaxWidth()) { prov = true }
                DropdownMenu(prov, { prov = false }) {
                    models.forEach { (p, l) -> DropdownMenuItem({ Text(p, fontFamily = BpMono, color = if (l.isEmpty() || l.any { it.ok }) dcs.onSurface else dfaint) }, { prov = false; change(pick.copy(provider = p, model = "")) }, Modifier.testTag("swarm.pick.provider.$p")) }
                }
            }
            Box(Modifier.weight(1f)) {
                ModelChip(choices.firstOrNull { it.value == pick.model }?.value ?: Msg.common_default, "swarm.new.model.$n", Modifier.fillMaxWidth()) { mod = true }
                DropdownMenu(mod, { mod = false }) {
                    DropdownMenuItem({ Text(Msg.common_default, fontFamily = BpMono) }, { mod = false; change(pick.copy(model = "")) }, Modifier.testTag("swarm.pick.model.default"))
                    choices.forEach { c -> DropdownMenuItem({ Column { Text(Tr.t(c.label), fontFamily = BpMono, color = if (c.ok) dcs.onSurface else dfaint); if (!c.ok && c.why.isNotEmpty()) DMono(Tr.t(c.why), dfaint, 10) } },
                        { mod = false; change(pick.copy(model = c.value)) }, Modifier.testTag("swarm.pick.model.${c.value}"), enabled = c.ok) }
                }
            }
        }
        DField(pick.role, { change(pick.copy(role = it)) }, Msg.android_swarm_role, Modifier.testTag("swarm.new.role.$n"))
    }
}

private fun loadModels(scope: kotlinx.coroutines.CoroutineScope, into: (List<Pair<String, List<DModel>>>) -> Unit) { scope.launch { runCatching { Swarms.models() }.onSuccess(into) } }

/** New swarm, from a project: topic, goal, seed and 2 to 8 members; Start sends `swarm.create` and opens the swarm. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NewSwarmSheet(project: String, close: () -> Unit, created: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var topic by remember { mutableStateOf("") }
    var goal by remember { mutableStateOf("") }
    var seed by remember { mutableStateOf("") }
    var models by remember { mutableStateOf<List<Pair<String, List<DModel>>>>(emptyList()) }
    var members by remember { mutableStateOf<List<DSwarmPick>>(emptyList()) }
    var busy by remember { mutableStateOf(false) }
    var err by remember { mutableStateOf("") }
    LaunchedEffect(Unit) { loadModels(scope) { models = it; if (members.isEmpty()) members = List(SwarmUi.MIN) { _ -> SwarmUi.firstPick(it) } } }
    val title = DataClient.projects.firstOrNull { it.id == project }?.title ?: ""
    ModalBottomSheet(onDismissRequest = close, shape = RectangleShape, containerColor = dcs.surfaceVariant, dragHandle = null, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().fillMaxHeight(.92f).testTag("swarm.new")) {
            Row(Modifier.fillMaxWidth().height(48.dp).padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                DMono(Msg.android_swarm_new + (if (title.isNotEmpty()) " · $title" else ""), dcs.onSurface, 13, Modifier.weight(1f), bold = true)
                DBtn(if (busy) Msg.state_starting else Msg.android_start, true, !busy && SwarmUi.canCreate(topic, members), Modifier.testTag("swarm.new.start")) {
                    busy = true; err = ""
                    scope.launch { try { val r = Swarms.create(project, topic, goal, seed, members); close(); if (r.isNotEmpty()) created(r) } catch (e: Exception) { err = e.message ?: ""; busy = false } }
                }
            }
            DLine()
            Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState()).padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 12, Modifier.testTag("swarm.new.error"), lines = 4)
                DField(topic, { topic = it }, Msg.android_swarm_topic, Modifier.testTag("swarm.new.topic"))
                DField(goal, { goal = it }, Msg.android_swarm_goal, Modifier.testTag("swarm.new.goal"), 3)
                DField(seed, { seed = it }, Msg.android_swarm_seed, Modifier.heightIn(min = 90.dp).testTag("swarm.new.seed"), 8)
                members.forEachIndexed { i, m ->
                    MemberPickRow(i + 1, m, models, { n -> members = members.toMutableList().also { it[i] = n } }, if (members.size > SwarmUi.MIN) ({ members = members.filterIndexed { j, _ -> j != i } }) else null)
                }
                if (members.size < SwarmUi.MAX) DBtn(Msg.android_swarm_member_add, false, models.isNotEmpty(), Modifier.testTag("swarm.new.add")) { members = members + (members.lastOrNull()?.copy(role = "") ?: SwarmUi.firstPick(models)) }
            }
        }
    }
}

/** Add member, on the swarm screen: one member row and Add. */
@Composable
private fun AddMemberDialog(close: () -> Unit, add: (DSwarmPick) -> Unit) {
    val scope = rememberCoroutineScope()
    var models by remember { mutableStateOf<List<Pair<String, List<DModel>>>>(emptyList()) }
    var pick by remember { mutableStateOf<DSwarmPick?>(null) }
    LaunchedEffect(Unit) { loadModels(scope) { models = it; if (pick == null) pick = SwarmUi.firstPick(it) } }
    Dialog(onDismissRequest = close) {
        Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(12.dp).testTag("swarm.addsheet"), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            DMono(Msg.android_swarm_member_add, dcs.onSurfaceVariant, 11)
            pick?.let { p -> MemberPickRow(1, p, models, { pick = it }, null) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                DBtn(Msg.android_swarm_member_add, true, pick != null, Modifier.testTag("swarm.add.ok")) { pick?.let(add) }
                DBtn(Msg.common_cancel, false, onClick = close)
            }
        }
    }
}
