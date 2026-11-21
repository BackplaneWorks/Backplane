package dev.backplane.mobile

import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import kotlin.coroutines.coroutineContext

// One download, start to whole (docs/file-links.md, "Resuming downloads"): the requests FileFetch plans, each answer's body streamed into the `.part`
// file, never held in memory. Plain JVM (OkHttp, no Android), so FileFetchLiveTest runs this same code against a live hub. Call it off the main thread.
object FileFetchRun {
    /** Where to ask, as it is now (the hub the app is connected to may change between tries); null: not connected. */
    data class Where(val url: String, val token: String?)
    sealed class Out {
        /** The `.part` file holds the whole file. */
        data class Whole(val part: File) : Out()
        /** Given up: the hub's words, or the connection's. */
        data class Fail(val why: String) : Out()
    }
    private const val BUF = 256 shl 10

    suspend fun fetch(
        http: OkHttpClient, where: () -> Where?, part: File, tagFile: File, size: Long, offline: String, failed: String,
        progress: suspend (have: Long, total: Long) -> Unit = { _, _ -> }, wait: suspend (Long) -> Unit = { delay(it) },
    ): Out {
        var tries = 0; var total = size
        while (true) {
            coroutineContext.ensureActive()
            val w = where() ?: return Out.Fail(offline)
            val have = if (part.exists()) part.length() else 0L
            val tag = if (tagFile.exists()) tagFile.readText() else ""
            val ask = FileFetch.ask(have, tag)
            val req = Request.Builder().url(w.url).apply { w.token?.let { header("Authorization", "Bearer $it") }; ask.forEach { (k, v) -> header(k, v) } }.build()
            val from = if (ask.isEmpty()) 0L else have
            try {
                val step = http.newCall(req).execute().use { r ->
                    val body = r.body
                    val st = FileFetch.step(r.code, from, r.header("Content-Range"), body?.contentLength() ?: -1, if (r.code in 200..299) "" else body?.string()?.take(400) ?: "")
                    if (st is FileFetch.Step.Write && body != null) {
                        r.header("ETag")?.let { tagFile.writeText(it) } ?: tagFile.delete()
                        total = if (st.total >= 0) st.total else if (body.contentLength() >= 0) st.from + body.contentLength() else -1
                        FileOutputStream(part, st.from > 0).use { o ->
                            val inp = body.byteStream(); val buf = ByteArray(BUF); var got = st.from; var shown = -1L
                            while (true) {
                                coroutineContext.ensureActive()
                                val n = inp.read(buf); if (n < 0) break
                                o.write(buf, 0, n); got += n
                                if (shown < 0 || got - shown >= (1 shl 20)) { shown = got; progress(got, total) }
                            }
                            if (got > st.from) tries = 0   // it moved on: a dropped connection starts its count again
                        }
                    }
                    st
                }
                when (step) {
                    is FileFetch.Step.Fail -> return Out.Fail(step.why)
                    FileFetch.Step.Again -> { part.delete(); tagFile.delete(); if (++tries >= FileFetch.TRIES) return Out.Fail(failed); continue }
                    FileFetch.Step.Done -> return Out.Whole(part)
                    is FileFetch.Step.Write -> {
                        if (FileFetch.complete(part.length(), total) || total < 0) return Out.Whole(part)
                        // the body ended short (the link dropped, the file changed): ask for the rest
                        if (++tries >= FileFetch.TRIES) return Out.Fail(failed); wait(FileFetch.backoffMs(tries))
                    }
                }
            } catch (e: IOException) {
                if (part.exists() && part.length() > from) tries = 0   // it moved on before the link dropped
                if (++tries >= FileFetch.TRIES) return Out.Fail(e.message ?: failed)
                progress(if (part.exists()) part.length() else 0, total)
                wait(FileFetch.backoffMs(tries))
            }
        }
    }
}
