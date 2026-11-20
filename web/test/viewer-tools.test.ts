import { describe, expect, test } from "bun:test";
import {
  HistoryStripState,
  matchesCompareKey,
  SOLID_VIEW_PRESETS,
  solidViewAngles,
} from "../src/viewers/viewer-tools";

describe("viewer-tools", () => {
  test("provides all 8 GPUI 3D view presets with finite angles", () => {
    expect(SOLID_VIEW_PRESETS.map((p) => p.id)).toEqual([
      "fit",
      "iso",
      "top",
      "bottom",
      "front",
      "right",
      "back",
      "left",
    ]);
    for (const p of SOLID_VIEW_PRESETS) {
      const { yaw, pitch } = solidViewAngles(p.id);
      expect(Number.isFinite(yaw)).toBe(true);
      expect(Number.isFinite(pitch)).toBe(true);
    }
    expect(solidViewAngles("top").pitch).toBeGreaterThan(1.5);
    expect(solidViewAngles("bottom").pitch).toBeLessThan(-1.5);
    expect(solidViewAngles("front")).toEqual({ yaw: 0, pitch: 0 });
  });

  test("HistoryStripState steps between Live and saved refs and toggles Highlight edits off/on on same ref", () => {
    const st = new HistoryStripState();
    st.setRefs([
      { kind: "turn", label: "Turn 2", commit: "aaa222" },
      { kind: "turn", label: "Turn 1", commit: "aaa111" },
    ]);
    expect(st.isLive()).toBe(true);
    expect(st.compareRequest()).toBeNull();

    // Step older -> index 0 (Turn 2)
    expect(st.step(1)).toBe(true);
    expect(st.activeRef()?.commit).toBe("aaa222");
    expect(st.compareRequest()).toEqual({ commit: "aaa222", bits: 0 });

    // Toggle highlight edits OFF on selected ref -> returns to Live while remembering aaa222
    expect(st.toggleHighlight()).toBe(false);
    expect(st.isLive()).toBe(true);
    expect(st.compareRequest()).toBeNull();

    // Toggle highlight edits ON again -> restores aaa222 compare
    expect(st.toggleHighlight()).toBe(true);
    expect(st.isLive()).toBe(false);
    expect(st.activeRef()?.commit).toBe("aaa222");
    expect(st.compareRequest()).toEqual({ commit: "aaa222", bits: 0 });

    // Step older -> index 1 (Turn 1), clamped at end
    expect(st.step(1)).toBe(true);
    expect(st.activeRef()?.commit).toBe("aaa111");
    expect(st.step(1)).toBe(false);

    // Step newer -> index 0 -> Live (-1)
    expect(st.step(-1)).toBe(true);
    expect(st.step(-1)).toBe(true);
    expect(st.isLive()).toBe(true);
    expect(st.compareRequest()).toBeNull();
  });

  test("toggling Highlight edits from Live automatically selects newest ref", () => {
    const st = new HistoryStripState();
    st.setRefs([{ kind: "turn", label: "Checkpoint", commit: "ccc333" }]);
    expect(st.isLive()).toBe(true);
    expect(st.toggleHighlight()).toBe(true);
    expect(st.isLive()).toBe(false);
    expect(st.compareRequest()).toEqual({ commit: "ccc333", bits: 0 });
    expect(st.goLive()).toBe(true);
    expect(st.isLive()).toBe(true);
  });

  test("matchesCompareKey routes only exact requested commit and file, rejecting delayed commit or sibling sheet frames", () => {
    const prefix = "0|/proj|diff\n";
    const frameA = "0|/proj|diff\ngit\t/proj\taaa111\t5\t/proj/board.kicad_pcb\n/proj/board.kicad_pcb";
    const frameB = "0|/proj|diff\ngit\t/proj\tbbb222\t5\t/proj/board.kicad_pcb\n/proj/board.kicad_pcb";
    // Delayed Compare A -> B: only B matches while waiting for bbb222
    expect(matchesCompareKey(frameA, prefix, "bbb222", "/proj/board.kicad_pcb")).toBe(false);
    expect(matchesCompareKey(frameB, prefix, "bbb222", "/proj/board.kicad_pcb")).toBe(true);

    // Same commit, two schematic sheets: late frame for sheet A cannot satisfy sheet B
    const schPrefix = "1|/proj|diff\n";
    const sheetA = "1|/proj|diff\ngit\t/proj\tbbb222\t5\t/proj/power.kicad_sch\n/proj/power.kicad_sch";
    const sheetB = "1|/proj|diff\ngit\t/proj\tbbb222\t5\t/proj/amp.kicad_sch\n/proj/amp.kicad_sch";
    expect(matchesCompareKey(sheetA, schPrefix, "bbb222", "/proj/amp.kicad_sch")).toBe(false);
    expect(matchesCompareKey(sheetB, schPrefix, "bbb222", "/proj/amp.kicad_sch")).toBe(true);
    expect(matchesCompareKey(sheetA, schPrefix, "bbb222", "amp.kicad_sch")).toBe(false);
    expect(matchesCompareKey(sheetB, schPrefix, "bbb222", "amp.kicad_sch")).toBe(true);

    // Rejects empty expectedFile (e.g. unresolved viewer.files metadata), wrong prefix, or empty guards
    expect(matchesCompareKey(sheetA, schPrefix, "bbb222", "")).toBe(false);
    expect(matchesCompareKey(frameB, prefix, "bbb222", "")).toBe(false);
    expect(matchesCompareKey(frameB, "1|/proj|diff\n", "bbb222", "/proj/board.kicad_pcb")).toBe(false);
    expect(matchesCompareKey(frameB, "", "bbb222", "/proj/board.kicad_pcb")).toBe(false);
    expect(matchesCompareKey(frameB, prefix, "", "/proj/board.kicad_pcb")).toBe(false);
  });
});
