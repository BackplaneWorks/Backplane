// Isolated health discovery and broadcasts. Never contacts the live hub.
// bun test/tools/kicad_health_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const [binary = "build/backplane-health", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
let failed = false;
function check(name: string, ok: boolean, got?: unknown) {
  console.log(`${ok ? "ok" : "FAIL"} ${name}${ok ? "" : `: ${JSON.stringify(got)?.slice(0, 500)}`}`);
  if (!ok) failed = true;
}
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
async function until<T>(f: () => T | undefined, ms = 25000): Promise<T> {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = f();
    if (v !== undefined) return v;
    await sleep(50);
  }
  throw new Error("timed out waiting for hub health");
}
function freePort() {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port; s.stop(true);
  if (p === 3787) return freePort();
  return p;
}
const root = mkdtempSync(join(tmpdir(), "bp-health-e2e-"));
// Backplane KiCad 10.99 with API 1 is the least that serves IPC (KH.min_version, KH.min_api)
const legacy = { sourceRepository: "https://github.com/i2cjak/Backplane_KiCad", version: "10.99.0-bp.1", sourceCommit: "abc" };
const v1 = { ...legacy, apiVersion: 1 };
const v2 = { ...v1, manifestVersion: 2, kicadVersion: "10.99.0", upstreamCommit: "up", capabilities: ["api-server", "headless-pcb"] };
function runtime(name: string, manifest?: any, script = "printf '10.0.6\\n'", bundle = false, describe?: any) {
  const dir = join(root, name);
  const cli = join(dir, bundle ? "Contents/MacOS/kicad-cli" : "bin/kicad-cli");
  mkdirSync(join(cli, ".."), { recursive: true });
  const desc = describe ? `if [ "$1 $2" = "api-server --describe" ]; then printf '%s' '${JSON.stringify(describe)}'; exit 0; fi\n` : "";
  writeFileSync(cli, `#!/bin/sh\n${desc}[ "$1" = version ] || exit 2\nif [ -n "$BACKPLANE_KICAD_TEST_COUNT" ]; then printf x >> "$BACKPLANE_KICAD_TEST_COUNT"; fi\n${script}\n`); chmodSync(cli, 0o755);
  const mdir = bundle ? join(dir, "Contents/Resources") : dir;
  mkdirSync(mdir, { recursive: true });
  if (manifest) writeFileSync(join(mdir, "manifest.json"), JSON.stringify(manifest));
  else rmSync(join(mdir, "manifest.json"), { force: true });
  return { dir, cli };
}
// Providers and network helpers are stand-ins; HOME and every cache are
// temporary, so probing cannot use a real account or touch the live home.
const fake = join(root, "fake-bin"); mkdirSync(fake);
for (const name of ["claude", "codex", "grok", "muse", "agy", "curl"]) {
  const p = join(fake, name); writeFileSync(p, "#!/bin/sh\nexit 1\n"); chmodSync(p, 0o755);
}
const processes: ReturnType<typeof Bun.spawn>[] = [];
const sockets: WebSocket[] = [];
async function start(name: string, path: string, homeOverride?: string) {
  const home = homeOverride ?? join(root, `home-${name}`);
  mkdirSync(home, { recursive: true });
  const skills = join(home, ".cache/backplane/kistack/ecf16c119c7ed610f5f251c479b03884b8d98b09/skills/pcb");
  mkdirSync(skills, { recursive: true }); writeFileSync(join(skills, "SKILL.md"), "---\nname: fixture\ndescription: test fixture\n---\n");
  const port = freePort();
  const proc = Bun.spawn([resolve(binary), "--foreground", "--home", home, "--port", String(port), "--no-tailscale"], {
    env: { ...process.env, HOME: home, XDG_CACHE_HOME: join(home, ".cache"), PATH: `${fake}:/usr/bin:/bin`, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_HOME: home, BACKPLANE_KICAD_TEST_COUNT: join(home, "version-calls"), BACKPLANE_KICAD: path, BACKPLANE_KICAD_CLI: "", BACKPLANE_PEERS: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_WAKE_TICK_MS: "1000", BACKPLANE_LOCK_DIR: join(home, "locks") },
    stdout: Bun.file(join(home, "stdout.log")), stderr: Bun.file(join(home, "stderr.log"))
  });
  processes.push(proc);
  await until(() => existsSync(join(home, "token")) ? true : undefined);
  // HTTP is checked asynchronously; no provider or real project is used.
  for (let i = 0; ; i++) {
    try { const r = await fetch(`http://127.0.0.1:${port}/hello`); if (r.ok) break; } catch {}
    if (i > 200) throw new Error(`hub ${name} did not listen: ${readFileSync(join(home, "stderr.log"), "utf8")}`);
    await sleep(50);
  }
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); sockets.push(ws); ws.binaryType = "arraybuffer";
  ws.onmessage = e => seen.push(JSON.parse(W.decode(new Uint8Array(e.data))));
  await new Promise<void>((r, reject) => { ws.onopen = () => r(); ws.onerror = reject; });
  let id = 0;
  const send = (m: string, p = {}) => { const n = ++id; ws.send(W.encode(JSON.stringify({ id: n, m, p }))); return n; };
  const health = (status: string, after = 0) => until(() => seen.slice(after).find(o => (o.t === "info" || o.t === "log") && o.info?.["kicad.status"] === status)?.info);
  return { proc, ws, seen, send, health, home, port };
}
async function stop(p: ReturnType<typeof Bun.spawn>) { if (p.exitCode === null) p.kill("SIGTERM"); await p.exited; }
try {
  for (const [name, manifest, script, expected] of [
    ["legacy", legacy, "printf '10.99.0-bp.1\\n'", "old"],
    ["v1", v1, "printf '10.99.0-bp.1\\n'", "ok"],
    ["v2", v2, "printf '10.99.0-bp.1\\n'", "ok"],
    ["stock", undefined, "printf '10.0.6\\n'", "stock"],
    ["broken", undefined, "printf 'cannot execute\\nmore output\\n' >&2; exit 1", "broken"],
    ["old", { ...v1, version: "10.0.5" }, "printf '10.0.5\\n'", "old"],
  ] as const) {
    const rt = runtime(name, manifest, script);
    const h = await start(name, rt.dir);
    const info = await h.health(expected);
    check(`${name} health ${expected}`, info["kicad.status"] === expected, info);
    check(`${name} environment root source`, info["kicad.source"] === "env" && info["kicad.cli"] === rt.cli, info);
    if (name === "legacy") check("a manifest without apiVersion is API 0, which is too old", info["kicad.api"] === "0", info);
    if (name === "v1") {
      await sleep(2000);
      const calls = readFileSync(join(h.home, "version-calls"), "utf8").length;
      await sleep(2200);
      check("unchanged periodic stat does not rerun CLI", readFileSync(join(h.home, "version-calls"), "utf8").length === calls);
    }
    if (name === "v2") check("v2 API and caps", info["kicad.api"] === "1" && info["kicad.caps"] === "api-server,headless-pcb", info);
    if (name === "broken") {
      check("failure reason first line", info["kicad.why"] === "cannot execute", info);
      check("failure output is not reported as version", info["kicad.version"] === "", info);
    }
    if (name === "stock") {
      check("stock uses version output", info["kicad.version"] === "10.0.6", info);
      const at = h.seen.length; runtime(name, v1); h.send("kicad.retry");
      const updated = await h.health("ok", at);
      check("in-place stock to fork retry broadcasts", updated["kicad.fork"] === "10.99.0-bp.1", updated);
      const at2 = h.seen.length;
      h.send("setting.set", { key: "kicad.path", value: join(root, "not-there", "kicad-cli") });
      const missing = await h.health("missing", at2);
      check("setting overrides working env without fallback", missing["kicad.source"] === "setting", missing);
      const at3 = h.seen.length; h.send("setting.set", { key: "kicad.path", value: rt.dir });
      const restored = await h.health("ok", at3);
      check("setting runtime root accepted", restored["kicad.cli"] === rt.cli, restored);
    }
    await until(() => existsSync(join(h.home, "kicad-health.json")) ? true : undefined);
    const cache = JSON.parse(readFileSync(join(h.home, "kicad-health.json"), "utf8"));
    check(`${name} full record cached`, !!cache.stamp && typeof cache.runs === "boolean" && cache.status !== "checking", cache);
    h.ws.close(); await stop(h.proc);
    if (name === "v1") {
      const restarted = await start("cached-v1", rt.dir, h.home);
      await restarted.health("ok");
      // the connect message (t: "log") carries the info the hub booted with
      const first = restarted.seen.find(o => o.t === "log" || o.t === "info")?.info;
      check("unchanged cache is served at boot", first?.["kicad.status"] === "ok", first);
      restarted.ws.close(); await stop(restarted.proc);
    }
  }
  const h = await start("missing", "/nonexistent");
  const info = await h.health("missing");
  check("explicit nonexistent does not fall through", info["kicad.source"] === "env" && info["kicad.why"].startsWith("no kicad-cli at /nonexistent"), info);
  const proj = h.send("project.add", { path: root });
  const created = await until(() => h.seen.flatMap(o => o.items ?? []).find(o => o.$ === "ThreadCreated"));
  const at = h.seen.length; const rid = h.send("kicad.open", { thread: created.id });
  const reply = await until(() => h.seen.slice(at).find(o => String(o.id) === String(rid) && o.result?.kicad));
  check("open carries structured status and thread", reply.result.kicad === "missing" && reply.result.thread === created.id, reply);
  h.ws.close(); await stop(h.proc);
  const later = await start("later", join(root, "later-runtime"));
  await later.health("missing");
  const after = later.seen.length;
  runtime("later-runtime", v1);
  check("periodic stat notices runtime becoming present", (await later.health("ok", after))["kicad.fork"] === "10.99.0-bp.1");
  later.ws.close(); await stop(later.proc);
  const bundle = runtime("KiCad.app", v2, "printf '10.99.0-bp.1\\n'", true);
  const app = await start("bundle", bundle.dir);
  check("macOS bundle Resources manifest", (await app.health("ok"))["kicad.api"] === "1");
  app.ws.close(); await stop(app.proc);
  const desc = runtime("described", undefined, "printf '10.99.0-bp.1\\n'", false, v2);
  const dh = await start("described", desc.dir);
  const dinfo = await dh.health("ok");
  check("no manifest: api-server --describe identifies the fork", dinfo["kicad.api"] === "1" && dinfo["kicad.fork"] === "10.99.0-bp.1", dinfo);
  dh.ws.close(); await stop(dh.proc);
} catch (e) { check("e2e completed", false, String(e)); }
finally {
  for (const ws of sockets) ws.close();
  for (const proc of processes) await stop(proc);
  if (failed) console.log(`fixtures and hub logs retained at ${root}`);
  else rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
