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
],"mode":"viewer"}
"""

let reply = try! JSONDecoder().decode(DViewerRailReply.self, from: Data(json.utf8))
let noGerb = ViewerRail.phoneTabs(reply.rail, gerbersSupported: false)
check(noGerb.map(\.tab) == ["Files", "Board", "Schematic"], "hub order preserved, tools/sim/gerbers/unknown dropped", noGerb.map(\.tab))
check(noGerb.map(\.label) == ["Files", "Board", "Schematic"], "hub labels preserved", noGerb.map(\.label))
check(noGerb[1].disabled && noGerb[1].why == "No board in this project", "disabled entry keeps why", noGerb[1])
check(noGerb[2].on && !noGerb[2].disabled, "on entry lit", noGerb[2])

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

// Fresh reply after newer local tap or file link keeps enabled tab; displaces disabled/missing tab
let entries = [
    DViewerRailEntry(id: "board", label: "Board", group: "design", state: "disabled", why: "No board"),
    DViewerRailEntry(id: "schematic", label: "Schematic", group: "design", state: "on"),
    DViewerRailEntry(id: "files", label: "Files", group: "work", state: "off"),
]
let (s1, r1) = ViewerRailState(threadId: "t1").beginRequest(threadId: "t1", hasRail: true)
let afterLocal = s1.onLocalSelect()
let keepLocal = afterLocal.onReply(r1, entries: entries, hasRail: true, gerbersSupported: false, currentTab: "Files", hasFileLink: false)
check(keepLocal.accepted && keepLocal.tab == "Files" && !keepLocal.stopWatch, "newer local tap wins while enabled")

let keepLink = s1.onReply(r1, entries: entries, hasRail: true, gerbersSupported: false, currentTab: "Files", hasFileLink: true)
check(keepLink.tab == "Files" && !keepLink.stopWatch, "file link wins while enabled")

let displaced = afterLocal.onReply(r1, entries: entries, hasRail: true, gerbersSupported: false, currentTab: "Board", hasFileLink: true)
check(displaced.tab == "Schematic" && displaced.stopWatch, "disabled current tab is displaced and stops watch")

// Stale request seq and stale thread dropped
let (s2, r2) = keepLocal.state.beginRequest(threadId: "t1", hasRail: true)
let (s3, r3) = s2.beginRequest(threadId: "t1", hasRail: true)
let staleSeq = s3.onReply(r2, entries: [], hasRail: true, gerbersSupported: false, currentTab: "Files", hasFileLink: false)
check(!staleSeq.accepted && staleSeq.tab == "Files", "older request id dropped")

let (otherThread, _) = s3.beginRequest(threadId: "t2", hasRail: true)
let staleThread = otherThread.onReply(r3, entries: entries, hasRail: true, gerbersSupported: false, currentTab: "Files", hasFileLink: false)
check(!staleThread.accepted, "other thread late reply dropped")

// Feature loss reverts to static strip and drops in-flight replies
let lost = s3.onFeatureChanged(hasRail: false)
check(lost.strip(hasRail: false, gerbersSupported: false) == ViewerRail.fallbackTabs, "feature loss reverts to static strip")
let afterLoss = lost.onReply(r3, entries: entries, hasRail: false, gerbersSupported: false, currentTab: "Files", hasFileLink: false)
check(!afterLoss.accepted, "reply dropped after feature loss")

// Disabled tab tap issues zero watches and records why
let tabs = ViewerRail.phoneTabs([
    DViewerRailEntry(id: "board", label: "Board", group: "design", state: "disabled", why: "No board in this project"),
    DViewerRailEntry(id: "files", label: "Files", group: "work", state: "on"),
], gerbersSupported: false)
let s0 = ViewerRailState(status: .loaded, hasGood: true, threadId: "t1")
let tapDisabled = s0.tap(tabs[0], currentTab: "Files")
check(tapDisabled.tab == "Files" && !tapDisabled.tabChanged && !tapDisabled.shouldWatch && tapDisabled.state.disabledWhy == "No board in this project" && tapDisabled.state.localSelectionSeq == 0, "disabled tap issues zero watches")

let tapEnabled = tapDisabled.state.tap(tabs[1], currentTab: "Board")
check(tapEnabled.tab == "Files" && tapEnabled.tabChanged && tapEnabled.shouldWatch && tapEnabled.state.disabledWhy == "" && tapEnabled.state.localSelectionSeq == 1, "enabled tap selects tab and clears why")

print("viewer-rail: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
