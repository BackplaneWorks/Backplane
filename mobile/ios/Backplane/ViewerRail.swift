import Foundation

// The phone viewer's horizontal strip driven by the hub's `viewer.rail` reply (feature `rail`, docs/PHONE-DATA.md).
// Pure filter, mapping, state-reconciliation, and ordered watch-replacement rules (mirrors Android `ViewerRail.kt`).

struct DViewerRailEntry: Decodable, Equatable {
    var id: String
    var label: String
    var group: String
    var state: String
    var why: String
    enum CodingKeys: String, CodingKey { case id, label, group, state, why }
    init(id: String, label: String, group: String, state: String, why: String = "") {
        self.id = id; self.label = label; self.group = group; self.state = state; self.why = why
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        func s(_ k: CodingKeys, _ def: String = "") -> String { (try? c.decode(String.self, forKey: k)) ?? def }
        id = s(.id); label = s(.label); group = s(.group); state = s(.state, "off"); why = s(.why)
    }
}

/// Authoritative design-source presence from `viewer.rail`'s `has` object (independent of `page`).
struct DViewerRailHas: Decodable, Equatable {
    var board: Bool
    var schematic: Bool
    var mech: Bool
    var gerbers: Bool
    enum CodingKeys: String, CodingKey { case board, schematic, mech, gerbers }
    init(board: Bool, schematic: Bool, mech: Bool, gerbers: Bool) {
        self.board = board; self.schematic = schematic; self.mech = mech; self.gerbers = gerbers
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        board = (try? c.decode(Bool.self, forKey: .board)) ?? false
        schematic = (try? c.decode(Bool.self, forKey: .schematic)) ?? false
        mech = (try? c.decode(Bool.self, forKey: .mech)) ?? false
        gerbers = (try? c.decode(Bool.self, forKey: .gerbers)) ?? false
    }
    func available(_ tab: String) -> Bool {
        switch tab {
        case "Board", "3D": return board
        case "Schematic": return schematic
        case "Split": return board || schematic
        case "Mech": return mech
        case "Gerbers": return gerbers
        default: return true
        }
    }
}

struct DViewerRailReply: Decodable, Equatable {
    var rail: [DViewerRailEntry]
    var has: DViewerRailHas?
    enum CodingKeys: String, CodingKey { case rail, has }
    init(rail: [DViewerRailEntry], has: DViewerRailHas? = nil) {
        self.rail = rail; self.has = has
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        rail = (try? c.decode([DViewerRailEntry].self, forKey: .rail)) ?? []
        has = try? c.decodeIfPresent(DViewerRailHas.self, forKey: .has)
    }
}

struct DViewerRailTab: Equatable, Identifiable {
    var id: String
    var tab: String
    var label: String
    var disabled: Bool
    var on: Bool
    var why: String
}

enum ViewerRailStatus: Equatable { case noFeature, loading, loaded, failed }

struct ViewerRailRequest: Equatable {
    var threadId: String
    var requestSeq: Int
    var localSelectionSeq: Int
}

struct ViewerRailTapOutcome: Equatable {
    var state: ViewerRailState
    var tab: String
    var tabChanged: Bool
    var shouldWatch: Bool
    var stopWatch: Bool
}

struct ViewerRailReconcileOutcome: Equatable {
    var state: ViewerRailState
    var tab: String
    var stopWatch: Bool
    var accepted: Bool
    var refetch: Bool = false
}

/// Pure phone rail state machine: distinguishes no-feature, loading, failure, and authoritative empty replies;
/// ties replies to (threadId, requestSeq, localSelectionSeq); reconciles active tab against `has` + entries;
/// orders watch replacement so stopWatch only runs when moving from a watch-owning tab to a non-watch tab.
struct ViewerRailState: Equatable {
    var status: ViewerRailStatus = .noFeature
    var entries: [DViewerRailEntry] = []
    var has: DViewerRailHas? = nil
    var hasGood = false
    var threadId = ""
    var requestSeq = 0
    var localSelectionSeq = 0
    var disabledWhy = ""

    func strip(hasRail: Bool, gerbersSupported: Bool) -> [DViewerRailTab] {
        if !hasRail { return ViewerRail.fallbackTabs }
        switch status {
        case .noFeature, .loading:
            return hasGood ? ViewerRail.phoneTabs(entries, gerbersSupported: gerbersSupported) : []
        case .loaded:
            return ViewerRail.phoneTabs(entries, gerbersSupported: gerbersSupported)
        case .failed:
            return hasGood ? ViewerRail.phoneTabs(entries, gerbersSupported: gerbersSupported) : ViewerRail.fallbackTabs
        }
    }

