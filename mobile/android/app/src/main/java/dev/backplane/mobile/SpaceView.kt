package dev.backplane.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

// A bot's space: the JSON src/mobile/space.bend emits (MSpace.json). Plain
// data; Bend has already kept what it understands and capped every size.
// A button calls act("space", send); an input calls
// act("space-input", send + "\u001f" + text).

data class SpaceBlock(
    val type: String, val text: String, val label: String, val value: String, val hint: String,
    val action: String, val placeholder: String, val tone: String, val pct: String, val send: String,
    val permille: Int, val items: List<String>, val head: List<String>, val rows: List<List<String>>,
    val blocks: List<SpaceBlock>,
) {
    // what a button or input sends: the bot's value, or the bare action
    val key: String get() = send.ifEmpty { action }
}

data class SpaceModel(val bot: String, val title: String, val blocks: List<SpaceBlock>)

private fun spaceStrs(a: JSONArray?): List<String> =
    if (a == null) emptyList() else (0 until a.length()).map { a.optString(it) }

private fun spaceRows(a: JSONArray?): List<List<String>> =
    if (a == null) emptyList() else (0 until a.length()).map { spaceStrs(a.optJSONArray(it)) }

private fun spaceBlocks(a: JSONArray?): List<SpaceBlock> =
    if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it)?.let(::spaceBlock) }

private fun spaceBlock(o: JSONObject): SpaceBlock = SpaceBlock(
    o.optString("type"), o.optString("text"), o.optString("label"),
    // a stat's value is text; a progress bar's is a number (read permille instead)
    if (o.opt("value") is String) o.optString("value") else "",
    o.optString("hint"), o.optString("action"), o.optString("placeholder"), o.optString("tone"),
    o.optString("pct"), o.optString("send"), o.optInt("permille"),
    spaceStrs(o.optJSONArray("items")), spaceStrs(o.optJSONArray("head")), spaceRows(o.optJSONArray("rows")),
    spaceBlocks(o.optJSONArray("blocks")),
)

fun spaceModel(o: JSONObject): SpaceModel =
    SpaceModel(o.optString("bot"), o.optString("title"), spaceBlocks(o.optJSONArray("blocks")))

@Composable
fun SpaceView(space: SpaceModel, act: (String, String) -> Unit) {
    LazyColumn(
        Modifier.fillMaxWidth(),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (space.title.isNotEmpty()) {
            item { Text(space.title, style = MaterialTheme.typography.titleLarge) }
        }
        itemsIndexed(space.blocks) { _, b -> SpaceBlockView(b, act) }
    }
}

@Composable
private fun SpaceBlockView(b: SpaceBlock, act: (String, String) -> Unit) {
    if (b.type == "row") {
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            b.blocks.forEach { SpaceLeaf(it, act) }
        }
    } else {
        SpaceLeaf(b, act)
    }
}

@Composable
private fun SpaceLeaf(b: SpaceBlock, act: (String, String) -> Unit) {
    val dim = MaterialTheme.colorScheme.onSurfaceVariant
    when (b.type) {
        "heading" -> Text(b.text, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 4.dp))
        "text" -> Text(b.text, style = MaterialTheme.typography.bodyMedium)
        "stat" -> Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = RoundedCornerShape(12.dp)) {
            Column(Modifier.widthIn(min = 110.dp).padding(12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(b.label, style = MaterialTheme.typography.labelMedium, color = dim)
                Text(b.value, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.SemiBold)
                if (b.hint.isNotEmpty()) Text(b.hint, style = MaterialTheme.typography.labelSmall, color = dim)
            }
        }
        "progress" -> Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(Modifier.fillMaxWidth()) {
                Text(b.label, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
                Text(b.pct, style = MaterialTheme.typography.bodyMedium, color = dim)
            }
            LinearProgressIndicator(progress = { b.permille / 1000f }, modifier = Modifier.fillMaxWidth())
        }
        "list" -> Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            b.items.forEach { s ->
                Row {
                    Text("•", color = dim)
                    Spacer(Modifier.width(8.dp))
                    Text(s, style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
        "kv" -> Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            b.rows.forEach { r ->
                Row(Modifier.fillMaxWidth()) {
                    Text(r.getOrElse(0) { "" }, style = MaterialTheme.typography.bodyMedium, color = dim)
                    Spacer(Modifier.width(12.dp))
                    Text(r.getOrElse(1) { "" }, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, textAlign = TextAlign.End)
                }
            }
        }
        "table" -> SpaceTable(b.head, b.rows)
        "button" -> OutlinedButton(onClick = { act("space", b.key) }) { Text(b.label) }
        "input" -> SpaceInput(b, act)
        "divider" -> HorizontalDivider()
        "badge" -> {
            val tint = when (b.tone) {
                "ok" -> Color(0xFF58C28A)
                "warn" -> Color(0xFFE3B341)
                "bad" -> Color(0xFFF06A6A)
                else -> dim
            }
            Text(
                b.text, color = tint, style = MaterialTheme.typography.labelMedium,
                modifier = Modifier.background(tint.copy(alpha = 0.15f), RoundedCornerShape(50)).padding(horizontal = 8.dp, vertical = 3.dp),
            )
        }
        "code" -> Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = RoundedCornerShape(8.dp), modifier = Modifier.fillMaxWidth()) {
            Text(
                b.text, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.horizontalScroll(rememberScrollState()).padding(10.dp),
            )
        }
        else -> {}
    }
}

@Composable
private fun SpaceTable(head: List<String>, rows: List<List<String>>) {
    val cols = maxOf(head.size, rows.maxOfOrNull { it.size } ?: 0)
    Column(Modifier.horizontalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (head.isNotEmpty()) {
            Row {
                (0 until cols).forEach { i ->
                    Text(head.getOrElse(i) { "" }, Modifier.width(110.dp), style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            HorizontalDivider(Modifier.width(110.dp * cols))
        }
        rows.forEach { r ->
            Row {
                (0 until cols).forEach { i -> Text(r.getOrElse(i) { "" }, Modifier.width(110.dp), style = MaterialTheme.typography.bodyMedium) }
            }
        }
    }
}

@Composable
private fun SpaceInput(b: SpaceBlock, act: (String, String) -> Unit) {
    val lang = LocalBackplaneLanguage.current
    var text by rememberSaveable(b.key) { mutableStateOf("") }
    val send = {
        val t = text.trim()
        if (t.isNotEmpty()) {
            act("space-input", b.key + "\u001f" + t)
            text = ""
        }
    }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                text, { text = it }, Modifier.weight(1f), singleLine = true,
                label = if (b.label.isEmpty()) null else ({ Text(b.label) }),
                placeholder = { Text(b.placeholder) },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(onSend = { send() }),
            )
            TextButton(onClick = send, enabled = text.isNotBlank()) { Text(NativeMessages.composer_send(lang)) }
        }
    }
}
