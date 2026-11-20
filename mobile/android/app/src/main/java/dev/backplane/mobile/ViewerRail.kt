package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject

/** One entry from the hub's `viewer.rail` reply (`present::rail::rail`, feature `rail`). */
data class ViewerRailEntry(
    val id: String,
    val label: String,
    val group: String,
    val state: String,
    val why: String = "",
)

/** Authoritative design-source presence from `viewer.rail`'s `has` object (independent of `page`). */
data class ViewerRailHas(
    val board: Boolean,
    val schematic: Boolean,
    val mech: Boolean,
    val gerbers: Boolean,
) {
    fun available(tab: String): Boolean = when (tab) {
        "Board", "3D" -> board
        "Schematic" -> schematic
        "Split" -> board || schematic
        "Mech" -> mech
        "Gerbers" -> gerbers
        else -> true
    }
}

data class ViewerRailReply(
    val entries: List<ViewerRailEntry>,
    val has: ViewerRailHas? = null,
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
    val stopWatch: Boolean,
)

data class ViewerRailReconcileOutcome(
    val state: ViewerRailState,
    val tab: String,
    val stopWatch: Boolean,
    val accepted: Boolean,
    val refetch: Boolean = false,
)

/** Pure phone rail state machine: distinguishes no-feature, loading, failure, and authoritative empty replies;
 *  ties replies to (threadId, requestSeq, localSelectionSeq); reconciles active tab against `has` + entries;
 *  orders watch replacement so stopWatch only runs when moving from a watch-owning tab to a non-watch tab. */
data class ViewerRailState(
    val status: ViewerRailStatus = ViewerRailStatus.NO_FEATURE,
    val entries: List<ViewerRailEntry> = emptyList(),
    val has: ViewerRailHas? = null,
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
                has = null,
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
                has = null,
                hasGood = false,
                threadId = threadId,
                disabledWhy = "",
            )
        } else if (threadId != this.threadId) {
            copy(
                status = ViewerRailStatus.LOADING,
                entries = emptyList(),
                has = null,
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
                stopWatch = false,
            )
        }
        val changed = item.tab != currentTab
        val destWatches = ViewerRail.watchesPlot(item.tab)
        return ViewerRailTapOutcome(
            state = onLocalSelect(),
            tab = item.tab,
            tabChanged = changed,
            shouldWatch = changed && destWatches,
            stopWatch = changed && ViewerRail.watchesPlot(currentTab) && !destWatches,
        )
    }

    fun onReply(
        token: ViewerRailRequest,
        entries: List<ViewerRailEntry>,
        has: ViewerRailHas? = null,
        hasRail: Boolean,
        gerbersSupported: Boolean,
        currentTab: String,
        hasFileLink: Boolean,
    ): ViewerRailReconcileOutcome {
        if (!hasRail || token.threadId != threadId || token.requestSeq != requestSeq) {
            return ViewerRailReconcileOutcome(this, currentTab, stopWatch = false, accepted = false, refetch = false)
        }
        // Drop a reply carrying an older local-selection sequence and request a fresh read instead of reconciling stale page state.
        if (token.localSelectionSeq < localSelectionSeq) {
            return ViewerRailReconcileOutcome(this, currentTab, stopWatch = false, accepted = false, refetch = true)
        }
        val tabs = ViewerRail.phoneTabs(entries, gerbersSupported)
        fun isAvailable(t: ViewerRailTab): Boolean = !t.disabled && (has == null || has.available(t.tab))
        val currentEntry = tabs.firstOrNull { it.tab == currentTab }
        val currentValid = currentEntry != null && isAvailable(currentEntry)
        val nextTab = if (!currentValid) {
            tabs.firstOrNull { it.on && isAvailable(it) }?.tab
                ?: tabs.firstOrNull { isAvailable(it) }?.tab
                ?: ""
        } else if (hasFileLink || localSelectionSeq > 0) {
            currentTab
        } else {
            tabs.firstOrNull { it.on && isAvailable(it) }?.tab ?: currentTab
        }
        val nextState = copy(
            status = ViewerRailStatus.LOADED,
            entries = entries,
            has = has,
            hasGood = true,
            disabledWhy = if (tabs.any { it.disabled && it.why == disabledWhy }) disabledWhy else "",
        )
        // Ordered watch replacement: only explicitly stop when moving from a watch-owning tab to a non-watch tab.
        val needsStop = nextTab != currentTab && ViewerRail.watchesPlot(currentTab) && !ViewerRail.watchesPlot(nextTab)
        return ViewerRailReconcileOutcome(
            state = nextState,
            tab = nextTab,
            stopWatch = needsStop,
            accepted = true,
            refetch = false,
        )
    }

    fun onFailure(token: ViewerRailRequest, hasRail: Boolean): ViewerRailState {
        if (!hasRail || token.threadId != threadId || token.requestSeq != requestSeq) return this
        return copy(status = ViewerRailStatus.FAILED)
    }
}

