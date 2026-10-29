package dev.backplane.mobile

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.border
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.height
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
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.CreateNewFolder
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Layers
import androidx.compose.material.icons.filled.DarkMode
import androidx.compose.material.icons.filled.LightMode
import androidx.compose.material.icons.filled.Memory
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.IconButton
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilterChip
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.foundation.content.MediaType
import androidx.compose.foundation.content.ReceiveContentListener
import androidx.compose.foundation.content.consume
import androidx.compose.foundation.content.contentReceiver
import androidx.compose.foundation.content.hasMediaType
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.graphics.SolidColor
import kotlinx.coroutines.launch
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.statusBars
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.material.icons.filled.AddCircleOutline
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.Folder
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.material.icons.automirrored.filled.Chat
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.SkipPrevious
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.clearAndSetSemantics
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.first

// A cached destination has no input until its matching screen arrives.
// The Back control sits outside the inert content, so opening is cancellable.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun OpeningDestination(m: AppModel, s: Screen, o: Core.Opening) {
    val lang = s.locale
    val page = m.seenPages[o.nav]
    Scaffold(topBar = {
        TopAppBar(title = { Text(o.title) }, navigationIcon = {
            IconButton(onClick = { m.back() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, NativeMessages.tour_back(lang)) }
        })
    }) { padding ->
        Box(Modifier.fillMaxSize().padding(padding).clearAndSetSemantics { }) {
            val b = m.seenBots[o.nav]
            val t = m.seenThreads[o.nav]
            if (b != null && page != null) BotDestination(m, page, b)
            else if (t != null) ThreadScreen(m, page ?: s.copy(thread = t, bot = null), t)
            else Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (o.cat.isNotEmpty()) BotCat(m, o.cat, 48.dp)
                    Text(o.title, style = MaterialTheme.typography.titleLarge)
                }
                repeat(6) {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Box(Modifier.fillMaxWidth(0.65f).height(18.dp).background(MaterialTheme.colorScheme.surfaceVariant))
                        Box(Modifier.fillMaxWidth().height(12.dp).background(MaterialTheme.colorScheme.surfaceVariant))
                    }
                }
            }
            // Covers cached fields, buttons and scroll gestures, not Back.
            Box(Modifier.fillMaxSize().pointerInput(o.nav) {
                awaitPointerEventScope {
                    while (true) { awaitPointerEvent(PointerEventPass.Initial).changes.forEach { it.consume() } }
                }
            })
        }
    }
}

@Composable
fun App(m: AppModel) {
    // the banner takes the status bar's room: the screen under it then
    // leaves none of its own (no gap between the two)
    var bannerUp by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize()) {
        Box(Modifier.onSizeChanged { bannerUp = it.height > 0 }) { ViewStartupBanner(m) }
        Box(Modifier.weight(1f).then(if (bannerUp) Modifier.consumeWindowInsets(WindowInsets.statusBars) else Modifier)) {
            AppContent(m)
            BubMoveOverlay(if (m.contentReady) m.screen?.bubMove else null)
        }
    }
}

