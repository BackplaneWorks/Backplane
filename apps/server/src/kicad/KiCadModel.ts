// @effect-diagnostics nodeBuiltinImport:off globalDate:off
import * as NodeChildProcess from "node:child_process";
import * as NodeFSP from "node:fs/promises";
import * as NodeOS from "node:os";
import * as NodePath from "node:path";
import * as NodeUtil from "node:util";
import { resolveKiCadEnvironment, resolveKiCadExecutable } from "./KiCadExecutable.ts";

import { nameKiCadGlbLayers } from "./KiCadGlb.ts";

const execFile = NodeUtil.promisify(NodeChildProcess.execFile);
const exportModel = async (board: string, output: string) => {
  const env = { ...process.env };
  // Electron AppImage paths must not redirect system KiCad's library lookup.
  delete env.APPDIR;
  delete env.APPIMAGE;
  await execFile(
    resolveKiCadExecutable(env),
    [
      "pcb",
      "export",
      "glb",
      "--subst-models",
      "--include-tracks",
      "--include-pads",
      "--include-zones",
      "--include-silkscreen",
      "--include-soldermask",
      "--cut-vias-in-body",
      "--output",
      output,
      board,
    ],
    {
      cwd: NodePath.dirname(board),
      env: resolveKiCadEnvironment(env),
      timeout: 120_000,
      windowsHide: true,
    },
  );
  await NodeFSP.writeFile(output, nameKiCadGlbLayers(await NodeFSP.readFile(output)));
};

/** Shares in-flight exports across viewers and retains only a small temporary preview cache. */
export function createKiCadModelCache(runExport = exportModel) {
  const entries = new Map<
    string,
    { result: Promise<string>; directory: string | undefined; expiresAt: number; settled: boolean }
  >();
  const remove = async (key: string) => {
    const entry = entries.get(key);
    entries.delete(key);
    if (entry?.directory) await NodeFSP.rm(entry.directory, { recursive: true, force: true });
  };
  return {
    get(board: string, revision: string): Promise<string> {
      const key = `${board}\0${revision}`;
      const existing = entries.get(key);
      if (existing && (!existing.settled || existing.expiresAt > Date.now()))
        return existing.result;
      const entry = {
        result: Promise.resolve(""),
        directory: undefined as string | undefined,
        expiresAt: Date.now() + 600_000,
        settled: false,
      };
      const result = (async () => {
        if (existing) await remove(key);
        for (const [oldKey, old] of entries) {
          if (old.settled && (old.expiresAt <= Date.now() || entries.size >= 8))
            await remove(oldKey);
        }
        if (entries.size >= 8)
          throw new Error("3D preview is busy. Try again after another export finishes.");
        entry.directory = await NodeFSP.mkdtemp(NodePath.join(NodeOS.tmpdir(), "t3-kicad-glb-"));
        const output = NodePath.join(entry.directory, "board.glb");
        try {
          await runExport(board, output);
          entry.settled = true;
          return output;
        } catch (cause) {
          await NodeFSP.rm(entry.directory, { recursive: true, force: true });
          throw cause;
        }
      })();
      entry.result = result.catch((cause) => {
        if (entries.get(key) === entry) entries.delete(key);
        throw cause;
      });
      entries.set(key, entry);
      return entry.result;
    },
    async dispose() {
      await Promise.allSettled([...entries.values()].map((entry) => entry.result));
      await Promise.all([...entries.keys()].map(remove));
    },
  };
}

export const kiCadModelCache = createKiCadModelCache();
