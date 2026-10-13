package dev.backplane.mobile

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Forum
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL

// Bots (docs/bots.md): the cats and rooms under the projects, the forms
// that make them, and a bot's, a room's or a remote bot's screen. All of
// it is the screen's (src/mobile/bots.bend); these views only draw it and
// send actions back.

private const val SEP = "\u001f"

private val square = RoundedCornerShape(4.dp)

// how a mood looks next to a cat
@Composable
internal fun moodTint(mood: String): Color = when (mood) {
    "wait" -> Color(0xFFFF9F0A)
    "work", "talk" -> Color(0xFF30D158)
    "think" -> Color(0xFF0A84FF)
    "error" -> MaterialTheme.colorScheme.error
    "idle" -> Color(0xFF63E6BE)
    else -> MaterialTheme.colorScheme.outline
}

@Composable
internal fun MoodDot(mood: String) {
    Box(Modifier.size(7.dp).clip(CircleShape).background(moodTint(mood)))
}

// a cat by its key: its rig, once the bridge has given it
@Composable
fun BotCat(m: AppModel, key: String, size: Dp) {
    LaunchedEffect(key) { m.cat(key) }
    CatView(m.cats[key] ?: emptyList(), size)
}

// The list
// --------

fun LazyListScope.botsSection(m: AppModel, s: Screen) {
    item(key = "bots") {
        var menu by remember { mutableStateOf(false) }
        ListItem(
            headlineContent = { Text("Bots", style = MaterialTheme.typography.titleMedium) },
            trailingContent = {
                Box {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Filled.Add, "New bot or room") }
                    DropdownMenu(menu, { menu = false }) {
                        // with several hubs, a bot or room is made on the one chosen
                        val many = s.hubs.size > 1
                        for (h in if (many) s.hubs else s.hubs.take(1)) {
                            val v = if (many) h.key + "|" else ""
                            val on = if (many) " on " + h.name else ""
                            DropdownMenuItem(text = { Text("New bot$on") }, onClick = { menu = false; m.act("bot-new", v) })
                            DropdownMenuItem(text = { Text("New room$on") }, onClick = { menu = false; m.act("room-new", v) })
                        }
                    }
                }
            },
        )
    }
    items(s.bots, key = { "b:" + it.id }) { b ->
        ListItem(
            headlineContent = { Text(b.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            supportingContent = {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    MoodDot(b.mood)
                    Text(listOf(b.note, b.peer, b.machine).filter { it.isNotEmpty() }.joinToString(" · "),
                        maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            },
            leadingContent = { BotCat(m, b.cat, 40.dp) },
            modifier = Modifier.alpha(if (b.mood == "away") 0.5f else 1f)
                .clickable { m.open(b.nav, b.act, b.id, b.name, b.cat, if (b.remote) "remote" else "bot") },
        )
    }
    s.boardEntry?.let { b -> item(key = "board") { ListItem(headlineContent = { Text(b.label) }, modifier = Modifier.clickable { m.act("tb-open", s.hub + "|") }) } }
    items(s.rooms, key = { "r:" + it.id }) { r ->
        ListItem(
            headlineContent = { Text(r.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            supportingContent = { Text(listOf("${r.members}", r.machine).filter { it.isNotEmpty() }.joinToString(" · ")) },
            leadingContent = { Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) { Icon(Icons.Filled.Forum, null) } },
            modifier = Modifier.clickable { m.open(r.nav, "room", r.id, r.name, kind = "room") },
        )
    }
}

// Forms
// -----

@Composable
fun NewBotDialog(m: AppModel, f: NewBot) {
    var name by rememberSaveable { mutableStateOf(f.name) }
    var persona by rememberSaveable { mutableStateOf(f.persona) }
    AlertDialog(
        onDismissRequest = { m.act("form-close", "@bnew") },
        title = { Text("New bot") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(name, { name = it; m.field("name", it) }, Modifier.fillMaxWidth(), singleLine = true,
                    label = { Text(f.hint) })
                OutlinedTextField(persona, { persona = it; m.field("persona", it) }, Modifier.fillMaxWidth(), minLines = 3, maxLines = 8,
                    label = { Text("Persona") })
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (p in f.providers) FilterChip(selected = p.provider == f.provider, onClick = { m.act("bfield", "provider$SEP${p.provider}") },
                        label = { Text(p.label) }, shape = square)
                }
            }
        },
        confirmButton = { TextButton(onClick = { m.act("bot-create") }, enabled = name.isNotBlank()) { Text("Create") } },
        dismissButton = { TextButton(onClick = { m.act("form-close", "@bnew") }) { Text("Cancel") } },
        shape = square,
    )
}

@Composable
fun NewRoomDialog(m: AppModel, f: NewRoom) {
    var name by rememberSaveable { mutableStateOf(f.name) }
    AlertDialog(
        onDismissRequest = { m.act("form-close", "@rnew") },
        title = { Text("New room") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                OutlinedTextField(name, { name = it; m.field("rname", it) }, Modifier.fillMaxWidth(), singleLine = true,
                    label = { Text("Name") })
                Text("Members", Modifier.padding(top = 8.dp), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.outline)
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    for (p in f.picks) Row(Modifier.fillMaxWidth().clickable { m.act("room-pick", p.id) }.padding(vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically) {
                        Text(p.name, Modifier.weight(1f))
                        if (p.on) Icon(Icons.Filled.Check, "In the room")
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = { m.act("room-save") }, enabled = f.picks.any { it.on }) { Text("Save") } },
        dismissButton = { TextButton(onClick = { m.act("form-close", "@rnew") }) { Text("Cancel") } },
        shape = square,
    )
}

// The screens
// -----------

@Composable
fun BotDestination(m: AppModel, s: Screen, b: BotView) {
    when (b.kind) {
        "bot" -> BotScreen(m, s, b)
        "room" -> TalkScreen(m, b, "rpost", "room-post")
        else -> TalkScreen(m, b, "rtext", "remote-send")
    }
}

// a cat, its name and how it is
@Composable
private fun BotHeader(m: AppModel, b: BotView, size: Dp) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.alpha(if (b.mood == "away") 0.5f else 1f)) { BotCat(m, b.cat, size) }
        Column {
            Text(b.title, style = MaterialTheme.typography.titleLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                MoodDot(b.mood)
                Text(listOf(b.note, if (b.title == b.name) b.peer else "").filter { it.isNotEmpty() }.joinToString(" · "),
                    style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.outline)
            }
        }
    }
}