@Composable
private fun AppContent(m: AppModel) {
    var pairing by rememberSaveable { mutableStateOf(false) }
    val s = m.screen
    // New Thread's page, left with Back before the hub made the thread
    var leftNew by remember { mutableStateOf(false) }
    LaunchedEffect(s?.making) { if (s?.making != true) leftNew = false }
    // a file link tapped in a message asks for its menu
    CompositionLocalProvider(LocalBackplaneLanguage provides (s?.locale ?: "en"), LocalFileTap provides { p: String -> m.act("file-menu", p) }) { when {
        m.links.isEmpty() -> Pair(s?.locale ?: "en", "", cancel = null) { m.pair(it) }
        s == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        pairing -> {
            BackHandler { pairing = false }
            Hubs(m, s) { pairing = false }
        }
        // New Thread: its page at once, waiting for the hub's thread
        s.making && !leftNew -> {
            BackHandler { leftNew = true; m.back() }
            MakingScreen { leftNew = true; m.back() }
        }
        m.leaving -> Tabs(m, s, onPair = { pairing = true })
        m.opening != null -> {
            BackHandler { m.back() }
            OpeningDestination(m, s, m.opening!!)
        }
        s.thread != null && s.thread.viewer.open == "files" && s.thread.viewer.files != null -> {
            BackHandler { m.act("view", "") }
            FilesScreen(m, s.thread.viewer, s.thread.viewer.files)
        }
        s.thread != null && (s.thread.viewer.open == "image" || s.thread.viewer.open == "pdf") && s.thread.viewer.file != null -> {
            BackHandler { m.act("fm") }
            FileScreen(m, s.thread.viewer, s.thread.viewer.file)
        }
        s.thread != null && s.thread.viewer.open == "review" && s.thread.viewer.review != null -> {
            BackHandler { m.act("view", "") }
            ReviewScreen(m, s.thread.viewer, s.thread.viewer.review)
        }
        s.thread != null && s.thread.viewer.open == "renders" && s.thread.viewer.mech != null -> {
            BackHandler { m.act("view", "") }
            MechScreen(m, s.thread.viewer, s.thread.viewer.mech)
        }
        s.thread != null && s.thread.viewer.open.isNotEmpty() -> {
            BackHandler { m.act("view", "") }
            PlotScreen(m, s.thread.viewer)
        }
        // the thread's shell, in place of the screen so it follows the keyboard
        s.thread?.term != null -> {
            BackHandler { m.act("term-toggle") }
            TermSheet(m, s.thread.term)
        }
        // a bot (its chat is its thread), a room, or a bot on a linked machine
        s.bot != null -> {
            BackHandler { m.back() }
            BotDestination(m, s, s.bot)
        }
        s.thread != null -> {
            BackHandler { m.back() }
            ThreadScreen(m, s, s.thread)
        }
        else -> Tabs(m, s, onPair = { pairing = true })
    } }
    if (s == null || m.links.isEmpty()) return
    val d = s.deleting
    // the delete just answered: its dialog stays down until the screen drops it
    var answered by remember(d?.id) { mutableStateOf(false) }
    if (d != null && !answered) AlertDialog(
        onDismissRequest = { answered = true; m.act("delete-no") },
        title = { Text(d.title) },
        text = { Text(d.body) },
        confirmButton = {
            TextButton(onClick = { answered = true; m.act("row-delete", d.id) }) {
                Text(d.yes, color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = { TextButton(onClick = { answered = true; m.act("delete-no") }) { Text(d.no) } },
    )
    val rn = s.renaming
    // a rename: the title edited here, sent whole with the thread's id
    var renamed by remember(rn?.id) { mutableStateOf(false) }
    var title by remember(rn?.id) { mutableStateOf(rn?.text ?: "") }
    if (rn != null && !renamed) AlertDialog(
        onDismissRequest = { renamed = true; m.act("rename-no") },
        title = { Text(rn.title) },
        text = {
            OutlinedTextField(title, { title = it }, Modifier.fillMaxWidth(), singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences))
        },
        confirmButton = {
            TextButton(onClick = { renamed = true; m.act("rename-save", rn.id + "\u001f" + title) }, enabled = title.isNotBlank()) { Text(rn.yes) }
        },
        dismissButton = { TextButton(onClick = { renamed = true; m.act("rename-no") }) { Text(rn.no) } },
    )
    val r = s.removing
    // likewise the remove just answered
    var removed by remember(r?.id) { mutableStateOf(false) }
    if (r != null && !removed) AlertDialog(
        onDismissRequest = { removed = true; m.act("proj-keep") },
        title = { Text(r.title) },
        text = { Text(r.body) },
        confirmButton = {
            TextButton(onClick = { removed = true; m.act("proj-remove", r.id) }) {
                Text(r.yes, color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = { TextButton(onClick = { removed = true; m.act("proj-keep") }) { Text(r.no) } },
    )
    // the first-run tour (core/onboard.bend): the same as iOS, dismissing it
    // (Back, a tap outside) is Skip. The screen model has no step while Bend
    // knows of another overlay; the app adds the hubs screen and every
    // dialog it shows itself (Overlay: a picture, a diff, a confirmation)
    s.tour?.let { t ->
        val free = !pairing && Overlay.n == 0
        if (free) AlertDialog(
            onDismissRequest = { m.act("ob-skip") },
            title = {
                Column {
                    Text(t.counter, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(t.title)
                }
            },
            text = {
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    Text(t.body)
                    for (c in t.choices) OutlinedButton(onClick = { m.act(c.action, c.value) }, Modifier.fillMaxWidth().padding(top = 8.dp)) { Text(c.label) }
                }
            },
            confirmButton = { TextButton(onClick = { m.act("ob-next") }) { Text(t.next) } },
            dismissButton = {
                Row {
                    TextButton(onClick = { m.act("ob-skip") }) { Text(t.skip) }
                    if (!t.first) TextButton(onClick = { m.act("ob-back") }) { Text(t.back) }
                }
            },
        )
    }
    s.find?.let { FindSheet(m, it) }
    // a row's, a project's or (in a thread) a file link's menu
    s.rowMenu?.let { RowMenuSheet(m, it) }
}

// the list's search field, always shown, never focused on its own:
// typing filters the list ("proj-find-q"), Go goes to the first project
// shown ("proj-go"), the x empties it
@Composable
private fun SearchField(m: AppModel, f: Search, first: String?) {
    val lang = LocalBackplaneLanguage.current
    var text by remember { mutableStateOf(f.query) }
    // what was typed here: a screen still echoing it never overwrites the field
    val typed = remember { mutableSetOf<String>() }
    LaunchedEffect(f.query) {
        if (f.query !in typed) { typed.clear(); text = f.query }
    }
    OutlinedTextField(text, { t -> text = t; if (t != f.query) { typed.add(t); m.act("proj-find-q", t) } },
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        singleLine = true, placeholder = { Text(f.hint) },
        leadingIcon = { Icon(Icons.Filled.Search, null) },
        trailingIcon = {
            if (text.isNotEmpty()) IconButton(onClick = { text = ""; typed.clear(); m.act("proj-find-q", "") }) {
                Icon(Icons.Filled.Close, NativeMessages.native_thread_search_clear(lang))
            }
        },
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false, imeAction = ImeAction.Go),
        keyboardActions = KeyboardActions(onGo = { first?.let { m.act("proj-go", it) } }))
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Pair(tag: String, link: String, cancel: (() -> Unit)?, done: (String) -> Unit) {
    var text by rememberSaveable { mutableStateOf(link) }
    Scaffold(topBar = {
        TopAppBar(title = { Text(NativeMessages.native_pair_title(tag)) }, navigationIcon = {
            if (cancel != null) IconButton(onClick = cancel) { Icon(Icons.AutoMirrored.Filled.ArrowBack, NativeMessages.tour_back(tag)) }
        })
    }) { pad ->
        Column(Modifier.padding(pad).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            val ctx = LocalContext.current
            Button(onClick = { scanPairing(ctx, tag, done) }) {
                Icon(Icons.Filled.QrCodeScanner, null)
                Text(NativeMessages.native_pair_scan(tag), Modifier.padding(start = 8.dp))
            }
            Text(NativeMessages.native_pair_computer_android(tag),
                style = MaterialTheme.typography.bodyMedium)
            OutlinedTextField(text, { text = it }, Modifier.fillMaxWidth(), singleLine = true,
                label = { Text(NativeMessages.settings_pair(tag)) }, placeholder = { Text("http://host:3787/#token=…") })
            Button(onClick = { done(text) }, enabled = text.isNotBlank()) { Text(NativeMessages.native_pair_connect(tag)) }
        }
    }
}

// the hubs this phone is paired with (the cross unpairs), the owner's
// other machines they know of (one tap pairs), and a field for a new link
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Hubs(m: AppModel, s: Screen, back: () -> Unit) {
    val tag = s.locale
    var text by rememberSaveable { mutableStateOf("") }
    Scaffold(topBar = {
        TopAppBar(title = { Text(NativeMessages.native_settings_hubs(tag)) }, navigationIcon = {
            IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, NativeMessages.tour_back(tag)) }
        })
    }) { pad ->
        LazyColumn(Modifier.fillMaxSize(), contentPadding = pad) {
            items(s.hubs, key = { "h:" + it.key }) { h ->
                ListItem(
                    headlineContent = { Text(h.name) },
                    supportingContent = { Text(h.key) },
                    leadingContent = { Status(if (h.online) "idle" else "stop") },
                    trailingContent = { IconButton(onClick = { m.unpair(h.key) }) { Icon(Icons.Filled.Close, NativeMessages.native_pair_unpair(tag)) } },
                )
            }
            if (s.found.isNotEmpty()) {
                item(key = "found") {
                    Text(NativeMessages.native_pair_tailnet(tag), Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                        style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.outline)
                }
                items(s.found, key = { "f:" + it.url }) { f ->
                    ListItem(
                        headlineContent = { Text(f.name) },
                        supportingContent = { Text(f.url) },
                        trailingContent = { IconButton(onClick = { m.pair(f.url) }) { Icon(Icons.Filled.Add, NativeMessages.native_pair_action(tag)) } },
                    )
                }
            }
            item(key = "add") {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(NativeMessages.native_pair_instructions(tag),
                        style = MaterialTheme.typography.bodyMedium)
                    OutlinedTextField(text, { text = it }, Modifier.fillMaxWidth(), singleLine = true,
                        label = { Text(NativeMessages.settings_pair(tag)) }, placeholder = { Text("http://host:3787/#token=…") })
                    val ctx = LocalContext.current
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = { m.pair(text); text = "" }, enabled = text.isNotBlank()) { Text(NativeMessages.native_pair_add(tag)) }
                        OutlinedButton(onClick = { scanPairing(ctx, tag) { m.pair(it) } }) {
                            Icon(Icons.Filled.QrCodeScanner, null)
                            Text(NativeMessages.native_pair_scan(tag), Modifier.padding(start = 8.dp))
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun Errors(m: AppModel, s: Screen, host: SnackbarHostState) {
    val lang = s.locale
    LaunchedEffect(s.error, lang) {
        if (s.error.isNotEmpty()) {
            val r = host.showSnackbar(s.error, actionLabel = NativeMessages.approval_dismiss(lang), withDismissAction = false)
            if (r == SnackbarResult.ActionPerformed || r == SnackbarResult.Dismissed) m.act("dismiss")
        }
    }
}

@Composable
private fun Status(state: String) {
    Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
        when (state) {
            "run" -> CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
            else -> Box(Modifier.size(8.dp).clip(CircleShape).background(
                when (state) {
                    "fail" -> MaterialTheme.colorScheme.error
                    "stop" -> MaterialTheme.colorScheme.tertiary
                    else -> MaterialTheme.colorScheme.outlineVariant
                }))
        }
    }
}

@Composable
private fun swipeColor(s: Swipe?): Color = when (s?.tone) {
    "danger" -> MaterialTheme.colorScheme.errorContainer
    "settle" -> MaterialTheme.colorScheme.primaryContainer
    else -> MaterialTheme.colorScheme.secondaryContainer
}

// Swiping from the start sends the row's lead action; from the end it
// opens the trail's (delete, snooze and its choices). The row always
// springs back: what happens to the thread comes with the next screen.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ThreadRow(m: AppModel, r: Row, hierarchy: Boolean = true) {
    val lang = LocalBackplaneLanguage.current
    var menu by remember { mutableStateOf(false) }
    val haptic = LocalHapticFeedback.current
    val lead = r.lead.firstOrNull()
    val state = rememberSwipeToDismissBoxState(confirmValueChange = {
        when (it) {
            SwipeToDismissBoxValue.StartToEnd -> lead?.let { l -> m.act(l.action, l.value) }
            SwipeToDismissBoxValue.EndToStart -> menu = true
            SwipeToDismissBoxValue.Settled -> {}
        }
        false
    })
    Box {
        SwipeToDismissBox(
            state,
            enableDismissFromStartToEnd = lead != null,
            enableDismissFromEndToStart = r.trail.isNotEmpty(),
            backgroundContent = {
                val start = state.dismissDirection == SwipeToDismissBoxValue.StartToEnd
                Box(Modifier.fillMaxSize().background(swipeColor(if (start) lead else r.trail.firstOrNull()))
                    .padding(horizontal = 24.dp), contentAlignment = if (start) Alignment.CenterStart else Alignment.CenterEnd) {
                    Text(if (start) lead?.label ?: "" else r.trail.joinToString(" · ") { it.label },
                        style = MaterialTheme.typography.labelLarge)
                }
            },
        ) {
            Column(
                Modifier.fillMaxWidth().padding(start = (r.depth.coerceIn(0, 4) * 12).dp)
                    .alpha(if (r.faded) 0.45f else 1f),
            ) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    // This button is a sibling of the selectable row, so its
                    // click cannot select/open the thread or invoke its menu.
                    if (hierarchy && (r.children > 0 || r.canChild)) IconButton(
                        onClick = { m.act("thread-tree", r.id) },
                        modifier = Modifier.size(40.dp),
                    ) {
                        Icon(if (r.expanded) Icons.Filled.ExpandMore else Icons.Filled.ChevronRight,
                            (if (r.expanded) "Collapse subthreads of " else "Expand subthreads of ") + r.title)
                    } else Spacer(Modifier.width(40.dp))
                    ListItem(
                        headlineContent = { Text(r.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        supportingContent = if (r.project.isEmpty()) null else {
                            { Text(r.project, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                        },
                        leadingContent = { StatusDot(r.state, r.status) },
                        trailingContent = {
                            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.widthIn(max = 88.dp)) {
                                if (r.pinned) Icon(Icons.Filled.PushPin, NativeMessages.tools_pinned(lang), Modifier.size(16.dp).padding(end = 4.dp))
                                if (r.agents.isNotEmpty()) Text(r.agents, style = MaterialTheme.typography.labelMedium, color = PhaseColor.accent,
                                    maxLines = 1, overflow = TextOverflow.Ellipsis)
                                else Text(r.ago, style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        },
                        // a long press asks for the row's menu (the screen's rowMenu)
                        modifier = Modifier.weight(1f).combinedClickableCompat(onLong = {
                            if (AppSettings.haptics) haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                            m.act("row-menu", r.id)
                        }) { m.open(r.id, "select", r.id, r.title) },
                    )
                }
                if (hierarchy && r.expanded && r.canChild) TextButton(
                    onClick = { m.act("thread-child", r.id) },
                    modifier = Modifier.padding(start = 56.dp),
                ) {
                    Icon(Icons.Filled.Add, null, Modifier.size(16.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("New subthread", style = MaterialTheme.typography.labelMedium)
                }
            }
        }
        Box(Modifier.align(Alignment.TopEnd)) {
            DropdownMenu(menu, { menu = false }) {
                for (t in r.trail) {
                    if (t.options.isEmpty()) DropdownMenuItem(
                        text = { Text(t.label, color = if (t.tone == "danger") MaterialTheme.colorScheme.error else Color.Unspecified) },
                        onClick = { menu = false; m.act(t.action, t.value) },
                    ) else {
                        HorizontalDivider()
                        Text(t.label, Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                            style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline)
                        for (o in t.options) DropdownMenuItem(text = { Text(o.label) }, onClick = { menu = false; m.act(t.action, o.value) })
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
internal fun Modifier.combinedClickableCompat(onLong: (() -> Unit)? = null, onClick: () -> Unit) =
    this.combinedClickable(onClick = onClick, onLongClick = onLong)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun Projects(m: AppModel, s: Screen, onPair: () -> Unit, state: LazyListState = rememberLazyListState()) {
    val lang = s.locale
    val snacks = remember { SnackbarHostState() }
    var choosing by remember { mutableStateOf(false) }
    var more by remember { mutableStateOf(false) }
    Errors(m, s, snacks)
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(NativeMessages.tour_title_threads(lang))
                        val online = s.online && m.linkUp(s.hub)
                        Text(NativeMessages.native_thread_online(lang, if (online) "online" else "connecting"),
                            style = MaterialTheme.typography.labelMedium,
                            color = if (online) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline)
                    }
                },
                actions = {
                    // with several hubs, the picker opens on the one chosen
                    Box {
                        IconButton(onClick = { if (s.hubs.size > 1) choosing = true else m.act("picker-open") }) {
                            Icon(Icons.Filled.CreateNewFolder, NativeMessages.sidebar_add_project(lang))
                        }
                        DropdownMenu(choosing, { choosing = false }) {
                            for (h in s.hubs) DropdownMenuItem(text = { Text(h.name) }, onClick = {
                                choosing = false
                                m.act("picker-open", h.key + "|")
                            })
                        }
                    }
                    IconButton(onClick = { m.selectTab(Tab.Settings) }) { Icon(Icons.Filled.Settings, NativeMessages.palette_settings(lang)) }
                    IconButton(onClick = onPair) { Icon(Icons.Filled.Link, NativeMessages.native_settings_hubs(lang)) }
                    Box {
                        IconButton(onClick = { more = true }) { Icon(Icons.Filled.MoreVert, NativeMessages.common_more(lang)) }
                        DropdownMenu(more, { more = false }) {
                            DropdownMenuItem(text = { Text(NativeMessages.palette_search(lang)) }, leadingIcon = { Icon(Icons.Filled.Search, null) },
                                onClick = { more = false; m.act("find-open", "search") })
                        }
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snacks) },
    ) { pad ->
        if (s.projects.isEmpty() && s.legacy.isEmpty() && s.active.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(pad), contentAlignment = Alignment.Center) {
                Text(s.empty, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.outline)
            }
        }
        LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = pad) {
            // the search is always the list's first row (projects, or in the
            // active view threads by title or project)
            s.search?.let { f -> item(key = "find") { SearchField(m, f, s.projects.firstOrNull()?.id) } }
            if (s.view == "active") {
                if (s.active.isEmpty() && s.settled == null) {
                    if (s.projects.isNotEmpty() && s.search?.query.isNullOrEmpty()) item(key = "none") {
                        Box(Modifier.fillMaxWidth().padding(vertical = 48.dp), contentAlignment = Alignment.Center) {
                            Text(NativeMessages.sidebar_nothing(lang), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.outline)
                        }
                    }
                } else {
                    // live rows; the settled ones follow their fold while it is open
                    items(s.active.filter { !it.faded }, key = { "t:" + it.id }) { ThreadRow(m, it) }
                    item(key = "d:active") { HorizontalDivider() }
                    s.settled?.let { o ->
                        item(key = "settled") {
                            ListItem(
                                headlineContent = { Text(NativeMessages.sidebar_settled_count(lang, o.count.toString()), style = MaterialTheme.typography.labelLarge) },
                                trailingContent = { Icon(if (o.open) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore, null) },
                                modifier = Modifier.combinedClickableCompat { m.act("side-settled", "") },
                            )
                        }
                    }
                    items(s.active.filter { it.faded }, key = { "t:" + it.id }) { ThreadRow(m, it) }
                }
            } else {
                for (p in s.projects) {
                    item(key = "p:" + p.id) {
                        // a long press asks for the project's menu (the screen's rowMenu)
                        val haptic = LocalHapticFeedback.current
                        ListItem(
                            headlineContent = { Text(p.title, style = MaterialTheme.typography.titleMedium) },
                            supportingContent = {
                                Text(if (p.machine.isEmpty()) p.root else p.machine + ": " + p.root,
                                    maxLines = 1, overflow = TextOverflow.Ellipsis)
                            },
                            trailingContent = {
                                IconButton(onClick = { m.act("new-thread", p.id) }) { Icon(Icons.Filled.Add, NativeMessages.thread_new(lang)) }
                            },
                            modifier = Modifier.combinedClickableCompat(onLong = {
                                if (AppSettings.haptics) haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                m.act("proj-menu", p.id)
                            }) {},
                            // quiet (no live thread): faded, as on iOS
                            colors = if (p.quiet) ListItemDefaults.colors(
                                headlineColor = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.5f),
                                supportingColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
                            ) else ListItemDefaults.colors(),
                        )
                    }
                    items(p.threads, key = { "t:" + it.id }) { ThreadRow(m, it) }
                    if (p.snoozed.isNotEmpty()) {
                        item(key = "z:" + p.id) {
                            ListItem(headlineContent = { Text(p.snoozedShelf, style = MaterialTheme.typography.labelLarge) })
                        }
                        items(p.snoozed, key = { "t:" + it.id }) { ThreadRow(m, it) }
                    }
                    if (p.settled.isNotEmpty()) {
                        item(key = "s:" + p.id) {
                            ListItem(
                                headlineContent = { Text(p.shelf, style = MaterialTheme.typography.labelLarge) },
                                trailingContent = {
                                    Icon(if (p.open) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore, null)
                                },
                                modifier = Modifier.combinedClickableCompat { m.act("toggle-settled", p.id) },
                            )
                        }
                        if (p.open) items(p.settled, key = { "t:" + it.id }) { ThreadRow(m, it) }
                    }
                    if (p.archived.isNotEmpty() && p.value.isNotEmpty()) {
                        item(key = "a:" + p.id) {
                            ListItem(
                                headlineContent = { Text(NativeMessages.native_thread_archived(lang, p.archived.size.toString()), style = MaterialTheme.typography.labelLarge) },
                                trailingContent = {
                                    Icon(if (p.archOpen) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore, null)
                                },
                                modifier = Modifier.combinedClickableCompat { m.act("toggle-settled", p.value) },
                            )
                        }
                        if (p.archOpen) items(p.archived, key = { "t:" + it.id }) { ThreadRow(m, it) }
                    }
                    item(key = "d:" + p.id) { HorizontalDivider() }
                }
                // the folded shelf of older projects, at the end
                s.older?.let { o ->
                    item(key = "older") {
                        ListItem(
                            headlineContent = { Text(NativeMessages.sidebar_older(lang, o.count.toString()), style = MaterialTheme.typography.labelLarge) },
                            trailingContent = { Icon(if (o.open) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore, null) },
                            modifier = Modifier.combinedClickableCompat { m.act("side-older", "") },
                        )
                    }
                    item(key = "d:older") { HorizontalDivider() }
                }
            }
            if (s.legacy.isNotEmpty()) {
                item(key = "legacy:header") {
                    ListItem(headlineContent = { Text("Conversation history", style = MaterialTheme.typography.labelLarge) })
                }
                items(s.legacy, key = { "legacy:" + it.id }) { ThreadRow(m, it, hierarchy = false) }
            }
        }
    }
    s.folders?.let { FolderPicker(m, it) }
}

// a long-pressed row's or project's menu, or a tapped file link's: an item sends its action (Delete
// and Remove then ask through the deleting/removing dialogs); let go
// without a choice, the menu is closed ("menu-close")
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RowMenuSheet(m: AppModel, menu: RowMenu) {
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = { m.act("menu-close", "") }, sheetState = state) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(bottom = 8.dp)) {
            Text(menu.title, Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
                style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.outline,
                maxLines = 2, overflow = TextOverflow.Ellipsis)
            for (i in menu.items) ListItem(
                headlineContent = {
                    Text(i.label, color = if (i.tone == "danger") MaterialTheme.colorScheme.error else Color.Unspecified)
                },
                modifier = Modifier.combinedClickableCompat { m.act(i.action, i.value) },
                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
            )
        }
    }
}

// the project picker: a path field over the listed folder's rows
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun FolderPicker(m: AppModel, p: Folders) {
    val lang = LocalBackplaneLanguage.current
    var text by remember { mutableStateOf(p.text) }
    // what was typed here: a screen still echoing it never overwrites the field
    val typed = remember { mutableSetOf<String>() }
    LaunchedEffect(p.text) {
        if (p.text !in typed) { typed.clear(); text = p.text }
    }
    Dialog(onDismissRequest = { m.act("proj-close") }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Scaffold(
            topBar = {
                TopAppBar(
                    title = { Text(NativeMessages.sidebar_add_project(lang)) },
                    navigationIcon = { IconButton(onClick = { m.act("proj-close") }) { Icon(Icons.Filled.Close, NativeMessages.common_cancel(lang)) } },
                )
            },
        ) { pad ->
            Column(Modifier.fillMaxSize().padding(pad).imePadding()) {
                OutlinedTextField(text, { t ->
                    text = t
                    typed.add(t)
                    m.act("picker-type", t)
                }, Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), singleLine = true,
                    placeholder = { Text(p.hint) }, textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false))
                if (p.error.isNotEmpty()) Text(p.error, Modifier.padding(horizontal = 16.dp),
                    color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                LazyColumn(Modifier.fillMaxSize()) {
                    items(p.items) { r ->
                        val icon = when (r.kind) {
                            "up" -> Icons.Filled.ArrowUpward
                            "add" -> Icons.Filled.AddCircleOutline
                            "new", "mkdir" -> Icons.Filled.CreateNewFolder
                            "off" -> Icons.Filled.Check
                            "machine" -> Icons.Filled.Computer
                            else -> Icons.Filled.Folder
                        }
                        val tone = if (r.kind == "dir" || r.kind == "up") MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.primary
                        ListItem(
                            headlineContent = { Text(r.label, maxLines = 1, overflow = TextOverflow.Ellipsis, color = tone) },
                            leadingContent = { Icon(icon, null, tint = tone) },
                            modifier = (if (r.action.isEmpty()) Modifier.alpha(0.5f) else Modifier.combinedClickableCompat { m.act(r.action, r.value) }),
                        )
                    }
                }
            }
        }
    }
}

// a new thread's page while the hub makes it
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MakingScreen(back: () -> Unit) {
    val lang = LocalBackplaneLanguage.current
    Scaffold(topBar = {
        TopAppBar(
            navigationIcon = { IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, NativeMessages.tour_back(lang)) } },
            title = { Text(NativeMessages.thread_new(lang)) },
        )
    }) { pad ->
        Box(Modifier.fillMaxSize().padding(pad), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
    }
}

// a message the hub has not stored yet
@Composable
private fun SendingView(text: String) {
    val lang = LocalBackplaneLanguage.current
    Column(Modifier.fillMaxWidth().alpha(0.55f), horizontalAlignment = Alignment.End,
        verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(Modifier.fillMaxWidth()) {
            Spacer(Modifier.weight(1f).widthIn(min = 48.dp))
            Surface(color = MaterialTheme.colorScheme.primaryContainer, shape = MaterialTheme.shapes.large) {
                Text(text, Modifier.padding(horizontal = 14.dp, vertical = 10.dp), style = MaterialTheme.typography.bodyLarge)
            }
        }
        Text(NativeMessages.native_thread_sending(lang), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
    }
}

// below: more under the top bar (a bot's cat and tabs)
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ThreadScreen(m: AppModel, s: Screen, t: ThreadView, below: (@Composable () -> Unit)? = null) {
    val lang = s.locale
    val snacks = remember { SnackbarHostState() }
    val list = rememberLazyListState()
    val userDragging by list.interactionSource.collectIsDraggedAsState()
    var scrollGesture by remember(t.id) { mutableStateOf(false) }
    var readingDirection by remember(t.id) { mutableStateOf(0) }
    var acceptedPage by remember(t.id) { mutableStateOf("") }
    LaunchedEffect(userDragging) {
        if (userDragging) { scrollGesture = true; acceptedPage = "" }
    }
    var menu by remember { mutableStateOf(false) }
    // the subagents sheet (the top bar's button) is up
    var subs by remember { mutableStateOf(false) }
    // the image open in the lightbox
    var shown by remember { mutableStateOf<String?>(null) }
    val termSize = rememberTermSize()
    val su = t.subs
    if (subs && su != null) {
        // opened, its rows show (folded once all are done)
        LaunchedEffect(Unit) { if (!su.open && su.act.isNotEmpty()) m.act(su.act, su.value) }
        ModalBottomSheet(onDismissRequest = { subs = false }, sheetState = rememberModalBottomSheetState()) {
            Box(Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp)) { SubsView(m, su) }
        }
    }
    Errors(m, s, snacks)
    val count = (if (t.parent != null) 1 else 0) +
        (if (t.earlier > 0) 1 else 0) + t.entries.size + t.sending.size + (if (t.live.isNotEmpty() || t.working.isNotEmpty()) 1 else 0) +
        (if (t.later > 0) 1 else 0)
    LaunchedEffect(t.id, m.scrolls) { if (count > 0) list.scrollToItem(count - 1) }
    // LazyColumn retains the first visible entry key AND its pixel offset.
    // An explicit scrollToItem after a prepend discards that offset and
    // cancels the ongoing fling, so paging never issues a scroll command.
    // The newest line stays in view while the person is at the thread's end:
    // a reply streams within its own row (no new row comes) and the keyboard
    // coming up shortens the view; either left the view on the reply's first
    // lines. Reaching the end takes it up; a drag up lets go (a reply read
    // while it streams holds still).
    var follow by remember(t.id) { mutableStateOf(false) }
    val later by rememberUpdatedState(t.later)
    LaunchedEffect(list, t.id) {
        snapshotFlow { Triple(list.canScrollForward, userDragging, later) }.collect { (more, drag, newer) ->
            if (!more && newer == 0) follow = true else if (drag) follow = false
        }
    }
    LaunchedEffect(list, t.id) {
        snapshotFlow { list.layoutInfo }.collect { li ->
            if (follow && later == 0 && !userDragging && !list.isScrollInProgress && list.canScrollForward && li.totalItemsCount > 0)
                list.scrollToItem(li.totalItemsCount - 1, Int.MAX_VALUE / 2)
        }
    }
    LaunchedEffect(count, t.live) {
        if (count > 0 && follow && !userDragging && !list.isScrollInProgress && t.later == 0)
            list.scrollToItem(count - 1, Int.MAX_VALUE / 2)
    }
    // Prefetch before the edge, including the deceleration after a real
    // drag. One accepted request per displayed window; a new page can
    // continue the same fling, while readiness alone cannot start one.
    LaunchedEffect(list, t.id, t.entries.firstOrNull()?.id, t.entries.lastOrNull()?.id, t.earlier, t.later, m.contentReady) {
        var previous = list.firstVisibleItemIndex to list.firstVisibleItemScrollOffset
        val pageKey = "${t.entries.firstOrNull()?.id}:${t.entries.lastOrNull()?.id}:${t.earlier}:${t.later}"
        snapshotFlow { Triple(list.firstVisibleItemIndex, list.firstVisibleItemScrollOffset, list.isScrollInProgress to userDragging) }
            .collect { (index, offset, motion) ->
                val (moving, dragging) = motion
                if (dragging) scrollGesture = true
                if (!moving && !dragging) scrollGesture = false
                if (scrollGesture && (index != previous.first || offset != previous.second))
                    readingDirection = if (index < previous.first || index == previous.first && offset < previous.second) -1 else 1
                previous = index to offset
                if (!scrollGesture || !m.contentReady || m.screen?.hub != s.hub || m.screen?.thread?.id != t.id) return@collect
                val head = (if (t.parent != null) 1 else 0) + (if (t.earlier > 0) 1 else 0)
                val action = when {
                    readingDirection < 0 && t.earlier > 0 && index <= head + 4 -> "earlier"
                    readingDirection > 0 && t.later > 0 && (list.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) >= count - 6 -> "later"
                    else -> ""
                }
                if (action.isNotEmpty() && acceptedPage != "$pageKey:$action") {
                    acceptedPage = "$pageKey:$action"
                    m.act(action, "")
                }
            }
    }
    // the design history's way back: the entry scrolled to (near the top)
    // and tinted, the tint fading out; after the rows the jump opened are laid out
    var lit by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(m.jump) {
        val j = m.jump ?: return@LaunchedEffect
        delay(60)
        val i = t.entries.indexOfFirst { it.id == j.id }
        if (i < 0) return@LaunchedEffect
        val head = (if (t.parent != null) 1 else 0) + (if (t.earlier > 0) 1 else 0)
        // the entry stays where it was brought, whatever streams after it
        follow = false
        list.animateScrollToItem(maxOf(0, head + i - 1))
        lit = j.id
        delay(400)
        if (lit == j.id) lit = null
    }
    Scaffold(
        // opaque, so the entries scrolled under it (a bot's cat and tabs) never show through
        topBar = { Column(Modifier.background(MaterialTheme.colorScheme.surface)) {
            TopAppBar(
                navigationIcon = { IconButton(onClick = { m.back() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, NativeMessages.tour_back(lang)) } },
                title = {
                    Column {
                        Text(t.title, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (t.branch.isNotEmpty()) Text(t.branch, style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.outline, maxLines = 1)
                        // the thread's Boat chip: a tap opens Settings (Usage)
                        if (t.boat.isNotEmpty()) Text(t.boat, style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.primary, maxLines = 1, modifier = Modifier.clickable { m.act("usage-open") })
                    }
                },
                actions = {
                    // the subagents sit in the top bar, a sheet a tap away, so
                    // they never cover the thread
                    t.subs?.let { u ->
                        TextButton(onClick = { subs = true }) {
                            Icon(Icons.Filled.Group, NativeMessages.native_thread_subagents(lang, u.word.ifEmpty { u.busy }),
                                tint = if (u.busy.isNotEmpty()) PhaseColor.accent else MaterialTheme.colorScheme.outline)
                            Text(u.word.ifEmpty { u.busy }.takeWhile { it.isDigit() }, Modifier.padding(start = 2.dp),
                                style = MaterialTheme.typography.labelMedium,
                                color = if (u.busy.isNotEmpty()) PhaseColor.accent else MaterialTheme.colorScheme.outline)
                        }
                    }
                    if (t.viewer.choices.isNotEmpty()) Box {
                        var boards by remember { mutableStateOf(false) }
                        IconButton(onClick = { boards = true }) { Icon(Icons.Filled.Memory, NativeMessages.native_thread_board(lang)) }
                        DropdownMenu(boards, { boards = false }) {
                            for (c in t.viewer.choices) DropdownMenuItem(
                                text = { Text(c.label) },
                                onClick = { boards = false; m.act("view", c.value) },
                            )
                        }
                    }
                    for (tool in t.tools) if (tool.action == "interrupt")
                        IconButton(onClick = { m.act(tool.action) }) { Icon(Icons.Filled.Stop, tool.label) }
                    Box {
                        IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, NativeMessages.common_more(lang)) }
                        DropdownMenu(menu, { menu = false }) {
                            if (t.canChild) DropdownMenuItem(
                                text = { Text("New subthread") },
                                leadingIcon = { Icon(Icons.Filled.Add, null) },
                                onClick = { menu = false; m.act("thread-child", t.id) },
                            )
                            for (tool in t.tools) if (tool.action != "interrupt") DropdownMenuItem(
                                text = { Text(tool.label) },
                                leadingIcon = {
                                    val icon = toolIcon(tool.icon)
                                    when {
                                        tool.on -> Icon(Icons.Filled.Check, null)
                                        icon != null -> Icon(icon, null)
                                        else -> Spacer(Modifier.width(24.dp))
                                    }
                                },
                                onClick = { menu = false; m.act(tool.action, tool.value) },
                            )
                            if (t.menu.isNotEmpty()) HorizontalDivider()
                            for (item in t.menu) {
                                if (item.options.isNotEmpty()) {
                                    Text(item.label, Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                                        style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline)
                                    for (o in item.options) DropdownMenuItem(
                                        text = { Text(o.label) },
                                        leadingIcon = { Spacer(Modifier.width(24.dp)) },
                                        onClick = { menu = false; m.act(item.action, o.value) },
                                    )
                                } else DropdownMenuItem(
                                    text = { Text(item.label, color = if (item.danger) MaterialTheme.colorScheme.error else Color.Unspecified) },
                                    leadingIcon = {
                                        val icon = toolIcon(item.icon)
                                        when {
                                            item.on -> Icon(Icons.Filled.Check, null)
                                            icon != null -> Icon(icon, null, tint = if (item.danger) MaterialTheme.colorScheme.error else Color.Unspecified)
                                            else -> Spacer(Modifier.width(24.dp))
                                        }
                                    },
                                    onClick = {
                                        menu = false
                                        // the terminal opens at the size this phone has room for
                                        m.act(item.action, if (item.action == "term-toggle") termSize() else item.value)
                                    },
                                )
                            }
                        }
                    }
                },
            )
            below?.invoke()
        } },
        bottomBar = {
            Surface(tonalElevation = 3.dp) {
                Column(Modifier.fillMaxWidth().navigationBarsPadding().imePadding().padding(8.dp)) {
                    // the terminals pinned to the top of the thread
                    for (e in t.embs) if (e.pinned) key(e.key) { Box(Modifier.padding(bottom = 8.dp)) { EmbView(m, e, top = true) } }
                    for (a in t.asks) Box(Modifier.padding(bottom = 8.dp)) { AskCard(m, a) }
                    t.todos?.let { td ->
                        Column(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp)) {
                            // the head opens and shuts the card
                            Row(Modifier.fillMaxWidth().then(if (td.toggle.isNotEmpty()) Modifier.clickable { m.act("fold", td.toggle) } else Modifier),
                                verticalAlignment = Alignment.CenterVertically) {
                                Text(td.head, Modifier.weight(1f), style = MaterialTheme.typography.labelMedium)
                                Text(if (td.open) "▾" else "▸", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline)
                            }
                            // every step scrolls within a third of the screen
                            Column(Modifier.heightIn(max = 220.dp).verticalScroll(rememberScrollState())) {
                                for (l in td.lines) Text(l.text, style = MaterialTheme.typography.labelSmall, maxLines = 1, overflow = TextOverflow.Ellipsis,
                                    color = if (l.status == "completed") MaterialTheme.colorScheme.outline else MaterialTheme.colorScheme.onSurface)
                            }
                            if (td.more.isNotEmpty()) Text(td.more, Modifier.clickable { m.act("fold", td.moreValue) }.padding(vertical = 2.dp),
                                style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
                        }
                    }
                    for (q in t.queue) Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(q.tag, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.tertiary)
                        Text(q.text, Modifier.weight(1f).padding(horizontal = 8.dp),
                            style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline,
                            maxLines = 1, overflow = TextOverflow.Ellipsis)
                        for (b in q.buttons) TextButton(onClick = { m.act(b.action, b.value) }) {
                            Text(b.label, style = MaterialTheme.typography.labelSmall)
                        }
                    }
                    Box {
                        var models by remember { mutableStateOf(false) }
                        TextButton(onClick = { models = true }) {
                            Text(t.picker.label, style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.outline, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Icon(Icons.Filled.ExpandMore, NativeMessages.composer_model_tip(lang), Modifier.size(16.dp),
                                tint = MaterialTheme.colorScheme.outline)
                        }
                        DropdownMenu(models, { models = false }) {
                            for (c in t.picker.models) DropdownMenuItem(
                                text = { Text(c.label) },
                                leadingIcon = { if (c.on) Icon(Icons.Filled.Check, null) else Spacer(Modifier.width(24.dp)) },
                                onClick = { models = false; m.act("model", c.value) },
                            )
                            if (t.picker.efforts.isNotEmpty()) {
                                HorizontalDivider()
                                for (c in t.picker.efforts) DropdownMenuItem(
                                    text = { Text(NativeMessages.native_thread_effort_item(lang, c.label)) },
                                    leadingIcon = { if (c.on) Icon(Icons.Filled.Check, null) else Spacer(Modifier.width(24.dp)) },
                                    onClick = { models = false; m.act("effort", c.value) },
                                )
                            }
                            // another provider takes the thread up from its next turn
                            if (t.picker.providers.isNotEmpty()) {
                                HorizontalDivider()
                                for (c in t.picker.providers) DropdownMenuItem(
                                    text = { Text(NativeMessages.native_thread_provider_item(lang, c.label)) },
                                    leadingIcon = { if (c.on) Icon(Icons.Filled.Check, null) else Spacer(Modifier.width(24.dp)) },
                                    onClick = { models = false; m.act("effort", c.value) },
                                )
                            }
                        }
                    }
                    ComposerExtras(m, t)
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        AttachButton(m)
                        ImageComposer(m, Modifier.weight(1f))
                        // a long press sends with the other follow-up mode (queue or steer);
                        // while a turn runs with nothing typed the button stops it, at once,
                        // as the top bar's Stop does (not held to contentReady: that is down
                        // while the Send that started the turn settles, and half the presses
                        // were lost; m.act still drops it on a page not yet live)
                        val stop = t.sendAct == "interrupt" && m.composer.isBlank()
                        Box(Modifier.size(48.dp).combinedClickableCompat(onLong = { if (m.contentReady && m.composer.isNotBlank()) m.act("send-alt") }) {
                            if (stop) m.act("interrupt") else if (m.contentReady && m.composer.isNotBlank()) m.act("send")
                        }, contentAlignment = Alignment.Center) {
                            if (stop) Icon(Icons.Filled.Stop, t.send, tint = MaterialTheme.colorScheme.error)
                            else Icon(Icons.AutoMirrored.Filled.Send, t.send,
                                tint = if (m.composer.isNotBlank()) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline)
                        }
                    }
                }
            }
        },
        snackbarHost = { SnackbarHost(snacks) },
    ) { pad ->
        LazyColumn(Modifier.fillMaxSize(), state = list, contentPadding = PaddingValues(
            start = 16.dp, end = 16.dp, top = pad.calculateTopPadding() + 8.dp, bottom = pad.calculateBottomPadding() + 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)) {
            t.parent?.let { p -> item(key = "parent") { EntryRow(m, p) { shown = it } } }
            // scrolled up to the top while earlier entries are left out: they
            // are shown, no button (the list keeps its place by entry key)
            // (keyed by how many are left out too: a page of entries the list
            // does not draw, todo steps or a fold's calls, moves nothing, and
            // the next is asked for when the person scrolls on)
            if (t.earlier > 0) item(key = "earlier:" + (t.entries.firstOrNull()?.id ?: "") + ":" + t.earlier) {
                Spacer(Modifier.size(1.dp))
            }
            items(t.entries, key = { it.id }) { e ->
                val on = lit == e.id
                val tint by animateFloatAsState(if (on) 0.18f else 0f, tween(if (on) 150 else 2000), label = "jump")
                Column(Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.primary.copy(alpha = tint))) {
                    if (e.kind == "emb") EmbRow(m, e, t.embs.firstOrNull { it.key == e.key })
                    else EntryRow(m, e) { u -> shown = u }
                    e.partial?.let { PartialMessageFooter(m, s.hub, t.id, e.id, it) }
                }
            }
            // on an earlier page, scrolled down to its end: the next page down
            // (as "earlier" at the top; the list keeps its place by entry key)
            if (t.later > 0) item(key = "later:" + (t.entries.lastOrNull()?.id ?: "") + ":" + t.later) {
                Spacer(Modifier.size(1.dp))
            }
            itemsIndexed(t.sending, key = { i, _ -> "sending:$i" }) { _, text -> SendingView(text) }
            if (t.live.isNotEmpty()) item(key = "live") { Markdown(t.live) }
            else if (t.working.isNotEmpty()) item(key = "live") {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        StatusDot(t.state, t.phase)
                        Text(t.working, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.outline)
                    }
                    if (t.phase == "active") LinearProgressIndicator(Modifier.width(120.dp), color = PhaseColor.accent)
                }
            }
        }
    }
    shown?.let { u -> Lightbox(u) { shown = null } }
    t.diff?.let { DiffSheet(m, it) }
}

