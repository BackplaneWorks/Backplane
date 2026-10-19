// End to end: failed STEP conversions never become reusable cached models.
// Synthetic converters and original GLB bytes; no CAD install or live hub.
//
//   bun test/tools/model_cache_e2e.ts BINARY
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";

const [binary = "build/backplane"] = process.argv.slice(2);
const root = realpathSync(mkdtempSync(join(tmpdir(), "bp-model-cache-e2e-")));
const prefix = join(root, "prefix");
const children = new Set<ReturnType<typeof Bun.spawn>>();
let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 500)}`);
  if (!ok) failed = true;
};
async function within<T>(promise: Promise<T>, ms: number): Promise<T | undefined> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([promise, new Promise<undefined>((resolve) => { timer = setTimeout(() => resolve(undefined), ms); })]); }
  finally { if (timer !== undefined) clearTimeout(timer); }
}
async function stop(proc: ReturnType<typeof Bun.spawn>) {
  if (proc.exitCode === null && proc.signalCode === null) proc.kill("SIGTERM");
  let exited = await within(proc.exited, 3000);
  if (exited === undefined) { proc.kill("SIGKILL"); exited = await within(proc.exited, 2000); }
  if (exited !== undefined) children.delete(proc);
  return exited !== undefined;
}
function model(): Buffer {
  const a = [0, 0, 0], b = [0.04, 0, 0], c = [0, 0.03, 0], d = [0, 0, 0.02];
  const vertices = [a, c, b, a, b, d, a, d, c, b, c, d].flat();
  const data = Buffer.alloc(vertices.length * 4);
  vertices.forEach((value, i) => data.writeFloatLE(value, i * 4));
  const text = JSON.stringify({
    asset: { version: "2.0" }, scene: 0, scenes: [{ nodes: [0] }], nodes: [{ mesh: 0 }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, material: 0 }] }],
    materials: [{ pbrMetallicRoughness: { baseColorFactor: [0.2, 0.6, 0.9, 1] }, doubleSided: true }],
    buffers: [{ byteLength: data.length }], bufferViews: [{ buffer: 0, byteLength: data.length }],
    accessors: [{ bufferView: 0, componentType: 5126, count: vertices.length / 3, type: "VEC3", min: a, max: [0.04, 0.03, 0.02] }],
  });
  const json = Buffer.from(text.padEnd(Math.ceil(Buffer.byteLength(text) / 4) * 4, " "));
  const glb = Buffer.alloc(28 + json.length + data.length);
  glb.writeUInt32LE(0x46546c67, 0); glb.writeUInt32LE(2, 4); glb.writeUInt32LE(glb.length, 8);
  glb.writeUInt32LE(json.length, 12); glb.writeUInt32LE(0x4e4f534a, 16); json.copy(glb, 20);
  glb.writeUInt32LE(data.length, 20 + json.length); glb.writeUInt32LE(0x004e4942, 24 + json.length); data.copy(glb, 28 + json.length);
  return glb;
}

type Mode = "fail" | "success" | "missing" | "empty" | "race-a" | "race-b";
function fixture(name: string) {
  const folder = join(root, name); mkdirSync(folder, { recursive: true });
  const path = join(folder, "part.step"), home = join(folder, "home"), calls = join(folder, "calls");
  mkdirSync(home); writeFileSync(path, "synthetic STEP input\n");
  const cache = join(home, "cache", "backplane-bend", "models"); mkdirSync(cache, { recursive: true });
  return { folder, path, home, calls, cache };
}
type Fixture = ReturnType<typeof fixture>;
function count(f: Fixture) { return existsSync(f.calls) ? readFileSync(f.calls, "utf8").trim().split("\n").length : 0; }
// A 3D-only plot has intentionally empty 2D bounds; its m line carries triangles.
function solid(stdout: string) { return /^m4(?: [0-9]+){7} /m.test(stdout); }
function cached(f: Fixture) { return readdirSync(f.cache).filter((path) => path.endsWith(".glb")); }
function shots(f: Fixture) { return ["iso", "front", "top", "right"].map((view) => join(dirname(f.path), "renders", `${basename(f.path, ".step")}.${view}.png`)); }
function png(path: string) { return existsSync(path) && readFileSync(path).subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])); }
async function run(name: string, f: Fixture, mode: Mode, render = false) {
  const env = {
    ...process.env, HOME: f.home, BACKPLANE_HOME: join(f.home, "backplane"), DISPLAY: "", WAYLAND_DISPLAY: "",
    XDG_CACHE_HOME: join(f.home, "cache"), XDG_CONFIG_HOME: join(f.home, "config"), XDG_DATA_HOME: join(f.home, "data"),
    BACKPLANE_NO_UPDATE: "1", BACKPLANE_GOOGLE_BAKE: "0", MODEL_CACHE_MODE: mode, MODEL_CACHE_CALLS: f.calls,
    MODEL_CACHE_BYTES: join(root, "complete.glb"), MODEL_CACHE_SYNC: f.folder,
  };
  const args = render ? ["--render", f.path, "--size", "64x48"] : ["--plot", "2", f.path];
  const proc = Bun.spawn([join(prefix, name), ...args], { cwd: f.folder, env, stdout: "pipe", stderr: "pipe" });
  children.add(proc);
  const output = Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text()]);
  const code = await within(proc.exited, 20000);
  if (code === undefined) { check(`${name} child stops after timeout`, await stop(proc)); throw new Error("model child timed out"); }
  children.delete(proc);
  const [stdout, stderr] = await within(output, 2000) ?? ["", "output pipes remained open"];
  return { code, stdout, stderr };
}
function oldKey(f: Fixture) {
  const st = statSync(f.path);
  let h = 2166136261;
  for (const c of `${f.path}|${st.size}.${Math.floor(st.mtimeMs / 1000) >>> 0}`) h = Math.imul(h ^ c.codePointAt(0)!, 16777619) >>> 0;
  return join(f.cache, `${h}.glb`);
}
async function checks(name: string) {
  const plot = fixture(`${name}-plot-failure`);
  for (let n = 1; n <= 2; n++) {
    const r = await run(name, plot, "fail");
    check(`${name} failed plot attempt ${n} reports converter failure`, r.code === 3 && r.stderr.includes("synthetic converter failure"), r);
    check(`${name} failed plot attempt ${n} publishes no cache entry`, cached(plot).length === 0, cached(plot));
    check(`${name} failed plot attempt ${n} calls converter again`, count(plot) === n, count(plot));
    check(`${name} failed plot attempt ${n} cleans partial output`, !readdirSync(plot.cache).some((path) => path.endsWith(".part")), readdirSync(plot.cache));
  }
  const recovered = await run(name, plot, "success");
  check(`${name} recovered plot produces a nonempty model`, recovered.code === 0 && solid(recovered.stdout), recovered);
  check(`${name} recovered plot retries converter`, count(plot) === 3, count(plot));
  const cache = cached(plot);
  check(`${name} recovered cache has complete GLB bytes`, cache.length === 1 && readFileSync(join(plot.cache, cache[0])).equals(model()), cache);
  const again = await run(name, plot, "fail");
  check(`${name} complete plot cache avoids converter`, again.code === 0 && again.stdout === recovered.stdout && count(plot) === 3, { again, count: count(plot) });
  // The app entry owns --render. A hub-only entry is exercised through --plot.
  if (name === "backplane") {
    await run(name, plot, "fail", true);
    check(`${name} native renderer shares the completed plot cache`, count(plot) === 3 && shots(plot).every(png), { count: count(plot), shots: shots(plot) });

    const native = fixture(`${name}-native-failure`);
    for (let n = 1; n <= 2; n++) {
      const r = await run(name, native, "fail", true);
      check(`${name} native failure attempt ${n} produces no render`, shots(native).every((p) => !existsSync(p)) && r.stderr.includes("could not mesh"), r);
      check(`${name} native failure attempt ${n} publishes no cache`, cached(native).length === 0 && count(native) === n, { files: cached(native), count: count(native) });
    }
    await run(name, native, "success", true);
    check(`${name} native retry recovers with complete model`, count(native) === 3 && shots(native).every(png), { count: count(native), files: cached(native) });
    const fromNative = await run(name, native, "fail");
    check(`${name} plot shares native renderer cache`, fromNative.code === 0 && count(native) === 3, fromNative);
  }

  const missing = fixture(`${name}-missing-output`);
  const absent = await run(name, missing, "missing");
  check(`${name} zero exit without output reports publication failure`, absent.code === 3 && absent.stderr.includes("Could not cache 3D model"), absent);
  check(`${name} absent converter output publishes no cache`, cached(missing).length === 0, cached(missing));

  const empty = fixture(`${name}-empty-output`);
  const zero = await run(name, empty, "empty");
  check(`${name} empty converter output reports failure`, zero.code === 3 && zero.stderr.includes("Could not cache 3D model"), zero);
  check(`${name} empty output cannot poison the cache`, cached(empty).length === 0, cached(empty));

  const legacy = fixture(`${name}-legacy-cache`);
  const old = oldKey(legacy); writeFileSync(old, "truncated-glb-fixture");
  const renewed = await run(name, legacy, "success");
  check(`${name} legacy failed cache is bypassed`, renewed.code === 0 && count(legacy) === 1 && solid(renewed.stdout), renewed);
  check(`${name} legacy entry does not share the new cache key`, cached(legacy).length === 2 && readFileSync(old, "utf8") === "truncated-glb-fixture", cached(legacy));
}

async function concurrent(name: string, nativeFirst: boolean) {
  const label = nativeFirst ? "native and plot" : "two plot requests";
  const f = fixture(`${name}-concurrent-${nativeFirst ? "native" : "plot"}`);
  const a = run(name, f, "race-a", nativeFirst);
  const b = run(name, f, "race-b");
  try {
    const first = await a;
    check(`${name} ${label} successful attempt reads its own model`, first.code === 0 && (nativeFirst ? shots(f).every(png) : solid(first.stdout)), first);
    const files = cached(f);
    check(`${name} ${label} publishes only complete bytes while second attempt runs`, files.length === 1 && readFileSync(join(f.cache, files[0])).equals(model()), files);
    const staging = readFileSync(f.calls, "utf8").trim().split("\n");
    check(`${name} ${label} uses different staging paths`, staging.length === 2 && staging[0] !== staging[1], staging);
  } finally { writeFileSync(join(f.folder, "release-b"), ""); }
  const second = await b;
  check(`${name} ${label} failed attempt reports failure`, second.code === 3, second);
  const final = cached(f);
  check(`${name} ${label} failed attempt preserves complete cached model`, final.length === 1 && readFileSync(join(f.cache, final[0])).equals(model()), final);
  check(`${name} ${label} removes both staging directories`, !readdirSync(f.cache).some((p) => p.includes(".part.")), readdirSync(f.cache));
  const again = await run(name, f, "fail");
  check(`${name} ${label} cache remains reusable`, again.code === 0 && solid(again.stdout) && count(f) === 2, again);
}

try {
  mkdirSync(prefix); writeFileSync(join(root, "complete.glb"), model());
  for (const name of ["backplane", "backplane-serve"]) { copyFileSync(resolve(binary), join(prefix, name)); chmodSync(join(prefix, name), 0o755); }
  const converter = join(prefix, "backplane-step2glb");
  writeFileSync(converter, `#!/bin/sh
