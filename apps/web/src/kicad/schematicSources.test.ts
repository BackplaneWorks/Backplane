import { expect, it } from "vite-plus/test";
import { loadSchematicSources } from "./schematicSources";

it("excludes unrelated legacy roots that can replace the selected schematic", async () => {
  const files = new Map([
    ["hardware/main.kicad_sch", '(kicad_sch (uuid "main") (symbol (lib_id "Device:R")))'],
    ["tools/example.kicad_sch", '(kicad_sch (sheet_instances (path "/" (page "1"))))'],
  ]);
  const reads: string[] = [];
  const result = await loadSchematicSources(
    "hardware/main.kicad_sch",
    [...files.keys()],
    async (path) => {
      reads.push(path);
      return files.get(path)!;
    },
  );
  expect(reads).toEqual(["hardware/main.kicad_sch"]);
  expect(result.map((source) => source.filename)).toEqual(reads);
});

it("loads nested child sheets relative to each parent, once per file", async () => {
  const files = new Map([
    [
      "hardware/main.kicad_sch",
      '(sheet (property "Sheetfile" "sheets/power.kicad_sch")) (sheet (property "Sheetfile" "sheets/power.kicad_sch"))',
    ],
    ["hardware/sheets/power.kicad_sch", '(sheet (property "Sheetfile" "../main.kicad_sch"))'],
    ["other.kicad_sch", "unrelated"],
  ]);
  const result = await loadSchematicSources(
    "hardware/main.kicad_sch",
    [...files.keys()],
    async (path) => files.get(path)!,
  );
  expect(result.map((source) => source.filename)).toEqual([
    "hardware/main.kicad_sch",
    "hardware/sheets/power.kicad_sch",
  ]);
});

it("reports a missing child instead of silently rendering an incomplete schematic", async () => {
  await expect(
    loadSchematicSources(
      "main.kicad_sch",
      ["main.kicad_sch"],
      async () => '(sheet (property "Sheetfile" "missing.kicad_sch"))',
    ),
  ).rejects.toThrow("Referenced schematic sheet not found: missing.kicad_sch");
});
