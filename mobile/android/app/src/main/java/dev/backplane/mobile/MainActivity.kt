package dev.backplane.mobile

import android.Manifest
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import android.os.Build

class MainActivity : ComponentActivity() {
    private val model: AppModel by viewModels()
    private val ask = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        intent?.getStringExtra("lang")?.let { model.forceLanguage(it) }
        pairFrom(intent)
        if (savedInstanceState == null) threadFrom(intent)
        setContent {
            // once paired, ask to post alerts
            LaunchedEffect(model.links.isNotEmpty()) { if (model.links.isNotEmpty()) askNotify() }
            Theme(AppSettings.appearance, model.screen?.locale == "sux-Xsux") { App(model) }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        intent.getStringExtra("lang")?.let { model.forceLanguage(it) }
        pairFrom(intent)
        threadFrom(intent)
    }

    // a tapped alert opens its thread; the live status with several opens
    // the app as it was (an older one's "pick" extra is dropped unread)
    private fun threadFrom(intent: Intent?) {
        intent?.removeExtra(Notes.EXTRA_PICK)
        val id = intent?.getStringExtra(Notes.EXTRA_THREAD) ?: return
        intent.removeExtra(Notes.EXTRA_THREAD)
        model.act("select", id)
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
        if (data.scheme == "backplane" && data.host != "google") model.pair(data.toString())
    }
}

// this device's appearance (App settings): light, dark, or the system's
@Composable
fun Theme(theme: String, cuneiform: Boolean, content: @Composable () -> Unit) {
    val dark = when (theme) { "light" -> false; "dark" -> true; else -> isSystemInDarkTheme() }
    val ctx = LocalContext.current
    val scheme = when {
        Build.VERSION.SDK_INT >= 31 -> if (dark) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
        dark -> darkColorScheme()
        else -> lightColorScheme()
    }
    val typography = remember(cuneiform) {
        Typography(defaultFontFamily = if (cuneiform) FontFamily(Typeface.createFromAsset(ctx.assets, "NotoSansCuneiform-Regular.ttf")) else FontFamily.Default)
    }
    MaterialTheme(colorScheme = scheme, typography = typography, content = content)
}
