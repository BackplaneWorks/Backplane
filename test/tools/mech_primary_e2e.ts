// End to end: .backplane.json's mech setting follows each thread's folder,
// is read afresh by mech.list, and survives mech.render's refreshed listing.
// Uses original GLB fixtures, temporary homes and stand-in agents; no CAD
// installation, existing project or running hub is needed.
//
//   bun test/tools/mech_primary_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane; WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire). The binary must support --render.
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 400)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until<T>(ms: number, f: () => T | undefined): Promise<T | undefined> {
  const end = Date.now() + ms;
  do {
    const value = f();
    if (value !== undefined) return value;
    await sleep(50);
  } while (Date.now() < end);
  return undefined;
}
function freePort(): number {
  for (let i = 0; i < 16; i++) {
    const server = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
    const port = server.port;
    server.stop(true);
    if (port >= 3799) return port;
  }
  throw new Error("could not find a test port above 3798");
}

// A tetrahedron in metres, unindexed triangles, in a standard GLB 2.0
// container. Both fixtures come from these bytes, with no external assets.
function tetrahedron(): Buffer {
  const a = [0, 0, 0], b = [0.04, 0, 0], c = [0, 0.03, 0], d = [0, 0, 0.02];
  const vertices = [a, c, b, a, b, d, a, d, c, b, c, d].flat();
  const data = Buffer.alloc(vertices.length * 4);
  vertices.forEach((value, i) => data.writeFloatLE(value, i * 4));
  const text = JSON.stringify({
    asset: { version: "2.0" }, scene: 0, scenes: [{ nodes: [0] }],
    nodes: [{ mesh: 0 }], meshes: [{ primitives: [{ attributes: { POSITION: 0 }, material: 0 }] }],
    materials: [{ pbrMetallicRoughness: { baseColorFactor: [0.2, 0.6, 0.9, 1] }, doubleSided: true }],
    buffers: [{ byteLength: data.length }], bufferViews: [{ buffer: 0, byteLength: data.length }],
    accessors: [{ bufferView: 0, componentType: 5126, count: vertices.length / 3, type: "VEC3", min: a, max: [0.04, 0.03, 0.02] }],
  });
  const json = Buffer.from(text.padEnd(Math.ceil(Buffer.byteLength(text) / 4) * 4, " "));
  const glb = Buffer.alloc(12 + 8 + json.length + 8 + data.length);
  glb.writeUInt32LE(0x46546c67, 0); glb.writeUInt32LE(2, 4); glb.writeUInt32LE(glb.length, 8);
  glb.writeUInt32LE(json.length, 12); glb.writeUInt32LE(0x4e4f534a, 16); json.copy(glb, 20);
  const offset = 20 + json.length;
  glb.writeUInt32LE(data.length, offset); glb.writeUInt32LE(0x004e4942, offset + 4); data.copy(glb, offset + 8);
  return glb;
}

const root = mkdtempSync(join(tmpdir(), "bp-mech-primary-e2e-"));
const home = join(root, "home"), user = join(root, "user"), prefix = join(root, "prefix"), fake = join(root, "agents");
const project = join(user, "fixture"), other = join(user, "other-fixture"), checkout = join(user, "checkout");
for (const dir of [home, prefix, fake, join(project, "mech"), join(other, "mech"), join(checkout, "mech")]) mkdirSync(dir, { recursive: true });
copyFileSync(resolve(bin), join(prefix, "backplane"));
chmodSync(join(prefix, "backplane"), 0o755);
for (const name of ["claude", "codex", "grok", "muse", "agy"]) {
  writeFileSync(join(fake, name), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, name), 0o755);
}
for (const dir of [project, other, checkout]) {
  for (const name of ["bracket.glb", "frame.glb"]) writeFileSync(join(dir, "mech", name), tetrahedron());
}
const config = join(project, ".backplane.json");
writeFileSync(config, JSON.stringify({ mech: "mech/frame.glb" }));
writeFileSync(join(other, ".backplane.json"), JSON.stringify({ mech: "mech/bracket.glb" }));
writeFileSync(join(checkout, ".backplane.json"), JSON.stringify({ mech: "mech/frame.glb" }));
// A recovered idle thread with a distinct worktree. Its setting must come
// from that folder, even though its parent project has a different one.
writeFileSync(join(home, "events.jsonl"), [
  { $: "ProjectCreated", id: "checkout-project", title: "Checkout fixture", root: other, at: 1 },
  { $: "ThreadCreated", id: "checkout-thread", project: "checkout-project", title: "Checkout", env: "worktree", branch: "fixture", worktree: checkout, provider: "claude", model: "sonnet", at: 1 },
  { $: "SettingSet", key: "restart.continue", value: "off", at: 1 },
].map((event) => JSON.stringify(event)).join("\n") + "\n");
const port = freePort();
const proc = Bun.spawn([join(prefix, "backplane"), "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
  env: {
    ...process.env, HOME: user, BACKPLANE_HOME: home, DISPLAY: "", WAYLAND_DISPLAY: "",
    XDG_CONFIG_HOME: join(root, "config"), XDG_DATA_HOME: join(root, "data"), XDG_CACHE_HOME: join(root, "cache"),
    BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    BACKPLANE_AGENTS: fake, BACKPLANE_LOCK_DIR: join(root, "locks"), PATH: `${fake}:${process.env.PATH}`,
  },
  stdout: "ignore", stderr: process.env.E2E_LOG ? "inherit" : "ignore",
});
async function exitsWithin(ms: number): Promise<boolean> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      proc.exited.then(() => true),
      new Promise<boolean>((resolve) => { timer = setTimeout(() => resolve(false), ms); }),
    ]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}
