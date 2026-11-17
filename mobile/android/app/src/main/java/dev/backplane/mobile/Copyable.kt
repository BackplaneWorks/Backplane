package dev.backplane.mobile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

// Copyable: one copy button wherever text is there to be copied (a code block, an inline command, a path, an id, a pairing code), as GitHub draws on a
// README's code, and Run in terminal beside a shell command (the desktop's screens/copyable.rs). On a touch screen the icon is always there; a tap (or
// Enter / D-pad centre when it has the focus) puts the exact text on the clipboard (CopyCore.kt `Copy.exact`) and the icon turns into a check, "Copied",
// for 1.5 s. What is copyable and what is a command comes from the hub (CopyCore.kt).

/** The system clipboard, written from the tap that asked for it. */
fun copyToClipboard(ctx: android.content.Context, text: String) {
    (ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager)
        .setPrimaryClip(android.content.ClipData.newPlainText("Backplane", Copy.exact(text)))
}

/** Whether the button was used within Copy.SHOWN_MS: the check shows, then goes by itself (one recomposition, no loop). */
@Composable
private fun rememberCopied(key: Any): MutableState<Boolean> {
    val s = remember(key) { mutableStateOf(false) }
    LaunchedEffect(s.value) { if (s.value) { delay(Copy.SHOWN_MS); s.value = false } }
    return s
}

/** The copy button for `text`: the icon (and the word with [word]), a check and "Copied" for a moment after a tap, Enter or D-pad centre. */
@Composable
fun CopyButton(text: String, modifier: Modifier = Modifier, word: Boolean = false, tag: String = "copy", size: Int = 16) {
    val ctx = LocalContext.current
    var copied by rememberCopied(text)
    var focused by remember { mutableStateOf(false) }
    val label = if (copied) Msg.android_copied else Msg.common_copy
    val c = if (copied) dok else dcs.onSurfaceVariant
    Row(modifier.onFocusChanged { focused = it.isFocused }.border(BorderStroke(1.dp, if (focused) dcs.primary else Color.Transparent))
        .semantics(mergeDescendants = true) { contentDescription = label; role = Role.Button }
        .clickable(onClickLabel = Msg.common_copy) { copyToClipboard(ctx, text); copied = true }
        .padding(horizontal = 4.dp, vertical = 3.dp).testTag(tag), verticalAlignment = Alignment.CenterVertically) {
        Icon(if (copied) Icons.Default.Check else Icons.Default.ContentCopy, null, tint = c, modifier = Modifier.size(size.dp))
        if (word) { Spacer(Modifier.width(4.dp)); Text(label, color = c, fontFamily = BpMono, fontSize = 11.sp, maxLines = 1) }
    }
}

/** Run in terminal beside a command's copy button: the chat's terminal opens with `cmd` typed at the prompt, waiting for the person's Enter. */
@Composable
fun RunButton(cmd: String, run: (String) -> Unit, modifier: Modifier = Modifier, word: Boolean = false, tag: String = "run", size: Int = 16) {
    var focused by remember { mutableStateOf(false) }
    val label = Msg.android_copy_run
    Row(modifier.onFocusChanged { focused = it.isFocused }.border(BorderStroke(1.dp, if (focused) dcs.primary else Color.Transparent))
        .semantics(mergeDescendants = true) { contentDescription = label; role = Role.Button }
        .clickable(onClickLabel = label) { run(cmd) }
        .padding(horizontal = 4.dp, vertical = 3.dp).testTag(tag), verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.Default.Terminal, null, tint = dcs.onSurfaceVariant, modifier = Modifier.size(size.dp))
        if (word) { Spacer(Modifier.width(4.dp)); Text(label, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 11.sp, maxLines = 1) }
    }
}

