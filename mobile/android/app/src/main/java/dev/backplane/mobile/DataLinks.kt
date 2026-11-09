package dev.backplane.mobile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject

// A tap on a link in a message (docs/PHONE-DATA.md, "Links in messages"; the iOS app's DataLinks.swift is the same). Web addresses open in the browser, emails in Mail,
// phone numbers in the dialer. A backplane:// link or a path is asked of the hub (link.resolve), which says what it opens: a thread (also one on a linked machine), a
// project, a room, a file at a line, or a short note why not. DataRoot follows `DataClient.linkTarget`.
data class LinkTarget(val kind: Kind, val id: String, val path: String = "", val line: Int = 0, val n: Long = System.nanoTime()) {
    enum class Kind { THREAD, PROJECT, ROOM, FILE }
}

/** What to ask the hub for a tapped link: a backplane:// address (this build's own scheme counts as it), or the path a bppath: link carries. */
private fun linkText(url: String): String? {
    pathOf(url)?.let { return it }
    if (url.startsWith("backplane://")) return url
    if (url.startsWith(BuildConfig.URL_SCHEME + "://")) return "backplane" + url.removePrefix(BuildConfig.URL_SCHEME)
    return null
}

fun DataClient.openLink(url: String, thread: String) {
    val text = linkText(url) ?: return
    scope.launch {
        // a link tapped before the hub has said hello (a cached list, just launched) waits a moment for it: only the hub knows what `desktop~t9` is
        for (i in 0 until 50) { if (conn == DataClient.Conn.ONLINE && has("links")) break; delay(100) }
        // a hub that does not know link.resolve: a thread link as before, the rest says so
        if (!has("links")) {
            val id = if (text.startsWith("backplane://thread/")) text.removePrefix("backplane://thread/").substringBefore('#').substringBefore('/') else ""
            if (id.isNotEmpty()) linkTarget = LinkTarget(LinkTarget.Kind.THREAD, id) else say(Msg.android_link_old)
            return@launch
        }
        try {
            val r = call("link.resolve", JSONObject().put("url", text).put("thread", thread))
            linkTarget = when (r.optString("kind")) {
                "thread" -> LinkTarget(LinkTarget.Kind.THREAD, r.optString("thread"))
                "project" -> LinkTarget(LinkTarget.Kind.PROJECT, r.optString("project"))
                "room" -> LinkTarget(LinkTarget.Kind.ROOM, r.optString("room"))
                "file" -> LinkTarget(LinkTarget.Kind.FILE, r.optString("thread"), r.optString("path"), r.optInt("line"))
                else -> { say(Tr.t(r.optString("note").ifEmpty { "That link does not open here." })); null }
            }
        } catch (e: Exception) { say(if (conn == DataClient.Conn.ONLINE) (e.message ?: Msg.android_link_failed) else Msg.android_link_offline) }
    }
}

/** A short note over the screen, gone by itself. */
fun DataClient.say(note: String) {
    linkNote = note
    scope.launch { delay(3500); if (linkNote == note) linkNote = "" }
}

/** The note a link that opens nothing leaves. */
@Composable
fun BoxScope.LinkNote() {
    val n = DataClient.linkNote
    if (n.isNotEmpty()) Text(Tr.t(n), Modifier.align(Alignment.BottomCenter).padding(bottom = 70.dp, start = 16.dp, end = 16.dp).background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 12.dp, vertical = 9.dp).testTag("link.note"),
        color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp)
}

/** Plain text with its links tappable (a user's message, a room post, a note): not Markdown, so `*` and `_` stay as typed. Selectable. */
@Composable
fun LinkText(text: String, thread: String, modifier: Modifier = Modifier, color: Color = dcs.onSurface, size: Int = 13, lineHeight: Int = 0) {
    val ctx = LocalContext.current
    SideEffect { MdLinks.ctx = ctx.applicationContext; if (thread.isNotEmpty()) MdLinks.thread = thread }
    val link = dcs.primary
    val styled = remember(text, link) {
        buildAnnotatedString {
            linkRuns(text).forEach { (t, u) ->
                if (u == null) append(t)
                else withLink(LinkAnnotation.Clickable(u, TextLinkStyles(SpanStyle(color = link, textDecoration = TextDecoration.Underline))) { MdLinks.open(u) }) { append(t) }
            }
        }
    }
    SelectionContainer { Text(styled, modifier, color = color, fontSize = size.sp, lineHeight = if (lineHeight > 0) lineHeight.sp else androidx.compose.ui.unit.TextUnit.Unspecified, fontFamily = BpMono) }
}
