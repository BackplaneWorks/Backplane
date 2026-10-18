package dev.backplane.mobile

import android.content.Intent
import android.net.Uri
import android.provider.Settings as AndroidSettings
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Forum
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Pets
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

// The tab bar (ios-snappy): Home, Threads, Bots, Routines, Settings. Each tab
// keeps its own scroll position (its list state lives here, above the
// switch); a page opens over the bar from the tab it belongs to and Back
// returns to that tab. The bar is a rail on a wide screen. Every list is an
// existing one moved, or a list over what the screen already carries.
private fun Tab.icon(): ImageVector = when (this) {
    Tab.Home -> Icons.Filled.Home
    Tab.Threads -> Icons.Filled.Forum
    Tab.Bots -> Icons.Filled.Pets
    Tab.Routines -> Icons.Filled.Schedule
    Tab.Settings -> Icons.Filled.Settings
}

private fun Screen.badge(t: Tab) = when (t) {
    Tab.Home -> tabs.home
    Tab.Bots -> tabs.bots
    Tab.Routines -> tabs.routines
    else -> 0
}

@Composable
private fun TabIcon(s: Screen, t: Tab) {
    val n = s.badge(t)
    if (n > 0) BadgedBox(badge = { Badge { Text("$n") } }) { Icon(t.icon(), t.title) } else Icon(t.icon(), t.title)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Tabs(m: AppModel, s: Screen, onPair: () -> Unit) {
    val states = Tab.entries.associateWith { rememberLazyListState() }
    var hubPage by rememberSaveable { mutableStateOf<String?>(null) }
    val wide = LocalConfiguration.current.screenWidthDp >= 840
    val tab = m.tab
    val page = hubPage
    if (page != null) {
        BackHandler { hubPage = null }
        HubSettingsPage(m, s, page) { hubPage = null }
    } else {
        val content: @Composable (androidx.compose.foundation.layout.PaddingValues) -> Unit = { pad ->
            Box(Modifier.fillMaxSize().padding(pad)) {
                when (tab) {
                    Tab.Home -> Scaffold(topBar = { TopAppBar(title = { Text("Home") }) }) { p ->
                        Box(Modifier.padding(p)) { HomeTab(m, s, states.getValue(Tab.Home)) }
                    }
                    Tab.Threads -> Projects(m, s, onPair, states.getValue(Tab.Threads))
                    Tab.Bots -> BotsPage(m, s, states.getValue(Tab.Bots))
                    Tab.Routines -> RoutinesPage(m, s, states.getValue(Tab.Routines))
                    Tab.Settings -> AppSettingsPage(m, s, onPair, states.getValue(Tab.Settings)) { hubPage = it }
                }
            }
        }
        if (wide) Row(Modifier.fillMaxSize()) {
            NavigationRail {
                for (t in Tab.entries) NavigationRailItem(selected = t == tab, onClick = { m.selectTab(t) }, icon = { TabIcon(s, t) }, label = { Text(t.title) })
            }
            Box(Modifier.weight(1f)) { content(androidx.compose.foundation.layout.PaddingValues(0.dp)) }
        } else Scaffold(
            contentWindowInsets = WindowInsets(0, 0, 0, 0),
            bottomBar = {
                NavigationBar {
                    for (t in Tab.entries) NavigationBarItem(selected = t == tab, onClick = { m.selectTab(t) }, icon = { TabIcon(s, t) }, label = { Text(t.title) })
                }
            },
        ) { pad -> content(pad) }
    }
    s.newBot?.let { NewBotDialog(m, it) }
    s.newRoom?.let { NewRoomDialog(m, it) }
}

// Bots
// ----

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun BotsPage(m: AppModel, s: Screen, state: LazyListState) {
    var part by rememberSaveable { mutableStateOf("bots") }
    var menu by remember { mutableStateOf(false) }
    Scaffold(topBar = {
        TopAppBar(title = { Text("Bots") }, actions = {
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
        })
    }) { pad ->
        LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = pad) {
            item(key = "seg") {
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(16.dp, 8.dp)) {
                    SegmentedButton(part == "bots", { part = "bots" }, SegmentedButtonDefaults.itemShape(0, 2)) { Text("Bots") }
                    SegmentedButton(part == "rooms", { part = "rooms" }, SegmentedButtonDefaults.itemShape(1, 2)) { Text("Rooms") }
                }
            }
            if (part == "bots") {
                if (s.bots.isEmpty()) item(key = "none") { Empty("No bots yet") }
                items(s.bots, key = { "b:" + it.id }) { b ->
                    ListItem(
                        headlineContent = { Text(b.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        supportingContent = {
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                MoodDot(b.mood)
                                Text(listOf(b.note, b.peer, b.machine).filter { it.isNotEmpty() }.joinToString(" · "), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        },
                        leadingContent = { BotCat(m, b.cat, 40.dp) },
                        modifier = Modifier.alpha(if (b.mood == "away") 0.5f else 1f)
                            .clickable { m.open(b.nav, b.act, b.id, b.name, b.cat, if (b.remote) "remote" else "bot") },
                    )
                }
            } else {
                if (s.rooms.isEmpty()) item(key = "none") { Empty("No rooms yet") }
                items(s.rooms, key = { "r:" + it.id }) { r ->
                    ListItem(
                        headlineContent = { Text(r.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        supportingContent = { Text(listOf("${r.members}", r.machine).filter { it.isNotEmpty() }.joinToString(" · ")) },
                        leadingContent = { Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) { Icon(Icons.Filled.Forum, null) } },
                        modifier = Modifier.clickable { m.open(r.nav, "room", r.id, r.name, kind = "room") },
                    )
                }
            }
        }
    }
}

@Composable
private fun Empty(text: String) {
    Box(Modifier.fillMaxWidth().padding(vertical = 48.dp), contentAlignment = Alignment.Center) {
        Text(text, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.outline)
    }
}

// Routines
// --------

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RoutinesPage(m: AppModel, s: Screen, state: LazyListState) {
    Scaffold(topBar = { TopAppBar(title = { Text("Routines") }) }) { pad ->
        LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = pad) {
            if (s.tabs.routineRows.isEmpty()) item(key = "none") { Empty("No routines") }
            items(s.tabs.routineRows, key = { "r:" + it.id }) { r ->
                val off = r.state == "off"
                ListItem(
                    headlineContent = { Text(r.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = {
                        Column {
                            Text(listOf(r.bot, r.machine).filter { it.isNotEmpty() }.joinToString(" · "), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(listOf(if (r.state == "done") r.doneText else r.sub, r.zone, if (r.ago.isEmpty()) "" else "last ${r.ago} ago").filter { it.isNotEmpty() }.joinToString(" · "), maxLines = 2)
                        }
                    },
                    leadingContent = { Icon(when (r.state) { "reminder" -> Icons.Filled.Notifications; "done" -> Icons.Filled.CheckCircle; else -> Icons.Filled.Schedule }, null) },
                    modifier = Modifier.alpha(if (off || r.state == "done" || r.away) 0.5f else 1f)
                        .clickable { m.open(r.nav, r.act, r.value, r.bot.ifEmpty { r.title }, kind = "routine") },
                )
            }
        }
    }
}

// Settings
// --------

// On this device, instant: nothing here waits on a hub. The hub's own
// settings are one row away, on a page that fills in as the hub answers.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AppSettingsPage(m: AppModel, s: Screen, onPair: () -> Unit, state: LazyListState, hub: (String) -> Unit) {
    val ctx = LocalContext.current
    var bytes by remember { mutableStateOf(m.cacheSize()) }
    var confirm by remember { mutableStateOf(false) }
    val list = AppSettings.threadList.ifEmpty { s.view }
    val allowed = Notes.allowed(ctx)
    // ask the hub for its settings in the background (a select, on a tab change, closes it)
    val askHub = s.hubs.firstOrNull { it.key == s.hub }?.key ?: s.hubs.firstOrNull()?.key
    LaunchedEffect(askHub) { if (askHub != null) m.act("hub-settings", "$askHub|on") }
    LaunchedEffect(Unit) { m.want("settings.lang") }
    val about = remember {
        runCatching { ctx.packageManager.getPackageInfo(ctx.packageName, 0).let { "${it.versionName} (${it.longVersionCode})" } }.getOrDefault("?")
    }
    Scaffold(topBar = { TopAppBar(title = { Text("Settings") }) }) { pad ->
        LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = pad) {
            item { Header("Appearance") }
            item { Choice(listOf("system" to "System", "light" to "Light", "dark" to "Dark"), AppSettings.appearance) { AppSettings.putAppearance(it) } }
            s.lang?.rows?.firstOrNull()?.let { row ->
                item { Text(m.t("settings.lang", "Language"), Modifier.padding(horizontal = 16.dp)) }
                item { Choice(row.buttons.map { it.value to it.label }, AppSettings.lang) { m.language(it) } }
            }
            s.lang?.rows?.drop(1)?.firstOrNull()?.let { row ->
                row.buttons.firstOrNull()?.let { button ->
                    item { ListItem(headlineContent = { Text(row.label) }, trailingContent = { Switch(button.on, { m.act(button.action, button.value) }) }) }
                }
            }
            item { Header("Behaviour") }
            item { Choice(listOf("projects" to "Projects", "active" to "Active"), list) { m.threadList(it) } }
            item {
                Text("Projects groups threads by project; Active lists what is going on, what needs you first.",
                    Modifier.padding(horizontal = 16.dp), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
            }
            item {
                var open by remember { mutableStateOf(false) }
                val name = Tab.entries.firstOrNull { it.name == AppSettings.defaultTab }?.title ?: "Automatic"
                Box {
                    ListItem(headlineContent = { Text("Opens on") }, supportingContent = { Text(name) }, modifier = Modifier.clickable { open = true })
                    DropdownMenu(open, { open = false }) {
                        DropdownMenuItem(text = { Text("Automatic") }, onClick = { open = false; AppSettings.putDefaultTab("") })
                        for (t in Tab.entries.filter { it != Tab.Settings }) DropdownMenuItem(text = { Text(t.title) }, onClick = { open = false; AppSettings.putDefaultTab(t.name) })
                    }
                }
            }
            item {
                ListItem(headlineContent = { Text("Haptics") }, trailingContent = { Switch(AppSettings.haptics, { AppSettings.putHaptics(it) }) })
            }
            item { Header("Notifications") }
            item {
                ListItem(headlineContent = { Text("Notifications") }, supportingContent = { Text(if (allowed) "On" else "Off") },
                    trailingContent = {
                        OutlinedButton(onClick = {
                            ctx.startActivity(Intent(AndroidSettings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(AndroidSettings.EXTRA_APP_PACKAGE, ctx.packageName).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                        }) { Text("System settings") }
                    })
            }
            // what the hub alerts for: its settings, from the last screen that carried them
            val nh = s.hubs.firstOrNull { it.key == s.hub } ?: s.hubs.firstOrNull()
            val nrows = nh?.let { h -> (if (s.hub == h.key) s.settings?.rows else null) ?: m.hubSettings[h.key]?.rows }
            if (nrows != null) hubSettingsRows(m, nrows, notify = true)
            if (s.hubs.size > 1 && nh != null) item {
                Text("Notify me for is a setting on ${nh.name}.", Modifier.padding(horizontal = 16.dp), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
            }
            item { Header("Hubs") }
            item { ListItem(headlineContent = { Text("Hubs") }, supportingContent = { Text("${s.hubs.size} paired") }, modifier = Modifier.clickable(onClick = onPair)) }
            items(s.hubs, key = { "hs:" + it.key }) { h ->
                ListItem(headlineContent = { Text(if (s.hubs.size > 1) "Hub settings · ${h.name}" else "Hub settings") }, modifier = Modifier.clickable { hub(h.key) })
            }
            item { Header("Cache") }
            item {
                ListItem(headlineContent = { Text("Kept state") }, supportingContent = { Text(android.text.format.Formatter.formatShortFileSize(ctx, bytes)) },
                    trailingContent = { OutlinedButton(onClick = { confirm = true }) { Text("Clear cached state") } })
            }
            item { Header("Tour") }
            item { ListItem(headlineContent = { Text("Show the tour") }, modifier = Modifier.clickable { m.act("ob-open") }) }
            item { Header("About") }
            item { ListItem(headlineContent = { Text("Backplane") }, supportingContent = { Text(about) }) }
        }
    }
    if (confirm) AlertDialog(
        onDismissRequest = { confirm = false },
        title = { Text("Clear the cached state?") },
        text = { Text("The next launch fetches each hub's history again.") },
        confirmButton = { TextButton(onClick = { confirm = false; m.clearCache(); bytes = 0 }) { Text("Clear", color = MaterialTheme.colorScheme.error) } },
        dismissButton = { TextButton(onClick = { confirm = false }) { Text("Cancel") } },
    )
}

@Composable
private fun Header(text: String) {
    Text(text, Modifier.padding(start = 16.dp, top = 20.dp, bottom = 6.dp), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
}

@Composable
private fun Choice(options: List<Pair<String, String>>, current: String, pick: (String) -> Unit) {
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
        options.forEachIndexed { i, (v, label) ->
            SegmentedButton(current == v, { pick(v) }, SegmentedButtonDefaults.itemShape(i, options.size)) { Text(label) }
        }
    }
}

// The hub's settings (the rows Bend sends for Settings), on a page of their
// own: it shows at once with the rows last seen (kept on disk), asks the hub
// in the background and takes the fresh rows in place; with none yet, the
// page's shape as placeholders. Leaving it closes the hub's Settings.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HubSettingsPage(m: AppModel, s: Screen, key: String, back: () -> Unit) {
    DisposableEffect(key) {
        m.act("hub-settings", "$key|on")
        onDispose { m.act("hub-settings", "$key|off") }
    }
    val rows = (if (s.hub == key) s.settings?.rows else null) ?: m.hubSettings[key]?.rows
    val name = s.hubs.firstOrNull { it.key == key }?.name ?: key
    Scaffold(topBar = {
        TopAppBar(title = { Text("Hub settings · $name", maxLines = 1, overflow = TextOverflow.Ellipsis) }, navigationIcon = {
            IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
        })
    }) { pad ->
        LazyColumn(Modifier.fillMaxSize(), contentPadding = pad) {
            if (rows != null) hubSettingsRows(m, rows)
            else items(7) {
                Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    val ph = MaterialTheme.colorScheme.surfaceVariant
                    Box(Modifier.fillMaxWidth(0.4f).height(16.dp).background(ph))
                    Box(Modifier.fillMaxWidth().height(12.dp).background(ph))
                }
                HorizontalDivider()
            }
        }
    }
}
