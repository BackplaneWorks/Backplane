import type { KiCadProjectConfig, KiCadProjectFile } from "@t3tools/contracts";

const auxiliaryDirectory =
  /(?:^|\/)(?:tools?|vendor|third.party|node_modules|\.git|\.history|\.venv|backups?|archive|examples?|tests?|fixtures?|build|dist|output)(?:\/|$)/i;
const intermediateFile =
  /(?:^|\/)(?:.*[-_.])?(?:backup|routing|routed|unrouted|autosave|copy)(?:[-_.]|$)/i;
const stem = (path: string) => path.replace(/\.kicad_(?:pcb|sch|pro)$/i, "");
const normalize = (path: string) => path.replaceAll("\\", "/").replace(/^\.\//, "");

export function resolveProjectDesign(
  files: readonly KiCadProjectFile[],
  config?: KiCadProjectConfig,
) {
  const find = (path: string | undefined, kind: "pcb" | "schematic") =>
    path ? files.find((file) => file.path === normalize(path) && file.kind === kind) : undefined;
  const assignedBoard = find(config?.pcb, "pcb");
  const assignedSchematic = find(config?.schematic, "schematic");
  if (config?.pcb || config?.schematic) {
    const anchor = assignedBoard ?? assignedSchematic;
    return {
      pcb: config.pcb ? assignedBoard : find(anchor && `${stem(anchor.path)}.kicad_pcb`, "pcb"),
      schematic: config.schematic
        ? assignedSchematic
        : find(anchor && `${stem(anchor.path)}.kicad_sch`, "schematic"),
      assigned: true,
    };
  }
  const candidates = files.filter(
    (file) => !auxiliaryDirectory.test(file.path) && !intermediateFile.test(file.path),
  );
  const projects = candidates.filter((file) => /\.kicad_pro$/i.test(file.path));
  const paired = projects.filter(
    (project) =>
      candidates.some((file) => file.kind === "pcb" && stem(file.path) === stem(project.path)) ||
      candidates.some(
        (file) => file.kind === "schematic" && stem(file.path) === stem(project.path),
      ),
  );
  const roots = new Set(
    (paired.length ? paired : candidates.filter((file) => file.kind === "pcb")).map((file) =>
      stem(file.path),
    ),
  );
  if (roots.size === 0) {
    for (const file of candidates.filter((file) => file.kind === "schematic"))
      roots.add(stem(file.path));
  }
  const root = roots.size === 1 ? [...roots][0] : undefined;
  return {
    pcb: root ? find(`${root}.kicad_pcb`, "pcb") : undefined,
    schematic: root ? find(`${root}.kicad_sch`, "schematic") : undefined,
    assigned: false,
  };
}
