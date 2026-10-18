package dev.backplane.mobile

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

// The tabs of the app, in the order of the bar.
enum class Tab(val title: String) { Home("Home"), Threads("Threads"), Bots("Bots"), Routines("Routines"), Settings("Settings") }

// Settings about this device: kept in SharedPreferences, applied at once,
// never a round trip to a hub. The thread list's view also goes to Bend
// (Backplane.local), which draws the list; nothing here is written to a hub.
object AppSettings {
    private val prefs get() = ctx!!.getSharedPreferences("backplane", Context.MODE_PRIVATE)
    private var ctx: Context? = null

    // "system", "light" or "dark"
    var appearance by mutableStateOf("system")
        private set
    var lang by mutableStateOf("auto")
        private set
    var langPreview by mutableStateOf(false)
        private set
    // "projects" or "active"; empty until the hub's choice was carried over
    var threadList by mutableStateOf("")
        private set
    // a tab's name, or empty: Home with an uberbot, else Threads
    var defaultTab by mutableStateOf("")
        private set
    var haptics by mutableStateOf(true)
        private set

    fun init(c: Context) {
        if (ctx != null) return
        ctx = c.applicationContext
        lang = prefs.getString("app.lang", "auto") ?: "auto"
        appearance = prefs.getString("app.appearance", "system") ?: "system"
        threadList = prefs.getString("app.threadList", "") ?: ""
        defaultTab = prefs.getString("app.defaultTab", "") ?: ""
        haptics = prefs.getBoolean("app.haptics", true)
    }

    fun putLang(v: String) { lang = v; prefs.edit().putString("app.lang", v).apply() }
    fun forceLang(v: String) { if (v == "ja" || v == "en") { langPreview = true; lang = v } }
    fun putAppearance(v: String) { appearance = v; prefs.edit().putString("app.appearance", v).apply() }
    fun putThreadList(v: String) { threadList = v; prefs.edit().putString("app.threadList", v).apply() }
    fun putDefaultTab(v: String) { defaultTab = v; prefs.edit().putString("app.defaultTab", v).apply() }
    fun putHaptics(v: Boolean) { haptics = v; prefs.edit().putBoolean("app.haptics", v).apply() }

    // The first screen of this build: what the hub had chosen (its theme, its
    // thread list) becomes this device's choice, once. Never written back.
    fun migrate(theme: String, view: String) {
        if (prefs.getBoolean("app.migrated.v1", false)) return
        prefs.edit().putBoolean("app.migrated.v1", true).apply()
        if (theme == "light" || theme == "dark") putAppearance(theme)
        if (view == "projects" || view == "active") putThreadList(view)
    }
}
