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
}
