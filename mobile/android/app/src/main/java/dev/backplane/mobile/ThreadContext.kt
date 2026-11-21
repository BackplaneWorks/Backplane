package dev.backplane.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.launch
import org.json.JSONObject

private fun JSONObject.rows(key: String): List<JSONObject> =
    optJSONArray(key)?.let { a -> (0 until a.length()).mapNotNull(a::optJSONObject) }.orEmpty()

@Composable
internal fun ThreadContextSheet(thread: String, close: () -> Unit) {
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    var snapshot by remember(thread) { mutableStateOf<JSONObject?>(null) }
    var tab by remember(thread) { mutableIntStateOf(0) }
    var expanded by remember(thread) { mutableStateOf("") }
    var busy by remember(thread) { mutableStateOf(false) }
    var error by remember(thread) { mutableStateOf("") }
    suspend fun load(action: String = "", config: JSONObject? = null) {
        if (busy) return
        busy = true
        error = ""
        try {
            val params = JSONObject().put("thread", thread)
            if (config != null) params.put("path", config.optString("path")).put("sha256", config.optString("sha256")).put("action", action)
            snapshot = DataClient.call(if (config == null) "thread.context" else "thread.context.mcp", params)
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            error = e.message.orEmpty()
        } finally {
            busy = false
        }
    }
    LaunchedEffect(thread) { load() }
    Dialog(onDismissRequest = close, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxWidth(.95f).fillMaxHeight(.9f).background(dcs.surface).padding(12.dp)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(Tr.t("Context & Prompt"), Modifier.weight(1f), color = dcs.onSurface)
                TextButton(onClick = { scope.launch { load() } }, enabled = !busy) { Text(Tr.t("Refresh")) }
                TextButton(onClick = close) { Text(Tr.t("Close")) }
            }
            snapshot?.optString("cwd")?.let { DMono(it, dcs.onSurfaceVariant, 11) }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                listOf("Instructions", "Prompt", "Skills", "MCPs").forEachIndexed { index, title ->
                    TextButton(onClick = { tab = index }, modifier = Modifier.weight(1f)) {
                        Text(Tr.t(title), color = if (tab == index) dcs.primary else dcs.onSurfaceVariant)
                    }
                }
            }
            DLine()
            if (busy) DMono(Tr.t("Reading thread context…"), dcs.onSurfaceVariant, 11)
            if (error.isNotEmpty()) Text(error, color = dcs.error)
            val data = snapshot
            if (data != null) key(tab) {
                LazyColumn(Modifier.weight(1f).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    when (tab) {
                        0 -> {
                            val files = data.rows("instructions")
                            if (files.isEmpty()) item { Text(Tr.t("No project instructions found.")) }
                            files.forEach { row -> item {
                                val path = row.optString("path")
                                Column(Modifier.fillMaxWidth().clickable { expanded = if (expanded == path) "" else path }.padding(6.dp)) {
                                    ContextStatus(path, row.optString("status"))
                                    DMono(Msg.thread_context_size(row.optLong("bytes"), row.optLong("estimated_tokens")), dcs.onSurfaceVariant, 11)
                                    if (expanded == path) {
                                        ContextCode(row.optString("preview"))
                                        if (row.optString("note").isNotEmpty()) Text(row.optString("note"))
                                    }
                                }
                                DLine()
                            } }
                        }
                        1 -> {
                            item {
                                Text(Tr.t("Backplane’s launch prompt. Native files are loaded by the provider."))
                                TextButton(onClick = {
                                    val clipboard = context.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                                    clipboard.setPrimaryClip(android.content.ClipData.newPlainText("Backplane", data.optString("prompt")))
                                }) { Text(Tr.t("Copy prompt")) }
                            }
                            val sections = data.optJSONObject("sections")
                            listOf("base" to "Backplane", "instructions" to "Injected project instructions", "skills" to "Available skills index").forEach { (key, label) ->
                                val text = sections?.optString(key).orEmpty()
                                if (text.isNotEmpty()) item { Text(Tr.t(label), color = dcs.primary); ContextCode(text) }
                            }
                        }
                        2 -> listOf("Project", "User", "Bundled").forEach { group ->
                            val skills = data.rows("skills").filter { it.optString("scope") == group }
                            if (skills.isNotEmpty()) item { Text(Tr.t(group), color = dcs.primary) }
                            skills.forEach { row -> item {
                                ContextStatus(row.optString("name"), row.optString("status"))
                                Text(row.optString("description"))
                                DMono(listOf("source", "provider", "model").map { row.optString(it) }.filter { it.isNotEmpty() }.joinToString(" · "), dcs.onSurfaceVariant, 11)
                                DMono(row.optString("path"), dcs.onSurfaceVariant, 10)
                                DLine()
                            } }
                        }
                        else -> {
                            val configs = data.rows("mcp")
                            if (configs.isEmpty()) item { Text(Tr.t("No project MCP configurations found.")) }
                            configs.forEach { config -> item {
                                val state = config.optString("state")
                                ContextStatus(config.optString("path"), state)
                                config.rows("servers").forEach { server ->
                                    ContextStatus(server.optString("name"), server.optString("status"))
                                    val args = server.optJSONArray("args")
                                    val argv = listOf(server.optString("command")) + (0 until (args?.length() ?: 0)).map { args!!.optString(it) }
                                    ContextCode(server.optString("url").ifEmpty { argv.joinToString(" ") { JSONObject.quote(it) } })
                                }
                                if (config.optString("error").isNotEmpty()) Text(config.optString("error"), color = dcs.error)
                                val action = when (state) {
                                    "Untrusted" -> "trust" to "Trust and enable"
                                    "Enabled" -> "disable" to "Disable"
                                    "Disabled" -> "enable" to "Enable"
                                    else -> null
                                }
                                if (action != null) TextButton(onClick = { scope.launch { load(action.first, config) } }, enabled = !busy) { Text(Tr.t(action.second)) }
                                DLine()
                            } }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ContextStatus(title: String, status: String) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, Modifier.weight(1f), color = dcs.onSurface)
        DMono(Tr.t(status), if (status == "Native" || status == "Enabled") dcs.primary else dcs.onSurfaceVariant, 11)
    }
}

@Composable
private fun ContextCode(text: String) {
    SelectionContainer {
        DMono(text, dcs.onSurface, 12, Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(8.dp))
    }
}
