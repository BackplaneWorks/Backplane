// ViewerRail (Backplane/ViewerRail.swift) against the hub's `viewer.rail` reply (feature `rail`).
// Compiled with ViewerRail.swift alone, outside the app: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

let json = """
{"rail":[
  {"id":"files","label":"Files","group":"work","state":"off"},
  {"id":"board","label":"Board","group":"design","state":"disabled","why":"No board in this project"},
  {"id":"schematic","label":"Schematic","group":"design","state":"on"},
  {"id":"gerbers","label":"Gerbers","group":"design","state":"off"},
  {"id":"simulation","label":"Simulation","group":"design","state":"off"},
  {"id":"terminal","label":"Terminal","group":"tools","state":"off"},
  {"id":"commands","label":"Commands","group":"tools","state":"off"},
  {"id":"unknown","label":"Unknown","group":"work","state":"off"}
],"has":{"board":false,"schematic":true,"mech":false,"gerbers":true},"mode":"viewer"}
"""

let reply = try! JSONDecoder().decode(DViewerRailReply.self, from: Data(json.utf8))
let noGerb = ViewerRail.phoneTabs(reply.rail, gerbersSupported: false)
check(noGerb.map(\.tab) == ["Files", "Board", "Schematic"], "hub order preserved, tools/sim/gerbers/unknown dropped", noGerb.map(\.tab))
check(noGerb.map(\.label) == ["Files", "Board", "Schematic"], "hub labels preserved", noGerb.map(\.label))
check(noGerb[1].disabled && noGerb[1].why == "No board in this project", "disabled entry keeps why", noGerb[1])
check(noGerb[2].on && !noGerb[2].disabled, "on entry lit", noGerb[2])
check(reply.has == DViewerRailHas(board: false, schematic: true, mech: false, gerbers: true), "decodes has object", reply.has as Any)

let withGerb = ViewerRail.phoneTabs(reply.rail, gerbersSupported: true)
check(withGerb.map(\.tab) == ["Files", "Board", "Schematic", "Gerbers"], "gerbers kept when feature present", withGerb.map(\.tab))

// Authoritative empty list stays empty (never falls back to static strip)
check(ViewerRail.phoneTabs([], gerbersSupported: false).isEmpty, "empty phoneTabs stays empty")
check(ViewerRail.phoneTabs([
    DViewerRailEntry(id: "terminal", label: "Terminal", group: "tools", state: "off"),
    DViewerRailEntry(id: "simulation", label: "Simulation", group: "design", state: "off"),
], gerbersSupported: false).isEmpty, "only tools/simulation stays empty")

let staticTabs = ["Board", "Schematic", "3D", "Split", "Mech", "Files", "Diff", "Review", "Browser"]
let noFeat = ViewerRailState(threadId: "t1")
check(noFeat.strip(hasRail: false, gerbersSupported: false).map(\.tab) == staticTabs, "no feature uses static strip")

let (loading, req1) = noFeat.beginRequest(threadId: "t1", hasRail: true)
check(loading.strip(hasRail: true, gerbersSupported: false).isEmpty, "loading before first reply is empty")

let failedNoGood = loading.onFailure(req1, hasRail: true)
check(failedNoGood.strip(hasRail: true, gerbersSupported: false).map(\.tab) == staticTabs, "failure with no prior good list falls back")

let (loading2, req2) = failedNoGood.beginRequest(threadId: "t1", hasRail: true)
let emptyOut = loading2.onReply(req2, entries: [], hasRail: true, gerbersSupported: false, currentTab: "Board", hasFileLink: false)
check(emptyOut.accepted && emptyOut.tab == "" && emptyOut.stopWatch, "authoritative empty reply clears tab and stops watch", emptyOut)
check(emptyOut.state.strip(hasRail: true, gerbersSupported: false).isEmpty, "authoritative empty reply keeps empty strip")

