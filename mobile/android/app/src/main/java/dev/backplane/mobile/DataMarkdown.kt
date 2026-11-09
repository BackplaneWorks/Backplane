package dev.backplane.mobile

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext

// A message's Markdown for the data/1 app, as the desktop draws it (native/gpui/src/screens/md.rs): headings, bold, italic, strike, inline code,
// fenced code with Copy, bullet, numbered and to-do lists, quotes, tables that scroll sideways, links (backplane://thread/<id> opens the thread), images, rules.
// A reply that is still arriving is parsed again as it grows: whatever is not closed yet (a fence, a **, a table's separator) shows as it stands.
// Parsed blocks and styled text are cached by their source text, so a message that did not change costs nothing to draw again.
sealed interface MdBlock {
    data class P(val text: String) : MdBlock
    data class H(val level: Int, val text: String) : MdBlock
    /** task: -1 not a to-do, 0 open, 1 done */
    data class Li(val depth: Int, val marker: String, val task: Int, val text: String) : MdBlock
    data class Code(val lang: String, val text: String) : MdBlock
    data class Quote(val blocks: List<MdBlock>) : MdBlock
    /** aligns: 0 left, 1 centre, 2 right */
    data class Table(val head: List<String>, val aligns: List<Int>, val rows: List<List<String>>) : MdBlock
    data class Img(val alt: String, val url: String) : MdBlock
    object Rule : MdBlock
}

private val reHead = Regex("^(#{1,6})\\s+(.*?)(\\s+#+)?$")
private val reRule = Regex("^([-*_])(\\s*\\1){2,}$")
private val reItem = Regex("^(\\s*)([-*+]|\\d{1,9}[.)])\\s+(.*)$")
private val reTask = Regex("^\\[([ xX])]\\s+(.*)$")
private val reSep = Regex("^\\|?\\s*:?-+:?\\s*(\\|\\s*:?-+:?\\s*)*\\|?$")
private val reImg = Regex("^!\\[([^\\]]*)]\\(\\s*<?([^)\\s>]+)>?(\\s+\"[^\"]*\")?\\s*\\)$")

private fun isFence(t: String) = t.startsWith("```") || t.startsWith("~~~")
private fun indentOf(l: String): Int { var n = 0; for (c in l) { if (c == ' ') n++ else if (c == '\t') n += 4 else break }; return n }
private fun startsBlock(l: String, next: String?): Boolean {
    val t = l.trim()
    return t.isEmpty() || isFence(t) || reHead.matches(t) || reRule.matches(t) || reItem.matches(l) || t.startsWith(">") ||
        (t.contains('|') && next != null && reSep.matches(next.trim()) && next.contains('-'))
}

fun splitCells(row: String): List<String> {
    var t = row.trim(); if (t.startsWith("|")) t = t.substring(1); if (t.endsWith("|") && !t.endsWith("\\|")) t = t.dropLast(1)
    val out = ArrayList<String>(); val b = StringBuilder(); var i = 0; var tick = false
    while (i < t.length) {
        val c = t[i]
        if (c == '\\' && i + 1 < t.length && t[i + 1] == '|') { b.append('|'); i += 2; continue }
        if (c == '`') tick = !tick
        if (c == '|' && !tick) { out += b.toString().trim(); b.setLength(0) } else b.append(c)
        i++
    }
    out += b.toString().trim(); return out
}

