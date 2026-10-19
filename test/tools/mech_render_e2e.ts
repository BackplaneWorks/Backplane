// End to end: native render failures reach the CLI and Mechanical page.
// Original, generated GLBs; temporary projects/homes; no CAD installation.
//
//   bun test/tools/mech_render_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane and is copied under both native entry
// names. WIREDIR defaults to build/wire (bend test/wire/index.html -o build/wire).
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";

const [binary = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const file of readdirSync(wire).filter((file) => file.endsWith(".js"))) (0, eval)(readFileSync(join(wire, file), "utf8"));
const W = (globalThis as any).Wire;
const root = realpathSync(mkdtempSync(join(tmpdir(), "bp-render-e2e-")));
const prefix = join(root, "prefix"), fake = join(root, "bin");
const children = new Set<ReturnType<typeof Bun.spawn>>();
let failed = false, serial = 0;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 500)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
async function within<T>(promise: Promise<T>, ms: number): Promise<T | undefined> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([promise, new Promise<undefined>((resolve) => { timer = setTimeout(() => resolve(undefined), ms); })]);
  } finally { if (timer !== undefined) clearTimeout(timer); }
}
async function stop(proc: ReturnType<typeof Bun.spawn>): Promise<boolean> {
  if (proc.exitCode === null) proc.kill("SIGTERM");
  let exited = await within(proc.exited, 3000);
  if (exited === undefined) { proc.kill("SIGKILL"); exited = await within(proc.exited, 2000); }
  if (exited !== undefined) children.delete(proc);
  return exited !== undefined;
}
function freePort(): number {
  for (let i = 0; i < 16; i++) {
    const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
    const port = listener.port; listener.stop(true);
    if (port >= 3799) return port;
  }
  throw new Error("no spare test port above 3798");
}
function environment(home: string) {
  const user = join(home, "user"); mkdirSync(user, { recursive: true });
  return {
    ...process.env, HOME: user, BACKPLANE_HOME: home, DISPLAY: "", WAYLAND_DISPLAY: "",
    XDG_CONFIG_HOME: join(home, "config"), XDG_DATA_HOME: join(home, "data"), XDG_CACHE_HOME: join(home, "cache"), XDG_STATE_HOME: join(home, "state"),
    BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    BACKPLANE_AGENTS: fake, BACKPLANE_LOCK_DIR: join(home, "locks"), PATH: `${fake}:${process.env.PATH}`,
    HTTP_PROXY: "http://127.0.0.1:1", HTTPS_PROXY: "http://127.0.0.1:1", ALL_PROXY: "http://127.0.0.1:1", NO_PROXY: "127.0.0.1,localhost",
  };
}

