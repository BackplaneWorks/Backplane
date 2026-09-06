// @effect-diagnostics nodeBuiltinImport:off
import * as NodeFSP from "node:fs/promises";
import * as NodeOS from "node:os";
import * as NodePath from "node:path";
import { expect, it } from "vite-plus/test";
import { createKiCadModelCache } from "./KiCadModel.ts";

it("shares concurrent exports and never writes outputs beside the source", async () => {
  const root = await NodeFSP.mkdtemp(NodePath.join(NodeOS.tmpdir(), "k3eda-model-test-"));
  const board = NodePath.join(root, "board.kicad_pcb");
  await NodeFSP.writeFile(board, "saved board");
  let calls = 0;
  const cache = createKiCadModelCache(async (source, output) => {
    calls += 1;
    expect(source).toBe(board);
    expect(NodePath.dirname(output)).not.toBe(root);
    await NodeFSP.writeFile(output, "glb");
  });
  try {
    const [first, second] = await Promise.all([cache.get(board, "a"), cache.get(board, "a")]);
    expect(first).toBe(second);
    expect(calls).toBe(1);
    expect(await NodeFSP.readFile(board, "utf8")).toBe("saved board");
    expect(await NodeFSP.readdir(root)).toEqual(["board.kicad_pcb"]);
    await cache.get(board, "b");
    expect(calls).toBe(2);
    await cache.dispose();
    await expect(NodeFSP.stat(NodePath.dirname(first))).rejects.toThrow();
  } finally {
    await cache.dispose();
    await NodeFSP.rm(root, { recursive: true, force: true });
  }
});

it("cleans failed exports and allows a fresh retry", async () => {
  let directory = "";
  const cache = createKiCadModelCache(async (_source, output) => {
    directory = NodePath.dirname(output);
    throw new Error("CLI failed");
  });
  await expect(cache.get("board.kicad_pcb", "a")).rejects.toThrow("CLI failed");
  await expect(NodeFSP.stat(directory)).rejects.toThrow();
  await expect(cache.get("board.kicad_pcb", "a")).rejects.toThrow("CLI failed");
  await cache.dispose();
});
