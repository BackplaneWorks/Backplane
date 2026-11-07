package dev.backplane.mobile

import android.graphics.BitmapFactory
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.viewinterop.AndroidView
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject

// The viewer over a thread (the desktop's right rail): Board, Schematic, 3D, Mech, Files and Browser, on the data/1 viewer requests (native/host/src/data/ext.rs).
// Plots and models arrive as the hub's own binary frames and are drawn by the app's GPU view (PlotStore, PlotSurface: pinch, pan, orbit, tap to inspect).
private val tabs = listOf("Board", "Schematic", "3D", "Split", "Mech", "Files", "Diff", "Review", "Browser")

/** A file asked to be opened from the Files tab: the viewer jumps to its tab with the path. */
private data class Target(val tab: String, val path: String)

@Composable
fun ViewerScreen(thread: DThread, back: () -> Unit) {
    var tab by remember { mutableStateOf("Files") }
    var target by remember { mutableStateOf<Target?>(null) }
    var files by remember { mutableStateOf<DataClient.ProjectFiles?>(null) }
    val online = DataClient.conn == DataClient.Conn.ONLINE
    LaunchedEffect(thread.id, online) {
        if (files != null) return@LaunchedEffect
        files = runCatching { DataClient.viewerFiles(thread.id) }.getOrNull(); if (files?.pcb?.isNotEmpty() == true && tab == "Files") tab = "Board"
        DataClient.openViewerTab?.takeIf { it in tabs }?.let { tab = it; DataClient.openViewerTab = null }
    }
    DisposableEffect(thread.id) { onDispose { DataClient.stopWatch(thread.id) } }
    Column(Modifier.fillMaxSize()) {
        DBar("Viewer · ${thread.title.ifEmpty { "New thread" }}", back)
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).horizontalScroll(rememberScrollState())) {
            tabs.forEach { t ->
                DMono(t, if (t == tab) dcs.onPrimary else dcs.onSurface, 12, Modifier.background(if (t == tab) dcs.primary else dcs.surfaceVariant).clickable { tab = t; target = null }.padding(horizontal = 14.dp, vertical = 12.dp).testTag("vtab.$t"), bold = t == tab)
            }
        }
        DLine()
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (tab) {
                "Board" -> PlotTab(thread, "board", target?.path?.takeIf { target?.tab == "Board" } ?: "", files == null || files?.pcb.orEmpty().isNotEmpty() || target != null, "No board in this project yet")
                "Schematic" -> PlotTab(thread, "schematic", target?.path?.takeIf { target?.tab == "Schematic" } ?: "", files == null || files?.sch.orEmpty().isNotEmpty() || target != null, "No schematic in this project yet")
                "3D" -> PlotTab(thread, "3d", "", files == null || files?.pcb.orEmpty().isNotEmpty(), "No board in this project yet")
                "Split" -> SplitTab(thread, files)
                "Diff" -> DiffTab(thread)
                "Review" -> ReviewTab(thread)
                "Mech" -> MechTab(thread, target?.path?.takeIf { target?.tab == "Mech" })
                "Files" -> FilesTab(thread) { t -> target = t; tab = t.tab }
                else -> BrowserTab(thread)
            }
        }
    }
}

