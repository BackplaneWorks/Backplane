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

// Conversation history (Android: DHistoryTest, the same cases)
let hl = [T(id: "live", project: "scratch")], hr = [T(id: "old1", project: "scratch"), T(id: "live", project: "scratch"), T(id: "old2", project: "scratch")]
check(ids(ThreadGroups.history(live: hl, rows: hr, open: false)) == ["live"], "history shut: the live chat alone", ids(ThreadGroups.history(live: hl, rows: hr, open: false)))
check(ids(ThreadGroups.history(live: hl, rows: hr, open: true)) == ["live", "old1", "old2"], "history open: live first, none twice", ids(ThreadGroups.history(live: hl, rows: hr, open: true)))
check(ThreadGroups.historyTitle(3) == "Conversation history (3)", "history title", ThreadGroups.historyTitle(3))

// subagents and subthreads nest under their parent, wherever it is; a lost parent or a loop makes roots (as ThreadGroupsTest.kt)
struct N: NestedThread { var id: String; var project = "p"; var settled = false; var pinned = false; var archived = false; var updatedAt = 0; var settledAt = 0; var parent = "" }
let nl = [N(id: "a", updatedAt: 9), N(id: "a1", updatedAt: 8, parent: "a"), N(id: "a2", updatedAt: 7, parent: "a"), N(id: "a11", updatedAt: 6, parent: "a1"), N(id: "b", updatedAt: 5),
          N(id: "lost", updatedAt: 4, parent: "gone"), N(id: "c1", updatedAt: 3, parent: "c2"), N(id: "c2", updatedAt: 2, parent: "c1"),
          N(id: "s", settled: true, updatedAt: 1, settledAt: 1), N(id: "s1", updatedAt: 10, parent: "s")]
let ng = ThreadGroups.splitNested(nl, project: "p")
check(ng.active.map(\.id) == ["a", "b", "lost", "c1", "c2"], "a child is in no group of its own", ng.active.map(\.id))
check(ng.settled.map(\.id) == ["s"], "the settled parent", ng.settled.map(\.id))
func nrows(_ roots: [N], _ open: Set<String>) -> [String] { ThreadGroups.tree(roots, all: ng.all, open: open).map { "\($0.t.id):\($0.depth):\($0.kids)" } }
check(nrows(ng.active, []) == ["a:0:2", "b:0:0", "lost:0:0", "c1:0:0", "c2:0:0"], "shut parents hide their children", nrows(ng.active, []))
check(nrows(ng.active, ["a", "a1"]) == ["a:0:2", "a1:1:1", "a11:2:0", "a2:1:0", "b:0:0", "lost:0:0", "c1:0:0", "c2:0:0"], "open parents show them, depth first", nrows(ng.active, ["a", "a1"]))
check(nrows(ng.settled, ["s"]) == ["s:0:1", "s1:1:0"], "an active child of a settled parent is under it", nrows(ng.settled, ["s"]))

print("thread groups: \(passed) passed, \(failed) failed"); exit(failed == 0 ? 0 : 1)
