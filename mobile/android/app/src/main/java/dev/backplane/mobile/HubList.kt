package dev.backplane.mobile

import org.json.JSONArray

// The hubs this phone is paired with, and where an invitation is tried. Pure: no Android types, so HubListTest runs it on the JVM.
// iOS's HubList.swift is the same, and test/mobile/hubs_cases.json holds both to the same answers. The first link is the hub in use.
object HubList {
    /** scheme://host[:port] of an http(s) address, or null: string rules, not a URL parser's, so both phones agree. */
    fun origin(link: String): String? {
        val s = link.trim()
        val at = s.indexOf("://"); if (at <= 0) return null
        val scheme = s.substring(0, at).lowercase()
        val rest = s.substring(at + 3)
        val end = rest.indexOfFirst { it == '/' || it == '?' || it == '#' }
        val auth = if (end < 0) rest else rest.substring(0, end)
        if (scheme != "http" && scheme != "https" || auth.isEmpty() || auth.contains('@')) return null
        return "$scheme://$auth"
    }
    /** host:port of an http(s) or ws(s) link */
    fun authority(link: String): String? {
        val s = link.trim().let { if (it.startsWith("ws://")) "http://" + it.drop(5) else if (it.startsWith("wss://")) "https://" + it.drop(6) else it }
        return origin(s)?.substringAfter("://")
    }

    /** A newly paired hub goes first; an older pairing with the same hub is replaced, every other hub is kept. */
    fun adopt(links: List<String>, link: String, key: (String) -> String?): List<String> { val k = key(link); return listOf(link) + links.filter { key(it) != k } }
    /** A hub an offer added goes second: the one in use stays in use; an older pairing with it is replaced. */
    fun added(links: List<String>, link: String, key: (String) -> String?): List<String> {
        val k = key(link); val rest = links.filter { key(it) != k }
        return if (rest.isEmpty()) listOf(link) else listOf(rest[0], link) + rest.drop(1)
    }
    /** Switching to a paired hub moves it first; an unknown key changes nothing. */
    fun use(links: List<String>, k: String, key: (String) -> String?): List<String> {
        val hit = links.firstOrNull { key(it) == k } ?: return links
        return listOf(hit) + links.filter { key(it) != k }
    }
    fun forget(links: List<String>, k: String, key: (String) -> String?): List<String> = links.filter { key(it) != k }

    /** The addresses to try an invitation at, in order: the link's own, then each other one its `routes` field names. */
    fun routes(text: String): List<String> {
        val out = mutableListOf<String>()
        origin(text)?.let { out += it }
        val frag = text.substringAfter('#', "")
        for (kv in frag.split('&')) {
            if (!kv.startsWith("routes=")) continue
            val xs = runCatching { JSONArray(java.net.URLDecoder.decode(kv.removePrefix("routes="), "UTF-8")) }.getOrNull() ?: continue
            for (i in 0 until xs.length()) { val o = (xs.opt(i) as? String)?.let { origin(it) } ?: continue; if (o !in out) out += o }
        }
        return out
    }
    fun loopback(origin: String): Boolean { val r = origin.substringAfter("://", ""); return r.startsWith("localhost") || r.startsWith("127.") || r.startsWith("[::1]") }
    /** Every address is the hub's own computer: a phone that opens it reaches itself, never the hub. */
    fun local(routes: List<String>): Boolean = routes.isNotEmpty() && routes.all { loopback(it) }


    // ---- the home hub (docs/pairing.md, Home hub): an offer to add it, and failing over to it
    data class Offer(val id: String, val name: String, val url: String, val why: String, val credential: String)
    /** The hub's `hub.offer` (a JSON string, "" for none), or null when it names no hub. */
    fun offer(text: String): Offer? {
        val o = runCatching { org.json.JSONObject(text) }.getOrNull() ?: return null
        val id = o.optString("id"); val url = origin(o.optString("url")) ?: return null
        if (id.isEmpty()) return null
        return Offer(id, o.optString("name"), url, o.optString("why"), o.optString("credential"))
    }
    sealed class Plan { object None : Plan(); data class Known(val key: String) : Plan(); data class Same(val key: String, val link: String) : Plan(); data class Take(val key: String) : Plan() }
    /** What an offer asks of this phone: nothing, a hub already paired (it becomes the home), the same token at the offered address, or a link to take and redeem.
     *  token is the one this phone holds for the hub in use; done the offers already answered. */
    fun plan(text: String, links: List<String>, token: String?, done: Set<String>, key: (String) -> String?): Plan {
        val o = offer(text) ?: return Plan.None
        if (o.id in done) return Plan.None
        val k = key(o.url) ?: return Plan.None
        if (links.firstOrNull()?.let(key) == k) return Plan.None
        if (links.any { key(it) == k }) return Plan.Known(k)
        if (o.credential == "same" && !token.isNullOrEmpty()) return Plan.Same(k, o.url + "/#token=" + token)
        return Plan.Take(k)
    }
    /** How long the hub in use may not answer before the home is used instead, in ms. */
    const val FAILOVER_MS = 20_000L
    /** Whether to be on the home hub: main is the hub in use, home the remembered one ("" for none), down how long main has not answered. */
    fun onHome(main: String, home: String, now: Boolean, downMs: Long, homeUp: Boolean, mainUp: Boolean): Boolean {
        if (home.isEmpty() || home == main) return false
        if (now) return !mainUp
        return downMs >= FAILOVER_MS && homeUp
    }

    /** What the phone says of an invitation that names only its hub's computer. */
    const val LOCAL = "This invitation names only its hub's own computer (127.0.0.1), so this phone cannot reach it. Start the hub with Tailscale on, or on an address this phone can reach, then make a new invitation."

    /** The stored list: a JSON array of links (an older build kept one link under "link"). */
    fun decode(json: String?, old: String?): List<String> {
        val a = json?.let { runCatching { JSONArray(it) }.getOrNull() }
        if (a != null) return (0 until a.length()).mapNotNull { a.opt(it) as? String }
        return listOfNotNull(old)
    }
    fun encode(links: List<String>): String = JSONArray(links).toString()
}