// ---- Board, Schematic and 3D: the hub's plot, drawn by the GPU view
@Composable
private fun PlotTab(thread: DThread, kind: String, path: String, has: Boolean, none: String) {
    val scope = rememberCoroutineScope()
    val three = kind == "3d"
    val head = if (three) "2|" else if (kind == "schematic") "1|" else "0|"
    var error by remember { mutableStateOf("") }
    var info by remember { mutableStateOf("") }
    var picked by remember { mutableStateOf("") }
    var layers by remember { mutableStateOf<DataClient.Layers?>(null) }
    var refs by remember { mutableStateOf<List<DataClient.Ref>?>(null) }
    var comparing by remember { mutableStateOf("") }
    var menu by remember { mutableStateOf(false) }
    LaunchedEffect(thread.id, kind, path, has, DataClient.conn == DataClient.Conn.ONLINE) {
        if (!has) return@LaunchedEffect
        error = ""; layers = null; refs = null; comparing = ""; info = ""; picked = ""
        runCatching { DataClient.watch(thread.id, kind, path) }.onFailure { error = it.message ?: "" }
    }
    val plots = DataClient.plots
    val frame = plots.frame?.takeIf { it.key.startsWith(if (three) "0|" else head) }
    val mesh = if (three) plots.mesh?.takeIf { it.key.startsWith("2|") } else null
    val bg = dcs.background.toArgb() and 0xFFFFFF
    val layersPresent = remember(frame) { frame?.chunks?.map { it.layer }?.distinct().orEmpty() }
    val bits = remember(layersPresent) { layersPresent.filter { it in 0..30 }.fold(0) { a, l -> a or (1 shl l) } }
    // a press: the hub answers what it does; a comparison on show keeps its marks to the active layer
    val act = { action: String, value: String -> scope.launch {
        val r = runCatching { DataClient.layers(kind, bits, layers?.own ?: 0, layers?.focus.orEmpty(), action, value) }.getOrNull() ?: return@launch
        val moved = (layers?.bits ?: 0) != r.bits
        layers = r
        if (moved && comparing.isNotEmpty()) runCatching { DataClient.compare(thread.id, kind, comparing, r.bits, path) }
        Unit
    }; Unit }
    LaunchedEffect(kind, path, bits) { if (!three && bits != 0 && DataClient.has("layers")) act("", "") }
    val off = layers?.off ?: 0; val dim = layers?.dim ?: 0; val lift = layers?.lift ?: 255
    Box(Modifier.fillMaxSize().background(dcs.background)) {
        AndroidView(factory = { PlotSurface(it) }, modifier = Modifier.fillMaxSize().testTag("viewer.surface"), update = { s ->
            s.bindSource(thread.id + "\u0000" + kind + "\u0000" + path)
            s.setThree(three)
            // what lies under the finger comes as candidates `[{"c": chunk, "p": piece, "a": area, "l": layer}]`; the smallest is what a tap means (a pad over its copper)
            s.onPick = { p ->
                val c = runCatching { org.json.JSONArray(p) }.getOrNull()
                val best = c?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.minByOrNull { it.optLong("a") } }
                if (best == null || frame == null) { info = ""; picked = ""; s.mark("") }
                else { val at = best.optInt("c"); val idx = best.optInt("p"); picked = "$at,$idx"; s.mark(picked)
                    scope.launch { info = runCatching { DataClient.info(frame.key, at, idx) }.getOrDefault("").ifEmpty { "piece $at,$idx" } } }
            }
            if (frame != null && frame.none.isEmpty()) s.show(frame, bg, 0x1f6f43, IntArray(0))
            s.mark(picked) // closing the card deselects
            if (three) s.mesh(mesh)
            if (s.renderer.off != off || s.renderer.dim != dim || s.renderer.lift != lift) { s.renderer.off = off; s.renderer.dim = dim; s.renderer.lift = lift; s.requestRender() }
        })
        val say = when {
            !has -> none
            error.isNotEmpty() -> error
            frame != null && frame.none.isNotEmpty() -> frame.none
            three && mesh != null && !mesh.decoding && mesh.none.isNotEmpty() -> mesh.none
            else -> ""
        }
        if (say.isNotEmpty() && three && mesh?.mesh != null) DMono(say, dcs.onSurfaceVariant, 11, Modifier.align(Alignment.TopStart).padding(8.dp).background(dcs.surfaceVariant.copy(alpha = 0.85f)).padding(8.dp).testTag("viewer.note"))
        else if (say.isNotEmpty()) DMono(say, dcs.onSurfaceVariant, 12, Modifier.align(Alignment.Center).padding(24.dp))
        else if (three && (mesh?.mesh == null || mesh.decoding)) Loading("Loading 3D model…", Modifier.align(Alignment.Center))
        else if (frame == null && !three) CircularProgressIndicator(Modifier.align(Alignment.Center))
        val l = layers
        if (!three && layersPresent.isNotEmpty() && l != null) {
            Row(Modifier.align(Alignment.TopStart).padding(8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                ViewChip(l.head, l.mode > 0, "viewer.layers") { refs = null; menu = !menu }
                if (comparing.isEmpty()) ViewChip("Compare", refs != null, "viewer.compare") { menu = false; if (refs != null) refs = null else scope.launch { refs = runCatching { DataClient.refs(thread.id) }.getOrDefault(emptyList()) } }
                else ViewChip("Close compare", true, "viewer.compare.close") { comparing = ""; scope.launch { runCatching { DataClient.watch(thread.id, kind, path) }.onFailure { error = it.message ?: "" } } }
            }
            if (menu) LayerMenu(l, layersPresent, act, Modifier.align(Alignment.TopStart).padding(top = 44.dp, start = 8.dp))
            refs?.let { rows -> RefMenu(rows, { c -> refs = null; scope.launch { runCatching { DataClient.compare(thread.id, kind, c, l.bits, path); comparing = c }.onFailure { error = it.message ?: "" } } }, Modifier.align(Alignment.TopStart).padding(top = 44.dp, start = 8.dp)) }
        }
        if (info.isNotEmpty()) Column(Modifier.align(Alignment.BottomCenter).fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(10.dp).testTag("viewer.info")) {
            Row { DMono("PICKED", dcs.primary, 11, Modifier.weight(1f), bold = true); DMono("close", dcs.onSurfaceVariant, 11, Modifier.clickable { info = ""; picked = "" }.testTag("viewer.info.close")) }
            Text(info.replace("\u0000", "  ").replace(";", "\n").trim(), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, maxLines = 8)
        }
    }
}