    func onFeatureChanged(hasRail: Bool) -> ViewerRailState {
        var next = self
        if !hasRail {
            next.status = .noFeature
            next.entries = []
            next.has = nil
            next.hasGood = false
            next.requestSeq += 1
            next.disabledWhy = ""
            return next
        }
        if next.status == .noFeature { next.status = .loading }
        return next
    }

    func beginRequest(threadId: String, hasRail: Bool) -> (ViewerRailState, ViewerRailRequest) {
        var next = self
        if !hasRail {
            next.status = .noFeature
            next.entries = []
            next.has = nil
            next.hasGood = false
            next.threadId = threadId
            next.disabledWhy = ""
        } else if threadId != self.threadId {
            next.status = .loading
            next.entries = []
            next.has = nil
            next.hasGood = false
            next.threadId = threadId
            next.localSelectionSeq = 0
            next.disabledWhy = ""
        } else if !next.hasGood {
            next.status = .loading
        }
        next.requestSeq += 1
        let req = ViewerRailRequest(threadId: threadId, requestSeq: next.requestSeq, localSelectionSeq: next.localSelectionSeq)
        return (next, req)
    }

    func onLocalSelect() -> ViewerRailState {
        var next = self
        next.localSelectionSeq += 1
        next.disabledWhy = ""
        return next
    }

    func tap(_ item: DViewerRailTab, currentTab: String) -> ViewerRailTapOutcome {
        if item.disabled {
            var next = self
            next.disabledWhy = item.why
            return ViewerRailTapOutcome(state: next, tab: currentTab, tabChanged: false, shouldWatch: false, stopWatch: false)
        }
        let changed = item.tab != currentTab
        let destWatches = ViewerRail.watchesPlot(item.tab)
        return ViewerRailTapOutcome(
            state: onLocalSelect(),
            tab: item.tab,
            tabChanged: changed,
            shouldWatch: changed && destWatches,
            stopWatch: changed && ViewerRail.watchesPlot(currentTab) && !destWatches
        )
    }

    func onReply(
        _ token: ViewerRailRequest,
        entries: [DViewerRailEntry],
        has: DViewerRailHas? = nil,
        hasRail: Bool,
        gerbersSupported: Bool,
        currentTab: String,
        hasFileLink: Bool
    ) -> ViewerRailReconcileOutcome {
        guard hasRail, token.threadId == threadId, token.requestSeq == requestSeq else {
            return ViewerRailReconcileOutcome(state: self, tab: currentTab, stopWatch: false, accepted: false, refetch: false)
        }
        // Drop a reply carrying an older local-selection sequence and request a fresh read instead of reconciling stale page state.
        if token.localSelectionSeq < localSelectionSeq {
            return ViewerRailReconcileOutcome(state: self, tab: currentTab, stopWatch: false, accepted: false, refetch: true)
        }
        let tabs = ViewerRail.phoneTabs(entries, gerbersSupported: gerbersSupported)
        func isAvailable(_ t: DViewerRailTab) -> Bool { !t.disabled && (has == nil || has!.available(t.tab)) }
        let currentEntry = tabs.first { $0.tab == currentTab }
        let currentValid = currentEntry != nil && isAvailable(currentEntry!)
        let nextTab: String
        if !currentValid {
            nextTab = tabs.first { $0.on && isAvailable($0) }?.tab
                ?? tabs.first { isAvailable($0) }?.tab
                ?? ""
        } else if hasFileLink || localSelectionSeq > 0 {
            nextTab = currentTab
        } else {
            nextTab = tabs.first { $0.on && isAvailable($0) }?.tab ?? currentTab
        }
        var next = self
        next.status = .loaded
        next.entries = entries
        next.has = has
        next.hasGood = true
        if !tabs.contains(where: { $0.disabled && $0.why == next.disabledWhy }) { next.disabledWhy = "" }
        // Ordered watch replacement: only explicitly stop when moving from a watch-owning tab to a non-watch tab.
        let needsStop = nextTab != currentTab && ViewerRail.watchesPlot(currentTab) && !ViewerRail.watchesPlot(nextTab)
        return ViewerRailReconcileOutcome(state: next, tab: nextTab, stopWatch: needsStop, accepted: true, refetch: false)
    }

    func onFailure(_ token: ViewerRailRequest, hasRail: Bool) -> ViewerRailState {
        guard hasRail, token.threadId == threadId, token.requestSeq == requestSeq else { return self }
        var next = self
        next.status = .failed
        return next
    }
}

