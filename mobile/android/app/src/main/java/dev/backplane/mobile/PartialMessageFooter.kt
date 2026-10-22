package dev.backplane.mobile

import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable

@Composable
fun PartialMessageFooter(m: AppModel, hub: String, thread: String, entry: String, partial: MessagePartial, live: Boolean = false) {
    if (partial.incomplete) Row {
        val lang = LocalBackplaneLanguage.current
        Text(if (lang == "ja") "出力の一部" else "Partial output", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
        if (partial.hasMore) TextButton(onClick = { m.moreMessage(hub, thread, entry, partial, live) }, enabled = m.contentReady && partial.canRequest) {
            Text(if (lang == "ja") "続きを読む" else "More")
        }
    }
}
