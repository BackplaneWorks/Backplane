// A phone's first launch on a long log, as the phones' engines run it: no
// JIT and a small stack. Starts a headless hub on a made-up log (many
// entries, one ProviderSet late in it), then a child bun with the phone's
// limits joins it through bridge.js from nothing, building a screen as the
// apps do (quiet while frames wait). Checks no call throws (Switch.count
// walked the whole log outside a tail call and overflowed: the frame was
// lost), every project arrives, the cost per frame stays flat (Bend 2.0.36
// marshalled the whole client state at every call: a cold join quadratic,
// minutes of an empty list on a phone), the screen has every field the
// iOS decoder needs with the type it needs, and a kept state, in this
// build's form and in the BigInt form older builds wrote, loads again.
//
//   bun test/tools/phone_cold_e2e.ts [BINARY] [BRIDGE]
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", bridge = "mobile/build/assets/bridge.js", mode, port, token] = process.argv.slice(2);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const PROJECTS = 8;
const THREADS = 40;
const PER = 2000; // messages per thread: 80000 entries

// how many of the made-up projects a screen lists (the hub adds Scratch)
const mine = (s: any) => (s?.projects ?? []).filter((p: any) => /^Project \d+$/.test(p.title)).length;

// The child: a phone's cold join (run with the phone's limits)
if (mode === "child") {
  (0, eval)(readFileSync(bridge, "utf8"));
  const B = (globalThis as any).Backplane;
  const key = `127.0.0.1:${port}`;
  let ws: WebSocket;
  let throws = 0;
  let screen: any = null;
  const handle = (t: string) => {
    const o = JSON.parse(t);
    if (o.screen) screen = o.screen;
    for (const c of o.cmds ?? []) if (c.type === "send") ws?.send(Buffer.from(c.data, "base64"));
    return o;
  };
  const call = (f: () => string) => {
    try {
      return handle(f());
    } catch (e) {
      throws++;
      console.error("throw:", String(e).slice(0, 120));
      return {};
    }
  };
  call(() => B.start("cold", "{}"));
  call(() => B.hubs([key]));
  const r = JSON.parse(B.resume(key));
  const q: string[] = [];
  ws = new WebSocket(`ws://${key}/ws?token=${token}&since=${r.since}&origin=${encodeURIComponent(r.origin)}&enc=cbor`);
  ws.binaryType = "arraybuffer";
  ws.onopen = () => call(() => B.online(key, true));
  ws.onmessage = (m) => q.push(Buffer.from(new Uint8Array(m.data as ArrayBuffer)).toString("base64"));
  const head = Number(process.env.HEAD);
  const costs: number[] = [];
  const t0 = Date.now();
  while (Number(JSON.parse(B.resume(key)).since) < head && Date.now() - t0 < 600000) {
    if (!q.length) {
      await sleep(10);
      continue;
    }
    const f = q.shift()!;
    const t = performance.now();
    call(() => B.recv(key, f, q.length > 0, false));
    costs.push(performance.now() - t);
  }
  call(() => B.screen());
  const since = Number(JSON.parse(B.resume(key)).since);
  // kept state: this build's, and the same with its naturals as BigInts
  // (["<digits>"]), as builds before Bend 2.0.36 wrote them
  const kept = B.save();
  const d = JSON.parse(kept);
  // (a node's index is [number]; a natural written as a BigInt, ["digits"])
  for (const n of d.nodes) if (!Array.isArray(n)) for (const k in n) if (Number.isInteger(n[k]) && n[k] >= 0) n[k] = [String(n[k])];
  const older = JSON.stringify(d);
  const reload = (text: string) => {
    call(() => B.start("cold", "{}"));
    const o = call(() => B.load(text));
    return mine(o.screen);
  };
  const keptProjects = reload(kept);
  const olderProjects = reload(older);
  // the older form still steps: a frame of its hub folds into it
  const after = call(() => B.online(key, true));
  ws.close();
  console.log(JSON.stringify({ throws, since, costs, screen, keptProjects, olderProjects, olderSteps: !!after.cmds }));
  process.exit(0);
}

// The parent: the hub and the checks
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const root = mkdtempSync(join(tmpdir(), "bp-phonecold-"));
const home = join(root, "home");
mkdirSync(home);
const lines: string[] = [];
// recent times: a project idle past Quiet.days (3 days) folds under "Older"
// and leaves the screen's `projects`, so a fixed date made every project
// check fail three days after it was written
const at = Math.floor(Date.now() / 1000) - 3600 - PER;
for (let p = 0; p < PROJECTS; p++) lines.push(JSON.stringify({ $: "ProjectCreated", id: `p${p}`, title: `Project ${p}`, root: join(root, `p${p}`), at }));
for (let t = 0; t < THREADS; t++)
  lines.push(JSON.stringify({ $: "ThreadCreated", id: `t${t}`, project: `p${t % PROJECTS}`, title: `Thread ${t}`, env: "local", branch: "", worktree: "", provider: "claude", model: "", at: at + 1 }));
