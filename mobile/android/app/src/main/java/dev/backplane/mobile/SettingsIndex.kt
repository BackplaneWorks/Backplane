package dev.backplane.mobile

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject

/** Settings' section index: each heading on the page registers its title and where it sits in the scrolled content. */
internal class SettingsIndex {
    val titles = mutableStateMapOf<String, String>()
    val ys = mutableStateMapOf<String, Int>()
    private val heads = HashMap<String, LayoutCoordinates>()
    private var content: LayoutCoordinates? = null

    fun content(c: LayoutCoordinates) { content = c; heads.keys.toList().forEach(::measure) }
    fun head(id: String, c: LayoutCoordinates) { heads[id] = c; measure(id) }
    fun leave(id: String) { heads.remove(id); titles.remove(id); ys.remove(id) }

    // the heading's y in the content, whatever the scroll (both are the content's own coordinates)
    private fun measure(id: String) {
        val c = content ?: return; val h = heads[id] ?: return
        if (!c.isAttached || !h.isAttached) return
        val y = c.localPositionOf(h, Offset.Zero).y.toInt()
        if (ys[id] != y) ys[id] = y
    }
}

internal val LocalSettingsIndex = staticCompositionLocalOf<SettingsIndex?> { null }

/** A Settings section's small heading, drawn as before; inside Settings it also joins the index at the top. */
@Composable
internal fun SettingsSection(id: String, title: String, upper: Boolean = false) {
    val ix = LocalSettingsIndex.current
    if (ix != null) {
        DisposableEffect(ix, id) { onDispose { ix.leave(id) } }
        SideEffect { if (ix.titles[id] != title) ix.titles[id] = title }
    }
    DMono(if (upper) title.uppercase() else title, dfaint, modifier = if (ix == null) Modifier else Modifier.onGloballyPositioned { ix.head(id, it) })
}

/** The index: a chip per section, in page order; a tap scrolls the section's heading to the top. No section, no index. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SettingsIndexChips(ix: SettingsIndex, scroll: ScrollState) {
    val shown = SettingsNav.order(ix.titles, ix.ys)
    if (shown.isEmpty()) return
    val scope = rememberCoroutineScope()
    val gap = with(LocalDensity.current) { 8.dp.roundToPx() }
    FlowRow(Modifier.testTag("settings.index"), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        shown.forEach { (id, title) ->
            DBtn(SettingsNav.chip(title), false, small = true, modifier = Modifier.testTag("settings.index.$id")) {
                val y = ix.ys[id] ?: return@DBtn
                scope.launch { scroll.animateScrollTo((y - gap).coerceAtLeast(0)) }
            }
        }
    }
    DLine()
}

/** What Settings' index and fold decide. Pure: SettingsNavTest. */
object SettingsNav {
    /** The sections measured so far, top to bottom: (id, title). */
    fun order(titles: Map<String, String>, ys: Map<String, Int>): List<Pair<String, String>> =
        titles.entries.filter { ys.containsKey(it.key) }.sortedBy { ys[it.key] }.map { it.key to it.value }

    /** A chip's label: the heading in normal case ("VOICE" -> "Voice"); a heading in mixed case or with no case (日本語) stays. */
    fun chip(title: String): String {
        val t = title.trim()
        if (t != t.uppercase() || t == t.lowercase()) return t
        return t.lowercase().replaceFirstChar { it.titlecase() }
    }

    /** A page's rows split into the plain ones and the ones marked `adv` (or named in [extra]), each in page order. */
    fun split(rows: List<JSONObject>, extra: Set<String> = emptySet()): Pair<List<JSONObject>, List<JSONObject>> =
        rows.partition { !(it.optBoolean("adv") || it.optString("label") in extra) }

    /** Debug builds only: `adb shell setprop debug.bp.adv "Label|Label"` marks those rows as adv, to see the fold before the hub sends the mark. */
    val debugAdv: Set<String> by lazy {
        if (!BuildConfig.DEBUG) emptySet() else runCatching {
            Runtime.getRuntime().exec(arrayOf("getprop", "debug.bp.adv")).inputStream.bufferedReader().use { it.readText() }
        }.getOrDefault("").split('|').map { it.trim() }.filter { it.isNotEmpty() }.toSet()
    }
}