let good = [
    DViewerRailEntry(id: "files", label: "Files", group: "work", state: "on"),
    DViewerRailEntry(id: "diff", label: "Diff", group: "work", state: "off"),
]
let (loading3, req3) = emptyOut.state.beginRequest(threadId: "t1", hasRail: true)
let goodOut = loading3.onReply(req3, entries: good, hasRail: true, gerbersSupported: false, currentTab: "", hasFileLink: false)
check(goodOut.state.strip(hasRail: true, gerbersSupported: false).map(\.tab) == ["Files", "Diff"], "loaded good list")
check(goodOut.tab == "Files", "selects hub on entry")

let (loading4, req4) = goodOut.state.beginRequest(threadId: "t1", hasRail: true)
let failedAfterGood = loading4.onFailure(req4, hasRail: true)
check(failedAfterGood.strip(hasRail: true, gerbersSupported: false).map(\.tab) == ["Files", "Diff"], "failure keeps last good list")

// Reconciles against `has` even when removed Gerbers entry is marked "on"
let gerbRemoved = [
    DViewerRailEntry(id: "gerbers", label: "Gerbers", group: "design", state: "on"),
    DViewerRailEntry(id: "files", label: "Files", group: "work", state: "off"),
]
let (sg, rg) = ViewerRailState(threadId: "t1").beginRequest(threadId: "t1", hasRail: true)
let gerbOut = sg.onReply(rg, entries: gerbRemoved, has: DViewerRailHas(board: false, schematic: false, mech: false, gerbers: false), hasRail: true, gerbersSupported: true, currentTab: "Gerbers", hasFileLink: false)
check(gerbOut.accepted && gerbOut.tab == "Files" && gerbOut.stopWatch, "reconciles away from removed Gerbers via has", gerbOut)

// Drops reply carrying older localSelectionSeq and requests refetch
let entries = [
    DViewerRailEntry(id: "board", label: "Board", group: "design", state: "disabled", why: "No board"),
    DViewerRailEntry(id: "schematic", label: "Schematic", group: "design", state: "on"),
    DViewerRailEntry(id: "files", label: "Files", group: "work", state: "off"),
]
let (s1, r1) = ViewerRailState(threadId: "t1").beginRequest(threadId: "t1", hasRail: true)
let afterLocal = s1.onLocalSelect()
let staleLocal = afterLocal.onReply(r1, entries: entries, hasRail: true, gerbersSupported: false, currentTab: "Board", hasFileLink: false)
check(!staleLocal.accepted && staleLocal.refetch && !staleLocal.stopWatch && staleLocal.tab == "Board", "older localSelectionSeq dropped with refetch", staleLocal)

let (s2, r2) = afterLocal.beginRequest(threadId: "t1", hasRail: true)
let fresh = s2.onReply(r2, entries: entries, hasRail: true, gerbersSupported: false, currentTab: "Board", hasFileLink: false)
check(fresh.accepted && !fresh.refetch && fresh.tab == "Schematic" && !fresh.stopWatch, "watch-to-watch reconcile skips stopWatch", fresh)

// Stale request seq, stale thread, and feature loss dropped
let (s3, r3) = fresh.state.beginRequest(threadId: "t1", hasRail: true)
let (s4, r4) = s3.beginRequest(threadId: "t1", hasRail: true)
check(!s4.onReply(r3, entries: [], hasRail: true, gerbersSupported: false, currentTab: "Schematic", hasFileLink: false).accepted, "older request id dropped")

let (otherThread, _) = s4.beginRequest(threadId: "t2", hasRail: true)
check(!otherThread.onReply(r4, entries: entries, hasRail: true, gerbersSupported: false, currentTab: "Schematic", hasFileLink: false).accepted, "other thread late reply dropped")