private fun parse(lines: List<String>): List<MdBlock> {
    val out = ArrayList<MdBlock>(); var i = 0
    while (i < lines.size) {
        val l = lines[i]; val t = l.trim()
        if (t.isEmpty()) { i++; continue }
        if (isFence(t)) {
            val mark = t.take(3); val lang = t.drop(3).trim().takeWhile { !it.isWhitespace() }
            val ind = indentOf(l); val b = ArrayList<String>(); i++
            while (i < lines.size && !lines[i].trim().startsWith(mark)) { b += lines[i].let { s -> var k = 0; while (k < ind && k < s.length && s[k] == ' ') k++; s.substring(k) }; i++ }
            i++; out += MdBlock.Code(lang, b.joinToString("\n")); continue
        }
        val h = reHead.find(t)
        if (h != null) { out += MdBlock.H(h.groupValues[1].length, h.groupValues[2]); i++; continue }
        if (reRule.matches(t)) { out += MdBlock.Rule; i++; continue }
        if (t.startsWith(">")) {
            val q = ArrayList<String>()
            while (i < lines.size && lines[i].trim().startsWith(">")) { q += lines[i].trim().removePrefix(">").removePrefix(" "); i++ }
            out += MdBlock.Quote(parse(q)); continue
        }
        val m = reItem.find(l)
        if (m != null) {
            var body = m.groupValues[3]; var task = -1
            reTask.find(body)?.let { task = if (it.groupValues[1] == " ") 0 else 1; body = it.groupValues[2] }
            val mk = m.groupValues[2]; val depth = (indentOf(l) / 2).coerceAtMost(6); i++
            val sb = StringBuilder(body)
            while (i < lines.size && lines[i].isNotBlank() && indentOf(lines[i]) >= indentOf(l) + 2 && !reItem.matches(lines[i]) && !isFence(lines[i].trim())) { sb.append('\n').append(lines[i].trim()); i++ }
            out += MdBlock.Li(depth, if (mk[0].isDigit()) mk else "•", task, sb.toString()); continue
        }
        if (t.contains('|') && i + 1 < lines.size && reSep.matches(lines[i + 1].trim()) && lines[i + 1].contains('-')) {
            val head = splitCells(t)
            val aligns = splitCells(lines[i + 1]).map { c -> if (c.startsWith(":") && c.endsWith(":")) 1 else if (c.endsWith(":")) 2 else 0 }
            i += 2; val rows = ArrayList<List<String>>()
            while (i < lines.size && lines[i].isNotBlank() && lines[i].contains('|')) { rows += splitCells(lines[i]); i++ }
            out += MdBlock.Table(head, aligns, rows); continue
        }
        val im = reImg.find(t)
        if (im != null) { out += MdBlock.Img(im.groupValues[1], im.groupValues[2]); i++; continue }
        val para = ArrayList<String>(); para += l.trimEnd(); i++
        while (i < lines.size && !startsBlock(lines[i], lines.getOrNull(i + 1))) { para += lines[i].trimEnd(); i++ }
        out += MdBlock.P(para.joinToString("\n"))
    }
    return out
}

private val blockCache = LruCache<String, List<MdBlock>>(64)
fun mdBlocks(src: String): List<MdBlock> {
    blockCache.get(src)?.let { return it }
    return parse(src.replace("\r\n", "\n").split('\n')).also { blockCache.put(src, it) }
}

/** The link a tap follows (DataLinks.kt): web addresses, emails and phone numbers go to the system; a backplane:// link or a path is asked of the hub and opens its screen here, or says why not. */
object MdLinks {
    var ctx: android.content.Context? = null
    /** the thread whose messages are on show: a path in them is looked for in its project */
    var thread: String = ""
    fun open(url: String) {
        val c = ctx
        val scheme = url.substringBefore(':', "")
        when {
            url.startsWith(PATH_SCHEME) || scheme == "backplane" || scheme == BuildConfig.URL_SCHEME -> DataClient.openLink(url, thread)
            scheme == "http" || scheme == "https" || scheme == "mailto" -> c?.let { runCatching { it.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url)).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)) } }
            scheme == "tel" -> c?.let { runCatching { it.startActivity(android.content.Intent(android.content.Intent.ACTION_DIAL, android.net.Uri.parse(url)).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)) } }
        }
    }
}

class MdPalette(val text: Color, val code: Color, val codeBg: Color, val link: Color, val dark: Boolean)

private val inlineCache = LruCache<String, AnnotatedString>(512)
fun mdInline(s: String, p: MdPalette): AnnotatedString {
    val key = (if (p.dark) "d" else "l") + s
    inlineCache.get(key)?.let { return it }
    return buildAnnotatedString { inl(s, p, 0) }.also { inlineCache.put(key, it) }
}

private fun AnnotatedString.Builder.link(url: String, p: MdPalette, body: AnnotatedString.Builder.() -> Unit) {
    withLink(LinkAnnotation.Clickable(url, TextLinkStyles(SpanStyle(color = p.link, textDecoration = TextDecoration.Underline))) { MdLinks.open(url) }) { body() }
}

