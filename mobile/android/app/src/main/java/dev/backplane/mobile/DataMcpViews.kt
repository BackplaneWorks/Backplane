package dev.backplane.mobile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.outlined.Cable
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject

private fun org.json.JSONArray?.items(): List<JSONObject> = McpPage.objs(this)

/** A small chip: lit (the accent), the page's main one (outlined in the accent), or plain. */
@Composable
internal fun McpChip(label: String, on: Boolean, modifier: Modifier = Modifier, main: Boolean = false, danger: Boolean = false, tall: Boolean = false, press: () -> Unit) {
    val bg = when { on && danger -> dcs.error; on -> dcs.primary; else -> dcs.surfaceContainerHigh }
    val fg = when { on && danger -> dcs.onError; on -> dcs.onPrimary; main -> dcs.primary; danger -> dcs.error; else -> dcs.onSurface }
    val line = when { on && danger -> dcs.error; on || main -> dcs.primary; else -> dcs.outlineVariant }
    Box(modifier.heightIn(min = if (tall) 40.dp else 32.dp).background(bg).border(BorderStroke(1.dp, line)).clickable(onClick = press).padding(horizontal = if (tall) 14.dp else 10.dp), contentAlignment = Alignment.Center) {
        Text(label, color = fg, fontFamily = BpMono, fontSize = 12.sp, fontWeight = if (on || main) FontWeight.Bold else FontWeight.Normal, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable private fun toneColor(t: String): Color = when (t) { "ok" -> dok; "warn" -> dwarn; "bad" -> dcs.error; "busy" -> dcs.primary; else -> dcs.onSurfaceVariant }
private val Light @Composable get() = dcs.background.red > .5f

/** Settings > MCP servers: the hub's page, read when it opens, after each press, every few seconds and as a paste is typed. */
@Composable
internal fun McpScreen(back: () -> Unit) {
    LaunchedEffect(Unit) { DataMcp.reset() }
    LaunchedEffect(DataClient.conn) {
        if (DataClient.conn != DataClient.Conn.ONLINE) return@LaunchedEffect
        DataMcp.load()
        while (true) { delay(McpPage.REFRESH_MS); DataMcp.tick() }
    }
    Column(Modifier.fillMaxSize().testTag("settings.mcp")) {
        DBar(Msg.android_mcp, back)
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(top = 14.dp, bottom = 32.dp)) {
            McpRows(DataMcp.rows, DataMcp.error)
        }
    }
}

/** The rows by kind; a row of no kind this app knows is a plain row (PlainRow, PageRows' look). */
@Composable
internal fun McpRows(rows: List<JSONObject>, error: String) {
    val scope = rememberCoroutineScope()
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val press: (JSONObject, JSONObject) -> Unit = { b, row ->
        val action = b.optString("action"); val value = b.optString("value")
        when {
            pageLocal(ctx, action, value) -> {}
            action == "secret-save" -> { val f = row.optJSONArray("fields").items().firstOrNull(McpPage::isVault); if (f != null) scope.launch { DataMcp.saveSecret(value, McpPage.vaultKey(row, f.optString("name"))) } }
            else -> scope.launch { DataMcp.act(action, value) }
        }
    }
    val type: (String, String) -> Unit = { name, v -> DataMcp.typed(scope, name, v) }
    var first = true
    if (error.isNotEmpty()) { Notice(JSONObject().put("label", error).put("tone", "bad"), Modifier.testTag("mcp.error")) { _ -> }; Spacer(Modifier.height(10.dp)) }
    McpPage.blocks(rows).forEach { blk ->
        val r = blk.head; val kind = McpPage.kind(r)
        Spacer(Modifier.height(if (first) 0.dp else gapBefore(kind, r)))
        first = false
        if (kind == "card" && blk.inner.isNotEmpty()) OpenCard(r, blk.inner, press, type)
        else McpRow(r, press, type, inCard = false)
    }
}

private fun gapBefore(kind: String, r: JSONObject): Dp = when (kind) {
    "group" -> 22.dp; "card" -> if (r.optBoolean("open")) 12.dp else 8.dp; "panel" -> 18.dp; "head" -> 0.dp; "actions" -> 14.dp; "empty" -> 12.dp; else -> 10.dp
}

@Composable
private fun McpRow(r: JSONObject, press: (JSONObject, JSONObject) -> Unit, type: (String, String) -> Unit, inCard: Boolean) {
    val p: (JSONObject) -> Unit = { press(it, r) }
    when (McpPage.kind(r)) {
        "head" -> Head(r, p)
        "group" -> Group(r, inCard)
        "card" -> Card(r, p, type, framed = true)
        "tool" -> Tool(r, p)
        "notice" -> Notice(r, press = p)
        "code" -> Code(r)
        "empty" -> Empty(r, p)
        "panel" -> Panel(r, p)
        "paste" -> Paste(r, type)
        "actions" -> Actions(r, p)
        else -> PlainRow(r, "mcp", { name, hub -> shownOf(r, name, hub) }, { name, v -> typeOf(r, name, v, type) }, compact = true, press = p)
    }
}

/** A field's text: a vault field's from the sealed side (by row), any other the typed one, else the hub's. */
private fun shownOf(r: JSONObject, name: String, hub: String): String {
    val f = r.optJSONArray("fields").items().firstOrNull { it.optString("name") == name }
    return if (f != null && McpPage.isVault(f)) DataMcp.form.vault(McpPage.vaultKey(r, name)) else DataMcp.form.shown(name, hub)
}
private fun typeOf(r: JSONObject, name: String, v: String, type: (String, String) -> Unit) {
    val f = r.optJSONArray("fields").items().firstOrNull { it.optString("name") == name }
    if (f != null && McpPage.isVault(f)) DataMcp.form.typeVault(McpPage.vaultKey(r, name), v) else type(name, v)
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Head(r: JSONObject, press: (JSONObject) -> Unit) {
    val bs = r.optJSONArray("buttons").items()
    // the main button stands out only while no view is lit
    val main = if (bs.any { it.optBoolean("on") }) -1 else McpPage.primary(r)
    Column(Modifier.fillMaxWidth().testTag("mcp.head")) {
        if (r.optString("note").isNotEmpty()) Text(Tr.t(r.optString("note")), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, lineHeight = 17.sp)
        if (bs.isNotEmpty()) Row(Modifier.fillMaxWidth().padding(top = 14.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            bs.forEachIndexed { i, b -> McpChip(Tr.t(b.optString("label")), b.optBoolean("on"), Modifier.weight(1f).testTag("mcp.head.$i"), main = i == main, tall = true) { press(b) } }
        }
        Spacer(Modifier.height(14.dp)); DLine()
    }
}

@Composable
private fun Group(r: JSONObject, inCard: Boolean) {
    Row(Modifier.fillMaxWidth().padding(bottom = if (inCard) 0.dp else 2.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(Tr.t(r.optString("label")).uppercase(), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.sp, letterSpacing = .6.sp, maxLines = 1)
        if (r.optString("note").isNotEmpty()) Text(r.optString("note"), Modifier.weight(1f), color = dfaint, fontFamily = BpMono, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** The square tile: the vendor's colour, white bold letters. */
@Composable
private fun Tile(icon: JSONObject?, size: Dp) {
    val bg = McpPage.hex(icon?.optString("bg").orEmpty())?.let { Color(it) } ?: dcs.surfaceContainerHigh
    val text = icon?.optString("text").orEmpty().ifEmpty { "·" }
    val fs = with(LocalDensity.current) { (size * McpPage.tileScale(text)).toSp() }
    Box(Modifier.size(size).background(bg).then(if (Light) Modifier else Modifier.border(BorderStroke(1.dp, Color.White.copy(alpha = .08f)))), contentAlignment = Alignment.Center) {
        Text(text, color = Color.White, fontSize = fs, fontWeight = FontWeight.Bold, maxLines = 1, letterSpacing = 0.sp)
    }
}

@Composable
private fun Pill(p: JSONObject?) {
    val text = p?.optString("text").orEmpty(); if (text.isEmpty()) return
    val c = toneColor(p?.optString("tone").orEmpty())
    Box(Modifier.background(c.copy(alpha = if (Light) .12f else .16f)).padding(horizontal = 6.dp, vertical = 1.dp)) {
        Text(Tr.t(text), color = c, fontFamily = BpMono, fontSize = 10.5.sp, maxLines = 1)
    }
}

/** A switch (a square track, the knob at the end it is set to), or with style "check" a box with a tick. */
@Composable
private fun Switch(t: JSONObject?, tag: String, press: (JSONObject) -> Unit) {
    if (t == null || t.optString("action").isEmpty()) return
    val on = t.optBoolean("on")
    val b = JSONObject().put("action", t.optString("action")).put("value", t.optString("value"))
    if (t.optString("style") == "check") {
        Box(Modifier.size(22.dp).background(if (on) dcs.primary else dcs.background).border(BorderStroke(1.5.dp, if (on) dcs.primary else dcs.onSurfaceVariant)).clickable { press(b) }.testTag(tag), contentAlignment = Alignment.Center) {
            if (on) Icon(Icons.Default.Check, null, tint = dcs.onPrimary, modifier = Modifier.size(16.dp))
        }
    } else {
        Box(Modifier.size(38.dp, 22.dp).background(if (on) dcs.primary else dcs.surfaceContainerHigh).border(BorderStroke(1.dp, if (on) dcs.primary else dcs.outlineVariant)).clickable { press(b) }.padding(4.dp).testTag(tag),
            contentAlignment = if (on) Alignment.CenterEnd else Alignment.CenterStart) {
            Box(Modifier.size(12.dp).background(if (on) Color.White else dcs.onSurfaceVariant))
        }
    }
}

/** A server: tile, title with its badge and status, its command or URL, a meta line, its switch; tapping it is its `press`. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Card(r: JSONObject, press: (JSONObject) -> Unit, type: (String, String) -> Unit, framed: Boolean) {
    val open = r.optBoolean("open"); val compact = r.optBoolean("compact")
    val tile = if (compact) 30.dp else 38.dp
    val tap = r.optJSONObject("press")?.takeIf { it.optString("action").isNotEmpty() }
    val extra = McpPage.cardButtons(r)
    val title = r.optString("title").ifEmpty { r.optString("label") }
    val sub = r.optString("sub"); val meta = r.optString("meta"); val badge = r.optString("badge")
    var m = Modifier.fillMaxWidth()
    if (framed) m = m.background(dcs.surfaceVariant).border(BorderStroke(1.dp, if (open) dcs.primary else dcs.outlineVariant))
    if (tap != null) m = m.clickable { press(tap) }
    Column(m.padding(horizontal = 12.dp, vertical = if (compact) 9.dp else 12.dp).testTag("mcp.card.${r.optString("id").ifEmpty { title }}")) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Tile(r.optJSONObject("icon"), tile)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(Tr.t(title), Modifier.weight(1f, fill = false), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.5.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (badge.isNotEmpty()) Box(Modifier.border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 4.dp)) { Text(Tr.t(badge), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 9.5.sp, maxLines = 1) }
                    Pill(r.optJSONObject("pill"))
                }
                if (sub.isNotEmpty()) Text(sub, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (meta.isNotEmpty()) Text(Tr.t(meta), color = dfaint, fontFamily = BpMono, fontSize = 10.5.sp, lineHeight = 14.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            r.optJSONObject("toggle")?.let { Spacer(Modifier.width(10.dp)); Switch(it, "mcp.toggle.${it.optString("value")}", press) }
        }
        r.optJSONArray("fields").items().forEach { f ->
            val name = f.optString("name")
            Box(Modifier.padding(start = tile + 12.dp, top = 10.dp)) { McpField(DataMcp.form.shown(name, f.optString("value")), { type(name, it) }, Tr.t(f.optString("hint")), 1, "mcp.field.$name") }
        }
        if (extra.isNotEmpty()) FlowRow(Modifier.padding(start = tile + 12.dp, top = 10.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            extra.forEachIndexed { i, b -> McpChip(Tr.t(b.optString("label")), b.optBoolean("on"), Modifier.testTag("mcp.cardbtn.${r.optString("title")}.$i")) { press(b) } }
        }
    }
}

/** An opened card: the card and the rows inside it on one surface, framed in the accent. */
@Composable
private fun OpenCard(r: JSONObject, inner: List<JSONObject>, press: (JSONObject, JSONObject) -> Unit, type: (String, String) -> Unit) {
    Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.primary)).testTag("mcp.open")) {
        Card(r, { press(it, r) }, type, framed = false)
        Box(Modifier.padding(horizontal = 12.dp).fillMaxWidth().height(1.dp).background(dcs.outlineVariant))
        Column(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = 12.dp, bottom = 4.dp)) {
            var prev: String? = null
            inner.forEach { x ->
                val k = McpPage.kind(x)
                // tools run together under their label; everything else breathes
                val gap = when { prev == null -> 0.dp; k == "tool" && (prev == "tool" || prev == "group") -> 2.dp; k == "group" -> 20.dp; k == "actions" -> 8.dp; else -> 16.dp }
                Spacer(Modifier.height(gap)); prev = k
                McpRow(x, press, type, inCard = true)
            }
        }
    }
}

@Composable
private fun Tool(r: JSONObject, press: (JSONObject) -> Unit) {
    val on = r.optJSONObject("toggle")?.optBoolean("on") != false
    Row(Modifier.fillMaxWidth().padding(vertical = 9.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(r.optString("label"), color = if (on) dcs.onSurface else dfaint, fontFamily = BpMono, fontSize = 12.5.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (r.optString("note").isNotEmpty()) Text(r.optString("note"), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.sp, lineHeight = 15.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        Spacer(Modifier.width(12.dp))
        Switch(r.optJSONObject("toggle"), "mcp.tool.${r.optString("label")}", press)
    }
    Box(Modifier.fillMaxWidth().height(1.dp).background(dcs.outlineVariant.copy(alpha = .6f)))
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Notice(r: JSONObject, modifier: Modifier = Modifier, press: (JSONObject) -> Unit) {
    val tone = r.optString("tone"); val c = toneColor(tone)
    val bs = r.optJSONArray("buttons").items()
    Row(modifier.fillMaxWidth().height(IntrinsicSize.Min).background(c.copy(alpha = if (Light) .07f else .09f)).testTag("mcp.notice"), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.width(3.dp).fillMaxHeight().background(c))
        Column(Modifier.weight(1f).padding(horizontal = 10.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(Tr.t(r.optString("label")), color = if (tone == "ok" || tone == "busy" || tone.isEmpty()) dcs.onSurface else c, fontFamily = BpMono, fontSize = 12.sp, lineHeight = 16.sp)
            if (r.optString("note").isNotEmpty()) Text(Tr.t(r.optString("note")), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.sp, lineHeight = 15.sp)
        }
        if (bs.isNotEmpty()) Row(Modifier.padding(end = 8.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) { bs.forEach { b -> McpChip(Tr.t(b.optString("label")), b.optBoolean("on")) { press(b) } } }
    }
}

@Composable
private fun Code(r: JSONObject) {
    Column(Modifier.fillMaxWidth().background(dview).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 10.dp, vertical = 8.dp).testTag("mcp.code"), verticalArrangement = Arrangement.spacedBy(1.dp)) {
        r.optString("text").lines().take(14).forEach { l -> Text(l, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.sp, lineHeight = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis) }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Empty(r: JSONObject, press: (JSONObject) -> Unit) {
    val line = dcs.outlineVariant
    Column(Modifier.fillMaxWidth().drawBehind {
        val w = 1.dp.toPx(); drawRect(line, topLeft = Offset(w / 2, w / 2), size = size.copy(size.width - w, size.height - w), style = Stroke(w, pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx()))))
    }.padding(horizontal = 20.dp, vertical = 24.dp).testTag("mcp.empty"), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Icon(Icons.Outlined.Cable, null, tint = dfaint, modifier = Modifier.size(26.dp))
        Text(Tr.t(r.optString("label")), color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center)
        if (r.optString("note").isNotEmpty()) Text(Tr.t(r.optString("note")), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, lineHeight = 17.sp, textAlign = TextAlign.Center)
        val bs = r.optJSONArray("buttons").items()
        if (bs.isNotEmpty()) FlowRow(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            bs.forEachIndexed { i, b -> McpChip(Tr.t(b.optString("label")), b.optBoolean("on"), main = i == 0) { press(b) } }
        }
    }
}

/** A form's title and, on the right, its modes (one action: a segmented control) or its buttons. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Panel(r: JSONObject, press: (JSONObject) -> Unit) {
    val bs = r.optJSONArray("buttons").items()
    val segmented = bs.size > 1 && bs.all { it.optString("action") == bs[0].optString("action") }
    Column(Modifier.fillMaxWidth().testTag("mcp.panel")) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(dcs.primary))
        Row(Modifier.fillMaxWidth().padding(top = 14.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(Tr.t(r.optString("label")), Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.5.sp, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis)
            if (segmented) Row(Modifier.border(BorderStroke(1.dp, dcs.outlineVariant))) {
                bs.forEachIndexed { i, b ->
                    if (i > 0) Box(Modifier.width(1.dp).height(30.dp).background(dcs.outlineVariant))
                    val on = b.optBoolean("on")
                    Box(Modifier.height(30.dp).background(if (on) dcs.primary else dcs.surfaceContainerHigh).clickable { press(b) }.padding(horizontal = 10.dp).testTag("mcp.mode.${b.optString("value")}"), contentAlignment = Alignment.Center) {
                        Text(Tr.t(b.optString("label")), color = if (on) dcs.onPrimary else dcs.onSurface, fontFamily = BpMono, fontSize = 11.5.sp, fontWeight = if (on) FontWeight.Bold else FontWeight.Normal, maxLines = 1)
                    }
                }
            } else bs.forEach { b -> McpChip(Tr.t(b.optString("label")), b.optBoolean("on")) { press(b) } }
        }
        if (r.optString("note").isNotEmpty()) Text(Tr.t(r.optString("note")), Modifier.padding(top = 6.dp), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.5.sp, lineHeight = 16.sp)
    }
}

@Composable
private fun McpField(v: String, on: (String) -> Unit, hint: String, lines: Int, tag: String) {
    val big = lines > 1
    val focus = remember { FocusRequester() }
    Box(Modifier.fillMaxWidth().background(dview).border(BorderStroke(1.dp, dcs.outlineVariant)).clickable(indication = null, interactionSource = remember { MutableInteractionSource() }) { runCatching { focus.requestFocus() } }.padding(horizontal = 10.dp, vertical = if (big) 10.dp else 9.dp).testTag(tag)) {
        val style = TextStyle(color = dcs.onSurface, fontFamily = BpMono, fontSize = if (big) 11.5.sp else 13.sp, lineHeight = if (big) 16.sp else 18.sp)
        if (v.isEmpty()) Text(hint, color = dfaint, fontFamily = BpMono, fontSize = style.fontSize, lineHeight = style.lineHeight, maxLines = 1, overflow = TextOverflow.Ellipsis)
        // a pasted config wraps (a sideways scroll lost typed characters to the cursor following them on the emulator)
        if (big) BasicTextField(v, on, textStyle = style, cursorBrush = SolidColor(dcs.primary), minLines = lines, maxLines = 60, modifier = Modifier.fillMaxWidth().focusRequester(focus))
        else BasicTextField(v, on, textStyle = style, cursorBrush = SolidColor(dcs.primary), singleLine = true, modifier = Modifier.fillMaxWidth().focusRequester(focus))
    }
}

@Composable
private fun Paste(r: JSONObject, type: (String, String) -> Unit) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (r.optString("note").isNotEmpty()) Text(Tr.t(r.optString("note")), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.5.sp, lineHeight = 16.sp)
        r.optJSONArray("fields").items().forEach { f ->
            val name = f.optString("name")
            McpField(DataMcp.form.shown(name, f.optString("value")), { type(name, it) }, Tr.t(f.optString("hint")), f.optInt("lines", 6).coerceIn(2, 14), "mcp.field.$name")
        }
    }
}

/** A line of buttons on the right, a small note on the left (above them when there are many). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Actions(r: JSONObject, press: (JSONObject) -> Unit) {
    val bs = r.optJSONArray("buttons").items(); val note = r.optString("note")
    // a second press removes: its note says so in the error colour, and its button is lit red
    val warn = bs.any { it.optString("action") == "mcp-remove" && it.optBoolean("on") }
    val chips: @Composable () -> Unit = {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            // the first button lit is the press the form is for (filled); a later one lit is a setting that is on (outlined in the accent)
            bs.forEachIndexed { i, b -> val a = b.optString("action"); val on = b.optBoolean("on"); val filled = on && (i == 0 || a == "mcp-remove")
                McpChip(Tr.t(b.optString("label")), filled, Modifier.testTag("mcp.act.$a"), main = on && !filled, danger = a == "mcp-remove") { press(b) } }
        }
    }
    val noteText: @Composable (Modifier) -> Unit = { m -> Text(Tr.t(note), m, color = if (warn) dcs.error else dfaint, fontFamily = BpMono, fontSize = 11.sp, lineHeight = 15.sp) }
    Column(Modifier.fillMaxWidth().testTag("mcp.actions"), horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (note.isNotEmpty() && bs.size > 2) { noteText(Modifier.fillMaxWidth()); chips() }
        else if (note.isNotEmpty()) Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) { noteText(Modifier.weight(1f)); chips() }
        else chips()
    }
}

/** Settings' entry. */
@Composable
internal fun McpEntry(open: () -> Unit) { DBtn(Msg.android_mcp, false, modifier = Modifier.testTag("settings.mcp.open"), onClick = open) }
