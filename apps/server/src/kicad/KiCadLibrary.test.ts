import { expect, it } from "vite-plus/test";
import { createKiCadLibraryCache } from "./KiCadLibrary.ts";

it("shares a library export and refreshes when the saved revision changes", async () => {
  let runs = 0;
  const cache = createKiCadLibraryCache(async () => {
    runs++;
    return [{ name: "resistor_unit1", svg: "<svg/>" }];
  });
  const [a, b] = await Promise.all([
    cache.get("symbol", "/parts.kicad_sym", "1"),
    cache.get("symbol", "/parts.kicad_sym", "1"),
  ]);
  expect(a).toEqual(b);
  expect(runs).toBe(1);
  await cache.get("symbol", "/parts.kicad_sym", "2");
  expect(runs).toBe(2);
});

it("retries an unsuccessful export", async () => {
  let runs = 0;
  const cache = createKiCadLibraryCache(async () => {
    if (++runs === 1) throw new Error("KiCad unavailable");
    return [{ name: "pad", svg: "<svg/>" }];
  });
  await expect(cache.get("footprint", "/pad.kicad_mod", "1")).rejects.toThrow("KiCad unavailable");
  expect(await cache.get("footprint", "/pad.kicad_mod", "1")).toHaveLength(1);
});
