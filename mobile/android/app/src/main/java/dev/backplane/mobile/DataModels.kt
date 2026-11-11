package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject

// docs/PHONE-DATA.md (data/1): the plain data the hub sends, as typed values. Parsing is forgiving: a missing field is empty, an extra one ignored.
private fun JSONObject.str(k: String): String = if (isNull(k)) "" else optString(k, "")
private fun JSONObject.lng(k: String): Long = optLong(k, 0L)
private fun <T> JSONArray?.list(f: (JSONObject) -> T): List<T> = if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it)?.let(f) }

data class DOption(val id: String, val label: String)
data class DTool(val name: String, val summary: String, val state: String, val sub: String = "", val result: String = "") // sub: an Agent call's subagent (feature subview); result: a subagent's tool result
data class DAsk(val id: String, val state: String, val question: String, val options: List<DOption>, val answered: String?) { val open get() = state == "open" }
data class DAttachment(val id: String, val name: String, val mime: String, val bytes: Long)
data class DEntry(
    val id: String, val kind: String, val text: String, val at: Long, val clientId: String? = null, val replaces: String? = null,
    val live: Boolean = false, val tool: DTool? = null, val ask: DAsk? = null, val attachments: List<DAttachment> = emptyList(),
    val sending: Boolean = false, val failed: Boolean = false,
    val localImages: List<ByteArray> = emptyList(), // phone-only: the pictures of a message this phone is sending
    val label: String = "", // a design step: what changed; a note: its name
    val agentOnly: Boolean = false, // a message only the agent reads (data/1 `agentOnly`): never shown
    val tone: String = "",  // a note: error, info…
    val term: DTermRef? = null, // a chat terminal (feature `terms`, kind "term"): its card
    val handoff: DHandoff? = null, // a `/give` received (kind "handoff"): its card
    val replyTo: DReplyTo? = null, // the person's reply to a message (data/1 `replyTo`): the chip over the bubble
)
/** What a reply answers (docs/PHONE-DATA.md, Entry): the message's id and its first line. */
data class DReplyTo(val id: String, val quote: String)
/** A `/give` received: where it came from, the person's note, the summary and what the work touched; a reference opens by `link.resolve {url, thread}`. */
data class DHandoff(val from: String, val title: String, val note: String, val summary: String, val refs: List<DRef>)
data class DRef(val kind: String, val label: String, val url: String, val thread: String)
fun parseHandoff(o: JSONObject) = DHandoff(o.str("from"), o.str("title"), o.str("note"), o.str("summary"), o.optJSONArray("refs").list { DRef(it.str("kind"), it.str("label"), it.str("url"), it.str("thread")) })
fun handoffJson(h: DHandoff): JSONObject = JSONObject().put("from", h.from).put("title", h.title).put("note", h.note).put("summary", h.summary)
    .put("refs", JSONArray(h.refs.map { JSONObject().put("kind", it.kind).put("label", it.label).put("url", it.url).put("thread", it.thread) }))
/** The `term` of a kind:"term" entry: the key its screen is read and watched under, and what it ran. */
data class DTermRef(val key: String, val id: String, val title: String, val cols: Int, val rows: Int, val cmd: String)
/** Where a project or thread of another machine runs (docs/PHONE-DATA.md, Your machines): `short` on a row, `name` in the header, `label` "on desktop" after a project's name, `runs` "runs on desktop" over the composer; `away` draws it dimmer. */
data class DMark(val name: String, val short: String, val away: Boolean, val label: String, val runs: String)
/** A choice of where a new thread starts: `value` is the project id there (this machine's, or `<link>~<id>`). */
data class DNewOn(val value: String, val label: String, val away: Boolean)
/** A chip of the machine switcher over the lists: "" every machine, "@here" this one, else a linked machine's name. */
data class DChip(val value: String, val label: String, val on: Boolean, val away: Boolean, val hint: String)
fun parseMark(o: JSONObject?) = o?.takeIf { it.optString("name").isNotEmpty() }?.let { DMark(it.str("name"), it.str("short"), it.optBoolean("away"), it.str("label"), it.str("runs")) }
fun parseChips(a: JSONArray?) = a.list { DChip(it.str("value"), it.str("label"), it.optBoolean("on"), it.optBoolean("away"), it.str("hint")) }
fun markJson(m: DMark?): Any = if (m == null) JSONObject.NULL else JSONObject().put("name", m.name).put("short", m.short).put("away", m.away).put("label", m.label).put("runs", m.runs)
data class DProject(val id: String, val title: String, val root: String, val threadCount: Int, val updatedAt: Long, val waiting: Int, val icon: String = "folder", val css: String = "", val settledCount: Int? = null,
    val mark: DMark? = null, val also: String = "", val newOn: List<DNewOn> = emptyList(), val runsOn: String = "", val key: String = "") // runsOn: "runs on desktop", where a new thread starts (the first of newOn) // settledCount: the hub's count (feature `settled`), null from a hub before it
