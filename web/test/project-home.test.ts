import { describe, expect, test } from "bun:test";
import { intentIsEmpty, intentLines, parseProjectHome, rawProjectId, sizeText } from "../src/project-home-model";

describe("Project Home model", () => {
  test("parseProjectHome reads the hub's answer and treats anything missing as empty", () => {
    const d = parseProjectHome({
      project: "p1",
      root: "/home/user/lamp",
      intent: "# Goal\n\nA lamp.\n",
      parts: [{ name: "LED board", kind: "pcb", path: "boards/led/led.fab.json" }, { nope: 1 }, "x"],
      boards: ["boards/led/led.kicad_pcb", 7, ""],
      mech: [{ path: "mech/lamp.step", name: "lamp.step", size: 90 }, { path: "", name: "" }],
      leadDays: 9,
      warnings: ["thin board", " fragile ", "thin board", ""],
    });
    expect(d.project).toBe("p1");
    expect(d.root).toBe("/home/user/lamp");
    expect(d.parts).toEqual([{ name: "LED board", kind: "pcb", path: "boards/led/led.fab.json" }]);
    expect(d.boards).toEqual(["boards/led/led.kicad_pcb"]);
    expect(d.mech).toEqual([{ path: "mech/lamp.step", name: "lamp.step", size: 90 }]);
    expect(d.leadDays).toBe(9);
    expect(d.warnings).toEqual(["thin board", "fragile"]);

    for (const raw of [null, undefined, "x", [], {}]) {
      expect(parseProjectHome(raw)).toEqual({
        project: "", root: "", intent: "", parts: [], boards: [], mech: [], leadDays: 0, warnings: [],
      });
    }
  });

  test("a lead time is whole days above zero, else none", () => {
    expect(parseProjectHome({ leadDays: 4.9 }).leadDays).toBe(4);
    for (const bad of [0, -3, "9", null, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(parseProjectHome({ leadDays: bad }).leadDays).toBe(0);
    }
  });

  test("intentLines reads headings, list items and text as the desktop does", () => {
    expect(intentLines("# Goal\n\nA lamp.\n\n# Parts\n\n- LED board\n* Shade\n## Notes")).toEqual([
      { kind: "h", text: "Goal" },
      { kind: "p", text: "A lamp." },
      { kind: "h", text: "Parts" },
      { kind: "li", text: "LED board" },
      { kind: "li", text: "Shade" },
      { kind: "h", text: "Notes" },
    ]);
    expect(intentLines("")).toEqual([]);
    expect(intentLines(Array.from({ length: 80 }, (_, i) => `line ${i}`).join("\n"))).toHaveLength(60);
  });

  test("an intent of headings only says nothing yet", () => {
    expect(intentIsEmpty("# Goal\n\n# Parts\n\n# Notes\n")).toBe(true);
    expect(intentIsEmpty("")).toBe(true);
    expect(intentIsEmpty("# Goal\n\nA lamp.\n")).toBe(false);
    expect(intentIsEmpty("- LED board")).toBe(false);
  });

  test("sizeText and rawProjectId", () => {
    expect(sizeText(0)).toBe("");
    expect(sizeText(512)).toBe("512 B");
    expect(sizeText(2048)).toBe("2.0 KB");
    expect(sizeText(200 * 1024)).toBe("200 KB");
    expect(sizeText(3 * 1024 * 1024)).toBe("3.0 MB");
    expect(rawProjectId("bench|p1")).toBe("p1");
    expect(rawProjectId("p1")).toBe("p1");
    expect(rawProjectId("lap~p9")).toBe("lap~p9");
  });
});
