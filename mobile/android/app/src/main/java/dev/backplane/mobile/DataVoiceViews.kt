package dev.backplane.mobile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

private fun JSONArray?.objs(): List<JSONObject> = if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it) }

/** The composer's mic chip: red while the hub has a session on. */
@Composable
internal fun MicChip(on: Boolean, press: () -> Unit) {
    Box(Modifier.size(40.dp).background(if (on) dcs.error else dcs.surfaceVariant).border(BorderStroke(1.dp, if (on) dcs.error else dcs.outlineVariant)).clickable(onClick = press).testTag("thread.mic"), contentAlignment = Alignment.Center) {
        Icon(if (on) Icons.Default.Stop else Icons.Default.Mic, "Dictate", tint = if (on) dcs.onError else dcs.onSurfaceVariant, modifier = Modifier.size(18.dp))
    }
}

@Composable
private fun VChip(label: String, primary: Boolean = false, tag: String, press: () -> Unit) {
    Box(Modifier.heightIn(min = 36.dp).background(if (primary) dcs.primary else dcs.surfaceVariant).border(BorderStroke(1.dp, if (primary) dcs.primary else dcs.outlineVariant)).clickable(onClick = press).padding(horizontal = 10.dp).testTag(tag), contentAlignment = Alignment.Center) {
        DMono(label, if (primary) dcs.onPrimary else dcs.onSurface, 12)
    }
}

/** What the hub's `view` shows above the composer: a note, the strip (status and level, never the words), the fixes as chips, the word to clarify. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun VoiceStrip(tid: String) {
    val v = DataVoice.views[tid] ?: return
    val on = v.optBoolean("on"); val note = v.optString("note")
    val fixes = v.optJSONArray("fixes").objs(); val ask = v.optJSONObject("ask")
    if (!on && note.isEmpty() && fixes.isEmpty() && ask == null) return
    Column(Modifier.fillMaxWidth().testTag("voice"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (note.isNotEmpty()) Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(note, Modifier.weight(1f).testTag("voice.note"), color = if (v.optBoolean("bad")) dcs.error else dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp)
            VChip("Close", tag = "voice.close") { DataVoice.act(tid, "voice-err", "") }
        }
        if (on) Row(Modifier.fillMaxWidth().testTag("voice.strip"), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DDot(dcs.error, 8)
            Text(v.optString("status"), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, maxLines = 1, modifier = Modifier.testTag("voice.status"))
            Box(Modifier.width(72.dp).height(4.dp).background(dcs.outlineVariant).testTag("voice.level")) { Box(Modifier.fillMaxHeight().fillMaxWidth(DataVoice.level / 100f).background(dcs.primary)) }
            Spacer(Modifier.weight(1f))
            VChip(v.optString("stop").ifEmpty { "Stop" }, tag = "voice.stop") { DataVoice.stop(tid) }
        }
        if (fixes.isNotEmpty()) FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            fixes.forEachIndexed { i, f -> VChip(f.optString("label"), tag = "voice.fix.$i") { DataVoice.act(tid, f.optString("action"), f.optString("value")) } }
        }
        if (ask != null) {
            val field = ask.optJSONObject("field")
            var typed by remember(ask.optString("text"), field?.optString("value")) { mutableStateOf(field?.optString("value") ?: "") }
            Text(ask.optString("text"), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, modifier = Modifier.testTag("voice.ask"))
            if (ask.optBoolean("fixing") && field != null) DField(typed, { typed = it }, field.optString("hint").ifEmpty { "The right spelling" }, Modifier.testTag("voice.fixfield"))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                ask.optJSONArray("buttons").objs().forEachIndexed { n, b ->
                    val action = b.optString("action")
                    VChip(b.optString("label"), b.optBoolean("primary"), "voice.ask.$n") { DataVoice.act(tid, action, if (action == "voice-fixed") typed else b.optString("value")) }
                }
            }
        }
    }
}

/** Settings, Voice: the hub's rows (label, note, buttons, fields); a press goes to the hub. */
@Composable
internal fun VoiceSettings() {
    LaunchedEffect(DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE) DataVoice.loadPage() }
    PageRows("VOICE", "voice", DataVoice.rows, DataVoice.pageError) { action, value, sent ->
        DataVoice.pageAct(action, value, sent)
        // the key and the term are spent once the hub has them
        setOf("secret-save" to "kval", "voice-term" to "vterm").firstOrNull { it.first == action }?.second
    }
}

