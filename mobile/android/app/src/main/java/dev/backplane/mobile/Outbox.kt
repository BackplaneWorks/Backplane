package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject

/** One message this phone could not hand to its hub: the socket was down. `hub` is the pairing key it was written to. */
data class OItem(val hub: String, val thread: String, val text: String, val clientId: String)

/**
 * The phone's outbox (docs/PHONE-DATA.md, `thread.queue`): messages written while the data socket was down, kept on disk, drawn in the
 * queue strip as `Unsent` rows with Edit and Remove of the phone's own (`u-edit` / `u-remove`, never sent to the hub), and sent in the
 * order written once connected, each with its clientId (`message.send` with a clientId posts once, so a repeated send is one message).
 * A value: every change answers a new Outbox. (iOS: Outbox.swift.)
 */
class Outbox(val items: List<OItem> = emptyList()) {
    fun push(hub: String, thread: String, text: String, clientId: String): Outbox =
        if (items.any { it.clientId == clientId }) this else Outbox(items + OItem(hub, thread, text, clientId))
    fun remove(clientId: String): Outbox = Outbox(items.filter { it.clientId != clientId })
    /** the hub answered ok: it is posted */
    fun sent(clientId: String): Outbox = remove(clientId)
    fun item(clientId: String): OItem? = items.firstOrNull { it.clientId == clientId }
    fun has(hub: String, thread: String): Boolean = items.any { it.hub == hub && it.thread == thread }
    /** the oldest message waiting for this hub: the next to send */
    fun next(hub: String): OItem? = items.firstOrNull { it.hub == hub }
    /** the strip's rows for a thread, oldest first */
    fun rows(hub: String, thread: String): List<QRow> = items.filter { it.hub == hub && it.thread == thread }.map {
        QRow(it.clientId, "Unsent", it.text.lineSequence().firstOrNull { l -> l.isNotBlank() }?.trim() ?: "", listOf(QButton("Edit", EDIT, it.clientId), QButton("Remove", REMOVE, it.clientId)))
    }
    fun encode(): String = JSONObject().put("v", 1).put("items", JSONArray(items.map { JSONObject().put("hub", it.hub).put("thread", it.thread).put("text", it.text).put("clientId", it.clientId) })).toString()
    override fun equals(other: Any?) = other is Outbox && other.items == items
    override fun hashCode() = items.hashCode()

    companion object {
        const val EDIT = "u-edit"
        const val REMOVE = "u-remove"
        fun isOwn(action: String) = action == EDIT || action == REMOVE
        /** what [encode] wrote; anything unreadable is an empty outbox */
        fun decode(s: String?): Outbox {
            if (s.isNullOrBlank()) return Outbox()
            val a = runCatching { JSONObject(s).optJSONArray("items") }.getOrNull() ?: return Outbox()
            return Outbox((0 until a.length()).mapNotNull { a.optJSONObject(it) }.mapNotNull { o ->
                val cid = o.optString("clientId"); val t = o.optString("thread")
                if (cid.isEmpty() || t.isEmpty()) null else OItem(o.optString("hub"), t, o.optString("text"), cid)
            })
        }
    }
}
