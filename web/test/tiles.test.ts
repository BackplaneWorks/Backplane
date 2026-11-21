import { describe, expect, test } from "bun:test";
import {
  canOverlayGerbers,
  formatLayoutError,
  isRowSplit,
  isTiledOrOverlayLayout,
  isTileKind,
  normalizeBounds,
  proposedSplitRatio,
  routeFrameToTiles,
  type LayoutReply,
} from "../src/viewers/tiles-state";

describe("tiles-state", () => {
  test("recognizes only data/1 tile kinds (never terminal or browser)", () => {
    for (const k of ["board", "schematic", "3d", "gerbers", "mech", "files", "diff", "review"]) {
      expect(isTileKind(k)).toBe(true);
    }
    for (const k of ["terminal", "browser", "split", "kicad", ""]) {
      expect(isTileKind(k)).toBe(false);
    }
  });

  test("routes incoming plot keys strictly by hub {tileId: [key...]} map", () => {
    const tileKeys = {
      t_board: ["0|/proj|a.kicad_pcb"],
      t_3d: ["2|/proj|a.kicad_pcb", "0|/proj|a.kicad_pcb"],
      t_gerbers: ["0|/proj|mfg/gerbers"],
    };
    expect(routeFrameToTiles("0|/proj|a.kicad_pcb", tileKeys)).toEqual(["t_board", "t_3d"]);
    expect(routeFrameToTiles("2|/proj|a.kicad_pcb", tileKeys)).toEqual(["t_3d"]);
    expect(routeFrameToTiles("0|/proj|mfg/gerbers", tileKeys)).toEqual(["t_gerbers"]);
    expect(routeFrameToTiles("1|/proj|other.kicad_sch", tileKeys)).toEqual([]);
    expect(routeFrameToTiles("", tileKeys)).toEqual([]);
  });

  test("detects multi-tile and overlay layouts", () => {
    const base: LayoutReply = {
      thread: "t1",
      rev: 1,
      mode: "full",
      bounds: [1000, 700],
      order: ["a"],
      tiles: [{ id: "a", kind: "board", source: "", focused: true, visible: true, rect: { x: 0, y: 0, w: 1, h: 1 } }],
      splits: [],
    };
    expect(isTiledOrOverlayLayout(base)).toBe(false);

    const withOverlay: LayoutReply = {
      ...base,
      tiles: [{ ...base.tiles[0], overlay: { kind: "gerbers", mode: "diff" } }],
    };
    expect(isTiledOrOverlayLayout(withOverlay)).toBe(true);

    const multi: LayoutReply = {
      ...base,
      order: ["a", "b"],
      tiles: [
        base.tiles[0],
        { id: "b", kind: "gerbers", source: "", focused: false, visible: true, rect: { x: 0.5, y: 0, w: 0.5, h: 1 } },
      ],
    };
    expect(isTiledOrOverlayLayout(multi)).toBe(true);
  });

  test("clamps bounds and splitter ratios safely", () => {
    expect(normalizeBounds(0, -5)).toEqual([100, 100]);
    expect(normalizeBounds(1440.4, 900.6)).toEqual([1440, 901]);
    expect(normalizeBounds(20000, Number.NaN)).toEqual([8192, 100]);
    expect(proposedSplitRatio(250, 100, 500)).toBeCloseTo(0.3, 5);
    expect(proposedSplitRatio(0, 100, 500)).toBe(0.1);
    expect(proposedSplitRatio(999, 100, 500)).toBe(0.9);
    expect(isRowSplit("row")).toBe(true);
    expect(isRowSplit("col")).toBe(false);
  });

  test("gates Gerbers overlay controls and formats hub refusals", () => {
    expect(canOverlayGerbers({ kind: "board", overlay: null }, true)).toBe(true);
    expect(canOverlayGerbers({ kind: "board", overlay: null }, false)).toBe(false);
    expect(canOverlayGerbers({ kind: "board", overlay: { kind: "gerbers", mode: "diff" } }, false)).toBe(true);
    expect(canOverlayGerbers({ kind: "schematic", overlay: null }, true)).toBe(false);
    expect(formatLayoutError({ error: { code: "tooManyTiles", message: "At most 8 tiles" } })).toBe("At most 8 tiles");
    expect(formatLayoutError(new Error("Layout refused"))).toBe("Layout refused");
  });
});
