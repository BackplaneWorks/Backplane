package dev.backplane.mobile

/** A reply being written (feature `reply`): the message answered, its text as shown, and who wrote it ("user" or "assistant"). */
data class DReplyRef(val id: String, val text: String, val role: String)

/**
 * The desktop's Reply on the phone (docs/PHONE-DATA.md, feature `reply`): which messages offer it, what the composer's strip shows, and what the send
 * carries. The hub builds the `<in_reply_to msg=… chars=…>` block from `replyTo {id, text}` (bots/reply.rs `wrap`), so the agent reads what it reads
 * from the desktop. Pure (test: ReplyDraftTest). iOS: ReplyDraft.swift.
 */
object ReplyDraft {
    /** the message a Reply on this entry answers; null where there is none: not a message, nothing written, or not yet on the hub */
    fun of(id: String, kind: String, text: String, live: Boolean = false): DReplyRef? =
        if ((kind == "user" || kind == "assistant") && !live && id.isNotEmpty() && !id.startsWith("local:") && !id.startsWith("live:") && text.isNotBlank()) DReplyRef(id, text, kind) else null
    /** the first line that is not blank, 200 characters at most: the strip's line and the bubble's quote (the hub's own, data/mod.rs) */
    fun firstLine(t: String): String = (t.split("\n").map { it.trim() }.firstOrNull { it.isNotEmpty() } ?: "").take(200)
    /** the sender: You or Agent */
    fun mine(r: DReplyRef) = r.role == "user"
    /** `message.send`'s `replyTo` */
    fun param(r: DReplyRef): Map<String, String> = mapOf("id" to r.id, "text" to r.text)
    /** the chip a sent reply shows until the hub's entry replaces it */
    fun chip(r: DReplyRef) = DReplyTo(r.id, firstLine(r.text))
    /** a swipe right far enough, and more right than down, is a Reply (in dp) */
    fun swipe(dx: Float, dy: Float) = dx >= 60f && kotlin.math.abs(dy) < dx / 2
}
