package dev.backplane.mobile

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

// Feature `cats` (docs/PHONE-DATA.md): a bot's cat in the lists and Settings' "Your uberbot". The hub answers each rig (`cat.rig`, already
// tinted, the look a name gives worked out by it); this keeps them by (look, colour, mood), which never change.
object Cats {
    val rigs = mutableStateMapOf<String, List<CatPart>>()
    /** Settings' colour row (name, "#rrggbb"), from the last rig read. */
    var presets by mutableStateOf<List<Pair<String, String>>>(emptyList()); private set
    /** The look each name gives (lower case: the hub's hash ignores case). */
    private val nameLooks = HashMap<String, Int>()
    private val asking = HashSet<String>()

    class Answer(val look: Int, val color: String, val parts: List<CatPart>)

    /** `cat.rig` with these parameters; the answer is kept under its own look and colour (and its name's look, when asked by name). */
    suspend fun ask(p: Map<String, Any>): Answer? {
        if (!DataClient.has("cats")) return null
        val q = JSONObject(); p.forEach { (k, v) -> q.put(k, v) }
        val r = runCatching { DataClient.call("cat.rig", q) }.getOrNull() ?: return null
        val a = Answer(r.optInt("look"), r.optString("color"), catRig(r.optJSONArray("parts") ?: JSONArray()))
        rigs[CatPick.key(a.look, a.color, p["mood"] as? String ?: "idle")] = a.parts
        (p["name"] as? String)?.let { nameLooks[it.lowercase()] = a.look }
        r.optJSONArray("presets")?.let { arr ->
            val ps = (0 until arr.length()).mapNotNull { arr.optJSONObject(it) }.map { it.optString("name") to it.optString("color") }
            if (ps.isNotEmpty() && ps != presets) presets = ps
        }
        return a
    }
    /** A bot's rig, read once. */
    suspend fun want(look: Int, color: String, mood: String = "idle") {
        val k = CatPick.key(look, color, mood)
        if (k in rigs || !asking.add(k)) return
        try { ask(mapOf("look" to look, "color" to color, "mood" to mood)) } finally { asking.remove(k) }
    }
    /** The look a name gives. */
    suspend fun lookOf(name: String): Int? = nameLooks[name.lowercase()] ?: ask(mapOf("name" to name, "mood" to "idle"))?.look
}

/** A bot's cat at `size`, from its `bots.list` row; room for it until its rig is in. */
@Composable
fun BotCat(look: Int, color: String, size: Dp) {
    val k = CatPick.key(look, color)
    LaunchedEffect(k) { Cats.want(look, color) }
    val r = Cats.rigs[k]
    if (r != null) CatView(r, size) else Spacer(Modifier.size(size))
}

/** the preview's crossfade, by name for tools (not shown) */
private const val CROSSFADE = "uber-cat"

private fun swatch(c: String): Color = CatPick.hex(c)?.let { Color(0xFF000000 or it.drop(1).toLong(16)) } ?: Color.Gray

/** Settings: "Your uberbot". Its name with a live cat beside it, the colours, Different cat and The name's cat; each change goes to
 *  `bot.cat` and the bots are read again. */
@Composable
internal fun UberSettings() {
    val b = DataClient.bots.firstOrNull { it.uber }
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        DMono(Msg.gpui_f_uber_title.uppercase(), dfaint)
        if (b == null) DMono(Msg.native_home_none, dcs.onSurfaceVariant, 12, Modifier.testTag("settings.uber.none"))
        else UberControls(b)
    }
}

@Composable
private fun UberControls(b: DataClient.Bot) {
    val scope = rememberCoroutineScope()
    var typed by remember(b.name) { mutableStateOf(b.name) }
    var nameLook by remember(b.name) { mutableStateOf<Int?>(null) }
    var custom by remember { mutableStateOf("") }
    var err by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var shown by remember { mutableStateOf<Pair<String, List<CatPart>>?>(null) }
    val picked = CatPick.picked(b.look, nameLook)
    val still = motionOff()
    // the look the name gives says whether the bot's look was picked
    LaunchedEffect(b.name) { nameLook = Cats.lookOf(b.name) }
    // the preview: a moment after the last key, the cat the name typed gives (or the picked one), faded in
    LaunchedEffect(typed, b.color, b.look, picked) {
        if (shown != null) delay(120)
        val a = Cats.ask(CatPick.ask(typed, b.color, b.look, picked)) ?: return@LaunchedEffect
        val k = CatPick.key(a.look, a.color)
        if (shown?.first != k) shown = k to a.parts
    }
    fun send(p: JSONObject) {
        if (busy) return
        busy = true
        scope.launch {
            try { DataClient.call("bot.cat", p); err = ""; if (p.has("color")) custom = "" } catch (e: Exception) { err = e.message ?: Msg.android_hub_refused }
            runCatching { DataClient.loadBots() }
            busy = false
        }
    }
    val name = typed.trim()
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(72.dp)) {
            Crossfade(targetState = shown, animationSpec = if (still) snap() else tween(150), label = CROSSFADE) { s -> if (s != null) CatView(s.second, 72.dp) }
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            DField(typed, { typed = it }, Msg.bots_name, Modifier.testTag("settings.uber.name"))
            DBtn(Msg.common_save, true, !busy && name.isNotEmpty() && name != b.name, Modifier.testTag("settings.uber.save")) { send(JSONObject().put("name", name)) }
        }
    }
    if (err.isNotEmpty()) DMono(err, dcs.error, 11, Modifier.testTag("settings.uber.err"), lines = 3)
    DMono(Msg.ios_viewerbg_color.uppercase(), dfaint)
    Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DBtn(Msg.cat_own, b.color.isEmpty(), !busy, Modifier.testTag("settings.uber.color.own")) { send(JSONObject().put("color", "")) }
        Cats.presets.forEach { (n, c) ->
            val on = CatPick.hex(c) == b.color
            Box(Modifier.size(34.dp).border(BorderStroke(2.dp, if (on) dcs.primary else Color.Transparent)).padding(3.dp).background(swatch(c))
                .clickable(enabled = !busy) { send(JSONObject().put("color", c)) }.semantics { contentDescription = n; selected = on }.testTag("settings.uber.color.$n"))
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DField(custom, { custom = it }, Msg.android_viewerbg_custom, Modifier.weight(1f).testTag("settings.uber.custom"))
        DBtn(Msg.gpui_use, false, !busy && custom.isNotBlank(), Modifier.testTag("settings.uber.custom.use")) {
            val c = CatPick.hex(custom)
            if (c == null) err = Msg.cat_color_bad else send(JSONObject().put("color", c))
        }
    }
    DMono(Msg.cat_cat.uppercase(), dfaint)
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DBtn(Msg.cat_different, false, !busy, Modifier.testTag("settings.uber.shuffle")) { send(JSONObject().put("shuffle", true)) }
        DBtn(Msg.cat_auto, false, !busy && picked, Modifier.testTag("settings.uber.auto")) { send(JSONObject().put("auto", true)) }
    }
}