@Composable
private fun ViewChip(label: String, on: Boolean, tag: String, onClick: () -> Unit) {
    DMono(label, if (on) dcs.primary else dcs.onSurface, 12, Modifier.background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, if (on) dcs.primary else dcs.outlineVariant)).clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 6.dp).testTag(tag))
}

/** KiCad's high contrast, as the hub lists it: how the others show (a board), a box per layer (show or hide it), its name (make it the active one, again: all back), a comparison's marks last, Reset layers. */
@Composable
private fun LayerMenu(l: DataClient.Layers, present: List<Int>, act: (String, String) -> Unit, modifier: Modifier) {
    Column(modifier.heightIn(max = 420.dp).verticalScroll(rememberScrollState()).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(8.dp).testTag("layer.menu"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (l.modes) Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            listOf("All", "Dim", "Only").forEachIndexed { i, label ->
                DMono(label, if (l.mode == i) dcs.primary else dcs.onSurface, 12, Modifier.border(BorderStroke(1.dp, if (l.mode == i) dcs.primary else dcs.outlineVariant)).clickable { act("mode", i.toString()) }.padding(horizontal = 8.dp, vertical = 3.dp).testTag("layer.mode.$i"))
            }
        }
        l.rows.filter { it.layer in present }.forEach { r ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(14.dp).border(BorderStroke(1.dp, dcs.outlineVariant)).background(if (r.on) dcs.primary else dcs.surfaceVariant).clickable { act("toggle", r.layer.toString()) }.testTag("layer.box.${r.layer}"))
                Spacer(Modifier.width(10.dp))
                DMono(r.name, if (r.active) dcs.primary else dcs.onSurface, 12, Modifier.clickable { act("pick", r.layer.toString()) }.testTag("layer.name.${r.layer}"))
            }
        }
        if (l.reset) DMono("Reset layers", dcs.primary, 12, Modifier.clickable { act("reset", "") }.testTag("layer.reset"))
    }
}

/** The versions a comparison may take, as the hub lists them; a press shows the Diff against that one. */
@Composable
private fun RefMenu(rows: List<DataClient.Ref>, pick: (String) -> Unit, modifier: Modifier) {
    Column(modifier.heightIn(max = 420.dp).verticalScroll(rememberScrollState()).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(8.dp).testTag("compare.menu"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (rows.isEmpty()) DMono("No versions to compare with", dcs.onSurfaceVariant, 12)
        rows.forEachIndexed { i, r ->
            Row(Modifier.clickable { pick(r.commit) }.testTag("compare.row.$i"), horizontalArrangement = Arrangement.spacedBy(8.dp)) { DMono(r.kind, dcs.onSurfaceVariant, 12); DMono(r.label, dcs.onSurface, 12) }
        }
    }
}

// ---- Mech: the project's models, one at a time
@Composable
private fun MechTab(thread: DThread, open: String?) {
    var mech by remember { mutableStateOf<DataClient.Mech?>(null) }
    var part by remember(open) { mutableStateOf(open) }
    LaunchedEffect(thread.id) { mech = runCatching { DataClient.mech(thread.id) }.getOrNull() }
    val m = mech
    Column(Modifier.fillMaxSize()) {
        when {
            m == null -> CircularProgressIndicator(Modifier.padding(24.dp))
            m.parts.isEmpty() -> DMono("No models in mech/ yet", dcs.onSurfaceVariant, 12, Modifier.padding(24.dp))
            else -> {
                Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).horizontalScroll(rememberScrollState()).padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    m.parts.forEach { p -> DMono(p.substringAfterLast('/'), if (p == part) dcs.onPrimary else dcs.onSurface, 12, Modifier.background(if (p == part) dcs.primary else dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant)).clickable { part = p }.padding(horizontal = 10.dp, vertical = 6.dp).testTag("mech.part.${p.substringAfterLast('/')}")) }
                }
                if (part == null) LaunchedEffect(m) { part = m.parts.first() }
                part?.let { MechSolid(thread, it) }
            }
        }
    }
}
@Composable
private fun MechSolid(thread: DThread, path: String) {
    var error by remember { mutableStateOf("") }
    LaunchedEffect(thread.id, path) { error = ""; runCatching { DataClient.watch(thread.id, "3d", path) }.onFailure { error = it.message ?: "" } }
    val mesh = DataClient.plots.mesh?.takeIf { it.key.endsWith("/" + path) }   // the part asked for, not the board's model of the 3D tab
    val bgArgb = dcs.background.toArgb()
    Box(Modifier.fillMaxSize().background(dcs.background)) {
        AndroidView(factory = { PlotSurface(it) }, modifier = Modifier.fillMaxSize().testTag("mech.surface"), update = { s ->
            s.bindSource(thread.id + "\u0000mech\u0000" + path); s.setThree(true)
            val c = bgArgb; s.renderer.bg = floatArrayOf(((c shr 16) and 255) / 255f, ((c shr 8) and 255) / 255f, (c and 255) / 255f)
            s.mesh(mesh)
        })
        val say = if (error.isNotEmpty()) error else mesh?.takeIf { !it.decoding }?.none.orEmpty()
        if (say.isNotEmpty()) DMono(say, dcs.onSurfaceVariant, 12, Modifier.align(Alignment.Center).padding(24.dp))
        else if (mesh?.mesh == null || mesh.decoding) Loading("Loading 3D model…", Modifier.align(Alignment.Center))
    }
}