@Composable
private fun BotTabs(m: AppModel, b: BotView) {
    Column {
        BotHeader(m, b, 64.dp)
        val i = b.tabs.indexOfFirst { it.id == b.tab }.coerceAtLeast(0)
        ScrollableTabRow(selectedTabIndex = i, edgePadding = 8.dp) {
            b.tabs.forEachIndexed { j, t ->
                Tab(selected = j == i, onClick = { m.act("bot-tab", t.id) }, text = { Text(t.label) })
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun BotScreen(m: AppModel, s: Screen, b: BotView) {
    if (b.tab == "chat" && s.thread != null) {
        ThreadScreen(m, s, s.thread) { BotTabs(m, b) }
        return
    }
    Scaffold(topBar = {
        Column {
            TopAppBar(
                title = { Text(b.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = { m.back() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
            )
            BotTabs(m, b)
        }
    }) { pad ->
        Box(Modifier.fillMaxSize().padding(pad).imePadding()) {
            when (b.tab) {
                "status" -> StatusTab(m, m.plane(s))
                "space" -> if (b.page != null) SpacePage(m, b.page) else b.space?.let { SpaceView(it) { a, v -> m.act(a, v) } }
                "browser" -> BrowserTab(m, b.browser)
                "memory" -> MemoryTab(m, b.memory)
                "routines" -> RoutinesTab(m, b)
                "hooks" -> HooksTab(m, b)
                "settings" -> b.settings?.let { SettingsTab(m, b, it) }
            }
        }
    }
}

@Composable
private fun Empty(text: String) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(text, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.outline)
    }
}

// the bot's page, as the hub's latest frame (fetched again when n moves)
@Composable
private fun BrowserTab(m: AppModel, b: BotBrowser?) {
    val url = b?.let { m.hubUrl(it.url, "n=" + it.n) }
    var image by remember { mutableStateOf<ImageBitmap?>(null) }
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
    val img = image
    if (img == null) Empty("No page yet")
    else Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Image(img, "The bot's page", Modifier.fillMaxWidth(), contentScale = ContentScale.FillWidth)
    }
}

@Composable
private fun Tag(text: String) {
    Text(text, Modifier.background(MaterialTheme.colorScheme.surfaceVariant, square).padding(horizontal = 5.dp, vertical = 1.dp),
        style = MaterialTheme.typography.labelSmall.copy(fontFamily = FontFamily.Monospace))
}

@Composable
private fun MemoryTab(m: AppModel, items: List<BotMemory>) {
    if (items.isEmpty()) { Empty("Nothing remembered"); return }
    LazyColumn(Modifier.fillMaxSize()) {
        items(items, key = { it.key }) { x ->
            ListItem(
                overlineContent = {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(x.key, fontWeight = FontWeight.SemiBold)
                        Tag(x.kind)
                        Text(x.updated)
                    }
                },
                headlineContent = { Text(x.text) },
                supportingContent = { if (x.tags.isNotEmpty()) Text(x.tags) },
                trailingContent = { IconButton(onClick = { m.act("mem-forget", x.key) }) { Icon(Icons.Filled.Close, "Forget") } },
            )
            HorizontalDivider()
        }
    }
}

@Composable
private fun RoutinesTab(m: AppModel, b: BotView) {
    LazyColumn(Modifier.fillMaxSize()) {
        item(key = "new") {
            ListItem(headlineContent = { Text("New routine") }, leadingContent = { Icon(Icons.Filled.Add, null) },
                modifier = Modifier.clickable { m.act("routine-edit", "") })
            HorizontalDivider()
        }
        items(b.routines, key = { it.id }) { r ->
            var menu by remember { mutableStateOf(false) }
            ListItem(
                headlineContent = { Text(r.name, fontWeight = FontWeight.SemiBold) },
                supportingContent = {
                    Column {
                        Text(r.schedule, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.labelMedium)
                        Text(r.prompt, maxLines = 2, overflow = TextOverflow.Ellipsis)
                        if (r.last.isNotEmpty()) Text("last " + r.last, style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.outline)
                    }
                },
                trailingContent = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Switch(r.on, { m.act("routine-toggle", r.id) })
                        Box {
                            IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, "More") }
                            DropdownMenu(menu, { menu = false }) {
                                DropdownMenuItem(text = { Text("Run now") }, onClick = { menu = false; m.act("routine-run", r.id) })
                                DropdownMenuItem(text = { Text("Edit") }, onClick = { menu = false; m.act("routine-edit", r.id) })
                                DropdownMenuItem(text = { Text("Delete", color = MaterialTheme.colorScheme.error) },
                                    onClick = { menu = false; m.act("routine-delete", r.id) })
                            }
                        }
                    }
                },
                modifier = Modifier.clickable { m.act("routine-edit", r.id) },
            )
            HorizontalDivider()
        }
    }
    b.routine?.takeIf { it.open }?.let { RoutineDialog(m, it) }
}

