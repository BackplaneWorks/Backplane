import Foundation

// Which threads sit under a project, and which behind its "Settled (N)" row. Pure: the hub decides what is settled (its `settled` flag and `settledAt`),
// this only splits and orders, so the grouping is one tested function (Tests/ThreadGroupsTests.swift, scripts/mobile/ios-thread-groups-test.sh).
protocol ShelvedThread {
    var id: String { get }
    var project: String { get }
    var settled: Bool { get }
    var pinned: Bool { get }
    var archived: Bool { get }
    var updatedAt: Int { get }
    var settledAt: Int { get }
}

/// A thread that may sit under another (subagents, subthreads: the hub's `parent`).
protocol NestedThread: ShelvedThread { var parent: String { get } }

enum ThreadGroups {
    /// A project's threads, `hidden` (bots' threads) left out: active ones first (pinned, then newest activity), settled ones newest settle first.
    static func split<T: ShelvedThread>(_ all: [T], project: String, hidden: Set<String> = []) -> (active: [T], settled: [T]) {
        let mine = all.filter { $0.project == project && !$0.archived && !hidden.contains($0.id) }
        let active = mine.filter { !$0.settled }.sorted { a, b in
            if a.pinned != b.pinned { return a.pinned }
            return a.updatedAt != b.updatedAt ? a.updatedAt > b.updatedAt : a.id < b.id
        }
        let settled = mine.filter(\.settled).sorted { a, b in a.settledAt != b.settledAt ? a.settledAt > b.settledAt : a.id < b.id }
        return (active, settled)
    }
    /// The number on the row: the hub's count when it sends one (the list may hold only a page), else what is loaded.
    static func count(hub: Int?, loaded: Int) -> Int { max(hub ?? 0, loaded) }
    static func title(_ n: Int) -> String { "Settled (\(n))" }
    /// Conversation history (`history.list`): what shows under the group's row. The live chats always; the rest only while it is open. No row here starts a chat.
    static func history<T: ShelvedThread>(live: [T], rows: [T], open: Bool) -> [T] { live + (open ? rows.filter { r in !live.contains { $0.id == r.id } } : []) }
    static func historyTitle(_ n: Int) -> String { "Conversation history (\(n))" }

    /// The parent a thread sits under: its `parent` when that is among `ids` and the ancestry does not loop (the desktop's Tree.display_parent); else nil, a root.
    static func parent<T: NestedThread>(_ t: T, in ids: [String: T]) -> String? {
        let p = t.parent; if p.isEmpty || p == t.id || ids[p] == nil { return nil }
        var cur = p
        for _ in 0..<64 { if cur == t.id { return nil }; guard let next = ids[cur]?.parent, !next.isEmpty, ids[next] != nil else { return p }; cur = next }
        return nil
    }
    /// `split` for threads that nest: a child whose parent is here is in neither group; it sits under its parent, wherever that is (the desktop's Kids.set).
    static func splitNested<T: NestedThread>(_ all: [T], project: String, hidden: Set<String> = []) -> (active: [T], settled: [T], all: [T]) {
        let mine = all.filter { $0.project == project && !$0.archived && !hidden.contains($0.id) }
        let ids = Dictionary(mine.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        let g = split(mine.filter { parent($0, in: ids) == nil }, project: project)
        return (g.active, g.settled, mine.sorted { a, b in a.updatedAt != b.updatedAt ? a.updatedAt > b.updatedAt : a.id < b.id })
    }
    struct Node<T> { let t: T; let depth: Int; let kids: Int }
    /// `roots` with their children from `all` under them, depth first, children newest first; a parent's children show only while it is in `open`.
    static func tree<T: NestedThread>(_ roots: [T], all: [T], open: Set<String>) -> [Node<T>] {
        let ids = Dictionary(all.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        var kids: [String: [T]] = [:]
        for t in all { if let p = parent(t, in: ids) { kids[p, default: []].append(t) } }
        var out: [Node<T>] = []
        func walk(_ t: T, _ d: Int) { let k = kids[t.id] ?? []; out.append(Node(t: t, depth: d, kids: k.count)); if open.contains(t.id) && d < 16 { for c in k { walk(c, d + 1) } } }
        for r in roots { walk(r, 0) }
        return out
    }
}