/** A model on its way: a spinner and what it waits for, over the board's plain slab while the parts come (never a blank view) */
@Composable
private fun Loading(text: String, modifier: Modifier) {
    Column(modifier.background(dcs.surfaceVariant.copy(alpha = 0.85f)).padding(16.dp).testTag("viewer.loading"), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        CircularProgressIndicator()
        DMono(text, dcs.onSurfaceVariant, 12)
    }
}

// ---- Files: browse the project, preview what a phone can show, open a board, schematic or model in its tab
private val imageExt = setOf("png", "jpg", "jpeg", "gif", "webp", "bmp")
private val textExt = setOf("txt", "md", "csv", "json", "log", "yaml", "yml", "toml", "kicad_pro", "sh", "rs", "py", "js", "ts", "kt", "swift", "bend", "cfg", "ini")
private fun ext(name: String) = name.substringAfterLast('.', "").lowercase()

@Composable
private fun FilesTab(thread: DThread, open: (Target) -> Unit) {
    var dir by remember { mutableStateOf("") }
    var list by remember { mutableStateOf<List<DataClient.Entry>?>(null) }
    var err by remember { mutableStateOf("") }
    var preview by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(thread.id, dir) { list = null; err = ""; runCatching { DataClient.entries(thread.id, dir) }.onSuccess { list = it }.onFailure { err = it.message ?: "" } }
    if (preview != null) { FilePreview(thread, preview!!) { preview = null }; return }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (dir.isNotEmpty()) DMono("..", dcs.primary, 13, Modifier.clickable { dir = dir.substringBeforeLast('/', "") }.padding(end = 12.dp).testTag("files.up"), bold = true)
            DMono("/" + dir, dcs.onSurface, 12)
        }
        DLine()
        if (err.isNotEmpty()) DMono(err, dcs.error, 12, Modifier.padding(12.dp))
        val l = list
        if (l == null && err.isEmpty()) CircularProgressIndicator(Modifier.padding(24.dp))
        l?.let { es ->
            if (es.isEmpty()) DMono("empty", dcs.onSurfaceVariant, 12, Modifier.padding(12.dp))
            LazyColumn(Modifier.fillMaxSize().testTag("files.list")) {
                items(es.sortedWith(compareBy({ !it.dir }, { it.name.lowercase() })), key = { it.name }) { e ->
                    val full = if (dir.isEmpty()) e.name else "$dir/${e.name}"
                    Column(Modifier.fillMaxWidth().clickable {
                        if (e.dir) dir = full else when (val x = ext(e.name)) {
                            "kicad_pcb" -> open(Target("Board", full)); "kicad_sch" -> open(Target("Schematic", full))
                            "step", "stp", "glb" -> open(Target("Mech", full))
                            else -> preview = full
                        }
                    }.testTag("file.${e.name}")) {
                        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                            androidx.compose.material3.Icon(if (e.dir) Icons.Default.Folder else Icons.Default.Description, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)); Spacer(Modifier.width(10.dp))
                            Text(e.name, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp, maxLines = 1)
                            if (!e.dir) DMono(size(e.size))
                        }
                        DLine()
                    }
                }
            }
        }
    }
}
private fun size(n: Long) = when { n < 1024 -> "$n B"; n < 1 shl 20 -> "${n / 1024} KB"; else -> "${n shr 20} MB" }

