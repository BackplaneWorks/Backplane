// The JS service's compile-time bake (scripts/build-js.sh): the prelude
// scripts/*-header.py write as .js, read by the JS twins of google.c and
// telemetry.c, and telemetry's proof of work answering as tf_pow_run does.
//   bun test/tools/js_bake_test.ts
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = join(import.meta.dir, "../..");
const effs: Record<string, Function> = {};
function load(file: string) {
  // bend puts each CID(Name) in as the constructor's id; here, its name
  const text = readFileSync(join(root, "src/server/effects", file), "utf8")
    .replace(/CID\(([A-Za-z0-9_.]+)\)/g, '"$1"');
  new Function("io_eff", "require", text)((id: string, f: Function) => { effs[id] = f; }, require);
}
let fails = 0;
function same(what: string, got: unknown, want: unknown) {
  if (got !== want) { fails++; console.log(`FAIL ${what}: ${JSON.stringify(got)} != ${JSON.stringify(want)}`); }
}

load("google.js");
load("telemetry.js");
delete (globalThis as any).BACKPLANE_BAKED;
same("unbaked google", effs["Gf.baked"]("BACKPLANE_GOOGLE_CLIENT_ID"), "");
same("unbaked telemetry", effs["Tf.baked"]("BACKPLANE_TELEMETRY_URL"), "");

const dir = mkdtempSync(join(tmpdir(), "bp-bake-"));
const env = { ...process.env, BACKPLANE_GOOGLE_CLIENT_ID: "id-1.apps", BACKPLANE_GOOGLE_CLIENT_SECRET: "sec_2",
  BACKPLANE_GOOGLE_BAKE: "1", BACKPLANE_TELEMETRY_BAKE: "0" };
execFileSync("python3", [join(root, "scripts/google-header.py"), join(dir, "g.js")], { env });
execFileSync("python3", [join(root, "scripts/telemetry-header.py"), join(dir, "t.js")], { env });
for (const f of ["t.js", "g.js"]) new Function(readFileSync(join(dir, f), "utf8"))();
same("baked id", effs["Gf.baked"]("BACKPLANE_GOOGLE_CLIENT_ID"), "id-1.apps");
same("baked secret", effs["Gf.baked"]("BACKPLANE_GOOGLE_CLIENT_SECRET"), "sec_2");
same("google reads only its names", effs["Gf.baked"]("BACKPLANE_TELEMETRY_KEY"), "");
same("telemetry off when not baked", effs["Tf.baked"]("BACKPLANE_TELEMETRY_KEY"), "");
same("telemetry reads only its names", effs["Tf.baked"]("BACKPLANE_GOOGLE_CLIENT_SECRET"), "");

const os = process.platform === "darwin" ? "darwin" : process.platform === "win32" ? "windows" : "linux";
same("platform", effs["Tf.platform"](), os + "-" + (process.arch === "arm64" ? "arm64" : "x64"));

// tf_pow_run: the first counter i, in hex, whose SHA-256 of
// "bp-pow1:<id>:<hex>" has its first word >> 14 == 0
const id = "0123abcd-0123-4abc-8def-0123456789ab";
let want = "";
for (let i = 0; ; i++) {
  const h = createHash("sha256").update(`bp-pow1:${id}:${i.toString(16)}`).digest();
  if ((h.readUInt32BE(0) >>> 14) === 0) { want = i.toString(16); break; }
}
same("pow", effs["Tf.pow"](id), want);
same("pow needs a 36-char id", effs["Tf.pow"]("short"), "");

if (fails) { console.log(`${fails} failed`); process.exit(1); }
console.log("js bake: ok");
