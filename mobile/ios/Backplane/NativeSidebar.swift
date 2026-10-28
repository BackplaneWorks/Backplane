import Foundation

// This metadata is optional for older hubs. When present it must be valid;
// opaque control values are never used as labels or accessibility IDs.
struct NativeSidebarCount: Decodable {
    let value: Int
    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        value = try c.decode(Int.self)
        guard value >= 0, value <= 9_007_199_254_740_991 else {
            throw DecodingError.dataCorruptedError(in: c, debugDescription: "Invalid sidebar count")
        }
    }
}

struct NativeSidebarControl: Decodable {
    let label, action, value: String
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        label = try c.decode(String.self, forKey: .label)
        action = try c.decode(String.self, forKey: .action)
        value = try c.decode(String.self, forKey: .value)
        guard !label.isEmpty, action == "native-sidebar-page",
              let split = value.firstIndex(of: "|"), split != value.startIndex,
              value.index(after: split) != value.endIndex, value[value.index(after: split)...].utf8.count <= 2048 else {
            throw DecodingError.dataCorruptedError(forKey: .action, in: c, debugDescription: "Invalid sidebar control")
        }
    }
    private enum CodingKeys: String, CodingKey { case label, action, value }
    func belongs(to hub: String) -> Bool { value.hasPrefix(hub + "|") }
}

struct NativeSidebarPager: Decodable {
    let version, total, offset, limit: Int
    let selectedOutside: Bool
    let previous, next: NativeSidebarControl?
    private enum CodingKeys: String, CodingKey { case version, total, offset, limit, selectedOutside, previous, next }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        version = try c.decode(Int.self, forKey: .version)
        total = try c.decode(NativeSidebarCount.self, forKey: .total).value
        offset = try c.decode(NativeSidebarCount.self, forKey: .offset).value
        limit = try c.decode(NativeSidebarCount.self, forKey: .limit).value
        selectedOutside = try c.decode(Bool.self, forKey: .selectedOutside)
        guard c.contains(.previous), c.contains(.next) else {
            throw DecodingError.dataCorruptedError(forKey: .previous, in: c, debugDescription: "Missing sidebar direction")
        }
        previous = try c.decodeIfPresent(NativeSidebarControl.self, forKey: .previous)
        next = try c.decodeIfPresent(NativeSidebarControl.self, forKey: .next)
        guard version == 1, total > 0, offset < total, (1...48).contains(limit),
              (previous != nil) == (offset > 0), (next != nil) == (total - offset > limit) else {
            throw DecodingError.dataCorruptedError(forKey: .version, in: c, debugDescription: "Invalid sidebar pager")
        }
    }
    func belongs(to hub: String) -> Bool {
        !hub.isEmpty && (previous?.belongs(to: hub) ?? true) && (next?.belongs(to: hub) ?? true)
    }
}

struct NativeSidebarHubPage: Decodable {
    let version: Int
    let hub: String
    let projects, active, legacy: NativeSidebarPager?
    private enum CodingKeys: String, CodingKey { case version, hub, projects, active, legacy }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        version = try c.decode(Int.self, forKey: .version)
        hub = try c.decode(String.self, forKey: .hub)
        guard c.contains(.projects), c.contains(.active), c.contains(.legacy) else {
            throw DecodingError.dataCorruptedError(forKey: .projects, in: c, debugDescription: "Missing sidebar list")
        }
        projects = try c.decodeIfPresent(NativeSidebarPager.self, forKey: .projects)
        active = try c.decodeIfPresent(NativeSidebarPager.self, forKey: .active)
        legacy = try c.decodeIfPresent(NativeSidebarPager.self, forKey: .legacy)
        guard version == 1, !hub.isEmpty, !hub.contains("|"),
              [projects, active, legacy].allSatisfy({ $0?.belongs(to: hub) ?? true }) else {
            throw DecodingError.dataCorruptedError(forKey: .hub, in: c, debugDescription: "Invalid sidebar owner")
        }
    }
}

// Only new fields are preflighted so old lossy row handling remains intact.
struct NativeSidebarProjectValidation: Decodable {
    let id: String?
    let settledCount, archivedCount: NativeSidebarCount?
    let threadsPage, snoozedPage, settledPage, archivedPage: NativeSidebarPager?
    private enum CodingKeys: String, CodingKey { case id, settledCount, archivedCount, threadsPage, snoozedPage, settledPage, archivedPage }
    init(from decoder: Decoder) throws {
        guard let c = try? decoder.container(keyedBy: CodingKeys.self) else {
            id = nil; settledCount = nil; archivedCount = nil
            threadsPage = nil; snoozedPage = nil; settledPage = nil; archivedPage = nil
            return
        }
        id = try? c.decodeIfPresent(String.self, forKey: .id)
        let fields: [CodingKeys] = [.settledCount, .archivedCount, .threadsPage, .snoozedPage, .settledPage, .archivedPage]
        if fields.contains(where: { c.contains($0) }), !fields.allSatisfy({ c.contains($0) }) {
            throw DecodingError.dataCorruptedError(forKey: .settledCount, in: c, debugDescription: "Incomplete sidebar project metadata")
        }
        settledCount = c.contains(.settledCount) ? try c.decode(NativeSidebarCount.self, forKey: .settledCount) : nil
        archivedCount = c.contains(.archivedCount) ? try c.decode(NativeSidebarCount.self, forKey: .archivedCount) : nil
        threadsPage = try c.decodeIfPresent(NativeSidebarPager.self, forKey: .threadsPage)
        snoozedPage = try c.decodeIfPresent(NativeSidebarPager.self, forKey: .snoozedPage)
        settledPage = try c.decodeIfPresent(NativeSidebarPager.self, forKey: .settledPage)
        archivedPage = try c.decodeIfPresent(NativeSidebarPager.self, forKey: .archivedPage)
        let pages = [threadsPage, snoozedPage, settledPage, archivedPage].compactMap { $0 }
        if !pages.isEmpty {
            guard let id, let separator = id.firstIndex(of: "|"), separator != id.startIndex,
                  pages.allSatisfy({ $0.belongs(to: String(id[..<separator])) }) else {
                throw DecodingError.dataCorruptedError(forKey: .id, in: c, debugDescription: "Invalid sidebar project owner")
            }
        }
    }
}