@Composable
private fun FilePreview(thread: DThread, path: String, close: () -> Unit) {
    var bytes by remember(path) { mutableStateOf<ByteArray?>(null) }
    var failed by remember(path) { mutableStateOf(false) }
    LaunchedEffect(path) { bytes = DataClient.fileBytes(thread.id, path); failed = bytes == null }
    androidx.activity.compose.BackHandler(onBack = close)
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            DMono("‹ " + path.substringAfterLast('/'), dcs.primary, 13, Modifier.clickable(onClick = close).testTag("preview.back"), bold = true)
        }
        DLine()
        val b = bytes
        when {
            failed -> DMono("This file could not be read.", dcs.error, 12, Modifier.padding(16.dp))
            b == null -> CircularProgressIndicator(Modifier.padding(24.dp))
            ext(path) in imageExt -> {
                val bmp = remember(b) { BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap() }
                if (bmp != null) Image(bmp, null, Modifier.fillMaxSize().padding(8.dp).testTag("preview.image"), contentScale = ContentScale.Fit) else DMono("Not a picture this phone can show.", dcs.onSurfaceVariant, 12, Modifier.padding(16.dp))
            }
            ext(path) == "pdf" -> PdfPreview(b)
            ext(path) in textExt || b.size < 200_000 && b.none { it.toInt() == 0 } -> {
                val text = remember(b) { String(b, Charsets.UTF_8).take(200_000) }
                Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(12.dp).testTag("preview.text")) {
                    if (ext(path) == "md") MdText(text) else Text(text, color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp)
                }
            }
            else -> DMono("No preview for this file (${size(b.size.toLong())}).", dcs.onSurfaceVariant, 12, Modifier.padding(16.dp))
        }
    }
}

@Composable
private fun PdfPreview(b: ByteArray) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    var page by remember(b) { mutableIntStateOf(0) }
    val file = remember(b) { java.io.File(ctx.cacheDir, "preview.pdf").also { it.writeBytes(b) } }
    val r = remember(b) { runCatching { android.graphics.pdf.PdfRenderer(android.os.ParcelFileDescriptor.open(file, android.os.ParcelFileDescriptor.MODE_READ_ONLY)) }.getOrNull() }
    DisposableEffect(b) { onDispose { runCatching { r?.close() } } }
    if (r == null) { DMono("This PDF could not be opened.", dcs.error, 12, Modifier.padding(16.dp)); return }
    val bmp = remember(b, page) { r.openPage(page).use { p -> val w = 1080; val h = (w * p.height.toFloat() / p.width).toInt()
        android.graphics.Bitmap.createBitmap(w, h, android.graphics.Bitmap.Config.ARGB_8888).also { it.eraseColor(android.graphics.Color.WHITE); p.render(it, null, null, android.graphics.pdf.PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY) } }.asImageBitmap() }
    Column(Modifier.fillMaxSize().testTag("preview.pdf")) {
        Row(Modifier.fillMaxWidth().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            DBtn("‹", false, page > 0) { page-- }; DMono("page ${page + 1} of ${r.pageCount}", dcs.onSurface, 12); DBtn("›", false, page + 1 < r.pageCount) { page++ }
        }
        Image(bmp, null, Modifier.fillMaxWidth().verticalScroll(rememberScrollState()), contentScale = ContentScale.FillWidth)
    }
}

