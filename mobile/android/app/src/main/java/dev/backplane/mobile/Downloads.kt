package dev.backplane.mobile

import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.webkit.MimeTypeMap
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.util.concurrent.TimeUnit

// Downloads of files on the hub's machine (docs/file-links.md): a file a reply names (`/file?path=`), a share (`/s/<token>/<name>`) or a Files row
// (`/files/get`), over HTTP on the hub the app is connected to, with the device's token. Streamed into `<name>.part` in the app's cache, never held in
// memory; after a dropped connection it goes on from where it stopped (FileFetch: Range and If-Range, 206 appends, 200 starts over). When it is whole it
// is copied to Downloads (MediaStore) and the row offers Open (ACTION_VIEW on its FileProvider address) and Share.
data class DownloadRow(val key: String, val name: String, val have: Long, val total: Long, val state: State, val note: String = "", val file: File? = null) {
    enum class State { RUN, DONE, FAIL }
    val fill: Float get() = if (total > 0) (have.toFloat() / total).coerceIn(0f, 1f) else 0f
}

object Downloads {
    val rows = mutableStateListOf<DownloadRow>()
    private val jobs = HashMap<String, Job>()
    private val http = OkHttpClient.Builder().connectTimeout(20, TimeUnit.SECONDS).readTimeout(60, TimeUnit.SECONDS).build()
    private var ctx: Context? = null
    private const val BUF = 256 shl 10

    /** A file a reply names: `/file?path=` (and `thread=` for a thread on another machine). */
    fun file(ctx: Context, f: DFileLink, thread: String) =
        start(ctx, "/file", FileFetch.fileQuery(f.path, thread, Uri::encode), f.name.ifEmpty { f.path.substringAfterLast('/') }, f.size)
    /** A share written in a message: its own address, no query. */
    fun share(ctx: Context, route: String) = start(ctx, route, "", FileFetch.shareName(route), -1)
    /** A file of the Files tab: `/files/get` (the tab's own containment). */
    fun project(ctx: Context, thread: String, path: String) =
        start(ctx, "/files/get", "thread=" + Uri.encode(thread) + "&path=" + Uri.encode(path), path.substringAfterLast('/'), -1)

    private fun dirOf(c: Context, key: String) = File(File(c.cacheDir, "downloads"), Integer.toHexString(key.hashCode()) + "-" + key.length)

    fun start(c: Context, route: String, query: String, given: String, size: Long) {
        ctx = c.applicationContext
        val name = given.replace('/', '_').replace('\\', '_').trim().ifEmpty { "download" }
        val key = "$route?$query"
        if (jobs[key]?.isActive == true) return
        put(DownloadRow(key, name, 0, size, DownloadRow.State.RUN))
        jobs[key] = DataClient.scope.launch { run(c.applicationContext, key, route, query, name, size) }
    }

    fun cancel(key: String) { jobs.remove(key)?.cancel(); rows.removeAll { it.key == key } }
    fun dismiss(key: String) { if (jobs[key]?.isActive != true) rows.removeAll { it.key == key } }
    fun retry(key: String) { val r = rows.firstOrNull { it.key == key } ?: return; val c = ctx ?: return
        val (route, query) = key.substringBefore('?') to key.substringAfter('?', ""); start(c, route, query, r.name, r.total) }

    private fun put(r: DownloadRow) { val i = rows.indexOfFirst { it.key == r.key }; if (i >= 0) rows[i] = r else rows.add(r) }
    private suspend fun show(r: DownloadRow) = withContext(Dispatchers.Main) { put(r) }

