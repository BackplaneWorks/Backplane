package dev.backplane.mobile

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.key.*
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

// Terminals on the phone (feature `terms`): the card in a thread, the full-screen view, and the Terminals list. The screen is the Vt port's
// (Vt.kt); what is held per terminal is TermScreen's (TermStream.kt); Terms (TermClient.kt) watches and writes. Redraw only when bytes arrive.

/** Opens a terminal full screen (provided by DataRoot). */
internal val LocalTermOpen = compositionLocalOf<(String) -> Unit> { {} }
/** Opens the Terminals list of a project (null: the hub's). */
internal val LocalTermsOpen = compositionLocalOf<(String?) -> Unit> { {} }

internal fun termsOn() = DataClient.has("terms")
/** A terminal's state word (running, exited N, stopped, failed to start) in the app's language. */
internal fun termStateLabel(s: String) = when {
    s == "running" -> Msg.state_running; s == "stopped" -> Msg.state_stopped; s == "failed to start" -> Msg.android_term_failed
    s.startsWith("exited ") -> Msg.android_term_exited(s.removePrefix("exited ")); else -> Tr.t(s)
}

// ---- drawing

internal class TermPaint(val px: Float, reg: Typeface, val bold: Typeface, val regular: Typeface = reg) {
    val p = Paint(Paint.ANTI_ALIAS_FLAG).apply { typeface = reg; textSize = px }
    val cw: Float = p.measureText("M")
    private val fm = p.fontMetrics
    val lh: Float = kotlin.math.ceil(fm.descent - fm.ascent + fm.leading)
    val base: Float = -fm.ascent
    val fill = Paint()
}
@Composable
internal fun rememberTermPaint(px: Float): TermPaint {
    val res = LocalContext.current.resources
    val reg = remember { res.getFont(R.font.jetbrains_mono_regular) }; val bold = remember { res.getFont(R.font.jetbrains_mono_bold) }
    return remember(px) { TermPaint(px, reg, bold) }
}

private fun argb(rgb: Int) = rgb or (0xFF shl 24)
/** Rows `lines` from the top of this draw scope; the cursor at (x, y) of those rows when given. */
internal fun DrawScope.drawTerm(lines: List<VLine>, tp: TermPaint, cursor: VCur?) {
    drawRect(Color(argb(VT_DEFAULT_BG)), size = size)
    drawIntoCanvas { cv ->
        val c = cv.nativeCanvas; val p = tp.p
        for ((r, line) in lines.withIndex()) {
            val top = r * tp.lh; if (top > size.height) break
            var col = 0
            for (run in line.runs) {
                val cells = run.text.codePoints().toArray().sumOf { vtWidth(it) }
                if (run.bg != VT_DEFAULT_BG) { tp.fill.color = argb(run.bg); c.drawRect(col * tp.cw, top, (col + cells) * tp.cw, top + tp.lh, tp.fill) }
                p.color = argb(run.fg); p.typeface = if (run.bold) tp.bold else tp.regular
                if (run.text.all { it.code < 128 }) c.drawText(run.text, col * tp.cw, top + tp.base, p)
                else { var x = col; run.text.codePoints().forEach { cp -> if (vtWidth(cp) > 0) c.drawText(String(Character.toChars(cp)), x * tp.cw, top + tp.base, p); x += vtWidth(cp) } }
                if (run.underline) { p.strokeWidth = 1f; c.drawLine(col * tp.cw, top + tp.lh - 2f, (col + cells) * tp.cw, top + tp.lh - 2f, p) }
                col += cells
            }
        }
        if (cursor != null && cursor.visible && cursor.y < lines.size) {
            tp.fill.color = (0x99 shl 24) or VT_DEFAULT_FG
            c.drawRect(cursor.x * tp.cw, cursor.y * tp.lh, (cursor.x + 1) * tp.cw, (cursor.y + 1) * tp.lh, tp.fill)
        }
    }
}

// ---- the card in a thread

/** The words of a state and the colour they take. */
@Composable
internal fun termStateColor(s: String): Color = when {
    s == "running" -> dok
    s.startsWith("exited ") && s != "exited 0" || s == "failed to start" -> dcs.error
    s == "stopped" -> dwarn
    else -> dfaint
}

/** Watches `key` while it is on screen and the app is in front and connected; otherwise the last screen held stays. */
@Composable
internal fun WatchTerm(key: String) {
    val on = DataClient.conn == DataClient.Conn.ONLINE && DataClient.foreground
    DisposableEffect(key, on) { if (on) Terms.watch(key); onDispose { if (on) Terms.unwatch(key) } }
}

