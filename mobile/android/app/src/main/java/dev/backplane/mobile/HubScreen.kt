package dev.backplane.mobile

import android.os.SystemClock
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Star
import androidx.compose.foundation.border
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.repeatOnLifecycle
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
            title = { Text(Tr.t(a.label) + if (Tr.lang == "ja") "？" else "?", fontFamily = BpMono, fontSize = 15.sp) },
            text = if (a.text.isEmpty()) null else { { Text(Tr.t(a.text), fontFamily = BpMono, fontSize = 13.sp) } },
            confirmButton = { DBtn(Tr.t(a.label), true, modifier = Modifier.testTag("hub.confirm")) { send(a) } },
            dismissButton = { DBtn(Msg.common_cancel, false, modifier = Modifier.testTag("hub.cancel")) { asking = null } })
    }
    val panel = HubRescue.panel(fetch)
    Column(Modifier.fillMaxSize()) {
        DBar(Msg.android_hub, back)
        Column(Modifier.padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            DMono(DataClient.hubName, dcs.onSurfaceVariant, 12)
            Text(Tr.t(panel.text), Modifier.testTag("hub.text"), color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp)
            panel.actions.forEach { a -> DBtn(Tr.t(a.label), false, !busy, Modifier.fillMaxWidth().testTag("hub.action.${a.id}")) { asking = a } }
            if (answer.isNotEmpty()) Text(Tr.t(answer), Modifier.testTag("hub.answer"), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp)
            if (progress.isNotEmpty()) Text(Tr.t(progress), Modifier.testTag("hub.progress"), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp)
            if (fetch != HubFetch.Loading) DBtn(Msg.common_refresh, false, !busy, Modifier.testTag("hub.refresh")) { fetch = HubFetch.Loading; scope.launch { load() } }
            panel.incidents.forEach { i ->
                DLine()
                DMono(dStamp(i.atMs), dfaint)
                Text(listOf(i.what, i.why, i.did).filter { it.isNotEmpty() }.joinToString(" · ") { Tr.t(it) }, color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp)
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
            Text(Tr.t(n.text), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        DLine()
    }
}

/** Alerts, asked for once the person sees why (Android 13+): one line, Allow alerts shows the system's question, Not now hides it until the next launch.
 *  Shown once paired and online, so it sits over Bub's chat or the list, never over the pairing screen. */
@Composable
internal fun AlertsBanner() {
    val act = LocalContext.current as? android.app.Activity ?: return
    if (!DataClient.alertsAsk || DataClient.conn != DataClient.Conn.ONLINE) return
    Column(Modifier.testTag("alerts.banner")) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(Msg.android_alerts_ask, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.width(8.dp))
            // a filled primary button, as iOS's DButton(primary: true)
            DBtn(Msg.android_alerts_allow, true, true, Modifier.testTag("alerts.allow")) {
                DataClient.alertsAsk = false
                if (android.os.Build.VERSION.SDK_INT >= 33) act.requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 1)
            }
            Spacer(Modifier.width(4.dp))
            DMono(Msg.android_alerts_later, dcs.onSurfaceVariant, 12, Modifier.testTag("alerts.later").clickable { DataClient.alertsAsk = false }.padding(horizontal = 4.dp, vertical = 4.dp))
        }
        DLine()
    }
}

/** The one-time telemetry notice (DataClient.telemetryNotice), one line: Turn off turns telemetry off, Settings opens the app's Settings, ✕ closes it.
 *  Shown sends "shown" once it is drawn (DataClient.telemetryShown). */
@Composable
internal fun TelemetryBanner(settings: () -> Unit) {
    val n = DataClient.telemetryNotice ?: return
    // "shown" only once it is really on screen: the app started (not in the background), the strip composed
    val owner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    LaunchedEffect(n.token, owner) { owner.lifecycle.repeatOnLifecycle(androidx.lifecycle.Lifecycle.State.STARTED) { DataClient.telemetryShown(n.token) } }
    Column(Modifier.testTag("telemetry.banner")) {
        Row(Modifier.fillMaxWidth().background(dcs.surfaceContainerHigh).padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(Tr.t(n.text), Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp, maxLines = 3, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.width(8.dp))
            DMono(Tr.t(n.off.ifEmpty { "Turn off" }), dcs.primary, 12, Modifier.testTag("telemetry.off").clickable { DataClient.telemetryAct("off") }.padding(horizontal = 4.dp, vertical = 4.dp), bold = true)
            DMono(Tr.t(n.settings.ifEmpty { "Settings" }), dcs.onSurfaceVariant, 12, Modifier.testTag("telemetry.settings").clickable(onClick = settings).padding(horizontal = 4.dp, vertical = 4.dp))
            Box(Modifier.size(28.dp).clickable { DataClient.telemetryAct("close") }.testTag("telemetry.close"), contentAlignment = Alignment.Center) { Icon(Icons.Default.Close, Msg.common_close, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)) }
        }
        DLine()
    }
}

/** The GitHub star suggestion (DataClient.star): a small card under the top bar with the hub's title, the repository, why stars help, a Star on
 *  GitHub (opens the page, decides open), Later (a week, decides later) and Don't ask again (decides dismiss). It waits for a pause (DStarOffer.due). */
@Composable
internal fun StarBanner() {
    val s = DataClient.star ?: return
    if (!DStarOffer.due(s, DataClient.threads.any { it.state == "working" || it.state == "asking" }, DataClient.typing())) return
    val ctx = LocalContext.current
    run {
        Column(Modifier.fillMaxWidth().padding(8.dp).background(dcs.surfaceContainerHigh).border(androidx.compose.foundation.BorderStroke(1.dp, dcs.primary)).padding(12.dp).testTag("star.banner"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Default.Star, null, tint = dcs.primary, modifier = Modifier.size(16.dp)); Spacer(Modifier.width(8.dp))
                Text(Tr.t(s.text), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Bold)
            }
            DMono(s.place, dfaint, 11)
            if (s.why.isNotEmpty()) Text(Tr.t(s.why), color = dcs.onSurfaceVariant, fontFamily = BpMono, fontSize = 12.sp)
            Row(Modifier.fillMaxWidth().height(36.dp).background(dcs.primary).testTag("star.open").clickable {
                runCatching { ctx.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(s.url)).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)) }
                DataClient.starDecide("open")
            }, horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Default.Star, null, tint = dcs.onPrimary, modifier = Modifier.size(14.dp)); Spacer(Modifier.width(6.dp))
                Text(Tr.t(s.label.ifEmpty { "Star on GitHub" }), color = dcs.onPrimary, fontFamily = BpMono, fontSize = 13.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Bold)
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                DBtn(Tr.t("Later"), false, modifier = Modifier.weight(1f).testTag("star.later")) { DataClient.starDecide("later") }
                DBtn(Tr.t("Don't ask again"), false, modifier = Modifier.weight(1f).testTag("star.dismiss")) { DataClient.starDecide("dismiss") }
            }
        }
    }
}
