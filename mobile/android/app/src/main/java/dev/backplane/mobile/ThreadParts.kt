package dev.backplane.mobile

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.provider.OpenableColumns
import android.util.LruCache
import android.webkit.MimeTypeMap
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.CheckCircleOutline
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.automirrored.filled.CallSplit
import androidx.compose.material.icons.automirrored.filled.Undo
import androidx.compose.material.icons.filled.VerticalSplit
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material.icons.filled.ShoppingCart
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.automirrored.filled.Chat
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.BrokenImage
import androidx.compose.material.icons.filled.Checklist
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.Difference
import androidx.compose.material.icons.filled.FindInPage
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Snooze
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material.icons.filled.Error
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.PanTool
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.QuestionAnswer
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL

// The pieces of a thread and the sheets over it, as iOS's ThreadParts.
// Every label, value and choice comes from the screen
// (src/mobile/screen.bend); these only draw them and send back the action
// each names.

private val corner = RoundedCornerShape(4.dp)
private val mono = FontFamily.Monospace

// the hub's palette for a thread's phase (src/web/style.css, src/app/theme.bend);
// a thread's activity: working accent (blue), finished ok (green), wanting
// you bad (red), stopped, idle or settled faint (grey) (style.css's --st-*)
object PhaseColor {
    val accent = Color(0xFF5FB3FF)
    val ok = Color(0xFF58C28A)
    val warn = Color(0xFFE3B341)
    val bad = Color(0xFFF06A6A)
    val input = Color(0xFFB48CFF)
    val queued = Color(0xFF4FC1C9)
    val waiting = Color(0xFFE07BD0)
    val faint = Color(0xFF5C6473)
}

// The projected activity wins over the legacy turn fallback. Waiting/queued
// data alone never proves a worker is running.
private fun threadStatusSignal(state: String, status: String): String = when (status) {
    "" -> when (state) { "run" -> "active"; "fail" -> "blocked"; else -> "inactive" }
    "blocked", "needs you", "approval", "input", "failed" -> "blocked"
    "active", "running", "watching", "cancelling", "working", "monitoring" -> "active"
    "finished", "complete" -> "finished"
    "settled", "done", "completed", "cancelled" -> "settled"
    else -> "inactive"
}

@Composable
fun StatusDot(state: String, status: String) {
    val lang = LocalBackplaneLanguage.current
    val signal = threadStatusSignal(state, status)
    val label = when (signal) {
        "blocked" -> if (lang == "ja") "対応が必要です" else "Blocked: needs interaction"
        "active" -> if (lang == "ja") "作業中" else "Working: agent or owned work active"
        "settled" -> if (lang == "ja") "完了済み" else "Settled"
        "finished" -> if (lang == "ja") "完了しました" else "Finished: ready for you to look at"
        else -> if (lang == "ja") "非稼働・実行中の作業は未確認です" else "Inactive: no confirmed active work"
    }
    Box(Modifier.size(24.dp).semantics(mergeDescendants = true) { contentDescription = label }, contentAlignment = Alignment.Center) {
        when (signal) {
            "blocked" -> Icon(Icons.Filled.Error, null, Modifier.size(18.dp), tint = PhaseColor.bad)
            "active" -> CircularProgressIndicator(Modifier.size(16.dp), color = PhaseColor.accent, strokeWidth = 2.dp)
            "finished" -> Icon(Icons.Filled.CheckCircle, null, Modifier.size(16.dp), tint = PhaseColor.ok)
            "settled" -> Icon(Icons.Filled.CheckCircleOutline, null, Modifier.size(16.dp), tint = PhaseColor.faint)
            else -> Icon(Icons.Filled.Pause, null, Modifier.size(16.dp), tint = PhaseColor.faint)
        }
    }
}

@Composable
private fun Dot(c: Color) = Box(Modifier.size(8.dp).clip(CircleShape).background(c))

// Images from the hub, kept by address and size (the bytes they hold)
private object Images {
    val cache = object : LruCache<String, Bitmap>(48 shl 20) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount
    }

    // an image at most max px on its longer side, or null
    fun load(url: String, max: Int): Bitmap? {
        val key = "$max|$url"
        cache.get(key)?.let { return it }
        val bytes = runCatching {
            val c = URL(url).openConnection() as HttpURLConnection
            c.connectTimeout = 5000
            c.readTimeout = 20000
            try {
                if (c.responseCode == 200) c.inputStream.use { it.readBytes() } else null
            } finally {
                c.disconnect()
            }
        }.getOrNull() ?: return null
        val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, o)
        var sample = 1
        while (maxOf(o.outWidth, o.outHeight) / (sample * 2) >= max) sample *= 2
        val b = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })
            ?: return null
        cache.put(key, b)
        return b
    }
}

// what an image load has come to: loading (null), failed or an image
private sealed interface Got {
    data object Failed : Got
    data class Ok(val b: Bitmap) : Got
}

@Composable
private fun hubImage(url: String, max: Int): Got? =
    produceState<Got?>(null, url, max) {
        value = withContext(Dispatchers.IO) { Images.load(url, max) }?.let { Got.Ok(it) } ?: Got.Failed
    }.value

// an image from the hub filling its width (a part's render); a tap opens it
@Composable
fun HubPicture(m: AppModel, path: String, modifier: Modifier, show: (String) -> Unit) {
    val lang = LocalBackplaneLanguage.current
    val u = m.web(path) ?: return
    when (val g = hubImage(u, 1200)) {
        null -> Box(modifier.aspectRatio(4f / 3f), contentAlignment = Alignment.Center) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
        }
        Got.Failed -> Box(modifier.aspectRatio(4f / 3f), contentAlignment = Alignment.Center) {
            Icon(Icons.Filled.BrokenImage, NativeMessages.native_image_missing(lang), tint = MaterialTheme.colorScheme.outline)
        }
        is Got.Ok -> Image(g.b.asImageBitmap(), NativeMessages.native_image_render(lang), modifier.aspectRatio(g.b.width.toFloat() / maxOf(g.b.height, 1))
            .clickable { show(u) }, contentScale = ContentScale.Fit)
    }
}

