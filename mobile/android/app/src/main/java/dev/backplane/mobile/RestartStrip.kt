package dev.backplane.mobile

import org.json.JSONObject

/** The hub's restart said in the connection strip (feature "restarting", docs/PHONE-DATA.md, Restarts). `text` is the hub's English (the strip
 *  translates it); `since` is when this phone heard it, on its own clock (the hub's `at` names the note, the hub's clock is not ours). */
data class RestartNote(val text: String, val since: Long)

object RestartStrip {
    /** a hub that said it was restarting and has not come back in this long: the usual reconnecting look again */
    const val AWAY_MS = 5 * 60_000L
    /** how long the note of a hub back from a crash shows as the app joins */
    const val BACK_MS = 6_000L

    /** The strip's words, or null for the usual look (the reconnecting strip while offline, nothing while online).
     *  `going`: the last `hub.restarting` heard since the last hello; `back`: a hub-back note this app has not shown before. */
    fun text(going: RestartNote?, back: RestartNote?, online: Boolean, now: Long): String? = when {
        online -> back?.takeIf { now - it.since in 0 until BACK_MS }?.text
        else -> going?.takeIf { now - it.since in 0 until AWAY_MS }?.text
    }

    /** When `text` next changes by time alone (the one delayed post), or null. */
    fun next(going: RestartNote?, back: RestartNote?, online: Boolean, now: Long): Long? {
        val end = if (online) back?.let { it.since + BACK_MS } else going?.let { it.since + AWAY_MS }
        return end?.takeIf { it > now && text(going, back, online, now) != null }
    }

    /** `hello.hubBack` / info `hub.back`: "" or a JSON string `{reason, text, at}`. Its `at` (to show each note once) and text, or null. */
    fun back(raw: String?): Pair<Long, String>? {
        if (raw.isNullOrBlank()) return null
        val o = runCatching { JSONObject(raw) }.getOrNull() ?: return null
        val t = o.optString("text"); if (t.isEmpty()) return null
        return o.optLong("at") to t
    }
}
