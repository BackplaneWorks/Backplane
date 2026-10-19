package dev.backplane.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Switch
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntOffset
import kotlin.math.roundToInt

// The viewer's Review tab (client.bend's Rp, mobile/view.bend's Rv.json): a
// chip per page, the page's title and the report as Markdown (the apps'
// own; its tables scroll sideways), or what to say while there is none.
// Refresh sits at the bottom, under the thumb.
@Composable
fun ReviewScreen(m: AppModel, v: Viewer, r: ReviewPage) {
    val lang = LocalBackplaneLanguage.current
    // a Surface, so text with no colour of its own follows the theme (it was black on the dark surface)
    androidx.compose.material3.Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface, contentColor = MaterialTheme.colorScheme.onSurface) {
    Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
        ViewerChoices(m, v)
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (t in r.tabs) FilterChip(t.on, onClick = { m.act("rv-page", t.page) }, label = { Text(t.label) },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = MaterialTheme.colorScheme.primary,
                    selectedLabelColor = MaterialTheme.colorScheme.onPrimary))
        }
        Text(r.title, Modifier.padding(horizontal = 16.dp, vertical = 8.dp), style = MaterialTheme.typography.titleMedium, maxLines = 2)
        HorizontalDivider()
        Box(Modifier.weight(1f).fillMaxWidth()) {
            if (r.blocks.isNotEmpty() && !r.busy) {
                Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp)) {
                    r.checks?.let { ChecksBoxes(m, it) }
                    r.release?.let { ReleaseBox(m, it, r.busy) }
                    Markdown(r.blocks)
                }
            } else if (r.release != null) {
                Column(Modifier.fillMaxSize().imePadding().verticalScroll(rememberScrollState()).padding(16.dp)) {
                    ReleaseBox(m, r.release, r.busy)
                    if (r.busy) CircularProgressIndicator()
                    Text(r.note, color = if (r.release.err.isEmpty()) MaterialTheme.colorScheme.outline else MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp).testTag("release-note"))
                }
            } else {
                Column(Modifier.align(Alignment.Center).padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    if (r.busy) CircularProgressIndicator()
                    Text(r.note, color = MaterialTheme.colorScheme.outline, textAlign = TextAlign.Center)
                }
            }
        }
        if (r.release == null) {
            HorizontalDivider()
            Button(onClick = { m.act("rv-run", "") }, enabled = !r.busy, modifier = Modifier.fillMaxWidth().padding(12.dp)) {
                Icon(Icons.Filled.Refresh, null)
                Text("  " + NativeMessages.common_refresh(lang))
            }
        }
    }
    }
}

// The Release page's form: the tag (empty releases the proposed one), two
// switches off until the person turns them on (Publish on GitHub pushes the
// tag and makes a GitHub release), and the button. The tag goes to Bend as
// the field "rltag" while it is typed.
@Composable
fun ReleaseBox(m: AppModel, f: ReleaseForm, busy: Boolean) {
    val lang = LocalBackplaneLanguage.current
    var text by remember { mutableStateOf(f.text) }
    Column(Modifier.fillMaxWidth().padding(bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        OutlinedTextField(text, onValueChange = { text = it; m.field("rltag", it) }, placeholder = { Text(f.next) },
            singleLine = true, modifier = Modifier.fillMaxWidth().testTag("release-tag"),
            keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Go),
            keyboardActions = androidx.compose.foundation.text.KeyboardActions(onGo = { if (!busy) m.act("rv-release", "") }))
        Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(NativeMessages.review_publish(lang), Modifier.weight(1f))
            Switch(f.publish, onCheckedChange = { m.act("rv-release-pub", "") }, modifier = Modifier.testTag("release-pub"))
        }
        Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(NativeMessages.review_force(lang), Modifier.weight(1f))
            Switch(f.force, onCheckedChange = { m.act("rv-release-force", "") }, modifier = Modifier.testTag("release-force"))
        }
        // the field is typed without a new screen, so the label follows the text here
        val tag = text.trim().ifEmpty { f.tag }
        Button(onClick = { m.act("rv-release", "") }, enabled = !busy && tag.isNotEmpty(),
            modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp).testTag("release-make")) { Text(NativeMessages.review_release_make(lang, tag)) }
    }
}