data class DThread(
    val id: String, val project: String, val title: String, val state: String, val updatedAt: Long, val pinned: Boolean, val settled: Boolean,
    val archived: Boolean, val provider: String, val model: String, val preview: String, val waiting: Int,
    val parent: String = "", val effort: String = "", val runtime: String = "", val chip: String = "", val settledAt: Long = 0, // chip: the composer's "claude / opus / high", in the hub's words
    val history: Boolean = false, // an old Scratch chat (feature history): listed under Conversation history, never under a project
    val mark: DMark? = null, // the machine it runs on when it is another one; null for this hub's own
    val pinOrder: Int = 0, // where a pinned thread stands on its project's Pinned shelf (the hub's order: as pinned, or as dragged)
    val status: String = "", // the sidebar dot's word (feature status): active, waiting, blocked, failed, stalled, finished (the desktop's green dot: done, not yet settled), settled, inactive; "" from an older hub
    val swarm: String = "", // the swarm room this thread is a member of (feature swarm), "" for none: left out of its project's list, which shows the swarm once
)
// the picker under the composer (`thread.picker`): sections the hub lists, drawn in order; a press is `thread.pick`
data class DChoice(val label: String, val value: String, val on: Boolean, val ok: Boolean, val why: String)
data class DPickerSection(val title: String, val action: String, val choices: List<DChoice>)
data class DPicker(val chip: String, val sections: List<DPickerSection>)
/** What a new thread will run with, chosen before it exists. */
data class DNewThreadChoice(val provider: String? = null, val model: String? = null, val effort: String? = null, val runtime: String? = null) {
    fun set(action: String, value: String) = when (action) { "provider" -> copy(provider = value, model = "", effort = ""); "model" -> copy(model = value); "effort" -> copy(effort = value); else -> copy(runtime = value) }
    fun into(p: JSONObject, skipEmpty: Boolean = false): JSONObject { listOf("provider" to provider, "model" to model, "effort" to effort, "runtime" to runtime).forEach { (k, v) -> if (v != null && (v.isNotEmpty() || !skipEmpty)) p.put(k, v) }; return p }
}
fun parsePicker(o: JSONObject) = DPicker(o.str("chip"), o.optJSONArray("sections").list { s -> DPickerSection(s.str("title"), s.str("action"), s.optJSONArray("choices").list { c -> DChoice(c.str("label").ifEmpty { c.str("value") }, c.str("value"), c.optBoolean("on"), c.optBoolean("ok", true), c.str("why")) }) })

fun parseOption(o: JSONObject) = DOption(o.str("id"), o.str("label").ifEmpty { o.str("id") })
fun parseAsk(o: JSONObject) = DAsk(o.str("id"), o.str("state"), o.str("question"), o.optJSONArray("options").list(::parseOption), if (o.has("answered") && !o.isNull("answered")) o.str("answered") else null)
fun parseEntry(o: JSONObject) = DEntry(
    o.str("id"), o.str("kind"), o.str("text"), o.lng("at"), o.optString("clientId", "").ifEmpty { null }, o.optString("replaces", "").ifEmpty { null },
    o.optBoolean("live", false),
    o.optJSONObject("tool")?.let { DTool(it.str("name"), it.str("summary"), it.str("state"), it.str("sub"), it.str("result")) },
    o.optJSONObject("ask")?.let(::parseAsk),
    o.optJSONArray("attachments").list { DAttachment(it.str("id"), it.str("name"), it.str("mime"), it.lng("bytes")) },
    label = o.str("label"), tone = o.str("tone"), agentOnly = o.optBoolean("agentOnly", false),
    term = o.optJSONObject("term")?.let { DTermRef(it.str("key"), it.str("id"), it.str("title"), it.optInt("cols", 80), it.optInt("rows", 24), it.str("cmd")) },
    handoff = o.optJSONObject("handoff")?.let(::parseHandoff),
    replyTo = o.optJSONObject("replyTo")?.takeIf { it.str("id").isNotEmpty() }?.let { DReplyTo(it.str("id"), it.str("quote")) },
)
fun parseProject(o: JSONObject) = DProject(o.str("id"), o.str("title"), o.str("root"), o.optInt("threadCount", 0), o.lng("updatedAt"), o.optInt("waiting", 0), o.optString("icon", "folder"), o.optString("css"), if (o.has("settledCount") && !o.isNull("settledCount")) o.optInt("settledCount") else null,
    parseMark(o.optJSONObject("mark")), o.str("also"), o.optJSONArray("newOn").list { DNewOn(it.str("value"), it.str("label"), it.optBoolean("away")) }, o.str("runsOn"), o.str("key"))
