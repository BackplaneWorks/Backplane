// Directory timing and persistence with two real temp-home hubs linked
// through bots.invite/bots.join, plus a TCP peer that can hang forever.
// bun test/tools/botdir_e2e.ts BINARY [WIREDIR] [--baseline]
import { createHmac } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, writeFileSync, chmodSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const [bin = "build/dir-after", wire = "build/wire"] = process.argv.slice(2).filter(x => !x.startsWith("--"));
const baseline = process.argv.includes("--baseline");
for (const f of readdirSync(wire).filter(x => x.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const root = mkdtempSync(join(tmpdir(), "bp-botdir-"));
const fakebin = join(root, "bin"); mkdirSync(fakebin);
for (const n of ["claude", "codex", "grok", "muse"]) { writeFileSync(join(fakebin, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fakebin, n), 0o755); }
const sleep = (n: number) => new Promise(r => setTimeout(r, n));
let failed = false;
function check(name: string, ok: unknown, got?: unknown) { console.log(`${ok ? "ok" : "FAIL"} ${name}${ok ? "" : ": " + JSON.stringify(got)}`); if (!ok) failed = true; }
async function until(ms: number, f: () => any) { const end = Date.now() + ms; do { const v = f(); if (v) return v; await sleep(20); } while (Date.now() < end); }
function freePort() { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; }
const procs: ReturnType<typeof Bun.spawn>[] = [];
type Hub = { name: string; home: string; port: number; proc: ReturnType<typeof Bun.spawn>; ws: WebSocket; n: number; replies: Map<number, any>; info: any; seen: any[]; started: number; infoAt: number; log: string };
async function start(name: string, port = freePort()): Promise<Hub> {
  const home = join(root, name); const started = Date.now();
  const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
    env: { ...process.env, HOME: root, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NAME: name, BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fakebin, PATH: `${fakebin}:${process.env.PATH}` }, stdout: "ignore", stderr: "pipe" });
  procs.push(proc);
  const h: Hub = { name, home, port, proc, ws: null as any, n: 0, replies: new Map(), info: null, seen: [], started, infoAt: 0, log: "" };
  void (async () => { for await (const b of proc.stderr as any) h.log += new TextDecoder().decode(b); })();
  for (let i = 0; i < 500; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {} await sleep(20); }
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
  ws.onmessage = e => { const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer))); if (o.info) { h.info = o.info; if (!h.infoAt) h.infoAt = Date.now(); } if (o.t === "reply") h.replies.set(Number(o.id), o); h.seen.push(...(o.items ?? [])); };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; }); h.ws = ws;
  await until(10000, () => h.info); return h;
}
async function stop(h: Hub) { h.ws.close(); h.proc.kill(); await h.proc.exited; }
async function rpc(h: Hub, m: string, p: object) { const id = ++h.n; h.ws.send(W.encode(JSON.stringify({ id, m, p }))); return until(10000, () => h.replies.get(id)); }
function rows(h: Hub): any[] { return JSON.parse(h.info?.["bots.remote"] ?? "[]"); }
function bot(h: Hub, name: string, state?: string) { return rows(h).find(o => o.name === name && (!state || o.state === state)); }
function signature(secret: string, now: number, body: string) { return "sha256=" + createHmac("sha256", Buffer.from(secret, "hex")).update(`${now}.${body}`).digest("hex"); }
async function signed(h: Hub, peer: string, secret: string, path: string, body?: string) {
  const ts = Math.floor(Date.now() / 1000), data = body ?? "";
  return fetch(`http://127.0.0.1:${h.port}${path}`, { method: body === undefined ? "GET" : "POST", body, headers: { "content-type": "application/json", "x-backplane-peer": peer, "x-backplane-timestamp": String(ts), "x-backplane-signature": signature(secret, ts, data) } });
}
// The third peer is a separate process: every directory connection is
// counted, and hanging sockets stay open until curl's deadline closes them.
const fakePath = join(root, "fake.ts"), modePath = join(root, "mode"), statsPath = join(root, "stats");
writeFileSync(modePath, "ready");
writeFileSync(fakePath, `import {readFileSync, writeFileSync} from 'node:fs';
let requests=0,active=0,max=0; const save=()=>writeFileSync(${JSON.stringify(statsPath)},JSON.stringify({requests,active,max}));
const s=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(sock,data){
 if(sock.data) return; sock.data=true; const request=new TextDecoder().decode(data);
 if(request.startsWith('GET /bots/dir ')){requests++;active++;max=Math.max(max,active);sock.data='dir';save();
 const mode=readFileSync(${JSON.stringify(modePath)},'utf8'); if(mode==='hang') return;
 const body=mode==='invalid'?'{}':JSON.stringify([{name:'ghost',look:2,state:'ready'}]);
 sock.end('HTTP/1.1 200 OK\\r\\nContent-Length: '+Buffer.byteLength(body)+'\\r\\nConnection: close\\r\\n\\r\\n'+body);
 } else sock.end('HTTP/1.1 404 Not Found\\r\\nContent-Length: 0\\r\\nConnection: close\\r\\n\\r\\n');
},close(sock){if(sock.data==='dir'){active--;save();}},error(){}}});console.log(s.port);`);
const fake = Bun.spawn([process.execPath, fakePath], { stdout: "pipe", stderr: "inherit" }); procs.push(fake);
const reader = fake.stdout.getReader(); const portLine = await reader.read(); const fakePort = Number(new TextDecoder().decode(portLine.value).trim()); reader.releaseLock();
const stats = () => { try { return JSON.parse(readFileSync(statsPath, "utf8")); } catch { return { requests: 0, active: 0, max: 0 }; } };
try {
  let a = await start("alpha"), b = await start("beta");
  check("real hubs start", a.info && b.info);
  check("create healthy bot", (await rpc(b, "bots.create", { name: "miso" }))?.ok);
  check("create inviter bot", (await rpc(a, "bots.create", { name: "nori" }))?.ok);
  const inv = await rpc(a, "bots.invite", { url: `http://127.0.0.1:${a.port}` });
  const linkedAt = Date.now(); check("link by invite/join", (await rpc(b, "bots.join", { invite: inv?.invite }))?.ok);
  if (!baseline) {
    check("join refreshes both sides before minute tick", await until(5000, () => bot(a, "miso") && bot(b, "nori")), [rows(a), rows(b)]);
    console.log(`MEASURE link_to_both_directories_ms=${Date.now() - linkedAt}`);
  }
  // A normal signed link from the fake peer; the hub owns the invite and
  // its secret exactly as for a real joining hub.
  const fi = await rpc(a, "bots.invite", { url: `http://127.0.0.1:${a.port}` });
  const secret = JSON.parse(readFileSync(join(a.home, "secrets", "peers", fi.peer), "utf8")).secret;
  check("link TCP fake peer", (await signed(a, fi.peer, secret, "/bots/link", JSON.stringify({ name: "fake", url: `http://127.0.0.1:${fakePort}` }))).ok);
  if (!baseline) check("cache fake peer's prior directory", await until(5000, () => bot(a, "ghost", "ready")), rows(a));
  // Leave a whole second between signed empty-body requests so the
  // existing replay protection cannot mistake a restart for a replay.
  await sleep(1200); writeFileSync(modePath, "hang");
  const aPort = a.port; await stop(a); const beforeRequests = stats().requests; a = await start("alpha", aPort);
  const initial = rows(a);
  const seen = await until(22000, () => bot(a, "miso", "ready"));
  const startMs = Date.now() - a.started;
  console.log(`MEASURE mode=${baseline ? "before" : "after"} start_to_info_ms=${a.infoAt - a.started} start_to_healthy_bot_ms=${startMs} fake_requests=${stats().requests - beforeRequests} fake_max_active=${stats().max}`);
  check("healthy peer bot appears with hanging TCP peer", seen, rows(a));
  if (!baseline) {
    check("startup serves cached rows immediately, away with seen", initial.some(o => o.name === "ghost" && o.state === "away" && o.seen > 0), initial);
    check("healthy result publishes before dead peer deadline", startMs < 3500, startMs);
    const ghostSeen = initial.find(o => o.name === "ghost")?.seen;
    const hungAt = Date.now(); await until(5500, () => stats().active === 0);
    console.log(`MEASURE fake_timeout_observed_ms=${Date.now() - a.started} after_healthy_ms=${Date.now() - hungAt}`);
    check("dead peer keeps its rows away and its last-seen time", bot(a, "ghost", "away")?.seen === ghostSeen, rows(a));
    check("failed peer never removes healthy peer", bot(a, "miso", "ready"), rows(a));
    check("directory fetch has short total deadline", Date.now() - a.started < 5500, Date.now() - a.started);
    check("cache lives outside event log and secrets", existsSync(join(a.home, "bots.cache")) && !readFileSync(join(a.home, "events.jsonl"), "utf8").includes('"seen"'), rows(a));
    // An authenticated return signal while a fetch is active is coalesced.
    await sleep(1100); const r0 = stats().requests;
    await signed(a, fi.peer, secret, "/bots/deliver", JSON.stringify({ to: "person", from: "ghost", text: "return one" }));
    await until(2000, () => stats().requests > r0);
    await signed(a, fi.peer, secret, "/bots/deliver", JSON.stringify({ to: "person", from: "ghost", text: "return two" }));
    await sleep(200); check("return triggers do not overlap a peer's fetch", stats().requests === r0 + 1 && stats().max === 1, stats());
    await until(5000, () => stats().active === 0);
    writeFileSync(modePath, "ready"); await sleep(1100); const returnedAt = Date.now();
    await signed(a, fi.peer, secret, "/bots/deliver", JSON.stringify({ to: "person", from: "ghost", text: "back online" }));
    check("peer return refreshes before minute tick", await until(2500, () => bot(a, "ghost", "ready")), rows(a));
    console.log(`MEASURE return_to_directory_ms=${Date.now() - returnedAt}`);
    writeFileSync(modePath, "invalid"); await sleep(1100);
    // Restart to trigger a sweep against an invalid successful JSON body.
    await stop(a); a = await start("alpha", aPort);
    await until(2000, () => stats().active === 0 && bot(a, "miso", "ready"));
    check("invalid JSON shape retains cached peer away", bot(a, "ghost", "away"), rows(a));
    const un = await rpc(a, "bots.unpeer", { peer: fi.peer });
    check("revoking peer removes cached directory", un?.ok && await until(2000, () => !bot(a, "ghost")), rows(a));
    await sleep(1100); await stop(a); a = await start("alpha", aPort);
    check("revoked cache stays gone across restart", !bot(a, "ghost"), rows(a));
  }
  await stop(a); await stop(b);
} catch (e) { failed = true; console.error(e); }
finally { for (const p of procs) { if (p.exitCode === null) { p.kill(); await p.exited; } } rmSync(root, { recursive: true, force: true }); }
process.exit(failed ? 1 : 0);
