import { expect, it } from "vite-plus/test";
import {
  clearPrismGerberRenderCache,
  resolvePrismPython,
  renderPrismGerber,
  renderPrismGerberComposite,
} from "./PrismGerber.ts";

it("uses the bundled Python override for packaged Gerber rendering", () => {
  expect(resolvePrismPython({ BACKPLANE_PYTHON: "/opt/backplane/python3" })).toBe(
    "/opt/backplane/python3",
  );
});

it("frames upstream Gerber artwork in SVG coordinates", async () => {
  const svg = await renderPrismGerber(
    "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1*%\nD10*\nX10000000Y20000000D03*\nM02*",
    "front.gtl",
  );
  const bounds = /viewBox="([^"]+)"/.exec(svg)?.[1]?.split(" ").map(Number);
  expect(bounds).toBeDefined();
  const [x, y, width, height] = bounds!;
  expect(x).toBeLessThan(10);
  expect(x! + width!).toBeGreaterThan(10);
  expect(y).toBeLessThan(-20);
  expect(y! + height!).toBeGreaterThan(-20);
  expect(svg).toContain('cy="-20"');
});

it("reports malformed layers instead of showing a blank successful preview", async () => {
  await expect(renderPrismGerber("not a Gerber", "broken.gbr")).rejects.toThrow(
    "Prism could not render",
  );
});

it("shares an in-flight render for identical layer content", async () => {
  clearPrismGerberRenderCache();
  const content = "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1*%\nD10*\nX10000000Y20000000D03*\nM02*";
  const first = renderPrismGerber(content, "front.gtl");
  const second = renderPrismGerber(content, "front.gtl");
  expect(first).toBe(second);
  await first;
});

it("aligns composite layers to one shared viewBox", async () => {
  clearPrismGerberRenderCache();
  const layer = (x: number) =>
    `%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1*%\nD10*\nX${x}Y20000000D03*\nM02*`;
  const svg = await renderPrismGerberComposite([
    { content: layer(10000000), filename: "front.gtl", colour: "#f00" },
    { content: layer(20000000), filename: "back.gbl", colour: "#0f0" },
  ]);
  expect((svg.match(/<svg /g) ?? []).length).toBe(1);
  expect(svg).toContain('fill="#f00"');
  expect(svg).toContain('fill="#0f0"');
  expect(svg).toContain('cx="10"');
  expect(svg).toContain('cx="20"');
  const ids = [...svg.matchAll(/ id="([^"]+)"/g)].map((match) => match[1]);
  expect(new Set(ids).size).toBe(ids.length);
});
