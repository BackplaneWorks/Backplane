// @effect-diagnostics nodeBuiltinImport:off globalDate:off
import * as NodeChildProcess from "node:child_process";
import * as NodeFSP from "node:fs/promises";
import * as NodeOS from "node:os";
import * as NodePath from "node:path";
import * as NodeUtil from "node:util";
import { resolveKiCadExecutable } from "./KiCadExecutable.ts";

const execFile = NodeUtil.promisify(NodeChildProcess.execFile);
const cleanEnv = () => {
  const env = { ...process.env };
  delete env.APPDIR;
  delete env.APPIMAGE;
  return env;
};

import type { KiCadLibraryMember } from "@t3tools/contracts";
export async function exportKiCadLibrary(
  kind: "footprint" | "symbol",
  input: string,
  name?: string,
): Promise<KiCadLibraryMember[]> {
  const directory = await NodeFSP.mkdtemp(NodePath.join(NodeOS.tmpdir(), "t3-kicad-library-"));
  try {
    const sourceDir = NodePath.join(directory, "source.pretty");
    await NodeFSP.mkdir(sourceDir);
    const source = NodePath.join(sourceDir, NodePath.basename(input));
    await NodeFSP.copyFile(input, source);
    const args =
      kind === "footprint"
        ? [
            "fp",
            "export",
            "svg",
            "--output",
            directory,
            "--footprint",
            NodePath.basename(input, ".kicad_mod"),
            sourceDir,
          ]
        : [
            "sym",
            "export",
            "svg",
            "--output",
            directory,
            ...(name ? ["--symbol", name] : []),
            source,
          ];
    await execFile(resolveKiCadExecutable(), args, {
      cwd: NodePath.dirname(input),
      env: cleanEnv(),
      timeout: 120_000,
      windowsHide: true,
    });
    const files = (await NodeFSP.readdir(directory)).filter((file) =>
      file.toLowerCase().endsWith(".svg"),
    );
    return await Promise.all(
      files.sort().map(async (file) => ({
        name: NodePath.basename(file, ".svg"),
        svg: await NodeFSP.readFile(NodePath.join(directory, file), "utf8"),
      })),
    );
  } finally {
    await NodeFSP.rm(directory, { recursive: true, force: true });
  }
}

export function createKiCadLibraryCache(runExport = exportKiCadLibrary) {
  const entries = new Map<string, { result: Promise<KiCadLibraryMember[]>; expiresAt: number }>();
  return {
    get(kind: "footprint" | "symbol", input: string, revision: string) {
      const key = `${kind}\0${input}\0${revision}`;
      const existing = entries.get(key);
      if (existing && existing.expiresAt > Date.now()) return existing.result;
      for (const [oldKey, old] of entries) {
        if (old.expiresAt <= Date.now()) entries.delete(oldKey);
      }
      if (entries.size >= 8) entries.delete(entries.keys().next().value!);
      const result = runExport(kind, input)
        .then((value) => {
          entries.set(key, { result, expiresAt: Date.now() + 600_000 });
          return value;
        })
        .catch((cause) => {
          entries.delete(key);
          throw cause;
        });
      entries.set(key, { result, expiresAt: Date.now() + 600_000 });
      return result;
    },
    clear() {
      entries.clear();
    },
  };
}
export const kiCadLibraryCache = createKiCadLibraryCache();
