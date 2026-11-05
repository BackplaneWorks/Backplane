// bun replay.ts GREETING.bin REPLY.bin : drive the bridge with captured frames, report which check fails.
import { readFileSync } from "node:fs";
(0, eval)(readFileSync(new URL("./bridge-instrumented.js", import.meta.url), "utf8"));
const B = (globalThis as any).Backplane; const [g, r] = process.argv.slice(2);
const key = "desktop.tail9e0cf2.ts.net:3815", ep = "1";
const j = (x: any) => typeof x === "string" ? JSON.parse(x) : x;
const b64 = (f: string) => readFileSync(f).toString("base64");
B.start("replay", "{}");
B.hubs([key]);
B.connection(key, ep, "view-v1");
B.online(key, true, ep);
let o = j(B.recv(key, b64(g), ep, false, false));
const send = (o.cmds || []).find((c: any) => c.type === "send");
if (!send) { console.log("no view.open sent", JSON.stringify(o).slice(0, 300)); process.exit(1); }
const req = Buffer.from(send.data, "base64");
const idm = /(\d{15})/.exec(req.toString("latin1"))![1];
let rep = readFileSync(r); const old = /(\d{15})/.exec(rep.toString("latin1"))![1];
rep = Buffer.from(rep.toString("latin1").replace(old, idm), "latin1");
o = j(B.recv(key, rep.toString("base64"), ep, false, false));
console.log("sync", JSON.stringify(o.sync), "screen", !!o.screen);
console.log("checks", JSON.stringify((globalThis as any).__chk));
const G = (globalThis as any).__G; const R = G.result; const sz = (x: any) => Buffer.byteLength(JSON.stringify(x));
console.log("G total", sz(G), "result keys", Object.entries(R).map(([k, v]) => `${k}:${sz(v)}`).join(" "));
console.log("shown keys", Object.entries(R.shown).map(([k, v]) => `${k}:${sz(v)}`).filter(s => +s.split(":")[1] > 500).join(" "));
console.log("overlay keys", Object.entries(R.overlay).map(([k, v]: any) => `${k}:${sz(v)}`).join(" ").slice(0, 300));
console.log("sample", JSON.stringify(R.shown.projects).slice(0, 700));
console.log("G keys", Object.entries(G).map(([k, v]) => `${k}:${sz(v)}`).join(" "));
const sj = JSON.stringify(R.shown.projects[0].threads[0]); console.log("one thread row json bytes", Buffer.byteLength(sj)); console.log(sj.slice(0, 1500));
console.log("rows", R.shown.projects[0].threads.length, "total projects json", sz(R.shown.projects));
