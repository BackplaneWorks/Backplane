package dev.backplane.mobile

import android.os.SystemClock
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

// The Hub screen (docs/PHONE-DATA.md): native, on plain HTTP, reachable with the socket down. What it shows is HubRescue's; the buttons are the hub's. The same as iOS's HubView.swift.
@Composable
internal fun HubScreen(back: () -> Unit) {
    val scope = rememberCoroutineScope()
    val http = remember(DataClient.link) { DataClient.hubHttp() }
    var fetch by remember { mutableStateOf<HubFetch>(HubFetch.Loading) }
    var asking by remember { mutableStateOf<HubAction?>(null) }
    var sending by remember { mutableStateOf(false) }
    var answer by remember { mutableStateOf("") }
    var progress by remember { mutableStateOf("") }
    var waitFrom by remember { mutableStateOf<Long?>(null) }
    var moved by remember { mutableStateOf(false) }
    val busy = sending || waitFrom != null

    suspend fun load() {
        val f = http?.status() ?: HubFetch.Unreachable
        fetch = f
        (f as? HubFetch.Got)?.status?.notice?.let(DataClient::heardNotice)
    }
    fun send(a: HubAction) {
        asking = null; sending = true; answer = ""; progress = ""
        scope.launch {
            val after = HubRescue.after(http?.control(a.id) ?: HubReply.Unreachable)
            answer = after.text; sending = false
            if (after.poll) { moved = false; waitFrom = SystemClock.elapsedRealtime() }
        }
    }
    LaunchedEffect(Unit) { load() }
    // ends with the screen, pauses with the app; the three minutes keep counting from the send
    LaunchedEffect(waitFrom, DataClient.foreground) {
        val from = waitFrom ?: return@LaunchedEffect
        if (!DataClient.foreground) return@LaunchedEffect
        while (true) {
            delay(HubRescue.POLL_MS)
            val s = HubRescue.step(http?.health(), SystemClock.elapsedRealtime() - from, moved)
            moved = s.moved; progress = s.text
            if (s.done) { load(); waitFrom = null; return@LaunchedEffect }
        }
    }

    asking?.let { a ->
        AlertDialog(onDismissRequest = { asking = null }, shape = RectangleShape,
            title = { Text(a.label + "?", fontFamily = BpMono, fontSize = 15.sp) },
            text = if (a.text.isEmpty()) null else { { Text(a.text, fontFamily = BpMono, fontSize = 13.sp) } },
            confirmButton = { DBtn(a.label, true, modifier = Modifier.testTag("hub.confirm")) { send(a) } },
            dismissButton = { DBtn("Cancel", false, modifier = Modifier.testTag("hub.cancel")) { asking = null } })
    }
    val panel = HubRescue.panel(fetch)
    Column(Modifier.fillMaxSize()) {
        DBar("Hub", back)
        Column(Modifier.padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            DMono(DataClient.hubName, dcs.onSurfaceVariant, 12)
            Text(panel.text, Modifier.testTag("hub.text"), color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp)
            panel.actions.forEach { a -> DBtn(a.label, false, !busy, Modifier.fillMaxWidth().testTag("hub.action.${a.id}")) { asking = a } }
            if (answer.isNotEmpty()) Text(answer, Modifier.testTag("hub.answer"), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp)
            if (progress.isNotEmpty()) Text(progress, Modifier.testTag("hub.progress"), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 13.sp)
            if (fetch != HubFetch.Loading) DBtn("Refresh", false, !busy, Modifier.testTag("hub.refresh")) { fetch = HubFetch.Loading; scope.launch { load() } }
            panel.incidents.forEach { i ->
                DLine()
                DMono(dStamp(i.atMs), dfaint)
                Text(listOf(i.what, i.why, i.did).filter { it.isNotEmpty() }.joinToString(" · "), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp)
            }
        }
    }
}

/** The hub's own word that something is off (hello `notice`), one line; a tap opens the Hub screen. */
@Composable
internal fun HubBanner(open: () -> Unit) {
    val n = DataClient.notice
    if (n.text.isEmpty()) return
    Column(Modifier.testTag("hub.banner").clickable(onClick = open)) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            DDot(if (n.warn) dwarn else dfaint, 8); Spacer(Modifier.width(8.dp))
            Text(n.text, color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        DLine()
    }
}
