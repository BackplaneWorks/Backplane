package dev.backplane.mobile

import org.json.JSONArray

/** One entry from the hub's `viewer.rail` reply (`present::rail::rail`, feature `rail`). */
data class ViewerRailEntry(
    val id: String,
    val label: String,
    val group: String,
    val state: String,
    val why: String = "",
)

/** One tab drawn in the phone viewer's horizontal strip. */
data class ViewerRailTab(
    val id: String,
    val tab: String,
    val label: String,
    val disabled: Boolean,
    val on: Boolean,
    val why: String = "",
)

/** Pure phone rail filter, mapping, and active-tab selection (mirrors iOS `ViewerRail.swift`). */
object ViewerRail {
    private val idToTab = mapOf(
        "board" to "Board",
        "schematic" to "Schematic",
        "3d" to "3D",
        "gerbers" to "Gerbers",
        "split" to "Split",
        "mech" to "Mech",
        "files" to "Files",
        "diff" to "Diff",
        "review" to "Review",
        "browser" to "Browser",
    )

    private val tabToId = idToTab.entries.associate { (k, v) -> v to k }

    /** Fallback static strip for hubs without feature `rail` or before `viewer.rail` answers. */
    val fallbackTabs: List<ViewerRailTab> = listOf(
        "board", "schematic", "3d", "split", "mech", "files", "diff", "review", "browser",
    ).map { id ->
        val tab = idToTab.getValue(id)
        ViewerRailTab(id = id, tab = tab, label = tab, disabled = false, on = false)
    }

    fun tabForId(id: String): String? = idToTab[id]

    fun idForTab(tab: String): String = tabToId[tab] ?: tab.lowercase()

    fun parse(arr: JSONArray?): List<ViewerRailEntry> {
        if (arr == null) return emptyList()
        return (0 until arr.length()).mapNotNull { i ->
            val o = arr.optJSONObject(i) ?: return@mapNotNull null
            val id = o.optString("id")
            if (id.isEmpty()) return@mapNotNull null
            ViewerRailEntry(
                id = id,
                label = o.optString("label"),
                group = o.optString("group"),
                state = o.optString("state", "off"),
                why = o.optString("why"),
            )
        }
    }

    /** Filter and map the hub's ordered `viewer.rail` entries to the phone strip.
     *  Preserves hub order, drops `group == "tools"` and unsupported ids (`simulation`, `gerbers` unless supported). */
    fun phoneTabs(entries: List<ViewerRailEntry>, gerbersSupported: Boolean): List<ViewerRailTab> {
        val out = entries.mapNotNull { e ->
            if (e.group == "tools") return@mapNotNull null
            if (e.id == "simulation") return@mapNotNull null
            if (e.id == "gerbers" && !gerbersSupported) return@mapNotNull null
            val tab = idToTab[e.id] ?: return@mapNotNull null
            ViewerRailTab(
                id = e.id,
                tab = tab,
                label = e.label.ifEmpty { tab },
                disabled = e.state == "disabled",
                on = e.state == "on",
                why = e.why,
            )
        }
        return out.ifEmpty { fallbackTabs }
    }

    /** Active tab from the hub's `"on"` entry unless the phone has a newer local press or an explicit file link. */
    fun activeTab(tabs: List<ViewerRailTab>, current: String, userPressed: Boolean, hasFileLink: Boolean): String {
        if (userPressed || hasFileLink) return current
        val hubOn = tabs.firstOrNull { it.on && !it.disabled }?.tab
        return hubOn ?: current
    }
}