// The viewer's source choices, each as wide as its words (six of them do not
// fit a phone side by side), the row scrolling sideways, the picked one in view
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChoiceRow(choices: List<Choice>, picked: String, modifier: Modifier = Modifier, pick: (String) -> Unit) {
    val scroll = rememberScrollState()
    LaunchedEffect(picked) { if (picked == choices.lastOrNull()?.value) scroll.scrollTo(scroll.maxValue) else scroll.scrollTo(0) }
    Row(modifier.horizontalScroll(scroll)) {
        SingleChoiceSegmentedButtonRow {
            choices.forEachIndexed { i, c ->
                SegmentedButton(selected = c.value == picked, onClick = { pick(c.value) },
                    shape = SegmentedButtonDefaults.itemShape(i, choices.size), icon = {},
                    colors = SegmentedButtonDefaults.colors(activeContainerColor = MaterialTheme.colorScheme.primary,
                        activeContentColor = MaterialTheme.colorScheme.onPrimary,
                        inactiveContainerColor = MaterialTheme.colorScheme.surface,
                        inactiveContentColor = MaterialTheme.colorScheme.onSurface)) {
                    Text(c.label, maxLines = 1, softWrap = false, style = MaterialTheme.typography.labelMedium)
                }
            }
        }
    }
}

// The Checks page's gate and checklist: a red "Fails: ..." or a green
// "Passes", then the boxes as checkboxes under their headings. A tap sends
// "rv-tick" index; the hub writes .backplane/checklist.md and answers the
// boxes, so a box shows what the hub says.
@Composable
fun ChecksBoxes(m: AppModel, c: ReviewChecks) {
    val lang = LocalBackplaneLanguage.current
    val pass = c.gate.isEmpty()
    Column(Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
        Text(if (pass) NativeMessages.review_passes(lang) else NativeMessages.review_fails(lang, c.gate), color = if (pass) Color(0xFF2E9E4F) else Color(0xFFD93025),
            style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(bottom = 8.dp).testTag("checks-gate"))
        var last: String? = null
        for (t in c.tasks) {
            if (t.group != last) {
                last = t.group
                if (t.group.isNotEmpty()) Text(t.group, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 8.dp, bottom = 2.dp))
            }
            Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).clickable { m.act("rv-tick", t.index.toString()) }.testTag("task-" + t.index),
                verticalAlignment = Alignment.CenterVertically) {
                Checkbox(t.done, onCheckedChange = { m.act("rv-tick", t.index.toString()) })
                Text(t.text, Modifier.weight(1f).padding(start = 4.dp))
            }
        }
    }
}

// A pin is a square in the accent colour (dimmed when resolved) with its
// reply count, in a 48 dp touch area, placed with the plot's own transform
// (the camera the surface reports), so it follows pan and zoom.
@Composable
fun PinLayer(m: AppModel, pins: List<ReviewPin>, cam: FloatArray) {
    if (cam[2] <= 0f) return
    val accent = MaterialTheme.colorScheme.primary
    val half = with(androidx.compose.ui.platform.LocalDensity.current) { 24.dp.roundToPx() }
    for (p in pins) {
        val x = (p.x * 1000f * cam[2] + cam[0]).roundToInt() - half
        val y = (p.y * 1000f * cam[2] + cam[1]).roundToInt() - half
        Box(Modifier.offset { IntOffset(x, y) }.size(48.dp).clickable { m.act("rv-pin", p.id) }.testTag("pin-" + p.id),
            contentAlignment = Alignment.Center) {
            Box(Modifier.size(24.dp).background(if (p.resolved) accent.copy(alpha = 0.4f) else accent).border(1.5.dp, Color.White),
                contentAlignment = Alignment.Center) {
                if (p.count > 0) Text(p.count.toString(), color = Color.White, style = MaterialTheme.typography.labelSmall)
            }
        }
    }
}