// The board viewer over the thread: the plot of the screen's source, the
// source choices, and a way back.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PlotScreen(m: AppModel, v: Viewer) {
    var showReflow by remember { mutableStateOf(false) }
    if (showReflow) ReflowSheet(m) { showReflow = false }
    val lang = LocalBackplaneLanguage.current
    // a plot for any other source is stale (a switch in flight), except,
    // while the design history shows, the last version of this kind and
    // project: it stays until the next arrives (no blank between steps)
    val f = m.plots.frame?.takeIf { it.key == v.layers || (v.hist != null && it.none.isEmpty() && plotHead(it.key) == plotHead(v.layers)) }
    val mesh = m.plots.mesh?.takeIf { it.key == v.key } ?: m.plots.cachedMesh(v.key)
    Column(Modifier.fillMaxSize().background(Color(0xFF000000.toInt() or v.bg))) {
    var cam by remember { mutableStateOf(floatArrayOf(0f, 0f, 0f)) }
    if (v.open == "3d" || v.open == "mech") TextButton(onClick = { showReflow = true }) { Text("Reflow preview · illustrative") }
    Box(Modifier.weight(1f).fillMaxWidth()) {
        // a part alone gets a surface of its own: nothing of a board is left on it
        key(if (v.layers.isEmpty()) v.key else "") {
        AndroidView(factory = { PlotSurface(it) }, modifier = Modifier.fillMaxSize(), update = { s ->
            val thermal = m.reflowThermal
            if (!thermal.contentEquals(s.renderer.thermal)) { s.renderer.thermal = thermal.copyOf(); s.requestRender() }
            s.bindSource((m.screen?.sel ?: "") + "\u0000" + v.key.ifEmpty { v.layers } + "\u0000" + v.open)
            s.margin = v.margin
            s.zmin = v.zmin
            s.zmax = v.zmax
            s.tap = v.tap
            s.fadeMs = v.fade
            s.onPick = { m.act("view-pick", it) }
            s.onSheet = { m.act("view-sheet", it) }
            s.sheetHits = v.sheets.mapNotNull { sh ->
                val n = sh.box.split(' ').mapNotNull { it.toFloatOrNull() }
                if (n.size != 4) null
                else sh.value to floatArrayOf(minOf(n[0], n[2]) * 1000f, minOf(n[1], n[3]) * 1000f, maxOf(n[0], n[2]) * 1000f, maxOf(n[1], n[3]) * 1000f)
            }
            s.onNote = { x, y -> m.act("rv-note", "%.3f\t%.3f\t\t".format(java.util.Locale.ROOT, x, y)) }
            s.onCamera = { x, y, k -> cam = floatArrayOf(x, y, k) }
            s.renderer.top = v.top
            s.renderer.bottom = v.bottom
            s.renderer.orbit.fov = v.fov
            s.setThree(v.open == "3d" || v.open == "mech")
            if (f != null && f.none.isEmpty()) s.show(f, v.bg, v.slab, v.look)
            if (s.renderer.off != v.off || s.renderer.dim != v.dim || s.renderer.lift != v.lift) {
                s.renderer.off = v.off; s.renderer.dim = v.dim; s.renderer.lift = v.lift; s.requestRender()
            }
            if (v.open == "3d" || v.open == "mech") s.mesh(mesh)
            s.mark(v.picked)
        })
        }
        when {
            // Mech with no part to show: what the page says
            v.open == "mech" && v.key.isEmpty() -> Text(v.mech?.say?.ifEmpty { null } ?: v.mech?.note ?: "", Modifier.align(Alignment.Center).padding(24.dp), color = Color.Gray)
            // a part alone (from the Mechanical page) has no board plot under it
            v.layers.isEmpty() && mesh == null -> CircularProgressIndicator(Modifier.align(Alignment.Center))
            v.layers.isEmpty() && mesh!!.none.isNotEmpty() -> Text(mesh.none, Modifier.align(Alignment.Center), color = Color.Gray)
            v.layers.isEmpty() -> {}
            f == null -> CircularProgressIndicator(Modifier.align(Alignment.Center))
            f.none.isNotEmpty() -> Text(f.none, Modifier.align(Alignment.Center), color = Color.Gray)
            v.open == "3d" && mesh != null && mesh.none.isNotEmpty() ->
                Text(mesh.none, Modifier.align(Alignment.BottomCenter).padding(16.dp), color = Color.Gray,
                    style = MaterialTheme.typography.labelMedium)
            v.open == "3d" && v.parts && v.note.isNotEmpty() ->
                Text(v.note, Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(16.dp), color = Color.Gray,
                    style = MaterialTheme.typography.labelMedium)
        }
        v.comments?.let { c -> if (v.open == "board" || v.open == "schematic") PinLayer(m, c.pins, cam) }
        Row(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = 12.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically) {
            ChoiceRow(v.choices, v.open, Modifier.weight(1f)) { m.act("view", it) }
            IconButton(onClick = { m.act("view", "") }) { Icon(Icons.Filled.Close, NativeMessages.common_close(lang), tint = if (v.light) Color.Black else Color.White) }
        }
        ViewerControls(m, v, f?.chunks?.map { it.layer }?.toSet() ?: emptySet(), Modifier.align(Alignment.TopStart).statusBarsPadding().padding(top = 56.dp))
        if (v.open == "mech") v.mech?.let { p -> MechBar(m, p, v.light, Modifier.align(Alignment.TopStart).statusBarsPadding().padding(top = 108.dp)) }
        v.card?.let { c -> PlotCard(m, c, Modifier.align(Alignment.BottomCenter)) }
    }
    // the design history under the plot, which gives it the room
    v.hist?.let { h -> HistoryBar(m, h, v.light) }
    }
    v.hist?.let { h -> h.menu?.let { rows -> HistMenu(m, h.side, rows) } }
    v.comments?.let { c ->
        if (c.at.isNotEmpty()) NoteSheet(m, c.at) else c.open?.let { CommentSheet(m, it) }
    }
}

