package dev.backplane.mobile

import android.graphics.Bitmap
import android.graphics.Color as AColor
import android.graphics.pdf.PdfRenderer
import android.os.ParcelFileDescriptor
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.DeveloperBoard
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.InsertDriveFile
import androidx.compose.material.icons.filled.Memory
import androidx.compose.material.icons.filled.ViewInAr
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

// The viewer's Files tab (client.bend's Fm, mobile/view.bend's Files.json):
// the project's folders and files; a board, schematic or model opens in the
// viewer, an image or a PDF here (a PDF drawn page by page by Android's own
// PdfRenderer, after one download).

// a row's icon, by the name core/icons.bend gives it
private fun fileIcon(name: String): ImageVector = when (name) {
    "folder" -> Icons.Filled.Folder
    "circuit-board" -> Icons.Filled.DeveloperBoard
    "cpu" -> Icons.Filled.Memory
    "box" -> Icons.Filled.ViewInAr
    "image" -> Icons.Filled.Image
    "file-text" -> Icons.Filled.Description
    else -> Icons.Filled.InsertDriveFile
}

// the viewer's choices (Board, Schematic, 3D, Mech, Files) and its close button
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ViewerChoices(m: AppModel, v: Viewer) {
    val lang = LocalBackplaneLanguage.current
    Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        ChoiceRow(v.choices, v.choices.firstOrNull { it.on }?.value ?: v.open, Modifier.weight(1f)) { m.act("view", it) }
        IconButton(onClick = { m.act("view", "") }) { Icon(Icons.Filled.Close, NativeMessages.common_close(lang)) }
    }
}

@Composable
fun FilesScreen(m: AppModel, v: Viewer, f: FilesPage) {
    val lang = LocalBackplaneLanguage.current
    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface).statusBarsPadding().navigationBarsPadding()) {
        ViewerChoices(m, v)
        LazyRow(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            items(f.trail) { c ->
                TextButton(onClick = { m.act(c.action, c.value) }) {
                    Text(c.label, maxLines = 1, color = if (c == f.trail.last()) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.outline)
                }
            }
        }
        HorizontalDivider()
        when {
            !f.ready -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            f.rows.isEmpty() -> Text(NativeMessages.viewer_files_empty(lang), Modifier.padding(16.dp), color = MaterialTheme.colorScheme.outline)
            else -> LazyColumn(Modifier.fillMaxSize()) {
                items(f.rows, key = { it.value }) { r ->
                    val on = r.action.isNotEmpty()
                    val tint = if (r.kind == "folder") MaterialTheme.colorScheme.primary
                        else if (on) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.outline
                    Row(Modifier.fillMaxWidth().then(if (on) Modifier.clickable { m.act(r.action, r.value) } else Modifier)
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Icon(fileIcon(r.icon), null, Modifier.size(20.dp), tint = tint)
                        Text(r.label, Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis,
                            color = if (on) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.outline)
                        if (r.detail.isNotEmpty()) Text(r.detail, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                    }
                }
            }
        }
    }
}

// an image or a PDF opened from the Files tab, under its name and a way back
@Composable
fun FileScreen(m: AppModel, v: Viewer, f: FileView) {
    val lang = LocalBackplaneLanguage.current
    var shown by remember { mutableStateOf<String?>(null) }
    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface).statusBarsPadding().navigationBarsPadding()) {
        ViewerChoices(m, v)
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { m.act("fm") }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, NativeMessages.files_back_tip(lang)) }
            Text(f.name, Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        HorizontalDivider()
        if (f.kind == "pdf") PdfPages(m, f.url)
        else Box(Modifier.fillMaxSize().padding(12.dp), contentAlignment = Alignment.Center) {
            HubPicture(m, f.url, Modifier.fillMaxWidth()) { shown = it }
        }
    }
    shown?.let { u -> Lightbox(u) { shown = null } }
}

// a PDF downloaded once into the app's cache, then drawn a page at a time
// as the list reaches it (one renderer, used by one page at a time)
private class Pdf(file: File) {
    private val fd = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    private val r = PdfRenderer(fd)
    val pages: Int get() = r.pageCount

    @Synchronized
    fun page(i: Int, width: Int): Bitmap = r.openPage(i).use { p ->
        val b = Bitmap.createBitmap(width, maxOf(1, width * p.height / maxOf(p.width, 1)), Bitmap.Config.ARGB_8888)
        b.eraseColor(AColor.WHITE)
        p.render(b, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
        b
    }

    @Synchronized
    fun close() {
        r.close()
        fd.close()
    }
}

private sealed interface PdfGot {
    data class Failed(val kind: String, val code: String = "") : PdfGot
    data class Ok(val pdf: Pdf) : PdfGot
}

private fun fetchPdf(url: String, into: File): PdfGot = try {
    val c = URL(url).openConnection() as HttpURLConnection
    c.connectTimeout = 5000
    c.readTimeout = 60000
    try {
        if (c.responseCode != 200) PdfGot.Failed(if (c.responseCode == 404) "send" else "http", c.responseCode.toString())
        else {
            c.inputStream.use { i -> into.outputStream().use { i.copyTo(it) } }
            PdfGot.Ok(Pdf(into))
        }
    } finally {
        c.disconnect()
    }
} catch (e: Exception) {
    PdfGot.Failed("show")
}

@Composable
private fun PdfPages(m: AppModel, path: String) {
    val lang = LocalBackplaneLanguage.current
    val ctx = LocalContext.current
    val url = m.web(path) ?: return
    val got = produceState<PdfGot?>(null, url) {
        value = withContext(Dispatchers.IO) { fetchPdf(url, File(ctx.cacheDir, "viewer.pdf")) }
    }.value
    DisposableEffect(got) { onDispose { (got as? PdfGot.Ok)?.pdf?.close() } }
    when (got) {
        null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is PdfGot.Failed -> Text(NativeMessages.native_file_pdf_error(lang, got.kind, got.code), Modifier.padding(16.dp), color = MaterialTheme.colorScheme.outline)
        is PdfGot.Ok -> LazyColumn(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surfaceVariant),
            verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(got.pdf.pages) { i ->
                val b = produceState<Bitmap?>(null, got, i) {
                    value = withContext(Dispatchers.IO) { runCatching { got.pdf.page(i, 1240) }.getOrNull() }
                }.value
                Box(Modifier.fillMaxWidth().aspectRatio(b?.let { it.width.toFloat() / it.height } ?: 0.707f), contentAlignment = Alignment.Center) {
                    if (b == null) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                    else Image(b.asImageBitmap(), NativeMessages.native_file_page(lang, (i + 1).toString()), Modifier.fillMaxSize(), contentScale = ContentScale.Fit)
                }
            }
        }
    }
}
