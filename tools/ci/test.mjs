import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const folder = mkdtempSync(path.join(os.tmpdir(), "bp-fast-tools-"));
const oxlint = path.join(here, "node_modules/.bin/oxlint");
const tsc = path.join(here, "node_modules/.bin/tsc");
function run(file, args) {
  const result = spawnSync(file, args, { cwd: root, encoding: "utf8", timeout: 5000 });
  assert.ifError(result.error);
  assert.equal(result.signal, null, result.stderr);
  return result;
}
try {
  const rules = [
    "no-constant-binary-expression",
    "no-debugger",
    "no-dupe-keys",
    "no-duplicate-case",
    "no-self-assign",
    "no-sparse-arrays",
    "use-isnan",
  ];
  const config = path.join(folder, "oxlint.json");
  writeFileSync(
    config,
    JSON.stringify({
      categories: { correctness: "off" },
      jsPlugins: [{ name: "backplane", specifier: path.join(here, "duplicate-parameters.mjs") }],
      rules: Object.fromEntries([
        ...rules.map((r) => [r, "error"]),
        ["backplane/no-dupe-args", "error"],
      ]),
    }),
  );
  const cases = {
    "no-constant-binary-expression": "const a = {} || b;",
    "no-debugger": "debugger;",
    "no-dupe-args": "function f(a, a) { return a; }",
    "no-dupe-keys": "const a = {x:1,x:2};",
    "no-duplicate-case": "switch(x){case 1:break;case 1:break;}",
    "no-self-assign": "x=x;",
    "no-sparse-arrays": "const a=[,];",
    "use-isnan": "if(x===NaN){}",
  };
  const files = Object.entries(cases).map(([rule, body]) => {
    const file = path.join(folder, rule + ".cjs");
    writeFileSync(file, body + "\n");
    return file;
  });
  const bad = run(oxlint, [
    "--config",
    config,
    "--no-ignore",
    "--deny-warnings",
    "--format",
    "json",
    ...files,
  ]);
  assert.notEqual(bad.status, 0, "mutated lint cases were accepted");
  const report = JSON.parse(bad.stdout);
  for (const rule of Object.keys(cases)) {
    assert(
      report.diagnostics.some((d) => d.code.endsWith(`(${rule})`)),
      `missing negative control ${rule}`,
    );
  }
  assert.equal(report.number_of_files, files.length);
  assert.equal(report.number_of_rules, 8, "requested rules were silently dropped");

  const good = path.join(folder, "good.cjs");
  writeFileSync(
    good,
    [
      "function declaration(a,b){var a; return a+b;}",
      "const expression=function(a,b){return a+b;};",
      "function nested(a){return function inner(a){return a;};}",
      "function patterns({x:a},{x:b},[c,,d],...rest){return [a,b,c,d,rest];}",
      "function defaults(a=1,b=2){return a+b;}",
      "const arrow=(a,b)=>a+b;",
    ].join("\n"),
  );
  const positive = run(oxlint, [
    "--config",
    config,
    "--no-ignore",
    "--deny-warnings",
    "--format",
    "json",
    good,
  ]);
  assert.equal(positive.status, 0, positive.stdout + positive.stderr);
  assert.equal(JSON.parse(positive.stdout).diagnostics.length, 0);

  const tsconfig = path.join(folder, "tsconfig.json");
  const sources = ["step2glb.ts", "check-glb.ts", "types.d.ts"].map((f) =>
    path.join(root, "tools/step2glb", f),
  );
  const options = {
    extends: path.join(root, "tools/step2glb/tsconfig.json"),
    compilerOptions: { typeRoots: [path.join(here, "node_modules/@types")] },
    files: sources,
    include: [],
  };
  writeFileSync(tsconfig, JSON.stringify(options));
  const typed = run(tsc, ["--project", tsconfig, "--pretty", "false"]);
  assert.equal(typed.status, 0, typed.stdout + typed.stderr);
  const wrong = path.join(folder, "bad.ts");
  writeFileSync(wrong, "const mustBeString: string = 123;\n");
  options.files = [...sources, wrong];
  writeFileSync(tsconfig, JSON.stringify(options));
  const invalid = run(tsc, ["--project", tsconfig, "--pretty", "false"]);
  assert.notEqual(invalid.status, 0, "real type error was accepted");
  assert(invalid.stdout.includes("TS2322"), invalid.stdout + invalid.stderr);

  // Formatting writes only to this temporary fixture, never application files.
  const fmt = path.join(here, "node_modules/.bin/oxfmt");
  const ugly = path.join(folder, "format.js");
  writeFileSync(ugly, "const x={a:1,b:[2,3]}\n");
  assert.notEqual(run(fmt, ["--check", ugly]).status, 0);
  assert.equal(run(fmt, ["--write", ugly]).status, 0);
  assert.equal(run(fmt, ["--check", ugly]).status, 0);
  console.log(
    "ok eight lint controls, valid binding scopes, strict project/type-error and format controls",
  );
} finally {
  rmSync(folder, { recursive: true, force: true });
}