// an image from the hub, as a thumbnail; a tap opens it full screen
@Composable
fun Thumb(m: AppModel, path: String, show: (String) -> Unit) {
    val lang = LocalBackplaneLanguage.current
    val u = m.web(path) ?: return
    when (val g = hubImage(u, 720)) {
        null -> Box(Modifier.size(80.dp, 60.dp), contentAlignment = Alignment.Center) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
        }
        Got.Failed -> Box(Modifier.size(80.dp, 60.dp), contentAlignment = Alignment.Center) {
            Icon(Icons.Filled.BrokenImage, NativeMessages.native_image_missing(lang), tint = MaterialTheme.colorScheme.outline)
        }
        is Got.Ok -> Image(g.b.asImageBitmap(), NativeMessages.native_image_plain(lang), Modifier.widthIn(max = 260.dp).heightIn(max = 200.dp)
            .clip(corner).clickable { show(u) }, contentScale = ContentScale.Fit)
    }
}

// A composer preview is decorative; its enclosing button removes the file.
@Composable
private fun PendingThumb(m: AppModel, path: String) {
    val url = m.web(path) ?: return
    Box(Modifier.size(96.dp, 72.dp), contentAlignment = Alignment.Center) {
        when (val image = hubImage(url, 192)) {
            null -> CircularProgressIndicator(Modifier.size(18.dp))
            Got.Failed -> Icon(Icons.Filled.BrokenImage, "Preview unavailable")
            is Got.Ok -> Image(image.b.asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit)
        }
    }
}

// how many dialogs the app shows on its own, outside the screen model (a
// picture opened large, a diff sheet, a confirmation): the first-run tour
// waits until there are none. Each one calls Overlaid() while it is shown.
object Overlay {
    var n by mutableIntStateOf(0)
}

@Composable
fun Overlaid() {
    DisposableEffect(Unit) {
        Overlay.n++
        onDispose { Overlay.n-- }
    }
}

// pinch to zoom, drag to pan, double-tap to zoom in or back, the cross (or
// back) closes it
@Composable
fun Lightbox(url: String, close: () -> Unit) {
    val lang = LocalBackplaneLanguage.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val image = hubImage(url, 2560)
    var saveError by remember(url) { mutableStateOf(false) }
    // The system picker grants only the chosen document. Save pixels,
    // never the authenticated hub URL, with no storage permission.
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("image/png")) { uri ->
        if (uri != null) scope.launch {
            saveError = !withContext(Dispatchers.IO) {
                runCatching {
                    val bitmap = (image as? Got.Ok)?.b ?: error("Image unavailable")
                    context.contentResolver.openOutputStream(uri)?.use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) } == true
                }.getOrDefault(false)
            }
        }
    }
    Overlaid()
    Dialog(close, DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        var scale by remember { mutableFloatStateOf(1f) }
        var offset by remember { mutableStateOf(Offset.Zero) }
        Box(Modifier.fillMaxSize().background(Color.Black)
            .pointerInput(Unit) {
                detectTapGestures(onDoubleTap = {
                    scale = if (scale > 1f) 1f else 2.5f
                    if (scale == 1f) offset = Offset.Zero
                })
            }
            .pointerInput(Unit) {
                detectTransformGestures { _, pan, zoom, _ ->
                    scale = (scale * zoom).coerceIn(1f, 8f)
                    offset = if (scale == 1f) Offset.Zero else offset + pan
                }
            }) {
            when (val g = image) {
                null -> CircularProgressIndicator(Modifier.align(Alignment.Center))
                Got.Failed -> Icon(Icons.Filled.BrokenImage, NativeMessages.native_image_missing(lang), Modifier.align(Alignment.Center), tint = Color.Gray)
                is Got.Ok -> Image(g.b.asImageBitmap(), NativeMessages.native_image_plain(lang), Modifier.fillMaxSize().graphicsLayer(
                    scaleX = scale, scaleY = scale, translationX = offset.x, translationY = offset.y), contentScale = ContentScale.Fit)
            }
            IconButton(close, Modifier.align(Alignment.TopEnd).statusBarsPadding().padding(8.dp)) {
                Icon(Icons.Filled.Close, NativeMessages.common_close(lang), tint = Color.White)
            }
            TextButton(onClick = { saveError = false; save.launch("image.png") }, enabled = image is Got.Ok,
                modifier = Modifier.align(Alignment.TopStart).statusBarsPadding().padding(8.dp)) {
                Text(NativeMessages.common_save(lang), color = Color.White)
            }
            if (saveError) Text(if (lang == "ja") "画像を保存できませんでした" else "Image could not be saved.",
                color = Color.White, modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(16.dp))
        }
    }
}

