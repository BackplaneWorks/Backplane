// Times the phone client (bridge.js) on a hub's view lane (view-v1), as the
// apps run it today: no JIT and the phone's stack. Starts a headless hub on
// a made-up log, then a child bun joins it through bridge.js the way the iOS
// app does (connection(key, epoch, "view-v1"), the view socket, each frame
// handed to recv as base64), opens threads and types into a draft as each
// app does (iOS draftAt, Android quiet "draft"). Prints ms per screen, per
// key, per thread switch, how many keys drew a screen (none should), and
// the composer each key answers with (its popup rows and send button).
// Compare builds with each other.
//
//   bun test/tools/phone_thin_bench.ts BINARY BRIDGE [keys]
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane-serve", bridge = "mobile/build/assets/bridge.js", a3 = "40", mode, portArg, token] = process.argv.slice(2);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const PROJECTS = 6, THREADS = 24, PER = 400;
const stats = (xs: number[]) => {
  const s = [...xs].sort((a, b) => a - b);
  const mean = s.reduce((a, b) => a + b, 0) / Math.max(s.length, 1);
  return { mean: +mean.toFixed(2), p50: +(s[s.length >> 1] ?? 0).toFixed(2), max: +(s[s.length - 1] ?? 0).toFixed(2), n: s.length };
};

if (mode === "child") {
  (0, eval)(readFileSync(bridge, "utf8"));
  const B = (globalThis as any).Backplane;
  const key = `127.0.0.1:${portArg}`;
  const epoch = "1";
  let ws: WebSocket | null = null;
  let screen: any = null, throws = 0;
  const handle = (t: string) => {
    const o = JSON.parse(t);
    if (o.screen) screen = o.screen;
    for (const c of o.cmds ?? []) if (c.type === "send" && c.hub === key) ws?.send(Buffer.from(c.data, "base64"));
    return o;
  };
  const call = (f: () => string) => { try { return handle(f()); } catch (e) { throws++; console.error("throw:", String(e).slice(0, 200)); return {}; } };
  const q: string[] = [];
  call(() => B.start("thin-bench", "{}"));
  call(() => B.hubs([key]));
  call(() => B.connection(key, epoch, "view-v1"));
  ws = new WebSocket(`ws://${key}/ws?token=${token}&view=1&mobile=1`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (m) => q.push(Buffer.from(new Uint8Array(m.data as ArrayBuffer)).toString("base64"));
  await new Promise((r, j) => { ws!.onopen = r; ws!.onerror = j; });
  call(() => B.online(key, true, false, epoch));
  // hand every frame that came to recv, as the app's worker does
  const drain = async (ms: number, until?: () => boolean) => {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      if (!q.length) { if (until?.()) return true; await sleep(5); continue; }
      const f = q.shift()!;
      call(() => B.recv(key, f, epoch, q.length > 0, false));
    }
    return !!until?.();
  };
  const shows = (id: string) => () => (screen?.thread?.id === id || screen?.thread?.id === `${key}|${id}`) && (screen?.thread?.entries?.length ?? 0) > 0;
  await drain(20000, () => (screen?.projects?.length ?? 0) > 0);
  const threads = ["t1", "t5", "t9", "t13"];
  const switches: number[] = [];
  for (const id of threads) {
    const t0 = performance.now();
    call(() => B.act("select", `${key}|${id}`));
    const ok = await drain(20000, shows(id));
    switches.push(performance.now() - t0);
    if (!ok) { console.error("no view of", id); throws++; }
  }
  await drain(500);
  const screensT: number[] = [];
  for (let i = 0; i < Number(process.env.SCREENS ?? 15); i++) { const t0 = performance.now(); call(() => B.screen()); screensT.push(performance.now() - t0); }
  // a key as each app sends it: iOS draftAt(thread, text, edit), Android
  // quiet("draft", text); neither asks for a screen
  const sel = String(screen?.thread?.id ?? "");
  const thread = sel.includes("|") ? sel : `${key}|${sel}`;
  const ios: number[] = [], android: number[] = [];
  let draft = "", edit = 0, screens = 0;
  const text = "the quick brown fox jumps over the lazy dog, again and again";
  for (let i = 0; i < Number(a3); i++) {
    draft += text[i % text.length];
    const t0 = performance.now();
    const o = call(() => B.draftAt(thread, draft, ++edit));
    ios.push(performance.now() - t0);
    if (o.screen) screens++;
    await drain(10);
  }
  call(() => B.draftAt(thread, "", ++edit));
  await drain(50);
  draft = "";
  for (let i = 0; i < Number(a3); i++) {
    draft += text[i % text.length];
    const t0 = performance.now();
    const o = call(() => B.quiet("draft", draft));
    android.push(performance.now() - t0);
    if (o.screen) screens++;
    await drain(10);
  }
  call(() => B.quiet("draft", ""));
  await drain(50);
  // what the composer offers while typing, from the key's own answer
  const probe: any = {};
  for (const d of ["/", "/co", "/btw why", "plain"]) {
    const a = call(() => B.draftAt(thread, d, ++edit));
    const q = call(() => B.quiet("draft", d));
    probe[d] = { ios: a.composer && { rows: (a.composer.skills ?? []).map((r: any) => r.name), send: a.composer.send, act: a.composer.sendAct },
      android: q.composer && { rows: (q.composer.skills ?? []).map((r: any) => r.name), send: q.composer.send } };
  }
  call(() => B.draftAt(thread, "", ++edit));
  const after = JSON.parse(B.screen()).screen?.thread;
  ws.close();
  console.log(JSON.stringify({ probe, screensPerKey: screens, screenRows: (after?.skills ?? []).length, throws, entries: screen?.thread?.entries?.length ?? 0, switch: stats(switches), screen: stats(screensT), keyIos: stats(ios), keyAndroid: stats(android) }));
  process.exit(0);
}