private fun AnnotatedString.Builder.inl(s: String, p: MdPalette, depth: Int) {
    var i = 0
    while (i < s.length) {
        val c = s[i]
        if (depth > 8) { append(s.substring(i)); return }
        // a backslash makes the next punctuation plain
        if (c == '\\' && i + 1 < s.length && s[i + 1] in "\\`*_{}[]()#+-.!|~<>") { append(s[i + 1]); i += 2; continue }
        if (c == '`') {
            var n = 0; while (i + n < s.length && s[i + n] == '`') n++
            val tick = "`".repeat(n); val e = s.indexOf(tick, i + n)
            if (e > 0) {
                val inner = s.substring(i + n, e).trim('`').let { if (it.startsWith(" ") && it.endsWith(" ") && it.isNotBlank()) it.trim() else it }
                val code: AnnotatedString.Builder.() -> Unit = { withStyle(SpanStyle(fontFamily = BpMono, color = p.code, background = p.codeBg, fontSize = 12.5.sp)) { append(inner) } }
                if (isPathText(inner)) link(pathLink(inner), p) { code() } else code()
                i = e + n; continue
            }
            append(tick); i += n; continue
        }
        if (c == '!' && s.startsWith("![", i) || c == '[') {
            val st = if (c == '!') i + 1 else i
            var d = 0; var j = st; var close = -1
            while (j < s.length) { if (s[j] == '\\') { j += 2; continue }; if (s[j] == '[') d++ else if (s[j] == ']') { d--; if (d == 0) { close = j; break } }; j++ }
            if (close > 0 && close + 1 < s.length && s[close + 1] == '(') {
                var pd = 0; var k = close + 1; var end = -1
                while (k < s.length) { if (s[k] == '(') pd++ else if (s[k] == ')') { pd--; if (pd == 0) { end = k; break } }; k++ }
                if (end > 0) {
                    val url = s.substring(close + 2, end).trim().removePrefix("<").substringBefore(' ').removeSuffix(">").let { if (isPathText(it)) pathLink(it) else it }
                    val label = s.substring(st + 1, close)
                    link(url, p) { if (c == '!') append(label.ifEmpty { url }) else inl(label, p, depth + 1) }
                    i = end + 1; continue
                }
            }
        }
        val wl = webLink(s, i)
        if (wl != null) { val url = wl.url; link(url, p) { append(url) }; i = wl.end; continue }
        if ((c == '*' || c == '_') && s.startsWith("$c$c", i) && i + 2 < s.length && !s[i + 2].isWhitespace() && (c == '*' || i == 0 || !s[i - 1].isLetterOrDigit())) {
            val tok = "$c$c"; var e = s.indexOf(tok, i + 2)
            while (e > 0 && s[e - 1].isWhitespace()) e = s.indexOf(tok, e + 2)
            while (e > 0 && e + 2 < s.length && s[e + 2] == c) e++
            if (e > i + 2) { withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { inl(s.substring(i + 2, e), p, depth + 1) }; i = e + 2; continue }
        }
        if (c == '~' && s.startsWith("~~", i) && i + 2 < s.length && !s[i + 2].isWhitespace()) {
            val e = s.indexOf("~~", i + 2)
            if (e > i + 2) { withStyle(SpanStyle(textDecoration = TextDecoration.LineThrough)) { inl(s.substring(i + 2, e), p, depth + 1) }; i = e + 2; continue }
        }
        if ((c == '*' || c == '_') && i + 1 < s.length && !s[i + 1].isWhitespace() && s[i + 1] != c && (c == '*' || i == 0 || !s[i - 1].isLetterOrDigit())) {
            var j = i + 1; var e = -1
            while (j < s.length) {
                if (s[j] == '\\') { j += 2; continue }
                if (s[j] == '`') { val q = s.indexOf('`', j + 1); if (q > 0) { j = q + 1; continue } }
                if (s[j] == c) { if (j + 1 < s.length && s[j + 1] == c) { j += 2; continue }; if (!s[j - 1].isWhitespace() && (c == '*' || j + 1 == s.length || !s[j + 1].isLetterOrDigit())) { e = j; break } }
                j++
            }
            if (e > i + 1) { withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { inl(s.substring(i + 1, e), p, depth + 1) }; i = e + 1; continue }
        }
        // emails, phone numbers and file paths in running text
        val al = autoLink(s, i)
        if (al != null) { val t = s.substring(i, al.end); link(al.url, p) { append(t) }; i = al.end; continue }
        append(c); i++
    }
}

