import Foundation
import CoreGraphics

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") {
    if ok { passed += 1 }
    else { failed += 1; print("FAIL \(name): \(got)") }
}

// 1. Unwrapping plan.get reply vs info push frame
let directJSON = Data(#"{"revision":4,"status":"awaiting","layers":[{"layer":"F.Cu","role":"signal","title":"L1 · TOP","items":[{"id":"c1","kind":"corridor"}]}]}"#.utf8)
let wrappedJSON = Data(#"{"result":{"panel":{"revision":7,"board_trimmed":true,"board":{"bbox":[0,0,80,50],"outline":[],"parts":[]}}}}"#.utf8)
check(PlanFloorplannerLogic.unwrapPanel(from: directJSON)?.revision == 4, "unwrap direct panel")
check(PlanFloorplannerLogic.unwrapPanel(from: wrappedJSON)?.revision == 7, "unwrap rpc result.panel")
check(PlanFloorplannerLogic.unwrapPanel(from: wrappedJSON)?.boardTrimmed == true, "board_trimmed parsed")

// 2. All copper layers + default active layer selection + canonical titles
let planned = [PlanLayer(layer: "In5.Cu", role: "signal", items: [PlanItem(id: "c1", kind: "corridor"), PlanItem(id: "c2", kind: "corridor")])]
let copper = [
    PlanCopperLayer(layer: "F.Cu", index: 1, role: "signal", title: "L1 · TOP"),
    PlanCopperLayer(layer: "In1.Cu", index: 2, role: "power", title: "L2 · GND1"),
    PlanCopperLayer(layer: "In5.Cu", index: 6, role: "signal", title: "L6 · SIG1"),
]
let allLayers = PlanFloorplannerLogic.allCanvasLayers(planned: planned, copper: copper)
check(allLayers.count == 3, "allCanvasLayers includes empty copper layers", allLayers.count)
check(allLayers[1].title == "L2 · GND1", "copper title preserved")
check(PlanFloorplannerLogic.defaultActiveLayer(layers: allLayers) == "In5.Cu", "defaultActiveLayer chooses busiest signal")
check(PlanFloorplannerLogic.layerDisplayTitle("In5.Cu", copper: copper, allLayers: allLayers) == "L6 · SIG1", "canonical display title")
check(PlanFloorplannerLogic.layerDisplayTitle("In9.Cu", copper: copper, allLayers: allLayers) == "In9.Cu", "fallback display title")

// 3. Rail & bus colors
check(PlanFloorplannerLogic.railColorRGB("+3V3", fallback: "#5a7a9a") == 0xF59E0B, "+3V3 rail amber")
check(PlanFloorplannerLogic.railColorRGB("+1V8") == 0xC084FC, "+1V8 rail violet")
check(PlanFloorplannerLogic.railColorRGB("+5V", fallback: "#123456") == 0xFB7185, "5V coral override")
check(PlanFloorplannerLogic.busColorRGB("SPI", fallback: "#14b8a6") == 0x14B8A6, "bus hex parse")

// 4. Camera fit / toPx / toWorld round-trip + pan & zoom
let cam = PlanCamera.fit(bbox: [0, 0, 100, 100], width: 200, height: 200, pad: 0)
let px = cam.toPx(x: 25, y: 75, width: 200, height: 200)
let world = cam.toWorld(px: px.x, py: px.y, width: 200, height: 200)
check(abs(Double(px.x) - 50) < 0.01 && abs(Double(px.y) - 150) < 0.01, "camera toPx", px)
check(abs(world[0] - 25) < 0.01 && abs(world[1] - 75) < 0.01, "camera toWorld", world)
let zoomed = cam.zoomed(factor: 2.0, anchorX: 100, anchorY: 100, width: 200, height: 200)
check(abs(zoomed.scale - cam.scale * 2.0) < 0.01, "camera zoom scale")

// 5. Polyline midpoint & vertex insert/remove min bounds
check(PlanFloorplannerLogic.polylineMidpoint(pts: [[10, 0], [20, 0], [20, 30]]) == [20, 10], "polyline midpoint")
let open: [[Double]] = [[0, 0], [10, 0]]
check(PlanFloorplannerLogic.removeVertex(pts: open, idx: 0, closed: false) == nil, "open min 2")
let ins = PlanFloorplannerLogic.insertVertex(pts: open, segIdx: 0, pos: [5, 4], closed: false)
check(ins.count == 3 && ins[1] == [5, 4], "open insert")
check(PlanFloorplannerLogic.removeVertex(pts: ins, idx: 1, closed: false) == open, "open remove back")
let tri: [[Double]] = [[0, 0], [10, 0], [10, 10]]
check(PlanFloorplannerLogic.removeVertex(pts: tri, idx: 0, closed: true) == nil, "closed min 3")

// 6. Hit test priority: vertex > midpoint > band > region (excluding holes)
let bands = [PlanBand(id: "c1", layer: "In5.Cu", name: "SPI", width: 4, nets: 4, path: [[10, 50], [90, 50]])]
let regions = [PlanRegion(id: "p1", layer: "In5.Cu", net: "+3V3", outline: [[5, 5], [95, 5], [95, 95], [5, 95]], holes: [[[30, 70], [45, 70], [45, 85], [30, 85]]])]
check(PlanFloorplannerLogic.hitTest(bands: bands, regions: regions, layer: "In5.Cu", selectedId: "c1", cam: cam, width: 200, height: 200, x: 22, y: 102) == .vertex(id: "c1", isPlane: false, idx: 0), "hit vertex")
check(PlanFloorplannerLogic.hitTest(bands: bands, regions: regions, layer: "In5.Cu", selectedId: "c1", cam: cam, width: 200, height: 200, x: 100, y: 100) == .midpoint(id: "c1", isPlane: false, segIdx: 0), "hit midpoint")
check(PlanFloorplannerLogic.hitTest(bands: bands, regions: regions, layer: "In5.Cu", selectedId: "", cam: cam, width: 200, height: 200, x: 120, y: 100) == .band(id: "c1"), "hit band")
check(PlanFloorplannerLogic.hitTest(bands: bands, regions: regions, layer: "In5.Cu", selectedId: "", cam: cam, width: 200, height: 200, x: 30, y: 40) == .region(id: "p1"), "hit region")
check(PlanFloorplannerLogic.hitTest(bands: bands, regions: regions, layer: "In5.Cu", selectedId: "", cam: cam, width: 200, height: 200, x: 76, y: 156) == nil, "hole excluded")

// 7. Corridor pill collision avoidance + pour label hole avoidance + label collision
let cam400 = PlanCamera.fit(bbox: [0, 0, 100, 100], width: 400, height: 400, pad: 0)
let bA = PlanBand(id: "b1", layer: "F.Cu", name: "SPI", nets: 4, path: [[10, 50], [90, 50]])
let bB = PlanBand(id: "b2", layer: "F.Cu", name: "I2C", nets: 2, path: [[10, 50], [90, 50]])
let pills = PlanFloorplannerLogic.placeCorridorPills(bands: [bA, bB], cam: cam400, width: 400, height: 400, selectedId: "b1")
check(pills.count == 2 && pills[0].text == "SPI · 4", "selected pill shows name · nets", pills)
check(!pills[0].rect.intersects(pills[1].rect), "corridor pills do not intersect")
let labelsBlocked = PlanFloorplannerLogic.layoutLabels(parts: [PlanPartBox(r: "U_BLK", s: "F", b: [40, 40, 60, 60], o: false)], cam: cam400, width: 400, height: 400, pinned: [], blocked: [pills[0].rect])
check(labelsBlocked.isEmpty, "refdes avoids pill box")

let pourPt = PlanFloorplannerLogic.pourLabelPoint(outer: [[0, 0], [100, 0], [100, 100], [0, 100]], holes: [[[35, 35], [65, 35], [65, 65], [35, 65]]])
check(pourPt != nil && !(pourPt![0] >= 35 && pourPt![0] <= 65 && pourPt![1] >= 35 && pourPt![1] <= 65), "pour label avoids center hole")
let emptyPourPt = PlanFloorplannerLogic.pourLabelPoint(outer: [[0, 0], [100, 0], [100, 100], [0, 100]], holes: [[[2, 2], [98, 2], [98, 98], [2, 98]]])
check(emptyPourPt == nil, "pour label returns nil when all candidates hit hole")

let crowded = (0..<8).map { i in
    PlanBand(id: "k\(i)", layer: "F.Cu", name: "BUS\(i)", nets: i + 1, path: [[50, 50], [51, 50]])
}
let crowdedPills = PlanFloorplannerLogic.placeCorridorPills(bands: crowded, cam: cam400, width: 400, height: 400, selectedId: "")
check(crowdedPills.count < 8, "overlapping corridor pills suppressed", crowdedPills.count)
var anyOverlap = false
for i in 0..<crowdedPills.count {
    for j in (i + 1)..<crowdedPills.count {
        if crowdedPills[i].rect.intersects(crowdedPills[j].rect) { anyOverlap = true }
    }
}
check(!anyOverlap, "no placed corridor pills overlap")

// 8. Optimistic merge moves item and preserves plane holes
let basePanel = PlanPanel(
    revision: 2,
    layers: allLayers,
    draw: PlanDraw(bands: bands, regions: regions)
)
let mergedPanel = PlanFloorplannerLogic.mergeOptimistic(panel: basePanel, patches: [
    "c1": PlanOptimisticPatch(width: 2.25, layer: "F.Cu"),
    "p1": PlanOptimisticPatch(outline: [[0, 0], [100, 0], [100, 100], [0, 100]]),
])
check(mergedPanel.draw?.regions.first?.holes.count == 1, "plane holes preserved on outline edit")
check(mergedPanel.layers.first(where: { $0.layer == "F.Cu" })?.items.contains(where: { $0.id == "c1" && $0.width == 2.25 }) == true, "optimistic layer move")

// 9. Canonical project decoding & nested badge decoding
let projOnly = Data(#"{"project":"/p/boards/b/b.kicad_pro","revision":5,"layers":[]}"#.utf8)
check(PlanFloorplannerLogic.unwrapPanel(from: projOnly)?.pro == "/p/boards/b/b.kicad_pro", "project field populates pro")
let outerPro = Data(#"{"pro":"/p/main.kicad_pro","panel":{"revision":6,"layers":[]}}"#.utf8)
check(PlanFloorplannerLogic.unwrapPanel(from: outerPro)?.pro == "/p/main.kicad_pro", "outer pro propagates to panel")
let badBadge = PlanFloorplannerLogic.decodeBadge(from: Data(#"{"badge":{"n":3,"tone":"bad"}}"#.utf8))
check(badBadge.n == 3 && badBadge.tone == "bad", "nested bad badge")
let warnBadge = PlanFloorplannerLogic.decodeBadge(from: Data(#"{"badge":{"n":2,"tone":"warn"}}"#.utf8))
check(warnBadge.n == 2 && warnBadge.tone == "warn", "nested warn badge")

// 10. File-link management project resolution
let boardRefs = [
    PlanBoardRef(value: "boards/main/main.kicad_pcb", pcb: "", sch: "", project: "/proj/boards/main/main.kicad_pro"),
    PlanBoardRef(value: "boards/sub/sub.kicad_pcb", pcb: "/proj/boards/sub/sub.kicad_pcb", sch: "/proj/boards/sub/sub.kicad_sch", project: "/proj/boards/sub/sub.kicad_pro"),
]
check(
    PlanFloorplannerLogic.resolveMgmtProject(targetPath: "/proj/boards/sub/sub.kicad_pcb", selectedBoard: boardRefs[0], boards: boardRefs, root: "/proj") == "/proj/boards/sub/sub.kicad_pro",
    "file link beats default picker board"
)
check(
    PlanFloorplannerLogic.resolveMgmtProject(targetPath: "boards/sub/power.kicad_sch", selectedBoard: boardRefs[0], boards: boardRefs, root: "/proj") == "/proj/boards/sub/sub.kicad_pro",
    "relative sub-sheet resolves to matching board dir"
)
check(
    PlanFloorplannerLogic.resolveMgmtProject(targetPath: "extra/test_jig.kicad_pcb", selectedBoard: boardRefs[0], boards: boardRefs, root: "/proj") == "/proj/extra/test_jig.kicad_pcb",
    "unlisted relative board file resolves to abs path"
)
check(
    PlanFloorplannerLogic.resolveMgmtProject(targetPath: nil, selectedBoard: boardRefs[0], boards: boardRefs, root: "/proj") == "/proj/boards/main/main.kicad_pro",
    "nil target uses selected board"
)

// 11. Per-item undo isolation & selective rollback patch folding
var ustack: [PlanUndoEntry] = [
    PlanUndoEntry(pro: "/p/a", id: "c1", patch: PlanOptimisticPatch(path: [[0, 0], [10, 0]]), onlyGeom: "path"),
    PlanUndoEntry(pro: "/p/a", id: "c2", patch: PlanOptimisticPatch(width: 1.5)),
]
check(!PlanFloorplannerLogic.hasUndo(stack: ustack, pro: "/p/a", id: "c3"), "unrelated item has no undo")
check(PlanFloorplannerLogic.popUndo(stack: &ustack, pro: "/p/a", id: "c1")?.id == "c1", "pops matching item undo")
check(!PlanFloorplannerLogic.hasUndo(stack: ustack, pro: "/p/a", id: "c1"), "only matching entry popped")
check(PlanFloorplannerLogic.hasUndo(stack: ustack, pro: "/p/a", id: "c2"), "other item undo intact")

let folded = PlanFloorplannerLogic.foldPatches(
    [
        ("c1", PlanOptimisticPatch(width: 2.0)),
        ("c2", PlanOptimisticPatch(width: 3.0)),
    ],
    preview: ("c1", PlanOptimisticPatch(path: [[1, 1], [2, 2]]))
)
check(folded["c1"]?.width == 2.0 && folded["c1"]?.path == [[1, 1], [2, 2]], "preview overlays without dropping pending fields")
let withoutPreview = PlanFloorplannerLogic.foldPatches([("c1", PlanOptimisticPatch(width: 2.0))])
check(withoutPreview["c1"]?.width == 2.0 && withoutPreview["c1"]?.path == nil, "cancelled drag leaves accepted patch intact")

print("plan-floorplanner: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