// an attachment chip; an image shows as a thumbnail
@Composable
fun ChipView(m: AppModel, c: Chip, show: (String) -> Unit) {
    if (c.image) Thumb(m, c.url, show)
    else Row(Modifier.background(MaterialTheme.colorScheme.surfaceVariant, corner).padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        Icon(Icons.Filled.Description, null, Modifier.size(14.dp))
        Text(c.label, style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

// a message's long press: Copy takes it whole, Select text opens it where
// any part of it can be selected and copied
@Composable
private fun MessageMenu(m: AppModel, text: String, open: Boolean, close: () -> Unit) {
    val lang = LocalBackplaneLanguage.current
    var selecting by remember { mutableStateOf(false) }
    DropdownMenu(expanded = open, onDismissRequest = close) {
        DropdownMenuItem(text = { Text(NativeMessages.common_copy(lang)) }, onClick = { close(); m.act("copy", text) })
        DropdownMenuItem(text = { Text(NativeMessages.native_text_select_action(lang)) }, onClick = { close(); selecting = true })
    }
    if (selecting) {
        Overlaid()
        AlertDialog(
            onDismissRequest = { selecting = false },
            text = {
                SelectionContainer {
                    Text(text, Modifier.heightIn(max = 480.dp).verticalScroll(rememberScrollState()), style = MaterialTheme.typography.bodyLarge)
                }
            },
            confirmButton = { TextButton(onClick = { selecting = false }) { Text(NativeMessages.tour_done(lang)) } },
            dismissButton = { TextButton(onClick = { m.act("copy", text) }) { Text(NativeMessages.native_text_copyall_action(lang)) } },
        )
    }
}

// when a message was sent, or a turn finished (core/stamp.bend)
@Composable
private fun Stamp(s: String) {
    if (s.isNotEmpty()) Text(s, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
}

@Composable
fun EntryRow(m: AppModel, e: Entry, show: (String) -> Unit) {
    val lang = LocalBackplaneLanguage.current
    var menu by remember(e.text) { mutableStateOf(false) }
    when (e.kind) {
        "user" -> Column(Modifier.fillMaxWidth().padding(start = 48.dp), horizontalAlignment = Alignment.End,
            verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (e.text.isNotEmpty()) Box {
                Surface(
                    color = MaterialTheme.colorScheme.primaryContainer,
                    shape = MaterialTheme.shapes.large,
                    modifier = Modifier.combinedClickableCompat(onLong = { menu = true }) {},
                ) { Text(e.text, Modifier.padding(horizontal = 14.dp, vertical = 10.dp), style = MaterialTheme.typography.bodyLarge) }
                MessageMenu(m, e.text, menu) { menu = false }
            }
            for (c in e.attachments) ChipView(m, c, show)
            Stamp(e.stamp)
        }
        "assistant" -> Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Box {
                Markdown(e.blocks, Modifier.fillMaxWidth().combinedClickableCompat(onLong = { menu = true }) {})
                MessageMenu(m, e.text, menu) { menu = false }
            }
            for (i in e.images) Thumb(m, i.url, show)
            Stamp(e.stamp)
        }
        "fold" -> Row(Modifier.fillMaxWidth().clickable { m.act("fold", e.value) }.padding(vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Icon(if (e.open) Icons.Filled.ExpandMore else Icons.AutoMirrored.Filled.KeyboardArrowRight,
                if (e.open) NativeMessages.common_hide(lang) else NativeMessages.common_show(lang), Modifier.size(16.dp), tint = MaterialTheme.colorScheme.outline)
            Text(e.text, style = MaterialTheme.typography.labelMedium.copy(fontFamily = mono),
                color = MaterialTheme.colorScheme.outline, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        "link" -> Text(e.text, Modifier.clickable { m.act("select", e.value) }.padding(vertical = 2.dp),
            style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.primary,
            maxLines = 1, overflow = TextOverflow.Ellipsis)
        // a reminder the hub delivered: a bell, Reminder, the text and when
        "reminder" -> Row(Modifier.fillMaxWidth().padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Icon(Icons.Filled.Notifications, NativeMessages.sched_kind_reminder(lang), Modifier.size(16.dp), tint = MaterialTheme.colorScheme.primary)
            Text(e.text, Modifier.weight(1f), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
            Text(e.at, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
        }
        // a design step: shows it in the viewer
        "step" -> Row(Modifier.fillMaxWidth().clickable { m.act("hist-show", e.value) }.padding(vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Icon(Icons.Filled.History, NativeMessages.native_entry_viewer_short(lang), Modifier.size(16.dp), tint = MaterialTheme.colorScheme.primary)
            Text(e.text, style = MaterialTheme.typography.labelMedium.copy(fontFamily = mono),
                color = MaterialTheme.colorScheme.primary, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        else -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val color = if (e.tone == "error") MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.outline
            val style = MaterialTheme.typography.labelMedium.copy(fontFamily = mono)
            Text(e.label, style = style, color = color.copy(alpha = 0.7f))
            Text(e.text, style = style, color = color, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
    }
}

private fun subColor(state: String) = when (state) {
    "active", "running", "watching", "cancelling" -> PhaseColor.accent
    "blocked", "needs you" -> PhaseColor.bad
    "finished" -> PhaseColor.ok
    else -> PhaseColor.faint
}

// the subagent panel: what this thread delegated (a tap opens its thread)
// and the agent's own (a tap opens or shuts its steps); outlined in the
// accent while any are at work
@Composable
fun SubsView(m: AppModel, subs: Subs) {
    val lang = LocalBackplaneLanguage.current
    val busy = subs.busy.isNotEmpty()
    Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = corner, modifier = Modifier.fillMaxWidth()
        .then(if (busy) Modifier.border(1.dp, PhaseColor.accent, corner) else Modifier)) {
        Column(Modifier.padding(10.dp).heightIn(max = 260.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            // the head opens and shuts the panel; done, it is folded to this line
            Row(Modifier.fillMaxWidth().then(if (subs.act.isNotEmpty()) Modifier.clickable { m.act(subs.act, subs.value) } else Modifier),
                verticalAlignment = Alignment.CenterVertically) {
                Text(NativeMessages.thread_subagents(lang), Modifier.weight(1f), style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.Bold,
                    color = if (busy) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.outline)
                Text(subs.word.ifEmpty { subs.busy }, style = MaterialTheme.typography.labelMedium,
                    color = if (busy) PhaseColor.accent else MaterialTheme.colorScheme.outline)
                Icon(if (subs.open) Icons.Filled.ExpandMore else Icons.AutoMirrored.Filled.KeyboardArrowRight, null,
                    Modifier.size(18.dp), tint = MaterialTheme.colorScheme.outline)
            }
            for (r in subs.rows) Column(Modifier.fillMaxWidth().clickable { m.act(r.act, r.value) }.padding(vertical = 4.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    StatusDot("", r.state)
                    Text(if (r.agent) (if (r.open) "▾" else "▸") else r.who, style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.outline, maxLines = 1)
                    Text(r.title, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (r.cancel.isNotEmpty()) Text(if (r.cancel == "wake-cancel") NativeMessages.common_cancel(lang) else NativeMessages.composer_stop(lang), Modifier.clickable { m.act(r.cancel, r.id) }.padding(horizontal = 4.dp),
                        style = MaterialTheme.typography.labelSmall, color = PhaseColor.warn)
                    Text(r.stateLabel.ifEmpty { r.state }, style = MaterialTheme.typography.labelSmall, color = subColor(r.state))
                }
                if (r.doing.isNotEmpty()) Text(r.doing, Modifier.padding(start = 16.dp), style = MaterialTheme.typography.labelSmall,
                    color = PhaseColor.accent, maxLines = 1, overflow = TextOverflow.Ellipsis)
                for (st in r.steps) Row(Modifier.padding(start = 16.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    val style = MaterialTheme.typography.labelSmall.copy(fontFamily = mono)
                    if (st.kind.isNotEmpty()) Text(st.kind, style = style, color = MaterialTheme.colorScheme.outline.copy(alpha = 0.7f))
                    Text(st.text, style = style, color = MaterialTheme.colorScheme.outline, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

// an approval, question or plan waiting on the user
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AskCard(m: AppModel, a: Ask) {
    if (a.kind == "browser") { BrowserAsk(m, a); return }
    Surface(color = PhaseColor.bad.copy(alpha = 0.14f), shape = corner, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(when (a.kind) { "plan" -> Icons.Filled.Checklist; "input" -> Icons.Filled.QuestionAnswer; else -> Icons.Filled.PanTool },
                    null, Modifier.size(18.dp), tint = PhaseColor.bad)
                Text(a.head, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
            }
            if (a.blocks.isNotEmpty()) Box(Modifier.heightIn(max = 220.dp).verticalScroll(rememberScrollState())) { Markdown(a.blocks) }
            else if (a.detail.isNotEmpty()) Box(Modifier.heightIn(max = 300.dp).verticalScroll(rememberScrollState())) {
                Text(a.detail, style = MaterialTheme.typography.labelMedium.copy(fontFamily = mono))
            }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (b in a.buttons) {
                    if (b.primary) Button(onClick = { m.act(b.action.ifEmpty { "answer" }, b.value) }, shape = corner) { Text(b.label) }
                    else OutlinedButton(onClick = { m.act(b.action.ifEmpty { "answer" }, b.value) }, shape = corner) { Text(b.label) }
                }
            }
        }
    }
}

// what the next message attaches (× takes one off), what is uploading,
// and what the word being typed completes to: a `$` skill, an `@` bot,
// a `>` thread, a `%` project or a `#` room, each with its summary
@Composable
fun ComposerExtras(m: AppModel, t: ThreadView) {
    val lang = LocalBackplaneLanguage.current
    t.btw?.let { b ->
        Surface(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp), shape = corner,
            color = MaterialTheme.colorScheme.surfaceVariant) {
            Column(Modifier.padding(start = 12.dp, bottom = 10.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(NativeMessages.native_composer_btw(lang, b.q), Modifier.weight(1f), style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.outline, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    IconButton(onClick = { m.act("btw-close") }) { Icon(Icons.Filled.Close, NativeMessages.common_close(lang)) }
                }
                Text(b.a, Modifier.padding(end = 12.dp).heightIn(max = 240.dp).verticalScroll(rememberScrollState()),
                    style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
    if (t.skills.isNotEmpty()) Surface(Modifier.fillMaxWidth(), color = MaterialTheme.colorScheme.surfaceContainer) {
        // Bend marks the highlighted row; the first one is it only when none is marked
        val hot = t.skills.indexOfFirst { it.on }.coerceAtLeast(0)
        val list = rememberLazyListState()
        LaunchedEffect(hot) { list.animateScrollToItem(hot) }
        // about five rows, fewer when the keyboard leaves little room (landscape)
        val density = androidx.compose.ui.platform.LocalDensity.current
        val ime = with(density) { WindowInsets.ime.getBottom(density).toDp() }
        val room = androidx.compose.ui.platform.LocalConfiguration.current.screenHeightDp.dp - ime
        val cap = (room * 0.4f).coerceIn(48.dp, 240.dp)
        Column {
            HorizontalDivider()
            Box {
                LazyColumn(Modifier.fillMaxWidth().heightIn(max = cap), state = list) {
                    itemsIndexed(t.skills) { i, k ->
                        val sel = i == hot
                        val label = if (k.desc.isEmpty()) k.name else k.name + ", " + k.desc
                        Surface(onClick = { m.act("skill", k.name) }, shape = corner,
                            modifier = Modifier.fillMaxWidth().semantics { contentDescription = label; selected = sel },
                            color = if (sel) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent) {
                            Column(Modifier.heightIn(min = 48.dp).padding(start = 12.dp, end = 56.dp, top = 6.dp, bottom = 6.dp),
                                verticalArrangement = Arrangement.Center) {
                                Text(k.name, style = MaterialTheme.typography.labelLarge, fontFamily = FontFamily.Monospace,
                                    fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis,
                                    color = if (sel) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.onSurface)
                                if (k.desc.isNotEmpty()) Text(k.desc, style = MaterialTheme.typography.bodySmall, maxLines = 2,
                                    overflow = TextOverflow.Ellipsis,
                                    color = if (sel) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
                IconButton(onClick = { m.act("skill-close") }, Modifier.align(Alignment.TopEnd).size(48.dp)) {
                    Icon(Icons.Filled.Close, NativeMessages.native_composer_suggestions_close(lang), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
    if (t.attaching.isNotEmpty() || t.uploading.isNotEmpty()) Row(
        Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        for (c in t.attaching) Surface(onClick = { m.act("detach", c.path) }, shape = corner,
            color = MaterialTheme.colorScheme.surfaceVariant) {
            Row(Modifier.padding(horizontal = 8.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                if (c.image) PendingThumb(m, c.url) else Icon(Icons.Filled.Description, null, Modifier.size(14.dp))
                Text(c.label, style = MaterialTheme.typography.labelMedium, maxLines = 1)
                Icon(Icons.Filled.Close, NativeMessages.native_composer_remove(lang, c.label), Modifier.size(14.dp))
            }
        }
        if (t.uploading.isNotEmpty()) {
            CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 2.dp)
            Text(t.uploading, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline, maxLines = 1)
        }
    }
}

// Photo picks, clipboard images and keyboard content share one loader.
internal suspend fun attachUris(ctx: android.content.Context, m: AppModel, uris: List<Uri>, photos: Boolean) {
    uris.forEachIndexed { i, u ->
        val got = withContext(Dispatchers.IO) {
            runCatching {
                val cr = ctx.contentResolver
                val data = cr.openInputStream(u)?.use { it.readBytes() } ?: return@runCatching null
                val mime = cr.getType(u) ?: ""
                if (photos) {
                    val ext = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime) ?: "jpg"
                    // HEIC becomes JPEG, which every agent reads
                    if (ext == "heic" || ext == "heif") {
                        val b = BitmapFactory.decodeByteArray(data, 0, data.size)
                        val out = ByteArrayOutputStream()
                        if (b != null && b.compress(Bitmap.CompressFormat.JPEG, 85, out)) out.toByteArray() to "photo-${i + 1}.jpg"
                        else data to "photo-${i + 1}.$ext"
                    } else data to "photo-${i + 1}.$ext"
                } else {
                    val name = cr.query(u, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                        if (c.moveToFirst()) c.getString(0) else null
                    } ?: u.lastPathSegment ?: "file"
                    data to name
                }
            }.getOrNull()
        }
        if (got != null) m.attach(got.first, got.second)
    }
}

// the paperclip: photos or files, each sent up in pieces
@Composable
fun AttachButton(m: AppModel) {
    val lang = LocalBackplaneLanguage.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var open by remember { mutableStateOf(false) }
    fun send(uris: List<Uri>, photos: Boolean) {
        scope.launch { attachUris(ctx, m, uris, photos) }
    }
    val photos = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia(10)) { send(it, true) }
    val files = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { send(it, false) }
    Box {
        IconButton(onClick = { open = true }) { Icon(Icons.Filled.AttachFile, NativeMessages.composer_attach(lang)) }
        DropdownMenu(open, { open = false }) {
            DropdownMenuItem(text = { Text(NativeMessages.native_attach_photos(lang)) }, leadingIcon = { Icon(Icons.Filled.Image, null) }, onClick = {
                open = false
                photos.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
            })
            DropdownMenuItem(text = { Text(NativeMessages.viewer_files(lang)) }, leadingIcon = { Icon(Icons.Filled.Folder, null) }, onClick = {
                open = false
                files.launch(arrayOf("*/*"))
            })
        }
    }
}

// a sheet over the whole screen: a top bar and its content; back closes it
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Sheet(
    title: String, close: () -> Unit, closeLabel: String? = null,
    bar: Color = Color.Unspecified, onBar: Color = Color.Unspecified,
    // a dialog, or (inline) drawn in place of the screen, which follows the
    // keyboard as the thread's composer does
    inline: Boolean = false,
    actions: @Composable () -> Unit = {}, content: @Composable (PaddingValues) -> Unit,
) {
    val lang = LocalBackplaneLanguage.current
    val resolvedClose = closeLabel ?: NativeMessages.common_close(lang)
    if (inline) SheetBody(title, close, resolvedClose, bar, onBar, actions, content)
    else Dialog(close, DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Overlaid()
        SheetBody(title, close, resolvedClose, bar, onBar, actions, content)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SheetBody(
    title: String, close: () -> Unit, closeLabel: String, bar: Color, onBar: Color,
    actions: @Composable () -> Unit, content: @Composable (PaddingValues) -> Unit,
) {
        Scaffold(topBar = {
            TopAppBar(
                title = { Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = close) { Icon(Icons.Filled.Close, closeLabel) } },
                actions = { actions() },
                colors = if (bar == Color.Unspecified) TopAppBarDefaults.topAppBarColors() else TopAppBarDefaults.topAppBarColors(
                    containerColor = bar, titleContentColor = onBar, navigationIconContentColor = onBar, actionIconContentColor = onBar),
            )
        }) { content(it) }
}

// what the thread changed, with the git actions
@Composable
fun DiffSheet(m: AppModel, d: Diff) {
    val lang = LocalBackplaneLanguage.current
    var git by remember { mutableStateOf(false) }
    var reverting by remember { mutableStateOf(false) }
    Sheet(NativeMessages.viewer_diff(lang), { m.act("panel") }, actions = {
        Box {
            TextButton(onClick = { git = true }) { Text("Git") }
            DropdownMenu(git, { git = false }) {
                DropdownMenuItem(text = { Text(NativeMessages.palette_commit(lang)) }, onClick = { git = false; m.act("git", "commit_push") })
                DropdownMenuItem(text = { Text(NativeMessages.viewer_diff_pr(lang)) }, onClick = { git = false; m.act("git", "commit_push_pr") })
                DropdownMenuItem(text = { Text(NativeMessages.palette_revert(lang), color = MaterialTheme.colorScheme.error) },
                    onClick = { git = false; reverting = true })
            }
        }
    }) { pad ->
        LazyColumn(Modifier.fillMaxSize(), contentPadding = pad) {
            item { Text(d.summary, Modifier.padding(16.dp), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline) }
            for (f in d.files) {
                item {
                    Row(Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surfaceVariant).padding(horizontal = 12.dp, vertical = 6.dp)) {
                        Text(f.name, Modifier.weight(1f), style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(f.status, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.outline)
                    }
                }
                items(f.lines) { l -> DiffRow(l) }
            }
        }
    }
    if (reverting) Overlaid()
    if (reverting) AlertDialog(
        onDismissRequest = { reverting = false },
        title = { Text(NativeMessages.native_diff_revert_ask(lang)) },
        confirmButton = {
            TextButton(onClick = { reverting = false; m.act("revert", "0") }) { Text(NativeMessages.palette_revert(lang), color = MaterialTheme.colorScheme.error) }
        },
        dismissButton = { TextButton(onClick = { reverting = false }) { Text(NativeMessages.common_cancel(lang)) } },
    )
}

@Composable
private fun DiffRow(l: DiffLine) {
    val style = TextStyle(fontFamily = mono, fontSize = 11.sp, lineHeight = 14.sp)
    val bg = when (l.k) { 1 -> Color(0x2443A047); 2 -> Color(0x24E53935); else -> Color.Transparent }
    Row(Modifier.fillMaxWidth().background(bg).padding(horizontal = 8.dp)) {
        if (l.k == 4) Text(l.t, style = style, color = Color(0xFF1E88E5))
        else {
            Text(if (l.k == 2) l.o else l.n, Modifier.width(30.dp).padding(end = 4.dp), style = style,
                color = MaterialTheme.colorScheme.outline, textAlign = androidx.compose.ui.text.style.TextAlign.End)
            Text((when (l.k) { 1 -> "+"; 2 -> "-"; else -> " " }) + l.t, style = style,
                color = if (l.k == 3) MaterialTheme.colorScheme.outline else MaterialTheme.colorScheme.onSurface)
        }
    }
}

private fun rgb(c: Int) = Color(0xFF000000.toInt() or c)

private val termStyle = TextStyle(fontFamily = mono, fontSize = 11.sp, lineHeight = 14.sp)

// the size a terminal has room for on this phone, as "<cols>x<rows>"
@Composable
fun rememberTermSize(): () -> String {
    val measure = rememberTextMeasurer()
    val density = LocalDensity.current
    val conf = androidx.compose.ui.platform.LocalConfiguration.current
    return {
        val cell = measure.measure("M", termStyle).size
        val w = with(density) { conf.screenWidthDp.dp.toPx() - 16.dp.toPx() }
        val h = with(density) { conf.screenHeightDp.dp.toPx() * 0.5f }
        "${maxOf((w / cell.width).toInt(), 20)}x${maxOf((h / cell.height).toInt(), 8)}"
    }
}

// a terminal's screen: its rows of styled runs and the cursor, in style
@Composable
fun TermScreen(t: Term, style: TextStyle, modifier: Modifier = Modifier) {
    val measure = rememberTextMeasurer()
    val cell = remember(style) { measure.measure("M", style).size }
    val density = LocalDensity.current
    val fg = rgb(t.fg)
    Box(modifier) {
        Column {
            for (l in t.lines) Text(buildAnnotatedString {
                for (r in l) withStyle(SpanStyle(
                    color = rgb(r.fg),
                    background = if (r.bg != t.bg) rgb(r.bg) else Color.Unspecified,
                    fontWeight = if (r.b) FontWeight.Bold else null,
                    textDecoration = if (r.u) TextDecoration.Underline else null,
                )) { append(r.t) }
            }, style = style, color = fg, softWrap = false, maxLines = 1)
        }
        if (t.cursor.on) Box(Modifier
            .offset { IntOffset(t.cursor.x * cell.width, t.cursor.y * cell.height) }
            .size(with(density) { cell.width.toDp() }, with(density) { cell.height.toDp() })
            .background(fg.copy(alpha = 0.6f)))
    }
}

// the hidden field a terminal's keys are typed in: what the field held
// against what it holds now: what went is backspaced, what came is typed.
// It starts with a space, so a backspace always has something to take,
// and starts over when emptied or long (never as the value it has, which
// the field would not take up)
@Composable
fun TermField(focus: FocusRequester, key: (String, Int) -> Unit, paste: (String) -> Unit, modifier: Modifier = Modifier) {
    var buf by remember { mutableStateOf(TextFieldValue(" ", TextRange(1))) }
    BasicTextField(buf, { v ->
        val old = buf.text
        val new = v.text
        var same = 0
        while (same < old.length && same < new.length && old[same] == new[same]) same++
        repeat(old.length - same) { key("Backspace", 0) }
        val parts = new.substring(same).split('\n')
        parts.forEachIndexed { i, p ->
            if (p.isNotEmpty()) paste(p)
            if (i < parts.size - 1) key("Enter", 0)
        }
        buf = if (new.isEmpty() || new.length > 200 || '\n' in new)
            (if (old == " ") TextFieldValue("  ", TextRange(2)) else TextFieldValue(" ", TextRange(1)))
        else v
    }, modifier.size(1.dp).focusRequester(focus), singleLine = true,
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false,
            keyboardType = KeyboardType.Password, imeAction = ImeAction.Send),
        keyboardActions = KeyboardActions(onSend = { key("Enter", 0) }),
        textStyle = TextStyle(color = Color.Transparent))
}

// the keys a phone keyboard lacks, and the keyboard's own toggle
@Composable
fun TermKeys(key: (String, Int) -> Unit, keyboard: () -> Unit) {
    val lang = LocalBackplaneLanguage.current
    Surface(tonalElevation = 3.dp) {
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp, vertical = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            @Composable fun k(label: String, go: () -> Unit) = OutlinedButton(onClick = go, shape = corner,
                contentPadding = PaddingValues(horizontal = 10.dp)) { Text(label, fontFamily = mono, style = MaterialTheme.typography.labelMedium) }
            @Composable fun ik(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, go: () -> Unit) =
                OutlinedButton(onClick = go, shape = corner, contentPadding = PaddingValues(horizontal = 10.dp)) {
                    Icon(icon, label, Modifier.size(16.dp))
                }
            k("esc") { key("Escape", 0) }
            k("tab") { key("Tab", 0) }
            k("^C") { key("c", 4) }
            k("^D") { key("d", 4) }
            k("^Z") { key("z", 4) }
            k("^L") { key("l", 4) }
            ik(Icons.AutoMirrored.Filled.ArrowBack, NativeMessages.native_key_left(lang)) { key("ArrowLeft", 0) }
            ik(Icons.Filled.ArrowUpward, NativeMessages.native_key_up(lang)) { key("ArrowUp", 0) }
            ik(Icons.Filled.ArrowDownward, NativeMessages.native_key_down(lang)) { key("ArrowDown", 0) }
            ik(Icons.AutoMirrored.Filled.ArrowForward, NativeMessages.native_key_right(lang)) { key("ArrowRight", 0) }
            ik(Icons.Filled.Keyboard, NativeMessages.native_key_keyboard(lang), keyboard)
        }
    }
}

// the thread's shell: the screen the hub's emulator keeps, keys typed in a
// hidden field, and a row of keys a phone keyboard lacks
@Composable
fun TermSheet(m: AppModel, t: Term) {
    val lang = LocalBackplaneLanguage.current
    val focus = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    var typing by remember { mutableStateOf(true) }
    val measure = rememberTextMeasurer()
    val cell = remember { measure.measure("M", termStyle).size }
    val density = LocalDensity.current
    val vs = rememberScrollState()
    val hs = rememberScrollState()
    val key: (String, Int) -> Unit = { k, mods -> m.quiet("term-key", "$k\t$mods") }
    // the cursor's row kept in view (the rows below it are mostly blank)
    val below = with(density) { 16.dp.roundToPx() }
    LaunchedEffect(t.cursor.y, vs.maxValue, vs.viewportSize) {
        vs.scrollTo(((t.cursor.y + 1) * cell.height + below - vs.viewportSize).coerceIn(0, vs.maxValue))
    }
    LaunchedEffect(Unit) { focus.requestFocus(); keyboard?.show() }
    val bg = rgb(t.bg)
    val fg = rgb(t.fg)
    Sheet(t.title.ifEmpty { NativeMessages.palette_terminal(lang) }, { m.act("term-toggle") }, bar = bg, onBar = fg, inline = true) { pad ->
        Column(Modifier.fillMaxSize().padding(pad).imePadding()) {
            Box(Modifier.weight(1f).fillMaxWidth().background(bg)
                .clickable { focus.requestFocus(); keyboard?.show(); typing = true }
                .verticalScroll(vs).horizontalScroll(hs).padding(8.dp)) {
                TermScreen(t, termStyle)
            }
            TermField(focus, key, { p -> m.quiet("term-paste", p) })
            TermKeys(key) {
                typing = !typing
                if (typing) { focus.requestFocus(); keyboard?.show() } else keyboard?.hide()
            }
        }
    }
}

// a terminal in the chat, live: a header (its title, the pin) over its
// screen, the font shrunk so its columns fit (7 sp at the least, then it
// scrolls sideways); a tap types into it ("emb-key", "emb-paste" with its
// key first). At the top of the thread (pinned) the pin is in the accent
// and the screen is held to a height, kept on the cursor.
@Composable
fun EmbView(m: AppModel, e: Emb, top: Boolean = false) {
    val lang = LocalBackplaneLanguage.current
    val focus = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    val focusManager = LocalFocusManager.current
    var typing by remember { mutableStateOf(false) }
    val key: (String, Int) -> Unit = { k, mods -> m.quiet("emb-key", "${e.key}\t$k\t$mods") }
    val t = e.term
    val bg = if (t != null) rgb(t.bg) else MaterialTheme.colorScheme.surfaceVariant
    val fg = if (t != null) rgb(t.fg) else MaterialTheme.colorScheme.onSurface
    Column(Modifier.fillMaxWidth().background(bg)) {
        Row(Modifier.fillMaxWidth().padding(start = 8.dp), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Icon(Icons.Filled.Terminal, null, Modifier.size(14.dp), tint = fg.copy(alpha = 0.7f))
            Text(e.title, Modifier.weight(1f), style = MaterialTheme.typography.labelMedium, color = fg,
                maxLines = 1, overflow = TextOverflow.Ellipsis)
            Box(Modifier.size(36.dp).clickable { m.act("emb-pin", e.key) }, contentAlignment = Alignment.Center) {
                Icon(Icons.Filled.PushPin, if (e.pinned) NativeMessages.menu_unpin(lang) else NativeMessages.terminal_pin(lang), Modifier.size(16.dp),
                    tint = if (e.pinned) PhaseColor.accent else fg.copy(alpha = 0.5f))
            }
        }
        if (t != null) BoxWithConstraints(Modifier.fillMaxWidth()) {
            val measure = rememberTextMeasurer()
            val density = LocalDensity.current
            val unit = remember { measure.measure("M", termStyle).size.width / 11f }
            val room = with(density) { maxWidth.toPx() - 8.dp.toPx() }
            val pt = if (unit > 0f) (room / e.cols.coerceAtLeast(1) / unit * 2f).toInt() / 2f else 11f
            val sz = pt.coerceIn(7f, 11f)
            val style = TextStyle(fontFamily = mono, fontSize = sz.sp, lineHeight = (sz * 14f / 11f).sp)
            val cellH = remember(style) { measure.measure("M", style).size.height }
            val vs = rememberScrollState()
            if (top) LaunchedEffect(t.cursor.y, vs.maxValue, vs.viewportSize) {
                vs.scrollTo(((t.cursor.y + 1) * cellH - vs.viewportSize).coerceIn(0, vs.maxValue))
            }
            Box(Modifier.fillMaxWidth()
                .then(if (top) Modifier.heightIn(max = 240.dp).verticalScroll(vs) else Modifier)
                .clickable { focus.requestFocus(); keyboard?.show() }
                .horizontalScroll(rememberScrollState()).padding(4.dp)) {
                TermScreen(t, style)
            }
        }
        TermField(focus, key, { p -> m.quiet("emb-paste", "${e.key}\t$p") }, Modifier.onFocusChanged { typing = it.isFocused })
        if (typing) TermKeys(key) { keyboard?.hide(); focusManager.clearFocus() }
    }
}

// a terminal's row in the timeline: live, its screen; pinned, a faint row
// that unpins it; else a gray row that starts it again
@Composable
fun EmbRow(m: AppModel, entry: Entry, e: Emb?) {
    val lang = LocalBackplaneLanguage.current
    val style = MaterialTheme.typography.labelMedium.copy(fontFamily = mono)
    val faint = MaterialTheme.colorScheme.outline
    when {
        e == null -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.Terminal, null, Modifier.size(14.dp), tint = faint)
            Text(entry.text, style = style, color = faint, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        e.pinned -> Row(Modifier.fillMaxWidth().clickable { m.act("emb-pin", e.key) }.padding(vertical = 2.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.PushPin, NativeMessages.menu_unpin(lang), Modifier.size(14.dp), tint = faint)
            Text(NativeMessages.web_terminal_pinned(lang, e.title), style = style, color = faint, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        e.live -> EmbView(m, e)
        else -> Row(Modifier.fillMaxWidth().clickable { m.act("emb-restart", e.restart) }.padding(vertical = 2.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.PlayArrow, NativeMessages.native_term_restart(lang), Modifier.size(14.dp), tint = faint)
            Text(NativeMessages.native_term_inactive(lang, e.title), style = style, color = faint, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

// thread search or the file picker: a field over the rows; a row sends its
// action with its value, then the sheet closes
@Composable
fun FindSheet(m: AppModel, f: Find) {
    val lang = LocalBackplaneLanguage.current
    var text by remember { mutableStateOf(f.query) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val files = f.mode == "files"
    Sheet(if (files) NativeMessages.palette_findfile(lang) else NativeMessages.palette_search(lang), { m.act("find-close") }, closeLabel = NativeMessages.common_cancel(lang)) { pad ->
        Column(Modifier.fillMaxSize().padding(pad).imePadding()) {
            OutlinedTextField(text, { text = it; m.act("find-q", it) },
                Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp).focusRequester(focus), singleLine = true,
                placeholder = { Text(if (files) NativeMessages.native_find_file_label(lang) else NativeMessages.native_find_threads_label(lang)) },
                leadingIcon = { Icon(Icons.Filled.Search, null) },
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false))
            if (f.rows.isEmpty()) Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text(if (f.query.isEmpty()) (if (files) NativeMessages.native_find_file_hint(lang) else NativeMessages.native_find_threads_hint(lang)) else NativeMessages.native_find_empty(lang),
                    color = MaterialTheme.colorScheme.outline)
            } else LazyColumn(Modifier.fillMaxSize()) {
                items(f.rows) { r ->
                    ListItem(
                        headlineContent = { Text(r.label, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                        leadingContent = {
                            Icon(when (r.kind) { "file" -> Icons.Filled.Description; "thread" -> Icons.AutoMirrored.Filled.Chat; else -> Icons.Filled.Bolt }, null)
                        },
                        // a message found: its thread opens on it (Core.seek)
                        modifier = Modifier.clickable { if (r.entry.isNotEmpty()) m.seek(r.value, r.entry); m.act(r.action, r.value); m.act("find-close") },
                    )
                }
            }
        }
    }
}

// the hub's settings, as the desktop has them
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun UsageDayChart(days: List<UsageDay>) {
    var expanded by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(Modifier.fillMaxWidth().height(180.dp), horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.Bottom) {
            days.forEach { day ->
                Box(Modifier.weight(1f).height(if (day.known) maxOf(2f, 180f * day.fraction / 10000f).dp else 180.dp)
                    .background(if (day.known) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline.copy(alpha = 0.15f))
                    .semantics { contentDescription = day.date + ": " + day.reading })
            }
        }
        Text("Dim columns: no readings · baseline: recorded zero", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
        TextButton(onClick = { expanded = !expanded }) { Text("Daily readings") }
        if (expanded) days.forEach { Text(it.date + " · " + it.reading, style = MaterialTheme.typography.bodySmall) }
    }
}

@OptIn(ExperimentalLayoutApi::class)
fun LazyListScope.hubSettingsRows(m: AppModel, rows: List<SetRow>, notify: Boolean = false) {
    items(rows.filter { if (notify) it.notify else !(it.local || it.notify) }) { r ->
        Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(r.label, style = MaterialTheme.typography.bodyLarge)
            if (r.note.isNotEmpty()) Text(r.note, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
            if (r.chart.isNotEmpty()) UsageDayChart(r.chart)
            for (f in r.fields) {
                var t by remember(r.label, f.name) { mutableStateOf(f.value) }
                // the secret box's value stays in the app until Save (Core.vaultText)
                OutlinedTextField(t, { t = it; if (f.vault) m.vault(it) else m.field(f.name, it) }, Modifier.fillMaxWidth(), singleLine = true, label = { Text(f.hint) },
                    visualTransformation = if (f.secret) PasswordVisualTransformation() else VisualTransformation.None,
                    keyboardOptions = KeyboardOptions(keyboardType = if (f.secret) KeyboardType.Password else KeyboardType.Ascii, autoCorrectEnabled = false))
            }
            if (r.buttons.isNotEmpty()) FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                for (b in r.buttons) {
                    if (b.on) Button(onClick = { m.act(b.action, b.value) }, shape = corner) { Text(b.label) }
                    else OutlinedButton(onClick = { m.act(b.action, b.value) }, shape = corner) { Text(b.label) }
                }
            }
            if (r.qr.isNotEmpty()) Column(Modifier.fillMaxWidth().clickable { m.act("flag", "pairqr") },
                horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                QrCode(r.qr, Modifier.fillMaxWidth())
                if (r.qrnote.isNotEmpty()) Text(r.qrnote, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
            }
        }
        HorizontalDivider()
    }
}

// the thread menu's icon for an action
// a tool's icon (core/icons.bend's name) as a Material icon
fun toolIcon(name: String) = when (name) {
    "pin" -> Icons.Filled.PushPin
    "circle-check" -> Icons.Filled.CheckCircleOutline
    "clock" -> Icons.Filled.Snooze
    "archive" -> Icons.Filled.Archive
    "trash" -> Icons.Filled.Delete
    "git-fork" -> Icons.AutoMirrored.Filled.CallSplit
    "undo" -> Icons.AutoMirrored.Filled.Undo
    "terminal" -> Icons.Filled.Terminal
    "panel-right" -> Icons.Filled.VerticalSplit
    "diff" -> Icons.Filled.Difference
    "search" -> Icons.Filled.FindInPage
    "stop" -> Icons.Filled.Stop
    "x" -> Icons.Filled.Close
    "box" -> Icons.Filled.ShoppingCart
    else -> null
}