/** The last rows worth drawing in a card: up to `max`, ending at the last row with something on it (or the cursor's). */
internal fun cardRows(lines: List<VLine>, cursorY: Int, max: Int): List<VLine> {
    val last = maxOf(lines.indexOfLast { it.runs.isNotEmpty() }, cursorY).coerceIn(0, maxOf(lines.size - 1, 0))
    val from = maxOf(0, last - max + 1)
    return lines.subList(from, minOf(lines.size, last + 1))
}

@Composable
internal fun TermCard(e: DEntry) {
    val t = e.term ?: return
    WatchTerm(t.key)
    val open = LocalTermOpen.current
    val sc = Terms.screen(t.key); val rev = Terms.rev(t.key)
    val tp = rememberTermPaint(with(LocalDensity.current) { 10.sp.toPx() })
    val state = sc.stateLine
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp).border(BorderStroke(1.dp, dcs.outlineVariant)).testTag("term.card.${t.id}")) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).clickable { open(t.key) }.padding(start = 10.dp, end = 4.dp).heightIn(min = 36.dp), verticalAlignment = Alignment.CenterVertically) {
            DMono(sc.title.ifEmpty { t.title }, dcs.onSurface, 12, Modifier.weight(1f), bold = true)
            if (state.isNotEmpty()) { Spacer(Modifier.width(8.dp)); DMono(termStateLabel(state), termStateColor(state), 11, Modifier.testTag("term.card.state")) }
            if (sc.running) { Spacer(Modifier.width(8.dp)); DBtn(Msg.composer_stop, false, modifier = Modifier.testTag("term.card.stop"), danger = true) { Terms.stop(t.key) } }
        }
        val rows = remember(rev) { cardRows(sc.vt.lines(0), sc.vt.cursor().y, 8) }
        val n = maxOf(rows.size, 3)
        val cursor = remember(rev) { sc.vt.cursor().let { c -> val first = maxOf(sc.vt.lines(0).indexOfLast { it.runs.isNotEmpty() }, c.y).coerceAtLeast(0) - rows.size + 1; VCur(c.x, c.y - maxOf(first, 0), c.visible && sc.running) } }
        val h = with(LocalDensity.current) { (tp.lh * n).toDp() }
        Canvas(Modifier.fillMaxWidth().height(h).clickable { open(t.key) }.testTag("term.card.screen")) { rev; drawTerm(rows, tp, cursor) }
    }
}

// ---- full screen

private const val SENT = "      " // the hidden field's text: a typed character is what follows it, a deleted one is what is missing from it