// a plot key's kind and project (a version's key keeps both)
private fun plotHead(k: String) = k.split("|", limit = 3).take(2).joinToString("|")

// The design history under a board or schematic (src/mobile/view.bend's
// Hist.json): the steps and the file now, a scrubber with a tick each,
// what the agent said before the step shown (a tap goes back to it in the
// thread), or a comparison of two versions and its legend
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun HistoryBar(m: AppModel, h: HistBar, light: Boolean) {
    val lang = LocalBackplaneLanguage.current
    val fg = if (light) Color.Black else Color.White
    val dim = fg.copy(alpha = 0.6f)
    val accent = MaterialTheme.colorScheme.primary
    Column(Modifier.fillMaxWidth().background(if (light) Color(0xFFF2F2F2) else Color(0xFF1C1C1E))
        .navigationBarsPadding().padding(horizontal = 12.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (h.cmp) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    HistChip(NativeMessages.history_old_label(lang, h.a), h.side == "a", fg, accent, Modifier.weight(1f, fill = false)) { m.act("cmp-side", "a") }
                    HistChip(NativeMessages.history_new_label(lang, h.b), h.side == "b", fg, accent, Modifier.weight(1f, fill = false)) { m.act("cmp-side", "b") }
                }
                HistChip(NativeMessages.review_bom(lang), false, fg, accent) { m.act("rv-bom", "") }
                HistChip(NativeMessages.common_close(lang), false, fg, accent) { m.act("cmp-close", "") }
            }
            val shown = h.showing.ifEmpty { "diff" }
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                val ways = listOf("old" to NativeMessages.viewer_history_old(lang), "diff" to NativeMessages.viewer_diff(lang), "new" to NativeMessages.viewer_history_new(lang))
                ways.forEachIndexed { i, w ->
                    SegmentedButton(selected = shown == w.first, onClick = { m.act("rv-as", w.first) },
                        shape = SegmentedButtonDefaults.itemShape(i, ways.size), icon = {}) { Text(w.second, maxLines = 1, style = MaterialTheme.typography.labelMedium) }
                }
            }
            if (shown == "diff") Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                val names = listOf(NativeMessages.viewer_history_removed(lang), NativeMessages.viewer_history_changed(lang), NativeMessages.viewer_history_added(lang))
                for (i in 0 until minOf(names.size, h.keys.size)) Row(verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    Box(Modifier.size(10.dp).background(Color(0xFF000000.toInt() or h.keys[i])))
                    Text(names[i], style = MaterialTheme.typography.labelSmall, color = dim)
                }
            }
        } else {
            // one row when the title fits, else the chips go under it
            FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Row(Modifier.align(Alignment.CenterVertically), verticalAlignment = Alignment.CenterVertically) {
                    HistIcon(Icons.Filled.SkipPrevious, NativeMessages.native_history_first(lang), fg) { m.act("hist-first", "") }
                    HistIcon(Icons.AutoMirrored.Filled.KeyboardArrowLeft, NativeMessages.native_history_previous(lang), fg) { m.act("hist-prev", "") }
                    HistIcon(if (h.playing) Icons.Filled.Pause else Icons.Filled.PlayArrow, if (h.playing) NativeMessages.history_pause(lang) else NativeMessages.native_history_play(lang), fg) { m.act("hist-play", "") }
                    HistIcon(Icons.AutoMirrored.Filled.KeyboardArrowRight, NativeMessages.native_history_next(lang), fg) { m.act("hist-next", "") }
                    HistIcon(Icons.Filled.SkipNext, NativeMessages.native_history_last(lang), fg) { m.act("hist-last", "") }
                    Text(h.title, Modifier.padding(start = 4.dp), style = MaterialTheme.typography.labelMedium, color = fg,
                        maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                HistChip(NativeMessages.viewer_history_changes(lang), h.diff, fg, accent, Modifier.align(Alignment.CenterVertically)) { m.act("hist-diff", "") }
                HistChip(NativeMessages.viewer_history_compare(lang), false, fg, accent, Modifier.align(Alignment.CenterVertically)) { m.act("cmp", "a") }
                if (h.at > 0) HistChip(NativeMessages.history_live(lang), false, fg, accent, Modifier.align(Alignment.CenterVertically)) { m.act("hist-live", "") }
            }
            // a tick per step, then the live file; a tap shows that one
            Box(Modifier.fillMaxWidth().height(28.dp), contentAlignment = Alignment.Center) {
                Box(Modifier.fillMaxWidth().height(2.dp).background(dim.copy(alpha = 0.4f)))
                Row(Modifier.fillMaxWidth().fillMaxHeight()) {
                    h.ticks.forEachIndexed { i, tk ->
                        Box(Modifier.weight(1f).fillMaxHeight()
                            .semantics {
                                contentDescription = if (tk.id.isEmpty()) NativeMessages.native_history_livefile(lang) else NativeMessages.history_step_label(lang, (i + 1).toString())
                                role = Role.Button
                                selected = tk.on
                            }
                            .clickable { m.act("hist-go", tk.id) },
                            contentAlignment = Alignment.Center) {
                            Box(Modifier.size(width = if (tk.on) 4.dp else 2.dp, height = if (tk.on) 20.dp else 10.dp)
                                .background(if (tk.on) accent else dim))
                        }
                    }
                }
            }
            if (h.at > 0) Row(Modifier.fillMaxWidth().clickable { m.act("hist-jump", "") }.padding(vertical = 2.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Icon(Icons.AutoMirrored.Filled.Chat, NativeMessages.native_history_thread_short(lang), Modifier.size(16.dp), tint = accent)
                Text(h.said, style = MaterialTheme.typography.labelMedium, color = fg, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            else Text(h.said, style = MaterialTheme.typography.labelMedium, color = dim, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
    }
}

@Composable
private fun HistIcon(icon: ImageVector, label: String, fg: Color, onClick: () -> Unit) {
    Box(Modifier.size(36.dp).clickable(role = Role.Button, onClick = onClick), contentAlignment = Alignment.Center) {
        Icon(icon, label, Modifier.size(22.dp), tint = fg)
    }
}

// square, filled with the accent while on
@Composable
private fun HistChip(label: String, on: Boolean, fg: Color, accent: Color, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Box(modifier.background(if (on) accent else fg.copy(alpha = 0.12f))
        .semantics { selected = on }
        .clickable(role = Role.Button, onClick = onClick)
        .padding(horizontal = 8.dp, vertical = 5.dp)) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = if (on) Color.White else fg,
            maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

// the versions a comparison's side may take ("cmp-pick" value); let go, it shuts
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HistMenu(m: AppModel, side: String, rows: List<HistRow>) {
    val lang = LocalBackplaneLanguage.current
    ModalBottomSheet(onDismissRequest = { m.act("cmp-shut", "") }, sheetState = rememberModalBottomSheetState(),
        shape = androidx.compose.ui.graphics.RectangleShape) {
        Text(if (side == "a") NativeMessages.viewer_history_compare_old(lang) else NativeMessages.viewer_history_compare_new(lang), Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            style = MaterialTheme.typography.titleMedium)
        // any commit, branch or tag (rv-ref "<side>\t<text>")
        var ref by remember { mutableStateOf("") }
        OutlinedTextField(ref, { ref = it }, Modifier.fillMaxWidth().padding(horizontal = 16.dp), singleLine = true,
            placeholder = { Text(NativeMessages.viewer_history_ref(lang)) },
            shape = androidx.compose.ui.graphics.RectangleShape,
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false, imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { if (ref.isNotBlank()) m.act("rv-ref", side + "\t" + ref) }))
        LazyColumn(Modifier.navigationBarsPadding().imePadding()) {
            items(rows) { r ->
                ListItem(
                    headlineContent = { Text(r.label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = if (r.sub.isEmpty()) null else ({
                        Text(r.sub, maxLines = 2, overflow = TextOverflow.Ellipsis, color = MaterialTheme.colorScheme.outline)
                    }),
                    modifier = Modifier.clickable { m.act("cmp-pick", r.value) },
                )
            }
        }
    }
}

// over a part in 3D: the thread's parts (a tap shows another) and Renders
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun MechBar(m: AppModel, p: MechPage, light: Boolean, modifier: Modifier) {
    val lang = LocalBackplaneLanguage.current
    val fg = if (light) Color.Black else Color.White
    FlowRow(modifier.padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        for (c in p.parts) FilterChip(c.on, onClick = { m.act("mech-part", c.value) },
            label = { Text(c.label, color = if (c.on) MaterialTheme.colorScheme.onSecondaryContainer else fg) })
        OutlinedButton(onClick = { m.act("mech-renders", p.rel) }) { Text(NativeMessages.viewer_mech_renders(lang), color = fg) }
    }
}

// The Mechanical page: the viewer's choices (Mech on), a chip per part,
// Open in 3D and Refresh, then the part's renders (a tap opens one full
// screen), and where to get FreeCAD when the hub has none
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun MechScreen(m: AppModel, v: Viewer, p: MechPage) {
    val lang = LocalBackplaneLanguage.current
    var shown by remember { mutableStateOf<String?>(null) }
    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface).statusBarsPadding().navigationBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            SingleChoiceSegmentedButtonRow(Modifier.weight(1f)) {
                v.choices.forEachIndexed { i, c ->
                    SegmentedButton(selected = c.value == v.open, onClick = { m.act("view", c.value) },
                        shape = SegmentedButtonDefaults.itemShape(i, v.choices.size), icon = {}) { Text(c.label, maxLines = 1, style = MaterialTheme.typography.labelMedium) }
                }
            }
            IconButton(onClick = { m.act("view", "") }) { Icon(Icons.Filled.Close, NativeMessages.common_close(lang)) }
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)) {
            if (p.say.isNotEmpty()) Text(p.say, color = MaterialTheme.colorScheme.outline, modifier = Modifier.padding(top = 12.dp))
            if (p.parts.isNotEmpty()) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    for (c in p.parts) FilterChip(c.on, onClick = { m.act("mech-part", c.value) }, label = { Text(c.label) })
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    OutlinedButton(onClick = { m.act("mech-3d", p.path) }, enabled = p.path.isNotEmpty()) { Text(NativeMessages.viewer_solid(lang)) }
                    // the render job: queued on the hub, its pictures in when done
                    Button(onClick = { m.act("mech-render", p.rel) }, enabled = !p.busy && p.rel.isNotEmpty()) {
                        if (p.busy) CircularProgressIndicator(Modifier.size(16.dp).padding(end = 6.dp), strokeWidth = 2.dp)
                        Text(p.render)
                    }
                    OutlinedButton(onClick = { m.act("mech-refresh") }) { Text(NativeMessages.common_refresh(lang)) }
                }
                if (p.err.isNotEmpty()) Text(NativeMessages.native_mech_failed(lang, p.err), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.labelMedium)
            }
            if (p.empty.isNotEmpty()) Text(p.empty, color = MaterialTheme.colorScheme.outline)
            for (s in p.shots) Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(s.view, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline)
                HubPicture(m, s.url, Modifier.fillMaxWidth()) { shown = it }
            }
            if (p.note.isNotEmpty()) Text(p.note, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline,
                modifier = Modifier.padding(bottom = 12.dp))
        }
    }
    shown?.let { u -> Lightbox(u) { shown = null } }
}