// A tetrahedron in metres: twelve vertices, no external assets or converter.
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
const views = ["iso", "front", "top", "right"];
const shots = (path: string) => views.map((view) => join(dirname(path), "renders", `${basename(path, ".glb")}.${view}.png`));
function png(path: string, width: number, height: number): boolean {
  if (!existsSync(path) || !statSync(path).isFile()) return false;
  const bytes = readFileSync(path);
  return bytes.length >= 24 && bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])) && bytes.readUInt32BE(16) === width && bytes.readUInt32BE(20) === height;
}
type Kind = "valid" | "missing" | "corrupt" | "empty" | "blocked" | "rename" | "part";
const previous = Buffer.from("previous published PNG bytes\n");
function fixture(folder: string, kind: Kind): string {
  mkdirSync(folder, { recursive: true });
  const path = join(folder, "part.glb");
  if (kind !== "missing") writeFileSync(path, kind === "corrupt" ? Buffer.from("not a GLB") : kind === "empty" ? Buffer.alloc(0) : model());
  if (kind === "blocked") writeFileSync(join(folder, "renders"), "keep this file\n");
  if (kind === "rename") {
    mkdirSync(shots(path)[0], { recursive: true });
    writeFileSync(join(shots(path)[0], "keep.txt"), "keep this directory\n");
  }
  if (kind === "part") {
    mkdirSync(`${shots(path)[0]}.part`, { recursive: true });
    writeFileSync(shots(path)[0], previous);
  }
  return path;
}
async function cli(name: string, cwd: string, args: string[]) {
  const home = join(root, "cli-home", String(++serial));
  const proc = Bun.spawn([join(prefix, name), "--render", ...args], { cwd, env: environment(home), stdout: "pipe", stderr: "pipe" });
  children.add(proc);
  const output = Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text()]);
  const exited = await within(proc.exited, 20000);
  const timedOut = exited === undefined;
  if (timedOut) check(`${name} render process stops after timeout`, await stop(proc));
  else children.delete(proc);
  const [stdout, stderr] = await within(output, 2000) ?? ["", "output pipe did not close"];
  return { code: proc.exitCode, timedOut, stdout, stderr };
}
async function renderDispatch(name: string): Promise<boolean> {
  const folder = join(root, "cli", name);
  const path = fixture(join(folder, "dispatch"), "valid");
  // Old server entries accept --render but start a hub. These extra flags
  // keep that baseline hub on a private home/spare port with no tailnet.
  // A fixed renderer treats the extra flags as additional failing inputs.
  // Four PNGs prove dispatch before any clean command is tried, including
  // a server-only build copied under the ordinary backplane name.
  const safeHome = join(root, "dispatch-home", name);
  const probe = await cli(name, folder, [path, "--size", "64x48", "--home", safeHome, "--port", String(freePort()), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"]);
  const dispatched = !probe.timedOut && shots(path).every((shot) => png(shot, 64, 48));
  check(`${name} dispatches render before starting a hub`, dispatched, probe);
  return dispatched;
}
async function cliChecks(name: string) {
  const folder = join(root, "cli", name);
  const valid = fixture(join(folder, "valid"), "valid");
  const success = await cli(name, folder, [valid, "--size", "64x48"]);
  check(`${name} valid CLI exits zero`, !success.timedOut && success.code === 0, success);
  check(`${name} valid CLI writes four sized PNGs`, shots(valid).every((shot) => png(shot, 64, 48)));
  check(`${name} valid CLI prints the four output paths`, shots(valid).every((shot) => success.stdout.trim().split("\n").includes(shot)), success.stdout);
  for (const kind of ["missing", "corrupt", "empty", "blocked", "rename", "part"] as const) {
    const path = fixture(join(folder, kind), kind);
    const result = await cli(name, folder, [path, "--size", "64x48"]);
    check(`${name} ${kind} CLI fails`, !result.timedOut && result.code !== null && result.code !== 0, result);
    const context = kind === "rename" || kind === "part" ? "part.iso.png" : "part.glb";
    check(`${name} ${kind} CLI explains the failed output or model`, result.stderr.includes("backplane --render:") && result.stderr.includes(context), result.stderr);
    if (kind === "rename") {
      check(`${name} rename failure preserves the destination directory`, readFileSync(join(shots(path)[0], "keep.txt"), "utf8") === "keep this directory\n");
      check(`${name} rename failure does not claim the blocked output`, !result.stdout.trim().split("\n").includes(shots(path)[0]), result.stdout);
    } else if (kind === "part") {
      check(`${name} failed open preserves the existing PNG`, readFileSync(shots(path)[0]).equals(previous));
      check(`${name} failed open preserves the pre-existing part directory`, statSync(`${shots(path)[0]}.part`).isDirectory() && readdirSync(`${shots(path)[0]}.part`).length === 0);
      check(`${name} failed open does not claim the previous PNG`, !result.stdout.trim().split("\n").includes(shots(path)[0]), result.stdout);
    } else if (kind === "blocked") {
      check(`${name} blocked output preserves the existing file`, readFileSync(join(dirname(path), "renders"), "utf8") === "keep this file\n");
      check(`${name} blocked output prints no successful paths`, !shots(path).some((shot) => result.stdout.trim().split("\n").includes(shot)), result.stdout);
    } else check(`${name} ${kind} model produces no PNGs`, shots(path).every((shot) => !existsSync(shot)));
  }
  const batch = join(folder, "batch"); mkdirSync(batch, { recursive: true });
  const first = join(batch, "first.glb"), bad = join(batch, "bad.glb"), last = join(batch, "last.glb");
  writeFileSync(first, model()); writeFileSync(bad, "not a GLB"); writeFileSync(last, model());
  const mixed = await cli(name, folder, [first, bad, last, "--size", "64x48"]);
  check(`${name} mixed CLI batch exits nonzero`, !mixed.timedOut && mixed.code !== null && mixed.code !== 0, mixed);
  check(`${name} mixed CLI batch continues after failure`, [...shots(first), ...shots(last)].every((shot) => png(shot, 64, 48)));
  check(`${name} mixed CLI batch names the failed input`, mixed.stderr.includes("bad.glb"), mixed.stderr);
}

async function rpcChecks(name: string) {
  const home = join(root, "hub", name), project = join(root, "rpc", name), emptyProject = join(root, "rpc-empty", name);
  mkdirSync(emptyProject, { recursive: true });
  const paths = new Map<Kind, string>();
  for (const kind of ["valid", "missing", "corrupt", "empty", "blocked", "rename", "part"] as const) paths.set(kind, fixture(join(project, "mech", kind), kind));
  const port = freePort();
  const proc = Bun.spawn([join(prefix, name), "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
    env: environment(home), stdout: "ignore", stderr: process.env.E2E_LOG ? "inherit" : "ignore",
  });
  children.add(proc);
  let ws: WebSocket | undefined;
  try {
    const deadline = Date.now() + 15000;
    let ready = false;
    while (Date.now() < deadline && proc.exitCode === null) {
      try { ready = (await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(1000) })).ok; } catch {}
      if (ready) break;
      await sleep(50);
    }
    if (!ready) throw new Error(`${name} isolated hub did not start`);
    const replies = new Map<number, any>();
    let decodeError: unknown;
    ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
    ws.onmessage = (event) => {
      try {
        const reply = JSON.parse(W.decode(new Uint8Array(event.data as ArrayBuffer)));
        if (reply.t === "reply") replies.set(Number(reply.id), reply);
      } catch (error) { decodeError = error; }
    };
    await within(new Promise<void>((resolve, reject) => { ws!.onopen = () => resolve(); ws!.onerror = reject; }), 10000);
    if (ws.readyState !== WebSocket.OPEN) throw new Error(`${name} isolated socket did not open`);
    let id = 0;
    const rpc = async (method: string, params: object, timeout = 10000) => {
      const key = ++id;
      ws!.send(W.encode(JSON.stringify({ id: key, m: method, p: params })));
      const deadline = Date.now() + timeout;
      while (Date.now() < deadline && proc.exitCode === null && !replies.has(key)) await sleep(50);
      const reply = replies.get(key);
      if (!reply) throw new Error(`${method} did not answer${decodeError ? `: ${decodeError}` : ""}`);
      return reply;
    };
    const added = await rpc("project.add", { path: project });
    const empty = await rpc("project.add", { path: emptyProject });
    if (!added.ok || !added.thread || !empty.ok || !empty.thread) throw new Error("isolated test projects were not added");
    const metadata = (reply: any, thread: string, folder: string, path: string) => reply.ok === true && reply.result?.thread === thread && reply.result?.root === folder && reply.result?.rendered === path && typeof reply.result?.mech === "string";
    for (const kind of ["missing", "corrupt", "empty", "blocked", "rename", "part"] as const) {
      const path = `mech/${kind}/part.glb`;
      const reply = await rpc("mech.render", { thread: added.thread, path }, 120000);
      check(`${name} ${kind} RPC reports a render error`, typeof reply.result?.error === "string" && reply.result.error.length > 0, reply);
      check(`${name} ${kind} RPC keeps listing and metadata`, metadata(reply, added.thread, project, path) && reply.result.mech.includes("mech/valid/part.glb"), reply);
    }
    const missing = "mech/missing.glb";
    const emptyReply = await rpc("mech.render", { thread: empty.thread, path: missing }, 120000);
    check(`${name} missing-only RPC preserves an empty listing`, metadata(emptyReply, empty.thread, emptyProject, missing) && emptyReply.result.mech === "", emptyReply);
    check(`${name} missing-only RPC still reports the error`, typeof emptyReply.result?.error === "string" && emptyReply.result.error.length > 0, emptyReply);
    const recovered = await rpc("mech.render", { thread: added.thread, path: "mech/valid/part.glb" }, 120000);
    check(`${name} valid RPC after failure clears the error`, metadata(recovered, added.thread, project, "mech/valid/part.glb") && recovered.result.error === "", recovered);
    check(`${name} valid RPC writes and lists four PNGs`, shots(paths.get("valid")!).every((shot) => png(shot, 800, 600) && recovered.result?.mech.includes(shot.slice(project.length + 1))), recovered);
    check(`${name} socket replies decode`, decodeError === undefined, String(decodeError));
    check(`${name} hub stays alive after render errors`, proc.exitCode === null, proc.exitCode);
  } finally {
    ws?.close();
    check(`${name} captured hub stops`, await stop(proc));
  }
}