@Composable
internal fun TermFull(key: String, title0: String, back: () -> Unit) {
    WatchTerm(key)
    val ctx = LocalContext.current
    val prefs = remember { ctx.getSharedPreferences("data.ui", android.content.Context.MODE_PRIVATE) }
    val sc = Terms.screen(key); val rev = Terms.rev(key)
    var fontSp by remember { mutableFloatStateOf(prefs.getFloat("term.font", 12f)) }
    var pinch by remember { mutableFloatStateOf(1f) }
    var scroll by remember { mutableIntStateOf(0) }
    var area by remember { mutableStateOf(IntSize.Zero) }
    var ctrl by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    val density = LocalDensity.current
    val tp = rememberTermPaint(with(density) { fontSp.sp.toPx() })
    val live = rememberTermPaint(with(density) { (fontSp * pinch).sp.toPx() })
    val cols = maxOf(2, (area.width / tp.cw).toInt()); val rows = maxOf(2, (area.height / tp.lh).toInt())
    val online = DataClient.conn == DataClient.Conn.ONLINE
    // the grid that fits changed (rotation, font size, the keyboard): the pty takes the new size, every client then gets a snapshot
    val sized = area.width > 0 && area.height > 0
    LaunchedEffect(key, cols, rows, online, sized) { if (online && sized) Terms.resize(key, cols, rows) }
    val focus = remember { FocusRequester() }
    val kb = LocalSoftwareKeyboardController.current
    val clip = LocalClipboardManager.current
    // the keyboard comes with a tap on the screen, not with opening it (the person: less trigger happy, 11-04)
    var tf by remember { mutableStateOf(TextFieldValue(SENT, TextRange(SENT.length))) }
    var last by remember { mutableStateOf(SENT) }

    fun out(b: ByteArray) { scroll = 0; Terms.send(key, b) }
    fun typed(cp: Int) {
        val c = if (cp == 10) 13 else cp
        out(sc.vt.key(c, c, if (ctrl) 4 else 0)); ctrl = false
    }
    fun press(sym: Int, text: Int = 0) { out(sc.vt.key(sym, text, if (ctrl) 4 else 0)); ctrl = false }
    fun paste() { clip.getText()?.text?.takeIf { it.isNotEmpty() }?.let { out(sc.vt.paste(it)) } }

    Column(Modifier.fillMaxSize().background(dcs.background)) {
        // a landscape screen with the keyboard up gives its rows to the terminal: the bar returns with the keyboard down
        @OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
        val tight = LocalContext.current.resources.configuration.orientation == android.content.res.Configuration.ORIENTATION_LANDSCAPE && WindowInsets.isImeVisible
        if (!tight) DBar(sc.vt.title().ifEmpty { sc.title.ifEmpty { title0 } }, back) {
            val state = sc.stateLine
            if (state.isNotEmpty()) DMono(termStateLabel(state), termStateColor(state), 11, Modifier.padding(end = 8.dp).testTag("term.state"))
            if (sc.running) DBtn(Msg.composer_stop, false, modifier = Modifier.testTag("term.stop"), danger = true) { Terms.stop(key) }
            Spacer(Modifier.width(4.dp))
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            Canvas(Modifier.fillMaxSize().onSizeChanged { area = it }.testTag("term.screen")
                .pointerInput(Unit) { detectTapGestures(onTap = { focus.requestFocus(); kb?.show() }, onLongPress = { menu = true }) }
                .pointerInput(tp) {
                    // one finger scrolls back through the scrollback; two pinch the font size, applied (and the pty resized) when they lift
                    awaitEachGesture {
                        awaitFirstDown(requireUnconsumed = false)
                        var zoom = 1f; var dy = 0f; var multi = false
                        do {
                            val ev = awaitPointerEvent(PointerEventPass.Main)
                            if (ev.changes.size >= 2) { multi = true; zoom *= ev.calculateZoom(); pinch = zoom.coerceIn(.4f, 3f); ev.changes.forEach { it.consume() } }
                            else if (!multi) {
                                dy += ev.calculatePan().y
                                val step = (dy / tp.lh).toInt()
                                if (step != 0) { dy -= step * tp.lh; scroll = (scroll + step).coerceIn(0, sc.vt.history()); ev.changes.forEach { it.consume() } }
                            }
                        } while (ev.changes.any { it.pressed })
                        if (multi) { val next = (fontSp * pinch).coerceIn(6f, 30f); pinch = 1f; if (next != fontSp) { fontSp = next; prefs.edit().putFloat("term.font", next).apply() } }
                    }
                }
            ) {
                rev
                val lines = sc.vt.lines(scroll)
                drawTerm(lines, live, if (scroll == 0) sc.vt.cursor() else null)
            }
            DropdownMenu(menu, { menu = false }) {
                DropdownMenuItem({ Text(Msg.android_paste, fontFamily = BpMono) }, { menu = false; paste() }, Modifier.testTag("term.menu.paste"))
                DropdownMenuItem({ Text(Msg.android_term_copy, fontFamily = BpMono) }, { menu = false; clip.setText(AnnotatedString(sc.vt.lines(0).joinToString("\n") { l -> l.runs.joinToString("") { it.text } }.trimEnd())) }, Modifier.testTag("term.menu.copy"))
            }
            // the keyboard's door: a field nobody sees; what it receives is the program's input
            BasicTextField(
                tf, { nv ->
                    // what changed since the last text seen (not since the sentinel: several characters can arrive before the field is rebuilt)
                    val t = nv.text; var p = 0
                    while (p < last.length && p < t.length && t[p] == last[p]) p++
                    repeat(last.length - p) { out(sc.vt.key(65288, 8, 0)) }
                    var i = p; while (i < t.length) { val cp = t.codePointAt(i); typed(cp); i += Character.charCount(cp) }
                    if (t.length > 64 || t.length < 2) { last = SENT; tf = TextFieldValue(SENT, TextRange(SENT.length)) } else { last = t; tf = nv }
                },
                Modifier.size(1.dp).focusRequester(focus).testTag("term.input").onPreviewKeyEvent { ev ->
                    if (ev.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                    val sym = when (ev.key) {
                        Key.Enter, Key.NumPadEnter -> 65293; Key.Backspace -> 65288; Key.Tab -> 65289; Key.Escape -> 65307; Key.Delete -> 65535
                        Key.DirectionLeft -> 65361; Key.DirectionUp -> 65362; Key.DirectionRight -> 65363; Key.DirectionDown -> 65364
                        Key.MoveHome -> 65360; Key.MoveEnd -> 65367; Key.PageUp -> 65365; Key.PageDown -> 65366
                        else -> 0
                    }
                    val mods = (if (ev.isShiftPressed) 1 else 0) or (if (ev.isCtrlPressed || ctrl) 4 else 0) or (if (ev.isAltPressed) 8 else 0)
                    if (sym != 0) { out(sc.vt.key(sym, if (sym == 65293) 13 else if (sym == 65288) 8 else if (sym == 65289) 9 else if (sym == 65307) 27 else 0, mods)); ctrl = false; true }
                    else if ((ev.isCtrlPressed || ctrl) && ev.utf16CodePoint != 0) { out(sc.vt.key(ev.utf16CodePoint, ev.utf16CodePoint, mods or 4)); ctrl = false; true }
                    else false
                },
                textStyle = TextStyle(color = Color.Transparent, fontSize = 1.sp),
                cursorBrush = androidx.compose.ui.graphics.SolidColor(Color.Transparent),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false, imeAction = ImeAction.None),
            )
        }
        // the keys a phone keyboard lacks
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).horizontalScroll(rememberScrollState()).testTag("term.keys"), verticalAlignment = Alignment.CenterVertically) {
            @Composable fun K(label: String, on: Boolean = false, tag: String = label, run: () -> Unit) {
                Box(Modifier.heightIn(min = 44.dp).widthIn(min = 44.dp).background(if (on) dcs.primary else Color.Transparent).clickable(onClick = run).padding(horizontal = 10.dp).testTag("term.key.$tag"), contentAlignment = Alignment.Center) {
                    Text(label, color = if (on) dcs.onPrimary else dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                }
            }
            K(Msg.native_key_esc, tag = "Esc") { press(65307, 27) }; K(Msg.native_key_tab, tag = "Tab") { press(65289, 9) }; K("Ctrl", ctrl) { ctrl = !ctrl }
            K("←", tag = "left") { press(65361) }; K("↑", tag = "up") { press(65362) }; K("→", tag = "right") { press(65363) }; K("↓", tag = "down") { press(65364) }
            K("|", tag = "pipe") { typed('|'.code) }; K("~", tag = "tilde") { typed('~'.code) }; K("/", tag = "slash") { typed('/'.code) }; K("-", tag = "dash") { typed('-'.code) }
            K(Msg.android_paste, tag = "Paste") { paste() }
        }
    }
}