// under the viewer's bar: the schematic's sheet, the layers shown, and
// the 3D model's parts
@Composable
private fun ViewerControls(m: AppModel, v: Viewer, present: Set<Int>, modifier: Modifier) {
    val lang = LocalBackplaneLanguage.current
    // only the layers this board or sheet has
    val layers = v.layerList.filter { it.layer in present }
    val fg = if (v.light) Color.Black else Color.White
    Row(modifier.padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        // the viewer's own light or dark ground
        IconButton(onClick = { m.act("vw-light") }) {
            Icon(if (v.light) Icons.Filled.DarkMode else Icons.Filled.LightMode, if (v.light) NativeMessages.native_viewer_ground_dark(lang) else NativeMessages.native_viewer_ground_light(lang), tint = fg)
        }
        // a project's boards lead the menu ("b:" values), then the sheets; the
        // button names the sheet on show, or the board where there are none
        if (v.sheets.isNotEmpty()) Box {
            var open by remember { mutableStateOf(false) }
            OutlinedButton(onClick = { open = true }) {
                Text(v.sheets.lastOrNull { it.on }?.label?.trim() ?: NativeMessages.native_viewer_sheet(lang), color = fg, maxLines = 1)
                Icon(Icons.Filled.ExpandMore, null, tint = fg)
            }
            DropdownMenu(open, { open = false }) {
                for ((i, sh) in v.sheets.withIndex()) {
                    if (i > 0 && v.sheets[i - 1].value.startsWith("b:") && !sh.value.startsWith("b:")) HorizontalDivider()
                    DropdownMenuItem(text = { Text(sh.label) }, enabled = !sh.loop,
                        leadingIcon = { if (sh.on) Icon(Icons.Filled.Check, null) else Spacer(Modifier.width(24.dp)) },
                        onClick = { open = false; m.act("view-sheet", sh.value) })
                }
            }
        }
        if (layers.isNotEmpty()) Box {
            var open by remember { mutableStateOf(false) }
            OutlinedButton(onClick = { open = true }) {
                Icon(Icons.Filled.Layers, null, tint = fg)
                Text(v.layerHead, color = fg, maxLines = 1, modifier = Modifier.padding(start = 6.dp))
            }
            // stays open: several layers are turned on and off in a row. A
            // row's box shows or hides it; its name makes it the active layer
            // (again: all back), and over a board's rows All, Dim, Only say how
            // the others show (core/layers.bend)
            DropdownMenu(open, { open = false }) {
                if (v.layerModes) Row(Modifier.padding(horizontal = 12.dp, vertical = 4.dp)) {
                    for ((i, label) in listOf(NativeMessages.viewer_layers_all(lang), NativeMessages.viewer_layers_dim(lang), NativeMessages.viewer_layers_only(lang)).withIndex())
                        FilterChip(v.layerMode == i, { m.act("view-layer-mode", i.toString()) }, label = { Text(label) },
                            modifier = Modifier.padding(end = 6.dp))
                }
                for (l in layers) DropdownMenuItem(
                    text = { Text(l.name, color = if (l.active) MaterialTheme.colorScheme.primary else Color.Unspecified) },
                    leadingIcon = { Checkbox(l.on, { m.act("view-layer", l.layer.toString()) }) },
                    onClick = { m.act("view-layer-pick", l.layer.toString()) })
            }
        }
        if (v.open == "3d" && v.layers.isNotEmpty()) FilterChip(v.parts, { m.act("view-parts") }, label = { Text(NativeMessages.native_viewer_parts(lang), color = fg) },
            leadingIcon = { if (v.parts) Icon(Icons.Filled.Check, null, tint = fg) })
    }
}

