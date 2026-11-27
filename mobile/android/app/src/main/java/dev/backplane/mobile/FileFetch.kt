package dev.backplane.mobile

// Files of the hub's machine on the phone (docs/file-links.md; the iOS app's FileFetch.swift is the same rules): which tapped link is a file the hub
// serves, the address it is fetched from, and how a download that stopped goes on. Pure Kotlin, so FileFetchTest runs it without the app.

/** A file an assistant entry names that the hub serves (data/1 feature `fileLinks`): `open` is "view" (Backplane's viewers draw it) or "download". */
data class DFileLink(val path: String, val name: String, val size: Long, val open: String, val project: String = "", val absPath: String = "")

object FileFetch {
    // what the hub's share_route writes: the token, then the name with everything but [A-Za-z0-9._~-] percent-encoded (never `\`, `?`, `#`, `..`)
    private val share = Regex("^/s/[0-9A-Fa-f]{48}/(?:[A-Za-z0-9._~-]|%[0-9A-Fa-f]{2})+$")
    private val lineTail = Regex("(?::\\d{1,6}){1,2}$|#L\\d{1,6}(?:-L?\\d{1,6})?$")

    /** A share's address (`/s/<token>/<name>`, written in a message): fetched as it is, the token its own credential. */
    fun shareRoute(p: String): String? {
        val clean = p.trim().substringBefore('#').substringBefore('?')
        return clean.takeIf { share.matches(it) && it.substringAfterLast('/') !in setOf(".", "..") }
    }
    /** A share's route carries its own credential: the device's token is not sent with it. */
    fun isShare(route: String) = route.startsWith("/s/")
    /** Whether the address made from a route still asks for that route (no `..` or `\` in it took it elsewhere). */
    fun sameRoute(encodedPath: String, route: String) = encodedPath == route

    /** A name safe to be a file's: no folder in it, never `.` or `..`. */
    fun safeName(n: String): String { val t = n.replace('/', '_').replace('\\', '_').trim(); return if (t.isEmpty() || t == "." || t == "..") "download" else t }

    /** A path as a link carries it, without `file://`, fragment, query, or the line it points at (`a.rs:12`, `a.rs:3:4`, `a.rs#L4-L9`). */
    fun bare(p: String): String {
        val s = fileUrlPath(p)?.substringBefore('#') ?: p.trim()
        return lineTail.replace(s, "")
    }

    /** The file of [files] a tapped path names, in the message's spelling (or resolved `absPath`). */
    fun find(p: String, files: List<DFileLink>): DFileLink? {
        val b = bare(p)
        return files.firstOrNull { it.path == p || it.path == b || bare(it.path) == b || (it.absPath.isNotEmpty() && bare(it.absPath) == b) }
    }

    /** A thread of another machine (`<link>~<id>`) is named on the request; this hub's own thread is not. */
    fun far(thread: String) = thread.contains('~')

    /** `/file`'s query: the path, then the far thread when there is one (values percent-encoded by [enc]). */
    fun fileQuery(path: String, thread: String, enc: (String) -> String): String = "path=" + enc(path) + if (far(thread)) "&thread=" + enc(thread) else ""

    /** The name a share saves as: its last part, percent-decoding undone. */
    fun shareName(route: String): String = route.substringAfterLast('/').let { runCatching { java.net.URLDecoder.decode(it.replace("+", "%2B"), "UTF-8") }.getOrDefault(it) }

    /** The headers that ask for the rest of a file: none from nothing (or without the tag the first answer had: the start again), else the range and its tag. */
    fun ask(have: Long, etag: String): Map<String, String> = if (have <= 0 || etag.isEmpty()) emptyMap() else mapOf("Range" to "bytes=$have-", "If-Range" to etag)

    /** What to do with an answer. */
    sealed class Step {
        /** Write from [from] (0: truncate the `.part` and start over; else append); [total] the whole file's length, -1 unknown. */
        data class Write(val from: Long, val total: Long) : Step()
        /** Nothing more to fetch: the `.part` is the file. */
        object Done : Step()
        /** The `.part` is no good (a range that does not join it): drop it and ask for the whole file. */
        object Again : Step()
        /** The hub refused: show [why] (its own words when it sent some). */
        data class Fail(val why: String) : Step()
    }

    /** The answer's status and headers made a [Step]. [have]: the bytes the `.part` holds and was asked to go on from. */
    fun step(code: Int, have: Long, contentRange: String?, contentLength: Long, body: String = ""): Step = when (code) {
        206 -> {
            val m = Regex("^bytes (\\d+)-(\\d+)/(\\d+|\\*)$").find(contentRange?.trim() ?: "")
            val a = m?.groupValues?.get(1)?.toLongOrNull(); val total = m?.groupValues?.get(3)?.toLongOrNull() ?: -1L
            // a range that does not start where the part ends cannot be appended: the start again
            if (a == have) Step.Write(have, total) else Step.Again
        }
        200 -> Step.Write(0, contentLength)
        // asked for bytes past the end: a part that is already the whole file, or a file that shrank (the start again)
        416 -> { val total = Regex("^bytes \\*/(\\d+)$").find(contentRange?.trim() ?: "")?.groupValues?.get(1)?.toLongOrNull(); if (total != null && total == have) Step.Done else Step.Again }
        else -> Step.Fail(body.trim().take(200).ifEmpty { "HTTP $code" })
    }

    /** Whether a `.part` of [have] bytes is the whole file. */
    fun complete(have: Long, total: Long) = total >= 0 && have == total

    /** How long to wait before try [n] (1, 2, …) after a dropped connection. */
    fun backoffMs(n: Int): Long = minOf(30_000L, 1000L shl (n - 1).coerceIn(0, 5))
    const val TRIES = 6
}
