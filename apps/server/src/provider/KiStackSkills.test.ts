// @effect-diagnostics nodeBuiltinImport:off
import * as NodeFSP from "node:fs/promises";
import * as NodeOS from "node:os";
import * as NodePath from "node:path";
import { expect, it } from "vite-plus/test";
import { buildKiStackInstructions, installKiStackSkills } from "./KiStackSkills.ts";
import bundle from "./kistack.bundle.json" with { type: "json" };

it("installs all nine skills and supporting files offline, and repairs missing resources", async () => {
  const directory = await NodeFSP.mkdtemp(NodePath.join(NodeOS.tmpdir(), "t3cad-kistack-test-"));
  try {
    await installKiStackSkills(directory);
    expect(bundle.skills).toHaveLength(9);
    for (const [relative, contents] of Object.entries(bundle.files)) {
      expect(await NodeFSP.readFile(NodePath.join(directory, relative), "utf8")).toBe(contents);
    }
    const schematic = NodePath.join(directory, "skills/schematic/SKILL.md");
    const before = (await NodeFSP.stat(schematic)).mtimeMs;
    const helper = NodePath.join(directory, "skills/export/scripts/convert_position.py");
    await NodeFSP.unlink(helper);
    await installKiStackSkills(directory);
    expect((await NodeFSP.stat(schematic)).mtimeMs).toBe(before);
    expect(await NodeFSP.readFile(helper, "utf8")).toBe(
      bundle.files["skills/export/scripts/convert_position.py"],
    );
    const instructions = buildKiStackInstructions(directory);
    for (const skill of bundle.skills) {
      expect(instructions).toContain(JSON.stringify(NodePath.join(directory, skill.path)));
    }
    expect(instructions).toContain("User instructions take precedence");
  } finally {
    await NodeFSP.rm(directory, { recursive: true, force: true });
  }
});

it("reports installation errors instead of advertising unavailable skills", async () => {
  const directory = await NodeFSP.mkdtemp(NodePath.join(NodeOS.tmpdir(), "t3cad-kistack-error-"));
  try {
    const file = NodePath.join(directory, "not-a-directory");
    await NodeFSP.writeFile(file, "existing");
    await expect(installKiStackSkills(file)).rejects.toThrow();
    expect(await NodeFSP.readFile(file, "utf8")).toBe("existing");
  } finally {
    await NodeFSP.rm(directory, { recursive: true, force: true });
  }
});