@Composable
private fun RoutineDialog(m: AppModel, f: RoutineForm) {
    var name by remember(f.id) { mutableStateOf(f.name) }
    var cron by remember(f.id) { mutableStateOf(f.cron) }
    var prompt by remember(f.id) { mutableStateOf(f.prompt) }
    AlertDialog(
        onDismissRequest = { m.act("form-close", "@onew") },
        title = { Text(if (f.id.isEmpty()) "New routine" else "Routine") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(name, { name = it; m.field("oname", it) }, Modifier.fillMaxWidth(), singleLine = true,
                    label = { Text("Name") })
                OutlinedTextField(cron, { cron = it; m.field("ocron", it) }, Modifier.fillMaxWidth(), singleLine = true,
                    label = { Text("When") }, placeholder = { Text("0 9 * * 1-5") },
                    supportingText = { Text("Cron, @daily, every 15m, weekdays 08:00") },
                    textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false))
                OutlinedTextField(prompt, { prompt = it; m.field("oprompt", it) }, Modifier.fillMaxWidth(), minLines = 3, maxLines = 10,
                    label = { Text("Prompt") })
            }
        },
        confirmButton = { TextButton(onClick = { m.act("routine-save") }, enabled = cron.isNotBlank()) { Text("Save") } },
        dismissButton = { TextButton(onClick = { m.act("form-close", "@onew") }) { Text("Cancel") } },
        shape = square,
    )
}

