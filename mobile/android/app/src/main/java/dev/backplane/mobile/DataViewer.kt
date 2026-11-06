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
import androidx.compose.ui.viewinterop.AndroidView
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject

// The viewer over a thread (the desktop's right rail): Board, Schematic, 3D, Mech, Files and Browser, on the data/1 viewer requests (native/host/src/data/ext.rs).
// Plots and models arrive as the hub's own binary frames and are drawn by the app's GPU view (PlotStore, PlotSurface: pinch, pan, orbit, tap to inspect).
private val tabs = listOf("Board", "Schematic", "3D", "Mech", "Files", "Browser")
private val boardLayers = listOf(12, 11, 10, 9, 8, 7, 6, 5, 13, 23, 24, 15, 2, 16, 3, 14, 4, 19, 20, 22, 21, 17, 1, 18, 0)
private val boardNames = listOf("B.Fab", "B.Courtyard", "B.Paste", "B.Silkscreen", "B.Mask", "B.Cu", "In6.Cu", "In5.Cu", "In4.Cu", "In3.Cu", "In2.Cu", "In1.Cu", "F.Cu", "Pads", "F.Mask", "F.Paste", "F.Silkscreen", "F.Courtyard", "F.Fab", "User.Drawings", "User.Comments", "Margin", "Edge.Cuts", "Vias", "Holes")
private val schNames = listOf("page", "sheet fill", "body fill", "sheet", "graphic", "note", "body", "pin", "pin name", "pin number", "field", "field other", "bus", "bus entry", "wire", "no connect", "junction", "label", "global label", "hier label", "sheet name", "sheet file", "sheet pin", "dnp")

/** A file asked to be opened from the Files tab: the viewer jumps to its tab with the path. */
private data class Target(val tab: String, val path: String)

@Composable
fun ViewerScreen(thread: DThread, back: () -> Unit) {
    var tab by remember { mutableStateOf("Files") }
    var target by remember { mutableStateOf<Target?>(null) }
    var files by remember { mutableStateOf<DataClient.ProjectFiles?>(null) }
    LaunchedEffect(thread.id) { files = runCatching { DataClient.viewerFiles(thread.id) }.getOrNull(); if (files?.pcb?.isNotEmpty() == true) tab = "Board" }
    DisposableEffect(thread.id) { onDispose { DataClient.stopWatch(thread.id) } }
    Column(Modifier.fillMaxSize()) {
        DBar("Viewer · ${thread.title.ifEmpty { "New thread" }}", back)
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).horizontalScroll(rememberScrollState())) {
            tabs.forEach { t ->
                DMono(t, if (t == tab) dcs.primary else dcs.onSurfaceVariant, 12, Modifier.clickable { tab = t; target = null }.padding(horizontal = 14.dp, vertical = 12.dp).testTag("vtab.$t"), bold = t == tab)
            }
        }
        DLine()
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (tab) {
                "Board" -> PlotTab(thread, "board", target?.path?.takeIf { target?.tab == "Board" } ?: "", files?.pcb.orEmpty().isNotEmpty() || target != null, "No board in this project yet")
                "Schematic" -> PlotTab(thread, "schematic", target?.path?.takeIf { target?.tab == "Schematic" } ?: "", files?.sch.orEmpty().isNotEmpty() || target != null, "No schematic in this project yet")
                "3D" -> PlotTab(thread, "3d", "", files?.pcb.orEmpty().isNotEmpty(), "No board in this project yet")
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
    var off by remember { mutableIntStateOf(0) }; var dim by remember { mutableIntStateOf(0) }; var lift by remember { mutableIntStateOf(255) }
    var menu by remember { mutableStateOf(false) }
    LaunchedEffect(thread.id, kind, path, has) {
        if (!has) return@LaunchedEffect
        error = ""; off = 0; dim = 0; lift = 255; info = ""
        runCatching { DataClient.watch(thread.id, kind, path) }.onFailure { error = it.message ?: "" }
    }
    val plots = DataClient.plots
    val frame = plots.frame?.takeIf { it.key.startsWith(if (three) "0|" else head) }
    val mesh = if (three) plots.mesh?.takeIf { it.key.startsWith("2|") } else null
    val bg = dcs.background.toArgb() and 0xFFFFFF
    val layersPresent = remember(frame) { frame?.chunks?.map { it.layer }?.distinct().orEmpty() }
    Box(Modifier.fillMaxSize().background(dcs.background)) {
        AndroidView(factory = { PlotSurface(it) }, modifier = Modifier.fillMaxSize().testTag("viewer.surface"), update = { s ->
            s.bindSource(thread.id + "\u0000" + kind + "\u0000" + path)
            s.setThree(three)
            // what lies under the finger comes as candidates `[{"c": chunk, "p": piece, "a": area, "l": layer}]`; the smallest is what a tap means (a pad over its copper)
            s.onPick = { p ->
                val c = runCatching { org.json.JSONArray(p) }.getOrNull()
                val best = c?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.minByOrNull { it.optLong("a") } }
                if (best == null || frame == null) { info = ""; s.mark("") }
                else { val at = best.optInt("c"); val idx = best.optInt("p"); s.mark("$at,$idx")
                    scope.launch { info = runCatching { DataClient.info(frame.key, at, idx) }.getOrDefault("").ifEmpty { "piece $at,$idx" } } }
            }
            if (frame != null && frame.none.isEmpty()) s.show(frame, bg, 0x1f6f43, IntArray(0))
            if (three) s.mesh(mesh)
            if (s.renderer.off != off || s.renderer.dim != dim || s.renderer.lift != lift) { s.renderer.off = off; s.renderer.dim = dim; s.renderer.lift = lift; s.requestRender() }
        })
        val say = when {
            !has -> none
            error.isNotEmpty() -> error
            frame != null && frame.none.isNotEmpty() -> frame.none
            three && mesh != null && mesh.none.isNotEmpty() -> mesh.none
            else -> ""
        }
        if (say.isNotEmpty()) DMono(say, dcs.onSurfaceVariant, 12, Modifier.align(Alignment.Center).padding(24.dp))
        else if (frame == null && (!three || mesh == null)) CircularProgressIndicator(Modifier.align(Alignment.Center))
        if (!three && layersPresent.isNotEmpty()) {
            DMono("Layers", dcs.onSurface, 12, Modifier.align(Alignment.TopStart).padding(8.dp).background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant)).clickable { menu = !menu }.padding(horizontal = 10.dp, vertical = 6.dp).testTag("viewer.layers"))
        }
        if (menu && !three) LayerMenu(kind, layersPresent, off, dim, lift, { off = it }, { d, l -> dim = d; lift = l }, Modifier.align(Alignment.TopStart).padding(top = 44.dp, start = 8.dp))
        if (info.isNotEmpty()) Column(Modifier.align(Alignment.BottomCenter).fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(10.dp).testTag("viewer.info")) {
            Row { DMono("PICKED", dcs.primary, 11, Modifier.weight(1f), bold = true); DMono("close", dcs.onSurfaceVariant, 11, Modifier.clickable { info = "" }) }
            Text(info.replace("\u0000", "  ").replace(";", "\n").trim(), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, maxLines = 8)
        }
    }
}

