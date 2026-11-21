import { describe, expect, test } from "bun:test";
import {
  canOverlayGerbers,
  classifyLayoutTransition,
  formatLayoutError,
  isRowSplit,
  isTiledOrOverlayLayout,
  isTileKind,
  MAX_TILE_PENDING_FRAMES,
  normalizeBounds,
  proposedSplitRatio,
  resolveGerberOverlayArg,
  routeFrameToTiles,
  TileFrameRouter,
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

  test("Finding 1: resolves Gerbers overlay source (prefers existing Gerbers tile, else hub set, never empty)", () => {
    const plainBoard: LayoutReply = {
      thread: "t1",
      rev: 1,
      mode: "full",
      bounds: [1000, 700],
      order: ["b1"],
      tiles: [{ id: "b1", kind: "board", source: "a.kicad_pcb", focused: true, visible: true, rect: { x: 0, y: 0, w: 1, h: 1 } }],
      splits: [],
    };
    // Refuses empty Gerber source
    expect(resolveGerberOverlayArg(plainBoard, "b1", "", "diff")).toBeNull();
    expect(canOverlayGerbers(plainBoard.tiles[0], "")).toBe(false);

    // Uses hub-provided root-relative Gerber set when present
    expect(canOverlayGerbers(plainBoard.tiles[0], "mfg/gerbers")).toBe(true);
    expect(resolveGerberOverlayArg(plainBoard, "b1", "mfg/gerbers", "diff")).toEqual({
      overlay: { kind: "gerbers", source: "mfg/gerbers", mode: "diff" },
      mode: "diff",
    });

    // Merges existing resolved Gerbers tile when present in layout
    const withGerbersTile: LayoutReply = {
      ...plainBoard,
      order: ["b1", "g1"],
      tiles: [
        plainBoard.tiles[0],
        { id: "g1", kind: "gerbers", source: "mfg/rev2", focused: false, visible: true, rect: { x: 0.5, y: 0, w: 0.5, h: 1 } },
      ],
    };
    expect(resolveGerberOverlayArg(withGerbersTile, "b1", "mfg/gerbers", "old")).toEqual({
      overlay: "g1",
      mode: "old",
    });
  });

  test("Finding 3: TileFrameRouter queues bounded early frames, replays latest to newly mounted tiles, clears empty tiles, and isolates resets", () => {
    const router = new TileFrameRouter<string>();
    router.beginWatch();
    const buf = new ArrayBuffer(16);
    expect(router.enqueue({ kind: "bin", buf })).toBe(true);
    expect(router.enqueue({ kind: "none", key: "k0", none: "wait", retryable: true })).toBe(true);

    // Bounded queue cap enforced
    for (let i = 0; i < MAX_TILE_PENDING_FRAMES + 5; i++) {
      router.enqueue({ kind: "none", key: `k${i}`, none: "x", retryable: false });
    }

    const first = router.commitWatch(["b1"], {
      thread: "t1",
      tiles: { b1: ["0|/p|board.kicad_pcb"] },
      keys: ["0|/p|board.kicad_pcb"],
    });
    expect(first.drained.length).toBe(MAX_TILE_PENDING_FRAMES);
    expect(first.emptyTileIds).toEqual([]);
    expect(first.changedTileIds).toEqual(["b1"]);

    // Record a plot frame and verify newly mounted tile sharing the same key gets immediate replay
    expect(router.recordPlot("0|/p|board.kicad_pcb", "FRAME_BOARD_V1")).toEqual(["b1"]);
    const second = router.commitWatch(["b1", "b2"], {
      thread: "t1",
      tiles: { b1: ["0|/p|board.kicad_pcb"], b2: ["0|/p|board.kicad_pcb"] },
      keys: ["0|/p|board.kicad_pcb"],
    });
    expect(second.emptyTileIds).toEqual([]);
    expect(router.replayForTile("b2")).toEqual({ plots: ["FRAME_BOARD_V1"], nones: [] });

    // Replacement with zero keys reports tile in emptyTileIds so stale artwork is cleared
    const third = router.commitWatch(["b1"], {
      thread: "t1",
      tiles: {},
      keys: [],
    });
    expect(third.emptyTileIds).toEqual(["b1"]);
    expect(router.replayForTile("b1")).toEqual({ plots: [], nones: [] });

    // Thread reset isolates all state
    router.reset();
    expect(router.watchPending).toBe(false);
    expect(router.tileKeys).toEqual({});
  });

  test("Finding 4: unified collapse transition preserves surviving alternate source and handles zero tiles", () => {
    const empty: LayoutReply = {
      thread: "t1",
      rev: 2,
      mode: "full",
      bounds: [1000, 700],
      order: [],
      tiles: [],
      splits: [],
    };
    expect(classifyLayoutTransition(empty)).toEqual({ action: "empty" });

    const altSingle: LayoutReply = {
      ...empty,
      order: ["s2"],
      tiles: [
        {
          id: "s2",
          kind: "schematic",
          source: "sheets/power.kicad_sch",
          focused: true,
          visible: true,
          rect: { x: 0, y: 0, w: 1, h: 1 },
        },
      ],
    };
    expect(isTiledOrOverlayLayout(altSingle)).toBe(false);
    expect(classifyLayoutTransition(altSingle)).toEqual({
      action: "single",
      kind: "schematic",
      source: "sheets/power.kicad_sch",
    });

    const withOverlay: LayoutReply = {
      ...altSingle,
      tiles: [{ ...altSingle.tiles[0], kind: "board", overlay: { kind: "gerbers", source: "mfg/gerbers", mode: "diff" } }],
    };
    expect(classifyLayoutTransition(withOverlay)).toEqual({ action: "grid" });
  });

  test("clamps bounds, splitter ratios, and formats refusals", () => {
    expect(normalizeBounds(0, -5)).toEqual([100, 100]);
    expect(normalizeBounds(1440.4, 900.6)).toEqual([1440, 901]);
    expect(normalizeBounds(20000, Number.NaN)).toEqual([8192, 100]);
    expect(proposedSplitRatio(250, 100, 500)).toBeCloseTo(0.3, 5);
    expect(proposedSplitRatio(0, 100, 500)).toBe(0.1);
    expect(proposedSplitRatio(999, 100, 500)).toBe(0.9);
    expect(isRowSplit("row")).toBe(true);
    expect(isRowSplit("col")).toBe(false);
    expect(formatLayoutError({ error: { code: "tooManyTiles", message: "At most 8 tiles" } })).toBe("At most 8 tiles");
    expect(formatLayoutError(new Error("Layout refused"))).toBe("Layout refused");
  });
});
