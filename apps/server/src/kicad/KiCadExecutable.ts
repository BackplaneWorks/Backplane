/* @effect-diagnostics nodeBuiltinImport:off */
import * as NodeFS from "node:fs";
import * as NodePath from "node:path";

/**
 * Locate the KiCad CLI shipped with Backplane before consulting PATH.
 *
 * The desktop bundle places the fork under resources/kicad. Keeping this
 * lookup here means every export (BOM, SVG, GLB, and schematic parsing) uses
 * the same stable runtime, while development and remote server installs can
 * continue to use an explicitly configured or system KiCad.
 */
export function resolveKiCadExecutable(env: NodeJS.ProcessEnv = process.env): string {
  const configured = env.BACKPLANE_KICAD_CLI?.trim() || env.KICAD_CLI?.trim();
  if (configured) return configured;

  const resourcesPath = (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath;
  const executableName = process.platform === "win32" ? "kicad-cli.exe" : "kicad-cli";
  const roots = [
    resourcesPath,
    env.BACKPLANE_KICAD_ROOT,
    env.APPDIR ? NodePath.join(env.APPDIR, "usr") : undefined,
  ].filter((value): value is string => Boolean(value));

  for (const root of roots) {
    const candidates = [
      NodePath.join(root, "kicad", "bin", executableName),
      NodePath.join(root, "kicad", executableName),
      NodePath.join(root, "bin", executableName),
    ];
    const found = candidates.find((candidate) => {
      try {
        return NodeFS.statSync(candidate).isFile();
      } catch {
        return false;
      }
    });
    if (found) return found;
  }

  // AppImage users may run the server outside Electron. In that case the
  // system path is still useful, but APPDIR must not shadow KiCad's libraries.
  return "kicad-cli";
}

/**
 * Add the standard libraries shipped beside a bundled CLI to its child
 * process environment. Explicit user values always win, including empty
 * values, so system and development KiCad installations keep their normal
 * lookup behavior.
 */
export function resolveKiCadEnvironment(env: NodeJS.ProcessEnv = process.env): NodeJS.ProcessEnv {
  const executable = resolveKiCadExecutable(env);
  if (!NodePath.isAbsolute(executable)) return { ...env };

  const runtimeRoot = NodePath.dirname(NodePath.dirname(executable));
  const bundled = {
    KICAD10_SYMBOL_DIR: NodePath.join(runtimeRoot, "share", "kicad", "symbols"),
    KICAD10_FOOTPRINT_DIR: NodePath.join(runtimeRoot, "share", "kicad", "footprints"),
    KICAD10_3DMODEL_DIR: NodePath.join(runtimeRoot, "share", "kicad", "3dmodels"),
    KICAD10_TEMPLATE_DIR: NodePath.join(runtimeRoot, "share", "kicad", "template"),
  } as const;
  const resolved = { ...env };
  for (const [name, directory] of Object.entries(bundled)) {
    if (resolved[name] === undefined && NodeFS.existsSync(directory)) resolved[name] = directory;
  }
  return resolved;
}
