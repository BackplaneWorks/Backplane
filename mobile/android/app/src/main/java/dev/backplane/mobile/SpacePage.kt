package dev.backplane.mobile

import android.graphics.Color
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.viewinterop.AndroidView
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL

// A bot's space as a web page (mobile/bots.bend's "page"): the hub serves
// it at url behind a policy that runs no script and loads nothing from the
// network, and the web view runs no script and blocks the network too. n
// is new when the page is. A tapped link or submitted form goes to Bend as
// ("space-link", bot + "\u001f" + url), which decides what a "space:" one
// sends; web links open in the browser.
data class SpacePageModel(val bot: String, val url: String, val n: String)

@Composable
fun SpacePage(m: AppModel, p: SpacePageModel) {
    val url = m.hubUrl(p.url, "n=" + p.n)
    var html by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(url) {
        if (url == null) return@LaunchedEffect
        val got = withContext(Dispatchers.IO) {
            runCatching {
                val c = URL(url).openConnection() as HttpURLConnection
                c.connectTimeout = 5000
                c.readTimeout = 15000
                try {
                    if (c.responseCode == 200) c.inputStream.use { it.readBytes().toString(Charsets.UTF_8) } else null
                } finally {
                    c.disconnect()
                }
            }.getOrNull()
        }
        if (got != null) html = got
    }
    val uri = LocalUriHandler.current
    val tapped by rememberUpdatedState { u: String ->
        when {
            u.startsWith("space:") -> m.act("space-link", p.bot + "\u001f" + u)
            u.startsWith("http://") || u.startsWith("https://") -> runCatching { uri.openUri(u) }
        }
    }
    val h = html
    if (h == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        return
    }
    AndroidView(
        factory = { ctx -> pageWeb(ctx) { u -> tapped(u) } },
        update = { w ->
            if (w.tag != h) {
                w.tag = h
                w.loadDataWithBaseURL(null, FIXED_VIEWPORT + h, "text/html", "utf-8", null)
            }
        },
        modifier = Modifier.fillMaxSize(),
    )
}

// a viewport that never scales, ahead of the page's own
private const val FIXED_VIEWPORT = "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no\">"

private fun pageWeb(ctx: android.content.Context, tapped: (String) -> Unit): WebView = WebView(ctx).apply {
    setBackgroundColor(Color.TRANSPARENT)
    settings.javaScriptEnabled = false
    // the page never zooms (pinch or double-tap): the app's own text, not a web page's
    settings.setSupportZoom(false)
    settings.builtInZoomControls = false
    settings.displayZoomControls = false
    settings.useWideViewPort = false
    settings.blockNetworkLoads = true
    settings.allowFileAccess = false
    settings.allowContentAccess = false
    settings.domStorageEnabled = false
    settings.setGeolocationEnabled(false)
    webViewClient = object : WebViewClient() {
        // the page itself (and its #anchors) loads; a link or form the
        // person used is a tap; anything else (a refresh the page makes on
        // its own) goes nowhere
        override fun shouldOverrideUrlLoading(v: WebView, r: WebResourceRequest): Boolean {
            val u = r.url.toString()
            if (u.startsWith("about:") || u.startsWith("data:")) return false
            if (r.hasGesture()) tapped(u)
            return true
        }
    }
}
