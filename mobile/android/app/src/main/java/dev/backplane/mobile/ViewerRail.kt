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

enum class ViewerRailStatus { NO_FEATURE, LOADING, LOADED, FAILED }

data class ViewerRailRequest(
    val threadId: String,
    val requestSeq: Int,
    val localSelectionSeq: Int,
)

data class ViewerRailTapOutcome(
    val state: ViewerRailState,
    val tab: String,
    val tabChanged: Boolean,
    val shouldWatch: Boolean,
)

data class ViewerRailReconcileOutcome(
    val state: ViewerRailState,
    val tab: String,
    val stopWatch: Boolean,
    val accepted: Boolean,
)

/** Pure phone rail state machine: distinguishes no-feature, loading, failure, and authoritative empty replies;
 *  ties replies to (threadId, requestSeq); reconciles active tab when tabs disappear or become disabled. */
data class ViewerRailState(
    val status: ViewerRailStatus = ViewerRailStatus.NO_FEATURE,
    val entries: List<ViewerRailEntry> = emptyList(),
    val hasGood: Boolean = false,
    val threadId: String = "",
    val requestSeq: Int = 0,
    val localSelectionSeq: Int = 0,
    val disabledWhy: String = "",
) {
    fun strip(hasRail: Boolean, gerbersSupported: Boolean): List<ViewerRailTab> {
        if (!hasRail) return ViewerRail.fallbackTabs
        return when (status) {
            ViewerRailStatus.NO_FEATURE, ViewerRailStatus.LOADING ->
                if (hasGood) ViewerRail.phoneTabs(entries, gerbersSupported) else emptyList()
            ViewerRailStatus.LOADED -> ViewerRail.phoneTabs(entries, gerbersSupported)
            ViewerRailStatus.FAILED ->
                if (hasGood) ViewerRail.phoneTabs(entries, gerbersSupported) else ViewerRail.fallbackTabs
        }
    }

    fun onFeatureChanged(hasRail: Boolean): ViewerRailState {
        if (!hasRail) {
            return copy(
                status = ViewerRailStatus.NO_FEATURE,
                entries = emptyList(),
                hasGood = false,
                requestSeq = requestSeq + 1,
                disabledWhy = "",
            )
        }
        return if (status == ViewerRailStatus.NO_FEATURE) copy(status = ViewerRailStatus.LOADING) else this
    }

    fun beginRequest(threadId: String, hasRail: Boolean): Pair<ViewerRailState, ViewerRailRequest> {
        val base = if (!hasRail) {
            copy(
                status = ViewerRailStatus.NO_FEATURE,
                entries = emptyList(),
                hasGood = false,
                threadId = threadId,
                disabledWhy = "",
            )
        } else if (threadId != this.threadId) {
            copy(
                status = ViewerRailStatus.LOADING,
                entries = emptyList(),
                hasGood = false,
                threadId = threadId,
                localSelectionSeq = 0,
                disabledWhy = "",
            )
        } else {
            copy(status = if (hasGood) status else ViewerRailStatus.LOADING)
        }
        val nextSeq = base.requestSeq + 1
        val next = base.copy(requestSeq = nextSeq)
        return next to ViewerRailRequest(threadId = threadId, requestSeq = nextSeq, localSelectionSeq = next.localSelectionSeq)
    }

    fun onLocalSelect(): ViewerRailState = copy(localSelectionSeq = localSelectionSeq + 1, disabledWhy = "")

    fun tap(item: ViewerRailTab, currentTab: String): ViewerRailTapOutcome {
        if (item.disabled) {
            return ViewerRailTapOutcome(
                state = copy(disabledWhy = item.why),
                tab = currentTab,
                tabChanged = false,
                shouldWatch = false,
            )
        }
        val changed = item.tab != currentTab
        return ViewerRailTapOutcome(
            state = onLocalSelect(),
            tab = item.tab,
            tabChanged = changed,
            shouldWatch = changed,
        )
    }

    fun onReply(
        token: ViewerRailRequest,
        entries: List<ViewerRailEntry>,
        hasRail: Boolean,
        gerbersSupported: Boolean,
        currentTab: String,
        hasFileLink: Boolean,
    ): ViewerRailReconcileOutcome {
        if (!hasRail || token.threadId != threadId || token.requestSeq != requestSeq) {
            return ViewerRailReconcileOutcome(this, currentTab, stopWatch = false, accepted = false)
        }
        val tabs = ViewerRail.phoneTabs(entries, gerbersSupported)
        val newerLocal = localSelectionSeq > token.localSelectionSeq
        val currentEntry = tabs.firstOrNull { it.tab == currentTab }
        val currentValid = currentEntry != null && !currentEntry.disabled
        val nextTab = if (!currentValid) {
            tabs.firstOrNull { it.on && !it.disabled }?.tab
                ?: tabs.firstOrNull { !it.disabled }?.tab
                ?: ""
        } else if (newerLocal || hasFileLink) {
            currentTab
        } else {
            tabs.firstOrNull { it.on && !it.disabled }?.tab ?: currentTab
        }
        val nextState = copy(
            status = ViewerRailStatus.LOADED,
            entries = entries,
            hasGood = true,
            disabledWhy = if (tabs.any { it.disabled && it.why == disabledWhy }) disabledWhy else "",
        )
        return ViewerRailReconcileOutcome(
            state = nextState,
            tab = nextTab,
            stopWatch = nextTab != currentTab,
            accepted = true,
        )
    }

    fun onFailure(token: ViewerRailRequest, hasRail: Boolean): ViewerRailState {
        if (!hasRail || token.threadId != threadId || token.requestSeq != requestSeq) return this
        return copy(status = ViewerRailStatus.FAILED)
    }
}

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

    /** Fallback static strip for hubs without feature `rail` (or failed request with no prior good list). */
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
     *  Preserves hub order, drops `group == "tools"` and unsupported ids (`simulation`, `gerbers` unless supported).
     *  Does NOT substitute `fallbackTabs` when empty: an authoritative empty reply stays empty. */
    fun phoneTabs(entries: List<ViewerRailEntry>, gerbersSupported: Boolean): List<ViewerRailTab> =
        entries.mapNotNull { e ->
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
}
