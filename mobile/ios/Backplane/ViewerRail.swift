import Foundation

// The phone viewer's horizontal strip driven by the hub's `viewer.rail` reply (feature `rail`, docs/PHONE-DATA.md).
// Pure filter, mapping, and active-tab rules (mirrors Android `ViewerRail.kt`).

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
    static func phoneTabs(_ entries: [DViewerRailEntry], gerbersSupported: Bool) -> [DViewerRailTab] {
        let out: [DViewerRailTab] = entries.compactMap { e in
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
        return out.isEmpty ? fallbackTabs : out
    }

    /// Active tab from the hub's `"on"` entry unless the phone has a newer local press or an explicit file link.
    static func activeTab(_ tabs: [DViewerRailTab], current: String, userPressed: Bool, hasFileLink: Bool) -> String {
        if userPressed || hasFileLink { return current }
        return tabs.first { $0.on && !$0.disabled }?.tab ?? current
    }
}
