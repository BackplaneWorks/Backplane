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
        // a check without hands (debug builds): --es thread <id> --es vtab 3D opens that thread's viewer on that tab
        if (BuildConfig.DEBUG) DataClient.openViewerTab = intent?.getStringExtra("vtab")
        DataClient.openFromIntent = intent?.getStringExtra("thread")?.ifEmpty { null }
        linked(intent)
        // a check without a hub (debug builds): --es mdpreview /data/local/tmp/x.md [--ez stream true] draws that file as a message
        val preview = if (BuildConfig.DEBUG) intent?.getStringExtra("mdpreview") else null
        val stream = intent?.getBooleanExtra("stream", false) == true
        if (preview != null) { setContent { Theme(AppSettings.appearance) { MdPreview(java.io.File(preview).readText(), stream) } }; return }
        DataClient.pendingAsk = intent?.getStringExtra("ask") ?: ""
        setContent { Theme(AppSettings.appearance) { DataRoot() } }
    }
    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        if (BuildConfig.DEBUG) DataClient.openViewerTab = intent.getStringExtra("vtab")
        intent.getStringExtra("thread")?.takeIf(String::isNotEmpty)?.let { DataClient.openFromIntent = it; DataClient.pendingAsk = intent.getStringExtra("ask") ?: "" }
        linked(intent)
        intent.data?.let { route(it) }
    }
    // an alert's link: "account:<id>" opens that account's sign-in (an expired sign-in's alert) instead of the thread
    private fun linked(i: android.content.Intent?) { i?.getStringExtra("link")?.takeIf { it.startsWith("account:") }?.let { DataClient.openAccount = it.removePrefix("account:"); DataClient.openFromIntent = null } }
    // backplane://thread/<id> opens that thread, backplane://account/<id> that account's sign-in; any other link of this app's scheme asks before it pairs
    private fun route(u: android.net.Uri) {
        if (u.scheme != BuildConfig.URL_SCHEME || u.host == "google") return
        // debug builds only: bpgpui://unpair forgets the hub in this process (a test's fresh start without pm clear or force-stop: on the emulator,
        // every app process that draws and then dies leaves its rendering memory behind on the host; scripts/android-emu.sh fresh)
        if (BuildConfig.DEBUG && u.host == "unpair") { DataClient.unpair(); return }
        // (a thread of another machine, `desktop~t9` or a link id: the hub says which thread it is; this machine's own opens as it did)
        if (u.host == "thread") u.pathSegments.firstOrNull()?.let { if (it.contains('~')) DataClient.openLink(u.toString(), "") else DataClient.openFromIntent = it }
        else if (u.host == "account") u.pathSegments.firstOrNull()?.let { DataClient.openAccount = it } else DataClient.askLink = u.toString()
    }
    override fun onResume() { super.onResume(); DataClient.foreground = true; DataClient.foreground() }
    override fun onPause() { super.onPause(); DataClient.foreground = false }
}
