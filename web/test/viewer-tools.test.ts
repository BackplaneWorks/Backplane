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

  test("matchesCompareKey routes only exact requested commit and rejects delayed previous commit frame", () => {
    const prefix = "0|/proj|diff\n";
    const frameA = "0|/proj|diff\ngit\t/proj\taaa111\t12\tboard.kicad_pcb";
    const frameB = "0|/proj|diff\ngit\t/proj\tbbb222\t12\tboard.kicad_pcb";
    // Delayed Compare A -> B: only B matches while waiting for bbb222
    expect(matchesCompareKey(frameA, prefix, "bbb222")).toBe(false);
    expect(matchesCompareKey(frameB, prefix, "bbb222")).toBe(true);
    // Rejects wrong prefix or empty guards
    expect(matchesCompareKey(frameB, "1|/proj|diff\n", "bbb222")).toBe(false);
    expect(matchesCompareKey(frameB, "", "bbb222")).toBe(false);
    expect(matchesCompareKey(frameB, prefix, "")).toBe(false);
  });
});
