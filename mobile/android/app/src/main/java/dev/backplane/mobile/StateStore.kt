package dev.backplane.mobile

import java.io.File

// client's whole state (Backplane.save()), kept between launches:
// loading it shows every hub's log at once, where folding the log again in
// QuickJS took seconds, and each hub then sends only what came since. The
// file is named for types the state is made of (bridge.js ends
// with their hash, scripts/state-key.py): a new build with the same types
// keeps it, one with other types starts afresh rather than load a state it
// may not read. A bridge.js without the key names it by its whole text.
class StateStore(private val root: File, bridge: String) {
    private val file = File(root, Regex("BackplaneStateKey = \"([0-9a-f]+)\"").find(bridge)?.let { "state-${it.groupValues[1]}.json" }
        ?: "state-%08x-%d.json".format(bridge.hashCode(), bridge.length))

    init {
        // A type-key miss must not erase the prior build's state before a
        // replacement has been saved. The screen cache is independent of it.
        if (!file.exists() && root.listFiles { f -> f.name.startsWith("state-") && f.name != file.name }?.isNotEmpty() == true)
            android.util.Log.i("Backplane", "state-cache miss_type_key")
    }

    // (never read whole into this process: Engine.loadFrom streams it)
    fun has(): Boolean = file.exists() && file.length() > 0

    // the kept state's size on disk (App settings, Cache), and forgetting it
    fun size(): Long = if (file.exists()) file.length() else 0L

    fun clear() {
        file.delete()
    }

    // the hub settings page's last rows, by hub (JSON beside the state)
    val settingsFile = File(root, "hub-settings.json")

    val path: File get() = file

    // written beside it, then put in its place
    val tmp: File get() = File(file.path + ".tmp")

    fun commit() {
        if (tmp.length() > 0) tmp.renameTo(file)
    }
}