    private suspend fun run(c: Context, key: String, route: String, query: String, name: String, size: Long) {
        val dir = dirOf(c, key); val part = File(dir, "$name.part"); val tagFile = File(dir, "etag")
        val fail = { why: String -> DownloadRow(key, name, if (part.exists()) part.length() else 0, size, DownloadRow.State.FAIL, why) }
        val out = withContext(Dispatchers.IO) {
            dir.mkdirs(); sweep(c, dir)
            var tries = 0; var total = size
            while (isActive) {
                val link = DataClient.link ?: return@withContext fail(Msg.android_link_offline)
                val url = Pairing.http(link, route, query, token = false) ?: return@withContext fail(Msg.android_link_offline)
                val have = if (part.exists()) part.length() else 0L
                val tag = if (tagFile.exists()) tagFile.readText() else ""
                val ask = FileFetch.ask(have, tag)
                val req = Request.Builder().url(url).apply { Pairing.token(link)?.let { header("Authorization", "Bearer $it") }; ask.forEach { (k, v) -> header(k, v) } }.build()
                val from = if (ask.isEmpty()) 0L else have
                try {
                    val step = http.newCall(req).execute().use { r ->
                        val body = r.body
                        val st = FileFetch.step(r.code, from, r.header("Content-Range"), body?.contentLength() ?: -1, if (r.code in 200..299) "" else body?.string()?.take(400) ?: "")
                        if (st is FileFetch.Step.Write && body != null) {
                            r.header("ETag")?.let { tagFile.writeText(it) } ?: tagFile.delete()
                            total = if (st.total >= 0) st.total else if (body.contentLength() >= 0) st.from + body.contentLength() else -1
                            val before = if (st.from == 0L) 0L else part.length()
                            FileOutputStream(part, st.from > 0).use { o ->
                                val inp = body.byteStream(); val buf = ByteArray(BUF); var got = before; var shown = 0L
                                while (isActive) {
                                    val n = inp.read(buf); if (n < 0) break
                                    o.write(buf, 0, n); got += n
                                    if (got - shown >= (1 shl 20) || shown == 0L) { shown = got; show(DownloadRow(key, name, got, total, DownloadRow.State.RUN)) }
                                }
                            }
                            if (part.length() > before) tries = 0   // it moved on: a dropped connection starts its count again
                        }
                        st
                    }
                    when (step) {
                        is FileFetch.Step.Fail -> return@withContext fail(step.why)
                        FileFetch.Step.Again -> { part.delete(); tagFile.delete(); if (++tries >= FileFetch.TRIES) return@withContext fail(Msg.android_link_failed); continue }
                        FileFetch.Step.Done -> {}
                        is FileFetch.Step.Write -> if (!(FileFetch.complete(part.length(), total) || total < 0)) {
                            // the body ended short (the link dropped, the file changed): ask for the rest
                            if (++tries >= FileFetch.TRIES) return@withContext fail(Msg.android_link_failed); delay(FileFetch.backoffMs(tries)); continue
                        }
                    }
                    val done = File(dir, name); done.delete(); part.renameTo(done); tagFile.delete()
                    val saved = runCatching { saveToDownloads(c, done) }.isSuccess
                    return@withContext DownloadRow(key, name, done.length(), done.length(), DownloadRow.State.DONE, if (saved) Msg.android_download_saved else "", done)
                } catch (e: IOException) {
                    if (++tries >= FileFetch.TRIES) return@withContext fail(e.message ?: Msg.android_link_failed)
                    show(DownloadRow(key, name, if (part.exists()) part.length() else 0, total, DownloadRow.State.RUN))
                    delay(FileFetch.backoffMs(tries))
                }
            }
            fail(Msg.android_link_failed)
        }
        withContext(Dispatchers.Main) { put(out); jobs.remove(key) }
    }

    /** Older downloads in the cache go after a day (never the one running). */
    private fun sweep(c: Context, keep: File) {
        val day = System.currentTimeMillis() - 86_400_000L
        File(c.cacheDir, "downloads").listFiles()?.forEach { d -> if (d != keep && d.lastModified() < day) d.deleteRecursively() }
    }

    private fun mime(name: String) = MimeTypeMap.getSingleton().getMimeTypeFromExtension(name.substringAfterLast('.', "").lowercase()) ?: "application/octet-stream"

