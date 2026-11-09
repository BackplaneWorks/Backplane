package dev.backplane.mobile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** A subagent row opens its view (feature subview): a fold's Agent call (`tool.sub`) and a Background work row (`key`) call this. */
internal val LocalOpenSub = staticCompositionLocalOf<(String) -> Unit> { {} }

/** A long result shows three lines until tapped (the desktop's subview.rs `result_lines`). */
internal fun subResultLines(text: String, open: Boolean): Pair<String, Boolean> {
    val ls = text.lines(); if (open || ls.size <= 3) return text to false
    return ls.take(3).joinToString("\n") to true
}

/** One subagent's own view over its thread: what it was asked, what it said, each tool call with its result and how it ended; live while it works, Back to the thread. */
@Composable
internal fun SubScreen(thread: String, key: String, back: () -> Unit) {
    DisposableEffect(thread, key) { DataClient.openSub(thread, key); onDispose { DataClient.closeSub() } }
    val v = DataClient.subView?.takeIf { it.key == key && it.thread == thread }
    val title = DataClient.threads.firstOrNull { it.id == thread }?.title ?: "the thread"
    val state = rememberLazyListState()
    val n = v?.entries?.size ?: 0
    // live: a new step is followed while the reader is at the end (or has just opened it)
    var seen by remember(key) { mutableStateOf(0) }
    LaunchedEffect(n) { if (n > 0 && (seen == 0 || !state.canScrollForward || state.layoutInfo.visibleItemsInfo.lastOrNull()?.index?.let { it >= seen - 1 } == true)) state.scrollToItem(n + 1); seen = n }
    Column(Modifier.fillMaxSize().testTag("sub.screen")) {
        DBar(v?.title ?: "Subagent", back)
        Column(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                DMono("Back to $title", dcs.primary, 12, Modifier.weight(1f).clickable(onClick = back).testTag("sub.back"))
                if (v != null) DMono(if (v.took.isEmpty()) v.state else "${v.state} · ${v.took}", if (v.live) dok else dfaint, 12, Modifier.testTag("sub.state"))
                if (v != null && v.label.isNotEmpty()) { Spacer(Modifier.width(8.dp)); DBtn(v.label, false, modifier = Modifier.testTag("sub.stop")) { DataClient.workAct(thread, DWorkRow("", "", "", "", v.act, v.value, v.label)) } }
            }
            if (v != null) listOf(v.meta, v.where).filter { it.isNotEmpty() }.forEach { DMono(it, dfaint, 11, lines = 2) }
        }
        LazyColumn(Modifier.weight(1f).fillMaxWidth().testTag("sub.list"), state = state, contentPadding = PaddingValues(vertical = 8.dp)) {
            if (v == null) item("loading") { DMono(if (DataClient.subGone) "This subagent's steps are no longer kept." else "Loading…", dcs.onSurfaceVariant, 13, Modifier.padding(14.dp)) }
            if (v != null && v.more > 0) item("more") { DMono("${v.more} earlier steps not shown", dfaint, 11, Modifier.padding(horizontal = 12.dp, vertical = 4.dp)) }
            if (v != null && v.entries.isEmpty()) item("none") { DMono("No steps reported yet.", dcs.onSurfaceVariant, 13, Modifier.padding(14.dp)) }
            items(v?.entries ?: emptyList(), key = { it.id }) { e -> SubEntry(thread, e) }
        }
    }
}

@Composable
private fun SubEntry(thread: String, e: DEntry) {
    when (e.kind) {
        "user" -> Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp).background(dcs.surfaceContainerHigh).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(10.dp).testTag("sub.asked")) {
            DMono("Asked", dfaint, 11); LinkText(e.text, thread, color = dcs.onSurface, size = 13, lineHeight = 19)
        }
        "assistant" -> Box(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp)) { LinkText(e.text, thread, color = dcs.onSurface, size = 13, lineHeight = 19) }
        "tool" -> {
            var open by remember(e.id) { mutableStateOf(false) }
            val t = e.tool; val st = t?.state ?: "done"
            val (res, cut) = subResultLines(t?.result ?: "", open)
            val tone = when (st) { "running" -> dok; "failed" -> dcs.error; else -> dfaint }
            Column(Modifier.fillMaxWidth().clickable(enabled = cut || open) { open = !open }.padding(horizontal = 12.dp, vertical = 3.dp).testTag("sub.tool.${e.id}")) {
                Row {
                    Text(when (st) { "running" -> "▸"; "failed" -> "✗"; else -> "✓" }, color = tone, fontFamily = BpMono, fontSize = 12.sp); Spacer(Modifier.width(6.dp))
                    Text(t?.name ?: "tool", color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, fontWeight = FontWeight.Bold); Spacer(Modifier.width(8.dp))
                    Text(t?.summary ?: "", color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
                if (res.isNotEmpty()) Text(if (cut) "$res\n…" else res, color = if (st == "failed") dcs.error else dfaint, fontFamily = BpMono, fontSize = 11.sp, modifier = Modifier.padding(start = 18.dp))
            }
        }
        else -> Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp).testTag("sub.note")) {
            DMono(e.label, dfaint, 11); Spacer(Modifier.width(8.dp)); DMono(e.text, if (e.tone.contains("error")) dcs.error else dcs.onSurfaceVariant, 12, Modifier.weight(1f), lines = 6)
        }
    }
}
