package dev.backplane.mobile

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import kotlinx.coroutines.delay
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun ViewStartupBanner(m: AppModel) {
    val hub = m.screen?.hub.orEmpty()
    val key = if (hub.isNotEmpty()) hub.takeIf { m.startup[it] == StartupPolicy.COMPATIBILITY_REQUIRED }
        else m.startup.keys.sorted().firstOrNull { m.startup[it] == StartupPolicy.COMPATIBILITY_REQUIRED }
    if (key == null) ReconnectLine(m, hub)
    if (key != null) Surface {
        Column(Modifier.fillMaxWidth().padding(10.dp)) {
            // a hub without views (older than 0.12.0): it needs an update;
            // the app never downloads a hub's whole history
            Text("This hub needs an update · $key", style = MaterialTheme.typography.labelSmall)
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                TextButton(onClick = { m.retryView(key) }) { Text("Retry") }
            }
        }
    }
}

// The shown hub's link while it is down ("Connecting…"), on every screen.
// Shown only once it has been down past 1.2 s, so a reconnect that lands at
// once (a deploy's quick restart) never flashes.
@Composable
private fun ReconnectLine(m: AppModel, hub: String) {
    val down = hub.isNotEmpty() && !m.linkUp(hub)
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(down) {
        if (!down) { shown = false; return@LaunchedEffect }
        delay(1200)
        shown = true
    }
    if (shown && down) Surface {
        Text(NativeMessages.native_thread_online(m.screen?.locale ?: "en", "connecting"),
            Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.statusBars).padding(horizontal = 12.dp, vertical = 6.dp),
            style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline)
    }
}
