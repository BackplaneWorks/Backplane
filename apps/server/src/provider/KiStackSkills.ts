// @effect-diagnostics nodeBuiltinImport:off
import * as NodeFSP from "node:fs/promises";
import * as NodeOS from "node:os";
import * as NodePath from "node:path";
import bundle from "./kistack.bundle.json" with { type: "json" };

// Imported JSON is bundled into the server, including packaged desktop/remote builds.
export const kiStackSkillsDirectory = NodePath.join(
  NodeOS.homedir(),
  ".cache",
  "t3cad",
  "kistack",
  bundle.revision,
);

/** Restore the pinned, app-owned copy without modifying provider homes or project skills. */
export async function installKiStackSkills(directory = kiStackSkillsDirectory): Promise<void> {
  for (const [relativePath, contents] of Object.entries(bundle.files)) {
    const path = NodePath.join(directory, relativePath);
    await NodeFSP.mkdir(NodePath.dirname(path), { recursive: true });
    // Preserve mtimes for unchanged resources and repair missing or edited bundled files.
    const existing = await NodeFSP.readFile(path, "utf8").catch((cause: unknown) => {
      if (cause && typeof cause === "object" && "code" in cause && cause.code === "ENOENT")
        return undefined;
      throw cause;
    });
    if (existing !== contents) await NodeFSP.writeFile(path, contents);
  }
}

export function buildKiStackInstructions(directory = kiStackSkillsDirectory): string {
  return [
    "<kistack_skills>",
    `T3CAD includes KiStack by American Embedded (${bundle.source}, revision ${bundle.revision}). These skills are always available in every project.`,
    "For relevant electronics work, read the matching SKILL.md before working and follow its workflow. Resolve referenced scripts and documents relative to that skill's directory. User instructions take precedence. Other installed skills remain available.",
    ...bundle.skills.map(
      (skill) =>
        `- ${skill.name}: ${skill.description} Read ${JSON.stringify(NodePath.join(directory, skill.path))}`,
    ),
    "</kistack_skills>",
  ].join("\n");
}
