// ThreadGroups against hand-made rows. Compiled with Backplane/ThreadGroups.swift alone, outside the app: scripts/mobile/ios-thread-groups-test.sh.
import Foundation

struct T: ShelvedThread { var id: String; var project = "p"; var settled = false; var pinned = false; var archived = false; var updatedAt = 0; var settledAt = 0 }
var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
func ids(_ ts: [T]) -> [String] { ts.map(\.id) }

let rows = [
    T(id: "a", updatedAt: 10), T(id: "b", updatedAt: 30), T(id: "c", pinned: true, updatedAt: 5),
    T(id: "s1", settled: true, updatedAt: 99, settledAt: 100), T(id: "s2", settled: true, updatedAt: 1, settledAt: 200),
    T(id: "x", project: "q", updatedAt: 50), T(id: "z", archived: true, updatedAt: 60), T(id: "bot", updatedAt: 70),
]
var g = ThreadGroups.split(rows, project: "p", hidden: ["bot"])
check(ids(g.active) == ["c", "b", "a"], "active: pinned first, then newest activity; other projects, archived and hidden left out", ids(g.active))
check(ids(g.settled) == ["s2", "s1"], "settled: newest settle first, whatever their activity", ids(g.settled))

// a thread settled live moves down; an unsettled one moves back up
var live = rows; live[1].settled = true; live[1].settledAt = 300
g = ThreadGroups.split(live, project: "p", hidden: ["bot"])
check(ids(g.active) == ["c", "a"] && ids(g.settled) == ["b", "s2", "s1"], "settled live", (ids(g.active), ids(g.settled)))
live[3].settled = false
g = ThreadGroups.split(live, project: "p", hidden: ["bot"])
check(ids(g.active) == ["c", "s1", "a"] && ids(g.settled) == ["b", "s2"], "unsettled live", (ids(g.active), ids(g.settled)))

// ties are stable; an empty project is empty
check(ids(ThreadGroups.split([T(id: "m", updatedAt: 1), T(id: "k", updatedAt: 1)], project: "p").active) == ["k", "m"], "ties by id", "")
check(ThreadGroups.split(rows, project: "none").active.isEmpty, "no threads", "")
check(ThreadGroups.count(hub: 40, loaded: 3) == 40 && ThreadGroups.count(hub: nil, loaded: 3) == 3 && ThreadGroups.count(hub: 0, loaded: 2) == 2, "count", "")
check(ThreadGroups.title(7) == "Settled (7)", "title", ThreadGroups.title(7))

print("thread groups: \(passed) passed, \(failed) failed"); exit(failed == 0 ? 0 : 1)