try {
  mkdirSync(prefix); mkdirSync(fake);
  for (const name of ["backplane", "backplane-serve"]) { copyFileSync(resolve(binary), join(prefix, name)); chmodSync(join(prefix, name), 0o755); }
  for (const name of ["claude", "codex", "grok", "muse", "agy", "tailscale", "curl", "wget", "ssh"]) { writeFileSync(join(fake, name), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, name), 0o755); }
  writeFileSync(join(prefix, "backplane-step2glb"), "#!/bin/sh\nexit 1\n"); chmodSync(join(prefix, "backplane-step2glb"), 0o755);
  const dispatch = new Map<string, boolean>();
  // Preflight both entries before any RPC: every hub's render job invokes
  // prefix/backplane, even when the hub itself is backplane-serve.
  for (const name of ["backplane", "backplane-serve"]) {
    try { dispatch.set(name, await renderDispatch(name)); }
    catch (error) { check(`${name} dispatch probe`, false, String(error)); dispatch.set(name, false); }
  }
  for (const name of ["backplane", "backplane-serve"]) {
    if (!dispatch.get(name)) continue;
    try {
      await cliChecks(name);
      if (dispatch.get("backplane")) await rpcChecks(name);
    }
    catch (error) { check(`${name} run`, false, String(error)); }
  }
} finally {
  for (const proc of children) check("captured child stops during final cleanup", await stop(proc));
  if (children.size === 0) rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