    /** A copy in the phone's Downloads: MediaStore from Android 10, the app's own Downloads folder before it. */
    private fun saveToDownloads(c: Context, f: File) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val cr = c.contentResolver
            val v = ContentValues().apply { put(MediaStore.Downloads.DISPLAY_NAME, f.name); put(MediaStore.Downloads.MIME_TYPE, mime(f.name)); put(MediaStore.Downloads.IS_PENDING, 1) }
            val u = cr.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, v) ?: throw IOException("no Downloads")
            try {
                cr.openOutputStream(u)?.use { o -> f.inputStream().use { it.copyTo(o, BUF) } } ?: throw IOException("no Downloads")
                cr.update(u, ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) }, null, null)
            } catch (e: Exception) { cr.delete(u, null, null); throw e }
        } else {
            val d = c.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS) ?: throw IOException("no Downloads")
            f.copyTo(File(d, f.name), overwrite = true)
        }
    }

    private fun uri(c: Context, f: File): Uri = androidx.core.content.FileProvider.getUriForFile(c, c.packageName + ".files", f)
    fun open(c: Context, r: DownloadRow) {
        val f = r.file ?: return
        val i = Intent(Intent.ACTION_VIEW).setDataAndType(uri(c, f), mime(f.name)).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        runCatching { c.startActivity(Intent.createChooser(i, f.name).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }.onFailure { DataClient.say(Msg.android_download_noapp) }
    }
    fun send(c: Context, r: DownloadRow) {
        val f = r.file ?: return
        val i = Intent(Intent.ACTION_SEND).setType(mime(f.name)).putExtra(Intent.EXTRA_STREAM, uri(c, f)).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        runCatching { c.startActivity(Intent.createChooser(i, f.name).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    }
}

private fun mb(n: Long) = when { n < 0 -> "?"; n < 1024 -> "$n B"; n < 1 shl 20 -> "${n / 1024} KB"; else -> String.format("%.1f MB", n / 1048576.0) }

/** The downloads under way and just done, over the screen's foot: each with its progress, then Open and Share, or the hub's words and Retry. */
@Composable
fun BoxScope.DownloadStrip() {
    val rows = Downloads.rows
    if (rows.isEmpty()) return
    val ctx = androidx.compose.ui.platform.LocalContext.current
    Column(Modifier.align(Alignment.BottomCenter).padding(bottom = 118.dp, start = 12.dp, end = 12.dp).fillMaxWidth().background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant)).testTag("downloads")) {
        rows.forEach { r ->
            Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp).testTag("download.${r.name}")) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(r.name, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, maxLines = 1)
                    when (r.state) {
                        DownloadRow.State.RUN -> { DMono(if (r.total > 0) Msg.android_download_of(mb(r.have), mb(r.total)) else mb(r.have)); Spacer(Modifier.width(10.dp)); DBtn(Msg.common_cancel, false, modifier = Modifier.testTag("download.cancel")) { Downloads.cancel(r.key) } }
                        DownloadRow.State.DONE -> {
                            DBtn(Msg.android_open, true, modifier = Modifier.testTag("download.open")) { Downloads.open(ctx, r) }; Spacer(Modifier.width(6.dp))
                            DBtn(Msg.android_download_share, false, modifier = Modifier.testTag("download.share")) { Downloads.send(ctx, r) }; Spacer(Modifier.width(6.dp))
                            DBtn("✕", false, modifier = Modifier.testTag("download.close")) { Downloads.dismiss(r.key) }
                        }
                        DownloadRow.State.FAIL -> {
                            DBtn(Msg.android_download_retry, false, modifier = Modifier.testTag("download.retry")) { Downloads.retry(r.key) }; Spacer(Modifier.width(6.dp))
                            DBtn("✕", false, modifier = Modifier.testTag("download.close")) { Downloads.dismiss(r.key) }
                        }
                    }
                }
                if (r.state == DownloadRow.State.RUN) Box(Modifier.fillMaxWidth().padding(top = 6.dp).height(3.dp).background(dcs.outlineVariant)) { Box(Modifier.fillMaxWidth(r.fill).fillMaxHeight().background(dcs.primary).testTag("download.bar")) }
                if (r.note.isNotEmpty()) DMono(Tr.t(r.note), if (r.state == DownloadRow.State.FAIL) dcs.error else dcs.onSurfaceVariant, 11, Modifier.padding(top = 4.dp), lines = 3)
            }
        }
    }
}