private object MdPics {
    val cache = LruCache<String, ImageBitmap>(24)
    val http = okhttp3.OkHttpClient()
    suspend fun load(url: String, tid: String?): ImageBitmap? {
        cache.get(url)?.let { return it }
        val bytes = withContext(Dispatchers.IO) {
            if (url.startsWith("http://") || url.startsWith("https://")) runCatching { http.newCall(okhttp3.Request.Builder().url(url).build()).execute().use { r -> if (r.isSuccessful) r.body?.bytes() else null } }.getOrNull()
            else if (tid != null && url.startsWith("/")) DataClient.fileBytes(tid, url) else null
        } ?: return null
        val bmp = withContext(Dispatchers.Default) { BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap() } ?: return null
        cache.put(url, bmp); return bmp
    }
}

private fun heading(level: Int) = when (level) { 1 -> 18; 2 -> 16; 3 -> 14 else -> 13 }

/** A message's Markdown. `thread` lets a picture at a path on the hub's machine load. Everything in it can be selected. */
@Composable
fun MdText(src: String, modifier: Modifier = Modifier, thread: String? = null) {
    val dcs = MaterialTheme.colorScheme
    val ctx = LocalContext.current
    SideEffect { MdLinks.ctx = ctx.applicationContext; MdLinks.thread = thread ?: "" }
    val dark = dcs.background.red < .5f
    val pal = remember(dcs, dark) { MdPalette(dcs.onSurface, dcs.onSurface, dcs.surfaceContainerHigh, dcs.primary, dark) }
    val blocks = remember(src) { mdBlocks(src) }
    SelectionContainer { MdBlocks(blocks, pal, thread, modifier) }
}

@Composable
private fun MdBlocks(blocks: List<MdBlock>, pal: MdPalette, tid: String?, modifier: Modifier = Modifier) {
    val dcs = MaterialTheme.colorScheme
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        blocks.forEachIndexed { idx, b ->
            key(idx) {
                when (b) {
                    // prose in the system face (the phone look); code spans and blocks stay mono
                    is MdBlock.P -> Text(mdInline(b.text, pal), color = dcs.onSurface, fontSize = 15.sp, lineHeight = 21.sp, fontFamily = BpUi)
                    is MdBlock.H -> Text(mdInline(b.text, pal), Modifier.padding(top = 4.dp), color = dcs.onSurface, fontSize = (heading(b.level) + 2).sp, lineHeight = (heading(b.level) + 9).sp, fontWeight = FontWeight.Bold, fontFamily = BpUi)
                    is MdBlock.Li -> Row(Modifier.padding(start = (b.depth * 14).dp)) {
                        if (b.task >= 0) Box(Modifier.padding(top = 3.dp, end = 8.dp).size(13.dp).border(BorderStroke(1.dp, if (b.task == 1) dcs.primary else dcs.outline)).background(if (b.task == 1) dcs.primary else Color.Transparent), contentAlignment = Alignment.Center) {
                            if (b.task == 1) Text("✓", color = dcs.onPrimary, fontSize = 10.sp, lineHeight = 10.sp, fontWeight = FontWeight.Bold)
                        } else Text(b.marker, Modifier.widthIn(min = 18.dp).padding(end = 4.dp), color = dcs.onSurfaceVariant, fontSize = 15.sp, lineHeight = 21.sp, fontFamily = BpUi)
                        Text(mdInline(b.text, pal), color = if (b.task == 1) dcs.onSurfaceVariant else dcs.onSurface, fontSize = 15.sp, lineHeight = 21.sp, fontFamily = BpUi,
                            textDecoration = if (b.task == 1) TextDecoration.LineThrough else null)
                    }
                    is MdBlock.Code -> CodeBlock(b)
                    is MdBlock.Quote -> Row(Modifier.height(IntrinsicSize.Min)) {
                        Box(Modifier.width(2.dp).fillMaxHeight().background(dcs.outline)); Spacer(Modifier.width(10.dp))
                        MdBlocks(b.blocks, pal, tid)
                    }
                    is MdBlock.Table -> TableBlock(b, pal)
                    is MdBlock.Img -> ImageBlock(b, tid)
                    MdBlock.Rule -> Box(Modifier.fillMaxWidth().height(1.dp).background(dcs.outlineVariant))
                }
            }
        }
    }
}