fun parseThread(o: JSONObject) = DThread(
    o.str("id"), o.str("project"), o.str("title"), o.str("state"), o.lng("updatedAt"), o.optBoolean("pinned"), o.optBoolean("settled"), o.optBoolean("archived"),
    o.str("provider"), o.str("model"), o.str("preview"), o.optInt("waiting", 0), o.str("parent"), o.str("effort"), o.str("runtime"), o.str("chip"), o.lng("settledAt"), o.optBoolean("history"), parseMark(o.optJSONObject("mark")), o.optInt("pinOrder", 0), o.str("status"), o.str("swarm"),
)
fun parseProjects(a: JSONArray?) = a.list(::parseProject)
fun parseThreads(a: JSONArray?) = a.list(::parseThread)
// an entry without an id (a hub that sent none) is keyed by its place, so a list never repeats a key
fun parseEntries(a: JSONArray?) = a.list(::parseEntry).mapIndexed { i, e -> if (e.id.isEmpty()) e.copy(id = "~$i:${e.kind}:${e.at}") else e }

// A thread's background work and locks (thread.work, the "work" push): the rows the hub decides, each with what its button sends.
data class DWorkRow(val title: String, val kind: String, val state: String, val doing: String, val act: String, val value: String, val label: String, val key: String = "", val meta: String = "") // key: a subagent's, to open its view
data class DWork(val word: String, val live: Int, val rows: List<DWorkRow>)
// Who holds a lock (locks.list, the "locks" push): `line` is the hub's words for it.
data class DLock(val name: String, val holder: String, val thread: String, val line: String, val waiting: Int)
fun parseWork(o: JSONObject) = DWork(o.str("word"), o.optInt("live", 0), o.optJSONArray("rows").list { DWorkRow(it.str("title"), it.str("kind"), it.str("state"), it.str("doing"), it.str("act"), it.str("value"), it.str("label"), if (it.has("detail") && !it.isNull("detail")) it.str("key") else "", it.str("meta")) })
/** One subagent's own view (feature subview: `thread.sub`, the `sub` push): its head as its panel row says it, and its steps as entries, oldest first. */
data class DSubView(val key: String, val thread: String, val title: String, val state: String, val live: Boolean, val meta: String, val took: String, val where: String, val ask: String,
    val outcome: String, val act: String, val value: String, val label: String, val more: Int, val entries: List<DEntry>)
fun parseSubView(o: JSONObject) = DSubView(o.str("key"), o.str("thread"), o.str("title"), o.str("state"), o.optBoolean("live"), o.str("meta"), o.str("took"), o.str("where"), o.str("ask"),
    o.str("outcome"), o.str("act"), o.str("value"), o.str("label"), o.optInt("more", 0), o.optJSONArray("entries").list(::parseEntry))
fun parseLocks(a: JSONArray?) = a.list { DLock(it.str("name"), it.str("holder"), it.str("thread"), it.str("line"), it.optInt("waiting", 0)) }

/** Conversation history (`history.list`): the old Scratch chats. `live` always have a row; `rows` only while the group is open; `count` is every chat. */
data class DHistory(val count: Int = 0, val live: List<DThread> = emptyList(), val rows: List<DThread> = emptyList(), val next: String? = null) {
    /** What the home list shows under the group's row: the live chats, then the rest when it is open. No row here ever starts a chat. */
    fun shown(open: Boolean): List<DThread> = live + (if (open) rows.filter { r -> live.none { it.id == r.id } } else emptyList())
    /** The group's row: drawn while any chat is in it. */
    val title get() = Msg.android_history_title(count)
    companion object {
        fun parse(o: JSONObject, before: DHistory? = null) = DHistory(o.optInt("count"), parseThreads(o.optJSONArray("live")),
            (before?.rows ?: emptyList()) + parseThreads(o.optJSONArray("threads")), if (o.isNull("next") || !o.has("next")) null else o.optString("next"))
    }
}

/** A row of the queue strip over the composer (`thread.queue`, docs/PHONE-DATA.md), the hub's or this phone's own outbox's. */
data class QButton(val label: String, val action: String, val value: String)
data class QRow(val msg: String, val tag: String, val text: String, val buttons: List<QButton>)