const root = mkdtempSync(join(tmpdir(), "bp-phonethin-"));
const home = join(root, "home");
mkdirSync(home);
const lines: string[] = [];
const at = Math.floor(Date.now() / 1000) - 3600 - PER;
const para = "A paragraph of a reply with `code`, **bold** words and a list:\n\n- one item\n- another item\n\n";
for (let p = 0; p < PROJECTS; p++) lines.push(JSON.stringify({ $: "ProjectCreated", id: `p${p}`, title: `Project ${p}`, root: join(root, `p${p}`), at }));
for (let t = 0; t < THREADS; t++)
  lines.push(JSON.stringify({ $: "ThreadCreated", id: `t${t}`, project: `p${t % PROJECTS}`, title: `Thread ${t}`, env: "local", branch: "", worktree: "", provider: "claude", model: "", at: at + 1 }));
for (let i = 0; i < PER; i++)
  for (let t = 0; t < THREADS; t++)
    lines.push(JSON.stringify({ $: "MessagePosted", thread: `t${t}`, msg: `m${t}-${i}`, role: i % 2 ? "assistant" : "user", text: i % 2 ? para.repeat(1 + (i % 4)) + `reply ${i}` : `message ${i} in thread ${t}`, at: at + 2 + i }));
lines.push(JSON.stringify({ $: "SettingSet", key: "restart.continue", value: "off", at: at + PER + 5 }));
writeFileSync(join(home, "events.jsonl"), lines.join("\n") + "\n");
const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const hubPort = s.port;
s.stop(true);
const hub = Bun.spawn([resolve(bin), "--home", home, "--port", String(hubPort), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
  env: { PATH: "/usr/bin:/bin", HOME: process.env.HOME ?? "", DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "" },
  stdout: "ignore", stderr: "ignore",
});
let code = 1;
try {
  let up = false;
  for (let i = 0; i < 600 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${hubPort}/hello`)).status === 200; } catch {} if (!up) await sleep(100); }
  if (!up) throw new Error("the hub did not start");
  const tok = readFileSync(join(home, "token"), "utf8").trim();
  // PROFILE=<dir>: the child under bun --cpu-prof (SCREENS=200 for a screen profile)
  const prof = process.env.PROFILE ? ["--cpu-prof", `--cpu-prof-dir=${resolve(process.env.PROFILE)}`] : [];
  const child = Bun.spawn([process.execPath, ...prof, import.meta.path, bin, resolve(bridge), a3, "child", String(hubPort), tok], {
    env: { ...process.env, BUN_JSC_useJIT: "0", BUN_JSC_maxPerThreadStackUsage: "4194304" }, stdout: "pipe", stderr: "inherit",
  });
  const out = await new Response(child.stdout).text();
  await child.exited;
  const o = JSON.parse(out.trim().split("\n").pop() || "{}");
  console.log(JSON.stringify(o, null, 1));
  code = o.throws === 0 && o.entries > 0 ? 0 : 1;
} finally {
  hub.kill();
  await hub.exited;
  rmSync(root, { recursive: true, force: true });
}
process.exit(code);
