package dev.backplane.mobile

import android.Manifest
import android.content.Intent
import android.content.res.Configuration
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import android.os.Build

// Which screen: the hub's page in the WebView shell (WebShell.kt), or the
// app's own native screens while nothing is paired (the pairing screen),
// while a hub refuses this phone (re-pair), or with Classic screens on.
class MainActivity : ComponentActivity() {
    private val model: AppModel by viewModels()
    private val ask = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}
    private val shell by lazy { WebShell(this, model) }
    private var web = false
    private var awayAt = 0L

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        if (intent?.getBooleanExtra("webdebug", false) == true) WebShell.debug = true
        shell // its result launchers must be registered before the activity starts
        intent?.getStringExtra("lang")?.let { model.forceLanguage(it) }
        pairFrom(intent)
        if (savedInstanceState == null) { threadFrom(intent); sharedFrom(intent); settingsFrom(intent) }
        // the page's Back first (the classic screens have their own BackHandlers)
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (web) shell.back()
                else { isEnabled = false; onBackPressedDispatcher.onBackPressed(); isEnabled = true }
            }
        })
        setContent {
            // once paired, ask to post alerts
            LaunchedEffect(model.links.isNotEmpty()) { if (model.links.isNotEmpty()) askNotify() }
            Theme(AppSettings.appearance) {
                val refused = model.links.any { model.startup[Pairing.key(it)] == StartupPolicy.PAIRING_REFUSED }
                val showWeb = !AppSettings.classic && model.links.isNotEmpty() && !refused
                SideEffect { web = showWeb; core.webMode = showWeb }
                if (showWeb) WebShellScreen(shell, model) else App(model)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        intent.getStringExtra("lang")?.let { model.forceLanguage(it) }
        pairFrom(intent)
        threadFrom(intent)
        sharedFrom(intent)
        settingsFrom(intent)
    }

    override fun onPause() {
        super.onPause()
        awayAt = android.os.SystemClock.elapsedRealtime()
        if (web) shell.paused()
    }

    override fun onResume() {
        super.onResume()
        if (web && awayAt > 0) shell.resumed(android.os.SystemClock.elapsedRealtime() - awayAt)
        awayAt = 0L
    }

    // rotation, the keyboard, dark mode and the text size keep the page
    // (the manifest's configChanges); only the text zoom follows
    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        shell.textZoom()
    }

    override fun onDestroy() {
        shell.destroy()
        super.onDestroy()
    }

    // a tapped alert opens its thread (ids are "host:port|thread"); the live
    // status with several opens the app as it was (an older one's "pick"
    // extra is dropped unread)
    private fun threadFrom(intent: Intent?) {
        intent?.removeExtra(Notes.EXTRA_PICK)
        val id = intent?.getStringExtra(Notes.EXTRA_THREAD) ?: return
        intent.removeExtra(Notes.EXTRA_THREAD)
        if (!AppSettings.classic && model.links.isNotEmpty()) {
            val key = id.substringBefore('|', "")
            val known = model.links.any { Pairing.key(it) == key }
            shell.openThread(if (known) key else "", if (known) id.substringAfter('|') else id)
        } else model.act("select", id)
    }

    // something shared into the app (text, a photo, a file) goes to the page's composer
    private fun sharedFrom(intent: Intent?) {
        if (intent?.action != Intent.ACTION_SEND && intent?.action != Intent.ACTION_SEND_MULTIPLE) return
        if (AppSettings.classic || model.links.isEmpty()) return
        shell.shareIn(intent)
        intent.action = Intent.ACTION_MAIN
    }

    // the launcher's "App settings" shortcut
    private fun settingsFrom(intent: Intent?) {
        if (intent?.getBooleanExtra("app-settings", false) != true) return
        intent.removeExtra("app-settings")
        // (the classic screens have Classic screens in their Settings tab)
        if (!AppSettings.classic) shell.over = WebShell.Over.Settings
    }

    private fun askNotify() {
        if (Build.VERSION.SDK_INT < 33 || Notes.allowed(this)) return
        val prefs = getSharedPreferences("backplane", MODE_PRIVATE)
        if (prefs.getBoolean("asked-notify", false)) return
        prefs.edit().putBoolean("asked-notify", true).apply()
        ask.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    // backplane://pair?url=... pairs with that hub too
    private fun pairFrom(intent: Intent?) {
        val data = intent?.data ?: return
        // (backplane://google ends a Google sign-in: GoogleLoop)
        if (data.scheme == BuildConfig.URL_SCHEME && data.host != "google") model.pair(data.toString())
    }
}
