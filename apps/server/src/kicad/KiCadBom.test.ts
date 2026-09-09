// @effect-diagnostics nodeBuiltinImport:off
import * as NodeFSP from "node:fs/promises";
import * as NodeOS from "node:os";
import * as NodePath from "node:path";
import { expect, it } from "vite-plus/test";
import { createKiCadBomCache, parseBomTable, prepareBomProject } from "./KiCadBom.ts";

it("reads quoted custom-delimiter BOMs without losing multiline cells or empty columns", () => {
  expect(
    parseBomTable('"Ref";"Notes";"Empty"\r\n"R1,R2";"A ""quote""\nand newline";""\r\n', ";"),
  ).toEqual([
    ["Ref", "Notes", "Empty"],
    ["R1,R2", 'A "quote"\nand newline', ""],
  ]);
  expect(parseBomTable("Ref\tValue\nR1\t10k", "\t", "")).toEqual([
    ["Ref", "Value"],
    ["R1", "10k"],
  ]);
  expect(() => parseBomTable('"broken')).toThrow("unterminated");
});

it("uses the active edited settings rather than an outdated named preset", () => {
  const fields = [{ name: "MPN", label: "Part number", show: true, group_by: true }];
  const prepared = prepareBomProject({
    schematic: {
      bom_settings: {
        name: "Assembly",
        fields_ordered: fields,
        group_symbols: true,
        exclude_dnp: true,
        filter_string: "R*",
        sort_asc: false,
      },
      bom_presets: [{ name: "Assembly", fields_ordered: [] }],
      bom_fmt_settings: { field_delimiter: ";", string_delimiter: '"', keep_line_breaks: true },
    },
  });
  expect(prepared.project.schematic.bom_presets).toEqual([
    {
      name: "T3CAD saved settings",
      fields_ordered: fields,
      group_symbols: true,
      exclude_dnp: true,
      filter_string: "R*",
      sort_asc: false,
    },
  ]);
  expect(prepared.delimiter).toBe(";");
  expect(prepared.preset).toBe("Current saved settings");
  expect(prepared.args).toEqual([
    "--preset",
    "T3CAD saved settings",
    "--format-preset",
    "T3CAD saved format",
  ]);
});

it("shares exports per revision, preserves source settings, and removes temporary files", async () => {
  const root = await NodeFSP.mkdtemp(NodePath.join(NodeOS.tmpdir(), "t3cad-bom-test-"));
  const original =
    '{"schematic":{"bom_settings":{"fields_ordered":[{"name":"Reference","show":true}]}}}';
  let runs = 0;
  let outputPath = "";
  try {
    await NodeFSP.writeFile(NodePath.join(root, "board.kicad_sch"), "(kicad_sch)");
    await NodeFSP.writeFile(NodePath.join(root, "board.kicad_pro"), original);
    const metadata = '{"format":"backplane-kicad-metadata","version":1,"items":{}}';
    await NodeFSP.writeFile(NodePath.join(root, "board.kicad_sch.backplane.json"), metadata);
    const cache = createKiCadBomCache(async (args, cwd) => {
      runs++;
      expect(cwd).not.toBe(root);
      expect(await NodeFSP.readFile(NodePath.join(cwd, "board.kicad_sch"), "utf8")).toBe(
        "(kicad_sch)",
      );
      expect(
        await NodeFSP.readFile(NodePath.join(cwd, "board.kicad_sch.backplane.json"), "utf8"),
      ).toBe(metadata);
      outputPath = args[args.indexOf("--output") + 1]!;
      await NodeFSP.writeFile(outputPath, '"Ref","Value"\n"R1","10k"\n');
    });
    const [a, b] = await Promise.all([
      cache.get(root, "board.kicad_sch", "1"),
      cache.get(root, "board.kicad_sch", "1"),
    ]);
    expect(a).toEqual(b);
    expect(a.rows).toEqual([["R1", "10k"]]);
    expect(runs).toBe(1);
    expect(await NodeFSP.readFile(NodePath.join(root, "board.kicad_pro"), "utf8")).toBe(original);
    await expect(NodeFSP.stat(outputPath)).rejects.toThrow();
    await cache.get(root, "board.kicad_sch", "2");
    expect(runs).toBe(2);
    await expect(cache.get(root, "../outside.kicad_sch", "1")).rejects.toThrow(
      "Select a saved schematic",
    );
  } finally {
    await NodeFSP.rm(root, { recursive: true, force: true });
  }
});