// The note for a new comment, a bottom sheet that leaves the board in view
// (partial, no scrim, the keyboard lifts it). The text goes to Bend as the
// field "rcnote" while it is typed; Send posts it.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NoteSheet(m: AppModel, at: String) {
    val lang = LocalBackplaneLanguage.current
    var text by remember { mutableStateOf("") }
    val parts = at.split("\t")
    val place = if (parts.size >= 2) NativeMessages.native_review_place(lang, "%.2f".format(java.util.Locale.ROOT, parts[0].toFloatOrNull() ?: 0f), "%.2f".format(java.util.Locale.ROOT, parts[1].toFloatOrNull() ?: 0f)) else ""
    val focus = remember { FocusRequester() }
    ModalBottomSheet(onDismissRequest = { m.act("rv-note-shut", "") }, sheetState = rememberModalBottomSheetState(),
        scrimColor = Color.Transparent, modifier = Modifier.imePadding()) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp).padding(bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(NativeMessages.native_review_comment(lang), style = MaterialTheme.typography.titleMedium)
                Text("  " + place, color = MaterialTheme.colorScheme.outline, style = MaterialTheme.typography.labelMedium, modifier = Modifier.weight(1f))
                TextButton(onClick = { m.act("rv-note-shut", "") }) { Text(NativeMessages.common_cancel(lang)) }
            }
            OutlinedTextField(text, onValueChange = { text = it; m.field("rcnote", it) }, placeholder = { Text(NativeMessages.native_review_write(lang)) },
                modifier = Modifier.fillMaxWidth().focusRequester(focus).testTag("note-field"), maxLines = 4)
            Button(onClick = { m.act("rv-note-send", "") }, enabled = text.isNotBlank(), modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp).testTag("note-send")) { Text(NativeMessages.composer_send(lang)) }
        }
    }
    LaunchedEffect(Unit) { focus.requestFocus() }
}

// The open comment: its words, who, the replies, a reply field, Resolve or
// Reopen, Close. No scrim, so the pin stays in view above the sheet.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CommentSheet(m: AppModel, o: ReviewOpen) {
    val lang = LocalBackplaneLanguage.current
    var text by remember(o.id) { mutableStateOf("") }
    ModalBottomSheet(onDismissRequest = { m.act("rv-pin", "") }, sheetState = rememberModalBottomSheetState(),
        scrimColor = Color.Transparent, modifier = Modifier.imePadding()) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp).padding(bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(o.by, style = MaterialTheme.typography.titleMedium)
                if (o.resolved) Text("  " + NativeMessages.native_review_resolved(lang), color = Color(0xFF2E9E4F), style = MaterialTheme.typography.labelMedium)
                Spacer(Modifier.weight(1f))
                TextButton(onClick = { m.act("rv-pin", "") }) { Text(NativeMessages.common_close(lang)) }
            }
            if (o.place.isNotEmpty()) Text(o.place, color = MaterialTheme.colorScheme.outline, style = MaterialTheme.typography.labelMedium)
            Column(Modifier.heightIn(max = 220.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(o.text)
                for (r in o.replies) Column(Modifier.padding(start = 12.dp)) {
                    Text(r.by, color = MaterialTheme.colorScheme.outline, style = MaterialTheme.typography.labelSmall)
                    Text(r.text)
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(text, onValueChange = { text = it; m.field("rcreply", it) }, placeholder = { Text(NativeMessages.review_reply(lang)) },
                    modifier = Modifier.weight(1f).testTag("reply-field"), maxLines = 3)
                Button(onClick = { m.act("rv-reply-send", ""); text = "" }, enabled = text.isNotBlank(), modifier = Modifier.heightIn(min = 48.dp).testTag("reply-send")) { Text(NativeMessages.review_reply(lang)) }
            }
            OutlinedButton(onClick = { m.act(if (o.resolved) "rv-reopen" else "rv-resolve", o.id) }, modifier = Modifier.heightIn(min = 48.dp).testTag("resolve")) {
                Text(if (o.resolved) NativeMessages.review_reopen(lang) else NativeMessages.review_resolve(lang))
            }
        }
    }
}