let ws: WebSocket | undefined;
try {
  let ready = false;
  const startupDeadline = Date.now() + 15000;
  while (Date.now() < startupDeadline) {
    try { ready = (await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(1000) })).ok; } catch {}
    if (ready) break;
    if (proc.exitCode !== null) throw new Error(`test hub exited with ${proc.exitCode}`);
    await sleep(50);
  }
  if (!ready) throw new Error("test hub did not start");
  const replies = new Map<number, any>();
  let decodeError: unknown;
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    try {
      const message = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
      if (message.t === "reply") replies.set(Number(message.id), message);
    } catch (e) { decodeError = e; }
  };
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("test socket did not connect")), 10000);
    ws!.onopen = () => { clearTimeout(timer); resolve(); };
    ws!.onerror = (e) => { clearTimeout(timer); reject(e); };
  });
  let nextId = 0;
  const rpc = async (method: string, params: object, timeout = 10000) => {
    const id = ++nextId;
    ws!.send(W.encode(JSON.stringify({ id, m: method, p: params })));
    const reply = await until(timeout, () => replies.get(id));
    if (!reply) throw new Error(`${method} timed out${decodeError ? `: ${decodeError}` : ""}`);
    return reply;
  };
  const made = await rpc("project.add", { path: project });
  // The secondary project was recovered above; add a local thread to it
  // rather than attempting to add the same folder as a second project.
  const added = await rpc("thread.create", { project: "checkout-project", title: "Project folder", env: "local" });
  if (!made.ok || !made.thread || !added.ok || !added.thread) throw new Error(`test threads were not created: ${JSON.stringify([made, added])}`);
  const list = async (label: string, primary: string) => {
    const reply = await rpc("mech.list", { thread: made.thread });
    check(`${label}: listing carries the current primary`, reply.ok === true && reply.result?.primary === primary, reply);
    check(`${label}: listing belongs to the requested folder`, reply.result?.root === project && reply.result?.thread === made.thread, reply);
    return reply.result;
  };
  const initial = await list("configured", "mech/frame.glb");
  const paths = (initial?.mech ?? "").trim().split("\n").map((line: string) => line.split(" ").slice(2).join(" "));
  check("configured: both original models are listed", ["mech/bracket.glb", "mech/frame.glb"].every((path) => paths.includes(path)), paths);
  const separate = await rpc("mech.list", { thread: added.thread });
  check("another thread uses its own project's setting", separate.result?.root === other && separate.result?.primary === "mech/bracket.glb", separate);
  const worktree = await rpc("mech.list", { thread: "checkout-thread" });
  check("a worktree thread reads its checkout's setting", worktree.result?.root === checkout && worktree.result?.primary === "mech/frame.glb", worktree);

  writeFileSync(config, JSON.stringify({ mech: "mech/bracket.glb" }));
  await list("changed without restarting", "mech/bracket.glb");
  rmSync(config);
  await list("removed", "");
  writeFileSync(config, '{"mech":');
  await list("malformed JSON", "");
  writeFileSync(config, JSON.stringify({ mech: ["mech/frame.glb"] }));
  await list("non-string setting", "");
  writeFileSync(config, JSON.stringify({ pcb: "board.kicad_pcb" }));
  await list("missing setting", "");

  // Render the other part: refreshing the files must retain the configured
  // primary rather than turning the rendered path into the default.
  writeFileSync(config, JSON.stringify({ mech: "mech/frame.glb" }));
  const rendered = await rpc("mech.render", { thread: made.thread, path: "mech/bracket.glb" }, 120000);
  check("render refresh retains the configured primary", rendered.ok === true && rendered.result?.primary === "mech/frame.glb", rendered);
  check("render refresh names its folder, thread and rendered part", rendered.result?.root === project && rendered.result?.thread === made.thread && rendered.result?.rendered === "mech/bracket.glb", rendered);
  check("the original GLB renders successfully", rendered.result?.error === "", rendered);
  const shots = ["iso", "front", "top", "right"].map((view) => `mech/renders/bracket.${view}.png`);
  const fresh = rendered.result?.mech ?? "";
  check("render refresh lists all four new views", shots.every((shot) => fresh.includes(` ${shot}\n`)), fresh);
  check("all four views are PNG files", shots.every((shot) => {
    const path = join(project, shot);
    return existsSync(path) && readFileSync(path).subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
  }));
  check("the socket decodes every reply", decodeError === undefined, String(decodeError));
} catch (e) {
  check("run", false, String(e));
} finally {
  check("the hub stays alive through the checks", proc.exitCode === null, proc.exitCode);
  ws?.close();
  if (proc.exitCode === null) proc.kill("SIGTERM");
  let stopped = await exitsWithin(5000);
  if (!stopped) {
    proc.kill("SIGKILL");
    stopped = await exitsWithin(2000);
  }
  check("the captured test hub exits during cleanup", stopped);
  if (stopped) rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
