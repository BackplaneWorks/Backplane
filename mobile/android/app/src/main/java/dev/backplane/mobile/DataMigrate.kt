package dev.backplane.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import org.json.JSONObject

// A 0.14 home coming across (docs/PHONE-DATA.md, docs/IMPORT-0.14.md Progress): the hub's card, words and all. hello `migrating` paints it on connect and
// every data/1 `migrating` frame replaces it. The app adds no words of its own.
data class DMigrateButton(val label: String, val action: String, val value: String, val on: Boolean)
data class DMigrateCard(val state: String, val title: String, val body: String, val pct: Int?, val lines: List<String>, val buttons: List<DMigrateButton>) {
    /** while the import runs nothing closes it */
    val running get() = state == "running"
    /** the banner's two lines under the bar */
    val head get() = lines.take(2)
    companion object {
        /** The card of a hello `migrating` value or a `migrating` frame; null when there is nothing to show. */
        fun of(m: JSONObject?): DMigrateCard? {
            val c = m?.optJSONObject("card") ?: return null
            val title = c.optString("title"); if (title.isEmpty()) return null
            val lines = c.optJSONArray("lines")?.let { a -> (0 until a.length()).map { a.optString(it) }.filter(String::isNotEmpty) } ?: emptyList()
            val buttons = c.optJSONArray("buttons")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map { DMigrateButton(it.optString("label"), it.optString("action"), it.optString("value"), it.optBoolean("on")) }.filter { it.label.isNotEmpty() && it.action.isNotEmpty() } } ?: emptyList()
            val pct = if (c.isNull("pct") || !c.has("pct")) null else c.optInt("pct").coerceIn(0, 100)
            return DMigrateCard(c.optString("state", m.optString("state")), title, c.optString("body"), pct, lines, buttons)
        }
    }
}

/** The bar: filled to pct, square, no animation. */
@Composable
private fun MigrateBar(pct: Int?, modifier: Modifier = Modifier) {
    if (pct == null) return
    Box(modifier.fillMaxWidth().height(4.dp).background(dcs.outlineVariant)) {
        if (pct > 0) Box(Modifier.fillMaxWidth(pct / 100f).fillMaxHeight().background(dcs.primary).testTag("migrate.fill"))
    }
}

/** Over the lists and the thread: the card's title, its bar and its first two lines; a tap opens the card. It has no close. */
@Composable
internal fun MigrateBanner() {
    val c = DataClient.migrating ?: return
    Column(Modifier.testTag("migrate.banner").clickable { DataClient.migrateOpen = true }) {
        Column(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                DDot(if (c.state == "failed") dwarn else dcs.primary, 8); Spacer(Modifier.width(8.dp))
                Text(c.title, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                c.pct?.let { DMono("$it%", dcs.onSurfaceVariant, 11) }
            }
            if (c.pct != null) { Spacer(Modifier.height(5.dp)); MigrateBar(c.pct) }
            c.head.forEach { Spacer(Modifier.height(3.dp)); DMono(it, dcs.onSurfaceVariant, 11) }
        }
        DLine()
    }
    if (DataClient.migrateOpen) MigrateSheet(c)
}

/** The whole card: title, body, bar, every line and the hub's buttons (onboard.act). Closing it leaves the banner; only a button ends the card. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MigrateSheet(c: DMigrateCard) {
    val scope = rememberCoroutineScope()
    var err by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    ModalBottomSheet(onDismissRequest = { DataClient.migrateOpen = false }, shape = RectangleShape, containerColor = dcs.surfaceVariant, dragHandle = null, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 16.dp).navigationBarsPadding().testTag("migrate.card")) {
            Text(c.title, color = dcs.onSurface, fontFamily = BpMono, fontSize = 15.sp, fontWeight = FontWeight.Bold)
            if (c.body.isNotEmpty()) { Spacer(Modifier.height(8.dp)); Text(c.body, color = dcs.onSurface, fontSize = 14.sp) }
            if (c.pct != null) {
                Spacer(Modifier.height(14.dp))
                Row(verticalAlignment = Alignment.CenterVertically) { MigrateBar(c.pct, Modifier.weight(1f)); Spacer(Modifier.width(8.dp)); DMono("${c.pct}%", dcs.onSurfaceVariant, 12) }
            }
            if (c.lines.isNotEmpty()) Spacer(Modifier.height(10.dp))
            c.lines.forEach { Text(it, Modifier.padding(vertical = 2.dp), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp) }
            if (err.isNotEmpty()) { Spacer(Modifier.height(8.dp)); Text(err, color = dcs.error, fontFamily = BpMono, fontSize = 12.sp) }
            if (c.buttons.isNotEmpty()) {
                Spacer(Modifier.height(16.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    c.buttons.forEach { b ->
                        DBtn(b.label, b.on, !busy, Modifier.testTag("migrate.act.${b.value}")) {
                            busy = true
                            scope.launch { err = try { DataClient.migrateAct(c, b); "" } catch (e: Exception) { e.message ?: "" }; busy = false }
                        }
                    }
                }
            }
        }
    }
}
