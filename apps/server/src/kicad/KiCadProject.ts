/* eslint-disable t3code/namespace-node-imports */
// @effect-diagnostics nodeBuiltinImport:off globalDate:off
import * as NodeCrypto from "node:crypto";
import * as NodeFSP from "node:fs/promises";
import type { Dirent } from "node:fs";
import * as NodePath from "node:path";

export type KiCadFileKind =
  | "gerber"
  | "pcb"
  | "schematic"
  | "model"
  | "project"
  | "footprint"
  | "symbol";

export interface KiCadProjectFile {
  readonly path: string;
  readonly kind: KiCadFileKind;
  readonly mimeType: string;
  readonly size: number;
  readonly mtimeMs: number;
}

export interface KiCadProjectManifest {
  readonly root: string;
  readonly revision: string;
  readonly files: readonly KiCadProjectFile[];
  readonly config?: KiCadProjectConfig;
  readonly warnings: readonly string[];
}
export interface KiCadProjectConfig {
  readonly analysisUrl?: string;
  readonly pcb?: string;
  readonly schematic?: string;
  readonly gerbers?: readonly string[];
  readonly symbol?: string;
  readonly symbolMember?: string;
  readonly footprint?: string;
}

const manifestCache = new Map<
  string,
  { readonly expiresAt: number; readonly manifest: KiCadProjectManifest }
>();

const MIME_TYPES: Record<string, string> = {
  ".gbr": "application/octet-stream",
  ".ger": "application/octet-stream",
  ".drl": "application/octet-stream",
  ".xln": "application/octet-stream",
  ".kicad_mod": "application/x-kicad-footprint",
  ".kicad_sym": "application/x-kicad-symbol",
  ".kicad_pcb": "application/x-kicad-pcb",
  ".kicad_sch": "application/x-kicad-schematic",
  ".step": "model/step",
  ".stp": "model/step",
  ".wrl": "model/vrml",
  ".glb": "model/gltf-binary",
  ".gltf": "model/gltf+json",
  ".obj": "text/plain",
  ".kicad_pro": "application/json",
  ".kicad_wks": "application/x-kicad-workbook",
};
const GERBER_EXTENSIONS = new Set([
  ".gbr",
  ".ger",
  ".gtl",
  ".gbl",
  ".gts",
  ".gbs",
  ".gta",
  ".gba",
  ".gto",
  ".gbo",
  ".gtp",
  ".gbp",
  ".gm1",
  ".gm2",
  ".gm3",
  ".gm13",
  ".gko",
  ".g1",
  ".g2",
  ".g3",
  ".g4",
  ".gbrjob",
  ".drl",
  ".xln",
]);
const MODEL_EXTENSIONS = new Set([".step", ".stp", ".wrl", ".glb", ".gltf", ".obj"]);
const IGNORED_DIRECTORIES = new Set([".git", ".history", "node_modules"]);

function fileKind(extension: string): KiCadFileKind | undefined {
  if (extension === ".kicad_mod") return "footprint";
  if (extension === ".kicad_sym") return "symbol";
  if (extension === ".kicad_pcb") return "pcb";
  if (extension === ".kicad_sch") return "schematic";
  if (extension === ".kicad_pro" || extension === ".kicad_wks") return "project";
  if (GERBER_EXTENSIONS.has(extension) || /^\.g\d+$/.test(extension)) return "gerber";
  if (MODEL_EXTENSIONS.has(extension)) return "model";
  return undefined;
}

