package dev.backplane.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge

// The launcher: the data/1 app (docs/gpui/PHONE-DATA.md). No Bend engine; the hub sends plain data and these screens draw it.
class DataActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        AppSettings.init(this)
        DataClient.init(this)
        intent?.data?.let { if (it.scheme == BuildConfig.URL_SCHEME && it.host != "google") DataClient.pair(it.toString()) }
        if (android.os.Build.VERSION.SDK_INT >= 33 && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 1)
        DataClient.openFromIntent = intent?.getStringExtra("thread")
        setContent { Theme(AppSettings.appearance) { DataRoot() } }
    }
    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        intent.getStringExtra("thread")?.let { DataClient.openFromIntent = it }
        intent.data?.let { if (it.scheme == BuildConfig.URL_SCHEME && it.host != "google") DataClient.pair(it.toString()) }
    }
    override fun onResume() { super.onResume(); DataClient.foreground = true; DataClient.foreground() }
    override fun onPause() { super.onPause(); DataClient.foreground = false }
}
