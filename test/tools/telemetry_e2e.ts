// bun test/tools/telemetry_e2e.ts [BINARY] [WIREDIR]
// Temporary homes, random ports, generated dev key. Never production secrets.
import { Database } from "bun:sqlite";
import { createHmac, createHash, randomBytes } from "node:crypto";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, readdirSync, statSync, chmodSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir, hostname, userInfo } from "node:os";
const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const report = resolve("build/telemetry-report"); mkdirSync(report, { recursive: true });
const root = mkdtempSync(join(tmpdir(), "bp-telemetry-"));
const key = randomBytes(32), kid = "dev-1";
const requests: { raw: string; body: any; status: number }[] = [];
let status = 202, fails = 0, serial = 0, ws: WebSocket | undefined;
const check = (ok: boolean, label: string) => { console.log(`${ok ? "ok" : "FAIL"} ${label}`); if (!ok) fails++; };
const delay = (ms: number) => new Promise(r => setTimeout(r, ms));
const until = async <T>(ms: number, fn: () => T | undefined, label: string): Promise<T> => {
  const end = Date.now() + ms;
  while (Date.now() < end) { const v = fn(); if (v !== undefined) return v; await delay(50); }
  throw new Error(`timed out: ${label}`);
};
const pow = (ev: any) => /^[0-9a-f]{1,16}$/.test(ev.pow ?? "") && (createHash("sha256").update(`bp-pow1:${ev.id}:${ev.pow}`).digest().readUInt32BE() >>> 14) === 0;
const stub = Bun.serve({ hostname: "127.0.0.1", port: 0, async fetch(req) {
  const raw = await req.text(); const body = JSON.parse(raw);
  const sig = "sha256=" + createHmac("sha256", key).update(`${body.ts}.${raw}`).digest("hex");
  check(req.headers.get("X-Backplane-Key") === kid && req.headers.get("X-Backplane-Timestamp") === String(body.ts) && req.headers.get("X-Backplane-Signature") === sig, "stub validates HMAC and timestamp headers");
  check(raw.length <= 16384 && body.v === 1 && /^[0-9a-f]{16}$/.test(body.nonce) && body.events.length >= 1 && body.events.length <= 50, "batch limits and nonce");
  check(!requests.some(x => x.body.nonce === body.nonce), "nonce fresh per request");
  for (const ev of body.events) {
    check(!("clients" in ev) && !(["made", "held", "busy"].some(k => k in ev)), "no clients or local bookkeeping on wire");
    if (ev.share) check(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(ev.id), "identified event has UUIDv4");
    else check(!("id" in ev) && !("pow" in ev) && ev.e !== "active", "anonymous event has no id, pow or activity");
    if (ev.e === "install" && ev.share) check(pow(ev), "18-bit install proof of work");
  }
  requests.push({ raw, body, status });
  return new Response(status === 202 ? "{}" : '{"error":"test"}', { status, headers: { "Retry-After": "1" } });
}});
const fake = join(root, "bin"); mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse", "antigravity"]) { const f = join(fake, n); writeFileSync(f, "#!/bin/sh\nexit 1\n"); chmodSync(f, 0o755); }
const homes: string[] = [];
const owned: ReturnType<typeof Bun.spawn>[] = [];
let hub: ReturnType<typeof Bun.spawn> | undefined, port = 0;
const state = (home: string) => { try { return JSON.parse(readFileSync(join(home, "telemetry/state.json"), "utf8")); } catch { return undefined; } };
const home = (label: string) => { const h = join(root, label); mkdirSync(h); writeFileSync(join(h, "events.jsonl"), ""); homes.push(h); return h; };
const baseEnv = { ...process.env, HOME: root, XDG_CACHE_HOME: join(root, "cache"), DISPLAY: "", WAYLAND_DISPLAY: "", CI: "", DO_NOT_TRACK: "", BACKPLANE_SANDBOX: "", BACKPLANE_TELEMETRY: "on", BACKPLANE_TELEMETRY_TEST: "1", BACKPLANE_TELEMETRY_CHANNEL: "dev", BACKPLANE_TELEMETRY_URL: `http://127.0.0.1:${stub.port}/api/v1/events`, BACKPLANE_TELEMETRY_KID: kid, BACKPLANE_TELEMETRY_KEY: key.toString("hex"), BACKPLANE_TELEMETRY_DAY_SECS: "3", BACKPLANE_WAKE_TICK_MS: "500", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_PEERS: "", BEND_NO_TELEMETRY: "1", BACKPLANE_LOCK_DIR: join(root, "locks"), PATH: `${fake}:${process.env.PATH}` };
const start = async (h: string, extra: Record<string, string> = {}, args: string[] = []) => {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); port = s.port; s.stop(true);
  if (port === 3787) throw new Error("refusing live port");
  hub = Bun.spawn([resolve(bin), "--home", h, "--web", resolve("build/telemetry-web"), "--port", String(port), "--no-tailscale", "--foreground", "--headless", ...args], { env: { ...baseEnv, ...extra }, stdout: Bun.file(join(h, "stdout.log")), stderr: Bun.file(join(h, "hub.log")) }); owned.push(hub);
  const end = Date.now() + 30000;
  while (Date.now() < end) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) return; } catch {} if (hub.exitCode != null) throw new Error(`hub exited: ${readFileSync(join(h, "hub.log"), "utf8")}`); await delay(100); }
  throw new Error(`hub start timeout: ${readFileSync(join(h, "hub.log"), "utf8")}`);
};
const stop = async () => { ws?.close(); ws = undefined; if (hub && hub.exitCode == null) { hub.kill(); await hub.exited; } hub = undefined; };
let infoId = "";
const pending = new Map<string, (o: any) => void>();
const connect = async () => {
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
  ws.onmessage = e => { const o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data))); if (o.info && "telemetry.id" in o.info) infoId = o.info["telemetry.id"]; if (o.t === "reply") { pending.get(String(o.id))?.(o); pending.delete(String(o.id)); } };
  await new Promise((r, j) => { ws!.onopen = r; ws!.onerror = j; });
};
const rpc = (m: string, p: object) => new Promise<any>((resolve, reject) => {
  const id = String(++serial); const timer = setTimeout(() => { pending.delete(id); reject(new Error(`RPC timeout: ${m}`)); }, 10000);
  pending.set(id, o => { clearTimeout(timer); if (o.ok === false || o.error) reject(new Error(JSON.stringify(o))); else resolve(o); });
  ws!.send(W.encode(JSON.stringify({ id, m, p })));
});
const setting = (key: string, value: string) => rpc("setting.set", { key, value });
const queueEvent = (ver: string) => ({ e: "update", day: new Date().toISOString().slice(0, 10), share: false, id: "", ver, prev: "0.11.3", ch: "release", os: "linux", arch: "x64", pow: "", made: Math.floor(Date.now()/1000), held: false });
try {
  const h = home("main"); await start(h); await connect();
  await until(10000, () => state(h)?.seen ? true : undefined, "held install state");
  await delay(1100); check(requests.length === 0, "unasked events held; no identified requests");
  check(state(h).id === "" && state(h).queue.every((e: any) => !e.id && !e.share), "unasked local events have no id");
  const proj = join(root, "private-project-telemetry-e2e"); mkdirSync(proj);
  await rpc("project.add", { path: proj });
  await setting("telemetry.share", "on"); await setting("telemetry.asked", "1");
  await until(15000, () => requests.find(r => r.body.events.some((e: any) => e.e === "install" && e.share)) ?? undefined, "identified install");
  check((statSync(join(h, "telemetry/state.json")).mode & 0o777) === 0o600, "state mode 0600");
  check((statSync(join(h, "telemetry")).mode & 0o777) === 0o700, "telemetry directory mode 0700");
  await until(12000, () => requests.find(r => r.body.events.some((e: any) => e.e === "active")) ?? undefined, "shortened-day active");
  check(true, "UTC day rollover emits active without clients");
  let oldId = state(h).id;
  await until(6000, () => infoId === oldId ? true : undefined, "install id delivered as info");
  check(infoId === oldId, "current id delivered only in telemetry.id info");
  check(!readFileSync(join(h, "events.jsonl"), "utf8").includes(oldId), "id absent from event log");
  const resetBefore = requests.length;
  await setting("telemetry.reset", "1");
  await until(15000, () => state(h)?.id && state(h).id !== oldId ? true : undefined, "reset creates fresh identity");
  const resetId = state(h).id;
  await until(10000, () => requests.slice(resetBefore).some(r => r.body.events.some((e: any) => e.e === "install" && e.id === resetId && pow(e))) ? true : undefined, "reset registers verified install");
  check(true, "reset replaces identity and registers with fresh proof");
  oldId = resetId;
  await setting("telemetry.share", "off");
  await until(6000, () => state(h)?.id === "" ? true : undefined, "opt-out persisted");
  await until(6000, () => infoId === "" ? true : undefined, "opt-out id info cleared");
  check(infoId === "", "opt-out clears id info");
  const offStart = requests.length;
  await stop();
  // Seed a version transition while opted out; it must be reported id-less.
  const st = state(h); st.ver = "0.11.2"; writeFileSync(join(h, "telemetry/state.json"), JSON.stringify(st), { mode: 0o600 });
  await start(h); await connect();
  await until(10000, () => requests.length > offStart ? true : undefined, "anonymous update");
  check(requests.slice(offStart).every(r => !r.raw.includes('"id"')), "all post-opt-out bodies have no id anywhere");
  await stop();
  // 500 retains and schedules >= an hour. Use a pending anonymous update.
  const st500 = state(h); st500.queue = [queueEvent("0.11.4")]; st500.next = 0;
  writeFileSync(join(h, "telemetry/state.json"), JSON.stringify(st500)); status = 500; const failStart = requests.length; const now500 = Math.floor(Date.now()/1000);
  await start(h);
  await until(10000, () => (state(h)?.next >= now500 + 3600 && requests.length > failStart) ? true : undefined, "500 backoff");
  check(state(h).queue.length > 0 && state(h).next >= now500 + 3600, "500 keeps queue and sets hour backoff");
  await delay(1100); check(requests.length === failStart + 1, "no retry storm during backoff"); await stop();
  const st400 = state(h); st400.next = 0; writeFileSync(join(h, "telemetry/state.json"), JSON.stringify(st400)); status = 400;
  await start(h); await until(10000, () => state(h)?.queue.length === 0 ? true : undefined, "400 permanent drop");
  check(true, "400 drops batch"); await stop(); status = 202;
  // An actual refused TCP connection follows the same bounded retry path.
  const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const offlinePort = listener.port; listener.stop(true);
  const offline = state(h); offline.queue = [queueEvent("0.11.5")]; offline.next = 0;
  writeFileSync(join(h, "telemetry/state.json"), JSON.stringify(offline));
  const offlineNow = Math.floor(Date.now() / 1000);
  await start(h, { BACKPLANE_TELEMETRY_URL: `http://127.0.0.1:${offlinePort}/api/v1/events` });
  await until(10000, () => state(h)?.next >= offlineNow + 3600 ? true : undefined, "offline connection backoff");
  check(state(h).queue.length > 0, "offline keeps queue for bounded later retry");
  await stop();
  for (const [label, env, args] of [
    ["sandbox", { BACKPLANE_SANDBOX: "1" }, []], ["disabled", { BACKPLANE_TELEMETRY: "0" }, []], ["sandbox-flag", {}, ["--sandbox"]], ["dnt", { DO_NOT_TRACK: "1" }, []], ["ci", { CI: "1" }, []], ["no-key", { BACKPLANE_TELEMETRY_KEY: "" }, []], ["no-url", { BACKPLANE_TELEMETRY_URL: "" }, []], ["dev-without-explicit-test", { BACKPLANE_TELEMETRY_TEST: "" }, []]
  ] as [string, Record<string,string>, string[]][]) {
    const hh = home(label), before = requests.length; await start(hh, env, args); await delay(1500); await stop();
    check(requests.length === before && !existsSync(join(hh, "telemetry")), `${label}: zero requests and no telemetry folder`);
  }
  const forbidden = [root, hostname(), "private-project-telemetry-e2e"];
  check(requests.every(r => forbidden.every(s => !r.raw.includes(s))), "bodies contain no temp path, hostname or project name");
  const user = userInfo().username;
  check(requests.every(r => r.body.events.every((e: any) => Object.values(e).every(v => typeof v !== "string" || (user.length >= 3 ? !v.includes(user) : v !== user)))), "event values contain no user name");
  if (process.env.TM_SERVER_ROOT) {
    // Run the real Flask app without changing its code/configuration or using
    // production credentials/data. Only the native hub signs and sends here.
    const backend = resolve(process.env.TM_SERVER_ROOT);
    const dbfile = join(root, "real-server.sqlite3");
    const env = { ...process.env, TELEMETRY_DATABASE: dbfile,
      SIGNUP_DATABASE: join(root, "unused-signups.sqlite3"),
      SECRET_KEY: randomBytes(32).toString("hex"),
      TELEMETRY_KEYS: JSON.stringify({ [kid]: { secret: key.toString("hex"), channels: ["dev"], versions: ["*"] } }),
      TM_SERVER_HEAD: process.env.TM_SERVER_HEAD ?? "cda39a4585f2c06014da7ed6b68f78ec087caa5f" };
    const python = process.env.TM_SERVER_PYTHON;
    const command = python ? [python] : ["uv", "run", "--frozen", "--project", backend, "python"];
    const server = Bun.spawn([...command, resolve("test/tools/telemetry_server_fixture.py"), backend],
      { env, stdout: "pipe", stderr: Bun.file(join(root, "real-server.log")) });
    owned.push(server);
    const reader = server.stdout.getReader(); let line = "";
    while (!line.includes("\n")) {
      const item = await reader.read(); if (item.done) throw new Error("real server did not start");
      line += new TextDecoder().decode(item.value);
    }
    const receipt = JSON.parse(line.split("\n")[0]);
    const endpoint = `http://127.0.0.1:${receipt.port}/api/v1/events`;
    const counts = () => {
      if (!existsSync(dbfile)) return [];
      const db = new Database(dbfile, { readonly: true });
      try { return db.query("SELECT e, share, COUNT(*) AS n, MIN(verified) AS verified, SUM(install_id IS NULL) AS anonymous FROM events GROUP BY e, share").all() as any[]; }
      finally { db.close(); }
    };
    const realHome = home("real-client");
    await start(realHome, { BACKPLANE_TELEMETRY_URL: endpoint }); await connect();
    await setting("telemetry.share", "on"); await setting("telemetry.asked", "1");
    await until(20000, () => counts().some(e => e.e === "install" && e.share === 1 && e.verified === 1) ? true : undefined, "native signed real-server install");
    await until(12000, () => counts().some(e => e.e === "active" && e.share === 1) ? true : undefined, "native real-server active");
    check(true, "actual native client ingestion: install + verified PoW + active in real server SQLite");
    await stop();
    const before = state(realHome); before.ver = "0.11.2";
    writeFileSync(join(realHome, "telemetry/state.json"), JSON.stringify(before), { mode: 0o600 });
    await start(realHome, { BACKPLANE_TELEMETRY_URL: endpoint }); await connect();
    await until(15000, () => counts().some(e => e.e === "update" && e.share === 1) ? true : undefined, "native real-server version update");
    await setting("telemetry.share", "off");
    await until(10000, () => state(realHome)?.id === "" ? true : undefined, "real client opt-out");
    await stop();
    const after = state(realHome); after.ver = "0.11.1";
    writeFileSync(join(realHome, "telemetry/state.json"), JSON.stringify(after), { mode: 0o600 });
    await start(realHome, { BACKPLANE_TELEMETRY_URL: endpoint });
    await until(15000, () => counts().some(e => e.e === "update" && e.share === 0 && e.anonymous === e.n) ? true : undefined, "anonymous native real-server update");
    await stop();
    const saved = { server_source: receipt.source, binary_sha256: createHash("sha256").update(readFileSync(resolve(bin))).digest("hex"), counts: counts() };
    writeFileSync(join(report, "real-server-receipt.json"), JSON.stringify(saved, null, 2));
    check(true, "real-server update and opted-out identifier-free update persisted");
  } else check(false, "TM_SERVER_ROOT required for actual signed local-server integration");

} catch (e) { console.error(e); fails++; }
finally {
  await stop(); for (const p of owned) if (p.exitCode == null) { p.kill(); await p.exited; } stub.stop(true);
  writeFileSync(join(report, "requests.json"), JSON.stringify(requests, null, 2));
  writeFileSync(join(report, "e2e-home.txt"), root + "\n");
  for (const h of homes) if (existsSync(join(h, "hub.log"))) writeFileSync(join(report, `${h.split("/").at(-1)}.hub.log`), readFileSync(join(h, "hub.log")));
  console.log(`artifacts ${report}`); console.log(`temporary homes ${root}`);
}
if (fails) process.exit(1);
console.log(`ok telemetry e2e (${requests.length} requests)`);