/** KiCad's high contrast over the layer list: a box shows or hides a layer, a name makes it the active one (the rest dimmed), again brings all back. */
@Composable
private fun LayerMenu(kind: String, present: List<Int>, off: Int, dim: Int, lift: Int, setOff: (Int) -> Unit, setFocus: (Int, Int) -> Unit, modifier: Modifier) {
    val sch = kind == "schematic"
    val order = if (sch) present.sorted() else boardLayers.filter { it in present }
    val name = { l: Int -> (if (sch) schNames else boardNames).getOrElse(l) { "layer $l" } }
    Column(modifier.heightIn(max = 420.dp).verticalScroll(rememberScrollState()).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(8.dp).testTag("layer.menu"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        order.forEach { l ->
            val on = l !in 0..31 || (off shr l) and 1 == 0
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(14.dp).border(BorderStroke(1.dp, dcs.outlineVariant)).background(if (on) dcs.primary else dcs.surfaceVariant).clickable { if (l in 0..31) setOff(off xor (1 shl l)) }.testTag("layer.box.$l"))
                Spacer(Modifier.width(10.dp))
                DMono(name(l), if (lift == l) dcs.primary else dcs.onSurface, 12, Modifier.clickable {
                    if (lift == l) setFocus(0, 255) else setFocus(order.filter { it != l && it in 0..31 }.fold(0) { a, o -> a or (1 shl o) }, l)
                }.testTag("layer.name.$l"))
            }
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
        val say = if (error.isNotEmpty()) error else mesh?.none.orEmpty()
        if (say.isNotEmpty()) DMono(say, dcs.onSurfaceVariant, 12, Modifier.align(Alignment.Center).padding(24.dp))
        else if (mesh == null) CircularProgressIndicator(Modifier.align(Alignment.Center))
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
                DBtn("Go", true, url.isNotBlank(), Modifier.testTag("browser.go")) { run("navigate") { put("url", if ("://" in url) url else "https://$url") } }
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
