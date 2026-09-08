// @effect-diagnostics nodeBuiltinImport:off
import * as NodeFS from "node:fs";
import * as NodeOS from "node:os";
import * as NodePath from "node:path";
import { expect, it } from "vite-plus/test";

import { resolveKiCadExecutable } from "./KiCadExecutable.ts";

it("honours an explicit Backplane override", () => {
  expect(resolveKiCadExecutable({ BACKPLANE_KICAD_CLI: "/opt/backplane/kicad-cli" })).toBe(
    "/opt/backplane/kicad-cli",
  );
});

it("prefers the packaged resources runtime", () => {
  const root = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), "backplane-resources-"));
  const executable = NodePath.join(root, "kicad", "bin", "kicad-cli");
  NodeFS.mkdirSync(NodePath.dirname(executable), { recursive: true });
  NodeFS.writeFileSync(executable, "#!/bin/sh\n");
  const original = (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath;
  (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath = root;
  try {
    expect(resolveKiCadExecutable({})).toBe(executable);
  } finally {
    if (original === undefined) {
      delete (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath;
    } else {
      (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath = original;
    }
    NodeFS.rmSync(root, { recursive: true, force: true });
  }
});

it("falls back to PATH for development", () => {
  const original = (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath;
  (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath = NodeFS.mkdtempSync(
    NodePath.join(NodeOS.tmpdir(), "backplane-empty-resources-"),
  );
  expect(resolveKiCadExecutable({})).toBe("kicad-cli");
  NodeFS.rmSync((process as NodeJS.Process & { resourcesPath?: string }).resourcesPath!, {
    recursive: true,
    force: true,
  });
  if (original === undefined) {
    delete (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath;
  } else {
    (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath = original;
  }
});
