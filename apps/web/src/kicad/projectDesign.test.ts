import { expect, it } from "vite-plus/test";
import type { KiCadProjectFile } from "@t3tools/contracts";
import { resolveProjectDesign } from "./projectDesign";
const file = (path: string): KiCadProjectFile => ({
  path,
  kind: path.endsWith(".kicad_pcb") ? "pcb" : path.endsWith(".kicad_sch") ? "schematic" : "project",
  size: 1,
  mtimeMs: 1,
  mimeType: "text/plain",
});
it("honors assigned files even in output directories and pairs the other view", () => {
  const files = [
    file("build/final.kicad_pcb"),
    file("build/final.kicad_sch"),
    file("hardware/other.kicad_pcb"),
  ];
  const result = resolveProjectDesign(files, { pcb: "./build/final.kicad_pcb" });
  expect(result.pcb?.path).toBe("build/final.kicad_pcb");
  expect(result.schematic?.path).toBe("build/final.kicad_sch");
  expect(result.assigned).toBe(true);
});
it("does not substitute a different design when an assigned file disappears", () => {
  expect(
    resolveProjectDesign([file("other.kicad_pcb")], { pcb: "missing.kicad_pcb" }).pcb,
  ).toBeUndefined();
});
it("infers the actual project without tools, examples, or routing intermediates", () => {
  const result = resolveProjectDesign([
    file("hardware/main.kicad_pro"),
    file("hardware/main.kicad_pcb"),
    file("hardware/main.kicad_sch"),
    file("hardware/main-routing.kicad_pcb"),
    file("build/routing-input.kicad_pcb"),
    file("tools/klc/example.kicad_pro"),
    file("tools/klc/example.kicad_pcb"),
  ]);
  expect(result.pcb?.path).toBe("hardware/main.kicad_pcb");
  expect(result.schematic?.path).toBe("hardware/main.kicad_sch");
});
it("requires a choice when multiple real projects exist", () => {
  const result = resolveProjectDesign([file("a.kicad_pcb"), file("b.kicad_pcb")]);
  expect(result.pcb).toBeUndefined();
});
it("keeps distinct explicit board and schematic assignments", () => {
  const files = [file("pcb/rev-b.kicad_pcb"), file("circuits/root.kicad_sch")];
  const result = resolveProjectDesign(files, { pcb: files[0]!.path, schematic: files[1]!.path });
  expect(result.pcb).toEqual(files[0]);
  expect(result.schematic).toEqual(files[1]);
});