let lost = s4.onFeatureChanged(hasRail: false)
check(lost.strip(hasRail: false, gerbersSupported: false) == ViewerRail.fallbackTabs, "feature loss reverts to static strip")
check(!lost.onReply(r4, entries: entries, hasRail: false, gerbersSupported: false, currentTab: "Schematic", hasFileLink: false).accepted, "reply dropped after feature loss")

// Disabled tab tap issues zero watches and records why
let tabs = ViewerRail.phoneTabs([
    DViewerRailEntry(id: "board", label: "Board", group: "design", state: "disabled", why: "No board in this project"),
    DViewerRailEntry(id: "files", label: "Files", group: "work", state: "on"),
], gerbersSupported: false)
let s0 = ViewerRailState(status: .loaded, hasGood: true, threadId: "t1")
let tapDisabled = s0.tap(tabs[0], currentTab: "Files")
check(tapDisabled.tab == "Files" && !tapDisabled.tabChanged && !tapDisabled.shouldWatch && !tapDisabled.stopWatch && tapDisabled.state.disabledWhy == "No board in this project" && tapDisabled.state.localSelectionSeq == 0, "disabled tap issues zero watches")

let tapEnabled = tapDisabled.state.tap(tabs[1], currentTab: "Board")
check(tapEnabled.tab == "Files" && tapEnabled.tabChanged && !tapEnabled.shouldWatch && tapEnabled.stopWatch && tapEnabled.state.disabledWhy == "" && tapEnabled.state.localSelectionSeq == 1, "enabled tap to non-watch tab stops watch")

// Cross-layer controller test with fake transport: ordered watch replacement + stale-local refetch
final class FakeTransport: ViewerRailTransport {
    var log: [String] = []
    var nextFetch: DViewerRailReply?
    func fetchRail(threadId: String, page: String, diff: Bool) -> DViewerRailReply? {
        log.append("fetch:\(threadId):\(page)")
        return nextFetch
    }
    func watch(threadId: String, kind: String) { log.append("watch:\(threadId):\(kind)") }
    func stopWatch(threadId: String) { log.append("stop:\(threadId)") }
}

let tr = FakeTransport()
let ctrl = ViewerRailController(transport: tr, state: ViewerRailState(threadId: "t1"), tab: "Board")

let (cs1, cr1) = ctrl.state.beginRequest(threadId: "t1", hasRail: true)
ctrl.state = cs1
let reply1 = DViewerRailReply(
    rail: [
        DViewerRailEntry(id: "board", label: "Board", group: "design", state: "on"),
        DViewerRailEntry(id: "schematic", label: "Schematic", group: "design", state: "off"),
        DViewerRailEntry(id: "files", label: "Files", group: "work", state: "off"),
    ],
    has: DViewerRailHas(board: false, schematic: true, mech: false, gerbers: false)
)
ctrl.applyReply(cr1, reply: reply1, hasRail: true, gerbersSupported: true)
check(ctrl.tab == "Schematic" && tr.log == ["watch:t1:schematic"], "watch-to-watch reconcile issues watch without stop", tr.log)

tr.log.removeAll()
let (cs2, cr2) = ctrl.state.beginRequest(threadId: "t1", hasRail: true)
ctrl.state = cs2
ctrl.selectLocal("Gerbers")
check(tr.log == ["watch:t1:gerbers"], "local watch-to-watch switch issues watch without stop", tr.log)
tr.log.removeAll()

tr.nextFetch = DViewerRailReply(
    rail: [
        DViewerRailEntry(id: "gerbers", label: "Gerbers", group: "design", state: "on"),
        DViewerRailEntry(id: "files", label: "Files", group: "work", state: "off"),
    ],
    has: DViewerRailHas(board: false, schematic: false, mech: false, gerbers: false)
)
ctrl.applyReply(cr2, reply: reply1, hasRail: true, gerbersSupported: true)
check(ctrl.tab == "Files" && tr.log == ["fetch:t1:gerbers", "stop:t1"], "stale localSelectionSeq refetches and stops watch before landing on Files", tr.log)

print("viewer-rail: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
