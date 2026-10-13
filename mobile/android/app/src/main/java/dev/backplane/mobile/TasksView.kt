package dev.backplane.mobile

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL

// Browser tasks (docs/teach.md): the Browser tab (the strip, the frame that
// takes the person's input while they drive, the save sheet), the Tasks tab,
// and the picture on a paused run's card. All of it is the screen's
// (src/mobile/bots.bend); these views only draw it and send actions back.

private val square = RoundedCornerShape(4.dp)
private val amber = Color(0xFFF57C00)

// a picture the hub serves (a path with its query), fetched with the
// pairing token; null until it arrives
@Composable
fun rememberHubImage(m: AppModel, path: String): ImageBitmap? {
    val url = if (path.isEmpty()) null else m.web(path)
    var image by remember(path) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(url) {
        if (url == null) return@LaunchedEffect
        val got = withContext(Dispatchers.IO) {
            runCatching {
                val c = URL(url).openConnection() as HttpURLConnection
                c.connectTimeout = 5000
                c.readTimeout = 15000
                try {
                    if (c.responseCode == 200) c.inputStream.use { BitmapFactory.decodeStream(it) } else null
                } finally {
                    c.disconnect()
                }
            }.getOrNull()
        }
        if (got != null) image = got.asImageBitmap()
    }
    return image
}

// where a paused run stopped
@Composable
fun HubPicture(m: AppModel, path: String, modifier: Modifier = Modifier, scale: ContentScale = ContentScale.FillWidth) {
    val img = rememberHubImage(m, path)
    if (img != null) Image(img, "Screenshot", modifier.border(1.dp, MaterialTheme.colorScheme.outlineVariant, square), contentScale = scale)
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Btns(m: AppModel, bs: List<TBtn>) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        for (b in bs) {
            if (b.primary) Button(onClick = { m.act(b.action, b.value) }, shape = square) { Text(b.label) }
            else OutlinedButton(onClick = { m.act(b.action, b.value) }, shape = square) { Text(b.label) }
        }
    }
}

// a field that keeps what is typed in the client's scratch ("bfield")
@Composable
private fun Field(m: AppModel, name: String, value: String, label: String, send: String = "", onSend: () -> Unit = {}, modifier: Modifier = Modifier.fillMaxWidth(), reset: Int = 0) {
    var text by remember(name, value, reset) { mutableStateOf(value) }
    OutlinedTextField(text, { text = it; m.field(name, it) }, modifier, singleLine = true, label = { Text(label) },
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false,
            imeAction = if (send.isEmpty()) ImeAction.Default else ImeAction.Go),
        keyboardActions = androidx.compose.foundation.text.KeyboardActions(onGo = { onSend() }))
}

// the card of a paused run: its question, the screenshot, the choices
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BrowserAsk(m: AppModel, a: Ask) {
    Surface(color = amber.copy(alpha = 0.14f), shape = square, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(a.head, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
            if (a.detail.isNotEmpty()) Text(a.detail, style = MaterialTheme.typography.bodyMedium)
            if (a.image.isNotEmpty()) HubPicture(m, a.image, Modifier.fillMaxWidth())
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                for (b in a.buttons) {
                    val act = b.action.ifEmpty { "answer" }
                    if (b.primary) Button(onClick = { m.act(act, b.value) }, shape = square) { Text(b.label) }
                    else OutlinedButton(onClick = { m.act(act, b.value) }, shape = square) { Text(b.label) }
                }
            }
        }
    }
}

// the frame, which takes the person's taps and vertical drags while they
// drive: a tap is "x y" in thousandths of the image as drawn, a drag is
// the page's pixels (about twice the finger's distance)
@Composable
private fun Frame(m: AppModel, b: BotBrowser, bot: BotView) {
    val img = rememberHubImage(m, b.url + "?n=" + b.n)
    if (img == null) {
        Box(Modifier.fillMaxWidth().height(160.dp), contentAlignment = Alignment.Center) {
            Text("No page yet", color = MaterialTheme.colorScheme.outline)
        }
        return
    }
    val density = LocalDensity.current.density
    var pull by remember { mutableStateOf(0f) }
    val base = Modifier.fillMaxWidth()
    val live = if (!b.input) base else base
        .pointerInput(b.input) {
            detectTapGestures { p ->
                val x = (p.x / size.width * 1000).toInt().coerceIn(0, 1000)
                val y = (p.y / size.height * 1000).toInt().coerceIn(0, 1000)
                m.act("bb-click", "$x $y")
            }
        }
        .pointerInput(b.input) {
            detectVerticalDragGestures(onDragEnd = { pull = 0f }, onDragCancel = { pull = 0f }) { _, d ->
                pull += d
                if (kotlin.math.abs(pull) >= 24f * density) {
                    // a finger moving up scrolls the page down
                    m.act("bb-scroll", (-pull / density * 2).toInt().toString())
                    pull = 0f
                }
            }
        }
    Image(img, bot.name + "'s browser", live.then(if (b.input) Modifier.border(2.dp, amber, square) else Modifier), contentScale = ContentScale.FillWidth)
}

