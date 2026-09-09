// @effect-diagnostics nodeBuiltinImport:off
import * as NodeFSP from "node:fs/promises";

/** Preserve fork-only data when staging a file that also opens in stock KiCad. */
export async function copyKiCadMetadata(source: string, destination: string): Promise<void> {
  const companion = `${source}.backplane.json`;
  try {
    // Companion files must remain beside their native file, including in isolated exports.
    if (!(await NodeFSP.lstat(companion)).isFile()) return;
    await NodeFSP.copyFile(companion, `${destination}.backplane.json`);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return;
    throw error;
  }
}