/// Injectable transport for crossing reducer reconciliation with ordered watch/stop calls in tests.
protocol ViewerRailTransport: AnyObject {
    func fetchRail(threadId: String, page: String, diff: Bool) -> DViewerRailReply?
    func watch(threadId: String, kind: String)
    func stopWatch(threadId: String)
}

/// Coordinates `ViewerRailState` with ordered watch replacement over a `ViewerRailTransport`.
final class ViewerRailController {
    let transport: ViewerRailTransport
    var state: ViewerRailState
    var tab: String
    init(transport: ViewerRailTransport, state: ViewerRailState = ViewerRailState(), tab: String = "Files") {
        self.transport = transport; self.state = state; self.tab = tab
    }

    func selectLocal(_ nextTab: String) {
        let prev = tab
        state = state.onLocalSelect()
        if nextTab != prev && ViewerRail.watchesPlot(prev) && !ViewerRail.watchesPlot(nextTab) {
            transport.stopWatch(threadId: state.threadId)
        }
        tab = nextTab
        if nextTab != prev && ViewerRail.watchesPlot(nextTab) {
            transport.watch(threadId: state.threadId, kind: ViewerRail.idForTab(nextTab))
        }
    }

    @discardableResult
    func applyReply(
        _ token: ViewerRailRequest,
        reply: DViewerRailReply,
        hasRail: Bool,
        gerbersSupported: Bool,
        hasFileLink: Bool = false
    ) -> ViewerRailReconcileOutcome {
        let prev = tab
        let out = state.onReply(
            token,
            entries: reply.rail,
            has: reply.has,
            hasRail: hasRail,
            gerbersSupported: gerbersSupported,
            currentTab: prev,
            hasFileLink: hasFileLink
        )
        if out.refetch {
            let (nextState, nextReq) = state.beginRequest(threadId: state.threadId, hasRail: hasRail)
            state = nextState
            if let fresh = transport.fetchRail(threadId: state.threadId, page: ViewerRail.idForTab(tab), diff: tab == "Diff") {
                return applyReply(nextReq, reply: fresh, hasRail: hasRail, gerbersSupported: gerbersSupported, hasFileLink: hasFileLink)
            }
            state = state.onFailure(nextReq, hasRail: hasRail)
            return out
        }
        guard out.accepted else { return out }
        state = out.state
        if out.stopWatch {
            transport.stopWatch(threadId: state.threadId)
        }
        tab = out.tab
        if out.tab != prev && ViewerRail.watchesPlot(out.tab) {
            transport.watch(threadId: state.threadId, kind: ViewerRail.idForTab(out.tab))
        }
        return out
    }
}

enum ViewerRail {
    private static let idToTab: [String: String] = [
        "board": "Board",
        "schematic": "Schematic",
        "3d": "3D",
        "gerbers": "Gerbers",
        "split": "Split",
        "mech": "Mech",
        "files": "Files",
        "diff": "Diff",
        "review": "Review",
        "browser": "Browser",
    ]
    private static let tabToId: [String: String] = Dictionary(uniqueKeysWithValues: idToTab.map { ($0.value, $0.key) })
    private static let watchTabs: Set<String> = ["Board", "Schematic", "3D", "Gerbers", "Split", "Mech"]

    static func watchesPlot(_ tab: String) -> Bool { watchTabs.contains(tab) }

    static let fallbackTabs: [DViewerRailTab] = [
        "board", "schematic", "3d", "split", "mech", "files", "diff", "review", "browser",
    ].map { id in
        let tab = idToTab[id]!
        return DViewerRailTab(id: id, tab: tab, label: tab, disabled: false, on: false, why: "")
    }

    static func tabForId(_ id: String) -> String? { idToTab[id] }
    static func idForTab(_ tab: String) -> String { tabToId[tab] ?? tab.lowercased() }

    /// Filter and map the hub's ordered `viewer.rail` entries to the phone strip.
    /// Preserves hub order, drops `group == "tools"` and unsupported ids (`simulation`, `gerbers` unless supported).
    /// Does NOT substitute `fallbackTabs` when empty: an authoritative empty reply stays empty.
    static func phoneTabs(_ entries: [DViewerRailEntry], gerbersSupported: Bool) -> [DViewerRailTab] {
        entries.compactMap { e in
            if e.group == "tools" { return nil }
            if e.id == "simulation" { return nil }
            if e.id == "gerbers" && !gerbersSupported { return nil }
            guard let tab = idToTab[e.id] else { return nil }
            return DViewerRailTab(
                id: e.id,
                tab: tab,
                label: e.label.isEmpty ? tab : e.label,
                disabled: e.state == "disabled",
                on: e.state == "on",
                why: e.why
            )
        }
    }
}