// while the person drives: the address, and the keys the frame cannot send
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Driving(m: AppModel, b: BotBrowser) {
    var text by remember { mutableStateOf("") }
    Column(Modifier.padding(horizontal = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedButton(onClick = { m.act("bb-back") }, shape = square) { Text("Back") }
            OutlinedButton(onClick = { m.act("bb-reload") }, shape = square) { Text("Reload") }
            Field(m, "bburl", b.addr, "Address", "go", { m.act("bb-nav") }, Modifier.weight(1f))
            Button(onClick = { m.act("bb-nav") }, shape = square) { Text("Go") }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(text, { text = it }, Modifier.weight(1f), singleLine = true, label = { Text("Type into the page") },
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false))
            Button(onClick = { if (text.isNotEmpty()) { m.act("bb-type", text); text = "" } }, shape = square) { Text("Send") }
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            for ((label, key) in listOf("Enter" to "Enter", "Tab" to "Tab", "⌫" to "Backspace", "Esc" to "Escape"))
                OutlinedButton(onClick = { m.act("bb-key", key) }, shape = square) { Text(label) }
        }
    }
}

// a recording stopped: name it, mark typed values as parameters
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SaveSheet(m: AppModel, b: BotBrowser) {
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text(b.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
        Text("Name it. Mark a value you typed to ask for it each time the task runs.", style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.outline)
        Field(m, "tkname", b.name, "Name")
        Field(m, "tkdesc", b.desc, "What it does (optional)")
        for (r in b.rows) {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("${r.n}.", color = MaterialTheme.colorScheme.outline, fontFamily = FontFamily.Monospace)
                    Text(r.text, Modifier.weight(1f))
                    if (r.secret) Text("private", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                }
                if (r.chip.isNotEmpty()) FilterChip(r.marked, { m.act("tk-mark", r.n.toString()) }, label = { Text(r.chip, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    shape = square)
                if (r.marked) Field(m, r.field, "", r.pname.ifEmpty { "Name" })
            }
        }
        Btns(m, b.btns)
    }
}

// Browser: the strip, then the page; a paused run's card leads
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BrowserTab(m: AppModel, bot: BotView) {
    val b = bot.browser
    if (b == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text("No page yet", color = MaterialTheme.colorScheme.outline) }
        return
    }
    if (b.mode == 2) { SaveSheet(m, b); return }
    var sent by remember { mutableStateOf(0) }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        for (a in b.asks) Box(Modifier.padding(horizontal = 12.dp, vertical = 4.dp)) { BrowserAsk(m, a) }
        Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (b.title.isNotEmpty()) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(b.title, fontWeight = FontWeight.SemiBold)
                if (b.mode == 1) Text(b.clock + " · " + b.steps + (if (b.steps == 1) " step" else " steps"),
                    color = MaterialTheme.colorScheme.outline, style = MaterialTheme.typography.labelMedium)
            }
            Btns(m, b.btns)
        }
        if (b.input) Driving(m, b)
        Frame(m, b, bot)
        if (b.mode == 1) Row(Modifier.padding(horizontal = 12.dp).padding(bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically) {
            Field(m, "tknote", b.note, "Tell it what matters", "go", { m.act("tk-note"); sent++ }, Modifier.weight(1f), sent)
            OutlinedButton(onClick = { m.act("tk-note"); sent++ }, shape = square) { Text("Add note") }
        }
    }
}

private fun statusColor(s: String): Color = when (s) {
    "ok" -> Color(0xFF30D158)
    "failed", "stopped" -> Color(0xFFFF453A)
    else -> Color(0xFF8E8E93)
}

// Tasks: what the bot has learned, each opened for its run form, then the
// recent runs with their screenshots
@Composable
fun TasksTab(m: AppModel, bot: BotView) {
    val t = bot.tasks ?: BotTasks(emptyList(), emptyList())
    LazyColumn(Modifier.fillMaxSize()) {
        item(key = "note") {
            Text(if (t.items.isEmpty()) "None yet. Teach one in the Browser tab, or tell it in chat." else "Open one to run it. A routine can run it too.",
                Modifier.padding(12.dp), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
        }
        items(t.items, key = { "t:" + it.slug }) { x ->
            Column(Modifier.fillMaxWidth()) {
                Row(Modifier.fillMaxWidth().clickable { m.act("tk-open", x.slug) }.padding(horizontal = 12.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Column(Modifier.weight(1f)) {
                        Text(x.name, fontWeight = FontWeight.SemiBold)
                        if (x.desc.isNotEmpty()) Text(x.desc, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
                        if (x.params.isNotEmpty()) Text(x.params.joinToString(", ") { it.label },
                            style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                    }
                    if (x.last.isNotEmpty()) Text(x.last, style = MaterialTheme.typography.labelMedium, color = statusColor(x.last))
                }
                if (x.open) Column(Modifier.padding(horizontal = 12.dp).padding(bottom = 10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (p in x.params) Field(m, p.field, p.value, p.label + (if (p.example.isEmpty()) "" else " (${p.example})"), reset = t.runs.size)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = { m.act("tk-run", x.slug) }, shape = square) { Text("Run") }
                        OutlinedButton(onClick = { m.act("tk-del", x.slug) }, shape = square) {
                            Text(if (x.asking) "Delete this task?" else "Delete", color = MaterialTheme.colorScheme.error)
                        }
                    }
                }
                HorizontalDivider()
            }
        }
        if (t.runs.isNotEmpty()) item(key = "runs") {
            Text("Recent runs", Modifier.padding(12.dp), style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
        }
        items(t.runs, key = { "r:" + it.run }) { r ->
            Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(r.slug, fontWeight = FontWeight.SemiBold)
                        if (r.summary.isNotEmpty()) Text(r.summary, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
                    }
                    Text(r.status, style = MaterialTheme.typography.labelMedium, color = statusColor(r.status))
                }
                if (r.shots.isNotEmpty()) LazyRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    items(r.shots) { s -> HubPicture(m, s, Modifier.width(120.dp), ContentScale.FillWidth) }
                }
                HorizontalDivider()
            }
        }
    }
}
