package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject

// This decodes the bounded MMail projection. Ordering, recipients, body windows,
// send correlation and automation policy remain in .
data class MailroomRow(val id: String, val subject: String, val who: String, val head: String,
    val unread: Boolean, val live: Boolean)
data class MailroomMessage(val id: String, val who: String, val text: String, val sourceURI: String)
data class MailroomTarget(val address: String, val label: String)
data class MailroomBody(val id: String, val subject: String, val messages: List<MailroomMessage>,
    val previousAnchor: String, val nextAnchor: String, val remoteEarlier: Boolean, val loading: Boolean,
    val targets: List<MailroomTarget>, val to: String, val draft: String, val sendState: String)
data class MailroomModel(val hub: String, val machine: String, val owner: String, val origin: String,
    val filter: String, val query: String, val status: String, val online: Boolean, val loading: Boolean,
    val searchCapped: Boolean, val first: Int, val total: Int, val rows: List<MailroomRow>, val body: MailroomBody?)

private fun <T> JSONArray?.mailItems(f: (JSONObject) -> T): List<T> =
    if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it)?.let(f) }

fun mailroomModel(o: JSONObject): MailroomModel {
    val body = o.optJSONObject("body")?.let { b ->
        MailroomBody(b.optString("id"), b.optString("subject"), b.optJSONArray("messages").mailItems {
            MailroomMessage(it.optString("id"), it.optString("who"), it.optString("text"), it.optString("sourceURI"))
        }, b.optString("previousAnchor"), b.optString("nextAnchor"), b.optBoolean("remoteEarlier"), b.optBoolean("loading"),
            b.optJSONArray("targets").mailItems { MailroomTarget(it.optString("address"), it.optString("label")) },
            b.optString("to"), b.optString("draft"), b.optString("sendState"))
    }
    return MailroomModel(o.optString("hub"), o.optString("machine"), o.optString("owner"), o.optString("origin"),
        o.optString("filter"), o.optString("query"), o.optString("status"), o.optBoolean("online"), o.optBoolean("loading"),
        o.optBoolean("searchCapped"), o.optInt("first"), o.optInt("total"), o.optJSONArray("rows").mailItems {
            MailroomRow(it.optString("id"), it.optString("subject"), it.optString("who"), it.optString("head"), it.optBoolean("unread"), it.optBoolean("live"))
        }, body)
}

object MailroomAction {
    fun sourceRequest(mail: MailroomModel, uri: String): String = JSONObject().put("uri", uri).put("hub", mail.hub).toString()
    fun waiting(contentReady: Boolean, online: Boolean, loading: Boolean) = !contentReady || !online || loading
    fun currentReady(contentReady: Boolean, paired: Boolean, captured: MailroomModel, current: MailroomModel?): Boolean {
        if (waiting(contentReady, captured.online, captured.loading || captured.body?.loading == true) || !paired) return false
        if (current == null || waiting(contentReady, current.online, current.loading || current.body?.loading == true)) return false
        return current.hub == captured.hub && current.origin == captured.origin && current.owner == captured.owner &&
            current.body?.id == captured.body?.id
    }
    fun busy(state: String) = state in setOf("sending", "ack", "pending", "accepted", "unknown")
    fun actionAllowed(current: MailroomModel?, action: String, value: String = "", recipient: String? = null): Boolean = when (action) {
        "mail-to" -> current?.body?.targets?.any { it.address == value } == true
        "mail-send" -> current?.body?.let { body -> !busy(body.sendState) && body.targets.any { it.address == recipient } } == true
        else -> true
    }
    fun request(mail: MailroomModel, action: String, value: String = "", draft: String? = null, recipient: String? = null): String =
        JSONObject().put("hub", mail.hub).put("owner", mail.owner).put("conversation", mail.body?.id ?: "")
            .put("action", action).put("value", value).apply {
                if (draft != null) put("draft", draft)
                if (recipient != null) put("recipient", recipient)
            }.toString()
}