/** Walks the project without consulting ignore files: generated fabrication output is often ignored. */
export async function discoverKiCadProject(root: string): Promise<KiCadProjectManifest> {
  const projectRoot = NodePath.resolve(root);
  const cached = manifestCache.get(projectRoot);
  if (cached && cached.expiresAt > Date.now()) return cached.manifest;
  const files: KiCadProjectFile[] = [];
  const metadataRevisions: string[] = [];
  let visited = 0;
  let scanLimitWarningAdded = false;
  let config: KiCadProjectConfig | undefined;
  const warnings: string[] = [];
  try {
    const parsed = JSON.parse(
      await NodeFSP.readFile(NodePath.join(projectRoot, ".k3eda.json"), "utf8"),
    ) as Record<string, unknown>;
    config = {
      ...(typeof parsed.analysisUrl === "string" ? { analysisUrl: parsed.analysisUrl } : {}),
      ...(typeof parsed.pcb === "string" ? { pcb: parsed.pcb } : {}),
      ...(typeof parsed.schematic === "string" ? { schematic: parsed.schematic } : {}),
      ...(Array.isArray(parsed.gerbers)
        ? { gerbers: parsed.gerbers.filter((value): value is string => typeof value === "string") }
        : {}),
      ...(typeof parsed.symbol === "string" ? { symbol: parsed.symbol } : {}),
      ...(typeof parsed.symbolMember === "string" ? { symbolMember: parsed.symbolMember } : {}),
      ...(typeof parsed.footprint === "string" ? { footprint: parsed.footprint } : {}),
    };
  } catch {
    try {
      if ((await NodeFSP.stat(NodePath.join(projectRoot, ".k3eda.json"))).isFile())
        warnings.push("Unable to parse .k3eda.json");
    } catch {
      /* configuration is optional */
    }
  }
  const walk = async (directory: string): Promise<void> => {
    if (++visited > 50_000) {
      if (!scanLimitWarningAdded) {
        warnings.push("KiCad project scan limit reached");
        scanLimitWarningAdded = true;
      }
      return;
    }
    let entries: Dirent<string>[];
    try {
      entries = await NodeFSP.readdir(directory, { withFileTypes: true, encoding: "utf8" });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        if (!IGNORED_DIRECTORIES.has(entry.name) && entry.name !== ".venv")
          await walk(NodePath.join(directory, entry.name));
        continue;
      }
      const extension = entry.name.slice(entry.name.lastIndexOf(".")).toLowerCase();
      const kind = fileKind(extension);
      const metadata = /\.kicad_(?:pcb|sch|mod|sym)\.backplane\.json$/i.test(entry.name);
      if (!kind && !metadata) continue;
      const absolute = NodePath.join(directory, entry.name);
      try {
        const info = await NodeFSP.lstat(absolute);
        if (info.isSymbolicLink()) continue;
        if (!info.isFile()) continue;
        if (metadata) {
          metadataRevisions.push(
            `${NodePath.relative(projectRoot, absolute)}\0${info.size}\0${info.mtimeMs}`,
          );
          continue;
        }
        if (!kind) continue;
        files.push({
          path: NodePath.relative(projectRoot, absolute).split(NodePath.sep).join("/"),
          kind,
          mimeType: MIME_TYPES[extension] ?? "application/octet-stream",
          size: info.size,
          mtimeMs: info.mtimeMs,
        });
      } catch {
        /* files can disappear while an agent is writing them */
      }
    }
  };
  await walk(projectRoot);
  files.sort((a, b) => a.path.localeCompare(b.path));
  if (config?.pcb && !files.some((file) => file.path === config!.pcb))
    warnings.push(`Configured PCB file not found: ${config.pcb}`);
  if (config?.schematic && !files.some((file) => file.path === config!.schematic))
    warnings.push(`Configured schematic file not found: ${config.schematic}`);
  if (
    config?.symbol &&
    !files.some((file) => file.path === config!.symbol && file.kind === "symbol")
  )
    warnings.push(`Configured symbol library not found: ${config.symbol}`);
  if (
    config?.footprint &&
    !files.some((file) => file.path === config!.footprint && file.kind === "footprint")
  )
    warnings.push(`Configured footprint file not found: ${config.footprint}`);
  for (const directory of config?.gerbers ?? [])
    if (
      !files.some(
        (file) =>
          file.kind === "gerber" &&
          (file.path === directory || file.path.startsWith(`${directory.replaceAll("\\", "/")}/`)),
      )
    )
      warnings.push(`Configured Gerber directory not found: ${directory}`);
  const configFingerprint = config ? JSON.stringify(config) : "";
  const revision = NodeCrypto.createHash("sha256")
    .update(
      `${configFingerprint}\n${JSON.stringify(warnings)}\n${files.map((file) => `${file.path}\0${file.size}\0${file.mtimeMs}`).join("\n")}`,
    )
    .update(metadataRevisions.sort().join("\n"))
    .digest("hex")
    .slice(0, 16);
  const manifest = { root: projectRoot, revision, files, ...(config ? { config } : {}), warnings };
  manifestCache.set(projectRoot, { manifest, expiresAt: Date.now() + 300 });
  return manifest;
}

export async function resolveKiCadProjectFile(
  root: string,
  requestedPath: string,
): Promise<{ absolutePath: string; file: KiCadProjectFile } | undefined> {
  const manifest = await discoverKiCadProject(root);
  const normalized = requestedPath.replaceAll("\\", "/");
  const file = manifest.files.find((candidate) => candidate.path === normalized);
  if (!file) return undefined;
  const absolutePath = NodePath.resolve(manifest.root, file.path);
  try {
    const [canonicalRoot, canonicalPath] = await Promise.all([
      NodeFSP.realpath(manifest.root),
      NodeFSP.realpath(absolutePath),
    ]);
    const rootPrefix = canonicalRoot.endsWith(NodePath.sep)
      ? canonicalRoot
      : `${canonicalRoot}${NodePath.sep}`;
    if (!canonicalPath.startsWith(rootPrefix) || !(await NodeFSP.lstat(canonicalPath)).isFile())
      return undefined;
    return { absolutePath: canonicalPath, file };
  } catch {
    return undefined;
  }
}