@Composable
private fun HooksTab(m: AppModel, b: BotView) {
    var name by rememberSaveable { mutableStateOf("") }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 16.dp)) {
        item(key = "new") {
            Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(name, { name = it; m.field("hname", it) }, Modifier.weight(1f), singleLine = true,
                    label = { Text("Name") })
                TextButton(onClick = { m.act("hook-new"); name = "" }) { Text("Add") }
            }
        }
        if (b.secret.isNotEmpty()) item(key = "secret") {
            Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("Secret, shown once", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.outline)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(b.secret, Modifier.weight(1f), fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                    IconButton(onClick = { m.act("copy", b.secret) }) { Icon(Icons.Filled.ContentCopy, "Copy") }
                }
                HorizontalDivider()
            }
        }
        items(b.hooks, key = { it.id }) { h ->
            val url = m.hubUrl(h.path, token = false) ?: h.path
            ListItem(
                headlineContent = { Text(h.name, fontWeight = FontWeight.SemiBold) },
                supportingContent = {
                    Column {
                        Text(url, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall,
                            maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(if (h.last.isEmpty()) "${h.count}" else "${h.count} · ${h.last}", style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.outline)
                    }
                },
                trailingContent = {
                    Row {
                        IconButton(onClick = { m.act("copy", url) }) { Icon(Icons.Filled.ContentCopy, "Copy URL") }
                        IconButton(onClick = { m.act("hook-revoke", h.id) }) { Icon(Icons.Filled.Close, "Revoke") }
                    }
                },
            )
            HorizontalDivider()
        }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(text, Modifier.padding(top = 20.dp, bottom = 4.dp), style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.outline)
}

@Composable
private fun Field(value: String, label: String, onChange: (String) -> Unit, modifier: Modifier = Modifier.fillMaxWidth(), secret: Boolean = false) {
    OutlinedTextField(value, onChange, modifier, singleLine = true, label = { Text(label) },
        visualTransformation = if (secret) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false))
}

@Composable
private fun SettingsTab(m: AppModel, b: BotView, st: BotSettings) {
    var persona by remember(b.id) { mutableStateOf(st.personaField.ifEmpty { st.persona }) }
    var gid by remember(b.id) { mutableStateOf(st.google.gid) }
    var gsecret by remember(b.id) { mutableStateOf(st.google.gsecret) }
    var gpaste by remember(b.id) { mutableStateOf("") }
    var gown by remember(b.id) { mutableStateOf(false) }
    var purl by remember(b.id) { mutableStateOf(st.purl) }
    var join by remember(b.id) { mutableStateOf("") }
    var deleting by remember { mutableStateOf(false) }
    val uri = androidx.compose.ui.platform.LocalUriHandler.current
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionLabel("Persona")
        OutlinedTextField(persona, { persona = it; m.field("persona", it) }, Modifier.fillMaxWidth(), minLines = 4, maxLines = 14)
        OutlinedButton(onClick = { m.act("bot-edit") }, enabled = persona != st.persona, shape = square) { Text("Save") }

        SectionLabel("Google")
        val ctx = androidx.compose.ui.platform.LocalContext.current
        LaunchedEffect(st.google.url) { GoogleLoop.open(ctx, st.google.url) }
        if (st.google.status.isNotEmpty()) Text(st.google.status, style = MaterialTheme.typography.bodyMedium)
        st.google.accounts.forEach { a ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(a, Modifier.weight(1f), maxLines = 1)
                TextButton(onClick = { m.act("google", "signout:$a") }) { Text("Sign out", color = MaterialTheme.colorScheme.error) }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { GoogleLoop.start { a, v -> m.act(a, v) } }, shape = square) { Text(if (st.google.accounts.isEmpty()) "Connect Google" else "Add account") }
            TextButton(onClick = { gown = !gown }) { Text(if (gown) "Hide own client" else "Own client") }
        }
        if (gown) {
            Field(gid, "Client ID (Desktop app)", { gid = it; m.field("gid", it) })
            Field(gsecret, "Client secret", { gsecret = it; m.field("gsecret", it) }, secret = true)
            OutlinedButton(onClick = { m.act("google", "configure") }, shape = square) { Text("Save client") }
        }
        if (st.google.url.isNotEmpty()) {
            TextButton(onClick = { runCatching { uri.openUri(st.google.url) } }) { Text("Open Google sign-in") }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Field(gpaste, "Redirected URL", { gpaste = it; m.field("gpaste", it) }, Modifier.weight(1f))
                TextButton(onClick = { m.act("google", "finish") }) { Text("Finish") }
            }
        }

        SectionLabel("Machines")
        for (p in st.peers) Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(p.name)
                Text(p.url, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline, maxLines = 1,
                    overflow = TextOverflow.Ellipsis)
            }
            IconButton(onClick = { m.act("peer-revoke", p.id) }) { Icon(Icons.Filled.Close, "Unlink") }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Field(purl, "This hub's address", { purl = it; m.field("purl", it) }, Modifier.weight(1f))
            TextButton(onClick = { m.act("peer-invite") }) { Text("Invite") }
        }
        if (st.invite.isNotEmpty()) Row(verticalAlignment = Alignment.CenterVertically) {
            Text(st.invite, Modifier.weight(1f), fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
            IconButton(onClick = { m.act("copy", st.invite) }) { Icon(Icons.Filled.ContentCopy, "Copy invite") }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Field(join, "Paste an invite", { join = it; m.field("invite", it) }, Modifier.weight(1f))
            TextButton(onClick = { m.act("peer-join"); join = "" }) { Text("Join") }
        }

        HorizontalDivider(Modifier.padding(top = 16.dp))
        TextButton(onClick = { deleting = true }) { Text("Delete bot", color = MaterialTheme.colorScheme.error) }
    }
    if (deleting) Overlaid()
    if (deleting) AlertDialog(
        onDismissRequest = { deleting = false },
        title = { Text("Delete ${b.name}?") },
        confirmButton = {
            TextButton(onClick = { deleting = false; m.act("bot-delete", b.id) }) { Text("Delete", color = MaterialTheme.colorScheme.error) }
        },
        dismissButton = { TextButton(onClick = { deleting = false }) { Text("Cancel") } },
        shape = square,
    )
}