/** Injectable transport for crossing reducer reconciliation with ordered watch/stop calls in tests. */
interface ViewerRailTransport {
    fun fetchRail(threadId: String, page: String, diff: Boolean): ViewerRailReply?
    fun watch(threadId: String, kind: String)
    fun stopWatch(threadId: String)
}

/** Coordinates `ViewerRailState` with ordered watch replacement over a `ViewerRailTransport`. */
class ViewerRailController(
    private val transport: ViewerRailTransport,
    var state: ViewerRailState = ViewerRailState(),
    var tab: String = "Files",
) {
    fun selectLocal(nextTab: String) {
        val prev = tab
        state = state.onLocalSelect()
        if (nextTab != prev && ViewerRail.watchesPlot(prev) && !ViewerRail.watchesPlot(nextTab)) {
            transport.stopWatch(state.threadId)
        }
        tab = nextTab
        if (nextTab != prev && ViewerRail.watchesPlot(nextTab)) {
            transport.watch(state.threadId, ViewerRail.idForTab(nextTab))
        }
    }

    fun applyReply(
        token: ViewerRailRequest,
        reply: ViewerRailReply,
        hasRail: Boolean,
        gerbersSupported: Boolean,
        hasFileLink: Boolean = false,
    ): ViewerRailReconcileOutcome {
        val prev = tab
        val out = state.onReply(
            token = token,
            entries = reply.entries,
            has = reply.has,
            hasRail = hasRail,
            gerbersSupported = gerbersSupported,
            currentTab = prev,
            hasFileLink = hasFileLink,
        )
        if (out.refetch) {
            val (nextState, nextReq) = state.beginRequest(state.threadId, hasRail = hasRail)
            state = nextState
            val fresh = transport.fetchRail(state.threadId, ViewerRail.idForTab(tab), tab == "Diff")
            if (fresh != null) {
                return applyReply(nextReq, fresh, hasRail, gerbersSupported, hasFileLink)
            }
            state = state.onFailure(nextReq, hasRail)
            return out
        }
        if (!out.accepted) return out
        state = out.state
        if (out.stopWatch) {
            transport.stopWatch(state.threadId)
        }
        tab = out.tab
        if (out.tab != prev && ViewerRail.watchesPlot(out.tab)) {
            transport.watch(state.threadId, ViewerRail.idForTab(out.tab))
        }
        return out
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

    private val watchTabs = setOf("Board", "Schematic", "3D", "Gerbers", "Split", "Mech")

    fun watchesPlot(tab: String): Boolean = tab in watchTabs

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

    fun parseHas(obj: JSONObject?): ViewerRailHas? {
        if (obj == null) return null
        return ViewerRailHas(
            board = obj.optBoolean("board", false),
            schematic = obj.optBoolean("schematic", false),
            mech = obj.optBoolean("mech", false),
            gerbers = obj.optBoolean("gerbers", false),
        )
    }

    fun parseReply(obj: JSONObject): ViewerRailReply =
        ViewerRailReply(entries = parse(obj.optJSONArray("rail")), has = parseHas(obj.optJSONObject("has")))

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
