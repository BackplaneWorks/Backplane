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
import androidx.compose.ui.graphics.Color
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
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlin.math.abs

// The viewer over a thread (the desktop's right rail): Board, Schematic, 3D, Mech, Files and Browser, on the data/1 viewer requests (native/host/src/data/ext.rs).
// Plots and models arrive as the hub's own binary frames and are drawn by the app's GPU view (PlotStore, PlotSurface: pinch, pan, orbit, tap to inspect).
private val tabs = listOf("Board", "Schematic", "3D", "Split", "Mech", "Files", "Diff", "Review", "Browser")
/** A tab's name on screen (the list above holds its id: tags, links and the debug intent use it). */
private fun tabLabel(t: String) = when (t) { "Board" -> Msg.viewer_board; "Schematic" -> Msg.viewer_schematic; "3D" -> Msg.viewer_solid; "Split" -> Msg.android_viewer_split; "Mech" -> Msg.viewer_mech; "Files" -> Msg.viewer_files; "Diff" -> Msg.viewer_diff; "Review" -> Msg.viewer_review; "Browser" -> Msg.bots_tab_browser; else -> t }

/** A file asked to be opened from the Files tab: the viewer jumps to its tab with the path. */
private data class Target(val tab: String, val path: String)

@Composable
fun ViewerScreen(thread: DThread, file: String = "", line: Int = 0, back: () -> Unit) {
    // a linked file opens where it belongs: a board or schematic in its tab, a model in Mech, anything else previewed in Files
    val fx = ext(file)
    var tab by remember { mutableStateOf(when { file.isEmpty() -> "Files"; fx == "kicad_pcb" -> "Board"; fx == "kicad_sch" -> "Schematic"; fx in setOf("step", "stp", "glb") -> "Mech"; else -> "Files" }) }
    var target by remember { mutableStateOf<Target?>(when { fx == "kicad_pcb" -> Target("Board", file); fx == "kicad_sch" -> Target("Schematic", file); fx in setOf("step", "stp", "glb") -> Target("Mech", file); else -> null }) }
    var files by remember { mutableStateOf<DataClient.ProjectFiles?>(null) }
    val online = DataClient.conn == DataClient.Conn.ONLINE
    LaunchedEffect(thread.id, online) {
        if (files != null) return@LaunchedEffect
        files = runCatching { DataClient.viewerFiles(thread.id) }.getOrNull(); if (files?.pcb?.isNotEmpty() == true && tab == "Files" && file.isEmpty()) tab = "Board"
        DataClient.openViewerTab?.takeIf { it in tabs }?.let { tab = it; DataClient.openViewerTab = null }
    }
    DisposableEffect(thread.id) { onDispose { DataClient.stopWatch(thread.id) } }
    Column(Modifier.fillMaxSize()) {
        DBar(Msg.android_viewer_title(thread.title.ifEmpty { Msg.thread_new }), back)
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).horizontalScroll(rememberScrollState())) {
            tabs.forEach { t ->
                DMono(tabLabel(t), if (t == tab) dcs.onPrimary else dcs.onSurface, 12, Modifier.background(if (t == tab) dcs.primary else dcs.surfaceVariant).clickable { tab = t; target = null }.padding(horizontal = 14.dp, vertical = 12.dp).testTag("vtab.$t"), bold = t == tab)
            }
        }
        DLine()
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (tab) {
                "Board" -> PlotTab(thread, "board", target?.path?.takeIf { target?.tab == "Board" } ?: "", files == null || files?.pcb.orEmpty().isNotEmpty() || target != null, Msg.android_viewer_noboard)
                "Schematic" -> PlotTab(thread, "schematic", target?.path?.takeIf { target?.tab == "Schematic" } ?: "", files == null || files?.sch.orEmpty().isNotEmpty() || target != null, Msg.android_viewer_nosch)
                "3D" -> PlotTab(thread, "3d", "", files == null || files?.pcb.orEmpty().isNotEmpty(), Msg.android_viewer_noboard)
                "Split" -> SplitTab(thread, files)
                "Diff" -> DiffTab(thread)
                "Review" -> ReviewTab(thread)
                "Mech" -> MechTab(thread, target?.path?.takeIf { target?.tab == "Mech" })
                "Files" -> FilesTab(thread, if (fx == "kicad_pcb" || fx == "kicad_sch") "" else file, line) { t -> target = t; tab = t.tab }
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
    var lit by remember { mutableStateOf(emptyList<String>()) } // the tapped piece's part and net, lit (the cross-probe)
    var layers by remember { mutableStateOf<DataClient.Layers?>(null) }
    var refs by remember { mutableStateOf<List<DataClient.Ref>?>(null) }
    var comparing by remember { mutableStateOf("") }
    var menu by remember { mutableStateOf(false) }
    LaunchedEffect(thread.id, kind, path, has, DataClient.conn == DataClient.Conn.ONLINE) {
        if (!has) return@LaunchedEffect
        error = ""; layers = null; refs = null; comparing = ""; info = ""; picked = ""; lit = emptyList()
        runCatching { DataClient.watch(thread.id, kind, path) }.onFailure { error = it.message ?: "" }
    }
    val plots = DataClient.plots
    val frame = plots.frame?.takeIf { it.key.startsWith(if (three) "0|" else head) }
    val mesh = if (three) plots.mesh?.takeIf { it.key.startsWith("2|") } else null
    val parts = if (three && mesh != null) plots.parts.parts?.takeIf { it.key == mesh.key } else null
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
    // debug builds: what is on show, by the hub's keys, to logcat (tag bpview; the switch test reads it: an interop view's semantics do not reach uiautomator)
    val onShow = listOfNotNull(frame?.key, mesh?.takeIf { it.mesh != null }?.key).joinToString(" + ").ifEmpty { "nothing" }
    if (BuildConfig.DEBUG) LaunchedEffect(onShow) { android.util.Log.i("bpview", "$kind: $onShow") }
    val vbg = viewerBackdrop()   // the 3D background: the setting, or the theme's (redraws when either changes)
    Box(Modifier.fillMaxSize().background(dcs.background)) {
        AndroidView(factory = { PlotSurface(it) }, modifier = Modifier.fillMaxSize().testTag("viewer.surface"), update = { s ->
            s.bindSource(thread.id + "\u0000" + kind + "\u0000" + path)
            s.setThree(three)
            s.backdrop(vbg)
            // what lies under the finger comes as candidates `[{"c": chunk, "p": piece, "a": area, "l": layer}]`; the smallest is what a tap means (a pad over its copper)
            s.onPick = { p ->
                val c = runCatching { org.json.JSONArray(p) }.getOrNull()
                val best = c?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.minByOrNull { it.optLong("a") } }
                if (best == null || frame == null) { info = ""; picked = ""; lit = emptyList(); s.mark("") }
                else { val at = best.optInt("c"); val idx = best.optInt("p"); picked = "$at,$idx"; lit = emptyList(); s.mark(picked)
                    scope.launch {
                        info = runCatching { DataClient.info(frame.key, at, idx) }.getOrDefault("").ifEmpty { Msg.android_viewer_piece("$at,$idx") }
                        lit = Probe.light(info, listOf(frame.key))[frame.key].orEmpty()
                    } }
            }
            if (frame != null && frame.none.isEmpty()) s.show(frame, bg, 0x1f6f43, IntArray(0))
            s.mark(picked) // closing the card deselects
            s.markAll(lit)
            if (three) { s.mesh(mesh); s.parts(parts) }
            if (s.renderer.off != off || s.renderer.dim != dim || s.renderer.lift != lift) { s.renderer.off = off; s.renderer.dim = dim; s.renderer.lift = lift; s.requestRender() }
        })
        val say = when {
            !has -> none
            error.isNotEmpty() -> error
            frame != null && frame.none.isNotEmpty() -> frame.none
            three && mesh != null && !mesh.decoding && mesh.none.isNotEmpty() -> mesh.none
            else -> ""
        }
        if (say.isNotEmpty() && three && mesh?.mesh != null) DMono(Tr.t(say), dcs.onSurfaceVariant, 11, Modifier.align(Alignment.TopStart).padding(8.dp).background(dcs.surfaceVariant.copy(alpha = 0.85f)).padding(8.dp).testTag("viewer.note"))
        else if (say.isNotEmpty()) DMono(Tr.t(say), dcs.onSurfaceVariant, 12, Modifier.align(Alignment.Center).padding(24.dp))
        else if (three && (mesh?.mesh == null || mesh.decoding)) Loading(Msg.android_viewer_loading3d, Modifier.align(Alignment.Center))
        else if (frame == null && !three) CircularProgressIndicator(Modifier.align(Alignment.Center))
        val progress = parts?.label.orEmpty()
        if (progress.isNotEmpty() && mesh?.mesh != null) DMono(Tr.t(progress), dcs.onSurfaceVariant, 11, Modifier.align(Alignment.BottomStart).padding(8.dp).background(dcs.surfaceVariant.copy(alpha = 0.85f)).padding(horizontal = 8.dp, vertical = 4.dp).testTag("viewer.parts"))
        val l = layers
        if (!three && layersPresent.isNotEmpty() && l != null) {
            Row(Modifier.align(Alignment.TopStart).padding(8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                ViewChip(Tr.t(l.head), l.mode > 0, "viewer.layers") { refs = null; menu = !menu }
                if (comparing.isEmpty()) ViewChip(Msg.viewer_history_compare, refs != null, "viewer.compare") { menu = false; if (refs != null) refs = null else scope.launch { refs = runCatching { DataClient.refs(thread.id) }.getOrDefault(emptyList()) } }
                else ViewChip(Msg.android_compare_close, true, "viewer.compare.close") { comparing = ""; scope.launch { runCatching { DataClient.watch(thread.id, kind, path) }.onFailure { error = it.message ?: "" } } }
            }
            if (menu) LayerMenu(l, layersPresent, act, Modifier.align(Alignment.TopStart).padding(top = 44.dp, start = 8.dp))
            refs?.let { rows -> RefMenu(rows, { c -> refs = null; scope.launch { runCatching { DataClient.compare(thread.id, kind, c, l.bits, path); comparing = c }.onFailure { error = it.message ?: "" } } }, Modifier.align(Alignment.TopStart).padding(top = 44.dp, start = 8.dp)) }
        }
        if (info.isNotEmpty()) ProbeCard(info, lit.size, Modifier.align(Alignment.BottomCenter)) { info = ""; picked = ""; lit = emptyList() }
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
            listOf(Msg.viewer_layers_all, Msg.viewer_layers_dim, Msg.viewer_layers_only).forEachIndexed { i, label ->
                DMono(label, if (l.mode == i) dcs.primary else dcs.onSurface, 12, Modifier.border(BorderStroke(1.dp, if (l.mode == i) dcs.primary else dcs.outlineVariant)).clickable { act("mode", i.toString()) }.padding(horizontal = 8.dp, vertical = 3.dp).testTag("layer.mode.$i"))
            }
        }
        l.rows.filter { it.layer in present }.forEach { r ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(14.dp).border(BorderStroke(1.dp, dcs.outlineVariant)).background(if (r.on) dcs.primary else dcs.surfaceVariant).clickable { act("toggle", r.layer.toString()) }.testTag("layer.box.${r.layer}"))
                Spacer(Modifier.width(10.dp))
                DMono(Tr.t(r.name), if (r.active) dcs.primary else dcs.onSurface, 12, Modifier.clickable { act("pick", r.layer.toString()) }.testTag("layer.name.${r.layer}"))
            }
        }
        if (l.reset) DMono(Msg.viewer_layers_reset, dcs.primary, 12, Modifier.clickable { act("reset", "") }.testTag("layer.reset"))
    }
}

