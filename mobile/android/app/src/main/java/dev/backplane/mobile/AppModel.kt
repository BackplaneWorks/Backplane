package dev.backplane.mobile

import android.app.Application
import androidx.lifecycle.AndroidViewModel

// The screen's view of the app's one client (Core): reads its state and
// passes the user's actions through. Holds nothing of its own.
class AppModel(app: Application) : AndroidViewModel(app) {
    private val core = app.core

    val links get() = core.links
    val screen get() = core.screen
    val contentReady get() = core.contentReady
    val startup get() = core.startup
    // the hub's socket is up (for a screen that says online)
    fun linkUp(hub: String) = hub.isEmpty() || core.linkUp[hub] != false
    fun retryView(key: String) = core.retryView(key)
    fun moreMessage(hub: String, thread: String, entry: String, partial: MessagePartial, live: Boolean = false) = core.moreMessage(hub, thread, entry, partial, live)
    val opening get() = core.opening
    val leaving get() = core.leaving
    val seenThreads get() = core.seenThreads
    val seenBots get() = core.seenBots
    val seenPages get() = core.seenPages
    fun open(nav: String, action: String, value: String, title: String, cat: String = "", kind: String = "thread") = core.open(nav, action, value, title, cat, kind)
    fun back() = core.back()
    val tab get() = core.tab
    fun selectTab(t: Tab) = core.selectTab(t)
    fun language(v: String) = core.language(v)
    fun forceLanguage(v: String) = core.forceLanguage(v)
    fun threadList(v: String) = core.threadList(v)
    val hubSettings get() = core.hubSettings
    fun cacheSize() = core.cacheSize()
    fun clearCache() = core.clearCache()
    // the card and the plane to draw: the screen's, else the last it carried
    fun uber(s: Screen): UberCard? = if (s.plane != null) s.uber else if (s.uber != null) core.lastUber ?: s.uber else null
    fun plane(s: Screen): PlaneModel? = s.plane ?: if (s.uber != null) core.lastPlane else null
    val composer get() = core.composer
    val scrolls get() = core.scrolls
    val jump get() = core.jump
    val plots get() = core.plots

    fun pair(text: String) = core.pair(text)
    fun unpair(key: String) = core.unpair(key)
    fun act(action: String, value: String = "") = core.act(action, value)
    fun quiet(action: String, value: String) = core.quiet(action, value)
    fun draft(text: String) = core.draft(text)
    val cats get() = core.cats
    fun cat(key: String) = core.cat(key)
    fun t(key: String, fallback: String) = core.t(key, fallback)
    fun want(key: String) = core.want(key)
    fun field(name: String, text: String) = core.field(name, text)
    fun web(path: String) = core.web(path)
    fun attach(data: ByteArray, name: String) = core.attach(data, name)
    fun hubUrl(path: String, query: String = "", token: Boolean = true) = core.hubUrl(path, query, token)
}
