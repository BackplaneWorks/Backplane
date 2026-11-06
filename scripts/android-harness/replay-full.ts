// bun replay-full.ts BRIDGE.js CAPTURE_PREFIX CONN : feed the greeting, then every later server frame of one connection (control parts, reply), ids patched to the app's request id.
import { readFileSync, readdirSync } from "node:fs";
const [bridge, prefix, conn] = process.argv.slice(2);
(0, eval)(readFileSync(bridge, "utf8"));
const B = (globalThis as any).Backplane; const key = "desktop.tail9e0cf2.ts.net:3815", ep = "1";
const j = (x: any) => typeof x === "string" ? JSON.parse(x) : x;
const frames = readdirSync(prefix.replace(/\/[^/]*$/, "")).filter(f => f.startsWith(prefix.split("/").pop()! + ".conn_" + conn + "_s2c_")).sort((a, b) => Number(a.match(/_(\d+)\.bin$/)![1]) - Number(b.match(/_(\d+)\.bin$/)![1]));
const dir = prefix.replace(/\/[^/]*$/, "") + "/";
B.start("replay", "{}"); B.hubs([key]); B.connection(key, ep, "view-v1"); B.online(key, true, ep);
let o = j(B.recv(key, readFileSync(dir + frames[0]).toString("base64"), ep, false, false));
const send = (o.cmds || []).find((c: any) => c.type === "send"); const idm = /(\d{15})/.exec(Buffer.from(send.data, "base64").toString("latin1"))![1];
let last: any = null, n = 0;
for (const f of frames.slice(1)) {
  if (/_s2c_(\d+)\.bin/.exec(f) && readFileSync(dir + f).length <= 5) continue; // heartbeats
  let buf = Buffer.from(readFileSync(dir + f).toString("latin1").replace(/\d{15}/g, idm), "latin1");
  o = j(B.recv(key, buf.toString("base64"), ep, false, false)); n++;
  if (o.sync?.length) last = o;
}
console.log("frames fed", n, "last sync", JSON.stringify(last?.sync), "screen", !!last?.screen);
console.log("checks", JSON.stringify((globalThis as any).__chk));
console.log("ctl", JSON.stringify((globalThis as any).__ctl?.slice(0,3)), "...", JSON.stringify((globalThis as any).__ctl?.slice(-3)), "n", (globalThis as any).__ctl?.length);