/** The versions a comparison may take, as the hub lists them; a press shows the Diff against that one. */
@Composable
private fun RefMenu(rows: List<DataClient.Ref>, pick: (String) -> Unit, modifier: Modifier) {
    Column(modifier.heightIn(max = 420.dp).verticalScroll(rememberScrollState()).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(8.dp).testTag("compare.menu"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (rows.isEmpty()) DMono(Msg.android_compare_none, dcs.onSurfaceVariant, 12)
        rows.forEachIndexed { i, r ->
            Row(Modifier.clickable { pick(r.commit) }.testTag("compare.row.$i"), horizontalArrangement = Arrangement.spacedBy(8.dp)) { DMono(Tr.t(r.kind), dcs.onSurfaceVariant, 12); DMono(Tr.t(r.label), dcs.onSurface, 12) }
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
            m.parts.isEmpty() -> DMono(Msg.android_mech_none, dcs.onSurfaceVariant, 12, Modifier.padding(24.dp))
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
    val vbg = viewerBackdrop()   // the 3D background: the setting, or the theme's (redraws when either changes)
    Box(Modifier.fillMaxSize().background(dcs.background)) {
        AndroidView(factory = { PlotSurface(it) }, modifier = Modifier.fillMaxSize().testTag("mech.surface"), update = { s ->
            s.bindSource(thread.id + "\u0000mech\u0000" + path); s.setThree(true)
            s.backdrop(vbg)
            val c = bgArgb; s.renderer.bg = floatArrayOf(((c shr 16) and 255) / 255f, ((c shr 8) and 255) / 255f, (c and 255) / 255f)
            s.mesh(mesh)
        })
        val say = if (error.isNotEmpty()) error else mesh?.takeIf { !it.decoding }?.none.orEmpty()
        if (say.isNotEmpty()) DMono(Tr.t(say), dcs.onSurfaceVariant, 12, Modifier.align(Alignment.Center).padding(24.dp))
        else if (mesh?.mesh == null || mesh.decoding) Loading(Msg.android_viewer_loading3d, Modifier.align(Alignment.Center))
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
private fun FilesTab(thread: DThread, start: String, startLine: Int, open: (Target) -> Unit) {
    // a linked file: its folder behind it, the file open at the line
    var dir by remember { mutableStateOf(start.substringBeforeLast('/', "")) }
    var list by remember { mutableStateOf<List<DataClient.Entry>?>(null) }
    var err by remember { mutableStateOf("") }
    var preview by remember { mutableStateOf<String?>(start.ifEmpty { null }) }
    var previewLine by remember { mutableStateOf(startLine) }
    LaunchedEffect(thread.id, dir) { list = null; err = ""; runCatching { DataClient.entries(thread.id, dir) }.onSuccess { list = it }.onFailure { err = it.message ?: "" } }
    if (preview != null) { FilePreview(thread, preview!!, previewLine) { preview = null; previewLine = 0 }; return }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (dir.isNotEmpty()) DMono("..", dcs.primary, 13, Modifier.clickable { dir = dir.substringBeforeLast('/', "") }.padding(end = 12.dp).testTag("files.up"), bold = true)
            DMono("/" + dir, dcs.onSurface, 12)
        }
        DLine()
        if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 12, Modifier.padding(12.dp))
        val l = list
        if (l == null && err.isEmpty()) CircularProgressIndicator(Modifier.padding(24.dp))
        l?.let { es ->
            if (es.isEmpty()) DMono(Msg.android_folder_empty, dcs.onSurfaceVariant, 12, Modifier.padding(12.dp))
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
/** A file with its line numbers, scrolled to the line a link named and that line marked. */
@Composable
private fun LinedText(text: String, line: Int) {
    val lines = remember(text) { text.split('\n') }
    val st = androidx.compose.foundation.lazy.rememberLazyListState()
    LaunchedEffect(line) { st.scrollToItem((line - 4).coerceIn(0, (lines.size - 1).coerceAtLeast(0))) }
    LazyColumn(Modifier.fillMaxSize().testTag("preview.text"), state = st) {
        items(lines.size) { i ->
            Row(Modifier.fillMaxWidth().background(if (i + 1 == line) dcs.primary.copy(alpha = .22f) else Color.Transparent).then(if (i + 1 == line) Modifier.testTag("preview.line") else Modifier).padding(vertical = 1.dp)) {
                Text("${i + 1}", Modifier.width(44.dp).padding(end = 8.dp), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, textAlign = androidx.compose.ui.text.style.TextAlign.End)
                Text(lines[i].ifEmpty { " " }, color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp)
            }
        }
    }
}
private fun size(n: Long) = when { n < 1024 -> "$n B"; n < 1 shl 20 -> "${n / 1024} KB"; else -> "${n shr 20} MB" }

@Composable
private fun FilePreview(thread: DThread, path: String, line: Int = 0, close: () -> Unit) {
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
            failed -> DMono(Msg.android_file_bad, dcs.error, 12, Modifier.padding(16.dp))
            b == null -> CircularProgressIndicator(Modifier.padding(24.dp))
            ext(path) in imageExt -> {
                val bmp = remember(b) { BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap() }
                if (bmp != null) Image(bmp, null, Modifier.fillMaxSize().padding(8.dp).testTag("preview.image"), contentScale = ContentScale.Fit) else DMono(Msg.android_file_noimage, dcs.onSurfaceVariant, 12, Modifier.padding(16.dp))
            }
            ext(path) == "pdf" -> PdfPreview(b)
            ext(path) in textExt || b.size < 200_000 && b.none { it.toInt() == 0 } -> {
                val text = remember(b) { String(b, Charsets.UTF_8).take(200_000) }
                if (line > 0) { LinedText(text, line); return@Column }
                Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(12.dp).testTag("preview.text")) {
                    if (ext(path) == "md") MdText(text) else Text(text, color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp)
                }
            }
            else -> DMono(Msg.android_file_nopreview(size(b.size.toLong())), dcs.onSurfaceVariant, 12, Modifier.padding(16.dp))
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
    if (r == null) { DMono(Msg.android_pdf_bad, dcs.error, 12, Modifier.padding(16.dp)); return }
    val bmp = remember(b, page) { r.openPage(page).use { p -> val w = 1080; val h = (w * p.height.toFloat() / p.width).toInt()
        android.graphics.Bitmap.createBitmap(w, h, android.graphics.Bitmap.Config.ARGB_8888).also { it.eraseColor(android.graphics.Color.WHITE); p.render(it, null, null, android.graphics.pdf.PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY) } }.asImageBitmap() }
    Column(Modifier.fillMaxSize().testTag("preview.pdf")) {
        Row(Modifier.fillMaxWidth().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            DBtn("‹", false, page > 0) { page-- }; DMono(Msg.android_pdf_page(page + 1, r.pageCount), dcs.onSurface, 12); DBtn("›", false, page + 1 < r.pageCount) { page++ }
        }
        Image(bmp, null, Modifier.fillMaxWidth().verticalScroll(rememberScrollState()), contentScale = ContentScale.FillWidth)
    }
}

// ---- Browser: the page's newest frame; once the person has the page a tap clicks, a drag scrolls and the key field types (BrowserInput.kt)
@Composable
private fun BrowserTab(thread: DThread) {
    val scope = rememberCoroutineScope()
    var frame by remember { mutableStateOf<androidx.compose.ui.graphics.ImageBitmap?>(null) }
    var none by remember { mutableStateOf(false) }
    var driving by remember { mutableStateOf(false) }
    var url by remember { mutableStateOf("") }
    var err by remember { mutableStateOf("") }
    var keys by remember { mutableStateOf(TextFieldValue(BrowserInput.SENTINEL, TextRange(BrowserInput.SENTINEL.length))) }
    val order = remember { Mutex() }   // the person's input reaches the hub in the order it was made
    suspend fun load() { val b = DataClient.screenBytes(thread.id); if (b == null) none = frame == null else { frame = BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap(); none = false } }
    LaunchedEffect(thread.id) { while (true) { load(); delay(2000) } }
    LaunchedEffect(DataClient.shot) { if (DataClient.shotBot == thread.id) load() }
    fun run(op: String, extra: JSONObject.() -> Unit = {}) = scope.launch { order.withLock { err = ""; runCatching { DataClient.browser(thread.id, op, extra) }.onFailure { err = it.message ?: "" } } }
    fun press(k: String) = run("press") { put("key", k) }
    fun keyed(v: TextFieldValue) {
        if (v.text == keys.text) { keys = v; return }
        for ((op, a) in BrowserInput.keys(keys.text, v.text)) if (op == "type") run("type") { put("text", a) } else press(a)
        val n = BrowserInput.next(v.text); keys = if (n == v.text) v else TextFieldValue(n, TextRange(n.length))
    }
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (!driving) DBtn(Msg.android_browser_takeover, true, modifier = Modifier.testTag("browser.takeover")) { driving = true; run("takeover") }
                else {
                    DBtn("‹", false, modifier = Modifier.testTag("browser.back")) { run("toolbar") { put("command", "back") } }
                    DBtn("›", false, modifier = Modifier.testTag("browser.forward")) { run("toolbar") { put("command", "forward") } }
                    DBtn("↻", false, modifier = Modifier.testTag("browser.reload")) { run("toolbar") { put("command", "reload") } }
                    Box(Modifier.weight(1f)) { DField(url, { url = it }, Msg.browser_address, Modifier.testTag("browser.url")) }
                    DBtn(Msg.common_go, true, url.isNotBlank(), Modifier.testTag("browser.go")) { run("navigate") { put("url", pageUrl(url)) } }
                }
            }
            if (driving) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                    // the key field: what the keyboard types here goes to the page (Enter, Tab and the arrows of a hardware keyboard too)
                    Box(Modifier.weight(1f).background(dview).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 10.dp, vertical = 10.dp)) {
                        if (keys.text == BrowserInput.SENTINEL) Text(Msg.android_browser_keys, color = dfaint, fontFamily = BpMono, fontSize = 13.sp, maxLines = 1)
                        BasicTextField(keys, ::keyed, singleLine = true,
                            textStyle = TextStyle(color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp), cursorBrush = SolidColor(dcs.primary),
                            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Send),
                            keyboardActions = KeyboardActions(onSend = { press("Enter") }),
                            modifier = Modifier.fillMaxWidth().testTag("browser.keys").onPreviewKeyEvent { ev ->
                                if (ev.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                                val k = when (ev.key) { Key.Enter, Key.NumPadEnter -> "Enter"; Key.Tab -> "Tab"; Key.DirectionLeft -> "ArrowLeft"; Key.DirectionUp -> "ArrowUp"; Key.DirectionRight -> "ArrowRight"; Key.DirectionDown -> "ArrowDown"; else -> "" }
                                if (k.isEmpty()) false else { press(k); true }
                            })
                    }
                    DBtn(Msg.android_browser_handback, false, modifier = Modifier.testTag("browser.handback")) { driving = false; run("handback") }
                }
                Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    for ((label, k) in BrowserInput.keyButtons) DBtn(label, false, modifier = Modifier.testTag("browser.key.$k")) { press(k) }
                }
            }
        }
        if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 12, Modifier.padding(horizontal = 12.dp, vertical = 4.dp))
        DLine()
        Box(Modifier.weight(1f).fillMaxWidth().background(dcs.background), contentAlignment = Alignment.Center) {
            val f = frame
            when {
                f != null -> { val frameW = rememberUpdatedState(f.width)   // a new frame must not restart a drag in progress
                    Image(f, Msg.android_browser_page, Modifier.fillMaxWidth().testTag("browser.frame").pointerInput(driving) {
                    detectTapGestures { p -> if (driving && size.width > 0 && size.height > 0) run("click") { put("x", (p.x * 1000 / size.width).toInt().coerceIn(0, 1000)); put("y", (p.y * 1000 / size.height).toInt().coerceIn(0, 1000)) } }
                }.pointerInput(driving) {
                    if (!driving) return@pointerInput
                    // a drag scrolls the page the other way, in steps; the rest goes when the finger lifts
                    var dx = 0f; var dy = 0f
                    fun send(all: Boolean) {
                        val (sx, sy) = BrowserInput.scroll(dx.toDouble(), dy.toDouble(), frameW.value.toDouble(), size.width.toDouble())
                        if ((all && (sx != 0 || sy != 0)) || abs(sx) + abs(sy) >= BrowserInput.STEP) { run("scroll") { put("dx", sx); put("dy", sy) }; dx = 0f; dy = 0f }
                    }
                    detectDragGestures(onDragEnd = { send(true) }, onDragCancel = { send(true) }) { c, d -> c.consume(); dx += d.x; dy += d.y; send(false) }
                }, contentScale = ContentScale.FillWidth) }
                none -> DMono(Msg.android_browser_none, dcs.onSurfaceVariant, 12, Modifier.padding(24.dp))
                else -> CircularProgressIndicator()
            }
        }
        if (driving) DMono(Msg.android_browser_driving, dcs.onSurfaceVariant, 11, Modifier.padding(8.dp))
    }
}


