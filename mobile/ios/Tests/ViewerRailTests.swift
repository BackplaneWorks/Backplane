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
check(noGerb[1].disabled && noGerb[1].why == "No board in this project", "disabled entry keeps why", noGerb[1])
check(noGerb[2].on && !noGerb[2].disabled, "on entry lit", noGerb[2])

let withGerb = ViewerRail.phoneTabs(reply.rail, gerbersSupported: true)
check(withGerb.map(\.tab) == ["Files", "Board", "Schematic", "Gerbers"], "gerbers kept when feature present", withGerb.map(\.tab))

let fb = ViewerRail.phoneTabs([], gerbersSupported: false)
check(fb.map(\.tab) == ["Board", "Schematic", "3D", "Split", "Mech", "Files", "Diff", "Review", "Browser"], "empty falls back to static strip", fb.map(\.tab))

check(ViewerRail.activeTab(noGerb, current: "Files", userPressed: false, hasFileLink: false) == "Schematic", "active tab follows hub on")
check(ViewerRail.activeTab(noGerb, current: "Files", userPressed: true, hasFileLink: false) == "Files", "local press wins over hub on")
check(ViewerRail.activeTab(noGerb, current: "Files", userPressed: false, hasFileLink: true) == "Files", "file link wins over hub on")

print("viewer-rail: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
