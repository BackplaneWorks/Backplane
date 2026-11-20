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
  refsFit,
  stockSummary,
  tierWords,
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
});
