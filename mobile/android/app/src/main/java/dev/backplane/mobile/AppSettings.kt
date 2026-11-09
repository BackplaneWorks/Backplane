package dev.backplane.mobile

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

// Settings about this device: kept in SharedPreferences, applied at once,
// never a round trip to a hub. The thread list's view also goes to
// (Backplane.local), which draws the list; nothing here is written to a hub.
object AppSettings {
    private val prefs get() = ctx!!.getSharedPreferences("backplane", Context.MODE_PRIVATE)
    private var ctx: Context? = null

    // "system", "light" or "dark"
    var appearance by mutableStateOf("system")
        private set
    // "auto" (the system's language), "en" or "ja": Tr.lang resolves it
    var lang by mutableStateOf("auto")
        private set
    // "projects" or "active"; empty until the hub's choice was carried over
    var threadList by mutableStateOf("")
        private set
    // a tab's name, or empty: Home with an uberbot, else Threads
    var defaultTab by mutableStateOf("")
        private set
    var haptics by mutableStateOf(true)
        private set
    // the 3D viewers' background (Mech and 3D board): "auto", "light", "dark" or "custom", and the custom "#rrggbb" (ViewerBg)
    var viewerBg by mutableStateOf("auto")
        private set
    var viewerBgCustom by mutableStateOf(ViewerBg.hex(ViewerBg.SWATCHES[0]))
        private set

    fun init(c: Context) {
        if (ctx != null) return
        ctx = c.applicationContext
        lang = prefs.getString("app.lang", "auto") ?: "auto"
        appearance = prefs.getString("app.appearance", "system") ?: "system"
        threadList = prefs.getString("app.threadList", "") ?: ""
        defaultTab = prefs.getString("app.defaultTab", "") ?: ""
        haptics = prefs.getBoolean("app.haptics", true)
        viewerBg = (prefs.getString("app.viewer_bg", "auto") ?: "auto").takeIf { it in ViewerBg.MODES } ?: "auto"
        viewerBgCustom = prefs.getString("app.viewer_bg_custom", null)?.let { ViewerBg.parse(it) }?.let { ViewerBg.hex(it) } ?: viewerBgCustom
    }

    fun putLang(v: String) { lang = v; prefs.edit().putString("app.lang", v).apply() }
    fun alertsOn() = prefs.getBoolean("data.alerts", true)
    fun putAlerts(v: Boolean) { prefs.edit().putBoolean("data.alerts", v).apply() }
    fun putAppearance(v: String) { appearance = v; prefs.edit().putString("app.appearance", v).apply() }
    fun putThreadList(v: String) { threadList = v; prefs.edit().putString("app.threadList", v).apply() }
    fun putDefaultTab(v: String) { defaultTab = v; prefs.edit().putString("app.defaultTab", v).apply() }
    fun putHaptics(v: Boolean) { haptics = v; prefs.edit().putBoolean("app.haptics", v).apply() }
    fun putViewerBg(v: String) { if (v !in ViewerBg.MODES) return; viewerBg = v; prefs.edit().putString("app.viewer_bg", v).apply() }
    /** A custom colour ("#rrggbb"): kept, and the background becomes custom; anything else is ignored. */
    fun putViewerBgCustom(v: String) {
        val c = ViewerBg.parse(v) ?: return
        viewerBgCustom = ViewerBg.hex(c); viewerBg = "custom"
        prefs.edit().putString("app.viewer_bg_custom", viewerBgCustom).putString("app.viewer_bg", "custom").apply()
    }

}
