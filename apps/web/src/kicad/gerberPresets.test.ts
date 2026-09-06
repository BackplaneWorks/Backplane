import { expect, it } from "vite-plus/test";
import { gerberPresetPaths } from "./gerberPresets";

it("keeps presets within a fabrication set and separates top and bottom assembly artwork", () => {
  const paths = [
    "fab/board-F_Cu.gtl",
    "fab/board-B_Cu.gbl",
    "fab/board-In1_Cu.g1",
    "fab/board-F_SilkS.gto",
    "fab/board-B_SilkS.gbo",
    "fab/board-F_Paste.gtp",
    "fab/board-Edge_Cuts.gm1",
    "fab/other-F_Cu.gtl",
    "old/board-F_Cu.gtl",
  ];
  expect(gerberPresetPaths(paths, paths[0]!, "front")).toEqual([paths[3], paths[5], paths[6]]);
  expect(gerberPresetPaths(paths, paths[0]!, "copper")).toEqual([
    paths[0],
    paths[1],
    paths[2],
    paths[6],
  ]);
  expect(gerberPresetPaths(paths, paths[0]!, "back")).toEqual([paths[4], paths[6]]);
});

it("recognizes generic extensions and returns no matching placement rather than unrelated artwork", () => {
  const paths = ["out/main-F_Cu.gbr", "out/main-B_Cu.gbr", "out/main-PTH.drl"];
  expect(gerberPresetPaths(paths, paths[0]!, "copper")).toEqual(paths);
  expect(gerberPresetPaths(["unclassified.gbr"], "unclassified.gbr", "front")).toEqual([]);
});