for (let i = 0; i < PER; i++)
  for (let t = 0; t < THREADS; t++)
    lines.push(JSON.stringify({ $: "MessagePosted", thread: `t${t}`, msg: `m${t}-${i}`, role: i % 2 ? "assistant" : "user", text: `message ${i} in thread ${t}`, at: at + 2 + i }));
// the switch late in the log: its count walked every entry before it
lines.push(JSON.stringify({ $: "ProviderSet", id: "t0", provider: "codex", model: "", at: at + PER + 3 }));
lines.push(JSON.stringify({ $: "MessagePosted", thread: "t1", msg: "m-last", role: "user", text: "after the switch", at: at + PER + 4 }));
lines.push(JSON.stringify({ $: "SettingSet", key: "restart.continue", value: "off", at: at + PER + 5 }));
writeFileSync(join(home, "events.jsonl"), lines.join("\n") + "\n");
const head = lines.length;

const hubPort = freePort();
const hub = Bun.spawn([resolve(bin), "--home", home, "--port", String(hubPort), "--no-tailscale", "--foreground"], {
  // no agent on the PATH: nothing in the log may start one
  env: { PATH: "/usr/bin:/bin", HOME: process.env.HOME ?? "", DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "" },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  let up = false;
  for (let i = 0; i < 600 && !up; i++) {
    try {
      up = (await fetch(`http://127.0.0.1:${hubPort}/hello`)).status === 200;
    } catch {}
    if (!up) await sleep(100);
  }
  check(up, "the hub serves the log");
  const tok = readFileSync(join(home, "token"), "utf8").trim();
  const child = Bun.spawn([process.execPath, import.meta.path, bin, resolve(bridge), "child", String(hubPort), tok], {
    env: { ...process.env, HEAD: String(head), BUN_JSC_useJIT: "0", BUN_JSC_maxPerThreadStackUsage: "4194304" },
    stdout: "pipe",
    stderr: "inherit",
  });
  const text = await new Response(child.stdout).text();
  await child.exited;
  const o = JSON.parse(text.trim().split("\n").pop() || "{}");
  check(o.throws === 0, `no call throws at the phone's stack (${o.throws})`);
  check(o.since >= head, `the whole log arrived (${o.since} of ${head})`);
  const c: number[] = o.costs ?? [];
  const q = Math.floor(c.length / 4);
  const sum = (a: number[]) => a.reduce((x, y) => x + y, 0);
  const first = sum(c.slice(0, q)) / Math.max(q, 1);
  const last = sum(c.slice(c.length - q)) / Math.max(q, 1);
  console.log(`     ${c.length} frames in ${(sum(c) / 1000).toFixed(1)} s, ${first.toFixed(1)} ms a frame in the first quarter, ${last.toFixed(1)} ms in the last`);
  check(last < 2 * first + 5, "a frame costs as much late in the join as early (no walk of the whole state per call)");
  const s = o.screen ?? {};
  check(mine(s) === PROJECTS, `every project is on the screen (${mine(s)})`);
  // the fields iOS's Screen needs (Screen.swift: not optional there)
  const str = (v: any) => typeof v === "string";
  const arr = (v: any) => Array.isArray(v);
  const need: [string, (v: any) => boolean][] = [
    ["online", (v) => typeof v === "boolean"],
    ...["version", "error", "note", "sel", "empty", "hub"].map((k) => [k, str] as [string, (v: any) => boolean]),
    ...["hubs", "found", "projects", "bots", "rooms"].map((k) => [k, arr] as [string, (v: any) => boolean]),
    ["island", (v) => v && Number.isInteger(v.running) && str(v.headline) && arr(v.lines)],
  ];
  const bad = need.filter(([k, ok]) => !ok(s[k])).map(([k]) => k);
  check(bad.length === 0, `the screen has every field iOS needs, typed (${bad.join(", ") || "all"})`);
  const projOk = (s.projects ?? []).every(
    (p: any) => [p.id, p.title, p.root, p.machine, p.snoozedShelf, p.shelf].every(str) && typeof p.open === "boolean" && [p.threads, p.snoozed, p.settled].every(arr),
  );
  check(projOk, "every project has the fields iOS needs");
  check(o.keptProjects === PROJECTS, `the kept state loads (${o.keptProjects} projects)`);
  check(o.olderProjects === PROJECTS && o.olderSteps, `a state kept by an older build (BigInt naturals) loads and steps (${o.olderProjects} projects)`);
} finally {
  hub.kill();
  await hub.exited;
  rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all passed");
process.exit(fail ? 1 : 0);
