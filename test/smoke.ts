// Smoke test for a built server: page, /hello, websocket (binary CBOR
// frames, through the hub's own codec built from test/wire), one project +
// thread, persisted log. Usage: bun test/smoke.ts PORT HOME WIREDIR
import { readdirSync, readFileSync } from "fs";
const hubToken = (dir: string): string => require('node:fs').readFileSync(dir + '/token', 'utf8').trim();
const [port, home, wire] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const base = `http://127.0.0.1:${port}`;
const fail = (m: string) => { console.error("smoke: " + m); process.exit(1); };

const page = await fetch(base + "/");
if (page.status !== 200 || !(await page.text()).includes("<title>Backplane</title>")) fail("index.html not served");

const hello = await fetch(base + "/hello");
if (hello.headers.get("content-type") !== "application/cbor" || !JSON.parse(W.decode(new Uint8Array(await hello.arrayBuffer()))).backplane)
  fail("/hello did not answer CBOR");

const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${hubToken(home)}`);
ws.binaryType = "arraybuffer";
const seen: any[] = [];
let n = 0;
const send = (m: string, p: any) => ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));
const done = new Promise<void>((resolve) => {
  ws.onmessage = (e) => {
    if (typeof e.data === "string") fail("the hub sent a text frame");
    const o = JSON.parse(W.decode(new Uint8Array(e.data)));
    for (const c of o.items ?? []) seen.push(c);
    const proj = seen.find((c) => c.$ === "ProjectCreated");
    if (o.t === "log") send("project.add", { path: home });
    if (o.t === "changes" && proj && !seen.some((c) => c.$ === "ThreadCreated")) send("thread.create", { project: proj.id, title: "Smoke" });
    if (seen.some((c) => c.$ === "ThreadCreated")) resolve();
  };
});
ws.onerror = () => fail("websocket failed");
await Promise.race([done, new Promise((_, rej) => setTimeout(() => rej(new Error("timeout")), 5000))]).catch((e) => fail(String(e)));
ws.close();

const log = await Bun.file(`${home}/events.jsonl`).text();
if (!log.includes('"ThreadCreated"') || !log.includes('"ProjectCreated"')) fail("events.jsonl missing changes");
console.log("smoke: ok");
process.exit(0);
