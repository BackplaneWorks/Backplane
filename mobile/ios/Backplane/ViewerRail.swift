import Foundation

// The phone viewer's horizontal strip driven by the hub's `viewer.rail` reply (feature `rail`, docs/PHONE-DATA.md).
// Pure filter, mapping, and state-reconciliation rules (mirrors Android `ViewerRail.kt`).

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

struct DViewerRailReply: Decodable {
    var rail: [DViewerRailEntry]
    enum CodingKeys: String, CodingKey { case rail }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        rail = (try? c.decode([DViewerRailEntry].self, forKey: .rail)) ?? []
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
}

struct ViewerRailReconcileOutcome: Equatable {
    var state: ViewerRailState
    var tab: String
    var stopWatch: Bool
    var accepted: Bool
}

/// Pure phone rail state machine: distinguishes no-feature, loading, failure, and authoritative empty replies;
/// ties replies to (threadId, requestSeq); reconciles active tab when tabs disappear or become disabled.
struct ViewerRailState: Equatable {
    var status: ViewerRailStatus = .noFeature
    var entries: [DViewerRailEntry] = []
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
            next.hasGood = false
            next.threadId = threadId
            next.disabledWhy = ""
        } else if threadId != self.threadId {
            next.status = .loading
            next.entries = []
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
            return ViewerRailTapOutcome(state: next, tab: currentTab, tabChanged: false, shouldWatch: false)
        }
        let changed = item.tab != currentTab
        return ViewerRailTapOutcome(state: onLocalSelect(), tab: item.tab, tabChanged: changed, shouldWatch: changed)
    }

    func onReply(
        _ token: ViewerRailRequest,
        entries: [DViewerRailEntry],
        hasRail: Bool,
        gerbersSupported: Bool,
        currentTab: String,
        hasFileLink: Bool
    ) -> ViewerRailReconcileOutcome {
        guard hasRail, token.threadId == threadId, token.requestSeq == requestSeq else {
            return ViewerRailReconcileOutcome(state: self, tab: currentTab, stopWatch: false, accepted: false)
        }
        let tabs = ViewerRail.phoneTabs(entries, gerbersSupported: gerbersSupported)
        let newerLocal = localSelectionSeq > token.localSelectionSeq
        let currentEntry = tabs.first { $0.tab == currentTab }
        let currentValid = currentEntry != nil && !(currentEntry!.disabled)
        let nextTab: String
        if !currentValid {
            nextTab = tabs.first { $0.on && !$0.disabled }?.tab
                ?? tabs.first { !$0.disabled }?.tab
                ?? ""
        } else if newerLocal || hasFileLink {
            nextTab = currentTab
        } else {
            nextTab = tabs.first { $0.on && !$0.disabled }?.tab ?? currentTab
        }
        var next = self
        next.status = .loaded
        next.entries = entries
        next.hasGood = true
        if !tabs.contains(where: { $0.disabled && $0.why == next.disabledWhy }) { next.disabledWhy = "" }
        return ViewerRailReconcileOutcome(state: next, tab: nextTab, stopWatch: nextTab != currentTab, accepted: true)
    }

    func onFailure(_ token: ViewerRailRequest, hasRail: Bool) -> ViewerRailState {
        guard hasRail, token.threadId == threadId, token.requestSeq == requestSeq else { return self }
        var next = self
        next.status = .failed
        return next
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