// what a tapped item is, as the window's inspector shows it
@Composable
private fun PlotCard(m: AppModel, c: Card, modifier: Modifier) {
    val lang = LocalBackplaneLanguage.current
    Surface(modifier.fillMaxWidth().navigationBarsPadding().padding(12.dp), tonalElevation = 6.dp) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(c.title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                IconButton(onClick = { m.act("view-unpick") }) { Icon(Icons.Filled.Close, NativeMessages.common_close(lang)) }
            }
            for ((k, value) in c.rows) Row {
                Text(k, Modifier.width(92.dp), color = MaterialTheme.colorScheme.outline, style = MaterialTheme.typography.bodyMedium)
                Text(value, style = MaterialTheme.typography.bodyMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            Button(onClick = { m.act("view-mention", c.info) }) { Text(NativeMessages.viewer_mention(lang)) }
        }
    }
}

// Compose 1.7's contentReceiver is supported by the state-based field.
// Keep text in that field; only image URI items are consumed as uploads.
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ImageComposer(m: AppModel, modifier: Modifier = Modifier) {
    val lang = LocalBackplaneLanguage.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val field = rememberTextFieldState(m.composer)
    var forwardedDraft by remember { mutableStateOf(m.composer) }
    LaunchedEffect(m.composer) {
        val incoming = m.composer
        if (incoming != forwardedDraft) {
            forwardedDraft = incoming
            if (field.text.toString() != incoming) field.setTextAndPlaceCursorAtEnd(incoming)
        }
    }
    LaunchedEffect(field) {
        snapshotFlow { field.text.toString() }.collect { text ->
            if (text != m.composer) {
                forwardedDraft = text
                m.draft(text)
            }
        }
    }
    val receiver = remember(m, ctx, scope) {
        ReceiveContentListener { content ->
            if (!content.hasMediaType(MediaType.Image)) content
            else content.consume { item ->
                val uri = item.uri
                val image = uri != null && runCatching {
                    ctx.contentResolver.getType(uri)?.startsWith("image/") == true
                }.getOrDefault(false)
                if (image) scope.launch { attachUris(ctx, m, listOf(uri!!), true) }
                image
            }
        }
    }
    BasicTextField(
        state = field,
        modifier = modifier.contentReceiver(receiver)
            .border(1.dp, MaterialTheme.colorScheme.outline, RoundedCornerShape(4.dp))
            .padding(horizontal = 16.dp, vertical = 12.dp),
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onSurface),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        lineLimits = TextFieldLineLimits.MultiLine(maxHeightInLines = 6),
        decorator = { inner ->
            Box {
                if (field.text.isEmpty()) Text(NativeMessages.native_thread_composer(lang), color = MaterialTheme.colorScheme.onSurfaceVariant)
                inner()
            }
        },
    )
}