/** Split's choice per thread, Board (false) or 3D (true): kept while the app runs, across tabs and threads. */
private val splitThree = androidx.compose.runtime.snapshots.SnapshotStateMap<String, Boolean>()

// ---- Split: the schematic, and the board or its 3D model (stacked in portrait, side by side in landscape). On a phone the board and 3D take
// turns in one pane (the person, 2026-11-04/05: layout XOR 3D on phones, all three only on the desktop); Board/3D on that pane switches, per thread.
@Composable
private fun SplitTab(thread: DThread, files: DataClient.ProjectFiles?) {
    val three = splitThree[thread.id] == true
    val root = files?.root.orEmpty()
    LaunchedEffect(thread.id, root) { if (root.isNotEmpty()) runCatching { DataClient.watch(thread.id, "split") } }
    val plots = DataClient.plots
    val seen = plots.frame to plots.mesh   // the store changes on each frame: read it so the panes follow
    val sch = plots.cachedFrame("1|$root|"); val brd = plots.cachedFrame("0|$root|"); val mdl = plots.cachedMesh("2|$root|")
    // the parts that stream in after the board model, as the 3D tab draws them (Split showed the bare board only)
    val prt = mdl?.let { m -> plots.parts.parts?.takeIf { it.key == m.key } }
    val bg = dcs.background.toArgb() and 0xFFFFFF
    // a tap in any pane: what it is, and its part lit on the schematic and the board, its net on the board (the desktop's cross-probe)
    val scope = rememberCoroutineScope()
    var info by remember(thread.id) { mutableStateOf("") }
    var lit by remember(thread.id) { mutableStateOf<Map<String, List<String>>>(emptyMap()) }
    val tap: (String, Int, Int) -> Unit = { key, at, idx -> scope.launch {
        info = runCatching { DataClient.info(key, at, idx) }.getOrDefault("").ifEmpty { Msg.android_viewer_piece("$at,$idx") }
        lit = Probe.light(info, listOf("1|$root|", "0|$root|"))
    } }
    val clear = { info = ""; lit = emptyMap() }
    BoxWithConstraints(Modifier.fillMaxSize().testTag("split")) {
        val panes: @Composable (Modifier) -> Unit = { m ->
            SplitPane("Schematic", thread.id + "|split|sch", sch, null, false, bg, if (root.isEmpty() || files?.sch.isNullOrEmpty()) Msg.android_viewer_nosch else "", m, lit = lit["1|$root|"].orEmpty(), tap = tap, untap = clear)
            val pick: @Composable (Modifier) -> Unit = { a -> Row(a.padding(4.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                ViewChip(Msg.viewer_board, !three, "split.pick.board") { splitThree[thread.id] = false }; ViewChip(Msg.viewer_solid, three, "split.pick.3d") { splitThree[thread.id] = true } } }
            if (three) SplitPane("3D", thread.id + "|split|3d", brd, mdl, true, bg, if (files?.pcb.isNullOrEmpty()) Msg.android_viewer_noboard else "", m, pick, lit["0|$root|"].orEmpty(), tap, clear, prt)
            else SplitPane("Board", thread.id + "|split|brd", brd, null, false, bg, if (files?.pcb.isNullOrEmpty()) Msg.android_viewer_noboard else "", m, pick, lit["0|$root|"].orEmpty(), tap, clear)
        }
        if (maxWidth > maxHeight) Row(Modifier.fillMaxSize()) { panes(Modifier.weight(1f).fillMaxHeight()) }
        else Column(Modifier.fillMaxSize()) { panes(Modifier.weight(1f).fillMaxWidth()) }
        if (info.isNotEmpty()) ProbeCard(info, lit.values.sumOf { it.size }, Modifier.align(Alignment.BottomCenter), clear)
    }
}
@Composable
private fun SplitPane(label: String, source: String, frame: PlotFrame?, mesh: MeshFrame?, three: Boolean, bg: Int, none: String, modifier: Modifier, pick: (@Composable (Modifier) -> Unit)? = null,
                      lit: List<String> = emptyList(), tap: (String, Int, Int) -> Unit = { _, _, _ -> }, untap: () -> Unit = {}, parts: PartsFrame? = null) {
    val tapNow by rememberUpdatedState(tap); val untapNow by rememberUpdatedState(untap); val frameNow by rememberUpdatedState(frame)
    val vbg = viewerBackdrop()   // the 3D background: the setting, or the theme's (redraws when either changes)
    Box(modifier.border(BorderStroke(1.dp, dcs.outlineVariant)).background(dcs.background)) {
        AndroidView(factory = { PlotSurface(it) }, modifier = Modifier.fillMaxSize().testTag("split.$label"), update = { s ->
            s.bindSource(source); s.setThree(three)
            s.backdrop(vbg)
            if (frame != null && frame.none.isEmpty()) s.show(frame, bg, 0x1f6f43, IntArray(0))
            if (three) { s.mesh(mesh); s.parts(parts) }
            s.onPick = { p ->
                val c = runCatching { org.json.JSONArray(p) }.getOrNull()
                val best = c?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.minByOrNull { it.optLong("a") } }
                val f = frameNow
                if (best == null || f == null) untapNow() else tapNow(f.key, best.optInt("c"), best.optInt("p"))
            }
            s.markAll(lit)
        })
        DMono(tabLabel(label).uppercase(), dcs.onSurfaceVariant, 10, Modifier.align(Alignment.TopStart).padding(6.dp))
        val say = if (none.isNotEmpty()) none else frame?.none.orEmpty()
        if (say.isNotEmpty()) DMono(Tr.t(say), dcs.onSurfaceVariant, 11, Modifier.align(Alignment.Center).padding(12.dp))
        else if (frame == null && (!three || mesh == null)) CircularProgressIndicator(Modifier.align(Alignment.Center).size(24.dp))
        pick?.invoke(Modifier.align(Alignment.TopEnd))
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
            DMono(Msg.android_changes_head, dcs.onSurfaceVariant, 11, Modifier.weight(1f).padding(start = 4.dp)); DBtn(Msg.common_refresh, false, modifier = Modifier.testTag("diff.refresh")) { n++ }
        }
        DLine()
        val d = diff
        when {
            err.isNotEmpty() -> DMono(Tr.t(err), dcs.error, 12, Modifier.padding(16.dp))
            d == null -> CircularProgressIndicator(Modifier.padding(24.dp))
            nogit.isNotEmpty() -> DMono(Msg.android_diff_nogit, dcs.onSurfaceVariant, 12, Modifier.padding(16.dp).testTag("diff.nogit"), lines = 4)
            d.isBlank() -> DMono(Msg.android_diff_none, dcs.onSurfaceVariant, 12, Modifier.padding(16.dp).testTag("diff.none"))
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
            listOf("Checks" to Msg.review_checks, "BOM" to Msg.review_bom, "Comments" to Msg.review_comments).forEach { (p, l) -> DBtn(l, page == p, modifier = Modifier.testTag("review.page.$p")) { page = p } }
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
        DBtn(if (ticks) Msg.android_checks_run else Msg.common_refresh, false, modifier = Modifier.testTag("review.run")) { run++ }
        val r = rep
        if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 12)
        if (r == null && err.isEmpty()) CircularProgressIndicator()
        r?.let {
            if (it.line.isNotEmpty()) DMono(Tr.t(it.line), dcs.primary, 12, bold = true)
            if (it.gate.isNotEmpty()) DMono(Tr.t(it.gate), dcs.onSurfaceVariant, 11)
            if (it.err.isNotEmpty()) DMono(Tr.t(it.err), dcs.error, 12)
            if (it.md.isNotEmpty()) MdText(it.md)
            if (ticks) tasks.forEachIndexed { i, t ->
                Row(Modifier.fillMaxWidth().clickable { scope.launch { runCatching { DataClient.tick(thread.id, i) }.onSuccess { n -> if (n.isNotEmpty()) tasks = n } } }.padding(vertical = 6.dp).testTag("review.task.$i"), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(16.dp).border(BorderStroke(1.dp, dcs.outlineVariant)).background(if (t.done) dcs.primary else dcs.surfaceVariant)); Spacer(Modifier.width(10.dp))
                    Text(t.text, color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp)
                }
            }
            if (it.md.isEmpty() && it.line.isEmpty() && it.err.isEmpty() && tasks.isEmpty()) DMono(Msg.android_nothing, dcs.onSurfaceVariant, 12)
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
            if (err.isNotEmpty()) DMono(Tr.t(err), dcs.error, 12)
            val l = list
            if (l == null) CircularProgressIndicator() else if (l.isEmpty()) DMono(Msg.android_comments_none, dcs.onSurfaceVariant, 12)
            l?.filter { it.parent.isEmpty() }?.forEach { c ->
                Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, if (c.resolved) dcs.outlineVariant else dcs.primary.copy(alpha = .6f))).padding(10.dp)) {
                    Row { DMono((c.by.ifEmpty { Msg.android_you }) + if (c.resolved) " · " + Msg.android_resolved else "", dcs.onSurfaceVariant, 11, Modifier.weight(1f))
                        DMono(if (c.resolved) Msg.android_reopen else Msg.android_resolve, dcs.primary, 11, Modifier.clickable { scope.launch { runCatching { DataClient.comment(thread.id, if (c.resolved) "reopen" else "resolve") { put("id", c.id) } }; load() } }.testTag("review.resolve.${c.id}")) }
                    Text(c.text, color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp, modifier = Modifier.padding(top = 4.dp))
                    l.filter { it.parent == c.id }.forEach { r -> Text("↳ " + r.text, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, modifier = Modifier.padding(top = 4.dp)) }
                }
            }
        }
        DLine()
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(8.dp), verticalAlignment = Alignment.Bottom) {
            Box(Modifier.weight(1f)) { DField(draft, { draft = it }, Msg.android_comment_hint, Modifier.testTag("review.composer"), 4) }
            Spacer(Modifier.width(8.dp))
            DBtn(Msg.common_add, true, draft.isNotBlank(), Modifier.testTag("review.add")) { val t = draft; draft = ""; scope.launch { runCatching { DataClient.comment(thread.id, "add") { put("text", t) } }.onFailure { err = it.message ?: "" }; load() } }
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

/** The cross-probe (the desktop's plotview.rs `cross_probe`): a tapped piece's part (`ref`) lit on every plot given, its net on the boards' (a schematic's info has no nets). */
object Probe {
    /** `name=value` of the piece's info (`kind;net=GND;ref=R1`): its first piece's, when the info holds several. */
    fun field(info: String, name: String): String = info.substringBefore('\u0000').split(';').firstNotNullOfOrNull { f -> f.trim().takeIf { it.startsWith("$name=") }?.substringAfter('=') }.orEmpty()
    suspend fun light(info: String, keys: List<String>): Map<String, List<String>> {
        val ref = part(info); val net = field(info, "net")
        return keys.filter { it.isNotEmpty() }.associateWith { k ->
            buildList {
                if (ref.isNotEmpty()) addAll(runCatching { DataClient.probe(k, "ref", ref) }.getOrDefault(emptyList()))
                if (net.isNotEmpty() && k.startsWith("0|")) addAll(runCatching { DataClient.probe(k, "net", net) }.getOrDefault(emptyList()))
            }.distinct()
        }
    }
    /** The part a piece belongs to: its `ref`, or a schematic pin's `symbol`. */
    fun part(info: String) = field(info, "ref").ifEmpty { field(info, "symbol") }
    /** The card's two lines: what it is (and its part), then the rest of its fields (net, layer, width…); `g`, the hub's net group id, is not for reading. */
    fun lines(info: String): Pair<String, String> {
        val fs = info.substringBefore('\u0000').split(';').map { it.trim() }.filter { it.isNotEmpty() }
        val kind = fs.firstOrNull { '=' !in it }.orEmpty(); val ref = part(info)
        val head = listOf(kind, ref).filter { it.isNotEmpty() }.joinToString(" · ").ifEmpty { fs.firstOrNull().orEmpty() }
        val rest = fs.filter { '=' in it && !it.startsWith("ref=") && !it.startsWith("symbol=") && !it.startsWith("g=") }.joinToString(" · ") { it.substringBefore('=') + " " + it.substringAfter('=') }
        return head to rest
    }
}

/** What was tapped, small (the person: the cross-probe's popup took too much room, 11-04): two lines and how many pieces are lit; × closes it and the light. */
@Composable
private fun ProbeCard(info: String, lit: Int, modifier: Modifier, close: () -> Unit) {
    val (head, rest) = Probe.lines(info)
    // the words can be selected (a long press), and Copy takes them all: what was picked and its values (the person: the value at the foot should copy)
    val ctx = androidx.compose.ui.platform.LocalContext.current
    var copied by remember(info) { mutableStateOf(false) }
    LaunchedEffect(copied) { if (copied) { kotlinx.coroutines.delay(1500); copied = false } }
    Row(modifier.fillMaxWidth().background(dcs.surfaceVariant.copy(alpha = .94f)).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(start = 12.dp, top = 6.dp, bottom = 6.dp).testTag("viewer.info"), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.weight(1f)) { androidx.compose.foundation.text.selection.SelectionContainer { Column {
            Text(head, color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Bold, maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
            if (rest.isNotEmpty() || lit > 1) Text(listOf(rest, if (lit > 1) Msg.android_probe_lit(lit) else "").filter { it.isNotEmpty() }.joinToString(" · "), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.sp, maxLines = 2, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
        } } }
        Box(Modifier.size(44.dp).clickable {
            (ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager).setPrimaryClip(android.content.ClipData.newPlainText("Backplane", listOf(head, rest).filter { it.isNotEmpty() }.joinToString("\n")))
            copied = true
        }.testTag("viewer.info.copy"), contentAlignment = Alignment.Center) { Icon(if (copied) Icons.Default.Check else Icons.Default.ContentCopy, Msg.common_copy, tint = if (copied) dcs.primary else dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)) }
        Box(Modifier.size(44.dp).clickable(onClick = close).testTag("viewer.info.close"), contentAlignment = Alignment.Center) { Icon(Icons.Default.Close, Msg.common_close, tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)) }
    }
}