// a room, or the conversation with a bot on a linked machine: posts and a
// composer (field is its "bfield", send the action that posts it)
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TalkScreen(m: AppModel, b: BotView, field: String, send: String) {
    var text by remember(b.id) { mutableStateOf(b.draft) }
    var menu by remember { mutableStateOf(false) }
    val list = rememberLazyListState()
    LaunchedEffect(b.id, b.posts.size, m.scrolls) { if (b.posts.isNotEmpty()) list.animateScrollToItem(b.posts.size - 1) }
    Scaffold(
        topBar = {
            Column {
                TopAppBar(
                    title = {
                        Column {
                            Text(b.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            if (b.kind == "room" && b.members.isNotEmpty()) Text(b.members, style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.outline, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    },
                    navigationIcon = { IconButton(onClick = { m.back() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                    actions = {
                        if (b.kind == "room") Box {
                            IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, "More") }
                            DropdownMenu(menu, { menu = false }) {
                                DropdownMenuItem(text = { Text("Delete room", color = MaterialTheme.colorScheme.error) },
                                    onClick = { menu = false; m.act("room-delete", b.id) })
                            }
                        }
                    },
                )
                if (b.kind == "remote") BotHeader(m, b, 48.dp)
            }
        },
        bottomBar = {
            Surface(tonalElevation = 3.dp) {
                Row(Modifier.fillMaxWidth().navigationBarsPadding().imePadding().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(text, { text = it; m.field(field, it) }, Modifier.weight(1f), maxLines = 6,
                        placeholder = { Text(if (b.kind == "room") "Post to the room" else "Message") })
                    IconButton(onClick = { m.act(send); text = "" }, enabled = text.isNotBlank()) {
                        Icon(Icons.AutoMirrored.Filled.Send, "Send",
                            tint = if (text.isNotBlank()) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline)
                    }
                }
            }
        },
    ) { pad ->
        if (b.posts.isEmpty()) Box(Modifier.padding(pad)) { Empty("No messages") }
        LazyColumn(Modifier.fillMaxSize(), state = list, contentPadding = PaddingValues(
            start = 16.dp, end = 16.dp, top = pad.calculateTopPadding() + 8.dp, bottom = pad.calculateBottomPadding() + 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)) {
            items(b.posts, key = { it.id }) { p ->
                Column(Modifier.fillMaxWidth(), horizontalAlignment = if (p.mine) Alignment.End else Alignment.Start,
                    verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(p.from, style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.SemiBold)
                        Text(p.ago, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                    }
                    Surface(color = if (p.mine) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant,
                        shape = square, modifier = Modifier.widthIn(max = 320.dp)) {
                        Text(p.text, Modifier.padding(horizontal = 12.dp, vertical = 8.dp), style = MaterialTheme.typography.bodyLarge)
                    }
                }
            }
        }
    }
}