@Composable
private fun CodeBlock(b: MdBlock.Code) {
    val dcs = MaterialTheme.colorScheme
    val clip = LocalClipboardManager.current
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copied) { if (copied) { delay(1500); copied = false } }
    Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant))) {
        Row(Modifier.fillMaxWidth().padding(start = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(b.lang, Modifier.weight(1f), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.sp)
            Text(if (copied) "Copied" else "Copy", Modifier.clickable { clip.setText(AnnotatedString(b.text)); copied = true }.padding(horizontal = 10.dp, vertical = 6.dp).testTag("md.copy"), color = dcs.primary, fontFamily = BpMono, fontSize = 11.sp)
        }
        Text(b.text, Modifier.horizontalScroll(rememberScrollState()).padding(start = 10.dp, end = 10.dp, bottom = 10.dp), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, lineHeight = 17.sp, softWrap = false)
    }
}

@Composable
private fun TableBlock(b: MdBlock.Table, pal: MdPalette) {
    val dcs = MaterialTheme.colorScheme
    val cols = maxOf(b.head.size, b.rows.maxOfOrNull { it.size } ?: 0)
    Row(Modifier.horizontalScroll(rememberScrollState()).border(BorderStroke(1.dp, dcs.outlineVariant)).testTag("md.table")) {
        for (c in 0 until cols) {
            Column(Modifier.width(IntrinsicSize.Max)) {
                fun al(): TextAlign = when (b.aligns.getOrNull(c)) { 1 -> TextAlign.Center; 2 -> TextAlign.End; else -> TextAlign.Start }
                Text(mdInline(b.head.getOrElse(c) { "" }, pal), Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 10.dp, vertical = 6.dp), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, fontWeight = FontWeight.Bold, softWrap = false, textAlign = al())
                b.rows.forEach { r -> Text(mdInline(r.getOrElse(c) { "" }, pal), Modifier.fillMaxWidth().border(BorderStroke(.5.dp, dcs.outlineVariant)).padding(horizontal = 10.dp, vertical = 6.dp), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, softWrap = false, textAlign = al()) }
            }
        }
    }
}

@Composable
private fun ImageBlock(b: MdBlock.Img, tid: String?) {
    val dcs = MaterialTheme.colorScheme
    var bmp by remember(b.url) { mutableStateOf(MdPics.cache.get(b.url)) }
    var failed by remember(b.url) { mutableStateOf(false) }
    LaunchedEffect(b.url) { if (bmp == null) { bmp = MdPics.load(b.url, tid); failed = bmp == null } }
    val m = bmp
    when {
        m != null -> androidx.compose.foundation.Image(m, b.alt, Modifier.widthIn(max = (m.width / 2).dp).heightIn(max = 360.dp).border(BorderStroke(1.dp, dcs.outlineVariant)).testTag("md.image"), contentScale = ContentScale.Fit, alignment = Alignment.CenterStart)
        failed -> Text(b.alt.ifEmpty { b.url }, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp)
        else -> Box(Modifier.fillMaxWidth().height(96.dp).background(dcs.surfaceVariant))
    }
}

/** Debug check: the text as a message, or arriving 7 characters every 60 ms. */
@Composable
fun MdPreview(full: String, stream: Boolean) {
    // the check's picture stands in for any image address (the app only allows plain http to the hub)
    remember { java.io.File("/data/local/tmp/pic.png").takeIf { it.exists() }?.readBytes()?.let { b -> BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap()?.let { bm -> Regex("\\]\\(([^)]+)\\)").findAll(full).forEach { m -> MdPics.cache.put(m.groupValues[1], bm) } } }; 0 }
    var n by remember { mutableStateOf(if (stream) 0 else full.length) }
    LaunchedEffect(Unit) { while (n < full.length) { delay(60); n = (n + 7).coerceAtMost(full.length) } }
    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).statusBarsPadding().verticalScroll(rememberScrollState()).padding(14.dp)) { MdText(full.take(n)) }
}