// ---- Browser: the page's newest frame; a tap is a click once the person has the page
@Composable
private fun BrowserTab(thread: DThread) {
    val scope = rememberCoroutineScope()
    var frame by remember { mutableStateOf<androidx.compose.ui.graphics.ImageBitmap?>(null) }
    var none by remember { mutableStateOf(false) }
    var driving by remember { mutableStateOf(false) }
    var url by remember { mutableStateOf("") }
    var err by remember { mutableStateOf("") }
    var size by remember { mutableStateOf(IntSize.Zero) }
    suspend fun load() { val b = DataClient.screenBytes(thread.id); if (b == null) none = frame == null else { frame = BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap(); none = false } }
    LaunchedEffect(thread.id) { while (true) { load(); delay(2000) } }
    LaunchedEffect(DataClient.shot) { if (DataClient.shotBot == thread.id) load() }
    fun run(op: String, extra: JSONObject.() -> Unit = {}) = scope.launch { err = ""; runCatching { DataClient.browser(thread.id, op, extra) }.onFailure { err = it.message ?: "" } }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (!driving) DBtn("Take over", true, modifier = Modifier.testTag("browser.takeover")) { driving = true; run("takeover") }
            else {
                DBtn("‹", false, modifier = Modifier.testTag("browser.back")) { run("toolbar") { put("command", "back") } }
                Box(Modifier.weight(1f)) { DField(url, { url = it }, "Address", Modifier.testTag("browser.url")) }
                DBtn("Go", true, url.isNotBlank(), Modifier.testTag("browser.go")) { run("navigate") { put("url", pageUrl(url)) } }
                DBtn("Hand back", false, modifier = Modifier.testTag("browser.handback")) { driving = false; run("handback") }
            }
        }
        if (err.isNotEmpty()) DMono(err, dcs.error, 12, Modifier.padding(horizontal = 12.dp, vertical = 4.dp))
        DLine()
        Box(Modifier.weight(1f).fillMaxWidth().background(dcs.background), contentAlignment = Alignment.Center) {
            val f = frame
            when {
                f != null -> Image(f, null, Modifier.fillMaxWidth().onSizeChanged { size = it }.testTag("browser.frame").pointerInput(driving) {
                    detectTapGestures { p -> if (driving && size.width > 0 && size.height > 0) run("click") { put("x", (p.x * 1000 / size.width).toInt().coerceIn(0, 1000)); put("y", (p.y * 1000 / size.height).toInt().coerceIn(0, 1000)) } }
                }, contentScale = ContentScale.FillWidth)
                none -> DMono("No browser page yet. An agent opens one when it needs the web.", dcs.onSurfaceVariant, 12, Modifier.padding(24.dp))
                else -> CircularProgressIndicator()
            }
        }
        if (driving) DMono("You have the page: a tap clicks. Hand it back when you are done.", dcs.onSurfaceVariant, 11, Modifier.padding(8.dp))
    }
}


// ---- Split: the schematic, the board and its 3D model at once (stacked in portrait, side by side in landscape)
@Composable
private fun SplitTab(thread: DThread, files: DataClient.ProjectFiles?) {
    val root = files?.root.orEmpty()
    LaunchedEffect(thread.id, root) { if (root.isNotEmpty()) runCatching { DataClient.watch(thread.id, "split") } }
    val plots = DataClient.plots
    val seen = plots.frame to plots.mesh   // the store changes on each frame: read it so the panes follow
    val sch = plots.cachedFrame("1|$root|"); val brd = plots.cachedFrame("0|$root|"); val mdl = plots.cachedMesh("2|$root|")
    val bg = dcs.background.toArgb() and 0xFFFFFF
    BoxWithConstraints(Modifier.fillMaxSize().testTag("split")) {
        val panes: @Composable (Modifier) -> Unit = { m ->
            SplitPane("Schematic", thread.id + "|split|sch", sch, null, false, bg, if (root.isEmpty() || files?.sch.isNullOrEmpty()) "No schematic in this project yet" else "", m)
            SplitPane("Board", thread.id + "|split|brd", brd, null, false, bg, if (files?.pcb.isNullOrEmpty()) "No board in this project yet" else "", m)
            SplitPane("3D", thread.id + "|split|3d", brd, mdl, true, bg, if (files?.pcb.isNullOrEmpty()) "No board in this project yet" else "", m)
        }
        if (maxWidth > maxHeight) Row(Modifier.fillMaxSize()) { panes(Modifier.weight(1f).fillMaxHeight()) }
        else Column(Modifier.fillMaxSize()) { panes(Modifier.weight(1f).fillMaxWidth()) }
    }
}
@Composable
private fun SplitPane(label: String, source: String, frame: PlotFrame?, mesh: MeshFrame?, three: Boolean, bg: Int, none: String, modifier: Modifier) {
    Box(modifier.border(BorderStroke(1.dp, dcs.outlineVariant)).background(dcs.background)) {
        AndroidView(factory = { PlotSurface(it) }, modifier = Modifier.fillMaxSize().testTag("split.$label"), update = { s ->
            s.bindSource(source); s.setThree(three)
            if (frame != null && frame.none.isEmpty()) s.show(frame, bg, 0x1f6f43, IntArray(0))
            if (three) s.mesh(mesh)
        })
        DMono(label.uppercase(), dcs.onSurfaceVariant, 10, Modifier.align(Alignment.TopStart).padding(6.dp))
        val say = if (none.isNotEmpty()) none else frame?.none.orEmpty()
        if (say.isNotEmpty()) DMono(say, dcs.onSurfaceVariant, 11, Modifier.align(Alignment.Center).padding(12.dp))
        else if (frame == null && (!three || mesh == null)) CircularProgressIndicator(Modifier.align(Alignment.Center).size(24.dp))
    }
}

