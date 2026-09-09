// @effect-diagnostics nodeBuiltinImport:off globalDate:off
import * as NodeFSP from "node:fs/promises";
import * as NodeOS from "node:os";
import * as NodePath from "node:path";
import { expect, it, vi } from "vite-plus/test";
import { copyKiCadMetadata } from "./KiCadMetadata.ts";
import { discoverKiCadProject } from "./KiCadProject.ts";

it("copies companion data into staged exports and excludes symlink companions", async () => {
  const root = await NodeFSP.mkdtemp(NodePath.join(NodeOS.tmpdir(), "backplane-metadata-"));
  const source = NodePath.join(root, "board.kicad_pcb");
  const destination = NodePath.join(root, "copy.kicad_pcb");
  try {
    await copyKiCadMetadata(source, destination);
    await NodeFSP.writeFile(`${source}.backplane.json`, "metadata");
    await copyKiCadMetadata(source, destination);
    expect(await NodeFSP.readFile(`${destination}.backplane.json`, "utf8")).toBe("metadata");
    await NodeFSP.unlink(`${source}.backplane.json`);
    await NodeFSP.unlink(`${destination}.backplane.json`);
    await NodeFSP.symlink(`${destination}.private`, `${source}.backplane.json`);
    await NodeFSP.writeFile(`${destination}.private`, "private");
    await copyKiCadMetadata(source, destination);
    await expect(NodeFSP.stat(`${destination}.backplane.json`)).rejects.toThrow();
  } finally {
    await NodeFSP.rm(root, { recursive: true, force: true });
  }
});

it("refreshes project revisions for metadata edits without listing companions as viewer files", async () => {
  const root = await NodeFSP.mkdtemp(NodePath.join(NodeOS.tmpdir(), "backplane-metadata-"));
  const board = NodePath.join(root, "board.kicad_pcb");
  try {
    await NodeFSP.writeFile(board, "board");
    await NodeFSP.writeFile(`${board}.backplane.json`, "first");
    const before = await discoverKiCadProject(root);
    await NodeFSP.writeFile(`${board}.backplane.json`, "second metadata");
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 500);
    const after = await discoverKiCadProject(root);
    expect(after.revision).not.toBe(before.revision);
    expect(after.files.map(({ path }) => path)).toEqual(["board.kicad_pcb"]);
  } finally {
    vi.restoreAllMocks();
    await NodeFSP.rm(root, { recursive: true, force: true });
  }
});
