package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject

// docs/PHONE-DATA.md (data/1): the plain data the hub sends, as typed values. Parsing is forgiving: a missing field is empty, an extra one ignored.
private fun JSONObject.str(k: String): String = if (isNull(k)) "" else optString(k, "")
private fun JSONObject.lng(k: String): Long = optLong(k, 0L)
private fun <T> JSONArray?.list(f: (JSONObject) -> T): List<T> = if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it)?.let(f) }

data class DOption(val id: String, val label: String)
data class DTool(val name: String, val summary: String, val state: String)
data class DAsk(val id: String, val state: String, val question: String, val options: List<DOption>, val answered: String?) { val open get() = state == "open" }
data class DAttachment(val id: String, val name: String, val mime: String, val bytes: Long)
data class DEntry(
    val id: String, val kind: String, val text: String, val at: Long, val clientId: String? = null, val replaces: String? = null,
    val live: Boolean = false, val tool: DTool? = null, val ask: DAsk? = null, val attachments: List<DAttachment> = emptyList(),
    val sending: Boolean = false, val failed: Boolean = false,
    val localImages: List<ByteArray> = emptyList(), // phone-only: the pictures of a message this phone is sending
)
data class DProject(val id: String, val title: String, val root: String, val threadCount: Int, val updatedAt: Long, val waiting: Int)
data class DThread(
    val id: String, val project: String, val title: String, val state: String, val updatedAt: Long, val pinned: Boolean, val settled: Boolean,
    val archived: Boolean, val provider: String, val model: String, val preview: String, val waiting: Int,
)

fun parseOption(o: JSONObject) = DOption(o.str("id"), o.str("label").ifEmpty { o.str("id") })
fun parseAsk(o: JSONObject) = DAsk(o.str("id"), o.str("state"), o.str("question"), o.optJSONArray("options").list(::parseOption), if (o.has("answered") && !o.isNull("answered")) o.str("answered") else null)
fun parseEntry(o: JSONObject) = DEntry(
    o.str("id"), o.str("kind"), o.str("text"), o.lng("at"), o.optString("clientId", "").ifEmpty { null }, o.optString("replaces", "").ifEmpty { null },
    o.optBoolean("live", false),
    o.optJSONObject("tool")?.let { DTool(it.str("name"), it.str("summary"), it.str("state")) },
    o.optJSONObject("ask")?.let(::parseAsk),
    o.optJSONArray("attachments").list { DAttachment(it.str("id"), it.str("name"), it.str("mime"), it.lng("bytes")) },
)
fun parseProject(o: JSONObject) = DProject(o.str("id"), o.str("title"), o.str("root"), o.optInt("threadCount", 0), o.lng("updatedAt"), o.optInt("waiting", 0))
fun parseThread(o: JSONObject) = DThread(
    o.str("id"), o.str("project"), o.str("title"), o.str("state"), o.lng("updatedAt"), o.optBoolean("pinned"), o.optBoolean("settled"), o.optBoolean("archived"),
    o.str("provider"), o.str("model"), o.str("preview"), o.optInt("waiting", 0),
)
fun parseProjects(a: JSONArray?) = a.list(::parseProject)
fun parseThreads(a: JSONArray?) = a.list(::parseThread)
// an entry without an id (a hub that sent none) is keyed by its place, so a list never repeats a key
fun parseEntries(a: JSONArray?) = a.list(::parseEntry).mapIndexed { i, e -> if (e.id.isEmpty()) e.copy(id = "~$i:${e.kind}:${e.at}") else e }