/**
 * The hub's rows {label, note, tone, buttons:[{label, action, value, on}], fields:[{name, hint, secret, value}]}, drawn the same for every page that has them (Voice, Machines).
 * A row may carry a code to scan: `qr` (one string of '0'/'1' per module row, '1' dark, no quiet zone) and `qrnote`, the line under it.
 * A press calls [act] with the button's action, value and the texts typed; it returns the name of a typed field to clear, if any.
 * The phone's own actions never go to the hub (PageRow.local): `copy` (the value to the clipboard), `share-text` (the share sheet), `open-url` (the browser).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun PageRows(title: String, tag: String, rows: List<org.json.JSONObject>, error: String, act: suspend (String, String, Map<String, String>) -> String?) {
    val scope = rememberCoroutineScope()
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val typed = remember { mutableStateMapOf<String, String>() }
    Column(Modifier.fillMaxWidth().testTag("settings.$tag"), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        DMono(title, dfaint)
        if (error.isNotEmpty()) DMono(error, dcs.error, 12)
        rows.forEach { r ->
            val label = r.optString("label"); val tone = r.optString("tone")
            Column(Modifier.fillMaxWidth().testTag("$tag.row.$label"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(label, color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp)
                if (r.optString("note").isNotEmpty()) Text(r.optString("note"), color = when (tone) { "bad" -> dcs.error; "warn" -> dwarn; else -> dcs.onSurfaceVariant }, fontFamily = BpMono, fontSize = 11.sp,
                    modifier = Modifier.testTag("$tag.note.$label"))
                // a code to scan (a pairing link's), as the hub drew it, and its line (the code, the address, until when)
                val qr = PageRow.qr(r)
                if (qr.isNotEmpty()) QrCode(qr, Modifier.fillMaxWidth().testTag("$tag.qr.$label"))
                if (r.optString("qrnote").isNotEmpty()) Text(r.optString("qrnote"), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, modifier = Modifier.testTag("$tag.qrnote.$label"))
                r.optJSONArray("fields").objs().forEach { f ->
                    val name = f.optString("name")
                    val v = typed[name] ?: f.optString("value")
                    Box(Modifier.fillMaxWidth().background(dview).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 10.dp, vertical = 10.dp).testTag("$tag.field.$name")) {
                        if (v.isEmpty()) Text(f.optString("hint"), color = dfaint, fontFamily = BpMono, fontSize = 13.sp, maxLines = 1)
                        BasicTextField(v, { typed[name] = it }, textStyle = TextStyle(color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp), cursorBrush = SolidColor(dcs.primary), singleLine = true,
                            visualTransformation = if (f.optBoolean("secret")) PasswordVisualTransformation() else VisualTransformation.None, modifier = Modifier.fillMaxWidth())
                    }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    r.optJSONArray("buttons").objs().forEachIndexed { i, b ->
                        DBtn(b.optString("label"), b.optBoolean("on"), modifier = Modifier.testTag("$tag.btn.$label.$i")) {
                            val action = b.optString("action"); val value = b.optString("value"); val sent = typed.toMap()
                            if (!pageLocal(ctx, action, value)) scope.launch { act(action, value, sent)?.let { typed.remove(it) } }
                        }
                    }
                }
            }
            DLine()
        }
    }
}

/** What a page row says that is the phone's own business. Pure: DevicesPageTest. */
object PageRow {
    /** The row's code: its rows when they draw a square of '0'/'1' (anything else draws nothing, never half a code). */
    fun qr(r: JSONObject): List<String> {
        val a = r.optJSONArray("qr") ?: return emptyList()
        val rows = (0 until a.length()).map { a.opt(it) as? String ?: return emptyList() }
        if (rows.isEmpty() || rows.any { it.length != rows.size || it.any { c -> c != '0' && c != '1' } }) return emptyList()
        return rows
    }
    /** Done on the phone, never sent: copy (and the older copy-text), share-text, open-url (http and https only). */
    fun local(action: String): Boolean = action in setOf("copy", "copy-text", "share-text", "open-url")
    fun webUrl(value: String): Boolean { val v = value.trim().lowercase(); return (v.startsWith("https://") || v.startsWith("http://")) && v.length > v.indexOf("://") + 3 }
}

/** A press the phone does itself (PageRow.local): true when it was one, whether or not it could be done. */
internal fun pageLocal(ctx: android.content.Context, action: String, value: String): Boolean {
    if (!PageRow.local(action)) return false
    when (action) {
        "copy", "copy-text" -> {
            (ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager).setPrimaryClip(android.content.ClipData.newPlainText("Backplane", value))
            DataClient.say("Copied")
        }
        "share-text" -> runCatching {
            ctx.startActivity(android.content.Intent.createChooser(android.content.Intent(android.content.Intent.ACTION_SEND).setType("text/plain").putExtra(android.content.Intent.EXTRA_TEXT, value), null)
                .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK))
        }
        "open-url" -> if (PageRow.webUrl(value)) runCatching {
            ctx.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(value.trim())).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK))
        }.onFailure { DataClient.say("No app here opens that link.") }
    }
    return true
}
