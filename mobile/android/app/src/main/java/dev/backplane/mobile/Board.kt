package dev.backplane.mobile

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.unit.dp
import org.json.JSONObject
import org.json.JSONArray

// Presentation-only models. Bend supplies sections, filters, due flags and actions.
data class BoardModel(val title: String, val filters: List<SetButton>, val sections: List<BoardSection>, val add: BoardAdd)
data class BoardAdd(val name: String, val hint: String, val text: String)
data class BoardSection(val title: String, val count: Int, val rows: List<BoardRow>)
data class BoardRow(val id: String, val title: String, val sub: String, val status: String, val mark: String,
    val overdue: Boolean, val dueSoon: Boolean, val open: Boolean, val notes: String, val edit: String, val editNotes: String,
    val links: List<SetButton>, val buttons: List<SetButton>)
private fun boardButtons(a: JSONArray?): List<SetButton> = if (a == null) emptyList() else (0 until a.length()).map { i ->
    val b = a.getJSONObject(i); SetButton(b.optString("label"), b.optString("action"), b.optString("value"), b.optBoolean("on"))
}
fun parseBoard(o: JSONObject): BoardModel {
    val a = o.getJSONObject("add")
    val ss = o.getJSONArray("sections")
    return BoardModel(o.optString("title"), boardButtons(o.optJSONArray("filters")), (0 until ss.length()).map { i ->
        val s = ss.getJSONObject(i); val rs = s.getJSONArray("rows")
        BoardSection(s.optString("title"), s.optInt("count"), (0 until rs.length()).map { n ->
            val r = rs.getJSONObject(n)
            BoardRow(r.optString("id"), r.optString("title"), r.optString("sub"), r.optString("status"), r.optString("mark"),
                r.optBoolean("overdue"), r.optBoolean("dueSoon"), r.optBoolean("open"), r.optString("notes"), r.optString("edit"), r.optString("editNotes"),
                boardButtons(r.optJSONArray("links")), boardButtons(r.optJSONArray("buttons")))
        })
    }, BoardAdd(a.optString("name"), a.optString("hint"), a.optString("text")))
}

@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun BoardScreen(m: AppModel, b: BoardModel, hub: String) {
    fun routed(v: String) = if (v.startsWith("$hub|")) v else "$hub|$v"
    var title by remember(hub) { mutableStateOf(b.add.text) }
    fun add() { m.field(b.add.name, title); m.act("tb-add", "$hub|"); title = "" }
    Scaffold(topBar = { TopAppBar(title = { Text(b.title) }, actions = { TextButton(onClick = { m.act("tb-close", "$hub|") }) { Text("Close") } }) }) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding).imePadding()) {
            item(key = "add") {
                Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(title, { title = it; m.field(b.add.name, it) }, label = { Text(b.add.hint) }, modifier = Modifier.weight(1f), singleLine = true,
                        keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Done),
                        keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { add() }))
                    TextButton(onClick = { add() }) { Text("Add") }
                }
                FlowRow(Modifier.padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    b.filters.forEach { f -> FilterChip(selected = f.on, onClick = { m.act(f.action, routed(f.value)) }, label = { Text(f.label) }) }
                }
            }
            b.sections.forEach { s ->
                stickyHeader(key = "section:${s.title}") { Text("${s.title} · ${s.count}", Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface).padding(12.dp), style = MaterialTheme.typography.titleSmall) }
                items(s.rows, key = { it.id }) { r ->
                    val swipe = rememberSwipeToDismissBoxState(confirmValueChange = { v ->
                        when (v) {
                            SwipeToDismissBoxValue.StartToEnd -> m.act("tb-status", routed(r.id) + "\tdone")
                            SwipeToDismissBoxValue.EndToStart -> m.act("tb-status", routed(r.id) + "\tdropped")
                            else -> {}
                        }; false
                    })
                    SwipeToDismissBox(state = swipe, backgroundContent = { Row(Modifier.fillMaxSize().padding(16.dp), horizontalArrangement = Arrangement.SpaceBetween) { Text("Done"); Text("Drop") } }) {
                        Column(Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface).padding(12.dp)) {
                            Row(verticalAlignment = Alignment.Top) {
                                TextButton(onClick = { m.act("tb-tick", routed(r.id)) }) { Text(r.mark) }
                                Column(Modifier.weight(1f).clickable { m.act("tb-row", routed(r.id)) }.padding(vertical = 8.dp)) {
                                    Text(r.title)
                                    Text(r.sub, style = MaterialTheme.typography.bodySmall, color = if (r.overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                            if (r.open) BoardDetail(m, r)
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BoardDetail(m: AppModel, r: BoardRow) {
    var title by remember(r.id) { mutableStateOf(r.edit) }
    var notes by remember(r.id) { mutableStateOf(r.editNotes) }
    if (r.notes.isNotEmpty()) Text(r.notes)
    r.links.forEach { l -> TextButton(onClick = { m.act(l.action, l.value) }) { Text(l.label) } }
    OutlinedTextField(title, { title = it; m.field("tbedit", it) }, label = { Text("Title") }, modifier = Modifier.fillMaxWidth())
    TextButton(onClick = { m.field("tbedit", title); m.act("tb-title", r.id) }) { Text("Rename") }
    OutlinedTextField(notes, { notes = it; m.field("tbnotes", it) }, label = { Text("Notes") }, modifier = Modifier.fillMaxWidth(), minLines = 3)
    TextButton(onClick = { m.field("tbnotes", notes); m.act("tb-notes", r.id) }) { Text("Save notes") }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) { r.buttons.forEach { b -> TextButton(onClick = { m.act(b.action, b.value) }) { Text(b.label) } } }
}
