// @effect-diagnostics nodeBuiltinImport:off globalDate:off
import * as NodeChildProcess from "node:child_process";
import * as NodeFSP from "node:fs/promises";
import * as NodeOS from "node:os";
import * as NodePath from "node:path";
import * as NodeUtil from "node:util";
import type { KiCadBom } from "@t3tools/contracts";
import { discoverKiCadProject, resolveKiCadProjectFile } from "./KiCadProject.ts";
import { resolveKiCadExecutable } from "./KiCadExecutable.ts";

const execFile = NodeUtil.promisify(NodeChildProcess.execFile);
const record = (value: unknown): Record<string, unknown> =>
  value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

/** Reads KiCad's configurable delimiters, including escaped quotes and multiline cells. */
export function parseBomTable(content: string, delimiter = ",", quote = '"'): string[][] {
  if (!delimiter || delimiter === quote)
    throw new Error("BOM field and string delimiters must differ.");
  const rows: string[][] = [];
  let row: string[] = [],
    cell = "",
    quoted = false;
  for (let index = 0; index < content.length;) {
    if (quote && content.startsWith(quote, index) && (quoted || cell === "")) {
      if (quoted && content.startsWith(quote + quote, index)) {
        cell += quote;
        index += quote.length * 2;
      } else {
        quoted = !quoted;
        index += quote.length;
      }
    } else if (!quoted && content.startsWith(delimiter, index)) {
      row.push(cell);
      cell = "";
      index += delimiter.length;
    } else if (!quoted && (content[index] === "\n" || content[index] === "\r")) {
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
      index += content.startsWith("\r\n", index) ? 2 : 1;
    } else {
      cell += content[index];
      index++;
    }
  }
  if (quoted) throw new Error("KiCad BOM contains an unterminated quoted field.");
  if (cell || row.length) rows.push([...row, cell]);
  return rows;
}

/** Names copies of the active settings, including edits that no longer match a named preset. */
export function prepareBomProject(value: unknown) {
  const project = record(value);
  const schematic = record(project.schematic);
  const settings = record(schematic.bom_settings);
  const format = record(schematic.bom_fmt_settings);
  const hasSettings = Array.isArray(settings.fields_ordered) && settings.fields_ordered.length > 0;
  const hasFormat = Object.keys(format).length > 0;
  const args: string[] = [];
  if (hasSettings) args.push("--preset", "T3CAD saved settings");
  if (hasFormat) args.push("--format-preset", "T3CAD saved format");
  return {
    project: {
      ...project,
      schematic: {
        ...schematic,
        ...(hasSettings ? { bom_presets: [{ ...settings, name: "T3CAD saved settings" }] } : {}),
        ...(hasFormat ? { bom_fmt_presets: [{ ...format, name: "T3CAD saved format" }] } : {}),
      },
    },
    args,
    // The active settings can be edited after a named preset was selected.
    // Report the export as current settings instead of surfacing that stale name.
    preset: hasSettings ? "Current saved settings" : "KiCad defaults",
    delimiter: typeof format.field_delimiter === "string" ? format.field_delimiter : ",",
    quote: typeof format.string_delimiter === "string" ? format.string_delimiter : '"',
    hasSettings,
  };
}

const runExport = async (args: string[], cwd: string) => {
  const env = { ...process.env };
  delete env.APPDIR;
  delete env.APPIMAGE;
  await execFile(resolveKiCadExecutable(env), args, {
    cwd,
    env,
    timeout: 120_000,
    windowsHide: true,
  });
};

export function createKiCadBomCache(run = runExport) {
  const entries = new Map<string, Promise<KiCadBom>>();
  return {
    get(root: string, path: string, revision: string): Promise<KiCadBom> {
      const key = `${root}\0${path}\0${revision}`;
      const existing = entries.get(key);
      if (existing) return existing;
      if (entries.size >= 16) entries.delete(entries.keys().next().value!);
      const result = (async (): Promise<KiCadBom> => {
        const asset = await resolveKiCadProjectFile(root, path);
        if (asset?.file.kind !== "schematic")
          throw new Error("Select a saved schematic for the BOM.");
        const manifest = await discoverKiCadProject(root);
        const projectPath = path.replace(/\.kicad_sch$/i, ".kicad_pro");
        const projectAsset = await resolveKiCadProjectFile(root, projectPath);
        const prepared = prepareBomProject(
          projectAsset ? JSON.parse(await NodeFSP.readFile(projectAsset.absolutePath, "utf8")) : {},
        );
        const directory = await NodeFSP.mkdtemp(NodePath.join(NodeOS.tmpdir(), "t3cad-bom-"));
        try {
          // Preserve relative sheet paths and project variables without writing alongside user files.
          for (const file of manifest.files.filter((item) => item.kind === "schematic")) {
            const source = await resolveKiCadProjectFile(root, file.path);
            if (!source) throw new Error(`Schematic disappeared during BOM export: ${file.path}`);
            const destination = NodePath.join(directory, file.path);
            await NodeFSP.mkdir(NodePath.dirname(destination), { recursive: true });
            await NodeFSP.copyFile(source.absolutePath, destination);
          }
          await NodeFSP.writeFile(
            NodePath.join(directory, projectPath),
            JSON.stringify(prepared.project),
          );
          const output = NodePath.join(directory, "t3cad-bom.csv");
          await run(
            [
              "sch",
              "export",
              "bom",
              ...prepared.args,
              "--output",
              output,
              NodePath.join(directory, path),
            ],
            NodePath.dirname(NodePath.join(directory, path)),
          );
          const content = await NodeFSP.readFile(output, "utf8");
          const [columns = [], ...rows] = parseBomTable(
            content,
            prepared.delimiter,
            prepared.quote,
          );
          return {
            columns,
            rows,
            content,
            preset: prepared.preset,
            sourceProject: projectAsset ? projectPath : null,
            warnings: prepared.hasSettings
              ? []
              : ["No saved BOM settings found for this schematic; using KiCad defaults."],
          };
        } finally {
          await NodeFSP.rm(directory, { recursive: true, force: true });
        }
      })().catch((error: unknown) => {
        entries.delete(key);
        throw error;
      });
      entries.set(key, result);
      return result;
    },
  };
}

export const kiCadBomCache = createKiCadBomCache();