// ---- Diff: the thread's working changes, unified, + green and - red
@Composable
private fun DiffTab(thread: DThread) {
    var diff by remember { mutableStateOf<String?>(null) }
    var nogit by remember { mutableStateOf("") }
    var err by remember { mutableStateOf("") }
    var n by remember { mutableIntStateOf(0) }
    LaunchedEffect(thread.id, n) { diff = null; err = ""; runCatching { DataClient.diff(thread.id) }.onSuccess { diff = it.first; nogit = it.second }.onFailure { err = it.message ?: "" } }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            DMono("CHANGES", dcs.onSurfaceVariant, 11, Modifier.weight(1f).padding(start = 4.dp)); DBtn("Refresh", false, modifier = Modifier.testTag("diff.refresh")) { n++ }
        }
        DLine()
        val d = diff
        when {
            err.isNotEmpty() -> DMono(err, dcs.error, 12, Modifier.padding(16.dp))
            d == null -> CircularProgressIndicator(Modifier.padding(24.dp))
            nogit.isNotEmpty() -> DMono("This project is not a git repository, so there is nothing to compare.", dcs.onSurfaceVariant, 12, Modifier.padding(16.dp).testTag("diff.nogit"))
            d.isBlank() -> DMono("No changes.", dcs.onSurfaceVariant, 12, Modifier.padding(16.dp).testTag("diff.none"))
            else -> {
                val green = androidx.compose.ui.graphics.Color(0xFF4CAF7A); val lines = remember(d) { d.lines().take(4000) }
                LazyColumn(Modifier.fillMaxSize().horizontalScroll(rememberScrollState()).testTag("diff.list")) {
                    items(lines.size) { i ->
                        val l = lines[i]
                        val (c, bgc) = when {
                            l.startsWith("+++") || l.startsWith("---") || l.startsWith("diff ") || l.startsWith("index ") -> dcs.onSurface to dcs.surfaceVariant
                            l.startsWith("+") -> green to green.copy(alpha = .12f)
                            l.startsWith("-") -> dcs.error to dcs.error.copy(alpha = .12f)
                            l.startsWith("@@") -> dcs.primary to androidx.compose.ui.graphics.Color.Transparent
                            else -> dcs.onSurfaceVariant to androidx.compose.ui.graphics.Color.Transparent
                        }
                        Text(l.ifEmpty { " " }, Modifier.fillMaxWidth().background(bgc).padding(horizontal = 8.dp, vertical = 1.dp), color = c, fontFamily = BpMono, fontSize = 11.sp, softWrap = false, fontWeight = if (l.startsWith("diff ")) FontWeight.Bold else FontWeight.Normal)
                    }
                }
            }
        }
    }
}