set -eu
printf '%s\\n' "$2" >> "$MODEL_CACHE_CALLS"
case "$MODEL_CACHE_MODE" in
  fail) printf truncated-glb-fixture > "$2"; printf 'synthetic converter failure\n' >&2; exit 23 ;;
  success) cp "$MODEL_CACHE_BYTES" "$2" ;;
  missing) exit 0 ;;
  empty) : > "$2" ;;
  race-a)
    cp "$MODEL_CACHE_BYTES" "$2"
    : > "$MODEL_CACHE_SYNC/a-ready"
    n=0
    while [ ! -f "$MODEL_CACHE_SYNC/b-ready" ]; do n=$((n+1)); [ "$n" -le 400 ] || exit 24; sleep .01; done
    ;;
  race-b)
    n=0
    while [ ! -f "$MODEL_CACHE_SYNC/a-ready" ]; do n=$((n+1)); [ "$n" -le 400 ] || exit 24; sleep .01; done
    printf truncated-by-failed-attempt > "$2"
    : > "$MODEL_CACHE_SYNC/b-ready"
    n=0
    while [ ! -f "$MODEL_CACHE_SYNC/release-b" ]; do n=$((n+1)); [ "$n" -le 400 ] || exit 24; sleep .01; done
    exit 23
    ;;
esac
`); chmodSync(converter, 0o755);
  for (const name of ["backplane", "backplane-serve"]) {
    try { await checks(name); await concurrent(name, false); if (name === "backplane") await concurrent(name, true); } catch (error) { check(`${name} model cache scenarios complete`, false, String(error)); }
  }
} finally {
  for (const proc of children) check("captured model child stops", await stop(proc));
  if (children.size === 0) rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
