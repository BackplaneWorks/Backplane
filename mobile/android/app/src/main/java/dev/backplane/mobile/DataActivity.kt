package dev.backplane.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge

// The launcher: the data/1 app (docs/PHONE-DATA.md). No engine; the hub sends plain data and these screens draw it.
class DataActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        AppSettings.init(this)
        DataClient.init(this)
        intent?.data?.let { route(it) }
        if (android.os.Build.VERSION.SDK_INT >= 33 && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 1)
        DataClient.openFromIntent = intent?.getStringExtra("thread")
        // a check without a hub (debug builds): --es mdpreview /data/local/tmp/x.md [--ez stream true] draws that file as a message
        val preview = if (BuildConfig.DEBUG) intent?.getStringExtra("mdpreview") else null
        val stream = intent?.getBooleanExtra("stream", false) == true
        if (preview != null) { setContent { Theme(AppSettings.appearance) { MdPreview(java.io.File(preview).readText(), stream) } }; return }
        DataClient.pendingAsk = intent?.getStringExtra("ask") ?: ""
        setContent { Theme(AppSettings.appearance) { DataRoot() } }
    }
    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        intent.getStringExtra("thread")?.let { DataClient.openFromIntent = it; DataClient.pendingAsk = intent.getStringExtra("ask") ?: "" }
        intent.data?.let { route(it) }
    }
    // backplane://thread/<id> opens that thread; any other link of this app's scheme asks before it pairs
    private fun route(u: android.net.Uri) {
        if (u.scheme != BuildConfig.URL_SCHEME || u.host == "google") return
        if (u.host == "thread") u.pathSegments.firstOrNull()?.let { DataClient.openFromIntent = it } else DataClient.askLink = u.toString()
    }
    override fun onResume() { super.onResume(); DataClient.foreground = true; DataClient.foreground() }
    override fun onPause() { super.onPause(); DataClient.foreground = false }
}