// ---- Review: the BOM, the design checks (with the checklist's boxes) and the thread's comments
@Composable
private fun ReviewTab(thread: DThread) {
    var page by remember { mutableStateOf("Checks") }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 8.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("Checks", "BOM", "Comments").forEach { p -> DBtn(p, page == p, modifier = Modifier.testTag("review.page.$p")) { page = p } }
        }
        DLine()
        when (page) { "BOM" -> ReportPage(thread, "bom", false); "Comments" -> CommentsPage(thread); else -> ReportPage(thread, "checks", true) }
    }
}
@Composable
private fun ReportPage(thread: DThread, kind: String, ticks: Boolean) {
    val scope = rememberCoroutineScope()
    var rep by remember(kind) { mutableStateOf<DataClient.Report?>(null) }
    var tasks by remember(kind) { mutableStateOf<List<DataClient.Task>>(emptyList()) }
    var err by remember(kind) { mutableStateOf("") }
    var run by remember(kind) { mutableIntStateOf(0) }
    LaunchedEffect(thread.id, kind, run) { rep = null; err = ""; runCatching { DataClient.reviewPage(thread.id, kind) }.onSuccess { rep = it; tasks = it.tasks }.onFailure { err = it.message ?: "" } }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(12.dp).testTag("review.$kind"), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        DBtn(if (ticks) "Run checks" else "Refresh", false, modifier = Modifier.testTag("review.run")) { run++ }
        val r = rep
        if (err.isNotEmpty()) DMono(err, dcs.error, 12)
        if (r == null && err.isEmpty()) CircularProgressIndicator()
        r?.let {
            if (it.line.isNotEmpty()) DMono(it.line, dcs.primary, 12, bold = true)
            if (it.gate.isNotEmpty()) DMono(it.gate, dcs.onSurfaceVariant, 11)
            if (it.err.isNotEmpty()) DMono(it.err, dcs.error, 12)
            if (it.md.isNotEmpty()) MdText(it.md)
            if (ticks) tasks.forEachIndexed { i, t ->
                Row(Modifier.fillMaxWidth().clickable { scope.launch { runCatching { DataClient.tick(thread.id, i) }.onSuccess { n -> if (n.isNotEmpty()) tasks = n } } }.padding(vertical = 6.dp).testTag("review.task.$i"), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(16.dp).border(BorderStroke(1.dp, dcs.outlineVariant)).background(if (t.done) dcs.primary else dcs.surfaceVariant)); Spacer(Modifier.width(10.dp))
                    Text(t.text, color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp)
                }
            }
            if (it.md.isEmpty() && it.line.isEmpty() && it.err.isEmpty() && tasks.isEmpty()) DMono("Nothing to show.", dcs.onSurfaceVariant, 12)
        }
    }
}
@Composable
private fun CommentsPage(thread: DThread) {
    val scope = rememberCoroutineScope()
    var list by remember { mutableStateOf<List<DataClient.Comment>?>(null) }
    var draft by remember { mutableStateOf("") }
    var err by remember { mutableStateOf("") }
    suspend fun load() { runCatching { DataClient.comments(thread.id) }.onSuccess { list = it }.onFailure { err = it.message ?: "" } }
    LaunchedEffect(thread.id) { load() }
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(12.dp).testTag("review.comments"), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            if (err.isNotEmpty()) DMono(err, dcs.error, 12)
            val l = list
            if (l == null) CircularProgressIndicator() else if (l.isEmpty()) DMono("No comments yet. Add one below; the agent reads the open ones with its next message.", dcs.onSurfaceVariant, 12)
            l?.filter { it.parent.isEmpty() }?.forEach { c ->
                Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, if (c.resolved) dcs.outlineVariant else dcs.primary.copy(alpha = .6f))).padding(10.dp)) {
                    Row { DMono((c.by.ifEmpty { "you" }) + if (c.resolved) " · resolved" else "", dcs.onSurfaceVariant, 11, Modifier.weight(1f))
                        DMono(if (c.resolved) "reopen" else "resolve", dcs.primary, 11, Modifier.clickable { scope.launch { runCatching { DataClient.comment(thread.id, if (c.resolved) "reopen" else "resolve") { put("id", c.id) } }; load() } }.testTag("review.resolve.${c.id}")) }
                    Text(c.text, color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp, modifier = Modifier.padding(top = 4.dp))
                    l.filter { it.parent == c.id }.forEach { r -> Text("↳ " + r.text, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, modifier = Modifier.padding(top = 4.dp)) }
                }
            }
        }
        DLine()
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(8.dp), verticalAlignment = Alignment.Bottom) {
            Box(Modifier.weight(1f)) { DField(draft, { draft = it }, "A review comment", Modifier.testTag("review.composer"), 4) }
            Spacer(Modifier.width(8.dp))
            DBtn("Add", true, draft.isNotBlank(), Modifier.testTag("review.add")) { val t = draft; draft = ""; scope.launch { runCatching { DataClient.comment(thread.id, "add") { put("text", t) } }.onFailure { err = it.message ?: "" }; load() } }
        }
    }
}

/** An address as typed: with a scheme it is as it is; a local or numeric host (localhost, 127.x, 10.x, 192.168.x, host:port) is http, anything else https. */
internal fun pageUrl(t: String): String {
    val u = t.trim(); if ("://" in u) return u
    val host = u.substringBefore('/')
    val local = host.startsWith("localhost") || Regex("^(127|10)\\.[0-9.]+(:[0-9]+)?$|^192\\.168\\.[0-9.]+(:[0-9]+)?$").matches(host) || Regex(":[0-9]+$").containsMatchIn(host)
    return (if (local) "http://" else "https://") + u
}