/** A labelled copy press of the hub's (a row's Copy link, Copy code: action `copy`): the same copy and check as CopyButton, the label "Copied" for a moment. */
@Composable
fun CopyChip(label: String, text: String, modifier: Modifier = Modifier, compact: Boolean = false) {
    val ctx = LocalContext.current
    var copied by rememberCopied(text)
    var focused by remember { mutableStateOf(false) }
    val shown = if (copied) Msg.android_copied else label
    Row(modifier.background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, if (focused) dcs.primary else dcs.outlineVariant)).onFocusChanged { focused = it.isFocused }
        .semantics(mergeDescendants = true) { contentDescription = shown; role = Role.Button }
        .clickable { copyToClipboard(ctx, text); copied = true }
        .heightIn(min = if (compact) 32.dp else 40.dp).padding(horizontal = if (compact) 10.dp else 14.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(if (copied) Icons.Default.Check else Icons.Default.ContentCopy, null, tint = if (copied) dok else dcs.onSurfaceVariant, modifier = Modifier.size(14.dp))
        Spacer(Modifier.width(6.dp)); Text(shown, color = dcs.onSurface, fontFamily = BpMono, fontSize = if (compact) 12.sp else 13.sp, maxLines = 1)
    }
}

/** A value shown to be copied (a path, an id, a link): the text and its copy button beside it. */
@Composable
fun CopyValue(shown: @Composable RowScope.() -> Unit, text: String, modifier: Modifier = Modifier, tag: String = "copy") {
    Row(modifier, verticalAlignment = Alignment.CenterVertically) { shown(); Spacer(Modifier.width(4.dp)); CopyButton(text, tag = tag, size = 14) }
}

/** The pieces of a plain text to copy as the hub lists them (a room's post, a notice): each one's first line in code type with its copy button, and Run in
 *  terminal beside a command when the chat on screen has a terminal. */
@Composable
fun CopyPieces(pieces: List<CopyPiece>, tag: String = "copy.piece", modifier: Modifier = Modifier) {
    if (pieces.isEmpty()) return
    val run = LocalChatTerm.current?.takeIf { DataClient.has("termChat") }
    Column(modifier.padding(top = 4.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        pieces.take(16).forEachIndexed { i, p ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(Copy.shown(p.text), Modifier.weight(1f, fill = false).background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 6.dp, vertical = 2.dp),
                    color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Spacer(Modifier.width(4.dp))
                CopyButton(p.text, tag = "$tag.$i.copy")
                if (p.cmd && run != null) RunButton(p.text, run::run, tag = "$tag.$i.run")
            }
        }
    }
}

// ---- the chat's terminal (feature termChat): each chat page says which shell it is

/** The chat on screen's shell: its key for `term.new {chat}` (a thread's id, a room's `room:<id>`), and how to show it. Provided by a chat's page. */
class ChatTerm(val key: String, private val show: (String) -> Unit) {
    /** Opens the chat's terminal full screen (the hub starts its shell, or only resizes the one that runs). */
    fun open() { Terms.openChat(key) { show(it) } }
    /** Run in terminal: the terminal opens with `cmd` typed at the prompt, not run. */
    fun run(cmd: String) { if (Copy.exact(cmd).isBlank()) return; Terms.openChat(key) { k -> Terms.prefill(k, cmd); show(k) } }
}
internal val LocalChatTerm = compositionLocalOf<ChatTerm?> { null }

/** The chat terminal a page provides, when the hub has the feature (else null: a hub that ignores `chat` would start a stray shell). */
@Composable
fun rememberChatTerm(key: String): ChatTerm? {
    val open = LocalTermOpen.current
    val on = DataClient.has("termChat") && key.isNotEmpty()
    return remember(key, on, open) { if (on) ChatTerm(key, open) else null }
}

/** The terminal button of a chat page's top bar. */
@Composable
fun ChatTermButton(ct: ChatTerm?) {
    if (ct == null) return
    androidx.compose.material3.IconButton(onClick = { ct.open() }, modifier = Modifier.testTag("chat.term")) {
        Icon(Icons.Default.Terminal, Msg.palette_terminal, tint = dcs.onSurfaceVariant)
    }
}