// ---- the Terminals list of a project, or of the hub

@Composable
internal fun TermsScreen(project: String?, back: () -> Unit, open: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var rows by remember(project) { mutableStateOf<List<Terms.Row>?>(null) }
    var err by remember { mutableStateOf("") }
    var epoch by remember { mutableIntStateOf(0) }
    LaunchedEffect(project, epoch, DataClient.conn) {
        if (DataClient.conn != DataClient.Conn.ONLINE) return@LaunchedEffect
        try { rows = Terms.list(project); err = "" } catch (e: Exception) { err = e.message ?: "" }
    }
    val title = project?.let { p -> DataClient.projects.firstOrNull { it.id == p }?.title }
    Column(Modifier.fillMaxSize().background(dcs.surfaceVariant)) {
        DBar(if (title != null) Msg.android_terminals_in(title) else Msg.android_terminals, back) {
            IconButton({ epoch++ }, modifier = Modifier.testTag("terms.refresh")) { Icon(Icons.Default.Refresh, Msg.common_refresh, tint = dcs.onSurfaceVariant) }
        }
        Row(Modifier.fillMaxWidth().padding(10.dp), verticalAlignment = Alignment.CenterVertically) {
            DBtn(Msg.android_term_new, true, DataClient.conn == DataClient.Conn.ONLINE, Modifier.testTag("terms.new")) {
                scope.launch { try { open(Terms.newShell(project, 80, 24)) } catch (e: Exception) { err = e.message ?: "" } }
            }
            if (err.isNotEmpty()) { Spacer(Modifier.width(10.dp)); DMono(err, dcs.error, 12, Modifier.weight(1f)) }
        }
        DLine()
        val list = rows
        if (list == null) DMono(if (DataClient.conn == DataClient.Conn.ONLINE) Msg.viewer_loading else Msg.android_offline_cap, dfaint, 12, Modifier.padding(14.dp))
        else if (list.isEmpty()) DMono(Msg.android_terms_none, dfaint, 12, Modifier.padding(14.dp).testTag("terms.none"))
        else LazyColumn(Modifier.fillMaxSize().testTag("terms.list")) {
            items(list, key = { it.key }) { r ->
                Column(Modifier.fillMaxWidth().clickable { open(r.key) }.testTag("terms.row.${r.key}")) {
                    Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        DDot(termStateColor(r.state)); Spacer(Modifier.width(10.dp))
                        Column(Modifier.weight(1f)) {
                            DMono(r.title.ifEmpty { r.key }, dcs.onSurface, 13, bold = true)
                            if (r.cwd.isNotEmpty()) DMono(r.cwd, dfaint, 11)
                        }
                        Column(horizontalAlignment = Alignment.End) { DMono(termStateLabel(r.state), termStateColor(r.state), 11); DMono(Tr.t(r.kind), dfaint, 11) }
                    }
                    DLine()
                }
            }
        }
    }
}
