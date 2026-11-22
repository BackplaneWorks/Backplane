import { describe, expect, test } from "bun:test";
import {
  abbrev,
  badgeOf,
  buildBomUndoParams,
  drcWords,
  filterBomLines,
  filterCheckGroups,
  findingRefs,
  formatUtcDay,
  isBomWriteCurrent,
  isChecksFixCurrent,
  lifeWord,
  mgmtPanelsOf,
  money,
  pkgName,
  planDone,
  planMark,
  planProgressSegments,
  planStatusBadge,
  refsFit,
  stockSummary,
  tierWords,
  planPolylineMidpoint,
  planRailColor,
  planBusColor,
  planFitCamera,
  planHitTest,
  planInsertVertex,
  planRemoveVertex,
  planDefaultLayerPair,
  planLayerDisplayTitle,
  planLayoutLabels,
  planListLayersForView,
  planPlaceCorridorPills,
  planPourLabelPoint,
  planAllCanvasLayers,
  unwrapPlanGetPanel,
  isPlanHydrationCurrent,
} from "../src/mgmt-model";

describe("Management strip pure model", () => {
  test("mgmtPanelsOf keeps known hub tabs in hub order", () => {
    const tabs = mgmtPanelsOf({
      panels: [
        { id: "bom", label: "BOM" },
        { id: "checks", label: "Checks" },
        { id: "unknown", label: "Other" },
        { id: "life", label: "Lifecycle" },
        { id: "plan", label: "Plan" },
        { id: "fab", label: "Fab" },
      ],
    });
    expect(tabs.map((t) => t.id)).toEqual(["bom", "checks", "life", "plan", "fab"]);
  });

  test("badgeOf caps at 99+ and defaults tone to warn", () => {
    expect(badgeOf(null)).toBeNull();
    expect(badgeOf({ badge: { n: 0, tone: "bad" } })).toBeNull();
    expect(badgeOf({ badge: { n: 3, tone: "bad" } })).toEqual({ text: "3", tone: "bad" });
    expect(badgeOf({ badge: { n: 120, tone: "other" } })).toEqual({ text: "99+", tone: "warn" });
  });

  test("refsFit, abbrev, money, pkgName, stockSummary match DataMgmt.kt", () => {
    expect(refsFit(["R1", "R2", "R3"], 20)).toBe("R1, R2, R3");
    expect(refsFit(["C1", "C2", "C3", "C4", "C5", "C6"], 12)).toBe("C1, C2 +4");
    expect(abbrev(450)).toBe("450");
    expect(abbrev(12400)).toBe("12k");
    expect(abbrev(1_060_770)).toBe("1.1M");
    expect(money(1.2)).toBe("$1.20");
    expect(money(0.5)).toBe("$0.50");
    expect(money(0.0029)).toBe("$0.0029");
    expect(pkgName("Capacitor_SMD:C_0402_1005Metric")).toBe("0402");
    expect(pkgName("Package_DFN_QFN:QFN-48-1EP_6x6mm")).toBe("QFN-48");
    expect(stockSummary({ status: "in", stock: 717000 })).toEqual({ text: "717k", tone: "ok" });
    expect(stockSummary({ status: "low", stock: 12 })).toEqual({ text: "low 12", tone: "warn" });
    expect(stockSummary({ status: "out", stock: 0 })).toEqual({ text: "out 0", tone: "bad" });
    expect(stockSummary({ status: "checking" })).toEqual({ text: "…", tone: "faint" });
  });

  test("filterBomLines and findingRefs", () => {
    const lines = [
      { id: "R1", refs: ["R1", "R2"], value: "10k", footprint: "R_0603", mpn: "RC0603", dnp: false },
      { id: "C1", refs: ["C1"], value: "100n", footprint: "C_0402", mpn: "", dnp: true },
    ];
    expect(filterBomLines(lines, "0603 10k").map((l) => l.id)).toEqual(["R1"]);
    expect(filterBomLines(lines, "dnp").map((l) => l.id)).toEqual(["C1"]);
    expect(findingRefs("GND · SW3.1 · R4 · SW3.2")).toEqual(["SW3", "R4"]);
  });

  test("checks, lifecycle, plan, and fab formatters", () => {
    const groups = [
      { source: "drc", severity: "error", rule: "clearance", n: 2, rows: [] },
      { source: "erc", severity: "warning", rule: "pin", n: 1, rows: [] },
    ];
    expect(filterCheckGroups(groups, "").length).toBe(2);
    expect(filterCheckGroups(groups, "drc").length).toBe(1);
    expect(lifeWord("eol")).toEqual({ label: "EOL", tone: "bad" });
    expect(formatUtcDay(1700000000)).toBe("2023-11-14");
    expect(planDone({ id: "a", state: "routed" })).toBe(true);
    expect(planDone({ id: "b", state: "proposed" })).toBe(false);
    expect(planMark({ id: "a", state: "routing", routed: 3, total: 8 })).toBe("3/8");
    expect(planMark({ id: "b", state: "blocked", problems: 2 })).toBe("⚠ 2");
    expect(planMark({ id: "c", state: "failed" })).toBe("✗");
    expect(planStatusBadge({ status: "blocked" })).toEqual({ key: "gpui.mgmt.plan.st.blocked", label: "Blocked", tone: "bad" });
    expect(planStatusBadge({ status: "verified" }).tone).toBe("ok");
    expect(planStatusBadge({ status: "awaiting" }).tone).toBe("warn");
    expect(planStatusBadge({ status: "partial" })).toEqual({ key: "gpui.mgmt.plan.st.partial", label: "Partial", tone: "warn" });
    expect(planStatusBadge({ status: "rejected" })).toEqual({ key: "gpui.mgmt.plan.st.rejected", label: "Rejected", tone: "bad" });
    expect(planProgressSegments({ total: 6, done: 2, approved: 1, awaiting: 2, blocked: 1, routing: false })).toEqual([
      { count: 2, tone: "ok" },
      { count: 0, tone: "accent" },
      { count: 1, tone: "input" },
      { count: 2, tone: "warn" },
      { count: 1, tone: "bad" },
    ]);
    expect(planProgressSegments({
      routing: true,
      layers: [{ layer: "F.Cu", items: [{ id: "1", state: "verified" }, { id: "2", state: "routing" }, { id: "3", state: "approved" }] }],
      awaiting: 1,
      blocked: 0,
    })).toEqual([
      { count: 1, tone: "ok" },
      { count: 1, tone: "accent" },
      { count: 1, tone: "input" },
      { count: 1, tone: "warn" },
      { count: 0, tone: "bad" },
    ]);
    expect(drcWords("stale")).toEqual({ text: "The DRC rules have older fab limits", button: "Update" });
    expect(drcWords("current").button).toBeNull();
    expect(tierWords("extra")).toBe("costs-more limits");
  });

  test("delayed BOM edit is discarded after switching project or editor part, and Undo sends original project/refs", () => {
    const startA = { watchGen: 1, editGen: 1, project: "/proj/A", editId: "R1" };
    // Still on project A, same editor -> applies
    expect(isBomWriteCurrent(startA, { watchGen: 1, editGen: 1, project: "/proj/A", editId: "R1" })).toBe(true);
    // Switched to project B and selected a different part -> stale save discarded
    expect(isBomWriteCurrent(startA, { watchGen: 2, editGen: 2, project: "/proj/B", editId: "C4" })).toBe(false);
    // Switched editor to another part in same project -> stale save discarded
    expect(isBomWriteCurrent(startA, { watchGen: 1, editGen: 2, project: "/proj/A", editId: "R2" })).toBe(false);
    // Closed and reopened same part (editGen bumped) -> stale save discarded
    expect(isBomWriteCurrent(startA, { watchGen: 1, editGen: 3, project: "/proj/A", editId: "R1" })).toBe(false);

    // Undo record binds its original project and refs, independent of any other open editor refs
    const undo = { project: "/proj/A", editId: "R1", refs: ["R1", "R2"], field: "mpn", was: "OLD-MPN" };
    expect(buildBomUndoParams(undo)).toEqual({
      project: "/proj/A",
      refs: ["R1", "R2"],
      field: "mpn",
      value: "OLD-MPN",
    });
  });

  test("delayed Checks Fix is discarded after switching threads within the same project or switching projects", () => {
    const start = { watchGen: 1, project: "/proj/A", threadId: "t1", threadGen: 5 };
    // Same thread & generation -> allowed to send
    expect(isChecksFixCurrent(start, { watchGen: 1, project: "/proj/A", threadId: "t1", threadGen: 5 })).toBe(true);
    // Switched to t2 within the SAME project -> discarded, t2 receives no message
    expect(isChecksFixCurrent(start, { watchGen: 1, project: "/proj/A", threadId: "t2", threadGen: 6 })).toBe(false);
    // Switched away and back to t1 (threadGen bumped) -> stale fix discarded
    expect(isChecksFixCurrent(start, { watchGen: 1, project: "/proj/A", threadId: "t1", threadGen: 7 })).toBe(false);
    // Switched project -> discarded
    expect(isChecksFixCurrent(start, { watchGen: 2, project: "/proj/B", threadId: "t1", threadGen: 5 })).toBe(false);
  });

  test("floorplanner canvas midpoint, hit-testing, vertex edit, label collision, and default pair", () => {
    expect(planPolylineMidpoint([])).toBeNull();
    expect(planPolylineMidpoint([[10, 0], [20, 0], [20, 30]])).toEqual([20, 10]);

    expect(planRailColor("+3V3", "#5a7a9a")).toBe("#f59e0b");
    expect(planRailColor("+1V8")).toBe("#c084fc");
    expect(planRailColor("+5V", "#123456")).toBe("#123456");
    expect(planBusColor("USB", "#14b8a6")).toBe("#14b8a6");

    const cam = planFitCamera([0, 0, 100, 100], 200, 200, 0);
    const bands = [{ id: "c1", layer: "In5.Cu", width: 4, locked: false, path: [[10, 50], [90, 50]] as Array<[number, number]> }];
    const regions = [{
      id: "p1",
      layer: "In5.Cu",
      locked: false,
      outline: [[5, 5], [95, 5], [95, 95], [5, 95]] as Array<[number, number]>,
      holes: [[[30, 70], [45, 70], [45, 85], [30, 85]]] as Array<Array<[number, number]>>,
    }];
    expect(planHitTest(bands, regions, "In5.Cu", "c1", cam, 200, 200, 21, 101)).toEqual({
      kind: "vertex",
      id: "c1",
      isPlane: false,
      idx: 0,
    });
    expect(planHitTest(bands, regions, "In5.Cu", "c1", cam, 200, 200, 100, 100)?.kind).toBe("midpoint");
    expect(planHitTest(bands, regions, "In5.Cu", "", cam, 200, 200, 120, 100)).toEqual({ kind: "band", id: "c1" });
    expect(planHitTest(bands, regions, "In5.Cu", "", cam, 200, 200, 30, 40)).toEqual({ kind: "region", id: "p1" });
    expect(planHitTest(bands, regions, "In5.Cu", "", cam, 200, 200, 76, 156)).toBeNull();

    const open: Array<[number, number]> = [[0, 0], [10, 0]];
    expect(planRemoveVertex(open, 0, false)).toBeNull();
    const inserted = planInsertVertex(open, 0, [5, 4], false);
    expect(inserted).toEqual([[0, 0], [5, 4], [10, 0]]);
    expect(planRemoveVertex(inserted, 1, false)).toEqual(open);

    const tri: Array<[number, number]> = [[0, 0], [10, 0], [10, 10]];
    expect(planRemoveVertex(tri, 0, true)).toBeNull();
    expect(planInsertVertex(tri, 2, [0, 10], true).length).toBe(4);

    const pair = planDefaultLayerPair([
      { layer: "F.Cu", role: "signal", items: [{ id: "a", kind: "corridor" }] },
      { layer: "In3.Cu", role: "power", items: [{ id: "b", kind: "plane" }, { id: "c", kind: "plane" }] },
      { layer: "In5.Cu", role: "signal", items: [{ id: "d", kind: "corridor" }, { id: "e", kind: "corridor" }] },
    ]);
    expect(pair).toEqual(["In5.Cu", "In3.Cu"]);

    const labels = planLayoutLabels(
      [
        { r: "U1", s: "F", b: [20, 20, 40, 40], o: false },
        { r: "R9", s: "F", b: [26, 26, 34, 34], o: false },
        { r: "U2", s: "F", b: [70, 70, 85, 85], o: false },
      ],
      planFitCamera([0, 0, 100, 100], 400, 400, 0),
      400,
      400,
      [],
    );
    expect(labels.map((l) => l.text)).toEqual(["U1", "U2"]);

    expect(unwrapPlanGetPanel(null)).toBeNull();
    expect(unwrapPlanGetPanel({ result: { panel: { revision: 3 } } })?.revision).toBe(3);
    expect(unwrapPlanGetPanel({ panel: { revision: 5 } })?.revision).toBe(5);
    const mergedAll = planAllCanvasLayers(
      [{ layer: "F.Cu", role: "signal", items: [{ id: "c1", kind: "corridor" }] }],
      [
        { layer: "F.Cu", index: 1, role: "signal", title: "L1 · TOP" },
        { layer: "In1.Cu", index: 2, role: "power", title: "L2 · GND", subtitle: "Faces TOP across 40 µm" },
      ],
    );
    expect(mergedAll.map((l) => l.layer)).toEqual(["F.Cu", "In1.Cu"]);
    expect(mergedAll[1].title).toBe("L2 · GND");

    expect(isPlanHydrationCurrent(1, 1, 2, 2, "/p/a", "/p/a", 4, undefined)).toBe(true);
    expect(isPlanHydrationCurrent(1, 2, 2, 2, "/p/a", "/p/a", 4, undefined)).toBe(false);
    expect(isPlanHydrationCurrent(1, 1, 1, 2, "/p/a", "/p/a", 4, undefined)).toBe(false);
    expect(isPlanHydrationCurrent(1, 1, 2, 2, "/p/a", "/p/b", 4, undefined)).toBe(false);
    expect(isPlanHydrationCurrent(1, 1, 2, 2, "/p/a", "/p/a", 3, 5)).toBe(false);

    expect(planLayerDisplayTitle("In5.Cu", [{ layer: "In5.Cu", index: 6, role: "signal", title: "L6 · SIG1" }])).toBe("L6 · SIG1");
    expect(planLayerDisplayTitle("In9.Cu", [])).toBe("In9.Cu");
    expect(planListLayersForView("split", mergedAll, ["In1.Cu"]).map((l) => l.layer)).toEqual(["In1.Cu"]);
    expect(planListLayersForView("split", mergedAll, ["In2.Cu"])).toEqual([]);
    expect(planListLayersForView("list", mergedAll, ["In1.Cu"]).map((l) => l.layer)).toEqual(["F.Cu", "In1.Cu"]);

    const pillCam = planFitCamera([0, 0, 100, 100], 400, 400, 0);
    const pills = planPlaceCorridorPills(
      [
        { id: "b1", layer: "F.Cu", name: "SPI", nets: 4, path: [[10, 50], [90, 50]] },
        { id: "b2", layer: "F.Cu", name: "I2C", nets: 2, path: [[10, 50], [90, 50]] },
      ],
      pillCam,
      400,
      400,
      "b1",
      "",
    );
    expect(pills.length).toBe(2);
    expect(pills[0].text).toBe("SPI · 4");
    expect(Math.hypot(pills[0].cx - pills[1].cx, pills[0].cy - pills[1].cy)).toBeGreaterThan(10);
    const p0Box: [number, number, number, number] = [
      pills[0].cx - pills[0].pw * 0.5,
      pills[0].cy - pills[0].ph * 0.5,
      pills[0].cx + pills[0].pw * 0.5,
      pills[0].cy + pills[0].ph * 0.5,
    ];
    expect(planLayoutLabels([{ r: "U_BLK", s: "F", b: [40, 40, 60, 60], o: false }], pillCam, 400, 400, [], [p0Box])).toEqual([]);

    const pourPt = planPourLabelPoint([[0, 0], [100, 0], [100, 100], [0, 100]], [[[35, 35], [65, 35], [65, 65], [35, 65]]]);
    expect(pourPt).not.toBeNull();
    if (pourPt) {
      expect(pourPt[0] >= 35 && pourPt[0] <= 65 && pourPt[1] >= 35 && pourPt[1] <= 65).toBe(false);
    }
    expect(planPourLabelPoint([[0, 0], [100, 0], [100, 100], [0, 100]], [[[2, 2], [98, 2], [98, 98], [2, 98]]])).toBeNull();

    const crowded = Array.from({ length: 8 }, (_, i) => ({
      id: `k${i}`,
      layer: "F.Cu",
      name: `BUS${i}`,
      nets: i + 1,
      path: [[50, 50], [51, 50]] as Array<[number, number]>,
    }));
    const crowdedPills = planPlaceCorridorPills(crowded, pillCam, 400, 400, "", "");
    expect(crowdedPills.length).toBeLessThan(8);
    for (let i = 0; i < crowdedPills.length; i++) {
      for (let j = i + 1; j < crowdedPills.length; j++) {
        const a = crowdedPills[i];
        const b = crowdedPills[j];
        const overlap = !(
          a.cx + a.pw * 0.5 <= b.cx - b.pw * 0.5 ||
          a.cx - a.pw * 0.5 >= b.cx + b.pw * 0.5 ||
          a.cy + a.ph * 0.5 <= b.cy - b.ph * 0.5 ||
          a.cy - a.ph * 0.5 >= b.cy + b.ph * 0.5
        );
        expect(overlap).toBe(false);
      }
    }
  });
});
